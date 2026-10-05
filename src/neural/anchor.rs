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
        loop {
            // SAFETY: readdir(3) on a live stream; the returned entry is valid
            // until the next call and its name is NUL-terminated.
            let entry = unsafe { libc::readdir(stream) };
            if entry.is_null() {
                break;
            }
            // SAFETY: see above.
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
                .to_bytes()
                .to_vec();
            if name != b"." && name != b".." {
                names.push(name);
            }
        }
        // SAFETY: the stream is live and closed exactly once.
        unsafe { libc::closedir(stream) };
        let mut out = Vec::with_capacity(names.len());
        for name in names {
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

    /// Create a NEW file (`O_EXCL | O_NOFOLLOW`, mode 0600), write `bytes`
    /// and sync it. An existing name of any kind is an error.
    pub fn write_new(&self, name: impl AsRef<OsStr>, bytes: &[u8]) -> io::Result<()> {
        let c = component(name.as_ref())?;
        let flags =
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        // SAFETY: openat(2) relative to a live directory descriptor; the
        // variadic mode argument is a `c_uint` as the ABI requires.
        let fd = cvt(unsafe { libc::openat(self.raw(), c.as_ptr(), flags, 0o600_u32) })?;
        let mut file = File::from(own(fd));
        file.write_all(bytes)?;
        file.sync_all()
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
}
