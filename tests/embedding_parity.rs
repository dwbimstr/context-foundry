//! Portable real-worker receipts. CPU and Metal keep distinct cache identities;
//! comparison establishes numerical parity only for the recorded exact inputs.
#![cfg(feature = "semantic")]
use context_foundry::Control;
use context_foundry::neural::{
    profile::SemanticProfile,
    provider::{CPU_ADAPTER_REVISION, FunctionDescriptor, METAL_ADAPTER_REVISION, TokenizedInput},
    supervisor,
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    v: u32,
    os: String,
    arch: String,
    worker_sha256: String,
    descriptor: FunctionDescriptor,
    input_ids: Vec<Vec<u32>>,
    vectors: Vec<Vec<f32>>,
    elapsed_ms: u64,
}

fn compare(a: &Receipt, b: &Receipt) -> Result<f64, String> {
    let mut descriptor = a.descriptor.clone();
    descriptor.adapter_revision = b.descriptor.adapter_revision;
    if a.v != 1
        || b.v != 1
        || descriptor != b.descriptor
        || a.input_ids != b.input_ids
        || a.input_ids.is_empty()
        || a.input_ids.len() != a.vectors.len()
        || b.input_ids.len() != b.vectors.len()
    {
        return Err("incompatible or incomplete receipts".into());
    }
    let mut minimum = 1.0f64;
    for (x, y) in a.vectors.iter().zip(&b.vectors) {
        if x.len() != a.descriptor.dims()
            || y.len() != x.len()
            || !x.iter().chain(y).all(|v| v.is_finite())
        {
            return Err("invalid vector dimensions or values".into());
        }
        let norm = |v: &[f32]| v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
        let (nx, ny) = (norm(x), norm(y));
        if (nx - 1.).abs() > 0.0001 || (ny - 1.).abs() > 0.0001 {
            return Err("vector not normalized".into());
        }
        let cosine = x
            .iter()
            .zip(y)
            .map(|(x, y)| f64::from(*x) * f64::from(*y))
            .sum::<f64>()
            / (nx * ny);
        if cosine < 0.9999 {
            return Err(format!("cosine below 0.9999: {cosine}"));
        }
        minimum = minimum.min(cosine);
    }
    Ok(minimum)
}

#[test]
fn comparison_requires_exact_inputs_model_geometry_and_finite_normalized_vectors() {
    let descriptor = context_foundry::neural::worker_runtime::fake_descriptor();
    let mut vector = vec![0.; descriptor.dims()];
    vector[0] = 1.;
    let a = Receipt {
        v: 1,
        os: "fixture".into(),
        arch: "fixture".into(),
        worker_sha256: "fixture".into(),
        descriptor,
        input_ids: vec![vec![1, 2, 3]],
        vectors: vec![vector],
        elapsed_ms: 0,
    };
    assert!(compare(&a, &a).is_ok());
    let mut b = a.clone();
    b.descriptor.adapter_revision += 1;
    assert!(
        compare(&a, &b).is_ok(),
        "backend revisions may differ; cache keys still must not be shared"
    );
    b.input_ids[0][0] += 1;
    assert!(compare(&a, &b).is_err());
    let mut b = a.clone();
    b.descriptor.pooling = "last".into();
    assert!(compare(&a, &b).is_err());
    let mut b = a.clone();
    b.vectors[0][0] = f32::NAN;
    assert!(compare(&a, &b).is_err());
    let mut b = a.clone();
    b.vectors[0][0] = 0.5;
    assert!(compare(&a, &b).is_err());
    let mut b = a.clone();
    b.vectors[0].pop();
    assert!(compare(&a, &b).is_err());
    let mut b = a.clone();
    b.vectors.clear();
    assert!(compare(&a, &b).is_err());
}

#[test]
#[ignore = "real native CPU or Metal worker; explicit profile and new receipt path outside Git"]
fn real_native_embedding_receipt() {
    let path = |key| PathBuf::from(std::env::var_os(key).expect(key));
    let output = path("FOUNDRY_EMBED_RECEIPT");
    assert!(output.is_absolute() && !output.starts_with(env!("CARGO_MANIFEST_DIR")));
    assert!(!output.exists(), "preserve previous receipts");
    let mut profile = SemanticProfile::load(&path("FOUNDRY_EMBED_PROFILE")).unwrap();
    let revision = match std::env::consts::OS {
        "linux" => CPU_ADAPTER_REVISION,
        "macos" => METAL_ADAPTER_REVISION,
        other => panic!("unsupported native validation target: {other}"),
    };
    assert_eq!(profile.descriptor.adapter_revision, revision);
    assert_eq!(
        profile.descriptor.input_limits().tokens,
        8192,
        "full production geometry required"
    );
    assert!(profile.document_limits().inputs >= 4);
    profile.card_tokens = 8192;
    let started = Instant::now();
    let control = Control::with_deadline(started + Duration::from_secs(600));
    let tokenizer =
        tokenizers::Tokenizer::from_file(profile.model_dir.join("tokenizer.json")).unwrap();
    let mut inputs: Vec<_> = [
        "query: retrieve retained records",
        "设置检索边界 — 🧩",
        "first\r\nsecond",
        "fn parse() {}",
    ]
    .iter()
    .map(|s| TokenizedInput {
        ids: tokenizer
            .encode(*s, profile.descriptor.add_special_tokens)
            .unwrap()
            .get_ids()
            .to_vec(),
    })
    .collect();
    let mut provider = supervisor::acquire_until(&profile, true, &control).unwrap();
    let mut vectors = Vec::new();
    for input in &inputs {
        vectors.push(
            provider
                .embed_documents(std::slice::from_ref(input), &control)
                .unwrap()
                .remove(0),
        );
    }
    let receipt = |inputs: &[TokenizedInput], vectors| Receipt {
        v: 1,
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        worker_sha256: profile.worker.executable_sha256.clone(),
        descriptor: profile.descriptor.clone(),
        input_ids: inputs.iter().map(|i| i.ids.clone()).collect(),
        vectors,
        elapsed_ms: started.elapsed().as_millis() as u64,
    };
    let singles = receipt(&inputs, vectors.clone());
    let batched = receipt(
        &inputs,
        provider.embed_documents(&inputs, &control).unwrap(),
    );
    compare(&singles, &batched).unwrap();
    let mut full = inputs[0].clone();
    full.ids.resize(8192, full.ids[1]);
    vectors.push(
        provider
            .embed_documents(std::slice::from_ref(&full), &control)
            .unwrap()
            .remove(0),
    );
    inputs.push(full);
    let result = receipt(&inputs, vectors);
    compare(&result, &result).unwrap();
    drop(provider);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .unwrap();
    serde_json::to_writer_pretty(file, &result).unwrap();
}

#[test]
#[ignore = "requires separately captured Linux CPU and macOS Metal receipts"]
fn compare_cpu_and_metal_receipts() {
    let read = |key| -> Receipt {
        let path = std::env::var_os(key).expect(key);
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.len() <= 2 * 1024 * 1024);
        serde_json::from_slice(&bytes).unwrap()
    };
    let cpu = read("FOUNDRY_EMBED_CPU_RECEIPT");
    let metal = read("FOUNDRY_EMBED_METAL_RECEIPT");
    assert_eq!(
        (cpu.os.as_str(), cpu.descriptor.adapter_revision),
        ("linux", CPU_ADAPTER_REVISION)
    );
    assert_eq!(
        (
            metal.os.as_str(),
            metal.arch.as_str(),
            metal.descriptor.adapter_revision
        ),
        ("macos", "aarch64", METAL_ADAPTER_REVISION)
    );
    assert_eq!(cpu.descriptor.input_limits().tokens, 8192);
    assert!(cpu.input_ids.iter().any(|i| i.len() == 8192));
    let minimum = compare(&cpu, &metal).unwrap();
    println!(
        "{}",
        serde_json::json!({"scope":"recorded_cpu_metal_numerical_parity_not_package_or_hardware_pressure_acceptance","minimum_cosine":minimum,"cpu_worker":cpu.worker_sha256,"metal_worker":metal.worker_sha256,"cpu_function":cpu.descriptor.digest(),"metal_function":metal.descriptor.digest()})
    );
}
