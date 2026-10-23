//! Placement: the order the dense and lexical lanes are offered in (spec 009
//! § T004 "Placement") and the per-file cap and cut both searches apply to
//! an offered order. Callers keep their own freshness checks, duplicate
//! handling and trace events.
use super::*;

/// A delivery unit within one store: `(path, start, end)`.
pub(super) type UnitRef<'a> = (&'a str, u64, u64);

/// One placed unit and, for a dense location, its representation.
#[cfg(feature = "semantic")]
#[derive(Clone, Copy)]
pub(super) struct Placed<'a> {
    pub(super) unit: UnitRef<'a>,
    pub(super) kind: Option<crate::neural::cache::CardKind>,
}

/// What refinement contributed to one dense window's placement.
#[cfg(feature = "semantic")]
pub(super) struct Refined {
    /// Refined hits scored.
    pub(super) hits: usize,
    /// Their locations admitted by the path filter and the final read.
    pub(super) eligible: usize,
    /// Files refinement localized within.
    pub(super) files: usize,
}

/// How the dense and lexical lanes merge.
#[cfg(feature = "semantic")]
pub(super) enum Order {
    /// Every dense unit in similarity order, then the lexical units.
    DenseFirst,
    /// One unit of each lane per step; the lexical lane leads unless
    /// refinement admitted a location.
    Interleave {
        policy: &'static str,
        coverage: &'static str,
        refined: Refined,
    },
}

#[cfg(feature = "semantic")]
impl Order {
    /// Coverage reports availability, not whether dense relevance outranks
    /// lexical evidence. Experimental body/hierarchy placement uses the same
    /// bounded hybrid order before and after preparation completes. Prose
    /// keeps the partial/address and coarse-only interleave; complete
    /// address coverage and ordinary search place the dense lane first.
    pub(super) fn resolve(
        prose: bool,
        coverage: &'static str,
        dense: &crate::neural::query::DenseWindow,
        refined: Refined,
    ) -> Self {
        if !prose {
            return Self::DenseFirst;
        }
        let policy = if dense.has_body_representations() {
            "body_prose_interleave_v1"
        } else if dense.file_overviews_only() {
            "coarse_prose_interleave_v1"
        } else if coverage == "partial" {
            "partial_prose_interleave_v1"
        } else {
            return Self::DenseFirst;
        };
        Self::Interleave {
            policy,
            coverage,
            refined,
        }
    }
}

/// The placed order: the `dense` units (in similarity order) and each
/// `lexical` candidate's selection unit (in today's order), merged as
/// `order` says. An interleave records its `placement_policy`.
#[cfg(feature = "semantic")]
pub(super) fn place<'a>(
    order: Order,
    mut dense: Vec<Placed<'a>>,
    lexical: &[&'a Candidate],
    prose: bool,
    control: &crate::Control,
) -> Vec<Placed<'a>> {
    let mut lexical_order = lexical.iter().map(|&candidate| Placed {
        unit: candidate.selection_unit(prose),
        kind: None,
    });
    let Order::Interleave {
        policy,
        coverage,
        refined,
    } = order
    else {
        dense.extend(lexical_order);
        return dense;
    };
    // Independently embedded body passages precede broad lexical documents.
    // Without eligible refinement, lexical evidence goes first. Neither
    // choice depends on coverage completeness.
    let lexical_first = refined.eligible == 0;
    let mut placed = Vec::with_capacity(dense.len() + lexical_order.len());
    let mut dense_order = dense.into_iter();
    loop {
        let (lexical, dense) = (lexical_order.next(), dense_order.next());
        if lexical.is_none() && dense.is_none() {
            break;
        }
        if lexical_first {
            placed.extend(lexical);
            placed.extend(dense);
        } else {
            placed.extend(dense);
            placed.extend(lexical);
        }
    }
    if let Some(trace) = control.trace() {
        trace.record(serde_json::json!({"stage":"placement_policy","policy":policy,"coverage":coverage,"lexical_first":lexical_first,"refined_hits":refined.hits,"eligible_refined_locations":refined.eligible,"refinement_files":refined.files}));
    }
    placed
}

/// `placed` with each callable's matching passages kept together across
/// both lanes ([`super::lexical::callable_passage_order`]); a dense unit is
/// never promoted as a matching lexical document.
#[cfg(feature = "semantic")]
pub(super) fn group_callables<'a>(
    placed: &[Placed<'a>],
    by_document: &std::collections::HashMap<UnitRef<'a>, &'a Candidate>,
    control: &crate::Control,
) -> FResult<Vec<Placed<'a>>> {
    let references: Vec<_> = placed
        .iter()
        .map(|item| match item.kind {
            None => by_document.get(&item.unit).copied(),
            Some(_) => None,
        })
        .collect();
    let order =
        super::lexical::callable_passage_order(&references, control, "semantic_owner_grouping")?;
    Ok(order.into_iter().map(|index| placed[index]).collect())
}

/// The baseline's per-file cap and cut over an offered order: at most
/// [`PER_FILE_CAP`] items per file, then at most `limit`. Diversity gets
/// first choice, but a sparse prose result must not discard additional
/// evidence solely because it lives in the same file: prose defers a capped
/// item and [`Self::finish`] backfills the slots left in offered order.
pub(super) struct Cut<'a, T> {
    per_file: std::collections::BTreeMap<&'a str, usize>,
    kept: Vec<T>,
    deferred: Vec<(usize, T)>,
    limit: usize,
    prose: bool,
}

impl<'a, T: Copy> Cut<'a, T> {
    pub(super) fn new(limit: usize, prose: bool) -> Self {
        Self {
            per_file: std::collections::BTreeMap::new(),
            kept: Vec::new(),
            deferred: Vec::new(),
            limit,
            prose,
        }
    }

    /// Offers `item` of file `path` at `rank`: its outcome (`kept`,
    /// `per_file_cap` or `limit`), or `None` when prose defers it. A cap
    /// counts as `capped`, a cut as `truncated`.
    pub(super) fn offer(
        &mut self,
        rank: usize,
        path: &'a str,
        item: T,
        counters: &mut CandidateCounters,
    ) -> Option<&'static str> {
        let count = self.per_file.entry(path).or_default();
        if *count == PER_FILE_CAP {
            if self.prose {
                self.deferred.push((rank, item));
                return None;
            }
            counters.capped += 1;
            return Some("per_file_cap");
        }
        *count += 1;
        if self.kept.len() == self.limit {
            counters.truncated = true;
            return Some("limit");
        }
        self.kept.push(item);
        Some("kept")
    }

    /// The kept items, after the deferred ones fill the slots left; `report`
    /// receives each deferred item's rank and outcome (`kept` or `limit`).
    pub(super) fn finish(
        self,
        counters: &mut CandidateCounters,
        mut report: impl FnMut(usize, T, &'static str),
    ) -> Vec<T> {
        let Self {
            mut kept,
            deferred,
            limit,
            ..
        } = self;
        for (rank, item) in deferred {
            if kept.len() < limit {
                report(rank, item, "kept");
                kept.push(item);
            } else {
                counters.truncated = true;
                report(rank, item, "limit");
            }
        }
        kept
    }
}
