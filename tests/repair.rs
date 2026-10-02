//! Explicit repair refuses every replacement identity or path it cannot
//! positively match, moving and deleting nothing it does not own.
use context_foundry::fault::{self, Action, names};
use context_foundry::testkit::{
    CORRUPT_INDEX_BYTES, Scratch, corrupt_search_index, knowledge, meta_value, quarantine_dirs,
    snapshot,
};
use context_foundry::{Control, Engine};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

/// A store with a corrupt derived index and a repair interrupted right after
/// its first enqueue page: marker set, original quarantined, and a replacement
/// directory that carries only this repair's identity file.
struct Interrupted {
    _dir: Scratch,
    store: PathBuf,
    knowledge_before: std::collections::BTreeMap<String, Vec<(String, String)>>,
}

fn owned(
    snapshot: &context_foundry::testkit::Snapshot,
) -> std::collections::BTreeMap<String, Vec<(String, String)>> {
    snapshot
        .iter()
        .map(|(name, rows)| ((*name).to_owned(), rows.clone()))
        .collect()
}

fn interrupted() -> Interrupted {
    let dir = Scratch::new();
    let root = dir.path().join("ws");
    fs::create_dir(&root).unwrap();
    let store = dir.path().join("store");
    {
        let mut engine = Engine::initialize(&store, &root).unwrap();
        for i in 0..10 {
            engine
                .replace_source(&format!("s{i}.rs"), &format!("body {i}\n"))
                .unwrap();
        }
        engine.refresh(&Control::unbounded()).unwrap();
    }
    corrupt_search_index(&store);
    fault::arm(
        names::REPAIR_AFTER_ENQUEUE_PAGE,
        0,
        Action::Fail("interrupted".into()),
    );
    let error = Engine::repair_index(&store, &Control::unbounded()).unwrap_err();
    fault::disarm_all();
    assert_eq!(error.code(), "internal");
    assert!(store.join("search").join("rebuild_id").is_file());
    let knowledge_before = owned(&knowledge(&snapshot(&store)));
    Interrupted {
        _dir: dir,
        store,
        knowledge_before,
    }
}

impl Interrupted {
    fn owner_id(&self) -> String {
        fs::read_to_string(self.store.join("search").join("rebuild_id")).unwrap()
    }

    /// The repair refuses with `repair_path_conflict`, and everything it was
    /// not allowed to touch is exactly as the operator left it.
    fn assert_refused(&self, what: &str) {
        let before = tree_listing(&self.store);
        let error = Engine::repair_index(&self.store, &Control::unbounded()).unwrap_err();
        assert_eq!(error.code(), "repair_path_conflict", "{what}: {error:?}");
        assert_eq!(
            tree_listing(&self.store),
            before,
            "{what}: files moved or deleted"
        );
        assert_eq!(
            owned(&knowledge(&snapshot(&self.store))),
            self.knowledge_before,
            "{what}"
        );
        // Still marked: search names repair_required, status stays available.
        let engine = Engine::open_existing(&self.store).unwrap();
        assert_eq!(engine.status().unwrap().index_state, "repair_required");
    }
}

/// Every path under `dir` with its kind and (for files) bytes.
fn tree_listing(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        for entry in fs::read_dir(&path).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            let p = entry.path();
            if p.file_name().is_some_and(|n| n == "knowledge.redb") {
                continue;
            }
            if kind.is_dir() {
                out.push((p.clone(), "dir".to_owned()));
                stack.push(p);
            } else if kind.is_symlink() {
                out.push((
                    p.clone(),
                    format!("link -> {:?}", fs::read_link(&p).unwrap()),
                ));
            } else {
                out.push((p.clone(), format!("file {}", fs::read(&p).unwrap().len())));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn replacement_with_an_empty_identity_is_refused() {
    let state = interrupted();
    fs::write(state.store.join("search").join("rebuild_id"), "").unwrap();
    fs::write(state.store.join("search").join("operator-file"), "keep me").unwrap();
    state.assert_refused("empty identity");
}

#[test]
fn replacement_with_a_malformed_identity_is_refused() {
    let state = interrupted();
    fs::write(
        state.store.join("search").join("rebuild_id"),
        "not-the-owner-id",
    )
    .unwrap();
    fs::write(state.store.join("search").join("operator-file"), "keep me").unwrap();
    state.assert_refused("malformed identity");
}

#[test]
fn replacement_with_a_symlinked_identity_is_refused_even_if_it_points_at_the_right_id() {
    let state = interrupted();
    let owner = state.owner_id();
    let elsewhere = state.store.parent().unwrap().join("outside-id");
    fs::write(&elsewhere, &owner).unwrap();
    fs::remove_file(state.store.join("search").join("rebuild_id")).unwrap();
    symlink(&elsewhere, state.store.join("search").join("rebuild_id")).unwrap();
    fs::write(state.store.join("search").join("operator-file"), "keep me").unwrap();
    state.assert_refused("symlinked identity");
    assert_eq!(
        fs::read_to_string(&elsewhere).unwrap(),
        owner,
        "outside file untouched"
    );
}

#[test]
fn replacement_without_identity_beside_other_files_is_refused() {
    let state = interrupted();
    fs::remove_file(state.store.join("search").join("rebuild_id")).unwrap();
    fs::write(state.store.join("search").join("mystery"), "unknown").unwrap();
    state.assert_refused("missing identity");
}

#[test]
fn replacement_path_that_is_a_symlink_is_refused_and_never_traversed() {
    let state = interrupted();
    let outside = state.store.parent().unwrap().join("outside-dir");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("precious"), "do not delete").unwrap();
    fs::remove_dir_all(state.store.join("search")).unwrap();
    symlink(&outside, state.store.join("search")).unwrap();
    state.assert_refused("symlinked replacement path");
    assert_eq!(
        fs::read_to_string(outside.join("precious")).unwrap(),
        "do not delete"
    );
}

#[test]
fn quarantine_path_conflict_after_recorded_intent_moves_nothing() {
    // Abort-equivalent: the intent to quarantine is durable but the rename
    // never ran; then something else occupies the quarantine name.
    let dir = Scratch::new();
    let root = dir.path().join("ws");
    fs::create_dir(&root).unwrap();
    let store = dir.path().join("store");
    {
        let mut engine = Engine::initialize(&store, &root).unwrap();
        engine.replace_source("a.rs", "body\n").unwrap();
        engine.refresh(&Control::unbounded()).unwrap();
    }
    corrupt_search_index(&store);
    fault::arm(
        names::REPAIR_BEFORE_QUARANTINE_RENAME,
        0,
        Action::Fail("interrupted".into()),
    );
    Engine::repair_index(&store, &Control::unbounded()).unwrap_err();
    fault::disarm_all();
    let marker: serde_json::Value =
        serde_json::from_str(&meta_value(&store, "search_rebuild_required").unwrap()).unwrap();
    let quarantine = store.join(marker["quarantine"].as_str().unwrap());
    fs::create_dir(&quarantine).unwrap();
    fs::write(quarantine.join("occupant"), "someone else's data").unwrap();
    let before = tree_listing(&store);
    let error = Engine::repair_index(&store, &Control::unbounded()).unwrap_err();
    assert_eq!(error.code(), "repair_path_conflict");
    assert_eq!(tree_listing(&store), before);
    // The original derived directory was never moved over the occupant.
    assert_eq!(
        fs::read(store.join("search").join("meta.json")).unwrap(),
        CORRUPT_INDEX_BYTES
    );
}

#[test]
fn a_completely_empty_replacement_directory_holds_nothing_and_is_adopted() {
    let state = interrupted();
    for entry in fs::read_dir(state.store.join("search")).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    let report = Engine::repair_index(&state.store, &Control::unbounded()).unwrap();
    assert!(report.repaired);
    assert_eq!(quarantine_dirs(&state.store).len(), 1);
    assert_eq!(
        owned(&knowledge(&snapshot(&state.store))),
        state.knowledge_before
    );
}

#[test]
fn a_positively_matched_replacement_is_reset_and_completes() {
    let state = interrupted();
    fs::write(
        state.store.join("search").join("partial-segment"),
        "half built",
    )
    .unwrap();
    let report = Engine::repair_index(&state.store, &Control::unbounded()).unwrap();
    assert!(report.repaired);
    assert!(!state.store.join("search").join("partial-segment").exists());
    let engine = Engine::open_existing(&state.store).unwrap();
    assert_eq!(engine.status().unwrap().index_state, "ready");
}
