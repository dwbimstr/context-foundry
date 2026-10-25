//! Offline, declared-scope task economics. Reuse receipt validation and count
//! failed attempts too. External outcome digests are caller-supplied evidence
//! identities, not checker execution or attestation by Foundry.
use crate::adapter_error::{AResult, AdapterError};
use crate::receipts::{self, Summary};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

const MANIFEST_BYTES: usize = 128 * 1024;
const TOTAL_LOG_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    v: u32,
    attempts: Vec<Attempt>,
    shared: Vec<Shared>,
    /// A projection over this many completed tasks, never an observed saving.
    reuse_completions: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Log {
    receipts: PathBuf,
    expected_requests: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Attempt {
    id: String,
    task: String,
    outcome: Outcome,
    verification_sha256: Option<String>,
    usage: Log,
}
#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Passed,
    Failed,
    Unknown,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Shared {
    phase: Phase,
    usage: Log,
}
#[derive(Clone, Copy, Deserialize, serde::Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Bootstrap,
    Maintenance,
    Retrieval,
    Auxiliary,
}
fn invalid(message: impl Into<String>) -> AdapterError {
    AdapterError::named("economics_invalid", message)
}
fn id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256
}
fn add(left: &mut u64, right: u64) -> AResult<()> {
    *left = left
        .checked_add(right)
        .ok_or_else(|| invalid("cost or token total overflow"))?;
    Ok(())
}

#[derive(Default, serde::Serialize)]
struct Totals {
    input_tokens: u64,
    output_tokens: u64,
    cached_input_tokens: u64,
    cost_microunits: BTreeMap<String, u64>,
}
impl Totals {
    fn absorb(&mut self, summary: &Summary) -> AResult<()> {
        add(&mut self.input_tokens, summary.input_tokens)?;
        add(&mut self.output_tokens, summary.output_tokens)?;
        add(&mut self.cached_input_tokens, summary.cached_input_tokens)?;
        for (currency, cost) in &summary.cost_microunits {
            add(
                self.cost_microunits.entry(currency.clone()).or_default(),
                *cost,
            )?;
        }
        Ok(())
    }
}

/// Read no stores, run no checkers and contact no providers. Relative log paths
/// resolve beside the manifest. Receipt tuples must be disjoint between logs.
pub fn report(path: &Path) -> AResult<Value> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(MANIFEST_BYTES as u64 + 1).read_to_end(&mut bytes))
        .map_err(|e| invalid(e.to_string()))?;
    if bytes.len() > MANIFEST_BYTES {
        return Err(invalid("manifest exceeds 128 KiB"));
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
    if manifest.v != 1
        || manifest.attempts.is_empty()
        || manifest.attempts.len() > 256
        || manifest.shared.len() > 4
        || manifest.reuse_completions == 0
    {
        return Err(invalid(
            "v1 requires 1..256 attempts, at most four shared phases, and positive reuse_completions",
        ));
    }
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let mut ids = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let mut outcomes_known = true;
    for attempt in &manifest.attempts {
        if !id(&attempt.id)
            || !id(&attempt.task)
            || !ids.insert(&attempt.id)
            || attempt
                .verification_sha256
                .as_deref()
                .is_some_and(|s| !crate::is_lower_hex(s, 64))
            || (attempt.outcome != Outcome::Unknown && attempt.verification_sha256.is_none())
        {
            return Err(invalid(
                "attempt IDs must be unique; known outcomes require a verification SHA-256",
            ));
        }
        outcomes_known &= attempt.outcome != Outcome::Unknown;
        if attempt.outcome == Outcome::Passed {
            completed.insert(&attempt.task);
        }
    }
    let mut phases = BTreeSet::new();
    if manifest.shared.iter().any(|s| !phases.insert(s.phase)) {
        return Err(invalid("shared phases must be unique"));
    }
    let mut total_bytes = 0;
    let mut files = BTreeSet::new();
    // Bound the aggregate input before reading any log. The per-file reader also
    // enforces its bound if a file grows after this check.
    for log in manifest
        .attempts
        .iter()
        .map(|a| &a.usage)
        .chain(manifest.shared.iter().map(|s| &s.usage))
    {
        let path = base
            .join(&log.receipts)
            .canonicalize()
            .map_err(|e| invalid(e.to_string()))?;
        let meta = std::fs::metadata(&path).map_err(|e| invalid(e.to_string()))?;
        if !meta.is_file()
            || !files.insert(path)
            || log.expected_requests > receipts::SUMMARIZE_MAX_ROWS as u64
        {
            return Err(invalid(
                "logs must be distinct regular files with bounded request counts",
            ));
        }
        add(&mut total_bytes, meta.len())?;
        if total_bytes > TOTAL_LOG_BYTES {
            return Err(invalid("combined logs exceed 64 MiB"));
        }
    }
    let mut attributed = BTreeSet::new();
    let mut token_complete = true;
    let mut cache_complete = true;
    let mut cost_complete = true;
    let mut rows = Vec::new();
    let mut read_bytes = 0;
    let mut read = |log: &Log, label: Value, totals: &mut Totals| -> AResult<()> {
        let summary = receipts::summarize_disjoint(&base.join(&log.receipts), &mut attributed)?;
        add(&mut read_bytes, summary.input_bytes)?;
        if read_bytes > TOTAL_LOG_BYTES {
            return Err(invalid("combined logs grew beyond 64 MiB"));
        }
        let requests: u64 = summary.outcomes.values().sum();
        let valid = requests == log.expected_requests
            && summary.invalid_rows == 0
            && summary.receipt_conflicts == 0;
        token_complete &= valid
            && !summary.missing.contains_key("input_tokens")
            && !summary.missing.contains_key("output_tokens");
        cache_complete &= valid
            && !summary.missing.contains_key("cached_input_tokens")
            && !summary.missing.contains_key("input_tokens");
        cost_complete &= valid && !summary.missing.contains_key("cost_microunits");
        totals.absorb(&summary)?;
        rows.push(json!({"attribution":label,"expected_requests":log.expected_requests,"request_count_matches":requests==log.expected_requests,"summary":summary}));
        Ok(())
    };
    let mut attempts = Totals::default();
    let mut shared = Totals::default();
    for attempt in &manifest.attempts {
        read(
            &attempt.usage,
            json!({"attempt":attempt.id,"task":attempt.task,"outcome":match attempt.outcome {Outcome::Passed=>"passed",Outcome::Failed=>"failed",Outcome::Unknown=>"unknown"},"verification_sha256":attempt.verification_sha256}),
            &mut attempts,
        )?;
    }
    for phase in &manifest.shared {
        read(&phase.usage, json!({"phase":phase.phase}), &mut shared)?;
    }
    let completions = completed.len() as u64;
    let comparable = outcomes_known && completions > 0;
    let all_cost_currencies: BTreeSet<_> = attempts
        .cost_microunits
        .keys()
        .chain(shared.cost_microunits.keys())
        .collect();
    let mut costs = BTreeMap::new();
    for currency in all_cost_currencies {
        let variable = attempts.cost_microunits.get(currency).copied().unwrap_or(0);
        let fixed = shared.cost_microunits.get(currency).copied().unwrap_or(0);
        let mut total = variable;
        add(&mut total, fixed)?;
        costs.insert(currency, json!({"known_total_microunits":total,
            "observed_per_reported_completion":(comparable && cost_complete).then(|| total as f64 / completions as f64),
            "projected_per_reported_completion":(comparable && cost_complete).then(|| variable as f64 / completions as f64 + fixed as f64 / manifest.reuse_completions as f64)}));
    }
    let mut input = attempts.input_tokens;
    let mut output = attempts.output_tokens;
    let mut cached = attempts.cached_input_tokens;
    add(&mut input, shared.input_tokens)?;
    add(&mut output, shared.output_tokens)?;
    add(&mut cached, shared.cached_input_tokens)?;
    let mut tokens = input;
    add(&mut tokens, output)?;
    Ok(
        json!({"v":1,"manifest_sha256":crate::digest(&bytes),"scope":"declared_logs_only",
        "outcomes":"externally_reported_not_attested","attempts":manifest.attempts.len(),"reported_completed_tasks":completions,
        "failed_attempts":manifest.attempts.iter().filter(|a|a.outcome==Outcome::Failed).count(),
        "unknown_attempts":manifest.attempts.iter().filter(|a|a.outcome==Outcome::Unknown).count(),
        "usage_complete_for_declared_requests":token_complete,"cache_complete_for_declared_requests":cache_complete,
        "cost_complete_for_declared_requests":cost_complete,"reuse_completions":manifest.reuse_completions,
        "projection_assumption":"declared shared costs occur once over reuse_completions; future maintenance and energy are not inferred",
        "attempt_subtotals":attempts,"shared_subtotals":shared,"costs":costs,
        "uncached_input_tokens":cache_complete.then(||input.checked_sub(cached)).flatten(),
        "tokens_per_reported_completion":(comparable && token_complete).then(||tokens as f64 / completions as f64),
        "comparison":"not_performed","logs":rows}),
    )
}
