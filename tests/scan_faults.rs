//! Reconciliation under named faults: root/file/ancestor/FIFO replacement,
//! deterministic unreadable-file injection, interruption, and page bounds.
use context_foundry::fault::{self, Action, Ctx, names};
use context_foundry::laya::{Feedback, Strategy as LayaStrategy};
use context_foundry::testkit::{self, Fixture, knowledge, new_fixture, snapshot};
use context_foundry::{Control, Engine, FoundryError, digest};
use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn feedback(task: &str) -> Feedback {
    Feedback {
        task_id: task.into(),
        query: format!("query for {task}"),
        correct_strategy: LayaStrategy::Graph,
        label_source: "operator".into(),
        allow_training: true,
    }
}

/// A store that already accepted `pkg/file.rs` ("inside v1"); the tree then
/// holds v2, and an outside directory holds a marker that must never be read.
struct Race {
    fx: Fixture,
    outside: PathBuf,
    v1_hash: String,
    feedback_rows: Vec<serde_json::Value>,
}

fn race() -> Race {
    let mut fx = new_fixture();
    let outside = fx.dir.path().join("outside");
    fs::create_dir_all(fx.root.join("pkg")).unwrap();
    fs::create_dir_all(outside.join("pkg")).unwrap();
    fs::write(outside.join("file.rs"), "OUTSIDE_MARKER_ZZZ\n").unwrap();
    fs::write(outside.join("pkg").join("file.rs"), "OUTSIDE_MARKER_ZZZ\n").unwrap();
    fs::write(fx.root.join("pkg").join("file.rs"), "inside v1\n").unwrap();
    let root = fx.root.clone();
    let first = fx.engine.index(&root, &Control::unbounded()).unwrap();
    assert_eq!(first.changed, 1);
    let v1_hash = fx.engine.source("pkg/file.rs").unwrap().unwrap().hash;
    fx.engine.record_feedback(&feedback("t1")).unwrap();
    let feedback_rows = fx.engine.training_examples().unwrap();
    fs::write(root.join("pkg").join("file.rs"), "inside v2\n").unwrap();
    Race {
        fx,
        outside,
        v1_hash,
        feedback_rows,
    }
}

impl Race {
    fn assert_preserved(&self) {
        assert_eq!(
            self.fx.engine.source("pkg/file.rs").unwrap().unwrap().hash,
            self.v1_hash,
            "the previous accepted record must survive"
        );
        assert!(
            self.fx
                .engine
                .search("OUTSIDE_MARKER_ZZZ", 5)
                .unwrap()
                .hits
                .is_empty()
        );
        assert_eq!(
            self.fx.engine.training_examples().unwrap(),
            self.feedback_rows
        );
    }
}

fn assert_failed_unsafe(race: &Race, report: &context_foundry::IndexReport) {
    assert_eq!(report.failures, 1, "{report:?}");
    assert!(
        report.failure_samples[0].contains("unsafe_source_path"),
        "{:?}",
        report.failure_samples
    );
    assert!(report.partial && report.deletions_deferred && !report.scan_complete);
    assert_eq!(report.changed, 0);
    race.assert_preserved();
}

#[test]
fn file_replaced_by_outside_symlink_between_enumeration_and_open_is_refused() {
    let mut race = race();
    let (root, outside) = (race.fx.root.clone(), race.outside.clone());
    fault::arm(
        names::SCAN_BEFORE_OPEN,
        0,
        Action::Call(Box::new(move |ctx| {
            if ctx.detail == "pkg/file.rs" {
                let victim = root.join("pkg").join("file.rs");
                fs::remove_file(&victim).unwrap();
                symlink(outside.join("file.rs"), victim).unwrap();
            }
        })),
    );
    let root = race.fx.root.clone();
    let report = race.fx.engine.index(&root, &Control::unbounded()).unwrap();
    fault::disarm_all();
    assert_failed_unsafe(&race, &report);
}

#[test]
fn ancestor_replaced_by_outside_symlink_between_enumeration_and_open_is_refused() {
    let mut race = race();
    let (root, outside) = (race.fx.root.clone(), race.outside.clone());
    let moved = race.fx.dir.path().join("pkg.moved");
    fault::arm(
        names::SCAN_BEFORE_OPEN,
        0,
        Action::Call(Box::new(move |ctx| {
            if ctx.detail == "pkg/file.rs" {
                fs::rename(root.join("pkg"), &moved).unwrap();
                symlink(outside.join("pkg"), root.join("pkg")).unwrap();
            }
        })),
    );
    let root = race.fx.root.clone();
    let report = race.fx.engine.index(&root, &Control::unbounded()).unwrap();
    fault::disarm_all();
    assert_failed_unsafe(&race, &report);
}

#[test]
fn file_replaced_by_fifo_is_refused_without_blocking() {
    let (tx, rx) = std::sync::mpsc::channel();
    // Faults are thread-local: arm and run on the worker thread. A regression
    // blocks in open(2) forever, which the timeout below turns into a failure.
    std::thread::spawn(move || {
        let mut race = race();
        let root = race.fx.root.clone();
        fault::arm(
            names::SCAN_BEFORE_OPEN,
            0,
            Action::Call(Box::new(move |ctx| {
                if ctx.detail == "pkg/file.rs" {
                    let victim = root.join("pkg").join("file.rs");
                    fs::remove_file(&victim).unwrap();
                    let path = std::ffi::CString::new(victim.to_str().unwrap()).unwrap();
                    // SAFETY: mkfifo(3) on a NUL-terminated path.
                    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o644) }, 0);
                }
            })),
        );
        let root = race.fx.root.clone();
        let report = race.fx.engine.index(&root, &Control::unbounded()).unwrap();
        fault::disarm_all();
        assert_failed_unsafe(&race, &report);
        let _ = tx.send(());
    });
    rx.recv_timeout(std::time::Duration::from_secs(20))
        .expect("a FIFO replacement blocked the scan instead of being refused");
}

/// A bound, indexed store with its engine closed, plus an outside directory
/// holding a marker that must never be read.
struct RootRace {
    _dir: testkit::Scratch,
    store: PathBuf,
    root: PathBuf,
    outside: PathBuf,
    before: testkit::Snapshot,
}

fn root_race() -> RootRace {
    let mut fx = new_fixture();
    let outside = fx.dir.path().join("elsewhere");
    fs::create_dir(&outside).unwrap();
    fs::write(
        outside.join("secret.rs"),
        "OUTSIDE_MARKER_ZZZ fn leaked() {}\n",
    )
    .unwrap();
    fs::write(fx.root.join("inside.rs"), "fn inside_probe() {}\n").unwrap();
    let root = fx.root.clone();
    fx.engine.index(&root, &Control::unbounded()).unwrap();
    fx.engine.record_feedback(&feedback("t1")).unwrap();
    let (dir, store, root) = fx.close();
    let before = snapshot(&store);
    RootRace {
        _dir: dir,
        store,
        root,
        outside,
        before,
    }
}

/// Replace `target` with a symlink to `outside`, keeping the original aside.
fn swap_for_symlink(target: &Path, outside: &Path) {
    let moved = target.with_file_name(format!(
        "{}.moved",
        target.file_name().unwrap().to_string_lossy()
    ));
    fs::rename(target, moved).unwrap();
    symlink(outside, target).unwrap();
}

impl RootRace {
    /// The refused scan changed nothing at all (scan id, scan status, source
    /// rows, revision, pending) and read no outside bytes.
    fn assert_refused_without_mutation(&self, error: FoundryError) {
        assert_eq!(error.code(), "unsafe_source_path", "{error:?}");
        assert_eq!(snapshot(&self.store), self.before);
        let engine = Engine::open_existing(&self.store).unwrap();
        assert!(
            engine
                .search("OUTSIDE_MARKER_ZZZ", 5)
                .unwrap()
                .hits
                .is_empty()
        );
        assert!(engine.source("secret.rs").unwrap().is_none());
    }
}

#[test]
fn root_replaced_by_symlink_before_acquisition_is_refused_before_any_mutation() {
    let race = root_race();
    let mut engine = Engine::open_existing(&race.store).unwrap();
    let outside = race.outside.clone();
    fault::arm(
        names::SCAN_BEFORE_ROOT_OPEN,
        0,
        Action::Call(Box::new(move |ctx| {
            swap_for_symlink(Path::new(ctx.detail), &outside);
        })),
    );
    let error = engine.index(&race.root, &Control::unbounded()).unwrap_err();
    fault::disarm_all();
    drop(engine);
    race.assert_refused_without_mutation(error);
}

#[test]
fn root_replaced_after_acquisition_is_refused_before_any_mutation() {
    for variant in ["symlink", "other directory"] {
        let race = root_race();
        let mut engine = Engine::open_existing(&race.store).unwrap();
        let outside = race.outside.clone();
        fault::arm(
            names::SCAN_AFTER_ROOT_OPEN,
            0,
            Action::Call(Box::new(move |ctx| {
                let target = Path::new(ctx.detail);
                if variant == "symlink" {
                    swap_for_symlink(target, &outside);
                } else {
                    fs::rename(target, target.with_file_name("ws.moved")).unwrap();
                    fs::create_dir(target).unwrap();
                    fs::write(target.join("secret.rs"), "OUTSIDE_MARKER_ZZZ\n").unwrap();
                }
            })),
        );
        let error = engine.index(&race.root, &Control::unbounded()).unwrap_err();
        fault::disarm_all();
        drop(engine);
        race.assert_refused_without_mutation(error);
    }
}

#[test]
fn root_ancestor_replaced_by_symlink_is_refused_before_any_mutation() {
    let fixture = tempfile::tempdir().unwrap();
    let nested = fixture.path().join("outer").join("inner").join("ws");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("inside.rs"), "fn inside_probe() {}\n").unwrap();
    let outside = fixture.path().join("elsewhere");
    fs::create_dir_all(outside.join("inner").join("ws")).unwrap();
    fs::write(
        outside.join("inner").join("ws").join("secret.rs"),
        "OUTSIDE_MARKER_ZZZ\n",
    )
    .unwrap();
    let store = fixture.path().join("store");
    let mut engine = Engine::initialize(&store, &nested).unwrap();
    engine.index(&nested, &Control::unbounded()).unwrap();
    drop(engine);
    let before = snapshot(&store);
    let mut engine = Engine::open_existing(&store).unwrap();
    fault::arm(
        names::SCAN_BEFORE_ROOT_OPEN,
        0,
        Action::Call(Box::new(move |ctx| {
            // The grandparent of the canonical root: ".../outer".
            let outer = Path::new(ctx.detail).parent().unwrap().parent().unwrap();
            swap_for_symlink(outer, &outside);
        })),
    );
    let error = engine.index(&nested, &Control::unbounded()).unwrap_err();
    fault::disarm_all();
    assert_eq!(error.code(), "unsafe_source_path");
    drop(engine);
    assert_eq!(snapshot(&store), before);
    let engine = Engine::open_existing(&store).unwrap();
    assert!(
        engine
            .search("OUTSIDE_MARKER_ZZZ", 5)
            .unwrap()
            .hits
            .is_empty()
    );
}

#[test]
fn injected_unreadable_files_keep_exact_counts_bounded_samples_and_prior_state() {
    let mut fx = new_fixture();
    for i in 0..25 {
        fs::write(
            fx.root.join(format!("fail-{i:02}.txt")),
            format!("content {i}\n"),
        )
        .unwrap();
        fs::write(fx.root.join(format!("bin-{i:02}.bin")), [0u8, 1, 2, 3]).unwrap();
    }
    fs::write(fx.root.join("good.rs"), "fn good() {}\n").unwrap();
    let root = fx.root.clone();
    // A previously accepted record for one of the failing files.
    fx.engine
        .replace_source("fail-00.txt", "accepted before\n")
        .unwrap();
    fx.engine.record_feedback(&feedback("t1")).unwrap();
    let prior = fx.engine.source("fail-00.txt").unwrap().unwrap().hash;
    let feedback_rows = fx.engine.training_examples().unwrap();
    // Deterministic, privilege-independent read failure; the message is long
    // so the 512-byte sample bound is exercised.
    fault::arm(
        names::SCAN_BEFORE_OPEN,
        0,
        Action::FailWhen {
            detail_contains: "fail-".into(),
            message: format!("injected unreadable file {}", "x".repeat(900)),
        },
    );
    let report = fx.engine.index(&root, &Control::unbounded()).unwrap();
    fault::disarm_all();
    assert_eq!(report.failures, 25);
    assert_eq!(report.failure_samples.len(), 20);
    assert_eq!(report.excluded, 25);
    assert_eq!(report.exclusion_samples.len(), 20);
    assert!(
        report
            .failure_samples
            .iter()
            .all(|s| s.len() <= 512 && s.contains("read_failed"))
    );
    assert!(report.exclusion_samples.iter().all(|s| s.len() <= 512));
    assert!(report.partial && report.deletions_deferred && !report.scan_complete);
    assert_eq!(report.reason_code, Some("scan_failures"));
    assert_eq!(report.changed, 1, "only good.rs was readable");
    // The unseen sweep was deferred: the prior record and feedback survive.
    assert_eq!(
        fx.engine.source("fail-00.txt").unwrap().unwrap().hash,
        prior
    );
    assert_eq!(fx.engine.training_examples().unwrap(), feedback_rows);
    assert_eq!(fx.engine.status().unwrap().scan_state, "incomplete");
    // The error a bounded consumer sees carries counts only.
    let error = report.index_error().unwrap();
    assert_eq!(error.code(), "index_incomplete");
    assert!(error.bounded_json().len() <= 1024);
    // Re-indexing safely retries: with the injection gone everything lands.
    let retry = fx.engine.index(&root, &Control::unbounded()).unwrap();
    assert_eq!(retry.failures, 0);
    assert!(retry.scan_complete);
    assert_eq!(fx.engine.status().unwrap().scan_state, "complete");
}

#[test]
fn ten_thousand_paths_keep_every_page_and_sample_within_bounds() {
    const TEXT: u64 = 5_000;
    const BINARY: u64 = 5_000;
    let mut fx = new_fixture();
    for i in 0..TEXT {
        fs::write(fx.root.join(format!("t{i:05}.txt")), format!("file {i}\n")).unwrap();
    }
    for i in 0..BINARY {
        fs::write(fx.root.join(format!("b{i:05}.bin")), [0u8, 1, 2, i as u8]).unwrap();
    }
    let root = fx.root.clone();
    // Record the size of every page/batch each bounded walk forms.
    let mut recorded: Vec<(&str, Rc<RefCell<Vec<usize>>>)> = Vec::new();
    for name in [
        names::SEEN_FLUSH,
        names::INDEX_AFTER_SEARCH_COMMIT,
        names::SWEEP_PAGE,
    ] {
        let sink = Rc::new(RefCell::new(Vec::<usize>::new()));
        let recorder = Rc::clone(&sink);
        fault::arm(
            name,
            0,
            Action::Call(Box::new(move |ctx: &Ctx<'_>| {
                recorder.borrow_mut().push(ctx.detail.parse().unwrap());
            })),
        );
        recorded.push((name, sink));
    }
    let sizes = |name: &str| -> Vec<usize> {
        recorded
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, sink)| sink.borrow().clone())
            .unwrap()
    };
    let report = fx.engine.index(&root, &Control::unbounded()).unwrap();
    assert!(!report.partial, "{report:?}");
    // Exact 64-bit aggregates far beyond the 20-sample diagnostics.
    assert_eq!(report.changed, TEXT);
    assert_eq!(report.excluded, BINARY);
    assert_eq!(report.exclusion_samples.len(), 20);
    assert!(report.exclusion_samples.iter().all(|s| s.len() <= 512));
    assert!(report.failure_samples.is_empty());
    assert_eq!(report.source_revision, TEXT);
    // Page high-water marks: every walk stayed within its 128-key bound, and
    // nothing close to the 10,000 paths was ever buffered at once.
    let seen = sizes(names::SEEN_FLUSH);
    assert_eq!(seen.iter().sum::<usize>(), (TEXT + BINARY) as usize);
    assert!(seen.iter().all(|n| *n <= 128), "seen flush pages: {seen:?}");
    assert_eq!(seen.iter().copied().max(), Some(128));
    let batches = sizes(names::INDEX_AFTER_SEARCH_COMMIT);
    assert_eq!(batches.iter().sum::<usize>(), TEXT as usize);
    assert!(
        batches.iter().all(|n| *n <= 128),
        "index batches: {batches:?}"
    );
    assert_eq!(
        sizes(names::SWEEP_PAGE).iter().sum::<usize>(),
        TEXT as usize
    );
    fault::disarm_all();
    // An unchanged tree changes nothing and does not bump the revision.
    let again = fx.engine.index(&root, &Control::unbounded()).unwrap();
    assert_eq!((again.unchanged, again.changed), (TEXT, 0));
    assert_eq!(again.excluded, BINARY);
    assert_eq!(again.source_revision, TEXT);
    // Removing every text file drives the paged sweep and the batched drain.
    for i in 0..TEXT {
        fs::remove_file(root.join(format!("t{i:05}.txt"))).unwrap();
    }
    let final_report = fx.engine.index(&root, &Control::unbounded()).unwrap();
    assert!(!final_report.partial && final_report.scan_complete);
    assert_eq!(final_report.deleted, TEXT);
    assert_eq!(fx.engine.status().unwrap().source_count, 0);
    assert_eq!(fx.engine.pending().unwrap(), 0);
    assert_eq!(final_report.source_revision, 2 * TEXT);
}

#[test]
fn interrupted_enumeration_and_sweep_retire_only_verified_rows_and_resume() {
    const GONE: u64 = 300;
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    fs::create_dir(&root).unwrap();
    for name in ["present.rs", "present2.rs", "present3.rs"] {
        fs::write(root.join(name), "present marker\n").unwrap();
    }
    // .ignore excludes a subtree whose records must retire only after a
    // complete scan; feedback must survive every interruption untouched.
    fs::write(root.join(".ignore"), "ignored/\n").unwrap();
    fs::create_dir(root.join("ignored")).unwrap();
    fs::write(root.join("ignored").join("x.rs"), "ignored body\n").unwrap();
    let store = fixture.path().join("store");
    {
        let engine = Engine::initialize(&store, &root).unwrap();
        for i in 0..GONE {
            engine
                .replace_source(&format!("gone-{i:03}.rs"), &format!("old {i}\n"))
                .unwrap();
        }
        engine
            .replace_source("ignored/x.rs", "ignored body\n")
            .unwrap();
        engine.record_feedback(&feedback("t1")).unwrap();
    }
    let seeded = knowledge(&snapshot(&store));
    let feedback_rows = seeded["feedback"].clone();
    let total = GONE + 1;
    let count = |engine: &Engine| engine.status().unwrap().source_count;

    // Enumeration interrupted at the second file: nothing may retire.
    {
        let mut engine = Engine::open_existing(&store).unwrap();
        fault::arm(names::SCAN_BEFORE_OPEN, 1, Action::Cancel);
        let report = engine.index(&root, &Control::unbounded()).unwrap();
        fault::disarm_all();
        assert!(report.partial && report.deletions_deferred && !report.scan_complete);
        assert_eq!(report.reason_code, Some("cancelled"));
        assert_eq!(report.deleted, 0);
        assert_eq!(count(&engine), total + report.changed);
        assert_eq!(engine.status().unwrap().scan_state, "incomplete");
    }
    let after_enumeration = knowledge(&snapshot(&store));
    assert_eq!(after_enumeration["feedback"], feedback_rows);

    // Sweep interrupted after one 128-row page: exactly that page retired.
    {
        let mut engine = Engine::open_existing(&store).unwrap();
        let before = count(&engine);
        fault::arm(names::SWEEP_PAGE, 0, Action::Cancel);
        let report = engine.index(&root, &Control::unbounded()).unwrap();
        fault::disarm_all();
        assert!(report.partial && report.deletions_deferred);
        assert!(!report.scan_complete);
        assert_eq!(report.deleted, 128, "one verified-unseen page retires");
        // Exactly one page retired; sources this scan newly admitted add on top.
        assert_eq!(count(&engine), before - 128 + report.changed);
        assert_eq!(report.reason_code, Some("cancelled"));
    }
    assert_eq!(knowledge(&snapshot(&store))["feedback"], feedback_rows);

    // A fresh full scan completes; the ignored subtree retires with it.
    let mut engine = Engine::open_existing(&store).unwrap();
    let finish = engine.index(&root, &Control::unbounded()).unwrap();
    assert!(!finish.partial && finish.scan_complete);
    assert!(engine.source("ignored/x.rs").unwrap().is_none());
    assert!(engine.source("present.rs").unwrap().is_some());
    assert_eq!(count(&engine), 3);
    assert_eq!(engine.status().unwrap().scan_state, "complete");
    assert_eq!(engine.pending().unwrap(), 0);
    drop(engine);
    // Retiring source rows never touches memory or feedback.
    assert_eq!(knowledge(&snapshot(&store))["feedback"], feedback_rows);
}

#[test]
fn drain_cancellation_after_complete_scan_keeps_scan_complete_true() {
    let mut fx = new_fixture();
    fs::write(fx.root.join("new.rs"), "fn fresh() {}\n").unwrap();
    fx.engine.replace_source("gone-1.rs", "old\n").unwrap();
    fx.engine.replace_source("gone-2.rs", "old\n").unwrap();
    let root = fx.root.clone();
    fault::arm(names::INDEX_AFTER_SEARCH_COMMIT, 0, Action::Cancel);
    let report = fx.engine.index(&root, &Control::unbounded()).unwrap();
    fault::disarm_all();
    // Enumeration and sweep finished; only the derived-index drain stopped.
    assert!(report.scan_complete, "{report:?}");
    assert!(!report.deletions_deferred);
    assert_eq!((report.changed, report.deleted), (1, 2));
    assert!(report.partial);
    assert_eq!(report.reason_code, Some("cancelled"));
    assert_eq!(fx.engine.status().unwrap().scan_state, "complete");
    assert_eq!(report.index_error().unwrap().code(), "cancelled");
}

#[test]
fn non_utf8_file_name_is_a_named_failure_that_prevents_the_unseen_sweep() {
    use std::os::unix::ffi::OsStrExt;
    let mut fx = new_fixture();
    fx.add(&[
        ("accepted.rs", "fn accepted() {}\n"),
        ("vanished.rs", "fn vanished() {}\n"),
    ]);
    fs::write(fx.root.join("accepted.rs"), "fn accepted() {}\n").unwrap();
    let name = std::ffi::OsStr::from_bytes(b"bad-\xff-name.rs");
    match fs::write(fx.root.join(name), "fn bad() {}\n") {
        Ok(()) => {}
        // Some filesystems (APFS) reject non-UTF-8 names outright, so the
        // condition cannot arise there; the path-encoding rule is exercised
        // wherever the filesystem permits the name (e.g. Linux ext4).
        Err(error) if error.raw_os_error() == Some(libc::EILSEQ) => {
            eprintln!(
                "filesystem rejects non-UTF-8 names ({error}); path-encoding branch not reachable here"
            );
            return;
        }
        Err(error) => panic!("unexpected failure creating the fixture: {error}"),
    }
    let root = fx.root.clone();
    let report = fx.engine.index(&root, &Control::unbounded()).unwrap();
    assert_eq!(report.failures, 1);
    assert!(report.failure_samples[0].contains("not UTF-8"));
    assert!(report.deletions_deferred && report.partial && !report.scan_complete);
    // The unseen sweep never ran: the absent record is preserved, nothing
    // unreachable was committed.
    assert!(fx.engine.source("vanished.rs").unwrap().is_some());
    assert_eq!(fx.engine.status().unwrap().source_count, 2);
}

#[test]
fn root_swapped_for_an_empty_directory_during_enumeration_retires_nothing() {
    let mut fx = new_fixture();
    fs::write(fx.root.join("here.rs"), "fn here() {}\n").unwrap();
    let root = fx.root.clone();
    fx.engine.index(&root, &Control::unbounded()).unwrap();
    fx.engine
        .replace_source("absent.rs", "fn absent() {}\n")
        .unwrap();
    let revision = fx.engine.source_revision().unwrap();
    let swap_root = root.clone();
    fault::arm(
        names::SCAN_BEFORE_WALK,
        0,
        Action::Call(Box::new(move |ctx| {
            // Swap the bound root for a fresh empty directory: the walker then
            // enumerates nothing, and the post-walk identity check must refuse
            // to let the sweep retire the store's rows.
            let moved = swap_root.with_file_name("ws.real");
            fs::rename(&swap_root, &moved).unwrap();
            fs::create_dir(&swap_root).unwrap();
            let _ = ctx;
        })),
    );
    let report = fx.engine.index(&root, &Control::unbounded()).unwrap();
    fault::disarm_all();
    assert_eq!(report.failures, 1);
    assert!(
        report.failure_samples[0].starts_with("(root): unsafe_source_path"),
        "{:?}",
        report.failure_samples
    );
    assert!(report.deletions_deferred && report.partial && !report.scan_complete);
    assert_eq!(report.deleted, 0);
    assert!(fx.engine.source("absent.rs").unwrap().is_some());
    assert_eq!(fx.engine.status().unwrap().source_count, 2);
    assert_eq!(fx.engine.source_revision().unwrap(), revision);
}

#[test]
fn one_read_failure_preserves_absent_records_until_a_clean_rerun_retires_them() {
    let mut fx = new_fixture();
    fs::write(fx.root.join("a.rs"), "fn readable() {}\n").unwrap();
    fs::write(fx.root.join("b.rs"), "fn also_readable() {}\n").unwrap();
    fx.engine
        .replace_source("gone.rs", "fn gone() {}\n")
        .unwrap();
    let root = fx.root.clone();
    fault::arm(
        names::SCAN_BEFORE_OPEN,
        0,
        Action::FailWhen {
            detail_contains: "a.rs".into(),
            message: "injected unreadable".into(),
        },
    );
    let report = fx.engine.index(&root, &Control::unbounded()).unwrap();
    fault::disarm_all();
    assert_eq!(report.failures, 1);
    assert!(report.deletions_deferred && report.partial);
    // The failure globally prevented the sweep: the absent record survives.
    assert!(fx.engine.source("gone.rs").unwrap().is_some());
    assert_eq!(report.changed, 1, "only b.rs was admitted");
    // Without the injection the complete scan retires it.
    let clean = fx.engine.index(&root, &Control::unbounded()).unwrap();
    assert!(clean.scan_complete && !clean.deletions_deferred);
    assert!(fx.engine.source("gone.rs").unwrap().is_none());
    assert_eq!(clean.deleted, 1);
}

#[test]
fn a_mutation_at_the_search_commit_keeps_pending_at_the_new_version() {
    let mut fx = new_fixture();
    fx.engine
        .replace_source("a.rs", "fn version_one() {}\n")
        .unwrap();
    fault::arm(
        names::INDEX_AFTER_SEARCH_COMMIT,
        0,
        Action::Call(Box::new(|ctx| {
            // A change commits right after the search commit for version one.
            ctx.engine
                .unwrap()
                .replace_source("a.rs", "fn version_two() {}\n")
                .unwrap();
            ctx.control.unwrap().cancel();
        })),
    );
    let error = fx.engine.refresh(&Control::unbounded()).unwrap_err();
    fault::disarm_all();
    assert_eq!(error.code(), "cancelled");
    // The pending row now carries version TWO's hash: the drain must not
    // clear it as if it had indexed the new bytes.
    let (_dir, store, _root) = fx.close();
    assert_eq!(
        testkit::pending_value(&store, "source:a.rs").as_deref(),
        Some(digest("fn version_two() {}\n".as_bytes()).as_str())
    );
    let mut engine = Engine::open_existing(&store).unwrap();
    assert_eq!(engine.pending().unwrap(), 1);
    engine.refresh(&Control::unbounded()).unwrap();
    assert_eq!(engine.pending().unwrap(), 0);
    assert_eq!(engine.search("version_two", 5).unwrap().hits.len(), 1);
    assert!(engine.search("version_one", 5).unwrap().hits.is_empty());
}

#[test]
fn an_unreadable_file_is_a_named_permission_failure_not_an_internal_error() {
    if unsafe { libc::geteuid() } == 0 {
        return; // root bypasses permission bits.
    }
    use std::os::unix::fs::PermissionsExt;
    let mut fx = new_fixture();
    fs::write(fx.root.join("open.rs"), "fn open_probe() {}\n").unwrap();
    let locked = fx.root.join("locked.rs");
    fs::write(&locked, "fn locked_probe() {}\n").unwrap();
    let mut perms = fs::metadata(&locked).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&locked, perms).unwrap();
    let root = fx.root.clone();
    let report = fx.engine.index(&root, &Control::unbounded()).unwrap();
    assert_eq!(report.failures, 1);
    assert!(
        report.failure_samples[0].contains("permission_denied"),
        "{:?}",
        report.failure_samples
    );
    assert!(report.deletions_deferred && report.partial);
    assert_eq!(report.changed, 1);
}
