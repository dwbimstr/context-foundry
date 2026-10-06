//! 013 T001 acceptance (SC-001): the exact permitted dataset. The renderer
//! is checked ID-for-ID against the pinned upstream fixture
//! (`tests/fixtures/learning/render.json`, generated from the UNCHANGED
//! `laya.common.build_sequence` at 4066d5d5 under the publisher tokenizer);
//! rows, consent, splits, duplicates, lineage, floors, output ownership and
//! read-back run the real preparation path end to end.
#![cfg(feature = "semantic")]
use context_foundry::decision_model::{self, SpecialIds};
use context_foundry::learning::{self, FeedbackRowV4, PrepareOutcome};
use context_foundry::scip::ImportReport;
use context_foundry::testkit;
use context_foundry::{Control, Engine, FResult, FoundryError};
use protobuf::{EnumOrUnknown, Message};
use scip::types::{Document, Index, Occurrence, PositionEncoding};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// The pinned checkpoint tokenizer (`CONTEXT_FOUNDRY_013_TOKENIZER` points a
/// hermetic run elsewhere; the fixture's IDs reproduce only with these exact
/// bytes, which the fixture's own hashes name).
fn tokenizer_dir() -> PathBuf {
    std::env::var_os("CONTEXT_FOUNDRY_013_TOKENIZER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(&std::env::var("HOME").expect("HOME"))
                .join("VSC_DEV/models/laya-typed-decisions-1a793eb5/tokenizer")
        })
}

fn renderer() -> decision_model::Renderer {
    let path = tokenizer_dir().join("tokenizer.json");
    let json = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("pinned tokenizer missing ({}): {e}", path.display()));
    decision_model::Renderer::load(&json, SpecialIds::PINNED).expect("tokenizer loads")
}

fn fixture() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/learning/render.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture readable"))
        .expect("fixture parses")
}

fn digest_of(bytes: &[u8]) -> String {
    context_foundry::digest(bytes)
}

/// The v2 policy pinning the real tokenizer files by hash.
fn write_policy(dir: &Path, seed: &str) -> PathBuf {
    let tokenizer = tokenizer_dir();
    let json = std::fs::read(tokenizer.join("tokenizer.json")).unwrap();
    let config = std::fs::read(tokenizer.join("tokenizer_config.json")).unwrap();
    let policy = policy_value(seed, &tokenizer, &digest_of(&json), &digest_of(&config));
    let path = dir.join(format!("policy-{seed}.json"));
    std::fs::write(&path, serde_json::to_string(&policy).unwrap()).unwrap();
    path
}

/// A complete v2 policy: T001's fields plus T002's fitting and selection
/// fields (one policy file serves prepare and train). Preparation reads no
/// weights, so the checkpoint pin here is a placeholder identity.
fn policy_value(
    seed: &str,
    tokenizer: &Path,
    json_sha: &str,
    config_sha: &str,
) -> serde_json::Value {
    serde_json::json!({
        "v": 2,
        "recipe": learning::RECIPE,
        "seed": seed,
        "tokenizer": {
            "dir": tokenizer,
            "json_sha256": json_sha,
            "config_sha256": config_sha,
        },
        "model": {
            "weights_sha256": "1".repeat(64),
            "encoder_config_sha256": "2".repeat(64),
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
        "max_steps": 8,
        "wall_seconds": 600,
        "memory_bytes": 4u64 << 30,
        "output_bytes": 256u64 << 20,
        "cpu_threads": 2,
        "isolation_profile": "/private/tmp/cf-013-learn-profile.json",
        "enforcement": {
            "memory": "supervised",
            "cpu": "hard",
            "output": "hard",
            "process_count": "hard",
        },
        "selection": {
            "threshold": 0.8,
            "coverage_floor": 0.5,
            "accepted_accuracy_floor": 0.9,
            "max_macro_accuracy_drop": 0.0,
            "critical_groups": [],
        },
    })
}

struct Env {
    dir: tempfile::TempDir,
    store: PathBuf,
    policy: PathBuf,
}

impl Env {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().join("ws");
        std::fs::create_dir(&ws).unwrap();
        let store = dir.path().join("store");
        drop(Engine::initialize(&store, &ws).unwrap());
        let policy = write_policy(dir.path(), "seed-alpha");
        Self { dir, store, policy }
    }

    fn out(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// A fresh engine; callers drop it before the next call because the
    /// store admits one owner.
    fn engine(&self) -> Engine {
        Engine::open_existing(&self.store).unwrap()
    }

    fn record(&self, rows: &[String]) {
        let engine = self.engine();
        for raw in rows {
            engine.record_learning_feedback(raw).unwrap();
        }
    }

    fn prepare_with(
        &self,
        out: &Path,
        policy: &Path,
        parent: Option<&Path>,
    ) -> Result<PrepareOutcome, FoundryError> {
        let engine = self.engine();
        learning::prepare(&engine, out, policy, parent, &Control::unbounded())
    }

    fn prepare(&self, out: &Path) -> learning::Prepared {
        match self
            .prepare_with(out, &self.policy, None)
            .expect("preparation completes")
        {
            PrepareOutcome::Completed(prepared) => *prepared,
            PrepareOutcome::NoNewData { .. } => panic!("expected a completed preparation"),
        }
    }

    fn prepare_child(&self, out: &Path, parent: &Path) -> learning::Prepared {
        match self
            .prepare_with(out, &self.policy, Some(parent))
            .expect("child preparation completes")
        {
            PrepareOutcome::Completed(prepared) => *prepared,
            PrepareOutcome::NoNewData { .. } => panic!("expected new data"),
        }
    }

    fn prepare_err(&self, out: &Path) -> FoundryError {
        self.prepare_with(out, &self.policy, None).unwrap_err()
    }

    fn prepare_child_err(&self, out: &Path, parent: &Path) -> FoundryError {
        self.prepare_with(out, &self.policy, Some(parent))
            .unwrap_err()
    }
}

fn raw_row(task: &str, group: &str, label: &str, order: [&str; 2], state: &str) -> String {
    serde_json::json!({
        "task_id": task,
        "task_group_id": group,
        "family": decision_model::FAMILY,
        "state": state,
        "option_ids": order,
        "correct_option_id": label,
        "label_source": "operator",
        "label_evidence": format!("evidence for {task}"),
        "rights_ref": "rights-checked",
        "allow_training": true,
    })
    .to_string()
}

fn simple_row(task: &str, group: &str, label: &str) -> String {
    raw_row(
        task,
        group,
        label,
        ["search", "graph"],
        &format!("where is {task} defined\ngraph: complete"),
    )
}

/// Group names bucketed by the deterministic split rule, exactly enough for
/// the three floors.
fn floor_groups() -> [Vec<String>; 3] {
    let mut train = Vec::new();
    let mut calibration = Vec::new();
    let mut evaluation = Vec::new();
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

/// A store whose eligible rows meet every floor with both labels per split.
fn floored_env() -> Env {
    let env = Env::new();
    let mut rows = Vec::new();
    for (split, groups) in ["t", "c", "e"].iter().zip(floor_groups()) {
        for (i, group) in groups.iter().enumerate() {
            let label = if i % 2 == 0 { "search" } else { "graph" };
            rows.push(simple_row(&format!("{split}{i}"), group, label));
        }
    }
    env.record(&rows);
    env
}

/// Replace one dataset member and re-seal the manifest around it (entry
/// bytes/sha/rows and the recomputed dataset id), so only the member's own
/// content can object.
fn reseal(dataset: &Path, name: &str, body: &[u8]) {
    std::fs::write(dataset.join(name), body).unwrap();
    let manifest_path = dataset.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    for entry in manifest["files"].as_array_mut().unwrap() {
        if entry["name"] == name {
            entry["sha256"] = digest_of(body).into();
            entry["bytes"] = body.len().into();
            entry["rows"] = body.iter().filter(|b| **b == b'\n').count().into();
        }
    }
    let file_sha = |manifest: &serde_json::Value, file: &str| -> String {
        manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"] == file)
            .unwrap()["sha256"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    manifest["dataset_id"] = digest_of(
        serde_json::to_string(&serde_json::json!([
            manifest["workspace_id"],
            manifest["parent_manifest_sha256"],
            manifest["model_function_sha256"],
            manifest["policy_sha256"],
            file_sha(&manifest, "train.jsonl"),
            file_sha(&manifest, "calibration.jsonl"),
            file_sha(&manifest, "evaluation.jsonl"),
        ]))
        .unwrap()
        .as_bytes(),
    )
    .into();
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();
}

fn read_json_lines(path: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// Write JSON values back as compact JSONL rows (each with its LF).
fn json_lines_body(rows: &[serde_json::Value]) -> String {
    rows.iter().map(|row| format!("{row}\n")).collect()
}

/// The SHA-256 of a dataset's exact manifest bytes: its parent identity.
fn manifest_sha(dataset: &Path) -> String {
    digest_of(&std::fs::read(dataset.join("manifest.json")).unwrap())
}

/// Every example ID a dataset's `groups.jsonl` coverage lists.
fn coverage_ids(dataset: &Path) -> Vec<String> {
    read_json_lines(&dataset.join("groups.jsonl"))
        .iter()
        .flat_map(|group| {
            group["examples"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["example_id"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn file_example_ids(path: &Path) -> Vec<String> {
    read_json_lines(path)
        .iter()
        .map(|row| row["example_id"].as_str().unwrap().to_owned())
        .collect()
}

/// Every stored v4 row, by example ID.
fn stored_rows(env: &Env) -> Vec<(String, FeedbackRowV4)> {
    env.engine()
        .learning_feedback_rows(&Control::unbounded())
        .unwrap()
}

/// The same row with one field replaced, as raw JSON for `record`.
fn with_field(row: &FeedbackRowV4, field: &str, value: serde_json::Value) -> String {
    let mut raw = serde_json::to_value(row).unwrap();
    raw[field] = value;
    raw.to_string()
}

fn check(dataset: &Path, policy: &Path) -> FResult<learning::CheckReport> {
    learning::check(
        &dataset.join("manifest.json"),
        policy,
        &Control::unbounded(),
    )
}

/// Generous for a process that loads the tokenizer; a parser that loops
/// never meets it.
const CLI_LIMIT: Duration = Duration::from_secs(60);

fn foundry(store: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_foundry"));
    command.arg("--store").arg(store);
    command
}

/// Run a real binary with `stdin`, failing — and killing it — if it has not
/// exited within [`CLI_LIMIT`]: a hang is a test failure, never a stuck suite.
fn run_bounded(mut command: Command, stdin: &[u8]) -> Output {
    use std::io::Write as _;
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // A child that refuses before reading stdin may close it first.
    let _ = child.stdin.take().unwrap().write_all(stdin);
    let deadline = Instant::now() + CLI_LIMIT;
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("{command:?} did not exit within {CLI_LIMIT:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

/// The bounded error JSON's `code` on stderr.
fn error_code(out: &Output) -> String {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let error: serde_json::Value = serde_json::from_str(stderr.trim())
        .unwrap_or_else(|_| panic!("no error JSON on stderr: {stderr}"));
    error["code"].as_str().unwrap().to_owned()
}

// ---------------------------------------------------------------------------
// Renderer parity with the pinned upstream fixture
// ---------------------------------------------------------------------------

#[test]
fn renderer_reproduces_the_pinned_upstream_fixture_exactly() {
    let renderer = renderer();
    let fixture = fixture();
    let tokenizer = tokenizer_dir();
    assert_eq!(
        fixture["tokenizer_json_sha256"].as_str().unwrap(),
        digest_of(&std::fs::read(tokenizer.join("tokenizer.json")).unwrap()),
        "the fixture was generated with these exact tokenizer bytes"
    );
    assert_eq!(
        fixture["tokenizer_config_sha256"].as_str().unwrap(),
        digest_of(&std::fs::read(tokenizer.join("tokenizer_config.json")).unwrap()),
    );
    let mask = fixture["special_ids"]["mask"].as_u64().unwrap() as u32;
    let mut rendered_cases = 0;
    let mut refused_cases = 0;
    for case in fixture["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let state = case["state"].as_str().unwrap();
        let order = case["option_order"].as_array().unwrap();
        let order = [order[0].as_str().unwrap(), order[1].as_str().unwrap()];
        match case["expect"].as_str().unwrap() {
            "render" => {
                let rendered = renderer
                    .render(state, order)
                    .unwrap_or_else(|e| panic!("case {name} must render within limits: {e}"));
                let expected: Vec<u32> = case["ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u32)
                    .collect();
                assert_eq!(rendered.ids, expected, "case {name} token ids");
                assert_eq!(
                    rendered.ids.len(),
                    case["untruncated_tokens"].as_u64().unwrap() as usize,
                    "case {name} length"
                );
                let markers = case["markers"].as_array().unwrap();
                assert_eq!(
                    rendered.markers,
                    [
                        markers[0].as_u64().unwrap() as usize,
                        markers[1].as_u64().unwrap() as usize
                    ],
                    "case {name} markers"
                );
                assert_eq!(rendered.ids[rendered.markers[0]], mask);
                assert_eq!(rendered.ids[rendered.markers[1]], mask);
                rendered_cases += 1;
            }
            "refuse" => {
                // Upstream truncates here; Foundry refuses, never truncates.
                let error = renderer.render(state, order).unwrap_err();
                assert_eq!(error.code(), "input_too_long", "case {name}: {error}");
                refused_cases += 1;
            }
            other => panic!("unknown fixture expectation {other:?}"),
        }
    }
    assert_eq!((rendered_cases, refused_cases), (8, 1));
    // The exact 1024-token case is in the fixture: the limit is inclusive.
    assert!(
        fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "total_1024" && c["ids"].as_array().unwrap().len() == 1024)
    );
}

#[test]
fn a_state_over_16_kib_is_refused_before_tokenization() {
    let renderer = renderer();
    let over = format!("{}\ngraph: complete", "x".repeat(16 * 1024 + 1));
    let error = renderer.render(&over, ["search", "graph"]).unwrap_err();
    assert_eq!(error.code(), "state_too_large");
    // A state UNDER the byte guard still refuses at the 1024-token total.
    let under = format!("{}\ngraph: complete", "alpha ".repeat(1200));
    assert!(under.len() < 16 * 1024);
    let error = renderer.render(&under, ["search", "graph"]).unwrap_err();
    assert_eq!(error.code(), "input_too_long");
}

#[test]
fn mask_literals_become_one_space_and_markers_stay_on_the_mask_id() {
    let renderer = renderer();
    let with_mask = "replace [MASK] here\ngraph: complete";
    let rendered = renderer.render(with_mask, ["graph", "search"]).unwrap();
    let cleaned = with_mask.replace("[MASK]", " ");
    let again = renderer.render(&cleaned, ["graph", "search"]).unwrap();
    assert_eq!(rendered.ids, again.ids);
    // The only [MASK] ids are the two option markers.
    let mask_positions: Vec<usize> = rendered
        .ids
        .iter()
        .enumerate()
        .filter(|(_, id)| **id == SpecialIds::PINNED.mask)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(mask_positions, rendered.markers.to_vec());
}

#[test]
fn unknown_or_duplicated_options_cannot_render() {
    let renderer = renderer();
    for order in [
        ["search", "search"],
        ["search", "lexis"],
        ["graph", "graph"],
    ] {
        let error = renderer.render("q\ngraph: complete", order).unwrap_err();
        assert_eq!(error.code(), "row_invalid", "{order:?}");
    }
}

#[test]
fn a_wrong_tokenizer_is_refused_before_any_rendering() {
    // A tokenizer whose special ids differ from the pinned ones.
    let json = std::fs::read(tokenizer_dir().join("tokenizer.json")).unwrap();
    let wrong = SpecialIds {
        cls: 1,
        ..SpecialIds::PINNED
    };
    match decision_model::Renderer::load(&json, wrong) {
        Ok(_) => panic!("wrong special id must be refused"),
        Err(error) => assert_eq!(error.code(), "tokenizer_invalid"),
    }
    assert_eq!(
        decision_model::Renderer::load(b"{}", SpecialIds::PINNED)
            .err()
            .map(|e| e.code()),
        Some("tokenizer_invalid")
    );
}

#[test]
fn input_and_example_digests_bind_order_task_and_exact_bytes() {
    let base = FeedbackRowV4::parse(&simple_row("task-a", "g1", "search")).unwrap();
    let reordered = FeedbackRowV4::parse(&raw_row(
        "task-a",
        "g1",
        "search",
        ["graph", "search"],
        "where is task-a defined\ngraph: complete",
    ))
    .unwrap();
    let crlf = FeedbackRowV4::parse(&raw_row(
        "task-a",
        "g1",
        "search",
        ["search", "graph"],
        "where is task-a defined\r\ngraph: complete",
    ))
    .unwrap();
    let other_task = FeedbackRowV4::parse(&raw_row(
        "task-b",
        "g1",
        "search",
        ["search", "graph"],
        "where is task-a defined\ngraph: complete",
    ))
    .unwrap();
    assert_ne!(base.input_sha256(), reordered.input_sha256());
    assert_ne!(base.input_sha256(), crlf.input_sha256());
    assert_eq!(base.input_sha256(), other_task.input_sha256());
    assert_ne!(base.example_id(), other_task.example_id());
    assert_ne!(base.example_id(), reordered.example_id());
    assert_eq!(base.example_id().len(), 64);
    // Normalization is for DUPLICATE detection only: CRLF and the original
    // bytes differ in identity but not in fingerprint.
    assert_eq!(base.fingerprint(), crlf.fingerprint());
    let spaced = FeedbackRowV4::parse(&raw_row(
        "task-a",
        "g1",
        "search",
        ["search", "graph"],
        "  where   is task-a\tdefined \ngraph: complete",
    ))
    .unwrap();
    assert_eq!(base.fingerprint(), spaced.fingerprint());
    // Option order never changes the fingerprint (sorted by stable id).
    assert_eq!(base.fingerprint(), reordered.fingerprint());
    // Case is NOT folded.
    let upper = FeedbackRowV4::parse(&raw_row(
        "task-a",
        "g1",
        "search",
        ["search", "graph"],
        "WHERE is task-a defined\ngraph: complete",
    ))
    .unwrap();
    assert_ne!(base.fingerprint(), upper.fingerprint());
}

// ---------------------------------------------------------------------------
// Row validation
// ---------------------------------------------------------------------------

fn row_value() -> serde_json::Value {
    serde_json::from_str(&simple_row("t", "g", "search")).unwrap()
}

#[test]
fn v4_rows_reject_unknown_null_missing_and_invalid_fields() {
    let mut cases: Vec<(&str, serde_json::Value)> = Vec::new();
    let mut with = |name, f: &dyn Fn(&mut serde_json::Value)| {
        let mut v = row_value();
        f(&mut v);
        cases.push((name, v));
    };
    with("unknown field", &|v| {
        v["extra"] = 1.into();
    });
    with("null field", &|v| {
        v["rights_ref"] = serde_json::Value::Null;
    });
    with("missing field", &|v| {
        v.as_object_mut().unwrap().remove("rights_ref");
    });
    with("oversized task id", &|v| {
        v["task_id"] = "x".repeat(257).into();
    });
    with("blank task id", &|v| {
        v["task_id"] = " ".into();
    });
    with("blank group", &|v| {
        v["task_group_id"] = "".into();
    });
    with("blank evidence", &|v| {
        v["label_evidence"] = "  ".into();
    });
    with("wrong family", &|v| {
        v["family"] = "other-v1".into();
    });
    with("one option", &|v| {
        v["option_ids"] = serde_json::json!(["search"]);
    });
    with("duplicated option", &|v| {
        v["option_ids"] = serde_json::json!(["search", "search"]);
    });
    with("unknown option", &|v| {
        v["option_ids"] = serde_json::json!(["search", "lexis"]);
    });
    with("wrong label", &|v| {
        v["correct_option_id"] = "lexis".into();
    });
    with("wrong label source", &|v| {
        v["label_source"] = "model".into();
    });
    with("oversized rights", &|v| {
        v["rights_ref"] = "r".repeat(1025).into();
    });
    with("oversized evidence", &|v| {
        v["label_evidence"] = "e".repeat(1025).into();
    });
    with("oversized state", &|v| {
        v["state"] = "s".repeat(16 * 1024 + 1).into();
    });
    with("empty state", &|v| {
        v["state"] = "".into();
    });
    with("non-boolean consent", &|v| {
        v["allow_training"] = "yes".into();
    });
    with("array option wrong type", &|v| {
        v["option_ids"] = "search,graph".into();
    });
    for (name, value) in cases {
        let error = FeedbackRowV4::parse(&value.to_string()).unwrap_err();
        assert_eq!(error.code(), "row_invalid", "{name}: {error}");
    }
    // A whole row over 24 KiB: escapes inflate the raw bytes past the bound
    // even though every logical field is within its own limit.
    let mut v = row_value();
    v["state"] = "\u{1}".repeat(16 * 1024).into();
    let raw = v.to_string();
    assert!(raw.len() > 24 * 1024);
    assert_eq!(
        FeedbackRowV4::parse(&raw).unwrap_err().code(),
        "row_invalid"
    );
}

#[test]
fn every_byte_limit_holds_at_the_limit_and_refuses_one_past_it() {
    // A leading multi-byte character makes each limit a UTF-8 BYTE count.
    for (field, limit) in [
        ("task_id", 256),
        ("task_group_id", 256),
        ("label_evidence", 1024),
        ("rights_ref", 1024),
        ("state", 16 * 1024),
    ] {
        let at = format!("é{}", "x".repeat(limit - 2));
        assert_eq!(at.len(), limit);
        let mut v = row_value();
        v[field] = at.clone().into();
        FeedbackRowV4::parse(&v.to_string())
            .unwrap_or_else(|e| panic!("{field} at {limit} bytes must be accepted: {e}"));
        v[field] = format!("{at}x").into();
        assert_eq!(
            FeedbackRowV4::parse(&v.to_string()).unwrap_err().code(),
            "row_invalid",
            "{field} at {} bytes",
            limit + 1
        );
    }
    // The whole row: a control character escapes to six raw bytes, so the
    // row reaches exactly 24 KiB while every field is within its own limit.
    let target = 24 * 1024;
    let mut v = row_value();
    v["state"] = "s".into();
    let fixed = v.to_string().len() - 1;
    let state = format!(
        "{}{}",
        "\u{1}".repeat((target - fixed) / 6),
        "s".repeat((target - fixed) % 6)
    );
    v["state"] = state.clone().into();
    let raw = v.to_string();
    assert_eq!(raw.len(), target);
    FeedbackRowV4::parse(&raw).expect("a row of exactly 24 KiB is accepted");
    v["state"] = format!("{state}s").into();
    let raw = v.to_string();
    assert_eq!(raw.len(), target + 1);
    assert_eq!(
        FeedbackRowV4::parse(&raw).unwrap_err().code(),
        "row_invalid"
    );
}

#[test]
fn duplicate_json_keys_are_rejected_in_rows_policies_and_nested_objects() {
    let dup = r#"{"task_id":"t","task_id":"t2","task_group_id":"g","family":"retrieval-route-v1","state":"s\ngraph: complete","option_ids":["search","graph"],"correct_option_id":"search","label_source":"operator","label_evidence":"e","rights_ref":"r","allow_training":true}"#;
    assert_eq!(FeedbackRowV4::parse(dup).unwrap_err().code(), "row_invalid");
    // Keys that differ only by an escape are still duplicates.
    let escaped = dup.replacen("\"task_id\":\"t2\"", "\"task\\u005fid\":\"t2\"", 1);
    assert_eq!(
        FeedbackRowV4::parse(&escaped).unwrap_err().code(),
        "row_invalid"
    );
    // A duplicate inside the policy's nested tokenizer object.
    let env = Env::new();
    let policy = std::fs::read_to_string(&env.policy).unwrap();
    let nested = policy.replacen(
        "\"json_sha256\"",
        "\"json_sha256\":\"0\",\"json_sha256\"",
        1,
    );
    let path = env.out("dup-policy.json");
    std::fs::write(&path, nested).unwrap();
    assert_eq!(
        learning::LearningPolicy::load(&path).unwrap_err().code(),
        "policy_invalid"
    );
}

#[test]
fn malformed_json_fails_at_once_through_the_real_cli() {
    // The strict parser is serde_json's own, so malformed syntax fails at the
    // first bad byte. Every case runs the real binary under an external
    // timeout: a parser that loops is a failing test, never a hung suite.
    let env = Env::new();
    let malformed = [
        r#"{"option_ids":[}"#.to_owned(),
        r#"{"option_ids":[,]}"#.to_owned(),
        r#"{"option_ids":["search" "graph"]}"#.to_owned(),
        r#"{"task_id":"t",}"#.to_owned(),
        r#"{"task_id":}"#.to_owned(),
        r#"{"task_id""t"}"#.to_owned(),
        r#"{"task_id":"t"} trailing"#.to_owned(),
        "{".to_owned(),
        "[".to_owned(),
        "}".to_owned(),
        r#""\u12"#.to_owned(),
        "[".repeat(200),
    ];
    for input in &malformed {
        let mut command = foundry(&env.store);
        command.args(["feedback", "v4"]);
        let out = run_bounded(command, input.as_bytes());
        assert_eq!(
            out.status.code(),
            Some(2),
            "{input}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(error_code(&out), "row_invalid", "{input}");
        assert!(out.stdout.is_empty(), "{input}");
    }
    assert!(
        testkit::table_rows(&env.store, "learning_feedback").is_empty(),
        "no malformed row was recorded"
    );
    // The same inputs as a run policy and as a dataset manifest.
    let dataset = env.out("malformed-dataset");
    std::fs::create_dir(&dataset).unwrap();
    let policy = env.out("malformed-policy.json");
    for input in &malformed {
        std::fs::write(&policy, input).unwrap();
        let mut command = foundry(&env.store);
        command
            .args(["learning", "prepare", "--out"])
            .arg(env.out("never"))
            .arg("--policy")
            .arg(&policy);
        let out = run_bounded(command, b"");
        assert_eq!(out.status.code(), Some(2), "policy {input}");
        assert_eq!(error_code(&out), "policy_invalid", "policy {input}");
        std::fs::write(dataset.join("manifest.json"), input).unwrap();
        let mut command = foundry(&env.store);
        command
            .args(["learning", "check", "--manifest"])
            .arg(dataset.join("manifest.json"))
            .arg("--policy")
            .arg(&env.policy);
        let out = run_bounded(command, b"");
        assert_eq!(out.status.code(), Some(2), "manifest {input}");
        assert_eq!(error_code(&out), "dataset_invalid", "manifest {input}");
    }
    assert!(!env.out("never").exists());
}

#[test]
fn policy_validation_is_strict() {
    let env = Env::new();
    let base: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&env.policy).unwrap()).unwrap();
    let bad = |name: &str, f: &dyn Fn(&mut serde_json::Value)| {
        let mut v = base.clone();
        f(&mut v);
        let path = env.out("p.json");
        std::fs::write(&path, v.to_string()).unwrap();
        let error = learning::LearningPolicy::load(&path).unwrap_err();
        assert_eq!(error.code(), "policy_invalid", "{name}: {error}");
    };
    bad("wrong version", &|v| v["v"] = 3.into());
    bad("wrong recipe", &|v| v["recipe"] = "other".into());
    bad("blank seed", &|v| v["seed"] = " ".into());
    bad("unknown field", &|v| v["extra"] = 1.into());
    bad("short hash", &|v| {
        v["tokenizer"]["json_sha256"] = "abc".into()
    });
    bad("uppercase hash", &|v| {
        v["tokenizer"]["config_sha256"] = "A".repeat(64).into()
    });
    // T002's fitting fields: exact pins, the recipe's optimizer, every bound
    // refused (never clamped) one past each end, an absolute profile path.
    bad("missing model", &|v| {
        v.as_object_mut().unwrap().remove("model");
    });
    bad("short weights pin", &|v| {
        v["model"]["weights_sha256"] = "abc".into()
    });
    bad("unknown dtype", &|v| {
        v["model"]["source_dtype"] = "F64".into()
    });
    bad("base not a digest", &|v| v["base"] = "latest".into());
    bad("other learning rate", &|v| {
        v["optimizer"]["learning_rate"] = 2e-4.into()
    });
    bad("other optimizer", &|v| {
        v["optimizer"]["name"] = "sgd".into()
    });
    bad("zero steps", &|v| v["max_steps"] = 0.into());
    bad("too many steps", &|v| v["max_steps"] = 1_000_001.into());
    bad("zero wall", &|v| v["wall_seconds"] = 0.into());
    bad("wall past 7200", &|v| v["wall_seconds"] = 7201.into());
    bad("memory below 4 GiB", &|v| {
        v["memory_bytes"] = ((4u64 << 30) - 1).into()
    });
    bad("memory past 8 GiB", &|v| {
        v["memory_bytes"] = ((8u64 << 30) + 1).into()
    });
    bad("output below 128 MiB", &|v| {
        v["output_bytes"] = ((128u64 << 20) - 1).into()
    });
    bad("output past 2 GiB", &|v| {
        v["output_bytes"] = ((2u64 << 30) + 1).into()
    });
    bad("zero threads", &|v| v["cpu_threads"] = 0.into());
    bad("five threads", &|v| v["cpu_threads"] = 5.into());
    bad("relative profile", &|v| {
        v["isolation_profile"] = "profile.json".into()
    });
    bad("unknown enforcement", &|v| {
        v["enforcement"]["memory"] = "soft".into()
    });
    bad("threshold past 1", &|v| {
        v["selection"]["threshold"] = 1.5.into()
    });
    bad("duplicate critical group", &|v| {
        v["selection"]["critical_groups"] = json!(["g", "g"])
    });
    let good = |name: &str, f: &dyn Fn(&mut serde_json::Value)| {
        let mut v = base.clone();
        f(&mut v);
        let path = env.out("p.json");
        std::fs::write(&path, v.to_string()).unwrap();
        learning::LearningPolicy::load(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
    };
    // The limits themselves are admitted, and the selection policy defaults.
    good("one step", &|v| v["max_steps"] = 1.into());
    good("the step limit", &|v| v["max_steps"] = 1_000_000.into());
    good("the wall limit", &|v| v["wall_seconds"] = 7200.into());
    good("8 GiB", &|v| v["memory_bytes"] = (8u64 << 30).into());
    good("2 GiB output", &|v| v["output_bytes"] = (2u64 << 30).into());
    good("four threads", &|v| v["cpu_threads"] = 4.into());
    good("default selection", &|v| {
        v.as_object_mut().unwrap().remove("selection");
    });
    let (policy, sha) = learning::LearningPolicy::load(&env.policy).unwrap();
    assert_eq!(policy.recipe, learning::RECIPE);
    assert_eq!(sha, digest_of(&std::fs::read(&env.policy).unwrap()));
    assert_eq!(policy.selection, learning::SelectionPolicy::default());
}

// ---------------------------------------------------------------------------
// Recording, correction and withdrawal
// ---------------------------------------------------------------------------

#[test]
fn recording_is_idempotent_and_a_correction_replaces_transactionally() {
    let env = Env::new();
    let engine = env.engine();
    let raw = simple_row("t", "g", "search");
    let (first, status) = engine.record_learning_feedback(&raw).unwrap();
    assert_eq!(status, "created");
    let (again, status) = engine.record_learning_feedback(&raw).unwrap();
    assert_eq!((again.as_str(), status), (first.as_str(), "unchanged"));
    // A label correction for the SAME input replaces the row.
    let corrected = simple_row("t", "g", "graph");
    let (third, status) = engine.record_learning_feedback(&corrected).unwrap();
    assert_eq!((third.as_str(), status), (first.as_str(), "replaced"));
    let rows = engine
        .learning_feedback_rows(&Control::unbounded())
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1.correct_option_id, "graph");
    // A changed state is a NEW example, never a mislabeled correction.
    let changed = raw_row(
        "t",
        "g",
        "search",
        ["search", "graph"],
        "other\ngraph: partial",
    );
    let (fourth, status) = engine.record_learning_feedback(&changed).unwrap();
    assert_ne!(fourth, first);
    assert_eq!(status, "created");
    // Changed option ORDER is also a new example.
    let reordered = raw_row(
        "t",
        "g",
        "search",
        ["graph", "search"],
        "where is t defined\ngraph: complete",
    );
    let (fifth, status) = engine.record_learning_feedback(&reordered).unwrap();
    assert_ne!(fifth, first);
    assert_eq!(status, "created");
    assert_eq!(
        engine
            .learning_feedback_rows(&Control::unbounded())
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn a_refused_row_mutates_nothing() {
    let env = Env::new();
    env.record(&[simple_row("keep", "g", "search")]);
    let before = testkit::snapshot(&env.store);
    let engine = env.engine();
    let mut bad = row_value();
    bad["rights_ref"] = serde_json::Value::Null;
    assert!(engine.record_learning_feedback(&bad.to_string()).is_err());
    drop(engine);
    assert_eq!(testkit::snapshot(&env.store), before);
}

#[test]
fn legacy_feedback_rows_stay_exportable_and_never_eligible() {
    let env = floored_env();
    {
        let engine = env.engine();
        let legacy = serde_json::json!({
            "task_id": "legacy-task",
            "query": "old legacy question text",
            "correct_strategy": "graph",
            "label_source": "operator",
            "allow_training": true,
        });
        let legacy: context_foundry::laya::Feedback = serde_json::from_value(legacy).unwrap();
        engine.record_feedback(&legacy).unwrap();
        let exported = engine.training_examples().unwrap();
        assert_eq!(exported.len(), 1, "legacy export is unchanged");
        assert_eq!(exported[0]["state"], "old legacy question text");
    }
    let out = env.out("dataset");
    let prepared = env.prepare(&out);
    for name in learning::FILES {
        let body = std::fs::read_to_string(out.join(name)).unwrap();
        assert!(
            !body.contains("old legacy question text") && !body.contains("legacy-task"),
            "{name} must not carry the legacy row"
        );
    }
    let rows = prepared.train_rows + prepared.calibration_rows + prepared.evaluation_rows;
    assert_eq!(rows, 22 + 11 + 21, "exactly the v4 rows");
}

#[test]
fn withdrawn_rows_stay_in_history_but_never_in_a_dataset() {
    let env = floored_env();
    let first = env.out("d1");
    env.prepare(&first);
    let rows: Vec<_> = {
        let engine = env.engine();
        engine
            .learning_feedback_rows(&Control::unbounded())
            .unwrap()
    };
    let (_, victim) = rows
        .iter()
        .find(|(_, row)| learning::split_of(&row.task_group_id) == "train")
        .unwrap();
    let mut withdrawn = serde_json::to_value(victim).unwrap();
    withdrawn["allow_training"] = false.into();
    let task = victim.task_id.clone();
    env.record(&[withdrawn.to_string()]);
    let second = env.out("d2");
    let prepared = env.prepare(&second);
    let all: String = learning::FILES
        .iter()
        .map(|name| std::fs::read_to_string(second.join(name)).unwrap())
        .collect();
    assert!(!all.contains(&format!("\"task_id\":\"{task}\"")));
    assert_eq!(
        prepared.train_rows + prepared.calibration_rows + prepared.evaluation_rows,
        22 + 11 + 21 - 1
    );
    // The withdrawn group keeps its split/example/fingerprint trail with no
    // text: history rows hold ids and digests only.
    let history = testkit::table_rows(&env.store, "learning_history");
    assert!(!history.is_empty());
    assert!(history.iter().all(|(_, raw)| !raw.contains("where is")));
}

#[test]
fn a_row_denied_training_at_admission_never_enters_a_dataset() {
    let env = floored_env();
    let train_group = floor_groups()[0][0].clone();
    let mut denied: serde_json::Value = serde_json::from_str(&raw_row(
        "denied-at-admission",
        &train_group,
        "graph",
        ["search", "graph"],
        "an input whose rights were denied\ngraph: complete",
    ))
    .unwrap();
    denied["allow_training"] = false.into();
    let (example_id, status) = env
        .engine()
        .record_learning_feedback(&denied.to_string())
        .unwrap();
    assert_eq!(status, "created", "the row itself is recorded");
    let out = env.out("dataset");
    let prepared = env.prepare(&out);
    assert_eq!(
        prepared.train_rows + prepared.calibration_rows + prepared.evaluation_rows,
        22 + 11 + 21,
        "exactly the permitted rows"
    );
    for name in learning::FILES.iter().chain(&["manifest.json"]) {
        let body = std::fs::read_to_string(out.join(name)).unwrap();
        assert!(
            !body.contains("denied-at-admission") && !body.contains(&example_id),
            "{name} must not carry the denied row"
        );
    }
    assert!(!coverage_ids(&out).contains(&example_id));
    assert!(
        stored_rows(&env)
            .iter()
            .any(|(id, row)| *id == example_id && !row.allow_training),
        "stored, never eligible"
    );
}

// ---------------------------------------------------------------------------
// Splits, floors and duplicate fingerprints
// ---------------------------------------------------------------------------

#[test]
fn split_rule_is_first_eight_bytes_big_endian_mod_ten() {
    for group in ["alpha", "beta", "gamma", "grp-0001", "日本語"] {
        let digest = context_foundry::digest(group.as_bytes());
        let mut prefix = [0u8; 8];
        for (i, byte) in prefix.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&digest[i * 2..i * 2 + 2], 16).unwrap();
        }
        let expected = match u64::from_be_bytes(prefix) % 10 {
            0 => "evaluation",
            1 => "calibration",
            _ => "train",
        };
        assert_eq!(learning::split_of(group), expected, "{group}");
    }
}

#[test]
fn prepare_is_deterministic_and_group_assignments_never_move() {
    let env = floored_env();
    let first = env.out("d1");
    let second = env.out("d2");
    let a = env.prepare(&first);
    let b = env.prepare(&second);
    assert_eq!(a.dataset_id, b.dataset_id, "identical inputs, identical id");
    for name in learning::FILES {
        assert_eq!(
            std::fs::read(first.join(name)).unwrap(),
            std::fs::read(second.join(name)).unwrap(),
            "{name} is byte-identical"
        );
    }
    let groups = read_json_lines(&first.join("groups.jsonl"));
    for (split, floor) in [("train", 20), ("calibration", 10), ("evaluation", 20)] {
        let count = groups.iter().filter(|g| g["split"] == split).count();
        assert!(count >= floor, "{split} has {count} groups, needs {floor}");
    }
    for group in &groups {
        assert_eq!(
            learning::split_of(group["group_id"].as_str().unwrap()),
            group["split"].as_str().unwrap()
        );
    }
}

#[test]
fn group_floors_refuse_small_datasets_and_missing_labels() {
    let env = Env::new();
    env.record(
        &(0..3)
            .map(|i| {
                simple_row(
                    &format!("t{i}"),
                    &format!("g{i}"),
                    if i % 2 == 0 { "search" } else { "graph" },
                )
            })
            .collect::<Vec<_>>(),
    );
    let error = env.prepare_err(&env.out("d"));
    assert_eq!(error.code(), "group_floors", "{error}");
    assert!(!env.out("d").exists());

    // Every split meets its group floor, but one split lacks a label.
    let env = Env::new();
    let mut rows = Vec::new();
    for (split, groups) in ["t", "c", "e"].iter().zip(floor_groups()) {
        for (i, group) in groups.iter().enumerate() {
            // Evaluation: every correct label is `search`.
            let label = if *split == "e" || i % 2 == 0 {
                "search"
            } else {
                "graph"
            };
            rows.push(simple_row(&format!("{split}{i}"), group, label));
        }
    }
    env.record(&rows);
    let error = env.prepare_err(&env.out("d"));
    assert_eq!(error.code(), "group_floors", "{error}");
    assert!(error.to_string().contains("evaluation"), "{error}");
}

#[test]
fn duplicate_fingerprints_across_groups_or_labels_refuse() {
    let state_a = "same normalized state\ngraph: complete";
    let state_b = "same   normalized state\r\ngraph: complete";
    // Across groups.
    let env = Env::new();
    env.record(&[
        raw_row("t1", "g1", "search", ["search", "graph"], state_a),
        raw_row("t2", "g2", "search", ["search", "graph"], state_b),
    ]);
    let error = env.prepare_err(&env.out("d"));
    assert_eq!(error.code(), "duplicate_conflict", "{error}");
    // Same group, conflicting current labels.
    let env = Env::new();
    env.record(&[
        raw_row("t1", "g1", "search", ["search", "graph"], state_a),
        raw_row("t2", "g1", "graph", ["search", "graph"], state_b),
    ]);
    let error = env.prepare_err(&env.out("d"));
    assert_eq!(error.code(), "duplicate_conflict", "{error}");
    assert!(!env.out("d").exists());
}

#[test]
fn historical_fingerprints_conflict_even_after_withdrawal() {
    let env = floored_env();
    env.prepare(&env.out("d1"));
    let rows: Vec<_> = {
        let engine = env.engine();
        engine
            .learning_feedback_rows(&Control::unbounded())
            .unwrap()
    };
    let (_, victim) = rows.first().unwrap();
    let victim_state = victim.state.clone();
    // Withdraw the victim; its group's fingerprint trail remains.
    let mut withdrawn = serde_json::to_value(victim).unwrap();
    withdrawn["allow_training"] = false.into();
    // A NEW example in a DIFFERENT group repeating that exact state.
    let clone = raw_row(
        "fresh-task",
        "fresh-group",
        "search",
        ["search", "graph"],
        &victim_state,
    );
    env.record(&[withdrawn.to_string(), clone]);
    let error = env.prepare_err(&env.out("d2"));
    assert_eq!(error.code(), "duplicate_conflict", "{error}");
    assert!(error.to_string().contains("historical"), "{error}");
    assert!(!env.out("d2").exists());
}

// ---------------------------------------------------------------------------
// Output, ownership and read-back
// ---------------------------------------------------------------------------

#[test]
fn prepare_writes_sorted_files_and_a_binding_manifest() {
    let env = floored_env();
    let out = env.out("dataset");
    let prepared = env.prepare(&out);
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["schema"], 4);
    assert_eq!(manifest["recipe"], learning::RECIPE);
    assert_eq!(manifest["dataset_id"], prepared.dataset_id);
    assert_eq!(manifest["parent_manifest_sha256"], serde_json::Value::Null);
    assert_eq!(manifest["base_candidate_sha256"], serde_json::Value::Null);
    assert_eq!(manifest["workspace_id"].as_str().unwrap().len(), 64);
    assert_eq!(manifest["split_group_counts"]["train"], 22);
    assert_eq!(manifest["split_group_counts"]["calibration"], 11);
    assert_eq!(manifest["split_group_counts"]["evaluation"], 21);
    let entries = manifest["files"].as_array().unwrap();
    let names: Vec<&str> = entries
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "calibration.jsonl",
            "evaluation.jsonl",
            "groups.jsonl",
            "train.jsonl"
        ]
    );
    for entry in entries {
        let name = entry["name"].as_str().unwrap();
        let body = std::fs::read(out.join(name)).unwrap();
        assert!(body.ends_with(b"\n"), "{name} ends with a trailing LF");
        assert_eq!(entry["bytes"].as_u64().unwrap(), body.len() as u64);
        assert_eq!(entry["sha256"].as_str().unwrap(), digest_of(&body));
        let rows = read_json_lines(&out.join(name));
        assert_eq!(entry["rows"].as_u64().unwrap(), rows.len() as u64);
        let key = if name == "groups.jsonl" {
            "group_id"
        } else {
            "example_id"
        };
        let ids: Vec<&str> = rows.iter().map(|r| r[key].as_str().unwrap()).collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "{name} sorted");
        assert!(
            std::fs::read_to_string(out.join(name))
                .unwrap()
                .lines()
                .all(|line| line.len() <= 48 * 1024)
        );
    }
    let report = check(&out, &env.policy).unwrap();
    assert_eq!(report.dataset_id, prepared.dataset_id);
    assert_eq!(
        report.rows,
        prepared.train_rows + prepared.calibration_rows + prepared.evaluation_rows
    );
    assert_eq!(report.groups, 22 + 11 + 21);
    // groups.jsonl carries each example's contribution: id, input digest,
    // label and permission digest, matching the stored row.
    let stored: std::collections::BTreeMap<String, FeedbackRowV4> =
        stored_rows(&env).into_iter().collect();
    for group in read_json_lines(&out.join("groups.jsonl")) {
        for contribution in group["examples"].as_array().unwrap() {
            let row = &stored[contribution["example_id"].as_str().unwrap()];
            assert_eq!(group["group_id"], row.task_group_id.as_str());
            assert_eq!(contribution["input_sha256"], row.input_sha256());
            assert_eq!(
                contribution["correct_option_id"],
                row.correct_option_id.as_str()
            );
            assert_eq!(contribution["permission_sha256"], row.permission_sha256());
        }
    }
    // Every row's tokens equal the pinned renderer's, markers on [MASK].
    let renderer = renderer();
    for row in read_json_lines(&out.join("train.jsonl")) {
        let feedback: FeedbackRowV4 = serde_json::from_value(row["feedback"].clone()).unwrap();
        let exact = renderer
            .render(&feedback.state, feedback.ordered_options())
            .unwrap();
        let ids: Vec<u32> = row["token_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u32)
            .collect();
        assert_eq!(ids, exact.ids);
        assert_eq!(row["input_sha256"], feedback.input_sha256());
        assert_eq!(row["example_id"], feedback.example_id());
    }
}

#[test]
fn check_detects_tampering_truncation_and_substitution() {
    let env = floored_env();
    let out = env.out("dataset");
    env.prepare(&out);
    let check = || check(&out, &env.policy);
    assert!(check().is_ok());

    // A flipped byte inside a data file.
    let path = out.join("calibration.jsonl");
    let original = std::fs::read(&path).unwrap();
    let mut flipped = original.clone();
    flipped[12] ^= 1;
    std::fs::write(&path, &flipped).unwrap();
    assert_eq!(check().unwrap_err().code(), "dataset_invalid");
    std::fs::write(&path, &original).unwrap();
    assert!(check().is_ok());

    // A truncated member: its length no longer matches the manifest.
    let path = out.join("train.jsonl");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, &original[..original.len() - 1]).unwrap();
    assert_eq!(check().unwrap_err().code(), "dataset_invalid");
    std::fs::write(&path, &original).unwrap();

    // Reordered rows.
    let path = out.join("evaluation.jsonl");
    let original = std::fs::read_to_string(&path).unwrap();
    let mut lines: Vec<&str> = original.lines().collect();
    lines.reverse();
    std::fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();
    assert_eq!(check().unwrap_err().code(), "dataset_invalid");
    std::fs::write(&path, &original).unwrap();

    // A data file replaced by a symlink to identical bytes is refused.
    let target = env.out("elsewhere.jsonl");
    std::fs::write(&target, &original).unwrap();
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&target, &path).unwrap();
    assert_eq!(check().unwrap_err().code(), "lineage_missing");
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, &original).unwrap();
    assert!(check().is_ok());

    // A symlinked DATASET DIRECTORY is refused as well: the directory is
    // opened once, without following a link.
    let link = env.out("dataset-link");
    std::os::unix::fs::symlink(&out, &link).unwrap();
    assert_eq!(
        learning::check(
            &link.join("manifest.json"),
            &env.policy,
            &Control::unbounded()
        )
        .unwrap_err()
        .code(),
        "lineage_missing"
    );

    // Forged token ids with a CONSISTENT manifest: the default read-back
    // re-renders every row through the exact renderer and refuses them.
    let train = out.join("train.jsonl");
    let body = std::fs::read_to_string(&train).unwrap();
    let forged = body.replacen("\"token_ids\":[50281,", "\"token_ids\":[50282,", 1);
    assert_ne!(forged, body);
    reseal(&out, "train.jsonl", forged.as_bytes());
    let error = check().unwrap_err();
    assert_eq!(error.code(), "dataset_invalid");
    assert!(error.to_string().contains("exact renderer"), "{error}");
}

#[test]
fn resealed_framing_errors_fail_at_their_own_validators() {
    // Every member hash, size and row count is re-sealed, so only the LF and
    // sort validators can object — not the hash check.
    let env = floored_env();
    let out = env.out("dataset");
    env.prepare(&out);
    let original = std::fs::read(out.join("train.jsonl")).unwrap();
    reseal(&out, "train.jsonl", &original[..original.len() - 1]);
    let error = check(&out, &env.policy).unwrap_err();
    assert_eq!(error.code(), "dataset_invalid");
    assert!(error.to_string().contains("trailing LF"), "{error}");

    let env = floored_env();
    let out = env.out("dataset");
    env.prepare(&out);
    let mut rows = read_json_lines(&out.join("evaluation.jsonl"));
    rows.swap(0, 1);
    reseal(&out, "evaluation.jsonl", json_lines_body(&rows).as_bytes());
    let error = check(&out, &env.policy).unwrap_err();
    assert_eq!(error.code(), "dataset_invalid");
    assert!(error.to_string().contains("not sorted"), "{error}");
}

#[test]
fn the_tokenizer_is_a_prerequisite_of_read_back_never_a_structural_pass() {
    let env = floored_env();
    let out = env.out("dataset");
    env.prepare(&out);
    // The same pinned hashes, but the tokenizer directory is gone.
    let mut policy: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&env.policy).unwrap()).unwrap();
    policy["tokenizer"]["dir"] = env.out("no-tokenizer-here").display().to_string().into();
    let missing = env.out("missing-tokenizer-policy.json");
    std::fs::write(&missing, policy.to_string()).unwrap();
    assert_eq!(
        check(&out, &missing).unwrap_err().code(),
        "artifact_unavailable"
    );
    // The CLI requires the policy: no flag, no structural-only pass.
    let mut command = foundry(&env.store);
    command
        .args(["learning", "check", "--manifest"])
        .arg(out.join("manifest.json"));
    let out_cli = run_bounded(command, b"");
    assert_eq!(out_cli.status.code(), Some(2));
    assert!(out_cli.stdout.is_empty());
}

#[test]
fn output_exists_and_output_in_source_root_refuse() {
    let env = floored_env();
    let existing = env.out("d");
    std::fs::create_dir(&existing).unwrap();
    std::fs::write(existing.join("keep.txt"), b"keep").unwrap();
    let error = env.prepare_err(&existing);
    assert_eq!(error.code(), "output_exists", "{error}");
    assert_eq!(std::fs::read(existing.join("keep.txt")).unwrap(), b"keep");

    // Inside the bound workspace root.
    let root = env
        .engine()
        .workspace_root()
        .map(str::to_owned)
        .expect("root bound");
    let inside = Path::new(&root).join("nested").join("dataset");
    let error = env.prepare_err(&inside);
    assert_eq!(error.code(), "output_in_source_root", "{error}");
    assert!(!Path::new(&root).join("nested").exists());
}

#[test]
fn output_containment_is_judged_through_descriptors_not_pathnames() {
    let env = floored_env();
    let root = PathBuf::from(
        env.engine()
            .workspace_root()
            .map(str::to_owned)
            .expect("root bound"),
    );
    std::fs::create_dir(root.join("sub")).unwrap();
    let listing = |dir: &Path| -> Vec<std::ffi::OsString> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        names
    };
    let before = listing(&root);
    // Symlinked parents: the lexical path never names the root, the
    // directory actually opened is the root or lies under it.
    let into_sub = env.out("link-into-root");
    std::os::unix::fs::symlink(root.join("sub"), &into_sub).unwrap();
    let to_root = env.out("link-to-root");
    std::os::unix::fs::symlink(&root, &to_root).unwrap();
    for out in [into_sub.join("dataset"), to_root.join("dataset")] {
        let error = env.prepare_err(&out);
        assert_eq!(
            error.code(),
            "output_in_source_root",
            "{}: {error}",
            out.display()
        );
    }
    // Relative outputs, resolved against the CLI process's own directory.
    for relative in [
        "ws/dataset",
        "ws/../ws/sub/dataset",
        "link-into-root/dataset",
    ] {
        let mut command = foundry(&env.store);
        command
            .current_dir(env.dir.path())
            .args(["learning", "prepare", "--out", relative, "--policy"])
            .arg(&env.policy);
        let out = run_bounded(command, b"");
        assert_eq!(
            out.status.code(),
            Some(2),
            "{relative}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(error_code(&out), "output_in_source_root", "{relative}");
    }
    // Nothing was created inside the root.
    assert_eq!(listing(&root), before);
    assert!(listing(&root.join("sub")).is_empty());
}

/// The `.learning-partial-*` entries of a directory.
fn partial_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".learning-partial-"))
        .collect();
    names.sort();
    names
}

#[test]
fn a_destination_created_during_preparation_is_never_replaced() {
    use context_foundry::fault::{self, Action};
    use std::os::unix::fs::MetadataExt as _;
    let env = floored_env();
    let out = env.out("dataset");
    // Another process takes the name after the preflight: an EMPTY
    // directory, exactly what an ordinary rename would silently replace.
    let inode = std::rc::Rc::new(std::cell::Cell::new(0u64));
    let (foreign, seen) = (out.clone(), inode.clone());
    fault::arm(
        learning::fault_names::BEFORE_RENAME,
        0,
        Action::Call(Box::new(move |_ctx| {
            std::fs::create_dir(&foreign).unwrap();
            seen.set(std::fs::metadata(&foreign).unwrap().ino());
        })),
    );
    let error = env.prepare_err(&out);
    fault::disarm_all();
    assert_eq!(error.code(), "output_exists", "{error}");
    assert_ne!(inode.get(), 0, "the race really ran");
    assert_eq!(
        std::fs::metadata(&out).unwrap().ino(),
        inode.get(),
        "the foreign directory is the same directory"
    );
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 0, "and untouched");
    assert!(
        partial_names(env.dir.path()).is_empty(),
        "the run's own partial was removed"
    );
    assert!(
        testkit::table_rows(&env.store, "learning_datasets").is_empty(),
        "nothing was recorded as published"
    );

    // A copy of this very dataset that the store already RECORDED appears at
    // a second destination: byte-identical manifest, but not a lost response
    // of this run, so it is never adopted.
    let recorded = env.out("recorded");
    env.prepare(&recorded);
    let second = env.out("second");
    let (copy_from, copy_to) = (recorded.clone(), second.clone());
    fault::arm(
        learning::fault_names::BEFORE_RENAME,
        0,
        Action::Call(Box::new(move |_ctx| {
            std::fs::create_dir(&copy_to).unwrap();
            for name in learning::FILES.iter().chain(&["manifest.json"]) {
                std::fs::copy(copy_from.join(name), copy_to.join(name)).unwrap();
            }
        })),
    );
    let error = env.prepare_err(&second);
    fault::disarm_all();
    assert_eq!(error.code(), "output_exists", "{error}");
    assert_eq!(
        std::fs::read(second.join("manifest.json")).unwrap(),
        std::fs::read(recorded.join("manifest.json")).unwrap()
    );
    assert!(partial_names(env.dir.path()).is_empty());
    assert_eq!(
        testkit::table_rows(&env.store, "learning_datasets").len(),
        1,
        "only the first publication is recorded"
    );
}

#[test]
fn the_publish_source_is_the_held_staged_dataset_never_a_substitute() {
    use context_foundry::fault::{self, Action};
    let env = floored_env();
    // 1. The staged dataset itself is swapped for a foreign directory inside
    // the run's staging directory, just before the rename: refused with
    // `output_ownership` (exit 1), nothing published or recorded, the
    // foreign contents kept, and the moved-away original never a parent.
    let out = env.out("dataset");
    let moved = env.out("moved-away");
    let (staging_parent, moved_to) = (env.dir.path().to_path_buf(), moved.clone());
    fault::arm(
        learning::fault_names::BEFORE_RENAME,
        0,
        Action::Call(Box::new(move |_ctx| {
            let staging = staging_parent.join(&partial_names(&staging_parent)[0]);
            std::fs::rename(staging.join("dataset"), &moved_to).unwrap();
            std::fs::create_dir(staging.join("dataset")).unwrap();
            std::fs::write(staging.join("dataset/foreign.txt"), b"foreign").unwrap();
        })),
    );
    let error = env.prepare_err(&out);
    fault::disarm_all();
    assert_eq!(error.code(), "output_ownership", "{error}");
    assert_eq!(error.exit_code(), 1);
    assert!(!out.exists(), "nothing was published");
    assert!(testkit::table_rows(&env.store, "learning_datasets").is_empty());
    let partials = partial_names(env.dir.path());
    assert_eq!(
        partials.len(),
        1,
        "the staging directory holds foreign contents"
    );
    assert_eq!(
        std::fs::read(
            env.dir
                .path()
                .join(&partials[0])
                .join("dataset/foreign.txt")
        )
        .unwrap(),
        b"foreign"
    );
    assert!(moved.join("manifest.json").exists());
    let error = env.prepare_child_err(&env.out("child"), &moved.join("manifest.json"));
    assert_eq!(error.code(), "lineage_missing", "{error}");
    std::fs::remove_dir_all(env.dir.path().join(&partials[0])).unwrap();

    // 2. The reviewer's reproduction: the staging directory's NAME is moved
    // away and a foreign directory put in its place. The rename resolves its
    // source through the held staging descriptor, so exactly this run's
    // dataset is published and recorded; the foreign directory is untouched.
    let out = env.out("dataset-2");
    let moved = env.out("staging-moved-away");
    let (staging_parent, moved_to) = (env.dir.path().to_path_buf(), moved.clone());
    let foreign_name = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let seen = foreign_name.clone();
    fault::arm(
        learning::fault_names::BEFORE_RENAME,
        0,
        Action::Call(Box::new(move |_ctx| {
            let name = partial_names(&staging_parent)[0].clone();
            let staging = staging_parent.join(&name);
            std::fs::rename(&staging, &moved_to).unwrap();
            std::fs::create_dir_all(staging.join("dataset")).unwrap();
            std::fs::write(staging.join("dataset/foreign.txt"), b"foreign").unwrap();
            *seen.borrow_mut() = name;
        })),
    );
    let prepared = env.prepare(&out);
    fault::disarm_all();
    let foreign = env.dir.path().join(foreign_name.borrow().as_str());
    assert_eq!(
        std::fs::read(foreign.join("dataset/foreign.txt")).unwrap(),
        b"foreign",
        "the foreign directory is untouched"
    );
    assert!(
        !out.join("foreign.txt").exists(),
        "the foreign directory was not published"
    );
    assert_eq!(
        check(&out, &env.policy).unwrap().dataset_id,
        prepared.dataset_id
    );
    let records = testkit::table_rows(&env.store, "learning_datasets");
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].0,
        manifest_sha(&out),
        "the record names the published bytes"
    );
    assert_eq!(
        std::fs::read_dir(&moved).unwrap().count(),
        0,
        "the moved staging directory is empty: its dataset was published"
    );
}

#[test]
fn interruption_leaves_no_output_removes_only_its_partial_and_retry_succeeds() {
    use context_foundry::fault::{self, Action};
    let env = floored_env();
    let out = env.out("dataset");
    let bystander = env.out(".learning-partial-not-ours");
    std::fs::create_dir(&bystander).unwrap();
    std::fs::write(bystander.join("x"), b"x").unwrap();
    for point in [
        learning::fault_names::BEFORE_HISTORY,
        learning::fault_names::BEFORE_RENAME,
    ] {
        fault::arm(point, 0, Action::Fail("injected".into()));
        let error = env.prepare_err(&out);
        fault::disarm_all();
        assert_eq!(error.code(), "internal", "{point}: {error}");
        assert!(!out.exists(), "{point}: no destination after interruption");
        let partials = partial_names(env.dir.path());
        assert_eq!(
            partials,
            vec![".learning-partial-not-ours".to_owned()],
            "{point}: only the run's own partial was removed"
        );
        assert_eq!(std::fs::read(bystander.join("x")).unwrap(), b"x");
    }
    // The retry completes: an interrupted run leaves no lineage that blocks
    // the same input.
    let prepared = env.prepare(&out);
    let report = check(&out, &env.policy).unwrap();
    assert_eq!(report.dataset_id, prepared.dataset_id);
}

#[test]
fn a_cancelled_run_writes_nothing() {
    let env = floored_env();
    let out = env.out("dataset");
    let engine = env.engine();
    let error =
        learning::prepare(&engine, &out, &env.policy, None, &Control::cancelled()).unwrap_err();
    assert_eq!(error.code(), "cancelled");
    assert!(!out.exists());
}

#[test]
fn process_death_at_publication_leaves_no_output_and_no_eligible_partial() {
    use std::os::unix::process::ExitStatusExt as _;
    let env = floored_env();
    let out = env.out("dataset");
    // The fault-arming twin of the CLI dies (abort) at the publish boundary:
    // the partial is complete and the group history committed, nothing is
    // renamed into place.
    let mut command = Command::new(env!("CARGO_BIN_EXE_foundry-faults"));
    command
        .arg("--store")
        .arg(&env.store)
        .args(["learning", "prepare", "--out"])
        .arg(&out)
        .arg("--policy")
        .arg(&env.policy)
        .env(
            "FOUNDRY_TEST_FAULT",
            format!("{}=abort", learning::fault_names::BEFORE_RENAME),
        );
    let died = run_bounded(command, b"");
    assert_eq!(
        died.status.signal(),
        Some(libc::SIGABRT),
        "{:?}",
        died.status
    );
    // Reopened: the store is consistent and nothing was published.
    drop(env.engine());
    assert!(!out.exists(), "no published output");
    let partials = partial_names(env.dir.path());
    assert_eq!(partials.len(), 1, "the dead run's partial: {partials:?}");
    // The staged dataset inside the dead run's staging directory.
    let partial = env.dir.path().join(&partials[0]).join("dataset");
    assert!(partial.join("manifest.json").exists());
    // A partial is never eligible: it was never recorded as published, so it
    // cannot serve as a parent.
    let error = env.prepare_child_err(&env.out("child"), &partial.join("manifest.json"));
    assert_eq!(error.code(), "lineage_missing", "{error}");
    // The retry completes and reads back; the published dataset is a parent.
    let prepared = env.prepare(&out);
    assert_eq!(
        check(&out, &env.policy).unwrap().dataset_id,
        prepared.dataset_id
    );
    match env
        .prepare_with(
            &env.out("next"),
            &env.policy,
            Some(&out.join("manifest.json")),
        )
        .unwrap()
    {
        PrepareOutcome::NoNewData { .. } => {}
        PrepareOutcome::Completed(_) => panic!("nothing new, expected no_new_data"),
    }
}

#[test]
fn a_publication_whose_record_was_lost_is_verified_and_adopted() {
    use std::os::unix::process::ExitStatusExt as _;
    let env = floored_env();
    let out = env.out("dataset");
    let prepare_cli = |policy: &Path, faults: Option<&str>| -> Output {
        let mut command = match faults {
            Some(spec) => {
                let mut command = Command::new(env!("CARGO_BIN_EXE_foundry-faults"));
                command.env("FOUNDRY_TEST_FAULT", spec);
                command
            }
            None => Command::new(env!("CARGO_BIN_EXE_foundry")),
        };
        command
            .arg("--store")
            .arg(&env.store)
            .args(["learning", "prepare", "--out"])
            .arg(&out)
            .arg("--policy")
            .arg(policy);
        run_bounded(command, b"")
    };
    // The process dies after the rename and the parent fsync, before the
    // dataset is recorded: the output is published, the response is lost.
    let died = prepare_cli(
        &env.policy,
        Some(format!("{}=abort", learning::fault_names::AFTER_PUBLISH).as_str()),
    );
    assert_eq!(
        died.status.signal(),
        Some(libc::SIGABRT),
        "{:?}",
        died.status
    );
    let published = std::fs::read(out.join("manifest.json")).expect("published");
    assert!(
        partial_names(env.dir.path()).is_empty(),
        "the partial became the output"
    );
    assert!(testkit::table_rows(&env.store, "learning_datasets").is_empty());
    // Not yet a dataset of this store: refused as a parent.
    let error = env.prepare_child_err(&env.out("child"), &out.join("manifest.json"));
    assert_eq!(error.code(), "lineage_missing", "{error}");
    // A DIFFERENT preparation (another seed, so other manifest bytes) finds
    // the unrecorded output and leaves it alone.
    let other = write_policy(env.dir.path(), "seed-other");
    let refused = prepare_cli(&other, None);
    assert_eq!(refused.status.code(), Some(2));
    assert_eq!(error_code(&refused), "output_exists");
    assert_eq!(std::fs::read(out.join("manifest.json")).unwrap(), published);
    assert!(partial_names(env.dir.path()).is_empty());
    // The SAME preparation with the published output damaged under its
    // unchanged manifest: a truncated member, then a flipped row byte (not
    // resealed). Identical manifest bytes are not verification; the full
    // read-back refuses, and the damaged output is left exactly as found.
    for (member, damage) in [
        (
            "train.jsonl",
            (|body: &mut Vec<u8>| {
                body.pop();
            }) as fn(&mut Vec<u8>),
        ),
        ("calibration.jsonl", |body: &mut Vec<u8>| body[12] ^= 1),
    ] {
        let path = out.join(member);
        let original = std::fs::read(&path).unwrap();
        let mut damaged = original.clone();
        damage(&mut damaged);
        std::fs::write(&path, &damaged).unwrap();
        let refused = prepare_cli(&env.policy, None);
        assert_eq!(refused.status.code(), Some(2), "{member}");
        assert_eq!(error_code(&refused), "output_exists", "{member}");
        assert_eq!(std::fs::read(&path).unwrap(), damaged, "{member} untouched");
        assert_eq!(std::fs::read(out.join("manifest.json")).unwrap(), published);
        assert!(partial_names(env.dir.path()).is_empty(), "{member}");
        assert!(testkit::table_rows(&env.store, "learning_datasets").is_empty());
        std::fs::write(&path, &original).unwrap();
    }
    // Rerunning the SAME preparation verifies the output and adopts it.
    let rerun = prepare_cli(&env.policy, None);
    assert_eq!(
        rerun.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&rerun.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&rerun.stdout).unwrap();
    assert_eq!(report["outcome"], "completed");
    assert_eq!(report["adopted"], true);
    let manifest: serde_json::Value = serde_json::from_slice(&published).unwrap();
    assert_eq!(report["dataset_id"], manifest["dataset_id"]);
    assert_eq!(
        std::fs::read(out.join("manifest.json")).unwrap(),
        published,
        "the published output is untouched"
    );
    assert!(
        partial_names(env.dir.path()).is_empty(),
        "the rerun removed only its own partial"
    );
    assert_eq!(
        testkit::table_rows(&env.store, "learning_datasets"),
        vec![(
            digest_of(&published),
            json!({"dataset_id": manifest["dataset_id"], "parent_manifest_sha256": null})
                .to_string()
        )]
    );
    check(&out, &env.policy).unwrap();
    // Now it is a dataset of this store: usable as a parent.
    match env
        .prepare_with(
            &env.out("next"),
            &env.policy,
            Some(&out.join("manifest.json")),
        )
        .unwrap()
    {
        PrepareOutcome::NoNewData {
            parent_manifest_sha256,
        } => assert_eq!(parent_manifest_sha256, digest_of(&published)),
        PrepareOutcome::Completed(_) => panic!("nothing new, expected no_new_data"),
    }
}

// ---------------------------------------------------------------------------
// Lineage: parents, replay, no_new_data and base validity
// ---------------------------------------------------------------------------

#[test]
fn zero_new_rows_with_a_valid_base_is_no_new_data() {
    let env = floored_env();
    let first = env.out("d1");
    env.prepare(&first);
    let second = env.out("d2");
    match env
        .prepare_with(
            &second,
            &env.policy,
            Some(first.join("manifest.json").as_path()),
        )
        .unwrap()
    {
        PrepareOutcome::NoNewData {
            parent_manifest_sha256,
        } => {
            assert_eq!(
                parent_manifest_sha256,
                digest_of(&std::fs::read(first.join("manifest.json")).unwrap())
            );
        }
        PrepareOutcome::Completed(_) => panic!("nothing new, expected no_new_data"),
    }
    assert!(!second.exists(), "no_new_data writes nothing");
}

#[test]
fn a_withdrawal_is_not_hidden_by_no_new_data() {
    let env = floored_env();
    let first = env.out("d1");
    env.prepare(&first);
    let rows: Vec<_> = {
        let engine = env.engine();
        engine
            .learning_feedback_rows(&Control::unbounded())
            .unwrap()
    };
    let (_, victim) = rows.first().unwrap();
    let mut withdrawn = serde_json::to_value(victim).unwrap();
    withdrawn["allow_training"] = false.into();
    env.record(&[withdrawn.to_string()]);
    // No NEW rows exist, yet the base problem is named — not no_new_data.
    let error = env.prepare_child_err(&env.out("d2"), &first.join("manifest.json"));
    assert_eq!(error.code(), "base_permission_changed", "{error}");
}

#[test]
fn a_correction_of_a_base_label_is_named_before_novelty() {
    let env = floored_env();
    let first = env.out("d1");
    env.prepare(&first);
    let rows: Vec<_> = {
        let engine = env.engine();
        engine
            .learning_feedback_rows(&Control::unbounded())
            .unwrap()
    };
    let (_, victim) = rows.first().unwrap();
    let mut corrected = serde_json::to_value(victim).unwrap();
    corrected["correct_option_id"] = if victim.correct_option_id == "search" {
        "graph"
    } else {
        "search"
    }
    .into();
    // Plus a brand-new row, so novelty alone would proceed.
    env.record(&[
        corrected.to_string(),
        simple_row("brand-new", "brand-new-group", "search"),
    ]);
    let second = env.out("d2");
    let error = env.prepare_child_err(&second, &first.join("manifest.json"));
    assert_eq!(error.code(), "base_permission_changed", "{error}");
    assert!(!second.exists());
}

#[test]
fn a_changed_rights_assertion_alone_changes_the_base_permission() {
    let env = floored_env();
    let first = env.out("d1");
    env.prepare(&first);
    let (_, victim) = stored_rows(&env).into_iter().next().unwrap();
    // Same input, label and consent: only the rights assertion changed.
    env.record(&[with_field(
        &victim,
        "rights_ref",
        "rights-reassessed".into(),
    )]);
    let error = env.prepare_child_err(&env.out("d2"), &first.join("manifest.json"));
    assert_eq!(error.code(), "base_permission_changed", "{error}");
    assert!(error.to_string().contains("permission"), "{error}");
}

/// Two rounds: D1 over the floored rows, then D2 after one new input in an
/// old train group, whose train file holds only that row plus one replay.
fn two_rounds(env: &Env) -> (PathBuf, PathBuf) {
    let first = env.out("d1");
    env.prepare(&first);
    env.record(&[raw_row(
        "round-two",
        &floor_groups()[0][0],
        "graph",
        ["search", "graph"],
        "a second-round input\ngraph: complete",
    )]);
    let second = env.out("d2");
    let child = env.prepare_child(&second, &first.join("manifest.json"));
    assert_eq!(
        (child.new_rows, child.replay_rows, child.train_rows),
        (1, 1, 2)
    );
    (first, second)
}

#[test]
fn a_third_round_with_nothing_new_is_no_new_data_without_old_directories() {
    let env = floored_env();
    let (first, second) = two_rounds(&env);
    // D2's train file is new + bounded replay; its coverage is cumulative.
    assert_eq!(file_example_ids(&second.join("train.jsonl")).len(), 2);
    assert_eq!(coverage_ids(&second).len(), 22 + 11 + 21 + 1);
    // The grandparent's directory need not stay on disk; its history does.
    std::fs::remove_dir_all(&first).unwrap();
    let third = env.out("d3");
    match env
        .prepare_with(&third, &env.policy, Some(&second.join("manifest.json")))
        .unwrap()
    {
        PrepareOutcome::NoNewData {
            parent_manifest_sha256,
        } => assert_eq!(parent_manifest_sha256, manifest_sha(&second)),
        PrepareOutcome::Completed(prepared) => panic!(
            "inherited inputs were rediscovered as {} new rows",
            prepared.new_rows
        ),
    }
    assert!(!third.exists());
}

#[test]
fn inherited_contributors_omitted_from_the_train_file_are_still_validated() {
    let env = floored_env();
    let (first, second) = two_rounds(&env);
    let materialized = file_example_ids(&second.join("train.jsonl"));
    let rows = stored_rows(&env);
    let (omitted_id, omitted) = rows
        .iter()
        .find(|(id, row)| {
            learning::split_of(&row.task_group_id) == "train" && !materialized.contains(id)
        })
        .expect("a D1 train contributor that D2's train file omits");
    assert!(file_example_ids(&first.join("train.jsonl")).contains(omitted_id));
    assert!(coverage_ids(&second).contains(omitted_id));
    let parent = second.join("manifest.json");
    let original = serde_json::to_string(omitted).unwrap();
    let flipped = if omitted.correct_option_id == "search" {
        "graph"
    } else {
        "search"
    };
    for (change, raw) in [
        (
            "label correction",
            with_field(omitted, "correct_option_id", flipped.into()),
        ),
        (
            "withdrawal",
            with_field(omitted, "allow_training", false.into()),
        ),
        (
            "rights assertion",
            with_field(omitted, "rights_ref", "rights-reassessed".into()),
        ),
    ] {
        env.record(&[raw]);
        let error = env.prepare_child_err(&env.out("d3"), &parent);
        assert_eq!(error.code(), "base_permission_changed", "{change}: {error}");
        assert!(
            error.to_string().contains(omitted_id.as_str()),
            "{change}: {error}"
        );
        // Restoring the exact row restores the valid base.
        env.record(std::slice::from_ref(&original));
        assert!(
            matches!(
                env.prepare_with(&env.out("d3"), &env.policy, Some(&parent))
                    .unwrap(),
                PrepareOutcome::NoNewData { .. }
            ),
            "{change} restored"
        );
    }
    assert!(!env.out("d3").exists());
}

#[test]
fn ancestor_missing_refuses_with_lineage_missing() {
    let env = floored_env();
    let first = env.out("d1");
    env.prepare(&first);
    // A member file is gone.
    std::fs::remove_file(first.join("groups.jsonl")).unwrap();
    let error = env.prepare_child_err(&env.out("d2"), &first.join("manifest.json"));
    assert_eq!(error.code(), "lineage_missing", "{error}");
    // A parent manifest that does not exist at all.
    let error = env.prepare_child_err(&env.out("d3"), &env.out("nope").join("manifest.json"));
    assert_eq!(error.code(), "lineage_missing", "{error}");
    assert!(!env.out("d2").exists() && !env.out("d3").exists());
}

#[test]
fn a_missing_ancestor_record_is_lineage_missing() {
    let env = floored_env();
    let (first, second) = two_rounds(&env);
    env.record(&[simple_row("round-three", "round-three-group", "graph")]);
    // The grandparent's recorded history is gone; its directory is not.
    testkit::set_learning_row(&env.store, "learning_datasets", &manifest_sha(&first), None);
    let error = env.prepare_child_err(&env.out("d3"), &second.join("manifest.json"));
    assert_eq!(error.code(), "lineage_missing", "{error}");
    assert!(error.to_string().contains(&manifest_sha(&first)), "{error}");
    // A parent the store has no record of is not a dataset it published.
    testkit::set_learning_row(
        &env.store,
        "learning_datasets",
        &manifest_sha(&second),
        None,
    );
    let error = env.prepare_child_err(&env.out("d3"), &second.join("manifest.json"));
    assert_eq!(error.code(), "lineage_missing", "{error}");
    assert!(!env.out("d3").exists());
}

#[test]
fn a_parent_from_another_workspace_or_function_is_refused() {
    // A genuine dataset from ANOTHER workspace.
    let env = floored_env();
    let other = floored_env();
    let foreign = other.out("d1");
    other.prepare(&foreign);
    env.record(&[simple_row("brand-new", "brand-new-group", "graph")]);
    let error = env.prepare_child_err(&env.out("d2"), &foreign.join("manifest.json"));
    assert_eq!(error.code(), "lineage_missing", "{error}");

    // A genuine dataset from a DIFFERENT model function: the same
    // workspace, a tokenizer config that hashes differently.
    let env = floored_env();
    let alt = env.out("alt-tokenizer");
    std::fs::create_dir(&alt).unwrap();
    let tokenizer = tokenizer_dir();
    std::fs::copy(tokenizer.join("tokenizer.json"), alt.join("tokenizer.json")).unwrap();
    let mut config = std::fs::read(tokenizer.join("tokenizer_config.json")).unwrap();
    config.push(b'\n');
    std::fs::write(alt.join("tokenizer_config.json"), &config).unwrap();
    let alt_policy = env.out("alt-policy.json");
    let policy = policy_value(
        "seed-alpha",
        &alt,
        &digest_of(&std::fs::read(alt.join("tokenizer.json")).unwrap()),
        &digest_of(&config),
    );
    std::fs::write(&alt_policy, policy.to_string()).unwrap();
    let first = env.out("d1");
    match env.prepare_with(&first, &alt_policy, None).unwrap() {
        PrepareOutcome::Completed(_) => {}
        PrepareOutcome::NoNewData { .. } => panic!("expected completion"),
    }
    env.record(&[simple_row("brand-new", "brand-new-group", "graph")]);
    let error = env.prepare_child_err(&env.out("d2"), &first.join("manifest.json"));
    assert_eq!(error.code(), "base_model_changed", "{error}");
}

#[test]
fn new_input_in_an_old_group_extends_the_dataset_with_bounded_replay() {
    let env = floored_env();
    let first = env.out("d1");
    let base = env.prepare(&first);
    let groups = read_json_lines(&first.join("groups.jsonl"));
    let train_group = groups.iter().find(|g| g["split"] == "train").unwrap()["group_id"]
        .as_str()
        .unwrap()
        .to_owned();
    env.record(&[raw_row(
        "new-in-old",
        &train_group,
        "graph",
        ["search", "graph"],
        "a new input in an old group\ngraph: complete",
    )]);
    let second = env.out("d2");
    let child = env.prepare_child(&second, &first.join("manifest.json"));
    assert_eq!(child.new_rows, 1);
    assert_eq!(child.replay_rows, 1, "one replay row per new row");
    assert_eq!(child.train_rows, 2, "new rows plus bounded replay");
    assert_ne!(base.dataset_id, child.dataset_id);
    // Held-out files keep the full current coverage.
    assert_eq!(child.calibration_rows, base.calibration_rows);
    assert_eq!(child.evaluation_rows, base.evaluation_rows);
    // The old group stays in its split and now lists the new example.
    let groups = read_json_lines(&second.join("groups.jsonl"));
    let group = groups
        .iter()
        .find(|g| g["group_id"] == train_group.as_str())
        .unwrap();
    assert_eq!(group["split"], "train");
    // Replay is chosen by increasing SHA256([seed,id]) over unchanged
    // permitted base TRAIN rows, never held-out ones.
    let base_train: Vec<String> = read_json_lines(&first.join("train.jsonl"))
        .iter()
        .map(|r| r["example_id"].as_str().unwrap().to_owned())
        .collect();
    let mut expected: Vec<String> = base_train.clone();
    expected
        .sort_by_key(|id| digest_of(serde_json::json!(["seed-alpha", id]).to_string().as_bytes()));
    let child_train: Vec<String> = read_json_lines(&second.join("train.jsonl"))
        .iter()
        .map(|r| r["example_id"].as_str().unwrap().to_owned())
        .collect();
    let replayed: Vec<&String> = child_train
        .iter()
        .filter(|id| base_train.contains(id))
        .collect();
    assert_eq!(replayed, vec![&expected[0]]);
    // The manifest binds the parent by its exact bytes.
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(second.join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest["parent_manifest_sha256"],
        digest_of(&std::fs::read(first.join("manifest.json")).unwrap())
    );
    let report = check(&second, &env.policy).unwrap();
    assert_eq!(
        report.rows,
        child.train_rows + child.calibration_rows + child.evaluation_rows
    );
    // Groups, not examples: the same 54 groups now cover 55 examples.
    assert_eq!(report.groups, 22 + 11 + 21);
    assert_eq!(coverage_ids(&second).len(), 22 + 11 + 21 + 1);
}

#[test]
fn a_history_split_that_disagrees_with_the_rule_refuses() {
    let env = floored_env();
    env.prepare(&env.out("d1"));
    let history = testkit::table_rows(&env.store, "learning_history");
    let (group_id, raw) = history.first().unwrap();
    let mut value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let flipped = if value["split"] == "train" {
        "evaluation"
    } else {
        "train"
    };
    value["split"] = flipped.into();
    testkit::set_learning_row(
        &env.store,
        "learning_history",
        group_id,
        Some(&value.to_string()),
    );
    let error = env.prepare_err(&env.out("d2"));
    assert_eq!(error.code(), "split_conflict", "{error}");
    assert!(!env.out("d2").exists());
    // With the untouched D1 as parent and nothing new, the conflict is still
    // named — never hidden behind `no_new_data`.
    let error = env.prepare_child_err(&env.out("d2"), &env.out("d1").join("manifest.json"));
    assert_eq!(error.code(), "split_conflict", "{error}");
    assert!(!env.out("d2").exists());
}

#[test]
fn a_parent_whose_group_split_breaks_the_rule_is_an_invalid_dataset() {
    let env = floored_env();
    let first = env.out("d1");
    env.prepare(&first);
    // Rewrite one group's split and re-seal the manifest's file entry, so
    // only the split rule can object.
    let path = first.join("groups.jsonl");
    let mut groups = read_json_lines(&path);
    let flipped = if groups[0]["split"] == "train" {
        "evaluation"
    } else {
        "train"
    };
    groups[0]["split"] = flipped.into();
    let body: String = groups.iter().map(|g| format!("{g}\n")).collect();
    std::fs::write(&path, &body).unwrap();
    let manifest_path = first.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    for entry in manifest["files"].as_array_mut().unwrap() {
        if entry["name"] == "groups.jsonl" {
            entry["sha256"] = digest_of(body.as_bytes()).into();
            entry["bytes"] = body.len().into();
        }
    }
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();
    env.record(&[simple_row("brand-new", "brand-new-group", "graph")]);
    let error = env.prepare_child_err(&env.out("d2"), &manifest_path);
    assert_eq!(error.code(), "dataset_invalid", "{error}");
    assert!(error.to_string().contains("split rule"), "{error}");
    assert_eq!(
        check(&first, &env.policy).unwrap_err().code(),
        "dataset_invalid"
    );
}

#[test]
fn a_forged_parent_is_refused_before_no_new_data() {
    let env = floored_env();
    let first = env.out("d1");
    env.prepare(&first);
    let body = std::fs::read_to_string(first.join("train.jsonl")).unwrap();
    let forged = body.replacen("\"token_ids\":[50281,", "\"token_ids\":[50282,", 1);
    assert_ne!(forged, body);
    reseal(&first, "train.jsonl", forged.as_bytes());
    // Nothing new: a base read back only structurally would be `no_new_data`.
    let error = env.prepare_child_err(&env.out("d2"), &first.join("manifest.json"));
    assert_eq!(error.code(), "dataset_invalid", "{error}");
    assert!(error.to_string().contains("exact renderer"), "{error}");
}

#[test]
fn a_resealed_swap_of_two_groups_coverage_is_refused() {
    let env = floored_env();
    let first = env.out("d1");
    env.prepare(&first);
    let mut groups = read_json_lines(&first.join("groups.jsonl"));
    let train: Vec<usize> = groups
        .iter()
        .enumerate()
        .filter(|(_, group)| group["split"] == "train")
        .map(|(i, _)| i)
        .take(2)
        .collect();
    let (a, b) = (train[0], train[1]);
    let swapped = groups[a]["examples"].clone();
    groups[a]["examples"] = groups[b]["examples"].clone();
    groups[b]["examples"] = swapped;
    reseal(&first, "groups.jsonl", json_lines_body(&groups).as_bytes());
    // Every group keeps its split, every id is listed once and the held-out
    // unions are unchanged: only the group OWNERSHIP is wrong.
    let error = check(&first, &env.policy).unwrap_err();
    assert_eq!(error.code(), "dataset_invalid");
    assert!(error.to_string().contains("lists it under"), "{error}");
    env.record(&[simple_row("brand-new", "brand-new-group", "graph")]);
    let error = env.prepare_child_err(&env.out("d2"), &first.join("manifest.json"));
    assert_eq!(error.code(), "dataset_invalid", "{error}");
}

#[test]
fn an_empty_resealed_base_is_refused_not_no_new_data() {
    // A correctly hashed schema-4 manifest for THIS workspace, model
    // function and policy with four empty files: never a valid base.
    let env = Env::new();
    let tokenizer = tokenizer_dir();
    let (policy, _) = learning::LearningPolicy::load(&env.policy).unwrap();
    let function = decision_model::model_function_sha256(
        &digest_of(&std::fs::read(tokenizer.join("tokenizer.json")).unwrap()),
        &digest_of(&std::fs::read(tokenizer.join("tokenizer_config.json")).unwrap()),
        SpecialIds::PINNED,
        &policy.model,
    );
    let workspace_id = env.engine().workspace_id().unwrap();
    let policy_sha = digest_of(&std::fs::read(&env.policy).unwrap());
    let empty = digest_of(b"");
    let dataset_id = digest_of(
        serde_json::to_string(&json!([
            workspace_id,
            null,
            function,
            policy_sha,
            empty,
            empty,
            empty
        ]))
        .unwrap()
        .as_bytes(),
    );
    let base = env.out("empty-base");
    std::fs::create_dir(&base).unwrap();
    let mut files = Vec::new();
    for name in learning::FILES {
        std::fs::write(base.join(name), b"").unwrap();
        files.push(json!({"name": name, "sha256": empty, "bytes": 0, "rows": 0}));
    }
    let manifest = json!({
        "schema": 4,
        "recipe": learning::RECIPE,
        "workspace_id": workspace_id,
        "dataset_id": dataset_id,
        "parent_manifest_sha256": null,
        "model_function_sha256": function,
        "policy_sha256": policy_sha,
        "base_candidate_sha256": null,
        "files": files,
        "split_group_counts": {},
    });
    std::fs::write(base.join("manifest.json"), manifest.to_string()).unwrap();
    assert_eq!(
        check(&base, &env.policy).unwrap_err().code(),
        "group_floors"
    );
    // In this empty store, accepting that base would have been `no_new_data`.
    let error = env.prepare_child_err(&env.out("d"), &base.join("manifest.json"));
    assert_eq!(error.code(), "group_floors", "{error}");
    assert!(!env.out("d").exists());
}

#[test]
fn dataset_identity_binds_policy_and_parent() {
    let env = floored_env();
    let a = env.prepare(&env.out("d1"));
    // A different policy (seed) changes the identity.
    let other = write_policy(env.dir.path(), "seed-beta");
    let c = match env
        .prepare_with(&env.out("d2"), &other, None)
        .expect("prepares")
    {
        PrepareOutcome::Completed(c) => *c,
        PrepareOutcome::NoNewData { .. } => panic!("expected completion"),
    };
    assert_ne!(a.dataset_id, c.dataset_id);
    // A child dataset binds its parent's manifest bytes.
    env.record(&[simple_row("brand-new", "brand-new-group", "graph")]);
    let child = env.prepare_child(&env.out("d3"), &env.out("d1").join("manifest.json"));
    assert_ne!(child.dataset_id, a.dataset_id);
}

#[test]
fn the_policy_pins_the_tokenizer_by_hash() {
    let env = floored_env();
    let mut policy: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&env.policy).unwrap()).unwrap();
    policy["tokenizer"]["json_sha256"] = "0".repeat(64).into();
    let bad = env.out("bad-policy.json");
    std::fs::write(&bad, policy.to_string()).unwrap();
    let error = env.prepare_with(&env.out("d"), &bad, None).unwrap_err();
    assert_eq!(error.code(), "tokenizer_mismatch", "{error}");
    assert!(!env.out("d").exists());
}

#[test]
fn nothing_but_permitted_feedback_reaches_a_dataset() {
    // Sources, memory, legacy feedback and the semantic tables all hold
    // distinguishable sentinel text. Preparation reads feedback and
    // permission only: no sentinel reaches a dataset byte and none of those
    // tables changes.
    const SENTINEL: &str = "SENTINEL_7f3a9c";
    let env = floored_env();
    {
        let engine = env.engine();
        engine
            .replace_source(
                "notes.md",
                &format!("# {SENTINEL} heading\n\n{SENTINEL} body\n"),
            )
            .unwrap();
        engine
            .memory_put(&context_foundry::memory::PutInput {
                fields: context_foundry::memory::RecordFields {
                    id: "sentinel-note".into(),
                    text: format!("{SENTINEL} memory"),
                    author: "tests".into(),
                    provenance: "tests".into(),
                    source_links: vec![],
                },
                workspace_id: engine.workspace_id().unwrap(),
            })
            .unwrap();
        let legacy: context_foundry::laya::Feedback = serde_json::from_value(json!({
            "task_id": format!("{SENTINEL}-legacy"),
            "query": format!("{SENTINEL} legacy question"),
            "correct_strategy": "graph",
            "label_source": "operator",
            "allow_training": true,
        }))
        .unwrap();
        engine.record_feedback(&legacy).unwrap();
    }
    testkit::write_raw_partition(
        &env.store,
        "notes.md",
        &json!({ "sentinel": SENTINEL }).to_string(),
    );
    testkit::tamper_semantic_cache_row(&env.store, "sentinel-entry", SENTINEL.as_bytes());
    let before = testkit::snapshot(&env.store);
    let cache_before = testkit::semantic_cache_rows(&env.store);
    let out = env.out("dataset");
    env.prepare(&out);
    let after = testkit::snapshot(&env.store);
    for (table, rows) in &before {
        if *table != "learning_history" && *table != "learning_datasets" {
            assert_eq!(&after[table], rows, "{table} unchanged");
        }
    }
    assert_eq!(testkit::semantic_cache_rows(&env.store), cache_before);
    for name in learning::FILES.iter().chain(&["manifest.json"]) {
        let body = std::fs::read(out.join(name)).unwrap();
        assert!(
            !body
                .windows(SENTINEL.len())
                .any(|window| window == SENTINEL.as_bytes()),
            "{name} carries sentinel text"
        );
    }
}

#[test]
fn oversized_inputs_are_refused_by_length_before_they_are_read() {
    // Sparse files: refusing one costs nothing, reading one would not.
    const HUGE: u64 = 1 << 34;
    let sparse = |path: &Path| std::fs::File::create(path).unwrap().set_len(HUGE).unwrap();
    let env = floored_env();
    let out = env.out("dataset");
    env.prepare(&out);
    let policy = env.out("huge-policy.json");
    sparse(&policy);
    assert_eq!(
        learning::LearningPolicy::load(&policy).unwrap_err().code(),
        "policy_invalid"
    );
    let manifest_path = out.join("manifest.json");
    let original = std::fs::read(&manifest_path).unwrap();
    sparse(&manifest_path);
    assert_eq!(
        check(&out, &env.policy).unwrap_err().code(),
        "dataset_bounds"
    );
    std::fs::write(&manifest_path, &original).unwrap();
    // A member grown far past its unchanged manifest entry.
    let member = out.join("calibration.jsonl");
    let body = std::fs::read(&member).unwrap();
    sparse(&member);
    assert_eq!(
        check(&out, &env.policy).unwrap_err().code(),
        "dataset_invalid"
    );
    std::fs::write(&member, &body).unwrap();
    assert!(check(&out, &env.policy).is_ok());
    // A manifest DECLARING an oversized member, or more than 100000 rows or
    // groups, is refused from the declaration alone.
    let manifest: serde_json::Value = serde_json::from_slice(&original).unwrap();
    for (name, field, value) in [
        ("train.jsonl", "bytes", HUGE),
        ("train.jsonl", "rows", 100_001),
        ("groups.jsonl", "rows", 100_001),
    ] {
        let mut changed = manifest.clone();
        for entry in changed["files"].as_array_mut().unwrap() {
            if entry["name"] == name {
                entry[field] = value.into();
            }
        }
        std::fs::write(&manifest_path, changed.to_string()).unwrap();
        assert_eq!(
            check(&out, &env.policy).unwrap_err().code(),
            "dataset_bounds",
            "{name} {field} {value}"
        );
    }
    std::fs::write(&manifest_path, &original).unwrap();
    assert!(check(&out, &env.policy).is_ok());
}

#[test]
fn read_back_enforces_the_byte_bounds_and_strict_manifest_keys() {
    let env = floored_env();
    let out = env.out("dataset");
    env.prepare(&out);
    let manifest_path = out.join("manifest.json");
    let check = || check(&out, &env.policy);
    assert!(check().is_ok());
    let original = std::fs::read_to_string(&manifest_path).unwrap();

    // A manifest over 1 MiB is refused by size before it is parsed.
    std::fs::write(
        &manifest_path,
        format!("{original}{}", " ".repeat(1024 * 1024)),
    )
    .unwrap();
    assert_eq!(check().unwrap_err().code(), "dataset_bounds");
    std::fs::write(&manifest_path, &original).unwrap();

    // Unknown, duplicate and traversal-bearing manifest content.
    let mut value: serde_json::Value = serde_json::from_str(&original).unwrap();
    value["extra"] = 1.into();
    std::fs::write(&manifest_path, value.to_string()).unwrap();
    assert_eq!(check().unwrap_err().code(), "dataset_invalid");
    let duplicated = original.replacen("{", "{\"schema\":4,", 1);
    std::fs::write(&manifest_path, duplicated).unwrap();
    assert_eq!(check().unwrap_err().code(), "dataset_invalid");
    let mut value: serde_json::Value = serde_json::from_str(&original).unwrap();
    value["files"][0]["name"] = "../escape.jsonl".into();
    std::fs::write(&manifest_path, value.to_string()).unwrap();
    assert_eq!(check().unwrap_err().code(), "dataset_invalid");
    let mut value: serde_json::Value = serde_json::from_str(&original).unwrap();
    value["policy_sha256"] = serde_json::Value::Null;
    std::fs::write(&manifest_path, value.to_string()).unwrap();
    assert_eq!(check().unwrap_err().code(), "dataset_invalid");
    std::fs::write(&manifest_path, &original).unwrap();
    assert!(check().is_ok());

    // JSONL rows: exactly 48 KiB with the LF is within the bound (and then
    // fails as JSON); one byte more is refused by the bound.
    let at_limit = format!("{}\n", "x".repeat(48 * 1024 - 1));
    reseal(&out, "train.jsonl", at_limit.as_bytes());
    assert_eq!(check().unwrap_err().code(), "dataset_invalid");
    let over = format!("{}\n", "x".repeat(48 * 1024));
    reseal(&out, "train.jsonl", over.as_bytes());
    assert_eq!(check().unwrap_err().code(), "dataset_bounds");
}

#[test]
fn the_operator_cli_records_prepares_and_checks_end_to_end() {
    // The real binary at every step: `feedback v4` — the only path that can
    // grant training consent — records the rows, `learning prepare` freezes
    // them and `learning check` reads the dataset back.
    let env = Env::new();
    let mut rows = Vec::new();
    for (split, groups) in ["t", "c", "e"].iter().zip(floor_groups()) {
        for (i, group) in groups.iter().enumerate() {
            let label = if i % 2 == 0 { "search" } else { "graph" };
            rows.push(simple_row(&format!("{split}{i}"), group, label));
        }
    }
    let feedback = |raw: &str| -> serde_json::Value {
        let mut command = foundry(&env.store);
        command.args(["feedback", "v4"]);
        let out = run_bounded(command, raw.as_bytes());
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let mut ids = Vec::new();
    for raw in &rows {
        let recorded = feedback(raw);
        assert_eq!(recorded["status"], "created");
        ids.push(recorded["example_id"].as_str().unwrap().to_owned());
    }
    assert_eq!(feedback(&rows[0])["status"], "unchanged");
    ids.sort();
    let stored: Vec<String> = stored_rows(&env).into_iter().map(|(id, _)| id).collect();
    assert_eq!(stored, ids, "exactly the CLI-recorded rows");

    let dataset = env.out("dataset");
    let mut command = foundry(&env.store);
    command
        .args(["learning", "prepare", "--out"])
        .arg(&dataset)
        .arg("--policy")
        .arg(&env.policy);
    let prepared = run_bounded(command, b"");
    assert_eq!(
        prepared.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let prepared: serde_json::Value = serde_json::from_slice(&prepared.stdout).unwrap();
    assert_eq!(prepared["outcome"], "completed");
    assert_eq!(prepared["adopted"], false);
    assert_eq!(prepared["groups"], 22 + 11 + 21);
    let mut covered = coverage_ids(&dataset);
    covered.sort();
    assert_eq!(covered, ids, "every recorded row is covered");

    let mut command = foundry(&env.store);
    command
        .args(["learning", "check", "--manifest"])
        .arg(dataset.join("manifest.json"))
        .arg("--policy")
        .arg(&env.policy);
    let checked = run_bounded(command, b"");
    assert_eq!(
        checked.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(report["dataset_id"], prepared["dataset_id"]);
    assert_eq!(report["rows"], 22 + 11 + 21);
    assert_eq!(report["groups"], 22 + 11 + 21);

    // A repeat round with nothing new is the named no-op, exit 0.
    let mut command = foundry(&env.store);
    command
        .args(["learning", "prepare", "--out"])
        .arg(env.out("again"))
        .arg("--policy")
        .arg(&env.policy)
        .arg("--parent")
        .arg(dataset.join("manifest.json"));
    let again = run_bounded(command, b"");
    assert_eq!(again.status.code(), Some(0));
    let again: serde_json::Value = serde_json::from_slice(&again.stdout).unwrap();
    assert_eq!(again["outcome"], "no_new_data");
    assert_eq!(again["parent_manifest_sha256"], manifest_sha(&dataset));
    assert!(!env.out("again").exists());
}

#[test]
fn a_schema_6_store_missing_a_learning_table_is_corrupt() {
    for table in testkit::LEARNING_TABLES {
        let env = Env::new();
        testkit::write_store(&env.store, |tx| {
            tx.delete_table(redb::TableDefinition::<&str, &str>::new(table))
                .unwrap();
        });
        let Err(error) = Engine::open_existing(&env.store) else {
            panic!("{table}: a store missing it must be refused");
        };
        assert_eq!(error.code(), "corrupt_store", "{table}: {error}");
    }
}

// ---------------------------------------------------------------------------
// State composition: the one composer, over the real index and importer
// ---------------------------------------------------------------------------

const RA: &str = "rust-analyzer";
/// A producer with no profile: a document its artifact lacks is UNKNOWN, so
/// its import coverage can only be `partial`.
const UNPROFILED: &str = "scip-other";
const DEF: i32 = 1;

/// Four one-function sources whose names make the two-tier ranking
/// unambiguous: a query naming a function finds that definition (tier 1,
/// ordered by path), and no other unit contains the name.
const SOURCES: [(&str, &str); 4] = [
    ("src/a.rs", "pub fn alpha_one() {}\n"),
    ("src/b.rs", "pub fn alpha_two() {}\n"),
    ("src/c.rs", "pub fn alpha_three() {}\n"),
    ("src/d.rs", "pub fn alpha_four() {}\n"),
];

fn occurrence(range: &[i32], symbol: &str, roles: i32) -> Occurrence {
    let mut occurrence = Occurrence::new();
    occurrence.range = range.to_vec();
    occurrence.symbol = symbol.to_owned();
    occurrence.symbol_roles = roles;
    occurrence
}

fn scip_document(path: &str, occurrences: Vec<Occurrence>) -> Document {
    let mut document = Document::new();
    document.relative_path = path.to_owned();
    document.language = "rust".to_owned();
    document.position_encoding =
        EnumOrUnknown::new(PositionEncoding::UTF8CodeUnitOffsetFromLineStart);
    document.occurrences = occurrences;
    document
}

/// A store with indexed sources, driven through the real lexical index and
/// the real SCIP importer.
struct Graph {
    dir: tempfile::TempDir,
    store: PathBuf,
    engine: Engine,
    paths: Vec<String>,
}

impl Graph {
    fn new() -> Self {
        Self::with_sources(&SOURCES)
    }

    fn with_sources(sources: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().join("ws");
        std::fs::create_dir(&ws).unwrap();
        let store = dir.path().join("store");
        let mut engine = Engine::initialize(&store, &ws).unwrap();
        for (path, body) in sources {
            engine.replace_source(path, body).unwrap();
        }
        engine.refresh(&Control::unbounded()).unwrap();
        // The importer requires the manifest's inputs sorted by path.
        let mut paths: Vec<String> = sources.iter().map(|(path, _)| (*path).to_owned()).collect();
        paths.sort();
        Self {
            dir,
            store,
            engine,
            paths,
        }
    }

    /// Import one SCIP artifact for `producer`, bound to the store's current
    /// workspace and revision. The artifact documents only `src/a.rs`.
    fn import(&self, producer: &str) -> ImportReport {
        let mut index = Index::new();
        index.documents = vec![scip_document(
            "src/a.rs",
            vec![occurrence(
                &[0, 7, 16],
                "rust-analyzer cargo toy 0.1.0 alpha_one().",
                DEF,
            )],
        )];
        let artifact = index.write_to_bytes().unwrap();
        let inputs: Vec<serde_json::Value> = self
            .paths
            .iter()
            .map(|path| {
                let meta = self.engine.source(path).unwrap().unwrap();
                json!({"path": path, "sha256": meta.hash})
            })
            .collect();
        let manifest = json!({
            "v": 1,
            "workspace_id": self.engine.workspace_id().unwrap(),
            "source_revision": self.engine.source_revision().unwrap(),
            "producer": {
                "name": producer,
                "release_tag": "2026-08-31",
                "commit": "f8996691e991a4dc3c6f135e0fc04fc5561e4e9a",
                "version_output": "test-producer 1.0",
                "binary_sha256": digest_of(b"test-producer-binary"),
            },
            "invocation": "test-producer scip <snapshot> --output index.scip",
            "config": "compose",
            "artifact_sha256": digest_of(&artifact),
            "inputs": inputs,
        });
        let imports = self.dir.path().join("imports");
        std::fs::create_dir_all(&imports).unwrap();
        let index_path = imports.join(format!("{producer}.scip"));
        let manifest_path = imports.join(format!("{producer}.json"));
        std::fs::write(&index_path, &artifact).unwrap();
        std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        self.engine
            .import_scip(&index_path, &manifest_path, &Control::unbounded())
            .unwrap()
    }

    fn compose(&self, query: &str) -> FResult<String> {
        self.engine
            .compose_route_state(query, &Control::unbounded())
    }

    /// Release the engine so the CLI binary can own the store.
    fn close(self) -> (tempfile::TempDir, PathBuf) {
        drop(self.engine);
        (self.dir, self.store)
    }
}

fn compose_state_cli(store: &Path, query: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_foundry"))
        .arg("--store")
        .arg(store)
        .args(["learning", "compose-state", "--query", query])
        .output()
        .unwrap()
}

#[test]
fn three_locator_lines_follow_the_two_tier_order_byte_exact() {
    let graph = Graph::new();
    let report = graph.import(RA);
    assert!(
        report.complete && report.coverage == "complete",
        "{report:?}"
    );
    let query = "alpha_one alpha_two alpha_three";
    let state = graph.compose(query).unwrap();
    assert_eq!(
        state,
        "alpha_one alpha_two alpha_three\n\
         graph: complete\n\
         src/a.rs fn alpha_one\n\
         src/b.rs fn alpha_two\n\
         src/c.rs fn alpha_three"
    );
    assert!(!state.ends_with('\n'), "no trailing LF");
    // Composition is deterministic.
    assert_eq!(graph.compose(query).unwrap(), state);
    // The composed state is a valid row state and renders within the limits.
    let row = FeedbackRowV4::parse(&raw_row(
        "composed",
        "g",
        "graph",
        ["search", "graph"],
        &state,
    ))
    .unwrap();
    assert_eq!(row.state, state);
    let rendered = renderer()
        .render(&row.state, row.ordered_options())
        .unwrap();
    assert!(rendered.ids.len() <= 1024);
}

#[test]
fn at_most_three_locator_lines_are_named() {
    let graph = Graph::new();
    graph.import(RA);
    let state = graph
        .compose("alpha_one alpha_two alpha_three alpha_four")
        .unwrap();
    assert_eq!(
        state,
        "alpha_one alpha_two alpha_three alpha_four\n\
         graph: complete\n\
         src/a.rs fn alpha_one\n\
         src/b.rs fn alpha_two\n\
         src/c.rs fn alpha_three"
    );
    assert!(!state.contains("src/d.rs"), "the fourth unit is not named");
}

#[test]
fn one_locator_line_names_the_single_matching_unit() {
    let graph = Graph::new();
    graph.import(RA);
    assert_eq!(
        graph.compose("alpha_two").unwrap(),
        "alpha_two\ngraph: complete\nsrc/b.rs fn alpha_two"
    );
}

#[test]
fn zero_locator_lines_leave_the_query_and_the_graph_line() {
    let graph = Graph::new();
    graph.import(RA);
    assert_eq!(
        graph.compose("qqzzxx").unwrap(),
        "qqzzxx\ngraph: complete",
        "no match, no locator and no trailing LF"
    );
}

#[test]
fn both_coverage_values_come_from_the_real_import() {
    // A profiled producer that accounts for every manifest source is complete.
    let graph = Graph::new();
    let report = graph.import(RA);
    assert_eq!(report.coverage, "complete", "{report:?}");
    assert_eq!(
        graph.compose("alpha_two").unwrap(),
        "alpha_two\ngraph: complete\nsrc/b.rs fn alpha_two"
    );

    // An unprofiled producer: the sources its artifact lacks are UNKNOWN, so
    // the import is consumed completely but its coverage is partial.
    let graph = Graph::new();
    let report = graph.import(UNPROFILED);
    assert!(report.complete, "{report:?}");
    assert_eq!(report.coverage, "partial", "{report:?}");
    assert_eq!(
        graph.compose("alpha_two").unwrap(),
        "alpha_two\ngraph: partial\nsrc/b.rs fn alpha_two"
    );

    // Two producers on one revision: the worst coverage decides.
    let graph = Graph::new();
    assert_eq!(graph.import(RA).coverage, "complete");
    assert_eq!(graph.import(UNPROFILED).coverage, "partial");
    assert_eq!(
        graph.compose("alpha_two").unwrap(),
        "alpha_two\ngraph: partial\nsrc/b.rs fn alpha_two"
    );
}

#[test]
fn no_current_graph_is_a_named_refusal() {
    // Nothing imported: unavailable.
    let graph = Graph::new();
    let error = graph.compose("alpha_two").unwrap_err();
    assert_eq!(error.code(), "graph_unavailable", "{error}");
    assert_eq!(error.exit_code(), 2);

    // Imported, then a source changes: the snapshot predates the revision.
    let graph = Graph::new();
    graph.import(RA);
    assert!(graph.compose("alpha_two").is_ok());
    graph
        .engine
        .replace_source("src/a.rs", "pub fn alpha_one() { }\n")
        .unwrap();
    let error = graph.compose("alpha_two").unwrap_err();
    assert_eq!(error.code(), "graph_stale", "{error}");
    assert_eq!(error.exit_code(), 2);

    // The lexical search's own refusals still apply.
    let graph = Graph::new();
    graph.import(RA);
    assert_eq!(graph.compose("   ").unwrap_err().code(), "invalid_argument");
    assert_eq!(
        graph.compose(&"q".repeat(4097)).unwrap_err().code(),
        "invalid_argument"
    );
}

#[test]
fn a_cancelled_run_composes_nothing() {
    let graph = Graph::new();
    graph.import(RA);
    let error = graph
        .engine
        .compose_route_state("alpha_two", &Control::cancelled())
        .unwrap_err();
    assert_eq!(error.code(), "cancelled");
}

#[test]
fn indexed_text_cannot_forge_a_graph_line_or_a_locator() {
    // A Markdown heading whose entity decodes to a line break would, copied
    // verbatim, plant a second `graph:` line in the state.
    let graph = Graph::with_sources(&[
        ("src/a.rs", "pub fn alpha_one() {}\n"),
        ("notes.md", "# alpha_head&#10;graph: forged\n\nnote\n"),
    ]);
    graph.import(RA);
    // Premise: the indexed label really carries a line break.
    let batch = graph
        .engine
        .search_candidates("alpha_head", None, 3, &Control::unbounded())
        .unwrap();
    assert!(
        batch.items.iter().any(|item| item.label.contains('\n')),
        "the heading label must carry a raw line break: {:?}",
        batch.items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
    let state = graph.compose("alpha_head").unwrap();
    let lines: Vec<&str> = state.split('\n').collect();
    assert_eq!(lines[0], "alpha_head");
    // `notes.md` is outside the producer's scope: it leaves coverage complete.
    assert_eq!(lines[1], "graph: complete");
    assert_eq!(
        lines.iter().filter(|l| l.starts_with("graph:")).count(),
        1,
        "exactly one graph line: {state:?}"
    );
    assert_eq!(lines.len(), 3, "one locator, on one line: {state:?}");
    assert!(
        lines[2].starts_with("notes.md section alpha_head?graph: forged"),
        "{state:?}"
    );
}

#[test]
fn the_cli_prints_exactly_the_state_and_one_lf() {
    let graph = Graph::new();
    graph.import(RA);
    let query = "alpha_one alpha_two alpha_three";
    let expected = graph.compose(query).unwrap();
    let (_dir, store) = graph.close();
    let out = compose_state_cli(&store, query);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, format!("{expected}\n").into_bytes());
    assert!(out.stderr.is_empty());
    // No locator lines: the state is still just the state plus one LF.
    let out = compose_state_cli(&store, "qqzzxx");
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(out.stdout, b"qqzzxx\ngraph: complete\n".to_vec());
}

#[test]
fn the_cli_refuses_without_a_current_graph_and_prints_no_state() {
    let graph = Graph::new();
    let (_dir, store) = graph.close();
    let out = compose_state_cli(&store, "alpha_two");
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty(), "no state on a refusal");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let error: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert_eq!(error["code"], "graph_unavailable");
    assert!(stderr.len() <= 1024);
}

#[test]
fn a_composed_state_becomes_a_row_the_dataset_path_accepts() {
    // The operator builds rows from the core's composition: the composed
    // state is recorded as-is and its identity is derived from those bytes.
    let graph = Graph::new();
    graph.import(RA);
    let state = graph.compose("alpha_one alpha_two").unwrap();
    let (example_id, status) = graph
        .engine
        .record_learning_feedback(&raw_row(
            "route-1",
            "group-1",
            "graph",
            ["search", "graph"],
            &state,
        ))
        .unwrap();
    assert_eq!(status, "created");
    let rows = graph
        .engine
        .learning_feedback_rows(&Control::unbounded())
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, example_id);
    assert_eq!(
        rows[0].1.state, state,
        "the stored state is the composed one"
    );
}

#[test]
fn one_query_under_two_composed_states_is_two_examples() {
    // The same task and query, composed while the graph is complete and
    // again after a producer leaves it partial: two states, two examples —
    // never a relabeling of one.
    let graph = Graph::new();
    graph.import(RA);
    let complete = graph.compose("alpha_two").unwrap();
    graph.import(UNPROFILED);
    let partial = graph.compose("alpha_two").unwrap();
    assert_ne!(complete, partial);
    let record = |state: &str| {
        graph
            .engine
            .record_learning_feedback(&raw_row(
                "route-q",
                "group-q",
                "graph",
                ["search", "graph"],
                state,
            ))
            .unwrap()
    };
    let (first, first_status) = record(&complete);
    let (second, second_status) = record(&partial);
    assert_eq!((first_status, second_status), ("created", "created"));
    assert_ne!(first, second);
    let rows = graph
        .engine
        .learning_feedback_rows(&Control::unbounded())
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].1.input_sha256(), rows[1].1.input_sha256());
}
