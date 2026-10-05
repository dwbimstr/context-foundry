//! 009 T002 candidate merge: the D001 deterministic ordering. Exact
//! definitions come first, then reciprocal-rank fusion with k = 60 over the
//! lexical top 256 and the dense top 64, ties ordered by path then start —
//! ordinary ranking, never a learned reranker. Pure functions over unit
//! identities so the order can be unit-tested without a store.
//!
//! A unit's identity is `(path, start, end)` — the span the evidence names
//! (001 § Deduplication: identity is path, hash, start and end; the source
//! hash is common to every unit of one path in one read). A lexical delivery
//! unit and a dense embedding unit are the SAME candidate only when their
//! spans coincide; overlapping but different spans stay separate candidates,
//! each delivered and accounted on its own.

/// The fusion constant (D001): `score = 1 / (K + rank)`, ranks 1-based.
pub const RRF_K: usize = 60;

/// One unit identity in the fusion.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MergeUnit {
    pub path: String,
    pub start: u64,
    pub end: u64,
}

/// One fused candidate and where it came from.
#[derive(Clone, Debug)]
pub struct FusedCandidate {
    pub unit: MergeUnit,
    /// 1 for a tier-1 exact definition (kept in its own order ahead of the
    /// fusion), otherwise 2.
    pub tier: u8,
    /// The fused reciprocal-rank score (0 for tier-1 entries).
    pub score: f32,
    /// The 0-based dense window rank when the unit appeared in it.
    pub dense_rank: Option<usize>,
    /// True when the lexical top-256 list carried the unit.
    pub lexical: bool,
}

/// The order of the fusion: tier-1 exact definitions first (in their own
/// order), then every other unit by descending fused score with ties by
/// path, then start, then end. Every arm is a ranked list of UNIQUE unit
/// identities: several search documents of one delivery unit (a split
/// oversized region) or several dense locations of one unit count once, at
/// the unit's earliest position, and a unit's rank is its position among
/// the arm's unique units. A unit present in both lists scores the sum of
/// its two reciprocal ranks, which is how RRF merges duplicates. A unit
/// already in tier 1 is not repeated.
pub fn fuse(
    tier1: &[MergeUnit],
    lexical: &[MergeUnit],
    dense: &[MergeUnit],
) -> Vec<FusedCandidate> {
    let mut exact: std::collections::HashSet<&MergeUnit> = std::collections::HashSet::new();
    let mut out: Vec<FusedCandidate> = tier1
        .iter()
        .filter(|unit| exact.insert(*unit))
        .map(|unit| FusedCandidate {
            unit: unit.clone(),
            tier: 1,
            score: 0.0,
            dense_rank: None,
            lexical: false,
        })
        .collect();
    let mut index: std::collections::HashMap<MergeUnit, usize> = std::collections::HashMap::new();
    let mut fused: Vec<FusedCandidate> = Vec::new();
    let mut add = |unit: &MergeUnit, rank: usize, from_dense: bool| {
        // 1-based rank in the formula: the best hit scores 1 / (60 + 1).
        let contribution = 1.0 / (RRF_K + rank + 1) as f32;
        match index.get(unit) {
            Some(&at) => {
                fused[at].score += contribution;
                if from_dense {
                    fused[at].dense_rank = Some(rank);
                } else {
                    fused[at].lexical = true;
                }
            }
            None => {
                index.insert(unit.clone(), fused.len());
                fused.push(FusedCandidate {
                    unit: unit.clone(),
                    tier: 2,
                    score: contribution,
                    dense_rank: from_dense.then_some(rank),
                    lexical: !from_dense,
                });
            }
        }
    };
    // Each arm contributes at most once per unit: its unique units, in
    // first-occurrence order, are its ranking.
    for (from_dense, list) in [(false, lexical), (true, dense)] {
        let mut seen: std::collections::HashSet<&MergeUnit> = std::collections::HashSet::new();
        let unique = list.iter().filter(|unit| seen.insert(*unit));
        for (rank, unit) in unique.enumerate() {
            if !exact.contains(unit) {
                add(unit, rank, from_dense);
            }
        }
    }
    fused.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.unit.path.cmp(&b.unit.path))
            .then_with(|| a.unit.start.cmp(&b.unit.start))
            .then_with(|| a.unit.end.cmp(&b.unit.end))
    });
    out.extend(fused);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(path: &str, start: u64, end: u64) -> MergeUnit {
        MergeUnit {
            path: path.into(),
            start,
            end,
        }
    }

    #[test]
    fn exact_definitions_keep_their_order_ahead_of_the_fusion() {
        let fused = fuse(
            &[unit("z.rs", 90, 99), unit("a.rs", 0, 10)],
            &[unit("m.rs", 0, 5)],
            &[],
        );
        assert_eq!(fused[0].unit.path, "z.rs");
        assert_eq!(fused[0].tier, 1);
        assert_eq!(fused[1].unit.path, "a.rs");
        assert_eq!(fused[1].tier, 1);
        assert_eq!(fused[2].unit.path, "m.rs");
        assert_eq!(fused[2].tier, 2);
    }

    #[test]
    fn a_unit_in_both_lists_scores_the_sum_of_its_reciprocal_ranks() {
        let both = fuse(&[], &[unit("a.rs", 0, 5)], &[unit("a.rs", 0, 5)]);
        let only_lexical = fuse(&[], &[unit("b.rs", 0, 5)], &[]);
        let expected = 2.0 / (RRF_K + 1) as f32;
        assert_eq!(both.len(), 1);
        assert!((both[0].score - expected).abs() < 1e-9);
        assert_eq!(both[0].dense_rank, Some(0));
        assert!(both[0].lexical);
        assert!(only_lexical[0].score < both[0].score);
    }

    #[test]
    fn exactly_tied_scores_break_by_path_then_start_then_end() {
        // b and a each hold rank 0 in one list and rank 1 in the other:
        // identical sums, so only the tie rule orders them.
        let tied = fuse(
            &[],
            &[unit("b.rs", 0, 1), unit("a.rs", 5, 6)],
            &[unit("a.rs", 5, 6), unit("b.rs", 0, 1)],
        );
        assert_eq!(tied[0].unit.path, "a.rs");
        assert_eq!(tied[1].unit.path, "b.rs");
        // Same path: the lower start first.
        let same_path = fuse(
            &[],
            &[unit("a.rs", 9, 10), unit("a.rs", 2, 3)],
            &[unit("a.rs", 2, 3), unit("a.rs", 9, 10)],
        );
        assert_eq!(same_path[0].score, same_path[1].score);
        assert_eq!(same_path[0].unit.start, 2);
        assert_eq!(same_path[1].unit.start, 9);
        // Same path and start: the lower end first.
        let same_start = fuse(
            &[],
            &[unit("a.rs", 2, 40), unit("a.rs", 2, 9)],
            &[unit("a.rs", 2, 9), unit("a.rs", 2, 40)],
        );
        assert_eq!(same_start[0].score, same_start[1].score);
        assert_eq!(same_start[0].unit.end, 9);
        assert_eq!(same_start[1].unit.end, 40);
    }

    #[test]
    fn a_unit_in_tier1_and_a_list_is_not_repeated() {
        let fused = fuse(&[unit("a.rs", 0, 5)], &[unit("a.rs", 0, 5)], &[]);
        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].tier, 1);
        assert_eq!(fused[0].dense_rank, None);
    }

    #[test]
    fn overlapping_but_different_spans_stay_separate_candidates() {
        let fused = fuse(&[], &[unit("a.rs", 0, 10)], &[unit("a.rs", 0, 40)]);
        assert_eq!(fused.len(), 2);
    }

    #[test]
    fn duplicate_documents_of_one_unit_contribute_once_per_arm() {
        // Three search documents of one split unit, then another unit; the
        // dense arm names the split unit twice.
        let big = unit("a.rs", 0, 30_000);
        let other = unit("a.rs", 30_000, 30_100);
        let fused = fuse(
            &[],
            &[big.clone(), big.clone(), big.clone(), other.clone()],
            &[big.clone(), big.clone()],
        );
        assert_eq!(fused.len(), 2);
        let one_each = 2.0 / (RRF_K + 1) as f32;
        assert!(
            (fused[0].score - one_each).abs() < 1e-9,
            "{}",
            fused[0].score
        );
        // The other unit is the lexical arm's SECOND unique unit.
        assert_eq!(fused[1].unit, other);
        assert!((fused[1].score - 1.0 / (RRF_K + 2) as f32).abs() < 1e-9);
        // Tier 1 lists a unit once however many documents carry it.
        let exact = fuse(&[big.clone(), big.clone(), other], &[big], &[]);
        assert_eq!(exact.len(), 2);
        assert!(exact.iter().all(|candidate| candidate.tier == 1));
    }
}
