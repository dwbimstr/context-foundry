//! Host session usage import (003 adapter-economics § Usage import).
//!
//! `foundry usage import --host omp|codex --session FILE` reads one host's
//! own session record offline: it opens no store and no provider connection.
//! Output is one JSON object of counters only — never content bytes — with
//! `total_tokens = input_tokens + output_tokens`; cached, cache-write and
//! reasoning tokens are subsets and are never added again. Totals use
//! checked u64 arithmetic: an unrepresentable total is the named
//! `usage_overflow` refusal (exit 1), never a wrapped or saturated value.
//! Input is bounded (512 MiB, lines of 32 MiB): excess refuses with the
//! existing `usage_input_too_large` (see `receipts::summarize`), never
//! truncates. Zero usage records is `usage_unavailable` (exit 1).
//!
//! Conservative readings of the recipe text (documented for review):
//! * `omp-v1` `host_version` is `null`: the session lines carry a log-format
//!   `version`, not a host/CLI version.
//! * A present-but-`null` usage category counts as missing (unknown), while
//!   a present non-integer value makes the whole line unparsed.
//! * OMP `input` maps to the `input_tokens` category (with `cacheRead` and
//!   `cacheWrite` added per record but keyed one-to-one on their own
//!   categories); a missing field contributes zero and counts in `missing`.
//! * Tool results attribute through the tool call join only: the
//!   `toolResult` record's own `toolName` is redundant and cannot classify a
//!   Foundry `xd://` device write, so a result whose `toolCallId` joins
//!   nothing counts in `unattributed_results` even when `toolName` is known.
//! * `result_bytes`/`result_o200k_estimate` accumulate per result event over
//!   the UTF-8 bytes of its text parts (OMP `content[].text`, Codex string
//!   `output` or `output[].text`); the estimate uses the same locked o200k
//!   tokenizer as delivery counting and is labeled an estimate because the
//!   host's model may use another tokenizer.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;
use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::adapter_error::{AResult, AdapterError};

/// One host session file is at most 512 MiB; larger input refuses.
pub const SESSION_MAX_BYTES: usize = 512 * 1024 * 1024;
/// One session line is at most 32 MiB; a longer line refuses.
pub const SESSION_MAX_LINE_BYTES: usize = 32 * 1024 * 1024;

/// The `xd://` device prefix whose writes are Foundry tool calls.
const FOUNDRY_DEVICE_PREFIX: &str = "xd://mcp__context_foundry_";

/// The host whose session record is imported (`omp` or `codex`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum UsageHost {
    Omp,
    Codex,
}

impl UsageHost {
    fn as_str(self) -> &'static str {
        match self {
            Self::Omp => "omp",
            Self::Codex => "codex",
        }
    }

    fn recipe(self) -> &'static str {
        match self {
            Self::Omp => "omp-v1",
            Self::Codex => "codex-v1",
        }
    }
}

/// Normalized provider usage counters. `total_tokens` is always
/// `input_tokens + output_tokens`; the other categories are subsets.
#[derive(Debug, Serialize)]
pub struct ProviderUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
}

/// Per-tool counters: call count plus the joined results' text bytes and an
/// o200k estimate of those bytes.
#[derive(Debug, Default, Serialize)]
pub struct ToolCounters {
    pub calls: u64,
    pub result_bytes: u64,
    pub result_o200k_estimate: u64,
}

/// The one JSON object `foundry usage import` prints (counters only).
#[derive(Debug, Serialize)]
pub struct UsageSummary {
    pub v: u64,
    pub host: &'static str,
    pub host_version: Option<String>,
    pub session_sha256: String,
    pub recipe: &'static str,
    pub models: Vec<String>,
    pub assistant_messages: u64,
    pub usage_records: u64,
    pub provider_usage: ProviderUsage,
    /// `<category> -> usage records lacking it`; empty means `complete`.
    pub missing: BTreeMap<String, u64>,
    pub complete: bool,
    pub tools: BTreeMap<String, ToolCounters>,
    pub unattributed_results: u64,
    pub unparsed_lines: u64,
}

impl UsageSummary {
    /// The one-line JSON object of the contract, counters only.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

fn overflow(what: &str) -> AdapterError {
    AdapterError::runtime(
        "usage_overflow",
        format!("the {what} total is not representable as u64; no total is reported"),
    )
}

fn too_large(what: &str, bound: usize) -> AdapterError {
    AdapterError::named(
        "usage_input_too_large",
        format!("usage input {what} exceeds {bound} bytes; refusing rather than truncating"),
    )
}

/// The five normalized usage categories of one usage record; `None` marks a
/// category the record lacks (counted in `missing`, contributing zero).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RecordUsage {
    input: Option<u64>,
    cached: Option<u64>,
    cache_write: Option<u64>,
    output: Option<u64>,
    reasoning: Option<u64>,
}

const CATEGORIES: [&str; 5] = [
    "input_tokens",
    "cached_input_tokens",
    "cache_write_tokens",
    "output_tokens",
    "reasoning_tokens",
];

impl RecordUsage {
    fn category(self, index: usize) -> Option<u64> {
        match index {
            0 => self.input,
            1 => self.cached,
            2 => self.cache_write,
            3 => self.output,
            _ => self.reasoning,
        }
    }

    /// The deduplication signature compared under one entry id.
    fn signature(&self) -> [Option<u64>; 5] {
        [
            self.input,
            self.cached,
            self.cache_write,
            self.output,
            self.reasoning,
        ]
    }
}

/// One nonnegative integer field. `Ok(None)` marks an absent or explicit-null
/// category (unknown); `Err(())` marks a present non-integer value, which
/// makes the whole line unparsed rather than silently zero.
fn count_field(object: &Map<String, Value>, name: &str) -> Result<Option<u64>, ()> {
    match object.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or(()),
    }
}

/// The five categories out of an OMP `usage` object.
fn omp_usage(object: &Map<String, Value>) -> Result<RecordUsage, ()> {
    Ok(RecordUsage {
        input: count_field(object, "input")?,
        cached: count_field(object, "cacheRead")?,
        cache_write: count_field(object, "cacheWrite")?,
        output: count_field(object, "output")?,
        reasoning: count_field(object, "reasoningTokens")?,
    })
}

/// The five categories out of a Codex `total_token_usage` object.
fn codex_usage(object: &Map<String, Value>) -> Result<RecordUsage, ()> {
    Ok(RecordUsage {
        input: count_field(object, "input_tokens")?,
        cached: count_field(object, "cached_input_tokens")?,
        cache_write: count_field(object, "cache_write_input_tokens")?,
        output: count_field(object, "output_tokens")?,
        reasoning: count_field(object, "reasoning_output_tokens")?,
    })
}

/// Accumulated totals over admitted usage records.
#[derive(Default)]
struct Totals {
    records: u64,
    input: u64,
    cached: u64,
    cache_write: u64,
    output: u64,
    reasoning: u64,
    missing: BTreeMap<&'static str, u64>,
}

impl Totals {
    fn checked_add(total: &mut u64, value: u64, what: &str) -> AResult<()> {
        *total = total.checked_add(value).ok_or_else(|| overflow(what))?;
        Ok(())
    }

    /// One admitted usage record. With `input_includes_cache` (codex-v1) the
    /// record's `input_tokens` already includes cached input, so it maps
    /// directly; otherwise (omp-v1) the per-record `input_tokens` is
    /// `input + cacheRead + cacheWrite` with missing parts as zero. Each
    /// lacking category counts once in `missing`.
    fn admit(&mut self, record: &RecordUsage, input_includes_cache: bool) -> AResult<()> {
        self.records += 1;
        // Both per-record additions are checked: a record whose categories
        // do not fit u64 is `usage_overflow`, never a wrapped value that the
        // accumulator below cannot detect.
        let per_record_input = if input_includes_cache {
            record.input.unwrap_or(0)
        } else {
            let input = record.input.unwrap_or(0);
            let input = input
                .checked_add(record.cached.unwrap_or(0))
                .ok_or_else(|| overflow("input token"))?;
            input
                .checked_add(record.cache_write.unwrap_or(0))
                .ok_or_else(|| overflow("input token"))?
        };
        Self::checked_add(&mut self.input, per_record_input, "input token")?;
        Self::checked_add(
            &mut self.cached,
            record.cached.unwrap_or(0),
            "cached input token",
        )?;
        Self::checked_add(
            &mut self.cache_write,
            record.cache_write.unwrap_or(0),
            "cache-write token",
        )?;
        Self::checked_add(&mut self.output, record.output.unwrap_or(0), "output token")?;
        Self::checked_add(
            &mut self.reasoning,
            record.reasoning.unwrap_or(0),
            "reasoning token",
        )?;
        for (index, category) in CATEGORIES.iter().enumerate() {
            if record.category(index).is_none() {
                *self.missing.entry(category).or_default() += 1;
            }
        }
        Ok(())
    }
}

/// One tool call's delivery name: a `write` to `xd://mcp__context_foundry_<op>`
/// is classified once as `foundry.<op>`; ordinary writes stay `write`.
fn classify(name: &str, arguments: Option<&Value>) -> String {
    if name != "write" {
        return name.to_owned();
    }
    arguments
        .and_then(|arguments| arguments.get("path"))
        .and_then(Value::as_str)
        .and_then(|path| path.strip_prefix(FOUNDRY_DEVICE_PREFIX))
        .filter(|op| !op.is_empty())
        .map_or_else(|| name.to_owned(), |op| format!("foundry.{op}"))
}

/// Tool call classification, the id join and joined result counters.
#[derive(Default)]
struct Tools {
    by_name: BTreeMap<String, ToolCounters>,
    calls_by_id: HashMap<String, String>,
    unattributed_results: u64,
}

impl Tools {
    /// Count one call under its classified name and return that name for the
    /// result join.
    fn call(&mut self, name: &str, arguments: Option<&Value>) -> String {
        let classified = classify(name, arguments);
        self.by_name.entry(classified.clone()).or_default().calls += 1;
        classified
    }

    /// One tool result joined to its call by id; an id joining nothing counts
    /// in `unattributed_results`. Byte and estimate sums are checked: the
    /// input bound keeps them far from u64, so a failure is a defect, not a
    /// data case.
    fn result(&mut self, call_id: Option<&str>, text: &str) {
        // The joined name is cloned out of the id map so the counter entry
        // can take `&mut self` afterwards.
        let Some(name) = call_id.and_then(|id| self.calls_by_id.get(id).cloned()) else {
            self.unattributed_results += 1;
            return;
        };
        let counters = self.by_name.entry(name).or_default();
        counters.result_bytes = counters
            .result_bytes
            .checked_add(text.len() as u64)
            .expect("result bytes fit u64 under the input bound");
        let estimate = crate::response::count_tokens(text) as u64;
        counters.result_o200k_estimate = counters
            .result_o200k_estimate
            .checked_add(estimate)
            .expect("token estimates fit u64 under the input bound");
    }
}

/// The concatenated `text` parts of a content array (OMP) or output array
/// (Codex); items without a text part contribute nothing.
fn text_parts(content: Option<&Value>) -> String {
    let Some(items) = content.and_then(Value::as_array) else {
        return String::new();
    };
    let mut text = String::new();
    for item in items {
        if let Some(part) = item.get("text").and_then(Value::as_str) {
            text.push_str(part);
        }
    }
    text
}

/// Working state shared by both recipes.
struct Import {
    totals: Totals,
    tools: Tools,
    models: BTreeSet<String>,
    assistant_messages: u64,
    host_version: Option<String>,
    unparsed_lines: u64,
}

/// One JSONL line as a JSON object, or `None` after counting it unparsed.
fn parse_line(line: &[u8], unparsed: &mut u64) -> Option<Value> {
    let trimmed: &[u8] = line.strip_suffix(b"\r").unwrap_or(line);
    match serde_json::from_slice::<Value>(trimmed) {
        Ok(value) if value.is_object() => Some(value),
        _ => {
            *unparsed += 1;
            None
        }
    }
}

/// A bounded rendering of one session-controlled entry id for error text:
/// its first bytes on a char boundary plus the total UTF-8 length, so the
/// message stays fixed-length however long the id is and `bounded_json`
/// never enters its per-character truncation loop.
fn bounded_entry(id: &str) -> String {
    let mut cut = id.len().min(16);
    while !id.is_char_boundary(cut) {
        cut -= 1;
    }
    if cut == id.len() {
        format!("{id:?} ({cut} bytes)")
    } else {
        format!("{:?}... ({} bytes total)", &id[..cut], id.len())
    }
}

/// The OMP recipe: assistant `usage` records deduplicated by entry id,
/// `role:"toolResult"` entries joined by `toolCallId` to the assistant's
/// tool calls.
fn import_omp(lines: &[&[u8]], out: &mut Import) -> AResult<()> {
    let mut seen: HashMap<String, Option<[Option<u64>; 5]>> = HashMap::new();
    for line in lines {
        let Some(value) = parse_line(line, &mut out.unparsed_lines) else {
            continue;
        };
        let object = value.as_object().expect("parse_line admits objects only");
        if object.get("type").and_then(Value::as_str) != Some("message") {
            // Other entry types (title, session, model_change, custom, ...)
            // are ignored, not unparsed.
            continue;
        }
        let Some(message) = object.get("message").and_then(Value::as_object) else {
            out.unparsed_lines += 1;
            continue;
        };
        // An entry id is optional; lines without one simply count every time.
        let entry = object
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty());
        let Some(role) = message.get("role").and_then(Value::as_str) else {
            out.unparsed_lines += 1;
            continue;
        };
        // An assistant `usage` must be an object when present; `null` or an
        // absent field means the entry carries no usage record. A usage field
        // on another role, or a non-object usage, is a malformed line.
        let usage: Option<RecordUsage> = if role == "assistant" {
            match message.get("usage") {
                None | Some(Value::Null) => None,
                Some(Value::Object(object)) => match omp_usage(object) {
                    Ok(usage) => Some(usage),
                    Err(()) => {
                        out.unparsed_lines += 1;
                        continue;
                    }
                },
                Some(_) => {
                    out.unparsed_lines += 1;
                    continue;
                }
            }
        } else if message.get("usage").is_none() {
            None
        } else {
            out.unparsed_lines += 1;
            continue;
        };
        if let Some(entry) = entry {
            // Lines with an entry id count once per id: identical repeats are
            // deduplicated, differing usage under one id is `usage_conflict`.
            let signature = usage.map(|usage| usage.signature());
            if let Some(previous) = seen.insert(entry.to_owned(), signature) {
                if previous != signature {
                    return Err(AdapterError::runtime(
                        "usage_conflict",
                        format!(
                            "entry id {} repeats with differing usage",
                            bounded_entry(entry)
                        ),
                    ));
                }
                continue;
            }
        }
        match role {
            "assistant" => {
                out.assistant_messages += 1;
                if let Some(model) = message.get("model").and_then(Value::as_str) {
                    out.models.insert(model.to_owned());
                }
                if let Some(content) = message.get("content").and_then(Value::as_array) {
                    for item in content {
                        let Some(item) = item.as_object() else {
                            continue;
                        };
                        if item.get("type").and_then(Value::as_str) != Some("toolCall") {
                            continue;
                        }
                        let Some(name) = item.get("name").and_then(Value::as_str) else {
                            continue;
                        };
                        let arguments = item.get("arguments").filter(|value| value.is_object());
                        let classified = out.tools.call(name, arguments);
                        if let Some(id) = item.get("id").and_then(Value::as_str) {
                            out.tools.calls_by_id.insert(id.to_owned(), classified);
                        }
                    }
                }
                if let Some(usage) = usage {
                    out.totals.admit(&usage, false)?;
                }
            }
            "toolResult" => {
                let call_id = message.get("toolCallId").and_then(Value::as_str);
                let text = text_parts(message.get("content"));
                out.tools.result(call_id, &text);
            }
            _ => {}
        }
    }
    Ok(())
}

/// The Codex recipe: the LAST `event_msg` `token_count` with a non-null
/// `info.total_token_usage` is used once and never summed, because its
/// values are cumulative; tool results are `response_item`
/// `function_call_output`/`custom_tool_call_output` entries joined by
/// `call_id` to `function_call`/`custom_tool_call` names.
fn import_codex(lines: &[&[u8]], out: &mut Import) -> AResult<()> {
    let mut last: Option<RecordUsage> = None;
    for line in lines {
        let Some(value) = parse_line(line, &mut out.unparsed_lines) else {
            continue;
        };
        let object = value.as_object().expect("parse_line admits objects only");
        let Some(payload) = object.get("payload").and_then(Value::as_object) else {
            continue;
        };
        match object.get("type").and_then(Value::as_str) {
            Some("session_meta") => {
                if let Some(version) = payload.get("cli_version").and_then(Value::as_str) {
                    out.host_version = Some(version.to_owned());
                }
            }
            Some("turn_context") => {
                if let Some(model) = payload.get("model").and_then(Value::as_str) {
                    out.models.insert(model.to_owned());
                }
            }
            Some("event_msg") => {
                if payload.get("type").and_then(Value::as_str) != Some("token_count") {
                    continue;
                }
                let Some(info) = payload.get("info").and_then(Value::as_object) else {
                    out.unparsed_lines += 1;
                    continue;
                };
                match info.get("total_token_usage") {
                    None | Some(Value::Null) => {}
                    Some(Value::Object(usage)) => match codex_usage(usage) {
                        Ok(usage) => last = Some(usage),
                        Err(()) => out.unparsed_lines += 1,
                    },
                    Some(_) => out.unparsed_lines += 1,
                }
            }
            Some("response_item") => match payload.get("type").and_then(Value::as_str) {
                Some("message") => {
                    if payload.get("role").and_then(Value::as_str) == Some("assistant") {
                        out.assistant_messages += 1;
                    }
                }
                Some("function_call") | Some("custom_tool_call") => {
                    let Some(name) = payload.get("name").and_then(Value::as_str) else {
                        continue;
                    };
                    let classified = out.tools.call(name, None);
                    if let Some(call_id) = payload.get("call_id").and_then(Value::as_str) {
                        out.tools.calls_by_id.insert(call_id.to_owned(), classified);
                    }
                }
                Some("function_call_output") | Some("custom_tool_call_output") => {
                    let call_id = payload.get("call_id").and_then(Value::as_str);
                    let text = match payload.get("output") {
                        Some(Value::String(text)) => text.clone(),
                        Some(output) => text_parts(Some(output)),
                        None => String::new(),
                    };
                    out.tools.result(call_id, &text);
                }
                _ => {}
            },
            _ => {}
        }
    }
    if let Some(last) = last {
        out.totals.admit(&last, true)?;
    }
    Ok(())
}

/// Import one host session file (offline; no store, no provider).
pub fn import_session(host: UsageHost, path: &Path) -> AResult<UsageSummary> {
    // Refuse an oversized file by its metadata before reading, then keep the
    // bounded reader as the second guard (the file may grow in between).
    let len = std::fs::metadata(path)
        .map_err(|e| AdapterError::runtime("usage_input_unreadable", e.to_string()))
        .and_then(|meta| {
            let len = meta.len();
            if len > SESSION_MAX_BYTES as u64 {
                Err(too_large("file", SESSION_MAX_BYTES))
            } else {
                Ok(len)
            }
        })?;
    let mut bytes = Vec::with_capacity(len.min(16 * 1024 * 1024) as usize);
    std::fs::File::open(path)
        .and_then(|file| {
            file.take(SESSION_MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|e| AdapterError::runtime("usage_input_unreadable", e.to_string()))?;
    if bytes.len() > SESSION_MAX_BYTES {
        return Err(too_large("file", SESSION_MAX_BYTES));
    }
    let session_sha256 = crate::digest(&bytes);
    let mut lines: Vec<&[u8]> = Vec::new();
    for line in bytes.split(|b| *b == b'\n') {
        if line.is_empty() {
            continue;
        }
        if line.len() > SESSION_MAX_LINE_BYTES {
            return Err(too_large("line", SESSION_MAX_LINE_BYTES));
        }
        lines.push(line);
    }
    let mut out = Import {
        totals: Totals::default(),
        tools: Tools::default(),
        models: BTreeSet::new(),
        assistant_messages: 0,
        host_version: None,
        unparsed_lines: 0,
    };
    match host {
        UsageHost::Omp => import_omp(&lines, &mut out)?,
        UsageHost::Codex => import_codex(&lines, &mut out)?,
    }
    if out.totals.records == 0 {
        return Err(AdapterError::runtime(
            "usage_unavailable",
            "the session holds no usage records; no usage is reported",
        ));
    }
    let total = out
        .totals
        .input
        .checked_add(out.totals.output)
        .ok_or_else(|| overflow("total token"))?;
    let complete = out.totals.missing.is_empty();
    let missing: BTreeMap<String, u64> = out
        .totals
        .missing
        .into_iter()
        .map(|(category, records)| (category.to_owned(), records))
        .collect();
    Ok(UsageSummary {
        v: 1,
        host: host.as_str(),
        host_version: out.host_version,
        session_sha256,
        recipe: host.recipe(),
        models: out.models.into_iter().collect(),
        assistant_messages: out.assistant_messages,
        usage_records: out.totals.records,
        provider_usage: ProviderUsage {
            input_tokens: out.totals.input,
            cached_input_tokens: out.totals.cached,
            cache_write_tokens: out.totals.cache_write,
            output_tokens: out.totals.output,
            reasoning_tokens: out.totals.reasoning,
            total_tokens: total,
        },
        missing,
        complete,
        tools: out.tools.by_name,
        unattributed_results: out.tools.unattributed_results,
        unparsed_lines: out.unparsed_lines,
    })
}
