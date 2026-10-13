//! Authoritative source store (redb) plus a disposable derived search index
//! (Tantivy). One owner per store; explicit initialization, explicit schema
//! upgrade and explicit repair. Reads never mutate authoritative state.
use crate::Strategy;
use crate::brief::BRIEFS;
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
use tantivy::query::{
    BooleanQuery, BoostQuery, ConstScoreQuery, Occur, PhraseQuery, Query, TermQuery,
};
use tantivy::schema::{
    FAST, Field, IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions, Value,
};
use tantivy::tokenizer::{LowerCaser, RawTokenizer, TextAnalyzer, Token, TokenStream, Tokenizer};
use tantivy::{
    DocId, Index, IndexReader, IndexWriter, ReloadPolicy, Score, SegmentReader, TantivyDocument,
    Term,
};

mod context;
mod lifecycle;
mod retrieve;
mod sources;

pub(crate) const SOURCES: TableDefinition<&str, &str> = TableDefinition::new("sources");
pub(crate) const CHUNKS: TableDefinition<&str, &str> = TableDefinition::new("chunks");
pub(crate) const PENDING: TableDefinition<&str, &str> = TableDefinition::new("pending_index");
pub(crate) const META: TableDefinition<&str, &str> = TableDefinition::new("meta");
pub(crate) const SEEN: TableDefinition<&str, &str> = TableDefinition::new("scan_seen");
pub(crate) const FEEDBACK: TableDefinition<&str, &str> = TableDefinition::new("feedback");

pub const SCHEMA_VERSION: u32 = 12;
const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
const PAGE: usize = 128;
/// Refresh builds a page's search documents on at most this many threads,
/// fewer when the machine has less available parallelism (context-v2
/// § Parallel indexing).
const MAX_INDEX_THREADS: usize = 8;
/// At most this many source bytes are handed out to the build threads and
/// not yet consumed: a source is consumed once the writer has added its
/// documents.
const HANDOUT_BYTES: usize = 64 * 1024 * 1024;
/// Named parse failures keep at most this many samples, the bound of every
/// other failure-sample list.
const PARSE_FAILURE_SAMPLES: usize = 20;
/// The `kind` term of the first document of a parse-fallback source: its
/// documents are the plain blocks of an unmapped source, read everywhere as
/// `block`. Only a parse panic writes the term, once per source, so its
/// documents count exactly the sources left unparsed.
const UNPARSED_KIND: &str = "unparsed";

/// Named fault points of the parallel refresh (test-faults only; release
/// builds carry no hook code and no fault-name strings). The detail is the
/// source path for the hand-out, the pending key before an add, and empty
/// otherwise.
#[cfg(feature = "test-faults")]
pub mod fault_names {
    /// Before a source is handed out to a build thread.
    pub const INDEX_HANDOUT: &str = "ctxfoundry-fault/index.handout";
    /// Before the writer deletes and adds one pending key's documents.
    pub const INDEX_BEFORE_ADD: &str = "ctxfoundry-fault/index.before_add";
    /// Every document of the page added, before the search commit.
    pub const INDEX_BEFORE_COMMIT: &str = "ctxfoundry-fault/index.before_commit";
    /// Search committed, before the reader reload.
    pub const INDEX_BEFORE_RELOAD: &str = "ctxfoundry-fault/index.before_reload";
}

#[cfg(feature = "test-faults")]
macro_rules! index_fault {
    ($name:ident, $control:expr, $detail:expr) => {
        crate::fault::hit(
            fault_names::$name,
            &crate::fault::Ctx {
                engine: None,
                control: Some($control),
                detail: $detail,
            },
        )
    };
}

#[cfg(not(feature = "test-faults"))]
macro_rules! index_fault {
    ($name:ident, $control:expr, $detail:expr) => {
        Ok::<(), FoundryError>(())
    };
}

/// Report one [`index_hooks::Event`] to the test seam; without the feature
/// the event is not even compiled.
#[cfg(feature = "test-faults")]
macro_rules! index_event {
    ($hooks:expr, $event:expr) => {
        $hooks.emit(&$event)
    };
}

#[cfg(not(feature = "test-faults"))]
macro_rules! index_event {
    ($hooks:expr, $event:expr) => {{
        let _ = &$hooks;
    }};
}

mod indexing;

/// Search tier 2 (lexical) examines at most this many candidates.
const CANDIDATE_LIMIT: usize = 256;
/// Memory search examines at most this many derived documents before the
/// live-row validation (008), the same window as the lexical tier.
pub(crate) const MEMORY_CANDIDATE_WINDOW: usize = CANDIDATE_LIMIT;
/// Search tier 1 (exact definitions) keeps at most this many documents; an
/// anchor's resolver window keeps at most this many definitions.
const TIER1_LIMIT: usize = 64;
/// Search tier 1 ranks at most this many distinct identifier runs: the first
/// by appearance in the query.
const TIER1_RUNS: usize = 32;
/// A query has at most this many anchors (context-v2 § Anchors and
/// qualifiers).
pub const MAX_ANCHORS: usize = 4;
/// Each anchor window carries its first this-many definitions to context:
/// an ambiguous anchor lists 16, a resolved one its first and 8 directory
/// lines (context-v2 § Anchored context).
pub const ANCHOR_LIST: usize = 16;
/// At most this many hits per file survive materialization.
pub(crate) const PER_FILE_CAP: usize = 4;
/// Context draws at most this many delivery units from the ranking.
const CONTEXT_UNITS: usize = 32;
/// Context adds at most this many file outlines.
const CONTEXT_OUTLINES: usize = 3;
/// Doors list at most this many files (context-v2 § Doors).
pub(crate) const DOOR_FILES: usize = 16;
/// A first anchor's tie group of at most this many definitions gets doors,
/// each its own (context-v2 § Doors, Target and Doors of a tie group).
pub(crate) const TIE_GROUP: usize = 4;
/// A tie-group entry's door lines: the resolved cap over the largest group.
pub(crate) const GROUP_FILES: usize = DOOR_FILES / TIE_GROUP;
/// Approximate doors examine at most this many delivery units.
const DOOR_WINDOW: usize = 256;
/// The META value naming the current search index format. `"7"` enriches the
/// source-level call-name postings introduced in `"6"` with conservative Python
/// explicit import-alias names, without changing units. `"5"` (001 T008,
/// context-v2 § City map › Languages) gives 15 more languages units,
/// addresses and import keys and parses every source on the `tree-sitter`
/// 0.26 runtime under a work budget; `"4"` (001 T007,
/// context-v2 § City map) put `def_name` on one document per definition
/// and added `role`, `name_case_hash`, `addr_hash`, `name_start`/`name_end`
/// and `imports`; `"3"` added the unit's own start (its head) and the
/// leading-run unit ranges (§ Unit forest).
/// Semantic partitions independently name SOURCE_UNIT_SCHEMA; changing only
/// these derived postings does not refit or re-embed model inputs.
pub(crate) const SEARCH_SCHEMA: &str = "7";
/// Search-only posting changes do not change source units or model inputs.
/// Keep semantic partition recipes bound to the unit grammar, independently.
pub(crate) const SOURCE_UNIT_SCHEMA: &str = "5";
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
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
    /// The 009 semantic word (`ready`, `partial`) or `None` on the baseline
    /// path.
    pub semantic: Option<String>,
}

/// One rendering of a ranked item (context-v2 § Forms): the exact unit bytes,
/// the unit's signature form, a file's `outline` and `outline-min` forms, or
/// an anchored definition's `[address]` form — its item line alone,
/// navigation as a locator line is (§ Ladder for anchored definitions).
/// Packing takes the first form that fits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderedForm {
    Verbatim(String),
    Signature(String),
    Outline(String),
    OutlineMin(String),
    Address,
}

/// Ranking tiers: exact definitions (1), lexical hits (2), file outlines.
pub const TIER_OUTLINE: u8 = 4;
/// Experimental behavioral support: existing lexical candidates owned by the
/// uniquely resolved type. Selected only by 001 T010's opt-in context policy.
pub const TIER_BEHAVIOR: u8 = 3;

/// One ranked, already revalidated candidate (context-v2 § Candidate seam).
/// `handle` keeps the full identities of the source item;
/// `start_line..=end_line` are the lines the handle's range touches and
/// `line` the locator's best line.
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
    /// The place of a tier-1 definition in its anchor's resolver window;
    /// `None` for every other candidate and for an anchor-less query's tier 1.
    pub resolver: Option<Resolver>,
}

/// One definition's place in its anchor's resolver window (context-v2
/// § Resolver order): the anchor it answers and the resolver tuple.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolver {
    /// The anchor's order: its group (1 marked chain names; 2 identifier-
    /// shaped runs and unmarked chain names; 3 capitalized runs) and the
    /// byte offset of its first appearance, equal in every root of a query.
    pub anchor: (u8, usize),
    /// Distinct query qualifiers equal to an address segment of the
    /// definition (compared by hash).
    pub qualifiers: u64,
    /// The definition's name equals the anchor exactly as written.
    pub exact: bool,
    /// The definition's path role (context-v2 § Roles).
    pub role: u64,
    /// The stored byte range of the definition's name node (`name_start`,
    /// `name_end`): exact doors match compiler definitions at exactly this
    /// range (context-v2 § Doors).
    pub name: (u64, u64),
}

impl Resolver {
    /// The resolver tuple as an ascending key: more qualifiers first, then
    /// the exact-case name, then the smaller role.
    pub fn key(&self) -> (Reverse<u64>, Reverse<bool>, u64) {
        (Reverse(self.qualifiers), Reverse(self.exact), self.role)
    }
}

impl RankedItem {
    /// A candidate only the dense window retrieved.
    pub fn is_dense_only(&self) -> bool {
        self.semantic
            .as_ref()
            .is_some_and(|evidence| evidence.dense_only)
    }
}

/// A unit dense retrieval placed (spec 009 § T004 "Placement"): it comes
/// before the lexical candidates and renders through the ordinary unit
/// ladder, tagged `[semantic]` (context-v2 § Evidence items).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticEvidence {
    /// True when no lexical candidate shares the unit: it carries no
    /// delivery-unit label (its locator says `semantic`).
    pub dense_only: bool,
}

/// What candidate selection dropped or bounded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateCounters {
    /// Candidates whose source changed or vanished since indexing, and door
    /// candidates or compiler records the final read dropped.
    pub stale: u64,
    /// Hits skipped by the per-file cap.
    pub capped: u64,
    /// A candidate examination window filled: lexical tiers, context doors,
    /// or bounded semantic provenance/discovery/refinement work.
    pub candidates_full: bool,
    /// More hits survived materialization than the requested limit.
    pub truncated: bool,
}

/// The doors state of a context that requested doors (context-v2 § Doors):
/// `exact` (compiler references), `approx` (import keys and identifiers),
/// `each` (a tie group whose entries have their own exact doors),
/// `ambiguous` (a first anchor's tie group of more than [`TIE_GROUP`]: no
/// doors) or `none` (no anchor, or no current definition for the first one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorState {
    Exact,
    Approx,
    Each,
    Ambiguous,
    None,
}

impl DoorState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Approx => "approx",
            Self::Each => "each",
            Self::Ambiguous => "ambiguous",
            Self::None => "none",
        }
    }
}

/// One door line (context-v2 § Doors): a file's first site, as the
/// enclosing delivery unit, the site's line and that line's text, with the
/// count of the file's further sites.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoorLine {
    /// The delivery unit enclosing the site (full identities).
    pub unit: SourceHandle,
    /// One-based line of the site.
    pub line: u64,
    /// `<kind> <qualified name>`, or the kind alone.
    pub label: String,
    /// The site's whole line, without its terminator.
    pub text: String,
    /// The file's further sites.
    pub more: usize,
}

/// The doors of a context that requested them, built in its final read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Doors {
    pub state: DoorState,
    /// The door groups in list order: for a resolved first anchor, one onto
    /// its definition; under `each`, one per tie-group entry with exact
    /// doors, onto that entry; for a tie group without exact doors, one onto
    /// no entry (`approx`); none for `ambiguous` and `none`.
    pub groups: Vec<DoorGroup>,
}

impl Doors {
    /// Doors that were requested but not built.
    pub fn unbuilt(state: DoorState) -> Self {
        Self {
            state,
            groups: Vec::new(),
        }
    }
}

/// One door group (context-v2 § Doors): the doors of one target, summarized
/// by file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoorGroup {
    /// The definition the group opens onto: `None` for the approximate
    /// doors a tie group's name shares, attributed to no entry.
    pub target: Option<SourceHandle>,
    /// One line per file in door order: at most [`DOOR_FILES`] for a
    /// resolved target, [`GROUP_FILES`] for a tie-group entry.
    pub lines: Vec<DoorLine>,
    /// Files with doors beyond the listed ones.
    pub more_files: usize,
}

/// The first anchor's doors target (context-v2 § Doors, Target), decided
/// from its window as collected, before the final read validates it.
pub(crate) enum DoorTarget {
    /// No anchor with a definition.
    None,
    /// The first anchor's tie group is larger than [`TIE_GROUP`].
    Ambiguous,
    /// The first anchor resolved to this definition, its window's first.
    Resolved(SourceHandle),
    /// The first anchor is ambiguous with this tie group, in window order:
    /// the definitions whose resolver tuple equals the first's.
    Tied(Vec<SourceHandle>),
}

/// A context's first anchor as collected, before its final read validates
/// the windows: the doors target is decided from it, so that read can drop
/// a target but never retarget the doors or let a lower namesake into a tie
/// group. A 007 merge decides from every root's, so a definition one root
/// dropped as stale never hands the doors to another root's namesake.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectedAnchor {
    pub anchor: String,
    pub definitions: u64,
    /// The window's first [`TIE_GROUP`] + 1 definitions in window order:
    /// enough to resolve and to know whether the tie group ends within the
    /// bound (a merge's are among its roots').
    pub head: Vec<(Option<SourceHandle>, Option<Resolver>)>,
}

impl CollectedAnchor {
    pub(crate) fn of(window: &AnchorWindow) -> Self {
        Self {
            anchor: window.anchor.clone(),
            definitions: window.definitions,
            head: window
                .entries
                .iter()
                .take(TIE_GROUP + 1)
                .map(|entry| (entry.handle.clone(), entry.resolver))
                .collect(),
        }
    }

    pub(crate) fn target(&self) -> DoorTarget {
        if self.definitions == 0 {
            return DoorTarget::None;
        }
        let tuple = |resolver: &Option<Resolver>| resolver.map(|resolver| resolver.key());
        if resolves(self.definitions, self.head.iter().map(|(_, r)| *r)) {
            return match self.head.first().and_then(|(handle, _)| handle.clone()) {
                Some(handle) => DoorTarget::Resolved(handle),
                None => DoorTarget::None,
            };
        }
        let Some((_, first)) = self.head.first() else {
            return DoorTarget::None;
        };
        let tied = || {
            self.head
                .iter()
                .take_while(|(_, resolver)| tuple(resolver) == tuple(first))
        };
        if tied().count() > TIE_GROUP {
            return DoorTarget::Ambiguous;
        }
        DoorTarget::Tied(tied().filter_map(|(handle, _)| handle.clone()).collect())
    }
}

/// Resolved (context-v2 § Resolver order): exactly one definition, or a
/// first definition (of `tuples`, in window order) whose tuple is strictly
/// better than the second's.
fn resolves(definitions: u64, mut tuples: impl Iterator<Item = Option<Resolver>>) -> bool {
    let key = |resolver: Option<Resolver>| resolver.map(|resolver| resolver.key());
    definitions == 1
        || matches!(
            (tuples.next(), tuples.next()),
            (Some(first), Some(second)) if key(first) < key(second)
        )
}

/// One anchor of the query and the head of its resolver window (context-v2
/// § Anchors and qualifiers, § Resolver order).
#[derive(Clone, Debug)]
pub struct AnchorWindow {
    /// The anchor exactly as written.
    pub anchor: String,
    /// The anchor's order (see [`Resolver::anchor`]).
    pub order: (u8, usize),
    /// Every matching definition under tier 1's own restriction (memory
    /// documents excluded, the `path` filter's `dir` term); a 007 merge sums
    /// it over the merged roots.
    pub definitions: u64,
    /// The window's first [`ANCHOR_LIST`] definitions in window order — the
    /// resolver tuple, then path and start; merged: the tuple, then root
    /// order, then each root's order — revalidated, each with its verbatim
    /// form and, in a context, its signature form when that differs.
    pub entries: Vec<RankedItem>,
}

impl AnchorWindow {
    /// Resolved: exactly one definition, or a first definition whose tuple
    /// is strictly better than the second's; otherwise ambiguous.
    pub fn resolved(&self) -> bool {
        resolves(
            self.definitions,
            self.entries.iter().map(|entry| entry.resolver),
        )
    }
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
    /// The query's anchors in anchor order, each with the head of its
    /// resolver window; empty for a query without anchors. Search ignores
    /// them; context packs the anchored selection from them.
    pub anchors: Vec<AnchorWindow>,
    /// A context's doors (context-v2 § Doors) when it requested them;
    /// `None` otherwise and for search.
    pub doors: Option<Doors>,
    /// A context that requested doors: its first anchor as collected, before
    /// its final read (a 007 merge decides its doors target from every
    /// root's); `None` otherwise, for search and for a merged batch.
    pub collected: Option<CollectedAnchor>,
}

impl CandidateBatch {
    /// Anchored (context-v2 § Anchored context): the query has an anchor
    /// with at least one definition.
    pub fn anchored(&self) -> bool {
        self.anchors.iter().any(|anchor| anchor.definitions > 0)
    }
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
    /// Sources left as the plain blocks of an unmapped source after a parse
    /// panic, until parsed again (context-v2 § Parallel indexing). Derived
    /// from the derived index; `None` when that index is not serving.
    pub parse_failures: Option<u64>,
    /// At most 20 of them, in path order.
    pub parse_failure_samples: Vec<String>,
}

/// Named parse failures (context-v2 § Parallel indexing, § Languages): an
/// exact count and at most 20 samples `<path>: parse_panicked: <detail>` or
/// `<path>: parse_stopped: <detail>`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParseFailures {
    pub count: u64,
    pub samples: Vec<String>,
}

impl ParseFailures {
    /// `failure` is the labeled failure of [`SourceBuild::panic`].
    fn push(&mut self, path: &str, failure: &str) {
        self.count = self.count.saturating_add(1);
        if self.samples.len() < PARSE_FAILURE_SAMPLES {
            self.samples.push(format!("{path}: {failure}"));
        }
    }
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
/// The Tantivy schema fields (context-v2 § Search index v2, amended by
/// § Definitions and addresses: schema `"4"`).
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
    role: Field,
    name_case_hash: Field,
    addr_hash: Field,
    name_start: Field,
    name_end: Field,
    imports: Field,
    call_name: Field,
    call_coverage: Field,
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
    /// The parse panics of the current drain, acknowledged page by page; the
    /// index run that drained them names each as a scan failure.
    parse_panics: ParseFailures,
    /// Shared ingestion/lexical CPU workers; lazy and retained across refreshes.
    index_pool: Option<std::sync::Arc<rayon::ThreadPool>>,
    /// Disposable verification/residency and affected-location views for this owner.
    #[cfg(feature = "semantic")]
    pub(crate) semantic_segments:
        std::sync::Arc<std::sync::Mutex<crate::neural::index::verification::Cache>>,
    #[cfg(feature = "semantic")]
    pub(crate) semantic_mapping: std::sync::Mutex<crate::neural::index::mapping::Journal>,
    /// Source-commit notification; redb owns durable work.
    pub(crate) semantic_wake: std::sync::Arc<crate::neural::work::Wake>,
    /// The store directory, held as a descriptor opened (`O_DIRECTORY |
    /// O_NOFOLLOW`) exactly once at bind time and kept for the Engine's
    /// life. Semantic purge, publication and validation duplicate it (see
    /// [`crate::neural::anchor`]) and never resolve the store pathname
    /// again, so a substituted ancestor cannot redirect them.
    pub(crate) semantic_dir: crate::neural::anchor::Dir,
}

/// Immutable, checked source bytes and their digest, computed away from the
/// writer. Private fields prevent a caller from supplying a mismatched hash.
pub(crate) struct PreparedSource {
    path: String,
    body: String,
    hash: String,
}

impl PreparedSource {
    pub(crate) fn new(path: String, body: String) -> FResult<Self> {
        validate_path(&path)?;
        if body.len() > MAX_SOURCE_BYTES {
            return Err(FoundryError::InvalidArgument(
                "source exceeds 2 MiB limit".into(),
            ));
        }
        let hash = crate::digest(body.as_bytes());
        Ok(Self { path, body, hash })
    }
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

/// A recorded source length past the 2 MiB source bound is `corrupt_source`.
fn check_recorded_length(path: &str, meta: &SourceMeta) -> FResult<()> {
    if meta.bytes > MAX_SOURCE_BYTES {
        return Err(FoundryError::CorruptSource(format!(
            "{path}: recorded length exceeds the 2 MiB bound"
        )));
    }
    Ok(())
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
    check_recorded_length(path, meta)?;
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
    // Schema "4" (context-v2 § Definitions and addresses).
    schema.add_u64_field("role", FAST | STORED);
    schema.add_u64_field("name_case_hash", FAST);
    schema.add_u64_field("addr_hash", FAST);
    schema.add_u64_field("name_start", STORED);
    schema.add_u64_field("name_end", STORED);
    schema.add_text_field("imports", STRING);
    schema.add_text_field("call_name", STRING);
    schema.add_text_field("call_coverage", STRING | STORED);
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
        role: field("role"),
        name_case_hash: field("name_case_hash"),
        addr_hash: field("addr_hash"),
        name_start: field("name_start"),
        name_end: field("name_end"),
        imports: field("imports"),
        call_name: field("call_name"),
        call_coverage: field("call_coverage"),
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

/// The content ranges of the query's backtick code spans (context-v2
/// § Two-tier query): a run of N backticks opens a span that the next run of
/// exactly N backticks closes; an opener without such a closer is literal
/// text.
fn code_spans(query: &str) -> Vec<(usize, usize)> {
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
    spans
}

/// The tier-1 runs of a query without anchors (context-v2 § Two-tier query,
/// the 2026-10-06 rule): the identifier runs inside its code spans when it
/// has any, otherwise all of them; lowercased, deduplicated, at most
/// [`TIER1_RUNS`] by first appearance.
fn tier1_runs(query: &str, spans: &[(usize, usize)]) -> Vec<String> {
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

/// One anchor candidate of a query: the name exactly as written, its group
/// and the byte offset of its first appearance (context-v2 § Anchors and
/// qualifiers).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchorCandidate {
    pub text: String,
    /// 1: a marked chain's name; 2: an identifier-shaped unmarked run or an
    /// unmarked `::`/`->` chain's name; 3: a capitalized unmarked run.
    pub group: u8,
    pub position: usize,
}

/// A query's anchors and qualifiers before the index decides group 3
/// (context-v2 § Anchors and qualifiers).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueryAnchors {
    /// Groups 1 and 2 in that order, each by first appearance, at most
    /// [`MAX_ANCHORS`] in all: anchors whatever the index holds.
    pub fixed: Vec<AnchorCandidate>,
    /// Group 3 by first appearance: capitalized unmarked runs other than the
    /// query's first word, each an anchor when it has an exact-case
    /// definition and fewer than four anchors precede it.
    pub capitalized: Vec<AnchorCandidate>,
    /// Chain qualifiers, path segments and marked runs that are not
    /// anchors, lowercased and distinct.
    pub qualifiers: Vec<String>,
}

impl QueryAnchors {
    /// Runs and code spans are those of § Two-tier query, except that inside
    /// a code span a run may also hold `-` between letters. A chain is runs
    /// joined only by `::` or `->` (inside a code span also by `.`); its last
    /// run is its name, the others its qualifiers. A token (whitespace- and
    /// backtick-separated, enclosing punctuation trimmed) holding `/` or `\`
    /// or ending in a mapped or listed extension is a path: never a chain or
    /// an anchor, its segments qualifiers. Anchors and candidates are
    /// distinct ASCII-case-insensitively; the first appearance wins.
    pub fn parse(query: &str) -> Self {
        let spans = code_spans(query);
        let span_of = |at: usize| spans.iter().position(|&(s, e)| s <= at && at < e);
        let mut out = QueryAnchors::default();
        let push_qualifier = |qualifiers: &mut Vec<String>, text: &str| {
            let text = text.to_lowercase();
            if !text.is_empty() && !qualifiers.contains(&text) {
                qualifiers.push(text);
            }
        };
        let mut paths: Vec<(usize, usize)> = Vec::new();
        for (start, end) in query_tokens(query) {
            let token = &query[start..end];
            if is_path_token(token) {
                paths.push((start, end));
                for segment in crate::syntax::path_segments(token) {
                    push_qualifier(&mut out.qualifiers, &segment);
                }
            }
        }
        // Runs outside paths; inside one code span, runs joined by a `-`
        // between letters are one run.
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for (start, end) in crate::syntax::identifier_runs(query) {
            if paths.iter().any(|&(s, e)| s <= start && start < e) {
                continue;
            }
            if let Some(last) = runs.last_mut()
                && last.1 + 1 == start
                && query.as_bytes()[last.1] == b'-'
                && query.as_bytes()[last.1 - 1].is_ascii_alphabetic()
                && query.as_bytes()[start].is_ascii_alphabetic()
                && span_of(start).is_some()
                && span_of(start) == span_of(last.0)
            {
                last.1 = end;
                continue;
            }
            runs.push((start, end));
        }
        // Chains of consecutive runs.
        let mut chains: Vec<Vec<(usize, usize)>> = Vec::new();
        for run in runs {
            if let Some(chain) = chains.last_mut() {
                let previous = *chain.last().expect("chains are never empty");
                let joiner = &query[previous.1..run.0];
                let span = span_of(run.0);
                let joined = span == span_of(previous.0)
                    && (matches!(joiner, "::" | "->") || (span.is_some() && joiner == "."));
                if joined {
                    chain.push(run);
                    continue;
                }
            }
            chains.push(vec![run]);
        }
        let first_word = {
            let start = query.len() - query.trim_start().len();
            let end = query[start..]
                .find(char::is_whitespace)
                .map_or(query.len(), |offset| start + offset);
            (start, end)
        };
        let mut marked: Vec<AnchorCandidate> = Vec::new();
        let mut shaped: Vec<AnchorCandidate> = Vec::new();
        for chain in &chains {
            let (name_start, name_end) = *chain.last().expect("chains are never empty");
            for &(start, end) in &chain[..chain.len() - 1] {
                push_qualifier(&mut out.qualifiers, &query[start..end]);
            }
            let candidate = |group: u8| AnchorCandidate {
                text: query[name_start..name_end].to_owned(),
                group,
                position: name_start,
            };
            let name = &query[name_start..name_end];
            if span_of(name_start).is_some() {
                marked.push(candidate(1));
            } else if chain.len() > 1 || (identifier_shaped(name) && !never_anchors(name)) {
                shaped.push(candidate(2));
            } else if name.as_bytes()[0].is_ascii_uppercase()
                && !never_anchors(name)
                && !(first_word.0 <= name_start && name_start < first_word.1)
            {
                out.capitalized.push(candidate(3));
            }
        }
        let mut seen: Vec<String> = Vec::new();
        for candidate in marked.into_iter().chain(shaped) {
            let folded = candidate.text.to_ascii_lowercase();
            if seen.contains(&folded) {
                continue;
            }
            seen.push(folded);
            if out.fixed.len() < MAX_ANCHORS {
                out.fixed.push(candidate);
            } else if candidate.group == 1 {
                // A marked run that is not an anchor qualifies.
                push_qualifier(&mut out.qualifiers, &candidate.text);
            }
        }
        if out.fixed.len() == MAX_ANCHORS {
            out.capitalized.clear();
        }
        out.capitalized.retain(|candidate| {
            let folded = candidate.text.to_ascii_lowercase();
            let new = !seen.contains(&folded);
            seen.push(folded);
            new
        });
        out
    }

    /// The anchors: [`Self::fixed`], then each capitalized candidate for
    /// which `exact` finds an exact-case definition, at most four in all.
    pub fn select<E>(
        &self,
        mut exact: impl FnMut(&AnchorCandidate) -> Result<bool, E>,
    ) -> Result<Vec<AnchorCandidate>, E> {
        let mut anchors = self.fixed.clone();
        for candidate in &self.capitalized {
            if anchors.len() == MAX_ANCHORS {
                break;
            }
            if exact(candidate)? {
                anchors.push(candidate.clone());
            }
        }
        Ok(anchors)
    }
}

/// One derived record search candidate before live-row validation (the
/// document's id and the revision it was indexed at).
#[derive(Clone)]
pub(crate) struct RecordCandidate {
    pub id: String,
    pub revision: u64,
}

/// The derived record candidates of one query, collected before the final
/// read: whether the 256-document selection window filled, and the (id,
/// indexed revision) pairs it produced.
pub(crate) struct RecordPlan {
    pub window_full: bool,
    pub candidates: Vec<RecordCandidate>,
}
#[cfg(feature = "semantic")]
impl RecordPlan {
    pub(crate) fn prepend(&mut self, candidates: impl IntoIterator<Item = RecordCandidate>) {
        let mut ids = std::collections::BTreeSet::new();
        let mut merged: Vec<_> = candidates
            .into_iter()
            .chain(std::mem::take(&mut self.candidates))
            .filter(|candidate| ids.insert(candidate.id.clone()))
            .collect();
        self.window_full |= merged.len() > 256;
        merged.truncate(256);
        self.candidates = merged;
    }
}

/// What a context request adds to its query and strategy
/// ([`Engine::context_candidates_with`]): 008 memory, the 009 T002 dense
/// window, a configured 013 policy routing an `auto` strategy, and the
/// anchors a multi-root owner chose over every serving root (007; `None`
/// chooses them from this store's index).
#[derive(Clone, Copy, Default)]
pub struct ContextOptions<'a> {
    pub compose_evidence: bool,
    pub compact_evidence: bool,
    pub boundary_config: Option<&'a str>,
    pub task: Option<crate::task_context::Scope<'a>>,
    pub memory: bool,
    pub briefs: bool,
    pub experimental_behavior: bool,
    #[cfg(feature = "semantic")]
    pub dense: Option<&'a crate::neural::query::DenseWindow>,
    pub policy: Option<&'a crate::policy::Policy>,
    pub anchors: Option<&'a [AnchorCandidate]>,
}

/// The query's tokens for path detection: whitespace- and backtick-separated
/// words with enclosing punctuation trimmed, as byte ranges.
fn query_tokens(query: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    for (at, c) in query
        .char_indices()
        .chain(std::iter::once((query.len(), ' ')))
    {
        if c.is_whitespace() || c == '`' {
            let word = &query[start..at];
            let trimmed = word.trim_start_matches(['(', '[', '{', '<', '"', '\'']);
            let from = start + word.len() - trimmed.len();
            let trimmed = trimmed
                .trim_end_matches([')', ']', '}', '>', '"', '\'', ',', ';', ':', '!', '?', '.']);
            if !trimmed.is_empty() {
                out.push((from, from + trimmed.len()));
            }
            start = at + c.len_utf8();
        }
    }
    out
}

/// A path token: it holds `/` or `\`, or ends in `.<extension>` where the
/// extension is one § Dependencies and languages maps or `md txt rst json
/// yaml yml toml lock`.
fn is_path_token(token: &str) -> bool {
    token.contains(['/', '\\'])
        || crate::syntax::Lang::from_path(token).is_some()
        || token.rsplit_once('.').is_some_and(|(stem, extension)| {
            !stem.is_empty()
                && matches!(
                    extension,
                    "md" | "txt" | "rst" | "json" | "yaml" | "yml" | "toml" | "lock"
                )
        })
}

/// Group 2's shape: a run holding `_` or `$`, or a lowercase letter followed
/// by an uppercase letter (`sleep_ms`, `toolSession`, `HttpServer`).
fn identifier_shaped(run: &str) -> bool {
    run.contains(['_', '$'])
        || run
            .as_bytes()
            .windows(2)
            .any(|pair| pair[0].is_ascii_lowercase() && pair[1].is_ascii_uppercase())
}

/// Unmarked runs that never anchor: single characters, all-uppercase runs
/// without `_` (`MCP`, `WAL`) and runs of letters then digits (`v2`, `T002`,
/// `utf8`). (`e.g.`/`i.e.` are single letters; URLs are paths.)
fn never_anchors(run: &str) -> bool {
    let bytes = run.as_bytes();
    let letters = bytes.iter().take_while(|b| b.is_ascii_alphabetic()).count();
    let letters_then_digits =
        letters > 0 && letters < bytes.len() && bytes[letters..].iter().all(u8::is_ascii_digit);
    let all_upper = !run.contains('_')
        && bytes.iter().any(u8::is_ascii_alphabetic)
        && !bytes.iter().any(u8::is_ascii_lowercase);
    bytes.len() <= 1 || all_upper || letters_then_digits
}

/// One scored definition of an anchor (context-v2 § Resolver order).
#[derive(Clone, Copy, Debug)]
struct Scored {
    qualifiers: u64,
    exact: bool,
    role: u64,
    key_hash: u64,
    address: tantivy::DocAddress,
}

impl Scored {
    /// The resolver tuple, then `key_hash` ascending: the window's cut.
    fn key(&self) -> (Reverse<u64>, Reverse<bool>, u64, u64, tantivy::DocAddress) {
        (
            Reverse(self.qualifiers),
            Reverse(self.exact),
            self.role,
            self.key_hash,
            self.address,
        )
    }
}

/// What one anchor's resolver search found: every matching definition
/// counted, and the best [`TIER1_LIMIT`] by the resolver tuple then
/// `key_hash`.
#[derive(Default)]
struct ResolverFruit {
    definitions: u64,
    window: Vec<Scored>,
}

fn keep_window(window: &mut Vec<Scored>) {
    window.sort_unstable_by_key(Scored::key);
    window.truncate(TIER1_LIMIT);
}

/// Scores every definition matching one anchor's `def_name` term by the
/// resolver tuple (context-v2 § Resolver order): distinct query qualifiers
/// equal to an address segment (by hash), the exact-case name
/// (`name_case_hash`), then the role.
struct ResolverCollector {
    qualifiers: Vec<u64>,
    name_case: u64,
}

struct ResolverSegment {
    segment: tantivy::SegmentOrdinal,
    qualifiers: Vec<u64>,
    name_case: u64,
    key_hash: std::sync::Arc<dyn tantivy::columnar::ColumnValues<u64>>,
    role: std::sync::Arc<dyn tantivy::columnar::ColumnValues<u64>>,
    names: tantivy::columnar::Column<u64>,
    addresses: tantivy::columnar::Column<u64>,
    fruit: ResolverFruit,
}

impl tantivy::collector::Collector for ResolverCollector {
    type Fruit = ResolverFruit;
    type Child = ResolverSegment;

    fn for_segment(
        &self,
        segment: tantivy::SegmentOrdinal,
        reader: &SegmentReader,
    ) -> tantivy::Result<ResolverSegment> {
        let fast = reader.fast_fields();
        Ok(ResolverSegment {
            segment,
            qualifiers: self.qualifiers.clone(),
            name_case: self.name_case,
            key_hash: fast.u64("key_hash")?.first_or_default_col(0),
            role: fast.u64("role")?.first_or_default_col(0),
            names: fast.u64("name_case_hash")?,
            addresses: fast.u64("addr_hash")?,
            fruit: ResolverFruit::default(),
        })
    }

    fn requires_scoring(&self) -> bool {
        false
    }

    fn merge_fruits(&self, fruits: Vec<ResolverFruit>) -> tantivy::Result<ResolverFruit> {
        let mut merged = ResolverFruit::default();
        for fruit in fruits {
            merged.definitions += fruit.definitions;
            merged.window.extend(fruit.window);
        }
        keep_window(&mut merged.window);
        Ok(merged)
    }
}

impl tantivy::collector::SegmentCollector for ResolverSegment {
    type Fruit = ResolverFruit;

    fn collect(&mut self, doc: DocId, _score: Score) {
        let qualifiers = self
            .qualifiers
            .iter()
            .filter(|&&qualifier| {
                self.addresses
                    .values_for_doc(doc)
                    .any(|segment| segment == qualifier)
            })
            .count() as u64;
        let exact = self.names.first(doc) == Some(self.name_case);
        self.fruit.definitions += 1;
        self.fruit.window.push(Scored {
            qualifiers,
            exact,
            role: self.role.get_val(doc),
            key_hash: self.key_hash.get_val(doc),
            address: tantivy::DocAddress::new(self.segment, doc),
        });
        if self.fruit.window.len() >= 16 * TIER1_LIMIT {
            keep_window(&mut self.fruit.window);
        }
    }

    fn harvest(mut self) -> ResolverFruit {
        keep_window(&mut self.fruit.window);
        self.fruit
    }
}

/// The group-3 admission probe (context-v2 § Anchors and qualifiers):
/// whether a definition matching the anchor's `def_name` term carries its
/// exact-case `name_case_hash`. It scores, counts and keeps nothing; only an
/// admitted anchor's window is built.
struct ExactCaseProbe {
    name_case: u64,
}

struct ExactCaseSegment {
    name_case: u64,
    names: tantivy::columnar::Column<u64>,
    found: bool,
}

impl tantivy::collector::Collector for ExactCaseProbe {
    type Fruit = bool;
    type Child = ExactCaseSegment;

    fn for_segment(
        &self,
        _segment: tantivy::SegmentOrdinal,
        reader: &SegmentReader,
    ) -> tantivy::Result<ExactCaseSegment> {
        Ok(ExactCaseSegment {
            name_case: self.name_case,
            names: reader.fast_fields().u64("name_case_hash")?,
            found: false,
        })
    }

    fn requires_scoring(&self) -> bool {
        false
    }

    fn merge_fruits(&self, fruits: Vec<bool>) -> tantivy::Result<bool> {
        Ok(fruits.into_iter().any(|found| found))
    }
}

impl tantivy::collector::SegmentCollector for ExactCaseSegment {
    type Fruit = bool;

    fn collect(&mut self, doc: DocId, _score: Score) {
        if !self.found {
            self.found = self.names.first(doc) == Some(self.name_case);
        }
    }

    fn harvest(self) -> bool {
        self.found
    }
}

/// `query` under tier 1's own restriction (context-v2 § Two-tier query): no
/// memory document (008: both namespaces live in one index), and only the
/// `dir` subtree of the path filter when there is one.
fn tier_restricted(fields: &Fields, dir: Option<&str>, query: Box<dyn Query>) -> Box<dyn Query> {
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
    clauses.push((
        Occur::MustNot,
        Box::new(TermQuery::new(
            Term::from_field_text(fields.kind, "brief"),
            IndexRecordOption::Basic,
        )),
    ));
    if let Some(dir) = dir {
        clauses.push((
            Occur::Must,
            Box::new(TermQuery::new(
                Term::from_field_text(fields.dir, dir),
                IndexRecordOption::Basic,
            )) as Box<dyn Query>,
        ));
    }
    Box::new(BooleanQuery::new(clauses))
}

/// Whether the index under `searcher` defines `name` exactly as written,
/// under tier 1's restriction: [`ExactCaseProbe`] over its `def_name` term.
fn exact_case_defined(
    searcher: &tantivy::Searcher,
    fields: &Fields,
    dir: Option<&str>,
    name: &str,
) -> FResult<bool> {
    let term = TermQuery::new(
        Term::from_field_text(fields.def_name, &name.to_lowercase()),
        IndexRecordOption::Basic,
    );
    Ok(searcher.search(
        tier_restricted(fields, dir, Box::new(term)).as_ref(),
        &ExactCaseProbe {
            name_case: hash64(name),
        },
    )?)
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

/// How a source's search documents are built.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Parsing {
    /// By its path's language.
    Parsed,
    /// After its parse panicked or exceeded its work budget: the plain
    /// blocks of an unmapped source, the first with `kind` [`UNPARSED_KIND`].
    Unparsed,
}

/// The search documents of one verified source (context-v2 § Search
/// documents, § Definitions and addresses): one per syntax document,
/// carrying its delivery unit and its path's role. Exactly one document per
/// definition — the one whose range holds the start of the unit's name node
/// — carries `def_name`, `name_case_hash`, `addr_hash` and the name node's
/// range; the file's first document carries its import keys. A parse that
/// exceeds its work budget gives no documents but its stop.
fn search_documents(
    fields: &Fields,
    path: &str,
    hash: &str,
    body: &str,
    parsing: Parsing,
) -> Result<Vec<TantivyDocument>, crate::syntax::ParseStopped> {
    let lang = match parsing {
        Parsing::Parsed => crate::syntax::Lang::from_path(path),
        Parsing::Unparsed => None,
    };
    let mut dirs: Vec<&str> = path.match_indices('/').map(|(i, _)| &path[..i]).collect();
    dirs.push(path);
    let role = path_role(path);
    let path_segments = crate::syntax::path_segments(path);
    let index = crate::syntax::index(body, lang)?;
    let imports = index.imports;
    let call_names = index.call_names;
    let call_coverage = if parsing == Parsing::Unparsed {
        "incomplete"
    } else {
        index.call_coverage
    };
    Ok(index
        .documents
        .into_iter()
        .enumerate()
        .map(|(position, document)| {
            let unit = &document.unit;
            let key = format!("{path}\0{}", document.start);
            let text = &body[document.start..document.end];
            let mut out = TantivyDocument::default();
            out.add_u64(fields.key_hash, hash64(&key));
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
            // Without a language every document is a block; a parse
            // fallback's first one carries the term naming it unparsed.
            let kind = match parsing {
                Parsing::Unparsed if position == 0 => UNPARSED_KIND,
                _ => unit.kind.as_str(),
            };
            out.add_text(fields.kind, kind);
            out.add_u64(fields.role, role);
            if let Some(lang) = lang {
                out.add_text(fields.lang, lang.tag());
            }
            if position == 0 {
                out.add_text(fields.call_coverage, call_coverage);
                for name in &call_names {
                    out.add_text(fields.call_name, name);
                }
                for key in &imports {
                    out.add_text(fields.imports, key);
                }
            }
            if let Some(name) = &unit.name {
                out.add_text(fields.name, name);
                if let (Some((name_start, name_end)), Some(_)) = (unit.name_range, lang)
                    && document.start <= name_start
                    && name_start < document.end
                {
                    out.add_text(fields.def_name, name);
                    out.add_u64(fields.name_case_hash, hash64(name));
                    for segment in crate::syntax::address_segments(&path_segments, &unit.qualifiers)
                    {
                        out.add_u64(fields.addr_hash, hash64(&segment));
                    }
                    out.add_u64(fields.name_start, name_start as u64);
                    out.add_u64(fields.name_end, name_end as u64);
                }
            }
            if let Some(qname) = &unit.qname {
                out.add_text(fields.qname, qname);
            }
            out.add_text(fields.ident, text);
            out.add_text(fields.body, text);
            out
        })
        .collect())
}

/// Test seam of the parallel refresh (feature `test-faults` only; release
/// builds carry none). Hooks are installed on the thread that calls
/// [`Engine::refresh`]; each page reads them once and shares them with its
/// build threads.
#[cfg(feature = "test-faults")]
pub mod index_hooks;

#[cfg(feature = "test-faults")]
use index_hooks::Hooks as IndexHooks;

/// Without the test seam a page has no hooks.
#[cfg(not(feature = "test-faults"))]
#[derive(Clone, Copy, Default)]
struct IndexHooks;

#[cfg(not(feature = "test-faults"))]
impl IndexHooks {
    fn current() -> Self {
        Self
    }

    fn threads(&self) -> Option<usize> {
        None
    }

    fn handout_bytes(&self) -> Option<usize> {
        None
    }
}

/// Sources left as the plain blocks of an unmapped source after a parse
/// panic or a parse over its work budget (context-v2 § Parallel indexing,
/// § Languages); the scan report named which, the sample says `unparsed`.
/// Each one's first document, and
/// no other, is of kind [`UNPARSED_KIND`], so one term query counts them
/// exactly without loading a document; samples load at most
/// [`PARSE_FAILURE_SAMPLES`] documents, the smallest `key_hash` ones, and
/// name them in path order.
fn unparsed_sources(searcher: &tantivy::Searcher, fields: &Fields) -> FResult<ParseFailures> {
    let query = TermQuery::new(
        Term::from_field_text(fields.kind, UNPARSED_KIND),
        IndexRecordOption::Basic,
    );
    let first = TopDocs::with_limit(PARSE_FAILURE_SAMPLES)
        .order_by_u64_field("key_hash", tantivy::Order::Asc);
    let (count, addresses) = searcher.search(&query, &(Count, first))?;
    let mut paths = Vec::with_capacity(addresses.len());
    for (_, address) in addresses {
        let doc: TantivyDocument = searcher.doc(address)?;
        if let Some(path) = doc.get_first(fields.path).and_then(|v| v.as_str()) {
            paths.push(path.to_owned());
        }
    }
    paths.sort_unstable();
    Ok(ParseFailures {
        count: count as u64,
        samples: paths
            .into_iter()
            .map(|path| format!("{path}: unparsed: indexed as plain blocks until parsed again"))
            .collect(),
    })
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
    record_document(fields, "memory", id, revision, text)
}

fn record_document(
    fields: &Fields,
    kind: &str,
    id: &str,
    revision: u64,
    text: &str,
) -> TantivyDocument {
    let key = format!("{kind}:{id}");
    let mut out = TantivyDocument::default();
    out.add_u64(fields.key_hash, hash64(&key));
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
    out.add_text(fields.kind, kind);
    out.add_u64(fields.role, path_role(""));
    out.add_text(fields.name, id);
    out.add_text(fields.ident, text);
    out.add_text(fields.body, text);
    out
}

/// The first 8 bytes of SHA-256 of `text`, big-endian: `key_hash` (the
/// deterministic cutoff tie-breaker of both search tiers), `name_case_hash`
/// and `addr_hash`.
fn hash64(text: &str) -> u64 {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(text.as_bytes());
    let mut first = [0u8; 8];
    first.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(first)
}

/// Roles order definitions in the resolver and nothing else (context-v2
/// § Roles).
pub mod role {
    pub const SOURCE: u64 = 0;
    pub const TEST: u64 = 1;
    pub const GENERATED: u64 = 2;
    pub const VENDORED: u64 = 3;
    pub const LOCK: u64 = 4;
    pub const SNAPSHOT: u64 = 5;
}

/// The role of a workspace-relative path: the first matching rule of
/// context-v2 § Roles, components compared case-sensitively (every
/// `/`-separated component, the basename included), basename patterns as
/// written (`*` matches any run, the empty one included).
pub fn path_role(path: &str) -> u64 {
    let components: Vec<&str> = path.split('/').collect();
    let basename = components.last().copied().unwrap_or("");
    let component = |names: &[&str]| components.iter().any(|part| names.contains(part));
    let matches = |patterns: &[&str]| patterns.iter().any(|pattern| glob(pattern, basename));
    let lock = [
        "bun.lock",
        "bun.lockb",
        "package-lock.json",
        "npm-shrinkwrap.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "Cargo.lock",
        "composer.lock",
        "Gemfile.lock",
        "poetry.lock",
        "uv.lock",
        "Pipfile.lock",
        "go.sum",
        "packages.lock.json",
        "Podfile.lock",
        "pubspec.lock",
        "mix.lock",
        "flake.lock",
    ];
    if lock.contains(&basename) {
        return role::LOCK;
    }
    if matches(&["*.snap"]) || component(&["__snapshots__"]) {
        return role::SNAPSHOT;
    }
    if matches(&[
        "*.min.js",
        "*.min.css",
        "*.g.cs",
        "*.Designer.cs",
        "*.designer.cs",
        "*_pb2.py",
        "*.pb.go",
        "*.generated.*",
    ]) || component(&["generated", "__generated__", "obj"])
    {
        return role::GENERATED;
    }
    if component(&["vendor", "third_party", "third-party", "Pods"]) {
        return role::VENDORED;
    }
    if component(&[
        "test",
        "tests",
        "__tests__",
        "testing",
        "testdata",
        "fixtures",
        "e2e",
        "spec",
        "benches",
    ]) || matches(&[
        "tests.rs",
        "conftest.py",
        "*_test.go",
        "test_*.py",
        "*_test.py",
        "*.test.*",
        "*.spec.*",
        "*_spec.rb",
        "*Test.java",
        "*Tests.java",
        "*Test.kt",
        "*Tests.kt",
        "*Test.swift",
        "*Tests.swift",
        "*Test.cs",
        "*Tests.cs",
        "*Test.php",
        "*_test.cc",
        "*_test.cpp",
        "*_unittest.cc",
        "*.t",
        "*.bats",
    ]) {
        return role::TEST;
    }
    role::SOURCE
}

/// Whether `text` matches `pattern`, whose `*` matches any run of
/// characters (the empty run included) and every other character itself.
fn glob(pattern: &str, text: &str) -> bool {
    let mut pieces = pattern.split('*');
    let first = pieces.next().unwrap_or("");
    let Some(mut rest) = text.strip_prefix(first) else {
        return false;
    };
    let pieces: Vec<&str> = pieces.collect();
    let Some((last, middle)) = pieces.split_last() else {
        return rest.is_empty();
    };
    for piece in middle {
        match rest.find(piece) {
            Some(at) => rest = &rest[at + piece.len()..],
            None => return false,
        }
    }
    rest.len() >= last.len() && rest.ends_with(last)
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

/// A bounded passage containing the most distinct query concepts, then their
/// tightest occurrence span. Count each concept once: repeated boilerplate must
/// not outrank evidence spread over adjacent lines. Offsets always cite source.
#[cfg(feature = "semantic")]
fn prose_passage(
    text: &str,
    wanted: &std::collections::BTreeSet<String>,
) -> Option<(usize, usize)> {
    const BYTES: usize = 1536;
    let words: std::collections::BTreeMap<_, _> = wanted
        .iter()
        .filter(|word| !prose_stopword(word))
        .take(64)
        .enumerate()
        .map(|(id, word)| (word.as_str(), id))
        .collect();
    if words.is_empty() {
        return None;
    }
    let hits: Vec<_> = crate::syntax::code_subtokens(text)
        .into_iter()
        .filter_map(|(start, end)| {
            words
                .get(text[start..end].to_lowercase().as_str())
                .map(|&id| (start, end, id))
        })
        .collect();
    let mut counts = [0usize; 64];
    let (mut right, mut distinct) = (0, 0);
    let mut best = None;
    for left in 0..hits.len() {
        while right < hits.len() && hits[right].1 - hits[left].0 <= BYTES {
            let count = &mut counts[hits[right].2];
            distinct += usize::from(*count == 0);
            *count += 1;
            right += 1;
        }
        if right == left {
            // A single pathological identifier cannot fit the passage.
            right += 1;
            continue;
        }
        let (start, end) = (hits[left].0, hits[right - 1].1);
        let key = (distinct, Reverse(end - start), Reverse(start));
        if best.is_none_or(|(previous, _, _)| key > previous) {
            best = Some((key, start, end));
        }
        counts[hits[left].2] -= 1;
        distinct -= usize::from(counts[hits[left].2] == 0);
    }
    let (_, start, end) = best?;
    // Include a little leading context, provided it cannot displace a match.
    let line = text[..start].rfind('\n').map_or(0, |at| at + 1);
    let before = text[..line]
        .match_indices('\n')
        .rev()
        .nth(3)
        .map_or(0, |(at, _)| at + 1);
    let mut from = if start - before <= 256 { before } else { line };
    if end - from > BYTES {
        from = start;
    }
    let mut to = (from + BYTES).min(text.len());
    while !text.is_char_boundary(to) {
        to -= 1;
    }
    if let Some(newline) = text[from..to].rfind('\n')
        && from + newline + 1 >= end
    {
        to = from + newline + 1;
    }
    Some((from, to))
}

#[cfg(all(test, feature = "semantic"))]
mod passage_tests {
    use super::*;

    #[test]
    fn dense_passages_join_adjacent_concepts_without_rewarding_repetition() {
        let wanted = ["refund", "cancelled", "payments"].map(String::from).into();
        let text = format!(
            "{}{}fn settle() {{\n    refund();\n    cancelled();\n    payments();\n}}\n{}",
            "refund ".repeat(400),
            "\n// ordinary setup\n".repeat(100),
            "🦀".repeat(600)
        );
        let (start, end) = prose_passage(&text, &wanted).unwrap();
        assert!(end - start <= 1536);
        let passage = &text[start..end];
        for needle in ["fn settle", "refund();", "cancelled();", "payments();"] {
            assert!(passage.contains(needle), "{passage}");
        }
        assert_eq!(prose_passage("only unrelated source", &wanted), None);
        assert_eq!(prose_passage(&text, &["the".into()].into()), None);
    }
}

/// One search document selected by a tier, before revalidation.
#[derive(Clone)]
struct Candidate {
    tier: u8,
    score: Score,
    path: String,
    hash: String,
    start: u64,
    end: u64,
    unit_start: u64,
    /// The delivery unit's own start, after its leading run: a tier-1 hit's
    /// best line.
    unit_head: u64,
    unit_end: u64,
    kind: String,
    lang: Option<String>,
    qname: Option<String>,
    /// A tier-1 definition's place in its anchor's resolver window.
    resolver: Option<Resolver>,
}

impl Candidate {
    /// The delivery unit's identity within this store.
    fn unit(&self) -> (String, u64, u64) {
        (self.path.clone(), self.unit_start, self.unit_end)
    }

    fn selection_unit(&self, prose: bool) -> (String, u64, u64) {
        if prose && self.tier == 2 && self.unit_end.saturating_sub(self.unit_start) > 1536 {
            (
                self.path.clone(),
                self.start.max(self.unit_start),
                self.end.min(self.unit_end),
            )
        } else {
            self.unit()
        }
    }

    fn trace(&self, control: &crate::Control, workspace: &str, rank: usize, outcome: &str) {
        if let Some(trace) = control.trace() {
            trace.record(serde_json::json!({
                "stage": "selection", "origin": "lexical", "tier": self.tier,
                "rank": rank, "outcome": outcome, "label": self.qname,
                "document": {"start": self.start, "end": self.end},
                "unit": {"workspace_id": workspace, "path": self.path,
                    "sha256": self.hash, "start": self.unit_start, "end": self.unit_end},
            }));
        }
    }

    fn trace_materialization(&self, control: &crate::Control, item: &RankedItem) {
        if let Some(trace) = control.trace()
            && let Some(handle) = &item.handle
        {
            trace.record(serde_json::json!({
                "stage":"materialization", "origin":"lexical", "rank":item.rank,
                "unit":{"workspace_id":handle.workspace_id,"path":self.path,"sha256":handle.sha256,
                    "start":self.unit_start,"end":self.unit_end},
                "document":{"start":self.start,"end":self.end},
                "excerpt":handle,
            }));
        }
    }
}

/// A long unit may contain several independently useful matches. Keep at most
/// two disjoint documents in experimental prose context, within the existing
/// candidate, per-file and delivery budgets. Anchored retrieval is unchanged.
#[derive(Default)]
struct LexicalSelection {
    spans: std::collections::BTreeMap<(String, u64, u64), Vec<(u64, u64)>>,
}

impl LexicalSelection {
    fn insert(&mut self, candidate: &Candidate, prose: bool) -> Result<(), &'static str> {
        let unit = candidate.unit();
        let (_, start, end) = candidate.selection_unit(prose);
        let limit = if (start, end) == (unit.1, unit.2) {
            1
        } else {
            2
        };
        let spans = self.spans.entry(unit).or_default();
        if spans.iter().any(|&(s, e)| start < e && s < end) {
            return Err("duplicate");
        }
        if spans.len() >= limit {
            return Err("unit_window_cap");
        }
        spans.push((start, end));
        Ok(())
    }
}

/// One revalidated candidate as a ranked item over its verified source
/// `body` (context-v2 § Hit materialization): its delivery unit's handle and
/// lines, its best line (a tier-1 hit's head line, otherwise the line with
/// the most distinct query subtokens), its label and its verbatim form.
fn ranked_item(
    candidate: &Candidate,
    rank: usize,
    workspace_id: &str,
    sha256: &str,
    body: &str,
    wanted: &std::collections::BTreeSet<String>,
) -> FResult<RankedItem> {
    let (from, to) = (candidate.unit_start as usize, candidate.unit_end as usize);
    if !(from < to && to <= body.len() && body.is_char_boundary(from) && body.is_char_boundary(to))
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
        start_line + best_line_index(text, wanted)
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
    Ok(RankedItem {
        tier: candidate.tier,
        rank,
        score: candidate.score,
        handle: Some(SourceHandle {
            workspace_id: workspace_id.to_owned(),
            path: candidate.path.clone(),
            sha256: sha256.to_owned(),
            start: candidate.unit_start,
            end: candidate.unit_end,
        }),
        start_line,
        end_line,
        line,
        label,
        lang: candidate.lang.clone(),
        semantic: None,
        resolver: candidate.resolver,
        forms: vec![RenderedForm::Verbatim(text.to_owned())],
    })
}

/// Keep the matching indexed document of a long lexical unit, with exact source
/// coordinates. A prefix of a function is not a substitute for its matching body.
#[allow(clippy::too_many_arguments)]
fn ranked_context_item(
    candidate: &Candidate,
    rank: usize,
    workspace_id: &str,
    sha256: &str,
    body: &str,
    wanted: &std::collections::BTreeSet<String>,
    prose: bool,
) -> FResult<RankedItem> {
    if !prose || candidate.unit_end - candidate.unit_start <= 1536 {
        return ranked_item(candidate, rank, workspace_id, sha256, body, wanted);
    }
    let start = candidate.start.max(candidate.unit_start) as usize;
    let end = candidate.end.min(candidate.unit_end) as usize;
    let text = body
        .get(start..end)
        .ok_or_else(|| FoundryError::CorruptStore("lexical document outside source".into()))?;
    let wanted: std::collections::BTreeSet<_> = wanted
        .iter()
        .filter(|w| !prose_stopword(w))
        .cloned()
        .collect();
    let best = best_line_index(text, &wanted) as usize;
    let lines: Vec<_> = text.split_inclusive('\n').collect();
    let before: usize = lines
        .iter()
        .take(best.saturating_sub(3))
        .map(|s| s.len())
        .sum();
    let focus: usize = lines.iter().take(best).map(|s| s.len()).sum();
    let mut from = before;
    if focus.saturating_sub(from) > 512 {
        from = focus;
    }
    // Long introductory docs can match well while leaving the callable itself
    // entirely outside the excerpt. In that case expose its declaration and
    // implementation start. Matches within the body retain their focused window.
    let head = candidate.unit_head as usize;
    let (start, text, from, focus) = if matches!(candidate.kind.as_str(), "fn" | "method")
        && start + focus < head
        && head >= start + (from + 1536).min(text.len())
    {
        let start = body
            .get(..head)
            .ok_or_else(|| FoundryError::CorruptStore("callable head outside source".into()))?
            .rfind('\n')
            .map_or(0, |at| at + 1)
            .max(candidate.unit_start as usize);
        let text = body
            .get(start..candidate.unit_end as usize)
            .ok_or_else(|| FoundryError::CorruptStore("callable head outside source".into()))?;
        (start, text, 0, head - start)
    } else {
        (start, text, from, focus)
    };
    let mut to = (from + 1536).min(text.len());
    while !text.is_char_boundary(to) {
        to -= 1;
    }
    if let Some(newline) = text[from..to].rfind('\n')
        && from + newline >= focus
    {
        to = from + newline + 1;
    }
    if from >= to {
        return ranked_item(candidate, rank, workspace_id, sha256, body, &wanted);
    }
    let mut excerpt = candidate.clone();
    excerpt.unit_start = (start + from) as u64;
    excerpt.unit_end = (start + to) as u64;
    excerpt.unit_head = excerpt.unit_start;
    let mut item = ranked_item(&excerpt, rank, workspace_id, sha256, body, &wanted)?;
    item.label = format!("excerpt {}", item.label);
    Ok(item)
}

pub(crate) fn prose_stopword(word: &str) -> bool {
    [
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "could", "do", "does", "for",
        "from", "how", "in", "into", "is", "it", "of", "on", "or", "should", "that", "the",
        "their", "this", "to", "was", "when", "where", "which", "while", "with", "without",
        "would",
    ]
    .contains(&word.to_ascii_lowercase().as_str())
}

/// One anchor's resolver window within tier 1: the anchor, its definition
/// count and its range of [`TwoTier::first`], in window order.
struct WindowSlice {
    anchor: String,
    order: (u8, usize),
    definitions: u64,
    range: std::ops::Range<usize>,
}

/// Context-only intent. Ordinary search retains exact-definition precedence.
#[derive(Clone, Copy, Default)]
struct SearchSelection<'a> {
    anchors: Option<&'a [AnchorCandidate]>,
    prose: bool,
}

/// Deliberately narrow: explicit code, paths and locator requests retain the
/// baseline. Anchor admission is checked separately over the indexed roots.
fn prose_query(query: &str) -> bool {
    let words: Vec<_> = query.split_whitespace().collect();
    words.len() >= 3
        && !["where", "find", "locate", "show", "who"]
            .contains(&words[0].to_ascii_lowercase().as_str())
        && words.iter().all(|word| {
            let word = word.trim_end_matches(['?', '.', ',', ':']);
            word.split('-')
                .all(|part| !part.is_empty() && part.chars().all(char::is_alphabetic))
        })
}

/// Shared by lexical selection and semantic admission. An active policy owns
/// auto routing; explicit graph requests retain their ordinary anchor rules.
pub(crate) fn prose_context_query(query: &str, strategy: Strategy, has_policy: bool) -> bool {
    prose_query(query)
        && match strategy {
            Strategy::Search => true,
            Strategy::Auto => {
                !has_policy && response::strategy_for_query(query) == Strategy::Search
            }
            Strategy::Graph => false,
        }
}

/// Unmarked prose compounds and language qualifiers (e.g. "a Go string")
/// should not resolve to an unrelated capitalized function. Explicit call/name
/// requests and marked/shaped identifiers retain ordinary anchor precedence.
/// A qualifier never filters admitted roots. Experimental prose ranking may
/// prefer its language, while retaining documentation and other-language hits.
pub(crate) fn incidental_prose_anchor(query: &str, anchor: &AnchorCandidate) -> bool {
    if anchor.group != 3 {
        return false;
    }
    let before = query.get(..anchor.position).unwrap_or_default();
    let after = anchor
        .position
        .checked_add(anchor.text.len())
        .and_then(|end| query.get(end..))
        .unwrap_or_default();
    if before.ends_with('-') || after.starts_with('-') {
        return true;
    }
    if ![
        "Go",
        "Rust",
        "Python",
        "Java",
        "Javascript",
        "Typescript",
        "JavaScript",
        "TypeScript",
        "Ruby",
        "Swift",
        "Kotlin",
        "Unicode",
    ]
    .contains(&anchor.text.as_str())
        || before.split_whitespace().any(|word| {
            ["call", "invoke", "named", "symbol", "function", "method"]
                .contains(&word.to_ascii_lowercase().as_str())
        })
    {
        return false;
    }
    after.split_whitespace().next().is_some_and(|word| {
        [
            "string",
            "slice",
            "map",
            "array",
            "boolean",
            "bytes",
            "channel",
            "code",
            "source",
            "package",
            "module",
            "path",
            "reader",
            "whitespace",
            "character",
            "characters",
            "text",
        ]
        .contains(&word.trim_end_matches(['?', '.', ',', ':']))
    })
}

/// Only an unambiguous, explicitly capitalized language followed by a source
/// noun supplies a preference. Ordinary English ("go through"), symbol calls,
/// and cross-language questions do not. This uses query text, never corpus labels.
fn prose_language(query: &str) -> Option<&'static str> {
    let words: Vec<_> = query.split_whitespace().collect();
    if words.iter().any(|word| {
        ["call", "invoke", "named", "symbol", "function", "method"]
            .contains(&word.to_ascii_lowercase().as_str())
    }) {
        return None;
    }
    let mut language = None;
    for pair in words.windows(2) {
        let candidate = match pair[0] {
            "Go" => "go",
            "Rust" => "rust",
            "Python" => "python",
            "Java" => "java",
            "Javascript" | "JavaScript" => "javascript",
            "Typescript" | "TypeScript" => "typescript",
            "Ruby" => "ruby",
            "Swift" => "swift",
            "Kotlin" => "kotlin",
            _ => continue,
        };
        if ![
            "string", "slice", "map", "array", "boolean", "bytes", "channel", "code", "source",
            "package", "module", "path", "reader",
        ]
        .contains(&pair[1].trim_end_matches(['?', '.', ',', ':']))
        {
            continue;
        }
        if language.is_some_and(|old| old != candidate) {
            return None;
        }
        language = Some(candidate);
    }
    language
}

/// Query-only prose normalization; stored identifiers and explicit code requests
/// retain their original tokenizer and schema.
fn prose_concepts(query: &str) -> Vec<Vec<String>> {
    query
        .split_whitespace()
        .flat_map(|word| {
            let word = word.trim_end_matches(['?', '.', ',', ':']);
            // CamelCase analysis splits "CPUs" as "CP"/"Us". In prose an
            // uppercase acronym followed only by plural s names the acronym;
            // ordinary/explicit identifier requests retain the indexed analysis.
            if let Some(stem) = word
                .strip_suffix('s')
                .filter(|stem| stem.len() >= 2 && stem.bytes().all(|b| b.is_ascii_uppercase()))
            {
                // Both cpu and cpus occur in ordinary source. They express
                // one query concept, not two separately rewarded matches.
                return vec![vec![stem.to_ascii_lowercase(), word.to_ascii_lowercase()]];
            }
            analyzed_terms(crate::syntax::code_subtokens, word)
                .into_iter()
                .map(|word| vec![word])
                .collect()
        })
        .collect()
}

fn prose_terms(query: &str) -> Vec<String> {
    prose_concepts(query).into_iter().flatten().collect()
}

/// Distinct concept coverage dominates frequency/rarity for prose. Adjacent
/// concepts within 16 positions break ties, with a total bonus below one term.
/// This uses the existing positional index; no expansion corpus or extra scan.
fn prose_lexical(query: &str, body: Field) -> Box<dyn Query> {
    let mut words: Vec<Vec<String>> = Vec::new();
    let language = prose_language(query);
    for concept in prose_concepts(query) {
        // A requested language is a metadata preference, not a required
        // behavioral concept. Rewarding its incidental appearance in long
        // comments can bury the implementation whose comments omit it.
        if language == Some(concept[0].as_str()) || prose_stopword(&concept[0]) {
            continue;
        }
        if let Some(previous) = words
            .iter_mut()
            .find(|previous| concept.iter().any(|word| previous.contains(word)))
        {
            for word in concept {
                if !previous.contains(&word) {
                    previous.push(word);
                }
            }
            continue;
        }
        words.push(concept);
        if words.len() == 64 {
            break;
        }
    }
    let mut clauses: Vec<Box<dyn Query>> = words
        .iter()
        .map(|concept| {
            Box::new(ConstScoreQuery::new(
                Box::new(BooleanQuery::union(
                    concept
                        .iter()
                        .map(|word| {
                            Box::new(TermQuery::new(
                                Term::from_field_text(body, word),
                                IndexRecordOption::Basic,
                            )) as Box<dyn Query>
                        })
                        .collect(),
                )),
                1.0,
            )) as Box<dyn Query>
        })
        .collect();
    let bonus = 0.5 / words.len().max(1) as f32;
    for pair in words.windows(2) {
        let mut phrases = Vec::new();
        for left in &pair[0] {
            for right in &pair[1] {
                let mut phrase = PhraseQuery::new(vec![
                    Term::from_field_text(body, left),
                    Term::from_field_text(body, right),
                ]);
                phrase.set_slop(16);
                phrases.push(Box::new(phrase) as Box<dyn Query>);
            }
        }
        clauses.push(Box::new(ConstScoreQuery::new(
            Box::new(BooleanQuery::union(phrases)),
            bonus,
        )));
    }
    Box::new(BooleanQuery::union(clauses))
}

/// The two-tier collection of one query, before revalidation.
struct TwoTier {
    prose: bool,
    first: Vec<Candidate>,
    second: Vec<Candidate>,
    /// A tier window filled.
    candidates_full: bool,
    /// The anchors' windows in anchor order; empty for a query without
    /// anchors.
    windows: Vec<WindowSlice>,
}

/// The anchor windows of one store from its tier-1 candidates: each
/// window's first [`ANCHOR_LIST`] current definitions, materialized from the
/// verified sources the caller's final read loads (`current` holds every
/// tier-1 path's metadata). Stale definitions are skipped here: the tier-1
/// pass already counted them.
#[allow(clippy::too_many_arguments)]
fn anchor_windows<C: ReadableTable<&'static str, &'static str>>(
    windows: &[WindowSlice],
    first: &[Candidate],
    current: &std::collections::BTreeMap<String, Option<SourceMeta>>,
    stored: &C,
    verified: &mut std::collections::BTreeMap<String, VerifiedSource>,
    workspace_id: &str,
    wanted: &std::collections::BTreeSet<String>,
) -> FResult<Vec<AnchorWindow>> {
    let mut out = Vec::with_capacity(windows.len());
    for window in windows {
        let mut entries = Vec::new();
        for candidate in &first[window.range.clone()] {
            if entries.len() == ANCHOR_LIST {
                break;
            }
            let Some(Some(meta)) = current.get(&candidate.path) else {
                continue;
            };
            if meta.hash != candidate.hash {
                continue;
            }
            if !verified.contains_key(&candidate.path) {
                let source = reconstruct_verified(stored, &candidate.path, meta)?;
                verified.insert(candidate.path.clone(), source);
            }
            let body = &verified[&candidate.path].body;
            let rank = entries.len();
            entries.push(ranked_item(
                candidate,
                rank,
                workspace_id,
                &meta.hash,
                body,
                wanted,
            )?);
        }
        out.push(AnchorWindow {
            anchor: window.anchor.clone(),
            order: window.order,
            definitions: window.definitions,
            entries,
        });
    }
    Ok(out)
}

/// One approximate-door candidate (context-v2 § Doors): a delivery unit
/// whose `ident` holds the definition's name, with its door-order keys.
struct DoorCandidate {
    /// Its file's import keys hold the name or the definition's module.
    importing: bool,
    role: u64,
    path: String,
    hash: String,
    unit_start: u64,
    unit_end: u64,
    /// `<kind> <qualified name>`, or the kind alone.
    label: String,
}

/// One verified source an approximate-doors pass reads, with its line starts.
struct DoorFile {
    hash: String,
    body: String,
    line_starts: Vec<usize>,
}

impl DoorFile {
    fn new(hash: String, body: String) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(body.match_indices('\n').map(|(at, _)| at + 1));
        Self {
            hash,
            body,
            line_starts,
        }
    }

    /// One-based line of the byte `offset`.
    fn line_of(&self, offset: usize) -> u64 {
        self.line_starts.partition_point(|&start| start <= offset) as u64
    }

    /// The text of one-based `line`, without its LF or CRLF terminator.
    fn line_text(&self, line: u64) -> &str {
        let index = (line as usize).saturating_sub(1);
        let start = self
            .line_starts
            .get(index)
            .copied()
            .unwrap_or(self.body.len());
        let end = self
            .line_starts
            .get(index + 1)
            .copied()
            .unwrap_or(self.body.len());
        let text = &self.body[start..end];
        let text = text.strip_suffix('\n').unwrap_or(text);
        text.strip_suffix('\r').unwrap_or(text)
    }
}

/// `path`'s verified bytes, loaded into `files` once in the caller's read
/// transaction; `None` when the source is absent or no longer carries
/// `hash`.
fn door_file<'f>(
    files: &'f mut std::collections::HashMap<String, Option<DoorFile>>,
    sources: &redb::ReadOnlyTable<&'static str, &'static str>,
    stored: &redb::ReadOnlyTable<&'static str, &'static str>,
    path: &str,
    hash: &str,
) -> FResult<Option<&'f DoorFile>> {
    if !files.contains_key(path) {
        let file = match sources.get(path)? {
            Some(raw) => {
                let meta = decode::<SourceMeta>(raw.value(), "source")?;
                let body = reconstruct_verified(stored, path, &meta)?.body;
                Some(DoorFile::new(meta.hash, body))
            }
            None => None,
        };
        files.insert(path.to_owned(), file);
    }
    Ok(files[path].as_ref().filter(|file| file.hash == hash))
}

/// The last segment of a definition's module (context-v2 § Doors): its file
/// stem, or its directory for an `index`, `mod`, `lib` or `__init__` stem and
/// for every Go file (its package).
fn module_key(path: &str) -> Option<&str> {
    let (directory, file) = match path.rsplit_once('/') {
        Some((directory, file)) => (Some(directory), file),
        None => (None, path),
    };
    let stem = Path::new(file).file_stem()?.to_str()?;
    if path.ends_with(".go") || matches!(stem, "index" | "mod" | "lib" | "__init__") {
        directory.map(|directory| directory.rsplit('/').next().unwrap_or(directory))
    } else {
        Some(stem)
    }
}

/// The byte offset of the first occurrence of `name` in `text` exactly as
/// written and as a whole identifier: no identifier character (alphanumeric,
/// `_` or `$`) touches it on either side.
fn find_word(text: &str, name: &str) -> Option<usize> {
    let identifier = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(name).find_map(|(at, _)| {
        let before = text[..at].chars().next_back().is_some_and(identifier);
        let after = text[at + name.len()..]
            .chars()
            .next()
            .is_some_and(identifier);
        (!before && !after).then_some(at)
    })
}

/// Write schema last in the initializing transaction.
fn initialize_tables(tx: &WriteTransaction) -> FResult<()> {
    tx.open_table(SOURCES)?;
    tx.open_table(CHUNKS)?;
    tx.open_table(PENDING)?;
    tx.open_table(MEMORY)?;
    tx.open_table(crate::task_context::TASKS)?;
    tx.open_table(BRIEFS)?;
    tx.open_table(FEEDBACK)?;
    tx.open_table(SEEN)?;
    crate::graph::init(tx)?;
    crate::boundary::init(tx)?;
    crate::graph::init_compiler(tx)?;
    crate::neural::cache::init_tables(tx)?;
    crate::neural::work::init(tx)?;
    crate::neural::mapping_store::init(tx)?;
    crate::knowledge::cache::init(tx)?;
    crate::learning::init_tables(tx)?;
    Ok(())
}

impl Engine {
    /// The parse panics of the latest [`Self::refresh`], each a named scan
    /// failure of the index run that drained it (context-v2 § Parallel
    /// indexing). A page's panics are recorded once its search commit
    /// succeeded; taking them leaves none.
    pub fn take_parse_failures(&mut self) -> ParseFailures {
        std::mem::take(&mut self.parse_panics)
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
    /// render them: locator lines never carry a selection tag, so a
    /// dense-only hit (009 T004: a placed card's unit with no lexical
    /// candidate) names its unit's handle under the `semantic` label.
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

    /// Candidate source paths across the admitted search index. Only the first
    /// document of each source has these exact syntax-name postings, so comments
    /// and repeated calls within a file cannot drown out other files. This reads
    /// at most nine stored index documents and no source bodies. Final source
    /// hash/AST validation belongs to the task's final read snapshot.
    pub(crate) fn task_call_sources(
        &self,
        names: &[String],
        control: &crate::Control,
    ) -> FResult<(Vec<SourceHandle>, bool)> {
        control.check()?;
        if names.is_empty() {
            return Ok((Vec::new(), false));
        }
        let handles = self.require_search()?;
        let fields = &handles.fields;
        let searcher = handles.reader.searcher();
        let clauses: Vec<Box<dyn Query>> = names
            .iter()
            .take(4)
            .map(|name| {
                Box::new(TermQuery::new(
                    Term::from_field_text(fields.call_name, name),
                    IndexRecordOption::Basic,
                )) as Box<dyn Query>
            })
            .collect();
        let window = searcher.search(
            &BooleanQuery::union(clauses),
            &TopDocs::with_limit(9).order_by_u64_field("key_hash", tantivy::Order::Asc),
        )?;
        control.check()?;
        let mut limited = names.len() > 4 || window.len() > 8;
        let workspace_id = self.workspace_id().ok_or(FoundryError::WorkspaceUnbound)?;
        let mut sources = Vec::new();
        for (_, address) in window.into_iter().take(8) {
            control.check()?;
            let doc: TantivyDocument = searcher.doc(address)?;
            let invalid =
                || FoundryError::CorruptStore("invalid call-source search document".into());
            let text = |field| doc.get_first(field).and_then(|value| value.as_str());
            let path = text(fields.path).ok_or_else(invalid)?;
            let hash = text(fields.hash).ok_or_else(invalid)?;
            limited |= text(fields.call_coverage) != Some("complete");
            sources.push(SourceHandle {
                workspace_id: workspace_id.clone(),
                path: path.to_owned(),
                sha256: hash.to_owned(),
                // This is an index identity receipt, never a requested body span.
                start: 0,
                end: 0,
            });
        }
        Ok((sources, limited))
    }

    /// Whether this store defines `name` exactly as written, under tier 1's
    /// restriction (`path` as the search's path filter): the group-3
    /// admission probe of context-v2 § Anchors and qualifiers, which a
    /// multi-root owner asks of every serving root before any root builds a
    /// window (007). It counts and keeps nothing.
    pub fn defines_exact_case(&self, name: &str, path: Option<&str>) -> FResult<bool> {
        let filter = path.map(path_filter).transpose()?;
        let handles = self.require_search()?;
        exact_case_defined(
            &handles.reader.searcher(),
            &handles.fields,
            filter.as_deref(),
            name,
        )
    }

    /// [`Self::search_candidates_with`], the anchors chosen from this
    /// store's own index.
    pub fn search_candidates(
        &self,
        query: &str,
        path: Option<&str>,
        limit: usize,
        control: &crate::Control,
    ) -> FResult<CandidateBatch> {
        self.search_candidates_with(query, path, limit, control, None)
    }

    /// Two-tier candidate selection for one store (context-v2 § Two-tier
    /// query, § Resolver order, § Hit materialization). Tier 1 is the
    /// anchors' resolver windows in anchor order or, without anchors, exact
    /// definitions of the tier-1 runs (at most 64, the most specific run
    /// first, smallest `key_hash` kept per run, ordered by path and start
    /// within it); tier 2 is lexical (at most 256 by score, `key_hash`
    /// breaking cutoff ties). `path` restricts both tiers to a file or
    /// directory subtree. Candidates are revalidated in one final read
    /// transaction, merged per delivery unit, capped at 4 per file and cut to
    /// `limit`; each anchor window's first [`ANCHOR_LIST`] definitions are
    /// materialized beside them, without that cap or cut. `anchors` are the
    /// ones a multi-root owner chose over every serving root (007); `None`
    /// chooses them from this store's index.
    pub fn search_candidates_with(
        &self,
        query: &str,
        path: Option<&str>,
        limit: usize,
        control: &crate::Control,
        anchors: Option<&[AnchorCandidate]>,
    ) -> FResult<CandidateBatch> {
        self.search_candidates_selected(
            query,
            path,
            limit,
            control,
            SearchSelection {
                anchors,
                prose: false,
            },
        )
    }

    fn search_candidates_selected(
        &self,
        query: &str,
        path: Option<&str>,
        limit: usize,
        control: &crate::Control,
        selection: SearchSelection<'_>,
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
        let TwoTier {
            prose,
            first,
            second,
            candidates_full,
            windows,
        } = self.collect_two_tier(query, path, control, selection)?;

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
        let mut seen = LexicalSelection::default();
        let mut per_file: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        let mut kept: Vec<&Candidate> = Vec::new();
        for (rank, candidate) in first.iter().chain(&second).enumerate() {
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
                candidate.trace(control, &workspace_id, rank, "stale");
                continue;
            }
            if let Err(outcome) = seen.insert(candidate, prose) {
                candidate.trace(control, &workspace_id, rank, outcome);
                continue;
            }
            let count = per_file.entry(candidate.path.clone()).or_default();
            if *count == PER_FILE_CAP {
                counters.capped += 1;
                candidate.trace(control, &workspace_id, rank, "per_file_cap");
                continue;
            }
            *count += 1;
            if kept.len() == limit {
                counters.truncated = true;
                candidate.trace(control, &workspace_id, rank, "limit");
                continue;
            }
            candidate.trace(control, &workspace_id, rank, "kept");
            kept.push(candidate);
        }
        let wanted: std::collections::BTreeSet<String> = if prose {
            prose_terms(query)
        } else {
            analyzed_terms(crate::syntax::code_subtokens, query)
        }
        .into_iter()
        .collect();
        let mut verified: std::collections::BTreeMap<String, VerifiedSource> =
            std::collections::BTreeMap::new();
        let mut items = Vec::with_capacity(kept.len());
        let mut delivered = std::collections::BTreeSet::new();
        for (rank, candidate) in kept.into_iter().enumerate() {
            let Some(Some(meta)) = current.get(&candidate.path) else {
                continue;
            };
            if !verified.contains_key(&candidate.path) {
                let source = reconstruct_verified(&stored, &candidate.path, meta)?;
                verified.insert(candidate.path.clone(), source);
            }
            let body = &verified[&candidate.path].body;
            let item = ranked_context_item(
                candidate,
                rank,
                &workspace_id,
                &meta.hash,
                body,
                &wanted,
                prose,
            )?;
            candidate.trace_materialization(control, &item);
            let handle = item.handle.as_ref().expect("source candidate");
            if delivered.insert((handle.path.clone(), handle.start, handle.end)) {
                items.push(item);
            }
        }
        let anchors = anchor_windows(
            &windows,
            &first,
            &current,
            &stored,
            &mut verified,
            &workspace_id,
            &wanted,
        )?;
        let freshness = self.freshness_in(&tx)?;
        Ok(CandidateBatch {
            freshness,
            items,
            counters,
            semantic: None,
            route: None,
            anchors,
            doors: None,
            collected: None,
        })
    }

    /// The two-tier candidate collection shared by the baseline and the
    /// semantic paths (context-v2 § Two-tier query, § Anchors and
    /// qualifiers, § Resolver order): tier 1 is the anchors' windows in
    /// anchor order (each the best 64 of every matching definition by the
    /// resolver tuple, then `key_hash`, listed by the tuple, then path and
    /// start) or, for a query without anchors, the exact definitions of its
    /// tier-1 runs (at most 64; runs by specificity, each contributing its
    /// smallest-`key_hash` definitions to the slots left, ordered by path and
    /// start within the run); tier 2 is lexical (at most 256 by score,
    /// `key_hash` breaking cutoff ties, tier-1 units removed). The anchors are
    /// `anchors` when a multi-root owner chose them over every serving root
    /// (007), otherwise this index's own choice: each capitalized candidate
    /// costs one exact-case probe, and only admitted anchors get windows.
    /// Also returns whether a window filled and each anchor's window range.
    /// No source is read here; validation happens in the callers' final read.
    fn collect_two_tier(
        &self,
        query: &str,
        path: Option<&str>,
        control: &crate::Control,
        selection: SearchSelection<'_>,
    ) -> FResult<TwoTier> {
        let filter = path.map(path_filter).transpose()?;
        let handles = self.require_search()?;
        let fields = &handles.fields;
        let restrict = |query: Box<dyn Query>| tier_restricted(fields, filter.as_deref(), query);
        let term = |field: Field, text: &str, option: IndexRecordOption| -> Box<dyn Query> {
            Box::new(TermQuery::new(Term::from_field_text(field, text), option))
        };
        let searcher = handles.reader.searcher();
        // Tier 1 groups in order, each its documents (with their resolver
        // tuples in an anchor's window) and, for a window, its anchor.
        struct Tier1Group {
            anchor: Option<(String, (u8, usize), u64)>,
            documents: Vec<(tantivy::DocAddress, Option<Resolver>)>,
        }
        let mut tier1: Vec<Tier1Group> = Vec::new();
        let mut tier1_full = false;
        let parsed = QueryAnchors::parse(query);
        let qualifiers: Vec<u64> = parsed.qualifiers.iter().map(|q| hash64(q)).collect();
        let mut anchors = match selection.anchors {
            Some(chosen) => chosen.to_vec(),
            None => parsed.select(|candidate| {
                control.check()?;
                exact_case_defined(&searcher, fields, filter.as_deref(), &candidate.text)
            })?,
        };
        if selection.prose && prose_query(query) {
            anchors.retain(|anchor| !incidental_prose_anchor(query, anchor));
        }
        let prose = selection.prose && anchors.is_empty() && prose_query(query);
        if prose && let Some(trace) = control.trace() {
            trace.record(serde_json::json!({"stage":"lexical_intent", "intent":"anchorless_prose", "exact_run_promotion":false, "scoring":"distinct_terms_proximity_v1", "term_limit":64}));
        }
        for anchor in anchors {
            // The anchor's window (context-v2 § Resolver order): every
            // matching definition scored under tier 1's own restriction.
            control.check()?;
            let fruit = searcher.search(
                restrict(term(
                    fields.def_name,
                    &anchor.text.to_lowercase(),
                    IndexRecordOption::Basic,
                ))
                .as_ref(),
                &ResolverCollector {
                    qualifiers: qualifiers.clone(),
                    name_case: hash64(&anchor.text),
                },
            )?;
            tier1_full |= fruit.definitions > TIER1_LIMIT as u64;
            let order = (anchor.group, anchor.position);
            let documents = fruit
                .window
                .iter()
                .map(|scored| {
                    // The name range is read with the document below.
                    let resolver = Resolver {
                        anchor: order,
                        qualifiers: scored.qualifiers,
                        exact: scored.exact,
                        role: scored.role,
                        name: (0, 0),
                    };
                    (scored.address, Some(resolver))
                })
                .collect();
            tier1.push(Tier1Group {
                anchor: Some((anchor.text, order, fruit.definitions)),
                documents,
            });
        }
        if tier1.is_empty() && !prose {
            // Without anchors the 2026-10-06 rule stands: one exact count and
            // the 64 smallest-`key_hash` definitions per run, under the same
            // restriction as the documents it keeps.
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
            for run in tier1_runs(query, &code_spans(query)) {
                let (count, top) = searcher.search(
                    restrict(term(fields.def_name, &run, IndexRecordOption::Basic)).as_ref(),
                    &definitions,
                )?;
                if count > 0 {
                    let top = top.into_iter().map(|(_, address)| address).collect();
                    runs.push((count, run, top));
                }
            }
            // Specificity: fewer definitions first, then run text. Each run
            // fills the slots left; a document already kept stays under its
            // earlier run.
            runs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            let defined: usize = runs.iter().map(|(count, ..)| count).sum();
            let mut kept: Vec<tantivy::DocAddress> = Vec::new();
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
                tier1.push(Tier1Group {
                    anchor: None,
                    documents: group.into_iter().map(|address| (address, None)).collect(),
                });
            }
            tier1_full = kept.len() == TIER1_LIMIT && defined > TIER1_LIMIT;
        }
        let mut clauses: Vec<Box<dyn Query>> = Vec::new();
        for part in query.split_whitespace() {
            if prose && prose_stopword(part.trim_end_matches(['?', '.', ',', ':'])) {
                continue;
            }
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
            if prose && prose_stopword(ident) {
                continue;
            }
            clauses.push(Box::new(BoostQuery::new(
                term(fields.ident, ident, IndexRecordOption::WithFreqs),
                3.0,
            )));
        }
        clauses.push(Box::new(BoostQuery::new(
            term(fields.path, query.trim(), IndexRecordOption::Basic),
            100.0,
        )));
        let per_lane = if prose {
            CANDIDATE_LIMIT / 2
        } else {
            CANDIDATE_LIMIT
        };
        let collector = TopDocs::with_limit(per_lane).tweak_score(|reader: &SegmentReader| {
            let key_hash = reader
                .fast_fields()
                .u64("key_hash")
                .expect("schema v2 fast field")
                .first_or_default_col(0);
            move |doc: DocId, score: Score| (score, Reverse(key_hash.get_val(doc)))
        });
        let lexical: Box<dyn Query> = if prose {
            let lexical = prose_lexical(query, fields.body);
            if let Some(language) = prose_language(query) {
                if let Some(trace) = control.trace() {
                    trace.record(serde_json::json!({"stage":"lexical_language","language":language,"policy":"prefer_explicit_language_v1","filters_roots":false}));
                }
                // Distinct-term scores are bounded by 64.5. Prefer matching
                // language before spending the fixed discovery window; adding
                // this after TopDocs would leave relevant files undiscovered.
                // The lexical Must keeps unrelated files out; Should preserves
                // fallback languages and the separate documentation window.
                Box::new(BooleanQuery::new(vec![
                    (Occur::Must, lexical),
                    (
                        Occur::Should,
                        Box::new(ConstScoreQuery::new(
                            term(fields.lang, language, IndexRecordOption::Basic),
                            65.0,
                        )),
                    ),
                ]))
            } else {
                lexical
            }
        } else {
            Box::new(BooleanQuery::union(clauses))
        };
        let mut full = false;
        let mut tier2 = Vec::new();
        let mut collect = |query: Box<dyn Query>| -> FResult<()> {
            let found = searcher.search(restrict(query).as_ref(), &collector)?;
            full |= found.len() == per_lane;
            tier2.extend(
                found
                    .into_iter()
                    .map(|((score, _), address)| (score, address)),
            );
            Ok(())
        };
        if prose {
            // Disjoint windows keep large documentation sets from exhausting
            // code discovery, and preserve documentation discovery in code-heavy
            // roots. Their combined cap is still 256 documents.
            let code: Box<dyn Query> = Box::new(BooleanQuery::new(vec![
                (Occur::Must, Box::new(tantivy::query::AllQuery)),
                (
                    Occur::MustNot,
                    term(fields.kind, "section", IndexRecordOption::Basic),
                ),
                (
                    Occur::MustNot,
                    term(fields.kind, "block", IndexRecordOption::Basic),
                ),
                (
                    Occur::MustNot,
                    term(fields.kind, UNPARSED_KIND, IndexRecordOption::Basic),
                ),
            ]));
            collect(Box::new(BooleanQuery::new(vec![
                (Occur::Must, lexical.box_clone()),
                (Occur::Must, code.box_clone()),
            ])))?;
            collect(Box::new(BooleanQuery::new(vec![
                (Occur::Must, lexical),
                (Occur::MustNot, code),
            ])))?;
        } else {
            collect(lexical)?;
        }
        let candidates_full = tier1_full || full;
        control.check()?;

        // A resolver-window document is a definition document: it carries
        // the stored name range its resolver records.
        let read =
            |tier: u8, score: Score, address, resolver: Option<Resolver>| -> FResult<Candidate> {
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
                let resolver = match resolver {
                    Some(resolver) => Some(Resolver {
                        name: (number(fields.name_start)?, number(fields.name_end)?),
                        ..resolver
                    }),
                    None => None,
                };
                Ok(Candidate {
                    tier,
                    score,
                    path: text(fields.path).ok_or_else(invalid)?,
                    hash: text(fields.hash).ok_or_else(invalid)?,
                    start: number(fields.start)?,
                    end: number(fields.end)?,
                    unit_start: number(fields.unit_start)?,
                    unit_head: number(fields.unit_head)?,
                    unit_end: number(fields.unit_end)?,
                    // A parse-fallback document is a block to every reader.
                    kind: text(fields.kind)
                        .map(|kind| match kind.as_str() {
                            UNPARSED_KIND => crate::syntax::UnitKind::Block.as_str().to_owned(),
                            _ => kind,
                        })
                        .ok_or_else(invalid)?,
                    lang: text(fields.lang),
                    qname: text(fields.qname),
                    resolver,
                })
            };
        // Tier 1: each anchor's window by the resolver tuple, then path and
        // start (today's tier-1 listing within a run); without anchors each
        // run's group by path and start.
        let mut first: Vec<Candidate> = Vec::new();
        let mut windows: Vec<WindowSlice> = Vec::new();
        for group in tier1 {
            let mut run: Vec<Candidate> = group
                .documents
                .into_iter()
                .map(|(address, resolver)| read(1, 0.0, address, resolver))
                .collect::<FResult<_>>()?;
            run.sort_by(|a, b| {
                let key = |candidate: &Candidate| candidate.resolver.map(|r| r.key());
                key(a)
                    .cmp(&key(b))
                    .then_with(|| a.path.cmp(&b.path))
                    .then(a.start.cmp(&b.start))
            });
            let range = first.len()..first.len() + run.len();
            first.extend(run);
            if let Some((anchor, order, definitions)) = group.anchor {
                windows.push(WindowSlice {
                    anchor,
                    order,
                    definitions,
                    range,
                });
            }
        }
        let units: std::collections::BTreeSet<_> = first.iter().map(Candidate::unit).collect();
        let mut second: Vec<Candidate> = Vec::new();
        for (score, address) in tier2 {
            let candidate = read(2, score, address, None)?;
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
        if prose {
            let (code, prose): (Vec<_>, Vec<_>) = second
                .into_iter()
                .partition(|c| !["section", "block", UNPARSED_KIND].contains(&c.kind.as_str()));
            second = Vec::with_capacity(code.len() + prose.len());
            let mut code = code.into_iter();
            let mut prose = prose.into_iter();
            loop {
                let (a, b) = (code.next(), prose.next());
                if a.is_none() && b.is_none() {
                    break;
                }
                second.extend(a);
                second.extend(b);
            }
        }
        Ok(TwoTier {
            prose,
            first,
            second,
            candidates_full,
            windows,
        })
    }

    /// The 009 T004 placement (spec 009 § T004 "Placement"), for a query
    /// without an anchor: the dense window's units first, in similarity
    /// order, then tier 1 and tier 2 in the baseline's order.
    ///
    /// Everything is validated in ONE final read BEFORE placement. A lexical
    /// document whose source changed is dropped and counted stale, so it can
    /// neither suppress nor coalesce with a current dense unit of the same
    /// span. A dense hit expands through the serving generation's own label
    /// map (no partition walk): each location must satisfy the request's
    /// path filter, then is dropped and counted stale unless its recorded
    /// source hash is the current one and its range lies inside the source.
    /// The placed list is then merged per unit (a unit appears once, at its
    /// first place), capped per file and cut to `limit` like the baseline.
    /// A dense unit renders through the ordinary ladder, tagged semantic;
    /// when a lexical candidate shares it, it keeps that candidate's label.
    /// The coverage word is `ready` only for a complete generation published
    /// at this read's source revision; otherwise `partial`. `anchors` as in
    /// [`Self::search_candidates_with`].
    #[cfg(feature = "semantic")]
    pub fn search_candidates_semantic(
        &self,
        query: &str,
        path: Option<&str>,
        limit: usize,
        control: &crate::Control,
        dense: &crate::neural::query::DenseWindow,
        anchors: Option<&[AnchorCandidate]>,
    ) -> FResult<CandidateBatch> {
        self.search_candidates_semantic_selected(
            query,
            path,
            limit,
            control,
            dense,
            SearchSelection {
                anchors,
                prose: false,
            },
        )
    }

    #[cfg(feature = "semantic")]
    fn search_candidates_semantic_selected(
        &self,
        query: &str,
        path: Option<&str>,
        limit: usize,
        control: &crate::Control,
        dense: &crate::neural::query::DenseWindow,
        selection: SearchSelection<'_>,
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
        // requirements, as in the baseline path.
        let filter = path.map(path_filter).transpose()?;
        let workspace_id = self.require_workspace_id()?;
        let TwoTier {
            prose,
            first,
            second,
            candidates_full,
            windows,
        } = self.collect_two_tier(query, path, control, selection)?;

        // Bound ordinary provenance once, before discovery or final-read
        // materialization. Shared vector labels can name arbitrarily many
        // sources; output caps alone do not bound that metadata expansion.
        let mut ordinary = dense
            .bounded_units(filter.as_deref(), 4096, control)
            .map_err(|error| FoundryError::Semantic {
                code: error.code,
                message: error.detail,
            })?;
        if let Some(trace) = control.trace() {
            trace.record(
                serde_json::json!({"stage":"dense_provenance", "available":ordinary.available,
                "examined":ordinary.items.len(), "limit":4096, "truncated":ordinary.truncated}),
            );
        }

        // A coarse hit discovers a file, not its answer passage. Reuse the
        // query vector to localize within a bounded union of discovered files.
        // This reads published vectors only; missing refinement is coverage
        // debt for the explicit preparation owner, never query-time inference.
        let mut refinement_paths = std::collections::BTreeSet::new();
        let mut refinement_limited = false;
        let refined = if prose && dense.has_overviews() && !dense.file_overviews_only() {
            let mut examined = 0;
            for (_, unit) in &ordinary.items {
                control.check()?;
                if refinement_paths.len() == 64 {
                    break;
                }
                examined += 1;
                refinement_paths.insert(unit.path.clone());
            }
            if let Some(trace) = control.trace() {
                trace.record(
                    serde_json::json!({"stage":"refinement_discovery", "examined":examined,
                    "files":refinement_paths.len(), "truncated":examined < ordinary.items.len()}),
                );
            }
            refinement_limited |= examined < ordinary.items.len();
            use crate::neural::query::{OwnerProof, OwnerScope};
            let mut refinement_scopes =
                std::collections::BTreeMap::<String, Vec<OwnerScope<'_>>>::new();
            for candidate in first.iter().chain(&second).take(16) {
                control.check()?;
                refinement_paths.insert(candidate.path.clone());
                if matches!(candidate.kind.as_str(), "fn" | "method") {
                    refinement_scopes
                        .entry(candidate.path.clone())
                        .or_default()
                        .push(OwnerScope {
                            source_hash: &candidate.hash,
                            start: candidate.unit_start,
                            end: candidate.unit_end,
                            proof: OwnerProof::Callable {
                                kind: &candidate.kind,
                                head: candidate.unit_head,
                                document: (candidate.start, candidate.end),
                            },
                        });
                }
            }
            // Discovery keeps its original first64 dense files and first16
            // lexical documents. Collect Parent scopes from the same bounded
            // provenance window, including already admitted files after64.
            // Body/File/ContainerFragment ranges cannot invent an owner.
            for (rank, unit) in &ordinary.items {
                control.check()?;
                if unit.kind == crate::neural::cache::CardKind::Parent
                    && refinement_paths.contains(&unit.path)
                {
                    refinement_scopes
                        .entry(unit.path.clone())
                        .or_default()
                        .push(OwnerScope {
                            source_hash: &unit.source_sha256,
                            start: unit.start,
                            end: unit.end,
                            proof: OwnerProof::Parent {
                                input_key: dense.input_key(&dense.hits[*rank]),
                            },
                        });
                }
            }
            match dense.refine(self, &refinement_paths, &refinement_scopes, control) {
                Ok(refinement) => {
                    refinement_limited |= refinement.stats.truncated;
                    if let Some(trace) = control.trace() {
                        trace.record(serde_json::json!({"stage":"refinement", "outcome":"scored", "method":"published_body_exact", "localization":"current_owner_narrowing_v1", "work":refinement.stats}));
                    }
                    refinement.hits
                }
                Err(error) => {
                    if let Some(trace) = control.trace() {
                        trace.record(serde_json::json!({"stage":"refinement", "outcome":"unavailable", "reason":error.code}));
                    }
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        control.check()?;

        // Final read: every candidate is validated here, before placement.
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let mut counters = CandidateCounters {
            candidates_full: candidates_full || ordinary.truncated || refinement_limited,
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
        let wanted: std::collections::BTreeSet<String> = if prose {
            prose_terms(query)
        } else {
            analyzed_terms(crate::syntax::code_subtokens, query)
        }
        .into_iter()
        .collect();
        let mut verified: std::collections::BTreeMap<String, VerifiedSource> =
            std::collections::BTreeMap::new();
        // The anchor windows, from the tier-1 documents as collected; their
        // stale definitions are counted with tier 1 below.
        for candidate in &first {
            load(&mut current, &candidate.path)?;
        }
        let anchors = anchor_windows(
            &windows,
            &first,
            &current,
            &stored,
            &mut verified,
            &workspace_id,
            &wanted,
        )?;
        // Lexical documents of the current source version only.
        let mut lexical: Vec<Candidate> = Vec::with_capacity(first.len() + second.len());
        let mut lexical_selection = LexicalSelection::default();
        for (rank, candidate) in first.into_iter().chain(second).enumerate() {
            load(&mut current, &candidate.path)?;
            if current[&candidate.path]
                .as_ref()
                .is_some_and(|meta| meta.hash == candidate.hash)
            {
                if let Err(outcome) = lexical_selection.insert(&candidate, prose) {
                    candidate.trace(control, &workspace_id, rank, outcome);
                    continue;
                }
                candidate.trace(control, &workspace_id, rank, "eligible");
                lexical.push(candidate);
            } else {
                candidate.trace(control, &workspace_id, rank, "stale");
                counters.stale += 1;
            }
        }
        // Dense locations in similarity order: the path restriction first,
        // then freshness.
        let mut placed = Vec::new();
        let refined_count = refined.len();
        let mut eligible_refined = 0;
        // Preserve similarity order after fair admission, using only the
        // ordinary locations already admitted and the exact refined joins.
        ordinary.items.sort_by_key(|(rank, _)| *rank);
        let refined_locations = refined.iter().enumerate().flat_map(|(rank, refined)| {
            refined
                .locations
                .iter()
                .copied()
                .map(move |location| (rank, &refined.hit, location, true))
        });
        let ordinary_locations = ordinary
            .items
            .iter()
            .map(|(rank, location)| (refined_count + rank, &dense.hits[*rank], *location, false));
        for (rank, hit, location, is_refined) in refined_locations.chain(ordinary_locations) {
            control.check()?;
            let trace_location = |outcome: &str| {
                if let Some(trace) = control.trace() {
                    trace.record(serde_json::json!({
                            "stage": "selection", "origin": "dense", "rank": rank,
                            "distance": hit.distance, "input_key": dense.input_key(hit), "outcome": outcome,
                            "representation": location.kind, "refined":is_refined,
                            "unit": {"workspace_id": workspace_id, "path": location.path,
                                "sha256": location.source_sha256, "start": location.start,
                                "end": location.end},
                        }));
                }
            };
            if filter
                .as_deref()
                .is_some_and(|filter| !within_filter(&location.path, filter))
            {
                trace_location("path_filter");
                continue;
            }
            load(&mut current, &location.path)?;
            let current_location = current[location.path.as_str()]
                .as_ref()
                .is_some_and(|meta| {
                    meta.hash == location.source_sha256
                        && location.end <= meta.bytes as u64
                        && (location.kind != crate::neural::cache::CardKind::File
                            || (location.start == 0 && location.end == meta.bytes as u64))
                });
            if !current_location {
                trace_location("stale");
                counters.stale += 1;
                continue;
            }
            trace_location("eligible");
            eligible_refined += usize::from(is_refined);
            placed.push((
                (location.path.clone(), location.start, location.end),
                Some(location.kind),
            ));
        }
        let freshness = self.freshness_in(&tx)?;
        let word = dense.coverage_at(freshness.source_revision);
        // Coverage reports availability, not whether dense relevance outranks
        // lexical evidence. Experimental body/hierarchy placement uses the same
        // bounded hybrid order before and after preparation completes. Keep the
        // existing partial/address and coarse-only admission rules; complete
        // address coverage and ordinary search retain their placement.
        // Freshness was checked above; no candidate/output slots are added.
        if prose
            && (word == "partial"
                || dense.file_overviews_only()
                || dense.has_body_representations())
        {
            let mut dense_order = std::mem::take(&mut placed).into_iter();
            let mut lexical_order = lexical.iter().map(|c| (c.selection_unit(prose), None));
            loop {
                let (lexical, dense) = (lexical_order.next(), dense_order.next());
                if lexical.is_none() && dense.is_none() {
                    break;
                }
                // Independently embedded body passages precede broad lexical
                // documents. Without eligible refinement, lexical evidence
                // goes first. Neither choice depends on coverage completeness.
                if eligible_refined > 0 {
                    placed.extend(dense);
                    placed.extend(lexical);
                } else {
                    placed.extend(lexical);
                    placed.extend(dense);
                }
            }
            if let Some(trace) = control.trace() {
                let policy = if dense.has_body_representations() {
                    "body_prose_interleave_v1"
                } else if dense.file_overviews_only() {
                    "coarse_prose_interleave_v1"
                } else {
                    "partial_prose_interleave_v1"
                };
                trace.record(serde_json::json!({"stage":"placement_policy","policy":policy,"coverage":word,"lexical_first":eligible_refined == 0,"refined_hits":refined_count,"eligible_refined_locations":eligible_refined,"refinement_files":refinement_paths.len()}));
            }
        } else {
            placed.extend(
                lexical
                    .iter()
                    .map(|candidate| (candidate.selection_unit(prose), None)),
            );
        }
        // The candidate carrying each lexical unit: its first (best-ranked)
        // current document, as in the baseline.
        let mut by_unit: std::collections::HashMap<(String, u64, u64), &Candidate> =
            std::collections::HashMap::new();
        let mut by_document = std::collections::HashMap::new();
        for candidate in &lexical {
            by_unit.entry(candidate.unit()).or_insert(candidate);
            by_document
                .entry(candidate.selection_unit(prose))
                .or_insert(candidate);
        }

        // The baseline's merge per unit, per-file cap and cut, over the
        // placed order.
        let mut seen = std::collections::BTreeSet::new();
        let mut per_file: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        let mut kept = Vec::new();
        for (rank, (unit, kind)) in placed.iter().enumerate() {
            let trace_placement = |outcome: &str| {
                if let Some(trace) = control.trace() {
                    trace.record(serde_json::json!({
                        "stage": "placement", "origin": if kind.is_some() { "dense" } else { "lexical" },
                        "rank": rank, "outcome": outcome,
                        "unit": {"workspace_id": workspace_id, "path": unit.0,
                            "sha256": current.get(&unit.0).and_then(|m| m.as_ref()).map(|m| &m.hash),
                            "start": unit.1, "end": unit.2},
                    }));
                }
            };
            if !seen.insert(unit) {
                trace_placement("duplicate");
                continue;
            }
            let count = per_file.entry(unit.0.clone()).or_default();
            if *count == PER_FILE_CAP {
                counters.capped += 1;
                trace_placement("per_file_cap");
                continue;
            }
            *count += 1;
            if kept.len() == limit {
                counters.truncated = true;
                trace_placement("limit");
                continue;
            }
            trace_placement("kept");
            kept.push((unit, *kind));
        }
        let mut items = Vec::with_capacity(kept.len());
        let mut delivered = std::collections::BTreeSet::new();
        for (rank, (unit, kind)) in kept.into_iter().enumerate() {
            control.check()?;
            let from_dense = kind.is_some();
            let exact_body = kind == Some(crate::neural::cache::CardKind::Body);
            let exact_span = matches!(
                kind,
                Some(
                    crate::neural::cache::CardKind::Body
                        | crate::neural::cache::CardKind::ContainerFragment
                )
            );
            let (unit_path, unit_start, unit_end) = unit;
            let Some(Some(meta)) = current.get(unit_path) else {
                continue;
            };
            if !verified.contains_key(unit_path) {
                let source = reconstruct_verified(&stored, unit_path, meta)?;
                verified.insert(unit_path.clone(), source);
            }
            let body = &verified[unit_path].body;
            let candidate = if from_dense {
                // An exact unit match may contribute its label and support.
                // A scored Body still renders its own unchanged byte range.
                by_unit.get(unit)
            } else {
                by_document.get(unit)
            }
            .copied();
            let mut item = match candidate {
                Some(candidate) if exact_span => {
                    ranked_item(candidate, rank, &workspace_id, &meta.hash, body, &wanted)?
                }
                Some(candidate) => ranked_context_item(
                    candidate,
                    rank,
                    &workspace_id,
                    &meta.hash,
                    body,
                    &wanted,
                    prose && (!from_dense || dense.has_overviews()),
                )?,
                None => {
                    // A dense-only unit: no delivery-unit label.
                    let (from, to) = (*unit_start as usize, *unit_end as usize);
                    if !(from < to
                        && to <= body.len()
                        && body.is_char_boundary(from)
                        && body.is_char_boundary(to))
                    {
                        return Err(FoundryError::CorruptStore(format!(
                            "candidate unit outside {unit_path}"
                        )));
                    }
                    let (from, to, focused) =
                        if prose && !exact_span && dense.has_overviews() && to - from > 1536 {
                            prose_passage(&body[from..to], &wanted)
                                .map_or((from, to, false), |(start, end)| {
                                    (from + start, from + end, true)
                                })
                        } else {
                            (from, to, false)
                        };
                    let text = &body[from..to];
                    let newlines = |bytes: &[u8]| bytes.iter().filter(|&&b| b == b'\n').count();
                    let start_line = newlines(&body.as_bytes()[..from]) as u64 + 1;
                    let end_line = start_line + newlines(&text.as_bytes()[..text.len() - 1]) as u64;
                    RankedItem {
                        tier: 2,
                        rank,
                        score: 0.0,
                        handle: Some(SourceHandle {
                            workspace_id: workspace_id.clone(),
                            path: unit_path.clone(),
                            sha256: meta.hash.clone(),
                            start: from as u64,
                            end: to as u64,
                        }),
                        start_line,
                        end_line,
                        line: start_line
                            + if focused {
                                best_line_index(text, &wanted)
                            } else {
                                0
                            },
                        label: if focused {
                            "excerpt".into()
                        } else {
                            String::new()
                        },
                        lang: crate::syntax::Lang::from_path(unit_path)
                            .map(|lang| lang.tag().to_owned()),
                        semantic: None,
                        resolver: None,
                        forms: vec![RenderedForm::Verbatim(text.to_owned())],
                    }
                }
            };
            if from_dense {
                item.semantic = Some(SemanticEvidence {
                    dense_only: candidate.is_none(),
                });
                if let Some(trace) = control.trace() {
                    let focused = item
                        .handle
                        .as_ref()
                        .is_some_and(|h| (h.start, h.end) != (*unit_start, *unit_end));
                    trace.record(serde_json::json!({
                        "stage":"materialization", "origin":"dense", "rank":rank,
                        "unit":{"workspace_id":workspace_id,"path":unit_path,"sha256":meta.hash,
                            "start":unit_start,"end":unit_end},
                        "excerpt":item.handle,
                        "local_query_focus":focused,
                        "representation":kind,
                        "passage_independently_embedded":exact_body,
                    }));
                }
            } else if let Some(candidate) = candidate {
                candidate.trace_materialization(control, &item);
            }
            let handle = item.handle.as_ref().expect("source candidate");
            if delivered.insert((handle.path.clone(), handle.start, handle.end)) {
                items.push(item);
            }
        }
        // The word describes THIS read (a context recomputes it against its
        // own later final read).
        Ok(CandidateBatch {
            freshness,
            items,
            counters,
            semantic: Some(word.to_owned()),
            route: None,
            anchors,
            doors: None,
            collected: None,
        })
    }

    /// Derived memory documents matching `query` (008 memory search): the
    /// tier-2 lexical clauses over `body`/`ident`, restricted to
    /// `kind:"memory"`, at most `limit` by (score, key_hash). Callers
    /// validate each candidate against the live row before delivery.
    pub(crate) fn memory_plan(&self, query: &str) -> FResult<RecordPlan> {
        self.record_plan(query, "memory")
    }

    pub(crate) fn record_plan(&self, query: &str, kind: &str) -> FResult<RecordPlan> {
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
                    Term::from_field_text(fields.kind, kind),
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
                .ok_or_else(|| FoundryError::CorruptStore("invalid record document".into()))?;
            let Some(id) = key.strip_prefix(&format!("{kind}:")) else {
                return Err(FoundryError::CorruptStore(format!(
                    "{kind} query matched another kind of document {key:?}"
                )));
            };
            let revision = doc
                .get_first(fields.hash)
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<u64>().ok())
                .ok_or_else(|| {
                    FoundryError::CorruptStore(format!("{kind} document {key:?} has no revision"))
                })?;
            out.push(RecordCandidate {
                id: id.to_owned(),
                revision,
            });
        }
        Ok(RecordPlan {
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

    /// The doors of a context that requested them (context-v2 § Doors),
    /// built in its final read `tx` for `target`, decided from the first
    /// anchor's window before this read. A target this read's validation of
    /// the anchor windows dropped as stale gets no doors, and no other
    /// definition takes its place: a resolved `D` gives `none`.
    ///
    /// Each surviving target reads one references window, exactly the
    /// resolved anchor's (at most [`TIE_GROUP`] per request), with a
    /// cancellation check before every window after the first. Exact doors
    /// when a current compiler scope of a target's path holds definition
    /// occurrences at its stored name range. A resolved `D` without them
    /// gets approximate doors; a tie group's entry without them gets no
    /// group, and only when no entry has exact doors does the name get
    /// approximate doors once, attributed to no entry. A malformed compiler
    /// row is component-local: it gives approximate doors, never a failed
    /// context.
    fn doors_in(
        &self,
        tx: &redb::ReadTransaction,
        target: &DoorTarget,
        anchors: &[AnchorWindow],
        counters: &mut CandidateCounters,
        control: &crate::Control,
    ) -> FResult<Doors> {
        let (targets, cap) = match target {
            DoorTarget::None => return Ok(Doors::unbuilt(DoorState::None)),
            DoorTarget::Ambiguous => return Ok(Doors::unbuilt(DoorState::Ambiguous)),
            DoorTarget::Resolved(handle) => (std::slice::from_ref(handle), DOOR_FILES),
            DoorTarget::Tied(handles) => (handles.as_slice(), GROUP_FILES),
        };
        // Validation only removes entries: a target survived exactly when it
        // is still in the first anchor's window.
        let surviving = |handle: &SourceHandle| {
            anchors
                .first()?
                .entries
                .iter()
                .find(|entry| entry.handle.as_ref() == Some(handle))?
                .resolver
        };
        let revision = self.freshness_in(tx)?.source_revision;
        let bound = self.workspace_id.clone().unwrap_or_default();
        let mut reads = graph::DoorReads::new(tx, revision, &bound);
        let mut groups: Vec<DoorGroup> = Vec::new();
        // The first surviving target names the approximate doors.
        let mut named: Option<(&SourceHandle, Resolver)> = None;
        for handle in targets {
            let Some(resolver) = surviving(handle) else {
                continue;
            };
            match named {
                // Every window after the first checks the request's
                // cancellation.
                Some(_) => control.check()?,
                None => named = Some((handle, resolver)),
            }
            match reads.exact_doors(&handle.path, &handle.sha256, resolver.name, cap) {
                Ok(graph::ExactResolution::Doors(exact)) => {
                    counters.stale += exact.stale as u64;
                    counters.candidates_full |= exact.full;
                    groups.push(DoorGroup {
                        target: Some(handle.clone()),
                        lines: exact.lines,
                        more_files: exact.more_files,
                    });
                }
                // A name scan that ran out of its allowance filled the
                // window, whatever the approximate window does.
                Ok(graph::ExactResolution::Approximate { exhausted }) => {
                    counters.candidates_full |= exhausted;
                }
                Err(FoundryError::GraphInvalid(_)) => {}
                Err(other) => return Err(other),
            }
        }
        let resolved = matches!(target, DoorTarget::Resolved(_));
        if !groups.is_empty() {
            let state = if resolved {
                DoorState::Exact
            } else {
                DoorState::Each
            };
            return Ok(Doors { state, groups });
        }
        let Some(group) = named
            .map(|(handle, resolver)| {
                self.approx_doors(tx, handle, resolver.name, targets, counters, control)
            })
            .transpose()?
            .flatten()
        else {
            return Ok(Doors::unbuilt(DoorState::None));
        };
        Ok(Doors {
            state: DoorState::Approx,
            groups: vec![DoorGroup {
                target: resolved.then(|| targets[0].clone()),
                ..group
            }],
        })
    }

    /// Approximate doors (context-v2 § Doors) for the name exactly as
    /// written at `name` in `named`'s verified bytes, without a trailing
    /// `?`, `!` or `'`: the units of `own` are excluded and their module keys
    /// are import keys. One group, onto no target; `None` when `named`'s
    /// source is gone from this read.
    fn approx_doors(
        &self,
        tx: &redb::ReadTransaction,
        named: &SourceHandle,
        name: (u64, u64),
        own: &[SourceHandle],
        counters: &mut CandidateCounters,
        control: &crate::Control,
    ) -> FResult<Option<DoorGroup>> {
        let bound = self.workspace_id.clone().unwrap_or_default();
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let mut files: std::collections::HashMap<String, Option<DoorFile>> =
            std::collections::HashMap::new();
        let Some(file) = door_file(&mut files, &sources, &stored, &named.path, &named.sha256)?
        else {
            return Ok(None);
        };
        let (start, end) = (name.0 as usize, name.1 as usize);
        let name = file
            .body
            .get(start..end)
            .ok_or_else(|| {
                FoundryError::CorruptStore(format!(
                    "a stored name range lies outside {}",
                    named.path
                ))
            })?
            .trim_end_matches(['?', '!', '\''])
            .to_owned();
        let mut doors = DoorGroup {
            target: None,
            lines: Vec::new(),
            more_files: 0,
        };
        // Identifiers of one character have no doors.
        if name.chars().count() < 2 {
            return Ok(Some(doors));
        }
        let (candidates, full) = self.door_candidates(&name, own, control)?;
        counters.candidates_full |= full;
        // A candidate becomes a door where a line of it holds the name
        // exactly as written; one site per unit, grouped by file in door
        // order (a file's first candidate is its first site).
        let mut groups: Vec<(DoorLine, usize)> = Vec::new();
        let mut group_of: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for candidate in &candidates {
            control.check()?;
            let Some(file) = door_file(
                &mut files,
                &sources,
                &stored,
                &candidate.path,
                &candidate.hash,
            )?
            else {
                counters.stale += 1;
                continue;
            };
            let (start, end) = (candidate.unit_start as usize, candidate.unit_end as usize);
            let Some(text) = file.body.get(start..end) else {
                return Err(FoundryError::CorruptStore(format!(
                    "search document unit outside {}",
                    candidate.path
                )));
            };
            let Some(at) = find_word(text, &name) else {
                continue;
            };
            match group_of.get(&candidate.path) {
                Some(&index) => groups[index].1 += 1,
                None => {
                    group_of.insert(candidate.path.clone(), groups.len());
                    let line = file.line_of(start + at);
                    groups.push((
                        DoorLine {
                            unit: SourceHandle {
                                workspace_id: bound.clone(),
                                path: candidate.path.clone(),
                                sha256: candidate.hash.clone(),
                                start: candidate.unit_start,
                                end: candidate.unit_end,
                            },
                            line,
                            label: candidate.label.clone(),
                            text: file.line_text(line).to_owned(),
                            more: 0,
                        },
                        0,
                    ));
                }
            }
        }
        doors.more_files = groups.len().saturating_sub(DOOR_FILES);
        doors.lines = groups
            .into_iter()
            .take(DOOR_FILES)
            .map(|(line, more)| DoorLine { more, ..line })
            .collect();
        Ok(Some(doors))
    }

    /// The approximate-door candidates of the definitions `own`, all named
    /// `name` (context-v2 § Doors): the delivery units, other than theirs,
    /// whose `ident` holds the name, in door order - importing files first (a
    /// file whose index-time import keys hold the name or one of their
    /// module keys), then role, path and start - cut to the first
    /// [`DOOR_WINDOW`]; and whether more existed. The request's cancellation
    /// is checked as each search document is read. Nothing is resolved, read
    /// from configuration or executed: the keys were computed at indexing.
    fn door_candidates(
        &self,
        name: &str,
        own: &[SourceHandle],
        control: &crate::Control,
    ) -> FResult<(Vec<DoorCandidate>, bool)> {
        let handles = self.require_search()?;
        let fields = &handles.fields;
        let searcher = handles.reader.searcher();
        let term = |field: Field, text: &str| -> Box<dyn Query> {
            Box::new(TermQuery::new(
                Term::from_field_text(field, text),
                IndexRecordOption::Basic,
            ))
        };
        let mut modules: Vec<&str> = vec![name];
        for module in own.iter().filter_map(|handle| module_key(&handle.path)) {
            if !modules.contains(&module) {
                modules.push(module);
            }
        }
        let keys = modules
            .into_iter()
            .map(|key| term(fields.imports, key))
            .collect();
        let mut importing: std::collections::HashSet<String> = std::collections::HashSet::new();
        for address in searcher.search(
            &BooleanQuery::union(keys),
            &tantivy::collector::DocSetCollector,
        )? {
            control.check()?;
            let doc: TantivyDocument = searcher.doc(address)?;
            if let Some(path) = doc.get_first(fields.path).and_then(|v| v.as_str()) {
                importing.insert(path.to_owned());
            }
        }
        let mentions = BooleanQuery::new(vec![
            (Occur::Must, term(fields.ident, &name.to_lowercase())),
            (Occur::MustNot, term(fields.kind, "memory")),
            (Occur::MustNot, term(fields.kind, "brief")),
        ]);
        let mut units: std::collections::BTreeMap<(String, u64, u64), DoorCandidate> =
            std::collections::BTreeMap::new();
        for address in searcher.search(&mentions, &tantivy::collector::DocSetCollector)? {
            control.check()?;
            let doc: TantivyDocument = searcher.doc(address)?;
            let invalid = || FoundryError::CorruptStore("invalid search document".into());
            let text = |field| doc.get_first(field).and_then(|v| v.as_str());
            let number = |field| {
                doc.get_first(field)
                    .and_then(|v| v.as_u64())
                    .ok_or_else(invalid)
            };
            let path = text(fields.path).ok_or_else(invalid)?;
            let (unit_start, unit_end) = (number(fields.unit_start)?, number(fields.unit_end)?);
            let own_unit = own.iter().any(|handle| {
                handle.path == path && handle.start == unit_start && handle.end == unit_end
            });
            if own_unit {
                continue;
            }
            let key = (path.to_owned(), unit_start, unit_end);
            if units.contains_key(&key) {
                continue;
            }
            let kind = match text(fields.kind).ok_or_else(invalid)? {
                UNPARSED_KIND => crate::syntax::UnitKind::Block.as_str(),
                kind => kind,
            };
            let label = match text(fields.qname) {
                Some(qname) => format!("{kind} {qname}"),
                None => kind.to_owned(),
            };
            units.insert(
                key,
                DoorCandidate {
                    importing: importing.contains(path),
                    role: number(fields.role)?,
                    path: path.to_owned(),
                    hash: text(fields.hash).ok_or_else(invalid)?.to_owned(),
                    unit_start,
                    unit_end,
                    label,
                },
            );
        }
        let mut candidates: Vec<DoorCandidate> = units.into_values().collect();
        candidates.sort_by(|a, b| {
            (!a.importing, a.role, &a.path, a.unit_start).cmp(&(
                !b.importing,
                b.role,
                &b.path,
                b.unit_start,
            ))
        });
        let full = candidates.len() > DOOR_WINDOW;
        candidates.truncate(DOOR_WINDOW);
        Ok((candidates, full))
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

pub(crate) fn revision_in(tx: &WriteTransaction) -> FResult<u64> {
    read_counter(&tx.open_table(META)?, "source_revision")
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

#[cfg(all(test, feature = "test-faults"))]
mod batch_tests {
    use super::*;
    use crate::fault::{self, Action, names};

    #[test]
    fn four_prepared_sources_roll_back_together_on_failure_and_cancellation() {
        for cancel in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("root");
            std::fs::create_dir(&root).unwrap();
            let store = dir.path().join("store");
            let engine = Engine::initialize(&store, &root).unwrap();
            let batch = |body: &str| {
                (0..4)
                    .map(|n| PreparedSource::new(format!("batch{n}.rs"), body.into()).unwrap())
                    .collect::<Vec<_>>()
            };
            engine
                .replace_prepared_sources(&batch("fn old() {}\n"), &crate::Control::unbounded())
                .unwrap();
            drop(engine);
            let before = crate::testkit::snapshot(&store);
            let engine = Engine::open_existing(&store).unwrap();
            fault::disarm_all();
            fault::arm(
                names::SOURCE_BATCH_BEFORE_COMMIT,
                0,
                if cancel {
                    Action::Cancel
                } else {
                    Action::Fail("injected batch failure".into())
                },
            );
            assert!(
                engine
                    .replace_prepared_sources(&batch("fn new() {}\n"), &crate::Control::unbounded())
                    .is_err()
            );
            assert_eq!(fault::reached(names::SOURCE_BATCH_BEFORE_COMMIT), 1);
            fault::disarm_all();
            drop(engine);
            assert_eq!(before, crate::testkit::snapshot(&store));
        }
    }
}
