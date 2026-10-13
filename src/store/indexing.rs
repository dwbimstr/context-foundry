//! The parallel refresh (context-v2 § Parallel indexing): the build pool, the
//! bounded hand-out, page builds and the one writer, and the `Engine` refresh
//! entry points. The test seam, parse-failure counts and the search schema
//! stay in the parent module.
use super::*;

/// The reusable ingestion/build pool: min(available parallelism, 8).
fn index_threads(hooks: &IndexHooks) -> usize {
    hooks
        .threads()
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map_or(1, std::num::NonZeroUsize::get)
                .min(MAX_INDEX_THREADS)
        })
        .max(1)
}

fn index_pool(
    slot: &mut Option<std::sync::Arc<rayon::ThreadPool>>,
    hooks: &IndexHooks,
) -> FResult<std::sync::Arc<rayon::ThreadPool>> {
    let count = index_threads(hooks);
    if slot
        .as_ref()
        .is_none_or(|pool| hooks.threads().is_some() && pool.current_num_threads() != count)
    {
        *slot = Some(std::sync::Arc::new(
            rayon::ThreadPoolBuilder::new()
                .num_threads(count)
                .thread_name(|index| format!("foundry-index-{index}"))
                .build()
                .map_err(|error| {
                    FoundryError::Internal(anyhow::anyhow!(
                        "cannot start indexing workers: {error}"
                    ))
                })?,
        ));
    }
    Ok(std::sync::Arc::clone(
        slot.as_ref().expect("pool initialized"),
    ))
}

/// One source handed out to a build thread: its page position and its
/// verified bytes.
struct BuildJob {
    position: usize,
    path: String,
    hash: String,
    body: String,
}

/// The documents built for one source and, when they are the plain blocks
/// of an unmapped source because its parse panicked or exceeded its work
/// budget, the labeled failure: `parse_panicked: <message>` or
/// `parse_stopped: <reason>`.
struct SourceBuild {
    documents: Vec<TantivyDocument>,
    panic: Option<String>,
}

/// A build thread's outcome for one source; `Err` only when even its
/// plain-block documents panicked.
type BuildOutcome = Result<SourceBuild, String>;

#[derive(Default)]
struct HandOutState {
    jobs: std::collections::VecDeque<BuildJob>,
    /// Finished builds the writer has not taken yet, by page position.
    built: std::collections::BTreeMap<usize, BuildOutcome>,
    /// Nothing more will be handed out.
    closed: bool,
    /// A build thread unwound outside its panic boundary: the writer never
    /// waits for a build it can no longer get.
    broken: Option<String>,
}

/// The hand-out between the calling thread and the build threads of one
/// page (context-v2 § Parallel indexing): queued jobs one way, finished
/// builds by page position the other.
#[derive(Default)]
struct HandOut {
    state: std::sync::Mutex<HandOutState>,
    /// A job was queued or the hand-out closed.
    work: std::sync::Condvar,
    /// A build finished or a build thread failed.
    finished: std::sync::Condvar,
}

impl HandOut {
    fn lock(&self) -> std::sync::MutexGuard<'_, HandOutState> {
        // The critical sections never panic; a poisoned guard is recovered.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn push(&self, job: BuildJob) {
        self.lock().jobs.push_back(job);
        self.work.notify_one();
    }

    /// The next job, waiting for one; `None` once the hand-out is closed and
    /// every job was taken.
    fn take(&self) -> Option<BuildJob> {
        let mut state = self.lock();
        loop {
            if let Some(job) = state.jobs.pop_front() {
                return Some(job);
            }
            if state.closed {
                return None;
            }
            state = self
                .work
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    fn finish(&self, position: usize, outcome: BuildOutcome) {
        self.lock().built.insert(position, outcome);
        self.finished.notify_one();
    }

    /// The finished build at `position`, waiting for it when `wait`;
    /// without `wait`, `None` while it is unfinished.
    fn built(&self, position: usize, wait: bool) -> FResult<Option<BuildOutcome>> {
        let mut state = self.lock();
        loop {
            if let Some(outcome) = state.built.remove(&position) {
                return Ok(Some(outcome));
            }
            if let Some(message) = &state.broken {
                return Err(FoundryError::Internal(anyhow::anyhow!(
                    "an index build thread failed: {message}"
                )));
            }
            if !wait {
                return Ok(None);
            }
            state = self
                .finished
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// Stop handing out and drop the jobs no thread has taken.
    fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        state.jobs.clear();
        drop(state);
        self.work.notify_all();
    }
}

/// Closes the hand-out when dropped, an unwind of the writer included, so no
/// build thread waits for work that never comes and the scope can finish.
struct Closing<'a>(&'a HandOut);

impl Drop for Closing<'_> {
    fn drop(&mut self) {
        self.0.close();
    }
}

/// Index one pending page (context-v2 § Parallel indexing). Its sources are
/// handed out in key order to at most [`index_threads`] build threads, while
/// the one writer, on this thread, adds every key's documents in key order
/// as they become ready. At most [`HANDOUT_BYTES`] of source bytes are handed
/// out and not yet consumed — a source's bytes are consumed once the writer
/// has added its documents. Every page job has finished when this returns;
/// pool threads remain available for later pages. Nothing is committed here.
///
/// The control is checked before each hand-out: a cancellation or deadline
/// seen there, or any failure, stops the hand-out and returns the error with
/// the page uncommitted. A cancellation after the page's last hand-out is
/// not seen here: the page's documents are added, and the caller commits
/// them and sees it after the commit. A build thread that panicked outside
/// its parse boundary fails the page by name. Returns the parse panics of
/// the page by position.
#[allow(clippy::too_many_arguments)]
fn index_page<S, C, M, B>(
    writer: &mut IndexWriter,
    fields: &Fields,
    pending: &[(String, String)],
    sources: &S,
    stored: &C,
    memory: &M,
    briefs: &B,
    control: &crate::Control,
    hooks: &IndexHooks,
    pool: Option<&rayon::ThreadPool>,
) -> FResult<Vec<(usize, String)>>
where
    S: ReadableTable<&'static str, &'static str>,
    C: ReadableTable<&'static str, &'static str>,
    M: ReadableTable<&'static str, &'static str>,
    B: ReadableTable<&'static str, &'static str>,
{
    let jobs = pending
        .iter()
        .filter(|(key, _)| key.starts_with("source:"))
        .count();
    let threads = pool
        .map_or(0, rayon::ThreadPool::current_num_threads)
        .min(jobs);
    let queue = HandOut::default();
    let mut page = PageWriter {
        writer,
        fields,
        memory,
        briefs,
        pending,
        queue: &queue,
        control,
        hooks,
        charged: vec![None; pending.len()],
        outstanding: 0,
        next: 0,
        panics: Vec::new(),
    };
    let indexed = if let Some(pool) = pool {
        pool.in_place_scope(|scope| {
            let _closing = Closing(&queue);
            for _ in 0..threads {
                let queue = &queue;
                scope.spawn(move |_| {
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        build_thread(queue, fields, hooks);
                    }));
                    if let Err(payload) = outcome {
                        let mut state = queue.lock();
                        state.broken = Some(panic_message(&*payload));
                        state.closed = true;
                        state.jobs.clear();
                        drop(state);
                        queue.finished.notify_all();
                        queue.work.notify_all();
                    }
                });
            }
            let indexed = page.hand_out(sources, stored);
            // Stop handing out. After a failure or cancellation the work no
            // job has taken is dropped; each job finishes its current source
            // and returns to the retained pool. The scope joins every job,
            // including a panic outside its parse boundary.
            queue.close();
            indexed
        })
    } else {
        page.hand_out(sources, stored)
    };
    if let Some(message) = &queue.lock().broken {
        return Err(FoundryError::Internal(anyhow::anyhow!(
            "an index build thread panicked: {message}"
        )));
    }
    indexed?;
    Ok(page.panics)
}

/// The page's one writer, on the calling thread.
struct PageWriter<'a, M, B> {
    writer: &'a mut IndexWriter,
    fields: &'a Fields,
    memory: &'a M,
    briefs: &'a B,
    pending: &'a [(String, String)],
    queue: &'a HandOut,
    control: &'a crate::Control,
    hooks: &'a IndexHooks,
    /// Per position: the bytes of a handed-out source, until its documents
    /// are added.
    charged: Vec<Option<usize>>,
    /// Source bytes handed out whose documents are not yet added.
    outstanding: usize,
    /// The next position to add.
    next: usize,
    panics: Vec<(usize, String)>,
}

impl<M: ReadableTable<&'static str, &'static str>, B: ReadableTable<&'static str, &'static str>>
    PageWriter<'_, M, B>
{
    /// Hand out the page's existing sources in key order, adding finished
    /// ones in key order as it goes, then add the rest.
    fn hand_out<S, C>(&mut self, sources: &S, stored: &C) -> FResult<()>
    where
        S: ReadableTable<&'static str, &'static str>,
        C: ReadableTable<&'static str, &'static str>,
    {
        let limit = self.hooks.handout_bytes().unwrap_or(HANDOUT_BYTES);
        let pending = self.pending;
        for (position, (key, _)) in pending.iter().enumerate() {
            let Some(path) = key.strip_prefix("source:") else {
                continue;
            };
            let meta: SourceMeta = match sources.get(path)? {
                Some(raw) => decode(raw.value(), "source")?,
                None => continue,
            };
            // A recorded length past the source bound is corruption, named
            // before any bound arithmetic.
            check_recorded_length(path, &meta)?;
            // Make room: the writer adds sources in key order, releasing
            // their bytes. Every earlier key is decided and the earliest one
            // not yet added was handed out or needs no build, so this always
            // progresses; a lone source always fits.
            let bytes = meta.bytes;
            let fits =
                |outstanding: usize| outstanding == 0 || outstanding.saturating_add(bytes) <= limit;
            if !fits(self.outstanding) {
                index_event!(
                    self.hooks,
                    index_hooks::Event::Wait {
                        outstanding: self.outstanding,
                        next: bytes,
                    }
                );
                while !fits(self.outstanding) && self.next < position {
                    self.add_until(self.next + 1, true)?;
                }
            }
            index_fault!(INDEX_HANDOUT, self.control, path)?;
            // Cancellation and the deadline stop the hand-out here.
            self.control.check()?;
            // Documents are built from the verified source bytes.
            let verified = reconstruct_verified(stored, path, &meta)?;
            self.charged[position] = Some(bytes);
            self.outstanding = self.outstanding.saturating_add(bytes);
            index_event!(
                self.hooks,
                index_hooks::Event::HandedOut {
                    path,
                    outstanding: self.outstanding,
                }
            );
            self.queue.push(BuildJob {
                position,
                path: path.to_owned(),
                hash: meta.hash,
                body: verified.body,
            });
            // Add whatever is ready, in key order, without waiting.
            self.add_until(position + 1, false)?;
        }
        self.add_until(pending.len(), true)
    }

    /// Add the keys before `upto` in key order. A handed-out source's build
    /// is awaited when `wait`; otherwise adding stops at the first
    /// unfinished one.
    fn add_until(&mut self, upto: usize, wait: bool) -> FResult<()> {
        while self.next < upto {
            let position = self.next;
            let build = match self.charged[position] {
                None => None,
                Some(_) => match self.queue.built(position, wait)? {
                    Some(outcome) => {
                        Some(outcome.map_err(|e| FoundryError::Internal(anyhow::anyhow!(e)))?)
                    }
                    None => return Ok(()),
                },
            };
            self.add(position, build)?;
            // Consumed: the source's documents are added.
            if let Some(bytes) = self.charged[position].take() {
                self.outstanding = self.outstanding.saturating_sub(bytes);
            }
            self.next += 1;
        }
        Ok(())
    }

    /// Delete and add one pending key's documents.
    fn add(&mut self, position: usize, build: Option<SourceBuild>) -> FResult<()> {
        let (pending, memory) = (self.pending, self.memory);
        let key = pending[position].0.as_str();
        index_fault!(INDEX_BEFORE_ADD, self.control, key)?;
        if let Some(path) = key.strip_prefix("source:") {
            self.writer
                .delete_term(Term::from_field_text(self.fields.path, path));
            if let Some(build) = build {
                for document in build.documents {
                    self.add_document(document)?;
                }
                if let Some(message) = build.panic {
                    self.panics.push((position, message));
                }
            }
        } else if let Some(id) = key.strip_prefix("memory:") {
            // The key field is unique per memory record and no source key
            // can equal it (source keys always contain NUL).
            self.writer
                .delete_term(Term::from_field_text(self.fields.key, key));
            if let Some(raw) = memory.get(id)? {
                // An undecodable row gets no document (it is named at get,
                // search validation and export) so one corrupt record can
                // never stall source indexing behind it.
                if let Ok(record) = serde_json::from_str::<MemoryRecord>(raw.value()) {
                    let document =
                        memory_document(self.fields, &record.id, record.revision, &record.text);
                    self.add_document(document)?;
                }
            }
        } else if let Some(id) = key.strip_prefix("brief:") {
            self.writer
                .delete_term(Term::from_field_text(self.fields.key, key));
            if let Some(raw) = self.briefs.get(id)?
                && let Ok(record) = crate::brief::decode_row(id, raw.value())
            {
                self.add_document(record_document(
                    self.fields,
                    "brief",
                    id,
                    record.revision,
                    &record.brief.searchable_text(),
                ))?;
            }
        }
        Ok(())
    }

    fn add_document(&mut self, document: TantivyDocument) -> FResult<()> {
        index_event!(
            self.hooks,
            index_hooks::Event::Add(&tantivy::Document::to_json(
                &document,
                &self.writer.index().schema()
            ))
        );
        self.writer.add_document(document)?;
        Ok(())
    }
}

/// One build thread: take sources until the hand-out closes. Its copy of a
/// source's bytes is dropped once the documents exist; the bytes stay
/// charged until the writer adds those documents.
fn build_thread(queue: &HandOut, fields: &Fields, hooks: &IndexHooks) {
    while let Some(job) = queue.take() {
        let outcome = build_source(fields, &job, hooks);
        index_event!(hooks, index_hooks::Event::Built(&job.path));
        let position = job.position;
        drop(job);
        queue.finish(position, outcome);
    }
}

/// The documents of one handed-out source. A panicking parse is caught, and
/// a parse that exceeds its work budget (context-v2 § Languages) is handled
/// the same way: the source gets the plain-block documents of an unmapped
/// source, the first of kind [`UNPARSED_KIND`], and its labeled failure, so
/// it is never dropped.
fn build_source(
    fields: &Fields,
    job: &BuildJob,
    hooks: &IndexHooks,
) -> Result<SourceBuild, String> {
    let parsed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        index_event!(hooks, index_hooks::Event::Build(&job.path));
        search_documents(fields, &job.path, &job.hash, &job.body, Parsing::Parsed)
    }));
    let failure = match parsed {
        Ok(Ok(documents)) => {
            return Ok(SourceBuild {
                documents,
                panic: None,
            });
        }
        Ok(Err(stopped)) => format!("parse_stopped: {stopped}"),
        Err(payload) => format!("parse_panicked: {}", panic_message(&*payload)),
    };
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        search_documents(fields, &job.path, &job.hash, &job.body, Parsing::Unparsed)
    }))
    .map_err(|fallback| {
        format!(
            "{}: plain-block documents panicked: {}",
            job.path,
            panic_message(&*fallback)
        )
    })
    .and_then(|documents| {
        // Without a language nothing is parsed, so nothing can stop.
        let documents = documents
            .map_err(|stopped| format!("{}: plain-block documents {stopped}", job.path))?;
        Ok(SourceBuild {
            documents,
            panic: Some(failure),
        })
    })
}

/// The text of a panic payload, when it carries one.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "the parse panicked".to_owned())
}

impl Engine {
    /// One index batch: at most `PAGE` pending keys. The page's sources are
    /// built on at most [`index_threads`] threads while the one writer adds
    /// their documents in key order ([`index_page`], context-v2 § Parallel
    /// indexing); every thread has finished before the commit. Search commit
    /// precedes clearing durable pending work; only the indexed version is
    /// cleared.
    ///
    /// The control is checked before each hand-out and after the commit. A
    /// cancellation after the page's last hand-out lets the page add and
    /// commit its documents, then stops before the pending clear: its keys
    /// stay pending, a later refresh clears them, and replay converges.
    pub fn refresh_index(&mut self, control: &crate::Control) -> FResult<(usize, usize)> {
        self.refresh_index_page(control, PAGE)
    }

    pub(crate) fn indexing_pool(&mut self) -> FResult<std::sync::Arc<rayon::ThreadPool>> {
        index_pool(&mut self.index_pool, &IndexHooks::current())
    }

    /// A smaller page for a background owner that yields to foreground requests.
    pub(crate) fn refresh_index_page(
        &mut self,
        control: &crate::Control,
        limit: usize,
    ) -> FResult<(usize, usize)> {
        if let Some(reason) = &self.repair_reason {
            return Err(FoundryError::RepairRequired(reason.clone()));
        }
        // Field-disjoint borrows: the read transaction owns self.db while the
        // writer mutably owns self.search.
        let tx = self.db.begin_read()?;
        let pending: Vec<(String, String)> = tx
            .open_table(PENDING)?
            .iter()?
            .take(limit.clamp(1, PAGE))
            .map(|row| {
                let (k, v) = row?;
                Ok((k.value().into(), v.value().into()))
            })
            .collect::<Result<Vec<(String, String)>, redb::StorageError>>()
            .map_err(FoundryError::from)?;
        if pending.is_empty() {
            return Ok((0, 0));
        }
        // Typed pending keys (008): the drain dispatches on the prefix. An
        // untyped key cannot exist in a v3 store — the upgrade migrated them
        // all — so it names authoritative corruption, before any work.
        if let Some((key, _)) = pending.iter().find(|(key, _)| {
            !key.starts_with("source:") && !key.starts_with("memory:") && !key.starts_with("brief:")
        }) {
            return Err(FoundryError::CorruptStore(format!(
                "pending key {key:?} is not typed (source:/memory:/brief:)"
            )));
        }
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let Some(handles) = self.search.as_mut() else {
            return Err(FoundryError::RepairRequired(
                "derived index unavailable".into(),
            ));
        };
        let hooks = IndexHooks::current();
        let pool = if pending.iter().any(|(key, _)| key.starts_with("source:")) {
            Some(index_pool(&mut self.index_pool, &hooks)?)
        } else {
            None
        };
        let memory = tx.open_table(MEMORY)?;
        let briefs = tx.open_table(BRIEFS)?;
        let SearchHandles {
            reader,
            writer,
            fields,
        } = handles;
        // Every page job has finished here. A page stopped at a hand-out,
        // or failed while building or adding, returns here uncommitted; one
        // cancelled after its last hand-out is committed below and stops
        // after the commit.
        let panics = index_page(
            writer,
            fields,
            &pending,
            &sources,
            &stored,
            &memory,
            &briefs,
            control,
            &hooks,
            pool.as_deref(),
        )?;
        drop(memory);
        drop(briefs);
        index_fault!(INDEX_BEFORE_COMMIT, control, "")?;
        writer.commit()?;
        // The page's documents are committed: its parse panics are this
        // drain's named scan failures, whatever stops it from here on (a
        // replayed page names them again when it replays).
        for (position, message) in &panics {
            let key = &pending[*position].0;
            self.parse_panics
                .push(key.strip_prefix("source:").unwrap_or(key), message);
        }
        index_fault!(INDEX_BEFORE_RELOAD, control, "")?;
        reader.reload()?;
        drop(stored);
        drop(sources);
        drop(tx);
        // Boundary: search documents committed, pending work not yet cleared.
        fault!(
            INDEX_AFTER_SEARCH_COMMIT,
            Some(&*self),
            Some(control),
            &pending.len().to_string()
        )?;
        // Cooperative checkpoint between search commit and pending clear:
        // cancellation here leaves committed search plus durable pending work,
        // and replay is idempotent.
        control.check()?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(PENDING)?;
            for (path, hash) in &pending {
                let matches = table.get(path.as_str())?.is_some_and(|v| v.value() == hash);
                if matches {
                    table.remove(path.as_str())?;
                }
            }
        }
        tx.commit()?;
        // Preserve the public (sources, memory documents) counters. Briefs
        // also drain here, so (0, 0) alone does not prove the queue is empty.
        Ok((
            pending
                .iter()
                .filter(|(key, _)| key.starts_with("source:"))
                .count(),
            pending
                .iter()
                .filter(|(key, _)| key.starts_with("memory:"))
                .count(),
        ))
    }

    /// Drain pending index work in bounded batches until empty or cancelled.
    /// [`Self::take_parse_failures`] then names this drain's parse panics.
    pub fn refresh(&mut self, control: &crate::Control) -> FResult<(usize, usize)> {
        self.parse_panics = ParseFailures::default();
        let mut total = (0usize, 0usize);
        loop {
            // Cooperative cancellation between index batches.
            control.check()?;
            match self.refresh_index(control) {
                Ok((0, 0)) if self.pending()? == 0 => return Ok(total),
                Ok((sources, memories)) => {
                    total.0 += sources;
                    total.1 += memories;
                }
                Err(FoundryError::Cancelled(_) | FoundryError::DeadlineExceeded(_)) => {
                    return Err(FoundryError::Cancelled(None));
                }
                Err(e) => return Err(e),
            }
        }
    }
}
