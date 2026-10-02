//! Serial workspace reconciliation with bounded buffers. Never collects all
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
    let mut report = IndexReport {
        scan_id,
        ..IndexReport::default()
    };
    let mut enumeration_ok = true;
    let mut cancel_code: Option<&'static str> = None;
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
    for entry in walker {
        if control.check().is_err() {
            cancel_code = Some(control.last_check_error().unwrap_or("cancelled"));
            enumeration_ok = false;
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                enumeration_ok = false;
                report.failures += 1;
                push_sample(&mut report.failure_samples, format!("(walker): {e}"));
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
                enumeration_ok = false;
                report.failures += 1;
                push_sample(
                    &mut report.failure_samples,
                    format!("(path): not UTF-8 relative path: {:?}", entry.path()),
                );
                continue;
            }
        };
        // Validate admitted paths against the shared handle rules before any
        // source commit; unsupported syntax is a named scan failure.
        if let Err(e) = crate::store::validate_path(&rel) {
            enumeration_ok = false;
            report.failures += 1;
            push_sample(
                &mut report.failure_samples,
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
        if matches!(
            entry.file_name().to_str(),
            Some(".env" | "id_rsa" | "id_ed25519")
        ) {
            report.deleted += u64::from(engine.delete_source(&rel)?);
            report.excluded += 1;
            push_sample(
                &mut report.exclusion_samples,
                format!("{rel}: sensitive filename"),
            );
            continue;
        }
        let components: Vec<&str> = rel.split('/').collect();
        let read = (|| -> Result<Vec<u8>, OpenFailure> {
            // Boundary between enumeration and open. An injected error here is
            // a deterministic unreadable-file failure for this path.
            fault!(SCAN_BEFORE_OPEN, Some(&*engine), Some(control), &rel)
                .map_err(|e| OpenFailure::Io(e.to_string()))?;
            let file = held::open_relative(&root_fd, &components)?;
            let mut bytes = Vec::new();
            file.take((MAX_SOURCE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| OpenFailure::Io(e.to_string()))?;
            Ok(bytes)
        })();
        let bytes = match read {
            Ok(bytes) => bytes,
            Err(failure) => {
                // Named failure: preserve the previous accepted record and
                // defer the unseen sweep globally.
                enumeration_ok = false;
                report.failures += 1;
                push_sample(
                    &mut report.failure_samples,
                    format!("{rel}: {}: {}", failure.code(), failure.detail()),
                );
                continue;
            }
        };
        let excluded = if bytes.len() > MAX_SOURCE_BYTES {
            Some("over 2 MiB")
        } else if bytes.contains(&0) {
            Some("binary NUL")
        } else if std::str::from_utf8(&bytes).is_err() {
            Some("not UTF-8")
        } else {
            None
        };
        if let Some(reason) = excluded {
            report.deleted += u64::from(engine.delete_source(&rel)?);
            report.excluded += 1;
            push_sample(&mut report.exclusion_samples, format!("{rel}: {reason}"));
            continue;
        }
        let Ok(content) = String::from_utf8(bytes) else {
            continue;
        };
        // A narrow deny rule, not a claim that arbitrary prose secrets can be detected.
        if content.contains("-----BEGIN PRIVATE KEY-----")
            || content.contains("-----BEGIN RSA PRIVATE KEY-----")
        {
            report.deleted += u64::from(engine.delete_source(&rel)?);
            report.excluded += 1;
            push_sample(
                &mut report.exclusion_samples,
                format!("{rel}: private key marker"),
            );
            continue;
        }
        if engine.replace_source(&rel, &content)? {
            report.changed += 1;
        } else {
            report.unchanged += 1;
        }
    }
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
    let mut drain_reason: Option<&'static str> = None;
    match engine.refresh(control) {
        Ok(_) => {}
        Err(FoundryError::Cancelled(_)) | Err(FoundryError::DeadlineExceeded(_)) => {
            drain_reason = Some(control.last_check_error().unwrap_or("cancelled"));
        }
        Err(FoundryError::RepairRequired(_)) => drain_reason = Some("repair_required"),
        Err(e) => return Err(e),
    }
    // Source-scan completion is independent of the derived-index drain: it
    // means complete enumeration AND a finished sweep. A cancelled or failed
    // drain keeps the operation partial and nonzero but does not make a
    // finished source scan look incomplete.
    let scan_complete = enumeration_ok && !sweep_interrupted;
    engine.finish_scan(scan_complete)?;
    report.scan_complete = scan_complete;
    report.pending_sources = engine.pending()?;
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

#[cfg(test)]
mod tests {
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
