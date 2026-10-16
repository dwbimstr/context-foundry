//! The bounded CLI preparation run (`semantic prepare`): the supervised
//! provider, budget and admission checks, the wave loop, publication and the
//! state row it begins and finalizes. Scheduling and the shared steps the MCP
//! driver also composes stay in the parent module.
use super::*;

/// The CLI entry: the provider seam resolves to the supervised worker on
/// macOS/Linux, and to an explicit `isolation_unavailable` elsewhere.
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
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    return Box::new(crate::neural::supervisor::acquire_verified_until);
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
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
    report.bootstrap.area_vectors = report.area_vectors;
    report.bootstrap.embedded_areas = report.embedded_areas;
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
    let (profile, deadline) = preflight(options)?;
    let engine = Engine::open_existing(store_dir)?;
    run_on_engine(&engine, options, &profile, deadline, acquire)
}

/// Bootstrap already owns and has checked the root-bound store. Keep that owner
/// across source indexing and semantic publication instead of reopening the store.
pub(crate) fn cli_prepare_on_engine(
    engine: &Engine,
    options: &PrepareOptions<'_>,
) -> FResult<PrepareReport> {
    let (profile, deadline) = preflight(options)?;
    run_on_engine(engine, options, &profile, deadline, supervisor_acquire())
}

fn preflight(options: &PrepareOptions<'_>) -> FResult<(SemanticProfile, Instant)> {
    options.control.check()?;
    validate_priority_paths(options.priority_paths)?;
    let deadline = options
        .started
        .checked_add(Duration::from_secs(options.budget_seconds))
        .ok_or_else(|| {
            FoundryError::InvalidArgument("budget-seconds overflows the clock".into())
        })?;
    // The profile is parsed strictly and bounded before any worker exists;
    // a missing/mismatched profile, worker or artifact is a named failure
    // with no download, install or fallback execution, and a descriptor v1
    // profile is `profile_unsupported` before the store is touched.
    let profile = SemanticProfile::load(options.profile_path)?;
    Ok((profile, deadline))
}

fn run_on_engine(
    engine: &Engine,
    options: &PrepareOptions<'_>,
    profile: &SemanticProfile,
    deadline: Instant,
    acquire: Acquire,
) -> FResult<PrepareReport> {
    let reserve = publication_reserve(options.budget_seconds);
    // ONE control bounds verification, acquisition, calls and publication.
    let run_control = options.control.bounded_by(deadline);
    let function_digest = profile.descriptor.digest();
    let recipe = profile.recipe_id();

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
    begin(engine, &report)?;

    let outcome = inner(
        engine,
        options,
        profile,
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
    finalize(engine, &mut report, &outcome)?;
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
    let revision = engine.source_revision()?;
    report.coverage = final_state
        .coverage
        .as_ref()
        .filter(|coverage| {
            coverage.source_revision == revision
                && coverage.function_digest == report.function_digest
                && coverage.recipe_id == report.recipe_id
        })
        .cloned();
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
    let artifacts = match profile.verified_tokenizer_until(control) {
        Ok(artifacts) => artifacts,
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
    let tokenizer = DocumentTokenizer::load_tokenizer_verified(profile, &artifacts, control)?;
    let mut artifacts = Some(artifacts);
    // Cache-only preparation verifies no unused weights. Missing inputs promote
    // the private tokenizer receipt to a fully verified acquisition capability.
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
    let mut order =
        SourceOrder::new(options.priority_paths)?.for_representation(profile.representation);
    let mut overview_phase = order.overview_first();
    let mut paused = PausedSources::default();
    let mut stop: Option<Stop> = None;
    'waves: while let Some(mut cursor) = order.next() {
        if let Some(halted) = halt(control, budget) {
            stop = Some(halted);
            break;
        }
        if overview_phase && !order.overview_first() {
            if let Some(halted) = publish(engine, control, budget, report)? {
                stop = Some(halted);
                break;
            }
            published = report.index_entries;
            unpublished = 0;
            overview_phase = false;
            snapshot_progress(report, options.started);
        }
        let resumed = cursor.resume.take();
        let before = resumed
            .as_ref()
            .map_or_else(|| cursor.after.clone(), |position| position.after.clone());
        let (progress, inputs) = if resumed.is_some() {
            (Progress::More, Vec::new())
        } else {
            steps.partition_inputs_in(
                &mut cursor.after,
                cursor.scope.as_deref(),
                cursor.exact,
                PARTITION_WAVE,
                None,
                report,
                &mut || halt(control, budget),
            )?
        };
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
            // The run owns the store, so no source can change under it.
            commit_partition(engine, computed.partition, control, report)?.committed()?;
            retained.extend(computed.retained);
        }
        let done = matches!(progress, Progress::Done);
        if cursor.after != before {
            report.bootstrap.waves += 1;
            report.bootstrap.priority_waves += u64::from(cursor.scope.is_some());
            let mut walk = Walk::wave(cursor.scope.clone(), before.clone(), cursor.after.clone())
                .with_retained(retained);
            if let Some(position) = resumed {
                walk = walk.restore(position, &mut paused);
            }
            walk.exact = cursor.exact;
            walk.overview_first = order.overview_first();
            let mut pending = Batch::default();
            let mut end = false;
            let mut wave_inputs = 0;
            loop {
                if pending.is_empty() {
                    let mut selection = Selection::default();
                    end = match steps.select_batch(
                        &mut walk,
                        &mut selection,
                        if order.waiting() {
                            steps.cards.limits().inputs.min(WAVE_INPUTS - wave_inputs)
                        } else {
                            steps.cards.limits().inputs
                        },
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
                    pending = batch?;
                }
                let batch = pending.take(steps.cards.limits(), steps.cards.limits().inputs)?;
                if batch.is_empty() {
                    break;
                }
                if let Some(halted) = admit(control, budget, deadline, reserve) {
                    stop = Some(halted);
                    break 'waves;
                }
                if provider.is_none() {
                    let acquired = match artifacts
                        .take()
                        .expect("first acquisition")
                        .complete(profile, control)
                        .and_then(|artifacts| {
                            acquire(
                                profile,
                                &std::sync::Arc::new(artifacts),
                                options.development,
                                control,
                            )
                        }) {
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
                        wave_inputs += size as usize;
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
                if end && pending.is_empty() {
                    break;
                }
                if wave_inputs >= WAVE_INPUTS && order.waiting() {
                    cursor.resume = Some(walk.pause(&mut paused));
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

    if stop.is_none() {
        let areas = super::area::inputs(engine, profile, &tokenizer, control)?;
        super::area::prune(engine, profile, &areas, control)?;
        for area in areas {
            if let Some(halted) = admit(control, budget, deadline, reserve) {
                stop = Some(halted);
                break;
            }
            if !super::area::cached(engine, profile, &area)? {
                if provider.is_none() {
                    let verified = artifacts
                        .take()
                        .expect("first acquisition")
                        .complete(profile, control)?;
                    let acquired = acquire(
                        profile,
                        &std::sync::Arc::new(verified),
                        options.development,
                        control,
                    )?;
                    if acquired.descriptor().digest() != function_digest {
                        return Err(FoundryError::Semantic {
                            code: "provider_malformed",
                            message: "area provider identity differs".into(),
                        });
                    }
                    provider = Some(acquired);
                    report.provider_state = Some("ready");
                }
                if let Some(halted) = admit(control, budget, deadline, reserve) {
                    stop = Some(halted);
                    break;
                }
                let batch = Batch {
                    keys: vec![area.key().to_owned()],
                    inputs: vec![TokenizedInput {
                        ids: area.ids.clone(),
                    }],
                };
                match flush_batch(
                    &steps,
                    provider.as_mut().expect("acquired area provider").as_mut(),
                    batch,
                    options.cache_cap_bytes,
                    control,
                    report,
                ) {
                    Ok(()) => report.embedded_areas += 1,
                    Err(StopOrError::Stop(partial)) => {
                        stop = Some(partial);
                        break;
                    }
                    Err(StopOrError::Error(error)) => return Err(error),
                }
            }
            // The run owns the store, so no brief or source can change under it.
            if super::area::bind(engine, profile, &area.record, control)? {
                report.area_vectors += 1;
            } else {
                stop = Some(Stop::partial(
                    "stale_area",
                    "area changed during preparation",
                ));
                break;
            }
        }
    }

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
