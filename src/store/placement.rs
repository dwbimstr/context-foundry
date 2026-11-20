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
