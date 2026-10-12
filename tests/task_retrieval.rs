//! Task-aware retrieval uses the real store, packer and SDK MCP transport.
use context_foundry::fault::{self, Action, names};
use context_foundry::store::ContextOptions;
use context_foundry::{Control, Engine, FoundryError, Strategy, response, task_context};
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, CallToolResult},
    transport::TokioChildProcess,
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};
const BIN: &str = env!("CARGO_BIN_EXE_foundry");
const SOURCE: &str = "pub fn retry_delay(retry_after: u64) -> u64 { retry_after.min(90) }\npub fn retry_attempts() -> u8 { 3 }\n";
struct Env {
    _dir: tempfile::TempDir,
    root: PathBuf,
    store: PathBuf,
    wid: String,
    revision: u64,
}
fn request(wid: &str, command: Value) -> task_context::Request {
    task_context::parse(json!({"workspace_id":wid,"task_id":"retry-task","command":command}))
        .unwrap()
}
fn call(engine: &Engine, command: Value) -> Result<Value, FoundryError> {
    engine.task_context(
        &request(&engine.workspace_id().unwrap(), command),
        &Control::unbounded(),
    )
}
fn add(id: &str, kind: &str, text: &str) -> Value {
    json!({"op":"add","entry":{"id":id,"kind":kind,"text":text,"author":"fixture user","provenance":"explicit sandbox instruction","source_links":[]}})
}
fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let store = dir.path().join("store");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("retry.rs"), SOURCE).unwrap();
    let mut engine = Engine::initialize(&store, &root).unwrap();
    engine.index(&root, &Control::unbounded()).unwrap();
    let wid = engine.workspace_id().unwrap();
    let r = call(&engine, json!({"op":"create","title":"Repair retry delay"})).unwrap()["revision"]
        .as_u64()
        .unwrap();
    let r=call(&engine,json!({"op":"apply","expected_revision":r,"edits":[add("constraint","constraint","Preserve the limit of three attempts."),add("trace","observation","Operator-only fact: retry-after is 73 seconds."),{"op":"fold","entry_id":"trace","summary":"Retained external retry-after observation; peek before changing backoff."}]})).unwrap()["revision"].as_u64().unwrap();
    Env {
        _dir: dir,
        root,
        store,
        wid,
        revision: r,
    }
}
fn context(
    engine: &Engine,
    e: &Env,
    query: &str,
    strategy: Strategy,
) -> Result<context_foundry::memory::MemoryContext, FoundryError> {
    engine.context_candidates_with(
        query,
        strategy,
        &Control::unbounded(),
        &ContextOptions {
            task: Some(task_context::Scope {
                workspace_id: &e.wid,
                task_id: "retry-task",
                expected_revision: e.revision,
            }),
            ..ContextOptions::default()
        },
    )
}
fn pack(
    out: &context_foundry::memory::MemoryContext,
    tokens: usize,
) -> Result<response::PackedText, FoundryError> {
    response::pack_task_context(
        &out.batch,
        None,
        &out.hits,
        out.task_view.as_ref(),
        response::Budget::request(tokens),
        &response::stdout_bytes,
    )
}
fn view(text: &str) -> Value {
    let line = text
        .lines()
        .find_map(|s| s.strip_prefix("task-view: "))
        .expect("one mandatory task view");
    let data: Value = serde_json::from_str(line).unwrap();
    assert_eq!(
        data["sha256"],
        context_foundry::digest(&serde_json::to_vec(&data["view"]).unwrap())
    );
    data["view"].clone()
}
#[test]
fn mandatory_task_view_and_source_share_exact_budget_under_each_context_strategy() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    for (query, strategy) in [
        ("retry_delay", Strategy::Search),
        ("delay retry backoff", Strategy::Search),
        ("callers of retry_delay", Strategy::Graph),
    ] {
        let out = context(&engine, &e, query, strategy).unwrap();
        let mut saw_success = false;
        let mut saw_refusal = false;
        for budget in [1, 64, 256, 512, 1024, 2048] {
            match pack(&out, budget) {
                Ok(packed) => {
                    saw_success = true;
                    assert_eq!(packed.tokens, response::count_tokens(&packed.text));
                    assert!(packed.tokens <= budget);
                    assert!(packed.text.len() <= response::BYTE_CAP);
                    let data = view(&packed.text);
                    assert_eq!(data["revision"], e.revision);
                    assert_eq!(
                        data["entries"][0]["text"],
                        "Preserve the limit of three attempts."
                    );
                    assert_eq!(data["entries"][1]["representation"], "caller_summary");
                    assert!(!packed.text.contains("73 seconds"));
                    assert_eq!(packed.text.matches("task-view: ").count(), 1);
                }
                Err(FoundryError::BudgetTooSmall { minimum_tokens }) => {
                    saw_refusal = true;
                    assert!(minimum_tokens > budget);
                    assert!(pack(&out, minimum_tokens).is_ok());
                }
                Err(other) => panic!("{other:?}"),
            }
        }
        assert!(saw_success && saw_refusal);
    }
    let out = context(&engine, &e, "retry_delay", Strategy::Search).unwrap();
    assert!(
        pack(&out, 2048)
            .unwrap()
            .text
            .contains("retry_after.min(90)")
    );
    let plain = engine
        .context_candidates("retry_delay", Strategy::Search, &Control::unbounded())
        .unwrap();
    assert_eq!(
        response::pack_context(
            &plain,
            response::Budget::request(1024),
            &response::stdout_bytes
        )
        .unwrap()
        .text,
        response::pack_task_context(
            &plain,
            None,
            &[],
            None,
            response::Budget::request(1024),
            &response::stdout_bytes
        )
        .unwrap()
        .text
    );
}
#[test]
fn a_task_change_or_forget_before_the_final_source_read_refuses_the_entire_response() {
    for forget in [false, true] {
        let e = env();
        let engine = Engine::open_existing(&e.store).unwrap();
        let wid = e.wid.clone();
        let r = e.revision;
        fault::arm(
            names::CONTEXT_BEFORE_FINAL_VALIDATION,
            0,
            Action::Call(Box::new(move |ctx| {
                let command = if forget {
                    json!({"op":"forget","expected_revision":r})
                } else {
                    json!({"op":"apply","expected_revision":r,"edits":[add("new-constraint","constraint","Preserve the public retry API.")]})
                };
                ctx.engine
                    .unwrap()
                    .task_context(&request(&wid, command), &Control::unbounded())
                    .unwrap();
            })),
        );
        let result = context(&engine, &e, "retry_delay", Strategy::Search);
        fault::disarm_all();
        if forget {
            assert!(matches!(result, Err(FoundryError::NotFound)));
        } else {
            assert!(matches!(result, Err(FoundryError::Conflict(_))));
        }
    }
}
#[test]
fn linked_task_freshness_and_source_evidence_use_one_final_snapshot() {
    let mut e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let mut entry = add("linked", "observation", "Observed old retry code.");
    entry["entry"]["source_links"] = json!([format!(
        "retry.rs#0-{}@{}.{}",
        SOURCE.len(),
        &context_foundry::digest(SOURCE.as_bytes())[..32],
        &e.wid[..16]
    )]);
    e.revision = call(
        &engine,
        json!({"op":"apply","expected_revision":e.revision,"edits":[entry]}),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    let before = pack(
        &context(&engine, &e, "retry_delay", Strategy::Search).unwrap(),
        4096,
    )
    .unwrap();
    assert_eq!(
        view(&before.text)["entries"][2]["source_links"][0]["status"],
        "fresh"
    );
    fault::arm(
        names::CONTEXT_BEFORE_FINAL_VALIDATION,
        0,
        Action::Call(Box::new(|ctx| {
            ctx.engine
                .unwrap()
                .replace_source("retry.rs", "pub fn retry_delay() -> u64 { 99 }\n")
                .unwrap();
        })),
    );
    let after = context(&engine, &e, "retry_delay", Strategy::Search).unwrap();
    fault::disarm_all();
    let packed = pack(&after, 4096).unwrap();
    assert_eq!(
        view(&packed.text)["entries"][2]["source_links"][0]["status"],
        "stale"
    );
    assert!(!packed.text.contains("retry_after.min(90)"));
    assert!(after.batch.counters.stale > 0);
    assert_ne!(view(&before.text), view(&packed.text));
}
#[test]
fn oversized_selected_observation_requires_an_explicit_fold_before_delivery() {
    let mut e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    e.revision=call(&engine,json!({"op":"apply","expected_revision":e.revision,"edits":[add("huge","observation",&"\"".repeat(140_000))]})).unwrap()["revision"].as_u64().unwrap();
    let out = context(&engine, &e, "retry_delay", Strategy::Search).unwrap();
    assert!(matches!(
        pack(&out, 32768),
        Err(FoundryError::UnsupportedMode(_))
    ));
    e.revision=call(&engine,json!({"op":"apply","expected_revision":e.revision,"edits":[{"op":"fold","entry_id":"huge","summary":"Recover the original quoted diagnostic if needed."}]})).unwrap()["revision"].as_u64().unwrap();
    assert!(
        pack(
            &context(&engine, &e, "retry_delay", Strategy::Search).unwrap(),
            2048
        )
        .is_ok()
    );
}
#[test]
fn recovery_search_finds_folded_and_deselected_originals_with_exact_utf8_offsets_after_restart() {
    let mut e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let payload = format!(
        "{}RÉSUMÉ retry-after: 73 seconds.{}",
        "🦀αβ".repeat(70),
        "γδ".repeat(100)
    );
    e.revision=call(&engine,json!({"op":"apply","expected_revision":e.revision,"edits":[add("large","observation",&payload),{"op":"fold","entry_id":"large","summary":"Full test output retained."},{"op":"select","entry_id":"large","selected":false}]})).unwrap()["revision"].as_u64().unwrap();
    drop(engine);
    let engine = Engine::open_existing(&e.store).unwrap();
    let result=call(&engine,json!({"op":"search","expected_revision":e.revision,"query":"RETRY-after:","limit":4,"max_bytes":4096})).unwrap();
    assert_eq!(result["matching_entries"], 1);
    let hit = &result["hits"][0];
    assert_eq!(hit["entry_id"], "large");
    let start = hit["offset"].as_u64().unwrap() as usize;
    let end = hit["end"].as_u64().unwrap() as usize;
    assert_eq!(hit["excerpt"], &payload[start..end]);
    assert_eq!(hit["sha256"], context_foundry::digest(payload.as_bytes()));
    assert_eq!(
        &payload[hit["match_start"].as_u64().unwrap() as usize
            ..hit["match_end"].as_u64().unwrap() as usize],
        "retry-after:"
    );
    let peek = call(
        &engine,
        json!({"op":"peek","entry_id":"large","offset":start,"bytes":end-start}),
    )
    .unwrap();
    assert_eq!(peek["text"], hit["excerpt"]);
}
#[test]
fn search_pagination_history_byte_limits_and_revision_conflicts_are_explicit() {
    let mut e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let mut replaced = add("z-new", "decision", "matching updated decision");
    replaced["entry"]["supersedes"] = json!("a-old");
    e.revision=call(&engine,json!({"op":"apply","expected_revision":e.revision,"edits":[add("a-old","decision","matching original decision"),add("b","observation","matching second"),add("c","observation","matching third"),replaced]})).unwrap()["revision"].as_u64().unwrap();
    let mut command = json!({"op":"search","expected_revision":e.revision,"query":"matching","limit":1,"max_bytes":2048});
    let mut ids = Vec::new();
    loop {
        let page = call(&engine, command.clone()).unwrap();
        assert_eq!(page["matching_entries"], 3);
        assert!(serde_json::to_vec(&page).unwrap().len() < 2048);
        ids.push(page["hits"][0]["entry_id"].as_str().unwrap().to_owned());
        if page["next_after_id"].is_null() {
            break;
        }
        command["after_id"] = page["next_after_id"].clone();
    }
    assert_eq!(ids, ["b", "c", "z-new"]);
    command.as_object_mut().unwrap().remove("after_id");
    command["include_history"] = json!(true);
    command["limit"] = json!(32);
    let all = call(&engine, command.clone()).unwrap();
    assert_eq!(all["matching_entries"], 4);
    assert_eq!(all["hits"][0]["historical"], true);
    let first_page_bytes = serde_json::to_vec(&all).unwrap().len();
    command["max_bytes"] = json!(first_page_bytes - 20);
    let limited = call(&engine, command.clone()).unwrap();
    assert!(limited["hits"].as_array().unwrap().len() < 4);
    assert!(limited["next_after_id"].is_string());
    call(&engine,json!({"op":"apply","expected_revision":e.revision,"edits":[add("later","work","new obligation")]})).unwrap();
    assert!(matches!(
        call(&engine, command),
        Err(FoundryError::Conflict(_))
    ));
}
#[test]
fn cli_task_context_matches_library_pack_and_keeps_the_original_observation_recoverable() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let expected = pack(
        &context(&engine, &e, "retry_delay", Strategy::Auto).unwrap(),
        1024,
    )
    .unwrap()
    .text;
    drop(engine);
    let out = Command::new(BIN)
        .arg("--store")
        .arg(&e.store)
        .args([
            "context",
            "retry_delay",
            "--tokens",
            "1024",
            "--task",
            "retry-task",
            "--task-revision",
            &e.revision.to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8(out.stdout).unwrap(), expected);
    let report: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(report["stdout_tokens"], response::count_tokens(&expected));
}

type Client = rmcp::service::RunningService<rmcp::RoleClient, ()>;
async fn client(e: &Env, args: &[String]) -> Client {
    let mut command = tokio::process::Command::new(BIN);
    command
        .arg("--store")
        .arg(&e.store)
        .arg("mcp")
        .arg("--root")
        .arg(&e.root)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    ().serve(TokioChildProcess::new(command).unwrap())
        .await
        .unwrap()
}
async fn tool(client: &Client, name: &str, args: Value) -> CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new(name.to_owned())
                .with_arguments(args.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}
fn tool_text(result: &CallToolResult) -> &str {
    let rmcp::model::ContentBlock::Text(text) = &result.content[0] else {
        panic!("text result")
    };
    &text.text
}
fn code(result: &CallToolResult) -> String {
    assert_eq!(result.is_error, Some(true));
    serde_json::from_str::<Value>(tool_text(result)).unwrap()["code"]
        .as_str()
        .unwrap()
        .to_owned()
}
#[tokio::test]
async fn mcp_advertises_only_opted_in_task_context_and_matches_cli_delivery() {
    let e = env();
    let args = json!({"query":"retry_delay","tokens":1024,"task_id":"retry-task","task_revision":e.revision});
    let default = client(&e, &[]).await;
    let catalog = default.list_tools(None).await.unwrap();
    assert!(
        !catalog
            .tools
            .iter()
            .find(|t| t.name == "context")
            .unwrap()
            .input_schema["properties"]
            .as_object()
            .unwrap()
            .contains_key("task_id")
    );
    assert_eq!(
        code(&tool(&default, "context", args.clone()).await),
        "unsupported_mode"
    );
    default.cancel().await.unwrap();
    let enabled = client(&e, &["--experimental-task-context".into()]).await;
    let catalog = enabled.list_tools(None).await.unwrap();
    assert!(
        catalog
            .tools
            .iter()
            .find(|t| t.name == "context")
            .unwrap()
            .input_schema["properties"]
            .as_object()
            .unwrap()
            .contains_key("task_id")
    );
    let result = tool(&enabled, "context", args).await;
    assert_ne!(result.is_error, Some(true), "{result:?}");
    assert!(response::count_tokens(tool_text(&result)) <= 1024);
    assert!(serde_json::to_vec(&result).unwrap().len() <= response::BYTE_CAP);
    view(tool_text(&result));
    for invalid in [
        json!({"query":"retry_delay","task_id":"retry-task"}),
        json!({"query":"retry_delay","task_revision":e.revision}),
        json!({"query":"retry_delay","task_id":null,"task_revision":e.revision}),
    ] {
        assert_eq!(
            code(&tool(&enabled, "context", invalid).await),
            "invalid_argument"
        );
    }
    let found=tool(&enabled,"task_context",json!({"workspace_id":e.wid,"task_id":"retry-task","command":{"op":"search","expected_revision":e.revision,"query":"73 seconds","limit":2,"max_bytes":2048}})).await;
    assert_ne!(found.is_error, Some(true));
    assert_eq!(
        serde_json::from_str::<Value>(tool_text(&found)).unwrap()["hits"][0]["entry_id"],
        "trace"
    );
    enabled.cancel().await.unwrap();
    let engine = Engine::open_existing(&e.store).unwrap();
    assert_eq!(
        tool_text(&result),
        pack(
            &context(&engine, &e, "retry_delay", Strategy::Auto).unwrap(),
            1024
        )
        .unwrap()
        .text
    );
}
#[tokio::test]
async fn mcp_task_retrieval_refunds_refusals_and_charges_all_delivered_tokens() {
    let e = env();
    let budget = e._dir.path().join("budget.json");
    std::fs::write(&budget,json!({"foundry_budget":{"v":1,"max_context_tokens":1024,"session_context_tokens":1000,"tokenizer_id":"o200k_base","scope":"delivery"}}).to_string()).unwrap();
    let client = client(
        &e,
        &[
            "--experimental-task-context".into(),
            "--budget".into(),
            budget.to_str().unwrap().into(),
        ],
    )
    .await;
    for _ in 0..4 {
        assert_eq!(code(&tool(&client,"context",json!({"query":"retry_delay","tokens":1024,"task_id":"retry-task","task_revision":999})).await),"conflict");
    }
    assert_eq!(code(&tool(&client,"context",json!({"query":"retry_delay","tokens":8,"task_id":"retry-task","task_revision":e.revision})).await),"budget_too_small");
    let mut spent = 0;
    let mut exhausted = false;
    for _ in 0..10 {
        let result=tool(&client,"context",json!({"query":"retry_delay","tokens":1024,"task_id":"retry-task","task_revision":e.revision})).await;
        if result.is_error == Some(true) {
            assert_eq!(code(&result), "budget_exhausted");
            exhausted = true;
            break;
        }
        spent += response::count_tokens(tool_text(&result));
        view(tool_text(&result));
    }
    assert!(spent > 0 && spent <= 1000 && exhausted);
    client.cancel().await.unwrap();
}
#[tokio::test]
async fn multi_root_context_includes_primary_task_once_and_requires_primary_selection() {
    let primary = env();
    let reference = env();
    let client = client(
        &primary,
        &[
            "--experimental-task-context".into(),
            "--reference".into(),
            format!("{}={}", reference.root.display(), reference.store.display()),
        ],
    )
    .await;
    let result=tool(&client,"context",json!({"query":"retry_delay","tokens":2048,"task_id":"retry-task","task_revision":primary.revision})).await;
    assert_ne!(result.is_error, Some(true), "{result:?}");
    let text = tool_text(&result);
    assert_eq!(text.matches("task-view: ").count(), 1);
    assert_eq!(view(text)["workspace_id"], primary.wid);
    assert!(text.contains(&reference.wid[..16]));
    assert!(response::count_tokens(text) <= 2048);
    let refused=tool(&client,"context",json!({"query":"retry_delay","tokens":2048,"task_id":"retry-task","task_revision":primary.revision,"roots":["ref1"]})).await;
    assert_eq!(code(&refused), "unsupported_mode");
    client.cancel().await.unwrap();
}

#[test]
fn mandatory_view_close_to_byte_cap_accounts_for_the_source_header_too() {
    let seed = env();
    let engine = Engine::open_existing(&seed.store).unwrap();
    call(&engine,json!({"op":"apply","expected_revision":seed.revision,"edits":[add("near-cap","observation","x")]})).unwrap();
    let view = call(&engine, json!({"op":"render","max_bytes":1048576})).unwrap();
    let line = format!(
        "task-view: {}\n",
        json!({"sha256":context_foundry::digest(&serde_json::to_vec(&view).unwrap()),"view":view})
    );
    let mut e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    // Spaces keep token usage well below the token cap while approaching
    // the byte cap. A nonblank prefix keeps this a valid observation.
    let payload = format!("x{}", " ".repeat(response::BYTE_CAP - 8 - line.len()));
    e.revision=call(&engine,json!({"op":"apply","expected_revision":e.revision,"edits":[add("near-cap","observation",&payload)]})).unwrap()["revision"].as_u64().unwrap();
    let view = call(&engine, json!({"op":"render","max_bytes":1048576})).unwrap();
    let line = format!(
        "task-view: {}\n",
        json!({"sha256":context_foundry::digest(&serde_json::to_vec(&view).unwrap()),"view":view})
    );
    assert_eq!(line.len(), response::BYTE_CAP - 8);
    assert!(response::count_tokens(&line) < response::MAX_BUDGET_TOKENS);
    for query in ["retry_delay", "retry delay"] {
        let out = context(&engine, &e, query, Strategy::Search).unwrap();
        // The task line alone fits, but the mandatory context header takes
        // the response over the cap. More tokens cannot repair a byte refusal.
        assert!(matches!(
            pack(&out, 32768),
            Err(FoundryError::UnsupportedMode(reason)) if reason.contains("and context header")
        ));
    }
}
