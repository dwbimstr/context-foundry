//! 007 multi-root context: launch-time admission, aliases, labels, coverage
//! and the cross-root candidate merge.
//!
//! One owner opens every admitted store. Admission happens once, at launch:
//! canonical paths are checked for duplicates, nesting (at a path-component
//! boundary) and `ws16` collisions before anything is opened or served, and
//! each reference's open outcome is its coverage for the whole session (no
//! retries). Query text never admits a root; `roots`/`root` only select
//! already admitted aliases.
//!
//! Merging works on the v2 candidate seam (`CandidateBatch`), never on packed
//! responses: one budget, one reservation and one charge cover the whole
//! merged response, packed once by the v2 ladder over the merged list.

use crate::store::{
    ANCHOR_LIST, AnchorCandidate, AnchorWindow, CandidateBatch, CandidateCounters, CollectedAnchor,
    DOOR_FILES, DoorGroup, DoorState, DoorTarget, Doors, GROUP_FILES, Hit, MAX_ANCHORS,
    QueryAnchors, RankedItem, RenderedForm, SearchOutcome, TIE_GROUP, TIER_OUTLINE,
};
use crate::{Control, Engine, FoundryError, error::FResult};
use std::path::{Path, PathBuf};

/// At most this many references may be admitted besides `primary`.
pub const MAX_REFERENCES: usize = 8;
/// Every admitted root: `primary` plus up to [`MAX_REFERENCES`] references.
pub const MAX_ROOTS: usize = MAX_REFERENCES + 1;
/// A multi-root context keeps this many delivery units in total
/// (context-v2 § Context candidates; the merged total, not per root).
pub const CONTEXT_UNITS: usize = 32;
/// …and this many file outlines in total.
pub const CONTEXT_OUTLINES: usize = 3;
/// The per-file hit cap applies per (root, path) after merging; each root's
/// own batch already capped its files, so the merged list keeps the bound.
pub const PER_FILE_CAP: usize = 4;
/// Search tier 2 (lexical) examines at most this many candidates per root
/// (context-v2 § Search index v2); recorded here for the merged report.
pub const CANDIDATE_LIMIT: usize = 256;
/// Reciprocal-rank fusion denominator for tier 2: `1/(60 + rank)`.
const RRF_K: f64 = 60.0;

/// Session-long coverage of one admitted root (007 § Admission at launch).
/// A reference's open outcome never changes within a session; restart the
/// owner to retry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    Ok,
    MissingStore,
    Busy,
    UnsupportedSchema,
    Corrupt,
    WrongWorkspace,
    RepairRequired,
}

impl Coverage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::MissingStore => "missing_store",
            Self::Busy => "busy",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::Corrupt => "corrupt",
            Self::WrongWorkspace => "wrong_workspace",
            Self::RepairRequired => "repair_required",
        }
    }

    /// True when the store is open and search/context may run in this root.
    /// `repair_required` keeps retrieve and status but skips search/context.
    pub fn serves_search(self) -> bool {
        self == Self::Ok
    }

    /// True when an engine was opened for this root (retrieve/status work).
    pub fn serves_reads(self) -> bool {
        matches!(self, Self::Ok | Self::RepairRequired)
    }
}

/// One `--reference ROOT=STORE` as given at launch. The first `=` separates
/// root from store, so a root path containing `=` cannot be a reference.
#[derive(Clone, Debug)]
pub struct ReferenceSpec {
    pub root: PathBuf,
    pub store: PathBuf,
}

/// Parse `ROOT=STORE`: both sides must be nonempty.
pub fn parse_reference(raw: &str) -> Result<ReferenceSpec, FoundryError> {
    let invalid = || {
        FoundryError::InvalidArgument(format!(
            "--reference must be ROOT=STORE with both sides nonempty: {raw:?}"
        ))
    };
    let (root, store) = raw.split_once('=').ok_or_else(invalid)?;
    if root.is_empty() || store.is_empty() {
        return Err(invalid());
    }
    Ok(ReferenceSpec {
        root: PathBuf::from(root),
        store: PathBuf::from(store),
    })
}

/// A root admitted at launch: its alias (the only addressable name), display
/// label, canonical path and the workspace identity every handle from this
/// root carries.
#[derive(Clone, Debug)]
pub struct AdmittedRoot {
    pub alias: String,
    pub label: String,
    pub root: PathBuf,
    pub workspace_id: String,
}

/// Refusal of an admission request, before any store is opened. Each variant
/// names its contract code; all are invalid-argument exit-2 failures.
#[derive(Clone, Debug)]
pub enum AdmissionError {
    /// A path problem that is not one of the named collisions below.
    Invalid(String),
    TooManyRoots {
        count: usize,
    },
    DuplicateRoot {
        a: PathBuf,
        b: PathBuf,
    },
    NestedRoot {
        outer: PathBuf,
        inner: PathBuf,
    },
    RootIdCollision {
        a: PathBuf,
        b: PathBuf,
        ws16: String,
    },
}

impl AdmissionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "invalid_argument",
            Self::TooManyRoots { .. } => "too_many_roots",
            Self::DuplicateRoot { .. } => "duplicate_root",
            Self::NestedRoot { .. } => "nested_root",
            Self::RootIdCollision { .. } => "root_id_collision",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Invalid(detail) => detail.clone(),
            Self::TooManyRoots { count } => {
                format!("{count} references admitted; at most {MAX_REFERENCES}")
            }
            Self::DuplicateRoot { a, b } => {
                format!(
                    "duplicate_root: {} and {} canonicalize to the same root",
                    a.display(),
                    b.display()
                )
            }
            Self::NestedRoot { outer, inner } => {
                format!(
                    "nested_root: {} lies inside {}",
                    inner.display(),
                    outer.display()
                )
            }
            Self::RootIdCollision { a, b, ws16 } => {
                format!(
                    "root_id_collision: {} and {} share workspace prefix {ws16}",
                    a.display(),
                    b.display()
                )
            }
        }
    }
}

/// One root's display label: the basename with control characters (except
/// TAB) replaced by `?`, cut at a UTF-8 boundary to 32 bytes. Labels may
/// repeat and never address anything (007 § Aliases and labels; context-v2
/// § Single-line fields).
pub fn label_for(root: &Path) -> String {
    let base = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let single: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_control() && c != '\t' {
                '?'
            } else {
                c
            }
        })
        .collect();
    if single.len() <= 32 {
        return single;
    }
    let mut cut = 32;
    while !single.is_char_boundary(cut) {
        cut -= 1;
    }
    single[..cut].to_owned()
}

/// Canonicalize every root and validate the whole admission (007 § Admission
/// at launch). Checks, in order: the reference count, canonicalization of
/// every path, `duplicate_root` (equal canonical paths), `nested_root` (one
/// root contains another at a path-component boundary) and `root_id_collision`
/// (two roots sharing the first 16 hex characters of their `workspace_id`).
/// Nothing is opened here; a refusal exits 2 before serving.
pub fn validate_admission(
    primary: &Path,
    references: &[ReferenceSpec],
) -> Result<Vec<AdmittedRoot>, AdmissionError> {
    if references.len() > MAX_REFERENCES {
        return Err(AdmissionError::TooManyRoots {
            count: references.len(),
        });
    }
    let canonical = |path: &Path| {
        path.canonicalize()
            .map_err(|e| AdmissionError::Invalid(format!("root {}: {e}", path.display())))
    };
    let mut roots = vec![canonical(primary)?];
    for reference in references {
        roots.push(canonical(&reference.root)?);
    }
    for i in 0..roots.len() {
        for j in (i + 1)..roots.len() {
            if roots[i] == roots[j] {
                return Err(AdmissionError::DuplicateRoot {
                    a: roots[i].clone(),
                    b: roots[j].clone(),
                });
            }
            // Component-boundary containment both ways: `strip_prefix` works
            // on components, so `/a/bc` is never treated as inside `/a/b`.
            if roots[j].strip_prefix(&roots[i]).is_ok() {
                return Err(AdmissionError::NestedRoot {
                    outer: roots[i].clone(),
                    inner: roots[j].clone(),
                });
            }
            if roots[i].strip_prefix(&roots[j]).is_ok() {
                return Err(AdmissionError::NestedRoot {
                    outer: roots[j].clone(),
                    inner: roots[i].clone(),
                });
            }
        }
    }
    let mut admitted: Vec<AdmittedRoot> = Vec::with_capacity(roots.len());
    for (index, root) in roots.iter().enumerate() {
        let workspace_id = crate::workspace_id_for_root(root)
            .map_err(|e| AdmissionError::Invalid(format!("root {}: {e}", root.display())))?;
        for other in &admitted {
            if other.workspace_id[..16] == workspace_id[..16] {
                return Err(AdmissionError::RootIdCollision {
                    a: other.root.clone(),
                    b: root.clone(),
                    ws16: workspace_id[..16].to_owned(),
                });
            }
        }
        admitted.push(AdmittedRoot {
            alias: if index == 0 {
                "primary".to_owned()
            } else {
                format!("ref{index}")
            },
            label: label_for(root),
            root: root.clone(),
            workspace_id,
        });
    }
    Ok(admitted)
}

// ---------------------------------------------------------------------------
// Merging (007 § Combined search and context)
// ---------------------------------------------------------------------------

/// One serving root's candidate batch, with the alias its items carry.
pub struct RootBatch {
    pub alias: String,
    pub batch: CandidateBatch,
}

fn summed_counters(batches: &[RootBatch]) -> CandidateCounters {
    let mut counters = CandidateCounters::default();
    for root in batches {
        let batch = &root.batch;
        counters.stale = counters.stale.saturating_add(batch.counters.stale);
        counters.capped = counters.capped.saturating_add(batch.counters.capped);
        counters.candidates_full |= batch.counters.candidates_full;
        counters.truncated |= batch.counters.truncated;
    }
    counters
}

/// A multi-root request's anchors (context-v2 § Anchors and qualifiers;
/// 007), chosen once over every serving root before any root collects
/// candidates: groups 1 and 2 from the query, then each capitalized
/// candidate, in order, that some root defines exactly as written (one
/// cheap probe per root until one admits it), at most four in all. This is
/// the union of the roots' own choices by text, in group and position
/// order, capped at four; every root then builds every chosen anchor's
/// window, so counts, windows and search's tier 1 cover all roots. The
/// cancel/deadline control is checked before each probe. `path` is the
/// search's path filter.
pub fn select_anchors(
    engines: &[&Engine],
    query: &str,
    path: Option<&str>,
    control: &Control,
) -> FResult<Vec<AnchorCandidate>> {
    QueryAnchors::parse(query).select(|candidate| {
        for engine in engines {
            control.check()?;
            if engine.defines_exact_case(&candidate.text, path)? {
                return Ok(true);
            }
        }
        Ok(false)
    })
}

/// The merged anchor windows (context-v2 § Resolver order; 007): one per
/// anchor in anchor order — every root holds the same anchors
/// ([`select_anchors`]) — each anchor's definitions summed over the merged
/// roots, and its entries ordered by the resolver tuple, then root order,
/// then each root's own order — `key_hash` decided only inside a root — cut
/// to [`ANCHOR_LIST`].
fn merged_anchors(batches: &[RootBatch]) -> Vec<AnchorWindow> {
    let mut merged: Vec<AnchorWindow> = Vec::new();
    for root in batches {
        for window in &root.batch.anchors {
            match merged
                .iter_mut()
                .find(|known| known.anchor == window.anchor)
            {
                Some(known) => {
                    known.definitions = known.definitions.saturating_add(window.definitions);
                    known.entries.extend(window.entries.iter().cloned());
                }
                None => merged.push(window.clone()),
            }
        }
    }
    merged.sort_by_key(|window| window.order);
    merged.truncate(MAX_ANCHORS);
    for window in &mut merged {
        // Stable: equal tuples keep root order, then each root's order.
        window
            .entries
            .sort_by_key(|entry| entry.resolver.map(|resolver| resolver.key()));
        window.entries.truncate(ANCHOR_LIST);
    }
    merged
}

/// The merged delivery-unit order over every root's batch:
///
/// 1. tier-1 items from all roots first: an anchored query's by anchor, then
///    the resolver tuple, then root order, then each root's own order
///    (context-v2 § Resolver order); otherwise by root order, then in each
///    root's own tier-1 order (§ Two-tier query: most specific run first,
///    then path, start within a run);
/// 2. tier-2 items by reciprocal-rank fusion `1/(60 + rank)`, where `rank` is
///    the item's 1-based position in its root's tier-2 list; ties break by
///    root order, path, start.
///
/// File outlines (tier 4) are not ranked here; the context merge places them
/// after the merged units.
fn merged_units(batches: &[RootBatch]) -> Vec<(usize, RankedItem)> {
    #[derive(Clone)]
    struct Unit<'a> {
        root: usize,
        path: &'a str,
        start: u64,
        rrf: f64,
        item: &'a RankedItem,
    }
    let mut tier1: Vec<Unit> = Vec::new();
    let mut tier2: Vec<Unit> = Vec::new();
    for (root, entry) in batches.iter().enumerate() {
        let mut rank2 = 0usize;
        for item in &entry.batch.items {
            let Some(handle) = &item.handle else { continue };
            let unit = Unit {
                root,
                path: &handle.path,
                start: handle.start,
                rrf: 0.0,
                item,
            };
            match item.tier {
                1 => tier1.push(unit),
                2 => {
                    rank2 += 1;
                    let mut ranked = unit.clone();
                    ranked.rrf = 1.0 / (RRF_K + rank2 as f64);
                    tier2.push(ranked);
                }
                _ => {}
            }
        }
    }
    // Stable: an anchored query's tier 1 by anchor and tuple, equal keys (and
    // an anchor-less tier 1) keeping root order, then each root's order.
    tier1.sort_by_key(|unit| {
        let resolver = unit.item.resolver;
        (
            resolver.is_none(),
            resolver.map(|resolver| (resolver.anchor, resolver.key())),
        )
    });
    tier2.sort_by(|a, b| {
        b.rrf
            .total_cmp(&a.rrf)
            .then_with(|| a.root.cmp(&b.root))
            .then_with(|| a.path.cmp(b.path))
            .then_with(|| a.start.cmp(&b.start))
    });
    tier1
        .into_iter()
        .chain(tier2)
        .map(|unit| (unit.root, unit.item.clone()))
        .collect()
}

/// Merge the serving roots' search batches into one outcome, cut to `limit`.
/// Each root already applied the per-file cap to its own paths, so the merged
/// list keeps at most [`PER_FILE_CAP`] hits per (root, path); the merged cut
/// counts as truncation. The scalar freshness fields carry the first root's
/// final read; a multi-root response renders each root's own facts instead.
pub fn merge_search(batches: &[RootBatch], limit: usize) -> SearchOutcome {
    let merged = merged_units(batches);
    let mut truncated = batches.iter().any(|root| root.batch.counters.truncated);
    let mut hits = Vec::with_capacity(merged.len().min(limit));
    let mut overflow = Vec::new();
    for (index, (_, item)) in merged.into_iter().enumerate() {
        if index < limit {
            hits.push(item);
        } else {
            overflow.push(item);
        }
    }
    truncated |= !overflow.is_empty();
    let counters = {
        let mut counters = summed_counters(batches);
        counters.truncated = truncated;
        counters
    };
    let freshness = &batches[0].batch.freshness;
    SearchOutcome {
        workspace_id: freshness.workspace_id.clone(),
        source_revision: freshness.source_revision,
        hits: hits
            .into_iter()
            .map(|item| {
                let dense_only = item.is_dense_only();
                let handle = item.handle.expect("search units carry handles");
                let text = item
                    .forms
                    .iter()
                    .find_map(|form| match form {
                        RenderedForm::Verbatim(text) => Some(text.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                Hit {
                    path: handle.path.clone(),
                    start_line: item.start_line,
                    end_line: item.end_line,
                    handle,
                    text,
                    // 009 T002: a dense hit has no delivery-unit label; its
                    // locator names it `semantic` (never `whole_unit`).
                    label: if dense_only {
                        "semantic".to_owned()
                    } else {
                        item.label
                    },
                    tier: item.tier,
                    line: item.line,
                }
            })
            .collect(),
        pending_sources: freshness.pending_sources,
        stale_candidates: counters.stale,
        capped: counters.capped,
        candidate_limit: CANDIDATE_LIMIT,
        candidate_limit_reached: counters.candidates_full,
        truncated: counters.truncated,
        scan_state: freshness.scan_state.clone(),
        // 009 T002: the primary root's semantic word (see
        // `primary_semantic`).
        semantic: primary_semantic(batches),
    }
}

/// Merge the serving roots' context batches: the merged units, at most
/// [`CONTEXT_UNITS`] across all roots, then the outlines of the first
/// [`CONTEXT_OUTLINES`] distinct (root, path) files. Counters are summed
/// across roots; the anchor windows merge as [`merged_anchors`] describes and
/// the doors as [`merged_doors`] does.
pub fn merge_context(batches: &[RootBatch]) -> CandidateBatch {
    let merged = merged_units(batches);
    let units: Vec<RankedItem> = merged
        .iter()
        .take(CONTEXT_UNITS)
        .map(|(_, item)| (*item).clone())
        .collect();
    // Outlines of the first three distinct (root, path) files among the
    // merged units, in merged order: a file CONSUMES one of the three slots
    // even when its outline was suppressed inside its own root (an empty
    // file or a file one of those units spans renders none), so a later
    // file never takes a suppressed file's place.
    let mut outlines: Vec<RankedItem> = Vec::new();
    let mut outlined: Vec<(usize, String)> = Vec::new();
    for (root, item) in merged.iter().take(units.len()) {
        if outlined.len() == CONTEXT_OUTLINES {
            break;
        }
        let Some(handle) = &item.handle else { continue };
        if outlined
            .iter()
            .any(|(r, path)| *r == *root && path == &handle.path)
        {
            continue;
        }
        outlined.push((*root, handle.path.clone()));
        if let Some(outline) = batches[*root].batch.items.iter().find_map(|candidate| {
            (candidate.tier == TIER_OUTLINE
                && candidate
                    .handle
                    .as_ref()
                    .is_some_and(|h| h.path == handle.path))
            .then(|| candidate.clone())
        }) {
            outlines.push(outline);
        }
    }
    // The units, then the outlines.
    let mut items = units;
    items.extend(outlines);
    for (rank, item) in items.iter_mut().enumerate() {
        item.rank = rank;
    }
    let anchors = merged_anchors(batches);
    CandidateBatch {
        freshness: batches[0].batch.freshness.clone(),
        items,
        counters: summed_counters(batches),
        semantic: primary_semantic(batches),
        // 013 T003: the policy routes the PRIMARY root only (its store
        // composes the state), with the same scope suffix as semantics.
        route: primary_word(batches[0].batch.route.as_ref(), batches.len()),
        doors: merged_doors(batches, &anchors),
        anchors,
        collected: None,
    }
}

/// The merged context's doors (context-v2 § Doors; 007): requested when the
/// primary root's context requested them, and built only for the target of
/// the merged first anchor as collected - every root's head of that anchor
/// before its final read, merged as the windows are (the tuple, then root
/// order, then each root's order). Every root builds its doors in its own
/// final read, from its own window: a merged resolution takes the doors of
/// the root whose own first anchor resolved to that same definition; a
/// merged tie group of at most [`TIE_GROUP`] takes each entry's exact group
/// from its own root, cut to [`GROUP_FILES`] lines (`each`), and only when
/// no entry has one, one approximate group attributed to no entry, joined
/// from the approximate doors every tie entry's root built
/// ([`shared_approx`]); a larger merged tie group gives `ambiguous`. No
/// merged anchor with a definition, or targets their roots' final reads
/// dropped as stale, give `none`: another root's namesake is never
/// promoted.
fn merged_doors(batches: &[RootBatch], anchors: &[AnchorWindow]) -> Option<Doors> {
    batches[0].batch.doors.as_ref()?;
    let Some(window) = anchors.first().filter(|window| window.definitions > 0) else {
        return Some(Doors::unbuilt(DoorState::None));
    };
    // Each root's first anchor as collected and its doors, with its index.
    let firsts = || {
        batches.iter().enumerate().filter_map(|(root, batch)| {
            let first = batch.batch.collected.as_ref()?;
            (first.anchor == window.anchor).then_some((root, first, batch.batch.doors.as_ref()?))
        })
    };
    let mut head: Vec<_> = firsts()
        .flat_map(|(_, first, _)| first.head.iter().cloned())
        .collect();
    // Stable: equal tuples keep root order, then each root's order.
    head.sort_by_key(|(_, resolver)| resolver.map(|resolver| resolver.key()));
    head.truncate(TIE_GROUP + 1);
    let collected = CollectedAnchor {
        anchor: window.anchor.clone(),
        definitions: window.definitions,
        head,
    };
    // The doors the root collecting `target` built for it (only when it
    // survived that root's read): a group onto it, or the approximate group
    // onto no entry of that root's own tie group.
    let built_for = |target: &crate::store::SourceHandle| {
        let (root, _, doors) = firsts().find(|(_, first, _)| {
            first
                .head
                .iter()
                .any(|(handle, _)| handle.as_ref() == Some(target))
        })?;
        let group = doors
            .groups
            .iter()
            .find(|group| group.target.as_ref().is_none_or(|onto| onto == target))?;
        Some((root, doors.state, group))
    };
    Some(match collected.target() {
        DoorTarget::None => Doors::unbuilt(DoorState::None),
        DoorTarget::Ambiguous => Doors::unbuilt(DoorState::Ambiguous),
        DoorTarget::Resolved(target) => match built_for(&target) {
            Some((_, state, group)) if group.target.is_some() => Doors {
                state,
                groups: vec![group.clone()],
            },
            _ => Doors::unbuilt(DoorState::None),
        },
        DoorTarget::Tied(tied) => {
            let exact: Vec<DoorGroup> = tied
                .iter()
                .filter_map(|target| match built_for(target)? {
                    (_, DoorState::Exact | DoorState::Each, group) if group.target.is_some() => {
                        let mut group = group.clone();
                        group.more_files += group.lines.len().saturating_sub(GROUP_FILES);
                        group.lines.truncate(GROUP_FILES);
                        Some(group)
                    }
                    _ => None,
                })
                .collect();
            if exact.is_empty() {
                shared_approx(tied.iter().filter_map(built_for))
            } else {
                Doors {
                    state: DoorState::Each,
                    groups: exact,
                }
            }
        }
    })
}

/// The one approximate group of a merged tie group without exact doors,
/// attributed to no entry, from the groups its entries' roots built, as
/// `(root, state, group)` in merged order (root order, then each root's
/// order): each root's approximate group once, one line per file (a line
/// keeps its root's handle), at most [`DOOR_FILES`] lines; every root's
/// further files and every line past the cap count in `more_files`. `none`
/// when no root built approximate doors.
fn shared_approx<'a>(built: impl Iterator<Item = (usize, DoorState, &'a DoorGroup)>) -> Doors {
    let mut roots: Vec<usize> = Vec::new();
    let mut shared = DoorGroup {
        target: None,
        lines: Vec::new(),
        more_files: 0,
    };
    for (root, state, group) in built {
        if state != DoorState::Approx || roots.contains(&root) {
            continue;
        }
        roots.push(root);
        shared.more_files += group.more_files;
        for line in &group.lines {
            let listed = shared.lines.iter().any(|known| {
                known.unit.workspace_id == line.unit.workspace_id
                    && known.unit.path == line.unit.path
            });
            if listed {
                continue;
            }
            if shared.lines.len() == DOOR_FILES {
                shared.more_files += 1;
            } else {
                shared.lines.push(line.clone());
            }
        }
    }
    if roots.is_empty() {
        return Doors::unbuilt(DoorState::None);
    }
    Doors {
        state: DoorState::Approx,
        groups: vec![shared],
    }
}

/// 009 T002: semantic evidence exists only for the PRIMARY root's store.
/// A single-root owner reports its word unchanged; with more than one
/// serving root the word says it covers the primary root only, so the merge
/// never silently drops or broadens it.
fn primary_semantic(batches: &[RootBatch]) -> Option<String> {
    primary_word(batches[0].batch.semantic.as_ref(), batches.len())
}

/// A primary-root-only header word, suffixed when other roots also serve.
fn primary_word(word: Option<&String>, serving: usize) -> Option<String> {
    let word = word?;
    Some(if serving > 1 {
        format!("{word}; primary root only")
    } else {
        word.clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::response::Freshness;
    use crate::store::{RankedItem, SourceHandle};

    fn freshness(revision: u64) -> Freshness {
        Freshness {
            workspace_id: format!("{revision:064x}"),
            source_revision: revision,
            scan_state: "complete".to_owned(),
            pending_sources: 0,
            indexed_snapshot: format!("revision={revision}"),
        }
    }

    fn unit(tier: u8, path: &str, start: u64) -> RankedItem {
        RankedItem {
            tier,
            rank: 0,
            score: 0.0,
            handle: Some(SourceHandle {
                workspace_id: "w".repeat(64),
                path: path.to_owned(),
                sha256: "h".repeat(64),
                start,
                end: start + 10,
            }),
            start_line: 1,
            end_line: 2,
            line: 1,
            label: "fn x".to_owned(),
            lang: None,
            semantic: None,
            resolver: None,
            forms: vec![RenderedForm::Verbatim("fn x() {}".to_owned())],
        }
    }

    fn batch(items: Vec<RankedItem>) -> CandidateBatch {
        CandidateBatch {
            freshness: freshness(1),
            items,
            counters: CandidateCounters::default(),
            semantic: None,
            route: None,
            anchors: Vec::new(),
            doors: None,
            collected: None,
        }
    }

    #[test]
    fn labels_sanitize_control_characters_and_cut_at_a_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("foo");
        assert_eq!(label_for(&plain), "foo");
        let control = dir.path().join("fo\u{7}o");
        assert_eq!(label_for(&control), "fo?o");
        // 33 ASCII bytes cut to 32; a multi-byte tail cuts at the boundary.
        let long = dir.path().join("a".repeat(33));
        assert_eq!(label_for(&long).len(), 32);
        let multibyte = dir.path().join(format!("{}é", "a".repeat(31)));
        let label = label_for(&multibyte);
        assert_eq!(label.len(), 31, "the 2-byte é does not fit: {label:?}");
    }

    #[test]
    fn references_parse_on_the_first_equals() {
        let spec = parse_reference("/a=/b=c").unwrap();
        assert_eq!(spec.root, Path::new("/a"));
        assert_eq!(spec.store, Path::new("/b=c"));
        assert!(parse_reference("=store").is_err());
        assert!(parse_reference("root=").is_err());
        assert!(parse_reference("root").is_err());
    }

    #[test]
    fn admission_refuses_count_duplicates_and_nesting() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        let c = dir.path().join("c");
        for root in [&a, &b, &c] {
            std::fs::create_dir_all(root).unwrap();
        }
        let spec = |root: &Path| ReferenceSpec {
            root: root.to_path_buf(),
            store: dir.path().join("store"),
        };
        let too_many = (0..9).map(|_| spec(&a)).collect::<Vec<_>>();
        assert_eq!(
            validate_admission(&a, &too_many).unwrap_err().code(),
            "too_many_roots"
        );
        assert_eq!(
            validate_admission(&a, &[spec(&a)]).unwrap_err().code(),
            "duplicate_root"
        );
        let nested = dir.path().join("a/inner");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(
            validate_admission(&a, &[spec(&nested)]).unwrap_err().code(),
            "nested_root"
        );
        // Component boundary: `bc` is not inside `b`.
        let sibling = dir.path().join("bc");
        std::fs::create_dir_all(&sibling).unwrap();
        let admitted = validate_admission(&a, &[spec(&b), spec(&sibling)]).unwrap();
        assert_eq!(
            admitted
                .iter()
                .map(|root| root.alias.as_str())
                .collect::<Vec<_>>(),
            vec!["primary", "ref1", "ref2"]
        );
        // Distinct roots have distinct ws16; two foo-named roots keep labels.
        let foo2 = dir.path().join("foo");
        std::fs::create_dir_all(&foo2).unwrap();
        let admitted = validate_admission(&a, &[spec(&foo2)]).unwrap();
        assert_eq!(admitted[1].label, "foo");
    }

    #[test]
    fn merged_units_order_tier_one_by_root_then_tier_two_by_rrf() {
        // Root 0 tier 2 ranks: b (rank 1), a (rank 2); root 1: a (rank 1).
        // RRF: ref1/a and primary/b tie at 1/61 and break by root order
        // (primary first); primary/a follows at 1/62.
        let first = RootBatch {
            alias: "primary".to_owned(),
            batch: batch(vec![unit(2, "src/b.rs", 0), unit(2, "src/a.rs", 40)]),
        };
        let second = RootBatch {
            alias: "ref1".to_owned(),
            batch: batch(vec![unit(1, "src/a.rs", 0), unit(2, "src/a.rs", 0)]),
        };
        let merged = merge_search(&[first, second], 10);
        let paths: Vec<&str> = merged.hits.iter().map(|hit| hit.path.as_str()).collect();
        assert_eq!(paths, vec!["src/a.rs", "src/b.rs", "src/a.rs", "src/a.rs"]);
        // The merged cut counts as truncation: two units survive the merge,
        // the cut to one drops the rest.
        let cut = merge_search(
            &[RootBatch {
                alias: "primary".to_owned(),
                batch: batch(vec![unit(1, "src/a.rs", 0), unit(2, "src/b.rs", 0)]),
            }],
            1,
        );
        assert_eq!(cut.hits.len(), 1);
        assert!(cut.truncated);
    }

    #[test]
    fn merged_units_keep_each_roots_own_tier_one_order() {
        // Each root ranks its most specific run first, not by path; the
        // merge keeps that order inside each root, roots in admission order.
        let first = RootBatch {
            alias: "primary".to_owned(),
            batch: batch(vec![unit(1, "src/z.rs", 0), unit(1, "src/a.rs", 0)]),
        };
        let second = RootBatch {
            alias: "ref1".to_owned(),
            batch: batch(vec![unit(1, "src/y.rs", 0), unit(1, "src/b.rs", 0)]),
        };
        let merged = merge_search(&[first, second], 10);
        let paths: Vec<&str> = merged.hits.iter().map(|hit| hit.path.as_str()).collect();
        assert_eq!(paths, vec!["src/z.rs", "src/a.rs", "src/y.rs", "src/b.rs"]);
    }

    #[test]
    fn merged_context_keeps_the_units_then_one_outline_per_root_file() {
        let mut outline = unit(4, "src/a.rs", 0);
        outline.forms = vec![RenderedForm::Outline("outline".to_owned())];
        let first = RootBatch {
            alias: "primary".to_owned(),
            batch: batch(vec![unit(1, "src/a.rs", 0), outline.clone()]),
        };
        let second = RootBatch {
            alias: "ref1".to_owned(),
            batch: batch(vec![unit(1, "src/a.rs", 5), outline]),
        };
        let merged = merge_context(&[first, second]);
        let rendered: Vec<String> = merged
            .items
            .iter()
            .map(|item| match &item.forms[0] {
                RenderedForm::Outline(_) => "outline".to_owned(),
                _ => item.handle.as_ref().unwrap().path.clone(),
            })
            .collect();
        assert_eq!(rendered, ["src/a.rs", "src/a.rs", "outline", "outline"]);
        assert!(merged.doors.is_none(), "no root requested doors");
    }

    /// A root whose only anchor `parse` lists `entries` (as collected and as
    /// validated) and whose context requested doors and built `doors`.
    fn with_doors(alias: &str, entries: Vec<RankedItem>, doors: Doors) -> RootBatch {
        let mut root = anchored(
            alias,
            vec![("parse", (1, 0), entries.len() as u64, entries)],
        );
        root.batch.collected = Some(CollectedAnchor::of(&root.batch.anchors[0]));
        root.batch.doors = Some(doors);
        root
    }

    /// `state` doors of one group onto `target` (`None`: onto no entry), of
    /// `lines` door lines and 3 more files.
    fn one_group(state: DoorState, target: Option<&RankedItem>, lines: usize) -> Doors {
        let line = |n: usize| crate::store::DoorLine {
            unit: SourceHandle {
                workspace_id: "w".repeat(64),
                path: format!("use{n}.rs"),
                sha256: "h".repeat(64),
                start: 0,
                end: 10,
            },
            line: 1,
            label: "fn user".to_owned(),
            text: "user();".to_owned(),
            more: 0,
        };
        Doors {
            state,
            groups: vec![DoorGroup {
                target: target.and_then(|item| item.handle.clone()),
                lines: (0..lines).map(line).collect(),
                more_files: 3,
            }],
        }
    }

    /// Approximate doors of one group onto `target` (`None`: onto no entry)
    /// built in the root whose workspace is `root`: `lines` door lines and
    /// `more_files` more.
    fn approx(target: Option<&RankedItem>, root: &str, lines: usize, more_files: usize) -> Doors {
        let mut doors = one_group(DoorState::Approx, target, lines);
        let group = &mut doors.groups[0];
        for line in &mut group.lines {
            line.unit.workspace_id = root.to_owned();
        }
        group.more_files = more_files;
        doors
    }

    /// The approximate doors onto no entry of `parts`' lines in order.
    fn shared(parts: &[(&str, usize)], more_files: usize) -> Doors {
        let mut doors = approx(None, "", 0, more_files);
        for (root, lines) in parts {
            let part = approx(None, root, *lines, 0);
            doors.groups[0]
                .lines
                .extend(part.groups[0].lines.iter().cloned());
        }
        doors
    }

    /// 007 (context-v2 § Doors): the merged first anchor decides; a merged
    /// resolution takes the doors of the root that resolved to the same
    /// definition; a merged tie group of at most four takes each entry's
    /// exact group from its own root, cut to four lines (`each`), even when
    /// each root resolved on its own; when none has exact doors, one
    /// approximate group onto no entry joins every tie entry's root's own
    /// approximate doors (each root's once, in root order, sixteen lines);
    /// five tied across roots are `ambiguous`; an entry its root dropped as
    /// stale gets no group and no lower namesake takes its place.
    #[test]
    fn merged_doors_follow_the_merged_first_anchor() {
        use DoorState::{Approx, Each, Exact};
        let qualified = definition("a.rs", (1, 0), 1, true);
        let plain = definition("b.rs", (1, 0), 0, true);
        // Root order puts the plain definition first; the qualifier wins.
        let merged = merge_context(&[
            with_doors(
                "primary",
                vec![plain.clone()],
                one_group(Exact, Some(&plain), 0),
            ),
            with_doors(
                "ref1",
                vec![qualified.clone()],
                one_group(Exact, Some(&qualified), 0),
            ),
        ]);
        let doors = merged.doors.expect("the primary requested doors");
        assert_eq!(doors, one_group(Exact, Some(&qualified), 0));
        // Equal tuples in two roots: each root resolved, the merge is a tie
        // group of two, each entry with its own root's group cut to four.
        let other = definition("c.rs", (1, 0), 0, true);
        let lower = definition("d.rs", (1, 0), 0, false);
        let pair = |primary: Doors, ref1: Doors| {
            merge_context(&[
                with_doors("primary", vec![plain.clone()], primary),
                with_doors("ref1", vec![other.clone(), lower.clone()], ref1),
            ])
            .doors
            .expect("the primary requested doors")
        };
        let doors = pair(
            one_group(Exact, Some(&plain), 6),
            one_group(Exact, Some(&other), 2),
        );
        assert_eq!(doors.state, Each);
        let groups: Vec<(&str, usize, usize)> = doors
            .groups
            .iter()
            .map(|group| {
                let target = group.target.as_ref().unwrap();
                (target.path.as_str(), group.lines.len(), group.more_files)
            })
            .collect();
        assert_eq!(groups, [("b.rs", 4, 5), ("c.rs", 2, 3)]);
        // An entry without exact doors gets no group.
        let doors = pair(
            one_group(Approx, Some(&plain), 1),
            one_group(Exact, Some(&other), 2),
        );
        assert_eq!(doors.state, Each);
        assert_eq!(doors.groups, one_group(Exact, Some(&other), 2).groups);
        // No entry has exact doors: one approximate group onto no entry,
        // every tie entry's root's lines in root order, their more files
        // summed.
        let doors = pair(
            approx(Some(&plain), "p", 1, 3),
            approx(Some(&other), "r", 2, 3),
        );
        assert_eq!(doors, shared(&[("p", 1), ("r", 2)], 6));
        // An empty group in the primary hides nothing.
        let doors = pair(
            approx(Some(&plain), "p", 0, 0),
            approx(Some(&other), "r", 2, 3),
        );
        assert_eq!(doors, shared(&[("r", 2)], 3));
        // Sixteen lines in all; the rest are counted.
        let doors = pair(
            approx(Some(&plain), "p", 10, 3),
            approx(Some(&other), "r", 10, 3),
        );
        assert_eq!(doors, shared(&[("p", 10), ("r", 6)], 3 + 3 + 4));
        // Two tied entries in one root: that root's group once.
        let twin = definition("c2.rs", (1, 0), 0, true);
        let doors = merge_context(&[
            with_doors(
                "primary",
                vec![plain.clone()],
                approx(Some(&plain), "p", 1, 0),
            ),
            with_doors("ref1", vec![other.clone(), twin], approx(None, "r", 2, 0)),
        ])
        .doors
        .expect("the primary requested doors");
        assert_eq!(doors, shared(&[("p", 1), ("r", 2)], 0));
        // The primary dropped its entry as stale: it gets no group, and the
        // lower namesake in ref1 never takes its place.
        let doors = pair(
            Doors::unbuilt(DoorState::None),
            one_group(Exact, Some(&other), 2),
        );
        assert_eq!(doors.state, Each);
        assert_eq!(doors.groups, one_group(Exact, Some(&other), 2).groups);
        let doors = pair(
            Doors::unbuilt(DoorState::None),
            one_group(Approx, Some(&other), 2),
        );
        assert_eq!(doors, one_group(Approx, None, 2));
        // Five tied across roots are ambiguous though each root's own tie
        // group is within the bound.
        let tied = |paths: &[&str]| -> Vec<RankedItem> {
            paths
                .iter()
                .map(|path| definition(path, (1, 0), 0, true))
                .collect()
        };
        let merged = merge_context(&[
            with_doors(
                "primary",
                tied(&["e.rs", "f.rs", "g.rs"]),
                Doors::unbuilt(Each),
            ),
            with_doors("ref1", tied(&["h.rs", "i.rs"]), Doors::unbuilt(Each)),
        ]);
        assert_eq!(merged.doors.unwrap(), Doors::unbuilt(DoorState::Ambiguous));
        // A primary that did not request doors requests none for the merge.
        let mut quiet = with_doors(
            "primary",
            vec![plain.clone()],
            one_group(Exact, Some(&plain), 0),
        );
        quiet.batch.doors = None;
        let merged = merge_context(&[
            quiet,
            with_doors("ref1", vec![other], Doors::unbuilt(DoorState::None)),
        ]);
        assert!(merged.doors.is_none());
    }

    /// One tier-1 definition of the anchor at `order`, with its tuple.
    fn definition(path: &str, order: (u8, usize), qualifiers: u64, exact: bool) -> RankedItem {
        RankedItem {
            resolver: Some(crate::store::Resolver {
                anchor: order,
                qualifiers,
                exact,
                role: 0,
                name: (0, 0),
            }),
            ..unit(1, path, 0)
        }
    }

    /// One window: anchor, order, definitions, entries in the root's own
    /// order.
    type Window<'a> = (&'a str, (u8, usize), u64, Vec<RankedItem>);

    /// A root whose windows are also its tier-1 items.
    fn anchored(alias: &str, windows: Vec<Window<'_>>) -> RootBatch {
        let items = windows
            .iter()
            .flat_map(|(_, _, _, entries)| entries.clone())
            .collect();
        let anchors = windows
            .into_iter()
            .map(|(anchor, order, definitions, entries)| AnchorWindow {
                anchor: anchor.to_owned(),
                order,
                definitions,
                entries,
            })
            .collect();
        RootBatch {
            alias: alias.to_owned(),
            batch: CandidateBatch {
                anchors,
                ..batch(items)
            },
        }
    }

    /// 007 (context-v2 § Resolver order): an anchor's definitions are summed
    /// over the roots, and its merged window orders by the tuple, then root
    /// order, then each root's own order — a root's `key_hash` cut and path
    /// listing are never compared across roots. Anchors merge in anchor
    /// order, one taken from a single root included.
    #[test]
    fn merged_windows_order_by_tuple_then_root_then_each_roots_order() {
        let dup = (1, 0);
        let engine = (3, 20);
        let roots = || {
            [
                anchored(
                    "primary",
                    vec![(
                        "dup",
                        dup,
                        70,
                        vec![
                            definition("z.rs", dup, 0, true),
                            definition("a.rs", dup, 0, true),
                            definition("t.rs", dup, 0, false),
                        ],
                    )],
                ),
                anchored(
                    "ref1",
                    vec![
                        (
                            "dup",
                            dup,
                            5,
                            vec![
                                definition("q.rs", dup, 1, false),
                                definition("c.rs", dup, 0, true),
                            ],
                        ),
                        (
                            "Engine",
                            engine,
                            1,
                            vec![definition("e.rs", engine, 0, true)],
                        ),
                    ],
                ),
            ]
        };
        let merged = merge_context(&roots());
        let anchors: Vec<(&str, u64)> = merged
            .anchors
            .iter()
            .map(|window| (window.anchor.as_str(), window.definitions))
            .collect();
        assert_eq!(anchors, [("dup", 75), ("Engine", 1)]);
        let paths = |items: &[RankedItem]| -> Vec<String> {
            items
                .iter()
                .map(|item| item.handle.as_ref().unwrap().path.clone())
                .collect()
        };
        let want = ["q.rs", "z.rs", "a.rs", "c.rs", "t.rs"];
        assert_eq!(paths(&merged.anchors[0].entries), want);
        // Search's merged tier 1 follows the windows in anchor order.
        let search = merge_search(&roots(), 10);
        let hits: Vec<&str> = search.hits.iter().map(|hit| hit.path.as_str()).collect();
        assert_eq!(hits, ["q.rs", "z.rs", "a.rs", "c.rs", "t.rs", "e.rs"]);
    }
}
