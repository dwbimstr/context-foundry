//! 003 T009: bounded task-guided retrieval, with coverage computed from the
//! evidence that survives final delivery. No model calls or task-state edits.
use crate::store::{ContextOptions, HandleRef, RankedItem, RenderedForm, SOURCES, SourceMeta};
use crate::task_context::{Entry, Kind};
use crate::{Control, Engine, FResult, FoundryError, SourceHandle, Strategy};
use redb::ReadableDatabase;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
pub(crate) mod reference;

const MAX_ITEMS: usize = 32;
const MAX_SPAN_BYTES: u64 = 32 * 1024;
#[derive(Clone, Debug)]
pub struct Support {
    pub facets: BTreeSet<&'static str>,
    pub via: BTreeSet<String>,
}
pub struct Plan {
    pub compact: bool,
    pub support: Vec<Support>,
    pub boundaries: Option<crate::boundary::Relations>,
    pub boundary_requested: bool,
    pub queried_entries: Vec<String>,
    pub unqueried_entries: usize,
    pub stale_links: usize,
    pub discovery_limited: bool,
    pub constraints: usize,
    pub compiler: Vec<crate::graph::ReferencesOutcome>,
    pub observed: Vec<crate::graph::ObservedEvidence>,
    /// Explicit requirement-to-source links, validated in the final snapshot.
    /// An execution overlap is traceability, never proof of requirement success.
    pub(crate) requirement_links: Vec<(String, SourceHandle)>,
    pub compiler_skipped_seeds: usize,
    pub syntax_candidates: usize,
    pub syntax_skipped_paths: usize,
    pub syntax_index_candidates: usize,
    pub syntax_index_stale: usize,
    pub syntax_index_limited: bool,
    pub local_test_candidates: usize,
    pub(crate) reference_uses: BTreeMap<String, reference::Use>,
    pub(crate) import_alias_calls: Vec<(SourceHandle, reference::ImportAliasCall)>,
    pub roots: Vec<String>,
}
impl Plan {
    /// Preserve primary task ownership and align attribution after ordinary
    /// root ranking and its single 32-unit cap. Workspace identity is part of
    /// the key even when paths, byte ranges and source bytes are identical.
    pub(crate) fn merge(
        mut plans: Vec<Self>,
        batches: &[crate::roots::RootBatch],
        merged: &crate::store::CandidateBatch,
    ) -> Self {
        let mut support = BTreeMap::new();
        for (plan, batch) in plans.iter_mut().zip(batches) {
            for (item, attribution) in batch
                .batch
                .items
                .iter()
                .zip(std::mem::take(&mut plan.support))
            {
                if let Some(handle) = &item.handle {
                    support.insert(handle.to_v2(), attribution);
                }
            }
        }
        let mut primary = plans.remove(0);
        primary.discovery_limited |= support.len() > merged.items.len();
        for other in plans {
            primary.roots.extend(other.roots);
            primary.compiler.extend(other.compiler);
            primary.observed.extend(other.observed);
            primary.requirement_links.extend(other.requirement_links);
            primary.reference_uses.extend(other.reference_uses);
            primary.import_alias_calls.extend(other.import_alias_calls);
            primary.compiler_skipped_seeds += other.compiler_skipped_seeds;
            primary.syntax_candidates += other.syntax_candidates;
            primary.syntax_skipped_paths += other.syntax_skipped_paths;
            primary.syntax_index_candidates += other.syntax_index_candidates;
            primary.syntax_index_stale += other.syntax_index_stale;
            primary.syntax_index_limited |= other.syntax_index_limited;
            primary.local_test_candidates += other.local_test_candidates;
            primary.discovery_limited |= other.discovery_limited;
        }
        primary.support = merged
            .items
            .iter()
            .map(|item| {
                support
                    .remove(&item.handle.as_ref().expect("evidence has a source").to_v2())
                    .expect("every merged source has attribution")
            })
            .collect();
        primary
    }

    pub(crate) fn observed_requirements(
        &self,
        evidence: &crate::graph::ObservedEvidence,
    ) -> Vec<&str> {
        self.requirement_links
            .iter()
            .filter_map(|(id, link)| {
                // The target is the observed call/covered line. The observing test
                // alone and a lexical similarity to a requirement establish no join.
                let target = &evidence.to;
                (link.workspace_id == target.workspace_id
                    && link.path == target.path
                    && link.sha256 == target.sha256
                    && link.start < target.end
                    && target.start < link.end)
                    .then_some(id.as_str())
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Availability is not delivery and lexical matching is not task completion.
    pub(crate) fn report(
        &self,
        items: &[RankedItem],
        included: &[(usize, usize)],
        brief_count: usize,
        relationship_count: usize,
        compiler_count: usize,
        observed_count: usize,
    ) -> Value {
        let facets: BTreeMap<_,_> = ["implementation","consumers","configuration","verification","constraints"].into_iter().map(|facet| {
            let candidates = self.support.iter().filter(|s|s.facets.contains(facet)).count();
            let bodies = included.iter().filter(|&&(i,f)|f==0 && self.support[i].facets.contains(facet)).count();
            let addresses = included.iter().filter(|&&(i,f)|f>0 && self.support[i].facets.contains(facet)).count();
            (facet,json!({"state":if bodies>0 {"source_candidates_delivered"} else if addresses>0 {"addresses_only"} else if candidates>0 {"omitted_budget"} else {"missing"},"bodies":bodies,"addresses":addresses,"candidates":candidates}))
        }).collect();
        let mut resolution = BTreeMap::<&str, usize>::new();
        let mut syntax = json!({"kind":"call_name_candidates","resolution":"unresolved","runtime_call":"unverified","available":self.syntax_candidates,"skipped_paths":self.syntax_skipped_paths,"discovery":{"scope":"admitted_source_call_postings","candidates":self.syntax_index_candidates,"stale":self.syntax_index_stale,"limited":self.syntax_index_limited,"coverage":"syntax_recipe_only_not_complete_evidence"},"local_tests":{"available":self.local_test_candidates,"basis":"same_file_task_terms","coverage":"not_established"}});
        if !self.import_alias_calls.is_empty() {
            syntax["import_aliases"] = json!({"available":self.import_alias_calls.len(),"basis":"explicit_module_from_import","resolution":"unresolved"});
        }
        if let Some(b) = &self.boundaries {
            for r in &b.relations {
                *resolution.entry(r.resolution).or_default() += 1;
            }
        }
        if self.compact {
            return json!({"v":2,"complete":false,"host_application":"unknown",
                "facets":facets.iter().map(|(name,value)|(*name,value["state"].clone())).collect::<BTreeMap<_,_>>(),
                "constraints":{"caller_assertions":self.constraints,"verified":false,"conflicts":"not_evaluated"},
                "discovery":{"queried_entries":self.queried_entries,"unqueried_entries":self.unqueried_entries,"stale_links":self.stale_links,"limited":self.discovery_limited},
                "boundary":{"requested":self.boundary_requested,"available":self.boundaries.as_ref().map_or(0,|b|b.relations.len()),"delivered":relationship_count,"resolution":resolution,"incomplete_paths":self.boundaries.as_ref().map(|b|&b.incomplete_paths),"truncated":self.boundaries.as_ref().is_some_and(|b|b.truncated)},
                "compiler":{"kind":"references","delivered":compiler_count,"skipped_seeds":self.compiler_skipped_seeds,"windows":self.compiler.iter().map(|c|json!({"coverage":c.coverage,"references":c.items.len(),"stale":c.stale,"limited":c.candidates_full||c.more||c.definitions_truncated})).collect::<Vec<_>>()},
                "syntax":syntax,"derived_briefs":brief_count,"verification":if observed_count > 0 {"producer_execution_observations_delivered"} else {"test_candidates_only_not_executed"},
                "execution":{"available":self.observed.len(),"delivered":observed_count,"complete_coverage":false,"attestation":"producer_report_only"},
                "sources":included.iter().enumerate().map(|(n,&(i,f))|json!({"item":n+1,"representation":if f==0{"body"}else{"address"},"facets":self.support[i].facets,"via":self.support[i].via})).collect::<Vec<_>>()});
        }
        json!({"v":1,"scope":if self.roots.len()>1 {"admitted_roots_primary_task"} else {"primary_task"},"roots":self.roots,"complete":false,"host_application":"unknown",
            "orientation":{"task_view":"delivered","derived_briefs":brief_count},
            "constraints":{"caller_assertions":self.constraints,"verified":false,"conflicts":"not_evaluated"},"facets":facets,
            "discovery":{"queried_entries":self.queried_entries,"unqueried_entries":self.unqueried_entries,"stale_links":self.stale_links,"limited":self.discovery_limited},
            "boundary":{"available_relationships":self.boundaries.as_ref().map_or(0,|b|b.relations.len()),"delivered_relationships":relationship_count,"state":if self.boundaries.is_some(){"configured"}else if self.boundary_requested{"no_fresh_seeds"}else{"not_requested"},"resolution":resolution,"incomplete_paths":self.boundaries.as_ref().map(|b|&b.incomplete_paths),"truncated":self.boundaries.as_ref().is_some_and(|b|b.truncated)},
            "syntax":syntax,"verification":if observed_count > 0 {"producer_execution_observations_delivered"} else {"test_candidates_only_not_executed"},
            "execution":{"available":self.observed.len(),"delivered":observed_count,"complete_coverage":false,"attestation":"producer_report_only"},
            "compiler_relationships":{"kind":"references","skipped_seeds":self.compiler_skipped_seeds,"delivered":compiler_count,"windows":self.compiler.iter().map(|c|json!({"workspace_id":c.freshness.workspace_id,"producer":c.producer,"coverage":c.coverage,"target":c.target,"references":c.items.len(),"examined":c.examined,"stale":c.stale,"limited":c.candidates_full||c.more||c.definitions_truncated})).collect::<Vec<_>>()},
            "sources":included.iter().map(|&(i,f)|json!({"handle":items[i].handle.as_ref().map(SourceHandle::to_v2),"representation":if f==0{"body"}else{"address"},"facets":self.support[i].facets,"via":self.support[i].via})).collect::<Vec<_>>()})
    }
}
fn test_path(path: &str) -> bool {
    path.split('/').any(|p| {
        p == "tests"
            || p == "test"
            || p.starts_with("test_")
            || p.ends_with("_test.py")
            || p.ends_with("_test.go")
            || p.contains(".test.")
            || p.contains(".spec.")
    })
}
fn facet(path: &str) -> &'static str {
    if test_path(path) {
        "verification"
    } else {
        "implementation"
    }
}
struct Candidate {
    handle: SourceHandle,
    support: Support,
}
fn add(
    items: &mut Vec<Candidate>,
    mut handle: SourceHandle,
    facet: &'static str,
    via: &str,
) -> bool {
    let mut support = Support {
        facets: [facet].into_iter().collect(),
        via: [via.into()].into_iter().collect(),
    };
    let mut first = items.len();
    // Repeat after each union: one new span can bridge several previous
    // discoveries. Merge their attribution as well as their source bytes.
    loop {
        let Some(i) = items.iter().position(|old| {
            old.handle.workspace_id == handle.workspace_id
                && old.handle.path == handle.path
                && old.handle.sha256 == handle.sha256
                && old.handle.start <= handle.end
                && handle.start <= old.handle.end
                && old.handle.end.max(handle.end) - old.handle.start.min(handle.start)
                    <= MAX_SPAN_BYTES
        }) else {
            break;
        };
        let old = items.remove(i);
        first = first.min(i);
        handle.start = handle.start.min(old.handle.start);
        handle.end = handle.end.max(old.handle.end);
        support.facets.extend(old.support.facets);
        support.via.extend(old.support.via);
    }
    if items.len() == MAX_ITEMS {
        return false;
    }
    items.insert(first.min(items.len()), Candidate { handle, support });
    true
}

impl Engine {
    pub(crate) fn compose_task_evidence(
        &self,
        query: &str,
        strategy: Strategy,
        control: &Control,
        options: &ContextOptions,
    ) -> FResult<crate::memory::MemoryContext> {
        let scope = options.task.ok_or_else(|| {
            FoundryError::InvalidArgument("compose_evidence requires task and revision".into())
        })?;
        let entries = self.task_entries_in(&self.db.begin_read()?, scope)?;
        self.compose_evidence_for_entries(query, strategy, control, options, &entries)
    }

    /// Reference roots consume primary task goals, never foreign task state.
    pub(crate) fn compose_evidence_for_entries(
        &self,
        query: &str,
        strategy: Strategy,
        control: &Control,
        options: &ContextOptions,
        entries: &[Entry],
    ) -> FResult<crate::memory::MemoryContext> {
        let goals: Vec<_> = entries
            .iter()
            .filter(|e| matches!(e.kind, Kind::Requirement | Kind::Question))
            .collect();
        let mut plan = Plan {
            compact: options.compact_evidence,
            support: vec![],
            compiler: vec![],
            observed: vec![],
            requirement_links: vec![],
            compiler_skipped_seeds: 0,
            syntax_candidates: 0,
            syntax_skipped_paths: 0,
            syntax_index_candidates: 0,
            syntax_index_stale: 0,
            syntax_index_limited: false,
            local_test_candidates: 0,
            reference_uses: BTreeMap::new(),
            import_alias_calls: Vec::new(),
            roots: vec![self.workspace_id().ok_or(FoundryError::WorkspaceUnbound)?],
            boundaries: None,
            boundary_requested: options.boundary_config.is_some(),
            queried_entries: vec![],
            unqueried_entries: goals.len().saturating_sub(4),
            stale_links: 0,
            discovery_limited: goals.len() > 4,
            constraints: entries
                .iter()
                .filter(|e| matches!(e.kind, Kind::Constraint))
                .count(),
        };
        let mut plain = *options;
        plain.compose_evidence = false;
        plain.compact_evidence = false;
        plain.boundary_config = None;
        plain.briefs = false;
        let mut context = self.context_candidates_with(query, strategy, control, &plain)?;
        let syntax_seeds: Vec<_> = context
            .batch
            .anchors
            .iter()
            .filter(|anchor| anchor.resolved())
            .filter_map(|anchor| {
                let entry = anchor.entries.first()?;
                let (start, end) = entry.resolver?.name;
                let definition = entry.handle.clone()?;
                Some((
                    SourceHandle {
                        start,
                        end,
                        ..definition.clone()
                    },
                    definition,
                ))
            })
            .take(4)
            .collect();
        // Canonical spelling comes from the already materialized declaration,
        // not the query's casing. Query postings before the final snapshot; both
        // the definition and candidate source identity are rechecked below.
        let syntax_names: Vec<String> = context
            .batch
            .anchors
            .iter()
            .filter(|anchor| anchor.resolved())
            .take(4)
            .filter_map(|anchor| {
                let entry = anchor.entries.first()?;
                let handle = entry.handle.as_ref()?;
                let (start, end) = entry.resolver?.name;
                let start = usize::try_from(start.checked_sub(handle.start)?).ok()?;
                let end = usize::try_from(end.checked_sub(handle.start)?).ok()?;
                entry.forms.iter().find_map(|form| match form {
                    RenderedForm::Verbatim(body) => body.get(start..end).map(str::to_owned),
                    _ => None,
                })
            })
            .collect();
        let (indexed_call_sources, call_window_limited) =
            self.task_call_sources(&syntax_names, control)?;
        plan.syntax_index_candidates = indexed_call_sources.len();
        plan.syntax_index_limited = call_window_limited
            || syntax_names.len() < syntax_seeds.len()
            || context.batch.freshness.pending_sources > 0;
        plan.discovery_limited |= plan.syntax_index_limited;
        let mut candidates = Vec::new();
        // Outlines describe a file; their handle is not a request for its entire
        // body. Promoting them to source spans swallows the selected functions
        // during union and can leave only an address under a small budget.
        let source_seeds: Vec<_> = context
            .batch
            .items
            .iter()
            .filter(|item| item.tier != crate::store::TIER_OUTLINE)
            .collect();
        plan.discovery_limited |= source_seeds.len() > 8 || context.batch.counters.candidates_full;
        // Compiler handle resolution needs exact definition units, so retain
        // original query spans before evidence-body union merges adjacent units.
        let mut compiler_seeds = Vec::new();
        let mut seed_seen = BTreeSet::new();
        let mut compiler_seed = |h: &SourceHandle| {
            if seed_seen.insert(h.to_v2()) {
                if compiler_seeds.len() < 4 {
                    compiler_seeds.push(h.clone());
                    false
                } else {
                    true
                }
            } else {
                false
            }
        };
        // Keep a bounded primary-query seed window before task-specific
        // discovery so one broad query cannot consume every evidence slot.
        for i in source_seeds.into_iter().take(8) {
            if let Some(h) = &i.handle {
                plan.discovery_limited |= compiler_seed(h);
                plan.discovery_limited |= !add(&mut candidates, h.clone(), facet(&h.path), "query");
            }
        }
        for e in goals.iter().take(4) {
            control.check()?;
            let mut end = e.text.len().min(1024);
            while !e.text.is_char_boundary(end) {
                end -= 1;
            }
            let text = e.text[..end].trim();
            if text.is_empty() {
                continue;
            }
            plan.queried_entries.push(e.id.clone());
            plan.discovery_limited |= end < e.text.len();
            let batch = self.search_candidates(text, None, 4, control)?;
            plan.discovery_limited |= batch.counters.candidates_full;
            for i in batch.items {
                if let Some(h) = i.handle {
                    plan.discovery_limited |= compiler_seed(&h);
                    let f = facet(&h.path);
                    plan.discovery_limited |=
                        !add(&mut candidates, h, f, &format!("task:{}", e.id));
                }
            }
        }
        #[allow(unused_mut)]
        let mut brief_plan = options
            .briefs
            .then(|| self.record_plan(query, "brief"))
            .transpose()?;
        #[cfg(feature = "semantic")]
        if let (Some(plan), Some(dense)) = (&mut brief_plan, options.dense) {
            plan.prepend(dense.areas.iter().cloned());
        }
        fault!(TASK_EVIDENCE_BEFORE_FINAL, Some(self), Some(control), "")?;
        let tx = self.db.begin_read()?;
        // All task state, graph dependencies, briefs, source bytes and memory
        // below belong to this final snapshot, after optional external routing.
        context.task_view = options
            .task
            .map(|scope| self.task_view_in(&tx, scope))
            .transpose()?;
        let sources = tx.open_table(SOURCES)?;
        let wid = self.workspace_id().ok_or(FoundryError::WorkspaceUnbound)?;
        let mut linked = 0;
        for e in entries {
            for link in &e.source_links {
                if linked == 8 {
                    plan.discovery_limited = true;
                    continue;
                }
                linked += 1;
                let h = HandleRef::parse(link)?;
                if h.ws16 != wid[..16] && options.task.is_none() {
                    // Primary source links cannot admit or identify a reference root.
                    continue;
                }
                let meta: Option<SourceMeta> = sources
                    .get(h.path.as_str())?
                    .map(|r| crate::store::decode(r.value(), "source"))
                    .transpose()?;
                if let Some(m) =
                    meta.filter(|m| m.hash.starts_with(&h.sha32) && h.ws16 == wid[..16])
                {
                    if h.start >= h.end || h.end > m.bytes as u64 {
                        plan.stale_links += 1;
                        continue;
                    }
                    let handle = SourceHandle {
                        workspace_id: wid.clone(),
                        path: h.path.clone(),
                        sha256: m.hash.clone(),
                        start: h.start,
                        end: h.end,
                    };
                    if e.kind == Kind::Requirement {
                        let Some((_, body)) = crate::boundary::source_in(&tx, &h.path)? else {
                            plan.stale_links += 1;
                            continue;
                        };
                        if body.get(h.start as usize..h.end as usize).is_none() {
                            plan.stale_links += 1;
                            continue;
                        }
                        plan.requirement_links.push((e.id.clone(), handle.clone()));
                    }
                    let f = if e.kind == Kind::Constraint {
                        "constraints"
                    } else {
                        facet(&h.path)
                    };
                    plan.discovery_limited |=
                        !add(&mut candidates, handle, f, &format!("task:{}", e.id));
                } else {
                    plan.stale_links += 1;
                }
            }
        }
        // Discard stale discovery seeds before using them to expand the graph.
        let mut fresh = Vec::new();
        for c in candidates {
            let meta: Option<SourceMeta> = sources
                .get(c.handle.path.as_str())?
                .map(|r| crate::store::decode(r.value(), "source"))
                .transpose()?;
            if meta.is_some_and(|m| m.hash == c.handle.sha256) {
                fresh.push(c);
            } else {
                context.batch.counters.stale += 1;
            }
        }
        let mut candidates = fresh;
        if let Some(bp) = brief_plan {
            let (briefs, stale, omitted) = self.brief_hits_in(&tx, &bp, 4, control)?;
            context.batch.counters.stale += stale;
            plan.discovery_limited |= omitted > 0;
            for b in briefs {
                let i = self.brief_source_in(&tx, &b)?;
                if let Some(h) = i.handle {
                    if add(
                        &mut candidates,
                        h.clone(),
                        "orientation",
                        "derived_brief_citation",
                    ) {
                        add(&mut candidates, h.clone(), facet(&h.path), "brief_source");
                        context.briefs.push(b);
                    } else {
                        plan.discovery_limited = true;
                    }
                }
            }
        }
        if let Some(config) = options.boundary_config {
            let mut seen = BTreeSet::new();
            let paths: Vec<String> = candidates
                .iter()
                .map(|c| c.handle.path.clone())
                .filter(|p| seen.insert(p.clone()))
                .take(8)
                .collect();
            plan.discovery_limited |= candidates
                .iter()
                .map(|c| &c.handle.path)
                .collect::<BTreeSet<_>>()
                .len()
                > 8;
            if !paths.is_empty() {
                let mut b = self.boundary_relations_in(&tx, config, &paths, control)?;
                if b.relations.len() > 8 {
                    b.relations.truncate(8);
                    b.truncated = true;
                }
                plan.discovery_limited |= b.truncated;
                plan.discovery_limited |= !add(
                    &mut candidates,
                    b.config.handle(&wid),
                    "configuration",
                    "boundary_config",
                );
                for r in &b.relations {
                    let c = &r.consumer;
                    let cf = if test_path(&c.context.path) {
                        "verification"
                    } else {
                        "consumers"
                    };
                    let consumer_span = if c.context.end - c.context.start > MAX_SPAN_BYTES {
                        plan.discovery_limited = true;
                        &c.evidence
                    } else {
                        &c.context
                    };
                    plan.discovery_limited |= !add(
                        &mut candidates,
                        consumer_span.handle(&wid),
                        cf,
                        "syntactic_http_consumer",
                    );
                    for p in &r.providers {
                        for dependency in &p.dependencies {
                            plan.discovery_limited |= !add(
                                &mut candidates,
                                dependency.handle(&wid),
                                "configuration",
                                "syntactic_router_mount",
                            );
                        }
                        let provider_span = if p.context.end - p.context.start > MAX_SPAN_BYTES {
                            plan.discovery_limited = true;
                            &p.evidence
                        } else {
                            &p.context
                        };
                        plan.discovery_limited |= !add(
                            &mut candidates,
                            provider_span.handle(&wid),
                            "implementation",
                            "syntactic_http_provider",
                        );
                    }
                }
                plan.boundaries = Some(b);
            } else {
                plan.discovery_limited = true;
            }
        }
        let mut reference_sources = BTreeMap::new();
        for seed in compiler_seeds {
            control.check()?;
            let found = match self.references_in(
                &tx,
                &crate::graph::ReferencesRequest {
                    seed: crate::graph::ReferencesSeed::Handle(seed.to_v2()),
                    limit: 8,
                    after: None,
                },
            ) {
                Ok(found) => found,
                Err(FoundryError::InvalidArgument(reason))
                    if reason.starts_with("no_compiler_definition:") =>
                {
                    plan.compiler_skipped_seeds += 1;
                    continue;
                }
                Err(FoundryError::StaleHandle | FoundryError::NotFound) => {
                    plan.compiler_skipped_seeds += 1;
                    continue;
                }
                Err(error) => return Err(error),
            };
            plan.discovery_limited |=
                found.candidates_full || found.more || found.definitions_truncated;
            for reference in &found.items {
                if !reference_sources.contains_key(&reference.path) {
                    control.check()?;
                    let parsed = crate::boundary::source_in(&tx, &reference.path)?.and_then(
                        |(meta, body)| {
                            let tree =
                                crate::syntax::parse_reference_source(&reference.path, &body)?;
                            Some((meta.hash, body, tree))
                        },
                    );
                    reference_sources.insert(reference.path.clone(), parsed);
                }
                let usage = reference_sources[&reference.path]
                    .as_ref()
                    .filter(|(hash, _, _)| hash == &reference.sha256)
                    .map(|(_, body, tree)| reference::classify(tree, body, reference))
                    .unwrap_or_default();
                let f = if test_path(&reference.path) || usage.test.is_some() {
                    "verification"
                } else {
                    "consumers"
                };
                let mut handle = reference.unit.clone();
                if handle.end - handle.start > MAX_SPAN_BYTES {
                    handle.start = reference.start;
                    handle.end = reference.end;
                    plan.discovery_limited = true;
                }
                plan.discovery_limited |= !add(&mut candidates, handle, f, "compiler_reference");
                if let Some(call) = &usage.call {
                    plan.discovery_limited |= !add(
                        &mut candidates,
                        call.clone(),
                        f,
                        "compiler_reference_call_syntax",
                    );
                }
                if let Some(test) = &usage.test {
                    plan.discovery_limited |= !add(
                        &mut candidates,
                        test.clone(),
                        "verification",
                        "compiler_reference_test_attribute",
                    );
                }
                plan.reference_uses.insert(reference::key(reference), usage);
            }
            for definition in &found.definitions {
                plan.discovery_limited |= !add(
                    &mut candidates,
                    SourceHandle {
                        workspace_id: wid.clone(),
                        path: definition.path.clone(),
                        sha256: definition.sha256.clone(),
                        start: definition.start,
                        end: definition.end,
                    },
                    "implementation",
                    "compiler_definition",
                );
            }
            plan.compiler.push(found);
        }
        // Indexed call postings extend discovery across the admitted source set.
        // Hydrate at most eight files, reusing compiler parses. No source scan,
        // execution or inferred import resolution is needed. All identities and
        // call syntax must survive this final snapshot.
        let mut names = BTreeSet::new();
        let mut definitions = BTreeMap::new();
        let mut paths = Vec::new();
        for (seed, definition) in syntax_seeds {
            let Some((meta, body)) = crate::boundary::source_in(&tx, &seed.path)? else {
                continue;
            };
            if meta.hash == seed.sha256
                && let Some(name) = body.get(seed.start as usize..seed.end as usize)
            {
                names.insert(name.to_owned());
                definitions.insert(name.to_owned(), definition);
                if !paths.contains(&seed.path) {
                    paths.push(seed.path);
                }
            }
        }
        if !names.is_empty() {
            let seed_paths = std::mem::take(&mut paths);
            for indexed in indexed_call_sources {
                let meta: Option<SourceMeta> = sources
                    .get(indexed.path.as_str())?
                    .map(|row| crate::store::decode(row.value(), "source"))
                    .transpose()?;
                if meta.is_some_and(|meta| meta.hash == indexed.sha256) {
                    if !paths.contains(&indexed.path) {
                        paths.push(indexed.path);
                    }
                } else {
                    plan.syntax_index_stale += 1;
                    plan.syntax_index_limited = true;
                    plan.discovery_limited = true;
                }
            }
            for path in seed_paths {
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
            let mut test_words = BTreeSet::new();
            for text in std::iter::once(query).chain(goals.iter().take(4).map(|e| e.text.as_str()))
            {
                let mut end = text.len().min(1024);
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                plan.discovery_limited |= end < text.len();
                let text = &text[..end];
                for (start, end) in crate::syntax::code_subtokens(text) {
                    let word = text[start..end].to_lowercase();
                    if word.len() > 2 && !crate::store::prose_stopword(&word) {
                        if test_words.len() < 64 {
                            test_words.insert(word);
                        } else {
                            plan.discovery_limited |= !test_words.contains(&word);
                        }
                    }
                }
            }
            for candidate in &candidates {
                if !paths.contains(&candidate.handle.path) {
                    paths.push(candidate.handle.path.clone());
                }
            }
            plan.discovery_limited |= paths.len() > 8;
            for path in paths.into_iter().take(8) {
                control.check()?;
                if !reference_sources.contains_key(&path) {
                    let parsed =
                        crate::boundary::source_in(&tx, &path)?.and_then(|(meta, body)| {
                            let tree = crate::syntax::parse_reference_source(&path, &body)?;
                            Some((meta.hash, body, tree))
                        });
                    reference_sources.insert(path.clone(), parsed);
                }
                let Some((hash, body, tree)) = &reference_sources[&path] else {
                    plan.syntax_skipped_paths += 1;
                    continue;
                };
                let source = SourceHandle {
                    workspace_id: wid.clone(),
                    path: path.clone(),
                    sha256: hash.clone(),
                    start: 0,
                    end: body.len() as u64,
                };
                let (found, limited) = reference::candidates(
                    tree,
                    body,
                    &source,
                    &names,
                    MAX_ITEMS - plan.syntax_candidates,
                    control,
                )?;
                plan.discovery_limited |= limited;
                plan.syntax_candidates += found
                    .iter()
                    .map(|c| 1 + usize::from(c.alias.is_some()))
                    .sum::<usize>();
                for candidate in found {
                    let facet = if candidate.attributed_test || test_path(&path) {
                        "verification"
                    } else {
                        "consumers"
                    };
                    plan.discovery_limited |= !add(
                        &mut candidates,
                        candidate.handle,
                        facet,
                        if candidate.alias.is_some() {
                            "syntactic_import_alias_call_name"
                        } else if candidate.attributed_test {
                            "syntactic_test_call_name"
                        } else {
                            "syntactic_call_name"
                        },
                    );
                    if let Some(alias) = candidate.alias {
                        let definition = definitions[&alias.name].clone();
                        plan.discovery_limited |= !add(
                            &mut candidates,
                            alias.declaration.clone(),
                            facet,
                            "syntactic_import_alias_declaration",
                        );
                        plan.discovery_limited |= !add(
                            &mut candidates,
                            definition.clone(),
                            "implementation",
                            "syntactic_import_alias_anchor",
                        );
                        plan.import_alias_calls.push((definition, alias));
                    }
                }
                if plan.local_test_candidates < 4 {
                    let (tests, limited) =
                        reference::related_tests(tree, body, &source, &test_words, control)?;
                    plan.discovery_limited |=
                        limited || tests.len() > 4 - plan.local_test_candidates;
                    for handle in tests.into_iter().take(4 - plan.local_test_candidates) {
                        plan.local_test_candidates += 1;
                        plan.discovery_limited |= !add(
                            &mut candidates,
                            handle,
                            "verification",
                            "same_file_test_task_terms",
                        );
                    }
                } else {
                    plan.discovery_limited = true;
                }
                if plan.syntax_candidates == MAX_ITEMS {
                    plan.discovery_limited = true;
                    break;
                }
            }
        }
        // Compiler and syntax discovery can introduce the only source path
        // indexed by an execution observation. Query once after that frontier is
        // assembled, preserving the existing 8-path/128-row/16-observation caps.
        let paths: Vec<_> = candidates
            .iter()
            .map(|candidate| candidate.handle.path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let (observed, limited) = self.execution_evidence_in(&tx, &paths, control)?;
        plan.discovery_limited |= limited;
        for evidence in &observed {
            for (handle, facet) in [
                (&evidence.from, "consumers"),
                (&evidence.to, "implementation"),
                (&evidence.test, "verification"),
            ] {
                plan.discovery_limited |= !add(
                    &mut candidates,
                    handle.clone(),
                    facet,
                    "producer_execution_observation",
                );
            }
        }
        plan.observed = observed;
        let memory = tx.open_table(crate::memory::MEMORY)?;
        let mut hits = Vec::new();
        for hit in context.hits {
            if let Some(raw) = memory.get(hit.id.as_str())? {
                let row = crate::memory::decode_row(&hit.id, raw.value())?;
                if row.revision == hit.revision {
                    hits.push(hit);
                }
            }
        }
        context.hits = hits;
        let mut items = Vec::new();
        // Round-robin facets before remaining discoveries, so tests/consumers
        // can fit even when many implementation candidates matched a query.
        let mut order = Vec::new();
        for f in [
            "implementation",
            "consumers",
            "configuration",
            "verification",
            "constraints",
            "orientation",
        ] {
            if let Some(i) = candidates.iter().position(|c| c.support.facets.contains(f))
                && !order.contains(&i)
            {
                order.push(i);
            }
        }
        for i in 0..candidates.len() {
            if !order.contains(&i) {
                order.push(i);
            }
        }
        for i in order {
            control.check()?;
            let c = &candidates[i];
            let Some((meta, body)) = crate::boundary::source_in(&tx, &c.handle.path)? else {
                continue;
            };
            if meta.hash != c.handle.sha256 {
                context.batch.counters.stale += 1;
                continue;
            }
            let mut h = c.handle.clone();
            if h.end - h.start > MAX_SPAN_BYTES {
                h.end = h.start + MAX_SPAN_BYTES;
                while !body.is_char_boundary(h.end as usize) {
                    h.end -= 1;
                }
                plan.discovery_limited = true;
            }
            let Some(text) = body.get(h.start as usize..h.end as usize) else {
                context.batch.counters.stale += 1;
                continue;
            };
            let start_line = 1 + body[..h.start as usize]
                .bytes()
                .filter(|b| *b == b'\n')
                .count() as u64;
            let end_line = start_line
                + text.as_bytes()[..text.len().saturating_sub(1)]
                    .iter()
                    .filter(|&&b| b == b'\n')
                    .count() as u64;
            items.push(RankedItem {
                tier: 2,
                rank: items.len(),
                score: 0.0,
                handle: Some(h),
                start_line,
                end_line,
                line: start_line,
                label: "task_evidence".into(),
                lang: None,
                forms: vec![RenderedForm::Verbatim(text.into()), RenderedForm::Address],
                semantic: None,
                resolver: None,
            });
            plan.support.push(c.support.clone());
        }
        let freshness = self.freshness_in(&tx)?;
        plan.syntax_index_limited |= freshness.pending_sources > 0;
        plan.discovery_limited |= plan.syntax_index_limited;
        if freshness.source_revision != context.batch.freshness.source_revision
            && context.batch.semantic.is_some()
        {
            context.batch.semantic = Some("partial".into());
        }
        context.batch.freshness = freshness;
        context.batch.items = items;
        context.batch.anchors.clear();
        context.batch.doors = None;
        context.batch.collected = None;
        context.evidence = Some(plan);
        Ok(context)
    }
}
