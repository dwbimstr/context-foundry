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
//!   (the owner's `take_engine_slot`), so steps are kept short: partition
//!   input collection and publication end at their first boundary after
//!   [`STEP_TIME`]. Partition CPU work and generation construction run with
//!   the slot free; computed partitions recheck their source at commit, and
//!   one whose source changed or was deleted meanwhile is dropped with its
//!   cards while the newer version stays pending. An overview area whose
//!   brief or sources changed before it is bound is skipped the same way;
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
//! Continuous maintenance is explicit (`maintain` / `--semantic-maintain`). It
//! consumes durable coalesced source obligations through this same driver and waits
//! on source notifications when caught up. Ordinary startup and watching alone do
//! not prepare. Provider/resource failures pause with pending work retained; retries
//! require an explicit action. No extra job service or model owner is introduced.
use crate::control::Control;
use crate::error::{FResult, FoundryError};
use crate::neural::cache::{self, SemanticState};
use crate::neural::index::{self, GenerationError, MappingWalk, Publication, validate_cached};
use crate::neural::prepare::{
    self, Batch, Cards, PrepareReport, Progress, Selected, Selection, Steps, Stop, StopOrError,
    Walk,
};
use crate::neural::provider::ProviderError;
#[cfg(test)]
use crate::neural::provider::TokenizedInput;
use crate::neural::query::QueryRuntime;
use crate::store::Engine;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// Each document call's own deadline. The supervised worker then grants its
/// in-flight grace before stopping the worker; the stop is the resumable
/// `provider_timeout`.
pub const DOCUMENT_CALL_TIMEOUT: Duration = Duration::from_secs(60);
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
    /// Shared background-call admission. A busy knowledge request defers the
    /// selected embedding batch intact instead of pausing semantic maintenance.
    fn try_model(&self) -> Option<crate::neural::work::ModelPermit> {
        Some(crate::neural::work::ModelPermit::unshared())
    }
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
    priority_paths: Vec<String>,
    priority_revision: u64,
    bootstrap: crate::neural::cache::BootstrapProgress,
    continuous: bool,
    waiting: bool,
}

/// The live state of this owner's driver, overlaid on the committed row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Live {
    Running,
    Waiting,
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
    /// CPU-only pool is lazy and survives explicit pause/resume runs.
    workers: prepare::PartitionWorkers,
}

impl Default for Preparation {
    fn default() -> Self {
        Self {
            state: Mutex::new(State {
                phase: Phase::Idle,
                in_call: false,
                priority_paths: Vec::new(),
                priority_revision: 0,
                bootstrap: Default::default(),
                continuous: false,
                waiting: false,
            }),
            foreground: Mutex::new(None),
            workers: prepare::PartitionWorkers::default(),
        }
    }
}

impl Preparation {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The owner calls this for foreground operations (status observation is
    /// excluded). For
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
        self.prepare_with_priorities(owner, runtime, None)
    }

    /// A new explicit list replaces scheduling preferences at the next wave;
    /// ordinary work always retains its alternating turn.
    pub fn prepare_with_priorities(
        self: &Arc<Self>,
        owner: Arc<dyn Owner>,
        runtime: Arc<QueryRuntime>,
        paths: Option<Vec<String>>,
    ) -> FResult<()> {
        self.start(owner, runtime, paths, false)
    }

    pub fn maintain(
        self: &Arc<Self>,
        owner: Arc<dyn Owner>,
        runtime: Arc<QueryRuntime>,
        paths: Option<Vec<String>>,
    ) -> FResult<()> {
        self.start(owner, runtime, paths, true)
    }

    fn start(
        self: &Arc<Self>,
        owner: Arc<dyn Owner>,
        runtime: Arc<QueryRuntime>,
        paths: Option<Vec<String>>,
        continuous: bool,
    ) -> FResult<()> {
        if let Some(paths) = &paths {
            prepare::validate_priority_paths(paths)?;
        }
        let busy = || FoundryError::Semantic {
            code: "provider_busy",
            message: "an earlier model call still occupies the runtime slot; nothing was \
                      started or queued; retry after it ends"
                .into(),
        };
        let mut state = self.lock();
        if state.phase != Phase::Running
            && (runtime.occupied() && !runtime.knowledge_occupied())
            && !state.in_call
        {
            return Err(busy());
        }
        if let Some(paths) = paths {
            state.priority_paths = paths;
            state.priority_revision = state.priority_revision.wrapping_add(1);
        }
        if state.continuous != continuous {
            state.waiting = false;
        }
        state.continuous = continuous;
        match state.phase {
            Phase::Running => Ok(()),
            Phase::Pausing | Phase::Stopping => {
                if (runtime.occupied() && !runtime.knowledge_occupied()) && !state.in_call {
                    return Err(busy());
                }
                state.phase = Phase::Running;
                Ok(())
            }
            Phase::Idle => {
                if runtime.occupied() && !runtime.knowledge_occupied() {
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
        state.continuous = false;
        state.waiting = false;
        if matches!(state.phase, Phase::Running | Phase::Stopping) {
            state.phase = Phase::Pausing;
        }
    }

    /// The live state while a driver thread exists.
    pub fn live(&self) -> Option<Live> {
        let state = self.lock();
        match state.phase {
            Phase::Idle => None,
            Phase::Running if state.waiting => Some(Live::Waiting),
            Phase::Pausing => Some(Live::Paused),
            Phase::Running | Phase::Stopping => Some(Live::Running),
        }
    }

    /// No driver thread exists (none started, or it recorded its stop).
    pub fn idle(&self) -> bool {
        self.lock().phase == Phase::Idle
    }

    pub fn progress(&self) -> Option<crate::neural::cache::BootstrapProgress> {
        let state = self.lock();
        (state.phase != Phase::Idle).then(|| state.bootstrap.clone())
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

/// Bounded scheduling state; accepted work lives in partitions and cache.
struct Frontier {
    order: prepare::SourceOrder,
    walk: Option<Walk>,
    paused: prepare::PausedSources,
    active_cursor: Option<prepare::SourceCursor>,
    wave_inputs: usize,
    priority_revision: Option<u64>,
    changed_only: bool,
}

impl Frontier {
    fn yield_wave(&mut self) {
        let walk = self.walk.take().expect("active wave");
        let mut cursor = self.active_cursor.take().expect("active wave cursor");
        cursor.resume = Some(walk.pause(&mut self.paused));
        self.order.advance(cursor, false);
    }
}

/// One run of the driver: identity, accounting and publication bookkeeping.
struct Run<'a> {
    started: Instant,
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
    publish_overview_boundary: bool,
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
        let recipe = profile.recipe_id();
        Self {
            owner,
            runtime,
            preparation,
            started: Instant::now(),
            control,
            report: PrepareReport {
                profile: profile.name.clone(),
                function_digest: digest.clone(),
                recipe_id: recipe.clone(),
                dimensions: profile.descriptor.dimensions,
                cache_cap_bytes: runtime.cache_cap_bytes,
                // The resident runtime is up; a failed call says otherwise.
                provider_state: Some("ready"),
                ..PrepareReport::default()
            },
            recipe,
            digest,
            unpublished: 0,
            published: 0,
            publish_overview_boundary: false,
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
        self.sync_progress();
        let report = &self.report;
        hold(self.owner, |engine| prepare::begin(engine, report))
    }

    fn sync_progress(&mut self) {
        self.preparation.lock().bootstrap =
            prepare::snapshot_progress(&mut self.report, self.started);
    }

    /// Passes until one saw an unchanged source revision, or a stop.
    fn passes(&mut self) -> FResult<Stop> {
        loop {
            if self.preparation.lock().continuous {
                if let Some(stop) = self.maintain()? {
                    return Ok(stop);
                }
                continue;
            }
            let stop = self.one_shot()?;
            if matches!(stop, Stop::Complete) && self.preparation.lock().continuous {
                continue;
            }
            return Ok(stop);
        }
    }

    fn one_shot(&mut self) -> FResult<Stop> {
        let work = hold(self.owner, |engine| {
            engine.derivation_reconcile(
                crate::neural::work::Lane::Semantic,
                &format!("{}:{}", self.digest, self.recipe),
            )
        })?;
        // Recover prior committed coverage before doing more provider work.
        if let Some(stop) = self.publish()? {
            return Ok(stop);
        }
        loop {
            let revision = hold(self.owner, |engine| engine.source_revision())?;
            let mut frontier = Frontier {
                order: prepare::SourceOrder::new(&[])?
                    .for_representation(self.runtime.profile.representation),
                walk: None,
                paused: prepare::PausedSources::default(),
                active_cursor: None,
                wave_inputs: 0,
                priority_revision: None,
                changed_only: false,
            };
            if let Some(stop) = self.embed_pass(&mut frontier)? {
                return Ok(stop);
            }
            if let Some(stop) = self.area_pass()? {
                return Ok(stop);
            }
            // Also publish new locations/complete coverage when every card was
            // reused: cache hits do not increment the new-vector counter.
            if let Some(stop) = self.publish()? {
                return Ok(stop);
            }
            if hold(self.owner, |engine| engine.source_revision())? == revision {
                hold(self.owner, |engine| {
                    engine.derivation_ack(crate::neural::work::Lane::Semantic, &work)
                })?;
                return Ok(Stop::Complete);
            }
        }
    }

    /// `None` hands control back to an explicitly requested one-shot pass.
    fn maintain(&mut self) -> FResult<Option<Stop>> {
        use crate::neural::work::Lane;
        let identity = format!("{}:{}", self.digest, self.recipe);
        let wake = hold(self.owner, |engine| Ok(Arc::clone(&engine.semantic_wake)))?;
        loop {
            if let Some(stop) = self.requested_stop() {
                return Ok(Some(stop));
            }
            let stamp = wake.stamp();
            let work = hold(self.owner, |engine| {
                engine.derivation_batch(Lane::Semantic, &identity)
            })?;
            if !work.requires_reconcile() && work.paths().is_empty() && !work.overviews_changed() {
                self.sync_progress();
                hold(self.owner, |engine| {
                    prepare::finalize(engine, &mut self.report, &Ok(Stop::Complete))
                })?;
                self.preparation.lock().waiting = true;
                loop {
                    if let Some(stop) = self.requested_stop() {
                        return Ok(Some(stop));
                    }
                    if !self.preparation.lock().continuous {
                        self.preparation.lock().waiting = false;
                        return Ok(None);
                    }
                    if wake.wait(stamp) {
                        break;
                    }
                }
                self.preparation.lock().waiting = false;
                hold(self.owner, |engine| prepare::begin(engine, &self.report))?;
                continue;
            }
            let mut frontier = Frontier {
                order: (if work.requires_reconcile() {
                    prepare::SourceOrder::new(&[])?
                } else {
                    prepare::SourceOrder::changed(work.paths().iter().map(|(path, _)| path.clone()))
                })
                .for_representation(self.runtime.profile.representation),
                walk: None,
                paused: prepare::PausedSources::default(),
                active_cursor: None,
                wave_inputs: 0,
                priority_revision: None,
                changed_only: !work.requires_reconcile(),
            };
            if let Some(stop) = self.embed_pass(&mut frontier)? {
                return Ok(Some(stop));
            }
            if let Some(stop) = self.area_pass()? {
                return Ok(Some(stop));
            }
            if let Some(stop) = self.publish()? {
                return Ok(Some(stop));
            }
            hold(self.owner, |engine| {
                engine.derivation_ack(Lane::Semantic, &work)
            })?;
            if !self.preparation.lock().continuous {
                self.preparation.lock().waiting = false;
                return Ok(None);
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

    fn partition_wave(
        &mut self,
        cursor: &mut prepare::SourceCursor,
    ) -> FResult<(Progress, Vec<prepare::PreparedCards>)> {
        if let Some(stop) = self.requested_stop() {
            return Ok((Progress::Halted(stop), Vec::new()));
        }
        let (owner, runtime, control) = (self.owner, self.runtime, self.control);
        let (recipe, digest, report) = (&self.recipe, &self.digest, &mut self.report);
        let (progress, inputs) = hold(owner, |engine| {
            Steps {
                engine,
                cards: Cards {
                    tokenizer: runtime.tokenizer(),
                    profile: &runtime.profile,
                    function_digest: digest,
                },
                recipe,
            }
            .partition_inputs_in(
                &mut cursor.after,
                cursor.scope.as_deref(),
                cursor.exact,
                prepare::PARTITION_WAVE,
                Some(Instant::now() + STEP_TIME),
                report,
                &mut || owner.closing().then(cancelled),
            )
        })?;
        if matches!(progress, Progress::Halted(_)) {
            return Ok((progress, Vec::new()));
        }
        if let Some(stop) = self.requested_stop() {
            return Ok((Progress::Halted(stop), Vec::new()));
        }
        let computed = Instant::now();
        let prepared = self.preparation.workers.prepare(
            Cards {
                tokenizer: runtime.tokenizer(),
                profile: &runtime.profile,
                function_digest: &self.digest,
            },
            inputs,
            &self.recipe,
            control,
        );
        self.report.bootstrap.partition_compute_ms += prepare::elapsed_ms(computed);
        let prepared = prepared?;
        let mut retained = Vec::new();
        for computed in prepared {
            if let Some(stop) = self.requested_stop() {
                return Ok((Progress::Halted(stop), Vec::new()));
            }
            let preparation = self.preparation;
            let report = &mut self.report;
            let mut committed = false;
            let stop = hold(owner, |engine| {
                if owner.closing() {
                    return Ok(Some(cancelled()));
                }
                if preparation.lock().phase == Phase::Pausing {
                    return Ok(Some(paused()));
                }
                committed = matches!(
                    prepare::commit_partition(engine, computed.partition, control, report)?,
                    cache::PartitionCommit::Committed
                );
                Ok(None)
            })?;
            if let Some(stop) = stop {
                return Ok((Progress::Halted(stop), Vec::new()));
            }
            // A superseded source's newer version stays pending; its cards
            // were rendered from the old one.
            if committed {
                retained.extend(computed.retained);
            }
        }
        Ok((progress, retained))
    }

    /// Select batches of up to the profile's batch of cards and embed them.
    /// A batch admitted smaller (foreground activity) keeps its remaining
    /// inputs, in walk order, for the next admission; the walk resumes only
    /// once fewer inputs remain than the current batch size. While a call
    /// runs, the next batch is selected and rendered ([`Self::embed`]).
    fn embed_pass(&mut self, frontier: &mut Frontier) -> FResult<Option<Stop>> {
        let full = self.runtime.profile.document_limits().inputs;
        let mut batch = Batch::default();
        let mut end = false;
        loop {
            if let Some(stop) = self.requested_stop() {
                return Ok(Some(stop));
            }
            if !end && batch.len() < self.preparation.batch_limit(Instant::now(), full) {
                let room = full - batch.len();
                let selected = self.fill(frontier, &mut batch, room, &[])?;
                if self.publish_overview_boundary && batch.is_empty() {
                    if let Some(stop) = self.publish()? {
                        return Ok(Some(stop));
                    }
                    self.publish_overview_boundary = false;
                }
                match selected {
                    Selected::Full => {}
                    Selected::End => end = true,
                    Selected::Yield => continue,
                    Selected::Halted(stop) => return Ok(Some(stop)),
                }
            }
            if !batch.is_empty()
                && let Some(stop) = self.embed(frontier, &mut batch, &mut end, full, true)?
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
    fn fill(
        &mut self,
        frontier: &mut Frontier,
        batch: &mut Batch,
        room: usize,
        in_flight: &[String],
    ) -> FResult<Selected> {
        let target = batch.len() + room;
        let mut waves = 0;
        loop {
            if let Some(stop) = self.requested_stop() {
                return Ok(Selected::Halted(stop));
            }
            // Drain the last file-view inputs, including foreground-sized
            // leftovers, before prefetching any refinement. Publication cannot
            // race ahead of those still uncommitted inputs.
            if self.publish_overview_boundary {
                return Ok(Selected::Full);
            }
            {
                let state = self.preparation.lock();
                if !frontier.changed_only
                    && frontier.priority_revision != Some(state.priority_revision)
                {
                    frontier.order.prioritize(&state.priority_paths)?;
                    frontier.priority_revision = Some(state.priority_revision);
                }
            }
            if frontier.walk.is_some()
                && frontier.wave_inputs >= prepare::WAVE_INPUTS
                && frontier.order.waiting()
            {
                frontier.yield_wave();
            }
            if frontier.walk.is_none() {
                // A prefetch must not become an all-corpus partition barrier
                // when the next areas have only cached or zero-card sources.
                if waves == 1 {
                    return Ok(Selected::Yield);
                }
                waves += 1;
                let overview_phase = frontier.order.overview_first();
                let Some(mut cursor) = frontier.order.next() else {
                    return Ok(Selected::End);
                };
                if overview_phase && !frontier.order.overview_first() {
                    self.publish_overview_boundary = true;
                    frontier.order.advance(cursor, false);
                    return Ok(Selected::Full);
                }
                let resumed = cursor.resume.take();
                let before = resumed
                    .as_ref()
                    .map_or_else(|| cursor.after.clone(), |position| position.after.clone());
                let (progress, retained) = if resumed.is_some() {
                    (Progress::More, Vec::new())
                } else {
                    self.partition_wave(&mut cursor)?
                };
                if let Progress::Halted(stop) = progress {
                    return Ok(Selected::Halted(stop));
                }
                let done = matches!(progress, Progress::Done);
                if cursor.after != before {
                    self.report.bootstrap.waves += 1;
                    self.report.bootstrap.priority_waves += u64::from(cursor.scope.is_some());
                    let mut walk =
                        Walk::wave(cursor.scope.clone(), before.clone(), cursor.after.clone())
                            .with_retained(retained);
                    if let Some(position) = resumed {
                        walk = walk.restore(position, &mut frontier.paused);
                    }
                    walk.exact = cursor.exact;
                    walk.overview_first = frontier.order.overview_first();
                    frontier.walk = Some(walk);
                    frontier.active_cursor = Some(cursor);
                    frontier.wave_inputs = 0;
                    self.sync_progress();
                } else {
                    frontier.order.advance(cursor, done);
                }
                if frontier.walk.is_none() {
                    continue;
                }
            }
            let (owner, runtime) = (self.owner, self.runtime);
            let (recipe, report) = (&self.recipe, &mut self.report);
            let cards = Cards {
                tokenizer: runtime.tokenizer(),
                profile: &runtime.profile,
                function_digest: &self.digest,
            };
            // Priority and background waves can overlap the active call. Keep
            // only bounded pending keys across waves; committed keys use cache.
            frontier
                .walk
                .as_mut()
                .expect("wave exists")
                .exclude(in_flight.iter().chain(batch.keys.iter()).cloned());
            let mut selection = Selection::default();
            let selected = hold(owner, |engine| {
                Steps {
                    engine,
                    cards,
                    recipe,
                }
                .select_batch(
                    frontier.walk.as_mut().expect("wave exists"),
                    &mut selection,
                    if frontier.order.waiting() {
                        (target - batch.len()).min(prepare::WAVE_INPUTS - frontier.wave_inputs)
                    } else {
                        target - batch.len()
                    },
                    SOURCES_PER_STEP,
                    report,
                    &mut || owner.closing().then(cancelled),
                )
            })?;
            if selection.len() > 0 {
                frontier.wave_inputs += selection.len();
                let started = Instant::now();
                let rendered = cards.render(selection);
                self.report.bootstrap.render_ms += prepare::elapsed_ms(started);
                let rendered = rendered?;
                batch.keys.extend(rendered.keys);
                batch.inputs.extend(rendered.inputs);
            }
            match selected {
                Selected::End => {
                    frontier.walk = None;
                    let mut cursor = frontier.active_cursor.take().expect("active wave cursor");
                    cursor.resume = None;
                    frontier.order.advance(cursor, false);
                    // Send a short first wave without waiting for later sources.
                    // The following wave is prepared by the one in-call prefetch.
                    if !batch.is_empty() {
                        return Ok(Selected::Full);
                    }
                    if self.published == 0
                        && self.report.reused_cached_units > 0
                        && let Some(stop) = self.publish()?
                    {
                        return Ok(Selected::Halted(stop));
                    }
                }
                other => {
                    if matches!(other, Selected::Full)
                        && frontier.wave_inputs >= prepare::WAVE_INPUTS
                        && frontier.order.waiting()
                    {
                        // Scalar progress skips already selected inputs; in-flight
                        // and buffered keys remain excluded across other cursors.
                        frontier.yield_wave();
                    }
                    return Ok(other);
                }
            }
        }
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
    /// committed. A failed prefetch is reported only after that commit, and
    /// the committed batch is counted first, so the stop publishes it.
    fn embed(
        &mut self,
        frontier: &mut Frontier,
        batch: &mut Batch,
        end: &mut bool,
        full: usize,
        source_inputs: bool,
    ) -> FResult<Option<Stop>> {
        #[cfg(feature = "test-faults")]
        self.owner.admitting();
        let model_permit = loop {
            if let Some(stop) = self.requested_stop() {
                return Ok(Some(stop));
            }
            if let Some(permit) = self.owner.try_model() {
                break permit;
            }
            std::thread::sleep(POLL);
        };
        let preparation = self.preparation;
        let started = Instant::now();
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
            let admitted = batch.take(self.runtime.profile.document_limits(), size)?;
            let keys = admitted.keys;
            let inputs = admitted.inputs;
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
            match self.fill(frontier, batch, full - batch.len(), &keys) {
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
                self.report.bootstrap.provider_ms += prepare::elapsed_ms(started);
                self.control.cancel();
                return Ok(Some(cancelled()));
            }
            if let Some(result) = call.wait(POLL) {
                break result;
            }
        };
        self.preparation.set_in_call(false);
        drop(model_permit);
        self.report.bootstrap.provider_ms += prepare::elapsed_ms(started);
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
        let (dims, cap) = (
            self.runtime.profile.descriptor.dims(),
            self.runtime.cache_cap_bytes,
        );
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
                engine, keys, vectors, digest, dims, cap, control, report,
            ))
        })?;
        match committed {
            Ok(()) => {}
            Err(StopOrError::Stop(stop)) => return Ok(Some(stop)),
            Err(StopOrError::Error(error)) => return Err(error),
        }
        // Committed is unpublished until a publication, whatever stops the
        // run next: the stop ([`Self::finish`]) publishes it.
        if source_inputs {
            self.unpublished += size;
            self.report.embedded_units += size;
        } else {
            self.report.embedded_areas += size;
        }
        self.sync_progress();
        if let Some(error) = prefetch_error {
            return Err(error);
        }
        let file_views_drained = self.publish_overview_boundary && batch.is_empty();
        if source_inputs && (file_views_drained || self.unpublished >= self.published.max(1)) {
            let stop = self.publish()?;
            if file_views_drained && stop.is_none() {
                self.publish_overview_boundary = false;
            }
            return Ok(stop);
        }
        Ok(None)
    }

    fn area_pass(&mut self) -> FResult<Option<Stop>> {
        let profile = &self.runtime.profile;
        let records = hold(self.owner, |engine| {
            super::area::snapshot(engine, profile, self.control)
        })?;
        // Tokenization holds neither the owner slot nor a database transaction.
        let areas = super::area::render(records, profile, self.runtime.tokenizer(), self.control)?;
        hold(self.owner, |engine| {
            super::area::prune(engine, profile, &areas, self.control)
        })?;
        self.report.area_vectors = 0;
        for area in areas {
            if let Some(stop) = self.requested_stop() {
                return Ok(Some(stop));
            }
            if !hold(self.owner, |engine| {
                super::area::cached(engine, profile, &area)
            })? {
                let mut frontier = Frontier {
                    order: prepare::SourceOrder::new(&[])?
                        .for_representation(self.runtime.profile.representation),
                    walk: None,
                    paused: prepare::PausedSources::default(),
                    active_cursor: None,
                    wave_inputs: 0,
                    priority_revision: None,
                    changed_only: false,
                };
                let mut batch = Batch {
                    keys: vec![area.key().to_owned()],
                    inputs: vec![super::provider::TokenizedInput {
                        ids: area.ids.clone(),
                    }],
                };
                if let Some(stop) = self.embed(&mut frontier, &mut batch, &mut true, 1, false)? {
                    return Ok(Some(stop));
                }
            }
            // Superseded: since the snapshot, its brief changed or was
            // forgotten, or one of its sources changed. That write keeps its
            // own obligation (pending overviews, or the source's path), so a
            // later pass binds the current area or drops it.
            if !hold(self.owner, |engine| {
                super::area::bind(engine, profile, &area.record, self.control)
            })? {
                continue;
            }
            self.report.area_vectors += 1;
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

    /// Publish committed coverage using bounded deltas, then drop the runtime's
    /// loaded index so the next request loads the new generation.
    fn publish(&mut self) -> FResult<Option<Stop>> {
        let started = Instant::now();
        let mut attempt = 0usize;
        let delays = [250u64, 1000, 3000];
        let outcome = loop {
            match self.generation() {
                Ok(publication) => break Ok(publication),
                Err(Failed::Owner(error)) => return Err(error),
                Err(Failed::Step(error)) => {
                    if attempt == delays.len()
                        || !matches!(error.code(), "internal" | "index_unavailable")
                    {
                        break Err(error);
                    }
                    let delay = delays[attempt];
                    attempt += 1;
                    self.report.index_published = false;
                    self.report.index_reason = Some(error.to_string());
                    self.report.bootstrap.publication_retries += 1;
                    self.report.bootstrap.publication_backoff_ms = Some(delay);
                    self.report.bootstrap.publication_error =
                        Some(error.to_string().chars().take(240).collect());
                    self.sync_progress();
                    let until = Instant::now() + Duration::from_millis(delay);
                    while Instant::now() < until {
                        if let Err(error) = self.control.check() {
                            self.report.bootstrap.publication_backoff_ms = None;
                            self.sync_progress();
                            return Err(error);
                        }
                        if let Some(stop) = self.requested_stop() {
                            self.report.bootstrap.publication_backoff_ms = None;
                            self.sync_progress();
                            return Ok(Some(stop));
                        }
                        std::thread::sleep(Duration::from_millis(25));
                    }
                }
            }
        };
        self.report.bootstrap.publication_backoff_ms = None;
        if outcome.is_ok() {
            self.report.bootstrap.publication_error = None;
        }
        let changed = matches!(
            outcome,
            Ok(Publication::Rebuilt(_) | Publication::Delta(_) | Publication::Mapped(_))
        );
        let stop = prepare::record_publication(outcome, 0, &mut self.report);
        self.report.bootstrap.publication_ms += prepare::elapsed_ms(started);
        self.sync_progress();
        if stop.is_none() {
            if self.report.index_published {
                if changed {
                    self.runtime.forget_index();
                }
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
    fn generation(&self) -> Result<Publication, Failed> {
        let control = self.control;
        let Some((identity, store, segments)) = self.step(|engine| {
            let Some(identity) = engine.semantic_identity()? else {
                return Ok(None);
            };
            Ok(Some((
                identity,
                engine.semantic_anchor()?,
                engine.semantic_segments.clone(),
            )))
        })?
        else {
            return Ok(Publication::Nothing);
        };
        let previous = match validate_cached(&store, &identity, control, false, &segments) {
            Ok((generation, _)) => Some(generation),
            Err(GenerationError::Interrupted(error)) => return Err(Failed::Step(error)),
            Err(_) => None,
        };
        if let Some(generation) = &previous
            && self.step(|engine| engine.semantic_generation_current(generation))?
        {
            return Ok(Publication::Current(generation.manifest.count));
        }
        let mut walk = self.step(|engine| {
            MappingWalk::resume(
                engine,
                &identity.digest,
                &identity.recipe,
                previous.as_ref(),
            )
        })?;
        while !self.step(|engine| walk.step(engine, control, Some(Instant::now() + STEP_TIME)))? {}
        let mapping = self.step(|engine| walk.finish(engine, control))?;
        if let Some(generation) = &previous
            && mapping.published_by(generation)
        {
            let directory = index::open_generation_dir(&store, &identity.digest, false)
                .map_err(Failed::Step)?
                .expect("validated generation");
            let id = directory.identity().map_err(|e| Failed::Step(e.into()))?;
            self.step(|engine| {
                engine.semantic_remember_mapping(
                    generation.manifest.mapping_id(),
                    mapping.units,
                    id,
                    control,
                )?;
                engine.semantic_record_coverage(mapping.coverage)
            })?;
            return Ok(Publication::Current(generation.manifest.count));
        }
        if previous.is_none() && mapping.units.is_empty() {
            self.step(|engine| engine.semantic_record_coverage(mapping.coverage))?;
            return Ok(Publication::Nothing);
        }
        let mut plan = index::publication::Plan::new(mapping, previous);
        let mut keys = plan.needed().into_iter();
        let mut entries = Vec::with_capacity(keys.len());
        while !self.step(|engine| {
            index::lookup_entries(
                engine,
                &mut keys,
                &identity,
                &mut plan.scope,
                &mut entries,
                control,
                Some(Instant::now() + STEP_TIME),
            )
        })? {}
        #[cfg(feature = "test-faults")]
        let built = || self.owner.staging();
        #[cfg(not(feature = "test-faults"))]
        let built = || {};
        let (staged, coverage, publication) = plan
            .stage(&store, &identity, &entries, control, &built)
            .map_err(Failed::Step)?;
        drop(entries);
        self.step(|engine| {
            engine.semantic_publish_staged(staged, control)?;
            engine.semantic_record_coverage(coverage)
        })?;
        Ok(publication)
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
        if !cancelled
            && self.report.index_reason.is_none()
            && (self.unpublished > 0 || !matches!(&outcome, Ok(Stop::Complete)))
        {
            // A paused wave may have updated only locations/partition coverage
            // through cache reuse. Publish that scope even without new vectors.
            match self.publish() {
                Ok(None) => {}
                Ok(Some(published)) => {
                    outcome = outcome.map(|stop| prepare::merge_publication(Some(stop), published));
                }
                Err(error) => outcome = outcome.and(Err(error)),
            }
        }
        self.sync_progress();
        let (owner, preparation, report) = (self.owner, self.preparation, &mut self.report);
        let resumed = hold(owner, |engine| {
            let mut state = preparation.lock();
            if state.phase == Phase::Running && !owner.closing() {
                return Ok(true);
            }
            let written = prepare::finalize(engine, report, &outcome);
            state.phase = Phase::Idle;
            state.waiting = false;
            state.continuous = false;
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

/// Cheap committed publication observation plus this owner's live state.
/// Source changes mark counters stale; payload integrity is checked on use/audit.
pub fn status_object(
    engine: &Engine,
    live: Option<Live>,
    runtime: &str,
    control: &Control,
) -> FResult<serde_json::Value> {
    let mut value = engine.semantic_progress(control)?;
    overlay(&mut value, live);
    value["runtime"] = runtime.into();
    Ok(value)
}

/// Explicit full audit, bounded to half of the remaining read deadline.
pub fn status_audit_object(
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
        Some(Live::Waiting) => {
            value["state"] = "waiting".into();
            value["last_error"] = serde_json::Value::Null;
        }
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

    #[test]
    fn large_priority_wave_yields_to_background_without_duplicate_prefetch() {
        struct Checked {
            inner: Sizes,
            owner: Arc<Quiet>,
            seen: Arc<Mutex<std::collections::HashSet<Vec<u32>>>>,
        }
        impl EmbeddingProvider for Checked {
            fn descriptor(&self) -> &FunctionDescriptor {
                self.inner.descriptor()
            }
            fn embed_query(
                &mut self,
                _: &TokenizedInput,
                _: Instant,
            ) -> Result<Vec<f32>, ProviderError> {
                unreachable!()
            }
            fn embed_documents(
                &mut self,
                inputs: &[TokenizedInput],
                control: &Control,
            ) -> Result<Vec<Vec<f32>>, ProviderError> {
                if self.inner.sizes.lock().unwrap().len() == 1 {
                    assert!(
                        self.owner
                            .0
                            .lock()
                            .unwrap()
                            .semantic_partition("note00.md")
                            .unwrap()
                            .is_some(),
                        "background must be prepared for the second call"
                    );
                }
                for input in inputs {
                    assert!(
                        self.seen.lock().unwrap().insert(input.ids.clone()),
                        "a prefetched or committed input must not be inferred twice"
                    );
                }
                self.inner.embed_documents(inputs, control)
            }
        }
        let (_dir, engine, profile) = notes();
        let large: String = (0..40)
            .map(|i| format!("pub fn priority_{i:02}() -> u32 {{ {i} }}\n"))
            .collect();
        engine.replace_source("z/large.rs", &large).unwrap();
        let owner = Arc::new(Quiet(Mutex::new(engine)));
        let sizes = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::new(Mutex::new(std::collections::HashSet::new()));
        let provider = Checked {
            inner: Sizes {
                descriptor: profile.descriptor.clone(),
                sizes: Arc::clone(&sizes),
                foreground: None,
            },
            owner: Arc::clone(&owner),
            seen: Arc::clone(&seen),
        };
        let runtime = Arc::new(
            QueryRuntime::start(
                profile,
                Box::new(move || Ok(Box::new(provider) as Box<dyn EmbeddingProvider>)),
            )
            .unwrap(),
        );
        let preparation = Arc::new(Preparation::default());
        preparation
            .prepare_with_priorities(
                Arc::clone(&owner) as Arc<dyn Owner>,
                Arc::clone(&runtime),
                Some(vec!["z".into()]),
            )
            .unwrap();
        until_idle(&preparation);
        runtime.shutdown();
        let state = owner.0.lock().unwrap().semantic_state().unwrap().unwrap();
        assert!(state.last_error.is_none(), "{state:?}");
        assert_eq!(state.committed_units, 52);
        assert_eq!(seen.lock().unwrap().len(), 52);
    }

    /// Exercise the actual CLI pending loop and MCP prefetch loop, including
    /// short source tails, cached holes, duplicate keys and non-divisor batches.
    #[test]
    fn cli_and_mcp_hierarchy_turns_preserve_mixed_pending_inputs() {
        use crate::neural::partition::Representation;
        use crate::neural::tokenize::DocumentTokenizer;
        use std::collections::HashSet;

        struct Checked {
            inner: Sizes,
            seen: Arc<Mutex<HashSet<Vec<u32>>>>,
            first: Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>,
        }
        impl EmbeddingProvider for Checked {
            fn descriptor(&self) -> &FunctionDescriptor {
                self.inner.descriptor()
            }
            fn embed_query(
                &mut self,
                _: &TokenizedInput,
                _: Instant,
            ) -> Result<Vec<f32>, ProviderError> {
                unreachable!()
            }
            fn embed_documents(
                &mut self,
                inputs: &[TokenizedInput],
                control: &Control,
            ) -> Result<Vec<Vec<f32>>, ProviderError> {
                for input in inputs {
                    assert!(
                        self.seen.lock().unwrap().insert(input.ids.clone()),
                        "no committed or prefetched input may be inferred twice"
                    );
                }
                let result = self.inner.embed_documents(inputs, control);
                if let Some((entered, release)) = self.first.take() {
                    entered.send(()).unwrap();
                    release.recv_timeout(Duration::from_secs(30)).unwrap();
                }
                result
            }
        }
        for mcp in [false, true] {
            for batch in [1, 3] {
                let (dir, engine, mut profile) = notes();
                Arc::make_mut(&mut profile).representation =
                    Representation::Hierarchy { parent_tokens: 32 };
                Arc::make_mut(&mut profile).card_tokens = 512;
                Arc::make_mut(&mut profile).batch = batch;
                let profile_path = dir.path().join("profile-notes.json");
                std::fs::write(&profile_path, serde_json::to_vec(profile.as_ref()).unwrap())
                    .unwrap();
                let mut paths: Vec<String> = (0..12).map(|n| format!("note{n:02}.md")).collect();
                for (path, count, family) in [
                    ("z/short.rs", 3, "short"),
                    ("z/twin.rs", 3, "short"),
                    ("z/large.rs", 41, "large"),
                    ("a/middle.rs", 17, "middle"),
                ] {
                    let body: String = (0..count)
                        .map(|n| format!("pub fn {family}_{n:02}() -> u32 {{ {n} }}\n"))
                        .collect();
                    engine.replace_source(path, &body).unwrap();
                    paths.push(path.into());
                }
                let digest = profile.descriptor.digest();
                let recipe = profile.recipe_id();
                let tokenizer = DocumentTokenizer::load(&profile).unwrap();
                let steps = Steps {
                    engine: &engine,
                    cards: Cards {
                        tokenizer: &tokenizer,
                        profile: &profile,
                        function_digest: &digest,
                    },
                    recipe: &recipe,
                };
                let mut after = None;
                while !matches!(
                    steps
                        .partition_page(
                            &mut after,
                            8,
                            None,
                            &Control::unbounded(),
                            &mut PrepareReport::default(),
                            &mut || None
                        )
                        .unwrap(),
                    Progress::Done
                ) {}
                let partitions: Vec<_> = paths
                    .iter()
                    .map(|path| engine.semantic_partition(path).unwrap().unwrap())
                    .collect();
                let keys: HashSet<_> = partitions
                    .iter()
                    .flat_map(|partition| partition.units.iter().map(|unit| unit.input_key.clone()))
                    .collect();
                let locations: usize = partitions
                    .iter()
                    .map(|partition| partition.units.len())
                    .sum();
                assert!(keys.len() < locations, "fixture includes shared input keys");
                assert!(
                    keys.len() > 8 * 4,
                    "fixture requires repeated contended turns"
                );
                let large = engine.semantic_partition("z/large.rs").unwrap().unwrap();
                let seeded: Vec<_> = large
                    .units
                    .iter()
                    .skip(2)
                    .step_by(5)
                    .take(3)
                    .map(|unit| (unit.input_key.clone(), unit_vector(&profile.descriptor)))
                    .collect();
                assert_eq!(seeded.len(), 3);
                engine
                    .semantic_cache_commit(&seeded, &digest, profile.descriptor.dims(), u64::MAX)
                    .unwrap();
                drop(tokenizer);
                let sizes = Arc::new(Mutex::new(Vec::new()));
                let seen = Arc::new(Mutex::new(HashSet::new()));
                let preparation = Arc::new(Preparation::default());
                let priorities = vec!["z".into(), "a".into()];
                if mcp {
                    let (entered_tx, entered) = mpsc::channel();
                    let (release, release_rx) = mpsc::channel();
                    let (prefetched_tx, prefetched) = mpsc::channel();
                    let owner = Arc::new(Counted {
                        engine: Mutex::new(engine),
                        in_flight: AtomicUsize::new(0),
                        closing: AtomicBool::new(false),
                        prefetched: Mutex::new(prefetched_tx),
                        fail_prefetch: AtomicBool::new(false),
                        armed: AtomicBool::new(false),
                    });
                    let provider = Checked {
                        inner: Sizes {
                            descriptor: profile.descriptor.clone(),
                            sizes: Arc::clone(&sizes),
                            foreground: Some(Arc::clone(&preparation)),
                        },
                        seen: Arc::clone(&seen),
                        first: Some((entered_tx, release_rx)),
                    };
                    let runtime = Arc::new(
                        QueryRuntime::start(
                            Arc::clone(&profile),
                            Box::new(move || Ok(Box::new(provider) as Box<dyn EmbeddingProvider>)),
                        )
                        .unwrap(),
                    );
                    preparation
                        .prepare_with_priorities(
                            Arc::clone(&owner) as Arc<dyn Owner>,
                            Arc::clone(&runtime),
                            Some(priorities),
                        )
                        .unwrap();
                    entered.recv_timeout(Duration::from_secs(30)).unwrap();
                    assert!(
                        prefetched.recv_timeout(Duration::from_secs(30)).unwrap() > 0,
                        "the real driver buffers inputs while the first call is blocked"
                    );
                    release.send(()).unwrap();
                    until_idle(&preparation);
                    if batch == 3 {
                        assert!(
                            prefetched.try_iter().any(|size| size == 3),
                            "a full prefetch must survive smaller foreground admissions"
                        );
                    }
                    let before = sizes.lock().unwrap().len();
                    preparation
                        .prepare(Arc::clone(&owner) as Arc<dyn Owner>, Arc::clone(&runtime))
                        .unwrap();
                    until_idle(&preparation);
                    runtime.shutdown();
                    assert_eq!(
                        sizes.lock().unwrap().len(),
                        before,
                        "unchanged MCP replay has zero calls"
                    );
                    let status = locked(&owner.engine)
                        .semantic_status(&Control::unbounded())
                        .unwrap();
                    assert!(status.last_error.is_none(), "{status:?}");
                    assert_eq!(status.cached_current_units, locations as u64, "{status:?}");
                    assert_eq!(
                        status.searchable_current_units, locations as u64,
                        "{status:?}"
                    );
                    assert_eq!(status.unpartitioned_sources, 0, "{status:?}");
                    assert_eq!(status.missing_units, 0, "{status:?}");
                    assert!(
                        sizes
                            .lock()
                            .unwrap()
                            .iter()
                            .skip(1)
                            .all(|size| *size <= FOREGROUND_BATCH)
                    );
                } else {
                    drop(engine);
                    let provider_sizes = Arc::clone(&sizes);
                    let provider_seen = Arc::clone(&seen);
                    let control = Control::unbounded();
                    let options = prepare::PrepareOptions {
                        profile_path: &profile_path,
                        budget_seconds: 60,
                        development: true,
                        cache_cap_bytes: u64::MAX,
                        started: Instant::now(),
                        control: &control,
                        priority_paths: &priorities,
                    };
                    let report = prepare::run(
                        &dir.path().join("store"),
                        &options,
                        Box::new(move |profile, _, _, _| {
                            Ok(Box::new(Checked {
                                inner: Sizes {
                                    descriptor: profile.descriptor.clone(),
                                    sizes: Arc::clone(&provider_sizes),
                                    foreground: None,
                                },
                                seen: Arc::clone(&provider_seen),
                                first: None,
                            }))
                        }),
                    )
                    .unwrap();
                    assert_eq!(
                        report.embedded_units as usize,
                        keys.len() - seeded.len(),
                        "{report:?}"
                    );
                    let replay = prepare::run(
                        &dir.path().join("store"),
                        &prepare::PrepareOptions {
                            started: Instant::now(),
                            ..options
                        },
                        Box::new(|_, _, _, _| {
                            panic!("unchanged CLI replay must not acquire a provider")
                        }),
                    )
                    .unwrap();
                    assert_eq!(replay.document_calls, 0, "{replay:?}");
                    let engine = Engine::open_existing(&dir.path().join("store")).unwrap();
                    let status = engine.semantic_status(&control).unwrap();
                    assert!(status.last_error.is_none(), "{status:?}");
                    assert_eq!(status.cached_current_units, locations as u64, "{status:?}");
                    assert_eq!(
                        status.searchable_current_units, locations as u64,
                        "{status:?}"
                    );
                    assert_eq!(status.unpartitioned_sources, 0, "{status:?}");
                    assert_eq!(status.missing_units, 0, "{status:?}");
                }
                assert!(
                    sizes
                        .lock()
                        .unwrap()
                        .iter()
                        .all(|size| *size <= batch as usize)
                );
                assert_eq!(
                    sizes.lock().unwrap().iter().sum::<usize>(),
                    keys.len() - seeded.len(),
                    "all missing unique inputs committed exactly once: mcp={mcp}, batch={batch}"
                );
                assert_eq!(seen.lock().unwrap().len(), keys.len() - seeded.len());
            }
        }
    }

    #[test]
    fn continuous_area_changes_wake_the_driver_without_reselecting_sources() {
        use crate::brief;
        use serde_json::json;
        let (_dir, engine, mut profile) = notes();
        Arc::make_mut(&mut profile).representation =
            super::super::partition::Representation::Hierarchy { parent_tokens: 32 };
        Arc::make_mut(&mut profile).card_tokens = 512;
        let sizes = Arc::new(Mutex::new(Vec::new()));
        let provider = Sizes {
            descriptor: profile.descriptor.clone(),
            sizes: sizes.clone(),
            foreground: None,
        };
        let runtime = Arc::new(
            QueryRuntime::start(
                profile,
                Box::new(move || Ok(Box::new(provider) as Box<dyn EmbeddingProvider>)),
            )
            .unwrap(),
        );
        let owner = Arc::new(Quiet(Mutex::new(engine)));
        let preparation = Arc::new(Preparation::default());
        preparation
            .maintain(owner.clone(), runtime.clone(), None)
            .unwrap();
        let settled = |areas| {
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                if preparation.live() == Some(Live::Waiting)
                    && !owner
                        .0
                        .lock()
                        .unwrap()
                        .derivation_pending(crate::neural::work::Lane::Semantic)
                        .unwrap()
                        .overviews_pending
                    && preparation.lock().bootstrap.area_vectors == areas
                {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "overview maintenance did not settle"
                );
                std::thread::sleep(POLL);
            }
        };
        settled(0);
        let selected = preparation.lock().bootstrap.selected_sources;
        let calls = sizes.lock().unwrap().len();
        let wid = owner.0.lock().unwrap().workspace_id().unwrap();
        let spans: Vec<_> = (0..2)
            .map(|n| {
                let source = format!("# Note {n}\n\nbody of note {n}\n");
                format!(
                    "note{n:02}.md#0-{}@{}.{}",
                    source.len(),
                    &crate::digest(source.as_bytes())[..32],
                    &wid[..16]
                )
            })
            .collect();
        let mut revision = None;
        let mut id = String::new();
        for text in [
            "Explain the shared note workflow.",
            "Explain the revised shared note workflow.",
        ] {
            let input = json!({"cited_spans":spans,"recipe_sha256":"a".repeat(64),"model":"fixture",
                "artifact_sha256":"b".repeat(64),"prompt_sha256":crate::digest(text.as_bytes()),"generated_at_unix_ms":1,
                "purpose":[{"text":text,"citations":[0,1],"identifiers":[]}],
                "behavior":[{"text":"Retain both notes.","citations":[0,1],"identifiers":[]}],
                "provides":[],"consumes":[],"constraints":[],"tests":[],"unknowns":[]});
            let request = brief::parse(&serde_json::to_vec(&json!({"workspace_id":wid,"command":{"op":"put","brief":input,"expected_revision":revision}})).unwrap()).unwrap();
            let result = owner
                .0
                .lock()
                .unwrap()
                .brief(&request, &Control::unbounded())
                .unwrap();
            revision = result["revision"].as_u64();
            id = result["id"].as_str().unwrap().to_owned();
            settled(1);
            assert_eq!(preparation.lock().bootstrap.selected_sources, selected);
        }
        assert_eq!(
            sizes.lock().unwrap().len(),
            calls + 2,
            "one new area input per changed brief"
        );
        let request = brief::parse(&serde_json::to_vec(&json!({"workspace_id":wid,"command":{"op":"forget","id":id,"expected_revision":revision}})).unwrap()).unwrap();
        owner
            .0
            .lock()
            .unwrap()
            .brief(&request, &Control::unbounded())
            .unwrap();
        settled(0);
        assert_eq!(
            sizes.lock().unwrap().len(),
            calls + 2,
            "forgetting embeds nothing"
        );
        assert_eq!(preparation.lock().bootstrap.selected_sources, selected);
        preparation.pause();
        until_idle(&preparation);
        runtime.shutdown();
    }

    #[test]
    fn a_pause_publishes_changed_locations_without_new_vectors() {
        let (_dir, engine, profile) = notes();
        let sizes = Arc::new(Mutex::new(Vec::new()));
        let provider = Sizes {
            descriptor: profile.descriptor.clone(),
            sizes: Arc::clone(&sizes),
            foreground: None,
        };
        let runtime = QueryRuntime::start(
            profile,
            Box::new(move || Ok(Box::new(provider) as Box<dyn EmbeddingProvider>)),
        )
        .unwrap();
        let owner = Quiet(Mutex::new(engine));
        let preparation = Preparation::default();
        preparation.lock().phase = Phase::Running;
        let control = Control::unbounded();
        let mut run = Run::new(&owner, &runtime, &preparation, &control);
        run.begin().unwrap();
        assert!(matches!(run.passes().unwrap(), Stop::Complete));
        assert_eq!(run.published, 12);
        assert_eq!(run.unpublished, 0);
        owner.0.lock().unwrap().delete_source("note00.md").unwrap();
        preparation.pause();
        assert!(!run.finish(Ok(paused())));
        let engine = owner.0.lock().unwrap();
        let status = engine.semantic_status(&control).unwrap();
        assert_eq!(status.searchable_current_units, 11);
        let state = engine.semantic_state().unwrap().unwrap();
        assert_eq!(state.bootstrap.unwrap().published_entries, 11);
        assert_eq!(sizes.lock().unwrap().iter().sum::<usize>(), 12);
    }

    #[test]
    fn publication_failure_backs_off_visibly_without_the_owner_slot_or_model_calls() {
        use crate::fault::{self, Action};
        use crate::neural::fault_names::PUBLISH_BEFORE_MANIFEST;
        let (_dir, engine, profile) = notes();
        let sizes = Arc::new(Mutex::new(Vec::new()));
        let provider = Sizes {
            descriptor: profile.descriptor.clone(),
            sizes: Arc::clone(&sizes),
            foreground: None,
        };
        let runtime = QueryRuntime::start(
            profile,
            Box::new(move || Ok(Box::new(provider) as Box<dyn EmbeddingProvider>)),
        )
        .unwrap();
        let owner = Quiet(Mutex::new(engine));
        let preparation = Preparation::default();
        preparation.lock().phase = Phase::Running;
        let control = Control::unbounded();
        let mut run = Run::new(&owner, &runtime, &preparation, &control);
        run.begin().unwrap();
        assert!(matches!(run.passes().unwrap(), Stop::Complete));
        owner.0.lock().unwrap().delete_source("note00.md").unwrap();
        let before = fault::reached(PUBLISH_BEFORE_MANIFEST);
        fault::arm(
            PUBLISH_BEFORE_MANIFEST,
            0,
            Action::Fail("publication disk failure".into()),
        );
        let stop = std::thread::scope(|scope| {
            let observed = scope.spawn(|| {
                let until = Instant::now() + Duration::from_secs(10);
                loop {
                    if preparation
                        .progress()
                        .unwrap()
                        .publication_backoff_ms
                        .is_some()
                    {
                        let engine = owner.0.try_lock().expect("backoff holds no serving slot");
                        let status = engine.semantic_status(&Control::unbounded()).unwrap();
                        assert!(status.index.available, "prior generation is still readable");
                        assert!(preparation.progress().unwrap().publication_error.is_some());
                        break;
                    }
                    assert!(Instant::now() < until, "backoff not observable");
                    std::thread::sleep(POLL);
                }
            });
            let stop = run
                .publish()
                .unwrap()
                .expect("persistent failure stops after bounded retries");
            observed.join().unwrap();
            stop
        });
        assert_eq!(run.report.bootstrap.publication_retries, 3);
        assert_eq!(fault::reached(PUBLISH_BEFORE_MANIFEST), before + 4);
        assert!(run.report.index_reason.is_some());
        preparation.decided();
        assert!(!run.finish(Ok(stop)));
        assert_eq!(
            fault::reached(PUBLISH_BEFORE_MANIFEST),
            before + 4,
            "finalization must not start another retry cycle"
        );
        fault::disarm_all();
        assert_eq!(
            owner
                .0
                .lock()
                .unwrap()
                .derivation_pending(crate::neural::work::Lane::Semantic)
                .unwrap()
                .paths,
            1
        );
        preparation.lock().phase = Phase::Running;
        let mut resumed = Run::new(&owner, &runtime, &preparation, &control);
        resumed.begin().unwrap();
        assert!(matches!(resumed.passes().unwrap(), Stop::Complete));
        assert_eq!(
            sizes.lock().unwrap().iter().sum::<usize>(),
            12,
            "publication retries do not repeat inference"
        );
        assert_eq!(
            owner
                .0
                .lock()
                .unwrap()
                .derivation_pending(crate::neural::work::Lane::Semantic)
                .unwrap()
                .paths,
            0
        );
    }

    #[test]
    fn partition_computation_releases_the_engine_slot_and_obeys_pause_and_shutdown() {
        struct Available {
            engine: Mutex<Engine>,
            closing: AtomicBool,
        }
        impl Owner for Available {
            fn try_primary(&self, step: &mut dyn FnMut(&Engine)) -> FResult<bool> {
                step(&self.engine.lock().unwrap());
                Ok(true)
            }
            fn closing(&self) -> bool {
                self.closing.load(Ordering::SeqCst)
            }
        }
        for stop in ["none", "pause", "shutdown"] {
            let (_dir, engine, profile) = notes();
            let descriptor = profile.descriptor.clone();
            let runtime = QueryRuntime::start(
                profile,
                Box::new(move || {
                    Ok(Box::new(Sizes {
                        descriptor,
                        sizes: Arc::new(Mutex::new(Vec::new())),
                        foreground: None,
                    }) as Box<dyn EmbeddingProvider>)
                }),
            )
            .unwrap();
            let owner = Arc::new(Available {
                engine: Mutex::new(engine),
                closing: AtomicBool::new(false),
            });
            let preparation = Arc::new(Preparation::default());
            preparation.lock().phase = Phase::Running;
            let foreground = Arc::clone(&owner);
            let pause = Arc::clone(&preparation);
            let entered = Arc::new(AtomicBool::new(false));
            let seen = Arc::clone(&entered);
            crate::fault::arm(
                prepare::fault_names::PARTITION_BEFORE_COMPUTE,
                0,
                crate::fault::Action::Call(Box::new(move |_| {
                    let engine = foreground
                        .engine
                        .try_lock()
                        .expect("partition CPU work must release the serving slot");
                    let evidence = engine
                        .context_candidates(
                            "note00.md",
                            crate::Strategy::Search,
                            &Control::unbounded(),
                        )
                        .unwrap();
                    assert!(!evidence.items.is_empty());
                    // A write succeeds too: there is no lingering redb writer.
                    engine
                        .replace_source("foreground.md", "# Foreground source\n")
                        .unwrap();
                    seen.store(true, Ordering::SeqCst);
                    if stop == "pause" {
                        pause.pause();
                    }
                    if stop == "shutdown" {
                        foreground.closing.store(true, Ordering::SeqCst);
                    }
                })),
            );
            let control = Control::unbounded();
            let (result, _) = Run::new(&*owner, &runtime, &preparation, &control)
                .partition_wave(&mut prepare::SourceCursor::default())
                .unwrap();
            crate::fault::disarm_all();
            assert!(entered.load(Ordering::SeqCst));
            let engine = owner.engine.lock().unwrap();
            if stop == "none" {
                assert!(!matches!(result, Progress::Halted(_)));
                assert!(engine.semantic_partition("note00.md").unwrap().is_some());
            } else {
                assert!(
                    matches!(result, Progress::Halted(Stop::Partial { code, .. }) if code == if stop == "pause" { "paused" } else { "cancelled" })
                );
                assert!(engine.semantic_partition("note00.md").unwrap().is_none());
            }
        }
    }

    /// A foreground write queued for the owner's engine slot.
    type Write = Box<dyn FnOnce(&Engine) + Send>;

    /// An owner with no foreground operation in flight whose queued writes
    /// run as soon as a store step gives the engine slot back, before the
    /// driver's next step: a foreground write between two of its steps.
    struct Writes {
        engine: Mutex<Engine>,
        queued: Mutex<Vec<Write>>,
    }

    impl Owner for Writes {
        fn try_primary(&self, step: &mut dyn FnMut(&Engine)) -> FResult<bool> {
            let engine = self.engine.lock().unwrap();
            step(&engine);
            for write in std::mem::take(&mut *self.queued.lock().unwrap()) {
                write(&engine);
            }
            Ok(true)
        }
        fn closing(&self) -> bool {
            false
        }
    }

    /// The twelve notes behind a [`Writes`] owner, and a runtime serving
    /// their profile.
    fn writable_notes() -> (tempfile::TempDir, Arc<Writes>, QueryRuntime) {
        let (dir, engine, profile) = notes();
        let descriptor = profile.descriptor.clone();
        let runtime = QueryRuntime::start(
            profile,
            Box::new(move || {
                Ok(Box::new(Sizes {
                    descriptor,
                    sizes: Arc::new(Mutex::new(Vec::new())),
                    foreground: None,
                }) as Box<dyn EmbeddingProvider>)
            }),
        )
        .unwrap();
        let owner = Arc::new(Writes {
            engine: Mutex::new(engine),
            queued: Mutex::new(Vec::new()),
        });
        (dir, owner, runtime)
    }

    /// Replace `path` with a new version, or delete it.
    fn supersede(engine: &Engine, path: &str, delete: bool) {
        if delete {
            assert!(engine.delete_source(path).unwrap());
        } else {
            assert!(
                engine
                    .replace_source(path, "# Changed note\n\nnew body\n")
                    .unwrap()
            );
        }
    }

    /// FOLLOWUP F13: a source replaced or deleted while its partition is
    /// computed with the slot free is superseded, not an error. Its
    /// partition and cards are dropped and the rest of the wave commits.
    #[test]
    fn a_source_changed_during_partition_compute_is_superseded() {
        for delete in [false, true] {
            let (_dir, owner, runtime) = writable_notes();
            let preparation = Preparation::default();
            preparation.lock().phase = Phase::Running;
            let foreground = Arc::clone(&owner);
            crate::fault::arm(
                prepare::fault_names::PARTITION_BEFORE_COMPUTE,
                0,
                crate::fault::Action::Call(Box::new(move |_| {
                    supersede(&foreground.engine.lock().unwrap(), "note00.md", delete);
                })),
            );
            let control = Control::unbounded();
            let mut run = Run::new(&*owner, &runtime, &preparation, &control);
            let wave = run.partition_wave(&mut prepare::SourceCursor::default());
            crate::fault::disarm_all();
            let (progress, retained) = wave.unwrap();
            assert!(!matches!(progress, Progress::Halted(_)));
            assert_eq!(run.report.partitioned_sources, 7);
            assert_eq!(retained.len(), 7);
            let engine = owner.engine.lock().unwrap();
            assert!(engine.semantic_partition("note00.md").unwrap().is_none());
            for n in 1..8 {
                let path = format!("note{n:02}.md");
                assert_eq!(
                    engine
                        .semantic_partition(&path)
                        .unwrap()
                        .unwrap()
                        .source_hash,
                    engine.source(&path).unwrap().unwrap().hash
                );
            }
        }
    }

    /// FOLLOWUP F13: each commit rechecks its own source, so a foreground
    /// write between two commits of one wave supersedes only the later
    /// source, and the wave goes on.
    #[test]
    fn a_source_changed_between_partition_commits_is_superseded() {
        let (_dir, owner, runtime) = writable_notes();
        let preparation = Preparation::default();
        preparation.lock().phase = Phase::Running;
        let foreground = Arc::clone(&owner);
        let committed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&committed);
        crate::fault::arm(
            prepare::fault_names::PARTITION_AFTER_COMMIT,
            0,
            crate::fault::Action::Call(Box::new(move |ctx| {
                seen.lock().unwrap().push(ctx.detail.to_owned());
                if ctx.detail == "note00.md" {
                    foreground
                        .queued
                        .lock()
                        .unwrap()
                        .push(Box::new(|engine: &Engine| {
                            supersede(engine, "note01.md", false);
                        }));
                }
            })),
        );
        let control = Control::unbounded();
        let mut run = Run::new(&*owner, &runtime, &preparation, &control);
        let wave = run.partition_wave(&mut prepare::SourceCursor::default());
        crate::fault::disarm_all();
        let (progress, retained) = wave.unwrap();
        assert!(!matches!(progress, Progress::Halted(_)));
        let expected: Vec<_> = (0..8)
            .filter(|&n| n != 1)
            .map(|n| format!("note{n:02}.md"))
            .collect();
        assert_eq!(*committed.lock().unwrap(), expected);
        assert_eq!(run.report.partitioned_sources, 7);
        assert_eq!(retained.len(), 7);
        let engine = owner.engine.lock().unwrap();
        assert!(engine.semantic_partition("note01.md").unwrap().is_none());
        for path in &expected {
            assert!(engine.semantic_partition(path).unwrap().is_some());
        }
    }

    /// FOLLOWUP F13: continuous maintenance converges after a source changes
    /// or is deleted while its partition is computed. The superseded
    /// version's obligation stays pending, a later batch derives the current
    /// version, and the driver waits caught up with nothing pending.
    #[test]
    fn maintenance_converges_after_a_source_changes_during_partition_compute() {
        for delete in [false, true] {
            let (_dir, owner, runtime) = writable_notes();
            let preparation = Arc::new(Preparation::default());
            {
                let mut state = preparation.lock();
                state.phase = Phase::Running;
                state.continuous = true;
            }
            let foreground = Arc::clone(&owner);
            let changed = std::cell::Cell::new(false);
            crate::fault::arm(
                prepare::fault_names::PARTITION_BEFORE_COMPUTE,
                0,
                crate::fault::Action::Call(Box::new(move |_| {
                    if !changed.replace(true) {
                        supersede(&foreground.engine.lock().unwrap(), "note00.md", delete);
                    }
                })),
            );
            // Once the driver waits caught up, hand control back to the test.
            let finished = Arc::new(AtomicBool::new(false));
            let watcher = {
                let (preparation, finished) = (Arc::clone(&preparation), Arc::clone(&finished));
                std::thread::spawn(move || {
                    let deadline = Instant::now() + Duration::from_secs(60);
                    while !finished.load(Ordering::SeqCst) {
                        let waiting = preparation.live() == Some(Live::Waiting);
                        if waiting || Instant::now() >= deadline {
                            preparation.lock().continuous = false;
                            return waiting;
                        }
                        std::thread::sleep(POLL);
                    }
                    false
                })
            };
            let control = Control::unbounded();
            let mut run = Run::new(&*owner, &runtime, &preparation, &control);
            let outcome = run.begin().and_then(|()| run.maintain());
            finished.store(true, Ordering::SeqCst);
            crate::fault::disarm_all();
            let waited = watcher.join().unwrap();
            assert!(outcome.unwrap().is_none());
            assert!(waited, "maintenance never waited caught up");
            let engine = owner.engine.lock().unwrap();
            let pending = engine
                .derivation_pending(crate::neural::work::Lane::Semantic)
                .unwrap();
            assert_eq!(pending.paths, 0);
            assert!(!pending.reconcile_required);
            let partition = engine.semantic_partition("note00.md").unwrap();
            match engine.source("note00.md").unwrap() {
                None => assert!(delete && partition.is_none()),
                Some(source) => {
                    assert!(!delete);
                    assert_eq!(source.hash, crate::digest(b"# Changed note\n\nnew body\n"));
                    assert_eq!(partition.unwrap().source_hash, source.hash);
                }
            }
            for n in 1..12 {
                let path = format!("note{n:02}.md");
                assert!(engine.semantic_partition(&path).unwrap().is_some());
            }
        }
    }

    /// FOLLOWUP F13: a one-shot pass completes too. The superseding write
    /// moved the source revision, so a further pass derives the current
    /// version.
    #[test]
    fn a_one_shot_pass_completes_after_a_source_changes_during_partition_compute() {
        for delete in [false, true] {
            let (_dir, owner, runtime) = writable_notes();
            let preparation = Preparation::default();
            preparation.lock().phase = Phase::Running;
            let foreground = Arc::clone(&owner);
            let changed = std::cell::Cell::new(false);
            crate::fault::arm(
                prepare::fault_names::PARTITION_BEFORE_COMPUTE,
                0,
                crate::fault::Action::Call(Box::new(move |_| {
                    if !changed.replace(true) {
                        supersede(&foreground.engine.lock().unwrap(), "note00.md", delete);
                    }
                })),
            );
            let control = Control::unbounded();
            let mut run = Run::new(&*owner, &runtime, &preparation, &control);
            let outcome = run.begin().and_then(|()| run.one_shot());
            crate::fault::disarm_all();
            assert!(matches!(outcome.unwrap(), Stop::Complete));
            let engine = owner.engine.lock().unwrap();
            let partition = engine.semantic_partition("note00.md").unwrap();
            match engine.source("note00.md").unwrap() {
                None => assert!(delete && partition.is_none()),
                Some(source) => {
                    assert!(!delete);
                    assert_eq!(partition.unwrap().source_hash, source.hash);
                }
            }
            for n in 1..12 {
                let path = format!("note{n:02}.md");
                assert!(engine.semantic_partition(&path).unwrap().is_some());
            }
        }
    }

    /// Embeds unit vectors. A write armed in `armed` runs on the next document
    /// call, which holds no engine slot, as a foreground write would.
    struct Interleaved {
        descriptor: FunctionDescriptor,
        owner: Arc<Writes>,
        armed: Arc<Mutex<Option<Write>>>,
    }

    impl EmbeddingProvider for Interleaved {
        fn descriptor(&self) -> &FunctionDescriptor {
            &self.descriptor
        }
        fn embed_documents(
            &mut self,
            batch: &[TokenizedInput],
            _control: &Control,
        ) -> Result<Vec<Vec<f32>>, ProviderError> {
            if let Some(write) = self.armed.lock().unwrap().take() {
                write(&self.owner.engine.lock().unwrap());
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

    /// Put a brief over two notes whose purpose is `text`; its id and revision.
    fn put_brief(
        engine: &Engine,
        notes: [usize; 2],
        text: &str,
        expected: Option<u64>,
    ) -> (String, u64) {
        use serde_json::json;
        let wid = engine.workspace_id().unwrap();
        let spans: Vec<_> = notes
            .iter()
            .map(|n| {
                let source = format!("# Note {n}\n\nbody of note {n}\n");
                format!(
                    "note{n:02}.md#0-{}@{}.{}",
                    source.len(),
                    &crate::digest(source.as_bytes())[..32],
                    &wid[..16]
                )
            })
            .collect();
        let input = json!({"cited_spans":spans,"recipe_sha256":"a".repeat(64),"model":"fixture",
            "artifact_sha256":"b".repeat(64),"prompt_sha256":crate::digest(text.as_bytes()),"generated_at_unix_ms":1,
            "purpose":[{"text":text,"citations":[0,1],"identifiers":[]}],
            "behavior":[{"text":"Retain both notes.","citations":[0,1],"identifiers":[]}],
            "provides":[],"consumes":[],"constraints":[],"tests":[],"unknowns":[]});
        let request = crate::brief::parse(&serde_json::to_vec(&json!({"workspace_id":wid,"command":{"op":"put","brief":input,"expected_revision":expected}})).unwrap()).unwrap();
        let result = engine.brief(&request, &Control::unbounded()).unwrap();
        (
            result["id"].as_str().unwrap().to_owned(),
            result["revision"].as_u64().unwrap(),
        )
    }

    /// How a test supersedes an area while its model call runs.
    #[derive(Clone, Copy, PartialEq)]
    enum AreaChange {
        /// A new revision of its brief.
        Brief,
        /// Its brief forgotten.
        Forget,
        /// One of its brief's sources replaced, so the brief is no longer
        /// eligible.
        Source,
    }

    /// Twelve notes prepared under a hierarchy profile, then two briefs over
    /// notes 0–1 and 2–3, not yet embedded as areas. The armed write
    /// supersedes the area the pass visits first (briefs go in id order); the
    /// briefs come back in that order.
    struct Areas {
        _dir: tempfile::TempDir,
        owner: Arc<Writes>,
        runtime: QueryRuntime,
        preparation: Arc<Preparation>,
        briefs: [String; 2],
    }

    fn areas(change: AreaChange) -> Areas {
        let (dir, engine, mut profile) = notes();
        Arc::make_mut(&mut profile).representation =
            super::super::partition::Representation::Hierarchy { parent_tokens: 32 };
        Arc::make_mut(&mut profile).card_tokens = 512;
        let owner = Arc::new(Writes {
            engine: Mutex::new(engine),
            queued: Mutex::new(Vec::new()),
        });
        let armed = Arc::new(Mutex::new(None));
        let provider = Interleaved {
            descriptor: profile.descriptor.clone(),
            owner: Arc::clone(&owner),
            armed: Arc::clone(&armed),
        };
        let runtime = QueryRuntime::start(
            profile,
            Box::new(move || Ok(Box::new(provider) as Box<dyn EmbeddingProvider>)),
        )
        .unwrap();
        let preparation = Arc::new(Preparation::default());
        preparation.lock().phase = Phase::Running;
        let control = Control::unbounded();
        let mut run = Run::new(&*owner, &runtime, &preparation, &control);
        let prepared = run.begin().and_then(|()| run.one_shot()).unwrap();
        assert!(matches!(prepared, Stop::Complete));
        let mut briefs = {
            let engine = owner.engine.lock().unwrap();
            [[0, 1], [2, 3]].map(|notes| {
                let (id, revision) = put_brief(&engine, notes, "Explain the note pair.", None);
                (id, revision, notes)
            })
        };
        briefs.sort();
        let (first, revision, notes) = briefs[0].clone();
        *armed.lock().unwrap() = Some(Box::new(move |engine: &Engine| match change {
            AreaChange::Brief => {
                put_brief(
                    engine,
                    notes,
                    "Explain the revised note pair.",
                    Some(revision),
                );
            }
            AreaChange::Forget => {
                let request = crate::brief::parse(
                    &serde_json::to_vec(&serde_json::json!({
                        "workspace_id": engine.workspace_id().unwrap(),
                        "command": {"op":"forget","id":first,"expected_revision":revision},
                    }))
                    .unwrap(),
                )
                .unwrap();
                engine.brief(&request, &Control::unbounded()).unwrap();
            }
            AreaChange::Source => {
                supersede(engine, &format!("note{:02}.md", notes[0]), false);
            }
        }));
        Areas {
            _dir: dir,
            owner,
            runtime,
            preparation,
            briefs: briefs.map(|(id, _, _)| id),
        }
    }

    impl Areas {
        /// The briefs whose current area is bound, in id order.
        fn bound(&self) -> Vec<String> {
            let engine = self.owner.engine.lock().unwrap();
            let profile = &self.runtime.profile;
            let mut ids: Vec<_> = super::super::area::search(
                &engine,
                profile,
                &unit_vector(&profile.descriptor),
                &Control::unbounded(),
            )
            .unwrap()
            .into_iter()
            .map(|candidate| candidate.id)
            .collect();
            ids.sort();
            ids
        }
    }

    const AREA_CHANGES: [AreaChange; 3] =
        [AreaChange::Brief, AreaChange::Forget, AreaChange::Source];

    /// FOLLOWUP F13: an area whose brief changes, is forgotten, or loses a
    /// source while its model call runs is superseded, not a stop. The pass
    /// skips it and binds the next area.
    #[test]
    fn an_area_changed_during_its_model_call_is_superseded() {
        for change in AREA_CHANGES {
            let areas = areas(change);
            let control = Control::unbounded();
            let mut run = Run::new(&*areas.owner, &areas.runtime, &areas.preparation, &control);
            assert!(run.area_pass().unwrap().is_none());
            assert_eq!(run.report.area_vectors, 1);
            assert_eq!(areas.bound(), [areas.briefs[1].clone()]);
        }
    }

    /// FOLLOWUP F13: continuous maintenance converges after an area is
    /// superseded. The change keeps its own obligation (a brief write marks
    /// overviews pending, a source write its path), so a later batch binds
    /// the current area or drops it, and the driver waits caught up.
    #[test]
    fn maintenance_converges_after_an_area_changes_during_its_model_call() {
        for change in AREA_CHANGES {
            let areas = areas(change);
            areas.preparation.lock().continuous = true;
            let finished = Arc::new(AtomicBool::new(false));
            let watcher = {
                let (preparation, finished) =
                    (Arc::clone(&areas.preparation), Arc::clone(&finished));
                std::thread::spawn(move || {
                    let deadline = Instant::now() + Duration::from_secs(60);
                    while !finished.load(Ordering::SeqCst) {
                        let waiting = preparation.live() == Some(Live::Waiting);
                        if waiting || Instant::now() >= deadline {
                            preparation.lock().continuous = false;
                            return waiting;
                        }
                        std::thread::sleep(POLL);
                    }
                    false
                })
            };
            let control = Control::unbounded();
            let mut run = Run::new(&*areas.owner, &areas.runtime, &areas.preparation, &control);
            let outcome = run.begin().and_then(|()| run.maintain());
            finished.store(true, Ordering::SeqCst);
            let waited = watcher.join().unwrap();
            assert!(outcome.unwrap().is_none());
            assert!(waited, "maintenance never waited caught up");
            let pending = areas
                .owner
                .engine
                .lock()
                .unwrap()
                .derivation_pending(crate::neural::work::Lane::Semantic)
                .unwrap();
            assert_eq!(pending.paths, 0);
            assert!(!pending.overviews_pending && !pending.reconcile_required);
            let expected = if change == AreaChange::Brief {
                areas.briefs.to_vec()
            } else {
                vec![areas.briefs[1].clone()]
            };
            assert_eq!(areas.bound(), expected);
        }
    }

    /// FOLLOWUP F13: a one-shot pass completes after an area is superseded.
    /// A changed or forgotten brief stays pending as an overview obligation;
    /// a replaced source moved the source revision, so a further pass ran.
    #[test]
    fn a_one_shot_pass_completes_after_an_area_changes_during_its_model_call() {
        for change in AREA_CHANGES {
            let areas = areas(change);
            let control = Control::unbounded();
            let mut run = Run::new(&*areas.owner, &areas.runtime, &areas.preparation, &control);
            let outcome = run.begin().and_then(|()| run.one_shot());
            assert!(matches!(outcome.unwrap(), Stop::Complete));
            assert_eq!(areas.bound(), [areas.briefs[1].clone()]);
            let pending = areas
                .owner
                .engine
                .lock()
                .unwrap()
                .derivation_pending(crate::neural::work::Lane::Semantic)
                .unwrap();
            assert_eq!(pending.overviews_pending, change != AreaChange::Source);
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
    /// driver's prefetch and can be closed. With `fail_prefetch`, the first
    /// prefetch's selection step meets one injected store failure: armed on
    /// the driver thread at the first admission, disarmed when that prefetch
    /// is reported.
    struct Counted {
        engine: Mutex<Engine>,
        in_flight: AtomicUsize,
        closing: AtomicBool,
        prefetched: Mutex<mpsc::Sender<usize>>,
        fail_prefetch: AtomicBool,
        armed: AtomicBool,
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
        fn admitting(&self) {
            if self.fail_prefetch.swap(false, Ordering::SeqCst) {
                crate::fault::arm(
                    crate::neural::fault_names::PARTITION_READ,
                    0,
                    crate::fault::Action::Fail("injected store failure".into()),
                );
                self.armed.store(true, Ordering::SeqCst);
            }
        }
        fn prefetched(&self, inputs: usize) {
            if self.armed.swap(false, Ordering::SeqCst) {
                crate::fault::arm(
                    crate::neural::fault_names::PARTITION_READ,
                    0,
                    crate::fault::Action::Call(Box::new(|_| {})),
                );
            }
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
            let fixture = Self::start(false);
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

        /// Start the driver over twelve notes behind a gated provider.
        fn start(fail_prefetch: bool) -> Self {
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
                fail_prefetch: AtomicBool::new(fail_prefetch),
                armed: AtomicBool::new(false),
            });
            let preparation = Arc::new(Preparation::default());
            preparation
                .prepare(Arc::clone(&owner) as Arc<dyn Owner>, Arc::clone(&runtime))
                .unwrap();
            Self {
                _dir: dir,
                owner,
                runtime,
                preparation,
                calls,
                entered,
                release,
                prefetched,
            }
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

    /// 009 T004 M3: a prefetch whose selection step fails stops the run with
    /// that error once the in-flight batch is committed, and that committed
    /// batch is published at the stop: cached AND searchable.
    #[test]
    fn a_failed_prefetch_still_publishes_the_batch_committed_before_it() {
        let fixture = Prefetch::start(true);
        assert_eq!(fixture.entered(), 8, "the first batch runs");
        assert_eq!(
            fixture
                .prefetched
                .recv_timeout(Duration::from_secs(30))
                .expect("the prefetch was attempted during the call"),
            0,
            "its selection step failed"
        );
        fixture.release.send(()).unwrap();
        until_idle(&fixture.preparation);
        fixture.runtime.shutdown();
        assert_eq!(fixture.calls(), ["documents 8"]);
        let state = fixture.state();
        assert_eq!(state.state, "paused", "{state:?}");
        assert_eq!(state.committed_units, 8, "{state:?}");
        assert!(
            state
                .last_error
                .as_ref()
                .is_some_and(|error| error.message.contains("injected store failure")),
            "{state:?}"
        );
        let status = locked(&fixture.owner.engine)
            .semantic_status(&Control::unbounded())
            .unwrap();
        assert_eq!(status.cache.entries, 8, "{status:?}");
        assert_eq!(status.cached_current_units, 8, "{status:?}");
        assert_eq!(status.searchable_current_units, 8, "{status:?}");
        assert!(status.index.available, "{status:?}");
    }
}
