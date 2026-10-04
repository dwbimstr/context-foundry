//! One renderer/packer seam shared by CLI and MCP. Packing decisions are made
//! on the FINAL emitted bytes: the caller supplies the boundary renderer that
//! maps compact application JSON to exactly what its transport emits (the MCP
//! tool-result serializer, or identity for the CLI), and every trial, token
//! budget, byte cap and `budget_too_small` hint is computed on that output.
//! Token counting uses the locked `o200k_base` tokenizer with no
//! character-per-token fallback.
use crate::Strategy;
use crate::error::{FResult, FoundryError};
use crate::store::{ContextOutcome, Evidence, RetrieveOutcome, SearchOutcome, SourceHandle};
use serde::Serialize;
use serde_json::{Value, json};

pub const TOKENIZER: &str = "o200k_base";
pub const BYTE_CAP: usize = 256 * 1024;
const RETRIEVE_PREFIX_CAP: usize = 128 * 1024;

/// Maps compact application JSON text to the exact bytes the boundary emits.
pub type FinalRender<'a> = &'a dyn Fn(&str) -> String;

/// Freshness metadata every successful context/retrieve response carries.
#[derive(Clone, Debug, Serialize)]
pub struct Freshness {
    pub workspace_id: String,
    pub source_revision: u64,
    pub scan_state: String,
    pub pending_sources: u64,
    pub indexed_snapshot: String,
}

/// A packed CLI result: `text` is exactly what stdout carries.
#[derive(Debug)]
pub struct PackedText {
    pub text: String,
    pub tokens: usize,
    pub omitted: usize,
    pub truncated: bool,
}

/// A packed application result. Emit `emitted`, never `application_json`:
/// `emitted == render(application_json)`, `tokens == count_tokens(&emitted)`
/// and `emitted.len() <= byte_cap`.
#[derive(Debug)]
pub struct PackedJson {
    pub application_json: String,
    pub emitted: String,
    pub tokens: usize,
    pub omitted: usize,
    pub truncated: bool,
}

/// Count with `encode_ordinary` from the locked o200k tokenizer.
pub fn count_tokens(text: &str) -> usize {
    tiktoken_rs::o200k_base_singleton()
        .encode_ordinary(text)
        .len()
}

/// The one serializer for counted and emitted application bytes.
pub fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

fn identity(application: &str) -> String {
    application.to_owned()
}

const GRAPH_KEYWORDS: [&str; 11] = [
    "calls",
    "caller",
    "callers",
    "depends",
    "impact",
    "dependency",
    "dependencies",
    "reference",
    "references",
    "usage",
    "usages",
];

/// Deterministic auto routing: ASCII-lowercase the query, tokenize maximal
/// runs of ASCII letters, digits or `_`; any whole keyword token selects
/// graph. Substring matches such as `preferences` or `calls_tracker` do not.
pub fn strategy_for_query(query: &str) -> Strategy {
    let lowered = query.to_ascii_lowercase();
    let mut token = String::new();
    let mut graph = false;
    let flush = |token: &mut String, graph: &mut bool| {
        if GRAPH_KEYWORDS.contains(&token.as_str()) {
            *graph = true;
        }
        token.clear();
    };
    for ch in lowered.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            token.push(ch);
        } else {
            flush(&mut token, &mut graph);
        }
    }
    flush(&mut token, &mut graph);
    if graph {
        Strategy::Graph
    } else {
        Strategy::Search
    }
}

/// Smallest budget whose final rendering fits. The rendered budget field
/// itself contains the budget's digits, so iterate to a fixed point: the
/// returned value `b` satisfies `tokens(render(b)) <= b`.
fn sufficient_budget(requested: usize, tokens_at: &dyn Fn(usize) -> usize) -> usize {
    let mut budget = requested;
    for _ in 0..8 {
        let needed = tokens_at(budget);
        if needed <= budget {
            return budget;
        }
        budget = needed;
    }
    budget
}

struct Fit {
    application: String,
    emitted: String,
    tokens: usize,
    omitted: usize,
}

/// Greedy in-order packing against the final boundary. `render_app` produces
/// the application JSON for the included items, the omission count and the
/// budget value shown in the envelope.
fn pack_ordered<T: Clone>(
    items: &[T],
    budget: usize,
    byte_cap: usize,
    boundary: FinalRender,
    render_app: &dyn Fn(&[T], usize, usize) -> String,
) -> FResult<Fit> {
    let emit = |included: &[T], omitted: usize, shown: usize| -> (String, String) {
        let application = render_app(included, omitted, shown);
        let emitted = boundary(&application);
        (application, emitted)
    };
    let fits = |emitted: &str| emitted.len() <= byte_cap && count_tokens(emitted) <= budget;
    let mut included: Vec<T> = Vec::new();
    let mut omitted = 0usize;
    for item in items {
        let mut trial = included.clone();
        trial.push(item.clone());
        if fits(&emit(&trial, omitted, budget).1) {
            included = trial;
        } else {
            omitted += 1;
        }
    }
    // Omission metadata can change the fit: drop trailing items until the
    // final rendering fits.
    loop {
        let (application, emitted) = emit(&included, omitted, budget);
        if fits(&emitted) {
            let tokens = count_tokens(&emitted);
            return Ok(Fit {
                application,
                emitted,
                tokens,
                omitted,
            });
        }
        if included.pop().is_none() {
            break;
        }
        omitted += 1;
    }
    let minimum = sufficient_budget(budget, &|b| count_tokens(&emit(&[], items.len(), b).1));
    Err(FoundryError::BudgetTooSmall {
        minimum_tokens: minimum,
    })
}

fn graph_label(outcome: &ContextOutcome) -> String {
    match (outcome.strategy, outcome.graph_reason) {
        (Strategy::Graph, Some(reason)) => reason.to_owned(),
        (Strategy::Graph, None) => "ok".to_owned(),
        _ => "not_requested".to_owned(),
    }
}

fn source_citation(path: &str, start_line: u64, end_line: u64, handle: &SourceHandle) -> String {
    format!(
        "{path}:{start_line}-{end_line} [sha256:{}; bytes {}-{}]",
        handle.sha256, handle.start, handle.end
    )
}

fn cli_item(evidence: &Evidence) -> String {
    match evidence {
        Evidence::Source {
            path,
            start_line,
            end_line,
            handle,
            text,
        } => format!(
            "{}\n{text}",
            source_citation(path, *start_line, *end_line, handle)
        ),
        Evidence::Graph { text, .. } => text.clone(),
    }
}

fn application_item(evidence: &Evidence) -> Value {
    match evidence {
        Evidence::Source {
            path,
            start_line,
            end_line,
            handle,
            text,
        } => json!({
            "kind": "source",
            "path": path,
            "start_line": start_line,
            "end_line": end_line,
            "handle": handle,
            "text": text,
        }),
        Evidence::Graph { text, .. } => json!({"kind": "graph", "text": text}),
    }
}

/// CLI context stdout: counted metadata envelope (freshness, strategy, graph
/// coverage, stale/candidate/truncation limits) plus evidence, all inside the
/// budget. Line citations are one-based inclusive; byte ranges are authoritative.
pub fn pack_context_cli(outcome: &ContextOutcome) -> FResult<PackedText> {
    let items: Vec<String> = outcome.candidates.iter().map(cli_item).collect();
    let graph = graph_label(outcome);
    let render = |included: &[String], omitted: usize, shown: usize| -> String {
        let f = &outcome.freshness;
        let mut text = format!(
            "indexed_snapshot: {}; pending_sources: {}\nworkspace_id: {}\ntokenizer: {TOKENIZER}; boundary: cli_stdout; budget: {shown}; budget_satisfied: true\nstrategy: {}; graph: {graph}\nstale_candidates: {}; candidate_limit: {}; candidate_limit_reached: {}; search_truncated: {}; omitted_candidates: {omitted}\n\n",
            f.indexed_snapshot,
            f.pending_sources,
            f.workspace_id,
            outcome.strategy,
            outcome.stale_candidates,
            outcome.candidate_limit,
            outcome.candidate_limit_reached,
            outcome.search_truncated,
        );
        text.push_str(&included.join("\n\n"));
        if !included.is_empty() {
            text.push('\n');
        }
        text
    };
    let fit = pack_ordered(
        &items,
        outcome.requested_tokens,
        BYTE_CAP,
        &identity,
        &render,
    )?;
    Ok(PackedText {
        text: fit.emitted,
        tokens: fit.tokens,
        omitted: fit.omitted,
        truncated: fit.omitted > 0,
    })
}

/// MCP application JSON for context. `adapter_metadata` fields merge into the
/// application envelope before rendering, so they are counted. `render` is the
/// final boundary serializer; `outcome.requested_tokens` is the whole budget.
pub fn pack_context_application(
    outcome: &ContextOutcome,
    adapter_metadata: Option<&Value>,
    render: FinalRender,
    byte_cap: usize,
) -> FResult<PackedJson> {
    let items: Vec<Value> = outcome.candidates.iter().map(application_item).collect();
    let render_app = |included: &[Value], omitted: usize, shown: usize| -> String {
        let f = &outcome.freshness;
        let mut value = json!({
            "format_version": 1,
            "strategy": outcome.strategy,
            "graph_reason": outcome.graph_reason,
            "workspace_id": f.workspace_id,
            "source_revision": f.source_revision,
            "scan_state": f.scan_state,
            "pending_sources": f.pending_sources,
            "indexed_snapshot": f.indexed_snapshot,
            "tokenizer": TOKENIZER,
            "boundary": "mcp_tool_result",
            "requested_budget": shown,
            "budget_satisfied": true,
            "omitted_count": omitted,
            "stale_candidates": outcome.stale_candidates,
            "candidate_limit": outcome.candidate_limit,
            "candidate_limit_reached": outcome.candidate_limit_reached,
            "search_truncated": outcome.search_truncated,
            "evidence": included,
        });
        merge_adapter(&mut value, adapter_metadata);
        compact_json(&value)
    };
    let fit = pack_ordered(
        &items,
        outcome.requested_tokens,
        byte_cap,
        render,
        &render_app,
    )?;
    Ok(PackedJson {
        application_json: fit.application,
        emitted: fit.emitted,
        tokens: fit.tokens,
        omitted: fit.omitted,
        truncated: fit.omitted > 0,
    })
}

fn merge_adapter(value: &mut Value, adapter_metadata: Option<&Value>) {
    if let (Some(extra), Some(object)) = (adapter_metadata, value.as_object_mut()) {
        for (key, field) in extra.as_object().into_iter().flatten() {
            object.insert(key.clone(), field.clone());
        }
    }
}

fn retrieve_next_handle(out: &RetrieveOutcome, delivered: usize) -> Option<SourceHandle> {
    let returned_end = out.requested.start + delivered as u64;
    (returned_end < out.requested.end).then(|| SourceHandle {
        v: 1,
        workspace_id: out.requested.workspace_id.clone(),
        path: out.requested.path.clone(),
        sha256: out.requested.sha256.clone(),
        start: returned_end,
        end: out.requested.end,
    })
}

fn returned_handle(out: &RetrieveOutcome, delivered: usize) -> SourceHandle {
    SourceHandle {
        v: 1,
        workspace_id: out.requested.workspace_id.clone(),
        path: out.requested.path.clone(),
        sha256: out.requested.sha256.clone(),
        start: out.requested.start,
        end: out.requested.start + delivered as u64,
    }
}

/// Renders retrieve application JSON for `(span prefix, continuation handle,
/// delivered byte length, budget shown in the envelope)`.
type RenderPrefix<'a> = &'a dyn Fn(&str, Option<&SourceHandle>, usize, usize) -> String;

/// An accepted retrieve prefix: its application JSON, the exact emitted bytes,
/// their token count, and whether a continuation handle remains.
struct Prefix {
    application: String,
    emitted: String,
    tokens: usize,
    truncated: bool,
}

/// Retrieve prefix fitting. Start with at most 128 KiB of the requested range
/// ending on a UTF-8 boundary; if the fully rendered final result does not fit,
/// halve the byte length (retreating to a boundary) and retry, ending with the
/// first complete codepoint: at most 19 nonempty trials. An empty span has a
/// single trial. When nothing fits, return `budget_too_small` with a budget
/// that is sufficient for the smallest tested result. The accepted trial's
/// `next` handle advances exactly to its delivered end.
fn fit_prefix(
    out: &RetrieveOutcome,
    budget: usize,
    byte_cap: usize,
    boundary: FinalRender,
    render_app: RenderPrefix,
) -> FResult<Prefix> {
    // Spans are validated on UTF-8 boundaries, so the bytes are valid UTF-8.
    let span = std::str::from_utf8(&out.span)
        .map_err(|e| FoundryError::Internal(anyhow::anyhow!("span is not UTF-8: {e}")))?;
    let first = span.chars().next().map_or(0, char::len_utf8);
    let mut lengths: Vec<usize> = Vec::new();
    if span.is_empty() {
        lengths.push(0);
    } else {
        let mut length = span.len().min(RETRIEVE_PREFIX_CAP);
        while !span.is_char_boundary(length) {
            length -= 1;
        }
        lengths.push(length);
        while length > first {
            length = (length / 2).max(first);
            while !span.is_char_boundary(length) {
                length -= 1;
            }
            length = length.max(first);
            if lengths.last() == Some(&length) {
                break;
            }
            lengths.push(length);
        }
    }
    debug_assert!(lengths.len() <= 19);
    let emit = |length: usize, shown: usize| -> (String, String, Option<SourceHandle>) {
        let next = retrieve_next_handle(out, length);
        let application = render_app(&span[..length], next.as_ref(), length, shown);
        let emitted = boundary(&application);
        (application, emitted, next)
    };
    for &length in &lengths {
        let (application, emitted, next) = emit(length, budget);
        if emitted.len() <= byte_cap && count_tokens(&emitted) <= budget {
            let tokens = count_tokens(&emitted);
            return Ok(Prefix {
                application,
                emitted,
                tokens,
                truncated: next.is_some(),
            });
        }
    }
    let smallest = lengths.last().copied().unwrap_or(0);
    let minimum = sufficient_budget(budget, &|b| count_tokens(&emit(smallest, b).1));
    Err(FoundryError::BudgetTooSmall {
        minimum_tokens: minimum,
    })
}

fn freshness_head(f: &Freshness) -> String {
    format!(
        "indexed_snapshot: {}; pending_sources: {}\nworkspace_id: {}\n",
        f.indexed_snapshot, f.pending_sources, f.workspace_id
    )
}

/// CLI retrieve stdout: metadata lines, then the exact span bytes after a
/// `---` separator line as the exact tail of stdout. Source bytes are never
/// rewritten.
pub fn pack_retrieve_cli(out: &RetrieveOutcome) -> FResult<PackedText> {
    let render = |span: &str, next: Option<&SourceHandle>, delivered: usize, shown: usize| {
        let handle = returned_handle(out, delivered);
        format!(
            "{}tokenizer: {TOKENIZER}; boundary: cli_stdout; budget: {shown}; budget_satisfied: true\nhandle: {}\nnext: {}\n---\n{span}",
            freshness_head(&out.freshness),
            handle.to_json(),
            next.map_or_else(|| "null".to_owned(), SourceHandle::to_json)
        )
    };
    let fit = fit_prefix(out, out.requested_tokens, BYTE_CAP, &identity, &render)?;
    Ok(PackedText {
        text: fit.emitted,
        tokens: fit.tokens,
        omitted: 0,
        truncated: fit.truncated,
    })
}

/// MCP application JSON for retrieve (see [`pack_context_application`]).
pub fn pack_retrieve_application(
    out: &RetrieveOutcome,
    adapter_metadata: Option<&Value>,
    render: FinalRender,
    byte_cap: usize,
) -> FResult<PackedJson> {
    let render_app = |span: &str, next: Option<&SourceHandle>, delivered: usize, shown: usize| {
        let f = &out.freshness;
        let mut value = json!({
            "format_version": 1,
            "handle": returned_handle(out, delivered),
            "next": next,
            "text": span,
            "workspace_id": f.workspace_id,
            "source_revision": f.source_revision,
            "scan_state": f.scan_state,
            "pending_sources": f.pending_sources,
            "indexed_snapshot": f.indexed_snapshot,
            "tokenizer": TOKENIZER,
            "boundary": "mcp_tool_result",
            "requested_budget": shown,
            "budget_satisfied": true,
        });
        merge_adapter(&mut value, adapter_metadata);
        compact_json(&value)
    };
    let fit = fit_prefix(out, out.requested_tokens, byte_cap, render, &render_app)?;
    Ok(PackedJson {
        application_json: fit.application,
        emitted: fit.emitted,
        tokens: fit.tokens,
        omitted: 0,
        truncated: fit.truncated,
    })
}

fn search_value(outcome: &SearchOutcome) -> Value {
    json!({
        "format_version": 1,
        "workspace_id": &outcome.workspace_id,
        "source_revision": outcome.source_revision,
        "scan_state": &outcome.scan_state,
        "hits": &outcome.hits,
        "pending_sources": outcome.pending_sources,
        "stale_candidates": outcome.stale_candidates,
        "candidate_limit": outcome.candidate_limit,
        "candidate_limit_reached": outcome.candidate_limit_reached,
        "truncated": outcome.truncated,
    })
}

/// Search output with `format_version:1` and handle fields. The output cap is
/// a byte cap on the FINAL emitted bytes (not a token claim): trailing hits
/// are dropped until the rendering fits and `truncated` is set. An empty
/// search never proves absence outside the examined window.
pub fn search_application(
    outcome: &mut SearchOutcome,
    render: FinalRender,
    byte_cap: usize,
) -> PackedJson {
    loop {
        let application_json = compact_json(&search_value(outcome));
        let emitted = render(&application_json);
        if emitted.len() <= byte_cap || outcome.hits.is_empty() {
            let tokens = count_tokens(&emitted);
            return PackedJson {
                application_json,
                emitted,
                tokens,
                omitted: 0,
                truncated: outcome.truncated,
            };
        }
        outcome.hits.pop();
        outcome.truncated = true;
    }
}

/// CLI search stdout (identity boundary, 256 KiB cap).
pub fn search_json(outcome: &mut SearchOutcome) -> String {
    search_application(outcome, &identity, BYTE_CAP).emitted
}
