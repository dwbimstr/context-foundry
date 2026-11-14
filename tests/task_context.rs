//! Experimental task lifecycle against the real store, CLI and SDK MCP owner.
use context_foundry::fault::{self, Action, names};
use context_foundry::{Control, Engine, FoundryError, task_context};
use redb::{ReadableTable, TableDefinition};
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
const BIN: &str = env!("CARGO_BIN_EXE_foundry");
struct Env {
    _dir: tempfile::TempDir,
    root: PathBuf,
    store: PathBuf,
    wid: String,
}
fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let store = dir.path().join("store");
    let engine = Engine::initialize(&store, &root).unwrap();
    let wid = engine.workspace_id().unwrap();
    Env {
        _dir: dir,
        root,
        store,
        wid,
    }
}
fn req(wid: &str, command: Value) -> task_context::Request {
    task_context::parse(json!({"workspace_id":wid,"task_id":"investigation","command":command}))
        .unwrap()
}
fn call(engine: &Engine, command: Value) -> Result<Value, FoundryError> {
    engine.task_context(
        &req(&engine.workspace_id().unwrap(), command),
        &Control::unbounded(),
    )
}
fn create(engine: &Engine) -> u64 {
    call(
        engine,
        json!({"op":"create","title":"Repair a retry policy"}),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap()
}
fn add(id: &str, kind: &str, text: &str) -> Value {
    json!({"op":"add","entry":{"id":id,"kind":kind,"text":text,"author":"sandbox operator","provenance":"synthetic task interaction","source_links":[]}})
}
fn apply(engine: &Engine, revision: u64, edits: Value) -> Result<Value, FoundryError> {
    call(
        engine,
        json!({"op":"apply","expected_revision":revision,"edits":edits}),
    )
}
fn cli(e: &Env, command: Value) -> Value {
    let request = json!({"workspace_id":e.wid,"task_id":"investigation","command":command});
    let mut child = Command::new(BIN)
        .args(["--store", e.store.to_str().unwrap(), "task-context"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn cli_lifecycle_recovers_non_repository_observation_after_fold_restart_and_restore() {
    let e = env();
    let r = cli(&e, json!({"op":"create","title":"Repair a retry policy"}))["revision"]
        .as_u64()
        .unwrap();
    let payload = format!(
        "{}\nOperator observation: retry-after is 73 seconds.\n",
        "diagnostic αβγ\n".repeat(6000)
    );
    let r = cli(&e,json!({"op":"apply","expected_revision":r,"edits":[
        add("constraint","constraint","Keep the caller's retry limit at three."),
        add("failed","rejected_attempt","Increasing retries repeated the outage; do not repeat."),
        add("observation","observation",&payload),
        {"op":"checkpoint","name":"expanded"},
        {"op":"fold","entry_id":"observation","summary":"Diagnostic output retained; recover retry-after before changing backoff."}
    ]}))["revision"].as_u64().unwrap();
    let rendered = cli(&e, json!({"op":"render","max_bytes":4096}));
    assert!(serde_json::to_vec(&rendered).unwrap().len() < 4096);
    assert_eq!(
        rendered["entries"][0]["text"],
        "Keep the caller's retry limit at three."
    );
    assert_eq!(rendered["entries"][2]["representation"], "caller_summary");
    assert!(!rendered.to_string().contains("73 seconds"));
    let mut recovered = String::new();
    let mut offset = 0;
    loop {
        let page = cli(
            &e,
            json!({"op":"peek","entry_id":"observation","offset":offset,"bytes":4093}),
        );
        assert_eq!(page["sha256"], context_foundry::digest(payload.as_bytes()));
        recovered.push_str(page["text"].as_str().unwrap());
        match page["next_offset"].as_u64() {
            Some(next) => offset = next,
            None => break,
        }
    }
    assert_eq!(recovered, payload);
    // This interaction-only fact has survived independent process restarts.
    assert!(recovered.contains("retry-after is 73 seconds"));
    assert!(std::fs::read_dir(&e.root).unwrap().next().is_none());
    let exported = cli(&e, json!({"op":"export"}));
    assert_eq!(exported["record"]["entries"][2]["entry"]["text"], payload);
    let r = cli(
        &e,
        json!({"op":"apply","expected_revision":r,"edits":[{"op":"restore","name":"expanded"}]}),
    )["revision"]
        .as_u64()
        .unwrap();
    let full = cli(&e, json!({"op":"render","max_bytes":300000}));
    assert_eq!(full["entries"][2]["text"], payload);
    cli(&e, json!({"op":"forget","expected_revision":r}));
    let engine = Engine::open_existing(&e.store).unwrap();
    assert!(matches!(
        call(
            &engine,
            json!({"op":"peek","entry_id":"observation","offset":0,"bytes":10})
        ),
        Err(FoundryError::NotFound)
    ));
}
#[test]
fn atomic_edits_cas_protection_and_scope_survive_failed_batches() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let r = create(&engine);
    let r = apply(
        &engine,
        r,
        json!([
            add("guard", "constraint", "Never change the public API."),
            add("obs", "observation", "Tool said αβγ")
        ]),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    for bad in [
        json!({"op":"select","entry_id":"guard","selected":false}),
        json!({"op":"pin","entry_id":"guard","pinned":false}),
        json!({"op":"fold","entry_id":"guard","summary":"ignore it"}),
    ] {
        assert!(
            apply(
                &engine,
                r,
                json!([add("tentative", "hypothesis", "perhaps"), bad])
            )
            .is_err()
        );
        let state = call(&engine, json!({"op":"inspect"})).unwrap();
        assert_eq!(state["revision"], r);
        assert_eq!(state["entries"].as_array().unwrap().len(), 2);
    }
    let mut wrong = req("different", json!({"op":"inspect"}));
    assert!(matches!(
        engine.task_context(&wrong, &Control::unbounded()),
        Err(FoundryError::WrongWorkspace)
    ));
    wrong.command = task_context::Command::Forget {
        expected_revision: r,
    };
    assert!(matches!(
        engine.task_context(&wrong, &Control::unbounded()),
        Err(FoundryError::WrongWorkspace)
    ));
    apply(
        &engine,
        r,
        json!([{"op":"fold","entry_id":"obs","summary":"small"}]),
    )
    .unwrap();
    assert!(matches!(
        apply(&engine, r, json!([{"op":"expand","entry_id":"obs"}])),
        Err(FoundryError::Conflict(_))
    ));
    assert!(call(&engine, json!({"op":"render","max_bytes":256})).is_err());
    assert!(
        call(
            &engine,
            json!({"op":"peek","entry_id":"obs","offset":11,"bytes":4})
        )
        .is_err()
    );
    let peek = call(
        &engine,
        json!({"op":"peek","entry_id":"obs","offset":10,"bytes":4}),
    )
    .unwrap();
    assert_eq!(peek["text"], "αβ");
    assert_eq!(peek["next_offset"], 14);
}
#[test]
fn superseding_constraints_survive_restoring_an_old_view() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let r = create(&engine);
    let r=apply(&engine,r,json!([add("old","constraint","Timeout is 10 seconds."),{"op":"checkpoint","name":"before"}])).unwrap()["revision"].as_u64().unwrap();
    let mut change = add("new", "constraint", "Timeout is now 20 seconds.");
    change["entry"]["supersedes"] = json!("old");
    apply(&engine, r, json!([change,{"op":"restore","name":"before"}])).unwrap();
    let rendered = call(&engine, json!({"op":"render","max_bytes":4096})).unwrap();
    assert_eq!(rendered["entries"].as_array().unwrap().len(), 1);
    assert_eq!(rendered["entries"][0]["id"], "new");
    let old = call(
        &engine,
        json!({"op":"peek","entry_id":"old","offset":0,"bytes":100}),
    )
    .unwrap();
    assert_eq!(old["historical"], true);
    assert_eq!(old["text"], "Timeout is 10 seconds.");
}
#[test]
fn linked_evidence_becomes_stale_without_rewriting_original_bytes() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let source = "pub fn retry() {}\n";
    engine.replace_source("retry.rs", source).unwrap();
    let handle = format!(
        "retry.rs#0-{}@{}.{}",
        source.len(),
        &context_foundry::digest(source.as_bytes())[..32],
        &e.wid[..16]
    );
    let mut entry = add("source", "observation", "retry was empty");
    entry["entry"]["source_links"] = json!([handle]);
    let r = create(&engine);
    let r = apply(&engine, r, json!([entry.clone()])).unwrap()["revision"]
        .as_u64()
        .unwrap();
    let view = call(&engine, json!({"op":"render","max_bytes":4096})).unwrap();
    assert_eq!(view["entries"][0]["source_links"][0]["status"], "fresh");
    engine
        .replace_source("retry.rs", "pub fn retry() { panic!() }\n")
        .unwrap();
    let view = call(&engine, json!({"op":"render","max_bytes":4096})).unwrap();
    assert_eq!(view["entries"][0]["source_links"][0]["status"], "stale");
    assert_eq!(view["entries"][0]["text"], "retry was empty");
    entry["entry"]["id"] = json!("stale-copy");
    assert!(matches!(
        apply(&engine, r, json!([entry])),
        Err(FoundryError::StaleHandle)
    ));
    assert_eq!(
        call(&engine, json!({"op":"inspect"})).unwrap()["revision"],
        r
    );
}
#[test]
fn forget_recreate_never_reuses_revision_and_repair_preserves_task_bytes() {
    let e = env();
    let mut engine = Engine::open_existing(&e.store).unwrap();
    let r = create(&engine);
    let r = apply(
        &engine,
        r,
        json!([add("obs", "observation", "private-task-canary")]),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    let before = call(&engine, json!({"op":"export"})).unwrap();
    engine
        .replace_source("src.rs", "pub fn sample() {}\n")
        .unwrap();
    engine.refresh(&Control::unbounded()).unwrap();
    drop(engine);
    Engine::repair_index(&e.store, &Control::unbounded()).unwrap();
    let engine = Engine::open_existing(&e.store).unwrap();
    assert_eq!(call(&engine, json!({"op":"export"})).unwrap(), before);
    let result = call(&engine, json!({"op":"forget","expected_revision":r})).unwrap();
    assert!(!result.to_string().contains("private-task-canary"));
    drop(engine);
    let engine = Engine::open_existing(&e.store).unwrap();
    let next = create(&engine);
    assert!(next > r);
    assert!(matches!(
        apply(&engine, r, json!([add("old", "work", "stale task writer")])),
        Err(FoundryError::Conflict(_))
    ));
    let exported = call(&engine, json!({"op":"export"})).unwrap();
    assert!(!exported.to_string().contains("private-task-canary"));
}
#[test]
fn engine_enforces_bounds_and_strict_input_without_consuming_revision() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let r = create(&engine);
    for edits in [
        json!([]),
        json!([add("too-big", "observation", &"x".repeat(256 * 1024 + 1))]),
        json!([add("guard", "constraint", &"x".repeat(16385))]),
        json!([add("bad/id", "work", "todo")]),
    ] {
        assert!(apply(&engine, r, edits).is_err());
    }
    assert!(
        task_context::parse(
            json!({"workspace_id":e.wid,"task_id":"x","command":{"op":"inspect","extra":1}})
        )
        .is_err()
    );
    let cancelled = Control::unbounded();
    cancelled.cancel();
    assert!(
        engine
            .task_context(
                &req(&e.wid, json!({"op":"forget","expected_revision":r})),
                &cancelled
            )
            .is_err()
    );
    assert_eq!(
        call(&engine, json!({"op":"inspect"})).unwrap()["revision"],
        r
    );
    assert_eq!(
        apply(&engine, r, json!([add("okay", "work", "still here")])).unwrap()["revision"],
        r + 1
    );
}
#[test]
fn schema_six_upgrade_preserves_existing_source_and_memory_and_refuses_implicit_upgrade() {
    let e = env();
    {
        let engine = Engine::open_existing(&e.store).unwrap();
        engine
            .replace_source("keep.rs", "pub fn keep() {}\n")
            .unwrap();
        engine
            .memory_put(&context_foundry::memory::PutInput {
                workspace_id: e.wid.clone(),
                fields: context_foundry::memory::RecordFields {
                    id: "existing".into(),
                    text: "Preserve this project decision.".into(),
                    author: "operator".into(),
                    provenance: "before schema upgrade".into(),
                    source_links: vec![],
                },
            })
            .unwrap();
    }
    let db = redb::Database::open(e.store.join("knowledge.redb")).unwrap();
    let tx = db.begin_write().unwrap();
    tx.delete_table(TableDefinition::<&str, &str>::new("task_context"))
        .unwrap();
    {
        let mut meta = tx
            .open_table(TableDefinition::<&str, &str>::new("meta"))
            .unwrap();
        meta.insert("schema", "6").unwrap();
        meta.remove("task_revision").unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    assert!(
        matches!(Engine::open_existing(&e.store),Err(FoundryError::UpgradeRequired{found}) if found=="6")
    );
    Engine::upgrade_store(
        &e.store,
        context_foundry::SCHEMA_VERSION,
        &Control::unbounded(),
    )
    .unwrap();
    let engine = Engine::open_existing(&e.store).unwrap();
    assert_eq!(engine.status().unwrap().source_revision, 1);
    assert_eq!(
        engine.memory_get("existing", &e.wid).unwrap().text,
        "Preserve this project decision."
    );
    create(&engine);
    drop(engine);
    // A current-version open must never recreate an accidentally deleted table.
    let db = redb::Database::open(e.store.join("knowledge.redb")).unwrap();
    let tx = db.begin_write().unwrap();
    tx.delete_table(TableDefinition::<&str, &str>::new("task_context"))
        .unwrap();
    tx.commit().unwrap();
    drop(db);
    assert!(matches!(
        Engine::open_existing(&e.store),
        Err(FoundryError::CorruptStore(_))
    ));
}
#[test]
fn task_fault_child() {
    let Ok(path) = std::env::var("CF_TASK_FAULT_DIR") else {
        return;
    };
    let mode = std::env::var("CF_TASK_FAULT_MODE").unwrap();
    let engine = Engine::open_existing(Path::new(&path)).unwrap();
    let r = call(&engine, json!({"op":"inspect"})).unwrap()["revision"]
        .as_u64()
        .unwrap();
    fault::arm(
        if mode.ends_with("before") {
            names::TASK_BEFORE_COMMIT
        } else {
            names::TASK_AFTER_COMMIT
        },
        0,
        Action::Abort,
    );
    if mode.starts_with("promote") {
        let _ = call(&engine, promotion(r));
    } else if mode.starts_with("forget") {
        let _ = call(&engine, json!({"op":"forget","expected_revision":r}));
    } else {
        let _ = apply(
            &engine,
            r,
            json!([{"op":"fold","entry_id":"obs","summary":"folded"}, asserted("atomic-fact","observation","scope:atomic","equals","committed")]),
        );
    }
    panic!("fault was not reached");
}
#[test]
fn abrupt_exit_is_atomic_for_payload_view_and_forget() {
    for mode in [
        "apply-before",
        "apply-after",
        "forget-before",
        "forget-after",
    ] {
        let e = env();
        let engine = Engine::open_existing(&e.store).unwrap();
        let r = create(&engine);
        let r = apply(
            &engine,
            r,
            json!([add("obs", "observation", "original recoverable bytes")]),
        )
        .unwrap()["revision"]
            .as_u64()
            .unwrap();
        drop(engine);
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "task_fault_child"])
            .env("CF_TASK_FAULT_DIR", &e.store)
            .env("CF_TASK_FAULT_MODE", mode)
            .output()
            .unwrap();
        assert!(!out.status.success());
        let engine = Engine::open_existing(&e.store).unwrap();
        let result = call(&engine, json!({"op":"export"}));
        if mode == "forget-after" {
            assert!(matches!(result, Err(FoundryError::NotFound)));
            continue;
        }
        let record = result.unwrap()["record"].clone();
        assert_eq!(
            record["entries"][0]["entry"]["text"],
            "original recoverable bytes"
        );
        assert_eq!(
            record["revision"],
            if mode == "apply-after" { r + 1 } else { r }
        );
        assert_eq!(
            record["entries"].as_array().unwrap().len(),
            if mode == "apply-after" { 2 } else { 1 }
        );
        if mode == "apply-after" {
            assert_eq!(
                record["entries"][1]["entry"]["assertions"][0]["value"],
                "committed"
            );
            assert_eq!(
                record["entries"][1]["assertions_sha256"]
                    .as_str()
                    .unwrap()
                    .len(),
                64
            );
        }
        assert_eq!(
            record["entries"][0]["view"]["summary"],
            if mode == "apply-after" {
                json!("folded")
            } else {
                Value::Null
            }
        );
    }
}
#[test]
fn corrupted_payload_and_exhausted_counter_fail_visibly() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let r = create(&engine);
    apply(&engine, r, json!([add("obs", "observation", "original")])).unwrap();
    drop(engine);
    let db = redb::Database::open(e.store.join("knowledge.redb")).unwrap();
    let tx = db.begin_write().unwrap();
    {
        let mut table = tx
            .open_table(TableDefinition::<&str, &str>::new("task_context"))
            .unwrap();
        let raw = table
            .get("investigation")
            .unwrap()
            .unwrap()
            .value()
            .to_owned();
        let mut record: Value = serde_json::from_str(&raw).unwrap();
        record["entries"][0]["entry"]["text"] = json!("tampered");
        table
            .insert("investigation", record.to_string().as_str())
            .unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    let engine = Engine::open_existing(&e.store).unwrap();
    assert!(matches!(
        call(&engine, json!({"op":"export"})),
        Err(FoundryError::CorruptStore(_))
    ));
    drop(engine);
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let r = create(&engine);
    drop(engine);
    let db = redb::Database::open(e.store.join("knowledge.redb")).unwrap();
    let tx = db.begin_write().unwrap();
    tx.open_table(TableDefinition::<&str, &str>::new("meta"))
        .unwrap()
        .insert("task_revision", u64::MAX.to_string().as_str())
        .unwrap();
    tx.commit().unwrap();
    drop(db);
    let engine = Engine::open_existing(&e.store).unwrap();
    assert!(matches!(
        apply(
            &engine,
            r,
            json!([add("obs", "observation", "never committed")])
        ),
        Err(FoundryError::RevisionExhausted)
    ));
    assert!(
        call(&engine, json!({"op":"inspect"})).unwrap()["entries"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn experimental_mcp_tool_is_opt_in_and_matches_cli_state() {
    use rmcp::{ServiceExt, model::CallToolRequestParams, transport::TokioChildProcess};
    let e = env();
    let mut final_structured_review = None;
    for enabled in [false, true] {
        let mut command = tokio::process::Command::new(BIN);
        command.args([
            "--store",
            e.store.to_str().unwrap(),
            "mcp",
            "--root",
            e.root.to_str().unwrap(),
        ]);
        if enabled {
            command.arg("--experimental-task-context");
        }
        let client = ().serve(TokioChildProcess::new(command).unwrap()).await.unwrap();
        let catalog = client.list_tools(None).await.unwrap();
        assert_eq!(
            catalog.tools.iter().any(|t| t.name == "task_context"),
            enabled
        );
        if enabled {
            let schema = &catalog
                .tools
                .iter()
                .find(|t| t.name == "task_context")
                .unwrap()
                .input_schema;
            assert_eq!(
                schema["properties"]["command"]["properties"]["assertions"]["maxItems"],
                16
            );
            assert_eq!(
                schema["properties"]["command"]["properties"]["edits"]["items"]["properties"]["entry"]
                    ["properties"]["assertions"]["items"]["properties"]["relation"]["enum"],
                json!(["equals", "not_equals"])
            );
            for command in [
                json!({"op":"create","title":"MCP task"}),
                json!({"op":"apply","expected_revision":1,"edits":[{"op":"add","entry":{"id":"obs","kind":"observation","text":"MCP recovery bytes","author":"sandbox operator","provenance":"synthetic task interaction","source_links":[],"assertions":[{"key":"scope:mcp","relation":"equals","value":"3"}]}}]}),
                json!({"op":"peek","entry_id":"obs","offset":0,"bytes":100}),
                json!({"op":"review","expected_revision":2,"paths":[],"finishing":true,"max_bytes":4096}),
                assertion_review(
                    2,
                    json!([{"key":"scope:mcp","relation":"equals","value":"4"}]),
                    4096,
                ),
            ] {
                let result=client.call_tool(CallToolRequestParams::new("task_context").with_arguments(json!({"workspace_id":e.wid,"task_id":"investigation","command":command}).as_object().unwrap().clone())).await.unwrap();
                assert_ne!(result.is_error, Some(true), "{result:?}");
                if command["op"] == "peek" {
                    let rmcp::model::ContentBlock::Text(text) = &result.content[0] else {
                        panic!()
                    };
                    assert_eq!(
                        serde_json::from_str::<Value>(&text.text).unwrap()["text"],
                        "MCP recovery bytes"
                    );
                }
                if command["op"] == "review" {
                    let rmcp::model::ContentBlock::Text(text) = &result.content[0] else {
                        panic!()
                    };
                    let review: Value = serde_json::from_str(&text.text).unwrap();
                    assert_eq!(review["silence"], command.get("assertions").is_none());
                    assert_eq!(review["verified"], false);
                    if command.get("assertions").is_some() {
                        assert_eq!(review["advice"][0]["trigger"], "assertion_conflict");
                        assert_eq!(review["advice"][0]["assertions"][0]["value"], "3");
                    }
                }
            }
        }
        if enabled {
            let arguments = |id| {
                json!({"workspace_id":e.wid,"task_id":"investigation","command":{"op":"apply","expected_revision":2,"edits":[add(id,"work","race candidate")]}}).as_object().unwrap().clone()
            };
            let (a, b) = tokio::join!(
                client.call_tool(
                    CallToolRequestParams::new("task_context").with_arguments(arguments("a"))
                ),
                client.call_tool(
                    CallToolRequestParams::new("task_context").with_arguments(arguments("b"))
                )
            );
            let results = [a.unwrap(), b.unwrap()];
            assert_eq!(
                results.iter().filter(|r| r.is_error == Some(true)).count(),
                1
            );
            let command = assertion_review(
                3,
                json!([{"key":"scope:mcp","relation":"equals","value":"4"}]),
                4096,
            );
            let result = client
                .call_tool(
                    CallToolRequestParams::new("task_context").with_arguments(
                        json!({"workspace_id":e.wid,"task_id":"investigation","command":command})
                            .as_object()
                            .unwrap()
                            .clone(),
                    ),
                )
                .await
                .unwrap();
            assert_ne!(result.is_error, Some(true));
            let rmcp::model::ContentBlock::Text(text) = &result.content[0] else {
                panic!()
            };
            final_structured_review = Some(serde_json::from_str::<Value>(&text.text).unwrap());
        }
        client.cancel().await.unwrap();
    }
    // Opening in a separate process proves persistence beyond the MCP session.
    let peek = cli(
        &e,
        json!({"op":"peek","entry_id":"obs","offset":0,"bytes":100}),
    );
    assert_eq!(peek["text"], "MCP recovery bytes");
    assert_eq!(
        final_structured_review.unwrap(),
        cli(
            &e,
            assertion_review(
                3,
                json!([{"key":"scope:mcp","relation":"equals","value":"4"}]),
                4096
            )
        )
    );
}

#[test]
fn advice_is_grounded_bounded_silent_and_dismissal_survives_restart() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let r = create(&engine);
    let failure_text = format!(
        "The unbounded retry loop exhausted the test service. {}",
        "Synthetic diagnostic. ".repeat(40)
    );
    let mut failure = add("failure", "rejected_attempt", &failure_text);
    failure["entry"]["action_key"] = json!("retry-loop-args-v1-environment-73");
    let r = apply(
        &engine,
        r,
        json!([failure,{"op":"checkpoint","name":"before-advice"}]),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    let advice = call(
        &engine,
        json!({"op":"advise","action_key":"retry-loop-args-v1-environment-73","max_bytes":4096}),
    )
    .unwrap();
    assert_eq!(advice["silence"], false);
    assert_eq!(advice["advice"]["entry_id"], "failure");
    assert_eq!(
        advice["advice"]["sha256"],
        context_foundry::digest(failure_text.as_bytes())
    );
    for (key, budget) in [
        ("different-environment", 4096),
        ("retry-loop-args-v1-environment-73", 512),
    ] {
        let silent = call(
            &engine,
            json!({"op":"advise","action_key":key,"max_bytes":budget}),
        )
        .unwrap();
        assert_eq!(silent["silence"], true);
        assert!(serde_json::to_vec(&silent).unwrap().len() < budget);
    }
    apply(
        &engine,
        r,
        json!([{"op":"dismiss","entry_id":"failure"},{"op":"restore","name":"before-advice"}]),
    )
    .unwrap();
    drop(engine);
    let engine = Engine::open_existing(&e.store).unwrap();
    assert_eq!(
        call(
            &engine,
            json!({"op":"advise","action_key":"retry-loop-args-v1-environment-73","max_bytes":4096})
        )
        .unwrap()["silence"],
        true
    );
}
#[test]
fn review_checks_explicit_scope_and_open_obligations_without_claiming_conflicts() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let source = "def retry(): return 3\n";
    engine.replace_source("retry.py", source).unwrap();
    let handle = context_foundry::SourceHandle {
        workspace_id: e.wid.clone(),
        path: "retry.py".into(),
        sha256: context_foundry::digest(source.as_bytes()),
        start: 0,
        end: source.len() as u64,
    }
    .to_v2();
    let mut constraint = add("limit", "constraint", "Keep three retries.");
    constraint["entry"]["source_links"] = json!([handle]);
    let mut failed = add(
        "failed",
        "rejected_attempt",
        "Unlimited retries exhausted the server.",
    );
    failed["entry"]["source_links"] = constraint["entry"]["source_links"].clone();
    let revision = apply(&engine, create(&engine), json!([constraint, failed, add("test", "work", "Run the retry regression."), add("open", "question", "Does the server send Retry-After?"), {"op":"checkpoint","name":"before"}])).unwrap()["revision"].as_u64().unwrap();
    let review = |paths: Value, finishing: bool, revision, max_bytes| json!({"op":"review","expected_revision":revision,"paths":paths,"finishing":finishing,"max_bytes":max_bytes});
    let scoped = call(&engine, review(json!(["retry.py"]), false, revision, 4096)).unwrap();
    assert_eq!(scoped["advice"].as_array().unwrap().len(), 2);
    assert_eq!(scoped["advice"][0]["trigger"], "constraint_check");
    assert_eq!(scoped["advice"][1]["trigger"], "related_failed_attempt");
    assert_eq!(scoped["verified"], false);
    assert_eq!(
        call(
            &engine,
            review(json!(["unrelated.py"]), false, revision, 4096)
        )
        .unwrap()["silence"],
        true
    );
    let finishing = call(&engine, review(json!([]), true, revision, 4096)).unwrap();
    assert_eq!(finishing["advice"].as_array().unwrap().len(), 3);
    assert!(call(&engine, review(json!([]), true, revision - 1, 4096)).is_err());
    assert!(call(&engine, review(json!(["../escape"]), false, revision, 4096)).is_err());
    let bounded = call(&engine, review(json!(["retry.py"]), true, revision, 512)).unwrap();
    assert!(serde_json::to_vec(&bounded).unwrap().len() < 512);
    assert!(bounded["omitted"].as_u64().unwrap() > 0);
    engine
        .replace_source("retry.py", "def retry(): return 4\n")
        .unwrap();
    let stale = call(&engine, review(json!(["retry.py"]), false, revision, 4096)).unwrap();
    assert_eq!(stale["silence"], true);
    assert_eq!(stale["stale"], 2);
    let revision = apply(&engine, revision, json!([{"op":"dismiss","entry_id":"failed"},{"op":"restore","name":"before"},{"op":"select","entry_id":"test","selected":false},{"op":"select","entry_id":"open","selected":false}])).unwrap()["revision"].as_u64().unwrap();
    drop(engine);
    let reopened = cli(&e, review(json!([]), true, revision, 4096));
    assert_eq!(reopened["silence"], true);
    assert_eq!(reopened["stale"], 1);
}
fn promotion(revision: u64) -> Value {
    json!({"op":"promote","expected_revision":revision,"entry_id":"decision","memory_id":"backoff-policy","author":"operator","provenance":"explicit sandbox promotion"})
}

fn asserted(id: &str, kind: &str, key: &str, relation: &str, value: &str) -> Value {
    let mut edit = add(id, kind, "Caller-supplied policy or observation.");
    edit["entry"]["assertions"] = json!([{ "key":key, "relation":relation, "value":value }]);
    edit
}
fn assertion_review(revision: u64, assertions: Value, max_bytes: usize) -> Value {
    json!({"op":"review","expected_revision":revision,"paths":[],"finishing":false,"assertions":assertions,"max_bytes":max_bytes})
}
#[test]
fn structured_assertions_detect_conflicts_and_constraint_violations_without_prose_inference() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let revision = apply(
        &engine,
        create(&engine),
        json!([
            asserted(
                "observed",
                "observation",
                "server-v1:retry.limit",
                "equals",
                "3"
            ),
            asserted(
                "required",
                "requirement",
                "server-v1:retry.limit",
                "equals",
                "3"
            ),
            asserted(
                "forbidden",
                "constraint",
                "server-v1:retry.limit",
                "not_equals",
                "unlimited"
            ),
            asserted(
                "hypothesis",
                "hypothesis",
                "server-v1:retry.limit",
                "equals",
                "3"
            ),
            asserted(
                "decision",
                "decision",
                "server-v1:retry.limit",
                "equals",
                "3"
            ),
            add("prose", "observation", "The retry limit is unlimited.")
        ]),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    let result = call(
        &engine,
        assertion_review(
            revision,
            json!([
                {"key":"server-v1:retry.limit","relation":"equals","value":"unlimited"}
            ]),
            8192,
        ),
    )
    .unwrap();
    assert_eq!(result["silence"], false);
    assert_eq!(result["verified"], false);
    assert_eq!(result["host_application"], "unknown");
    let advice = result["advice"].as_array().unwrap();
    assert_eq!(advice.len(), 5);
    assert_eq!(advice[0]["trigger"], "assertion_conflict");
    assert_eq!(advice[1]["trigger"], "constraint_violation");
    assert_eq!(advice[2]["trigger"], "constraint_violation");
    assert_eq!(advice[3]["trigger"], "assertion_conflict");
    assert_eq!(advice[4]["trigger"], "assertion_conflict");
    for row in advice {
        assert_eq!(row["conflicts"][0]["proposed"]["value"], "unlimited");
        assert_eq!(row["assertions_sha256"].as_str().unwrap().len(), 64);
        assert_eq!(row["author"], "sandbox operator");
    }
    for assertions in [
        json!([]),
        json!([{"key":"server-v1:retry.limit","relation":"equals","value":"3"}]),
        json!([{"key":"server-v2:retry.limit","relation":"equals","value":"unlimited"}]),
        json!([{"key":"Server-v1:retry.limit","relation":"equals","value":"unlimited"}]),
    ] {
        assert_eq!(
            call(&engine, assertion_review(revision, assertions, 8192)).unwrap()["silence"],
            true
        );
    }
    assert_eq!(
        call(&engine, json!({"op":"inspect"})).unwrap()["revision"],
        revision
    );
}

#[test]
fn malformed_structured_assertions_roll_back_the_entire_batch_without_consuming_revision() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let revision = create(&engine);
    let valid = json!({"key":"scope:key","relation":"equals","value":"3"});
    let mut bad = vec![
        Value::Null,
        json!([{}]),
        json!([{"key":"x","value":"3"}]),
        json!([{"key":"x","relation":"less_than","value":"3"}]),
        json!([{"key":"x","relation":"equals","value":"3","extra":true}]),
        json!([valid.clone(), valid.clone()]),
        json!(vec![valid.clone(); 17]),
    ];
    for (field, value) in [
        ("key", json!("")),
        ("key", json!(" scope")),
        ("key", json!("key key")),
        ("key", json!("é")),
        ("key", json!("k".repeat(129))),
        ("value", json!(" \n")),
        ("value", json!("α".repeat(257))),
        ("value", json!(3)),
    ] {
        let mut row = valid.clone();
        row[field] = value;
        bad.push(json!([row]));
    }
    for assertions in bad {
        let mut edit = add(
            "invalid",
            "observation",
            "Malformed assertion must not persist.",
        );
        edit["entry"]["assertions"] = assertions.clone();
        let request = json!({"workspace_id":e.wid,"task_id":"investigation","command":{"op":"apply","expected_revision":revision,"edits":[add("first","work","Roll back this earlier edit too."),edit]}});
        let result = task_context::parse(request)
            .and_then(|request| engine.task_context(&request, &Control::unbounded()));
        assert!(
            matches!(result, Err(FoundryError::InvalidArgument(_))),
            "{assertions}: {result:?}"
        );
        let inspect = call(&engine, json!({"op":"inspect"})).unwrap();
        assert_eq!(inspect["revision"], revision);
        assert_eq!(inspect["entries"], json!([]));
    }
    for (kind, relation) in [
        ("observation", "not_equals"),
        ("decision", "not_equals"),
        ("work", "equals"),
        ("artifact", "equals"),
        ("rejected_attempt", "equals"),
    ] {
        assert!(
            apply(
                &engine,
                revision,
                json!([asserted("invalid", kind, "scope:key", relation, "3")])
            )
            .is_err()
        );
    }
    for assertions in [
        json!([{"key":"x","relation":"not_equals","value":"3"}]),
        json!([valid.clone(), valid.clone()]),
        json!(vec![valid; 17]),
    ] {
        assert!(call(&engine, assertion_review(revision, assertions, 4096)).is_err());
    }
    assert_eq!(
        call(&engine, json!({"op":"inspect"})).unwrap()["revision"],
        revision
    );
}

#[test]
fn structured_review_retains_exact_values_through_fold_selection_supersession_and_restart() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let revision=apply(&engine,create(&engine),json!([
        asserted("old","observation","scope:key","equals","3"),
        {"op":"checkpoint","name":"original"},
        {"op":"fold","entry_id":"old","summary":"Caller summary says four, without changing the assertion."}
    ])).unwrap()["revision"].as_u64().unwrap();
    let rendered = call(&engine, json!({"op":"render","max_bytes":8192})).unwrap();
    assert_eq!(rendered["entries"][0]["assertions"][0]["value"], "3");
    assert_eq!(rendered["entries"][0]["representation"], "caller_summary");
    let mut replacement = asserted("new", "observation", "scope:key", "equals", "4");
    replacement["entry"]["supersedes"] = json!("old");
    let revision = apply(
        &engine,
        revision,
        json!([replacement,{"op":"restore","name":"original"}]),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    let proposed = json!([{"key":"scope:key","relation":"equals","value":"3"}]);
    let review = call(&engine, assertion_review(revision, proposed.clone(), 8192)).unwrap();
    assert_eq!(review["advice"].as_array().unwrap().len(), 1);
    assert_eq!(review["advice"][0]["entry_id"], "new");
    let revision = apply(
        &engine,
        revision,
        json!([{"op":"select","entry_id":"new","selected":false}]),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    assert_eq!(
        call(&engine, assertion_review(revision, proposed.clone(), 8192)).unwrap()["silence"],
        true
    );
    let revision = apply(
        &engine,
        revision,
        json!([{"op":"select","entry_id":"new","selected":true}]),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    drop(engine);
    let reopened = cli(&e, assertion_review(revision, proposed, 8192));
    assert_eq!(reopened["advice"][0]["assertions"][0]["value"], "4");
    let expected =
        context_foundry::digest(&serde_json::to_vec(&reopened["advice"][0]["assertions"]).unwrap());
    assert_eq!(reopened["advice"][0]["assertions_sha256"], expected);
    let peek = cli(
        &e,
        json!({"op":"peek","entry_id":"new","offset":0,"bytes":4}),
    );
    assert_eq!(peek["assertions_sha256"], expected);
    let engine = Engine::open_existing(&e.store).unwrap();
    for value in ["4", "04", "4 "] {
        let response = call(
            &engine,
            assertion_review(
                revision,
                json!([{"key":"scope:key","relation":"equals","value":value}]),
                8192,
            ),
        )
        .unwrap();
        assert_eq!(response["silence"], value == "4");
    }
    assert!(matches!(
        call(&engine, assertion_review(revision - 1, json!([]), 8192)),
        Err(FoundryError::Conflict(_))
    ));
    drop(engine);
    cli(&e, json!({"op":"forget","expected_revision":revision}));
    let engine = Engine::open_existing(&e.store).unwrap();
    assert!(matches!(
        call(&engine, assertion_review(revision, json!([]), 8192)),
        Err(FoundryError::NotFound)
    ));
}

#[test]
fn structured_review_checks_freshness_and_returns_one_complete_bounded_original_per_entry() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let source = "pub fn retry() -> u8 { 3 }\n";
    engine.replace_source("retry.rs", source).unwrap();
    let handle = context_foundry::SourceHandle {
        workspace_id: e.wid.clone(),
        path: "retry.rs".into(),
        sha256: context_foundry::digest(source.as_bytes()),
        start: 0,
        end: source.len() as u64,
    }
    .to_v2();
    let mut edits = Vec::new();
    for i in 0..9 {
        let mut edit = asserted(
            &format!("rule{i}"),
            "constraint",
            "scope:key",
            "equals",
            "3",
        );
        edit["entry"]["source_links"] = json!([handle]);
        edit["entry"]["assertions"]
            .as_array_mut()
            .unwrap()
            .push(json!({"key":"scope:second","relation":"not_equals","value":"forbidden"}));
        edits.push(edit);
    }
    let revision = apply(&engine, create(&engine), json!(edits)).unwrap()["revision"]
        .as_u64()
        .unwrap();
    let assertions = json!([{"key":"scope:key","relation":"equals","value":"4"},{"key":"scope:second","relation":"equals","value":"forbidden"}]);
    let mut request = assertion_review(revision, assertions.clone(), 65536);
    request["paths"] = json!(["retry.rs"]);
    request["finishing"] = json!(true);
    let full = call(&engine, request).unwrap();
    assert_eq!(full["advice"].as_array().unwrap().len(), 8);
    assert_eq!(full["omitted"], 1);
    for row in full["advice"].as_array().unwrap() {
        assert_eq!(row["trigger"], "constraint_violation");
        assert_eq!(row["conflicts"].as_array().unwrap().len(), 2);
        assert_eq!(row["source_links"][0]["status"], "fresh");
        assert_eq!(row["text"], "Caller-supplied policy or observation.");
    }
    let cut = call(&engine, assertion_review(revision, assertions.clone(), 512)).unwrap();
    assert_eq!(cut["silence"], true);
    assert_eq!(cut["omitted"], 9);
    assert!(serde_json::to_vec(&cut).unwrap().len() < 512);
    engine
        .replace_source("retry.rs", "pub fn retry() -> u8 { 4 }\n")
        .unwrap();
    let stale = call(&engine, assertion_review(revision, assertions, 65536)).unwrap();
    assert_eq!(stale["silence"], true);
    assert_eq!(stale["stale"], 9);
}

#[test]
fn altered_structured_assertions_or_their_digest_are_corrupt_instead_of_new_facts() {
    for damage in ["value", "digest", "missing_digest"] {
        let e = env();
        let engine = Engine::open_existing(&e.store).unwrap();
        apply(
            &engine,
            create(&engine),
            json!([asserted(
                "fact",
                "decision",
                "scope:key",
                "equals",
                "original"
            )]),
        )
        .unwrap();
        drop(engine);
        let db = redb::Database::open(e.store.join("knowledge.redb")).unwrap();
        let tx = db.begin_write().unwrap();
        {
            let mut table = tx
                .open_table(TableDefinition::<&str, &str>::new("task_context"))
                .unwrap();
            let raw = table
                .get("investigation")
                .unwrap()
                .unwrap()
                .value()
                .to_owned();
            let mut row: Value = serde_json::from_str(&raw).unwrap();
            match damage {
                "value" => row["entries"][0]["entry"]["assertions"][0]["value"] = json!("tampered"),
                "digest" => row["entries"][0]["assertions_sha256"] = json!("0".repeat(64)),
                _ => {
                    row["entries"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove("assertions_sha256");
                }
            }
            table
                .insert("investigation", row.to_string().as_str())
                .unwrap();
        }
        tx.commit().unwrap();
        drop(db);
        let engine = Engine::open_existing(&e.store).unwrap();
        assert!(
            matches!(
                call(&engine, json!({"op":"export"})),
                Err(FoundryError::CorruptStore(_))
            ),
            "{damage}"
        );
    }
}

#[test]
fn structured_decision_promotion_binds_assertions_without_copying_rules_into_memory() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let revision = apply(
        &engine,
        create(&engine),
        json!([asserted("decision", "decision", "scope:key", "equals", "3")]),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    let digest = call(&engine, json!({"op":"inspect"})).unwrap()["entries"][0]["assertions_sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    let revision = call(&engine, promotion(revision)).unwrap()["revision"]
        .as_u64()
        .unwrap();
    let memory = engine.memory_get("backoff-policy", &e.wid).unwrap();
    assert_eq!(memory.text, "Caller-supplied policy or observation.");
    assert!(memory.provenance.contains(&format!("assertions:{digest}")));
    call(&engine, json!({"op":"forget","expected_revision":revision})).unwrap();
    assert!(matches!(
        engine.memory_get("backoff-policy", &e.wid),
        Err(FoundryError::NotFound)
    ));
}
#[test]
fn promotion_is_explicit_indexed_and_forget_removes_derived_retrieval() {
    let e = env();
    let r = cli(&e, json!({"op":"create","title":"Policy decision"}))["revision"]
        .as_u64()
        .unwrap();
    let r=cli(&e,json!({"op":"apply","expected_revision":r,"edits":[add("decision","decision","quartzbackoff uses Retry-After with the original retry limit.")]}))["revision"].as_u64().unwrap();
    let promoted = cli(&e, promotion(r));
    let r = promoted["revision"].as_u64().unwrap();
    let mut engine = Engine::open_existing(&e.store).unwrap();
    let memory = engine.memory_get("backoff-policy", &e.wid).unwrap();
    assert!(
        memory
            .provenance
            .contains("task:investigation/entry:decision@")
    );
    let hits = engine
        .memory_search(&context_foundry::memory::SearchInput {
            workspace_id: e.wid.clone(),
            query: "quartzbackoff".into(),
            limit: 10,
            tokens: 1024,
        })
        .unwrap();
    assert_eq!(hits.hits.len(), 1);
    let result = call(&engine, json!({"op":"forget","expected_revision":r})).unwrap();
    assert_eq!(result["forgotten_memory_ids"], json!(["backoff-policy"]));
    // Intentionally leave the old Tantivy document behind: authoritative
    // validation must exclude it before the derived deletion is drained.
    assert!(matches!(
        engine.memory_get("backoff-policy", &e.wid),
        Err(FoundryError::NotFound)
    ));
    let hits = engine
        .memory_search(&context_foundry::memory::SearchInput {
            workspace_id: e.wid.clone(),
            query: "quartzbackoff".into(),
            limit: 10,
            tokens: 1024,
        })
        .unwrap();
    assert!(hits.hits.is_empty());
    assert_eq!(hits.stale_candidates, 1);
    engine.refresh(&Control::unbounded()).unwrap();
}
#[test]
fn promoted_copy_correction_blocks_destructive_forget_and_supersession() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let r = create(&engine);
    let r = apply(
        &engine,
        r,
        json!([add("decision", "decision", "Use bounded backoff.")]),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    let promoted = call(&engine, promotion(r)).unwrap();
    let r = promoted["revision"].as_u64().unwrap();
    let mut replacement = add("corrected", "decision", "Use fixed backoff.");
    replacement["entry"]["supersedes"] = json!("decision");
    assert!(matches!(
        apply(&engine, r, json!([replacement.clone()])),
        Err(FoundryError::Conflict(_))
    ));
    let updated = engine
        .memory_update(&context_foundry::memory::UpdateInput {
            workspace_id: e.wid.clone(),
            expected_revision: promoted["memory_revision"].as_u64().unwrap(),
            fields: context_foundry::memory::RecordFields {
                id: "backoff-policy".into(),
                text: "An independent corrected project decision.".into(),
                author: "operator".into(),
                provenance: "explicit correction".into(),
                source_links: vec![],
            },
        })
        .unwrap();
    assert!(matches!(
        call(&engine, json!({"op":"forget","expected_revision":r})),
        Err(FoundryError::Conflict(_))
    ));
    assert_eq!(
        call(&engine, json!({"op":"inspect"})).unwrap()["revision"],
        r
    );
    engine
        .memory_forget(
            &context_foundry::memory::ForgetInput {
                id: "backoff-policy".into(),
                workspace_id: e.wid.clone(),
                expected_revision: updated.revision,
            },
            &Control::unbounded(),
        )
        .unwrap();
    let r = apply(&engine, r, json!([replacement])).unwrap()["revision"]
        .as_u64()
        .unwrap();
    call(&engine, json!({"op":"forget","expected_revision":r})).unwrap();
}
#[test]
fn promotion_rejects_non_decisions_oversize_and_stale_sources_atomically() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let r = create(&engine);
    let r = apply(
        &engine,
        r,
        json!([
            add("hypothesis", "hypothesis", "probably true"),
            add("decision", "decision", &"x".repeat(16385))
        ]),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    assert!(call(&engine, promotion(r)).is_err());
    let mut wrong = promotion(r);
    wrong["entry_id"] = json!("hypothesis");
    assert!(call(&engine, wrong).is_err());
    assert!(matches!(
        engine.memory_get("backoff-policy", &e.wid),
        Err(FoundryError::NotFound)
    ));
    assert_eq!(
        call(&engine, json!({"op":"inspect"})).unwrap()["revision"],
        r
    );
    // Staleness is checked again at promotion, after admission of the entry.
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let r = create(&engine);
    let source = "fn f() {}\n";
    engine.replace_source("f.rs", source).unwrap();
    let mut decision = add("decision", "decision", "Keep the f implementation.");
    decision["entry"]["source_links"] = json!([format!(
        "f.rs#0-{}@{}.{}",
        source.len(),
        &context_foundry::digest(source.as_bytes())[..32],
        &e.wid[..16]
    )]);
    let r = apply(&engine, r, json!([decision])).unwrap()["revision"]
        .as_u64()
        .unwrap();
    engine.replace_source("f.rs", "fn f(){panic!()}\n").unwrap();
    assert!(matches!(
        call(&engine, promotion(r)),
        Err(FoundryError::StaleHandle)
    ));
    assert!(matches!(
        engine.memory_get("backoff-policy", &e.wid),
        Err(FoundryError::NotFound)
    ));
}

#[test]
fn abrupt_promotion_exit_commits_memory_task_lineage_and_revisions_together() {
    for mode in ["promote-before", "promote-after"] {
        let e = env();
        let engine = Engine::open_existing(&e.store).unwrap();
        let r = create(&engine);
        let r = apply(
            &engine,
            r,
            json!([add("decision", "decision", "Keep bounded backoff.")]),
        )
        .unwrap()["revision"]
            .as_u64()
            .unwrap();
        drop(engine);
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "task_fault_child"])
            .env("CF_TASK_FAULT_DIR", &e.store)
            .env("CF_TASK_FAULT_MODE", mode)
            .output()
            .unwrap();
        assert!(!out.status.success());
        let engine = Engine::open_existing(&e.store).unwrap();
        let task = call(&engine, json!({"op":"inspect"})).unwrap();
        if mode.ends_with("before") {
            assert_eq!(task["revision"], r);
            assert!(task["promotions"].as_object().unwrap().is_empty());
            assert!(matches!(
                engine.memory_get("backoff-policy", &e.wid),
                Err(FoundryError::NotFound)
            ));
            assert_eq!(call(&engine, promotion(r)).unwrap()["memory_revision"], 1);
        } else {
            assert_eq!(task["revision"], r + 1);
            assert_eq!(task["promotions"]["backoff-policy"]["memory_revision"], 1);
            assert_eq!(
                engine.memory_get("backoff-policy", &e.wid).unwrap().text,
                "Keep bounded backoff."
            );
        }
    }
}

#[test]
fn cascade_forget_is_atomic_across_task_and_promoted_memory_on_abrupt_exit() {
    for mode in ["forget-before", "forget-after"] {
        let e = env();
        let engine = Engine::open_existing(&e.store).unwrap();
        let r = create(&engine);
        let r = apply(
            &engine,
            r,
            json!([add("decision", "decision", "Retained project decision.")]),
        )
        .unwrap()["revision"]
            .as_u64()
            .unwrap();
        let r = call(&engine, promotion(r)).unwrap()["revision"]
            .as_u64()
            .unwrap();
        drop(engine);
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "task_fault_child"])
            .env("CF_TASK_FAULT_DIR", &e.store)
            .env("CF_TASK_FAULT_MODE", mode)
            .output()
            .unwrap();
        assert!(!out.status.success());
        let engine = Engine::open_existing(&e.store).unwrap();
        if mode.ends_with("before") {
            assert_eq!(
                call(&engine, json!({"op":"inspect"})).unwrap()["revision"],
                r
            );
            assert_eq!(
                engine.memory_get("backoff-policy", &e.wid).unwrap().text,
                "Retained project decision."
            );
        } else {
            assert!(matches!(
                call(&engine, json!({"op":"inspect"})),
                Err(FoundryError::NotFound)
            ));
            assert!(matches!(
                engine.memory_get("backoff-policy", &e.wid),
                Err(FoundryError::NotFound)
            ));
        }
    }
}

#[test]
fn task_and_checkpoint_capacity_refusals_preserve_all_committed_state() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let mut r = create(&engine);
    for batch in 0..4 {
        let edits: Vec<_> = (0..32)
            .map(|i| add(&format!("entry{}", batch * 32 + i), "work", "retained item"))
            .collect();
        r = apply(&engine, r, json!(edits)).unwrap()["revision"]
            .as_u64()
            .unwrap();
    }
    assert!(
        apply(
            &engine,
            r,
            json!([add("overflow", "work", "must not replace an item")])
        )
        .is_err()
    );
    let checkpoints: Vec<_> = (0..16)
        .map(|i| json!({"op":"checkpoint","name":format!("c{i}")}))
        .collect();
    r = apply(&engine, r, json!(checkpoints)).unwrap()["revision"]
        .as_u64()
        .unwrap();
    assert!(apply(&engine, r, json!([{"op":"checkpoint","name":"overflow"}])).is_err());
    let state = call(&engine, json!({"op":"inspect"})).unwrap();
    assert_eq!(state["entries"].as_array().unwrap().len(), 128);
    assert_eq!(state["checkpoints"].as_array().unwrap().len(), 16);
    assert_eq!(state["revision"], r);
    // Repeating create after response loss returns the current incarnation.
    assert_eq!(create(&engine), r);
}

#[test]
fn deleted_promotions_keep_their_lineage_and_cannot_be_reassigned() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let r = create(&engine);
    let r = apply(
        &engine,
        r,
        json!([
            add("decision", "decision", "First decision."),
            add("later", "decision", "Another decision.")
        ]),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    let result = call(&engine, promotion(r)).unwrap();
    let r = result["revision"].as_u64().unwrap();
    engine
        .memory_forget(
            &context_foundry::memory::ForgetInput {
                id: "backoff-policy".into(),
                workspace_id: e.wid.clone(),
                expected_revision: result["memory_revision"].as_u64().unwrap(),
            },
            &Control::unbounded(),
        )
        .unwrap();
    let mut reuse = promotion(r);
    reuse["entry_id"] = json!("later");
    assert!(matches!(
        call(&engine, reuse),
        Err(FoundryError::Conflict(_))
    ));
    let task = call(&engine, json!({"op":"inspect"})).unwrap();
    assert_eq!(task["revision"], r);
    assert_eq!(task["promotions"]["backoff-policy"]["entry_id"], "decision");
    assert!(matches!(
        engine.memory_get("backoff-policy", &e.wid),
        Err(FoundryError::NotFound)
    ));
}

#[test]
fn exported_host_packet_binds_a_folded_observation_and_recovers_original_after_restart() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let r = create(&engine);
    let original = "long tool output\n".repeat(400);
    let r=apply(&engine,r,json!([add("keep","constraint","Keep all existing user requirements."),add("output","observation",&original),{"op":"fold","entry_id":"output","summary":"The tool observed the expected file."}])).unwrap()["revision"].as_u64().unwrap();
    let export = |revision, entry| json!({"op":"export_host","expected_revision":revision,"entry_id":entry,"tool_call_id":"call_9","max_request_bytes":1048576});
    assert!(matches!(
        call(&engine, export(r - 1, "output")),
        Err(FoundryError::Conflict(_))
    ));
    assert!(call(&engine, export(r, "keep")).is_err());
    drop(engine);
    let packet = cli(&e, export(r, "output"));
    let parsed: context_foundry::task_host::Packet =
        serde_json::from_value(packet.clone()).unwrap();
    parsed.validate().unwrap();
    assert_eq!(
        packet["original_sha256"],
        context_foundry::digest(original.as_bytes())
    );
    assert_eq!(
        packet["view"]["entries"][0]["text"],
        "Keep all existing user requirements."
    );
    assert_eq!(packet["view"]["entries"][0]["representation"], "original");
    assert_eq!(packet["view"]["host_application"], "unknown");
    let engine = Engine::open_existing(&e.store).unwrap();
    let peek = call(
        &engine,
        json!({"op":"peek","entry_id":"output","offset":0,"bytes":65536}),
    )
    .unwrap();
    assert_eq!(peek["text"], original);
    let r = apply(&engine, r, json!([{"op":"expand","entry_id":"output"}])).unwrap()["revision"]
        .as_u64()
        .unwrap();
    assert!(
        call(&engine, export(r, "output")).is_err(),
        "an unfolded entry is not an admitted replacement"
    );
}
