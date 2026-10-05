//! Recovery verification with REAL child-process exits at NAMED boundaries.
//!
//! The test binary re-executes itself. The child seeds a store, arms one named
//! fault point with `Action::Abort` and performs the operation, so it dies by
//! SIGABRT exactly at that boundary. The parent then inspects the durable state
//! that identifies the boundary, reopens, retries, and requires exact source
//! hashes, graph rows and feedback records. These prove application ordering,
//! not hardware power-loss immunity.
use context_foundry::fault::{self, Action, names};
use context_foundry::graph::{Edge, Endpoint, GraphBundle};
use context_foundry::laya::{Feedback, Strategy as LayaStrategy};
use context_foundry::testkit;
use context_foundry::testkit::{
    CORRUPT_INDEX_BYTES, KEPT_BODY, Snapshot, corrupt_search_index, craft_v1_store, knowledge,
    quarantine_dirs, schema_marker, snapshot,
};
use context_foundry::{Control, Engine, digest};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

type Owned = BTreeMap<String, Vec<(String, String)>>;

fn owned(snapshot: &Snapshot) -> Owned {
    snapshot
        .iter()
        .map(|(name, rows)| ((*name).to_owned(), rows.clone()))
        .collect()
}

fn work_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("FOUNDRY_TEST_WORK").unwrap())
}

const V1: &str = "fn child_version_one() {}\n";
const V2: &str = "fn child_version_two() {}\n";
// The delivery units of V1 and V2: the function without the final LF.
const V1_UNIT: &str = "fn child_version_one() {}";
const V2_UNIT: &str = "fn child_version_two() {}";
const PROBE: &str = "fn probe_a() { probe_b(); }\n";
const PROBE_B: &str = "fn probe_b() {}\n";

fn endpoint(path: &str, body: &str) -> Endpoint {
    Endpoint {
        path: path.into(),
        line: 1,
        symbol: path.into(),
        hash: digest(body.as_bytes()),
    }
}

/// A store holding `n` filler sources plus a graph edge, feedback and one
/// versioned source, derived index drained; engine closed. Returns the exact
/// snapshot, also written for the parent to compare against.
fn seed(work: &Path, filler: u32) -> Snapshot {
    let store = work.join("store");
    let ws = work.join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    {
        let mut engine = Engine::initialize(&store, &ws).unwrap();
        engine.replace_source("a.rs", V1).unwrap();
        engine.replace_source("probe_a.rs", PROBE).unwrap();
        engine.replace_source("probe_b.rs", PROBE_B).unwrap();
        for i in 0..filler {
            engine
                .replace_source(&format!("s{i:03}.rs"), &format!("body {i}\n"))
                .unwrap();
        }
        engine
            .import_graph(&GraphBundle {
                provider: "fixture".into(),
                revision: "r1".into(),
                edges: vec![Edge {
                    from: endpoint("probe_a.rs", PROBE),
                    to: endpoint("probe_b.rs", PROBE_B),
                    kind: "calls".into(),
                    evidence: "manual".into(),
                }],
            })
            .unwrap();
        engine
            .record_feedback(&Feedback {
                task_id: "t1".into(),
                query: "who calls probe_b".into(),
                correct_strategy: LayaStrategy::Graph,
                label_source: "operator".into(),
                allow_training: true,
            })
            .unwrap();
        engine.refresh(&Control::unbounded()).unwrap();
    }
    let before = snapshot(&store);
    std::fs::write(
        work.join("before.json"),
        serde_json::to_string(&before).unwrap(),
    )
    .unwrap();
    before
}

fn before_of(work: &Path) -> Owned {
    serde_json::from_str(&std::fs::read_to_string(work.join("before.json")).unwrap()).unwrap()
}

fn child_fault(stage: &str) -> ! {
    let work = work_dir();
    let store = work.join("store");
    match stage {
        "before_source_commit" | "after_source_commit" | "after_search_commit" => {
            seed(&work, 0);
            let mut engine = Engine::open_existing(&store).unwrap();
            match stage {
                "before_source_commit" => {
                    fault::arm(names::SOURCE_BEFORE_COMMIT, 0, Action::Abort);
                    let _ = engine.replace_source("a.rs", V2);
                }
                "after_source_commit" => {
                    fault::arm(names::SOURCE_AFTER_COMMIT, 0, Action::Abort);
                    let _ = engine.replace_source("a.rs", V2);
                }
                _ => {
                    engine.replace_source("a.rs", V2).unwrap();
                    fault::arm(names::INDEX_AFTER_SEARCH_COMMIT, 0, Action::Abort);
                    let _ = engine.refresh(&Control::unbounded());
                }
            }
        }
        "upgrade_before_commit" | "upgrade_after_commit" => {
            craft_v1_store(&store, Some(&work.join("ws")));
            std::fs::write(
                work.join("before.json"),
                serde_json::to_string(&snapshot(&store)).unwrap(),
            )
            .unwrap();
            let point = if stage == "upgrade_before_commit" {
                names::UPGRADE_BEFORE_COMMIT
            } else {
                names::UPGRADE_AFTER_COMMIT
            };
            fault::arm(point, 0, Action::Abort);
            let _ = Engine::upgrade_store(&store, 6, &Control::unbounded());
        }
        repair if repair.starts_with("repair_") => {
            seed(&work, 300);
            corrupt_search_index(&store);
            std::fs::write(
                work.join("before.json"),
                serde_json::to_string(&snapshot(&store)).unwrap(),
            )
            .unwrap();
            let (point, skip) = match repair {
                "repair_after_marker" => (names::REPAIR_AFTER_MARKER, 0),
                "repair_before_rename" => (names::REPAIR_BEFORE_QUARANTINE_RENAME, 0),
                "repair_after_rename" => (names::REPAIR_AFTER_QUARANTINE_RENAME, 0),
                // The second of three 128-source pages: two pages committed.
                "repair_enqueue_page" => (names::REPAIR_AFTER_ENQUEUE_PAGE, 1),
                "repair_before_replacement_index" => (names::REPAIR_BEFORE_REPLACEMENT_INDEX, 0),
                "repair_after_replacement_index" => (names::REPAIR_AFTER_REPLACEMENT_INDEX, 0),
                // The replacement's second search commit: one 128-source batch
                // committed and cleared, the next committed but not cleared.
                "repair_after_index_commit" => (names::INDEX_AFTER_SEARCH_COMMIT, 1),
                "repair_before_marker_clear" => (names::REPAIR_BEFORE_MARKER_CLEAR, 0),
                "repair_after_schema_publication" => (names::REPAIR_AFTER_SCHEMA_PUBLICATION, 0),
                other => panic!("unknown repair stage {other}"),
            };
            fault::arm(point, skip, Action::Abort);
            let _ = Engine::repair_index(&store, &Control::unbounded());
        }
        other => panic!("unknown fault stage {other}"),
    }
    // Reaching here means the named boundary was never hit.
    eprintln!("fault stage {stage} did not reach its boundary");
    std::process::exit(7);
}

/// Run `stage` in a child and require death by SIGABRT at its boundary.
fn crash_at(stage: &str) -> tempfile::TempDir {
    use std::os::unix::process::ExitStatusExt;
    let work = tempfile::tempdir().unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .env("FOUNDRY_TEST_FAULT_STAGE", stage)
        .env("FOUNDRY_TEST_WORK", work.path())
        .status()
        .unwrap();
    assert_eq!(
        status.signal(),
        Some(libc::SIGABRT),
        "stage {stage} must die by SIGABRT at its named boundary, got {status:?}"
    );
    work
}

fn knowledge_owned(store: &Path) -> Owned {
    owned(&knowledge(&snapshot(store)))
}

fn knowledge_of(before: &Owned) -> Owned {
    before
        .iter()
        .filter(|(name, _)| context_foundry::testkit::KNOWLEDGE_TABLES.contains(&name.as_str()))
        .map(|(name, rows)| (name.clone(), rows.clone()))
        .collect()
}

fn exit_before_source_commit_leaves_the_prior_state_exactly() {
    let work = crash_at("before_source_commit");
    let store = work.path().join("store");
    // Not one row of any table changed: bytes, revision, pending, index state.
    assert_eq!(owned(&snapshot(&store)), before_of(work.path()));
    let engine = Engine::open_existing(&store).unwrap();
    let hit = engine
        .search("child_version_one", 5)
        .unwrap()
        .hits
        .remove(0);
    assert_eq!(hit.text, V1_UNIT);
    assert!(
        engine
            .search("child_version_two", 5)
            .unwrap()
            .hits
            .is_empty()
    );
}

fn exit_after_source_commit_keeps_new_bytes_and_rejects_the_old_hit() {
    let work = crash_at("after_source_commit");
    let store = work.path().join("store");
    let before = before_of(work.path());
    let mut engine = Engine::open_existing(&store).unwrap();
    let source = engine.source("a.rs").unwrap().unwrap();
    assert_eq!(source.hash, digest(V2.as_bytes()));
    assert_eq!(engine.pending().unwrap(), 1);
    // Old indexed version is ineligible; the new one is not yet searchable.
    let old = engine.search("child_version_one", 5).unwrap();
    assert!(old.hits.is_empty() && old.stale_candidates == 1);
    assert!(
        engine
            .search("child_version_two", 5)
            .unwrap()
            .hits
            .is_empty()
    );
    engine.refresh(&Control::unbounded()).unwrap();
    assert_eq!(
        engine.search("child_version_two", 5).unwrap().hits[0].text,
        V2_UNIT
    );
    drop(engine);
    // Graph rows and feedback records are exactly what was committed before.
    let now = knowledge_owned(&store);
    for table in ["feedback", "provider_bundles", "edges_out", "edges_in"] {
        assert_eq!(now[table], before[table], "{table}");
    }
}

fn exit_between_search_commit_and_pending_clear_replays_idempotently() {
    let work = crash_at("after_search_commit");
    let store = work.path().join("store");
    let mut engine = Engine::open_existing(&store).unwrap();
    assert_eq!(engine.pending().unwrap(), 1, "pending was not yet cleared");
    // The search commit DID happen: the current version is searchable before
    // any replay. A boundary moved before the commit could not pass this.
    let hits = engine.search("child_version_two", 5).unwrap();
    assert_eq!(hits.hits.len(), 1);
    assert_eq!(hits.hits[0].text, V2_UNIT);
    engine.refresh(&Control::unbounded()).unwrap();
    assert_eq!(engine.pending().unwrap(), 0);
    assert_eq!(engine.search("child_version_two", 5).unwrap().hits.len(), 1);
    assert!(
        engine
            .search("child_version_one", 5)
            .unwrap()
            .hits
            .is_empty()
    );
    let hit = engine
        .search("child_version_two", 5)
        .unwrap()
        .hits
        .remove(0);
    assert_eq!(
        engine
            .retrieve(&hit.handle.to_v2(), None, 4096)
            .unwrap()
            .span,
        V2_UNIT.as_bytes()
    );
}

/// After every repair abort: sources, chunks, graph rows and feedback are
/// exactly the pre-repair rows; a rerun completes with pending 0, marker
/// clear and exactly one retained quarantine holding the original bytes.
fn finish_repair_and_verify(work: &Path) {
    let store = work.join("store");
    let before = knowledge_of(&before_of(work));
    assert_eq!(
        knowledge_owned(&store),
        before,
        "knowledge changed by the fault"
    );
    let report = Engine::repair_index(&store, &Control::unbounded()).unwrap();
    assert!(report.repaired);
    assert_eq!(
        quarantine_dirs(&store).len(),
        1,
        "exactly one retained quarantine"
    );
    assert_eq!(
        std::fs::read(quarantine_dirs(&store)[0].join("meta.json")).unwrap(),
        CORRUPT_INDEX_BYTES
    );
    let engine = Engine::open_existing(&store).unwrap();
    assert_converged(&engine);
    drop(engine);
    assert_eq!(knowledge_owned(&store), before, "repair changed knowledge");
}

/// A converged store serves the rebuilt index with no stale documents: every
/// candidate in a full 256-document window matches its current source.
fn assert_converged(engine: &Engine) {
    let status = engine.status().unwrap();
    assert_eq!(
        (status.index_state.as_str(), status.pending_count),
        ("ready", 0)
    );
    let one = engine.search("child_version_one", 5).unwrap();
    assert_eq!((one.hits.len(), one.stale_candidates), (1, 0));
    let body = engine.search("body", 64).unwrap();
    assert_eq!((body.hits.len(), body.stale_candidates), (64, 0));
    assert!(body.candidate_limit_reached, "the full window was examined");
}

fn repair_state(work: &Path) -> (Engine, std::path::PathBuf) {
    let store = work.join("store");
    (Engine::open_existing(&store).unwrap(), store)
}

fn exit_after_marker_before_any_move() {
    let work = crash_at("repair_after_marker");
    let (engine, store) = repair_state(work.path());
    assert_eq!(engine.status().unwrap().index_state, "repair_required");
    drop(engine);
    assert!(quarantine_dirs(&store).is_empty());
    assert_eq!(
        std::fs::read(store.join("search").join("meta.json")).unwrap(),
        CORRUPT_INDEX_BYTES
    );
    finish_repair_and_verify(work.path());
}

fn exit_after_quarantine_intent_before_rename() {
    let work = crash_at("repair_before_rename");
    let (engine, store) = repair_state(work.path());
    assert_eq!(engine.status().unwrap().index_state, "repair_required");
    drop(engine);
    // Intent recorded, nothing moved yet: the original is still in place.
    assert!(quarantine_dirs(&store).is_empty());
    assert_eq!(
        std::fs::read(store.join("search").join("meta.json")).unwrap(),
        CORRUPT_INDEX_BYTES
    );
    finish_repair_and_verify(work.path());
}

fn exit_between_quarantine_rename_and_publication() {
    let work = crash_at("repair_after_rename");
    let (engine, store) = repair_state(work.path());
    assert_eq!(engine.status().unwrap().index_state, "repair_required");
    drop(engine);
    // The rename happened but original_handled was never published: the
    // original lives only in the quarantine and `search/` does not exist.
    assert_eq!(quarantine_dirs(&store).len(), 1);
    assert!(!store.join("search").exists());
    finish_repair_and_verify(work.path());
}

fn exit_after_two_enqueue_pages_commit() {
    let work = crash_at("repair_enqueue_page");
    let (engine, store) = repair_state(work.path());
    let status = engine.status().unwrap();
    // 303 sources enqueue in pages of 128: two pages are durably committed.
    assert_eq!(status.index_state, "repair_required");
    assert_eq!(
        status.pending_count, 256,
        "a partial enqueue page set committed"
    );
    drop(engine);
    assert_eq!(quarantine_dirs(&store).len(), 1);
    assert!(store.join("search").join("rebuild_id").is_file());
    finish_repair_and_verify(work.path());
}

fn exit_after_drain_before_marker_clear() {
    let work = crash_at("repair_before_marker_clear");
    let (engine, store) = repair_state(work.path());
    let status = engine.status().unwrap();
    assert_eq!(status.index_state, "repair_required", "marker still set");
    assert_eq!(status.pending_count, 0, "the drain had finished");
    drop(engine);
    assert_eq!(quarantine_dirs(&store).len(), 1);
    finish_repair_and_verify(work.path());
}

fn exit_before_replacement_index_creation() {
    let work = crash_at("repair_before_replacement_index");
    let (engine, store) = repair_state(work.path());
    let status = engine.status().unwrap();
    assert_eq!(status.index_state, "repair_required");
    assert_eq!(status.pending_count, 303, "every source enqueued");
    drop(engine);
    assert_eq!(quarantine_dirs(&store).len(), 1);
    // The identified replacement directory exists without an index.
    assert!(store.join("search").join("rebuild_id").is_file());
    assert!(!store.join("search").join("meta.json").exists());
    finish_repair_and_verify(work.path());
}

fn exit_after_replacement_index_creation() {
    let work = crash_at("repair_after_replacement_index");
    let (engine, store) = repair_state(work.path());
    let status = engine.status().unwrap();
    assert_eq!(status.index_state, "repair_required");
    assert_eq!(status.pending_count, 303);
    drop(engine);
    assert_eq!(quarantine_dirs(&store).len(), 1);
    assert!(store.join("search").join("meta.json").is_file());
    finish_repair_and_verify(work.path());
}

fn exit_after_replacement_index_commit() {
    let work = crash_at("repair_after_index_commit");
    let (engine, store) = repair_state(work.path());
    let status = engine.status().unwrap();
    // The partly built replacement is never served while the marker is set.
    assert_eq!(status.index_state, "repair_required");
    assert_eq!(status.pending_count, 303 - 128);
    assert_eq!(
        engine.search("body", 5).unwrap_err().code(),
        "repair_required"
    );
    drop(engine);
    assert_eq!(quarantine_dirs(&store).len(), 1);
    finish_repair_and_verify(work.path());
}

fn exit_after_schema_publication() {
    let work = crash_at("repair_after_schema_publication");
    let store = work.path().join("store");
    // Publication committed: the reopened store is converged without a
    // rerun, holding exactly the one original quarantine.
    let engine = Engine::open_existing(&store).unwrap();
    assert_converged(&engine);
    drop(engine);
    let quarantines = quarantine_dirs(&store);
    assert_eq!(quarantines.len(), 1);
    assert_eq!(
        std::fs::read(quarantines[0].join("meta.json")).unwrap(),
        CORRUPT_INDEX_BYTES
    );
    assert_eq!(
        knowledge_owned(&store),
        knowledge_of(&before_of(work.path()))
    );
}

fn exit_during_upgrade_transaction_leaves_wholly_v1() {
    let work = crash_at("upgrade_before_commit");
    let store = work.path().join("store");
    assert_eq!(schema_marker(&store), "1");
    // Row for row the v1 store: records, feedback, bookkeeping.
    assert_eq!(owned(&snapshot(&store)), before_of(work.path()));
    assert_eq!(
        Engine::open_existing(&store).unwrap_err().code(),
        "upgrade_required"
    );
    Engine::upgrade_store(&store, 6, &Control::unbounded()).unwrap();
    assert_eq!(schema_marker(&store), "6");
    let engine = Engine::open_existing(&store).unwrap();
    assert_eq!(engine.status().unwrap().source_count, 1);
    assert_eq!(
        engine.source("kept.rs").unwrap().unwrap().bytes,
        KEPT_BODY.len()
    );
    assert_eq!(engine.training_examples().unwrap().len(), 1);
    drop(engine);
    assert_eq!(
        knowledge_owned(&store),
        knowledge_of(&before_of(work.path()))
    );
}

fn exit_after_upgrade_commit_is_wholly_v2() {
    let work = crash_at("upgrade_after_commit");
    let store = work.path().join("store");
    assert_eq!(schema_marker(&store), "6");
    let engine = Engine::open_existing(&store).unwrap();
    let status = engine.status().unwrap();
    assert_eq!(
        (status.schema, status.source_revision, status.source_count),
        (6, 0, 1)
    );
    assert_eq!(engine.training_examples().unwrap().len(), 1);
    drop(engine);
    assert_eq!(
        knowledge_owned(&store),
        knowledge_of(&before_of(work.path()))
    );
}

fn three_interrupted_retries_retain_exactly_one_original_quarantine() {
    let work = tempfile::tempdir().unwrap();
    seed(work.path(), 300);
    let store = work.path().join("store");
    corrupt_search_index(&store);
    let before = knowledge_owned(&store);
    for attempt in 0..3 {
        // Each retry interrupts one page later than the last.
        fault::arm(
            names::REPAIR_AFTER_ENQUEUE_PAGE,
            attempt,
            Action::Fail("injected interruption".into()),
        );
        let err = Engine::repair_index(&store, &Control::unbounded()).unwrap_err();
        fault::disarm_all();
        assert_eq!(err.code(), "internal", "attempt {attempt}");
        assert_eq!(quarantine_dirs(&store).len(), 1, "attempt {attempt}");
        assert_eq!(
            std::fs::read(quarantine_dirs(&store)[0].join("meta.json")).unwrap(),
            CORRUPT_INDEX_BYTES
        );
    }
    finish_repair_and_verify(work.path());
    assert_eq!(knowledge_owned(&store), before);
}

fn global_arming_fires_on_worker_threads_for_spawned_processes() {
    use context_foundry::fault::{self, GlobalAction, names};
    use std::time::{Duration, Instant};
    let mut fx = testkit::new_fixture();
    fx.add(&[("d.rs", "fn delay_probe() {}\n")]);
    let handle = fx
        .engine
        .search("delay_probe", 5)
        .unwrap()
        .hits
        .remove(0)
        .handle;
    // Env-armed faults are process-global: a server arming on its main thread
    // must stall engine calls running on any worker thread.
    fault::arm_global(
        names::RETRIEVE_BEFORE_FINAL_READ,
        0,
        GlobalAction::Delay(Duration::from_millis(150)),
    );
    let handle_v2 = handle.to_v2();
    let (_dir, store, _root) = fx.close();
    let started = Instant::now();
    let worker = std::thread::spawn(move || {
        let engine = Engine::open_existing(&store).unwrap();
        engine.retrieve(&handle_v2, None, 2048).unwrap().span
    });
    assert_eq!(worker.join().unwrap(), b"fn delay_probe() {}".to_vec());
    assert!(started.elapsed() >= Duration::from_millis(150));
    assert!(fault::reached(names::RETRIEVE_BEFORE_FINAL_READ) >= 1);
    fault::disarm_all();
}

fn main() {
    if let Ok(stage) = std::env::var("FOUNDRY_TEST_FAULT_STAGE") {
        child_fault(&stage);
    }
    let tests: Vec<(&str, fn())> = vec![
        (
            "exit_before_source_commit",
            exit_before_source_commit_leaves_the_prior_state_exactly,
        ),
        (
            "exit_after_source_commit",
            exit_after_source_commit_keeps_new_bytes_and_rejects_the_old_hit,
        ),
        (
            "exit_between_search_commit_and_pending_clear",
            exit_between_search_commit_and_pending_clear_replays_idempotently,
        ),
        (
            "exit_after_marker_before_any_move",
            exit_after_marker_before_any_move,
        ),
        (
            "exit_after_quarantine_intent_before_rename",
            exit_after_quarantine_intent_before_rename,
        ),
        (
            "exit_between_quarantine_rename_and_publication",
            exit_between_quarantine_rename_and_publication,
        ),
        (
            "exit_after_two_enqueue_pages_commit",
            exit_after_two_enqueue_pages_commit,
        ),
        (
            "exit_after_drain_before_marker_clear",
            exit_after_drain_before_marker_clear,
        ),
        (
            "exit_before_replacement_index_creation",
            exit_before_replacement_index_creation,
        ),
        (
            "exit_after_replacement_index_creation",
            exit_after_replacement_index_creation,
        ),
        (
            "exit_after_replacement_index_commit",
            exit_after_replacement_index_commit,
        ),
        (
            "exit_after_schema_publication",
            exit_after_schema_publication,
        ),
        (
            "exit_during_upgrade_transaction_leaves_wholly_v1",
            exit_during_upgrade_transaction_leaves_wholly_v1,
        ),
        (
            "exit_after_upgrade_commit_is_wholly_v2",
            exit_after_upgrade_commit_is_wholly_v2,
        ),
        (
            "three_interrupted_retries_retain_one_original_quarantine",
            three_interrupted_retries_retain_exactly_one_original_quarantine,
        ),
        (
            "global_arming_fires_on_worker_threads",
            global_arming_fires_on_worker_threads_for_spawned_processes,
        ),
    ];
    let mut failed = 0usize;
    for (name, test) in tests {
        print!("test {name} ... ");
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(test)) {
            Ok(()) => println!("ok"),
            Err(_) => {
                failed += 1;
                println!("FAILED");
            }
        }
    }
    if failed > 0 {
        println!("{failed} recovery test(s) failed");
        std::process::exit(1);
    }
    println!("all recovery tests passed");
}
