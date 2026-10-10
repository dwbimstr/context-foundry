//! 001 T011: real native events through the production MCP owner and SDK.
use context_foundry::{Control, Engine, testkit::parse_v2};
use rmcp::{ServiceExt, model::CallToolRequestParams, transport::TokioChildProcess};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Stdio, time::Duration};

type Client = rmcp::service::RunningService<rmcp::RoleClient, ()>;

#[tokio::test]
async fn shared_http_watches_only_primary_and_failed_bind_releases_store() {
    use context_foundry::mcp::{HttpOptions, ServerOptions, serve_http};
    use rmcp::transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    };
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let store = dir.path().join("store");
    let reference = dir.path().join("reference");
    let reference_store = dir.path().join("reference-store");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&reference).unwrap();
    fs::write(root.join("a.rs"), "fn primary_before() {}\n").unwrap();
    fs::write(reference.join("a.rs"), "fn reference_before() {}\n").unwrap();
    drop(Engine::initialize(&store, &root).unwrap());
    let mut engine = Engine::initialize(&reference_store, &reference).unwrap();
    engine.index(&reference, &Control::unbounded()).unwrap();
    drop(engine);
    // Unique, constant test-only key; no other test or child reads this variable.
    const ENV: &str = "FOUNDRY_WATCH_TEST_TOKEN";
    unsafe { std::env::set_var(ENV, "watch-test-token") };
    let options = || ServerOptions {
        model_resources: None,
        knowledge: None,
        retrieval_trace_dir: None,
        experimental_behavior_context: false,
        store: store.clone(),
        root: root.clone(),
        references: vec![context_foundry::roots::ReferenceSpec {
            root: reference.clone(),
            store: reference_store.clone(),
        }],
        budget: Default::default(),
        no_memory: false,
        experimental_task_context: false,
        watch: true,
        semantic: None,
        policy: None,
    };
    let shutdown = tokio_util::sync::CancellationToken::new();
    let http = |port| HttpOptions {
        port,
        token_env: ENV.into(),
        keep_alive: Duration::from_secs(30),
        shutdown: shutdown.clone(),
    };
    let occupied = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    assert!(
        serve_http(options(), http(occupied.local_addr().unwrap().port()))
            .await
            .is_err()
    );
    drop(Engine::open_existing(&store).expect("failed launch cannot retain a watching owner"));
    let mut serve = serve_http(options(), http(0)).await.unwrap();
    let config =
        StreamableHttpClientTransportConfig::with_uri(format!("http://{}/mcp", serve.address))
            .auth_header("watch-test-token");
    let client = ()
        .serve(StreamableHttpClientTransport::with_client(
            reqwest::Client::new(),
            config,
        ))
        .await
        .unwrap();
    until(&client, |s| idle(s) && s["source_count"] == 1).await;
    fs::write(root.join("a.rs"), "fn primary_after() {}\n").unwrap();
    fs::write(reference.join("a.rs"), "fn reference_after() {}\n").unwrap();
    until(&client, |s| idle(s) && s["watch"]["changed"] == 2).await;
    assert_eq!(handles(&client, "primary_after").await.len(), 1);
    client.cancel().await.unwrap();
    shutdown.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(15), serve.done.changed())
        .await
        .expect("HTTP owner stopped");
    let primary = Engine::open_existing(&store).unwrap();
    let reference = Engine::open_existing(&reference_store).unwrap();
    assert_eq!(
        primary.source("a.rs").unwrap().unwrap().hash,
        context_foundry::digest(b"fn primary_after() {}\n")
    );
    assert_eq!(
        reference.source("a.rs").unwrap().unwrap().hash,
        context_foundry::digest(b"fn reference_before() {}\n")
    );
}

async fn launch(store: &Path, root: &Path, watch: bool) -> (Client, u32) {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_foundry"));
    command
        .arg("--store")
        .arg(store)
        .arg("mcp")
        .arg("--root")
        .arg(root);
    if watch {
        command.arg("--watch");
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let transport = TokioChildProcess::new(command).unwrap();
    let pid = transport.id().unwrap();
    (().serve(transport).await.unwrap(), pid)
}

async fn call(client: &Client, tool: &'static str, args: Value) -> (bool, String) {
    let result = tokio::time::timeout(
        Duration::from_secs(35),
        client.call_tool(
            CallToolRequestParams::new(tool).with_arguments(args.as_object().unwrap().clone()),
        ),
    )
    .await
    .expect("foreground tool deadline")
    .unwrap();
    let rmcp::model::ContentBlock::Text(text) = &result.content[0] else {
        panic!("expected text");
    };
    (result.is_error == Some(true), text.text.clone())
}
async fn status(client: &Client) -> Value {
    let (error, text) = call(client, "status", json!({})).await;
    assert!(!error, "foreground status failed: {text}");
    serde_json::from_str(&text).unwrap()
}
fn idle(s: &Value) -> bool {
    s["watch"]["phase"] == "idle"
        && s["watch"]["pending_paths"] == 0
        && s["watch"]["dirty_root"] == false
        && s["watch"]["pending_index"] == 0
}
async fn until(client: &Client, predicate: impl Fn(&Value) -> bool) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(180);
    loop {
        let s = status(client).await;
        if predicate(&s) {
            assert!(s["watch"]["last_error"].is_null(), "{s}");
            assert!(s["watch"]["backend_error"].is_null(), "{s}");
            return s;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "watch did not converge: {s}"
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}
async fn handles(client: &Client, query: &str) -> Vec<String> {
    let (error, text) = call(
        client,
        "search",
        json!({"query":query,"limit":32,"tokens":4096}),
    )
    .await;
    assert!(!error, "{text}");
    parse_v2(&text)
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.handle)
        .collect()
}
async fn closed(client: Client, store: &Path) -> Engine {
    client.cancel().await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        match Engine::open_existing(store) {
            Ok(engine) => return engine,
            Err(error) => {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "owner did not release store: {error}"
                );
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }
    }
}

#[tokio::test]
async fn native_changes_rename_directories_ignore_rules_and_stale_handles_converge() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let store = dir.path().join("store");
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join("a.rs"), "pub fn original_marker() {}\n").unwrap();
    drop(Engine::initialize(&store, &root).unwrap());
    let (client, _) = launch(&store, &root, true).await;
    until(&client, |s| idle(s) && s["source_count"] == 1).await;
    let old = handles(&client, "original_marker").await;
    assert_eq!(old.len(), 1);

    // Atomic-save rename over an existing file must refresh the accepted bytes.
    fs::write(root.join(".save"), "pub fn replaced_marker() {}\n").unwrap();
    fs::rename(root.join(".save"), root.join("a.rs")).unwrap();
    until(&client, |s| {
        idle(s) && s["watch"]["changed"].as_u64().unwrap() >= 2
    })
    .await;
    let (error, text) = call(&client, "retrieve", json!({"handle":old[0],"tokens":512})).await;
    assert!(error && text.contains("stale_handle"), "{text}");
    assert_eq!(handles(&client, "replaced_marker").await.len(), 1);

    fs::create_dir(root.join("src")).unwrap();
    for i in 0..12 {
        fs::write(
            root.join(format!("src/f{i}.rs")),
            format!("pub fn directory_marker_{i}() {{}}\n"),
        )
        .unwrap();
    }
    until(&client, |s| idle(s) && s["source_count"] == 13).await;
    fs::rename(root.join("src"), root.join("moved")).unwrap();
    until(&client, |s| {
        idle(s) && s["watch"]["deleted"].as_u64().unwrap() >= 12
    })
    .await;
    let moved = handles(&client, "directory_marker_0").await;
    assert!(
        moved.iter().any(|h| h.starts_with("moved/f0.rs#")),
        "{moved:?}"
    );
    assert!(moved.iter().all(|h| h.starts_with("moved/")), "{moved:?}");

    // Ignore-policy changes require a root reconciliation, including removals.
    let prior = status(&client).await["watch"]["full_reconciliations"]
        .as_u64()
        .unwrap();
    fs::write(root.join(".gitignore"), "moved/\n").unwrap();
    until(&client, |s| {
        idle(s)
            && s["source_count"] == 1
            && s["watch"]["full_reconciliations"].as_u64().unwrap() > prior
    })
    .await;
    assert!(handles(&client, "directory_marker_0").await.is_empty());
    fs::write(root.join(".gitignore"), "").unwrap();
    until(&client, |s| idle(s) && s["source_count"] == 13).await;
    fs::remove_dir_all(root.join("moved")).unwrap();
    fs::remove_file(root.join("a.rs")).unwrap();
    until(&client, |s| idle(s) && s["source_count"] == 0).await;
    assert!(handles(&client, "replaced_marker").await.is_empty());
    let engine = closed(client, &store).await;
    assert_eq!(engine.status().unwrap().source_count, 0);
    assert_eq!(engine.status().unwrap().scan_state, "complete");
}

#[tokio::test]
async fn six_thousand_files_allow_foreground_calls_and_no_idle_body_rescans() {
    const FILES: u64 = 6000;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let store = dir.path().join("store");
    fs::create_dir(&root).unwrap();
    for i in 0..FILES {
        fs::write(
            root.join(format!("f{i:04}.rs")),
            format!("pub fn marker_{i}() {{}}\n"),
        )
        .unwrap();
    }
    drop(Engine::initialize(&store, &root).unwrap());
    let (client, _) = launch(&store, &root, true).await;
    let partial = until(&client, |s| {
        let count = s["source_count"].as_u64().unwrap();
        count > 0 && count < FILES
    })
    .await;
    assert_ne!(
        partial["watch"]["phase"], "idle",
        "foreground served during reconciliation"
    );
    let baseline = until(&client, |s| idle(s) && s["source_count"] == FILES).await;
    assert_eq!(baseline["watch"]["source_reads"], FILES);
    let reads = baseline["watch"]["source_reads"].as_u64().unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(status(&client).await["watch"]["source_reads"], reads);
    fs::write(root.join("f3000.rs"), "pub fn edited_marker() {}\n").unwrap();
    let changed = until(&client, |s| idle(s) && s["watch"]["changed"] == FILES + 1).await;
    // Native backends may emit duplicate hints, but never reread the repository.
    assert!(
        changed["watch"]["source_reads"].as_u64().unwrap() <= reads + 3,
        "{changed}"
    );
    assert_eq!(
        changed["watch"]["full_reconciliations"],
        baseline["watch"]["full_reconciliations"]
    );
    assert_eq!(handles(&client, "edited_marker").await.len(), 1);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        status(&client).await["watch"]["source_reads"],
        changed["watch"]["source_reads"]
    );
    let engine = closed(client, &store).await;
    assert_eq!(
        engine.source("f3000.rs").unwrap().unwrap().hash,
        context_foundry::digest(b"pub fn edited_marker() {}\n")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn abrupt_owner_exit_recovers_offline_changes_and_watch_is_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let store = dir.path().join("store");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("a.rs"), "fn before_restart() {}\n").unwrap();
    fs::write(root.join("deleted.rs"), "fn deleted_marker() {}\n").unwrap();
    let mut engine = Engine::initialize(&store, &root).unwrap();
    engine.index(&root, &Control::unbounded()).unwrap();
    drop(engine);
    let (client, _) = launch(&store, &root, false).await;
    assert!(status(&client).await.get("watch").is_none());
    fs::write(root.join("a.rs"), "fn before_crash() {}\n").unwrap();
    let engine = closed(client, &store).await;
    assert_eq!(
        engine.source("a.rs").unwrap().unwrap().hash,
        context_foundry::digest(b"fn before_restart() {}\n")
    );
    drop(engine);
    let (client, pid) = launch(&store, &root, true).await;
    until(&client, idle).await;
    assert_eq!(handles(&client, "before_crash").await.len(), 1);
    // Kill only the child this test owns, to lose its disposable hint queue.
    assert_eq!(unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) }, 0);
    drop(client);
    fs::write(root.join("a.rs"), "fn recovered_marker() {}\n").unwrap();
    fs::remove_file(root.join("deleted.rs")).unwrap();
    fs::write(root.join("new.rs"), "fn new_marker() {}\n").unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(engine) = Engine::open_existing(&store) {
            drop(engine);
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let (client, _) = launch(&store, &root, true).await;
    until(&client, |s| idle(s) && s["source_count"] == 2).await;
    assert_eq!(handles(&client, "recovered_marker").await.len(), 1);
    assert!(handles(&client, "deleted_marker").await.is_empty());
    let engine = closed(client, &store).await;
    assert!(engine.source("new.rs").unwrap().is_some());
    assert!(engine.source("deleted.rs").unwrap().is_none());
}
