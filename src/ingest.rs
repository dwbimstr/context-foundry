//! Ordered workspace reconciliation with bounded read-ahead. Never collects all
//! paths, keys or reports; samples are bounded and counts are exact u64s.
//! Candidates are opened relative to a held root-directory descriptor,
//! refusing symlink components and non-regular files at open time.
use crate::Control;
use crate::error::{FResult, FoundryError, PartialIndexCounts};
use crate::store::Engine;
use serde::Serialize;
use std::io::Read;
use std::path::Path;

const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
const SAMPLE_LIMIT: usize = 20;
const SAMPLE_BYTES: usize = 512;
/// Seen-path commit page: the same 128 bound as every other paged walk.
const SEEN_PAGE: usize = 128;
/// At most eight reads/results, each containing at most 2 MiB plus one sentinel
/// byte. Completed but uncommitted results occupy the same slots as active reads.
const READ_AHEAD: usize = 8;
/// Up to four ready files share one durable source commit; the remaining slots keep
/// read-ahead bounded and able to overlap the one writer.
const COMMIT_BATCH: usize = 4;
/// Publish lexical progress before the whole enumeration has finished.
const PUBLISH_EVERY: usize = 128;

#[cfg(feature = "test-faults")]
pub mod pipeline_hooks {
    use std::cell::RefCell;
    use std::sync::Arc;

    #[derive(Debug)]
    pub enum Event<'a> {
        Reading(&'a str),
        Ready(&'a str),
        Queued(usize),
    }
    pub type Observer = Arc<dyn Fn(&Event<'_>) + Send + Sync>;
    thread_local! {
        static OBSERVER: RefCell<Option<Observer>> = const { RefCell::new(None) };
    }
    pub fn install(observer: Option<Observer>) {
        OBSERVER.with(|slot| *slot.borrow_mut() = observer);
    }
    pub(crate) fn current() -> Option<Observer> {
        OBSERVER.with(|slot| slot.borrow().clone())
    }
}

struct ReadFile {
    file: std::fs::File,
    version: std::fs::Metadata,
    content: Result<crate::store::PreparedSource, &'static str>,
}

struct PendingRead {
    path: String,
    result: std::sync::mpsc::Receiver<Result<ReadFile, OpenFailure>>,
}

#[derive(Default)]
struct ScanProgress {
    report: IndexReport,
    enumeration_failed: bool,
    cancel_code: Option<&'static str>,
    drain_reason: Option<&'static str>,
    pending: std::collections::VecDeque<PendingRead>,
    since_publish: usize,
}

impl ScanProgress {
    fn check(&mut self, control: &Control) -> bool {
        if control.check().is_err() {
            self.cancel_code = Some(control.last_check_error().unwrap_or("cancelled"));
            self.enumeration_failed = true;
            false
        } else {
            true
        }
    }

    fn failed_read(&mut self, path: &str, failure: OpenFailure) {
        self.enumeration_failed = true;
        self.report.failures += 1;
        push_sample(
            &mut self.report.failure_samples,
            format!("{path}: {}: {}", failure.code(), failure.detail()),
        );
    }

    fn next(
        &mut self,
        engine: &mut Engine,
        root_fd: &std::fs::File,
        canonical: &Path,
        control: &Control,
    ) -> FResult<bool> {
        if !self.check(control) {
            return Ok(false);
        }
        let mut accepted = Vec::new();
        // Opportunistic group sizes need not divide the publication interval.
        // Finish exactly at its boundary so early visibility/cancellation keep
        // their existing source-count contract.
        let until_publish = if self.drain_reason.is_none() {
            PUBLISH_EVERY.saturating_sub(self.since_publish).max(1)
        } else {
            COMMIT_BATCH
        };
        for _ in 0..self.pending.len().min(COMMIT_BATCH).min(until_publish) {
            let pending = self.pending.pop_front().expect("a queued read");
            let disconnected = || {
                Err(OpenFailure::Io(
                    "an ingestion worker exited without its result".into(),
                ))
            };
            let read = if accepted.is_empty() {
                pending.result.recv().unwrap_or_else(|_| disconnected())
            } else {
                // Batch ready neighbors, but never withhold a ready prefix while
                // the next ordered read is still running. Its receiver retains
                // its slot and is consumed first on the next turn.
                match pending.result.try_recv() {
                    Ok(read) => read,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        self.pending.push_front(pending);
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => disconnected(),
                }
            };
            if !self.check(control) {
                return Ok(false);
            }
            match read {
                Ok(read) => {
                    fault!(
                        SCAN_AFTER_READ,
                        Some(&*engine),
                        Some(control),
                        &pending.path
                    )?;
                    if !self.check(control) {
                        return Ok(false);
                    }
                    if read.content.is_err() {
                        // Keep exclusion-driven deletions in walker order. They
                        // retain their single-file transaction and final check.
                        if !self.commit_ready(engine, root_fd, canonical, control, &mut accepted)? {
                            return Ok(false);
                        }
                        if let Err(failure) = verify_read(&read, root_fd, canonical, &pending.path)
                        {
                            self.failed_read(&pending.path, failure);
                            continue;
                        }
                        let reason = read.content.err().expect("excluded file");
                        self.report.deleted += u64::from(engine.delete_source(&pending.path)?);
                        self.report.excluded += 1;
                        push_sample(
                            &mut self.report.exclusion_samples,
                            format!("{}: {reason}", pending.path),
                        );
                        self.since_publish += 1;
                    } else {
                        accepted.push((pending.path, read));
                    }
                }
                Err(failure) => self.failed_read(&pending.path, failure),
            }
        }
        if !self.commit_ready(engine, root_fd, canonical, control, &mut accepted)? {
            return Ok(false);
        }
        if self.since_publish >= PUBLISH_EVERY && self.drain_reason.is_none() {
            self.since_publish = 0;
            let result = engine.refresh_index(control);
            record_parse_failures(engine, &mut self.report);
            match result {
                Ok(_) => {}
                Err(FoundryError::Cancelled(_) | FoundryError::DeadlineExceeded(_)) => {
                    self.check(control);
                    return Ok(false);
                }
                Err(FoundryError::RepairRequired(_)) => self.drain_reason = Some("repair_required"),
                Err(error) => return Err(error),
            }
        }
        Ok(true)
    }
    fn commit_ready(
        &mut self,
        engine: &Engine,
        root_fd: &std::fs::File,
        canonical: &Path,
        control: &Control,
        ready: &mut Vec<(String, ReadFile)>,
    ) -> FResult<bool> {
        let mut sources = Vec::new();
        let mut held = Vec::new();
        for (path, read) in ready.drain(..) {
            if !self.check(control) {
                return Ok(false);
            }
            if let Err(failure) = verify_read(&read, root_fd, canonical, &path) {
                self.failed_read(&path, failure);
                continue;
            }
            sources.push(read.content.expect("only supported sources are batched"));
            held.push(read.file);
        }
        if sources.is_empty() {
            return Ok(true);
        }
        let changed = match engine.replace_prepared_sources(&sources, control) {
            Ok(changed) => changed,
            Err(FoundryError::Cancelled(_) | FoundryError::DeadlineExceeded(_)) => {
                self.check(control);
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        self.report.changed += changed as u64;
        self.report.unchanged += (sources.len() - changed) as u64;
        self.since_publish += sources.len();
        drop(held);
        Ok(true)
    }
}

fn record_parse_failures(engine: &mut Engine, report: &mut IndexReport) {
    let panics = engine.take_parse_failures();
    report.failures += panics.count;
    for sample in panics.samples {
        push_sample(&mut report.failure_samples, sample);
    }
}

fn read_file(root: &std::fs::File, path: &str, control: &Control) -> Result<ReadFile, OpenFailure> {
    control
        .check()
        .map_err(|e| OpenFailure::Io(e.to_string()))?;
    let mut file = held::open_relative(root, &path.split('/').collect::<Vec<_>>())?;
    let before = file
        .metadata()
        .map_err(|e| OpenFailure::Io(e.to_string()))?;
    // Exclusion is a decision about this exact file too. Hold and recheck it,
    // but never read the contents of a sensitive filename.
    if matches!(
        path.rsplit('/').next(),
        Some(".env" | "id_rsa" | "id_ed25519")
    ) {
        return Ok(ReadFile {
            file,
            version: before,
            content: Err("sensitive filename"),
        });
    }
    let capacity = before
        .len()
        .saturating_add(1)
        .min((MAX_SOURCE_BYTES + 1) as u64) as usize;
    let mut bytes = Vec::with_capacity(capacity);
    (&mut file)
        .take((MAX_SOURCE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| OpenFailure::Io(e.to_string()))?;
    let after = file
        .metadata()
        .map_err(|e| OpenFailure::Io(e.to_string()))?;
    if !same_file(&before, &after) {
        return Err(OpenFailure::SourceChanged(
            "file changed during read".into(),
        ));
    }
    control
        .check()
        .map_err(|e| OpenFailure::Io(e.to_string()))?;
    let content = match source_content(&bytes) {
        Ok(_) => Ok(crate::store::PreparedSource::new(
            path.to_owned(),
            String::from_utf8(bytes).expect("source content validated UTF-8"),
        )
        .map_err(|e| OpenFailure::Io(e.to_string()))?),
        Err(reason) => Err(reason),
    };
    Ok(ReadFile {
        file,
        version: after,
        content,
    })
}

fn verify_read(
    read: &ReadFile,
    root: &std::fs::File,
    canonical: &Path,
    path: &str,
) -> Result<(), OpenFailure> {
    held::root_still_bound(root, canonical)?;
    let held = read
        .file
        .metadata()
        .map_err(|e| OpenFailure::Io(e.to_string()))?;
    let named = held::open_relative(root, &path.split('/').collect::<Vec<_>>())?
        .metadata()
        .map_err(|e| OpenFailure::Io(e.to_string()))?;
    if !same_file(&read.version, &held) || !same_file(&held, &named) {
        return Err(OpenFailure::SourceChanged(
            "file changed before publication".into(),
        ));
    }
    Ok(())
}

#[derive(Default, Debug, Clone, Serialize)]
pub struct IndexReport {
    pub scan_id: u64,
    pub scan_complete: bool,
    pub changed: u64,
    pub unchanged: u64,
    pub deleted: u64,
    pub excluded: u64,
    pub failures: u64,
    pub failure_samples: Vec<String>,
    pub exclusion_samples: Vec<String>,
    pub deletions_deferred: bool,
    pub partial: bool,
    pub reason: Option<String>,
    /// Machine-readable partial cause: `cancelled`, `deadline_exceeded`,
    /// `repair_required` or `scan_failures`.
    pub reason_code: Option<&'static str>,
    pub pending_sources: u64,
    pub source_revision: u64,
}

impl IndexReport {
    /// Counts-only partial state for bounded errors; no samples.
    pub fn partial_counts(&self) -> PartialIndexCounts {
        PartialIndexCounts {
            changed: self.changed,
            unchanged: self.unchanged,
            deleted: self.deleted,
            excluded: self.excluded,
            failed: self.failures,
            pending_sources: self.pending_sources,
            scan_complete: self.scan_complete,
            deletions_deferred: self.deletions_deferred,
        }
    }

    /// The bounded MCP error for an incomplete index run, if any. Cancellation
    /// and deadline carry committed counts; other incompleteness is
    /// `index_incomplete` and is not automatically retryable.
    pub fn index_error(&self) -> Option<FoundryError> {
        if !self.partial {
            return None;
        }
        let counts = self.partial_counts();
        Some(match self.reason_code {
            Some("cancelled") => FoundryError::Cancelled(Some(counts)),
            Some("deadline_exceeded") => FoundryError::DeadlineExceeded(Some(counts)),
            _ => FoundryError::IndexIncomplete(counts),
        })
    }
}

fn push_sample(samples: &mut Vec<String>, text: String) {
    if samples.len() >= SAMPLE_LIMIT {
        return;
    }
    let mut bounded = text;
    if bounded.len() > SAMPLE_BYTES {
        let mut cut = SAMPLE_BYTES;
        while cut > 0 && !bounded.is_char_boundary(cut) {
            cut -= 1;
        }
        bounded.truncate(cut);
    }
    samples.push(bounded);
}

/// Named scan-time failures for one candidate file or the workspace root.
#[derive(Debug)]
enum OpenFailure {
    /// A component was replaced by a symlink or is not a regular file.
    UnsafeSourcePath(String),
    /// The enumerated file vanished before or during the read.
    SourceChanged(String),
    Io(String),
    /// Permission was denied on a component or the root.
    #[cfg(unix)]
    Permission(String),
    /// The platform lacks the no-follow descriptor primitives.
    #[cfg(not(unix))]
    Unsupported(String),
}

impl OpenFailure {
    fn code(&self) -> &'static str {
        match self {
            OpenFailure::UnsafeSourcePath(_) => "unsafe_source_path",
            OpenFailure::SourceChanged(_) => "source_changed",
            OpenFailure::Io(_) => "read_failed",
            #[cfg(unix)]
            OpenFailure::Permission(_) => "permission_denied",
            #[cfg(not(unix))]
            OpenFailure::Unsupported(_) => "platform_unsupported",
        }
    }

    fn detail(&self) -> &str {
        match self {
            OpenFailure::UnsafeSourcePath(d)
            | OpenFailure::SourceChanged(d)
            | OpenFailure::Io(d) => d,
            #[cfg(unix)]
            OpenFailure::Permission(d) => d,
            #[cfg(not(unix))]
            OpenFailure::Unsupported(d) => d,
        }
    }

    /// The error for failing to acquire the bound workspace root.
    fn into_root_error(self) -> FoundryError {
        match self {
            OpenFailure::UnsafeSourcePath(detail) | OpenFailure::SourceChanged(detail) => {
                FoundryError::UnsafeSourcePath(format!("workspace root: {detail}"))
            }
            OpenFailure::Io(detail) => {
                FoundryError::Internal(anyhow::anyhow!("cannot hold workspace root: {detail}"))
            }
            #[cfg(unix)]
            OpenFailure::Permission(detail) => {
                FoundryError::PermissionDenied(format!("workspace root: {detail}"))
            }
            #[cfg(not(unix))]
            OpenFailure::Unsupported(detail) => FoundryError::PlatformUnsupported(detail),
        }
    }
}

#[cfg(unix)]
mod held {
    use std::ffi::CString;
    use std::fs::File;
    use std::io;
    use std::os::unix::io::{AsRawFd, FromRawFd};
    use std::path::Path;

    use super::OpenFailure;

    /// Open directory components for traversal only: `O_SEARCH` on macOS,
    /// `O_PATH` on Linux. Neither requires read permission on the directory,
    /// which is exactly the guarantee holding an ancestor should need.
    #[cfg(target_os = "macos")]
    pub(crate) const DIR_OPEN: libc::c_int = libc::O_SEARCH;
    #[cfg(target_os = "linux")]
    pub(crate) const DIR_OPEN: libc::c_int = libc::O_PATH;
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    pub(crate) const DIR_OPEN: libc::c_int = libc::O_RDONLY;

    /// Acquire the BOUND canonical root as a held directory descriptor by
    /// component-wise traversal from `/`, opening every component with
    /// `O_NOFOLLOW | O_DIRECTORY`. A canonical path contains no symlinks by
    /// construction, so a symlink or non-directory component means the root
    /// (or an ancestor) was replaced after validation. The caller's pathname is
    /// never re-opened through a following `open`.
    pub fn open_root(canonical: &Path) -> Result<File, OpenFailure> {
        use std::path::Component;
        let slash = CString::new("/").expect("static path");
        // SAFETY: plain open(2) on a NUL-terminated path.
        let fd = unsafe {
            libc::open(
                slash.as_ptr(),
                DIR_OPEN | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(OpenFailure::Io(io::Error::last_os_error().to_string()));
        }
        // SAFETY: `fd` is a fresh descriptor owned by nothing else.
        let mut current = unsafe { File::from_raw_fd(fd) };
        for component in canonical.components() {
            let name = match component {
                Component::RootDir => continue,
                Component::Normal(name) => name,
                _ => {
                    return Err(OpenFailure::UnsafeSourcePath(
                        "bound root is not a canonical absolute path".into(),
                    ));
                }
            };
            let Ok(c) = CString::new(name.as_encoded_bytes()) else {
                return Err(OpenFailure::UnsafeSourcePath(
                    "root component contains NUL".into(),
                ));
            };
            let flags = DIR_OPEN | libc::O_CLOEXEC | libc::O_NOFOLLOW;
            // SAFETY: openat(2) relative to a live directory descriptor.
            let next = unsafe { libc::openat(current.as_raw_fd(), c.as_ptr(), flags) };
            if next < 0 {
                return Err(map(io::Error::last_os_error(), &name.to_string_lossy()));
            }
            // SAFETY: `next` is a fresh descriptor; the parent closes on drop.
            current = unsafe { File::from_raw_fd(next) };
        }
        Ok(current)
    }

    /// The held descriptor must still be the directory the bound pathname
    /// names: same device/inode and not a symlink. Checked after acquisition
    /// and before any scan mutation, so replacing the pathname after the
    /// descriptor was opened cannot redirect enumeration.
    pub fn root_still_bound(held: &File, canonical: &Path) -> Result<(), OpenFailure> {
        use std::os::unix::fs::MetadataExt;
        let held_meta = held
            .metadata()
            .map_err(|e| OpenFailure::Io(e.to_string()))?;
        let named = std::fs::symlink_metadata(canonical)
            .map_err(|e| OpenFailure::SourceChanged(e.to_string()))?;
        if !named.file_type().is_dir()
            || named.dev() != held_meta.dev()
            || named.ino() != held_meta.ino()
        {
            return Err(OpenFailure::UnsafeSourcePath(
                "the root pathname no longer names the held directory".into(),
            ));
        }
        Ok(())
    }

    /// Open `components` relative to the held root, refusing every symlink
    /// component (including replaced ancestors) and non-regular final files.
    pub fn open_relative(root: &File, components: &[&str]) -> Result<File, OpenFailure> {
        if components.is_empty() {
            return Err(OpenFailure::UnsafeSourcePath("empty relative path".into()));
        }
        let mut owned: Option<File> = None;
        let last = components.len() - 1;
        for (i, comp) in components.iter().enumerate() {
            let Ok(c) = CString::new(*comp) else {
                return Err(OpenFailure::UnsafeSourcePath(format!(
                    "component contains NUL: {comp:?}"
                )));
            };
            let directory = i < last;
            // O_NONBLOCK on the final component: a regular file replaced by a
            // FIFO must not block in open(2) before the fstat regular-file
            // check below can refuse it. Directories open traversal-only.
            let flags = if directory {
                DIR_OPEN | libc::O_CLOEXEC | libc::O_NOFOLLOW
            } else {
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK
            };
            let parent = owned
                .as_ref()
                .map(|f| f.as_raw_fd())
                .unwrap_or_else(|| root.as_raw_fd());
            // SAFETY: openat(2) relative to a live directory descriptor.
            let fd = unsafe { libc::openat(parent, c.as_ptr(), flags) };
            if fd < 0 {
                let err = io::Error::last_os_error();
                return Err(map(err, comp));
            }
            drop(owned.take());
            owned = Some(unsafe { File::from_raw_fd(fd) });
        }
        let file = owned.expect("at least one component");
        // Reject non-regular files at open time.
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: fstat(2) on a live descriptor.
        if unsafe { libc::fstat(file.as_raw_fd(), &mut stat) } != 0 {
            return Err(OpenFailure::Io(io::Error::last_os_error().to_string()));
        }
        // SAFETY: plain mode-bit arithmetic over the fstat result.
        if (stat.st_mode & libc::S_IFMT) != libc::S_IFREG {
            return Err(OpenFailure::UnsafeSourcePath("not a regular file".into()));
        }
        Ok(file)
    }

    fn map(err: io::Error, component: &str) -> OpenFailure {
        match err.raw_os_error() {
            Some(libc::EACCES) | Some(libc::EPERM) => {
                OpenFailure::Permission(format!("{component:?}: {err}"))
            }
            Some(libc::ELOOP) => {
                OpenFailure::UnsafeSourcePath(format!("symlink component {component:?}: {err}"))
            }
            Some(libc::ENOTDIR) => OpenFailure::UnsafeSourcePath(format!(
                "non-directory component {component:?}: {err}"
            )),
            Some(libc::ENOENT) => OpenFailure::SourceChanged(err.to_string()),
            _ => OpenFailure::Io(err.to_string()),
        }
    }
}

#[cfg(not(unix))]
mod held {
    use std::fs::File;
    use std::path::Path;

    use super::OpenFailure;

    const UNSUPPORTED: &str = "held-root scanning requires unix directory-descriptor primitives";

    pub fn open_root(_canonical: &Path) -> Result<File, OpenFailure> {
        Err(OpenFailure::Unsupported(UNSUPPORTED.into()))
    }

    pub fn root_still_bound(_held: &File, _canonical: &Path) -> Result<(), OpenFailure> {
        Err(OpenFailure::Unsupported(UNSUPPORTED.into()))
    }

    pub fn open_relative(_root: &File, _components: &[&str]) -> Result<File, OpenFailure> {
        Err(OpenFailure::Unsupported(UNSUPPORTED.into()))
    }
}

/// Confirm the root can actually be held as a descriptor, for callers that
/// must not create any state on refusal (store initialization).
pub(crate) fn ensure_holdable_root(canonical: &Path) -> FResult<()> {
    let held = held::open_root(canonical).map_err(OpenFailure::into_root_error)?;
    held::root_still_bound(&held, canonical).map_err(OpenFailure::into_root_error)
}

/// Explicit indexing over the bound root. Committed work is never rolled
/// back; interruption returns a partial report with committed counts.
pub fn index(engine: &mut Engine, root: &Path, control: &Control) -> FResult<IndexReport> {
    // Everything that can refuse runs before the first scan mutation: the
    // binding check, no-follow acquisition of the bound root as a held
    // descriptor, and confirmation that the pathname still names it.
    let canonical = engine.verify_binding(root)?;
    // Required counters are validated before any scan mutation.
    engine.source_revision()?;
    fault!(
        SCAN_BEFORE_ROOT_OPEN,
        Some(&*engine),
        Some(control),
        &canonical.to_string_lossy()
    )?;
    let root_fd = held::open_root(&canonical).map_err(OpenFailure::into_root_error)?;
    fault!(
        SCAN_AFTER_ROOT_OPEN,
        Some(&*engine),
        Some(control),
        &canonical.to_string_lossy()
    )?;
    held::root_still_bound(&root_fd, &canonical).map_err(OpenFailure::into_root_error)?;
    // Boundary after the pre-walk identity check, before the walker reads.
    fault!(SCAN_BEFORE_WALK, Some(&*engine), Some(control), "")?;
    engine.bind_workspace(&canonical)?;
    let scan_id = engine.begin_scan()?;
    // Periodic page drains contribute only failures from this scan, just as
    // the final full drain did before ingestion gained read-ahead.
    let _ = engine.take_parse_failures();
    let mut progress = ScanProgress {
        report: IndexReport {
            scan_id,
            ..IndexReport::default()
        },
        ..ScanProgress::default()
    };
    let mut seen_page: Vec<String> = Vec::with_capacity(SEEN_PAGE);
    let data_directory = engine.directory().to_path_buf();
    let walker = ignore::WalkBuilder::new(&canonical)
        .hidden(true)
        .follow_links(false)
        .filter_entry(move |entry| {
            entry.path() != data_directory
                && !matches!(
                    entry.file_name().to_str(),
                    Some("target" | "node_modules" | ".context-foundry")
                )
        })
        .build();
    let pool = engine.indexing_pool()?;
    #[cfg(feature = "test-faults")]
    let observer = pipeline_hooks::current();
    pool.in_place_scope(|scope| -> FResult<()> {
        for entry in walker {
            if !progress.check(control) {
                break;
            }
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    progress.enumeration_failed = true;
                    progress.report.failures += 1;
                    push_sample(
                        &mut progress.report.failure_samples,
                        format!("(walker): {e}"),
                    );
                    continue;
                }
            };
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let rel = match entry
                .path()
                .strip_prefix(&canonical)
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_owned()))
            {
                Some(rel) => rel,
                None => {
                    // Path-encoding errors prevent the unseen sweep globally.
                    progress.enumeration_failed = true;
                    progress.report.failures += 1;
                    push_sample(
                        &mut progress.report.failure_samples,
                        format!("(path): not UTF-8 relative path: {:?}", entry.path()),
                    );
                    continue;
                }
            };
            // Validate admitted paths against the shared handle rules before any
            // source commit; unsupported syntax is a named scan failure.
            if let Err(e) = crate::store::validate_path(&rel) {
                progress.enumeration_failed = true;
                progress.report.failures += 1;
                push_sample(
                    &mut progress.report.failure_samples,
                    format!("{rel}: invalid_source_path: {e}"),
                );
                continue;
            }
            // Seen when encountered, even if unreadable. Marks are committed in
            // bounded pages and always flushed before the sweep.
            seen_page.push(rel.clone());
            if seen_page.len() >= SEEN_PAGE {
                engine.mark_seen_batch(&seen_page, scan_id)?;
                seen_page.clear();
            }
            let (send, result) = std::sync::mpsc::sync_channel(1);
            // Keep the existing deterministic enumeration/open boundary on
            // the owner thread. Workers receive no Engine or transaction.
            let opened = fault!(SCAN_BEFORE_OPEN, Some(&*engine), Some(control), &rel)
                .map_err(|e| OpenFailure::Io(e.to_string()));
            match opened {
                Err(error) => {
                    let _ = send.send(Err(error));
                }
                Ok(()) => {
                    let path = rel.clone();
                    let root_fd = &root_fd;
                    #[cfg(feature = "test-faults")]
                    let observer = observer.clone();
                    scope.spawn(move |_| {
                        let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            #[cfg(feature = "test-faults")]
                            if let Some(observer) = &observer {
                                observer(&pipeline_hooks::Event::Reading(&path));
                            }
                            let read = read_file(root_fd, &path, control);
                            #[cfg(feature = "test-faults")]
                            if let Some(observer) = &observer {
                                observer(&pipeline_hooks::Event::Ready(&path));
                            }
                            read
                        }))
                        .unwrap_or_else(|_| {
                            Err(OpenFailure::Io("an ingestion worker panicked".into()))
                        });
                        let _ = send.send(read);
                    });
                }
            }
            progress
                .pending
                .push_back(PendingRead { path: rel, result });
            #[cfg(feature = "test-faults")]
            if let Some(observer) = &observer {
                observer(&pipeline_hooks::Event::Queued(progress.pending.len()));
            }
            if progress.pending.len() == READ_AHEAD
                && !progress.next(engine, &root_fd, &canonical, control)?
            {
                break;
            }
        }
        while !progress.pending.is_empty() && progress.cancel_code.is_none() {
            if !progress.next(engine, &root_fd, &canonical, control)? {
                break;
            }
        }
        Ok(())
    })?;
    let ScanProgress {
        mut report,
        enumeration_failed,
        mut cancel_code,
        mut drain_reason,
        ..
    } = progress;
    let mut enumeration_ok = !enumeration_failed;
    // The root was this directory when the walk started. Re-check before any
    // retirement decision: a root swapped during enumeration must not let the
    // sweep retire rows by names it never enumerated. (Residual: a
    // same-identity replace-enumerate-restore swap remains undetectable.)
    if let Err(failure) = held::root_still_bound(&root_fd, &canonical) {
        enumeration_ok = false;
        report.failures += 1;
        push_sample(
            &mut report.failure_samples,
            format!("(root): {}: {}", failure.code(), failure.detail()),
        );
    }
    engine.mark_seen_batch(&seen_page, scan_id)?;
    drop(seen_page);
    // Successful enumeration permits the paged sweep of unseen source rows;
    // otherwise absence deletions stay deferred and prior state is preserved.
    let mut sweep_interrupted = false;
    if enumeration_ok && cancel_code.is_none() {
        match engine.sweep_unseen(scan_id, control) {
            Ok((deleted, interrupted)) => {
                report.deleted += deleted;
                sweep_interrupted = interrupted;
                if interrupted {
                    cancel_code = Some(control.last_check_error().unwrap_or("cancelled"));
                }
            }
            Err(FoundryError::Cancelled(_)) | Err(FoundryError::DeadlineExceeded(_)) => {
                sweep_interrupted = true;
                cancel_code = Some(control.last_check_error().unwrap_or("cancelled"));
            }
            Err(e) => return Err(e),
        }
    }
    report.deletions_deferred = !enumeration_ok || sweep_interrupted || cancel_code.is_some();
    // Drain pending index work in bounded batches; cancellation and deadline
    // leave committed work durable and the report partial.
    match engine.refresh(control) {
        Ok(_) => {}
        Err(FoundryError::Cancelled(_)) | Err(FoundryError::DeadlineExceeded(_)) => {
            drain_reason = Some(control.last_check_error().unwrap_or("cancelled"));
        }
        Err(FoundryError::RepairRequired(_)) => drain_reason = Some("repair_required"),
        Err(e) => return Err(e),
    }
    // A parse panic is a named scan failure (context-v2 § Parallel indexing):
    // the drained source was indexed as the plain blocks of an unmapped
    // source and `status` names it until it is parsed again.
    record_parse_failures(engine, &mut report);
    // Source-scan completion is independent of the derived-index drain: it
    // means complete enumeration AND a finished sweep. A cancelled or failed
    // drain keeps the operation partial and nonzero but does not make a
    // finished source scan look incomplete.
    let scan_complete = enumeration_ok && !sweep_interrupted;
    engine.finish_scan(scan_complete)?;
    report.scan_complete = scan_complete;
    // 008: the scan report counts source work only; readiness uses totals.
    report.pending_sources = engine.pending_source_work()?;
    report.source_revision = engine.source_revision()?;
    report.partial = !scan_complete
        || report.failures > 0
        || report.pending_sources > 0
        || drain_reason.is_some();
    let reason_code = cancel_code.or(drain_reason).or(if report.failures > 0 {
        Some("scan_failures")
    } else {
        None
    });
    report.reason_code = if report.partial { reason_code } else { None };
    report.reason = if report.partial {
        Some(match reason_code {
            Some("cancelled") => {
                "indexing cancelled; committed counts and pending work reported".into()
            }
            Some("deadline_exceeded") => {
                "indexing deadline exceeded; committed counts and pending work reported".into()
            }
            Some("repair_required") => {
                "derived index needs explicit repair; committed counts and pending work reported"
                    .into()
            }
            _ => "indexing incomplete; inspect failure samples".into(),
        })
    } else {
        None
    };
    Ok(report)
}

/// One held root reused by the watcher. Paths are hints, never trusted bytes.
pub(crate) struct ChangeReader {
    pub(crate) root: std::path::PathBuf,
    root_fd: std::fs::File,
    store: std::path::PathBuf,
}

impl ChangeReader {
    pub(crate) fn new(engine: &Engine, root: &Path) -> FResult<Self> {
        let root = engine.verify_binding(root)?;
        let root_fd = held::open_root(&root).map_err(OpenFailure::into_root_error)?;
        held::root_still_bound(&root_fd, &root).map_err(OpenFailure::into_root_error)?;
        Ok(Self {
            root,
            root_fd,
            store: engine.directory().to_path_buf(),
        })
    }
    pub(crate) fn check_root(&self) -> FResult<()> {
        held::root_still_bound(&self.root_fd, &self.root).map_err(OpenFailure::into_root_error)
    }
    /// Known file edits avoid walking siblings. Unknown paths use the same ignore
    /// walker as explicit indexing; a symlink/directory replacement is a scoped walk.
    pub(crate) fn direct(&self, engine: &Engine, path: &str) -> FResult<bool> {
        if engine.source(path)?.is_none() {
            return Ok(false);
        }
        self.check_root()?;
        match std::fs::symlink_metadata(self.root.join(path)) {
            Ok(meta) => Ok(meta.is_file()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
            Err(_) => Ok(true), // the descriptor read below reports the named failure
        }
    }
    pub(crate) fn update(
        &self,
        engine: &Engine,
        path: &str,
        control: &Control,
    ) -> FResult<ChangeCounts> {
        crate::store::validate_path(path)?;
        control.check()?;
        self.check_root()?;
        let parts: Vec<&str> = path.split('/').collect();
        fault!(WATCH_BEFORE_OPEN, Some(engine), Some(control), path)?;
        let mut file = match held::open_relative(&self.root_fd, &parts) {
            Ok(file) => file,
            Err(OpenFailure::SourceChanged(_)) => {
                // Absence is verified through no-follow ancestors, not inferred
                // from a remove hint. A permission/symlink error cannot delete.
                self.check_root()?;
                control.check()?;
                return Ok(ChangeCounts {
                    deleted: u64::from(engine.delete_source(path)?),
                    ..Default::default()
                });
            }
            Err(failure) => return Err(change_error(path, failure)),
        };
        let before = file.metadata()?;
        let mut bytes = Vec::new();
        (&mut file)
            .take((MAX_SOURCE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        fault!(WATCH_AFTER_READ, Some(engine), Some(control), path)?;
        let after = file.metadata()?;
        let named = held::open_relative(&self.root_fd, &parts)
            .map_err(|f| change_error(path, f))?
            .metadata()?;
        if !same_file(&before, &after) || !same_file(&after, &named) {
            return Err(change_error(
                path,
                OpenFailure::SourceChanged("file changed during refresh".into()),
            ));
        }
        self.check_root()?;
        control.check()?;
        let content = if matches!(
            path.rsplit('/').next(),
            Some(".env" | "id_rsa" | "id_ed25519")
        ) {
            Err("sensitive filename")
        } else {
            source_content(&bytes)
        };
        match content {
            Ok(content) => {
                let changed = engine.replace_source(path, content)?;
                Ok(ChangeCounts {
                    read: 1,
                    changed: u64::from(changed),
                    unchanged: u64::from(!changed),
                    ..Default::default()
                })
            }
            Err(_) => Ok(ChangeCounts {
                read: 1,
                excluded: 1,
                deleted: u64::from(engine.delete_source(path)?),
                ..Default::default()
            }),
        }
    }
}

fn change_error(path: &str, failure: OpenFailure) -> FoundryError {
    FoundryError::InvalidArgument(format!("{path}: {}: {}", failure.code(), failure.detail()))
}
fn same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        a.dev() == b.dev()
            && a.ino() == b.ino()
            && a.size() == b.size()
            && a.mtime() == b.mtime()
            && a.mtime_nsec() == b.mtime_nsec()
            && a.ctime() == b.ctime()
            && a.ctime_nsec() == b.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        a.len() == b.len() && a.modified().ok() == b.modified().ok()
    }
}
fn source_content(bytes: &[u8]) -> Result<&str, &'static str> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err("over 2 MiB");
    }
    if bytes.contains(&0) {
        return Err("binary NUL");
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "not UTF-8")?;
    // Keep the exact runtime markers, without causing this detector's own
    // source to be excluded when a user indexes Foundry itself.
    if text.contains(concat!("-----BEGIN ", "PRIVATE KEY-----"))
        || text.contains(concat!("-----BEGIN ", "RSA PRIVATE KEY-----"))
    {
        return Err("private key marker");
    }
    Ok(text)
}

#[derive(Default, Clone, Debug, Serialize)]
pub(crate) struct ChangeCounts {
    pub read: u64,
    pub changed: u64,
    pub unchanged: u64,
    pub deleted: u64,
    pub excluded: u64,
}

/// A streaming root/scoped walk. One step visits at most one yielded entry or
/// one 128-row deletion page, with no root-sized in-memory seen set. The existing
/// redb seen table supplies the deletion proof. Failed walks never enter sweep.
pub(crate) struct ChangeScan {
    reader: std::sync::Arc<ChangeReader>,
    scope: Option<String>,
    scan_id: u64,
    walker: ignore::Walk,
    sweeping: bool,
    seen: Vec<String>,
    after: Option<String>,
    failed: Option<String>,
}
impl ChangeScan {
    pub(crate) fn start(
        engine: &Engine,
        reader: std::sync::Arc<ChangeReader>,
        scope: Option<String>,
    ) -> FResult<Self> {
        reader.check_root()?;
        if let Some(path) = &scope {
            crate::store::validate_path(path)?;
        }
        let scan_id = engine.begin_scan_kind(scope.is_none())?;
        let wanted = scope.as_ref().map(|s| reader.root.join(s));
        let store = reader.store.clone();
        let walker = ignore::WalkBuilder::new(&reader.root)
            .hidden(true)
            .follow_links(false)
            .filter_entry(move |entry| {
                entry.path() != store
                    && !matches!(
                        entry.file_name().to_str(),
                        Some("target" | "node_modules" | ".context-foundry")
                    )
                    && wanted.as_ref().is_none_or(|path| {
                        path.starts_with(entry.path()) || entry.path().starts_with(path)
                    })
            })
            .build();
        Ok(Self {
            reader,
            scope,
            scan_id,
            walker,
            sweeping: false,
            seen: Vec::with_capacity(SEEN_PAGE),
            after: None,
            failed: None,
        })
    }
    pub(crate) fn abort(&self, engine: &Engine) -> FResult<()> {
        if self.scope.is_none() && engine.current_scan_id()? == self.scan_id {
            engine.finish_scan(false)?;
        }
        Ok(())
    }
    pub(crate) fn step(
        &mut self,
        engine: &Engine,
        control: &Control,
    ) -> FResult<(ChangeCounts, bool)> {
        control.check()?;
        self.reader.check_root()?;
        if engine.current_scan_id()? != self.scan_id {
            return Err(FoundryError::Conflict(
                "watch reconciliation superseded by an explicit scan".into(),
            ));
        }
        if !self.sweeping {
            match self.walker.next() {
                Some(Err(error)) => {
                    self.failed = Some(format!("watch walk: {error}"));
                }
                Some(Ok(entry)) => {
                    if let Some(error) = entry.error() {
                        self.failed = Some(format!("watch ignore rules: {error}"));
                    }
                    if entry.file_type().is_some_and(|ty| ty.is_file()) {
                        let Some(path) = entry
                            .path()
                            .strip_prefix(&self.reader.root)
                            .ok()
                            .and_then(Path::to_str)
                        else {
                            self.failed = Some("watch path is not relative UTF-8".into());
                            return Ok((ChangeCounts::default(), false));
                        };
                        if let Err(error) = crate::store::validate_path(path) {
                            self.failed = Some(error.to_string());
                            return Ok((ChangeCounts::default(), false));
                        }
                        // Persist seen before reading: a failure cannot prove absence.
                        self.seen.push(path.to_owned());
                        if self.seen.len() >= SEEN_PAGE {
                            engine.mark_seen_batch(&self.seen, self.scan_id)?;
                            self.seen.clear();
                        }
                        match self.reader.update(engine, path, control) {
                            Ok(counts) => return Ok((counts, false)),
                            Err(error) => {
                                self.failed = Some(error.to_string());
                            }
                        }
                    }
                }
                None => {
                    engine.mark_seen_batch(&self.seen, self.scan_id)?;
                    self.seen.clear();
                    if let Some(reason) = &self.failed {
                        self.abort(engine)?;
                        return Err(FoundryError::InvalidArgument(reason.clone()));
                    }
                    self.sweeping = true;
                }
            }
            return Ok((ChangeCounts::default(), false));
        }
        // Recheck the held root before EVERY deletion page. The caller also
        // cancels this scan if ignore/overflow hints changed its admission epoch.
        let (deleted, after) = engine.sweep_unseen_page(
            self.scan_id,
            self.scope.as_deref(),
            self.after.as_deref(),
            control,
        )?;
        let done = after.is_none();
        self.after = after;
        if done && self.scope.is_none() {
            engine.finish_scan(true)?;
        }
        Ok((
            ChangeCounts {
                deleted,
                ..Default::default()
            },
            done,
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn private_key_markers_still_exclude_inputs_without_excluding_the_detector() {
        assert!(super::source_content(include_bytes!("ingest.rs")).is_ok());
        for kind in ["PRIVATE KEY", "RSA PRIVATE KEY"] {
            let text = format!("before\n-----BEGIN {kind}-----\nafter\n");
            assert_eq!(
                super::source_content(text.as_bytes()),
                Err("private key marker")
            );
        }
    }
    use super::OpenFailure;
    use super::held::{open_relative, open_root};
    use std::fs;

    fn temp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn symlinked_file_component_is_refused_not_followed() {
        let work = temp();
        let outside = work.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("secret.txt"), "outside bytes\n").unwrap();
        let root = work.path().join("root");
        fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("file.txt")).unwrap();
        let held = open_root(&root.canonicalize().unwrap()).unwrap();
        let err = open_relative(&held, &["file.txt"]).unwrap_err();
        assert!(matches!(err, OpenFailure::UnsafeSourcePath(_)), "{err:?}");
    }

    #[test]
    fn replaced_ancestor_symlink_is_refused_and_outside_bytes_stay_unread() {
        let work = temp();
        let outside = work.path().join("outside");
        fs::create_dir_all(outside.join("pkg")).unwrap();
        fs::write(
            outside.join("pkg").join("file.rs"),
            "OUTSIDE MUST NOT BE READ\n",
        )
        .unwrap();
        let root = work.path().join("root");
        fs::create_dir_all(root.join("pkg")).unwrap();
        fs::write(root.join("pkg").join("file.rs"), "inside bytes\n").unwrap();
        // Hold the root before the swap, exactly like a scan does.
        let held = open_root(&root.canonicalize().unwrap()).unwrap();
        // Replace the enumerated ancestor directory with an outside-pointing
        // symlink between enumeration and open.
        fs::rename(root.join("pkg"), work.path().join("pkg.orig")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("pkg")).unwrap();
        let err = open_relative(&held, &["pkg", "file.rs"]).unwrap_err();
        assert!(
            matches!(err, OpenFailure::UnsafeSourcePath(_)),
            "expected unsafe_source_path, got {err:?}"
        );
    }

    #[test]
    fn replaced_file_becomes_symlink_and_is_named_source_change_or_unsafe() {
        let work = temp();
        let root = work.path().join("root");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("gone.txt"), "original\n").unwrap();
        let held = open_root(&root.canonicalize().unwrap()).unwrap();
        fs::remove_file(root.join("gone.txt")).unwrap();
        let err = open_relative(&held, &["gone.txt"]).unwrap_err();
        assert!(matches!(err, OpenFailure::SourceChanged(_)), "{err:?}");
    }

    #[test]
    fn held_root_descriptor_survives_root_path_replacement() {
        let work = temp();
        let root = work.path().join("root");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("file.txt"), "pinned by descriptor\n").unwrap();
        let held = open_root(&root.canonicalize().unwrap()).unwrap();
        // The original pathname is replaced; the held descriptor still reads
        // the original tree, never the replacement.
        fs::rename(&root, work.path().join("root.moved")).unwrap();
        fs::create_dir(&root).unwrap();
        fs::write(root.join("file.txt"), "replacement bytes\n").unwrap();
        use std::io::Read;
        let mut file = open_relative(&held, &["file.txt"]).unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"pinned by descriptor\n");
    }

    #[test]
    fn non_regular_final_component_is_refused() {
        let work = temp();
        let root = work.path().join("root");
        fs::create_dir_all(root.join("sub")).unwrap();
        let held = open_root(&root.canonicalize().unwrap()).unwrap();
        let err = open_relative(&held, &["sub"]).unwrap_err();
        assert!(matches!(err, OpenFailure::UnsafeSourcePath(_)), "{err:?}");
    }
}

#[cfg(test)]
mod open_flags_tests {
    use super::held::{open_root, root_still_bound};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn temp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn a_root_under_an_unreadable_ancestor_can_still_be_held() {
        if unsafe { libc::geteuid() } == 0 {
            return; // root bypasses permission bits.
        }
        let fixture = temp();
        let root = fixture.path().join("outer").join("inner").join("ws");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("probe.rs"), "fn probe() {}\n").unwrap();
        let mut perms = fs::metadata(fixture.path().join("outer"))
            .unwrap()
            .permissions();
        // Traversal without read: exactly what holding an ancestor needs.
        perms.set_mode(0o311);
        fs::set_permissions(fixture.path().join("outer"), perms).unwrap();
        let canonical = root.canonicalize().unwrap();
        let held = open_root(&canonical).expect("search-only opens need execute, not read");
        root_still_bound(&held, &canonical).unwrap();
        // A read-only open would fail here; restoring the mode keeps the
        // fixture removable.
        let mut perms = fs::metadata(fixture.path().join("outer"))
            .unwrap()
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(fixture.path().join("outer"), perms).unwrap();
    }
}
