//! 009 semantic storage (store schema 5): the partition table, the f32
//! vector cache and the preparation-state row. Compiled in every build —
//! semantic rows survive a build without the `semantic` feature; only the
//! tokenizer and the ANN index are feature-gated elsewhere.
//!
//! Conventions follow the store: rows are strict JSON (or fixed-layout
//! bytes), a decode failure is corruption by name, cache commits precede any
//! derived publication, and the disk cap stops preparation with `cache_full`
//! instead of evicting. Every eligibility check compares the CURRENT source
//! hash, recipe, function digest AND the card ranges against the source's
//! recorded length, so a source change (or a malformed mapping) makes the
//! old mapping ineligible immediately; source commits never touch these
//! tables. Acceptance writes only the cards the verified body renders
//! ([`Engine::semantic_record_partition`]): every recorded
//! `(start, end, input_key)` tuple is bound to its rendered input.
//!
//! 009 T004 cache rows are self-describing: the 64-hex function digest, the
//! dimension as `u32` LE, then that many `f32` LE values. A row of exactly
//! [`LEGACY_ROW_BYTES`] (a digest and 2048 values, no dimension) is a
//! descriptor v1 row: retained and accounted, never served by a v2
//! profile, removed only by `semantic purge`. Accounting sums the actual
//! row lengths.
use crate::control::Control;
use crate::error::{FResult, FoundryError};
use crate::neural::anchor::{Dir, conflict_or_io};
use crate::neural::index::SEMANTIC_DIR;
use crate::neural::provider::{self, MAX_DIMENSIONS};
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
/// Default cache cap: 2 GiB per workspace (D001 chosen values). An operator
/// may set another per run (`semantic prepare --cache-cap`, `mcp
/// --semantic-cache-cap`); the run records it in the state row.
pub const DEFAULT_CACHE_CAP_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// A descriptor v1 cache row: 64 hex digest bytes and 2048 f32 LE values,
/// with no dimension field.
pub const LEGACY_ROW_BYTES: usize = 64 + 2048 * 4;
/// The paged-walk bound shared with every other store census.
pub const PAGE: usize = 128;

/// The byte length of a self-describing row of `dims` values.
pub const fn row_bytes(dims: usize) -> usize {
    64 + 4 + dims * 4
}

/// One partitioned source version: the source hash and recipe it belongs to,
/// and its card ranges with their document-input keys. An empty `units`
/// list is a COMPLETED partition of a source without cards, distinguishable
/// from a missing row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartitionRecord {
    pub source_hash: String,
    pub recipe_id: String,
    /// The function digest the card input keys were computed under.
    pub function_digest: String,
    pub units: Vec<PartitionUnit>,
}

/// One card's provenance: the unit range it was rendered from and the key
/// of its exact rendered input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    /// 009 T004: the output dimension of the recorded profile. A row
    /// written before T004 has none: its profile (descriptor v1) is no
    /// longer served, and its generation is kept, never rebuilt.
    #[serde(default)]
    pub dimensions: Option<u32>,
    /// `stopped`, `paused` or `running`.
    pub state: String,
    pub last_error: Option<StateError>,
    /// Vectors durably committed since the last purge; updated in the SAME
    /// transaction as each cache batch.
    pub committed_units: u64,
    /// Exact byte total of the cache rows (their actual lengths), updated
    /// with each batch commit.
    pub cache_bytes: u64,
    /// The cache cap of the last run that began; `None` before one recorded
    /// it, which means [`DEFAULT_CACHE_CAP_BYTES`].
    #[serde(default)]
    pub cache_cap_bytes: Option<u64>,
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
    /// Valid self-describing layout, stored under the active function's
    /// digest.
    Current,
    /// Valid layout stored under ANOTHER function's digest, or a descriptor
    /// v1 row: retention.
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
    /// Valid descriptor v1 rows (no dimension field), retained until purge.
    pub legacy: u64,
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

/// Validate a card list against a source of `len` bytes: nonempty ranges
/// inside the source in the unit forest's pre-order (start ascending, the
/// enclosing range first among equal starts), no range twice, and key
/// shape. Any source may carry zero cards. With `body` (acceptance, where
/// the immutable body is at hand) every range must also sit on UTF-8
/// boundaries.
pub fn validate_units(
    units: &[PartitionUnit],
    len: usize,
    body: Option<&str>,
) -> Result<(), String> {
    for (index, unit) in units.iter().enumerate() {
        if unit.end <= unit.start {
            return Err(format!("card {index} is empty or inverted"));
        }
        if unit.end > len {
            return Err(format!(
                "card {index} ends at {} beyond the {len}-byte source",
                unit.end
            ));
        }
        if !is_key(&unit.input_key) {
            return Err(format!("card {index} has a malformed input key"));
        }
        if index > 0 {
            let previous = &units[index - 1];
            let ordered = previous.start < unit.start
                || (previous.start == unit.start && previous.end > unit.end);
            if !ordered {
                return Err(format!("card {index} is out of order or repeats a range"));
            }
        }
        if let Some(body) = body
            && body.get(unit.start..unit.end).is_none()
        {
            return Err(format!("card {index} splits a UTF-8 character"));
        }
    }
    Ok(())
}

/// Where a claimed card list departs from the cards the body renders: the
/// first card whose range or key differs, else a different count; `None`
/// when every `(start, end, input_key)` tuple matches.
fn first_difference(claimed: &[PartitionUnit], rendered: &[PartitionUnit]) -> Option<String> {
    let short = |key: &str| key.get(..8).unwrap_or(key).to_owned();
    if let Some(index) = claimed.iter().zip(rendered).position(|(a, b)| a != b) {
        let (claim, card) = (&claimed[index], &rendered[index]);
        return Some(format!(
            "card {index} claims {}..{} under key {}…, but the body renders {}..{} under key {}…",
            claim.start,
            claim.end,
            short(&claim.input_key),
            card.start,
            card.end,
            short(&card.input_key)
        ));
    }
    (claimed.len() != rendered.len()).then(|| {
        format!(
            "the mapping claims {} cards, but the body renders {}",
            claimed.len(),
            rendered.len()
        )
    })
}

/// Whether one partition row is current for this source version, recipe and
/// document function, with card ranges that still satisfy the
/// metadata-level invariants against the source's recorded length. Every
/// serving and eligibility lookup checks all of it; a source change makes
/// its old mapping ineligible immediately.
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

/// The layout of one cache row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
    /// A descriptor v1 row: digest and 2048 values, no dimension.
    Legacy,
    /// A self-describing row of this many values.
    Dims(usize),
    Corrupt,
}

fn hex_digest(bytes: &[u8]) -> bool {
    bytes.iter().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn layout(bytes: &[u8]) -> Layout {
    if bytes.len() < 64 || !hex_digest(&bytes[..64]) {
        return Layout::Corrupt;
    }
    if bytes.len() == LEGACY_ROW_BYTES {
        return Layout::Legacy;
    }
    let Some(field) = bytes.get(64..68) else {
        return Layout::Corrupt;
    };
    let dims = u32::from_le_bytes([field[0], field[1], field[2], field[3]]) as usize;
    if (1..=MAX_DIMENSIONS).contains(&dims) && bytes.len() == row_bytes(dims) {
        Layout::Dims(dims)
    } else {
        Layout::Corrupt
    }
}

fn probe_row(bytes: &[u8], function_digest: &str) -> CacheProbe {
    match layout(bytes) {
        Layout::Corrupt => CacheProbe::Corrupt,
        Layout::Legacy => CacheProbe::Retained,
        Layout::Dims(_) if &bytes[..64] == function_digest.as_bytes() => CacheProbe::Current,
        Layout::Dims(_) => CacheProbe::Retained,
    }
}

/// One self-describing row: digest, dimension, values.
fn encode_row(function_digest: &str, vector: &[f32]) -> Vec<u8> {
    let mut value = Vec::with_capacity(row_bytes(vector.len()));
    value.extend_from_slice(function_digest.as_bytes());
    value.extend_from_slice(&(vector.len() as u32).to_le_bytes());
    for component in vector {
        value.extend_from_slice(&component.to_le_bytes());
    }
    value
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

    /// Accept one completed card mapping (possibly without cards) for the
    /// CURRENT version of `path`, named by `source_hash`. That version and
    /// its immutable body are read in the write transaction, and the row
    /// written is `cards` applied to THAT body (the run's card renderer
    /// under `recipe_id` and `function_digest`): every accepted
    /// `(start, end, input_key)` tuple is the card the verified body renders
    /// at that range, whether or not its vector is cached, so a vector is
    /// never published under a range it was not rendered from. `claimed`, a
    /// mapping rendered elsewhere, must equal that rendering tuple for
    /// tuple. A stale version, a differing claim, or a rendering outside the
    /// structural invariants (bounds, order, UTF-8 boundaries, key shape) is
    /// `partition_invalid` and nothing is written.
    pub fn semantic_record_partition(
        &self,
        path: &str,
        source_hash: &str,
        recipe_id: &str,
        function_digest: &str,
        claimed: Option<&[PartitionUnit]>,
        cards: &dyn Fn(&str) -> FResult<Vec<PartitionUnit>>,
    ) -> FResult<()> {
        let tx = self.db.begin_write()?;
        {
            let sources = tx.open_table(SOURCES)?;
            let meta: SourceMeta = match sources.get(path)? {
                Some(raw) => decode(raw.value(), "source")?,
                None => return Err(FoundryError::NotFound),
            };
            if meta.hash != source_hash {
                return Err(invalid_partition(format!(
                    "{path}: the mapping names a stale source version"
                )));
            }
            let chunks = tx.open_table(CHUNKS)?;
            let body = reconstruct_verified(&chunks, path, &meta)?.body;
            let units = cards(&body)?;
            validate_units(&units, body.len(), Some(&body))
                .map_err(|m| invalid_partition(format!("{path}: {m}")))?;
            if let Some(claimed) = claimed
                && let Some(message) = first_difference(claimed, &units)
            {
                return Err(invalid_partition(format!("{path}: {message}")));
            }
            let record = PartitionRecord {
                source_hash: meta.hash,
                recipe_id: recipe_id.to_owned(),
                function_digest: function_digest.to_owned(),
                units,
            };
            let mut table = tx.open_table(PARTITIONS)?;
            table.insert(
                path,
                serde_json::to_string(&record)
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

    /// Record a preparation run's start in ONE write transaction: `start`
    /// sets the run's identity on the state row, after the row's cache-byte
    /// total was reconciled from the actual row lengths. Another binary may
    /// have recorded a different total (a pre-T004 finalizer counts 8,256
    /// bytes per row); nothing is purged, and cache-cap admission counts real
    /// bytes from the run's first commit on.
    pub fn semantic_begin_run(&self, start: impl FnOnce(&mut SemanticState)) -> FResult<()> {
        let tx = self.db.begin_write()?;
        {
            let cache = tx.open_table(CACHE)?;
            let mut bytes = 0u64;
            for row in cache.iter()? {
                bytes += row?.1.value().len() as u64;
            }
            let mut table = tx.open_table(STATE)?;
            let mut state = table
                .get(STATE_KEY)?
                .map(|raw| decode_state(raw.value()))
                .transpose()?
                .unwrap_or_default();
            state.cache_bytes = bytes;
            start(&mut state);
            table.insert(
                STATE_KEY,
                serde_json::to_string(&state)
                    .map_err(FoundryError::from)?
                    .as_str(),
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Metadata-only classification of one cache row (layout and digest
    /// only; no vector decode).
    pub fn semantic_cache_probe(&self, key: &str, function_digest: &str) -> FResult<CacheProbe> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(CACHE)?;
        Ok(match table.get(key)? {
            None => CacheProbe::Absent,
            Some(raw) => probe_row(raw.value(), function_digest),
        })
    }

    /// One serving lookup under the CURRENT function digest and its
    /// dimension. A row carrying another digest, a descriptor v1 row, a
    /// wrong layout or dimension, or nonfinite bytes is corrupt for this
    /// lookup: named, never served, never reset. This is the lookup that
    /// catches a payload tampered with after commit (a same-length
    /// NaN/infinity row passes the metadata probe but not this decode);
    /// preparation and the index rebuild both use it, disable the row by
    /// name and re-embed it.
    pub fn semantic_cache_lookup(
        &self,
        key: &str,
        function_digest: &str,
        dims: usize,
    ) -> FResult<CacheLookup> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(CACHE)?;
        let Some(raw) = table.get(key)? else {
            return Ok(CacheLookup::Miss);
        };
        let bytes = raw.value();
        let short = key.get(..8).unwrap_or(key);
        match (probe_row(bytes, function_digest), layout(bytes)) {
            (CacheProbe::Current, Layout::Dims(stored)) if stored == dims => {
                match decode_vector(&bytes[68..]) {
                    Some(vector) => Ok(CacheLookup::Hit(vector)),
                    None => Ok(CacheLookup::Corrupt(format!(
                        "semantic_cache[{short}…] holds a nonfinite vector"
                    ))),
                }
            }
            (CacheProbe::Current, Layout::Dims(stored)) => Ok(CacheLookup::Corrupt(format!(
                "semantic_cache[{short}…] holds {stored} values; the profile has {dims}"
            ))),
            (CacheProbe::Retained, Layout::Legacy) => Ok(CacheLookup::Corrupt(format!(
                "semantic_cache[{short}…] is a descriptor v1 row"
            ))),
            (CacheProbe::Retained, _) => Ok(CacheLookup::Corrupt(format!(
                "semantic_cache[{short}…] carries a foreign function digest"
            ))),
            _ => Ok(CacheLookup::Corrupt(format!(
                "semantic_cache[{short}…] has an invalid layout ({} bytes)",
                bytes.len()
            ))),
        }
    }

    /// Cache rows and the exact sum of their actual lengths (one read).
    pub fn semantic_cache_totals(&self) -> FResult<(u64, u64)> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(CACHE)?;
        let (mut rows, mut bytes) = (0u64, 0u64);
        for row in table.iter()? {
            let (_, value) = row?;
            rows += 1;
            bytes += value.value().len() as u64;
        }
        Ok((rows, bytes))
    }

    /// Paged, metadata-only census of the whole cache: each row is validated
    /// against its OWN layout (a self-describing row or a descriptor v1 row;
    /// length, dimension and digest only, no vector decode — status trusts
    /// committed payloads), valid rows no current mapping references are
    /// retention, and `control` is checked before every page.
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
                match layout(bytes) {
                    Layout::Corrupt => census.corrupt += 1,
                    found => {
                        if found == Layout::Legacy {
                            census.legacy += 1;
                        }
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

    /// Commit one validated batch of `dims`-value vectors under
    /// `function_digest`, cache commits before any index publication. The
    /// disk cap stops the run with `cache_full` BEFORE this batch is
    /// written: nothing is evicted and earlier valid data stays intact. The
    /// committed count and the exact cache-byte total (actual row lengths:
    /// a replaced row's length leaves it, the new row's enters, from the
    /// total the run reconciled at its start, [`Self::semantic_begin_run`])
    /// are updated in the SAME transaction, so durable vectors and durable
    /// progress can never disagree.
    pub fn semantic_cache_commit(
        &self,
        entries: &[(String, Vec<f32>)],
        function_digest: &str,
        dims: usize,
        cap_bytes: u64,
    ) -> FResult<()> {
        if entries.is_empty() {
            return Ok(());
        }
        for (key, vector) in entries {
            provider::validate_vector(vector, dims).map_err(FoundryError::from)?;
            if !is_key(key) {
                return Err(FoundryError::InvalidArgument(format!(
                    "cache key {key:?} is not a 64-hex digest"
                )));
            }
        }
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(CACHE)?;
            let mut state_table = tx.open_table(STATE)?;
            let mut state = state_table
                .get(STATE_KEY)?
                .map(|raw| decode_state(raw.value()))
                .transpose()?
                .unwrap_or_else(SemanticState::stopped);
            let mut bytes = state.cache_bytes;
            let mut replaced: HashSet<&str> = HashSet::new();
            for (key, _) in entries {
                if replaced.insert(key.as_str())
                    && let Some(old) = table.get(key.as_str())?
                {
                    bytes = bytes.saturating_sub(old.value().len() as u64);
                }
            }
            bytes += replaced.len() as u64 * row_bytes(dims) as u64;
            if bytes > cap_bytes {
                return Err(FoundryError::Semantic {
                    code: "cache_full",
                    message: format!(
                        "committing {} vectors would put the cache at {bytes} bytes, over \
                         {cap_bytes}; nothing was evicted",
                        entries.len()
                    ),
                });
            }
            for (key, vector) in entries {
                table.insert(key.as_str(), encode_row(function_digest, vector).as_slice())?;
            }
            state.committed_units += entries.len() as u64;
            state.cache_bytes = bytes;
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
    /// untouched; nothing repopulates later. Descriptor v1 rows and
    /// generations go with everything else: this is the only removal.
    pub fn semantic_purge(&self) -> FResult<SemanticPurgeReport> {
        let store = self.semantic_anchor()?;
        let plan = plan_generation_removal(&store)?;
        let (partitions, cache_rows, cache_bytes) = {
            let tx = self.db.begin_write()?;
            let (partitions, rows, bytes) = {
                let mut partition_table = tx.open_table(PARTITIONS)?;
                let mut cache = tx.open_table(CACHE)?;
                let partitions = partition_table.len()?;
                let rows = cache.len()?;
                let mut partition_keys = Vec::new();
                for row in partition_table.iter()? {
                    partition_keys.push(row?.0.value().to_owned());
                }
                let mut cache_keys = Vec::new();
                let mut bytes = 0u64;
                for row in cache.iter()? {
                    let (key, value) = row?;
                    bytes += value.value().len() as u64;
                    cache_keys.push(key.value().to_owned());
                }
                for key in partition_keys {
                    partition_table.remove(key.as_str())?;
                }
                for key in cache_keys {
                    cache.remove(key.as_str())?;
                }
                (partitions, rows, bytes)
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
            (partitions, rows, bytes)
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

/// Cache-row vector decode of a row's value bytes: finite values, else
/// `None`.
fn decode_vector(bytes: &[u8]) -> Option<Vec<f32>> {
    let mut vector = Vec::with_capacity(bytes.len() / 4);
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

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "abababababababababababababababababababababababababababababababab";

    #[test]
    fn rows_describe_their_own_dimension_and_legacy_rows_are_retained() {
        for dims in [768, 512, 256, 128, 2048] {
            let row = encode_row(DIGEST, &vec![0.5; dims]);
            assert_eq!(row.len(), row_bytes(dims));
            assert_eq!(layout(&row), Layout::Dims(dims));
            assert_eq!(probe_row(&row, DIGEST), CacheProbe::Current);
            assert_eq!(probe_row(&row, &"cd".repeat(32)), CacheProbe::Retained);
            assert_eq!(decode_vector(&row[68..]).unwrap().len(), dims);
        }
        // A descriptor v1 row: digest and 2048 values, no dimension field.
        let mut legacy = DIGEST.as_bytes().to_vec();
        legacy.extend(std::iter::repeat_n(0u8, 2048 * 4));
        assert_eq!(layout(&legacy), Layout::Legacy);
        assert_eq!(probe_row(&legacy, DIGEST), CacheProbe::Retained);
        // A dimension field that disagrees with the length, a zero or
        // oversized dimension, a non-hex digest: corrupt.
        let mut wrong = encode_row(DIGEST, &[1.0; 8]);
        wrong.truncate(wrong.len() - 4);
        assert_eq!(layout(&wrong), Layout::Corrupt);
        assert_eq!(layout(&encode_row(DIGEST, &[])), Layout::Corrupt);
        let mut huge = encode_row(DIGEST, &[1.0; 4]);
        huge[64..68].copy_from_slice(&4096u32.to_le_bytes());
        assert_eq!(layout(&huge), Layout::Corrupt);
        let mut upper = encode_row(DIGEST, &[1.0; 4]);
        upper[0] = b'A';
        assert_eq!(layout(&upper), Layout::Corrupt);
        // No self-describing row of a supported dimension has the legacy
        // length (2047 values would; no profile can pin that width).
        assert!(
            provider::SUPPORTED_DIMENSIONS
                .iter()
                .all(|&dims| row_bytes(dims) != LEGACY_ROW_BYTES)
        );
    }

    #[test]
    fn cards_may_nest_and_be_absent_but_never_repeat_or_leave_the_source() {
        let key = "e".repeat(64);
        let unit = |start: usize, end: usize| PartitionUnit {
            start,
            end,
            input_key: key.clone(),
        };
        assert!(validate_units(&[], 10, None).is_ok());
        assert!(validate_units(&[unit(0, 10), unit(0, 4), unit(5, 9)], 10, None).is_ok());
        assert!(validate_units(&[unit(0, 4), unit(0, 10)], 10, None).is_err());
        assert!(validate_units(&[unit(2, 4), unit(2, 4)], 10, None).is_err());
        assert!(validate_units(&[unit(5, 9), unit(0, 4)], 10, None).is_err());
        assert!(validate_units(&[unit(3, 3)], 10, None).is_err());
        assert!(validate_units(&[unit(0, 11)], 10, None).is_err());
        assert!(validate_units(&[unit(1, 3)], 4, Some("é é")).is_err());
        let mut bad = unit(0, 2);
        bad.input_key = "x".into();
        assert!(validate_units(&[bad], 10, None).is_err());
    }
}
