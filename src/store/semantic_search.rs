//! Dense-aware search (spec 009): `Engine::search_candidates_semantic`, the
//! selection it shares with context assembly, and the path filter and prose
//! passages only it uses, with their test. Placement order, callable grouping,
//! per-file cap and cut live in `super::placement`; shared candidate types stay
//! in the parent module.
use super::placement::{Cut, Order, Placed, Refined, group_callables, place};
use super::*;

/// Whether `path` lies under a normalized [`path_filter`]: the file itself
/// or a component-boundary subtree (`src/a` admits `src/a/x.rs`, never
/// `src/ab.rs`) — the predicate the lexical tiers apply through `dir` terms.
#[cfg(feature = "semantic")]
fn within_filter(path: &str, filter: &str) -> bool {
    path.strip_prefix(filter)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
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
    pub(super) fn search_candidates_semantic_selected(
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
                        trace.record(serde_json::json!({"stage":"refinement", "outcome":"scored", "method":"published_body_exact", "localization":"owner_prioritized_file_refinement_v2", "work":refinement.stats}));
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
        let mut lexical: Vec<&Candidate> = Vec::with_capacity(first.len() + second.len());
        let mut lexical_selection = LexicalSelection::default();
        for (rank, candidate) in first.iter().chain(&second).enumerate() {
            load(&mut current, &candidate.path)?;
            if current[&candidate.path]
                .as_ref()
                .is_some_and(|meta| meta.hash == candidate.hash)
            {
                if let Err(outcome) = lexical_selection.insert(candidate, prose) {
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
            placed.push(Placed {
                unit: (location.path.as_str(), location.start, location.end),
                kind: Some(location.kind),
            });
        }
        let freshness = self.freshness_in(&tx)?;
        let word = dense.coverage_at(freshness.source_revision);
        // Freshness was checked above; no candidate/output slots are added.
        let order = Order::resolve(
            prose,
            word,
            dense,
            Refined {
                hits: refined_count,
                eligible: eligible_refined,
                files: refinement_paths.len(),
            },
        );
        let mut placed = place(order, placed, &lexical, prose, control);
        // The candidate carrying each lexical unit: its first (best-ranked)
        // current document, as in the baseline.
        let mut by_unit = std::collections::HashMap::new();
        let mut by_document = std::collections::HashMap::new();
        for &candidate in &lexical {
            by_unit.entry(candidate.unit()).or_insert(candidate);
            by_document
                .entry(candidate.selection_unit(prose))
                .or_insert(candidate);
        }

        if prose {
            placed = group_callables(&placed, &by_document, control)?;
        }

        // The baseline's merge per unit, per-file cap and cut, over the
        // placed order.
        let mut seen = std::collections::BTreeSet::new();
        let mut cut = Cut::new(limit, prose);
        let trace_placement = |rank: usize, item: Placed<'_>, outcome: &str| {
            if let Some(trace) = control.trace() {
                trace.record(serde_json::json!({
                    "stage": "placement", "origin": if item.kind.is_some() { "dense" } else { "lexical" },
                    "rank": rank, "outcome": outcome,
                    "unit": {"workspace_id": workspace_id, "path": item.unit.0,
                        "sha256": current.get(item.unit.0).and_then(|m| m.as_ref()).map(|m| &m.hash),
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
                verified.insert(unit_path.to_owned(), source);
            }
            let body = &verified[unit_path].body;
            let candidate = if from_dense {
                // An exact unit match may contribute its label and support.
                // A scored Body still renders its own unchanged byte range.
                by_unit.get(&unit)
            } else {
                by_document.get(&unit)
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
                    let (from, to) = (unit_start as usize, unit_end as usize);
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
                            path: unit_path.to_owned(),
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
        })
    }
}
