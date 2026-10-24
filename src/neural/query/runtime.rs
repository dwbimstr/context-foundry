//! The provider-thread runtime: the one model slot shared by query embeds,
//! document batches and knowledge calls, its admission and ceilings, and the
//! dense-index cache it serves, with their tests. The dense index and the
//! refinement stay in the parent module.
use super::*;

/// One job for the provider thread: a query embedding or (009 T003) one
/// document batch of the owner's preparation driver. Both kinds share the
/// ONE admission slot.
enum Job {
    Query {
        input: TokenizedInput,
        deadline: Instant,
        reply: mpsc::Sender<Result<Vec<f32>, ProviderError>>,
    },
    Documents {
        inputs: Vec<TokenizedInput>,
        control: Control,
        reply: mpsc::Sender<Result<Vec<Vec<f32>>, ProviderError>>,
    },
}

/// Who holds the one model slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Holder {
    Query,
    /// One document batch of this owner's preparation driver.
    Documents,
    Knowledge,
}

/// The ONE model slot, shared with the provider thread.
#[derive(Default)]
struct Slot {
    state: Mutex<SlotState>,
    /// Signalled whenever the slot is freed.
    freed: Condvar,
}

#[derive(Default)]
struct SlotState {
    /// Set from the dispatch of a query or document batch until the
    /// provider thread finished it: a request whose caller timed out keeps
    /// the slot until its late reply is dropped, exactly like the supervised
    /// worker.
    holder: Option<Holder>,
    /// 009 T003: one query waits for the document batch holding the slot
    /// to end. The slot is promised to it: every other query and every
    /// document admission is refused meanwhile.
    waiter: bool,
    /// The last terminal failure a provider call returned (the worker
    /// exited or was stopped at its memory ceiling), cleared by a later
    /// call that succeeded: the status word, read without a model call.
    failure: Option<&'static str>,
}

impl Slot {
    fn lock(&self) -> MutexGuard<'_, SlotState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Free the slot after a provider call ended with `outcome`, and wake the
    /// waiting query, if any.
    fn free_after<T>(&self, outcome: &Result<T, ProviderError>) {
        let mut state = self.lock();
        state.holder = None;
        match outcome {
            Ok(_) => state.failure = None,
            Err(error @ (ProviderError::WorkerExited(_) | ProviderError::ResourceLimit(_))) => {
                state.failure = Some(error.code());
            }
            Err(_) => {}
        }
        drop(state);
        self.freed.notify_all();
    }

    /// Free the slot claimed for a call that never reached the provider.
    fn free(&self) {
        self.lock().holder = None;
        self.freed.notify_all();
    }
}

/// One admitted document call. Waiting is sliced so the caller can notice
/// its own stop; dropping the call discards the late reply, and the slot
/// stays held until the provider call actually ends.
pub struct DocumentCall {
    answer: mpsc::Receiver<Result<Vec<Vec<f32>>, ProviderError>>,
}

impl DocumentCall {
    /// The call's outcome if it ended within `slice`; `None` while it runs.
    pub fn wait(&self, slice: Duration) -> Option<Result<Vec<Vec<f32>>, ProviderError>> {
        match self.answer.recv_timeout(slice) {
            Ok(result) => Some(result),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => Some(Err(ProviderError::WorkerExited(
                "the provider thread ended during the document call".into(),
            ))),
        }
    }
}

/// A configured HTTP generation request shares the existing model admission
/// slot. Dropping it releases client admission; external compute may outlive HTTP.
pub(crate) struct KnowledgeCall {
    slot: Arc<Slot>,
}
impl Drop for KnowledgeCall {
    fn drop(&mut self) {
        self.slot.free();
    }
}

/// Counts a foreground query from the start of its request path until it
/// returns, so no document batch is admitted while it is being dispatched.
/// Registration takes the runtime's admission lock, so it and a document
/// admission decision are one ordered decision, never interleaved.
struct Dispatching<'a>(&'a AtomicUsize);

impl<'a> Dispatching<'a> {
    fn enter(runtime: &'a QueryRuntime) -> Self {
        let _admission = runtime.admission();
        runtime.dispatching.fetch_add(1, Ordering::SeqCst);
        Self(&runtime.dispatching)
    }
}

impl Drop for Dispatching<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The builder of the provider, run ON the provider thread: the provider is
/// created, used and dropped on that one thread, so it needs no `Send`
/// bound. Dropping the runtime closes the channel, ends the thread and
/// drops the provider (the supervised worker shuts down);
/// [`QueryRuntime::shutdown`] does the same and waits until it happened.
pub type MakeProvider =
    Box<dyn FnOnce() -> Result<Box<dyn EmbeddingProvider>, ProviderError> + Send>;

/// The resident runtime one owner (MCP) or one command (CLI) holds: the
/// verified profile, its exact-input tokenizer, the embedding provider
/// (confined to its own thread, ONE admission slot for queries and document
/// batches alike) and the loaded dense generation. Owner/profile/manifest checks
/// refresh generation metadata; the Engine retains verified immutable ANN segments
/// across refreshes. Source edits are caught by the final source read.
pub struct QueryRuntime {
    pub profile: Arc<SemanticProfile>,
    tokenizer: DocumentTokenizer,
    /// `None` once [`Self::shutdown`] closed it.
    jobs: Mutex<Option<mpsc::Sender<Job>>>,
    /// The provider thread, joined by [`Self::shutdown`].
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// The one model slot for queries and document batches; the provider
    /// thread frees it only when the provider call really ended.
    slot: Arc<Slot>,
    /// Foreground queries currently in their request path.
    dispatching: AtomicUsize,
    /// Orders query registration and each document admission decision; it
    /// is held for neither inference nor any wait.
    admission: Mutex<()>,
    /// The provider's probe of a call it already returned from that still
    /// runs in the model (the supervised worker's abandoned query).
    late: Option<LateCall>,
    /// The function digest of the provider's own descriptor.
    served_digest: String,
    index: Mutex<Option<Arc<DenseIndex>>>,
    /// The cache cap the owner's preparation runs under.
    pub(crate) cache_cap_bytes: u64,
}

impl QueryRuntime {
    /// Acquire the supervised worker (the profile is verified and the worker
    /// started, bounded by the profile's load timeout, before anything
    /// serves), then load the exact-input tokenizer. `Err` is the named
    /// reason the baseline path reports. The supervised worker exists only
    /// on macOS; elsewhere this is `isolation_unavailable`.
    pub fn acquire(
        profile_path: &std::path::Path,
        development: bool,
    ) -> Result<Self, ProviderError> {
        let profile = Arc::new(SemanticProfile::load(profile_path)?);
        Self::acquire_profile(profile, development, None)
    }

    pub(crate) fn acquire_profile(
        profile: Arc<SemanticProfile>,
        development: bool,
        reservation: Option<crate::model_resources::Reservation>,
    ) -> Result<Self, ProviderError> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let control = Control::unbounded()
                .bounded_by(Instant::now() + Duration::from_secs(profile.load_timeout_seconds));
            let artifacts = Arc::new(profile.verified_artifacts_until(&control)?);
            let tokenizer = DocumentTokenizer::load_verified(&profile, &artifacts)?;
            let for_worker = Arc::clone(&profile);
            Self::start_tokenized(
                profile,
                Box::new(move || {
                    crate::neural::supervisor::acquire_verified_until(
                        &for_worker,
                        &artifacts,
                        development,
                        &control,
                    )
                }),
                tokenizer,
                reservation,
            )
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = (profile, development, reservation);
            Err(ProviderError::IsolationUnavailable(
                "no accepted isolation profile admits model execution on this platform".into(),
            ))
        }
    }

    /// Start a runtime over `make` (the supervised worker, or a test
    /// provider). Blocks until the provider is built or refused.
    pub fn start(profile: Arc<SemanticProfile>, make: MakeProvider) -> Result<Self, ProviderError> {
        Self::start_reserved(profile, make, None)
    }

    pub(crate) fn start_reserved(
        profile: Arc<SemanticProfile>,
        make: MakeProvider,
        reservation: Option<crate::model_resources::Reservation>,
    ) -> Result<Self, ProviderError> {
        let tokenizer = DocumentTokenizer::load(&profile)?;
        Self::start_tokenized(profile, make, tokenizer, reservation)
    }

    fn start_tokenized(
        profile: Arc<SemanticProfile>,
        make: MakeProvider,
        tokenizer: DocumentTokenizer,
        reservation: Option<crate::model_resources::Reservation>,
    ) -> Result<Self, ProviderError> {
        let (jobs_tx, jobs_rx) = mpsc::channel::<Job>();
        let (ready_tx, ready_rx) =
            mpsc::channel::<Result<(String, Option<LateCall>), ProviderError>>();
        let slot = Arc::new(Slot::default());
        let thread_slot = Arc::clone(&slot);
        let thread = std::thread::Builder::new()
            .name("foundry-query-provider".into())
            .spawn(move || {
                // Declared before the provider: released only AFTER its drop
                // stops/reaps the process, even if the owner abandons a call.
                let _reservation = reservation;
                let mut provider = match make() {
                    Ok(provider) => {
                        let _ = ready_tx
                            .send(Ok((provider.descriptor().digest(), provider.late_call())));
                        provider
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };
                // Free the slot BEFORE replying: a late reply nobody waits
                // for still releases it, and only when the call really ended.
                while let Ok(job) = jobs_rx.recv() {
                    match job {
                        Job::Query {
                            input,
                            deadline,
                            reply,
                        } => {
                            let result = provider.embed_query(&input, deadline);
                            thread_slot.free_after(&result);
                            let _ = reply.send(result);
                        }
                        Job::Documents {
                            inputs,
                            control,
                            reply,
                        } => {
                            let result = provider.embed_documents(&inputs, &control);
                            thread_slot.free_after(&result);
                            let _ = reply.send(result);
                        }
                    }
                }
            })
            .map_err(|e| ProviderError::WorkerExited(format!("provider thread: {e}")))?;
        let (served_digest, late) = match ready_rx.recv() {
            Ok(Ok(ready)) => ready,
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                return Err(ProviderError::WorkerExited(
                    "the provider thread ended before it was ready".into(),
                ));
            }
        };
        Ok(Self {
            profile,
            tokenizer,
            jobs: Mutex::new(Some(jobs_tx)),
            thread: Mutex::new(Some(thread)),
            slot,
            dispatching: AtomicUsize::new(0),
            admission: Mutex::new(()),
            late,
            served_digest,
            index: Mutex::new(None),
            cache_cap_bytes: crate::neural::cache::DEFAULT_CACHE_CAP_BYTES,
        })
    }

    /// The owner's preparation runs under `bytes` of cache instead of the
    /// default (`mcp --semantic-cache-cap`).
    pub fn with_cache_cap(mut self, bytes: u64) -> Self {
        self.cache_cap_bytes = bytes;
        self
    }

    fn admission(&self) -> MutexGuard<'_, ()> {
        self.admission
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// True while a call the provider already returned from still runs.
    fn late_call_running(&self) -> bool {
        self.late.as_ref().is_some_and(|late| late())
    }

    /// Hand one job to the provider thread; `false` once it is gone.
    fn send(&self, job: Job) -> bool {
        let jobs = self.jobs.lock().unwrap_or_else(PoisonError::into_inner);
        jobs.as_ref().is_some_and(|jobs| jobs.send(job).is_ok())
    }

    /// Stop the runtime: no new job is accepted; the provider thread finishes
    /// its current call, drops the provider — the supervised worker is
    /// stopped and reaped — and is joined. Blocks until then.
    pub fn shutdown(&self) {
        self.jobs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let thread = self
            .thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(thread) = thread {
            let _ = thread.join();
        }
    }

    /// The query embedding under the ceiling derived from the request's
    /// shared read `deadline`: min(1500 ms, HALF the time left to it), so a
    /// cut-off embedding leaves the baseline fallback at least as much time
    /// as it took. The wait itself is cut at the ceiling whatever the
    /// provider does, so a stalled provider cannot hold the request past
    /// it; the provider slot stays busy until its call really ends, and a
    /// call the provider already gave up on (its late call) keeps it busy too.
    /// 009 T003: a slot held by this owner's document batch is waited for,
    /// by one query at a time, up to that same ceiling
    /// ([`Self::claim_query_slot`]).
    pub fn embed(&self, query: &str, deadline: Instant) -> Result<Vec<f32>, ProviderError> {
        let _dispatching = Dispatching::enter(self);
        let _ = neural_fault!(QUERY_REGISTERED, None, query);
        let input = tokenize_query_with_limit(
            &self.tokenizer,
            &self.profile.query_template,
            query,
            self.profile.descriptor.input_limits().tokens,
        )?;
        let now = Instant::now();
        let ceiling = now + QUERY_CEILING.min(deadline.saturating_duration_since(now) / 2);
        if Instant::now() >= ceiling {
            return Err(ProviderError::Timeout);
        }
        self.claim_query_slot(ceiling)?;
        let (reply, answer) = mpsc::channel();
        let sent = self.send(Job::Query {
            input,
            deadline: ceiling,
            reply,
        });
        if !sent {
            self.slot.free();
            return Err(ProviderError::WorkerExited(
                "the provider thread is gone".into(),
            ));
        }
        match answer.recv_timeout(ceiling.saturating_duration_since(Instant::now())) {
            Ok(result) => result.and_then(|vector| {
                provider::validate_vector(&vector, self.profile.descriptor.dims())?;
                Ok(vector)
            }),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ProviderError::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ProviderError::WorkerExited(
                "the provider thread ended during the query".into(),
            )),
        }
    }

    /// 009 T003: admit ONE document batch (at most the profile's batch of
    /// inputs, limits checked first) on the same slot as queries, with the
    /// caller's `control` (its deadline and cancellation reach the provider).
    /// Refused `Busy` — nothing is queued — while the slot is held by a call
    /// that has not actually ended (the provider's late call included), is
    /// promised to a query waiting for it, or while a foreground query is
    /// being dispatched. The decision is taken under the admission lock that
    /// query registration takes too: a query registered before it always
    /// wins.
    pub fn dispatch_documents(
        &self,
        inputs: Vec<TokenizedInput>,
        control: Control,
    ) -> Result<DocumentCall, ProviderError> {
        provider::check_document_batch(&inputs, self.profile.document_limits())?;
        let _ = neural_fault!(DOCUMENT_ADMISSION, Some(&control), "");
        self.claim_background(Holder::Documents, "documents")?;
        let (reply, answer) = mpsc::channel();
        let sent = self.send(Job::Documents {
            inputs,
            control,
            reply,
        });
        if !sent {
            self.slot.free();
            return Err(ProviderError::WorkerExited(
                "the provider thread is gone".into(),
            ));
        }
        Ok(DocumentCall { answer })
    }

    pub(crate) fn admit_knowledge(&self) -> Result<KnowledgeCall, ProviderError> {
        self.claim_background(Holder::Knowledge, "knowledge")?;
        Ok(KnowledgeCall {
            slot: self.slot.clone(),
        })
    }

    /// Claim a free slot for a background `holder` (a document batch or a
    /// knowledge call), never waiting, under the admission lock query
    /// registration takes too: `Busy` while a foreground query is being
    /// dispatched, or while the slot is held or promised to a waiting
    /// query. The slot FIRST, then the provider's late call
    /// ([`Self::probe_late_call`]), both under that lock. Nothing is created
    /// or sent on a refusal.
    fn claim_background(&self, holder: Holder, detail: &str) -> Result<(), ProviderError> {
        let _admission = self.admission();
        if self.dispatching.load(Ordering::SeqCst) > 0 {
            return Err(ProviderError::Busy);
        }
        {
            let mut slot = self.slot.lock();
            if slot.holder.is_some() || slot.waiter {
                return Err(ProviderError::Busy);
            }
            slot.holder = Some(holder);
        }
        self.probe_late_call(detail)
    }

    /// Claim the slot for a query, then probe the provider's late call. A
    /// free slot is claimed at once. 009 T003: a slot held by this owner's
    /// document batch may be waited for until `ceiling`, by ONE query at a
    /// time. The batch's end frees the slot under the lock the waiter
    /// claims it under, and while it waits every other query and every
    /// document admission is refused, so nothing is admitted in between. A
    /// ceiling that ends first is `Busy`, as is a slot held by a query or
    /// already promised to a waiter. The worker still runs one call and
    /// queues none: this wait is the owner's, bounded by the request's own
    /// ceiling.
    fn claim_query_slot(&self, ceiling: Instant) -> Result<(), ProviderError> {
        {
            let mut slot = self.slot.lock();
            match (slot.holder, slot.waiter) {
                (None, false) => {}
                (Some(Holder::Documents | Holder::Knowledge), false) => {
                    slot.waiter = true;
                    // The ceiling is read again after every reacquisition and
                    // right before the claim: a batch that ended after the
                    // ceiling, while this waiter was still reacquiring the
                    // lock, is never claimed.
                    loop {
                        let left = ceiling.saturating_duration_since(Instant::now());
                        if left.is_zero() {
                            slot.waiter = false;
                            return Err(ProviderError::Busy);
                        }
                        if slot.holder.is_none() {
                            break;
                        }
                        slot = self
                            .slot
                            .freed
                            .wait_timeout(slot, left)
                            .unwrap_or_else(PoisonError::into_inner)
                            .0;
                    }
                    slot.waiter = false;
                }
                _ => return Err(ProviderError::Busy),
            }
            slot.holder = Some(Holder::Query);
        }
        self.probe_late_call("query")
    }

    /// The second half of every claim: the provider's late call. The
    /// provider thread frees the slot only after the provider returned, and
    /// the supervisor marks a call abandoned before it returns, so once a
    /// claim succeeded any late call is already visible; a claim that finds
    /// one is released at once.
    fn probe_late_call(
        &self,
        #[cfg_attr(not(feature = "test-faults"), allow(unused_variables))] detail: &str,
    ) -> Result<(), ProviderError> {
        let _ = neural_fault!(SLOT_CLAIMED, None, detail);
        if self.late_call_running() {
            self.slot.free();
            return Err(ProviderError::Busy);
        }
        Ok(())
    }

    /// An external generation request owns the client admission slot. The
    /// semantic driver can start and wait on shared background admission.
    pub(crate) fn knowledge_occupied(&self) -> bool {
        self.slot.lock().holder == Some(Holder::Knowledge)
    }

    /// True while the one model slot is held by a call that has not actually
    /// ended (the provider's late call included), or a foreground query is
    /// being dispatched. The slot is read before the late call, in the order
    /// the provider thread publishes them.
    pub fn occupied(&self) -> bool {
        let held = self.slot.lock().holder.is_some();
        held || self.dispatching.load(Ordering::SeqCst) > 0 || self.late_call_running()
    }

    /// The resident runtime's status word, read without a model call:
    /// `ready`, or `fallback:<code>` after a provider call returned a
    /// terminal failure (`provider_exited`, `resource_limit`) and no call has
    /// succeeded since.
    pub fn status_word(&self) -> String {
        match self.slot.lock().failure {
            None => "ready".to_owned(),
            Some(code) => fallback_word(code),
        }
    }

    /// The function digest of the descriptor the provider itself serves.
    pub fn served_digest(&self) -> &str {
        &self.served_digest
    }

    /// The profile's exact-input tokenizer (the preparation driver's too).
    pub(crate) fn tokenizer(&self) -> &DocumentTokenizer {
        &self.tokenizer
    }

    /// Refresh generation metadata after publication. The Engine retains at
    /// most two verified immutable segments for reuse by the next request.
    pub fn forget_index(&self) {
        if let Ok(mut slot) = self.index.lock() {
            *slot = None;
        }
    }

    /// Reuse the generation while its owner, prepared identity and manifest
    /// version remain current. A store prepared for another profile is refused
    /// by name HERE, before any query embedding is spent.
    fn dense_index(&self, engine: &Engine, control: &Control) -> Result<Arc<DenseIndex>, Fallback> {
        let mut slot = self
            .index
            .lock()
            .map_err(|_| Fallback::new("internal", "the dense index lock was poisoned"))?;
        control.check()?;
        if let Some(index) = slot.as_ref()
            && index
                .owner
                .as_ref()
                .is_some_and(|owner| Arc::ptr_eq(owner, &engine.semantic_wake))
            && engine.semantic_identity()?.as_ref() == Some(&index.identity)
            && index
                .receipt
                .current(&engine.semantic_anchor()?, &index.identity.digest)
        {
            return Ok(Arc::clone(index));
        }
        *slot = None;
        let profile = &self.profile;
        let loaded = DenseIndex::load(
            engine,
            &profile.descriptor.digest(),
            &profile.recipe_id(),
            profile.descriptor.dims(),
            control,
        )?;
        *slot = Some(Arc::clone(&loaded));
        Ok(loaded)
    }

    /// The dense window for one query against `engine`: the validated dense
    /// index (refused by name when the store was prepared for another
    /// profile), then the query embedding under its ceiling, then the top
    /// [`DENSE_WINDOW`]. `Err` is the named fallback; it never fails the
    /// call. The hits' unit locations are revalidated in the candidate
    /// assembly's final read. The whole request path counts as a foreground
    /// query being dispatched: no document batch is admitted meanwhile.
    pub fn window(
        &self,
        engine: &Engine,
        query: &str,
        deadline: Instant,
        control: &Control,
    ) -> Result<DenseWindow, Fallback> {
        let _dispatching = Dispatching::enter(self);
        let generation = self.dense_index(engine, control)?;
        let vector = self.embed(query, deadline)?;
        control.check()?;
        let hits = generation.search(&vector, DENSE_WINDOW)?;
        let areas = super::area::search(engine, &self.profile, &vector, control)?;
        Ok(DenseWindow {
            hits,
            generation,
            vector,
            areas,
        })
    }

    /// The function digest of the serving profile, for identity checks
    /// against the store's state row.
    pub fn function_digest(&self) -> String {
        self.profile.descriptor.digest()
    }
}

#[cfg(all(test, feature = "test-faults"))]
mod tests {
    //! 009 T003, captain decision 2026-10-06: a query that finds the slot
    //! held by this owner's document batch waits for it, bounded by its own
    //! ceiling. Barriers: the provider reports every call it enters and
    //! holds it until the test releases it; the waiting query is read from
    //! the slot itself.
    use super::*;
    use crate::neural::provider::FunctionDescriptor;
    use std::sync::atomic::AtomicBool;

    fn unit_vector() -> Vec<f32> {
        let mut vector = vec![0f32; crate::testkit::FIXTURE_DIMENSIONS as usize];
        vector[0] = 1.0;
        vector
    }

    fn one_input() -> Vec<TokenizedInput> {
        vec![TokenizedInput { ids: vec![1, 2, 3] }]
    }

    /// Every document call, and every query while `hold_queries`, reports
    /// that it entered and then waits for one release. Queries are counted;
    /// `late` is the provider's late-call probe.
    struct Gated {
        descriptor: FunctionDescriptor,
        entered: mpsc::Sender<&'static str>,
        release: mpsc::Receiver<()>,
        hold_queries: bool,
        queries: Arc<AtomicUsize>,
        late: Arc<AtomicBool>,
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
            let _ = self.entered.send("documents");
            let _ = self.release.recv();
            Ok(batch.iter().map(|_| unit_vector()).collect())
        }
        fn embed_query(
            &mut self,
            _input: &TokenizedInput,
            _deadline: Instant,
        ) -> Result<Vec<f32>, ProviderError> {
            self.queries.fetch_add(1, Ordering::SeqCst);
            if self.hold_queries {
                let _ = self.entered.send("query");
                let _ = self.release.recv();
            }
            Ok(unit_vector())
        }
        fn late_call(&self) -> Option<LateCall> {
            let late = Arc::clone(&self.late);
            Some(Arc::new(move || late.load(Ordering::SeqCst)))
        }
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        runtime: Arc<QueryRuntime>,
        entered: mpsc::Receiver<&'static str>,
        release: mpsc::Sender<()>,
        queries: Arc<AtomicUsize>,
        late: Arc<AtomicBool>,
    }

    impl Fixture {
        fn new(hold_queries: bool) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let path = crate::testkit::write_semantic_profile(dir.path(), "wait", |_| {});
            let profile = Arc::new(SemanticProfile::load(&path).unwrap());
            let (entered_tx, entered) = mpsc::channel();
            let (release, release_rx) = mpsc::channel();
            let queries = Arc::new(AtomicUsize::new(0));
            let late = Arc::new(AtomicBool::new(false));
            let gated = Gated {
                descriptor: profile.descriptor.clone(),
                entered: entered_tx,
                release: release_rx,
                hold_queries,
                queries: Arc::clone(&queries),
                late: Arc::clone(&late),
            };
            let runtime = QueryRuntime::start(
                profile,
                Box::new(move || Ok(Box::new(gated) as Box<dyn EmbeddingProvider>)),
            )
            .unwrap();
            Self {
                _dir: dir,
                runtime: Arc::new(runtime),
                entered,
                release,
                queries,
                late,
            }
        }

        /// The next call the provider entered.
        fn entered(&self) -> &'static str {
            self.entered
                .recv_timeout(Duration::from_secs(30))
                .expect("the provider entered a call")
        }

        /// Admit one document batch and wait until the provider runs it.
        fn batch_in_flight(&self) -> DocumentCall {
            let call = self
                .runtime
                .dispatch_documents(one_input(), Control::unbounded())
                .expect("the slot is free");
            assert_eq!(self.entered(), "documents");
            call
        }

        /// One query on its own thread; its read deadline is 60 s away, so
        /// its ceiling is the full 1500 ms.
        fn query(&self) -> std::thread::JoinHandle<Result<Vec<f32>, ProviderError>> {
            let runtime = Arc::clone(&self.runtime);
            std::thread::spawn(move || {
                runtime.embed("dusk", Instant::now() + Duration::from_secs(60))
            })
        }

        /// A query on this thread that must be refused before its own
        /// ceiling: a waiting query returns only once its ceiling ended.
        fn refused_at_once(&self, query: &str) {
            let started = Instant::now();
            let result = self
                .runtime
                .embed(query, Instant::now() + Duration::from_secs(60));
            assert_eq!(result, Err(ProviderError::Busy));
            assert!(
                started.elapsed() < QUERY_CEILING,
                "{query}: refused only after {:?}, so it waited",
                started.elapsed()
            );
        }

        fn waiting(&self) -> bool {
            self.runtime.slot.lock().waiter
        }

        /// Barrier: a query waits for the document batch.
        fn until_waiting(&self) {
            let deadline = Instant::now() + Duration::from_secs(30);
            while !self.waiting() {
                assert!(Instant::now() < deadline, "no query ever waited");
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        fn documents_done(call: &DocumentCall) {
            match call.wait(Duration::from_secs(30)) {
                Some(Ok(vectors)) => assert_eq!(vectors.len(), 1),
                Some(Err(error)) => panic!("the document call failed: {error}"),
                None => panic!("the document call never ended"),
            }
        }
    }

    #[test]
    fn knowledge_admission_defers_to_queries_and_keeps_document_calls_exclusive() {
        let fixture = Fixture::new(false);
        let knowledge = fixture.runtime.admit_knowledge().unwrap();
        assert!(matches!(
            fixture
                .runtime
                .dispatch_documents(one_input(), Control::unbounded()),
            Err(ProviderError::Busy)
        ));
        let query = fixture.query();
        fixture.until_waiting();
        assert!(matches!(
            fixture.runtime.admit_knowledge(),
            Err(ProviderError::Busy)
        ));
        drop(knowledge);
        assert_eq!(query.join().unwrap(), Ok(unit_vector()));
        let documents = fixture.batch_in_flight();
        assert!(matches!(
            fixture.runtime.admit_knowledge(),
            Err(ProviderError::Busy)
        ));
        fixture.release.send(()).unwrap();
        Fixture::documents_done(&documents);
        fixture.late.store(true, Ordering::SeqCst);
        assert!(matches!(
            fixture.runtime.admit_knowledge(),
            Err(ProviderError::Busy)
        ));
        fixture.late.store(false, Ordering::SeqCst);
        drop(fixture.runtime.admit_knowledge().unwrap());
        assert!(!fixture.runtime.occupied());
    }

    #[test]
    fn a_query_waits_for_the_document_batch_and_gets_the_model() {
        let fixture = Fixture::new(false);
        let call = fixture.batch_in_flight();
        let query = fixture.query();
        fixture.until_waiting();
        assert_eq!(fixture.queries.load(Ordering::SeqCst), 0);
        fixture.release.send(()).unwrap();
        assert_eq!(
            query.join().unwrap(),
            Ok(unit_vector()),
            "the query got the model once the batch ended"
        );
        Fixture::documents_done(&call);
        assert_eq!(fixture.queries.load(Ordering::SeqCst), 1);
        assert!(!fixture.waiting());
        assert!(!fixture.runtime.occupied());
    }

    /// The query's ceiling ends while the batch still runs: `provider_busy`,
    /// which the request path serves as baseline results
    /// (`fallback:provider_busy`); the model never saw the query and the
    /// batch ends normally.
    #[test]
    fn a_query_whose_ceiling_ends_first_gets_provider_busy() {
        let fixture = Fixture::new(false);
        let call = fixture.batch_in_flight();
        // A 200 ms read deadline: a 100 ms ceiling.
        let result = fixture
            .runtime
            .embed("dusk", Instant::now() + Duration::from_millis(200));
        assert_eq!(result, Err(ProviderError::Busy));
        assert_eq!(Fallback::from(ProviderError::Busy).code, "provider_busy");
        assert!(!fixture.waiting(), "the query no longer waits");
        fixture.release.send(()).unwrap();
        Fixture::documents_done(&call);
        assert_eq!(fixture.queries.load(Ordering::SeqCst), 0);
        assert!(!fixture.runtime.occupied());
    }

    /// At most one query waits: a second one is `provider_busy` at once
    /// while the first still waits. A slot held by a query, or by the
    /// provider's late call, is `provider_busy` at once too.
    #[test]
    fn only_one_query_waits_and_any_other_holder_refuses_at_once() {
        let fixture = Fixture::new(true);
        let call = fixture.batch_in_flight();
        let first = fixture.query();
        fixture.until_waiting();
        fixture.refused_at_once("a second query");
        assert!(fixture.waiting(), "the first query still waits");

        // The batch ends; the waiter's query now holds the slot.
        fixture.release.send(()).unwrap();
        Fixture::documents_done(&call);
        assert_eq!(fixture.entered(), "query");
        fixture.refused_at_once("a query while a query runs");
        fixture.release.send(()).unwrap();
        assert_eq!(first.join().unwrap(), Ok(unit_vector()));

        // A call the provider abandoned still runs in the model.
        fixture.late.store(true, Ordering::SeqCst);
        fixture.refused_at_once("a query beside a late call");
        fixture.late.store(false, Ordering::SeqCst);
        assert_eq!(fixture.queries.load(Ordering::SeqCst), 1);
        assert!(!fixture.runtime.occupied());
    }

    /// Review M4: the batch ends AFTER the waiter's ceiling but before the
    /// waiter reacquires the slot lock. Barrier: the test holds the slot lock
    /// while the ceiling passes, frees the slot under it and only then lets
    /// the waiter reacquire. The waiter must not claim the freed slot: it
    /// returns `provider_busy`, no query job is sent and the promise is
    /// withdrawn.
    #[test]
    fn a_batch_ending_after_the_ceiling_is_never_claimed_by_the_waiter() {
        let fixture = Fixture::new(false);
        let call = fixture.batch_in_flight();
        let query = {
            let runtime = Arc::clone(&fixture.runtime);
            // A 400 ms read deadline: a 200 ms ceiling.
            std::thread::spawn(move || {
                runtime.embed("dusk", Instant::now() + Duration::from_millis(400))
            })
        };
        fixture.until_waiting();
        {
            let mut slot = fixture.runtime.slot.lock();
            std::thread::sleep(Duration::from_millis(400));
            assert!(slot.waiter, "the waiter cannot reacquire meanwhile");
            slot.holder = None;
            fixture.runtime.slot.freed.notify_all();
        }
        assert_eq!(query.join().unwrap(), Err(ProviderError::Busy));
        {
            let slot = fixture.runtime.slot.lock();
            assert!(!slot.waiter, "the promise was withdrawn");
            assert_eq!(slot.holder, None, "the waiter claimed nothing");
        }
        fixture.release.send(()).unwrap();
        Fixture::documents_done(&call);
        assert_eq!(fixture.queries.load(Ordering::SeqCst), 0, "no query job");
    }

    /// While a query waits, no document batch is admitted: neither while the
    /// batch it waits for runs nor once that batch ended and the waiter took
    /// the slot. Only after the query returned is the next batch admitted.
    #[test]
    fn no_document_batch_is_admitted_while_a_query_waits() {
        let fixture = Fixture::new(true);
        let first = fixture.batch_in_flight();
        let query = fixture.query();
        fixture.until_waiting();
        assert!(matches!(
            fixture
                .runtime
                .dispatch_documents(one_input(), Control::unbounded()),
            Err(ProviderError::Busy)
        ));
        fixture.release.send(()).unwrap();
        // The batch ended (its reply follows the slot's release); the query
        // is still in its request path, so nothing is admitted.
        Fixture::documents_done(&first);
        assert!(matches!(
            fixture
                .runtime
                .dispatch_documents(one_input(), Control::unbounded()),
            Err(ProviderError::Busy)
        ));
        assert_eq!(fixture.entered(), "query");
        fixture.release.send(()).unwrap();
        assert_eq!(query.join().unwrap(), Ok(unit_vector()));
        let next = fixture
            .runtime
            .dispatch_documents(one_input(), Control::unbounded())
            .expect("admitted once the query returned");
        assert_eq!(fixture.entered(), "documents");
        fixture.release.send(()).unwrap();
        Fixture::documents_done(&next);
        assert_eq!(fixture.queries.load(Ordering::SeqCst), 1);
    }
}
