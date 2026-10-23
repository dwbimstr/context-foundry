//! Linux consumers of the shared embedding supervisor and descriptor-only worker.
#![cfg(target_os = "linux")]

use context_foundry::Control;
use context_foundry::neural::{
    profile::{CpuLimits, SemanticProfile, WorkerSpec},
    provider::{EmbeddingProvider, ProviderError, TokenizedInput},
    supervisor::{self, WorkerProvider},
    worker_runtime,
};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn profile(dir: &Path) -> SemanticProfile {
    let model_dir = dir.join("model");
    let bundle = dir.join("worker");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::create_dir_all(&bundle).unwrap();
    let binary = std::fs::read(env!("CARGO_BIN_EXE_foundry-embed-fake")).unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_foundry-embed-fake"),
        bundle.join("foundry-embed"),
    )
    .unwrap();
    let descriptor = worker_runtime::fake_descriptor();
    for file in &descriptor.artifact_files {
        std::fs::write(model_dir.join(&file.name), []).unwrap();
    }
    SemanticProfile {
        v: 2,
        name: "linux-worker".into(),
        model_dir,
        worker: WorkerSpec {
            bundle,
            executable_sha256: context_foundry::digest(&binary),
            scratch_root: dir.join("scratch"),
        },
        descriptor,
        query_template: "query: {text}".into(),
        card_tokens: 128,
        representation: context_foundry::neural::partition::Representation::Address,
        batch: 8,
        memory_ceiling_bytes: 512 * 1024 * 1024,
        load_timeout_seconds: 10,
        cpu: CpuLimits {
            threads: 2,
            seconds: 30,
        },
    }
}

fn input() -> Vec<TokenizedInput> {
    vec![TokenizedInput { ids: vec![1, 2, 3] }]
}

#[test]
fn supervised_linux_worker_enforces_isolation_and_serves_then_reaps() {
    let dir = tempfile::tempdir().unwrap();
    let profile = profile(dir.path());
    assert!(matches!(
        supervisor::acquire_until(&profile, false, &Control::unbounded()),
        Err(ProviderError::IsolationUnavailable(_))
    ));
    let mut worker = WorkerProvider::launch(
        &profile,
        vec!["--isolation-probe".into(), "--stray-stdout".into()],
    )
    .unwrap();
    let vectors = worker
        .embed_documents(
            &input(),
            &Control::with_deadline(Instant::now() + Duration::from_secs(5)),
        )
        .unwrap();
    assert_eq!((vectors.len(), vectors[0].len()), (1, 768));
    assert!(vectors[0].iter().all(|v| v.is_finite()));
    assert!(worker.stderr_excerpt().contains("stray stdout"));
    worker.shutdown();
    assert!(worker.stopped());
    assert_eq!(
        std::fs::read_dir(&profile.worker.scratch_root)
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn linux_worker_rejects_changed_artifacts_and_corrupt_replies() {
    let dir = tempfile::tempdir().unwrap();
    let profile = profile(dir.path());
    let mut worker =
        WorkerProvider::launch(&profile, vec!["--reply-fault".into(), "nan".into()]).unwrap();
    assert!(
        worker
            .embed_documents(
                &input(),
                &Control::with_deadline(Instant::now() + Duration::from_secs(5))
            )
            .is_err()
    );
    worker.shutdown();
    let mut worker = WorkerProvider::launch(&profile, vec![]).unwrap();
    std::fs::write(profile.model_dir.join(&profile.descriptor.gguf), b"changed").unwrap();
    assert!(
        worker
            .embed_documents(
                &input(),
                &Control::with_deadline(Instant::now() + Duration::from_secs(5))
            )
            .is_err()
    );
    assert!(worker.stopped());
}

#[test]
#[ignore = "subprocess helper for the owner-death test"]
fn linux_owner_helper() {
    let Some(path) = std::env::var_os("FOUNDRY_LINUX_OWNER_PROFILE") else {
        return;
    };
    let profile = SemanticProfile::load(Path::new(&path)).unwrap();
    let worker = WorkerProvider::launch(&profile, vec![]).unwrap();
    println!("WORKER_PID={}", worker.worker_pid().unwrap());
    std::thread::sleep(Duration::from_secs(20));
}

#[test]
fn owner_death_closes_linux_worker_liveness() {
    let dir = tempfile::tempdir().unwrap();
    let profile = profile(dir.path());
    let path = dir.path().join("profile.json");
    std::fs::write(&path, serde_json::to_vec(&profile).unwrap()).unwrap();
    let mut owner = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "linux_owner_helper", "--ignored", "--nocapture"])
        .env("FOUNDRY_LINUX_OWNER_PROFILE", &path)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = owner.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(pid) = line.strip_prefix("WORKER_PID=") {
                let _ = tx.send(pid.parse::<u32>().unwrap());
                break;
            }
        }
    });
    let pid = rx.recv_timeout(Duration::from_secs(15));
    owner.kill().unwrap();
    owner.wait().unwrap();
    let pid = pid.expect("worker must report readiness before owner is killed");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let status = std::fs::read_to_string(format!("/proc/{pid}/stat"));
        if status.is_err() || status.unwrap().split_once(") ").unwrap().1.starts_with('Z') {
            break;
        }
        assert!(Instant::now() < deadline, "worker outlived owner");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(feature = "semantic")]
#[test]
#[ignore = "requires a verified FOUNDRY_EMBED_PROFILE and the real native worker"]
fn real_linux_worker_full_geometry() {
    let mut profile = SemanticProfile::load(Path::new(
        &std::env::var_os("FOUNDRY_EMBED_PROFILE").expect("explicit profile"),
    ))
    .unwrap();
    // Admit full-window documents explicitly; a normal body profile may select
    // much smaller cards even though its worker supports the complete geometry.
    profile.card_tokens = profile.descriptor.input_limits().tokens as u32;
    assert_eq!(
        profile.descriptor.adapter_revision,
        context_foundry::neural::provider::CPU_ADAPTER_REVISION
    );
    let started = Instant::now();
    let mut worker = supervisor::acquire_until(&profile, true, &Control::unbounded()).unwrap();
    let tokenizer =
        tokenizers::Tokenizer::from_file(profile.model_dir.join("tokenizer.json")).unwrap();
    let inputs: Vec<_> = [
        "query: retrieve retained records",
        "设置检索边界 — 🧩",
        "first\r\nsecond",
        "fn parse() {}",
    ]
    .iter()
    .map(|text| TokenizedInput {
        ids: tokenizer.encode(*text, true).unwrap().get_ids().to_vec(),
    })
    .collect();
    let deadline = || Instant::now() + Duration::from_secs(240);
    let single: Vec<_> = inputs
        .iter()
        .map(|i| {
            worker
                .embed_documents(std::slice::from_ref(i), &Control::with_deadline(deadline()))
                .unwrap()
                .remove(0)
        })
        .collect();
    let compare = |a: &[Vec<f32>], b: &[Vec<f32>]| {
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(b) {
            assert_eq!(a.len(), profile.descriptor.dimensions as usize);
            let norm = |v: &[f32]| v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
            assert!(a.iter().chain(b).all(|v| v.is_finite()));
            assert!((norm(a) - 1.).abs() < 0.0001 && (norm(b) - 1.).abs() < 0.0001);
            assert!(
                a.iter()
                    .zip(b)
                    .map(|(a, b)| f64::from(*a) * f64::from(*b))
                    .sum::<f64>()
                    / (norm(a) * norm(b))
                    >= 0.9999
            );
        }
    };
    compare(
        &worker
            .embed_documents(&inputs, &Control::with_deadline(deadline()))
            .unwrap(),
        &single,
    );
    let mut full = inputs[0].clone();
    full.ids
        .resize(profile.descriptor.input_limits().tokens, full.ids[1]);
    let reference = worker
        .embed_documents(
            std::slice::from_ref(&full),
            &Control::with_deadline(deadline()),
        )
        .unwrap();
    compare(
        &reference,
        &worker
            .embed_documents(
                std::slice::from_ref(&full),
                &Control::with_deadline(deadline()),
            )
            .unwrap(),
    );
    full.ids.push(1);
    assert!(
        worker
            .embed_documents(&[full], &Control::with_deadline(deadline()))
            .is_err()
    );
    println!(
        "LINUX_WORKER_RESULT {}",
        serde_json::json!({"input_tokens":profile.descriptor.input_limits().tokens,"descriptor":profile.descriptor,"elapsed_ms":started.elapsed().as_millis(),"real_protocol":true})
    );
}
