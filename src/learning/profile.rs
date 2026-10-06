//! 013 T002 learning-worker isolation profile: the owner-authorized
//! development profile (spec "Development isolation, 2026-10-05"), never
//! advertised as production isolation. It mirrors 009's semantic profile
//! worker section ([`crate::neural::profile::WorkerSpec`]: bundle, executable
//! SHA-256, scratch root) and adds the two read-only grants the signed
//! bundle carries — the checkpoint directory and the LibTorch lib directory
//! — the load timeout and the resource ceilings the installed profile
//! admits. `scripts/learn-worker-bundle.sh` builds the bundle from exactly
//! this file, so the grants and the supervisor cannot disagree.
//!
//! The enforcement matrix ([`PROVIDED`]) is what this profile actually
//! enforces, checked before launch: a policy that requests more than the
//! profile provides for any resource is refused (`enforcement_unavailable`),
//! never run at a weaker level.
use super::{Enforcement, LearningPolicy, Level, fail, read_capped, strict_json};
use crate::error::FResult;
use crate::neural::profile::{WorkerSpec, overlaps, resolve};
use crate::neural::provider::ProviderError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const PROFILE_VERSION: u32 = 1;
const MAX_PROFILE_BYTES: u64 = 64 * 1024;
/// Worker start plus checkpoint load; the run's wall clock still applies.
const MAX_LOAD_TIMEOUT_SECONDS: u64 = 3600;
/// The bundle's executable.
pub const EXECUTABLE: &str = "foundry-learn";

/// The most the installed profile admits; a policy may ask for less.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ceilings {
    pub memory_bytes: u64,
    pub wall_seconds: u64,
    pub output_bytes: u64,
    pub cpu_threads: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearnProfile {
    pub v: u32,
    /// Human label only.
    pub name: String,
    /// 009's worker section; the executable is `foundry-learn`.
    pub worker: WorkerSpec,
    /// The pinned checkpoint (`model.safetensors`, `encoder/config.json`):
    /// the bundle's first read-only grant.
    pub checkpoint_dir: PathBuf,
    /// LibTorch's `lib` directory: the second read-only grant, and the
    /// worker's rpath.
    pub libtorch_dir: PathBuf,
    pub load_timeout_seconds: u64,
    pub ceilings: Ceilings,
}

/// What the development profile enforces (docs/deployment.md § Learning
/// worker enforcement):
/// * process count — hard: `RLIMIT_NPROC=0` set before exec;
/// * output — hard: `RLIMIT_FSIZE` = `output_bytes` per file, plus the
///   supervised total of the scratch run directory;
/// * CPU — hard: `RLIMIT_CPU` = `wall_seconds × cpu_threads` seconds, plus
///   LibTorch's thread count set to `cpu_threads`;
/// * memory — supervised only: physical-footprint polling every 250 ms that
///   fails closed (macOS has no enforced hard memory limit for this process).
pub const PROVIDED: Enforcement = Enforcement {
    memory: Level::Supervised,
    cpu: Level::Hard,
    output: Level::Hard,
    process_count: Level::Hard,
};

/// Refuse any requested level above what the profile provides.
pub fn check_enforcement(requested: &Enforcement) -> FResult<()> {
    for (resource, asked, provided) in [
        ("memory", requested.memory, PROVIDED.memory),
        ("cpu", requested.cpu, PROVIDED.cpu),
        ("output", requested.output, PROVIDED.output),
        (
            "process_count",
            requested.process_count,
            PROVIDED.process_count,
        ),
    ] {
        if asked > provided {
            return Err(fail(
                "enforcement_unavailable",
                format!(
                    "the policy requests {asked:?} {resource} enforcement; this profile provides \
                     only {provided:?}, and enforcement is never downgraded"
                ),
            ));
        }
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> crate::FoundryError {
    fail("profile_invalid", message)
}

impl LearnProfile {
    /// Read at most 64 KiB of a regular file (not a symlink), parse
    /// strictly, validate structure. No checkpoint or LibTorch file is read.
    pub fn load(path: &Path) -> FResult<Self> {
        let file = super::open_regular(path)
            .map_err(|e| invalid(format!("profile {}: {e}", path.display())))?;
        let raw = read_capped(file, MAX_PROFILE_BYTES)
            .map_err(|e| invalid(format!("profile {}: {e}", path.display())))?
            .ok_or_else(|| invalid(format!("profile exceeds {MAX_PROFILE_BYTES} bytes")))?;
        let profile: Self = strict_json(&raw, "profile_invalid", "learning profile")?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> FResult<()> {
        if self.v != PROFILE_VERSION {
            return Err(invalid(format!(
                "profile version {} is not {PROFILE_VERSION}",
                self.v
            )));
        }
        for (what, path) in [
            ("worker.bundle", &self.worker.bundle),
            ("worker.scratch_root", &self.worker.scratch_root),
            ("checkpoint_dir", &self.checkpoint_dir),
            ("libtorch_dir", &self.libtorch_dir),
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
        if !super::hex64(&self.worker.executable_sha256) {
            return Err(invalid("worker.executable_sha256 must be a SHA-256"));
        }
        if self.load_timeout_seconds == 0 || self.load_timeout_seconds > MAX_LOAD_TIMEOUT_SECONDS {
            return Err(invalid(format!(
                "load_timeout_seconds must be 1..={MAX_LOAD_TIMEOUT_SECONDS}"
            )));
        }
        self.check_scratch_disjoint()
            .map_err(|e| invalid(e.to_string()))
    }

    /// The scratch root is the worker's only write grant. It must not equal,
    /// contain or lie inside the checkpoint or LibTorch grant, aliases
    /// resolved (009's rule and resolver).
    pub fn check_scratch_disjoint(&self) -> Result<(), ProviderError> {
        let scratch = resolve(&self.worker.scratch_root)?;
        for what in [&self.checkpoint_dir, &self.libtorch_dir] {
            let other = resolve(what)?;
            if scratch == other || overlaps(&scratch, &other) || overlaps(&other, &scratch) {
                return Err(ProviderError::ProfileInvalid(format!(
                    "worker.scratch_root ({}) overlaps {} ({}) after resolving symlinks",
                    scratch.display(),
                    what.display(),
                    other.display()
                )));
            }
        }
        Ok(())
    }

    pub fn executable(&self) -> PathBuf {
        self.worker.bundle.join("Contents/MacOS").join(EXECUTABLE)
    }

    /// The policy's requests must fit this profile: every ceiling at most
    /// the profile's, and the enforcement matrix.
    pub fn admit(&self, policy: &LearningPolicy) -> FResult<()> {
        let c = &self.ceilings;
        for (name, asked, most) in [
            ("memory_bytes", policy.memory_bytes, c.memory_bytes),
            ("wall_seconds", policy.wall_seconds, c.wall_seconds),
            ("output_bytes", policy.output_bytes, c.output_bytes),
            (
                "cpu_threads",
                u64::from(policy.cpu_threads),
                u64::from(c.cpu_threads),
            ),
        ] {
            if asked > most {
                return Err(fail(
                    "enforcement_unavailable",
                    format!(
                        "the policy requests {name} {asked}; this profile admits at most {most}"
                    ),
                ));
            }
        }
        check_enforcement(&policy.enforcement)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_above_the_provided_level_is_refused_never_downgraded() {
        let all_supervised = Enforcement {
            memory: Level::Supervised,
            cpu: Level::Supervised,
            output: Level::Supervised,
            process_count: Level::Supervised,
        };
        check_enforcement(&all_supervised).unwrap();
        check_enforcement(&PROVIDED).unwrap();
        let hard_memory = Enforcement {
            memory: Level::Hard,
            ..PROVIDED
        };
        assert_eq!(
            check_enforcement(&hard_memory).unwrap_err().code(),
            "enforcement_unavailable"
        );
    }
}
