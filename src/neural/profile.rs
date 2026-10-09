//! 009 semantic profile file: a bounded, versioned JSON document naming the
//! model directory, the worker, the document function and the retrieval
//! recipe (query template, card limit, batch size). Parsed before any worker
//! exists; every named object is resolved and verified before the worker
//! loads. Nothing is downloaded.
//!
//! 009 T004: profile v2 pins a GGUF, the llama.cpp commit, pooling, the
//! output dimension and the templates. A v1 profile (descriptor v1, the MLX
//! worker and its Python closure) is refused at load with
//! `profile_unsupported`; its retained cache rows and generations stay until
//! an explicit `semantic purge`.
use super::provider::{
    DocumentLimits, FunctionDescriptor, MAX_DOCUMENT_BATCH, ProviderError, SERVING_LIMIT_TOKENS,
    check_template,
};
use crate::control::Control;
use crate::error::FoundryError;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const PROFILE_VERSION: u32 = 2;
pub const MAX_PROFILE_BYTES: u64 = 64 * 1024;
/// Default supervised memory ceiling (D001 measured 1.82 GiB for 8 × 1024).
pub const DEFAULT_MEMORY_CEILING_BYTES: u64 = 3 * 1024 * 1024 * 1024;
/// Default card limit: tokens of one rendered card, template and special
/// tokens included (spec 009 T004).
pub const DEFAULT_CARD_TOKENS: u32 = 128;
/// Default sequences per document call.
pub const DEFAULT_BATCH: u32 = 8;
/// The smallest card limit: the template, the special tokens and an
/// address line must fit.
pub const MIN_CARD_TOKENS: u32 = 16;

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

fn default_card_tokens() -> u32 {
    DEFAULT_CARD_TOKENS
}

fn default_batch() -> u32 {
    DEFAULT_BATCH
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
    pub descriptor: FunctionDescriptor,
    /// The query template ([`super::provider::TEXT_SLOT`] exactly once):
    /// part of the retrieval profile, never of the document function.
    pub query_template: String,
    /// The card limit: tokens of one rendered card. Part of the partition
    /// recipe; changing it re-partitions, and only cards whose text changed
    /// get new keys.
    #[serde(default = "default_card_tokens")]
    pub card_tokens: u32,
    /// Sequences per document call (one llama.cpp context).
    #[serde(default = "default_batch")]
    pub batch: u32,
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

/// The version fields alone, read before the strict parse so a profile this
/// build no longer serves is refused by name, not as a decode failure.
#[derive(Deserialize)]
struct Versions {
    v: Option<u32>,
    descriptor: Option<DescriptorVersion>,
}

#[derive(Deserialize)]
struct DescriptorVersion {
    v: Option<u32>,
}

/// The refusal of a descriptor v1 (MLX) profile.
pub fn unsupported_v1() -> ProviderError {
    ProviderError::ProfileUnsupported(
        "descriptor v1 (MLX worker) profiles are no longer served; semantic retrieval stays \
         off and the retained cache rows and generations are kept until `semantic purge`; \
         install a v2 (llama.cpp) profile"
            .into(),
    )
}

impl SemanticProfile {
    /// Read at most [`MAX_PROFILE_BYTES`] (a regular file, not a symlink),
    /// refuse a v1 profile with `profile_unsupported`, parse strictly and
    /// validate structure. No model files are opened.
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
        if let Ok(versions) = serde_json::from_slice::<Versions>(&bytes)
            && (versions.v == Some(1) || versions.descriptor.and_then(|d| d.v) == Some(1))
        {
            return Err(unsupported_v1());
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
        check_template("query_template", &self.query_template).map_err(invalid)?;
        let (cards, batch) = (self.card_tokens as usize, self.batch as usize);
        if self.card_tokens < MIN_CARD_TOKENS || cards > SERVING_LIMIT_TOKENS {
            return Err(invalid(format!(
                "card_tokens {cards} is outside {MIN_CARD_TOKENS}..={SERVING_LIMIT_TOKENS}"
            )));
        }
        if batch == 0 || batch > MAX_DOCUMENT_BATCH {
            return Err(invalid(format!(
                "batch {batch} is outside 1..={MAX_DOCUMENT_BATCH}"
            )));
        }
        if batch * cards > SERVING_LIMIT_TOKENS {
            return Err(invalid(format!(
                "batch {batch} × card_tokens {cards} exceeds one {SERVING_LIMIT_TOKENS}-token \
                 llama.cpp context"
            )));
        }
        self.check_scratch_disjoint()?;
        self.descriptor.validate().map_err(invalid)
    }

    /// The limits of one document call under this profile.
    pub fn document_limits(&self) -> DocumentLimits {
        DocumentLimits {
            inputs: self.batch as usize,
            tokens: self.card_tokens as usize,
        }
    }

    /// The scratch root is the worker's only WRITE grant. It must not
    /// overlap the read-only model directory: a scratch directory inside (or
    /// containing) it would let the worker rewrite files the profile hashes,
    /// or let a hash-verified file move under the write grant. Paths are
    /// resolved through symlinks where they exist, so an alias cannot
    /// smuggle an overlap past the check.
    pub fn check_scratch_disjoint(&self) -> Result<(), ProviderError> {
        let scratch = resolve(&self.worker.scratch_root)?;
        let other = resolve(&self.model_dir)?;
        if scratch == other || overlaps(&scratch, &other) || overlaps(&other, &scratch) {
            return Err(invalid(format!(
                "worker.scratch_root ({}) overlaps {} ({}) after resolving symlinks",
                scratch.display(),
                self.model_dir.display(),
                other.display()
            )));
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
    /// Verify every artifact file against the descriptor by reading it
    /// through a descriptor opened without following a final symlink.
    /// Returns the verified total byte count.
    pub fn verify_artifacts(&self) -> Result<u64, ProviderError> {
        self.verify_artifacts_until(&Control::unbounded())
    }

    /// [`Self::verify_artifacts`] under a caller's [`Control`]: the streamed
    /// hashing checks it every chunk, so a deadline or cancellation ends a
    /// large verification promptly (`Timeout` / `Cancelled`).
    pub fn verify_artifacts_until(&self, control: &Control) -> Result<u64, ProviderError> {
        Ok(self.verified_artifacts_until(control)?.total_bytes)
    }

    /// A launch-local capability over the exact opened and hashed files.
    /// Consumers use retained descriptors/bytes, never reopen a checked pathname.
    pub fn verified_artifacts_until(
        &self,
        control: &Control,
    ) -> Result<VerifiedArtifacts, ProviderError> {
        if self.descriptor.artifact_files.len() > 64 {
            return Err(invalid("artifact inventory exceeds 64 open files"));
        }
        let mut verified = VerifiedArtifacts {
            function_digest: self.descriptor.digest(),
            model_dir: self.model_dir.clone(),
            files: Vec::new(),
            tokenizer: Vec::new(),
            total_bytes: 0,
        };
        for file in &self.descriptor.artifact_files {
            let path = self.model_dir.join(&file.name);
            let opened = open_regular(&path)?;
            let version = opened.metadata().map_err(|e| invalid(e.to_string()))?;
            let tokenizer = file.name == "tokenizer.json";
            if tokenizer && version.len() > 64 * 1024 * 1024 {
                return Err(invalid("tokenizer.json exceeds the 64 MiB artifact limit"));
            }
            let (sha, bytes, retained) = hash_open_file(&opened, &path, control, tokenizer)?;
            if sha != file.sha256 {
                return Err(invalid(format!(
                    "{} does not match its expected SHA-256",
                    file.name
                )));
            }
            check_version(&opened, &version, &path)?;
            verified.total_bytes = verified
                .total_bytes
                .checked_add(bytes)
                .ok_or_else(|| invalid("artifact byte total overflow"))?;
            if tokenizer {
                verified.tokenizer = retained;
            }
            verified.files.push(VerifiedFile {
                name: file.name.clone(),
                file: opened,
                version,
            });
        }
        verified.check(self, control)?;
        Ok(verified)
    }
}

struct VerifiedFile {
    name: String,
    file: std::fs::File,
    version: std::fs::Metadata,
}

/// Private contents bind a verification receipt to live file descriptors and
/// immutable tokenizer bytes. It cannot be deserialized or forged by a caller.
pub struct VerifiedArtifacts {
    function_digest: String,
    model_dir: PathBuf,
    files: Vec<VerifiedFile>,
    tokenizer: Vec<u8>,
    pub total_bytes: u64,
}

impl VerifiedArtifacts {
    pub fn check(&self, profile: &SemanticProfile, control: &Control) -> Result<(), ProviderError> {
        if let Some(stop) = control_stop(control) {
            return Err(stop);
        }
        if self.function_digest != profile.descriptor.digest()
            || self.model_dir != profile.model_dir
        {
            return Err(invalid(
                "verified artifacts belong to a different profile or directory",
            ));
        }
        for artifact in &self.files {
            let path = self.model_dir.join(&artifact.name);
            check_version(&artifact.file, &artifact.version, &path)?;
            check_version(&open_regular(&path)?, &artifact.version, &path)?;
        }
        Ok(())
    }

    #[cfg(feature = "semantic")]
    pub(crate) fn tokenizer_bytes(&self) -> Result<&[u8], ProviderError> {
        if self.tokenizer.is_empty() {
            return Err(invalid(
                "the descriptor does not pin a nonempty tokenizer.json",
            ));
        }
        Ok(&self.tokenizer)
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn model_fd(
        &self,
        profile: &SemanticProfile,
    ) -> Result<std::os::fd::RawFd, ProviderError> {
        use std::os::fd::AsRawFd;
        self.files
            .iter()
            .find(|file| file.name == profile.descriptor.gguf)
            .map(|file| file.file.as_raw_fd())
            .ok_or_else(|| invalid("verified GGUF descriptor is absent"))
    }
}

fn check_version(
    file: &std::fs::File,
    expected: &std::fs::Metadata,
    path: &Path,
) -> Result<(), ProviderError> {
    use std::os::unix::fs::MetadataExt;
    let current = file.metadata().map_err(|e| invalid(e.to_string()))?;
    if (
        current.dev(),
        current.ino(),
        current.len(),
        current.mtime(),
        current.mtime_nsec(),
        current.ctime(),
        current.ctime_nsec(),
    ) != (
        expected.dev(),
        expected.ino(),
        expected.len(),
        expected.mtime(),
        expected.mtime_nsec(),
        expected.ctime(),
        expected.ctime_nsec(),
    ) {
        return Err(invalid(format!(
            "{} changed after artifact verification",
            path.display()
        )));
    }
    Ok(())
}

fn open_regular(path: &Path) -> Result<std::fs::File, ProviderError> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
    if !file
        .metadata()
        .map_err(|e| invalid(e.to_string()))?
        .is_file()
    {
        return Err(invalid(format!("{} is not a regular file", path.display())));
    }
    Ok(file)
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
    if let Some(stop) = control_stop(control) {
        return Err(stop);
    }
    let file = open_regular(path)?;
    let version = file.metadata().map_err(|e| invalid(e.to_string()))?;
    let (digest, bytes, _) = hash_open_file(&file, path, control, false)?;
    check_version(&file, &version, path)?;
    Ok((digest, bytes))
}

fn hash_open_file(
    file: &std::fs::File,
    path: &Path,
    control: &Control,
    retain: bool,
) -> Result<(String, u64, Vec<u8>), ProviderError> {
    use sha2::{Digest, Sha256};
    if let Some(stop) = control_stop(control) {
        return Err(stop);
    }
    neural_fault!(ARTIFACT_HASH, Some(control), &path.to_string_lossy())
        .map_err(|e| invalid(e.to_string()))?;
    let mut hasher = Sha256::new();
    use std::os::unix::fs::FileExt;
    let mut buf = vec![0u8; 1 << 20];
    let mut bytes = 0u64;
    let mut retained = Vec::new();
    loop {
        let n = file
            .read_at(&mut buf, bytes)
            .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        if retain {
            if retained.len().saturating_add(n) > 64 * 1024 * 1024 {
                return Err(invalid("tokenizer.json exceeds the 64 MiB artifact limit"));
            }
            retained.extend_from_slice(&buf[..n]);
        }
        bytes += n as u64;
        if let Some(stop) = control_stop(control) {
            return Err(stop);
        }
    }
    Ok((format!("{:x}", hasher.finalize()), bytes, retained))
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
