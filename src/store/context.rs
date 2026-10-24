//! Context assembly (context-v2 § Context candidates and routing): the
//! `Engine::context_candidates*` entry points and the bounded behavior-intent
//! heuristic only they use. Search, doors, freshness and the shared candidate
//! types stay in the parent module.
use super::*;

/// Conservative, bounded intent recognition. Unknown wording, locator and
/// relationship requests keep the baseline. No model or language guess is used.
fn behavior_terms(
    query: &str,
    anchors: &[AnchorWindow],
) -> Option<std::collections::BTreeSet<String>> {
    let lower = query.trim().to_ascii_lowercase();
    if ![
        "how does ",
        "how do ",
        "how is ",
        "why does ",
        "explain how ",
        "describe how ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
        || anchors.len() != 1
        || !anchors[0].resolved()
    {
        return None;
    }
    let anchor = anchors[0].entries.first()?;
    let (kind, name) = anchor.label.split_once(' ')?;
    if !["struct", "class", "interface", "enum", "trait"].contains(&kind) {
        return None;
    }
    let words = analyzed_terms(crate::syntax::code_subtokens, query);
    if words.iter().any(|word| {
        ["find", "locate", "defined", "definition", "definitions"].contains(&word.as_str())
    }) {
        return None;
    }
    let names = analyzed_terms(crate::syntax::code_subtokens, name);
    let terms: std::collections::BTreeSet<_> = words
        .into_iter()
        .filter(|word| {
            !names.contains(word)
                && ![
                    "how", "does", "do", "is", "why", "explain", "describe", "the", "a", "an",
                    "it", "its", "and", "or", "to", "of", "in", "on", "with", "for", "when",
                    "before", "after", "first", "that", "this", "handle", "handles",
                ]
                .contains(&word.as_str())
        })
        .collect();
    (!terms.is_empty()).then_some(terms)
}

fn behavior_supports(
    anchor: &RankedItem,
    item: &RankedItem,
    terms: &std::collections::BTreeSet<String>,
) -> bool {
    let (Some(owner), Some(unit)) = (&anchor.handle, &item.handle) else {
        return false;
    };
    if owner.workspace_id != unit.workspace_id
        || owner.path != unit.path
        || owner.sha256 != unit.sha256
        || owner == unit
    {
        return false;
    }
    let (Some((_, name)), Some((kind, method))) =
        (anchor.label.split_once(' '), item.label.split_once(' '))
    else {
        return false;
    };
    if !["fn", "method", "function"].contains(&kind)
        || !(method.starts_with(&format!("{name}::")) || method.starts_with(&format!("{name}.")))
    {
        return false;
    }
    item.forms.iter().any(|form| match form {
        RenderedForm::Verbatim(body) => analyzed_terms(crate::syntax::code_subtokens, body)
            .iter()
            .any(|word| terms.contains(word)),
        _ => false,
    })
}

/// The signature text of a source item that is not a block: its range with
/// every elidable span folded; `None` without its file's outliner.
fn render_signature(
    item: &RankedItem,
    outliners: &std::collections::BTreeMap<&str, crate::syntax::Outliner<'_>>,
) -> Option<String> {
    let handle = item.handle.as_ref()?;
    if item.label == "block" {
        return None;
    }
    let outliner = outliners.get(handle.path.as_str())?;
    Some(outliner.render(handle.start as usize..handle.end as usize, 0, 0))
}

/// Appends `signature` as a signature form unless a verbatim form already
/// reads the same.
fn append_signature(forms: &mut Vec<RenderedForm>, signature: std::borrow::Cow<'_, str>) {
    let same = forms
        .iter()
        .any(|form| matches!(form, RenderedForm::Verbatim(text) if *text == *signature));
    if !same {
        forms.push(RenderedForm::Signature(signature.into_owned()));
    }
}

impl Engine {
    /// Context candidates (context-v2 § Context candidates and routing), in
    /// order: up to 32 delivery units from the two-tier ranking, then up to
    /// 3 file outlines for the first distinct files among the units; and,
    /// when the strategy resolves to graph, the doors of the query's first
    /// anchor (context-v2 § Doors). Every candidate is revalidated, its
    /// signature and outline forms are built and its doors are read in one
    /// final read transaction.
    pub fn context_candidates(
        &self,
        query: &str,
        strategy: Strategy,
        control: &crate::Control,
    ) -> FResult<CandidateBatch> {
        Ok(self
            .context_candidates_with(query, strategy, control, &ContextOptions::default())?
            .batch)
    }

    /// [`Self::context_candidates`] with 008 memory: the derived memory
    /// candidates are collected BEFORE the final read and validated against
    /// the live rows in the SAME final read transaction as the source and
    /// graph candidates, so one snapshot governs the whole response.
    pub fn context_candidates_memory(
        &self,
        query: &str,
        strategy: Strategy,
        control: &crate::Control,
    ) -> FResult<crate::memory::MemoryContext> {
        let options = ContextOptions {
            memory: true,
            ..ContextOptions::default()
        };
        self.context_candidates_with(query, strategy, control, &options)
    }

    /// [`Self::context_candidates`] with `options`: 008 memory, the 009 T002
    /// dense window fused into the candidate ranking, a configured 013 policy
    /// routing an `auto` strategy (AFTER the 009 merge and BEFORE doors are
    /// built, outside every engine transaction and under the request's own
    /// read deadline; the batch carries the route word), and the anchors a
    /// multi-root owner chose (007).
    pub fn context_candidates_with(
        &self,
        query: &str,
        strategy: Strategy,
        control: &crate::Control,
        options: &ContextOptions,
    ) -> FResult<crate::memory::MemoryContext> {
        if options.compact_evidence && !options.compose_evidence {
            return Err(FoundryError::InvalidArgument(
                "compact_evidence requires compose_evidence".into(),
            ));
        }
        if options.compose_evidence {
            return self.compose_task_evidence(query, strategy, control, options);
        }
        if options.boundary_config.is_some() {
            return Err(FoundryError::InvalidArgument(
                "boundary_config requires compose_evidence".into(),
            ));
        }
        if let Some(task) = options.task {
            self.check_task_scope(task)?;
        }
        let (policy, anchors) = (options.policy, options.anchors);
        if let Some(trace) = control.trace() {
            trace.record(serde_json::json!({
                "stage": "query", "query_sha256": crate::digest(query.as_bytes()),
                "workspace_id": self.workspace_id, "strategy": strategy.to_string(),
            }));
        }
        let memory = options
            .memory
            .then(|| self.memory_plan(query))
            .transpose()?;
        if query.trim().is_empty() || query.len() > 4096 {
            return Err(FoundryError::InvalidArgument(
                "query must contain 1..4096 nonblank bytes".into(),
            ));
        }
        let selection = SearchSelection {
            anchors,
            prose: options.experimental_behavior
                && prose_context_query(query, strategy, policy.is_some()),
        };
        #[cfg(feature = "semantic")]
        let search = match options.dense {
            Some(dense) => self.search_candidates_semantic_selected(
                query,
                None,
                CONTEXT_UNITS,
                control,
                dense,
                selection,
            )?,
            None => {
                self.search_candidates_selected(query, None, CONTEXT_UNITS, control, selection)?
            }
        };
        #[cfg(not(feature = "semantic"))]
        let search =
            self.search_candidates_selected(query, None, CONTEXT_UNITS, control, selection)?;
        control.check()?;
        // 013 T003: routing happens HERE, after the 009 merge and before
        // doors are built. Only `auto` with a configured policy consults it;
        // explicit strategies never do, and without a policy the
        // deterministic rule (the doors request words) decides.
        let (resolved, route) = match (strategy, policy) {
            (Strategy::Auto, Some(policy)) => {
                let (resolved, word) = policy.route(self, query, control)?;
                (resolved, Some(word))
            }
            (Strategy::Auto, None) => (response::strategy_for_query(query), None),
            (explicit, _) => (explicit, None),
        };
        let behavior_terms = search
            .anchors
            .first()
            .and_then(|anchor| anchor.entries.first())
            .and_then(|entry| entry.handle.clone())
            .zip(
                (options.experimental_behavior && resolved == Strategy::Search)
                    .then(|| behavior_terms(query, &search.anchors))
                    .flatten(),
            );
        #[allow(unused_mut)]
        let mut brief_plan = (options.briefs && search.anchors.is_empty())
            .then(|| self.record_plan(query, "brief"))
            .transpose()?;
        #[cfg(feature = "semantic")]
        if let (Some(plan), Some(dense)) = (&mut brief_plan, options.dense) {
            plan.prepend(dense.areas.iter().cloned());
        }
        // A context resolved to graph requests doors (context-v2 § Doors),
        // for the first anchor's target as resolved from the collected
        // windows: the final read can drop that definition, never retarget.
        // `Some(None)`: requested, but the query has no anchor.
        let collected: Option<Option<CollectedAnchor>> =
            (resolved == Strategy::Graph).then(|| search.anchors.first().map(CollectedAnchor::of));
        control.check()?;
        // Candidates are collected. Anything may commit before the final read.
        fault!(
            CONTEXT_BEFORE_FINAL_VALIDATION,
            Some(self),
            Some(control),
            ""
        )?;
        let tx = self.db.begin_read()?;
        let task_view = options
            .task
            .map(|scope| self.task_view_in(&tx, scope))
            .transpose()?;
        let mut sources = FinalSources::open(&tx)?;
        let mut counters = search.counters;
        let mut units: Vec<RankedItem> = Vec::new();
        let mut dropped: std::collections::BTreeSet<(String, u64, u64)> =
            std::collections::BTreeSet::new();
        for item in search.items {
            let Some(handle) = &item.handle else {
                continue;
            };
            if sources.is_current(&handle.path, &handle.sha256)? {
                units.push(item);
            } else {
                if let Some(trace) = control.trace() {
                    trace.record(
                        serde_json::json!({"stage":"validation", "outcome":"stale", "unit":handle}),
                    );
                }
                dropped.insert((handle.path.clone(), handle.start, handle.end));
                counters.stale += 1;
            }
        }
        // The anchor windows' definitions validate in the same read; one the
        // units above already dropped is counted once.
        let mut anchors = search.anchors;
        for window in &mut anchors {
            let mut fresh_entries = Vec::with_capacity(window.entries.len());
            for entry in std::mem::take(&mut window.entries) {
                let Some(handle) = &entry.handle else {
                    continue;
                };
                if sources.is_current(&handle.path, &handle.sha256)? {
                    fresh_entries.push(entry);
                } else if dropped.insert((handle.path.clone(), handle.start, handle.end)) {
                    if let Some(trace) = control.trace() {
                        trace.record(serde_json::json!({"stage":"validation", "origin":"anchor", "outcome":"stale", "unit":handle}));
                    }
                    counters.stale += 1;
                }
            }
            window.entries = fresh_entries;
        }
        // A packing repair, not another candidate generator: preserve the
        // original lexical window, caps and ranks. Final-read identity must
        // still match the one used to recognize behavioral intent.
        if let Some((owner, terms)) = behavior_terms
            && anchors.len() == 1
            && anchors[0].resolved()
            && let Some(anchor) = anchors[0].entries.first()
            && anchor.handle.as_ref() == Some(&owner)
        {
            for item in &mut units {
                control.check()?;
                if behavior_supports(anchor, item, &terms) {
                    item.tier = TIER_BEHAVIOR;
                    if let Some(trace) = control.trace() {
                        trace.record(serde_json::json!({"stage":"behavior_support",
                            "unit":item.handle, "anchor":anchor.handle,
                            "reason":"same_source_owned_method_with_query_terms"}));
                    }
                }
            }
        }
        // Doors, from the validated windows, in this same read.
        let doors = match &collected {
            Some(first) => {
                let target = first
                    .as_ref()
                    .map_or(DoorTarget::None, CollectedAnchor::target);
                Some(self.doors_in(&tx, &target, &anchors, &mut counters, control)?)
            }
            None => None,
        };
        let briefs = match &brief_plan {
            Some(plan) => {
                let (hits, stale, _) =
                    self.brief_hits_in(&tx, plan, crate::brief::CONTEXT_HITS, control)?;
                counters.stale += stale;
                counters.candidates_full |= plan.window_full;
                let mut evidence = Vec::new();
                for hit in &hits {
                    let source = self.brief_source_in(&tx, hit)?;
                    // One primary representation per source; generated forms do
                    // not displace independent files in the source window.
                    units.retain(|unit| {
                        unit.handle.as_ref().map(|h| &h.path)
                            != source.handle.as_ref().map(|h| &h.path)
                    });
                    evidence.push(source);
                }
                evidence.append(&mut units);
                evidence.truncate(CONTEXT_UNITS);
                units = evidence;
                hits
            }
            None => Vec::new(),
        };
        // Verified bodies, once per path, for the signature forms of units in
        // languages with units and for the first distinct files' outlines.
        let mut outline_paths: Vec<String> = Vec::new();
        for item in &units {
            let Some(handle) = &item.handle else {
                continue;
            };
            if outline_paths.len() < CONTEXT_OUTLINES && !outline_paths.contains(&handle.path) {
                outline_paths.push(handle.path.clone());
            }
        }
        // The anchor windows' definitions render their signatures from the
        // same verified bodies.
        let anchor_entries = anchors.iter().flat_map(|window| &window.entries);
        for item in units.iter().chain(anchor_entries) {
            let Some(handle) = &item.handle else {
                continue;
            };
            let has_units = crate::syntax::Lang::from_path(&handle.path)
                .is_some_and(crate::syntax::Lang::has_units);
            let wanted =
                (has_units && item.label != "block") || outline_paths.contains(&handle.path);
            if wanted {
                sources.body(&handle.path)?;
            }
        }
        let outliners: std::collections::BTreeMap<&str, crate::syntax::Outliner> = sources
            .verified_bodies()
            .filter_map(|(path, body)| {
                crate::syntax::Lang::from_path(path)
                    .map(|lang| (path, crate::syntax::Outliner::new(body, lang)))
            })
            .collect();
        // Signature forms. Each unit's renders once; an anchor entry with a
        // unit's full handle reuses that text instead of rendering it again.
        let signatures: Vec<Option<String>> = units
            .iter()
            .map(|item| render_signature(item, &outliners))
            .collect();
        for entry in anchors
            .iter_mut()
            .flat_map(|window| window.entries.iter_mut())
        {
            if entry.label == "block" {
                continue;
            }
            let reused = units
                .iter()
                .zip(&signatures)
                .find_map(|(unit, text)| text.as_deref().filter(|_| unit.handle == entry.handle));
            let signature = match reused {
                Some(text) => std::borrow::Cow::Borrowed(text),
                None => match render_signature(entry, &outliners) {
                    Some(text) => std::borrow::Cow::Owned(text),
                    None => continue,
                },
            };
            append_signature(&mut entry.forms, signature);
        }
        for (item, signature) in units.iter_mut().zip(signatures) {
            if let Some(signature) = signature {
                append_signature(&mut item.forms, std::borrow::Cow::Owned(signature));
            }
        }
        // An anchored definition's last rung (context-v2 § Ladder for
        // anchored definitions): its item line alone, `[address]`.
        for entry in anchors
            .iter_mut()
            .flat_map(|window| window.entries.iter_mut())
        {
            entry.forms.push(RenderedForm::Address);
        }
        let mut outlines: Vec<RankedItem> = Vec::new();
        for path in &outline_paths {
            let (Some((meta, body)), Some(outliner)) =
                (sources.verified_body(path), outliners.get(path.as_str()))
            else {
                continue;
            };
            // Skip a file when one of the units spans the whole file.
            let whole = units
                .iter()
                .filter_map(|item| item.handle.as_ref())
                .any(|handle| {
                    handle.path == *path
                        && body[..handle.start as usize].trim().is_empty()
                        && body[handle.end as usize..].trim().is_empty()
                });
            if whole || body.is_empty() {
                continue;
            }
            let end_line = 1 + body.as_bytes()[..body.len() - 1]
                .iter()
                .filter(|&&b| b == b'\n')
                .count() as u64;
            outlines.push(RankedItem {
                tier: TIER_OUTLINE,
                rank: 0,
                score: 0.0,
                handle: Some(SourceHandle {
                    workspace_id: self.workspace_id.clone().unwrap_or_default(),
                    path: path.clone(),
                    sha256: meta.hash.clone(),
                    start: 0,
                    end: body.len() as u64,
                }),
                start_line: 1,
                end_line,
                line: 1,
                label: String::new(),
                lang: crate::syntax::Lang::from_path(path).map(|lang| lang.tag().to_owned()),
                semantic: None,
                resolver: None,
                forms: vec![
                    RenderedForm::Outline(outliner.render(0..body.len(), 60, 120)),
                    RenderedForm::OutlineMin(outliner.render(0..body.len(), 0, 0)),
                ],
            });
        }
        // 008: memory rows validate in this same transaction; their stale
        // drops and a filled candidate window join the batch's counters.
        let hits = match memory {
            Some(plan) => {
                let table = tx.open_table(MEMORY)?;
                let (hits, stale) = crate::memory::validate_hits(
                    &table,
                    plan.candidates,
                    crate::memory::CONTEXT_MEMORY_HITS,
                )?;
                counters.stale += stale;
                if plan.window_full {
                    counters.candidates_full = true;
                }
                hits
            }
            None => Vec::new(),
        };
        let freshness = self.freshness_in(&tx)?;
        // 009 T002: the coverage word must describe THIS final read, not the
        // earlier search read: a source committed in between moves the
        // revision past the serving generation, so `ready` cannot carry over.
        #[cfg(feature = "semantic")]
        let semantic = match options.dense {
            Some(dense) => Some(dense.coverage_at(freshness.source_revision).to_owned()),
            None => search.semantic.clone(),
        };
        #[cfg(not(feature = "semantic"))]
        let semantic = search.semantic.clone();
        // The units, then the outlines.
        let mut items = units;
        items.extend(outlines);
        for (rank, item) in items.iter_mut().enumerate() {
            item.rank = rank;
        }
        let context = crate::memory::MemoryContext {
            evidence: None,
            briefs,
            task_view,
            batch: CandidateBatch {
                semantic,
                route,
                freshness,
                items,
                counters,
                // The anchored selection packs from these windows (context-v2
                // § Anchored context); ranking and routing above never read
                // them.
                anchors,
                doors,
                collected: collected.flatten(),
            },
            hits,
        };
        if let Some(trace) = control.trace() {
            trace.record(serde_json::json!({
                "stage": "packing_input", "workspace_id": self.workspace_id,
                "source_revision": context.batch.freshness.source_revision,
                "semantic": context.batch.semantic, "anchored": context.batch.anchored(),
                "items": context.batch.items.iter().map(|i| serde_json::json!({
                    "unit":i.handle, "label":i.label, "rank":i.rank, "tier":i.tier,
                })).collect::<Vec<_>>(),
                "anchors": context.batch.anchors.iter().map(|a| serde_json::json!({
                    "resolved": a.resolved(), "definitions": a.definitions,
                    "units": a.entries.iter().filter_map(|i| i.handle.as_ref()).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            }));
        }
        Ok(context)
    }
}
