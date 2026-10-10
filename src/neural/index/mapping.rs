//! A disposable, owner-local view of current partition locations. redb remains
//! authoritative; restart can resume a matching redb lookup. Overflow, profile
//! change and explicit repair reconcile.
use super::*;
use crate::neural::mapping_store::{self as durable, Checkpoint};
use redb::{ReadableDatabase, ReadableTable, ReadableTableMetadata};
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
    baseline: Option<redb::ReadTransaction>,
    durable_revision: Option<u64>,
    partitioned: u64,
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
    fn remove(&mut self, path: &str) -> FResult<()> {
        if let Some(tx) = &self.baseline {
            let table = tx.open_table(durable::ROWS)?;
            if let Some(raw) = table.get(path)? {
                self.rows
                    .insert(path.to_owned(), serde_json::from_str(raw.value())?);
            }
        }
        let Some(record) = self.rows.remove(path) else {
            return Ok(());
        };
        let corrupt = || FoundryError::CorruptStore("invalid publication lookup counters".into());
        self.partitioned = self.partitioned.checked_sub(1).ok_or_else(corrupt)?;
        self.eligible = self
            .eligible
            .checked_sub(record.units.len() as u64)
            .ok_or_else(corrupt)?;
        self.bytes = self
            .bytes
            .checked_sub(row_bytes(path, &record))
            .ok_or_else(corrupt)?;
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
                self.cached = self
                    .cached
                    .checked_sub((before - units.len()) as u64)
                    .ok_or_else(corrupt)?;
                if units.is_empty() {
                    self.units.remove(&unit.input_key);
                }
            }
        }
        Ok(())
    }
    fn refresh(
        &mut self,
        page: &cache::MappingPage,
        path: &str,
        probed: &mut HashMap<String, bool>,
        control: &Control,
    ) -> FResult<()> {
        control.check()?;
        self.remove(path)?;
        let Some(record) = page.partition(path, &self.digest, &self.recipe, control)? else {
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
                    let cached = page.cached(&unit.input_key, &self.digest)?;
                    if probed.len() >= MAX_DIRTY {
                        probed.clear();
                    }
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
        self.partitioned += 1;
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
    touched: BTreeSet<String>,
}
impl MappingWalk {
    #[cfg(test)]
    pub(crate) fn start(engine: &Engine, digest: &str, recipe: &str) -> FResult<Self> {
        Self::begin(engine, digest, recipe, false, None)
    }
    pub(crate) fn audit(engine: &Engine, digest: &str, recipe: &str) -> FResult<Self> {
        Self::begin(engine, digest, recipe, true, None)
    }
    pub(crate) fn resume(
        engine: &Engine,
        digest: &str,
        recipe: &str,
        generation: Option<&Generation>,
    ) -> FResult<Self> {
        Self::begin(engine, digest, recipe, false, generation)
    }
    fn begin(
        engine: &Engine,
        digest: &str,
        recipe: &str,
        audit: bool,
        generation: Option<&Generation>,
    ) -> FResult<Self> {
        let mut state = engine
            .semantic_mapping
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut view = if audit {
            None
        } else {
            state
                .view
                .take()
                .filter(|v| v.digest == digest && v.recipe == recipe)
        };
        let mut dirty: BTreeSet<_> = state.paths.keys().cloned().collect();
        let mut keys: BTreeSet<_> = state.keys.keys().cloned().collect();
        if view.is_none()
            && !audit
            && let Some(generation) = generation
            && let Some(restored) = restore(engine, generation)?
        {
            view = Some(restored.view);
            dirty.extend(restored.paths);
            keys.extend(restored.keys);
        }
        if let Some(view) = &view {
            for key in &keys {
                if let Some(tx) = &view.baseline {
                    let reverse = tx.open_multimap_table(durable::KEYS)?;
                    for path in reverse.get(key.as_str())? {
                        dirty.insert(path?.value().to_owned());
                        if dirty.len() > MAX_DIRTY {
                            break;
                        }
                    }
                }
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
            touched: BTreeSet::new(),
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
            let reader = cache::MappingPage::new(engine, control)?;
            for path in &page {
                self.view
                    .refresh(&reader, path, &mut self.probed, control)?;
                self.touched.insert(path.clone());
            }
            self.after = page.last().cloned();
            if until.is_some_and(|until| Instant::now() >= until) {
                return Ok(false);
            }
        }
    }
    pub(crate) fn finish(mut self, engine: &Engine, control: &Control) -> FResult<CurrentMapping> {
        control.check()?;
        // Match the source writer's lock order: redb writer, then mapping mutex.
        let tx = engine.db.begin_write()?;
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
            unpartitioned_sources: self.sources.saturating_sub(self.view.partitioned),
            eligible_units: self.view.eligible,
            cached_current_units: self.view.cached,
            searchable_current_units: self.view.cached,
            index_entries: self.view.units.len() as u64,
        };
        let complete = stable
            && self.view.partitioned == self.sources
            && self.view.eligible == self.view.cached;
        if stable {
            checkpoint(
                &tx,
                &self.view,
                &self.touched,
                self.mapping_revision,
                &coverage,
                control,
            )?;
            neural_fault!(
                MAPPING_CHECKPOINT_BEFORE_COMMIT,
                Some(control),
                "checkpoint"
            )?;
            control.check()?;
            tx.commit()?;
            self.view.durable_revision = Some(self.mapping_revision);
        }
        if !stable && self.view.baseline.is_none() {
            // The retained full view includes these touched rows, but they were
            // not checkpointed. A later stable turn must write the whole view;
            // its own dirty set alone would omit this turn's accepted changes.
            self.view.durable_revision = None;
        }
        let units = self.view.units.clone();
        if self.epoch == state.epoch
            && state
                .view
                .as_ref()
                .is_none_or(|view| view.ticket <= self.ticket)
        {
            self.view.ticket = self.ticket;
            // A resumed lookup retains only touched rows. Its redb checkpoint is
            // the next baseline, paired with the independently verified generation.
            state.view = self.view.baseline.is_none().then_some(self.view);
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

struct Restored {
    view: View,
    paths: Vec<String>,
    keys: Vec<String>,
}
/// Reuse only a checkpoint paired with this independently verified publication.
fn restore(engine: &Engine, generation: &Generation) -> FResult<Option<Restored>> {
    let tx = engine.db.begin_read()?;
    let checkpoint: Option<Checkpoint> = tx
        .open_table(crate::store::META)?
        .get(durable::CHECKPOINT)?
        .map(|v| serde_json::from_str(v.value()))
        .transpose()?;
    let Some(cp) = checkpoint else {
        return Ok(None);
    };
    let Some(partitioned) = cp
        .coverage
        .sources
        .checked_sub(cp.coverage.unpartitioned_sources)
    else {
        return Ok(None);
    };
    if generation.manifest.mapping_revision != Some(cp.revision)
        || generation.manifest.coverage_stats.as_ref() != Some(&cp.coverage)
        || cp.bytes > MAX_META_BYTES
        || tx.open_table(durable::ROWS)?.len()? != partitioned
    {
        return Ok(None);
    }
    let Some((paths, keys)) = durable::pending(&tx, cp.revision)? else {
        return Ok(None);
    };
    Ok(Some(Restored {
        view: View {
            digest: cp.coverage.function_digest,
            recipe: cp.coverage.recipe_id,
            durable_revision: Some(cp.revision),
            partitioned,
            eligible: cp.coverage.eligible_units,
            cached: cp.coverage.cached_current_units,
            bytes: cp.bytes,
            units: generation.labels.clone(),
            baseline: Some(tx),
            ..Default::default()
        },
        paths,
        keys,
    }))
}
fn checkpoint(
    tx: &redb::WriteTransaction,
    view: &View,
    touched: &BTreeSet<String>,
    revision: u64,
    coverage: &cache::CoverageSnapshot,
    control: &Control,
) -> FResult<()> {
    let previous: Option<Checkpoint> = tx
        .open_table(crate::store::META)?
        .get(durable::CHECKPOINT)?
        .map(|v| serde_json::from_str(v.value()))
        .transpose()?;
    let incremental = previous
        .as_ref()
        .is_some_and(|cp| Some(cp.revision) == view.durable_revision);
    if !incremental {
        // A resumed view is partial; a concurrently replaced checkpoint cannot
        // be reconstructed from it. Refuse instead of blessing incomplete rows.
        if view.baseline.is_some() {
            return Err(FoundryError::Semantic {
                code: "index_unavailable",
                message: "mapping checkpoint changed during reconciliation".into(),
            });
        }
        tx.delete_table(durable::ROWS)?;
        tx.delete_multimap_table(durable::KEYS)?;
    }
    let mut rows = tx.open_table(durable::ROWS)?;
    let mut reverse = tx.open_multimap_table(durable::KEYS)?;
    let paths: Box<dyn Iterator<Item = &String>> = if incremental {
        Box::new(touched.iter())
    } else {
        Box::new(view.rows.keys())
    };
    for path in paths {
        control.check()?;
        let old: Option<cache::PartitionRecord> = rows
            .remove(path.as_str())?
            .map(|v| serde_json::from_str(v.value()))
            .transpose()?;
        if let Some(old) = old {
            for unit in old.units {
                reverse.remove(unit.input_key.as_str(), path.as_str())?;
            }
        }
        if let Some(record) = view.rows.get(path) {
            rows.insert(path.as_str(), serde_json::to_string(record)?.as_str())?;
            for unit in &record.units {
                reverse.insert(unit.input_key.as_str(), path.as_str())?;
            }
        }
    }
    tx.open_table(crate::store::META)?.insert(
        durable::CHECKPOINT,
        serde_json::to_string(&Checkpoint {
            revision,
            coverage: coverage.clone(),
            bytes: view.bytes,
        })?
        .as_str(),
    )?;
    durable::acknowledge(tx, revision)?;
    Ok(())
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
        walk.finish(engine, &Control::unbounded()).unwrap()
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
    fn an_unstable_retained_view_cannot_omit_uncheckpointed_rows_on_retry() {
        let (_dir, engine) = fixture(3);
        walk(&engine);
        record(&engine, "f0001.md", 1001);
        let mut interrupted = MappingWalk::start(&engine, &digest(), "recipe").unwrap();
        interrupted
            .step(&engine, &Control::unbounded(), None)
            .unwrap();
        record(&engine, "f0002.md", 1002);
        assert!(
            interrupted
                .finish(&engine, &Control::unbounded())
                .unwrap()
                .mapping_revision
                .is_none()
        );
        assert!(walk(&engine).mapping_revision.is_some());
        let tx = engine.db.begin_read().unwrap();
        let rows = tx.open_table(durable::ROWS).unwrap();
        for (path, key) in [("f0001.md", 1001), ("f0002.md", 1002)] {
            let row: cache::PartitionRecord =
                serde_json::from_str(rows.get(path).unwrap().unwrap().value()).unwrap();
            assert_eq!(row.units[0].input_key, format!("{key:064x}"));
            let reverse = tx.open_multimap_table(durable::KEYS).unwrap();
            assert_eq!(
                reverse
                    .get(format!("{key:064x}").as_str())
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .value(),
                path
            );
        }
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
        assert!(
            !pending
                .finish(&engine, &Control::unbounded())
                .unwrap()
                .scope
                .complete
        );
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
        assert!(
            other
                .finish(&engine, &Control::unbounded())
                .unwrap()
                .units
                .is_empty()
        );
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
