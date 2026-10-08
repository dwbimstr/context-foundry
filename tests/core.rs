use context_foundry::fault::{self, Action, names};
use context_foundry::testkit;
use context_foundry::testkit::{
    CORRUPT_INDEX_BYTES, KEPT_BODY, corrupt_search_index, craft_v1_store, parse_v2,
    quarantine_dirs, schema_marker,
};
use context_foundry::{
    Control, Engine, FoundryError, Strategy, digest,
    graph::{Edge, Endpoint, GraphBundle},
    learning::Feedback,
    response::{self, Budget},
    store::{HandleRef, SourceHandle},
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

/// The full identities of a ranked source item.
fn source(item: &context_foundry::store::RankedItem) -> &SourceHandle {
    item.handle.as_ref().expect("a source item")
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
    assert_eq!(engine.status().unwrap().schema, 6);
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
        correct_strategy: Strategy::Search,
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
    let outcome = engine.retrieve(&handle.to_v2(), None, 2048).unwrap();
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
        correct_strategy: Strategy::Search,
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
    Engine::upgrade_store(&v1, 6, &Control::unbounded()).unwrap_err();
    fault::disarm_all();
    assert_eq!(schema_marker(&v1), "1");
    assert_eq!(testkit::snapshot(&v1), before);
    // Only the current version is a target: the previous one is refused.
    let err = Engine::upgrade_store(&v1, 2, &Control::unbounded()).unwrap_err();
    assert_eq!(code(&err), "unsupported_mode");
    let err = Engine::upgrade_store(&v1, 3, &Control::unbounded()).unwrap_err();
    assert_eq!(code(&err), "unsupported_mode");
    Engine::upgrade_store(&v1, 6, &Control::unbounded()).unwrap();
    assert_eq!(schema_marker(&v1), "6");
    let engine = Engine::open_existing(&v1).unwrap();
    let status = engine.status().unwrap();
    assert_eq!(status.schema, 6);
    assert_eq!(status.source_revision, 0);
    assert_eq!(status.source_count, 1);
    assert_eq!(status.scan_state, "never");
    assert_eq!(engine.training_examples().unwrap().len(), 1);
    assert_eq!(
        engine.source("kept.rs").unwrap().unwrap().bytes,
        KEPT_BODY.len()
    );
    // Its derived index predates search schema v2: the open writes nothing
    // and names the explicit repair.
    assert_eq!(status.index_state, "repair_required");
    assert!(
        status
            .index_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("search_schema")),
        "{status:?}"
    );
    drop(engine);
    assert!(
        Engine::repair_index(&v1, &Control::unbounded())
            .unwrap()
            .repaired
    );
    // Upgraded and repaired, the store indexes and searches.
    let mut engine = Engine::open_existing(&v1).unwrap();
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
    Engine::upgrade_store(&v1, 6, &Control::unbounded()).unwrap();
    let engine = Engine::open_existing(&v1).unwrap();
    let status = engine.status().unwrap();
    assert!(status.workspace_id.is_none());
    let err = engine.search("anything", 5).unwrap_err();
    assert_eq!(code(&err), "workspace_unbound");
    let handle = SourceHandle {
        workspace_id: digest(b"some root"),
        path: "kept.rs".into(),
        sha256: digest(KEPT_BODY.as_bytes()),
        start: 0,
        end: 5,
    };
    let err = engine.retrieve(&handle.to_v2(), None, 2048).unwrap_err();
    assert_eq!(code(&err), "workspace_unbound");
}

#[test]
fn wrong_root_refuses_before_any_mutation() {
    let fixture = tempfile::tempdir().unwrap();
    let root_a = fixture.path().join("a");
    let root_b = fixture.path().join("b");
    std::fs::create_dir_all(&root_a).unwrap();
    std::fs::create_dir_all(&root_b).unwrap();
    // Markers share no subtoken of 2 or more characters.
    std::fs::write(root_a.join("a.rs"), "alpha quokka_apricot\n").unwrap();
    std::fs::write(root_b.join("b.rs"), "beta walrus_banana\n").unwrap();
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
    assert_eq!(engine_b.search("walrus_banana", 5).unwrap().hits.len(), 1);
    assert!(
        engine_b
            .search("quokka_apricot", 5)
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
        "beta walrus_banana\n".len()
    );
    // A foreign handle is rejected after field validation.
    let handle_b = engine_b
        .search("walrus_banana", 5)
        .unwrap()
        .hits
        .remove(0)
        .handle;
    let err = engine_a
        .retrieve(&handle_b.to_v2(), None, 2048)
        .unwrap_err();
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
    let outcome = engine.retrieve(&hit.handle.to_v2(), None, 32768).unwrap();
    assert_eq!(outcome.span, hit.text.as_bytes());
    assert_eq!(outcome.freshness.workspace_id, workspace);

    // Edit invalidates old handles; new handles return the new bytes.
    engine
        .replace_source("big.rs", &"pub fn parse_record(v: u8) -> u8 {\n".repeat(60))
        .unwrap();
    let err = engine
        .retrieve(&hit.handle.to_v2(), None, 32768)
        .unwrap_err();
    assert_eq!(code(&err), "stale_handle");
    drain(&mut engine);
    let new_hit = engine.search("parse_record", 10).unwrap().hits.remove(0);
    let outcome = engine
        .retrieve(&new_hit.handle.to_v2(), None, 32768)
        .unwrap();
    assert!(
        std::str::from_utf8(&outcome.span)
            .unwrap()
            .contains("v: u8")
    );
    // Delete makes the handle not_found.
    engine.delete_source("big.rs").unwrap();
    let err = engine
        .retrieve(&new_hit.handle.to_v2(), None, 32768)
        .unwrap_err();
    assert_eq!(code(&err), "not_found");

    // CRLF and source bytes are preserved exactly (the hit is the whole small file).
    let crlf = engine.search("line one", 5).unwrap().hits.remove(0);
    let outcome = engine.retrieve(&crlf.handle.to_v2(), None, 32768).unwrap();
    assert_eq!(outcome.span, b"line one\r\nline two\r\n");
    // Empty file: [0,0) is a valid range returning an empty span.
    let empty = SourceHandle {
        workspace_id: workspace.clone(),
        path: "empty.txt".into(),
        sha256: digest("".as_bytes()),
        start: 0,
        end: 0,
    };
    let outcome = engine.retrieve(&empty.to_v2(), None, 2048).unwrap();
    assert!(outcome.span.is_empty());
    // Range validation: outside the source and mid-codepoint offsets.
    let tail = engine.search("final", 5).unwrap().hits.remove(0);
    let mut beyond = tail.handle.clone();
    beyond.end = 4096;
    let err = engine.retrieve(&beyond.to_v2(), None, 2048).unwrap_err();
    assert_eq!(code(&err), "invalid_range");
    let mut mid_utf8 = tail.handle.clone();
    // Byte 18 falls inside the three-byte 東 character in the source.
    mid_utf8.start = 18;
    mid_utf8.end = 25;
    let err = engine.retrieve(&mid_utf8.to_v2(), None, 2048).unwrap_err();
    assert_eq!(code(&err), "invalid_range");
    // Field validation precedes workspace/existence checks.
    for raw in [
        "not a handle",
        "{\"v\":1}",
        "{\"v\":2,\"workspace_id\":\"a\",\"path\":\"x\",\"sha256\":\"y\",\"start\":0,\"end\":1}",
        "{\"v\":1,\"workspace_id\":\"a\",\"path\":\"../escape\",\"sha256\":\"y\",\"start\":0,\"end\":1}",
    ] {
        let err = engine.retrieve(raw, None, 2048).unwrap_err();
        assert_eq!(code(&err), "invalid_argument", "input: {raw}");
    }
    let mut uppercase = tail.handle.clone();
    uppercase.sha256 = uppercase
        .sha256
        .to_uppercase()
        .replace(char::is_numeric, "A");
    let err = engine.retrieve(&uppercase.to_v2(), None, 2048).unwrap_err();
    assert_eq!(code(&err), "invalid_argument");
    // Budget failure leaves no output and names a sufficient budget.
    let tiny = engine.retrieve(&crlf.handle.to_v2(), None, 1).unwrap();
    match response::pack_retrieve(&tiny, Budget::request(1), &response::stdout_bytes) {
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
    let tiny = engine.retrieve(&whole.to_v2(), None, 1).unwrap();
    let minimum = match response::pack_retrieve(&tiny, Budget::request(1), &response::stdout_bytes)
    {
        Err(FoundryError::BudgetTooSmall { minimum_tokens }) => minimum_tokens,
        other => panic!("budget 1 must report budget_too_small, got {other:?}"),
    };
    let budget = minimum + 60;
    let mut handle = whole.to_v2();
    let mut collected = Vec::new();
    let mut calls = 0;
    loop {
        calls += 1;
        assert!(calls < 500, "continuation must make forward progress");
        let outcome = engine.retrieve(&handle, None, budget).unwrap();
        let packed =
            response::pack_retrieve(&outcome, Budget::request(budget), &response::stdout_bytes)
                .unwrap();
        assert!(packed.tokens <= budget);
        let parsed = parse_v2(&packed.text).unwrap();
        let delivered = &parsed.items[0].body;
        assert!(!delivered.is_empty(), "every continuation delivers bytes");
        collected.extend_from_slice(delivered.as_bytes());
        let Some(next) = parsed.next else { break };
        let (current, next_range) = (
            HandleRef::parse(&handle).unwrap(),
            HandleRef::parse(&next).unwrap(),
        );
        assert_eq!(
            next_range.start,
            current.start + delivered.len() as u64,
            "next advances exactly to the delivered end"
        );
        assert_eq!(next_range.end, current.end);
        handle = next;
    }
    assert_eq!(collected, cont.as_bytes());
    assert!(calls > 1, "the budget must force at least one continuation");
}

/// context-v2 § Source handles: `<path>#<start>-<end>@<sha32>.<ws16>` parses from
/// the end, so any admitted path (including `#`, `@`, `.` and JSON-special bytes)
/// round-trips; malformed input is `invalid_argument` before any store check.
#[test]
fn v2_handle_parse_and_format_round_trip_with_stage_one_refusals() {
    const HANDLE_GRAMMAR: &str = "handle must be a v2 string `path#start-end@sha32.ws16`";
    let ws = digest(b"workspace");
    let sha = digest(b"source");
    let component = "a#1-2@b.c \"\\{}".repeat(16);
    let mut path = vec![component.clone(); 16].join("/");
    path.push('/');
    path.push_str(&"q".repeat(4096 - path.len()));
    assert_eq!(path.len(), 4096);
    let full = SourceHandle {
        workspace_id: ws.clone(),
        path: path.clone(),
        sha256: sha.clone(),
        start: u64::MAX,
        end: u64::MAX,
    };
    let text = full.to_v2();
    assert_eq!(
        text,
        format!(
            "{path}#18446744073709551615-18446744073709551615@{}.{}",
            &sha[..32],
            &ws[..16]
        )
    );
    assert_eq!(text.len(), 4096 + 92, "the longest suffix is 92 bytes");
    assert!(text.len() <= 4200, "fits the 4200-byte input cap");
    let parsed = HandleRef::parse(&text).unwrap();
    assert_eq!(parsed.path, path);
    assert_eq!((parsed.start, parsed.end), (u64::MAX, u64::MAX));
    assert_eq!(
        (parsed.sha32.as_str(), parsed.ws16.as_str()),
        (&sha[..32], &ws[..16])
    );
    assert_eq!(parsed.to_string(), text);
    // A path that starts with `{` is an admitted path, not a v1 object.
    let braced = format!("{{a}}.rs#0-1@{}.{}", &sha[..32], &ws[..16]);
    assert_eq!(HandleRef::parse(&braced).unwrap().path, "{a}.rs");
    // A v1 handle object fails the end-anchored grammar with the same
    // message, short or over the 4200-byte cap.
    let v1_object = |path: &str| {
        serde_json::json!({"v": 1, "workspace_id": ws, "path": path, "sha256": sha,
                           "start": 0, "end": 1})
        .to_string()
    };
    let err = HandleRef::parse(&v1_object("a.rs")).unwrap_err();
    assert!(err.to_string().contains(HANDLE_GRAMMAR), "{err}");

    let suffix = format!("@{}.{}", &sha[..32], &ws[..16]);
    let err = HandleRef::parse(&v1_object(&path)).unwrap_err();
    assert_eq!(code(&err), "invalid_argument");
    assert!(err.to_string().contains(HANDLE_GRAMMAR), "{err}");
    for bad in [
        format!("a.rs#0-1@{}.{}", sha[..32].to_uppercase(), &ws[..16]),
        format!("a.rs#0-1@{}.{}", &sha[..32], ws[..16].to_uppercase()),
        format!("a.rs#0-1@{}.{}", &sha[..31], &ws[..16]),
        format!("a.rs#0-1@{}.{}", &sha[..32], &ws[..15]),
        format!("a.rs#01-2{suffix}"),
        format!("a.rs#0-02{suffix}"),
        format!("a.rs#-1{suffix}"),
        format!("a.rs#0-{suffix}"),
        format!("a.rs#+1-2{suffix}"),
        format!("a.rs0-1{suffix}"),
        format!("a.rs#0-18446744073709551616{suffix}"),
        format!("a.rs#5-4{suffix}"),
        format!("#0-1{suffix}"),
        format!("a/../b.rs#0-1{suffix}"),
        format!("/a.rs#0-1{suffix}"),
        format!("a\nb.rs#0-1{suffix}"),
        format!("a.rs#0-1{suffix} "),
        format!("{}#0-1{suffix}", "p".repeat(4097)),
        String::new(),
    ] {
        let err = HandleRef::parse(&bad).unwrap_err();
        assert_eq!(code(&err), "invalid_argument", "input: {bad:?}");
    }
    // The 4200-byte cap is checked before parsing.
    let err = HandleRef::parse(&"x".repeat(4201)).unwrap_err();
    assert_eq!(code(&err), "invalid_argument");
}

/// context-v2 validation order for `retrieve`: syntax → workspace → existence →
/// digest prefix → range, each against one authoritative read, no partial evidence.
#[test]
fn v2_retrieve_validates_in_contract_order_against_stored_identities() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    engine
        .replace_source("crlf.txt", "line one\r\nline two\r\n")
        .unwrap();
    engine
        .replace_source("tail.txt", "no final newline 東京")
        .unwrap();
    engine.replace_source("empty.txt", "").unwrap();
    engine
        .replace_source("parse.rs", "pub fn parse_record() {}\n")
        .unwrap();
    drain(&mut engine);
    let workspace = engine.workspace_id().unwrap();
    let handle = |path: &str, body: &str, start: u64, end: u64| SourceHandle {
        workspace_id: workspace.clone(),
        path: path.into(),
        sha256: digest(body.as_bytes()),
        start,
        end,
    };

    // A search hit's v2 rendering reads the same bytes and recovers full identities.
    let hit = engine.search("parse_record", 5).unwrap().hits.remove(0);
    let outcome = engine.retrieve(&hit.handle.to_v2(), None, 2048).unwrap();
    assert_eq!(outcome.span, hit.text.as_bytes());
    assert_eq!(outcome.requested.sha256, hit.handle.sha256);
    assert_eq!(outcome.requested.workspace_id, workspace);
    assert_eq!(outcome.requested.to_v2(), hit.handle.to_v2());
    let crlf = handle("crlf.txt", "line one\r\nline two\r\n", 0, 20);
    assert_eq!(
        engine.retrieve(&crlf.to_v2(), None, 2048).unwrap().span,
        b"line one\r\nline two\r\n"
    );
    let empty = handle("empty.txt", "", 0, 0);
    assert!(
        engine
            .retrieve(&empty.to_v2(), None, 2048)
            .unwrap()
            .span
            .is_empty()
    );

    let tail = "no final newline 東京";
    let foreign = |h: &SourceHandle| {
        let mut h = h.clone();
        h.workspace_id = digest(b"another workspace");
        assert_ne!(h.workspace_id[..16], workspace[..16]);
        h
    };
    let cases: Vec<(&str, String, &str)> = vec![
        // Stage 1 wins over every later stage.
        (
            "syntax over workspace",
            format!("{}X", foreign(&crlf).to_v2()),
            "invalid_argument",
        ),
        (
            "inverted range",
            handle("crlf.txt", "x", 5, 4).to_v2(),
            "invalid_argument",
        ),
        ("budget", crlf.to_v2(), "invalid_argument"),
        // Stage 2: workspace prefix, before existence.
        (
            "foreign workspace",
            foreign(&crlf).to_v2(),
            "wrong_workspace",
        ),
        (
            "foreign and missing",
            foreign(&handle("gone.rs", "x", 0, 1)).to_v2(),
            "wrong_workspace",
        ),
        // Stage 3: existence, before digest.
        ("missing", handle("gone.rs", "x", 0, 1).to_v2(), "not_found"),
        // Stage 4: digest prefix, before range.
        (
            "stale",
            handle("crlf.txt", "old bytes", 0, 1).to_v2(),
            "stale_handle",
        ),
        (
            "stale and out of range",
            handle("crlf.txt", "old", 0, 999).to_v2(),
            "stale_handle",
        ),
        // Stage 5: range against the source.
        (
            "beyond",
            handle("crlf.txt", "line one\r\nline two\r\n", 0, 21).to_v2(),
            "invalid_range",
        ),
        (
            "mid codepoint",
            handle("tail.txt", tail, 18, 25).to_v2(),
            "invalid_range",
        ),
        (
            "empty range on nonempty",
            handle("tail.txt", tail, 0, 0).to_v2(),
            "invalid_range",
        ),
        (
            "start at end",
            handle("tail.txt", tail, 23, 23).to_v2(),
            "invalid_range",
        ),
    ];
    for (label, raw, expected) in cases {
        let tokens = if label == "budget" { 0 } else { 2048 };
        let err = engine.retrieve(&raw, None, tokens).unwrap_err();
        assert_eq!(code(&err), expected, "{label}: {raw}");
    }

    // Edited source: the old v2 handle is stale; deleted source: not_found.
    let before = hit.handle.to_v2();
    engine
        .replace_source("parse.rs", "pub fn parse_record(v: u8) {}\n")
        .unwrap();
    assert_eq!(
        code(&engine.retrieve(&before, None, 2048).unwrap_err()),
        "stale_handle"
    );
    engine.delete_source("parse.rs").unwrap();
    assert_eq!(
        code(&engine.retrieve(&before, None, 2048).unwrap_err()),
        "not_found"
    );
}

/// context-v2 `lines`: `"A"` or `"A-B"`, absolute 1-based LF-delimited file lines
/// (CR stays with its line, an unterminated last line ends at EOF, a trailing LF
/// opens no line). The selected whole lines intersect the handle's range and
/// never widen it; the returned handle names the intersection.
#[test]
fn v2_retrieve_lines_intersects_absolute_file_lines_with_the_handle() {
    const RECORDS: &str = include_str!("fixtures/agent-task/src/records.rs");
    const UNITS: &str = "fn a() {}\n    fn b() { x }\nfn c() {}\n";
    let sources: [(&str, &str); 7] = [
        ("records.rs", RECORDS),
        ("units.rs", UNITS),
        ("crlf.txt", "a\r\nb\r\nc\r\n"),
        ("open.txt", "x\ny"),
        ("closed.txt", "x\ny\n"),
        ("wide.txt", "東京\n大阪\n"),
        ("empty.txt", ""),
    ];
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, engine) = setup(&root);
    for (path, body) in sources {
        engine.replace_source(path, body).unwrap();
    }
    let workspace = engine.workspace_id().unwrap();
    let body_of = |path: &str| sources.iter().find(|(p, _)| *p == path).unwrap().1;
    let handle = |path: &str, start: usize, end: usize| SourceHandle {
        workspace_id: workspace.clone(),
        path: path.into(),
        sha256: digest(body_of(path).as_bytes()),
        start: start as u64,
        end: end as u64,
    };
    let whole = |path: &str| handle(path, 0, body_of(path).len());
    // Delivered bytes; the outcome's handle must name exactly those bytes,
    // stay inside the requested handle and read them back without `lines`.
    let select = |h: &SourceHandle, lines: &str| -> String {
        let out = engine
            .retrieve(&h.to_v2(), Some(lines), 2048)
            .unwrap_or_else(|e| panic!("{}#{lines}: {e}", h.path));
        let (start, end) = (out.requested.start, out.requested.end);
        assert!(h.start <= start && end <= h.end, "{lines}: never widened");
        assert_eq!(
            out.span,
            &body_of(&h.path).as_bytes()[start as usize..end as usize]
        );
        assert_eq!(out.requested.sha256, h.sha256);
        let again = engine.retrieve(&out.requested.to_v2(), None, 2048).unwrap();
        assert_eq!(again.span, out.span, "{lines}: returned handle reads back");
        String::from_utf8(out.span).unwrap()
    };
    let refused = |h: &SourceHandle, lines: &str| {
        let err = engine.retrieve(&h.to_v2(), Some(lines), 2048).unwrap_err();
        code(&err).to_owned()
    };

    // The 001 T004 clipping example: parse_record is lines 5-7 after a
    // three-line comment and a blank line. A handle covering exactly 5-7 with
    // lines 4-6 returns lines 5-6; the whole file returns lines 4-6.
    let five = RECORDS.find("pub fn parse_record").unwrap();
    let seven = RECORDS.rfind("}\n").unwrap();
    let unit = handle("records.rs", five, RECORDS.len());
    assert_eq!(select(&unit, "4-6"), &RECORDS[five..seven]);
    assert_eq!(
        select(&whole("records.rs"), "4-6"),
        &RECORDS[five - 1..seven]
    );
    assert_eq!(select(&unit, "7"), "}\n");

    // Mid-line unit boundaries: the handle starts inside line 2 and ends inside line 3.
    let line3 = UNITS.find("fn c").unwrap();
    let (b, c_end) = (UNITS.find("fn b").unwrap(), line3 + "fn c()".len());
    let mid = handle("units.rs", b, c_end);
    assert_eq!(select(&mid, "2"), "fn b() { x }\n");
    assert_eq!(select(&mid, "1-3"), &UNITS[b..c_end]);
    assert_eq!(select(&mid, "3"), "fn c()");
    assert_eq!(select(&mid, "3-9"), "fn c()");
    assert_eq!(refused(&mid, "1"), "invalid_range");
    // A continuation handle starts mid-line at the delivered end: lines never
    // rewind it to the start of its line.
    let resume = UNITS.find("{ x").unwrap();
    let continuation = handle("units.rs", resume, UNITS.len());
    assert_eq!(select(&continuation, "2"), "{ x }\n");
    assert_eq!(select(&continuation, "2-3"), &UNITS[resume..]);

    // CRLF: CR belongs to its line; a handle ending between CR and LF is not widened.
    assert_eq!(select(&whole("crlf.txt"), "2"), "b\r\n");
    assert_eq!(select(&whole("crlf.txt"), "2-3"), "b\r\nc\r\n");
    assert_eq!(select(&handle("crlf.txt", 0, 5), "2"), "b\r");
    // Unterminated last line ends at EOF; lines past the end select nothing.
    assert_eq!(select(&whole("open.txt"), "2"), "y");
    assert_eq!(select(&whole("open.txt"), "2-5"), "y");
    assert_eq!(select(&whole("open.txt"), "1-2"), "x\ny");
    assert_eq!(refused(&whole("open.txt"), "3"), "invalid_range");
    // A trailing LF creates no extra line.
    assert_eq!(select(&whole("closed.txt"), "2"), "y\n");
    assert_eq!(select(&whole("closed.txt"), "1-9"), "x\ny\n");
    assert_eq!(refused(&whole("closed.txt"), "3"), "invalid_range");
    assert_eq!(select(&whole("wide.txt"), "2"), "大阪\n");
    // An empty file has no lines; A > B and an empty intersection are range errors.
    assert_eq!(refused(&handle("empty.txt", 0, 0), "1"), "invalid_range");
    assert_eq!(refused(&whole("closed.txt"), "2-1"), "invalid_range");
    assert_eq!(refused(&handle("closed.txt", 0, 2), "2"), "invalid_range");

    for bad in [
        "",
        "0",
        "0-2",
        "1-0",
        "01",
        "1-02",
        "1-",
        "-2",
        "a",
        "1-2-3",
        " 1",
        "1 ",
        "+1",
        "1.5",
        "18446744073709551616",
    ] {
        assert_eq!(
            refused(&whole("closed.txt"), bad),
            "invalid_argument",
            "{bad:?}"
        );
    }

    // Order: malformed lines are syntax (stage 1); A > B is a range error, so
    // workspace, existence and digest failures come first.
    let mut foreign = whole("closed.txt");
    foreign.workspace_id = digest(b"elsewhere");
    assert_eq!(refused(&foreign, "x"), "invalid_argument");
    assert_eq!(refused(&foreign, "2-1"), "wrong_workspace");
    let missing = SourceHandle {
        path: "gone.rs".into(),
        ..whole("closed.txt")
    };
    assert_eq!(refused(&missing, "2-1"), "not_found");
    let mut stale = whole("closed.txt");
    stale.sha256 = digest(b"old bytes");
    assert_eq!(refused(&stale, "2-1"), "stale_handle");
    assert_eq!(refused(&handle("closed.txt", 0, 99), "1"), "invalid_range");
}

/// A `lines` selection with nothing inside the handle names the lines the
/// handle covers: an agent that copied `#start-end` as line numbers must not
/// read a byte-range error. Genuine byte-range errors keep their own.
#[test]
fn an_empty_line_selection_names_the_lines_its_handle_covers() {
    const BODY: &str = "one\ntwo\nthree\nfour\nfive\nsix\n";
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, engine) = setup(&root);
    engine.replace_source("six.txt", BODY).unwrap();
    engine.replace_source("empty.txt", "").unwrap();
    let workspace = engine.workspace_id().unwrap();
    let handle = |path: &str, body: &str, start: usize, end: usize| {
        SourceHandle {
            workspace_id: workspace.clone(),
            path: path.into(),
            sha256: digest(body.as_bytes()),
            start: start as u64,
            end: end as u64,
        }
        .to_v2()
    };
    let (three, five) = (BODY.find("three").unwrap(), BODY.find("five").unwrap());
    // Lines 3-4 as a byte range: lines past it, past the file or reversed.
    for (raw, lines, covers) in [
        (handle("six.txt", BODY, three, five), "5-6", "lines 3-4"),
        (handle("six.txt", BODY, three, five), "9", "lines 3-4"),
        (handle("six.txt", BODY, three, five), "4-3", "lines 3-4"),
        (handle("six.txt", BODY, five, five + 3), "1", "line 5"),
        (handle("empty.txt", "", 0, 0), "1", "no lines"),
    ] {
        let err = engine.retrieve(&raw, Some(lines), 2048).unwrap_err();
        assert_eq!(
            (code(&err), err.exit_code(), err.retryable()),
            ("invalid_range", 2, false),
            "{raw} lines {lines}"
        );
        let message = err.to_string();
        assert!(
            message.contains(&format!("covers {covers};")),
            "{raw} lines {lines}: {message}"
        );
        assert!(
            message.contains("#start-end is a byte range"),
            "{raw} lines {lines}: {message}"
        );
    }
    // A byte range outside the source is still reported as one.
    let err = engine
        .retrieve(&handle("six.txt", BODY, 0, 99), None, 2048)
        .unwrap_err();
    assert_eq!(code(&err), "invalid_range");
    assert!(!err.to_string().contains("covers"), "{err}");
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
        let outcome = engine.retrieve(&handle.to_v2(), None, tokens).unwrap();
        match response::pack_retrieve(&outcome, Budget::request(tokens), &response::stdout_bytes) {
            Ok(packed) => {
                assert!(packed.tokens <= tokens, "budget {tokens} over by fitting");
                assert!(packed.text.len() <= response::BYTE_CAP);
                let parsed = parse_v2(&packed.text).unwrap();
                let span = &parsed.items[0].body;
                assert!(!span.is_empty() || handle.start == handle.end);
                assert!(
                    body.starts_with(span.as_str()),
                    "delivered bytes must be a source prefix"
                );
            }
            Err(e) => assert_eq!(code(&e), "budget_too_small"),
        }
    }
}

#[test]
fn search_text_carries_v2_handles_and_locators() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    engine
        .replace_source("mod.rs", "fn parse_record() {}\n")
        .unwrap();
    drain(&mut engine);
    let outcome = engine.search("parse_record", 10).unwrap();
    let packed =
        response::pack_search(&outcome, Budget::request(1024), &response::stdout_bytes).unwrap();
    let parsed = parse_v2(&packed.text).unwrap();
    assert_eq!(parsed.header[0], "foundry search");
    let hit = &parsed.items[0];
    assert_eq!(HandleRef::parse(&hit.handle).unwrap().path, "mod.rs");
    assert_eq!(hit.lines.as_deref(), Some("L1"));
    assert_eq!(hit.label.as_deref(), Some("fn parse_record"));
    assert_eq!(hit.body, "fn parse_record() {}");
    assert!(packed.text.len() <= response::BYTE_CAP);
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
            .context_candidates("source retrieval", Strategy::Auto, &Control::unbounded())
            .unwrap();
        match response::pack_context(&outcome, Budget::request(tokens), &response::stdout_bytes) {
            Ok(packed) => {
                assert!(packed.tokens <= tokens);
                assert_eq!(
                    packed.tokens,
                    context_foundry::response::count_tokens(&packed.text)
                );
                let parsed = parse_v2(&packed.text).unwrap();
                assert_eq!(parsed.header[0], "foundry context");
                assert!(parsed.header.contains(&format!("budget:{tokens}")));
            }
            Err(e) => assert_eq!(code(&e), "budget_too_small"),
        }
    }
    let outcome = engine
        .context_candidates("source retrieval", Strategy::Auto, &Control::unbounded())
        .unwrap();
    let packed =
        response::pack_context(&outcome, Budget::request(256), &response::stdout_bytes).unwrap();
    assert!(packed.omitted > 0);
    assert!(
        packed
            .text
            .lines()
            .next()
            .unwrap()
            .contains(&format!(" · omitted:{}", packed.omitted))
    );
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
        .context_candidates(
            "references to parse_record",
            Strategy::Auto,
            &Control::unbounded(),
        )
        .unwrap();
    assert_eq!(outcome.counters.graph, Some("ok"), "auto resolved to graph");
    let packed =
        response::pack_context(&outcome, Budget::request(1024), &response::stdout_bytes).unwrap();
    let source_pos = packed
        .text
        .find("fn parse_record")
        .expect("source evidence");
    let graph_pos = packed.text.find("\nedge ").unwrap_or(packed.text.len());
    assert!(
        source_pos < graph_pos,
        "a fitting source span must precede graph annotations"
    );
    // Explicit graph without edges returns source results and a reason.
    let outcome = engine
        .context_candidates(
            "usage of parse_record",
            Strategy::Graph,
            &Control::unbounded(),
        )
        .unwrap();
    assert_eq!(outcome.counters.graph, Some("ok"));
    engine
        .import_graph(&GraphBundle {
            provider: "fixture".into(),
            revision: "r2".into(),
            edges: vec![],
        })
        .unwrap();
    let outcome = engine
        .context_candidates(
            "usage of parse_record",
            Strategy::Graph,
            &Control::unbounded(),
        )
        .unwrap();
    assert_eq!(outcome.counters.graph, Some("graph_unavailable"));
    assert!(!outcome.items.is_empty());
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
        correct_strategy: Strategy::Graph,
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

// ---------------------------------------------------------------------------
// 001 T005: syntax-unit search documents, two-tier ranking, `path` filter.

/// Tier 1: the exact definition is hit #1 over more than 3 call sites and
/// repeated identifiers; the hit is its delivery unit, labelled by kind and
/// qualified name, at the unit's first line.
#[test]
fn search_candidates_rank_the_exact_definition_first() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    for i in 0..4 {
        let caller = format!(
            "fn caller_{i}() {{\n    parse_record(\"a\");\n    parse_record(\"b\");\n    let parse_record = 1;\n    parse_record + parse_record\n}}\n"
        );
        engine
            .replace_source(&format!("calls_{i}.rs"), &caller)
            .unwrap();
    }
    let unit = "pub fn parse_record(input: &str) -> u8 {\n    input.len() as u8\n}";
    engine
        .replace_source("defs.rs", &format!("// helpers\n\n{unit}\n"))
        .unwrap();
    drain(&mut engine);
    let batch = engine
        .search_candidates("parse_record", None, 10, &Control::unbounded())
        .unwrap();
    let first = &batch.items[0];
    assert_eq!((first.tier, source(first).path.as_str()), (1, "defs.rs"));
    assert_eq!((first.label.as_str(), first.line), ("fn parse_record", 3));
    assert_eq!(
        engine
            .retrieve(&source(first).to_v2(), None, 2048)
            .unwrap()
            .span,
        unit.as_bytes(),
        "the handle covers the delivery unit"
    );
    assert!(batch.items.len() > 4, "the call sites follow");
    assert!(batch.items[1..].iter().all(|item| item.tier == 2));
}

/// A parenthesized C declarator is still an exact definition: tier 1 matches
/// the innermost identifier, ahead of the call sites.
#[test]
fn a_parenthesized_c_definition_ranks_as_the_exact_definition() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    engine
        .replace_source(
            "calls.c",
            "int use(void) {\n    return probe() + probe();\n}\n",
        )
        .unwrap();
    engine
        .replace_source("defs.c", "int (probe)(void) { return 1; }\n")
        .unwrap();
    drain(&mut engine);
    let batch = engine
        .search_candidates("probe", None, 10, &Control::unbounded())
        .unwrap();
    let first = &batch.items[0];
    assert_eq!(
        (
            first.tier,
            source(first).path.as_str(),
            source(first).start,
            source(first).end,
            first.label.as_str()
        ),
        (1, "defs.c", 0, 31, "fn probe")
    );
    assert_eq!(source(&batch.items[1]).path, "calls.c");
    assert_eq!(batch.items[1].tier, 2);
}

/// `parseRecord` and `parse_record` are both found by `parse record`: the
/// body is indexed by code subtokens.
#[test]
fn camel_and_snake_identifiers_are_found_by_their_subtokens() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    engine
        .replace_source(
            "a.ts",
            "export function parseRecord(s: string) {\n  return s;\n}\n",
        )
        .unwrap();
    engine
        .replace_source("b.rs", "fn parse_record() {}\n")
        .unwrap();
    engine
        .replace_source("c.txt", "an unrelated note\n")
        .unwrap();
    drain(&mut engine);
    let batch = engine
        .search_candidates("parse record", None, 10, &Control::unbounded())
        .unwrap();
    let mut paths: Vec<&str> = batch
        .items
        .iter()
        .map(|item| source(item).path.as_str())
        .collect();
    paths.sort();
    assert_eq!(paths, ["a.ts", "b.rs"]);
}

/// `path` restricts both tiers to one file or directory subtree; one leading
/// `./` and one trailing `/` are stripped; anything else must satisfy the
/// handle path rules.
#[test]
fn the_path_filter_restricts_both_tiers_to_a_subtree() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    for path in ["src/a/x.rs", "src/b/x.rs", "src/ab.rs"] {
        engine
            .replace_source(path, "fn probe_fn() {}\n\nfn user() { probe_fn(); }\n")
            .unwrap();
    }
    drain(&mut engine);
    let control = Control::unbounded();
    let paths_for = |path: Option<&str>| -> Vec<String> {
        let mut paths: Vec<String> = engine
            .search_candidates("probe_fn", path, 64, &control)
            .unwrap()
            .items
            .into_iter()
            .map(|item| source(&item).path.clone())
            .collect();
        paths.sort();
        paths.dedup();
        paths
    };
    assert_eq!(paths_for(None), ["src/a/x.rs", "src/ab.rs", "src/b/x.rs"]);
    for filter in ["src/a", "./src/a/", "src/a/x.rs"] {
        assert_eq!(paths_for(Some(filter)), ["src/a/x.rs"], "{filter}");
    }
    let tier_one = engine
        .search_candidates("probe_fn", Some("src/b"), 64, &control)
        .unwrap();
    assert_eq!(tier_one.items[0].tier, 1);
    assert_eq!(source(&tier_one.items[0]).path, "src/b/x.rs");
    for bad in ["../x", "/abs", "a//b", ""] {
        let err = engine
            .search_candidates("probe_fn", Some(bad), 64, &control)
            .unwrap_err();
        assert_eq!(code(&err), "invalid_argument", "{bad:?}");
    }
}

/// The contract's `key_hash`, computed independently of the store: the first
/// 8 bytes of SHA-256 of `path\0start`, big-endian.
fn contract_key_hash(path: &str, start: u64) -> u64 {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("{path}\0{start}").as_bytes());
    u64::from_be_bytes(digest[..8].try_into().unwrap())
}

/// Index `sources` in the given order, one refresh (one Tantivy commit) per
/// batch, so segment and document order follow `order`.
fn index_in_batches(engine: &mut Engine, sources: &[(String, &str)], order: &[usize], size: usize) {
    for batch in order.chunks(size) {
        for &i in batch {
            engine.replace_source(&sources[i].0, sources[i].1).unwrap();
        }
        drain(engine);
    }
}

#[test]
fn tier_one_keeps_the_64_definitions_with_the_smallest_key_hash() {
    const DEF: &str = "fn shared_def() {}\n";
    let sources: Vec<(String, &str)> = (0..80).map(|i| (format!("d{i:02}.rs"), DEF)).collect();
    let mut by_hash: Vec<&str> = sources.iter().map(|(path, _)| path.as_str()).collect();
    by_hash.sort_by_key(|path| contract_key_hash(path, 0));
    let mut window = by_hash[..64].to_vec();
    window.sort();
    let by_name: Vec<String> = (0..64).map(|i| format!("d{i:02}.rs")).collect();
    assert_ne!(window, by_name, "the hash cutoff differs from name order");
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    let order: Vec<usize> = (0..80).map(|i| (i * 37 + 11) % 80).collect();
    index_in_batches(&mut engine, &sources, &order, 16);
    let batch = engine
        .search_candidates("shared_def", None, 64, &Control::unbounded())
        .unwrap();
    let got: Vec<(u8, &str, u64, u64)> = batch
        .items
        .iter()
        .map(|item| {
            (
                item.tier,
                source(item).path.as_str(),
                source(item).start,
                source(item).end,
            )
        })
        .collect();
    // `fn shared_def() {}` is bytes 0-18 of each file.
    let want: Vec<(u8, &str, u64, u64)> = window.iter().map(|path| (1, *path, 0, 18)).collect();
    assert_eq!(got, want);
    assert!(batch.counters.candidates_full);
    assert!(
        batch.counters.truncated,
        "16 tier-2 hits survive past the limit"
    );
    assert_eq!(batch.counters.capped, 0);
}

/// The 013 corpus defect: an English word of the query (`find`) has more than
/// 64 definitions and the intended identifier sorts last by path. A
/// backtick-marked run alone feeds tier 1; unmarked runs are ranked by how
/// few definitions they have. Search and context share that ranking.
#[test]
fn tier_one_puts_the_marked_or_most_specific_run_before_a_common_word() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    for i in 0..70 {
        engine
            .replace_source(&format!("f{i:02}.rs"), "fn find() {}\n")
            .unwrap();
    }
    engine
        .replace_source("z/sleep.rs", "fn sleep_ms() {}\n")
        .unwrap();
    drain(&mut engine);
    let first = |batch: &context_foundry::store::CandidateBatch| {
        let item = &batch.items[0];
        (item.tier, source(item).path.clone(), item.label.clone())
    };
    let sleep_ms = (1, "z/sleep.rs".to_owned(), "fn sleep_ms".to_owned());
    let tier_one = |batch: &context_foundry::store::CandidateBatch| -> Vec<String> {
        batch
            .items
            .iter()
            .filter(|item| item.tier == 1)
            .map(|item| item.label.clone())
            .collect()
    };

    // Marked runs only: the unmarked word's definitions are not tier 1. A
    // double-backtick span marks too.
    for query in ["find `sleep_ms`", "``sleep_ms`` find"] {
        let batch = engine
            .search_candidates(query, None, 64, &Control::unbounded())
            .unwrap();
        assert_eq!(first(&batch), sleep_ms, "{query}");
        assert_eq!(tier_one(&batch), ["fn sleep_ms"], "{query}");
    }
    let context = engine
        .context_candidates("find `sleep_ms`", Strategy::Search, &Control::unbounded())
        .unwrap();
    assert_eq!(first(&context), sleep_ms, "the first context unit");

    // Unmarked (an unclosed backtick is literal): 1 definition before 70, then
    // `find` fills the 63 slots left, with definitions left over.
    for query in ["find sleep_ms", "sleep_ms `find"] {
        let batch = engine
            .search_candidates(query, None, 64, &Control::unbounded())
            .unwrap();
        assert_eq!(first(&batch), sleep_ms, "{query}");
        let labels = tier_one(&batch);
        assert_eq!(labels.len(), 64, "{query}");
        assert!(
            labels[1..].iter().all(|label| label == "fn find"),
            "{query}"
        );
        assert!(batch.counters.candidates_full, "{query}");
    }
}

/// Runs with equally many definitions order by run text (not query order or
/// path); the count obeys the `path` filter, so a run common elsewhere but
/// rare in the subtree comes first there.
#[test]
fn tier_one_breaks_count_ties_by_run_text_and_counts_within_the_path_filter() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    engine
        .replace_source("ties/a.rs", "fn beta_tie() {}\n")
        .unwrap();
    engine
        .replace_source("ties/b.rs", "fn alpha_tie() {}\n")
        .unwrap();
    // `wide`: 6 definitions, 1 under `app/`; `narrow`: 2, both under `app/`.
    for i in 0..5 {
        engine
            .replace_source(&format!("other/w{i}.rs"), "fn wide() {}\n")
            .unwrap();
    }
    engine
        .replace_source("app/wide.rs", "fn wide() {}\n")
        .unwrap();
    for i in 0..2 {
        engine
            .replace_source(&format!("app/narrow{i}.rs"), "fn narrow() {}\n")
            .unwrap();
    }
    drain(&mut engine);
    let control = Control::unbounded();
    let tier_one = |query: &str, path: Option<&str>| -> Vec<(String, String)> {
        engine
            .search_candidates(query, path, 64, &control)
            .unwrap()
            .items
            .iter()
            .filter(|item| item.tier == 1)
            .map(|item| (item.label.clone(), source(item).path.clone()))
            .collect()
    };
    let unit = |label: &str, path: &str| (label.to_owned(), path.to_owned());
    assert_eq!(
        tier_one("beta_tie alpha_tie", None),
        [
            unit("fn alpha_tie", "ties/b.rs"),
            unit("fn beta_tie", "ties/a.rs"),
        ]
    );
    let everywhere = tier_one("wide narrow", None);
    assert_eq!(everywhere.len(), 8);
    assert_eq!(
        everywhere[..3],
        [
            unit("fn narrow", "app/narrow0.rs"),
            unit("fn narrow", "app/narrow1.rs"),
            unit("fn wide", "app/wide.rs"),
        ]
    );
    assert_eq!(
        tier_one("wide narrow", Some("app")),
        [
            unit("fn wide", "app/wide.rs"),
            unit("fn narrow", "app/narrow0.rs"),
            unit("fn narrow", "app/narrow1.rs"),
        ]
    );
}

/// Tier 1 ranks the first 32 distinct runs only (a repeat uses no slot), and
/// reports a full window only when definitions are left over after 64.
#[test]
fn tier_one_bounds_its_runs_and_fills_only_with_definitions_left_over() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    for i in 0..64 {
        engine
            .replace_source(&format!("d{i:02}.rs"), "fn edge_def() {}\n")
            .unwrap();
    }
    drain(&mut engine);
    let tier_one = |engine: &Engine, query: &str| {
        let batch = engine
            .search_candidates(query, None, 64, &Control::unbounded())
            .unwrap();
        let count = batch.items.iter().filter(|item| item.tier == 1).count();
        (count, batch.counters.candidates_full)
    };
    let filler = |n: usize| -> String {
        (0..n)
            .map(|i| format!("w{i:02}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_eq!(tier_one(&engine, "edge_def"), (64, false), "exactly 64");
    assert_eq!(
        tier_one(&engine, &format!("edge_def {}", filler(32))).0,
        64,
        "the first 32 distinct runs are ranked"
    );
    assert_eq!(
        tier_one(&engine, &format!("{} edge_def", filler(32))).0,
        0,
        "the 33rd distinct run is not ranked"
    );
    assert_eq!(
        tier_one(&engine, &format!("{} w00 edge_def", filler(31))).0,
        64,
        "a repeated run is the same run"
    );
    engine
        .replace_source("d64.rs", "fn edge_def() {}\n")
        .unwrap();
    drain(&mut engine);
    assert_eq!(tier_one(&engine, "edge_def"), (64, true), "65 overflow");
}

#[test]
fn more_than_256_equal_score_ties_select_identical_handles_across_shuffled_builds() {
    // One identical comment block per file: equal scores, distinct keys.
    const TIE: &str = "// zephyr tie\n";
    const N: usize = 300;
    let sources: Vec<(String, &str)> = (0..N).map(|i| (format!("t{i:03}.rs"), TIE)).collect();
    let mut by_hash: Vec<&str> = sources.iter().map(|(path, _)| path.as_str()).collect();
    by_hash.sort_by_key(|path| contract_key_hash(path, 0));
    let mut window = by_hash[..256].to_vec();
    window.sort();
    // Both builds bind the same root, so whole v2 handles compare: the block
    // is bytes 0-14, the source digest and workspace prefix are fixed.
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    std::fs::create_dir(&root).unwrap();
    let sha32 = {
        use sha2::{Digest, Sha256};
        Sha256::digest(TIE.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()[..32]
            .to_owned()
    };
    let ws16 = context_foundry::workspace_id_for_root(&root).unwrap()[..16].to_owned();
    let handle = |path: &str| format!("{path}#0-14@{sha32}.{ws16}");
    let expected: Vec<String> = window[..64].iter().map(|path| handle(path)).collect();
    let by_name: Vec<String> = (0..64).map(|i| handle(&format!("t{i:03}.rs"))).collect();
    assert_ne!(expected, by_name, "the tie cutoff differs from name order");
    let orders: [(Vec<usize>, usize); 2] = [
        ((0..N).collect(), 60),
        ((0..N).map(|i| (i * 7919 + 13) % N).collect(), 37),
    ];
    let mut builds: Vec<Vec<String>> = Vec::new();
    for (order, size) in &orders {
        let store = tempfile::tempdir().unwrap();
        let mut engine = Engine::initialize(store.path(), &root).unwrap();
        index_in_batches(&mut engine, &sources, order, *size);
        let batch = engine
            .search_candidates("zephyr", None, 64, &Control::unbounded())
            .unwrap();
        assert!(batch.items.iter().all(|item| item.tier == 2));
        assert!(batch.counters.candidates_full && batch.counters.truncated);
        builds.push(
            batch
                .items
                .iter()
                .map(|item| source(item).to_v2())
                .collect(),
        );
    }
    assert_eq!(
        builds[0], builds[1],
        "identical handles across shuffled builds"
    );
    assert_eq!(builds[0], expected);
}

#[test]
fn the_per_file_cap_counts_every_skip_before_the_limit_cut_in_search_and_context() {
    // Six equal-score units in a.rs and one in b.rs; each unit is 24 bytes
    // plus its LF.
    const A: &str = "fn capa() { capmark(); }\nfn capb() { capmark(); }\nfn capc() { capmark(); }\nfn capd() { capmark(); }\nfn cape() { capmark(); }\nfn capf() { capmark(); }\n";
    const B: &str = "fn capz() { capmark(); }\n";
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    engine.replace_source("a.rs", A).unwrap();
    engine.replace_source("b.rs", B).unwrap();
    drain(&mut engine);
    let survivors = [
        ("a.rs", 0, 24, "fn capa"),
        ("a.rs", 25, 49, "fn capb"),
        ("a.rs", 50, 74, "fn capc"),
        ("a.rs", 75, 99, "fn capd"),
        ("b.rs", 0, 24, "fn capz"),
    ];
    for (limit, truncated) in [(64usize, false), (1, true)] {
        let batch = engine
            .search_candidates("capmark", None, limit, &Control::unbounded())
            .unwrap();
        let got: Vec<(&str, u64, u64, &str)> = batch
            .items
            .iter()
            .map(|item| {
                (
                    source(item).path.as_str(),
                    source(item).start,
                    source(item).end,
                    item.label.as_str(),
                )
            })
            .collect();
        assert_eq!(got, survivors[..limit.min(5)], "limit {limit}");
        // `cape` and `capf` are capped whatever the limit.
        assert_eq!(batch.counters.capped, 2, "limit {limit}");
        assert_eq!(batch.counters.truncated, truncated, "limit {limit}");
        let outcome = engine.search("capmark", limit).unwrap();
        assert_eq!(outcome.capped, 2, "limit {limit}");
        let packed =
            response::pack_search(&outcome, Budget::request(1024), &response::stdout_bytes)
                .unwrap();
        let parsed = parse_v2(&packed.text).unwrap();
        assert!(parsed.header.contains(&"capped:2".to_owned()), "{parsed:?}");
    }
    let batch = engine
        .context_candidates("capmark", Strategy::Search, &Control::unbounded())
        .unwrap();
    assert_eq!(batch.counters.capped, 2);
    let spans = |tier: fn(u8) -> bool| -> Vec<(&str, u64, u64)> {
        batch
            .items
            .iter()
            .filter(|item| tier(item.tier))
            .map(|item| {
                (
                    source(item).path.as_str(),
                    source(item).start,
                    source(item).end,
                )
            })
            .collect()
    };
    let want: Vec<(&str, u64, u64)> = survivors
        .iter()
        .map(|(path, start, end, _)| (*path, *start, *end))
        .collect();
    assert_eq!(spans(|tier| tier <= 2), want);
    // b.rs's one unit spans the file, so only a.rs (6 × 25 bytes) gets an
    // outline.
    assert_eq!(spans(|tier| tier == 4), [("a.rs", 0, 150)]);
    let packed =
        response::pack_context(&batch, Budget::request(4096), &response::stdout_bytes).unwrap();
    let parsed = parse_v2(&packed.text).unwrap();
    assert!(parsed.header.contains(&"capped:2".to_owned()), "{parsed:?}");
}

// ---------------------------------------------------------------------------
// 001 T006: forms, outlines and retrieve views
// ---------------------------------------------------------------------------

/// Four units, each a signature line, a 30-line interior and a closing line,
/// then a blank line: unit k spans lines 33k+1 ..= 33k+32.
fn four_units() -> String {
    (0..4)
        .map(|k| {
            let body: String = (0..30).map(|j| format!("    let v{j} = {k};\n")).collect();
            format!("fn unit_{k}() {{\n{body}}}\n\n")
        })
        .collect()
}

/// The `⋯ a-b` markers of an outline body, in order.
fn markers(body: &str) -> Vec<(usize, usize)> {
    body.lines()
        .filter_map(|line| line.trim_start().strip_prefix("⋯ "))
        .map(|range| {
            let (a, b) = range.split_once('-').unwrap();
            (a.parse().unwrap(), b.parse().unwrap())
        })
        .collect()
}

#[test]
fn outline_view_markers_retrieve_exactly_and_the_view_never_paginates() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    let body = four_units();
    engine.replace_source("big.rs", &body).unwrap();
    engine.replace_source("notes.txt", "alpha_note\n").unwrap();
    drain(&mut engine);
    let unit = engine
        .search_candidates("unit_0", None, 1, &Control::unbounded())
        .unwrap()
        .items
        .remove(0);
    let file = SourceHandle {
        start: 0,
        end: body.len() as u64,
        ..source(&unit).clone()
    }
    .to_v2();
    let out = engine.retrieve_outline(&file, None, 32768).unwrap();
    // outline-min folds all four interiors (16 visible lines); the outline
    // form unfolds breadth-first: 16 + 29 = 45 < 60, then 74, which reaches
    // 60 within 120.
    assert_eq!(
        markers(&out.outline_min),
        [(2, 31), (35, 64), (68, 97), (101, 130)]
    );
    assert_eq!(markers(&out.outline), [(68, 97), (101, 130)]);
    assert_eq!((out.start_line, out.end_line), (1, 132));
    // Every marker's lines are exactly what `retrieve --lines a-b` returns.
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    for (a, b) in markers(&out.outline_min) {
        let read = engine
            .retrieve(&file, Some(&format!("{a}-{b}")), 32768)
            .unwrap();
        assert_eq!(read.span, lines[a - 1..b].concat().as_bytes(), "{a}-{b}");
    }
    // The view is whole-or-nothing: outline, else outline-min, else refused
    // with a sufficient budget; never `next`.
    let pack = |tokens: usize| {
        response::pack_retrieve_outline(&out, Budget::request(tokens), &response::stdout_bytes)
    };
    let full = pack(32768).unwrap();
    let parsed = parse_v2(&full.text).unwrap();
    assert!(parsed.next.is_none() && !full.text.contains("\nnext: "));
    assert_eq!(parsed.items[0].form.as_deref(), Some("outline"));
    assert_eq!(parsed.items[0].body, out.outline);
    assert_eq!(parsed.items[0].lines.as_deref(), Some("L1-132"));
    // The refusal names a budget sufficient for outline-min, far below the
    // outline form; at that budget the ladder falls back to outline-min.
    let minimum = match pack(1).unwrap_err() {
        FoundryError::BudgetTooSmall { minimum_tokens } => minimum_tokens,
        other => panic!("{other:?}"),
    };
    assert!(minimum + 100 < full.tokens, "{minimum} vs {}", full.tokens);
    let smaller = pack(minimum).unwrap();
    let parsed = parse_v2(&smaller.text).unwrap();
    assert_eq!(parsed.items[0].form.as_deref(), Some("outline"));
    assert_eq!(parsed.items[0].body, out.outline_min);
    assert!(parsed.next.is_none() && !smaller.text.contains("\nnext: "));
    // An unmapped language has no outline view.
    let note = engine
        .search("alpha_note", 1)
        .unwrap()
        .hits
        .remove(0)
        .handle;
    assert_eq!(
        code(
            &engine
                .retrieve_outline(&note.to_v2(), None, 2048)
                .unwrap_err()
        ),
        "unsupported_mode"
    );
}

#[test]
fn a_unit_larger_than_the_budget_is_delivered_in_its_signature_form() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    let lets: String = (0..200)
        .map(|j| format!("    let v{j} = a + {j};\n"))
        .collect();
    engine
        .replace_source(
            "huge.rs",
            &format!("pub fn huge_unit(a: u8) -> u8 {{\n{lets}    a\n}}\n"),
        )
        .unwrap();
    drain(&mut engine);
    let batch = engine
        .context_candidates("huge_unit", Strategy::Search, &Control::unbounded())
        .unwrap();
    // The only unit spans the file, so there is no file outline.
    assert_eq!(batch.items.len(), 1);
    let signature = "pub fn huge_unit(a: u8) -> u8 {\n    ⋯ 2-202\n}";
    assert_eq!(
        batch.items[0].forms[1],
        context_foundry::store::RenderedForm::Signature(signature.to_owned())
    );
    let packed =
        response::pack_context(&batch, Budget::request(256), &response::stdout_bytes).unwrap();
    let parsed = parse_v2(&packed.text).unwrap();
    let item = &parsed.items[0];
    assert_eq!(item.form.as_deref(), Some("signature"));
    assert_eq!(item.label.as_deref(), Some("fn huge_unit"));
    assert_eq!(item.lines.as_deref(), Some("L1-203"));
    // A form body is line-structured: its framing LF cannot be told from
    // content (only verbatim bodies take their length from the handle).
    assert_eq!(item.body, format!("{signature}\n"));
    // With room, the verbatim form comes first.
    let packed =
        response::pack_context(&batch, Budget::request(32768), &response::stdout_bytes).unwrap();
    assert_eq!(parse_v2(&packed.text).unwrap().items[0].form, None);
}

#[test]
fn context_adds_at_most_three_file_outlines_for_the_first_distinct_files() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    for i in 0..5 {
        engine
            .replace_source(
                &format!("f{i}.rs"),
                "fn left() {\n    spread_probe();\n}\n\nfn right() {\n    spread_probe();\n}\n",
            )
            .unwrap();
    }
    drain(&mut engine);
    let batch = engine
        .context_candidates("spread_probe", Strategy::Search, &Control::unbounded())
        .unwrap();
    let outlines: Vec<&str> = batch
        .items
        .iter()
        .filter(|item| item.tier == context_foundry::store::TIER_OUTLINE)
        .map(|item| source(item).path.as_str())
        .collect();
    assert_eq!(outlines, ["f0.rs", "f1.rs", "f2.rs"]);
    // Outlines follow every unit.
    let first_outline = batch.items.iter().position(|item| item.tier == 4).unwrap();
    assert_eq!(first_outline, batch.items.len() - 3);
    assert_eq!(first_outline, 10, "two units in each of five files");
}

#[test]
fn a_python_unit_signature_form_markers_retrieve_through_its_clipped_handle() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    // Line 1 `def`, lines 2-201 assignments, line 202 `return`: the unit's
    // range ends at `return v0`, before the final LF.
    let assignments: String = (0..200).map(|i| format!("    v{i} = {i}\n")).collect();
    let text = format!("def compute():\n{assignments}    return v0\n");
    engine.replace_source("calc.py", &text).unwrap();
    drain(&mut engine);
    let batch = engine
        .context_candidates("compute", Strategy::Search, &Control::unbounded())
        .unwrap();
    let unit = &batch.items[0];
    assert_eq!(source(unit).end as usize, text.len() - 1);
    assert_eq!(
        unit.forms[1],
        context_foundry::store::RenderedForm::Signature("def compute():\n    ⋯ 2-202\n".into())
    );
    let packed =
        response::pack_context(&batch, Budget::request(128), &response::stdout_bytes).unwrap();
    assert_eq!(
        parse_v2(&packed.text).unwrap().items[0].form.as_deref(),
        Some("signature")
    );
    // The marker's lines, read through the unit's own handle, are exactly
    // the elided bytes: lines 2-202 without the LF outside the handle.
    let read = engine
        .retrieve(&source(unit).to_v2(), Some("2-202"), 32768)
        .unwrap();
    let from = text.find('\n').unwrap() + 1;
    assert_eq!(read.span, &text.as_bytes()[from..text.len() - 1]);
}

#[test]
fn a_filled_graph_examination_window_reports_candidates_full() {
    for (count, full) in [(31usize, false), (32, true), (33, true)] {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("ws");
        let (_store, mut engine) = setup(&root);
        const HUB: &str = "fn hub_probe() {}\n";
        engine.replace_source("hub.rs", HUB).unwrap();
        let mut edges = Vec::new();
        for i in 0..count {
            let (path, body) = (format!("leaf_{i}.rs"), format!("fn leaf_{i}() {{}}\n"));
            engine.replace_source(&path, &body).unwrap();
            edges.push(Edge {
                from: endpoint("hub.rs", HUB),
                to: endpoint(&path, &body),
                kind: "calls".into(),
                evidence: "manual".into(),
            });
        }
        drain(&mut engine);
        engine
            .import_graph(&GraphBundle {
                provider: "fixture".into(),
                revision: "r1".into(),
                edges,
            })
            .unwrap();
        let batch = engine
            .context_candidates("hub_probe", Strategy::Graph, &Control::unbounded())
            .unwrap();
        assert_eq!(batch.counters.graph, Some("ok"));
        assert_eq!(batch.counters.candidates_full, full, "{count} edges");
        let packed =
            response::pack_context(&batch, Budget::request(32768), &response::stdout_bytes)
                .unwrap();
        let parsed = parse_v2(&packed.text).unwrap();
        assert_eq!(
            parsed.header.contains(&"candidates:full".to_owned()),
            full,
            "{count} edges: {:?}",
            parsed.header
        );
    }
}

#[test]
fn an_outline_no_budget_can_deliver_is_unsupported_mode_not_a_false_hint() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    // A mapped language without units: both outline forms are the text.
    // Over 32768 tokens at any budget.
    let big: String = (0..20_000).map(|i| format!("key_{i} = {i}\n")).collect();
    engine.replace_source("big.toml", &big).unwrap();
    // About 128 KiB that fits 32768 tokens, but whose JSON escaping (each
    // tab becomes two bytes) exceeds 256 KiB.
    let tabs = format!("x{}\n", "\t".repeat(1022)).repeat(128);
    engine.replace_source("tabs.toml", &tabs).unwrap();
    drain(&mut engine);
    let whole = |path: &str, body: &str| {
        let id = engine.status().unwrap().workspace_id.unwrap();
        SourceHandle {
            workspace_id: id,
            path: path.to_owned(),
            sha256: digest(body.as_bytes()),
            start: 0,
            end: body.len() as u64,
        }
        .to_v2()
    };
    let escaped = |text: &str| serde_json::to_string(text).unwrap().len();
    for (path, body, boundary) in [
        (
            "big.toml",
            big.as_str(),
            &response::stdout_bytes as response::ByteMeasure,
        ),
        ("tabs.toml", tabs.as_str(), &escaped),
    ] {
        let out = engine
            .retrieve_outline(&whole(path, body), None, 32768)
            .unwrap();
        let err =
            response::pack_retrieve_outline(&out, Budget::request(32768), boundary).unwrap_err();
        assert_eq!(code(&err), "unsupported_mode", "{path}: {err:?}");
    }
    // The tab-heavy outline does fit the CLI's stdout measure at 32768.
    let out = engine
        .retrieve_outline(&whole("tabs.toml", &tabs), None, 32768)
        .unwrap();
    assert!(
        response::pack_retrieve_outline(&out, Budget::request(32768), &response::stdout_bytes)
            .is_ok()
    );
}

/// 001 T005 leading-run amendment: a definition's doc comment and attribute
/// join its unit (so a question worded like the doc finds it), while a tier-1
/// hit's best line stays the definition's own line, not a doc or attribute.
#[test]
fn a_documented_definition_owns_its_doc_and_tier_one_names_its_definition() {
    const DOCUMENTED: &str = "use std::fmt;\n\n/// Kestrel ledger merging.\n/// Second line.\n#[derive(Default)]\npub struct LedgerMerge {\n    seen: u8,\n}\n";
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    engine.replace_source("src/ledger.rs", DOCUMENTED).unwrap();
    drain(&mut engine);
    let control = Control::unbounded();
    let start = DOCUMENTED.find("/// Kestrel").unwrap() as u64;
    let end = DOCUMENTED.rfind('}').unwrap() as u64 + 1;

    let exact = engine
        .search_candidates("LedgerMerge", None, 10, &control)
        .unwrap();
    let hit = &exact.items[0];
    assert_eq!(hit.tier, 1);
    assert_eq!((source(hit).start, source(hit).end), (start, end));
    assert_eq!((hit.start_line, hit.end_line), (3, 8));
    assert_eq!(
        hit.line, 6,
        "the best line is the definition, not its attribute"
    );

    let worded = engine
        .search_candidates("kestrel ledger", None, 10, &control)
        .unwrap();
    let first = &worded.items[0];
    assert_eq!(first.tier, 2);
    assert_eq!(
        (source(first).start, source(first).end),
        (start, end),
        "the doc's words find the documented unit, not the preceding block"
    );
}

// --- Compact context (context-v2 § Compact context, 2026-10-06) -----------

use context_foundry::response::{RootHeader, stdout_bytes};
use context_foundry::roots::{RootBatch, merge_context};
use context_foundry::store::{CandidateBatch, TIER_OUTLINE};
use context_foundry::testkit::{V2Kind, V2Response};

/// `fn sleep_ms`, defined once with its doc, alone in its file (so that file
/// has no outline).
const CLOCK: &str = "/// Sleeps for `ms` milliseconds.\npub fn sleep_ms(ms: u64) {\n    let micros = ms * 1000;\n    std::thread::sleep(std::time::Duration::from_micros(micros));\n}\n";

/// The `i`th caller of `sleep_ms`: one unit calling it, then a second
/// function, so the caller's file has an outline.
fn caller_source(i: usize) -> String {
    format!("fn w{i}() {{\n    sleep_ms({i});\n}}\nfn x{i}() {{}}\n")
}

/// `sleep_ms` defined once and called from twelve files; `alpha_one` and
/// `beta_two` defined once and called together from `both`; `retry_twice`
/// defined twice and `gamma_three` three times.
fn compact_fixture(root: &Path) -> (tempfile::TempDir, Engine) {
    let (store, mut engine) = setup(root);
    engine.replace_source("src/clock.rs", CLOCK).unwrap();
    for i in 0..12 {
        engine
            .replace_source(&format!("src/use{i:02}.rs"), &caller_source(i))
            .unwrap();
    }
    for (path, body) in [
        ("src/alpha.rs", "fn alpha_one() {}\n"),
        ("src/beta.rs", "fn beta_two() {}\n"),
        ("src/both.rs", "fn both() { alpha_one(); beta_two(); }\n"),
        ("src/retry/a.rs", "fn retry_twice() {}\n"),
        ("src/retry/b.rs", "fn retry_twice() {}\n"),
        ("src/gamma/a.rs", "fn gamma_three() {}\n"),
        ("src/gamma/b.rs", "fn gamma_three() {}\n"),
        ("src/gamma/c.rs", "fn gamma_three() {}\n"),
    ] {
        engine.replace_source(path, body).unwrap();
    }
    drain(&mut engine);
    (store, engine)
}

/// One context at 2048 tokens: its candidate batch, packed text and parse.
fn context_of(
    engine: &Engine,
    query: &str,
    strategy: Strategy,
) -> (CandidateBatch, response::PackedText, V2Response) {
    let batch = engine
        .context_candidates(query, strategy, &Control::unbounded())
        .unwrap();
    let packed =
        response::pack_context(&batch, Budget::request(2048), &response::stdout_bytes).unwrap();
    let parsed = parse_v2(&packed.text).unwrap_or_else(|e| panic!("{e}\n{}", packed.text));
    (batch, packed, parsed)
}

/// The pre-amendment rendering of a batch: without its marked counts the
/// compact rule cannot apply.
fn packed_without_marks(batch: &CandidateBatch) -> String {
    let mut plain = batch.clone();
    plain.marked.clear();
    response::pack_context(&plain, Budget::request(2048), &response::stdout_bytes)
        .unwrap()
        .text
}

fn says_compact(parsed: &V2Response) -> bool {
    parsed.header.iter().any(|segment| segment == "compact")
}

fn kinds(parsed: &V2Response) -> Vec<V2Kind> {
    parsed.items.iter().map(|item| item.kind).collect()
}

/// A query whose one marked name has exactly one definition gets that
/// definition verbatim, then the next 8 candidates in context order as one
/// locator line each; file outlines and the rest are omitted and counted,
/// and the answer costs fewer tokens than the same question unmarked.
#[test]
fn a_unique_marked_definition_packs_compact_with_at_most_eight_pointers() {
    let fixture = tempfile::tempdir().unwrap();
    let (_store, engine) = compact_fixture(&fixture.path().join("ws"));
    let (batch, packed, parsed) = context_of(&engine, "find `sleep_ms`", Strategy::Search);
    assert!(batch.compact());
    let header = packed.text.lines().next().unwrap();
    assert!(says_compact(&parsed), "{header}");
    assert!(response::count_tokens(header) <= 40, "{header}");

    let definition = &parsed.items[0];
    let unit = source(&batch.items[0]);
    assert_eq!(definition.kind, V2Kind::Source);
    assert_eq!(definition.handle, unit.to_v2());
    assert_eq!(definition.label.as_deref(), Some("fn sleep_ms"));
    assert_eq!(definition.form, None);
    let bytes = &CLOCK[unit.start as usize..unit.end as usize];
    assert_eq!(definition.body, bytes);

    // The pointers: the first 8 candidates past the definition, outlines
    // skipped, each a locator line quoting its best line.
    let pointed: Vec<String> = batch
        .items
        .iter()
        .filter(|item| item.tier != 1 && item.tier != TIER_OUTLINE)
        .take(8)
        .map(|item| source(item).to_v2())
        .collect();
    let pointers = &parsed.items[1..];
    assert_eq!(pointers.len(), 8);
    assert_eq!(pointed.len(), 8);
    for (pointer, handle) in pointers.iter().zip(&pointed) {
        assert_eq!(pointer.kind, V2Kind::Locator);
        assert_eq!(&pointer.handle, handle);
        assert!(pointer.body.starts_with("sleep_ms("), "{pointer:?}");
    }

    // Four more callers and the two caller-file outlines are omitted.
    assert!(batch.items.iter().any(|item| item.tier == TIER_OUTLINE));
    assert_eq!(packed.omitted, batch.items.len() - 9);
    let omitted = format!("omitted:{}", packed.omitted);
    assert!(parsed.header.contains(&omitted), "{header}");

    // The same question unmarked fills the budget as before.
    let (plain_batch, plain, _) = context_of(&engine, "find sleep_ms", Strategy::Search);
    assert!(!plain_batch.compact());
    assert!(!plain.text.lines().next().unwrap().contains("compact"));
    let (compact, full) = (packed.tokens, plain.tokens);
    assert!(compact < full, "{compact} vs {full}");
}

/// Compact exactly when some marked run has one definition and none has
/// more; a marked run without definitions neither triggers nor blocks it,
/// and an unmarked query never is. Every other response is the
/// pre-amendment rendering, byte for byte.
#[test]
fn the_compact_rule_reads_every_marked_run_count() {
    let fixture = tempfile::tempdir().unwrap();
    let (_store, engine) = compact_fixture(&fixture.path().join("ws"));
    // Each query and the unique definitions it delivers, which arrive in
    // tier-1 order (equal counts order by run text).
    let cases = [
        ("`alpha_one`", 1),
        ("`beta_two` and `alpha_one`", 2),
        ("`alpha_one` `no_such_name`", 1),
        ("`no_such_name`", 0),
        ("`retry_twice`", 0),
        ("`alpha_one` `gamma_three`", 0),
        ("alpha_one", 0),
    ];
    let labels = ["fn alpha_one", "fn beta_two"];
    for (query, unique) in cases {
        let (batch, packed, parsed) = context_of(&engine, query, Strategy::Search);
        let compact = unique > 0;
        assert_eq!(batch.compact(), compact, "{query}");
        assert_eq!(says_compact(&parsed), compact, "{query}");
        if compact {
            // The definitions, then `both`, the one other candidate, as a
            // pointer.
            let mut want = vec![V2Kind::Source; unique];
            want.push(V2Kind::Locator);
            assert_eq!(kinds(&parsed), want, "{query}");
            for (item, label) in parsed.items.iter().zip(&labels[..unique]) {
                assert_eq!(item.label.as_deref(), Some(*label), "{query}");
                assert_eq!(item.form, None, "{query}");
            }
        } else {
            assert_eq!(packed.text, packed_without_marks(&batch), "{query}");
            assert!(!kinds(&parsed).contains(&V2Kind::Locator), "{query}");
        }
    }
}

/// Under the graph strategy the edge lines follow the definition as
/// pointers, inside the eight.
#[test]
fn a_compact_graph_context_points_with_its_edge_lines() {
    let fixture = tempfile::tempdir().unwrap();
    let (_store, engine) = compact_fixture(&fixture.path().join("ws"));
    let bundle = GraphBundle {
        provider: "fixture".into(),
        revision: "r1".into(),
        edges: vec![Edge {
            from: endpoint("src/use00.rs", &caller_source(0)),
            to: endpoint("src/clock.rs", CLOCK),
            kind: "calls".into(),
            evidence: "manual".into(),
        }],
    };
    engine.import_graph(&bundle).unwrap();
    let (batch, _, parsed) = context_of(&engine, "find `sleep_ms`", Strategy::Graph);
    assert_eq!(batch.counters.graph, Some("ok"));
    assert!(says_compact(&parsed));
    let mut want = vec![V2Kind::Source, V2Kind::Edge];
    want.resize(1 + 8, V2Kind::Locator);
    assert_eq!(kinds(&parsed), want);
    assert!(parsed.items[1].body.contains("--calls--> src/clock.rs"));
}

/// A definition too large to fit verbatim takes its signature form exactly
/// as an unmarked context delivers it; a budget below the header refuses.
#[test]
fn a_compact_definition_keeps_the_ladder_and_the_refusal() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    let lets: String = (0..200)
        .map(|j| format!("    let v{j} = a + {j};\n"))
        .collect();
    engine
        .replace_source(
            "huge.rs",
            &format!("pub fn huge_unit(a: u8) -> u8 {{\n{lets}    a\n}}\n"),
        )
        .unwrap();
    drain(&mut engine);
    let control = Control::unbounded();
    let pack = |query: &str, tokens: usize| {
        let batch = engine
            .context_candidates(query, Strategy::Search, &control)
            .unwrap();
        response::pack_context(&batch, Budget::request(tokens), &response::stdout_bytes)
    };
    let compact = parse_v2(&pack("`huge_unit`", 256).unwrap().text).unwrap();
    let plain = parse_v2(&pack("huge_unit", 256).unwrap().text).unwrap();
    assert!(says_compact(&compact));
    assert!(!says_compact(&plain));
    assert_eq!(compact.items, plain.items);
    assert_eq!(compact.items[0].form.as_deref(), Some("signature"));
    let refused = pack("`huge_unit`", 1).unwrap_err();
    assert_eq!(code(&refused), "budget_too_small");
}

/// The rule reads tier 1's exact count, so the `path` filter that restricts
/// tier 1 also decides which definitions count.
#[test]
fn the_path_filter_decides_which_definitions_count() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    for path in ["app/wide.rs", "other/wide.rs"] {
        engine.replace_source(path, "fn wide() {}\n").unwrap();
    }
    drain(&mut engine);
    let control = Control::unbounded();
    let batch = |query: &str, filter: Option<&str>| {
        engine
            .search_candidates(query, filter, 10, &control)
            .unwrap()
    };
    let counts = |path: Option<&str>| -> Vec<u64> {
        let marked = batch("`wide` `absent`", path).marked;
        marked.iter().map(|run| run.definitions).collect()
    };
    assert_eq!(counts(None), [2, 0]);
    assert_eq!(counts(Some("app")), [1, 0]);
    assert!(!batch("`wide`", None).compact());
    assert!(batch("`wide`", Some("app")).compact());
    assert!(!batch("wide", Some("app")).compact(), "unmarked");
}

/// Context over the 007 merge of `roots`: the merged batch's compactness and
/// the packed response, parsed.
fn merged_context(roots: &[(&str, Engine)], query: &str) -> (bool, V2Response) {
    let control = Control::unbounded();
    let mut batches = Vec::new();
    let mut headers = Vec::new();
    for (alias, engine) in roots {
        let batch = engine
            .context_candidates(query, Strategy::Search, &control)
            .unwrap();
        let revision = batch.freshness.source_revision;
        headers.push(RootHeader {
            alias: (*alias).to_owned(),
            label: (*alias).to_owned(),
            serving: Some((revision, "complete".to_owned(), 0)),
            coverage: None,
        });
        batches.push(RootBatch {
            alias: (*alias).to_owned(),
            batch,
        });
    }
    let merged = merge_context(&batches);
    let budget = Budget::request(2048);
    let packed = response::pack_context_roots(&merged, &headers, budget, &stdout_bytes);
    let text = packed.unwrap().text;
    (merged.compact(), parse_v2(&text).unwrap())
}

/// A multi-root run's count is summed over the merged roots: a name defined
/// once in each of two roots is not unique, while one defined in a single
/// root is.
#[test]
fn multi_root_compactness_sums_each_marked_run_over_the_roots() {
    let fixture = tempfile::tempdir().unwrap();
    let mut stores = Vec::new();
    let mut roots = Vec::new();
    for (alias, body) in [
        ("primary", "fn shared_name() {}\n\nfn only_here() {}\n"),
        ("ref1", "fn shared_name() {}\n"),
    ] {
        let (store, mut engine) = setup(&fixture.path().join(alias));
        engine.replace_source("src/lib.rs", body).unwrap();
        drain(&mut engine);
        stores.push(store);
        roots.push((alias, engine));
    }
    let control = Control::unbounded();
    for (alias, engine) in &roots {
        let alone = engine
            .context_candidates("`shared_name`", Strategy::Search, &control)
            .unwrap();
        assert!(alone.compact(), "{alias} alone defines it once");
    }
    let (compact, parsed) = merged_context(&roots, "`shared_name`");
    assert!(!compact);
    assert!(!says_compact(&parsed));
    assert_eq!(kinds(&parsed)[..2], [V2Kind::Source, V2Kind::Source]);

    let (compact, parsed) = merged_context(&roots, "`only_here`");
    assert!(compact);
    assert!(says_compact(&parsed));
    assert_eq!(parsed.items[0].label.as_deref(), Some("fn only_here"));
}

// ---------------------------------------------------------------------------
// 001 T009: parallel indexing (context-v2 § Parallel indexing).

use context_foundry::store::fault_names as index_points;
use context_foundry::store::index_hooks::{self, Event, Hooks, committed_documents};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

const KIB: usize = 1024;
/// Every T009 barrier and bounded run gives up after this long: a timeout is
/// a test failure, never a hang.
const T009_WAIT: Duration = Duration::from_secs(10);

/// What hooked refreshes observed: every added document as JSON in add
/// order, the paths in build-completion order, the hand-out accounting.
#[derive(Default)]
struct Observed {
    added: Vec<String>,
    built: Vec<String>,
    max_outstanding: usize,
    /// `(outstanding, next, sources built so far)` at each wait for room.
    waits: Vec<(usize, usize, usize)>,
    /// A wait for room first lets this many builds finish.
    built_before_release: usize,
    /// A wait for room happened.
    released: bool,
    /// A barrier gave up waiting.
    timed_out: bool,
}

#[derive(Default)]
struct Watch {
    observed: Mutex<Observed>,
    changed: Condvar,
}

impl Watch {
    /// Record every event; `on_build` runs first on each build, inside the
    /// parse panic boundary.
    fn hooks(
        self: &Arc<Self>,
        threads: usize,
        on_build: impl Fn(&Watch, &str) + Send + Sync + 'static,
    ) -> Hooks {
        let watch = Arc::clone(self);
        Hooks {
            threads: Some(threads),
            handout_bytes: None,
            observer: Some(Arc::new(move |event: &Event<'_>| match event {
                Event::Build(path) => on_build(&watch, path),
                Event::Built(path) => {
                    watch
                        .observed
                        .lock()
                        .unwrap()
                        .built
                        .push((*path).to_owned());
                    watch.changed.notify_all();
                }
                Event::HandedOut { outstanding, .. } => {
                    let mut observed = watch.observed.lock().unwrap();
                    observed.max_outstanding = observed.max_outstanding.max(*outstanding);
                }
                Event::Wait { outstanding, next } => {
                    let mut observed =
                        watch.wait_until(|seen| seen.built.len() >= seen.built_before_release);
                    let built = observed.built.len();
                    observed.waits.push((*outstanding, *next, built));
                    observed.released = true;
                    watch.changed.notify_all();
                }
                Event::Add(json) => watch
                    .observed
                    .lock()
                    .unwrap()
                    .added
                    .push((*json).to_owned()),
            })),
        }
    }

    /// Wait until `ready`, at most [`T009_WAIT`]; giving up is recorded and
    /// fails the test at [`Self::take`].
    fn wait_until(&self, ready: impl Fn(&Observed) -> bool) -> MutexGuard<'_, Observed> {
        let deadline = Instant::now() + T009_WAIT;
        let mut observed = self.observed.lock().unwrap();
        while !ready(&observed) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                observed.timed_out = true;
                break;
            }
            observed = self.changed.wait_timeout(observed, left).unwrap().0;
        }
        observed
    }

    fn take(&self) -> Observed {
        let observed = std::mem::take(&mut *self.observed.lock().unwrap());
        assert!(!observed.timed_out, "a T009 barrier timed out");
        observed
    }
}

/// Run `f` on its own thread (hooks and faults are per thread) and return
/// its outcome, an unwind included; not finishing within [`T009_WAIT`] fails
/// the test instead of hanging it.
fn within<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> std::thread::Result<T> {
    let (done, outcome) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)));
    });
    outcome
        .recv_timeout(T009_WAIT)
        .unwrap_or_else(|_| panic!("did not finish within {T009_WAIT:?}"))
}

/// A store bound to `<fixture>/<name>` with `sources` pending, closed: the
/// store path, ready to reopen.
fn pending_store(fixture: &Path, name: &str, sources: &[(&str, &str)]) -> std::path::PathBuf {
    let root = fixture.join(name);
    let store = fixture.join(format!("{name}-store"));
    std::fs::create_dir(&root).unwrap();
    let engine = Engine::initialize(&store, &root).unwrap();
    for (path, body) in sources {
        engine.replace_source(path, body).unwrap();
    }
    store
}

/// Every document shape — units in eight languages, Markdown sections, a
/// mapped language without units, plain blocks, an empty source — and
/// enough tiny generated sources for two pages. The earliest key is
/// `a/first.rs`.
fn t009_sources() -> Vec<(String, String)> {
    let mut sources: Vec<(String, String)> = [
        (
            "a/first.rs",
            "pub mod outer {\n    pub struct Holder {\n        v: u32,\n    }\n    impl Holder {\n        pub fn first_unit(&self) -> u32 {\n            self.v\n        }\n    }\n}\n",
        ),
        (
            "b/tool.py",
            "class Tool:\n    def run(self, x):\n        return x\n\n\ndef helper():\n    return Tool()\n",
        ),
        (
            "c/app.ts",
            "export interface Shape {\n  area(): number;\n}\nexport class Square implements Shape {\n  area(): number {\n    return 4;\n  }\n}\n",
        ),
        (
            "c/view.tsx",
            "export function View() {\n  return <div>view</div>;\n}\n",
        ),
        (
            "c/util.js",
            "function util(a) {\n  return a + 1;\n}\nmodule.exports = { util };\n",
        ),
        (
            "d/main.go",
            "package main\n\nfunc main() {\n\tprintln(\"go\")\n}\n",
        ),
        ("d/x.c", "int add(int a, int b) {\n  return a + b;\n}\n"),
        ("d/Main.java", "class Main {\n  void run() {}\n}\n"),
        (
            "docs/guide.md",
            "# Guide\n\nIntro text.\n\n## Usage\n\nRun it.\n",
        ),
        ("conf.toml", "[package]\nname = \"x\"\n"),
        ("notes.txt", "plain words here\n\nanother paragraph\n"),
        ("empty.rs", ""),
    ]
    .into_iter()
    .map(|(path, body)| (path.to_owned(), body.to_owned()))
    .collect();
    for i in 0..140 {
        sources.push((format!("gen/f{i:03}.rs"), format!("fn g{i}() {{}}\n")));
    }
    sources
}

/// The documents added for `path`, parsed.
fn added_for(added: &[String], path: &str) -> Vec<serde_json::Value> {
    added
        .iter()
        .map(|json| serde_json::from_str::<serde_json::Value>(json).unwrap())
        .filter(|doc| doc["path"][0] == path)
        .collect()
}

#[test]
fn one_and_eight_build_threads_give_equal_documents_and_tables_even_out_of_order() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    std::fs::create_dir(&root).unwrap();
    let sources = t009_sources();
    let mut runs = Vec::new();
    for threads in [1usize, 8] {
        let store = fixture.path().join(format!("store{threads}"));
        let mut engine = Engine::initialize(&store, &root).unwrap();
        for (path, body) in &sources {
            engine.replace_source(path, body).unwrap();
        }
        let watch = Arc::new(Watch::default());
        // On 8 threads the earliest key finishes last on its page: its build
        // waits until the page's 127 other sources were built.
        let hooks = watch.hooks(threads, move |watch, path| {
            if threads > 1 && path == "a/first.rs" {
                drop(watch.wait_until(|seen| seen.built.len() >= 127));
            }
        });
        index_hooks::install(hooks);
        let drained = engine.refresh(&Control::unbounded()).unwrap();
        index_hooks::clear();
        assert_eq!(drained, (sources.len(), 0));
        assert_eq!(engine.status().unwrap().parse_failures, Some(0));
        drop(engine);
        runs.push((threads, watch.take(), testkit::snapshot(&store)));
    }
    let (_, one, one_tables) = &runs[0];
    let (_, eight, eight_tables) = &runs[1];
    // Completions arrived out of order on 8 threads, in key order on 1.
    assert_eq!(one.built[0], "a/first.rs");
    assert_ne!(eight.built[0], "a/first.rs");
    assert_eq!(
        eight.built.iter().position(|path| path == "a/first.rs"),
        Some(127),
        "the earliest key completed last on its page"
    );
    // The same documents, field for field, in the same (key) order.
    assert_eq!(one.added, eight.added);
    let first: Vec<String> = one
        .added
        .iter()
        .map(|json| {
            serde_json::from_str::<serde_json::Value>(json).unwrap()["path"][0]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    let mut in_key_order = first.clone();
    in_key_order.sort();
    assert_eq!(first, in_key_order, "documents are added in key order");
    in_key_order.dedup();
    let mut with_documents: Vec<&str> = sources
        .iter()
        .map(|(path, _)| path.as_str())
        .filter(|path| *path != "empty.rs")
        .collect();
    with_documents.sort_unstable();
    assert_eq!(in_key_order, with_documents, "every non-empty source");
    // Every document shape is present.
    let kinds: std::collections::BTreeSet<String> = one
        .added
        .iter()
        .map(|json| serde_json::from_str::<serde_json::Value>(json).unwrap()["kind"][0].to_string())
        .collect();
    for kind in [
        "\"fn\"",
        "\"struct\"",
        "\"class\"",
        "\"section\"",
        "\"block\"",
    ] {
        assert!(kinds.contains(kind), "{kind} in {kinds:?}");
    }
    // The same store tables.
    assert_eq!(one_tables, eight_tables);
}

#[test]
fn a_panic_in_the_earliest_keys_build_names_it_and_indexes_every_other_key() {
    // Two plain blocks: the filler takes the first past the 2 KiB merge.
    let first_body = format!(
        "fn first_unit() -> u32 {{\n    1\n}}\n\n{}\nfn first_other() {{}}\n",
        "// first filler line\n".repeat(110)
    );
    let first_body = first_body.as_str();
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (store, mut engine) = setup(&root);
    for (path, body) in [
        ("a/first.rs", first_body),
        // The same bytes as an unmapped source: the expected plain blocks.
        ("a/first.txt", first_body),
        ("b/second.rs", "fn second_unit() {}\n"),
        ("c/third.py", "def third_unit():\n    return 3\n"),
        ("d/fourth.md", "# Fourth heading\n\ntext\n"),
    ] {
        engine.replace_source(path, body).unwrap();
    }
    let panicking = |watch: &Arc<Watch>| {
        watch.hooks(8, |_, path| {
            if path == "a/first.rs" {
                panic!("injected parse panic");
            }
        })
    };
    let watch = Arc::new(Watch::default());
    index_hooks::install(panicking(&watch));
    assert_eq!(engine.refresh(&Control::unbounded()).unwrap(), (5, 0));
    index_hooks::clear();
    let observed = watch.take();
    // A named scan failure for the drain, and every pending key cleared.
    assert_eq!(
        engine.take_parse_failures(),
        context_foundry::store::ParseFailures {
            count: 1,
            samples: vec!["a/first.rs: parse_panicked: injected parse panic".into()],
        }
    );
    assert_eq!(engine.take_parse_failures().count, 0, "taken once");
    assert_eq!(engine.pending().unwrap(), 0);
    // The panicked source got the plain blocks of an unmapped source, field
    // for field, except that the first one's kind is `unparsed`.
    let first = added_for(&observed.added, "a/first.rs");
    let plain = added_for(&observed.added, "a/first.txt");
    assert!(first.len() >= 2, "{first:?}");
    let kinds = |docs: &[serde_json::Value]| -> Vec<String> {
        docs.iter()
            .map(|doc| doc["kind"][0].as_str().unwrap().to_owned())
            .collect()
    };
    let mut expected = vec!["block".to_owned(); first.len()];
    expected[0] = "unparsed".to_owned();
    assert_eq!(kinds(&first), expected);
    assert_eq!(kinds(&plain), vec!["block".to_owned(); plain.len()]);
    let without = |docs: &[serde_json::Value]| -> Vec<serde_json::Value> {
        docs.iter()
            .map(|doc| {
                let mut doc = doc.clone();
                let fields = doc.as_object_mut().unwrap();
                for name in ["key", "key_hash", "path", "dir", "kind"] {
                    fields.remove(name);
                }
                doc
            })
            .collect()
    };
    assert_eq!(without(&first), without(&plain));
    // Every reader treats them as blocks.
    let hits = engine.search("first_unit", 5).unwrap().hits;
    let hit = hits
        .iter()
        .find(|hit| hit.path == "a/first.rs")
        .expect("its bytes stay searchable");
    assert_eq!(hit.label, "block");
    // Every other key of the page is indexed with its units.
    for (path, unit) in [("b/second.rs", "second_unit"), ("c/third.py", "third_unit")] {
        let docs = added_for(&observed.added, path);
        assert!(
            docs.iter()
                .any(|doc| doc["def_name"][0] == unit && doc.get("lang").is_some()),
            "{path}: {docs:?}"
        );
        assert_eq!(engine.search(unit, 5).unwrap().hits[0].path, path);
    }
    assert!(
        added_for(&observed.added, "d/fourth.md")
            .iter()
            .any(|doc| doc["kind"][0] == "section")
    );
    // Status names the source, also after a restart, until it is parsed again.
    let named = vec!["a/first.rs: parse_panicked: indexed as plain blocks until parsed again"];
    let status = engine.status().unwrap();
    assert_eq!(status.parse_failures, Some(1));
    assert_eq!(status.parse_failure_samples, named);
    drop(engine);
    let engine = Engine::open_existing(store.path()).unwrap();
    let status = engine.status().unwrap();
    assert_eq!(
        (status.parse_failures, status.parse_failure_samples),
        (Some(1), named.iter().map(|s| (*s).to_owned()).collect())
    );
    drop(engine);
    // `repair-index` rebuilds every source's documents: parsed again.
    Engine::repair_index(store.path(), &Control::unbounded()).unwrap();
    let mut engine = Engine::open_existing(store.path()).unwrap();
    assert_eq!(engine.status().unwrap().parse_failures, Some(0));
    // A change that panics again is named again; its next change parses.
    let watch = Arc::new(Watch::default());
    index_hooks::install(panicking(&watch));
    engine
        .replace_source("a/first.rs", "fn first_unit() {}\n")
        .unwrap();
    drain(&mut engine);
    index_hooks::clear();
    assert_eq!(engine.take_parse_failures().count, 1);
    assert_eq!(engine.status().unwrap().parse_failures, Some(1));
    engine
        .replace_source("a/first.rs", "fn first_unit() { }\n")
        .unwrap();
    drain(&mut engine);
    assert_eq!(engine.take_parse_failures().count, 0);
    let status = engine.status().unwrap();
    assert_eq!(
        (status.parse_failures, status.parse_failure_samples.len()),
        (Some(0), 0)
    );
    assert_eq!(
        engine.search("first_unit", 5).unwrap().hits[0].label,
        "fn first_unit"
    );
}

/// Without a parse panic nothing is named, also for sources legitimately
/// without units: over the 1 MiB parse bound, unmapped, of a mapped
/// language without units, and empty.
#[test]
fn status_names_no_source_without_a_parse_panic() {
    // Just over the 1 MiB parse bound; one-letter lines index cheaply.
    let big = "a\n".repeat(512 * KIB + 1);
    let fixture = tempfile::tempdir().unwrap();
    let (_store, mut engine) = setup(&fixture.path().join("ws"));
    for (path, body) in [
        ("big.rs", big.as_str()),
        ("notes.txt", "plain words\n"),
        ("conf.toml", "[package]\nname = \"x\"\n"),
        ("empty.rs", ""),
        ("unit.rs", "fn unit() {}\n"),
    ] {
        engine.replace_source(path, body).unwrap();
    }
    drain(&mut engine);
    assert_eq!(engine.take_parse_failures().count, 0);
    let status = engine.status().unwrap();
    assert_eq!(
        (status.parse_failures, status.parse_failure_samples.len()),
        (Some(0), 0)
    );
}

/// When every build panics, every panic is a named scan failure and every
/// key is acknowledged, but `status` names only the sources that have
/// `unparsed` documents: an empty or whitespace-only source has none.
#[test]
fn every_parse_panic_is_a_scan_failure_and_status_names_the_sources_with_text() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    for (path, body) in [
        ("a.rs", "fn a_unit() {}\n"),
        ("empty.rs", ""),
        ("blank.rs", " \n\n\t\n"),
        ("notes.txt", "plain words\n"),
    ] {
        std::fs::write(root.join(path), body).unwrap();
    }
    let watch = Arc::new(Watch::default());
    index_hooks::install(watch.hooks(8, |_, _| panic!("injected parse panic")));
    let report = engine.index(&root, &Control::unbounded()).unwrap();
    index_hooks::clear();
    drop(watch.take());
    assert_eq!(report.failures, 4);
    let mut samples = report.failure_samples.clone();
    samples.sort();
    assert_eq!(
        samples,
        [
            "a.rs: parse_panicked: injected parse panic",
            "blank.rs: parse_panicked: injected parse panic",
            "empty.rs: parse_panicked: injected parse panic",
            "notes.txt: parse_panicked: injected parse panic",
        ]
    );
    assert_eq!(report.reason_code, Some("scan_failures"));
    assert_eq!(report.pending_sources, 0);
    assert_eq!(engine.pending().unwrap(), 0);
    let status = engine.status().unwrap();
    assert_eq!(status.parse_failures, Some(2));
    assert_eq!(
        status.parse_failure_samples,
        [
            "a.rs: parse_panicked: indexed as plain blocks until parsed again",
            "notes.txt: parse_panicked: indexed as plain blocks until parsed again",
        ]
    );
}

/// Under the hand-out bound (64 MiB, overridden to 64 KiB here) a source's
/// bytes count until the writer has added its documents: built is not
/// consumed.
#[test]
fn the_hand_out_bound_counts_sources_built_but_not_yet_added() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    // 33 sources of 2 KiB: one more than the bound holds.
    let body = "a\n".repeat(KIB);
    for i in 0..33 {
        engine
            .replace_source(&format!("s{i:02}.txt"), &body)
            .unwrap();
    }
    let watch = Arc::new(Watch::default());
    // The earliest key's build is held until the hand-out must make room,
    // so the writer can add nothing in key order. The wait first lets the
    // 31 other handed-out sources finish building.
    watch.observed.lock().unwrap().built_before_release = 31;
    let hooks = watch.hooks(8, |watch, path| {
        if path == "s00.txt" {
            drop(watch.wait_until(|seen| seen.released));
        }
    });
    index_hooks::install(Hooks {
        handout_bytes: Some(64 * KIB),
        ..hooks
    });
    assert_eq!(engine.refresh(&Control::unbounded()).unwrap(), (33, 0));
    index_hooks::clear();
    let observed = watch.take();
    assert_eq!(observed.max_outstanding, 64 * KIB, "reached, never passed");
    assert_eq!(
        observed.waits,
        [(64 * KIB, 2 * KIB, 31)],
        "one wait, while 31 built sources were not yet added"
    );
    assert_eq!(engine.pending().unwrap(), 0);
    let mut paths: Vec<String> = observed
        .added
        .iter()
        .map(|json| serde_json::from_str::<serde_json::Value>(json).unwrap()["path"][0].to_string())
        .collect();
    paths.dedup();
    assert_eq!(paths.len(), 33);
}

/// One injected interruption of a refresh: where, what, the error code it
/// gives, and whether the page's search commit happened.
struct Interruption {
    label: &'static str,
    point: &'static str,
    skip: usize,
    action: fn() -> Action,
    code: &'static str,
    committed: bool,
}

/// Add, commit and reload failures and cancellation before and after the
/// search commit lose no pending key and acknowledge none early; a replay
/// (after a restart in one case) converges with exactly one live committed
/// document per source, counted in the index itself. A cancellation after
/// the page's last hand-out lets the page commit and leaves its keys
/// pending.
#[test]
fn index_failures_and_cancellation_acknowledge_no_pending_key_early() {
    fn case(
        label: &'static str,
        point: &'static str,
        skip: usize,
        action: fn() -> Action,
        code: &'static str,
        committed: bool,
    ) -> Interruption {
        Interruption {
            label,
            point,
            skip,
            action,
            code,
            committed,
        }
    }
    let cases = [
        case(
            "handout",
            index_points::INDEX_HANDOUT,
            1,
            || Action::Cancel,
            "cancelled",
            false,
        ),
        case(
            "handout_fail",
            index_points::INDEX_HANDOUT,
            2,
            || Action::Fail("read".into()),
            "internal",
            false,
        ),
        case(
            "add",
            index_points::INDEX_BEFORE_ADD,
            1,
            || Action::Fail("add".into()),
            "internal",
            false,
        ),
        case(
            "last_handout",
            index_points::INDEX_BEFORE_COMMIT,
            0,
            || Action::Cancel,
            "cancelled",
            true,
        ),
        case(
            "commit",
            index_points::INDEX_BEFORE_COMMIT,
            0,
            || Action::Fail("commit".into()),
            "internal",
            false,
        ),
        case(
            "reload",
            index_points::INDEX_BEFORE_RELOAD,
            0,
            || Action::Fail("reload".into()),
            "internal",
            true,
        ),
        case(
            "after_commit",
            names::INDEX_AFTER_SEARCH_COMMIT,
            0,
            || Action::Cancel,
            "cancelled",
            true,
        ),
    ];
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (store, mut engine) = setup(&root);
    let paths = ["a.rs", "b.rs", "c.rs"];
    for Interruption {
        label,
        point,
        skip,
        action,
        code: expected,
        committed,
    } in cases
    {
        let body = |version: &str, path: &str| {
            format!(
                "fn {label}_{version}_{}() {{}}\n",
                path.trim_end_matches(".rs")
            )
        };
        // Each case starts from its own indexed first version.
        for path in paths {
            engine.replace_source(path, &body("one", path)).unwrap();
        }
        drain(&mut engine);
        for path in paths {
            engine.replace_source(path, &body("two", path)).unwrap();
        }
        index_hooks::install(Hooks {
            threads: Some(8),
            handout_bytes: None,
            observer: None,
        });
        fault::arm(point, skip, action());
        let err = engine.refresh(&Control::unbounded()).unwrap_err();
        fault::disarm_all();
        index_hooks::clear();
        assert_eq!(code(&err), expected, "{label}");
        assert_eq!(engine.pending().unwrap(), 3, "{label}: no key acknowledged");
        // Exactly the committed version is live: the first one unless the
        // failure came after the commit.
        let live = if committed { "two" } else { "one" };
        for path in paths {
            assert_eq!(
                committed_documents(store.path(), path),
                [(digest(body(live, path).as_bytes()), 0)],
                "{label}: {path}"
            );
        }
        if committed {
            // The commit happened; a reload-failed reader sees it on reopen.
            drop(engine);
            engine = Engine::open_existing(store.path()).unwrap();
        }
        // Shared subtokens make other units lexical hits; count the exact one.
        let exact = |engine: &Engine, name: &str| {
            let outcome = engine.search(name, 5).unwrap();
            let exact = outcome
                .hits
                .iter()
                .filter(|hit| hit.text.contains(name))
                .count();
            (exact, outcome.stale_candidates)
        };
        assert_eq!(
            exact(&engine, &format!("{label}_two_a")).0,
            usize::from(committed),
            "{label}"
        );
        if label == "commit" {
            // A restart replays from the durable pending keys.
            drop(engine);
            engine = Engine::open_existing(store.path()).unwrap();
        }
        drain(&mut engine);
        assert_eq!(engine.pending().unwrap(), 0, "{label}");
        for path in paths {
            let unit = path.trim_end_matches(".rs");
            let two = exact(&engine, &format!("{label}_two_{unit}"));
            assert_eq!(two, (1, 0), "{label}: {path} once, nothing stale");
            let one = exact(&engine, &format!("{label}_one_{unit}"));
            assert_eq!(one, (0, 0), "{label}: {path}");
            // One live committed document: the replay's, no duplicate of
            // an earlier uncommitted add the same writer later committed.
            assert_eq!(
                committed_documents(store.path(), path),
                [(digest(body("two", path).as_bytes()), 0)],
                "{label}: {path}"
            );
        }
    }
}

const THREE: [(&str, &str); 3] = [
    ("a.rs", "fn a_unit() {}\n"),
    ("b.rs", "fn b_unit() {}\n"),
    ("c.rs", "fn c_unit() {}\n"),
];

/// Eight build threads whose observer runs `on_event` on every event.
fn observing(on_event: impl Fn(&Event<'_>) + Send + Sync + 'static) -> Hooks {
    Hooks {
        threads: Some(8),
        handout_bytes: None,
        observer: Some(Arc::new(on_event)),
    }
}

/// A build thread that panics outside its parse boundary is joined and
/// names the page's failure; the refresh returns instead of unwinding and
/// acknowledges nothing.
#[test]
fn a_build_thread_panic_outside_the_parse_boundary_fails_the_page_by_name() {
    let fixture = tempfile::tempdir().unwrap();
    let store = pending_store(fixture.path(), "ws", &THREE);
    let (refreshed, pending) = within(move || {
        let mut engine = Engine::open_existing(&store).unwrap();
        index_hooks::install(observing(|event| {
            if matches!(event, Event::Built("b.rs")) {
                panic!("injected build-thread panic");
            }
        }));
        // Returning at all means every build thread was joined.
        let refreshed = engine.refresh(&Control::unbounded());
        index_hooks::clear();
        (
            refreshed.map_err(|e| e.to_string()),
            engine.pending().unwrap(),
        )
    })
    .expect("the refresh returns instead of unwinding");
    let message = refreshed.unwrap_err();
    assert!(
        message.starts_with("internal: an index build thread panicked")
            && message.contains("injected build-thread panic"),
        "{message}"
    );
    assert_eq!(pending, 3, "no key acknowledged");
}

/// A panic of the writer itself closes the hand-out, so every build thread
/// finishes and the panic propagates instead of the scope waiting forever.
#[test]
fn a_writer_panic_closes_the_hand_out_and_propagates() {
    let fixture = tempfile::tempdir().unwrap();
    let store = pending_store(fixture.path(), "ws", &THREE);
    let (unwound, pending) = within(move || {
        let mut engine = Engine::open_existing(&store).unwrap();
        index_hooks::install(observing(|event| {
            if matches!(event, Event::HandedOut { path: "b.rs", .. }) {
                panic!("injected writer panic");
            }
        }));
        let refreshed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.refresh(&Control::unbounded())
        }));
        index_hooks::clear();
        (refreshed.is_err(), engine.pending().unwrap())
    })
    .expect("the bounded run finishes");
    assert!(unwound, "the writer's panic propagates");
    assert_eq!(pending, 3, "no key acknowledged");
}

/// A recorded length past the 2 MiB bound is `corrupt_source` before any
/// hand-out arithmetic, never an overflow panic, also while an earlier
/// source is still handed out.
#[test]
fn a_corrupt_recorded_length_is_named_before_the_bound_arithmetic() {
    let fixture = tempfile::tempdir().unwrap();
    let store = pending_store(fixture.path(), "ws", &THREE[..2]);
    testkit::write_store(&store, |tx| {
        use redb::ReadableTable;
        let mut sources = tx
            .open_table(redb::TableDefinition::<&str, &str>::new("sources"))
            .unwrap();
        let raw = sources.get("b.rs").unwrap().unwrap().value().to_owned();
        let mut meta: serde_json::Value = serde_json::from_str(&raw).unwrap();
        meta["bytes"] = serde_json::json!(usize::MAX);
        sources.insert("b.rs", meta.to_string().as_str()).unwrap();
    });
    let (refreshed, pending) = within(move || {
        let mut engine = Engine::open_existing(&store).unwrap();
        // a.rs's build lingers, so it is normally still outstanding when
        // b.rs's length is read.
        index_hooks::install(observing(|event| {
            if matches!(event, Event::Build("a.rs")) {
                std::thread::sleep(Duration::from_millis(100));
            }
        }));
        let refreshed = engine.refresh(&Control::unbounded());
        index_hooks::clear();
        (
            refreshed.map_err(|e| e.to_string()),
            engine.pending().unwrap(),
        )
    })
    .expect("named, not a panic");
    let message = refreshed.unwrap_err();
    assert!(
        message.starts_with("corrupt_source:")
            && message.contains("b.rs: recorded length exceeds the 2 MiB bound"),
        "{message}"
    );
    assert_eq!(pending, 2, "no key acknowledged");
}

/// A parse panic whose documents were committed is a named scan failure of
/// the index run even when cancellation stops it before the pending clear;
/// the pending key stays.
#[test]
fn a_committed_parse_panic_is_named_although_the_run_is_cancelled_after_commit() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let (_store, mut engine) = setup(&root);
    std::fs::write(root.join("a.rs"), "fn panicked_unit() {}\n").unwrap();
    std::fs::write(root.join("b.rs"), "fn quiet_unit() {}\n").unwrap();
    let watch = Arc::new(Watch::default());
    index_hooks::install(watch.hooks(8, |_, path| {
        if path == "a.rs" {
            panic!("injected parse panic");
        }
    }));
    fault::arm(names::INDEX_AFTER_SEARCH_COMMIT, 0, Action::Cancel);
    let report = engine.index(&root, &Control::unbounded()).unwrap();
    fault::disarm_all();
    index_hooks::clear();
    drop(watch.take());
    assert!(report.partial);
    assert_eq!(report.reason_code, Some("cancelled"));
    assert_eq!(report.failures, 1);
    assert_eq!(
        report.failure_samples,
        ["a.rs: parse_panicked: injected parse panic"]
    );
    assert_eq!(report.pending_sources, 2, "the page was not acknowledged");
    assert_eq!(engine.status().unwrap().parse_failures, Some(1));
}
