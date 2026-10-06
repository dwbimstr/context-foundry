//! 009 semantic storage (store schema 5): the partition table, the f32
//! vector cache and the preparation-state row. Compiled in every build —
//! semantic rows survive a build without the `semantic` feature; only the
//! tokenizer and the ANN index are feature-gated elsewhere.
//!
//! Conventions follow the store: rows are strict JSON (or fixed-layout
//! bytes), a decode failure is corruption by name, cache commits precede any
//! derived publication, and the disk cap stops preparation with `cache_full`
//! instead of evicting. Every eligibility check compares the CURRENT source
//! hash, recipe, function digest AND the unit ranges against the source's
//! recorded length, so a source change (or a malformed mapping) makes the old
//! mapping ineligible immediately; source commits never touch these tables.
//! Accepting a mapping additionally binds every unit key to the exact
//! rendered bytes it names.
use crate::control::Control;
use crate::error::{FResult, FoundryError};
use crate::neural::anchor::{Dir, conflict_or_io};
use crate::neural::index::SEMANTIC_DIR;
use crate::neural::provider;
use crate::store::{CHUNKS, Engine, SOURCES, SourceMeta, decode, reconstruct_verified};
use redb::{
    Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition,
    WriteTransaction,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsString;
use std::ops::Bound;

pub(crate) const PARTITIONS: TableDefinition<&str, &str> =
    TableDefinition::new("semantic_partitions");
pub(crate) const CACHE: TableDefinition<&str, &[u8]> = TableDefinition::new("semantic_cache");
pub(crate) const STATE: TableDefinition<&str, &str> = TableDefinition::new("semantic_state");

/// The single state-row key.
pub const STATE_KEY: &str = "state";
/// Default cache cap: 2 GiB per workspace (D001 chosen values).
pub const DEFAULT_CACHE_CAP_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Fixed cache-row layout: 64 hex digest bytes + `DIMENSIONS` f32 LE values.
pub const CACHE_VALUE_BYTES: usize = 64 + provider::DIMENSIONS * 4;
/// The paged-walk bound shared with every other store census.
pub const PAGE: usize = 128;

/// One partitioned source version: the source hash and recipe it belongs to,
/// and the unit ranges with their document-input keys. An empty `units` list
/// is a COMPLETED zero-unit partition (legal only for an empty source),
/// distinguishable from a missing row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartitionRecord {
    pub source_hash: String,
    pub recipe_id: String,
    /// The function digest the unit input keys were computed under.
    pub function_digest: String,
    pub units: Vec<PartitionUnit>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartitionUnit {
    pub start: usize,
    pub end: usize,
    pub input_key: String,
}

/// The last provider state preparation observed. It is a record of what a
/// run saw, never a live probe: status reports it with its time and profile.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderObservation {
    /// `ready` (acquired), `refused` (acquisition refused: isolation, profile,
    /// busy) or `failed` (a batch failed: exited, malformed, resource limit).
    pub state: String,
    pub code: Option<String>,
    pub observed_at_unix: u64,
    pub profile: String,
    pub function_digest: String,
}

/// The preparation state row: profile identity, running/paused/stopped, the
/// last error and the committed progress counts.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SemanticState {
    pub profile_name: Option<String>,
    pub function_digest: Option<String>,
    /// The recipe id the recorded partitions were built under.
    pub recipe_id: Option<String>,
    /// `stopped`, `paused` or `running`.
    pub state: String,
    pub last_error: Option<StateError>,
    /// Vectors durably committed since the last purge; updated in the SAME
    /// transaction as each cache batch.
    pub committed_units: u64,
    /// Exact byte total of the cache rows, updated with each batch commit.
    pub cache_bytes: u64,
    /// The last provider state a preparation run observed, if any.
    #[serde(default)]
    pub provider: Option<ProviderObservation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateError {
    pub code: String,
    pub message: String,
}

impl SemanticState {
    pub fn stopped() -> Self {
        Self {
            state: "stopped".into(),
            ..Self::default()
        }
    }
}

/// What one cache lookup found. `Corrupt` rows are disabled by name and
/// counted; they are never served and never silently reset.
#[derive(Clone, Debug)]
pub enum CacheLookup {
    Hit(Vec<f32>),
    Miss,
    Corrupt(String),
}

/// A metadata-only classification of one cache row (no vector decode).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheProbe {
    Absent,
    /// Valid layout, stored under the active function's digest.
    Current,
    /// Valid layout stored under ANOTHER function's digest: retention.
    Retained,
    Corrupt,
}

/// Totals of one cache census.
#[derive(Clone, Copy, Debug, Default)]
pub struct CacheCensus {
    pub entries: u64,
    pub bytes: u64,
    pub corrupt: u64,
    /// Valid rows no current partition references (any function).
    pub orphan: u64,
}

/// The bounded page of admitted sources after `after`.
pub(crate) fn source_page(
    db: &Database,
    after: Option<&str>,
) -> FResult<Vec<(String, SourceMeta)>> {
    let tx = db.begin_read()?;
    let sources = tx.open_table(SOURCES)?;
    let rows = match after {
        None => sources.range::<&str>(..)?,
        Some(key) => sources.range::<&str>((Bound::Excluded(key), Bound::Unbounded))?,
    };
    let mut page = Vec::with_capacity(PAGE);
    for row in rows.take(PAGE) {
        let (key, raw) = row?;
        let meta: SourceMeta = decode(raw.value(), "source")?;
        page.push((key.value().to_owned(), meta));
    }
    Ok(page)
}

/// Create the three semantic tables (initialization and the v5 upgrade).
pub(crate) fn init_tables(tx: &WriteTransaction) -> FResult<()> {
    tx.open_table(PARTITIONS)?;
    tx.open_table(CACHE)?;
    tx.open_table(STATE)?;
    Ok(())
}

/// A schema-5 store without its semantic tables is corrupt; an open never
/// recreates them.
pub(crate) fn check_tables(db: &Database) -> FResult<()> {
    let tx = db.begin_read()?;
    let missing = |name: &str, error: redb::TableError| {
        FoundryError::CorruptStore(format!("{name} table: {error}"))
    };
    tx.open_table(PARTITIONS)
        .map_err(|e| missing("semantic_partitions", e))?;
    tx.open_table(CACHE)
        .map_err(|e| missing("semantic_cache", e))?;
    tx.open_table(STATE)
        .map_err(|e| missing("semantic_state", e))?;
    Ok(())
}

/// Decode one stored partition row; undecodable rows are corruption by name.
pub fn decode_partition(raw: &str) -> FResult<PartitionRecord> {
    serde_json::from_str(raw).map_err(|e| {
        FoundryError::CorruptStore(format!("semantic partition row cannot be decoded: {e}"))
    })
}

fn decode_state(raw: &str) -> FResult<SemanticState> {
    serde_json::from_str(raw).map_err(|e| {
        FoundryError::CorruptStore(format!("semantic state row cannot be decoded: {e}"))
    })
}

/// The immutable source body and function digest a mapping is checked
/// against at acceptance.
#[derive(Clone, Copy)]
pub struct Identity<'a> {
    pub body: &'a str,
    pub function_digest: &'a str,
}

/// Validate a unit list against a source of `len` bytes: exact contiguous
/// nonoverlapping cover, no empty or inverted range and key shape. An empty
/// list is legal only for an empty source, and an empty source carries no
/// units. With `identity` (acceptance, where the immutable body is at hand)
/// every range must also sit on UTF-8 boundaries and every key must equal
/// `input_key(function digest, render_document(exact unit bytes))`: an
/// arbitrary or cross-source key is refused.
pub fn validate_units(
    units: &[PartitionUnit],
    len: usize,
    identity: Option<Identity<'_>>,
) -> Result<(), String> {
    if len == 0 {
        return if units.is_empty() {
            Ok(())
        } else {
            Err("an empty source cannot carry units".into())
        };
    }
    if units.is_empty() {
        return Err("a nonempty source cannot carry an empty partition".into());
    }
    let mut cursor = 0usize;
    for (index, unit) in units.iter().enumerate() {
        if unit.start != cursor {
            return Err(format!(
                "unit {index} starts at {} but the cover is at {cursor}",
                unit.start
            ));
        }
        if unit.end <= unit.start {
            return Err(format!("unit {index} is empty or inverted"));
        }
        if unit.end > len {
            return Err(format!(
                "unit {index} ends at {} beyond the {len}-byte source",
                unit.end
            ));
        }
        if !is_key(&unit.input_key) {
            return Err(format!("unit {index} has a malformed input key"));
        }
        if let Some(identity) = identity {
            let Some(text) = identity.body.get(unit.start..unit.end) else {
                return Err(format!("unit {index} splits a UTF-8 character"));
            };
            let expected =
                provider::input_key(identity.function_digest, &provider::render_document(text));
            if unit.input_key != expected {
                return Err(format!(
                    "unit {index} key does not name its exact rendered input"
                ));
            }
        }
        cursor = unit.end;
    }
    if cursor != len {
        return Err(format!("units cover {cursor} of {len} bytes"));
    }
    Ok(())
}

/// Whether one partition row is current for this source version, recipe and
/// document function, with unit ranges that still satisfy the metadata-level
/// invariants against the source's recorded length. Every serving and
/// eligibility lookup checks all of it; a source change makes its old mapping
/// ineligible immediately.
pub fn partition_is_current(
    record: &PartitionRecord,
    meta: &SourceMeta,
    recipe_id: &str,
    function_digest: &str,
) -> bool {
    record.source_hash == meta.hash
        && record.recipe_id == recipe_id
        && record.function_digest == function_digest
        && validate_units(&record.units, meta.bytes, None).is_ok()
}

fn invalid_partition(message: impl Into<String>) -> FoundryError {
    FoundryError::Semantic {
        code: "partition_invalid",
        message: message.into(),
    }
}

fn probe_row(bytes: &[u8], function_digest: &str) -> CacheProbe {
    if bytes.len() != CACHE_VALUE_BYTES {
        return CacheProbe::Corrupt;
    }
    let stored = &bytes[..64];
    if !stored
        .iter()
        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return CacheProbe::Corrupt;
    }
    if stored == function_digest.as_bytes() {
        CacheProbe::Current
    } else {
        CacheProbe::Retained
    }
}

impl Engine {
    /// The recorded partition of one source, if any. Metadata-only.
    pub fn semantic_partition(&self, path: &str) -> FResult<Option<PartitionRecord>> {
        neural_fault!(PARTITION_READ, None, path)?;
        let tx = self.db.begin_read()?;
        let table = tx.open_table(PARTITIONS)?;
        match table.get(path)? {
            None => Ok(None),
            Some(raw) => decode_partition(raw.value()).map(Some),
        }
    }

    /// Accept one completed partition (or a completed empty one). The CURRENT
    /// source version and its immutable body are read in the same write
    /// transaction, and the mapping is refused unless it names that version
    /// and covers it exactly: bounds, ordering, UTF-8 boundaries, key shape,
    /// empty-partition legality and — from the body — that every key is the
    /// identity of the exact rendered bytes of its range.
    pub fn semantic_record_partition(&self, path: &str, record: &PartitionRecord) -> FResult<()> {
        let tx = self.db.begin_write()?;
        {
            let sources = tx.open_table(SOURCES)?;
            let meta: SourceMeta = match sources.get(path)? {
                Some(raw) => decode(raw.value(), "source")?,
                None => return Err(FoundryError::NotFound),
            };
            if meta.hash != record.source_hash {
                return Err(invalid_partition(format!(
                    "{path}: the mapping names a stale source version"
                )));
            }
            let chunks = tx.open_table(CHUNKS)?;
            let body = reconstruct_verified(&chunks, path, &meta)?.body;
            validate_units(
                &record.units,
                body.len(),
                Some(Identity {
                    body: &body,
                    function_digest: &record.function_digest,
                }),
            )
            .map_err(|m| invalid_partition(format!("{path}: {m}")))?;
            let mut table = tx.open_table(PARTITIONS)?;
            table.insert(
                path,
                serde_json::to_string(record)
                    .map_err(FoundryError::from)?
                    .as_str(),
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The state row; `None` before the first preparation or after a purge.
    /// A persisted `running` can only be a dead owner's leftover (preparation
    /// runs only inside the store's one owner: an exclusive CLI command, or
    /// the MCP owner, which overlays its own live driver state), so it reads
    /// back as `stopped` with an `interrupted` last error — never as `running`.
    pub fn semantic_state(&self) -> FResult<Option<SemanticState>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(STATE)?;
        let Some(raw) = table.get(STATE_KEY)? else {
            return Ok(None);
        };
        let mut state = decode_state(raw.value())?;
        if state.state == "running" {
            state.state = "stopped".into();
            state.last_error.get_or_insert_with(|| StateError {
                code: "interrupted".into(),
                message: "the preparation owner exited before finalizing; \
                          committed work stays and an explicit prepare resumes"
                    .into(),
            });
        }
        Ok(Some(state))
    }

    /// Write the state row atomically.
    pub fn semantic_set_state(&self, state: &SemanticState) -> FResult<()> {
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(STATE)?;
            table.insert(
                STATE_KEY,
                serde_json::to_string(state)
                    .map_err(FoundryError::from)?
                    .as_str(),
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Metadata-only classification of one cache row (length and digest
    /// only; no vector decode).
    pub fn semantic_cache_probe(&self, key: &str, function_digest: &str) -> FResult<CacheProbe> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(CACHE)?;
        Ok(match table.get(key)? {
            None => CacheProbe::Absent,
            Some(raw) => probe_row(raw.value(), function_digest),
        })
    }

    /// One serving lookup under the CURRENT function digest. A row carrying
    /// another digest, a wrong layout or nonfinite bytes is corrupt: named,
    /// never served, never reset. This is the lookup that catches a payload
    /// tampered with after commit (a same-length NaN/infinity row passes the
    /// metadata probe but not this decode); preparation and the index rebuild
    /// both use it, disable the row by name and re-embed it.
    pub fn semantic_cache_lookup(&self, key: &str, function_digest: &str) -> FResult<CacheLookup> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(CACHE)?;
        let Some(raw) = table.get(key)? else {
            return Ok(CacheLookup::Miss);
        };
        let bytes = raw.value();
        let short = key.get(..8).unwrap_or(key);
        match probe_row(bytes, function_digest) {
            CacheProbe::Corrupt | CacheProbe::Absent => Ok(CacheLookup::Corrupt(format!(
                "semantic_cache[{short}…] has an invalid layout ({} bytes)",
                bytes.len()
            ))),
            CacheProbe::Retained => Ok(CacheLookup::Corrupt(format!(
                "semantic_cache[{short}…] carries a foreign function digest"
            ))),
            CacheProbe::Current => match decode_vector(&bytes[64..]) {
                Some(vector) => Ok(CacheLookup::Hit(vector)),
                None => Ok(CacheLookup::Corrupt(format!(
                    "semantic_cache[{short}…] holds a nonfinite vector"
                ))),
            },
        }
    }

    /// Cache rows and their exact byte total (fixed-layout rows).
    pub fn semantic_cache_totals(&self) -> FResult<(u64, u64)> {
        let tx = self.db.begin_read()?;
        let rows = tx.open_table(CACHE)?.len()?;
        Ok((rows, rows * CACHE_VALUE_BYTES as u64))
    }

    /// Paged, metadata-only census of the whole cache: each row is validated
    /// against its OWN layout and stored digest (length and digest only, no
    /// vector decode — status trusts committed payloads), valid rows no
    /// current mapping references are retention, and `control` is checked
    /// before every page.
    pub fn semantic_cache_census(
        &self,
        control: &Control,
        referenced: &HashSet<String>,
    ) -> FResult<CacheCensus> {
        let mut census = CacheCensus::default();
        let mut after: Option<String> = None;
        loop {
            control.check()?;
            let tx = self.db.begin_read()?;
            let table = tx.open_table(CACHE)?;
            let rows = match &after {
                None => table.range::<&str>(..)?,
                Some(key) => {
                    table.range::<&str>((Bound::Excluded(key.as_str()), Bound::Unbounded))?
                }
            };
            let mut seen = 0usize;
            let mut last: Option<String> = None;
            for row in rows.take(PAGE) {
                let (key, value) = row?;
                let bytes = value.value();
                census.entries += 1;
                census.bytes += bytes.len() as u64;
                match probe_row(bytes, "") {
                    CacheProbe::Corrupt => census.corrupt += 1,
                    _ => {
                        if !referenced.contains(key.value()) {
                            census.orphan += 1;
                        }
                    }
                }
                seen += 1;
                last = Some(key.value().to_owned());
            }
            if seen == 0 {
                return Ok(census);
            }
            after = last;
        }
    }

    /// Commit one validated batch of vectors under `function_digest`, cache
    /// commits before any index publication. The disk cap stops the run with
    /// `cache_full` BEFORE this batch is written: nothing is evicted and
    /// earlier valid data stays intact. The committed count and the exact
    /// cache-byte total are updated in the SAME transaction, so durable
    /// vectors and durable progress can never disagree.
    pub fn semantic_cache_commit(
        &self,
        entries: &[(String, Vec<f32>)],
        function_digest: &str,
        cap_bytes: u64,
    ) -> FResult<()> {
        if entries.is_empty() {
            return Ok(());
        }
        for (key, vector) in entries {
            provider::validate_vector(vector).map_err(FoundryError::from)?;
            if !is_key(key) {
                return Err(FoundryError::InvalidArgument(format!(
                    "cache key {key:?} is not a 64-hex digest"
                )));
            }
        }
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(CACHE)?;
            let existing = table.len()?;
            let mut fresh = 0u64;
            for (key, _) in entries {
                if table.get(key.as_str())?.is_none() {
                    fresh += 1;
                }
            }
            if (existing + fresh) * CACHE_VALUE_BYTES as u64 > cap_bytes {
                return Err(FoundryError::Semantic {
                    code: "cache_full",
                    message: format!(
                        "committing {fresh} vectors would put the cache over \
                         {cap_bytes} bytes ({existing} rows held); nothing was evicted"
                    ),
                });
            }
            for (key, vector) in entries {
                let mut value = Vec::with_capacity(CACHE_VALUE_BYTES);
                value.extend_from_slice(function_digest.as_bytes());
                for component in vector {
                    value.extend_from_slice(&component.to_le_bytes());
                }
                table.insert(key.as_str(), value.as_slice())?;
            }
            let mut state_table = tx.open_table(STATE)?;
            let mut state = state_table
                .get(STATE_KEY)?
                .map(|raw| decode_state(raw.value()))
                .transpose()?
                .unwrap_or_else(SemanticState::stopped);
            state.committed_units += entries.len() as u64;
            state.cache_bytes = (existing + fresh) * CACHE_VALUE_BYTES as u64;
            state_table.insert(
                STATE_KEY,
                serde_json::to_string(&state)
                    .map_err(FoundryError::from)?
                    .as_str(),
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Reconstruct and verify one source's current bytes inside a read
    /// transaction (preparation input; never a query or status side effect).
    #[cfg(feature = "semantic")]
    pub(crate) fn semantic_source_body(&self, path: &str, meta: &SourceMeta) -> FResult<String> {
        let tx = self.db.begin_read()?;
        let chunks = tx.open_table(CHUNKS)?;
        Ok(reconstruct_verified(&chunks, path, meta)?.body)
    }

    /// Offline purge under sole ownership: partitions, cache and state in one
    /// transaction, then the derived generation directories. Everything on
    /// disk starts from a duplicate of the Engine's bound store-directory
    /// descriptor: the plan (a symlink anywhere refuses the purge before any
    /// row changes) and the deletion both work through it, so renaming or
    /// substituting an ancestor path — before OR after the plan — cannot
    /// redirect the deletion. Sources, graph, memory and feedback are
    /// untouched; nothing repopulates later.
    pub fn semantic_purge(&self) -> FResult<SemanticPurgeReport> {
        let store = self.semantic_anchor()?;
        let plan = plan_generation_removal(&store)?;
        let (partitions, cache_rows, cache_bytes) = {
            let tx = self.db.begin_write()?;
            let (partitions, rows) = {
                let mut partition_table = tx.open_table(PARTITIONS)?;
                let mut cache = tx.open_table(CACHE)?;
                let partitions = partition_table.len()?;
                let rows = cache.len()?;
                let mut partition_keys = Vec::new();
                for row in partition_table.iter()? {
                    partition_keys.push(row?.0.value().to_owned());
                }
                let mut cache_keys = Vec::new();
                for row in cache.iter()? {
                    cache_keys.push(row?.0.value().to_owned());
                }
                for key in partition_keys {
                    partition_table.remove(key.as_str())?;
                }
                for key in cache_keys {
                    cache.remove(key.as_str())?;
                }
                (partitions, rows)
            };
            {
                let mut state = tx.open_table(STATE)?;
                state.insert(
                    STATE_KEY,
                    serde_json::to_string(&SemanticState::stopped())
                        .map_err(FoundryError::from)?
                        .as_str(),
                )?;
            }
            tx.commit()?;
            (partitions, rows, rows * CACHE_VALUE_BYTES as u64)
        };
        // Rows are gone; the descriptor-relative deletion follows. The hook
        // is where tests substitute the path of `semantic/` AFTER the plan.
        neural_fault!(PURGE_AFTER_PLAN, None, &self.directory().to_string_lossy())?;
        let mut removed = 0u32;
        if let Some((root, names)) = plan {
            for name in names {
                if root
                    .remove_tree(name)
                    .map_err(|e| conflict_or_io(e, "generation directory"))?
                {
                    removed += 1;
                }
            }
        }
        Ok(SemanticPurgeReport {
            removed_partitions: partitions,
            removed_cache_entries: cache_rows,
            removed_cache_bytes: cache_bytes,
            removed_generation_dirs: removed,
            state: "stopped",
        })
    }
}

/// What `foundry semantic purge` did.
#[derive(Clone, Debug, Serialize)]
pub struct SemanticPurgeReport {
    pub removed_partitions: u64,
    pub removed_cache_entries: u64,
    pub removed_cache_bytes: u64,
    pub removed_generation_dirs: u32,
    pub state: &'static str,
}

/// The open `semantic/` root (a descriptor under the Engine's bound store
/// directory) and the names of its real generation directories. A symlink
/// as the root or as a child refuses the whole plan before anything is
/// removed; regular files are not generations and are left alone. `None`
/// when there is no `semantic/` directory.
pub(crate) fn plan_generation_removal(store: &Dir) -> FResult<Option<(Dir, Vec<OsString>)>> {
    let Some(root) = store
        .open_dir(SEMANTIC_DIR)
        .map_err(|e| conflict_or_io(e, "semantic root"))?
    else {
        return Ok(None);
    };
    let mut names = Vec::new();
    for (name, kind) in root
        .entries()
        .map_err(|e| conflict_or_io(e, "semantic root"))?
    {
        match kind {
            crate::neural::anchor::Kind::Symlink => {
                return Err(FoundryError::RepairPathConflict(format!(
                    "generation directory {} is a symlink",
                    name.to_string_lossy()
                )));
            }
            crate::neural::anchor::Kind::Dir => names.push(name),
            _ => {}
        }
    }
    Ok(Some((root, names)))
}

/// Cache-row vector decode: exact length and finite values, else `None`.
fn decode_vector(bytes: &[u8]) -> Option<Vec<f32>> {
    if bytes.len() != provider::DIMENSIONS * 4 {
        return None;
    }
    let mut vector = Vec::with_capacity(provider::DIMENSIONS);
    for chunk in bytes.chunks_exact(4) {
        let value = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        if !value.is_finite() {
            return None;
        }
        vector.push(value);
    }
    Some(vector)
}

pub(crate) fn is_key(key: &str) -> bool {
    key.len() == 64 && key.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
