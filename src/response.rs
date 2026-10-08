//! One renderer per boundary for the context-v2 text wire, shared by CLI and
//! MCP. The text is identical at both boundaries — CLI stdout and the MCP
//! result's single text block — and is exactly what is counted, with the
//! locked `o200k_base` tokenizer and no character-per-token fallback; nothing
//! is appended after counting. Each boundary supplies the measure of the
//! bytes it emits for a text, so the 256 KiB cap applies to what it emits.
use crate::error::{FResult, FoundryError};
use crate::graph::ReferencesOutcome;
use crate::store::{
    ANCHOR_LIST, CandidateBatch, DoorGroup, DoorState, HandleRef, OutlineOutcome, RankedItem,
    RenderedForm, RetrieveOutcome, SearchOutcome, SourceHandle,
};
use serde::{Deserialize, Serialize};
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

/// The words that request doors under `auto` (context-v2 § Doors).
const DOOR_WORDS: [&str; 22] = [
    "callers",
    "caller",
    "calls",
    "called",
    "invokes",
    "invoked",
    "invokers",
    "uses",
    "used",
    "usage",
    "usages",
    "references",
    "reference",
    "referenced",
    "dependents",
    "depends",
    "dependency",
    "dependencies",
    "impact",
    "affects",
    "break",
    "breaks",
];

/// The context retrieval strategy: `auto` routes (deterministically, or by
/// the selected 013 policy), `search` and `graph` are explicit.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    Auto,
    Search,
    Graph,
}

impl std::fmt::Display for Strategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Strategy::Auto => "auto",
            Strategy::Search => "search",
            Strategy::Graph => "graph",
        })
    }
}

/// Deterministic auto routing: ASCII-lowercase the query, tokenize maximal
/// runs of ASCII letters, digits or `_`; any whole token among the doors
/// request words selects graph, which builds doors (context-v2 § Doors).
/// Substring matches such as `preferences` or `calls_tracker` do not.
pub fn strategy_for_query(query: &str) -> Strategy {
    let lowered = query.to_ascii_lowercase();
    let mut token = String::new();
    let mut graph = false;
    let flush = |token: &mut String, graph: &mut bool| {
        if DOOR_WORDS.contains(&token.as_str()) {
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
/// One admitted root's header segment (007 § Response header and status):
/// `alias(label) r<rev>` plus ` scan:<state>`/` pending:<n>` when not at
/// their default, or `alias(label) <coverage>` for a root that cannot serve.
#[derive(Clone, Debug)]
pub struct RootHeader {
    pub alias: String,
    pub label: String,
    /// `(source revision, scan state, pending sources)` of that root's own
    /// final read; `None` when the root cannot serve.
    pub serving: Option<(u64, String, u64)>,
    /// The coverage word of a root that cannot serve.
    pub coverage: Option<String>,
}

impl RootHeader {
    fn render(&self) -> String {
        match &self.serving {
            Some((revision, scan_state, pending)) => {
                let mut segment = format!(
                    "{}({}) r{revision}",
                    single_line(&self.alias),
                    single_line(&self.label)
                );
                if scan_state != "complete" {
                    segment.push_str(&format!(" scan:{}", single_line(scan_state)));
                }
                if *pending > 0 {
                    segment.push_str(&format!(" pending:{pending}"));
                }
                segment
            }
            None => format!(
                "{}({}) {}",
                single_line(&self.alias),
                single_line(&self.label),
                self.coverage.as_deref().unwrap_or("unavailable")
            ),
        }
    }
}

/// Header facts that do not depend on packing; `line` adds the budget and counts.
struct HeaderV2<'a> {
    op: &'static str,
    /// The per-root segments of a multi-root owner (007). When present they
    /// replace the single-root `r<rev>`/`scan:`/`pending:` segments; each
    /// root carries its own.
    roots: Option<&'a [RootHeader]>,
    revision: u64,
    scan_state: &'a str,
    pending: u64,
    /// Search and context render `shown:`/`omitted:`; retrieve does not.
    lists: bool,
    /// Hits skipped by the per-file cap (search and context).
    capped: u64,
    stale: u64,
    candidates_full: bool,
    /// The 009 T002 semantic word: `ready`, `partial` or
    /// `fallback:<reason>`; `None` keeps the baseline header byte-for-byte.
    semantic: Option<&'a str>,
    /// The 013 T003 route word: `policy` or `fallback:<reason>`, present
    /// only when a configured policy routed an `auto` context; `None` keeps
    /// the baseline header byte-for-byte.
    route: Option<&'a str>,
    /// `defs:<n>`: an anchored context with an ambiguous anchor, the largest
    /// definition count among its ambiguous anchors (context-v2 § Anchored
    /// context); `None` keeps the header unchanged.
    defs: Option<u64>,
    /// `doors:<state>`: a context that requested doors (context-v2 § Doors);
    /// `None` keeps the header unchanged.
    doors: Option<&'static str>,
    /// The bare `anchored` segment, last: a context packed as the anchored
    /// selection; `false` keeps the header unchanged.
    anchored: bool,
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
        let mut segments = vec![format!("foundry {}", self.op)];
        match self.roots {
            None => {
                segments.push(format!("r{}", self.revision));
                if self.scan_state != "complete" {
                    segments.push(format!("scan:{}", single_line(self.scan_state)));
                }
                if self.pending > 0 {
                    segments.push(format!("pending:{}", self.pending));
                }
            }
            Some(roots) => {
                for root in roots {
                    segments.push(root.render());
                }
            }
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
        if let Some(semantic) = self.semantic {
            segments.push(format!("semantic:{}", single_line(semantic)));
        }
        if let Some(route) = self.route {
            segments.push(format!("route:{}", single_line(route)));
        }
        if let Some(defs) = self.defs {
            segments.push(format!("defs:{defs}"));
        }
        if let Some(doors) = self.doors {
            segments.push(format!("doors:{doors}"));
        }
        if self.anchored {
            segments.push("anchored".into());
        }
        let mut line = segments.join(" · ");
        line.push('\n');
        line
    }
}

/// Single-line fields (labels, graph text, excerpts): every control character
/// except TAB becomes `?`, so indexed text cannot forge a header, item or fence.
pub(crate) fn single_line(text: &str) -> String {
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
    let mut item = item_line(handle, lines, label, tag);
    item.push_str(&fenced(body, lang));
    item
}

/// An item line, `<handle>[ L<a>-<b>][ <label>][ <tag>]` and LF.
fn item_line(handle: &str, lines: Option<(u64, u64)>, label: &str, tag: Option<&str>) -> String {
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

/// The selection tag of a unit dense retrieval placed (009 T004): it sits
/// right after `L<a>-<b>`, before the optional label (context-v2 § Evidence
/// items), so a source-derived label ending in it stays label text.
const SEMANTIC_TAG: &str = "[semantic]";

/// One form of a ranked item, rendered; empty when it cannot render (a
/// source form without a handle).
fn rendered_form(item: &RankedItem, form: &RenderedForm) -> String {
    let handle = item.handle.as_ref();
    let lines = handle
        .filter(|handle| handle.start < handle.end)
        .map(|_| (item.start_line, item.end_line));
    let lang = item.lang.as_deref();
    let label: std::borrow::Cow<'_, str> = match &item.semantic {
        None => std::borrow::Cow::Borrowed(&item.label),
        Some(_) if item.label.is_empty() => std::borrow::Cow::Borrowed(SEMANTIC_TAG),
        Some(_) => std::borrow::Cow::Owned(format!("{SEMANTIC_TAG} {}", item.label)),
    };
    let source = |tag: Option<&str>, body: &str| {
        handle.map_or_else(String::new, |handle| {
            item_text(&handle.to_v2(), lines, &label, tag, lang, body)
        })
    };
    match form {
        RenderedForm::Verbatim(body) => source(None, body),
        RenderedForm::Signature(body) => source(Some("[signature]"), body),
        RenderedForm::Outline(body) | RenderedForm::OutlineMin(body) => {
            source(Some("[outline]"), body)
        }
        RenderedForm::Address => handle.map_or_else(String::new, |handle| {
            item_line(&handle.to_v2(), lines, &label, Some("[address]"))
        }),
    }
}

/// Every form of a ranked item, rendered in ladder order.
fn ranked_forms(item: &RankedItem) -> Vec<String> {
    item.forms
        .iter()
        .map(|form| rendered_form(item, form))
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
    refusal_floor_impl(op, handle, None)
}

/// The multi-root form of [`refusal_floor`]: every admitted root renders its
/// worst-case segment (`alias(label) r<max digits> scan:incomplete
/// pending:<max>`), so the hint stays sufficient for this owner's header.
pub fn refusal_floor_roots(
    op: &'static str,
    handle: Option<&HandleRef>,
    roots: &[(String, String)],
) -> usize {
    let worst: Vec<RootHeader> = roots
        .iter()
        .map(|(alias, label)| RootHeader {
            alias: alias.clone(),
            label: label.clone(),
            serving: Some((u64::MAX, "incomplete".to_owned(), u64::MAX)),
            coverage: None,
        })
        .collect();
    refusal_floor_impl(op, handle, Some(&worst))
}

fn refusal_floor_impl(
    op: &'static str,
    handle: Option<&HandleRef>,
    roots: Option<&[RootHeader]>,
) -> usize {
    let longest = u64::MAX;
    let header = HeaderV2 {
        op,
        roots,
        revision: longest,
        scan_state: "incomplete",
        pending: longest,
        lists: op != "retrieve",
        capped: longest,
        stale: longest,
        candidates_full: true,
        semantic: None,
        route: None,
        // Like `route:`, the hint reserves no room for `defs:`, `doors:` or
        // `anchored`: the worst-case numbers above dominate them.
        defs: None,
        doors: None,
        anchored: false,
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
///
/// `tail` holds already-rendered lines that are NOT ladder candidates (008
/// memory lines): after the ladder, each tail line that fits the remaining
/// budget is appended and one that does not is skipped — source spans keep
/// priority — counted in `shown` but never in `omitted`.
fn pack(
    items: &[Vec<String>],
    tail: &[String],
    header: &HeaderV2,
    budget: Budget,
    byte_cap: usize,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    let render = |included: &[(usize, usize)],
                  tail_kept: &[usize],
                  omitted: usize,
                  at_budget: usize,
                  limited_by: BudgetLimiter| {
        let mut text = header.line(
            at_budget,
            limited_by,
            included.len() + tail_kept.len(),
            omitted,
        );
        for &(index, form) in included {
            text.push_str(&items[index][form]);
        }
        for &index in tail_kept {
            text.push_str(&tail[index]);
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
                &[],
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
    let mut tail_kept: Vec<usize> = Vec::new();
    let base_fits = loop {
        let text = render(
            &included,
            &tail_kept,
            omitted,
            budget.tokens,
            budget.limited_by,
        );
        if fits(&text) {
            break true;
        }
        if included.pop().is_none() {
            break false;
        }
        omitted += 1;
    };
    if !base_fits {
        // Not even the header fits; memory lines cannot rescue it.
        return Err(too_small(budget.tokens, &|shown, limited_by| {
            render(&[], &[], items.len(), shown, limited_by)
        }));
    }
    // 008: fill the remaining budget with memory lines, one fitting line at
    // a time; a line that does not fit is skipped, never traded for a source
    // item and never counted as omitted.
    for index in 0..tail.len() {
        if !tail_kept.contains(&index) {
            let mut trial = tail_kept.clone();
            trial.push(index);
            trial.sort_unstable();
            if fits(&render(
                &included,
                &trial,
                omitted,
                budget.tokens,
                budget.limited_by,
            )) {
                tail_kept = trial;
            }
        }
    }
    let text = render(
        &included,
        &tail_kept,
        omitted,
        budget.tokens,
        budget.limited_by,
    );
    let tokens = count_tokens(&text);
    Ok(PackedText {
        text,
        tokens,
        omitted,
        truncated: omitted > 0,
    })
}

/// v2 retrieve text view: header, one item naming the delivered prefix, and
/// `next: <handle>` while bytes of the requested range remain. The prefix
/// follows the shared halving schedule; `next` starts exactly at its end.
pub fn pack_retrieve(
    out: &RetrieveOutcome,
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    pack_retrieve_impl(out, None, budget, boundary)
}

/// The multi-root form of [`pack_retrieve`]: segment 2 lists every admitted
/// root's revision or coverage (007), so unavailable references stay visible.
pub fn pack_retrieve_roots(
    out: &RetrieveOutcome,
    roots: &[RootHeader],
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    pack_retrieve_impl(out, Some(roots), budget, boundary)
}

fn pack_retrieve_impl(
    out: &RetrieveOutcome,
    roots: Option<&[RootHeader]>,
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    let span = std::str::from_utf8(&out.span)
        .map_err(|e| FoundryError::Internal(anyhow::anyhow!("span is not UTF-8: {e}")))?;
    let f = &out.freshness;
    let header = HeaderV2 {
        op: "retrieve",
        roots,
        revision: f.source_revision,
        scan_state: &f.scan_state,
        pending: f.pending_sources,
        lists: false,
        capped: 0,
        stale: 0,
        candidates_full: false,
        semantic: None,
        route: None,
        defs: None,
        doors: None,
        anchored: false,
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
    pack_retrieve_outline_impl(out, None, budget, boundary)
}

/// The multi-root form of [`pack_retrieve_outline`] (007 per-root segments).
pub fn pack_retrieve_outline_roots(
    out: &OutlineOutcome,
    roots: &[RootHeader],
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    pack_retrieve_outline_impl(out, Some(roots), budget, boundary)
}

fn pack_retrieve_outline_impl(
    out: &OutlineOutcome,
    roots: Option<&[RootHeader]>,
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    let f = &out.freshness;
    let header = HeaderV2 {
        op: "retrieve",
        roots,
        revision: f.source_revision,
        scan_state: &f.scan_state,
        pending: f.pending_sources,
        lists: false,
        capped: 0,
        stale: 0,
        candidates_full: false,
        semantic: None,
        route: None,
        defs: None,
        doors: None,
        anchored: false,
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
/// the remaining units, file outlines), ladder-packed over their forms; an
/// anchored batch packs its anchored selection instead (context-v2
/// § Anchored context). Without memory this is byte-identical to the
/// pre-008 rendering.
pub fn pack_context(
    batch: &CandidateBatch,
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    pack_context_impl(batch, None, &[], budget, boundary)
}

/// [`pack_context`] with 008 memory included: after the ladder places the
/// first fitting source item, validated memory hits fill the remaining
/// budget as compact `mem:` lines. Source spans keep priority; a line that
/// does not fit is skipped.
pub fn pack_context_with_memory(
    batch: &CandidateBatch,
    hits: &[crate::memory::MemoryHit],
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    let tail: Vec<String> = hits.iter().map(memory_line).collect();
    pack_context_impl(batch, None, &tail, budget, boundary)
}

/// The multi-root form of [`pack_context`]: the batch is the 007 merge of
/// several roots' batches (never a merge of packed responses), and segment 2
/// names every listed root's revision or coverage. Memory lines come from
/// the primary root only (008).
pub fn pack_context_roots(
    batch: &CandidateBatch,
    roots: &[RootHeader],
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    pack_context_impl(batch, Some(roots), &[], budget, boundary)
}

/// [`pack_context_roots`] with 008 memory from the primary root.
pub fn pack_context_roots_with_memory(
    batch: &CandidateBatch,
    roots: &[RootHeader],
    hits: &[crate::memory::MemoryHit],
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    let tail: Vec<String> = hits.iter().map(memory_line).collect();
    pack_context_impl(batch, Some(roots), &tail, budget, boundary)
}

/// A resolved anchor lists at most this many of its other definitions as
/// directory lines (context-v2 § Anchored context).
const DIRECTORY_LINES: usize = 8;

/// The context header over a batch's facts; `defs` and `anchored` are the
/// anchored selection's last segments, and `doors:<state>` is present when
/// the context requested doors.
fn context_header<'a>(
    batch: &'a CandidateBatch,
    roots: Option<&'a [RootHeader]>,
    defs: Option<u64>,
    anchored: bool,
) -> HeaderV2<'a> {
    let f = &batch.freshness;
    HeaderV2 {
        op: "context",
        roots,
        revision: f.source_revision,
        scan_state: &f.scan_state,
        pending: f.pending_sources,
        lists: true,
        capped: batch.counters.capped,
        stale: batch.counters.stale,
        candidates_full: batch.counters.candidates_full,
        semantic: batch.semantic.as_deref(),
        route: batch.route.as_deref(),
        defs,
        doors: batch.doors.as_ref().map(|doors| doors.state.as_str()),
        anchored,
    }
}

fn pack_context_impl(
    batch: &CandidateBatch,
    roots: Option<&[RootHeader]>,
    tail: &[String],
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    if batch.anchored() {
        return pack_anchored(batch, roots, tail, budget, boundary);
    }
    let items: Vec<Vec<String>> = batch.items.iter().map(ranked_forms).collect();
    let header = context_header(batch, roots, None, false);
    pack(&items, tail, &header, budget, BYTE_CAP, boundary)
}

/// One entry of the anchored selection: its renderings in ladder order —
/// verbatim, the signature when it differs, then the `[address]` line
/// ([`RenderedForm::Address`]); or a directory line alone — and the one it
/// shows.
struct Slot {
    forms: Vec<String>,
    /// An ambiguous entry's second-pass form: its signature, else verbatim.
    preferred: usize,
    /// Its first-pass form: the `[address]` line, else its last rung;
    /// `None` when nothing renders.
    address: Option<usize>,
    chosen: Option<usize>,
}

impl Slot {
    /// An anchored definition's ladder.
    fn ladder(entry: &RankedItem) -> Self {
        let mut forms: Vec<String> = Vec::with_capacity(entry.forms.len());
        let (mut preferred, mut address) = (0, None);
        for form in &entry.forms {
            let rendered = rendered_form(entry, form);
            if rendered.is_empty() {
                continue;
            }
            match form {
                RenderedForm::Signature(_) => preferred = forms.len(),
                RenderedForm::Address => address = Some(forms.len()),
                _ => {}
            }
            forms.push(rendered);
        }
        Slot {
            address: address.or(forms.len().checked_sub(1)),
            forms,
            preferred,
            chosen: None,
        }
    }

    /// A directory line.
    fn line(line: String) -> Self {
        Slot {
            forms: vec![line],
            preferred: 0,
            address: Some(0),
            chosen: None,
        }
    }
}

/// A tie-group entry's door group (context-v2 § Doors, `doors:each`): one
/// door line per file, the last carrying the group's `⋯ <m> more files`
/// line, so the group fits or is omitted whole.
fn group_slots(group: &DoorGroup) -> Vec<Slot> {
    let mut lines: Vec<String> = group
        .lines
        .iter()
        .map(|door| door_line(door, false))
        .collect();
    if group.more_files > 0
        && let Some(last) = lines.last_mut()
    {
        last.push_str(&format!("⋯ {} more files\n", group.more_files));
    }
    lines.into_iter().map(Slot::line).collect()
}

/// The anchored selection (context-v2 § Anchored context), per anchor in
/// order: a resolved anchor's first definition through the ladder
/// (verbatim, signature when it differs, `[address]`), then at most
/// [`DIRECTORY_LINES`] of its other definitions as directory lines; an
/// ambiguous anchor's first [`ANCHOR_LIST`] definitions in three passes —
/// each takes its `[address]` line in list order (the first that does not
/// fit is omitted with every entry after it), then in list order each is
/// upgraded to its signature (verbatim when it has none), then to verbatim,
/// when the difference fits. Every decision is a trial of the whole response
/// with the header updated for it; an entry is omitted only when not even
/// its last form fits. Under `doors:each`, each of the first anchor's
/// entries with a door group shows it right after the entry: the groups are
/// tried in list order after the address pass (after the ladder of a first
/// definition the final read left resolved) and before any upgrade, each
/// placed whole or omitted with its lines counted, and an omitted entry's
/// group with it. Otherwise the door lines of § Doors, when the context
/// requested doors and they were built: each placed when it fits, then the
/// `⋯ <m> more files` line when it fits (navigation, not an item). Nothing
/// else is packed: every other candidate is counted in `omitted:<n>`.
/// Opt-in 008 memory lines then fill the remaining budget under their
/// existing rule.
fn pack_anchored(
    batch: &CandidateBatch,
    roots: Option<&[RootHeader]>,
    tail: &[String],
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    enum Plan {
        Resolved {
            first: usize,
            group: std::ops::Range<usize>,
            directory: std::ops::Range<usize>,
        },
        /// Each listed entry's slot, then its door group's slots.
        Ambiguous(Vec<(usize, std::ops::Range<usize>)>),
    }
    let doors = batch.doors.as_ref();
    // `doors:each`: the first anchor's entries' own groups.
    let each: &[DoorGroup] = match doors {
        Some(doors) if doors.state == DoorState::Each => &doors.groups,
        _ => &[],
    };
    let mut slots: Vec<Slot> = Vec::new();
    let mut plans: Vec<Plan> = Vec::new();
    let mut selected: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut defs: Option<u64> = None;
    for (index, window) in batch.anchors.iter().enumerate() {
        if window.definitions == 0 {
            continue;
        }
        let resolved = window.resolved();
        if !resolved {
            defs = Some(defs.map_or(window.definitions, |most| most.max(window.definitions)));
        }
        let Some(first) = window.entries.first() else {
            continue;
        };
        let listed = if resolved {
            1 + DIRECTORY_LINES
        } else {
            ANCHOR_LIST
        };
        selected.extend(
            window
                .entries
                .iter()
                .take(listed)
                .filter_map(|entry| entry.handle.as_ref().map(SourceHandle::to_v2)),
        );
        // An entry's slot, then its own door group's slots.
        let entry_slots = |slots: &mut Vec<Slot>, entry: &RankedItem| {
            let at = slots.len();
            slots.push(Slot::ladder(entry));
            let group = slots.len();
            if let Some(doors) = each
                .iter()
                .find(|group| index == 0 && group.target.is_some() && group.target == entry.handle)
            {
                slots.extend(group_slots(doors));
            }
            (at, group..slots.len())
        };
        if resolved {
            let (at, group) = entry_slots(&mut slots, first);
            let directory = slots.len();
            slots.extend(
                window.entries[1..]
                    .iter()
                    .take(DIRECTORY_LINES)
                    .filter_map(locator_line)
                    .map(Slot::line),
            );
            plans.push(Plan::Resolved {
                first: at,
                group,
                directory: directory..slots.len(),
            });
        } else {
            let entries = window
                .entries
                .iter()
                .take(ANCHOR_LIST)
                .map(|entry| entry_slots(&mut slots, entry))
                .collect();
            plans.push(Plan::Ambiguous(entries));
        }
    }
    // The door lines (context-v2 § Doors), after every anchor's entries,
    // unless they are the entries' own groups.
    let doors = doors.filter(|doors| doors.state != DoorState::Each);
    let group = doors.and_then(|doors| doors.groups.first());
    let door_slots = slots.len();
    if let Some(group) = group {
        let approx = doors.is_some_and(|doors| doors.state == DoorState::Approx);
        slots.extend(
            group
                .lines
                .iter()
                .map(|door| Slot::line(door_line(door, approx))),
        );
    }
    let door_slots = door_slots..slots.len();
    let more_files = group
        .filter(|group| group.more_files > 0)
        .map(|group| format!("⋯ {} more files\n", group.more_files));
    // Everything outside the selection is omitted from the start.
    let outside = batch
        .items
        .iter()
        .filter(|item| {
            item.handle
                .as_ref()
                .is_none_or(|handle| !selected.contains(&handle.to_v2()))
        })
        .count();
    let header = context_header(batch, roots, defs, true);
    let render =
        |slots: &[Slot], dropped: usize, more: bool, tail_kept: &[usize], at_budget, limited_by| {
            let shown = slots.iter().filter(|slot| slot.chosen.is_some()).count();
            let mut text = header.line(
                at_budget,
                limited_by,
                shown + tail_kept.len(),
                outside + dropped,
            );
            for slot in slots {
                if let Some(form) = slot.chosen {
                    text.push_str(&slot.forms[form]);
                }
            }
            if more && let Some(line) = &more_files {
                text.push_str(line);
            }
            for &index in tail_kept {
                text.push_str(&tail[index]);
            }
            text
        };
    let fits = |text: &str| boundary(text) <= BYTE_CAP && count_tokens(text) <= budget.tokens;
    let trial = |slots: &[Slot], dropped: usize| {
        fits(&render(
            slots,
            dropped,
            false,
            &[],
            budget.tokens,
            budget.limited_by,
        ))
    };
    // The first of `order`'s forms that fits, else the entry is omitted.
    let place = |slots: &mut [Slot], index: usize, order: &[usize], dropped: &mut usize| {
        for &form in order {
            slots[index].chosen = Some(form);
            if trial(slots, *dropped) {
                return;
            }
        }
        slots[index].chosen = None;
        *dropped += 1;
    };
    // A door group fits whole or is omitted, its lines counted; so is an
    // omitted entry's.
    let place_group =
        |slots: &mut [Slot], entry: usize, group: &std::ops::Range<usize>, dropped: &mut usize| {
            if !group.is_empty() && slots[entry].chosen.is_some() {
                for index in group.clone() {
                    slots[index].chosen = Some(0);
                }
                if trial(slots, *dropped) {
                    return;
                }
                for index in group.clone() {
                    slots[index].chosen = None;
                }
            }
            *dropped += group.len();
        };
    let mut dropped = 0usize;
    for plan in &plans {
        match plan {
            Plan::Resolved {
                first,
                group,
                directory,
            } => {
                let ladder: Vec<usize> = (0..slots[*first].forms.len()).collect();
                place(&mut slots, *first, &ladder, &mut dropped);
                place_group(&mut slots, *first, group, &mut dropped);
                for index in directory.clone() {
                    place(&mut slots, index, &[0], &mut dropped);
                }
            }
            Plan::Ambiguous(entries) => {
                // Pass 1: every listed entry's address line, in list order.
                // The first that does not fit is omitted with every entry
                // after it.
                let before = dropped;
                for &(index, _) in entries {
                    if dropped > before {
                        dropped += 1;
                        continue;
                    }
                    let address = slots[index].address;
                    place(&mut slots, index, address.as_slice(), &mut dropped);
                }
                // `doors:each`: the entries' groups, in list order, before
                // any upgrade.
                for (index, group) in entries {
                    place_group(&mut slots, *index, group, &mut dropped);
                }
                // Pass 2 upgrades to the signature (verbatim when there is
                // none), pass 3 to verbatim: each in list order when the
                // difference fits; one that does not keeps its form.
                for verbatim in [false, true] {
                    for &(index, _) in entries {
                        let Some(previous) = slots[index].chosen else {
                            continue;
                        };
                        let form = if verbatim { 0 } else { slots[index].preferred };
                        if form < previous {
                            slots[index].chosen = Some(form);
                            if !trial(&slots, dropped) {
                                slots[index].chosen = Some(previous);
                            }
                        }
                    }
                }
            }
        }
    }
    for index in door_slots {
        place(&mut slots, index, &[0], &mut dropped);
    }
    // The final header carries the final counts: drop the last shown entry
    // until the whole response fits. A door group goes whole, every line
    // counted; it follows its entry, so an entry goes only after its group.
    let mut group_of: Vec<Option<std::ops::Range<usize>>> = vec![None; slots.len()];
    let mut mark = |group: &std::ops::Range<usize>| {
        for index in group.clone() {
            group_of[index] = Some(group.clone());
        }
    };
    for plan in &plans {
        match plan {
            Plan::Resolved { group, .. } => mark(group),
            Plan::Ambiguous(entries) => entries.iter().for_each(|(_, group)| mark(group)),
        }
    }
    let base_fits = loop {
        if trial(&slots, dropped) {
            break true;
        }
        match slots.iter().rposition(|slot| slot.chosen.is_some()) {
            Some(last) => {
                let shown = group_of[last].clone().unwrap_or(last..last + 1);
                for index in shown.clone() {
                    slots[index].chosen = None;
                }
                dropped += shown.len();
            }
            None => break false,
        }
    };
    if !base_fits {
        return Err(too_small(budget.tokens, &|at_budget, limited_by| {
            header.line(at_budget, limited_by, 0, outside + slots.len())
        }));
    }
    // The `⋯ <m> more files` line follows the door lines when it fits.
    let more = more_files.is_some()
        && fits(&render(
            &slots,
            dropped,
            true,
            &[],
            budget.tokens,
            budget.limited_by,
        ));
    // 008: memory lines fill the remaining budget, as after the ladder.
    let mut tail_kept: Vec<usize> = Vec::new();
    for index in 0..tail.len() {
        tail_kept.push(index);
        if !fits(&render(
            &slots,
            dropped,
            more,
            &tail_kept,
            budget.tokens,
            budget.limited_by,
        )) {
            tail_kept.pop();
        }
    }
    let text = render(
        &slots,
        dropped,
        more,
        &tail_kept,
        budget.tokens,
        budget.limited_by,
    );
    let tokens = count_tokens(&text);
    let omitted = outside + dropped;
    Ok(PackedText {
        text,
        tokens,
        omitted,
        truncated: omitted > 0,
    })
}

/// A source item's search locator line (context-v2 § Search locator lines):
/// an anchored context's directory line. `None` without verbatim bytes.
fn locator_line(item: &RankedItem) -> Option<String> {
    let handle = item.handle.as_ref()?;
    let text = item.forms.iter().find_map(|form| match form {
        RenderedForm::Verbatim(text) => Some(text.as_str()),
        _ => None,
    })?;
    let quote = excerpt(text, item.start_line, item.line);
    Some(locator(handle, item.line, &item.label, &quote))
}

/// v2 search: one locator line per hit, `<handle> L<line> <label>:
/// <excerpt>`, packed in hit order; the best line comes from hit
/// materialization.
pub fn pack_search(
    outcome: &SearchOutcome,
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    pack_search_impl(outcome, None, budget, boundary)
}

/// The multi-root form of [`pack_search`] (007 per-root segments).
pub fn pack_search_roots(
    outcome: &SearchOutcome,
    roots: &[RootHeader],
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    pack_search_impl(outcome, Some(roots), budget, boundary)
}

fn pack_search_impl(
    outcome: &SearchOutcome,
    roots: Option<&[RootHeader]>,
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    let items: Vec<Vec<String>> = outcome
        .hits
        .iter()
        .map(|hit| {
            let quote = excerpt(&hit.text, hit.start_line, hit.line);
            vec![locator(&hit.handle, hit.line, &hit.label, &quote)]
        })
        .collect();
    let header = HeaderV2 {
        op: "search",
        roots,
        revision: outcome.source_revision,
        scan_state: &outcome.scan_state,
        pending: outcome.pending_sources,
        lists: true,
        capped: outcome.capped,
        stale: outcome.stale_candidates,
        candidates_full: outcome.candidate_limit_reached,
        semantic: outcome.semantic.as_deref(),
        route: None,
        defs: None,
        doors: None,
        anchored: false,
    };
    pack(&items, &[], &header, budget, BYTE_CAP, boundary)
}

/// 008 memory search: the v2 header exactly as `search` builds it (from the
/// same final read) except segment 1 names `foundry memory`, then one
/// compact `mem:` line per validated hit, ladder-packed like search's
/// locator lines. No hits → header only.
pub fn pack_memory_search(
    outcome: &crate::memory::MemorySearchOutcome,
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    let items: Vec<Vec<String>> = outcome
        .hits
        .iter()
        .map(|hit| vec![memory_line(hit)])
        .collect();
    let f = &outcome.freshness;
    let header = HeaderV2 {
        op: "memory",
        roots: None,
        revision: f.source_revision,
        scan_state: &f.scan_state,
        pending: f.pending_sources,
        lists: true,
        capped: 0,
        stale: outcome.stale_candidates,
        candidates_full: outcome.candidates_full,
        semantic: None,
        route: None,
        defs: None,
        doors: None,
        anchored: false,
    };
    pack(&items, &[], &header, budget, BYTE_CAP, boundary)
}

/// One compact memory line (008): `mem:<id>@r<revision> <author>:
/// <first line>`. The first line is the record text's first line cut at a
/// UTF-8 boundary to at most 120 bytes; author and first line are
/// single-line fields, so stored text cannot forge headers, items or fences.
pub fn memory_line(hit: &crate::memory::MemoryHit) -> String {
    let first = hit.text.split('\n').next().unwrap_or("");
    let first = first.strip_suffix('\r').unwrap_or(first);
    let mut cut = first.len().min(crate::memory::MEM_FIRST_LINE_BYTES);
    while !first.is_char_boundary(cut) {
        cut -= 1;
    }
    format!(
        "mem:{}@r{} {}: {}\n",
        hit.id,
        hit.revision,
        single_line(&hit.author),
        single_line(&first[..cut])
    )
}

const EXCERPT_BYTES: usize = 160;

/// One search locator line (context-v2 § Search locator lines),
/// `<handle> L<best> <label>: <excerpt>`, where `best` is the delivery
/// unit's best line and `quote` its [`excerpt`]. Search hits and an
/// anchored context's directory lines share it.
fn locator(handle: &SourceHandle, best: u64, label: &str, quote: &str) -> String {
    let label = single_line(label);
    format!("{} L{best} {label}: {quote}\n", handle.to_v2())
}

/// One door line (context-v2 § Doors): `<handle> L<line> in <label>:
/// <excerpt>` for a file's first site - the enclosing unit's handle, the
/// site's line and at most 160 bytes of it under the locator excerpt rules -
/// suffixed ` (+<n>)` when the file has `n` more sites and ` [approx]` for an
/// approximate door.
fn door_line(door: &crate::store::DoorLine, approx: bool) -> String {
    let mut line = format!(
        "{} L{} in {}: {}",
        door.unit.to_v2(),
        door.line,
        single_line(&door.label),
        excerpt(&door.text, door.line, door.line)
    );
    if door.more > 0 {
        line.push_str(&format!(" (+{})", door.more));
    }
    if approx {
        line.push_str(" [approx]");
    }
    line.push('\n');
    line
}

/// The best line of a delivery unit's `text`, whose first line is
/// `start_line`, without its LF or CRLF terminator and leading whitespace,
/// cut at a UTF-8 boundary to at most 160 bytes with `…` appended when cut,
/// single-line.
fn excerpt(text: &str, start_line: u64, best: u64) -> String {
    let index = best.saturating_sub(start_line) as usize;
    let line = text.split_inclusive('\n').nth(index).unwrap_or("");
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

/// The `foundry references` header (context-v2 § Header line): segments 1-5,
/// 7, 9 and 10 keep their meanings; 12 `examined`, 13 `unresolved` (only when
/// positive) and 14 `coverage` are references-only. There is no `shown`,
/// `capped` or `graph` segment.
struct ReferencesHeader<'a> {
    /// The per-root segments of a multi-root owner (007); they replace the
    /// single-root `r<rev>`/`scan:`/`pending:` segments.
    roots: Option<&'a [RootHeader]>,
    revision: u64,
    scan_state: &'a str,
    pending: u64,
    stale: usize,
    candidates_full: bool,
    examined: usize,
    unresolved: usize,
    coverage: &'static str,
}

impl ReferencesHeader<'_> {
    fn line(&self, budget: usize, limited_by: BudgetLimiter, omitted: usize) -> String {
        let mut segments = vec!["foundry references".to_owned()];
        match self.roots {
            None => {
                segments.push(format!("r{}", self.revision));
                if self.scan_state != "complete" {
                    segments.push(format!("scan:{}", single_line(self.scan_state)));
                }
                if self.pending > 0 {
                    segments.push(format!("pending:{}", self.pending));
                }
            }
            Some(roots) => {
                for root in roots {
                    segments.push(root.render());
                }
            }
        }
        segments.push(match limited_by {
            BudgetLimiter::Request => format!("budget:{budget}"),
            BudgetLimiter::Ceiling => format!("budget:{budget}(ceiling)"),
            BudgetLimiter::Session => format!("budget:{budget}(session)"),
        });
        if omitted > 0 {
            segments.push(format!("omitted:{omitted}"));
        }
        if self.stale > 0 {
            segments.push(format!("stale:{}", self.stale));
        }
        if self.candidates_full {
            segments.push("candidates:full".into());
        }
        segments.push(format!("examined:{}", self.examined));
        if self.unresolved > 0 {
            segments.push(format!("unresolved:{}", self.unresolved));
        }
        segments.push(format!("coverage:{}", self.coverage));
        let mut line = segments.join(" · ");
        line.push('\n');
        line
    }
}

/// 005 `references` text: the header, one line per reference in
/// `(path, start, end)` order, `<handle> L<line> in <kind> <qualified name>`,
/// then `next: after=<path>#<start>-<end>` while references remain. The
/// delivered lines are always a bounded PREFIX of the outcome's lines: every
/// record is its own page unit, so a prefix may end anywhere (including
/// between references sharing one `(path, start)` — the cursor's `end`
/// disambiguates) and no undelivered record is ever skipped. If lines exist
/// but not even the first fits, the answer is `budget_too_small` with a
/// sufficient budget - never an unchanged continuation.
pub fn pack_references(
    outcome: &ReferencesOutcome,
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    pack_references_impl(outcome, None, budget, boundary)
}

/// The multi-root form of [`pack_references`] (007): the header names every
/// listed root's own revision, or the coverage of a root that cannot serve,
/// exactly as `retrieve` does; the references and the cursor are unchanged.
pub fn pack_references_roots(
    outcome: &ReferencesOutcome,
    roots: &[RootHeader],
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    pack_references_impl(outcome, Some(roots), budget, boundary)
}

fn pack_references_impl(
    outcome: &ReferencesOutcome,
    roots: Option<&[RootHeader]>,
    budget: Budget,
    boundary: ByteMeasure,
) -> FResult<PackedText> {
    let f = &outcome.freshness;
    let header = ReferencesHeader {
        roots,
        revision: f.source_revision,
        scan_state: &f.scan_state,
        pending: f.pending_sources,
        stale: outcome.stale,
        candidates_full: outcome.candidates_full,
        examined: outcome.examined,
        unresolved: outcome.unresolved,
        coverage: outcome.coverage.as_str(),
    };
    let items = &outcome.items;
    let total = items.len();
    let lines: Vec<String> = items
        .iter()
        .map(|item| {
            format!(
                "{} L{} in {}\n",
                item.unit.to_v2(),
                item.line,
                single_line(&item.label)
            )
        })
        .collect();
    let continuation = |n: usize| -> Option<String> {
        match n {
            0 if total > 0 => None,
            n if n < total => Some(items[n - 1].cursor()),
            _ => outcome.more.then(|| outcome.resume.clone()).flatten(),
        }
    };
    let render = |n: usize, at_budget: usize, limited_by: BudgetLimiter| {
        let mut text = header.line(at_budget, limited_by, total - n);
        for line in &lines[..n] {
            text.push_str(line);
        }
        if let Some(cursor) = continuation(n) {
            text.push_str(&format!("next: after={cursor}\n"));
        }
        text
    };
    let fits = |text: &str| boundary(text) <= BYTE_CAP && count_tokens(text) <= budget.tokens;
    let mut included = 0usize;
    for n in 1..=total {
        if !fits(&render(n, budget.tokens, budget.limited_by)) {
            break;
        }
        included = n;
    }
    if included == 0 {
        let hint = total.min(1);
        if total > 0 || !fits(&render(0, budget.tokens, budget.limited_by)) {
            return Err(too_small(budget.tokens, &|b, limited_by| {
                render(hint, b, limited_by)
            }));
        }
    }
    let text = render(included, budget.tokens, budget.limited_by);
    let tokens = count_tokens(&text);
    Ok(PackedText {
        text,
        tokens,
        omitted: total - included,
        truncated: included < total || outcome.more,
    })
}

/// The outcome-free sufficient budget for a `references` refusal made before
/// any engine work (a refused session reservation admits no engine work, yet
/// its `budget_exhausted` must carry a hint valid under every limiter label).
/// The size of the first reference line cannot be bounded below the largest
/// budget: its handle carries a source path of up to 4096 bytes - whose
/// token count depends on the characters - the label is a source-derived
/// qualified name, the `next:` cursor repeats the path, and a multi-root
/// owner's header adds one segment per root. So, like the whole-or-nothing
/// outline view, the hint is the largest budget; a reference that cannot fit
/// even that is refused by the packer itself with its real minimum.
pub fn references_refusal_floor() -> usize {
    MAX_BUDGET_TOKENS
}
