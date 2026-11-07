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
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::protocol::FaultClass;
use super::{
    CaptureFault, CheckStep, IndependentCheckReceipt, LaunchManifest, ManifestSlot, ManifestView,
    PinEntry, PreparedSlot, PreviewAdmission, RenderCommitments, create_private_dir, policy,
    render_binding, sha256_hex, write_private_new,
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
        create_private_dir(path)?;
        let root = fs::canonicalize(path)
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
        for d in std::iter::once(".").chain(ROOT_DIRS) {
            let meta = fs::symlink_metadata(root.join(d))
                .map_err(|_| fault(FaultClass::Authority, "pilot_root_incomplete"))?;
            // SAFETY: getuid has no failure mode.
            let uid = unsafe { libc::getuid() };
            if !meta.is_dir() || meta.mode() & 0o077 != 0 || meta.uid() != uid {
                return Err(fault(FaultClass::Authority, "pilot_root_not_private"));
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
        let bytes = fs::read(src).map_err(|_| pin_fault("seed_unreadable"))?;
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
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).map_err(|_| fault(FaultClass::Containment, "tree_unreadable"))? {
            let p = e
                .map_err(|_| fault(FaultClass::Containment, "tree_unreadable"))?
                .path();
            let meta = fs::symlink_metadata(&p)
                .map_err(|_| fault(FaultClass::Containment, "tree_unreadable"))?;
            if meta.is_dir() {
                stack.push(p);
            } else {
                let rel = p
                    .strip_prefix(dir)
                    .map_err(|_| fault(FaultClass::Containment, "tree_unreadable"))?
                    .to_string_lossy()
                    .into_owned();
                out.push(rel);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), CaptureFault> {
    create_private_dir(to)?;
    for rel in list_files(from)? {
        let dest = to.join(&rel);
        if let Some(parent) = dest.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)
                .map_err(|_| fault(FaultClass::Persistence, "copy_failed"))?;
            fs::set_permissions(parent, std::os::unix::fs::PermissionsExt::from_mode(0o700))
                .map_err(|_| fault(FaultClass::Persistence, "copy_failed"))?;
        }
        let bytes =
            fs::read(from.join(&rel)).map_err(|_| fault(FaultClass::Persistence, "copy_failed"))?;
        write_private_new(&dest, &bytes)?;
    }
    Ok(())
}

/// Restore the canonical workspace `name` from the seed. An existing arm's
/// workspace is first archived unchanged under `archive/<archive_as>`.
/// Returns the restored sources (relative path, sha256) in frozen order.
pub fn restore_workspace(
    root: &PilotRoot,
    name: &str,
    archive_as: Option<&str>,
) -> Result<(PathBuf, Vec<(String, String)>), CaptureFault> {
    check_seed_tree(&root.seed())?;
    let ws = root.workspace(name);
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
    let record: ViewRecord = serde_json::from_slice(
        &fs::read(root.view_record(pair))
            .map_err(|_| fault(FaultClass::Authority, "view_record_missing"))?,
    )
    .map_err(|_| fault(FaultClass::Authority, "view_record_invalid"))?;
    let view = fs::read_to_string(root.view(pair))
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
) -> Result<(LaunchManifest, LaunchManifest), CaptureFault> {
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
        kind: "bounded-pilot-v1-launch".into(),
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

const CHECK_ENV_NAMES: [&str; 6] = [
    "HOME",
    "LANG",
    "PATH",
    "TMPDIR",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
];

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

/// The checker's sandbox: no network, the owner's home unreadable and
/// unwritable except the private checker root, no writes outside it.
fn check_policy(root: &Path, real_home: &Path) -> Result<String, CaptureFault> {
    let (root, home) = (sbpl(root)?, sbpl(real_home)?);
    Ok(format!(
        "(version 1)(allow default)(deny network*)\
         (deny file-read-data (subpath \"{home}\"))(deny file-write* (subpath \"{home}\"))\
         (deny file-write* (subpath \"/private/tmp\"))(deny file-write* (subpath \"/private/var/folders\"))\
         (allow file-read-data (subpath \"{root}\"))(allow file-write* (subpath \"{root}\"))"
    ))
}

/// Copy the stopped host's candidate and the private oracle into a new
/// checker root outside agent access, then run the accepted four-step recipe
/// under containment with a minimal no-credential environment and the 60 s
/// post-check ceiling. Untrusted candidate code only ever runs inside the
/// sandbox. The result binds slot, candidate, oracle, compiler, argv,
/// environment names, exits and output/artifact digests.
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
    for d in ["src", "out", "home", "tmp"] {
        create_private_dir(&root.join(d))?;
    }
    let mut candidate = Vec::new();
    for rel in candidate_order {
        let src = workspace.join(rel);
        let meta = fs::symlink_metadata(&src).map_err(|_| prov("check_candidate_missing"))?;
        if !meta.is_file() {
            return Err(prov("check_candidate_not_regular"));
        }
        let bytes = fs::read(&src).map_err(|_| prov("check_candidate_missing"))?;
        write_private_new(&root.join(rel), &bytes)?;
        candidate.push((rel.clone(), sha256_hex(&bytes)));
    }
    let oracle_bytes = fs::read(oracle).map_err(|_| prov("check_oracle_changed"))?;
    write_private_new(&root.join("check.rs"), &oracle_bytes)?;
    let home = fs::canonicalize(std::env::var("HOME").unwrap_or_else(|_| "/".into()))
        .unwrap_or_else(|_| PathBuf::from("/"));
    let real_home = super::real_home_for_checks().unwrap_or(home);
    let sandbox = check_policy(&root, &real_home)?;
    let env: Vec<(&str, String)> = vec![
        ("HOME", root.join("home").to_string_lossy().into_owned()),
        ("LANG", "C".into()),
        ("PATH", "/usr/bin:/bin".into()),
        ("TMPDIR", format!("{}/", root.join("tmp").display())),
        (
            "XDG_CONFIG_HOME",
            root.join("home/.config").to_string_lossy().into_owned(),
        ),
        (
            "XDG_DATA_HOME",
            root.join("home/.local/share")
                .to_string_lossy()
                .into_owned(),
        ),
    ];
    let version = Command::new(policy::RUSTC)
        .arg("--version")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()
        .map_err(|_| prov("check_compiler_unavailable"))?;
    let compiler_version = String::from_utf8_lossy(&version.stdout).trim().to_string();
    if !compiler_version.starts_with(policy::RUSTC_VERSION_PREFIX) {
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
        let left = deadline.saturating_duration_since(Instant::now());
        let started = Instant::now();
        let mut cmd = Command::new("/usr/bin/sandbox-exec");
        cmd.arg("-p")
            .arg(&sandbox)
            .args(&argv)
            .current_dir(&root)
            .env_clear()
            .envs(env.iter().map(|(k, v)| (*k, v.as_str())))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut child = cmd.spawn().map_err(|_| prov("check_spawn_failed"))?;
        let capture = |mut pipe: Box<dyn Read + Send>| {
            std::thread::spawn(move || {
                let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
                let mut buf = [0u8; 16 * 1024];
                while let Ok(n) = pipe.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    sha2::Digest::update(&mut hasher, &buf[..n]);
                }
                sha2::Digest::finalize(hasher)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            })
        };
        let out_t = capture(Box::new(
            child.stdout.take().ok_or(prov("check_spawn_failed"))?,
        ));
        let err_t = capture(Box::new(
            child.stderr.take().ok_or(prov("check_spawn_failed"))?,
        ));
        let pgid = child.id() as i32;
        let mut timed_out = false;
        let status = loop {
            if let Ok(Some(s)) = child.try_wait() {
                break s;
            }
            if started.elapsed() >= left {
                timed_out = true;
                // SAFETY: signalling our own child's process group.
                unsafe {
                    libc::kill(-pgid, libc::SIGKILL);
                }
                break child.wait().map_err(|_| prov("check_wait_failed"))?;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let stdout_sha256 = out_t.join().map_err(|_| prov("check_output_unbound"))?;
        let stderr_sha256 = err_t.join().map_err(|_| prov("check_output_unbound"))?;
        let artifact = |rel: &str| -> Vec<(String, String)> {
            fs::read(root.join(rel))
                .ok()
                .map(|b| vec![(rel.to_string(), sha256_hex(&b))])
                .unwrap_or_default()
        };
        let artifacts = match name {
            "noalloc_compile" => artifact("out/librecords-noalloc.rlib"),
            "alloc_compile" => artifact("out/librecords-alloc.rlib"),
            "oracle_compile" => artifact("out/check"),
            _ => Vec::new(),
        };
        let ok = status.code() == Some(0) && status.signal().is_none() && !timed_out;
        steps.push(CheckStep {
            step: name.to_string(),
            argv,
            exit: status.code(),
            signal: status.signal(),
            timed_out,
            stdout_sha256,
            stderr_sha256,
            artifacts,
            wall_ms: started.elapsed().as_millis() as u64,
        });
        if !ok {
            break;
        }
    }
    Ok(IndependentCheckReceipt {
        slot_id: slot_id.to_string(),
        checker_root: root,
        candidate,
        oracle_sha256: policy::ORACLE_SHA256.into(),
        compiler: PathBuf::from(policy::RUSTC),
        compiler_sha256: policy::RUSTC_SHA256.into(),
        compiler_version,
        env_names: CHECK_ENV_NAMES.iter().map(|s| s.to_string()).collect(),
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
    let (workspace, sources) = restore_workspace(root, "probe", Some("probe-before-run"))?;
    let capture = session.run_slot(PreparedSlot {
        slot_id: policy::PROBE_SLOT.into(),
        workspace,
        initial_user_text: policy::PROBE_INSTRUCTION.into(),
        expected_sources: sources,
    })?;
    let seal = session.seal_probe(&capture);
    Ok((capture, seal))
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
            restore_workspace(root, &format!("pair-{pair}"), archive.as_deref())?;
        let (view, _) = load_view(root, pair)?;
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
        previous.insert(pair, slot_id.to_string());
        let order: Vec<String> = capture
            .candidate_inventory
            .iter()
            .map(|(r, _)| r.clone())
            .collect();
        let sealed = run_independent_check(
            slot_id,
            &workspace,
            &order,
            &verified.oracle,
            &root
                .root
                .join("checks")
                .join(format!("{slot_id}-{}", capture.host_id)),
        )
        .and_then(|receipt| {
            write_private_new(
                &root
                    .root
                    .join("checks")
                    .join(format!("{slot_id}-{}.receipt.json", capture.host_id)),
                &serde_json::to_vec_pretty(&receipt)
                    .map_err(|_| fault(FaultClass::Persistence, "receipt_encode_failed"))?,
            )?;
            session.seal_slot(&capture, &receipt)
        });
        rows.push(slot_row(slot_id, Some(&capture), Some(&sealed)));
    }
    let receipt = session.finish()?;
    Ok(summarize(mode, rows, receipt))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
