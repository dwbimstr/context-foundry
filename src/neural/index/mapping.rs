//! A disposable, owner-local view of current partition locations. redb remains
//! authoritative; restart, overflow, profile change and explicit repair reconcile.
use super::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

const MAX_DIRTY: usize = 1024;
const MAX_DIRTY_BYTES: usize = 1024 * 1024;

#[derive(Default)]
pub(crate) struct Journal {
    serial: u64,
    epoch: u64,
    paths: BTreeMap<String, u64>,
    keys: BTreeMap<String, u64>,
    bytes: usize,
    view: Option<View>,
}
impl Journal {
    pub(crate) fn commit<'a>(
        &mut self,
        tx: redb::WriteTransaction,
        paths: impl IntoIterator<Item = &'a str>,
        keys: impl IntoIterator<Item = &'a str>,
        reset: bool,
    ) -> FResult<()> {
        let next = self
            .serial
            .checked_add(1)
            .ok_or(FoundryError::RevisionExhausted)?;
        tx.commit()?;
        self.serial = next;
        if reset {
            self.reset();
            return Ok(());
        }
        for (is_path, value) in paths
            .into_iter()
            .map(|p| (true, p))
            .chain(keys.into_iter().map(|k| (false, k)))
        {
            let table = if is_path {
                &mut self.paths
            } else {
                &mut self.keys
            };
            if table.insert(value.to_owned(), next).is_none() {
                self.bytes += value.len();
            }
            if self.paths.len() + self.keys.len() > MAX_DIRTY || self.bytes > MAX_DIRTY_BYTES {
                self.reset();
            }
        }
        Ok(())
    }
    fn reset(&mut self) {
        self.epoch = self.serial;
        self.paths.clear();
        self.keys.clear();
        self.bytes = 0;
        self.view = None;
    }
}

#[derive(Default)]
struct View {
    ticket: u64,
    digest: String,
    recipe: String,
    rows: BTreeMap<String, cache::PartitionRecord>,
    reverse: BTreeMap<String, BTreeSet<String>>,
    units: Locations,
    eligible: u64,
    cached: u64,
    bytes: u64,
}
impl View {
    fn remove(&mut self, path: &str) {
        let Some(record) = self.rows.remove(path) else {
            return;
        };
        self.eligible -= record.units.len() as u64;
        self.bytes -= row_bytes(path, &record);
        for unit in &record.units {
            if let Some(paths) = self.reverse.get_mut(&unit.input_key) {
                paths.remove(path);
                if paths.is_empty() {
                    self.reverse.remove(&unit.input_key);
                }
            }
            if let Some(units) = self.units.get_mut(&unit.input_key) {
                let units = std::sync::Arc::make_mut(units);
                let before = units.len();
                units.retain(|unit| unit.path != path);
                self.cached -= (before - units.len()) as u64;
                if units.is_empty() {
                    self.units.remove(&unit.input_key);
                }
            }
        }
    }
    fn refresh(
        &mut self,
        engine: &Engine,
        path: &str,
        probed: &mut HashMap<String, bool>,
        control: &Control,
    ) -> FResult<()> {
        control.check()?;
        self.remove(path);
        let Some(meta) = engine.source(path)? else {
            return Ok(());
        };
        let Some(record) = engine.semantic_partition(path)?.filter(|record| {
            cache::partition_is_current(record, &meta, &self.recipe, &self.digest)
        }) else {
            return Ok(());
        };
        self.bytes = self.bytes.saturating_add(row_bytes(path, &record));
        within_publication_limit(self.bytes, MAX_META_BYTES, "mapping view")?;
        self.eligible += record.units.len() as u64;
        for unit in &record.units {
            self.reverse
                .entry(unit.input_key.clone())
                .or_default()
                .insert(path.to_owned());
            let cached = match probed.get(&unit.input_key) {
                Some(&cached) => cached,
                None => {
                    neural_fault!(MAPPING_CACHE_PROBE, Some(control), &unit.input_key)?;
                    let cached = engine.semantic_cache_probe(&unit.input_key, &self.digest)?
                        == CacheProbe::Current;
                    probed.insert(unit.input_key.clone(), cached);
                    cached
                }
            };
            if cached {
                self.cached += 1;
                let units = self.units.entry(unit.input_key.clone()).or_default();
                let units = std::sync::Arc::make_mut(units);
                let location = UnitLocation {
                    path: path.to_owned(),
                    start: unit.start as u64,
                    end: unit.end as u64,
                    source_sha256: record.source_hash.clone(),
                };
                let order = |a: &UnitLocation| {
                    (&a.path, a.start, a.end).cmp(&(&location.path, location.start, location.end))
                };
                let position = if units.last().is_none_or(|a| !order(a).is_gt()) {
                    units.len()
                } else {
                    units.partition_point(|a| order(a).is_lt())
                };
                units.insert(position, location);
            }
        }
        self.rows.insert(path.to_owned(), record);
        Ok(())
    }
}
fn row_bytes(path: &str, record: &cache::PartitionRecord) -> u64 {
    // Conservative logical retained payload: partition, reverse references,
    // locations, keys and path strings. Allocator/tree overhead is separate.
    256 + path.len() as u64 * 6
        + record
            .units
            .iter()
            .map(|unit| {
                location_bytes(path, &record.source_hash) * 2
                    + unit.input_key.len() as u64 * 12
                    + 192
            })
            .sum::<u64>()
}

pub(crate) struct MappingWalk {
    view: View,
    ticket: u64,
    epoch: u64,
    source_revision: u64,
    mapping_revision: u64,
    sources: u64,
    full: bool,
    after: Option<String>,
    dirty: VecDeque<String>,
    probed: HashMap<String, bool>,
}
impl MappingWalk {
    pub(crate) fn start(engine: &Engine, digest: &str, recipe: &str) -> FResult<Self> {
        Self::begin(engine, digest, recipe, false)
    }
    pub(crate) fn audit(engine: &Engine, digest: &str, recipe: &str) -> FResult<Self> {
        Self::begin(engine, digest, recipe, true)
    }
    fn begin(engine: &Engine, digest: &str, recipe: &str, audit: bool) -> FResult<Self> {
        let mut state = engine
            .semantic_mapping
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let view = if audit {
            None
        } else {
            state
                .view
                .take()
                .filter(|v| v.digest == digest && v.recipe == recipe)
        };
        let mut dirty: BTreeSet<_> = state.paths.keys().cloned().collect();
        if let Some(view) = &view {
            for key in state.keys.keys() {
                if let Some(paths) = view.reverse.get(key) {
                    for path in paths {
                        dirty.insert(path.clone());
                        if dirty.len() > MAX_DIRTY {
                            break;
                        }
                    }
                }
                if dirty.len() > MAX_DIRTY {
                    break;
                }
            }
        }
        let full = view.is_none() || dirty.len() > MAX_DIRTY;
        let view = if full {
            View {
                digest: digest.into(),
                recipe: recipe.into(),
                ..Default::default()
            }
        } else {
            view.unwrap()
        };
        Ok(Self {
            view,
            ticket: state.serial,
            epoch: state.epoch,
            source_revision: engine.source_revision()?,
            mapping_revision: engine.semantic_mapping_revision()?,
            sources: cache::source_count(&engine.db)?,
            full,
            after: None,
            dirty: dirty.into_iter().collect(),
            probed: HashMap::new(),
        })
    }
    pub(crate) fn step(
        &mut self,
        engine: &Engine,
        control: &Control,
        until: Option<Instant>,
    ) -> FResult<bool> {
        loop {
            control.check()?;
            let page = if self.full {
                neural_fault!(MAPPING_FULL_PAGE, Some(control), "partition page")?;
                cache::partition_paths(&engine.db, self.after.as_deref())?
            } else {
                self.dirty
                    .drain(..self.dirty.len().min(cache::PAGE))
                    .collect()
            };
            if page.is_empty() {
                return Ok(true);
            }
            for path in &page {
                self.view.refresh(engine, path, &mut self.probed, control)?;
            }
            self.after = page.last().cloned();
            if until.is_some_and(|until| Instant::now() >= until) {
                return Ok(false);
            }
        }
    }
    pub(crate) fn finish(mut self, engine: &Engine) -> FResult<CurrentMapping> {
        let mut state = engine
            .semantic_mapping
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let stable = self.epoch == state.epoch
            && self.ticket == state.serial
            && self.source_revision == engine.source_revision()?;
        let coverage = cache::CoverageSnapshot {
            version: 1,
            source_revision: self.source_revision,
            function_digest: self.view.digest.clone(),
            recipe_id: self.view.recipe.clone(),
            sources: self.sources,
            unpartitioned_sources: self.sources.saturating_sub(self.view.rows.len() as u64),
            eligible_units: self.view.eligible,
            cached_current_units: self.view.cached,
            searchable_current_units: self.view.cached,
            index_entries: self.view.units.len() as u64,
        };
        let complete = stable
            && self.view.rows.len() as u64 == self.sources
            && self.view.eligible == self.view.cached;
        let units = self.view.units.clone();
        if self.epoch == state.epoch
            && state
                .view
                .as_ref()
                .is_none_or(|view| view.ticket <= self.ticket)
        {
            self.view.ticket = self.ticket;
            state.view = Some(self.view);
            state.paths.retain(|_, serial| *serial > self.ticket);
            state.keys.retain(|_, serial| *serial > self.ticket);
            state.bytes = state
                .paths
                .keys()
                .chain(state.keys.keys())
                .map(String::len)
                .sum();
        }
        Ok(CurrentMapping {
            coverage,
            units,
            mapping_revision: stable.then_some(self.mapping_revision),
            scope: GenerationScope {
                source_revision: self.source_revision,
                complete,
            },
        })
    }
}

#[cfg(all(test, feature = "test-faults"))]
mod tests {
    use super::*;
    use crate::fault;
    use crate::neural::fault_names::{MAPPING_CACHE_PROBE, MAPPING_FULL_PAGE, PARTITION_READ};

    fn digest() -> String {
        "ab".repeat(32)
    }
    fn record(engine: &Engine, path: &str, key: usize) {
        let hash = engine.source(path).unwrap().unwrap().hash;
        engine
            .semantic_record_partition(path, &hash, "recipe", &digest(), None, &|body| {
                Ok(vec![cache::PartitionUnit {
                    start: 0,
                    end: body.len(),
                    input_key: format!("{key:064x}"),
                }])
            })
            .unwrap();
    }
    fn cache(engine: &Engine, key: usize) {
        let mut vector = vec![0.0; 128];
        vector[0] = 1.0;
        engine
            .semantic_cache_commit(
                &[(format!("{key:064x}"), vector)],
                &digest(),
                128,
                cache::DEFAULT_CACHE_CAP_BYTES,
            )
            .unwrap();
    }
    fn walk(engine: &Engine) -> CurrentMapping {
        let mut walk = MappingWalk::start(engine, &digest(), "recipe").unwrap();
        assert!(walk.step(engine, &Control::unbounded(), None).unwrap());
        walk.finish(engine).unwrap()
    }
    fn fixture(count: usize) -> (tempfile::TempDir, Engine) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
        for i in 0..count {
            let path = format!("f{i:04}.md");
            engine.replace_source(&path, "current body\n").unwrap();
            record(&engine, &path, i);
        }
        (dir, engine)
    }

    #[test]
    fn committed_paths_and_shared_cache_keys_refresh_only_affected_locations() {
        let (_dir, engine) = fixture(260);
        let initial = walk(&engine);
        assert_eq!(initial.coverage.eligible_units, 260);
        let before = (
            fault::reached(MAPPING_FULL_PAGE),
            fault::reached(PARTITION_READ),
            fault::reached(MAPPING_CACHE_PROBE),
        );
        assert_eq!(walk(&engine).units.len(), 0);
        assert_eq!(
            before,
            (
                fault::reached(MAPPING_FULL_PAGE),
                fault::reached(PARTITION_READ),
                fault::reached(MAPPING_CACHE_PROBE)
            )
        );
        // Cache completion has no source revision change, but refreshes both users.
        record(&engine, "f0001.md", 0);
        walk(&engine);
        let reads = fault::reached(PARTITION_READ);
        cache(&engine, 0);
        let shared = walk(&engine);
        assert_eq!(fault::reached(PARTITION_READ) - reads, 2);
        assert_eq!(shared.units[&format!("{:064x}", 0)].len(), 2);
        engine.delete_source("f0000.md").unwrap();
        engine.replace_source("f0001.md", "changed body\n").unwrap();
        let stale = walk(&engine);
        assert!(stale.units.is_empty());
        assert_eq!(stale.coverage.unpartitioned_sources, 1);
        record(&engine, "f0001.md", 0);
        let current = walk(&engine);
        assert_eq!(current.coverage.cached_current_units, 1);
        assert_eq!(
            current.units.values().next().unwrap()[0].source_sha256,
            engine.source("f0001.md").unwrap().unwrap().hash
        );
        assert_eq!(
            fault::reached(MAPPING_FULL_PAGE),
            before.0,
            "no warm census"
        );
        let audit = engine
            .semantic_current_mapping(&Control::unbounded(), &digest(), "recipe")
            .unwrap();
        assert_eq!(current.units, audit.units);
        assert!(
            fault::reached(MAPPING_FULL_PAGE) > before.0,
            "explicit audit still scans"
        );
    }

    #[test]
    fn interrupted_walk_and_changes_between_pages_cannot_lose_committed_work() {
        let (_dir, engine) = fixture(260);
        let mut pending = MappingWalk::start(&engine, &digest(), "recipe").unwrap();
        assert!(
            !pending
                .step(&engine, &Control::unbounded(), Some(Instant::now()))
                .unwrap()
        );
        engine
            .replace_source("f0000.md", "changed after first page\n")
            .unwrap();
        record(&engine, "f0000.md", 900);
        cache(&engine, 900);
        assert!(pending.step(&engine, &Control::unbounded(), None).unwrap());
        assert!(!pending.finish(&engine).unwrap().scope.complete);
        let reads = fault::reached(PARTITION_READ);
        let refreshed = walk(&engine);
        assert_eq!(fault::reached(PARTITION_READ) - reads, 1);
        assert!(refreshed.units.contains_key(&format!("{:064x}", 900)));
        let mut cancelled = MappingWalk::start(&engine, &digest(), "recipe").unwrap();
        assert!(
            cancelled
                .step(&engine, &Control::cancelled(), None)
                .is_err()
        );
        drop(cancelled);
        let pages = fault::reached(MAPPING_FULL_PAGE);
        assert_eq!(walk(&engine).units, refreshed.units);
        assert!(fault::reached(MAPPING_FULL_PAGE) > pages);
    }

    #[test]
    fn scan_sweeps_only_dirty_retired_paths_and_failed_source_commits_stay_clean() {
        let (_dir, engine) = fixture(2);
        cache(&engine, 0);
        cache(&engine, 1);
        walk(&engine);
        engine
            .mark_seen_batch(&["f0000.md".into(), "f0001.md".into()], 1)
            .unwrap();
        engine.sweep_unseen(1, &Control::unbounded()).unwrap();
        let reads = fault::reached(PARTITION_READ);
        assert!(walk(&engine).scope.complete);
        assert_eq!(
            fault::reached(PARTITION_READ),
            reads,
            "unchanged scan leaves mapping clean"
        );
        let revision = engine.semantic_mapping_revision().unwrap();
        fault::arm(
            crate::fault::names::SOURCE_BEFORE_COMMIT,
            0,
            crate::fault::Action::Fail("commit failed".into()),
        );
        assert!(engine.replace_source("f0000.md", "uncommitted").is_err());
        assert_eq!(engine.semantic_mapping_revision().unwrap(), revision);
        fault::disarm_all();
        let reads = fault::reached(PARTITION_READ);
        walk(&engine);
        assert_eq!(fault::reached(PARTITION_READ), reads);
        engine.mark_seen_batch(&["f0001.md".into()], 2).unwrap();
        assert_eq!(engine.sweep_unseen(2, &Control::unbounded()).unwrap().0, 1);
        let after = walk(&engine);
        assert!(after.scope.complete);
        assert_eq!(after.coverage.sources, 1);
        assert_eq!(after.units.len(), 1);
        assert!(after.units.contains_key(&format!("{:064x}", 1)));
    }

    #[test]
    fn overflow_profile_purge_and_restart_reconcile_from_authoritative_rows() {
        let (dir, engine) = fixture(2);
        cache(&engine, 0);
        let original = walk(&engine);
        let keys: Vec<_> = (0..=MAX_DIRTY).map(|i| format!("{i:064x}")).collect();
        engine
            .semantic_commit(
                engine.db.begin_write().unwrap(),
                std::iter::empty(),
                keys.iter().map(String::as_str),
                false,
            )
            .unwrap();
        let pages = fault::reached(MAPPING_FULL_PAGE);
        assert_eq!(walk(&engine).units, original.units);
        assert!(fault::reached(MAPPING_FULL_PAGE) > pages);
        let mut other = MappingWalk::start(&engine, &"cd".repeat(32), "recipe").unwrap();
        other.step(&engine, &Control::unbounded(), None).unwrap();
        assert!(other.finish(&engine).unwrap().units.is_empty());
        assert_eq!(walk(&engine).units, original.units);
        drop(engine);
        let engine = Engine::open_existing(&dir.path().join("store")).unwrap();
        let pages = fault::reached(MAPPING_FULL_PAGE);
        assert_eq!(walk(&engine).units, original.units);
        assert!(fault::reached(MAPPING_FULL_PAGE) > pages);
        let pages = fault::reached(MAPPING_FULL_PAGE);
        walk(&engine);
        assert_eq!(fault::reached(MAPPING_FULL_PAGE), pages);
        engine.semantic_purge().unwrap();
        assert!(walk(&engine).units.is_empty());
        assert!(fault::reached(MAPPING_FULL_PAGE) > pages);
    }
}
