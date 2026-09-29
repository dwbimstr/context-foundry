use pyo3::{prelude::*, types::PyDict};
use std::time::Instant;
fn main() -> PyResult<()> {
    let path = std::env::args().nth(1).expect("model path");
    let start = Instant::now();
    Python::attach(|py| {
        let sys = py.import("sys")?;
        sys.getattr("path")?.call_method1("insert", (0, &path))?;
        let module = py.import("nemotron3_embed_mlx")?;
        let loaded = module.getattr("load")?.call1((&path,))?;
        let load_ms = start.elapsed().as_millis();
        let model = loaded.get_item(0)?;
        let tokenizer = loaded.get_item(1)?;
        let args = PyDict::new(py);
        args.set_item("input_type", "query")?;
        args.set_item("batch_size", 1)?;
        args.set_item("max_length", 512)?;
        let a = module.getattr("encode")?.call(
            (
                &model,
                &tokenizer,
                vec!["Where is the parser for configuration records?"],
            ),
            Some(&args),
        )?;
        let rows: Vec<Vec<f32>> = a.call_method0("tolist")?.extract()?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].len(), 2048);
        assert!(rows[0].iter().all(|x| x.is_finite()));
        let norm: f32 = rows[0].iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
        let tok_args = PyDict::new(py);
        tok_args.set_item("truncation", false)?;
        let long = format!("query: {}", "parser ".repeat(600));
        let count: usize = tokenizer
            .call((&long,), Some(&tok_args))?
            .get_item("input_ids")?
            .len()?;
        assert!(count > 512);
        println!(
            "{}",
            serde_json::json!({"bridge":"rust-pyo3-publisher", "dimensions":rows[0].len(),"finite":true,"norm":norm,"load_ms":load_ms,"total_ms":start.elapsed().as_millis(),"oversize_tokens":count,"oversize_refused_before_encode":true})
        );
        Ok(())
    })
}
