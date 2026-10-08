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
//! engine-slot holds: [`Steps::partition_page`], [`Steps::select_batch`],
//! [`Cards::render`], [`commit_batch`], [`publish`], [`begin`] and
//! [`finalize`]. Only the ownership around them differs: the CLI holds the
//! store exclusively and calls the provider directly under its budget; the
//! driver takes the owner's engine slot per store step and embeds through
//! the resident runtime.
//!
//! 009 T004: the embedding inputs are cards ([`super::partition`]). A
//! partition is accepted only as the cards its verified body renders, in
//! the acceptance transaction. A batch is selected in a store step (which
//! cards are missing, with their recorded tuples and their source's body)
//! and only those cards are rendered and tokenized by [`Cards::render`],
//! which needs no engine and no transaction and checks each against its
//! recorded tuple, so the driver can prepare the next batch while one
//! inference runs.
//!
//! The provider comes through the [`Acquire`] seam (slice B's supervised
//! worker); whenever no accepted isolation profile admits model execution it
//! returns `isolation_unavailable` and source access stays intact. The state
//! the run observed (ready / refused / failed) is recorded with its time and
//! profile for `status`; there is no live probe.
use crate::control::Control;
use crate::error::{FResult, FoundryError};
use crate::neural::cache::{
    self, CacheLookup, DEFAULT_CACHE_CAP_BYTES, PartitionUnit, ProviderObservation, StateError,
};
use crate::neural::index::Publication;
use crate::neural::partition::{self, CardRecipe, TokenCount};
use crate::neural::profile::SemanticProfile;
use crate::neural::provider::{
    self, DocumentLimits, EmbeddingProvider, ProviderError, TokenizedInput,
};
use crate::neural::tokenize::DocumentTokenizer;
use crate::store::Engine;
use crate::syntax::{Lang, Unit};
use serde::Serialize;
use std::cell::OnceCell;
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The provider seam: profile, isolation flag and the run control (caller
/// cancellation plus the budget deadline) to one embedding function.
pub type Acquire = Box<
    dyn Fn(&SemanticProfile, bool, &Control) -> Result<Box<dyn EmbeddingProvider>, ProviderError>,
>;

/// The publication reserve: new batches stop when the remaining budget falls
/// below `max(5 s, budget/10)`.
pub fn publication_reserve(budget_seconds: u64) -> Duration {
    Duration::from_secs((budget_seconds / 10).max(5))
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PrepareReport {
    pub profile: String,
    pub function_digest: String,
    pub recipe_id: String,
    /// 009 T004: the profile's output dimension, recorded in the state row
    /// with the digest and recipe.
    pub dimensions: u32,
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

/// The CLI entry: the provider seam resolves to the supervised worker on
/// macOS, and to an explicit `isolation_unavailable` elsewhere.
pub fn cli_prepare(
    store_dir: &Path,
    profile_path: &Path,
    budget_seconds: u64,
    development: bool,
    started: Instant,
    control: &Control,
) -> FResult<PrepareReport> {
    run(
        store_dir,
        &PrepareOptions {
            profile_path,
            budget_seconds,
            development,
            cache_cap_bytes: DEFAULT_CACHE_CAP_BYTES,
            started,
            control,
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
    return Box::new(crate::neural::supervisor::acquire_until);
    #[cfg(not(target_os = "macos"))]
    return Box::new(
        |_profile: &SemanticProfile,
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

/// One bounded preparation run. Committed work is never rolled back; every
/// stop is named and recorded in the state row.
pub fn run(
    store_dir: &Path,
    options: &PrepareOptions<'_>,
    acquire: Acquire,
) -> FResult<PrepareReport> {
    options.control.check()?;
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
        budget_seconds: options.budget_seconds,
        publication_reserve_seconds: reserve.as_secs(),
        ..PrepareReport::default()
    };
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
    finalize(&engine, &mut report, &outcome)?;
    match outcome {
        Ok(_) => Ok(report),
        Err(e) => Err(e),
    }
}

/// Record a run's start in the state row: the profile identity (digest,
/// recipe and dimension), `running`, no error, and the cache-byte total
/// reconciled from the actual rows in the same transaction, so the disk cap
/// never trusts a total another binary recorded. A profile change never
/// purges: rows and generations of the earlier profile stay retained.
pub(crate) fn begin(engine: &Engine, report: &PrepareReport) -> FResult<()> {
    engine.semantic_begin_run(|state| {
        state.profile_name = Some(report.profile.clone());
        state.function_digest = Some(report.function_digest.clone());
        state.recipe_id = Some(report.recipe_id.clone());
        state.dimensions = Some(report.dimensions);
        state.state = "running".into();
        state.last_error = None;
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

/// Publish committed coverage under the run control: `rebuild` replays the
/// generation from the cache; otherwise only pending (uncovered) coverage is
/// published. The outcome is recorded in the report — a publication that
/// cannot finish is a named stop with its reason, never swallowed.
pub(crate) fn publish(
    engine: &Engine,
    control: &Control,
    budget: u64,
    report: &mut PrepareReport,
    rebuild: bool,
) -> Result<Option<Stop>, FoundryError> {
    prepare_fault!(BEFORE_PUBLISH, Some(control), "")?;
    let outcome = if rebuild {
        engine.semantic_rebuild_index(control).map(|summary| {
            if summary.rebuilt {
                Publication::Rebuilt(summary.entries)
            } else {
                Publication::Nothing
            }
        })
    } else {
        engine.semantic_publish_pending(control)
    };
    let stop = record_publication(outcome, budget, report);
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
    match outcome {
        Ok(Publication::Nothing) => None,
        Ok(Publication::Current(entries) | Publication::Rebuilt(entries)) => {
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

/// The bounded work: verification, partition pass, pending publication,
/// embed pass, publication.
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
    if let Err(e) = profile.verify_artifacts_until(control) {
        return match halt(control, budget) {
            Some(stop) => Ok(stop),
            None => Err(FoundryError::from(e)),
        };
    }
    if let Some(stop) = halt(control, budget) {
        return Ok(stop);
    }
    let tokenizer = DocumentTokenizer::load(profile)?;
    // The provider seam gets the run control. A failure caused by the budget
    // or the caller's cancellation is that named stop; any other refusal
    // (isolation, profile) is a hard error that touches nothing.
    let mut provider = match acquire(profile, options.development, control) {
        Ok(provider) => {
            report.provider_state = Some("ready");
            provider
        }
        Err(error) => {
            return match halt(control, budget) {
                Some(stop) => Ok(stop),
                None => {
                    report.provider_state = Some("refused");
                    report.provider_code = Some(lifetime_code(error.code()));
                    Err(FoundryError::from(error))
                }
            };
        }
    };
    if provider.descriptor().digest() != function_digest {
        return Err(FoundryError::Semantic {
            code: "provider_malformed",
            message: "the worker's document function does not match the profile".into(),
        });
    }
    if let Some(stop) = halt(control, budget) {
        return Ok(stop);
    }

    let steps = Steps {
        engine,
        cards: Cards {
            tokenizer: &tokenizer,
            profile,
            function_digest,
        },
        recipe,
    };
    // --- Partition pass: current sources, pages of 128, commit per source.
    let mut after: Option<String> = None;
    loop {
        match steps.partition_page(&mut after, usize::MAX, None, control, report, &mut || {
            halt(control, budget)
        })? {
            Progress::More => {}
            Progress::Done => break,
            Progress::Halted(stop) => return Ok(stop),
        }
    }

    // --- Pending publication: committed coverage from earlier runs becomes
    // searchable BEFORE any new inference is admitted.
    if let Some(stop) = publish(engine, control, budget, report, false)? {
        return Ok(stop);
    }

    // --- Embed pass: remaining work = current cards minus valid cached
    // results, pages of 128, batches of at most the profile's batch, one
    // outstanding, commit per batch. Admission checks the run control AND
    // the publication reserve immediately before every batch, and the
    // control after every flush.
    let mut walk = Walk::default();
    let mut committed: u64 = 0;
    let mut stop: Option<Stop> = None;
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
                break;
            }
        };
        let batch = steps.cards.render(selection)?;
        if batch.is_empty() {
            break;
        }
        if let Some(halted) = admit(control, budget, deadline, reserve) {
            stop = Some(halted);
            break;
        }
        let size = batch.len() as u64;
        match flush_batch(
            &steps,
            provider.as_mut(),
            batch,
            options.cache_cap_bytes,
            control,
            report,
        ) {
            Ok(()) => committed += size,
            Err(StopOrError::Stop(partial)) => {
                stop = Some(partial);
                break;
            }
            Err(StopOrError::Error(error)) => return Err(error),
        }
        // Every batch, the short final one included, gets the post-flush
        // check: a budget that expired during the call is reported, never
        // silently swallowed as success.
        if let Some(halted) = halt(control, budget) {
            stop = Some(halted);
            break;
        }
        if end {
            break;
        }
    }
    report.embedded_units = committed;
    report.missing_units = report
        .eligible_units
        .saturating_sub(report.reused_cached_units + committed);

    // --- Publication: new vectors were committed, so replay the generation
    // from the cache under the run control — after a complete pass AND after
    // a bounded stop, so partial coverage is searchable. A cancelled run
    // publishes nothing more. If publication cannot finish, that is recorded
    // in the report and the state; the original stop reason is kept.
    let cancelled = matches!(&stop, Some(Stop::Partial { code, .. }) if *code == "cancelled");
    if committed > 0 && !cancelled {
        match publish(engine, control, budget, report, true)? {
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
    let vectors = match provider.embed_documents(&batch.inputs, control) {
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
    pub tokenizer: &'a dyn TokenCount,
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
/// is the source version in progress and its next card, and `seen` holds
/// the input keys this run already resolved — identical rendered inputs
/// share one result.
#[derive(Default)]
pub(crate) struct Walk {
    after: Option<String>,
    partial: Option<(Rc<SourceVersion>, usize)>,
    seen: HashSet<String>,
}

/// One source version a selection drew cards from: its verified body and,
/// once rendering needed them, its carded units. The version the walk
/// pauses in carries both to its next selection, so a source consumed over
/// many batches is read and parsed once.
struct SourceVersion {
    path: String,
    hash: String,
    body: String,
    units: OnceCell<Vec<Unit>>,
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

    /// The card tuples of one source body: what acceptance records.
    fn units(&self, path: &str, body: &str) -> FResult<Vec<PartitionUnit>> {
        let cards = partition::cards(
            body,
            path,
            Lang::from_path(path),
            &self.recipe(),
            self.tokenizer,
        )?;
        Ok(cards
            .into_iter()
            .map(|card| PartitionUnit {
                start: card.start,
                end: card.end,
                input_key: card.input_key,
            })
            .collect())
    }

    /// Render and tokenize a selection into its batch, in selection order,
    /// with no engine and no transaction. Only the selected cards are
    /// rendered (a source version's units are parsed once per walk), and
    /// each must be exactly the card its partition records, the same range
    /// under the same key, else the mapping is `partition_invalid`.
    pub(crate) fn render(&self, selection: Selection) -> FResult<Batch> {
        let recipe = self.recipe();
        let mut batch = Batch::default();
        for selected in selection.sources {
            let source = &selected.source;
            let units = source.units.get_or_init(|| {
                partition::carded_units(&source.body, Lang::from_path(&source.path))
            });
            if units.len() != selected.cards {
                return Err(partition_invalid(format!(
                    "{}: the partition records {} cards, but the body has {} carded units",
                    source.path,
                    selected.cards,
                    units.len()
                )));
            }
            for (index, recorded) in selected.wanted {
                let card = partition::card(
                    &source.body,
                    &source.path,
                    &units[index],
                    &recipe,
                    self.tokenizer,
                )?;
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
    /// Partition the current sources after `after`, one page at most, until
    /// `max_new` partitions were written or, once at least one source was
    /// examined, `until` passed (009 T003: the MCP owner's driver keeps each
    /// engine-slot step short): each source without a current partition is
    /// carded from its committed body and its mapping accepted in its own
    /// transaction. `halt` is asked before the page and before every source
    /// that needs work. (`control` reaches only the test-faults point after
    /// each partition commit.)
    #[cfg_attr(not(feature = "test-faults"), allow(unused_variables))]
    pub(crate) fn partition_page(
        &self,
        after: &mut Option<String>,
        max_new: usize,
        until: Option<Instant>,
        control: &Control,
        report: &mut PrepareReport,
        halt: &mut dyn FnMut() -> Option<Stop>,
    ) -> FResult<Progress> {
        if let Some(stop) = halt() {
            return Ok(Progress::Halted(stop));
        }
        let page = cache::source_page(&self.engine.db, after.as_deref())?;
        if page.is_empty() {
            return Ok(Progress::Done);
        }
        let mut written = 0usize;
        for (examined, (path, meta)) in page.iter().enumerate() {
            if written == max_new
                || (examined > 0 && until.is_some_and(|until| Instant::now() >= until))
            {
                return Ok(Progress::More);
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
                return Ok(Progress::Halted(stop));
            }
            let cards = self.cards;
            self.engine.semantic_record_partition(
                path,
                &meta.hash,
                self.recipe,
                cards.function_digest,
                None,
                &|body| cards.units(path, body),
            )?;
            report.partitioned_sources += 1;
            written += 1;
            *after = Some(path.clone());
            prepare_fault!(PARTITION_AFTER_COMMIT, Some(control), path)?;
        }
        Ok(Progress::More)
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
            let page = cache::source_page(&self.engine.db, walk.after.as_deref())?;
            if page.is_empty() {
                return Ok(Selected::End);
            }
            for (path, meta) in &page {
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
                        if !walk.seen.insert(unit.input_key.clone()) {
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
                        wanted.push((index, unit.clone()));
                        if selection.len + wanted.len() == room {
                            full_at = Some(index + 1);
                            break;
                        }
                    }
                    if !wanted.is_empty() {
                        let source = match resumed {
                            Some((source, _)) => source,
                            None => Rc::new(SourceVersion {
                                path: path.clone(),
                                hash: meta.hash.clone(),
                                body: self.engine.semantic_source_body(path, meta)?,
                                units: OnceCell::new(),
                            }),
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

#[cfg(all(test, feature = "test-faults"))]
mod tests {
    use super::*;

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
        encodes: std::cell::Cell<usize>,
    }

    impl<'a> Counting<'a> {
        fn new(inner: &'a DocumentTokenizer) -> Self {
            Self {
                inner,
                encodes: std::cell::Cell::new(0),
            }
        }
    }

    impl TokenCount for Counting<'_> {
        fn encode(&self, rendered: &str) -> Result<partition::TokenizedText, ProviderError> {
            self.encodes.set(self.encodes.get() + 1);
            self.inner.encode(rendered)
        }
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
            counting.encodes.get(),
            once.encodes.get(),
            "each selected card is rendered once, nothing else"
        );
    }
}
