//! Lexical search (context-v2 § Two-tier query): the resolver and exact-case
//! tier collectors, and the `Engine` search, candidate and two-tier collection
//! methods with the record plans of memory and brief search. Query parsing,
//! the shared candidate types, prose helpers and doors stay in the parent
//! module.
use super::*;

/// One scored definition of an anchor (context-v2 § Resolver order).
#[derive(Clone, Copy, Debug)]
struct Scored {
    qualifiers: u64,
    exact: bool,
    role: u64,
    key_hash: u64,
    address: tantivy::DocAddress,
}

impl Scored {
    /// The resolver tuple, then `key_hash` ascending: the window's cut.
    fn key(&self) -> (Reverse<u64>, Reverse<bool>, u64, u64, tantivy::DocAddress) {
        (
            Reverse(self.qualifiers),
            Reverse(self.exact),
            self.role,
            self.key_hash,
            self.address,
        )
    }
}

/// What one anchor's resolver search found: every matching definition
/// counted, and the best [`TIER1_LIMIT`] by the resolver tuple then
/// `key_hash`.
#[derive(Default)]
struct ResolverFruit {
    definitions: u64,
    window: Vec<Scored>,
}

fn keep_window(window: &mut Vec<Scored>) {
    window.sort_unstable_by_key(Scored::key);
    window.truncate(TIER1_LIMIT);
}

/// Scores every definition matching one anchor's `def_name` term by the
/// resolver tuple (context-v2 § Resolver order): distinct query qualifiers
/// equal to an address segment (by hash), the exact-case name
/// (`name_case_hash`), then the role.
struct ResolverCollector {
    qualifiers: Vec<u64>,
    name_case: u64,
}

struct ResolverSegment {
    segment: tantivy::SegmentOrdinal,
    qualifiers: Vec<u64>,
    name_case: u64,
    key_hash: std::sync::Arc<dyn tantivy::columnar::ColumnValues<u64>>,
    role: std::sync::Arc<dyn tantivy::columnar::ColumnValues<u64>>,
    names: tantivy::columnar::Column<u64>,
    addresses: tantivy::columnar::Column<u64>,
    fruit: ResolverFruit,
}

impl tantivy::collector::Collector for ResolverCollector {
    type Fruit = ResolverFruit;
    type Child = ResolverSegment;

    fn for_segment(
        &self,
        segment: tantivy::SegmentOrdinal,
        reader: &SegmentReader,
    ) -> tantivy::Result<ResolverSegment> {
        let fast = reader.fast_fields();
        Ok(ResolverSegment {
            segment,
            qualifiers: self.qualifiers.clone(),
            name_case: self.name_case,
            key_hash: fast.u64("key_hash")?.first_or_default_col(0),
            role: fast.u64("role")?.first_or_default_col(0),
            names: fast.u64("name_case_hash")?,
            addresses: fast.u64("addr_hash")?,
            fruit: ResolverFruit::default(),
        })
    }

    fn requires_scoring(&self) -> bool {
        false
    }

    fn merge_fruits(&self, fruits: Vec<ResolverFruit>) -> tantivy::Result<ResolverFruit> {
        let mut merged = ResolverFruit::default();
        for fruit in fruits {
            merged.definitions += fruit.definitions;
            merged.window.extend(fruit.window);
        }
        keep_window(&mut merged.window);
        Ok(merged)
    }
}

impl tantivy::collector::SegmentCollector for ResolverSegment {
    type Fruit = ResolverFruit;

    fn collect(&mut self, doc: DocId, _score: Score) {
        let qualifiers = self
            .qualifiers
            .iter()
            .filter(|&&qualifier| {
                self.addresses
                    .values_for_doc(doc)
                    .any(|segment| segment == qualifier)
            })
            .count() as u64;
        let exact = self.names.first(doc) == Some(self.name_case);
        self.fruit.definitions += 1;
        self.fruit.window.push(Scored {
            qualifiers,
            exact,
            role: self.role.get_val(doc),
            key_hash: self.key_hash.get_val(doc),
            address: tantivy::DocAddress::new(self.segment, doc),
        });
        if self.fruit.window.len() >= 16 * TIER1_LIMIT {
            keep_window(&mut self.fruit.window);
        }
    }

    fn harvest(mut self) -> ResolverFruit {
        keep_window(&mut self.fruit.window);
        self.fruit
    }
}

/// The group-3 admission probe (context-v2 § Anchors and qualifiers):
/// whether a definition matching the anchor's `def_name` term carries its
/// exact-case `name_case_hash`. It scores, counts and keeps nothing; only an
/// admitted anchor's window is built.
struct ExactCaseProbe {
    name_case: u64,
}

struct ExactCaseSegment {
    name_case: u64,
    names: tantivy::columnar::Column<u64>,
    found: bool,
}

impl tantivy::collector::Collector for ExactCaseProbe {
    type Fruit = bool;
    type Child = ExactCaseSegment;

    fn for_segment(
        &self,
        _segment: tantivy::SegmentOrdinal,
        reader: &SegmentReader,
    ) -> tantivy::Result<ExactCaseSegment> {
        Ok(ExactCaseSegment {
            name_case: self.name_case,
            names: reader.fast_fields().u64("name_case_hash")?,
            found: false,
        })
    }

    fn requires_scoring(&self) -> bool {
        false
    }

    fn merge_fruits(&self, fruits: Vec<bool>) -> tantivy::Result<bool> {
        Ok(fruits.into_iter().any(|found| found))
    }
}

impl tantivy::collector::SegmentCollector for ExactCaseSegment {
    type Fruit = bool;

    fn collect(&mut self, doc: DocId, _score: Score) {
        if !self.found {
            self.found = self.names.first(doc) == Some(self.name_case);
        }
    }

    fn harvest(self) -> bool {
        self.found
    }
}

/// `query` under tier 1's own restriction (context-v2 § Two-tier query): no
/// memory document (008: both namespaces live in one index), and only the
/// `dir` subtree of the path filter when there is one.
fn tier_restricted(fields: &Fields, dir: Option<&str>, query: Box<dyn Query>) -> Box<dyn Query> {
    let mut clauses = vec![
        (Occur::Must, query),
        (
            Occur::MustNot,
            Box::new(TermQuery::new(
                Term::from_field_text(fields.kind, "memory"),
                IndexRecordOption::Basic,
            )) as Box<dyn Query>,
        ),
    ];
    clauses.push((
        Occur::MustNot,
        Box::new(TermQuery::new(
            Term::from_field_text(fields.kind, "brief"),
            IndexRecordOption::Basic,
        )),
    ));
    if let Some(dir) = dir {
        clauses.push((
            Occur::Must,
            Box::new(TermQuery::new(
                Term::from_field_text(fields.dir, dir),
                IndexRecordOption::Basic,
            )) as Box<dyn Query>,
        ));
    }
    Box::new(BooleanQuery::new(clauses))
}

/// Whether the index under `searcher` defines `name` exactly as written,
/// under tier 1's restriction: [`ExactCaseProbe`] over its `def_name` term.
fn exact_case_defined(
    searcher: &tantivy::Searcher,
    fields: &Fields,
    dir: Option<&str>,
    name: &str,
) -> FResult<bool> {
    let term = TermQuery::new(
        Term::from_field_text(fields.def_name, &name.to_lowercase()),
        IndexRecordOption::Basic,
    );
    Ok(searcher.search(
        tier_restricted(fields, dir, Box::new(term)).as_ref(),
        &ExactCaseProbe {
            name_case: hash64(name),
        },
    )?)
}

impl Engine {
    /// Search hits for context and path-less callers: [`Self::search_in`]
    /// without a `path` filter.
    pub fn search(&self, query: &str, limit: usize) -> FResult<SearchOutcome> {
        self.search_in(query, None, limit)
    }

    /// Search hits for the CLI and MCP: the materialized ranking of
    /// [`Self::search_candidates`], restricted by `path` to one file or
    /// directory subtree when given.
    pub fn search_in(
        &self,
        query: &str,
        path: Option<&str>,
        limit: usize,
    ) -> FResult<SearchOutcome> {
        Ok(Self::search_outcome(self.search_candidates(
            query,
            path,
            limit,
            &crate::Control::unbounded(),
        )?))
    }

    /// The materialized hits of one candidate batch, as the CLI and MCP
    /// render them: locator lines never carry a selection tag, so a
    /// dense-only hit (009 T004: a placed card's unit with no lexical
    /// candidate) names its unit's handle under the `semantic` label.
    pub fn search_outcome(batch: CandidateBatch) -> SearchOutcome {
        let semantic = batch.semantic;
        let hits = batch
            .items
            .into_iter()
            .filter_map(|item| {
                let dense_only = item.is_dense_only();
                let handle = item.handle?;
                let text = item.forms.into_iter().find_map(|form| match form {
                    RenderedForm::Verbatim(text) => Some(text),
                    _ => None,
                })?;
                let label = if dense_only {
                    "semantic".to_owned()
                } else {
                    item.label
                };
                Some(Hit {
                    path: handle.path.clone(),
                    start_line: item.start_line,
                    end_line: item.end_line,
                    handle,
                    text,
                    label,
                    tier: item.tier,
                    line: item.line,
                })
            })
            .collect();
        let counters = batch.counters;
        let freshness = batch.freshness;
        SearchOutcome {
            workspace_id: freshness.workspace_id,
            source_revision: freshness.source_revision,
            hits,
            pending_sources: freshness.pending_sources,
            stale_candidates: counters.stale,
            capped: counters.capped,
            candidate_limit: CANDIDATE_LIMIT,
            candidate_limit_reached: counters.candidates_full,
            truncated: counters.truncated || counters.candidates_full,
            scan_state: freshness.scan_state,
            semantic,
        }
    }

    /// Candidate source paths across the admitted search index. Only the first
    /// document of each source has these exact syntax-name postings, so comments
    /// and repeated calls within a file cannot drown out other files. This reads
    /// at most nine stored index documents and no source bodies. Final source
    /// hash/AST validation belongs to the task's final read snapshot.
    pub(crate) fn task_call_sources(
        &self,
        names: &[String],
        control: &crate::Control,
    ) -> FResult<(Vec<SourceHandle>, bool)> {
        control.check()?;
        if names.is_empty() {
            return Ok((Vec::new(), false));
        }
        let handles = self.require_search()?;
        let fields = &handles.fields;
        let searcher = handles.reader.searcher();
        let clauses: Vec<Box<dyn Query>> = names
            .iter()
            .take(4)
            .map(|name| {
                Box::new(TermQuery::new(
                    Term::from_field_text(fields.call_name, name),
                    IndexRecordOption::Basic,
                )) as Box<dyn Query>
            })
            .collect();
        let window = searcher.search(
            &BooleanQuery::union(clauses),
            &TopDocs::with_limit(9).order_by_u64_field("key_hash", tantivy::Order::Asc),
        )?;
        control.check()?;
        let mut limited = names.len() > 4 || window.len() > 8;
        let workspace_id = self.workspace_id().ok_or(FoundryError::WorkspaceUnbound)?;
        let mut sources = Vec::new();
        for (_, address) in window.into_iter().take(8) {
            control.check()?;
            let doc: TantivyDocument = searcher.doc(address)?;
            let invalid =
                || FoundryError::CorruptStore("invalid call-source search document".into());
            let text = |field| doc.get_first(field).and_then(|value| value.as_str());
            let path = text(fields.path).ok_or_else(invalid)?;
            let hash = text(fields.hash).ok_or_else(invalid)?;
            limited |= text(fields.call_coverage) != Some("complete");
            sources.push(SourceHandle {
                workspace_id: workspace_id.clone(),
                path: path.to_owned(),
                sha256: hash.to_owned(),
                // This is an index identity receipt, never a requested body span.
                start: 0,
                end: 0,
            });
        }
        Ok((sources, limited))
    }

    /// Whether this store defines `name` exactly as written, under tier 1's
    /// restriction (`path` as the search's path filter): the group-3
    /// admission probe of context-v2 § Anchors and qualifiers, which a
    /// multi-root owner asks of every serving root before any root builds a
    /// window (007). It counts and keeps nothing.
    pub fn defines_exact_case(&self, name: &str, path: Option<&str>) -> FResult<bool> {
        let filter = path.map(path_filter).transpose()?;
        let handles = self.require_search()?;
        exact_case_defined(
            &handles.reader.searcher(),
            &handles.fields,
            filter.as_deref(),
            name,
        )
    }

    /// [`Self::search_candidates_with`], the anchors chosen from this
    /// store's own index.
    pub fn search_candidates(
        &self,
        query: &str,
        path: Option<&str>,
        limit: usize,
        control: &crate::Control,
    ) -> FResult<CandidateBatch> {
        self.search_candidates_with(query, path, limit, control, None)
    }

    /// Two-tier candidate selection for one store (context-v2 § Two-tier
    /// query, § Resolver order, § Hit materialization). Tier 1 is the
    /// anchors' resolver windows in anchor order or, without anchors, exact
    /// definitions of the tier-1 runs (at most 64, the most specific run
    /// first, smallest `key_hash` kept per run, ordered by path and start
    /// within it); tier 2 is lexical (at most 256 by score, `key_hash`
    /// breaking cutoff ties). `path` restricts both tiers to a file or
    /// directory subtree. Candidates are revalidated in one final read
    /// transaction, merged per delivery unit, capped at 4 per file and cut to
    /// `limit`; each anchor window's first [`ANCHOR_LIST`] definitions are
    /// materialized beside them, without that cap or cut. `anchors` are the
    /// ones a multi-root owner chose over every serving root (007); `None`
    /// chooses them from this store's index.
    pub fn search_candidates_with(
        &self,
        query: &str,
        path: Option<&str>,
        limit: usize,
        control: &crate::Control,
        anchors: Option<&[AnchorCandidate]>,
    ) -> FResult<CandidateBatch> {
        self.search_candidates_selected(
            query,
            path,
            limit,
            control,
            SearchSelection {
                anchors,
                prose: false,
            },
        )
    }

    pub(super) fn search_candidates_selected(
        &self,
        query: &str,
        path: Option<&str>,
        limit: usize,
        control: &crate::Control,
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
        // requirements (error precedence the extraction must not change).
        path.map(path_filter).transpose()?;
        let workspace_id = self.require_workspace_id()?;
        let TwoTier {
            prose,
            first,
            second,
            candidates_full,
            windows,
        } = self.collect_two_tier(query, path, control, selection)?;

        // Final read: revalidate, merge per delivery unit and cap per file over
        // the whole candidate window, then cut to `limit`; stale and capped
        // skips are counted past the cut.
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let mut counters = CandidateCounters {
            candidates_full,
            ..CandidateCounters::default()
        };
        let mut current: std::collections::BTreeMap<String, Option<SourceMeta>> =
            std::collections::BTreeMap::new();
        let mut seen = LexicalSelection::default();
        let mut per_file: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        let mut kept: Vec<&Candidate> = Vec::new();
        for (rank, candidate) in first.iter().chain(&second).enumerate() {
            if !current.contains_key(&candidate.path) {
                let meta = match sources.get(candidate.path.as_str())? {
                    Some(raw) => Some(decode::<SourceMeta>(raw.value(), "source")?),
                    None => None,
                };
                current.insert(candidate.path.clone(), meta);
            }
            if current[&candidate.path]
                .as_ref()
                .is_none_or(|meta| meta.hash != candidate.hash)
            {
                counters.stale += 1;
                candidate.trace(control, &workspace_id, rank, "stale");
                continue;
            }
            if let Err(outcome) = seen.insert(candidate, prose) {
                candidate.trace(control, &workspace_id, rank, outcome);
                continue;
            }
            let count = per_file.entry(candidate.path.clone()).or_default();
            if *count == PER_FILE_CAP {
                counters.capped += 1;
                candidate.trace(control, &workspace_id, rank, "per_file_cap");
                continue;
            }
            *count += 1;
            if kept.len() == limit {
                counters.truncated = true;
                candidate.trace(control, &workspace_id, rank, "limit");
                continue;
            }
            candidate.trace(control, &workspace_id, rank, "kept");
            kept.push(candidate);
        }
        let wanted: std::collections::BTreeSet<String> = if prose {
            prose_terms(query)
        } else {
            analyzed_terms(crate::syntax::code_subtokens, query)
        }
        .into_iter()
        .collect();
        let mut verified: std::collections::BTreeMap<String, VerifiedSource> =
            std::collections::BTreeMap::new();
        let mut items = Vec::with_capacity(kept.len());
        let mut delivered = std::collections::BTreeSet::new();
        for (rank, candidate) in kept.into_iter().enumerate() {
            let Some(Some(meta)) = current.get(&candidate.path) else {
                continue;
            };
            if !verified.contains_key(&candidate.path) {
                let source = reconstruct_verified(&stored, &candidate.path, meta)?;
                verified.insert(candidate.path.clone(), source);
            }
            let body = &verified[&candidate.path].body;
            let item = ranked_context_item(
                candidate,
                rank,
                &workspace_id,
                &meta.hash,
                body,
                &wanted,
                prose,
            )?;
            candidate.trace_materialization(control, &item);
            let handle = item.handle.as_ref().expect("source candidate");
            if delivered.insert((handle.path.clone(), handle.start, handle.end)) {
                items.push(item);
            }
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
        let freshness = self.freshness_in(&tx)?;
        Ok(CandidateBatch {
            freshness,
            items,
            counters,
            semantic: None,
            route: None,
            anchors,
            doors: None,
            collected: None,
        })
    }

    /// The two-tier candidate collection shared by the baseline and the
    /// semantic paths (context-v2 § Two-tier query, § Anchors and
    /// qualifiers, § Resolver order): tier 1 is the anchors' windows in
    /// anchor order (each the best 64 of every matching definition by the
    /// resolver tuple, then `key_hash`, listed by the tuple, then path and
    /// start) or, for a query without anchors, the exact definitions of its
    /// tier-1 runs (at most 64; runs by specificity, each contributing its
    /// smallest-`key_hash` definitions to the slots left, ordered by path and
    /// start within the run); tier 2 is lexical (at most 256 by score,
    /// `key_hash` breaking cutoff ties, tier-1 units removed). The anchors are
    /// `anchors` when a multi-root owner chose them over every serving root
    /// (007), otherwise this index's own choice: each capitalized candidate
    /// costs one exact-case probe, and only admitted anchors get windows.
    /// Also returns whether a window filled and each anchor's window range.
    /// No source is read here; validation happens in the callers' final read.
    pub(super) fn collect_two_tier(
        &self,
        query: &str,
        path: Option<&str>,
        control: &crate::Control,
        selection: SearchSelection<'_>,
    ) -> FResult<TwoTier> {
        let filter = path.map(path_filter).transpose()?;
        let handles = self.require_search()?;
        let fields = &handles.fields;
        let restrict = |query: Box<dyn Query>| tier_restricted(fields, filter.as_deref(), query);
        let term = |field: Field, text: &str, option: IndexRecordOption| -> Box<dyn Query> {
            Box::new(TermQuery::new(Term::from_field_text(field, text), option))
        };
        let searcher = handles.reader.searcher();
        // Tier 1 groups in order, each its documents (with their resolver
        // tuples in an anchor's window) and, for a window, its anchor.
        struct Tier1Group {
            anchor: Option<(String, (u8, usize), u64)>,
            documents: Vec<(tantivy::DocAddress, Option<Resolver>)>,
        }
        let mut tier1: Vec<Tier1Group> = Vec::new();
        let mut tier1_full = false;
        let parsed = QueryAnchors::parse(query);
        let qualifiers: Vec<u64> = parsed.qualifiers.iter().map(|q| hash64(q)).collect();
        let mut anchors = match selection.anchors {
            Some(chosen) => chosen.to_vec(),
            None => parsed.select(|candidate| {
                control.check()?;
                exact_case_defined(&searcher, fields, filter.as_deref(), &candidate.text)
            })?,
        };
        if selection.prose && prose_query(query) {
            anchors.retain(|anchor| !incidental_prose_anchor(query, anchor));
        }
        let prose = selection.prose && anchors.is_empty() && prose_query(query);
        if prose && let Some(trace) = control.trace() {
            trace.record(serde_json::json!({"stage":"lexical_intent", "intent":"anchorless_prose", "exact_run_promotion":false, "scoring":"distinct_terms_proximity_v1", "term_limit":64}));
        }
        for anchor in anchors {
            // The anchor's window (context-v2 § Resolver order): every
            // matching definition scored under tier 1's own restriction.
            control.check()?;
            let fruit = searcher.search(
                restrict(term(
                    fields.def_name,
                    &anchor.text.to_lowercase(),
                    IndexRecordOption::Basic,
                ))
                .as_ref(),
                &ResolverCollector {
                    qualifiers: qualifiers.clone(),
                    name_case: hash64(&anchor.text),
                },
            )?;
            tier1_full |= fruit.definitions > TIER1_LIMIT as u64;
            let order = (anchor.group, anchor.position);
            let documents = fruit
                .window
                .iter()
                .map(|scored| {
                    // The name range is read with the document below.
                    let resolver = Resolver {
                        anchor: order,
                        qualifiers: scored.qualifiers,
                        exact: scored.exact,
                        role: scored.role,
                        name: (0, 0),
                    };
                    (scored.address, Some(resolver))
                })
                .collect();
            tier1.push(Tier1Group {
                anchor: Some((anchor.text, order, fruit.definitions)),
                documents,
            });
        }
        if tier1.is_empty() && !prose {
            // Without anchors the 2026-10-06 rule stands: one exact count and
            // the 64 smallest-`key_hash` definitions per run, under the same
            // restriction as the documents it keeps.
            let definitions = (
                Count,
                TopDocs::with_limit(TIER1_LIMIT).tweak_score(|reader: &SegmentReader| {
                    let key_hash = reader
                        .fast_fields()
                        .u64("key_hash")
                        .expect("schema v2 fast field")
                        .first_or_default_col(0);
                    move |doc: DocId, _score: Score| Reverse(key_hash.get_val(doc))
                }),
            );
            let mut runs: Vec<(usize, String, Vec<tantivy::DocAddress>)> = Vec::new();
            for run in tier1_runs(query, &code_spans(query)) {
                let (count, top) = searcher.search(
                    restrict(term(fields.def_name, &run, IndexRecordOption::Basic)).as_ref(),
                    &definitions,
                )?;
                if count > 0 {
                    let top = top.into_iter().map(|(_, address)| address).collect();
                    runs.push((count, run, top));
                }
            }
            // Specificity: fewer definitions first, then run text. Each run
            // fills the slots left; a document already kept stays under its
            // earlier run.
            runs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            let defined: usize = runs.iter().map(|(count, ..)| count).sum();
            let mut kept: Vec<tantivy::DocAddress> = Vec::new();
            for (_, _, top) in runs {
                let room = TIER1_LIMIT - kept.len();
                if room == 0 {
                    break;
                }
                let group: Vec<tantivy::DocAddress> = top
                    .into_iter()
                    .filter(|address| !kept.contains(address))
                    .take(room)
                    .collect();
                kept.extend(&group);
                tier1.push(Tier1Group {
                    anchor: None,
                    documents: group.into_iter().map(|address| (address, None)).collect(),
                });
            }
            tier1_full = kept.len() == TIER1_LIMIT && defined > TIER1_LIMIT;
        }
        let mut clauses: Vec<Box<dyn Query>> = Vec::new();
        for part in query.split_whitespace() {
            if prose && prose_stopword(part.trim_end_matches(['?', '.', ',', ':'])) {
                continue;
            }
            let terms = analyzed_terms(crate::syntax::code_subtokens, part);
            match terms.as_slice() {
                [] => {}
                [one] => clauses.push(term(fields.body, one, IndexRecordOption::WithFreqs)),
                many => clauses.push(Box::new(PhraseQuery::new(
                    many.iter()
                        .map(|subtoken| Term::from_field_text(fields.body, subtoken))
                        .collect(),
                ))),
            }
        }
        let mut idents = analyzed_terms(crate::syntax::identifier_runs, query);
        idents.sort();
        idents.dedup();
        for ident in &idents {
            if prose && prose_stopword(ident) {
                continue;
            }
            clauses.push(Box::new(BoostQuery::new(
                term(fields.ident, ident, IndexRecordOption::WithFreqs),
                3.0,
            )));
        }
        clauses.push(Box::new(BoostQuery::new(
            term(fields.path, query.trim(), IndexRecordOption::Basic),
            100.0,
        )));
        let per_lane = if prose {
            CANDIDATE_LIMIT / 2
        } else {
            CANDIDATE_LIMIT
        };
        let collector = TopDocs::with_limit(per_lane).tweak_score(|reader: &SegmentReader| {
            let key_hash = reader
                .fast_fields()
                .u64("key_hash")
                .expect("schema v2 fast field")
                .first_or_default_col(0);
            move |doc: DocId, score: Score| (score, Reverse(key_hash.get_val(doc)))
        });
        let lexical: Box<dyn Query> = if prose {
            let lexical = prose_lexical(query, fields.body);
            if let Some(language) = prose_language(query) {
                if let Some(trace) = control.trace() {
                    trace.record(serde_json::json!({"stage":"lexical_language","language":language,"policy":"prefer_explicit_language_v1","filters_roots":false}));
                }
                // Distinct-term scores are bounded by 64.5. Prefer matching
                // language before spending the fixed discovery window; adding
                // this after TopDocs would leave relevant files undiscovered.
                // The lexical Must keeps unrelated files out; Should preserves
                // fallback languages and the separate documentation window.
                Box::new(BooleanQuery::new(vec![
                    (Occur::Must, lexical),
                    (
                        Occur::Should,
                        Box::new(ConstScoreQuery::new(
                            term(fields.lang, language, IndexRecordOption::Basic),
                            65.0,
                        )),
                    ),
                ]))
            } else {
                lexical
            }
        } else {
            Box::new(BooleanQuery::union(clauses))
        };
        let mut full = false;
        let mut tier2 = Vec::new();
        let mut collect = |query: Box<dyn Query>| -> FResult<()> {
            let found = searcher.search(restrict(query).as_ref(), &collector)?;
            full |= found.len() == per_lane;
            tier2.extend(
                found
                    .into_iter()
                    .map(|((score, _), address)| (score, address)),
            );
            Ok(())
        };
        if prose {
            // Disjoint windows keep large documentation sets from exhausting
            // code discovery, and preserve documentation discovery in code-heavy
            // roots. Their combined cap is still 256 documents.
            let code: Box<dyn Query> = Box::new(BooleanQuery::new(vec![
                (Occur::Must, Box::new(tantivy::query::AllQuery)),
                (
                    Occur::MustNot,
                    term(fields.kind, "section", IndexRecordOption::Basic),
                ),
                (
                    Occur::MustNot,
                    term(fields.kind, "block", IndexRecordOption::Basic),
                ),
                (
                    Occur::MustNot,
                    term(fields.kind, UNPARSED_KIND, IndexRecordOption::Basic),
                ),
            ]));
            collect(Box::new(BooleanQuery::new(vec![
                (Occur::Must, lexical.box_clone()),
                (Occur::Must, code.box_clone()),
            ])))?;
            collect(Box::new(BooleanQuery::new(vec![
                (Occur::Must, lexical),
                (Occur::MustNot, code),
            ])))?;
        } else {
            collect(lexical)?;
        }
        let candidates_full = tier1_full || full;
        control.check()?;

        // A resolver-window document is a definition document: it carries
        // the stored name range its resolver records.
        let read =
            |tier: u8, score: Score, address, resolver: Option<Resolver>| -> FResult<Candidate> {
                let doc: TantivyDocument = searcher.doc(address)?;
                let invalid = || FoundryError::CorruptStore("invalid search document".into());
                let text = |field| {
                    doc.get_first(field)
                        .and_then(|v| v.as_str())
                        .map(str::to_owned)
                };
                let number = |field| {
                    doc.get_first(field)
                        .and_then(|v| v.as_u64())
                        .ok_or_else(invalid)
                };
                let resolver = match resolver {
                    Some(resolver) => Some(Resolver {
                        name: (number(fields.name_start)?, number(fields.name_end)?),
                        ..resolver
                    }),
                    None => None,
                };
                Ok(Candidate {
                    tier,
                    score,
                    path: text(fields.path).ok_or_else(invalid)?,
                    hash: text(fields.hash).ok_or_else(invalid)?,
                    start: number(fields.start)?,
                    end: number(fields.end)?,
                    unit_start: number(fields.unit_start)?,
                    unit_head: number(fields.unit_head)?,
                    unit_end: number(fields.unit_end)?,
                    // A parse-fallback document is a block to every reader.
                    kind: text(fields.kind)
                        .map(|kind| match kind.as_str() {
                            UNPARSED_KIND => crate::syntax::UnitKind::Block.as_str().to_owned(),
                            _ => kind,
                        })
                        .ok_or_else(invalid)?,
                    lang: text(fields.lang),
                    qname: text(fields.qname),
                    resolver,
                })
            };
        // Tier 1: each anchor's window by the resolver tuple, then path and
        // start (today's tier-1 listing within a run); without anchors each
        // run's group by path and start.
        let mut first: Vec<Candidate> = Vec::new();
        let mut windows: Vec<WindowSlice> = Vec::new();
        for group in tier1 {
            let mut run: Vec<Candidate> = group
                .documents
                .into_iter()
                .map(|(address, resolver)| read(1, 0.0, address, resolver))
                .collect::<FResult<_>>()?;
            run.sort_by(|a, b| {
                let key = |candidate: &Candidate| candidate.resolver.map(|r| r.key());
                key(a)
                    .cmp(&key(b))
                    .then_with(|| a.path.cmp(&b.path))
                    .then(a.start.cmp(&b.start))
            });
            let range = first.len()..first.len() + run.len();
            first.extend(run);
            if let Some((anchor, order, definitions)) = group.anchor {
                windows.push(WindowSlice {
                    anchor,
                    order,
                    definitions,
                    range,
                });
            }
        }
        let units: std::collections::BTreeSet<_> = first.iter().map(Candidate::unit).collect();
        let mut second: Vec<Candidate> = Vec::new();
        for (score, address) in tier2 {
            let candidate = read(2, score, address, None)?;
            if !units.contains(&candidate.unit()) {
                second.push(candidate);
            }
        }
        second.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.start.cmp(&b.start))
        });
        if prose {
            let (code, prose): (Vec<_>, Vec<_>) = second
                .into_iter()
                .partition(|c| !["section", "block", UNPARSED_KIND].contains(&c.kind.as_str()));
            second = Vec::with_capacity(code.len() + prose.len());
            let mut code = code.into_iter();
            let mut prose = prose.into_iter();
            loop {
                let (a, b) = (code.next(), prose.next());
                if a.is_none() && b.is_none() {
                    break;
                }
                second.extend(a);
                second.extend(b);
            }
        }
        Ok(TwoTier {
            prose,
            first,
            second,
            candidates_full,
            windows,
        })
    }

    /// Derived memory documents matching `query` (008 memory search): the
    /// tier-2 lexical clauses over `body`/`ident`, restricted to
    /// `kind:"memory"`, at most `limit` by (score, key_hash). Callers
    /// validate each candidate against the live row before delivery.
    pub(crate) fn memory_plan(&self, query: &str) -> FResult<RecordPlan> {
        self.record_plan(query, "memory")
    }

    pub(crate) fn record_plan(&self, query: &str, kind: &str) -> FResult<RecordPlan> {
        if query.trim().is_empty() || query.len() > 4096 {
            return Err(FoundryError::InvalidArgument(
                "query must contain 1..4096 nonblank bytes".into(),
            ));
        }
        // The whole 256-document window is examined; whether it filled is
        // reported as `candidates:full` (context-v2 § Header line).
        let limit = MEMORY_CANDIDATE_WINDOW;
        let handles = self.require_search()?;
        let fields = &handles.fields;
        let mut clauses: Vec<Box<dyn Query>> = Vec::new();
        for part in query.split_whitespace() {
            let terms = analyzed_terms(crate::syntax::code_subtokens, part);
            match terms.as_slice() {
                [] => {}
                [one] => clauses.push(Box::new(TermQuery::new(
                    Term::from_field_text(fields.body, one),
                    IndexRecordOption::WithFreqs,
                ))),
                many => clauses.push(Box::new(PhraseQuery::new(
                    many.iter()
                        .map(|subtoken| Term::from_field_text(fields.body, subtoken))
                        .collect(),
                ))),
            }
        }
        let mut idents = analyzed_terms(crate::syntax::identifier_runs, query);
        idents.sort();
        idents.dedup();
        for ident in &idents {
            clauses.push(Box::new(BoostQuery::new(
                Box::new(TermQuery::new(
                    Term::from_field_text(fields.ident, ident),
                    IndexRecordOption::WithFreqs,
                )),
                3.0,
            )));
        }
        let top = BooleanQuery::new(vec![
            (
                Occur::Must,
                Box::new(TermQuery::new(
                    Term::from_field_text(fields.kind, kind),
                    IndexRecordOption::Basic,
                )) as Box<dyn Query>,
            ),
            (
                Occur::Must,
                Box::new(BooleanQuery::union(clauses)) as Box<dyn Query>,
            ),
        ]);
        let collector = TopDocs::with_limit(limit).tweak_score(|reader: &SegmentReader| {
            let key_hash = reader
                .fast_fields()
                .u64("key_hash")
                .expect("schema v2 fast field")
                .first_or_default_col(0);
            move |doc: DocId, score: Score| (score, Reverse(key_hash.get_val(doc)))
        });
        let searcher = handles.reader.searcher();
        let mut out = Vec::new();
        for (_, address) in searcher.search(&top, &collector)? {
            let doc: TantivyDocument = searcher.doc(address)?;
            let key = doc
                .get_first(fields.key)
                .and_then(|v| v.as_str())
                .ok_or_else(|| FoundryError::CorruptStore("invalid record document".into()))?;
            let Some(id) = key.strip_prefix(&format!("{kind}:")) else {
                return Err(FoundryError::CorruptStore(format!(
                    "{kind} query matched another kind of document {key:?}"
                )));
            };
            let revision = doc
                .get_first(fields.hash)
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<u64>().ok())
                .ok_or_else(|| {
                    FoundryError::CorruptStore(format!("{kind} document {key:?} has no revision"))
                })?;
            out.push(RecordCandidate {
                id: id.to_owned(),
                revision,
            });
        }
        Ok(RecordPlan {
            window_full: out.len() >= MEMORY_CANDIDATE_WINDOW,
            candidates: out,
        })
    }
}
