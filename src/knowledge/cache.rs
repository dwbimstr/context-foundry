//! Bounded memoized completions in the existing source database. Cache and brief
//! publication share one transaction; this is not another source/job ledger.
use super::*;
use redb::{ReadableTableMetadata, TableDefinition, WriteTransaction};

pub(crate) const CACHE: TableDefinition<&str, &str> = TableDefinition::new("knowledge_cache");
const MAX_CACHE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ENTRIES: u64 = 4096;
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Meta {
    pub bytes: u64,
    pub entries: u64,
    pub backoff: Option<Backoff>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Backoff {
    pub provider: String,
    pub until_unix_ms: u64,
    pub code: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    record: brief::Record,
    usage: Usage,
}
pub(crate) fn init(tx: &WriteTransaction) -> FResult<()> {
    let mut table = tx.open_table(CACHE)?;
    if table.get("")?.is_none() {
        table.insert("", serde_json::to_string(&Meta::default())?.as_str())?;
    }
    Ok(())
}
fn meta(table: &impl ReadableTable<&'static str, &'static str>) -> FResult<Meta> {
    let raw = table
        .get("")?
        .ok_or_else(|| corrupt("missing cache accounting"))?;
    let state: Meta = serde_json::from_str(raw.value()).map_err(|_| corrupt("cache accounting"))?;
    if state.bytes > MAX_CACHE_BYTES || state.entries > MAX_ENTRIES {
        return Err(corrupt("cache bounds"));
    }
    Ok(state)
}
pub(crate) fn check(db: &redb::Database) -> FResult<()> {
    let tx = db.begin_read()?;
    let table = tx
        .open_table(CACHE)
        .map_err(|_| corrupt("missing knowledge cache"))?;
    let state = meta(&table)?;
    if table.len()? != state.entries + 1 {
        return Err(corrupt("cache entry accounting"));
    }
    Ok(())
}
fn decode(key: &str, raw: &str) -> FResult<Entry> {
    if raw.len() > brief::MAX_RECORD_BYTES + 4096 {
        return Err(corrupt("cache row too large"));
    }
    let entry: Entry = serde_json::from_str(raw).map_err(|_| corrupt("cache row"))?;
    let record = brief::decode_row(&entry.record.id, &serde_json::to_string(&entry.record)?)?;
    if record.derivation.is_none() || key != record.brief.prompt_sha256 {
        return Err(corrupt("cache identity"));
    }
    Ok(entry)
}
pub(crate) fn get(engine: &Engine, key: &str) -> FResult<Option<brief::Record>> {
    let tx = engine.db.begin_read()?;
    tx.open_table(CACHE)?
        .get(key)?
        .map(|raw| decode(key, raw.value()).map(|v| v.record))
        .transpose()
}
pub(crate) fn put(
    tx: &WriteTransaction,
    record: &brief::Record,
    usage: &Usage,
    cap: u64,
) -> FResult<()> {
    let key = &record.brief.prompt_sha256;
    let encoded = serde_json::to_string(&Entry {
        record: record.clone(),
        usage: usage.clone(),
    })?;
    if encoded.len() > brief::MAX_RECORD_BYTES + 4096
        || encoded.len() as u64 > cap
        || cap > MAX_CACHE_BYTES
    {
        return Err(fail(
            "knowledge_cache_full",
            "completion exceeds the cache bound",
        ));
    }
    let mut table = tx.open_table(CACHE)?;
    let mut state = meta(&table)?;
    if let Some(raw) = table.get(key.as_str())? {
        let old = decode(key, raw.value())?;
        if old.record.input_sha256 != record.input_sha256 || old.record.brief != record.brief {
            return Err(corrupt("conflicting completion for identical request"));
        }
        return Ok(());
    }
    reserve(&mut table, &mut state, cap, encoded.len() as u64)?;
    table.insert(key.as_str(), encoded.as_str())?;
    state.entries += 1;
    state.bytes += encoded.len() as u64;
    table.insert("", serde_json::to_string(&state)?.as_str())?;
    Ok(())
}
fn reserve(
    table: &mut redb::Table<&str, &str>,
    state: &mut Meta,
    cap: u64,
    extra: u64,
) -> FResult<()> {
    // Deterministic eviction keeps bounded storage without a second LRU index.
    // The current accepted brief still enables a no-call replay after eviction.
    while state.bytes + extra > cap || state.entries + u64::from(extra > 0) > MAX_ENTRIES {
        let (old, len) = table
            .range("!"..)?
            .next()
            .transpose()?
            .map(|(k, v)| (k.value().to_owned(), v.value().len() as u64))
            .ok_or_else(|| corrupt("cache accounting has no evictable row"))?;
        table.remove(old.as_str())?;
        state.bytes = state
            .bytes
            .checked_sub(len)
            .ok_or_else(|| corrupt("cache byte accounting"))?;
        state.entries -= 1;
    }
    Ok(())
}
pub(crate) fn trim(engine: &Engine, cap: u64) -> FResult<()> {
    let tx = engine.db.begin_write()?;
    {
        let mut table = tx.open_table(CACHE)?;
        let mut state = meta(&table)?;
        reserve(&mut table, &mut state, cap, 0)?;
        table.insert("", serde_json::to_string(&state)?.as_str())?;
    }
    tx.commit()?;
    Ok(())
}
pub(crate) fn forget(tx: &WriteTransaction, ids: &BTreeSet<String>) -> FResult<()> {
    let mut table = tx.open_table(CACHE)?;
    let mut state = meta(&table)?;
    let mut remove = Vec::new();
    for row in table.range("!"..)? {
        let (key, raw) = row?;
        let entry = decode(key.value(), raw.value())?;
        if ids.contains(&entry.record.id)
            || entry
                .record
                .derivation
                .as_ref()
                .is_some_and(|d| d.children.keys().any(|id| ids.contains(id)))
        {
            remove.push((key.value().to_owned(), raw.value().len() as u64));
        }
    }
    for (key, bytes) in remove {
        table.remove(key.as_str())?;
        state.entries = state
            .entries
            .checked_sub(1)
            .ok_or_else(|| corrupt("cache entry accounting"))?;
        state.bytes = state
            .bytes
            .checked_sub(bytes)
            .ok_or_else(|| corrupt("cache byte accounting"))?;
    }
    table.insert("", serde_json::to_string(&state)?.as_str())?;
    Ok(())
}
pub(crate) fn backoff(engine: &Engine, identity: &str) -> FResult<bool> {
    let tx = engine.db.begin_read()?;
    Ok(meta(&tx.open_table(CACHE)?)?
        .backoff
        .is_some_and(|b| b.provider == identity && b.until_unix_ms > now_ms()))
}
pub(crate) fn record_failure(engine: &Engine, identity: &str, code: Option<&str>) -> FResult<()> {
    let tx = engine.db.begin_write()?;
    {
        let mut table = tx.open_table(CACHE)?;
        let mut state = meta(&table)?;
        state.backoff = code.map(|code| Backoff {
            provider: identity.into(),
            until_unix_ms: now_ms().saturating_add(30_000),
            code: code.into(),
        });
        table.insert("", serde_json::to_string(&state)?.as_str())?;
    }
    tx.commit()?;
    Ok(())
}
impl Engine {
    pub fn knowledge_status(&self) -> FResult<Value> {
        let tx = self.db.begin_read()?;
        let state = meta(&tx.open_table(CACHE)?)?;
        Ok(json!({"cache": state, "pending": self.derivation_pending(Lane::Knowledge)?}))
    }
    /// Purges memoized outputs only. Current briefs have their own explicit forget
    /// operation, which also forgets their memoized copies and derived ancestors.
    pub fn knowledge_purge_cache(&self) -> FResult<Value> {
        let tx = self.db.begin_write()?;
        let state = meta(&tx.open_table(CACHE)?)?;
        tx.delete_table(CACHE)?;
        init(&tx)?;
        tx.commit()?;
        Ok(json!({"removed_entries":state.entries,"removed_bytes":state.bytes}))
    }
}
