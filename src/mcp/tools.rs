//! The MCP tool layer: the `FoundryMcp` router and its tools, delivery
//! economics and multi-root fan-out, and the `ServerHandler`. Owner state,
//! admission, transports and startup stay in the parent module.
use super::*;

/// Build the header segments for the roots a response lists (007): facts for
/// the roots that served, coverage for the others.
fn root_headers(meta: &[RootMeta], listed: &[usize], facts: &RootFacts) -> Vec<RootHeader> {
    listed
        .iter()
        .map(|&index| {
            let root = &meta[index];
            match facts.iter().find(|(served, ..)| *served == index) {
                Some((_, revision, scan_state, pending)) => RootHeader {
                    alias: root.alias.clone(),
                    label: root.label.clone(),
                    serving: Some((*revision, scan_state.clone(), *pending)),
                    coverage: None,
                },
                None => RootHeader {
                    alias: root.alias.clone(),
                    label: root.label.clone(),
                    serving: None,
                    coverage: Some(root.coverage.as_str().to_owned()),
                },
            }
        })
        .collect()
}

/// `(meta index, revision, scan state, pending)` of every root that has an
/// open engine, read live for a retrieve response's header.
fn root_facts(engines: &[Option<Engine>]) -> FResult<RootFacts> {
    let mut facts = Vec::new();
    for (index, engine) in engines.iter().enumerate() {
        let Some(engine) = engine else { continue };
        let status = engine.status()?;
        facts.push((
            index,
            status.source_revision,
            status.scan_state,
            // 008: the per-root header segment counts source work only.
            engine.pending_source_work()?,
        ));
    }
    Ok(facts)
}

/// Run one candidate selection per selected serving root, sequentially inside
/// the shared read deadline (007 § Combined search and context). The query's
/// anchors are chosen first, once over every serving root
/// ([`roots::select_anchors`], `path` as the search's path filter), and every
/// root's selection receives them. Then a cooperative deadline/cancellation
/// check and the `roots.before_root` test-faults point precede each root's
/// call, so a stall in one root holds the single engine slot until it
/// returns and the deadline fails the whole request. Returns each root's
/// batch with its own final-read facts.
fn collect_root_batches<F>(
    engines: &[Option<Engine>],
    control: &Control,
    meta: &[RootMeta],
    serving: &[usize],
    query: &str,
    path: Option<&str>,
    mut select: F,
) -> FResult<(Vec<roots::RootBatch>, RootFacts)>
where
    // FnMut: the 008 multi-root path lets the primary's call stash its
    // validated memory hits beside the batch it returns.
    F: FnMut(
        &Engine,
        &Control,
        &[crate::store::AnchorCandidate],
    ) -> FResult<crate::store::CandidateBatch>,
{
    let serving_engines: Vec<&Engine> = serving
        .iter()
        .map(|&index| {
            engines[index]
                .as_ref()
                .expect("a serving root holds an engine")
        })
        .collect();
    let anchors = roots::select_anchors(&serving_engines, query, path, control)?;
    let mut batches = Vec::new();
    let mut facts = Vec::new();
    for (&index, engine) in serving.iter().zip(serving_engines) {
        control.check()?;
        fault!(
            ROOTS_BEFORE_ROOT,
            Some(engine),
            Some(control),
            &meta[index].alias
        )?;
        let batch = select(engine, control, &anchors)?;
        facts.push((
            index,
            batch.freshness.source_revision,
            batch.freshness.scan_state.clone(),
            batch.freshness.pending_sources,
        ));
        batches.push(roots::RootBatch {
            alias: meta[index].alias.clone(),
            batch,
        });
    }
    Ok((batches, facts))
}

// ---------------------------------------------------------------------------
// The server: exactly five tools
// ---------------------------------------------------------------------------

pub struct FoundryMcp {
    tool_router: ToolRouter<Self>,
    state: Arc<Shared>,
}

impl Clone for FoundryMcp {
    fn clone(&self) -> Self {
        Self {
            tool_router: self.tool_router.clone(),
            state: Arc::clone(&self.state),
        }
    }
}

#[tool_router]
impl FoundryMcp {
    pub(crate) fn new(state: Arc<Shared>) -> Self {
        let mut tool_router = Self::tool_router();
        if !state.experimental_task_context {
            tool_router.remove_route("task_context");
        }
        if state.experimental_task_context {
            let route = tool_router
                .map
                .get_mut("context")
                .expect("context route exists");
            let schema = Arc::make_mut(&mut route.attr.input_schema);
            let properties = schema
                .get_mut("properties")
                .and_then(serde_json::Value::as_object_mut)
                .expect("context properties");
            properties.insert("task_id".into(), serde_json::json!({"type":"string","minLength":1,"maxLength":128,"description":"Primary task ID; requires task_revision."}));
            properties.insert("compose_evidence".into(), serde_json::json!({"type":"boolean","description":"Compose bounded evidence across selected admitted roots using the primary task; requires task_id and task_revision."}));
            properties.insert("compact_evidence".into(), serde_json::json!({"type":"boolean","description":"Compact evidence accounting; requires compose_evidence. Source citations and task constraints are retained."}));
            properties.insert("boundary_config".into(), serde_json::json!({"type":"string","description":"Indexed boundary configuration path; requires compose_evidence."}));
            properties.insert("task_revision".into(), serde_json::json!({"type":"integer","minimum":1,"description":"Expected task revision; requires task_id."}));
        }
        if state.experimental_task_context {
            let route = tool_router
                .map
                .get_mut("index")
                .expect("index route exists");
            let schema = Arc::make_mut(&mut route.attr.input_schema);
            schema.get_mut("properties").and_then(serde_json::Value::as_object_mut).expect("index properties").insert("boundary".into(), serde_json::json!({"type":"object","additionalProperties":false,"required":["config"],"properties":{"maintain":{"type":"boolean","description":"Enable or pause one boundary configuration on the existing --watch owner."},"config":{"type":"string","maxLength":4096},"paths":{"type":"array","maxItems":64,"items":{"type":"string","maxLength":4096}}}}));
        }
        if state.knowledge.is_some() {
            for (tool, key, field) in [
                (
                    "index",
                    "knowledge",
                    serde_json::json!({"enum":["prepare","maintain","pause"]}),
                ),
                (
                    "context",
                    "include_briefs",
                    serde_json::json!({"type":"boolean","description":"Include derived primary-root knowledge with exact source evidence."}),
                ),
            ] {
                let route = tool_router.map.get_mut(tool).expect("tool exists");
                if tool == "index" {
                    route.attr.description = Some("Index an admitted root or control configured semantic/knowledge maintenance.".into());
                    if let Some(annotations) = &mut route.attr.annotations {
                        annotations.open_world_hint = Some(true);
                        annotations.idempotent_hint = Some(false);
                    }
                }
                let schema = Arc::make_mut(&mut route.attr.input_schema);
                schema
                    .get_mut("properties")
                    .and_then(serde_json::Value::as_object_mut)
                    .expect("properties")
                    .insert(key.into(), field);
            }
        }
        if state.no_memory {
            tool_router.remove_route("memory");
        }
        Self { tool_router, state }
    }

    /// stdio carries the guard directly in the request extensions; HTTP
    /// carries it inside the propagated `http::request::Parts` extensions.
    fn admission_guard(ctx: &RequestContext<RoleServer>) -> Option<Arc<AdmissionGuard>> {
        ctx.extensions
            .get::<Arc<AdmissionGuard>>()
            .or_else(|| {
                ctx.extensions
                    .get::<http::request::Parts>()
                    .and_then(|parts| parts.extensions.get::<Arc<AdmissionGuard>>())
            })
            .cloned()
    }

    #[tool(
        name = "search",
        description = "Use BEFORE grep/rg for indexed code: one line per hit with handle, line, symbol and match. Follow handles with retrieve. Indexed snapshot, not live disk.",
        input_schema = schema(r#"{"type":"object","additionalProperties":false,"required":["query"],"properties":{"query":{"type":"string","minLength":1,"maxLength":4096},"limit":{"type":"integer","minimum":1,"maximum":64,"default":10},"tokens":{"type":"integer","minimum":1,"maximum":32768,"default":1024},"path":{"type":"string","minLength":1},"roots":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":9,"uniqueItems":true}}}"#),
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn search(
        &self,
        ctx: RequestContext<RoleServer>,
        arguments: JsonObject,
    ) -> Result<CallToolResult, ErrorData> {
        if let Err(e) = unknown_fields(&arguments, &["query", "limit", "tokens", "path", "roots"]) {
            return Ok(foundry_error_result(&e));
        }
        let query = match required_str(&arguments, "query") {
            Ok(query) => query,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        if query.trim().is_empty() || query.len() > 4096 {
            return Ok(error_result(
                "invalid_argument",
                "query must contain 1..4096 bytes and be nonblank",
                false,
            ));
        }
        let limit = match optional_u64(&arguments, "limit", 10, 1, 64) {
            Ok(limit) => limit as usize,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        let tokens = match optional_u64(&arguments, "tokens", 1024, 1, 32768) {
            Ok(tokens) => tokens,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        // Normalized and validated before engine admission.
        let path = match optional_str(&arguments, "path") {
            Ok(path) => match path.map(crate::store::path_filter).transpose() {
                Ok(path) => path,
                Err(e) => return Ok(foundry_error_result(&e)),
            },
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        // `roots` selects already admitted aliases, validated before dispatch.
        let roots = match roots_argument(&arguments) {
            Ok(roots) => roots,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        // Validated in BOTH owner modes: an unknown alias never silently
        // falls back to the primary on a no-reference owner.
        if let Err(e) = self.state.validate_aliases(roots.as_deref()) {
            return Ok(foundry_error_result(&e));
        }
        let query = query.to_owned();
        if self.state.meta.len() > 1 {
            return Ok(self
                .search_roots(&ctx, tokens, query, path, limit, roots)
                .await);
        }
        let semantic = self.state.semantic.clone();
        Ok(self
            .deliver(
                &ctx,
                tokens,
                "search",
                || response::refusal_floor("search", None),
                move |engines, control, _budget| {
                    let engine = engines[0].as_ref().expect("the primary engine is open");
                    if !semantic_on(&semantic) {
                        return engine.search_in(&query, path.as_deref(), limit);
                    }
                    Ok(Engine::search_outcome(search_primary(
                        &semantic,
                        engine,
                        &query,
                        path.as_deref(),
                        limit,
                        control,
                        None,
                    )?))
                },
                response::pack_search,
            )
            .await)
    }

    #[tool(
        name = "context",
        description = "Use INSTEAD of exploratory file reads: budgeted, cited symbols (verbatim or signatures when large), graph edges and outlines.",
        input_schema = schema(r#"{"type":"object","additionalProperties":false,"required":["query"],"properties":{"query":{"type":"string","minLength":1,"maxLength":4096},"tokens":{"type":"integer","minimum":1,"maximum":32768,"default":2048},"strategy":{"type":"string","enum":["auto","search","graph"],"default":"auto"},"roots":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":9,"uniqueItems":true},"include_memory":{"type":"boolean"}}}"#),
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn context(
        &self,
        ctx: RequestContext<RoleServer>,
        arguments: JsonObject,
    ) -> Result<CallToolResult, ErrorData> {
        if let Err(e) = unknown_fields(
            &arguments,
            &[
                "query",
                "tokens",
                "strategy",
                "roots",
                "include_memory",
                "include_briefs",
                "task_id",
                "task_revision",
                "compose_evidence",
                "compact_evidence",
                "boundary_config",
            ],
        ) {
            return Ok(foundry_error_result(&e));
        }
        let compose_evidence = match arguments.get("compose_evidence") {
            None => false,
            Some(serde_json::Value::Bool(v)) => *v,
            _ => {
                return Ok(error_result(
                    "invalid_argument",
                    "compose_evidence must be boolean",
                    false,
                ));
            }
        };
        let compact_evidence = match arguments.get("compact_evidence") {
            None => false,
            Some(serde_json::Value::Bool(v)) if !*v || compose_evidence => *v,
            _ => {
                return Ok(error_result(
                    "invalid_argument",
                    "compact_evidence must be boolean and requires compose_evidence",
                    false,
                ));
            }
        };
        let boundary_config = match arguments.get("boundary_config") {
            None => None,
            Some(serde_json::Value::String(v)) if !v.is_empty() && v.len() <= 4096 => {
                Some(v.clone())
            }
            _ => {
                return Ok(error_result(
                    "invalid_argument",
                    "boundary_config must be a bounded indexed path",
                    false,
                ));
            }
        };
        if boundary_config.is_some() && !compose_evidence {
            return Ok(error_result(
                "invalid_argument",
                "boundary_config requires compose_evidence",
                false,
            ));
        }
        let task = match (arguments.get("task_id"), arguments.get("task_revision")) {
            (None, None) => None,
            (Some(serde_json::Value::String(id)), Some(revision))
                if !id.is_empty()
                    && id.len() <= 128
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                    && revision.as_u64().is_some_and(|r| r > 0) =>
            {
                Some((id.clone(), revision.as_u64().expect("validated revision")))
            }
            _ => {
                return Ok(error_result(
                    "invalid_argument",
                    "task_id and positive task_revision must be supplied together",
                    false,
                ));
            }
        };
        if compose_evidence && task.is_none() {
            return Ok(error_result(
                "invalid_argument",
                "compose_evidence requires task_id and task_revision",
                false,
            ));
        }
        if task.is_some() && !self.state.experimental_task_context {
            return Ok(error_result(
                "unsupported_mode",
                "task-aware context requires --experimental-task-context",
                false,
            ));
        }
        let query = match required_str(&arguments, "query") {
            Ok(query) => query,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        if query.trim().is_empty() || query.len() > 4096 {
            return Ok(error_result(
                "invalid_argument",
                "query must contain 1..4096 bytes and be nonblank",
                false,
            ));
        }
        let tokens = match optional_u64(&arguments, "tokens", 2048, 1, 32768) {
            Ok(tokens) => tokens,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        let strategy = match arguments.get("strategy") {
            None => Strategy::Auto,
            Some(serde_json::Value::Null) => {
                return Ok(error_result(
                    "invalid_argument",
                    "optional argument `strategy` must be omitted, not null",
                    false,
                ));
            }
            Some(serde_json::Value::String(s)) if s == "auto" => Strategy::Auto,
            Some(serde_json::Value::String(s)) if s == "search" => Strategy::Search,
            Some(serde_json::Value::String(s)) if s == "graph" => Strategy::Graph,
            Some(_) => {
                return Ok(error_result(
                    "invalid_argument",
                    "argument `strategy` must be one of auto|search|graph",
                    false,
                ));
            }
        };
        let include_briefs = match arguments.get("include_briefs") {
            None => false,
            Some(serde_json::Value::Bool(value)) => *value,
            _ => {
                return Ok(error_result(
                    "invalid_argument",
                    "include_briefs must be boolean",
                    false,
                ));
            }
        };
        if include_briefs && self.state.knowledge.is_none() {
            return Ok(error_result(
                "unsupported_mode",
                "derived context requires --knowledge-profile",
                false,
            ));
        }
        let include_memory = match arguments.get("include_memory") {
            None => false,
            Some(serde_json::Value::Bool(include)) => *include,
            Some(_) => {
                return Ok(error_result(
                    "invalid_argument",
                    "optional argument `include_memory` must be a boolean, omitted or not null",
                    false,
                ));
            }
        };
        if include_memory && self.state.no_memory {
            return Ok(error_result(
                "unsupported_mode",
                "memory is disabled on this owner (--no-memory)",
                false,
            ));
        }
        // `roots` selects already admitted aliases, validated before dispatch.
        let roots = match roots_argument(&arguments) {
            Ok(roots) => roots,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        // Validated in BOTH owner modes: an unknown alias never silently
        // falls back to the primary on a no-reference owner.
        if let Err(e) = self.state.validate_aliases(roots.as_deref()) {
            return Ok(foundry_error_result(&e));
        }
        let query = query.to_owned();
        if self.state.meta.len() > 1 {
            return Ok(self
                .context_roots(
                    &ctx,
                    tokens,
                    query,
                    strategy,
                    roots,
                    include_memory,
                    task,
                    include_briefs,
                    compose_evidence,
                    compact_evidence,
                    boundary_config,
                )
                .await);
        }
        let task_requested = task.is_some();
        let semantic = self.state.semantic.clone();
        let policy = Arc::clone(&self.state.policy);
        let experimental_behavior = self.state.experimental_behavior_context;
        Ok(self
            .deliver(
                &ctx,
                tokens,
                "context",
                move || {
                    if task_requested {
                        response::MAX_BUDGET_TOKENS
                    } else {
                        response::refusal_floor("context", None)
                    }
                },
                move |engines, control, _budget| {
                    let engine = engines[0].as_ref().expect("the primary engine is open");
                    // 013 T003: a configured policy routes `auto`; without
                    // one this is exactly the baseline path.
                    let routed = policy.routes().then_some(&*policy);
                    let workspace_id = engine
                        .workspace_id()
                        .ok_or(FoundryError::WorkspaceUnbound)?;
                    let task_scope =
                        task.as_ref()
                            .map(|(id, revision)| crate::task_context::Scope {
                                workspace_id: &workspace_id,
                                task_id: id,
                                expected_revision: *revision,
                            });
                    let combined = context_primary_with_options(
                        &semantic,
                        engine,
                        &query,
                        strategy,
                        control,
                        crate::store::ContextOptions {
                            task: task_scope,
                            compose_evidence,
                            compact_evidence,
                            boundary_config: boundary_config.as_deref(),
                            memory: include_memory,
                            briefs: include_briefs,
                            policy: routed,
                            experimental_behavior,
                            ..crate::store::ContextOptions::default()
                        },
                    )?;
                    Ok(combined)
                },
                |outcome: &memory::MemoryContext,
                 budget: response::Budget,
                 boundary: response::ByteMeasure| {
                    response::pack_enriched_context(outcome, None, budget, boundary)
                },
            )
            .await)
    }

    #[tool(
        name = "retrieve",
        description = r#"Read exact indexed source by handle; stale handles rejected. `lines`: line range; `view:"outline"`: skeleton with elided line ranges."#,
        input_schema = schema(r#"{"type":"object","additionalProperties":false,"required":["handle"],"properties":{"handle":{"type":"string","maxLength":4200},"tokens":{"type":"integer","minimum":1,"maximum":32768,"default":2048},"lines":{"type":["string","array"],"pattern":"^[1-9][0-9]*(-[1-9][0-9]*)?$","items":{"type":"integer"}},"view":{"type":"string","enum":["text","outline"],"default":"text"}}}"#),
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn retrieve(
        &self,
        ctx: RequestContext<RoleServer>,
        arguments: JsonObject,
    ) -> Result<CallToolResult, ErrorData> {
        if let Err(e) = unknown_fields(&arguments, &["handle", "tokens", "lines", "view"]) {
            return Ok(foundry_error_result(&e));
        }
        let handle = match arguments.get("handle") {
            Some(serde_json::Value::String(handle)) => handle.as_str(),
            None | Some(serde_json::Value::Null) => {
                return Ok(error_result(
                    "invalid_argument",
                    "missing required string argument `handle`",
                    false,
                ));
            }
            // A v1 handle object (or any other non-string) names the grammar.
            Some(_) => {
                return Ok(error_result(
                    "invalid_argument",
                    crate::store::HANDLE_V2_GRAMMAR,
                    false,
                ));
            }
        };
        let lines = match optional_lines(&arguments) {
            Ok(lines) => lines,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        // Field-stage validation before any reservation or engine admission:
        // a malformed handle or `lines` is `invalid_argument` even while the
        // single engine slot is held by another request. Workspace,
        // existence, digest and range stay in the authoritative read.
        let parsed = match HandleRef::parse(handle).and_then(|parsed| {
            lines
                .as_deref()
                .map(LineSelection::parse)
                .transpose()
                .map(|_| parsed)
        }) {
            Ok(parsed) => parsed,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        let tokens = match optional_u64(&arguments, "tokens", 2048, 1, 32768) {
            Ok(tokens) => tokens,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        let outline = match optional_str(&arguments, "view") {
            Ok(None | Some("text")) => false,
            Ok(Some("outline")) => true,
            Ok(Some(_)) => {
                return Ok(error_result(
                    "invalid_argument",
                    "argument `view` must be text or outline",
                    false,
                ));
            }
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        let handle = handle.to_owned();
        // A multi-root owner resolves the root by the handle's `ws16` before
        // dispatch (007 § Multi-root identity): unknown is `wrong_workspace`,
        // a known root without an open engine is `root_unavailable`.
        let root = if self.state.meta.len() > 1 {
            match self.root_of_handle(&parsed.ws16) {
                Ok(root) => Some(root),
                Err(result) => return Ok(result),
            }
        } else {
            None
        };
        if outline {
            if let Some(root) = root {
                return Ok(self
                    .retrieve_outline_roots(&ctx, tokens, handle, lines, root)
                    .await);
            }
            return Ok(self
                .deliver(
                    &ctx,
                    tokens,
                    "retrieve",
                    response::outline_refusal_floor,
                    move |engines, _control, budget| {
                        engines[0]
                            .as_ref()
                            .expect("the primary engine is open")
                            .retrieve_outline(&handle, lines.as_deref(), budget)
                    },
                    response::pack_retrieve_outline,
                )
                .await);
        }
        if let Some(root) = root {
            return Ok(self
                .retrieve_roots(&ctx, tokens, handle, lines, parsed, root)
                .await);
        }
        Ok(self
            .deliver(
                &ctx,
                tokens,
                "retrieve",
                move || response::refusal_floor("retrieve", Some(&parsed)),
                move |engines, _control, budget| {
                    engines[0]
                        .as_ref()
                        .expect("the primary engine is open")
                        .retrieve(&handle, lines.as_deref(), budget)
                },
                response::pack_retrieve,
            )
            .await)
    }

    #[tool(
        name = "index",
        description = "Index an admitted root or control semantic preparation.",
        input_schema = schema(r#"{"type":"object","additionalProperties":false,"properties":{"timeout_ms":{"type":"integer","minimum":1,"maximum":1200000,"default":30000},"root":{"type":"string"},"scip":{"type":"object","additionalProperties":false,"required":["index_file","snapshot_file"],"properties":{"index_file":{"type":"string"},"snapshot_file":{"type":"string"}}},"semantic":{"enum":["prepare","maintain","pause"]},"priority_paths":{"type":"array","maxItems":16,"items":{"type":"string","maxLength":4096}}}}"#),
        // `index` writes only Foundry's own store for the bound root; it
        // never modifies workspace files, and re-indexing converges.
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn index(
        &self,
        ctx: RequestContext<RoleServer>,
        arguments: JsonObject,
    ) -> Result<CallToolResult, ErrorData> {
        if let Err(e) = unknown_fields(
            &arguments,
            &[
                "timeout_ms",
                "root",
                "scip",
                "semantic",
                "priority_paths",
                "knowledge",
                "boundary",
            ],
        ) {
            return Ok(foundry_error_result(&e));
        }
        if let Some(value) = arguments.get("boundary") {
            if !self.state.experimental_task_context {
                return Ok(error_result(
                    "unsupported_mode",
                    "boundary publication requires --experimental-task-context",
                    false,
                ));
            }
            if arguments
                .keys()
                .any(|k| !["boundary", "timeout_ms"].contains(&k.as_str()))
            {
                return Ok(error_result(
                    "invalid_argument",
                    "boundary publication is a separate primary-store index operation",
                    false,
                ));
            }
            let request =
                match serde_json::from_value::<crate::boundary::IndexRequest>(value.clone()) {
                    Ok(r) => r,
                    Err(_) => {
                        return Ok(error_result(
                            "invalid_argument",
                            "boundary requires config and optional paths",
                            false,
                        ));
                    }
                };
            let watch = self.state.watch.clone();
            if request.maintain.is_some() && watch.is_none() {
                return Ok(error_result(
                    "unsupported_mode",
                    "boundary maintenance requires --watch",
                    false,
                ));
            }
            if request.config.is_empty()
                || request.config.len() > 4096
                || request.maintain == Some(false) && !request.paths.is_empty()
            {
                return Ok(error_result(
                    "invalid_argument",
                    "boundary config must be bounded; pause accepts no paths",
                    false,
                ));
            }
            let timeout_ms = match optional_u64(
                &arguments,
                "timeout_ms",
                INDEX_TIMEOUT_MS,
                INDEX_TIMEOUT_RANGE.0,
                INDEX_TIMEOUT_RANGE.1,
            ) {
                Ok(v) => v,
                Err(e) => return Ok(foundry_error_result(&e)),
            };
            let result = run_op(
                &self.state,
                Instant::now() + Duration::from_millis(timeout_ms),
                Some(ctx.ct.clone()),
                Self::admission_guard(&ctx),
                false,
                move |engines, control| {
                    if request.maintain == Some(false) {
                        watch
                            .as_ref()
                            .expect("validated watcher")
                            .boundary_maintenance(request.config, false)?;
                        return Ok(serde_json::json!({"boundary_maintenance":"paused"}));
                    }
                    let report = engines[0]
                        .as_ref()
                        .expect("primary store")
                        .index_boundaries(&request.config, &request.paths, control)?;
                    if request.maintain == Some(true) {
                        watch
                            .as_ref()
                            .expect("validated watcher")
                            .boundary_maintenance(request.config, true)?;
                    }
                    Ok(serde_json::to_value(report).expect("publication serializes"))
                },
            )
            .await;
            return Ok(match result {
                Ok(report) => text_result(response::compact_json(
                    &serde_json::to_value(report).expect("publication serializes"),
                )),
                Err(e) => foundry_error_result(&e),
            });
        }
        if let Some(action) = arguments.get("knowledge") {
            if arguments.len() != 1 {
                return Ok(error_result(
                    "invalid_argument",
                    "knowledge control cannot be combined with indexing or semantic arguments",
                    false,
                ));
            }
            let Some(knowledge) = &self.state.knowledge else {
                return Ok(error_result(
                    "unsupported_mode",
                    "knowledge requires --knowledge-profile",
                    false,
                ));
            };
            match action.as_str() {
                Some("prepare" | "maintain") => {
                    if let Err(error) = knowledge.start(self.state.clone(), action == "maintain") {
                        return Ok(foundry_error_result(&error));
                    }
                }
                Some("pause") => knowledge.pause(),
                _ => {
                    return Ok(error_result(
                        "invalid_argument",
                        "knowledge must be prepare, maintain or pause",
                        false,
                    ));
                }
            }
            return Ok(text_result(response::compact_json(&knowledge.status())));
        }
        // 009 T003: `semantic` starts/resumes or pauses the owner's
        // progressive preparation, and `index` then does ONLY that.
        match arguments.get("semantic") {
            None => {}
            Some(serde_json::Value::String(action))
                if matches!(action.as_str(), "prepare" | "maintain" | "pause") =>
            {
                if arguments.contains_key("root") || arguments.contains_key("scip") {
                    return Ok(error_result(
                        "invalid_argument",
                        "`semantic` cannot be combined with `root` or `scip`",
                        false,
                    ));
                }
                if let Err(e) = optional_u64(
                    &arguments,
                    "timeout_ms",
                    INDEX_TIMEOUT_MS,
                    INDEX_TIMEOUT_RANGE.0,
                    INDEX_TIMEOUT_RANGE.1,
                ) {
                    return Ok(foundry_error_result(&e));
                }
                let priorities = match arguments.get("priority_paths") {
                    Some(value) if action != "pause" => {
                        match serde_json::from_value::<Vec<String>>(value.clone()) {
                            Ok(paths) => Some(paths),
                            Err(_) => {
                                return Ok(error_result(
                                    "invalid_argument",
                                    "priority_paths must be an array of relative paths",
                                    false,
                                ));
                            }
                        }
                    }
                    Some(_) => {
                        return Ok(error_result(
                            "invalid_argument",
                            "priority_paths requires semantic prepare or maintain",
                            false,
                        ));
                    }
                    None => None,
                };
                return Ok(self.index_semantic(&ctx, action, priorities).await);
            }
            Some(serde_json::Value::Null) => {
                return Ok(error_result(
                    "invalid_argument",
                    "optional argument `semantic` must be omitted, not null",
                    false,
                ));
            }
            Some(_) => {
                return Ok(error_result(
                    "invalid_argument",
                    "argument `semantic` must be prepare, maintain or pause",
                    false,
                ));
            }
        }
        if arguments.contains_key("priority_paths") {
            return Ok(error_result(
                "invalid_argument",
                "priority_paths requires semantic prepare",
                false,
            ));
        }
        // `root` re-indexes one admitted alias through its own store
        // (default `primary`), validated before dispatch: a root whose
        // coverage is not `ok` is `root_unavailable`.
        let root = match optional_str(&arguments, "root") {
            Ok(None) => "primary".to_owned(),
            Ok(Some(alias)) => alias.to_owned(),
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        let index = match self.state.alias_index(&root) {
            Some(index) => index,
            None => {
                return Ok(error_result(
                    "invalid_argument",
                    &format!("unknown root alias `{root}`; roots are admitted only at launch"),
                    false,
                ));
            }
        };
        if !self.state.meta[index].coverage.serves_search() {
            return Ok(error_result(
                "root_unavailable",
                &format!(
                    "root `{}` cannot serve: {}",
                    root,
                    self.state.meta[index].coverage.as_str()
                ),
                false,
            ));
        }
        let timeout_ms = match optional_u64(
            &arguments,
            "timeout_ms",
            INDEX_TIMEOUT_MS,
            INDEX_TIMEOUT_RANGE.0,
            INDEX_TIMEOUT_RANGE.1,
        ) {
            Ok(timeout_ms) => timeout_ms,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        // 005 T003: with `scip`, `index` imports the staged artifact and
        // manifest INSTEAD of refreshing sources.
        let scip = match arguments.get("scip") {
            None => None,
            Some(serde_json::Value::Null) => {
                return Ok(error_result(
                    "invalid_argument",
                    "optional argument `scip` must be omitted, not null",
                    false,
                ));
            }
            Some(value @ serde_json::Value::Object(_)) => Some(value),
            Some(_) => {
                return Ok(error_result(
                    "invalid_argument",
                    "argument `scip` must be an object naming `index_file` and `snapshot_file`",
                    false,
                ));
            }
        };
        if let Some(scip) = scip {
            return Ok(self.index_scip(&ctx, index, scip, timeout_ms).await);
        }
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let meta = Arc::clone(&self.state.meta);
        let attempt = run_op(
            &self.state,
            deadline,
            Some(ctx.ct.clone()),
            Self::admission_guard(&ctx),
            false,
            move |engines, control| {
                engines[index]
                    .as_mut()
                    .expect("a serving root holds an engine")
                    .index(&meta[index].root, control)
            },
        )
        .await;
        let report = match attempt {
            Ok(report) => report,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        // The core builds the bounded counts-only partial error (fixed
        // ASCII message, "partial" embedded, <=1024 bytes serialized).
        if let Some(error) = report.index_error() {
            for sample in report.failure_samples.iter().take(20) {
                eprintln!("foundry-mcp: index failure sample: {sample}");
            }
            return Ok(foundry_error_result(&error));
        }
        let report_value = serde_json::to_value(&report).unwrap_or(serde_json::Value::Null);
        let result = text_result(response::compact_json(&report_value));
        if serialize_result(&result).len() > OUTPUT_BYTE_CAP {
            return Ok(error_result(
                "budget_too_small",
                "serialized index report exceeds the 256 KiB output cap",
                false,
            ));
        }
        Ok(result)
    }

    /// 005 T003: `index {scip}` — import a staged artifact and manifest for
    /// the selected root's store instead of refreshing sources. Inside the
    /// one engine slot, the engine's canonical store directory is opened,
    /// its `imports` directory below it and each named entry below that,
    /// all without following links and relative to the descriptor above;
    /// each entry is `fstat`ed as a regular file, and the importer copies
    /// THOSE descriptors (never a re-resolved name), so a swap after the
    /// check cannot change what is imported. The caller's staged files are
    /// only read, never modified or removed. A controlled partial or
    /// cancelled import is the report with `complete:false` and its
    /// committed counts; preflight refusals keep their bounded errors, and a
    /// competing owner is `store_busy`.
    async fn index_scip(
        &self,
        ctx: &RequestContext<RoleServer>,
        index: usize,
        scip: &serde_json::Value,
        timeout_ms: u64,
    ) -> CallToolResult {
        let invalid = |detail: &str| error_result("invalid_argument", detail, false);
        let names = ["index_file", "snapshot_file"];
        let staged: Vec<String> = match names
            .iter()
            .map(|field| scip.get(*field).and_then(serde_json::Value::as_str))
            .collect::<Option<Vec<&str>>>()
        {
            Some(values) if scip.as_object().is_some_and(|map| map.len() == names.len()) => {
                values.into_iter().map(str::to_owned).collect()
            }
            _ => {
                return invalid(
                    "argument `scip` must name exactly `index_file` and `snapshot_file` strings",
                );
            }
        };
        for name in &staged {
            if !valid_staged_name(name) {
                return invalid(&format!(
                    "`{name}` is not a staged import name: 1..128 ASCII letters, digits, `.`, `_` or `-`"
                ));
            }
        }
        let (index_name, snapshot_name) = (staged[0].clone(), staged[1].clone());
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let attempt = run_op(
            &self.state,
            deadline,
            Some(ctx.ct.clone()),
            Self::admission_guard(ctx),
            false,
            move |engines, control| {
                let engine = engines[index]
                    .as_mut()
                    .expect("a serving root holds an engine");
                let (index_file, snapshot_file) =
                    open_staged_pair(engine.directory(), [&index_name, &snapshot_name])?;
                engine.import_scip_inputs(
                    ImportInput::Open(index_file),
                    ImportInput::Open(snapshot_file),
                    control,
                    &ImportLimits::default(),
                )
            },
        )
        .await;
        let report = match attempt {
            Ok(report) => report,
            Err(e) => return foundry_error_result(&e),
        };
        let report_value = serde_json::to_value(&report).unwrap_or(serde_json::Value::Null);
        let result = text_result(response::compact_json(&report_value));
        if serialize_result(&result).len() > OUTPUT_BYTE_CAP {
            return error_result(
                "budget_too_small",
                "serialized import report exceeds the 256 KiB output cap",
                false,
            );
        }
        result
    }

    #[tool(
        name = "references",
        description = "Compiler references to one symbol from imported SCIP: `symbol_id`, or `handle` + `byte_offset`. Page with `after`.",
        input_schema = schema(r#"{"type":"object","additionalProperties":false,"properties":{"symbol_id":{"type":"string"},"handle":{"type":"string","maxLength":4200},"byte_offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":256,"default":64},"tokens":{"type":"integer","minimum":1,"maximum":32768,"default":1024},"after":{"type":"string"}}}"#),
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn references(
        &self,
        ctx: RequestContext<RoleServer>,
        arguments: JsonObject,
    ) -> Result<CallToolResult, ErrorData> {
        if let Err(e) = unknown_fields(
            &arguments,
            &[
                "symbol_id",
                "handle",
                "byte_offset",
                "limit",
                "tokens",
                "after",
            ],
        ) {
            return Ok(foundry_error_result(&e));
        }
        let symbol_id = match optional_str(&arguments, "symbol_id") {
            Ok(symbol_id) => symbol_id,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        let handle = match optional_str(&arguments, "handle") {
            Ok(handle) => handle,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        let byte_offset = match arguments.get("byte_offset") {
            None => None,
            Some(serde_json::Value::Null) => {
                return Ok(error_result(
                    "invalid_argument",
                    "optional argument `byte_offset` must be omitted, not null",
                    false,
                ));
            }
            Some(value) => match value.as_u64() {
                Some(offset) => Some(offset),
                None => {
                    return Ok(error_result(
                        "invalid_argument",
                        "argument `byte_offset` must be a nonnegative integer",
                        false,
                    ));
                }
            },
        };
        // Exactly one seed form: `symbol_id`, `handle` with `byte_offset`,
        // or `handle` alone (its definition unit's symbols, context-v2
        // § Doors).
        let seed = match (symbol_id, handle, byte_offset) {
            (Some(_), Some(_), _) | (Some(_), None, Some(_)) | (None, None, _) => {
                return Ok(error_result(
                    "invalid_argument",
                    "exactly one seed form is required: `symbol_id`, or `handle` with an optional `byte_offset`",
                    false,
                ));
            }
            (Some(symbol_id), None, None) => ReferencesSeed::SymbolId(symbol_id.to_owned()),
            (None, Some(handle), Some(byte_offset)) => ReferencesSeed::Position {
                handle: handle.to_owned(),
                byte_offset,
            },
            (None, Some(handle), None) => ReferencesSeed::Handle(handle.to_owned()),
        };
        let limit = match optional_u64(
            &arguments,
            "limit",
            REFERENCES_DEFAULT_LIMIT as u64,
            1,
            REFERENCES_MAX_LIMIT as u64,
        ) {
            Ok(limit) => limit as usize,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        let tokens = match optional_u64(&arguments, "tokens", 1024, 1, 32768) {
            Ok(tokens) => tokens,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        let after = match optional_str(&arguments, "after") {
            Ok(after) => after.map(str::to_owned),
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        let request = ReferencesRequest { seed, limit, after };
        // Field-stage validation of EVERY field - types, bounds, nulls, the
        // symbol-id and handle grammars and the cursor grammar - before any
        // root routing, reservation or engine admission, exactly like
        // `retrieve`: a malformed field is `invalid_argument` even for a
        // foreign handle, an unavailable root or a busy slot. Existence,
        // digest and range stay in the authoritative read.
        if let Err(e) = request.validate() {
            return Ok(foundry_error_result(&e));
        }
        // A multi-root owner routes a handle seed by its `ws16` exactly
        // like `retrieve`; a symbol seed queries the primary root.
        if self.state.meta.len() > 1 {
            let root = match &request.seed {
                ReferencesSeed::Position { handle, .. } | ReferencesSeed::Handle(handle) => {
                    let parsed = match HandleRef::parse(handle) {
                        Ok(parsed) => parsed,
                        Err(e) => return Ok(foundry_error_result(&e)),
                    };
                    match self.root_of_handle(&parsed.ws16) {
                        Ok(root) => root,
                        Err(result) => return Ok(result),
                    }
                }
                ReferencesSeed::SymbolId(_) => 0,
            };
            return Ok(self.references_roots(&ctx, tokens, request, root).await);
        }
        Ok(self
            .deliver(
                &ctx,
                tokens,
                "references",
                response::references_refusal_floor,
                move |engines, _control, _budget| {
                    engines[0]
                        .as_ref()
                        .expect("the primary engine is open")
                        .references(&request)
                },
                response::pack_references,
            )
            .await)
    }

    #[tool(
        name = "status",
        description = "Progress and coverage for admitted roots; optional semantic audit.",
        input_schema = schema(r#"{"type":"object","additionalProperties":false,"properties":{"semantic_audit":{"type":"boolean","default":false}}}"#),
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn status(
        &self,
        ctx: RequestContext<RoleServer>,
        arguments: JsonObject,
    ) -> Result<CallToolResult, ErrorData> {
        if let Err(e) = unknown_fields(&arguments, &["semantic_audit"]) {
            return Ok(foundry_error_result(&e));
        }
        let semantic_audit = match arguments.get("semantic_audit") {
            None => false,
            Some(serde_json::Value::Bool(audit)) => *audit,
            _ => {
                return Ok(error_result(
                    "invalid_argument",
                    "semantic_audit must be boolean",
                    false,
                ));
            }
        };
        let meta = Arc::clone(&self.state.meta);
        let multi = meta.len() > 1;
        // 009 T003: an owner serving a semantic profile adds the `semantic`
        // object (committed metadata plus its live driver state); without a
        // profile the status is unchanged.
        let semantic = self.state.semantic.clone();
        let watch = self.state.watch.clone();
        let knowledge = self.state.knowledge.clone();
        #[cfg(feature = "semantic")]
        let preparation = Arc::clone(&self.state.preparation);
        let mut outcome = match run_engine_op(
            &self.state,
            Instant::now() + READ_DEADLINE,
            Some(ctx.ct.clone()),
            Self::admission_guard(&ctx),
            move |engines, control| {
                let primary = engines[0].as_ref().expect("the primary engine is open");
                let status = primary.status()?;
                let mut value = serde_json::to_value(&status).unwrap_or(serde_json::Value::Null);
                #[cfg(feature = "semantic")]
                if let Some(slot) = &semantic {
                    // The resident runtime's word, from its last observed
                    // call outcome; status never calls the model.
                    let runtime = match slot {
                        Ok(runtime) => runtime.status_word(),
                        Err(word) => word.clone(),
                    };
                    value["semantic"] =
                        (if semantic_audit {
                            crate::neural::driver::status_audit_object
                        } else {
                            crate::neural::driver::status_object
                        })(primary, preparation.live(), &runtime, control)?;
                    if let Some(progress) = preparation.progress() {
                        value["semantic"]["bootstrap"] = serde_json::to_value(progress)?;
                    }
                }
                #[cfg(not(feature = "semantic"))]
                let _ = (&semantic, control, semantic_audit);
                if let Some(knowledge) = &knowledge {
                    let mut observed = primary.knowledge_status()?;
                    observed["maintenance"] = knowledge.status();
                    value["knowledge"] = observed;
                }
                if let Some(watch) = &watch {
                    value["watch"] = watch.status(primary.pending()?);
                }
                if multi {
                    // 007: every admitted root, with nulls where a store
                    // could not be opened.
                    let mut roots = Vec::with_capacity(meta.len());
                    for (index, root) in meta.iter().enumerate() {
                        let mut entry = serde_json::json!({
                            "alias": root.alias,
                            "label": root.label,
                            "root": root.root.display().to_string(),
                            "workspace_id": serde_json::Value::Null,
                            "coverage": root.coverage.as_str(),
                            "source_revision": serde_json::Value::Null,
                            "pending_sources": serde_json::Value::Null,
                            "scan_state": serde_json::Value::Null,
                            "index_state": serde_json::Value::Null,
                        });
                        if let Some(engine) = engines[index].as_ref() {
                            let status = engine.status()?;
                            entry["workspace_id"] = serde_json::json!(engine.workspace_id());
                            entry["source_revision"] = serde_json::json!(status.source_revision);
                            entry["pending_sources"] =
                                serde_json::json!(engine.pending_source_work()?);
                            entry["scan_state"] = serde_json::json!(status.scan_state);
                            entry["index_state"] = serde_json::json!(status.index_state);
                        }
                        roots.push(entry);
                    }
                    value["roots"] = serde_json::Value::Array(roots);
                }
                Ok(value)
            },
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        // 013 T003: the policy state (no engine work).
        outcome["policy"] = self.state.policy.status();
        if let Some(resources) = &self.state.model_resources {
            outcome["model_resources"] = resources.status();
        }
        Ok(text_result(response::compact_json(&outcome)))
    }

    #[tool(
        name = "task_context",
        description = "Experimental task state and recoverable views. Explicit scoped JSON commands; export uses CLI.",
        input_schema = schema(r#"{"type":"object","additionalProperties":false,"required":["workspace_id","task_id","command"],"properties":{"workspace_id":{"type":"string"},"task_id":{"type":"string"},"command":{"type":"object","additionalProperties":false,"required":["op"],"properties":{"op":{"enum":["create","apply","inspect","peek","render","forget","advise","review","promote","search"]},"title":{"type":"string"},"expected_revision":{"type":"integer","minimum":0},"edits":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["op"],"properties":{"op":{"enum":["add","select","pin","fold","expand","checkpoint","restore","dismiss"]},"entry":{"type":"object","additionalProperties":false,"required":["id","kind","text","author","provenance","source_links"],"properties":{"id":{"type":"string"},"kind":{"enum":["requirement","constraint","observation","hypothesis","decision","rejected_attempt","question","work","artifact"]},"text":{"type":"string"},"author":{"type":"string"},"provenance":{"type":"string"},"source_links":{"type":"array","items":{"type":"string"}},"supersedes":{"type":["string","null"]},"action_key":{"type":["string","null"]}}},"entry_id":{"type":"string"},"selected":{"type":"boolean"},"pinned":{"type":"boolean"},"summary":{"type":"string"},"name":{"type":"string"}}},"minItems":1,"maxItems":32},"entry_id":{"type":"string"},"offset":{"type":"integer","minimum":0},"bytes":{"type":"integer","minimum":4,"maximum":65536},"max_bytes":{"type":"integer","minimum":256,"maximum":1048576},"action_key":{"type":"string"},"paths":{"type":"array","maxItems":16,"items":{"type":"string"}},"finishing":{"type":"boolean"},"memory_id":{"type":"string"},"author":{"type":"string"},"provenance":{"type":"string"},"query":{"type":"string","minLength":1,"maxLength":256},"limit":{"type":"integer","minimum":1,"maximum":32},"after_id":{"type":["string","null"]},"include_history":{"type":"boolean"}}}}}"#),
        annotations(read_only_hint = false, open_world_hint = false)
    )]
    async fn task_context(
        &self,
        ctx: RequestContext<RoleServer>,
        arguments: JsonObject,
    ) -> Result<CallToolResult, ErrorData> {
        let request = match crate::task_context::parse(serde_json::Value::Object(arguments)) {
            Ok(request) => request,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        if self.state.no_memory
            && matches!(
                request.command,
                crate::task_context::Command::Promote { .. }
            )
        {
            return Ok(foundry_error_result(&FoundryError::UnsupportedMode(
                "project memory is disabled on this owner".into(),
            )));
        }
        if matches!(
            request.command,
            crate::task_context::Command::Export {}
                | crate::task_context::Command::ExportHost { .. }
        ) {
            return Ok(foundry_error_result(&FoundryError::UnsupportedMode(
                "task export uses CLI".into(),
            )));
        }
        let read = matches!(
            request.command,
            crate::task_context::Command::Search { .. }
                | crate::task_context::Command::Advise { .. }
                | crate::task_context::Command::Review { .. }
                | crate::task_context::Command::Inspect {}
                | crate::task_context::Command::Peek { .. }
                | crate::task_context::Command::Render { .. }
        );
        Ok(self
            .memory_op(&ctx, read, move |engine, control| {
                let result = engine.task_context(&request, control)?;
                engine.drain_task_memory(&result);
                Ok(result)
            })
            .await)
    }

    #[tool(
        name = "memory",
        description = "Explicit project memory records.",
        input_schema = schema(r#"{"type":"object","additionalProperties":false,"required":["op","workspace_id"],"properties":{"op":{"type":"string"},"id":{"type":"string"},"text":{"type":"string"},"author":{"type":"string"},"provenance":{"type":"string"},"source_links":{"type":"array","items":{"type":"string"}},"workspace_id":{"type":"string"},"expected_revision":{"type":"integer"},"query":{"type":"string"}}}"#),
        annotations(read_only_hint = false, open_world_hint = false)
    )]
    async fn memory(
        &self,
        ctx: RequestContext<RoleServer>,
        arguments: JsonObject,
    ) -> Result<CallToolResult, ErrorData> {
        // `limit` and `tokens` are deliberately not part of this tool: search
        // uses the default hit limit and the same effective budget as `search`.
        if let Err(e) = unknown_fields(
            &arguments,
            &[
                "op",
                "id",
                "text",
                "author",
                "provenance",
                "source_links",
                "workspace_id",
                "expected_revision",
                "query",
            ],
        ) {
            return Ok(foundry_error_result(&e));
        }
        let request = match memory::parse_request(&arguments) {
            Ok(request) => request,
            Err(e) => return Ok(foundry_error_result(&e)),
        };
        // Memory lives only in the primary (writable) store (007): every
        // operation runs on engine 0, and a reference root's workspace ID is
        // `wrong_workspace` at the engine's scope check.
        Ok(match request {
            MemoryRequest::Put(input) => {
                self.memory_op(&ctx, false, move |engine, _| {
                    let report = engine.memory_put(&input)?;
                    engine.drain_memory_key(&report.id);
                    Ok(serde_json::to_value(&report)?)
                })
                .await
            }
            MemoryRequest::Update(input) => {
                self.memory_op(&ctx, false, move |engine, _| {
                    let report = engine.memory_update(&input)?;
                    engine.drain_memory_key(&report.id);
                    Ok(serde_json::to_value(&report)?)
                })
                .await
            }
            MemoryRequest::Forget(input) => {
                self.memory_op(&ctx, false, move |engine, control| {
                    let report = engine.memory_forget(&input, control)?;
                    engine.drain_memory_key(&report.id);
                    Ok(serde_json::to_value(&report)?)
                })
                .await
            }
            MemoryRequest::Get { id, workspace_id } => {
                self.memory_op(&ctx, true, move |engine, _| {
                    Ok(serde_json::to_value(
                        engine.memory_get(&id, &workspace_id)?,
                    )?)
                })
                .await
            }
            MemoryRequest::Search(input) => {
                let tokens = input.tokens as u64;
                self.deliver(
                    &ctx,
                    tokens,
                    "memory",
                    || response::refusal_floor("search", None),
                    move |engines, _control, _budget| {
                        engines[0]
                            .as_ref()
                            .expect("the primary engine is open")
                            .memory_search(&input)
                    },
                    response::pack_memory_search,
                )
                .await
            }
        })
    }
}

impl FoundryMcp {
    /// One `memory` operation on the primary engine under the single engine
    /// slot. A write opts out of the post-call deadline check (`check_after`
    /// false): a committed mutation is never turned into an error. The result
    /// is the content-free JSON report or the exact record of `get`.
    async fn memory_op<F>(
        &self,
        ctx: &RequestContext<RoleServer>,
        read: bool,
        f: F,
    ) -> CallToolResult
    where
        F: FnOnce(&mut Engine, &Control) -> FResult<serde_json::Value> + Send + 'static,
    {
        let outcome = run_op(
            &self.state,
            Instant::now() + READ_DEADLINE,
            Some(ctx.ct.clone()),
            Self::admission_guard(ctx),
            read,
            move |engines, control| {
                f(
                    engines[0].as_mut().expect("the primary engine is open"),
                    control,
                )
            },
        )
        .await;
        match outcome {
            Ok(value) => {
                let result = text_result(response::compact_json(&value));
                if serialize_result(&result).len() > OUTPUT_BYTE_CAP {
                    return error_result(
                        "budget_too_small",
                        "serialized memory result exceeds the 256 KiB output cap",
                        false,
                    );
                }
                result
            }
            Err(e) => foundry_error_result(&e),
        }
    }

    /// 009 T003: `index {semantic: "prepare" | "pause"}` controls the owner's
    /// progressive preparation of the primary root and returns at once with
    /// the resulting state and reason. `prepare` starts or resumes the
    /// driver; while an earlier model call still occupies the runtime slot it
    /// is refused `provider_busy` before anything is allocated. `pause`
    /// admits no new batch. Without a semantic profile, or after a refused
    /// start, the answer is `semantic_unavailable` with the fallback reason.
    async fn index_semantic(
        &self,
        ctx: &RequestContext<RoleServer>,
        action: &str,
        priority_paths: Option<Vec<String>>,
    ) -> CallToolResult {
        #[cfg(feature = "semantic")]
        {
            let unavailable = |message: String| {
                foundry_error_result(&FoundryError::Semantic {
                    code: "semantic_unavailable",
                    message,
                })
            };
            let runtime = match &self.state.semantic {
                Some(Ok(runtime)) => Arc::clone(runtime),
                Some(Err(word)) => {
                    return unavailable(format!("the semantic runtime did not start ({word})"));
                }
                None => {
                    return unavailable(
                        "this owner serves no semantic profile; start it with --semantic-profile"
                            .into(),
                    );
                }
            };
            let preparation = Arc::clone(&self.state.preparation);
            if action != "pause" {
                let owner: Arc<dyn crate::neural::driver::Owner> = self.state.clone();
                let result = if action == "maintain" {
                    preparation.maintain(owner, runtime, priority_paths)
                } else {
                    preparation.prepare_with_priorities(owner, runtime, priority_paths)
                };
                if let Err(e) = result {
                    return foundry_error_result(&e);
                }
            } else {
                preparation.pause();
            }
            let value = match preparation.live() {
                Some(live) => crate::neural::driver::brief(Some(live), None),
                // No driver: the committed row, read like any other status.
                None => match run_engine_op(
                    &self.state,
                    Instant::now() + READ_DEADLINE,
                    Some(ctx.ct.clone()),
                    Self::admission_guard(ctx),
                    |engines, _| {
                        let row = engines[0]
                            .as_ref()
                            .expect("the primary engine is open")
                            .semantic_state()?;
                        Ok(crate::neural::driver::brief(None, row.as_ref()))
                    },
                )
                .await
                {
                    Ok(value) => value,
                    Err(e) => return foundry_error_result(&e),
                },
            };
            text_result(response::compact_json(&value))
        }
        #[cfg(not(feature = "semantic"))]
        {
            let _ = (ctx, action, priority_paths);
            foundry_error_result(&refuse_unsupported_semantic())
        }
    }
}

/// `Some` when the shared read deadline passed or the request was cancelled:
/// never a successful delivery after either.
fn expired(deadline: Instant, ct: &CancellationToken) -> Option<FoundryError> {
    if ct.is_cancelled() {
        Some(FoundryError::Cancelled(None))
    } else if Instant::now() >= deadline {
        Some(FoundryError::DeadlineExceeded(None))
    } else {
        None
    }
}

impl FoundryMcp {
    /// The shared delivery flow for `search`, `context` and `retrieve`: ONE
    /// atomic reservation against the connection-local allowance, ONE engine
    /// call under ONE read deadline with the effective budget as the core
    /// budget, packing through the MCP byte measure (tokens counted on the
    /// emitted text block, the 256 KiB cap on the serialized result), a final
    /// deadline and cancellation check, and settlement by the counted tokens.
    async fn deliver<T, F, P, R>(
        &self,
        ctx: &RequestContext<RoleServer>,
        tokens: u64,
        what: &'static str,
        floor: R,
        run: F,
        pack: P,
    ) -> CallToolResult
    where
        F: FnOnce(&mut Vec<Option<Engine>>, &Control, usize) -> FResult<T> + Send + 'static,
        T: Send + 'static,
        P: Fn(&T, response::Budget, response::ByteMeasure) -> FResult<response::PackedText>,
        R: FnOnce() -> usize,
    {
        let trace = self
            .state
            .trace_sink
            .as_ref()
            .map(|_| crate::retrieval_trace::RetrievalTrace::default());
        if let Some(trace) = &trace {
            trace.record(serde_json::json!({"stage":"request", "request_id":ctx.id,
                "session_id":Shared::session_key(ctx), "requested_tokens":tokens}));
        }
        let run_trace = trace.clone();
        let result = self
            .deliver_inner(
                ctx,
                tokens,
                what,
                floor,
                move |engines, control, budget| {
                    let control = control
                        .bounded_by(control.deadline().expect("delivery deadline"))
                        .with_trace(run_trace);
                    run(engines, &control, budget)
                },
                pack,
            )
            .await;
        if let (Some(sink), Some(trace)) = (&self.state.trace_sink, trace) {
            // This is the server's result, not a client/provider receipt. A write
            // failure cannot change an already settled delivery allowance.
            if sink
                .save(
                    what,
                    &trace,
                    serde_json::to_value(&result).unwrap_or(serde_json::Value::Null),
                )
                .is_err()
            {
                eprintln!("retrieval_trace_unavailable: run is incomplete");
            }
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    async fn deliver_inner<T, F, P, R>(
        &self,
        ctx: &RequestContext<RoleServer>,
        tokens: u64,
        what: &'static str,
        floor: R,
        run: F,
        pack: P,
    ) -> CallToolResult
    where
        F: FnOnce(&mut Vec<Option<Engine>>, &Control, usize) -> FResult<T> + Send + 'static,
        T: Send + 'static,
        P: Fn(&T, response::Budget, response::ByteMeasure) -> FResult<response::PackedText>,
        R: FnOnce() -> usize,
    {
        let deadline = Instant::now() + READ_DEADLINE;
        let session = Shared::session_key(ctx);
        let (effective, limiter) = match self.state.reserve_effective(&session, tokens) {
            Ok(reserved) => reserved,
            // No engine work is admitted, so the sufficient hint comes from
            // the outcome-free floor; it is valid under every limiter label.
            Err(refusal) => {
                let floor = floor();
                return error_result(
                    "budget_exhausted",
                    &format!(
                        "{what} cannot fit within 0 tokens (limited by {}); minimum {floor} tokens",
                        refusal.limited_by.label()
                    ),
                    false,
                );
            }
        };
        // Every path below settles this one reservation exactly once:
        // `refund` on each refusal (busy, engine error, packing failure,
        // deadline, cancellation), `charge` on the delivery.
        let refund = || self.state.refund(&session, effective);
        let outcome = run_engine_op(
            &self.state,
            deadline,
            Some(ctx.ct.clone()),
            Self::admission_guard(ctx),
            move |engine, control| run(engine, control, effective as usize),
        )
        .await;
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                refund();
                return foundry_error_result(&error);
            }
        };
        let budget = response::Budget {
            tokens: effective as usize,
            limited_by: limiter,
        };
        match pack(&outcome, budget, &emitted_bytes) {
            Ok(packed) => {
                if let Some(error) = expired(deadline, &ctx.ct) {
                    refund();
                    return foundry_error_result(&error);
                }
                self.state.charge(&session, effective, packed.tokens as u64);
                text_result(packed.text)
            }
            Err(FoundryError::BudgetTooSmall { minimum_tokens }) => {
                refund();
                // The core's hint is sufficient under every limiter label.
                // When the session allowance (not the request or the
                // configured ceiling) is the bound that cannot fit even the
                // header, the refusal is allowance exhaustion.
                let code = if limiter == BudgetLimiter::Session {
                    "budget_exhausted"
                } else {
                    "budget_too_small"
                };
                error_result(
                    code,
                    &format!(
                        "{what} cannot fit within {effective} tokens (limited by {}); minimum {minimum_tokens} tokens",
                        limiter.label()
                    ),
                    false,
                )
            }
            Err(error) => {
                refund();
                foundry_error_result(&error)
            }
        }
    }

    /// The meta index of the root a handle's `ws16` names, or the bounded
    /// refusal: unknown `ws16` is `wrong_workspace`; a known root without an
    /// open engine (busy, missing, corrupt, wrong workspace) cannot serve
    /// reads and is `root_unavailable`.
    fn root_of_handle(&self, ws16: &str) -> Result<usize, CallToolResult> {
        match self
            .state
            .meta
            .iter()
            .position(|meta| meta.workspace_id.starts_with(ws16))
        {
            Some(index) if self.state.meta[index].coverage.serves_reads() => Ok(index),
            Some(index) => Err(error_result(
                "root_unavailable",
                &format!(
                    "root `{}` cannot serve reads: {}",
                    self.state.meta[index].alias,
                    self.state.meta[index].coverage.as_str()
                ),
                false,
            )),
            None => Err(error_result(
                "wrong_workspace",
                "handle names no admitted root",
                false,
            )),
        }
    }

    /// The root indices a response's header lists (007): every admitted root
    /// without a `roots` selection — so unavailable references stay visible —
    /// or the selected roots with one.
    fn listed_roots(&self, selected: Option<&[String]>, selection: &[usize]) -> Vec<usize> {
        match selected {
            None => (0..self.state.meta.len()).collect(),
            Some(_) => selection.to_vec(),
        }
    }

    /// The aliases and labels of every admitted root, for the multi-root
    /// refusal floor.
    fn root_labels(&self) -> Vec<(String, String)> {
        Self::meta_labels(&self.state.meta)
    }

    fn meta_labels(meta: &[RootMeta]) -> Vec<(String, String)> {
        meta.iter()
            .map(|meta| (meta.alias.clone(), meta.label.clone()))
            .collect()
    }

    /// 007 combined search: the selected serving roots run sequentially
    /// inside the one read deadline, their batches merge on the candidate
    async fn search_roots(
        &self,
        ctx: &RequestContext<RoleServer>,
        tokens: u64,
        query: String,
        path: Option<String>,
        limit: usize,
        selected: Option<Vec<String>>,
    ) -> CallToolResult {
        let selection = match self.state.resolve_roots(selected.as_deref()) {
            Ok(selection) => selection,
            Err(e) => return foundry_error_result(&e),
        };
        let serving: Vec<usize> = selection
            .iter()
            .copied()
            .filter(|&index| self.state.meta[index].coverage.serves_search())
            .collect();
        if serving.is_empty() {
            // The refusal names every root the response would have listed:
            // all admitted roots when `roots` was omitted, the selected ones
            // otherwise (007 § Combined search and context).
            return self
                .state
                .unavailable_error(&self.listed_roots(selected.as_deref(), &selection));
        }
        let listed = self.listed_roots(selected.as_deref(), &selection);
        let meta = Arc::clone(&self.state.meta);
        let floor_labels = self.root_labels();
        let run_meta = Arc::clone(&meta);
        let pack_meta = Arc::clone(&meta);
        let semantic = self.state.semantic.clone();
        self.deliver(
            ctx,
            tokens,
            "search",
            move || response::refusal_floor_roots("search", None, &floor_labels),
            move |engines, control, _budget| {
                let engines = &*engines;
                let (batches, facts) = collect_root_batches(
                    engines,
                    control,
                    &run_meta,
                    &serving,
                    &query,
                    path.as_deref(),
                    |engine, control, anchors| {
                        let is_primary = std::ptr::eq(
                            engine,
                            engines[0].as_ref().expect("the primary engine is open"),
                        );
                        if is_primary && semantic_on(&semantic) {
                            search_primary(
                                &semantic,
                                engine,
                                &query,
                                path.as_deref(),
                                limit,
                                control,
                                Some(anchors),
                            )
                        } else {
                            engine.search_candidates_with(
                                &query,
                                path.as_deref(),
                                limit,
                                control,
                                Some(anchors),
                            )
                        }
                    },
                )?;
                Ok(MultiOutcome {
                    served: roots::merge_search(&batches, limit),
                    root_facts: facts,
                })
            },
            move |outcome: &MultiOutcome<crate::store::SearchOutcome>,
                  budget: response::Budget,
                  boundary: response::ByteMeasure| {
                let headers = root_headers(&pack_meta, &listed, &outcome.root_facts);
                response::pack_search_roots(&outcome.served, &headers, budget, boundary)
            },
        )
        .await
    }

    /// 007 combined context: same flow as [`Self::search_roots`] over
    /// `context_candidates`; graph items keep their seed root's alias and
    /// identity, freshness and graph evidence stay inside each root.
    #[allow(clippy::too_many_arguments)]
    async fn context_roots(
        &self,
        ctx: &RequestContext<RoleServer>,
        tokens: u64,
        query: String,
        strategy: Strategy,
        selected: Option<Vec<String>>,
        include_memory: bool,
        task: Option<(String, u64)>,
        include_briefs: bool,
        compose_evidence: bool,
        compact_evidence: bool,
        boundary_config: Option<String>,
    ) -> CallToolResult {
        let selection = match self.state.resolve_roots(selected.as_deref()) {
            Ok(selection) => selection,
            Err(e) => return foundry_error_result(&e),
        };
        let serving: Vec<usize> = selection
            .iter()
            .copied()
            .filter(|&index| self.state.meta[index].coverage.serves_search())
            .collect();
        if (task.is_some() || include_briefs) && !serving.contains(&0) {
            return error_result(
                "unsupported_mode",
                "task/derived context requires the primary root to be selected and serving",
                false,
            );
        }
        if serving.is_empty() {
            // The refusal names every root the response would have listed:
            // all admitted roots when `roots` was omitted, the selected ones
            // otherwise (007 § Combined search and context).
            return self
                .state
                .unavailable_error(&self.listed_roots(selected.as_deref(), &selection));
        }
        let listed = self.listed_roots(selected.as_deref(), &selection);
        let meta = Arc::clone(&self.state.meta);
        let floor_labels = self.root_labels();
        let run_meta = Arc::clone(&meta);
        let pack_meta = Arc::clone(&meta);
        let task_requested = task.is_some();
        let semantic = self.state.semantic.clone();
        let policy = Arc::clone(&self.state.policy);
        let experimental_behavior = self.state.experimental_behavior_context;
        self.deliver(
            ctx,
            tokens,
            "context",
            move || {
                if task_requested {
                    response::MAX_BUDGET_TOKENS
                } else {
                    response::refusal_floor_roots("context", None, &floor_labels)
                }
            },
            move |engines, control, _budget| {
                // 008: memory lives only in the primary store, and only when
                // the primary is selected AND serving. The primary's memory
                // validates inside its own final read (context_candidates_
                // memory); any memory trouble degrades to no lines — a
                // multi-root response never fails because of memory.
                let primary_memory = include_memory && serving.contains(&0);
                let mut hits: Vec<memory::MemoryHit> = Vec::new();
                let hits_ref = &mut hits;
                let mut task_view = None;
                let mut briefs = Vec::new();
                let mut evidence_plans = Vec::new();
                let task_entries = if compose_evidence {
                    use redb::ReadableDatabase;
                    let primary = engines[0].as_ref().expect("primary store");
                    let (id, revision) = task.as_ref().expect("validated composition task");
                    primary.task_entries_in(
                        &primary.db.begin_read()?,
                        crate::task_context::Scope {
                            workspace_id: &run_meta[0].workspace_id,
                            task_id: id,
                            expected_revision: *revision,
                        },
                    )?
                } else {
                    Vec::new()
                };
                let engines = &*engines;
                // 005 T004: the first serving root's resolved doors request
                // (a policy may have made it) governs every later root, so
                // the root holding the merged first anchor's definition builds
                // its doors; the policy is consulted once.
                let mut decided: Option<Strategy> = None;
                let (batches, facts) = collect_root_batches(
                    engines,
                    control,
                    &run_meta,
                    &serving,
                    &query,
                    None,
                    |engine, control, anchors| {
                        let strategy = decided.unwrap_or(strategy);
                        let is_primary = std::ptr::eq(
                            engine,
                            engines[0].as_ref().expect("the primary engine is open"),
                        );
                        let task_scope =
                            task.as_ref().filter(|_| is_primary).map(|(id, revision)| {
                                crate::task_context::Scope {
                                    workspace_id: &run_meta[0].workspace_id,
                                    task_id: id,
                                    expected_revision: *revision,
                                }
                            });
                        // 009 T002: semantic evidence belongs to the PRIMARY
                        // root's store only (the merged header says so); so
                        // does 013 T003 routing, whose state the primary
                        // store composes.
                        let routed = policy.routes().then_some(&*policy);
                        let options = crate::store::ContextOptions {
                            task: task_scope,
                            compose_evidence,
                            compact_evidence,
                            boundary_config: boundary_config.as_deref().filter(|_| is_primary),
                            memory: primary_memory && is_primary,
                            briefs: include_briefs && is_primary,
                            policy: routed.filter(|_| is_primary),
                            anchors: Some(anchors),
                            experimental_behavior,
                            #[cfg(feature = "semantic")]
                            dense: None,
                        };
                        let mut combined = if is_primary
                            && (semantic_on(&semantic) || routed.is_some())
                        {
                            context_primary_with_options(
                                &semantic, engine, &query, strategy, control, options,
                            )?
                        } else if compose_evidence && !is_primary {
                            engine.compose_evidence_for_entries(
                                &query,
                                strategy,
                                control,
                                &options,
                                &task_entries,
                            )?
                        } else {
                            engine.context_candidates_with(&query, strategy, control, &options)?
                        };
                        if is_primary {
                            *hits_ref = combined.hits;
                            task_view = combined.task_view;
                            briefs = combined.briefs;
                        }
                        if let Some(plan) = combined.evidence.take() {
                            evidence_plans.push(plan);
                        }
                        let batch = combined.batch;
                        decided.get_or_insert(if batch.doors.is_some() {
                            Strategy::Graph
                        } else {
                            Strategy::Search
                        });
                        Ok(batch)
                    },
                )?;
                let batch = roots::merge_context(&batches);
                let evidence = compose_evidence
                    .then(|| crate::task_evidence::Plan::merge(evidence_plans, &batches, &batch));
                Ok(MultiOutcome {
                    served: memory::MemoryContext {
                        evidence,
                        batch,
                        hits,
                        task_view,
                        briefs,
                    },
                    root_facts: facts,
                })
            },
            move |outcome: &MultiOutcome<memory::MemoryContext>,
                  budget: response::Budget,
                  boundary: response::ByteMeasure| {
                let headers = root_headers(&pack_meta, &listed, &outcome.root_facts);
                response::pack_enriched_context(&outcome.served, Some(&headers), budget, boundary)
            },
        )
        .await
    }

    /// 007 retrieve routed by the handle's root; the header lists every
    /// admitted root's live revision or coverage.
    async fn retrieve_roots(
        &self,
        ctx: &RequestContext<RoleServer>,
        tokens: u64,
        handle: String,
        lines: Option<String>,
        parsed: HandleRef,
        root: usize,
    ) -> CallToolResult {
        let meta = Arc::clone(&self.state.meta);
        let floor_meta = Arc::clone(&meta);
        let pack_meta = Arc::clone(&meta);
        self.deliver(
            ctx,
            tokens,
            "retrieve",
            move || {
                response::refusal_floor_roots(
                    "retrieve",
                    Some(&parsed),
                    &Self::meta_labels(&floor_meta),
                )
            },
            move |engines, _control, budget| {
                let outcome = engines[root]
                    .as_ref()
                    .expect("a read-serving root holds an engine")
                    .retrieve(&handle, lines.as_deref(), budget)?;
                Ok(MultiOutcome {
                    served: outcome,
                    root_facts: root_facts(engines)?,
                })
            },
            move |outcome: &MultiOutcome<crate::store::RetrieveOutcome>,
                  budget: response::Budget,
                  boundary: response::ByteMeasure| {
                let listed: Vec<usize> = (0..pack_meta.len()).collect();
                let headers = root_headers(&pack_meta, &listed, &outcome.root_facts);
                response::pack_retrieve_roots(&outcome.served, &headers, budget, boundary)
            },
        )
        .await
    }

    /// 007 `view:"outline"` retrieve routed by the handle's root.
    async fn retrieve_outline_roots(
        &self,
        ctx: &RequestContext<RoleServer>,
        tokens: u64,
        handle: String,
        lines: Option<String>,
        root: usize,
    ) -> CallToolResult {
        let meta = Arc::clone(&self.state.meta);
        let pack_meta = Arc::clone(&meta);
        self.deliver(
            ctx,
            tokens,
            "retrieve",
            response::outline_refusal_floor,
            move |engines, _control, budget| {
                let outcome = engines[root]
                    .as_ref()
                    .expect("a read-serving root holds an engine")
                    .retrieve_outline(&handle, lines.as_deref(), budget)?;
                Ok(MultiOutcome {
                    served: outcome,
                    root_facts: root_facts(engines)?,
                })
            },
            move |outcome: &MultiOutcome<crate::store::OutlineOutcome>,
                  budget: response::Budget,
                  boundary: response::ByteMeasure| {
                let listed: Vec<usize> = (0..pack_meta.len()).collect();
                let headers = root_headers(&pack_meta, &listed, &outcome.root_facts);
                response::pack_retrieve_outline_roots(&outcome.served, &headers, budget, boundary)
            },
        )
        .await
    }

    /// 005 `references` on a multi-root owner (007): the request runs in the
    /// selected root (the handle's root, or the primary for a symbol seed)
    /// and the header lists every admitted root's own live revision or
    /// coverage, exactly as `retrieve` does - with one reservation and one
    /// charge.
    async fn references_roots(
        &self,
        ctx: &RequestContext<RoleServer>,
        tokens: u64,
        request: ReferencesRequest,
        root: usize,
    ) -> CallToolResult {
        let pack_meta = Arc::clone(&self.state.meta);
        self.deliver(
            ctx,
            tokens,
            "references",
            response::references_refusal_floor,
            move |engines, _control, _budget| {
                let outcome = engines[root]
                    .as_ref()
                    .expect("a read-serving root holds an engine")
                    .references(&request)?;
                Ok(MultiOutcome {
                    served: outcome,
                    root_facts: root_facts(engines)?,
                })
            },
            move |outcome: &MultiOutcome<crate::graph::ReferencesOutcome>,
                  budget: response::Budget,
                  boundary: response::ByteMeasure| {
                let listed: Vec<usize> = (0..pack_meta.len()).collect();
                let headers = root_headers(&pack_meta, &listed, &outcome.root_facts);
                response::pack_references_roots(&outcome.served, &headers, budget, boundary)
            },
        )
        .await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for FoundryMcp {
    /// Foreground work bounds document batches. Status observation uses the
    /// ordinary request slot but does not renew the 60-second batch throttle.
    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::CallToolResponse, ErrorData> {
        #[cfg(feature = "semantic")]
        if request.name != "status" {
            self.state.preparation.foreground();
        }
        if request.name != "status"
            && let Some(knowledge) = &self.state.knowledge
        {
            knowledge.foreground();
        }
        let tcc = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
        self.tool_router.call(tcc).await
    }

    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(INIT_INSTRUCTIONS)
    }

    /// Pin both transports to the session-bearing protocol shape through
    /// 2025-11-25; the stateless session-less 2026-07-28 shape is refused
    /// (session caps, connection-local allowances and DELETE-based
    /// cancellation all depend on sessions).
    fn supported_protocol_versions(&self) -> std::borrow::Cow<'static, [ProtocolVersion]> {
        std::borrow::Cow::Borrowed(ProtocolVersion::known_up_to(&ProtocolVersion::V_2025_11_25))
    }
}
