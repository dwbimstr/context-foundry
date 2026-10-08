//! 009 T003: progressive preparation inside the MCP owner (`index
//! {semantic: "prepare" | "pause"}`).
//!
//! ONE background driver thread per owner, for the PRIMARY root only. It
//! composes the CLI's own preparation steps ([`super::prepare`]): partition a
//! few sources, publish pending committed coverage, select one batch of at
//! most the profile's batch of missing cards, render it, embed it, validate
//! and commit it, publish. Only the ownership around the steps differs:
//!
//! - every store step takes the owner's ONE engine slot, and only while no
//!   foreground operation is in flight, and gives it back before the next
//!   step; a foreground operation arriving during such a step waits for it
//!   (the owner's `take_engine_slot`), so steps are kept short: a partition
//!   step and each step of a publication end at their first boundary after
//!   [`STEP_TIME`], and the generation itself is built with the slot free;
//! - the model call runs on the owner's ONE resident worker
//!   ([`QueryRuntime`]) with NO engine slot and NO transaction held. Its
//!   admission is refused, never queued, while that slot is occupied or a
//!   foreground query is being dispatched, and the refusal pauses
//!   preparation with `provider_busy`;
//! - while the owner served a foreground operation in the last
//!   [`FOREGROUND_WINDOW`] ([`Preparation::foreground`]), a selected batch
//!   is embedded at most [`FOREGROUND_BATCH`] inputs per call, so a query
//!   that finds the model busy with a batch waits for a short one
//!   ([`QueryRuntime::embed`]);
//! - 009 T004, one prefetched batch: while one document call runs, the
//!   driver selects the next batch in a store step under the same slot rule
//!   and renders and tokenizes it with no engine slot and no transaction. It
//!   is admitted afresh after the call ended and was committed, under the
//!   rules above: a foreground operation that arrived first holds the
//!   commit's store step until it ends, so it goes first; a pause, owner
//!   shutdown or EOF drops the prefetched batch unsent.
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
use crate::neural::index::{
    self, GenerationError, MappingWalk, Publication, validate_generation_with,
};
use crate::neural::partition;
use crate::neural::prepare::{
    self, Batch, Cards, PrepareReport, Progress, Selected, Selection, Steps, Stop, StopOrError,
    Walk,
};
use crate::neural::provider::{ProviderError, TokenizedInput};
use crate::neural::query::QueryRuntime;
use crate::store::Engine;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// Each document call's own deadline. The supervised worker then grants its
/// in-flight grace before stopping the worker; the stop is the resumable
/// `provider_timeout`.
pub const DOCUMENT_CALL_TIMEOUT: Duration = Duration::from_secs(60);
/// New partitions one store step may write.
const PARTITIONS_PER_STEP: usize = 8;
/// Sources one selection step may examine.
const SOURCES_PER_STEP: usize = cache::PAGE;
/// How long the driver waits before looking again for a free engine slot,
/// and the slice of its wait on a running document call.
const POLL: Duration = Duration::from_millis(10);
/// The largest document batch admitted while the owner serves foreground
/// operations (009 T003, captain decision 2026-10-06).
pub const FOREGROUND_BATCH: usize = 2;
/// How long after a foreground operation batches stay at
/// [`FOREGROUND_BATCH`]; afterwards they are the profile's batch again.
pub const FOREGROUND_WINDOW: Duration = Duration::from_secs(60);
/// The engine-slot time one partition or publication step aims at (009
/// T003, captain decision 2026-10-06): the step ends at its first boundary
/// after it (a source, a page of sources, a cached vector), so a foreground
/// operation that finds the slot held by the driver waits about this long.
const STEP_TIME: Duration = Duration::from_millis(100);

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
    /// Test seam: while a document call runs, the driver rendered the next
    /// batch (`inputs` cards buffered, none of them sent).
    #[cfg(feature = "test-faults")]
    fn prefetched(&self, _inputs: usize) {}
    /// Test seam: the driver is inside a publication's staging work, its
    /// index built in memory and not yet serialized, with no engine slot held.
    #[cfg(feature = "test-faults")]
    fn staging(&self) {}
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
/// take only this state. The foreground mark has its own lock, held for
/// nothing else.
pub struct Preparation {
    state: Mutex<State>,
    /// When the owner last started a foreground operation.
    foreground: Mutex<Option<Instant>>,
}

impl Default for Preparation {
    fn default() -> Self {
        Self {
            state: Mutex::new(State {
                phase: Phase::Idle,
                in_call: false,
            }),
            foreground: Mutex::new(None),
        }
    }
}

impl Preparation {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 009 T003: the owner calls this for every MCP request it serves
    /// (search, context, retrieve, index, status, memory, references). For
    /// the next [`FOREGROUND_WINDOW`] the driver embeds at most
    /// [`FOREGROUND_BATCH`] inputs per document call.
    pub fn foreground(&self) {
        *self
            .foreground
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
    }

    /// The document batch size to admit at `now` for a profile batch of
    /// `full`.
    fn batch_limit(&self, now: Instant, full: usize) -> usize {
        let last = *self
            .foreground
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        batch_limit(last, now, full)
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

/// [`FOREGROUND_BATCH`] (at most `full`) while the last foreground operation
/// started less than [`FOREGROUND_WINDOW`] before `now`, else `full`, the
/// profile's batch.
fn batch_limit(last_foreground: Option<Instant>, now: Instant, full: usize) -> usize {
    match last_foreground {
        Some(at) if now.saturating_duration_since(at) < FOREGROUND_WINDOW => {
            FOREGROUND_BATCH.min(full)
        }
        _ => full,
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

/// Why the driver's publication did not finish: the owner can never run a
/// store step again, or a step failed (recorded as the publication's named
/// stop).
enum Failed {
    Owner(FoundryError),
    Step(FoundryError),
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
        let recipe = partition::recipe_id(&profile.descriptor.tokenizer, profile.card_tokens);
        Self {
            owner,
            runtime,
            preparation,
            control,
            report: PrepareReport {
                profile: profile.name.clone(),
                function_digest: digest.clone(),
                recipe_id: recipe.clone(),
                dimensions: profile.descriptor.dimensions,
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
                    cards: Cards {
                        tokenizer: runtime.tokenizer(),
                        profile: &runtime.profile,
                        function_digest: digest,
                    },
                    recipe,
                }
                .partition_page(
                    &mut after,
                    PARTITIONS_PER_STEP,
                    Some(Instant::now() + STEP_TIME),
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

    /// Select batches of up to the profile's batch of cards and embed them.
    /// A batch admitted smaller (foreground activity) keeps its remaining
    /// inputs, in walk order, for the next admission; the walk resumes only
    /// once fewer inputs remain than the current batch size. While a call
    /// runs, the next batch is selected and rendered ([`Self::embed`]).
    fn embed_pass(&mut self) -> FResult<Option<Stop>> {
        let full = self.runtime.profile.document_limits().inputs;
        let mut walk = Walk::default();
        let mut batch = Batch::default();
        let mut end = false;
        loop {
            if let Some(stop) = self.requested_stop() {
                return Ok(Some(stop));
            }
            if !end && batch.len() < self.preparation.batch_limit(Instant::now(), full) {
                let room = full - batch.len();
                match self.fill(&mut walk, &mut batch, room)? {
                    Selected::Full => {}
                    Selected::End => end = true,
                    Selected::Yield => continue,
                    Selected::Halted(stop) => return Ok(Some(stop)),
                }
            }
            if !batch.is_empty()
                && let Some(stop) = self.embed(&mut walk, &mut batch, &mut end, full)?
            {
                return Ok(Some(stop));
            }
            if end && batch.is_empty() {
                return Ok(None);
            }
        }
    }

    /// Up to `room` more cards into `batch`: one selection step under the
    /// engine slot (the missing cards and their sources' bodies), then their
    /// rendering and tokenization with NO engine slot and no transaction.
    fn fill(&mut self, walk: &mut Walk, batch: &mut Batch, room: usize) -> FResult<Selected> {
        let (owner, runtime) = (self.owner, self.runtime);
        let (recipe, report) = (&self.recipe, &mut self.report);
        let cards = Cards {
            tokenizer: runtime.tokenizer(),
            profile: &runtime.profile,
            function_digest: &self.digest,
        };
        let mut selection = Selection::default();
        let selected = hold(owner, |engine| {
            Steps {
                engine,
                cards,
                recipe,
            }
            .select_batch(
                walk,
                &mut selection,
                room,
                SOURCES_PER_STEP,
                report,
                &mut || owner.closing().then(cancelled),
            )
        })?;
        if selection.len() > 0 {
            let rendered = cards.render(selection)?;
            batch.keys.extend(rendered.keys);
            batch.inputs.extend(rendered.inputs);
        }
        Ok(selected)
    }

    /// One admission: the first inputs of `batch`, as many as the batch size
    /// allows at the admission decision; the rest stay in `batch`. The final
    /// stop check, the size, the admission on the runtime slot and the
    /// in-call mark are ONE decision under the preparation-state lock that
    /// `pause` takes, released before any wait on inference; the call runs
    /// with no engine slot or transaction held; validation and commit run
    /// under the slot.
    ///
    /// 009 T004: while the call runs, at most one next batch is prefetched
    /// into `batch` ([`Self::fill`]): its store step follows the slot rule
    /// and its rendering holds nothing. It is never sent here; the next
    /// admission decides it afresh, after this call ended and was
    /// committed. A failed prefetch is reported only after that commit.
    fn embed(
        &mut self,
        walk: &mut Walk,
        batch: &mut Batch,
        end: &mut bool,
        full: usize,
    ) -> FResult<Option<Stop>> {
        #[cfg(feature = "test-faults")]
        self.owner.admitting();
        let preparation = self.preparation;
        let (keys, tokens, call) = {
            let mut state = preparation.lock();
            if self.owner.closing() {
                self.control.cancel();
                return Ok(Some(cancelled()));
            }
            if state.phase == Phase::Pausing {
                return Ok(Some(paused()));
            }
            let size = batch
                .len()
                .min(preparation.batch_limit(Instant::now(), full));
            let keys: Vec<String> = batch.keys.drain(..size).collect();
            let inputs: Vec<TokenizedInput> = batch.inputs.drain(..size).collect();
            let tokens: u64 = inputs.iter().map(|input| input.ids.len() as u64).sum();
            let job = self
                .control
                .bounded_by(Instant::now() + DOCUMENT_CALL_TIMEOUT);
            match self.runtime.dispatch_documents(inputs, job) {
                Ok(call) => {
                    state.in_call = true;
                    (keys, tokens, call)
                }
                Err(error) => {
                    drop(state);
                    return Ok(Some(self.failed(error)));
                }
            }
        };
        self.report.document_calls += 1;
        self.report.input_tokens += tokens;
        // The prefetch: a halt (owner shutdown) is caught by the wait below.
        let mut prefetch_error = None;
        if !*end && batch.len() < full {
            match self.fill(walk, batch, full - batch.len()) {
                Ok(Selected::End) => *end = true,
                Ok(_) => {}
                Err(error) => prefetch_error = Some(error),
            }
            #[cfg(feature = "test-faults")]
            self.owner.prefetched(batch.len());
        }
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
        let dims = self.runtime.profile.descriptor.dims();
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
                dims,
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
        if let Some(error) = prefetch_error {
            return Err(error);
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
        let outcome = match self.generation(rebuild) {
            Ok(publication) => Ok(publication),
            Err(Failed::Step(error)) => Err(error),
            Err(Failed::Owner(error)) => return Err(error),
        };
        let stop = prepare::record_publication(outcome, 0, &mut self.report);
        if stop.is_none() {
            if self.report.index_published {
                self.runtime.forget_index();
                self.published = self.report.index_entries;
            }
            self.unpublished = 0;
        }
        Ok(stop)
    }

    /// The publication itself, the CLI's `Engine::semantic_rebuild_index` /
    /// `semantic_publish_pending` in short steps (009 T003, captain decision
    /// 2026-10-06): the mapping a page of sources per step and the cached
    /// vectors a chunk per step, each step ending at its first boundary after
    /// [`STEP_TIME`]; the existing generation is validated and the new one
    /// built and staged with the engine slot free; the slot is taken again
    /// only to publish the staged set. Sources edited between the steps make
    /// the mapping incomplete, never wrong: locations revalidate per request.
    fn generation(&self, rebuild: bool) -> Result<Publication, Failed> {
        let control = self.control;
        let Some((identity, store, mut walk)) = self.step(|engine| {
            let Some(identity) = engine.semantic_identity()? else {
                return Ok(None);
            };
            let walk = MappingWalk::start(engine, &identity.digest, &identity.recipe)?;
            Ok(Some((identity, engine.semantic_anchor()?, walk)))
        })?
        else {
            return Ok(Publication::Nothing);
        };
        while !self.step(|engine| walk.step(engine, control, Some(Instant::now() + STEP_TIME)))? {}
        let mapping = self.step(|engine| walk.finish(engine))?;
        if !rebuild {
            if mapping.units.is_empty() {
                return Ok(Publication::Nothing);
            }
            // Current means the same keys, unit locations, coverage AND
            // source revision.
            match validate_generation_with(
                &store,
                &identity.digest,
                &identity.recipe,
                identity.dimensions,
                control,
            ) {
                Ok(generation) if mapping.published_by(&generation) => {
                    return Ok(Publication::Current(generation.manifest.count));
                }
                Err(GenerationError::Interrupted(error)) => return Err(Failed::Step(error)),
                _ => {}
            }
        }
        let mut scope = mapping.scope;
        let mut keys = mapping.units.into_iter();
        let mut entries = Vec::with_capacity(keys.len());
        while !self.step(|engine| {
            index::lookup_entries(
                engine,
                &mut keys,
                &identity,
                &mut scope,
                &mut entries,
                control,
                Some(Instant::now() + STEP_TIME),
            )
        })? {}
        #[cfg(feature = "test-faults")]
        let built = || self.owner.staging();
        #[cfg(not(feature = "test-faults"))]
        let built = || {};
        let staged = index::stage_generation(&store, &identity, &entries, scope, control, &built)
            .map_err(Failed::Step)?;
        drop(entries);
        let count = self.step(|_| staged.publish(control))?;
        Ok(Publication::Rebuilt(count))
    }

    /// One short store step of a publication under the engine slot. A run
    /// whose owner is shutting down publishes nothing more: its control is
    /// cancelled before the step.
    fn step<T>(&self, step: impl FnOnce(&Engine) -> FResult<T>) -> Result<T, Failed> {
        let (owner, control) = (self.owner, self.control);
        hold(owner, |engine| {
            if owner.closing() {
                control.cancel();
            }
            Ok(step(engine))
        })
        .map_err(Failed::Owner)?
        .map_err(Failed::Step)
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

#[cfg(all(test, feature = "test-faults"))]
mod tests {
    //! 009 T003, captain decision 2026-10-06: document batches are at most
    //! [`FOREGROUND_BATCH`] inputs while the owner serves foreground
    //! operations and the profile's batch otherwise. 009 T004: one batch is
    //! prefetched while a call runs, admitted afresh after it, and dropped
    //! on pause and owner shutdown.
    use super::*;
    use crate::neural::profile::{DEFAULT_BATCH, SemanticProfile};
    use crate::neural::provider::{EmbeddingProvider, FunctionDescriptor};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::mpsc;

    /// An owner of one engine with no foreground operation in flight.
    struct Quiet(Mutex<Engine>);

    impl Owner for Quiet {
        fn try_primary(&self, step: &mut dyn FnMut(&Engine)) -> FResult<bool> {
            step(&self.0.lock().unwrap_or_else(PoisonError::into_inner));
            Ok(true)
        }
        fn closing(&self) -> bool {
            false
        }
    }

    fn unit_vector(descriptor: &FunctionDescriptor) -> Vec<f32> {
        let mut vector = vec![0f32; descriptor.dims()];
        vector[0] = 1.0;
        vector
    }

    /// Records the size of every document batch. With `foreground`, every
    /// call also marks a foreground operation, as a request the owner served
    /// meanwhile would, so the window never lapses during the run.
    struct Sizes {
        descriptor: FunctionDescriptor,
        sizes: Arc<Mutex<Vec<usize>>>,
        foreground: Option<Arc<Preparation>>,
    }

    impl EmbeddingProvider for Sizes {
        fn descriptor(&self) -> &FunctionDescriptor {
            &self.descriptor
        }
        fn embed_documents(
            &mut self,
            batch: &[TokenizedInput],
            _control: &Control,
        ) -> Result<Vec<Vec<f32>>, ProviderError> {
            self.sizes.lock().unwrap().push(batch.len());
            if let Some(preparation) = &self.foreground {
                preparation.foreground();
            }
            Ok(vec![unit_vector(&self.descriptor); batch.len()])
        }
        fn embed_query(
            &mut self,
            _input: &TokenizedInput,
            _deadline: Instant,
        ) -> Result<Vec<f32>, ProviderError> {
            unreachable!("preparation embeds no query")
        }
    }

    /// A store of twelve notes, one card each, and its fixture profile.
    fn notes() -> (tempfile::TempDir, Engine, Arc<SemanticProfile>) {
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
        let path = crate::testkit::write_semantic_profile(dir.path(), "notes", |_| {});
        let profile = Arc::new(SemanticProfile::load(&path).unwrap());
        (dir, engine, profile)
    }

    fn until_idle(preparation: &Preparation) {
        let deadline = Instant::now() + Duration::from_secs(60);
        while !preparation.idle() {
            assert!(Instant::now() < deadline, "the driver never stopped");
            std::thread::sleep(POLL);
        }
    }

    /// Prepare twelve single-card notes to completion; the sizes of the
    /// document batches, in order.
    fn batch_sizes(foreground: bool) -> Vec<usize> {
        let (_dir, engine, profile) = notes();
        let preparation = Arc::new(Preparation::default());
        let sizes = Arc::new(Mutex::new(Vec::new()));
        let provider = Sizes {
            descriptor: profile.descriptor.clone(),
            sizes: Arc::clone(&sizes),
            foreground: foreground.then(|| Arc::clone(&preparation)),
        };
        let runtime = Arc::new(
            QueryRuntime::start(
                profile,
                Box::new(move || Ok(Box::new(provider) as Box<dyn EmbeddingProvider>)),
            )
            .unwrap(),
        );
        if foreground {
            preparation.foreground();
        }
        let owner = Arc::new(Quiet(Mutex::new(engine)));
        preparation
            .prepare(Arc::clone(&owner) as Arc<dyn Owner>, Arc::clone(&runtime))
            .unwrap();
        until_idle(&preparation);
        runtime.shutdown();
        let state = owner.0.lock().unwrap().semantic_state().unwrap().unwrap();
        assert_eq!(state.state, "stopped", "{state:?}");
        assert!(state.last_error.is_none(), "{state:?}");
        assert_eq!(state.committed_units, 12, "{state:?}");
        sizes.lock().unwrap().clone()
    }

    #[test]
    fn document_batches_are_small_while_the_owner_serves_foreground_operations() {
        assert_eq!(batch_sizes(false), [8, 4], "no foreground operation");
        assert_eq!(
            batch_sizes(true),
            [FOREGROUND_BATCH; 6],
            "a foreground operation within the window"
        );
    }

    #[test]
    fn the_foreground_window_ends_sixty_seconds_after_the_last_operation() {
        let at = Instant::now();
        let full = DEFAULT_BATCH as usize;
        assert_eq!(batch_limit(None, at, full), full);
        assert_eq!(batch_limit(Some(at), at, full), FOREGROUND_BATCH);
        assert_eq!(
            batch_limit(Some(at), at + Duration::from_secs(59), full),
            FOREGROUND_BATCH
        );
        assert_eq!(batch_limit(Some(at), at + FOREGROUND_WINDOW, full), full);
        // A mark taken after the admission read the clock is recent.
        assert_eq!(
            batch_limit(Some(at + Duration::from_secs(1)), at, full),
            FOREGROUND_BATCH
        );
        // A profile batch below the foreground batch is never exceeded.
        assert_eq!(batch_limit(Some(at), at, 1), 1);
    }

    /// An owner whose foreground operations are counted: while one is in
    /// flight no store step runs (the MCP owner's rule). It reports the
    /// driver's prefetch and can be closed.
    struct Counted {
        engine: Mutex<Engine>,
        in_flight: AtomicUsize,
        closing: AtomicBool,
        prefetched: Mutex<mpsc::Sender<usize>>,
    }

    impl Owner for Counted {
        fn try_primary(&self, step: &mut dyn FnMut(&Engine)) -> FResult<bool> {
            if self.in_flight.load(Ordering::SeqCst) > 0 {
                return Ok(false);
            }
            step(&self.engine.lock().unwrap_or_else(PoisonError::into_inner));
            Ok(true)
        }
        fn closing(&self) -> bool {
            self.closing.load(Ordering::SeqCst)
        }
        fn prefetched(&self, inputs: usize) {
            let _ = locked(&self.prefetched).send(inputs);
        }
    }

    /// The crate's lock convention: a poisoned lock still yields its guard.
    fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Every document call is logged, reports that it entered and waits for
    /// one release; queries are logged and answered at once.
    struct Gated {
        descriptor: FunctionDescriptor,
        calls: Arc<Mutex<Vec<String>>>,
        entered: mpsc::Sender<usize>,
        release: mpsc::Receiver<()>,
    }

    impl EmbeddingProvider for Gated {
        fn descriptor(&self) -> &FunctionDescriptor {
            &self.descriptor
        }
        fn embed_documents(
            &mut self,
            batch: &[TokenizedInput],
            _control: &Control,
        ) -> Result<Vec<Vec<f32>>, ProviderError> {
            locked(&self.calls).push(format!("documents {}", batch.len()));
            let _ = self.entered.send(batch.len());
            let _ = self.release.recv();
            Ok(vec![unit_vector(&self.descriptor); batch.len()])
        }
        fn embed_query(
            &mut self,
            _input: &TokenizedInput,
            _deadline: Instant,
        ) -> Result<Vec<f32>, ProviderError> {
            locked(&self.calls).push("query".into());
            Ok(unit_vector(&self.descriptor))
        }
    }

    struct Prefetch {
        _dir: tempfile::TempDir,
        owner: Arc<Counted>,
        runtime: Arc<QueryRuntime>,
        preparation: Arc<Preparation>,
        calls: Arc<Mutex<Vec<String>>>,
        entered: mpsc::Receiver<usize>,
        release: mpsc::Sender<()>,
        prefetched: mpsc::Receiver<usize>,
    }

    impl Prefetch {
        /// Start the driver over twelve notes and wait until its first
        /// batch of 8 runs and the remaining 4 cards were rendered beside it.
        fn started() -> Self {
            let (dir, engine, profile) = notes();
            let (entered_tx, entered) = mpsc::channel();
            let (release, release_rx) = mpsc::channel();
            let (prefetched_tx, prefetched) = mpsc::channel();
            let calls = Arc::new(Mutex::new(Vec::new()));
            let provider = Gated {
                descriptor: profile.descriptor.clone(),
                calls: Arc::clone(&calls),
                entered: entered_tx,
                release: release_rx,
            };
            let runtime = Arc::new(
                QueryRuntime::start(
                    profile,
                    Box::new(move || Ok(Box::new(provider) as Box<dyn EmbeddingProvider>)),
                )
                .unwrap(),
            );
            let owner = Arc::new(Counted {
                engine: Mutex::new(engine),
                in_flight: AtomicUsize::new(0),
                closing: AtomicBool::new(false),
                prefetched: Mutex::new(prefetched_tx),
            });
            let preparation = Arc::new(Preparation::default());
            preparation
                .prepare(Arc::clone(&owner) as Arc<dyn Owner>, Arc::clone(&runtime))
                .unwrap();
            let fixture = Self {
                _dir: dir,
                owner,
                runtime,
                preparation,
                calls,
                entered,
                release,
                prefetched,
            };
            assert_eq!(fixture.entered(), 8, "the first batch runs");
            assert_eq!(
                fixture
                    .prefetched
                    .recv_timeout(Duration::from_secs(30))
                    .expect("the next batch was prefetched during the call"),
                4
            );
            assert_eq!(fixture.calls(), ["documents 8"], "nothing more was sent");
            fixture
        }

        fn entered(&self) -> usize {
            self.entered
                .recv_timeout(Duration::from_secs(30))
                .expect("a document call entered")
        }

        fn calls(&self) -> Vec<String> {
            locked(&self.calls).clone()
        }

        fn state(&self) -> SemanticState {
            locked(&self.owner.engine)
                .semantic_state()
                .unwrap()
                .unwrap()
        }
    }

    /// A foreground query that arrived while the first batch ran gets the
    /// model before the prefetched batch is admitted.
    #[test]
    fn a_prefetched_batch_is_admitted_only_after_a_foreground_query_that_arrived_first() {
        let fixture = Prefetch::started();
        fixture.owner.in_flight.fetch_add(1, Ordering::SeqCst);
        let query = {
            let (runtime, owner) = (Arc::clone(&fixture.runtime), Arc::clone(&fixture.owner));
            std::thread::spawn(move || {
                let embedded = runtime.embed("dusk", Instant::now() + Duration::from_secs(30));
                owner.in_flight.fetch_sub(1, Ordering::SeqCst);
                embedded
            })
        };
        fixture.release.send(()).unwrap();
        assert!(query.join().unwrap().is_ok(), "the query embedded");
        assert_eq!(
            fixture.entered(),
            4,
            "the prefetched batch, admitted afterwards"
        );
        fixture.release.send(()).unwrap();
        until_idle(&fixture.preparation);
        fixture.runtime.shutdown();
        assert_eq!(fixture.calls(), ["documents 8", "query", "documents 4"]);
        let state = fixture.state();
        assert_eq!(state.state, "stopped", "{state:?}");
        assert_eq!(state.committed_units, 12);
    }

    /// A pause lets the in-flight batch commit and drops the prefetched one
    /// unsent.
    #[test]
    fn a_pause_drops_the_prefetched_batch() {
        let fixture = Prefetch::started();
        fixture.preparation.pause();
        fixture.release.send(()).unwrap();
        until_idle(&fixture.preparation);
        fixture.runtime.shutdown();
        assert_eq!(fixture.calls(), ["documents 8"]);
        let state = fixture.state();
        assert_eq!(state.state, "paused", "{state:?}");
        assert_eq!(state.committed_units, 8, "the in-flight batch committed");
    }

    /// Owner shutdown or EOF discards the in-flight batch and drops the
    /// prefetched one unsent.
    #[test]
    fn owner_shutdown_drops_the_in_flight_and_the_prefetched_batch() {
        let fixture = Prefetch::started();
        fixture.owner.closing.store(true, Ordering::SeqCst);
        fixture.release.send(()).unwrap();
        until_idle(&fixture.preparation);
        fixture.runtime.shutdown();
        assert_eq!(fixture.calls(), ["documents 8"]);
        let state = fixture.state();
        assert_eq!(state.committed_units, 0, "nothing uncommitted survives");
        assert_eq!(
            state.last_error.map(|error| error.code).as_deref(),
            Some("cancelled")
        );
    }
}
