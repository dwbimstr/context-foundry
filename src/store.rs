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
mod doors;
mod lexical;
mod lifecycle;
mod placement;
mod retrieve;
#[cfg(feature = "semantic")]
mod semantic_search;
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
    /// A source fallback from the same verified file. Its coordinates belong
    /// to these exact bytes, not to the preferred document's wider handle.
    Excerpt {
        start: u64,
        start_line: u64,
        body: String,
    },
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
    /// Fields drop in declaration order. Keep the authoritative ownership lock
    /// until the search writer (including its worker shutdown) and the other
    /// engine resources are gone. Releasing it first lets a new owner acquire
    /// redb while Tantivy is still locked and falsely report repair_required.
    pub(crate) db: Database,
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
struct SplitTokenizer {
    split: fn(&str) -> Vec<(usize, usize)>,
    token: Token,
}

impl SplitTokenizer {
    fn new(split: fn(&str) -> Vec<(usize, usize)>) -> Self {
        SplitTokenizer {
            split,
            token: Token::default(),
        }
    }
}

/// The tokens of one text, streamed through the tokenizer's reused `Token`.
struct SplitTokens<'a> {
    text: &'a str,
    ranges: std::vec::IntoIter<(usize, usize)>,
    token: &'a mut Token,
}

impl Tokenizer for SplitTokenizer {
    type TokenStream<'a> = SplitTokens<'a>;
    fn token_stream<'a>(&'a mut self, text: &'a str) -> SplitTokens<'a> {
        self.token.reset();
        SplitTokens {
            text,
            ranges: (self.split)(text).into_iter(),
            token: &mut self.token,
        }
    }
}

impl TokenStream for SplitTokens<'_> {
    fn advance(&mut self) -> bool {
        let text = self.text;
        let Some((from, to)) = self
            .ranges
            .find(|&(from, to)| text[from..to].chars().nth(1).is_some())
        else {
            return false;
        };
        let piece = &text[from..to];
        self.token.offset_from = from;
        self.token.offset_to = to;
        // `reset` sets `usize::MAX`: the first token is at position 0.
        self.token.position = self.token.position.wrapping_add(1);
        self.token.text.clear();
        // Equal to `str::to_lowercase`, which owns the final-sigma rule.
        if piece.is_ascii() {
            self.token.text.push_str(piece);
            self.token.text.make_ascii_lowercase();
        } else {
            self.token.text.push_str(&piece.to_lowercase());
        }
        true
    }
    fn token(&self) -> &Token {
        self.token
    }
    fn token_mut(&mut self) -> &mut Token {
        self.token
    }
}

/// The query-side terms of one analysis, identical to the indexed tokens.
fn analyzed_terms(split: fn(&str) -> Vec<(usize, usize)>, text: &str) -> Vec<String> {
    let mut tokenizer = SplitTokenizer::new(split);
    let mut tokens = tokenizer.token_stream(text);
    let mut terms = Vec::new();
    while tokens.advance() {
        terms.push(tokens.token().text.clone());
    }
    terms
}

#[cfg(test)]
mod split_tests {
    use super::*;
    use crate::syntax::{code_subtokens, identifier_runs};

    #[test]
    fn streamed_tokens_are_the_lowercased_pieces_of_at_least_two_chars_in_order() {
        for split in [code_subtokens, identifier_runs] {
            let mut tokenizer = SplitTokenizer::new(split);
            for text in [
                "HTTPServer2x ΟΔΟΣ snake_case é a1 Ⅻ",
                "",
                "İSTANBUL naïveCafé ΣΑΣ $id_9 x",
            ] {
                let expected: Vec<Token> = split(text)
                    .into_iter()
                    .filter(|&(from, to)| text[from..to].chars().count() >= 2)
                    .enumerate()
                    .map(|(position, (from, to))| Token {
                        offset_from: from,
                        offset_to: to,
                        position,
                        text: text[from..to].to_lowercase(),
                        position_length: 1,
                    })
                    .collect();
                let mut tokens = tokenizer.token_stream(text);
                let mut streamed = Vec::new();
                while tokens.advance() {
                    streamed.push(tokens.token().clone());
                }
                assert_eq!(streamed, expected, "{text:?}");
                let terms: Vec<String> = expected.into_iter().map(|token| token.text).collect();
                assert_eq!(analyzed_terms(split, text), terms, "{text:?}");
            }
        }
    }
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
        SplitTokenizer::new(crate::syntax::identifier_runs),
    );
    tokenizers.register(
        "foundry_code",
        SplitTokenizer::new(crate::syntax::code_subtokens),
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

/// The 0-based index of the line of `text` holding the most distinct query
/// subtokens, the earliest on ties.
fn best_line_index(text: &str, wanted: &std::collections::BTreeSet<String>) -> u64 {
    let mut tokenizer = SplitTokenizer::new(crate::syntax::code_subtokens);
    // The distinct wanted terms of one line.
    let mut matched: Vec<&str> = Vec::new();
    let mut best = (0usize, 0u64);
    for (index, line) in text.split_inclusive('\n').enumerate() {
        matched.clear();
        let mut tokens = tokenizer.token_stream(line);
        while tokens.advance() {
            if let Some(term) = wanted.get(tokens.token().text.as_str())
                && !matched.contains(&term.as_str())
            {
                matched.push(term);
            }
        }
        if matched.len() > best.0 {
            best = (matched.len(), index as u64);
        }
    }
    best.1
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
    fn unit(&self) -> placement::UnitRef<'_> {
        (self.path.as_str(), self.unit_start, self.unit_end)
    }

    fn selection_unit(&self, prose: bool) -> placement::UnitRef<'_> {
        if prose && self.tier == 2 && self.unit_end.saturating_sub(self.unit_start) > 1536 {
            (
                self.path.as_str(),
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
struct LexicalSelection<'a> {
    spans: std::collections::BTreeMap<placement::UnitRef<'a>, Vec<(u64, u64)>>,
}

impl<'a> LexicalSelection<'a> {
    fn insert(&mut self, candidate: &'a Candidate, prose: bool) -> Result<(), &'static str> {
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
    let document_start = start;
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
    let mut focused = ranked_item(&excerpt, rank, workspace_id, sha256, body, &wanted)?;
    if start != document_start {
        // A docs-only match redirected to the callable. Keep that existing
        // declaration-first behavior instead of preferring the long comment.
        focused.label = format!("excerpt {}", focused.label);
        return Ok(focused);
    }
    // Preserve the matching document until delivery knows the token budget.
    // An irreversible 1,536-byte cut here discarded relevant evidence even
    // when the complete, bounded index document would have fit the response.
    let mut document = candidate.clone();
    document.unit_start = candidate.start.max(candidate.unit_start);
    document.unit_end = candidate.end.min(candidate.unit_end);
    document.unit_head = document.unit_start;
    let mut item = ranked_item(&document, rank, workspace_id, sha256, body, &wanted)?;
    item.label = format!("excerpt {}", item.label);
    if document.unit_start != excerpt.unit_start || document.unit_end != excerpt.unit_end {
        item.forms.push(RenderedForm::Excerpt {
            start: excerpt.unit_start,
            start_line: focused.start_line,
            body: body[excerpt.unit_start as usize..excerpt.unit_end as usize].to_owned(),
        });
    }
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

/// The terms that pick a materialized item's best line and passage: a prose
/// query's concepts, otherwise its indexed analysis.
fn wanted_terms(query: &str, prose: bool) -> std::collections::BTreeSet<String> {
    if prose {
        prose_concepts(query).into_iter().flatten().collect()
    } else {
        analyzed_terms(crate::syntax::code_subtokens, query)
            .into_iter()
            .collect()
    }
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

/// The request checks both searches share, in their error precedence: the
/// query, the limit, then the path filter (before the workspace and index
/// requirements). Returns the normalized filter.
fn check_search(query: &str, path: Option<&str>, limit: usize) -> FResult<Option<String>> {
    if query.trim().is_empty() || query.len() > 4096 {
        return Err(FoundryError::InvalidArgument(
            "query must contain 1..4096 nonblank bytes".into(),
        ));
    }
    if !(1..=64).contains(&limit) {
        return Err(FoundryError::InvalidArgument("limit must be 1..64".into()));
    }
    path.map(path_filter).transpose()
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

/// The sources of one final read: each path's metadata read once and each
/// current path's body reconstructed and verified once, on first use.
struct FinalSources {
    sources: redb::ReadOnlyTable<&'static str, &'static str>,
    stored: redb::ReadOnlyTable<&'static str, &'static str>,
    metas: std::collections::BTreeMap<String, Option<SourceMeta>>,
    verified: std::collections::BTreeMap<String, String>,
}

impl FinalSources {
    fn open(tx: &redb::ReadTransaction) -> FResult<Self> {
        Ok(Self {
            sources: tx.open_table(SOURCES)?,
            stored: tx.open_table(CHUNKS)?,
            metas: std::collections::BTreeMap::new(),
            verified: std::collections::BTreeMap::new(),
        })
    }

    /// `path`'s metadata in this read; `None` for a removed source.
    fn meta(&mut self, path: &str) -> FResult<Option<&SourceMeta>> {
        Self::load(&self.sources, &mut self.metas, path)
    }

    /// Whether `path`'s source in this read is the version hashed `hash`.
    fn is_current(&mut self, path: &str, hash: &str) -> FResult<bool> {
        Ok(self.meta(path)?.is_some_and(|meta| meta.hash == hash))
    }

    /// `path`'s metadata as already read; never reads.
    fn loaded(&self, path: &str) -> Option<&SourceMeta> {
        self.metas.get(path).and_then(Option::as_ref)
    }

    /// `path`'s metadata and verified body in this read; `None` for a
    /// removed source.
    fn body(&mut self, path: &str) -> FResult<Option<(&SourceMeta, &str)>> {
        let Some(meta) = Self::load(&self.sources, &mut self.metas, path)? else {
            return Ok(None);
        };
        if !self.verified.contains_key(path) {
            let source = reconstruct_verified(&self.stored, path, meta)?;
            self.verified.insert(path.to_owned(), source.body);
        }
        Ok(Some((meta, self.verified[path].as_str())))
    }

    /// The bodies verified so far, by path.
    fn verified_bodies(&self) -> impl Iterator<Item = (&str, &str)> {
        self.verified
            .iter()
            .map(|(path, body)| (path.as_str(), body.as_str()))
    }

    /// `path`'s metadata and body as already verified; never reads.
    fn verified_body(&self, path: &str) -> Option<(&SourceMeta, &str)> {
        Some((self.loaded(path)?, self.verified.get(path)?.as_str()))
    }

    fn load<'a>(
        sources: &redb::ReadOnlyTable<&'static str, &'static str>,
        metas: &'a mut std::collections::BTreeMap<String, Option<SourceMeta>>,
        path: &str,
    ) -> FResult<Option<&'a SourceMeta>> {
        if !metas.contains_key(path) {
            let meta = match sources.get(path)? {
                Some(raw) => Some(decode::<SourceMeta>(raw.value(), "source")?),
                None => None,
            };
            metas.insert(path.to_owned(), meta);
        }
        Ok(metas[path].as_ref())
    }
}

/// The anchor windows of one store from its tier-1 candidates: each
/// window's first [`ANCHOR_LIST`] current definitions, materialized from the
/// final read's verified `sources`. Stale definitions are skipped here: the
/// tier-1 pass counts them.
fn anchor_windows(
    windows: &[WindowSlice],
    first: &[Candidate],
    sources: &mut FinalSources,
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
            if !sources.is_current(&candidate.path, &candidate.hash)? {
                continue;
            }
            let Some((meta, body)) = sources.body(&candidate.path)? else {
                continue;
            };
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
