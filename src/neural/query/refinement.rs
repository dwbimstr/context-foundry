//! Hierarchy localization (experimental prose selection only): owner scopes and proofs, the bounded working
//! set and candidate window, and `DenseWindow::refine`, with their tests. The
//! dense index, the provider runtime and the query entry points stay in the
//! parent module.
use super::*;

/// A discovery scope, not a new source candidate or an inherited body score.
/// Lexical owners use the indexed unit kind/hash/geometry, never its display
/// label. Parent scopes must join their exact current partition tuple below.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OwnerScope<'a> {
    pub source_hash: &'a str,
    pub start: u64,
    pub end: u64,
    pub proof: OwnerProof<'a>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum OwnerProof<'a> {
    Callable {
        kind: &'a str,
        head: u64,
        document: (u64, u64),
    },
    Parent {
        input_key: &'a str,
    },
}

#[derive(Default)]
struct OwnerRanges {
    ranges: Vec<(u64, u64)>,
    valid: usize,
    stale: usize,
    invalid: usize,
    narrowed: usize,
    conflict: bool,
}

impl OwnerRanges {
    /// Valid nested ranges reduce to leaves, which are disjoint and sorted.
    /// A Body must fit whole to receive owner priority. Other published bodies
    /// remain eligible in the file lane; an owner is a hint, not an answer proof.
    fn contains(&self, start: u64, end: u64) -> bool {
        self.ranges
            .partition_point(|&(from, _)| from <= start)
            .checked_sub(1)
            .is_some_and(|index| end <= self.ranges[index].1)
    }
}

fn refinement_scopes(
    scopes: &[OwnerScope<'_>],
    source_hash: &str,
    source_len: usize,
    parents: &std::collections::BTreeSet<(u64, u64, &str)>,
    control: &Control,
) -> Result<OwnerRanges, Fallback> {
    control.check()?;
    let mut result = OwnerRanges::default();
    for scope in scopes {
        control.check()?;
        if scope.source_hash != source_hash {
            result.stale += 1;
            continue;
        }
        let geometry = scope.start < scope.end && scope.end <= source_len as u64;
        let proven = match scope.proof {
            OwnerProof::Callable {
                kind,
                head,
                document: (from, to),
            } => {
                matches!(kind, "fn" | "method")
                    && scope.start <= head
                    && head < scope.end
                    && scope.start <= from
                    && from < to
                    && to <= scope.end
            }
            OwnerProof::Parent { input_key } => {
                parents.contains(&(scope.start, scope.end, input_key))
            }
        };
        if !geometry || !proven {
            result.invalid += 1;
            continue;
        }
        result.valid += 1;
        result.ranges.push((scope.start, scope.end));
    }
    result
        .ranges
        .sort_unstable_by_key(|&(start, end)| (start, std::cmp::Reverse(end)));
    result.ranges.dedup();
    // Same-source syntax scopes form a forest. Non-nested overlap is not a
    // proven owner relationship: retain the old file fallback for that path.
    let mut ends = Vec::new();
    for &(start, end) in &result.ranges {
        control.check()?;
        while ends.last().is_some_and(|&through| through <= start) {
            ends.pop();
        }
        if ends.last().is_some_and(|&through| end > through) {
            result.conflict = true;
            break;
        }
        ends.push(end);
    }
    if result.conflict {
        result.ranges.clear();
        return Ok(result);
    }
    // Declared experimental tradeoff: a current narrower owner localizes a
    // broader discovered scope. Disjoint owners remain a union; no min/max
    // envelope is allowed to admit the source between them.
    let mut narrowed = Vec::with_capacity(result.ranges.len());
    for (index, &range) in result.ranges.iter().enumerate() {
        control.check()?;
        if result
            .ranges
            .get(index + 1)
            .is_some_and(|&next| next.1 <= range.1)
        {
            result.narrowed += 1;
        } else {
            narrowed.push(range);
        }
    }
    result.ranges = narrowed;
    Ok(result)
}

impl DenseWindow {
    /// Localize coarse files by scoring a bounded working set of their published
    /// body inputs. Selected partition point reads identify keys; only this
    /// generation's matching locations and resident vectors can supply evidence.
    /// No graph traversal, source inference, or corpus-wide metadata walk occurs.
    pub(crate) fn refine(
        &self,
        engine: &Engine,
        paths: &std::collections::BTreeSet<String>,
        scopes: &std::collections::BTreeMap<String, Vec<OwnerScope<'_>>>,
        control: &Control,
    ) -> Result<Refinement<'_>, Fallback> {
        use std::collections::{BTreeSet, VecDeque};
        const MAX_PATHS: usize = 80;
        const MAX_ROW_BYTES: usize = 512 * 1024;
        const MAX_MAPPING_BYTES: usize = 8 * 1024 * 1024;
        const MAX_UNITS: usize = 32_768;
        const MAX_VECTORS: usize = 4096;

        control.check()?;
        let page = super::cache::MappingPage::new(engine, control)?;
        let mut stats = RefinementStats {
            truncated: paths.len() > MAX_PATHS,
            ..Default::default()
        };
        let mut queues = Vec::new();
        // References retain only provenance that passed this request's point
        // join. A shared vector key never admits other generation locations.
        let mut joined: std::collections::BTreeMap<usize, Vec<&UnitLocation>> =
            std::collections::BTreeMap::new();
        for path in paths.iter().take(MAX_PATHS) {
            control.check()?;
            stats.paths += 1;
            let bytes = page.partition_bytes(path)?;
            if bytes > MAX_ROW_BYTES || bytes > MAX_MAPPING_BYTES - stats.mapping_bytes {
                stats.truncated = true;
                stats.mapping_rows_skipped += 1;
                continue;
            }
            stats.mapping_bytes += bytes;
            let Some((record, source_len)) = page.partition_with_source_len(
                path,
                &self.generation.identity.digest,
                &self.generation.identity.recipe,
                control,
            )?
            else {
                continue;
            };
            let mut file_bodies = Vec::new();
            let mut parents = BTreeSet::new();
            for unit in &record.units {
                control.check()?;
                if stats.units_considered == MAX_UNITS {
                    stats.truncated = true;
                    break;
                }
                stats.units_considered += 1;
                if unit.kind == super::cache::CardKind::Parent {
                    parents.insert((unit.start as u64, unit.end as u64, unit.input_key.as_str()));
                }
                if unit.kind != super::cache::CardKind::Body {
                    continue;
                }
                // Location vectors are produced in (path,start,end) order. A
                // failed point join is never replaced with a cache-only vector.
                // The full tuple, role and hash are checked after the lookup.
                let published = self
                    .generation
                    .labels
                    .get(&unit.input_key)
                    .and_then(|locations| {
                        locations
                            .binary_search_by(|location| {
                                (location.path.as_str(), location.start, location.end).cmp(&(
                                    path.as_str(),
                                    unit.start as u64,
                                    unit.end as u64,
                                ))
                            })
                            .ok()
                            .and_then(|position| locations.get(position))
                    })
                    .filter(|location| {
                        location.kind == super::cache::CardKind::Body
                            && location.source_sha256 == record.source_hash
                    });
                let Some(location) = published else {
                    stats.unpublished_units += 1;
                    continue;
                };
                let Some(label) = self.generation.label_for_key(&unit.input_key) else {
                    return Err(Fallback::new(
                        "index_unavailable",
                        "published body has no resident vector",
                    ));
                };
                file_bodies.push((label, location));
                stats.joined_locations += 1;
            }
            let requested = scopes.get(path).map_or(&[][..], Vec::as_slice);
            let owners = refinement_scopes(
                requested,
                &record.source_hash,
                source_len,
                &parents,
                control,
            )?;
            stats.owner_scopes += requested.len();
            stats.owner_scopes_valid += owners.valid;
            stats.owner_scopes_stale += owners.stale;
            stats.owner_scopes_invalid += owners.invalid;
            stats.owner_scopes_narrowed += owners.narrowed;
            stats.owner_scope_conflicts += usize::from(owners.conflict);
            let mut has_scoped_body = false;
            for &(_, location) in &file_bodies {
                control.check()?;
                has_scoped_body |= owners.contains(location.start, location.end);
            }
            let fallback = if owners.conflict {
                Some("incompatible_scopes")
            } else if owners.ranges.is_empty() {
                Some("no_current_owner")
            } else if !has_scoped_body {
                Some("no_published_body_in_owner")
            } else {
                None
            };
            if fallback.is_some() {
                stats.owner_fallback_paths += 1;
            } else {
                stats.owner_scoped_paths += 1;
            }
            if let Some(trace) = control.trace() {
                trace.record(serde_json::json!({
                    "stage":"refinement_scope", "policy":"owner_prioritized_file_refinement_v2",
                    "path":path, "sha256":record.source_hash,
                    "requested":requested.len(), "valid":owners.valid,
                    "stale":owners.stale, "invalid":owners.invalid,
                    "narrowed":owners.narrowed, "conflict":owners.conflict,
                    "ranges":owners.ranges, "fallback":fallback,
                }));
            }
            let mut owner_queue = VecDeque::new();
            let mut file_queue = VecDeque::new();
            for (label, location) in file_bodies {
                control.check()?;
                joined.entry(label).or_default().push(location);
                if fallback.is_none() && owners.contains(location.start, location.end) {
                    stats.owner_prioritized_locations += 1;
                    owner_queue.push_back(label);
                } else {
                    stats.file_lane_locations += 1;
                    file_queue.push_back(label);
                }
            }
            // A lexical/Parent owner must not veto competing implementations
            // before their own vectors are scored. Alternate owner and remaining
            // file bodies within this file's fair turn, deduplicating shared
            // labels. The global scoring/candidate/provenance limits stay fixed.
            let (queue, _) =
                refinement_working_set(vec![owner_queue, file_queue], MAX_UNITS, control)?;
            if !queue.is_empty() {
                queues.push(VecDeque::from(queue));
            }
            if stats.units_considered == MAX_UNITS {
                stats.truncated |= stats.paths < paths.len();
                break;
            }
        }
        drop(page);

        let (labels, truncated) = refinement_working_set(queues, MAX_VECTORS, control)?;
        stats.truncated |= truncated;
        let mut hits = Vec::with_capacity(labels.len());
        let mut scratch = vec![0.0f32; self.generation.dimensions];
        for label in labels {
            control.check()?;
            hits.push(
                self.generation
                    .score_label(&self.vector, label, &mut scratch)?,
            );
        }
        stats.vectors_scored = hits.len();
        hits.sort_by(|a, b| {
            a.distance.total_cmp(&b.distance).then_with(|| {
                self.generation
                    .input_key(a.label)
                    .cmp(self.generation.input_key(b.label))
            })
        });
        let hits = refinement_candidate_window(hits, joined, &mut stats, control)?;
        control.check()?;
        Ok(Refinement { hits, stats })
    }

    /// These opt-in recipes can map a small extractive input to a whole file
    /// or parent. Context may focus their source locally; this does not mean
    /// the resulting passage received its own embedding or similarity score.
    pub(crate) fn has_overviews(&self) -> bool {
        self.file_overviews_only() || self.generation.identity.recipe.starts_with("hierarchy-v1+")
    }

    /// Independent passage representations keep hybrid admission even after
    /// preparation completes; coverage is availability, not relevance.
    pub(crate) fn has_body_representations(&self) -> bool {
        let recipe = &self.generation.identity.recipe;
        recipe.starts_with("body-v1+") || recipe.starts_with("hierarchy-v1+")
    }

    pub(crate) fn file_overviews_only(&self) -> bool {
        self.generation.identity.recipe.starts_with("overview-v1+")
    }

    /// The document-input key of one hit.
    pub fn input_key(&self, hit: &DenseHit) -> &str {
        self.generation.input_key(hit.label)
    }

    /// The unit locations one hit stood for at publication. Each is a
    /// candidate only after the request's final read revalidates it.
    pub fn units(&self, hit: &DenseHit) -> &[UnitLocation] {
        &self.generation.labels[self.input_key(hit)]
    }

    /// Admit at most `limit` ordinary provenance locations fairly across the
    /// bounded hit window. Filter lookup is logarithmic in each sorted map;
    /// duplicate provenance cannot force a corpus-wide expansion before caps.
    pub(crate) fn bounded_units(
        &self,
        filter: Option<&str>,
        limit: usize,
        control: &Control,
    ) -> Result<DenseLocations<'_>, Fallback> {
        let units = self
            .hits
            .iter()
            .take(DENSE_WINDOW)
            .map(|hit| self.units(hit))
            .collect();
        let mut locations = bounded_unit_locations(units, filter, limit, control)?;
        locations.truncated |= self.hits.len() > DENSE_WINDOW;
        Ok(locations)
    }

    /// The source revision a COMPLETE generation was published at; `None`
    /// for a generation that covers only part of the eligible units. The
    /// coverage word is `ready` only when this equals the request's revision.
    pub fn complete_at(&self) -> Option<u64> {
        self.generation
            .complete
            .then_some(self.generation.source_revision)
    }

    /// The coverage word of a response whose final read saw
    /// `source_revision`: `ready` only for a complete generation published
    /// at exactly that revision, otherwise `partial`. Known coverage, never
    /// inferred from the candidate set.
    pub fn coverage_at(&self, source_revision: u64) -> &'static str {
        if self.complete_at() == Some(source_revision) {
            "ready"
        } else {
            "partial"
        }
    }
}

/// Deterministic fair admission, with no more than `limit` distinct vector keys.
/// Duplicate provenance consumes neither a scoring slot nor another file's turn.
fn refinement_working_set(
    mut queues: Vec<std::collections::VecDeque<usize>>,
    limit: usize,
    control: &Control,
) -> Result<(Vec<usize>, bool), Fallback> {
    let mut seen = std::collections::BTreeSet::new();
    let mut labels = Vec::new();
    loop {
        let mut progressed = false;
        for queue in &mut queues {
            control.check()?;
            while let Some(label) = queue.pop_front() {
                control.check()?;
                if !seen.insert(label) {
                    continue;
                }
                if labels.len() == limit {
                    return Ok((labels, true));
                }
                labels.push(label);
                progressed = true;
                break;
            }
        }
        if !progressed {
            return Ok((labels, false));
        }
    }
}

/// Apply the existing delivery quota before spending the fixed label window.
/// The input is already sorted by cosine and input key. Backfill only from that
/// same bounded scored set: a file's fifth body cannot evict another file's first
/// body and then be discarded during placement. Shared labels keep only their
/// point-joined locations. Unused slots backfill from deferred locations in score
/// order. Final-read freshness and placement still apply.
fn refinement_candidate_window<'a>(
    hits: Vec<DenseHit>,
    mut joined: std::collections::BTreeMap<usize, Vec<&'a UnitLocation>>,
    stats: &mut RefinementStats,
    control: &Control,
) -> Result<Vec<RefinedHit<'a>>, Fallback> {
    use std::collections::{BTreeMap, BTreeSet};
    control.check()?;
    let mut kept = Vec::with_capacity(DENSE_WINDOW.min(hits.len()));
    let mut overflow = Vec::new();
    let mut per_file = BTreeMap::<&str, usize>::new();
    let mut seen = BTreeSet::new();
    for hit in hits {
        control.check()?;
        if kept.len() == DENSE_WINDOW {
            break;
        }
        stats.labels_examined += 1;
        let mut locations = Vec::new();
        let mut deferred = Vec::new();
        for location in joined.remove(&hit.label).unwrap_or_default() {
            control.check()?;
            if !seen.insert((location.path.as_str(), location.start, location.end)) {
                stats.duplicate_locations += 1;
                continue;
            }
            let count = per_file.entry(location.path.as_str()).or_default();
            if *count == crate::store::PER_FILE_CAP {
                deferred.push(location);
                continue;
            }
            *count += 1;
            locations.push(location);
        }
        if !deferred.is_empty() {
            overflow.push(RefinedHit {
                hit: hit.clone(),
                locations: deferred,
            });
        }
        if !locations.is_empty() {
            stats.retained_locations += locations.len();
            kept.push(RefinedHit { hit, locations });
        }
    }
    // Preserve the previous worst-case provenance allowance (80 discovery
    // paths * four locations), while permitting a sparse set to use it.
    const MAX_LOCATIONS: usize = 80 * crate::store::PER_FILE_CAP;
    for mut deferred in overflow {
        control.check()?;
        let remaining = MAX_LOCATIONS.saturating_sub(stats.retained_locations);
        if deferred.locations.len() > remaining {
            stats.quota_skipped_locations += deferred.locations.len() - remaining;
            stats.truncated = true;
            deferred.locations.truncate(remaining);
        }
        if deferred.locations.is_empty() {
            continue;
        }
        // A shared vector may already have locations from another file.
        if let Some(existing) = kept
            .iter_mut()
            .find(|entry| entry.hit.label == deferred.hit.label)
        {
            stats.retained_locations += deferred.locations.len();
            existing.locations.extend(deferred.locations);
        } else if kept.len() < DENSE_WINDOW {
            stats.retained_locations += deferred.locations.len();
            kept.push(deferred);
        } else {
            stats.quota_skipped_locations += deferred.locations.len();
        }
    }
    Ok(kept)
}

pub(crate) struct RefinedHit<'a> {
    pub hit: DenseHit,
    /// Only the exact locations admitted by the bounded current-partition join.
    pub locations: Vec<&'a UnitLocation>,
}

pub(crate) struct Refinement<'a> {
    pub hits: Vec<RefinedHit<'a>>,
    pub stats: RefinementStats,
}

pub(crate) struct DenseLocations<'a> {
    /// Fair admission order; the caller may restore hit order for placement.
    pub items: Vec<(usize, &'a UnitLocation)>,
    pub available: usize,
    pub truncated: bool,
}

fn bounded_unit_locations<'a>(
    units: Vec<&'a [UnitLocation]>,
    filter: Option<&str>,
    limit: usize,
    control: &Control,
) -> Result<DenseLocations<'a>, Fallback> {
    control.check()?;
    let subtree = filter.map(|path| (format!("{path}/"), format!("{path}0")));
    let mut available = 0usize;
    let mut cursors = Vec::with_capacity(units.len());
    for units in units {
        control.check()?;
        let (exact, children) = if let Some(path) = filter {
            // Exact path and component-boundary subtree are disjoint ranges:
            // e.g. a-b and a.rs can sort between a and a/child.
            let exact_from = units.partition_point(|unit| unit.path.as_str() < path);
            let exact_to = units.partition_point(|unit| unit.path.as_str() <= path);
            let (lower, upper) = subtree.as_ref().expect("filter subtree bounds");
            let child_from = units.partition_point(|unit| unit.path.as_str() < lower.as_str());
            let child_to = units.partition_point(|unit| unit.path.as_str() < upper.as_str());
            (&units[exact_from..exact_to], &units[child_from..child_to])
        } else {
            (units, &units[..0])
        };
        available = available
            .saturating_add(exact.len())
            .saturating_add(children.len());
        cursors.push(exact.iter().chain(children));
    }
    let mut items = Vec::with_capacity(limit.min(available));
    while items.len() < limit {
        let mut progressed = false;
        for (rank, cursor) in cursors.iter_mut().enumerate() {
            control.check()?;
            if let Some(location) = cursor.next() {
                items.push((rank, location));
                progressed = true;
                if items.len() == limit {
                    break;
                }
            }
        }
        if !progressed {
            break;
        }
    }
    Ok(DenseLocations {
        truncated: items.len() < available,
        items,
        available,
    })
}

#[derive(Default, serde::Serialize)]
pub(crate) struct RefinementStats {
    paths: usize,
    mapping_bytes: usize,
    mapping_rows_skipped: usize,
    units_considered: usize,
    unpublished_units: usize,
    joined_locations: usize,
    owner_scopes: usize,
    owner_scopes_valid: usize,
    owner_scopes_stale: usize,
    owner_scopes_invalid: usize,
    owner_scopes_narrowed: usize,
    owner_scope_conflicts: usize,
    owner_scoped_paths: usize,
    owner_fallback_paths: usize,
    owner_prioritized_locations: usize,
    file_lane_locations: usize,
    retained_locations: usize,
    vectors_scored: usize,
    labels_examined: usize,
    /// Candidate-window quota skips, before final-read placement counters.
    quota_skipped_locations: usize,
    duplicate_locations: usize,
    pub(crate) truncated: bool,
}

#[cfg(all(test, feature = "test-faults"))]
mod tests {
    use super::*;

    fn callable_owner(start: u64, end: u64) -> OwnerScope<'static> {
        OwnerScope {
            source_hash: "current",
            start,
            end,
            proof: OwnerProof::Callable {
                kind: "fn",
                head: start,
                document: (start, end),
            },
        }
    }

    #[test]
    fn owners_narrow_nested_discoveries_but_preserve_disjoint_scopes_and_exact_bodies() {
        let scopes = [
            OwnerScope {
                source_hash: "current",
                start: 0,
                end: 200,
                proof: OwnerProof::Parent {
                    input_key: "published_parent",
                },
            },
            callable_owner(10, 60),
            callable_owner(100, 150),
            callable_owner(10, 60),
        ];
        let parents = [(0, 200, "published_parent")].into();
        let result =
            refinement_scopes(&scopes, "current", 300, &parents, &Control::unbounded()).unwrap();
        assert_eq!(result.ranges, [(10, 60), (100, 150)]);
        assert_eq!(result.valid, 4);
        assert_eq!(result.narrowed, 1);
        assert!(result.contains(10, 60));
        assert!(result.contains(110, 130));
        assert!(!result.contains(60, 100), "the gap is not an owner");
        assert!(
            !result.contains(0, 30),
            "overlap cannot clip an embedded body"
        );
        assert!(
            !result.contains(50, 110),
            "two owners cannot combine into a new span"
        );
        let reversed: Vec<_> = scopes.into_iter().rev().collect();
        let replay =
            refinement_scopes(&reversed, "current", 300, &parents, &Control::unbounded()).unwrap();
        assert_eq!(replay.ranges, result.ranges);
    }

    #[test]
    fn stale_malformed_and_unpublished_owner_proofs_cannot_narrow_refinement() {
        let scopes = [
            OwnerScope {
                source_hash: "old",
                ..callable_owner(10, 60)
            },
            OwnerScope {
                start: 70,
                end: 60,
                ..callable_owner(10, 60)
            },
            OwnerScope {
                end: 301,
                ..callable_owner(10, 60)
            },
            OwnerScope {
                proof: OwnerProof::Callable {
                    kind: "section",
                    head: 10,
                    document: (10, 60),
                },
                ..callable_owner(10, 60)
            },
            OwnerScope {
                proof: OwnerProof::Callable {
                    kind: "method",
                    head: 61,
                    document: (10, 60),
                },
                ..callable_owner(10, 60)
            },
            OwnerScope {
                proof: OwnerProof::Callable {
                    kind: "fn",
                    head: 10,
                    document: (10, 70),
                },
                ..callable_owner(10, 60)
            },
            OwnerScope {
                proof: OwnerProof::Parent {
                    input_key: "wrong_key",
                },
                ..callable_owner(10, 60)
            },
            OwnerScope {
                proof: OwnerProof::Parent {
                    input_key: "published_parent",
                },
                ..callable_owner(10, 59)
            },
        ];
        let parents = [(10, 60, "published_parent")].into();
        let result =
            refinement_scopes(&scopes, "current", 300, &parents, &Control::unbounded()).unwrap();
        assert!(
            result.ranges.is_empty(),
            "the old bounded file fallback remains available"
        );
        assert_eq!(result.stale, 1);
        assert_eq!(result.invalid, 7);
        assert_eq!(result.valid, 0);
        let parent = OwnerScope {
            proof: OwnerProof::Parent {
                input_key: "published_parent",
            },
            ..callable_owner(10, 60)
        };
        assert_eq!(
            refinement_scopes(&[parent], "current", 300, &parents, &Control::unbounded())
                .unwrap()
                .ranges,
            [(10, 60)]
        );
        assert!(refinement_scopes(&[], "current", 300, &parents, &Control::cancelled()).is_err());
    }

    #[test]
    fn overlapping_nonnested_owner_claims_fall_back_instead_of_widening() {
        let scopes = [callable_owner(10, 80), callable_owner(60, 100)];
        let result = refinement_scopes(
            &scopes,
            "current",
            300,
            &Default::default(),
            &Control::unbounded(),
        )
        .unwrap();
        assert!(result.conflict);
        assert!(result.ranges.is_empty());
    }

    #[test]
    fn refinement_working_set_is_fair_deduplicated_bounded_and_cancellable() {
        use std::collections::VecDeque;
        let queues: Vec<VecDeque<_>> = (0..80)
            .map(|path| (0..100).map(|ordinal| path * 100 + ordinal).collect())
            .collect();
        let (labels, truncated) =
            refinement_working_set(queues, 4096, &Control::unbounded()).unwrap();
        assert_eq!(labels.len(), 4096);
        assert!(truncated);
        assert_eq!(
            labels
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            4096
        );
        let mut counts = [0usize; 80];
        for label in labels {
            counts[label / 100] += 1;
        }
        assert!(counts.iter().all(|&count| count == 51 || count == 52));
        let shared = vec![VecDeque::from([1, 2, 3]), VecDeque::from([1, 2, 4])];
        assert_eq!(
            refinement_working_set(shared.clone(), 4, &Control::unbounded()).unwrap(),
            (vec![1, 2, 3, 4], false)
        );
        assert_eq!(
            refinement_working_set(shared, 3, &Control::unbounded()).unwrap(),
            (vec![1, 2, 3], true)
        );
        assert!(
            refinement_working_set(vec![VecDeque::from([1])], 4096, &Control::cancelled()).is_err()
        );
        assert_eq!(
            refinement_working_set(vec![VecDeque::new(); 80], 4096, &Control::unbounded()).unwrap(),
            (vec![], false)
        );
    }

    #[test]
    fn refinement_window_backfills_after_file_quota_without_growing_the_window() {
        let locations: Vec<_> = (0..140)
            .map(|label| UnitLocation {
                kind: super::super::cache::CardKind::Body,
                path: if label < 64 {
                    "src/repeated.rs".into()
                } else {
                    format!("src/independent_{label:03}.rs")
                },
                start: (label * 8) as u64,
                end: (label * 8 + 8) as u64,
                source_sha256: "ab".repeat(32),
            })
            .collect();
        let hits = || {
            (0..locations.len())
                .map(|label| DenseHit {
                    label,
                    distance: label as f32 / 1000.0,
                })
                .collect()
        };
        let joined = || {
            locations
                .iter()
                .enumerate()
                .map(|(label, location)| (label, vec![location]))
                .collect()
        };
        let mut stats = RefinementStats::default();
        let selected =
            refinement_candidate_window(hits(), joined(), &mut stats, &Control::unbounded())
                .unwrap();
        // Truncating the raw scores to 64 first would yield only four useful
        // candidates, all from repeated.rs, after the ordinary file cap.
        assert_eq!(selected.len(), DENSE_WINDOW);
        let expected: Vec<_> = (0..crate::store::PER_FILE_CAP).chain(64..124).collect();
        assert_eq!(
            selected.iter().map(|hit| hit.hit.label).collect::<Vec<_>>(),
            expected
        );
        assert_eq!(stats.labels_examined, 124);
        assert_eq!(stats.quota_skipped_locations, 60);
        assert_eq!(stats.retained_locations, 64);
        assert_eq!(stats.duplicate_locations, 0);
        assert!(
            !stats.truncated,
            "this is the existing delivery quota, not an examination limit"
        );
        let replay = refinement_candidate_window(
            hits(),
            joined(),
            &mut RefinementStats::default(),
            &Control::unbounded(),
        )
        .unwrap();
        assert_eq!(
            replay.iter().map(|hit| hit.hit.label).collect::<Vec<_>>(),
            expected
        );
        assert!(
            refinement_candidate_window(
                hits(),
                joined(),
                &mut RefinementStats::default(),
                &Control::cancelled()
            )
            .is_err()
        );
    }

    #[test]
    fn shared_refinement_backfill_keeps_the_existing_global_provenance_bound() {
        let locations: Vec<_> = (0..400)
            .map(|n| UnitLocation {
                kind: super::super::cache::CardKind::Body,
                path: "src/one.rs".into(),
                start: n * 8,
                end: n * 8 + 8,
                source_sha256: "ab".repeat(32),
            })
            .collect();
        let mut stats = RefinementStats::default();
        let selected = refinement_candidate_window(
            vec![DenseHit {
                label: 0,
                distance: 0.0,
            }],
            [(0, locations.iter().collect())].into(),
            &mut stats,
            &Control::unbounded(),
        )
        .unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].locations.len(), 320);
        assert_eq!(stats.retained_locations, 320);
        assert_eq!(stats.quota_skipped_locations, 80);
        assert!(stats.truncated);
    }

    #[test]
    fn refinement_window_backfills_shared_locations_and_deduplicates_spans() {
        let locations: Vec<_> = ["a", "a", "a", "a", "a", "b", "b", "b", "b", "b", "c"]
            .into_iter()
            .enumerate()
            .map(|(index, path)| UnitLocation {
                kind: super::super::cache::CardKind::Body,
                path: format!("src/{path}.rs"),
                start: (index * 8) as u64,
                end: (index * 8 + 8) as u64,
                source_sha256: "ab".repeat(32),
            })
            .collect();
        let joined = [
            (0, locations[..4].iter().collect()),
            (1, locations[4..10].iter().collect()),
            (2, vec![&locations[10]]),
            (3, vec![&locations[0], &locations[10]]),
        ]
        .into();
        let hits = (0..4)
            .map(|label| DenseHit {
                label,
                distance: label as f32,
            })
            .collect();
        let mut stats = RefinementStats::default();
        let selected =
            refinement_candidate_window(hits, joined, &mut stats, &Control::unbounded()).unwrap();
        assert_eq!(
            selected.iter().map(|hit| hit.hit.label).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(selected[1].locations.len(), 6);
        assert_eq!(
            selected[1]
                .locations
                .iter()
                .filter(|l| l.path == "src/a.rs")
                .count(),
            1
        );
        assert_eq!(
            selected[1]
                .locations
                .iter()
                .filter(|l| l.path == "src/b.rs")
                .count(),
            5
        );
        assert_eq!(stats.retained_locations, 11);
        assert_eq!(stats.quota_skipped_locations, 0);
        assert_eq!(stats.duplicate_locations, 2);
        assert_eq!(stats.labels_examined, 4);
    }

    #[test]
    fn ordinary_provenance_is_bounded_fair_filtered_and_cancellable() {
        let location = |path: String| UnitLocation {
            kind: super::super::cache::CardKind::Body,
            path,
            start: 0,
            end: 8,
            source_sha256: "ab".repeat(32),
        };
        let shared: Vec<_> = (0..10_000)
            .map(|i| location(format!("src/mirror_{i:05}.rs")))
            .collect();
        let independent = vec![
            location("src/independent_a.rs".into()),
            location("src/independent_b.rs".into()),
        ];
        let bounded = bounded_unit_locations(
            vec![&shared, &independent],
            None,
            4096,
            &Control::unbounded(),
        )
        .unwrap();
        assert_eq!(bounded.available, 10_002);
        assert_eq!(bounded.items.len(), 4096);
        assert!(bounded.truncated);
        assert_eq!(
            bounded.items.iter().filter(|(rank, _)| *rank == 1).count(),
            2
        );
        assert_eq!(bounded.items[1].1.path, "src/independent_a.rs");
        let filtered = bounded_unit_locations(
            vec![&shared],
            Some("src/mirror_09999.rs"),
            4096,
            &Control::unbounded(),
        )
        .unwrap();
        assert_eq!(filtered.items.len(), 1);
        assert_eq!(filtered.items[0].1.path, "src/mirror_09999.rs");
        assert!(!filtered.truncated);
        let paths: Vec<_> = ["a", "a-b", "a.rs", "a/x", "a/x/z", "ab", "b"]
            .into_iter()
            .map(|path| location(path.into()))
            .collect();
        let filtered =
            bounded_unit_locations(vec![&paths], Some("a"), 4096, &Control::unbounded()).unwrap();
        assert_eq!(
            filtered
                .items
                .iter()
                .map(|(_, unit)| unit.path.as_str())
                .collect::<Vec<_>>(),
            ["a", "a/x", "a/x/z"]
        );
        assert!(bounded_unit_locations(vec![&shared], None, 4096, &Control::cancelled()).is_err());
    }
}
