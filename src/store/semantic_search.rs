//! Dense-aware search (spec 009): current dense locations in similarity order,
//! followed by lexical units, with one final source validation before placement.
//! The lexical-only T010 option does not change this path.
use super::placement::{Cut, Placed, UnitRef};
use super::*;

/// Whether `path` lies under a normalized [`path_filter`]: the file itself
/// or a component-boundary subtree (`src/a` admits `src/a/x.rs`, never
/// `src/ab.rs`) — the predicate the lexical tiers apply through `dir` terms.
#[cfg(feature = "semantic")]
fn within_filter(path: &str, filter: &str) -> bool {
    path.strip_prefix(filter)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

impl Engine {
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
        let filter = check_search(query, path, limit)?;
        let workspace_id = self.require_workspace_id()?;
        let TwoTier {
            prose: _,
            first,
            second,
            candidates_full,
            windows,
        } = self.collect_two_tier(
            query,
            path,
            control,
            SearchSelection {
                anchors,
                prose: false,
            },
        )?;

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

        control.check()?;

        // Final read: every candidate is validated here, before placement.
        let tx = self.db.begin_read()?;
        let mut sources = FinalSources::open(&tx)?;
        let mut counters = CandidateCounters {
            candidates_full: candidates_full || ordinary.truncated,
            ..CandidateCounters::default()
        };
        let wanted = wanted_terms(query, false);
        // The anchor windows, from the tier-1 documents as collected (their
        // metadata read first, in that order); their stale definitions are
        // counted with tier 1 below.
        for candidate in &first {
            sources.meta(&candidate.path)?;
        }
        let anchors = anchor_windows(&windows, &first, &mut sources, &workspace_id, &wanted)?;
        let lexical = current_lexical(
            &first,
            &second,
            &mut sources,
            control,
            &workspace_id,
            &mut counters,
        )?;
        let DenseOrder { mut placed, stale } = current_dense(
            dense,
            &mut ordinary.items,
            filter.as_deref(),
            &mut sources,
            control,
            &workspace_id,
        )?;
        counters.stale += stale;
        let freshness = self.freshness_in(&tx)?;
        let word = dense.coverage_at(freshness.source_revision);
        placed.extend(lexical.iter().map(|candidate| Placed {
            unit: candidate.unit(),
            kind: None,
        }));
        // The candidate carrying each lexical unit: its first (best-ranked)
        // current document, as in the baseline.
        let mut by_unit = std::collections::HashMap::new();
        for &candidate in &lexical {
            by_unit.entry(candidate.unit()).or_insert(candidate);
        }

        // The baseline's merge per unit, per-file cap and cut, over the
        // placed order.
        let mut seen = std::collections::BTreeSet::new();
        let mut cut = Cut::new(limit, false);
        let trace_placement = |rank: usize, item: Placed<'_>, outcome: &str| {
            if let Some(trace) = control.trace() {
                trace.record(serde_json::json!({
                    "stage": "placement", "origin": if item.kind.is_some() { "dense" } else { "lexical" },
                    "rank": rank, "outcome": outcome,
                    "unit": {"workspace_id": workspace_id, "path": item.unit.0,
                        "sha256": sources.loaded(item.unit.0).map(|meta| &meta.hash),
                        "start": item.unit.1, "end": item.unit.2},
                }));
            }
        };
        for (rank, &item) in placed.iter().enumerate() {
            if !seen.insert(item.unit) {
                trace_placement(rank, item, "duplicate");
                continue;
            }
            if let Some(outcome) = cut.offer(rank, item.unit.0, item, &mut counters) {
                trace_placement(rank, item, outcome);
            }
        }
        let kept = cut.finish(&mut counters, trace_placement);
        let mut items = Vec::with_capacity(kept.len());
        let mut delivered = std::collections::BTreeSet::new();
        for (rank, Placed { unit, kind }) in kept.into_iter().enumerate() {
            control.check()?;
            let from_dense = kind.is_some();
            let exact_body = kind == Some(crate::neural::cache::CardKind::Body);
            let (unit_path, unit_start, unit_end) = unit;
            let Some((meta, body)) = sources.body(unit_path)? else {
                continue;
            };
            let candidate = by_unit.get(&unit).copied();
            let mut item = match candidate {
                Some(candidate) => {
                    ranked_item(candidate, rank, &workspace_id, &meta.hash, body, &wanted)?
                }
                None => dense_only_item(unit, rank, &workspace_id, &meta.hash, body)?,
            };
            if from_dense {
                item.semantic = Some(SemanticEvidence {
                    dense_only: candidate.is_none(),
                });
                if let Some(trace) = control.trace() {
                    let focused = item
                        .handle
                        .as_ref()
                        .is_some_and(|h| (h.start, h.end) != (unit_start, unit_end));
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
            // The request's own trace, for final packing decisions.
            trace: control.trace().cloned(),
        })
    }
}

/// The lexical documents of the source version this read sees, tier 1 then
/// tier 2 as collected, each unit at most once; a stale document is counted and dropped.
#[cfg(feature = "semantic")]
fn current_lexical<'c>(
    first: &'c [Candidate],
    second: &'c [Candidate],
    sources: &mut FinalSources,
    control: &crate::Control,
    workspace_id: &str,
    counters: &mut CandidateCounters,
) -> FResult<Vec<&'c Candidate>> {
    let mut lexical = Vec::with_capacity(first.len() + second.len());
    let mut lexical_selection = LexicalSelection::default();
    for (rank, candidate) in first.iter().chain(second).enumerate() {
        if sources.is_current(&candidate.path, &candidate.hash)? {
            if let Err(outcome) = lexical_selection.insert(candidate, false) {
                candidate.trace(control, workspace_id, rank, outcome);
                continue;
            }
            candidate.trace(control, workspace_id, rank, "eligible");
            lexical.push(candidate);
        } else {
            candidate.trace(control, workspace_id, rank, "stale");
            counters.stale += 1;
        }
    }
    Ok(lexical)
}

/// The dense lane of one final read.
#[cfg(feature = "semantic")]
struct DenseOrder<'d> {
    placed: Vec<Placed<'d>>,
    /// Locations dropped as stale.
    stale: u64,
}

/// The dense locations in similarity order: each
/// must lie within the path filter, then be current in this read (its
/// recorded source hash the current one, its range inside the source).
#[cfg(feature = "semantic")]
fn current_dense<'d>(
    dense: &'d crate::neural::query::DenseWindow,
    ordinary: &mut [(usize, &'d crate::neural::index::UnitLocation)],
    filter: Option<&str>,
    sources: &mut FinalSources,
    control: &crate::Control,
    workspace_id: &str,
) -> FResult<DenseOrder<'d>> {
    let mut placed = Vec::new();
    let mut stale = 0;
    // Restore similarity order after fair bounded provenance admission.
    ordinary.sort_by_key(|(rank, _)| *rank);
    for &(rank, location) in ordinary.iter() {
        let hit = &dense.hits[rank];
        control.check()?;
        let trace_location = |outcome: &str| {
            if let Some(trace) = control.trace() {
                trace.record(serde_json::json!({
                    "stage": "selection", "origin": "dense", "rank": rank,
                    "distance": hit.distance, "input_key": dense.input_key(hit), "outcome": outcome,
                    "representation": location.kind, "refined":false,
                    "unit": {"workspace_id": workspace_id, "path": location.path,
                        "sha256": location.source_sha256, "start": location.start,
                        "end": location.end},
                }));
            }
        };
        if filter.is_some_and(|filter| !within_filter(&location.path, filter)) {
            trace_location("path_filter");
            continue;
        }
        let current_location = sources.meta(&location.path)?.is_some_and(|meta| {
            *meta.hash == *location.source_sha256
                && location.end <= meta.bytes as u64
                && (location.kind != crate::neural::cache::CardKind::File
                    || (location.start == 0 && location.end == meta.bytes as u64))
        });
        if !current_location {
            trace_location("stale");
            stale += 1;
            continue;
        }
        trace_location("eligible");
        placed.push(Placed {
            unit: (&*location.path, location.start, location.end),
            kind: Some(location.kind),
        });
    }
    Ok(DenseOrder { placed, stale })
}

/// A dense-only unit with its scored source span and no delivery-unit label.
#[cfg(feature = "semantic")]
fn dense_only_item(
    (path, unit_start, unit_end): UnitRef<'_>,
    rank: usize,
    workspace_id: &str,
    sha256: &str,
    body: &str,
) -> FResult<RankedItem> {
    let (from, to) = (unit_start as usize, unit_end as usize);
    if !(from < to && to <= body.len() && body.is_char_boundary(from) && body.is_char_boundary(to))
    {
        return Err(FoundryError::CorruptStore(format!(
            "candidate unit outside {path}"
        )));
    }
    let text = &body[from..to];
    let newlines = |bytes: &[u8]| bytes.iter().filter(|&&b| b == b'\n').count();
    let start_line = newlines(&body.as_bytes()[..from]) as u64 + 1;
    let end_line = start_line + newlines(&text.as_bytes()[..text.len() - 1]) as u64;
    Ok(RankedItem {
        tier: 2,
        rank,
        score: 0.0,
        handle: Some(SourceHandle {
            workspace_id: workspace_id.to_owned(),
            path: path.to_owned(),
            sha256: sha256.to_owned(),
            start: from as u64,
            end: to as u64,
        }),
        start_line,
        end_line,
        line: start_line,
        label: String::new(),
        lang: crate::syntax::Lang::from_path(path).map(|lang| lang.tag().to_owned()),
        semantic: None,
        resolver: None,
        forms: vec![RenderedForm::Verbatim(text.to_owned())],
    })
}
