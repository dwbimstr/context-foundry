//! Disposable synthetic feasibility fixture, not a production model implementation.
use pyo3::{prelude::*, types::PyDict};
use std::{collections::HashMap, fs, time::Instant};
use tch::{Device, Kind, Tensor};
type Weights = HashMap<String, Tensor>;
fn linear(x: &Tensor, w: &Weights, p: &str) -> Tensor {
    x.linear(&w[&format!("{p}.weight")], w.get(&format!("{p}.bias")))
}
fn norm(x: &Tensor, w: &Weights, p: &str) -> Tensor {
    x.layer_norm(
        [1024],
        Some(&w[&format!("{p}.weight")]),
        w.get(&format!("{p}.bias")),
        1e-5,
        false,
    )
}
fn attn(x: &Tensor, w: &Weights, p: &str, encoder: bool, local: bool) -> Tensor {
    let s = x.size()[1];
    let qkv = if encoder {
        linear(x, w, &format!("{p}.Wqkv"))
    } else {
        x.linear(
            &w[&format!("{p}.in_proj_weight")],
            Some(&w[&format!("{p}.in_proj_bias")]),
        )
    };
    let qkv = qkv.reshape([1, s, 3, 16, 64]);
    let mut q = qkv.select(2, 0).transpose(1, 2);
    let mut k = qkv.select(2, 1).transpose(1, 2);
    let v = qkv.select(2, 2).transpose(1, 2);
    if encoder {
        let theta: f64 = if local { 10000. } else { 160000. };
        let freq = Tensor::full([32], theta, (Kind::Float, Device::Cpu))
            .pow(&(Tensor::arange(32, (Kind::Float, Device::Cpu)) * 2. / 64.))
            .reciprocal();
        let angles = Tensor::arange(s, (Kind::Float, Device::Cpu)).unsqueeze(1) * freq.unsqueeze(0);
        let angles = Tensor::cat(&[&angles, &angles], 1)
            .unsqueeze(0)
            .unsqueeze(0);
        let rotate = |z: &Tensor| Tensor::cat(&[&-z.narrow(-1, 32, 32), &z.narrow(-1, 0, 32)], -1);
        q = &q * angles.cos() + rotate(&q) * angles.sin();
        k = &k * angles.cos() + rotate(&k) * angles.sin();
    }
    let mut scores = q.matmul(&k.transpose(-2, -1)) / 8.;
    if local {
        let pos = Tensor::arange(s, (Kind::Int64, Device::Cpu));
        let mask = (pos.unsqueeze(0) - pos.unsqueeze(1)).abs().gt(64);
        scores = scores.masked_fill(&mask, -1e30);
    }
    let out = scores
        .softmax(-1, Kind::Float)
        .matmul(&v)
        .transpose(1, 2)
        .contiguous()
        .reshape([1, s, 1024]);
    linear(
        &out,
        w,
        &format!("{p}.{}", if encoder { "Wo" } else { "out_proj" }),
    )
}
fn encoder(ids: &Tensor, w: &Weights) -> Tensor {
    let mut h = norm(
        &w["encoder.embeddings.tok_embeddings.weight"]
            .index_select(0, &ids.view([-1]))
            .unsqueeze(0),
        w,
        "encoder.embeddings.norm",
    );
    for i in 0..28 {
        let p = format!("encoder.layers.{i}");
        let x = if i == 0 {
            h.shallow_clone()
        } else {
            norm(&h, w, &format!("{p}.attn_norm"))
        };
        h = &h + attn(&x, w, &format!("{p}.attn"), true, i % 3 != 0);
        let x = linear(
            &norm(&h, w, &format!("{p}.mlp_norm")),
            w,
            &format!("{p}.mlp.Wi"),
        );
        let x = x.narrow(-1, 0, 2624).gelu("none") * x.narrow(-1, 2624, 2624);
        h = &h + linear(&x, w, &format!("{p}.mlp.Wo"));
    }
    norm(&h, w, "encoder.final_norm")
}
fn head(h: &Tensor, markers: &Tensor, w: &Weights) -> Tensor {
    let mut h = h + w["type_emb.weight"].get(0).reshape([1, 1, 1024]);
    for i in 0..2 {
        let p = format!("head.layers.{i}");
        h = &h
            + attn(
                &norm(&h, w, &format!("{p}.norm1")),
                w,
                &format!("{p}.self_attn"),
                false,
                false,
            );
        h = &h
            + linear(
                &linear(
                    &norm(&h, w, &format!("{p}.norm2")),
                    w,
                    &format!("{p}.linear1"),
                )
                .relu(),
                w,
                &format!("{p}.linear2"),
            );
    }
    let h = h.index_select(1, markers);
    linear(
        &linear(&norm(&h, w, "scorer.0"), w, "scorer.1").gelu("none"),
        w,
        "scorer.3",
    )
    .squeeze_dim(-1)
}
fn reference(root: &str, encoder_grad: bool) -> PyResult<()> {
    Python::attach(|py| {
        let torch = py.import("torch")?;
        torch.call_method1("set_num_threads", (2,))?;
        py.import("sys")?
            .getattr("path")?
            .call_method1("insert", (0, format!("{root}/input/reference")))?;
        let common = py.import("common")?;
        let tf = py.import("transformers")?;
        let cfg = tf.getattr("ModernBertConfig")?.call_method1(
            "from_json_file",
            (format!(
                "{root}/input/laya-typed-decisions/encoder_config.json"
            ),),
        )?;
        cfg.setattr("_attn_implementation", "eager")?;
        let enc = tf.getattr("ModernBertModel")?.call1((&cfg,))?;
        let m = common.getattr("DecisionModel")?.call1((enc, 2))?;
        let weights = py
            .import("safetensors.torch")?
            .getattr("load_file")?
            .call1((format!(
                "{root}/input/laya-typed-decisions/model.safetensors"
            ),))?;
        m.call_method1("load_state_dict", (&weights,))?;
        drop(weights);
        m.call_method0("float")?;
        m.call_method0("eval")?;
        // Freeze only encoder; reference gradient is for the decision head.
        for item in m
            .getattr("encoder")?
            .call_method0("parameters")?
            .try_iter()?
        {
            item?.call_method1("requires_grad_", (false,))?;
        }
        if encoder_grad {
            for i in 0..2 {
                m.getattr("encoder")?
                    .getattr("layers")?
                    .get_item(i)?
                    .getattr("attn")?
                    .getattr("Wqkv")?
                    .getattr("weight")?
                    .call_method1("requires_grad_", (true,))?;
            }
        }
        let opts = PyDict::new(py);
        opts.set_item("local_files_only", true)?;
        let tok = tf.getattr("AutoTokenizer")?.call_method(
            "from_pretrained",
            (format!("{root}/input/laya-typed-decisions/tokenizer"),),
            Some(&opts),
        )?;
        let q = PyDict::new(py);
        q.set_item("t", "choice")?;
        q.set_item("ins", "Choose a retrieval strategy.")?;
        let crit = PyDict::new(py);
        crit.set_item("search", "find source text")?;
        crit.set_item("graph", "follow symbol relationships")?;
        q.set_item("crit", crit)?;
        let state = format!(
            "Find callers of parse_config. {}",
            "synthetic context ".repeat(45)
        );
        let seq = common
            .getattr("build_sequence")?
            .call1((&tok, state, &q, 1024, 256))?;
        let ids: Vec<i64> = seq.get_item(0)?.extract()?;
        let markers: Vec<i64> = seq.get_item(1)?.extract()?;
        let x = torch.call_method1("tensor", (vec![ids.clone()],))?;
        let mask = torch.call_method1("ones_like", (&x,))?;
        let mp = torch.call_method1("tensor", (vec![markers.clone()],))?;
        let mm = torch
            .call_method1("ones_like", (&mp,))?
            .call_method0("bool")?;
        let qt = torch.call_method1("tensor", (vec![0i64],))?;
        let enc_args = PyDict::new(py);
        enc_args.set_item("input_ids", &x)?;
        enc_args.set_item("attention_mask", &mask)?;
        let hidden = m
            .getattr("encoder")?
            .call((), Some(&enc_args))?
            .getattr("last_hidden_state")?;
        let out = m.call((&x, &mask, &mp, &mm, &qt), None)?.get_item(0)?;
        let label = torch.call_method1("tensor", (vec![0i64],))?;
        let loss = py
            .import("torch.nn.functional")?
            .getattr("cross_entropy")?
            .call1((&out, label))?;
        loss.call_method0("backward")?;
        let state = m.call_method0("state_dict")?;
        let grad = m
            .getattr("scorer")?
            .get_item(3)?
            .getattr("weight")?
            .getattr("grad")?;
        let tensors = PyDict::new(py);
        tensors.set_item(
            "hidden",
            hidden.call_method0("detach")?.call_method0("contiguous")?,
        )?;
        tensors.set_item(
            "logits",
            out.call_method0("detach")?.call_method0("contiguous")?,
        )?;
        tensors.set_item("scorer_gradient", grad.call_method0("contiguous")?)?;
        if encoder_grad {
            for i in 0..2 {
                tensors.set_item(
                    format!("qkv_gradient_{i}"),
                    m.getattr("encoder")?
                        .getattr("layers")?
                        .get_item(i)?
                        .getattr("attn")?
                        .getattr("Wqkv")?
                        .getattr("weight")?
                        .getattr("grad")?
                        .call_method0("contiguous")?,
                )?;
            }
        }
        py.import("safetensors.torch")?
            .getattr("save_file")?
            .call1((tensors, format!("{root}/output/reference.safetensors")))?;
        let logits: Vec<Vec<f32>> = out.call_method0("tolist")?.extract()?;
        let _ = state;
        fs::write(
            format!("{root}/output/sequence.json"),
            serde_json::to_vec(&serde_json::json!({"ids":ids,"markers":markers})).unwrap(),
        )
        .unwrap();
        println!(
            "{}",
            serde_json::json!({"reference":"unchanged Laya plus transformers","sequence_tokens":ids.len(),"logits":logits})
        );
        Ok(())
    })
}
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let root = &args[2];
    let start = Instant::now();
    let encoder_grad = args[1].ends_with("encoder");
    if args[1].starts_with("reference") {
        reference(root, encoder_grad).map_err(|e| anyhow::anyhow!("{e}"))?;
        return Ok(());
    }
    tch::set_num_threads(2);
    let mut w: Weights = Tensor::read_safetensors(format!(
        "{root}/input/laya-typed-decisions/model.safetensors"
    ))?
    .into_iter()
    .map(|(k, v)| (k, v.to_kind(Kind::Float)))
    .collect();
    let seq: serde_json::Value =
        serde_json::from_slice(&fs::read(format!("{root}/output/sequence.json"))?)?;
    let ids: Vec<i64> = serde_json::from_value(seq["ids"].clone())?;
    let markers: Vec<i64> = serde_json::from_value(seq["markers"].clone())?;
    let markers = Tensor::from_slice(&markers);
    let r: Weights = Tensor::read_safetensors(format!("{root}/output/reference.safetensors"))?
        .into_iter()
        .collect();
    if encoder_grad {
        for i in 0..2 {
            let _ = w[&format!("encoder.layers.{i}.attn.Wqkv.weight")].set_requires_grad(true);
        }
    }
    let h = if encoder_grad {
        encoder(&Tensor::from_slice(&ids), &w)
    } else {
        tch::no_grad(|| encoder(&Tensor::from_slice(&ids), &w))
    };
    let hidden_error = (&h - &r["hidden"]).abs().max().double_value(&[]);
    for (k, v) in &w {
        if !k.starts_with("encoder.") && !k.starts_with("act_head.") && k != "temperature" {
            let _ = v.set_requires_grad(true);
        }
    }
    let logits = head(&h, &markers, &w);
    let logits_error = (&logits - &r["logits"]).abs().max().double_value(&[]);
    logits
        .cross_entropy_for_logits(&Tensor::from_slice(&[0i64]))
        .backward();
    let gradient_error = (w["scorer.3.weight"].grad() - &r["scorer_gradient"])
        .abs()
        .max()
        .double_value(&[]);
    if encoder_grad {
        let errors: Vec<f64> = (0..2)
            .map(|i| {
                (w[&format!("encoder.layers.{i}.attn.Wqkv.weight")].grad()
                    - &r[&format!("qkv_gradient_{i}")])
                    .abs()
                    .max()
                    .double_value(&[])
            })
            .collect();
        let qk_nonzero: Vec<bool> = (0..2)
            .map(|i| {
                w[&format!("encoder.layers.{i}.attn.Wqkv.weight")]
                    .grad()
                    .narrow(0, 0, 2048)
                    .abs()
                    .max()
                    .double_value(&[])
                    > 0.
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({"scope":"global layer0 and local layer1 QKV gradients through Rust ModernBERT; no optimizer step","qkv_gradient_max_abs_errors":errors,"qk_nonzero":qk_nonzero,"hidden_max_abs_error":hidden_error,"logits_max_abs_error":logits_error,"elapsed_ms":start.elapsed().as_millis()})
        );
        anyhow::ensure!(
            errors.iter().all(|v| v.is_finite() && *v < 1e-5) && qk_nonzero.iter().all(|v| *v),
            "encoder gradient parity failed"
        );
        return Ok(());
    }
    let before = w["scorer.3.weight"].copy();
    tch::no_grad(|| {
        for (k, v) in &mut w {
            if !k.starts_with("encoder.") && v.grad().defined() {
                let updated = &*v - v.grad() * 1e-3;
                v.copy_(&updated);
            }
        }
    });
    let change = (&w["scorer.3.weight"] - before)
        .abs()
        .max()
        .double_value(&[]);
    let checkpoint: Vec<(&str, &Tensor)> = w
        .iter()
        .filter(|(k, _)| !k.starts_with("encoder."))
        .map(|(k, v)| (k.as_str(), v))
        .collect();
    Tensor::write_safetensors(&checkpoint, format!("{root}/output/head.safetensors"))?;
    let after = head(&h, &markers, &w).detach();
    for (k, v) in Tensor::read_safetensors(format!("{root}/output/head.safetensors"))? {
        w.insert(k, v);
    }
    let reload_error = (after - head(&h, &markers, &w))
        .abs()
        .max()
        .double_value(&[]);
    let encoder_gradients = w
        .iter()
        .any(|(k, v)| k.starts_with("encoder.") && v.grad().defined());
    println!(
        "{}",
        serde_json::json!({"scope":"one unpadded fixture, frozen Rust ModernBERT encoder plus choice head, one SGD step","sequence_tokens":ids.len(),"hidden_max_abs_error":hidden_error,"logits_max_abs_error":logits_error,"scorer_gradient_max_abs_error":gradient_error,"scorer_weight_change":change,"reload_max_abs_error":reload_error,"encoder_gradients":encoder_gradients,"elapsed_ms":start.elapsed().as_millis()})
    );
    anyhow::ensure!(
        hidden_error < 1e-5
            && logits_error < 1e-5
            && gradient_error < 1e-5
            && change > 0.
            && reload_error == 0.
            && !encoder_gradients,
        "fixture parity failed"
    );
    Ok(())
}
