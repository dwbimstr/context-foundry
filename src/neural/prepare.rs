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
/// Retained card payload per source, including allocated ids/key/array capacity.
/// At most one eight-source wave survives into selection. Larger sources use
/// the existing bounded on-demand render path instead of retaining their cards.
const RETAINED_CARD_BYTES: usize = 2 * 1024 * 1024;

/// Lazily started CPU workers, reused across waves (and MCP prepare runs).
/// No store, profile or source is retained here. `install` joins each wave,
/// including errors/panics, before its borrowed inputs can leave the caller.
pub(crate) struct PartitionWorkers {
    count: usize,
    pool: OnceLock<Result<rayon::ThreadPool, String>>,
}

impl Default for PartitionWorkers {
    fn default() -> Self {
        Self::new(std::thread::available_parallelism().map_or(1, usize::from))
    }
}

#[derive(Debug)]
pub(crate) struct ComputedPartition {
    pub partition: cache::PreparedPartition,
    pub retained: Option<PreparedCards>,
}

/// Only produced by rendering a verified PartitionSource. Never accepted
/// from persisted or caller-supplied token ids. Selection checks source,
/// recipe and function identity; rendering still compares every stored tuple.
#[derive(Debug)]
pub(crate) struct PreparedCards {
    path: String,
    hash: String,
    recipe: String,
    function_digest: String,
    cards: Vec<partition::Card>,
}

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
    pub sources: u64,
    pub partitioned_sources: u64,
    pub reused_partitions: u64,
    pub eligible_units: u64,
    pub embedded_units: u64,
    pub reused_cached_units: u64,
    pub missing_units: u64,
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

#[derive(Default)]
pub(crate) struct SourceCursor {
    pub exact: bool,
    pub scope: Option<String>,
    pub after: Option<String>,
}

/// At most sixteen priority cursors and one ordinary cursor. Alternate one wave
/// of each; priority scopes rotate so neither an area nor the background starves.
pub(crate) struct SourceOrder {
    priority: VecDeque<SourceCursor>,
    background: Option<SourceCursor>,
    priority_turn: bool,
}
impl SourceOrder {
    pub(crate) fn changed(paths: impl Iterator<Item = String>) -> Self {
        Self {
            priority: paths
                .map(|path| SourceCursor {
                    scope: Some(path),
                    after: None,
                    exact: true,
                })
                .collect(),
            background: None,
            priority_turn: true,
        }
    }
    pub(crate) fn prioritize(&mut self, paths: &[String]) -> FResult<()> {
        self.priority = Self::new(paths)?.priority;
        Ok(())
    }
    pub(crate) fn new(paths: &[String]) -> FResult<Self> {
        validate_priority_paths(paths)?;
        let mut unique = Vec::new();
        for path in paths {
            if !unique.contains(path) {
                unique.push(path.clone());
            }
        }
        Ok(Self {
            priority: unique
                .into_iter()
                .map(|scope| SourceCursor {
                    scope: Some(scope),
                    after: None,
                    exact: false,
                })
                .collect(),
            background: Some(SourceCursor::default()),
            priority_turn: true,
        })
    }
    pub(crate) fn next(&mut self) -> Option<SourceCursor> {
        if (self.priority_turn || self.background.is_none()) && !self.priority.is_empty() {
            self.priority_turn = false;
            self.priority.pop_front()
        } else {
            self.priority_turn = true;
            self.background.take()
        }
    }
    pub(crate) fn advance(&mut self, cursor: SourceCursor, done: bool) {
        if done {
            return;
        }
        if cursor.scope.is_some() {
            self.priority.push_back(cursor);
        } else {
            self.background = Some(cursor);
        }
    }
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

impl PartitionWorkers {
    fn new(count: usize) -> Self {
        Self {
            count: count.clamp(1, PARTITION_WAVE),
            pool: OnceLock::new(),
        }
    }

    pub(crate) fn prepare(
        &self,
        cards: Cards<'_>,
        sources: Vec<cache::PartitionSource>,
        recipe: &str,
        control: &Control,
    ) -> FResult<Vec<ComputedPartition>> {
        if sources.len() > PARTITION_WAVE {
            return Err(partition_invalid(
                "partition wave exceeds its source bound".into(),
            ));
        }
        if sources.is_empty() {
            return Ok(Vec::new());
        }
        control.check()?;
        prepare_fault!(PARTITION_BEFORE_COMPUTE, Some(control), "")?;
        let pool = self
            .pool
            .get_or_init(|| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(self.count)
                    .thread_name(|index| format!("foundry-partition-{index}"))
                    .build()
                    .map_err(|error| error.to_string())
            })
            .as_ref()
            .map_err(|message| FoundryError::Semantic {
                code: "partition_worker_failed",
                message: format!("cannot start partition workers: {message}"),
            })?;
        // Indexed parallel collection preserves source order. Collect every
        // result before propagating errors so all borrowed work has finished.
        let results: Vec<_> = pool.install(|| {
            sources
                .into_par_iter()
                .map(|source| {
                    control.check()?;
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        cards.prepare_source(source, recipe, RETAINED_CARD_BYTES)
                    })) {
                        Ok(result) => result,
                        Err(_) => Err(FoundryError::Semantic {
                            code: "partition_worker_failed",
                            message:
                                "a partition worker panicked; unpublished mappings were discarded"
                                    .into(),
                        }),
                    }
                })
                .collect()
        });
        results.into_iter().collect()
    }
}

/// The CLI entry: the provider seam resolves to the supervised worker on
/// macOS, and to an explicit `isolation_unavailable` elsewhere.
pub fn cli_prepare(
    store_dir: &Path,
    profile_path: &Path,
    budget_seconds: u64,
    development: bool,
    cache_cap_bytes: u64,
    started: Instant,
    control: &Control,
) -> FResult<PrepareReport> {
    cli_prepare_with_priorities(
        store_dir,
        profile_path,
        budget_seconds,
        development,
        cache_cap_bytes,
        started,
        control,
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
pub fn cli_prepare_with_priorities(
    store_dir: &Path,
    profile_path: &Path,
    budget_seconds: u64,
    development: bool,
    cache_cap_bytes: u64,
    started: Instant,
    control: &Control,
    priority_paths: &[String],
) -> FResult<PrepareReport> {
    run(
        store_dir,
        &PrepareOptions {
            profile_path,
            budget_seconds,
            development,
            cache_cap_bytes,
            started,
            control,
            priority_paths,
        },
        supervisor_acquire(),
    )
}

/// The provider seam. Slice B's supervisor provides `acquire_until`, which
/// bounds worker start-up and model load by the run control (budget deadline
/// and caller cancellation) and by the profile's load timeout. Until an
/// accepted isolation profile admits execution, every request fails with
/// `isolation_unavailable` and no model work is attempted.
fn supervisor_acquire() -> Acquire {
    #[cfg(target_os = "macos")]
    return Box::new(crate::neural::supervisor::acquire_verified_until);
    #[cfg(not(target_os = "macos"))]
    return Box::new(
        |_profile: &SemanticProfile,
         _artifacts: &std::sync::Arc<crate::neural::profile::VerifiedArtifacts>,
         _development: bool,
         _control: &Control|
         -> Result<Box<dyn EmbeddingProvider>, ProviderError> {
            Err(ProviderError::IsolationUnavailable(
                "no accepted isolation profile admits model execution on this platform".into(),
            ))
        },
    );
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

pub(crate) fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

pub(crate) fn snapshot_progress(
    report: &mut PrepareReport,
    started: Instant,
) -> cache::BootstrapProgress {
    report.bootstrap.elapsed_ms = elapsed_ms(started);
    report.bootstrap.selected_sources = report.sources;
    report.bootstrap.embedded_units = report.embedded_units;
    report.bootstrap.reused_cached_units = report.reused_cached_units;
    report.bootstrap.input_tokens = report.input_tokens;
    report.bootstrap.published_entries = report.index_entries;
    if report.index_published
        && report.index_entries > 0
        && report.bootstrap.first_searchable_ms.is_none()
    {
        report.bootstrap.first_searchable_ms = Some(report.bootstrap.elapsed_ms);
    }
    report.bootstrap.clone()
}

/// One bounded preparation run. Committed work is never rolled back; every
/// stop is named and recorded in the state row.
pub fn run(
    store_dir: &Path,
    options: &PrepareOptions<'_>,
    acquire: Acquire,
) -> FResult<PrepareReport> {
    options.control.check()?;
    validate_priority_paths(options.priority_paths)?;
    let deadline = options
        .started
        .checked_add(Duration::from_secs(options.budget_seconds))
        .ok_or_else(|| {
            FoundryError::InvalidArgument("budget-seconds overflows the clock".into())
        })?;
    let reserve = publication_reserve(options.budget_seconds);
    // ONE control for the whole run: the caller's cancel flag plus the budget
    // deadline. It is what verification, acquisition, every document call and
    // publication receive.
    let run_control = options.control.bounded_by(deadline);

    // The profile is parsed strictly and bounded before any worker exists;
    // a missing/mismatched profile, worker or artifact is a named failure
    // with no download, install or fallback execution, and a descriptor v1
    // profile is `profile_unsupported` before the store is touched.
    let profile = SemanticProfile::load(options.profile_path)?;
    let function_digest = profile.descriptor.digest();
    let engine = Engine::open_existing(store_dir)?;
    let recipe = partition::recipe_id(&profile.descriptor.tokenizer, profile.card_tokens);

    let mut report = PrepareReport {
        profile: profile.name.clone(),
        function_digest: function_digest.clone(),
        recipe_id: recipe.clone(),
        dimensions: profile.descriptor.dimensions,
        cache_cap_bytes: options.cache_cap_bytes,
        budget_seconds: options.budget_seconds,
        publication_reserve_seconds: reserve.as_secs(),
        ..PrepareReport::default()
    };
    let work = engine.derivation_reconcile(
        crate::neural::work::Lane::Semantic,
        &format!("{function_digest}:{recipe}"),
    )?;
    begin(&engine, &report)?;

    let outcome = inner(
        &engine,
        options,
        &profile,
        &function_digest,
        &recipe,
        &run_control,
        deadline,
        reserve,
        acquire,
        &mut report,
    );

    // Every exit path finalizes the state row: stopped when complete, paused
    // with the named reason when partial. The committed counts already moved
    // with each batch commit; startup never resumes, only an explicit prepare
    // does.
    report.elapsed_ms = options
        .started
        .elapsed()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    snapshot_progress(&mut report, options.started);
    if matches!(outcome, Ok(Stop::Complete)) {
        engine.derivation_ack(crate::neural::work::Lane::Semantic, &work)?;
    }
    finalize(&engine, &mut report, &outcome)?;
    match outcome {
        Ok(_) => Ok(report),
        Err(e) => Err(e),
    }
}

/// Record a run's start in the state row: the profile identity (digest,
/// recipe and dimension), the run's cache cap, `running`, no error, and the
/// cache-byte total reconciled from the actual rows in the same
/// transaction, so the disk cap never trusts a total another binary
/// recorded. A profile change never purges: rows and generations of the
/// earlier profile stay retained.
pub(crate) fn begin(engine: &Engine, report: &PrepareReport) -> FResult<()> {
    engine.semantic_begin_run(|state| {
        state.profile_name = Some(report.profile.clone());
        state.function_digest = Some(report.function_digest.clone());
        state.recipe_id = Some(report.recipe_id.clone());
        state.dimensions = Some(report.dimensions);
        state.cache_cap_bytes = Some(report.cache_cap_bytes);
        state.state = "running".into();
        state.last_error = None;
        state.bootstrap = None;
    })
}

/// Finalize the state row for a run's outcome: `stopped` when complete,
/// `paused` with the named reason otherwise, the provider observation the
/// run made, and the cache totals; the report gets the same facts.
pub(crate) fn finalize(
    engine: &Engine,
    report: &mut PrepareReport,
    outcome: &Result<Stop, FoundryError>,
) -> FResult<()> {
    let mut final_state = engine.semantic_state()?.unwrap_or_default();
    final_state.profile_name = Some(report.profile.clone());
    final_state.function_digest = Some(report.function_digest.clone());
    final_state.recipe_id = Some(report.recipe_id.clone());
    final_state.dimensions = Some(report.dimensions);
    let (totals_rows, totals_bytes) = engine.semantic_cache_totals()?;
    final_state.cache_bytes = totals_bytes;
    final_state.bootstrap = Some(report.bootstrap.clone());
    let (state_name, reason) = match outcome {
        Ok(Stop::Complete) => ("stopped", None),
        Ok(Stop::Partial { code, message }) => ("paused", Some((*code, message.clone()))),
        Err(e) => ("paused", Some((lifetime_code(e.code()), e.to_string()))),
    };
    final_state.state = state_name.into();
    // A publication that did not finish is recorded with the stop: the error
    // is never swallowed.
    final_state.last_error = reason.as_ref().map(|(code, message)| {
        let message = match (&report.index_reason, report.index_published) {
            (Some(why), false) if !message.contains(why.as_str()) => {
                format!("{message}; index not published: {why}")
            }
            _ => message.clone(),
        };
        StateError {
            code: code.to_string(),
            message,
        }
    });
    if let Some(observed) = report.provider_state {
        final_state.provider = Some(ProviderObservation {
            state: observed.to_owned(),
            code: report.provider_code.map(str::to_owned),
            observed_at_unix: unix_now(),
            profile: report.profile.clone(),
            function_digest: report.function_digest.clone(),
        });
    }
    report.state = state_name;
    if let Some((code, _)) = reason {
        report.partial = true;
        report.reason_code = Some(code);
    }
    report.cache_entries = totals_rows;
    report.cache_bytes = totals_bytes;
    engine.semantic_set_state(&final_state)
}

/// Semantic stop codes that can persist in the state row.
pub(crate) fn lifetime_code(code: &str) -> &'static str {
    const KNOWN: &[&str] = &[
        "budget_exhausted",
        "cache_full",
        "cancelled",
        "deadline_exceeded",
        "profile_invalid",
        "profile_unsupported",
        "isolation_unavailable",
        "provider_busy",
        "provider_timeout",
        "provider_exited",
        "provider_malformed",
        "resource_limit",
        "input_too_large",
        "index_unavailable",
        "repair_path_conflict",
        "partition_invalid",
        "partition_worker_failed",
    ];
    KNOWN
        .iter()
        .copied()
        .find(|known| *known == code)
        .unwrap_or("internal")
}

/// The named stop for the run control, if it demands one: caller
/// cancellation wins, then the budget deadline.
fn halt(control: &Control, budget_seconds: u64) -> Option<Stop> {
    match control.check() {
        Ok(()) => None,
        Err(FoundryError::DeadlineExceeded(_)) => Some(Stop::partial(
            "budget_exhausted",
            format!("the {budget_seconds}s preparation budget elapsed"),
        )),
        Err(_) => Some(Stop::partial(
            "cancelled",
            "preparation was cancelled by the caller",
        )),
    }
}

/// Whether one more document batch may be admitted: the run control must be
/// live AND at least the publication reserve must remain.
fn admit(control: &Control, budget: u64, deadline: Instant, reserve: Duration) -> Option<Stop> {
    if let Some(stop) = halt(control, budget) {
        return Some(stop);
    }
    (deadline.saturating_duration_since(Instant::now()) < reserve).then(|| {
        Stop::partial(
            "budget_exhausted",
            format!(
                "fewer than {}s of the {budget}s budget remain; the publication reserve is kept",
                reserve.as_secs()
            ),
        )
    })
}

/// Map a provider failure to its named stop. When the run control itself has
/// expired or been cancelled, THAT is the cause (`budget_exhausted` or
/// `cancelled`), whatever the provider reported.
fn provider_stop(error: ProviderError, control: &Control) -> Stop {
    let code = match (&error, control.check()) {
        (
            ProviderError::Timeout | ProviderError::Cancelled,
            Err(FoundryError::DeadlineExceeded(_)),
        ) => "budget_exhausted",
        (ProviderError::Timeout | ProviderError::Cancelled, Err(_)) => "cancelled",
        _ => lifetime_code(error.code()),
    };
    Stop::partial(
        code,
        format!("the provider stopped the batch ({error}); committed work stays"),
    )
}

/// Publish committed coverage under the run control, reusing immutable base
/// vectors and rebuilding only a bounded delta unless compaction is due. The outcome is recorded in the report — a publication that
/// cannot finish is a named stop with its reason, never swallowed.
pub(crate) fn publish(
    engine: &Engine,
    control: &Control,
    budget: u64,
    report: &mut PrepareReport,
) -> Result<Option<Stop>, FoundryError> {
    let started = Instant::now();
    prepare_fault!(BEFORE_PUBLISH, Some(control), "")?;
    let outcome = engine.semantic_publish_pending(control);
    let stop = record_publication(outcome, budget, report);
    report.bootstrap.publication_ms += elapsed_ms(started);
    if stop.is_none() {
        prepare_fault!(AFTER_PUBLISH, Some(control), "")?;
    }
    Ok(stop)
}

/// Record a publication's outcome in the report: published with its entry
/// count, nothing to publish, or a named stop (the budget, cancellation, a
/// failure) with its reason — never swallowed. The CLI's [`publish`] and the
/// MCP owner's chunked publication (`super::driver`) share it.
pub(crate) fn record_publication(
    outcome: FResult<Publication>,
    budget: u64,
    report: &mut PrepareReport,
) -> Option<Stop> {
    if matches!(&outcome, Ok(Publication::Rebuilt(_))) {
        report.bootstrap.generation_builds += 1;
    }
    match &outcome {
        Ok(Publication::Delta(_)) => report.bootstrap.delta_publications += 1,
        Ok(Publication::Mapped(_)) => report.bootstrap.mapping_publications += 1,
        _ => {}
    }
    match outcome {
        Ok(Publication::Nothing) => None,
        Ok(
            Publication::Current(entries)
            | Publication::Rebuilt(entries)
            | Publication::Delta(entries)
            | Publication::Mapped(entries),
        ) => {
            report.index_published = true;
            report.index_entries = entries as u64;
            report.index_reason = None;
            None
        }
        Err(FoundryError::DeadlineExceeded(_)) => {
            let why = format!(
                "publication was cut off by the {budget}s budget; the vectors stay cached \
                 and the index is not published"
            );
            report.index_published = false;
            report.index_reason = Some(why.clone());
            Some(Stop::partial("budget_exhausted", why))
        }
        Err(FoundryError::Cancelled(_)) => {
            let why = "publication was cancelled by the caller; the vectors stay cached".to_owned();
            report.index_published = false;
            report.index_reason = Some(why.clone());
            Some(Stop::partial("cancelled", why))
        }
        Err(error) => {
            let why = error.to_string();
            report.index_published = false;
            report.index_reason = Some(why.clone());
            Some(Stop::partial(
                lifetime_code(error.code()),
                format!("generation publication failed: {why}"),
            ))
        }
    }
}

/// The bounded work: verification, retained coverage, then partition/embed
/// waves with geometric and final publication.
#[allow(clippy::too_many_arguments)]
fn inner(
    engine: &Engine,
    options: &PrepareOptions<'_>,
    profile: &SemanticProfile,
    function_digest: &str,
    recipe: &str,
    control: &Control,
    deadline: Instant,
    reserve: Duration,
    acquire: Acquire,
    report: &mut PrepareReport,
) -> Result<Stop, FoundryError> {
    let budget = options.budget_seconds;
    if let Some(stop) = halt(control, budget) {
        return Ok(stop);
    }
    // Budget counts from argument parsing: artifact verification included,
    // and it obeys the run control (budget deadline, caller cancellation).
    let artifacts = match profile.verified_artifacts_until(control) {
        Ok(artifacts) => std::sync::Arc::new(artifacts),
        Err(e) => {
            return match halt(control, budget) {
                Some(stop) => Ok(stop),
                None => Err(FoundryError::from(e)),
            };
        }
    };
    if let Some(stop) = halt(control, budget) {
        return Ok(stop);
    }
    let tokenizer = DocumentTokenizer::load_verified(profile, &artifacts)?;
    // Cached-only preparation needs no model worker. Artifact verification
    // still precedes tokenization; acquisition retains its own trust checks.
    let mut provider: Option<Box<dyn EmbeddingProvider>> = None;
    let workers = PartitionWorkers::default();

    let steps = Steps {
        engine,
        cards: Cards {
            tokenizer: &tokenizer,
            profile,
            function_digest,
        },
        recipe,
    };
    // Recover accepted coverage before admitting new inference. A cold store's
    // publication visits zero prepared rows, not the whole source corpus.
    if let Some(stop) = publish(engine, control, budget, report)? {
        return Ok(stop);
    }
    let mut published = report.index_entries;
    snapshot_progress(report, options.started);
    let mut unpublished = 0u64;
    let mut order = SourceOrder::new(options.priority_paths)?;
    let mut stop: Option<Stop> = None;
    'waves: while let Some(mut cursor) = order.next() {
        let before = cursor.after.clone();
        let (progress, inputs) = steps.partition_inputs_in(
            &mut cursor.after,
            cursor.scope.as_deref(),
            cursor.exact,
            PARTITION_WAVE,
            None,
            report,
            &mut || halt(control, budget),
        )?;
        if let Progress::Halted(halted) = progress {
            stop = Some(halted);
            break;
        }
        let computed = Instant::now();
        let prepared = workers.prepare(steps.cards, inputs, recipe, control);
        report.bootstrap.partition_compute_ms += elapsed_ms(computed);
        let prepared = prepared?;
        let mut retained = Vec::new();
        for computed in prepared {
            if let Some(halted) = halt(control, budget) {
                stop = Some(halted);
                break 'waves;
            }
            commit_partition(engine, computed.partition, control, report)?;
            retained.extend(computed.retained);
        }
        let done = matches!(progress, Progress::Done);
        if cursor.after != before {
            report.bootstrap.waves += 1;
            report.bootstrap.priority_waves += u64::from(cursor.scope.is_some());
            let mut walk = Walk::wave(cursor.scope.clone(), before, cursor.after.clone())
                .with_retained(retained);
            loop {
                let mut selection = Selection::default();
                let end = match steps.select_batch(
                    &mut walk,
                    &mut selection,
                    steps.cards.limits().inputs,
                    usize::MAX,
                    report,
                    &mut || halt(control, budget),
                )? {
                    Selected::Full => false,
                    Selected::End => true,
                    Selected::Yield => continue,
                    Selected::Halted(halted) => {
                        stop = Some(halted);
                        break 'waves;
                    }
                };
                let rendered = Instant::now();
                let batch = steps.cards.render(selection);
                report.bootstrap.render_ms += elapsed_ms(rendered);
                let batch = batch?;
                if batch.is_empty() {
                    break;
                }
                if let Some(halted) = admit(control, budget, deadline, reserve) {
                    stop = Some(halted);
                    break 'waves;
                }
                if provider.is_none() {
                    artifacts.check(profile, control)?;
                    let acquired = match acquire(profile, &artifacts, options.development, control)
                    {
                        Ok(provider) => provider,
                        Err(error) => {
                            if let Some(halted) = halt(control, budget) {
                                stop = Some(halted);
                                break 'waves;
                            }
                            report.provider_state = Some("refused");
                            report.provider_code = Some(lifetime_code(error.code()));
                            return Err(FoundryError::from(error));
                        }
                    };
                    if acquired.descriptor().digest() != function_digest {
                        report.provider_state = Some("refused");
                        report.provider_code = Some("provider_malformed");
                        return Err(FoundryError::Semantic {
                            code: "provider_malformed",
                            message: "the worker's document function does not match the profile"
                                .into(),
                        });
                    }
                    report.provider_state = Some("ready");
                    provider = Some(acquired);
                    // Startup shares the same deadline and publication reserve.
                    if let Some(halted) = admit(control, budget, deadline, reserve) {
                        stop = Some(halted);
                        break 'waves;
                    }
                }
                let size = batch.len() as u64;
                match flush_batch(
                    &steps,
                    provider
                        .as_mut()
                        .expect("acquired for missing inputs")
                        .as_mut(),
                    batch,
                    options.cache_cap_bytes,
                    control,
                    report,
                ) {
                    Ok(()) => {
                        walk.seen.clear(); // Accepted keys are now in the durable cache.
                        report.embedded_units += size;
                        unpublished += size;
                    }
                    Err(StopOrError::Stop(partial)) => {
                        stop = Some(partial);
                        break 'waves;
                    }
                    Err(StopOrError::Error(error)) => return Err(error),
                }
                if let Some(halted) = halt(control, budget) {
                    stop = Some(halted);
                    break 'waves;
                }
                // Geometric publication: first batch, then approximately doubled
                // coverage. The total cold-build vector work is amortized.
                if unpublished >= published.max(1) {
                    if let Some(halted) = publish(engine, control, budget, report)? {
                        stop = Some(halted);
                        break 'waves;
                    }
                    published = report.index_entries;
                    unpublished = 0;
                    snapshot_progress(report, options.started);
                }
                if end {
                    break;
                }
            }
            // Repartitioned inputs may all hit retained cache rows. Make the
            // first such wave searchable even without a document call.
            if published == 0 && report.reused_cached_units > 0 {
                if let Some(halted) = publish(engine, control, budget, report)? {
                    stop = Some(halted);
                    break;
                }
                published = report.index_entries;
                snapshot_progress(report, options.started);
            }
        }
        order.advance(cursor, done);
    }
    report.missing_units = report
        .eligible_units
        .saturating_sub(report.reused_cached_units + report.embedded_units);

    // --- Publication: new vectors were committed, so replay the generation
    // from the cache under the run control — after a complete pass AND after
    // a bounded stop, so partial coverage is searchable. A cancelled run
    // publishes nothing more. If publication cannot finish, that is recorded
    // in the report and the state; the original stop reason is kept.
    let cancelled = matches!(&stop, Some(Stop::Partial { code, .. }) if *code == "cancelled");
    if !cancelled {
        match publish(engine, control, budget, report)? {
            None => {}
            Some(published_stop) => stop = Some(merge_publication(stop, published_stop)),
        }
    }
    Ok(stop.unwrap_or(Stop::Complete))
}

/// A stop that comes with a publication that could not finish: the run's own
/// reason is kept and the publication's is appended; a run that had no stop
/// takes the publication's.
pub(crate) fn merge_publication(stop: Option<Stop>, published: Stop) -> Stop {
    match (stop, published) {
        (
            Some(Stop::Partial { code, message }),
            Stop::Partial {
                message: publication,
                ..
            },
        ) => Stop::Partial {
            code,
            message: format!("{message}; {publication}"),
        },
        (_, published) => published,
    }
}

/// Marker to route stop-vs-error out of a batch flush.
pub(crate) enum StopOrError {
    Stop(Stop),
    Error(FoundryError),
}

/// The CLI's batch: one direct document call, then the shared validation and
/// commit. Limits are checked before any model work. The run control crosses
/// the seam unchanged; the supervisor owns any in-flight grace.
fn flush_batch(
    steps: &Steps<'_>,
    provider: &mut dyn EmbeddingProvider,
    batch: Batch,
    cap_bytes: u64,
    control: &Control,
    report: &mut PrepareReport,
) -> Result<(), StopOrError> {
    if let Err(e) = provider::check_document_batch(&batch.inputs, steps.cards.limits()) {
        return Err(StopOrError::Error(FoundryError::from(e)));
    }
    count_document_call(report, &batch.inputs);
    let started = Instant::now();
    let outcome = provider.embed_documents(&batch.inputs, control);
    report.bootstrap.provider_ms += elapsed_ms(started);
    let vectors = match outcome {
        Ok(vectors) => vectors,
        Err(e) => {
            let stop = provider_stop(e, control);
            if let Stop::Partial { code, .. } = &stop
                && !matches!(*code, "budget_exhausted" | "cancelled")
            {
                report.provider_state = Some("failed");
                report.provider_code = Some(*code);
            }
            return Err(StopOrError::Stop(stop));
        }
    };
    commit_batch(
        steps.engine,
        batch.keys,
        vectors,
        steps.cards.function_digest,
        steps.cards.dims(),
        cap_bytes,
        control,
        report,
    )
}

// ---------------------------------------------------------------------------
// Shared steps (009 T003): the CLI above and the MCP driver compose these.
// ---------------------------------------------------------------------------

/// Count one admitted document call and its exact input tokens.
pub(crate) fn count_document_call(report: &mut PrepareReport, inputs: &[TokenizedInput]) {
    report.document_calls += 1;
    report.input_tokens += inputs
        .iter()
        .map(|input| input.ids.len() as u64)
        .sum::<u64>();
}

/// Validate and commit one batch's vectors: one vector per input, each of
/// the profile's exact dimension with finite values, then ONE cache
/// transaction (the committed counts move with it), before any publication.
/// A malformed reply commits nothing and stops `provider_malformed`; the
/// disk cap stops `cache_full` without evicting. Vectors are keyed by exact
/// rendered input: a late result for a source edited or deleted since its
/// selection may populate the cache, but eligibility is always recomputed
/// from the CURRENT sources, so it never restores stale eligibility.
/// (`control` reaches only the test-faults commit points.)
#[cfg_attr(not(feature = "test-faults"), allow(unused_variables))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn commit_batch(
    engine: &Engine,
    keys: Vec<String>,
    vectors: Vec<Vec<f32>>,
    function_digest: &str,
    dims: usize,
    cap_bytes: u64,
    control: &Control,
    report: &mut PrepareReport,
) -> Result<(), StopOrError> {
    if vectors.len() != keys.len() {
        report.provider_state = Some("failed");
        report.provider_code = Some("provider_malformed");
        return Err(StopOrError::Stop(Stop::partial(
            "provider_malformed",
            format!(
                "the provider returned {} vectors for {} inputs",
                vectors.len(),
                keys.len()
            ),
        )));
    }
    for vector in &vectors {
        if let Err(e) = provider::validate_vector(vector, dims) {
            // The batch is NOT committed: valid data stays intact.
            report.provider_state = Some("failed");
            report.provider_code = Some("provider_malformed");
            return Err(StopOrError::Stop(Stop::partial(
                "provider_malformed",
                e.to_string(),
            )));
        }
    }
    let entries: Vec<(String, Vec<f32>)> = keys.into_iter().zip(vectors).collect();
    prepare_fault!(
        CACHE_BEFORE_COMMIT,
        Some(control),
        &entries.len().to_string()
    )
    .map_err(StopOrError::Error)?;
    if let Err(e) = engine.semantic_cache_commit(&entries, function_digest, dims, cap_bytes) {
        return Err(match e.code() {
            "cache_full" => StopOrError::Stop(Stop::partial(
                "cache_full",
                "the workspace cache reached its disk cap; nothing was evicted",
            )),
            _ => StopOrError::Error(e),
        });
    }
    // Commit-adjacent fault point: an exit here leaves the batch committed.
    prepare_fault!(
        CACHE_AFTER_COMMIT,
        Some(control),
        &entries.len().to_string()
    )
    .map_err(StopOrError::Error)?;
    Ok(())
}

/// The fixed inputs of one run's store steps. The CLI builds it once; the
/// MCP driver builds it inside each engine-slot hold.
pub(crate) struct Steps<'a> {
    pub engine: &'a Engine,
    pub cards: Cards<'a>,
    pub recipe: &'a str,
}

/// What card rendering needs, and nothing of the store: the profile, its
/// tokenizer and its function digest. [`Cards::render`] runs with no engine
/// slot and no transaction.
#[derive(Clone, Copy)]
pub(crate) struct Cards<'a> {
    pub tokenizer: &'a (dyn TokenCount + Sync),
    pub profile: &'a SemanticProfile,
    pub function_digest: &'a str,
}

/// What one partition step did.
pub(crate) enum Progress {
    /// Sources remain after `after`.
    More,
    /// Every current source has been examined.
    Done,
    Halted(Stop),
}

/// What one selection step did to the caller's [`Selection`].
pub(crate) enum Selected {
    /// The selection holds the cards asked for; the walk continues after it.
    Full,
    /// The step examined its source bound; the walk (and the selection)
    /// continue.
    Yield,
    /// The walk is over; the selection (possibly empty) is the last one.
    End,
    Halted(Stop),
}

/// The embed walk's position: every source up to `after` is done, `partial`
/// is the source version in progress and its next card. `seen` deduplicates
/// pending inputs; callers replace it at batch boundaries, and accepted
/// inputs deduplicate through the durable cache.
#[derive(Default)]
pub(crate) struct Walk {
    pub(crate) exact: bool,
    after: Option<String>,
    scope: Option<String>,
    through: Option<String>,
    partial: Option<(Rc<SourceVersion>, usize)>,
    seen: HashSet<String>,
    retained: Vec<PreparedCards>,
}
impl Walk {
    pub(crate) fn with_retained(mut self, retained: Vec<PreparedCards>) -> Self {
        self.retained = retained;
        self
    }

    pub(crate) fn exclude(&mut self, keys: impl IntoIterator<Item = String>) {
        self.seen.clear();
        self.seen.extend(keys);
    }

    pub(crate) fn wave(
        scope: Option<String>,
        after: Option<String>,
        through: Option<String>,
    ) -> Self {
        Self {
            scope,
            after,
            through,
            ..Self::default()
        }
    }
}

/// One verified source version: either fresh prepared cards or its body
/// and lazily parsed units. A partially consumed source carries this input
/// across batches; unchanged fresh inputs need no second body read or parse.
struct SourceVersion {
    path: String,
    hash: String,
    content: SourceCards,
}

enum SourceCards {
    Body {
        body: String,
        units: OnceCell<Vec<Unit>>,
    },
    Prepared(PreparedCards),
}

/// Cards a selection step found missing, not yet rendered: per source
/// version, how many cards its partition records and the missing ones
/// (index and recorded tuple), in walk order. Rendering
/// ([`Cards::render`]) needs no engine and no transaction.
#[derive(Default)]
pub(crate) struct Selection {
    sources: Vec<SelectedSource>,
    len: usize,
}

struct SelectedSource {
    source: Rc<SourceVersion>,
    cards: usize,
    wanted: Vec<(usize, PartitionUnit)>,
}

impl Selection {
    pub(crate) fn len(&self) -> usize {
        self.len
    }
}

/// One document batch being formed: cache keys and the exact model inputs.
#[derive(Default)]
pub(crate) struct Batch {
    pub keys: Vec<String>,
    pub inputs: Vec<TokenizedInput>,
}

impl Batch {
    pub(crate) fn len(&self) -> usize {
        self.keys.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

fn partition_invalid(message: String) -> FoundryError {
    FoundryError::Semantic {
        code: "partition_invalid",
        message,
    }
}

impl Cards<'_> {
    #[cfg(test)]
    fn prepare_partitions(
        &self,
        sources: Vec<cache::PartitionSource>,
        recipe: &str,
        control: &Control,
    ) -> FResult<Vec<cache::PreparedPartition>> {
        self.prepare_partitions_with_workers(sources, recipe, control, PARTITION_WAVE)
    }

    #[cfg(test)]
    fn prepare_partitions_with_workers(
        &self,
        sources: Vec<cache::PartitionSource>,
        recipe: &str,
        control: &Control,
        workers: usize,
    ) -> FResult<Vec<cache::PreparedPartition>> {
        Ok(PartitionWorkers::new(workers)
            .prepare(*self, sources, recipe, control)?
            .into_iter()
            .map(|computed| computed.partition)
            .collect())
    }

    fn prepare_source(
        &self,
        source: cache::PartitionSource,
        recipe: &str,
        retain_limit: usize,
    ) -> FResult<ComputedPartition> {
        let path = source.path().to_owned();
        let hash = source.source_hash().to_owned();
        let mut retained = None;
        let partition = source.prepare(recipe, self.function_digest, None, |body| {
            let units = partition::carded_units(body, Lang::from_path(&path));
            let identity_bytes = std::mem::size_of::<PreparedCards>()
                + path.capacity()
                + hash.capacity()
                + recipe.len()
                + self.function_digest.len();
            let mut kept_bytes = identity_bytes.saturating_add(
                units
                    .len()
                    .saturating_mul(std::mem::size_of::<partition::Card>()),
            );
            let mut kept = (kept_bytes <= retain_limit).then(|| Vec::with_capacity(units.len()));
            let mut tuples = Vec::with_capacity(units.len());
            for unit in &units {
                let card = partition::card(body, &path, unit, &self.recipe(), self.tokenizer)?;
                tuples.push(PartitionUnit {
                    start: card.start,
                    end: card.end,
                    input_key: card.input_key.clone(),
                });
                if kept.is_some() {
                    kept_bytes = kept_bytes
                        .saturating_add(card.input_key.capacity())
                        .saturating_add(
                            card.ids
                                .capacity()
                                .saturating_mul(std::mem::size_of::<u32>()),
                        );
                    if kept_bytes <= retain_limit {
                        kept.as_mut().expect("retaining cards").push(card);
                    } else {
                        kept = None;
                    }
                }
            }
            retained = kept;
            Ok(tuples)
        })?;
        Ok(ComputedPartition {
            partition,
            retained: retained.map(|cards| PreparedCards {
                path,
                hash,
                recipe: recipe.to_owned(),
                function_digest: self.function_digest.to_owned(),
                cards,
            }),
        })
    }

    /// The limits of one document call under the run's profile.
    pub(crate) fn limits(&self) -> DocumentLimits {
        self.profile.document_limits()
    }

    /// The profile's output dimension.
    pub(crate) fn dims(&self) -> usize {
        self.profile.descriptor.dims()
    }

    fn recipe(&self) -> CardRecipe<'_> {
        CardRecipe {
            template: &self.profile.descriptor.document_template,
            function_digest: self.function_digest,
            card_tokens: self.profile.card_tokens as usize,
        }
    }

    /// Form the selected batch without an engine or transaction. Reuse
    /// privately prepared cards when available; otherwise render only the
    /// selected units. Both paths require each recorded range/key to match.
    pub(crate) fn render(&self, selection: Selection) -> FResult<Batch> {
        let recipe = self.recipe();
        let mut batch = Batch::default();
        for selected in selection.sources {
            let source = &selected.source;
            let count = match &source.content {
                SourceCards::Body { body, units } => units
                    .get_or_init(|| partition::carded_units(body, Lang::from_path(&source.path)))
                    .len(),
                SourceCards::Prepared(prepared) => {
                    if prepared.function_digest != self.function_digest
                        || prepared.recipe
                            != partition::recipe_id(
                                &self.profile.descriptor.tokenizer,
                                self.profile.card_tokens,
                            )
                    {
                        return Err(partition_invalid("retained card identity changed".into()));
                    }
                    prepared.cards.len()
                }
            };
            if count != selected.cards {
                return Err(partition_invalid(format!(
                    "{}: the partition records {} cards, but the body has {} carded units",
                    source.path, selected.cards, count
                )));
            }
            for (index, recorded) in selected.wanted {
                let card = match &source.content {
                    SourceCards::Prepared(prepared) => prepared.cards[index].clone(),
                    SourceCards::Body { body, units } => partition::card(
                        body,
                        &source.path,
                        &units.get().expect("units initialized")[index],
                        &recipe,
                        self.tokenizer,
                    )?,
                };
                if (card.start, card.end, card.input_key.as_str())
                    != (recorded.start, recorded.end, recorded.input_key.as_str())
                {
                    return Err(partition_invalid(format!(
                        "{}: card {index} is recorded as {}..{}, but renders as {}..{} under \
                         {} key",
                        source.path,
                        recorded.start,
                        recorded.end,
                        card.start,
                        card.end,
                        if card.input_key == recorded.input_key {
                            "the same"
                        } else {
                            "another"
                        }
                    )));
                }
                batch.keys.push(card.input_key);
                batch.inputs.push(TokenizedInput { ids: card.ids });
            }
        }
        Ok(batch)
    }
}

impl Steps<'_> {
    /// CLI composition of snapshot, parallel computation and ordered commits.
    /// Select at most min(max_new, eight) new sources, respecting `until` at
    /// source boundaries. Recheck cancellation and the source version before
    /// each atomic commit. MCP composes these stages around separate slot holds.
    #[cfg(test)]
    pub(crate) fn partition_page(
        &self,
        after: &mut Option<String>,
        max_new: usize,
        until: Option<Instant>,
        control: &Control,
        report: &mut PrepareReport,
        halt: &mut dyn FnMut() -> Option<Stop>,
    ) -> FResult<Progress> {
        let (progress, sources) = self.partition_inputs(after, max_new, until, report, halt)?;
        if matches!(progress, Progress::Halted(_)) {
            return Ok(progress);
        }
        let prepared = self
            .cards
            .prepare_partitions(sources, self.recipe, control)?;
        for partition in prepared {
            if let Some(stop) = halt() {
                return Ok(Progress::Halted(stop));
            }
            commit_partition(self.engine, partition, control, report)?;
        }
        Ok(progress)
    }

    /// Snapshot only. Card parsing/tokenization happens after the caller
    /// releases its engine slot. Source order also determines commit order.
    #[cfg(test)]
    pub(crate) fn partition_inputs(
        &self,
        after: &mut Option<String>,
        max_new: usize,
        until: Option<Instant>,
        report: &mut PrepareReport,
        halt: &mut dyn FnMut() -> Option<Stop>,
    ) -> FResult<(Progress, Vec<cache::PartitionSource>)> {
        self.partition_inputs_in(after, None, false, max_new, until, report, halt)
    }

    fn source_page(
        &self,
        after: Option<&str>,
        scope: Option<&str>,
        exact: bool,
    ) -> FResult<Vec<(String, crate::store::SourceMeta)>> {
        if !exact {
            return cache::source_page_in(&self.engine.db, after, scope);
        }
        if after.is_some() {
            return Ok(Vec::new());
        }
        let path = scope.expect("exact source cursor has a path");
        Ok(self
            .engine
            .source(path)?
            .map(|meta| vec![(path.to_owned(), meta)])
            .unwrap_or_default())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn partition_inputs_in(
        &self,
        after: &mut Option<String>,
        scope: Option<&str>,
        exact: bool,
        max_new: usize,
        until: Option<Instant>,
        report: &mut PrepareReport,
        halt: &mut dyn FnMut() -> Option<Stop>,
    ) -> FResult<(Progress, Vec<cache::PartitionSource>)> {
        let mut inputs = Vec::new();
        if let Some(stop) = halt() {
            return Ok((Progress::Halted(stop), inputs));
        }
        let page = self.source_page(after.as_deref(), scope, exact)?;
        if page.is_empty() {
            return Ok((Progress::Done, inputs));
        }
        for (examined, (path, meta)) in page.iter().enumerate() {
            if inputs.len() == max_new.min(PARTITION_WAVE)
                || (examined > 0 && until.is_some_and(|until| Instant::now() >= until))
            {
                return Ok((Progress::More, inputs));
            }
            report.sources += 1;
            if let Some(existing) = self.engine.semantic_partition(path)?
                && cache::partition_is_current(
                    &existing,
                    meta,
                    self.recipe,
                    self.cards.function_digest,
                )
            {
                report.reused_partitions += 1;
                *after = Some(path.clone());
                continue;
            }
            if let Some(stop) = halt() {
                return Ok((Progress::Halted(stop), inputs));
            }
            inputs.push(self.engine.semantic_partition_source(path, &meta.hash)?);
            *after = Some(path.clone());
        }
        Ok((Progress::More, inputs))
    }

    /// Add the next missing cards to `selection` (a store step: no
    /// rendering, no tokenization): the walk visits the cards of every
    /// source with a CURRENT partition in path order, skips inputs this run
    /// resolved or the cache holds valid, and records each missing one (its
    /// index and recorded tuple) with its source version's verified body,
    /// until the selection holds `room` cards, the walk ends, or
    /// `max_sources` sources were examined. Pages are read fresh at every
    /// step; the version the walk paused in keeps its body (and parsed
    /// units) for the next step, and a source whose version changed since
    /// restarts at its first card. `halt` is asked before every page.
    pub(crate) fn select_batch(
        &self,
        walk: &mut Walk,
        selection: &mut Selection,
        room: usize,
        max_sources: usize,
        report: &mut PrepareReport,
        halt: &mut dyn FnMut() -> Option<Stop>,
    ) -> FResult<Selected> {
        let dims = self.cards.dims();
        let mut examined = 0usize;
        loop {
            if let Some(stop) = halt() {
                return Ok(Selected::Halted(stop));
            }
            if selection.len >= room {
                return Ok(Selected::Full);
            }
            let page =
                self.source_page(walk.after.as_deref(), walk.scope.as_deref(), walk.exact)?;
            if page.is_empty() {
                return Ok(Selected::End);
            }
            for (path, meta) in &page {
                if walk.through.as_ref().is_some_and(|end| path > end) {
                    return Ok(Selected::End);
                }
                if examined == max_sources {
                    return Ok(Selected::Yield);
                }
                examined += 1;
                let resumed = walk
                    .partial
                    .take()
                    .filter(|(source, _)| source.path == *path && source.hash == meta.hash);
                let first = resumed.as_ref().map_or(0, |(_, next)| *next);
                if let Some(record) = self.engine.semantic_partition(path)?
                    && cache::partition_is_current(
                        &record,
                        meta,
                        self.recipe,
                        self.cards.function_digest,
                    )
                    && first < record.units.len()
                {
                    let mut wanted = Vec::new();
                    let mut full_at = None;
                    for (index, unit) in record.units.iter().enumerate().skip(first) {
                        report.eligible_units += 1;
                        if walk.seen.contains(&unit.input_key) {
                            // Identical rendered inputs share this run's result.
                            report.reused_cached_units += 1;
                            continue;
                        }
                        // The lookup that USES a vector decodes it: a
                        // same-length nonfinite (or otherwise tampered) row is
                        // disabled by name here and replaced by a fresh
                        // embedding.
                        let needed = match self.engine.semantic_cache_lookup(
                            &unit.input_key,
                            self.cards.function_digest,
                            dims,
                        )? {
                            CacheLookup::Hit(_) => false,
                            CacheLookup::Corrupt(_) => {
                                report.corrupt_cache_rows += 1;
                                true
                            }
                            CacheLookup::Miss => true,
                        };
                        if !needed {
                            report.reused_cached_units += 1;
                            continue;
                        }
                        walk.seen.insert(unit.input_key.clone());
                        wanted.push((index, unit.clone()));
                        if selection.len + wanted.len() == room {
                            full_at = Some(index + 1);
                            break;
                        }
                    }
                    if !wanted.is_empty() {
                        let source = match resumed {
                            Some((source, _)) => source,
                            None => {
                                let retained = walk.retained.iter().position(|cards| {
                                    cards.path == *path
                                        && cards.hash == meta.hash
                                        && cards.recipe == self.recipe
                                        && cards.function_digest == self.cards.function_digest
                                });
                                let content = match retained {
                                    Some(index) => {
                                        SourceCards::Prepared(walk.retained.swap_remove(index))
                                    }
                                    None => SourceCards::Body {
                                        body: self.engine.semantic_source_body(path, meta)?,
                                        units: OnceCell::new(),
                                    },
                                };
                                Rc::new(SourceVersion {
                                    path: path.clone(),
                                    hash: meta.hash.clone(),
                                    content,
                                })
                            }
                        };
                        selection.len += wanted.len();
                        selection.sources.push(SelectedSource {
                            source: Rc::clone(&source),
                            cards: record.units.len(),
                            wanted,
                        });
                        if let Some(next) = full_at {
                            walk.partial = Some((source, next));
                            return Ok(Selected::Full);
                        }
                    }
                }
                walk.after = Some(path.clone());
            }
        }
    }
}

/// Publish a computed mapping only against its still-current source version.
#[cfg_attr(not(feature = "test-faults"), allow(unused_variables))]
pub(crate) fn commit_partition(
    engine: &Engine,
    partition: cache::PreparedPartition,
    control: &Control,
    report: &mut PrepareReport,
) -> FResult<()> {
    control.check()?;
    let path = partition.path().to_owned();
    engine.semantic_accept_partition(partition)?;
    report.partitioned_sources += 1;
    prepare_fault!(PARTITION_AFTER_COMMIT, Some(control), &path)?;
    Ok(())
}

#[cfg(all(test, feature = "test-faults"))]
mod tests {
    use super::*;

    fn partition_fixture() -> (tempfile::TempDir, Engine, SemanticProfile) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        for n in 0..12 {
            std::fs::write(
                root.join(format!("note{n:02}.md")),
                format!("# Note {n} é\n\nRetained detail {n}.\n"),
            )
            .unwrap();
        }
        let mut engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
        engine.index(&root, &Control::unbounded()).unwrap();
        let profile = SemanticProfile::load(&crate::testkit::write_semantic_profile(
            dir.path(),
            "partitions",
            |_| {},
        ))
        .unwrap();
        (dir, engine, profile)
    }

    fn inputs(engine: &Engine, count: usize) -> Vec<cache::PartitionSource> {
        (0..count)
            .map(|n| {
                let path = format!("note{n:02}.md");
                let hash = engine.source(&path).unwrap().unwrap().hash;
                engine.semantic_partition_source(&path, &hash).unwrap()
            })
            .collect()
    }

    #[test]
    fn parallel_partitions_equal_serial_and_snapshot_admission_is_bounded() {
        let (_dir, engine, profile) = partition_fixture();
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let digest = profile.descriptor.digest();
        let recipe = partition::recipe_id(&profile.descriptor.tokenizer, profile.card_tokens);
        let cards = Cards {
            tokenizer: &tokenizer,
            profile: &profile,
            function_digest: &digest,
        };
        let control = Control::unbounded();
        let mut after = None;
        let mut report = PrepareReport::default();
        let (_, wave) = Steps {
            engine: &engine,
            cards,
            recipe: &recipe,
        }
        .partition_inputs(&mut after, usize::MAX, None, &mut report, &mut || None)
        .unwrap();
        assert_eq!(wave.len(), PARTITION_WAVE);
        assert_eq!(after.as_deref(), Some("note07.md"));
        assert_eq!(report.partitioned_sources, 0);
        assert!(engine.semantic_partition("note00.md").unwrap().is_none());
        let mut snapshots = Vec::new();
        for workers in [1, 8] {
            let prepared = cards
                .prepare_partitions_with_workers(inputs(&engine, 8), &recipe, &control, workers)
                .unwrap();
            for (n, partition) in prepared.into_iter().enumerate() {
                assert_eq!(partition.path(), format!("note{n:02}.md"));
                engine.semantic_accept_partition(partition).unwrap();
            }
            snapshots.push(
                (0..8)
                    .map(|n| {
                        serde_json::to_value(
                            engine
                                .semantic_partition(&format!("note{n:02}.md"))
                                .unwrap(),
                        )
                        .unwrap()
                    })
                    .collect::<Vec<_>>(),
            );
        }
        assert_eq!(snapshots[0], snapshots[1]);
        assert_eq!(
            cards
                .prepare_partitions(inputs(&engine, 9), &recipe, &control)
                .unwrap_err()
                .code(),
            "partition_invalid"
        );
    }

    #[test]
    fn partition_cpu_workers_overlap_without_a_store_transaction() {
        use std::sync::{Condvar, Mutex};
        struct Concurrent<'a> {
            inner: &'a DocumentTokenizer,
            arrivals: Mutex<usize>,
            ready: Condvar,
        }
        impl TokenCount for Concurrent<'_> {
            fn encode(&self, text: &str) -> Result<partition::TokenizedText, ProviderError> {
                let mut arrived = self.arrivals.lock().unwrap();
                *arrived += 1;
                self.ready.notify_all();
                let (arrived, timed) = self
                    .ready
                    .wait_timeout_while(arrived, Duration::from_secs(5), |arrived| *arrived < 2)
                    .unwrap();
                drop(arrived);
                assert!(
                    !timed.timed_out(),
                    "a second CPU worker must enter while the first is blocked"
                );
                self.inner.encode(text)
            }
        }
        let (_dir, engine, profile) = partition_fixture();
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let concurrent = Concurrent {
            inner: &tokenizer,
            arrivals: Mutex::new(0),
            ready: Condvar::new(),
        };
        let digest = profile.descriptor.digest();
        let cards = Cards {
            tokenizer: &concurrent,
            profile: &profile,
            function_digest: &digest,
        };
        let prepared = cards
            .prepare_partitions_with_workers(inputs(&engine, 2), "recipe", &Control::unbounded(), 2)
            .unwrap();
        assert_eq!(prepared.len(), 2);
        assert!(engine.semantic_partition("note00.md").unwrap().is_none());
    }

    #[test]
    fn partition_cancellation_and_worker_panic_publish_nothing() {
        struct Panics;
        impl TokenCount for Panics {
            fn encode(&self, _: &str) -> Result<partition::TokenizedText, ProviderError> {
                panic!("injected tokenizer panic")
            }
        }
        let (_dir, engine, profile) = partition_fixture();
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let digest = profile.descriptor.digest();
        let cards = Cards {
            tokenizer: &tokenizer,
            profile: &profile,
            function_digest: &digest,
        };
        assert_eq!(
            cards
                .prepare_partitions(inputs(&engine, 8), "recipe", &Control::cancelled())
                .unwrap_err()
                .code(),
            "cancelled"
        );
        let mut prepared = cards
            .prepare_partitions(inputs(&engine, 1), "recipe", &Control::unbounded())
            .unwrap();
        assert_eq!(
            commit_partition(
                &engine,
                prepared.pop().unwrap(),
                &Control::cancelled(),
                &mut PrepareReport::default()
            )
            .unwrap_err()
            .code(),
            "cancelled"
        );
        let panics = Cards {
            tokenizer: &Panics,
            ..cards
        };
        let error = panics
            .prepare_partitions_with_workers(inputs(&engine, 8), "recipe", &Control::unbounded(), 8)
            .unwrap_err();
        assert_eq!(error.code(), "partition_worker_failed");
        let mut report = PrepareReport {
            profile: profile.name.clone(),
            function_digest: digest,
            recipe_id: "recipe".into(),
            dimensions: profile.descriptor.dimensions,
            ..PrepareReport::default()
        };
        finalize(&engine, &mut report, &Err(error)).unwrap();
        assert_eq!(
            engine
                .semantic_state()
                .unwrap()
                .unwrap()
                .last_error
                .unwrap()
                .code,
            "partition_worker_failed"
        );
        assert!(engine.semantic_partition("note00.md").unwrap().is_none());
    }

    /// 009 T003, captain decision 2026-10-06: a partition step stops at its
    /// time bound once it examined a source, and at its count bound without
    /// one. The injected bound has already passed, so each step writes
    /// exactly one partition.
    #[test]
    fn a_partition_step_stops_at_its_time_bound() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        for n in 0..12 {
            std::fs::write(
                root.join(format!("note{n:02}.md")),
                format!("# Note {n}\n\nbody of note {n}\n"),
            )
            .unwrap();
        }
        let mut engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
        engine.index(&root, &Control::unbounded()).unwrap();
        let path = crate::testkit::write_semantic_profile(dir.path(), "steps", |_| {});
        let profile = SemanticProfile::load(&path).unwrap();
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let digest = profile.descriptor.digest();
        let recipe = partition::recipe_id(&profile.descriptor.tokenizer, profile.card_tokens);
        let steps = Steps {
            engine: &engine,
            cards: Cards {
                tokenizer: &tokenizer,
                profile: &profile,
                function_digest: &digest,
            },
            recipe: &recipe,
        };
        let control = Control::unbounded();
        let mut report = PrepareReport::default();
        let mut after = None;
        let mut step = |until: Option<Instant>, report: &mut PrepareReport| {
            steps
                .partition_page(&mut after, 8, until, &control, report, &mut || None)
                .unwrap()
        };
        for written in 1..=3 {
            assert!(matches!(
                step(Some(Instant::now()), &mut report),
                Progress::More
            ));
            assert_eq!(report.partitioned_sources, written, "one source per step");
        }
        assert!(matches!(step(None, &mut report), Progress::More));
        assert_eq!(report.partitioned_sources, 11, "the count bound: 8 more");
        assert!(matches!(step(None, &mut report), Progress::More));
        assert_eq!(report.partitioned_sources, 12);
        assert!(matches!(step(None, &mut report), Progress::Done));
        assert_eq!(report.sources, 12, "every source examined once");
    }

    /// Counts every tokenization: the cost of rendering cards.
    struct Counting<'a> {
        inner: &'a DocumentTokenizer,
        encodes: std::sync::atomic::AtomicUsize,
    }

    impl<'a> Counting<'a> {
        fn new(inner: &'a DocumentTokenizer) -> Self {
            Self {
                inner,
                encodes: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl TokenCount for Counting<'_> {
        fn encode(&self, rendered: &str) -> Result<partition::TokenizedText, ProviderError> {
            self.encodes
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.inner.encode(rendered)
        }
    }

    #[test]
    fn cpu_pool_is_lazy_reused_and_recovers_after_a_panicking_wave() {
        use std::sync::Mutex;
        struct Threads<'a> {
            inner: &'a DocumentTokenizer,
            seen: Mutex<HashSet<std::thread::ThreadId>>,
        }
        impl TokenCount for Threads<'_> {
            fn encode(&self, text: &str) -> Result<partition::TokenizedText, ProviderError> {
                self.seen
                    .lock()
                    .unwrap()
                    .insert(std::thread::current().id());
                self.inner.encode(text)
            }
        }
        struct Panics;
        impl TokenCount for Panics {
            fn encode(&self, _: &str) -> Result<partition::TokenizedText, ProviderError> {
                panic!("injected partition panic")
            }
        }
        let (_dir, engine, profile) = partition_fixture();
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let threads = Threads {
            inner: &tokenizer,
            seen: Mutex::new(HashSet::new()),
        };
        let digest = profile.descriptor.digest();
        let cards = Cards {
            tokenizer: &threads,
            profile: &profile,
            function_digest: &digest,
        };
        let pool = PartitionWorkers::new(1);
        pool.prepare(cards, vec![], "recipe", &Control::unbounded())
            .unwrap();
        assert!(pool.pool.get().is_none(), "idle work starts no threads");
        assert!(
            pool.prepare(cards, inputs(&engine, 1), "recipe", &Control::cancelled())
                .is_err()
        );
        assert!(
            pool.pool.get().is_none(),
            "cancelled work starts no threads"
        );
        for _ in 0..3 {
            pool.prepare(cards, inputs(&engine, 2), "recipe", &Control::unbounded())
                .unwrap();
        }
        let error = pool
            .prepare(
                Cards {
                    tokenizer: &Panics,
                    ..cards
                },
                inputs(&engine, 2),
                "recipe",
                &Control::unbounded(),
            )
            .unwrap_err();
        assert_eq!(error.code(), "partition_worker_failed");
        pool.prepare(cards, inputs(&engine, 2), "recipe", &Control::unbounded())
            .unwrap();
        assert_eq!(
            threads.seen.lock().unwrap().len(),
            1,
            "same CPU thread across waves and failure"
        );
        assert!(engine.semantic_partition("note00.md").unwrap().is_none());
    }

    #[test]
    fn retained_cards_avoid_second_tokenization_across_batches_with_bounded_fallback() {
        use std::sync::atomic::Ordering;
        let (_dir, engine, profile) = partition_fixture();
        let body: String = (0..40)
            .map(|n| format!("/// Item {n}.\npub fn item_{n:02}() -> u32 {{\n    {n}\n}}\n\n"))
            .collect();
        engine.replace_source("lib.rs", &body).unwrap();
        let hash = engine.source("lib.rs").unwrap().unwrap().hash;
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let counting = Counting::new(&tokenizer);
        let digest = profile.descriptor.digest();
        let recipe = partition::recipe_id(&profile.descriptor.tokenizer, profile.card_tokens);
        let cards = Cards {
            tokenizer: &counting,
            profile: &profile,
            function_digest: &digest,
        };
        let expected = partition::cards(
            &body,
            "lib.rs",
            Lang::from_path("lib.rs"),
            &cards.recipe(),
            &counting,
        )
        .unwrap();
        let once = counting.encodes.load(Ordering::Relaxed);
        let pool = PartitionWorkers::new(2);
        // 512 admits the array poorly; 4096 allows some ids before overflowing.
        // Both must fall back without changing any card or dropping later units.
        for limit in [RETAINED_CARD_BYTES, 0, 512, 4096] {
            counting.encodes.store(0, Ordering::Relaxed);
            let source = engine.semantic_partition_source("lib.rs", &hash).unwrap();
            let computed = if limit == RETAINED_CARD_BYTES {
                pool.prepare(cards, vec![source], &recipe, &Control::unbounded())
                    .unwrap()
                    .pop()
                    .unwrap()
            } else {
                cards.prepare_source(source, &recipe, limit).unwrap()
            };
            let kept = computed.retained.is_some();
            assert_eq!(kept, limit == RETAINED_CARD_BYTES);
            engine
                .semantic_accept_partition(computed.partition)
                .unwrap();
            let mut walk = Walk::wave(Some("lib.rs".into()), None, Some("lib.rs".into()))
                .with_retained(computed.retained.into_iter().collect());
            let steps = Steps {
                engine: &engine,
                cards,
                recipe: &recipe,
            };
            let mut actual = Vec::new();
            loop {
                let mut selection = Selection::default();
                let end = matches!(
                    steps
                        .select_batch(
                            &mut walk,
                            &mut selection,
                            8,
                            usize::MAX,
                            &mut PrepareReport::default(),
                            &mut || None
                        )
                        .unwrap(),
                    Selected::End
                );
                let batch = cards.render(selection).unwrap();
                actual.extend(
                    batch
                        .keys
                        .into_iter()
                        .zip(batch.inputs.into_iter().map(|input| input.ids)),
                );
                if end {
                    break;
                }
            }
            assert_eq!(
                actual,
                expected
                    .iter()
                    .map(|card| (card.input_key.clone(), card.ids.clone()))
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                counting.encodes.load(Ordering::Relaxed),
                once * if kept { 1 } else { 2 }
            );
        }
    }

    #[test]
    fn retained_cards_recheck_versions_and_reject_differing_recorded_tuples() {
        let (_dir, engine, profile) = partition_fixture();
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let digest = profile.descriptor.digest();
        let recipe = partition::recipe_id(&profile.descriptor.tokenizer, profile.card_tokens);
        let cards = Cards {
            tokenizer: &tokenizer,
            profile: &profile,
            function_digest: &digest,
        };
        let pool = PartitionWorkers::new(1);
        let computed = pool
            .prepare(cards, inputs(&engine, 1), &recipe, &Control::unbounded())
            .unwrap()
            .pop()
            .unwrap();
        engine
            .semantic_accept_partition(computed.partition)
            .unwrap();
        let stale = computed.retained.unwrap();
        engine
            .replace_source("note00.md", "# Changed source\n\nNew behavior.\n")
            .unwrap();
        let computed = pool
            .prepare(cards, inputs(&engine, 1), &recipe, &Control::unbounded())
            .unwrap()
            .pop()
            .unwrap();
        engine
            .semantic_accept_partition(computed.partition)
            .unwrap();
        let current = computed.retained.unwrap();
        let expected = current.cards.clone();
        let counting = Counting::new(&tokenizer);
        let steps = Steps {
            engine: &engine,
            cards: Cards {
                tokenizer: &counting,
                ..cards
            },
            recipe: &recipe,
        };
        let mut walk = Walk::wave(Some("note00.md".into()), None, Some("note00.md".into()))
            .with_retained(vec![stale]);
        let mut selection = Selection::default();
        steps
            .select_batch(
                &mut walk,
                &mut selection,
                8,
                usize::MAX,
                &mut PrepareReport::default(),
                &mut || None,
            )
            .unwrap();
        let batch = steps.cards.render(selection).unwrap();
        assert_eq!(
            batch.keys,
            expected
                .iter()
                .map(|card| card.input_key.clone())
                .collect::<Vec<_>>()
        );
        assert!(
            counting.encodes.load(std::sync::atomic::Ordering::Relaxed) > 0,
            "a newer version is freshly rendered, never taken from the old handoff"
        );

        let mut walk = Walk::wave(Some("note00.md".into()), None, Some("note00.md".into()))
            .with_retained(vec![current]);
        let mut selection = Selection::default();
        steps
            .select_batch(
                &mut walk,
                &mut selection,
                8,
                usize::MAX,
                &mut PrepareReport::default(),
                &mut || None,
            )
            .unwrap();
        selection.sources[0].wanted[0].1.input_key = "00".repeat(32);
        assert_eq!(
            cards.render(selection).err().unwrap().code(),
            "partition_invalid"
        );
    }

    /// 009 T004 M4: the embed walk renders and tokenizes only the cards it
    /// selected. Five batches of eight cards from ONE source of forty
    /// definitions cost exactly what rendering each card once costs, not
    /// five renderings of the whole source.
    #[test]
    fn a_partially_consumed_source_renders_only_its_selected_cards() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        let mut source = String::new();
        for n in 0..40 {
            source.push_str(&format!(
                "/// Item {n}.\npub fn item_{n:02}() -> u32 {{\n    {n}\n}}\n\n"
            ));
        }
        std::fs::write(root.join("lib.rs"), &source).unwrap();
        let mut engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
        engine.index(&root, &Control::unbounded()).unwrap();
        let path = crate::testkit::write_semantic_profile(dir.path(), "steps", |_| {});
        let profile = SemanticProfile::load(&path).unwrap();
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let digest = profile.descriptor.digest();
        let recipe = partition::recipe_id(&profile.descriptor.tokenizer, profile.card_tokens);
        let control = Control::unbounded();
        let mut report = PrepareReport::default();
        let partitioning = Steps {
            engine: &engine,
            cards: Cards {
                tokenizer: &tokenizer,
                profile: &profile,
                function_digest: &digest,
            },
            recipe: &recipe,
        };
        let mut after = None;
        while let Progress::More = partitioning
            .partition_page(
                &mut after,
                usize::MAX,
                None,
                &control,
                &mut report,
                &mut || None,
            )
            .unwrap()
        {}
        // What rendering every card once costs.
        let once = Counting::new(&tokenizer);
        let cards = partition::cards(
            &source,
            "lib.rs",
            Lang::from_path("lib.rs"),
            &partitioning.cards.recipe(),
            &once,
        )
        .unwrap();
        assert_eq!(cards.len(), 40);
        // The embed walk, selection then rendering, batch by batch.
        let counting = Counting::new(&tokenizer);
        let steps = Steps {
            engine: &engine,
            cards: Cards {
                tokenizer: &counting,
                profile: &profile,
                function_digest: &digest,
            },
            recipe: &recipe,
        };
        let mut walk = Walk::default();
        let mut batches = Vec::new();
        loop {
            let mut selection = Selection::default();
            let end = match steps
                .select_batch(
                    &mut walk,
                    &mut selection,
                    8,
                    usize::MAX,
                    &mut report,
                    &mut || None,
                )
                .unwrap()
            {
                Selected::Full => false,
                Selected::End => true,
                Selected::Yield => continue,
                Selected::Halted(_) => unreachable!("nothing halts this walk"),
            };
            let batch = steps.cards.render(selection).unwrap();
            if !batch.is_empty() {
                batches.push(batch.len());
            }
            if end {
                break;
            }
        }
        assert_eq!(batches, [8; 5]);
        assert_eq!(
            counting.encodes.load(std::sync::atomic::Ordering::Relaxed),
            once.encodes.load(std::sync::atomic::Ordering::Relaxed),
            "each selected card is rendered once, nothing else"
        );
    }
}
