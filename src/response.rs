//! One renderer per boundary for the context-v2 text wire, shared by CLI and
//! MCP. The text is identical at both boundaries — CLI stdout and the MCP
//! result's single text block — and is exactly what is counted, with the
//! locked `o200k_base` tokenizer and no character-per-token fallback; nothing
//! is appended after counting. Each boundary supplies the measure of the
//! bytes it emits for a text, so the 256 KiB cap applies to what it emits.
use crate::Strategy;
use crate::error::{FResult, FoundryError};
use crate::store::{
    CandidateBatch, HandleRef, Hit, OutlineOutcome, RankedItem, RenderedForm, RetrieveOutcome,
    SearchOutcome, SourceHandle,
};
use serde::Serialize;
use serde_json::Value;

pub const TOKENIZER: &str = "o200k_base";
pub const BYTE_CAP: usize = 256 * 1024;
/// The largest token budget any request may name.
pub const MAX_BUDGET_TOKENS: usize = 32_768;
const RETRIEVE_PREFIX_CAP: usize = 128 * 1024;

/// Maps compact error JSON text to the exact bytes the boundary emits.
pub type FinalRender<'a> = &'a dyn Fn(&str) -> String;

/// The bytes a boundary emits for one success text: the text itself on CLI
/// stdout, the serialized `CallToolResult` carrying it on MCP.
pub type ByteMeasure<'a> = &'a dyn Fn(&str) -> usize;

/// The CLI boundary's measure: stdout is exactly the text.
pub fn stdout_bytes(text: &str) -> usize {
    text.len()
}

/// Freshness metadata every successful context/retrieve response carries.
#[derive(Clone, Debug, Serialize)]
pub struct Freshness {
    pub workspace_id: String,
    pub source_revision: u64,
    pub scan_state: String,
    pub pending_sources: u64,
    pub indexed_snapshot: String,
}

/// A packed v2 result: `text` is exactly what the boundary carries (CLI
/// stdout, or the MCP text block) and `tokens` is its exact count.
#[derive(Debug)]
pub struct PackedText {
    pub text: String,
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

/// The one serializer for bounded JSON reports and errors.
pub fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
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

/// The retrieve prefix trial lengths: at most 128 KiB of `span` ending on a
/// UTF-8 boundary, then halved (retreating to a boundary) down to the first
/// complete codepoint, at most 19 nonempty trials. An empty span has the
/// single trial 0. Shared by every retrieve renderer.
fn prefix_lengths(span: &str) -> Vec<usize> {
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
    lengths
}

// ---------------------------------------------------------------------------
// context-v2 text wire (001 T004).

/// The bound that produced the effective budget: the header's `budget:` suffix
/// and the refusal label of the adapter economics contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetLimiter {
    /// The request itself, and the CLI; ties report the request.
    Request,
    /// The configured `max_context_tokens` ceiling.
    Ceiling,
    /// The remaining session allowance.
    Session,
}

impl BudgetLimiter {
    pub const ALL: [Self; 3] = [Self::Request, Self::Ceiling, Self::Session];

    /// The refusal vocabulary: `request`, `context_ceiling`, `session_allowance`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Ceiling => "context_ceiling",
            Self::Session => "session_allowance",
        }
    }
}

/// The effective token budget of one v2 response and the bound that set it.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    pub tokens: usize,
    pub limited_by: BudgetLimiter,
}

impl Budget {
    /// A budget set by the request (the CLI, or an unconstrained MCP call).
    pub fn request(tokens: usize) -> Self {
        Self {
            tokens,
            limited_by: BudgetLimiter::Request,
        }
    }
}

/// Header facts that do not depend on packing; `line` adds the budget and counts.
struct HeaderV2<'a> {
    op: &'static str,
    revision: u64,
    scan_state: &'a str,
    pending: u64,
    /// Search and context render `shown:`/`omitted:`; retrieve does not.
    lists: bool,
    /// Hits skipped by the per-file cap (search and context).
    capped: u64,
    stale: u64,
    candidates_full: bool,
    graph: Option<&'a str>,
}

impl HeaderV2<'_> {
    /// Segments in contract order joined by ` · `; optional segments appear
    /// only when not at their default.
    fn line(
        &self,
        budget: usize,
        limited_by: BudgetLimiter,
        shown: usize,
        omitted: usize,
    ) -> String {
        let mut segments = vec![
            format!("foundry {}", self.op),
            format!("r{}", self.revision),
        ];
        if self.scan_state != "complete" {
            segments.push(format!("scan:{}", single_line(self.scan_state)));
        }
        if self.pending > 0 {
            segments.push(format!("pending:{}", self.pending));
        }
        segments.push(match limited_by {
            BudgetLimiter::Request => format!("budget:{budget}"),
            BudgetLimiter::Ceiling => format!("budget:{budget}(ceiling)"),
            BudgetLimiter::Session => format!("budget:{budget}(session)"),
        });
        if self.lists {
            segments.push(format!("shown:{shown}"));
            if omitted > 0 {
                segments.push(format!("omitted:{omitted}"));
            }
        }
        if self.capped > 0 {
            segments.push(format!("capped:{}", self.capped));
        }
        if self.stale > 0 {
            segments.push(format!("stale:{}", self.stale));
        }
        if self.candidates_full {
            segments.push("candidates:full".into());
        }
        if let Some(graph) = self.graph {
            segments.push(format!("graph:{graph}"));
        }
        let mut line = segments.join(" · ");
        line.push('\n');
        line
    }
}

/// Single-line fields (labels, graph text, excerpts): every control character
/// except TAB becomes `?`, so indexed text cannot forge a header, item or fence.
fn single_line(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_control() && c != '\t' {
                '?'
            } else {
                c
            }
        })
        .collect()
}

/// A fenced body: backticks of length max(3, 1 + the longest backtick run that
/// begins a body line after at most three spaces), the language tag as info
/// string, the exact body bytes, one framing LF when the body does not end
/// with LF, then the closing fence.
fn fenced(body: &str, lang: Option<&str>) -> String {
    let longest = body
        .split('\n')
        .map(|line| {
            let indent = line.bytes().take(3).take_while(|&b| b == b' ').count();
            line[indent..].bytes().take_while(|&b| b == b'`').count()
        })
        .max()
        .unwrap_or(0);
    let fence = "`".repeat((longest + 1).max(3));
    let mut out = String::with_capacity(body.len() + 2 * fence.len() + 16);
    out.push_str(&fence);
    out.push_str(lang.unwrap_or(""));
    out.push('\n');
    out.push_str(body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&fence);
    out.push('\n');
    out
}

/// First and last 1-based lines a range touches (the line of `start` and the
/// line of `end - 1`); an empty range touches none.
fn touched_lines(start_line: u64, body: &str) -> Option<(u64, u64)> {
    let last = body.len().checked_sub(1)?;
    let newlines = body.as_bytes()[..last]
        .iter()
        .filter(|&&b| b == b'\n')
        .count() as u64;
    Some((start_line, start_line + newlines))
}

/// `<handle>[ L<a>-<b>][ <label>][ <tag>]` then the fenced body.
fn item_text(
    handle: &str,
    lines: Option<(u64, u64)>,
    label: &str,
    tag: Option<&str>,
    lang: Option<&str>,
    body: &str,
) -> String {
    let mut item = handle.to_owned();
    if let Some((first, last)) = lines {
        item.push_str(&format!(" L{first}-{last}"));
    }
    if !label.is_empty() {
        item.push(' ');
        item.push_str(&single_line(label));
    }
    if let Some(tag) = tag {
        item.push(' ');
        item.push_str(tag);
    }
    item.push('\n');
    item.push_str(&fenced(body, lang));
    item
}

/// A retrieve item: `<handle>[ L<a>-<b>]` then the fenced verbatim body.
fn source_item(handle: &str, start_line: u64, path: &str, body: &str) -> String {
    let lang = crate::syntax::Lang::from_path(path).map(crate::syntax::Lang::tag);
    item_text(
        handle,
        touched_lines(start_line, body),
        "",
        None,
        lang,
        body,
    )
}

/// Every form of a ranked item, rendered in ladder order.
fn ranked_forms(item: &RankedItem) -> Vec<String> {
    let handle = item.handle.as_ref();
    let lines = handle
        .filter(|handle| handle.start < handle.end)
        .map(|_| (item.start_line, item.end_line));
    let lang = item.lang.as_deref();
    let source = |tag: Option<&str>, body: &str| {
        handle.map_or_else(String::new, |handle| {
            item_text(&handle.to_v2(), lines, &item.label, tag, lang, body)
        })
    };
    item.forms
        .iter()
        .map(|form| match form {
            RenderedForm::Verbatim(body) => source(None, body),
            RenderedForm::Signature(body) => source(Some("[signature]"), body),
            RenderedForm::Outline(body) | RenderedForm::OutlineMin(body) => {
                source(Some("[outline]"), body)
            }
            RenderedForm::Line(text) => format!("edge {}\n", single_line(text)),
        })
        .filter(|rendered| !rendered.is_empty())
        .collect()
}

/// The fixed point of the most expensive limiter label's rendering: a budget
/// sufficient under every label.
fn sufficient_under_every_label(
    requested: usize,
    render: &dyn Fn(usize, BudgetLimiter) -> String,
) -> usize {
    let worst = |budget: usize| {
        BudgetLimiter::ALL
            .iter()
            .map(|&limiter| count_tokens(&render(budget, limiter)))
            .max()
            .unwrap_or(0)
    };
    sufficient_budget(requested, &worst)
}

/// `budget_too_small` with a hint sufficient under every limiter label.
fn too_small(requested: usize, render: &dyn Fn(usize, BudgetLimiter) -> String) -> FoundryError {
    FoundryError::BudgetTooSmall {
        minimum_tokens: sufficient_under_every_label(requested, render),
    }
}

/// A sufficient budget for the smallest successful response of `op` when no
/// engine outcome exists: a refused session reservation admits no engine
/// work, yet its `budget_exhausted` must still carry a hint valid under any
/// limiter label. Every variable header segment takes its longest form;
/// retrieve adds one item of one maximal codepoint under `handle`'s path and
/// identities with maximal offsets and lines, plus its `next:` line.
/// Sufficient, not minimal.
pub fn refusal_floor(op: &'static str, handle: Option<&HandleRef>) -> usize {
    let longest = u64::MAX;
    let header = HeaderV2 {
        op,
        revision: longest,
        scan_state: "incomplete",
        pending: longest,
        lists: op != "retrieve",
        capped: longest,
        stale: longest,
        candidates_full: true,
        graph: (op == "context").then_some("graph_unavailable"),
    };
    let item = handle.map_or_else(String::new, |handle| {
        let widest = HandleRef {
            start: longest,
            end: longest,
            ..handle.clone()
        }
        .to_string();
        let mut item = source_item(&widest, longest, &handle.path, "\u{10FFFF}");
        item.push_str("next: ");
        item.push_str(&widest);
        item.push('\n');
        item
    });
    sufficient_under_every_label(1, &|budget, limited_by| {
        let mut text = header.line(budget, limited_by, 0, usize::MAX);
        text.push_str(&item);
        text
    })
}

/// Ladder packing (context-v2 § Ladder packing) over an already ordered list
/// of candidates, each given as its rendered forms in ladder order: include
/// the first form whose complete response (header updated for that
/// inclusion) fits the token budget and the byte cap, else omit and count
/// the candidate; then drop last-added items until the final header fits.
/// If even the header cannot fit, refuse with a sufficient budget.
fn pack(
    items: &[Vec<String>],
    header: &HeaderV2,
    budget: Budget,
    byte_cap: usize,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    let render =
        |included: &[(usize, usize)], omitted: usize, shown: usize, limited_by: BudgetLimiter| {
            let mut text = header.line(shown, limited_by, included.len(), omitted);
            for &(index, form) in included {
                text.push_str(&items[index][form]);
            }
            text
        };
    let fits = |text: &str| boundary(text) <= byte_cap && count_tokens(text) <= budget.tokens;
    let mut included: Vec<(usize, usize)> = Vec::new();
    let mut omitted = 0usize;
    for (index, forms) in items.iter().enumerate() {
        let mut placed = false;
        for form in 0..forms.len() {
            included.push((index, form));
            if fits(&render(
                &included,
                omitted,
                budget.tokens,
                budget.limited_by,
            )) {
                placed = true;
                break;
            }
            included.pop();
        }
        if !placed {
            omitted += 1;
        }
    }
    loop {
        let text = render(&included, omitted, budget.tokens, budget.limited_by);
        if fits(&text) {
            let tokens = count_tokens(&text);
            return Ok(PackedText {
                text,
                tokens,
                omitted,
                truncated: omitted > 0,
            });
        }
        if included.pop().is_none() {
            break;
        }
        omitted += 1;
    }
    Err(too_small(budget.tokens, &|shown, limited_by| {
        render(&[], items.len(), shown, limited_by)
    }))
}

/// v2 retrieve text view: header, one item naming the delivered prefix, and
/// `next: <handle>` while bytes of the requested range remain. The prefix
/// follows the shared halving schedule; `next` starts exactly at its end.
pub fn pack_retrieve(
    out: &RetrieveOutcome,
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    // Spans are validated on UTF-8 boundaries, so the bytes are valid UTF-8.
    let span = std::str::from_utf8(&out.span)
        .map_err(|e| FoundryError::Internal(anyhow::anyhow!("span is not UTF-8: {e}")))?;
    let f = &out.freshness;
    let header = HeaderV2 {
        op: "retrieve",
        revision: f.source_revision,
        scan_state: &f.scan_state,
        pending: f.pending_sources,
        lists: false,
        capped: 0,
        stale: 0,
        candidates_full: false,
        graph: None,
    };
    let render = |length: usize, shown: usize, limited_by: BudgetLimiter| {
        let split = out.requested.start + length as u64;
        let delivered = SourceHandle {
            end: split,
            ..out.requested.clone()
        };
        let mut text = header.line(shown, limited_by, 0, 0);
        text.push_str(&source_item(
            &delivered.to_v2(),
            out.start_line,
            &out.requested.path,
            &span[..length],
        ));
        if length < span.len() {
            let next = SourceHandle {
                start: split,
                ..out.requested.clone()
            };
            text.push_str("next: ");
            text.push_str(&next.to_v2());
            text.push('\n');
        }
        text
    };
    let lengths = prefix_lengths(span);
    for &length in &lengths {
        let text = render(length, budget.tokens, budget.limited_by);
        if boundary(&text) <= BYTE_CAP && count_tokens(&text) <= budget.tokens {
            let tokens = count_tokens(&text);
            return Ok(PackedText {
                text,
                tokens,
                omitted: 0,
                truncated: length < span.len(),
            });
        }
    }
    let smallest = lengths.last().copied().unwrap_or(0);
    Err(too_small(budget.tokens, &|shown, limited_by| {
        render(smallest, shown, limited_by)
    }))
}

/// `view:"outline"`: the requested range in the `outline` form, else
/// `outline-min`; never paginated, never `next`. If neither fits, refuse with
/// a budget sufficient for `outline-min`; when no budget can deliver even
/// `outline-min` (it exceeds the largest budget or the byte cap under some
/// limiter label), the range is `unsupported_mode` for this view.
pub fn pack_retrieve_outline(
    out: &OutlineOutcome,
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    let f = &out.freshness;
    let header = HeaderV2 {
        op: "retrieve",
        revision: f.source_revision,
        scan_state: &f.scan_state,
        pending: f.pending_sources,
        lists: false,
        capped: 0,
        stale: 0,
        candidates_full: false,
        graph: None,
    };
    let handle = out.requested.to_v2();
    let lines = (out.requested.start < out.requested.end).then_some((out.start_line, out.end_line));
    let render = |body: &str, shown: usize, limited_by: BudgetLimiter| {
        let mut text = header.line(shown, limited_by, 0, 0);
        text.push_str(&item_text(
            &handle,
            lines,
            "",
            Some("[outline]"),
            Some(out.lang),
            body,
        ));
        text
    };
    for body in [&out.outline, &out.outline_min] {
        let text = render(body, budget.tokens, budget.limited_by);
        if boundary(&text) <= BYTE_CAP && count_tokens(&text) <= budget.tokens {
            let tokens = count_tokens(&text);
            return Ok(PackedText {
                text,
                tokens,
                omitted: 0,
                truncated: false,
            });
        }
    }
    // The smallest form must fit the largest budget and the byte cap under
    // every limiter label, else no retry can deliver this outline.
    let undeliverable = || {
        FoundryError::UnsupportedMode(format!(
            "view:\"outline\" of this range cannot fit {MAX_BUDGET_TOKENS} tokens and the 256 KiB result cap at any budget; narrow it with `lines` or use view:\"text\""
        ))
    };
    let deliverable = BudgetLimiter::ALL.iter().all(|&limited_by| {
        let text = render(&out.outline_min, MAX_BUDGET_TOKENS, limited_by);
        boundary(&text) <= BYTE_CAP && count_tokens(&text) <= MAX_BUDGET_TOKENS
    });
    if !deliverable {
        return Err(undeliverable());
    }
    match too_small(budget.tokens, &|shown, limited_by| {
        render(&out.outline_min, shown, limited_by)
    }) {
        FoundryError::BudgetTooSmall { minimum_tokens } if minimum_tokens > MAX_BUDGET_TOKENS => {
            Err(undeliverable())
        }
        refusal => Err(refusal),
    }
}

/// The outcome-free sufficient budget for a `view:"outline"` refusal made
/// before any engine work: outline-min is whole-or-nothing and its size
/// depends on the source, so the hint is the largest budget. Any outline that
/// can be delivered at all fits within it; one that cannot is refused as
/// `unsupported_mode`.
pub fn outline_refusal_floor() -> usize {
    MAX_BUDGET_TOKENS
}

/// v2 context: the batch's candidates in order (the first unit, graph items,
/// the remaining units, file outlines), ladder-packed over their forms.
pub fn pack_context(
    batch: &CandidateBatch,
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    let items: Vec<Vec<String>> = batch.items.iter().map(ranked_forms).collect();
    let f = &batch.freshness;
    let header = HeaderV2 {
        op: "context",
        revision: f.source_revision,
        scan_state: &f.scan_state,
        pending: f.pending_sources,
        lists: true,
        capped: batch.counters.capped,
        stale: batch.counters.stale,
        candidates_full: batch.counters.candidates_full,
        graph: batch.counters.graph,
    };
    pack(&items, &header, budget, BYTE_CAP, boundary)
}

/// v2 search: one locator line per hit, `<handle> L<line> <label>:
/// <excerpt>`, packed in hit order; the best line comes from hit
/// materialization.
pub fn pack_search(
    outcome: &SearchOutcome,
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    let items: Vec<Vec<String>> = outcome
        .hits
        .iter()
        .map(|hit| {
            vec![format!(
                "{} L{} {}: {}\n",
                hit.handle.to_v2(),
                hit.line,
                single_line(&hit.label),
                excerpt(hit)
            )]
        })
        .collect();
    let header = HeaderV2 {
        op: "search",
        revision: outcome.source_revision,
        scan_state: &outcome.scan_state,
        pending: outcome.pending_sources,
        lists: true,
        capped: outcome.capped,
        stale: outcome.stale_candidates,
        candidates_full: outcome.candidate_limit_reached,
        graph: None,
    };
    pack(&items, &header, budget, BYTE_CAP, boundary)
}

const EXCERPT_BYTES: usize = 160;

/// The hit's best line without its LF or CRLF terminator and leading
/// whitespace, cut at a UTF-8 boundary to at most 160 bytes with `…` appended
/// when cut, single-line.
fn excerpt(hit: &Hit) -> String {
    let index = hit.line.saturating_sub(hit.start_line) as usize;
    let line = hit.text.split_inclusive('\n').nth(index).unwrap_or("");
    let line = line
        .strip_suffix('\n')
        .map_or(line, |l| l.strip_suffix('\r').unwrap_or(l))
        .trim_start();
    if line.len() <= EXCERPT_BYTES {
        single_line(line)
    } else {
        let mut cut = EXCERPT_BYTES;
        while !line.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}…", single_line(&line[..cut]))
    }
}
