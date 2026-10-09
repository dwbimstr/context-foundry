//! Synthetic generator outputs against real redb, Tantivy, packer and CLI.
//! No model was used and these fixtures are not real-task quality attribution.
use context_foundry::brief;
use context_foundry::fault::{self, Action, names};
use context_foundry::response::{self, Budget};
use context_foundry::store::ContextOptions;
use context_foundry::{Control, Engine, FoundryError, Strategy};
use redb::{ReadableDatabase, ReadableTable, TableDefinition};
use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const BODY: &str =
    "pub fn qz_reap(q: &mut Vec<u8>, n: usize) {\n    while q.len() > n { q.remove(0); }\n}\n";
const TEST: &str = "#[test]\nfn qz_check() { let mut q = vec![1, 2, 3]; qz_reap(&mut q, 2); assert_eq!(q, vec![2, 3]); }\n";
const QUERY: &str = "capacity eviction";
struct Env {
    _dir: tempfile::TempDir,
    store: PathBuf,
    wid: String,
}
fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let store = dir.path().join("store");
    let mut engine = Engine::initialize(&store, &root).unwrap();
    let wid = engine.workspace_id().unwrap();
    engine.replace_source("opaque.rs", BODY).unwrap();
    engine.replace_source("checks.rs", TEST).unwrap();
    drain(&mut engine);
    Env {
        _dir: dir,
        store,
        wid,
    }
}
fn drain(engine: &mut Engine) {
    while engine.pending().unwrap() != 0 {
        engine.refresh_index(&Control::unbounded()).unwrap();
    }
}
fn handle(wid: &str, path: &str, body: &str) -> String {
    format!(
        "{path}#0-{}@{}.{}",
        body.len(),
        &context_foundry::digest(body.as_bytes())[..32],
        &wid[..16]
    )
}
fn input(wid: &str) -> Value {
    json!({"cited_spans":[handle(wid,"opaque.rs",BODY),handle(wid,"checks.rs",TEST)],
        "recipe_sha256":"a".repeat(64),"model":"synthetic-fixture/no-provider",
        "artifact_sha256":"b".repeat(64),"prompt_sha256":"c".repeat(64),"generated_at_unix_ms":1,
        "purpose":[{"text":"capacity eviction","citations":[0],"identifiers":["qz_reap"]}],
        "behavior":[{"text":"Remove oldest queued entries until length fits capacity.","citations":[0],"identifiers":[]}],
        "provides":[],"consumes":[],"constraints":[],
        "tests":[{"text":"Example removes the first element.","citations":[1],"identifiers":["qz_check"]}],
        "unknowns":["Thread safety and amortized performance are unknown."]})
}
fn call(engine: &Engine, command: Value) -> Result<Value, FoundryError> {
    let request = brief::parse(
        &serde_json::to_vec(&json!({"workspace_id":engine.workspace_id(),"command":command}))
            .unwrap(),
    )?;
    engine.brief(&request, &Control::unbounded())
}
fn put(engine: &Engine) -> Value {
    call(
        engine,
        json!({"op":"put","brief":input(&engine.workspace_id().unwrap())}),
    )
    .unwrap()
}
fn search(engine: &Engine) -> Value {
    call(
        engine,
        json!({"op":"search","query":QUERY,"limit":8,"max_bytes":262144}),
    )
    .unwrap()
}
fn context(engine: &Engine, query: &str, briefs: bool) -> context_foundry::memory::MemoryContext {
    engine
        .context_candidates_with(
            query,
            Strategy::Search,
            &Control::unbounded(),
            &ContextOptions {
                briefs,
                ..Default::default()
            },
        )
        .unwrap()
}
fn pack(context: &context_foundry::memory::MemoryContext, tokens: usize) -> response::PackedText {
    response::pack_enriched_context(
        context,
        None,
        Budget::request(tokens),
        &response::stdout_bytes,
    )
    .unwrap()
}
#[test]
fn conceptual_vocabulary_recovers_opaque_source_and_tests_without_changing_exact_names() {
    let e = env();
    let mut engine = Engine::open_existing(&e.store).unwrap();
    // Frozen constructed baseline: neither source contains either query term.
    assert!(context(&engine, QUERY, false).batch.items.is_empty());
    let exact_before = pack(&context(&engine, "qz_reap", false), 4096).text;
    put(&engine);
    drain(&mut engine);
    assert!(context(&engine, QUERY, false).batch.items.is_empty());
    assert_eq!(
        pack(&context(&engine, "qz_reap", true), 4096).text,
        exact_before
    );
    let combined = context(&engine, QUERY, true);
    assert_eq!(combined.briefs.len(), 1);
    assert_eq!(
        combined.batch.items[0].handle.as_ref().unwrap().path,
        "opaque.rs"
    );
    let text = pack(&combined, 4096).text;
    assert!(text.contains(BODY));
    assert!(text.contains("brief: {\"kind\":\"derived\""));
    for (i, original) in [BODY, TEST].iter().enumerate() {
        let exact = engine
            .retrieve(&combined.briefs[0].brief.cited_spans[i], None, 4096)
            .unwrap();
        assert_eq!(exact.span, original.as_bytes());
    }
    assert_eq!(search(&engine)["hits"].as_array().unwrap().len(), 1);
}
#[test]
fn source_or_dependency_edit_delete_and_pending_index_never_serve_old_text_as_current() {
    for (path, deletion) in [
        ("opaque.rs", false),
        ("checks.rs", false),
        ("opaque.rs", true),
        ("checks.rs", true),
    ] {
        let e = env();
        let mut engine = Engine::open_existing(&e.store).unwrap();
        let r = put(&engine);
        drain(&mut engine);
        if deletion {
            engine.delete_source(path).unwrap();
        } else {
            engine.replace_source(path, "// changed\n").unwrap();
        }
        let stale = call(&engine, json!({"op":"get","id":r["id"]})).unwrap();
        assert_eq!(stale["eligible"], false);
        assert!(!stale.to_string().contains(QUERY));
        assert!(search(&engine)["hits"].as_array().unwrap().is_empty());
        assert_eq!(search(&engine)["stale_candidates"], 1);
        assert!(context(&engine, QUERY, true).briefs.is_empty());
        let exported = call(&engine, json!({"op":"export","id":r["id"]})).unwrap();
        assert_eq!(exported["eligible"], false);
        assert!(exported.to_string().contains(QUERY));
        // A late result cannot pass current source validation, even on replay.
        assert!(call(&engine, json!({"op":"put","brief":input(&e.wid)})).is_err());
    }
}
#[test]
fn replay_cas_replacement_and_forget_are_monotonic_and_do_not_leak_stale_documents() {
    let e = env();
    let mut engine = Engine::open_existing(&e.store).unwrap();
    let r = put(&engine);
    drain(&mut engine);
    let mut next = input(&e.wid);
    next["generated_at_unix_ms"] = json!(2);
    assert_eq!(
        call(&engine, json!({"op":"put","brief":next})).unwrap()["outcome"],
        "unchanged"
    );
    assert_eq!(engine.pending().unwrap(), 0);
    next["purpose"][0]["text"] = json!("Different model output for same input");
    assert!(matches!(
        call(
            &engine,
            json!({"op":"put","brief":next,"expected_revision":r["revision"]})
        ),
        Err(FoundryError::Conflict(_))
    ));
    next["prompt_sha256"] = json!("d".repeat(64));
    assert!(matches!(
        call(&engine, json!({"op":"put","brief":next})),
        Err(FoundryError::Conflict(_))
    ));
    let replaced = call(
        &engine,
        json!({"op":"put","brief":next,"expected_revision":r["revision"]}),
    )
    .unwrap();
    assert_eq!(replaced["id"], r["id"]);
    assert_eq!(replaced["revision"], 2);
    assert!(search(&engine)["hits"].as_array().unwrap().is_empty());
    drain(&mut engine);
    // Behavior still contains capacity, so the new record can match.
    assert_eq!(search(&engine)["hits"][0]["revision"], 2);
    assert!(matches!(
        call(
            &engine,
            json!({"op":"forget","id":r["id"],"expected_revision":1})
        ),
        Err(FoundryError::Conflict(_))
    ));
    let forgotten = call(
        &engine,
        json!({"op":"forget","id":r["id"],"expected_revision":2}),
    )
    .unwrap();
    assert_eq!(forgotten["revision"], 3);
    assert!(!forgotten.to_string().contains(QUERY));
    assert!(search(&engine)["hits"].as_array().unwrap().is_empty());
    assert!(matches!(
        call(&engine, json!({"op":"export","id":r["id"]})),
        Err(FoundryError::NotFound)
    ));
    assert_eq!(put(&engine)["revision"], 4);
    drain(&mut engine);
    assert_eq!(search(&engine)["hits"][0]["revision"], 4);
}
#[test]
fn edits_during_generation_and_between_candidate_selection_and_delivery_are_revalidated() {
    let e = env();
    let mut engine = Engine::open_existing(&e.store).unwrap();
    fault::arm(
        names::BRIEF_BEFORE_ADMISSION,
        0,
        Action::Call(Box::new(|ctx| {
            ctx.engine
                .unwrap()
                .replace_source("checks.rs", "// edited while model ran\n")
                .unwrap();
        })),
    );
    assert!(matches!(
        call(&engine, json!({"op":"put","brief":input(&e.wid)})),
        Err(FoundryError::StaleHandle)
    ));
    fault::disarm_all();
    engine.replace_source("checks.rs", TEST).unwrap();
    put(&engine);
    drain(&mut engine);
    for point in [
        names::BRIEF_BEFORE_FINAL_VALIDATION,
        names::CONTEXT_BEFORE_FINAL_VALIDATION,
    ] {
        fault::arm(
            point,
            0,
            Action::Call(Box::new(|ctx| {
                ctx.engine
                    .unwrap()
                    .replace_source("checks.rs", "// edited after query\n")
                    .unwrap();
            })),
        );
        if point == names::BRIEF_BEFORE_FINAL_VALIDATION {
            assert!(search(&engine)["hits"].as_array().unwrap().is_empty());
        } else {
            assert!(context(&engine, QUERY, true).briefs.is_empty());
        }
        fault::disarm_all();
        engine.replace_source("checks.rs", TEST).unwrap();
    }
}
#[test]
fn malformed_unknown_fields_unsupported_identifiers_utf8_and_bounds_refuse_without_writes() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let mut bads = Vec::new();
    let mut bad = input(&e.wid);
    bad["extra"] = json!(true);
    bads.push(bad);
    let mut bad = input(&e.wid);
    bad["purpose"][0]["identifiers"] = json!(["imagined_api"]);
    bads.push(bad);
    let mut bad = input(&e.wid);
    bad["purpose"][0]["citations"] = json!([32]);
    bads.push(bad);
    let mut bad = input(&e.wid);
    bad["purpose"][0]["citations"] = json!([]);
    bads.push(bad);
    let mut bad = input(&e.wid);
    bad["recipe_sha256"] = json!("unknown");
    bads.push(bad);
    let mut bad = input(&e.wid);
    bad["unknowns"] = json!(["x".repeat(16385)]);
    bads.push(bad);
    let mut bad = input(&e.wid);
    bad["cited_spans"] = json!(vec![handle(&e.wid, "opaque.rs", BODY); 33]);
    bads.push(bad);
    let mut bad = input(&e.wid);
    bad["cited_spans"][0] = json!(handle(&"f".repeat(64), "opaque.rs", BODY));
    bads.push(bad);
    for bad in bads {
        assert!(call(&engine, json!({"op":"put","brief":bad})).is_err());
    }
    assert_eq!(engine.pending().unwrap(), 0);
    engine.replace_source("unicode.txt", "é").unwrap();
    let mut bad = input(&e.wid);
    bad["cited_spans"][0] = json!(handle(&e.wid, "unicode.txt", "é").replace("#0-2", "#1-2"));
    assert!(matches!(
        call(&engine, json!({"op":"put","brief":bad})),
        Err(FoundryError::InvalidRange)
    ));
    assert!(brief::parse(&vec![b' '; brief::MAX_REQUEST_BYTES + 1]).is_err());
    assert_eq!(put(&engine)["revision"], 1);
}
#[test]
fn misleading_summary_remains_derived_and_never_replaces_required_source_under_budget() {
    let e = env();
    let mut engine = Engine::open_existing(&e.store).unwrap();
    let mut misleading = input(&e.wid);
    // Citations are all valid, yet this prose is intentionally false.
    misleading["behavior"][0]["text"] =
        json!("Eviction removes newest entries and is thread safe.");
    call(&engine, json!({"op":"put","brief":misleading})).unwrap();
    drain(&mut engine);
    let combined = context(&engine, QUERY, true);
    for tokens in [128, 256, 512, 1024, 2048, 4096] {
        let out = pack(&combined, tokens);
        assert!(out.tokens <= tokens);
        assert!(out.text.len() <= 256 * 1024);
        if out.text.contains("brief:") {
            assert!(out.text.contains(BODY));
        }
        assert!(!out.text.contains("verified_complete"));
    }
    let full = pack(&combined, 4096).text;
    assert!(full.contains("\"kind\":\"derived\""));
    assert!(full.contains("newest"));
    assert!(full.contains("q.remove(0)"));
    // A source too large for the allowance cannot be silently replaced by prose.
    let large = format!(
        "{BODY}{}",
        "// ballast alpha beta gamma delta\n".repeat(2000)
    );
    engine.replace_source("opaque.rs", &large).unwrap();
    let mut next = misleading;
    next["cited_spans"][0] = json!(handle(&e.wid, "opaque.rs", &large));
    call(
        &engine,
        json!({"op":"put","brief":next,"expected_revision":1}),
    )
    .unwrap();
    drain(&mut engine);
    let out = pack(&context(&engine, QUERY, true), 2048);
    assert!(!out.text.contains("brief:"));
    assert!(out.truncated);
}
#[test]
fn multiple_recipes_deduplicate_source_and_search_output_obeys_byte_budget() {
    let e = env();
    let mut engine = Engine::open_existing(&e.store).unwrap();
    put(&engine);
    let mut second = input(&e.wid);
    second["recipe_sha256"] = json!("d".repeat(64));
    call(&engine, json!({"op":"put","brief":second})).unwrap();
    drain(&mut engine);
    assert_eq!(search(&engine)["hits"].as_array().unwrap().len(), 1);
    assert_eq!(context(&engine, QUERY, true).briefs.len(), 1);
    let out = search(&engine);
    let size = serde_json::to_vec(&out).unwrap().len() + 1;
    let exact = call(
        &engine,
        json!({"op":"search","query":QUERY,"limit":8,"max_bytes":size}),
    )
    .unwrap();
    assert!(serde_json::to_vec(&exact).unwrap().len() < size);
    assert!(
        call(
            &engine,
            json!({"op":"search","query":QUERY,"limit":8,"max_bytes":size-1})
        )
        .is_err()
    );
    // Unknown capability text is not indexed as asserted behavior.
    let unknown = call(
        &engine,
        json!({"op":"search","query":"amortized","limit":8,"max_bytes":4096}),
    )
    .unwrap();
    assert!(unknown["hits"].as_array().unwrap().is_empty());
}
fn cli(e: &Env, command: Value) -> Value {
    let request = json!({"workspace_id":e.wid,"command":command});
    let mut child = Command::new(env!("CARGO_BIN_EXE_foundry"))
        .args(["--store", e.store.to_str().unwrap(), "brief"])
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
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
#[test]
fn cli_import_context_get_and_forget_survive_independent_processes() {
    let e = env();
    let r = cli(&e, json!({"op":"put","brief":input(&e.wid)}));
    assert_eq!(r["outcome"], "created");
    assert_eq!(cli(&e, json!({"op":"get","id":r["id"]}))["eligible"], true);
    let out = Command::new(env!("CARGO_BIN_EXE_foundry"))
        .args([
            "--store",
            e.store.to_str().unwrap(),
            "context",
            QUERY,
            "--include-briefs",
            "--tokens",
            "4096",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains(BODY));
    assert!(text.contains("brief:"));
    let r = cli(
        &e,
        json!({"op":"forget","id":r["id"],"expected_revision":r["revision"]}),
    );
    assert_eq!(r["outcome"], "deleted");
    let engine = Engine::open_existing(&e.store).unwrap();
    assert_eq!(engine.pending().unwrap(), 0);
    assert!(search(&engine)["hits"].as_array().unwrap().is_empty());
}
#[test]
fn restart_repair_and_failed_index_commit_converge_on_the_authoritative_row() {
    let e = env();
    let mut engine = Engine::open_existing(&e.store).unwrap();
    let r = put(&engine);
    fault::arm(
        names::INDEX_AFTER_SEARCH_COMMIT,
        0,
        Action::Fail("crash after derived commit".into()),
    );
    assert!(engine.refresh_index(&Control::unbounded()).is_err());
    assert_eq!(engine.pending().unwrap(), 1);
    fault::disarm_all();
    drop(engine);
    let mut engine = Engine::open_existing(&e.store).unwrap();
    drain(&mut engine);
    assert_eq!(search(&engine)["hits"][0]["id"], r["id"]);
    drop(engine);
    Engine::repair_index(&e.store, &Control::unbounded()).unwrap();
    let engine = Engine::open_existing(&e.store).unwrap();
    assert_eq!(search(&engine)["hits"][0]["id"], r["id"]);
}
#[test]
fn brief_fault_child() {
    let Ok(path) = std::env::var("CF_BRIEF_FAULT_STORE") else {
        return;
    };
    let mode = std::env::var("CF_BRIEF_FAULT_MODE").unwrap();
    let engine = Engine::open_existing(std::path::Path::new(&path)).unwrap();
    fault::arm(
        if mode.ends_with("before") {
            names::BRIEF_BEFORE_COMMIT
        } else {
            names::BRIEF_AFTER_COMMIT
        },
        0,
        Action::Abort,
    );
    if mode.starts_with("forget") {
        let r = search(&engine)["hits"][0].clone();
        let _ = call(
            &engine,
            json!({"op":"forget","id":r["id"],"expected_revision":r["revision"]}),
        );
    } else {
        put(&engine);
    }
    panic!("fault not reached");
}
#[test]
fn abrupt_exit_keeps_row_counter_and_pending_key_atomic() {
    for mode in ["put-before", "put-after", "forget-before", "forget-after"] {
        let e = env();
        if mode.starts_with("forget") {
            cli(&e, json!({"op":"put","brief":input(&e.wid)}));
        }
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "brief_fault_child"])
            .env("CF_BRIEF_FAULT_STORE", &e.store)
            .env("CF_BRIEF_FAULT_MODE", mode)
            .output()
            .unwrap();
        assert!(!out.status.success());
        let db = redb::Database::open(e.store.join("knowledge.redb")).unwrap();
        let tx = db.begin_read().unwrap();
        let meta = tx
            .open_table(TableDefinition::<&str, &str>::new("meta"))
            .unwrap();
        let expected = match mode {
            "put-before" => "0",
            "put-after" | "forget-before" => "1",
            _ => "2",
        };
        assert_eq!(
            meta.get("brief_revision").unwrap().unwrap().value(),
            expected
        );
        drop(meta);
        drop(tx);
        drop(db);
        let mut engine = Engine::open_existing(&e.store).unwrap();
        assert_eq!(
            engine.pending().unwrap(),
            u64::from(mode.ends_with("after"))
        );
        drain(&mut engine);
        assert_eq!(
            search(&engine)["hits"].as_array().unwrap().len(),
            usize::from(matches!(mode, "put-after" | "forget-before"))
        );
        let r = put(&engine);
        assert_eq!(r["revision"], if mode == "forget-after" { 3 } else { 1 });
        drain(&mut engine);
        assert_eq!(search(&engine)["hits"].as_array().unwrap().len(), 1);
    }
}
#[test]
fn schema_seven_upgrade_is_explicit_atomic_and_preserves_task_and_memory() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let task = context_foundry::task_context::parse(json!({"workspace_id":e.wid,"task_id":"kept","command":{"op":"create","title":"Kept task"}})).unwrap();
    engine.task_context(&task, &Control::unbounded()).unwrap();
    engine
        .memory_put(&context_foundry::memory::PutInput {
            workspace_id: e.wid.clone(),
            fields: context_foundry::memory::RecordFields {
                id: "kept".into(),
                text: "Keep explicit memory".into(),
                author: "operator".into(),
                provenance: "before upgrade".into(),
                source_links: vec![],
            },
        })
        .unwrap();
    drop(engine);
    let db = redb::Database::open(e.store.join("knowledge.redb")).unwrap();
    let tx = db.begin_write().unwrap();
    tx.delete_table(TableDefinition::<&str, &str>::new("source_briefs"))
        .unwrap();
    {
        let mut meta = tx
            .open_table(TableDefinition::<&str, &str>::new("meta"))
            .unwrap();
        meta.insert("schema", "7").unwrap();
        meta.remove("brief_revision").unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    assert!(matches!(
        Engine::open_existing(&e.store),
        Err(FoundryError::UpgradeRequired { .. })
    ));
    fault::arm(
        names::UPGRADE_BEFORE_COMMIT,
        0,
        Action::Fail("stop before commit".into()),
    );
    assert!(Engine::upgrade_store(&e.store, 8, &Control::unbounded()).is_err());
    assert!(matches!(
        Engine::open_existing(&e.store),
        Err(FoundryError::UpgradeRequired { .. })
    ));
    fault::disarm_all();
    Engine::upgrade_store(&e.store, 8, &Control::unbounded()).unwrap();
    let engine = Engine::open_existing(&e.store).unwrap();
    assert_eq!(engine.status().unwrap().source_revision, 2);
    assert_eq!(
        engine.memory_get("kept", &e.wid).unwrap().text,
        "Keep explicit memory"
    );
    let task = context_foundry::task_context::parse(
        json!({"workspace_id":e.wid,"task_id":"kept","command":{"op":"inspect"}}),
    )
    .unwrap();
    assert_eq!(
        engine.task_context(&task, &Control::unbounded()).unwrap()["revision"],
        1
    );
    assert_eq!(put(&engine)["revision"], 1);
}
#[test]
fn corrupt_brief_is_named_without_poisoning_source_repair() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let r = put(&engine);
    drop(engine);
    let db = redb::Database::open(e.store.join("knowledge.redb")).unwrap();
    let tx = db.begin_write().unwrap();
    {
        let mut table = tx
            .open_table(TableDefinition::<&str, &str>::new("source_briefs"))
            .unwrap();
        table.insert(r["id"].as_str().unwrap(), "{}").unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    Engine::repair_index(&e.store, &Control::unbounded()).unwrap();
    let engine = Engine::open_existing(&e.store).unwrap();
    assert!(matches!(
        call(&engine, json!({"op":"get","id":r["id"]})),
        Err(FoundryError::CorruptStore(_))
    ));
    assert_eq!(
        engine
            .retrieve(&handle(&e.wid, "opaque.rs", BODY), None, 4096)
            .unwrap()
            .span,
        BODY.as_bytes()
    );
}

#[test]
fn forget_between_selection_and_delivery_drops_the_whole_derived_record() {
    for point in [
        names::BRIEF_BEFORE_FINAL_VALIDATION,
        names::CONTEXT_BEFORE_FINAL_VALIDATION,
    ] {
        let e = env();
        let mut engine = Engine::open_existing(&e.store).unwrap();
        let r = put(&engine);
        drain(&mut engine);
        fault::arm(
            point,
            0,
            Action::Call(Box::new(move |ctx| {
                call(
                    ctx.engine.unwrap(),
                    json!({"op":"forget","id":r["id"],"expected_revision":r["revision"]}),
                )
                .unwrap();
            })),
        );
        if point == names::BRIEF_BEFORE_FINAL_VALIDATION {
            assert!(search(&engine)["hits"].as_array().unwrap().is_empty());
        } else {
            let out = context(&engine, QUERY, true);
            assert!(out.briefs.is_empty());
            assert!(out.batch.items.is_empty());
        }
        fault::disarm_all();
    }
}
#[test]
fn brief_namespace_cannot_collide_with_a_source_path_and_forget_preserves_user_memory() {
    let e = env();
    let mut engine = Engine::open_existing(&e.store).unwrap();
    let r = put(&engine);
    let path = format!("brief:{}", r["id"].as_str().unwrap());
    engine
        .replace_source(&path, "namespace sentinel\n")
        .unwrap();
    engine
        .memory_put(&context_foundry::memory::PutInput {
            workspace_id: e.wid.clone(),
            fields: context_foundry::memory::RecordFields {
                id: "keep".into(),
                text: "Explicit decision survives generated knowledge".into(),
                author: "operator".into(),
                provenance: "user decision".into(),
                source_links: vec![],
            },
        })
        .unwrap();
    drain(&mut engine);
    call(
        &engine,
        json!({"op":"forget","id":r["id"],"expected_revision":r["revision"]}),
    )
    .unwrap();
    drain(&mut engine);
    assert_eq!(
        engine
            .retrieve(&handle(&e.wid, &path, "namespace sentinel\n"), None, 4096)
            .unwrap()
            .span,
        b"namespace sentinel\n"
    );
    assert!(
        engine
            .memory_get("keep", &e.wid)
            .unwrap()
            .text
            .contains("Explicit decision")
    );
}
#[test]
fn current_schema_missing_briefs_is_corruption_and_admission_hash_checks_source_bytes() {
    let e = env();
    let db = redb::Database::open(e.store.join("knowledge.redb")).unwrap();
    let tx = db.begin_write().unwrap();
    {
        let mut chunks = tx
            .open_table(TableDefinition::<&str, &str>::new("chunks"))
            .unwrap();
        let (key, mut raw) = chunks
            .iter()
            .unwrap()
            .find_map(|row| {
                let (key, raw) = row.unwrap();
                let value: Value = serde_json::from_str(raw.value()).unwrap();
                (value["path"] == "opaque.rs").then(|| (key.value().to_owned(), value))
            })
            .unwrap();
        raw["body"] = json!(BODY.replace("remove(0)", "remove(1)"));
        chunks
            .insert(key.as_str(), raw.to_string().as_str())
            .unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    let engine = Engine::open_existing(&e.store).unwrap();
    assert!(call(&engine, json!({"op":"put","brief":input(&e.wid)})).is_err());
    assert_eq!(engine.pending().unwrap(), 0);
    drop(engine);
    let db = redb::Database::open(e.store.join("knowledge.redb")).unwrap();
    let tx = db.begin_write().unwrap();
    tx.delete_table(TableDefinition::<&str, &str>::new("source_briefs"))
        .unwrap();
    tx.commit().unwrap();
    drop(db);
    assert!(matches!(
        Engine::open_existing(&e.store),
        Err(FoundryError::CorruptStore(_))
    ));
}

#[test]
fn escaped_dependency_paths_fit_the_record_bound_and_pending_replacement_is_not_cleared() {
    let e = env();
    let mut engine = Engine::open_existing(&e.store).unwrap();
    let mut large = input(&e.wid);
    let mut citations = vec![];
    for i in 0..32 {
        let path = format!("{i}/{}", "\"".repeat(3500));
        engine.replace_source(&path, BODY).unwrap();
        citations.push(handle(&e.wid, &path, BODY));
    }
    large["cited_spans"] = json!(citations);
    large["tests"] = json!([]);
    let r = call(&engine, json!({"op":"put","brief":large})).unwrap();
    let read = call(&engine, json!({"op":"get","id":r["id"]})).unwrap();
    assert_eq!(read["eligible"], true);
    assert!(serde_json::to_vec(&read).unwrap().len() < 513 * 1024);
    // One brief updated between derived commit and pending clear must remain queued.
    let r = put(&engine);
    let wid = e.wid.clone();
    fault::arm(
        names::INDEX_AFTER_SEARCH_COMMIT,
        0,
        Action::Call(Box::new(move |ctx| {
            let mut next = input(&wid);
            next["prompt_sha256"] = json!("d".repeat(64));
            call(
                ctx.engine.unwrap(),
                json!({"op":"put","brief":next,"expected_revision":r["revision"]}),
            )
            .unwrap();
        })),
    );
    engine.refresh_index(&Control::unbounded()).unwrap();
    fault::disarm_all();
    assert_eq!(engine.pending().unwrap(), 1);
    drain(&mut engine);
    assert_eq!(engine.pending().unwrap(), 0);
}
