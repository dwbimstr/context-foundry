//! 013 T003 learned routing (contract `learning-loop.md` § Serving,
//! selection and rollback): config v2, `learning select`, the served policy
//! and its named deterministic fallbacks.
//!
//! * **Config v2** ([`PolicyConfig`]) is `{"v":2,"enabled":false}` or pins
//!   the candidate's path and manifest SHA-256, the model function, the
//!   evaluation report's SHA-256, the threshold the candidate was evaluated
//!   at and the learning isolation profile. It is validated once at owner
//!   startup ([`Policy::start`]), eligibility and the economics gate
//!   included; an invalid config is `policy_config_invalid` and baseline
//!   retrieval still starts. There is no ambient latest directory, no
//!   automatic promotion and no live threshold edit. A `--lifecycle-check`
//!   selection carries `"lifecycle_check": true` (present only as `true`):
//!   startup skips ONLY the economics gate and every `status` of it, served
//!   or not, shows the mark. It exists solely for lifecycle and package
//!   verification (013 T004, D001); it is not enablement and never a shipped
//!   default config.
//! * **Selection** ([`select`]) reads a candidate back under T002's rules,
//!   requires its eligibility, the CURRENT consent of every example it was
//!   fitted, calibrated and evaluated on, and the offline economics gate
//!   (its report's routed option gains required evidence on more evaluation
//!   tasks than it loses: `economics_unknown`, `candidate_no_benefit`;
//!   `--lifecycle-check` overrides that gate alone), and writes a NEW config
//!   by anchored no-replace publication; it never overwrites. The operator
//!   installs it with `--policy-config FILE` and a restart. Rollback is a
//!   restart with the prior config file, or with none. One model path at a
//!   time: an owner serves exactly the config it started with.
//! * **Routing** ([`Policy::route`]) runs only for `strategy: auto` with a
//!   configured policy, after the 009 merge and before graph expansion,
//!   outside every engine transaction and under the request's read
//!   deadline. No current graph, a busy or unavailable policy, an oversized
//!   input or too little time (half the remaining read deadline below 50 ms:
//!   `policy_insufficient_time`) route deterministically without a model call.
//!   The core thresholds the UNROUNDED maximum of the supplied probability
//!   vector after validating the whole reply ([`validate_reply`]); the
//!   reported confidence never decides. Abstention is `policy_abstained`.
//! * **Header**: one segment, `route:policy` or `route:fallback:<reason>`,
//!   only when a config is given and not disabled (context-v2 § Header
//!   line, 013 T003 amendment).
use crate::control::Control;
use crate::decision_model::{self, OPTIONS, SpecialIds};
use crate::error::{FResult, FoundryError};
use crate::learning::candidate::{self, CandidateManifest, VerifiedCandidate};
use crate::learning::ipc::{PredictReply, Probabilities};
use crate::learning::profile::LearnProfile;
use crate::neural::anchor::Dir;
use crate::response::Strategy;
use crate::store::Engine;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The only config version.
pub const CONFIG_VERSION: u32 = 2;
const CONFIG_MAX_BYTES: u64 = 64 * 1024;
/// Every config refusal names this code.
pub const CONFIG_INVALID: &str = "policy_config_invalid";
/// The reply tolerance for the probability sum and the reported confidence.
pub const TOLERANCE: f64 = 1e-6;
/// Status details are bounded.
const DETAIL_MAX_CHARS: usize = 512;

fn fail(code: &'static str, message: impl Into<String>) -> FoundryError {
    FoundryError::Learning {
        code,
        message: message.into(),
    }
}

// ---------------------------------------------------------------------------
// Config v2
// ---------------------------------------------------------------------------

/// An enabled config: exactly these fields, `enabled: true`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnabledConfig {
    pub v: u32,
    pub enabled: bool,
    /// The candidate directory, absolute.
    pub candidate_path: PathBuf,
    /// SHA-256 of the candidate's exact `manifest.json` bytes.
    pub candidate_sha256: String,
    pub model_function_sha256: String,
    /// SHA-256 of the candidate's `evaluation.json`.
    pub report_sha256: String,
    /// The threshold the candidate's evaluation report was computed at.
    pub threshold: f64,
    /// The learning-worker isolation profile, absolute.
    pub isolation_profile: PathBuf,
    /// `true` only when `learning select --lifecycle-check` wrote the config,
    /// absent otherwise: startup then skips ONLY the economics gate. Solely
    /// for lifecycle and package verification (013 T004, D001); never
    /// enablement and never a shipped default.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lifecycle_check: bool,
}

/// A disabled config: exactly `{"v":2,"enabled":false}`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DisabledConfig {
    v: u32,
    #[allow(dead_code)]
    enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PolicyConfig {
    Disabled,
    Enabled(EnabledConfig),
}

fn absolute_plain(path: &Path) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
}

impl PolicyConfig {
    /// Read at most 64 KiB of a regular file (never through a final-component
    /// link) and parse it strictly.
    pub fn load(path: &Path) -> FResult<Self> {
        let unreadable = |e: std::io::Error| {
            fail(
                CONFIG_INVALID,
                format!("policy config {}: {e}", path.display()),
            )
        };
        let raw = crate::learning::read_capped(
            crate::learning::open_regular(path).map_err(unreadable)?,
            CONFIG_MAX_BYTES,
        )
        .map_err(unreadable)?
        .ok_or_else(|| {
            fail(
                CONFIG_INVALID,
                format!("policy config exceeds {CONFIG_MAX_BYTES} bytes"),
            )
        })?;
        Self::parse(&raw)
    }

    /// Strict JSON (no duplicate keys, unknown or null fields), version 2,
    /// then the disabled or the enabled shape and every field's bounds.
    pub fn parse(raw: &[u8]) -> FResult<Self> {
        let value: serde_json::Value =
            crate::learning::strict_json(raw, CONFIG_INVALID, "policy config")?;
        let shape = |e: serde_json::Error| fail(CONFIG_INVALID, format!("policy config: {e}"));
        let version = |v: u32| {
            if v == CONFIG_VERSION {
                Ok(())
            } else {
                Err(fail(
                    CONFIG_INVALID,
                    format!("policy config v must be {CONFIG_VERSION}, not {v}"),
                ))
            }
        };
        match value.get("enabled") {
            Some(serde_json::Value::Bool(false)) => {
                let config: DisabledConfig = serde_json::from_value(value).map_err(shape)?;
                version(config.v)?;
                Ok(Self::Disabled)
            }
            Some(serde_json::Value::Bool(true)) => {
                if value
                    .get("lifecycle_check")
                    .is_some_and(|flag| *flag != serde_json::Value::Bool(true))
                {
                    return Err(fail(
                        CONFIG_INVALID,
                        "policy config `lifecycle_check` may only be present as true",
                    ));
                }
                let config: EnabledConfig = serde_json::from_value(value).map_err(shape)?;
                version(config.v)?;
                config.validate()?;
                Ok(Self::Enabled(config))
            }
            _ => Err(fail(
                CONFIG_INVALID,
                "policy config `enabled` must be true or false",
            )),
        }
    }
}

impl EnabledConfig {
    fn validate(&self) -> FResult<()> {
        let invalid = |message: String| Err(fail(CONFIG_INVALID, message));
        for (name, path) in [
            ("candidate_path", &self.candidate_path),
            ("isolation_profile", &self.isolation_profile),
        ] {
            if !absolute_plain(path) {
                return invalid(format!("{name} must be an absolute path without `..`"));
            }
        }
        for (name, digest) in [
            ("candidate_sha256", &self.candidate_sha256),
            ("model_function_sha256", &self.model_function_sha256),
            ("report_sha256", &self.report_sha256),
        ] {
            if !crate::learning::hex64(digest) {
                return invalid(format!("{name} must be 64 lowercase hex characters"));
            }
        }
        if !self.threshold.is_finite() || !(0.0..=1.0).contains(&self.threshold) {
            return invalid("threshold must be finite in [0, 1]".into());
        }
        Ok(())
    }
}

/// The model function a candidate's manifest identities define.
fn candidate_function(manifest: &CandidateManifest) -> String {
    decision_model::model_function_sha256(
        &manifest.tokenizer.json_sha256,
        &manifest.tokenizer.config_sha256,
        SpecialIds::PINNED,
        &manifest.encoder.checkpoint,
    )
}

fn member_sha256(manifest: &CandidateManifest, name: &str) -> Option<String> {
    manifest
        .files
        .iter()
        .find(|member| member.name == name)
        .map(|member| member.sha256.clone())
}

/// An enabled config, resolved and checked against the candidate it pins.
pub struct Verified {
    pub config: EnabledConfig,
    /// The candidate directory, held by descriptor since its read-back.
    pub dir: Dir,
    pub candidate: VerifiedCandidate,
    pub profile: LearnProfile,
    pub head_sha256: String,
}

/// The offline economics gate (contract § Fitting, artifacts and evaluation,
/// 2026-10-06): the candidate's routed option must deliver the required
/// evidence on more evaluation tasks than deterministic routing; token
/// savings at equal evidence never enable. A refusal names its code
/// (`economics_unknown` or `candidate_no_benefit`) and the counts.
fn economics_gate(verified: &VerifiedCandidate) -> Result<(), (&'static str, String)> {
    let Some(economics) = &verified.report.economics else {
        return Err((
            "economics_unknown",
            "the candidate's evaluation report has no economics: not every evaluation row \
             carries task-checker evidence for both options"
                .to_owned(),
        ));
    };
    if economics.gained <= economics.lost {
        return Err((
            "candidate_no_benefit",
            format!(
                "routed evidence {} of {} vs baseline {} (delivered tokens {} vs {}); changed \
                 {}, gained {}, lost {}: no net evidence gain over deterministic routing, and \
                 token savings alone never enable",
                economics.routed.evidence,
                economics.rows,
                economics.baseline.evidence,
                economics.routed.delivered_tokens,
                economics.baseline.delivered_tokens,
                economics.changed_routes,
                economics.gained,
                economics.lost
            ),
        ));
    }
    Ok(())
}

/// Validate an enabled config against the candidate it pins: the complete
/// T002 read-back (temperature refusals included), the manifest SHA-256,
/// the model function (pinned, and recomputed from the manifest's
/// identities), the report SHA-256, eligibility, the economics gate (skipped,
/// alone, for a `lifecycle_check` config), the exact threshold the candidate
/// was evaluated at, this workspace and the isolation profile. Every refusal
/// is `policy_config_invalid` naming its cause.
pub fn verify_config(
    config: &EnabledConfig,
    workspace_id: Option<&str>,
    control: &Control,
) -> FResult<Verified> {
    let invalid = |message: String| fail(CONFIG_INVALID, message);
    let (dir, verified) = candidate::open(&config.candidate_path, CONFIG_INVALID, control)
        .map_err(|e| match e {
            FoundryError::Cancelled(_) | FoundryError::DeadlineExceeded(_) => e,
            other if other.code() == CONFIG_INVALID => other,
            other => invalid(format!("{}: {other}", other.code())),
        })?;
    let manifest = &verified.manifest;
    if verified.manifest_sha256 != config.candidate_sha256 {
        return Err(invalid(format!(
            "the candidate at {} is {}; the config pins {}",
            config.candidate_path.display(),
            verified.manifest_sha256,
            config.candidate_sha256
        )));
    }
    if manifest.model_function_sha256 != config.model_function_sha256
        || candidate_function(manifest) != config.model_function_sha256
    {
        return Err(invalid(
            "the candidate's model function is not the one the config pins".into(),
        ));
    }
    if member_sha256(manifest, candidate::EVALUATION).as_deref() != Some(&config.report_sha256) {
        return Err(invalid(
            "the candidate's evaluation report is not the one the config pins".into(),
        ));
    }
    if !manifest.eligible || !verified.report.eligibility.eligible {
        return Err(invalid("the candidate is not eligible".into()));
    }
    if !config.lifecycle_check {
        economics_gate(&verified).map_err(|(code, why)| invalid(format!("{code}: {why}")))?;
    }
    if verified.report.selection.threshold != config.threshold {
        return Err(invalid(format!(
            "threshold {} is not the threshold {} the candidate was evaluated at; a threshold \
             cannot be edited to reuse eligibility",
            config.threshold, verified.report.selection.threshold
        )));
    }
    if let Some(workspace_id) = workspace_id
        && manifest.workspace_id != workspace_id
    {
        return Err(invalid("the candidate belongs to another workspace".into()));
    }
    let profile = LearnProfile::load(&config.isolation_profile)
        .map_err(|e| invalid(format!("isolation profile: {}: {e}", e.code())))?;
    let head_sha256 = member_sha256(manifest, candidate::HEAD)
        .ok_or_else(|| invalid("the candidate binds no head".into()))?;
    Ok(Verified {
        config: config.clone(),
        dir,
        candidate: verified,
        profile,
        head_sha256,
    })
}

// ---------------------------------------------------------------------------
// Replies
// ---------------------------------------------------------------------------

/// The identity a reply must echo.
pub struct Expected<'a> {
    pub candidate_sha256: &'a str,
    pub model_function_sha256: &'a str,
    /// `SHA256(compact JSON [family, state, ordered option IDs])` of the
    /// request.
    pub input_sha256: &'a str,
}

/// A reply the core validated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prediction {
    pub probabilities: Probabilities,
    /// The maximum/tie choice, which the reply's `choice` equalled.
    pub choice: &'static str,
    /// The reported confidence: checked, never thresholded.
    pub answer_confidence: f64,
}

impl Prediction {
    /// The unrounded maximum of the supplied vector: the ONE number a
    /// threshold applies to.
    pub fn maximum(&self) -> f64 {
        self.probabilities.search.max(self.probabilities.graph)
    }

    pub fn strategy(&self) -> Strategy {
        if self.choice == "graph" {
            Strategy::Graph
        } else {
            Strategy::Search
        }
    }
}

/// The contract's choice for a probability vector: the maximum; an exact
/// tie takes the lexicographically smallest UTF-8 option ID.
pub fn rule_choice(p: &Probabilities) -> &'static str {
    if p.search > p.graph {
        "search"
    } else if p.graph > p.search {
        "graph"
    } else {
        OPTIONS
            .iter()
            .map(|option| option.id)
            .min()
            .expect("two options")
    }
}

/// Validate one reply (the frame layer already refused unknown, null and
/// duplicate fields — the legacy `confidence` among them — and any other
/// version): the echoed candidate, model function and input digest; both
/// probabilities finite in [0, 1] and summing to 1 within 1e-6; the choice
/// equal to the maximum/tie rule; the reported confidence within 1e-6 of the
/// maximum. `Err` is `(code, message)`: `reply_identity_mismatch` or
/// `reply_invalid`, both terminal for the worker.
pub fn validate_reply(
    reply: &PredictReply,
    expected: &Expected<'_>,
) -> Result<Prediction, (&'static str, String)> {
    if reply.candidate_sha256 != expected.candidate_sha256
        || reply.model_function_sha256 != expected.model_function_sha256
    {
        return Err((
            "reply_identity_mismatch",
            "the reply names another candidate or model function than this owner loaded".into(),
        ));
    }
    if reply.input_sha256 != expected.input_sha256 {
        return Err((
            "reply_identity_mismatch",
            "the reply was computed on another input than the request's".into(),
        ));
    }
    let p = reply.probabilities;
    for (id, value) in [("search", p.search), ("graph", p.graph)] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err((
                "reply_invalid",
                format!("probability {id} = {value} is not finite in [0, 1]"),
            ));
        }
    }
    let sum = p.search + p.graph;
    if (sum - 1.0).abs() > TOLERANCE {
        return Err((
            "reply_invalid",
            format!("the probabilities sum to {sum}, not 1 within {TOLERANCE}"),
        ));
    }
    let rule = rule_choice(&p);
    if reply.choice != rule {
        let shown: String = reply.choice.chars().take(32).collect();
        return Err((
            "reply_invalid",
            format!("choice {shown:?} is not the maximum/tie choice {rule}"),
        ));
    }
    let maximum = p.search.max(p.graph);
    if !reply.answer_confidence.is_finite() || (reply.answer_confidence - maximum).abs() > TOLERANCE
    {
        return Err((
            "reply_invalid",
            format!(
                "answer_confidence {} is not the maximum probability {maximum} within {TOLERANCE}",
                reply.answer_confidence
            ),
        ));
    }
    Ok(Prediction {
        probabilities: p,
        choice: rule,
        answer_confidence: reply.answer_confidence,
    })
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

/// `learning select --candidate DIR --isolation-profile FILE --out CONFIG
/// [--lifecycle-check]`.
pub struct SelectRequest<'a> {
    pub candidate: &'a Path,
    pub isolation_profile: &'a Path,
    pub out: &'a Path,
    /// Lifecycle and package verification only (013 T004, D001): select an
    /// otherwise valid candidate that fails the economics gate, and mark the
    /// config `"lifecycle_check": true`. Never enablement.
    pub lifecycle_check: bool,
}

#[derive(Debug, Serialize)]
pub struct Selected {
    pub outcome: &'static str,
    pub config: PathBuf,
    pub candidate_sha256: String,
    pub threshold: f64,
    /// Present only as `true`, for a `--lifecycle-check` selection.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub lifecycle_check: bool,
}

/// Every example the candidate was fitted, calibrated or evaluated on must
/// still be permitted with its identical input, label and rights
/// (contract 106-110: consent is checked again at selection).
fn check_current_consent(
    engine: &Engine,
    verified: &VerifiedCandidate,
    control: &Control,
) -> FResult<()> {
    let changed = |id: &str, what: &str| {
        Err(fail(
            "contribution_changed",
            format!("example {id}: {what} since the candidate was trained; nothing was selected"),
        ))
    };
    let contributions = &verified.contributions;
    for contribution in [&contributions.fitting, &contributions.calibration]
        .into_iter()
        .flat_map(|lineage| lineage.new.iter().chain(&lineage.inherited))
    {
        control.check()?;
        let id = &contribution.example_id;
        let what = match engine.learning_current_row(id)? {
            None => Some("it is no longer in the store"),
            Some(row) if !row.allow_training => Some("its training consent was withdrawn"),
            Some(row) if row.permission_sha256() != contribution.permission_sha256 => {
                Some("its permission (rights assertion) changed")
            }
            Some(row) if row.input_sha256() != contribution.input_sha256 => {
                Some("its exact input changed")
            }
            Some(row) if row.correct_option_id != contribution.correct_option_id => {
                Some("its label changed")
            }
            Some(_) => None,
        };
        if let Some(what) = what {
            return changed(id, what);
        }
    }
    for case in &verified.report.cases {
        control.check()?;
        let id = &case.example_id;
        let what = match engine.learning_current_row(id)? {
            None => Some("it is no longer in the store"),
            Some(row) if !row.allow_training => Some("its training consent was withdrawn"),
            Some(row) if row.permission_sha256() != case.permission_sha256 => {
                Some("its permission (rights assertion) changed")
            }
            Some(row)
                if row.correct_option_id != case.expected || row.option_ids != case.option_ids =>
            {
                Some("its label or options changed")
            }
            Some(_) => None,
        };
        if let Some(what) = what {
            return changed(id, what);
        }
    }
    Ok(())
}

/// Select an eligible candidate: read it back completely, require this
/// workspace, its model-function identity, eligibility and the current
/// consent of its examples, load the isolation profile, require a net
/// evidence gain over deterministic routing (contract § Fitting, artifacts
/// and evaluation, 2026-10-06; `lifecycle_check` waives that gate alone and
/// marks the config), then publish a NEW config naming them. Never overwrites
/// (`output_exists`).
pub fn select(
    engine: &Engine,
    request: &SelectRequest<'_>,
    control: &Control,
) -> FResult<Selected> {
    control.check()?;
    let workspace_id = engine
        .workspace_id()
        .ok_or(FoundryError::WorkspaceUnbound)?;
    let candidate_path = std::fs::canonicalize(request.candidate).map_err(|e| {
        fail(
            "candidate_invalid",
            format!("candidate {}: {e}", request.candidate.display()),
        )
    })?;
    let (_dir, verified) = candidate::open(&candidate_path, "candidate_invalid", control)?;
    let manifest = &verified.manifest;
    if manifest.workspace_id != workspace_id {
        return Err(fail(
            "candidate_invalid",
            "the candidate belongs to another workspace",
        ));
    }
    if candidate_function(manifest) != manifest.model_function_sha256 {
        return Err(fail(
            "candidate_invalid",
            "the candidate's model function does not match its tokenizer and checkpoint identities",
        ));
    }
    if !manifest.eligible || !verified.report.eligibility.eligible {
        return Err(fail(
            "candidate_ineligible",
            format!(
                "the candidate is not eligible ({}); a rejected candidate is lifecycle evidence, \
                 never a selection",
                verified.report.eligibility.reasons.join(", ")
            ),
        ));
    }
    check_current_consent(engine, &verified, control)?;
    let isolation_profile = std::fs::canonicalize(request.isolation_profile).map_err(|e| {
        fail(
            "profile_invalid",
            format!("profile {}: {e}", request.isolation_profile.display()),
        )
    })?;
    LearnProfile::load(&isolation_profile)?;
    if !request.lifecycle_check {
        economics_gate(&verified)
            .map_err(|(code, why)| fail(code, format!("{why}; nothing was selected")))?;
    }
    let config = EnabledConfig {
        v: CONFIG_VERSION,
        enabled: true,
        candidate_path,
        candidate_sha256: verified.manifest_sha256.clone(),
        model_function_sha256: manifest.model_function_sha256.clone(),
        report_sha256: member_sha256(manifest, candidate::EVALUATION).ok_or_else(|| {
            fail(
                "candidate_invalid",
                "the candidate binds no evaluation report",
            )
        })?,
        threshold: verified.report.selection.threshold,
        isolation_profile,
        lifecycle_check: request.lifecycle_check,
    };
    let mut bytes = serde_json::to_vec_pretty(&config).map_err(FoundryError::from)?;
    bytes.push(b'\n');
    if PolicyConfig::parse(&bytes)? != PolicyConfig::Enabled(config.clone()) {
        return Err(fail(
            "output_write",
            "the config does not parse back as written",
        ));
    }
    publish_config(engine, request.out, &bytes)?;
    Ok(Selected {
        outcome: "selected",
        config: request.out.to_path_buf(),
        candidate_sha256: config.candidate_sha256,
        threshold: config.threshold,
        lifecycle_check: config.lifecycle_check,
    })
}

/// Publish `bytes` as the NEW file `out`: the parent is opened once and held;
/// the output may not lie under the admitted source root; a partial sibling
/// is written, flushed and fsynced, renamed WITHOUT replacing anything, and
/// the parent fsynced; the file is then read back through the held parent.
fn publish_config(engine: &Engine, out: &Path, bytes: &[u8]) -> FResult<()> {
    let write_error = |what: &str, e: std::io::Error| fail("output_write", format!("{what}: {e}"));
    let name = match out.components().next_back() {
        Some(std::path::Component::Normal(name)) => name.to_owned(),
        _ => {
            return Err(FoundryError::InvalidArgument(format!(
                "--out {} must name a new config file",
                out.display()
            )));
        }
    };
    let parent_path = out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let resolved =
        std::fs::canonicalize(parent_path).map_err(|e| write_error("config parent", e))?;
    let parent = Dir::open_path(&resolved).map_err(|e| write_error("open config parent", e))?;
    crate::learning::refuse_inside_root(engine, &parent, out)?;
    let occupied = |out: &Path| {
        fail(
            "output_exists",
            format!(
                "{} already exists; select never overwrites a config",
                out.display()
            ),
        )
    };
    if parent
        .kind_of(&name)
        .map_err(|e| write_error("config destination", e))?
        .is_some()
    {
        return Err(occupied(out));
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let partial = OsString::from(format!(
        ".policy-config-partial-{}-{nanos}",
        std::process::id()
    ));
    if let Err(e) = parent.write_new(&partial, bytes) {
        let _ = parent.remove_tree(&partial);
        return Err(write_error("write the config", e));
    }
    match parent.rename_noreplace_into(&partial, &parent, &name) {
        Ok(()) => {}
        Err(e) => {
            let _ = parent.remove_tree(&partial);
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                return Err(occupied(out));
            }
            return Err(write_error("publish the config", e));
        }
    }
    parent
        .sync_all()
        .map_err(|e| write_error("fsync the config parent", e))?;
    let back = parent
        .open_file(&name)
        .and_then(|file| crate::learning::read_capped(file, CONFIG_MAX_BYTES))
        .map_err(|e| write_error("read the config back", e))?;
    if back.as_deref() != Some(bytes) {
        return Err(fail(
            "output_write",
            "the published config does not read back as written",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The served policy
// ---------------------------------------------------------------------------

/// The launch-time configuration (`--policy-config FILE
/// [--development-isolation]`).
pub struct PolicyServing {
    pub config: PathBuf,
    pub development: bool,
    /// Tests only: the fake worker's hooks.
    #[cfg(feature = "test-faults")]
    pub worker_args: Vec<String>,
}

impl PolicyServing {
    pub fn new(config: PathBuf, development: bool) -> Self {
        Self {
            config,
            development,
            #[cfg(feature = "test-faults")]
            worker_args: Vec::new(),
        }
    }

    /// Tests only: development isolation with the fake worker's hooks.
    #[cfg(feature = "test-faults")]
    pub fn with_worker_args(config: PathBuf, worker_args: Vec<String>) -> Self {
        Self {
            config,
            development: true,
            worker_args,
        }
    }
}

/// One owner's (MCP) or one command's (CLI) policy, fixed at its start.
pub struct Policy {
    state: State,
}

enum State {
    /// No config, or a disabled one: no routing and no header segment.
    Off,
    /// A config was given but cannot serve: invalid, or its worker could not
    /// start. Every `auto` context reports `fallback:policy_unavailable`.
    Unavailable {
        reason: &'static str,
        detail: String,
        candidate: Option<String>,
        /// The config parsed as a `--lifecycle-check` selection.
        lifecycle_check: bool,
    },
    #[cfg(all(target_os = "macos", feature = "semantic"))]
    Serving(Box<Serving>),
}

/// The resident serving state: the exact-input renderer (the tokenizer
/// preflight) and the supervised worker.
#[cfg(all(target_os = "macos", feature = "semantic"))]
struct Serving {
    candidate: String,
    threshold: f64,
    /// The config is a `--lifecycle-check` selection: status says so.
    lifecycle_check: bool,
    renderer: decision_model::Renderer,
    worker: crate::learning::serve::PolicyWorker,
}

fn bounded(detail: &str) -> String {
    detail.chars().take(DETAIL_MAX_CHARS).collect()
}

/// A parsed `lifecycle_check` config's status says so in every state,
/// enabled or not, so it can never be mistaken for normal enablement.
fn marked(mut status: serde_json::Value, lifecycle_check: bool) -> serde_json::Value {
    if lifecycle_check {
        status["lifecycle_check"] = serde_json::Value::Bool(true);
    }
    status
}

fn fallback(strategy: Strategy, reason: &str) -> FResult<(Strategy, String)> {
    Ok((strategy, format!("fallback:{reason}")))
}

impl Policy {
    /// No policy: every response stays byte-identical to an owner without
    /// one.
    pub fn off() -> Self {
        Self { state: State::Off }
    }

    fn unavailable(
        reason: &'static str,
        detail: String,
        candidate: Option<String>,
        lifecycle_check: bool,
    ) -> Self {
        Self {
            state: State::Unavailable {
                reason,
                detail,
                candidate,
                lifecycle_check,
            },
        }
    }

    /// Validate the config at owner startup and, when it is enabled, start
    /// the serving worker (bounded by the load ceiling). Never fails the
    /// owner: an invalid config or a failed start is a named unavailable
    /// state, and baseline retrieval serves.
    pub fn start(launch: Option<PolicyServing>, workspace_id: Option<&str>) -> Self {
        let Some(launch) = launch else {
            return Self::off();
        };
        let config = match PolicyConfig::load(&launch.config) {
            Ok(PolicyConfig::Disabled) => return Self::off(),
            Ok(PolicyConfig::Enabled(config)) => config,
            Err(e) => return Self::unavailable(CONFIG_INVALID, e.to_string(), None, false),
        };
        let candidate = Some(config.candidate_sha256.clone());
        let lifecycle_check = config.lifecycle_check;
        let verified = match verify_config(&config, workspace_id, &Control::unbounded()) {
            Ok(verified) => verified,
            Err(e) => {
                return Self::unavailable(
                    CONFIG_INVALID,
                    e.to_string(),
                    candidate,
                    lifecycle_check,
                );
            }
        };
        if !launch.development {
            return Self::unavailable(
                "isolation_unavailable",
                "the learning worker runs only with --development-isolation until signing and \
                 package acceptance close; it is never production isolation"
                    .into(),
                candidate,
                lifecycle_check,
            );
        }
        #[cfg(feature = "test-faults")]
        let extra_args = launch.worker_args;
        #[cfg(not(feature = "test-faults"))]
        let extra_args: Vec<String> = Vec::new();
        #[cfg(all(target_os = "macos", feature = "semantic"))]
        {
            match Serving::start(verified, extra_args) {
                Ok(serving) => Self {
                    state: State::Serving(Box::new(serving)),
                },
                Err(e) => Self::unavailable(e.code(), e.to_string(), candidate, lifecycle_check),
            }
        }
        #[cfg(not(all(target_os = "macos", feature = "semantic")))]
        {
            let _ = (verified, extra_args);
            if cfg!(target_os = "macos") {
                Self::unavailable(
                    "learning_unavailable",
                    "this build has no learning renderer; rebuild with the `semantic` feature"
                        .into(),
                    candidate,
                    lifecycle_check,
                )
            } else {
                Self::unavailable(
                    "isolation_unavailable",
                    "the learning worker's isolation profile targets macOS".into(),
                    candidate,
                    lifecycle_check,
                )
            }
        }
    }

    /// True when a config was given and is not disabled: `auto` contexts are
    /// routed through [`Self::route`] and carry a `route:` header word.
    pub fn routes(&self) -> bool {
        !matches!(self.state, State::Off)
    }

    /// Route one `auto` context: the strategy and its header word, `policy`
    /// or `fallback:<reason>`. Model work happens only for an enabled,
    /// non-busy policy with a current graph scope and a state that passes
    /// the exact tokenizer preflight, and only within the read deadline.
    /// Only database, cancellation and deadline errors fail the request.
    pub fn route(
        &self,
        engine: &Engine,
        query: &str,
        control: &Control,
    ) -> FResult<(Strategy, String)> {
        let deterministic = crate::response::strategy_for_query(query);
        match &self.state {
            #[cfg(all(target_os = "macos", feature = "semantic"))]
            State::Serving(serving) => serving.route(engine, query, control, deterministic),
            _ => {
                let _ = (engine, control);
                fallback(deterministic, "policy_unavailable")
            }
        }
    }

    /// The `status` object: `disabled`, `enabled` (candidate, threshold,
    /// consecutive timeouts, whether the slot is occupied) or `unavailable`
    /// with its reason; any state of a parsed `lifecycle_check` config adds
    /// `"lifecycle_check": true`.
    pub fn status(&self) -> serde_json::Value {
        match &self.state {
            State::Off => serde_json::json!({"state": "disabled"}),
            State::Unavailable {
                reason,
                detail,
                candidate,
                lifecycle_check,
            } => marked(
                serde_json::json!({
                    "state": "unavailable",
                    "reason": reason,
                    "detail": bounded(detail),
                    "candidate": candidate,
                    "consecutive_timeouts": 0,
                }),
                *lifecycle_check,
            ),
            #[cfg(all(target_os = "macos", feature = "semantic"))]
            State::Serving(serving) => {
                let worker = serving.worker.state();
                let status = match worker.terminal {
                    Some((reason, detail)) => serde_json::json!({
                        "state": "unavailable",
                        "reason": reason,
                        "detail": bounded(&detail),
                        "candidate": serving.candidate,
                        "consecutive_timeouts": worker.consecutive_timeouts,
                    }),
                    None => serde_json::json!({
                        "state": "enabled",
                        "candidate": serving.candidate,
                        "threshold": serving.threshold,
                        "consecutive_timeouts": worker.consecutive_timeouts,
                        "busy": worker.busy,
                    }),
                };
                marked(status, serving.lifecycle_check)
            }
        }
    }

    /// CLI `status --policy-config FILE`: the config validated exactly as an
    /// owner validates it at startup, without starting a worker (a CLI
    /// command has no resident worker, so it has no timeouts). A parsed
    /// `lifecycle_check` config is marked whether it verifies or not.
    pub fn inspect(config: &Path, workspace_id: Option<&str>) -> serde_json::Value {
        let unavailable = |detail: String, candidate: Option<&str>| {
            serde_json::json!({
                "state": "unavailable",
                "reason": CONFIG_INVALID,
                "detail": bounded(&detail),
                "candidate": candidate,
                "consecutive_timeouts": 0,
            })
        };
        match PolicyConfig::load(config) {
            Err(e) => unavailable(e.to_string(), None),
            Ok(PolicyConfig::Disabled) => serde_json::json!({"state": "disabled"}),
            Ok(PolicyConfig::Enabled(enabled)) => {
                let candidate = Some(enabled.candidate_sha256.as_str());
                let status = match verify_config(&enabled, workspace_id, &Control::unbounded()) {
                    Ok(_) => serde_json::json!({
                        "state": "enabled",
                        "candidate": candidate,
                        "threshold": enabled.threshold,
                        "consecutive_timeouts": 0,
                        "resident": false,
                    }),
                    Err(e) => unavailable(e.to_string(), candidate),
                };
                marked(status, enabled.lifecycle_check)
            }
        }
    }
}

#[cfg(all(target_os = "macos", feature = "semantic"))]
impl Serving {
    /// The exact tokenizer preflight renderer: the pinned checkpoint's own
    /// `tokenizer/` (the isolation profile's checkpoint directory), held to
    /// the candidate's tokenizer identity (`tokenizer_mismatch`); then the
    /// worker, loaded with the candidate within the load ceiling.
    fn start(verified: Verified, extra_args: Vec<String>) -> FResult<Self> {
        use crate::learning::serve::{PolicyWorker, ServeLaunch};
        let manifest = &verified.candidate.manifest;
        let loaded = crate::learning::load_pinned_renderer(&crate::learning::TokenizerPin {
            dir: verified.profile.checkpoint_dir.join("tokenizer"),
            json_sha256: manifest.tokenizer.json_sha256.clone(),
            config_sha256: manifest.tokenizer.config_sha256.clone(),
        })?;
        let worker = PolicyWorker::start(
            ServeLaunch {
                profile: &verified.profile,
                checkpoint: &manifest.encoder.checkpoint,
                model_function_sha256: &manifest.model_function_sha256,
                candidate_sha256: &verified.candidate.manifest_sha256,
                temperature: manifest.temperature,
                candidate: &verified.dir,
                head_sha256: &verified.head_sha256,
                extra_args,
            },
            &Control::unbounded(),
        )?;
        Ok(Self {
            candidate: verified.candidate.manifest_sha256.clone(),
            threshold: verified.config.threshold,
            lifecycle_check: verified.config.lifecycle_check,
            renderer: loaded.renderer,
            worker,
        })
    }

    fn route(
        &self,
        engine: &Engine,
        query: &str,
        control: &Control,
        deterministic: Strategy,
    ) -> FResult<(Strategy, String)> {
        use crate::learning::serve::Refused;
        // Cheap checks first: a terminated or busy policy composes nothing.
        let worker = self.worker.state();
        if worker.terminal.is_some() {
            return fallback(deterministic, "policy_unavailable");
        }
        if worker.busy {
            return fallback(deterministic, "policy_busy");
        }
        // The core composes the state; no current graph scope skips the
        // model, and the coverage line is model input, not the gate.
        let state = match engine.compose_route_state(query, control) {
            Ok(state) => state,
            Err(e) if matches!(e.code(), "graph_unavailable" | "graph_stale") => {
                return fallback(deterministic, e.code());
            }
            Err(e) if e.code() == "state_too_large" => {
                return fallback(deterministic, "policy_input_oversize");
            }
            Err(e) => return Err(e),
        };
        // The exact tokenizer preflight, before any encoder work: a refused
        // input is a named fallback, never a truncation.
        let order = [OPTIONS[0].id, OPTIONS[1].id];
        let rendered = match self.renderer.render(&state, order) {
            Ok(rendered) => rendered,
            Err(e)
                if matches!(
                    e.code(),
                    "state_too_large" | "input_too_long" | "header_too_long" | "option_too_long"
                ) =>
            {
                return fallback(deterministic, "policy_input_oversize");
            }
            Err(_) => return fallback(deterministic, "policy_unavailable"),
        };
        let deadline = control
            .deadline()
            .unwrap_or_else(|| std::time::Instant::now() + crate::mcp::READ_DEADLINE);
        match self.worker.predict(&state, order, &rendered, deadline) {
            Ok(prediction) if prediction.maximum() >= self.threshold => {
                Ok((prediction.strategy(), "policy".to_owned()))
            }
            Ok(_) => fallback(deterministic, "policy_abstained"),
            Err(Refused::Busy) => fallback(deterministic, "policy_busy"),
            Err(Refused::Timeout) => fallback(deterministic, "policy_timeout"),
            Err(Refused::InsufficientTime) => fallback(deterministic, "policy_insufficient_time"),
            Err(Refused::Unavailable) => fallback(deterministic, "policy_unavailable"),
            Err(Refused::Oversize) => fallback(deterministic, "policy_input_oversize"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(search: f64, graph: f64, choice: &str, confidence: f64) -> PredictReply {
        PredictReply {
            v: 2,
            request_id: 2,
            candidate_sha256: "c".repeat(64),
            model_function_sha256: "f".repeat(64),
            input_sha256: "1".repeat(64),
            choice: choice.to_owned(),
            probabilities: Probabilities { search, graph },
            answer_confidence: confidence,
        }
    }

    fn expected() -> (String, String, String) {
        ("c".repeat(64), "f".repeat(64), "1".repeat(64))
    }

    fn check(reply: &PredictReply) -> Result<Prediction, (&'static str, String)> {
        let (c, f, i) = expected();
        validate_reply(
            reply,
            &Expected {
                candidate_sha256: &c,
                model_function_sha256: &f,
                input_sha256: &i,
            },
        )
    }

    #[test]
    fn the_threshold_reads_the_unrounded_maximum_not_the_reported_confidence() {
        // [0.8, 0.2] at threshold 0.8 accepts; its normalized-entropy
        // confidence (about 0.278) is a different scale and never consulted.
        let accepted = check(&reply(0.8, 0.2, "search", 0.8)).unwrap();
        assert!(accepted.maximum() >= 0.8);
        assert_eq!(accepted.strategy(), Strategy::Search);
        let entropy = 1.0 + (0.8f64 * 0.8f64.log2() + 0.2 * 0.2f64.log2());
        assert!((entropy - 0.278).abs() < 1e-3 && entropy < 0.8);
        // [0.79999, 0.20001] abstains at 0.8, before any rounding.
        let below = check(&reply(0.79999, 0.20001, "search", 0.79999)).unwrap();
        assert!(below.maximum() < 0.8);
        // A reported confidence inside the tolerance but at or over the
        // threshold cannot lift a maximum that is under it.
        let lifted = check(&reply(0.7999995, 0.2000005, "search", 0.8000004)).unwrap();
        assert!(lifted.answer_confidence >= 0.8 && lifted.maximum() < 0.8);
    }

    #[test]
    fn a_malformed_distribution_or_choice_never_validates() {
        for (bad, why) in [
            (reply(0.7, 0.2, "search", 0.7), "sum"),
            (reply(1.2, -0.2, "search", 1.2), "range"),
            (reply(f64::NAN, 0.5, "search", 0.5), "nonfinite"),
            (reply(0.2, 0.8, "search", 0.8), "minority choice"),
            (reply(0.8, 0.2, "delete_workspace", 0.8), "unknown choice"),
            (reply(0.8, 0.2, "search", 0.79), "confidence"),
            (
                reply(0.8, 0.2, "search", f64::INFINITY),
                "nonfinite confidence",
            ),
        ] {
            assert_eq!(check(&bad).unwrap_err().0, "reply_invalid", "{why}");
        }
        // Within 1e-6 of a sum of 1 and of the maximum is still valid.
        assert!(check(&reply(0.8000004, 0.2, "search", 0.8)).is_ok());
    }

    #[test]
    fn an_exact_tie_takes_the_lexicographically_smallest_option() {
        assert_eq!(
            rule_choice(&Probabilities {
                search: 0.5,
                graph: 0.5
            }),
            "graph"
        );
        assert!(check(&reply(0.5, 0.5, "graph", 0.5)).is_ok());
        assert_eq!(
            check(&reply(0.5, 0.5, "search", 0.5)).unwrap_err().0,
            "reply_invalid"
        );
    }

    #[test]
    fn identities_are_cross_checked() {
        let mut other = reply(0.8, 0.2, "search", 0.8);
        other.candidate_sha256 = "d".repeat(64);
        assert_eq!(check(&other).unwrap_err().0, "reply_identity_mismatch");
        let mut other = reply(0.8, 0.2, "search", 0.8);
        other.model_function_sha256 = "e".repeat(64);
        assert_eq!(check(&other).unwrap_err().0, "reply_identity_mismatch");
        let mut other = reply(0.8, 0.2, "search", 0.8);
        other.input_sha256 = "2".repeat(64);
        assert_eq!(check(&other).unwrap_err().0, "reply_identity_mismatch");
    }

    fn enabled() -> serde_json::Value {
        serde_json::json!({
            "v": 2,
            "enabled": true,
            "candidate_path": "/abs/candidate",
            "candidate_sha256": "a".repeat(64),
            "model_function_sha256": "b".repeat(64),
            "report_sha256": "c".repeat(64),
            "threshold": 0.8,
            "isolation_profile": "/abs/profile.json",
        })
    }

    #[test]
    fn config_v2_is_strict() {
        assert_eq!(
            PolicyConfig::parse(br#"{"v":2,"enabled":false}"#).unwrap(),
            PolicyConfig::Disabled
        );
        assert!(matches!(
            PolicyConfig::parse(enabled().to_string().as_bytes()).unwrap(),
            PolicyConfig::Enabled(_)
        ));
        let mut cases: Vec<String> = vec![
            r#"{"v":2,"enabled":false,"candidate_path":"/x"}"#.into(),
            r#"{"v":1,"enabled":false}"#.into(),
            r#"{"v":2}"#.into(),
            r#"{"v":2,"enabled":null}"#.into(),
            r#"{"v":2,"enabled":false,"enabled":false}"#.into(),
        ];
        for (field, value) in [
            ("candidate_path", serde_json::json!("relative/candidate")),
            ("candidate_path", serde_json::json!("/abs/../escape")),
            ("isolation_profile", serde_json::json!("profile.json")),
            ("candidate_sha256", serde_json::json!("A".repeat(64))),
            ("report_sha256", serde_json::json!("c".repeat(63))),
            ("threshold", serde_json::json!(1.5)),
            ("threshold", serde_json::Value::Null),
            ("latest", serde_json::json!(true)),
        ] {
            let mut config = enabled();
            config[field] = value;
            cases.push(config.to_string());
        }
        let mut missing = enabled();
        missing.as_object_mut().unwrap().remove("report_sha256");
        cases.push(missing.to_string());
        for case in cases {
            let error = PolicyConfig::parse(case.as_bytes()).unwrap_err();
            assert_eq!(error.code(), CONFIG_INVALID, "{case}");
        }
    }
}
