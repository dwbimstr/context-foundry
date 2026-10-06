//! Authoritative source store (redb) plus a disposable derived search index
//! (Tantivy). One owner per store; explicit initialization, explicit schema
//! upgrade and explicit repair. Reads never mutate authoritative state.
use crate::Strategy;
use crate::error::{FResult, FoundryError};
use crate::graph;
use crate::memory::{MEMORY, MemoryRecord, memory_pending_key, source_pending_key};
use crate::response::{self, Freshness};
use redb::{
    Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition,
    WriteTransaction,
};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::ops::Bound;
use std::path::{Path, PathBuf};
use tantivy::collector::{Count, TopDocs};
use tantivy::query::{BooleanQuery, BoostQuery, Occur, PhraseQuery, Query, TermQuery};
use tantivy::schema::{
    FAST, Field, IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions, Value,
};
use tantivy::tokenizer::{LowerCaser, RawTokenizer, TextAnalyzer, Token, TokenStream, Tokenizer};
use tantivy::{
    DocId, Index, IndexReader, IndexWriter, ReloadPolicy, Score, SegmentReader, TantivyDocument,
    Term,
};

pub(crate) const SOURCES: TableDefinition<&str, &str> = TableDefinition::new("sources");
pub(crate) const CHUNKS: TableDefinition<&str, &str> = TableDefinition::new("chunks");
pub(crate) const PENDING: TableDefinition<&str, &str> = TableDefinition::new("pending_index");
pub(crate) const META: TableDefinition<&str, &str> = TableDefinition::new("meta");
pub(crate) const SEEN: TableDefinition<&str, &str> = TableDefinition::new("scan_seen");
pub(crate) const FEEDBACK: TableDefinition<&str, &str> = TableDefinition::new("feedback");

pub const SCHEMA_VERSION: u32 = 6;
const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
const PAGE: usize = 128;
/// Search tier 2 (lexical) examines at most this many candidates.
const CANDIDATE_LIMIT: usize = 256;
/// Memory search examines at most this many derived documents before the
/// live-row validation (008), the same window as the lexical tier.
pub(crate) const MEMORY_CANDIDATE_WINDOW: usize = CANDIDATE_LIMIT;
/// Search tier 1 (exact definitions) keeps at most this many documents.
const TIER1_LIMIT: usize = 64;
/// Search tier 1 ranks at most this many distinct identifier runs: the first
/// by appearance in the query.
const TIER1_RUNS: usize = 32;
/// At most this many hits per file survive materialization.
const PER_FILE_CAP: usize = 4;
/// Context draws at most this many delivery units from the ranking.
const CONTEXT_UNITS: usize = 32;
/// Context adds at most this many file outlines.
const CONTEXT_OUTLINES: usize = 3;
/// Context graph expansion examines at most this many rows per seed and
/// direction.
const CONTEXT_GRAPH_EDGES: usize = 32;
/// The META value naming the current search index format. `"3"` adds the
/// unit's own start (its head) and the leading-run unit ranges (context-v2
/// § Unit forest).
const SEARCH_SCHEMA: &str = "3";
const SEARCH_SCHEMA_REASON: &str =
    "search_schema: search index format changed; run `foundry repair-index`";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceMeta {
    pub hash: String,
    pub chunks: usize,
    pub bytes: usize,
    pub lines: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub path: String,
    pub hash: String,
    pub start_line: usize,
    pub end_line: usize,
    pub body: String,
}

/// Full-identity source reference: full 64-hex `workspace_id` and `sha256`
/// plus a half-open byte range. The wire form is the v2 string of
/// [`HandleRef`]; the only empty range is `[0,0)` on an empty source.
#[derive(Clone, Debug, Serialize)]
pub struct SourceHandle {
    pub workspace_id: String,
    pub path: String,
    pub sha256: String,
    pub start: u64,
    pub end: u64,
}

/// context-v2 handle input cap: a 4096-byte path plus the longest 92-byte suffix.
const HANDLE_V2_MAX_BYTES: usize = 4200;
pub(crate) const HANDLE_V2_GRAMMAR: &str = "handle must be a v2 string `path#start-end@sha32.ws16`";

/// context-v2 source handle `<path>#<start>-<end>@<sha32>.<ws16>`. It names the
/// first 32 hex digits of the source SHA-256 and the first 16 of `workspace_id`;
/// `Engine::retrieve` compares those prefixes with the stored full values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandleRef {
    pub path: String,
    pub start: u64,
    pub end: u64,
    pub sha32: String,
    pub ws16: String,
}

impl HandleRef {
    /// Stage 1 only (syntax and bounds): everything is `invalid_argument`.
    /// Equivalent to the contract's end-anchored
    /// `#(0|[1-9][0-9]*)-(0|[1-9][0-9]*)@([0-9a-f]{32})\.([0-9a-f]{16})$`, with
    /// the path being everything before the match. The suffix alphabet has no
    /// `#`, so scanning back from the end finds the only possible match.
    pub fn parse(raw: &str) -> FResult<Self> {
        let grammar = || FoundryError::InvalidArgument(HANDLE_V2_GRAMMAR.into());
        if raw.len() > HANDLE_V2_MAX_BYTES {
            // Over the cap nothing is a valid v2 handle; a v1 JSON object still
            // names the v2 grammar. Under the cap a v1 object fails the
            // end-anchored grammar below with the same message, while a v2 path
            // that legitimately starts with `{` still parses.
            if raw.starts_with('{') {
                return Err(grammar());
            }
            return Err(FoundryError::InvalidArgument(format!(
                "handle exceeds {HANDLE_V2_MAX_BYTES} bytes"
            )));
        }
        let bytes = raw.as_bytes();
        let n = bytes.len();
        // `@` + 32 hex + `.` + 16 hex is 50 bytes; `#0-0` needs 4 more.
        if n < 54 {
            return Err(grammar());
        }
        let lower_hex = |s: &[u8]| s.iter().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        let (at, dot) = (n - 50, n - 17);
        if bytes[at] != b'@'
            || bytes[dot] != b'.'
            || !lower_hex(&bytes[at + 1..dot])
            || !lower_hex(&bytes[dot + 1..])
        {
            return Err(grammar());
        }
        let digits_from = |end: usize| {
            let mut s = end;
            while s > 0 && bytes[s - 1].is_ascii_digit() {
                s -= 1;
            }
            s
        };
        let end_digits = digits_from(at);
        if end_digits == at || end_digits == 0 || bytes[end_digits - 1] != b'-' {
            return Err(grammar());
        }
        let dash = end_digits - 1;
        let start_digits = digits_from(dash);
        if start_digits == dash || start_digits == 0 || bytes[start_digits - 1] != b'#' {
            return Err(grammar());
        }
        let number = |digits: &[u8]| {
            canonical_decimal(digits).ok_or_else(|| {
                FoundryError::InvalidArgument(
                    "handle offsets must be decimal u64 without leading zeros".into(),
                )
            })
        };
        let start = number(&bytes[start_digits..dash])?;
        let end = number(&bytes[end_digits..at])?;
        // `#` is ASCII, so the byte index is a char boundary.
        let path = &raw[..start_digits - 1];
        validate_path(path)
            .map_err(|e| FoundryError::InvalidArgument(format!("handle path: {e}")))?;
        if start > end {
            return Err(FoundryError::InvalidArgument(
                "handle end precedes start".into(),
            ));
        }
        Ok(HandleRef {
            path: path.to_owned(),
            start,
            end,
            sha32: raw[at + 1..dot].to_owned(),
            ws16: raw[dot + 1..].to_owned(),
        })
    }
}

impl std::fmt::Display for HandleRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}#{}-{}@{}.{}",
            self.path, self.start, self.end, self.sha32, self.ws16
        )
    }
}

impl From<&SourceHandle> for HandleRef {
    /// Shortens full identities to their prefixes. A malformed short identity
    /// is kept whole, so the rendered handle fails `parse` instead of panicking.
    fn from(handle: &SourceHandle) -> Self {
        HandleRef {
            path: handle.path.clone(),
            start: handle.start,
            end: handle.end,
            sha32: handle.sha256.get(..32).unwrap_or(&handle.sha256).to_owned(),
            ws16: handle
                .workspace_id
                .get(..16)
                .unwrap_or(&handle.workspace_id)
                .to_owned(),
        }
    }
}

impl SourceHandle {
    /// context-v2 rendering of this full-identity handle.
    pub fn to_v2(&self) -> String {
        HandleRef::from(self).to_string()
    }
}

/// Decimal u64 without leading zeros (`0` itself is allowed): the number rule
/// shared by v2 handle offsets and `lines`.
pub(crate) fn canonical_decimal(digits: &[u8]) -> Option<u64> {
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    if digits.len() > 1 && digits[0] == b'0' {
        return None;
    }
    // ASCII digits only, so this is valid UTF-8 and only overflow fails.
    std::str::from_utf8(digits).ok()?.parse().ok()
}

/// context-v2 retrieve `lines`: `"A"` or `"A-B"`, absolute 1-based file lines.
#[derive(Clone, Copy, Debug)]
pub struct LineSelection {
    first: u64,
    last: u64,
}

impl LineSelection {
    /// Syntax only (stage 1): malformed or `0` is `invalid_argument`. `A > B`
    /// parses and is refused as a range in `clip`, after the handle checks.
    pub fn parse(raw: &str) -> FResult<Self> {
        let number = |digits: &str| {
            canonical_decimal(digits.as_bytes())
                .filter(|&n| n > 0)
                .ok_or_else(|| {
                    FoundryError::InvalidArgument(
                        "lines must be \"A\" or \"A-B\": 1-based decimal line numbers without leading zeros"
                            .into(),
                    )
                })
        };
        let (first, last) = raw.split_once('-').unwrap_or((raw, raw));
        Ok(Self {
            first: number(first)?,
            last: number(last)?,
        })
    }

    /// Intersect the selected whole lines with `[start, end)`, never widening it.
    /// Lines are LF-delimited: CR stays with its line, an unterminated last line
    /// ends at EOF, a trailing LF opens no line and an empty source has none;
    /// lines past the end select nothing. `A > B`, an empty selection or an
    /// empty intersection is `invalid_range`, naming the lines the handle
    /// covers. Line edges follow an LF or touch 0/EOF, so the result keeps the
    /// handle's UTF-8 boundaries.
    fn clip(self, body: &[u8], start: usize, end: usize) -> FResult<(usize, usize)> {
        if self.first > self.last {
            return Err(self.selects_nothing(body, start, end));
        }
        let mut offset = 0;
        let mut selected: Option<(usize, usize)> = None;
        for (index, line) in body.split_inclusive(|&b| b == b'\n').enumerate() {
            let number = index as u64 + 1;
            let next = offset + line.len();
            if number >= self.first {
                selected = Some((selected.map_or(offset, |(from, _)| from), next));
            }
            if number >= self.last {
                break;
            }
            offset = next;
        }
        let Some((from, to)) = selected else {
            return Err(self.selects_nothing(body, start, end));
        };
        let (from, to) = (from.max(start), to.min(end));
        if from >= to {
            return Err(self.selects_nothing(body, start, end));
        }
        Ok((from, to))
    }

    /// The `invalid_range` refusal of a selection with nothing in
    /// `[start, end)`: it names the lines that range covers, since a handle's
    /// `#start-end` is easily mistaken for line numbers.
    fn selects_nothing(self, body: &[u8], start: usize, end: usize) -> FoundryError {
        let line_at = |at: usize| body[..at].iter().filter(|&&b| b == b'\n').count() + 1;
        let covers = match (start < end).then(|| (line_at(start), line_at(end - 1))) {
            None => "no lines".to_owned(),
            Some((first, last)) if first == last => format!("line {first}"),
            Some((first, last)) => format!("lines {first}-{last}"),
        };
        let selection = if self.first == self.last {
            self.first.to_string()
        } else {
            format!("{}-{}", self.first, self.last)
        };
        FoundryError::EmptyLineSelection(format!(
            "lines {selection} select nothing in this handle, which covers {covers}; \
             a handle's #start-end is a byte range"
        ))
    }
}

/// One materialized search hit: its delivery unit's handle and verbatim text,
/// the unit's label (`<kind>[ <qualified name>]`), the ranking tier (1 exact
/// definition, 2 lexical) and the best line for its locator.
#[derive(Clone, Debug, Serialize)]
pub struct Hit {
    pub path: String,
    pub start_line: u64,
    pub end_line: u64,
    pub handle: SourceHandle,
    pub text: String,
    pub label: String,
    pub tier: u8,
    pub line: u64,
}

#[derive(Debug, Serialize)]
pub struct SearchOutcome {
    pub workspace_id: String,
    pub source_revision: u64,
    pub hits: Vec<Hit>,
    pub pending_sources: u64,
    pub stale_candidates: u64,
    /// Hits skipped by the per-file cap.
    pub capped: u64,
    pub candidate_limit: usize,
    pub candidate_limit_reached: bool,
    pub truncated: bool,
    pub scan_state: String,
    /// The 009 T002 semantic word (`ready`, `partial`) or `None` on the
    /// baseline path; search locators never claim `whole_unit`.
    pub semantic: Option<String>,
}

/// One rendering of a ranked item (context-v2 § Forms): the exact unit bytes,
/// the unit's signature form, a file's `outline` and `outline-min` forms, or
/// a graph item's single line. Packing takes the first form that fits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderedForm {
    Verbatim(String),
    Signature(String),
    Outline(String),
    OutlineMin(String),
    Line(String),
}

/// Ranking tiers: exact definitions, lexical hits, graph items, file outlines.
pub const TIER_GRAPH: u8 = 3;
pub const TIER_OUTLINE: u8 = 4;
/// Source-bearing compiler-graph units (005 T003): delivery units that enclose
/// an eligible reference of a uniquely resolved seed symbol. They are not
/// graph edge lines, so the multi-root merge counts them with the units.
pub const TIER_COMPILER: u8 = 5;

/// One ranked, already revalidated candidate (context-v2 § Candidate seam).
/// `handle` keeps the full identities of a source item and is `None` for a
/// graph item; `start_line..=end_line` are the lines the handle's range
/// touches and `line` the locator's best line.
#[derive(Clone, Debug)]
pub struct RankedItem {
    pub tier: u8,
    pub rank: usize,
    pub score: f32,
    pub handle: Option<SourceHandle>,
    pub start_line: u64,
    pub end_line: u64,
    pub line: u64,
    pub label: String,
    pub lang: Option<String>,
    pub forms: Vec<RenderedForm>,
    /// 009 T002 neural evidence; `None` for every non-dense candidate.
    pub semantic: Option<SemanticEvidence>,
}

impl RankedItem {
    /// A candidate only the dense window retrieved.
    pub fn is_dense_only(&self) -> bool {
        self.semantic
            .as_ref()
            .is_some_and(|evidence| evidence.dense_only)
    }
}

/// The neural evidence of one dense candidate (009 T002): the whole matched
/// embedding unit, its verbatim bytes, and — when a lexical span retrieved
/// in the same request intersects the unit — that span clipped to the
/// unit's bounds. The packer tries the whole unit, then the span, then a
/// bounded prefix labeled `preview`; the selection is decided by what fits.
#[derive(Clone, Debug)]
pub struct SemanticEvidence {
    /// The matched handle: the whole embedding unit.
    pub matched: SourceHandle,
    /// The unit's verbatim bytes.
    pub unit_body: String,
    /// The intersecting lexical span clipped to the unit, with its own
    /// first line.
    pub span: Option<(SourceHandle, String, u64)>,
    /// The unit's first 1-based line.
    pub unit_start_line: u64,
    /// True when the unit came from the dense window alone (no lexical
    /// delivery unit shares its span): it carries no delivery-unit label
    /// and never seeds graph expansion, since a preview is not a localized
    /// seed.
    pub dense_only: bool,
}

/// What candidate selection dropped or bounded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateCounters {
    /// Candidates whose source changed or vanished since indexing, and graph
    /// items whose row or endpoints changed.
    pub stale: u64,
    /// Hits skipped by the per-file cap.
    pub capped: u64,
    /// A candidate window filled: search tier 1 at 64, tier 2 at 256, or a
    /// context graph examination window at 32 rows.
    pub candidates_full: bool,
    /// More hits survived materialization than the requested limit.
    pub truncated: bool,
    /// `ok`, `graph_unavailable`, `graph_stale` or `graph_invalid` when the
    /// context strategy resolved to graph.
    pub graph: Option<&'static str>,
}

/// The ordered candidates of one store, revalidated in its final read.
#[derive(Clone, Debug)]
pub struct CandidateBatch {
    pub freshness: Freshness,
    pub items: Vec<RankedItem>,
    pub counters: CandidateCounters,
    /// The semantic header word (009 T002): `ready`, `partial`, or
    /// `fallback:<reason>`; `None` keeps the baseline header byte-for-byte.
    pub semantic: Option<String>,
    /// The 013 T003 route word (`policy` or `fallback:<reason>`): present
    /// only when a context's `auto` strategy was routed with a configured
    /// policy; `None` keeps the baseline header byte-for-byte.
    pub route: Option<String>,
}

/// The `outline` and `outline-min` renderings of a retrieve range
/// (context-v2 § Retrieve views).
#[derive(Clone, Debug, Serialize)]
pub struct OutlineOutcome {
    pub requested: SourceHandle,
    pub start_line: u64,
    pub end_line: u64,
    pub lang: &'static str,
    pub outline: String,
    pub outline_min: String,
    pub freshness: Freshness,
}

#[derive(Clone, Debug, Serialize)]
pub struct RetrieveOutcome {
    pub requested: SourceHandle,
    pub requested_tokens: usize,
    /// Full requested span bytes; packers may deliver a fitting prefix.
    pub span: Vec<u8>,
    /// One-based line of `requested.start` in the source, for v2 `L<a>-<b>`.
    pub start_line: u64,
    pub source_bytes: u64,
    pub freshness: Freshness,
}

#[derive(Clone, Debug, Serialize)]
pub struct StoreStatus {
    pub schema: u32,
    pub workspace_id: Option<String>,
    pub source_revision: u64,
    pub source_count: u64,
    pub pending_count: u64,
    pub index_state: String,
    pub index_reason: Option<String>,
    pub scan_state: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RebuildMarker {
    pub owner_id: String,
    /// Basename of the single retained quarantine of the original derived
    /// directory. Recorded as intent before the move.
    pub quarantine: Option<String>,
    /// True once the original derived directory has been quarantined or was
    /// positively absent. From then on `search/` can only be this repair's
    /// replacement, so an unrecognized directory there is a conflict.
    pub original_handled: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct RepairReport {
    pub repaired: bool,
    pub quarantined_to: Option<PathBuf>,
    pub drained_sources: usize,
    /// 008: memory documents rebuilt by the same drain.
    pub drained_memory: usize,
    pub reason: Option<String>,
    /// 009: the semantic F16 generation replayed from the f32 cache by the
    /// same repair (zero document calls); `None` without the feature or a
    /// recorded profile.
    pub semantic_index: Option<crate::neural::index::SemanticIndexReport>,
}
/// The Tantivy schema v2 fields (context-v2 § Search index v2).
struct Fields {
    key: Field,
    key_hash: Field,
    path: Field,
    dir: Field,
    hash: Field,
    start: Field,
    end: Field,
    unit_start: Field,
    unit_head: Field,
    unit_end: Field,
    kind: Field,
    lang: Field,
    name: Field,
    qname: Field,
    def_name: Field,
    ident: Field,
    body: Field,
}

struct SearchHandles {
    reader: IndexReader,
    writer: IndexWriter,
    fields: Fields,
}

pub struct Engine {
    pub(crate) db: Database,
    pub(crate) directory: PathBuf,
    search: Option<SearchHandles>,
    repair_reason: Option<String>,
    workspace: Option<String>,
    workspace_id: Option<String>,
    schema: u32,
    /// The store directory, held as a descriptor opened (`O_DIRECTORY |
    /// O_NOFOLLOW`) exactly once at bind time and kept for the Engine's
    /// life. Semantic purge, publication and validation duplicate it (see
    /// [`crate::neural::anchor`]) and never resolve the store pathname
    /// again, so a substituted ancestor cannot redirect them.
    pub(crate) semantic_dir: crate::neural::anchor::Dir,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("directory", &self.directory)
            .field("schema", &self.schema)
            .field("workspace_id", &self.workspace_id)
            .field("search_available", &self.search.is_some())
            .finish_non_exhaustive()
    }
}

/// Normalized workspace-relative path rules shared by handles, graph
/// endpoints and admitted scan paths. Validation runs on the RAW `/`-separated
/// components before any normalized interpretation, so `a//b`, `a/./b`,
/// `a/b/` and `/a` are rejected instead of being normalized into aliases.
pub fn validate_path(path: &str) -> FResult<()> {
    if path.is_empty() || path.len() > 4096 {
        return Err(FoundryError::InvalidArgument(
            "source path must be 1..4096 UTF-8 bytes".into(),
        ));
    }
    if path.contains(['\0', '\n', '\r']) {
        return Err(FoundryError::InvalidArgument(
            "source path contains forbidden control characters".into(),
        ));
    }
    if path
        .split('/')
        .any(|c| c.is_empty() || c == "." || c == "..")
    {
        return Err(FoundryError::InvalidArgument(
            "source path must be normalized: no absolute prefix and no empty, . or .. components"
                .into(),
        ));
    }
    Ok(())
}

fn chunk_key(path: &str, ordinal: usize) -> String {
    format!("{path}\0{ordinal:010}")
}

/// Decode stored authoritative JSON; a decode failure is corruption, never an
/// invented empty value.
pub(crate) fn decode<T: for<'de> Deserialize<'de>>(raw: &str, what: &str) -> FResult<T> {
    serde_json::from_str(raw)
        .map_err(|e| FoundryError::CorruptStore(format!("{what} record cannot be decoded: {e}")))
}

/// Required schema-2 counter: absent or malformed means authoritative
/// corruption, never an invented zero. Zero defaults belong only in
/// initialization and the explicit upgrade.
pub(crate) fn read_counter<T: ReadableTable<&'static str, &'static str>>(
    meta: &T,
    key: &str,
) -> FResult<u64> {
    let raw = meta
        .get(key)?
        .ok_or_else(|| FoundryError::CorruptStore(format!("{key} metadata missing")))?;
    raw.value().parse::<u64>().map_err(|_| {
        FoundryError::CorruptStore(format!("{key} metadata is not an unsigned integer"))
    })
}

/// Count the pending keys in one typed namespace (008); the sorted table
/// clusters a prefix, so the scan stops at the first key outside it.
pub(crate) fn count_pending_prefix(tx: &redb::ReadTransaction, prefix: &str) -> FResult<u64> {
    let pending = tx.open_table(PENDING)?;
    let mut count = 0u64;
    for row in pending.range::<&str>((Bound::Included(prefix), Bound::Unbounded))? {
        let (key, _) = row?;
        if !key.value().starts_with(prefix) {
            break;
        }
        count += 1;
    }
    Ok(count)
}

/// Required scan state. A persisted `running` means a scan died before it
/// finished; report it conservatively as `incomplete`.
fn read_scan_state<T: ReadableTable<&'static str, &'static str>>(meta: &T) -> FResult<String> {
    let raw = meta
        .get("scan_status")?
        .ok_or_else(|| FoundryError::CorruptStore("scan_status metadata missing".into()))?;
    match raw.value() {
        state @ ("never" | "complete" | "incomplete") => Ok(state.to_owned()),
        "running" => Ok("incomplete".to_owned()),
        other => Err(FoundryError::CorruptStore(format!(
            "scan_status metadata has unknown value {other:?}"
        ))),
    }
}

/// A source reconstructed from its stored chunks in ordinal order and
/// verified (keys, ownership, length, SHA-256) against its metadata.
pub(crate) struct VerifiedSource {
    pub(crate) body: String,
}

/// Reconstruct a source inside the caller's transaction and verify every
/// chunk key/ownership plus the full length and hash before anything is
/// sliced. Any inconsistency is `corrupt_source`, never partial evidence.
pub(crate) fn reconstruct_verified<C: ReadableTable<&'static str, &'static str>>(
    stored: &C,
    path: &str,
    meta: &SourceMeta,
) -> FResult<VerifiedSource> {
    let corrupt = |what: &str| FoundryError::CorruptSource(format!("{path}: {what}"));
    if meta.bytes > MAX_SOURCE_BYTES {
        return Err(corrupt("recorded length exceeds the 2 MiB bound"));
    }
    let first = chunk_key(path, 0);
    let end = chunk_key(path, meta.chunks);
    let mut body = String::with_capacity(meta.bytes);
    let mut count = 0usize;
    for row in stored.range(first.as_str()..end.as_str())? {
        let (key, raw) = row?;
        if key.value() != chunk_key(path, count) {
            return Err(corrupt(
                "chunk keys are not the contiguous ordinal sequence",
            ));
        }
        let chunk: Chunk = serde_json::from_str(raw.value())
            .map_err(|_| corrupt("chunk record cannot be decoded"))?;
        if chunk.path != path || chunk.hash != meta.hash {
            return Err(corrupt("chunk belongs to a different source version"));
        }
        body.push_str(&chunk.body);
        if body.len() > meta.bytes {
            return Err(corrupt("chunks exceed the recorded length"));
        }
        count += 1;
    }
    if count != meta.chunks {
        return Err(corrupt("chunk count differs from the source record"));
    }
    if body.len() != meta.bytes || crate::digest(body.as_bytes()) != meta.hash {
        return Err(corrupt(
            "reconstructed bytes do not match the recorded hash",
        ));
    }
    Ok(VerifiedSource { body })
}

fn chunks(path: &str, hash: &str, content: &str) -> Vec<Chunk> {
    let mut result = Vec::new();
    let mut offset = 0;
    let mut line = 1;
    while offset < content.len() {
        let tail = &content[offset..];
        let mut end = tail.len().min(2048);
        while !tail.is_char_boundary(end) {
            end -= 1;
        }
        if end < tail.len()
            && let Some(nl) = tail[..end].rfind('\n')
        {
            end = nl + 1;
        }
        let body = &tail[..end];
        let newlines = body.bytes().filter(|b| *b == b'\n').count();
        result.push(Chunk {
            path: path.into(),
            hash: hash.into(),
            start_line: line,
            end_line: line + newlines - usize::from(body.ends_with('\n')),
            body: body.into(),
        });
        line += newlines;
        offset += end;
    }
    result
}

fn search_schema() -> Schema {
    let analyzed = |tokenizer: &str, positions: bool| {
        TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(tokenizer)
                .set_index_option(if positions {
                    IndexRecordOption::WithFreqsAndPositions
                } else {
                    IndexRecordOption::WithFreqs
                }),
        )
    };
    let mut schema = Schema::builder();
    schema.add_text_field("key", STRING | STORED);
    schema.add_u64_field("key_hash", FAST | STORED);
    schema.add_text_field("path", STRING | STORED);
    schema.add_text_field("dir", STRING);
    schema.add_text_field("hash", STRING | STORED);
    for field in ["start", "end", "unit_start", "unit_head", "unit_end"] {
        schema.add_u64_field(field, STORED);
    }
    schema.add_text_field("kind", STRING | STORED);
    schema.add_text_field("lang", STRING | STORED);
    schema.add_text_field("name", STORED);
    schema.add_text_field("qname", STORED);
    schema.add_text_field("def_name", analyzed("foundry_lower", false));
    schema.add_text_field("ident", analyzed("foundry_ident", false));
    schema.add_text_field("body", analyzed("foundry_code", true));
    schema.build()
}

fn fields_of(schema: &Schema) -> Fields {
    // Every caller holds a schema equal to `search_schema()` (created from it,
    // or compared with it before use).
    let field = |name: &str| schema.get_field(name).expect("schema v2 field");
    Fields {
        key: field("key"),
        key_hash: field("key_hash"),
        path: field("path"),
        dir: field("dir"),
        hash: field("hash"),
        start: field("start"),
        end: field("end"),
        unit_start: field("unit_start"),
        unit_head: field("unit_head"),
        unit_end: field("unit_end"),
        kind: field("kind"),
        lang: field("lang"),
        name: field("name"),
        qname: field("qname"),
        def_name: field("def_name"),
        ident: field("ident"),
        body: field("body"),
    }
}

/// A Tantivy tokenizer over one `syntax` analysis: lowercased tokens of at
/// least 2 characters at consecutive positions.
#[derive(Clone)]
struct SplitTokenizer(fn(&str) -> Vec<(usize, usize)>);

struct SplitTokens {
    tokens: Vec<Token>,
    next: usize,
}

impl Tokenizer for SplitTokenizer {
    type TokenStream<'a> = SplitTokens;
    fn token_stream<'a>(&'a mut self, text: &'a str) -> SplitTokens {
        SplitTokens {
            tokens: analyzed_tokens(self.0, text),
            next: 0,
        }
    }
}

/// Tantivy reads a token only after `advance` returned true.
impl TokenStream for SplitTokens {
    fn advance(&mut self) -> bool {
        let more = self.next < self.tokens.len();
        self.next += usize::from(more);
        more
    }
    fn token(&self) -> &Token {
        &self.tokens[self.next - 1]
    }
    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.next - 1]
    }
}

fn analyzed_tokens(split: fn(&str) -> Vec<(usize, usize)>, text: &str) -> Vec<Token> {
    split(text)
        .into_iter()
        .filter(|&(from, to)| text[from..to].chars().nth(1).is_some())
        .enumerate()
        .map(|(position, (from, to))| Token {
            offset_from: from,
            offset_to: to,
            position,
            text: text[from..to].to_lowercase(),
            position_length: 1,
        })
        .collect()
}

/// The query-side terms of one analysis, identical to the indexed tokens.
fn analyzed_terms(split: fn(&str) -> Vec<(usize, usize)>, text: &str) -> Vec<String> {
    analyzed_tokens(split, text)
        .into_iter()
        .map(|token| token.text)
        .collect()
}

/// The query's tier-1 runs (context-v2 § Two-tier query): the identifier
/// runs inside its backtick code spans when it has any, otherwise all of
/// them; lowercased, deduplicated, at most [`TIER1_RUNS`] by first
/// appearance. A run of N backticks opens a span that the next run of exactly
/// N backticks closes; an opener without such a closer is literal text.
fn tier1_runs(query: &str) -> Vec<String> {
    let bytes = query.as_bytes();
    let mut ticks: Vec<(usize, usize)> = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        while at < bytes.len() && bytes[at] == b'`' {
            at += 1;
        }
        if at == start {
            at += 1;
        } else {
            ticks.push((start, at));
        }
    }
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut open = 0;
    while open < ticks.len() {
        let (from, to) = ticks[open];
        let closer = ticks[open + 1..]
            .iter()
            .position(|&(start, end)| end - start == to - from);
        match closer {
            Some(offset) => {
                spans.push((to, ticks[open + 1 + offset].0));
                open += offset + 2;
            }
            None => open += 1,
        }
    }
    let all = crate::syntax::identifier_runs(query);
    let marked: Vec<(usize, usize)> = all
        .iter()
        .copied()
        .filter(|&(from, to)| spans.iter().any(|&(start, end)| start <= from && to <= end))
        .collect();
    let chosen = if marked.is_empty() { all } else { marked };
    let mut runs: Vec<String> = Vec::new();
    for (from, to) in chosen {
        let run = query[from..to].to_lowercase();
        if runs.contains(&run) {
            continue;
        }
        if runs.len() == TIER1_RUNS {
            break;
        }
        runs.push(run);
    }
    runs
}

/// Registers the schema v2 tokenizers (they are per index instance, never
/// persisted) and opens the reader and the single writer.
fn search_handles(index: Index) -> tantivy::Result<SearchHandles> {
    let tokenizers = index.tokenizers();
    tokenizers.register(
        "foundry_lower",
        TextAnalyzer::builder(RawTokenizer::default())
            .filter(LowerCaser)
            .build(),
    );
    tokenizers.register(
        "foundry_ident",
        SplitTokenizer(crate::syntax::identifier_runs),
    );
    tokenizers.register(
        "foundry_code",
        SplitTokenizer(crate::syntax::code_subtokens),
    );
    let fields = fields_of(&index.schema());
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()?;
    let writer = index.writer_with_num_threads(1, 20_000_000)?;
    Ok(SearchHandles {
        reader,
        writer,
        fields,
    })
}

fn open_search(dir: &Path) -> Result<SearchHandles, String> {
    let index_dir = dir.join("search");
    if !index_dir.join("meta.json").is_file() {
        return Err("derived index is missing".into());
    }
    if index_dir.is_symlink() {
        return Err("derived index path is a symlink".into());
    }
    let index = Index::open_in_dir(&index_dir).map_err(|e| format!("derived index open: {e}"))?;
    // The actual Tantivy field set must be schema v2.
    if index.schema() != search_schema() {
        return Err(SEARCH_SCHEMA_REASON.into());
    }
    search_handles(index).map_err(|e| format!("derived index open: {e}"))
}

/// The schema v2 documents of one verified source (context-v2 § Search
/// documents): one per syntax document, carrying its delivery unit; a
/// document delivered as a named programming-language unit (not a Markdown
/// section or block) also carries that unit's `def_name`.
fn search_documents(fields: &Fields, path: &str, hash: &str, body: &str) -> Vec<TantivyDocument> {
    use crate::syntax::UnitKind;
    let lang = crate::syntax::Lang::from_path(path);
    let mut dirs: Vec<&str> = path.match_indices('/').map(|(i, _)| &path[..i]).collect();
    dirs.push(path);
    crate::syntax::documents(body, lang)
        .into_iter()
        .map(|document| {
            let unit = &document.unit;
            let key = format!("{path}\0{}", document.start);
            let text = &body[document.start..document.end];
            let mut out = TantivyDocument::default();
            out.add_u64(fields.key_hash, key_hash(&key));
            out.add_text(fields.key, &key);
            out.add_text(fields.path, path);
            for dir in &dirs {
                out.add_text(fields.dir, dir);
            }
            out.add_text(fields.hash, hash);
            out.add_u64(fields.start, document.start as u64);
            out.add_u64(fields.end, document.end as u64);
            out.add_u64(fields.unit_start, unit.start as u64);
            out.add_u64(fields.unit_head, unit.head as u64);
            out.add_u64(fields.unit_end, unit.end as u64);
            out.add_text(fields.kind, unit.kind.as_str());
            if let Some(lang) = lang {
                out.add_text(fields.lang, lang.tag());
            }
            if let Some(name) = &unit.name {
                out.add_text(fields.name, name);
                if !matches!(unit.kind, UnitKind::Section | UnitKind::Block) {
                    out.add_text(fields.def_name, name);
                }
            }
            if let Some(qname) = &unit.qname {
                out.add_text(fields.qname, qname);
            }
            out.add_text(fields.ident, text);
            out.add_text(fields.body, text);
            out
        })
        .collect()
}

/// Validate one memory source link against the write transaction's own
/// authoritative tables, with the same codes and checks as retrieve:
/// workspace, existence, digest, then a full chunk reconstruct/verify (length
/// and SHA-256) followed by range and UTF-8 boundary checks.
pub(crate) fn validate_link_span(tx: &WriteTransaction, bound: &str, handle: &str) -> FResult<()> {
    let parsed = HandleRef::parse(handle)?;
    if !bound.starts_with(&parsed.ws16) {
        return Err(FoundryError::WrongWorkspace);
    }
    let sources = tx.open_table(SOURCES)?;
    let stored = tx.open_table(CHUNKS)?;
    let raw = sources
        .get(parsed.path.as_str())?
        .ok_or(FoundryError::NotFound)?;
    let meta: SourceMeta = decode(raw.value(), "source")?;
    if !meta.hash.starts_with(&parsed.sha32) {
        return Err(FoundryError::StaleHandle);
    }
    let verified = reconstruct_verified(&stored, &parsed.path, &meta)?;
    let body = verified.body;
    let len = body.len() as u64;
    let valid_empty = parsed.start == 0 && parsed.end == 0 && len == 0;
    let valid_span = parsed.start < parsed.end
        && parsed.end <= len
        && body.is_char_boundary(parsed.start as usize)
        && body.is_char_boundary(parsed.end as usize);
    if !valid_empty && !valid_span {
        return Err(FoundryError::InvalidRange);
    }
    Ok(())
}

/// The derived search document of one memory record (008): `kind:"memory"`,
/// key `memory:<id>`, `hash` its revision in decimal. `path` is empty — no
/// valid source path is empty, so a source delete by `path` can never match
/// it — and memory documents carry no `def_name`, so definition matching
/// stays source-only even before the kind filter excludes them.
fn memory_document(fields: &Fields, id: &str, revision: u64, text: &str) -> TantivyDocument {
    let key = memory_pending_key(id);
    let mut out = TantivyDocument::default();
    out.add_u64(fields.key_hash, key_hash(&key));
    out.add_text(fields.key, &key);
    out.add_text(fields.path, "");
    out.add_text(fields.hash, revision.to_string());
    for field in [
        fields.start,
        fields.end,
        fields.unit_start,
        fields.unit_head,
        fields.unit_end,
    ] {
        out.add_u64(field, 0);
    }
    out.add_text(fields.kind, "memory");
    out.add_text(fields.name, id);
    out.add_text(fields.ident, text);
    out.add_text(fields.body, text);
    out
}
/// The first 8 bytes of SHA-256 of `key`, big-endian: the deterministic
/// cutoff tie-breaker of both search tiers.
fn key_hash(key: &str) -> u64 {
    let hex = crate::digest(key.as_bytes());
    // SHA-256 hex is 64 lowercase hex digits, so the first 16 always parse.
    u64::from_str_radix(&hex[..16], 16).unwrap_or(0)
}

/// The `path` search input: one leading `./` and one trailing `/` stripped;
/// the rest must satisfy the handle path rules.
pub(crate) fn path_filter(raw: &str) -> FResult<String> {
    let trimmed = raw.strip_prefix("./").unwrap_or(raw);
    let trimmed = trimmed.strip_suffix('/').unwrap_or(trimmed);
    validate_path(trimmed)
        .map_err(|e| FoundryError::InvalidArgument(format!("path filter: {e}")))?;
    Ok(trimmed.to_owned())
}

/// Whether `path` lies under a normalized [`path_filter`]: the file itself
/// or a component-boundary subtree (`src/a` admits `src/a/x.rs`, never
/// `src/ab.rs`) — the predicate the lexical tiers apply through `dir` terms.
#[cfg(feature = "semantic")]
fn within_filter(path: &str, filter: &str) -> bool {
    path.strip_prefix(filter)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// The 0-based index of the line of `text` holding the most distinct query
/// subtokens, the earliest on ties.
fn best_line_index(text: &str, wanted: &std::collections::BTreeSet<String>) -> u64 {
    let mut best = (0usize, 0u64);
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let distinct: std::collections::BTreeSet<String> =
            analyzed_terms(crate::syntax::code_subtokens, line)
                .into_iter()
                .collect();
        let score = distinct.intersection(wanted).count();
        if score > best.0 {
            best = (score, index as u64);
        }
    }
    best.1
}

/// One search document selected by a tier, before revalidation.
struct Candidate {
    tier: u8,
    score: Score,
    path: String,
    hash: String,
    start: u64,
    unit_start: u64,
    /// The delivery unit's own start, after its leading run: a tier-1 hit's
    /// best line.
    unit_head: u64,
    unit_end: u64,
    kind: String,
    lang: Option<String>,
    qname: Option<String>,
}

impl Candidate {
    /// The delivery unit's identity within this store.
    fn unit(&self) -> (String, u64, u64) {
        (self.path.clone(), self.unit_start, self.unit_end)
    }
}

/// Write schema last in the initializing transaction.
fn initialize_tables(tx: &WriteTransaction) -> FResult<()> {
    tx.open_table(SOURCES)?;
    tx.open_table(CHUNKS)?;
    tx.open_table(PENDING)?;
    tx.open_table(MEMORY)?;
    tx.open_table(FEEDBACK)?;
    tx.open_table(SEEN)?;
    crate::graph::init(tx)?;
    crate::graph::init_compiler(tx)?;
    crate::neural::cache::init_tables(tx)?;
    crate::learning::init_tables(tx)?;
    Ok(())
}

impl Engine {
    /// Explicit initializer: accepts a missing or empty directory, refuses a
    /// nonempty directory without a recognized store, binds the workspace
    /// root and creates the empty derived index.
    pub fn initialize(store_dir: &Path, workspace_root: &Path) -> FResult<Self> {
        let db_path = store_dir.join("knowledge.redb");
        // Validate arguments/root before initializing anything: invalid index
        // arguments — or a root that cannot be held as a descriptor — must not
        // create a store at all.
        let root = workspace_root
            .canonicalize()
            .map_err(|e| FoundryError::InvalidArgument(format!("workspace root: {e}")))?;
        if !root.is_dir() {
            return Err(FoundryError::InvalidArgument(
                "workspace root must be a directory".into(),
            ));
        }
        // A not-yet-existing store directory cannot equal an existing root.
        if store_dir
            .canonicalize()
            .is_ok_and(|existing_store| existing_store == root)
        {
            return Err(FoundryError::InvalidArgument(
                "store directory cannot be the workspace root".into(),
            ));
        }
        crate::ingest::ensure_holdable_root(&root)?;
        if store_dir.exists() {
            let nonempty = std::fs::read_dir(store_dir)
                .map_err(FoundryError::from)?
                .any(|_| true);
            if nonempty && !db_path.exists() {
                return Err(FoundryError::UnrecognizedStore(
                    store_dir.display().to_string(),
                ));
            }
        } else {
            std::fs::create_dir_all(store_dir)?;
        }
        let db = Database::create(&db_path)?;
        let tx = db.begin_write()?;
        {
            let mut meta = tx.open_table(META)?;
            if let Some(version) = meta.get("schema")? {
                return Err(FoundryError::InvalidArgument(format!(
                    "store already initialized with schema {}",
                    version.value()
                )));
            }
            initialize_tables(&tx)?;
            meta.insert("schema", SCHEMA_VERSION.to_string().as_str())?;
            meta.insert("source_revision", "0")?;
            meta.insert("scan_id", "0")?;
            meta.insert("scan_status", "never")?;
            meta.insert("memory_revision", "0")?;
            meta.insert("search_schema", SEARCH_SCHEMA)?;
            let root_str = root.to_str().ok_or_else(|| {
                FoundryError::InvalidArgument("workspace path is not UTF-8".into())
            })?;
            meta.insert("workspace", root_str)?;
            meta.insert("workspace_id", crate::digest(root_str.as_bytes()).as_str())?;
        }
        tx.commit()?;
        std::fs::create_dir_all(store_dir.join("search"))?;
        let index = Index::create_in_dir(store_dir.join("search"), search_schema())?;
        let handles = search_handles(index)?;
        // The canonical store path, computed ONCE at bind time; the
        // descriptor below and the stored display path both come from it and
        // neither is ever resolved again for semantic work.
        let canonical = store_dir.canonicalize()?;
        // Bind the store-directory descriptor once, after the database (and
        // its lock) is held: every later semantic operation works through a
        // duplicate of it, never through the pathname.
        let semantic_dir = crate::neural::anchor::Dir::open_path(&canonical)
            .map_err(|e| crate::neural::anchor::conflict_or_io(e, "store directory"))?;
        let (workspace, workspace_id) = Self::read_binding(&db)?;
        Ok(Self {
            db,
            directory: canonical,
            search: Some(handles),
            repair_reason: None,
            workspace,
            workspace_id,
            schema: SCHEMA_VERSION,
            semantic_dir,
        })
    }

    fn read_binding(db: &Database) -> FResult<(Option<String>, Option<String>)> {
        let tx = db.begin_read()?;
        let meta = tx.open_table(META)?;
        Ok((
            meta.get("workspace")?.map(|v| v.value().to_owned()),
            meta.get("workspace_id")?.map(|v| v.value().to_owned()),
        ))
    }

    /// Open existing state only. Never creates files, tables or directories,
    /// never upgrades schema. Derived-index trouble degrades search to
    /// `repair_required`; authoritative reads stay available.
    pub fn open_existing(store_dir: &Path) -> FResult<Self> {
        Self::open_with(store_dir, true)
    }

    /// Authoritative-only open used by repair: no Tantivy construction first.
    pub(crate) fn open_authoritative(store_dir: &Path) -> FResult<Self> {
        Self::open_with(store_dir, false)
    }

    fn open_with(store_dir: &Path, construct_search: bool) -> FResult<Self> {
        let db_path = store_dir.join("knowledge.redb");
        if !db_path.is_file() {
            return Err(FoundryError::StoreNotFound);
        }
        let db = Database::open(&db_path)?;
        let schema = {
            let tx = db.begin_read()?;
            let meta = tx
                .open_table(META)
                .map_err(|e| FoundryError::CorruptStore(format!("meta table unreadable: {e}")))?;
            let Some(version) = meta.get("schema")? else {
                return Err(FoundryError::UnrecognizedStore(
                    "store has no schema marker".into(),
                ));
            };
            match version.value() {
                "1" | "2" | "3" | "4" | "5" => {
                    return Err(FoundryError::UpgradeRequired {
                        found: version.value().to_owned(),
                    });
                }
                "6" => 6u32,
                other => {
                    return Err(FoundryError::UnsupportedSchema {
                        found: other.to_owned(),
                    });
                }
            }
        };
        // Confirm the schema-6 tables exist; missing authoritative tables in
        // a schema-6 store are corruption, not something an open recreates.
        {
            let tx = db.begin_read()?;
            tx.open_table(SEEN).map_err(|e| {
                FoundryError::CorruptStore(format!("scan_seen table unreadable: {e}"))
            })?;
            tx.open_table(SOURCES)
                .map_err(|e| FoundryError::CorruptStore(format!("sources table: {e}")))?;
            tx.open_table(CHUNKS)
                .map_err(|e| FoundryError::CorruptStore(format!("chunks table: {e}")))?;
            tx.open_table(PENDING)
                .map_err(|e| FoundryError::CorruptStore(format!("pending table: {e}")))?;
            tx.open_table(MEMORY)
                .map_err(|e| FoundryError::CorruptStore(format!("memory table: {e}")))?;
            crate::graph::check_compiler_tables(&tx)?;
            crate::neural::cache::check_tables(&db)?;
            crate::learning::check_tables(&db)?;
        }
        let (workspace, workspace_id) = Self::read_binding(&db)?;
        let marker = Self::read_marker(&db)?;
        let mut repair_reason = marker
            .is_some()
            .then(|| "search_rebuild_required marker is set".to_owned());
        let mut search = None;
        if marker.is_none() && construct_search {
            // Index version gate: anything but the current format, recorded
            // or actual, needs an explicit repair; an open never rebuilds.
            let recorded = {
                let tx = db.begin_read()?;
                let meta = tx.open_table(META)?;
                meta.get("search_schema")?
                    .map(|v| v.value() == SEARCH_SCHEMA)
            };
            if recorded != Some(true) {
                repair_reason = Some(SEARCH_SCHEMA_REASON.to_owned());
            } else {
                match open_search(store_dir) {
                    Ok(handles) => search = Some(handles),
                    Err(reason) => repair_reason = Some(reason),
                }
            }
        }
        // Bind the store-directory descriptor once, after the database (and
        // its lock) is held. The path is canonicalized here — ONCE, at bind
        // time — and never resolved again: every later semantic operation
        // works through a duplicate of this descriptor, so an ancestor
        // renamed or substituted after the bind cannot redirect it.
        let canonical = store_dir.canonicalize()?;
        let semantic_dir = crate::neural::anchor::Dir::open_path(&canonical)
            .map_err(|e| crate::neural::anchor::conflict_or_io(e, "store directory"))?;
        Ok(Self {
            db,
            directory: canonical,
            search,
            repair_reason,
            workspace,
            workspace_id,
            schema,
            semantic_dir,
        })
    }

    fn read_marker(db: &Database) -> FResult<Option<RebuildMarker>> {
        let tx = db.begin_read()?;
        let meta = tx.open_table(META)?;
        match meta.get("search_rebuild_required")? {
            None => Ok(None),
            Some(raw) => decode::<RebuildMarker>(raw.value(), "rebuild marker")
                .map(Some)
                .map_err(|_| FoundryError::CorruptStore("rebuild marker cannot be decoded".into())),
        }
    }

    /// Explicit v1..=v5 -> v6 transaction under exclusive ownership. A
    /// v1 store first receives the v2 steps, a v1|v2 store the v3 steps, a
    /// v1|v2|v3 store the v4 steps, a v1..=v4 store the v5 steps, then every
    /// store the v6 steps; all run in ONE write transaction. The v3 steps create the memory table and
    /// its never-reset revision counter and migrate every pending key to the
    /// typed form (`source:<path>`); the v4 steps (005) create the empty
    /// compiler-fact tables; the v5 steps (009) create the empty semantic
    /// tables (partitions, vector cache, state); the v6 steps (013) create
    /// the empty learning tables (v4 feedback rows, group history, published
    /// dataset lineage). `schema
    /// = "6"` is published last. Every earlier table, legacy feedback
    /// included, is preserved, so an interrupted upgrade leaves the store
    /// wholly old or wholly v6. Only the current version is
    /// a target; this is a clean cutover.
    pub fn upgrade_store(store_dir: &Path, to: u32, control: &crate::Control) -> FResult<()> {
        if to != SCHEMA_VERSION {
            return Err(FoundryError::UnsupportedMode(format!(
                "only upgrade-store --to {SCHEMA_VERSION} is supported"
            )));
        }
        control.check()?;
        let db_path = store_dir.join("knowledge.redb");
        if !db_path.is_file() {
            return Err(FoundryError::StoreNotFound);
        }
        let db = Database::open(&db_path)?;
        let from = {
            let tx = db.begin_read()?;
            let meta = tx.open_table(META)?;
            match meta.get("schema")? {
                None => {
                    return Err(FoundryError::UnrecognizedStore(
                        "store has no schema marker".into(),
                    ));
                }
                Some(v) if v.value() == "6" => return Ok(()), // already upgraded
                Some(v) if v.value() == "1" => 1u32,
                Some(v) if v.value() == "2" => 2u32,
                Some(v) if v.value() == "3" => 3u32,
                Some(v) if v.value() == "4" => 4u32,
                Some(v) if v.value() == "5" => 5u32,
                Some(v) => {
                    return Err(FoundryError::UnsupportedSchema {
                        found: v.value().to_owned(),
                    });
                }
            }
        };
        control.check()?;
        let tx = db.begin_write()?;
        {
            tx.open_table(SEEN)?;
            let mut meta = tx.open_table(META)?;
            if from == 1 {
                // The v2 steps, unchanged: initialize revision/scan metadata
                // and derive the bound workspace identity.
                meta.insert("source_revision", "0")?;
                meta.insert("scan_id", "0")?;
                meta.insert("scan_status", "never")?;
                let bound_root = meta.get("workspace")?.map(|v| v.value().to_owned());
                if let Some(root) = bound_root {
                    let id = crate::digest(root.as_bytes());
                    meta.insert("workspace_id", id.as_str())?;
                }
            }
            if from < 3 {
                // The v3 steps: the memory table (empty; records arrive only
                // by explicit puts) and its revision counter, which starts
                // at 0 and is never reset, even when the table is empty
                // again. A v3 store keeps both untouched.
                tx.open_table(MEMORY)?;
                meta.insert("memory_revision", "0")?;
                // Typed pending keys: every pre-v3 key is a raw source path
                // (v3 writers have always typed theirs), so prefix them all.
                // A path may itself contain `:` — `source:memory:x` stays
                // distinct from the memory key `memory:x`.
                let mut pending = tx.open_table(PENDING)?;
                let rows: Vec<(String, String)> = pending
                    .iter()?
                    .map(|row| {
                        let (k, v) = row?;
                        Ok((k.value().to_owned(), v.value().to_owned()))
                    })
                    .collect::<Result<Vec<_>, redb::StorageError>>()
                    .map_err(FoundryError::from)?;
                // Remove EVERY original key before inserting any typed key:
                // interleaving would overwrite a raw `source:a` row while
                // migrating `a`, losing that path's pending work.
                for (key, _) in &rows {
                    pending.remove(key.as_str())?;
                }
                for (key, value) in rows {
                    pending.insert(source_pending_key(&key).as_str(), value.as_str())?;
                }
            }
            // The v4 steps (005): the empty compiler-fact tables. Facts
            // arrive only by an explicit `import-scip`.
            if from < 4 {
                crate::graph::init_compiler(&tx)?;
            }
            // The v5 steps (009): the empty semantic tables. Rows arrive
            // only by an explicit `semantic prepare`.
            if from < 5 {
                crate::neural::cache::init_tables(&tx)?;
            }
            // The v6 steps (013): the empty learning tables. Rows arrive
            // only by explicit operator `feedback v4` input and `learning
            // prepare`.
            if from < 6 {
                crate::learning::init_tables(&tx)?;
            }
            // Publish the schema last inside the same transaction.
            meta.insert("schema", SCHEMA_VERSION.to_string().as_str())?;
        }
        // The upgrade transaction is live and fully written but uncommitted:
        // an exit here must leave the store wholly v1/v2/v3/v4.
        fault!(UPGRADE_BEFORE_COMMIT, None, Some(control), "")?;
        control.check()?;
        tx.commit()?;
        fault!(UPGRADE_AFTER_COMMIT, None, Some(control), "")?;
        Ok(())
    }

    pub fn status(&self) -> FResult<StoreStatus> {
        let (revision, sources, pending, scan_state) = {
            let tx = self.db.begin_read()?;
            let meta = tx.open_table(META)?;
            let revision = read_counter(&meta, "source_revision")?;
            let sources = tx.open_table(SOURCES)?.len()?;
            let pending = tx.open_table(PENDING)?.len()?;
            let scan_state = read_scan_state(&meta)?;
            (revision, sources, pending, scan_state)
        };
        let (index_state, index_reason) = if self.schema != SCHEMA_VERSION {
            ("unsupported".to_owned(), None)
        } else if let Some(reason) = &self.repair_reason {
            ("repair_required".to_owned(), Some(reason.clone()))
        } else if self.search.is_some() {
            if pending == 0 {
                ("ready".to_owned(), None)
            } else {
                ("lagging".to_owned(), None)
            }
        } else {
            (
                "repair_required".to_owned(),
                Some("derived index unavailable".into()),
            )
        };
        Ok(StoreStatus {
            schema: self.schema,
            workspace_id: self.workspace_id.clone(),
            source_revision: revision,
            source_count: sources,
            pending_count: pending,
            index_state,
            index_reason,
            scan_state,
        })
    }

    pub fn workspace_id(&self) -> Option<String> {
        self.workspace_id.clone()
    }

    /// The bound workspace root path, if a root is bound (013 uses it to
    /// keep learning output out of the admitted source root).
    pub fn workspace_root(&self) -> Option<&str> {
        self.workspace.as_deref()
    }

    fn require_workspace_id(&self) -> FResult<String> {
        self.workspace_id
            .clone()
            .ok_or(FoundryError::WorkspaceUnbound)
    }

    fn require_search(&self) -> FResult<&SearchHandles> {
        if let Some(reason) = &self.repair_reason {
            return Err(FoundryError::RepairRequired(reason.clone()));
        }
        self.search
            .as_ref()
            .ok_or_else(|| FoundryError::RepairRequired("derived index unavailable".into()))
    }

    /// Verify a scan root against the bound workspace before any mutation.
    pub(crate) fn verify_binding(&self, root: &Path) -> FResult<PathBuf> {
        let canonical = root
            .canonicalize()
            .map_err(|e| FoundryError::InvalidArgument(format!("workspace root: {e}")))?;
        let canonical_str = canonical
            .to_str()
            .ok_or_else(|| FoundryError::InvalidArgument("workspace path is not UTF-8".into()))?;
        if let Some(existing) = &self.workspace
            && existing != canonical_str
        {
            return Err(FoundryError::WrongWorkspace);
        }
        if canonical == self.directory {
            return Err(FoundryError::InvalidArgument(
                "cannot index the store directory".into(),
            ));
        }
        Ok(canonical)
    }

    /// Bind (or confirm) the workspace and update this live engine's cached
    /// identity, so a long-lived owner of an upgraded unbound store can
    /// search/retrieve right after its first explicit index.
    pub(crate) fn bind_workspace(&mut self, root: &Path) -> FResult<()> {
        let canonical = root
            .canonicalize()
            .map_err(|e| FoundryError::InvalidArgument(format!("workspace root: {e}")))?;
        let root_str = canonical
            .to_str()
            .ok_or_else(|| FoundryError::InvalidArgument("workspace path is not UTF-8".into()))?;
        let id = crate::digest(root_str.as_bytes());
        let tx = self.db.begin_write()?;
        {
            let mut meta = tx.open_table(META)?;
            if let Some(existing) = meta.get("workspace")? {
                if existing.value() != root_str {
                    return Err(FoundryError::WrongWorkspace);
                }
            } else {
                meta.insert("workspace", root_str)?;
                meta.insert("workspace_id", id.as_str())?;
            }
        }
        tx.commit()?;
        self.workspace = Some(root_str.to_owned());
        self.workspace_id = Some(id);
        Ok(())
    }

    pub fn source(&self, path: &str) -> FResult<Option<SourceMeta>> {
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        match sources.get(path)? {
            None => Ok(None),
            Some(v) => decode::<SourceMeta>(v.value(), "source").map(Some),
        }
    }

    /// Begin a scan: checked scan-id increment and `running` status, before
    /// any tree mutation.
    pub(crate) fn begin_scan(&self) -> FResult<u64> {
        let tx = self.db.begin_write()?;
        let scan_id = {
            let mut meta = tx.open_table(META)?;
            // Required state is validated, never silently overwritten.
            read_scan_state(&meta)?;
            let current = read_counter(&meta, "scan_id")?;
            let next = current
                .checked_add(1)
                .ok_or(FoundryError::ScanIdExhausted)?;
            meta.insert("scan_id", next.to_string().as_str())?;
            meta.insert("scan_status", "running")?;
            next
        };
        tx.commit()?;
        Ok(scan_id)
    }

    pub(crate) fn finish_scan(&self, complete: bool) -> FResult<()> {
        let tx = self.db.begin_write()?;
        {
            let mut meta = tx.open_table(META)?;
            meta.insert(
                "scan_status",
                if complete { "complete" } else { "incomplete" },
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Record a bounded page of encountered paths as seen in `scan_id` with one
    /// commit. Callers flush before the sweep, so the sweep reads a complete set.
    pub(crate) fn mark_seen_batch(&self, paths: &[String], scan_id: u64) -> FResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        fault!(SEEN_FLUSH, Some(self), None, &paths.len().to_string())?;
        let id = scan_id.to_string();
        let tx = self.db.begin_write()?;
        {
            let mut seen = tx.open_table(SEEN)?;
            for path in paths {
                seen.insert(path.as_str(), id.as_str())?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Paged sweep of source rows not seen in `scan_id`. Each page commits
    /// atomically; an interruption retires only already-committed pages.
    pub(crate) fn sweep_unseen(
        &self,
        scan_id: u64,
        control: &crate::Control,
    ) -> FResult<(u64, bool)> {
        let mut deleted = 0u64;
        let mut after: Option<String> = None;
        loop {
            if control.check().is_err() {
                return Ok((deleted, true));
            }
            let page: Vec<String> = {
                let tx = self.db.begin_read()?;
                let sources = tx.open_table(SOURCES)?;
                let rows = match &after {
                    None => sources.range::<&str>(..)?,
                    Some(key) => {
                        sources.range::<&str>((Bound::Excluded(key.as_str()), Bound::Unbounded))?
                    }
                };
                rows.take(PAGE)
                    .map(|row| row.map(|(k, _)| k.value().to_owned()))
                    .collect::<Result<Vec<String>, redb::StorageError>>()
                    .map_err(FoundryError::from)?
            };
            if page.is_empty() {
                return Ok((deleted, false));
            }
            // A cancel armed here lets this page commit and stops before the next.
            fault!(
                SWEEP_PAGE,
                Some(self),
                Some(control),
                &page.len().to_string()
            )?;
            let mut retired = 0u64;
            let tx = self.db.begin_write()?;
            {
                let mut sources = tx.open_table(SOURCES)?;
                let mut stored = tx.open_table(CHUNKS)?;
                let mut pending = tx.open_table(PENDING)?;
                let mut seen = tx.open_table(SEEN)?;
                let mut meta = tx.open_table(META)?;
                for path in &page {
                    let seen_now = seen
                        .get(path.as_str())?
                        .and_then(|v| v.value().parse::<u64>().ok());
                    if seen_now != Some(scan_id)
                        && let Some(old) = sources.remove(path.as_str())?
                    {
                        let old: SourceMeta = decode(old.value(), "source")?;
                        for i in 0..old.chunks {
                            stored.remove(chunk_key(path, i).as_str())?;
                        }
                        pending.insert(source_pending_key(path).as_str(), "deleted")?;
                        seen.remove(path.as_str())?;
                        retired += 1;
                    }
                }
                if retired > 0 {
                    let revision = read_counter(&meta, "source_revision")?;
                    let next = revision
                        .checked_add(retired)
                        .ok_or(FoundryError::RevisionExhausted)?;
                    meta.insert("source_revision", next.to_string().as_str())?;
                }
            }
            tx.commit()?;
            deleted += retired;
            after = page.last().cloned();
        }
    }

    /// All authoritative source/chunk changes, pending index work and the
    /// source revision commit together. Acknowledgement follows commit.
    pub fn replace_source(&self, path: &str, content: &str) -> FResult<bool> {
        validate_path(path)?;
        if content.len() > MAX_SOURCE_BYTES {
            return Err(FoundryError::InvalidArgument(
                "source exceeds 2 MiB limit".into(),
            ));
        }
        let hash = crate::digest(content.as_bytes());
        let pieces = chunks(path, &hash, content);
        let tx = self.db.begin_write()?;
        {
            let mut sources = tx.open_table(SOURCES)?;
            let old = match sources.get(path)? {
                Some(v) => Some(decode::<SourceMeta>(v.value(), "source")?),
                None => None,
            };
            if old.as_ref().is_some_and(|old| old.hash == hash) {
                return Ok(false);
            }
            let mut stored = tx.open_table(CHUNKS)?;
            if let Some(old) = &old {
                for i in 0..old.chunks {
                    stored.remove(chunk_key(path, i).as_str())?;
                }
            }
            for (i, chunk) in pieces.iter().enumerate() {
                stored.insert(
                    chunk_key(path, i).as_str(),
                    serde_json::to_string(chunk)?.as_str(),
                )?;
            }
            sources.insert(
                path,
                serde_json::to_string(&SourceMeta {
                    hash: hash.clone(),
                    chunks: pieces.len(),
                    bytes: content.len(),
                    lines: content.lines().count(),
                })?
                .as_str(),
            )?;
            tx.open_table(PENDING)?
                .insert(source_pending_key(path).as_str(), hash.as_str())?;
            bump_revision(&tx, 1)?;
        }
        // A failure here drops the transaction: prior source bytes stay intact.
        fault!(SOURCE_BEFORE_COMMIT, Some(self), None, path)?;
        tx.commit()?;
        fault!(SOURCE_AFTER_COMMIT, Some(self), None, path)?;
        Ok(true)
    }

    pub fn delete_source(&self, path: &str) -> FResult<bool> {
        validate_path(path)?;
        let tx = self.db.begin_write()?;
        {
            let mut sources = tx.open_table(SOURCES)?;
            let old = match sources.remove(path)? {
                Some(v) => Some(decode::<SourceMeta>(v.value(), "source")?),
                None => return Ok(false),
            };
            let Some(old) = old else {
                return Ok(false);
            };
            let mut stored = tx.open_table(CHUNKS)?;
            for i in 0..old.chunks {
                stored.remove(chunk_key(path, i).as_str())?;
            }
            tx.open_table(PENDING)?
                .insert(source_pending_key(path).as_str(), "deleted")?;
            bump_revision(&tx, 1)?;
        }
        fault!(SOURCE_BEFORE_COMMIT, Some(self), None, path)?;
        tx.commit()?;
        fault!(SOURCE_AFTER_COMMIT, Some(self), None, path)?;
        Ok(true)
    }

    /// Pending `source:` work only (008): scan and multi-root readers report
    /// their own namespace; readiness and drains keep the total.
    pub fn pending_source_work(&self) -> FResult<u64> {
        let tx = self.db.begin_read()?;
        count_pending_prefix(&tx, "source:")
    }

    pub fn pending(&self) -> FResult<u64> {
        Ok(self.db.begin_read()?.open_table(PENDING)?.len()?)
    }

    pub fn source_revision(&self) -> FResult<u64> {
        let tx = self.db.begin_read()?;
        read_counter(&tx.open_table(META)?, "source_revision")
    }

    /// Best-effort: apply the one `memory:<id>` pending key to the derived
    /// index right after its mutation committed, through the same document
    /// and clear-if-unchanged rules as the normal drain. Any failure (broken
    /// or missing index, I/O) leaves the key pending and is not reported:
    /// the authoritative mutation already succeeded and replay converges.
    pub(crate) fn drain_memory_key(&mut self, id: &str) {
        let _ = self.try_drain_memory_key(id);
    }

    fn try_drain_memory_key(&mut self, id: &str) -> FResult<()> {
        if self.repair_reason.is_some() {
            return Ok(());
        }
        let key = memory_pending_key(id);
        let (value, record) = {
            let tx = self.db.begin_read()?;
            let Some(value) = tx
                .open_table(PENDING)?
                .get(key.as_str())?
                .map(|v| v.value().to_owned())
            else {
                return Ok(());
            };
            let record = match tx.open_table(MEMORY)?.get(id)? {
                Some(raw) => serde_json::from_str::<MemoryRecord>(raw.value()).ok(),
                None => None,
            };
            (value, record)
        };
        let Some(handles) = self.search.as_mut() else {
            return Ok(());
        };
        handles
            .writer
            .delete_term(Term::from_field_text(handles.fields.key, &key));
        if let Some(record) = &record {
            handles.writer.add_document(memory_document(
                &handles.fields,
                &record.id,
                record.revision,
                &record.text,
            ))?;
        }
        handles.writer.commit()?;
        handles.reader.reload()?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(PENDING)?;
            let unchanged = table.get(key.as_str())?.is_some_and(|v| v.value() == value);
            if unchanged {
                table.remove(key.as_str())?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// One index batch: at most `PAGE` pending keys. Search commit precedes
    /// clearing durable pending work; only the indexed version is cleared.
    pub fn refresh_index(&mut self, control: &crate::Control) -> FResult<(usize, usize)> {
        if let Some(reason) = &self.repair_reason {
            return Err(FoundryError::RepairRequired(reason.clone()));
        }
        // Field-disjoint borrows: the read transaction owns self.db while the
        // writer mutably owns self.search.
        let tx = self.db.begin_read()?;
        let pending: Vec<(String, String)> = tx
            .open_table(PENDING)?
            .iter()?
            .take(PAGE)
            .map(|row| {
                let (k, v) = row?;
                Ok((k.value().into(), v.value().into()))
            })
            .collect::<Result<Vec<(String, String)>, redb::StorageError>>()
            .map_err(FoundryError::from)?;
        if pending.is_empty() {
            return Ok((0, 0));
        }
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let Some(handles) = self.search.as_mut() else {
            return Err(FoundryError::RepairRequired(
                "derived index unavailable".into(),
            ));
        };
        let memory = tx.open_table(MEMORY)?;
        for (key, _) in &pending {
            // Typed pending keys (008): the drain dispatches on the prefix.
            // An untyped key cannot exist in a v3 store — the upgrade
            // migrated them all — so it names authoritative corruption.
            if let Some(path) = key.strip_prefix("source:") {
                handles
                    .writer
                    .delete_term(Term::from_field_text(handles.fields.path, path));
                if let Some(source) = sources.get(path)? {
                    let source: SourceMeta = decode(source.value(), "source")?;
                    // Documents are built from the verified source bytes.
                    let verified = reconstruct_verified(&stored, path, &source)?;
                    for document in
                        search_documents(&handles.fields, path, &source.hash, &verified.body)
                    {
                        handles.writer.add_document(document)?;
                    }
                }
            } else if let Some(id) = key.strip_prefix("memory:") {
                // The key field is unique per memory record and no source
                // key can equal it (source keys always contain NUL).
                handles
                    .writer
                    .delete_term(Term::from_field_text(handles.fields.key, key));
                if let Some(raw) = memory.get(id)? {
                    // An undecodable row gets no document (it is named at get,
                    // search validation and export) so one corrupt record can
                    // never stall source indexing behind it.
                    if let Ok(record) = serde_json::from_str::<MemoryRecord>(raw.value()) {
                        handles.writer.add_document(memory_document(
                            &handles.fields,
                            &record.id,
                            record.revision,
                            &record.text,
                        ))?;
                    }
                }
            } else {
                return Err(FoundryError::CorruptStore(format!(
                    "pending key {key:?} is not typed (source:/memory:)"
                )));
            }
        }
        drop(memory);
        handles.writer.commit()?;
        handles.reader.reload()?;
        drop(stored);
        drop(sources);
        drop(tx);
        // Boundary: search documents committed, pending work not yet cleared.
        fault!(
            INDEX_AFTER_SEARCH_COMMIT,
            Some(&*self),
            Some(control),
            &pending.len().to_string()
        )?;
        // Cooperative checkpoint between search commit and pending clear:
        // cancellation here leaves committed search plus durable pending work,
        // and replay is idempotent.
        control.check()?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(PENDING)?;
            for (path, hash) in &pending {
                let matches = table.get(path.as_str())?.is_some_and(|v| v.value() == hash);
                if matches {
                    table.remove(path.as_str())?;
                }
            }
        }
        tx.commit()?;
        // (sources drained, memory documents drained); any other key shape
        // was refused above, so the split covers the whole batch.
        Ok((
            pending
                .iter()
                .filter(|(key, _)| key.starts_with("source:"))
                .count(),
            pending
                .iter()
                .filter(|(key, _)| key.starts_with("memory:"))
                .count(),
        ))
    }

    /// Drain pending index work in bounded batches until empty or cancelled.
    pub fn refresh(&mut self, control: &crate::Control) -> FResult<(usize, usize)> {
        let mut total = (0usize, 0usize);
        loop {
            // Cooperative cancellation between index batches.
            control.check()?;
            match self.refresh_index(control) {
                Ok((0, 0)) => return Ok(total),
                Ok((sources, memories)) => {
                    total.0 += sources;
                    total.1 += memories;
                }
                Err(FoundryError::Cancelled(_) | FoundryError::DeadlineExceeded(_)) => {
                    return Err(FoundryError::Cancelled(None));
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Search hits for context and path-less callers: [`Self::search_in`]
    /// without a `path` filter.
    pub fn search(&self, query: &str, limit: usize) -> FResult<SearchOutcome> {
        self.search_in(query, None, limit)
    }

    /// Search hits for the CLI and MCP: the materialized ranking of
    /// [`Self::search_candidates`], restricted by `path` to one file or
    /// directory subtree when given.
    pub fn search_in(
        &self,
        query: &str,
        path: Option<&str>,
        limit: usize,
    ) -> FResult<SearchOutcome> {
        Ok(Self::search_outcome(self.search_candidates(
            query,
            path,
            limit,
            &crate::Control::unbounded(),
        )?))
    }

    /// The materialized hits of one candidate batch, as the CLI and MCP
    /// render them: locator lines never carry a selection tag, so a dense
    /// hit names its matched unit's handle under the `semantic` label and
    /// never claims `whole_unit` (009 T002).
    pub fn search_outcome(batch: CandidateBatch) -> SearchOutcome {
        let semantic = batch.semantic;
        let hits = batch
            .items
            .into_iter()
            .filter_map(|item| {
                let dense_only = item.is_dense_only();
                let handle = item.handle?;
                let text = item.forms.into_iter().find_map(|form| match form {
                    RenderedForm::Verbatim(text) => Some(text),
                    _ => None,
                })?;
                let label = if dense_only {
                    "semantic".to_owned()
                } else {
                    item.label
                };
                Some(Hit {
                    path: handle.path.clone(),
                    start_line: item.start_line,
                    end_line: item.end_line,
                    handle,
                    text,
                    label,
                    tier: item.tier,
                    line: item.line,
                })
            })
            .collect();
        let counters = batch.counters;
        let freshness = batch.freshness;
        SearchOutcome {
            workspace_id: freshness.workspace_id,
            source_revision: freshness.source_revision,
            hits,
            pending_sources: freshness.pending_sources,
            stale_candidates: counters.stale,
            capped: counters.capped,
            candidate_limit: CANDIDATE_LIMIT,
            candidate_limit_reached: counters.candidates_full,
            truncated: counters.truncated || counters.candidates_full,
            scan_state: freshness.scan_state,
            semantic,
        }
    }

    /// Two-tier candidate selection for one store (context-v2 § Two-tier
    /// query, § Hit materialization). Tier 1 is exact definitions (at most
    /// 64, the most specific tier-1 run first, smallest `key_hash` kept per
    /// run, ordered by path and start within it); tier 2 is lexical (at most
    /// 256 by score, `key_hash` breaking cutoff ties).
    /// `path` restricts both tiers to a file or directory subtree. Candidates
    /// are revalidated in one final read transaction, merged per delivery
    /// unit, capped at 4 per file and cut to `limit`.
    pub fn search_candidates(
        &self,
        query: &str,
        path: Option<&str>,
        limit: usize,
        control: &crate::Control,
    ) -> FResult<CandidateBatch> {
        if query.trim().is_empty() || query.len() > 4096 {
            return Err(FoundryError::InvalidArgument(
                "query must contain 1..4096 nonblank bytes".into(),
            ));
        }
        if !(1..=64).contains(&limit) {
            return Err(FoundryError::InvalidArgument("limit must be 1..64".into()));
        }
        // The path filter is validated before the workspace and index
        // requirements (error precedence the extraction must not change).
        path.map(path_filter).transpose()?;
        let workspace_id = self.require_workspace_id()?;
        let (first, second, candidates_full) = self.collect_two_tier(query, path, control)?;

        // Final read: revalidate, merge per delivery unit and cap per file over
        // the whole candidate window, then cut to `limit`; stale and capped
        // skips are counted past the cut.
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let mut counters = CandidateCounters {
            candidates_full,
            ..CandidateCounters::default()
        };
        let mut current: std::collections::BTreeMap<String, Option<SourceMeta>> =
            std::collections::BTreeMap::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut per_file: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        let mut kept: Vec<Candidate> = Vec::new();
        for candidate in first.into_iter().chain(second) {
            if !current.contains_key(&candidate.path) {
                let meta = match sources.get(candidate.path.as_str())? {
                    Some(raw) => Some(decode::<SourceMeta>(raw.value(), "source")?),
                    None => None,
                };
                current.insert(candidate.path.clone(), meta);
            }
            if current[&candidate.path]
                .as_ref()
                .is_none_or(|meta| meta.hash != candidate.hash)
            {
                counters.stale += 1;
                continue;
            }
            if !seen.insert(candidate.unit()) {
                continue;
            }
            let count = per_file.entry(candidate.path.clone()).or_default();
            if *count == PER_FILE_CAP {
                counters.capped += 1;
                continue;
            }
            *count += 1;
            if kept.len() == limit {
                counters.truncated = true;
                continue;
            }
            kept.push(candidate);
        }
        let wanted: std::collections::BTreeSet<String> =
            analyzed_terms(crate::syntax::code_subtokens, query)
                .into_iter()
                .collect();
        let mut verified: std::collections::BTreeMap<String, VerifiedSource> =
            std::collections::BTreeMap::new();
        let mut items = Vec::with_capacity(kept.len());
        for (rank, candidate) in kept.into_iter().enumerate() {
            let Some(Some(meta)) = current.get(&candidate.path) else {
                continue;
            };
            if !verified.contains_key(&candidate.path) {
                let source = reconstruct_verified(&stored, &candidate.path, meta)?;
                verified.insert(candidate.path.clone(), source);
            }
            let body = &verified[&candidate.path].body;
            let (from, to) = (candidate.unit_start as usize, candidate.unit_end as usize);
            if !(from < to
                && to <= body.len()
                && body.is_char_boundary(from)
                && body.is_char_boundary(to))
            {
                return Err(FoundryError::CorruptStore(format!(
                    "search document unit outside {}",
                    candidate.path
                )));
            }
            let text = &body[from..to];
            let line_of = |at: usize| {
                body.as_bytes()[..at]
                    .iter()
                    .filter(|&&b| b == b'\n')
                    .count() as u64
                    + 1
            };
            let start_line = line_of(from);
            let head = candidate.unit_head as usize;
            if !(from <= head && head < to) {
                return Err(FoundryError::CorruptStore(format!(
                    "search document head outside its unit in {}",
                    candidate.path
                )));
            }
            let line = if candidate.tier == 1 {
                line_of(head)
            } else {
                start_line + best_line_index(text, &wanted)
            };
            let label = match &candidate.qname {
                Some(qname) => format!("{} {qname}", candidate.kind),
                None => candidate.kind.clone(),
            };
            let end_line = start_line
                + text.as_bytes()[..text.len().saturating_sub(1)]
                    .iter()
                    .filter(|&&b| b == b'\n')
                    .count() as u64;
            items.push(RankedItem {
                tier: candidate.tier,
                rank,
                score: candidate.score,
                handle: Some(SourceHandle {
                    workspace_id: workspace_id.clone(),
                    path: candidate.path,
                    sha256: meta.hash.clone(),
                    start: candidate.unit_start,
                    end: candidate.unit_end,
                }),
                start_line,
                end_line,
                line,
                label,
                lang: candidate.lang,
                semantic: None,
                forms: vec![RenderedForm::Verbatim(text.to_owned())],
            });
        }
        let freshness = self.freshness_in(&tx)?;
        Ok(CandidateBatch {
            freshness,
            items,
            counters,
            semantic: None,
            route: None,
        })
    }

    /// The two-tier candidate collection shared by the baseline and the
    /// semantic paths (context-v2 § Two-tier query): tier 1 exact
    /// definitions of the query's tier-1 runs (at most 64; runs by
    /// specificity, each contributing its smallest-`key_hash` definitions to
    /// the slots left, ordered by path and start within the run), tier 2
    /// lexical (at most 256 by score, `key_hash` breaking cutoff ties, tier-1
    /// units removed), and whether a window filled. No source is read here;
    /// validation happens in the callers' final read.
    fn collect_two_tier(
        &self,
        query: &str,
        path: Option<&str>,
        control: &crate::Control,
    ) -> FResult<(Vec<Candidate>, Vec<Candidate>, bool)> {
        let filter = path.map(path_filter).transpose()?;
        let handles = self.require_search()?;
        let fields = &handles.fields;
        let restrict = |query: Box<dyn Query>| -> Box<dyn Query> {
            // Source tiers never see memory documents (008): both namespaces
            // live in one index, so every source query excludes kind:"memory".
            let mut clauses = vec![
                (Occur::Must, query),
                (
                    Occur::MustNot,
                    Box::new(TermQuery::new(
                        Term::from_field_text(fields.kind, "memory"),
                        IndexRecordOption::Basic,
                    )) as Box<dyn Query>,
                ),
            ];
            if let Some(dir) = &filter {
                clauses.push((
                    Occur::Must,
                    Box::new(TermQuery::new(
                        Term::from_field_text(fields.dir, dir),
                        IndexRecordOption::Basic,
                    )) as Box<dyn Query>,
                ));
            }
            Box::new(BooleanQuery::new(clauses))
        };
        let term = |field: Field, text: &str, option: IndexRecordOption| -> Box<dyn Query> {
            Box::new(TermQuery::new(Term::from_field_text(field, text), option))
        };
        let searcher = handles.reader.searcher();
        // Tier 1: one exact count and the 64 smallest-`key_hash` definitions
        // per run, under the same restriction as the documents it keeps.
        let definitions = (
            Count,
            TopDocs::with_limit(TIER1_LIMIT).tweak_score(|reader: &SegmentReader| {
                let key_hash = reader
                    .fast_fields()
                    .u64("key_hash")
                    .expect("schema v2 fast field")
                    .first_or_default_col(0);
                move |doc: DocId, _score: Score| Reverse(key_hash.get_val(doc))
            }),
        );
        let mut runs: Vec<(usize, String, Vec<tantivy::DocAddress>)> = Vec::new();
        for run in tier1_runs(query) {
            let (count, top) = searcher.search(
                restrict(term(fields.def_name, &run, IndexRecordOption::Basic)).as_ref(),
                &definitions,
            )?;
            if count > 0 {
                let top = top.into_iter().map(|(_, address)| address).collect();
                runs.push((count, run, top));
            }
        }
        // Specificity: fewer definitions first, then run text. Each run fills
        // the slots left; a document already kept stays under its earlier run.
        runs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let defined: usize = runs.iter().map(|(count, ..)| count).sum();
        let mut kept: Vec<tantivy::DocAddress> = Vec::new();
        let mut tier1: Vec<Vec<tantivy::DocAddress>> = Vec::new();
        for (_, _, top) in runs {
            let room = TIER1_LIMIT - kept.len();
            if room == 0 {
                break;
            }
            let group: Vec<tantivy::DocAddress> = top
                .into_iter()
                .filter(|address| !kept.contains(address))
                .take(room)
                .collect();
            kept.extend(&group);
            tier1.push(group);
        }
        let mut clauses: Vec<Box<dyn Query>> = Vec::new();
        for part in query.split_whitespace() {
            let terms = analyzed_terms(crate::syntax::code_subtokens, part);
            match terms.as_slice() {
                [] => {}
                [one] => clauses.push(term(fields.body, one, IndexRecordOption::WithFreqs)),
                many => clauses.push(Box::new(PhraseQuery::new(
                    many.iter()
                        .map(|subtoken| Term::from_field_text(fields.body, subtoken))
                        .collect(),
                ))),
            }
        }
        let mut idents = analyzed_terms(crate::syntax::identifier_runs, query);
        idents.sort();
        idents.dedup();
        for ident in &idents {
            clauses.push(Box::new(BoostQuery::new(
                term(fields.ident, ident, IndexRecordOption::WithFreqs),
                3.0,
            )));
        }
        clauses.push(Box::new(BoostQuery::new(
            term(fields.path, query.trim(), IndexRecordOption::Basic),
            100.0,
        )));
        let collector =
            TopDocs::with_limit(CANDIDATE_LIMIT).tweak_score(|reader: &SegmentReader| {
                let key_hash = reader
                    .fast_fields()
                    .u64("key_hash")
                    .expect("schema v2 fast field")
                    .first_or_default_col(0);
                move |doc: DocId, score: Score| (score, Reverse(key_hash.get_val(doc)))
            });
        let tier2: Vec<(Score, tantivy::DocAddress)> = searcher
            .search(
                restrict(Box::new(BooleanQuery::union(clauses))).as_ref(),
                &collector,
            )?
            .into_iter()
            .map(|((score, _), address)| (score, address))
            .collect();
        // A window filled: tier 1's 64 slots with definitions left over, or
        // tier 2's limit.
        let candidates_full =
            (kept.len() == TIER1_LIMIT && defined > TIER1_LIMIT) || tier2.len() >= CANDIDATE_LIMIT;
        control.check()?;

        let read = |tier: u8, score: Score, address| -> FResult<Candidate> {
            let doc: TantivyDocument = searcher.doc(address)?;
            let invalid = || FoundryError::CorruptStore("invalid search document".into());
            let text = |field| {
                doc.get_first(field)
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            };
            let number = |field| {
                doc.get_first(field)
                    .and_then(|v| v.as_u64())
                    .ok_or_else(invalid)
            };
            Ok(Candidate {
                tier,
                score,
                path: text(fields.path).ok_or_else(invalid)?,
                hash: text(fields.hash).ok_or_else(invalid)?,
                start: number(fields.start)?,
                unit_start: number(fields.unit_start)?,
                unit_head: number(fields.unit_head)?,
                unit_end: number(fields.unit_end)?,
                kind: text(fields.kind).ok_or_else(invalid)?,
                lang: text(fields.lang),
                qname: text(fields.qname),
            })
        };
        let mut first: Vec<Candidate> = Vec::with_capacity(kept.len());
        for group in tier1 {
            let mut run: Vec<Candidate> = group
                .into_iter()
                .map(|address| read(1, 0.0, address))
                .collect::<FResult<_>>()?;
            run.sort_by(|a, b| a.path.cmp(&b.path).then(a.start.cmp(&b.start)));
            first.extend(run);
        }
        let units: std::collections::BTreeSet<_> = first.iter().map(Candidate::unit).collect();
        let mut second: Vec<Candidate> = Vec::new();
        for (score, address) in tier2 {
            let candidate = read(2, score, address)?;
            if !units.contains(&candidate.unit()) {
                second.push(candidate);
            }
        }
        second.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.start.cmp(&b.start))
        });
        Ok((first, second, candidates_full))
    }

    /// The 009 T002 fused candidate selection: the D001 merge — tier-1
    /// exact definitions first, then reciprocal-rank fusion (k = 60) over
    /// the lexical top 256 and the dense top 64, ties by path then start.
    ///
    /// Everything is validated in ONE final read BEFORE fusion. A lexical
    /// document whose source changed is dropped and counted stale, so it can
    /// neither suppress nor coalesce with a current dense unit of the same
    /// span. A dense hit expands through the serving generation's own label
    /// map (no partition walk): each location must satisfy the request's
    /// path filter, then is dropped and counted stale unless its recorded
    /// source hash is the current one and its range lies inside the source.
    /// The fused list is then merged per unit, capped per file and cut to
    /// `limit` like the baseline. A unit the dense window retrieved renders
    /// as neural evidence (whole unit / lexical span / preview decided by
    /// the packer); every other candidate renders exactly as the baseline
    /// does. The coverage word is `ready` only for a complete generation
    /// published at this read's source revision; otherwise `partial`.
    #[cfg(feature = "semantic")]
    pub fn search_candidates_semantic(
        &self,
        query: &str,
        path: Option<&str>,
        limit: usize,
        control: &crate::Control,
        dense: &crate::neural::query::DenseWindow,
    ) -> FResult<CandidateBatch> {
        use crate::neural::merge::MergeUnit;
        if query.trim().is_empty() || query.len() > 4096 {
            return Err(FoundryError::InvalidArgument(
                "query must contain 1..4096 nonblank bytes".into(),
            ));
        }
        if !(1..=64).contains(&limit) {
            return Err(FoundryError::InvalidArgument("limit must be 1..64".into()));
        }
        // The path filter is validated before the workspace and index
        // requirements, as in the baseline path.
        let filter = path.map(path_filter).transpose()?;
        let workspace_id = self.require_workspace_id()?;
        let (first, second, candidates_full) = self.collect_two_tier(query, path, control)?;

        // Final read: every candidate is validated here, before fusion.
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let mut counters = CandidateCounters {
            candidates_full,
            ..CandidateCounters::default()
        };
        let mut current: std::collections::BTreeMap<String, Option<SourceMeta>> =
            std::collections::BTreeMap::new();
        let load = |current: &mut std::collections::BTreeMap<String, Option<SourceMeta>>,
                    path: &str|
         -> FResult<()> {
            if !current.contains_key(path) {
                let meta = match sources.get(path)? {
                    Some(raw) => Some(decode::<SourceMeta>(raw.value(), "source")?),
                    None => None,
                };
                current.insert(path.to_owned(), meta);
            }
            Ok(())
        };
        // Lexical documents of the current source version only.
        let mut fresh: [Vec<Candidate>; 2] = [Vec::new(), Vec::new()];
        for (kept, documents) in fresh.iter_mut().zip([first, second]) {
            for candidate in documents {
                load(&mut current, &candidate.path)?;
                if current[&candidate.path]
                    .as_ref()
                    .is_some_and(|meta| meta.hash == candidate.hash)
                {
                    kept.push(candidate);
                } else {
                    counters.stale += 1;
                }
            }
        }
        let [first, second] = fresh;
        // Dense locations: the path restriction first, then freshness.
        let mut dense_units: Vec<MergeUnit> = Vec::new();
        for hit in &dense.hits {
            for location in dense.units(hit) {
                if filter
                    .as_deref()
                    .is_some_and(|filter| !within_filter(&location.path, filter))
                {
                    continue;
                }
                load(&mut current, &location.path)?;
                let current_location =
                    current[location.path.as_str()]
                        .as_ref()
                        .is_some_and(|meta| {
                            meta.hash == location.source_sha256 && location.end <= meta.bytes as u64
                        });
                if !current_location {
                    counters.stale += 1;
                    continue;
                }
                dense_units.push(MergeUnit {
                    path: location.path.clone(),
                    start: location.start,
                    end: location.end,
                });
            }
        }
        let unit_of = |candidate: &Candidate| MergeUnit {
            path: candidate.path.clone(),
            start: candidate.unit_start,
            end: candidate.unit_end,
        };
        let tier1_units: Vec<MergeUnit> = first.iter().map(unit_of).collect();
        let lexical_units: Vec<MergeUnit> = second.iter().map(unit_of).collect();
        let fused = crate::neural::merge::fuse(&tier1_units, &lexical_units, &dense_units);
        // The candidate carrying each lexical unit: its first (best-ranked)
        // current document, as in the baseline.
        let mut by_unit: std::collections::HashMap<(String, u64, u64), &Candidate> =
            std::collections::HashMap::new();
        for candidate in first.iter().chain(&second) {
            by_unit.entry(candidate.unit()).or_insert(candidate);
        }

        // The baseline's merge per unit, per-file cap and cut, over the
        // FUSED order.
        let mut seen = std::collections::BTreeSet::new();
        let mut per_file: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        struct Kept<'a> {
            candidate: Option<&'a Candidate>,
            unit: MergeUnit,
            dense_rank: Option<usize>,
        }
        let mut kept: Vec<Kept<'_>> = Vec::new();
        for entry in &fused {
            if !seen.insert(&entry.unit) {
                continue;
            }
            let count = per_file.entry(entry.unit.path.clone()).or_default();
            if *count == PER_FILE_CAP {
                counters.capped += 1;
                continue;
            }
            *count += 1;
            if kept.len() == limit {
                counters.truncated = true;
                continue;
            }
            kept.push(Kept {
                candidate: by_unit
                    .get(&(entry.unit.path.clone(), entry.unit.start, entry.unit.end))
                    .copied(),
                unit: entry.unit.clone(),
                dense_rank: entry.dense_rank,
            });
        }
        let wanted: std::collections::BTreeSet<String> =
            analyzed_terms(crate::syntax::code_subtokens, query)
                .into_iter()
                .collect();
        let mut verified: std::collections::BTreeMap<String, VerifiedSource> =
            std::collections::BTreeMap::new();
        let mut items = Vec::with_capacity(kept.len());
        for (rank, kept_one) in kept.into_iter().enumerate() {
            let Some(Some(meta)) = current.get(&kept_one.unit.path) else {
                continue;
            };
            if !verified.contains_key(&kept_one.unit.path) {
                let source = reconstruct_verified(&stored, &kept_one.unit.path, meta)?;
                verified.insert(kept_one.unit.path.clone(), source);
            }
            let body = &verified[&kept_one.unit.path].body;
            let (from, to) = (kept_one.unit.start as usize, kept_one.unit.end as usize);
            if !(from < to
                && to <= body.len()
                && body.is_char_boundary(from)
                && body.is_char_boundary(to))
            {
                return Err(FoundryError::CorruptStore(format!(
                    "candidate unit outside {}",
                    kept_one.unit.path
                )));
            }
            let text = &body[from..to];
            let line_of = |at: usize| {
                body.as_bytes()[..at]
                    .iter()
                    .filter(|&&b| b == b'\n')
                    .count() as u64
                    + 1
            };
            let start_line = line_of(from);
            let end_line = start_line
                + text.as_bytes()[..text.len().saturating_sub(1)]
                    .iter()
                    .filter(|&&b| b == b'\n')
                    .count() as u64;
            let handle = SourceHandle {
                workspace_id: workspace_id.clone(),
                path: kept_one.unit.path.clone(),
                sha256: meta.hash.clone(),
                start: kept_one.unit.start,
                end: kept_one.unit.end,
            };
            match kept_one.candidate {
                Some(candidate) => {
                    let head = candidate.unit_head as usize;
                    if !(from <= head && head < to) {
                        return Err(FoundryError::CorruptStore(format!(
                            "search document head outside its unit in {}",
                            kept_one.unit.path
                        )));
                    }
                    let line = if candidate.tier == 1 {
                        line_of(head)
                    } else {
                        start_line + best_line_index(text, &wanted)
                    };
                    let label = match &candidate.qname {
                        Some(qname) => format!("{} {qname}", candidate.kind),
                        None => candidate.kind.clone(),
                    };
                    items.push(RankedItem {
                        tier: candidate.tier,
                        rank,
                        score: candidate.score,
                        handle: Some(handle.clone()),
                        start_line,
                        end_line,
                        line,
                        label,
                        lang: candidate.lang.clone(),
                        semantic: kept_one.dense_rank.map(|_| SemanticEvidence {
                            matched: handle.clone(),
                            unit_body: text.to_owned(),
                            span: None,
                            unit_start_line: start_line,
                            dense_only: false,
                        }),
                        forms: vec![RenderedForm::Verbatim(text.to_owned())],
                    });
                }
                None => {
                    // A dense-only unit: neural evidence. The selected
                    // lexical span is the highest-ranked retrieved current
                    // lexical span intersecting the unit, clipped to its
                    // bounds.
                    let span = first.iter().chain(&second).find_map(|candidate| {
                        if candidate.path != kept_one.unit.path {
                            return None;
                        }
                        let (s, e) = (candidate.unit_start, candidate.unit_end);
                        let (clip_from, clip_to) =
                            (s.max(kept_one.unit.start), e.min(kept_one.unit.end));
                        (clip_from < clip_to).then(|| {
                            (
                                SourceHandle {
                                    workspace_id: workspace_id.clone(),
                                    path: kept_one.unit.path.clone(),
                                    sha256: meta.hash.clone(),
                                    start: clip_from,
                                    end: clip_to,
                                },
                                body[clip_from as usize..clip_to as usize].to_owned(),
                                line_of(clip_from as usize),
                            )
                        })
                    });
                    items.push(RankedItem {
                        tier: 2,
                        rank,
                        score: 0.0,
                        handle: Some(handle.clone()),
                        start_line,
                        end_line,
                        line: start_line,
                        label: String::new(),
                        lang: crate::syntax::Lang::from_path(&kept_one.unit.path)
                            .map(|lang| lang.tag().to_owned()),
                        semantic: Some(SemanticEvidence {
                            matched: handle,
                            unit_body: text.to_owned(),
                            span,
                            unit_start_line: start_line,
                            dense_only: true,
                        }),
                        forms: vec![RenderedForm::Verbatim(text.to_owned())],
                    });
                }
            }
        }
        let freshness = self.freshness_in(&tx)?;
        // The word describes THIS read (a context recomputes it against its
        // own later final read).
        let word = dense.coverage_at(freshness.source_revision);
        Ok(CandidateBatch {
            freshness,
            items,
            counters,
            semantic: Some(word.to_owned()),
            route: None,
        })
    }

    /// Derived memory documents matching `query` (008 memory search): the
    /// tier-2 lexical clauses over `body`/`ident`, restricted to
    /// `kind:"memory"`, at most `limit` by (score, key_hash). Callers
    /// validate each candidate against the live row before delivery.
    pub(crate) fn memory_plan(&self, query: &str) -> FResult<crate::memory::MemoryPlan> {
        if query.trim().is_empty() || query.len() > 4096 {
            return Err(FoundryError::InvalidArgument(
                "query must contain 1..4096 nonblank bytes".into(),
            ));
        }
        // The whole 256-document window is examined; whether it filled is
        // reported as `candidates:full` (context-v2 § Header line).
        let limit = MEMORY_CANDIDATE_WINDOW;
        let handles = self.require_search()?;
        let fields = &handles.fields;
        let mut clauses: Vec<Box<dyn Query>> = Vec::new();
        for part in query.split_whitespace() {
            let terms = analyzed_terms(crate::syntax::code_subtokens, part);
            match terms.as_slice() {
                [] => {}
                [one] => clauses.push(Box::new(TermQuery::new(
                    Term::from_field_text(fields.body, one),
                    IndexRecordOption::WithFreqs,
                ))),
                many => clauses.push(Box::new(PhraseQuery::new(
                    many.iter()
                        .map(|subtoken| Term::from_field_text(fields.body, subtoken))
                        .collect(),
                ))),
            }
        }
        let mut idents = analyzed_terms(crate::syntax::identifier_runs, query);
        idents.sort();
        idents.dedup();
        for ident in &idents {
            clauses.push(Box::new(BoostQuery::new(
                Box::new(TermQuery::new(
                    Term::from_field_text(fields.ident, ident),
                    IndexRecordOption::WithFreqs,
                )),
                3.0,
            )));
        }
        let top = BooleanQuery::new(vec![
            (
                Occur::Must,
                Box::new(TermQuery::new(
                    Term::from_field_text(fields.kind, "memory"),
                    IndexRecordOption::Basic,
                )) as Box<dyn Query>,
            ),
            (
                Occur::Must,
                Box::new(BooleanQuery::union(clauses)) as Box<dyn Query>,
            ),
        ]);
        let collector = TopDocs::with_limit(limit).tweak_score(|reader: &SegmentReader| {
            let key_hash = reader
                .fast_fields()
                .u64("key_hash")
                .expect("schema v2 fast field")
                .first_or_default_col(0);
            move |doc: DocId, score: Score| (score, Reverse(key_hash.get_val(doc)))
        });
        let searcher = handles.reader.searcher();
        let mut out = Vec::new();
        for (_, address) in searcher.search(&top, &collector)? {
            let doc: TantivyDocument = searcher.doc(address)?;
            let key = doc
                .get_first(fields.key)
                .and_then(|v| v.as_str())
                .ok_or_else(|| FoundryError::CorruptStore("invalid memory document".into()))?;
            let Some(id) = key.strip_prefix("memory:") else {
                return Err(FoundryError::CorruptStore(format!(
                    "memory query matched non-memory document {key:?}"
                )));
            };
            let revision = doc
                .get_first(fields.hash)
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<u64>().ok())
                .ok_or_else(|| {
                    FoundryError::CorruptStore(format!("memory document {key:?} has no revision"))
                })?;
            out.push(crate::memory::MemoryCandidate {
                id: id.to_owned(),
                revision,
            });
        }
        Ok(crate::memory::MemoryPlan {
            window_full: out.len() >= MEMORY_CANDIDATE_WINDOW,
            candidates: out,
        })
    }

    pub(crate) fn freshness_in(&self, tx: &redb::ReadTransaction) -> FResult<Freshness> {
        let meta = tx.open_table(META)?;
        let revision = read_counter(&meta, "source_revision")?;
        let scan_state = read_scan_state(&meta)?;
        Ok(Freshness {
            workspace_id: self.workspace_id.clone().unwrap_or_default(),
            source_revision: revision,
            scan_state: scan_state.clone(),
            // Source headers count source work only (008): memory pending
            // keys belong to the memory search header, not this one.
            pending_sources: count_pending_prefix(tx, "source:")?,
            indexed_snapshot: format!("revision={revision}; scan={scan_state}"),
        })
    }

    /// Context candidates (context-v2 § Context candidates and routing), in
    /// order: the first of up to 32 delivery units from the two-tier ranking,
    /// bounded graph items when the strategy resolves to graph (seeded by the
    /// paths of the top 3 units), the compiler units of the first three
    /// spans' symbols (005 T003), the remaining units, then up to 3 file
    /// outlines for the first distinct files among the units. Every candidate
    /// is revalidated, and its signature and outline forms are built, in one
    /// final read transaction.
    pub fn context_candidates(
        &self,
        query: &str,
        strategy: Strategy,
        control: &crate::Control,
    ) -> FResult<CandidateBatch> {
        Ok(self
            .context_candidates_inner(
                query,
                strategy,
                None,
                control,
                #[cfg(feature = "semantic")]
                None,
                None,
            )?
            .batch)
    }

    #[cfg(feature = "semantic")]
    pub fn context_candidates_memory_semantic(
        &self,
        query: &str,
        strategy: Strategy,
        control: &crate::Control,
        dense: &crate::neural::query::DenseWindow,
    ) -> FResult<crate::memory::MemoryContext> {
        let plan = self.memory_plan(query)?;
        self.context_candidates_inner(query, strategy, Some(plan), control, Some(dense), None)
    }

    /// [`Self::context_candidates`] with the 009 T002 dense window fused
    /// into the candidate ranking.
    #[cfg(feature = "semantic")]
    pub fn context_candidates_semantic(
        &self,
        query: &str,
        strategy: Strategy,
        control: &crate::Control,
        dense: &crate::neural::query::DenseWindow,
    ) -> FResult<CandidateBatch> {
        Ok(self
            .context_candidates_inner(query, strategy, None, control, Some(dense), None)?
            .batch)
    }

    /// [`Self::context_candidates`] with 008 memory: the derived memory
    /// candidates are collected BEFORE the final read and validated against
    /// the live rows in the SAME final read transaction as the source and
    /// graph candidates, so one snapshot governs the whole response.
    pub fn context_candidates_memory(
        &self,
        query: &str,
        strategy: Strategy,
        control: &crate::Control,
    ) -> FResult<crate::memory::MemoryContext> {
        let plan = self.memory_plan(query)?;
        self.context_candidates_inner(
            query,
            strategy,
            Some(plan),
            control,
            #[cfg(feature = "semantic")]
            None,
            None,
        )
    }

    /// [`Self::context_candidates`] with a configured 013 policy (and,
    /// optionally, 008 memory and the 009 dense window): an `auto` strategy
    /// is routed by [`crate::policy::Policy::route`] AFTER the 009 merge and
    /// BEFORE graph expansion, outside every engine transaction and under
    /// the request's own read deadline; the batch carries the route word.
    pub fn context_candidates_routed(
        &self,
        query: &str,
        strategy: Strategy,
        memory: bool,
        control: &crate::Control,
        #[cfg(feature = "semantic")] dense: Option<&crate::neural::query::DenseWindow>,
        policy: &crate::policy::Policy,
    ) -> FResult<crate::memory::MemoryContext> {
        let plan = memory.then(|| self.memory_plan(query)).transpose()?;
        self.context_candidates_inner(
            query,
            strategy,
            plan,
            control,
            #[cfg(feature = "semantic")]
            dense,
            Some(policy),
        )
    }

    fn context_candidates_inner(
        &self,
        query: &str,
        strategy: Strategy,
        memory: Option<crate::memory::MemoryPlan>,
        control: &crate::Control,
        #[cfg(feature = "semantic")] dense: Option<&crate::neural::query::DenseWindow>,
        policy: Option<&crate::policy::Policy>,
    ) -> FResult<crate::memory::MemoryContext> {
        if query.trim().is_empty() || query.len() > 4096 {
            return Err(FoundryError::InvalidArgument(
                "query must contain 1..4096 nonblank bytes".into(),
            ));
        }
        #[cfg(feature = "semantic")]
        let search = match dense {
            Some(dense) => {
                self.search_candidates_semantic(query, None, CONTEXT_UNITS, control, dense)?
            }
            None => self.search_candidates(query, None, CONTEXT_UNITS, control)?,
        };
        #[cfg(not(feature = "semantic"))]
        let search = self.search_candidates(query, None, CONTEXT_UNITS, control)?;
        control.check()?;
        // 013 T003: routing happens HERE, after the 009 merge and before
        // graph expansion. Only `auto` with a configured policy consults it;
        // explicit strategies never do, and without a policy the
        // deterministic rule decides exactly as before.
        let (resolved, route) = match (strategy, policy) {
            (Strategy::Auto, Some(policy)) => {
                let (resolved, word) = policy.route(self, query, control)?;
                (resolved, Some(word))
            }
            (Strategy::Auto, None) => (response::strategy_for_query(query), None),
            (explicit, _) => (explicit, None),
        };
        let mut edges: Vec<graph::GraphEvidence> = Vec::new();
        let mut graph_state: Option<&'static str> = None;
        // The graph examination window (32 rows per seed and direction) filled.
        let mut graph_full = false;
        // 005 T003: the compiler half of a graph context, filled only when
        // the strategy resolves to graph; its units validate in the final
        // read below either way.
        let mut compiler = graph::ContextGraphUnits::empty();
        if resolved == Strategy::Graph {
            let mut seeds: Vec<&str> = Vec::new();
            for item in &search.items {
                if let Some(handle) = &item.handle
                    && seeds.len() < 3
                    && !item.is_dense_only()
                    && !seeds.contains(&handle.path.as_str())
                {
                    seeds.push(&handle.path);
                }
            }
            let mut seen = std::collections::BTreeSet::new();
            let (mut fresh_edges, mut stale_edges, mut invalid) = (0usize, 0usize, false);
            for path in seeds {
                for reverse in [false, true] {
                    match self.graph(path, reverse, 1, CONTEXT_GRAPH_EDGES) {
                        Ok(graph) => {
                            graph_full |=
                                graph.truncated || graph.examined_edges >= CONTEXT_GRAPH_EDGES;
                            fresh_edges += graph.edges.len();
                            stale_edges += graph.stale_edges;
                            for evidence in graph.edges {
                                if seen.insert(evidence.raw.clone()) {
                                    edges.push(evidence);
                                }
                            }
                        }
                        // Component-local: baseline source context survives.
                        // Database errors keep their own named codes.
                        Err(FoundryError::GraphInvalid(_)) => invalid = true,
                        Err(other) => return Err(other),
                    }
                    control.check()?;
                }
            }
            // 005 T003: the compiler half, seeded by the first three
            // retrieved spans. Component-local like the manual edges: an
            // invalid compiler graph never fails the call.
            let mut spans: Vec<(String, u64, u64)> = Vec::new();
            let mut taken = std::collections::BTreeSet::new();
            for item in &search.items {
                let Some(handle) = &item.handle else {
                    continue;
                };
                taken.insert((handle.path.clone(), handle.start, handle.end));
                if spans.len() < graph::CONTEXT_GRAPH_SPANS && !item.is_dense_only() {
                    spans.push((handle.path.clone(), handle.start, handle.end));
                }
            }
            if !spans.is_empty() {
                match self.context_graph_units(&spans, &taken) {
                    Ok(outcome) => compiler = outcome,
                    Err(FoundryError::GraphInvalid(_)) => invalid = true,
                    Err(other) => return Err(other),
                }
            }
            graph_full |= compiler.full;
            graph_state = Some(if invalid {
                "graph_invalid"
            } else if fresh_edges > 0
                || !compiler.units.is_empty()
                || !compiler.already_selected.is_empty()
            {
                "ok"
            } else if stale_edges > 0 || compiler.stale > 0 || compiler.state_stale {
                "graph_stale"
            } else {
                "graph_unavailable"
            });
        }
        control.check()?;
        // Candidates are collected. Anything may commit before the final read.
        fault!(
            CONTEXT_BEFORE_FINAL_VALIDATION,
            Some(self),
            Some(control),
            ""
        )?;
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let mut current: std::collections::BTreeMap<String, Option<SourceMeta>> =
            std::collections::BTreeMap::new();
        let mut current_meta = |path: &str| -> FResult<Option<SourceMeta>> {
            if let Some(meta) = current.get(path) {
                return Ok(meta.clone());
            }
            let meta = match sources.get(path)? {
                Some(raw) => Some(decode::<SourceMeta>(raw.value(), "source")?),
                None => None,
            };
            current.insert(path.to_owned(), meta.clone());
            Ok(meta)
        };
        let mut counters = CandidateCounters {
            graph: graph_state,
            candidates_full: search.counters.candidates_full || graph_full,
            ..search.counters
        };
        let mut units: Vec<RankedItem> = Vec::new();
        for item in search.items {
            let Some(handle) = &item.handle else {
                continue;
            };
            let fresh = current_meta(&handle.path)?.is_some_and(|meta| meta.hash == handle.sha256);
            if fresh {
                units.push(item);
            } else {
                counters.stale += 1;
            }
        }
        let mut graph_items: Vec<RankedItem> = Vec::new();
        let mut graph_dropped = 0usize;
        for evidence in &edges {
            let edge = &evidence.edge;
            let mut valid = graph::edge_row_present(&tx, &edge.from.path, &evidence.raw)?;
            for endpoint in [&edge.from, &edge.to] {
                valid &=
                    current_meta(&endpoint.path)?.is_some_and(|meta| meta.hash == endpoint.hash);
            }
            if !valid {
                counters.stale += 1;
                graph_dropped += 1;
                continue;
            }
            graph_items.push(RankedItem {
                tier: TIER_GRAPH,
                rank: 0,
                score: 0.0,
                handle: None,
                start_line: 0,
                end_line: 0,
                line: 0,
                label: String::new(),
                lang: None,
                semantic: None,
                forms: vec![RenderedForm::Line(format!(
                    "{}:{} ({}) --{}--> {}:{} ({}) [{}; provider={}@{}]",
                    edge.from.path,
                    edge.from.line,
                    edge.from.symbol,
                    edge.kind,
                    edge.to.path,
                    edge.to.line,
                    edge.to.symbol,
                    edge.evidence,
                    evidence.provider,
                    evidence.revision
                ))],
            });
        }
        // 005 T003: the compiler evidence goes through the same final read.
        // The DELIVERED units are selected FIRST, under the one 32-unit
        // bound over the lexical ranking and the compiler candidates (the
        // first unit keeps its place, the compiler units follow the graph
        // edges, the lexical tail yields); only they are proven. One
        // witness per delivered unit is revalidated in THIS transaction
        // against the snapshot it was collected under (still selected,
        // current for the revision) and against its witness rows, and every
        // graph record that pass reads is charged to one shared allowance -
        // a depleted allowance leaves the proof unfinished and drops the
        // expansion with `candidates_full`. A failing unit is counted in
        // `stale` and dropped. A malformed producer or scope row is
        // component-local, like the collection phase: the compiler evidence
        // is discarded and the graph reports `graph_invalid`, while valid
        // source and manual-edge evidence survives.
        let final_revision = self.freshness_in(&tx)?.source_revision;
        let mut compiler_items: Vec<RankedItem> = Vec::new();
        let compiler_collected =
            !compiler.units.is_empty() || !compiler.already_selected.is_empty();
        let mut compiler_used = false;
        let mut compiler_invalid = false;
        // The selection: candidates removed here are never probed.
        let first = usize::from(!units.is_empty());
        let delivered: Vec<&graph::ContextUnit> = {
            let mut candidates: Vec<&graph::ContextUnit> = compiler.units.iter().collect();
            if candidates.len() > CONTEXT_UNITS - first {
                candidates.truncate(CONTEXT_UNITS - first);
                counters.candidates_full = true;
            }
            candidates
        };
        let lexical_room = CONTEXT_UNITS - delivered.len();
        if units.len() > lexical_room {
            units.truncate(lexical_room);
            counters.candidates_full = true;
        }
        // The verified-body cache context rendering uses below. The final
        // relation proof fills it first - every source a relation stands on
        // (seed, definition, reference) is reconstructed and hash-checked in
        // this transaction - so each file is verified once, and a corrupt
        // chunk is a named `corrupt_source`, never an empty graph.
        let mut bodies: std::collections::BTreeMap<String, (String, String)> =
            std::collections::BTreeMap::new();
        if compiler_collected {
            // One witness per delivered compiler unit, plus one for an
            // already-selected lexical unit - but only where that unit STAYS
            // delivered after the cut above.
            let delivered_lexical: std::collections::BTreeSet<(&str, u64, u64)> = units
                .iter()
                .filter_map(|item| {
                    item.handle
                        .as_ref()
                        .map(|handle| (handle.path.as_str(), handle.start, handle.end))
                })
                .collect();
            let witnesses: Vec<&graph::ContextWitness> = delivered
                .iter()
                .map(|unit| &unit.witness)
                .chain(compiler.already_selected.iter().filter(|witness| {
                    delivered_lexical.contains(&(
                        witness.unit.0.as_str(),
                        witness.unit.1,
                        witness.unit.2,
                    ))
                }))
                .collect();
            let verdicts = {
                let mut proof =
                    |path: &str, hash: &str, start: u64, end: u64| -> FResult<graph::SourceProof> {
                        let Some(meta) = current_meta(path)? else {
                            return Ok(graph::SourceProof::Missing);
                        };
                        if meta.hash != hash {
                            return Ok(graph::SourceProof::Missing);
                        }
                        if !bodies.contains_key(path) {
                            let verified = reconstruct_verified(&stored, path, &meta)?;
                            bodies.insert(path.to_owned(), (meta.hash, verified.body));
                        }
                        let body = &bodies[path].1;
                        let (start, end) = (start as usize, end as usize);
                        Ok(
                            if end <= body.len()
                                && body.is_char_boundary(start)
                                && body.is_char_boundary(end)
                            {
                                graph::SourceProof::Verified
                            } else {
                                graph::SourceProof::Outside
                            },
                        )
                    };
                match graph::witnesses_hold(&tx, final_revision, &witnesses, &mut proof) {
                    Ok(verdicts) => verdicts,
                    Err(FoundryError::GraphInvalid(_)) => {
                        compiler_invalid = true;
                        graph::WitnessVerdicts {
                            holds: vec![false; witnesses.len()],
                            unfinished: false,
                        }
                    }
                    Err(other) => return Err(other),
                }
            };
            if verdicts.unfinished {
                counters.candidates_full = true;
            }
            compiler_used = verdicts.holds.iter().any(|&holds| holds);
            for (unit, &holds) in delivered.iter().zip(&verdicts.holds) {
                if compiler_invalid {
                    break;
                }
                let fresh = current_meta(&unit.path)?.is_some_and(|meta| meta.hash == unit.sha256);
                if !(holds && fresh) {
                    counters.stale += 1;
                    graph_dropped += 1;
                    continue;
                }
                compiler_items.push(RankedItem {
                    tier: TIER_COMPILER,
                    rank: 0,
                    score: 0.0,
                    handle: Some(SourceHandle {
                        workspace_id: self.workspace_id.clone().unwrap_or_default(),
                        path: unit.path.clone(),
                        sha256: unit.sha256.clone(),
                        start: unit.start,
                        end: unit.end,
                    }),
                    start_line: 0,
                    end_line: 0,
                    line: unit.line,
                    label: unit.label.clone(),
                    lang: crate::syntax::Lang::from_path(&unit.path)
                        .map(|lang| lang.tag().to_owned()),
                    semantic: None,
                    forms: Vec::new(),
                });
            }
        }
        if compiler_invalid {
            counters.graph = Some("graph_invalid");
        } else if graph_state == Some("ok")
            && graph_items.is_empty()
            && !compiler_used
            && (graph_dropped > 0 || compiler_collected)
        {
            counters.graph = Some("graph_stale");
        }
        // (The one 32-unit bound was applied above, before the final proof.)
        // Verified bodies, once per path, for the signature forms of units in
        // languages with units and for the first distinct files' outlines.
        let mut outline_paths: Vec<String> = Vec::new();
        for item in &units {
            let Some(handle) = &item.handle else {
                continue;
            };
            if outline_paths.len() < CONTEXT_OUTLINES && !outline_paths.contains(&handle.path) {
                outline_paths.push(handle.path.clone());
            }
        }
        for item in &units {
            let Some(handle) = &item.handle else {
                continue;
            };
            let has_units = crate::syntax::Lang::from_path(&handle.path)
                .is_some_and(crate::syntax::Lang::has_units);
            let wanted =
                (has_units && item.label != "block") || outline_paths.contains(&handle.path);
            if wanted
                && !bodies.contains_key(&handle.path)
                && let Some(meta) = current_meta(&handle.path)?
            {
                let verified = reconstruct_verified(&stored, &handle.path, &meta)?;
                bodies.insert(handle.path.clone(), (meta.hash, verified.body));
            }
        }
        // 005 T003: the compiler units' bodies join the same verified map,
        // so their forms render from the same bytes as every other unit.
        for item in &compiler_items {
            let Some(handle) = &item.handle else {
                continue;
            };
            if !bodies.contains_key(&handle.path)
                && let Some(meta) = current_meta(&handle.path)?
            {
                let verified = reconstruct_verified(&stored, &handle.path, &meta)?;
                bodies.insert(handle.path.clone(), (meta.hash, verified.body));
            }
        }
        let outliners: std::collections::BTreeMap<&str, crate::syntax::Outliner> = bodies
            .iter()
            .filter_map(|(path, (_, body))| {
                crate::syntax::Lang::from_path(path)
                    .map(|lang| (path.as_str(), crate::syntax::Outliner::new(body, lang)))
            })
            .collect();
        for item in &mut units {
            let Some(handle) = &item.handle else {
                continue;
            };
            if item.label == "block" {
                continue;
            }
            let Some(outliner) = outliners.get(handle.path.as_str()) else {
                continue;
            };
            let signature = outliner.render(handle.start as usize..handle.end as usize, 0, 0);
            let same = item
                .forms
                .iter()
                .any(|form| matches!(form, RenderedForm::Verbatim(text) if *text == signature));
            if !same {
                item.forms.push(RenderedForm::Signature(signature));
            }
        }
        // 005 T003: source-first forms for the compiler units, from the same
        // verified bodies and outliners as the search units: the verbatim
        // unit text, then its signature when the language has units.
        for item in &mut compiler_items {
            let Some(handle) = &item.handle else {
                continue;
            };
            let Some((_, body)) = bodies.get(&handle.path) else {
                continue;
            };
            let (start, end) = (handle.start as usize, handle.end as usize);
            // The bounds cannot fail — the unit came from these same bytes
            // (the hash was revalidated above) — but a unit that somehow
            // cannot render is skipped rather than delivered empty.
            if start >= end
                || end > body.len()
                || !body.is_char_boundary(start)
                || !body.is_char_boundary(end)
            {
                continue;
            }
            item.start_line = 1 + body[..start].bytes().filter(|&b| b == b'\n').count() as u64;
            item.end_line = 1 + body[..end - 1].bytes().filter(|&b| b == b'\n').count() as u64;
            item.forms
                .push(RenderedForm::Verbatim(body[start..end].to_owned()));
            if item.label != "block"
                && let Some(outliner) = outliners.get(handle.path.as_str())
            {
                item.forms
                    .push(RenderedForm::Signature(outliner.render(start..end, 0, 0)));
            }
        }
        let mut outlines: Vec<RankedItem> = Vec::new();
        for path in &outline_paths {
            let (Some((hash, body)), Some(outliner)) =
                (bodies.get(path), outliners.get(path.as_str()))
            else {
                continue;
            };
            // Skip a file when one of the units spans the whole file.
            let whole = units
                .iter()
                .filter_map(|item| item.handle.as_ref())
                .any(|handle| {
                    handle.path == *path
                        && body[..handle.start as usize].trim().is_empty()
                        && body[handle.end as usize..].trim().is_empty()
                });
            if whole || body.is_empty() {
                continue;
            }
            let end_line = 1 + body.as_bytes()[..body.len() - 1]
                .iter()
                .filter(|&&b| b == b'\n')
                .count() as u64;
            outlines.push(RankedItem {
                tier: TIER_OUTLINE,
                rank: 0,
                score: 0.0,
                handle: Some(SourceHandle {
                    workspace_id: self.workspace_id.clone().unwrap_or_default(),
                    path: path.clone(),
                    sha256: hash.clone(),
                    start: 0,
                    end: body.len() as u64,
                }),
                start_line: 1,
                end_line,
                line: 1,
                label: String::new(),
                lang: crate::syntax::Lang::from_path(path).map(|lang| lang.tag().to_owned()),
                semantic: None,
                forms: vec![
                    RenderedForm::Outline(outliner.render(0..body.len(), 60, 120)),
                    RenderedForm::OutlineMin(outliner.render(0..body.len(), 0, 0)),
                ],
            });
        }
        // 008: memory rows validate in this same transaction; their stale
        // drops and a filled candidate window join the batch's counters.
        let hits = match &memory {
            Some(plan) => {
                let table = tx.open_table(MEMORY)?;
                let mut hits = Vec::with_capacity(
                    crate::memory::CONTEXT_MEMORY_HITS.min(plan.candidates.len()),
                );
                for candidate in &plan.candidates {
                    let record = match table.get(candidate.id.as_str())? {
                        // Same checked decoder as standalone search: the row
                        // must decode AND name this table key.
                        Some(raw) => crate::memory::decode_row(&candidate.id, raw.value())?,
                        None => {
                            counters.stale += 1;
                            continue;
                        }
                    };
                    if record.revision != candidate.revision {
                        counters.stale += 1;
                        continue;
                    }
                    if hits.len() < crate::memory::CONTEXT_MEMORY_HITS {
                        hits.push(crate::memory::MemoryHit {
                            id: record.id,
                            revision: record.revision,
                            author: record.author,
                            text: record.text,
                        });
                    }
                }
                if plan.window_full {
                    counters.candidates_full = true;
                }
                hits
            }
            None => Vec::new(),
        };
        let freshness = self.freshness_in(&tx)?;
        // 009 T002: the coverage word must describe THIS final read, not the
        // earlier search read: a source committed in between moves the
        // revision past the serving generation, so `ready` cannot carry over.
        #[cfg(feature = "semantic")]
        let semantic = match dense {
            Some(dense) => Some(dense.coverage_at(freshness.source_revision).to_owned()),
            None => search.semantic.clone(),
        };
        #[cfg(not(feature = "semantic"))]
        let semantic = search.semantic.clone();
        // The first unit, then graph items, then the compiler units, then
        // the remaining units, then outlines: a fitting first unit precedes
        // graph evidence, and compiler units sit between the graph edges and
        // the remaining search units (they are graph evidence too, ranked
        // after the manual edge lines by the same tier).
        let mut rest = units.into_iter();
        let mut items: Vec<RankedItem> = rest.next().into_iter().collect();
        items.extend(graph_items);
        items.extend(compiler_items);
        items.extend(rest);
        items.extend(outlines);
        for (rank, item) in items.iter_mut().enumerate() {
            item.rank = rank;
        }
        Ok(crate::memory::MemoryContext {
            batch: CandidateBatch {
                semantic,
                route,
                freshness,
                items,
                counters,
            },
            hits,
        })
    }

    /// Direct authoritative read in context-v2 contract order: syntax and
    /// bounds (handle and `lines`), `ws16` prefix of the bound `workspace_id`,
    /// existence, `sha32` prefix of the stored SHA-256, then range — the last
    /// three against one final read transaction. With `lines`, the outcome's
    /// handle names the intersection of those file lines with the handle's
    /// range. The outcome carries the full stored identities. Never touches
    /// the derived index.
    pub fn retrieve(
        &self,
        handle: &str,
        lines: Option<&str>,
        tokens: usize,
    ) -> FResult<RetrieveOutcome> {
        check_token_budget(tokens)?;
        let parsed = HandleRef::parse(handle)?;
        let lines = lines.map(LineSelection::parse).transpose()?;
        let bound = self.require_workspace_id()?;
        if !bound.starts_with(&parsed.ws16) {
            return Err(FoundryError::WrongWorkspace);
        }
        let read =
            self.read_handle_span(&parsed.path, parsed.start, parsed.end, &parsed.sha32, lines)?;
        let requested = SourceHandle {
            workspace_id: bound,
            path: parsed.path,
            sha256: read.sha256.clone(),
            start: read.start,
            end: read.end,
        };
        Ok(read.outcome(requested, tokens))
    }

    /// `view:"outline"` (context-v2 § Retrieve views): the requested range,
    /// after any `lines` clipping, in the `outline` and `outline-min` forms,
    /// validated in contract order like [`Self::retrieve`]. An unmapped
    /// language is `unsupported_mode`; a mapped source over 1 MiB is not
    /// parsed, so its outline equals its text.
    pub fn retrieve_outline(
        &self,
        handle: &str,
        lines: Option<&str>,
        tokens: usize,
    ) -> FResult<OutlineOutcome> {
        check_token_budget(tokens)?;
        let parsed = HandleRef::parse(handle)?;
        let lines = lines.map(LineSelection::parse).transpose()?;
        let bound = self.require_workspace_id()?;
        if !bound.starts_with(&parsed.ws16) {
            return Err(FoundryError::WrongWorkspace);
        }
        let read =
            self.read_handle_span(&parsed.path, parsed.start, parsed.end, &parsed.sha32, lines)?;
        let Some(lang) = crate::syntax::Lang::from_path(&parsed.path) else {
            return Err(FoundryError::UnsupportedMode(
                "view:\"outline\" needs a mapped language".into(),
            ));
        };
        let outliner = crate::syntax::Outliner::new(&read.body, lang);
        let range = read.start as usize..read.end as usize;
        let end_line = read.start_line
            + read.span[..read.span.len().saturating_sub(1)]
                .iter()
                .filter(|&&b| b == b'\n')
                .count() as u64;
        let outline = outliner.render(range.clone(), 60, 120);
        let outline_min = outliner.render(range, 0, 0);
        Ok(OutlineOutcome {
            requested: SourceHandle {
                workspace_id: bound,
                path: parsed.path,
                sha256: read.sha256,
                start: read.start,
                end: read.end,
            },
            start_line: read.start_line,
            end_line,
            lang: lang.tag(),
            outline,
            outline_min,
            freshness: read.freshness,
        })
    }

    /// Existence, digest (`sha32` against the prefix of the stored full
    /// SHA-256) and range, then the optional `lines` intersection, in one final
    /// read transaction.
    fn read_handle_span(
        &self,
        path: &str,
        start: u64,
        end: u64,
        sha32: &str,
        lines: Option<LineSelection>,
    ) -> FResult<HandleRead> {
        // Boundary after validation, before the final authoritative read.
        fault!(RETRIEVE_BEFORE_FINAL_READ, Some(self), None, path)?;
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let Some(raw) = sources.get(path)? else {
            return Err(FoundryError::NotFound);
        };
        let meta: SourceMeta = decode(raw.value(), "source")?;
        if !meta.hash.starts_with(sha32) {
            return Err(FoundryError::StaleHandle);
        }
        let verified = reconstruct_verified(&stored, path, &meta)?;
        let body = &verified.body;
        let len = body.len() as u64;
        let valid_empty = start == 0 && end == 0 && meta.bytes == 0;
        let valid_span = start < end
            && end <= len
            && body.is_char_boundary(start as usize)
            && body.is_char_boundary(end as usize);
        if !valid_empty && !valid_span {
            return Err(FoundryError::InvalidRange);
        }
        let (start, end) = match lines {
            None => (start as usize, end as usize),
            Some(lines) => lines.clip(body.as_bytes(), start as usize, end as usize)?,
        };
        // Boundaries were validated above; slicing the byte view is equivalent.
        let span = body.as_bytes()[start..end].to_vec();
        let start_line = body.as_bytes()[..start]
            .iter()
            .filter(|&&b| b == b'\n')
            .count() as u64
            + 1;
        let freshness = self.freshness_in(&tx)?;
        Ok(HandleRead {
            sha256: meta.hash,
            start: start as u64,
            end: end as u64,
            span,
            start_line,
            source_bytes: len,
            freshness,
            body: verified.body,
        })
    }

    fn persist_marker(&self, marker: &RebuildMarker) -> FResult<()> {
        let tx = self.db.begin_write()?;
        tx.open_table(META)?.insert(
            "search_rebuild_required",
            serde_json::to_string(marker)?.as_str(),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Explicit derived-index repair under exclusive ownership: durable
    /// marker, one retained quarantine, paged enqueue, drained replacement.
    pub fn repair_index(store_dir: &Path, control: &crate::Control) -> FResult<RepairReport> {
        control.check()?;
        let mut engine = Self::open_authoritative(store_dir)?;
        if engine.schema != SCHEMA_VERSION {
            return Err(FoundryError::UpgradeRequired {
                found: engine.schema.to_string(),
            });
        }
        let existing = Self::read_marker(&engine.db)?;
        let search_dir = store_dir.join("search");
        let mut marker = existing.unwrap_or_else(|| {
            let nonce = format!(
                "{}:{}:{}",
                std::process::id(),
                engine.directory.display(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            );
            RebuildMarker {
                owner_id: crate::digest(nonce.as_bytes()),
                quarantine: None,
                original_handled: false,
            }
        });
        // Decide the quarantine action BEFORE persisting a marker, so a refused
        // path (symlink, conflicting quarantine) leaves a healthy index usable.
        enum Quarantine {
            Done,
            Move { basename: String },
            AdoptCompletedMove,
            NothingToRetain,
        }
        let quarantine = if marker.original_handled {
            Quarantine::Done
        } else {
            if search_dir.is_symlink() {
                return Err(FoundryError::RepairPathConflict(
                    "search path is a symlink".into(),
                ));
            }
            let basename = marker
                .quarantine
                .clone()
                .unwrap_or_else(|| format!("search.quarantine-{}", &marker.owner_id[..12]));
            let search_exists = search_dir.exists();
            let target_exists = store_dir.join(&basename).symlink_metadata().is_ok();
            match (search_exists, target_exists) {
                (true, false) => Quarantine::Move { basename },
                // The rename finished before an earlier crash: adopt it.
                (false, true) if marker.quarantine.is_some() => Quarantine::AdoptCompletedMove,
                // No original derived directory existed: nothing to retain.
                (false, false) => Quarantine::NothingToRetain,
                _ => {
                    return Err(FoundryError::RepairPathConflict(format!(
                        "quarantine {basename} conflicts with the current search path"
                    )));
                }
            }
        };
        // Persist the marker before touching any derived file.
        engine.persist_marker(&marker)?;
        fault!(REPAIR_AFTER_MARKER, Some(&engine), Some(control), "")?;
        control.check()?;
        match quarantine {
            Quarantine::Done => {}
            Quarantine::Move { basename } => {
                // Record the move as intent first, so a crash on either side
                // of the rename is recognizable on retry.
                marker.quarantine = Some(basename.clone());
                engine.persist_marker(&marker)?;
                fault!(
                    REPAIR_BEFORE_QUARANTINE_RENAME,
                    Some(&engine),
                    Some(control),
                    ""
                )?;
                std::fs::rename(&search_dir, store_dir.join(&basename)).map_err(|e| {
                    FoundryError::RepairPathConflict(format!("quarantine move failed: {e}"))
                })?;
                fault!(
                    REPAIR_AFTER_QUARANTINE_RENAME,
                    Some(&engine),
                    Some(control),
                    ""
                )?;
                marker.original_handled = true;
                engine.persist_marker(&marker)?;
            }
            Quarantine::AdoptCompletedMove => {
                marker.original_handled = true;
                engine.persist_marker(&marker)?;
            }
            Quarantine::NothingToRetain => {
                marker.quarantine = None;
                marker.original_handled = true;
                engine.persist_marker(&marker)?;
            }
        }
        let quarantined_to = marker.quarantine.as_ref().map(|b| store_dir.join(b));
        control.check()?;
        // The replacement directory is reset only when it positively carries
        // this repair's identity: a regular `rebuild_id` file whose nonempty
        // content equals the marker's owner id. A completely empty directory
        // holds nothing to lose (a crash between creating it and writing the
        // identity) and is adopted. Everything else, including a missing,
        // empty, malformed or symlinked identity beside other files, is a
        // conflict: nothing is moved or deleted.
        let id_file = search_dir.join("rebuild_id");
        if search_dir.is_symlink() {
            return Err(FoundryError::RepairPathConflict(
                "replacement search path is a symlink".into(),
            ));
        }
        if search_dir.exists() {
            let matched = std::fs::symlink_metadata(&id_file)
                .ok()
                .filter(|meta| meta.file_type().is_file())
                .and_then(|_| std::fs::read_to_string(&id_file).ok())
                .is_some_and(|content| {
                    let content = content.trim();
                    !content.is_empty() && content == marker.owner_id
                });
            let empty = std::fs::read_dir(&search_dir)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(false);
            if matched {
                for entry in std::fs::read_dir(&search_dir)? {
                    let entry = entry?;
                    if entry.file_name() == "rebuild_id" {
                        continue;
                    }
                    // file_type() does not follow symlinks: a link is removed
                    // as a link, never traversed.
                    if entry.file_type()?.is_dir() {
                        std::fs::remove_dir_all(entry.path())?;
                    } else {
                        std::fs::remove_file(entry.path())?;
                    }
                }
            } else if empty {
                std::fs::write(&id_file, &marker.owner_id)?;
            } else {
                return Err(FoundryError::RepairPathConflict(
                    "replacement directory does not carry this repair's identity".into(),
                ));
            }
        } else {
            std::fs::create_dir_all(&search_dir)?;
            std::fs::write(&id_file, &marker.owner_id)?;
        }
        // Enqueue every source version in pages.
        let mut after: Option<String> = None;
        loop {
            control.check()?;
            let page: Vec<(String, String)> = {
                let tx = engine.db.begin_read()?;
                let sources = tx.open_table(SOURCES)?;
                let rows = match &after {
                    None => sources.range::<&str>(..)?,
                    Some(key) => {
                        sources.range::<&str>((Bound::Excluded(key.as_str()), Bound::Unbounded))?
                    }
                };
                rows.take(PAGE)
                    .map(|row| {
                        let (k, v) = row?;
                        Ok((k.value().to_owned(), v.value().to_owned()))
                    })
                    .collect::<Result<Vec<(String, String)>, redb::StorageError>>()
                    .map_err(FoundryError::from)?
            };
            if page.is_empty() {
                break;
            }
            let tx = engine.db.begin_write()?;
            {
                let mut pending = tx.open_table(PENDING)?;
                for (path, hash) in &page {
                    pending.insert(source_pending_key(path).as_str(), hash.as_str())?;
                }
            }
            tx.commit()?;
            fault!(REPAIR_AFTER_ENQUEUE_PAGE, Some(&engine), Some(control), "")?;
            after = page.last().map(|(k, _)| k.clone());
        }
        // Enqueue every live memory record (008): the replacement index is
        // derived, so repair rebuilds memory documents from the live table.
        let mut after: Option<String> = None;
        loop {
            control.check()?;
            let page: Vec<(String, String)> = {
                let tx = engine.db.begin_read()?;
                let memory = tx.open_table(MEMORY)?;
                let rows = match &after {
                    None => memory.range::<&str>(..)?,
                    Some(key) => {
                        memory.range::<&str>((Bound::Excluded(key.as_str()), Bound::Unbounded))?
                    }
                };
                rows.take(PAGE)
                    .map(|row| {
                        let (k, v) = row?;
                        // An undecodable row still queues (so a stale document
                        // is dropped) under a marker value; it gets no document.
                        let revision = serde_json::from_str::<MemoryRecord>(v.value())
                            .map_or_else(|_| "corrupt".to_owned(), |r| r.revision.to_string());
                        Ok((k.value().to_owned(), revision))
                    })
                    .collect::<Result<Vec<(String, String)>, redb::StorageError>>()
                    .map_err(FoundryError::from)?
            };
            if page.is_empty() {
                break;
            }
            let tx = engine.db.begin_write()?;
            {
                let mut pending = tx.open_table(PENDING)?;
                for (id, revision) in &page {
                    pending.insert(memory_pending_key(id).as_str(), revision.as_str())?;
                }
            }
            tx.commit()?;
            after = page.last().map(|(k, _)| k.clone());
        }
        control.check()?;
        // Construct the replacement index and drain.
        fault!(
            REPAIR_BEFORE_REPLACEMENT_INDEX,
            Some(&engine),
            Some(control),
            ""
        )?;
        let index = Index::create_in_dir(&search_dir, search_schema())?;
        engine.search = Some(search_handles(index)?);
        fault!(
            REPAIR_AFTER_REPLACEMENT_INDEX,
            Some(&engine),
            Some(control),
            ""
        )?;
        engine.repair_reason = None;
        let mut drained = (0usize, 0usize);
        loop {
            control.check()?;
            match engine.refresh_index(control)? {
                (0, 0) => break,
                (sources, memories) => {
                    drained.0 += sources;
                    drained.1 += memories;
                }
            }
        }
        // Clear the marker only after successful index commit/reload and an
        // empty pending table. Cancellation here leaves the marker set.
        if engine.pending()? != 0 {
            return Err(FoundryError::RepairRequired(
                "pending work remains after drain; marker retained".into(),
            ));
        }
        fault!(REPAIR_BEFORE_MARKER_CLEAR, Some(&engine), Some(control), "")?;
        control.check()?;
        {
            // The current index format is published in the same transaction
            // that clears the marker.
            let tx = engine.db.begin_write()?;
            {
                let mut meta = tx.open_table(META)?;
                meta.remove("search_rebuild_required")?;
                meta.insert("search_schema", SEARCH_SCHEMA)?;
            }
            tx.commit()?;
        }
        fault!(
            REPAIR_AFTER_SCHEMA_PUBLICATION,
            Some(&engine),
            Some(control),
            ""
        )?;
        // 009: a lexical repair also replays the semantic generation from
        // the retained f32 cache — derived state both, zero document calls.
        // A semantic failure is named in the report, never a rollback of the
        // lexical repair.
        #[cfg(feature = "semantic")]
        let semantic_index = Some(engine.semantic_rebuild_index(control).unwrap_or_else(|e| {
            crate::neural::index::SemanticIndexReport {
                rebuilt: false,
                entries: 0,
                reason: Some(e.to_string()),
            }
        }));
        #[cfg(not(feature = "semantic"))]
        let semantic_index: Option<crate::neural::index::SemanticIndexReport> = None;
        Ok(RepairReport {
            repaired: true,
            quarantined_to,
            drained_sources: drained.0,
            drained_memory: drained.1,
            reason: None,
            semantic_index,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

/// The shared `tokens` range of search, context and retrieve: 1..32768.
pub(crate) fn check_token_budget(tokens: usize) -> FResult<()> {
    if !(1..=response::MAX_BUDGET_TOKENS).contains(&tokens) {
        return Err(FoundryError::InvalidArgument(
            "token budget must be 1..32768".into(),
        ));
    }
    Ok(())
}

/// The validated bytes of one handle read, their range and the stored full digest.
struct HandleRead {
    sha256: String,
    start: u64,
    end: u64,
    span: Vec<u8>,
    start_line: u64,
    source_bytes: u64,
    freshness: Freshness,
    /// The whole verified source, for outline views.
    body: String,
}

impl HandleRead {
    fn outcome(self, requested: SourceHandle, requested_tokens: usize) -> RetrieveOutcome {
        RetrieveOutcome {
            requested,
            requested_tokens,
            span: self.span,
            start_line: self.start_line,
            source_bytes: self.source_bytes,
            freshness: self.freshness,
        }
    }
}
/// The expected workspace identity for a root: lowercase SHA-256 of the
/// canonical absolute root's UTF-8 bytes. One definition for bind, status and
/// handle checks; callers never mirror the rule.
pub fn workspace_id_for_root(root: &Path) -> FResult<String> {
    let canonical = root
        .canonicalize()
        .map_err(|e| FoundryError::InvalidArgument(format!("workspace root: {e}")))?;
    // Test-faults-only ws16 collision seam (007 T001): a test may force a
    // root's identity so `root_id_collision` is reachable without ~2^32 hash
    // evaluations. Absent from every default and release build.
    #[cfg(feature = "test-faults")]
    if let Some(path) = canonical.to_str()
        && let Some(id) = crate::fault::workspace_id_override(path)
    {
        return Ok(id);
    }
    canonical
        .to_str()
        .map(|s| crate::digest(s.as_bytes()))
        .ok_or_else(|| FoundryError::InvalidArgument("workspace path is not UTF-8".into()))
}

fn bump_revision(tx: &WriteTransaction, by: u64) -> FResult<()> {
    let mut meta = tx.open_table(META)?;
    let current = read_counter(&meta, "source_revision")?;
    let next = current
        .checked_add(by)
        .ok_or(FoundryError::RevisionExhausted)?;
    meta.insert("source_revision", next.to_string().as_str())?;
    Ok(())
}

impl Engine {
    /// Explicit indexing entry point shared by CLI and MCP.
    pub fn index(
        &mut self,
        root: &Path,
        control: &crate::Control,
    ) -> FResult<crate::ingest::IndexReport> {
        crate::ingest::index(self, root, control)
    }
}
