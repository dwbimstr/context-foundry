//! 003 T009: bounded task-guided retrieval, with coverage computed from the
//! evidence that survives final delivery. No model calls or task-state edits.
use crate::store::{ContextOptions, HandleRef, RankedItem, RenderedForm, SOURCES, SourceMeta};
use crate::task_context::Kind;
use crate::{Control, Engine, FResult, FoundryError, SourceHandle, Strategy};
use redb::ReadableDatabase;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const MAX_ITEMS: usize = 32;
const MAX_SPAN_BYTES: u64 = 32 * 1024;
#[derive(Clone, Debug)]
pub struct Support {
    pub facets: BTreeSet<&'static str>,
    pub via: BTreeSet<String>,
}
pub struct Plan {
    pub support: Vec<Support>,
    pub boundaries: Option<crate::boundary::Relations>,
    pub boundary_requested: bool,
    pub queried_entries: Vec<String>,
    pub unqueried_entries: usize,
    pub stale_links: usize,
    pub discovery_limited: bool,
    pub constraints: usize,
}
impl Plan {
    /// Availability is not delivery and lexical matching is not task completion.
    pub(crate) fn report(
        &self,
        items: &[RankedItem],
        included: &[(usize, usize)],
        brief_count: usize,
        relationship_count: usize,
    ) -> Value {
        let facets: BTreeMap<_,_> = ["implementation","consumers","configuration","verification","constraints"].into_iter().map(|facet| {
            let candidates = self.support.iter().filter(|s|s.facets.contains(facet)).count();
            let bodies = included.iter().filter(|&&(i,f)|f==0 && self.support[i].facets.contains(facet)).count();
            let addresses = included.iter().filter(|&&(i,f)|f>0 && self.support[i].facets.contains(facet)).count();
            (facet,json!({"state":if bodies>0 {"source_candidates_delivered"} else if addresses>0 {"addresses_only"} else if candidates>0 {"omitted_budget"} else {"missing"},"bodies":bodies,"addresses":addresses,"candidates":candidates}))
        }).collect();
        let mut resolution = BTreeMap::<&str, usize>::new();
        if let Some(b) = &self.boundaries {
            for r in &b.relations {
                *resolution.entry(r.resolution).or_default() += 1;
            }
        }
        json!({"v":1,"scope":"primary_task","complete":false,"host_application":"unknown",
            "orientation":{"task_view":"delivered","derived_briefs":brief_count},
            "constraints":{"caller_assertions":self.constraints,"verified":false,"conflicts":"not_evaluated"},"facets":facets,
            "discovery":{"queried_entries":self.queried_entries,"unqueried_entries":self.unqueried_entries,"stale_links":self.stale_links,"limited":self.discovery_limited},
            "boundary":{"available_relationships":self.boundaries.as_ref().map_or(0,|b|b.relations.len()),"delivered_relationships":relationship_count,"state":if self.boundaries.is_some(){"configured"}else if self.boundary_requested{"no_fresh_seeds"}else{"not_requested"},"resolution":resolution,"incomplete_paths":self.boundaries.as_ref().map(|b|&b.incomplete_paths),"truncated":self.boundaries.as_ref().is_some_and(|b|b.truncated)},
            "verification":"test_candidates_only_not_executed",
            "compiler_relationships":"not_composed",
            "sources":included.iter().map(|&(i,f)|json!({"handle":items[i].handle.as_ref().map(SourceHandle::to_v2),"representation":if f==0{"body"}else{"address"},"facets":self.support[i].facets,"via":self.support[i].via})).collect::<Vec<_>>()})
    }
}
fn test_path(path: &str) -> bool {
    path.split('/').any(|p| {
        p == "tests"
            || p == "test"
            || p.starts_with("test_")
            || p.ends_with("_test.py")
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
            old.handle.path == handle.path
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
        if options.anchors.is_some() {
            return Err(FoundryError::UnsupportedMode(
                "task evidence composition currently supports the primary store only".into(),
            ));
        }
        let entries = self.task_entries_in(&self.db.begin_read()?, scope)?;
        let goals: Vec<_> = entries
            .iter()
            .filter(|e| matches!(e.kind, Kind::Requirement | Kind::Question))
            .collect();
        let mut plan = Plan {
            support: vec![],
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
        plain.boundary_config = None;
        plain.briefs = false;
        let mut context = self.context_candidates_with(query, strategy, control, &plain)?;
        let mut candidates = Vec::new();
        plan.discovery_limited |=
            context.batch.items.len() > 8 || context.batch.counters.candidates_full;
        // Keep a bounded primary-query seed window before task-specific
        // discovery so one broad query cannot consume every evidence slot.
        for i in context.batch.items.iter().take(8) {
            if let Some(h) = &i.handle {
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
                    let f = facet(&h.path);
                    plan.discovery_limited |=
                        !add(&mut candidates, h, f, &format!("task:{}", e.id));
                }
            }
        }
        let brief_plan = options
            .briefs
            .then(|| self.record_plan(query, "brief"))
            .transpose()?;
        fault!(TASK_EVIDENCE_BEFORE_FINAL, Some(self), Some(control), "")?;
        let tx = self.db.begin_read()?;
        // All task state, graph dependencies, briefs, source bytes and memory
        // below belong to this final snapshot, after optional external routing.
        context.task_view = Some(self.task_view_in(&tx, scope)?);
        let sources = tx.open_table(SOURCES)?;
        let wid = self.workspace_id().ok_or(FoundryError::WorkspaceUnbound)?;
        let mut linked = 0;
        for e in &entries {
            for link in &e.source_links {
                if linked == 8 {
                    plan.discovery_limited = true;
                    continue;
                }
                linked += 1;
                let h = HandleRef::parse(link)?;
                let meta: Option<SourceMeta> = sources
                    .get(h.path.as_str())?
                    .map(|r| crate::store::decode(r.value(), "source"))
                    .transpose()?;
                if let Some(m) =
                    meta.filter(|m| m.hash.starts_with(&h.sha32) && h.ws16 == wid[..16])
                {
                    let f = if e.kind == Kind::Constraint {
                        "constraints"
                    } else {
                        facet(&h.path)
                    };
                    plan.discovery_limited |= !add(
                        &mut candidates,
                        SourceHandle {
                            workspace_id: wid.clone(),
                            path: h.path,
                            sha256: m.hash,
                            start: h.start,
                            end: h.end,
                        },
                        f,
                        &format!("task:{}", e.id),
                    );
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
