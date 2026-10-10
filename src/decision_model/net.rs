//! 013 T002: the LibTorch model (`learning-worker` only). ModernBERT-large
//! exactly as the pinned checkpoint's `encoder/config.json` defines it, and
//! the choice head exactly as unchanged Laya `DecisionModel.forward`
//! (`laya/common.py` @4066d5d5, lines 181-198):
//!
//! * encoder: token embedding, bias-free embedding norm, 28 layers (layer 0
//!   without attention pre-normalization), global attention on every third
//!   layer with RoPE θ 160000 and local attention at inclusive distance
//!   <= 64 with RoPE θ 10000 otherwise, gated-GELU MLP 2624, final norm;
//! * head: `h + type_emb[choice=0]`, two pre-norm `nn.TransformerEncoderLayer`
//!   (d 1024, 16 heads, feedforward 4096, ReLU, dropout 0.1), the two marker
//!   rows, then LayerNorm → Linear → GELU(erf) → Linear and the upstream
//!   `masked_fill(~marker_mask, -1e4)`.
//!
//! Float32 on the CPU, batch one, so no padding is introduced. The encoder
//! is frozen: it runs without autograd, in evaluation mode, and is recomputed
//! for every example (no hidden-state cache; contract § Exact input and
//! identity). Only the contract's trainable set ([`super::trainable`]) is
//! optimizer-visible: both head layers, the scorer and `type_emb` row 0 as
//! its own tensor, so rows 1-2 get no gradient and no weight decay. AdamW
//! follows torch's single-tensor CPU implementation; the global-norm clip
//! follows `torch.nn.utils.clip_grad_norm_`. Checkpoint code never runs:
//! tensors are read by exact name and shape through the strict header
//! parser, and float16 data is upcast to float32.
use super::arch::*;
use super::recipe::{BETA1, BETA2, CLIP_GLOBAL_NORM, EPSILON, LEARNING_RATE, WEIGHT_DECAY};
use super::safetensors::{self, Dtype};
use super::{CHOICE_ROW, checkpoint_tensors, trainable};
use crate::error::{FResult, FoundryError};
use sha2::{Digest, Sha256};
use std::path::Path;
use tch::{Device, Kind, Tensor};

const CPU: Device = Device::Cpu;
/// The checkpoint is about 0.8 GiB; anything past this is not it.
const MAX_CHECKPOINT_BYTES: u64 = 2 << 30;
const MAX_CONFIG_BYTES: u64 = 64 * 1024;

fn fail(code: &'static str, message: impl Into<String>) -> FoundryError {
    FoundryError::Learning {
        code,
        message: message.into(),
    }
}

/// Evaluation (every dropout off) or training with head dropout `dropout`.
/// Training always uses [`HEAD_DROPOUT`]; `Train { dropout: 0.0 }` is the
/// parity suite's knob, and equals [`Mode::Eval`] exactly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Eval,
    Train { dropout: f64 },
}

impl Mode {
    fn dropout(self) -> (f64, bool) {
        match self {
            Mode::Eval => (0.0, false),
            Mode::Train { dropout } => (dropout, true),
        }
    }
}

struct EncoderLayer {
    wqkv: Tensor,
    wo: Tensor,
    wi: Tensor,
    mlp_wo: Tensor,
    attn_norm: Option<Tensor>,
    mlp_norm: Tensor,
}

struct Encoder {
    tok: Tensor,
    emb_norm: Tensor,
    layers: Vec<EncoderLayer>,
    final_norm: Tensor,
    /// Every encoder tensor by name, for the frozen hash.
    named: Vec<(String, Tensor)>,
}

/// One trainable tensor and its AdamW moments.
struct Param {
    name: String,
    value: Tensor,
    exp_avg: Tensor,
    exp_avg_sq: Tensor,
}

/// The loaded model.
pub struct Model {
    encoder: Encoder,
    params: Vec<Param>,
    /// `type_emb.weight` rows 1-2: frozen, never optimizer-visible.
    frozen_type_rows: Tensor,
    /// AdamW's step count since the last (re)load.
    steps: u64,
    pub source_dtype: Dtype,
    pub weights_sha256: String,
    pub encoder_config_sha256: String,
    pub encoder_parameters: usize,
    pub trainable_parameters: usize,
    /// Rows 1-2 of the type table plus the unused `act_head` and
    /// `temperature` tensors: validated, never used or trained.
    pub frozen_other_parameters: usize,
}

/// `[CLS]`-free encoder output: `[1, S, 1024]`, no autograd.
pub type Hidden = Tensor;

fn read_capped(path: &Path, cap: u64, code: &'static str) -> FResult<Vec<u8>> {
    use std::io::Read as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| fail(code, format!("{}: {e}", path.display())))?;
    let meta = file
        .metadata()
        .map_err(|e| fail(code, format!("{}: {e}", path.display())))?;
    if !meta.is_file() || meta.len() > cap {
        return Err(fail(
            code,
            format!(
                "{} is not a regular file of at most {cap} bytes",
                path.display()
            ),
        ));
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| fail(code, format!("{}: {e}", path.display())))?;
    if bytes.len() as u64 > cap {
        return Err(fail(
            code,
            format!("{} grew past {cap} bytes", path.display()),
        ));
    }
    Ok(bytes)
}

/// The encoder configuration must be exactly the architecture this code
/// implements; anything else is refused before a weight is read.
fn check_config(bytes: &[u8]) -> FResult<()> {
    let config: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| fail("checkpoint_invalid", format!("encoder/config.json: {e}")))?;
    let field = |name: &str| config.get(name).cloned().unwrap_or(serde_json::Value::Null);
    let expect = |name: &str, want: serde_json::Value| -> FResult<()> {
        let got = field(name);
        let same = match (got.as_f64(), want.as_f64()) {
            (Some(a), Some(b)) => a == b,
            _ => got == want,
        };
        if !same {
            return Err(fail(
                "checkpoint_invalid",
                format!("encoder/config.json {name} is {got}, the implemented model needs {want}"),
            ));
        }
        Ok(())
    };
    expect("model_type", "modernbert".into())?;
    expect("hidden_size", HIDDEN.into())?;
    expect("vocab_size", VOCAB.into())?;
    expect("num_hidden_layers", LAYERS.into())?;
    expect("num_attention_heads", HEADS.into())?;
    expect("intermediate_size", INTERMEDIATE.into())?;
    expect("global_attn_every_n_layers", GLOBAL_EVERY.into())?;
    expect("local_attention", (2 * LOCAL_WINDOW).into())?;
    expect("hidden_activation", "gelu".into())?;
    expect("norm_eps", NORM_EPS.into())?;
    expect("norm_bias", false.into())?;
    expect("attention_bias", false.into())?;
    expect("mlp_bias", false.into())?;
    let rope = field("rope_parameters");
    let theta = |kind: &str| rope[kind]["rope_theta"].as_f64();
    if theta("full_attention") != Some(GLOBAL_ROPE_THETA)
        || theta("sliding_attention") != Some(LOCAL_ROPE_THETA)
    {
        return Err(fail(
            "checkpoint_invalid",
            format!(
                "encoder/config.json rope_parameters {rope} are not {GLOBAL_ROPE_THETA}/{LOCAL_ROPE_THETA}"
            ),
        ));
    }
    Ok(())
}

fn kind_of(dtype: Dtype) -> Kind {
    match dtype {
        Dtype::F16 => Kind::Half,
        Dtype::BF16 => Kind::BFloat16,
        Dtype::F32 => Kind::Float,
    }
}

fn dims(shape: &[usize]) -> Vec<i64> {
    shape.iter().map(|d| *d as i64).collect()
}

fn all_finite(t: &Tensor) -> bool {
    t.isfinite().all().int64_value(&[]) != 0
}

/// Float32 copies of `entries` read from `data`, refused if any value is
/// nonfinite.
fn tensors_from(
    entries: &[safetensors::Entry],
    data: &[u8],
    code: &'static str,
) -> FResult<Vec<(String, Tensor)>> {
    entries
        .iter()
        .map(|entry| {
            let bytes = &data[entry.start as usize..entry.end as usize];
            let t = Tensor::f_from_data_size(bytes, &dims(&entry.shape), kind_of(entry.dtype))
                .map_err(|e| fail(code, format!("tensor {}: {e}", entry.name)))?
                .to_kind(Kind::Float);
            if !all_finite(&t) {
                return Err(fail(
                    "nonfinite_weight",
                    format!("tensor {} holds a nonfinite value", entry.name),
                ));
            }
            Ok((entry.name.clone(), t))
        })
        .collect()
}

/// Parse a whole safetensors file held in memory.
fn parse_file(bytes: &[u8], code: &'static str) -> FResult<Vec<safetensors::Entry>> {
    if bytes.len() < 8 {
        return Err(fail(code, "file shorter than its length prefix"));
    }
    let len = safetensors::header_len(bytes[..8].try_into().expect("8 bytes"), code)?;
    let data_start = 8 + len;
    if (bytes.len() as u64) < data_start {
        return Err(fail(code, "file shorter than its header"));
    }
    safetensors::parse_header(
        &bytes[8..data_start as usize],
        bytes.len() as u64 - data_start,
        code,
    )
}

fn data_of(bytes: &[u8]) -> &[u8] {
    let len = u64::from_le_bytes(bytes[..8].try_into().expect("8 bytes"));
    &bytes[8 + len as usize..]
}

fn layer_norm(x: &Tensor, weight: &Tensor, bias: Option<&Tensor>) -> Tensor {
    x.layer_norm([HIDDEN as i64], Some(weight), bias, NORM_EPS, false)
}

/// RoPE `cos`/`sin` for `seq` positions, built the way the reference builds
/// them: float32 `θ ** (arange(0, 64, 2) / 64)` and its reciprocal, then the
/// position outer product. (Scalar f64 construction rounds differently and
/// failed the feasibility tolerance; see docs/review/feasibility.md.)
fn rope(seq: i64, theta: f64) -> (Tensor, Tensor) {
    let half = (HEAD_DIM / 2) as i64;
    let exponent = Tensor::arange(half, (Kind::Float, CPU)) * 2.0 / HEAD_DIM as f64;
    let freq = Tensor::full([half], theta, (Kind::Float, CPU))
        .pow(&exponent)
        .reciprocal();
    let angles = Tensor::arange(seq, (Kind::Float, CPU)).unsqueeze(1) * freq.unsqueeze(0);
    let angles = Tensor::cat(&[&angles, &angles], 1)
        .unsqueeze(0)
        .unsqueeze(0);
    (angles.cos(), angles.sin())
}

fn rotate_half(x: &Tensor) -> Tensor {
    let half = (HEAD_DIM / 2) as i64;
    Tensor::cat(&[&-x.narrow(-1, half, half), &x.narrow(-1, 0, half)], -1)
}

/// `[1, S, 3·H·D]` → three `[1, H, S, D]`.
fn split_heads(qkv: &Tensor, seq: i64) -> (Tensor, Tensor, Tensor) {
    let qkv = qkv.reshape([1, seq, 3, HEADS as i64, HEAD_DIM as i64]);
    (
        qkv.select(2, 0).transpose(1, 2),
        qkv.select(2, 1).transpose(1, 2),
        qkv.select(2, 2).transpose(1, 2),
    )
}

fn merge_heads(x: &Tensor, seq: i64) -> Tensor {
    x.transpose(1, 2)
        .contiguous()
        .reshape([1, seq, HIDDEN as i64])
}

/// The upstream marker mask: a marker that is not valid scores −1e4. With
/// exactly two options both markers are always valid, so this is the
/// identity in the supported family; it is kept (and unit-tested with a
/// synthetic mask) so the head stays the upstream function.
pub fn apply_marker_mask(logits: &Tensor, valid: [bool; 2]) -> Tensor {
    let invalid = Tensor::from_slice(&[!valid[0], !valid[1]]).reshape([1, 2]);
    logits.masked_fill(&invalid, MASKED_LOGIT)
}

impl Model {
    /// Load the pinned checkpoint from `dir` (`model.safetensors` and
    /// `encoder/config.json`): the config must be the implemented
    /// architecture; the tensors must be exactly the checkpoint table in one
    /// dtype, finite, upcast to float32. The exact bytes are hashed for the
    /// identity the owner checks against its pins.
    pub fn load(dir: &Path) -> FResult<Self> {
        let config = read_capped(
            &dir.join("encoder").join("config.json"),
            MAX_CONFIG_BYTES,
            "checkpoint_invalid",
        )?;
        check_config(&config)?;
        let bytes = read_capped(
            &dir.join("model.safetensors"),
            MAX_CHECKPOINT_BYTES,
            "checkpoint_invalid",
        )?;
        let weights_sha256 = crate::digest(&bytes);
        let entries = parse_file(&bytes, "checkpoint_invalid")?;
        let source_dtype = entries
            .first()
            .map(|e| e.dtype)
            .ok_or_else(|| fail("checkpoint_invalid", "the checkpoint holds no tensor"))?;
        safetensors::check_tensors(
            &entries,
            &checkpoint_tensors(),
            source_dtype,
            "checkpoint_invalid",
        )?;
        let mut loaded: std::collections::BTreeMap<String, Tensor> =
            tensors_from(&entries, data_of(&bytes), "checkpoint_invalid")?
                .into_iter()
                .collect();
        drop(bytes);
        let mut take = |name: &str| -> Tensor {
            loaded
                .remove(name)
                .unwrap_or_else(|| panic!("{name} was checked present"))
        };

        let mut named = Vec::new();
        let mut keep = |name: String, t: Tensor| -> Tensor {
            named.push((name, t.shallow_clone()));
            t
        };
        let tok = keep(
            "encoder.embeddings.tok_embeddings.weight".into(),
            take("encoder.embeddings.tok_embeddings.weight"),
        );
        let emb_norm = keep(
            "encoder.embeddings.norm.weight".into(),
            take("encoder.embeddings.norm.weight"),
        );
        let mut layers = Vec::with_capacity(LAYERS);
        for layer in 0..LAYERS {
            let p = format!("encoder.layers.{layer}");
            let mut field = |suffix: &str| {
                let name = format!("{p}.{suffix}");
                let t = take(&name);
                keep(name, t)
            };
            layers.push(EncoderLayer {
                wqkv: field("attn.Wqkv.weight"),
                wo: field("attn.Wo.weight"),
                wi: field("mlp.Wi.weight"),
                mlp_wo: field("mlp.Wo.weight"),
                mlp_norm: field("mlp_norm.weight"),
                attn_norm: (layer > 0).then(|| field("attn_norm.weight")),
            });
        }
        let final_norm = keep(
            "encoder.final_norm.weight".into(),
            take("encoder.final_norm.weight"),
        );
        named.sort_by(|a, b| a.0.cmp(&b.0));
        let encoder_parameters = named.iter().map(|(_, t)| t.numel()).sum();

        let type_table = take("type_emb.weight");
        let mut params = Vec::with_capacity(31);
        for (name, _) in trainable() {
            let value = if name == CHOICE_ROW {
                type_table.get(0).copy()
            } else {
                take(&name)
            };
            params.push(Param {
                exp_avg: value.zeros_like(),
                exp_avg_sq: value.zeros_like(),
                value: value.set_requires_grad(true),
                name,
            });
        }
        let frozen_type_rows = type_table.narrow(0, 1, (TYPE_ROWS - 1) as i64).copy();
        let trainable_parameters = params.iter().map(|p| p.value.numel()).sum();
        // What remains is exactly the unused act head and the reference
        // temperature buffer: validated present, then dropped unused.
        let unused: usize = loaded.values().map(Tensor::numel).sum();
        debug_assert!(
            loaded
                .keys()
                .all(|n| n.starts_with("act_head.") || n == "temperature")
        );
        Ok(Self {
            encoder: Encoder {
                tok,
                emb_norm,
                layers,
                final_norm,
                named,
            },
            params,
            frozen_type_rows,
            steps: 0,
            source_dtype,
            weights_sha256,
            encoder_config_sha256: crate::digest(&config),
            encoder_parameters,
            trainable_parameters,
            frozen_other_parameters: frozen_type_rows_len() + unused,
        })
    }

    /// Replace every trainable tensor with a candidate head's (a base or an
    /// incumbent): exactly the trainable set, float32, finite. AdamW starts
    /// fresh (contract: no optimizer resume).
    pub fn load_head(&mut self, bytes: &[u8]) -> FResult<()> {
        let entries = parse_file(bytes, "artifact_invalid")?;
        safetensors::check_tensors(&entries, &trainable(), Dtype::F32, "artifact_invalid")?;
        let mut loaded: std::collections::BTreeMap<String, Tensor> =
            tensors_from(&entries, data_of(bytes), "artifact_invalid")?
                .into_iter()
                .collect();
        tch::no_grad(|| {
            for param in &mut self.params {
                let source = loaded.remove(&param.name).expect("checked present");
                param.value.copy_(&source);
                let _ = param.exp_avg.zero_();
                let _ = param.exp_avg_sq.zero_();
            }
        });
        self.steps = 0;
        Ok(())
    }

    /// The frozen encoder's output for `ids`: no autograd, evaluation mode.
    pub fn encode(&self, ids: &[u32]) -> Hidden {
        let enc = &self.encoder;
        tch::no_grad(|| {
            let ids: Vec<i64> = ids.iter().map(|id| i64::from(*id)).collect();
            let seq = ids.len() as i64;
            let ids = Tensor::from_slice(&ids);
            let mut h = layer_norm(
                &enc.tok.index_select(0, &ids).unsqueeze(0),
                &enc.emb_norm,
                None,
            );
            let global = rope(seq, GLOBAL_ROPE_THETA);
            let local = rope(seq, LOCAL_ROPE_THETA);
            let positions = Tensor::arange(seq, (Kind::Int64, CPU));
            // SDPA boolean mask: true where attention is allowed.
            let window = (positions.unsqueeze(0) - positions.unsqueeze(1))
                .abs()
                .le(LOCAL_WINDOW as i64);
            for (index, layer) in enc.layers.iter().enumerate() {
                let is_global = index % GLOBAL_EVERY == 0;
                let x = match &layer.attn_norm {
                    Some(norm) => layer_norm(&h, norm, None),
                    None => h.shallow_clone(),
                };
                let (q, k, v) = split_heads(&x.linear::<&Tensor>(&layer.wqkv, None), seq);
                let (cos, sin) = if is_global { &global } else { &local };
                let q = &q * cos + rotate_half(&q) * sin;
                let k = &k * cos + rotate_half(&k) * sin;
                let attended = Tensor::scaled_dot_product_attention(
                    &q,
                    &k,
                    &v,
                    (!is_global).then_some(&window),
                    0.0,
                    false,
                    None,
                    false,
                );
                h = &h + merge_heads(&attended, seq).linear::<&Tensor>(&layer.wo, None);
                let x = layer_norm(&h, &layer.mlp_norm, None).linear::<&Tensor>(&layer.wi, None);
                let width = INTERMEDIATE as i64;
                let x = x.narrow(-1, 0, width).gelu("none") * x.narrow(-1, width, width);
                h = &h + x.linear::<&Tensor>(&layer.mlp_wo, None);
            }
            layer_norm(&h, &enc.final_norm, None)
        })
    }

    fn param(&self, name: &str) -> &Tensor {
        &self
            .params
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("{name} is a trainable tensor"))
            .value
    }

    /// The choice head on encoder output: logits `[1, 2]` for the two
    /// marker rows. Autograd follows the trainable tensors when it is on.
    pub fn head_logits(&self, hidden: &Hidden, markers: [usize; 2], mode: Mode) -> Tensor {
        let (p, train) = mode.dropout();
        let seq = hidden.size()[1];
        let mut h = hidden + self.param(CHOICE_ROW).view([1, 1, HIDDEN as i64]);
        for layer in 0..HEAD_LAYERS {
            let name = |suffix: &str| format!("head.layers.{layer}.{suffix}");
            let w = |suffix: &str| self.param(&name(suffix));
            // Self-attention block (norm first): MHA with the packed input
            // projection, attention-probability dropout, `dropout1`.
            let y = layer_norm(&h, w("norm1.weight"), Some(w("norm1.bias")));
            let (q, k, v) = split_heads(
                &y.linear(
                    w("self_attn.in_proj_weight"),
                    Some(w("self_attn.in_proj_bias")),
                ),
                seq,
            );
            let attended = Tensor::scaled_dot_product_attention::<&Tensor>(
                &q, &k, &v, None, p, false, None, false,
            );
            let attended = merge_heads(&attended, seq).linear(
                w("self_attn.out_proj.weight"),
                Some(w("self_attn.out_proj.bias")),
            );
            h = &h + attended.dropout(p, train);
            // Feedforward block: linear1 → ReLU → dropout → linear2 →
            // `dropout2`.
            let y = layer_norm(&h, w("norm2.weight"), Some(w("norm2.bias")));
            let y = y
                .linear(w("linear1.weight"), Some(w("linear1.bias")))
                .relu()
                .dropout(p, train)
                .linear(w("linear2.weight"), Some(w("linear2.bias")));
            h = &h + y.dropout(p, train);
        }
        let index = Tensor::from_slice(&[markers[0] as i64, markers[1] as i64]);
        let rows = h.index_select(1, &index);
        let s = |name: &str| self.param(name);
        let logits = layer_norm(&rows, s("scorer.0.weight"), Some(s("scorer.0.bias")))
            .linear(s("scorer.1.weight"), Some(s("scorer.1.bias")))
            .gelu("none")
            .linear(s("scorer.3.weight"), Some(s("scorer.3.bias")))
            .squeeze_dim(-1);
        apply_marker_mask(&logits, [true, true])
    }

    /// Evaluation logits for one input, refused if nonfinite.
    pub fn logits(&self, ids: &[u32], markers: [usize; 2]) -> FResult<[f32; 2]> {
        let hidden = self.encode(ids);
        let logits = tch::no_grad(|| self.head_logits(&hidden, markers, Mode::Eval));
        to_pair(&logits, "nonfinite_logits")
    }

    fn zero_grads(&mut self) {
        for param in &mut self.params {
            param.value.zero_grad();
        }
    }

    /// Cross-entropy over the two marker logits against `target`, then
    /// backward: gradients are left on the trainable tensors. Returns the
    /// loss and the logits; refuses a nonfinite loss or gradient.
    pub fn backward(
        &mut self,
        hidden: &Hidden,
        markers: [usize; 2],
        target: usize,
        mode: Mode,
    ) -> FResult<(f64, [f32; 2])> {
        self.zero_grads();
        let logits = self.head_logits(hidden, markers, mode);
        let loss = logits.cross_entropy_for_logits(&Tensor::from_slice(&[target as i64]));
        let value = loss.double_value(&[]);
        if !value.is_finite() {
            return Err(fail("nonfinite_loss", format!("the loss is {value}")));
        }
        loss.backward();
        for param in &self.params {
            let grad = param.value.grad();
            if !grad.defined() || !all_finite(&grad) {
                return Err(fail(
                    "nonfinite_gradient",
                    format!("the gradient of {} is missing or nonfinite", param.name),
                ));
            }
        }
        let pair = to_pair(&logits.detach(), "nonfinite_logits")?;
        Ok((value, pair))
    }

    /// Clip the gradients to global norm 1.0 (returning the norm before
    /// clipping) and take one AdamW step. Refuses a nonfinite norm or an
    /// updated weight that is not finite.
    pub fn clip_and_step(&mut self) -> FResult<f64> {
        let grads: Vec<Tensor> = self.params.iter().map(|p| p.value.grad()).collect();
        let norms: Vec<Tensor> = grads.iter().map(Tensor::norm).collect();
        let total = Tensor::stack(&norms, 0).norm();
        let norm = total.double_value(&[]);
        if !norm.is_finite() {
            return Err(fail(
                "nonfinite_gradient",
                format!("the gradient norm is {norm}"),
            ));
        }
        self.steps += 1;
        let step = self.steps as f64;
        let (beta1, beta2) = (BETA1, BETA2);
        let bias_correction1 = 1.0 - beta1.powf(step);
        let bias_correction2 = 1.0 - beta2.powf(step);
        let step_size = LEARNING_RATE / bias_correction1;
        let bias_correction2_sqrt = bias_correction2.sqrt();
        tch::no_grad(|| {
            let coefficient = (CLIP_GLOBAL_NORM / (&total + 1e-6)).clamp_max(1.0);
            for (param, grad) in self.params.iter_mut().zip(grads) {
                let mut grad = grad;
                let _ = grad.g_mul_(&coefficient);
                let _ = param
                    .value
                    .g_mul_scalar_(1.0 - LEARNING_RATE * WEIGHT_DECAY);
                let _ = param.exp_avg.lerp_(&grad, 1.0 - beta1);
                let _ = param.exp_avg_sq.g_mul_scalar_(beta2);
                let _ = param.exp_avg_sq.g_add_(&(&grad * (1.0 - beta2) * &grad));
                let denom = param.exp_avg_sq.sqrt() / bias_correction2_sqrt + EPSILON;
                let _ = param
                    .value
                    .g_add_(&(&param.exp_avg * (-step_size) / &denom));
            }
        });
        if let Some(bad) = self.params.iter().find(|p| !all_finite(&p.value)) {
            return Err(fail(
                "nonfinite_weight",
                format!("{} is nonfinite after the update", bad.name),
            ));
        }
        Ok(norm)
    }

    /// One training update on one example (train mode, head dropout 0.1,
    /// encoder recomputed): loss and the gradient norm before clipping.
    pub fn train_step(
        &mut self,
        ids: &[u32],
        markers: [usize; 2],
        target: usize,
    ) -> FResult<(f64, f64)> {
        let hidden = self.encode(ids);
        let (loss, _) = self.backward(
            &hidden,
            markers,
            target,
            Mode::Train {
                dropout: HEAD_DROPOUT,
            },
        )?;
        let norm = self.clip_and_step()?;
        Ok((loss, norm))
    }

    /// The trainable tensors, in the trainable-set order.
    pub fn params(&self) -> Vec<(&str, &Tensor)> {
        self.params
            .iter()
            .map(|p| (p.name.as_str(), &p.value))
            .collect()
    }

    /// The current gradients, in the trainable-set order (undefined before
    /// the first backward).
    pub fn grads(&self) -> Vec<(&str, Tensor)> {
        self.params
            .iter()
            .map(|p| (p.name.as_str(), p.value.grad()))
            .collect()
    }

    /// `type_emb.weight` rows 1-2, frozen.
    pub fn frozen_type_rows(&self) -> &Tensor {
        &self.frozen_type_rows
    }

    /// SHA-256 over every encoder tensor's name and float32 little-endian
    /// bytes, in name order: identical before and after training. Hashed in
    /// 1 Mi-element slices, so the 51 M-element embedding is never copied
    /// whole.
    pub fn frozen_encoder_sha256(&self) -> String {
        const SLICE: i64 = 1 << 20;
        let mut hasher = Sha256::new();
        for (name, tensor) in &self.encoder.named {
            hasher.update(name.as_bytes());
            let flat = tensor.detach().contiguous().view([-1]);
            let total = flat.size()[0];
            let mut start = 0;
            while start < total {
                let len = SLICE.min(total - start);
                hasher.update(f32_bytes(&flat.narrow(0, start, len)));
                start += len;
            }
        }
        format!("{:x}", hasher.finalize())
    }

    /// `head.safetensors`: exactly the trainable set, float32, name order.
    pub fn head_bytes(&self) -> Vec<u8> {
        let table: Vec<(String, Vec<usize>)> = trainable();
        let (mut out, ranges) = safetensors::encode_f32_header(&table);
        let mut order: Vec<usize> = (0..table.len()).collect();
        order.sort_by_key(|i| ranges[*i].0);
        for index in order {
            out.extend_from_slice(&f32_bytes(&self.params[index].value));
        }
        out
    }
}

fn frozen_type_rows_len() -> usize {
    (TYPE_ROWS - 1) * HIDDEN
}

fn f32_bytes(tensor: &Tensor) -> Vec<u8> {
    let flat = tensor.detach().contiguous().view([-1]);
    let values = Vec::<f32>::try_from(&flat).expect("a float32 CPU tensor");
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn to_pair(logits: &Tensor, code: &'static str) -> FResult<[f32; 2]> {
    let values = Vec::<f32>::try_from(&logits.detach().contiguous().view([-1]))
        .map_err(|e| fail(code, format!("logits: {e}")))?;
    match values.as_slice() {
        [a, b] if a.is_finite() && b.is_finite() => Ok([*a, *b]),
        other => Err(fail(
            code,
            format!("logits {other:?} are not two finite values"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipped_adamw_matches_the_pytorch_213_cpu_reference() {
        // A small operation-level fixture, not checkpoint/model parity. Expected
        // values came from torch 2.13.0+cpu AdamW(foreach=False, fused=False),
        // the recipe above and clip_grad_norm_(max_norm=1). This exercises the
        // actual product optimizer, including autograd, clipping and moments.
        let empty = || Tensor::zeros([0], (Kind::Float, CPU));
        let value = Tensor::from_slice(&[1f32, -2.]).set_requires_grad(true);
        let mut model = Model {
            encoder: Encoder {
                tok: empty(),
                emb_norm: empty(),
                layers: vec![],
                final_norm: empty(),
                named: vec![],
            },
            params: vec![Param {
                name: "fixture".into(),
                exp_avg: Tensor::zeros_like(&value),
                exp_avg_sq: Tensor::zeros_like(&value),
                value,
            }],
            frozen_type_rows: empty(),
            steps: 0,
            source_dtype: Dtype::F32,
            weights_sha256: String::new(),
            encoder_config_sha256: String::new(),
            encoder_parameters: 0,
            trainable_parameters: 2,
            frozen_other_parameters: 0,
        };
        let reference = [
            ([6f32, 8.], 10., [0.9998989701271057, -2.0000979900360107]),
            (
                [-1., 2.],
                2.2360680103302,
                [0.9998887181282043, -2.0001959800720215],
            ),
            (
                [0.1, -0.2],
                0.22360679507255554,
                [0.9998721480369568, -2.0002596378326416],
            ),
        ];
        for (gradient, norm, expected) in reference {
            model.params[0].value.zero_grad();
            (&model.params[0].value * Tensor::from_slice(&gradient))
                .sum(Kind::Float)
                .backward();
            assert!((model.clip_and_step().unwrap() - norm).abs() < 1e-6);
            let actual = Vec::<f32>::try_from(&model.params[0].value).unwrap();
            for (got, want) in actual.into_iter().zip(expected) {
                assert!((f64::from(got) - want).abs() < 3e-7, "{got} vs {want}");
            }
        }
        // A failed numerical update must leave parameters and step count alone.
        let before = f32_bytes(&model.params[0].value);
        model.params[0].value.zero_grad();
        (&model.params[0].value * f64::NAN)
            .sum(Kind::Float)
            .backward();
        assert!(matches!(
            model.clip_and_step(),
            Err(FoundryError::Learning {
                code: "nonfinite_gradient",
                ..
            })
        ));
        assert_eq!(model.steps, 3);
        assert_eq!(f32_bytes(&model.params[0].value), before);
    }

    #[test]
    fn the_marker_mask_scores_an_invalid_marker_at_minus_1e4() {
        // F8: unreachable with two valid markers, still the upstream rule.
        let logits = Tensor::from_slice(&[0.25f32, -0.5]).reshape([1, 2]);
        let both =
            Vec::<f32>::try_from(&apply_marker_mask(&logits, [true, true]).view([-1])).unwrap();
        assert_eq!(both, [0.25, -0.5]);
        let second =
            Vec::<f32>::try_from(&apply_marker_mask(&logits, [true, false]).view([-1])).unwrap();
        assert_eq!(second, [0.25, -1e4]);
        let first =
            Vec::<f32>::try_from(&apply_marker_mask(&logits, [false, true]).view([-1])).unwrap();
        assert_eq!(first, [-1e4, -0.5]);
    }

    #[test]
    fn rope_frequencies_are_built_in_float32() {
        let (cos, sin) = rope(3, GLOBAL_ROPE_THETA);
        assert_eq!(cos.size(), [1, 1, 3, HEAD_DIM as i64]);
        // Position 0 rotates by nothing.
        let first = Vec::<f32>::try_from(&cos.select(2, 0).view([-1])).unwrap();
        assert!(first.iter().all(|v| *v == 1.0));
        let zero = Vec::<f32>::try_from(&sin.select(2, 0).view([-1])).unwrap();
        assert!(zero.iter().all(|v| *v == 0.0));
    }
}
