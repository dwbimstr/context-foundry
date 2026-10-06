//! 013 T002 `learning train --input MANIFEST --policy FILE --out DIR
//! [--base CANDIDATE] [--incumbent CANDIDATE] --development-isolation`
//! (contract § Dataset and repeat rounds, § Fitting, artifacts and
//! evaluation).
//!
//! Trust split: the worker is a numerical engine only. The core, under
//! exclusive store ownership held for the whole run, does everything else:
//!
//! 1. reads the dataset back exactly as `learning check` does (every row
//!    re-rendered), requires it to be this store's published dataset for
//!    this workspace, model function and EXACT policy (`policy_mismatch`);
//! 2. validates the profile, its enforcement matrix and ceilings, the base
//!    and the incumbent, the deterministic comparator's query projection of
//!    every evaluation row and the critical groups — all before launch;
//! 3. launches the worker, loads the checkpoint (and the base head), and,
//!    immediately before the first update, re-reads the CURRENT feedback
//!    row of every intended contribution — new and inherited, fitting,
//!    calibration and evaluation — refusing any whose exact input, label or
//!    permission changed or was withdrawn (`contribution_changed`,
//!    `base_permission_changed`) before any update;
//! 4. drives `max_steps` updates over seeded epochs within the wall clock;
//!    checks the frozen encoder's digest before and after;
//! 5. fits the temperature on calibration rows only, evaluates the
//!    candidate against deterministic routing and the incumbent, saves and
//!    reload-checks the head, validates it by descriptor, and publishes the
//!    candidate through the anchored no-replace path, adopting an occupied
//!    destination only after validating all of it; then reads the published
//!    candidate back and requires identical calibrated probabilities.
//!
//! Any failure stops and reaps the worker first; nothing partial is ever
//! eligible or published.
use super::candidate::{self, Base, CandidateManifest, Contributions, Lineage};
use super::eval::{self, CaseInput, Output};
use super::profile::LearnProfile;
use super::{
    Contribution, LearningPolicy, OutputTarget, Publication, VerifiedDataset, compact_digest, fail,
    load_renderer, model_function_of, open_dataset, verify_dataset_with, verify_lineage,
};
use crate::control::Control;
use crate::decision_model::arch::HEAD_DROPOUT;
use crate::error::{FResult, FoundryError};
use crate::neural::anchor::Dir;
use crate::store::Engine;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// What the operator asked for.
pub struct TrainRequest<'a> {
    pub input: &'a Path,
    pub policy: &'a Path,
    pub out: &'a Path,
    pub base: Option<&'a Path>,
    pub incumbent: Option<&'a Path>,
    /// The owner-authorized development isolation profile; without it
    /// admission is `isolation_unavailable`.
    pub development_isolation: bool,
}

/// A completed run: a published (or adopted) candidate, eligible or not. A
/// rejected candidate is valid lifecycle evidence, not quality success.
#[derive(Debug, Serialize)]
pub struct Trained {
    pub outcome: &'static str,
    pub candidate: PathBuf,
    pub candidate_manifest_sha256: String,
    pub eligible: bool,
    pub reasons: Vec<String>,
    pub steps_completed: u64,
    pub temperature: f64,
    /// The destination already held exactly this candidate (an earlier run
    /// whose response was lost) and was validated and adopted.
    pub adopted: bool,
}

/// One training, calibration or evaluation row as the run uses it.
struct Row {
    contribution: Contribution,
    group_id: String,
    option_ids: [String; 2],
    expected: String,
    /// Row-order index of the expected option.
    target: u32,
    ids: Vec<u32>,
    markers: [u32; 2],
    /// Evaluation rows only: deterministic routing's choice.
    baseline: Option<&'static str>,
}

fn row_of(row: super::DatasetRow, baseline: Option<&'static str>) -> Row {
    let option_ids = [
        row.feedback.option_ids[0].clone(),
        row.feedback.option_ids[1].clone(),
    ];
    let target = option_ids
        .iter()
        .position(|id| *id == row.feedback.correct_option_id)
        .expect("a validated row's label is one of its options") as u32;
    Row {
        contribution: Contribution {
            example_id: row.example_id,
            input_sha256: row.input_sha256,
            correct_option_id: row.feedback.correct_option_id.clone(),
            permission_sha256: row.feedback.permission_sha256(),
        },
        group_id: row.feedback.task_group_id,
        option_ids,
        expected: row.feedback.correct_option_id,
        target,
        ids: row.token_ids,
        markers: [row.markers[0] as u32, row.markers[1] as u32],
        baseline,
    }
}

/// The seed of the head dropout: the first eight bytes of
/// SHA-256(compact JSON `["dropout", seed]`), big-endian.
pub fn dropout_seed(seed: &str) -> u64 {
    let digest = compact_digest(&serde_json::json!(["dropout", seed]));
    u64::from_str_radix(&digest[..16], 16).expect("hex digest")
}

/// One epoch's visiting order: ascending SHA-256(compact JSON `[seed,
/// epoch, example_id]`).
pub fn epoch_order(seed: &str, epoch: u64, example_ids: &[&str]) -> Vec<usize> {
    let mut keyed: Vec<(String, usize)> = example_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (compact_digest(&serde_json::json!([seed, epoch, id])), index))
        .collect();
    keyed.sort_unstable();
    keyed.into_iter().map(|(_, index)| index).collect()
}

/// The pre-fit permission gate (contract 106-110, 116-118): every intended
/// contribution's CURRENT row must still be permitted with the identical
/// input, label and permission (consent plus rights assertion). A valid
/// export is not current consent.
fn permission_gate(
    engine: &Engine,
    coverage: &BTreeMap<String, (String, Contribution)>,
    inherited: &[&Contribution],
    control: &Control,
) -> FResult<()> {
    let check = |contribution: &Contribution,
                 group: Option<&str>,
                 code: &'static str|
     -> FResult<()> {
        let id = &contribution.example_id;
        let Some(current) = engine.learning_current_row(id)? else {
            return Err(fail(
                code,
                format!("example {id} is no longer in the store"),
            ));
        };
        let changed = if !current.allow_training {
            Some("its training consent was withdrawn")
        } else if current.permission_sha256() != contribution.permission_sha256 {
            Some("its permission (rights assertion) changed")
        } else if current.input_sha256() != contribution.input_sha256 {
            Some("its exact input changed")
        } else if current.correct_option_id != contribution.correct_option_id {
            Some("its label changed")
        } else if group.is_some_and(|group| current.task_group_id != group) {
            Some("its group changed")
        } else {
            None
        };
        match changed {
            Some(what) => Err(fail(
                code,
                format!("example {id}: {what} since the dataset was prepared; nothing was trained"),
            )),
            None => Ok(()),
        }
    };
    for (group, contribution) in coverage.values() {
        control.check()?;
        check(contribution, Some(group), "contribution_changed")?;
    }
    for contribution in inherited {
        control.check()?;
        check(contribution, None, "base_permission_changed")?;
    }
    Ok(())
}

/// Union of a lineage's new and inherited contributions, by example ID.
fn lineage_union(lineage: &Lineage) -> BTreeMap<String, Contribution> {
    lineage
        .new
        .iter()
        .chain(&lineage.inherited)
        .map(|c| (c.example_id.clone(), c.clone()))
        .collect()
}

/// `learning train`.
pub fn train(engine: &Engine, request: &TrainRequest<'_>, control: &Control) -> FResult<Trained> {
    control.check()?;
    if !request.development_isolation {
        return Err(fail(
            "isolation_unavailable",
            "normal training admission stays closed until platform isolation acceptance; pass \
             --development-isolation under the owner's authorization (never production isolation)",
        ));
    }
    let workspace_id = engine
        .workspace_id()
        .ok_or(FoundryError::WorkspaceUnbound)?;
    let (policy, policy_sha256) = LearningPolicy::load(request.policy)?;
    let loaded = load_renderer(&policy)?;
    let model_function = model_function_of(&policy, &loaded);

    // 1. The dataset, read back exactly, bound to this policy.
    let opened = open_dataset(request.input)?;
    if opened.manifest.workspace_id != workspace_id {
        return Err(fail(
            "dataset_invalid",
            "the dataset belongs to another workspace",
        ));
    }
    if opened.manifest.model_function_sha256 != model_function {
        return Err(fail(
            "dataset_invalid",
            "the dataset was prepared for another model function than the policy pins",
        ));
    }
    if opened.manifest.policy_sha256 != policy_sha256 {
        return Err(fail(
            "policy_mismatch",
            format!(
                "the dataset was prepared under policy {}; this policy is {policy_sha256}: one \
                 policy file serves prepare and train",
                opened.manifest.policy_sha256
            ),
        ));
    }
    let mut kept: Vec<(&'static str, super::DatasetRow)> = Vec::new();
    let dataset = verify_dataset_with(opened, &loaded, control, &mut |split, row| {
        kept.push((split, row));
    })?;
    verify_lineage(engine, &dataset, control)?;

    // 2. Profile, base, incumbent, rows and output — all before launch.
    let profile = LearnProfile::load(&policy.isolation_profile)?;
    profile.admit(&policy)?;
    let base = open_base(
        &policy,
        request.base,
        &dataset,
        &model_function,
        &workspace_id,
        control,
    )?;
    let incumbent = request
        .incumbent
        .map(|path| open_incumbent(path, &model_function, &workspace_id, control))
        .transpose()?;
    let mut train_rows = Vec::new();
    let mut calibration_rows = Vec::new();
    let mut evaluation_rows = Vec::new();
    for (split, row) in kept {
        match split {
            "train" => train_rows.push(row_of(row, None)),
            "calibration" => calibration_rows.push(row_of(row, None)),
            _ => {
                let baseline = eval::baseline_choice(&row.feedback.state)?;
                evaluation_rows.push(row_of(row, Some(baseline)));
            }
        }
    }
    if train_rows.is_empty() {
        return Err(fail("dataset_invalid", "the dataset has no training row"));
    }
    if calibration_rows.is_empty() {
        return Err(fail(
            "calibration_failed",
            "the dataset has no calibration row; there is nothing to fit a temperature on",
        ));
    }
    if evaluation_rows.is_empty() {
        return Err(fail("dataset_invalid", "the dataset has no evaluation row"));
    }
    for group in &policy.selection.critical_groups {
        if !evaluation_rows.iter().any(|row| row.group_id == *group) {
            return Err(fail(
                "critical_slice_empty",
                format!("critical group {group} has no evaluation row"),
            ));
        }
    }
    let dataset_sha = dataset.manifest_sha256.clone();
    let policy_for_adopt = policy_sha256.clone();
    let target = OutputTarget::open(engine, request.out, &|bytes| {
        // An occupied destination may be this run's own earlier candidate
        // (lost response) only if it is a candidate of this exact dataset
        // and policy; everything is validated before any adoption.
        Ok(
            candidate::parse_manifest(bytes, "candidate_invalid").is_ok_and(|m| {
                m.dataset_manifest_sha256 == dataset_sha && m.policy_sha256 == policy_for_adopt
            }),
        )
    })?;

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (
            target,
            base,
            incumbent,
            train_rows,
            calibration_rows,
            evaluation_rows,
            profile,
        );
        return Err(fail(
            "isolation_unavailable",
            "the learning worker's isolation profile targets macOS",
        ));
    }
    #[cfg(target_os = "macos")]
    {
        let inherited: Vec<Contribution> = base
            .as_ref()
            .map(|(_, verified)| {
                lineage_union(&verified.contributions.fitting)
                    .into_values()
                    .chain(lineage_union(&verified.contributions.calibration).into_values())
                    .collect()
            })
            .unwrap_or_default();
        let run = macos::run(
            engine,
            &macos::Plan {
                policy: &policy,
                profile: &profile,
                model_function: &model_function,
                dataset: &dataset,
                inherited: &inherited,
                base: base.as_ref(),
                incumbent: incumbent.as_ref(),
                train: &train_rows,
                calibration: &calibration_rows,
                evaluation: &evaluation_rows,
            },
            control,
        )?;
        publish(
            engine,
            request,
            &target,
            Publishing {
                policy: &policy,
                policy_sha256: &policy_sha256,
                loaded: &loaded,
                workspace_id,
                model_function,
                dataset: &dataset,
                base: base.as_ref(),
                incumbent: incumbent.as_ref(),
                train: &train_rows,
                calibration: &calibration_rows,
                evaluation: &evaluation_rows,
                run,
            },
            control,
        )
    }
}

/// The base candidate a policy pins (`--base` must name it), fully read
/// back: this workspace and model function, eligible, trained on the input
/// dataset's parent, carrying one valid fitted scalar. With `base: null`
/// the pinned initial checkpoint is the base and `--base` is refused.
fn open_base(
    policy: &LearningPolicy,
    path: Option<&Path>,
    dataset: &VerifiedDataset,
    model_function: &str,
    workspace_id: &str,
    control: &Control,
) -> FResult<Option<(Dir, candidate::VerifiedCandidate)>> {
    let invalid = |message: String| Err(fail("base_invalid", message));
    let (pinned, path) = match (&policy.base, path) {
        (None, None) => return Ok(None),
        (None, Some(_)) => {
            return invalid(
                "the policy pins the initial checkpoint as the base (base: null); --base names \
                 another"
                    .into(),
            );
        }
        (Some(_), None) => {
            return invalid("the policy pins a base candidate; name it with --base".into());
        }
        (Some(pinned), Some(path)) => (pinned, path),
    };
    let (dir, verified) = candidate::open(path, "base_invalid", control)?;
    if verified.manifest_sha256 != *pinned {
        return invalid(format!(
            "--base is candidate {}; the policy pins {pinned}",
            verified.manifest_sha256
        ));
    }
    let m = &verified.manifest;
    if m.workspace_id != workspace_id || m.model_function_sha256 != model_function {
        return invalid("the base belongs to another workspace or model function".into());
    }
    if !m.eligible {
        return invalid("the base candidate is not eligible; no rejected base is used".into());
    }
    if dataset.manifest.parent_manifest_sha256.as_deref()
        != Some(m.dataset_manifest_sha256.as_str())
    {
        return invalid(
            "the base candidate was not trained on this dataset's parent dataset".into(),
        );
    }
    Ok(Some((dir, verified)))
}

/// An explicitly requested incumbent must exist and be compatible: this
/// workspace and model function, fully valid. Never dropped silently.
fn open_incumbent(
    path: &Path,
    model_function: &str,
    workspace_id: &str,
    control: &Control,
) -> FResult<(Dir, candidate::VerifiedCandidate)> {
    let (dir, verified) = candidate::open(path, "incumbent_invalid", control)?;
    let m = &verified.manifest;
    if m.workspace_id != workspace_id || m.model_function_sha256 != model_function {
        return Err(fail(
            "incumbent_invalid",
            "the incumbent belongs to another workspace or model function",
        ));
    }
    Ok((dir, verified))
}

/// What the worker produced, checked by the core.
pub(super) struct Run {
    frozen_encoder_sha256: String,
    steps: u64,
    epochs: u64,
    first_loss: f64,
    last_loss: f64,
    clipped: u64,
    calibration: eval::Calibration,
    evaluation: Vec<[f32; 2]>,
    incumbent: Option<Vec<[f32; 2]>>,
    probe: [f32; 2],
    head: std::fs::File,
    head_sha256: String,
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use crate::learning::ipc::HeadSlot;
    use crate::learning::supervisor::LearnWorker;
    use std::time::{Duration, Instant};

    pub(super) struct Plan<'a> {
        pub policy: &'a LearningPolicy,
        pub profile: &'a LearnProfile,
        pub model_function: &'a str,
        pub dataset: &'a VerifiedDataset,
        pub inherited: &'a [Contribution],
        pub base: Option<&'a (Dir, candidate::VerifiedCandidate)>,
        pub incumbent: Option<&'a (Dir, candidate::VerifiedCandidate)>,
        pub train: &'a [Row],
        pub calibration: &'a [Row],
        pub evaluation: &'a [Row],
    }

    /// Place a verified candidate's head in the worker's scratch run
    /// directory under `slot`; its digest must be the manifest's.
    fn stage_head(
        worker: &LearnWorker,
        from: &(Dir, candidate::VerifiedCandidate),
        slot: HeadSlot,
        code: &'static str,
    ) -> FResult<(HeadSlot, String)> {
        let sha = worker.stage_head(&from.0, slot)?;
        let expected = from
            .1
            .manifest
            .files
            .iter()
            .find(|f| f.name == candidate::HEAD)
            .map(|f| f.sha256.as_str());
        if expected != Some(sha.as_str()) {
            return Err(fail(
                code,
                "the candidate's head changed while it was staged",
            ));
        }
        Ok((slot, sha))
    }

    /// Calibration-phase failures of the model are calibration failures.
    fn in_calibration(error: FoundryError) -> FoundryError {
        match error.code() {
            "nonfinite_logits" => fail(
                "calibration_failed",
                format!("a calibration logit was refused: {error}"),
            ),
            _ => error,
        }
    }

    pub(super) fn run(engine: &Engine, plan: &Plan<'_>, control: &Control) -> FResult<Run> {
        let policy = plan.policy;
        let wall_deadline = Instant::now() + Duration::from_secs(policy.wall_seconds);
        let mut worker = LearnWorker::launch(plan.profile, policy, wall_deadline, control)?;
        let seed = dropout_seed(&policy.seed);
        let head = plan
            .base
            .map(|base| stage_head(&worker, base, HeadSlot::Base, "base_invalid"))
            .transpose()?;
        let loaded = worker.load(policy, plan.model_function, head, seed, control)?;
        let frozen_before = worker.frozen_hash(control)?;
        if frozen_before != loaded.frozen_encoder_sha256 {
            return Err(fail(
                "frozen_encoder_changed",
                "the frozen encoder digest changed between load and its first check",
            ));
        }
        // Immediately before fitting, under the ownership held since the
        // command began.
        let inherited: Vec<&Contribution> = plan.inherited.iter().collect();
        permission_gate(engine, &plan.dataset.coverage, &inherited, control)?;

        let ids: Vec<&str> = plan
            .train
            .iter()
            .map(|row| row.contribution.example_id.as_str())
            .collect();
        let (mut steps, mut epochs, mut clipped) = (0u64, 0u64, 0u64);
        let (mut first_loss, mut last_loss) = (f64::NAN, f64::NAN);
        'fit: loop {
            let order = epoch_order(&policy.seed, epochs, &ids);
            epochs += 1;
            for index in order {
                if steps == policy.max_steps {
                    break 'fit;
                }
                learning_fault!(BEFORE_STEP, control, &steps.to_string())?;
                let row = &plan.train[index];
                let (loss, norm) = worker.step(&row.ids, row.markers, row.target, control)?;
                if steps == 0 {
                    first_loss = loss;
                }
                last_loss = loss;
                if norm > crate::decision_model::recipe::CLIP_GLOBAL_NORM {
                    clipped += 1;
                }
                steps += 1;
            }
            if steps == policy.max_steps {
                break;
            }
        }
        let frozen_after = worker.frozen_hash(control)?;
        if frozen_after != frozen_before {
            return Err(fail(
                "frozen_encoder_changed",
                format!(
                    "the frozen encoder digest changed during training ({frozen_before} -> \
                     {frozen_after})"
                ),
            ));
        }
        let mut calibration_logits = Vec::with_capacity(plan.calibration.len());
        for row in plan.calibration {
            let logits = worker
                .logits(&row.ids, row.markers, control)
                .map_err(in_calibration)?;
            calibration_logits.push((logits, row.target as usize));
        }
        let calibration = eval::fit_temperature(&calibration_logits)?;
        let mut evaluation = Vec::with_capacity(plan.evaluation.len());
        for row in plan.evaluation {
            evaluation.push(worker.logits(&row.ids, row.markers, control)?);
        }
        // Save and reload: the reloaded head must give the probe exactly
        // the logits training-eval gave it.
        let probe_row = &plan.evaluation[0];
        let saved = worker.save(&probe_row.ids, probe_row.markers, control)?;
        if saved.values != evaluation[0] {
            return Err(fail(
                "artifact_invalid",
                format!(
                    "the saved-and-reloaded head gives the probe {:?}; training-eval gave {:?}",
                    saved.values, evaluation[0]
                ),
            ));
        }
        if saved.bytes > policy.output_bytes {
            return Err(fail(
                "output_limit",
                format!(
                    "head.safetensors is {} bytes; the output ceiling is {}",
                    saved.bytes, policy.output_bytes
                ),
            ));
        }
        let head = worker
            .scratch()
            .open_file(candidate::HEAD)
            .map_err(|e| fail("artifact_invalid", format!("{}: {e}", candidate::HEAD)))?;
        let check = head
            .try_clone()
            .map_err(|e| fail("artifact_invalid", format!("{}: {e}", candidate::HEAD)))?;
        let head_sha256 =
            candidate::validate_head(check, saved.bytes, "artifact_invalid", control)?;
        if head_sha256 != saved.sha256 {
            return Err(fail(
                "artifact_invalid",
                "head.safetensors does not hash to the digest the worker reported",
            ));
        }
        let incumbent = match plan.incumbent {
            Some(incumbent) => {
                let staged =
                    stage_head(&worker, incumbent, HeadSlot::Incumbent, "incumbent_invalid")?;
                worker.load(policy, plan.model_function, Some(staged), seed, control)?;
                let mut outputs = Vec::with_capacity(plan.evaluation.len());
                for row in plan.evaluation {
                    outputs.push(worker.logits(&row.ids, row.markers, control)?);
                }
                Some(outputs)
            }
            None => None,
        };
        // The checked end: the run is publishable only if the worker ended
        // cleanly with nothing after its last reply and no late breach.
        worker.finish(control)?;
        Ok(Run {
            frozen_encoder_sha256: frozen_before,
            steps,
            epochs,
            first_loss,
            last_loss,
            clipped,
            calibration,
            evaluation,
            incumbent,
            probe: saved.values,
            head,
            head_sha256,
        })
    }
}

struct Publishing<'a> {
    policy: &'a LearningPolicy,
    policy_sha256: &'a str,
    loaded: &'a super::LoadedRenderer,
    workspace_id: String,
    model_function: String,
    dataset: &'a VerifiedDataset,
    base: Option<&'a (Dir, candidate::VerifiedCandidate)>,
    incumbent: Option<&'a (Dir, candidate::VerifiedCandidate)>,
    train: &'a [Row],
    calibration: &'a [Row],
    evaluation: &'a [Row],
    run: Run,
}

fn cases_of(rows: &[Row]) -> Vec<CaseInput> {
    rows.iter()
        .map(|row| CaseInput {
            example_id: row.contribution.example_id.clone(),
            group_id: row.group_id.clone(),
            option_ids: row.option_ids.clone(),
            expected: row.expected.clone(),
            baseline: row.baseline.unwrap_or("search").to_owned(),
            permission_sha256: row.contribution.permission_sha256.clone(),
        })
        .collect()
}

/// Evaluate, build and publish the candidate, then read it back.
fn publish(
    _engine: &Engine,
    request: &TrainRequest<'_>,
    target: &OutputTarget,
    p: Publishing<'_>,
    control: &Control,
) -> FResult<Trained> {
    let Publishing {
        policy,
        policy_sha256,
        loaded,
        workspace_id,
        model_function,
        dataset,
        base,
        incumbent,
        train,
        calibration,
        evaluation,
        run,
    } = p;
    let cases = cases_of(evaluation);
    let outputs: Vec<Output> = run.evaluation.iter().map(|z| Output::Logits(*z)).collect();
    let incumbent_outputs: Option<Vec<Output>> = run
        .incumbent
        .as_ref()
        .map(|z| z.iter().map(|z| Output::Logits(*z)).collect());
    let report = eval::evaluate(
        &cases,
        &outputs,
        run.calibration.temperature,
        incumbent
            .zip(incumbent_outputs.as_deref())
            .map(|((_, verified), outputs)| eval::Incumbent {
                outputs,
                temperature: verified.manifest.temperature,
            }),
        &policy.selection,
    );
    let inherited_fitting = base
        .map(|(_, verified)| lineage_union(&verified.contributions.fitting))
        .unwrap_or_default();
    let inherited_calibration = base
        .map(|(_, verified)| lineage_union(&verified.contributions.calibration))
        .unwrap_or_default();
    let fitting_new: BTreeMap<String, Contribution> = train
        .iter()
        .filter(|row| !inherited_fitting.contains_key(&row.contribution.example_id))
        .map(|row| {
            (
                row.contribution.example_id.clone(),
                row.contribution.clone(),
            )
        })
        .collect();
    let calibration_new: BTreeMap<String, Contribution> = calibration
        .iter()
        .map(|row| {
            (
                row.contribution.example_id.clone(),
                row.contribution.clone(),
            )
        })
        .collect();
    let contributions = Contributions {
        fitting: Lineage {
            new: fitting_new.into_values().collect(),
            inherited: inherited_fitting.into_values().collect(),
        },
        calibration: Lineage {
            new: calibration_new.into_values().collect(),
            inherited: inherited_calibration.into_values().collect(),
        },
    };
    let distinct_train: BTreeSet<&str> = train
        .iter()
        .map(|row| row.contribution.example_id.as_str())
        .collect();
    let manifest = CandidateManifest {
        schema: 4,
        kind: candidate::KIND.to_owned(),
        recipe: super::RECIPE.to_owned(),
        workspace_id,
        model_function_sha256: model_function,
        dataset_manifest_sha256: dataset.manifest_sha256.clone(),
        dataset_id: dataset.manifest.dataset_id.clone(),
        policy_sha256: policy_sha256.to_owned(),
        base: match base {
            Some((_, verified)) => Base::Candidate {
                manifest_sha256: verified.manifest_sha256.clone(),
            },
            None => Base::Initial,
        },
        encoder: candidate::EncoderIdentity {
            checkpoint: policy.model.clone(),
            frozen_encoder_sha256: run.frozen_encoder_sha256.clone(),
        },
        tokenizer: candidate::TokenizerIdentity {
            json_sha256: loaded.json_sha.clone(),
            config_sha256: loaded.config_sha.clone(),
        },
        training: candidate::Training {
            seed: policy.seed.clone(),
            max_steps: policy.max_steps,
            steps_completed: run.steps,
            epochs: run.epochs,
            train_rows: distinct_train.len(),
            optimizer: policy.optimizer.clone(),
            head_dropout: HEAD_DROPOUT,
            batch: 1,
            first_loss: run.first_loss,
            last_loss: run.last_loss,
            clipped_steps: run.clipped,
        },
        temperature: run.calibration.temperature,
        calibration_rows: run.calibration.rows,
        calibration_mean_nll: run.calibration.mean_nll,
        evaluation_rows: report.rows,
        eligible: report.eligibility.eligible,
        probe: candidate::Probe {
            example_id: evaluation[0].contribution.example_id.clone(),
            logits: run.probe,
        },
        files: Vec::new(),
    };
    let pre_export: Vec<Option<[f64; 2]>> = report.cases.iter().map(|c| c.probabilities).collect();
    let eligible = report.eligibility.eligible;
    let reasons = report.eligibility.reasons.clone();
    let (partial, guard) = target.create_partial()?;
    let manifest_bytes = candidate::write(
        &partial,
        candidate::Draft {
            head: run.head,
            head_sha256: run.head_sha256,
            report,
            contributions,
            manifest,
        },
        policy.output_bytes,
        control,
    )?;
    let manifest_sha256 = crate::digest(&manifest_bytes);
    control.check()?;
    let adopted = match target.publish(&guard)? {
        Publication::Published => false,
        Publication::Occupied => {
            // Adopt only an occupied destination that IS this candidate:
            // identical manifest bytes and every member validated in full
            // (lengths, digests, head tensors and values, report). The head's
            // digest equals the one this run's worker saved and reload-
            // checked, so the reload check covers those exact bytes.
            let occupied = target
                .parent
                .open_dir(&target.name)
                .ok()
                .flatten()
                .ok_or_else(|| {
                    fail("output_exists", "the output destination is not a directory")
                })?;
            match candidate::verify_dir(&occupied, "output_exists", control) {
                Ok(verified) if verified.manifest_sha256 == manifest_sha256 => {}
                Err(e @ (FoundryError::Cancelled(_) | FoundryError::DeadlineExceeded(_))) => {
                    return Err(e);
                }
                _ => {
                    return Err(fail(
                        "output_exists",
                        format!(
                            "output destination {} holds something other than this candidate; \
                             it was left untouched",
                            request.out.display()
                        ),
                    ));
                }
            }
            target.sync_parent()?;
            true
        }
    };
    drop(guard);
    learning_fault!(CANDIDATE_PUBLISHED, control, &manifest_sha256)?;
    // Read the published candidate back and require identical calibrated
    // probabilities: the exported scalar and logits reproduce exactly what
    // the evaluation computed before export.
    let published = target
        .parent
        .open_dir(&target.name)
        .ok()
        .flatten()
        .ok_or_else(|| fail("artifact_invalid", "the published candidate vanished"))?;
    let verified = candidate::verify_dir(&published, "artifact_invalid", control)?;
    if verified.manifest_sha256 != manifest_sha256 {
        return Err(fail(
            "artifact_invalid",
            "the published candidate's manifest is not the one this run wrote",
        ));
    }
    for (case, before) in verified.report.cases.iter().zip(&pre_export) {
        let after = case
            .logits
            .map(|z| eval::softmax(z, verified.manifest.temperature));
        if after != *before || case.probabilities != *before {
            return Err(fail(
                "artifact_invalid",
                format!(
                    "example {}: calibrated probabilities differ after export and reload",
                    case.example_id
                ),
            ));
        }
    }
    Ok(Trained {
        outcome: "completed",
        candidate: request.out.to_path_buf(),
        candidate_manifest_sha256: manifest_sha256,
        eligible,
        reasons,
        steps_completed: run.steps,
        temperature: verified.manifest.temperature,
        adopted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epochs_visit_every_row_once_in_a_seeded_order() {
        let ids = ["a", "b", "c", "d", "e"];
        let first = epoch_order("seed", 0, &ids);
        let mut sorted = first.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3, 4]);
        assert_eq!(first, epoch_order("seed", 0, &ids), "deterministic");
        assert_ne!(first, epoch_order("seed", 1, &ids), "each epoch reshuffles");
        assert_ne!(first, epoch_order("other", 0, &ids), "the seed decides");
        assert_ne!(dropout_seed("seed"), dropout_seed("other"));
    }
}
