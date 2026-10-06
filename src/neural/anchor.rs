//! Descriptor-anchored filesystem work for derived semantic state.
//!
//! Purge, generation publication, removal and generation reads work ONLY
//! through descriptor-relative calls (`openat`, `mkdirat`, `unlinkat`,
//! `renameat`, `fstatat` with `AT_SYMLINK_NOFOLLOW`). The store directory
//! descriptor is opened (`O_DIRECTORY | O_NOFOLLOW`) exactly ONCE, by the
//! Engine, when it binds and locks the store; every semantic operation
//! receives a duplicate of that descriptor and never resolves the store
//! pathname again. Renaming or substituting an ancestor after the bind —
//! including a symlink planted on the store's own path — therefore cannot
//! redirect any later operation: the descriptor keeps naming the directory
//! that was verified. Nothing here follows a link, and a symlink or
//! non-directory where a directory is required is a named path conflict.
use crate::error::{FResult, FoundryError};
use std::ffi::{CStr, CString, OsStr, OsString};
use std::fs::File;
use std::io::{self, Write as _};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;

const DIR_FLAGS: libc::c_int =
    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
/// Generation trees are at most a few levels deep; anything deeper is not
/// ours and is refused rather than walked.
const MAX_DEPTH: usize = 8;

/// What a directory entry is, from `fstatat(AT_SYMLINK_NOFOLLOW)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Dir,
    Symlink,
    File,
    Other,
}

/// An open directory descriptor. Every method is descriptor-relative.
pub struct Dir {
    fd: OwnedFd,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// One path component: no empty name, `.`, `..`, separator or NUL.
fn component(name: &OsStr) -> io::Result<CString> {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes == b"." || bytes == b".." || bytes.contains(&b'/') {
        return Err(invalid("not a single path component"));
    }
    CString::new(bytes).map_err(|_| invalid("name contains NUL"))
}

fn cvt(rc: libc::c_int) -> io::Result<libc::c_int> {
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(rc)
    }
}

fn own(fd: libc::c_int) -> OwnedFd {
    // SAFETY: `fd` is a fresh descriptor returned by the kernel and owned by
    // no one else.
    unsafe { OwnedFd::from_raw_fd(fd) }
}

impl Dir {
    /// Open `path` as a directory without following a symlink at its final
    /// component. The path should be canonical (the store directory is).
    pub fn open_path(path: &Path) -> io::Result<Dir> {
        let c = CString::new(path.as_os_str().as_bytes()).map_err(|_| invalid("path has NUL"))?;
        // SAFETY: open(2) with a NUL-terminated path.
        let fd = cvt(unsafe { libc::open(c.as_ptr(), DIR_FLAGS) })?;
        Ok(Dir { fd: own(fd) })
    }

    fn raw(&self) -> RawFd {
        self.fd.as_raw_fd()
    }

    /// Duplicate the descriptor (`F_DUPFD_CLOEXEC`, so the clone keeps the
    /// close-on-exec invariant): an independent handle on the SAME directory.
    /// The Engine hands the store directory it opened and verified at bind
    /// time to one semantic operation this way, without sharing ownership.
    pub fn try_clone(&self) -> io::Result<Dir> {
        // SAFETY: fcntl(2) on a live descriptor owned by `self`; the third
        // argument is the lowest acceptable descriptor number.
        let fd = cvt(unsafe { libc::fcntl(self.raw(), libc::F_DUPFD_CLOEXEC, 0) })?;
        Ok(Dir { fd: own(fd) })
    }

    /// Open a child directory. `None` when absent; a symlink or a
    /// non-directory is an error (`ELOOP` / `ENOTDIR`), never followed.
    pub fn open_dir(&self, name: impl AsRef<OsStr>) -> io::Result<Option<Dir>> {
        let c = component(name.as_ref())?;
        // SAFETY: openat(2) relative to a live directory descriptor.
        let fd = unsafe { libc::openat(self.raw(), c.as_ptr(), DIR_FLAGS) };
        if fd < 0 {
            let error = io::Error::last_os_error();
            return if error.kind() == io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(error)
            };
        }
        Ok(Some(Dir { fd: own(fd) }))
    }

    /// `mkdirat(0700)`; an existing name is an `AlreadyExists` error.
    pub fn create_dir(&self, name: impl AsRef<OsStr>) -> io::Result<()> {
        let c = component(name.as_ref())?;
        // SAFETY: mkdirat(2) relative to a live directory descriptor.
        cvt(unsafe { libc::mkdirat(self.raw(), c.as_ptr(), 0o700) }).map(|_| ())
    }

    /// Open a child directory, creating it when absent. An existing symlink
    /// or non-directory is an error. A concurrent creator is tolerated.
    pub fn open_or_create_dir(&self, name: impl AsRef<OsStr>) -> io::Result<Dir> {
        let name = name.as_ref();
        if let Some(dir) = self.open_dir(name)? {
            return Ok(dir);
        }
        match self.create_dir(name) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        self.open_dir(name)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "directory vanished"))
    }

    /// The kind of a child, without following it; `None` when absent.
    pub fn kind_of(&self, name: impl AsRef<OsStr>) -> io::Result<Option<Kind>> {
        let c = component(name.as_ref())?;
        // SAFETY: an all-zero `stat` is a valid out-parameter for fstatat(2).
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: fstatat(2) relative to a live directory descriptor.
        let rc =
            unsafe { libc::fstatat(self.raw(), c.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) };
        if rc < 0 {
            let error = io::Error::last_os_error();
            return if error.kind() == io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(error)
            };
        }
        let format = u32::from(st.st_mode) & u32::from(libc::S_IFMT);
        Ok(Some(match format {
            f if f == u32::from(libc::S_IFDIR) => Kind::Dir,
            f if f == u32::from(libc::S_IFLNK) => Kind::Symlink,
            f if f == u32::from(libc::S_IFREG) => Kind::File,
            _ => Kind::Other,
        }))
    }

    /// Every entry (excluding `.` and `..`) with its no-follow kind.
    pub fn entries(&self) -> io::Result<Vec<(OsString, Kind)>> {
        self.entries_while(|| true)
    }

    /// [`Self::entries`] with a checkpoint before every filesystem step:
    /// `proceed` runs before each `readdir` of the listing and before each
    /// `fstatat` of a listed name, and the first `false` ends the walk with an
    /// `Interrupted` error. A stop is noticed within one step, however large
    /// the directory; the names are still all read before any is looked at,
    /// so a caller that removes entries never removes during the listing.
    pub fn entries_while(
        &self,
        mut proceed: impl FnMut() -> bool,
    ) -> io::Result<Vec<(OsString, Kind)>> {
        // SAFETY: fcntl(F_DUPFD_CLOEXEC) duplicates a live descriptor; the
        // duplicate belongs to the DIR stream and is closed by closedir.
        let dup = cvt(unsafe { libc::fcntl(self.raw(), libc::F_DUPFD_CLOEXEC, 0) })?;
        // SAFETY: fdopendir(3) takes ownership of `dup` on success.
        let stream = unsafe { libc::fdopendir(dup) };
        if stream.is_null() {
            let error = io::Error::last_os_error();
            // SAFETY: `dup` is still ours when fdopendir failed.
            unsafe { libc::close(dup) };
            return Err(error);
        }
        // SAFETY: the stream is live; the duplicate shares the original's
        // offset, so rewind before listing.
        unsafe { libc::rewinddir(stream) };
        let mut names: Vec<Vec<u8>> = Vec::new();
        let listed = loop {
            if !proceed() {
                break false;
            }
            // SAFETY: readdir(3) on a live stream; the returned entry is valid
            // until the next call and its name is NUL-terminated.
            let entry = unsafe { libc::readdir(stream) };
            if entry.is_null() {
                break true;
            }
            // SAFETY: see above.
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
                .to_bytes()
                .to_vec();
            if name != b"." && name != b".." {
                names.push(name);
            }
        };
        // SAFETY: the stream is live and closed exactly once.
        unsafe { libc::closedir(stream) };
        if !listed {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            if !proceed() {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let name = OsString::from_vec(name);
            // An entry that vanished since the listing is simply gone.
            if let Some(kind) = self.kind_of(&name)? {
                out.push((name, kind));
            }
        }
        Ok(out)
    }

    /// Open an existing REGULAR file for reading, never following a link.
    pub fn open_file(&self, name: impl AsRef<OsStr>) -> io::Result<File> {
        let c = component(name.as_ref())?;
        let flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC;
        // SAFETY: openat(2) relative to a live directory descriptor.
        let fd = cvt(unsafe { libc::openat(self.raw(), c.as_ptr(), flags) })?;
        let file = File::from(own(fd));
        if !file.metadata()?.file_type().is_file() {
            return Err(invalid("not a regular file"));
        }
        Ok(file)
    }

    /// Create a NEW file for writing (`O_EXCL | O_NOFOLLOW`, mode 0600) and
    /// return it. An existing name of any kind is an error. Callers that
    /// stream a large member write to the returned file and `sync_all` it.
    pub fn create_new(&self, name: impl AsRef<OsStr>) -> io::Result<File> {
        let c = component(name.as_ref())?;
        let flags =
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        // SAFETY: openat(2) relative to a live directory descriptor; the
        // variadic mode argument is a `c_uint` as the ABI requires.
        let fd = cvt(unsafe { libc::openat(self.raw(), c.as_ptr(), flags, 0o600_u32) })?;
        Ok(File::from(own(fd)))
    }

    /// Create a NEW file (`O_EXCL | O_NOFOLLOW`, mode 0600), write `bytes`
    /// and sync it. An existing name of any kind is an error.
    pub fn write_new(&self, name: impl AsRef<OsStr>, bytes: &[u8]) -> io::Result<()> {
        let mut file = self.create_new(name)?;
        file.write_all(bytes)?;
        file.sync_all()
    }

    /// Flush this directory's entries to stable storage (`fsync` on the
    /// descriptor, so no pathname is resolved).
    pub fn sync_all(&self) -> io::Result<()> {
        File::from(self.try_clone()?.fd).sync_all()
    }

    /// The `(device, inode)` of this directory, read from the descriptor.
    /// Two descriptors name the same directory exactly when these are equal,
    /// whatever pathnames or symlinks led to them.
    // The `st_dev`/`st_ino` widths differ by platform; the casts are the
    // portable common type, not a no-op everywhere.
    #[allow(clippy::unnecessary_cast)]
    pub fn identity(&self) -> io::Result<(u64, u64)> {
        // SAFETY: an all-zero `stat` is a valid out-parameter for fstat(2).
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: fstat(2) on a live descriptor owned by `self`.
        cvt(unsafe { libc::fstat(self.raw(), &mut st) })?;
        Ok((st.st_dev as u64, st.st_ino as u64))
    }

    /// Open this directory's parent (`..`) through the descriptor. The
    /// kernel's `..` of a held directory is independent of any pathname used
    /// to reach it, so a walk to the root cannot be redirected by a symlink.
    pub fn open_parent(&self) -> io::Result<Dir> {
        // SAFETY: openat(2) of ".." relative to a live directory descriptor.
        let fd = cvt(unsafe { libc::openat(self.raw(), c"..".as_ptr(), DIR_FLAGS) })?;
        Ok(Dir { fd: own(fd) })
    }

    /// `renameat(self/from -> to_dir/to)`; replaces a regular file at `to`.
    pub fn rename_into(
        &self,
        from: impl AsRef<OsStr>,
        to_dir: &Dir,
        to: impl AsRef<OsStr>,
    ) -> io::Result<()> {
        let from = component(from.as_ref())?;
        let to = component(to.as_ref())?;
        // SAFETY: renameat(2) between two live directory descriptors.
        cvt(unsafe { libc::renameat(self.raw(), from.as_ptr(), to_dir.raw(), to.as_ptr()) })
            .map(|_| ())
    }

    /// Like [`Self::rename_into`], but NEVER replaces: a destination of any
    /// kind that exists at the moment of the rename is `AlreadyExists` and is
    /// left untouched (`renameatx_np(RENAME_EXCL)` on macOS,
    /// `renameat2(RENAME_NOREPLACE)` on Linux). An ordinary `renameat`
    /// silently replaces an empty directory, which would let a destination
    /// created after a preflight check be overwritten.
    pub fn rename_noreplace_into(
        &self,
        from: impl AsRef<OsStr>,
        to_dir: &Dir,
        to: impl AsRef<OsStr>,
    ) -> io::Result<()> {
        let from = component(from.as_ref())?;
        let to = component(to.as_ref())?;
        #[cfg(target_os = "macos")]
        // SAFETY: renameatx_np(2) between two live directory descriptors.
        let rc = unsafe {
            libc::renameatx_np(
                self.raw(),
                from.as_ptr(),
                to_dir.raw(),
                to.as_ptr(),
                libc::RENAME_EXCL,
            )
        };
        #[cfg(target_os = "linux")]
        // SAFETY: renameat2(2) between two live directory descriptors.
        let rc = unsafe {
            libc::renameat2(
                self.raw(),
                from.as_ptr(),
                to_dir.raw(),
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let rc: libc::c_int = {
            let _ = (&from, &to, to_dir);
            return Err(io::Error::from(io::ErrorKind::Unsupported));
        };
        cvt(rc).map(|_| ())
    }

    fn unlink(&self, name: &OsStr, directory: bool) -> io::Result<()> {
        let c = component(name)?;
        let flags = if directory { libc::AT_REMOVEDIR } else { 0 };
        // SAFETY: unlinkat(2) relative to a live directory descriptor. On a
        // symlink it removes the link itself, never its target.
        cvt(unsafe { libc::unlinkat(self.raw(), c.as_ptr(), flags) }).map(|_| ())
    }

    /// Remove a child tree: files and symlinks are unlinked (a symlink's
    /// target is never touched), directories are emptied through descriptors
    /// opened `O_NOFOLLOW` and removed with `unlinkat(AT_REMOVEDIR)`.
    /// `Ok(false)` when the name is absent.
    pub fn remove_tree(&self, name: impl AsRef<OsStr>) -> io::Result<bool> {
        self.remove_tree_at(name.as_ref(), 0)
    }

    /// `unlinkat(AT_REMOVEDIR)`: remove an EMPTY child directory. A
    /// non-empty directory, a symlink or a file is an error and untouched.
    pub fn remove_dir(&self, name: impl AsRef<OsStr>) -> io::Result<()> {
        self.unlink(name.as_ref(), true)
    }

    fn remove_tree_at(&self, name: &OsStr, depth: usize) -> io::Result<bool> {
        if depth > MAX_DEPTH {
            return Err(io::Error::other(
                "directory tree is deeper than a generation",
            ));
        }
        match self.kind_of(name)? {
            None => Ok(false),
            Some(Kind::Dir) => {
                let Some(child) = self.open_dir(name)? else {
                    return Ok(false);
                };
                for (entry, kind) in child.entries()? {
                    if kind == Kind::Dir {
                        child.remove_tree_at(&entry, depth + 1)?;
                    } else {
                        child.unlink(&entry, false)?;
                    }
                }
                drop(child);
                self.unlink(name, true)?;
                Ok(true)
            }
            Some(_) => {
                self.unlink(name, false)?;
                Ok(true)
            }
        }
    }
}

/// Map an I/O error to the shared error type: a symlink or non-directory
/// where a directory is required is the named `repair_path_conflict`.
pub fn conflict_or_io(error: io::Error, what: &str) -> FoundryError {
    match error.raw_os_error() {
        Some(libc::ELOOP) | Some(libc::ENOTDIR) => {
            FoundryError::RepairPathConflict(format!("{what} is a symlink or not a directory"))
        }
        _ => FoundryError::from(error),
    }
}

impl crate::store::Engine {
    /// A private duplicate of the store-directory descriptor the Engine
    /// opened (and the kernel verified as a directory, `O_NOFOLLOW` on the
    /// final component) exactly once, when it bound and locked the store.
    /// Every semantic purge, publication and generation validation starts
    /// here: the store pathname is never resolved again during the Engine's
    /// life, so an ancestor renamed or substituted after the bind cannot
    /// redirect semantic work to another tree.
    pub(crate) fn semantic_anchor(&self) -> FResult<Dir> {
        self.semantic_dir.try_clone().map_err(FoundryError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_removal_never_follows_a_symlink_or_a_substituted_parent() {
        let scratch = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(outside.path().join("victim/inner")).unwrap();
        std::fs::write(outside.path().join("victim/inner/keep.txt"), b"keep").unwrap();

        let root = scratch.path().join("root");
        std::fs::create_dir_all(root.join("gen/sub")).unwrap();
        std::fs::write(root.join("gen/sub/file.txt"), b"x").unwrap();
        // A symlink INSIDE the tree is unlinked, never followed.
        std::os::unix::fs::symlink(outside.path().join("victim"), root.join("gen/link")).unwrap();

        let held = Dir::open_path(&root).unwrap();
        // Substitute the parent path after the descriptor was opened.
        let moved = scratch.path().join("moved");
        std::fs::rename(&root, &moved).unwrap();
        std::os::unix::fs::symlink(outside.path(), &root).unwrap();

        assert!(held.remove_tree("gen").unwrap());
        assert!(!held.remove_tree("gen").unwrap(), "already gone");
        assert_eq!(
            std::fs::read(outside.path().join("victim/inner/keep.txt")).unwrap(),
            b"keep"
        );
        assert_eq!(std::fs::read_dir(&moved).unwrap().count(), 0);
    }

    #[test]
    fn opens_refuse_links_and_non_directories() {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::create_dir(scratch.path().join("real")).unwrap();
        std::fs::write(scratch.path().join("file"), b"f").unwrap();
        std::os::unix::fs::symlink(scratch.path().join("real"), scratch.path().join("link"))
            .unwrap();
        let dir = Dir::open_path(scratch.path()).unwrap();
        assert!(dir.open_dir("real").unwrap().is_some());
        assert!(dir.open_dir("absent").unwrap().is_none());
        for name in ["link", "file"] {
            let error = dir.open_dir(name).err().expect("refused");
            assert_eq!(
                conflict_or_io(error, "x").code(),
                "repair_path_conflict",
                "{name}"
            );
        }
        assert!(Dir::open_path(&scratch.path().join("link")).is_err());
        assert!(dir.open_dir("../real").is_err(), "no path traversal");
    }

    #[test]
    fn a_no_replace_rename_never_replaces_an_existing_destination() {
        use std::os::unix::fs::MetadataExt as _;
        let scratch = tempfile::tempdir().unwrap();
        std::fs::create_dir(scratch.path().join("src")).unwrap();
        std::fs::write(scratch.path().join("src/payload"), b"payload").unwrap();
        // An EMPTY directory is exactly what an ordinary rename would replace.
        std::fs::create_dir(scratch.path().join("dest")).unwrap();
        let identity = std::fs::metadata(scratch.path().join("dest"))
            .unwrap()
            .ino();
        let dir = Dir::open_path(scratch.path()).unwrap();
        let error = dir
            .rename_noreplace_into("src", &dir, "dest")
            .expect_err("an existing destination is refused");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::metadata(scratch.path().join("dest"))
                .unwrap()
                .ino(),
            identity,
            "the foreign directory is the same inode and still empty"
        );
        assert_eq!(
            std::fs::read_dir(scratch.path().join("dest"))
                .unwrap()
                .count(),
            0
        );
        assert!(
            scratch.path().join("src/payload").exists(),
            "the source stays"
        );
        // With the destination gone it renames.
        std::fs::remove_dir(scratch.path().join("dest")).unwrap();
        dir.rename_noreplace_into("src", &dir, "dest").unwrap();
        assert_eq!(
            std::fs::read(scratch.path().join("dest/payload")).unwrap(),
            b"payload"
        );
        assert!(!scratch.path().join("src").exists());
    }

    #[test]
    fn identity_and_parent_walk_follow_descriptors_not_pathnames() {
        let scratch = tempfile::tempdir().unwrap();
        let real = scratch.path().join("real");
        std::fs::create_dir_all(real.join("inner/deeper")).unwrap();
        let link = scratch.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let deeper = Dir::open_path(&real.join("inner/deeper")).unwrap();
        let via_link = Dir::open_path(&link.canonicalize().unwrap()).unwrap();
        // `..` twice from the held directory is `real`, however it was reached.
        let two_up = deeper.open_parent().unwrap().open_parent().unwrap();
        assert_eq!(two_up.identity().unwrap(), via_link.identity().unwrap());
        assert_ne!(deeper.identity().unwrap(), two_up.identity().unwrap());
        // The walk ends at the root: `..` of `/` is `/`.
        let mut current = Dir::open_path(Path::new("/")).unwrap();
        let up = current.open_parent().unwrap();
        assert_eq!(up.identity().unwrap(), current.identity().unwrap());
        current = up;
        assert!(current.identity().is_ok());
    }

    #[test]
    fn create_new_refuses_an_existing_name_and_a_dangling_symlink() {
        let scratch = tempfile::tempdir().unwrap();
        let dir = Dir::open_path(scratch.path()).unwrap();
        dir.write_new("file", b"one").unwrap();
        assert!(
            dir.create_new("file").is_err(),
            "O_EXCL on an existing file"
        );
        std::os::unix::fs::symlink("/nonexistent/target", scratch.path().join("dangling")).unwrap();
        assert!(
            dir.create_new("dangling").is_err(),
            "O_EXCL never follows a link"
        );
        assert!(!std::path::Path::new("/nonexistent/target").exists());
        dir.sync_all().unwrap();
    }

    /// Review M5 (009 scratch reclamation): every step of a listing is a
    /// checkpoint. Refusing step `k`, for each `k` from the first name read to
    /// the last kind check, ends the walk at once with `Interrupted`.
    #[test]
    fn a_listing_stops_at_the_first_refused_step_of_either_pass() {
        const FILES: usize = 20;
        let scratch = tempfile::tempdir().unwrap();
        for n in 0..FILES {
            std::fs::write(scratch.path().join(format!("f{n}")), b"x").unwrap();
        }
        let dir = Dir::open_path(scratch.path()).unwrap();
        let mut steps = 0;
        let all = dir
            .entries_while(|| {
                steps += 1;
                true
            })
            .unwrap();
        assert_eq!(all.len(), FILES);
        assert!(all.iter().all(|(_, kind)| *kind == Kind::File));
        // FILES names and the end of the listing read, then FILES kinds.
        assert!(steps > 2 * FILES, "{steps} steps");
        for refused in 0..steps {
            let mut taken = 0;
            let error = dir
                .entries_while(|| {
                    taken += 1;
                    taken <= refused
                })
                .expect_err("the refused step ends the listing");
            assert_eq!(error.kind(), io::ErrorKind::Interrupted, "step {refused}");
            assert_eq!(taken, refused + 1, "no step runs after the refusal");
        }
    }
}
