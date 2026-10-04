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

use crate::FoundryError;
use crate::store::{
    CandidateBatch, CandidateCounters, Hit, RankedItem, RenderedForm, SearchOutcome, TIER_GRAPH,
    TIER_OUTLINE,
};
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

/// The worst graph coverage across roots when the strategy resolved to graph:
/// invalid over stale over unavailable over ok. Identical rows in two roots
/// stay distinct; deduplication stays inside each root.
fn worst_graph(a: Option<&'static str>, b: Option<&'static str>) -> Option<&'static str> {
    let severity = |state: &str| match state {
        "graph_invalid" => 3,
        "graph_stale" => 2,
        "graph_unavailable" => 1,
        _ => 0,
    };
    match (a, b) {
        (None, other) | (other, None) => other,
        (Some(a), Some(b)) => Some(if severity(a) >= severity(b) { a } else { b }),
    }
}

fn summed_counters(batches: &[RootBatch]) -> CandidateCounters {
    let mut counters = CandidateCounters::default();
    for root in batches {
        let batch = &root.batch;
        counters.stale = counters.stale.saturating_add(batch.counters.stale);
        counters.capped = counters.capped.saturating_add(batch.counters.capped);
        counters.candidates_full |= batch.counters.candidates_full;
        counters.truncated |= batch.counters.truncated;
        counters.graph = worst_graph(counters.graph, batch.counters.graph);
    }
    counters
}

/// The merged delivery-unit order over every root's batch:
///
/// 1. tier-1 items from all roots first, by root order, then path, then start;
/// 2. tier-2 items by reciprocal-rank fusion `1/(60 + rank)`, where `rank` is
///    the item's 1-based position in its root's tier-2 list; ties break by
///    root order, path, start.
///
/// Graph items (tier 3) and file outlines (tier 4) are not ranked here; the
/// context merge places them after the merged units.
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
    tier1.sort_by(|a, b| {
        a.root
            .cmp(&b.root)
            .then_with(|| a.path.cmp(b.path))
            .then_with(|| a.start.cmp(&b.start))
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
                    label: item.label,
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
    }
}

/// Merge the serving roots' context batches: the first [`CONTEXT_UNITS`]
/// merged units, graph items after the first merged unit in root order
/// (carrying their root's alias), then the remaining units and the outlines
/// of the first [`CONTEXT_OUTLINES`] distinct (root, path) files. Counters
/// are summed across roots; graph takes the worst coverage.
pub fn merge_context(batches: &[RootBatch]) -> CandidateBatch {
    let merged = merged_units(batches);
    let units: Vec<RankedItem> = merged
        .iter()
        .take(CONTEXT_UNITS)
        .map(|(_, item)| (*item).clone())
        .collect();
    // Graph items, in root order, each carrying its seed root's alias.
    let mut graph: Vec<RankedItem> = Vec::new();
    for root in batches {
        for item in &root.batch.items {
            if item.tier != TIER_GRAPH {
                continue;
            }
            let mut aliased = item.clone();
            aliased.forms = item
                .forms
                .iter()
                .map(|form| match form {
                    RenderedForm::Line(text) => {
                        RenderedForm::Line(format!("{} {text}", root.alias))
                    }
                    other => other.clone(),
                })
                .collect();
            graph.push(aliased);
        }
    }
    // Outlines of the first three distinct (root, path) files among the
    // merged units, in merged order: a file CONSUMES one of the three slots
    // even when its outline was suppressed inside its own root (an empty
    // file or a file one of those units spans renders none), so a later
    // file never takes a suppressed file's place.
    let mut outlines: Vec<RankedItem> = Vec::new();
    let mut outlined: Vec<(usize, String)> = Vec::new();
    for (root, item) in merged.iter().take(CONTEXT_UNITS) {
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
    // The first unit, then graph items, then the remaining units, then
    // outlines: a fitting first unit precedes graph items.
    let mut rest = units.into_iter();
    let mut items: Vec<RankedItem> = rest.next().into_iter().collect();
    items.append(&mut graph);
    items.extend(rest);
    items.extend(outlines);
    for (rank, item) in items.iter_mut().enumerate() {
        item.rank = rank;
    }
    CandidateBatch {
        freshness: batches[0].batch.freshness.clone(),
        items,
        counters: summed_counters(batches),
    }
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
            forms: vec![RenderedForm::Verbatim("fn x() {}".to_owned())],
        }
    }

    fn batch(items: Vec<RankedItem>) -> CandidateBatch {
        CandidateBatch {
            freshness: freshness(1),
            items,
            counters: CandidateCounters::default(),
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
    fn merged_context_keeps_graph_items_after_the_first_unit_with_aliases() {
        let mut graph = unit(3, "ignored", 0);
        graph.handle = None;
        graph.forms = vec![RenderedForm::Line(
            "src/a.rs:1 (f) --calls--> src/b.rs:2 (g) [manual; provider=p@1]".to_owned(),
        )];
        let mut outline = unit(4, "src/a.rs", 0);
        outline.forms = vec![RenderedForm::Outline("outline".to_owned())];
        let first = RootBatch {
            alias: "primary".to_owned(),
            batch: batch(vec![unit(1, "src/a.rs", 0), graph.clone(), outline.clone()]),
        };
        let second = RootBatch {
            alias: "ref1".to_owned(),
            batch: batch(vec![unit(1, "src/a.rs", 5), graph, outline]),
        };
        let merged = merge_context(&[first, second]);
        let rendered: Vec<String> = merged
            .items
            .iter()
            .map(|item| match &item.forms[0] {
                RenderedForm::Line(text) => format!("edge {text}"),
                RenderedForm::Outline(_) => "outline".to_owned(),
                _ => item.handle.as_ref().unwrap().path.clone(),
            })
            .collect();
        // First merged unit, both roots' graph items in root order, the
        // remaining unit, then one outline per distinct (root, path) file.
        assert_eq!(
            rendered,
            vec![
                "src/a.rs".to_owned(),
                "edge primary src/a.rs:1 (f) --calls--> src/b.rs:2 (g) [manual; provider=p@1]"
                    .to_owned(),
                "edge ref1 src/a.rs:1 (f) --calls--> src/b.rs:2 (g) [manual; provider=p@1]"
                    .to_owned(),
                "src/a.rs".to_owned(),
                "outline".to_owned(),
                "outline".to_owned(),
            ]
        );
    }
}
