//! 013 T002 acceptance (SC-002): train and evaluate the actual model in one
//! accepted profile.
//!
//! (a) Default build, fake worker (`foundry-learn-fake` in a fake bundle
//!     under the real supervisor, isolation profile and core): the full
//!     pipeline, every failure path with no eligible output, the pre-fit
//!     permission gate, exclusive ownership, disk-full fault points, reply
//!     correlation, owner death and the child's environment, descriptors and
//!     limits. Sandbox denials are the development-bundle test at the end
//!     (`dev_learn_sandbox_negative_probes`, ignored: measurement phase).
//! (b) `--features learning-worker` with the real checkpoint: parity with
//!     `tests/fixtures/learning/reference-t002.json` at atol 1e-5 / rtol 1e-4
//!     and the numerical model's training properties (module `real`).
//! (c) Pure-core calibration, decision and evaluation tests live with the
//!     code (`learning::eval`, `learning::candidate`); the real
//!     `rl_agent_config.json` refusals are here.
//!
//! External assets follow 013 T001's convention: the pinned tokenizer at
//! `CONTEXT_FOUNDRY_013_TOKENIZER` (default the local checkpoint's
//! `tokenizer/`) and, for (b), the checkpoint at
//! `CONTEXT_FOUNDRY_013_CHECKPOINT`; a missing asset is a named failure.
#![cfg(all(feature = "semantic", target_os = "macos"))]
use context_foundry::learning::{self, candidate, supervisor, train};
use context_foundry::{Control, Engine, FoundryError};
use serde_json::json;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const MODELS: &str = "VSC_DEV/models/laya-typed-decisions-1a793eb5";

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("HOME"))
}

/// The pinned checkpoint directory (`CONTEXT_FOUNDRY_013_CHECKPOINT`).
fn checkpoint_dir() -> PathBuf {
    std::env::var_os("CONTEXT_FOUNDRY_013_CHECKPOINT")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(MODELS))
}

/// The pinned tokenizer (`CONTEXT_FOUNDRY_013_TOKENIZER`).
fn tokenizer_dir() -> PathBuf {
    std::env::var_os("CONTEXT_FOUNDRY_013_TOKENIZER")
        .map(PathBuf::from)
        .unwrap_or_else(|| checkpoint_dir().join("tokenizer"))
}

fn digest_of(bytes: &[u8]) -> String {
    context_foundry::digest(bytes)
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn fake_exe() -> &'static str {
    env!("CARGO_BIN_EXE_foundry-learn-fake")
}

/// Group names bucketed by the deterministic split rule, enough for every
/// floor.
fn floor_groups() -> [Vec<String>; 3] {
    let (mut train, mut calibration, mut evaluation) = (Vec::new(), Vec::new(), Vec::new());
    for i in 0.. {
        let name = format!("grp-{i:04}");
        match learning::split_of(&name) {
            "train" if train.len() < 22 => train.push(name),
            "calibration" if calibration.len() < 11 => calibration.push(name),
            "evaluation" if evaluation.len() < 21 => evaluation.push(name),
            _ => {}
        }
        if train.len() == 22 && calibration.len() == 11 && evaluation.len() == 21 {
            break;
        }
    }
    [train, calibration, evaluation]
}

fn row(task: &str, group: &str, label: &str, state: &str) -> serde_json::Value {
    json!({
        "task_id": task,
        "task_group_id": group,
        "family": "retrieval-route-v1",
        "state": state,
        "option_ids": ["search", "graph"],
        "correct_option_id": label,
        "label_source": "operator",
        "label_evidence": format!("evidence for {task}"),
        "rights_ref": "rights-checked",
        "allow_training": true,
    })
}

/// One training environment: a store with rows meeting every floor, the
/// pinned tokenizer, a fake checkpoint, a fake signed-bundle layout around
/// `foundry-learn-fake`, its isolation profile, a policy and a prepared
/// dataset.
struct Env {
    dir: tempfile::TempDir,
    store: PathBuf,
    rows: Vec<serde_json::Value>,
    profile: PathBuf,
    scratch_root: PathBuf,
    policy: PathBuf,
    dataset: PathBuf,
}

/// The policy's tunable parts.
#[derive(Clone)]
struct Knobs {
    seed: &'static str,
    max_steps: u64,
    wall_seconds: u64,
    output_bytes: u64,
    memory: &'static str,
    critical_groups: Vec<String>,
    base: Option<String>,
    /// A selection policy every finite candidate meets (threshold 0, floors
    /// 0, any macro drop): an eligible base for the repeat-round test.
    lenient: bool,
    /// The profile's load timeout.
    load_timeout_seconds: u64,
}

impl Default for Knobs {
    fn default() -> Self {
        Self {
            seed: "seed-alpha",
            max_steps: 4,
            wall_seconds: 120,
            output_bytes: 128 << 20,
            memory: "supervised",
            critical_groups: Vec::new(),
            base: None,
            lenient: false,
            load_timeout_seconds: 60,
        }
    }
}

impl Env {
    fn new() -> Self {
        Self::with(&Knobs::default())
    }

    fn with(knobs: &Knobs) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let ws = root.join("ws");
        std::fs::create_dir(&ws).unwrap();
        let store = root.join("store");
        drop(Engine::initialize(&store, &ws).unwrap());
        // Fake checkpoint: the fake worker hashes exactly these files.
        let checkpoint = root.join("checkpoint");
        std::fs::create_dir_all(checkpoint.join("encoder")).unwrap();
        std::fs::write(checkpoint.join("model.safetensors"), b"fake weights").unwrap();
        std::fs::write(checkpoint.join("encoder/config.json"), b"{}").unwrap();
        let libtorch = root.join("libtorch-lib");
        std::fs::create_dir(&libtorch).unwrap();
        // The bundle layout the supervisor resolves, around the fake.
        let bundle = root.join("FoundryLearnFake.app");
        let macos = bundle.join("Contents/MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        let exe = macos.join("foundry-learn");
        std::fs::copy(fake_exe(), &exe).unwrap();
        let scratch_root = root.join("scratch");
        let profile = root.join("learn-profile.json");
        std::fs::write(
            &profile,
            json!({
                "v": 1,
                "name": "fake learning worker",
                "worker": {
                    "bundle": bundle,
                    "executable_sha256": digest_of(&read(&exe)),
                    "scratch_root": scratch_root,
                },
                "checkpoint_dir": checkpoint,
                "libtorch_dir": libtorch,
                "load_timeout_seconds": knobs.load_timeout_seconds,
                "ceilings": {
                    "memory_bytes": 8u64 << 30,
                    "wall_seconds": 7200,
                    "output_bytes": 2u64 << 30,
                    "cpu_threads": 4,
                },
            })
            .to_string(),
        )
        .unwrap();
        let mut rows = Vec::new();
        for (split, groups) in ["t", "c", "e"].iter().zip(floor_groups()) {
            for (i, group) in groups.iter().enumerate() {
                let label = if i % 2 == 0 { "search" } else { "graph" };
                rows.push(row(
                    &format!("{split}{i}"),
                    group,
                    label,
                    &format!("where is {split}{i} defined\ngraph: complete\nsrc/lib.rs fn x{i}"),
                ));
            }
        }
        let mut env = Self {
            dir,
            store,
            rows,
            profile,
            scratch_root,
            policy: PathBuf::new(),
            dataset: PathBuf::new(),
        };
        env.record_all();
        env.policy = env.write_policy("policy.json", knobs);
        env.dataset = env.prepare("dataset", &env.policy.clone());
        env
    }

    fn path(&self, name: &str) -> PathBuf {
        std::fs::canonicalize(self.dir.path()).unwrap().join(name)
    }

    fn engine(&self) -> Engine {
        Engine::open_existing(&self.store).unwrap()
    }

    fn record_all(&self) {
        let engine = self.engine();
        for row in &self.rows {
            engine.record_learning_feedback(&row.to_string()).unwrap();
        }
    }

    fn record(&self, row: &serde_json::Value) {
        self.engine()
            .record_learning_feedback(&row.to_string())
            .unwrap();
    }

    fn write_policy(&self, name: &str, knobs: &Knobs) -> PathBuf {
        let tokenizer = tokenizer_dir();
        let checkpoint = self.path("checkpoint");
        let policy = json!({
            "v": 2,
            "recipe": learning::RECIPE,
            "seed": knobs.seed,
            "tokenizer": {
                "dir": tokenizer,
                "json_sha256": digest_of(&read(&tokenizer.join("tokenizer.json"))),
                "config_sha256": digest_of(&read(&tokenizer.join("tokenizer_config.json"))),
            },
            "model": {
                "weights_sha256": digest_of(&read(&checkpoint.join("model.safetensors"))),
                "encoder_config_sha256": digest_of(&read(&checkpoint.join("encoder/config.json"))),
                "source_dtype": "F16",
            },
            "base": knobs.base,
            "optimizer": {
                "name": "adamw",
                "learning_rate": 1e-4,
                "beta1": 0.9,
                "beta2": 0.999,
                "epsilon": 1e-8,
                "weight_decay": 0.01,
                "clip_global_norm": 1.0,
            },
            "max_steps": knobs.max_steps,
            "wall_seconds": knobs.wall_seconds,
            "memory_bytes": 4u64 << 30,
            "output_bytes": knobs.output_bytes,
            "cpu_threads": 2,
            "isolation_profile": self.profile,
            "enforcement": {
                "memory": knobs.memory,
                "cpu": "hard",
                "output": "hard",
                "process_count": "hard",
            },
            "selection": {
                "threshold": if knobs.lenient { 0.0 } else { 0.8 },
                "coverage_floor": if knobs.lenient { 0.0 } else { 0.5 },
                "accepted_accuracy_floor": if knobs.lenient { 0.0 } else { 0.9 },
                "max_macro_accuracy_drop": if knobs.lenient { 1.0 } else { 0.0 },
                "critical_groups": knobs.critical_groups,
            },
        });
        let path = self.path(name);
        std::fs::write(&path, policy.to_string()).unwrap();
        path
    }

    fn prepare(&self, name: &str, policy: &Path) -> PathBuf {
        self.prepare_child(name, policy, None)
    }

    fn prepare_child(&self, name: &str, policy: &Path, parent: Option<&Path>) -> PathBuf {
        let out = self.path(name);
        match learning::prepare(&self.engine(), &out, policy, parent, &Control::unbounded())
            .expect("the dataset prepares")
        {
            learning::PrepareOutcome::Completed(prepared) => prepared.manifest_path,
            learning::PrepareOutcome::NoNewData { .. } => panic!("expected a dataset"),
        }
    }

    /// Train in-process with the fake worker's `hooks`.
    fn train_with(
        &self,
        out: &Path,
        policy: &Path,
        hooks: &[&str],
        base: Option<&Path>,
        incumbent: Option<&Path>,
    ) -> Result<train::Trained, FoundryError> {
        supervisor::set_test_worker_args(hooks.iter().map(|h| h.to_string()).collect());
        let engine = self.engine();
        let result = train::train(
            &engine,
            &train::TrainRequest {
                input: &self.dataset,
                policy,
                out,
                base,
                incumbent,
                development_isolation: true,
            },
            &Control::unbounded(),
        );
        supervisor::set_test_worker_args(Vec::new());
        result
    }

    fn train(&self, out: &Path, hooks: &[&str]) -> Result<train::Trained, FoundryError> {
        self.train_with(out, &self.policy, hooks, None, None)
    }

    fn train_ok(&self, name: &str) -> (PathBuf, train::Trained) {
        let out = self.path(name);
        let trained = self.train(&out, &[]).expect("training completes");
        (out, trained)
    }

    /// Train and expect a named failure that leaves nothing behind.
    fn train_fails(&self, hooks: &[&str]) -> FoundryError {
        let out = self.path("never");
        let pid_file = self.path("worker.pid");
        let _ = std::fs::remove_file(&pid_file);
        let mut all: Vec<&str> = hooks.to_vec();
        let pid_arg = pid_file.display().to_string();
        all.extend(["--pid-file", &pid_arg]);
        let error = self.train(&out, &all).expect_err("the run must fail");
        self.assert_nothing_left(&out);
        if let Ok(pid) = std::fs::read_to_string(&pid_file) {
            assert_reaped(pid.trim().parse().unwrap());
        }
        error
    }

    /// No output, no partial sibling, no scratch run directory.
    fn assert_nothing_left(&self, out: &Path) {
        assert!(!out.exists(), "a failed run left {}", out.display());
        for entry in std::fs::read_dir(out.parent().unwrap()).unwrap() {
            let name = entry.unwrap().file_name();
            assert!(
                !name.to_string_lossy().starts_with(".learning-partial-"),
                "a partial sibling {name:?} survived"
            );
        }
        if self.scratch_root.exists() {
            assert_eq!(
                std::fs::read_dir(&self.scratch_root).unwrap().count(),
                0,
                "a scratch run directory survived"
            );
        }
    }

    fn foundry(&self, faults: bool) -> Command {
        let exe = if faults {
            env!("CARGO_BIN_EXE_foundry-faults")
        } else {
            env!("CARGO_BIN_EXE_foundry")
        };
        let mut command = Command::new(exe);
        command.arg("--store").arg(&self.store);
        command
    }

    fn train_cli(&self, command: &mut Command, out: &Path) {
        command
            .args(["learning", "train", "--input"])
            .arg(&self.dataset)
            .arg("--policy")
            .arg(&self.policy)
            .arg("--out")
            .arg(out);
    }
}

fn assert_reaped(pid: u32) {
    let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
    assert!(
        rc == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH),
        "worker {pid} was not reaped"
    );
}

fn run_bounded(mut command: Command) -> Output {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("{command:?} did not exit in time");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

fn error_code(out: &Output) -> String {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let error: serde_json::Value = serde_json::from_str(stderr.trim())
        .unwrap_or_else(|_| panic!("no error JSON on stderr: {stderr}"));
    error["code"].as_str().unwrap().to_owned()
}

fn json_file(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&read(path)).unwrap()
}

// ---------------------------------------------------------------------------
// (a) The pipeline
// ---------------------------------------------------------------------------

#[test]
fn a_fake_run_publishes_a_complete_readable_candidate() {
    let env = Env::new();
    let (out, trained) = env.train_ok("candidate");
    assert_eq!(trained.outcome, "completed");
    assert!(!trained.adopted);
    assert_eq!(trained.steps_completed, 4);
    let manifest_bytes = read(&out.join("manifest.json"));
    assert_eq!(
        trained.candidate_manifest_sha256,
        digest_of(&manifest_bytes)
    );
    let manifest = json_file(&out.join("manifest.json"));
    assert_eq!(manifest["schema"], 4);
    assert_eq!(manifest["kind"], "candidate");
    assert_eq!(manifest["base"], json!({"kind": "initial"}));
    assert_eq!(manifest["training"]["steps_completed"], 4);
    assert_eq!(manifest["training"]["max_steps"], 4);
    assert_eq!(manifest["training"]["head_dropout"], 0.1);
    assert_eq!(
        manifest["dataset_manifest_sha256"],
        digest_of(&read(&env.dataset))
    );
    let names: Vec<&str> = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, candidate::FILES);
    for file in manifest["files"].as_array().unwrap() {
        let bytes = read(&out.join(file["name"].as_str().unwrap()));
        assert_eq!(file["sha256"], digest_of(&bytes));
        assert_eq!(file["bytes"], bytes.len());
    }
    // One fitted scalar on the grid, no per-option override anywhere.
    let t = manifest["temperature"].as_f64().unwrap();
    assert!((0.5..=3.0).contains(&t) && ((t * 20.0).round() / 20.0 - t).abs() < 1e-12);
    assert!(!String::from_utf8_lossy(&manifest_bytes).contains("temperature_by_options"));
    // The report: every evaluation row, with the deterministic comparator.
    let report = json_file(&out.join("evaluation.json"));
    assert_eq!(report["rows"], 21);
    assert_eq!(report["cases"].as_array().unwrap().len(), 21);
    assert_eq!(report["baseline"]["rows"], 21);
    assert_eq!(report["temperature"], manifest["temperature"]);
    assert_eq!(report["eligibility"]["eligible"], manifest["eligible"]);
    // Calibrated probabilities reproduce from the exported scalar and logits.
    for case in report["cases"].as_array().unwrap() {
        let z: Vec<f64> = case["logits"]
            .as_array()
            .unwrap()
            .iter()
            // Logits are f32: widen exactly what was exported.
            .map(|v| f64::from(v.as_f64().unwrap() as f32))
            .collect();
        let max = z[0].max(z[1]);
        let e = [((z[0] - max) / t).exp(), ((z[1] - max) / t).exp()];
        let p = [e[0] / (e[0] + e[1]), e[1] / (e[0] + e[1])];
        assert_eq!(case["probabilities"][0].as_f64().unwrap(), p[0]);
        assert_eq!(case["probabilities"][1].as_f64().unwrap(), p[1]);
        // No raw state in the report.
        assert!(case.get("state").is_none());
    }
    // Lineage: every train row is a new fitting contribution, every
    // calibration row a new calibration contribution; none inherited.
    let contributions = json_file(&out.join("contributions.json"));
    assert_eq!(
        contributions["fitting"]["new"].as_array().unwrap().len(),
        22
    );
    assert_eq!(
        contributions["calibration"]["new"]
            .as_array()
            .unwrap()
            .len(),
        11
    );
    assert!(
        contributions["fitting"]["inherited"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    // The head: the exact trainable set, float32, finite.
    let head = std::fs::File::open(out.join("head.safetensors")).unwrap();
    let bytes = head.metadata().unwrap().len();
    candidate::validate_head(head, bytes, "artifact_invalid", &Control::unbounded()).unwrap();
    env.assert_nothing_left(&env.path("not-written"));
}

#[test]
fn without_development_isolation_training_is_isolation_unavailable() {
    let env = Env::new();
    let out = env.path("candidate");
    let mut command = env.foundry(false);
    env.train_cli(&mut command, &out);
    let output = run_bounded(command);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(error_code(&output), "isolation_unavailable");
    assert!(!out.exists());
}

#[test]
fn the_cli_completes_with_exit_0_and_a_concurrent_owner_is_store_busy() {
    let env = Env::new();
    let out = env.path("candidate");
    // A concurrent owner (an MCP server or online worker would hold the
    // store exactly so): busy, exit 3, nothing started.
    let holder = env.engine();
    let mut command = env.foundry(false);
    env.train_cli(&mut command, &out);
    command.arg("--development-isolation");
    let output = run_bounded(command);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    assert_eq!(error_code(&output), "store_busy");
    assert!(!out.exists());
    drop(holder);
    // With the store free the release CLI completes (exit 0); the profile's
    // bundle is this test's fake worker.
    let mut command = env.foundry(false);
    env.train_cli(&mut command, &out);
    command.arg("--development-isolation");
    let output = run_bounded(command);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["outcome"], "completed");
    assert!(out.join("manifest.json").exists());
}

// ---------------------------------------------------------------------------
// (a) D2: the pre-fit permission gate
// ---------------------------------------------------------------------------

/// Requests the fake worker received, by kind.
fn requests(log: &Path) -> Vec<String> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(|line| line.split(' ').next().unwrap().to_owned())
        .collect()
}

#[test]
fn withdrawal_correction_or_rights_change_after_prepare_refuses_before_any_update() {
    type Change = fn(&mut serde_json::Value);
    let changes: [(&str, Change); 4] = [
        ("withdrawn", |row| row["allow_training"] = false.into()),
        ("label corrected", |row| {
            let flipped = if row["correct_option_id"] == "search" {
                "graph"
            } else {
                "search"
            };
            row["correct_option_id"] = flipped.into();
        }),
        ("rights changed", |row| {
            row["rights_ref"] = "rights-revoked".into()
        }),
        // A held-out (evaluation) contribution counts as much as a fitting one.
        ("evaluation row withdrawn", |row| {
            row["allow_training"] = false.into()
        }),
    ];
    for (what, change) in changes {
        let env = Env::new();
        let victim_prefix = if what.starts_with("evaluation") {
            "e"
        } else {
            "t"
        };
        let mut victim = env
            .rows
            .iter()
            .find(|r| r["task_id"].as_str().unwrap().starts_with(victim_prefix))
            .unwrap()
            .clone();
        change(&mut victim);
        env.record(&victim);
        let log = env.path("requests.log");
        let log_arg = log.display().to_string();
        let error = env.train_fails(&["--request-log", &log_arg]);
        assert_eq!(error.code(), "contribution_changed", "{what}: {error}");
        assert_eq!(error.exit_code(), 2);
        let received = requests(&log);
        assert!(
            received.contains(&"load".to_owned()),
            "{what}: {received:?}"
        );
        assert!(
            !received.iter().any(|kind| kind == "step"),
            "{what}: an update was sent before the gate: {received:?}"
        );
    }
}

#[test]
fn a_dataset_from_another_policy_or_a_tampered_dataset_is_refused_by_name() {
    let env = Env::new();
    // The same rows under another policy (another seed): policy_mismatch.
    let other = env.write_policy(
        "other.json",
        &Knobs {
            seed: "seed-beta",
            ..Knobs::default()
        },
    );
    let out = env.path("never");
    let error = env
        .train_with(&out, &other, &[], None, None)
        .expect_err("another policy");
    assert_eq!(error.code(), "policy_mismatch", "{error}");
    // A row whose bytes no longer match the manifest.
    let train_file = env.dataset.parent().unwrap().join("train.jsonl");
    let mut body = read(&train_file);
    body[10] ^= 0x01;
    std::fs::write(&train_file, body).unwrap();
    let error = env.train(&out, &[]).expect_err("tampered dataset");
    assert_eq!(error.code(), "dataset_invalid", "{error}");
    env.assert_nothing_left(&out);
}

// ---------------------------------------------------------------------------
// (a) Steps and epochs
// ---------------------------------------------------------------------------

#[test]
fn one_rows_and_rows_plus_one_steps_run_exactly_that_many_updates() {
    // 22 training rows: one update, exactly one epoch, one into the second.
    for (max_steps, epochs) in [(1u64, 1u64), (22, 1), (23, 2)] {
        let env = Env::with(&Knobs {
            max_steps,
            ..Knobs::default()
        });
        let log = env.path("requests.log");
        let log_arg = log.display().to_string();
        let out = env.path("candidate");
        let trained = env
            .train(&out, &["--request-log", &log_arg])
            .expect("training completes");
        assert_eq!(trained.steps_completed, max_steps);
        let steps = requests(&log).iter().filter(|k| *k == "step").count() as u64;
        assert_eq!(steps, max_steps);
        let manifest = json_file(&out.join("manifest.json"));
        assert_eq!(manifest["training"]["steps_completed"], max_steps);
        assert_eq!(manifest["training"]["epochs"], epochs);
        assert_eq!(manifest["training"]["train_rows"], 22);
    }
}

// ---------------------------------------------------------------------------
// (a) Failures: none leaves eligible output
// ---------------------------------------------------------------------------

#[test]
fn the_wall_clock_stops_and_reaps_a_hanging_worker() {
    let env = Env::with(&Knobs {
        wall_seconds: 2,
        ..Knobs::default()
    });
    let started = Instant::now();
    let error = env.train_fails(&["--hang-at-step", "2"]);
    assert_eq!(error.code(), "worker_timeout", "{error}");
    assert_eq!(error.exit_code(), 1);
    assert!(started.elapsed() < Duration::from_secs(30));
}

#[test]
fn a_term_ignoring_hung_worker_is_killed_after_the_grace_and_reaped() {
    let env = Env::with(&Knobs {
        wall_seconds: 1,
        ..Knobs::default()
    });
    let error = env.train_fails(&["--hang-at-step", "1", "--ignore-term"]);
    assert_eq!(error.code(), "worker_timeout", "{error}");
}

#[test]
fn a_crashing_worker_is_a_named_failure() {
    let env = Env::new();
    let error = env.train_fails(&["--crash-at-step", "2"]);
    assert_eq!(error.code(), "worker_failed", "{error}");
    assert_eq!(error.exit_code(), 1);
}

#[test]
fn nonfinite_loss_or_gradient_is_refused_immediately() {
    let env = Env::new();
    let error = env.train_fails(&["--nonfinite-loss-at", "1"]);
    assert_eq!(error.code(), "nonfinite_loss", "{error}");
    let error = env.train_fails(&["--nonfinite-grad-at", "2"]);
    assert_eq!(error.code(), "nonfinite_gradient", "{error}");
    assert_eq!(error.exit_code(), 1);
}

#[test]
fn a_calibration_failure_stops_the_run() {
    // The first logits request is the first calibration row.
    let env = Env::new();
    let error = env.train_fails(&["--nonfinite-logits-at", "1"]);
    assert_eq!(error.code(), "calibration_failed", "{error}");
    assert_eq!(error.exit_code(), 1);
}

#[test]
fn malformed_missing_or_tampered_head_output_is_refused() {
    let env = Env::new();
    for (hook, code) in [
        ("--head-missing-tensor", "artifact_invalid"),
        ("--head-bad-shape", "artifact_invalid"),
        ("--head-nonfinite", "nonfinite_weight"),
        ("--head-tamper", "artifact_invalid"),
    ] {
        let error = env.train_fails(&[hook]);
        assert_eq!(error.code(), code, "{hook}: {error}");
        assert_eq!(error.exit_code(), 1, "{hook}");
    }
}

#[test]
fn a_duplicate_final_saved_reply_publishes_nothing() {
    // The last request of a run without an incumbent is `save`: its
    // duplicate can only be found by the checked completion, after the
    // worker exited and its output was read to the end.
    let env = Env::new();
    let error = env.train_fails(&["--duplicate-saved"]);
    assert_eq!(error.code(), "worker_failed", "{error}");
    assert!(error.to_string().contains("extra saved frame"), "{error}");
    assert_eq!(error.exit_code(), 1);
}

#[test]
fn a_measurement_lost_after_the_final_save_publishes_nothing() {
    use context_foundry::fault::{self, Action};
    use context_foundry::neural::fault_names::FOOTPRINT_MEASURE;
    // Armed on this thread only: the monitor polls on its own thread and
    // never sees it, so the one measurement that fails is the completion's
    // own, after the final `saved` reply was consumed.
    let env = Env::new();
    fault::arm(
        FOOTPRINT_MEASURE,
        0,
        Action::Fail("the footprint cannot be read".into()),
    );
    let error = env.train_fails(&[]);
    let reached = fault::reached(FOOTPRINT_MEASURE);
    fault::disarm_all();
    assert_eq!(error.code(), "memory_unmeasurable", "{error}");
    assert_eq!(error.exit_code(), 1);
    assert_eq!(reached, 1, "only the completion measured on this thread");
}

/// Arm the reply-received point so a reply of `kind` is judged only after
/// its earliest stop passed: the hook sleeps the remaining milliseconds the
/// point reports, plus a margin. Returns whether the hook fired.
fn delay_past_the_stop(kind: &'static str) -> std::rc::Rc<std::cell::Cell<bool>> {
    use context_foundry::fault::{self, Action};
    let fired = std::rc::Rc::new(std::cell::Cell::new(false));
    let seen = std::rc::Rc::clone(&fired);
    fault::arm(
        learning::fault_names::REPLY_RECEIVED,
        0,
        Action::Call(Box::new(move |ctx| {
            let mut parts = ctx.detail.split(' ');
            if parts.next() == Some(kind) {
                let left: u64 = parts.next().unwrap().parse().unwrap();
                std::thread::sleep(Duration::from_millis(left + 100));
                seen.set(true);
            }
        })),
    );
    fired
}

#[test]
fn a_valid_final_saved_reply_after_the_wall_clock_is_refused() {
    let env = Env::with(&Knobs {
        wall_seconds: 15,
        ..Knobs::default()
    });
    let fired = delay_past_the_stop("saved");
    let error = env.train_fails(&[]);
    context_foundry::fault::disarm_all();
    assert!(
        fired.get(),
        "the saved reply reached the hook before the deadline"
    );
    assert_eq!(error.code(), "worker_timeout", "{error}");
    assert!(error.to_string().contains("wall clock"), "{error}");
}

#[test]
fn a_loaded_reply_after_the_load_timeout_is_refused() {
    let env = Env::with(&Knobs {
        load_timeout_seconds: 3,
        ..Knobs::default()
    });
    let log = env.path("requests.log");
    let log_arg = log.display().to_string();
    let fired = delay_past_the_stop("loaded");
    let error = env.train_fails(&["--request-log", &log_arg]);
    context_foundry::fault::disarm_all();
    assert!(
        fired.get(),
        "the loaded reply reached the hook before the timeout"
    );
    assert_eq!(error.code(), "worker_timeout", "{error}");
    assert!(error.to_string().contains("load timeout"), "{error}");
    assert_eq!(requests(&log), ["load"], "nothing followed the late load");
}

#[test]
fn duplicate_stale_or_drifted_replies_are_terminal() {
    let env = Env::new();
    // The first logits reply sent twice: the duplicate answers nothing.
    let error = env.train_fails(&["--duplicate-logits", "1"]);
    assert_eq!(error.code(), "worker_failed", "{error}");
    // The third logits reply under the previous request's ID.
    let error = env.train_fails(&["--stale-logits", "3"]);
    assert_eq!(error.code(), "worker_failed", "{error}");
    // After the save/reload the worker reports another identity.
    let error = env.train_fails(&["--drift-after-save"]);
    assert_eq!(error.code(), "worker_failed", "{error}");
    assert!(error.to_string().contains("identity"), "{error}");
}

#[test]
fn a_frozen_encoder_that_changes_during_training_is_refused() {
    let env = Env::new();
    let error = env.train_fails(&["--frozen-drift"]);
    assert_eq!(error.code(), "frozen_encoder_changed", "{error}");
}

#[test]
fn the_output_quota_is_enforced_per_file_and_in_total() {
    let env = Env::new();
    // The head grows past output_bytes (128 MiB): the hard per-file limit
    // (RLIMIT_FSIZE) stops the worker.
    let error = env.train_fails(&["--save-bloat-mb", "40"]);
    assert_eq!(error.code(), "output_limit", "{error}");
    // Small files whose total passes output_bytes: the supervised total.
    let error = env.train_fails(&["--save-extra-mb", "140"]);
    assert_eq!(error.code(), "output_limit", "{error}");
    assert_eq!(error.exit_code(), 1);
}

#[test]
fn losing_the_memory_measurement_stops_the_worker_by_name() {
    // The poll runs on the supervisor's own thread: arm the fault point in a
    // separate CLI process (process-wide), with a worker slow enough to be
    // measured.
    let env = Env::new();
    let out = env.path("candidate");
    let mut command = env.foundry(true);
    env.train_cli(&mut command, &out);
    command
        .arg("--development-isolation")
        .env(
            "FOUNDRY_TEST_FAULT",
            "ctxfoundry-fault/supervisor.footprint_measure=fail",
        )
        .env("FOUNDRY_TEST_LEARN_WORKER_ARGS", "--load-ms 3000");
    let output = run_bounded(command);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(error_code(&output), "memory_unmeasurable");
    env.assert_nothing_left(&out);
}

#[test]
fn cancellation_exits_130_with_no_output() {
    let env = Env::new();
    let out = env.path("candidate");
    let mut command = env.foundry(true);
    env.train_cli(&mut command, &out);
    command.arg("--development-isolation").env(
        "FOUNDRY_TEST_FAULT",
        "ctxfoundry-fault/learning.before_step=cancel:1",
    );
    let output = run_bounded(command);
    assert_eq!(output.status.code(), Some(130), "{output:?}");
    assert_eq!(error_code(&output), "cancelled");
    env.assert_nothing_left(&out);
}

#[test]
fn disk_full_during_head_and_manifest_writes_leaves_nothing_and_retries_cleanly() {
    let env = Env::new();
    // An earlier candidate beside the destination is preserved byte for byte.
    let (earlier, _) = env.train_ok("earlier");
    let earlier_manifest = read(&earlier.join("manifest.json"));
    let out = env.path("candidate");
    for point in [
        "head_write",
        "head_flush",
        "head_fsync",
        "manifest_write",
        "manifest_flush",
        "manifest_fsync",
    ] {
        let mut command = env.foundry(true);
        env.train_cli(&mut command, &out);
        command.arg("--development-isolation").env(
            "FOUNDRY_TEST_FAULT",
            format!("ctxfoundry-fault/learning.{point}=fail"),
        );
        let output = run_bounded(command);
        assert_eq!(output.status.code(), Some(1), "{point}: {output:?}");
        assert_eq!(error_code(&output), "output_write", "{point}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("No space left on device"),
            "{point}: {stderr}"
        );
        env.assert_nothing_left(&out);
        assert_eq!(read(&earlier.join("manifest.json")), earlier_manifest);
    }
    // The retry, with the disk free again, completes.
    let trained = env.train(&out, &[]).expect("the retry completes");
    assert!(!trained.adopted);
    assert!(out.join("manifest.json").exists());
}

// ---------------------------------------------------------------------------
// (a) F5: adoption only after full validation
// ---------------------------------------------------------------------------

#[test]
fn an_occupied_identical_candidate_is_adopted_only_when_every_member_validates() {
    let env = Env::new();
    let (out, first) = env.train_ok("candidate");
    // A lost response: the same run again finds exactly its candidate.
    let again = env.train(&out, &[]).expect("the identical rerun adopts");
    assert!(again.adopted);
    assert_eq!(
        again.candidate_manifest_sha256,
        first.candidate_manifest_sha256
    );
    let head = out.join("head.safetensors");
    let original = read(&head);
    // Identical manifest, truncated head: refused, untouched.
    std::fs::write(&head, &original[..original.len() - 4]).unwrap();
    let error = env
        .train(&out, &[])
        .expect_err("a truncated head is not adopted");
    assert_eq!(error.code(), "output_exists", "{error}");
    assert_eq!(read(&head).len(), original.len() - 4, "left untouched");
    // Identical manifest, same-length replaced head: refused.
    let mut replaced = original.clone();
    let last = replaced.len() - 1;
    replaced[last] ^= 0x01;
    std::fs::write(&head, &replaced).unwrap();
    let error = env
        .train(&out, &[])
        .expect_err("a replaced head is not adopted");
    assert_eq!(error.code(), "output_exists", "{error}");
    // A foreign directory at the destination is refused before any work.
    let foreign = env.path("foreign");
    std::fs::create_dir(&foreign).unwrap();
    std::fs::write(foreign.join("manifest.json"), b"{}").unwrap();
    let error = env.train(&foreign, &[]).expect_err("foreign destination");
    assert_eq!(error.code(), "output_exists", "{error}");
}

// ---------------------------------------------------------------------------
// (a) D6/D8 preflight refusals
// ---------------------------------------------------------------------------

#[test]
fn a_requested_enforcement_above_the_profile_is_refused_never_downgraded() {
    let env = Env::with(&Knobs {
        memory: "hard",
        ..Knobs::default()
    });
    let error = env.train(&env.path("never"), &[]).expect_err("hard memory");
    assert_eq!(error.code(), "enforcement_unavailable", "{error}");
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn an_empty_critical_slice_refuses() {
    let env = Env::with(&Knobs {
        critical_groups: vec!["no-such-group".into()],
        ..Knobs::default()
    });
    let error = env.train(&env.path("never"), &[]).expect_err("empty slice");
    assert_eq!(error.code(), "critical_slice_empty", "{error}");
}

#[test]
fn an_evaluation_state_without_a_coverage_line_refuses() {
    let env = Env::new();
    // Re-prepare after adding an evaluation row whose state the
    // deterministic comparator cannot project.
    let groups = &floor_groups()[2];
    env.record(&row(
        "bad",
        &groups[0],
        "search",
        "a query with no coverage line",
    ));
    let dataset = env.prepare("dataset2", &env.policy);
    let mut env = env;
    env.dataset = dataset;
    let error = env
        .train(&env.path("never"), &[])
        .expect_err("no projection");
    assert_eq!(error.code(), "state_invalid", "{error}");
}

#[test]
fn an_explicit_incumbent_that_is_missing_or_incompatible_refuses() {
    let env = Env::new();
    let out = env.path("never");
    let missing = env.path("no-such-candidate");
    let error = env
        .train_with(&out, &env.policy, &[], None, Some(&missing))
        .expect_err("missing incumbent");
    assert_eq!(error.code(), "incumbent_invalid", "{error}");
    // A compatible incumbent is compared on the same rows.
    let (incumbent, _) = env.train_ok("incumbent");
    let out = env.path("candidate");
    env.train_with(&out, &env.policy, &[], None, Some(&incumbent))
        .expect("training against an incumbent completes");
    let report = json_file(&out.join("evaluation.json"));
    assert_eq!(report["incumbent"]["rows"], 21);
    assert!(
        report["groups"]
            .as_array()
            .unwrap()
            .iter()
            .all(|g| g["incumbent"].is_number())
    );
    // An incumbent carrying an inherited per-option temperature is refused
    // by name (the manifest re-sealed so only the temperature objects).
    let tampered = env.path("tampered");
    copy_dir(&incumbent, &tampered);
    let mut manifest = json_file(&tampered.join("manifest.json"));
    manifest["temperature_by_options"] = json!({"choice:11+": 0.10058280825614929});
    std::fs::write(tampered.join("manifest.json"), manifest.to_string()).unwrap();
    let error = env
        .train_with(&env.path("never2"), &env.policy, &[], None, Some(&tampered))
        .expect_err("inherited temperature");
    assert_eq!(error.code(), "inherited_temperature", "{error}");
}

#[test]
fn a_base_must_be_the_pinned_eligible_candidate_of_the_parent_dataset() {
    let lenient = Knobs {
        lenient: true,
        ..Knobs::default()
    };
    let env = Env::with(&lenient);
    let (first, trained) = env.train_ok("first");
    assert!(trained.eligible, "{:?}", trained.reasons);
    // `base: null` pins the initial checkpoint; naming a base is refused.
    let error = env
        .train_with(&env.path("never"), &env.policy, &[], Some(&first), None)
        .expect_err("base without a pin");
    assert_eq!(error.code(), "base_invalid", "{error}");
    // Round two: one new training row, a policy pinning the first
    // candidate, and a child dataset of the first round's.
    let manifest_sha = digest_of(&read(&first.join("manifest.json")));
    let pinned = env.write_policy(
        "pinned.json",
        &Knobs {
            base: Some(manifest_sha.clone()),
            ..lenient.clone()
        },
    );
    env.record(&row(
        "t-new",
        &floor_groups()[0][0],
        "graph",
        "who calls t-new\ngraph: partial",
    ));
    let child = env.prepare_child("dataset2", &pinned, Some(&env.dataset));
    let mut env = env;
    env.dataset = child;
    // The pinned base must be named.
    let error = env
        .train_with(&env.path("never"), &pinned, &[], None, None)
        .expect_err("pinned base missing");
    assert_eq!(error.code(), "base_invalid", "{error}");
    // Another candidate than the pinned one is refused.
    let (other, _) = (env.path("other"), ());
    copy_dir(&first, &other);
    let mut manifest = json_file(&other.join("manifest.json"));
    manifest["training"]["seed"] = "another".into();
    std::fs::write(other.join("manifest.json"), manifest.to_string()).unwrap();
    let error = env
        .train_with(&env.path("never"), &pinned, &[], Some(&other), None)
        .expect_err("another base");
    assert_eq!(error.code(), "base_invalid", "{error}");
    // The pinned, eligible base of the parent dataset trains on: lineage
    // inherits its contributions; only the new row is a new fitting one.
    let out = env.path("second");
    env.train_with(&out, &pinned, &[], Some(&first), None)
        .expect("the repeat round completes");
    let manifest = json_file(&out.join("manifest.json"));
    assert_eq!(
        manifest["base"],
        json!({"kind": "candidate", "manifest_sha256": manifest_sha})
    );
    let contributions = json_file(&out.join("contributions.json"));
    assert_eq!(contributions["fitting"]["new"].as_array().unwrap().len(), 1);
    assert_eq!(
        contributions["fitting"]["inherited"]
            .as_array()
            .unwrap()
            .len(),
        22
    );
    assert_eq!(
        contributions["calibration"]["inherited"]
            .as_array()
            .unwrap()
            .len(),
        11
    );
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
    }
}

// ---------------------------------------------------------------------------
// (a) The child: environment, descriptors, limits, owner death
// ---------------------------------------------------------------------------

#[test]
fn the_worker_inherits_no_environment_or_descriptors_and_cannot_spawn() {
    let env = Env::new();
    // A descriptor this process leaves inheritable must not reach the
    // worker (the supervisor's child setup closes it).
    let file = std::fs::File::open(&env.policy).unwrap();
    let leaked = unsafe { libc::dup(std::os::fd::AsRawFd::as_raw_fd(&file)) };
    assert!(leaked > 2);
    let report_path = env.path("env.json");
    let report_arg = report_path.display().to_string();
    env.train(&env.path("candidate"), &["--env-report", &report_arg])
        .expect("training completes");
    unsafe { libc::close(leaked) };
    let report = json_file(&report_path);
    let names: Vec<&str> = report["env"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    // Exactly what the supervisor set, plus only what macOS itself injects
    // into every process (`__CF_USER_TEXT_ENCODING`); the test process's
    // own environment (CARGO_*, RUST_*, HOME of the user…) never arrives.
    let set: Vec<&str> = names
        .iter()
        .copied()
        .filter(|name| !name.starts_with("__CF"))
        .collect();
    assert_eq!(
        set,
        ["HOME", "OMP_NUM_THREADS", "PATH", "TMPDIR"],
        "{names:?}"
    );
    let fds: Vec<i64> = report["fds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    // The inherited liveness pipe plus the worker's own kqueue and private
    // frame channel: nothing else.
    assert_eq!(fds.len(), 3, "{fds:?}");
    // Zero descendants: the hard process limit refuses a spawn (EAGAIN).
    assert_eq!(report["spawn_errno"], libc::EAGAIN, "{report}");
    assert_eq!(report["limits"]["nproc"], 0);
    assert_eq!(
        report["limits"]["cpu"],
        120 * 2,
        "wall_seconds × cpu_threads"
    );
    assert_eq!(report["limits"]["fsize"], 128u64 << 20, "output_bytes");
    // The positive control: this process can spawn.
    assert!(Command::new("/usr/bin/true").status().unwrap().success());
    // Its working directory was the private scratch run directory, now gone.
    let cwd = PathBuf::from(report["cwd"].as_str().unwrap());
    assert!(cwd.starts_with(&env.scratch_root), "{cwd:?}");
    assert!(!cwd.exists());
}

#[test]
fn owner_death_ends_the_worker_within_two_seconds() {
    use context_foundry::learning::ipc::{Identity, LEARN_PROTOCOL, LearnHeader, Message};
    let env = Env::new();
    let checkpoint = env.path("checkpoint");
    let pid_file = env.path("worker.pid");
    let mut shim = Command::new(fake_exe())
        .arg("--shim")
        .arg("--checkpoint-dir")
        .arg(&checkpoint)
        .args(["--load-ms", "20000", "--pid-file"])
        .arg(&pid_file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdout = shim.stdout.take().unwrap();
    let mut stdin = shim.stdin.take().unwrap();
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    while stdout.read(&mut byte).unwrap() == 1 && byte[0] != b'\n' {
        line.push(byte[0]);
    }
    let line = String::from_utf8(line).unwrap();
    let worker: u32 = line
        .strip_prefix("shim-worker-pid ")
        .and_then(|pid| pid.parse().ok())
        .unwrap_or_else(|| panic!("shim printed {line:?}"));
    let policy: serde_json::Value = serde_json::from_slice(&read(&env.policy)).unwrap();
    context_foundry::neural::protocol::write_frame_as(
        &mut stdin,
        &LearnHeader {
            protocol: LEARN_PROTOCOL,
            request_id: 1,
            identity: Identity {
                model_function_sha256: "x".into(),
                head_sha256: None,
                steps: 0,
            },
            message: Message::Load {
                checkpoint: serde_json::from_value(policy["model"].clone()).unwrap(),
                head: None,
                threads: 1,
                seed: 1,
            },
        },
        &[],
    )
    .unwrap();
    // The worker is inside its (20 s) load when its owner dies.
    let deadline = Instant::now() + Duration::from_secs(10);
    while !pid_file.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(200));
    shim.kill().unwrap();
    shim.wait().unwrap();
    let killed = Instant::now();
    while unsafe { libc::kill(worker as libc::pid_t, 0) } == 0 {
        assert!(
            killed.elapsed() < Duration::from_secs(2),
            "the worker outlived its owner by 2 s"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

// ---------------------------------------------------------------------------
// (c) The real upstream temperatures are refused
// ---------------------------------------------------------------------------

/// A structurally complete candidate manifest around `temperature`, plus
/// whatever `extra` fields.
fn manifest_with(temperature: serde_json::Value, extra: &[(&str, serde_json::Value)]) -> Vec<u8> {
    let mut manifest = json!({
        "schema": 4,
        "kind": "candidate",
        "recipe": learning::RECIPE,
        "workspace_id": "w",
        "model_function_sha256": "f".repeat(64),
        "dataset_manifest_sha256": "d".repeat(64),
        "dataset_id": "i".repeat(64),
        "policy_sha256": "p".repeat(64),
        "base": {"kind": "initial"},
        "encoder": {
            "checkpoint": {
                "weights_sha256": "1".repeat(64),
                "encoder_config_sha256": "2".repeat(64),
                "source_dtype": "F16",
            },
            "frozen_encoder_sha256": "3".repeat(64),
        },
        "tokenizer": {"json_sha256": "4".repeat(64), "config_sha256": "5".repeat(64)},
        "training": {
            "seed": "s",
            "max_steps": 1,
            "steps_completed": 1,
            "epochs": 1,
            "train_rows": 1,
            "optimizer": serde_json::to_value(learning::OptimizerPin::recipe()).unwrap(),
            "head_dropout": 0.1,
            "batch": 1,
            "first_loss": 0.7,
            "last_loss": 0.6,
            "clipped_steps": 0,
        },
        "temperature": temperature,
        "calibration_rows": 10,
        "calibration_mean_nll": 0.6,
        "evaluation_rows": 20,
        "eligible": false,
        "probe": {"example_id": "e", "logits": [0.1, 0.2]},
        "files": candidate::FILES.iter().map(|name| json!({"name": name, "sha256": "0".repeat(64), "bytes": 0})).collect::<Vec<_>>(),
    });
    for (key, value) in extra {
        manifest[*key] = value.clone();
    }
    manifest.to_string().into_bytes()
}

fn rl_agent_config() -> serde_json::Value {
    let path = checkpoint_dir().join("rl_agent_config.json");
    serde_json::from_slice(&std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "the pinned checkpoint's rl_agent_config.json is missing ({}): {e}; set \
             CONTEXT_FOUNDRY_013_CHECKPOINT",
            path.display()
        )
    }))
    .unwrap()
}

#[test]
fn the_real_upstream_bucket_temperatures_and_a_scalar_below_half_are_refused() {
    let config = rl_agent_config();
    let buckets = config["temperature_by_options"].clone();
    assert_eq!(buckets["choice:11+"].as_f64(), Some(0.10058280825614929));
    // A candidate carrying the inherited per-option table.
    let bytes = manifest_with(json!(1.0), &[("temperature_by_options", buckets.clone())]);
    assert_eq!(
        candidate::parse_manifest(&bytes, "candidate_invalid")
            .unwrap_err()
            .code(),
        "inherited_temperature"
    );
    // The upstream per-type `temperature` array in the scalar's place.
    let bytes = manifest_with(config["temperature"].clone(), &[]);
    assert_eq!(
        candidate::parse_manifest(&bytes, "candidate_invalid")
            .unwrap_err()
            .code(),
        "inherited_temperature"
    );
    // Laya's over-confident `choice:11+` value as the fitted scalar.
    let bytes = manifest_with(buckets["choice:11+"].clone(), &[]);
    assert_eq!(
        candidate::parse_manifest(&bytes, "candidate_invalid")
            .unwrap_err()
            .code(),
        "temperature_invalid"
    );
    // Just below the floor, and off the grid, both refuse; the floor and the
    // grid's interior are admitted.
    for bad in [0.499_999, 1.01] {
        let bytes = manifest_with(json!(bad), &[]);
        assert_eq!(
            candidate::parse_manifest(&bytes, "candidate_invalid")
                .unwrap_err()
                .code(),
            "temperature_invalid",
            "{bad}"
        );
    }
    for good in [0.5, 1.05, 3.0] {
        candidate::parse_manifest(&manifest_with(json!(good), &[]), "candidate_invalid")
            .unwrap_or_else(|e| panic!("{good}: {e}"));
    }
}

// ---------------------------------------------------------------------------
// Development bundle and measurement phase (ignored; run by the captain)
// ---------------------------------------------------------------------------

/// The signed development profile (`CF_LEARN_DEV_PROFILE`), built by
/// `scripts/learn-worker-bundle.sh` around `foundry-learn`.
fn dev_profile() -> learning::profile::LearnProfile {
    let path = std::env::var("CF_LEARN_DEV_PROFILE")
        .unwrap_or_else(|_| "/private/tmp/cf-013-dev/learn-profile.json".into());
    learning::profile::LearnProfile::load(Path::new(&path)).expect("load the development profile")
}

/// Negative isolation probes with positive controls against the signed
/// learning bundle (009's probes, through `foundry-learn --probe`), plus
/// unsandboxed controls through the raw binary.
#[test]
#[ignore = "measurement phase: signed sandbox bundle required (scripts/learn-worker-bundle.sh)"]
fn dev_learn_sandbox_negative_probes() {
    use context_foundry::neural::supervisor::child_setup;
    let profile = dev_profile();
    let executable = profile.executable();
    let scratch = &profile.worker.scratch_root;
    std::fs::create_dir_all(scratch).ok();
    let outside = tempfile::tempdir_in("/private/tmp").unwrap();
    let sentinel = outside.path().join("outside-sentinel");
    std::fs::write(&sentinel, b"sentinel").unwrap();
    let probe = |name: &str, args: &[&str]| -> serde_json::Value {
        let (reader, writer) = std::io::pipe().unwrap();
        let read_fd = std::os::fd::AsRawFd::as_raw_fd(&reader);
        let mut command = Command::new(&executable);
        command
            .arg("--probe")
            .arg(name)
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", scratch)
            .env("TMPDIR", scratch)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        // SAFETY: the supervisor's own child setup.
        unsafe { std::os::unix::process::CommandExt::pre_exec(&mut command, child_setup(read_fd)) };
        let output = command.output().unwrap();
        drop((reader, writer));
        serde_json::from_slice(&output.stdout).unwrap()
    };
    let denied = |what: &str, verdict: &serde_json::Value, errnos: &[i32]| {
        assert_eq!(verdict["allowed"], false, "{what}: {verdict}");
        let errno = verdict["errno"].as_i64().unwrap_or(-1) as i32;
        assert!(errnos.contains(&errno), "{what}: errno {errno}: {verdict}");
    };
    let eperm = [libc::EPERM, libc::EACCES];
    denied(
        "outside read",
        &probe("read", &[&sentinel.display().to_string()]),
        &eperm,
    );
    denied(
        "outside write",
        &probe(
            "write",
            &[&outside.path().join("escape").display().to_string()],
        ),
        &eperm,
    );
    denied(
        "checkpoint write",
        &probe(
            "write",
            &[&profile.checkpoint_dir.join("escape").display().to_string()],
        ),
        &eperm,
    );
    denied(
        "libtorch write",
        &probe(
            "write",
            &[&profile.libtorch_dir.join("escape").display().to_string()],
        ),
        &eperm,
    );
    let granted = probe(
        "read",
        &[&profile
            .checkpoint_dir
            .join("encoder/config.json")
            .display()
            .to_string()],
    );
    assert_eq!(
        granted["allowed"], true,
        "granted checkpoint read: {granted}"
    );
    let granted = probe(
        "read",
        &[&profile
            .libtorch_dir
            .join("libtorch.dylib")
            .display()
            .to_string()],
    );
    assert_eq!(granted["allowed"], true, "granted LibTorch read: {granted}");
    let scratch_write = probe(
        "write",
        &[&scratch.join("probe-write").display().to_string()],
    );
    assert_eq!(
        scratch_write["allowed"], true,
        "scratch write: {scratch_write}"
    );
    // A symlink inside the scratch grant pointing out is judged by target.
    let escape = scratch.join("escape-link");
    let _ = std::fs::remove_file(&escape);
    std::os::unix::fs::symlink(&sentinel, &escape).unwrap();
    denied(
        "read through a link",
        &probe("read", &[&escape.display().to_string()]),
        &eperm,
    );
    denied(
        "write through a link",
        &probe("write", &[&escape.display().to_string()]),
        &eperm,
    );
    assert_eq!(read(&sentinel), b"sentinel");
    // Network, child execution, limit restoration.
    let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    denied(
        "tcp connect",
        &probe("tcp-connect", &[&tcp.local_addr().unwrap().to_string()]),
        &eperm,
    );
    denied("tcp listen", &probe("tcp-listen", &["127.0.0.1:0"]), &eperm);
    assert_eq!(probe("dns", &["example.com"])["allowed"], false);
    let no_process = [libc::EAGAIN, libc::EPERM, libc::EACCES];
    denied("fork", &probe("fork", &[]), &no_process);
    denied("spawn", &probe("spawn", &[]), &no_process);
    denied("nproc restore", &probe("nproc-restore", &[]), &eperm);
    // Inherited environment and descriptors.
    let fds = probe("fds", &[]);
    let open: Vec<i64> = serde_json::from_str(fds["detail"].as_str().unwrap()).unwrap();
    assert_eq!(open.len(), 1, "only the liveness fd: {open:?}");
    // Unsandboxed controls through the raw binary: the same probes succeed.
    let raw = std::env::var("CF_LEARN_RAW_WORKER")
        .unwrap_or_else(|_| "target/debug/foundry-learn".into());
    let control = |name: &str, args: &[&str]| -> serde_json::Value {
        let output = Command::new(&raw)
            .arg("--probe")
            .arg(name)
            .args(args)
            .output()
            .unwrap();
        serde_json::from_slice(&output.stdout).unwrap()
    };
    assert_eq!(
        control("read", &[&sentinel.display().to_string()])["allowed"],
        true
    );
    assert_eq!(control("fork", &[])["allowed"], true);
    assert_eq!(
        control("tcp-connect", &[&tcp.local_addr().unwrap().to_string()])["allowed"],
        true
    );
}

// ---------------------------------------------------------------------------
// (b) The real model: parity with the reference and training properties
// ---------------------------------------------------------------------------

#[cfg(feature = "learning-worker")]
mod real {
    use super::*;
    use context_foundry::decision_model::net::{Mode, Model};
    use std::sync::Mutex;
    use tch::Tensor;

    /// One real model at a time: each holds about 2 GiB, and the head
    /// dropout draws from LibTorch's global generator.
    static LOCK: Mutex<()> = Mutex::new(());

    const ATOL: f64 = 1e-5;
    const RTOL: f64 = 1e-4;

    fn reference() -> serde_json::Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/learning/reference-t002.json");
        serde_json::from_slice(&read(&path)).unwrap()
    }

    fn load() -> Model {
        let dir = checkpoint_dir();
        assert!(
            dir.join("model.safetensors").exists(),
            "the pinned checkpoint is missing at {}; set CONTEXT_FOUNDRY_013_CHECKPOINT",
            dir.display()
        );
        tch::set_num_threads(4);
        Model::load(&dir).expect("the pinned checkpoint loads")
    }

    thread_local! {
        /// Largest absolute and relative error seen, per quantity.
        static WORST: std::cell::RefCell<std::collections::BTreeMap<&'static str, (f64, f64)>> =
            const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
    }

    fn quantity(what: &str) -> &'static str {
        let after = what.contains("after");
        match () {
            _ if what.contains("hidden") => "hidden rows",
            _ if what.contains("logit") && after => "logits after the sequence",
            _ if what.contains("logit") => "logits",
            _ if what.contains("pre-clip") => "sequence pre-clip norms",
            _ if what.starts_with("step") => "sequence losses",
            _ if what.contains("loss") => "losses",
            _ if what.contains("global gradient norm") => "global gradient norms",
            _ if after => "parameters after the sequence",
            _ => "gradients",
        }
    }

    fn report_worst() {
        WORST.with(|worst| {
            for (quantity, (abs, rel)) in worst.borrow().iter() {
                println!("PARITY {quantity}: max abs {abs:.3e}, max rel {rel:.3e}");
            }
        });
    }

    fn close(what: &str, got: f64, want: f64) {
        let abs = (got - want).abs();
        let rel = if want == 0.0 { 0.0 } else { abs / want.abs() };
        WORST.with(|worst| {
            let mut worst = worst.borrow_mut();
            let entry = worst.entry(quantity(what)).or_insert((0.0, 0.0));
            entry.0 = entry.0.max(abs);
            entry.1 = entry.1.max(rel);
        });
        assert!(
            (got - want).abs() <= ATOL + RTOL * want.abs(),
            "{what}: {got} vs reference {want} (atol {ATOL}, rtol {RTOL})"
        );
    }

    fn values(t: &Tensor) -> Vec<f64> {
        Vec::<f32>::try_from(&t.detach().contiguous().view([-1]))
            .unwrap()
            .into_iter()
            .map(f64::from)
            .collect()
    }

    /// The reference's sample rule: SHA-256(`<name>:<k>`) first eight bytes
    /// big-endian modulo numel, k = 0..63, sorted unique.
    fn sample_idx(name: &str, n: usize) -> Vec<usize> {
        use sha2::{Digest, Sha256};
        let mut idx: Vec<usize> = (0..64)
            .map(|k| {
                let digest = Sha256::digest(format!("{name}:{k}").as_bytes());
                (u64::from_be_bytes(digest[..8].try_into().unwrap()) % n as u64) as usize
            })
            .collect();
        idx.sort_unstable();
        idx.dedup();
        idx
    }

    /// Compare one tensor with the reference's description of it: L2, sum,
    /// the sampled entries (and every entry when the reference has them).
    fn compare(what: &str, name: &str, got: &[f64], want: &serde_json::Value) {
        let l2 = got.iter().map(|v| v * v).sum::<f64>().sqrt();
        let sum: f64 = got.iter().sum();
        close(
            &format!("{what} {name} L2"),
            l2,
            want["l2"].as_f64().unwrap(),
        );
        close(
            &format!("{what} {name} sum"),
            sum,
            want["sum"].as_f64().unwrap(),
        );
        let idx = sample_idx(name, got.len());
        let want_idx: Vec<usize> = want["idx"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect();
        assert_eq!(idx, want_idx, "{what} {name}: the sample rule");
        for (i, want) in idx.iter().zip(want["values"].as_array().unwrap()) {
            close(
                &format!("{what} {name}[{i}]"),
                got[*i],
                want.as_f64().unwrap(),
            );
        }
        if let Some(full) = want.get("full").and_then(|f| f.as_array()) {
            assert_eq!(full.len(), got.len());
            for (i, want) in full.iter().enumerate() {
                close(
                    &format!("{what} {name}[{i}] (full)"),
                    got[i],
                    want.as_f64().unwrap(),
                );
            }
        }
    }

    fn case_input(case: &serde_json::Value) -> (Vec<u32>, [usize; 2]) {
        let ids: Vec<u32> = case["ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u32)
            .collect();
        let m = case["markers"].as_array().unwrap();
        (
            ids,
            [
                m[0].as_u64().unwrap() as usize,
                m[1].as_u64().unwrap() as usize,
            ],
        )
    }

    fn pair(t: &Tensor) -> [f64; 2] {
        let v = values(t);
        [v[0], v[1]]
    }

    /// Parity: every case's logits, loss, gradient norm, sampled encoder
    /// rows and every trainable tensor's gradient (L2, sum, samples, the
    /// whole final scorer layer); then the three-step AdamW sequence (two
    /// steps engage the clip): per-step loss and pre-clip norm, the
    /// parameters after it and every case's logits after it. Eval mode,
    /// dropout off, as the reference ran.
    #[test]
    fn parity_with_the_reference_fixture() {
        let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let fixture = reference();
        assert_eq!(fixture["tolerance"]["atol"], ATOL);
        assert_eq!(fixture["tolerance"]["rtol"], RTOL);
        let mut model = load();
        assert_eq!(model.source_dtype.as_str(), "F16");
        assert_eq!(model.weights_sha256, fixture["checkpoint_model_sha256"]);
        let trainable: Vec<&str> = fixture["trainable"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let names: Vec<&str> = model.params().iter().map(|(n, _)| *n).collect();
        assert_eq!(names, trainable, "the contract's trainable set, in order");
        let mut hidden_of = std::collections::BTreeMap::new();
        for case in fixture["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let (ids, markers) = case_input(case);
            let hidden = model.encode(&ids);
            // Encoder rows: CLS and both markers (full/local attention
            // boundaries across 34, 74, 157 and 1024 tokens).
            for (row, want) in case["hidden_rows"].as_object().unwrap() {
                let r: i64 = row.parse().unwrap();
                let got = values(&hidden.get(0).get(r));
                compare(name, &format!("hidden:{r}"), &got, want);
            }
            let target = case["target_index"].as_u64().unwrap() as usize;
            let (loss, logits) = model
                .backward(&hidden, markers, target, Mode::Eval)
                .unwrap();
            let want = case["logits"].as_array().unwrap();
            close(
                &format!("{name} logit 0"),
                f64::from(logits[0]),
                want[0].as_f64().unwrap(),
            );
            close(
                &format!("{name} logit 1"),
                f64::from(logits[1]),
                want[1].as_f64().unwrap(),
            );
            close(
                &format!("{name} loss"),
                loss,
                case["loss"].as_f64().unwrap(),
            );
            let mut norm = 0.0;
            for (tensor, grad) in model.grads() {
                let got = values(&grad);
                norm += got.iter().map(|v| v * v).sum::<f64>();
                compare(name, tensor, &got, &case["gradients"][tensor]);
            }
            close(
                &format!("{name} global gradient norm"),
                norm.sqrt(),
                case["grad_global_norm"].as_f64().unwrap(),
            );
            hidden_of.insert(name.to_owned(), (hidden, markers));
        }
        // The three-step AdamW sequence, one optimizer from the base weights.
        let rows_before = values(model.frozen_type_rows());
        let sequence = &fixture["adamw_sequence"];
        for (i, step) in sequence["steps"].as_array().unwrap().iter().enumerate() {
            let (hidden, markers) = &hidden_of[step["case"].as_str().unwrap()];
            let target = step["target_index"].as_u64().unwrap() as usize;
            let (loss, _) = model
                .backward(hidden, *markers, target, Mode::Eval)
                .unwrap();
            let norm = model.clip_and_step().unwrap();
            close(
                &format!("step {i} loss"),
                loss,
                step["loss"].as_f64().unwrap(),
            );
            close(
                &format!("step {i} pre-clip norm"),
                norm,
                step["grad_norm_before_clip"].as_f64().unwrap(),
            );
            assert_eq!(
                norm > 1.0,
                step["clipped"].as_bool().unwrap(),
                "step {i} clip"
            );
        }
        for (tensor, value) in model.params() {
            compare(
                "after",
                tensor,
                &values(value),
                &sequence["params_after"][tensor],
            );
        }
        assert_eq!(
            values(model.frozen_type_rows()),
            rows_before,
            "type rows 1-2 frozen"
        );
        for (name, want) in sequence["logits_after"].as_object().unwrap() {
            let (hidden, markers) = &hidden_of[name];
            let got = pair(&tch::no_grad(|| {
                model.head_logits(hidden, *markers, Mode::Eval)
            }));
            let want = want.as_array().unwrap();
            close(
                &format!("{name} logit 0 after"),
                got[0],
                want[0].as_f64().unwrap(),
            );
            close(
                &format!("{name} logit 1 after"),
                got[1],
                want[1].as_f64().unwrap(),
            );
        }
        report_worst();
    }

    /// float16 bits → float32, exactly (the upcast the loader performs).
    fn f16_to_f32(bits: u16) -> f32 {
        let sign = u32::from(bits >> 15) << 31;
        let exponent = (bits >> 10) & 0x1f;
        let mantissa = u32::from(bits & 0x3ff);
        let value = match exponent {
            0 => (mantissa as f32) * 2f32.powi(-24),
            0x1f => f32::INFINITY,
            e => f32::from_bits(((u32::from(e) + 112) << 23) | (mantissa << 13)),
        };
        f32::from_bits(value.to_bits() | sign)
    }

    #[test]
    fn the_numerical_model_trains_only_its_trainable_set_reproducibly() {
        use context_foundry::decision_model::safetensors;
        let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let fixture = reference();
        let case = &fixture["cases"][1];
        let (ids, markers) = case_input(case);
        let mut model = load();
        // F16 → F32: a trainable tensor holds exactly the upcast checkpoint
        // values.
        let bytes = read(&checkpoint_dir().join("model.safetensors"));
        let len = safetensors::header_len(bytes[..8].try_into().unwrap(), "x").unwrap() as usize;
        let entries =
            safetensors::parse_header(&bytes[8..8 + len], (bytes.len() - 8 - len) as u64, "x")
                .unwrap();
        let bias = entries.iter().find(|e| e.name == "scorer.1.bias").unwrap();
        assert_eq!(bias.dtype, safetensors::Dtype::F16);
        let raw = &bytes[8 + len + bias.start as usize..8 + len + bias.end as usize];
        let upcast: Vec<f64> = raw
            .chunks_exact(2)
            .map(|b| f64::from(f16_to_f32(u16::from_le_bytes([b[0], b[1]]))))
            .collect();
        let (_, loaded) = model
            .params()
            .into_iter()
            .find(|(n, _)| *n == "scorer.1.bias")
            .unwrap();
        assert_eq!(loaded.kind(), tch::Kind::Float);
        assert_eq!(values(loaded), upcast);
        drop(bytes);

        // Deterministic evaluation.
        assert_eq!(
            model.logits(&ids, markers).unwrap(),
            model.logits(&ids, markers).unwrap()
        );
        let hidden = model.encode(&ids);
        let eval = pair(&tch::no_grad(|| {
            model.head_logits(&hidden, markers, Mode::Eval)
        }));
        // The parity knob: train mode with dropout 0 IS evaluation, in the
        // logits and in every gradient.
        let knob = pair(&tch::no_grad(|| {
            model.head_logits(&hidden, markers, Mode::Train { dropout: 0.0 })
        }));
        assert_eq!(knob, eval);
        model.backward(&hidden, markers, 1, Mode::Eval).unwrap();
        let eval_grads: Vec<Vec<f64>> = model.grads().iter().map(|(_, g)| values(g)).collect();
        model
            .backward(&hidden, markers, 1, Mode::Train { dropout: 0.0 })
            .unwrap();
        let knob_grads: Vec<Vec<f64>> = model.grads().iter().map(|(_, g)| values(g)).collect();
        assert_eq!(knob_grads, eval_grads);
        // Seeded train dropout: same seed identical, another seed differs,
        // and train (p 0.1) differs from eval.
        let dropout = Mode::Train { dropout: 0.1 };
        tch::manual_seed(7);
        let a = pair(&tch::no_grad(|| {
            model.head_logits(&hidden, markers, dropout)
        }));
        tch::manual_seed(7);
        let b = pair(&tch::no_grad(|| {
            model.head_logits(&hidden, markers, dropout)
        }));
        tch::manual_seed(8);
        let c = pair(&tch::no_grad(|| {
            model.head_logits(&hidden, markers, dropout)
        }));
        assert_eq!(a, b, "the same seed reproduces the dropout");
        assert_ne!(a, c, "another seed draws another dropout");
        assert_ne!(a, eval, "training with p 0.1 is not evaluation");

        // Training: every trainable group gets a nonzero gradient and
        // changes; the frozen encoder and type rows 1-2 do not.
        let frozen = model.frozen_encoder_sha256();
        let rows = values(model.frozen_type_rows());
        let before: Vec<Vec<f64>> = model.params().iter().map(|(_, t)| values(t)).collect();
        tch::manual_seed(11);
        let (loss, norm) = model.train_step(&ids, markers, 1).unwrap();
        assert!(loss.is_finite() && norm > 0.0);
        for (name, grad) in model.grads() {
            assert!(
                values(&grad).iter().any(|v| *v != 0.0),
                "{name} got no gradient"
            );
        }
        for ((name, after), before) in model.params().iter().zip(&before) {
            assert_ne!(values(after), *before, "{name} did not change");
        }
        for _ in 0..3 {
            model.train_step(&ids, markers, 0).unwrap();
        }
        assert_eq!(
            model.frozen_encoder_sha256(),
            frozen,
            "the encoder is frozen"
        );
        assert_eq!(
            values(model.frozen_type_rows()),
            rows,
            "type rows 1-2 are bit-identical"
        );

        // Saved and reloaded logits equal training-eval logits exactly.
        let trained = model.logits(&ids, markers).unwrap();
        let head = model.head_bytes();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("head.safetensors");
        std::fs::write(&path, &head).unwrap();
        let mut fresh = load();
        assert_ne!(fresh.logits(&ids, markers).unwrap(), trained);
        fresh.load_head(&read(&path)).unwrap();
        assert_eq!(fresh.logits(&ids, markers).unwrap(), trained);
        assert_eq!(fresh.head_bytes(), head);
    }

    /// Opt-in exhaustive elementwise comparison with the full float32
    /// reference (`CONTEXT_FOUNDRY_013_FULL_REFERENCE` →
    /// reference-t002-full.safetensors; its SHA-256 is in the fixture).
    #[test]
    #[ignore = "measurement phase: exhaustive reference-t002-full.safetensors comparison (315 MB, not committed)"]
    fn exhaustive_elementwise_parity_with_the_full_reference() {
        let Some(path) = std::env::var_os("CONTEXT_FOUNDRY_013_FULL_REFERENCE") else {
            eprintln!(
                "skipped: set CONTEXT_FOUNDRY_013_FULL_REFERENCE to reference-t002-full.safetensors"
            );
            return;
        };
        let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let fixture = reference();
        let bytes = read(Path::new(&path));
        assert_eq!(digest_of(&bytes), fixture["full_reference"]["sha256"]);
        let full: std::collections::BTreeMap<String, Tensor> = Tensor::read_safetensors(&path)
            .unwrap()
            .into_iter()
            .collect();
        let mut model = load();
        let elementwise = |what: &str, got: &Tensor, want: &Tensor| {
            let (got, want) = (values(got), values(want));
            assert_eq!(got.len(), want.len(), "{what}");
            for (i, (g, w)) in got.iter().zip(&want).enumerate() {
                close(&format!("{what}[{i}]"), *g, *w);
            }
        };
        let mut hidden_of = std::collections::BTreeMap::new();
        for case in fixture["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap().to_owned();
            let (ids, markers) = case_input(case);
            let hidden = model.encode(&ids);
            let target = case["target_index"].as_u64().unwrap() as usize;
            model
                .backward(&hidden, markers, target, Mode::Eval)
                .unwrap();
            for (tensor, grad) in model.grads() {
                if let Some(want) = full.get(&format!("grad/{name}/{tensor}")) {
                    elementwise(&format!("grad/{name}/{tensor}"), &grad, want);
                }
            }
            hidden_of.insert(name, (hidden, markers));
        }
        for step in fixture["adamw_sequence"]["steps"].as_array().unwrap() {
            let (hidden, markers) = &hidden_of[step["case"].as_str().unwrap()];
            let target = step["target_index"].as_u64().unwrap() as usize;
            model
                .backward(hidden, *markers, target, Mode::Eval)
                .unwrap();
            model.clip_and_step().unwrap();
        }
        for (tensor, value) in model.params() {
            elementwise(
                &format!("after/{tensor}"),
                value,
                &full[&format!("after/{tensor}")],
            );
        }
    }
}
