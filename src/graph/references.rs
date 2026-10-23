//! The budgeted references query (spec 005) and exact doors (context-v2
//! § Doors): seeds, eligible scopes, verified file views, the reference and
//! target windows and the door reads. The edge graph, the compiler facts and
//! their storage key codec stay in the parent module.
use super::*;

// ---------------------------------------------------------------------------
// Budgeted references query (spec 005 § Publication, queries and limits).
// ---------------------------------------------------------------------------

/// Default and maximum `limit` of one references request.
pub const REFERENCES_DEFAULT_LIMIT: usize = 64;
pub const REFERENCES_MAX_LIMIT: usize = 256;
/// The shared examination allowance: seed resolution, definition lookup and
/// reference records all draw from it.
pub const REFERENCES_MAX_EXAMINED: usize = 256;
/// Distinct eligible origin files one window may read.
pub const REFERENCES_MAX_FILES: usize = 64;
/// Definition candidates a query names.
pub const DEFINITION_CANDIDATES: usize = 8;
/// A position seed's containing-occurrence scan leaves the last two records
/// of the shared allowance to the definition lookup and to at least one
/// record of reference work, so a uniquely resolved target can always make
/// progress.
const SEED_SCAN_LIMIT: usize = REFERENCES_MAX_EXAMINED - 2;
/// The definition lookup never consumes the last record of the shared
/// allowance: that record stays reserved for reference work. A lookup that
/// cannot conclude within its share leaves the target `Unfinished` (partial,
/// never certified), but the reference loop still delivers the next
/// reference with a cursor that continues losslessly.
const DEFINITION_SCAN_LIMIT: usize = REFERENCES_MAX_EXAMINED - 1;

/// Exactly one seed form: a 16-hex symbol id prefix, a source handle plus an
/// absolute byte offset within it, or a handle alone - the symbols of the
/// handle's definition unit by the exact-doors rule (context-v2 § Doors).
#[derive(Clone, Debug)]
pub enum ReferencesSeed {
    SymbolId(String),
    Position { handle: String, byte_offset: u64 },
    Handle(String),
}

#[derive(Clone, Debug)]
pub struct ReferencesRequest {
    pub seed: ReferencesSeed,
    /// 1..=256 reference lines.
    pub limit: usize,
    /// `<path>#<start>`: continue strictly after that reference position.
    pub after: Option<String>,
}

/// Coverage of the answer, for the producer that owns the seed symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Coverage {
    /// Selected snapshot current, fully consumed, and the seed resolved to a
    /// unique definition: complete for supported indexed input, inside the
    /// examined window.
    Complete,
    /// Current but incomplete: the import is unfinished or failed documents,
    /// or the seed has unresolved/ambiguous targets, or some examined record
    /// belongs to another snapshot.
    Partial,
    /// A source changed after the selected snapshot's revision: every
    /// compiler fact is ineligible.
    Stale,
    /// No compiler snapshot is selected.
    Unavailable,
}

impl Coverage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Stale => "stale",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Whether the seed symbol's definition was resolved inside the allowance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetResolution {
    /// Exactly one eligible definition was found and no other exists.
    Unique,
    /// Two or more eligible definitions.
    Ambiguous,
    /// No eligible definition: external or unknown.
    Unknown,
    /// The examination allowance ran out before uniqueness was known.
    Unfinished,
}

#[derive(Clone, Debug, Serialize)]
pub struct DefinitionCandidate {
    pub path: String,
    pub sha256: String,
    pub start: u64,
    pub end: u64,
}

/// One reference occurrence with its enclosing delivery unit.
#[derive(Clone, Debug, Serialize)]
pub struct ReferenceItem {
    pub occurrence_id: String,
    pub path: String,
    pub sha256: String,
    pub start: u64,
    pub end: u64,
    /// One-based line of `start`.
    pub line: u64,
    /// The enclosing delivery unit (full identities).
    pub unit: SourceHandle,
    /// `<kind> <qualified name>`, or the kind alone for a block or an
    /// unnamed unit.
    pub label: String,
    /// Only for a uniquely resolved target.
    pub edge_id: Option<String>,
}

impl ReferenceItem {
    /// The `after` cursor that continues strictly after this reference.
    pub fn cursor(&self) -> String {
        format!("{}#{}-{}", self.path, self.start, self.end)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ReferencesOutcome {
    pub freshness: Freshness,
    pub producer: Option<String>,
    /// The selected snapshot tuple the answer was judged against: artifact
    /// digest, manifest digest, source revision and producer/config identity.
    pub snapshot: Option<SnapshotTuple>,
    /// Full 64-hex id of the resolved seed symbol; for a handle seed whose
    /// definition has several split identities, the first of
    /// [`Self::symbol_ids`].
    pub symbol_id: Option<String>,
    /// Every symbol the window read: one, or the split identities of one
    /// definition (a handle seed, context-v2 § Doors) in (producer, id) order.
    pub symbol_ids: Vec<String>,
    pub target: Option<TargetResolution>,
    pub definitions: Vec<DefinitionCandidate>,
    pub definitions_truncated: bool,
    pub items: Vec<ReferenceItem>,
    /// Graph records examined in this window, definition lookups included.
    pub examined: usize,
    /// Examined reference occurrences whose target is external, unknown or
    /// ambiguous.
    pub unresolved: usize,
    /// Examined records dropped by the final read as ineligible.
    pub stale: usize,
    pub candidates_full: bool,
    pub coverage: Coverage,
    /// More references exist after the window.
    pub more: bool,
    /// `<path>#<start>` after the last examined reference, when `more`.
    pub resume: Option<String>,
}

enum Seed {
    Symbol(String),
    Position(HandleRef, u64),
    /// A handle alone: the symbols of its unit by the exact-doors rule.
    Handle(HandleRef),
}

/// One symbol a references window reads, with the producer that owns it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Target {
    namespace: String,
    symbol_id: String,
}

enum Seeded {
    /// One symbol, or the split identities of one definition, in
    /// `(namespace, symbol id)` order.
    Found(Vec<Target>),
    /// A successful line-less answer; `coverage` says why. `truncated` marks
    /// an examination window that ended before the answer was known.
    Unanswerable {
        coverage: Coverage,
        symbol_id: Option<String>,
        producer: Option<String>,
        truncated: bool,
    },
}

fn parse_cursor(raw: &str) -> FResult<(String, u64, u64)> {
    let bad = || invalid("after must be `<path>#<start>-<end>`");
    let (path, span) = raw.rsplit_once('#').ok_or_else(bad)?;
    validate_path(path)?;
    let (start, end) = span.split_once('-').ok_or_else(bad)?;
    let start = canonical_decimal(start.as_bytes()).ok_or_else(bad)?;
    let end = canonical_decimal(end.as_bytes()).ok_or_else(bad)?;
    if start >= end {
        return Err(bad());
    }
    Ok((path.to_owned(), start, end))
}

fn symbol_prefix(raw: &str) -> FResult<String> {
    if raw.len() == 16 && raw.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        Ok(raw.to_owned())
    } else {
        Err(invalid("symbol_id must be 16 lowercase hex characters"))
    }
}

fn ambiguous_symbol(ids: &[String], truncated: bool) -> FoundryError {
    let shown: Vec<&str> = ids
        .iter()
        .take(8)
        .map(|id| id.get(..16).unwrap_or(id))
        .collect();
    crate::scip::fail(
        crate::scip::code::AMBIGUOUS_SYMBOL,
        format!(
            "the seed names several symbols; candidates (at most 8): {}{}",
            shown.join(" "),
            if truncated { "; more exist" } else { "" }
        ),
    )
}

fn symbol_not_found(message: &str) -> FoundryError {
    crate::scip::fail(crate::scip::code::SYMBOL_NOT_FOUND, message)
}

/// A scope row is eligible when it was published under the selected snapshot.
struct EligibleScopes {
    scopes: redb::ReadOnlyTable<&'static str, &'static str>,
    namespace: String,
    snapshot_id: String,
    cache: HashMap<String, Option<ScopeRow>>,
}

impl EligibleScopes {
    fn get(&mut self, path: &str) -> FResult<Option<ScopeRow>> {
        if let Some(row) = self.cache.get(path) {
            return Ok(row.clone());
        }
        let row = self
            .scopes
            .get(scope_key(&self.namespace, path).as_str())?
            .map(|raw| stored_row::<ScopeRow>(raw.value(), "compiler scope"))
            .transpose()?
            .filter(|row| row.snapshot_id == self.snapshot_id);
        self.cache.insert(path.to_owned(), row.clone());
        Ok(row)
    }
}

/// One origin source, verified and indexed for line and delivery-unit lookups.
/// Its delivery units are parsed on first use, so a file whose sites are only
/// counted (doors beyond the shown files) is never parsed.
pub(crate) struct FileView {
    /// The SHA-256 of the bytes `body` was verified against.
    pub(crate) hash: String,
    pub(crate) body: String,
    lang: Option<crate::syntax::Lang>,
    line_starts: Vec<usize>,
    documents: std::cell::OnceCell<Vec<crate::syntax::Document>>,
}

impl FileView {
    pub(crate) fn new(path: &str, hash: String, body: String) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(body.match_indices('\n').map(|(at, _)| at + 1));
        Self {
            hash,
            body,
            lang: crate::syntax::Lang::from_path(path),
            line_starts,
            documents: std::cell::OnceCell::new(),
        }
    }

    fn documents(&self) -> &[crate::syntax::Document] {
        self.documents
            .get_or_init(|| crate::syntax::documents(&self.body, self.lang))
    }

    /// One-based line of the byte `offset`.
    pub(crate) fn line_of(&self, offset: usize) -> u64 {
        self.line_starts.partition_point(|&start| start <= offset) as u64
    }

    /// The text of one-based `line`, without its LF or CRLF terminator.
    pub(crate) fn line_text(&self, line: u64) -> &str {
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

    fn unit_at(&self, offset: usize) -> Option<&crate::syntax::DeliveryUnit> {
        let documents = self.documents();
        let index = documents.partition_point(|d| d.start <= offset);
        let document = documents.get(index.checked_sub(1)?)?;
        (offset < document.end).then_some(&document.unit)
    }

    /// The name node of the definition unit spanning exactly
    /// `start..end`, if one does (context-v2 § Definitions and addresses).
    fn definition_name(&self, start: u64, end: u64) -> Option<(u64, u64)> {
        self.documents().iter().find_map(|document| {
            let unit = &document.unit;
            (unit.start as u64 == start && unit.end as u64 == end)
                .then_some(unit.name_range)
                .flatten()
                .map(|(from, to)| (from as u64, to as u64))
        })
    }
}

/// The verified view of `path` when its source still carries
/// `expected_hash`, the bytes the caller's scope was published from; `None`
/// when the source is absent or carries other bytes (stale). A source is read
/// and verified at most once per response, and every access compares the
/// view's verified hash with the caller's: a scope expecting other bytes never
/// reuses bytes verified for another.
fn verified_file<'f>(
    files: &'f mut HashMap<String, Option<FileView>>,
    sources: &redb::ReadOnlyTable<&'static str, &'static str>,
    chunks: &redb::ReadOnlyTable<&'static str, &'static str>,
    path: &str,
    expected_hash: &str,
) -> FResult<Option<&'f FileView>> {
    if !files.contains_key(path) {
        let view = match sources.get(path)? {
            None => None,
            Some(raw) => {
                let meta: SourceMeta = crate::store::decode(raw.value(), "source")?;
                if meta.hash != expected_hash {
                    // Stale for this caller and not read: a later caller
                    // expecting the stored bytes still loads them.
                    return Ok(None);
                }
                let body = reconstruct_verified(chunks, path, &meta)?.body;
                Some(FileView::new(path, meta.hash, body))
            }
        };
        files.insert(path.to_owned(), view);
    }
    Ok(cached_file(files, path, expected_hash))
}

/// The already verified view of `path`, only when it carries `expected_hash`.
fn cached_file<'f>(
    files: &'f HashMap<String, Option<FileView>>,
    path: &str,
    expected_hash: &str,
) -> Option<&'f FileView> {
    files
        .get(path)
        .and_then(Option::as_ref)
        .filter(|view| view.hash == expected_hash)
}

fn unit_label(unit: &crate::syntax::DeliveryUnit) -> String {
    match unit.qname.as_deref() {
        Some(qname) if !qname.is_empty() => format!("{} {qname}", unit.kind.as_str()),
        _ => unit.kind.as_str().to_owned(),
    }
}

/// A prefix seed: the stored symbol ids starting with it (at most 9 probed).
fn resolve_prefix(
    tx: &redb::ReadTransaction,
    prefix: &str,
    producers: &[(String, ProducerRow)],
) -> FResult<Seeded> {
    if !producers.iter().any(|(_, row)| row.selected.is_some()) {
        return Ok(Seeded::Unanswerable {
            coverage: Coverage::Unavailable,
            symbol_id: None,
            producer: None,
            truncated: false,
        });
    }
    let by_symbol = tx.open_table(COMPILER_BY_SYMBOL)?;
    let mut found: Vec<(String, String)> = Vec::new();
    let mut from = prefix.to_owned();
    while found.len() <= 8 {
        let Some(entry) = by_symbol.range(from.as_str()..)?.next() else {
            break;
        };
        let (key, value) = entry?;
        let key = key.value();
        if !key.starts_with(prefix) {
            break;
        }
        let id = key
            .get(..64)
            .filter(|id| is_symbol_id(id) && key.as_bytes().get(64) == Some(&0))
            .ok_or_else(|| corrupt_key("compiler symbol"))?
            .to_owned();
        from = format!("{id}\u{1}");
        found.push((id, value.value().to_owned()));
    }
    match found.len() {
        0 => Err(symbol_not_found(
            "no stored compiler symbol has that id prefix",
        )),
        1 => {
            let (symbol_id, namespace) = found.remove(0);
            Ok(Seeded::Found(vec![Target {
                namespace,
                symbol_id,
            }]))
        }
        n => {
            let ids: Vec<String> = found.into_iter().map(|(id, _)| id).collect();
            Err(ambiguous_symbol(&ids, n > 8))
        }
    }
}

/// A handle seed's verified source, by 001's handle precedence (the
/// workspace was checked first): `not_found` without a source,
/// `stale_handle` when it carries other bytes, then the handle's range
/// checked against the verified bytes.
fn seed_view<'f>(
    tx: &redb::ReadTransaction,
    handle: &HandleRef,
    files: &'f mut HashMap<String, Option<FileView>>,
    visited: &mut BTreeSet<String>,
) -> FResult<&'f FileView> {
    let sources = tx.open_table(SOURCES)?;
    let meta: SourceMeta = match sources.get(handle.path.as_str())? {
        None => return Err(FoundryError::NotFound),
        Some(raw) => crate::store::decode(raw.value(), "source")?,
    };
    if !meta.hash.starts_with(&handle.sha32) {
        return Err(FoundryError::StaleHandle);
    }
    // The seed's own source read is part of the response's shared file
    // budget: it goes through the same cache every later file uses.
    visited.insert(handle.path.clone());
    let Some(view) = verified_file(
        files,
        &sources,
        &tx.open_table(CHUNKS)?,
        &handle.path,
        &meta.hash,
    )?
    else {
        return Err(FoundryError::CorruptSource(format!(
            "{}: the indexed source could not be loaded",
            handle.path
        )));
    };
    let body = view.body.as_str();
    let valid_empty = handle.start == 0 && handle.end == 0 && meta.bytes == 0;
    let valid_span = handle.start < handle.end
        && handle.end <= body.len() as u64
        && body.is_char_boundary(handle.start as usize)
        && body.is_char_boundary(handle.end as usize);
    if !valid_empty && !valid_span {
        return Err(FoundryError::InvalidRange);
    }
    Ok(view)
}

/// A position seed: 001's handle precedence (workspace was checked first),
/// then the NARROWEST occurrence containing `offset` across every producer's
/// scope for that source. Several distinct symbols sharing that narrowest
/// span are `ambiguous_symbol` (at most eight candidates). A search the
/// shared allowance did not finish is reported truncated — never as a
/// resolution and never as an absence.
#[allow(clippy::too_many_arguments)] // the seed shares the response's one window state
fn resolve_position(
    tx: &redb::ReadTransaction,
    handle: &HandleRef,
    offset: u64,
    producers: &[(String, ProducerRow)],
    revision: u64,
    examined: &mut usize,
    files: &mut HashMap<String, Option<FileView>>,
    visited: &mut BTreeSet<String>,
) -> FResult<Seeded> {
    let view = seed_view(tx, handle, files, visited)?;
    let body = view.body.as_str();
    if !(handle.start <= offset && offset < handle.end) {
        return Err(FoundryError::InvalidRange);
    }
    let selected_rows: Vec<&(String, ProducerRow)> = producers
        .iter()
        .filter(|(_, row)| row.selected.is_some())
        .collect();
    if selected_rows.is_empty() {
        return Ok(Seeded::Unanswerable {
            coverage: Coverage::Unavailable,
            symbol_id: None,
            producer: None,
            truncated: false,
        });
    }
    // Every selected snapshot predates the source revision: all compiler
    // facts are ineligible, which is decided BEFORE any deep occurrence
    // scan so that scan's exhaustion can never relabel a stale graph.
    if selected_rows.iter().all(|(_, row)| {
        row.selected
            .as_ref()
            .is_some_and(|selected| selected.tuple.source_revision != revision)
    }) {
        return Ok(Seeded::Unanswerable {
            coverage: Coverage::Stale,
            symbol_id: None,
            producer: (selected_rows.len() == 1).then(|| selected_rows[0].0.clone()),
            truncated: false,
        });
    }
    let scopes = tx.open_table(COMPILER_SCOPES)?;
    let occurrences = tx.open_table(COMPILER_OCCURRENCES)?;
    // `(span, symbol id, namespace, eligible)` of every containing
    // occurrence the allowance could reach, across all producers.
    let mut entries: Vec<(u64, String, String, bool)> = Vec::new();
    let mut exhausted = false;
    // Each producer's scope for this source, eligible producers first: once
    // an eligible containing occurrence is known, ineligible producers can
    // no longer contribute to the pool and are not scanned at all.
    let mut scanned: Vec<(&String, ScopeRow, bool)> = Vec::new();
    for (namespace, row) in producers {
        let Some(scope) = scopes
            .get(scope_key(namespace, &handle.path).as_str())?
            .map(|raw| stored_row::<ScopeRow>(raw.value(), "compiler scope"))
            .transpose()?
        else {
            continue;
        };
        // Facts read from other bytes cannot describe this handle.
        if scope.source_hash != view.hash {
            continue;
        }
        let eligible = row.selected.as_ref().is_some_and(|selected| {
            selected.tuple.source_revision == revision
                && selected.tuple.snapshot_id == scope.snapshot_id
        });
        scanned.push((namespace, scope, eligible));
    }
    scanned.sort_by_key(|(_, _, eligible)| !eligible);
    // The narrowest span among ELIGIBLE containing occurrences found so far.
    let mut narrowest_eligible: Option<u64> = None;
    for (namespace, scope, eligible) in &scanned {
        if !eligible && narrowest_eligible.is_some() {
            continue;
        }
        let low = format!("{namespace}\0{}\0", handle.path);
        let high = format!("{namespace}\0{}\0{:020}", handle.path, offset + 1);
        for entry in occurrences.range(low.as_str()..high.as_str())?.rev() {
            if *examined >= SEED_SCAN_LIMIT {
                exhausted = true;
                break;
            }
            let (key, _) = entry?;
            *examined += 1;
            let parsed = parse_occurrence_key(key.value())?;
            // The parsed range is checked against the already-verified seed
            // body before its span or either stop rule is used: a stored
            // key that does not name a real range of this source is named
            // corruption, never a resolved seed.
            if parsed.end > body.len() as u64
                || !body.is_char_boundary(parsed.start as usize)
                || !body.is_char_boundary(parsed.end as usize)
            {
                return Err(FoundryError::GraphInvalid(format!(
                    "a stored occurrence lies outside its source: {}",
                    handle.path
                )));
            }
            // Descending starts: an occurrence that starts at or before
            // `offset - L` and still contains `offset` is strictly wider
            // than L, so no narrower occurrence and no equal-span tie can
            // remain (ties start after `offset - L` and are still scanned).
            if narrowest_eligible
                .is_some_and(|narrowest| parsed.start.saturating_add(narrowest) <= offset)
            {
                break;
            }
            if parsed.end > offset {
                let span = parsed.end - parsed.start;
                if *eligible {
                    narrowest_eligible = Some(narrowest_eligible.map_or(span, |n| n.min(span)));
                }
                entries.push((span, parsed.symbol_id, (*namespace).clone(), *eligible));
            }
            // Nothing earlier can contain `offset` once the longest stored
            // occurrence starting here would still end at or before it.
            if parsed.start.saturating_add(scope.max_span) <= offset {
                break;
            }
        }
        if exhausted {
            break;
        }
    }
    if exhausted {
        // The containing set was not fully searched: neither the narrowest
        // winner nor an absence may be certified.
        return Ok(Seeded::Unanswerable {
            coverage: Coverage::Partial,
            symbol_id: None,
            producer: None,
            truncated: true,
        });
    }
    // The narrowest span wins among the eligible pool; several symbols of
    // exactly that span are ambiguous.
    let has_eligible = entries.iter().any(|e| e.3);
    let pool: Vec<&(u64, String, String, bool)> =
        entries.iter().filter(|e| e.3 || !has_eligible).collect();
    let narrowest = pool.iter().map(|e| e.0).min();
    let mut matches: BTreeMap<String, (String, bool)> = BTreeMap::new();
    for entry in pool.iter().filter(|e| Some(e.0) == narrowest) {
        let found = matches
            .entry(entry.1.clone())
            .or_insert_with(|| (entry.2.clone(), false));
        found.1 |= entry.3;
    }
    let eligible: Vec<(&String, &String)> = matches
        .iter()
        .filter(|(_, (_, eligible))| *eligible)
        .map(|(id, (namespace, _))| (id, namespace))
        .collect();
    if eligible.len() == 1 {
        return Ok(Seeded::Found(vec![Target {
            namespace: eligible[0].1.clone(),
            symbol_id: eligible[0].0.clone(),
        }]));
    }
    if eligible.len() > 1 {
        let ids: Vec<String> = eligible.iter().map(|(id, _)| (*id).clone()).collect();
        return Err(ambiguous_symbol(&ids, ids.len() > 8));
    }
    let any_stale = producers.iter().any(|(_, row)| {
        row.selected
            .as_ref()
            .is_some_and(|selected| selected.tuple.source_revision != revision)
    });
    if any_stale || !matches.is_empty() {
        let only = (matches.len() == 1)
            .then(|| matches.iter().next())
            .flatten();
        return Ok(Seeded::Unanswerable {
            coverage: if any_stale {
                Coverage::Stale
            } else {
                Coverage::Partial
            },
            symbol_id: only.map(|(id, _)| id.clone()),
            producer: only.map(|(_, (namespace, _))| namespace.clone()),
            truncated: false,
        });
    }
    Err(symbol_not_found(
        "no compiler occurrence at that byte offset",
    ))
}

/// What the current compiler scopes say about one definition's name range
/// (context-v2 § Doors, exact doors).
enum NameSymbols {
    /// No producer has a selected snapshot.
    Unavailable,
    /// Every selected snapshot predates the source revision.
    Stale { producer: Option<String> },
    /// No current scope of the source holds a definition occurrence whose
    /// range equals the name range.
    Undefined,
    /// The shared allowance ran out before the scan finished.
    Unfinished,
    /// The definition occurrences' symbols, distinct, in (producer, id)
    /// order: one, or several split identities of the same definition.
    Found(Vec<Target>),
}

/// The symbols of the definition occurrences whose range equals `name`
/// exactly, in every producer scope of `path` that is current: selected, at
/// this source revision, published under that selection and over the bytes
/// `source_hash`. Each occurrence record read is counted against the shared
/// allowance.
fn symbols_at_name(
    tx: &redb::ReadTransaction,
    producers: &[(String, ProducerRow)],
    revision: u64,
    path: &str,
    source_hash: &str,
    name: (u64, u64),
    examined: &mut usize,
) -> FResult<NameSymbols> {
    let selected: Vec<(&String, &SelectedSnapshot)> = producers
        .iter()
        .filter_map(|(namespace, row)| row.selected.as_ref().map(|chosen| (namespace, chosen)))
        .collect();
    if selected.is_empty() {
        return Ok(NameSymbols::Unavailable);
    }
    if selected
        .iter()
        .all(|(_, chosen)| chosen.tuple.source_revision != revision)
    {
        return Ok(NameSymbols::Stale {
            producer: (selected.len() == 1).then(|| selected[0].0.clone()),
        });
    }
    let scopes = tx.open_table(COMPILER_SCOPES)?;
    let occurrences = tx.open_table(COMPILER_OCCURRENCES)?;
    let mut found: BTreeSet<Target> = BTreeSet::new();
    for (namespace, chosen) in selected {
        if chosen.tuple.source_revision != revision {
            continue;
        }
        let Some(scope) = scopes
            .get(scope_key(namespace, path).as_str())?
            .map(|raw| stored_row::<ScopeRow>(raw.value(), "compiler scope"))
            .transpose()?
        else {
            continue;
        };
        if scope.snapshot_id != chosen.tuple.snapshot_id || scope.source_hash != source_hash {
            continue;
        }
        let (start, end) = name;
        let low = format!("{namespace}\0{path}\0{start:020}\0{end:020}\0d\0");
        let high = format!("{namespace}\0{path}\0{start:020}\0{end:020}\0d\u{1}");
        for entry in occurrences.range(low.as_str()..high.as_str())? {
            if *examined >= SEED_SCAN_LIMIT {
                return Ok(NameSymbols::Unfinished);
            }
            let (key, _) = entry?;
            *examined += 1;
            found.insert(Target {
                namespace: namespace.clone(),
                symbol_id: parse_occurrence_key(key.value())?.symbol_id,
            });
        }
    }
    Ok(if found.is_empty() {
        NameSymbols::Undefined
    } else {
        NameSymbols::Found(found.into_iter().collect())
    })
}

/// A handle seed (context-v2 § Doors, `references {handle}`): 001's handle
/// precedence (the workspace was checked first), then the definition unit
/// spanning exactly the handle's range and, by the exact-doors rule, the
/// symbols of the definition occurrences at its name range. A handle that
/// names no definition unit, or whose unit no current compiler scope
/// defines, is `invalid_argument` naming `no_compiler_definition`.
fn resolve_handle(
    tx: &redb::ReadTransaction,
    handle: &HandleRef,
    producers: &[(String, ProducerRow)],
    revision: u64,
    examined: &mut usize,
    files: &mut HashMap<String, Option<FileView>>,
    visited: &mut BTreeSet<String>,
) -> FResult<Seeded> {
    let view = seed_view(tx, handle, files, visited)?;
    let undefined = |why: &str| invalid(format!("no_compiler_definition: {why}"));
    let Some(name) = view.definition_name(handle.start, handle.end) else {
        return Err(undefined("the handle names no definition unit"));
    };
    let line_less = |coverage: Coverage, producer: Option<String>, truncated: bool| {
        Ok(Seeded::Unanswerable {
            coverage,
            symbol_id: None,
            producer,
            truncated,
        })
    };
    match symbols_at_name(
        tx,
        producers,
        revision,
        &handle.path,
        &view.hash,
        name,
        examined,
    )? {
        NameSymbols::Unavailable => line_less(Coverage::Unavailable, None, false),
        NameSymbols::Stale { producer } => line_less(Coverage::Stale, producer, false),
        NameSymbols::Unfinished => line_less(Coverage::Partial, None, true),
        NameSymbols::Undefined => Err(undefined(
            "no current compiler scope has a definition occurrence at the unit's name",
        )),
        NameSymbols::Found(targets) => Ok(Seeded::Found(targets)),
    }
}

/// One window over a symbol set's reference records.
#[derive(Default)]
struct ReferenceWindow {
    items: Vec<ReferenceItem>,
    /// Delivered references whose target is not uniquely resolved.
    unresolved: usize,
    /// Records dropped as ineligible: outside the selected snapshot, or a
    /// source no longer carrying the bytes the scope was published from.
    stale: usize,
    /// A cap stopped the window with a record still pending.
    want_more: bool,
    /// The last record consumed: the continuation cursor's position.
    consumed: Option<(String, u64, u64)>,
}

/// The eligibility view of each producer a target set names: its scopes
/// under the producer's selected snapshot. A producer without a selection
/// gets none, so every record it owns is ineligible.
fn eligible_scopes(
    tx: &redb::ReadTransaction,
    producers: &[(String, ProducerRow)],
    targets: &[Target],
) -> FResult<HashMap<String, EligibleScopes>> {
    let mut scopes = HashMap::new();
    for target in targets {
        if scopes.contains_key(&target.namespace) {
            continue;
        }
        let Some(chosen) = producers
            .iter()
            .find(|(name, _)| *name == target.namespace)
            .and_then(|(_, row)| row.selected.as_ref())
        else {
            continue;
        };
        scopes.insert(
            target.namespace.clone(),
            EligibleScopes {
                scopes: tx.open_table(COMPILER_SCOPES)?,
                namespace: target.namespace.clone(),
                snapshot_id: chosen.tuple.snapshot_id.clone(),
                cache: HashMap::new(),
            },
        );
    }
    Ok(scopes)
}

/// One window over `targets`' reference records, merged in `(path, start,
/// end)` order strictly after `after`, ties in target order. A site several
/// targets share (split identities of one definition) is delivered once.
/// `limit` and the shared allowance are checked per record on the EXISTENCE
/// of a next record, before any is decoded: an exhausted window never reads
/// (and so never trips over) a record it could not deliver anyway; the
/// visited-file budget is checked before a new file is read. `unique` is the
/// target's unique definition: it gives each item its edge id, and without
/// it every delivered item counts as unresolved. `labeled` names each item's
/// enclosing delivery unit (parsing its file); otherwise the unit is the site
/// itself and the label empty, and the caller labels what it shows.
#[allow(clippy::too_many_arguments)] // the window shares the response's one read state
fn reference_window(
    by_symbol: &redb::ReadOnlyTable<&'static str, &'static str>,
    sources: &redb::ReadOnlyTable<&'static str, &'static str>,
    chunks: &redb::ReadOnlyTable<&'static str, &'static str>,
    scopes: &mut HashMap<String, EligibleScopes>,
    targets: &[Target],
    after: Option<&(String, u64, u64)>,
    limit: usize,
    unique: Option<&DefinitionCandidate>,
    bound: &str,
    labeled: bool,
    examined: &mut usize,
    files: &mut HashMap<String, Option<FileView>>,
    visited: &mut BTreeSet<String>,
) -> FResult<ReferenceWindow> {
    let bounds: Vec<(String, String)> = targets
        .iter()
        .map(|target| {
            let id = &target.symbol_id;
            let low = match after {
                Some((path, start, end)) => format!("{id}\0r\0{path}\0{start:020}\0{end:020}\u{1}"),
                None => format!("{id}\0r\0"),
            };
            (low, format!("{id}\0r\u{1}"))
        })
        .collect();
    let mut heads = Vec::with_capacity(bounds.len());
    for (low, high) in &bounds {
        heads.push(by_symbol.range(low.as_str()..high.as_str())?.peekable());
    }
    let mut window = ReferenceWindow::default();
    let mut delivered: Option<(String, u64, u64)> = None;
    loop {
        let mut pending = false;
        for head in &mut heads {
            if matches!(head.peek(), Some(Err(_)))
                && let Some(Err(error)) = head.next()
            {
                return Err(error.into());
            }
            pending |= head.peek().is_some();
        }
        if !pending {
            break;
        }
        if window.items.len() >= limit || *examined >= REFERENCES_MAX_EXAMINED {
            window.want_more = true;
            break;
        }
        let mut next: Option<(usize, SymbolKey)> = None;
        for (index, head) in heads.iter_mut().enumerate() {
            let Some(Ok((key, _))) = head.peek() else {
                continue;
            };
            let parsed = parse_symbol_key(key.value())?;
            let earlier = next.as_ref().is_none_or(|(_, best)| {
                (parsed.path.as_str(), parsed.start, parsed.end)
                    < (best.path.as_str(), best.start, best.end)
            });
            if earlier {
                next = Some((index, parsed));
            }
        }
        let Some((index, parsed)) = next else {
            break;
        };
        let target = &targets[index];
        let site = (parsed.path.clone(), parsed.start, parsed.end);
        let scope = match scopes.get_mut(&target.namespace) {
            Some(eligible) => eligible.get(&parsed.path)?,
            None => None,
        };
        let Some(scope) = scope else {
            heads[index].next();
            *examined += 1;
            window.stale += 1;
            window.consumed = Some(site);
            continue;
        };
        if !visited.contains(&parsed.path) && visited.len() >= REFERENCES_MAX_FILES {
            window.want_more = true;
            break;
        }
        heads[index].next();
        *examined += 1;
        window.consumed = Some(site.clone());
        // A scope expecting other bytes than the verified ones is stale,
        // even where another producer's scope loaded the file first.
        let Some(view) = verified_file(files, sources, chunks, &parsed.path, &scope.source_hash)?
        else {
            window.stale += 1;
            continue;
        };
        visited.insert(parsed.path.clone());
        let (start, end) = (parsed.start as usize, parsed.end as usize);
        if end > view.body.len()
            || !view.body.is_char_boundary(start)
            || !view.body.is_char_boundary(end)
        {
            return Err(FoundryError::GraphInvalid(format!(
                "a stored occurrence lies outside its source: {}",
                parsed.path
            )));
        }
        if delivered.as_ref() == Some(&site) {
            continue;
        }
        delivered = Some(site);
        if unique.is_none() {
            window.unresolved += 1;
        }
        let (unit_start, unit_end, label) = match labeled.then(|| view.unit_at(start)) {
            Some(Some(unit)) => (unit.start as u64, unit.end as u64, unit_label(unit)),
            Some(None) => (parsed.start, parsed.end, "block".to_owned()),
            None => (parsed.start, parsed.end, String::new()),
        };
        let edge = unique.map(|definition| {
            edge_id(
                &target.namespace,
                "references",
                EdgeEnd {
                    identity: &parsed.path,
                    hash: &scope.source_hash,
                    start: parsed.start,
                    end: parsed.end,
                },
                EdgeEnd {
                    identity: &definition.path,
                    hash: &definition.sha256,
                    start: definition.start,
                    end: definition.end,
                },
            )
        });
        window.items.push(ReferenceItem {
            occurrence_id: occurrence_id(
                &target.namespace,
                OccurrenceKind::Reference,
                &parsed.path,
                &scope.source_hash,
                parsed.start,
                parsed.end,
                &target.symbol_id,
            ),
            path: parsed.path.clone(),
            sha256: scope.source_hash.clone(),
            start: parsed.start,
            end: parsed.end,
            line: view.line_of(start),
            unit: SourceHandle {
                workspace_id: bound.to_owned(),
                path: parsed.path,
                sha256: scope.source_hash,
                start: unit_start,
                end: unit_end,
            },
            label,
            edge_id: edge,
        });
    }
    Ok(window)
}

/// What the definition lookup and the reference window found for a target
/// set.
struct TargetWindow {
    definitions: Vec<DefinitionCandidate>,
    definitions_truncated: bool,
    target: TargetResolution,
    window: ReferenceWindow,
    /// Definition and reference records dropped as ineligible.
    stale: usize,
}

/// The definition lookup, then one reference window, over `targets` in the
/// caller's read `tx`, drawing on the response's shared allowance
/// (`examined`), file cache and visited-file budget: the one window that
/// `references` and exact doors share (context-v2 § Doors). `labeled` gives
/// `references` items (enclosing unit, label, edge id); otherwise door sites,
/// which the caller labels per shown file.
#[allow(clippy::too_many_arguments)] // the window shares the response's one read state
fn target_window(
    tx: &redb::ReadTransaction,
    producers: &[(String, ProducerRow)],
    targets: &[Target],
    after: Option<&(String, u64, u64)>,
    limit: usize,
    bound: &str,
    labeled: bool,
    examined: &mut usize,
    files: &mut HashMap<String, Option<FileView>>,
    visited: &mut BTreeSet<String>,
) -> FResult<TargetWindow> {
    let by_symbol = tx.open_table(COMPILER_BY_SYMBOL)?;
    let sources = tx.open_table(SOURCES)?;
    let chunks = tx.open_table(CHUNKS)?;
    let mut scopes = eligible_scopes(tx, producers, targets)?;
    let mut stale = 0usize;

    // Definition lookup: at most nine eligible records (eight candidates and
    // one more to know that more exist; stale drops draw too), taken from the
    // shared allowance but never from its last record
    // (`DEFINITION_SCAN_LIMIT`). A split identity's definition at a site
    // already listed is the same definition. Every candidate that survives
    // is re-verified against its indexed bytes before it can make the target
    // unique.
    let mut definitions: Vec<DefinitionCandidate> = Vec::new();
    let mut eligible_definitions = 0usize;
    let mut definitions_truncated = false;
    let mut lookup_finished = true;
    'lookup: for target in targets {
        let low = format!("{}\0d\0", target.symbol_id);
        let high = format!("{}\0d\u{1}", target.symbol_id);
        for entry in by_symbol.range(low.as_str()..high.as_str())? {
            if *examined >= DEFINITION_SCAN_LIMIT {
                lookup_finished = false;
                break 'lookup;
            }
            let (key, _) = entry?;
            *examined += 1;
            let parsed = parse_symbol_key(key.value())?;
            let scope = match scopes.get_mut(&target.namespace) {
                Some(eligible) => eligible.get(&parsed.path)?,
                None => None,
            };
            let Some(scope) = scope else {
                stale += 1;
                continue;
            };
            if definitions.iter().any(|known| {
                known.path == parsed.path && known.start == parsed.start && known.end == parsed.end
            }) {
                continue;
            }
            if definitions.len() == DEFINITION_CANDIDATES {
                // A ninth eligible candidate only proves that more exist.
                eligible_definitions += 1;
                definitions_truncated = true;
                break 'lookup;
            }
            // Verify the candidate's source from the same transaction and
            // the same file budget BEFORE it can count as a target: an
            // absent or re-hashed source (or bytes verified for another
            // scope's hash) is a stale drop, an unverifiable range is a named
            // corruption.
            let Some(view) =
                verified_file(files, &sources, &chunks, &parsed.path, &scope.source_hash)?
            else {
                stale += 1;
                continue;
            };
            let body = &view.body;
            let verified = parsed.end <= body.len() as u64
                && body.is_char_boundary(parsed.start as usize)
                && body.is_char_boundary(parsed.end as usize);
            if !verified {
                return Err(FoundryError::GraphInvalid(format!(
                    "a stored definition lies outside its source: {}",
                    parsed.path
                )));
            }
            eligible_definitions += 1;
            visited.insert(parsed.path.clone());
            definitions.push(DefinitionCandidate {
                path: parsed.path,
                sha256: scope.source_hash,
                start: parsed.start,
                end: parsed.end,
            });
        }
    }
    if !lookup_finished {
        definitions_truncated = true;
    }
    let target = match eligible_definitions {
        0 if lookup_finished => TargetResolution::Unknown,
        1 if lookup_finished => TargetResolution::Unique,
        n if n >= 2 => TargetResolution::Ambiguous,
        _ => TargetResolution::Unfinished,
    };

    // References in (path, start, end) order, strictly after the cursor.
    // Door sites carry no edge ids.
    let unique = (labeled && target == TargetResolution::Unique)
        .then(|| definitions.first())
        .flatten();
    let window = reference_window(
        &by_symbol,
        &sources,
        &chunks,
        &mut scopes,
        targets,
        after,
        limit,
        unique,
        bound,
        labeled,
        examined,
        files,
        visited,
    )?;
    stale += window.stale;
    Ok(TargetWindow {
        definitions,
        definitions_truncated,
        target,
        window,
        stale,
    })
}

impl Engine {
    /// Budget-independent references query: seed resolution, definition
    /// lookup and a bounded, ordered reference window, all inside one final
    /// read transaction. Every fact is revalidated there: its scope must
    /// belong to the producer's selected snapshot, the store's source
    /// revision must still equal that snapshot's, and the origin and
    /// definition sources are re-verified (chunks, hash, range) from the
    /// same transaction and the same shared file budget. Ordering and the
    /// `after` cursor are `(path, start, end)`; `limit` and the shared
    /// 256-record allowance are enforced per record, so a page never
    /// overshoots them. A handle seed's split identities are read as one
    /// set: their definitions and references deduplicated by site.
    pub fn references(&self, request: &ReferencesRequest) -> FResult<ReferencesOutcome> {
        self.references_in(&self.db.begin_read()?, request, &mut HashMap::new())
    }

    /// Compose compiler evidence in the caller's final source/task snapshot.
    /// `files` is the caller's verified-file cache for `tx`: calls sharing it
    /// read and verify a source once, and each call keeps its own allowance
    /// and visited-file budget.
    pub(crate) fn references_in(
        &self,
        tx: &redb::ReadTransaction,
        request: &ReferencesRequest,
        files: &mut HashMap<String, Option<FileView>>,
    ) -> FResult<ReferencesOutcome> {
        if !(1..=REFERENCES_MAX_LIMIT).contains(&request.limit) {
            return Err(invalid("references limit must be 1..256"));
        }
        let after = request.after.as_deref().map(parse_cursor).transpose()?;
        let seed = match &request.seed {
            ReferencesSeed::SymbolId(raw) => Seed::Symbol(symbol_prefix(raw)?),
            ReferencesSeed::Position {
                handle,
                byte_offset,
            } => Seed::Position(HandleRef::parse(handle)?, *byte_offset),
            ReferencesSeed::Handle(handle) => Seed::Handle(HandleRef::parse(handle)?),
        };
        let bound = self.workspace_id().ok_or(FoundryError::WorkspaceUnbound)?;
        if let Seed::Position(handle, _) | Seed::Handle(handle) = &seed
            && !bound.starts_with(&handle.ws16)
        {
            return Err(FoundryError::WrongWorkspace);
        }
        let freshness = self.freshness_in(tx)?;
        let producers = read_producers(tx)?;
        // One cache and one visited-file budget serve the seed read, the
        // definition verification and the reference labeling alike.
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut examined = 0usize;
        let seeded = match &seed {
            Seed::Symbol(prefix) => resolve_prefix(tx, prefix, &producers)?,
            Seed::Position(handle, offset) => resolve_position(
                tx,
                handle,
                *offset,
                &producers,
                freshness.source_revision,
                &mut examined,
                files,
                &mut visited,
            )?,
            Seed::Handle(handle) => resolve_handle(
                tx,
                handle,
                &producers,
                freshness.source_revision,
                &mut examined,
                files,
                &mut visited,
            )?,
        };
        let line_less = |freshness: Freshness,
                         coverage: Coverage,
                         symbol_id: Option<String>,
                         producer: Option<String>,
                         examined: usize,
                         truncated: bool| ReferencesOutcome {
            freshness,
            snapshot: producer
                .as_deref()
                .and_then(|name| producers.iter().find(|(n, _)| n == name))
                .and_then(|(_, row)| row.selected.as_ref())
                .map(|selected| selected.tuple.clone()),
            producer,
            symbol_ids: symbol_id.iter().cloned().collect(),
            symbol_id,
            target: None,
            definitions: Vec::new(),
            definitions_truncated: truncated,
            items: Vec::new(),
            examined,
            unresolved: 0,
            stale: 0,
            candidates_full: truncated,
            coverage,
            more: false,
            resume: None,
        };
        let targets = match seeded {
            Seeded::Found(targets) => targets,
            Seeded::Unanswerable {
                coverage,
                symbol_id,
                producer,
                truncated,
            } => {
                return Ok(line_less(
                    freshness, coverage, symbol_id, producer, examined, truncated,
                ));
            }
        };
        // The first target names the answer's producer and snapshot; a
        // handle seed's targets all come from current scopes.
        let (symbol_id, namespace) = (targets[0].symbol_id.clone(), targets[0].namespace.clone());
        let selected = producers
            .iter()
            .find(|(name, _)| *name == namespace)
            .and_then(|(_, row)| row.selected.as_ref());
        let Some(selected) = selected else {
            return Ok(line_less(
                freshness,
                Coverage::Unavailable,
                Some(symbol_id),
                Some(namespace),
                examined,
                false,
            ));
        };
        if selected.tuple.source_revision != freshness.source_revision {
            return Ok(line_less(
                freshness,
                Coverage::Stale,
                Some(symbol_id),
                Some(namespace),
                examined,
                false,
            ));
        }
        let complete = targets.iter().all(|target| {
            producers
                .iter()
                .find(|(name, _)| *name == target.namespace)
                .and_then(|(_, row)| row.selected.as_ref())
                .is_some_and(|chosen| chosen.state == SnapshotState::Complete)
        });

        let TargetWindow {
            definitions,
            definitions_truncated,
            target,
            window,
            stale,
        } = target_window(
            tx,
            &producers,
            &targets,
            after.as_ref(),
            request.limit,
            &bound,
            true,
            &mut examined,
            files,
            &mut visited,
        )?;
        // `more` promises a continuation cursor. A window that stopped with
        // a reference still pending but before consuming ANY record has no
        // cursor to hand back: that is a budget cliff, not an end. It is
        // reported as a filled window (candidates:full) with partial
        // coverage, never as completion and never by clearing a real `more`.
        // The seed scan and the definition lookup both stop short of the
        // reserved record, so this is a backstop: a future change to those
        // limits cannot turn an exhausted window into a false end.
        let no_progress = window.want_more && window.consumed.is_none();
        let candidates_full = no_progress
            || examined >= REFERENCES_MAX_EXAMINED
            || visited.len() >= REFERENCES_MAX_FILES;
        let more = window.want_more && window.consumed.is_some();
        let resume = window
            .consumed
            .map(|(path, start, end)| format!("{path}#{start}-{end}"));
        let coverage = if complete
            && stale == 0
            && window.unresolved == 0
            && !no_progress
            && target == TargetResolution::Unique
        {
            Coverage::Complete
        } else {
            Coverage::Partial
        };
        Ok(ReferencesOutcome {
            freshness,
            snapshot: Some(selected.tuple.clone()),
            producer: Some(namespace),
            symbol_id: Some(symbol_id),
            symbol_ids: targets.into_iter().map(|target| target.symbol_id).collect(),
            target: Some(target),
            definitions,
            definitions_truncated,
            items: window.items,
            examined,
            unresolved: window.unresolved,
            stale,
            candidates_full,
            coverage,
            more,
            resume: more.then_some(resume).flatten(),
        })
    }
}

// ---------------------------------------------------------------------------
// Exact doors (context-v2 § Doors, 005 T004).
// ---------------------------------------------------------------------------

/// The exact doors of one definition, summarized by file.
pub(crate) struct ExactDoors {
    /// One line per file in path order, at most the caller's cap.
    pub(crate) lines: Vec<crate::store::DoorLine>,
    /// Files with sites beyond the listed ones.
    pub(crate) more_files: usize,
    /// The references window filled (records, files or lines).
    pub(crate) full: bool,
    /// Definition and reference records the window dropped as ineligible.
    pub(crate) stale: usize,
}

/// What exact-door resolution found for a definition.
pub(crate) enum ExactResolution {
    Doors(ExactDoors),
    /// No current compiler scope defines the name: approximate doors serve.
    /// `exhausted` when the name scan ran out of the shared allowance before
    /// it finished - the window filled (`candidates:full`) even though the
    /// approximate window may not.
    Approximate {
        exhausted: bool,
    },
}

/// The exact-door reads of one response (context-v2 § Doors), in its final
/// read transaction: one references window per target, each exactly the
/// window `references {handle}` on that target reads (its own record, file
/// and line caps). The windows share only the verified-file cache - a source
/// is read and verified at most once per response - and the sites already
/// summarized: a site appears at most once in a response.
pub(crate) struct DoorReads<'t> {
    tx: &'t redb::ReadTransaction,
    revision: u64,
    bound: &'t str,
    files: HashMap<String, Option<FileView>>,
    /// Per path, the `(start, end)` of every site a listed file summarized.
    summarized: HashMap<String, BTreeSet<(u64, u64)>>,
}

impl<'t> DoorReads<'t> {
    pub(crate) fn new(tx: &'t redb::ReadTransaction, revision: u64, bound: &'t str) -> Self {
        Self {
            tx,
            revision,
            bound,
            files: HashMap::new(),
            summarized: HashMap::new(),
        }
    }

    /// Exact doors (context-v2 § Doors) of the definition whose stored name
    /// range in `path` (over the bytes `source_hash`) is `name`: when a
    /// current compiler scope of `path` holds definition occurrences whose
    /// range equals it, their symbols - one, or several split identities of
    /// the same definition - give their references, deduplicated by site
    /// and read with 005's scope, snapshot, revision and source checks, in
    /// exactly the window `references {handle}` on the definition reads at
    /// limit 256: the definition's own source is its first file, charged
    /// once, then the definition lookup and the references draw on the same
    /// record, file and line caps. The window's sites, less those an earlier
    /// window's listed files summarized, give at most `cap` lines, one per
    /// file in path order. No such occurrence gives approximate doors. Only
    /// the listed files' first sites are labeled, so only those files are
    /// parsed.
    pub(crate) fn exact_doors(
        &mut self,
        path: &str,
        source_hash: &str,
        name: (u64, u64),
        cap: usize,
    ) -> FResult<ExactResolution> {
        let tx = self.tx;
        let producers = read_producers(tx)?;
        let mut examined = 0usize;
        let targets = match symbols_at_name(
            tx,
            &producers,
            self.revision,
            path,
            source_hash,
            name,
            &mut examined,
        )? {
            NameSymbols::Found(targets) => targets,
            NameSymbols::Unfinished => return Ok(ExactResolution::Approximate { exhausted: true }),
            NameSymbols::Unavailable | NameSymbols::Stale { .. } | NameSymbols::Undefined => {
                return Ok(ExactResolution::Approximate { exhausted: false });
            }
        };
        let mut visited: BTreeSet<String> = BTreeSet::new();
        // The definition's source joins the window first, as a handle seed's
        // does: it counts once toward the visited-file budget.
        if verified_file(
            &mut self.files,
            &tx.open_table(SOURCES)?,
            &tx.open_table(CHUNKS)?,
            path,
            source_hash,
        )?
        .is_none()
        {
            return Ok(ExactResolution::Approximate { exhausted: false });
        }
        visited.insert(path.to_owned());
        let found = target_window(
            tx,
            &producers,
            &targets,
            None,
            REFERENCES_MAX_LIMIT,
            self.bound,
            false,
            &mut examined,
            &mut self.files,
            &mut visited,
        )?;
        let window = found.window;
        let mut doors = ExactDoors {
            lines: Vec::new(),
            more_files: 0,
            full: window.want_more
                || examined >= REFERENCES_MAX_EXAMINED
                || visited.len() >= REFERENCES_MAX_FILES,
            stale: found.stale,
        };
        for sites in window.items.chunk_by(|a, b| a.path == b.path) {
            let known = self.summarized.get(&sites[0].path);
            let mut fresh = sites
                .iter()
                .filter(|site| known.is_none_or(|known| !known.contains(&(site.start, site.end))));
            let Some(first) = fresh.next() else {
                continue;
            };
            if doors.lines.len() == cap {
                doors.more_files += 1;
                continue;
            }
            let more = fresh.count();
            let Some(view) = cached_file(&self.files, &first.path, &first.sha256) else {
                return Err(FoundryError::CorruptStore(format!(
                    "{}: a delivered reference's source is not loaded",
                    first.path
                )));
            };
            let (start, end, label) = match view.unit_at(first.start as usize) {
                Some(unit) => (unit.start as u64, unit.end as u64, unit_label(unit)),
                None => (first.start, first.end, "block".to_owned()),
            };
            doors.lines.push(crate::store::DoorLine {
                unit: SourceHandle {
                    workspace_id: self.bound.to_owned(),
                    path: first.path.clone(),
                    sha256: first.sha256.clone(),
                    start,
                    end,
                },
                line: first.line,
                label,
                text: view.line_text(first.line).to_owned(),
                more,
            });
            self.summarized
                .entry(first.path.clone())
                .or_default()
                .extend(sites.iter().map(|site| (site.start, site.end)));
        }
        Ok(ExactResolution::Doors(doors))
    }
}

impl ReferencesRequest {
    /// The request's pure syntax and bounds, checked with no store access:
    /// `limit`, the `after` cursor grammar, the symbol-id prefix or the
    /// handle's v2 grammar. Existence, digest and range stay in the
    /// authoritative read. The MCP adapter runs this before routing,
    /// reservation and engine admission.
    pub fn validate(&self) -> FResult<()> {
        if !(1..=REFERENCES_MAX_LIMIT).contains(&self.limit) {
            return Err(invalid("references limit must be 1..256"));
        }
        self.after.as_deref().map(parse_cursor).transpose()?;
        match &self.seed {
            ReferencesSeed::SymbolId(raw) => {
                symbol_prefix(raw)?;
            }
            ReferencesSeed::Position { handle, .. } | ReferencesSeed::Handle(handle) => {
                HandleRef::parse(handle)?;
            }
        }
        Ok(())
    }
}
