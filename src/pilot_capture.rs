//! Bounded pilot v1 live-capture boundary (opt-in `pilot-capture` feature).
//!
//! Normative policy: `specs/003-agent-retrieval-context/contracts/bounded-pilot-v1.md`
//! (SHA-256 [`policy::CONTRACT_SHA256`], recorded outside its body). This
//! module implements the approved Rust-owned capture session: Rust owns
//! admission, credentials, provider HTTPS, counting, reservations, usage
//! observation, deadlines, the durable journal, host lifecycle and seals. The
//! native OMP bridge (`pilot_capture/bridge.ts`) only adapts the host's public
//! extension API and relays fetches over private inherited pipes.
//!
//! Nothing here authorizes execution. Live admission validates every frozen
//! gate and refuses with a named missing-evidence code; verification mode
//! uses owned synthetic credentials and an owned TLS fixture and cannot be
//! turned into live authority by changing a flag or URL.

pub mod apparatus;
pub mod host;
pub mod protocol;
pub mod wire;

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use protocol::{CloseReason, FaultClass, ParentMessage};

/// Frozen policy values from bounded pilot v1. Changing any of them is a
/// contract revision, not a tuning step.
pub mod policy {
    /// SHA-256 of the canonical bounded-pilot-v1 contract bytes.
    pub const CONTRACT_SHA256: &str =
        "1a00758ffdafbfa18a59dd69e0b9912ef8beca9f7010f375bab8e553c554502d";
    /// Pinned OMP host.
    pub const OMP_VERSION: &str = "18.13.2";
    pub const OMP_SHA256: &str = "38c3645283ceba06f66462830ae15f27270cc7f84575e40a4c6bd68452a63d0f";

    pub const PROVIDER: &str = "anthropic";
    pub const MODEL: &str = "claude-sonnet-5-5";
    /// Custom API identifier the bridge registers for the cloned model.
    pub const BRIDGE_API: &str = "cf-pilot-capture-v1";
    pub const THINKING_LEVEL: &str = "xhigh";
    pub const EFFORT: &str = "xhigh";
    pub const SERVICE_TIER: &str = "standard_only";
    /// Tools the final request must advertise, in host order.
    pub const TOOL_NAMES: [&str; 2] = ["read", "edit"];

    pub const ORIGIN_HOST: &str = "api.anthropic.com";
    pub const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
    pub const COUNT_URL: &str = "https://api.anthropic.com/v1/messages/count_tokens";
    pub const ANTHROPIC_VERSION: &str = "2023-06-01";
    /// Beta tokens the pinned host may send for this configuration.
    pub const ALLOWED_BETAS: &[&str] = &[
        "effort-2025-11-24",
        "mid-conversation-system-clear-at-2026-08-21",
        "mid-conversation-tool-changes-2026-07-01",
        "mid-conversation-output-config-2026-07-01",
        "fallback-credit-2026-06-01",
        "thinking-binding-controls-2026-08-01",
        "context-management-2025-06-27",
    ];
    /// Beta tokens the frozen settings require.
    pub const REQUIRED_BETAS: &[&str] = &[
        "effort-2025-11-24",
        "thinking-binding-controls-2026-08-01",
        "context-management-2025-06-27",
    ];
    /// The only credential the network-denied bridge may present. Rust
    /// verifies it and substitutes the admitted transport credential.
    pub const BRIDGE_SYNTHETIC_KEY: &str = "sk-ant-api03-cf-pilot-bridge-synthetic-not-a-key";

    /// Request and observed-SSE bounds.
    pub const MAX_BODY_BYTES: usize = 1024 * 1024;
    pub const MAX_SSE_BYTES: usize = 1024 * 1024;

    /// Token bounds.
    pub const INPUT_ADMISSION_THRESHOLD: u64 = 30_720;
    pub const INPUT_RESERVATION: u64 = 32_768;
    pub const MAX_OUTPUT_TOKENS: u64 = 4096;

    /// Frozen budget-model rates in microUSD per token (USD10/M, USD25/M).
    pub const INPUT_MICRO_USD_PER_TOKEN: u64 = 10;
    pub const OUTPUT_MICRO_USD_PER_TOKEN: u64 = 25;
    /// `(32768 × 10) + (4096 × 25)`.
    pub const FULL_RESERVATION_MICRO_USD: u64 = 430_080;
    pub const CODING_SLOT_ENVELOPE_MICRO_USD: u64 = 1_000_000;
    pub const PROBE_ENVELOPE_MICRO_USD: u64 = 500_000;
    pub const PILOT_ENVELOPE_MICRO_USD: u64 = 10_000_000;

    /// Request caps.
    pub const PERMITS_PER_CODING_SLOT: u32 = 6;
    pub const PERMITS_PER_PROBE: u32 = 1;
    pub const MAX_GENERATION_PERMITS: u32 = 49;
    pub const MAX_COUNT_CALLS: u32 = 49;
    pub const MAX_CATALOG_GETS: u32 = 9;
    pub const MAX_PROVIDER_HTTP_ATTEMPTS: u32 = 107;

    /// Time bounds in seconds.
    pub const CODING_SLOT_WALL_SECS: u64 = 600;
    pub const GENERATION_DEADLINE_SECS: u64 = 120;
    pub const POST_CHECK_SECS: u64 = 60;
    pub const PAID_PHASE_WALL_SECS: u64 = 7200;

    /// Frozen slot order (allocation draw 5: BA, BA, AB, AB).
    pub const SLOT_ORDER: [&str; 8] = ["1B", "1A", "2B", "2A", "3A", "3B", "4A", "4B"];
    /// The fixed-first optional transport probe.
    pub const PROBE_SLOT: &str = "probe";
    pub const PROBE_INSTRUCTION: &str = "Reply exactly PILOT_TRANSPORT_OK. Do not use tools, read files, edit files, or solve a coding task.";

    /// Canonical read set (the five-file corpus) and edit set, relative to
    /// the owned workspace.
    pub const READ_SET: [&str; 5] = [
        "Cargo.toml",
        "src/lib.rs",
        "src/ascii_set.rs",
        "LICENSE-MIT",
        "LICENSE-APACHE",
    ];
    pub const EDIT_SET: [&str; 1] = ["src/lib.rs"];

    /// Frozen independent-check pins (contract §2, §9).
    pub const ORACLE_SHA256: &str =
        "f5ac14d370d852090c49ead61e23f4d9185eed491b33b123ba6e6ff2dec09bb9";
    pub const RUSTC: &str = "/opt/homebrew/bin/rustc";
    pub const RUSTC_SHA256: &str =
        "0239590431f4f2dd0e8499f6502a08ebdb2848455efc2951620398dccb685bbf";
    pub const RUSTC_VERSION_PREFIX: &str = "rustc 1.97.1";
    /// The accepted command recipe (apparatus run-mvthtmxg-vpxs7
    /// case-ctrl-oracle/positive/steps.json); `<ROOT>` is the private
    /// checker root, the only substitution allowed.
    pub const CHECK_RECIPE: [(&str, &[&str]); 4] = [
        (
            "noalloc_compile",
            &[
                RUSTC,
                "--edition",
                "2018",
                "--crate-type",
                "lib",
                "--crate-name",
                "records",
                "src/lib.rs",
                "-o",
                "out/librecords-noalloc.rlib",
            ],
        ),
        (
            "alloc_compile",
            &[
                RUSTC,
                "--edition",
                "2018",
                "--crate-type",
                "lib",
                "--crate-name",
                "records",
                "--cfg",
                "feature=\"alloc\"",
                "src/lib.rs",
                "-o",
                "out/librecords-alloc.rlib",
            ],
        ),
        (
            "oracle_compile",
            &[
                RUSTC,
                "--edition",
                "2021",
                "--crate-type",
                "bin",
                "check.rs",
                "--extern",
                "records=out/librecords-alloc.rlib",
                "-o",
                "out/check",
            ],
        ),
        ("oracle_run", &["<ROOT>/out/check"]),
    ];

    /// Full reservation in microUSD, derived (checked at compile time).
    pub const fn full_reservation() -> u64 {
        INPUT_RESERVATION * INPUT_MICRO_USD_PER_TOKEN
            + MAX_OUTPUT_TOKENS * OUTPUT_MICRO_USD_PER_TOKEN
    }
    const _: () = assert!(full_reservation() == FULL_RESERVATION_MICRO_USD);
}

/// A closed capture failure: a fault class plus a static diagnostic code.
/// Never carries upstream bodies, headers, credentials or prompt material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CaptureFault {
    pub class: FaultClass,
    pub code: &'static str,
}

impl CaptureFault {
    pub const fn new(class: FaultClass, code: &'static str) -> Self {
        Self { class, code }
    }
}

impl std::fmt::Display for CaptureFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.class.as_str(), self.code)
    }
}

impl std::error::Error for CaptureFault {}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(b & 0x0f), 16).unwrap_or('0'));
    }
    out
}

// ===========================================================================
// Private files, clocks and the durable journal.
// ===========================================================================

fn persistence(code: &'static str) -> CaptureFault {
    CaptureFault::new(FaultClass::Persistence, code)
}

fn fsync_dir(path: &Path) -> Result<(), CaptureFault> {
    File::open(path)
        .and_then(|d| d.sync_all())
        .map_err(|_| persistence("dir_fsync_failed"))
}

/// Create one private (0700) directory; it must not exist. The parent is
/// fsynced so the directory survives a crash.
pub(crate) fn create_private_dir(path: &Path) -> Result<(), CaptureFault> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .map_err(|_| persistence("private_dir_create_failed"))?;
    let meta = fs::symlink_metadata(path).map_err(|_| persistence("private_dir_create_failed"))?;
    if !meta.is_dir() || meta.mode() & 0o077 != 0 || meta.uid() != current_uid() {
        return Err(persistence("private_dir_not_private"));
    }
    if let Some(parent) = path.parent() {
        fsync_dir(parent)?;
    }
    Ok(())
}

/// Create one private (0600) file exclusively without following symlinks,
/// write it and fsync it.
pub(crate) fn write_private_new(path: &Path, bytes: &[u8]) -> Result<(), CaptureFault> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| persistence("private_file_create_failed"))?;
    f.write_all(bytes)
        .and_then(|()| f.sync_all())
        .map_err(|_| persistence("private_file_write_failed"))?;
    Ok(())
}

fn current_uid() -> u32 {
    // SAFETY: getuid has no failure mode.
    unsafe { libc::getuid() }
}

/// The owner's real home from the account database (not `$HOME`).
fn real_home() -> Result<PathBuf, CaptureFault> {
    // SAFETY: getpwuid_r with caller-owned buffers.
    unsafe {
        let mut pwd: libc::passwd = std::mem::zeroed();
        let mut buf = vec![0 as libc::c_char; 16 * 1024];
        let mut out: *mut libc::passwd = std::ptr::null_mut();
        let rc = libc::getpwuid_r(
            current_uid(),
            &mut pwd,
            buf.as_mut_ptr(),
            buf.len(),
            &mut out,
        );
        if rc != 0 || out.is_null() || pwd.pw_dir.is_null() {
            return Err(CaptureFault::new(
                FaultClass::Bootstrap,
                "real_home_unavailable",
            ));
        }
        let dir = std::ffi::CStr::from_ptr(pwd.pw_dir)
            .to_string_lossy()
            .into_owned();
        fs::canonicalize(dir)
            .map_err(|_| CaptureFault::new(FaultClass::Bootstrap, "real_home_unavailable"))
    }
}

/// The owner's real home, for the independent checker's sandbox.
pub(crate) fn real_home_for_checks() -> Result<PathBuf, CaptureFault> {
    real_home()
}

/// Nanoseconds on the platform-continuous clock (advances during sleep).
pub fn continuous_nanos() -> Result<u64, CaptureFault> {
    #[cfg(target_os = "macos")]
    let clock = libc::CLOCK_MONOTONIC;
    #[cfg(not(target_os = "macos"))]
    let clock = libc::CLOCK_BOOTTIME;
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime into a caller-owned timespec.
    let rc = unsafe { libc::clock_gettime(clock, &mut ts) };
    if rc != 0 || ts.tv_sec < 0 || ts.tv_nsec < 0 {
        return Err(CaptureFault::new(
            FaultClass::Deadline,
            "continuous_clock_unavailable",
        ));
    }
    (ts.tv_sec as u64)
        .checked_mul(1_000_000_000)
        .and_then(|n| n.checked_add(ts.tv_nsec as u64))
        .ok_or(CaptureFault::new(
            FaultClass::Deadline,
            "continuous_clock_overflow",
        ))
}

/// An identity of the current boot; a reboot changes it.
pub fn boot_identity() -> Result<String, CaptureFault> {
    let unavailable = CaptureFault::new(FaultClass::Deadline, "boot_identity_unavailable");
    #[cfg(target_os = "macos")]
    {
        let name = std::ffi::CString::new("kern.bootsessionuuid").map_err(|_| unavailable)?;
        let mut buf = [0u8; 128];
        let mut len = buf.len();
        // SAFETY: sysctlbyname into a caller-owned buffer of `len` bytes.
        let rc = unsafe {
            libc::sysctlbyname(
                name.as_ptr(),
                buf.as_mut_ptr().cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc != 0 || len == 0 || len > buf.len() {
            return Err(unavailable);
        }
        let text = String::from_utf8_lossy(&buf[..len])
            .trim_end_matches('\0')
            .trim()
            .to_string();
        if text.is_empty() {
            return Err(unavailable);
        }
        Ok(text)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let text =
            fs::read_to_string("/proc/sys/kernel/random/boot_id").map_err(|_| unavailable)?;
        let text = text.trim().to_string();
        if text.is_empty() {
            return Err(unavailable);
        }
        Ok(text)
    }
}

fn display_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

const MAX_JOURNAL_RECORD_BYTES: usize = 64 * 1024;

/// Private, bounded, append-and-fsync evidence journal. Every record names
/// the SHA-256 of the previous record, so truncation or edits are evident.
/// No replay engine or resume path reads it back except the clean
/// probe-to-coding handoff, which only verifies its digest.
struct Journal {
    file: File,
    path: PathBuf,
    prev: String,
    records: u64,
    fail_after: Option<u64>,
}

impl Journal {
    fn create(path: &Path, fail_after: Option<u64>) -> Result<Self, CaptureFault> {
        let file = OpenOptions::new()
            .append(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| persistence("journal_create_failed"))?;
        if let Some(parent) = path.parent() {
            fsync_dir(parent)?;
        }
        Ok(Self {
            file,
            path: path.to_path_buf(),
            prev: String::new(),
            records: 0,
            fail_after,
        })
    }

    /// Reopen for the probe-to-coding handoff after verifying the digest
    /// sealed by the probe.
    fn reopen(path: &Path, expected_sha256: &str, records: u64) -> Result<Self, CaptureFault> {
        let meta = fs::symlink_metadata(path).map_err(|_| persistence("journal_missing"))?;
        if !meta.is_file() || meta.mode() & 0o077 != 0 || meta.uid() != current_uid() {
            return Err(persistence("journal_not_private"));
        }
        let bytes = fs::read(path).map_err(|_| persistence("journal_unreadable"))?;
        if sha256_hex(&bytes) != expected_sha256 {
            return Err(CaptureFault::new(
                FaultClass::Authority,
                "journal_digest_mismatch",
            ));
        }
        let lines: Vec<&[u8]> = bytes
            .split(|&b| b == b'\n')
            .filter(|l| !l.is_empty())
            .collect();
        if lines.len() as u64 != records {
            return Err(CaptureFault::new(
                FaultClass::Authority,
                "journal_record_count_mismatch",
            ));
        }
        let prev = lines.last().map(|l| sha256_hex(l)).unwrap_or_default();
        let file = OpenOptions::new()
            .append(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| persistence("journal_reopen_failed"))?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
            prev,
            records,
            fail_after: None,
        })
    }

    fn append(&mut self, kind: &str, fields: Value) -> Result<(), CaptureFault> {
        if let Some(limit) = self.fail_after
            && self.records >= limit
        {
            return Err(persistence("journal_append_failed"));
        }
        let mut record = match fields {
            Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        // Reserved keys are written last so no caller field can replace them.
        record.insert("seq".into(), json!(self.records + 1));
        record.insert("kind".into(), json!(kind));
        record.insert("prev".into(), json!(self.prev));
        record.insert("at_unix_ms".into(), json!(display_unix_ms()));
        let mut line = serde_json::to_vec(&Value::Object(record))
            .map_err(|_| persistence("journal_encode_failed"))?;
        if line.len() > MAX_JOURNAL_RECORD_BYTES {
            return Err(persistence("journal_record_too_large"));
        }
        let digest = sha256_hex(&line);
        line.push(b'\n');
        self.file
            .write_all(&line)
            .and_then(|()| self.file.sync_data())
            .map_err(|_| persistence("journal_append_failed"))?;
        self.prev = digest;
        self.records += 1;
        Ok(())
    }

    /// Credential admission already consumed the run/authority claim. Persist
    /// its terminal refusal before returning; a failed append is itself the
    /// reported failure, never a claim that the refusal was recorded.
    fn credential_refusal(&mut self, fault: CaptureFault) -> CaptureFault {
        match self.append(
            "halt",
            json!({"phase": "credential_admission", "class": fault.class.as_str(), "code": fault.code}),
        ) {
            Ok(()) => fault,
            Err(journal_fault) => journal_fault,
        }
    }

    fn digest(&self) -> Result<String, CaptureFault> {
        let bytes = fs::read(&self.path).map_err(|_| persistence("journal_unreadable"))?;
        Ok(sha256_hex(&bytes))
    }
}

// ===========================================================================
// Admission.
// ===========================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Verification,
    Live,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Probe,
    Coding,
}

/// Fault injection honoured only by verification sessions.
#[derive(Clone, Copy, Debug, Default)]
pub struct VerificationFaults {
    /// Fail the journal append after this many records.
    pub journal_fail_after: Option<u64>,
}

/// Verification: the real pinned host and bridge, an owned numeric-loopback
/// TLS fixture, an owned CA and an owned synthetic credential. It is a
/// different type from live admission and cannot become live authority.
pub struct VerificationAdmission {
    pub run_dir: PathBuf,
    pub omp: PathBuf,
    pub scope: Scope,
    pub fixture_addr: SocketAddr,
    pub fixture_ca_pem: Vec<u8>,
    /// Owned synthetic credential; must start with `sk-ant-verify-`.
    pub credential: String,
    pub faults: VerificationFaults,
    /// An optional frozen launch manifest, so verification exercises the
    /// same slot, prompt, source and render bindings as live.
    pub manifest: Option<LaunchManifest>,
}

/// Live: every frozen gate is bound by evidence files; any missing or
/// mismatching gate is a named refusal before credentials or network.
pub struct LiveAdmission {
    pub run_dir: PathBuf,
    pub omp: PathBuf,
    pub scope: Scope,
    /// The bounded-pilot-v1 contract bytes (digest checked).
    pub contract: PathBuf,
    /// Explicit owner authorization receipt (gate 5).
    pub authority: PathBuf,
    /// Frozen launch manifest (gate 4).
    pub launch_manifest: PathBuf,
    /// Standard-service account/model ratecard evidence (gate 3).
    pub ratecard: PathBuf,
    /// Accepted compatibility evidence: a probe seal (gate 2; coding only).
    pub compatibility: Option<PathBuf>,
    /// The transport credential (0600, owned, outside every run path).
    pub credential_file: PathBuf,
}

pub enum LaunchAdmission {
    Verification(VerificationAdmission),
    Live(LiveAdmission),
}

/// Preview rendering: no credential, no network, no provider request.
pub struct PreviewAdmission {
    pub run_dir: PathBuf,
    pub omp: PathBuf,
}

/// Owner authorization receipt (gate 5).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityReceipt {
    pub kind: String,
    pub contract_sha256: String,
    pub implementation_sha256: String,
    pub launch_manifest_sha256: String,
    pub ratecard_sha256: String,
    pub scope: Scope,
    pub authorized_by: String,
    pub authorized_at: String,
    /// The owner approved task-data rights and the account's provider
    /// retention/training terms for this run (gate 3). Never implied.
    pub rights_approved: bool,
}

/// One frozen slot of the launch manifest.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManifestSlot {
    pub slot_id: String,
    pub arm: String,
    pub workspace: PathBuf,
    pub prompt_sha256: String,
    pub sources: Vec<(String, String)>,
    /// The actual host's rendered request commitments for this slot; the
    /// slot's first live request must match them exactly.
    pub render: RenderCommitments,
}

/// Stable request commitments rendered before any paid request (the
/// volatile reminder and its date are excluded).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RenderCommitments {
    pub system_sha256: String,
    pub tools_sha256: String,
    pub settings_sha256: String,
    pub stable_prefix_sha256: String,
    pub beta: String,
    pub cache_layout: Vec<String>,
    pub initial_user_sha256: String,
}

/// One verified launch pin: a named file and its SHA-256.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PinEntry {
    pub name: String,
    pub path: PathBuf,
    pub sha256: String,
}

/// One frozen pair view bound by the manifest.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManifestView {
    pub pair: u8,
    pub path: PathBuf,
    pub sha256: String,
    pub delivered_o200k_tokens: u64,
    pub store_revision: String,
    pub citations_sha256: String,
    pub recipe_id: String,
}

/// Frozen launch manifest (gate 4).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchManifest {
    pub kind: String,
    pub contract_sha256: String,
    pub omp_sha256: String,
    pub slots: Vec<ManifestSlot>,
    /// Executables, profile, worker, model, oracle, task, prompts, seed and
    /// allocation, each re-verified at admission.
    pub pins: Vec<PinEntry>,
    pub allocation_sha256: String,
    pub views: Vec<ManifestView>,
    /// The private pilot root: the host may read and write nothing under it
    /// except its own slot workspace (other arms, stores, views, checker
    /// roots and the oracle copy stay out of reach wherever the root is).
    pub isolation_root: PathBuf,
}

/// Standard-service ratecard evidence (gate 3). Rates are decimal USD per
/// million tokens as strings.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ratecard {
    pub kind: String,
    pub model: String,
    pub service: String,
    pub input_usd_per_mtok: String,
    pub cache_write_usd_per_mtok: String,
    pub cache_read_usd_per_mtok: String,
    pub output_usd_per_mtok: String,
    pub account_identity_sha256: String,
    pub evidence_sha256: String,
}

fn authority(code: &'static str) -> CaptureFault {
    CaptureFault::new(FaultClass::Authority, code)
}

fn pins(code: &'static str) -> CaptureFault {
    CaptureFault::new(FaultClass::Pins, code)
}

/// Read a bounded regular file (no symlink) for evidence.
fn read_evidence(path: &Path, missing: &'static str, max: u64) -> Result<Vec<u8>, CaptureFault> {
    let meta = fs::symlink_metadata(path).map_err(|_| authority(missing))?;
    if !meta.is_file() || meta.len() > max {
        return Err(authority("evidence_shape_invalid"));
    }
    fs::read(path).map_err(|_| authority(missing))
}

fn file_sha256(path: &Path, missing: &'static str) -> Result<String, CaptureFault> {
    let meta = fs::metadata(path).map_err(|_| pins(missing))?;
    if !meta.is_file() {
        return Err(pins(missing));
    }
    let mut f = File::open(path).map_err(|_| pins(missing))?;
    let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|_| pins(missing))?;
        if n == 0 {
            break;
        }
        sha2::Digest::update(&mut hasher, &buf[..n]);
    }
    Ok(sha2::Digest::finalize(hasher)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Parse a decimal USD-per-million rate into microUSD per million tokens
/// (exact, no floating point).
fn rate_micro(text: &str) -> Option<u64> {
    let (int, frac) = text.split_once('.').unwrap_or((text, ""));
    if int.is_empty()
        || int.len() > 6
        || frac.len() > 6
        || !int.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let mut micro: u64 = int.parse::<u64>().ok()?.checked_mul(1_000_000)?;
    let mut scale = 100_000u64;
    for b in frac.bytes() {
        micro = micro.checked_add(u64::from(b - b'0') * scale)?;
        scale /= 10;
    }
    Some(micro)
}

/// Validate every live gate in order; returns the parsed manifest, the
/// authority receipt digest and any cited probe seal. Nothing secret is
/// read here: the credential is read only after the authority and run
/// claims succeed.
fn validate_live(
    a: &LiveAdmission,
) -> Result<(LaunchManifest, String, Option<ProbeSeal>), CaptureFault> {
    let contract = read_evidence(&a.contract, "contract_missing", 1 << 20)?;
    if sha256_hex(&contract) != policy::CONTRACT_SHA256 {
        return Err(authority("contract_digest_mismatch"));
    }
    if file_sha256(&a.omp, "omp_missing")? != policy::OMP_SHA256 {
        return Err(pins("omp_pin_mismatch"));
    }
    let authority_bytes = read_evidence(&a.authority, "authority_receipt_missing", 64 * 1024)?;
    let receipt: AuthorityReceipt = serde_json::from_slice(&authority_bytes)
        .map_err(|_| authority("authority_receipt_invalid"))?;
    if receipt.kind != "bounded-pilot-v1-authority"
        || receipt.authorized_by.trim().is_empty()
        || receipt.authorized_at.trim().is_empty()
    {
        return Err(authority("authority_receipt_invalid"));
    }
    if !receipt.rights_approved {
        return Err(authority("rights_not_approved"));
    }
    if receipt.contract_sha256 != policy::CONTRACT_SHA256 {
        return Err(authority("authority_contract_mismatch"));
    }
    if receipt.scope != a.scope {
        return Err(authority("authority_scope_mismatch"));
    }
    let exe = std::env::current_exe().map_err(|_| authority("implementation_unidentified"))?;
    if file_sha256(&exe, "implementation_unidentified")? != receipt.implementation_sha256 {
        return Err(authority("implementation_digest_mismatch"));
    }
    let manifest_bytes = read_evidence(&a.launch_manifest, "launch_manifest_missing", 1 << 20)?;
    if sha256_hex(&manifest_bytes) != receipt.launch_manifest_sha256 {
        return Err(authority("launch_manifest_digest_mismatch"));
    }
    let manifest: LaunchManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| authority("launch_manifest_invalid"))?;
    if manifest.kind != "bounded-pilot-v1-launch"
        || manifest.contract_sha256 != policy::CONTRACT_SHA256
        || manifest.omp_sha256 != policy::OMP_SHA256
    {
        return Err(authority("launch_manifest_invalid"));
    }
    let expected: Vec<&str> = match a.scope {
        Scope::Probe => vec![policy::PROBE_SLOT],
        Scope::Coding => policy::SLOT_ORDER.to_vec(),
    };
    if manifest
        .slots
        .iter()
        .map(|s| s.slot_id.as_str())
        .collect::<Vec<_>>()
        != expected
    {
        return Err(authority("launch_manifest_slot_order"));
    }
    verify_manifest_pins(&manifest)?;
    let ratecard_bytes = read_evidence(&a.ratecard, "ratecard_missing", 64 * 1024)?;
    if sha256_hex(&ratecard_bytes) != receipt.ratecard_sha256 {
        return Err(authority("ratecard_digest_mismatch"));
    }
    let card: Ratecard =
        serde_json::from_slice(&ratecard_bytes).map_err(|_| authority("ratecard_invalid"))?;
    if card.kind != "bounded-pilot-v1-ratecard"
        || card.model != policy::MODEL
        || card.service != "standard"
    {
        return Err(authority("ratecard_invalid"));
    }
    // The account the run bills and the pricing evidence are pinned by
    // digest (gate 3); an unbound draft cannot admit a run.
    if !protocol::is_sha256_hex(&card.account_identity_sha256)
        || !protocol::is_sha256_hex(&card.evidence_sha256)
    {
        return Err(authority("ratecard_account_unbound"));
    }
    let input_cap = policy::INPUT_MICRO_USD_PER_TOKEN * 1_000_000;
    let output_cap = policy::OUTPUT_MICRO_USD_PER_TOKEN * 1_000_000;
    for (rate, cap) in [
        (&card.input_usd_per_mtok, input_cap),
        (&card.cache_write_usd_per_mtok, input_cap),
        (&card.cache_read_usd_per_mtok, input_cap),
        (&card.output_usd_per_mtok, output_cap),
    ] {
        match rate_micro(rate) {
            Some(r) if r <= cap => {}
            Some(_) => return Err(authority("ratecard_exceeds_budget_rates")),
            None => return Err(authority("ratecard_invalid")),
        }
    }
    let seal = match (a.scope, &a.compatibility) {
        (Scope::Probe, _) => None,
        (Scope::Coding, None) => return Err(authority("compatibility_evidence_missing")),
        (Scope::Coding, Some(path)) => {
            let bytes = read_evidence(path, "compatibility_evidence_missing", 1 << 20)?;
            let seal: ProbeSeal = serde_json::from_slice(&bytes)
                .map_err(|_| authority("compatibility_evidence_invalid"))?;
            if seal.kind != "bounded-pilot-v1-probe-seal"
                || seal.mode != Mode::Live
                || seal.contract_sha256 != policy::CONTRACT_SHA256
                || seal.omp_sha256 != policy::OMP_SHA256
            {
                return Err(authority("compatibility_evidence_invalid"));
            }
            Some(seal)
        }
    };
    Ok((manifest, sha256_hex(&authority_bytes), seal))
}

/// A verification manifest gets the same structural and pin checks as live.
fn check_verification_manifest(
    manifest: Option<LaunchManifest>,
    scope: Scope,
) -> Result<Option<LaunchManifest>, CaptureFault> {
    let Some(m) = manifest else { return Ok(None) };
    let expected: Vec<&str> = match scope {
        Scope::Probe => vec![policy::PROBE_SLOT],
        Scope::Coding => policy::SLOT_ORDER.to_vec(),
    };
    if m.kind != "bounded-pilot-v1-launch"
        || m.contract_sha256 != policy::CONTRACT_SHA256
        || m.omp_sha256 != policy::OMP_SHA256
        || m.slots
            .iter()
            .map(|s| s.slot_id.as_str())
            .collect::<Vec<_>>()
            != expected
    {
        return Err(authority("launch_manifest_invalid"));
    }
    verify_manifest_pins(&m)?;
    Ok(Some(m))
}

/// Re-verify every file a launch manifest pins (resolved launch-pin checks)
/// and every frozen view it binds.
fn verify_manifest_pins(manifest: &LaunchManifest) -> Result<(), CaptureFault> {
    if manifest.pins.is_empty() || manifest.views.len() != apparatus::pins::PAIRS.len() {
        return Err(authority("launch_manifest_unpinned"));
    }
    for pin in &manifest.pins {
        if file_sha256(&pin.path, "launch_pin_missing")? != pin.sha256 {
            return Err(pins("launch_pin_changed"));
        }
    }
    for view in &manifest.views {
        if file_sha256(&view.path, "launch_view_missing")? != view.sha256 {
            return Err(pins("launch_view_changed"));
        }
    }
    Ok(())
}

/// Claim an owner authorization receipt exclusively: one receipt admits one
/// run. The claim marker sits beside the receipt and is never removed.
fn claim_authority(
    receipt: &Path,
    receipt_sha256: &str,
    run_dir: &Path,
) -> Result<(), CaptureFault> {
    let mut marker = receipt.as_os_str().to_owned();
    marker.push(".claim");
    let body = serde_json::to_vec(&json!({
        "receipt_sha256": receipt_sha256,
        "run_dir": run_dir,
        "claimed_at_unix_ms": display_unix_ms(),
    }))
    .map_err(|_| persistence("claim_encode_failed"))?;
    match write_private_new(Path::new(&marker), &body) {
        Ok(()) => Ok(()),
        Err(_) if Path::new(&marker).symlink_metadata().is_ok() => {
            Err(authority("authority_already_claimed"))
        }
        Err(f) => Err(f),
    }
}

/// The canonical form a not-yet-created run directory will have.
fn canonical_run_dir(run_dir: &Path) -> Result<PathBuf, CaptureFault> {
    let parent = run_dir.parent().ok_or(authority("run_dir_invalid"))?;
    let parent = fs::canonicalize(parent).map_err(|_| authority("run_dir_parent_missing"))?;
    Ok(parent.join(run_dir.file_name().ok_or(authority("run_dir_invalid"))?))
}

/// The handoff trusts nothing in a caller-supplied seal that its own
/// journal does not state: the journal's terminal record is the seal, with
/// the same counters, identity sequences, paid clock, boot and run; the run
/// was admitted for a probe in the same mode and never halted.
fn verify_seal_against_journal(seal: &ProbeSeal, journal: &Path) -> Result<(), CaptureFault> {
    let mismatch = || authority("probe_seal_not_journaled");
    let text = fs::read_to_string(journal).map_err(|_| persistence("journal_unreadable"))?;
    let records: Vec<Value> = text
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|_| mismatch())?;
    let kind = |r: &Value| r.get("kind").and_then(Value::as_str).map(str::to_string);
    let admitted = records.first().ok_or_else(mismatch)?;
    let last = records.last().ok_or_else(mismatch)?;
    let paid: Vec<&Value> = records
        .iter()
        .filter(|r| kind(r).as_deref() == Some("paid_phase_started"))
        .collect();
    let ok = kind(admitted).as_deref() == Some("admitted")
        && admitted.get("scope") == Some(&json!(Scope::Probe))
        && admitted.get("mode") == Some(&json!(seal.mode))
        && admitted.get("boot_identity") == Some(&json!(seal.boot_identity))
        && kind(last).as_deref() == Some("probe_sealed")
        && last.get("counters") == Some(&json!(seal.counters))
        && last.get("attempt_seq") == Some(&json!(seal.attempt_seq))
        && last.get("host_seq") == Some(&json!(seal.host_seq))
        && last.get("paid_started_ns") == Some(&json!(seal.paid_started_ns))
        && last.get("paid_deadline_ns") == Some(&json!(seal.paid_deadline_ns))
        && last.get("message_id") == Some(&json!(seal.probe_message_id))
        && last.get("run_dir") == Some(&json!(seal.run_dir))
        && paid.len() == 1
        && paid[0].get("started_ns") == Some(&json!(seal.paid_started_ns))
        && paid[0].get("deadline_ns") == Some(&json!(seal.paid_deadline_ns))
        && !records.iter().any(|r| kind(r).as_deref() == Some("halt"));
    if ok { Ok(()) } else { Err(mismatch()) }
}

/// Verification admission: the pinned host, an owned synthetic credential,
/// a numeric loopback fixture and exactly one owned CA. Shared by `open` and
/// the probe-to-coding handoff.
fn check_verification(v: &VerificationAdmission) -> Result<(), CaptureFault> {
    if file_sha256(&v.omp, "omp_missing")? != policy::OMP_SHA256 {
        return Err(pins("omp_pin_mismatch"));
    }
    if !v.credential.starts_with("sk-ant-verify-") {
        return Err(CaptureFault::new(
            FaultClass::Privacy,
            "verification_credential_not_owned",
        ));
    }
    if !v.fixture_addr.ip().is_loopback() {
        return Err(authority("verification_endpoint_not_loopback"));
    }
    Ok(())
}

fn read_credential(
    path: &Path,
    run_dir: &Path,
    manifest: &LaunchManifest,
) -> Result<wire::Credential, CaptureFault> {
    let privacy = |code| CaptureFault::new(FaultClass::Privacy, code);
    let meta = fs::symlink_metadata(path).map_err(|_| privacy("credential_missing"))?;
    if !meta.is_file()
        || meta.mode() & 0o077 != 0
        || meta.uid() != current_uid()
        || meta.len() > 512
    {
        return Err(privacy("credential_file_not_private"));
    }
    let canonical = fs::canonicalize(path).map_err(|_| privacy("credential_missing"))?;
    let run_dir = fs::canonicalize(run_dir).unwrap_or_else(|_| run_dir.to_path_buf());
    if canonical.starts_with(&run_dir)
        || manifest
            .slots
            .iter()
            .any(|s| canonical.starts_with(&s.workspace))
    {
        return Err(privacy("credential_inside_run_paths"));
    }
    let bytes = fs::read(&canonical).map_err(|_| privacy("credential_missing"))?;
    let text = std::str::from_utf8(&bytes).map_err(|_| privacy("credential_invalid"))?;
    wire::Credential::new(text.trim_end_matches(['\n', '\r']))
}

// ===========================================================================
// Ledger: permits, reservations and checked microUSD arithmetic.
// ===========================================================================

/// Session-wide counters and money (budget-model microUSD).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counters {
    pub spent_micro_usd: u64,
    pub outstanding_micro_usd: u64,
    pub generation_permits: u32,
    pub count_calls: u32,
    pub messages_attempts: u32,
    pub catalog_gets: u32,
    pub provider_http_attempts: u32,
}

#[derive(Clone, Copy, Debug)]
struct SlotLedger {
    envelope: u64,
    permit_cap: u32,
    spent: u64,
    outstanding: u64,
    permits: u32,
}

fn checked(a: u64, b: u64) -> Result<u64, CaptureFault> {
    a.checked_add(b)
        .ok_or(CaptureFault::new(FaultClass::Usage, "arithmetic_overflow"))
}

/// Budget-model cost of an observed generation, exact integers.
pub fn budget_cost_micro(inclusive_input: u64, output: u64) -> Option<u64> {
    inclusive_input
        .checked_mul(policy::INPUT_MICRO_USD_PER_TOKEN)?
        .checked_add(output.checked_mul(policy::OUTPUT_MICRO_USD_PER_TOKEN)?)
}

/// Whether a new full reservation fits both envelopes.
pub fn reservation_fits(spent: u64, outstanding: u64, envelope: u64) -> Option<bool> {
    let need = spent
        .checked_add(outstanding)?
        .checked_add(policy::FULL_RESERVATION_MICRO_USD)?;
    Some(need <= envelope)
}

// ===========================================================================
// Slot inputs and evidence types.
// ===========================================================================

/// The next frozen slot. Exposes no budget, order or fallback override.
#[derive(Clone, Debug)]
pub struct PreparedSlot {
    pub slot_id: String,
    /// Canonical owned workspace for this pair.
    pub workspace: PathBuf,
    /// Exact protected initial-user prompt text.
    pub initial_user_text: String,
    /// Expected workspace sources before launch: (relative path, sha256).
    pub expected_sources: Vec<(String, String)>,
}

/// Request commitments rendered by the actual host and bridge, with no
/// request contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderBinding {
    pub slot_id: String,
    pub body_sha256: String,
    pub body_len: usize,
    pub system_sha256: String,
    pub tools_sha256: String,
    pub settings_sha256: String,
    pub stable_prefix_sha256: String,
    pub initial_user_sha256: String,
    pub volatile_reminder_sha256: Option<String>,
    pub cache_layout: Vec<String>,
    pub beta: String,
    pub count_projection_sha256: String,
    pub bridge_sha256: String,
    pub omp_sha256: String,
}

impl RenderBinding {
    fn from_validated(slot_id: &str, v: &wire::ValidatedRequest) -> Self {
        Self {
            slot_id: slot_id.to_string(),
            body_sha256: v.body_sha256.clone(),
            body_len: v.body_len,
            system_sha256: v.system_sha256.clone(),
            tools_sha256: v.tools_sha256.clone(),
            settings_sha256: v.settings_sha256.clone(),
            stable_prefix_sha256: v.stable_prefix_sha256.clone(),
            initial_user_sha256: v.initial_user_sha256.clone(),
            volatile_reminder_sha256: v.volatile_reminder_sha256.clone(),
            cache_layout: v.cache_layout.clone(),
            beta: v.beta.clone(),
            count_projection_sha256: v.count_projection_sha256.clone(),
            bridge_sha256: sha256_hex(host::BRIDGE_SOURCE),
            omp_sha256: policy::OMP_SHA256.to_string(),
        }
    }

    /// Within a pair only the declared evidence slot (and its recorded
    /// volatile metadata) may differ.
    pub fn matches_pair(&self, other: &RenderBinding) -> Result<(), CaptureFault> {
        let same = self.system_sha256 == other.system_sha256
            && self.tools_sha256 == other.tools_sha256
            && self.settings_sha256 == other.settings_sha256
            && self.beta == other.beta
            && self.cache_layout == other.cache_layout
            && self.bridge_sha256 == other.bridge_sha256
            && self.omp_sha256 == other.omp_sha256;
        if same {
            Ok(())
        } else {
            Err(CaptureFault::new(
                FaultClass::RequestBinding,
                "pair_render_mismatch",
            ))
        }
    }
}

/// One generation permit's evidence.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationRecord {
    pub permit: u32,
    pub invocation: u32,
    pub fetch: u32,
    pub body_sha256: String,
    pub count_projection_sha256: String,
    pub followup: bool,
    pub count_attempt: Option<u32>,
    pub count_status: Option<u16>,
    pub count_request_id: Option<String>,
    pub estimate_lexeme: Option<String>,
    pub messages_attempt: Option<u32>,
    pub messages_status: Option<u16>,
    pub request_id: Option<String>,
    pub message_id: Option<String>,
    pub returned_model: Option<String>,
    pub served_service_tier: Option<String>,
    pub stop_reason: Option<String>,
    pub terminal: bool,
    pub usage: Option<UsageRecord>,
    pub tool_uses: Vec<String>,
    pub cost_micro_usd: Option<u64>,
    /// The native consumer cancelled the response body (it does so right
    /// after `message_stop`); Rust kept reading upstream to its terminal.
    pub consumer_cancelled: bool,
    pub outcome: String,
}

/// Exact wire usage lexemes and checked aggregates ("pilot-only live wire
/// aggregate observation").
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageRecord {
    pub uncached_input: Option<String>,
    pub cache_creation_input: Option<String>,
    pub cache_read_input: Option<String>,
    pub output: Option<String>,
    pub reasoning: Option<String>,
    pub inclusive_input: Option<u64>,
    pub total: Option<u64>,
    pub complete: bool,
    pub problems: Vec<String>,
}

impl UsageRecord {
    fn from_wire(u: &wire::WireUsage) -> Self {
        let lex = |c: &Option<wire::Counter>| c.as_ref().map(|c| c.lexeme.clone());
        Self {
            uncached_input: lex(&u.uncached_input),
            cache_creation_input: lex(&u.cache_creation_input),
            cache_read_input: lex(&u.cache_read_input),
            output: lex(&u.output),
            reasoning: lex(&u.reasoning),
            inclusive_input: u.inclusive_input(),
            total: u.total(),
            complete: u.complete(),
            problems: u.problems.iter().map(|p| p.to_string()).collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolRecord {
    pub call: String,
    pub name: String,
    pub admitted: bool,
    pub refusal: Option<String>,
    pub outcome: Option<String>,
    pub result_sha256: Option<String>,
    pub result_bytes: Option<u64>,
}

/// How a slot ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Disposition {
    /// The host completed its turn and every attempt settled.
    Completed { terminal_status: String },
    /// Expected slot-local exhaustion: budget, permits, wall or input cap.
    Partial { reason: String },
    /// A global safety halt; no further paid request is admitted.
    Halted { class: String, code: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NativeEvidence {
    pub session_sha256: String,
    pub session_bytes: u64,
    pub assistant_entries: u32,
    /// (SSE message id, native entry ids with that responseId).
    pub joins: Vec<(String, Vec<String>)>,
    pub unmatched_native_response_ids: Vec<String>,
    pub model_usage_entries: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostExitRecord {
    pub normal: bool,
    pub code: Option<i32>,
    pub signal: Option<i32>,
    pub forced: bool,
    pub group_gone: bool,
    pub stderr_bytes: u64,
    pub stderr_sha256: String,
}

/// Everything one slot produced; private evidence, not an invoice.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlotCapture {
    pub slot_id: String,
    pub host_id: String,
    pub disposition: Disposition,
    pub render: Option<RenderBinding>,
    pub generations: Vec<GenerationRecord>,
    pub tools: Vec<ToolRecord>,
    /// Candidate inventory after the host stopped: (relative path, sha256).
    pub candidate_inventory: Vec<(String, String)>,
    pub native: Option<NativeEvidence>,
    pub host_exit: Option<HostExitRecord>,
    pub host_wall_ms: u64,
    pub prompt_status: Option<String>,
}

/// One step of an independently executed check.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckStep {
    pub step: String,
    pub argv: Vec<String>,
    pub exit: Option<i32>,
    pub signal: Option<i32>,
    pub timed_out: bool,
    pub stdout_sha256: String,
    pub stderr_sha256: String,
    pub artifacts: Vec<(String, String)>,
    pub wall_ms: u64,
}

/// The independently produced check of one captured candidate. The
/// session validates its bindings; it never runs the checker and never
/// accepts model self-grading.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependentCheckReceipt {
    pub slot_id: String,
    pub checker_root: PathBuf,
    pub candidate: Vec<(String, String)>,
    pub oracle_sha256: String,
    pub compiler: PathBuf,
    pub compiler_sha256: String,
    pub compiler_version: String,
    pub env_names: Vec<String>,
    pub steps: Vec<CheckStep>,
    /// Fixture receipts may satisfy verification, never live admission.
    pub fixture: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckOutcome {
    Pass,
    Fail,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SealedSlot {
    pub slot_id: String,
    pub outcome: CheckOutcome,
    pub receipt_sha256: String,
    pub disposition: Disposition,
}

/// A fully settled successful probe, sealed for an explicit coding
/// authorization. Preserves counters, spend and the original paid expiry.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeSeal {
    pub kind: String,
    pub mode: Mode,
    pub contract_sha256: String,
    pub omp_sha256: String,
    pub implementation_sha256: String,
    pub bridge_sha256: String,
    pub run_dir: PathBuf,
    pub journal_sha256: String,
    pub journal_records: u64,
    pub counters: Counters,
    pub paid_started_ns: u64,
    pub paid_deadline_ns: u64,
    pub boot_identity: String,
    /// Last allocated attempt and host identities, so coding continues them.
    pub attempt_seq: u32,
    pub host_seq: u32,
    pub probe_message_id: String,
    pub probe_request_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlotSummary {
    pub slot_id: String,
    pub state: String,
    pub disposition: Option<Disposition>,
    pub check: Option<CheckOutcome>,
}

/// Aggregate provenance result; not an invoice or live-readiness assertion.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptureReceipt {
    pub mode: Mode,
    pub scope: Scope,
    pub counters: Counters,
    pub slots: Vec<SlotSummary>,
    pub halt: Option<(String, String)>,
    pub paid_started: bool,
    pub paid_elapsed_ms: Option<u64>,
    pub journal_sha256: String,
    pub journal_records: u64,
}

// ===========================================================================
// The session.
// ===========================================================================

/// Shutdown grace after RPC stdin closes.
const HOST_EXIT_GRACE: Duration = Duration::from_secs(15);
/// After a halt: how long to wait for the host's turn to end after `abort`.
const ABORT_GRACE: Duration = Duration::from_secs(10);

struct Inner {
    mode: Mode,
    scope: Scope,
    run_dir: PathBuf,
    hosts_dir: PathBuf,
    omp: PathBuf,
    real_home: PathBuf,
    deny_literals: Vec<PathBuf>,
    journal: Journal,
    transport: Option<wire::Transport>,
    manifest: Option<LaunchManifest>,
    session_id: String,
    host_seq: u32,
    attempt_seq: u32,
    counters: Counters,
    paid_started_ns: Option<u64>,
    paid_deadline_ns: Option<u64>,
    boot_identity: String,
    halt: Option<CaptureFault>,
    slot_index: usize,
    awaiting_seal: Option<SlotCapture>,
    summaries: Vec<SlotSummary>,
    probe_sealed: bool,
}

/// The single owner of one pilot run's ledger across sequential fresh
/// native hosts. Not cloneable.
pub struct CaptureSession {
    rt: tokio::runtime::Runtime,
    inner: Inner,
}

fn runtime() -> Result<tokio::runtime::Runtime, CaptureFault> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| CaptureFault::new(FaultClass::Bootstrap, "runtime_unavailable"))
}

fn planned_slots(scope: Scope) -> Vec<SlotSummary> {
    let ids: Vec<&str> = match scope {
        Scope::Probe => vec![policy::PROBE_SLOT],
        Scope::Coding => policy::SLOT_ORDER.to_vec(),
    };
    ids.into_iter()
        .map(|id| SlotSummary {
            slot_id: id.to_string(),
            state: "not_started".into(),
            disposition: None,
            check: None,
        })
        .collect()
}

/// Claim a fresh run directory exclusively: it must not exist and its
/// parent must be an existing directory.
fn claim_run_dir(run_dir: &Path) -> Result<(PathBuf, PathBuf), CaptureFault> {
    let parent = run_dir.parent().ok_or(authority("run_dir_invalid"))?;
    let parent = fs::canonicalize(parent).map_err(|_| authority("run_dir_parent_missing"))?;
    let name = run_dir.file_name().ok_or(authority("run_dir_invalid"))?;
    let dir = parent.join(name);
    create_private_dir(&dir).map_err(|_| authority("run_dir_claim_failed"))?;
    let hosts = dir.join("hosts");
    create_private_dir(&hosts)?;
    write_private_new(
        &dir.join("claim.json"),
        serde_json::to_vec(
            &json!({"pid": std::process::id(), "claimed_at_unix_ms": display_unix_ms()}),
        )
        .map_err(|_| persistence("claim_encode_failed"))?
        .as_slice(),
    )?;
    Ok((dir, hosts))
}

impl CaptureSession {
    /// Validate authority, mode, pins, manifest and private ownership, then
    /// exclusively claim the run. No credential access or network happens
    /// before every other gate passed.
    pub fn open(admission: LaunchAdmission) -> Result<Self, CaptureFault> {
        let rt = runtime()?;
        let home = real_home()?;
        let boot = boot_identity()?;
        let (
            mode,
            scope,
            run_dir,
            hosts_dir,
            journal,
            omp,
            transport_parts,
            manifest,
            deny_literals,
        ) = match admission {
            LaunchAdmission::Verification(v) => {
                check_verification(&v)?;
                let manifest = check_verification_manifest(v.manifest.clone(), v.scope)?;
                let (run_dir, hosts_dir) = claim_run_dir(&v.run_dir)?;
                let mut journal =
                    Journal::create(&run_dir.join("journal.jsonl"), v.faults.journal_fail_after)?;
                let credential = wire::Credential::new(&v.credential)
                    .map_err(|fault| journal.credential_refusal(fault))?;
                let endpoint = wire::Endpoint::Verification {
                    addr: v.fixture_addr,
                    ca_pem: v.fixture_ca_pem.clone(),
                };
                (
                    Mode::Verification,
                    v.scope,
                    run_dir,
                    hosts_dir,
                    journal,
                    v.omp,
                    (endpoint, credential),
                    manifest,
                    Vec::new(),
                )
            }
            LaunchAdmission::Live(l) => {
                let (manifest, receipt_sha256, seal) = validate_live(&l)?;
                if seal.is_some() {
                    // A fresh coding session may cite a sealed probe as
                    // compatibility evidence, but the paid clock and
                    // counters continue only through `open_after_probe`.
                    return Err(authority("coding_requires_probe_handoff"));
                }
                let target = canonical_run_dir(&l.run_dir)?;
                if target.symlink_metadata().is_ok() {
                    return Err(authority("run_dir_claim_failed"));
                }
                claim_authority(&l.authority, &receipt_sha256, &target)?;
                let (run_dir, hosts_dir) = claim_run_dir(&target)?;
                let mut journal = Journal::create(&run_dir.join("journal.jsonl"), None)?;
                let credential = read_credential(&l.credential_file, &run_dir, &manifest)
                    .map_err(|fault| journal.credential_refusal(fault))?;
                let credential_path = fs::canonicalize(&l.credential_file).map_err(|_| {
                    journal.credential_refusal(CaptureFault::new(
                        FaultClass::Privacy,
                        "credential_missing",
                    ))
                })?;
                (
                    Mode::Live,
                    l.scope,
                    run_dir,
                    hosts_dir,
                    journal,
                    l.omp,
                    (wire::Endpoint::Live, credential),
                    Some(manifest),
                    vec![credential_path],
                )
            }
        };
        let transport = {
            let _guard = rt.enter();
            wire::Transport::new(&transport_parts.0, transport_parts.1)?
        };
        let mut inner = Inner {
            mode,
            scope,
            hosts_dir,
            omp: fs::canonicalize(&omp).map_err(|_| pins("omp_missing"))?,
            real_home: home,
            deny_literals,
            journal,
            transport: Some(transport),
            manifest,
            session_id: format!("s-{}", uuid::Uuid::new_v4().simple()),
            host_seq: 0,
            attempt_seq: 0,
            counters: Counters::default(),
            paid_started_ns: None,
            paid_deadline_ns: None,
            boot_identity: boot,
            halt: None,
            slot_index: 0,
            awaiting_seal: None,
            summaries: planned_slots(scope),
            probe_sealed: false,
            run_dir,
        };
        inner.journal.append(
            "admitted",
            json!({
                "mode": mode, "scope": scope,
                "contract_sha256": policy::CONTRACT_SHA256,
                "omp_sha256": policy::OMP_SHA256,
                "bridge_sha256": sha256_hex(host::BRIDGE_SOURCE),
                "boot_identity": inner.boot_identity,
                "session": inner.session_id,
            }),
        )?;
        Ok(Self { rt, inner })
    }

    /// Continue the same run after a clean, sealed probe with a new explicit
    /// coding admission. Counters, spend, journal and the original paid
    /// expiry carry over; expiry, reboot, clock or identity mismatch refuse.
    pub fn open_after_probe(
        seal: &ProbeSeal,
        admission: LaunchAdmission,
    ) -> Result<Self, CaptureFault> {
        let rt = runtime()?;
        if seal.kind != "bounded-pilot-v1-probe-seal"
            || seal.contract_sha256 != policy::CONTRACT_SHA256
            || seal.omp_sha256 != policy::OMP_SHA256
            || seal.bridge_sha256 != sha256_hex(host::BRIDGE_SOURCE)
            || seal.counters.outstanding_micro_usd != 0
        {
            return Err(authority("probe_seal_invalid"));
        }
        if boot_identity()? != seal.boot_identity {
            return Err(CaptureFault::new(
                FaultClass::Deadline,
                "probe_seal_boot_changed",
            ));
        }
        let now = continuous_nanos()?;
        if now < seal.paid_started_ns {
            return Err(CaptureFault::new(
                FaultClass::Deadline,
                "probe_seal_clock_mismatch",
            ));
        }
        if now >= seal.paid_deadline_ns {
            return Err(CaptureFault::new(
                FaultClass::Deadline,
                "paid_wall_exhausted",
            ));
        }
        let (mode, journal, omp, transport, manifest, deny_literals) = match admission {
            LaunchAdmission::Verification(v) => {
                if seal.mode != Mode::Verification || v.scope != Scope::Coding {
                    return Err(authority("probe_handoff_mode_mismatch"));
                }
                check_verification(&v)?;
                let manifest = check_verification_manifest(v.manifest.clone(), Scope::Coding)?;
                // Admit this verification path before touching the sealed journal.
                let mut journal = Journal::reopen(
                    &seal.run_dir.join("journal.jsonl"),
                    &seal.journal_sha256,
                    seal.journal_records,
                )?;
                verify_seal_against_journal(seal, &journal.path)?;
                let credential = wire::Credential::new(&v.credential)
                    .map_err(|fault| journal.credential_refusal(fault))?;
                let endpoint = wire::Endpoint::Verification {
                    addr: v.fixture_addr,
                    ca_pem: v.fixture_ca_pem.clone(),
                };
                let _guard = rt.enter();
                (
                    Mode::Verification,
                    journal,
                    v.omp,
                    wire::Transport::new(&endpoint, credential)?,
                    manifest,
                    Vec::new(),
                )
            }
            LaunchAdmission::Live(l) => {
                if seal.mode != Mode::Live || l.scope != Scope::Coding {
                    return Err(authority("probe_handoff_mode_mismatch"));
                }
                let (manifest, receipt_sha256, cited) = validate_live(&l)?;
                match cited {
                    Some(c)
                        if c.journal_sha256 == seal.journal_sha256 && c.run_dir == seal.run_dir => {
                    }
                    _ => return Err(authority("compatibility_evidence_not_this_probe")),
                }
                // Only validated coding admission may inspect this cited probe.
                // A terminal prefix refuses before consuming fresh authority;
                // no journal append is allowed until the claim succeeds.
                let mut journal = Journal::reopen(
                    &seal.run_dir.join("journal.jsonl"),
                    &seal.journal_sha256,
                    seal.journal_records,
                )?;
                verify_seal_against_journal(seal, &journal.path)?;
                claim_authority(&l.authority, &receipt_sha256, &seal.run_dir)?;
                let credential = read_credential(&l.credential_file, &seal.run_dir, &manifest)
                    .map_err(|fault| journal.credential_refusal(fault))?;
                let credential_path = fs::canonicalize(&l.credential_file).map_err(|_| {
                    journal.credential_refusal(CaptureFault::new(
                        FaultClass::Privacy,
                        "credential_missing",
                    ))
                })?;
                let _guard = rt.enter();
                (
                    Mode::Live,
                    journal,
                    l.omp,
                    wire::Transport::new(&wire::Endpoint::Live, credential)?,
                    Some(manifest),
                    vec![credential_path],
                )
            }
        };
        let mut inner = Inner {
            mode,
            scope: Scope::Coding,
            hosts_dir: seal.run_dir.join("hosts"),
            omp: fs::canonicalize(&omp).map_err(|_| pins("omp_missing"))?,
            real_home: real_home()?,
            deny_literals,
            journal,
            transport: Some(transport),
            manifest,
            session_id: format!("s-{}", uuid::Uuid::new_v4().simple()),
            host_seq: seal.host_seq,
            attempt_seq: seal.attempt_seq,
            counters: seal.counters,
            paid_started_ns: Some(seal.paid_started_ns),
            paid_deadline_ns: Some(seal.paid_deadline_ns),
            boot_identity: seal.boot_identity.clone(),
            halt: None,
            slot_index: 0,
            awaiting_seal: None,
            summaries: planned_slots(Scope::Coding),
            probe_sealed: false,
            run_dir: seal.run_dir.clone(),
        };
        if file_sha256(&inner.omp, "omp_missing")? != policy::OMP_SHA256 {
            return Err(pins("omp_pin_mismatch"));
        }
        inner.journal.append(
            "coding_handoff",
            json!({"probe_journal_sha256": seal.journal_sha256, "counters": seal.counters, "session": inner.session_id}),
        )?;
        Ok(Self { rt, inner })
    }

    pub fn mode(&self) -> Mode {
        self.inner.mode
    }

    pub fn counters(&self) -> Counters {
        self.inner.counters
    }

    pub fn halted(&self) -> Option<CaptureFault> {
        self.inner.halt
    }

    pub fn run_dir(&self) -> &Path {
        &self.inner.run_dir
    }

    /// Launch a fresh host for the next frozen slot, require Ready before
    /// the sole prompt, enforce every generation and tool rule and capture
    /// candidate and transport facts.
    pub fn run_slot(&mut self, slot: PreparedSlot) -> Result<SlotCapture, CaptureFault> {
        let Self { rt, inner } = self;
        rt.block_on(inner.run_slot(slot, Purpose::Capture))
    }

    /// Validate an independently produced check of the awaiting slot.
    pub fn seal_slot(
        &mut self,
        capture: &SlotCapture,
        receipt: &IndependentCheckReceipt,
    ) -> Result<SealedSlot, CaptureFault> {
        self.inner.seal_slot(capture, receipt)
    }

    /// Seal a fully settled successful probe.
    pub fn seal_probe(&mut self, capture: &SlotCapture) -> Result<ProbeSeal, CaptureFault> {
        self.inner.seal_probe(capture)
    }

    /// Close the session and return the aggregate receipt.
    pub fn finish(mut self) -> Result<CaptureReceipt, CaptureFault> {
        self.inner.finish()
    }
}

/// Render the actual host's request commitments for a slot without any
/// provider request, count or credential.
pub fn render_binding(
    admission: &PreviewAdmission,
    slot: &PreparedSlot,
) -> Result<RenderBinding, CaptureFault> {
    let rt = runtime()?;
    if file_sha256(&admission.omp, "omp_missing")? != policy::OMP_SHA256 {
        return Err(pins("omp_pin_mismatch"));
    }
    let (run_dir, hosts_dir) = claim_run_dir(&admission.run_dir)?;
    let journal = Journal::create(&run_dir.join("journal.jsonl"), None)?;
    let mut inner = Inner {
        mode: Mode::Verification,
        scope: Scope::Coding,
        hosts_dir,
        omp: fs::canonicalize(&admission.omp).map_err(|_| pins("omp_missing"))?,
        real_home: real_home()?,
        deny_literals: Vec::new(),
        journal,
        transport: None,
        manifest: None,
        session_id: format!("s-{}", uuid::Uuid::new_v4().simple()),
        host_seq: 0,
        attempt_seq: 0,
        counters: Counters::default(),
        paid_started_ns: None,
        paid_deadline_ns: None,
        boot_identity: boot_identity()?,
        halt: None,
        slot_index: 0,
        awaiting_seal: None,
        summaries: Vec::new(),
        probe_sealed: false,
        run_dir,
    };
    inner
        .journal
        .append("preview_admitted", json!({"slot": slot.slot_id}))?;
    let capture = rt.block_on(inner.run_slot(slot.clone(), Purpose::Preview))?;
    capture.render.ok_or(CaptureFault::new(
        FaultClass::Bootstrap,
        "preview_not_rendered",
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    Preview,
    Capture,
}

struct Active {
    fetch: u32,
    record_index: usize,
    stream: wire::MessagesStream,
    deadline: tokio::time::Instant,
    validated: wire::ValidatedRequest,
    consumer_cancelled: bool,
}

/// Mutable state of one slot's host run.
struct SlotRun {
    slot_id: String,
    host_id: String,
    purpose: Purpose,
    ledger: SlotLedger,
    expect: wire::RequestExpectation,
    /// The manifest's frozen render for this slot, when admitted with one.
    frozen_render: Option<RenderCommitments>,
    tool_policy: host::ToolPolicy,
    rpc_ready: bool,
    bridge_ready: bool,
    setup_sent: bool,
    pending_setup: Vec<&'static str>,
    prompt_sent: bool,
    prompt_status: Option<String>,
    last_invocation: u32,
    prior: Option<wire::PriorGeneration>,
    completed_calls: Vec<String>,
    generations: Vec<GenerationRecord>,
    tools: Vec<ToolRecord>,
    render: Option<RenderBinding>,
    active: Option<Active>,
    settled_fetches: Vec<u32>,
    seen_fetches: Vec<u32>,
    partial: Option<String>,
    preview_done: bool,
}

const SETUP_COMMANDS: [&str; 4] = ["c1", "c2", "c3", "c4"];

fn halt_class(code: &'static str) -> FaultClass {
    match code {
        "host_extension_error" => FaultClass::Bootstrap,
        "host_model_changed" | "host_model_fallback" | "host_compaction" => {
            FaultClass::RequestBinding
        }
        _ => FaultClass::Retry,
    }
}

fn bridge_fault_code(code: protocol::BridgeFault) -> &'static str {
    use protocol::BridgeFault as B;
    match code {
        B::Config => "bridge_config",
        B::DelegateImport => "bridge_delegate_import",
        B::ModelSelection => "bridge_model_selection",
        B::Settings => "bridge_settings",
        B::Protocol => "bridge_protocol",
        B::Invocation => "bridge_invocation",
        B::ServiceTier => "bridge_service_tier",
        B::FetchShape => "bridge_fetch_shape",
        B::ChannelWrite => "bridge_channel_write",
        B::Internal => "bridge_internal",
    }
}

/// How the pinned host renders a canonical working directory in its
/// volatile reminder: on macOS the `/private` firmlink prefix of `/var`,
/// `/tmp` and `/etc` is dropped (observed on OMP 18.13.2). Exact mapping,
/// not a loosened match.
pub fn host_display_path(canonical: &Path) -> String {
    let text = canonical.to_string_lossy().into_owned();
    if cfg!(target_os = "macos") {
        for alias in ["/var/", "/tmp/", "/etc/"] {
            if let Some(rest) = text
                .strip_prefix("/private")
                .filter(|r| r.starts_with(alias))
            {
                return rest.to_string();
            }
        }
    }
    text
}

fn hash_sources(workspace: &Path, rels: &[String]) -> Result<Vec<(String, String)>, CaptureFault> {
    let mut out = Vec::with_capacity(rels.len());
    for rel in rels {
        let path = workspace.join(rel);
        let meta = fs::symlink_metadata(&path)
            .map_err(|_| CaptureFault::new(FaultClass::Containment, "source_missing"))?;
        if !meta.is_file() {
            return Err(CaptureFault::new(
                FaultClass::Containment,
                "source_not_regular",
            ));
        }
        let bytes = fs::read(&path)
            .map_err(|_| CaptureFault::new(FaultClass::Containment, "source_unreadable"))?;
        out.push((rel.clone(), sha256_hex(&bytes)));
    }
    Ok(out)
}

/// Every regular file under the workspace, relative, sorted (bounded).
fn list_workspace(workspace: &Path) -> Result<Vec<String>, CaptureFault> {
    let mut out = Vec::new();
    let mut stack = vec![workspace.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir)
            .map_err(|_| CaptureFault::new(FaultClass::Containment, "workspace_unreadable"))?;
        for entry in entries {
            let entry = entry
                .map_err(|_| CaptureFault::new(FaultClass::Containment, "workspace_unreadable"))?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path)
                .map_err(|_| CaptureFault::new(FaultClass::Containment, "workspace_unreadable"))?;
            let rel = path
                .strip_prefix(workspace)
                .map_err(|_| CaptureFault::new(FaultClass::Containment, "workspace_unreadable"))?
                .to_string_lossy()
                .into_owned();
            if meta.is_dir() {
                stack.push(path);
            } else {
                out.push(rel);
            }
            if out.len() > 4096 {
                return Err(CaptureFault::new(
                    FaultClass::Containment,
                    "workspace_effect_out_of_scope",
                ));
            }
        }
    }
    out.sort();
    Ok(out)
}

impl Inner {
    fn record(&mut self, kind: &str, fields: Value) -> Result<(), CaptureFault> {
        self.journal.append(kind, fields)
    }

    /// Record evidence after a host launched: a failure enters the global
    /// halt instead of abandoning a running host.
    fn record_or_halt(&mut self, kind: &str, fields: Value) {
        if let Err(f) = self.journal.append(kind, fields) {
            self.enter_halt(f);
        }
    }

    /// Enter the monotonic global halt (first fault wins; later evidence
    /// never resumes admission).
    fn enter_halt(&mut self, f: CaptureFault) {
        if self.halt.is_none() {
            self.halt = Some(f);
            let _ = self
                .journal
                .append("halt", json!({"class": f.class.as_str(), "code": f.code}));
        }
    }

    fn next_attempt(&mut self) -> Result<u32, CaptureFault> {
        self.attempt_seq = self.attempt_seq.checked_add(1).ok_or(CaptureFault::new(
            FaultClass::Usage,
            "attempt_ids_exhausted",
        ))?;
        Ok(self.attempt_seq)
    }

    fn expected_slot(&self) -> Option<&'static str> {
        match self.scope {
            Scope::Probe => (self.slot_index == 0).then_some(policy::PROBE_SLOT),
            Scope::Coding => policy::SLOT_ORDER.get(self.slot_index).copied(),
        }
    }

    fn check_slot_admission(
        &self,
        slot: &PreparedSlot,
        purpose: Purpose,
    ) -> Result<(), CaptureFault> {
        if purpose == Purpose::Preview {
            return Ok(());
        }
        if let Some(f) = self.halt {
            let _ = f;
            return Err(authority("session_halted"));
        }
        if self.awaiting_seal.is_some() {
            return Err(authority("previous_slot_unsealed"));
        }
        if self.probe_sealed {
            return Err(authority("probe_already_sealed"));
        }
        if self.expected_slot() != Some(slot.slot_id.as_str()) {
            return Err(authority("slot_out_of_order"));
        }
        if let Some(manifest) = &self.manifest {
            let entry = manifest
                .slots
                .iter()
                .find(|s| s.slot_id == slot.slot_id)
                .ok_or(authority("slot_not_in_manifest"))?;
            if entry.workspace != slot.workspace
                || entry.prompt_sha256 != sha256_hex(slot.initial_user_text.as_bytes())
                || entry.sources != slot.expected_sources
            {
                return Err(authority("slot_manifest_mismatch"));
            }
        }
        Ok(())
    }

    async fn run_slot(
        &mut self,
        slot: PreparedSlot,
        purpose: Purpose,
    ) -> Result<SlotCapture, CaptureFault> {
        // Caller errors (order, seal, manifest identity) refuse without
        // changing the session; once admitted, every pin, source, bound or
        // launch fault enters the global halt so the slot cannot be re-run.
        self.check_slot_admission(&slot, purpose)?;
        match self.run_admitted_slot(slot, purpose).await {
            Err(f) if purpose == Purpose::Capture => {
                self.enter_halt(f);
                Err(f)
            }
            other => other,
        }
    }

    async fn run_admitted_slot(
        &mut self,
        slot: PreparedSlot,
        purpose: Purpose,
    ) -> Result<SlotCapture, CaptureFault> {
        if purpose == Purpose::Capture {
            let now = continuous_nanos()?;
            if self.paid_deadline_ns.is_some_and(|d| now >= d) {
                return Err(CaptureFault::new(
                    FaultClass::Deadline,
                    "paid_wall_exhausted",
                ));
            }
            let c = &self.counters;
            let bounds_left = c.generation_permits < policy::MAX_GENERATION_PERMITS
                && c.count_calls < policy::MAX_COUNT_CALLS
                && c.provider_http_attempts.saturating_add(2) <= policy::MAX_PROVIDER_HTTP_ATTEMPTS
                && reservation_fits(
                    c.spent_micro_usd,
                    c.outstanding_micro_usd,
                    policy::PILOT_ENVELOPE_MICRO_USD,
                )
                .unwrap_or(false);
            if !bounds_left {
                return Err(CaptureFault::new(
                    FaultClass::SlotLimit,
                    "pilot_bounds_exhausted",
                ));
            }
        }
        let workspace = fs::canonicalize(&slot.workspace)
            .map_err(|_| CaptureFault::new(FaultClass::Bootstrap, "workspace_missing"))?;
        if workspace != slot.workspace {
            return Err(CaptureFault::new(
                FaultClass::Bootstrap,
                "workspace_not_canonical",
            ));
        }
        if workspace.starts_with(&self.run_dir) || self.run_dir.starts_with(&workspace) {
            return Err(CaptureFault::new(
                FaultClass::Bootstrap,
                "workspace_overlaps_run",
            ));
        }
        let rels: Vec<String> = slot
            .expected_sources
            .iter()
            .map(|(r, _)| r.clone())
            .collect();
        let mut expected_rels: Vec<&str> = rels.iter().map(String::as_str).collect();
        expected_rels.sort_unstable();
        let mut read_set: Vec<&str> = policy::READ_SET.to_vec();
        read_set.sort_unstable();
        if expected_rels != read_set {
            return Err(CaptureFault::new(
                FaultClass::Pins,
                "slot_sources_not_frozen_set",
            ));
        }
        if hash_sources(&workspace, &rels)? != slot.expected_sources {
            return Err(CaptureFault::new(FaultClass::Pins, "slot_sources_changed"));
        }
        if list_workspace(&workspace)?.len() != rels.len() {
            return Err(CaptureFault::new(
                FaultClass::Pins,
                "slot_workspace_not_seed",
            ));
        }
        if file_sha256(&self.omp, "omp_missing")? != policy::OMP_SHA256 {
            return Err(pins("omp_pin_mismatch"));
        }
        let is_probe = slot.slot_id == policy::PROBE_SLOT;
        if is_probe && slot.initial_user_text != policy::PROBE_INSTRUCTION {
            return Err(CaptureFault::new(
                FaultClass::RequestBinding,
                "probe_instruction_mismatch",
            ));
        }
        self.host_seq = self.host_seq.checked_add(1).ok_or(CaptureFault::new(
            FaultClass::Bootstrap,
            "host_ids_exhausted",
        ))?;
        let host_id = format!("h{}-{}", self.host_seq, slot.slot_id);
        let identity = protocol::ChannelIdentity::new(self.session_id.clone(), host_id.clone())
            .map_err(|f| CaptureFault::new(FaultClass::Protocol, f.0))?;
        let mut deny = vec![self.run_dir.clone()];
        if let Some(m) = &self.manifest {
            deny.push(m.isolation_root.clone());
        }
        // The slot's own workspace is re-allowed after these denials.
        deny.retain(|d| !d.starts_with(&workspace));
        let spec = host::HostSpec {
            omp: self.omp.clone(),
            root: self.hosts_dir.join(&host_id),
            workspace: workspace.clone(),
            real_home: self.real_home.clone(),
            deny,
            deny_literals: self.deny_literals.clone(),
            identity,
        };
        let tool_policy = if is_probe || purpose == Purpose::Preview {
            host::ToolPolicy {
                workspace: workspace.clone(),
                read: Vec::new(),
                edit: Vec::new(),
            }
        } else {
            host::ToolPolicy::pilot(workspace.clone())
        };
        let (envelope, permit_cap) = if is_probe {
            (policy::PROBE_ENVELOPE_MICRO_USD, policy::PERMITS_PER_PROBE)
        } else {
            (
                policy::CODING_SLOT_ENVELOPE_MICRO_USD,
                policy::PERMITS_PER_CODING_SLOT,
            )
        };
        let mut run = SlotRun {
            slot_id: slot.slot_id.clone(),
            host_id: host_id.clone(),
            purpose,
            ledger: SlotLedger {
                envelope,
                permit_cap,
                spent: 0,
                outstanding: 0,
                permits: 0,
            },
            expect: wire::RequestExpectation {
                initial_user_text: slot.initial_user_text.clone(),
                workspace: host_display_path(&workspace),
            },
            tool_policy,
            rpc_ready: false,
            bridge_ready: false,
            setup_sent: false,
            pending_setup: SETUP_COMMANDS.to_vec(),
            prompt_sent: false,
            prompt_status: None,
            last_invocation: 0,
            prior: None,
            completed_calls: Vec::new(),
            generations: Vec::new(),
            tools: Vec::new(),
            render: None,
            active: None,
            settled_fetches: Vec::new(),
            seen_fetches: Vec::new(),
            partial: None,
            preview_done: false,
            frozen_render: self
                .manifest
                .as_ref()
                .filter(|_| purpose == Purpose::Capture)
                .and_then(|m| m.slots.iter().find(|s| s.slot_id == slot.slot_id))
                .map(|s| s.render.clone()),
        };
        let started = tokio::time::Instant::now();
        // The wall covers host startup through shutdown, so the event loop
        // ends early enough to leave the abort and exit graces inside it.
        let wall = Duration::from_secs(if purpose == Purpose::Preview || is_probe {
            policy::GENERATION_DEADLINE_SECS
        } else {
            policy::CODING_SLOT_WALL_SECS
        });
        let slot_deadline = started + wall.saturating_sub(ABORT_GRACE + HOST_EXIT_GRACE);
        if purpose == Purpose::Capture
            && let Some(s) = self.summaries.get_mut(self.slot_index)
        {
            s.state = "started".into();
        }
        let (host, mut events) = host::Host::launch(&spec)?;
        self.record_or_halt(
            "host_launched",
            json!({
                "slot": run.slot_id, "host": host_id, "pid": host.launch.pid,
                "argv_sha256": host.launch.argv_sha256, "policy_sha256": host.launch.policy_sha256,
                "bridge_sha256": host.launch.bridge_sha256, "models_yml_sha256": host.launch.models_yml_sha256,
                "config_yml_sha256": host.launch.config_yml_sha256, "env_names": host.launch.env_names,
            }),
        );

        let outcome = self
            .drive(&host, &mut events, &mut run, slot_deadline)
            .await;
        // Shutdown: normal path closes the channel and RPC stdin; a halt or
        // slot exhaustion first aborts the host's turn.
        if outcome.is_err()
            || run.partial.is_some()
            || (run.prompt_status.is_none() && run.prompt_sent)
        {
            if let Some(active) = run.active.take() {
                // A transmitted generation abandoned by a halt keeps its
                // observation so far and its full reservation.
                let obs = active.stream.observation();
                if let Some(record) = run.generations.get_mut(active.record_index) {
                    fill_observation(record, &obs);
                    record.consumer_cancelled = active.consumer_cancelled;
                    record.outcome = "abandoned_after_halt".into();
                }
                self.record_or_halt(
                    "generation_abandoned",
                    json!({"slot": run.slot_id, "fetch": active.fetch, "message_id": obs.message_id,
                           "request_id": obs.request_id, "reservation": "retained"}),
                );
                let _ = host
                    .send_bridge(
                        ParentMessage::ResponseError {
                            fetch: active.fetch,
                            class: FaultClass::Transport,
                        },
                        Vec::new(),
                    )
                    .await;
            }
            if run.prompt_sent && run.prompt_status.is_none() {
                let _ = host.send_rpc(&json!({"id": "a1", "type": "abort"})).await;
                let until = tokio::time::Instant::now() + ABORT_GRACE;
                while run.prompt_status.is_none() {
                    match tokio::time::timeout_at(until, events.recv()).await {
                        Ok(Some(host::HostEvent::Rpc(host::RpcEvent::PromptResult {
                            status,
                            ..
                        }))) => {
                            run.prompt_status = Some(status);
                        }
                        Ok(Some(host::HostEvent::Bridge(inbound))) => match inbound.msg {
                            protocol::ChildMessage::ToolCheck { call, .. } => {
                                let _ = host
                                    .send_bridge(
                                        ParentMessage::ToolDecision { call, allow: false },
                                        Vec::new(),
                                    )
                                    .await;
                            }
                            protocol::ChildMessage::FetchBegin { fetch, .. } => {
                                // Never transmitted: refused and classified.
                                let _ = host
                                    .send_bridge(
                                        ParentMessage::ResponseError {
                                            fetch,
                                            class: FaultClass::Retry,
                                        },
                                        Vec::new(),
                                    )
                                    .await;
                                self.record_or_halt(
                                    "fetch_refused_after_stop",
                                    json!({"slot": run.slot_id, "fetch": fetch}),
                                );
                                self.enter_halt(CaptureFault::new(
                                    FaultClass::Retry,
                                    "host_request_after_stop",
                                ));
                            }
                            protocol::ChildMessage::Fault { code } => self.enter_halt(
                                CaptureFault::new(FaultClass::Bootstrap, bridge_fault_code(code)),
                            ),
                            _ => {}
                        },
                        Ok(Some(host::HostEvent::Rpc(host::RpcEvent::Forbidden(code)))) => {
                            self.enter_halt(CaptureFault::new(halt_class(code), code));
                        }
                        Ok(Some(_)) => {}
                        Ok(None) | Err(_) => break,
                    }
                }
            }
        }
        let close = if outcome.is_ok() && self.halt.is_none() {
            CloseReason::Completed
        } else {
            CloseReason::Halted
        };
        let _ = host
            .send_bridge(ParentMessage::Close { reason: close }, Vec::new())
            .await;
        let paths = host.paths.clone();
        let (drained, exit) = host.finish(events, HOST_EXIT_GRACE).await;
        let host_wall_ms = started.elapsed().as_millis() as u64;
        // Nothing the host does after its turn may pass unexamined: a fetch
        // or tool call after the end, a bridge fault or a forbidden host
        // behaviour is classified and halts.
        if purpose == Purpose::Capture {
            for event in drained {
                let fault = match event {
                    host::HostEvent::Bridge(inbound) => match inbound.msg {
                        protocol::ChildMessage::FetchBegin { .. } => Some(CaptureFault::new(
                            FaultClass::Retry,
                            "host_request_after_turn",
                        )),
                        protocol::ChildMessage::ToolCheck { .. } => Some(CaptureFault::new(
                            FaultClass::Containment,
                            "host_tool_after_turn",
                        )),
                        protocol::ChildMessage::Fault { code } => Some(CaptureFault::new(
                            FaultClass::Bootstrap,
                            bridge_fault_code(code),
                        )),
                        _ => None,
                    },
                    host::HostEvent::BridgeFault(f) => {
                        Some(CaptureFault::new(FaultClass::Protocol, f.0))
                    }
                    host::HostEvent::Rpc(host::RpcEvent::Forbidden(code)) => {
                        Some(CaptureFault::new(halt_class(code), code))
                    }
                    _ => None,
                };
                if let Some(f) = fault {
                    self.record_or_halt(
                        "post_turn_event",
                        json!({"slot": run.slot_id, "class": f.class.as_str(), "code": f.code}),
                    );
                    self.enter_halt(f);
                }
            }
            if Duration::from_millis(host_wall_ms) > wall && run.partial.is_none() {
                run.partial = Some("slot_wall_exhausted".into());
            }
        }
        let exit_record = HostExitRecord {
            normal: exit.normal,
            code: exit.code,
            signal: exit.signal,
            forced: exit.forced,
            group_gone: exit.group_gone,
            stderr_bytes: exit.stderr_bytes,
            stderr_sha256: exit.stderr_sha256.clone(),
        };
        if let Err(f) = outcome {
            self.enter_halt(f);
        }
        self.record_or_halt(
            "host_exit",
            json!({"slot": run.slot_id, "host": run.host_id, "exit": exit_record}),
        );
        if purpose == Purpose::Capture && (!exit.normal || !exit.group_gone) {
            self.enter_halt(CaptureFault::new(
                FaultClass::Cleanup,
                "host_cleanup_unresolved",
            ));
        }

        // Native identity reconciliation (read-only).
        let native = match host::read_native_session(&paths.sessions) {
            Ok(Some(session)) => {
                let wire_ids: Vec<String> = run
                    .generations
                    .iter()
                    .filter_map(|g| g.message_id.clone())
                    .collect();
                let joins: Vec<(String, Vec<String>)> = wire_ids
                    .iter()
                    .map(|id| {
                        let entries = session
                            .assistant
                            .iter()
                            .filter(|a| a.response_id.as_deref() == Some(id.as_str()))
                            .map(|a| a.entry_id.clone())
                            .collect();
                        (id.clone(), entries)
                    })
                    .collect();
                let unmatched: Vec<String> = session
                    .assistant
                    .iter()
                    .filter_map(|a| a.response_id.clone())
                    .filter(|r| !wire_ids.contains(r))
                    .collect();
                if purpose == Purpose::Capture {
                    if !unmatched.is_empty() {
                        self.enter_halt(CaptureFault::new(
                            FaultClass::Containment,
                            "native_unobserved_generation",
                        ));
                    }
                    if session.unparsed_lines > 0 {
                        self.enter_halt(CaptureFault::new(
                            FaultClass::Usage,
                            "native_session_unparsed",
                        ));
                    }
                    if session.model_usage_entries > 0 {
                        self.enter_halt(CaptureFault::new(
                            FaultClass::Retry,
                            "native_out_of_transcript_usage",
                        ));
                    }
                }
                let evidence = NativeEvidence {
                    session_sha256: session.sha256,
                    session_bytes: session.bytes,
                    assistant_entries: session.assistant.len() as u32,
                    joins,
                    unmatched_native_response_ids: unmatched,
                    model_usage_entries: session.model_usage_entries,
                };
                self.record_or_halt(
                    "native_session",
                    json!({"slot": run.slot_id, "native": evidence}),
                );
                Some(evidence)
            }
            Ok(None) => {
                // A transmitted generation without any native session is an
                // evidence gap, not an empty result.
                if purpose == Purpose::Capture
                    && run.generations.iter().any(|g| g.messages_attempt.is_some())
                {
                    self.enter_halt(CaptureFault::new(
                        FaultClass::Usage,
                        "native_session_missing",
                    ));
                }
                None
            }
            Err(f) => {
                if purpose == Purpose::Capture {
                    self.enter_halt(f);
                }
                None
            }
        };

        // Candidate inventory and containment of effects.
        let rels: Vec<String> = slot
            .expected_sources
            .iter()
            .map(|(r, _)| r.clone())
            .collect();
        let inventory = hash_sources(&workspace, &rels).unwrap_or_default();
        if purpose == Purpose::Capture {
            let listing = list_workspace(&workspace).unwrap_or_default();
            let mut expected = rels.clone();
            expected.sort();
            if listing != expected {
                self.enter_halt(CaptureFault::new(
                    FaultClass::Containment,
                    "workspace_effect_out_of_scope",
                ));
            }
            for ((rel, before), (_, after)) in slot.expected_sources.iter().zip(inventory.iter()) {
                let editable = policy::EDIT_SET.contains(&rel.as_str()) && !is_probe;
                if before != after && !editable {
                    self.enter_halt(CaptureFault::new(
                        FaultClass::Containment,
                        "protected_file_changed",
                    ));
                }
            }
            if inventory.len() != rels.len() {
                self.enter_halt(CaptureFault::new(
                    FaultClass::Containment,
                    "candidate_inventory_incomplete",
                ));
            }
        }

        let disposition = match (self.halt, &run.partial) {
            (Some(f), _) if purpose == Purpose::Capture => Disposition::Halted {
                class: f.class.as_str().to_string(),
                code: f.code.to_string(),
            },
            (_, Some(reason)) => Disposition::Partial {
                reason: reason.clone(),
            },
            _ => Disposition::Completed {
                terminal_status: run
                    .prompt_status
                    .clone()
                    .unwrap_or_else(|| "unknown".into()),
            },
        };
        let capture = SlotCapture {
            slot_id: run.slot_id.clone(),
            host_id: run.host_id.clone(),
            disposition: disposition.clone(),
            render: run.render.clone(),
            generations: run.generations.clone(),
            tools: run.tools.clone(),
            candidate_inventory: inventory,
            native,
            host_exit: Some(exit_record),
            host_wall_ms,
            prompt_status: run.prompt_status.clone(),
        };
        self.record_or_halt(
            "slot_closed",
            json!({"slot": capture.slot_id, "disposition": disposition, "generations": capture.generations.len(),
                   "tools": capture.tools.len(), "counters": self.counters, "host_wall_ms": host_wall_ms}),
        );
        if purpose == Purpose::Capture {
            if let Some(s) = self.summaries.get_mut(self.slot_index) {
                s.state = "closed".into();
                s.disposition = Some(disposition);
            }
            if self.scope == Scope::Coding {
                self.awaiting_seal = Some(capture.clone());
            } else {
                self.slot_index += 1;
            }
        }
        Ok(capture)
    }

    /// The slot event loop. Returns Err on a global fault; Ok when the
    /// host's turn ended (or the slot closed as partial).
    async fn drive(
        &mut self,
        host: &host::Host,
        events: &mut tokio::sync::mpsc::Receiver<host::HostEvent>,
        run: &mut SlotRun,
        slot_deadline: tokio::time::Instant,
    ) -> Result<(), CaptureFault> {
        loop {
            if let Some(f) = self.halt {
                return Err(f);
            }
            if run.prompt_status.is_some() && run.active.is_none() {
                return Ok(());
            }
            if run.partial.is_some() && run.active.is_none() {
                return Ok(());
            }
            if run.preview_done && run.purpose == Purpose::Preview {
                return Ok(());
            }
            let streaming = run.active.is_some();
            let active_deadline = run
                .active
                .as_ref()
                .map(|a| a.deadline)
                .unwrap_or(slot_deadline);
            tokio::select! {
                biased;
                ev = events.recv() => {
                    let Some(ev) = ev else {
                        return Err(CaptureFault::new(FaultClass::Bootstrap, "host_channels_closed"));
                    };
                    self.on_host_event(host, run, ev, slot_deadline).await?;
                }
                next = async {
                    match run.active.as_mut() {
                        Some(a) => a.stream.next_event(active_deadline).await,
                        None => std::future::pending().await,
                    }
                }, if streaming => {
                    self.on_stream(host, run, next).await?;
                }
                _ = tokio::time::sleep_until(slot_deadline) => {
                    if run.active.is_some() {
                        return Err(CaptureFault::new(FaultClass::Deadline, "slot_wall_during_generation"));
                    }
                    if run.purpose == Purpose::Preview {
                        return Err(CaptureFault::new(FaultClass::Deadline, "preview_wall_exhausted"));
                    }
                    run.partial = Some("slot_wall_exhausted".into());
                    self.record("slot_limit", json!({"slot": run.slot_id, "reason": "slot_wall_exhausted"}))?;
                    return Ok(());
                }
            }
        }
    }

    async fn on_host_event(
        &mut self,
        host: &host::Host,
        run: &mut SlotRun,
        ev: host::HostEvent,
        slot_deadline: tokio::time::Instant,
    ) -> Result<(), CaptureFault> {
        use host::{HostEvent as E, RpcEvent as R};
        match ev {
            E::Rpc(R::Ready) => {
                run.rpc_ready = true;
                self.maybe_setup(host, run).await?;
            }
            E::Rpc(R::Response {
                id,
                command,
                success,
                state,
            }) => {
                let id = id.unwrap_or_default();
                if let Some(pos) = run.pending_setup.iter().position(|c| *c == id) {
                    if !success {
                        return Err(CaptureFault::new(
                            FaultClass::Bootstrap,
                            "host_setup_refused",
                        ));
                    }
                    if id == "c4" {
                        let s = state.ok_or(CaptureFault::new(
                            FaultClass::Bootstrap,
                            "host_state_missing",
                        ))?;
                        let ok = s.provider.as_deref() == Some(policy::PROVIDER)
                            && s.model.as_deref() == Some(policy::MODEL)
                            && s.api.as_deref() == Some(policy::BRIDGE_API)
                            && s.max_tokens == Some(policy::MAX_OUTPUT_TOKENS)
                            && s.thinking_level.as_deref() == Some(policy::THINKING_LEVEL)
                            && s.auto_compaction == Some(false);
                        if !ok {
                            return Err(CaptureFault::new(
                                FaultClass::Bootstrap,
                                "host_state_not_frozen",
                            ));
                        }
                    }
                    run.pending_setup.remove(pos);
                    if run.pending_setup.is_empty() && !run.prompt_sent {
                        host.send_rpc(&json!({"id": "p1", "type": "prompt", "message": run.expect.initial_user_text}))
                            .await?;
                        run.prompt_sent = true;
                        self.record(
                            "prompt_sent",
                            json!({"slot": run.slot_id, "prompt_sha256": sha256_hex(run.expect.initial_user_text.as_bytes())}),
                        )?;
                    }
                } else if id == "p1" {
                    if !success || command != "prompt" {
                        return Err(CaptureFault::new(FaultClass::Bootstrap, "prompt_refused"));
                    }
                } else if id == "a1" {
                } else {
                    return Err(CaptureFault::new(
                        FaultClass::Protocol,
                        "rpc_response_unexpected",
                    ));
                }
            }
            E::Rpc(R::PromptResult { id, status }) => {
                if id.as_deref() != Some("p1") {
                    return Err(CaptureFault::new(
                        FaultClass::Protocol,
                        "rpc_prompt_result_unexpected",
                    ));
                }
                self.record(
                    "prompt_result",
                    json!({"slot": run.slot_id, "status": status}),
                )?;
                run.prompt_status = Some(status);
            }
            E::Rpc(R::SessionSettled) | E::Rpc(R::AgentEnd) | E::Rpc(R::Other) => {}
            E::Rpc(R::Forbidden(code)) => return Err(CaptureFault::new(halt_class(code), code)),
            E::RpcFault(code) => return Err(CaptureFault::new(FaultClass::Bootstrap, code)),
            E::RpcEof => {
                if run.prompt_status.is_none() {
                    return Err(CaptureFault::new(
                        FaultClass::Bootstrap,
                        "host_exited_early",
                    ));
                }
            }
            E::BridgeFault(f) => return Err(CaptureFault::new(FaultClass::Protocol, f.0)),
            E::BridgeEof => {
                if run.prompt_status.is_none() {
                    return Err(CaptureFault::new(
                        FaultClass::Protocol,
                        "bridge_channel_eof",
                    ));
                }
            }
            E::BridgeWriteFailed(code) => {
                return Err(CaptureFault::new(FaultClass::Protocol, code));
            }
            E::Bridge(inbound) => self.on_bridge(host, run, inbound, slot_deadline).await?,
        }
        Ok(())
    }

    async fn maybe_setup(
        &mut self,
        host: &host::Host,
        run: &mut SlotRun,
    ) -> Result<(), CaptureFault> {
        if run.rpc_ready && run.bridge_ready && !run.setup_sent {
            run.setup_sent = true;
            host.send_rpc(&json!({"id": "c1", "type": "set_auto_retry", "enabled": false}))
                .await?;
            host.send_rpc(&json!({"id": "c2", "type": "set_auto_compaction", "enabled": false}))
                .await?;
            host.send_rpc(&json!({"id": "c3", "type": "set_cache_warming", "mode": "off"}))
                .await?;
            host.send_rpc(&json!({"id": "c4", "type": "get_state"}))
                .await?;
        }
        Ok(())
    }

    async fn on_bridge(
        &mut self,
        host: &host::Host,
        run: &mut SlotRun,
        inbound: protocol::Inbound,
        slot_deadline: tokio::time::Instant,
    ) -> Result<(), CaptureFault> {
        use protocol::ChildMessage as C;
        match inbound.msg {
            C::Ready {
                api,
                delegate,
                model,
                thinking_level,
                active_tools,
            } => {
                if run.bridge_ready {
                    return Err(CaptureFault::new(
                        FaultClass::Protocol,
                        "bridge_ready_repeated",
                    ));
                }
                let mut tools = active_tools.clone();
                tools.sort();
                let ok = api == policy::BRIDGE_API
                    && delegate == "function"
                    && model.provider == policy::PROVIDER
                    && model.id == policy::MODEL
                    && model.api == policy::BRIDGE_API
                    && model.max_tokens == policy::MAX_OUTPUT_TOKENS
                    && thinking_level == policy::THINKING_LEVEL
                    && tools == ["edit", "read"];
                if !ok {
                    return Err(CaptureFault::new(
                        FaultClass::Bootstrap,
                        "bridge_ready_not_frozen",
                    ));
                }
                run.bridge_ready = true;
                self.record(
                    "bridge_ready",
                    json!({"slot": run.slot_id, "host": run.host_id}),
                )?;
                self.maybe_setup(host, run).await?;
            }
            C::FetchBegin {
                invocation,
                fetch,
                method,
                url,
                headers,
            } => {
                if !run.prompt_sent {
                    return Err(CaptureFault::new(
                        FaultClass::Protocol,
                        "fetch_before_prompt",
                    ));
                }
                self.on_fetch(
                    host,
                    run,
                    invocation,
                    fetch,
                    &method,
                    &url,
                    &headers,
                    &inbound.payload,
                    slot_deadline,
                )
                .await?;
            }
            C::FetchCancel { fetch, reason } => {
                if let Some(active) = run.active.as_mut().filter(|a| a.fetch == fetch) {
                    // The native client cancels the body right after
                    // `message_stop`; a cancel is never refund or completion
                    // evidence, so Rust keeps reading upstream to a verified
                    // terminal and settles from its own observation.
                    if !active.consumer_cancelled {
                        active.consumer_cancelled = true;
                        let reason = match reason {
                            protocol::CancelReason::Aborted => "aborted",
                            protocol::CancelReason::StreamCancelled => "stream_cancelled",
                        };
                        self.record(
                            "consumer_cancelled",
                            json!({"slot": run.slot_id, "fetch": fetch, "reason": reason}),
                        )?;
                    }
                } else if !run.settled_fetches.contains(&fetch)
                    && !run.seen_fetches.contains(&fetch)
                {
                    return Err(CaptureFault::new(
                        FaultClass::Protocol,
                        "fetch_cancel_unknown",
                    ));
                }
            }
            C::ToolCheck { call, name } => {
                if run.tools.iter().any(|t| t.call == call) {
                    return Err(CaptureFault::new(
                        FaultClass::Protocol,
                        "tool_call_repeated",
                    ));
                }
                let verdict = host::admit_tool(&run.tool_policy, &name, &inbound.payload);
                let (admitted, refusal) = match verdict {
                    host::ToolVerdict::Allow => (true, None),
                    host::ToolVerdict::Refuse(code) => (false, Some(code.to_string())),
                };
                self.record(
                    "tool_check",
                    json!({"slot": run.slot_id, "call": call, "name": name, "admitted": admitted, "refusal": refusal}),
                )?;
                host.send_bridge(
                    ParentMessage::ToolDecision {
                        call: call.clone(),
                        allow: admitted,
                    },
                    Vec::new(),
                )
                .await?;
                // A refused call is complete as soon as Rust refuses it: the
                // host synthesizes its blocked result without a tool_result
                // hook, and the followup must carry exactly that result.
                if !admitted {
                    run.completed_calls.push(call.clone());
                }
                run.tools.push(ToolRecord {
                    call,
                    name,
                    admitted,
                    refusal,
                    outcome: (!admitted).then(|| "blocked".to_string()),
                    result_sha256: None,
                    result_bytes: None,
                });
            }
            C::ToolDone {
                call,
                outcome,
                result_sha256,
                result_bytes,
            } => {
                let outcome_text = match outcome {
                    protocol::ToolOutcome::Ok => "ok",
                    protocol::ToolOutcome::Error => "error",
                    protocol::ToolOutcome::Blocked => "blocked",
                };
                self.record(
                    "tool_done",
                    json!({"slot": run.slot_id, "call": call, "outcome": outcome_text,
                           "result_sha256": result_sha256, "result_bytes": result_bytes}),
                )?;
                match run.tools.iter_mut().find(|t| t.call == call) {
                    Some(t) => {
                        if t.admitted == (outcome == protocol::ToolOutcome::Blocked) {
                            return Err(CaptureFault::new(
                                FaultClass::Containment,
                                "tool_outcome_contradicts_decision",
                            ));
                        }
                        if t.result_sha256.is_some() || (t.admitted && t.outcome.is_some()) {
                            return Err(CaptureFault::new(
                                FaultClass::Protocol,
                                "tool_done_repeated",
                            ));
                        }
                        let already_complete = !t.admitted;
                        t.outcome = Some(outcome_text.into());
                        t.result_sha256 = Some(result_sha256);
                        t.result_bytes = Some(result_bytes);
                        if already_complete {
                            return Ok(());
                        }
                    }
                    // No admission check preceded this call. Every registered
                    // tool passes the admission hook, so only a host-registry
                    // refusal of an unregistered tool may end this way, and
                    // only unsuccessfully; a successful result is an
                    // unadmitted execution. (Effects are also checked against
                    // the workspace after the host stops.)
                    None if outcome == protocol::ToolOutcome::Ok => {
                        return Err(CaptureFault::new(
                            FaultClass::Containment,
                            "tool_executed_without_admission",
                        ));
                    }
                    None => run.tools.push(ToolRecord {
                        call: call.clone(),
                        name: String::new(),
                        admitted: false,
                        refusal: Some("host_registry".into()),
                        outcome: Some(outcome_text.into()),
                        result_sha256: Some(result_sha256),
                        result_bytes: Some(result_bytes),
                    }),
                }
                run.completed_calls.push(call);
            }
            C::Fault { code } => {
                return Err(CaptureFault::new(
                    FaultClass::Bootstrap,
                    bridge_fault_code(code),
                ));
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn on_fetch(
        &mut self,
        host: &host::Host,
        run: &mut SlotRun,
        invocation: u32,
        fetch: u32,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: &[u8],
        slot_deadline: tokio::time::Instant,
    ) -> Result<(), CaptureFault> {
        let refuse = |class: FaultClass| ParentMessage::ResponseError { fetch, class };
        run.seen_fetches.push(fetch);
        // At most one generation in flight: a previous generation is
        // settled from Rust's own upstream observation before the next fetch
        // is considered (the native side may move on as soon as it saw
        // `message_stop`).
        if run.active.is_some() {
            self.drain_active(host, run).await?;
        }
        // One fetch per native invocation.
        if invocation <= run.last_invocation {
            let _ = host
                .send_bridge(refuse(FaultClass::Retry), Vec::new())
                .await;
            self.record(
                "retry_refused",
                json!({"slot": run.slot_id, "invocation": invocation, "fetch": fetch}),
            )?;
            return Err(CaptureFault::new(
                FaultClass::Retry,
                "retry_within_invocation",
            ));
        }
        if invocation != run.last_invocation + 1 {
            let _ = host
                .send_bridge(refuse(FaultClass::Protocol), Vec::new())
                .await;
            return Err(CaptureFault::new(
                FaultClass::Protocol,
                "invocation_out_of_order",
            ));
        }
        run.last_invocation = invocation;
        let prior = if invocation > 1 {
            match run.prior.as_mut() {
                Some(p) => {
                    p.completed_calls = run.completed_calls.clone();
                    Some(p.clone())
                }
                None => {
                    let _ = host
                        .send_bridge(refuse(FaultClass::Retry), Vec::new())
                        .await;
                    return Err(CaptureFault::new(
                        FaultClass::Retry,
                        "followup_without_settled_generation",
                    ));
                }
            }
        } else {
            None
        };
        let validated = match wire::validate_request(
            method,
            url,
            headers,
            body,
            &run.expect,
            prior.as_ref(),
        ) {
            Ok(v) => v,
            Err(f) => {
                let _ = host.send_bridge(refuse(f.class), Vec::new()).await;
                self.record(
                    "request_refused",
                    json!({"slot": run.slot_id, "invocation": invocation, "fetch": fetch,
                           "class": f.class.as_str(), "code": f.code, "body_sha256": sha256_hex(body)}),
                )?;
                return Err(f);
            }
        };
        self.record(
            "request_validated",
            json!({"slot": run.slot_id, "invocation": invocation, "fetch": fetch,
                   "body_sha256": validated.body_sha256, "body_len": validated.body_len,
                   "system_sha256": validated.system_sha256, "tools_sha256": validated.tools_sha256,
                   "settings_sha256": validated.settings_sha256, "stable_prefix_sha256": validated.stable_prefix_sha256,
                   "initial_user_sha256": validated.initial_user_sha256,
                   "volatile_reminder_sha256": validated.volatile_reminder_sha256,
                   "cache_layout": validated.cache_layout, "beta": validated.beta,
                   "count_projection_sha256": validated.count_projection_sha256,
                   "messages": validated.message_count, "followup": validated.followup}),
        )?;
        if run.render.is_none() {
            run.render = Some(RenderBinding::from_validated(&run.slot_id, &validated));
        }
        // Undeclared prefix or layout differences stop admission (§4): the
        // first request must match the slot's frozen render exactly.
        if !validated.followup
            && let Some(frozen) = &run.frozen_render
        {
            let actual = RenderCommitments {
                system_sha256: validated.system_sha256.clone(),
                tools_sha256: validated.tools_sha256.clone(),
                settings_sha256: validated.settings_sha256.clone(),
                stable_prefix_sha256: validated.stable_prefix_sha256.clone(),
                beta: validated.beta.clone(),
                cache_layout: validated.cache_layout.clone(),
                initial_user_sha256: validated.initial_user_sha256.clone(),
            };
            if &actual != frozen {
                let _ = host
                    .send_bridge(refuse(FaultClass::RequestBinding), Vec::new())
                    .await;
                return Err(CaptureFault::new(
                    FaultClass::RequestBinding,
                    "render_not_frozen",
                ));
            }
        }
        if run.purpose == Purpose::Preview {
            let _ = host
                .send_bridge(refuse(FaultClass::Authority), Vec::new())
                .await;
            run.preview_done = true;
            return Ok(());
        }

        // Slot-local limits close the slot as partial; global limits halt.
        let slot_limit = if run.ledger.permits >= run.ledger.permit_cap {
            Some("slot_permits_exhausted")
        } else if !reservation_fits(
            run.ledger.spent,
            run.ledger.outstanding,
            run.ledger.envelope,
        )
        .unwrap_or(false)
        {
            Some("slot_budget_exhausted")
        } else if tokio::time::Instant::now() >= slot_deadline {
            Some("slot_wall_exhausted")
        } else {
            None
        };
        if let Some(reason) = slot_limit {
            let _ = host
                .send_bridge(refuse(FaultClass::SlotLimit), Vec::new())
                .await;
            self.record(
                "slot_limit",
                json!({"slot": run.slot_id, "reason": reason, "fetch": fetch}),
            )?;
            if run.slot_id == policy::PROBE_SLOT {
                return Err(CaptureFault::new(FaultClass::SlotLimit, reason));
            }
            run.partial = Some(reason.into());
            return Ok(());
        }
        let c = &self.counters;
        let global = if c.generation_permits >= policy::MAX_GENERATION_PERMITS {
            Some("pilot_permits_exhausted")
        } else if c.count_calls >= policy::MAX_COUNT_CALLS {
            Some("pilot_count_calls_exhausted")
        } else if c.provider_http_attempts.saturating_add(2) > policy::MAX_PROVIDER_HTTP_ATTEMPTS {
            Some("pilot_http_attempts_exhausted")
        } else if !reservation_fits(
            c.spent_micro_usd,
            c.outstanding_micro_usd,
            policy::PILOT_ENVELOPE_MICRO_USD,
        )
        .unwrap_or(false)
        {
            Some("pilot_budget_exhausted")
        } else {
            None
        };
        if let Some(code) = global {
            let _ = host
                .send_bridge(refuse(FaultClass::SlotLimit), Vec::new())
                .await;
            return Err(CaptureFault::new(FaultClass::SlotLimit, code));
        }
        let now_ns = continuous_nanos()?;
        if let Some(deadline) = self.paid_deadline_ns
            && now_ns >= deadline
        {
            let _ = host
                .send_bridge(refuse(FaultClass::Deadline), Vec::new())
                .await;
            return Err(CaptureFault::new(
                FaultClass::Deadline,
                "paid_wall_exhausted",
            ));
        }

        // Permit, reservation and attempt identity are durable before any
        // provider byte is sent.
        let permit = self.counters.generation_permits + 1;
        let count_attempt = self.next_attempt()?;
        let reserve = |spent: u64, out: u64| {
            checked(out, policy::FULL_RESERVATION_MICRO_USD).map(|o| (spent, o))
        };
        let (_, slot_out) = reserve(run.ledger.spent, run.ledger.outstanding)?;
        let (_, pilot_out) = reserve(
            self.counters.spent_micro_usd,
            self.counters.outstanding_micro_usd,
        )?;
        if self.paid_started_ns.is_none() {
            let deadline = checked(now_ns, policy::PAID_PHASE_WALL_SECS * 1_000_000_000)?;
            if let Err(f) = self.record(
                "paid_phase_started",
                json!({"started_ns": now_ns, "deadline_ns": deadline, "boot_identity": self.boot_identity}),
            ) {
                let _ = host.send_bridge(refuse(FaultClass::Persistence), Vec::new()).await;
                return Err(f);
            }
            self.paid_started_ns = Some(now_ns);
            self.paid_deadline_ns = Some(deadline);
        }
        if let Err(f) = self.record(
            "reservation",
            json!({"slot": run.slot_id, "permit": permit, "invocation": invocation, "fetch": fetch,
                   "reserved_micro_usd": policy::FULL_RESERVATION_MICRO_USD,
                   "slot_outstanding": slot_out, "pilot_outstanding": pilot_out}),
        ) {
            let _ = host
                .send_bridge(refuse(FaultClass::Persistence), Vec::new())
                .await;
            return Err(f);
        }
        run.ledger.permits += 1;
        run.ledger.outstanding = slot_out;
        self.counters.generation_permits = permit;
        self.counters.outstanding_micro_usd = pilot_out;
        let paid_deadline = self.paid_deadline_ns.unwrap_or(now_ns);
        let remaining_paid = Duration::from_nanos(paid_deadline.saturating_sub(now_ns));
        let gen_deadline = (tokio::time::Instant::now()
            + Duration::from_secs(policy::GENERATION_DEADLINE_SECS))
        .min(slot_deadline)
        .min(tokio::time::Instant::now() + remaining_paid);
        let mut record = GenerationRecord {
            permit,
            invocation,
            fetch,
            body_sha256: validated.body_sha256.clone(),
            count_projection_sha256: validated.count_projection_sha256.clone(),
            followup: validated.followup,
            count_attempt: Some(count_attempt),
            count_status: None,
            count_request_id: None,
            estimate_lexeme: None,
            messages_attempt: None,
            messages_status: None,
            request_id: None,
            message_id: None,
            returned_model: None,
            served_service_tier: None,
            stop_reason: None,
            terminal: false,
            usage: None,
            tool_uses: Vec::new(),
            cost_micro_usd: None,
            consumer_cancelled: false,
            outcome: "counting".into(),
        };

        if let Err(f) = self.record(
            "http_begin",
            json!({"attempt": count_attempt, "route": "count", "permit": permit}),
        ) {
            // Nothing was transmitted: the provisional reservation is released.
            self.release(run, &mut record, "persistence_before_count")?;
            run.generations.push(record);
            let _ = host
                .send_bridge(refuse(FaultClass::Persistence), Vec::new())
                .await;
            return Err(f);
        }
        self.counters.count_calls += 1;
        self.counters.provider_http_attempts += 1;
        let transport = self
            .transport
            .as_ref()
            .ok_or(authority("transport_unavailable"))?;
        let count = transport
            .count(&validated.count_projection, &validated.beta, gen_deadline)
            .await;
        // From here on every outcome is recorded in the capture before any
        // fallible step; journal failures after a send enter the halt.
        let count = match count {
            Ok(c) => c,
            Err(fail) => {
                record.count_status = fail.status;
                record.count_request_id = fail.request_id.clone();
                self.record_or_halt(
                    "http_end",
                    json!({"attempt": count_attempt, "route": "count", "ok": false, "status": fail.status,
                           "request_id": fail.request_id, "class": fail.fault.class.as_str(), "code": fail.fault.code}),
                );
                // No generation byte was sent: the provisional reservation
                // is released; the consumed permit is not restored.
                self.release(run, &mut record, "count_failed")?;
                run.generations.push(record);
                let _ = host
                    .send_bridge(refuse(FaultClass::Counting), Vec::new())
                    .await;
                return Err(CaptureFault::new(FaultClass::Counting, fail.fault.code));
            }
        };
        record.count_status = Some(count.status);
        record.count_request_id = count.request_id.clone();
        record.estimate_lexeme = Some(count.input_tokens_lexeme.clone());
        self.record_or_halt(
            "http_end",
            json!({"attempt": count_attempt, "route": "count", "ok": true, "status": count.status,
                   "request_id": count.request_id, "input_tokens": count.input_tokens_lexeme}),
        );
        if let Some(f) = self.halt {
            self.release(run, &mut record, "persistence_after_count")?;
            run.generations.push(record);
            let _ = host
                .send_bridge(refuse(FaultClass::Persistence), Vec::new())
                .await;
            return Err(f);
        }
        if count.input_tokens > policy::INPUT_ADMISSION_THRESHOLD {
            self.release(run, &mut record, "input_cap_refusal")?;
            run.generations.push(record);
            let _ = host
                .send_bridge(refuse(FaultClass::SlotLimit), Vec::new())
                .await;
            self.record_or_halt(
                "slot_limit",
                json!({"slot": run.slot_id, "reason": "input_cap_refusal", "fetch": fetch}),
            );
            if run.slot_id == policy::PROBE_SLOT {
                return Err(CaptureFault::new(
                    FaultClass::Counting,
                    "probe_input_cap_refusal",
                ));
            }
            run.partial = Some("input_cap_refusal".into());
            return Ok(());
        }

        let messages_attempt = self.next_attempt()?;
        record.messages_attempt = Some(messages_attempt);
        if let Err(f) = self.record(
            "http_begin",
            json!({"attempt": messages_attempt, "route": "messages", "permit": permit}),
        ) {
            record.messages_attempt = None;
            self.release(run, &mut record, "persistence_before_messages")?;
            run.generations.push(record);
            let _ = host
                .send_bridge(refuse(FaultClass::Persistence), Vec::new())
                .await;
            return Err(f);
        }
        self.counters.messages_attempts += 1;
        self.counters.provider_http_attempts += 1;
        let transport = self
            .transport
            .as_ref()
            .ok_or(authority("transport_unavailable"))?;
        match transport
            .messages(body, &validated.beta, gen_deadline)
            .await
        {
            Err(fail) => {
                record.messages_status = fail.status;
                record.request_id = fail.request_id.clone();
                record.outcome = "messages_failed".into();
                run.generations.push(record);
                self.record_or_halt(
                    "http_end",
                    json!({"attempt": messages_attempt, "route": "messages", "ok": false, "status": fail.status,
                           "request_id": fail.request_id, "class": fail.fault.class.as_str(), "code": fail.fault.code,
                           "reservation": "retained"}),
                );
                let _ = host
                    .send_bridge(refuse(FaultClass::Transport), Vec::new())
                    .await;
                Err(fail.fault)
            }
            Ok(stream) => {
                record.messages_status = Some(stream.status());
                record.request_id = stream.request_id().map(str::to_string);
                record.outcome = "streaming".into();
                let status = stream.status();
                let request_id = stream.request_id().map(str::to_string);
                let headers = stream.relay_headers();
                run.generations.push(record);
                run.active = Some(Active {
                    fetch,
                    record_index: run.generations.len() - 1,
                    stream,
                    deadline: gen_deadline,
                    validated,
                    consumer_cancelled: false,
                });
                self.record_or_halt(
                    "http_response",
                    json!({"attempt": messages_attempt, "route": "messages", "status": status,
                           "request_id": request_id}),
                );
                if let Some(f) = self.halt {
                    // The shutdown path keeps the active observation and its
                    // full reservation.
                    return Err(f);
                }
                host.send_bridge(
                    ParentMessage::ResponseStart {
                        fetch,
                        status,
                        headers,
                    },
                    Vec::new(),
                )
                .await
            }
        }
    }

    /// Read the active generation to its end (bounded by its own deadline)
    /// and settle it.
    async fn drain_active(
        &mut self,
        host: &host::Host,
        run: &mut SlotRun,
    ) -> Result<(), CaptureFault> {
        while let Some(active) = run.active.as_mut() {
            let deadline = active.deadline;
            let next = active.stream.next_event(deadline).await;
            self.on_stream(host, run, next).await?;
        }
        Ok(())
    }

    /// Release an unspent provisional reservation (no generation byte sent).
    fn release(
        &mut self,
        run: &mut SlotRun,
        record: &mut GenerationRecord,
        outcome: &str,
    ) -> Result<(), CaptureFault> {
        run.ledger.outstanding = run
            .ledger
            .outstanding
            .checked_sub(policy::FULL_RESERVATION_MICRO_USD)
            .ok_or(CaptureFault::new(
                FaultClass::Usage,
                "reservation_underflow",
            ))?;
        self.counters.outstanding_micro_usd = self
            .counters
            .outstanding_micro_usd
            .checked_sub(policy::FULL_RESERVATION_MICRO_USD)
            .ok_or(CaptureFault::new(
                FaultClass::Usage,
                "reservation_underflow",
            ))?;
        record.outcome = outcome.into();
        self.record_or_halt(
            "reservation_released",
            json!({"slot": run.slot_id, "permit": record.permit, "reason": outcome,
                   "pilot_outstanding": self.counters.outstanding_micro_usd}),
        );
        Ok(())
    }

    async fn on_stream(
        &mut self,
        host: &host::Host,
        run: &mut SlotRun,
        next: Result<Option<bytes::Bytes>, CaptureFault>,
    ) -> Result<(), CaptureFault> {
        let fetch = run.active.as_ref().map(|a| a.fetch).unwrap_or(0);
        match next {
            Ok(Some(event)) => {
                if !run
                    .active
                    .as_ref()
                    .map(|a| a.consumer_cancelled)
                    .unwrap_or(true)
                {
                    host.send_bridge(ParentMessage::ResponseData { fetch }, event.to_vec())
                        .await?;
                }
                Ok(())
            }
            Ok(None) => {
                let active = run
                    .active
                    .take()
                    .ok_or(CaptureFault::new(FaultClass::Protocol, "stream_state"))?;
                let obs = active.stream.observation();
                self.settle(host, run, active, obs).await
            }
            Err(f) => {
                let active = run
                    .active
                    .take()
                    .ok_or(CaptureFault::new(FaultClass::Protocol, "stream_state"))?;
                let obs = active.stream.observation();
                let record = &mut run.generations[active.record_index];
                fill_observation(record, &obs);
                record.outcome = "stream_failed".into();
                self.record(
                    "generation_observed",
                    json!({"slot": run.slot_id, "permit": record.permit, "outcome": "stream_failed",
                           "class": f.class.as_str(), "code": f.code, "message_id": obs.message_id,
                           "request_id": obs.request_id, "sse_bytes": obs.sse_bytes, "events": obs.events,
                           "usage": record.usage, "reservation": "retained"}),
                )?;
                let _ = host
                    .send_bridge(
                        ParentMessage::ResponseError {
                            fetch,
                            class: f.class,
                        },
                        Vec::new(),
                    )
                    .await;
                Err(f)
            }
        }
    }

    async fn settle(
        &mut self,
        host: &host::Host,
        run: &mut SlotRun,
        active: Active,
        obs: wire::Observation,
    ) -> Result<(), CaptureFault> {
        let fetch = active.fetch;
        let record_index = active.record_index;
        {
            let record = &mut run.generations[record_index];
            fill_observation(record, &obs);
            record.consumer_cancelled = active.consumer_cancelled;
        }
        let usage = &obs.usage;
        let fail = |code| CaptureFault::new(FaultClass::Usage, code);
        let verdict: Result<(u64, u64, u64), CaptureFault> = (|| {
            if !obs.terminal {
                return Err(CaptureFault::new(
                    FaultClass::Transport,
                    "stream_not_terminal",
                ));
            }
            if obs.returned_model.as_deref() != Some(policy::MODEL) {
                return Err(CaptureFault::new(
                    FaultClass::RequestBinding,
                    "returned_model_mismatch",
                ));
            }
            if obs.message_id.is_none() {
                return Err(fail("message_id_missing"));
            }
            if let Some(tier) = obs.served_service_tier.as_deref()
                && tier != "standard"
            {
                return Err(CaptureFault::new(
                    FaultClass::RequestBinding,
                    "served_service_tier_not_standard",
                ));
            }
            if !usage.complete() {
                return Err(fail("usage_incomplete"));
            }
            let input = usage.inclusive_input().ok_or(fail("usage_incomplete"))?;
            let output = usage
                .output
                .as_ref()
                .map(|c| c.value)
                .ok_or(fail("usage_incomplete"))?;
            let cost = budget_cost_micro(input, output).ok_or(fail("arithmetic_overflow"))?;
            Ok((input, output, cost))
        })();
        let (input, output, cost) = match verdict {
            Ok(v) => v,
            Err(f) => {
                let record = &mut run.generations[record_index];
                record.outcome = "unsettled".into();
                self.record(
                    "generation_observed",
                    json!({"slot": run.slot_id, "permit": record.permit, "outcome": "unsettled",
                           "class": f.class.as_str(), "code": f.code, "message_id": obs.message_id,
                           "usage": record.usage, "reservation": "retained"}),
                )?;
                let _ = host
                    .send_bridge(
                        ParentMessage::ResponseError {
                            fetch,
                            class: f.class,
                        },
                        Vec::new(),
                    )
                    .await;
                return Err(f);
            }
        };
        // Reconcile once: replace this attempt's full reservation by its
        // observed budget-model cost.
        let slot_out = run
            .ledger
            .outstanding
            .checked_sub(policy::FULL_RESERVATION_MICRO_USD)
            .ok_or(fail("reservation_underflow"))?;
        let pilot_out = self
            .counters
            .outstanding_micro_usd
            .checked_sub(policy::FULL_RESERVATION_MICRO_USD)
            .ok_or(fail("reservation_underflow"))?;
        run.ledger.outstanding = slot_out;
        run.ledger.spent = checked(run.ledger.spent, cost)?;
        self.counters.outstanding_micro_usd = pilot_out;
        self.counters.spent_micro_usd = checked(self.counters.spent_micro_usd, cost)?;
        let breach = input > policy::INPUT_RESERVATION || output > policy::MAX_OUTPUT_TOKENS;
        {
            let record = &mut run.generations[record_index];
            record.cost_micro_usd = Some(cost);
            record.outcome = if breach {
                "provider_limit_breach".into()
            } else {
                "settled".into()
            };
        }
        let record = run.generations[record_index].clone();
        self.record(
            "reconciled",
            json!({"slot": run.slot_id, "permit": record.permit, "message_id": record.message_id,
                   "request_id": record.request_id, "stop_reason": record.stop_reason, "usage": record.usage,
                   "cost_micro_usd": cost, "released_micro_usd": policy::FULL_RESERVATION_MICRO_USD.saturating_sub(cost),
                   "slot_spent": run.ledger.spent, "pilot_spent": self.counters.spent_micro_usd,
                   "pilot_outstanding": self.counters.outstanding_micro_usd, "tool_uses": record.tool_uses.len()}),
        )?;
        if breach {
            let _ = host
                .send_bridge(
                    ParentMessage::ResponseError {
                        fetch,
                        class: FaultClass::ProviderLimitBreach,
                    },
                    Vec::new(),
                )
                .await;
            return Err(CaptureFault::new(
                FaultClass::ProviderLimitBreach,
                "provider_limit_breach",
            ));
        }
        host.send_bridge(ParentMessage::ResponseEnd { fetch }, Vec::new())
            .await?;
        run.settled_fetches.push(fetch);
        run.prior = Some(wire::PriorGeneration {
            messages: active.validated.messages.clone(),
            system_sha256: active.validated.system_sha256.clone(),
            tools_sha256: active.validated.tools_sha256.clone(),
            settings_sha256: active.validated.settings_sha256.clone(),
            beta: active.validated.beta.clone(),
            tool_uses: obs.tool_uses.clone(),
            completed_calls: Vec::new(),
        });
        // Completed calls are not cleared: tool checks and completions for
        // this generation's tool uses can arrive before its upstream EOF.
        // Call ids are unique within a slot, and the followup check binds
        // the result sequence to these exact tool uses.
        Ok(())
    }

    fn seal_slot(
        &mut self,
        capture: &SlotCapture,
        receipt: &IndependentCheckReceipt,
    ) -> Result<SealedSlot, CaptureFault> {
        let prov = |code| CaptureFault::new(FaultClass::CheckerProvenance, code);
        let awaiting = self
            .awaiting_seal
            .as_ref()
            .ok_or(authority("no_slot_awaiting_seal"))?;
        if awaiting.slot_id != capture.slot_id
            || awaiting.host_id != capture.host_id
            || awaiting.candidate_inventory != capture.candidate_inventory
        {
            return Err(authority("seal_capture_mismatch"));
        }
        let result = (|| -> Result<CheckOutcome, CaptureFault> {
            if receipt.slot_id != capture.slot_id {
                return Err(prov("check_slot_mismatch"));
            }
            if receipt.fixture && self.mode == Mode::Live {
                return Err(prov("fixture_check_not_admissible"));
            }
            if receipt.candidate != capture.candidate_inventory || receipt.candidate.is_empty() {
                return Err(prov("check_candidate_mismatch"));
            }
            if receipt.oracle_sha256 != policy::ORACLE_SHA256 {
                return Err(prov("check_oracle_mismatch"));
            }
            if receipt.compiler != Path::new(policy::RUSTC)
                || receipt.compiler_sha256 != policy::RUSTC_SHA256
                || !receipt
                    .compiler_version
                    .starts_with(policy::RUSTC_VERSION_PREFIX)
            {
                return Err(prov("check_compiler_mismatch"));
            }
            let root = receipt
                .checker_root
                .to_str()
                .filter(|r| receipt.checker_root.is_absolute() && !r.is_empty())
                .ok_or(prov("check_root_invalid"))?;
            let overlaps = |p: &Path| {
                receipt.checker_root.starts_with(p) || p.starts_with(&receipt.checker_root)
            };
            if overlaps(&self.run_dir)
                || self
                    .manifest
                    .iter()
                    .flat_map(|m| m.slots.iter())
                    .any(|s| overlaps(&s.workspace))
            {
                return Err(prov("check_root_not_private"));
            }
            if receipt.env_names.iter().any(|n| {
                let upper = n.to_ascii_uppercase();
                upper.contains("KEY")
                    || upper.contains("TOKEN")
                    || upper.contains("PROXY")
                    || upper.contains("ANTHROPIC")
            }) {
                return Err(prov("check_environment_not_minimal"));
            }
            if receipt.steps.is_empty() || receipt.steps.len() > policy::CHECK_RECIPE.len() {
                return Err(prov("check_steps_invalid"));
            }
            let mut wall: u64 = 0;
            let mut pass = receipt.steps.len() == policy::CHECK_RECIPE.len();
            for (i, step) in receipt.steps.iter().enumerate() {
                let (name, template) = policy::CHECK_RECIPE[i];
                let expected: Vec<String> =
                    template.iter().map(|a| a.replace("<ROOT>", root)).collect();
                if step.step != name || step.argv != expected {
                    return Err(prov("check_recipe_mismatch"));
                }
                if !protocol::is_sha256_hex(&step.stdout_sha256)
                    || !protocol::is_sha256_hex(&step.stderr_sha256)
                {
                    return Err(prov("check_output_unbound"));
                }
                if step
                    .artifacts
                    .iter()
                    .any(|(_, h)| !protocol::is_sha256_hex(h))
                {
                    return Err(prov("check_artifact_unbound"));
                }
                wall = wall.saturating_add(step.wall_ms);
                let ok = step.exit == Some(0) && step.signal.is_none() && !step.timed_out;
                if !ok {
                    pass = false;
                    if i + 1 != receipt.steps.len() {
                        return Err(prov("check_continued_after_failure"));
                    }
                }
            }
            if wall > policy::POST_CHECK_SECS * 1000 {
                return Err(prov("post_check_wall_exceeded"));
            }
            // A receipt may stop early only at a failing step; a shortened
            // all-success recipe is missing provenance, not a behaviour fail.
            let last_ok = receipt
                .steps
                .last()
                .is_some_and(|s| s.exit == Some(0) && s.signal.is_none() && !s.timed_out);
            if receipt.steps.len() < policy::CHECK_RECIPE.len() && last_ok {
                return Err(prov("check_steps_truncated"));
            }
            Ok(if pass {
                CheckOutcome::Pass
            } else {
                CheckOutcome::Fail
            })
        })();
        let receipt_sha256 =
            sha256_hex(&serde_json::to_vec(receipt).map_err(|_| prov("check_encode_failed"))?);
        let outcome = match result {
            Ok(o) => o,
            Err(f) => {
                self.record("check_refused", json!({"slot": capture.slot_id, "code": f.code, "receipt_sha256": receipt_sha256}))?;
                self.enter_halt(f);
                return Err(f);
            }
        };
        self.record(
            "slot_sealed",
            json!({"slot": capture.slot_id, "outcome": outcome, "receipt_sha256": receipt_sha256, "fixture": receipt.fixture}),
        )?;
        if let Some(s) = self.summaries.get_mut(self.slot_index) {
            s.state = "sealed".into();
            s.check = Some(outcome);
        }
        self.awaiting_seal = None;
        self.slot_index += 1;
        Ok(SealedSlot {
            slot_id: capture.slot_id.clone(),
            outcome,
            receipt_sha256,
            disposition: capture.disposition.clone(),
        })
    }

    fn seal_probe(&mut self, capture: &SlotCapture) -> Result<ProbeSeal, CaptureFault> {
        let refuse = |code| CaptureFault::new(FaultClass::Authority, code);
        if self.scope != Scope::Probe || capture.slot_id != policy::PROBE_SLOT || self.probe_sealed
        {
            return Err(refuse("not_a_probe_session"));
        }
        if self.halt.is_some() {
            return Err(refuse("probe_session_halted"));
        }
        let first = match capture.generations.as_slice() {
            [g] => g,
            _ => return Err(refuse("probe_generation_count")),
        };
        let usage_ok = first.usage.as_ref().map(|u| u.complete).unwrap_or(false);
        let native_ok = capture
            .native
            .as_ref()
            .map(|n| {
                n.joins.len() == 1
                    && n.joins[0].1.len() == 1
                    && n.unmatched_native_response_ids.is_empty()
            })
            .unwrap_or(false);
        let exit_ok = capture
            .host_exit
            .as_ref()
            .map(|e| e.normal && e.group_gone)
            .unwrap_or(false);
        let ok = matches!(capture.disposition, Disposition::Completed { .. })
            && capture.prompt_status.as_deref() == Some("completed")
            && first.outcome == "settled"
            && first.terminal
            && first.returned_model.as_deref() == Some(policy::MODEL)
            && first.tool_uses.is_empty()
            && capture.tools.is_empty()
            && usage_ok
            && native_ok
            && exit_ok
            && self.counters.outstanding_micro_usd == 0;
        if !ok {
            let f = refuse("probe_not_successful");
            self.enter_halt(f);
            return Err(f);
        }
        let message_id = first
            .message_id
            .clone()
            .ok_or(refuse("probe_not_successful"))?;
        let (started, deadline) = match (self.paid_started_ns, self.paid_deadline_ns) {
            (Some(s), Some(d)) => (s, d),
            _ => return Err(refuse("probe_not_successful")),
        };
        // The seal is the journal's terminal record: the handoff verifies
        // every sealed field against it and refuses any later record.
        self.record(
            "probe_sealed",
            json!({"message_id": message_id, "counters": self.counters,
                   "attempt_seq": self.attempt_seq, "host_seq": self.host_seq,
                   "paid_started_ns": started, "paid_deadline_ns": deadline,
                   "boot_identity": self.boot_identity, "run_dir": self.run_dir}),
        )?;
        let implementation_sha256 = std::env::current_exe()
            .ok()
            .and_then(|p| file_sha256(&p, "implementation_unidentified").ok())
            .unwrap_or_default();
        self.probe_sealed = true;
        Ok(ProbeSeal {
            kind: "bounded-pilot-v1-probe-seal".into(),
            mode: self.mode,
            contract_sha256: policy::CONTRACT_SHA256.into(),
            omp_sha256: policy::OMP_SHA256.into(),
            implementation_sha256,
            bridge_sha256: sha256_hex(host::BRIDGE_SOURCE),
            run_dir: self.run_dir.clone(),
            journal_sha256: self.journal.digest()?,
            journal_records: self.journal.records,
            counters: self.counters,
            paid_started_ns: started,
            paid_deadline_ns: deadline,
            boot_identity: self.boot_identity.clone(),
            attempt_seq: self.attempt_seq,
            host_seq: self.host_seq,
            probe_message_id: message_id,
            probe_request_id: first.request_id.clone(),
        })
    }

    fn finish(&mut self) -> Result<CaptureReceipt, CaptureFault> {
        let elapsed = match self.paid_started_ns {
            Some(s) => Some(continuous_nanos()?.saturating_sub(s) / 1_000_000),
            None => None,
        };
        let halt = self
            .halt
            .map(|f| (f.class.as_str().to_string(), f.code.to_string()));
        // A sealed probe's journal ends at its seal (the coding handoff
        // continues it); every other session records its close.
        if !self.probe_sealed {
            self.record(
            "finished",
            json!({"counters": self.counters, "halt": halt, "paid_elapsed_ms": elapsed,
                   "slots": self.summaries.iter().map(|s| (s.slot_id.clone(), s.state.clone())).collect::<Vec<_>>()}),
            )?;
        }
        self.transport = None;
        Ok(CaptureReceipt {
            mode: self.mode,
            scope: self.scope,
            counters: self.counters,
            slots: self.summaries.clone(),
            halt,
            paid_started: self.paid_started_ns.is_some(),
            paid_elapsed_ms: elapsed,
            journal_sha256: self.journal.digest()?,
            journal_records: self.journal.records,
        })
    }
}

fn fill_observation(record: &mut GenerationRecord, obs: &wire::Observation) {
    record.message_id = obs.message_id.clone();
    record.returned_model = obs.returned_model.clone();
    record.served_service_tier = obs.served_service_tier.clone();
    record.stop_reason = obs.stop_reason.clone();
    record.terminal = obs.terminal;
    record.usage = Some(UsageRecord::from_wire(&obs.usage));
    record.tool_uses = obs.tool_uses.iter().map(|t| t.name.clone()).collect();
    if record.request_id.is_none() {
        record.request_id = obs.request_id.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_reservation_arithmetic() {
        assert_eq!(policy::full_reservation(), 430_080);
        assert_eq!(budget_cost_micro(32_768, 4096), Some(430_080));
        assert_eq!(budget_cost_micro(u64::MAX, 1), None);
        // Two full reservations fit a coding slot; a third does not.
        assert_eq!(reservation_fits(0, 430_080, 1_000_000), Some(true));
        assert_eq!(reservation_fits(0, 860_160, 1_000_000), Some(false));
        // A reconciled cheap generation frees room for further permits.
        assert_eq!(reservation_fits(250_000, 0, 1_000_000), Some(true));
        assert_eq!(reservation_fits(570_000, 0, 1_000_000), Some(false));
        // Probe envelope admits exactly one full reservation.
        assert_eq!(reservation_fits(0, 0, 500_000), Some(true));
        assert_eq!(reservation_fits(u64::MAX, 1, 10_000_000), None);
    }

    #[test]
    fn host_display_paths_drop_only_macos_firmlink_prefixes() {
        let p = |s: &str| host_display_path(Path::new(s));
        assert_eq!(p("/Users/u/ws"), "/Users/u/ws");
        assert_eq!(p("/private/varnish/ws"), "/private/varnish/ws");
        if cfg!(target_os = "macos") {
            assert_eq!(p("/private/var/folders/x/ws"), "/var/folders/x/ws");
            assert_eq!(p("/private/tmp/ws"), "/tmp/ws");
        }
    }

    #[test]
    fn an_authority_receipt_admits_one_run_only() {
        let dir = tempfile::tempdir().unwrap();
        let receipt = dir.path().join("authority.json");
        fs::write(&receipt, b"{}").unwrap();
        claim_authority(&receipt, "abc", &dir.path().join("run-1")).unwrap();
        assert_eq!(
            claim_authority(&receipt, "abc", &dir.path().join("run-2"))
                .unwrap_err()
                .code,
            "authority_already_claimed"
        );
        let marker = dir.path().join("authority.json.claim");
        assert_eq!(fs::metadata(&marker).unwrap().mode() & 0o777, 0o600);
    }

    #[test]
    fn ratecards_parse_exactly() {
        assert_eq!(rate_micro("10"), Some(10_000_000));
        assert_eq!(rate_micro("2.5"), Some(2_500_000));
        assert_eq!(rate_micro("0.000001"), Some(1));
        assert_eq!(rate_micro("1.0000001"), None);
        assert_eq!(rate_micro("-1"), None);
        assert_eq!(rate_micro(""), None);
        assert_eq!(rate_micro("1e3"), None);
    }

    #[test]
    fn journal_chains_records_and_detects_tampering() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("j.jsonl");
        let mut j = Journal::create(&path, None).unwrap();
        j.append("a", json!({"x": 1})).unwrap();
        j.append("b", json!({"y": 2})).unwrap();
        let digest = j.digest().unwrap();
        let lines: Vec<Value> = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines[0]["prev"], "");
        assert_eq!(lines[1]["prev"].as_str().unwrap().len(), 64);
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        drop(j);
        let mut reopened = Journal::reopen(&path, &digest, 2).unwrap();
        reopened.append("c", json!({})).unwrap();
        assert!(Journal::reopen(&path, &digest, 3).is_err());
        let mut failing = Journal::create(&dir.path().join("k.jsonl"), Some(1)).unwrap();
        failing.append("a", json!({})).unwrap();
        assert_eq!(
            failing.append("b", json!({})).unwrap_err().code,
            "journal_append_failed"
        );
    }

    #[test]
    fn continuous_clock_and_boot_identity_are_available() {
        let a = continuous_nanos().unwrap();
        let b = continuous_nanos().unwrap();
        assert!(b >= a);
        assert!(!boot_identity().unwrap().is_empty());
    }
}
