//! 009 semantic profile file: a bounded, versioned JSON document naming the
//! model directory, the worker, the runtime and the expected document
//! function. Parsed before any worker exists; every named object is resolved
//! and verified before the publisher loader runs. Nothing is downloaded.
use super::provider::{FunctionDescriptor, ProviderError};
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
        self.descriptor.validate().map_err(invalid)
    }

    /// Verify every artifact file and the requirements listing against the
    /// descriptor by reading them through descriptors opened without
    /// following a final symlink. Returns the verified total byte count.
    pub fn verify_artifacts(&self) -> Result<u64, ProviderError> {
        let mut total = 0u64;
        for file in &self.descriptor.artifact_files {
            let path = self.model_dir.join(&file.name);
            let (sha, bytes) = hash_regular_file(&path)?;
            if sha != file.sha256 {
                return Err(invalid(format!(
                    "{} does not match its expected SHA-256",
                    file.name
                )));
            }
            total += bytes;
        }
        let (sha, _) = hash_regular_file(&self.runtime.requirements)?;
        if sha != self.descriptor.runtime.requirements_sha256 {
            return Err(invalid("runtime requirements do not match the descriptor"));
        }
        Ok(total)
    }
}

/// SHA-256 and length of a regular file opened with `O_NOFOLLOW`; symlinks,
/// FIFOs and directories are refused by name, and the open never blocks.
pub fn hash_regular_file(path: &Path) -> Result<(String, u64), ProviderError> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::OpenOptionsExt;
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
    }
    Ok((format!("{:x}", hasher.finalize()), bytes))
}
