//! Durable, independently acknowledged source obligations. Source transactions
//! coalesce current versions; bounded overflow requests a full reconciliation.
//! Lexical pending keys and model/provider activity are deliberately separate.
use crate::{FResult, FoundryError, store::Engine};
use redb::{
    ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition, WriteTransaction,
};
use serde::{Deserialize, Serialize};
use std::sync::{Condvar, Mutex};
#[cfg(feature = "semantic")]
use std::time::Duration;

const SEMANTIC: TableDefinition<&str, &str> = TableDefinition::new("semantic_work");
const KNOWLEDGE: TableDefinition<&str, &str> = TableDefinition::new("knowledge_work");
const META: &str = "";
pub const MAX_PATHS: u64 = 1024;
pub const MAX_PATH_BYTES: u64 = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    Semantic,
    Knowledge,
}
impl Lane {
    fn table(self) -> TableDefinition<'static, &'static str, &'static str> {
        match self {
            Self::Semantic => SEMANTIC,
            Self::Knowledge => KNOWLEDGE,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Meta {
    version: u32,
    epoch: u64,
    reconcile: bool,
    identity: Option<String>,
    path_bytes: u64,
    #[serde(default)]
    after: Option<String>,
}
impl Default for Meta {
    fn default() -> Self {
        Self {
            version: 1,
            epoch: 0,
            reconcile: true,
            identity: None,
            path_bytes: 0,
            after: None,
        }
    }
}
fn decode<T: serde::de::DeserializeOwned>(raw: &str) -> FResult<T> {
    serde_json::from_str(raw)
        .map_err(|e| FoundryError::CorruptStore(format!("derivation work: {e}")))
}
fn meta(table: &impl ReadableTable<&'static str, &'static str>) -> FResult<Meta> {
    let raw = table
        .get(META)?
        .ok_or_else(|| FoundryError::CorruptStore("derivation work metadata missing".into()))?;
    let meta: Meta = decode(raw.value())?;
    if meta.version != 1 || meta.path_bytes > MAX_PATH_BYTES || table.len()? > MAX_PATHS + 1 {
        return Err(FoundryError::CorruptStore(
            "invalid derivation work metadata".into(),
        ));
    }
    Ok(meta)
}
fn next_epoch(meta: &mut Meta) -> FResult<()> {
    meta.epoch = meta
        .epoch
        .checked_add(1)
        .ok_or(FoundryError::RevisionExhausted)?;
    meta.reconcile = true;
    meta.after = None;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub revision: u64,
    pub hash: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Batch {
    owner: std::sync::Arc<Wake>,
    lane: Lane,
    identity: String,
    epoch: u64,
    reconcile: bool,
    revision: u64,
    paths: Vec<(String, Version)>,
}
impl Batch {
    pub fn requires_reconcile(&self) -> bool {
        self.reconcile
    }
    pub fn paths(&self) -> &[(String, Version)] {
        &self.paths
    }
}

#[derive(Debug, Serialize)]
pub struct Pending {
    pub paths: u64,
    pub path_bytes: u64,
    pub reconcile_required: bool,
    pub epoch: u64,
}

pub(crate) fn init(tx: &WriteTransaction) -> FResult<()> {
    for lane in [Lane::Semantic, Lane::Knowledge] {
        let mut table = tx.open_table(lane.table())?;
        if table.get(META)?.is_none() {
            table.insert(META, serde_json::to_string(&Meta::default())?.as_str())?;
        }
    }
    Ok(())
}
pub(crate) fn invalidate(tx: &WriteTransaction, lane: Lane) -> FResult<()> {
    let mut table = tx.open_table(lane.table())?;
    let mut state = meta(&table)?;
    next_epoch(&mut state)?;
    table.insert(META, serde_json::to_string(&state)?.as_str())?;
    Ok(())
}
pub(crate) fn check(db: &redb::Database) -> FResult<()> {
    let tx = db.begin_read()?;
    for lane in [Lane::Semantic, Lane::Knowledge] {
        let table = tx
            .open_table(lane.table())
            .map_err(|e| FoundryError::CorruptStore(format!("derivation work table: {e}")))?;
        meta(&table)?;
    }
    Ok(())
}
/// Called inside the source publication/deletion transaction, including sweeps.
pub(crate) fn changed(
    tx: &WriteTransaction,
    path: &str,
    hash: Option<&str>,
    revision: u64,
) -> FResult<()> {
    let version = serde_json::to_string(&Version {
        revision,
        hash: hash.map(str::to_owned),
    })?;
    for lane in [Lane::Semantic, Lane::Knowledge] {
        let mut table = tx.open_table(lane.table())?;
        let mut state = meta(&table)?;
        let new = table.get(path)?.is_none();
        if new
            && (table.len()? > MAX_PATHS || state.path_bytes + path.len() as u64 > MAX_PATH_BYTES)
        {
            // The table is bounded. Keep tracking changes after overflow so edits
            // during the ensuing full walk cannot disappear behind its cursor.
            let keys = table
                .iter()?
                .map(|row| row.map(|(key, _)| key.value().to_owned()))
                .collect::<Result<Vec<_>, _>>()?;
            for key in keys {
                if key != META {
                    table.remove(key.as_str())?;
                }
            }
            state.path_bytes = 0;
            next_epoch(&mut state)?;
        }
        if path.len() as u64 <= MAX_PATH_BYTES {
            if table.get(path)?.is_none() {
                state.path_bytes += path.len() as u64;
            }
            table.insert(path, version.as_str())?;
        } else {
            next_epoch(&mut state)?;
        }
        table.insert(META, serde_json::to_string(&state)?.as_str())?;
    }
    Ok(())
}

impl Engine {
    pub fn derivation_pending(&self, lane: Lane) -> FResult<Pending> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(lane.table())?;
        let state = meta(&table)?;
        Ok(Pending {
            paths: table.len()? - 1,
            path_bytes: state.path_bytes,
            reconcile_required: state.reconcile,
            epoch: state.epoch,
        })
    }
    /// An explicit one-shot preparation audits all sources. Persist that
    /// obligation before it can alter publication, so interruption is resumable.
    pub fn derivation_reconcile(&self, lane: Lane, identity: &str) -> FResult<Batch> {
        if identity.is_empty() || identity.len() > 256 {
            return Err(FoundryError::InvalidArgument(
                "derivation identity must be 1..256 bytes".into(),
            ));
        }
        let tx = self.db.begin_write()?;
        invalidate(&tx, lane)?;
        tx.commit()?;
        if lane == Lane::Semantic {
            self.semantic_wake.notify();
        }
        self.derivation_batch(lane, identity)
    }

    /// Claim nothing: a private snapshot remains pending until exact acknowledgment.
    /// A profile/recipe change requires a complete walk. Initial/overflow snapshots
    /// capture a revision fence instead of retaining corpus-sized path collections.
    pub fn derivation_batch(&self, lane: Lane, identity: &str) -> FResult<Batch> {
        if identity.is_empty() || identity.len() > 256 {
            return Err(FoundryError::InvalidArgument(
                "derivation identity must be 1..256 bytes".into(),
            ));
        }
        let tx = self.db.begin_write()?;
        let batch = {
            let mut table = tx.open_table(lane.table())?;
            let mut state = meta(&table)?;
            if state.identity.as_deref() != Some(identity) {
                state.identity = Some(identity.to_owned());
                next_epoch(&mut state)?;
                table.insert(META, serde_json::to_string(&state)?.as_str())?;
            }
            let paths = if state.reconcile {
                Vec::new()
            } else {
                use std::ops::Bound::{Excluded, Included, Unbounded};
                let after = state.after.as_deref().unwrap_or(META);
                let mut paths = table
                    .range::<&str>((Excluded(after), Unbounded))?
                    .take(8)
                    .map(|row| {
                        let (path, raw) = row?;
                        Ok((path.value().to_owned(), decode(raw.value())?))
                    })
                    .collect::<FResult<Vec<_>>>()?;
                if paths.len() < 8 && after != META {
                    for row in table
                        .range::<&str>((Excluded(META), Included(after)))?
                        .take(8 - paths.len())
                    {
                        let (path, raw) = row?;
                        paths.push((path.value().to_owned(), decode(raw.value())?));
                    }
                }
                state.after = paths.last().map(|(path, _)| path.clone());
                table.insert(META, serde_json::to_string(&state)?.as_str())?;
                paths
            };
            Batch {
                owner: std::sync::Arc::clone(&self.semantic_wake),
                lane,
                identity: identity.to_owned(),
                epoch: state.epoch,
                reconcile: state.reconcile,
                revision: crate::store::revision_in(&tx)?,
                paths,
            }
        };
        tx.commit()?;
        Ok(batch)
    }
    /// Only after successful derivation AND publication. Later edits, overflow,
    /// or another recipe keep their obligations even when old work finishes late.
    pub fn derivation_ack(&self, lane: Lane, batch: &Batch) -> FResult<()> {
        if lane != batch.lane || !std::sync::Arc::ptr_eq(&batch.owner, &self.semantic_wake) {
            return Err(FoundryError::InvalidArgument(
                "derivation acknowledgment belongs to another owner or lane".into(),
            ));
        }
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(lane.table())?;
            let mut state = meta(&table)?;
            if state.identity.as_deref() != Some(&batch.identity) || state.epoch != batch.epoch {
                return Ok(());
            }
            let remove = if batch.reconcile {
                let mut keys = Vec::new();
                for row in table
                    .range::<&str>((std::ops::Bound::Excluded(META), std::ops::Bound::Unbounded))?
                {
                    let (path, raw) = row?;
                    if decode::<Version>(raw.value())?.revision <= batch.revision {
                        keys.push(path.value().to_owned());
                    }
                }
                state.reconcile = false;
                keys
            } else {
                let mut keys = Vec::new();
                for (path, version) in &batch.paths {
                    if let Some(raw) = table.get(path.as_str())?
                        && decode::<Version>(raw.value())? == *version
                    {
                        keys.push(path.clone());
                    }
                }
                keys
            };
            for path in remove {
                table.remove(path.as_str())?;
                state.path_bytes =
                    state
                        .path_bytes
                        .checked_sub(path.len() as u64)
                        .ok_or_else(|| {
                            FoundryError::CorruptStore(
                                "derivation path accounting underflow".into(),
                            )
                        })?;
            }
            table.insert(META, serde_json::to_string(&state)?.as_str())?;
        }
        tx.commit()?;
        Ok(())
    }
}

/// In-process notification only; redb remains the durable authority. No idle
/// source/cache polling. The timeout bounds pause/shutdown latency, not retries.
#[derive(Debug, Default)]
pub(crate) struct Wake {
    epoch: Mutex<u64>,
    changed: Condvar,
}
impl Wake {
    #[cfg(feature = "semantic")]
    pub(crate) fn stamp(&self) -> u64 {
        *self
            .epoch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub(crate) fn notify(&self) {
        let mut epoch = self
            .epoch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *epoch = epoch.wrapping_add(1);
        self.changed.notify_all();
    }
    #[cfg(feature = "semantic")]
    pub(crate) fn wait(&self, stamp: u64) -> bool {
        let guard = self
            .epoch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (epoch, _) = self
            .changed
            .wait_timeout_while(guard, Duration::from_millis(250), |epoch| *epoch == stamp)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *epoch != stamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Control;
    fn fixture() -> (tempfile::TempDir, Engine) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
        (dir, engine)
    }
    fn reconcile(engine: &Engine, lane: Lane, identity: &str) {
        let batch = engine.derivation_batch(lane, identity).unwrap();
        engine.derivation_ack(lane, &batch).unwrap();
    }
    #[test]
    fn coalescing_acknowledges_only_its_version_and_lane_across_restart() {
        let (dir, mut engine) = fixture();
        for lane in [Lane::Semantic, Lane::Knowledge] {
            reconcile(&engine, lane, "recipe-a");
        }
        engine.replace_source("one.rs", "fn one() {}\n").unwrap();
        let old = engine.derivation_batch(Lane::Semantic, "recipe-a").unwrap();
        assert_eq!(old.paths.len(), 1);
        assert!(engine.derivation_ack(Lane::Knowledge, &old).is_err());
        let (_other_dir, other) = fixture();
        reconcile(&other, Lane::Semantic, "recipe-a");
        other.replace_source("one.rs", "fn one() {}\n").unwrap();
        assert!(other.derivation_ack(Lane::Semantic, &old).is_err());
        engine.replace_source("one.rs", "fn two() {}\n").unwrap();
        // Even a revert has a new version and must survive an old acknowledgment.
        engine.replace_source("one.rs", "fn one() {}\n").unwrap();
        engine.derivation_ack(Lane::Semantic, &old).unwrap();
        assert_eq!(engine.derivation_pending(Lane::Semantic).unwrap().paths, 1);
        engine.refresh_index(&Control::unbounded()).unwrap();
        assert_eq!(engine.derivation_pending(Lane::Semantic).unwrap().paths, 1);
        let work = engine.derivation_batch(Lane::Semantic, "recipe-a").unwrap();
        engine.derivation_ack(Lane::Semantic, &work).unwrap();
        assert_eq!(engine.derivation_pending(Lane::Semantic).unwrap().paths, 0);
        assert_eq!(engine.derivation_pending(Lane::Knowledge).unwrap().paths, 1);
        engine.delete_source("one.rs").unwrap();
        drop(engine);
        let engine = Engine::open_existing(&dir.path().join("store")).unwrap();
        let deleted = engine.derivation_batch(Lane::Semantic, "recipe-a").unwrap();
        assert_eq!(deleted.paths.len(), 1);
        assert_eq!(deleted.paths[0].1.hash, None);
    }
    #[test]
    fn overflow_and_recipe_changes_fence_late_reconciliation() {
        let (_dir, engine) = fixture();
        let old = engine.derivation_batch(Lane::Semantic, "first").unwrap();
        // Use the same ordinary transaction hook, without allocating source bodies.
        let tx = engine.db.begin_write().unwrap();
        for revision in 1..=MAX_PATHS + 1 {
            changed(&tx, &format!("p{revision}"), None, revision).unwrap();
        }
        tx.commit().unwrap();
        let pending = engine.derivation_pending(Lane::Semantic).unwrap();
        assert!(pending.reconcile_required);
        assert_eq!(pending.paths, 1);
        assert!(pending.path_bytes <= MAX_PATH_BYTES);
        engine.derivation_ack(Lane::Semantic, &old).unwrap();
        assert!(
            engine
                .derivation_pending(Lane::Semantic)
                .unwrap()
                .reconcile_required
        );
        let old = engine.derivation_batch(Lane::Semantic, "first").unwrap();
        let new = engine.derivation_batch(Lane::Semantic, "second").unwrap();
        engine.derivation_ack(Lane::Semantic, &old).unwrap();
        assert!(
            engine
                .derivation_pending(Lane::Semantic)
                .unwrap()
                .reconcile_required
        );
        engine.derivation_ack(Lane::Semantic, &new).unwrap();
        // The synthetic version exceeds the snapshot's source revision fence.
        assert_eq!(engine.derivation_pending(Lane::Semantic).unwrap().paths, 1);
    }
    #[test]
    fn full_walk_keeps_edits_arriving_behind_its_cursor_and_purge_requests_reconcile() {
        let (_dir, engine) = fixture();
        engine.replace_source("z.rs", "fn z() {}\n").unwrap();
        let full = engine.derivation_batch(Lane::Semantic, "r").unwrap();
        engine.replace_source("a.rs", "fn a() {}\n").unwrap();
        engine.derivation_ack(Lane::Semantic, &full).unwrap();
        let next = engine.derivation_batch(Lane::Semantic, "r").unwrap();
        assert!(!next.reconcile);
        assert_eq!(next.paths.len(), 1);
        assert_eq!(next.paths[0].0, "a.rs");
        engine.derivation_ack(Lane::Semantic, &next).unwrap();
        engine.semantic_purge().unwrap();
        assert!(
            engine
                .derivation_pending(Lane::Semantic)
                .unwrap()
                .reconcile_required
        );
    }
    #[test]
    fn hot_paths_cannot_starve_the_rest_of_the_durable_frontier() {
        let (_dir, engine) = fixture();
        reconcile(&engine, Lane::Semantic, "r");
        for n in 0..32 {
            engine
                .replace_source(&format!("p{n:02}.rs"), "fn original() {}\n")
                .unwrap();
        }
        let mut visited = std::collections::BTreeSet::new();
        for round in 0..4 {
            let batch = engine.derivation_batch(Lane::Semantic, "r").unwrap();
            for (path, _) in &batch.paths {
                visited.insert(path.clone());
                // Late completion cannot clear these hot versions, but the
                // persisted selection cursor must still visit later paths.
                engine
                    .replace_source(path, &format!("fn edit_{round}() {{}}\n"))
                    .unwrap();
            }
            engine.derivation_ack(Lane::Semantic, &batch).unwrap();
        }
        assert_eq!(visited.len(), 32);
        assert_eq!(engine.derivation_pending(Lane::Semantic).unwrap().paths, 32);
    }

    #[test]
    fn schema_eight_upgrade_preserves_existing_rows_and_requires_reconciliation() {
        use redb::ReadableDatabase;
        let (dir, engine) = fixture();
        engine.replace_source("a.rs", "fn a() {}\n").unwrap();
        engine
            .semantic_cache_commit(
                &[("b".repeat(64), vec![1.0; 128])],
                &"a".repeat(64),
                128,
                1 << 20,
            )
            .unwrap();
        let cache_totals = engine.semantic_cache_totals().unwrap();
        let revision = engine.source_revision().unwrap();
        drop(engine);
        let store = dir.path().join("store");
        {
            let db = redb::Database::open(store.join("knowledge.redb")).unwrap();
            let tx = db.begin_write().unwrap();
            tx.delete_table(SEMANTIC).unwrap();
            tx.delete_table(KNOWLEDGE).unwrap();
            tx.open_table(crate::store::META)
                .unwrap()
                .insert("schema", "8")
                .unwrap();
            tx.commit().unwrap();
            assert!(db.begin_read().unwrap().open_table(SEMANTIC).is_err());
        }
        assert!(matches!(
            Engine::open_existing(&store),
            Err(FoundryError::UpgradeRequired { .. })
        ));
        Engine::upgrade_store(&store, crate::store::SCHEMA_VERSION, &Control::unbounded()).unwrap();
        let engine = Engine::open_existing(&store).unwrap();
        assert_eq!(engine.source_revision().unwrap(), revision);
        assert_eq!(engine.semantic_cache_totals().unwrap(), cache_totals);
        assert!(engine.source("a.rs").unwrap().is_some());
        assert!(
            engine
                .derivation_pending(Lane::Semantic)
                .unwrap()
                .reconcile_required
        );
        assert!(
            engine
                .derivation_pending(Lane::Knowledge)
                .unwrap()
                .reconcile_required
        );
    }
}
