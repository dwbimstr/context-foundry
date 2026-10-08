//! 013 T003 acceptance (SC-003): serving, selection, rollback and the
//! retired legacy path, over the real request path (`mcp::context_primary`,
//! the function the MCP owner and the CLI both call), the real candidate
//! artifacts T002 publishes, and `foundry-learn-fake` behind the real
//! supervised spawn. Fault hooks are the fake's (`--predict-*`); every
//! number they inject goes through the core's own validation.
//!
//! `--features learning-worker` adds the real-checkpoint parity test: the
//! served function's probabilities equal T002's evaluation probabilities
//! for the same rows.
//!
//! Deferred to the measurement phase (owner directive 2026-10-05): the
//! combined 009+013 residency test and any real latency figure
//! (`docs/learning.md`). Enablement is the offline economics gate that
//! `learning select` enforces and owner startup re-checks (contract,
//! 2026-10-06), tested here.
#![cfg(all(feature = "semantic", target_os = "macos"))]
use context_foundry::learning::{self, train};
use context_foundry::policy::{self, Policy, PolicyServing};
use context_foundry::response::{self, Budget};
use context_foundry::{Control, Engine, FoundryError, Strategy, testkit};
use protobuf::{EnumOrUnknown, Message as _};
use scip::types::{Document, Index, Occurrence, PositionEncoding};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const MODELS: &str = "VSC_DEV/models/laya-typed-decisions-1a793eb5";
const DEF: i32 = 1;
const SOURCES: [(&str, &str); 4] = [
    ("src/a.rs", "pub fn alpha_one() {}\n"),
    ("src/b.rs", "pub fn alpha_two() {}\n"),
    ("src/c.rs", "pub fn alpha_three() {}\n"),
    ("src/d.rs", "pub fn alpha_four() {}\n"),
];
/// Deterministic routing sends this to search, the next one to graph.
const SEARCH_QUERY: &str = "where is alpha_one defined";
const GRAPH_QUERY: &str = "who calls alpha_one";

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

fn json_file(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&read(path)).unwrap()
}

fn fake_exe() -> &'static str {
    env!("CARGO_BIN_EXE_foundry-learn-fake")
}

const BIN: &str = env!("CARGO_BIN_EXE_foundry");

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
        "label_source": "task_checker",
        "label_evidence": checked(label),
        "rights_ref": "rights-checked",
        "allow_training": true,
    })
}

/// Task-checker evidence as the 013 labeling run writes it: `(pass,
/// tokens)` for search and graph.
fn evidence(search: (bool, u64), graph: (bool, u64)) -> String {
    json!({
        "checker": "task-checker-v1",
        "search": {"pass": search.0, "tokens": search.1},
        "graph": {"pass": graph.0, "tokens": graph.1},
    })
    .to_string()
}

/// The fixture's evidence, by the checker's labeling rule: a `graph` task is
/// delivered by graph alone, a `search` task by both, search with fewer
/// tokens. Deterministic routing sends every fixture query to search, so
/// routing a task to graph never loses evidence and gains it on `graph`
/// tasks: a candidate that routes any `graph` task there passes the
/// economics gate.
fn checked(label: &str) -> String {
    if label == "graph" {
        evidence((false, 2048), (true, 1900))
    } else {
        evidence((true, 1500), (true, 1700))
    }
}

fn occurrence(range: &[i32], symbol: &str, roles: i32) -> Occurrence {
    let mut occurrence = Occurrence::new();
    occurrence.range = range.to_vec();
    occurrence.symbol = symbol.to_owned();
    occurrence.symbol_roles = roles;
    occurrence
}

/// Which worker an environment runs.
#[derive(Clone, Copy, PartialEq)]
enum Worker {
    Fake,
    #[cfg(feature = "learning-worker")]
    Real,
}

/// The run policy's selection knobs.
#[derive(Clone, Copy)]
struct Selection {
    threshold: f64,
    /// Floors a fake candidate cannot meet: an ineligible candidate.
    strict: bool,
}

impl Selection {
    fn lenient(threshold: f64) -> Self {
        Self {
            threshold,
            strict: false,
        }
    }
}

/// A store with indexed sources, a CURRENT compiler graph (one SCIP import
/// at the current revision), learning rows meeting every floor, a
/// checkpoint directory (fake or real) holding the pinned tokenizer, a
/// bundle around the worker and its isolation profile. Candidates are
/// trained at `threshold` under lenient floors (eligible) unless asked.
struct Env {
    dir: tempfile::TempDir,
    store: PathBuf,
    ws: PathBuf,
    checkpoint: PathBuf,
    profile: PathBuf,
    workspace_id: String,
    cpu_threads: u32,
    threshold: f64,
}

impl Env {
    fn new(threshold: f64) -> Self {
        Self::with(Worker::Fake, 2, threshold)
    }

    fn with(worker: Worker, cpu_threads: u32, threshold: f64) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let ws = root.join("ws");
        std::fs::create_dir(&ws).unwrap();
        let store = root.join("store");
        let mut engine = Engine::initialize(&store, &ws).unwrap();
        for (path, body) in SOURCES {
            engine.replace_source(path, body).unwrap();
        }
        engine.refresh(&Control::unbounded()).unwrap();
        import_graph(&engine, &root);
        let workspace_id = engine.workspace_id().unwrap();
        let rows = rows();
        for row in &rows {
            engine.record_learning_feedback(&row.to_string()).unwrap();
        }
        drop(engine);
        let (checkpoint, exe) = match worker {
            Worker::Fake => {
                // Fake weights the fake worker hashes, plus the pinned
                // tokenizer the core's preflight serves from.
                let checkpoint = root.join("checkpoint");
                fake_checkpoint(&checkpoint, b"fake weights");
                (checkpoint, fake_exe())
            }
            #[cfg(feature = "learning-worker")]
            Worker::Real => (checkpoint_dir(), env!("CARGO_BIN_EXE_foundry-learn")),
        };
        let profile = write_profile(
            &root,
            "learn-profile.json",
            &checkpoint,
            exe,
            // The real checkpoint's load (F16 upcast, frozen-encoder digest)
            // takes longer than the fake's; serving still caps it at 30 s.
            if exe == fake_exe() { 60 } else { 1800 },
            cpu_threads,
        );
        Self {
            dir,
            store,
            ws,
            checkpoint,
            profile,
            workspace_id,
            cpu_threads,
            threshold,
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        std::fs::canonicalize(self.dir.path()).unwrap().join(name)
    }

    fn engine(&self) -> Engine {
        Engine::open_existing(&self.store).unwrap()
    }

    fn write_policy(&self, seed: &str, selection: Selection) -> PathBuf {
        let tokenizer = tokenizer_dir();
        let model = |name: &str| digest_of(&read(&self.checkpoint.join(name)));
        let lenient = !selection.strict;
        let policy = json!({
            "v": 2,
            "recipe": learning::RECIPE,
            "seed": seed,
            "tokenizer": {
                "dir": tokenizer,
                "json_sha256": digest_of(&read(&tokenizer.join("tokenizer.json"))),
                "config_sha256": digest_of(&read(&tokenizer.join("tokenizer_config.json"))),
            },
            "model": {
                "weights_sha256": model("model.safetensors"),
                "encoder_config_sha256": model("encoder/config.json"),
                "source_dtype": "F16",
            },
            "base": null,
            "optimizer": {
                "name": "adamw",
                "learning_rate": 1e-4,
                "beta1": 0.9,
                "beta2": 0.999,
                "epsilon": 1e-8,
                "weight_decay": 0.01,
                "clip_global_norm": 1.0,
            },
            "max_steps": 2,
            "wall_seconds": 1800,
            "memory_bytes": 8u64 << 30,
            "output_bytes": 128u64 << 20,
            "cpu_threads": self.cpu_threads,
            "isolation_profile": self.profile,
            "enforcement": {
                "memory": "supervised",
                "cpu": "hard",
                "output": "hard",
                "process_count": "hard",
            },
            "selection": {
                "threshold": selection.threshold,
                "coverage_floor": if lenient { 0.0 } else { 1.0 },
                "accepted_accuracy_floor": if lenient { 0.0 } else { 1.0 },
                "max_macro_accuracy_drop": if lenient { 1.0 } else { 0.0 },
                "critical_groups": [],
            },
        });
        let path = self.path(&format!("policy-{seed}.json"));
        std::fs::write(&path, policy.to_string()).unwrap();
        path
    }

    /// Prepare a dataset under the `seed` policy and train one candidate on
    /// it: a distinct candidate identity per seed.
    fn candidate(&self, seed: &str, selection: Selection) -> PathBuf {
        let policy = self.write_policy(seed, selection);
        let dataset = self.path(&format!("dataset-{seed}"));
        let engine = self.engine();
        let manifest =
            match learning::prepare(&engine, &dataset, &policy, None, &Control::unbounded())
                .expect("the dataset prepares")
            {
                learning::PrepareOutcome::Completed(prepared) => prepared.manifest_path,
                learning::PrepareOutcome::NoNewData { .. } => panic!("expected a dataset"),
            };
        let out = self.path(&format!("candidate-{seed}"));
        let trained = train::train(
            &engine,
            &train::TrainRequest {
                input: &manifest,
                policy: &policy,
                out: &out,
                base: None,
                incumbent: None,
                development_isolation: true,
            },
            &Control::unbounded(),
        )
        .expect("training completes");
        assert_eq!(trained.outcome, "completed");
        out
    }

    fn select(&self, candidate: &Path, name: &str) -> Result<PathBuf, FoundryError> {
        let out = self.path(name);
        let engine = self.engine();
        policy::select(
            &engine,
            &policy::SelectRequest {
                candidate,
                isolation_profile: &self.profile,
                out: &out,
                lifecycle_check: false,
            },
            &Control::unbounded(),
        )
        .map(|_| out)
    }

    /// An eligible candidate and its selected config.
    fn selected(&self, seed: &str) -> (PathBuf, PathBuf) {
        let candidate = self.candidate(seed, Selection::lenient(self.threshold));
        let config = self
            .select(&candidate, &format!("policy-config-{seed}.json"))
            .expect("the candidate is selected");
        (candidate, config)
    }

    /// Re-record every evaluation row (`e*` tasks) with `checked(label)` as
    /// its checker evidence: the same examples, labels and permissions.
    fn reevidence(&self, checked: impl Fn(&str) -> String) {
        let engine = self.engine();
        for mut row in rows() {
            let label = row["correct_option_id"].as_str().unwrap().to_owned();
            if row["task_id"].as_str().unwrap().starts_with('e') {
                row["label_evidence"] = checked(&label).into();
                let (_, status) = engine.record_learning_feedback(&row.to_string()).unwrap();
                assert_ne!(status, "created", "the same example");
            }
        }
    }

    /// A hand-written enabled config pinning `candidate` exactly as `select`
    /// would write it, without `select`'s refusals.
    fn pin(&self, candidate: &Path, name: &str) -> PathBuf {
        let manifest = json_file(&candidate.join("manifest.json"));
        let config = json!({
            "v": 2,
            "enabled": true,
            "candidate_path": candidate,
            "candidate_sha256": manifest_sha(candidate),
            "model_function_sha256": manifest["model_function_sha256"],
            "report_sha256": digest_of(&read(&candidate.join("evaluation.json"))),
            "threshold": self.threshold,
            "isolation_profile": self.profile,
        });
        let path = self.path(name);
        std::fs::write(&path, config.to_string()).unwrap();
        path
    }

    /// Start a policy from `config` with the fake worker's `hooks`.
    fn start(&self, config: &Path, hooks: &[String]) -> Policy {
        Policy::start(
            Some(PolicyServing::with_worker_args(
                config.to_path_buf(),
                hooks.to_vec(),
            )),
            Some(&self.workspace_id),
        )
    }

    fn log(&self, name: &str) -> PathBuf {
        self.path(name)
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(BIN)
            .arg("--store")
            .arg(&self.store)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}

fn rows() -> Vec<serde_json::Value> {
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
    rows
}

/// One SCIP artifact for `src/a.rs`, bound to the store's current revision:
/// the compiler graph scope is current, so the policy may run.
fn import_graph(engine: &Engine, root: &Path) {
    let mut index = Index::new();
    let mut document = Document::new();
    document.relative_path = "src/a.rs".to_owned();
    document.language = "rust".to_owned();
    document.position_encoding =
        EnumOrUnknown::new(PositionEncoding::UTF8CodeUnitOffsetFromLineStart);
    document.occurrences = vec![occurrence(
        &[0, 7, 16],
        "rust-analyzer cargo toy 0.1.0 alpha_one().",
        DEF,
    )];
    index.documents = vec![document];
    let artifact = index.write_to_bytes().unwrap();
    let mut paths: Vec<&str> = SOURCES.iter().map(|(path, _)| *path).collect();
    paths.sort_unstable();
    let inputs: Vec<serde_json::Value> = paths
        .iter()
        .map(|path| json!({"path": path, "sha256": engine.source(path).unwrap().unwrap().hash}))
        .collect();
    let manifest = json!({
        "v": 1,
        "workspace_id": engine.workspace_id().unwrap(),
        "source_revision": engine.source_revision().unwrap(),
        "producer": {
            "name": "scip-test",
            "release_tag": "2026-08-31",
            "commit": "f8996691e991a4dc3c6f135e0fc04fc5561e4e9a",
            "version_output": "test-producer 1.0",
            "binary_sha256": digest_of(b"test-producer-binary"),
        },
        "invocation": "test-producer scip <snapshot> --output index.scip",
        "config": "policy",
        "artifact_sha256": digest_of(&artifact),
        "inputs": inputs,
    });
    let imports = root.join("imports");
    std::fs::create_dir_all(&imports).unwrap();
    let index_path = imports.join("index.scip");
    let manifest_path = imports.join("snapshot.json");
    std::fs::write(&index_path, &artifact).unwrap();
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();
    engine
        .import_scip(&index_path, &manifest_path, &Control::unbounded())
        .unwrap();
}

/// A fake checkpoint: the files the fake worker hashes and the pinned
/// tokenizer the serving preflight loads.
fn fake_checkpoint(dir: &Path, weights: &[u8]) {
    std::fs::create_dir_all(dir.join("encoder")).unwrap();
    std::fs::create_dir_all(dir.join("tokenizer")).unwrap();
    std::fs::write(dir.join("model.safetensors"), weights).unwrap();
    std::fs::write(dir.join("encoder/config.json"), b"{}").unwrap();
    for name in ["tokenizer.json", "tokenizer_config.json"] {
        std::fs::copy(tokenizer_dir().join(name), dir.join("tokenizer").join(name)).unwrap();
    }
}

/// A bundle layout around `exe` and its isolation profile.
fn write_profile(
    root: &Path,
    name: &str,
    checkpoint: &Path,
    exe: &str,
    load_timeout_seconds: u64,
    cpu_threads: u32,
) -> PathBuf {
    let bundle = root.join(format!("{name}.app"));
    let macos = bundle.join("Contents/MacOS");
    std::fs::create_dir_all(&macos).unwrap();
    let copy = macos.join("foundry-learn");
    std::fs::copy(exe, &copy).unwrap();
    let libtorch = root.join("libtorch-lib");
    std::fs::create_dir_all(&libtorch).unwrap();
    let path = root.join(name);
    std::fs::write(
        &path,
        json!({
            "v": 1,
            "name": "test learning worker",
            "worker": {
                "bundle": bundle,
                "executable_sha256": digest_of(&read(&copy)),
                "scratch_root": root.join("scratch"),
            },
            "checkpoint_dir": checkpoint,
            "libtorch_dir": libtorch,
            "load_timeout_seconds": load_timeout_seconds,
            "ceilings": {
                "memory_bytes": 8u64 << 30,
                "wall_seconds": 7200,
                "output_bytes": 2u64 << 30,
                "cpu_threads": cpu_threads,
            },
        })
        .to_string(),
    )
    .unwrap();
    path
}

fn hooks(list: &[&str]) -> Vec<String> {
    list.iter().map(|hook| (*hook).to_owned()).collect()
}

fn path_arg(path: &Path) -> String {
    path.display().to_string()
}

/// One context through the real request path with `policy`, under a read
/// deadline `deadline` from now; the packed v2 text.
fn context(
    engine: &Engine,
    policy: Option<&Policy>,
    query: &str,
    strategy: Strategy,
    deadline: Duration,
) -> String {
    let control = Control::with_deadline(Instant::now() + deadline);
    let combined = context_foundry::mcp::context_primary(
        &None, policy, engine, query, strategy, &control, false,
    )
    .expect("context candidates");
    response::pack_context(
        &combined.batch,
        Budget::request(2048),
        &response::stdout_bytes,
    )
    .unwrap()
    .text
}

fn header(text: &str) -> Vec<String> {
    text.lines()
        .next()
        .unwrap_or_default()
        .split(" · ")
        .map(str::to_owned)
        .collect()
}

/// The `route:` word of a header, if any.
fn route(text: &str) -> Option<String> {
    header(text)
        .into_iter()
        .find_map(|segment| segment.strip_prefix("route:").map(str::to_owned))
}

/// True when the context resolved to graph (the `graph:` segment appears
/// exactly then).
fn resolved_graph(text: &str) -> bool {
    header(text)
        .iter()
        .any(|segment| segment.starts_with("graph:"))
}

/// The predictions the fake worker received: `(request id, candidate)`.
fn predictions(log: &Path) -> Vec<(u64, String)> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut parts = line.split(' ');
            (parts.next() == Some("predict")).then(|| {
                (
                    parts.next().unwrap().parse().unwrap(),
                    parts.next().unwrap().to_owned(),
                )
            })
        })
        .collect()
}

fn wait_until(what: &str, limit: Duration, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + limit;
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting until {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn assert_reaped(pid_file: &Path) {
    let pid: libc::pid_t = std::fs::read_to_string(pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    wait_until("the worker is reaped", Duration::from_secs(8), || {
        let rc = unsafe { libc::kill(pid, 0) };
        rc == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
    });
}

fn manifest_sha(candidate: &Path) -> String {
    digest_of(&read(&candidate.join("manifest.json")))
}

/// The candidate's evaluation-report `economics` (`null` when absent).
fn economics(candidate: &Path) -> serde_json::Value {
    json_file(&candidate.join("evaluation.json"))["economics"].clone()
}

fn error_code(out: &Output) -> String {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let error: serde_json::Value = serde_json::from_str(stderr.trim())
        .unwrap_or_else(|_| panic!("no error JSON on stderr: {stderr}"));
    error["code"].as_str().unwrap().to_owned()
}

// ---------------------------------------------------------------------------
// Selection, restart and rollback
// ---------------------------------------------------------------------------

/// Two candidate identities: `select` writes a NEW config for each (pinning
/// candidate, function, report and threshold) and never overwrites; an
/// owner serves exactly the config it starts with, so rollback is a restart
/// with the prior config, or none. Nothing here changes user state.
#[test]
fn select_writes_new_configs_refuses_overwrite_and_restart_rolls_back() {
    let env = Env::new(0.6);
    let a = env.candidate("seed-a", Selection::lenient(0.6));
    let b = env.candidate("seed-b", Selection::lenient(0.6));
    assert_ne!(
        manifest_sha(&a),
        manifest_sha(&b),
        "two candidate identities"
    );
    // The fixture's checker evidence: routing a task to graph never loses
    // evidence and gains it on `graph` tasks, so both pass the gate.
    for candidate in [&a, &b] {
        let report = economics(candidate);
        assert_eq!(report["rows"], 21, "{report}");
        assert_eq!(report["lost"], 0, "{report}");
        assert!(report["gained"].as_u64().unwrap() > 0, "{report}");
    }
    let before = testkit::snapshot(&env.store);
    let cache_before = testkit::semantic_cache_rows(&env.store);

    let config_a = env.select(&a, "policy-a.json").unwrap();
    // The CLI writes the second one: exit 0 and one JSON line.
    let config_b = env.path("policy-b.json");
    let out = env.cli(&[
        "learning",
        "select",
        "--candidate",
        &path_arg(&b),
        "--isolation-profile",
        &path_arg(&env.profile),
        "--out",
        &path_arg(&config_b),
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let selected: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(selected["outcome"], "selected");
    assert_eq!(selected["candidate_sha256"], manifest_sha(&b));

    let config = json_file(&config_a);
    assert_eq!(config["v"], 2);
    assert_eq!(config["enabled"], true);
    assert_eq!(config["candidate_path"], path_arg(&a));
    assert_eq!(config["candidate_sha256"], manifest_sha(&a));
    assert_eq!(
        config["report_sha256"],
        digest_of(&read(&a.join("evaluation.json")))
    );
    assert_eq!(
        config["model_function_sha256"],
        json_file(&a.join("manifest.json"))["model_function_sha256"]
    );
    assert_eq!(config["threshold"], 0.6);
    assert_eq!(config["isolation_profile"], path_arg(&env.profile));
    // A normal selection carries no lifecycle mark, in the config or the output.
    assert_eq!(config.get("lifecycle_check"), None, "{config}");
    assert_eq!(selected.get("lifecycle_check"), None, "{selected}");

    // Never overwritten, by the library or the CLI (exit 2), and nothing
    // partial is left behind.
    let bytes_a = read(&config_a);
    let error = env.select(&b, "policy-a.json").unwrap_err();
    assert_eq!(error.code(), "output_exists", "{error}");
    let out = env.cli(&[
        "learning",
        "select",
        "--candidate",
        &path_arg(&b),
        "--isolation-profile",
        &path_arg(&env.profile),
        "--out",
        &path_arg(&config_a),
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(error_code(&out), "output_exists");
    assert_eq!(read(&config_a), bytes_a);
    for entry in std::fs::read_dir(env.dir.path()).unwrap() {
        let name = entry.unwrap().file_name();
        assert!(
            !name
                .to_string_lossy()
                .starts_with(".policy-config-partial-"),
            "a partial config survived: {name:?}"
        );
    }

    // Serve B; roll back to A by restarting with A's config; then to none.
    let log = env.log("predict.log");
    let tamper = ["--predict-tamper", "1:probs=0.9/0.1", "--request-log"];
    let engine = env.engine();
    for (config, candidate) in [(&config_b, &b), (&config_a, &a)] {
        let _ = std::fs::remove_file(&log);
        let mut args = hooks(&tamper);
        args.push(path_arg(&log));
        let served = env.start(config, &args);
        let status = served.status();
        assert_eq!(status["state"], "enabled", "{status}");
        assert_eq!(status["candidate"], manifest_sha(candidate));
        assert_eq!(status["threshold"], 0.6);
        let text = context(
            &engine,
            Some(&served),
            GRAPH_QUERY,
            Strategy::Auto,
            Duration::from_secs(5),
        );
        assert_eq!(route(&text).as_deref(), Some("policy"), "{text}");
        assert!(!resolved_graph(&text), "the policy chose search: {text}");
        assert_eq!(
            predictions(&log),
            vec![(2, manifest_sha(candidate))],
            "the request named exactly the served candidate"
        );
    }
    let none = Policy::start(None, Some(&env.workspace_id));
    assert_eq!(none.status(), json!({"state": "disabled"}));
    let text = context(
        &engine,
        Some(&none),
        GRAPH_QUERY,
        Strategy::Auto,
        Duration::from_secs(5),
    );
    assert_eq!(route(&text), None, "no policy, no route segment");
    assert!(resolved_graph(&text), "deterministic routing again");
    drop(engine);

    // Select, serve, restart and roll back preserved every user table and
    // the semantic cache.
    assert_eq!(testkit::snapshot(&env.store), before);
    assert_eq!(testkit::semantic_cache_rows(&env.store), cache_before);
}

/// An ineligible candidate is never selected, and neither is one whose
/// training consent was withdrawn after it was trained.
#[test]
fn select_refuses_an_ineligible_candidate_and_withdrawn_consent() {
    let env = Env::new(0.6);
    let strict = env.candidate(
        "seed-strict",
        Selection {
            threshold: 0.99,
            strict: true,
        },
    );
    assert_eq!(json_file(&strict.join("manifest.json"))["eligible"], false);
    let error = env.select(&strict, "never.json").unwrap_err();
    assert_eq!(error.code(), "candidate_ineligible", "{error}");
    assert!(!env.path("never.json").exists());

    let eligible = env.candidate("seed-a", Selection::lenient(0.6));
    let mut withdrawn = rows()[0].clone();
    withdrawn["allow_training"] = false.into();
    env.engine()
        .record_learning_feedback(&withdrawn.to_string())
        .unwrap();
    let error = env.select(&eligible, "never.json").unwrap_err();
    assert_eq!(error.code(), "contribution_changed", "{error}");
    assert!(!env.path("never.json").exists());
}

/// The offline economics gate (contract, 2026-10-06), after every other
/// check: a candidate whose report has no economics (one evaluation row
/// without checker evidence) is `economics_unknown`; one without a net
/// evidence gain is `candidate_no_benefit`, whether routing gains evidence on
/// some tasks and loses it on more, or changes only delivered tokens (round
/// 1's shape: changed routes, gained 0, lost 0, fewer tokens). The CLI exits
/// 2 and nothing is written. The net-gain success is
/// `select_writes_new_configs_refuses_overwrite_and_restart_rolls_back`.
#[test]
fn select_requires_a_net_evidence_gain_over_deterministic_routing() {
    let refused = |env: &Env, candidate: &Path, code: &str| {
        let error = env.select(candidate, "never.json").unwrap_err();
        assert_eq!(error.code(), code, "{error}");
        assert!(!env.path("never.json").exists(), "no config was written");
        error.to_string()
    };

    // One evaluation row without checker evidence: no economics at all.
    let env = Env::new(0.6);
    let mut plain = rows()
        .into_iter()
        .find(|row| row["task_id"] == "e0")
        .unwrap();
    plain["label_source"] = "operator".into();
    plain["label_evidence"] = "evidence for e0".into();
    env.engine()
        .record_learning_feedback(&plain.to_string())
        .unwrap();
    let unknown = env.candidate("seed-a", Selection::lenient(0.6));
    assert_eq!(economics(&unknown), serde_json::Value::Null);
    refused(&env, &unknown, "economics_unknown");

    // Only the labeled option delivers each task: routing to graph gains the
    // `graph` tasks it moves and loses the `search` ones, more of them.
    env.reevidence(|label| {
        if label == "graph" {
            evidence((false, 2048), (true, 1900))
        } else {
            evidence((true, 1500), (false, 2048))
        }
    });
    let net_loss = env.candidate("seed-b", Selection::lenient(0.6));
    let report = economics(&net_loss);
    assert!(report["gained"].as_u64().unwrap() > 0, "{report}");
    refused(&env, &net_loss, "candidate_no_benefit");

    // Both options deliver every task, the labeled one with fewer tokens:
    // routing changes routes and delivered tokens, never evidence.
    let round_one = Env::new(0.6);
    round_one.reevidence(|label| {
        if label == "graph" {
            evidence((true, 2000), (true, 1000))
        } else {
            evidence((true, 1500), (true, 1510))
        }
    });
    let tokens_only = round_one.candidate("seed-a", Selection::lenient(0.6));
    let report = economics(&tokens_only);
    let tokens = |arm: &str| report[arm]["delivered_tokens"].as_u64().unwrap();
    assert!(report["changed_routes"].as_u64().unwrap() > 0, "{report}");
    assert!(tokens("routed") < tokens("baseline"), "{report}");
    let message = refused(&round_one, &tokens_only, "candidate_no_benefit");
    assert!(message.contains("gained 0, lost 0"), "{message}");
    let never = round_one.path("never.json");
    let out = round_one.cli(&[
        "learning",
        "select",
        "--candidate",
        &path_arg(&tokens_only),
        "--isolation-profile",
        &path_arg(&round_one.profile),
        "--out",
        &path_arg(&never),
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(error_code(&out), "candidate_no_benefit");
    assert!(!never.exists());
}

/// `--lifecycle-check` (lifecycle and package verification only, 013 T004 and
/// D001) selects an otherwise valid candidate that fails the economics gate
/// and marks the config; every status of that parsed config carries the mark
/// (served, a failed start, an inspection that verifies or fails), so it is
/// never mistaken for enablement. The mark is accepted only as `true`, and a
/// config that does not parse carries none.
#[test]
fn lifecycle_check_selects_and_serves_a_no_benefit_candidate_visibly() {
    let env = Env::new(0.6);
    // Round 1's shape: both options deliver every task.
    env.reevidence(|label| {
        if label == "graph" {
            evidence((true, 2000), (true, 1000))
        } else {
            evidence((true, 1500), (true, 1510))
        }
    });
    let candidate = env.candidate("seed-a", Selection::lenient(0.6));
    let config = env.path("lifecycle.json");
    let out = env.cli(&[
        "learning",
        "select",
        "--candidate",
        &path_arg(&candidate),
        "--isolation-profile",
        &path_arg(&env.profile),
        "--out",
        &path_arg(&config),
        "--lifecycle-check",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let selected: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(selected["lifecycle_check"], true, "{selected}");
    let written = json_file(&config);
    assert_eq!(written["lifecycle_check"], true, "{written}");

    let served = env.start(&config, &[]);
    let status = served.status();
    assert_eq!(status["state"], "enabled", "{status}");
    assert_eq!(status["lifecycle_check"], true, "{status}");
    drop(served);

    // A failed start keeps the mark: no development isolation.
    let mut launch = PolicyServing::with_worker_args(config.clone(), Vec::new());
    launch.development = false;
    let status = Policy::start(Some(launch), Some(&env.workspace_id)).status();
    assert_eq!(status["reason"], "isolation_unavailable", "{status}");
    assert_eq!(status["lifecycle_check"], true, "{status}");

    // Inspection marks both outcomes: verified, and failed verification (an
    // isolation profile that is gone).
    let status = Policy::inspect(&config, Some(&env.workspace_id));
    assert_eq!(status["state"], "enabled", "{status}");
    assert_eq!(status["lifecycle_check"], true, "{status}");
    let mut gone = written.clone();
    gone["isolation_profile"] = json!(env.path("no-such-profile.json"));
    let path = env.path("gone.json");
    std::fs::write(&path, gone.to_string()).unwrap();
    let status = Policy::inspect(&path, Some(&env.workspace_id));
    assert_eq!(status["reason"], "policy_config_invalid", "{status}");
    assert_eq!(status["lifecycle_check"], true, "{status}");

    // The mark exists only as `true`; a config that does not parse has none.
    for value in [
        json!(false),
        json!("true"),
        json!(1),
        serde_json::Value::Null,
    ] {
        let mut edited = written.clone();
        edited["lifecycle_check"] = value;
        let path = env.path("edited.json");
        std::fs::write(&path, edited.to_string()).unwrap();
        let status = env.start(&path, &[]).status();
        assert_eq!(status["state"], "unavailable", "{edited}: {status}");
        assert_eq!(status["reason"], "policy_config_invalid", "{status}");
        assert_eq!(status.get("lifecycle_check"), None, "{status}");
    }
}

/// Changing ONLY an evaluated example's rights assertion (same task, state,
/// options, label and consent, so the same example ID) refuses selection:
/// the report binds each evaluated example's permission digest.
#[test]
fn select_refuses_a_changed_rights_assertion_on_an_evaluated_example() {
    let env = Env::new(0.6);
    let candidate = env.candidate("seed-a", Selection::lenient(0.6));
    let evaluated: std::collections::BTreeSet<String> =
        json_file(&candidate.join("evaluation.json"))["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|case| case["example_id"].as_str().unwrap().to_owned())
            .collect();
    let engine = env.engine();
    let row = rows()
        .into_iter()
        .find(|row| {
            let parsed = learning::FeedbackRowV4::parse(&row.to_string()).unwrap();
            evaluated.contains(&parsed.example_id())
        })
        .expect("an evaluated row");
    let mut changed = row.clone();
    changed["rights_ref"] = "rights-rechecked-elsewhere".into();
    let (example_id, status) = engine
        .record_learning_feedback(&changed.to_string())
        .unwrap();
    assert_eq!(status, "replaced", "the same example, a new permission");
    assert!(evaluated.contains(&example_id));
    drop(engine);
    let error = env.select(&candidate, "never.json").unwrap_err();
    assert_eq!(error.code(), "contribution_changed", "{error}");
    assert!(error.to_string().contains("rights"), "{error}");
    assert!(!env.path("never.json").exists(), "no config was written");
}

// ---------------------------------------------------------------------------
// Routing
// ---------------------------------------------------------------------------

/// Acceptance compares the UNROUNDED maximum of the supplied vector with the
/// threshold: exactly 0.6 accepts; 0.59999 abstains; a reported confidence
/// over the threshold cannot lift a maximum under it. An accepted choice
/// overrides deterministic routing either way; abstention falls back to it.
#[test]
fn the_threshold_reads_the_unrounded_maximum_and_abstention_is_deterministic() {
    let env = Env::new(0.6);
    let (_, config) = env.selected("seed-a");
    let served = env.start(
        &config,
        &hooks(&[
            "--predict-tamper",
            "1:probs=0.6/0.4,2:probs=0.59999/0.40001,3:probs=0.5999995/0.4000005/0.6000004,\
             4:probs=0.1/0.9,5:probs=0.9/0.1",
        ]),
    );
    let engine = env.engine();
    let run = |query: &str| {
        context(
            &engine,
            Some(&served),
            query,
            Strategy::Auto,
            Duration::from_secs(5),
        )
    };
    let exact = run(SEARCH_QUERY);
    assert_eq!(route(&exact).as_deref(), Some("policy"), "{exact}");
    assert!(!resolved_graph(&exact));
    let below = run(GRAPH_QUERY);
    assert_eq!(
        route(&below).as_deref(),
        Some("fallback:policy_abstained"),
        "{below}"
    );
    assert!(
        resolved_graph(&below),
        "abstention routes deterministically"
    );
    let lifted = run(GRAPH_QUERY);
    assert_eq!(
        route(&lifted).as_deref(),
        Some("fallback:policy_abstained"),
        "{lifted}"
    );
    let to_graph = run(SEARCH_QUERY);
    assert_eq!(route(&to_graph).as_deref(), Some("policy"), "{to_graph}");
    assert!(
        resolved_graph(&to_graph),
        "the policy overrode search: {to_graph}"
    );
    let to_search = run(GRAPH_QUERY);
    assert_eq!(route(&to_search).as_deref(), Some("policy"), "{to_search}");
    assert!(
        !resolved_graph(&to_search),
        "the policy overrode graph: {to_search}"
    );
    assert_eq!(served.status()["consecutive_timeouts"], 0);
}

/// Explicit strategies and a stale graph never reach the model; explicit
/// output is byte-identical to an owner without a policy.
#[test]
fn explicit_strategy_and_a_stale_graph_bypass_the_policy() {
    let env = Env::new(0.6);
    let (_, config) = env.selected("seed-a");
    let log = env.log("predict.log");
    let served = env.start(&config, &hooks(&["--request-log", &path_arg(&log)]));
    let mut engine = env.engine();
    for strategy in [Strategy::Search, Strategy::Graph] {
        for query in [SEARCH_QUERY, GRAPH_QUERY] {
            let with = context(
                &engine,
                Some(&served),
                query,
                strategy,
                Duration::from_secs(5),
            );
            let without = context(&engine, None, query, strategy, Duration::from_secs(5));
            assert_eq!(with, without, "explicit {strategy} is untouched");
            assert_eq!(route(&with), None);
        }
    }
    assert!(
        predictions(&log).is_empty(),
        "no model call for an explicit strategy"
    );
    // An edit moves the revision past the compiler snapshot: no current
    // graph scope, no model call, deterministic routing by name.
    engine
        .replace_source("src/b.rs", "pub fn alpha_two() { let _ = 2; }\n")
        .unwrap();
    engine.refresh(&Control::unbounded()).unwrap();
    for (query, graph) in [(SEARCH_QUERY, false), (GRAPH_QUERY, true)] {
        let text = context(
            &engine,
            Some(&served),
            query,
            Strategy::Auto,
            Duration::from_secs(5),
        );
        assert_eq!(
            route(&text).as_deref(),
            Some("fallback:graph_stale"),
            "{text}"
        );
        assert_eq!(resolved_graph(&text), graph);
    }
    assert!(
        predictions(&log).is_empty(),
        "no model call without a current graph"
    );
}

/// A semantic profile that could not start (no Nemotron) leaves the policy
/// serving; both words are in the header.
#[test]
fn missing_nemotron_does_not_disable_the_policy() {
    let env = Env::new(0.6);
    let (_, config) = env.selected("seed-a");
    let served = env.start(&config, &hooks(&["--predict-tamper", "1:probs=0.1/0.9"]));
    let engine = env.engine();
    let slot: context_foundry::mcp::SemanticSlot = Some(Err("fallback:profile_invalid".into()));
    let control = Control::with_deadline(Instant::now() + Duration::from_secs(5));
    let combined = context_foundry::mcp::context_primary(
        &slot,
        Some(&served),
        &engine,
        SEARCH_QUERY,
        Strategy::Auto,
        &control,
        false,
    )
    .unwrap();
    let text = response::pack_context(
        &combined.batch,
        Budget::request(2048),
        &response::stdout_bytes,
    )
    .unwrap()
    .text;
    let segments = header(&text);
    assert!(
        segments.contains(&"semantic:fallback:profile_invalid".to_owned()),
        "{text}"
    );
    // The query anchors `alpha_one`, so `anchored` follows `route:`
    // (context-v2 § Anchored context, 001 T007).
    assert_eq!(
        segments[segments.len().saturating_sub(2)..],
        ["route:policy", "anchored"],
        "{text}"
    );
    assert!(resolved_graph(&text));
}

/// A state over the 1024-token total is a named fallback before dispatch.
#[test]
fn an_oversized_state_falls_back_before_any_dispatch() {
    let env = Env::new(0.6);
    let (_, config) = env.selected("seed-a");
    let log = env.log("predict.log");
    let served = env.start(&config, &hooks(&["--request-log", &path_arg(&log)]));
    let engine = env.engine();
    let query: String = (0..800)
        .map(|i| format!("q{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(query.len() <= 4096);
    let text = context(
        &engine,
        Some(&served),
        &query,
        Strategy::Auto,
        Duration::from_secs(5),
    );
    assert_eq!(
        route(&text).as_deref(),
        Some("fallback:policy_input_oversize"),
        "{text}"
    );
    assert!(predictions(&log).is_empty());
    assert_eq!(
        served.status()["state"],
        "enabled",
        "an oversized input is not terminal"
    );
}

// ---------------------------------------------------------------------------
// The slot: timeouts, busy, terminal failures and EOF
// ---------------------------------------------------------------------------

/// One timeout falls back for that request only; while its work runs later
/// requests are busy (not counted, not resetting); its late reply frees the
/// slot and is never delivered; only a valid in-ceiling reply resets the
/// count; three consecutive timeouts terminate the worker and leave the
/// policy unavailable until restart.
#[test]
fn timeouts_fall_back_alone_keep_the_slot_and_three_terminate() {
    let env = Env::new(0.6);
    let (_, config) = env.selected("seed-a");
    let log = env.log("predict.log");
    let pid = env.path("worker.pid");
    let served = env.start(
        &config,
        &hooks(&[
            "--predict-delays-ms",
            "2600,0,2600,2600,2600",
            // The late reply would route to search; the next valid reply
            // routes to graph.
            "--predict-tamper",
            "1:probs=0.9/0.1,2:probs=0.1/0.9",
            "--request-log",
            &path_arg(&log),
            "--pid-file",
            &path_arg(&pid),
        ]),
    );
    let engine = env.engine();
    // The owner's 5 s read deadline: each prediction's ceiling is 2 s, and a
    // timed-out prediction leaves the request time to finish deterministically.
    let run = || {
        context(
            &engine,
            Some(&served),
            SEARCH_QUERY,
            Strategy::Auto,
            Duration::from_secs(5),
        )
    };
    let state = |key: &str| served.status()[key].clone();
    let idle = || {
        wait_until(
            "the late reply frees the slot",
            Duration::from_secs(5),
            || state("busy") == json!(false),
        )
    };

    let first = run();
    assert_eq!(
        route(&first).as_deref(),
        Some("fallback:policy_timeout"),
        "{first}"
    );
    assert!(!resolved_graph(&first), "that request alone fell back");
    assert_eq!(state("consecutive_timeouts"), 1);
    assert_eq!(
        state("busy"),
        true,
        "the timed-out work still holds the slot"
    );
    let busy = run();
    assert_eq!(
        route(&busy).as_deref(),
        Some("fallback:policy_busy"),
        "{busy}"
    );
    assert_eq!(predictions(&log).len(), 1, "busy dispatches nothing");
    assert_eq!(state("consecutive_timeouts"), 1, "busy does not count");
    idle();
    let valid = run();
    assert_eq!(route(&valid).as_deref(), Some("policy"), "{valid}");
    assert!(
        resolved_graph(&valid),
        "the late search reply was never delivered; this request got its own"
    );
    assert_eq!(state("consecutive_timeouts"), 0, "a valid reply resets");

    let second = run();
    assert_eq!(route(&second).as_deref(), Some("fallback:policy_timeout"));
    assert_eq!(state("consecutive_timeouts"), 1);
    let busy = run();
    assert_eq!(route(&busy).as_deref(), Some("fallback:policy_busy"));
    assert_eq!(
        state("consecutive_timeouts"),
        1,
        "busy does not reset either"
    );
    idle();
    let third = run();
    assert_eq!(route(&third).as_deref(), Some("fallback:policy_timeout"));
    assert_eq!(state("consecutive_timeouts"), 2);
    idle();
    let fourth = run();
    assert_eq!(route(&fourth).as_deref(), Some("fallback:policy_timeout"));
    let status = served.status();
    assert_eq!(status["state"], "unavailable", "{status}");
    assert_eq!(status["reason"], "prediction_timeouts");
    assert_eq!(status["consecutive_timeouts"], 3);
    assert_reaped(&pid);
    let after = run();
    assert_eq!(
        route(&after).as_deref(),
        Some("fallback:policy_unavailable")
    );
    assert_eq!(
        predictions(&log).len(),
        5,
        "nothing is dispatched after termination"
    );
    let ids: Vec<u64> = predictions(&log).iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, [2, 3, 4, 5, 6], "monotonic request IDs after the load");
}

/// A reply PUBLISHED after its ceiling but before the waiter looks is a
/// timeout: the waiter is held until the publication, which happens at
/// 2.2 s, past the 2 s ceiling. Each late reply (which would route to
/// search) is discarded, the count climbs once per request, and the third
/// terminates the worker; no route is granted.
#[test]
fn a_reply_published_after_its_ceiling_is_a_timeout_whoever_locks_first() {
    let env = Env::new(0.6);
    let (_, config) = env.selected("seed-a");
    let pid = env.path("worker.pid");
    // The ceiling is 2 s under a 5 s read deadline; each reply is read at
    // least 2.2 s after its dispatch, so strictly after its ceiling.
    let served = env.start(
        &config,
        &hooks(&[
            "--predict-delays-ms",
            "2200,2200,2200",
            "--predict-tamper",
            "1:probs=0.9/0.1,2:probs=0.9/0.1,3:probs=0.9/0.1",
            "--pid-file",
            &path_arg(&pid),
        ]),
    );
    let engine = env.engine();
    context_foundry::learning::serve::set_test_wait(
        context_foundry::learning::serve::TestWait::UntilPublished,
    );
    for n in 1..=3u64 {
        let text = context(
            &engine,
            Some(&served),
            GRAPH_QUERY,
            Strategy::Auto,
            Duration::from_secs(5),
        );
        assert_eq!(
            route(&text).as_deref(),
            Some("fallback:policy_timeout"),
            "late reply {n} grants nothing: {text}"
        );
        assert!(
            resolved_graph(&text),
            "deterministic routing, not the late search reply"
        );
        let status = served.status();
        if n < 3 {
            assert_eq!(status["consecutive_timeouts"], n, "{status}");
            assert_eq!(
                status["busy"], false,
                "the late reply ended the work: {status}"
            );
        }
    }
    context_foundry::learning::serve::set_test_wait(
        context_foundry::learning::serve::TestWait::Normal,
    );
    let status = served.status();
    assert_eq!(status["state"], "unavailable", "{status}");
    assert_eq!(status["reason"], "prediction_timeouts");
    assert_eq!(status["consecutive_timeouts"], 3);
    assert_reaped(&pid);
    let after = context(
        &engine,
        Some(&served),
        GRAPH_QUERY,
        Strategy::Auto,
        Duration::from_secs(5),
    );
    assert_eq!(
        route(&after).as_deref(),
        Some("fallback:policy_unavailable")
    );
}

/// Ordering (a): the reader has READ the reply long before the ceiling but
/// is paused before publishing it; the waiter reaches the ceiling and wins
/// the arbitration. That is one timeout; the later publication is discarded
/// (no route, no reset) and frees the slot.
#[test]
fn a_reply_read_but_not_published_by_the_ceiling_is_one_timeout() {
    use context_foundry::learning::serve;
    let env = Env::new(0.6);
    let (_, config) = env.selected("seed-a");
    serve::set_test_hold_publication(true);
    let served = env.start(
        &config,
        &hooks(&["--predict-tamper", "1:probs=0.9/0.1,2:probs=0.9/0.1"]),
    );
    serve::set_test_hold_publication(false);
    let engine = env.engine();
    for n in 1..=2u64 {
        let text = context(
            &engine,
            Some(&served),
            GRAPH_QUERY,
            Strategy::Auto,
            Duration::from_secs(5),
        );
        assert_eq!(
            route(&text).as_deref(),
            Some("fallback:policy_timeout"),
            "{text}"
        );
        assert!(
            resolved_graph(&text),
            "the unpublished search reply granted nothing"
        );
        wait_until(
            "the discarded publication frees the slot",
            Duration::from_secs(5),
            || served.status()["busy"] == json!(false),
        );
        let status = served.status();
        assert_eq!(status["consecutive_timeouts"], n, "counted once: {status}");
        assert_eq!(status["state"], "enabled", "{status}");
    }
}

/// Ordering (b): the reader PUBLISHES before the ceiling while the waiter
/// is held past it before it looks. That is an on-time success: the route is
/// granted and the count, raised by an earlier timeout, resets.
#[test]
fn a_reply_published_before_the_ceiling_succeeds_when_the_waiter_looks_late() {
    use context_foundry::learning::serve;
    let env = Env::new(0.6);
    let (_, config) = env.selected("seed-a");
    let served = env.start(
        &config,
        &hooks(&[
            "--predict-delays-ms",
            "2600,900",
            "--predict-tamper",
            "2:probs=0.9/0.1",
        ]),
    );
    let engine = env.engine();
    let run = || {
        context(
            &engine,
            Some(&served),
            GRAPH_QUERY,
            Strategy::Auto,
            Duration::from_secs(5),
        )
    };
    let first = run();
    assert_eq!(route(&first).as_deref(), Some("fallback:policy_timeout"));
    assert_eq!(served.status()["consecutive_timeouts"], 1);
    wait_until(
        "the late reply frees the slot",
        Duration::from_secs(5),
        || served.status()["busy"] == json!(false),
    );
    serve::set_test_wait(serve::TestWait::PastCeiling);
    let second = run();
    serve::set_test_wait(serve::TestWait::Normal);
    assert_eq!(route(&second).as_deref(), Some("policy"), "{second}");
    assert!(
        !resolved_graph(&second),
        "the policy's search choice was granted"
    );
    assert_eq!(
        served.status()["consecutive_timeouts"],
        0,
        "an on-time reply resets"
    );
}

/// A short read deadline: the prediction waits half the remaining time, so
/// a 3 s prediction under an 800 ms deadline still leaves the deterministic
/// baseline its time, through the real `context_primary` path, with no
/// deadline error.
#[test]
fn a_short_read_deadline_still_delivers_the_baseline() {
    let env = Env::new(0.6);
    let (_, config) = env.selected("seed-a");
    let served = env.start(
        &config,
        &hooks(&[
            "--predict-delays-ms",
            "3000",
            "--predict-tamper",
            "1:probs=0.9/0.1",
        ]),
    );
    let engine = env.engine();
    let text = context(
        &engine,
        Some(&served),
        GRAPH_QUERY,
        Strategy::Auto,
        Duration::from_millis(800),
    );
    let word = route(&text).expect("a route word");
    assert!(
        word == "fallback:policy_timeout" || word == "fallback:policy_insufficient_time",
        "{text}"
    );
    assert!(
        resolved_graph(&text),
        "the deterministic baseline was delivered: {text}"
    );
}

/// Every malformed, legacy-field, inconsistent or wrong-identity reply is
/// terminal: it never grants a route, the worker is terminated, and the
/// policy stays unavailable (nothing more is dispatched).
#[test]
fn malformed_or_wrong_identity_replies_are_terminal_and_grant_nothing() {
    let env = Env::new(0.6);
    let (_, config) = env.selected("seed-a");
    let engine = env.engine();
    for (kind, reason) in [
        ("legacy_confidence", "reply_invalid"),
        ("extra_field", "reply_invalid"),
        ("missing_graph", "reply_invalid"),
        ("extra_option", "reply_invalid"),
        ("null_choice", "reply_invalid"),
        ("duplicate_key", "reply_invalid"),
        ("sum", "reply_invalid"),
        ("range", "reply_invalid"),
        ("minority_choice", "reply_invalid"),
        ("tie_search", "reply_invalid"),
        ("confidence", "reply_invalid"),
        ("wrong_id", "reply_invalid"),
        ("wrong_candidate", "reply_identity_mismatch"),
        ("wrong_model", "reply_identity_mismatch"),
        ("wrong_input", "reply_identity_mismatch"),
    ] {
        let log = env.log(&format!("predict-{kind}.log"));
        let pid = env.path(&format!("worker-{kind}.pid"));
        let served = env.start(
            &config,
            &hooks(&[
                "--predict-tamper",
                &format!("1:{kind}"),
                "--request-log",
                &path_arg(&log),
                "--pid-file",
                &path_arg(&pid),
            ]),
        );
        for query in [GRAPH_QUERY, GRAPH_QUERY] {
            let text = context(
                &engine,
                Some(&served),
                query,
                Strategy::Auto,
                Duration::from_secs(5),
            );
            assert_eq!(
                route(&text).as_deref(),
                Some("fallback:policy_unavailable"),
                "{kind}: {text}"
            );
            assert!(resolved_graph(&text), "{kind}: deterministic routing");
        }
        let status = served.status();
        assert_eq!(status["state"], "unavailable", "{kind}: {status}");
        assert_eq!(status["reason"], reason, "{kind}: {status}");
        assert_eq!(
            predictions(&log).len(),
            1,
            "{kind}: nothing after termination"
        );
        assert_reaped(&pid);
    }
    // A duplicated reply: the first answers its request; the duplicate is
    // unsolicited and terminal.
    let served = env.start(&config, &hooks(&["--predict-tamper", "1:twice"]));
    let _ = context(
        &engine,
        Some(&served),
        SEARCH_QUERY,
        Strategy::Auto,
        Duration::from_secs(5),
    );
    wait_until("the duplicate is terminal", Duration::from_secs(5), || {
        served.status()["state"] == "unavailable"
    });
    assert_eq!(served.status()["reason"], "reply_invalid");
}

/// A worker that dies mid-prediction is terminal; owner EOF ends a healthy
/// worker (no restart loop either way).
#[test]
fn a_dead_worker_is_terminal_and_owner_eof_ends_the_worker() {
    let env = Env::new(0.6);
    let (_, config) = env.selected("seed-a");
    let engine = env.engine();
    let pid = env.path("dying.pid");
    let served = env.start(
        &config,
        &hooks(&["--predict-die-at", "1", "--pid-file", &path_arg(&pid)]),
    );
    let text = context(
        &engine,
        Some(&served),
        SEARCH_QUERY,
        Strategy::Auto,
        Duration::from_secs(5),
    );
    assert_eq!(
        route(&text).as_deref(),
        Some("fallback:policy_unavailable"),
        "{text}"
    );
    let status = served.status();
    assert_eq!(status["state"], "unavailable");
    assert_eq!(status["reason"], "worker_failed", "{status}");
    assert_reaped(&pid);

    let pid = env.path("healthy.pid");
    let served = env.start(&config, &hooks(&["--pid-file", &path_arg(&pid)]));
    assert_eq!(served.status()["state"], "enabled");
    drop(served);
    // Shutdown reaps before it returns.
    let worker: libc::pid_t = std::fs::read_to_string(&pid)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let rc = unsafe { libc::kill(worker, 0) };
    assert!(
        rc == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH),
        "owner EOF ended and reaped the worker"
    );
}

/// An invalid config, a wrong tokenizer or weights identity, a load past the
/// ceiling and missing development isolation all leave deterministic
/// routing with the reason named; a disabled config is no policy at all.
#[test]
fn invalid_configs_and_failed_starts_keep_deterministic_routing() {
    let env = Env::new(0.6);
    let (candidate, config) = env.selected("seed-a");
    let engine = env.engine();
    let base = json_file(&config);
    let write = |name: &str, value: &serde_json::Value| {
        let path = env.path(name);
        std::fs::write(&path, value.to_string()).unwrap();
        path
    };
    let root = env.path("");
    let alternate =
        |name: &str, weights: &[u8], tokenizer_config: Option<&[u8]>, load_timeout: u64| {
            let checkpoint = env.path(&format!("checkpoint-{name}"));
            fake_checkpoint(&checkpoint, weights);
            if let Some(bytes) = tokenizer_config {
                std::fs::write(checkpoint.join("tokenizer/tokenizer_config.json"), bytes).unwrap();
            }
            let profile = write_profile(
                &root,
                &format!("profile-{name}.json"),
                &checkpoint,
                fake_exe(),
                load_timeout,
                2,
            );
            let mut value = base.clone();
            value["isolation_profile"] = json!(profile);
            write(&format!("config-{name}.json"), &value)
        };
    let mut cases: Vec<(PathBuf, &str, Vec<String>, bool)> = Vec::new();
    std::fs::write(env.path("garbage.json"), b"{not json").unwrap();
    cases.push((
        env.path("garbage.json"),
        "policy_config_invalid",
        Vec::new(),
        true,
    ));
    let mut edited = base.clone();
    edited["threshold"] = json!(0.81);
    cases.push((
        write("threshold.json", &edited),
        "policy_config_invalid",
        Vec::new(),
        true,
    ));
    let mut legacy = base.clone();
    legacy["confidence"] = json!(0.9);
    cases.push((
        write("legacy.json", &legacy),
        "policy_config_invalid",
        Vec::new(),
        true,
    ));
    let copy = env.path("tampered-candidate");
    std::fs::create_dir(&copy).unwrap();
    for entry in std::fs::read_dir(&candidate).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), copy.join(entry.file_name())).unwrap();
    }
    let mut report = read(&copy.join("evaluation.json"));
    report.push(b'\n');
    std::fs::write(copy.join("evaluation.json"), report).unwrap();
    let mut tampered = base.clone();
    tampered["candidate_path"] = json!(copy);
    cases.push((
        write("tampered.json", &tampered),
        "policy_config_invalid",
        Vec::new(),
        true,
    ));
    cases.push((
        alternate("tokenizer", b"fake weights", Some(b"{}"), 60),
        "tokenizer_mismatch",
        Vec::new(),
        true,
    ));
    cases.push((
        alternate("weights", b"other weights", None, 60),
        "checkpoint_invalid",
        Vec::new(),
        true,
    ));
    cases.push((
        alternate("slow", b"fake weights", None, 1),
        "worker_timeout",
        hooks(&["--load-ms", "3000"]),
        true,
    ));
    cases.push((config.clone(), "isolation_unavailable", Vec::new(), false));
    for (path, reason, worker_args, development) in cases {
        let mut launch = PolicyServing::with_worker_args(path.clone(), worker_args);
        launch.development = development;
        let served = Policy::start(Some(launch), Some(&env.workspace_id));
        let status = served.status();
        assert_eq!(
            status["state"],
            "unavailable",
            "{}: {status}",
            path.display()
        );
        assert_eq!(status["reason"], reason, "{}: {status}", path.display());
        let text = context(
            &engine,
            Some(&served),
            SEARCH_QUERY,
            Strategy::Auto,
            Duration::from_secs(5),
        );
        assert_eq!(
            route(&text).as_deref(),
            Some("fallback:policy_unavailable"),
            "{text}"
        );
        assert!(!resolved_graph(&text));
    }
    // Nothing a failed start launched survives it.
    let scratch = env.path("scratch");
    if scratch.exists() {
        assert_eq!(std::fs::read_dir(&scratch).unwrap().count(), 0);
    }
    // A disabled config is no policy: no worker, no segment, baseline bytes.
    let disabled = write("disabled.json", &json!({"v": 2, "enabled": false}));
    let off = env.start(&disabled, &[]);
    assert!(!off.routes());
    assert_eq!(off.status(), json!({"state": "disabled"}));
    assert_eq!(
        context(
            &engine,
            Some(&off),
            SEARCH_QUERY,
            Strategy::Auto,
            Duration::from_secs(5)
        ),
        context(
            &engine,
            None,
            SEARCH_QUERY,
            Strategy::Auto,
            Duration::from_secs(5)
        ),
    );
}

/// Owner startup re-checks the economics gate as it re-checks eligibility: a
/// hand-written config naming a candidate without a net evidence gain, or one
/// whose report has no economics, is `policy_config_invalid` naming the cause
/// (and the counts); the policy is unavailable and routing stays
/// deterministic.
#[test]
fn startup_refuses_a_config_naming_a_candidate_without_a_net_evidence_gain() {
    let env = Env::new(0.6);
    // Both options deliver every task: routing gains no evidence.
    env.reevidence(|label| {
        if label == "graph" {
            evidence((true, 2000), (true, 1000))
        } else {
            evidence((true, 1500), (true, 1510))
        }
    });
    let no_benefit = env.candidate("seed-a", Selection::lenient(0.6));
    // One evaluation row without checker evidence: no economics.
    let mut plain = rows()
        .into_iter()
        .find(|row| row["task_id"] == "e0")
        .unwrap();
    plain["label_source"] = "operator".into();
    plain["label_evidence"] = "evidence for e0".into();
    env.engine()
        .record_learning_feedback(&plain.to_string())
        .unwrap();
    let unknown = env.candidate("seed-b", Selection::lenient(0.6));
    let engine = env.engine();
    for (candidate, cause, named) in [
        (&no_benefit, "candidate_no_benefit", "gained 0, lost 0"),
        (&unknown, "economics_unknown", "no economics"),
    ] {
        let served = env.start(&env.pin(candidate, &format!("{cause}.json")), &[]);
        let status = served.status();
        assert_eq!(status["state"], "unavailable", "{status}");
        assert_eq!(status["reason"], "policy_config_invalid", "{status}");
        let detail = status["detail"].as_str().unwrap();
        assert!(detail.contains(cause), "{status}");
        assert!(detail.contains(named), "{status}");
        let text = context(
            &engine,
            Some(&served),
            SEARCH_QUERY,
            Strategy::Auto,
            Duration::from_secs(5),
        );
        assert_eq!(
            route(&text).as_deref(),
            Some("fallback:policy_unavailable"),
            "{text}"
        );
    }
}

/// The worker cross-checks every request against the candidate it loaded:
/// a request naming another candidate gets no reply, and the worker ends.
#[test]
fn the_worker_refuses_a_request_for_another_candidate() {
    use context_foundry::decision_model::{self, SpecialIds};
    use context_foundry::learning::ipc::{
        self, Identity, LEARN_PROTOCOL, LearnHeader, Message, PredictReply, PredictRequest,
    };
    use context_foundry::neural::protocol::{FrameError, read_frame_as, write_frame_as};

    let env = Env::new(0.6);
    let a = env.candidate("seed-a", Selection::lenient(0.6));
    let b = env.candidate("seed-b", Selection::lenient(0.6));
    let manifest = json_file(&a.join("manifest.json"));
    let run = env.path("raw-run");
    std::fs::create_dir(&run).unwrap();
    let elsewhere = env.path("raw-elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    std::fs::copy(
        a.join("head.safetensors"),
        run.join("serve-head.safetensors"),
    )
    .unwrap();
    // This process is the worker's owner: its PID and a liveness pipe whose
    // read end the worker inherits (the write end stays here).
    let (liveness, _keep) = std::io::pipe().unwrap();
    let liveness_fd = std::os::fd::AsRawFd::as_raw_fd(&liveness);
    let mut command = Command::new(fake_exe());
    command
        .arg("--owner-pid")
        .arg(std::process::id().to_string())
        .arg("--liveness-fd")
        .arg(liveness_fd.to_string())
        .arg("--checkpoint-dir")
        .arg(&env.checkpoint)
        .arg("--run-dir")
        .arg(&run)
        // Start somewhere else, as App Sandbox does (it moves a sandboxed
        // process's working directory into its container): the worker must
        // re-enter the named run directory to find the serve head.
        .current_dir(&elsewhere)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // SAFETY: fcntl(2) only, between fork and exec.
    unsafe {
        std::os::unix::process::CommandExt::pre_exec(&mut command, move || {
            if libc::fcntl(liveness_fd, libc::F_SETFD, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut worker = command.spawn().unwrap();
    let mut stdout = worker.stdout.take().unwrap();
    let mut stdin = worker.stdin.take().unwrap();
    let function = manifest["model_function_sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    write_frame_as(
        &mut stdin,
        &LearnHeader {
            protocol: LEARN_PROTOCOL,
            request_id: 1,
            identity: Identity {
                model_function_sha256: function.clone(),
                head_sha256: Some(digest_of(&read(&a.join("head.safetensors")))),
                steps: 0,
            },
            message: Message::Serve {
                checkpoint: serde_json::from_value(manifest["encoder"]["checkpoint"].clone())
                    .unwrap(),
                candidate_sha256: manifest_sha(&a),
                temperature: manifest["temperature"].as_f64().unwrap(),
                threads: 1,
            },
        },
        &[],
    )
    .unwrap();
    let (loaded, _) = read_frame_as::<LearnHeader, _>(&mut stdout).unwrap();
    assert_eq!(loaded.message.kind(), "loaded");
    let renderer = decision_model::Renderer::load(
        &read(&tokenizer_dir().join("tokenizer.json")),
        SpecialIds::PINNED,
    )
    .unwrap();
    let state = format!("{SEARCH_QUERY}\ngraph: complete");
    let rendered = renderer.render(&state, ["search", "graph"]).unwrap();
    let payload = ipc::encode_predict_payload(
        &rendered.ids,
        [rendered.markers[0] as u32, rendered.markers[1] as u32],
    );
    let request = |id: u64, candidate: String| PredictRequest {
        v: 2,
        request_id: id,
        candidate_sha256: candidate,
        model_function_sha256: function.clone(),
        family: "retrieval-route-v1".into(),
        state: state.clone(),
        option_ids: ["search".into(), "graph".into()],
    };
    // The loaded candidate: a reply echoing it.
    write_frame_as(&mut stdin, &request(2, manifest_sha(&a)), &payload).unwrap();
    let (reply, _) = read_frame_as::<PredictReply, _>(&mut stdout).unwrap();
    assert_eq!(reply.candidate_sha256, manifest_sha(&a));
    assert_eq!(
        reply.input_sha256,
        decision_model::input_sha256(&state, ["search", "graph"])
    );
    // Another candidate: no reply, the worker ends.
    write_frame_as(&mut stdin, &request(3, manifest_sha(&b)), &payload).unwrap();
    assert_eq!(
        read_frame_as::<PredictReply, _>(&mut stdout).unwrap_err(),
        FrameError::Eof
    );
    // The stream ended without a reply: the worker refused and exited.
    let status = worker.wait().unwrap();
    assert!(
        !status.success(),
        "the refusing worker exits nonzero: {status}"
    );
}

// ---------------------------------------------------------------------------
// CLI and MCP surfaces
// ---------------------------------------------------------------------------

/// No policy (or a disabled one) leaves CLI context and search output
/// byte-identical; an enabled policy adds only the `route:` segment to
/// `auto` contexts; `status` reports the policy; `--laya-port` is refused
/// with migration guidance.
#[test]
fn the_cli_keeps_output_identical_without_a_policy_and_reports_it() {
    let env = Env::new(0.6);
    let (candidate, config) = env.selected("seed-a");
    let disabled = env.path("disabled.json");
    std::fs::write(&disabled, br#"{"v":2,"enabled":false}"#).unwrap();
    let stdout = |args: &[&str]| {
        let out = env.cli(args);
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    let plain = stdout(&["context", SEARCH_QUERY]);
    assert_eq!(route(&plain), None);
    assert_eq!(
        stdout(&[
            "context",
            SEARCH_QUERY,
            "--policy-config",
            &path_arg(&disabled)
        ]),
        plain
    );
    assert_eq!(
        route(&stdout(&["search", SEARCH_QUERY])),
        None,
        "search never routes"
    );
    let explicit = stdout(&["context", GRAPH_QUERY, "--strategy", "graph"]);
    assert_eq!(
        stdout(&[
            "context",
            GRAPH_QUERY,
            "--strategy",
            "graph",
            "--policy-config",
            &path_arg(&config),
            "--development-isolation",
        ]),
        explicit,
        "an explicit strategy bypasses the policy byte for byte"
    );
    let routed = stdout(&[
        "context",
        SEARCH_QUERY,
        "--policy-config",
        &path_arg(&config),
        "--development-isolation",
    ]);
    let word = route(&routed).expect("an enabled policy names its route");
    assert!(
        word == "policy" || word == "fallback:policy_abstained",
        "{routed}"
    );
    // Without development isolation the CLI keeps deterministic routing.
    let closed = stdout(&[
        "context",
        SEARCH_QUERY,
        "--policy-config",
        &path_arg(&config),
    ]);
    assert_eq!(
        route(&closed).as_deref(),
        Some("fallback:policy_unavailable")
    );

    let status: serde_json::Value = serde_json::from_str(&stdout(&["status"])).unwrap();
    assert_eq!(status["policy"], json!({"state": "disabled"}));
    assert_eq!(status["schema"], 6, "the store fields are unchanged");
    let status: serde_json::Value =
        serde_json::from_str(&stdout(&["status", "--policy-config", &path_arg(&config)])).unwrap();
    assert_eq!(status["policy"]["state"], "enabled");
    assert_eq!(status["policy"]["candidate"], manifest_sha(&candidate));
    std::fs::write(env.path("garbage.json"), b"[]").unwrap();
    let status: serde_json::Value = serde_json::from_str(&stdout(&[
        "status",
        "--policy-config",
        &path_arg(&env.path("garbage.json")),
    ]))
    .unwrap();
    assert_eq!(status["policy"]["state"], "unavailable");
    assert_eq!(status["policy"]["reason"], "policy_config_invalid");

    let out = env.cli(&["context", SEARCH_QUERY, "--laya-port", "8765"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(error_code(&out), "unsupported_mode");
    let message = String::from_utf8_lossy(&out.stderr);
    assert!(
        message.contains("learning select") && message.contains("--policy-config"),
        "{message}"
    );
}

/// A stopped owner releases the store before the next one starts.
fn released(store: &Path) {
    wait_until(
        "the owner releases the store",
        Duration::from_secs(10),
        || Engine::open_existing(store).is_ok(),
    );
}

async fn mcp_owner(
    env: &Env,
    extra: &[String],
) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    use rmcp::ServiceExt as _;
    let mut command = tokio::process::Command::new(BIN);
    command
        .arg("--store")
        .arg(&env.store)
        .arg("mcp")
        .arg("--root")
        .arg(&env.ws)
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    ().serve(rmcp::transport::TokioChildProcess::new(command).unwrap())
        .await
        .unwrap()
}

async fn call(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    tool: &'static str,
    arguments: serde_json::Value,
) -> String {
    let result = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new(tool)
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(false), "{tool}: {result:?}");
    let rmcp::model::ContentBlock::Text(text) = &result.content[0] else {
        panic!("one text block");
    };
    text.text.to_string()
}

/// The real MCP owner: no policy and a disabled config serve identical
/// bytes; an enabled config adds `route:` to `auto` contexts only, and
/// `status` names the policy; restarting without it rolls back.
#[tokio::test]
async fn the_mcp_owner_serves_routes_reports_and_rolls_back_by_restart() {
    let env = Env::new(0.6);
    let (candidate, config) = env.selected("seed-a");
    let disabled = env.path("disabled.json");
    std::fs::write(&disabled, br#"{"v":2,"enabled":false}"#).unwrap();
    let auto = json!({"query": SEARCH_QUERY});
    let explicit = json!({"query": GRAPH_QUERY, "strategy": "graph"});
    let search = json!({"query": SEARCH_QUERY});

    let mut baseline = Vec::new();
    for extra in [
        Vec::new(),
        vec!["--policy-config".to_owned(), path_arg(&disabled)],
    ] {
        let client = mcp_owner(&env, &extra).await;
        let texts = vec![
            call(&client, "context", auto.clone()).await,
            call(&client, "context", explicit.clone()).await,
            call(&client, "search", search.clone()).await,
        ];
        let status: serde_json::Value =
            serde_json::from_str(&call(&client, "status", json!({})).await).unwrap();
        assert_eq!(status["policy"], json!({"state": "disabled"}));
        client.cancel().await.unwrap();
        released(&env.store);
        assert!(texts.iter().all(|text| route(text).is_none()));
        if baseline.is_empty() {
            baseline = texts;
        } else {
            assert_eq!(
                texts, baseline,
                "a disabled config is byte-identical to none"
            );
        }
    }

    let client = mcp_owner(
        &env,
        &[
            "--policy-config".to_owned(),
            path_arg(&config),
            "--development-isolation".to_owned(),
        ],
    )
    .await;
    let status: serde_json::Value =
        serde_json::from_str(&call(&client, "status", json!({})).await).unwrap();
    assert_eq!(status["policy"]["state"], "enabled", "{status}");
    assert_eq!(status["policy"]["candidate"], manifest_sha(&candidate));
    let routed = call(&client, "context", auto.clone()).await;
    let word = route(&routed).expect("route segment");
    assert!(
        word == "policy" || word == "fallback:policy_abstained",
        "{routed}"
    );
    assert_eq!(
        call(&client, "context", explicit.clone()).await,
        baseline[1]
    );
    assert_eq!(call(&client, "search", search.clone()).await, baseline[2]);
    client.cancel().await.unwrap();
    released(&env.store);

    // Rollback: the same owner restarted without the config.
    let client = mcp_owner(&env, &[]).await;
    assert_eq!(call(&client, "context", auto).await, baseline[0]);
    client.cancel().await.unwrap();
}

// ---------------------------------------------------------------------------
// Real checkpoint
// ---------------------------------------------------------------------------

#[cfg(feature = "learning-worker")]
mod real {
    use super::*;
    use context_foundry::decision_model::{self, SpecialIds};
    use context_foundry::learning::candidate::{HEAD, open};
    use context_foundry::learning::profile::LearnProfile;
    use context_foundry::learning::serve::{PolicyWorker, ServeLaunch};

    /// The served function is the evaluated function: a candidate T002
    /// trains on the tiny dataset with the real checkpoint, served by the
    /// real worker, gives every evaluation row exactly the calibrated
    /// probabilities its evaluation report recorded (same rows, same option
    /// order).
    #[test]
    fn served_probabilities_equal_the_evaluation_probabilities() {
        let env = Env::with(Worker::Real, 2, 0.0);
        let candidate = env.candidate("seed-a", Selection::lenient(0.0));
        // Parity is about the served function, not enablement: whether the
        // real model's routes over this fixture gain evidence (the economics
        // gate `select` and owner startup enforce) is not what this test
        // fixes, so it reads the candidate and the profile back itself, as
        // `verify_config` does.
        let control = Control::unbounded();
        let (dir, verified) = open(&candidate, "candidate_invalid", &control).unwrap();
        let profile = LearnProfile::load(&env.profile).unwrap();
        let manifest = &verified.manifest;
        let head = manifest
            .files
            .iter()
            .find(|file| file.name == HEAD)
            .expect("the manifest binds the head");
        let worker = PolicyWorker::start(
            ServeLaunch {
                profile: &profile,
                checkpoint: &manifest.encoder.checkpoint,
                model_function_sha256: &manifest.model_function_sha256,
                candidate_sha256: &verified.manifest_sha256,
                temperature: manifest.temperature,
                candidate: &dir,
                head_sha256: &head.sha256,
                extra_args: Vec::new(),
            },
            &control,
        )
        .expect("the real worker loads the candidate");
        let renderer = decision_model::Renderer::load(
            &read(&tokenizer_dir().join("tokenizer.json")),
            SpecialIds::PINNED,
        )
        .unwrap();
        // The evaluation rows' exact states, by example ID.
        let dataset = env.path("dataset-seed-a");
        let states: std::collections::BTreeMap<String, String> =
            std::fs::read_to_string(dataset.join("evaluation.jsonl"))
                .unwrap()
                .lines()
                .map(|line| {
                    let row: serde_json::Value = serde_json::from_str(line).unwrap();
                    (
                        row["example_id"].as_str().unwrap().to_owned(),
                        row["feedback"]["state"].as_str().unwrap().to_owned(),
                    )
                })
                .collect();
        let report = &verified.report;
        assert!(!report.cases.is_empty());
        let mut worst = 0f64;
        for case in &report.cases {
            let order = [case.option_ids[0].as_str(), case.option_ids[1].as_str()];
            let state = &states[&case.example_id];
            let rendered = renderer.render(state, order).unwrap();
            let prediction = worker
                .predict(
                    state,
                    order,
                    &rendered,
                    Instant::now() + Duration::from_secs(60),
                )
                .unwrap_or_else(|refused| panic!("{}: {refused:?}", case.example_id));
            let evaluated = case
                .probabilities
                .expect("the report kept the probabilities");
            let served = order.map(|id| {
                if id == "search" {
                    prediction.probabilities.search
                } else {
                    prediction.probabilities.graph
                }
            });
            for (a, b) in served.iter().zip(&evaluated) {
                worst = worst.max((a - b).abs());
            }
            assert_eq!(
                served, evaluated,
                "example {}: served {served:?}, evaluated {evaluated:?}",
                case.example_id
            );
        }
        println!(
            "PARITY served vs evaluated probabilities: {} rows, max abs {worst:.3e}",
            report.cases.len()
        );
    }
}
