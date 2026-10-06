//! 009 semantic profile file: a bounded, versioned JSON document naming the
//! model directory, the worker, the runtime and the expected document
//! function. Parsed before any worker exists; every named object is resolved
//! and verified before the publisher loader runs. Nothing is downloaded.
use super::provider::{FunctionDescriptor, ProviderError};
use crate::control::Control;
use crate::error::FoundryError;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const PROFILE_VERSION: u32 = 1;
pub const MAX_PROFILE_BYTES: u64 = 64 * 1024;
/// Default supervised memory ceiling (D001 measured 1.82 GiB for 8 × 1024).
pub const DEFAULT_MEMORY_CEILING_BYTES: u64 = 3 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerSpec {
    /// Absolute path of the signed bundle (`…/FoundryEmbed.app`) whose
    /// executable is `foundry-embed`.
    pub bundle: PathBuf,
    /// Lowercase hex SHA-256 of the bundle's executable.
    pub executable_sha256: String,
    /// Absolute owner-private scratch root granted read-write to the signed
    /// bundle (its only write grant). The bundle script grants exactly this
    /// path and the supervisor creates per-run directories under it, so the
    /// two always agree. Not part of the document function.
    pub scratch_root: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSpec {
    /// Absolute `PYTHONHOME` of the operator's interpreter.
    pub python_home: PathBuf,
    /// Absolute site-packages directory holding the pinned closure; the only
    /// import path added besides the standard library.
    pub site_packages: PathBuf,
    /// Absolute path of the frozen requirements listing whose SHA-256 is the
    /// descriptor's `runtime.requirements_sha256`.
    pub requirements: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticProfile {
    pub v: u32,
    /// Human label only; not identity.
    pub name: String,
    /// Absolute model directory; `descriptor.artifact_files` are relative to it.
    pub model_dir: PathBuf,
    pub worker: WorkerSpec,
    pub runtime: RuntimeSpec,
    pub descriptor: FunctionDescriptor,
    pub memory_ceiling_bytes: u64,
    /// Upper bound on worker start, verification and model load.
    pub load_timeout_seconds: u64,
}

fn invalid(message: impl Into<String>) -> ProviderError {
    ProviderError::ProfileInvalid(message.into())
}

fn hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

impl SemanticProfile {
    /// Read at most [`MAX_PROFILE_BYTES`] (a regular file, not a symlink),
    /// parse strictly and validate structure. No model files are opened.
    pub fn load(path: &Path) -> Result<Self, ProviderError> {
        let meta = std::fs::symlink_metadata(path)
            .map_err(|e| invalid(format!("profile {}: {e}", path.display())))?;
        if !meta.file_type().is_file() {
            return Err(invalid(format!(
                "profile {} is not a regular file",
                path.display()
            )));
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .and_then(|f| f.take(MAX_PROFILE_BYTES + 1).read_to_end(&mut bytes))
            .map_err(|e| invalid(format!("profile {}: {e}", path.display())))?;
        if bytes.len() as u64 > MAX_PROFILE_BYTES {
            return Err(invalid(format!(
                "profile exceeds {MAX_PROFILE_BYTES} bytes"
            )));
        }
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|e| invalid(format!("profile JSON: {e}")))?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<(), ProviderError> {
        if self.v != PROFILE_VERSION {
            return Err(invalid(format!(
                "profile version {} is not {PROFILE_VERSION}",
                self.v
            )));
        }
        for (what, path) in [
            ("model_dir", &self.model_dir),
            ("worker.bundle", &self.worker.bundle),
            ("runtime.python_home", &self.runtime.python_home),
            ("runtime.site_packages", &self.runtime.site_packages),
            ("runtime.requirements", &self.runtime.requirements),
            ("worker.scratch_root", &self.worker.scratch_root),
        ] {
            if !path.is_absolute()
                || path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err(invalid(format!(
                    "{what} must be an absolute path without `..`"
                )));
            }
        }
        if !hex64(&self.worker.executable_sha256) {
            return Err(invalid("worker.executable_sha256 must be a SHA-256"));
        }
        if self.memory_ceiling_bytes == 0 || self.load_timeout_seconds == 0 {
            return Err(invalid(
                "memory_ceiling_bytes and load_timeout_seconds must be positive",
            ));
        }
        if self.load_timeout_seconds > MAX_LOAD_TIMEOUT_SECONDS {
            return Err(invalid(format!(
                "load_timeout_seconds {} exceeds the maximum of \
                 {MAX_LOAD_TIMEOUT_SECONDS} (the acquisition bound must stay \
                 representable and bounded)",
                self.load_timeout_seconds
            )));
        }
        self.check_scratch_disjoint()?;
        self.descriptor.validate().map_err(invalid)
    }

    /// The scratch root is the worker's only WRITE grant. It must not
    /// overlap any read-only grant: a scratch directory inside (or
    /// containing) the model directory, the Python home, the site-packages
    /// tree or the requirements listing would let the worker rewrite files
    /// the profile hashes, or let a hash-verified file move under the write
    /// grant. Paths are resolved through symlinks where they exist, so an
    /// alias cannot smuggle an overlap past the check.
    pub fn check_scratch_disjoint(&self) -> Result<(), ProviderError> {
        let scratch = resolve(&self.worker.scratch_root)?;
        for what in [
            &self.model_dir,
            &self.runtime.python_home,
            &self.runtime.site_packages,
            &self.runtime.requirements,
        ] {
            let other = resolve(what)?;
            if scratch == other || overlaps(&scratch, &other) || overlaps(&other, &scratch) {
                return Err(invalid(format!(
                    "worker.scratch_root ({}) overlaps {} ({}) after resolving symlinks",
                    scratch.display(),
                    what.display(),
                    other.display()
                )));
            }
        }
        Ok(())
    }
}

/// The largest `load_timeout_seconds` a profile may name.
const MAX_LOAD_TIMEOUT_SECONDS: u64 = 3600;

/// `path` canonicalized: the nearest EXISTING ancestor is canonicalized
/// (which resolves every symlink on it and fails closed on cycles, `ELOOP`
/// and permission errors alike), and the missing trailing components are
/// appended unchanged. A path that does not exist yet therefore still
/// resolves through its existing ancestors, and nothing is guessed. 013's
/// learning profile judges its scratch/read-only separation with this too.
pub(crate) fn resolve(path: &Path) -> Result<PathBuf, ProviderError> {
    match std::fs::canonicalize(path) {
        Ok(canonical) => return Ok(canonical),
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(invalid(format!(
                "{} cannot be resolved ({e}); refusing to guess",
                path.display()
            )));
        }
        Err(_) => {}
    }
    let mut existing = path.to_path_buf();
    let mut suffix = PathBuf::new();
    loop {
        let Some(parent) = existing.parent().map(Path::to_path_buf) else {
            return Err(invalid(format!(
                "{} has no existing ancestor to resolve through",
                path.display()
            )));
        };
        // Components are removed deepest-first, so each one is prepended to
        // keep the missing suffix in its original order.
        suffix = match existing.file_name() {
            Some(name) if suffix.as_os_str().is_empty() => PathBuf::from(name),
            Some(name) => Path::new(name).join(&suffix),
            None => suffix,
        };
        existing = parent;
        if existing.as_os_str().is_empty() {
            return Err(invalid(format!(
                "{} has no existing ancestor to resolve through",
                path.display()
            )));
        }
        match std::fs::symlink_metadata(&existing) {
            Ok(_) => break,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(invalid(format!(
                    "{} cannot be resolved ({e}); refusing to guess",
                    path.display()
                )));
            }
        }
    }
    let canonical = std::fs::canonicalize(&existing).map_err(|e| {
        invalid(format!(
            "{} cannot be resolved ({e}); refusing to guess",
            path.display()
        ))
    })?;
    Ok(canonical.join(suffix))
}

/// True when `inner` lies strictly below `outer` (a proper subpath).
pub(crate) fn overlaps(outer: &Path, inner: &Path) -> bool {
    inner.starts_with(outer) && inner != outer
}

impl SemanticProfile {
    /// Verify every artifact file and the requirements listing against the
    /// descriptor by reading them through descriptors opened without
    /// following a final symlink. Returns the verified total byte count.
    pub fn verify_artifacts(&self) -> Result<u64, ProviderError> {
        self.verify_artifacts_until(&Control::unbounded())
    }

    /// [`Self::verify_artifacts`] under a caller's [`Control`]: the streamed
    /// hashing checks it every chunk, so a deadline or cancellation ends a
    /// large verification promptly (`Timeout` / `Cancelled`).
    pub fn verify_artifacts_until(&self, control: &Control) -> Result<u64, ProviderError> {
        let mut total = 0u64;
        for file in &self.descriptor.artifact_files {
            let path = self.model_dir.join(&file.name);
            let (sha, bytes) = hash_regular_file_until(&path, control)?;
            if sha != file.sha256 {
                return Err(invalid(format!(
                    "{} does not match its expected SHA-256",
                    file.name
                )));
            }
            total += bytes;
        }
        let (sha, _) = hash_regular_file_until(&self.runtime.requirements, control)?;
        if sha != self.descriptor.runtime.requirements_sha256 {
            return Err(invalid("runtime requirements do not match the descriptor"));
        }
        Ok(total)
    }
}

/// What stopped `control`, as the provider error that names it: a passed
/// deadline is `Timeout`, a cancellation `Cancelled`; `None` while it runs.
pub fn control_stop(control: &Control) -> Option<ProviderError> {
    match control.check() {
        Ok(()) => None,
        Err(FoundryError::DeadlineExceeded(_)) => Some(ProviderError::Timeout),
        Err(_) => Some(ProviderError::Cancelled),
    }
}

/// SHA-256 and length of a regular file opened with `O_NOFOLLOW`; symlinks,
/// FIFOs and directories are refused by name, and the open never blocks.
pub fn hash_regular_file(path: &Path) -> Result<(String, u64), ProviderError> {
    hash_regular_file_until(path, &Control::unbounded())
}

/// [`hash_regular_file`] streamed in 1 MiB chunks with `control` checked
/// before the open and after every chunk: a deadline or cancellation ends a
/// large or slow read at once (`Timeout` / `Cancelled`) instead of after the
/// whole file.
pub fn hash_regular_file_until(
    path: &Path,
    control: &Control,
) -> Result<(String, u64), ProviderError> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::OpenOptionsExt;
    if let Some(stop) = control_stop(control) {
        return Err(stop);
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
    let meta = file
        .metadata()
        .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
    if !meta.file_type().is_file() {
        return Err(invalid(format!("{} is not a regular file", path.display())));
    }
    let mut hasher = Sha256::new();
    let mut reader = std::io::BufReader::with_capacity(1 << 20, file);
    let mut buf = vec![0u8; 1 << 20];
    let mut bytes = 0u64;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        bytes += n as u64;
        if let Some(stop) = control_stop(control) {
            return Err(stop);
        }
    }
    Ok((format!("{:x}", hasher.finalize()), bytes))
}

#[cfg(test)]
mod tests {
    use super::resolve;

    #[test]
    fn missing_components_keep_their_order_beneath_the_nearest_existing_ancestor() {
        let dir = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(dir.path()).unwrap();
        assert_eq!(
            resolve(&base.join("a").join("b")).unwrap(),
            base.join("a").join("b")
        );
        assert_eq!(resolve(&base.join("leaf")).unwrap(), base.join("leaf"));
        // An existing symlinked ancestor is resolved; the missing tail follows it.
        std::fs::create_dir(base.join("real")).unwrap();
        std::os::unix::fs::symlink(base.join("real"), base.join("alias")).unwrap();
        assert_eq!(
            resolve(&base.join("alias").join("x").join("y")).unwrap(),
            base.join("real").join("x").join("y")
        );
    }
}
