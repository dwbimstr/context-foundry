//! 001 T011: bounded filesystem hints inside the admitted primary owner.
//! notify owns OS registration; source reads/commits reuse ingest and redb.
//! The queue is disposable: every owner start reconciles the root, so an abrupt
//! exit cannot make an in-memory hint the only record of required recovery.
use crate::ingest::{ChangeCounts, ChangeReader, ChangeScan};
use crate::{Control, Engine, FResult, FoundryError};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_PATHS: usize = 2048;
const MAX_PATH_BYTES: usize = 1024 * 1024;
const QUIET: Duration = Duration::from_millis(75);
const MAX_DEBOUNCE: Duration = Duration::from_secs(1);
const INDEX_PAGE: usize = 8;

/// The existing owner supplies its one foreground-prioritized engine slot.
pub(crate) trait Owner: Send + Sync {
    fn try_watch_step(&self, step: &mut dyn FnMut(&mut Engine)) -> FResult<bool>;
    fn closing(&self) -> bool;
}

#[derive(Clone, Serialize)]
struct Status {
    phase: &'static str,
    events: u64,
    coalesced: u64,
    overflows: u64,
    full_reconciliations: u64,
    scoped_reconciliations: u64,
    source_reads: u64,
    changed: u64,
    deleted: u64,
    excluded: u64,
    last_success_unix_ms: Option<u64>,
    last_error: Option<String>,
    backend_error: Option<String>,
}
struct Queue {
    paths: BTreeSet<String>,
    bytes: usize,
    dirty: bool,
    epoch: u64,
    foreground_generation: u64,
    first: Instant,
    last: Instant,
    status: Status,
}
impl Queue {
    fn dirty(&mut self, overflow: bool) {
        self.paths.clear();
        self.bytes = 0;
        self.dirty = true;
        self.epoch = self.epoch.wrapping_add(1);
        if overflow {
            self.status.overflows = self.status.overflows.saturating_add(1);
        }
    }
    fn insert(&mut self, path: String) {
        if self.dirty {
            self.status.coalesced = self.status.coalesced.saturating_add(1);
            return;
        }
        if self.paths.iter().any(|p| {
            path == *p
                || path
                    .strip_prefix(p)
                    .is_some_and(|rest| rest.starts_with('/'))
        }) {
            self.status.coalesced = self.status.coalesced.saturating_add(1);
            return;
        }
        let prefix = format!("{path}/");
        self.paths.retain(|p| !p.starts_with(&prefix));
        self.bytes = self.paths.iter().map(String::len).sum();
        if self.paths.len() == MAX_PATHS || self.bytes.saturating_add(path.len()) > MAX_PATH_BYTES {
            self.dirty(true);
            return;
        }
        self.bytes += path.len();
        self.paths.insert(path);
    }
}

pub(crate) struct Watch {
    root: PathBuf,
    store: PathBuf,
    queue: Mutex<Queue>,
    boundary: Mutex<Option<crate::boundary::maintenance::Maintenance>>,
    wake: Condvar,
    stop: Control,
    running: AtomicBool,
    #[cfg(test)]
    contention_wait: Mutex<Option<std::sync::mpsc::Sender<()>>>,
}
impl Watch {
    pub(crate) fn new(root: PathBuf, store: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            root,
            store,
            boundary: Mutex::new(None),
            queue: Mutex::new(Queue {
                paths: BTreeSet::new(),
                bytes: 0,
                dirty: true,
                epoch: 0,
                foreground_generation: 0,
                first: Instant::now(),
                last: Instant::now(),
                status: Status {
                    phase: "starting",
                    events: 0,
                    coalesced: 0,
                    overflows: 0,
                    full_reconciliations: 0,
                    scoped_reconciliations: 0,
                    source_reads: 0,
                    changed: 0,
                    deleted: 0,
                    excluded: 0,
                    last_success_unix_ms: None,
                    last_error: None,
                    backend_error: None,
                },
            }),
            wake: Condvar::new(),
            stop: Control::unbounded(),
            running: AtomicBool::new(false),
            #[cfg(test)]
            contention_wait: Mutex::new(None),
        })
    }
    pub(crate) fn start(
        self: &Arc<Self>,
        owner: Arc<dyn Owner>,
        reader: Arc<ChangeReader>,
    ) -> FResult<()> {
        let state = Arc::clone(self);
        let mut backend = RecommendedWatcher::new(
            move |event| state.event(event),
            Config::default().with_follow_symlinks(false),
        )
        .map_err(watch_error)?;
        backend
            .watch(&self.root, RecursiveMode::Recursive)
            .map_err(watch_error)?;
        self.running.store(true, Ordering::SeqCst);
        let state = Arc::clone(self);
        if let Err(error) = std::thread::Builder::new()
            .name("foundry-watch".into())
            .spawn(move || {
                let _finish = Running(Arc::clone(&state));
                let _backend = backend;
                state.drive(owner.as_ref(), reader);
            })
        {
            self.running.store(false, Ordering::SeqCst);
            return Err(error.into());
        }
        Ok(())
    }
    pub(crate) fn cancel(&self) {
        self.stop.cancel();
        self.wake.notify_all();
    }
    /// The last foreground engine operation released its slot. This is only a
    /// scheduling hint: it creates no source work and changes no admission rule.
    pub(crate) fn foreground_released(&self) {
        let mut queue = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.stop.is_cancelled() {
            return;
        }
        queue.foreground_generation = queue.foreground_generation.wrapping_add(1);
        self.wake.notify_all();
    }
    pub(crate) fn foreground_generation(&self) -> u64 {
        self.queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .foreground_generation
    }
    /// Capture the generation before trying the owner slot, then compare under
    /// the condvar's mutex. A release before waiting cannot become a lost wake.
    /// The return value says whether the retry interval actually elapsed.
    pub(crate) fn wait_after_contention(&self, generation: u64, time: Duration) -> bool {
        let queue = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.stop.is_cancelled() || queue.foreground_generation != generation {
            return false;
        }
        #[cfg(test)]
        if let Some(armed) = self.contention_wait.lock().unwrap().take() {
            // Signal while holding the queue lock: a concurrent release must
            // wait until Condvar atomically unlocks it and starts waiting.
            armed.send(()).unwrap();
        }
        self.wake
            .wait_timeout(queue, time)
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .1
            .timed_out()
    }
    #[cfg(test)]
    pub(crate) fn observe_contention_wait(&self, armed: std::sync::mpsc::Sender<()>) {
        *self.contention_wait.lock().unwrap() = Some(armed);
    }
    pub(crate) fn idle(&self) -> bool {
        !self.running.load(Ordering::SeqCst)
    }
    /// Source work is pending: semantic steps yield eight source steps and
    /// knowledge calls wait for source work, with foreground operations first.
    pub(crate) fn source_pending(&self) -> bool {
        if self.idle() || self.stop.is_cancelled() {
            return false;
        }
        let q = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        q.dirty
            || !q.paths.is_empty()
            || matches!(
                q.status.phase,
                "starting" | "reconciling" | "updating" | "indexing"
            )
    }
    pub(crate) fn status(&self, pending_index: u64) -> serde_json::Value {
        let q = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut value = serde_json::to_value(&q.status).expect("watch status serializes");
        value["pending_paths"] = q.paths.len().into();
        value["pending_path_bytes"] = q.bytes.into();
        value["dirty_root"] = q.dirty.into();
        value["pending_index"] = pending_index.into();
        value["running"] = (!self.idle()).into();
        drop(q);
        if let Some(boundary) = self
            .boundary
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            value["boundary"] = boundary.status();
        }
        value
    }
    pub(crate) fn boundary_maintenance(&self, config: String, enabled: bool) -> FResult<()> {
        let mut state = self
            .boundary
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if enabled {
            *state = Some(crate::boundary::maintenance::Maintenance::new(config));
        } else if let Some(state) = state.as_mut() {
            state.pause(&config)?;
        }
        self.wake.notify_all();
        Ok(())
    }
    fn boundary_step(&self, engine: &Engine) -> bool {
        self.boundary
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_mut()
            .is_some_and(|state| state.step(engine, &self.stop))
    }
    fn event(&self, result: notify::Result<Event>) {
        if self.stop.is_cancelled() {
            return;
        }
        let mut q = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let event = match result {
            Ok(event) => event,
            Err(error) => {
                q.status.backend_error = Some(bounded(&error.to_string()));
                q.dirty(true);
                q.last = Instant::now();
                self.wake.notify_all();
                return;
            }
        };
        // Indexing/queries generate read access events; those cannot create work.
        if matches!(event.kind, EventKind::Access(kind) if !matches!(kind,
            notify::event::AccessKind::Close(notify::event::AccessMode::Write)))
        {
            return;
        }
        q.status.events = q.status.events.saturating_add(1);
        if q.paths.is_empty() && !q.dirty {
            q.first = Instant::now();
        }
        if event.need_rescan() || event.paths.is_empty() {
            q.dirty(true);
        } else {
            for path in &event.paths {
                if path.starts_with(&self.store) {
                    continue;
                }
                let Ok(relative) = path.strip_prefix(&self.root) else {
                    continue;
                };
                if relative.components().any(|c| {
                    matches!(
                        c.as_os_str().to_str(),
                        Some("target" | "node_modules" | ".context-foundry")
                    )
                }) {
                    continue;
                }
                if policy_hint(relative) {
                    q.dirty(false);
                    continue;
                }
                if relative.components().any(|c| c.as_os_str() == ".git") {
                    continue;
                }
                if relative.as_os_str().is_empty() {
                    q.dirty(false);
                    continue;
                }
                let Some(path) = relative
                    .to_str()
                    .filter(|p| crate::store::validate_path(p).is_ok())
                else {
                    q.dirty(true);
                    continue;
                };
                q.insert(path.to_owned());
            }
        }
        q.last = Instant::now();
        self.wake.notify_all();
    }
    fn pause(&self, time: Duration) {
        let guard = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = self
            .wake
            .wait_timeout(guard, time)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
    }
    fn phase(&self, phase: &'static str) {
        self.queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .status
            .phase = phase;
    }
    fn counts(&self, counts: ChangeCounts) {
        let mut q = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        q.status.source_reads = q.status.source_reads.saturating_add(counts.read);
        q.status.changed = q.status.changed.saturating_add(counts.changed);
        q.status.deleted = q.status.deleted.saturating_add(counts.deleted);
        q.status.excluded = q.status.excluded.saturating_add(counts.excluded);
    }
    fn drive(&self, owner: &dyn Owner, reader: Arc<ChangeReader>) {
        let mut scan: Option<ChangeScan> = None;
        let mut scan_epoch = 0;
        let mut drain = true;
        let mut retry_at = Instant::now();
        let mut failures = 0u32;
        let mut source_steps = 0usize;
        let mut boundary_turn = 0usize;
        while !self.stop.is_cancelled() && !owner.closing() {
            if Instant::now() < retry_at {
                self.pause(Duration::from_millis(100));
                continue;
            }
            let mut result = Ok(());
            let mut idle = false;
            let foreground_generation = self.foreground_generation();
            let admitted = owner.try_watch_step(&mut |engine| {
                result = (|| -> FResult<()> {
                    self.stop.check()?;
                    boundary_turn += 1;
                    if boundary_turn >= 8 || !self.source_pending() {
                        boundary_turn = 0;
                        if self.boundary_step(engine) {
                            return Ok(());
                        }
                    }
                    // A policy change/overflow supersedes cached walk rules. Never
                    // sweep with an old admission view. Foreground explicit scans
                    // are detected by ChangeScan's scan-ID check.
                    let epoch = self
                        .queue
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .epoch;
                    if scan.is_some() && epoch != scan_epoch {
                        scan.take().expect("scan exists").abort(engine)?;
                    }
                    // Publish periodically even under a continuous stream of
                    // known-file edits. Queue quiet is not a readiness barrier.
                    if source_steps >= 32 {
                        source_steps = 0;
                        if engine.pending()? > 0 {
                            self.phase("indexing");
                            engine.refresh_index_page(&self.stop, INDEX_PAGE)?;
                            return Ok(());
                        }
                    }
                    if let Some(active) = &mut scan {
                        self.phase("reconciling");
                        let (counts, done) = active.step(engine, &self.stop)?;
                        self.counts(counts);
                        source_steps += 1;
                        if done {
                            scan = None;
                            drain = true;
                        }
                        return Ok(());
                    }
                    let (work, work_epoch) = {
                        let mut q = self
                            .queue
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        let ready = q.last.elapsed() >= QUIET || q.first.elapsed() >= MAX_DEBOUNCE;
                        if !ready && (q.dirty || !q.paths.is_empty()) {
                            idle = true;
                            return Ok(());
                        }
                        let work = if q.dirty {
                            q.dirty = false;
                            q.status.phase = "reconciling";
                            q.status.full_reconciliations =
                                q.status.full_reconciliations.saturating_add(1);
                            Some(None)
                        } else {
                            q.paths.pop_first().map(|path| {
                                q.bytes -= path.len();
                                q.status.phase = "updating";
                                Some(path)
                            })
                        };
                        (work, q.epoch)
                    };
                    if let Some(scope) = work {
                        if let Some(path) = &scope
                            && reader.direct(engine, path)?
                        {
                            self.phase("updating");
                            self.counts(reader.update(engine, path, &self.stop)?);
                            source_steps += 1;
                            drain = true;
                        } else {
                            self.phase("reconciling");
                            if scope.is_some() {
                                let mut q = self
                                    .queue
                                    .lock()
                                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                                q.status.scoped_reconciliations =
                                    q.status.scoped_reconciliations.saturating_add(1);
                            }
                            scan_epoch = work_epoch;
                            scan = Some(ChangeScan::start(engine, Arc::clone(&reader), scope)?);
                        }
                        return Ok(());
                    }
                    if drain {
                        self.phase("indexing");
                        if engine.pending()? > 0 {
                            engine.refresh_index_page(&self.stop, INDEX_PAGE)?;
                            let failures = engine.take_parse_failures();
                            if failures.count > 0 {
                                return Err(FoundryError::InvalidArgument(format!(
                                    "watch index: {} parse failures",
                                    failures.count
                                )));
                            }
                            return Ok(());
                        }
                        drain = false;
                        failures = 0;
                        let mut q = self
                            .queue
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        q.status.last_success_unix_ms = Some(now_ms());
                        q.status.last_error = None;
                    }
                    self.phase("idle");
                    idle = true;
                    Ok(())
                })();
                if result.is_err()
                    && let Some(active) = scan.take()
                {
                    let _ = active.abort(engine);
                }
            });
            let contended = matches!(admitted, Ok(false));
            if let Err(error) = admitted {
                result = Err(error);
            }
            if let Err(error) = result {
                if self.stop.is_cancelled() || owner.closing() {
                    break;
                }
                failures = failures.saturating_add(1);
                let seconds = (1u64 << failures.min(5)).min(30);
                retry_at = Instant::now() + Duration::from_secs(seconds);
                let mut q = self
                    .queue
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                q.dirty(false);
                q.status.last_error = Some(bounded(&error.to_string()));
                q.status.phase = "backoff";
                drain = true;
            } else if contended {
                self.wait_after_contention(foreground_generation, Duration::from_millis(25));
            } else if idle {
                self.pause(Duration::from_millis(25));
            } else {
                std::thread::yield_now();
            }
        }
        // No scan can be published as complete on shutdown. A full scan was
        // already marked incomplete when it started. A closing owner refuses
        // all watch admission, so do not retry an impossible cleanup slot;
        // next startup always reconciles. A separately cancelled watcher can
        // still record its abort while its owner remains available.
        if let Some(active) = scan {
            for _ in 0..1000 {
                if owner.closing() {
                    break;
                }
                match owner.try_watch_step(&mut |engine| {
                    let _ = active.abort(engine);
                }) {
                    Ok(false) if !owner.closing() => self.pause(Duration::from_millis(5)),
                    _ => break,
                }
            }
        }
    }
}
fn policy_hint(path: &Path) -> bool {
    if path.components().any(|c| c.as_os_str() == ".git") {
        // Git metadata inside nested repositories can also affect admission.
        // Arbitrary object/index contents are never ignore-policy hints.
        return [".git", ".git/info/exclude", ".git/config", ".git/info"]
            .iter()
            .any(|suffix| path.ends_with(suffix));
    }
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some(".gitignore" | ".ignore")
    )
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
fn bounded(value: &str) -> String {
    let mut end = value.len().min(512);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].into()
}
fn watch_error(error: notify::Error) -> FoundryError {
    FoundryError::InvalidArgument(format!(
        "watch registration: {}",
        bounded(&error.to_string())
    ))
}
struct Running(Arc<Watch>);
impl Drop for Running {
    fn drop(&mut self) {
        let mut q = self
            .0
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if std::thread::panicking() {
            q.status.last_error = Some("watch worker panicked".into());
        }
        q.status.phase = "stopped";
        self.0.running.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fault::{self, Action, names};
    use serde_json::json;
    use std::fs;

    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
        engine: Engine,
    }
    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join(".git")).unwrap();
        let engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
        Fixture {
            _dir: dir,
            root,
            engine,
        }
    }
    fn scan(f: &Fixture, scope: Option<&str>) -> FResult<()> {
        let reader = Arc::new(ChangeReader::new(&f.engine, &f.root)?);
        let mut scan = ChangeScan::start(&f.engine, reader, scope.map(str::to_owned))?;
        while !scan.step(&f.engine, &Control::unbounded())?.1 {}
        Ok(())
    }
    fn event(root: &Path, path: &str) -> Event {
        Event::new(EventKind::Modify(notify::event::ModifyKind::Any)).add_path(root.join(path))
    }
    #[test]
    fn bursts_coalesce_by_subtree_and_overflow_to_one_bounded_root_marker() {
        let watch = Watch::new("/fixture".into(), "/fixture/.context-foundry".into());
        watch.queue.lock().unwrap().dirty = false;
        for _ in 0..10000 {
            watch.event(Ok(event(&watch.root, "src/a.rs")));
        }
        assert_eq!(watch.status(0)["pending_paths"], 1);
        watch.event(Ok(event(&watch.root, "src")));
        assert_eq!(watch.queue.lock().unwrap().paths.first().unwrap(), "src");
        for i in 0..MAX_PATHS + 1 {
            watch.event(Ok(event(&watch.root, &format!("files/{i}.rs"))));
        }
        let status = watch.status(0);
        assert_eq!(status["dirty_root"], true);
        assert_eq!(status["pending_paths"], 0);
        assert_eq!(status["overflows"], 1);
        assert_eq!(status["pending_path_bytes"], 0);
        watch.queue.lock().unwrap().dirty = false;
        for i in 0..400 {
            watch.event(Ok(event(&watch.root, &format!("{i}/{}", "x".repeat(4000)))));
        }
        assert!(watch.status(0)["overflows"].as_u64().unwrap() >= 2);
        assert!(watch.status(0)["pending_path_bytes"].as_u64().unwrap() <= MAX_PATH_BYTES as u64);
    }
    #[test]
    fn read_access_store_noise_and_outside_hints_do_not_create_work() {
        let watch = Watch::new("/fixture".into(), "/fixture/data".into());
        watch.queue.lock().unwrap().dirty = false;
        watch.event(Ok(Event::new(EventKind::Access(
            notify::event::AccessKind::Read,
        ))
        .add_path("/fixture/source.rs".into())));
        for path in [
            "/fixture/data/search/1",
            "/other/a.rs",
            "/fixture/target/build/a",
            "/fixture/target/build/.ignore",
            "/fixture/node_modules/pkg/.gitignore",
            "/fixture/.git/objects/a",
            "/fixture/.git/objects/.ignore",
        ] {
            watch.event(Ok(Event::new(EventKind::Any).add_path(path.into())));
        }
        assert_eq!(watch.status(0)["pending_paths"], 0);
        assert_eq!(watch.status(0)["dirty_root"], false);
        watch.event(Ok(event(&watch.root, ".git/info/exclude")));
        assert_eq!(watch.status(0)["dirty_root"], true);
        watch.queue.lock().unwrap().dirty = false;
        watch.event(Ok(event(&watch.root, "nested/.git/info/exclude")));
        assert_eq!(watch.status(0)["dirty_root"], true);
    }
    #[test]
    fn scoped_walks_preserve_ignore_precedence_and_delete_only_the_target_subtree() {
        let mut f = fixture();
        fs::create_dir(f.root.join("src")).unwrap();
        fs::create_dir(f.root.join("src_extra")).unwrap();
        fs::write(f.root.join(".gitignore"), "*.tmp\n.hidden\n").unwrap();
        fs::write(f.root.join(".ignore"), "!.hidden\n").unwrap();
        for path in ["src/a.rs", "src/b.tmp", "src_extra/keep.rs", ".hidden"] {
            fs::write(f.root.join(path), "pub fn kept() {}\n").unwrap();
        }
        f.engine.index(&f.root, &Control::unbounded()).unwrap();
        fs::write(f.root.join("src/new.tmp"), "excluded\n").unwrap();
        scan(&f, Some("src/new.tmp")).unwrap();
        assert!(f.engine.source("src/new.tmp").unwrap().is_none());
        assert!(f.engine.source(".hidden").unwrap().is_some());
        fs::remove_dir_all(f.root.join("src")).unwrap();
        scan(&f, Some("src")).unwrap();
        assert!(f.engine.source("src/a.rs").unwrap().is_none());
        assert!(f.engine.source("src_extra/keep.rs").unwrap().is_some());
        assert_eq!(f.engine.status().unwrap().scan_state, "complete");
        // A newly created file gets the same whitelist rule as explicit index.
        f.engine.delete_source(".hidden").unwrap();
        scan(&f, Some(".hidden")).unwrap();
        assert!(f.engine.source(".hidden").unwrap().is_some());
    }
    #[test]
    fn incomplete_walk_preserves_unseen_sources_and_superseded_scan_cannot_sweep() {
        let mut f = fixture();
        fs::write(f.root.join("gone.rs"), "fn gone() {}\n").unwrap();
        fs::write(f.root.join("bad.rs"), "fn bad() {}\n").unwrap();
        f.engine.index(&f.root, &Control::unbounded()).unwrap();
        fs::remove_file(f.root.join("gone.rs")).unwrap();
        fault::arm(
            names::WATCH_BEFORE_OPEN,
            0,
            Action::FailWhen {
                detail_contains: "bad.rs".into(),
                message: "unreadable".into(),
            },
        );
        assert!(scan(&f, None).is_err());
        fault::disarm_all();
        assert!(f.engine.source("gone.rs").unwrap().is_some());
        assert_eq!(f.engine.status().unwrap().scan_state, "incomplete");
        let mut old = ChangeScan::start(
            &f.engine,
            Arc::new(ChangeReader::new(&f.engine, &f.root).unwrap()),
            None,
        )
        .unwrap();
        scan(&f, None).unwrap();
        assert!(matches!(
            old.step(&f.engine, &Control::unbounded()),
            Err(FoundryError::Conflict(_))
        ));
        old.abort(&f.engine).unwrap();
        assert_eq!(f.engine.status().unwrap().scan_state, "complete");
        assert!(f.engine.source("gone.rs").unwrap().is_none());
    }
    #[test]
    fn edited_during_read_and_cancelled_refresh_keep_prior_source() {
        let mut f = fixture();
        fs::write(f.root.join("a.rs"), "fn old() {}\n").unwrap();
        f.engine.index(&f.root, &Control::unbounded()).unwrap();
        let reader = ChangeReader::new(&f.engine, &f.root).unwrap();
        let old = f.engine.source("a.rs").unwrap().unwrap().hash;
        let path = f.root.join("a.rs");
        fault::arm(
            names::WATCH_AFTER_READ,
            0,
            Action::Call(Box::new(move |_| {
                fs::write(&path, "fn new() {}\n").unwrap();
            })),
        );
        assert!(
            reader
                .update(&f.engine, "a.rs", &Control::unbounded())
                .is_err()
        );
        fault::disarm_all();
        assert_eq!(f.engine.source("a.rs").unwrap().unwrap().hash, old);
        assert!(
            reader
                .update(&f.engine, "a.rs", &Control::cancelled())
                .is_err()
        );
        assert_eq!(f.engine.source("a.rs").unwrap().unwrap().hash, old);
        reader
            .update(&f.engine, "a.rs", &Control::unbounded())
            .unwrap();
        assert_ne!(f.engine.source("a.rs").unwrap().unwrap().hash, old);
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_and_root_replacement_never_import_outside_bytes() {
        use std::os::unix::fs::symlink;
        let mut f = fixture();
        fs::write(f.root.join("a.rs"), "fn admitted() {}\n").unwrap();
        f.engine.index(&f.root, &Control::unbounded()).unwrap();
        let outside = f._dir.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("a.rs"), "fn outside_secret() {}\n").unwrap();
        fs::remove_file(f.root.join("a.rs")).unwrap();
        symlink(outside.join("a.rs"), f.root.join("a.rs")).unwrap();
        scan(&f, Some("a.rs")).unwrap();
        assert!(f.engine.source("a.rs").unwrap().is_none());
        let reader = ChangeReader::new(&f.engine, &f.root).unwrap();
        fs::rename(&f.root, f._dir.path().join("moved")).unwrap();
        symlink(&outside, &f.root).unwrap();
        assert!(
            reader
                .update(&f.engine, "a.rs", &Control::unbounded())
                .is_err()
        );
        assert!(f.engine.source("a.rs").unwrap().is_none());
    }
    struct TestOwner {
        engine: Mutex<Engine>,
        closing: AtomicBool,
    }
    impl Owner for TestOwner {
        fn try_watch_step(&self, step: &mut dyn FnMut(&mut Engine)) -> FResult<bool> {
            let Ok(mut engine) = self.engine.try_lock() else {
                return Ok(false);
            };
            step(&mut engine);
            Ok(true)
        }
        fn closing(&self) -> bool {
            self.closing.load(Ordering::SeqCst)
        }
    }
    fn wait(watch: &Watch, predicate: impl Fn(&serde_json::Value) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let status = watch.status(0);
            if predicate(&status) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "watch did not converge: {status}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn busy_changed_path_queue_still_publishes_lexical_progress() {
        struct Observed {
            base: TestOwner,
            watch: Arc<Watch>,
            published_with_hints: AtomicBool,
        }
        impl Owner for Observed {
            fn try_watch_step(&self, step: &mut dyn FnMut(&mut Engine)) -> FResult<bool> {
                self.base.try_watch_step(&mut |engine| {
                    step(engine);
                    if self.watch.status(0)["pending_paths"].as_u64().unwrap() > 0
                        && !engine.search("after_change", 8).unwrap().hits.is_empty()
                    {
                        self.published_with_hints.store(true, Ordering::SeqCst);
                    }
                })
            }
            fn closing(&self) -> bool {
                false
            }
        }
        let mut f = fixture();
        for i in 0..48 {
            fs::write(
                f.root.join(format!("f{i:02}.rs")),
                "fn before_change() {}\n",
            )
            .unwrap();
        }
        f.engine.index(&f.root, &Control::unbounded()).unwrap();
        let reader = Arc::new(ChangeReader::new(&f.engine, &f.root).unwrap());
        let watch = Watch::new(f.root.clone(), f.engine.directory().to_owned());
        watch.queue.lock().unwrap().dirty = false;
        for i in 0..48 {
            let path = format!("f{i:02}.rs");
            fs::write(f.root.join(&path), "fn after_change() {}\n").unwrap();
            watch.event(Ok(event(&f.root, &path)));
        }
        let owner = Arc::new(Observed {
            base: TestOwner {
                engine: Mutex::new(f.engine),
                closing: AtomicBool::new(false),
            },
            watch: watch.clone(),
            published_with_hints: AtomicBool::new(false),
        });
        watch.running.store(true, Ordering::SeqCst);
        let worker = std::thread::spawn({
            let (watch, owner) = (watch.clone(), owner.clone());
            move || {
                let _finish = Running(watch.clone());
                watch.drive(owner.as_ref(), reader);
            }
        });
        wait(&watch, |s| s["phase"] == "idle");
        // Foreground observation may finish while the watcher is idle. These
        // scheduling wakes must never become filesystem hints or body rescans.
        for _ in 0..16 {
            watch.foreground_released();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(watch.status(0)["source_reads"], 48);
        assert_eq!(watch.status(0)["pending_paths"], 0);
        assert_eq!(watch.status(0)["dirty_root"], false);
        watch.cancel();
        worker.join().unwrap();
        assert!(
            owner.published_with_hints.load(Ordering::SeqCst),
            "search must progress before hints drain"
        );
        assert_eq!(watch.status(0)["source_reads"], 48);
        assert_eq!(watch.status(0)["full_reconciliations"], 0);
    }

    #[test]
    fn owner_shutdown_skips_refused_cleanup_and_restart_reconciles_the_incomplete_scan() {
        struct ClosingOwner {
            engine: Mutex<Engine>,
            closing: AtomicBool,
            initial_scan: u64,
            refused_cleanup: std::sync::atomic::AtomicUsize,
        }
        impl Owner for ClosingOwner {
            fn try_watch_step(&self, step: &mut dyn FnMut(&mut Engine)) -> FResult<bool> {
                if self.closing() {
                    self.refused_cleanup.fetch_add(1, Ordering::SeqCst);
                    return Ok(false);
                }
                let mut engine = self.engine.lock().unwrap();
                step(&mut engine);
                // Close at the actual boundary immediately after ChangeScan
                // starts, before it can visit or sweep any source.
                if engine.current_scan_id()? != self.initial_scan {
                    self.closing.store(true, Ordering::SeqCst);
                }
                Ok(true)
            }
            fn closing(&self) -> bool {
                self.closing.load(Ordering::SeqCst)
            }
        }
        let mut f = fixture();
        fs::write(f.root.join("a.rs"), "fn before() {}\n").unwrap();
        fs::write(f.root.join("removed.rs"), "fn old() {}\n").unwrap();
        f.engine.index(&f.root, &Control::unbounded()).unwrap();
        let initial_scan = f.engine.current_scan_id().unwrap();
        let store = f.engine.directory().to_owned();
        fs::write(f.root.join("a.rs"), "fn after() {}\n").unwrap();
        fs::remove_file(f.root.join("removed.rs")).unwrap();
        let reader = Arc::new(ChangeReader::new(&f.engine, &f.root).unwrap());
        let watch = Watch::new(f.root.clone(), store.clone());
        let owner = ClosingOwner {
            engine: Mutex::new(f.engine),
            closing: AtomicBool::new(false),
            initial_scan,
            refused_cleanup: std::sync::atomic::AtomicUsize::new(0),
        };
        watch.running.store(true, Ordering::SeqCst);
        {
            let _finish = Running(Arc::clone(&watch));
            watch.drive(&owner, reader);
        }
        assert!(owner.closing());
        assert!(watch.idle());
        assert_eq!(
            owner.refused_cleanup.load(Ordering::SeqCst),
            0,
            "a closing owner can never admit abort; do not poll it 1000 times"
        );
        assert_eq!(watch.status(0)["source_reads"], 0);
        let engine = owner.engine.into_inner().unwrap();
        assert_eq!(engine.status().unwrap().scan_state, "incomplete");
        assert!(
            engine.source("removed.rs").unwrap().is_some(),
            "no interrupted sweep"
        );
        drop(engine);
        let mut reopened = Engine::open_existing(&store).unwrap();
        assert_eq!(reopened.status().unwrap().scan_state, "incomplete");
        reopened.index(&f.root, &Control::unbounded()).unwrap();
        assert_eq!(reopened.status().unwrap().scan_state, "complete");
        assert_eq!(reopened.status().unwrap().source_count, 1);
        assert!(reopened.source("removed.rs").unwrap().is_none());
        assert_eq!(
            reopened.source("a.rs").unwrap().unwrap().hash,
            crate::digest(b"fn after() {}\n")
        );
    }
    #[test]
    fn lost_hint_reconciliation_and_restart_recover_changes_without_a_durable_event_queue() {
        let f = fixture();
        fs::write(f.root.join("a.rs"), "fn old() {}\n").unwrap();
        let reader = Arc::new(ChangeReader::new(&f.engine, &f.root).unwrap());
        let watch = Watch::new(f.root.clone(), f.engine.directory().to_owned());
        let owner = Arc::new(TestOwner {
            engine: Mutex::new(f.engine),
            closing: AtomicBool::new(false),
        });
        // Run the real driver without a native backend: deliberately lose a hint.
        watch.running.store(true, Ordering::SeqCst);
        let worker = {
            let watch = watch.clone();
            let owner = owner.clone();
            let reader = reader.clone();
            std::thread::spawn(move || {
                let _finish = Running(watch.clone());
                watch.drive(owner.as_ref(), reader);
            })
        };
        wait(&watch, |s| s["phase"] == "idle");
        fs::write(f.root.join("a.rs"), "fn after_loss() {}\n").unwrap();
        std::thread::sleep(Duration::from_millis(125));
        assert_eq!(watch.status(0)["source_reads"], 1);
        watch.event(Ok(
            Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan)
        ));
        wait(&watch, |s| {
            s["phase"] == "idle" && s["full_reconciliations"] == 2
        });
        assert_eq!(
            owner
                .engine
                .lock()
                .unwrap()
                .source("a.rs")
                .unwrap()
                .unwrap()
                .hash,
            crate::digest(b"fn after_loss() {}\n")
        );
        watch.cancel();
        worker.join().unwrap();
        fs::write(f.root.join("a.rs"), "fn after_restart() {}\n").unwrap();
        let watch2 = Watch::new(
            f.root.clone(),
            owner.engine.lock().unwrap().directory().to_owned(),
        );
        watch2.start(owner.clone(), reader).unwrap();
        wait(&watch2, |s| s["phase"] == "idle");
        assert_eq!(
            owner
                .engine
                .lock()
                .unwrap()
                .source("a.rs")
                .unwrap()
                .unwrap()
                .hash,
            crate::digest(b"fn after_restart() {}\n")
        );
        watch2.cancel();
        wait(&watch2, |s| s["phase"] == "stopped");
    }
    #[cfg(unix)]
    #[test]
    fn named_read_failure_uses_backoff_and_does_not_retire_prior_records() {
        let f = fixture();
        f.engine
            .replace_source("kept.rs", "fn preserved() {}\n")
            .unwrap();
        // APFS rejects non-UTF-8 filenames before the watcher can inspect them.
        // A newline is valid in Unix filenames but violates Foundry's source-path
        // contract, so both macOS and Linux exercise a real failed reconciliation.
        fs::write(f.root.join("bad\npath.rs"), "bad path").unwrap();
        let reader = Arc::new(ChangeReader::new(&f.engine, &f.root).unwrap());
        let watch = Watch::new(f.root.clone(), f.engine.directory().to_owned());
        let owner = Arc::new(TestOwner {
            engine: Mutex::new(f.engine),
            closing: AtomicBool::new(false),
        });
        watch.start(owner.clone(), reader).unwrap();
        wait(&watch, |s| s["phase"] == "backoff");
        let before = watch.status(0);
        assert!(
            before["last_error"]
                .as_str()
                .unwrap()
                .contains("source path contains forbidden control characters")
        );
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(
            watch.status(0)["full_reconciliations"],
            before["full_reconciliations"]
        );
        assert!(
            owner
                .engine
                .lock()
                .unwrap()
                .source("kept.rs")
                .unwrap()
                .is_some()
        );
        watch.cancel();
        wait(&watch, |s| s["phase"] == "stopped");
    }
    #[test]
    fn publishing_a_change_invalidates_derived_briefs_before_lexical_drain() {
        let mut f = fixture();
        let body = "fn named() {}\n";
        fs::write(f.root.join("a.rs"), body).unwrap();
        f.engine.index(&f.root, &Control::unbounded()).unwrap();
        let wid = f.engine.workspace_id().unwrap();
        let handle = format!(
            "a.rs#0-{}@{}.{}",
            body.len(),
            &crate::digest(body.as_bytes())[..32],
            &wid[..16]
        );
        let claim = json!({"text":"synthetic purpose","citations":[0],"identifiers":["named"]});
        let request=crate::brief::parse(&serde_json::to_vec(&json!({"workspace_id":wid,"command":{"op":"put","brief":{
            "cited_spans":[handle],"recipe_sha256":"a".repeat(64),"model":"fixture","artifact_sha256":"b".repeat(64),
            "prompt_sha256":"c".repeat(64),"generated_at_unix_ms":1,"purpose":[claim],"behavior":[claim],
            "provides":[],"consumes":[],"constraints":[],"tests":[],"unknowns":[]}}})).unwrap()).unwrap();
        let r = f.engine.brief(&request, &Control::unbounded()).unwrap();
        f.engine.refresh(&Control::unbounded()).unwrap();
        fs::write(f.root.join("a.rs"), "fn changed() {}\n").unwrap();
        ChangeReader::new(&f.engine, &f.root)
            .unwrap()
            .update(&f.engine, "a.rs", &Control::unbounded())
            .unwrap();
        assert!(f.engine.pending().unwrap() > 0);
        let get = crate::brief::parse(
            &serde_json::to_vec(&json!({"workspace_id":wid,"command":{"op":"get","id":r["id"]}}))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            f.engine.brief(&get, &Control::unbounded()).unwrap()["eligible"],
            false
        );
        assert!(matches!(
            f.engine.retrieve(&handle, None, 100),
            Err(FoundryError::StaleHandle)
        ));
    }
}
