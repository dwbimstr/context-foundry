//! Bounded pilot v1 apparatus around the capture session: pinned evidence
//! verification, seed restoration, the four pair views materialized with the
//! pinned release Foundry (contract §3), the contained independent checker
//! (§9), arm prompts and the frozen launch manifests (§12 gate 4).
//!
//! Every input is verified against the frozen pins before use; nothing here
//! makes a provider request, reads a credential or runs model-written code
//! outside the checker's sandbox. Private paths are inputs, never source.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::protocol::FaultClass;
use super::{
    CHECK_RECEIPT_KIND, CHECK_STDERR_MAX_BYTES, CaptureFault, CheckStep, IndependentCheckReceipt,
    LaunchManifest, ManifestSlot, ManifestView, PinEntry, PreparedSlot, PreviewAdmission,
    PrivateArtifact, PrivateTreeBudget, RenderCommitments, collect_private_tree,
    create_private_dir, open_private_file, policy, private_child, render_binding,
    require_private_directory, sha256_hex, verify_private_tree, write_private_new,
};

/// Frozen input pins (contract §2) beyond those in [`policy`].
pub mod pins {
    pub const FOUNDRY_RELEASE_SHA256: &str =
        "9dcbec8fdfff79d491410c82ab6e3ea922d3c19d5c9397f9d231fc08713c8614";
    pub const PROFILE_SHA256: &str =
        "cd3a5326579bc720f3ba1e6849be0e53ff9de3dc635eb51cd3212f4f13043e2a";
    pub const WORKER_SHA256: &str =
        "b0f7e02009ae31d622240469f9eddb4d580a170141f537e76e09e35ba31b0cb3";
    pub const GGUF_SHA256: &str =
        "68bae29d62fb8c7d23e98d21fd4662753ddd636e6b62b8a70dfe059a9844f216";
    pub const TOKENIZER_SHA256: &str =
        "4d777ef5bdc1aa36227abdfb77c3e49e7b9c892d16e1b6bda41c393504828be4";
    pub const TASK_SHA256: &str =
        "d634da1c6b74ab5bc247f4457109742213880e051f9fa68fe81f62de3ce26c31";
    pub const ARM_A_PROMPT_SHA256: &str =
        "7026176f92fd56e99055f7c899250f7da0e7fe65bd08b3fb159a17df3bd8d836";
    pub const ALLOCATION_SHA256: &str =
        "fd3add3fc280d4a512a3c4fe58983a001849bd0ffe007efa26b62809c11bbf4a";
    pub const PROTOCOL_SHA256: &str =
        "3e55bde1588953f40e6986b9d97c1404c31a2ec6cfdd5fe2d48bb038a6fcd3da";
    /// The five-file seed corpus: the control corpus with only `src/lib.rs`
    /// replaced by the pinned seed (contract §2).
    pub const SEED: [(&str, &str); 5] = [
        (
            "Cargo.toml",
            "e4ea043c47bbd5551fc5fcf8151eab94428b4be6f2337834a0f9020a4392fb19",
        ),
        (
            "src/lib.rs",
            "bb97304a4602082df298e81d6ac0b500e8d6d89a525d8585a16c267ef6c84ac5",
        ),
        (
            "src/ascii_set.rs",
            "f75552e4f446ed240cef64c1367e848cf3ae65f77602085459566d835a1d8587",
        ),
        (
            "LICENSE-MIT",
            "b38f11f6096706e6de553dabe2a7ed142d59b6fa8c97e290c67496154745cdd5",
        ),
        (
            "LICENSE-APACHE",
            "a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2",
        ),
    ];
    pub const CONTEXT_QUERY: &str = "preserve malformed percent escapes without dropping bytes in iteration and borrowed UTF-8 conversions";
    pub const CONTEXT_TOKENS: usize = 2048;
    /// The existing initial-user framing; arm B's view follows it verbatim.
    pub const EVIDENCE_FRAMING: &str = "\nInitial repository evidence:\n";
    pub const PAIR_PREPARATION_SECS: u64 = 180;
    pub const MATERIALIZATION_TOTAL_SECS: u64 = 900;
    pub const PAIRS: [u8; 4] = [1, 2, 3, 4];
}

fn fault(class: FaultClass, code: &'static str) -> CaptureFault {
    CaptureFault::new(class, code)
}

fn pin_fault(code: &'static str) -> CaptureFault {
    fault(FaultClass::Pins, code)
}

/// SHA-256 of a regular file (streamed).
pub fn file_sha256(path: &Path) -> Result<String, CaptureFault> {
    let meta = fs::metadata(path).map_err(|_| pin_fault("pinned_input_missing"))?;
    if !meta.is_file() {
        return Err(pin_fault("pinned_input_not_file"));
    }
    let mut f = fs::File::open(path).map_err(|_| pin_fault("pinned_input_unreadable"))?;
    let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|_| pin_fault("pinned_input_unreadable"))?;
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

fn require_pin(
    name: &str,
    path: &Path,
    expected: &str,
    out: &mut Vec<PinEntry>,
) -> Result<(), CaptureFault> {
    let actual = file_sha256(path)?;
    if actual != expected {
        return Err(pin_fault("pinned_input_changed"));
    }
    out.push(PinEntry {
        name: name.to_string(),
        path: path.to_path_buf(),
        sha256: actual,
    });
    Ok(())
}

/// The owner's private evidence roots and executables. Every path is
/// verified against the frozen pins before anything uses it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceInputs {
    /// `R`: the paired calibration root (task, seed, oracle, control corpus).
    pub paired_root: PathBuf,
    /// `F`: the pilot freeze root (allocation draw).
    pub freeze_root: PathBuf,
    pub foundry: PathBuf,
    pub profile: PathBuf,
    pub omp: PathBuf,
}

/// Verified inputs, ready for materialization, checks and manifests.
#[derive(Clone, Debug)]
pub struct Verified {
    pub inputs: EvidenceInputs,
    pub pins: Vec<PinEntry>,
    pub task: String,
    pub arm_a_prompt: String,
    pub oracle: PathBuf,
    pub seed: Vec<(String, PathBuf)>,
}

impl EvidenceInputs {
    fn seed_sources(&self) -> Vec<(String, PathBuf)> {
        let r = &self.paired_root;
        vec![
            ("Cargo.toml".into(), r.join("oracle/control/Cargo.toml")),
            ("src/lib.rs".into(), r.join("inputs/seed-lib.rs")),
            (
                "src/ascii_set.rs".into(),
                r.join("oracle/control/src/ascii_set.rs"),
            ),
            ("LICENSE-MIT".into(), r.join("oracle/control/LICENSE-MIT")),
            (
                "LICENSE-APACHE".into(),
                r.join("oracle/control/LICENSE-APACHE"),
            ),
        ]
    }

    /// Verify every pinned input, including the profile-declared embedding
    /// worker, model and tokenizer (contract §2: identity established before
    /// any admitted local preparation).
    pub fn verify(&self) -> Result<Verified, CaptureFault> {
        let mut out = Vec::new();
        require_pin(
            "foundry",
            &self.foundry,
            pins::FOUNDRY_RELEASE_SHA256,
            &mut out,
        )?;
        require_pin("profile", &self.profile, pins::PROFILE_SHA256, &mut out)?;
        require_pin("omp", &self.omp, policy::OMP_SHA256, &mut out)?;
        require_pin(
            "rustc",
            Path::new(policy::RUSTC),
            policy::RUSTC_SHA256,
            &mut out,
        )?;
        let profile: Value = serde_json::from_slice(
            &fs::read(&self.profile).map_err(|_| pin_fault("profile_unreadable"))?,
        )
        .map_err(|_| pin_fault("profile_invalid"))?;
        let bundle = profile
            .pointer("/worker/bundle")
            .and_then(Value::as_str)
            .ok_or(pin_fault("profile_invalid"))?;
        let model_dir = profile
            .get("model_dir")
            .and_then(Value::as_str)
            .ok_or(pin_fault("profile_invalid"))?;
        if profile
            .pointer("/worker/executable_sha256")
            .and_then(Value::as_str)
            != Some(pins::WORKER_SHA256)
        {
            return Err(pin_fault("profile_worker_pin_changed"));
        }
        let worker = Path::new(bundle).join("Contents/MacOS/foundry-embed");
        require_pin("embedding_worker", &worker, pins::WORKER_SHA256, &mut out)?;
        require_pin(
            "embedding_model",
            &Path::new(model_dir).join("embeddinggemma-2-BF16.gguf"),
            pins::GGUF_SHA256,
            &mut out,
        )?;
        require_pin(
            "embedding_tokenizer",
            &Path::new(model_dir).join("tokenizer.json"),
            pins::TOKENIZER_SHA256,
            &mut out,
        )?;
        let r = &self.paired_root;
        require_pin(
            "protocol",
            &r.join("protocol.json"),
            pins::PROTOCOL_SHA256,
            &mut out,
        )?;
        require_pin("task", &r.join("task.txt"), pins::TASK_SHA256, &mut out)?;
        require_pin(
            "arm_a_prompt",
            &r.join("arms/a/prompt.txt"),
            pins::ARM_A_PROMPT_SHA256,
            &mut out,
        )?;
        require_pin(
            "oracle",
            &r.join("oracle/check.rs"),
            policy::ORACLE_SHA256,
            &mut out,
        )?;
        require_pin(
            "allocation",
            &self.freeze_root.join("allocation.json"),
            pins::ALLOCATION_SHA256,
            &mut out,
        )?;
        let seed = self.seed_sources();
        for ((rel, src), (pin_rel, sha)) in seed.iter().zip(pins::SEED.iter()) {
            debug_assert_eq!(rel, pin_rel);
            require_pin(&format!("seed:{rel}"), src, sha, &mut out)?;
        }
        let task =
            fs::read_to_string(r.join("task.txt")).map_err(|_| pin_fault("task_unreadable"))?;
        let arm_a_prompt = fs::read_to_string(r.join("arms/a/prompt.txt"))
            .map_err(|_| pin_fault("prompt_unreadable"))?;
        if !arm_a_prompt.starts_with(&format!("{task}{}", pins::EVIDENCE_FRAMING)) {
            return Err(pin_fault("arm_a_prompt_not_framed"));
        }
        Ok(Verified {
            inputs: self.clone(),
            pins: out,
            task,
            arm_a_prompt,
            oracle: r.join("oracle/check.rs"),
            seed,
        })
    }
}

/// The private pilot root (0700) and its fixed layout.
#[derive(Clone, Debug)]
pub struct PilotRoot {
    pub root: PathBuf,
}

const ROOT_DIRS: [&str; 9] = [
    "seed",
    "workspaces",
    "stores",
    "views",
    "renders",
    "manifests",
    "runs",
    "checks",
    "archive",
];

impl PilotRoot {
    /// Claim a fresh pilot root exclusively.
    pub fn create(path: &Path) -> Result<Self, CaptureFault> {
        // Only a fresh leaf is claimed. Resolve its admitted existing parent,
        // never an existing input leaf, so ordinary /tmp aliases remain usable.
        let parent = fs::canonicalize(
            path.parent()
                .ok_or(fault(FaultClass::Persistence, "pilot_root_unresolvable"))?,
        )
        .map_err(|_| fault(FaultClass::Persistence, "pilot_root_unresolvable"))?;
        let target = parent.join(
            path.file_name()
                .ok_or(fault(FaultClass::Persistence, "pilot_root_unresolvable"))?,
        );
        create_private_dir(&target)?;
        let root = fs::canonicalize(&target)
            .map_err(|_| fault(FaultClass::Persistence, "pilot_root_unresolvable"))?;
        for d in ROOT_DIRS {
            create_private_dir(&root.join(d))?;
        }
        Ok(Self { root })
    }

    /// Open an existing private pilot root.
    pub fn open(path: &Path) -> Result<Self, CaptureFault> {
        let root = fs::canonicalize(path)
            .map_err(|_| fault(FaultClass::Authority, "pilot_root_missing"))?;
        require_private_directory(path)
            .map_err(|_| fault(FaultClass::Authority, "pilot_root_not_private"))?;
        let mut budget = PrivateTreeBudget::new(None);
        for d in ROOT_DIRS {
            let child = root.join(d);
            fs::symlink_metadata(&child)
                .map_err(|_| fault(FaultClass::Authority, "pilot_root_incomplete"))?;
            require_private_directory(&child)
                .map_err(|_| fault(FaultClass::Authority, "pilot_root_not_private"))?;
            if d == "checks" {
                // Derive only the fixed checks/<one-root>/out/check exception;
                // both inventory and validation consume this same pass budget.
                let files = collect_private_tree(&child, false, &[], &mut budget)?;
                let executables: Vec<&Path> = files
                    .iter()
                    .filter(|p| {
                        let parts: Vec<_> = p.components().collect();
                        parts.len() == 3
                            && parts[1].as_os_str() == "out"
                            && parts[2].as_os_str() == "check"
                    })
                    .map(PathBuf::as_path)
                    .collect();
                verify_private_tree(&child, &executables, &mut budget)?;
            } else if d == "runs" {
                // Fixed PilotRoot layout only: runs/<one-run>/hosts/<one-host>
                // followed by the host-owned exact native-module suffix.
                let files = collect_private_tree(&child, false, &[], &mut budget)?;
                let executables: Vec<&Path> = files
                    .iter()
                    .filter(|p| {
                        let mut parts = p.components();
                        matches!(parts.next(), Some(std::path::Component::Normal(_)))
                            && super::host::is_run_native_module(parts.as_path())
                    })
                    .map(PathBuf::as_path)
                    .collect();
                verify_private_tree(&child, &executables, &mut budget)?;
            } else {
                verify_private_tree(&child, &[], &mut budget)?;
            }
        }
        Ok(Self { root })
    }

    pub fn seed(&self) -> PathBuf {
        self.root.join("seed")
    }
    pub fn workspace(&self, name: &str) -> PathBuf {
        self.root.join("workspaces").join(name)
    }
    pub fn store(&self, pair: u8) -> PathBuf {
        self.root.join("stores").join(format!("pair-{pair}"))
    }
    pub fn view(&self, pair: u8) -> PathBuf {
        self.root.join("views").join(format!("pair-{pair}.txt"))
    }
    pub fn view_record(&self, pair: u8) -> PathBuf {
        self.root.join("views").join(format!("pair-{pair}.json"))
    }
    pub fn manifest(&self, scope: super::Scope) -> PathBuf {
        self.root.join("manifests").join(match scope {
            super::Scope::Probe => "probe-manifest.json",
            super::Scope::Coding => "coding-manifest.json",
        })
    }
}

/// Copy the verified seed into the root's private seed directory (0600).
pub fn install_seed(root: &PilotRoot, verified: &Verified) -> Result<(), CaptureFault> {
    let seed = root.seed();
    create_private_dir(&seed.join("src"))?;
    for (rel, src) in &verified.seed {
        let mut input = super::open_nofollow(src, false)?;
        let mut bytes = Vec::new();
        input
            .read_to_end(&mut bytes)
            .map_err(|_| pin_fault("seed_unreadable"))?;
        write_private_new(&seed.join(rel), &bytes)?;
    }
    check_seed_tree(&seed)
}

fn check_seed_tree(dir: &Path) -> Result<(), CaptureFault> {
    let listing = list_files(dir)?;
    let mut expected: Vec<String> = pins::SEED.iter().map(|(r, _)| r.to_string()).collect();
    expected.sort();
    if listing != expected {
        return Err(pin_fault("seed_tree_not_exact"));
    }
    for (rel, sha) in pins::SEED {
        if file_sha256(&dir.join(rel))? != sha {
            return Err(pin_fault("seed_changed"));
        }
    }
    Ok(())
}

fn list_files(dir: &Path) -> Result<Vec<String>, CaptureFault> {
    collect_private_tree(dir, false, &[], &mut PrivateTreeBudget::new(None))?
        .into_iter()
        .map(|p| {
            p.into_os_string()
                .into_string()
                .map_err(|_| fault(FaultClass::Containment, "tree_unreadable"))
        })
        .collect()
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), CaptureFault> {
    let files = collect_private_tree(from, false, &[], &mut PrivateTreeBudget::new(None))?;
    create_private_dir(to)?;
    for rel in files {
        let dest = to.join(&rel);
        let mut parent = to.to_path_buf();
        if let Some(relative_parent) = rel.parent() {
            for component in relative_parent.components() {
                let std::path::Component::Normal(name) = component else {
                    return Err(fault(FaultClass::Persistence, "copy_failed"));
                };
                parent.push(name);
                match fs::symlink_metadata(&parent) {
                    Ok(_) => require_private_directory(&parent)?,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        create_private_dir(&parent)?
                    }
                    Err(_) => return Err(fault(FaultClass::Persistence, "copy_failed")),
                }
            }
        }
        let mut input = super::open_nofollow(&from.join(&rel), false)?;
        if !input
            .metadata()
            .map_err(|_| fault(FaultClass::Persistence, "copy_failed"))?
            .is_file()
        {
            return Err(fault(FaultClass::Containment, "tree_unreadable"));
        }
        let mut bytes = Vec::new();
        input
            .read_to_end(&mut bytes)
            .map_err(|_| fault(FaultClass::Persistence, "copy_failed"))?;
        write_private_new(&dest, &bytes)?;
    }
    verify_private_tree(to, &[], &mut PrivateTreeBudget::new(None))
}

/// Restore the canonical workspace `name` from the seed. An existing arm's
/// workspace is first archived unchanged under `archive/<archive_as>`.
/// Returns the restored sources (relative path, sha256) in frozen order.
pub fn restore_workspace(
    root: &PilotRoot,
    name: &str,
    archive_as: Option<&str>,
) -> Result<(PathBuf, Vec<(String, String)>), CaptureFault> {
    let ws = root.workspace(name);
    if ws.symlink_metadata().is_ok() {
        verify_private_tree(&ws, &[], &mut PrivateTreeBudget::new(None))?;
    }
    check_seed_tree(&root.seed())?;
    if ws.symlink_metadata().is_ok() {
        if let Some(label) = archive_as {
            copy_tree(&ws, &root.root.join("archive").join(label))?;
        }
        fs::remove_dir_all(&ws)
            .map_err(|_| fault(FaultClass::Persistence, "workspace_reset_failed"))?;
    }
    copy_tree(&root.seed(), &ws)?;
    let ws = fs::canonicalize(&ws)
        .map_err(|_| fault(FaultClass::Persistence, "workspace_reset_failed"))?;
    check_seed_tree(&ws)?;
    let sources = pins::SEED
        .iter()
        .map(|(r, s)| (r.to_string(), s.to_string()))
        .collect();
    Ok((ws, sources))
}

// ---------------------------------------------------------------------------
// View materialization (contract §3).
// ---------------------------------------------------------------------------

/// Evidence of one frozen pair view.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewRecord {
    pub pair: u8,
    pub workspace: PathBuf,
    pub store: PathBuf,
    pub view_sha256: String,
    pub view_bytes: u64,
    pub delivered_o200k_tokens: u64,
    pub header: String,
    pub store_revision: String,
    pub citations: Vec<String>,
    pub sources: Vec<(String, String)>,
    pub recipe_id: String,
    pub function_digest: String,
    pub profile_name: String,
    pub embedded_units: u64,
    pub searchable_units: u64,
    pub eligible_units: u64,
    pub index_ms: u64,
    pub prepare_ms: u64,
    pub context_ms: u64,
    pub foundry_sha256: String,
    pub profile_sha256: String,
}

struct Ran {
    status: Option<i32>,
    stdout: Vec<u8>,
    elapsed: Duration,
    timed_out: bool,
}

/// Run one pinned executable with a minimal environment, bounded output and
/// a hard deadline (its process group is killed when the deadline passes).
fn run_bounded(
    exe: &Path,
    args: &[&std::ffi::OsStr],
    cwd: &Path,
    env: &[(&str, String)],
    limit: Duration,
) -> Result<Ran, CaptureFault> {
    let started = Instant::now();
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (*k, v.as_str())))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0);
    private_child(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|_| fault(FaultClass::Bootstrap, "apparatus_spawn_failed"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or(fault(FaultClass::Bootstrap, "apparatus_spawn_failed"))?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = (&mut stdout).take(8 * 1024 * 1024).read_to_end(&mut buf);
        let mut sink = [0u8; 8192];
        while matches!(stdout.read(&mut sink), Ok(n) if n > 0) {}
        buf
    });
    let pgid = child.id() as i32;
    let mut timed_out = false;
    let status = loop {
        if let Ok(Some(s)) = child.try_wait() {
            break Some(s);
        }
        if started.elapsed() >= limit {
            timed_out = true;
            // SAFETY: signalling our own child's process group.
            unsafe {
                libc::kill(-pgid, libc::SIGKILL);
            }
            break child.wait().ok();
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = reader.join().unwrap_or_default();
    Ok(Ran {
        status: status.and_then(|s| s.code()),
        stdout,
        elapsed: started.elapsed(),
        timed_out,
    })
}

fn foundry_env(root: &PilotRoot) -> Vec<(&'static str, String)> {
    // The worker needs the owner's real home for its bundle; no provider or
    // proxy variables are passed.
    let home = std::env::var("HOME").unwrap_or_default();
    vec![
        ("HOME", home),
        ("PATH", "/usr/bin:/bin".into()),
        ("LANG", "C".into()),
        (
            "TMPDIR",
            format!("{}/", root.root.join("renders").display()),
        ),
    ]
}

/// Handles in a delivered view: `path#start-end@sha32.ws16`.
fn citations(view: &str) -> Vec<String> {
    view.lines()
        .filter_map(|l| l.split_whitespace().next())
        .filter(|tok| {
            tok.split_once('#')
                .is_some_and(|(_, rest)| rest.contains('@') && rest.contains('-'))
        })
        .map(str::to_string)
        .collect()
}

/// Materialize and freeze all four pair views: fresh pair workspace from the
/// seed, fresh isolated store, pinned release Foundry and profile with
/// development isolation, the exact query and the 2048-token allowance.
/// Requires `semantic:ready`, complete coverage and the unchanged seed;
/// enforces 180 s per pair and 900 s in total.
pub fn materialize_views(
    root: &PilotRoot,
    verified: &Verified,
) -> Result<Vec<ViewRecord>, CaptureFault> {
    let started = Instant::now();
    let total = Duration::from_secs(pins::MATERIALIZATION_TOTAL_SECS);
    let foundry = &verified.inputs.foundry;
    let profile = &verified.inputs.profile;
    let env = foundry_env(root);
    let mut records = Vec::new();
    for pair in pins::PAIRS {
        let pair_started = Instant::now();
        let pair_limit = Duration::from_secs(pins::PAIR_PREPARATION_SECS)
            .min(total.saturating_sub(started.elapsed()));
        let remaining = |label: &'static str| -> Result<Duration, CaptureFault> {
            let left = pair_limit.saturating_sub(pair_started.elapsed());
            if left.is_zero() {
                return Err(fault(FaultClass::Deadline, label));
            }
            Ok(left)
        };
        let (ws, sources) = restore_workspace(root, &format!("pair-{pair}"), None)?;
        let store = root.store(pair);
        if store.symlink_metadata().is_ok() {
            return Err(fault(FaultClass::Pins, "pair_store_not_fresh"));
        }
        let os = |s: &Path| s.as_os_str().to_owned();
        let index = run_bounded(
            foundry,
            &[
                std::ffi::OsStr::new("index"),
                std::ffi::OsStr::new("--store"),
                &os(&store),
                &os(&ws),
            ],
            &root.root,
            &env,
            remaining("pair_preparation_wall")?,
        )?;
        verify_private_tree(
            &store,
            &[],
            &mut PrivateTreeBudget::new(Some(pair_started + pair_limit)),
        )?;
        let report: Value =
            serde_json::from_slice(&index.stdout).map_err(|_| pin_fault("index_report_invalid"))?;
        if index.status != Some(0)
            || index.timed_out
            || report.get("scan_complete") != Some(&Value::Bool(true))
            || report.get("failures").and_then(Value::as_u64) != Some(0)
            || report.get("partial") != Some(&Value::Bool(false))
        {
            return Err(pin_fault("index_incomplete"));
        }
        let budget = remaining("pair_preparation_wall")?
            .as_secs()
            .max(1)
            .to_string();
        let prepare = run_bounded(
            foundry,
            &[
                "semantic".as_ref(),
                "prepare".as_ref(),
                "--store".as_ref(),
                &os(&store),
                "--profile".as_ref(),
                &os(profile),
                "--budget-seconds".as_ref(),
                budget.as_ref(),
                "--development-isolation".as_ref(),
            ],
            &root.root,
            &env,
            remaining("pair_preparation_wall")?,
        )?;
        verify_private_tree(
            &store,
            &[],
            &mut PrivateTreeBudget::new(Some(pair_started + pair_limit)),
        )?;
        let prep: Value = serde_json::from_slice(&prepare.stdout)
            .map_err(|_| pin_fault("prepare_report_invalid"))?;
        let u = |p: &str| prep.pointer(p).and_then(Value::as_u64).unwrap_or(0);
        let s = |p: &str| {
            prep.pointer(p)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let eligible = u("/coverage/eligible_units");
        let searchable = u("/coverage/searchable_current_units");
        if prepare.status != Some(0)
            || prepare.timed_out
            || prep.get("partial") != Some(&Value::Bool(false))
            || s("/provider_state") != "ready"
            || eligible == 0
            || searchable != eligible
        {
            return Err(pin_fault("semantic_preparation_incomplete"));
        }
        let tokens = pins::CONTEXT_TOKENS.to_string();
        let context = run_bounded(
            foundry,
            &[
                "context".as_ref(),
                "--store".as_ref(),
                &os(&store),
                "--tokens".as_ref(),
                tokens.as_ref(),
                "--semantic-profile".as_ref(),
                &os(profile),
                "--development-isolation".as_ref(),
                pins::CONTEXT_QUERY.as_ref(),
            ],
            &root.root,
            &env,
            remaining("pair_preparation_wall")?,
        )?;
        verify_private_tree(
            &store,
            &[],
            &mut PrivateTreeBudget::new(Some(pair_started + pair_limit)),
        )?;
        let view = String::from_utf8(context.stdout).map_err(|_| pin_fault("view_not_utf8"))?;
        let header = view.lines().next().unwrap_or_default().to_string();
        let delivered = crate::response::count_tokens(&view) as u64;
        if context.status != Some(0)
            || context.timed_out
            || !header.starts_with("foundry context · r")
            || !header.split(" · ").any(|f| f == "semantic:ready")
            || delivered > pins::CONTEXT_TOKENS as u64
            || view.trim().is_empty()
        {
            return Err(pin_fault("view_not_ready"));
        }
        // Preparation never writes into the workspace.
        check_seed_tree(&ws)?;
        let store_revision = header
            .split(" · ")
            .nth(1)
            .filter(|f| f.starts_with('r'))
            .unwrap_or_default()
            .to_string();
        write_private_new(&root.view(pair), view.as_bytes())?;
        let record = ViewRecord {
            pair,
            workspace: ws,
            store: fs::canonicalize(&store).map_err(|_| pin_fault("store_missing"))?,
            view_sha256: sha256_hex(view.as_bytes()),
            view_bytes: view.len() as u64,
            delivered_o200k_tokens: delivered,
            header,
            store_revision,
            citations: citations(&view),
            sources,
            recipe_id: s("/recipe_id"),
            function_digest: s("/function_digest"),
            profile_name: s("/profile"),
            embedded_units: u("/embedded_units"),
            searchable_units: searchable,
            eligible_units: eligible,
            index_ms: index.elapsed.as_millis() as u64,
            prepare_ms: prepare.elapsed.as_millis() as u64,
            context_ms: context.elapsed.as_millis() as u64,
            foundry_sha256: pins::FOUNDRY_RELEASE_SHA256.into(),
            profile_sha256: pins::PROFILE_SHA256.into(),
        };
        write_private_new(
            &root.view_record(pair),
            &serde_json::to_vec_pretty(&record)
                .map_err(|_| fault(FaultClass::Persistence, "record_encode_failed"))?,
        )?;
        records.push(record);
    }
    if started.elapsed() > total {
        return Err(fault(FaultClass::Deadline, "materialization_wall"));
    }
    Ok(records)
}

/// Read back a frozen view and verify it against its record.
pub fn load_view(root: &PilotRoot, pair: u8) -> Result<(String, ViewRecord), CaptureFault> {
    let record: ViewRecord = serde_json::from_slice(&super::read_private_evidence(
        &root.view_record(pair),
        "view_record_missing",
        1 << 20,
    )?)
    .map_err(|_| fault(FaultClass::Authority, "view_record_invalid"))?;
    let view = String::from_utf8(super::read_private_evidence(
        &root.view(pair),
        "view_missing",
        8 << 20,
    )?)
    .map_err(|_| fault(FaultClass::Authority, "view_missing"))?;
    if record.pair != pair || sha256_hex(view.as_bytes()) != record.view_sha256 {
        return Err(fault(FaultClass::Pins, "view_changed"));
    }
    Ok((view, record))
}

// ---------------------------------------------------------------------------
// Arms, renders and launch manifests.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arm {
    A,
    B,
}

/// The pair number and arm of a frozen coding slot id such as `2B`.
pub fn slot_arm(slot_id: &str) -> Option<(u8, Arm)> {
    let mut chars = slot_id.chars();
    let pair = chars.next()?.to_digit(10)? as u8;
    let arm = match chars.next()? {
        'A' => Arm::A,
        'B' => Arm::B,
        _ => return None,
    };
    (chars.next().is_none() && pins::PAIRS.contains(&pair)).then_some((pair, arm))
}

/// The exact initial-user prompt of an arm: A is the frozen arm-A prompt
/// (no injected evidence); B is the task, the existing framing and the
/// pair's frozen view verbatim.
pub fn arm_prompt(verified: &Verified, arm: Arm, view: &str) -> String {
    match arm {
        Arm::A => verified.arm_a_prompt.clone(),
        Arm::B => format!("{}{}{}", verified.task, pins::EVIDENCE_FRAMING, view),
    }
}

fn commitments(b: &super::RenderBinding) -> RenderCommitments {
    RenderCommitments {
        system_sha256: b.system_sha256.clone(),
        tools_sha256: b.tools_sha256.clone(),
        settings_sha256: b.settings_sha256.clone(),
        stable_prefix_sha256: b.stable_prefix_sha256.clone(),
        beta: b.beta.clone(),
        cache_layout: b.cache_layout.clone(),
        initial_user_sha256: b.initial_user_sha256.clone(),
    }
}

/// A slot's canonical workspace, exact prompt and frozen sources.
type SlotInputs = (PathBuf, String, Vec<(String, String)>);

/// Render every slot with the actual host (no request leaves the machine),
/// require each pair's arms to differ only in the evidence slot, and write
/// the frozen probe and coding launch manifests.
pub fn build_manifests(
    root: &PilotRoot,
    verified: &Verified,
    run_dir: &Path,
) -> Result<(LaunchManifest, LaunchManifest), CaptureFault> {
    let run_dir = super::resolved_run_dir(run_dir)?;
    let mut views = Vec::new();
    let mut prompts: BTreeMap<String, SlotInputs> = BTreeMap::new();
    for pair in pins::PAIRS {
        let (view, record) = load_view(root, pair)?;
        views.push(ManifestView {
            pair,
            path: root.view(pair),
            sha256: record.view_sha256.clone(),
            delivered_o200k_tokens: record.delivered_o200k_tokens,
            store_revision: record.store_revision.clone(),
            citations_sha256: sha256_hex(record.citations.join("\n").as_bytes()),
            recipe_id: record.recipe_id.clone(),
        });
        for arm in [Arm::A, Arm::B] {
            let id = format!("{pair}{}", if arm == Arm::A { 'A' } else { 'B' });
            prompts.insert(
                id,
                (
                    record.workspace.clone(),
                    arm_prompt(verified, arm, &view),
                    record.sources.clone(),
                ),
            );
        }
    }
    let (probe_ws, probe_sources) = restore_workspace(root, "probe", None)?;
    let render = |slot_id: &str, ws: &Path, prompt: &str, sources: &[(String, String)]| {
        render_binding(
            &PreviewAdmission {
                run_dir: root.root.join("renders").join(format!("render-{slot_id}")),
                omp: verified.inputs.omp.clone(),
            },
            &PreparedSlot {
                slot_id: slot_id.to_string(),
                workspace: ws.to_path_buf(),
                initial_user_text: prompt.to_string(),
                expected_sources: sources.to_vec(),
            },
        )
    };
    let mut coding_slots = Vec::new();
    for slot_id in policy::SLOT_ORDER {
        let (ws, prompt, sources) = prompts.get(slot_id).ok_or(pin_fault("slot_unknown"))?;
        let binding = render(slot_id, ws, prompt, sources)?;
        if binding.initial_user_sha256 != sha256_hex(prompt.as_bytes()) {
            return Err(fault(FaultClass::RequestBinding, "render_prompt_unbound"));
        }
        coding_slots.push((
            slot_id.to_string(),
            binding,
            ws.clone(),
            prompt.clone(),
            sources.clone(),
        ));
    }
    for pair in pins::PAIRS {
        let find = |arm: char| {
            coding_slots
                .iter()
                .find(|(id, ..)| *id == format!("{pair}{arm}"))
                .map(|(_, b, ..)| b.clone())
                .ok_or(pin_fault("slot_unknown"))
        };
        find('A')?.matches_pair(&find('B')?)?;
    }
    let probe_binding = render(
        policy::PROBE_SLOT,
        &probe_ws,
        policy::PROBE_INSTRUCTION,
        &probe_sources,
    )?;
    let manifest = |slots: Vec<ManifestSlot>| LaunchManifest {
        kind: super::LAUNCH_MANIFEST_KIND.into(),
        run_dir: run_dir.clone(),
        contract_sha256: policy::CONTRACT_SHA256.into(),
        omp_sha256: policy::OMP_SHA256.into(),
        slots,
        pins: verified.pins.clone(),
        allocation_sha256: pins::ALLOCATION_SHA256.into(),
        views: views.clone(),
        isolation_root: root.root.clone(),
    };
    let probe = manifest(vec![ManifestSlot {
        slot_id: policy::PROBE_SLOT.into(),
        arm: "probe".into(),
        workspace: probe_ws,
        prompt_sha256: sha256_hex(policy::PROBE_INSTRUCTION.as_bytes()),
        sources: probe_sources,
        render: commitments(&probe_binding),
    }]);
    let coding = manifest(
        coding_slots
            .into_iter()
            .map(|(slot_id, binding, ws, prompt, sources)| ManifestSlot {
                arm: slot_id[1..].to_string(),
                slot_id,
                workspace: ws,
                prompt_sha256: sha256_hex(prompt.as_bytes()),
                sources,
                render: commitments(&binding),
            })
            .collect(),
    );
    for (scope, m) in [
        (super::Scope::Probe, &probe),
        (super::Scope::Coding, &coding),
    ] {
        write_private_new(
            &root.manifest(scope),
            &serde_json::to_vec_pretty(m)
                .map_err(|_| fault(FaultClass::Persistence, "manifest_encode_failed"))?,
        )?;
    }
    Ok((probe, coding))
}

// ---------------------------------------------------------------------------
// The contained independent checker (contract §9).
// ---------------------------------------------------------------------------

fn sbpl(path: &Path) -> Result<String, CaptureFault> {
    let text = path.to_str().ok_or(fault(
        FaultClass::CheckerProvenance,
        "check_path_unquotable",
    ))?;
    if !path.is_absolute()
        || text
            .chars()
            .any(|c| c == '"' || c == '\\' || c.is_control())
    {
        return Err(fault(
            FaultClass::CheckerProvenance,
            "check_path_unquotable",
        ));
    }
    Ok(text.to_string())
}

/// The existing checker sandbox bytes, also validated by the receipt owner.
/// This binds the profile actually supplied; it is not an OS-jail attestation.
pub(super) fn check_policy(root: &Path, real_home: &Path) -> Result<String, CaptureFault> {
    let (root, home) = (sbpl(root)?, sbpl(real_home)?);
    Ok(format!(
        "(version 1)(allow default)(deny network*)\
         (deny file-read-data (subpath \"{home}\"))(deny file-write* (subpath \"{home}\"))\
         (deny file-write* (subpath \"/private/tmp\"))(deny file-write* (subpath \"/private/var/folders\"))\
         (allow file-read-data (subpath \"{root}\"))(allow file-write* (subpath \"{root}\"))"
    ))
}

/// The complete sanitized environment supplied to each fixed checker step.
pub(super) fn check_environment(root: &Path) -> Result<BTreeMap<String, String>, CaptureFault> {
    let root = sbpl(root)?;
    Ok([
        ("HOME".into(), format!("{root}/home")),
        ("LANG".into(), "C".into()),
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("TMPDIR".into(), format!("{root}/tmp/")),
        ("XDG_CONFIG_HOME".into(), format!("{root}/home/.config")),
        ("XDG_DATA_HOME".into(), format!("{root}/home/.local/share")),
    ]
    .into_iter()
    .collect())
}

/// Hash only a completely observed checker stream. A read failure must
/// never turn its observed prefix into an apparently complete output hash.
fn check_output_sha256(mut pipe: impl Read) -> Result<String, CaptureFault> {
    let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
    let mut buf = [0u8; 16 * 1024];
    loop {
        let n = pipe
            .read(&mut buf)
            .map_err(|_| fault(FaultClass::CheckerProvenance, "check_output_unbound"))?;
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

/// Retain at most the selected cap; an error carries only its observed prefix.
fn check_stderr_bytes(mut pipe: impl Read) -> Result<Vec<u8>, (CaptureFault, Vec<u8>)> {
    let mut bytes = Vec::new();
    let mut buf = [0u8; 16 * 1024];
    loop {
        let n = match pipe.read(&mut buf) {
            Ok(n) => n,
            Err(_) => {
                return Err((
                    fault(FaultClass::CheckerProvenance, "check_stderr_read_failed"),
                    bytes,
                ));
            }
        };
        if n == 0 {
            return Ok(bytes);
        }
        let remaining = CHECK_STDERR_MAX_BYTES - bytes.len();
        bytes.extend_from_slice(&buf[..n.min(remaining)]);
        if n > remaining {
            return Err((
                fault(FaultClass::CheckerProvenance, "check_stderr_overflow"),
                bytes,
            ));
        }
    }
}

/// Poll only these owned pipe descriptors, so cancellation also bounds reads
/// after the leader exits while an owned descendant still holds a pipe open.
struct CheckPipe<R> {
    pipe: R,
    cancelled: Arc<AtomicBool>,
}

impl<R: Read + AsRawFd> Read for CheckPipe<R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        loop {
            if self.cancelled.load(Ordering::Acquire) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "checker capture cancelled",
                ));
            }
            let mut fd = libc::pollfd {
                fd: self.pipe.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: fd points to one initialized pollfd for this live owned
            // descriptor; poll neither takes ownership nor changes the fd.
            let ready = unsafe { libc::poll(&mut fd, 1, 10) };
            if ready < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if ready == 0 {
                continue;
            }
            if fd.revents & (libc::POLLIN | libc::POLLHUP) != 0 {
                // This thread is the sole reader of this pipe. Read only after
                // readiness/HUP; HUP can still have buffered bytes before EOF.
                return self.pipe.read(bytes);
            }
            return Err(std::io::Error::other("checker pipe unavailable"));
        }
    }
}

#[derive(Debug)]
struct CheckProcessOutput {
    status: ExitStatus,
    stdout_sha256: String,
    stderr_sha256: String,
    stderr_path: String,
    stderr_bytes: u64,
}

fn retain_check_stderr(
    root: &Path,
    name: &str,
    bytes: &[u8],
    partial: bool,
) -> Result<String, CaptureFault> {
    let suffix = if partial {
        ".partial.stderr"
    } else {
        ".stderr"
    };
    let relative = format!("diagnostics/{name}{suffix}");
    write_private_new(&root.join(&relative), bytes)?;
    if partial {
        write_private_new(
            &root.join(format!("{relative}.sha256")),
            format!("{}\n", sha256_hex(bytes)).as_bytes(),
        )?;
    }
    super::fsync_dir(&root.join("diagnostics"))?;
    Ok(relative)
}

/// Execute one actual fixed checker step. The caller supplies the same absolute
/// deadline to all four steps; it covers the leader and both pipe readers.
fn run_check_step(
    cmd: &mut Command,
    root: &Path,
    name: &str,
    deadline: Instant,
) -> Result<CheckProcessOutput, CaptureFault> {
    let prov = |code| fault(FaultClass::CheckerProvenance, code);
    if !policy::CHECK_RECIPE.iter().any(|(step, _)| *step == name) {
        return Err(prov("check_recipe_mismatch"));
    }
    super::validate_check_directory(root)?;
    super::validate_check_directory(&root.join("diagnostics"))?;
    if Instant::now() >= deadline {
        return Err(prov("post_check_wall_exceeded"));
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    private_child(cmd);
    let mut child = cmd.spawn().map_err(|_| prov("check_spawn_failed"))?;
    let pgid = child.id() as i32;
    let stop_group = || {
        // SAFETY: only the process group created above for this owned child is
        // signalled. Keep this group cleanup even after its leader has exited.
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
    };
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        stop_group();
        let _ = child.wait();
        return Err(prov("check_spawn_failed"));
    };
    let cancelled = Arc::new(AtomicBool::new(false));
    let (send_fault, faults) = std::sync::mpsc::channel();
    let out_cancelled = cancelled.clone();
    let out_fault = send_fault.clone();
    let out_t = std::thread::spawn(move || {
        let output = check_output_sha256(CheckPipe {
            pipe: stdout,
            cancelled: out_cancelled,
        });
        if let Err(error) = output.as_ref() {
            let _ = out_fault.send(*error);
        }
        output
    });
    let err_cancelled = cancelled.clone();
    let err_t = std::thread::spawn(move || {
        let output = check_stderr_bytes(CheckPipe {
            pipe: stderr,
            cancelled: err_cancelled,
        });
        if let Err((error, _)) = output.as_ref() {
            let _ = send_fault.send(*error);
        }
        output
    });
    let mut status = None;
    let mut first_fault = None;
    let mut stopped = false;
    loop {
        if first_fault.is_none() {
            first_fault = faults.try_recv().ok();
        }
        if first_fault.is_none() && status.is_none() {
            match child.try_wait() {
                Ok(observed) => status = observed,
                Err(_) => first_fault = Some(prov("check_wait_failed")),
            }
        }
        if first_fault.is_none() && Instant::now() >= deadline {
            first_fault = Some(prov("post_check_wall_exceeded"));
        }
        if first_fault.is_some() {
            cancelled.store(true, Ordering::Release);
            stop_group();
            let _ = child.wait();
            stopped = true;
            break;
        }
        if status.is_some() && out_t.is_finished() && err_t.is_finished() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // Both readers use cancellable polling; always join both before returning,
    // including on timeout or an earlier reader failure.
    let stdout = out_t.join();
    let stderr = err_t.join();
    if first_fault.is_none() {
        first_fault = faults.try_recv().ok();
    }
    let (stderr_bytes, partial) = match stderr {
        // Killing the group can produce EOF before a reader observes cancellation.
        // Stopped capture is conservatively partial, never certified by cleanup EOF.
        Ok(Ok(bytes)) => (bytes, stopped),
        Ok(Err((error, bytes))) => {
            first_fault.get_or_insert(error);
            (bytes, true)
        }
        Err(_) => {
            if !stopped {
                stop_group();
                let _ = child.wait();
            }
            return Err(first_fault.unwrap_or(prov("check_output_unbound")));
        }
    };
    let stdout = match stdout {
        Ok(result) => result,
        Err(_) => Err(prov("check_output_unbound")),
    };
    if let Err(error) = stdout.as_ref() {
        first_fault.get_or_insert(*error);
    }
    if first_fault.is_some() && !stopped {
        stop_group();
        let _ = child.wait();
    }
    // Retain the complete bytes or explicitly labelled partial prefix even if
    // another capture fault prevents a normal receipt. Persistence failure is
    // itself an error; never claim an artifact was retained when it was not.
    let stderr_path = retain_check_stderr(root, name, &stderr_bytes, partial)?;
    if let Some(error) = first_fault {
        return Err(error);
    }
    let stdout_sha256 = stdout?;
    Ok(CheckProcessOutput {
        status: status.ok_or(prov("check_wait_failed"))?,
        stdout_sha256,
        stderr_sha256: sha256_hex(&stderr_bytes),
        stderr_path,
        stderr_bytes: stderr_bytes.len() as u64,
    })
}

/// Copy the stopped host's candidate and the private oracle into a new
/// checker root outside agent access, then run the accepted four-step recipe
/// under containment with a minimal no-credential environment and the 60 s
/// post-check ceiling. Untrusted candidate code only ever runs inside the
/// sandbox. The result binds slot, candidate, oracle, compiler, argv,
/// exact environment/profile, exits and retained output/artifact digests.
/// Capture failures retain bounded partial stderr and return Err; only complete
/// output can be represented by a normal receipt.
pub fn run_independent_check(
    slot_id: &str,
    workspace: &Path,
    candidate_order: &[String],
    oracle: &Path,
    checker_root: &Path,
) -> Result<IndependentCheckReceipt, CaptureFault> {
    let prov = |code| fault(FaultClass::CheckerProvenance, code);
    if file_sha256(oracle)? != policy::ORACLE_SHA256 {
        return Err(prov("check_oracle_changed"));
    }
    if file_sha256(Path::new(policy::RUSTC))? != policy::RUSTC_SHA256 {
        return Err(prov("check_compiler_changed"));
    }
    create_private_dir(checker_root)?;
    let root = fs::canonicalize(checker_root).map_err(|_| prov("check_root_unresolvable"))?;
    for d in ["src", "out", "home", "tmp", "diagnostics"] {
        create_private_dir(&root.join(d))?;
    }
    let mut candidate = Vec::new();
    for rel in candidate_order {
        let src = workspace.join(rel);
        let meta = fs::symlink_metadata(&src).map_err(|_| prov("check_candidate_missing"))?;
        if !meta.is_file() {
            return Err(prov("check_candidate_not_regular"));
        }
        let mut input = open_private_file(&src, PrivateArtifact::File)?;
        let mut bytes = Vec::new();
        input
            .read_to_end(&mut bytes)
            .map_err(|_| prov("check_candidate_missing"))?;
        write_private_new(&root.join(rel), &bytes)?;
        candidate.push((rel.clone(), sha256_hex(&bytes)));
    }
    let mut oracle_input = super::open_nofollow(oracle, false)?;
    let mut oracle_bytes = Vec::new();
    oracle_input
        .read_to_end(&mut oracle_bytes)
        .map_err(|_| prov("check_oracle_changed"))?;
    let oracle_sha256 = sha256_hex(&oracle_bytes);
    if oracle_sha256 != policy::ORACLE_SHA256 {
        return Err(prov("check_oracle_changed"));
    }
    write_private_new(&root.join("check.rs"), &oracle_bytes)?;
    let real_home = super::real_home_for_checks()?;
    let sandbox = check_policy(&root, &real_home)?;
    let environment = check_environment(&root)?;
    // This is the observed header value, not a copied policy constant. The
    // compiler is freshly observed again immediately before each compile step.
    let compiler_sha256 = file_sha256(Path::new(policy::RUSTC))?;
    if compiler_sha256 != policy::RUSTC_SHA256 {
        return Err(prov("check_compiler_changed"));
    }
    let version = Command::new(policy::RUSTC)
        .arg("--version")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()
        .map_err(|_| prov("check_compiler_unavailable"))?;
    let compiler_version = String::from_utf8_lossy(&version.stdout).trim().to_string();
    if !version.status.success() || !compiler_version.starts_with(policy::RUSTC_VERSION_PREFIX) {
        return Err(prov("check_compiler_changed"));
    }
    let root_text = root
        .to_str()
        .ok_or(prov("check_path_unquotable"))?
        .to_string();
    let deadline = Instant::now() + Duration::from_secs(policy::POST_CHECK_SECS);
    let mut steps = Vec::new();
    for (name, template) in policy::CHECK_RECIPE {
        let argv: Vec<String> = template
            .iter()
            .map(|a| a.replace("<ROOT>", &root_text))
            .collect();
        let started = Instant::now();
        if name == "oracle_compile"
            && file_sha256(&root.join("check.rs")).map_err(|_| prov("check_oracle_changed"))?
                != oracle_sha256
        {
            return Err(prov("check_oracle_changed"));
        }
        // Fresh pathname observations are not an atomic hash-and-exec binding.
        let executable_sha256 = if name == "oracle_run" {
            open_private_file(&root.join("out/check"), PrivateArtifact::Executable)?;
            file_sha256(&root.join("out/check")).map_err(|_| prov("check_artifact_unbound"))?
        } else {
            let observed = file_sha256(Path::new(policy::RUSTC))?;
            if observed != compiler_sha256 {
                return Err(prov("check_compiler_changed"));
            }
            observed
        };
        let mut cmd = Command::new("/usr/bin/sandbox-exec");
        cmd.arg("-p")
            .arg(&sandbox)
            .args(&argv)
            .current_dir(&root)
            .env_clear()
            .envs(&environment);
        let output = run_check_step(&mut cmd, &root, name, deadline)?;
        verify_private_tree(
            &root,
            &[Path::new("out/check")],
            &mut PrivateTreeBudget::new(Some(deadline)),
        )?;
        let ok = output.status.success();
        let artifact = |rel: &str| -> Result<Vec<(String, String)>, CaptureFault> {
            match fs::read(root.join(rel)) {
                Ok(bytes) => Ok(vec![(rel.to_string(), sha256_hex(&bytes))]),
                Err(_) if ok => Err(prov("check_artifact_unbound")),
                Err(_) => Ok(Vec::new()),
            }
        };
        let artifacts = match name {
            "noalloc_compile" => artifact("out/librecords-noalloc.rlib")?,
            "alloc_compile" => artifact("out/librecords-alloc.rlib")?,
            "oracle_compile" => artifact("out/check")?,
            "oracle_run" => vec![("out/check".into(), executable_sha256.clone())],
            _ => Vec::new(),
        };
        steps.push(CheckStep {
            step: name.to_string(),
            argv,
            executable_sha256,
            exit: output.status.code(),
            signal: output.status.signal(),
            timed_out: false,
            stdout_sha256: output.stdout_sha256,
            stderr_sha256: output.stderr_sha256,
            stderr_path: output.stderr_path,
            stderr_bytes: output.stderr_bytes,
            artifacts,
            wall_ms: started.elapsed().as_millis() as u64,
        });
        if !ok {
            break;
        }
    }
    Ok(IndependentCheckReceipt {
        kind: CHECK_RECEIPT_KIND.into(),
        slot_id: slot_id.to_string(),
        checker_root: root,
        candidate,
        oracle_sha256,
        compiler: PathBuf::from(policy::RUSTC),
        compiler_sha256,
        compiler_version,
        environment,
        sandbox_profile: sandbox,
        steps,
        fixture: false,
    })
}

// ---------------------------------------------------------------------------
// Executor and descriptive report (contract §10–§12).
// ---------------------------------------------------------------------------

/// One slot's descriptive row. Unstarted slots keep their row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlotRow {
    pub slot_id: String,
    pub pair: Option<u8>,
    pub arm: String,
    pub state: String,
    pub disposition: Option<super::Disposition>,
    pub check: Option<super::CheckOutcome>,
    pub check_refusal: Option<String>,
    pub generations: u32,
    pub tool_calls: u32,
    pub refused_tool_calls: u32,
    pub inclusive_input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub budget_cost_micro_usd: u64,
    pub host_wall_ms: Option<u64>,
    pub prompt_status: Option<String>,
    pub usage_complete: bool,
}

/// Per-pair paired comparison: success counts only, no inference.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairRow {
    pub pair: u8,
    pub a_pass: Option<bool>,
    pub b_pass: Option<bool>,
    /// B − A correctness (1, 0 or −1) when both arms were checked.
    pub b_minus_a: Option<i8>,
    pub valid: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Spread {
    pub n: u32,
    pub median: Option<f64>,
    pub min: Option<u64>,
    pub max: Option<u64>,
}

fn spread(values: &[u64]) -> Spread {
    let mut v = values.to_vec();
    v.sort_unstable();
    let n = v.len();
    let median = match n {
        0 => None,
        _ if n % 2 == 1 => Some(v[n / 2] as f64),
        _ => Some((v[n / 2 - 1] as f64 + v[n / 2] as f64) / 2.0),
    };
    Spread {
        n: n as u32,
        median,
        min: v.first().copied(),
        max: v.last().copied(),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArmSummary {
    pub arm: String,
    pub checked: u32,
    pub passed: u32,
    pub host_wall_ms: Spread,
    pub inclusive_input_tokens: Spread,
    pub output_tokens: Spread,
}

/// The descriptive pilot report. It states denominators and missing
/// observations explicitly and makes no significance, causal, savings,
/// generalization, T006 or invoice claim (contract §1, §12).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PilotReport {
    pub kind: String,
    pub mode: super::Mode,
    pub slots: Vec<SlotRow>,
    pub pairs: Vec<PairRow>,
    pub arms: Vec<ArmSummary>,
    pub receipt: super::CaptureReceipt,
    pub claim_ceiling: String,
}

fn slot_row(
    slot_id: &str,
    capture: Option<&super::SlotCapture>,
    sealed: Option<&Result<super::SealedSlot, CaptureFault>>,
) -> SlotRow {
    let (pair, arm) = match slot_arm(slot_id) {
        Some((p, Arm::A)) => (Some(p), "A".to_string()),
        Some((p, Arm::B)) => (Some(p), "B".to_string()),
        None => (None, slot_id.to_string()),
    };
    let mut row = SlotRow {
        slot_id: slot_id.to_string(),
        pair,
        arm,
        state: "not_started".into(),
        disposition: None,
        check: None,
        check_refusal: None,
        generations: 0,
        tool_calls: 0,
        refused_tool_calls: 0,
        inclusive_input_tokens: None,
        output_tokens: None,
        cache_read_input_tokens: None,
        cache_creation_input_tokens: None,
        budget_cost_micro_usd: 0,
        host_wall_ms: None,
        prompt_status: None,
        usage_complete: false,
    };
    let Some(c) = capture else { return row };
    row.state = "captured".into();
    row.disposition = Some(c.disposition.clone());
    row.generations = c.generations.len() as u32;
    row.tool_calls = c.tools.len() as u32;
    row.refused_tool_calls = c.tools.iter().filter(|t| !t.admitted).count() as u32;
    row.host_wall_ms = Some(c.host_wall_ms);
    row.prompt_status = c.prompt_status.clone();
    let transmitted: Vec<&super::GenerationRecord> = c
        .generations
        .iter()
        .filter(|g| g.messages_attempt.is_some())
        .collect();
    row.usage_complete = transmitted
        .iter()
        .all(|g| g.usage.as_ref().is_some_and(|u| u.complete));
    let sum = |f: &dyn Fn(&super::UsageRecord) -> Option<u64>| -> Option<u64> {
        transmitted.iter().try_fold(0u64, |acc, g| {
            g.usage
                .as_ref()
                .and_then(f)
                .and_then(|v| acc.checked_add(v))
        })
    };
    let lex = |l: &Option<String>| l.as_deref().and_then(|t| t.parse::<u64>().ok());
    row.inclusive_input_tokens = sum(&|u| u.inclusive_input);
    row.output_tokens = sum(&|u| lex(&u.output));
    row.cache_read_input_tokens = sum(&|u| lex(&u.cache_read_input));
    row.cache_creation_input_tokens = sum(&|u| lex(&u.cache_creation_input));
    row.budget_cost_micro_usd = c.generations.iter().filter_map(|g| g.cost_micro_usd).sum();
    match sealed {
        Some(Ok(s)) => {
            row.state = "sealed".into();
            row.check = Some(s.outcome);
        }
        Some(Err(f)) => row.check_refusal = Some(f.code.to_string()),
        None => {}
    }
    row
}

fn summarize(
    mode: super::Mode,
    slots: Vec<SlotRow>,
    receipt: super::CaptureReceipt,
) -> PilotReport {
    let pairs = pins::PAIRS
        .iter()
        .map(|&pair| {
            let pass = |arm: &str| {
                slots
                    .iter()
                    .find(|r| r.pair == Some(pair) && r.arm == arm)
                    .and_then(|r| r.check)
                    .map(|c| c == super::CheckOutcome::Pass)
            };
            let (a, b) = (pass("A"), pass("B"));
            PairRow {
                pair,
                a_pass: a,
                b_pass: b,
                b_minus_a: match (a, b) {
                    (Some(a), Some(b)) => Some(b as i8 - a as i8),
                    _ => None,
                },
                valid: a.is_some() && b.is_some(),
            }
        })
        .collect();
    let arms = ["A", "B"]
        .iter()
        .map(|arm| {
            let rows: Vec<&SlotRow> = slots
                .iter()
                .filter(|r| r.arm == *arm && r.check.is_some())
                .collect();
            ArmSummary {
                arm: arm.to_string(),
                checked: rows.len() as u32,
                passed: rows
                    .iter()
                    .filter(|r| r.check == Some(super::CheckOutcome::Pass))
                    .count() as u32,
                host_wall_ms: spread(
                    &rows
                        .iter()
                        .filter_map(|r| r.host_wall_ms)
                        .collect::<Vec<_>>(),
                ),
                inclusive_input_tokens: spread(
                    &rows
                        .iter()
                        .filter(|r| r.usage_complete)
                        .filter_map(|r| r.inclusive_input_tokens)
                        .collect::<Vec<_>>(),
                ),
                output_tokens: spread(
                    &rows
                        .iter()
                        .filter(|r| r.usage_complete)
                        .filter_map(|r| r.output_tokens)
                        .collect::<Vec<_>>(),
                ),
            }
        })
        .collect();
    PilotReport {
        kind: "bounded-pilot-v1-report".into(),
        mode,
        slots,
        pairs,
        arms,
        receipt,
        claim_ceiling: "Descriptive only: one task repeated four times per arm; no significance, interval, noninferiority, causal savings, generalization, T006 acceptance, default promotion or invoiced-cost claim.".into(),
    }
}

/// Run the fixed-first transport probe on a fresh seed workspace and seal
/// it. A failed probe returns its capture with the seal refusal.
pub fn run_probe(
    session: &mut super::CaptureSession,
    root: &PilotRoot,
) -> Result<(super::SlotCapture, Result<super::ProbeSeal, CaptureFault>), CaptureFault> {
    let (workspace, sources) = match restore_workspace(root, "probe", Some("probe-before-run")) {
        Ok(restored) => restored,
        Err(fault) => {
            session.halt(fault);
            return Err(fault);
        }
    };
    let capture = session.run_slot(PreparedSlot {
        slot_id: policy::PROBE_SLOT.into(),
        workspace,
        initial_user_text: policy::PROBE_INSTRUCTION.into(),
        expected_sources: sources,
    })?;
    let seal = session.seal_probe(&capture);
    Ok((capture, seal))
}

/// Retain the independent result and apply it to the captured slot. Any
/// capture, receipt-write or sealing error uses the session's existing halt.
fn finish_candidate_check(
    session: &mut super::CaptureSession,
    root: &PilotRoot,
    capture: &super::SlotCapture,
    checked: Result<IndependentCheckReceipt, CaptureFault>,
) -> Result<super::SealedSlot, CaptureFault> {
    checked
        .and_then(|receipt| {
            verify_private_tree(
                &receipt.checker_root,
                &[Path::new("out/check")],
                &mut PrivateTreeBudget::new(None),
            )?;
            write_private_new(
                &root.root.join("checks").join(format!(
                    "{}-{}.receipt.json",
                    capture.slot_id, capture.host_id
                )),
                &serde_json::to_vec_pretty(&receipt)
                    .map_err(|_| fault(FaultClass::Persistence, "receipt_encode_failed"))?,
            )?;
            session.seal_slot(capture, &receipt)
        })
        .inspect_err(|fault| session.halt(*fault))
}

/// Run the eight frozen coding slots in order: restore each pair's canonical
/// workspace from the seed (archiving the previous arm), run the slot,
/// independently check the stopped candidate under containment and seal it.
/// Stops admitting slots once the session halts; every planned slot keeps
/// its row. Consumes the session and returns the descriptive report.
pub fn run_coding(
    mut session: super::CaptureSession,
    root: &PilotRoot,
    verified: &Verified,
) -> Result<PilotReport, CaptureFault> {
    let mode = session.mode();
    let mut rows = Vec::new();
    let mut previous: BTreeMap<u8, String> = BTreeMap::new();
    for slot_id in policy::SLOT_ORDER {
        let (pair, arm) = slot_arm(slot_id).ok_or(pin_fault("slot_unknown"))?;
        if session.halted().is_some() {
            rows.push(slot_row(slot_id, None, None));
            continue;
        }
        let archive = previous.get(&pair).map(|prev| format!("after-{prev}"));
        let (workspace, sources) =
            match restore_workspace(root, &format!("pair-{pair}"), archive.as_deref()) {
                Ok(restored) => restored,
                Err(fault) => {
                    session.halt(fault);
                    rows.push(slot_row(slot_id, None, None));
                    continue;
                }
            };
        let (view, _) = match load_view(root, pair) {
            Ok(value) => value,
            Err(fault) => {
                session.halt(fault);
                rows.push(slot_row(slot_id, None, None));
                continue;
            }
        };
        let prompt = arm_prompt(verified, arm, &view);
        let capture = match session.run_slot(PreparedSlot {
            slot_id: slot_id.to_string(),
            workspace: workspace.clone(),
            initial_user_text: prompt,
            expected_sources: sources,
        }) {
            Ok(c) => c,
            Err(_) => {
                rows.push(slot_row(slot_id, None, None));
                continue;
            }
        };
        if session.halted().is_some() {
            rows.push(slot_row(slot_id, Some(&capture), None));
            continue;
        }
        previous.insert(pair, slot_id.to_string());
        let order: Vec<String> = capture
            .candidate_inventory
            .iter()
            .map(|(r, _)| r.clone())
            .collect();
        let checked = run_independent_check(
            slot_id,
            &workspace,
            &order,
            &verified.oracle,
            &root
                .root
                .join("checks")
                .join(format!("{slot_id}-{}", capture.host_id)),
        );
        let sealed = finish_candidate_check(&mut session, root, &capture, checked);
        rows.push(slot_row(slot_id, Some(&capture), Some(&sealed)));
    }
    let receipt = session.finish()?;
    Ok(summarize(mode, rows, receipt))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;

    // Compatibility setup for NEW owned synthetic fixtures only. Existing
    // entries keep their modes, so deliberate negative mutations stay negative.
    // These helpers do not participate in product permission qualification.
    fn fixture_private_tempdir() -> std::io::Result<tempfile::TempDir> {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::Builder::new()
            .prefix("cf-owned-pilot-fixture-")
            .tempdir_in("/private/tmp")?;
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700))?;
        Ok(temp)
    }

    use super::*;

    /// Real local semantic producers through the production child launcher.
    /// Explicit test-only inputs avoid replacing the frozen v1 executable pin.
    #[test]
    #[ignore = "needs CF_PILOT_SEMANTIC_FOUNDRY, CF_PILOT_SEMANTIC_FOUNDRY_SHA256, CF_PILOT_SEMANTIC_PROFILE and CF_PILOT_SEMANTIC_EVIDENCE_ROOT; actual local Gemma, no provider"]
    fn privacy_real_semantic_outputs_from_the_bounded_launcher() {
        let path = |key| PathBuf::from(std::env::var_os(key).expect(key));
        let foundry = path("CF_PILOT_SEMANTIC_FOUNDRY");
        let profile = path("CF_PILOT_SEMANTIC_PROFILE");
        let evidence = path("CF_PILOT_SEMANTIC_EVIDENCE_ROOT");
        let expected_foundry = std::env::var("CF_PILOT_SEMANTIC_FOUNDRY_SHA256").unwrap();
        assert_eq!(file_sha256(&foundry).unwrap(), expected_foundry);
        assert_eq!(file_sha256(&profile).unwrap(), pins::PROFILE_SHA256);
        require_private_directory(&evidence).unwrap();
        let root = PilotRoot::create(&evidence.join("producer")).unwrap();
        eprintln!(
            "retained semantic permission evidence: {}",
            root.root.display()
        );
        // The outer test process must start with umask 0022. Public marker
        // bytes outside the producer root witness the unchanged parent mask.
        let before = evidence.join("ambient-before");
        fs::write(&before, b"public test marker").unwrap();
        assert_eq!(fs::metadata(&before).unwrap().mode() & 0o7777, 0o644);
        let workspace = root.workspace("semantic-fixture");
        create_private_dir(&workspace).unwrap();
        create_private_dir(&workspace.join("src")).unwrap();
        write_private_new(
            &workspace.join("src/lib.rs"),
            b"/// Preserve every input byte while decoding escaped sequences.\n\
              pub fn preserve_bytes(input: &[u8]) -> Vec<u8> { input.to_vec() }\n\
              /// Keep incomplete escape sequences unchanged.\n\
              pub fn incomplete_escape(input: &[u8]) -> &[u8] { input }\n",
        )
        .unwrap();
        write_private_new(
            &workspace.join("README.md"),
            b"# Byte preservation\nEscaped input must retain malformed and incomplete sequences.\n",
        )
        .unwrap();
        let source_files = list_files(&workspace).unwrap();
        let source_hashes = || {
            source_files
                .iter()
                .map(|relative| file_sha256(&workspace.join(relative)).unwrap())
                .collect::<Vec<_>>()
        };
        let source_before = source_hashes();
        let store = root.store(1);
        let environment = foundry_env(&root);
        let deadline = Instant::now() + Duration::from_secs(600);
        let execute = |name: &str, args: Vec<std::ffi::OsString>, limit: u64| {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(!remaining.is_zero());
            let argv: Vec<_> = args.iter().map(|value| value.as_os_str()).collect();
            let result = run_bounded(
                &foundry,
                &argv,
                &root.root,
                &environment,
                Duration::from_secs(limit).min(remaining),
            )
            .unwrap();
            write_private_new(
                &root.root.join("renders").join(format!("{name}.stdout")),
                &result.stdout,
            )
            .unwrap();
            assert_eq!(result.status, Some(0), "semantic producer {name}");
            assert!(!result.timed_out, "semantic producer {name} timed out");
            PilotRoot::open(&root.root)
                .unwrap_or_else(|fault| panic!("semantic producer {name} mode check: {fault:?}"));
            result.stdout
        };
        let index: Value = serde_json::from_slice(&execute(
            "index",
            vec![
                "index".into(),
                "--store".into(),
                store.clone().into(),
                workspace.clone().into(),
            ],
            60,
        ))
        .unwrap();
        assert_eq!(index["scan_complete"], true);
        assert_eq!(index["failures"], 0);
        let preparation: Value = serde_json::from_slice(&execute(
            "prepare",
            vec![
                "semantic".into(),
                "prepare".into(),
                "--store".into(),
                store.clone().into(),
                "--profile".into(),
                profile.clone().into(),
                "--budget-seconds".into(),
                "180".into(),
                "--development-isolation".into(),
            ],
            180,
        ))
        .unwrap();
        assert_eq!(preparation["partial"], false);
        assert_eq!(preparation["provider_state"], "ready");
        let eligible = preparation["coverage"]["eligible_units"].as_u64().unwrap();
        assert!(eligible > 0);
        assert_eq!(
            preparation["coverage"]["searchable_current_units"],
            eligible
        );
        assert!(preparation["embedded_units"].as_u64().unwrap() > 0);
        let context = execute(
            "context",
            vec![
                "context".into(),
                "--store".into(),
                store.into(),
                "--tokens".into(),
                "512".into(),
                "--semantic-profile".into(),
                profile.clone().into(),
                "--development-isolation".into(),
                "preserve escaped byte sequences without dropping input".into(),
            ],
            90,
        );
        let text = std::str::from_utf8(&context).unwrap();
        assert!(
            text.lines()
                .next()
                .unwrap()
                .split(" · ")
                .any(|s| s == "semantic:ready")
        );
        assert!(crate::response::count_tokens(text) <= 512);
        assert_eq!(list_files(&workspace).unwrap(), source_files);
        assert_eq!(source_hashes(), source_before);
        assert_eq!(file_sha256(&foundry).unwrap(), expected_foundry);
        assert_eq!(file_sha256(&profile).unwrap(), pins::PROFILE_SHA256);
        let after = evidence.join("ambient-after");
        fs::write(&after, b"public test marker").unwrap();
        assert_eq!(fs::metadata(after).unwrap().mode() & 0o7777, 0o644);
        write_private_new(
            &root.root.join("renders/semantic-producer.json"),
            &serde_json::to_vec_pretty(&serde_json::json!({
                "status": "passed", "eligible_units": eligible,
                "embedded_units": preparation["embedded_units"],
                "foundry_sha256": expected_foundry, "profile_sha256": pins::PROFILE_SHA256,
                "checked_after": ["index", "prepare", "context"],
                "parent_mask_before_after": "0022", "provider_calls": 0,
                "source_unchanged": true,
            }))
            .unwrap(),
        )
        .unwrap();
        PilotRoot::open(&root.root).unwrap();
    }

    #[test]
    fn slot_ids_map_to_pairs_and_arms() {
        assert_eq!(slot_arm("1B"), Some((1, Arm::B)));
        assert_eq!(slot_arm("4A"), Some((4, Arm::A)));
        assert_eq!(slot_arm("5A"), None);
        assert_eq!(slot_arm("1C"), None);
        assert_eq!(slot_arm("1AB"), None);
        assert!(policy::SLOT_ORDER.iter().all(|s| slot_arm(s).is_some()));
    }

    #[test]
    fn spreads_report_medians_and_ranges() {
        let s = spread(&[5, 1, 3]);
        assert_eq!(
            (s.n, s.median, s.min, s.max),
            (3, Some(3.0), Some(1), Some(5))
        );
        let e = spread(&[2, 4]);
        assert_eq!(e.median, Some(3.0));
        assert_eq!(spread(&[]).median, None);
    }

    #[test]
    fn citations_are_handles_only() {
        let view = "foundry context · r5 · budget:2048 · semantic:ready\nsrc/lib.rs#8812-9057@bb97304a4602082df298e81d6ac0b500.736d59a549b8aa20 L261-269 [semantic] fn x\n```rust\nfn x() {}\n```\n";
        assert_eq!(
            citations(view),
            vec![
                "src/lib.rs#8812-9057@bb97304a4602082df298e81d6ac0b500.736d59a549b8aa20"
                    .to_string()
            ]
        );
    }

    #[test]
    fn checker_policy_denies_network_and_home_but_allows_its_root() {
        let p = check_policy(Path::new("/x/checks/1B"), Path::new("/Users/u")).unwrap();
        assert!(p.starts_with("(version 1)(allow default)(deny network*)"));
        assert!(
            p.find("(deny file-read-data (subpath \"/Users/u\"))")
                .unwrap()
                < p.find("(allow file-read-data (subpath \"/x/checks/1B\"))")
                    .unwrap()
        );
        assert!(!p.contains("network-outbound"));
        assert!(check_policy(Path::new("rel"), Path::new("/Users/u")).is_err());
    }

    // These exercise the actual private helpers, not a new fake-checker API.
    struct OraclePrefixThenError {
        prefix: std::io::Cursor<Vec<u8>>,
    }

    impl std::io::Read for OraclePrefixThenError {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if out.is_empty() {
                return Ok(0);
            }
            let n = std::io::Read::read(&mut self.prefix, out)?;
            if n != 0 {
                Ok(n)
            } else {
                Err(std::io::Error::other(
                    "owned synthetic pipe read failure after prefix",
                ))
            }
        }
    }

    #[test]
    fn oracle_pipe_prefix_then_read_error_is_not_successful_eof() {
        // Public, owned bytes only; this is not the private oracle's success line.
        let prefix = b"owned completion-looking prefix\n";
        let error = check_output_sha256(OraclePrefixThenError {
            prefix: std::io::Cursor::new(prefix.to_vec()),
        })
        .unwrap_err();
        assert_eq!(error.class, FaultClass::CheckerProvenance);
        assert_eq!(error.code, "check_output_unbound");
        assert_eq!(
            check_output_sha256(std::io::Cursor::new(prefix)).unwrap(),
            sha256_hex(prefix)
        );
        assert_eq!(
            check_output_sha256(std::io::Cursor::new(b"")).unwrap(),
            sha256_hex(b"")
        );
    }

    #[test]
    fn oracle_checker_chain_error_halts_before_any_later_slot() {
        use crate::pilot_capture::{
            CaptureSession, Counters, Disposition, Inner, Journal, Mode, Scope, SlotCapture,
            planned_slots, runtime,
        };
        for failure in [
            "stdout-read",
            "stderr-read",
            "stderr-overflow",
            "stderr-persist",
            "receipt-persist",
            "stderr-halt-persist",
            "refusal-persist",
        ] {
            eprintln!("owned checker chain case: {failure}");
            let producer_failure = !matches!(failure, "receipt-persist" | "refusal-persist");
            let temp = fixture_private_tempdir().unwrap();
            let root = PilotRoot::create(&temp.path().join("pilot")).unwrap();
            let run_dir = root.root.join("runs/owned-check-chain");
            create_private_dir(&run_dir).unwrap();
            let hosts_dir = run_dir.join("hosts");
            create_private_dir(&hosts_dir).unwrap();
            let journal_path = run_dir.join("journal.jsonl");
            let mut journal = Journal::create(
                &journal_path,
                matches!(failure, "stderr-halt-persist" | "refusal-persist").then_some(1),
            )
            .unwrap();
            journal
                .append(
                    "admitted",
                    serde_json::json!({"synthetic_owner_fixture":true}),
                )
                .unwrap();
            // No transport, credential or runnable host. This isolates the same
            // checker-result -> terminal-halt -> remaining-slot guard used by run_coding.
            let mut session = CaptureSession {
                rt: runtime().unwrap(),
                inner: Inner {
                    mode: Mode::Verification,
                    scope: Scope::Coding,
                    run_dir: run_dir.clone(),
                    hosts_dir: hosts_dir.clone(),
                    omp: root.root.join("unused-omp"),
                    real_home: root.root.clone(),
                    deny_literals: Vec::new(),
                    journal,
                    transport: None,
                    manifest: None,
                    session_id: "owned-check-chain".into(),
                    host_seq: 0,
                    attempt_seq: 0,
                    counters: Counters::default(),
                    paid_started_ns: None,
                    paid_deadline_ns: None,
                    boot_identity: String::new(),
                    halt: None,
                    halt_record_error: None,
                    slot_index: 0,
                    awaiting_seal: None,
                    summaries: planned_slots(Scope::Coding),
                    probe_sealed: false,
                },
            };
            let capture = SlotCapture {
                slot_id: "1B".into(),
                host_id: "owned-capture".into(),
                disposition: Disposition::Completed {
                    terminal_status: "synthetic-only".into(),
                },
                render: None,
                generations: Vec::new(),
                tools: Vec::new(),
                candidate_inventory: Vec::new(),
                native: None,
                host_exit: None,
                host_wall_ms: 0,
                prompt_status: None,
            };
            if failure == "refusal-persist" {
                session.inner.awaiting_seal = Some(capture.clone());
            }
            let receipt_path = root.root.join("checks/1B-owned-capture.receipt.json");
            let (input, expected_class, expected_code) = if producer_failure {
                let error = match failure {
                    "stdout-read" => check_output_sha256(OraclePrefixThenError {
                        prefix: std::io::Cursor::new(b"owned prefix before error".to_vec()),
                    })
                    .unwrap_err(),
                    "stderr-read" | "stderr-halt-persist" | "stderr-overflow" => {
                        let (error, bytes) = if failure == "stderr-overflow" {
                            check_stderr_bytes(std::io::Cursor::new(vec![b'x'; 1_048_577]))
                                .unwrap_err()
                        } else {
                            check_stderr_bytes(OraclePrefixThenError {
                                prefix: std::io::Cursor::new(
                                    b"owned stderr prefix before error".to_vec(),
                                ),
                            })
                            .unwrap_err()
                        };
                        let diagnostics = root.root.join("checks/owned-stderr");
                        create_private_dir(&diagnostics).unwrap();
                        create_private_dir(&diagnostics.join("diagnostics")).unwrap();
                        let relative =
                            retain_check_stderr(&diagnostics, "noalloc_compile", &bytes, true)
                                .unwrap();
                        checker_assert_retained(&diagnostics, &relative, &bytes);
                        checker_assert_retained(
                            &diagnostics,
                            &format!("{relative}.sha256"),
                            format!("{}\n", sha256_hex(&bytes)).as_bytes(),
                        );
                        error
                    }
                    "stderr-persist" => {
                        let diagnostics = root.root.join("checks/owned-stderr");
                        create_private_dir(&diagnostics).unwrap();
                        create_private_dir(&diagnostics.join("diagnostics")).unwrap();
                        let path = diagnostics.join("diagnostics/noalloc_compile.stderr");
                        write_private_new(&path, b"owned sentinel").unwrap();
                        let error = retain_check_stderr(
                            &diagnostics,
                            "noalloc_compile",
                            b"new stderr",
                            false,
                        )
                        .unwrap_err();
                        assert_eq!(fs::read(path).unwrap(), b"owned sentinel");
                        error
                    }
                    _ => unreachable!(),
                };
                let expected = match failure {
                    "stdout-read" => (FaultClass::CheckerProvenance, "check_output_unbound"),
                    "stderr-read" | "stderr-halt-persist" => {
                        (FaultClass::CheckerProvenance, "check_stderr_read_failed")
                    }
                    "stderr-overflow" => (FaultClass::CheckerProvenance, "check_stderr_overflow"),
                    "stderr-persist" => (FaultClass::Persistence, "private_file_create_failed"),
                    _ => unreachable!(),
                };
                (Err(error), expected.0, expected.1)
            } else {
                // Exclusive receipt creation must fail without replacing this
                // owned sentinel. The real helper must halt on this path too.
                if failure == "receipt-persist" {
                    create_private_dir(&receipt_path).unwrap();
                }
                // This fixture must now pass the new whole-checker privacy
                // gate before reaching its existing persistence assertion.
                create_private_dir(&root.root.join("checks/unused-checker")).unwrap();
                (
                    Ok(IndependentCheckReceipt {
                        kind: "bounded-pilot-v1-check-v2".into(),
                        slot_id: capture.slot_id.clone(),
                        checker_root: root.root.join("checks/unused-checker"),
                        candidate: Vec::new(),
                        oracle_sha256: policy::ORACLE_SHA256.into(),
                        compiler: PathBuf::from(policy::RUSTC),
                        compiler_sha256: policy::RUSTC_SHA256.into(),
                        compiler_version: "rustc 1.97.1 (owned fixture; not executed)".into(),
                        environment: std::collections::BTreeMap::new(),
                        sandbox_profile: String::new(),
                        steps: Vec::new(),
                        fixture: true,
                    }),
                    FaultClass::Persistence,
                    if failure == "refusal-persist" {
                        "journal_append_failed"
                    } else {
                        "private_file_create_failed"
                    },
                )
            };
            let fault = finish_candidate_check(&mut session, &root, &capture, input).unwrap_err();
            assert_eq!((fault.class, fault.code), (expected_class, expected_code));
            assert_eq!(session.halted(), Some(fault));
            if producer_failure {
                assert!(!receipt_path.exists());
            } else if failure == "refusal-persist" {
                assert!(receipt_path.is_file());
                let persisted: IndependentCheckReceipt =
                    serde_json::from_slice(&fs::read(&receipt_path).unwrap()).unwrap();
                assert!(
                    persisted.candidate.is_empty(),
                    "owned invalid candidate reaches the real refusal append"
                );
            } else {
                assert!(receipt_path.is_dir());
                assert_eq!(fs::read_dir(&receipt_path).unwrap().count(), 0);
            }
            let verified = Verified {
                inputs: EvidenceInputs {
                    paired_root: root.root.join("unused-paired"),
                    freeze_root: root.root.join("unused-freeze"),
                    foundry: root.root.join("unused-foundry"),
                    profile: root.root.join("unused-profile"),
                    omp: root.root.join("unused-omp"),
                },
                pins: Vec::new(),
                task: String::new(),
                arm_a_prompt: String::new(),
                oracle: root.root.join("unused-oracle"),
                seed: Vec::new(),
            };
            // A missing seed/host would fail if run_coding tried even one slot.
            // Its halted branch must leave all workspace/host paths untouched.
            if matches!(failure, "stderr-halt-persist" | "refusal-persist") {
                let before = fs::read(&journal_path).unwrap();
                session.inner.journal.fail_after = None;
                let error = run_coding(session, &root, &verified).err().unwrap();
                assert_eq!(
                    (error.class, error.code),
                    (FaultClass::Persistence, "journal_append_failed")
                );
                assert_eq!(fs::read(&journal_path).unwrap(), before);
                assert_eq!(fs::read_dir(&hosts_dir).unwrap().count(), 0);
                for pair in pins::PAIRS {
                    assert!(!root.workspace(&format!("pair-{pair}")).exists());
                }
                continue;
            }
            let report = run_coding(session, &root, &verified).unwrap();
            assert_eq!(
                report.receipt.halt,
                Some((expected_class.as_str().into(), expected_code.into()))
            );
            assert_eq!(report.receipt.counters, Counters::default());
            assert_eq!(report.slots.len(), policy::SLOT_ORDER.len());
            assert!(report.slots.iter().all(|slot| slot.state == "not_started"));
            assert!(
                report
                    .receipt
                    .slots
                    .iter()
                    .all(|slot| slot.state == "not_started")
            );
            assert!(
                report
                    .arms
                    .iter()
                    .all(|arm| arm.checked == 0 && arm.passed == 0)
            );
            assert_eq!(fs::read_dir(&hosts_dir).unwrap().count(), 0);
            for pair in pins::PAIRS {
                assert!(!root.workspace(&format!("pair-{pair}")).exists());
            }
            let rows: Vec<Value> = fs::read_to_string(&journal_path)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert_eq!(
                rows.iter()
                    .map(|row| row["kind"].as_str().unwrap())
                    .collect::<Vec<_>>(),
                ["admitted", "halt", "finished"]
            );
            assert_eq!(rows[1]["class"], expected_class.as_str());
            assert_eq!(rows[1]["code"], expected_code);
            assert_eq!(
                report.receipt.journal_sha256,
                sha256_hex(&fs::read(&journal_path).unwrap())
            );
        }
    }
    fn checker_owned_diagnostics_root(label: &str) -> PathBuf {
        let temp = tempfile::Builder::new()
            .prefix("cf-checker-pipes-")
            .tempdir()
            .unwrap();
        let base = fs::canonicalize(temp.path()).unwrap();
        std::mem::forget(temp);
        let root = base.join(label);
        create_private_dir(&root).unwrap();
        create_private_dir(&root.join("diagnostics")).unwrap();
        eprintln!("owned checker pipe evidence: {}", root.display());
        root
    }

    fn checker_assert_retained(root: &Path, relative: &str, expected: &[u8]) {
        use std::os::unix::fs::MetadataExt;
        let path = root.join(relative);
        let meta = fs::symlink_metadata(&path).unwrap();
        assert!(meta.is_file() && !meta.file_type().is_symlink());
        assert_eq!(meta.mode() & 0o7777, 0o600);
        assert_eq!(meta.uid(), unsafe { libc::getuid() });
        assert_eq!(meta.len(), expected.len() as u64);
        assert_eq!(fs::read(path).unwrap(), expected);
    }

    #[test]
    fn checker_stderr_reader_requires_eof_and_caps_retained_prefix() {
        assert_eq!(CHECK_STDERR_MAX_BYTES, 1_048_576);
        for bytes in [
            Vec::new(),
            b"owned\xff\0diagnostic\n".to_vec(),
            vec![b'x'; 1_048_576],
        ] {
            assert_eq!(
                check_stderr_bytes(std::io::Cursor::new(&bytes)).unwrap(),
                bytes
            );
        }
        let oversized = vec![b'x'; 1_048_577];
        let overflow = check_stderr_bytes(std::io::Cursor::new(oversized));
        assert!(
            overflow.is_err(),
            "stderr beyond the cap must be a capture error"
        );
        let (fault, prefix) = overflow.unwrap_err();
        assert_eq!(
            (fault.class, fault.code),
            (FaultClass::CheckerProvenance, "check_stderr_overflow")
        );
        assert_eq!(prefix, vec![b'x'; 1_048_576]);
        let original = b"owned diagnostic before read error\xff".to_vec();
        let (fault, prefix) = check_stderr_bytes(OraclePrefixThenError {
            prefix: std::io::Cursor::new(original.clone()),
        })
        .unwrap_err();
        assert_eq!(
            (fault.class, fault.code),
            (FaultClass::CheckerProvenance, "check_stderr_read_failed")
        );
        assert_eq!(prefix, original);
    }

    #[test]
    fn checker_stderr_retention_is_private_exact_and_exclusive() {
        let root = checker_owned_diagnostics_root("retention");
        let bytes = b"owned diagnostic\xff\0\n";
        let complete = retain_check_stderr(&root, "noalloc_compile", bytes, false).unwrap();
        assert_eq!(complete, "diagnostics/noalloc_compile.stderr");
        checker_assert_retained(&root, &complete, bytes);
        let partial = retain_check_stderr(&root, "alloc_compile", bytes, true).unwrap();
        assert_eq!(partial, "diagnostics/alloc_compile.partial.stderr");
        checker_assert_retained(&root, &partial, bytes);
        checker_assert_retained(
            &root,
            &format!("{partial}.sha256"),
            format!("{}\n", sha256_hex(bytes)).as_bytes(),
        );
        for (name, partial, relative) in [
            ("noalloc_compile", false, complete),
            ("alloc_compile", true, partial),
        ] {
            let fault =
                retain_check_stderr(&root, name, b"replacement must not overwrite", partial)
                    .unwrap_err();
            assert_eq!(
                (fault.class, fault.code),
                (FaultClass::Persistence, "private_file_create_failed")
            );
            checker_assert_retained(&root, &relative, bytes);
        }
        // Failure after prefix creation cannot be called successfully retained.
        let digest = root.join("diagnostics/oracle_compile.partial.stderr.sha256");
        write_private_new(&digest, b"owned sentinel digest").unwrap();
        let fault = retain_check_stderr(&root, "oracle_compile", bytes, true).unwrap_err();
        assert_eq!(fault.code, "private_file_create_failed");
        checker_assert_retained(&root, "diagnostics/oracle_compile.partial.stderr", bytes);
        assert_eq!(fs::read(digest).unwrap(), b"owned sentinel digest");
    }

    // Defensive cleanup on a failed assertion. This guard can signal only the
    // fresh group whose leader wrote its PID into this test's private root.
    struct CheckerOwnedGroupGuard(PathBuf);
    impl Drop for CheckerOwnedGroupGuard {
        fn drop(&mut self) {
            if let Ok(text) = fs::read_to_string(self.0.join("leader.pid"))
                && let Ok(pid) = text.trim().parse::<i32>()
                && pid > 1
            {
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                }
            }
        }
    }

    fn checker_assert_group_gone(root: &Path) {
        let pid: i32 = fs::read_to_string(root.join("leader.pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            let result = unsafe { libc::kill(-pid, 0) };
            if result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                break;
            }
            assert!(
                Instant::now() < until,
                "owned checker group {pid} survived cleanup"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn checker_owned_shell(root: &Path, script: &str) -> Command {
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c")
            .arg(script)
            .current_dir(root)
            .env_clear()
            .env("PATH", "/usr/bin:/bin");
        cmd
    }

    #[test]
    fn checker_real_step_preserves_complete_stderr_and_ordinary_nonzero() {
        for exit in [0, 9] {
            let root = checker_owned_diagnostics_root(&format!("complete-{exit}"));
            let _guard = CheckerOwnedGroupGuard(root.clone());
            let mut cmd = checker_owned_shell(
                &root,
                &format!(
                    "echo $$ > leader.pid; printf 'owned stdout'; printf '\\377\\000owned diagnostic\\n' >&2; exit {exit}"
                ),
            );
            let output = run_check_step(
                &mut cmd,
                &root,
                "noalloc_compile",
                Instant::now() + Duration::from_secs(3),
            )
            .unwrap();
            assert_eq!(output.status.code(), Some(exit));
            assert_eq!(output.stdout_sha256, sha256_hex(b"owned stdout"));
            assert_eq!(output.stderr_path, "diagnostics/noalloc_compile.stderr");
            assert_eq!(
                output.stderr_bytes,
                b"\xff\0owned diagnostic\n".len() as u64
            );
            assert_eq!(
                output.stderr_sha256,
                sha256_hex(b"\xff\0owned diagnostic\n")
            );
            checker_assert_retained(&root, &output.stderr_path, b"\xff\0owned diagnostic\n");
            checker_assert_group_gone(&root);
        }
    }

    #[test]
    fn checker_real_step_overflow_is_not_a_nonzero_result_and_cleans_owned_group() {
        let root = checker_owned_diagnostics_root("overflow");
        let _guard = CheckerOwnedGroupGuard(root.clone());
        let mut cmd = checker_owned_shell(
            &root,
            "echo $$ > leader.pid; /usr/bin/yes owned-diagnostic >&2",
        );
        let started = Instant::now();
        let fault = run_check_step(
            &mut cmd,
            &root,
            "noalloc_compile",
            started + Duration::from_secs(3),
        )
        .err()
        .unwrap();
        assert_eq!(
            (fault.class, fault.code),
            (FaultClass::CheckerProvenance, "check_stderr_overflow")
        );
        assert!(started.elapsed() < Duration::from_secs(3));
        checker_assert_group_gone(&root);
        let relative = "diagnostics/noalloc_compile.partial.stderr";
        let bytes = fs::read(root.join(relative)).unwrap();
        assert_eq!(bytes.len(), 1_048_576);
        checker_assert_retained(&root, relative, &bytes);
        checker_assert_retained(
            &root,
            &format!("{relative}.sha256"),
            format!("{}\n", sha256_hex(&bytes)).as_bytes(),
        );
        assert!(!root.join("diagnostics/noalloc_compile.stderr").exists());
    }

    #[test]
    fn checker_real_step_deadline_covers_descendant_held_pipe_after_leader_exit() {
        let root = checker_owned_diagnostics_root("held-pipe");
        let _guard = CheckerOwnedGroupGuard(root.clone());
        // The leader exits zero immediately, while its owned same-group child
        // retains both pipes for five seconds unless the production owner kills it.
        let mut cmd = checker_owned_shell(
            &root,
            "echo $$ > leader.pid; /bin/sleep 5 & echo $! > descendant.pid; printf 'owned held pipe' >&2; exit 0",
        );
        let started = Instant::now();
        let fault = run_check_step(
            &mut cmd,
            &root,
            "noalloc_compile",
            started + Duration::from_millis(500),
        )
        .err()
        .unwrap();
        assert_eq!(
            (fault.class, fault.code),
            (FaultClass::CheckerProvenance, "post_check_wall_exceeded")
        );
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "leader-only deadline left reader joins unbounded"
        );
        checker_assert_group_gone(&root);
        let descendant: i32 = fs::read_to_string(root.join("descendant.pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(
            unsafe { libc::kill(descendant, 0) },
            -1,
            "owned descendant survived"
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
        let relative = "diagnostics/noalloc_compile.partial.stderr";
        checker_assert_retained(&root, relative, b"owned held pipe");
        checker_assert_retained(
            &root,
            &format!("{relative}.sha256"),
            format!("{}\n", sha256_hex(b"owned held pipe")).as_bytes(),
        );
        assert!(!root.join("diagnostics/noalloc_compile.stderr").exists());
    }

    #[test]
    fn checker_real_step_diagnostic_persistence_failure_returns_error_without_overwrite() {
        let root = checker_owned_diagnostics_root("persistence");
        let _guard = CheckerOwnedGroupGuard(root.clone());
        let path = root.join("diagnostics/noalloc_compile.stderr");
        write_private_new(&path, b"owned sentinel").unwrap();
        let mut cmd = checker_owned_shell(
            &root,
            "echo $$ > leader.pid; printf 'new diagnostic' >&2; exit 0",
        );
        let fault = run_check_step(
            &mut cmd,
            &root,
            "noalloc_compile",
            Instant::now() + Duration::from_secs(3),
        )
        .err()
        .unwrap();
        assert_eq!(
            (fault.class, fault.code),
            (FaultClass::Persistence, "private_file_create_failed")
        );
        assert_eq!(fs::read(path).unwrap(), b"owned sentinel");
        checker_assert_group_gone(&root);
    }
    #[test]
    fn privacy_copy_creates_private_descendants_without_changing_source_bytes_or_modes() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let temp = fixture_private_tempdir().unwrap();
        let base = fs::canonicalize(temp.path()).unwrap();
        let source = base.join("source");
        fs::create_dir_all(source.join("a/b/c")).unwrap();
        let file = source.join("a/b/c/input.rs");
        fs::write(&file, b"pub fn owned_copy() {}\n").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(source.join("a"), fs::Permissions::from_mode(0o750)).unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o444)).unwrap();
        let originals: Vec<_> = [
            source.clone(),
            source.join("a"),
            source.join("a/b"),
            source.join("a/b/c"),
            file.clone(),
        ]
        .into_iter()
        .map(|path| {
            let mode = fs::metadata(&path).unwrap().mode();
            (path, mode)
        })
        .collect();
        let destination = base.join("copy");
        copy_tree(&source, &destination).unwrap();
        verify_private_tree(&destination, &[], &mut PrivateTreeBudget::new(None)).unwrap();
        assert_eq!(
            fs::read(destination.join("a/b/c/input.rs")).unwrap(),
            b"pub fn owned_copy() {}\n"
        );
        assert_eq!(fs::read(&file).unwrap(), b"pub fn owned_copy() {}\n");
        for (path, mode) in originals {
            assert_eq!(fs::metadata(path).unwrap().mode(), mode);
        }
    }

    #[test]
    fn privacy_copy_refuses_links_without_reading_or_changing_the_owned_outside_target() {
        use std::os::unix::fs::{MetadataExt, symlink};
        let temp = fixture_private_tempdir().unwrap();
        let base = fs::canonicalize(temp.path()).unwrap();
        let source = base.join("source");
        create_private_dir(&source).unwrap();
        let outside = base.join("outside");
        write_private_new(&outside, b"owned outside sentinel").unwrap();
        let mode = fs::metadata(&outside).unwrap().mode();
        symlink(&outside, source.join("link")).unwrap();
        let destination = base.join("copy");
        assert!(copy_tree(&source, &destination).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"owned outside sentinel");
        assert_eq!(fs::metadata(outside).unwrap().mode(), mode);
        assert!(
            fs::symlink_metadata(source.join("link"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(!destination.join("link").exists());
    }

    #[test]
    fn privacy_restore_refuses_old_workspace_before_archive_or_seed_replacement() {
        use std::os::unix::fs::PermissionsExt;
        let temp = fixture_private_tempdir().unwrap();
        let root = PilotRoot::create(&temp.path().join("pilot")).unwrap();
        let workspace = root.workspace("owned-old");
        create_private_dir(&workspace).unwrap();
        let file = workspace.join("retained");
        write_private_new(&file, b"owned old-workspace evidence").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
        let fault = restore_workspace(&root, "owned-old", Some("must-not-be-created")).unwrap_err();
        assert_eq!(
            (fault.class, fault.code),
            (FaultClass::Privacy, "private_artifact_not_private")
        );
        assert_eq!(fs::read(&file).unwrap(), b"owned old-workspace evidence");
        assert_eq!(fs::metadata(file).unwrap().mode() & 0o7777, 0o640);
        assert!(!root.root.join("archive/must-not-be-created").exists());
        assert!(workspace.is_dir());
        // The seed is deliberately empty. This proves the new earlier privacy
        // gate, not successful restoration of the private frozen seed.
    }

    #[test]
    fn privacy_pilot_root_applies_the_executable_exception_only_to_checker_children() {
        use std::os::unix::fs::PermissionsExt;
        let temp = fixture_private_tempdir().unwrap();
        let root = PilotRoot::create(&temp.path().join("pilot")).unwrap();
        let checker = root.root.join("checks/owned-checker");
        create_private_dir(&checker).unwrap();
        create_private_dir(&checker.join("out")).unwrap();
        write_private_new(&checker.join("out/check"), b"owned inert checker image").unwrap();
        fs::set_permissions(checker.join("out/check"), fs::Permissions::from_mode(0o700)).unwrap();
        PilotRoot::open(&root.root).unwrap();
        let store = root.store(1);
        create_private_dir(&store).unwrap();
        create_private_dir(&store.join("out")).unwrap();
        write_private_new(&store.join("out/check"), b"owned inert store file").unwrap();
        fs::set_permissions(store.join("out/check"), fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            PilotRoot::open(&root.root).is_err(),
            "same basename in a store is not a checker executable"
        );
    }

    #[test]
    fn privacy_actual_launchers_set_child_only_masks_under_a_permissive_parent() {
        use std::os::unix::process::CommandExt;
        const CHILD: &str = "CF_PRIVATE_LAUNCHER_TEST_CHILD";
        if let Some(path) = std::env::var_os(CHILD) {
            let base = PathBuf::from(path);
            std::fs::write(base.join("ambient-before"), []).unwrap();
            let ambient_mode = std::fs::metadata(base.join("ambient-before"))
                .unwrap()
                .mode()
                & 0o7777;
            let source_root = base.join("bounded");
            create_private_dir(&source_root).unwrap();
            let script =
                std::ffi::OsStr::new("umask; mkdir made-directory; printf owned > made-file");
            let result = run_bounded(
                Path::new("/bin/sh"),
                &[std::ffi::OsStr::new("-c"), script],
                &source_root,
                &[("PATH", "/usr/bin:/bin".into())],
                Duration::from_secs(3),
            )
            .unwrap();
            assert_eq!(result.status, Some(0));
            assert!(!result.timed_out);
            assert_eq!(
                u32::from_str_radix(std::str::from_utf8(&result.stdout).unwrap().trim(), 8)
                    .unwrap(),
                0o77
            );
            verify_private_tree(&source_root, &[], &mut PrivateTreeBudget::new(None)).unwrap();
            let checker = base.join("checker");
            create_private_dir(&checker).unwrap();
            create_private_dir(&checker.join("diagnostics")).unwrap();
            let mut cmd = Command::new("/bin/sh");
            cmd.args([
                "-c",
                "umask; mkdir made-directory; printf owned > made-file",
            ])
            .current_dir(&checker)
            .env_clear()
            .env("PATH", "/usr/bin:/bin");
            let result = run_check_step(
                &mut cmd,
                &checker,
                "noalloc_compile",
                Instant::now() + Duration::from_secs(3),
            )
            .unwrap();
            assert!(result.status.success());
            // Shell umask output formatting is platform-specific; the first
            // launcher observed numeric0077, and these actual artifacts bind
            // the second launcher's effect independently of its stdout hash.
            verify_private_tree(&checker, &[], &mut PrivateTreeBudget::new(None)).unwrap();
            assert_eq!(
                fs::metadata(checker.join("made-file")).unwrap().mode() & 0o7777,
                0o600
            );
            assert_eq!(
                fs::metadata(checker.join("made-directory")).unwrap().mode() & 0o7777,
                0o700
            );
            std::fs::write(base.join("ambient-after"), []).unwrap();
            assert_eq!(
                std::fs::metadata(base.join("ambient-after"))
                    .unwrap()
                    .mode()
                    & 0o7777,
                ambient_mode,
                "product operation changed its caller mask"
            );
            return;
        }
        let temp = fixture_private_tempdir().unwrap();
        let base = fs::canonicalize(temp.path()).unwrap();
        std::fs::write(base.join("parent-before"), b"owned parent mask witness").unwrap();
        let before = fs::metadata(base.join("parent-before")).unwrap().mode() & 0o7777;
        let mut child = Command::new(std::env::current_exe().unwrap());
        child.args(["--exact","pilot_capture::apparatus::tests::privacy_actual_launchers_set_child_only_masks_under_a_permissive_parent","--nocapture"]).env(CHILD,&base);
        unsafe {
            child.pre_exec(|| {
                libc::umask(0);
                Ok(())
            });
        }
        assert!(child.status().unwrap().success());
        std::fs::write(base.join("parent-after"), b"owned parent mask witness").unwrap();
        assert_eq!(
            fs::metadata(base.join("parent-after")).unwrap().mode() & 0o7777,
            before
        );
    }

    #[test]
    fn privacy_actual_checker_steps_create_private_rlib_and_executable() {
        let root = checker_owned_diagnostics_root("actual-rust-modes");
        for d in ["out", "home", "tmp"] {
            create_private_dir(&root.join(d)).unwrap();
        }
        assert_eq!(
            file_sha256(Path::new(policy::RUSTC)).unwrap(),
            policy::RUSTC_SHA256
        );
        let library = root.join("owned-lib.rs");
        write_private_new(&library, b"pub fn owned_value() -> u8 { 7 }\n").unwrap();
        let binary = root.join("owned-main.rs");
        write_private_new(
            &binary,
            b"fn main() { println!(\"owned private checker\"); }\n",
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        for (name, input, kind, output) in [
            (
                "noalloc_compile",
                &library,
                "lib",
                root.join("out/owned.rlib"),
            ),
            ("oracle_compile", &binary, "bin", root.join("out/check")),
        ] {
            let mut cmd = Command::new(policy::RUSTC);
            cmd.arg(input)
                .args([
                    "--edition=2021",
                    "--crate-name",
                    "owned_private_fixture",
                    "--crate-type",
                    kind,
                    "-o",
                ])
                .arg(output)
                .current_dir(&root)
                .env_clear()
                .envs(check_environment(&root).unwrap());
            let result = run_check_step(&mut cmd, &root, name, deadline).unwrap();
            assert!(result.status.success());
            verify_private_tree(
                &root,
                &[Path::new("out/check")],
                &mut PrivateTreeBudget::new(Some(deadline)),
            )
            .unwrap();
        }
        assert_eq!(
            fs::metadata(root.join("out/owned.rlib")).unwrap().mode() & 0o7777,
            0o600
        );
        assert_eq!(
            fs::metadata(root.join("out/check")).unwrap().mode() & 0o7777,
            0o700
        );
        // This is the actual per-step runner and compiler, with public owned
        // source. The existing ignored producer test covers the frozen oracle.
    }
    #[test]
    fn privacy_pilot_root_native_exception_is_limited_to_exact_run_host_locations() {
        use std::os::unix::fs::PermissionsExt;
        const NATIVE: &str = "home/.omp/natives/18.13.2/pi_natives.darwin-arm64.node";
        for case in [
            "bound",
            "wrong-version",
            "nested-location",
            "store",
            "workspace",
            "archive",
        ] {
            eprintln!("owned native PilotRoot metadata case: {case}");
            let temp = fixture_private_tempdir().unwrap();
            let root = PilotRoot::create(&temp.path().join("pilot")).unwrap();
            let relative = match case {
                "bound" => format!("runs/run1/hosts/h1/{NATIVE}"),
                "wrong-version" => format!(
                    "runs/run1/hosts/h1/{}",
                    NATIVE.replace("18.13.2", "18.13.3")
                ),
                "nested-location" => format!("runs/run1/nested/hosts/h1/{NATIVE}"),
                "store" => format!("stores/pair-1/hosts/h1/{NATIVE}"),
                "workspace" => format!("workspaces/pair-1/hosts/h1/{NATIVE}"),
                "archive" => format!("archive/run1/hosts/h1/{NATIVE}"),
                _ => unreachable!(),
            };
            let mut parent = root.root.clone();
            for component in Path::new(&relative).parent().unwrap().components() {
                parent.push(component);
                if !parent.exists() {
                    create_private_dir(&parent).unwrap();
                }
            }
            let path = root.root.join(relative);
            write_private_new(&path, b"owned synthetic native metadata fixture").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            let result = PilotRoot::open(&root.root);
            if case == "bound" {
                result.unwrap();
            } else {
                assert_eq!(result.unwrap_err().code, "private_artifact_not_private");
            }
        }
    }
}
