//! Disposable publication lookup and bounded transactional invalidation frontier.
//! These redb rows accelerate restart; source/partition/cache rows remain authority.
use crate::{FResult, FoundryError};
use redb::{
    MultimapTableDefinition, ReadableTable, ReadableTableMetadata, TableDefinition,
    WriteTransaction,
};

pub(crate) const ROWS: TableDefinition<&str, &str> = TableDefinition::new("semantic_mapping_rows");
pub(crate) const KEYS: MultimapTableDefinition<&str, &str> =
    MultimapTableDefinition::new("semantic_mapping_keys");
const WORK: TableDefinition<&str, u64> = TableDefinition::new("semantic_mapping_work");
pub(crate) const CHECKPOINT: &str = "semantic_mapping_checkpoint";
const FLOOR: &str = "semantic_mapping_floor";
const BYTES: &str = "semantic_mapping_dirty_bytes";
pub(crate) const MAX_DIRTY: usize = 1024;
pub(crate) const MAX_BYTES: usize = 1024 * 1024;

pub(crate) fn init(tx: &WriteTransaction) -> FResult<()> {
    tx.open_table(ROWS)?;
    tx.open_multimap_table(KEYS)?;
    tx.open_table(WORK)?;
    Ok(())
}
pub(crate) fn purge(tx: &WriteTransaction) -> FResult<()> {
    tx.delete_table(ROWS)?;
    tx.delete_multimap_table(KEYS)?;
    tx.delete_table(WORK)?;
    init(tx)?;
    let mut meta = tx.open_table(crate::store::META)?;
    meta.remove(CHECKPOINT)?;
    meta.remove(BYTES)?;
    Ok(())
}
pub(crate) fn check(tx: &redb::ReadTransaction) -> FResult<()> {
    tx.open_table(ROWS)?;
    tx.open_multimap_table(KEYS)?;
    tx.open_table(WORK)?;
    Ok(())
}
fn counter(table: &impl ReadableTable<&'static str, &'static str>, key: &str) -> FResult<u64> {
    table
        .get(key)?
        .map(|v| {
            v.value()
                .parse()
                .map_err(|_| FoundryError::CorruptStore(format!("invalid {key}")))
        })
        .transpose()
        .map(|v| v.unwrap_or(0))
}
/// The revision advances in this same transaction, including lexical-only builds.
pub(crate) fn changed(
    tx: &WriteTransaction,
    revision: u64,
    paths: &[String],
    keys: &[String],
    reset: bool,
) -> FResult<()> {
    let mut meta = tx.open_table(crate::store::META)?;
    let mut work = tx.open_table(WORK)?;
    let mut bytes = counter(&meta, BYTES)?;
    if reset {
        work.retain(|_, _| false)?;
        bytes = 0;
        meta.insert(FLOOR, revision.to_string().as_str())?;
        meta.remove(CHECKPOINT)?;
    } else if meta.get(CHECKPOINT)?.is_none() {
        // Cold ingestion has no reusable lookup to invalidate. Recording each
        // path would only build and overflow a queue that cannot be replayed.
        // A later stable checkpoint captures this revision from authority.
        meta.insert(FLOOR, revision.to_string().as_str())?;
    } else {
        for (kind, value) in paths
            .iter()
            .map(|v| ("p:", v))
            .chain(keys.iter().map(|v| ("k:", v)))
        {
            let key = format!("{kind}{value}");
            if work.get(key.as_str())?.is_none() {
                bytes += key.len() as u64;
            }
            if work.len()? >= MAX_DIRTY as u64 && work.get(key.as_str())?.is_none()
                || bytes > MAX_BYTES as u64
            {
                work.retain(|_, _| false)?;
                bytes = 0;
                meta.insert(FLOOR, revision.to_string().as_str())?;
                meta.remove(CHECKPOINT)?;
                break;
            }
            work.insert(key.as_str(), revision)?;
        }
    }
    meta.insert(BYTES, bytes.to_string().as_str())?;
    Ok(())
}

#[cfg(feature = "semantic")]
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct Checkpoint {
    pub revision: u64,
    pub coverage: super::cache::CoverageSnapshot,
    pub bytes: u64,
}
#[cfg(feature = "semantic")]
pub(crate) fn pending(
    tx: &redb::ReadTransaction,
    since: u64,
) -> FResult<Option<(Vec<String>, Vec<String>)>> {
    let meta = tx.open_table(crate::store::META)?;
    if counter(&meta, FLOOR)? > since {
        return Ok(None);
    }
    let work = tx.open_table(WORK)?;
    if work.len()? > MAX_DIRTY as u64 || counter(&meta, BYTES)? > MAX_BYTES as u64 {
        return Err(FoundryError::CorruptStore(
            "publication frontier exceeds its bound".into(),
        ));
    }
    let mut paths = Vec::new();
    let mut keys = Vec::new();
    for row in work.iter()? {
        let (key, revision) = row?;
        if revision.value() <= since {
            continue;
        }
        let key = key.value();
        if let Some(path) = key.strip_prefix("p:") {
            paths.push(path.to_owned());
        } else if let Some(key) = key.strip_prefix("k:") {
            keys.push(key.to_owned());
        } else {
            return Err(FoundryError::CorruptStore(
                "invalid publication frontier key".into(),
            ));
        }
    }
    Ok(Some((paths, keys)))
}
#[cfg(feature = "semantic")]
pub(crate) fn acknowledge(tx: &WriteTransaction, revision: u64) -> FResult<()> {
    let mut work = tx.open_table(WORK)?;
    work.retain(|_, r| r > revision)?;
    let bytes = work.iter()?.try_fold(0u64, |bytes, row| -> FResult<_> {
        Ok(bytes + row?.0.value().len() as u64)
    })?;
    tx.open_table(crate::store::META)?
        .insert(BYTES, bytes.to_string().as_str())?;
    Ok(())
}
