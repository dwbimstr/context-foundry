//! 009 bounded semantic preparation under exclusive ownership
//! (`foundry semantic prepare --profile FILE --budget-seconds N
//! [--development-isolation]`).
//!
//! `--budget-seconds` bounds the WHOLE command, publication included. The
//! budget counts from argument parsing: profile verification, worker start,
//! loading, partitioning, inference and index publication are all inside it.
//! ONE run [`Control`] carries the caller's cancel flag plus the budget
//! deadline; it reaches artifact verification, acquisition, every document
//! call and publication unchanged (the supervisor owns any in-flight grace —
//! nothing here adds another), and every admission checks it first.
//!
//! Publication shares the budget. New batches stop being admitted when the
//! remaining time falls below a publication reserve of max(5 s, budget/10);
//! publication then runs under the run control. If it cannot finish, the run
//! reports `budget_exhausted`, the committed vectors stay cached and the
//! report says `index_published = false` WITH the reason. Every run first
//! publishes pending committed coverage (a validated generation replayed from
//! the f32 cache, zero inference) BEFORE admitting new inference, and again
//! after a run committed new vectors — even when it stopped early, so partial
//! coverage is searchable.
//!
//! Work pages current sources in pages of 128, embeds at most the profile's
//! batch of cards per call with ONE batch outstanding, and commits the cache
//! per batch (the committed counts move in the same transaction). A
//! stop caused by the budget is reported as `budget_exhausted` whatever the
//! provider returned. Startup never resumes preparation: only this explicit
//! command (or the MCP owner's explicit `index {semantic: "prepare"}`) does.
//!
//! 009 T003: the steps are shared with the MCP owner's background driver
//! ([`super::driver`]), which composes the SAME functions between its own
//! engine-slot holds: [`Steps::partition_inputs_in`], [`commit_partition`], [`Steps::select_batch`],
//! [`Cards::render`], [`commit_batch`], [`publish`], [`begin`] and
//! [`finalize`]. Only the ownership around them differs: the CLI holds the
//! store exclusively and calls the provider directly under its budget; the
//! driver takes the owner's engine slot per store step and embeds through
//! the resident runtime. Both prepare at most eight source partitions in
//! parallel outside transactions; the driver releases its engine slot too.
//!
//! 009 T004: the embedding inputs are cards ([`super::partition`]). A
//! partition is accepted only as the cards its verified body renders, in
//! the acceptance transaction. A batch is selected in a store step (which
//! cards are missing, with their recorded tuples and their source's body)
//! and only those cards are needed by [`Cards::render`]. A bounded wave
//! handoff reuses fresh tokenized cards; other selections render on demand.
//! Both paths check each card against its recorded tuple with no engine or
//! transaction, so the driver can prepare the next batch while inference runs.
//!
//! The provider comes through the [`Acquire`] seam (slice B's supervised
//! worker); whenever no accepted isolation profile admits model execution it
//! returns `isolation_unavailable` and source access stays intact. The state
//! the run observed (ready / refused / failed) is recorded with its time and
//! profile for `status`; there is no live probe.
use crate::control::Control;
use crate::error::{FResult, FoundryError};
use crate::neural::cache::{self, CacheLookup, PartitionUnit, ProviderObservation, StateError};
use crate::neural::index::Publication;
use crate::neural::partition::{self, CardRecipe, TokenCount};
use crate::neural::profile::SemanticProfile;
use crate::neural::provider::{
    self, DocumentLimits, EmbeddingProvider, ProviderError, TokenizedInput,
};
use crate::neural::tokenize::DocumentTokenizer;
use crate::store::Engine;
use crate::syntax::{Lang, Unit};
use rayon::prelude::*;
use serde::Serialize;
use std::cell::OnceCell;
use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A wave retains at most eight verified source bodies (16 MiB maximum).
pub(crate) const PARTITION_WAVE: usize = 8;
/// Yield a contended source wave after this many selected inputs. A large file
/// must not monopolize inference while other requested areas wait.
pub(crate) const WAVE_INPUTS: usize = 8;
/// Retained card payload per source, including allocated ids/key/array capacity.
/// At most one eight-source wave survives into selection. Larger sources use
/// the existing bounded on-demand render path instead of retaining their cards.
const RETAINED_CARD_BYTES: usize = 2 * 1024 * 1024;
/// One shared budget for paused hierarchy bodies/plans across all cursors.
/// Token arrays never enter this cache. Eviction preserves scalar progress.
const PAUSED_PLAN_BYTES: usize = 16 * 1024 * 1024;
const PAUSED_PLAN_SLOTS: usize = 32;

/// The provider seam: profile, isolation flag and the run control (caller
/// cancellation plus the budget deadline) to one embedding function.
pub type Acquire = Box<
    dyn Fn(
        &SemanticProfile,
        &std::sync::Arc<crate::neural::profile::VerifiedArtifacts>,
        bool,
        &Control,
    ) -> Result<Box<dyn EmbeddingProvider>, ProviderError>,
>;

/// The publication reserve: new batches stop when the remaining budget falls
/// below `max(5 s, budget/10)`.
pub fn publication_reserve(budget_seconds: u64) -> Duration {
    Duration::from_secs((budget_seconds / 10).max(5))
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PrepareReport {
    pub bootstrap: cache::BootstrapProgress,
    pub profile: String,
    pub function_digest: String,
    pub recipe_id: String,
    /// 009 T004: the profile's output dimension, recorded in the state row
    /// with the digest and recipe.
    pub dimensions: u32,
    /// The run's cache cap in bytes (the operator's setting, default
    /// [`cache::DEFAULT_CACHE_CAP_BYTES`]), recorded in the state row.
    pub cache_cap_bytes: u64,
    /// Sources examined in this run, not the total admitted corpus.
    pub sources: u64,
    pub partitioned_sources: u64,
    pub reused_partitions: u64,
    /// Cards examined in this run. A partial run may not visit every partition.
    pub eligible_units: u64,
    pub embedded_units: u64,
    /// Independently embedded multi-source brief overviews (outside source ANN).
    pub area_vectors: u64,
    pub embedded_areas: u64,
    pub reused_cached_units: u64,
    pub missing_units: u64,
    /// Last publication's corpus-wide counts, only when source revision, function
    /// and recipe are still current. None means unknown; this is not a fresh
    /// on-disk audit. Includes unpartitioned sources, unlike the run counters.
    pub coverage: Option<cache::CoverageSnapshot>,
    pub document_calls: u64,
    /// Real-tokenizer token count of the document inputs sent in this run's
    /// document calls (prefix and special tokens included).
    pub input_tokens: u64,
    pub corrupt_cache_rows: u64,
    pub cache_entries: u64,
    pub cache_bytes: u64,
    /// A validated generation covering the cached vectors exists at the end
    /// of the run.
    pub index_published: bool,
    pub index_entries: u64,
    /// Why publication did not finish (budget, cancellation, a path conflict,
    /// a build failure). `None` when it finished or had nothing to publish.
    pub index_reason: Option<String>,
    /// What this run observed of the provider: `ready`, `refused` or
    /// `failed`; `None` when the run never reached it.
    pub provider_state: Option<&'static str>,
    pub provider_code: Option<&'static str>,
    pub publication_reserve_seconds: u64,
    pub state: &'static str,
    pub reason_code: Option<&'static str>,
    pub partial: bool,
    pub budget_seconds: u64,
    pub elapsed_ms: u64,
}

impl PrepareReport {
    /// The bounded CLI error for a partial run, if any.
    pub fn error(&self) -> Option<FoundryError> {
        if !self.partial {
            return None;
        }
        self.reason_code.map(|code| FoundryError::Semantic {
            code,
            message: format!(
                "preparation stopped after committing {} of {} examined units (state {})",
                self.embedded_units, self.eligible_units, self.state
            ),
        })
    }
}

/// How a run stopped.
pub(crate) enum Stop {
    Complete,
    Partial { code: &'static str, message: String },
}

impl Stop {
    pub(crate) fn partial(code: &'static str, message: impl Into<String>) -> Self {
        Self::Partial {
            code,
            message: message.into(),
        }
    }
}

pub struct PrepareOptions<'a> {
    pub profile_path: &'a Path,
    pub budget_seconds: u64,
    pub development: bool,
    pub cache_cap_bytes: u64,
    /// Budget origin: the instant argument parsing finished.
    pub started: Instant,
    pub control: &'a Control,
    /// Bounded workspace-relative files/subtrees, interleaved with the normal walk.
    pub priority_paths: &'a [String],
}

/// Explicit priority never enrolls disk paths: it orders already admitted sources.
pub fn validate_priority_paths(paths: &[String]) -> FResult<()> {
    if paths.len() > 16 {
        return Err(FoundryError::InvalidArgument(
            "at most 16 semantic priority paths are allowed".into(),
        ));
    }
    for path in paths {
        crate::store::validate_path(path)?;
    }
    Ok(())
}

/// Named fault points of the preparation boundary (test-faults only;
/// release builds carry no hook code and no fault-name strings).
#[cfg(feature = "test-faults")]
pub mod fault_names {
    macro_rules! point {
        ($ident:ident, $name:literal) => {
            pub const $ident: &str = concat!("ctxfoundry-fault/", $name);
        };
    }
    point!(PARTITION_AFTER_COMMIT, "prepare.partition_after_commit");
    point!(PARTITION_BEFORE_COMPUTE, "prepare.partition_before_compute");
    point!(CACHE_BEFORE_COMMIT, "prepare.cache_before_commit");
    point!(CACHE_AFTER_COMMIT, "prepare.cache_after_commit");
    point!(BEFORE_PUBLISH, "prepare.before_publish");
    point!(AFTER_PUBLISH, "prepare.after_publish");
}

#[cfg(feature = "test-faults")]
macro_rules! prepare_fault {
    ($name:ident, $control:expr, $detail:expr) => {
        $crate::neural::prepare::hit_fault(
            $crate::neural::prepare::fault_names::$name,
            $control,
            $detail,
        )
    };
}

#[cfg(not(feature = "test-faults"))]
macro_rules! prepare_fault {
    ($name:ident, $control:expr, $detail:expr) => {
        Ok::<(), $crate::FoundryError>(())
    };
}

mod cli;
mod order;
mod steps;
use super::area;
pub(crate) use cli::{
    StopOrError, begin, cli_prepare_on_engine, elapsed_ms, finalize, lifetime_code,
    merge_publication, record_publication, snapshot_progress,
};
pub use cli::{cli_prepare, cli_prepare_with_priorities, run};
pub(crate) use order::{SourceCursor, SourceOrder};
pub(crate) use steps::{
    Batch, Cards, PartitionWorkers, PausedSources, PreparedCards, Progress, Selected, Selection,
    Steps, Walk, WalkPosition, commit_batch, commit_partition, count_document_call,
};

/// The fault hook entry (test-faults only).
#[cfg(feature = "test-faults")]
pub(crate) fn hit_fault(name: &str, control: Option<&Control>, detail: &str) -> FResult<()> {
    crate::fault::hit(
        name,
        &crate::fault::Ctx {
            engine: None,
            control,
            detail,
        },
    )
}
