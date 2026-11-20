//! The run policy (v2) that `learning prepare` and `learning train` share:
//! its fields, the optimizer and tokenizer pins, the enforcement levels, the
//! selection policy, the bounds and the strict load that validates them.
//! The dataset types and the renderer stay in the parent module.
use super::*;

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

/// The run policy (v2): one file serves `prepare` and `train`. T001 reads
/// recipe, seed and the pinned tokenizer identity; T002 adds the fitting
/// fields (contract § Fitting, artifacts and evaluation, 158-168) and the
/// selection policy (200-202). The dataset manifest binds the policy's
/// exact bytes by `policy_sha256`, and training refuses a dataset prepared
/// under any other policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningPolicy {
    pub v: u32,
    pub recipe: String,
    /// The seed string ordering base-replay selection, each epoch's training
    /// order and the head dropout.
    pub seed: String,
    pub tokenizer: TokenizerPin,
    /// The checkpoint the model function pins (its location is the isolation
    /// profile's `checkpoint_dir`, which the worker is granted).
    pub model: decision_model::CheckpointPin,
    /// `null`: the pinned initial checkpoint is the base. Otherwise the
    /// SHA-256 of the exact manifest bytes of the accepted candidate that
    /// `train --base` must name.
    pub base: Option<String>,
    pub optimizer: OptimizerPin,
    /// Updates in this round, 1..=1000000; each epoch visits the training
    /// rows in seeded order until this many steps are done.
    pub max_steps: u64,
    /// The wall clock of the whole fitting run, 1..=7200 seconds.
    pub wall_seconds: u64,
    /// The worker's supervised physical-footprint ceiling, 4..=8 GiB.
    pub memory_bytes: u64,
    /// The worker's output ceiling (per file and in total), 128 MiB..=2 GiB.
    pub output_bytes: u64,
    /// LibTorch intra-op threads, 1..=4.
    pub cpu_threads: u32,
    /// Absolute path of the learning-worker isolation profile.
    pub isolation_profile: PathBuf,
    /// Requested enforcement per resource; never downgraded.
    pub enforcement: Enforcement,
    #[serde(default)]
    pub selection: SelectionPolicy,
}

/// The recipe's optimizer, stated in the policy; it must equal
/// [`decision_model::recipe`] exactly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptimizerPin {
    pub name: String,
    pub learning_rate: f64,
    pub beta1: f64,
    pub beta2: f64,
    pub epsilon: f64,
    pub weight_decay: f64,
    pub clip_global_norm: f64,
}

impl OptimizerPin {
    /// The recipe's constants.
    pub fn recipe() -> Self {
        use decision_model::recipe::*;
        Self {
            name: OPTIMIZER.to_owned(),
            learning_rate: LEARNING_RATE,
            beta1: BETA1,
            beta2: BETA2,
            epsilon: EPSILON,
            weight_decay: WEIGHT_DECAY,
            clip_global_norm: CLIP_GLOBAL_NORM,
        }
    }
}

/// An enforcement level. `hard` is a kernel limit the worker cannot exceed;
/// `supervised` is the owner measuring and stopping the worker. A request
/// is satisfied only by the level requested or a stronger one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Supervised,
    Hard,
}

/// Requested enforcement per resource (contract 167-169).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enforcement {
    pub memory: Level,
    pub cpu: Level,
    pub output: Level,
    pub process_count: Level,
}

/// The predeclared selection policy (contract 200-202): fractions finite
/// in [0, 1]; defaults for the initial trial 0.8 / 0.5 / 0.9 / 0 and no
/// critical group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SelectionPolicy {
    /// Accept a prediction exactly when its unrounded answer confidence is
    /// at least this.
    pub threshold: f64,
    /// Accepted rows over all evaluation rows must reach this.
    pub coverage_floor: f64,
    /// Correct accepted rows over accepted rows must reach this.
    pub accepted_accuracy_floor: f64,
    /// The candidate's macro accuracy may trail either comparator by at
    /// most this (also per critical group).
    pub max_macro_accuracy_drop: f64,
    /// Evaluation group IDs that must each be present and must not regress
    /// past the drop allowance.
    pub critical_groups: Vec<String>,
}

impl Default for SelectionPolicy {
    fn default() -> Self {
        Self {
            threshold: 0.8,
            coverage_floor: 0.5,
            accepted_accuracy_floor: 0.9,
            max_macro_accuracy_drop: 0.0,
            critical_groups: Vec::new(),
        }
    }
}

/// Policy bounds (contract 163, 167-168).
pub const MAX_STEPS: u64 = 1_000_000;
pub const MAX_WALL_SECONDS: u64 = 7200;
pub const MIN_MEMORY_BYTES: u64 = 4 << 30;
pub const MAX_MEMORY_BYTES: u64 = 8 << 30;
pub const MIN_OUTPUT_BYTES: u64 = 128 << 20;
pub const MAX_OUTPUT_BYTES: u64 = 2 << 30;
pub const MAX_CPU_THREADS: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenizerPin {
    /// Directory holding `tokenizer.json` and `tokenizer_config.json`.
    pub dir: PathBuf,
    pub json_sha256: String,
    pub config_sha256: String,
}

fn fraction(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

impl LearningPolicy {
    /// Strict load plus the exact-bytes digest the manifest binds. The file
    /// is opened without following a link and refused by its length before
    /// it is read.
    pub fn load(path: &Path) -> FResult<(Self, String)> {
        let unavailable = |e: std::io::Error| {
            FoundryError::ArtifactUnavailable(format!("policy {}: {e}", path.display()))
        };
        let raw = read_capped(open_regular(path).map_err(unavailable)?, POLICY_MAX_BYTES)
            .map_err(unavailable)?
            .ok_or_else(|| fail("policy_invalid", "policy exceeds 64 KiB"))?;
        let policy: LearningPolicy = strict_json(&raw, "policy_invalid", "policy")?;
        if policy.v != 2 {
            return Err(fail(
                "policy_invalid",
                format!("policy v must be 2, not {}", policy.v),
            ));
        }
        if policy.recipe != RECIPE {
            return Err(fail(
                "policy_invalid",
                format!("unknown recipe {:?}; expected {RECIPE}", policy.recipe),
            ));
        }
        if policy.seed.trim().is_empty() || policy.seed.len() > 256 {
            return Err(fail(
                "policy_invalid",
                "seed must be nonblank and at most 256 bytes",
            ));
        }
        for (name, hash) in [
            ("json_sha256", &policy.tokenizer.json_sha256),
            ("config_sha256", &policy.tokenizer.config_sha256),
        ] {
            if !crate::is_lower_hex(hash, 64) {
                return Err(fail(
                    "policy_invalid",
                    format!("tokenizer.{name} must be 64 lowercase hex characters"),
                ));
            }
        }
        policy.validate_fitting()?;
        Ok((policy, crate::digest(&raw)))
    }

    /// The T002 fields: exact checkpoint pins, the recipe's optimizer, every
    /// bound (refused, never clamped), an absolute profile path and the
    /// selection fractions.
    fn validate_fitting(&self) -> FResult<()> {
        let invalid = |message: String| Err(fail("policy_invalid", message));
        for (name, hash) in [
            ("model.weights_sha256", &self.model.weights_sha256),
            (
                "model.encoder_config_sha256",
                &self.model.encoder_config_sha256,
            ),
        ] {
            if !crate::is_lower_hex(hash, 64) {
                return invalid(format!("{name} must be 64 lowercase hex characters"));
            }
        }
        if decision_model::safetensors::Dtype::parse(&self.model.source_dtype).is_none() {
            return invalid(format!(
                "model.source_dtype {:?} is not F16, BF16 or F32",
                self.model.source_dtype
            ));
        }
        if let Some(base) = &self.base
            && !crate::is_lower_hex(base, 64)
        {
            return invalid("base must be null or a 64-hex candidate manifest SHA-256".into());
        }
        if self.optimizer != OptimizerPin::recipe() {
            return invalid(format!(
                "optimizer must be exactly the {RECIPE} recipe: {:?}",
                OptimizerPin::recipe()
            ));
        }
        let bounded = |name: &str, value: u64, low: u64, high: u64| -> FResult<()> {
            if !(low..=high).contains(&value) {
                return Err(fail(
                    "policy_invalid",
                    format!("{name} is {value}; it must be {low}..={high}"),
                ));
            }
            Ok(())
        };
        bounded("max_steps", self.max_steps, 1, MAX_STEPS)?;
        bounded("wall_seconds", self.wall_seconds, 1, MAX_WALL_SECONDS)?;
        bounded(
            "memory_bytes",
            self.memory_bytes,
            MIN_MEMORY_BYTES,
            MAX_MEMORY_BYTES,
        )?;
        bounded(
            "output_bytes",
            self.output_bytes,
            MIN_OUTPUT_BYTES,
            MAX_OUTPUT_BYTES,
        )?;
        bounded(
            "cpu_threads",
            u64::from(self.cpu_threads),
            1,
            u64::from(MAX_CPU_THREADS),
        )?;
        if !self.isolation_profile.is_absolute()
            || self
                .isolation_profile
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return invalid("isolation_profile must be an absolute path without `..`".into());
        }
        let selection = &self.selection;
        for (name, value) in [
            ("threshold", selection.threshold),
            ("coverage_floor", selection.coverage_floor),
            ("accepted_accuracy_floor", selection.accepted_accuracy_floor),
            ("max_macro_accuracy_drop", selection.max_macro_accuracy_drop),
        ] {
            if !fraction(value) {
                return invalid(format!("selection.{name} must be finite in [0, 1]"));
            }
        }
        let mut seen = BTreeSet::new();
        if selection.critical_groups.len() > MAX_GROUPS {
            return invalid("selection.critical_groups lists too many groups".into());
        }
        for group in &selection.critical_groups {
            if group.trim().is_empty() || group.len() > 256 || !seen.insert(group.as_str()) {
                return invalid(
                    "selection.critical_groups must be distinct nonblank ids of at most 256 bytes"
                        .into(),
                );
            }
        }
        Ok(())
    }
}
