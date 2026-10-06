//! 013 T002 calibration, decision and evaluation (contract § Fitting,
//! artifacts and evaluation, 176-219). Pure functions over finite logits:
//! no I/O, no model. The core owns all of this; the worker only computes
//! logits.
//!
//! * Temperature: one scalar fitted on calibration rows only by grid search
//!   over `T = k/20`, k = 10..=60, minimizing the mean negative log
//!   likelihood from a stable log-softmax; exact ties take the lower T. An
//!   empty or nonfinite calibration set refuses.
//! * Decision: `p_i = exp((z_i − max z)/T) / Σ_j exp((z_j − max z)/T)`; the
//!   maximum, exact ties to the lexicographically smaller option ID; accept
//!   exactly when the unrounded maximum is at least the threshold.
//! * Evaluation: per-case rows, coverage, accepted accuracy, macro
//!   (per-group) accuracy, fallback-inclusive accuracy against deterministic
//!   routing and an explicitly requested incumbent, paired group
//!   differences, mean NLL and 15-bin ECE with denominators and error
//!   counts, and eligibility against the predeclared selection policy.
//!   Eligibility permits a trial; it is not an improvement claim.
//! * Economics (contract § Fitting, artifacts and evaluation, 2026-10-06):
//!   only when every evaluation row carries task-checker evidence for both
//!   options ([`option_evidence`]), the evidence and delivered tokens of
//!   deterministic routing, the fallback-inclusive route and the label, and
//!   the changed routes that gained or lost evidence. `learning select`
//!   gates on it; label accuracy is not task benefit.
use super::{FeedbackRowV4, SelectionPolicy, fail, strict_json};
use crate::error::FResult;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The temperature grid: `k / 20` for these `k` (0.5..=3.0).
pub const GRID: std::ops::RangeInclusive<u32> = 10..=60;
/// ECE bins: the first is `[0, 1/15]`, then `(lo, hi]`.
pub const ECE_BINS: usize = 15;

/// `k / 20`.
pub fn grid_temperature(k: u32) -> f64 {
    f64::from(k) / 20.0
}

/// Stable log-softmax of a pair at temperature `t`: the larger scaled logit
/// is 0, so nothing overflows and nothing underflows to `-inf` before the
/// log.
pub fn log_softmax(logits: [f32; 2], t: f64) -> [f64; 2] {
    let z = [f64::from(logits[0]), f64::from(logits[1])];
    let max = z[0].max(z[1]);
    let a = (z[0] - max) / t;
    let b = (z[1] - max) / t;
    // One of `a`, `b` is exactly 0; the other is <= 0.
    let lse = a.max(b) + (a.min(b) - a.max(b)).exp().ln_1p();
    [a - lse, b - lse]
}

/// The contract's stable softmax at temperature `t`.
pub fn softmax(logits: [f32; 2], t: f64) -> [f64; 2] {
    let z = [f64::from(logits[0]), f64::from(logits[1])];
    let max = z[0].max(z[1]);
    let e = [((z[0] - max) / t).exp(), ((z[1] - max) / t).exp()];
    let sum = e[0] + e[1];
    [e[0] / sum, e[1] / sum]
}

/// The fitted scalar and the grid it was chosen from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Calibration {
    pub temperature: f64,
    pub mean_nll: f64,
    pub rows: usize,
}

/// Fit one temperature on `rows` (row-order logits, expected index).
/// Refuses (`calibration_failed`) an empty set or any nonfinite logit; the
/// lowest mean NLL wins and an exact tie keeps the lower temperature.
pub fn fit_temperature(rows: &[([f32; 2], usize)]) -> FResult<Calibration> {
    if rows.is_empty() {
        return Err(fail(
            "calibration_failed",
            "the calibration set is empty; there is nothing to fit a temperature on",
        ));
    }
    if rows
        .iter()
        .any(|(z, y)| !z[0].is_finite() || !z[1].is_finite() || *y > 1)
    {
        return Err(fail(
            "calibration_failed",
            "a calibration logit is nonfinite or a label is out of range",
        ));
    }
    let mut best: Option<(f64, f64)> = None;
    for k in GRID {
        let t = grid_temperature(k);
        let nll = mean_nll(rows, t);
        if !nll.is_finite() {
            return Err(fail(
                "calibration_failed",
                format!("the mean negative log likelihood at T={t} is {nll}"),
            ));
        }
        // Ascending T: only a strictly lower NLL replaces, so ties keep the
        // lower temperature.
        if best.is_none_or(|(_, lowest)| nll < lowest) {
            best = Some((t, nll));
        }
    }
    let (temperature, mean_nll) = best.expect("the grid is not empty");
    Ok(Calibration {
        temperature,
        mean_nll,
        rows: rows.len(),
    })
}

fn mean_nll(rows: &[([f32; 2], usize)], t: f64) -> f64 {
    let total: f64 = rows.iter().map(|(z, y)| -log_softmax(*z, t)[*y]).sum();
    total / rows.len() as f64
}

/// A temperature a candidate may carry: one grid value, at least 0.5.
pub fn valid_temperature(t: f64) -> bool {
    t.is_finite() && GRID.into_iter().any(|k| grid_temperature(k) == t)
}

/// One routed decision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decision {
    /// Row-order probabilities.
    pub probabilities: [f64; 2],
    /// Row-order index of the selected option.
    pub selected: usize,
    pub confidence: f64,
    pub accepted: bool,
}

/// The decision rule on a probability vector (row order): the maximum,
/// exact ties to the lexicographically smaller UTF-8 option ID; accepted
/// exactly when the unrounded maximum is at least `threshold`.
pub fn decide(probabilities: [f64; 2], option_ids: [&str; 2], threshold: f64) -> Decision {
    let selected = if probabilities[0] > probabilities[1] {
        0
    } else if probabilities[1] > probabilities[0] {
        1
    } else if option_ids[0] <= option_ids[1] {
        0
    } else {
        1
    };
    let confidence = probabilities[selected];
    Decision {
        probabilities,
        selected,
        confidence,
        accepted: confidence >= threshold,
    }
}

/// What the model produced for one evaluation row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Output {
    /// Finite logits in row order.
    Logits([f32; 2]),
    /// No prediction (an ordinary unavailable model): deterministic routing,
    /// counted as uncovered, eligibility unaffected.
    Unavailable,
    /// Malformed or nonfinite output: deterministic routing, counted as an
    /// error, and the candidate is not eligible.
    Invalid,
}

/// One evaluation row's identity and labels.
#[derive(Clone, Debug)]
pub struct CaseInput {
    pub example_id: String,
    pub group_id: String,
    pub option_ids: [String; 2],
    pub expected: String,
    /// Deterministic routing's choice for this row's query.
    pub baseline: String,
    /// The example's permission identity (consent plus rights assertion) at
    /// preparation, so selection can require it unchanged (013 T003).
    pub permission_sha256: String,
    /// The row's economics evidence in row option order, if it carries any
    /// ([`option_evidence`]).
    pub evidence: Option<[OptionEvidence; 2]>,
}

/// One option's task-checker outcome on an evaluation row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptionEvidence {
    /// The option delivered all of the task's required evidence.
    pub pass: bool,
    /// Tokens the option's response delivered.
    pub tokens: u64,
}

/// A row's economics evidence in row option order (contract § Feedback and
/// permission, 2026-10-06): only a `task_checker` row whose `label_evidence`
/// is one strict JSON object (no duplicate keys) holding, for EACH of the
/// row's option IDs, a member object with a boolean `pass` and an integer
/// `tokens` that fits u64. Other members are ignored. Any other row simply
/// carries none; nothing is refused and training is unaffected.
pub fn option_evidence(row: &FeedbackRowV4) -> Option<[OptionEvidence; 2]> {
    let [first, second] = row.option_ids.as_slice() else {
        return None;
    };
    if row.label_source != "task_checker" {
        return None;
    }
    let value: serde_json::Value = strict_json(
        row.label_evidence.as_bytes(),
        "row_invalid",
        "label evidence",
    )
    .ok()?;
    let object = value.as_object()?;
    let option = |id: &str| {
        let member = object.get(id)?.as_object()?;
        Some(OptionEvidence {
            pass: member.get("pass")?.as_bool()?,
            tokens: member.get("tokens")?.as_u64()?,
        })
    };
    Some([option(first.as_str())?, option(second.as_str())?])
}

/// A reported case: no raw state, only identities, labels and the model's
/// numbers (contract 212-214).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseRow {
    pub example_id: String,
    pub group_id: String,
    pub option_ids: [String; 2],
    pub expected: String,
    pub baseline: String,
    /// The evaluated example's permission identity; the manifest binds the
    /// report, so selection compares the CURRENT row against it.
    pub permission_sha256: String,
    /// Row-order logits; `None` when the model produced none.
    pub logits: Option<[f32; 2]>,
    /// Row-order calibrated probabilities.
    pub probabilities: Option<[f64; 2]>,
    pub selected: Option<String>,
    pub confidence: Option<f64>,
    /// `accepted`, `fallback` (abstained), `unavailable` or `error`.
    pub outcome: String,
    /// The option actually routed: the selection when accepted, otherwise
    /// deterministic routing.
    pub routed: String,
}

/// One comparator's accuracy on the evaluation rows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Comparator {
    pub correct: usize,
    pub rows: usize,
    pub accuracy: f64,
    pub macro_accuracy: f64,
}

/// Per-group accuracy of the candidate (fallback-inclusive) and each
/// comparator, with the paired differences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupPair {
    pub group_id: String,
    pub rows: usize,
    pub candidate: f64,
    pub baseline: f64,
    pub incumbent: Option<f64>,
    /// `candidate − baseline`.
    pub minus_baseline: f64,
    /// `candidate − incumbent`.
    pub minus_incumbent: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EceBin {
    pub lo: f64,
    pub hi: f64,
    pub count: usize,
    pub correct: usize,
    pub confidence_sum: f64,
}

/// Diagnostics on valid predictions (contract 217-219).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostics {
    /// Valid predictions (the NLL and ECE denominator).
    pub valid: usize,
    pub errors: usize,
    pub unavailable: usize,
    pub mean_nll: Option<f64>,
    pub ece: Option<f64>,
    pub bins: Vec<EceBin>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Eligibility {
    pub eligible: bool,
    /// Every failed check, by name; empty when eligible.
    pub reasons: Vec<String>,
}

/// One arm of the economics comparison over the evaluation rows.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arm {
    /// Rows whose arm option passed.
    pub evidence: usize,
    /// The arm options' delivered tokens, summed (checked, never wrapped).
    pub delivered_tokens: u64,
}

impl Arm {
    fn add(&mut self, option: OptionEvidence, arm: &str) -> FResult<()> {
        self.evidence += usize::from(option.pass);
        self.delivered_tokens = self
            .delivered_tokens
            .checked_add(option.tokens)
            .ok_or_else(|| {
                fail(
                    "economics_overflow",
                    format!("the {arm} arm's delivered tokens exceed {}", u64::MAX),
                )
            })?;
        Ok(())
    }
}

/// What routing delivers on the evaluation rows by their task-checker
/// evidence (contract § Fitting, artifacts and evaluation, 2026-10-06).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Economics {
    pub rows: usize,
    /// Deterministic routing's option.
    pub baseline: Arm,
    /// The fallback-inclusive routed option.
    pub routed: Arm,
    /// The labeled option.
    pub oracle: Arm,
    /// Rows routed to another option than deterministic routing's.
    pub changed_routes: usize,
    /// Changed rows whose routed option passes and baseline option does not.
    pub gained: usize,
    /// Changed rows whose baseline option passes and routed option does not.
    pub lost: usize,
}

/// The evaluation report a candidate carries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub temperature: f64,
    pub selection: SelectionPolicy,
    pub rows: usize,
    pub accepted: usize,
    pub accepted_correct: usize,
    pub coverage: f64,
    /// `None` when nothing was accepted (which fails eligibility).
    pub accepted_accuracy: Option<f64>,
    /// Abstention, unavailable and error rows routed deterministically.
    pub fallback_inclusive: Comparator,
    pub baseline: Comparator,
    pub incumbent: Option<Comparator>,
    pub groups: Vec<GroupPair>,
    pub diagnostics: Diagnostics,
    pub eligibility: Eligibility,
    /// `None` unless every evaluation row carries economics evidence; reports
    /// written before 2026-10-06 lack the member and read back as `None`.
    #[serde(default)]
    pub economics: Option<Economics>,
    pub cases: Vec<CaseRow>,
}

/// The incumbent comparator's input: its outputs on the same rows and its
/// own fitted temperature.
pub struct Incumbent<'a> {
    pub outputs: &'a [Output],
    pub temperature: f64,
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

/// Fallback-inclusive routing of one row: the selection when accepted,
/// otherwise deterministic routing.
fn route(case: &CaseInput, output: Output, t: f64, threshold: f64) -> (Option<Decision>, String) {
    match output {
        Output::Logits(logits) => {
            let ids = [case.option_ids[0].as_str(), case.option_ids[1].as_str()];
            let decision = decide(softmax(logits, t), ids, threshold);
            let routed = if decision.accepted {
                case.option_ids[decision.selected].clone()
            } else {
                case.baseline.clone()
            };
            (Some(decision), routed)
        }
        Output::Unavailable | Output::Invalid => (None, case.baseline.clone()),
    }
}

/// Per-row correctness → overall and per-group (macro) accuracy.
fn comparator(cases: &[CaseInput], correct: &[bool]) -> (Comparator, BTreeMap<String, f64>) {
    let mut groups: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for (case, ok) in cases.iter().zip(correct) {
        let entry = groups.entry(case.group_id.as_str()).or_default();
        entry.0 += usize::from(*ok);
        entry.1 += 1;
    }
    let per_group: BTreeMap<String, f64> = groups
        .iter()
        .map(|(group, (ok, n))| ((*group).to_owned(), ratio(*ok, *n)))
        .collect();
    let macro_accuracy = if per_group.is_empty() {
        0.0
    } else {
        per_group.values().sum::<f64>() / per_group.len() as f64
    };
    let hits = correct.iter().filter(|ok| **ok).count();
    (
        Comparator {
            correct: hits,
            rows: cases.len(),
            accuracy: ratio(hits, cases.len()),
            macro_accuracy,
        },
        per_group,
    )
}

/// A row's `[baseline, routed, oracle]` option evidence, if it carries
/// economics evidence for the options it is routed between.
fn arms(case: &CaseInput, routed: &str) -> Option<[OptionEvidence; 3]> {
    let evidence = case.evidence?;
    let of = |option: &str| {
        let index = case.option_ids.iter().position(|id| id == option)?;
        Some(evidence[index])
    };
    Some([
        of(case.baseline.as_str())?,
        of(routed)?,
        of(case.expected.as_str())?,
    ])
}

/// The economics of the reported routes (`rows`, in `cases` order): `None`
/// unless EVERY case carries economics evidence; a token sum past u64 is
/// `economics_overflow`.
fn economics(cases: &[CaseInput], rows: &[CaseRow]) -> FResult<Option<Economics>> {
    if !cases
        .iter()
        .zip(rows)
        .all(|(case, row)| arms(case, &row.routed).is_some())
    {
        return Ok(None);
    }
    let mut economics = Economics {
        rows: cases.len(),
        ..Economics::default()
    };
    for (case, row) in cases.iter().zip(rows) {
        let Some([baseline, routed, oracle]) = arms(case, &row.routed) else {
            unreachable!("every case carries economics evidence");
        };
        economics.baseline.add(baseline, "baseline")?;
        economics.routed.add(routed, "routed")?;
        economics.oracle.add(oracle, "oracle")?;
        if row.routed != case.baseline {
            economics.changed_routes += 1;
            economics.gained += usize::from(routed.pass && !baseline.pass);
            economics.lost += usize::from(baseline.pass && !routed.pass);
        }
    }
    Ok(Some(economics))
}

fn ece_bin(confidence: f64) -> usize {
    (0..ECE_BINS)
        .find(|b| confidence <= (*b + 1) as f64 / ECE_BINS as f64)
        .unwrap_or(ECE_BINS - 1)
}

/// Evaluate the candidate's `outputs` on `cases` (same order) at
/// `temperature` against deterministic routing and, when requested, the
/// incumbent; judge eligibility by `selection`; add the economics when every
/// case carries evidence (`economics_overflow` refuses a token sum past
/// u64).
pub fn evaluate(
    cases: &[CaseInput],
    outputs: &[Output],
    temperature: f64,
    incumbent: Option<Incumbent<'_>>,
    selection: &SelectionPolicy,
) -> FResult<Report> {
    assert_eq!(cases.len(), outputs.len(), "one output per evaluation row");
    let threshold = selection.threshold;
    let mut rows = Vec::with_capacity(cases.len());
    let mut final_correct = Vec::with_capacity(cases.len());
    let mut baseline_correct = Vec::with_capacity(cases.len());
    let (mut accepted, mut accepted_correct, mut errors, mut unavailable) = (0, 0, 0, 0);
    let mut bins: Vec<EceBin> = (0..ECE_BINS)
        .map(|b| EceBin {
            lo: b as f64 / ECE_BINS as f64,
            hi: (b + 1) as f64 / ECE_BINS as f64,
            count: 0,
            correct: 0,
            confidence_sum: 0.0,
        })
        .collect();
    let mut nll_sum = 0.0;
    let mut valid = 0usize;
    for (case, output) in cases.iter().zip(outputs) {
        let (decision, routed) = route(case, *output, temperature, threshold);
        let expected_index = case.option_ids.iter().position(|id| *id == case.expected);
        let outcome = match (output, &decision) {
            (Output::Invalid, _) => {
                errors += 1;
                "error"
            }
            (Output::Unavailable, _) => {
                unavailable += 1;
                "unavailable"
            }
            (Output::Logits(logits), Some(decision)) => {
                valid += 1;
                let right = case.option_ids[decision.selected] == case.expected;
                if let Some(y) = expected_index {
                    nll_sum += -log_softmax(*logits, temperature)[y];
                }
                let bin = &mut bins[ece_bin(decision.confidence)];
                bin.count += 1;
                bin.correct += usize::from(right);
                bin.confidence_sum += decision.confidence;
                if decision.accepted {
                    accepted += 1;
                    accepted_correct += usize::from(right);
                    "accepted"
                } else {
                    "fallback"
                }
            }
            (Output::Logits(_), None) => unreachable!("logits always decide"),
        };
        final_correct.push(routed == case.expected);
        baseline_correct.push(case.baseline == case.expected);
        rows.push(CaseRow {
            example_id: case.example_id.clone(),
            group_id: case.group_id.clone(),
            option_ids: case.option_ids.clone(),
            expected: case.expected.clone(),
            baseline: case.baseline.clone(),
            permission_sha256: case.permission_sha256.clone(),
            logits: match output {
                Output::Logits(z) => Some(*z),
                _ => None,
            },
            probabilities: decision.map(|d| d.probabilities),
            selected: decision.map(|d| case.option_ids[d.selected].clone()),
            confidence: decision.map(|d| d.confidence),
            outcome: outcome.to_owned(),
            routed,
        });
    }
    let (fallback_inclusive, candidate_groups) = comparator(cases, &final_correct);
    let (baseline, baseline_groups) = comparator(cases, &baseline_correct);
    let incumbent = incumbent.map(|incumbent| {
        assert_eq!(incumbent.outputs.len(), cases.len());
        let correct: Vec<bool> = cases
            .iter()
            .zip(incumbent.outputs)
            .map(|(case, output)| {
                route(case, *output, incumbent.temperature, threshold).1 == case.expected
            })
            .collect();
        comparator(cases, &correct)
    });
    let groups: Vec<GroupPair> = candidate_groups
        .iter()
        .map(|(group, candidate)| {
            let base = baseline_groups[group];
            let inc = incumbent.as_ref().map(|(_, per)| per[group]);
            GroupPair {
                group_id: group.clone(),
                rows: cases.iter().filter(|c| c.group_id == *group).count(),
                candidate: *candidate,
                baseline: base,
                incumbent: inc,
                minus_baseline: candidate - base,
                minus_incumbent: inc.map(|i| candidate - i),
            }
        })
        .collect();
    let ece = (valid > 0).then(|| {
        bins.iter()
            .filter(|bin| bin.count > 0)
            .map(|bin| {
                let n = bin.count as f64;
                (n / valid as f64) * (bin.correct as f64 / n - bin.confidence_sum / n).abs()
            })
            .sum()
    });
    let diagnostics = Diagnostics {
        valid,
        errors,
        unavailable,
        mean_nll: (valid > 0).then(|| nll_sum / valid as f64),
        ece,
        bins,
    };
    let coverage = ratio(accepted, cases.len());
    let accepted_accuracy = (accepted > 0).then(|| ratio(accepted_correct, accepted));
    let mut reasons = Vec::new();
    if errors > 0 {
        reasons.push(format!(
            "model_output_invalid: {errors} row(s) had malformed or nonfinite output"
        ));
    }
    if accepted == 0 {
        reasons.push("no_accepted_rows: nothing reached the threshold".to_owned());
    }
    if coverage < selection.coverage_floor {
        reasons.push(format!(
            "coverage {coverage} is below the floor {}",
            selection.coverage_floor
        ));
    }
    if let Some(acc) = accepted_accuracy
        && acc < selection.accepted_accuracy_floor
    {
        reasons.push(format!(
            "accepted accuracy {acc} is below the floor {}",
            selection.accepted_accuracy_floor
        ));
    }
    let drop = selection.max_macro_accuracy_drop;
    let mut comparators = vec![("deterministic routing", &baseline, &baseline_groups)];
    if let Some((inc, per)) = &incumbent {
        comparators.push(("the incumbent", inc, per));
    }
    for (name, other, per_group) in &comparators {
        if other.macro_accuracy - fallback_inclusive.macro_accuracy > drop {
            reasons.push(format!(
                "macro accuracy {} trails {name}'s {} by more than {drop}",
                fallback_inclusive.macro_accuracy, other.macro_accuracy
            ));
        }
        for group in &selection.critical_groups {
            if let (Some(candidate), Some(theirs)) =
                (candidate_groups.get(group), per_group.get(group))
                && theirs - candidate > drop
            {
                reasons.push(format!(
                    "critical group {group}: {candidate} trails {name}'s {theirs} by more than {drop}"
                ));
            }
        }
    }
    let economics = economics(cases, &rows)?;
    Ok(Report {
        temperature,
        selection: selection.clone(),
        rows: cases.len(),
        accepted,
        accepted_correct,
        coverage,
        accepted_accuracy,
        fallback_inclusive,
        baseline,
        incumbent: incumbent.map(|(comparator, _)| comparator),
        groups,
        diagnostics,
        eligibility: Eligibility {
            eligible: reasons.is_empty(),
            reasons,
        },
        economics,
        cases: rows,
    })
}

/// The query part of a composed state (contract § Exact input and
/// identity): everything before the LF that precedes the LAST line equal
/// to `graph: complete` or `graph: partial`. A locator line can never equal
/// either (its label starts with a unit kind, and neither word is one), so
/// the last such line is the coverage line; LFs inside a multiline query
/// are kept. A state without such a line is refused (`state_invalid`).
pub fn project_query(state: &str) -> FResult<&str> {
    let mut found = None;
    let mut line_start = 0;
    for (index, ch) in state.char_indices() {
        if ch == '\n' {
            check_line(state, line_start, index, &mut found);
            line_start = index + 1;
        }
    }
    check_line(state, line_start, state.len(), &mut found);
    found.map(|lf| &state[..lf]).ok_or_else(|| {
        fail(
            "state_invalid",
            "the state has no `graph: complete|partial` line after its query; the \
             deterministic comparator needs the query",
        )
    })
}

fn check_line(state: &str, start: usize, end: usize, found: &mut Option<usize>) {
    let line = &state[start..end];
    if start > 0 && (line == "graph: complete" || line == "graph: partial") {
        *found = Some(start - 1);
    }
}

/// Deterministic routing's choice for a state: 003's query-only rule on
/// the projected query, never on the composed state.
pub fn baseline_choice(state: &str) -> FResult<&'static str> {
    let query = project_query(state)?;
    Ok(match crate::response::strategy_for_query(query) {
        crate::Strategy::Graph => "graph",
        _ => "search",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn the_grid_is_k_over_20_from_half_to_three() {
        let grid: Vec<f64> = GRID.map(grid_temperature).collect();
        assert_eq!(grid.len(), 51);
        assert_eq!(grid[0], 0.5);
        assert_eq!(grid[50], 3.0);
        assert!(valid_temperature(1.05) && valid_temperature(0.5) && valid_temperature(3.0));
        assert!(!valid_temperature(0.45) && !valid_temperature(3.05) && !valid_temperature(1.01));
        assert!(!valid_temperature(f64::NAN));
    }

    #[test]
    fn exact_ties_take_the_lowest_temperature() {
        // Zero logits: every temperature gives log 2 exactly.
        let rows = vec![([0.0f32, 0.0], 0), ([0.0, 0.0], 1)];
        let fit = fit_temperature(&rows).unwrap();
        assert_eq!(fit.temperature, 0.5);
        assert!(close(fit.mean_nll, std::f64::consts::LN_2));
    }

    #[test]
    fn the_fit_reaches_both_grid_edges() {
        // Confidently right: sharper is better, down to the lowest T.
        let right = vec![([4.0f32, -4.0], 0); 3];
        assert_eq!(fit_temperature(&right).unwrap().temperature, 0.5);
        // Confidently wrong: flatter is better, up to the highest T.
        let wrong = vec![([4.0f32, -4.0], 1); 3];
        assert_eq!(fit_temperature(&wrong).unwrap().temperature, 3.0);
        // Mixed: an interior optimum.
        let mixed = vec![([1.0f32, 0.0], 0), ([1.0, 0.0], 0), ([1.0, 0.0], 1)];
        let t = fit_temperature(&mixed).unwrap().temperature;
        assert!(t > 0.5 && t < 3.0, "{t}");
    }

    #[test]
    fn calibration_refuses_an_empty_or_nonfinite_set() {
        assert_eq!(
            fit_temperature(&[]).unwrap_err().code(),
            "calibration_failed"
        );
        assert_eq!(
            fit_temperature(&[([f32::NAN, 0.0], 0)]).unwrap_err().code(),
            "calibration_failed"
        );
        assert_eq!(
            fit_temperature(&[([f32::INFINITY, 0.0], 0)])
                .unwrap_err()
                .code(),
            "calibration_failed"
        );
    }

    #[test]
    fn the_nll_stays_finite_on_extreme_logits() {
        let max = f32::MAX;
        // Right and wrong at the f32 extremes and at ±1e4.
        for (z, y) in [
            ([max, -max], 0),
            ([max, -max], 1),
            ([1e4, -1e4], 1),
            ([-1e4, 1e4], 1),
        ] {
            let lp = log_softmax(z, 0.5);
            assert!(lp[y].is_finite(), "{z:?} {y}: {lp:?}");
            assert!(lp[0] <= 0.0 && lp[1] <= 0.0);
        }
        // A wrong 2e4 margin costs exactly margin / T, not infinity.
        assert!(close(-log_softmax([1e4, -1e4], 0.5)[1], 4e4));
        assert!(fit_temperature(&[([max, -max], 1)]).is_ok());
        let p = softmax([max, -max], 3.0);
        assert_eq!(p, [1.0, 0.0]);
    }

    #[test]
    fn the_threshold_is_compared_unrounded() {
        let ids = ["search", "graph"];
        let at = decide([0.8, 0.2], ids, 0.8);
        assert!(at.accepted);
        assert_eq!(at.selected, 0);
        let below = decide([0.79999, 0.20001], ids, 0.8);
        assert!(!below.accepted);
        // Exact ties go to the lexicographically smaller option ID,
        // whatever the row order.
        assert_eq!(decide([0.5, 0.5], ["search", "graph"], 0.8).selected, 1);
        assert_eq!(decide([0.5, 0.5], ["graph", "search"], 0.8).selected, 0);
    }

    fn case(id: &str, group: &str, expected: &str, baseline: &str) -> CaseInput {
        CaseInput {
            example_id: id.to_owned(),
            group_id: group.to_owned(),
            option_ids: ["search".to_owned(), "graph".to_owned()],
            expected: expected.to_owned(),
            baseline: baseline.to_owned(),
            permission_sha256: "p".repeat(64),
            evidence: None,
        }
    }

    /// A case in group `g` with checker evidence, `(pass, tokens)` for
    /// search and graph.
    fn checked(
        id: &str,
        expected: &str,
        baseline: &str,
        search: (bool, u64),
        graph: (bool, u64),
    ) -> CaseInput {
        let of = |(pass, tokens): (bool, u64)| OptionEvidence { pass, tokens };
        CaseInput {
            evidence: Some([of(search), of(graph)]),
            ..case(id, "g", expected, baseline)
        }
    }

    /// Logits whose T=1 softmax gives `p` to the first option.
    fn logits_for(p: f64) -> Output {
        Output::Logits([(p / (1.0 - p)).ln() as f32, 0.0])
    }

    #[test]
    fn counts_denominators_groups_and_ece_match_hand_computation() {
        let cases = vec![
            case("e1", "g1", "search", "search"),
            case("e2", "g1", "graph", "search"),
            case("e3", "g2", "search", "graph"),
            case("e4", "g2", "graph", "graph"),
            case("e5", "g3", "search", "search"),
        ];
        // e1 accepted right (0.9), e2 accepted wrong (0.95 for search), e3
        // abstains (0.6 search, fallback graph: wrong), e4 error (fallback
        // graph: right), e5 unavailable (fallback search: right).
        let outputs = vec![
            logits_for(0.9),
            logits_for(0.95),
            logits_for(0.6),
            Output::Invalid,
            Output::Unavailable,
        ];
        let selection = SelectionPolicy::default();
        let report = evaluate(&cases, &outputs, 1.0, None, &selection).unwrap();
        assert_eq!(report.rows, 5);
        assert_eq!(report.accepted, 2);
        assert_eq!(report.accepted_correct, 1);
        assert!(close(report.coverage, 0.4));
        assert_eq!(report.accepted_accuracy, Some(0.5));
        assert_eq!(report.diagnostics.valid, 3);
        assert_eq!(report.diagnostics.errors, 1);
        assert_eq!(report.diagnostics.unavailable, 1);
        let outcomes: Vec<&str> = report.cases.iter().map(|c| c.outcome.as_str()).collect();
        assert_eq!(
            outcomes,
            ["accepted", "accepted", "fallback", "error", "unavailable"]
        );
        // Fallback-inclusive: e1 right, e2 wrong, e3 wrong, e4 right, e5 right.
        assert_eq!(report.fallback_inclusive.correct, 3);
        assert!(close(report.fallback_inclusive.accuracy, 0.6));
        // Macro: g1 0.5, g2 0.5, g3 1.0.
        assert!(close(report.fallback_inclusive.macro_accuracy, 2.0 / 3.0));
        // Baseline: e1 right, e2 wrong, e3 wrong, e4 right, e5 right; macro same.
        assert_eq!(report.baseline.correct, 3);
        assert_eq!(report.groups.len(), 3);
        assert!(report.groups.iter().all(|g| close(g.minus_baseline, 0.0)));
        // NLL over the three valid rows at T=1: -ln 0.9, -ln 0.05, -ln 0.6.
        let nll = (-(0.9f64).ln() - (0.05f64).ln() - (0.6f64).ln()) / 3.0;
        assert!((report.diagnostics.mean_nll.unwrap() - nll).abs() < 1e-6);
        // ECE: confidences 0.9 (bin 13, right), 0.95 (bin 14, wrong), 0.6
        // (bin 8, selected search, expected search: right).
        let bins = &report.diagnostics.bins;
        assert_eq!((bins[13].count, bins[13].correct), (1, 1));
        assert_eq!((bins[14].count, bins[14].correct), (1, 0));
        assert_eq!((bins[8].count, bins[8].correct), (1, 1));
        let ece = ((1.0 - 0.9) + 0.95 + (1.0 - 0.6)) / 3.0;
        assert!((report.diagnostics.ece.unwrap() - ece).abs() < 1e-6);
        // The invalid row voids eligibility, as do the floors.
        assert!(!report.eligibility.eligible);
        assert!(
            report
                .eligibility
                .reasons
                .iter()
                .any(|r| r.starts_with("model_output_invalid"))
        );
    }

    #[test]
    fn ece_bin_edges_are_first_closed_then_left_open() {
        assert_eq!(ece_bin(0.0), 0);
        assert_eq!(ece_bin(1.0 / 15.0), 0);
        assert_eq!(ece_bin(1.0 / 15.0 + 1e-12), 1);
        assert_eq!(ece_bin(0.5), 7);
        assert_eq!(ece_bin(1.0), 14);
    }

    #[test]
    fn zero_accepted_rows_fail_and_paired_differences_are_reported() {
        let cases = vec![
            case("e1", "g1", "search", "search"),
            case("e2", "g2", "graph", "graph"),
        ];
        let outputs = vec![logits_for(0.6), logits_for(0.6)];
        let report = evaluate(&cases, &outputs, 1.0, None, &SelectionPolicy::default()).unwrap();
        assert_eq!(report.accepted, 0);
        assert_eq!(report.accepted_accuracy, None);
        assert!(!report.eligibility.eligible);
        assert!(
            report
                .eligibility
                .reasons
                .iter()
                .any(|r| r.starts_with("no_accepted_rows"))
        );
        // An incumbent that is right where the candidate falls back.
        let inc_outputs = vec![logits_for(0.99), Output::Logits([-10.0, 10.0])];
        let report = evaluate(
            &cases,
            &[logits_for(0.99), logits_for(0.99)],
            1.0,
            Some(Incumbent {
                outputs: &inc_outputs,
                temperature: 1.0,
            }),
            &SelectionPolicy::default(),
        )
        .unwrap();
        let g2 = report.groups.iter().find(|g| g.group_id == "g2").unwrap();
        assert_eq!(g2.candidate, 0.0);
        assert_eq!(g2.incumbent, Some(1.0));
        assert_eq!(g2.minus_incumbent, Some(-1.0));
        assert!(!report.eligibility.eligible);
        assert!(
            report
                .eligibility
                .reasons
                .iter()
                .any(|r| r.contains("incumbent"))
        );
    }

    #[test]
    fn an_eligible_candidate_meets_every_floor_and_critical_group() {
        let cases: Vec<CaseInput> = (0..4)
            .map(|i| case(&format!("e{i}"), &format!("g{}", i % 2), "graph", "search"))
            .collect();
        let outputs = vec![Output::Logits([-5.0, 5.0]); 4];
        let selection = SelectionPolicy {
            critical_groups: vec!["g1".to_owned()],
            ..SelectionPolicy::default()
        };
        let report = evaluate(&cases, &outputs, 1.0, None, &selection).unwrap();
        assert!(report.eligibility.eligible, "{:?}", report.eligibility);
        assert_eq!(report.coverage, 1.0);
        assert_eq!(report.baseline.accuracy, 0.0);
        // A critical group that regresses fails eligibility by name.
        let mut regress = outputs.clone();
        regress[1] = Output::Logits([5.0, -5.0]);
        regress[3] = Output::Logits([5.0, -5.0]);
        let cases_right: Vec<CaseInput> = (0..4)
            .map(|i| case(&format!("e{i}"), &format!("g{}", i % 2), "graph", "graph"))
            .collect();
        let report = evaluate(&cases_right, &regress, 1.0, None, &selection).unwrap();
        assert!(
            report
                .eligibility
                .reasons
                .iter()
                .any(|r| r.contains("critical group g1"))
        );
    }

    #[test]
    fn economics_need_evidence_on_every_row_and_count_each_arm() {
        // Threshold 0.8: `logits_for(0.1)` accepts graph at 0.9 and
        // `logits_for(0.6)` abstains.
        let mut cases = vec![
            // Changed to graph, which alone passes: gained.
            checked("e1", "graph", "search", (false, 2000), (true, 1500)),
            // Changed to graph, which fails where search passes: lost.
            checked("e2", "search", "search", (true, 1000), (false, 2048)),
            // Changed to graph and both pass: fewer tokens, neither gained nor lost.
            checked("e3", "graph", "search", (true, 1800), (true, 1200)),
            // Accepted graph agrees with the baseline: unchanged.
            checked("e4", "search", "graph", (true, 900), (true, 1000)),
            // Abstains to the baseline: unchanged although graph would pass.
            checked("e5", "graph", "search", (false, 2048), (true, 700)),
            // An unavailable model routes the baseline: unchanged.
            checked("e6", "search", "graph", (true, 500), (false, 2048)),
        ];
        let outputs = vec![
            logits_for(0.1),
            logits_for(0.1),
            logits_for(0.1),
            logits_for(0.1),
            logits_for(0.6),
            Output::Unavailable,
        ];
        let selection = SelectionPolicy::default();
        let report = evaluate(&cases, &outputs, 1.0, None, &selection).unwrap();
        let routed: Vec<&str> = report.cases.iter().map(|c| c.routed.as_str()).collect();
        assert_eq!(
            routed,
            ["graph", "graph", "graph", "graph", "search", "graph"]
        );
        assert_eq!(
            report.economics,
            Some(Economics {
                rows: 6,
                baseline: Arm {
                    evidence: 3,
                    delivered_tokens: 9896,
                },
                routed: Arm {
                    evidence: 3,
                    delivered_tokens: 9844,
                },
                oracle: Arm {
                    evidence: 6,
                    delivered_tokens: 5800,
                },
                changed_routes: 3,
                gained: 1,
                lost: 1,
            })
        );
        // One row without evidence: no economics at all.
        cases[5].evidence = None;
        let report = evaluate(&cases, &outputs, 1.0, None, &selection).unwrap();
        assert_eq!(report.economics, None);
    }

    #[test]
    fn delivered_tokens_are_a_checked_sum() {
        let mut cases = vec![
            checked("e1", "search", "search", (true, u64::MAX - 1), (true, 0)),
            checked("e2", "search", "search", (true, 1), (true, 0)),
        ];
        let outputs = vec![Output::Unavailable; 2];
        let selection = SelectionPolicy::default();
        let report = evaluate(&cases, &outputs, 1.0, None, &selection).unwrap();
        let economics = report.economics.unwrap();
        assert_eq!(economics.baseline.delivered_tokens, u64::MAX);
        // One token more is a named error, never a wrapped sum.
        cases[1] = checked("e2", "search", "search", (true, 2), (true, 0));
        let error = evaluate(&cases, &outputs, 1.0, None, &selection).unwrap_err();
        assert_eq!(error.code(), "economics_overflow");
        // Without evidence on every row there are no economics to sum.
        cases[0].evidence = None;
        let report = evaluate(&cases, &outputs, 1.0, None, &selection).unwrap();
        assert_eq!(report.economics, None);
    }

    #[test]
    fn a_report_written_before_economics_reads_back_without_them() {
        let cases = vec![checked("e1", "search", "search", (true, 10), (false, 20))];
        let report = evaluate(
            &cases,
            &[logits_for(0.9)],
            1.0,
            None,
            &SelectionPolicy::default(),
        )
        .unwrap();
        assert!(report.economics.is_some());
        let mut value = serde_json::to_value(&report).unwrap();
        let object = value.as_object_mut().unwrap();
        assert!(object.remove("economics").is_some());
        let old = serde_json::to_vec(&value).unwrap();
        let back: Report = strict_json(&old, "candidate_invalid", "evaluation report").unwrap();
        assert_eq!(back.economics, None);
        assert_eq!(
            back,
            Report {
                economics: None,
                ..report
            }
        );
    }

    #[test]
    fn only_task_checker_json_with_both_options_carries_economics_evidence() {
        const ORDER: [&str; 2] = ["search", "graph"];
        let row = |source: &str, evidence: &str, order: [&str; 2]| FeedbackRowV4 {
            task_id: "t".into(),
            task_group_id: "g".into(),
            family: crate::decision_model::FAMILY.into(),
            state: "q\ngraph: complete".into(),
            option_ids: order.map(String::from).to_vec(),
            correct_option_id: "search".into(),
            label_source: source.into(),
            label_evidence: evidence.into(),
            rights_ref: "r".into(),
            allow_training: true,
        };
        let checker = |evidence: &str| option_evidence(&row("task_checker", evidence, ORDER));
        // The 013 labeling run's shape: other members are ignored.
        let labeled = r#"{"checker":"task-checker-v1","revision":60741,"intent":"definition","search":{"pass":true,"tokens":2047,"digest":"aa"},"graph":{"pass":false,"tokens":2035,"digest":"bb"},"required":["library/core/src/char/methods.rs",28433,28437]}"#;
        let search = OptionEvidence {
            pass: true,
            tokens: 2047,
        };
        let graph = OptionEvidence {
            pass: false,
            tokens: 2035,
        };
        assert_eq!(checker(labeled), Some([search, graph]));
        // Row option order, whatever the member order.
        assert_eq!(
            option_evidence(&row("task_checker", labeled, ["graph", "search"])),
            Some([graph, search])
        );
        assert_eq!(option_evidence(&row("operator", labeled, ORDER)), None);
        let max = r#"{"search":{"pass":true,"tokens":18446744073709551615},"graph":{"pass":false,"tokens":0}}"#;
        assert_eq!(checker(max).map(|e| e[0].tokens), Some(u64::MAX));
        for evidence in [
            "evidence for t",
            "[]",
            r#"{"search":{"pass":true,"tokens":1}}"#,
            r#"{"search":{"pass":true,"tokens":1},"graph":true}"#,
            r#"{"search":{"pass":true,"tokens":1},"graph":{"pass":true}}"#,
            r#"{"search":{"pass":true,"tokens":1},"graph":{"pass":"true","tokens":1}}"#,
            r#"{"search":{"pass":true,"tokens":1},"graph":{"pass":true,"tokens":-1}}"#,
            r#"{"search":{"pass":true,"tokens":1},"graph":{"pass":true,"tokens":1.5}}"#,
            r#"{"search":{"pass":true,"tokens":1},"graph":{"pass":true,"tokens":18446744073709551616}}"#,
            r#"{"search":{"pass":true,"tokens":1},"graph":{"pass":true,"tokens":1},"graph":{"pass":false,"tokens":1}}"#,
        ] {
            assert_eq!(checker(evidence), None, "{evidence}");
        }
    }

    #[test]
    fn the_query_is_projected_before_the_last_coverage_line() {
        // A locator mentioning graph keywords never reaches the baseline.
        let state = "where is parse_config defined\ngraph: complete\nsrc/calls.rs fn callers_of";
        assert_eq!(
            project_query(state).unwrap(),
            "where is parse_config defined"
        );
        assert_eq!(baseline_choice(state).unwrap(), "search");
        assert_eq!(
            crate::response::strategy_for_query(state),
            crate::Strategy::Graph,
            "routing the whole state would have chosen graph"
        );
        // A multiline query keeps its LFs; an earlier coverage-like line in
        // the query does not end it.
        let state = "find the\ncallers of x\ngraph: partial\ngraph: complete\nlib.rs fn x";
        assert_eq!(
            project_query(state).unwrap(),
            "find the\ncallers of x\ngraph: partial"
        );
        assert_eq!(baseline_choice(state).unwrap(), "graph");
        // No coverage line, or one with nothing before it, is refused.
        for bad in [
            "just a query",
            "graph: complete",
            "q\ngraph: completely",
            "q\r\ngraph: x",
        ] {
            assert_eq!(
                project_query(bad).unwrap_err().code(),
                "state_invalid",
                "{bad:?}"
            );
        }
    }

    /// Every unit kind, through an exhaustive match: a new kind fails to
    /// compile here until it is listed.
    fn all_kinds() -> Vec<crate::syntax::UnitKind> {
        use crate::syntax::UnitKind::*;
        let kinds = vec![
            Fn, Struct, Enum, Union, Trait, Impl, Mod, Macro, Const, Static, Type, Class, Method,
            Interface, Section, Block,
        ];
        for kind in &kinds {
            match kind {
                Fn | Struct | Enum | Union | Trait | Impl | Mod | Macro | Const | Static | Type
                | Class | Method | Interface | Section | Block => {}
            }
        }
        kinds
    }

    #[test]
    fn no_locator_line_can_equal_a_coverage_line() {
        // A locator is `<path> <label>`; the label is a kind, optionally
        // followed by a space and a qualified name. `graph: complete` has
        // exactly one space, so it could only be path `graph:` with a bare
        // kind `complete` (or `partial`), and neither is a kind.
        for kind in all_kinds() {
            assert!(
                !["complete", "partial"].contains(&kind.as_str()),
                "{kind:?}"
            );
        }
    }
}
