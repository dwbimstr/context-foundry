//! Real HTTP transport, CLI, database, cache and retrieval; synthetic completions.
//! No real-model quality, weight attestation or provider-economics claim.
use context_foundry::fault::{self, Action, names};
use context_foundry::neural::work::Lane;
use context_foundry::{
    Control, Engine, FoundryError, brief,
    knowledge::{self, Node, Profile, RunLimits, SourceSpan},
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
async fn deadline_closes_the_call_and_missing_usage_stays_unknown() {
    let server = Server::new(|_, _| {
        (
            200,
            response("fixture-model", output()).to_string(),
            Duration::from_millis(250),
        )
    });
    let mut env = Env::new(&server);
    let started = Instant::now();
    let report = knowledge::prepare(
        env.engine.as_mut().unwrap(),
        &env.profile,
        RunLimits {
            calls: 8,
            request_tokens: 100_000,
            output_tokens: 10_000,
        },
        &Control::with_deadline(started + Duration::from_millis(80)),
    )
    .await
    .unwrap();
    assert!(!report.complete);
    assert!(started.elapsed() < Duration::from_secs(1));
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
    assert!(Engine::upgrade_store(&env.store, 10, &Control::unbounded()).is_err());
    fault::disarm_all();
    assert_eq!(context_foundry::testkit::schema_marker(&env.store), "9");
    Engine::upgrade_store(&env.store, 10, &Control::unbounded()).unwrap();
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
