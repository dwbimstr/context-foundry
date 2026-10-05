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
//! Work pages current sources in pages of 128, embeds at most
//! [`DOCUMENT_BATCH`] inputs per call with ONE batch outstanding, and commits
//! the cache per batch (the committed counts move in the same transaction). A
//! stop caused by the budget is reported as `budget_exhausted` whatever the
//! provider returned. Startup never resumes preparation: only this explicit
//! command does.
//!
//! The provider comes through the [`Acquire`] seam (slice B's supervised
//! worker); whenever no accepted isolation profile admits model execution it
//! returns `isolation_unavailable` and source access stays intact. The state
//! the run observed (ready / refused / failed) is recorded with its time and
//! profile for `status`; there is no live probe.
use crate::control::Control;
use crate::error::{FResult, FoundryError};
use crate::neural::cache::{
    self, CacheLookup, DEFAULT_CACHE_CAP_BYTES, PartitionRecord, PartitionUnit,
    ProviderObservation, StateError,
};
use crate::neural::index::Publication;
use crate::neural::partition::{self, TokenCount as _};
use crate::neural::profile::SemanticProfile;
use crate::neural::provider::{
    self, DOCUMENT_BATCH, EmbeddingProvider, ProviderError, TokenizedInput,
};
use crate::neural::tokenize::DocumentTokenizer;
use crate::store::Engine;
use crate::syntax::Lang;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
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
    pub sources: u64,
    pub partitioned_sources: u64,
    pub reused_partitions: u64,
    pub eligible_units: u64,
    pub embedded_units: u64,
    pub reused_cached_units: u64,
    pub missing_units: u64,
    pub document_calls: u64,
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
enum Stop {
    Complete,
    Partial { code: &'static str, message: String },
}

impl Stop {
    fn partial(code: &'static str, message: impl Into<String>) -> Self {
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
    // a missing/mismatched profile, worker, runtime or artifact is a named
    // failure with no download, install or fallback execution.
    let profile = SemanticProfile::load(options.profile_path)?;
    let function_digest = profile.descriptor.digest();
    let engine = Engine::open_existing(store_dir)?;
    let recipe = partition::recipe_id(&profile.descriptor.tokenizer);

    let mut report = PrepareReport {
        profile: profile.name.clone(),
        function_digest: function_digest.clone(),
        recipe_id: recipe.clone(),
        budget_seconds: options.budget_seconds,
        publication_reserve_seconds: reserve.as_secs(),
        ..PrepareReport::default()
    };
    let mut state = engine.semantic_state()?.unwrap_or_default();
    state.profile_name = Some(profile.name.clone());
    state.function_digest = Some(function_digest.clone());
    state.recipe_id = Some(recipe.clone());
    state.state = "running".into();
    state.last_error = None;
    engine.semantic_set_state(&state)?;

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
    let mut final_state = engine.semantic_state()?.unwrap_or_default();
    final_state.profile_name = Some(profile.name.clone());
    final_state.function_digest = Some(function_digest.clone());
    final_state.recipe_id = Some(recipe.clone());
    let (totals_rows, totals_bytes) = engine.semantic_cache_totals()?;
    final_state.cache_bytes = totals_bytes;
    let (state_name, reason) = match &outcome {
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
            profile: profile.name.clone(),
            function_digest: function_digest.clone(),
        });
    }
    report.state = state_name;
    if let Some((code, _)) = reason {
        report.partial = true;
        report.reason_code = Some(code);
    }
    report.cache_entries = totals_rows;
    report.cache_bytes = totals_bytes;
    report.elapsed_ms = options
        .started
        .elapsed()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    engine.semantic_set_state(&final_state)?;
    match outcome {
        Ok(_) => Ok(report),
        Err(e) => Err(e),
    }
}

/// Semantic stop codes that can persist in the state row.
fn lifetime_code(code: &str) -> &'static str {
    const KNOWN: &[&str] = &[
        "budget_exhausted",
        "cache_full",
        "cancelled",
        "deadline_exceeded",
        "profile_invalid",
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
fn publish(
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
    let stop = match outcome {
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
    };
    if stop.is_none() {
        prepare_fault!(AFTER_PUBLISH, Some(control), "")?;
    }
    Ok(stop)
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

    // --- Partition pass: current sources, pages of 128, commit per source.
    let mut after: Option<String> = None;
    loop {
        if let Some(stop) = halt(control, budget) {
            return Ok(stop);
        }
        let page = cache::source_page(&engine.db, after.as_deref())?;
        if page.is_empty() {
            break;
        }
        for (path, meta) in &page {
            report.sources += 1;
            if let Some(existing) = engine.semantic_partition(path)?
                && cache::partition_is_current(&existing, meta, recipe, function_digest)
            {
                report.reused_partitions += 1;
                continue;
            }
            if let Some(stop) = halt(control, budget) {
                return Ok(stop);
            }
            let body = engine.semantic_source_body(path, meta)?;
            let lang = Lang::from_path(path);
            let units = partition::partition(&body, lang, function_digest, &tokenizer)?;
            let record = PartitionRecord {
                source_hash: meta.hash.clone(),
                recipe_id: recipe.to_owned(),
                function_digest: function_digest.to_owned(),
                units: units
                    .iter()
                    .map(|unit| PartitionUnit {
                        start: unit.start,
                        end: unit.end,
                        input_key: unit.input_key.clone(),
                    })
                    .collect(),
            };
            engine.semantic_record_partition(path, &record)?;
            report.partitioned_sources += 1;
            prepare_fault!(PARTITION_AFTER_COMMIT, Some(control), path)?;
        }
        after = page.last().map(|(path, _)| path.clone());
    }

    // --- Pending publication: committed coverage from earlier runs becomes
    // searchable BEFORE any new inference is admitted.
    if let Some(stop) = publish(engine, control, budget, report, false)? {
        return Ok(stop);
    }

    // --- Embed pass: remaining work = current units minus valid cached
    // results, pages of 128, batches of at most 8, one outstanding, commit
    // per batch. Admission checks the run control AND the publication
    // reserve immediately before every batch, and the control after every
    // flush.
    let mut resolved_keys: HashSet<String> = HashSet::new();
    let mut pending: Vec<(String, Vec<u32>)> = Vec::with_capacity(DOCUMENT_BATCH);
    let mut committed: u64 = 0;
    let mut stop: Option<Stop> = None;
    let mut after: Option<String> = None;
    'embed: loop {
        if let Some(halted) = halt(control, budget) {
            stop = Some(halted);
            break 'embed;
        }
        let page = cache::source_page(&engine.db, after.as_deref())?;
        if page.is_empty() {
            break 'embed;
        }
        for (path, meta) in &page {
            let Some(record) = engine.semantic_partition(path)? else {
                continue;
            };
            if !cache::partition_is_current(&record, meta, recipe, function_digest) {
                continue;
            }
            let body = engine.semantic_source_body(path, meta)?;
            for unit in &record.units {
                report.eligible_units += 1;
                if !resolved_keys.insert(unit.input_key.clone()) {
                    // Identical rendered inputs share this run's result.
                    report.reused_cached_units += 1;
                    continue;
                }
                // The lookup that USES a vector decodes it: a same-length
                // nonfinite (or otherwise tampered) row is disabled by name
                // here and replaced by a fresh embedding below.
                let needed = match engine.semantic_cache_lookup(&unit.input_key, function_digest)? {
                    CacheLookup::Hit(_) => false,
                    CacheLookup::Corrupt(_) => {
                        report.corrupt_cache_rows += 1;
                        true
                    }
                    CacheLookup::Miss => true,
                };
                if needed {
                    let text = body.get(unit.start..unit.end).ok_or_else(|| {
                        FoundryError::CorruptStore(format!(
                            "semantic partition of {path}: {}..{} is not a valid slice",
                            unit.start, unit.end
                        ))
                    })?;
                    let rendered = provider::render_document(text);
                    let ids = tokenizer.encode(&rendered)?.ids;
                    pending.push((unit.input_key.clone(), ids));
                } else {
                    report.reused_cached_units += 1;
                }
                if pending.len() == DOCUMENT_BATCH {
                    if let Some(halted) = admit(control, budget, deadline, reserve) {
                        stop = Some(halted);
                        break 'embed;
                    }
                    match flush_batch(
                        engine,
                        provider.as_mut(),
                        &pending,
                        function_digest,
                        options.cache_cap_bytes,
                        control,
                        report,
                    ) {
                        Ok(()) => committed += pending.len() as u64,
                        Err(StopOrError::Stop(partial)) => {
                            stop = Some(partial);
                            break 'embed;
                        }
                        Err(StopOrError::Error(error)) => return Err(error),
                    }
                    pending.clear();
                    if let Some(halted) = halt(control, budget) {
                        stop = Some(halted);
                        break 'embed;
                    }
                }
            }
        }
        after = page.last().map(|(path, _)| path.clone());
    }
    if stop.is_none() && !pending.is_empty() {
        if let Some(halted) = admit(control, budget, deadline, reserve) {
            stop = Some(halted);
        } else {
            match flush_batch(
                engine,
                provider.as_mut(),
                &pending,
                function_digest,
                options.cache_cap_bytes,
                control,
                report,
            ) {
                Ok(()) => {
                    committed += pending.len() as u64;
                    pending.clear();
                    // The short final batch gets the same post-flush check
                    // as a full one: a budget that expired during the call
                    // is reported, never silently swallowed as success.
                    stop = halt(control, budget);
                }
                Err(StopOrError::Stop(partial)) => stop = Some(partial),
                Err(StopOrError::Error(error)) => return Err(error),
            }
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
            Some(published_stop) => {
                stop = Some(match (stop, published_stop) {
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
                    (_, published_stop) => published_stop,
                });
            }
        }
    }
    Ok(stop.unwrap_or(Stop::Complete))
}

/// Marker to route stop-vs-error out of a batch flush.
enum StopOrError {
    Stop(Stop),
    Error(FoundryError),
}

/// Embed and commit one batch: limits checked before any model work, vectors
/// validated before the cache commit, the cache commit before any
/// publication. The run control crosses the seam unchanged; the supervisor
/// owns any in-flight grace.
fn flush_batch(
    engine: &Engine,
    provider: &mut dyn EmbeddingProvider,
    batch: &[(String, Vec<u32>)],
    function_digest: &str,
    cap_bytes: u64,
    control: &Control,
    report: &mut PrepareReport,
) -> Result<(), StopOrError> {
    let inputs: Vec<TokenizedInput> = batch
        .iter()
        .map(|(_, ids)| TokenizedInput { ids: ids.clone() })
        .collect();
    if let Err(e) = provider::check_document_batch(&inputs) {
        return Err(StopOrError::Error(FoundryError::from(e)));
    }
    report.document_calls += 1;
    let vectors = match provider.embed_documents(&inputs, control) {
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
    if vectors.len() != batch.len() {
        report.provider_state = Some("failed");
        report.provider_code = Some("provider_malformed");
        return Err(StopOrError::Stop(Stop::partial(
            "provider_malformed",
            format!(
                "the provider returned {} vectors for {} inputs",
                vectors.len(),
                batch.len()
            ),
        )));
    }
    for vector in &vectors {
        if let Err(e) = provider::validate_vector(vector) {
            // The batch is NOT committed: valid data stays intact.
            report.provider_state = Some("failed");
            report.provider_code = Some("provider_malformed");
            return Err(StopOrError::Stop(Stop::partial(
                "provider_malformed",
                e.to_string(),
            )));
        }
    }
    let entries: Vec<(String, Vec<f32>)> = batch
        .iter()
        .zip(vectors)
        .map(|((key, _), vector)| (key.clone(), vector))
        .collect();
    prepare_fault!(CACHE_BEFORE_COMMIT, Some(control), &batch.len().to_string())
        .map_err(StopOrError::Error)?;
    if let Err(e) = engine.semantic_cache_commit(&entries, function_digest, cap_bytes) {
        return Err(match e.code() {
            "cache_full" => StopOrError::Stop(Stop::partial(
                "cache_full",
                "the workspace cache reached its disk cap; nothing was evicted",
            )),
            _ => StopOrError::Error(e),
        });
    }
    // Commit-adjacent fault point: an exit here leaves the batch committed.
    prepare_fault!(CACHE_AFTER_COMMIT, Some(control), &batch.len().to_string())
        .map_err(StopOrError::Error)?;
    Ok(())
}
