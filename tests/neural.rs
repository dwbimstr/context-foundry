//! 009 T001 acceptance (SC-001): bounded preparation commits exact-profile
//! vectors and resumes from cache; source/search repair preserves them.
//! Every FakeProvider document call is counted, so each FR-002 reuse claim
//! is an exact call-count assertion. Partition coverage cases live beside
//! the partition implementation; the cases here run the real Rust tokenizer
//! (a byte-level fixture: one token per UTF-8 byte) through the real
//! preparation path end to end.
#![cfg(feature = "semantic")]
use context_foundry::neural::cache::{CacheLookup, DEFAULT_CACHE_CAP_BYTES, PartitionRecord};
use context_foundry::neural::fake::{FakeBehavior, FakeFactory};
use context_foundry::neural::index;
use context_foundry::neural::partition::{self, TokenCount as _};
use context_foundry::neural::prepare::{self, PrepareOptions, PrepareReport};
use context_foundry::neural::profile::SemanticProfile;
use context_foundry::neural::provider::{
    self, DOCUMENT_BATCH, DOCUMENT_UNIT_TOKENS, ProviderError, SERVING_LIMIT_TOKENS, TokenizedInput,
};
use context_foundry::neural::status::SemanticStatus;
use context_foundry::neural::tokenize::DocumentTokenizer;
use context_foundry::testkit;
use context_foundry::{Control, Engine, FoundryError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// One body of exactly `bytes` ASCII bytes.
fn body(bytes: usize) -> String {
    "x".repeat(bytes)
}

struct Env {
    _dir: tempfile::TempDir,
    store: PathBuf,
    profile: PathBuf,
    engine: Option<Engine>,
    factory: FakeFactory,
}

impl Env {
    /// A store holding `sources`, the fake profile and a fresh fake factory.
    fn new(sources: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().join("ws");
        std::fs::create_dir(&ws).unwrap();
        let store = dir.path().join("store");
        let mut engine = Engine::initialize(&store, &ws).unwrap();
        for (path, content) in sources {
            engine.replace_source(path, content).unwrap();
        }
        engine.refresh(&Control::unbounded()).unwrap();
        let profile = testkit::write_semantic_profile(dir.path(), "fake-a", |_| {});
        let loaded = SemanticProfile::load(&profile).unwrap();
        let factory = FakeFactory::new(loaded.descriptor.clone(), FakeBehavior::Ok);
        Self {
            _dir: dir,
            store,
            profile,
            engine: Some(engine),
            factory,
        }
    }

    fn open(&mut self) -> &Engine {
        self.engine
            .get_or_insert_with(|| Engine::open_existing(&self.store).unwrap())
    }

    fn open_mut(&mut self) -> &mut Engine {
        self.engine
            .get_or_insert_with(|| Engine::open_existing(&self.store).unwrap())
    }

    fn close(&mut self) {
        self.engine = None;
    }

    /// One preparation run under the fake provider. The engine is released
    /// for the run, exactly like the CLI process boundary.
    fn prepare(&mut self, budget_seconds: u64) -> PrepareReport {
        self.prepare_with(budget_seconds, DEFAULT_CACHE_CAP_BYTES)
    }

    fn prepare_with(&mut self, budget_seconds: u64, cap: u64) -> PrepareReport {
        self.engine = None;
        let profile = self.profile.clone();
        let report = prepare::run(
            &self.store,
            &PrepareOptions {
                profile_path: &profile,
                budget_seconds,
                development: false,
                cache_cap_bytes: cap,
                started: Instant::now(),
                control: &Control::unbounded(),
            },
            self.factory.acquire(),
        )
        .expect("preparation runs");
        self.engine = Some(Engine::open_existing(&self.store).unwrap());
        report
    }

    fn prepare_err(&mut self, budget_seconds: u64) -> FoundryError {
        self.engine = None;
        let profile = self.profile.clone();
        let error = prepare::run(
            &self.store,
            &PrepareOptions {
                profile_path: &profile,
                budget_seconds,
                development: false,
                cache_cap_bytes: u64::MAX,
                started: Instant::now(),
                control: &Control::unbounded(),
            },
            self.factory.acquire(),
        )
        .unwrap_err();
        self.engine = Some(Engine::open_existing(&self.store).unwrap());
        error
    }

    fn calls(&self) -> u64 {
        self.factory.handle().document_calls()
    }

    fn status(&mut self) -> SemanticStatus {
        self.open().semantic_status(&Control::unbounded()).unwrap()
    }

    fn partition_rows(&mut self) -> Vec<(String, String)> {
        self.close();
        testkit::table_rows(&self.store, "semantic_partitions")
    }

    fn digest(&self) -> String {
        SemanticProfile::load(&self.profile)
            .unwrap()
            .descriptor
            .digest()
    }
}

fn first_preparation(sources: &[(&str, &str)]) -> (Env, PrepareReport) {
    let mut env = Env::new(sources);
    let report = env.prepare(60);
    (env, report)
}

/// Run one preparation against `profile` on `store`, swapping the engine
/// slot exactly like a CLI restart; returns the total document calls so far
/// and the report.
fn run_once(store: &Path, profile: &Path, slot: &mut Option<Engine>) -> (u64, PrepareReport) {
    *slot = None;
    let loaded = SemanticProfile::load(profile).unwrap();
    let factory = FakeFactory::new(loaded.descriptor.clone(), FakeBehavior::Ok);
    let report = prepare::run(
        store,
        &PrepareOptions {
            profile_path: profile,
            budget_seconds: 60,
            development: false,
            cache_cap_bytes: u64::MAX,
            started: Instant::now(),
            control: &Control::unbounded(),
        },
        factory.acquire(),
    )
    .expect("preparation runs");
    *slot = Some(Engine::open_existing(store).unwrap());
    (factory.handle().document_calls(), report)
}

#[test]
fn first_preparation_counts_calls_and_commits() {
    // Three one-unit sources: one batch of three inputs in one call.
    let (mut env, report) = first_preparation(&[
        ("a.txt", &body(100)),
        ("b.txt", &body(120)),
        ("c.txt", &body(140)),
    ]);
    assert!(!report.partial, "{report:?}");
    assert_eq!(report.sources, 3);
    assert_eq!(report.partitioned_sources, 3);
    assert_eq!(report.eligible_units, 3);
    assert_eq!(report.embedded_units, 3);
    assert_eq!(report.document_calls, 1);
    assert_eq!(env.calls(), 1);
    env.factory.handle().assert_batches_bounded();
    let status = env.status();
    assert_eq!(status.state, "stopped");
    assert_eq!(status.corpus, "nonempty");
    assert_eq!(status.partition_coverage, "complete");
    assert_eq!(status.eligible_units, 3);
    assert_eq!(status.cached_current_units, 3);
    assert_eq!(status.searchable_current_units, 3);
    assert_eq!(status.missing_units, 0);
    assert_eq!(status.cache.entries, 3);
    assert_eq!(status.cache.bytes, 3 * 8256);
    assert_eq!(status.cache.orphan_entries, 0);
    assert!(status.index.available, "{status:?}");
}

#[test]
fn unchanged_restart_makes_zero_document_calls() {
    let (mut env, first) = first_preparation(&[("a.txt", &body(100)), ("b.txt", &body(120))]);
    let before = env.calls();
    let second = env.prepare(60);
    assert_eq!(second.document_calls, 0);
    assert_eq!(env.calls(), before);
    assert_eq!(second.embedded_units, 0);
    assert_eq!(second.reused_partitions, 2);
    assert_eq!(second.reused_cached_units, first.eligible_units);
    assert!(!second.partial);
    assert_eq!(second.index_entries, first.index_entries);
}

#[test]
fn edited_input_reembeds_only_its_units() {
    let (mut env, first) = first_preparation(&[
        ("a.txt", &body(100)),
        ("b.txt", &body(120)),
        ("c.txt", &body(140)),
    ]);
    let before = env.calls();
    // A one-byte edit inside one whole-file unit.
    env.open()
        .replace_source("b.txt", &format!("{}\n", body(119)))
        .unwrap();
    env.open_mut().refresh(&Control::unbounded()).unwrap();
    // The old mapping is ineligible immediately, before any preparation.
    let status = env.status();
    assert_eq!(status.unpartitioned_sources, 1);
    assert_eq!(status.partition_coverage, "partial");
    assert_eq!(status.corpus, "unknown");
    assert_eq!(status.eligible_units, first.eligible_units - 1);
    assert_eq!(
        status.missing_units, 0,
        "the unpartitioned source is unknown, not missing"
    );
    let report = env.prepare(60);
    assert_eq!(report.document_calls, 1);
    assert_eq!(env.calls(), before + 1);
    assert_eq!(report.embedded_units, 1);
    assert_eq!(report.reused_partitions, 2);
    assert_eq!(report.missing_units, 0);
    assert_eq!(env.partition_rows().len(), 3);
}

#[test]
fn rename_without_title_in_input_is_zero_calls() {
    // Rendering is exactly `passage: ` plus the unit's source bytes: no path,
    // no title, no language. A rename alone cannot invalidate a vector.
    let (mut env, _) = first_preparation(&[("a.txt", &body(100)), ("b.txt", &body(120))]);
    let before = env.calls();
    let engine = env.open_mut();
    engine.delete_source("a.txt").unwrap();
    engine.replace_source("z.txt", &body(100)).unwrap();
    engine.refresh(&Control::unbounded()).unwrap();
    let report = env.prepare(60);
    assert_eq!(report.document_calls, 0, "{report:?}");
    assert_eq!(env.calls(), before);
    assert_eq!(report.embedded_units, 0);
    assert_eq!(report.reused_cached_units, 2);
    let status = env.status();
    assert_eq!(status.eligible_units, 2);
    assert_eq!(status.cached_current_units, 2);
}

#[test]
fn retained_cache_revert_and_orphans_are_counted() {
    let (mut env, _) = first_preparation(&[("a.txt", &body(100))]);
    let original = body(100);
    let edited = body(101);
    env.open().replace_source("a.txt", &edited).unwrap();
    env.open_mut().refresh(&Control::unbounded()).unwrap();
    let report = env.prepare(60);
    assert_eq!(report.document_calls, 1);
    assert_eq!(env.status().cache.entries, 2);
    // Reverting reuses the retained vector: zero calls.
    env.open().replace_source("a.txt", &original).unwrap();
    env.open_mut().refresh(&Control::unbounded()).unwrap();
    let report = env.prepare(60);
    assert_eq!(report.document_calls, 0);
    assert_eq!(report.embedded_units, 0);
    // The edited version's vector is retained cache, disclosed as orphans.
    let status = env.status();
    assert_eq!(status.cache.entries, 2);
    assert_eq!(status.cache.orphan_entries, 1);
    assert_eq!(status.cached_current_units, 1);
}

#[test]
fn different_document_function_reembeds_and_retains_both() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir(&ws).unwrap();
    let store = dir.path().join("store");
    let mut engine = Engine::initialize(&store, &ws).unwrap();
    engine.replace_source("a.txt", &body(100)).unwrap();
    engine.refresh(&Control::unbounded()).unwrap();
    let profile_a = testkit::write_semantic_profile(dir.path(), "fake-a", |_| {});
    let profile_b = testkit::write_semantic_profile(dir.path(), "fake-b", |descriptor| {
        // A different quantization is a different document function.
        descriptor.quantization = "affine bits=8".into();
    });
    let digest_a = SemanticProfile::load(&profile_a)
        .unwrap()
        .descriptor
        .digest();
    let digest_b = SemanticProfile::load(&profile_b)
        .unwrap()
        .descriptor
        .digest();
    assert_ne!(digest_a, digest_b);
    let mut slot = Some(engine);
    let (calls_a, report_a) = run_once(&store, &profile_a, &mut slot);
    assert_eq!((calls_a, report_a.embedded_units), (1, 1));
    let (calls_b, report_b) = run_once(&store, &profile_b, &mut slot);
    assert_eq!(
        (calls_b, report_b.embedded_units),
        (1, 1),
        "a different document function re-embeds"
    );
    assert_eq!(report_b.reused_partitions, 0, "keys are re-derived");
    assert_ne!(report_a.function_digest, report_b.function_digest);
    // Both functions' vectors are retained under their own keys.
    let engine = slot.as_ref().unwrap();
    let (entries, _) = engine.semantic_cache_totals().unwrap();
    assert_eq!(entries, 2);
    drop(slot);
}

#[test]
fn display_only_change_is_zero_calls() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir(&ws).unwrap();
    let store = dir.path().join("store");
    let mut engine = Engine::initialize(&store, &ws).unwrap();
    engine.replace_source("a.txt", &body(100)).unwrap();
    engine.refresh(&Control::unbounded()).unwrap();
    // Same descriptor (identical document function), different display name.
    let profile_a = testkit::write_semantic_profile(dir.path(), "alpha", |_| {});
    let profile_b = testkit::write_semantic_profile(dir.path(), "beta", |_| {});
    let digest_a = SemanticProfile::load(&profile_a)
        .unwrap()
        .descriptor
        .digest();
    let digest_b = SemanticProfile::load(&profile_b)
        .unwrap()
        .descriptor
        .digest();
    assert_eq!(digest_a, digest_b, "display labels are not identity");
    let mut slot = Some(engine);
    let (calls, _) = run_once(&store, &profile_a, &mut slot);
    assert_eq!(calls, 1);
    let (calls_again, report) = run_once(&store, &profile_b, &mut slot);
    assert_eq!(calls_again, 0, "a display-only change re-embeds nothing");
    assert_eq!(report.embedded_units, 0);
    drop(slot);
}

#[test]
fn interrupt_before_and_after_cache_commit() {
    use context_foundry::fault::{self, Action};
    // Nine one-unit sources force two batches (8 + 1).
    let sources: Vec<(String, String)> = (0..9)
        .map(|i| (format!("f{i}.txt"), body(100 + i)))
        .collect();
    let borrowed: Vec<(&str, &str)> = sources
        .iter()
        .map(|(path, content)| (path.as_str(), content.as_str()))
        .collect();
    let mut env = Env::new(&borrowed);

    // Before commit: batch one commits, batch two is lost with the run.
    fault::arm(
        prepare::fault_names::CACHE_BEFORE_COMMIT,
        1,
        Action::Fail("injected".into()),
    );
    let error = env.prepare_err(60);
    assert_eq!(error.code(), "internal");
    fault::disarm_all();
    let (entries, _) = env.open().semantic_cache_totals().unwrap();
    assert_eq!(entries, 8, "the first batch stays committed");
    let state = env.open().semantic_state().unwrap().unwrap();
    assert_eq!(
        state.committed_units, 8,
        "progress is durable with the batch"
    );
    assert_eq!(state.cache_bytes, 8 * 8256);
    let status = env.status();
    assert_eq!(status.state, "paused");
    assert_eq!(status.cached_current_units, 8);
    // Explicit resume embeds only the lost batch.
    let before = env.calls();
    let report = env.prepare(60);
    assert_eq!(report.document_calls, 1);
    assert_eq!(env.calls(), before + 1);
    assert_eq!(report.embedded_units, 1);
    assert!(!report.partial);

    // After commit: the batch IS committed; a resume embeds nothing.
    for i in 0..9 {
        env.open()
            .replace_source(&format!("g{i}.txt"), &body(300 + i))
            .unwrap();
    }
    env.open_mut().refresh(&Control::unbounded()).unwrap();
    fault::arm(
        prepare::fault_names::CACHE_AFTER_COMMIT,
        1,
        Action::Fail("injected".into()),
    );
    let error = env.prepare_err(60);
    assert_eq!(error.code(), "internal");
    fault::disarm_all();
    let report = env.prepare(60);
    assert_eq!(
        report.document_calls, 0,
        "the committed batch is not repeated"
    );
    assert_eq!(report.embedded_units, 0);
    assert!(!report.partial);
    let state = env.open().semantic_state().unwrap().unwrap();
    assert_eq!(
        state.committed_units, 18,
        "both fault points keep durable progress"
    );
    assert_eq!(state.cache_bytes, 18 * 8256);
}

#[test]
fn repair_index_rebuilds_the_semantic_generation_with_zero_calls() {
    let (mut env, first) = first_preparation(&[
        ("a.txt", &body(100)),
        ("b.txt", &body(120)),
        ("c.txt", &body(140)),
    ]);
    let before = env.calls();
    // Corrupt the published index file; validation must refuse it.
    env.close();
    let generation = index::generation_dir(&env.store, &env.digest());
    std::fs::write(generation.join(index::INDEX_FILE), b"broken").unwrap();
    let engine = Engine::open_existing(&env.store).unwrap();
    let status = engine.semantic_status(&Control::unbounded()).unwrap();
    assert!(!status.index.available, "{status:?}");
    assert_eq!(status.searchable_current_units, 0);
    drop(engine);
    // `foundry repair-index` replays the generation from the f32 cache.
    let report = Engine::repair_index(&env.store, &Control::unbounded()).unwrap();
    assert!(report.repaired);
    let semantic = report.semantic_index.expect("semantic rebuild ran");
    assert!(semantic.rebuilt, "{semantic:?}");
    assert_eq!(semantic.entries as u64, first.eligible_units);
    assert_eq!(env.calls(), before, "zero document calls");
    let engine = Engine::open_existing(&env.store).unwrap();
    let status = engine.semantic_status(&Control::unbounded()).unwrap();
    assert!(status.index.available, "{status:?}");
    assert_eq!(status.searchable_current_units, first.eligible_units);
    drop(engine);
}

#[test]
fn f16_rebuild_from_f32_cache_is_zero_calls() {
    let (mut env, first) = first_preparation(&[("a.txt", &body(100))]);
    let before = env.calls();
    // Remove the whole generation directory: preparation rebuilds it from
    // cache without any document call.
    env.close();
    std::fs::remove_dir_all(index::generation_dir(&env.store, &env.digest())).unwrap();
    let report = env.prepare(60);
    assert_eq!(report.document_calls, 0);
    assert_eq!(env.calls(), before);
    assert!(report.index_published);
    assert_eq!(report.index_entries, first.index_entries);
    assert!(env.status().index.available);
}

#[test]
fn short_and_nonfinite_vectors_preserve_valid_data() {
    for behavior in [FakeBehavior::ShortVector, FakeBehavior::Nonfinite] {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().join("ws");
        std::fs::create_dir(&ws).unwrap();
        let store = dir.path().join("store");
        let mut engine = Engine::initialize(&store, &ws).unwrap();
        engine.replace_source("a.txt", &body(100)).unwrap();
        engine.replace_source("b.txt", &body(120)).unwrap();
        engine.refresh(&Control::unbounded()).unwrap();
        let profile = testkit::write_semantic_profile(dir.path(), "fake-a", |_| {});
        let loaded = SemanticProfile::load(&profile).unwrap();
        let factory = FakeFactory::new(loaded.descriptor.clone(), behavior);
        drop(engine);
        let report = prepare::run(
            &store,
            &PrepareOptions {
                profile_path: &profile,
                budget_seconds: 60,
                development: false,
                cache_cap_bytes: u64::MAX,
                started: Instant::now(),
                control: &Control::unbounded(),
            },
            factory.acquire(),
        )
        .expect("the run stops with a named reason, not a crash");
        assert!(report.partial, "{report:?}");
        assert_eq!(report.reason_code, Some("provider_malformed"));
        assert_eq!(
            report.error().expect("partial names its error").code(),
            "provider_malformed"
        );
        let engine = Engine::open_existing(&store).unwrap();
        let (entries, _) = engine.semantic_cache_totals().unwrap();
        assert_eq!(entries, 0, "the malformed batch is not committed");
        assert!(
            engine.source("a.txt").unwrap().is_some(),
            "source access stays available"
        );
        drop(engine);
    }
}

#[test]
fn wrong_profile_provider_is_refused_before_any_cache_write() {
    let mut env = Env::new(&[("a.txt", &body(100))]);
    let loaded = SemanticProfile::load(&env.profile).unwrap();
    env.factory = FakeFactory::new(loaded.descriptor.clone(), FakeBehavior::WrongDescriptor);
    let error = env.prepare_err(60);
    assert_eq!(error.code(), "provider_malformed", "{error}");
    assert_eq!(env.calls(), 0);
    let (entries, _) = env.open().semantic_cache_totals().unwrap();
    assert_eq!(entries, 0);
}

#[test]
fn disk_cap_stops_with_cache_full_and_preserves_valid_data() {
    // Nine one-unit sources: the first batch of eight fits a cap of eight
    // entries; the ninth crosses it and the run stops with `cache_full`.
    let sources: Vec<(String, String)> = (0..9)
        .map(|i| (format!("f{i}.txt"), body(100 + i)))
        .collect();
    let borrowed: Vec<(&str, &str)> = sources
        .iter()
        .map(|(path, content)| (path.as_str(), content.as_str()))
        .collect();
    let mut env = Env::new(&borrowed);
    let cap = 8 * 8256;
    let report = env.prepare_with(60, cap);
    assert!(report.partial, "{report:?}");
    assert_eq!(report.reason_code, Some("cache_full"));
    assert_eq!(report.embedded_units, 8);
    let (entries, bytes) = env.open().semantic_cache_totals().unwrap();
    assert_eq!((entries, bytes), (8, cap));
    let status = env.status();
    assert_eq!(status.state, "paused");
    assert_eq!(status.cached_current_units, 8);
    assert!(report.index_published, "{report:?}");
    assert_eq!(
        status.searchable_current_units, 8,
        "committed coverage is published at the stop"
    );
    assert_eq!(status.missing_units, 1);
    assert_eq!(status.cache.cap_bytes, DEFAULT_CACHE_CAP_BYTES);
    // The ninth source itself is untouched.
    assert!(env.open().source("f8.txt").unwrap().is_some());
}

#[test]
fn profile_change_across_restart_refuses_old_generation_and_restores() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir(&ws).unwrap();
    let store = dir.path().join("store");
    let mut engine = Engine::initialize(&store, &ws).unwrap();
    engine.replace_source("a.txt", &body(100)).unwrap();
    engine.refresh(&Control::unbounded()).unwrap();
    let profile_a = testkit::write_semantic_profile(dir.path(), "a", |_| {});
    let profile_b = testkit::write_semantic_profile(dir.path(), "b", |descriptor| {
        descriptor.quantization = "affine bits=8".into();
    });
    let digest_a = SemanticProfile::load(&profile_a)
        .unwrap()
        .descriptor
        .digest();
    let digest_b = SemanticProfile::load(&profile_b)
        .unwrap()
        .descriptor
        .digest();
    let recipe = partition::recipe_id("fake-bytes 1");
    let anchor = context_foundry::neural::anchor::Dir::open_path(&store).unwrap();
    let mut slot = Some(engine);
    let (calls_a, report_a) = run_once(&store, &profile_a, &mut slot);
    assert_eq!(calls_a, 1);
    assert!(index::validate_generation(&anchor, &digest_a, &recipe).is_ok());
    // Profile change happens only across restart (a new run): the new
    // profile builds its own generation, and serving validation binds digest
    // and recipe by content, never by directory name.
    let (calls_b, report_b) = run_once(&store, &profile_b, &mut slot);
    assert_eq!(calls_b, 1);
    assert_eq!(report_b.function_digest, digest_b);
    assert!(index::validate_generation(&anchor, &digest_b, &recipe).is_ok());
    assert!(
        index::validate_generation(&anchor, &digest_a, &recipe).is_ok(),
        "the prior generation is retained on disk"
    );
    // Both generation directories exist; the cache holds both functions.
    let semantic_dir = store.join("semantic");
    let dirs = std::fs::read_dir(&semantic_dir).unwrap().count();
    assert_eq!(dirs, 2);
    let engine = slot.as_ref().unwrap();
    assert_eq!(engine.semantic_cache_totals().unwrap().0, 2);
    let retained = engine.semantic_status(&Control::unbounded()).unwrap();
    assert_eq!(
        retained.cache.corrupt_entries, 0,
        "another function's valid row is retention"
    );
    assert_eq!(retained.cache.orphan_entries, 1);
    // Restoring the prior profile rebuilds its index from retained cache.
    let (calls_a2, report_a2) = run_once(&store, &profile_a, &mut slot);
    assert_eq!(calls_a2, 0, "no model calls for cached inputs");
    assert_eq!(report_a2.embedded_units, 0);
    assert_eq!(report_a2.index_entries, report_a.index_entries);
    let restored = slot
        .as_ref()
        .unwrap()
        .semantic_status(&Control::unbounded())
        .unwrap();
    assert_eq!(restored.cache.corrupt_entries, 0);
    assert_eq!(restored.cache.orphan_entries, 1);
    drop(slot);
}

#[test]
fn purge_beside_serving_is_busy_then_offline_purge_loses_nothing() {
    let (mut env, _) = first_preparation(&[("a.txt", &body(100)), ("b.txt", &body(120))]);
    env.close();
    let knowledge_before = testkit::knowledge(&testkit::snapshot(&env.store));
    // A serving owner holds the store: purge cannot even open it.
    let _serving = env.open();
    let busy = Engine::open_existing(&env.store).unwrap_err();
    assert_eq!(busy.code(), "store_busy");
    // Offline purge under sole ownership.
    env.close();
    let engine = Engine::open_existing(&env.store).unwrap();
    let report = engine.semantic_purge().unwrap();
    assert_eq!(report.removed_partitions, 2);
    assert_eq!(report.removed_cache_entries, 2);
    assert_eq!(report.removed_cache_bytes, 2 * 8256);
    assert_eq!(report.removed_generation_dirs, 1);
    drop(engine);
    // Sources, graph, memory and feedback survive; semantic tables are
    // empty and the generation directory is gone.
    let snapshot_after = testkit::snapshot(&env.store);
    let knowledge_after = testkit::knowledge(&snapshot_after);
    assert_eq!(knowledge_after, knowledge_before);
    assert!(snapshot_after["semantic_partitions"].is_empty());
    assert_eq!(snapshot_after["semantic_state"].len(), 1);
    assert!(!env.store.join("semantic").join(env.digest()).exists());
    let engine = Engine::open_existing(&env.store).unwrap();
    let status = engine.semantic_status(&Control::unbounded()).unwrap();
    assert_eq!(status.state, "stopped");
    assert!(status.profile.is_none());
    assert_eq!(status.unpartitioned_sources, 2);
    assert_eq!(status.corpus, "unknown");
    assert_eq!(status.cache.entries, 0);
    assert!(!status.index.available);
    // No repopulation: only an explicit prepare re-embeds.
    drop(engine);
    let report = env.prepare(60);
    assert_eq!(report.document_calls, 1);
    assert_eq!(report.embedded_units, 2);
}

#[test]
fn source_reads_stay_usable_after_malformed_cache_and_state() {
    let (mut env, _) = first_preparation(&[("a.txt", &body(100))]);
    env.close();
    // A malformed cache row disables that semantic data BY NAME; source
    // reads stay available and status counts the corruption honestly.
    let (key, _) = testkit::semantic_cache_rows(&env.store)
        .into_iter()
        .next()
        .unwrap();
    testkit::tamper_semantic_cache_row(&env.store, &key, b"garbage");
    let engine = Engine::open_existing(&env.store).unwrap();
    let source = engine.source("a.txt").unwrap().expect("source readable");
    assert_eq!(source.bytes, 100);
    let status = engine.semantic_status(&Control::unbounded()).unwrap();
    assert_eq!(status.cache.corrupt_entries, 1);
    assert_eq!(status.cached_current_units, 0);
    drop(engine);
    // An explicit prepare overwrites the corrupt row with a fresh vector.
    let report = env.prepare(60);
    assert_eq!(report.document_calls, 1);
    assert_eq!(report.corrupt_cache_rows, 1);
    let status = env.status();
    assert_eq!(status.cache.corrupt_entries, 0);
    assert_eq!(status.cached_current_units, 1);

    // An undecodable STATE row is named corruption: status refuses, source
    // reads still work, and an explicit purge is the recovery path.
    env.close();
    testkit::tamper_semantic_state(&env.store, "{not json");
    let engine = Engine::open_existing(&env.store).unwrap();
    assert!(engine.source("a.txt").unwrap().is_some());
    assert!(
        engine.semantic_status(&Control::unbounded()).is_err(),
        "the state row corruption is named"
    );
    let purged = engine.semantic_purge().unwrap();
    assert_eq!(purged.removed_cache_entries, 1);
    let status = engine.semantic_status(&Control::unbounded()).unwrap();
    assert_eq!(status.state, "stopped");
    drop(engine);
}

#[test]
fn cold_status_reports_unknown_totals_and_never_tokenizes() {
    let mut env = Env::new(&[("a.txt", &body(100)), ("b.txt", &body(120))]);
    let status = env.status();
    assert_eq!(status.sources, 2);
    assert_eq!(status.unpartitioned_sources, 2);
    assert_eq!(status.partition_coverage, "partial");
    assert_eq!(status.corpus, "unknown");
    assert_eq!(status.eligible_units, 0);
    assert_eq!(status.missing_units, 0);
    assert_eq!(status.state, "stopped");
    assert!(status.profile.is_none());
    // Zero units known is NOT an empty corpus while sources are unpartitioned.
    let report = env.prepare(60);
    assert!(report.index_published);
    // Repeated status performs no tokenization and no inference: it works
    // with the profile file and the whole model directory deleted.
    env.close();
    let model_dir = SemanticProfile::load(&env.profile).unwrap().model_dir;
    std::fs::remove_dir_all(&model_dir).unwrap();
    std::fs::remove_file(&env.profile).unwrap();
    let engine = Engine::open_existing(&env.store).unwrap();
    let again = engine.semantic_status(&Control::unbounded()).unwrap();
    assert_eq!(again.corpus, "nonempty");
    assert_eq!(again.cached_current_units, again.eligible_units);
    let _ = engine.semantic_status(&Control::unbounded()).unwrap();
    drop(engine);
}

#[test]
fn empty_file_is_a_completed_zero_unit_partition_not_missing_metadata() {
    // Only an empty source: a completed zero-unit partition, corpus `empty`.
    let (mut env, report) = first_preparation(&[("empty.txt", "")]);
    assert_eq!(report.sources, 1);
    assert_eq!(report.eligible_units, 0);
    assert_eq!(report.embedded_units, 0);
    assert_eq!(report.document_calls, 0);
    let status = env.status();
    assert_eq!(status.corpus, "empty");
    assert_eq!(status.partition_coverage, "complete");
    assert_eq!(status.unpartitioned_sources, 0);
    // The row exists and carries zero units — distinguishable from absent.
    let rows = env.partition_rows();
    assert_eq!(rows.len(), 1);
    let record: PartitionRecord = serde_json::from_str(&rows[0].1).unwrap();
    assert!(record.units.is_empty());
    // A second, unpartitioned source makes the corpus unknown again.
    env.open().replace_source("note.txt", &body(50)).unwrap();
    env.open_mut().refresh(&Control::unbounded()).unwrap();
    let status = env.status();
    assert_eq!(status.unpartitioned_sources, 1);
    assert_eq!(status.corpus, "unknown");
    assert_eq!(status.eligible_units, 0, "the empty file contributes zero");
}

#[test]
fn one_source_change_rejects_its_mapping_without_retokenizing_others() {
    let (mut env, _) = first_preparation(&[
        ("a.txt", &body(100)),
        ("b.txt", &body(120)),
        ("c.txt", &body(140)),
    ]);
    let a_before = env
        .partition_rows()
        .into_iter()
        .find(|(path, _)| path == "a.txt")
        .unwrap();
    env.open()
        .replace_source("b.txt", &format!("{}\n", body(119)))
        .unwrap();
    env.open_mut().refresh(&Control::unbounded()).unwrap();
    let report = env.prepare(60);
    assert_eq!(report.reused_partitions, 2);
    assert_eq!(report.partitioned_sources, 1);
    // a.txt's partition row is untouched, byte for byte.
    let a_after = env
        .partition_rows()
        .into_iter()
        .find(|(path, _)| path == "a.txt")
        .unwrap();
    assert_eq!(a_before, a_after);
    assert_eq!(report.document_calls, 1);
    assert_eq!(report.embedded_units, 1);
}

#[test]
fn budget_exhaustion_names_itself_and_commits_nothing() {
    let mut env = Env::new(&[("a.txt", &body(100)), ("b.txt", &body(120))]);
    // A zero budget expires during profile verification: no partitioning,
    // no provider, no cache writes; state paused with the named reason.
    let report = env.prepare(0);
    assert!(report.partial, "{report:?}");
    assert_eq!(report.reason_code, Some("budget_exhausted"));
    assert_eq!(report.partitioned_sources, 0);
    assert_eq!(report.document_calls, 0);
    assert_eq!(env.calls(), 0);
    let status = env.status();
    assert_eq!(status.state, "paused");
    assert_eq!(status.unpartitioned_sources, 2);
    assert_eq!(status.last_error.as_ref().unwrap().code, "budget_exhausted");
    assert!(env.open().source("a.txt").unwrap().is_some());
}

#[test]
fn cancelled_partition_run_leaves_partial_census() {
    use context_foundry::fault::{self, Action};
    let mut env = Env::new(&[("a.txt", &body(100)), ("b.txt", &body(120))]);
    fault::arm(
        prepare::fault_names::PARTITION_AFTER_COMMIT,
        0,
        Action::Cancel,
    );
    let report = env.prepare(60);
    fault::disarm_all();
    assert!(report.partial);
    assert_eq!(report.reason_code, Some("cancelled"));
    assert_eq!(report.partitioned_sources, 1);
    assert_eq!(
        env.calls(),
        0,
        "a cancel before the first admission admits no call"
    );
    let status = env.status();
    // One file partitioned (its units known), one unpartitioned (unknown).
    assert_eq!(status.unpartitioned_sources, 1);
    assert_eq!(status.partition_coverage, "partial");
    assert_eq!(status.corpus, "unknown");
    assert_eq!(status.eligible_units, 1);
}

#[test]
fn batches_are_bounded_and_units_share_vectors_across_files() {
    // Twenty one-unit sources: batches of 8, 8 and 4; two files share one
    // body, so identical rendered inputs share a vector.
    let shared = body(500);
    let mut sources: Vec<(String, String)> = Vec::new();
    for i in 0..18 {
        sources.push((format!("f{i:02}.txt"), body(100 + i)));
    }
    sources.push(("dup1.txt".into(), shared.clone()));
    sources.push(("dup2.txt".into(), shared));
    let borrowed: Vec<(&str, &str)> = sources
        .iter()
        .map(|(path, content)| (path.as_str(), content.as_str()))
        .collect();
    let mut env = Env::new(&borrowed);
    let report = env.prepare(60);
    assert_eq!(report.eligible_units, 20);
    assert_eq!(report.embedded_units, 19, "the duplicate shares one vector");
    assert_eq!(report.document_calls, 3, "8 + 8 + 3");
    env.factory.handle().assert_batches_bounded();
    let status = env.status();
    assert_eq!(status.cached_current_units, 20);
    assert_eq!(status.cache.entries, 19);
    assert!(!report.partial, "{report:?}");
    assert!(
        report.index_published,
        "shared inputs must not fail publication"
    );
    assert_eq!(report.index_entries, 19);
    assert!(status.index.available, "{status:?}");
    assert_eq!(
        status.searchable_current_units, 20,
        "duplicate units are searchable too"
    );
    // Storage order never becomes unit identity: the two duplicate units
    // carry the same input key in their partition rows.
    let mut keys = Vec::new();
    for (_, raw) in env.partition_rows() {
        let record: PartitionRecord = serde_json::from_str(&raw).unwrap();
        keys.extend(record.units.into_iter().map(|unit| unit.input_key));
    }
    assert_eq!(keys.len(), 20);
    assert_eq!(
        keys.iter().collect::<std::collections::HashSet<_>>().len(),
        19
    );
}

#[test]
fn exact_input_keys_include_the_prefix_and_nothing_else() {
    // The stored input key is exactly the function digest and `passage: ` +
    // the unit's source bytes — no path, title or language, one prefix.
    let (mut env, _) = first_preparation(&[("a.txt", &body(1015))]);
    let digest = env.digest();
    let engine = env.open();
    let body_text = body(1015);
    let meta = engine.source("a.txt").unwrap().unwrap();
    let stored = engine.semantic_partition("a.txt").unwrap().unwrap();
    assert_eq!(stored.units.len(), 1);
    let unit = &stored.units[0];
    assert_eq!(unit.start, 0);
    assert_eq!(unit.end, meta.bytes);
    let expected = provider::input_key(&digest, &provider::render_document(&body_text));
    assert_eq!(unit.input_key, expected);
    assert_eq!(
        provider::render_document(&body_text)
            .matches("passage: ")
            .count(),
        1,
        "exactly one prefix, applied once"
    );
}

#[test]
fn unit_limit_boundaries_with_the_real_tokenizer() {
    // The fixture tokenizer counts one token per UTF-8 byte: 1015 body bytes
    // render to exactly 1024 model tokens (one unit); 1016 must split into
    // two units, never truncate.
    let (mut env, report) = first_preparation(&[("at.txt", &body(1015))]);
    assert_eq!(report.eligible_units, 1);
    let record = env.open().semantic_partition("at.txt").unwrap().unwrap();
    assert_eq!(record.units.len(), 1);

    let (mut env, report) = first_preparation(&[("over.txt", &body(1016))]);
    assert_eq!(report.eligible_units, 2);
    assert_eq!(report.embedded_units, 2);
    let record = env.open().semantic_partition("over.txt").unwrap().unwrap();
    assert_eq!(record.units.len(), 2);
    // Complete, nonoverlapping coverage with the exact expected spans.
    let spans: Vec<(usize, usize)> = record
        .units
        .iter()
        .map(|unit| (unit.start, unit.end))
        .collect();
    assert_eq!(spans, [(0, 1015), (1015, 1016)]);
}

#[test]
fn serving_limit_is_checked_before_any_model_work() {
    // 2048 model tokens pass; 2049 are refused before encode.
    let ok_query = TokenizedInput {
        ids: vec![7; SERVING_LIMIT_TOKENS],
    };
    assert!(provider::check_query(&ok_query).is_ok());
    let over = TokenizedInput {
        ids: vec![7; SERVING_LIMIT_TOKENS + 1],
    };
    assert_eq!(
        provider::check_query(&over).unwrap_err().code(),
        "input_too_large"
    );
    // Document batches: at most 8 inputs of at most 1024 ids.
    let batch: Vec<TokenizedInput> = (0..DOCUMENT_BATCH)
        .map(|_| TokenizedInput {
            ids: vec![7; DOCUMENT_UNIT_TOKENS],
        })
        .collect();
    assert!(provider::check_document_batch(&batch).is_ok());
    let too_many: Vec<TokenizedInput> = (0..=DOCUMENT_BATCH)
        .map(|_| TokenizedInput { ids: vec![7; 8] })
        .collect();
    assert_eq!(
        provider::check_document_batch(&too_many)
            .unwrap_err()
            .code(),
        "input_too_large"
    );
    let mut too_long = batch.clone();
    too_long[0].ids.push(7);
    assert_eq!(
        provider::check_document_batch(&too_long)
            .unwrap_err()
            .code(),
        "input_too_large"
    );
}

#[test]
fn markdown_fences_and_crlf_partition_exactly() {
    let (_env, report) = first_preparation(&[(
        "doc.md",
        "# real section\n\nbody one\n\n```text\n# not a heading\n```\nmore body\n",
    )]);
    assert_eq!(
        report.eligible_units, 1,
        "the fence does not split the section"
    );
    let (mut env, report) = first_preparation(&[("crlf.txt", "alpha\r\nbeta\r\n\r\ngamma\r\n")]);
    assert_eq!(report.eligible_units, 1);
    let record = env.open().semantic_partition("crlf.txt").unwrap().unwrap();
    assert_eq!(record.units.len(), 1);
    assert_eq!(record.units[0].end, "alpha\r\nbeta\r\n\r\ngamma\r\n".len());
}

#[test]
fn schema_5_upgrade_from_v4_preserves_every_table_atomically() {
    use context_foundry::fault::{self, Action, names};
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let ws = dir.path().join("ws");
    std::fs::create_dir(&ws).unwrap();
    // A populated schema-4 store: initialize, add a source, downgrade.
    {
        let engine = Engine::initialize(&store, &ws).unwrap();
        engine.replace_source("kept.rs", "kept\n").unwrap();
        drop(engine);
        testkit::downgrade_to_v4(&store);
    }
    let before = testkit::snapshot(&store);
    assert_eq!(testkit::schema_marker(&store), "4");
    // A v4 store refuses to open: explicit upgrade required.
    let refused = Engine::open_existing(&store).unwrap_err();
    assert_eq!(refused.code(), "upgrade_required");
    // Interrupted before commit: wholly v4, row for row.
    fault::arm(
        names::UPGRADE_BEFORE_COMMIT,
        0,
        Action::Fail("injected".into()),
    );
    Engine::upgrade_store(&store, 6, &Control::unbounded()).unwrap_err();
    fault::disarm_all();
    assert_eq!(testkit::snapshot(&store), before);
    // The previous version is no longer a target.
    let error = Engine::upgrade_store(&store, 4, &Control::unbounded()).unwrap_err();
    assert_eq!(error.code(), "unsupported_mode");
    // The interrupted-after-commit boundary leaves it wholly v6.
    fault::arm(
        names::UPGRADE_AFTER_COMMIT,
        0,
        Action::Fail("injected".into()),
    );
    Engine::upgrade_store(&store, 6, &Control::unbounded()).unwrap_err();
    fault::disarm_all();
    assert_eq!(testkit::schema_marker(&store), "6");
    let after = testkit::snapshot(&store);
    for table in [
        "sources",
        "chunks",
        "pending_index",
        "meta",
        "feedback",
        "scan_seen",
        "provider_bundles",
        "edges_out",
        "edges_in",
        "memory",
        "compiler_producers",
        "compiler_scopes",
        "compiler_occurrences",
        "compiler_by_symbol",
    ] {
        let mut left = after[table].clone();
        let mut right = before[table].clone();
        if table == "meta" {
            left.retain(|(key, _)| key != "schema");
            right.retain(|(key, _)| key != "schema");
        }
        assert_eq!(left, right, "{table} preserved");
    }
    for table in testkit::SEMANTIC_TABLES {
        assert!(after[table].is_empty(), "{table} starts empty");
    }
    // The upgraded store opens and serves source reads.
    let engine = Engine::open_existing(&store).unwrap();
    assert!(engine.source("kept.rs").unwrap().is_some());
}

#[test]
fn schema_6_upgrade_from_v5_preserves_every_table_and_the_vector_cache() {
    use context_foundry::fault::{self, Action, names};
    // A populated schema-5 store: a committed semantic preparation (cache,
    // partitions, state), memory and legacy feedback; then the 013 learning
    // tables are removed and the marker says 5.
    let (mut env, report) = first_preparation(&[("a.txt", &body(100)), ("b.txt", &body(200))]);
    assert!(!report.partial);
    {
        let engine = env.open();
        engine
            .memory_put(&context_foundry::memory::PutInput {
                fields: context_foundry::memory::RecordFields {
                    id: "kept-note".into(),
                    text: "kept memory".into(),
                    author: "tests".into(),
                    provenance: "tests".into(),
                    source_links: vec![],
                },
                workspace_id: engine.workspace_id().unwrap(),
            })
            .unwrap();
        engine
            .record_feedback(&context_foundry::laya::Feedback {
                task_id: "t".into(),
                query: "q".into(),
                correct_strategy: context_foundry::laya::Strategy::Search,
                label_source: "operator".into(),
                allow_training: true,
            })
            .unwrap();
    }
    env.close();
    testkit::downgrade_to_v5(&env.store);
    let before = testkit::snapshot(&env.store);
    let cache_before = testkit::semantic_cache_rows(&env.store);
    assert!(!cache_before.is_empty(), "the vector cache is populated");
    assert!(!before["semantic_partitions"].is_empty());
    assert!(!before["memory"].is_empty() && !before["feedback"].is_empty());
    assert_eq!(testkit::schema_marker(&env.store), "5");
    let Err(refused) = Engine::open_existing(&env.store) else {
        panic!("a schema-5 store must not open");
    };
    assert_eq!(refused.code(), "upgrade_required");
    // Interrupted before commit: wholly v5, row for row.
    fault::arm(
        names::UPGRADE_BEFORE_COMMIT,
        0,
        Action::Fail("injected".into()),
    );
    Engine::upgrade_store(&env.store, 6, &Control::unbounded()).unwrap_err();
    fault::disarm_all();
    assert_eq!(testkit::snapshot(&env.store), before);
    assert_eq!(testkit::semantic_cache_rows(&env.store), cache_before);
    assert_eq!(testkit::schema_marker(&env.store), "5");
    Engine::upgrade_store(&env.store, 6, &Control::unbounded()).unwrap();
    assert_eq!(testkit::schema_marker(&env.store), "6");
    let after = testkit::snapshot(&env.store);
    for (table, rows) in &before {
        let (mut left, mut right) = (after[table].clone(), rows.clone());
        if *table == "meta" {
            left.retain(|(key, _)| key != "schema");
            right.retain(|(key, _)| key != "schema");
        }
        assert_eq!(left, right, "{table} preserved");
    }
    assert_eq!(
        testkit::semantic_cache_rows(&env.store),
        cache_before,
        "the vector cache is preserved byte for byte"
    );
    for table in testkit::LEARNING_TABLES {
        assert!(after[table].is_empty(), "{table} starts empty");
    }
    // The upgraded store opens and serves source reads.
    assert!(env.open().source("a.txt").unwrap().is_some());
}

#[test]
fn missing_artifacts_fail_by_name_without_downloads() {
    // A profile whose model directory lost a file: a named failure, no
    // download attempt, no cache reset, no state damage.
    let mut env = Env::new(&[("a.txt", &body(100))]);
    let report = env.prepare(60);
    assert!(!report.partial);
    env.close();
    let model_dir = SemanticProfile::load(&env.profile).unwrap().model_dir;
    std::fs::remove_file(model_dir.join("model.safetensors")).unwrap();
    let error = env.prepare_err(60);
    assert_eq!(error.code(), "profile_invalid", "{error}");
    // The committed cache survives the refused run untouched.
    let (entries, _) = env.open().semantic_cache_totals().unwrap();
    assert_eq!(entries, 1);
}

#[test]
fn byte_tokenizer_fixture_counts_one_token_per_byte() {
    // Guard the fixture itself: the byte tokenizer is the oracle the exact
    // token counts above depend on, loaded through the real tokenizer path.
    let dir = tempfile::tempdir().unwrap();
    let profile_path = testkit::write_semantic_profile(dir.path(), "toy", |_| {});
    let profile = SemanticProfile::load(&profile_path).unwrap();
    profile.verify_artifacts().unwrap();
    let tokenizer = DocumentTokenizer::load(&profile).unwrap();
    for probe in [
        "passage: x",
        "alpha\r\nbeta\r\n",
        "gamma γamma 🦀 delta\n",
        &"y".repeat(4096),
    ] {
        assert_eq!(
            tokenizer.count(probe).unwrap(),
            probe.len(),
            "{probe:?} must be one token per byte"
        );
    }
}

/// Nine one-unit sources: a full batch of eight plus a batch of one.
fn nine_sources() -> Vec<(String, String)> {
    (0..9)
        .map(|i| (format!("f{i}.txt"), body(100 + i)))
        .collect()
}

fn env_with(sources: &[(String, String)], behavior: FakeBehavior) -> Env {
    let borrowed: Vec<(&str, &str)> = sources
        .iter()
        .map(|(path, content)| (path.as_str(), content.as_str()))
        .collect();
    let mut env = Env::new(&borrowed);
    let loaded = SemanticProfile::load(&env.profile).unwrap();
    env.factory = FakeFactory::new(loaded.descriptor.clone(), behavior);
    env
}

#[test]
fn no_batch_is_admitted_below_the_publication_reserve() {
    // An 8 s budget keeps a 5 s publication reserve. The first batch is
    // admitted with at least 5 s left; its 3.5 s call leaves less, so no
    // second batch is admitted, and publication then runs inside the time
    // that remains.
    let mut env = env_with(&nine_sources(), FakeBehavior::SlowMs(3500));
    let report = env.prepare(8);
    assert!(report.partial, "{report:?}");
    assert_eq!(report.reason_code, Some("budget_exhausted"));
    assert_eq!(report.publication_reserve_seconds, 5);
    assert_eq!(
        report.document_calls, 1,
        "no second batch below the reserve"
    );
    assert_eq!(env.calls(), 1);
    assert_eq!(report.embedded_units, 8, "the in-flight batch committed");
    let (entries, _) = env.open().semantic_cache_totals().unwrap();
    assert_eq!(entries, 8, "committed counts are reported exactly");
    assert!(report.index_published, "{report:?}");
    let status = env.status();
    assert_eq!(status.state, "paused");
    assert_eq!(status.last_error.as_ref().unwrap().code, "budget_exhausted");
    assert_eq!(status.cached_current_units, 8);
    assert_eq!(status.missing_units, 1);
    assert_eq!(
        status.searchable_current_units, 8,
        "committed coverage is searchable"
    );
}

#[test]
fn no_inference_is_admitted_when_the_budget_is_below_the_reserve() {
    // A 4 s budget is inside the 5 s reserve from the start: nothing is
    // embedded, nothing is lost, and the stop is named.
    let mut env = env_with(&nine_sources(), FakeBehavior::Ok);
    let report = env.prepare(4);
    assert!(report.partial, "{report:?}");
    assert_eq!(report.reason_code, Some("budget_exhausted"));
    assert_eq!(report.document_calls, 0);
    assert_eq!(env.calls(), 0);
    assert_eq!(env.open().semantic_cache_totals().unwrap().0, 0);
    assert!(env.open().source("f0.txt").unwrap().is_some());
}

#[test]
fn a_provider_timeout_stops_preparation_with_committed_counts_intact() {
    let mut env = env_with(&nine_sources(), FakeBehavior::TimeoutOnCall(2));
    let report = env.prepare(60);
    assert!(report.partial, "{report:?}");
    assert_eq!(report.reason_code, Some("provider_timeout"));
    assert_eq!(env.calls(), 2, "the timed-out call was made, none after it");
    assert_eq!(report.embedded_units, 8);
    let (entries, _) = env.open().semantic_cache_totals().unwrap();
    assert_eq!(entries, 8, "the first batch stays committed");
    let status = env.status();
    assert_eq!(status.state, "paused");
    assert_eq!(status.last_error.as_ref().unwrap().code, "provider_timeout");
    assert_eq!(status.cached_current_units, 8);
    assert_eq!(status.missing_units, 1);
    assert_eq!(
        status.searchable_current_units, 8,
        "committed coverage is searchable"
    );
    // Source access is untouched; an explicit resume embeds only the rest.
    assert!(env.open().source("f8.txt").unwrap().is_some());
    env.factory = FakeFactory::new(
        SemanticProfile::load(&env.profile)
            .unwrap()
            .descriptor
            .clone(),
        FakeBehavior::Ok,
    );
    let resumed = env.prepare(60);
    assert!(!resumed.partial, "{resumed:?}");
    assert_eq!(resumed.embedded_units, 1);
    assert_eq!(env.calls(), 1);
}

// --- review round 2 --------------------------------------------------------

fn outside_dir() -> tempfile::TempDir {
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(outside.path().join("victim/inner")).unwrap();
    std::fs::write(outside.path().join("victim/inner/keep.txt"), b"keep").unwrap();
    std::fs::write(outside.path().join("top.txt"), b"top").unwrap();
    outside
}

fn assert_outside_untouched(outside: &tempfile::TempDir) {
    assert_eq!(
        std::fs::read(outside.path().join("victim/inner/keep.txt")).unwrap(),
        b"keep"
    );
    assert_eq!(
        std::fs::read(outside.path().join("top.txt")).unwrap(),
        b"top"
    );
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 2);
}

fn one_entry() -> Vec<(String, Vec<f32>)> {
    vec![("00".repeat(32), vec![0.5f32; provider::DIMENSIONS])]
}

#[test]
fn purge_and_publication_refuse_a_symlinked_semantic_root() {
    let (mut env, _) = first_preparation(&[("a.txt", &body(100))]);
    env.close();
    let outside = outside_dir();
    let root = env.store.join("semantic");
    std::fs::remove_dir_all(&root).unwrap();
    std::os::unix::fs::symlink(outside.path(), &root).unwrap();
    let before = testkit::snapshot(&env.store);
    let engine = Engine::open_existing(&env.store).unwrap();
    assert_eq!(
        engine.semantic_purge().unwrap_err().code(),
        "repair_path_conflict"
    );
    drop(engine);
    assert_outside_untouched(&outside);
    assert_eq!(
        testkit::snapshot(&env.store),
        before,
        "a refused purge changes no row"
    );
    let err = index::build_generation(
        &context_foundry::neural::anchor::Dir::open_path(&env.store).unwrap(),
        &env.digest(),
        "recipe",
        &one_entry(),
        &Control::unbounded(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "repair_path_conflict");
    assert_outside_untouched(&outside);
    let recipe = partition::recipe_id("fake-bytes 1");
    assert!(
        index::validate_generation(
            &context_foundry::neural::anchor::Dir::open_path(&env.store).unwrap(),
            &env.digest(),
            &recipe
        )
        .is_err()
    );
}

#[test]
fn purge_and_publication_refuse_a_symlinked_generation_directory() {
    let (mut env, _) = first_preparation(&[("a.txt", &body(100))]);
    env.close();
    let outside = outside_dir();
    let generation = index::generation_dir(&env.store, &env.digest());
    std::fs::remove_dir_all(&generation).unwrap();
    std::os::unix::fs::symlink(outside.path(), &generation).unwrap();
    let before = testkit::snapshot(&env.store);
    let engine = Engine::open_existing(&env.store).unwrap();
    assert_eq!(
        engine.semantic_purge().unwrap_err().code(),
        "repair_path_conflict"
    );
    drop(engine);
    assert_outside_untouched(&outside);
    assert_eq!(testkit::snapshot(&env.store), before);
    let err = index::build_generation(
        &context_foundry::neural::anchor::Dir::open_path(&env.store).unwrap(),
        &env.digest(),
        "recipe",
        &one_entry(),
        &Control::unbounded(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "repair_path_conflict");
    assert_outside_untouched(&outside);
    let recipe = partition::recipe_id("fake-bytes 1");
    assert!(
        index::validate_generation(
            &context_foundry::neural::anchor::Dir::open_path(&env.store).unwrap(),
            &env.digest(),
            &recipe
        )
        .is_err()
    );
}

#[test]
fn the_run_budget_reaches_the_provider_call_and_names_budget_exhausted() {
    // A 7 s budget admits the first batch (reserve 5 s); the stalled call is
    // released only by the run control's deadline.
    let mut env = env_with(&nine_sources(), FakeBehavior::StallUntilControl);
    let started = Instant::now();
    let report = env.prepare(7);
    assert!(
        started.elapsed() < Duration::from_secs(9),
        "the call was not bounded"
    );
    assert!(report.partial, "{report:?}");
    assert_eq!(report.reason_code, Some("budget_exhausted"), "{report:?}");
    assert_eq!(env.calls(), 1);
    assert_eq!(report.embedded_units, 0);
    assert_eq!(env.open().semantic_cache_totals().unwrap().0, 0);
}

#[test]
fn caller_cancellation_reaches_the_provider_call() {
    let mut env = env_with(&nine_sources(), FakeBehavior::StallUntilControl);
    let control = Control::unbounded();
    let flag = control.cancel_flag();
    // Cancel only once the provider call is observed in flight, so the
    // test never races the run's pre-admission work (verification, nine
    // partition commits): a cancel that lands BEFORE the first admission
    // correctly admits no call at all (see the cancelled-partition test).
    let handle = env.factory.handle().clone();
    let canceller = std::thread::spawn(move || {
        let started = Instant::now();
        while handle.document_calls() == 0 && started.elapsed() < Duration::from_secs(7) {
            std::thread::sleep(Duration::from_millis(10));
        }
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    env.engine = None;
    let profile = env.profile.clone();
    let report = prepare::run(
        &env.store,
        &PrepareOptions {
            profile_path: &profile,
            budget_seconds: 60,
            development: false,
            cache_cap_bytes: DEFAULT_CACHE_CAP_BYTES,
            started: Instant::now(),
            control: &control,
        },
        env.factory.acquire(),
    )
    .expect("a cancelled run is a named stop");
    canceller.join().unwrap();
    assert_eq!(report.reason_code, Some("cancelled"), "{report:?}");
    assert_eq!(env.calls(), 1);
}

#[test]
fn the_run_budget_reaches_worker_acquisition_and_names_budget_exhausted() {
    let mut env = env_with(&nine_sources(), FakeBehavior::StallAcquire);
    let started = Instant::now();
    let report = env.prepare(1);
    assert!(
        started.elapsed() < Duration::from_secs(7),
        "acquisition was not bounded"
    );
    assert!(report.partial, "{report:?}");
    assert_eq!(report.reason_code, Some("budget_exhausted"), "{report:?}");
    assert_eq!(env.calls(), 0, "no document call before a provider exists");
    assert_eq!(report.partitioned_sources, 0, "nothing was partitioned");
    let status = env.status();
    assert_eq!(status.state, "paused");
    assert_eq!(status.last_error.as_ref().unwrap().code, "budget_exhausted");
    assert_eq!(env.open().semantic_cache_totals().unwrap().0, 0);
    assert!(env.open().source("f0.txt").unwrap().is_some());
}

#[test]
fn the_final_short_batch_after_expiry_reports_budget_exhausted() {
    // One input; the call outlasts the whole 7 s budget. The final short
    // batch gets the post-flush check: budget_exhausted, never Complete.
    // Publication shares the budget, so it cannot run on an expired control:
    // the vectors stay cached, the index is NOT published, and the reason is
    // recorded in the report AND the state.
    let sources = vec![("only.txt".to_owned(), body(100))];
    let mut env = env_with(&sources, FakeBehavior::SlowMs(7500));
    let report = env.prepare(7);
    assert!(report.partial, "{report:?}");
    assert_eq!(report.reason_code, Some("budget_exhausted"));
    assert_eq!(report.document_calls, 1);
    assert_eq!(report.embedded_units, 1, "the in-flight batch committed");
    assert!(!report.index_published, "{report:?}");
    let reason = report.index_reason.clone().expect("the reason is recorded");
    assert!(reason.contains("budget"), "{reason}");
    let status = env.status();
    assert_eq!(status.state, "paused");
    assert_eq!(status.cached_current_units, 1);
    assert_eq!(status.searchable_current_units, 0);
    assert!(
        status
            .last_error
            .as_ref()
            .unwrap()
            .message
            .contains("not published"),
        "{:?}",
        status.last_error
    );
    // The next run publishes the pending coverage BEFORE any inference and
    // embeds nothing (everything is cached).
    let before = env.calls();
    let again = env.prepare(60);
    assert!(!again.partial, "{again:?}");
    assert_eq!(env.calls(), before);
    assert!(again.index_published);
    assert_eq!(env.status().searchable_current_units, 1);
}

#[test]
fn mapping_acceptance_validates_the_current_body() {
    use context_foundry::neural::cache::PartitionUnit;
    let a_text = body(100);
    let u_text = "é".repeat(10);
    let (mut env, _) = first_preparation(&[("a.txt", &a_text), ("u.txt", &u_text), ("e.txt", "")]);
    let digest = env.digest();
    let recipe = partition::recipe_id("fake-bytes 1");
    let key_of = |text: &str| provider::input_key(&digest, &provider::render_document(text));
    // Units whose keys are the TRUE identity of their range of `text`, so a
    // structural refusal below is about structure, not about the key.
    let units_of = |text: &str, ranges: &[(usize, usize)]| -> Vec<PartitionUnit> {
        ranges
            .iter()
            .map(|&(start, end)| PartitionUnit {
                start,
                end,
                input_key: key_of(text.get(start..end).unwrap_or("?")),
            })
            .collect()
    };
    let engine = env.open();
    let hash_of = |path: &str| engine.source(path).unwrap().unwrap().hash;
    let record = |hash: String, units: Vec<PartitionUnit>| PartitionRecord {
        source_hash: hash,
        recipe_id: recipe.clone(),
        function_digest: digest.clone(),
        units,
    };
    let refused = |path: &str, units: Vec<PartitionUnit>, why: &str| {
        let err = engine
            .semantic_record_partition(path, &record(hash_of(path), units))
            .unwrap_err();
        assert_eq!(err.code(), "partition_invalid", "{why}: {err}");
    };
    refused("a.txt", units_of(&a_text, &[(0, 101)]), "past the end");
    refused("a.txt", units_of(&a_text, &[(0, 50), (60, 100)]), "a gap");
    refused(
        "a.txt",
        units_of(&a_text, &[(0, 60), (50, 100)]),
        "an overlap",
    );
    refused("a.txt", units_of(&a_text, &[(0, 99)]), "a short cover");
    refused("a.txt", Vec::new(), "empty for a nonempty source");
    refused(
        "u.txt",
        units_of(&u_text, &[(0, 1), (1, 20)]),
        "inside a character",
    );
    refused(
        "e.txt",
        units_of("", &[(0, 1)]),
        "units for an empty source",
    );
    // A mapping naming a stale source version is refused.
    let stale = record("0".repeat(64), units_of(&a_text, &[(0, 100)]));
    assert_eq!(
        engine
            .semantic_record_partition("a.txt", &stale)
            .unwrap_err()
            .code(),
        "partition_invalid"
    );
    // A malformed key, an ARBITRARY well-shaped key, and ANOTHER source's
    // valid key are all refused: a key must be the identity of its exact
    // rendered bytes.
    let with_key = |key: &str| {
        vec![PartitionUnit {
            start: 0,
            end: 100,
            input_key: key.to_owned(),
        }]
    };
    refused("a.txt", with_key("short"), "a malformed key");
    refused("a.txt", with_key(&"ab".repeat(32)), "an arbitrary key");
    refused("a.txt", with_key(&key_of(&u_text)), "another source's key");
    // Legal mappings (an exact cover under the true key; an empty list for
    // an empty source).
    engine
        .semantic_record_partition(
            "a.txt",
            &record(hash_of("a.txt"), units_of(&a_text, &[(0, 100)])),
        )
        .unwrap();
    engine
        .semantic_record_partition("e.txt", &record(hash_of("e.txt"), Vec::new()))
        .unwrap();
}

#[test]
fn a_stored_mapping_with_a_bad_range_is_ineligible_not_a_panic() {
    let (mut env, _) = first_preparation(&[("a.txt", &body(100))]);
    let digest = env.digest();
    let recipe = partition::recipe_id("fake-bytes 1");
    let hash = env.open().source("a.txt").unwrap().unwrap().hash;
    let good = env.open().semantic_partition("a.txt").unwrap().unwrap();
    env.close();
    let bad = serde_json::json!({
        "source_hash": hash,
        "recipe_id": recipe,
        "function_digest": digest,
        "units": [{"start": 0, "end": 101, "input_key": "ab".repeat(32)}]
    });
    testkit::write_raw_partition(&env.store, "a.txt", &bad.to_string());
    let status = env.status();
    assert_eq!(
        status.unpartitioned_sources, 1,
        "range constraints are rechecked at every eligibility lookup"
    );
    assert_eq!(status.eligible_units, 0);
    // Preparation repartitions the source instead of slicing out of range.
    let report = env.prepare(60);
    assert!(!report.partial, "{report:?}");
    assert_eq!(report.partitioned_sources, 1);
    assert_eq!(report.document_calls, 0, "the vector was cached");
    let repaired = env.open().semantic_partition("a.txt").unwrap().unwrap();
    assert_eq!(repaired.units[0].input_key, good.units[0].input_key);
    assert_eq!(repaired.units[0].end, 100);
}

#[test]
fn status_honors_its_deadline_in_validation_and_the_census() {
    use context_foundry::fault::{self, Action};
    use context_foundry::neural::status::fault_names;
    let (mut env, _) = first_preparation(&[("a.txt", &body(100))]);
    // During generation validation.
    fault::arm(
        fault_names::BEFORE_VALIDATION,
        0,
        Action::Delay(Duration::from_millis(600)),
    );
    let control = Control::with_deadline(Instant::now() + Duration::from_millis(300));
    let error = env.open().semantic_status(&control).unwrap_err();
    fault::disarm_all();
    assert_eq!(error.code(), "deadline_exceeded");
    // During the census of a retained-cache-only store (its sources gone).
    env.open_mut().delete_source("a.txt").unwrap();
    fault::arm(
        fault_names::BEFORE_CENSUS,
        0,
        Action::Delay(Duration::from_millis(600)),
    );
    let control = Control::with_deadline(Instant::now() + Duration::from_millis(300));
    let error = env.open().semantic_status(&control).unwrap_err();
    fault::disarm_all();
    assert_eq!(error.code(), "deadline_exceeded");
    // Unbounded, the same store reads back honestly: retention, empty corpus.
    let status = env.status();
    assert_eq!(status.cache.orphan_entries, 1);
    assert_eq!(status.corpus, "empty");
}

#[test]
fn status_names_the_workspace_and_reads_a_dead_owners_running_state_as_stopped() {
    let (mut env, _) = first_preparation(&[("a.txt", &body(100))]);
    let status = env.status();
    assert_eq!(status.workspace_id.as_deref().map(str::len), Some(64));
    assert!(status.source_revision >= 1);
    let mut value = serde_json::to_value(env.open().semantic_state().unwrap().unwrap()).unwrap();
    env.close();
    // A dead owner leaves `running` behind.
    value["state"] = "running".into();
    testkit::tamper_semantic_state(&env.store, &value.to_string());
    let status = env.status();
    assert_eq!(status.state, "stopped");
    assert_eq!(status.last_error.as_ref().unwrap().code, "interrupted");
    assert_eq!(env.calls(), 1, "reading status started no preparation");
}

// --- review round 3 ----------------------------------------------------------

/// An outside tree with a real `<digest>` child, to be reached only if a
/// destructive call follows a substituted path.
fn outside_with_generation(digest: &str) -> tempfile::TempDir {
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(outside.path().join(digest).join("inner")).unwrap();
    std::fs::write(outside.path().join(digest).join("inner/keep.txt"), b"keep").unwrap();
    std::fs::write(outside.path().join("top.txt"), b"top").unwrap();
    outside
}

fn assert_generation_outside_untouched(outside: &tempfile::TempDir, digest: &str) {
    assert_eq!(
        std::fs::read(outside.path().join(digest).join("inner/keep.txt")).unwrap(),
        b"keep"
    );
    assert_eq!(
        std::fs::read(outside.path().join("top.txt")).unwrap(),
        b"top"
    );
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 2);
    assert_eq!(
        std::fs::read_dir(outside.path().join(digest))
            .unwrap()
            .count(),
        1,
        "nothing was written into the outside generation"
    );
}

#[test]
fn purge_deletion_is_anchored_to_the_descriptor_not_the_path() {
    use context_foundry::fault::{self, Action};
    let (mut env, _) = first_preparation(&[("a.txt", &body(100))]);
    env.close();
    let digest = env.digest();
    let outside = outside_with_generation(&digest);
    let root = env.store.join("semantic");
    let moved = env.store.join("semantic-moved");
    // After the plan and the row removal, BEFORE the deletion: swap the
    // semantic root for a symlink to the outside tree.
    let (hook_root, hook_moved, hook_outside) =
        (root.clone(), moved.clone(), outside.path().to_owned());
    fault::arm(
        context_foundry::neural::fault_names::PURGE_AFTER_PLAN,
        0,
        Action::Call(Box::new(move |_ctx| {
            std::fs::rename(&hook_root, &hook_moved).unwrap();
            std::os::unix::fs::symlink(&hook_outside, &hook_root).unwrap();
        })),
    );
    let engine = Engine::open_existing(&env.store).unwrap();
    let report = engine.semantic_purge().unwrap();
    fault::disarm_all();
    drop(engine);
    assert_eq!(report.removed_generation_dirs, 1);
    assert_generation_outside_untouched(&outside, &digest);
    assert_eq!(
        std::fs::read_dir(&moved).unwrap().count(),
        0,
        "the ORIGINAL directory, held by descriptor, was the one emptied"
    );
}

#[test]
fn publication_is_anchored_to_the_descriptors_not_the_path() {
    use context_foundry::fault::{self, Action};
    let (mut env, _) = first_preparation(&[("a.txt", &body(100))]);
    env.close();
    let digest = env.digest();
    let outside = outside_with_generation(&digest);
    let root = env.store.join("semantic");
    let moved = env.store.join("semantic-moved");
    // After the check (the semantic root and generation directory are open),
    // BEFORE anything is written: swap the root for a symlink.
    let (hook_root, hook_moved, hook_outside) =
        (root.clone(), moved.clone(), outside.path().to_owned());
    fault::arm(
        context_foundry::neural::fault_names::PUBLISH_AFTER_CHECK,
        0,
        Action::Call(Box::new(move |_ctx| {
            std::fs::rename(&hook_root, &hook_moved).unwrap();
            std::os::unix::fs::symlink(&hook_outside, &hook_root).unwrap();
        })),
    );
    let store = context_foundry::neural::anchor::Dir::open_path(&env.store).unwrap();
    let count = index::build_generation(
        &store,
        &digest,
        "recipe",
        &one_entry(),
        &Control::unbounded(),
    )
    .unwrap();
    fault::disarm_all();
    assert_eq!(count, 1);
    assert_generation_outside_untouched(&outside, &digest);
    // The publication landed in the ORIGINAL (renamed) directory.
    let published: Vec<_> = std::fs::read_dir(moved.join(&digest))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(published.len(), 3, "{published:?}");
}

/// C1 bootstrap binding: the store-directory descriptor is taken ONCE, when
/// the Engine binds the store, and never re-resolved. Here `/parent/work` —
/// an ANCESTOR of the store — is renamed away after the bind and replaced by
/// a symlink to an outside tree that carries a plausible `store/semantic/
/// <digest>` with files. Purge and publication on the STILL-OPEN Engine act
/// on the bound directory; the outside tree is untouched.
#[test]
fn purge_and_publication_follow_the_bound_store_not_a_substituted_ancestor() {
    let parent = tempfile::tempdir().unwrap();
    let work = parent.path().join("work");
    let store = work.join("store");
    let ws = parent.path().join("ws");
    std::fs::create_dir(&ws).unwrap();
    let mut init = Engine::initialize(&store, &ws).unwrap();
    init.replace_source("a.txt", &body(100)).unwrap();
    init.refresh(&Control::unbounded()).unwrap();
    drop(init);
    let profile = testkit::write_semantic_profile(parent.path(), "fake-a", |_| {});
    let loaded = SemanticProfile::load(&profile).unwrap();
    let factory = FakeFactory::new(loaded.descriptor.clone(), FakeBehavior::Ok);
    let report = prepare::run(
        &store,
        &PrepareOptions {
            profile_path: &profile,
            budget_seconds: 60,
            development: false,
            cache_cap_bytes: DEFAULT_CACHE_CAP_BYTES,
            started: Instant::now(),
            control: &Control::unbounded(),
        },
        factory.acquire(),
    )
    .expect("preparation runs");
    assert!(report.index_published, "{report:?}");
    let digest = loaded.descriptor.digest();

    // The decoy: an outside tree with a full-looking generation of its own.
    let outside = tempfile::tempdir().unwrap();
    let decoy = outside.path().join("store/semantic").join(&digest);
    std::fs::create_dir_all(&decoy).unwrap();
    std::fs::write(decoy.join(index::INDEX_FILE), b"decoy-index").unwrap();
    std::fs::write(decoy.join(index::LABELS_FILE), b"decoy-labels").unwrap();
    std::fs::write(decoy.join(index::MANIFEST_FILE), b"decoy-manifest").unwrap();

    // Bind BEFORE the substitution, then keep the Engine open across it.
    let engine = Engine::open_existing(&store).unwrap();
    let moved = parent.path().join("work-moved");
    std::fs::rename(&work, &moved).unwrap();
    std::os::unix::fs::symlink(outside.path(), &work).unwrap();
    // Sanity: the substituted pathname now resolves to the decoy generation.
    assert!(
        work.join("store/semantic")
            .join(&digest)
            .join(index::INDEX_FILE)
            .exists()
    );

    // Publication on the still-open Engine: the bound generation's index file
    // is removed (reached through the moved path), so pending coverage must
    // be rebuilt — into the bound directory, not into the decoy.
    std::fs::remove_file(
        moved
            .join("store/semantic")
            .join(&digest)
            .join(index::INDEX_FILE),
    )
    .unwrap();
    match engine
        .semantic_publish_pending(&Control::unbounded())
        .unwrap()
    {
        index::Publication::Rebuilt(entries) => assert_eq!(entries, 1),
        other => panic!("expected a rebuild into the bound store, got {other:?}"),
    }
    assert!(
        moved
            .join("store/semantic")
            .join(&digest)
            .join(index::INDEX_FILE)
            .exists(),
        "the bound store's generation was rebuilt"
    );
    assert_eq!(
        std::fs::read(decoy.join(index::INDEX_FILE)).unwrap(),
        b"decoy-index"
    );

    // Purge on the still-open Engine: the bound store's semantic tree is
    // deleted; every decoy file survives.
    let purged = engine.semantic_purge().unwrap();
    assert_eq!(purged.removed_generation_dirs, 1, "{purged:?}");
    assert_eq!(
        std::fs::read_dir(moved.join("store/semantic"))
            .unwrap()
            .count(),
        0,
        "the ORIGINAL store, held by descriptor, is the one emptied"
    );
    assert_eq!(
        std::fs::read(decoy.join(index::INDEX_FILE)).unwrap(),
        b"decoy-index"
    );
    assert_eq!(
        std::fs::read(decoy.join(index::LABELS_FILE)).unwrap(),
        b"decoy-labels"
    );
    assert_eq!(
        std::fs::read(decoy.join(index::MANIFEST_FILE)).unwrap(),
        b"decoy-manifest"
    );
    drop(engine);
}

/// Stretch the profile's weights file into a huge sparse file so hashing it
/// outlasts any test budget; the stop must come from the control.
fn grow_weights(env: &Env) {
    let model_dir = SemanticProfile::load(&env.profile).unwrap().model_dir;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(model_dir.join("model.safetensors"))
        .unwrap();
    file.set_len(16 << 30).unwrap();
}

#[test]
fn artifact_verification_obeys_the_run_budget() {
    let mut env = Env::new(&[("a.txt", &body(100))]);
    grow_weights(&env);
    let started = Instant::now();
    let report = env.prepare(1);
    assert!(
        started.elapsed() < Duration::from_secs(6),
        "verification was not bounded: {:?}",
        started.elapsed()
    );
    assert!(report.partial, "{report:?}");
    assert_eq!(report.reason_code, Some("budget_exhausted"), "{report:?}");
    assert_eq!(report.partitioned_sources, 0);
    assert_eq!(env.calls(), 0);
    assert_eq!(env.open().semantic_cache_totals().unwrap().0, 0);
    assert!(env.open().source("a.txt").unwrap().is_some());
}

#[test]
fn artifact_verification_obeys_caller_cancellation() {
    let mut env = Env::new(&[("a.txt", &body(100))]);
    grow_weights(&env);
    let control = Control::unbounded();
    let flag = control.cancel_flag();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    env.engine = None;
    let profile = env.profile.clone();
    let started = Instant::now();
    let report = prepare::run(
        &env.store,
        &PrepareOptions {
            profile_path: &profile,
            budget_seconds: 60,
            development: false,
            cache_cap_bytes: DEFAULT_CACHE_CAP_BYTES,
            started: Instant::now(),
            control: &control,
        },
        env.factory.acquire(),
    )
    .expect("a cancelled verification is a named stop");
    canceller.join().unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "cancellation did not reach verification"
    );
    assert_eq!(report.reason_code, Some("cancelled"), "{report:?}");
    assert_eq!(report.partitioned_sources, 0);
    assert_eq!(env.calls(), 0);
}

/// Nine sources, a first run that commits one batch and fails right after
/// the commit (no publication happens), leaving 8 committed-but-unpublished
/// vectors. Returns the env with that state.
fn committed_but_unpublished() -> Env {
    use context_foundry::fault::{self, Action};
    let mut env = env_with(&nine_sources(), FakeBehavior::Ok);
    fault::arm(
        prepare::fault_names::CACHE_AFTER_COMMIT,
        0,
        Action::Fail("injected".into()),
    );
    let error = env.prepare_err(60);
    fault::disarm_all();
    assert_eq!(error.code(), "internal");
    assert_eq!(env.open().semantic_cache_totals().unwrap().0, 8);
    assert_eq!(env.status().searchable_current_units, 0);
    env
}

#[test]
fn every_run_publishes_pending_coverage_before_admitting_new_inference() {
    let mut env = committed_but_unpublished();
    // The very first inference call of the next run fails. Nothing new is
    // committed, so ONLY the pre-inference publication can make the earlier
    // 8 vectors searchable.
    let loaded = SemanticProfile::load(&env.profile).unwrap();
    env.factory = FakeFactory::new(loaded.descriptor.clone(), FakeBehavior::TimeoutOnCall(1));
    let report = env.prepare(60);
    assert!(report.partial, "{report:?}");
    assert_eq!(report.reason_code, Some("provider_timeout"));
    assert_eq!(report.embedded_units, 0);
    assert!(report.index_published, "{report:?}");
    assert_eq!(env.status().searchable_current_units, 8);
}

#[test]
fn publication_cut_off_by_the_budget_records_its_reason_and_keeps_the_vectors() {
    use context_foundry::fault::{self, Action};
    let mut env = committed_but_unpublished();
    // A 2 s budget (inside the 5 s reserve, so no inference is admitted);
    // the publication hook stalls past it.
    fault::arm(
        prepare::fault_names::BEFORE_PUBLISH,
        0,
        Action::Delay(Duration::from_millis(2500)),
    );
    let report = env.prepare(2);
    fault::disarm_all();
    assert!(report.partial, "{report:?}");
    assert_eq!(report.reason_code, Some("budget_exhausted"));
    assert!(!report.index_published, "{report:?}");
    let reason = report.index_reason.clone().expect("the reason is recorded");
    assert!(reason.contains("publication was cut off"), "{reason}");
    assert_eq!(report.document_calls, 0, "publication needs no inference");
    assert_eq!(env.open().semantic_cache_totals().unwrap().0, 8);
    let status = env.status();
    assert_eq!(status.state, "paused");
    assert!(
        status
            .last_error
            .as_ref()
            .unwrap()
            .message
            .contains("publication was cut off"),
        "{:?}",
        status.last_error
    );
    assert_eq!(status.searchable_current_units, 0);
    assert_eq!(status.cached_current_units, 8);
}

#[test]
fn a_same_length_nonfinite_cache_row_is_named_by_the_lookup_and_replaced_by_prepare() {
    let (mut env, _) = first_preparation(&[("a.txt", &body(100))]);
    env.close();
    let digest = env.digest();
    let (key, mut bytes) = testkit::semantic_cache_rows(&env.store)
        .into_iter()
        .next()
        .unwrap();
    // Keep the length and the stored digest; poison one component.
    bytes[64..68].copy_from_slice(&f32::NAN.to_le_bytes());
    testkit::tamper_semantic_cache_row(&env.store, &key, &bytes);
    // Status trusts committed metadata and does not scan payloads: it still
    // counts the row (the documented limitation).
    let status = env.status();
    assert_eq!(status.cached_current_units, 1);
    assert_eq!(status.cache.corrupt_entries, 0);
    // The lookup that USES the vector names it.
    match env.open().semantic_cache_lookup(&key, &digest).unwrap() {
        CacheLookup::Corrupt(message) => assert!(message.contains("nonfinite"), "{message}"),
        other => panic!("expected named corruption, got {other:?}"),
    }
    // Preparation disables it by name and replaces it.
    let before = env.calls();
    let report = env.prepare(60);
    assert!(!report.partial, "{report:?}");
    assert_eq!(report.corrupt_cache_rows, 1);
    assert_eq!(report.document_calls, 1);
    assert_eq!(env.calls(), before + 1);
    assert_eq!(report.embedded_units, 1);
    assert!(matches!(
        env.open().semantic_cache_lookup(&key, &digest).unwrap(),
        CacheLookup::Hit(_)
    ));
}

#[test]
fn status_reports_the_last_provider_observation_never_a_live_probe() {
    // A fresh store: nothing observed.
    let mut env = Env::new(&[("a.txt", &body(100))]);
    let status = env.status();
    assert_eq!(status.provider.state, "unknown");
    assert!(status.provider.observed_at_unix.is_none());
    assert!(status.provider.profile.is_none());
    // A successful run observed `ready`, with its time and profile.
    env.prepare(60);
    let status = env.status();
    assert_eq!(status.provider.state, "ready");
    assert_eq!(status.provider.profile.as_deref(), Some("fake-a"));
    assert!(status.provider.observed_at_unix.is_some());
    // A failed batch is observed as `failed` with its code.
    let mut failing = env_with(&nine_sources(), FakeBehavior::TimeoutOnCall(1));
    failing.prepare(60);
    let status = failing.status();
    assert_eq!(status.provider.state, "failed");
    assert_eq!(status.provider.code.as_deref(), Some("provider_timeout"));
    // A refused acquisition is observed as `refused`.
    let mut refused = Env::new(&[("a.txt", &body(100))]);
    refused.engine = None;
    let profile = refused.profile.clone();
    let error = prepare::run(
        &refused.store,
        &PrepareOptions {
            profile_path: &profile,
            budget_seconds: 60,
            development: false,
            cache_cap_bytes: DEFAULT_CACHE_CAP_BYTES,
            started: Instant::now(),
            control: &Control::unbounded(),
        },
        Box::new(|_profile, _development, _control| {
            Err(ProviderError::IsolationUnavailable("no profile".into()))
        }),
    )
    .unwrap_err();
    assert_eq!(error.code(), "isolation_unavailable");
    let status = refused.status();
    assert_eq!(status.provider.state, "refused");
    assert_eq!(
        status.provider.code.as_deref(),
        Some("isolation_unavailable")
    );
    assert_eq!(status.state, "paused");
    // Status itself never starts preparation or probes the provider.
    let calls = failing.calls();
    let _ = failing.status();
    assert_eq!(failing.calls(), calls);
}
