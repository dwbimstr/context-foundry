//! Real HTTP transport, CLI, database, cache and retrieval; synthetic completions.
//! No real-model quality, weight attestation or provider-economics claim.
use context_foundry::fault::{self, Action, names};
use context_foundry::neural::work::Lane;
use context_foundry::{
    Control, Engine, FoundryError, brief,
    knowledge::{self, Node, Profile, RunLimits, SourceSpan},
    model_resources,
};
use redb::{ReadableDatabase, TableDefinition};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

const A: &str = "pub fn opaque(q: &mut Vec<u8>) { q.remove(0); }\n";
fn response(model: &str, content: Value) -> Value {
    json!({"model":model,"choices":[{"finish_reason":"stop","message":{"content":content.to_string()}}],"usage":{"prompt_tokens":12,"completion_tokens":7}})
}
fn output() -> Value {
    json!({"purpose":[{"text":"capacity eviction","citations":[0],"identifiers":[]}],"behavior":[{"text":"Inspect original code to establish the removal contract.","citations":[0],"identifiers":[]}],"provides":[],"consumes":[],"constraints":[],"tests":[],"unknowns":["Concurrency is unknown."]})
}
struct Server {
    url: String,
    requests: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    fn new(handler: impl Fn(&Value, usize) -> (u16, String, Duration) + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (done, seen) = (stop.clone(), requests.clone());
        let thread = std::thread::spawn(move || {
            while !done.load(Ordering::SeqCst) {
                let (mut stream, _) = match listener.accept() {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let request = read_request(&mut stream);
                let n = {
                    let mut log = seen.lock().unwrap();
                    log.push(request.clone());
                    log.len()
                };
                let (status, body, delay) = handler(&request, n);
                std::thread::sleep(delay);
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Self {
            url,
            requests,
            stop,
            thread: Some(thread),
        }
    }
    fn good() -> Self {
        Self::new(|_, _| {
            (
                200,
                response("fixture-model", output()).to_string(),
                Duration::ZERO,
            )
        })
    }
    fn calls(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn read_request(stream: &mut TcpStream) -> Value {
    let mut bytes = Vec::new();
    let split = loop {
        let mut one = [0; 1];
        stream.read_exact(&mut one).unwrap();
        bytes.push(one[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            break bytes.len();
        }
        assert!(bytes.len() < 8192);
    };
    let header = String::from_utf8(bytes.clone()).unwrap();
    let size = header
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")
                .map(|v| v.parse::<usize>().unwrap())
        })
        .unwrap();
    assert!(size <= 131072);
    bytes.resize(split + size, 0);
    stream.read_exact(&mut bytes[split..]).unwrap();
    serde_json::from_slice(&bytes[split..]).unwrap()
}
struct Env {
    dir: tempfile::TempDir,
    store: PathBuf,
    profile: Profile,
    engine: Option<Engine>,
}
impl Env {
    fn new(server: &Server) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let store = dir.path().join("store");
        let mut engine = Engine::initialize(&store, &root).unwrap();
        for (path, body) in [
            ("src/a.rs", A),
            ("src/b.rs", "fn other() {}\n"),
            ("tests/a.rs", "#[test] fn removal() {}\n"),
            ("area.md", "# Queue contract\n"),
            ("system.md", "# System map\n"),
        ] {
            engine.replace_source(path, body).unwrap();
        }
        engine.refresh(&Control::unbounded()).unwrap();
        let span = |path: &str| SourceSpan {
            path: path.into(),
            start: 0,
            end: None,
        };
        let profile = Profile {
            v: 1,
            workspace_id: engine.workspace_id().unwrap(),
            recipe: "fixture-v1".into(),
            model: "fixture-model".into(),
            artifact_sha256: "a".repeat(64),
            endpoint: server.url.clone(),
            local: None,
            allow_remote: false,
            token_env: None,
            source_scope: vec![
                "src".into(),
                "tests".into(),
                "area.md".into(),
                "system.md".into(),
            ],
            instructions: "Use only supplied evidence.".into(),
            max_request_tokens: 8192,
            max_request_bytes: 131072,
            output_tokens: 512,
            call_seconds: 5,
            cache_bytes: 1024 * 1024,
            nodes: vec![
                Node {
                    id: "a".into(),
                    sources: vec![span("src/a.rs"), span("tests/a.rs")],
                    children: vec![],
                },
                Node {
                    id: "b".into(),
                    sources: vec![span("src/b.rs")],
                    children: vec![],
                },
                Node {
                    id: "area".into(),
                    sources: vec![span("area.md")],
                    children: vec!["a".into(), "b".into()],
                },
                Node {
                    id: "system".into(),
                    sources: vec![span("system.md")],
                    children: vec!["area".into()],
                },
            ],
        };
        Self {
            dir,
            store,
            profile,
            engine: Some(engine),
        }
    }
    fn engine(&self) -> &Engine {
        self.engine.as_ref().unwrap()
    }
    async fn run(&mut self, calls: u32) -> knowledge::Report {
        knowledge::prepare(
            self.engine.as_mut().unwrap(),
            &self.profile,
            RunLimits {
                calls,
                request_tokens: 100_000,
                output_tokens: 10_000,
            },
            &Control::with_deadline(Instant::now() + Duration::from_secs(10)),
        )
        .await
        .unwrap()
    }
    fn reopen(&mut self) {
        self.engine = None;
        self.engine = Some(Engine::open_existing(&self.store).unwrap());
    }
    fn records(&self) -> Vec<brief::Record> {
        // Query through the actual brief API, including eligibility checks.
        let result = self
            .engine()
            .brief(
                &brief::Request {
                    workspace_id: self.profile.workspace_id.clone(),
                    command: brief::Command::Search {
                        query: "capacity eviction".into(),
                        limit: 8,
                        max_bytes: 262144,
                    },
                },
                &Control::unbounded(),
            )
            .unwrap();
        serde_json::from_value(result["hits"].clone()).unwrap()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn exact_inputs_resume_across_restart_and_only_changed_transitive_dependencies_regenerate() {
    let server = Server::good();
    let mut env = Env::new(&server);
    let partial = env.run(2).await;
    assert!(!partial.complete, "{partial:?}");
    assert_eq!(partial.calls, 2);
    assert_eq!(partial.generated, 2);
    env.reopen();
    let finished = env.run(8).await;
    assert!(finished.complete, "{finished:?}");
    assert_eq!(finished.calls, 2);
    assert_eq!(finished.reused_current, 2);
    assert_eq!(finished.observed_prompt_tokens, 24);
    assert_eq!(finished.observed_completion_tokens, 14);
    assert_eq!(finished.usage_unknown_calls, 0);
    assert_eq!(
        env.engine()
            .derivation_pending(Lane::Knowledge)
            .unwrap()
            .paths,
        0
    );
    assert!(
        env.engine()
            .derivation_pending(Lane::Semantic)
            .unwrap()
            .paths
            > 0
    );
    env.engine()
        .replace_source("unselected.rs", "fn unrelated_pending() {}\n")
        .unwrap();
    let pending = env.engine().pending().unwrap();
    assert!(pending > 0);
    assert!(env.run(8).await.complete);
    assert_eq!(
        env.engine().pending().unwrap(),
        pending,
        "knowledge must not drain unrelated source indexing"
    );
    let records = env.records();
    assert_eq!(records.len(), 4);
    let root = records
        .iter()
        .find(|r| r.derivation.as_ref().unwrap().node == "system")
        .unwrap();
    assert_eq!(root.dependencies.len(), 5);
    assert_eq!(root.derivation.as_ref().unwrap().children.len(), 3);
    let exact = env
        .engine()
        .retrieve(
            &records
                .iter()
                .find(|r| r.derivation.as_ref().unwrap().node == "a")
                .unwrap()
                .brief
                .cited_spans[0],
            None,
            4096,
        )
        .unwrap();
    assert_eq!(exact.span, A.as_bytes());
    let context = env
        .engine()
        .context_candidates_with(
            "capacity eviction",
            context_foundry::Strategy::Search,
            &Control::unbounded(),
            &context_foundry::store::ContextOptions {
                briefs: true,
                ..Default::default()
            },
        )
        .unwrap();
    let packed = context_foundry::response::pack_enriched_context(
        &context,
        None,
        context_foundry::response::Budget::request(4096),
        &context_foundry::response::stdout_bytes,
    )
    .unwrap();
    assert!(packed.text.contains("kind\":\"derived"));
    assert!(packed.text.contains("q.remove(0)"));
    assert!(packed.tokens <= 4096);
    fault::arm(
        names::KNOWLEDGE_INPUT,
        0,
        Action::Fail("unchanged inputs must not be read".into()),
    );
    let unchanged = env.run(8).await;
    assert!(unchanged.complete, "{unchanged:?}");
    assert_eq!(unchanged.calls, 0);
    fault::disarm_all();
    env.engine()
        .replace_source("src/a.rs", &A.replace("remove(0)", "remove(1)"))
        .unwrap();
    assert_eq!(
        env.records().len(),
        1,
        "all transitive ancestors stale before reindex/generation"
    );
    let changed = env.run(8).await;
    assert!(changed.complete, "{changed:?}");
    assert_eq!(changed.generated, 3);
    assert_eq!(changed.reused_current, 1);
    assert_eq!(
        server.calls(),
        7,
        "identical synthetic prose cannot hide dependency changes"
    );
    env.engine().replace_source("src/a.rs", A).unwrap();
    env.reopen();
    let reverted = env.run(8).await;
    assert!(reverted.complete, "{reverted:?}");
    assert_eq!(reverted.calls, 0);
    assert_eq!(reverted.reused_cached, 3);
    assert_eq!(server.calls(), 7);
    env.engine()
        .replace_source("unselected.rs", "fn unrelated() {}\n")
        .unwrap();
    assert_eq!(env.run(8).await.calls, 0);
}

#[tokio::test(flavor = "current_thread")]
async fn edit_during_generation_and_interrupted_acceptance_never_publish_stale_ancestors() {
    let server = Server::good();
    let mut env = Env::new(&server);
    fault::arm(
        names::KNOWLEDGE_BEFORE_CALL,
        0,
        Action::Call(Box::new(|ctx| {
            ctx.engine
                .unwrap()
                .replace_source("src/a.rs", "fn changed() {}\n")
                .unwrap();
        })),
    );
    let report = env.run(8).await;
    fault::disarm_all();
    assert!(!report.complete);
    assert_eq!(report.nodes[0].reason.as_deref(), Some("stale_handle"));
    assert_eq!(env.records().len(), 1);
    let fixed = env.run(8).await;
    assert!(fixed.complete, "{fixed:?}");
    env.engine()
        .replace_source("src/b.rs", "fn new_body() {}\n")
        .unwrap();
    fault::arm(
        names::BRIEF_AFTER_COMMIT,
        0,
        Action::Fail("interrupted after acceptance".into()),
    );
    let interrupted = env.run(8).await;
    fault::disarm_all();
    assert!(!interrupted.complete);
    env.reopen();
    let retry = env.run(8).await;
    assert!(retry.complete, "{retry:?}");
    assert_eq!(
        retry.calls, 2,
        "committed child is reused, only ancestors call"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn forgetting_a_child_removes_memoized_copies_and_generated_ancestors_atomically() {
    let server = Server::good();
    let mut env = Env::new(&server);
    assert!(env.run(8).await.complete);
    let leaf = env
        .records()
        .into_iter()
        .find(|r| r.derivation.as_ref().unwrap().node == "a")
        .unwrap();
    env.engine()
        .brief(
            &brief::Request {
                workspace_id: env.profile.workspace_id.clone(),
                command: brief::Command::Forget {
                    id: leaf.id,
                    expected_revision: leaf.revision,
                },
            },
            &Control::unbounded(),
        )
        .unwrap();
    assert_eq!(
        env.engine().knowledge_status().unwrap()["cache"]["entries"],
        1
    );
    assert_eq!(env.records().len(), 1);
    let report = env.run(8).await;
    assert!(report.complete, "{report:?}");
    assert_eq!(report.generated, 3);
    let purged = env.engine().knowledge_purge_cache().unwrap();
    assert_eq!(purged["removed_entries"], 4);
    assert_eq!(
        env.run(8).await.calls,
        0,
        "current accepted records survive cache purge"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn transport_failures_are_bounded_redacted_and_back_off_across_restart() {
    let mut unfinished = response("fixture-model", output());
    unfinished["choices"][0]["finish_reason"] = json!("length");
    let mut tools = response("fixture-model", output());
    tools["choices"][0]["message"]["tool_calls"] = json!([{"type":"function"}]);
    let mut overrun = response("fixture-model", output());
    overrun["usage"]["completion_tokens"] = json!(513);
    for (status, body) in [
        (429, "secret provider detail".into()),
        (302, "".into()),
        (200, "not-json secret".into()),
        (200, "x".repeat(262145)),
        (200, response("wrong-model", output()).to_string()),
        (200, unfinished.to_string()),
        (200, tools.to_string()),
        (200, overrun.to_string()),
        (
            200,
            response("fixture-model", json!({"unexpected":"secret"})).to_string(),
        ),
    ] {
        let server = Server::new(move |_, _| (status, body.clone(), Duration::ZERO));
        let mut env = Env::new(&server);
        let report = env.run(8).await;
        assert!(!report.complete);
        assert_eq!(report.calls, 1, "{report:?}");
        assert!(!serde_json::to_string(&report).unwrap().contains("secret"));
        assert_eq!(
            env.engine().knowledge_status().unwrap()["cache"]["entries"],
            0
        );
        env.reopen();
        let retry = env.run(8).await;
        assert_eq!(retry.calls, 0, "backoff survives restart");
        assert_eq!(server.calls(), 1);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn provider_deadline_closes_an_admitted_call_and_missing_usage_stays_unknown() {
    let server = Server::new(|_, _| {
        (
            200,
            response("fixture-model", output()).to_string(),
            Duration::from_secs(2),
        )
    });
    let mut env = Env::new(&server);
    // Start the tested timeout at provider admission, not before source/hash/
    // token preparation. The old 80 ms whole-run deadline could correctly admit
    // zero calls on a loaded host, contradicting its usage assertion.
    env.profile.call_seconds = 1;
    let started = Instant::now();
    let report = knowledge::prepare(
        env.engine.as_mut().unwrap(),
        &env.profile,
        RunLimits {
            calls: 8,
            request_tokens: 100_000,
            output_tokens: 10_000,
        },
        &Control::with_deadline(started + Duration::from_secs(10)),
    )
    .await
    .unwrap();
    assert!(!report.complete);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(report.calls, 1);
    assert_eq!(server.calls(), 1);
    assert_eq!(report.usage_unknown_calls, 1);
    let server = Server::new(|_, _| {
        let mut r = response("fixture-model", output());
        r.as_object_mut().unwrap().remove("usage");
        (200, r.to_string(), Duration::ZERO)
    });
    let mut env = Env::new(&server);
    let report = env.run(8).await;
    assert!(report.complete);
    assert_eq!(report.usage_unknown_calls, 4);
    assert_eq!(report.observed_prompt_tokens, 0);
}

#[test]
fn plan_validation_refuses_cycles_scope_escape_and_unbounded_work() {
    let server = Server::good();
    let env = Env::new(&server);
    for mutate in [
        (|p: &mut Profile| p.nodes[0].children.push("system".into())) as fn(&mut Profile),
        |p| p.nodes[0].sources[0].path = "src-else/private.rs".into(),
        |p| p.endpoint = "http://example.com/v1/chat/completions".into(),
        |p| p.endpoint = "https://example.com/v1/chat/completions".into(),
        |p| p.nodes[0].sources[0].path = "src/../private.rs".into(),
        |p| p.nodes.extend(vec![p.nodes[0].clone(); 128]),
        |p| p.output_tokens = 100_000,
    ] {
        let mut p = env.profile.clone();
        mutate(&mut p);
        assert!(p.validate().is_err());
    }
    let mut p = env.profile.clone();
    p.endpoint = "https://example.com/v1/chat/completions".into();
    p.allow_remote = true;
    assert!(p.validate().is_ok());
    assert_eq!(server.calls(), 0);
}

#[test]
fn schema_nine_requires_explicit_atomic_upgrade_without_losing_sources() {
    let server = Server::good();
    let mut env = Env::new(&server);
    env.engine = None;
    let db = redb::Database::open(env.store.join("knowledge.redb")).unwrap();
    let tx = db.begin_write().unwrap();
    tx.delete_table(TableDefinition::<&str, &str>::new("knowledge_cache"))
        .unwrap();
    tx.open_table(TableDefinition::<&str, &str>::new("meta"))
        .unwrap()
        .insert("schema", "9")
        .unwrap();
    tx.commit().unwrap();
    drop(db);
    assert!(matches!(
        Engine::open_existing(&env.store),
        Err(FoundryError::UpgradeRequired { .. })
    ));
    fault::arm(
        names::UPGRADE_BEFORE_COMMIT,
        0,
        Action::Fail("interrupt".into()),
    );
    assert!(
        Engine::upgrade_store(
            &env.store,
            context_foundry::SCHEMA_VERSION,
            &Control::unbounded()
        )
        .is_err()
    );
    fault::disarm_all();
    assert_eq!(context_foundry::testkit::schema_marker(&env.store), "9");
    Engine::upgrade_store(
        &env.store,
        context_foundry::SCHEMA_VERSION,
        &Control::unbounded(),
    )
    .unwrap();
    env.reopen();
    assert_eq!(env.engine().source_revision().unwrap(), 5);
    assert_eq!(
        env.engine().knowledge_status().unwrap()["cache"]["entries"],
        0
    );
}

#[test]
fn actual_cli_generates_and_reuses_an_explicit_http_profile() {
    let server = Server::good();
    let mut env = Env::new(&server);
    let profile = env.dir.path().join("profile.json");
    std::fs::write(&profile, serde_json::to_vec(&env.profile).unwrap()).unwrap();
    env.engine = None;
    for expected in [4, 0] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_foundry"))
            .arg("--store")
            .arg(&env.store)
            .args(["knowledge", "prepare", "--profile"])
            .arg(&profile)
            .args(["--budget-seconds", "10"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let report: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(report["calls"], expected);
        assert_eq!(report["complete"], true);
    }
    assert_eq!(server.calls(), 4);
}

#[tokio::test(flavor = "current_thread")]
async fn model_identity_changes_rekey_and_budget_input_and_claim_limits_refuse_before_publication()
{
    let server = Server::good();
    let mut env = Env::new(&server);
    assert!(env.run(8).await.complete);
    env.profile.artifact_sha256 = "b".repeat(64);
    let changed = env.run(8).await;
    assert_eq!(changed.generated, 4);
    assert!(changed.complete);
    env.profile.artifact_sha256 = "a".repeat(64);
    let revert = env.run(8).await;
    assert_eq!(revert.reused_cached, 4);
    assert_eq!(revert.calls, 0);
    env.profile.nodes.truncate(1);
    env.engine()
        .replace_source("src/a.rs", &"very large source bytes\n".repeat(7000))
        .unwrap();
    let oversized = env.run(8).await;
    assert_eq!(oversized.calls, 0);
    assert_eq!(
        oversized.nodes[0].reason.as_deref(),
        Some("knowledge_input_limit")
    );
    env.profile.nodes[0].sources[0].end = Some(22);
    let budget = knowledge::prepare(
        env.engine.as_mut().unwrap(),
        &env.profile,
        RunLimits {
            calls: 8,
            request_tokens: 1,
            output_tokens: 512,
        },
        &Control::unbounded(),
    )
    .await
    .unwrap();
    assert_eq!(budget.calls, 0);
    assert!(!budget.complete);
    assert!(
        env.run(8).await.complete,
        "explicit exact span fits without silently truncating the whole file"
    );
    let server = Server::new(|_, _| {
        let mut value = output();
        value["behavior"][0]["identifiers"] = json!(["nonexistent_identifier"]);
        (
            200,
            response("fixture-model", value).to_string(),
            Duration::ZERO,
        )
    });
    let mut env = Env::new(&server);
    let invalid = env.run(8).await;
    assert_eq!(invalid.calls, 1);
    assert_eq!(
        invalid.nodes[0].reason.as_deref(),
        Some("knowledge_output_invalid")
    );
    assert_eq!(
        env.engine().knowledge_status().unwrap()["cache"]["entries"],
        0
    );
}

#[tokio::test(flavor = "current_thread")]
async fn bounded_cache_evicts_completed_history_and_retains_current_no_call_replay() {
    let server = Server::new(|_, _| {
        let mut value = output();
        value["behavior"][0]["text"] = json!("x".repeat(15000));
        (
            200,
            response("fixture-model", value).to_string(),
            Duration::ZERO,
        )
    });
    let mut env = Env::new(&server);
    env.profile.cache_bytes = 2 * 1024 * 1024;
    env.profile.nodes.clear();
    for i in 0..72 {
        let path = format!("src/p{i}.rs");
        env.engine()
            .replace_source(&path, &format!("fn p{i}() {{}}\n"))
            .unwrap();
        env.profile.nodes.push(Node {
            id: format!("p{i}"),
            sources: vec![SourceSpan {
                path,
                start: 0,
                end: None,
            }],
            children: vec![],
        });
    }
    let report = knowledge::prepare(
        env.engine.as_mut().unwrap(),
        &env.profile,
        RunLimits {
            calls: 128,
            request_tokens: 1_000_000,
            output_tokens: 100_000,
        },
        &Control::unbounded(),
    )
    .await
    .unwrap();
    assert!(report.complete, "{report:?}");
    assert_eq!(report.calls, 72);
    assert_eq!(
        env.engine().knowledge_status().unwrap()["cache"]["entries"],
        72
    );
    env.profile.cache_bytes = 1024 * 1024;
    assert_eq!(
        env.run(8).await.calls,
        0,
        "lowered cap trims history without model calls"
    );
    let stats = env.engine().knowledge_status().unwrap();
    assert!(stats["cache"]["entries"].as_u64().unwrap() < 72);
    assert!(stats["cache"]["bytes"].as_u64().unwrap() <= 1024 * 1024);
    env.engine = None;
    let db = redb::Database::open(env.store.join("knowledge.redb")).unwrap();
    let tx = db.begin_read().unwrap();
    let table = tx
        .open_table(TableDefinition::<&str, &str>::new("knowledge_cache"))
        .unwrap();
    let bytes: u64 = table
        .range("!"..)
        .unwrap()
        .map(|r| r.unwrap().1.value().len() as u64)
        .sum();
    assert_eq!(stats["cache"]["bytes"], bytes);
    drop(table);
    drop(tx);
    drop(db);
    env.reopen();
    let unchanged = env.run(8).await;
    assert!(unchanged.complete);
    assert_eq!(unchanged.calls, 0);
}

#[test]
fn knowledge_crash_child() {
    let Ok(store) = std::env::var("CF_KNOWLEDGE_CRASH_STORE") else {
        return;
    };
    let profile = Profile::load(
        PathBuf::from(std::env::var("CF_KNOWLEDGE_CRASH_PROFILE").unwrap()).as_path(),
    )
    .unwrap();
    let point = if std::env::var("CF_KNOWLEDGE_CRASH_STAGE").unwrap() == "before" {
        names::BRIEF_BEFORE_COMMIT
    } else {
        names::BRIEF_AFTER_COMMIT
    };
    let mut engine = Engine::open_existing(PathBuf::from(store).as_path()).unwrap();
    fault::arm(point, 0, Action::Abort);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(knowledge::prepare(
            &mut engine,
            &profile,
            RunLimits {
                calls: 8,
                request_tokens: 100_000,
                output_tokens: 10000,
            },
            &Control::unbounded(),
        ))
        .unwrap();
    panic!("child should have aborted");
}
#[tokio::test(flavor = "current_thread")]
async fn process_death_keeps_cache_and_live_brief_in_the_same_transaction() {
    for (stage, expected) in [("before", 4), ("after", 3)] {
        let server = Server::good();
        let mut env = Env::new(&server);
        let profile = env.dir.path().join("profile.json");
        std::fs::write(&profile, serde_json::to_vec(&env.profile).unwrap()).unwrap();
        env.engine = None;
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["knowledge_crash_child", "--exact", "--nocapture"])
            .env("CF_KNOWLEDGE_CRASH_STORE", &env.store)
            .env("CF_KNOWLEDGE_CRASH_PROFILE", &profile)
            .env("CF_KNOWLEDGE_CRASH_STAGE", stage)
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert_eq!(server.calls(), 1);
        env.reopen();
        assert_eq!(
            env.engine().knowledge_status().unwrap()["cache"]["entries"],
            if stage == "before" { 0 } else { 1 }
        );
        let retried = env.run(8).await;
        assert!(retried.complete, "{retried:?}");
        assert_eq!(retried.calls, expected);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn ancestors_bind_changed_prose_even_when_evicted_child_inputs_return_to_the_same_identity() {
    let server = Server::new(|_, call| {
        let mut value = output();
        if call > 4 {
            value["behavior"][0]["text"] =
                json!("Different synthetic phrasing after cache eviction.");
        }
        (
            200,
            response("fixture-model", value).to_string(),
            Duration::ZERO,
        )
    });
    let mut env = Env::new(&server);
    assert!(env.run(8).await.complete);
    env.engine()
        .replace_source("src/a.rs", &A.replace("remove(0)", "remove(1)"))
        .unwrap();
    let partial = env.run(1).await;
    assert!(!partial.complete);
    assert_eq!(partial.generated, 1);
    env.engine().knowledge_purge_cache().unwrap();
    env.engine().replace_source("src/a.rs", A).unwrap();
    let reverted = env.run(8).await;
    assert!(reverted.complete, "{reverted:?}");
    assert_eq!(
        reverted.generated, 3,
        "original parent input identity alone cannot validate newly generated child prose"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn recipe_and_child_source_selection_changes_preserve_exact_plan_identity() {
    let server = Server::good();
    let mut env = Env::new(&server);
    assert_eq!(env.run(8).await.generated, 4);
    env.profile.recipe = "fixture-v2".into();
    let changed = env.run(8).await;
    assert!(changed.complete, "{changed:?}");
    assert_eq!(changed.generated, 4);
    env.profile.recipe = "fixture-v1".into();
    let reverted = env.run(8).await;
    assert!(reverted.complete, "{reverted:?}");
    assert_eq!((reverted.calls, reverted.reused_current), (0, 4));

    // The old child still exists and remains current. The selected child now has
    // a different primary source, and must replace it in both ancestor inputs.
    env.profile.nodes[0].sources[0].path = "src/b.rs".into();
    let moved = env.run(8).await;
    assert!(moved.complete, "{moved:?}");
    assert_eq!((moved.generated, moved.reused_current), (3, 1));
    env.profile.nodes[0].sources[0].path = "src/a.rs".into();
    let restored = env.run(8).await;
    assert!(restored.complete, "{restored:?}");
    assert_eq!((restored.calls, restored.reused_cached), (0, 2));
}

// Real MCP child-process consumer, using the same HTTP completion fixture above.
type McpClient = rmcp::service::RunningService<rmcp::RoleClient, ()>;
async fn launch_mcp(env: &mut Env, maintain: bool, watch: bool) -> McpClient {
    launch_mcp_resources(env, maintain, watch, None).await
}
async fn launch_mcp_resources(
    env: &mut Env,
    maintain: bool,
    watch: bool,
    resources: Option<model_resources::Config>,
) -> McpClient {
    use rmcp::ServiceExt;
    let root = env.dir.path().join("root");
    for (path, body) in [
        ("src/a.rs", A),
        ("src/b.rs", "fn other() {}\n"),
        ("tests/a.rs", "#[test] fn removal() {}\n"),
        ("area.md", "# Queue contract\n"),
        ("system.md", "# System map\n"),
    ] {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // Preserve edits across owner restarts.
        if !path.exists() {
            std::fs::write(path, body).unwrap();
        }
    }
    let profile = env.dir.path().join("knowledge.json");
    std::fs::write(&profile, serde_json::to_vec(&env.profile).unwrap()).unwrap();
    env.engine = None;
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_foundry"));
    command
        .arg("--store")
        .arg(&env.store)
        .arg("mcp")
        .arg("--root")
        .arg(&root)
        .arg("--knowledge-profile")
        .arg(profile);
    if maintain {
        command.arg("--knowledge-maintain");
    }
    if watch {
        command.arg("--watch");
    }
    if let Some(resources) = resources {
        let path = env.dir.path().join("model-resources.json");
        std::fs::write(&path, serde_json::to_vec(&resources).unwrap()).unwrap();
        command.arg("--model-resources").arg(path);
    }
    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    ().serve(rmcp::transport::TokioChildProcess::new(command).unwrap())
        .await
        .unwrap()
}
async fn mcp_call(client: &McpClient, name: &'static str, args: Value) -> String {
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        client.call_tool(
            rmcp::model::CallToolRequestParams::new(name)
                .with_arguments(args.as_object().unwrap().clone()),
        ),
    )
    .await
    .expect("foreground remained responsive")
    .unwrap();
    let rmcp::model::ContentBlock::Text(text) = &result.content[0] else {
        panic!("text result required")
    };
    assert_ne!(result.is_error, Some(true), "{}", text.text);
    text.text.clone()
}
async fn mcp_until(client: &McpClient, predicate: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let value: Value =
            serde_json::from_str(&mcp_call(client, "status", json!({})).await).unwrap();
        if predicate(&value) {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "maintenance did not converge: {value}"
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}
async fn stop_mcp(client: McpClient, env: &mut Env) {
    client.cancel().await.unwrap();
    reopen_when_idle(env).await;
}
async fn reopen_when_idle(env: &mut Env) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match Engine::open_existing(&env.store) {
            Ok(engine) => {
                env.engine = Some(engine);
                return;
            }
            Err(FoundryError::StoreBusy) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(20)).await
            }
            Err(error) => panic!("owner did not release its store: {error:?}"),
        }
    }
}
fn caught_up(value: &Value) -> bool {
    value["knowledge"]["maintenance"]["state"] == "waiting"
        && value["knowledge"]["pending"]["paths"] == 0
        && value["knowledge"]["pending"]["reconcile_required"] == false
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_opt_in_watching_generates_changed_areas_and_delivers_exact_derived_context() {
    let server = Server::good();
    let mut env = Env::new(&server);
    let client = launch_mcp(&mut env, false, true).await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(
        server.calls(),
        0,
        "watching and a profile alone never generate"
    );
    let catalog = client.list_all_tools().await.unwrap();
    let context = catalog.iter().find(|tool| tool.name == "context").unwrap();
    assert!(
        context.input_schema["properties"]
            .get("include_briefs")
            .is_some()
    );
    mcp_call(&client, "index", json!({"knowledge":"maintain"})).await;
    let ready = mcp_until(&client, caught_up).await;
    assert_eq!(
        ready["knowledge"]["maintenance"]["completed_pass_usage"]["calls"],
        4
    );
    let text = mcp_call(
        &client,
        "context",
        json!({"query":"capacity eviction","include_briefs":true,"tokens":8192}),
    )
    .await;
    assert!(text.contains("brief:"), "{text}");
    assert!(text.contains("q.remove(0)"), "{text}");
    assert!(text.contains("derived"), "{text}");
    std::fs::write(
        env.dir.path().join("root/src/a.rs"),
        A.replace("remove(0)", "remove(1)"),
    )
    .unwrap();
    let changed = mcp_until(&client, |s| {
        caught_up(s) && s["knowledge"]["maintenance"]["completed_pass_usage"]["calls"] == 7
    })
    .await;
    assert_eq!(
        changed["knowledge"]["maintenance"]["last_run"]["generated"],
        3
    );
    std::fs::write(env.dir.path().join("root/src/a.rs"), A).unwrap();
    mcp_until(&client, |s| {
        caught_up(s) && s["knowledge"]["maintenance"]["last_run"]["reused_cached"] == 3
    })
    .await;
    for _ in 0..5 {
        mcp_call(&client, "status", json!({})).await;
    }
    assert_eq!(
        server.calls(),
        7,
        "revert and idle observation make no model calls"
    );
    stop_mcp(client, &mut env).await;
    let restarted = launch_mcp(&mut env, true, false).await;
    let ready = mcp_until(&restarted, caught_up).await;
    assert_eq!(
        ready["knowledge"]["maintenance"]["completed_pass_usage"]["calls"],
        0
    );
    assert_eq!(server.calls(), 7);
    stop_mcp(restarted, &mut env).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_pause_finishes_one_inflight_call_and_resume_keeps_completed_results() {
    let release = Arc::new(AtomicBool::new(false));
    let gate = release.clone();
    let server = Server::new(move |_, call| {
        if call == 1 {
            let until = Instant::now() + Duration::from_secs(10);
            while !gate.load(Ordering::SeqCst) && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        (
            200,
            response("fixture-model", output()).to_string(),
            Duration::ZERO,
        )
    });
    let mut env = Env::new(&server);
    let client = launch_mcp(&mut env, true, false).await;
    let deadline = Instant::now() + Duration::from_secs(5);
    while server.calls() == 0 {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // A blocked HTTP provider must not hold the store or MCP request slot.
    let text = mcp_call(&client, "context", json!({"query":"opaque","tokens":2048})).await;
    assert!(text.contains("opaque"));
    mcp_call(&client, "index", json!({"knowledge":"pause"})).await;
    release.store(true, Ordering::SeqCst);
    let paused = mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["state"] == "paused"
    })
    .await;
    assert_eq!(server.calls(), 1);
    assert_eq!(paused["knowledge"]["cache"]["entries"], 1);
    assert!(
        paused["knowledge"]["pending"]["reconcile_required"]
            .as_bool()
            .unwrap()
    );
    mcp_call(&client, "index", json!({"knowledge":"maintain"})).await;
    mcp_until(&client, caught_up).await;
    assert_eq!(server.calls(), 4);
    stop_mcp(client, &mut env).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_admission_window_survives_pause_resume_and_restart_reuses_eight_completed_nodes() {
    let server = Server::good();
    let mut env = Env::new(&server);
    env.profile.nodes = (0..9)
        .map(|i| Node {
            id: format!("leaf{i}"),
            sources: vec![env.profile.nodes[0].sources[0].clone()],
            children: vec![],
        })
        .collect();
    let client = launch_mcp(&mut env, true, false).await;
    mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["state"] == "budget_wait"
    })
    .await;
    assert_eq!(server.calls(), 8);
    mcp_call(&client, "index", json!({"knowledge":"pause"})).await;
    mcp_call(&client, "index", json!({"knowledge":"maintain"})).await;
    mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["state"] == "budget_wait"
    })
    .await;
    assert_eq!(
        server.calls(),
        8,
        "resuming does not reset the owner's window"
    );
    stop_mcp(client, &mut env).await;
    let restarted = launch_mcp(&mut env, true, false).await;
    let ready = mcp_until(&restarted, caught_up).await;
    assert_eq!(server.calls(), 9);
    assert_eq!(
        ready["knowledge"]["maintenance"]["last_run"]["reused_current"],
        8
    );
    stop_mcp(restarted, &mut env).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_watcher_accepts_edits_during_http_and_rejects_the_stale_completion() {
    let release = Arc::new(AtomicBool::new(false));
    let gate = release.clone();
    let server = Server::new(move |_, call| {
        if call == 1 {
            let until = Instant::now() + Duration::from_secs(10);
            while !gate.load(Ordering::SeqCst) && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        (
            200,
            response("fixture-model", output()).to_string(),
            Duration::ZERO,
        )
    });
    let mut env = Env::new(&server);
    let client = launch_mcp(&mut env, true, true).await;
    let deadline = Instant::now() + Duration::from_secs(5);
    while server.calls() == 0 {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let before: Value =
        serde_json::from_str(&mcp_call(&client, "status", json!({})).await).unwrap();
    std::fs::write(
        env.dir.path().join("root/src/a.rs"),
        A.replace("remove(0)", "remove(1)"),
    )
    .unwrap();
    mcp_until(&client, |s| {
        s["source_revision"].as_u64() > before["source_revision"].as_u64()
    })
    .await;
    let context = mcp_call(&client, "context", json!({"query":"opaque","tokens":2048})).await;
    assert!(
        context.contains("remove(1)"),
        "source acceptance proceeds during HTTP: {context}"
    );
    release.store(true, Ordering::SeqCst);
    mcp_until(&client, caught_up).await;
    assert_eq!(server.calls(), 5, "only the stale child call is repeated");
    stop_mcp(client, &mut env).await;
    assert_eq!(env.records().len(), 4);
    let current_hash = context_foundry::digest(A.replace("remove(0)", "remove(1)").as_bytes());
    for record in env.records() {
        if let Some(hash) = record.dependencies.get("src/a.rs") {
            assert_eq!(hash, &current_hash);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_shutdown_closes_generation_without_waiting_for_external_compute() {
    let server = Server::new(|_, _| {
        (
            200,
            response("fixture-model", output()).to_string(),
            Duration::from_secs(4),
        )
    });
    let mut env = Env::new(&server);
    let client = launch_mcp(&mut env, true, false).await;
    let deadline = Instant::now() + Duration::from_secs(5);
    while server.calls() == 0 {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let started = Instant::now();
    stop_mcp(client, &mut env).await;
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "owner waited on external compute"
    );
    assert_eq!(
        env.engine().knowledge_status().unwrap()["cache"]["entries"],
        0
    );
    assert!(
        env.engine()
            .derivation_pending(Lane::Knowledge)
            .unwrap()
            .reconcile_required
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_http_delivers_primary_derived_context_across_roots_and_stops_cleanly() {
    use context_foundry::mcp::{HttpOptions, ServerOptions, serve_http};
    use rmcp::{
        ServiceExt,
        transport::{
            StreamableHttpClientTransport,
            streamable_http_client::StreamableHttpClientTransportConfig,
        },
    };
    let server = Server::good();
    let mut env = Env::new(&server);
    let reference = env.dir.path().join("reference");
    let reference_store = env.dir.path().join("reference-store");
    std::fs::create_dir(&reference).unwrap();
    let mut engine = Engine::initialize(&reference_store, &reference).unwrap();
    engine
        .replace_source("extra.rs", "fn reference_only() {}\n")
        .unwrap();
    engine.refresh(&Control::unbounded()).unwrap();
    drop(engine);
    let profile = env.dir.path().join("http-knowledge.json");
    std::fs::write(&profile, serde_json::to_vec(&env.profile).unwrap()).unwrap();
    env.engine = None;
    const ENV: &str = "FOUNDRY_KNOWLEDGE_HTTP_FIXTURE_TOKEN";
    unsafe {
        std::env::set_var(ENV, "knowledge-fixture-token");
    }
    let shutdown = tokio_util::sync::CancellationToken::new();
    let mut serving = serve_http(
        ServerOptions {
            retrieval_trace_dir: None,
            experimental_behavior_context: false,
            store: env.store.clone(),
            root: env.dir.path().join("root"),
            references: vec![context_foundry::roots::ReferenceSpec {
                root: reference,
                store: reference_store,
            }],
            budget: Default::default(),
            no_memory: false,
            experimental_task_context: false,
            watch: false,
            semantic: None,
            policy: None,
            model_resources: None,
            knowledge: Some(knowledge::maintenance::Serving {
                profile,
                maintain: true,
            }),
        },
        HttpOptions {
            port: 0,
            token_env: ENV.into(),
            keep_alive: Duration::from_secs(30),
            shutdown: shutdown.clone(),
        },
    )
    .await
    .unwrap();
    let config =
        StreamableHttpClientTransportConfig::with_uri(format!("http://{}/mcp", serving.address))
            .auth_header("knowledge-fixture-token");
    let client = ()
        .serve(StreamableHttpClientTransport::with_client(
            reqwest::Client::new(),
            config,
        ))
        .await
        .unwrap();
    mcp_until(&client, caught_up).await;
    let text = mcp_call(
        &client,
        "context",
        json!({"query":"capacity eviction","include_briefs":true,"tokens":8192}),
    )
    .await;
    assert!(
        text.contains("brief:") && text.contains("q.remove(0)") && text.contains("ref1"),
        "{text}"
    );
    let rejected = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("context").with_arguments(
                json!({"query":"capacity eviction","include_briefs":true,"roots":["ref1"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(rejected.is_error, Some(true));
    client.cancel().await.unwrap();
    shutdown.cancel();
    tokio::time::timeout(Duration::from_secs(5), serving.done.changed())
        .await
        .unwrap()
        .unwrap();
    reopen_when_idle(&mut env).await;
    assert_eq!(env.records().len(), 4);
    assert_eq!(server.calls(), 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_provider_failure_pauses_even_when_later_nodes_exhaust_the_window() {
    let server = Server::new(|_, call| {
        (
            200,
            response(
                "fixture-model",
                if call == 8 {
                    json!({"invalid":true})
                } else {
                    output()
                },
            )
            .to_string(),
            Duration::ZERO,
        )
    });
    let mut env = Env::new(&server);
    env.profile.nodes = (0..9)
        .map(|i| Node {
            id: format!("leaf{i}"),
            sources: vec![env.profile.nodes[0].sources[0].clone()],
            children: vec![],
        })
        .collect();
    let client = launch_mcp(&mut env, true, false).await;
    let paused = mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["state"] == "paused"
    })
    .await;
    assert_eq!(
        paused["knowledge"]["maintenance"]["reason"],
        "knowledge_output_invalid"
    );
    assert_eq!(paused["knowledge"]["maintenance"]["continuous"], false);
    assert_eq!(server.calls(), 8);
    assert_eq!(paused["knowledge"]["cache"]["entries"], 7);
    stop_mcp(client, &mut env).await;
}

#[cfg(target_os = "linux")]
fn use_local(env: &mut Env) {
    use knowledge::local::{Config, Device, LLAMA_CPP, MemoryEnforcement};
    let executable = env.dir.path().join("generator");
    std::fs::copy(env!("CARGO_BIN_EXE_foundry-generate-fake"), &executable).unwrap();
    let executable_sha256 = context_foundry::digest(&std::fs::read(&executable).unwrap());
    let gguf = env.dir.path().join("fixture.gguf");
    std::fs::write(&gguf, b"synthetic model fixture").unwrap();
    env.profile.artifact_sha256 = context_foundry::digest(b"synthetic model fixture");
    env.profile.endpoint.clear();
    env.profile.local = Some(Config {
        development: true,
        executable,
        executable_sha256,
        gguf,
        llama_cpp: LLAMA_CPP.into(),
        chat_template: "chatml".into(),
        device: Device::Cpu,
        context_tokens: 4096,
        batch_tokens: 128,
        threads: 1,
        memory_bytes: 512 * 1024 * 1024,
        memory_enforcement: MemoryEnforcement::HardAddressSpace,
        cpu_seconds: 30,
        load_seconds: 10,
        idle_seconds: 1,
    });
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn failed_local_computation_keeps_bounded_usage_without_publishing_a_brief() {
    for (command, expected, unknown) in [
        ("failed_usage", "knowledge_output_limit", 0),
        ("failed_usage_invalid", "knowledge_provider_malformed", 1),
    ] {
        let server = Server::good();
        let mut env = Env::new(&server);
        use_local(&mut env);
        env.profile
            .instructions
            .push_str(&format!("\nfixture:{command}"));
        let report = env.run(1).await;
        assert_eq!(report.nodes[0].reason.as_deref(), Some(expected));
        assert_eq!(report.calls, 1);
        assert_eq!(report.generated, 0);
        assert_eq!(report.usage_unknown_calls, unknown);
        assert_eq!(
            report.observed_prompt_tokens,
            if unknown == 0 { 37 } else { 0 }
        );
        assert_eq!(
            report.observed_completion_tokens,
            if unknown == 0 { 128 } else { 0 }
        );
        assert!(env.records().is_empty());
        assert_eq!(server.calls(), 0);
    }
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_mcp_memory_policy_tracks_residency_and_cached_replay_bypasses_pressure() {
    let server = Server::good();
    let mut env = Env::new(&server);
    use_local(&mut env);
    env.profile.local.as_mut().unwrap().idle_seconds = 3;
    let policy = || model_resources::Config {
        resident_bytes: 512 * 1024 * 1024,
        reserve_bytes: 1024 * 1024,
        resume_margin_bytes: 1024 * 1024,
        background_cpu: None,
    };
    let client = launch_mcp_resources(&mut env, true, false, Some(policy())).await;
    let complete = mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["last_run"]["complete"] == true
    })
    .await;
    assert_eq!(
        complete["model_resources"]["reserved_generation_bytes"],
        512 * 1024 * 1024u64
    );
    assert!(complete["knowledge"]["maintenance"]["local_worker"]["pid"].is_number());
    let unloaded = mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["local_worker"]["pid"].is_null()
    })
    .await;
    assert_eq!(unloaded["model_resources"]["reserved_generation_bytes"], 0);
    assert_eq!(
        unloaded["knowledge"]["maintenance"]["local_worker"]["last_stop"],
        "idle"
    );
    stop_mcp(client, &mut env).await;
    // Source-bound cached/current data needs neither headroom nor model files.
    let local = env.profile.local.as_ref().unwrap();
    std::fs::remove_file(&local.gguf).unwrap();
    std::fs::remove_file(&local.executable).unwrap();
    let pressure = model_resources::Config {
        reserve_bytes: 1 << 60,
        ..policy()
    };
    let client = launch_mcp_resources(&mut env, true, false, Some(pressure)).await;
    let replay = mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["last_run"]["complete"] == true
    })
    .await;
    assert_eq!(
        replay["knowledge"]["maintenance"]["completed_pass_usage"]["calls"],
        0
    );
    assert_eq!(
        replay["knowledge"]["maintenance"]["local_worker"]["launches"],
        0
    );
    stop_mcp(client, &mut env).await;
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_mcp_memory_policy_defers_without_calls_and_pause_keeps_work() {
    let server = Server::good();
    let mut env = Env::new(&server);
    use_local(&mut env);
    let client = launch_mcp_resources(
        &mut env,
        true,
        false,
        Some(model_resources::Config {
            resident_bytes: 512 * 1024 * 1024,
            reserve_bytes: 1 << 60,
            resume_margin_bytes: 1024,
            background_cpu: None,
        }),
    )
    .await;
    let waiting = mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["state"] == "resource_wait"
    })
    .await;
    assert_eq!(
        waiting["knowledge"]["maintenance"]["reason"],
        "model_memory_pressure"
    );
    assert_eq!(
        waiting["knowledge"]["maintenance"]["local_worker"]["launches"],
        0
    );
    assert_eq!(waiting["model_resources"]["reserved_generation_bytes"], 0);
    let context = mcp_call(&client, "context", json!({"query":"opaque","tokens":2048})).await;
    assert!(context.contains("remove(0)"));
    let start = Instant::now();
    mcp_call(&client, "index", json!({"knowledge":"pause"})).await;
    mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["state"] == "paused"
    })
    .await;
    stop_mcp(client, &mut env).await;
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(env.records().is_empty());
    assert!(
        env.engine().knowledge_status().unwrap()["pending"]["paths"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(server.calls(), 0);
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_mcp_memory_budget_refuses_before_worker_or_usage_and_restart_recovers() {
    let server = Server::good();
    let mut env = Env::new(&server);
    use_local(&mut env);
    let client = launch_mcp_resources(
        &mut env,
        true,
        false,
        Some(model_resources::Config {
            resident_bytes: 1,
            reserve_bytes: 1,
            resume_margin_bytes: 1,
            background_cpu: None,
        }),
    )
    .await;
    let paused = mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["state"] == "paused"
    })
    .await;
    assert_eq!(
        paused["knowledge"]["maintenance"]["reason"],
        "model_memory_budget"
    );
    assert_eq!(
        paused["knowledge"]["maintenance"]["completed_pass_usage"]["calls"],
        0
    );
    assert_eq!(
        paused["knowledge"]["maintenance"]["local_worker"]["launches"],
        0
    );
    stop_mcp(client, &mut env).await;
    let client = launch_mcp(&mut env, true, false).await;
    mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["last_run"]["complete"] == true
    })
    .await;
    stop_mcp(client, &mut env).await;
    assert_eq!(env.records().len(), 4);
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "current_thread")]
async fn local_worker_enforces_os_denials_and_reuses_source_bound_outputs() {
    let server = Server::good();
    let mut env = Env::new(&server);
    use_local(&mut env);
    let outside = env.dir.path().join("private-sentinel");
    std::fs::write(&outside, "private fixture must never enter the worker").unwrap();
    env.profile.instructions = format!("fixture:probe:{}", outside.display());
    let prepared = env.run(8).await;
    assert!(prepared.complete, "{prepared:?}");
    assert_eq!(prepared.generated, 4);
    assert_eq!(server.calls(), 0);
    for record in env.records() {
        assert_eq!(
            record.brief.unknowns,
            vec![
                "probe outside=true network=true exec=true model=true thread=true env=true async_owner=true"
            ]
        );
    }
    let local = env.profile.local.as_ref().unwrap();
    std::fs::remove_file(&local.gguf).unwrap();
    std::fs::remove_file(&local.executable).unwrap();
    let reused = env.run(8).await;
    assert!(reused.complete, "{reused:?}");
    assert_eq!((reused.calls, reused.reused_current), (0, 4));
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "current_thread")]
async fn local_worker_memory_limit_and_invalid_artifacts_preserve_pending_work() {
    let server = Server::good();
    let mut env = Env::new(&server);
    use_local(&mut env);
    env.profile.instructions = "fixture:allocate".into();
    let limited = env.run(8).await;
    assert_eq!(limited.calls, 1, "{limited:?}");
    assert_eq!(
        limited.nodes[0].reason.as_deref(),
        Some("knowledge_resource_limit")
    );
    assert!(env.records().is_empty());
    env.engine().knowledge_purge_cache().unwrap();
    env.profile.instructions = "Use supplied evidence.".into();
    std::fs::write(&env.profile.local.as_ref().unwrap().gguf, b"changed").unwrap();
    let invalid = env.run(8).await;
    assert_eq!(invalid.calls, 0);
    assert_eq!(
        invalid.nodes[0].reason.as_deref(),
        Some("knowledge_artifact_invalid")
    );
    assert!(env.records().is_empty());
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_mcp_unloads_idle_models_and_reuses_verified_snapshots_after_edits() {
    let server = Server::good();
    let mut env = Env::new(&server);
    use_local(&mut env);
    let client = launch_mcp(&mut env, false, true).await;
    let before = mcp_call(&client, "status", json!({})).await;
    let before: Value = serde_json::from_str(&before).unwrap();
    assert_eq!(
        before["knowledge"]["maintenance"]["local_worker"]["launches"],
        0
    );
    mcp_call(&client, "index", json!({"knowledge":"maintain"})).await;
    let prepared = mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["last_run"]["complete"] == true
            && s["knowledge"]["maintenance"]["local_worker"]["pid"].is_null()
    })
    .await;
    let observed = &prepared["knowledge"]["maintenance"]["local_worker"];
    assert_eq!(observed["launches"], 1);
    assert_eq!(observed["calls"], 4);
    assert_eq!(observed["last_stop"], "idle");
    assert_eq!(observed["isolation"], "linux-seccomp-fd-v1");
    let verified = observed["verified_bytes"].clone();
    // Path removal cannot invalidate the owned snapshots or force another hash.
    let local = env.profile.local.as_ref().unwrap();
    std::fs::remove_file(&local.gguf).unwrap();
    std::fs::remove_file(&local.executable).unwrap();
    std::fs::write(
        env.dir.path().join("root/src/a.rs"),
        A.replace("remove(0)", "remove(1)"),
    )
    .unwrap();
    let edited = mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["completed_pass_usage"]["calls"] == 7
            && s["knowledge"]["maintenance"]["state"] == "waiting"
            && s["knowledge"]["maintenance"]["last_run"]["complete"] == true
    })
    .await;
    assert_eq!(
        edited["knowledge"]["maintenance"]["local_worker"]["launches"],
        2
    );
    assert_eq!(
        edited["knowledge"]["maintenance"]["local_worker"]["verified_bytes"],
        verified
    );
    assert_eq!(server.calls(), 0);
    stop_mcp(client, &mut env).await;
    assert_eq!(env.records().len(), 4);
    let client = launch_mcp(&mut env, true, false).await;
    let restarted = mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["last_run"]["complete"] == true
    })
    .await;
    assert_eq!(
        restarted["knowledge"]["maintenance"]["local_worker"]["launches"],
        0
    );
    stop_mcp(client, &mut env).await;
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_mcp_reports_live_limits_and_reaps_an_inflight_worker_on_shutdown() {
    let server = Server::good();
    let mut env = Env::new(&server);
    use_local(&mut env);
    env.profile.instructions = "fixture:hang".into();
    env.profile.call_seconds = 30;
    let client = launch_mcp(&mut env, true, false).await;
    let status = mcp_until(&client, |s| {
        s["knowledge"]["maintenance"]["local_worker"]["calls"] == 1
    })
    .await;
    let pid = status["knowledge"]["maintenance"]["local_worker"]["pid"]
        .as_u64()
        .unwrap() as u32;
    let limits = std::fs::read_to_string(format!("/proc/{pid}/limits")).unwrap();
    assert!(
        limits
            .lines()
            .any(|line| line.starts_with("Max cpu time") && line.contains("30")),
        "{limits}"
    );
    assert!(
        limits
            .lines()
            .any(|line| line.starts_with("Max address space") && line.contains("536870912")),
        "{limits}"
    );
    let state = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
    let cpu: usize = state
        .lines()
        .find(|line| line.starts_with("Cpus_allowed_list:"))
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .expect("one allowed CPU, not a range/list");
    let mut allowed = unsafe { std::mem::zeroed::<libc::cpu_set_t>() };
    assert_eq!(
        unsafe { libc::sched_getaffinity(0, std::mem::size_of_val(&allowed), &mut allowed) },
        0
    );
    assert!(unsafe { libc::CPU_ISSET(cpu, &allowed) });
    assert!(
        state
            .lines()
            .any(|line| line.starts_with("NoNewPrivs:") && line.ends_with('1')),
        "{state}"
    );
    let context = mcp_call(&client, "context", json!({"query":"opaque","tokens":2048})).await;
    assert!(context.contains("remove(0)"), "{context}");
    let start = Instant::now();
    stop_mcp(client, &mut env).await;
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "owner must reap the worker before releasing its store"
    );
    assert!(env.records().is_empty());
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "current_thread")]
async fn local_cpu_time_limit_terminates_compute_and_keeps_generation_pending() {
    let server = Server::good();
    let mut env = Env::new(&server);
    use_local(&mut env);
    env.profile.local.as_mut().unwrap().cpu_seconds = 1;
    env.profile.instructions = "fixture:spin".into();
    env.profile.call_seconds = 10;
    let start = Instant::now();
    let stopped = env.run(8).await;
    assert_eq!(stopped.calls, 1, "{stopped:?}");
    assert_eq!(
        stopped.nodes[0].reason.as_deref(),
        Some("knowledge_worker_exited")
    );
    assert!(start.elapsed() < Duration::from_secs(8));
    assert!(env.records().is_empty());
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "current_thread")]
async fn local_worker_rejects_bad_frames_identities_and_usage_before_publication() {
    for mode in ["oversize", "wrong_id", "usage"] {
        let server = Server::good();
        let mut env = Env::new(&server);
        use_local(&mut env);
        env.profile.instructions = format!("fixture:{mode}");
        let report = env.run(8).await;
        assert_eq!(report.calls, 1, "{mode}: {report:?}");
        assert_eq!(
            report.nodes[0].reason.as_deref(),
            Some("knowledge_provider_malformed"),
            "{mode}: {report:?}"
        );
        assert!(report.local_worker.as_ref().unwrap().pid.is_none());
        assert!(env.records().is_empty());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn local_artifact_failure_is_not_repeated_for_every_selected_node() {
    let server = Server::good();
    let mut env = Env::new(&server);
    use_local(&mut env);
    let path = &env.profile.local.as_ref().unwrap().gguf;
    std::fs::write(path, "bad").unwrap();
    let mut session = knowledge::local::Session::default();
    assert_eq!(
        session
            .prepare(&env.profile, &Control::unbounded())
            .unwrap_err()
            .code(),
        "knowledge_artifact_invalid"
    );
    std::fs::write(path, b"synthetic model fixture").unwrap();
    assert_eq!(
        session
            .prepare(&env.profile, &Control::unbounded())
            .unwrap_err()
            .code(),
        "knowledge_artifact_invalid",
        "a refused acquisition is not retried in the same run"
    );
    let mut resumed = knowledge::local::Session::default();
    resumed
        .prepare(&env.profile, &Control::unbounded())
        .unwrap();
    assert!(resumed.observation.verified_bytes > 0);
    assert_eq!(
        resumed.observation.launches, 0,
        "verification alone never starts computation"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn local_unsupported_gpu_and_memory_profiles_refuse_without_loading_artifacts() {
    let server = Server::good();
    let mut env = Env::new(&server);
    use_local(&mut env);
    let local = env.profile.local.as_mut().unwrap();
    local.gguf = env.dir.path().join("absent.gguf");
    local.device = knowledge::local::Device::Metal {
        layers: 8,
        allocation_bytes: 128 * 1024 * 1024,
        reserve_bytes: 64 * 1024 * 1024,
    };
    let mut session = knowledge::local::Session::default();
    assert_eq!(
        session
            .prepare(&env.profile, &Control::unbounded())
            .unwrap_err()
            .code(),
        "knowledge_isolation_unavailable"
    );
    let local = env.profile.local.as_mut().unwrap();
    local.device = knowledge::local::Device::Cpu;
    local.memory_enforcement = knowledge::local::MemoryEnforcement::SupervisedFootprint;
    assert_eq!(
        session
            .prepare(&env.profile, &Control::unbounded())
            .unwrap_err()
            .code(),
        "knowledge_isolation_unavailable"
    );
    assert_eq!(session.observation.launches, 0);
    assert_eq!(session.observation.verified_bytes, 0);
}

#[cfg(target_os = "linux")]
#[test]
fn local_owner_death_stops_the_worker_and_leaves_generation_recoverable() {
    let server = Server::good();
    let mut env = Env::new(&server);
    use_local(&mut env);
    env.profile.instructions = "fixture:hang".into();
    env.profile.call_seconds = 30;
    let path = env.dir.path().join("local.json");
    std::fs::write(&path, serde_json::to_vec(&env.profile).unwrap()).unwrap();
    env.engine = None;
    struct Owner(std::process::Child);
    impl Drop for Owner {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut owner = Owner(
        std::process::Command::new(env!("CARGO_BIN_EXE_foundry"))
            .arg("--store")
            .arg(&env.store)
            .args(["knowledge", "prepare", "--profile"])
            .arg(path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let pid = owner.0.id();
    let deadline = Instant::now() + Duration::from_secs(10);
    let worker = loop {
        assert!(
            owner.0.try_wait().unwrap().is_none(),
            "owner exited before worker readiness"
        );
        // Some kernels omit /proc/PID/task/TID/children. Match PPid from
        // ordinary process status instead; inspect no command/input contents.
        let child = std::fs::read_dir("/proc")
            .unwrap()
            .filter_map(Result::ok)
            .find_map(|entry| {
                let child = entry.file_name().to_str()?.parse::<u32>().ok()?;
                let state = std::fs::read_to_string(entry.path().join("status")).ok()?;
                (state.lines().any(|line| {
                    line.strip_prefix("PPid:")
                        .is_some_and(|s| s.trim() == pid.to_string())
                }) && state
                    .lines()
                    .any(|l| l.starts_with("NoNewPrivs:") && l.ends_with('1')))
                .then_some(child)
            });
        if let Some(child) = child {
            break child;
        }
        assert!(Instant::now() < deadline, "worker did not become isolated");
        std::thread::sleep(Duration::from_millis(20));
    };
    owner.0.kill().unwrap();
    owner.0.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        // This container's PID 1 may retain an orphan zombie; no live process
        // or compute may survive. Normal shutdown separately proves reaping.
        let running = std::fs::read_to_string(format!("/proc/{worker}/stat"))
            .is_ok_and(|s| !s.rsplit_once(')').unwrap().1.trim_start().starts_with('Z'));
        if !running {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "orphan generator is still running"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    env.reopen();
    assert!(env.records().is_empty());
    assert!(
        env.engine().knowledge_status().unwrap()["pending"]["paths"]
            .as_u64()
            .unwrap()
            > 0
    );
    env.profile.instructions = "Use supplied evidence.".into();
    let report = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(env.run(8));
    assert!(report.complete, "{report:?}");
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires an explicitly supplied real generation worker and GGUF; no model download in tests"]
async fn local_real_cpu_generation_accepts_a_cited_brief_and_reuses_it() {
    real_generation_lifecycle(
        knowledge::local::Device::Cpu,
        knowledge::local::MemoryEnforcement::HardAddressSpace,
    )
    .await;
}

#[cfg(target_os = "macos")]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires a signed development generator, pinned Qwen 0.5B GGUF, Metal and an explicit new receipt path"]
async fn local_real_metal_generation_accepts_a_cited_brief_and_reuses_it() {
    assert!(
        std::env::var_os("FOUNDRY_GENERATION_SMOKE_RECEIPT").is_some(),
        "set FOUNDRY_GENERATION_SMOKE_RECEIPT to a new absolute path outside the checkout"
    );
    real_generation_lifecycle(
        knowledge::local::Device::Metal {
            // The pinned fixture has 24 repeating layers plus the output layer.
            layers: 25,
            allocation_bytes: 1024 * 1024 * 1024,
            reserve_bytes: 256 * 1024 * 1024,
        },
        knowledge::local::MemoryEnforcement::SupervisedFootprint,
    )
    .await;
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn real_generation_lifecycle(
    device: knowledge::local::Device,
    memory_enforcement: knowledge::local::MemoryEnforcement,
) {
    use knowledge::local::{Config, Device, LLAMA_CPP};
    let started = Instant::now();
    let metal = matches!(device, Device::Metal { .. });
    let label = if metal { "Metal" } else { "CPU" };
    // Fail before loading weights if the explicitly selected output is unsafe.
    // The receipt never replaces a file or writes inside this source checkout.
    let mut receipt = std::env::var_os("FOUNDRY_GENERATION_SMOKE_RECEIPT").map(|path| {
        let path = PathBuf::from(path);
        assert!(path.is_absolute(), "receipt path must be absolute");
        let parent = path.parent().unwrap().canonicalize().unwrap();
        let checkout = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .canonicalize()
            .unwrap();
        assert!(
            !parent.starts_with(checkout),
            "receipt must stay outside the checkout"
        );
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
    });
    let server = Server::good();
    let mut env = Env::new(&server);
    let executable: PathBuf = std::env::var_os("FOUNDRY_GENERATION_SMOKE_WORKER")
        .expect("worker path")
        .into();
    let gguf: PathBuf = std::env::var_os("FOUNDRY_GENERATION_SMOKE_GGUF")
        .expect("GGUF path")
        .into();
    let executable_sha256 = context_foundry::neural::profile::hash_regular_file(&executable)
        .unwrap()
        .0;
    env.profile.artifact_sha256 = context_foundry::neural::profile::hash_regular_file(&gguf)
        .unwrap()
        .0;
    if metal {
        assert_eq!(
            env.profile.artifact_sha256,
            "1d9614638d18024d0fbb36575a15f1302a3adf044df10345688ec4f6e1c4ff32",
            "native acceptance uses the explicitly pinned small fixture"
        );
    }
    env.profile.endpoint.clear();
    env.profile.local = Some(Config {
        development: true,
        executable,
        executable_sha256,
        gguf,
        llama_cpp: LLAMA_CPP.into(),
        chat_template: "chatml".into(),
        device,
        context_tokens: 4096,
        batch_tokens: 128,
        threads: 2,
        memory_bytes: 2 * 1024 * 1024 * 1024,
        memory_enforcement,
        cpu_seconds: 180,
        load_seconds: 90,
        idle_seconds: 1,
    });
    env.profile.nodes.truncate(1);
    env.profile.nodes[0].sources.truncate(1);
    env.profile.instructions = "Summarize only what the function does. Write one short claim for purpose and one for behavior, each with citations [0] and identifiers []. Leave provides, consumes, constraints, tests and unknowns empty.".into();
    env.profile.model = "Qwen2.5-Coder-0.5B-Instruct-Q4_K_M-smoke".into();
    env.profile.call_seconds = 90;
    let initial_profile = env.profile.clone();
    let report = knowledge::prepare(
        env.engine.as_mut().unwrap(),
        &env.profile,
        RunLimits {
            calls: 1,
            request_tokens: 100_000,
            output_tokens: 10_000,
        },
        &Control::with_deadline(Instant::now() + Duration::from_secs(180)),
    )
    .await
    .unwrap();
    eprintln!(
        "real {label} generation: {}",
        serde_json::to_string(&report).unwrap()
    );
    assert!(report.complete, "{report:?}");
    assert_eq!((report.calls, report.generated), (1, 1));
    assert!(report.observed_prompt_tokens > 0 && report.observed_completion_tokens > 0);
    assert_eq!(report.usage_unknown_calls, 0);
    assert_eq!(report.local_worker.as_ref().unwrap().launches, 1);
    assert_eq!(
        env.engine().knowledge_status().unwrap()["cache"]["entries"],
        1
    );
    let model_path = env.profile.local.as_ref().unwrap().gguf.clone();
    // A fully current replay must not need model files or worker acquisition.
    env.profile.local.as_mut().unwrap().gguf = env.dir.path().join("absent-model.gguf");
    let replay = env.run(1).await;
    assert!(replay.complete, "{replay:?}");
    assert_eq!(replay.calls, 0);
    assert_eq!(replay.local_worker.as_ref().unwrap().launches, 0);
    env.profile.local.as_mut().unwrap().gguf = model_path;
    env.engine()
        .replace_source("src/a.rs", &A.replace("remove(0)", "remove(1)"))
        .unwrap();
    let changed = knowledge::prepare(
        env.engine.as_mut().unwrap(),
        &env.profile,
        RunLimits {
            calls: 1,
            request_tokens: 100_000,
            output_tokens: 10_000,
        },
        &Control::with_deadline(Instant::now() + Duration::from_secs(180)),
    )
    .await
    .unwrap();
    eprintln!(
        "real {label} changed generation: {}",
        serde_json::to_string(&changed).unwrap()
    );
    assert!(changed.complete, "{changed:?}");
    assert_eq!((changed.calls, changed.generated), (1, 1));
    assert_eq!(changed.usage_unknown_calls, 0);
    assert!(changed.observed_prompt_tokens > 0 && changed.observed_completion_tokens > 0);
    assert_eq!(
        env.engine().knowledge_status().unwrap()["cache"]["entries"],
        2
    );
    env.engine().replace_source("src/a.rs", A).unwrap();
    env.reopen();
    env.profile.local.as_mut().unwrap().gguf = env.dir.path().join("absent-model.gguf");
    let restored = env.run(1).await;
    eprintln!(
        "real {label} restart/revert: {}",
        serde_json::to_string(&restored).unwrap()
    );
    assert!(restored.complete, "{restored:?}");
    assert_eq!((restored.calls, restored.reused_cached), (0, 1));
    assert_eq!(restored.local_worker.as_ref().unwrap().launches, 0);
    assert_eq!(server.calls(), 0);
    for generated in [&report, &changed] {
        let worker = generated.local_worker.as_ref().unwrap();
        assert_eq!((worker.launches, worker.calls), (1, 1));
        assert!(worker.verified_bytes > 0);
        assert!(worker.peak_memory_bytes > 0);
        assert!(worker.peak_memory_bytes <= 2 * 1024 * 1024 * 1024);
        assert_eq!(
            worker.pid, None,
            "the supervisor must reap before returning"
        );
        assert_eq!(worker.last_stop.as_deref(), Some("stopped"));
        assert_eq!(
            worker.isolation.as_deref(),
            Some(if metal {
                "macos-app-sandbox-development-v1"
            } else {
                "linux-seccomp-fd-v1"
            })
        );
        if metal {
            // Explicit GPU selection plus observed allocation prevents a CPU-only
            // smoke result from passing as native Metal acceptance. This does not
            // assert that every llama.cpp operation executes on the GPU.
            assert!(worker.peak_gpu_bytes > 0);
            assert!(worker.peak_gpu_bytes <= 1024 * 1024 * 1024);
        } else {
            assert_eq!(worker.peak_gpu_bytes, 0);
        }
    }
    for reused in [&replay, &restored] {
        let worker = reused.local_worker.as_ref().unwrap();
        assert_eq!(
            (worker.launches, worker.calls, worker.verified_bytes),
            (0, 0, 0)
        );
        assert_eq!(worker.pid, None);
        assert_eq!((worker.peak_memory_bytes, worker.peak_gpu_bytes), (0, 0));
    }
    if let Some(file) = &mut receipt {
        let evidence = json!({
            "v": 1,
            "status": "passed",
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "elapsed_seconds": started.elapsed().as_secs_f64(),
            "scope": "owned generation, source change, current and restart/revert reuse",
            "worker_sha256": initial_profile.local.as_ref().unwrap().executable_sha256,
            "artifact_sha256": initial_profile.artifact_sha256,
            "config": initial_profile.local,
            "model": initial_profile.model,
            "instructions": initial_profile.instructions,
            "output_tokens": initial_profile.output_tokens,
            "call_seconds": initial_profile.call_seconds,
            "fixture_source_sha256": context_foundry::digest(A.as_bytes()),
            "changed_source_sha256": context_foundry::digest(A.replace("remove(0)", "remove(1)").as_bytes()),
            "initial": report,
            "current": replay,
            "changed": changed,
            "restored": restored,
            "http_calls": server.calls(),
            "not_accepted": [
                "generated prose correctness or coding-task benefit",
                "all operations on GPU or performance comparison",
                "combined-model pressure or global GPU headroom",
                "owner-death or adversarial isolation acceptance",
                "Developer ID signed installed package"
            ]
        });
        serde_json::to_writer_pretty(&mut *file, &evidence).unwrap();
        file.write_all(b"\n").unwrap();
        file.sync_all().unwrap();
    }
}
