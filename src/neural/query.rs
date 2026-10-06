//! 009 T002 query-side retrieval: embed one query under its ceiling, load
//! the derived dense index through the store descriptor (never a re-resolved
//! path) from the very bytes that matched the generation manifest, with the
//! USearch header and geometry check T001 deferred to this task, and return
//! the bounded dense window the merge fuses with the lexical candidates.
//! A dense hit expands to unit locations through the generation's own label
//! map; no request walks the partition table.
//!
//! Nothing here reranks, rewrites the query or calls a second model: the
//! ordering decision is [`super::merge`]. A failure at any step is a NAMED
//! fallback ([`Fallback`]) for the baseline path; it never fails the response.
use crate::control::Control;
use crate::error::FoundryError;
use crate::neural::index::{
    self, COVERAGE_COMPLETE, GenerationError, LabelEntry, METRIC, SCALAR_KIND, UnitLocation,
};
use crate::neural::partition::TokenCount as _;
use crate::neural::profile::SemanticProfile;
use crate::neural::provider::{
    self, EmbeddingProvider, LateCall, ProviderError, QUERY_PREFIX, TokenizedInput,
};
use crate::neural::tokenize::DocumentTokenizer;
use crate::store::Engine;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::time::{Duration, Instant};

/// The dense candidate window (D001): the top 64 dense hits enter the fusion.
pub const DENSE_WINDOW: usize = 64;
/// The per-request query-embedding ceiling (D001): the earlier of 1500 ms
/// and HALF the remaining read deadline, so a cut-off embedding always
/// leaves the baseline fallback at least as much time as it got; deadlines
/// never accumulate.
pub const QUERY_CEILING: Duration = Duration::from_millis(1500);

/// Why one request is served without dense candidates: a stable category
/// `code` (`provider_timeout`, `profile_mismatch`, `index_unavailable`,
/// `index_geometry`, `semantic_unprepared`, …) and a detail. The response
/// header names it as `fallback:<code>[: <detail>]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fallback {
    pub code: &'static str,
    pub detail: String,
}

impl Fallback {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for Fallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.detail.is_empty() {
            f.write_str(self.code)
        } else {
            write!(f, "{}: {}", self.code, self.detail)
        }
    }
}

impl From<ProviderError> for Fallback {
    fn from(error: ProviderError) -> Self {
        let text = error.to_string();
        let detail = text
            .strip_prefix(error.code())
            .map_or(text.as_str(), |rest| rest.trim_start_matches(": "));
        Self::new(error.code(), detail)
    }
}

impl From<FoundryError> for Fallback {
    fn from(error: FoundryError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

/// One dense hit: the label of a stored vector in the serving generation
/// and its distance to the query vector (ascending = better).
#[derive(Clone, Debug)]
pub struct DenseHit {
    pub label: usize,
    pub distance: f32,
}

/// The dense window of one request: the top hits of the validated index,
/// nearest first, read against the generation that produced them.
pub struct DenseWindow {
    pub hits: Vec<DenseHit>,
    generation: Arc<DenseIndex>,
}

impl std::fmt::Debug for DenseWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DenseWindow")
            .field("hits", &self.hits)
            .finish_non_exhaustive()
    }
}

impl DenseWindow {
    /// The document-input key of one hit.
    pub fn input_key(&self, hit: &DenseHit) -> &str {
        &self.generation.labels[hit.label].input_key
    }

    /// The unit locations one hit stood for at publication. Each is a
    /// candidate only after the request's final read revalidates it.
    pub fn units(&self, hit: &DenseHit) -> &[UnitLocation] {
        &self.generation.labels[hit.label].units
    }

    /// The source revision a COMPLETE generation was published at; `None`
    /// for a generation that covers only part of the eligible units. The
    /// coverage word is `ready` only when this equals the request's revision.
    pub fn complete_at(&self) -> Option<u64> {
        self.generation
            .complete
            .then_some(self.generation.source_revision)
    }

    /// The coverage word of a response whose final read saw
    /// `source_revision`: `ready` only for a complete generation published
    /// at exactly that revision, otherwise `partial`. Known coverage, never
    /// inferred from the candidate set.
    pub fn coverage_at(&self, source_revision: u64) -> &'static str {
        if self.complete_at() == Some(source_revision) {
            "ready"
        } else {
            "partial"
        }
    }
}

/// A validated, loaded dense index. Availability came from VALIDATION
/// (manifest, file hashes, header and geometry), and the index was restored
/// from the very buffer whose hash matched the manifest, read through the
/// store's bound descriptor: no later file substitution can reach it.
pub struct DenseIndex {
    index: usearch::Index,
    /// `labels[label]` is that ordinal's input key and unit locations.
    labels: Vec<LabelEntry>,
    /// The source revision the label map was read at.
    source_revision: u64,
    /// True when the generation covers every eligible unit at that revision.
    complete: bool,
}

impl DenseIndex {
    /// Load the serving generation: the state row names the function digest
    /// and recipe; the manifest and label map are validated by content and
    /// the index file is read ONCE and hashed in memory; then the USearch
    /// header of THOSE bytes is checked against the manifest's geometry
    /// BEFORE they are restored, and the restored index's own geometry is
    /// checked again. `Err` is the named fallback the baseline path reports.
    pub fn load(
        engine: &Engine,
        expected_digest: &str,
        expected_recipe: &str,
        control: &Control,
    ) -> Result<Arc<Self>, Fallback> {
        let unprepared = || Fallback::new("semantic_unprepared", "no semantic profile prepared");
        let Some(state) = engine.semantic_state()? else {
            return Err(unprepared());
        };
        let (digest, recipe) = match (state.function_digest, state.recipe_id) {
            (Some(digest), Some(recipe)) => (digest, recipe),
            _ => return Err(unprepared()),
        };
        // A store prepared for ANOTHER profile or partition recipe never
        // serves this one: unavailable by name, baseline intact.
        if digest != expected_digest {
            return Err(Fallback::new(
                "profile_mismatch",
                "the store was prepared for a different document function; \
                 run `semantic prepare` for this profile",
            ));
        }
        if recipe != expected_recipe {
            return Err(Fallback::new(
                "profile_mismatch",
                "the store's partition recipe differs from this profile's",
            ));
        }
        let anchor = engine.semantic_anchor()?;
        let (generation, bytes) =
            match index::load_generation_with(&anchor, &digest, &recipe, control) {
                Ok(loaded) => loaded,
                Err(GenerationError::Interrupted(error)) => return Err(error.into()),
                Err(GenerationError::Unavailable(reason)) => {
                    return Err(Fallback::new("index_unavailable", reason));
                }
            };
        neural_fault!(LOAD_AFTER_VERIFY, Some(control), &digest)?;
        // The deferred T002 check: the library's own dense header, parsed
        // from the verified bytes and compared with the manifest's geometry
        // before any vector is searched.
        let geometry = |detail: String| Fallback::new("index_geometry", detail);
        let header = usearch::Index::metadata_from_buffer(&bytes)
            .map_err(|e| Fallback::new("index_unavailable", format!("usearch header: {e}")))?;
        let manifest = &generation.manifest;
        if header.dimensions as usize != manifest.dimensions
            || header.count_present as usize != manifest.count
            || header.multi
            || !format!("{:?}", header.metric).eq_ignore_ascii_case(METRIC)
            || !format!("{:?}", header.quantization).eq_ignore_ascii_case(SCALAR_KIND)
        {
            return Err(geometry(format!(
                "usearch header disagrees with the generation manifest \
                 (dimensions {}, metric {:?}, scalar {:?}, multi {}, present {})",
                header.dimensions,
                header.metric,
                header.quantization,
                header.multi,
                header.count_present
            )));
        }
        let loaded = usearch::Index::restore_from_buffer(&bytes)
            .map_err(|e| Fallback::new("index_unavailable", format!("usearch load: {e}")))?;
        if loaded.dimensions() != manifest.dimensions || loaded.size() != manifest.count {
            return Err(geometry(format!(
                "loaded index geometry disagrees with the manifest (dimensions {}, size {})",
                loaded.dimensions(),
                loaded.size()
            )));
        }
        let source_revision = manifest.source_revision;
        let complete = manifest.coverage == COVERAGE_COMPLETE;
        Ok(Arc::new(Self {
            index: loaded,
            labels: generation.labels.labels,
            source_revision,
            complete,
        }))
    }

    /// The top `k` labels by ascending distance. Labels the label map does
    /// not hold (a disagreement validation somehow missed) are skipped
    /// rather than served.
    pub fn search(&self, vector: &[f32], k: usize) -> Result<Vec<DenseHit>, Fallback> {
        if vector.len() != self.index.dimensions() {
            return Err(Fallback::new(
                "index_unavailable",
                format!(
                    "query vector has {} values, the index holds {}",
                    vector.len(),
                    self.index.dimensions()
                ),
            ));
        }
        let matches = self
            .index
            .search(vector, k)
            .map_err(|e| Fallback::new("index_unavailable", format!("usearch search: {e}")))?;
        Ok(matches
            .keys
            .iter()
            .zip(&matches.distances)
            .filter(|&(&key, _)| (key as usize) < self.labels.len())
            .map(|(&key, &distance)| DenseHit {
                label: key as usize,
                distance,
            })
            .collect())
    }
}

/// A named fallback word for the response header: `fallback:<reason>`, one
/// short line with no header separator, so a provider's message can never
/// forge a segment.
pub fn fallback_word(reason: &str) -> String {
    let cleaned: String = reason
        .chars()
        .map(|c| {
            if c.is_control() || c == '\u{b7}' {
                ' '
            } else {
                c
            }
        })
        .take(120)
        .collect();
    format!("fallback:{}", cleaned.trim())
}

/// Render and tokenize one query and refuse the serving limit: the exact
/// model input of a query.
pub fn tokenize_query(
    tokenizer: &DocumentTokenizer,
    query: &str,
) -> Result<TokenizedInput, ProviderError> {
    let rendered = format!("{QUERY_PREFIX}{query}");
    let tokenized = tokenizer.encode(&rendered)?;
    let input = TokenizedInput { ids: tokenized.ids };
    provider::check_query(&input)?;
    Ok(input)
}

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
/// batches alike) and the loaded dense index. The index is kept until the
/// owner's preparation driver publishes a new generation and calls
/// [`QueryRuntime::forget_index`]; source edits are caught by the final
/// read, not by the index.
pub struct QueryRuntime {
    pub profile: Arc<SemanticProfile>,
    tokenizer: DocumentTokenizer,
    /// `None` once [`Self::shutdown`] closed it.
    jobs: Mutex<Option<mpsc::Sender<Job>>>,
    /// The provider thread, joined by [`Self::shutdown`].
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// True from the dispatch of a query or document batch until the
    /// provider thread finished it: a request whose caller timed out keeps
    /// the slot Busy until the late reply is dropped, exactly like the
    /// supervised worker.
    busy: Arc<AtomicBool>,
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
}

impl QueryRuntime {
    /// Acquire the supervised worker (the profile is verified and the worker
    /// started, bounded by the profile's load timeout, before anything
    /// serves), then load the exact-input tokenizer. `Err` is the named
    /// reason the baseline path reports.
    pub fn acquire(
        profile_path: &std::path::Path,
        development: bool,
    ) -> Result<Self, ProviderError> {
        let profile = Arc::new(SemanticProfile::load(profile_path)?);
        let for_worker = Arc::clone(&profile);
        Self::start(
            profile,
            Box::new(move || {
                crate::neural::supervisor::acquire_until(
                    &for_worker,
                    development,
                    &Control::unbounded(),
                )
            }),
        )
    }

    /// Start a runtime over `make` (the supervised worker, or a test
    /// provider). Blocks until the provider is built or refused.
    pub fn start(profile: Arc<SemanticProfile>, make: MakeProvider) -> Result<Self, ProviderError> {
        let (jobs_tx, jobs_rx) = mpsc::channel::<Job>();
        let (ready_tx, ready_rx) =
            mpsc::channel::<Result<(String, Option<LateCall>), ProviderError>>();
        let busy = Arc::new(AtomicBool::new(false));
        let thread_busy = Arc::clone(&busy);
        let thread = std::thread::Builder::new()
            .name("foundry-query-provider".into())
            .spawn(move || {
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
                            thread_busy.store(false, Ordering::SeqCst);
                            let _ = reply.send(result);
                        }
                        Job::Documents {
                            inputs,
                            control,
                            reply,
                        } => {
                            let result = provider.embed_documents(&inputs, &control);
                            thread_busy.store(false, Ordering::SeqCst);
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
        let tokenizer = DocumentTokenizer::load(&profile)?;
        Ok(Self {
            profile,
            tokenizer,
            jobs: Mutex::new(Some(jobs_tx)),
            thread: Mutex::new(Some(thread)),
            busy,
            dispatching: AtomicUsize::new(0),
            admission: Mutex::new(()),
            late,
            served_digest,
            index: Mutex::new(None),
        })
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
    pub fn embed(&self, query: &str, deadline: Instant) -> Result<Vec<f32>, ProviderError> {
        let _dispatching = Dispatching::enter(self);
        let _ = neural_fault!(QUERY_REGISTERED, None, query);
        let input = tokenize_query(&self.tokenizer, query)?;
        let now = Instant::now();
        let ceiling = now + QUERY_CEILING.min(deadline.saturating_duration_since(now) / 2);
        if Instant::now() >= ceiling {
            return Err(ProviderError::Timeout);
        }
        self.claim_slot("query")?;
        let (reply, answer) = mpsc::channel();
        let sent = self.send(Job::Query {
            input,
            deadline: ceiling,
            reply,
        });
        if !sent {
            self.busy.store(false, Ordering::SeqCst);
            return Err(ProviderError::WorkerExited(
                "the provider thread is gone".into(),
            ));
        }
        match answer.recv_timeout(ceiling.saturating_duration_since(Instant::now())) {
            Ok(result) => result.and_then(|vector| {
                provider::validate_vector(&vector)?;
                Ok(vector)
            }),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ProviderError::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ProviderError::WorkerExited(
                "the provider thread ended during the query".into(),
            )),
        }
    }

    /// 009 T003: admit ONE document batch (at most [`provider::DOCUMENT_BATCH`]
    /// inputs, limits checked first) on the same slot as queries, with the
    /// caller's `control` (its deadline and cancellation reach the provider).
    /// Refused `Busy` — nothing is queued — while the slot is held by a call
    /// that has not actually ended (the provider's late call included), or
    /// while a foreground query is being dispatched. The decision is taken
    /// under the admission lock that query registration takes too: a query
    /// registered before it always wins.
    pub fn dispatch_documents(
        &self,
        inputs: Vec<TokenizedInput>,
        control: Control,
    ) -> Result<DocumentCall, ProviderError> {
        provider::check_document_batch(&inputs)?;
        let _ = neural_fault!(DOCUMENT_ADMISSION, Some(&control), "");
        {
            let _admission = self.admission();
            if self.dispatching.load(Ordering::SeqCst) > 0 {
                return Err(ProviderError::Busy);
            }
            self.claim_slot("documents")?;
        }
        let (reply, answer) = mpsc::channel();
        let sent = self.send(Job::Documents {
            inputs,
            control,
            reply,
        });
        if !sent {
            self.busy.store(false, Ordering::SeqCst);
            return Err(ProviderError::WorkerExited(
                "the provider thread is gone".into(),
            ));
        }
        Ok(DocumentCall { answer })
    }

    /// Claim the one model slot: the outer flag FIRST, then the provider's
    /// late call. The provider thread clears the flag only after the provider
    /// returned, and the supervisor marks a call abandoned before it returns,
    /// so once the claim succeeds any late call is already visible; a claim
    /// that finds one is released at once. Nothing is created or sent on a
    /// refusal.
    fn claim_slot(&self, detail: &str) -> Result<(), ProviderError> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(ProviderError::Busy);
        }
        let _ = neural_fault!(SLOT_CLAIMED, None, detail);
        if self.late_call_running() {
            self.busy.store(false, Ordering::SeqCst);
            return Err(ProviderError::Busy);
        }
        Ok(())
    }

    /// True while the one model slot is held by a call that has not actually
    /// ended (the provider's late call included), or a foreground query is
    /// being dispatched. The flag is read before the late call, in the order
    /// the provider thread publishes them.
    pub fn occupied(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
            || self.dispatching.load(Ordering::SeqCst) > 0
            || self.late_call_running()
    }

    /// The function digest of the descriptor the provider itself serves.
    pub fn served_digest(&self) -> &str {
        &self.served_digest
    }

    /// The profile's exact-input tokenizer (the preparation driver's too).
    pub(crate) fn tokenizer(&self) -> &DocumentTokenizer {
        &self.tokenizer
    }

    /// Drop the loaded dense index after a new generation was published; the
    /// next request validates and loads the new one.
    pub fn forget_index(&self) {
        if let Ok(mut slot) = self.index.lock() {
            *slot = None;
        }
    }

    /// The validated dense index of `engine`, loaded once and kept until
    /// [`Self::forget_index`]. A store prepared for another profile is refused
    /// by name HERE, before any query embedding is spent.
    fn dense_index(&self, engine: &Engine, control: &Control) -> Result<Arc<DenseIndex>, Fallback> {
        let mut slot = self
            .index
            .lock()
            .map_err(|_| Fallback::new("internal", "the dense index lock was poisoned"))?;
        if let Some(index) = slot.as_ref() {
            return Ok(Arc::clone(index));
        }
        let loaded = DenseIndex::load(
            engine,
            &self.profile.descriptor.digest(),
            &crate::neural::partition::recipe_id(&self.profile.descriptor.tokenizer),
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
        Ok(DenseWindow { hits, generation })
    }

    /// The function digest of the serving profile, for identity checks
    /// against the store's state row.
    pub fn function_digest(&self) -> String {
        self.profile.descriptor.digest()
    }
}
