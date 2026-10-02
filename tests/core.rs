use context_foundry::fault::{self, Action, names};
use context_foundry::testkit;
use context_foundry::testkit::{
    CORRUPT_INDEX_BYTES, KEPT_BODY, corrupt_search_index, craft_v1_store, quarantine_dirs,
    schema_marker,
};
use context_foundry::{
    Control, Engine, FoundryError, Strategy, digest,
    graph::{Edge, Endpoint, GraphBundle},
    laya::{Feedback, Strategy as LayaStrategy},
    response,
    store::SourceHandle,
};
use std::path::Path;

fn drain(engine: &mut Engine) {
    engine.refresh(&Control::unbounded()).unwrap();
}

fn endpoint(path: &str, body: &str) -> Endpoint {
    Endpoint {
        path: path.into(),
        line: 1,
        symbol: path.into(),
        hash: digest(body.as_bytes()),
    }
}

fn code(error: &FoundryError) -> &str {
    error.code()
}

fn setup(root: &Path) -> (tempfile::TempDir, Engine) {
    let store = tempfile::tempdir().unwrap();
    std::fs::create_dir(root).unwrap();
    let engine = Engine::initialize(store.path(), root).unwrap();
    (store, engine)
}

#[test]
fn explicit_initialize_and_existing_only_open() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    std::fs::create_dir(&root).unwrap();
    let store = fixture.path().join("store");
    // Missing store: reads create nothing and name store_not_found.
    assert!(!store.exists());
    let err = Engine::open_existing(&store).unwrap_err();
    assert_eq!(code(&err), "store_not_found");
    assert!(!store.exists(), "open must not create filesystem state");
    // Explicit initialization accepts a missing directory.
    let engine = Engine::initialize(&store, &root).unwrap();
    assert_eq!(engine.status().unwrap().schema, 2);
    assert!(engine.workspace_id().is_some());
    // Nonempty directory without a store is unrecognized.
    let stray = fixture.path().join("stray");
    std::fs::create_dir(&stray).unwrap();
    std::fs::write(stray.join("file.txt"), "x").unwrap();
    let err = Engine::initialize(&stray, &root).unwrap_err();
    assert_eq!(code(&err), "unrecognized_store");
    // Invalid root never initializes a store.
    let never = fixture.path().join("never");
    let err = Engine::initialize(&never, &fixture.path().join("missing-root")).unwrap_err();
    assert_eq!(code(&err), "invalid_argument");
    assert!(!never.exists());
}

#[test]
fn revision_counts_only_source_changes() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (store, mut engine) = setup(&root);
    assert_eq!(engine.source_revision().unwrap(), 0);
    engine.replace_source("a.rs", "one\n").unwrap();
    assert_eq!(engine.source_revision().unwrap(), 1);
    // Idempotent replay does not bump the revision.
    assert!(!engine.replace_source("a.rs", "one\n").unwrap());
    assert_eq!(engine.source_revision().unwrap(), 1);
    engine.replace_source("a.rs", "two\n").unwrap();
    assert_eq!(engine.source_revision().unwrap(), 2);
    assert!(engine.delete_source("a.rs").unwrap());
    assert_eq!(engine.source_revision().unwrap(), 3);
    assert!(!engine.delete_source("a.rs").unwrap());
    assert_eq!(engine.source_revision().unwrap(), 3);
    drain(&mut engine);
    // Refresh and feedback do not touch the revision.
    let feedback = Feedback {
        task_id: "t".into(),
        query: "q".into(),
        correct_strategy: LayaStrategy::Search,
        label_source: "operator".into(),
        allow_training: false,
    };
    engine.record_feedback(&feedback).unwrap();
    assert_eq!(engine.source_revision().unwrap(), 3);
    drop(engine);
    let engine = Engine::open_existing(store.path()).unwrap();
    assert_eq!(engine.source_revision().unwrap(), 3);
}

#[test]
fn changes_are_durable_and_stale_candidates_are_rejected() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (store, mut engine) = setup(&root);
    engine.replace_source("a.rs", "fn oldname() {}\n").unwrap();
    drain(&mut engine);
    assert_eq!(engine.search("oldname", 10).unwrap().hits.len(), 1);
    engine.replace_source("a.rs", "fn newname() {}\n").unwrap();
    let stale = engine.search("oldname", 10).unwrap();
    assert!(stale.hits.is_empty());
    assert_eq!(stale.stale_candidates, 1);
    assert_eq!(stale.pending_sources, 1);
    drop(engine);
    let mut engine = Engine::open_existing(store.path()).unwrap();
    assert_eq!(engine.pending().unwrap(), 1);
    drain(&mut engine);
    assert_eq!(engine.search("newname", 10).unwrap().hits.len(), 1);
    assert!(!engine.replace_source("a.rs", "fn newname() {}\n").unwrap());
    engine.delete_source("a.rs").unwrap();
    assert!(engine.search("newname", 10).unwrap().hits.is_empty());
    drain(&mut engine);
    assert_eq!(engine.pending().unwrap(), 0);
}

#[test]
fn missing_or_corrupt_derived_index_degrades_to_repair_required() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (store, mut engine) = setup(&root);
    engine.replace_source("a.rs", "durable source\n").unwrap();
    drain(&mut engine);
    let handle = engine.search("durable", 5).unwrap().hits.remove(0).handle;
    drop(engine);
    std::fs::remove_dir_all(store.path().join("search")).unwrap();
    let engine = Engine::open_existing(store.path()).unwrap();
    // Search names repair_required; no automatic rebuild or enqueue happens.
    let err = engine.search("durable", 5).unwrap_err();
    assert_eq!(code(&err), "repair_required");
    assert_eq!(
        engine.pending().unwrap(),
        0,
        "open must not enqueue rebuild"
    );
    let status = engine.status().unwrap();
    assert_eq!(status.index_state, "repair_required");
    // Authoritative reads stay available without the derived index.
    let outcome = engine.retrieve(&handle.to_json(), 2048).unwrap();
    assert_eq!(
        std::str::from_utf8(&outcome.span).unwrap(),
        "durable source\n"
    );
    // Corrupt index metadata behaves the same.
    std::fs::create_dir_all(store.path().join("search")).unwrap();
    std::fs::write(store.path().join("search").join("meta.json"), "not json").unwrap();
    drop(engine);
    let engine = Engine::open_existing(store.path()).unwrap();
    let err = engine.search("durable", 5).unwrap_err();
    assert_eq!(code(&err), "repair_required");
    let status = engine.status().unwrap();
    assert_eq!(status.index_state, "repair_required");
    assert_eq!(status.pending_count, 0);
    assert_eq!(status.index_state, "repair_required");
}

#[test]
fn explicit_repairs_rebuild_and_keep_one_quarantine() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (store, mut engine) = setup(&root);
    engine.replace_source("a.rs", "repairable\n").unwrap();
    engine.replace_source("b.rs", "also repairable\n").unwrap();
    drain(&mut engine);
    let feedback = Feedback {
        task_id: "t1".into(),
        query: "repair".into(),
        correct_strategy: LayaStrategy::Search,
        label_source: "operator".into(),
        allow_training: true,
    };
    engine.record_feedback(&feedback).unwrap();
    let revision = engine.source_revision().unwrap();
    drop(engine);
    corrupt_search_index(store.path());
    let report = Engine::repair_index(store.path(), &Control::unbounded()).unwrap();
    assert!(report.repaired);
    // The original corrupt derived directory is retained, bytes intact.
    let quarantine = report.quarantined_to.expect("original quarantined");
    assert_eq!(
        std::fs::read(quarantine.join("meta.json")).unwrap(),
        CORRUPT_INDEX_BYTES
    );
    assert_eq!(quarantine_dirs(store.path()), vec![quarantine]);
    let mut engine = Engine::open_existing(store.path()).unwrap();
    assert_eq!(engine.search("repairable", 5).unwrap().hits.len(), 2);
    assert_eq!(engine.status().unwrap().index_state, "ready");
    // Repair never touches sources, revision or feedback.
    assert_eq!(engine.source_revision().unwrap(), revision);
    assert_eq!(engine.training_examples().unwrap().len(), 1);
    drain(&mut engine);
    assert_eq!(engine.pending().unwrap(), 0);
}

#[test]
fn explicit_repair_with_missing_derived_directory_quarantines_nothing() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (store, mut engine) = setup(&root);
    engine.replace_source("a.rs", "recoverable\n").unwrap();
    drain(&mut engine);
    drop(engine);
    std::fs::remove_dir_all(store.path().join("search")).unwrap();
    let report = Engine::repair_index(store.path(), &Control::unbounded()).unwrap();
    assert!(report.repaired);
    assert!(
        report.quarantined_to.is_none(),
        "nothing existed to quarantine"
    );
    assert!(quarantine_dirs(store.path()).is_empty());
    let engine = Engine::open_existing(store.path()).unwrap();
    assert_eq!(engine.search("recoverable", 5).unwrap().hits.len(), 1);
}

#[test]
fn upgrade_preserves_records_and_is_atomic() {
    let fixture = tempfile::tempdir().unwrap();
    let v1 = fixture.path().join("v1store");
    craft_v1_store(&v1, Some(&fixture.path().join("ws")));
    // New binaries refuse v1 on ordinary open, without upgrading.
    let err = Engine::open_existing(&v1).unwrap_err();
    assert_eq!(code(&err), "upgrade_required");
    // A failure with the upgrade transaction written but uncommitted leaves a
    // wholly readable v1 store, row for row.
    let before = testkit::snapshot(&v1);
    fault::arm(
        names::UPGRADE_BEFORE_COMMIT,
        0,
        Action::Fail("injected".into()),
    );
    Engine::upgrade_store(&v1, 2, &Control::unbounded()).unwrap_err();
    fault::disarm_all();
    assert_eq!(schema_marker(&v1), "1");
    assert_eq!(testkit::snapshot(&v1), before);
    // Unknown targets are refused.
    let err = Engine::upgrade_store(&v1, 3, &Control::unbounded()).unwrap_err();
    assert_eq!(code(&err), "unsupported_mode");
    Engine::upgrade_store(&v1, 2, &Control::unbounded()).unwrap();
    assert_eq!(schema_marker(&v1), "2");
    let mut engine = Engine::open_existing(&v1).unwrap();
    let status = engine.status().unwrap();
    assert_eq!(status.schema, 2);
    assert_eq!(status.source_revision, 0);
    assert_eq!(status.source_count, 1);
    assert_eq!(status.scan_state, "never");
    assert_eq!(engine.training_examples().unwrap().len(), 1);
    assert_eq!(
        engine.source("kept.rs").unwrap().unwrap().bytes,
        KEPT_BODY.len()
    );
    // Upgraded store still indexes and searches.
    let ws = fixture.path().join("ws");
    std::fs::write(ws.join("kept.rs"), KEPT_BODY).unwrap();
    let report = engine.index(&ws, &Control::unbounded()).unwrap();
    assert!(!report.partial);
    assert_eq!(report.changed, 0);
    assert_eq!(report.unchanged, 1);
    assert_eq!(engine.search("kept", 5).unwrap().hits.len(), 1);
}

#[test]
fn unbound_store_names_workspace_unbound_for_queries() {
    let fixture = tempfile::tempdir().unwrap();
    let v1 = fixture.path().join("v1store");
    craft_v1_store(&v1, None);
    Engine::upgrade_store(&v1, 2, &Control::unbounded()).unwrap();
    let engine = Engine::open_existing(&v1).unwrap();
    let status = engine.status().unwrap();
    assert!(status.workspace_id.is_none());
    let err = engine.search("anything", 5).unwrap_err();
    assert_eq!(code(&err), "workspace_unbound");
    let handle = SourceHandle {
        v: 1,
        workspace_id: digest(b"some root"),
        path: "kept.rs".into(),
        sha256: digest(KEPT_BODY.as_bytes()),
        start: 0,
        end: 5,
    };
    let err = engine.retrieve(&handle.to_json(), 2048).unwrap_err();
    assert_eq!(code(&err), "workspace_unbound");
}

#[test]
fn wrong_root_refuses_before_any_mutation() {
    let fixture = tempfile::tempdir().unwrap();
    let root_a = fixture.path().join("a");
    let root_b = fixture.path().join("b");
    std::fs::create_dir_all(&root_a).unwrap();
    std::fs::create_dir_all(&root_b).unwrap();
    std::fs::write(root_a.join("a.rs"), "alpha unique_marker_a\n").unwrap();
    std::fs::write(root_b.join("b.rs"), "beta unique_marker_b\n").unwrap();
    let store_a = fixture.path().join("store-a");
    let store_b = fixture.path().join("store-b");
    let mut engine_a = Engine::initialize(&store_a, &root_a).unwrap();
    engine_a.index(&root_a, &Control::unbounded()).unwrap();
    let revision = engine_a.source_revision().unwrap();
    // Indexing root B into store A refuses before any source/revision change.
    let err = engine_a.index(&root_b, &Control::unbounded()).unwrap_err();
    assert_eq!(code(&err), "wrong_workspace");
    assert_eq!(engine_a.source_revision().unwrap(), revision);
    assert_eq!(engine_a.pending().unwrap(), 0);
    assert!(engine_a.source("b.rs").unwrap().is_none());
    // Store B indexes its own root fine and stays independent.
    let mut engine_b = Engine::initialize(&store_b, &root_b).unwrap();
    engine_b.index(&root_b, &Control::unbounded()).unwrap();
    assert_eq!(engine_b.search("unique_marker_b", 5).unwrap().hits.len(), 1);
    assert!(
        engine_b
            .search("unique_marker_a", 5)
            .unwrap()
            .hits
            .is_empty()
    );
    // Deleting/reconciling A leaves B intact.
    std::fs::remove_file(root_a.join("a.rs")).unwrap();
    engine_a.index(&root_a, &Control::unbounded()).unwrap();
    assert!(engine_a.source("a.rs").unwrap().is_none());
    assert_eq!(
        engine_b.source("b.rs").unwrap().unwrap().bytes,
        "beta unique_marker_b\n".len()
    );
    // A foreign handle is rejected after field validation.
    let handle_b = engine_b
        .search("unique_marker_b", 5)
        .unwrap()
        .hits
        .remove(0)
        .handle;
    let err = engine_a.retrieve(&handle_b.to_json(), 2048).unwrap_err();
    assert_eq!(code(&err), "wrong_workspace");
}

#[test]
fn handles_retrieve_exact_bytes_with_continuations_and_precedence() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    // CRLF, no final newline, unicode and an empty file.
    engine
        .replace_source("crlf.txt", "line one\r\nline two\r\n")
        .unwrap();
    engine
        .replace_source("tail.txt", "no final newline 東京")
        .unwrap();
    engine.replace_source("empty.txt", "").unwrap();
    engine
        .replace_source(
            "big.rs",
            &"pub fn parse_record(input: &str) -> Option<Record> {\n".repeat(60),
        )
        .unwrap();
    drain(&mut engine);
    let workspace = engine.workspace_id().unwrap();

    // Search handles round-trip to exact stored bytes.
    let hit = engine.search("parse_record", 10).unwrap().hits.remove(0);
    let outcome = engine.retrieve(&hit.handle.to_json(), 32768).unwrap();
    assert_eq!(outcome.span, hit.text.as_bytes());
    assert_eq!(outcome.freshness.workspace_id, workspace);

    // Edit invalidates old handles; new handles return the new bytes.
    engine
        .replace_source("big.rs", &"pub fn parse_record(v: u8) -> u8 {\n".repeat(60))
        .unwrap();
    let err = engine.retrieve(&hit.handle.to_json(), 32768).unwrap_err();
    assert_eq!(code(&err), "stale_handle");
    drain(&mut engine);
    let new_hit = engine.search("parse_record", 10).unwrap().hits.remove(0);
    let outcome = engine.retrieve(&new_hit.handle.to_json(), 32768).unwrap();
    assert!(
        std::str::from_utf8(&outcome.span)
            .unwrap()
            .contains("v: u8")
    );
    // Delete makes the handle not_found.
    engine.delete_source("big.rs").unwrap();
    let err = engine
        .retrieve(&new_hit.handle.to_json(), 32768)
        .unwrap_err();
    assert_eq!(code(&err), "not_found");

    // CRLF and source bytes are preserved exactly (the hit is the whole small file).
    let crlf = engine.search("line one", 5).unwrap().hits.remove(0);
    let outcome = engine.retrieve(&crlf.handle.to_json(), 32768).unwrap();
    assert_eq!(outcome.span, b"line one\r\nline two\r\n");
    // Empty file: [0,0) is a valid range returning an empty span.
    let empty = SourceHandle {
        v: 1,
        workspace_id: workspace.clone(),
        path: "empty.txt".into(),
        sha256: digest("".as_bytes()),
        start: 0,
        end: 0,
    };
    let outcome = engine.retrieve(&empty.to_json(), 2048).unwrap();
    assert!(outcome.span.is_empty());
    // Range validation: outside the source and mid-codepoint offsets.
    let tail = engine.search("final", 5).unwrap().hits.remove(0);
    let mut beyond = tail.handle.clone();
    beyond.end = 4096;
    let err = engine.retrieve(&beyond.to_json(), 2048).unwrap_err();
    assert_eq!(code(&err), "invalid_range");
    let mut mid_utf8 = tail.handle.clone();
    // Byte 18 falls inside the three-byte 東 character in the source.
    mid_utf8.start = 18;
    mid_utf8.end = 25;
    let err = engine.retrieve(&mid_utf8.to_json(), 2048).unwrap_err();
    assert_eq!(code(&err), "invalid_range");
    // Field validation precedes workspace/existence checks.
    for raw in [
        "not json",
        "{\"v\":1}",
        "{\"v\":2,\"workspace_id\":\"a\",\"path\":\"x\",\"sha256\":\"y\",\"start\":0,\"end\":1}",
        "{\"v\":1,\"workspace_id\":\"a\",\"path\":\"../escape\",\"sha256\":\"y\",\"start\":0,\"end\":1}",
    ] {
        let err = engine.retrieve(raw, 2048).unwrap_err();
        assert_eq!(code(&err), "invalid_argument", "input: {raw}");
    }
    let mut uppercase = tail.handle.clone();
    uppercase.sha256 = uppercase.sha256.to_uppercase();
    let err = engine.retrieve(&uppercase.to_json(), 2048).unwrap_err();
    assert_eq!(code(&err), "invalid_argument");
    // Budget failure leaves no output and names a sufficient budget.
    match response::pack_retrieve_cli(&engine.retrieve(&crlf.handle.to_json(), 1).unwrap()) {
        Err(e) => {
            assert_eq!(code(&e), "budget_too_small");
        }
        Ok(packed) => panic!("budget 1 must not fit: {}", packed.tokens),
    }
    // Forced continuation: a budget just above the one-codepoint minimum
    // delivers a prefix; following `next` handles must reconstruct every byte.
    let cont: String = (0..40).map(|i| format!("ライン {i} café\r\n")).collect();
    engine.replace_source("cont.txt", &cont).unwrap();
    drain(&mut engine);
    let whole = engine
        .search("café", 10)
        .unwrap()
        .hits
        .into_iter()
        .find(|h| h.path == "cont.txt")
        .expect("cont.txt hit")
        .handle;
    assert_eq!(whole.end as usize, cont.len());
    let minimum = match response::pack_retrieve_cli(&engine.retrieve(&whole.to_json(), 1).unwrap())
    {
        Err(FoundryError::BudgetTooSmall { minimum_tokens }) => minimum_tokens,
        other => panic!("budget 1 must report budget_too_small, got {other:?}"),
    };
    let budget = minimum + 60;
    let mut handle = whole.clone();
    let mut collected = Vec::new();
    let mut calls = 0;
    loop {
        calls += 1;
        assert!(calls < 500, "continuation must make forward progress");
        let outcome = engine.retrieve(&handle.to_json(), budget).unwrap();
        let packed = response::pack_retrieve_cli(&outcome).unwrap();
        assert!(packed.tokens <= budget);
        let (meta, delivered) = packed.text.split_once("---\n").expect("separator present");
        assert!(!delivered.is_empty(), "every continuation delivers bytes");
        collected.extend_from_slice(delivered.as_bytes());
        let next = meta
            .lines()
            .find_map(|line| line.strip_prefix("next: "))
            .expect("next line");
        if next == "null" {
            break;
        }
        let next = SourceHandle::from_json(next).unwrap();
        assert_eq!(
            next.start,
            handle.start + delivered.len() as u64,
            "next advances exactly to the delivered end"
        );
        assert_eq!(next.end, handle.end);
        handle = next;
    }
    assert_eq!(collected, cont.as_bytes());
    assert!(calls > 1, "the budget must force at least one continuation");
}

#[test]
fn retrieve_prefix_fitting_respects_budgets_and_utf8() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    let body = "東京タワーと parse_record のコード\n".repeat(40);
    engine.replace_source("unicode.rs", &body).unwrap();
    drain(&mut engine);
    let handle = engine
        .search("parse_record", 5)
        .unwrap()
        .hits
        .remove(0)
        .handle;
    for tokens in [1usize, 32, 64, 256, 1024, 32768] {
        let outcome = engine.retrieve(&handle.to_json(), tokens).unwrap();
        match response::pack_retrieve_cli(&outcome) {
            Ok(packed) => {
                assert!(packed.tokens <= tokens, "budget {tokens} over by fitting");
                assert!(packed.text.len() <= response::BYTE_CAP);
                let span = packed.text.split_once("---\n").unwrap().1;
                assert!(!span.is_empty() || handle.start == handle.end);
                assert!(
                    body.contains(span),
                    "delivered bytes must be a source prefix"
                );
            }
            Err(e) => assert_eq!(code(&e), "budget_too_small"),
        }
    }
}

#[test]
fn search_json_carries_handles_and_limit_metadata() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    engine
        .replace_source("mod.rs", "fn parse_record() {}\n")
        .unwrap();
    drain(&mut engine);
    let mut outcome = engine.search("parse_record", 10).unwrap();
    let json = response::search_json(&mut outcome);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["format_version"], 1);
    assert_eq!(value["candidate_limit"], 256);
    assert_eq!(value["hits"][0]["handle"]["path"], "mod.rs");
    assert_eq!(value["hits"][0]["handle"]["v"], 1);
    assert_eq!(value["hits"][0]["text"], "fn parse_record() {}\n");
    assert!(json.len() <= response::BYTE_CAP);
}

#[test]
fn auto_strategy_uses_whole_keywords_not_substrings() {
    assert_eq!(
        response::strategy_for_query("references to parse_record"),
        Strategy::Graph
    );
    assert_eq!(
        response::strategy_for_query("who calls parser"),
        Strategy::Graph
    );
    assert_eq!(
        response::strategy_for_query("show impact of dependency changes"),
        Strategy::Graph
    );
    assert_eq!(
        response::strategy_for_query("find preferences"),
        Strategy::Search
    );
    assert_eq!(
        response::strategy_for_query("find calls_tracker"),
        Strategy::Search
    );
    assert_eq!(
        response::strategy_for_query("where are my PREFERENCES"),
        Strategy::Search
    );
    assert_eq!(
        response::strategy_for_query("callers! of main"),
        Strategy::Graph
    );
}

#[test]
fn context_packs_source_first_budgets_and_metadata() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    engine
        .replace_source(
            "a.rs",
            &"café 東京 source retrieval parse_record\n".repeat(200),
        )
        .unwrap();
    drain(&mut engine);
    for tokens in [1usize, 32, 64, 256, 1024, 32768] {
        let outcome = engine
            .context(
                "source retrieval",
                tokens,
                Strategy::Auto,
                &Control::unbounded(),
            )
            .unwrap();
        match response::pack_context_cli(&outcome) {
            Ok(packed) => {
                assert!(packed.tokens <= tokens);
                assert_eq!(
                    packed.tokens,
                    context_foundry::response::count_tokens(&packed.text)
                );
                for line in [
                    "budget_satisfied: true",
                    "tokenizer: o200k_base",
                    "boundary: cli_stdout",
                ] {
                    assert!(packed.text.contains(line), "missing metadata: {line}");
                }
            }
            Err(e) => assert_eq!(code(&e), "budget_too_small"),
        }
    }
    let outcome = engine
        .context(
            "source retrieval",
            256,
            Strategy::Auto,
            &Control::unbounded(),
        )
        .unwrap();
    let packed = response::pack_context_cli(&outcome).unwrap();
    assert!(packed.omitted > 0);
    assert!(packed.text.contains("omitted_candidates: "));
    // Invalid budgets are named invalid_argument.
    for bad in [0usize, 32769] {
        let err = engine
            .context("query", bad, Strategy::Auto, &Control::unbounded())
            .unwrap_err();
        assert_eq!(code(&err), "invalid_argument");
    }
}

#[test]
fn graph_annotations_never_starve_a_fitting_source_span() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    for path in ["a.rs", "b.rs"] {
        engine
            .replace_source(path, "fn parse_record() { caller marker }\n")
            .unwrap();
    }
    drain(&mut engine);
    let bundle = GraphBundle {
        provider: "fixture".into(),
        revision: "r1".into(),
        edges: vec![Edge {
            from: endpoint("a.rs", "fn parse_record() { caller marker }\n"),
            to: endpoint("b.rs", "fn parse_record() { caller marker }\n"),
            kind: "calls".into(),
            evidence: "manual".into(),
        }],
    };
    engine.import_graph(&bundle).unwrap();
    // Graph keyword query with graph present: source span precedes graph lines.
    let outcome = engine
        .context(
            "references to parse_record",
            1024,
            Strategy::Auto,
            &Control::unbounded(),
        )
        .unwrap();
    assert_eq!(outcome.strategy, Strategy::Graph);
    let packed = response::pack_context_cli(&outcome).unwrap();
    let source_pos = packed
        .text
        .find("fn parse_record")
        .expect("source evidence");
    let graph_pos = packed.text.find("--calls-->").unwrap_or(packed.text.len());
    assert!(
        source_pos < graph_pos,
        "a fitting source span must precede graph annotations"
    );
    // Explicit graph without edges returns source results and a reason.
    let outcome = engine
        .context(
            "usage of parse_record",
            256,
            Strategy::Graph,
            &Control::unbounded(),
        )
        .unwrap();
    assert_eq!(outcome.graph_reason, None);
    engine
        .import_graph(&GraphBundle {
            provider: "fixture".into(),
            revision: "r2".into(),
            edges: vec![],
        })
        .unwrap();
    let outcome = engine
        .context(
            "usage of parse_record",
            256,
            Strategy::Graph,
            &Control::unbounded(),
        )
        .unwrap();
    assert_eq!(outcome.graph_reason, Some("graph_unavailable"));
    assert!(!outcome.candidates.is_empty());
}

#[test]
fn graph_preserves_producers_checks_freshness_and_reports_bounds() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, engine) = setup(&root);
    for p in ["a.rs", "b.rs", "c.rs"] {
        engine.replace_source(p, "source\n").unwrap();
    }
    let bundle = |provider: &str, to: &str| GraphBundle {
        provider: provider.into(),
        revision: "fixture-1".into(),
        edges: vec![Edge {
            from: endpoint("a.rs", "source\n"),
            to: endpoint(to, "source\n"),
            kind: "calls".into(),
            evidence: "manual".into(),
        }],
    };
    engine.import_graph(&bundle("one", "b.rs")).unwrap();
    engine.import_graph(&bundle("two", "c.rs")).unwrap();
    assert_eq!(engine.graph("a.rs", false, 1, 10).unwrap().edges.len(), 2);
    assert!(engine.graph("a.rs", false, 1, 1).unwrap().truncated);
    assert_eq!(engine.graph("b.rs", true, 1, 10).unwrap().edges.len(), 1);
    engine
        .import_graph(&GraphBundle {
            provider: "one".into(),
            revision: "fixture-2".into(),
            edges: vec![],
        })
        .unwrap();
    assert_eq!(engine.graph("a.rs", false, 1, 10).unwrap().edges.len(), 1);
    engine.replace_source("c.rs", "changed\n").unwrap();
    let stale = engine.graph("a.rs", false, 1, 10).unwrap();
    assert!(stale.edges.is_empty());
    assert_eq!(stale.stale_edges, 1);
    assert!(engine.import_graph(&bundle("two", "c.rs")).is_err());
}

#[test]
fn feedback_requires_explicit_training_opt_in_and_keeps_tasks_in_one_split() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, engine) = setup(&root);
    let mut feedback = Feedback {
        task_id: "task-1".into(),
        query: "who calls parse".into(),
        correct_strategy: LayaStrategy::Graph,
        label_source: "operator".into(),
        allow_training: false,
    };
    engine.record_feedback(&feedback).unwrap();
    assert!(engine.training_examples().unwrap().is_empty());
    feedback.allow_training = true;
    engine.record_feedback(&feedback).unwrap();
    feedback.query = "what depends on parse".into();
    engine.record_feedback(&feedback).unwrap();
    let examples = engine.training_examples().unwrap();
    assert_eq!(examples.len(), 2);
    assert_eq!(examples[0]["split"], examples[1]["split"]);
    feedback.allow_training = false;
    engine.record_feedback(&feedback).unwrap();
    assert_eq!(engine.training_examples().unwrap().len(), 1);
    feedback.label_source = "model_prediction".into();
    assert_eq!(
        engine.record_feedback(&feedback).unwrap_err().code(),
        "invalid_argument"
    );
}

#[test]
fn excluded_subtrees_retire_only_after_complete_scan() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(root.join("node_modules")).unwrap();
    std::fs::write(
        root.join("node_modules").join("dep.rs"),
        "vendored parse_record\n",
    )
    .unwrap();
    std::fs::write(root.join("main.rs"), "main parse_record\n").unwrap();
    let store = tempfile::tempdir().unwrap();
    let mut engine = Engine::initialize(store.path(), &root).unwrap();
    engine
        .replace_source("node_modules/dep.rs", "vendored parse_record\n")
        .unwrap();
    drain(&mut engine);
    let report = engine.index(&root, &Control::unbounded()).unwrap();
    assert!(!report.partial);
    assert!(engine.source("node_modules/dep.rs").unwrap().is_none());
    assert!(engine.source("main.rs").unwrap().is_some());
    // A symlinked path is never followed.
    std::os::unix::fs::symlink(fixture.path(), root.join("outside")).unwrap();
    let report = engine.index(&root, &Control::unbounded()).unwrap();
    assert!(!report.partial);
    assert!(engine.source("outside").unwrap().is_none());
}

#[test]
fn refresh_interrupted_between_search_commit_and_pending_clear_is_idempotent() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    engine.replace_source("a.rs", "version one\n").unwrap();
    // Cancel at the named boundary: search documents committed, pending not cleared.
    fault::arm(names::INDEX_AFTER_SEARCH_COMMIT, 0, Action::Cancel);
    let err = engine.refresh(&Control::unbounded()).unwrap_err();
    fault::disarm_all();
    assert_eq!(code(&err), "cancelled");
    assert_eq!(engine.pending().unwrap(), 1, "pending work stays durable");
    // The search commit already happened: the current version is searchable
    // BEFORE any replay, which a pre-commit interruption could not satisfy.
    assert_eq!(engine.search("version", 5).unwrap().hits.len(), 1);
    // Replay drains idempotently and keeps exactly one version searchable.
    drain(&mut engine);
    assert_eq!(engine.pending().unwrap(), 0);
    assert_eq!(engine.search("version", 5).unwrap().hits.len(), 1);
}

#[test]
fn failed_commit_preserves_prior_source_bytes_and_state() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (store, mut engine) = setup(&root);
    engine.replace_source("a.rs", "fn first() {}\n").unwrap();
    drain(&mut engine);
    drop(engine);
    let before = testkit::snapshot(store.path());
    let engine = Engine::open_existing(store.path()).unwrap();
    // Deterministic write failure at the boundary just before commit.
    fault::arm(
        names::SOURCE_BEFORE_COMMIT,
        0,
        Action::Fail("injected disk failure".into()),
    );
    let err = engine
        .replace_source("a.rs", "fn second() {}\n")
        .unwrap_err();
    assert_eq!(code(&err), "internal");
    let err = engine.delete_source("a.rs").unwrap_err();
    assert_eq!(code(&err), "internal");
    fault::disarm_all();
    drop(engine);
    // Not one row changed: bytes, revision, pending and derived bookkeeping.
    assert_eq!(testkit::snapshot(store.path()), before);
}
