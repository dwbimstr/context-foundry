//! Bootstrap owns source indexing and bounded preparation together. The cached
//! replay crosses the real CLI/supervisor boundary without a usable worker;
//! only initial cache population uses the counted in-process test provider.
#![cfg(feature = "semantic")]

use context_foundry::bootstrap::{self, ComponentState, SemanticRun};
use context_foundry::neural::cache::DEFAULT_CACHE_CAP_BYTES;
use context_foundry::neural::fake::{FakeBehavior, FakeFactory};
use context_foundry::neural::prepare::{self, PrepareOptions};
use context_foundry::neural::profile::SemanticProfile;
use context_foundry::{Control, Engine, testkit};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

struct Fixture {
    dir: tempfile::TempDir,
    root: PathBuf,
    store: PathBuf,
    profile: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(
            root.join("record.rs"),
            "pub fn parse_record() -> u32 { 7 }\n",
        )
        .unwrap();
        let store = dir.path().join("store");
        let profile = testkit::write_semantic_profile(dir.path(), "bootstrap", |_| {});
        Self {
            dir,
            root,
            store,
            profile,
        }
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_foundry"))
            .arg("--store")
            .arg(&self.store)
            .arg("bootstrap")
            .arg("--root")
            .arg(&self.root)
            .arg("--profile")
            .arg(&self.profile)
            .args(args)
            .output()
            .unwrap()
    }

    fn index(&self) {
        let result = self.cli(&["--apply", "--components", "lexical"]);
        assert!(result.status.success(), "{result:?}");
    }
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| panic!("{output:?}"))
}

#[test]
fn inspection_and_apply_without_budget_never_claim_runtime_readiness() {
    let f = Fixture::new();
    // Inspection must not parse a supplied profile or open an owned store.
    std::fs::write(&f.profile, "invalid JSON").unwrap();
    let output = f.cli(&["--components", "lexical,semantic"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(json(&output)["components"][1]["state"], "unverified");
    assert!(!f.store.exists());
    let output = f.cli(&["--apply", "--components", "lexical,semantic"]);
    assert_eq!(output.status.code(), Some(1));
    let report = json(&output);
    assert_eq!(report["components"][0]["state"], "ready");
    assert_eq!(report["components"][1]["state"], "unverified");
    assert!(report.get("semantic_preparation").is_none());
    let engine = Engine::open_existing(&f.store).unwrap();
    let held = f.cli(&["--components", "semantic"]);
    assert!(
        held.status.success(),
        "inspection must not acquire owner: {held:?}"
    );
    assert_eq!(engine.status().unwrap().source_count, 1);
}

#[test]
fn explicit_semantic_options_refuse_invalid_admission_before_store_creation() {
    let f = Fixture::new();
    for args in [
        vec!["--semantic-budget-seconds", "60"],
        vec![
            "--apply",
            "--semantic-budget-seconds",
            "60",
            "--components",
            "lexical",
        ],
        vec![
            "--apply",
            "--semantic-budget-seconds",
            "0",
            "--components",
            "lexical,semantic",
        ],
        vec![
            "--apply",
            "--semantic-budget-seconds",
            "60",
            "--components",
            "semantic",
        ],
        vec!["--apply", "--development-isolation"],
    ] {
        let output = f.cli(&args);
        assert!(!output.status.success(), "{args:?}: {output:?}");
        assert!(
            !f.store.exists(),
            "invalid admission wrote a store: {args:?}"
        );
    }
    let control = Control::unbounded();
    control
        .cancel_flag()
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let error = bootstrap::apply(
        &f.root,
        Some(&f.store),
        &["lexical".into(), "semantic".into()],
        None,
        None,
        Some(&f.profile),
        Some(&SemanticRun {
            budget_seconds: 60,
            development_isolation: false,
            cache_cap_bytes: DEFAULT_CACHE_CAP_BYTES,
            control: &control,
        }),
    )
    .unwrap_err();
    assert_eq!(error.code(), "cancelled");
    assert!(!f.store.exists());
}

#[test]
fn semantic_only_requires_a_completed_scan_and_accepts_an_indexed_empty_root() {
    let f = Fixture::new();
    let setup = f.cli(&["--apply", "--components", "semantic"]);
    assert_eq!(setup.status.code(), Some(1));
    assert_eq!(
        Engine::open_existing(&f.store)
            .unwrap()
            .status()
            .unwrap()
            .scan_state,
        "never"
    );
    let args = [
        "--apply",
        "--components",
        "semantic",
        "--semantic-budget-seconds",
        "60",
    ];
    let refused = f.cli(&args);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("completed source scan"));
    std::fs::remove_file(f.root.join("record.rs")).unwrap();
    f.index();
    let accepted = f.cli(&args);
    assert!(accepted.status.success(), "{accepted:?}");
    let report = json(&accepted);
    assert_eq!(report["complete"], true);
    assert_eq!(report["semantic_preparation"]["document_calls"], 0);
}

#[test]
fn invalid_profile_preserves_lexical_source_and_requested_retry_limits() {
    let f = Fixture::new();
    std::fs::write(&f.profile, "invalid JSON").unwrap();
    let output = f.cli(&[
        "--apply",
        "--components",
        "lexical,semantic",
        "--semantic-budget-seconds",
        "17",
        "--semantic-cache-cap",
        "123456",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let report = json(&output);
    assert_eq!(report["complete"], false);
    assert_eq!(report["components"][0]["state"], "ready");
    assert_eq!(report["components"][1]["state"], "failed");
    assert!(
        report["components"][1]["reason"]
            .as_str()
            .unwrap()
            .contains("profile_invalid")
    );
    let retry = report["components"][1]["next_action"].as_str().unwrap();
    assert!(
        retry.contains("--budget-seconds 17 --cache-cap 123456"),
        "{retry}"
    );
    assert!(!retry.contains("--development-isolation"));
    let found = Command::new(env!("CARGO_BIN_EXE_foundry"))
        .arg("--store")
        .arg(&f.store)
        .args(["context", "parse_record", "--tokens", "512"])
        .output()
        .unwrap();
    assert!(found.status.success(), "{found:?}");
    assert!(String::from_utf8_lossy(&found.stdout).contains("parse_record"));
}

#[test]
fn budget_stop_is_partial_and_reuses_the_same_owned_source_store() {
    let f = Fixture::new();
    let output = f.cli(&[
        "--apply",
        "--components",
        "lexical,semantic",
        "--semantic-budget-seconds",
        "1",
        "--semantic-cache-cap",
        "654321",
        "--development-isolation",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let report = json(&output);
    assert_eq!(report["components"][0]["state"], "ready");
    assert_eq!(report["components"][1]["state"], "partial");
    assert_eq!(
        report["semantic_preparation"]["reason_code"],
        "budget_exhausted"
    );
    assert_eq!(report["semantic_preparation"]["document_calls"], 0);
    assert!(
        report["components"][1]["next_action"]
            .as_str()
            .unwrap()
            .ends_with("--budget-seconds 1 --cache-cap 654321 --development-isolation")
    );
    assert_eq!(
        Engine::open_existing(&f.store)
            .unwrap()
            .status()
            .unwrap()
            .source_count,
        1
    );
}

#[test]
fn unchanged_bootstrap_replays_cached_vectors_without_a_worker_and_refuses_other_root() {
    let f = Fixture::new();
    f.index();
    let profile = SemanticProfile::load(&f.profile).unwrap();
    assert!(
        !profile.worker.bundle.exists(),
        "there is no executable to start"
    );
    let factory = FakeFactory::new(profile.descriptor.clone(), FakeBehavior::Ok);
    let prepared = prepare::run(
        &f.store,
        &PrepareOptions {
            profile_path: &f.profile,
            budget_seconds: 60,
            development: false,
            cache_cap_bytes: DEFAULT_CACHE_CAP_BYTES,
            started: Instant::now(),
            control: &Control::unbounded(),
            priority_paths: &[],
        },
        factory.acquire(),
    )
    .unwrap();
    assert!(!prepared.partial, "{prepared:?}");
    assert!(factory.handle().document_calls() > 0);
    for components in ["lexical,semantic", "semantic"] {
        let output = f.cli(&[
            "--apply",
            "--components",
            components,
            "--semantic-budget-seconds",
            "60",
        ]);
        assert!(output.status.success(), "{output:?}");
        let report = json(&output);
        assert_eq!(report["complete"], true);
        assert_eq!(report["semantic_preparation"]["document_calls"], 0);
        assert_eq!(report["semantic_preparation"]["embedded_units"], 0);
        assert!(report["semantic_preparation"]["provider_state"].is_null());
        assert!(
            report["semantic_preparation"]["reused_cached_units"]
                .as_u64()
                .unwrap()
                > 0
        );
    }
    let other = f.dir.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let error = bootstrap::apply(
        &other,
        Some(&f.store),
        &["semantic".into()],
        None,
        None,
        Some(&f.profile),
        Some(&SemanticRun {
            budget_seconds: 60,
            development_isolation: false,
            cache_cap_bytes: DEFAULT_CACHE_CAP_BYTES,
            control: &Control::unbounded(),
        }),
    )
    .unwrap_err();
    assert_eq!(error.code(), "wrong_workspace");
    // Without a semantic run, apply still retains explicit setup state.
    let report = bootstrap::apply(
        &f.root,
        Some(&f.store),
        &["semantic".into()],
        None,
        None,
        Some(Path::new("missing-profile")),
        None,
    )
    .unwrap();
    assert_eq!(report.components[0].state, ComponentState::NeedsSetup);
}
