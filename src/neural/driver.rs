//! 009 T003: progressive preparation inside the MCP owner (`index
//! {semantic: "prepare" | "pause"}`).
//!
//! ONE background driver thread per owner, for the PRIMARY root only. It
//! composes the CLI's own preparation steps ([`super::prepare`]): partition a
//! few sources, publish pending committed coverage, select one batch of at
//! most [`DOCUMENT_BATCH`] missing inputs, embed it, validate and commit it,
//! publish. Only the ownership around the steps differs:
//!
//! - every store step takes the owner's ONE engine slot, and only while no
//!   foreground operation is in flight, and gives it back before the next
//!   step; a foreground operation arriving during such a brief step gets the
//!   adapter's usual retryable `busy`;
//! - the model call runs on the owner's ONE resident worker
//!   ([`QueryRuntime`]) with NO engine slot and NO transaction held. Its
//!   admission is refused, never queued, while that slot is occupied or a
//!   foreground query is being dispatched, and the refusal pauses
//!   preparation with `provider_busy`.
//!
//! Publication happens at most once per committed batch: after a commit
//! that brings the vectors not yet published up to the size of the last
//! published generation (so partial coverage becomes searchable after the
//! first batch, while the total rebuild work stays linear in the corpus),
//! and at every stop except cancellation. Each publication makes the
//! runtime reload the dense index.
//!
//! Stops are recorded in the state row with their reason; committed work is
//! never rolled back:
//! - `pause` admits no new batch: the final stop check and the admission are
//!   one decision under the lock `pause` takes. The in-flight call finishes
//!   and is committed when valid (`paused`);
//! - owner shutdown or EOF discards the uncommitted batch, also one that
//!   returned and still waits for the engine slot (`cancelled`); the owner
//!   then stops and reaps the worker before it releases its store;
//! - a provider timeout ([`DOCUMENT_CALL_TIMEOUT`]), failure, malformed reply
//!   or wrong document function stops with its provider code and commits and
//!   publishes nothing from that batch;
//! - completion, a pass over an unchanged source revision, is `stopped` with
//!   no reason.
//!
//! There are no hidden retries, job journal, leases, retry queue or watcher.
//! Startup never resumes preparation: only an explicit `prepare` starts (or
//! resumes) the driver.
use crate::control::Control;
use crate::error::{FResult, FoundryError};
use crate::neural::cache::{self, DEFAULT_CACHE_CAP_BYTES, SemanticState};
use crate::neural::partition;
use crate::neural::prepare::{
    self, Batch, PrepareReport, Progress, Selected, Steps, Stop, StopOrError, Walk,
};
use crate::neural::provider::{DOCUMENT_BATCH, ProviderError};
use crate::neural::query::QueryRuntime;
use crate::store::Engine;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// Each document call's own deadline. The supervised worker then grants its
/// in-flight grace before stopping the worker; the stop is the resumable
/// `provider_timeout`.
pub const DOCUMENT_CALL_TIMEOUT: Duration = Duration::from_secs(60);
/// New partitions one store step may write.
const PARTITIONS_PER_STEP: usize = DOCUMENT_BATCH;
/// Sources one selection step may examine.
const SOURCES_PER_STEP: usize = cache::PAGE;
/// How long the driver waits before looking again for a free engine slot,
/// and the slice of its wait on a running document call.
const POLL: Duration = Duration::from_millis(10);

const PAUSED: &str = "preparation was paused on request; committed work stays and \
                      `index {semantic: \"prepare\"}` resumes";

/// What the driver needs from its owner.
pub trait Owner: Send + Sync {
    /// Run `step` on the primary engine under the owner's one engine slot,
    /// but only while no foreground operation is in flight. `Ok(false)` when
    /// the slot cannot be taken right now; `Err` when it never can again.
    fn try_primary(&self, step: &mut dyn FnMut(&Engine)) -> FResult<bool>;
    /// True once the owner is shutting down (EOF or owner shutdown).
    fn closing(&self) -> bool;
    /// Test seam: the driver selected a batch and is about to decide its
    /// admission.
    #[cfg(feature = "test-faults")]
    fn admitting(&self) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// No driver thread.
    Idle,
    Running,
    /// `pause` was requested: the driver admits no new batch.
    Pausing,
    /// The driver decided to stop and is recording it; an explicit `prepare`
    /// before that is written resumes the run instead of being lost.
    Stopping,
}

struct State {
    phase: Phase,
    /// The driver's own document call is in flight.
    in_call: bool,
}

/// The live state of this owner's driver, overlaid on the committed row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Live {
    Running,
    /// A pause was requested; at most the in-flight call still finishes.
    Paused,
}

/// The owner's preparation control: at most one driver thread and its
/// phase. Lock order: the engine slot, then this state; request handlers
/// take only this state.
pub struct Preparation {
    state: Mutex<State>,
}

impl Default for Preparation {
    fn default() -> Self {
        Self {
            state: Mutex::new(State {
                phase: Phase::Idle,
                in_call: false,
            }),
        }
    }
}

impl Preparation {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `index {semantic: "prepare"}`: start the driver, or resume a pausing
    /// or stopping one. While an earlier model call that is not the driver's
    /// own still occupies the runtime slot, this is refused `provider_busy`
    /// before anything is started or allocated; nothing is queued.
    pub fn prepare(
        self: &Arc<Self>,
        owner: Arc<dyn Owner>,
        runtime: Arc<QueryRuntime>,
    ) -> FResult<()> {
        let busy = || FoundryError::Semantic {
            code: "provider_busy",
            message: "an earlier model call still occupies the runtime slot; nothing was \
                      started or queued; retry after it ends"
                .into(),
        };
        let mut state = self.lock();
        match state.phase {
            Phase::Running => Ok(()),
            Phase::Pausing | Phase::Stopping => {
                if runtime.occupied() && !state.in_call {
                    return Err(busy());
                }
                state.phase = Phase::Running;
                Ok(())
            }
            Phase::Idle => {
                if runtime.occupied() {
                    return Err(busy());
                }
                let preparation = Arc::clone(self);
                std::thread::Builder::new()
                    .name("foundry-prepare".into())
                    .spawn(move || drive(owner.as_ref(), &runtime, &preparation))
                    .map_err(|e| {
                        FoundryError::Internal(anyhow::anyhow!("preparation thread: {e}"))
                    })?;
                state.phase = Phase::Running;
                Ok(())
            }
        }
    }

    /// `index {semantic: "pause"}`: no new batch is admitted.
    pub fn pause(&self) {
        let mut state = self.lock();
        if matches!(state.phase, Phase::Running | Phase::Stopping) {
            state.phase = Phase::Pausing;
        }
    }

    /// The live state while a driver thread exists.
    pub fn live(&self) -> Option<Live> {
        match self.lock().phase {
            Phase::Idle => None,
            Phase::Pausing => Some(Live::Paused),
            Phase::Running | Phase::Stopping => Some(Live::Running),
        }
    }

    /// No driver thread exists (none started, or it recorded its stop).
    pub fn idle(&self) -> bool {
        self.lock().phase == Phase::Idle
    }

    fn set_in_call(&self, in_call: bool) {
        self.lock().in_call = in_call;
    }

    /// The driver decided to stop on its own.
    fn decided(&self) {
        let mut state = self.lock();
        if state.phase == Phase::Running {
            state.phase = Phase::Stopping;
        }
    }
}

/// The driver thread: one run per start or explicit resume.
fn drive(owner: &dyn Owner, runtime: &QueryRuntime, preparation: &Preparation) {
    let control = Control::unbounded();
    loop {
        let mut run = Run::new(owner, runtime, preparation, &control);
        let outcome = run.begin().and_then(|()| run.passes());
        preparation.decided();
        if !run.finish(outcome) {
            return;
        }
    }
}

/// Run `step` on the primary engine under the engine slot, waiting while
/// foreground operations go first.
fn hold<T>(owner: &dyn Owner, step: impl FnOnce(&Engine) -> FResult<T>) -> FResult<T> {
    let mut step = Some(step);
    let mut out: Option<FResult<T>> = None;
    loop {
        let ran = owner.try_primary(&mut |engine| {
            if let Some(step) = step.take() {
                out = Some(step(engine));
            }
        })?;
        if ran && let Some(out) = out.take() {
            return out;
        }
        std::thread::sleep(POLL);
    }
}

fn cancelled() -> Stop {
    Stop::partial(
        "cancelled",
        "the owner shut down; committed work stays and the uncommitted batch was discarded",
    )
}

fn paused() -> Stop {
    Stop::partial("paused", PAUSED)
}

/// One run of the driver: the profile identity, its report and the
/// publication bookkeeping.
struct Run<'a> {
    owner: &'a dyn Owner,
    runtime: &'a QueryRuntime,
    preparation: &'a Preparation,
    /// Cancelled when the owner shuts down; each document call gets it
    /// bounded by [`DOCUMENT_CALL_TIMEOUT`].
    control: &'a Control,
    recipe: String,
    digest: String,
    report: PrepareReport,
    /// Vectors committed since the last publication.
    unpublished: u64,
    /// Entries of the last generation this run published.
    published: u64,
}

impl<'a> Run<'a> {
    fn new(
        owner: &'a dyn Owner,
        runtime: &'a QueryRuntime,
        preparation: &'a Preparation,
        control: &'a Control,
    ) -> Self {
        let profile = &runtime.profile;
        let digest = profile.descriptor.digest();
        let recipe = partition::recipe_id(&profile.descriptor.tokenizer);
        Self {
            owner,
            runtime,
            preparation,
            control,
            report: PrepareReport {
                profile: profile.name.clone(),
                function_digest: digest.clone(),
                recipe_id: recipe.clone(),
                // The resident runtime is up; a failed call says otherwise.
                provider_state: Some("ready"),
                ..PrepareReport::default()
            },
            recipe,
            digest,
            unpublished: 0,
            published: 0,
        }
    }

    /// The worker must compute the profile's document function; then the
    /// state row records the run.
    fn begin(&mut self) -> FResult<()> {
        if self.runtime.served_digest() != self.digest {
            self.report.provider_state = Some("failed");
            self.report.provider_code = Some("provider_malformed");
            return Err(FoundryError::Semantic {
                code: "provider_malformed",
                message: "the worker's document function does not match the profile".into(),
            });
        }
        let report = &self.report;
        hold(self.owner, |engine| prepare::begin(engine, report))
    }

    /// Passes until one saw an unchanged source revision, or a stop.
    fn passes(&mut self) -> FResult<Stop> {
        loop {
            let revision = hold(self.owner, |engine| engine.source_revision())?;
            if let Some(stop) = self.partition_pass()? {
                return Ok(stop);
            }
            if let Some(stop) = self.requested_stop() {
                return Ok(stop);
            }
            // Committed coverage of earlier runs becomes searchable before
            // any new inference.
            if let Some(stop) = self.publish(false)? {
                return Ok(stop);
            }
            if let Some(stop) = self.embed_pass()? {
                return Ok(stop);
            }
            // An explicit `index` during the pass can change sources the walk
            // already passed; walk again until a pass saw one revision.
            if hold(self.owner, |engine| engine.source_revision())? == revision {
                return Ok(Stop::Complete);
            }
        }
    }

    /// Owner shutdown cancels; a requested pause stops before the next batch.
    fn requested_stop(&self) -> Option<Stop> {
        if self.owner.closing() {
            self.control.cancel();
            return Some(cancelled());
        }
        (self.preparation.lock().phase == Phase::Pausing).then(paused)
    }

    fn partition_pass(&mut self) -> FResult<Option<Stop>> {
        let mut after: Option<String> = None;
        loop {
            if let Some(stop) = self.requested_stop() {
                return Ok(Some(stop));
            }
            let (owner, runtime, control) = (self.owner, self.runtime, self.control);
            let (recipe, digest, report) = (&self.recipe, &self.digest, &mut self.report);
            let progress = hold(owner, |engine| {
                Steps {
                    engine,
                    tokenizer: runtime.tokenizer(),
                    recipe,
                    function_digest: digest,
                }
                .partition_page(
                    &mut after,
                    PARTITIONS_PER_STEP,
                    control,
                    report,
                    &mut || owner.closing().then(cancelled),
                )
            })?;
            match progress {
                Progress::More => {}
                Progress::Done => return Ok(None),
                Progress::Halted(stop) => return Ok(Some(stop)),
            }
        }
    }

    fn embed_pass(&mut self) -> FResult<Option<Stop>> {
        let mut walk = Walk::default();
        let mut batch = Batch::default();
        loop {
            if let Some(stop) = self.requested_stop() {
                return Ok(Some(stop));
            }
            let (owner, runtime) = (self.owner, self.runtime);
            let (recipe, digest, report) = (&self.recipe, &self.digest, &mut self.report);
            let selected = hold(owner, |engine| {
                Steps {
                    engine,
                    tokenizer: runtime.tokenizer(),
                    recipe,
                    function_digest: digest,
                }
                .select_batch(
                    &mut walk,
                    &mut batch,
                    SOURCES_PER_STEP,
                    report,
                    &mut || owner.closing().then(cancelled),
                )
            })?;
            let end = match selected {
                Selected::Full => false,
                Selected::End => true,
                Selected::Yield => continue,
                Selected::Halted(stop) => return Ok(Some(stop)),
            };
            if !batch.is_empty()
                && let Some(stop) = self.embed(std::mem::take(&mut batch))?
            {
                return Ok(Some(stop));
            }
            if end {
                return Ok(None);
            }
        }
    }

    /// One batch. The final stop check, the admission on the runtime slot
    /// and the in-call mark are ONE decision under the preparation-state lock
    /// that `pause` takes, released before any wait on inference; the call
    /// runs with no engine slot or transaction held; validation and commit
    /// run under the slot.
    fn embed(&mut self, batch: Batch) -> FResult<Option<Stop>> {
        let Batch { keys, inputs } = batch;
        let tokens: u64 = inputs.iter().map(|input| input.ids.len() as u64).sum();
        #[cfg(feature = "test-faults")]
        self.owner.admitting();
        let preparation = self.preparation;
        let call = {
            let mut state = preparation.lock();
            if self.owner.closing() {
                self.control.cancel();
                return Ok(Some(cancelled()));
            }
            if state.phase == Phase::Pausing {
                return Ok(Some(paused()));
            }
            let job = self
                .control
                .bounded_by(Instant::now() + DOCUMENT_CALL_TIMEOUT);
            match self.runtime.dispatch_documents(inputs, job) {
                Ok(call) => {
                    state.in_call = true;
                    call
                }
                Err(error) => {
                    drop(state);
                    return Ok(Some(self.failed(error)));
                }
            }
        };
        self.report.document_calls += 1;
        self.report.input_tokens += tokens;
        let result = loop {
            if self.owner.closing() {
                // The uncommitted result is discarded; the call itself may
                // run on and keeps the runtime slot until it really ends.
                self.preparation.set_in_call(false);
                self.control.cancel();
                return Ok(Some(cancelled()));
            }
            if let Some(result) = call.wait(POLL) {
                break result;
            }
        };
        self.preparation.set_in_call(false);
        let vectors = match result {
            Ok(vectors) => vectors,
            Err(error) => return Ok(Some(self.failed(error))),
        };
        if let Some(stop) = self.owner.closing().then(cancelled) {
            self.control.cancel();
            return Ok(Some(stop));
        }
        let size = keys.len() as u64;
        let owner = self.owner;
        let (control, digest, report) = (self.control, &self.digest, &mut self.report);
        let committed = hold(owner, |engine| {
            // Shutdown may have arrived while the batch waited for the slot:
            // it is still uncommitted, so it is discarded and no transaction
            // starts.
            if owner.closing() {
                control.cancel();
                return Ok(Err(StopOrError::Stop(cancelled())));
            }
            Ok(prepare::commit_batch(
                engine,
                keys,
                vectors,
                digest,
                DEFAULT_CACHE_CAP_BYTES,
                control,
                report,
            ))
        })?;
        match committed {
            Ok(()) => {}
            Err(StopOrError::Stop(stop)) => return Ok(Some(stop)),
            Err(StopOrError::Error(error)) => return Err(error),
        }
        self.unpublished += size;
        if self.unpublished >= self.published.max(1) {
            return self.publish(true);
        }
        Ok(None)
    }

    /// The named stop for a refused or failed document call.
    fn failed(&mut self, error: ProviderError) -> Stop {
        if self.owner.closing() || error == ProviderError::Cancelled {
            return cancelled();
        }
        if error == ProviderError::Busy {
            return Stop::partial(
                "provider_busy",
                "the model slot was occupied by another call or a foreground query; \
                 nothing was queued and committed work stays",
            );
        }
        let code = prepare::lifetime_code(error.code());
        self.report.provider_state = Some("failed");
        self.report.provider_code = Some(code);
        Stop::partial(
            code,
            format!("the provider stopped the batch ({error}); committed work stays"),
        )
    }

    /// Publish committed coverage (`rebuild` replays the generation from the
    /// cache; otherwise only pending coverage), then drop the runtime's
    /// loaded index so the next request loads the new generation.
    fn publish(&mut self, rebuild: bool) -> FResult<Option<Stop>> {
        let owner = self.owner;
        let (control, report) = (self.control, &mut self.report);
        let stop = hold(owner, |engine| {
            // A run its owner is shutting down publishes nothing more.
            if owner.closing() {
                control.cancel();
            }
            prepare::publish(engine, control, 0, report, rebuild)
        })?;
        if stop.is_none() {
            if self.report.index_published {
                self.runtime.forget_index();
                self.published = self.report.index_entries;
            }
            self.unpublished = 0;
        }
        Ok(stop)
    }

    /// Record the stop: committed coverage is published first (except on
    /// cancellation), then the state row is finalized. Returns true when an
    /// explicit `prepare` arrived after the driver decided to stop: the
    /// driver then runs again instead of recording the stop.
    fn finish(mut self, outcome: FResult<Stop>) -> bool {
        let cancelled = self.owner.closing()
            || matches!(&outcome, Ok(Stop::Partial { code, .. }) if *code == "cancelled");
        let mut outcome = outcome;
        if !cancelled && self.unpublished > 0 {
            match self.publish(true) {
                Ok(None) => {}
                Ok(Some(published)) => {
                    outcome = outcome.map(|stop| prepare::merge_publication(Some(stop), published));
                }
                Err(error) => outcome = outcome.and(Err(error)),
            }
        }
        let (owner, preparation, report) = (self.owner, self.preparation, &mut self.report);
        let resumed = hold(owner, |engine| {
            let mut state = preparation.lock();
            if state.phase == Phase::Running && !owner.closing() {
                return Ok(true);
            }
            let written = prepare::finalize(engine, report, &outcome);
            state.phase = Phase::Idle;
            state.in_call = false;
            written.map(|()| false)
        });
        match resumed {
            Ok(resumed) => resumed,
            Err(error) => {
                eprintln!(
                    "foundry-mcp: semantic preparation could not record its stop: {error:.200}"
                );
                let mut state = preparation.lock();
                state.phase = Phase::Idle;
                state.in_call = false;
                false
            }
        }
    }
}

/// The `semantic` object of the MCP `status`: T001's metadata-only census of
/// the committed rows (paged, never a source body, tokenizer or model), with
/// this owner's live driver state overlaid, `reason` (the last error's code)
/// and the resident `runtime` (`ready` or its `fallback:` word). The census
/// gets half of the remaining read deadline; when that runs out, the object
/// carries the committed state row and names the cut-off census instead of
/// failing the whole status.
pub fn status_object(
    engine: &Engine,
    live: Option<Live>,
    runtime: &str,
    control: &Control,
) -> FResult<serde_json::Value> {
    let now = Instant::now();
    let bounded;
    let census_control = match control.deadline() {
        Some(deadline) => {
            bounded = control.bounded_by(now + deadline.saturating_duration_since(now) / 2);
            &bounded
        }
        None => control,
    };
    let mut value = match engine.semantic_status(census_control) {
        Ok(status) => serde_json::to_value(&status)?,
        Err(error @ FoundryError::DeadlineExceeded(_)) if control.check().is_ok() => {
            let row = engine
                .semantic_state()?
                .unwrap_or_else(SemanticState::stopped);
            serde_json::json!({
                "state": row.state,
                "last_error": row.last_error,
                "committed_units": row.committed_units,
                "census": error.code(),
            })
        }
        Err(error) => return Err(error),
    };
    overlay(&mut value, live);
    value["runtime"] = runtime.into();
    Ok(value)
}

/// The `index {semantic}` reply: the state and reason after the action, from
/// the live driver when one exists, else from the committed row.
pub fn brief(live: Option<Live>, row: Option<&SemanticState>) -> serde_json::Value {
    let mut value = serde_json::json!({
        "state": row.map_or("stopped", |row| row.state.as_str()),
        "last_error": row.and_then(|row| row.last_error.as_ref()),
    });
    overlay(&mut value, live);
    if let Some(object) = value.as_object_mut() {
        object.remove("last_error");
    }
    serde_json::json!({ "semantic": value })
}

/// Overlay this owner's live driver state; `reason` is the last error's
/// code (`null` when running or complete).
fn overlay(value: &mut serde_json::Value, live: Option<Live>) {
    match live {
        None => {}
        Some(Live::Running) => {
            value["state"] = "running".into();
            value["last_error"] = serde_json::Value::Null;
        }
        Some(Live::Paused) => {
            value["state"] = "paused".into();
            value["last_error"] = serde_json::json!({"code": "paused", "message": PAUSED});
        }
    }
    value["reason"] = value["last_error"]["code"].clone();
}
