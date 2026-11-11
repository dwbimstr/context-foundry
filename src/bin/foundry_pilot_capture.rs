//! `foundry-pilot-capture`: thin entrypoint for the bounded pilot v1 capture
//! boundary (opt-in `pilot-capture` feature).
//!
//! - `preflight`: local pin and capability checks; no network, no credential.
//! - `render`: the actual host's request commitments for one slot, stopped
//!   before any count or provider request (no credential, no network).
//! - `materialize`: claim a private pilot root, install the pinned seed and
//!   freeze the four pair views with the pinned release Foundry (local only).
//! - `manifests`: render every slot with the actual host (nothing leaves the
//!   machine) and freeze the probe and coding launch manifests.
//! - `probe`: the authorized transport-only probe; `coding`: the eight frozen
//!   slots after a sealed probe. Both validate every live gate and refuse with
//!   a named missing-evidence code until an explicit owner authorization,
//!   ratecard and credential exist; a probe authorization never covers coding.

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use context_foundry::pilot_capture::apparatus::{self, EvidenceInputs, PilotRoot};
use context_foundry::pilot_capture::{
    self, CaptureFault, CaptureSession, LaunchAdmission, LiveAdmission, PreparedSlot,
    PreviewAdmission, ProbeSeal, Scope, policy, protocol::FaultClass,
};
use serde::Deserialize;
use serde_json::json;

const DEFAULT_OMP: &str = "/Users/satishlomte/.local/bin/omp";

fn refuse(f: CaptureFault) -> ExitCode {
    eprintln!(
        "{}",
        json!({"refused": true, "class": f.class.as_str(), "code": f.code})
    );
    ExitCode::from(2)
}

fn usage() -> ExitCode {
    eprintln!(
        "usage:\n  foundry-pilot-capture preflight [--omp PATH] [--contract PATH]\n  \
         foundry-pilot-capture render --run-dir DIR --workspace DIR --prompt-file FILE [--omp PATH]\n  \
         foundry-pilot-capture materialize --pilot-root DIR --inputs FILE\n  \
         foundry-pilot-capture manifests --pilot-root DIR --inputs FILE --run-dir DIR\n  \
         foundry-pilot-capture probe --admission FILE\n  \
         foundry-pilot-capture coding --admission FILE"
    );
    ExitCode::from(2)
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn sha_file(path: &std::path::Path) -> Option<String> {
    std::fs::read(path)
        .ok()
        .map(|b| pilot_capture::sha256_hex(&b))
}

fn preflight(args: &[String]) -> ExitCode {
    let omp = PathBuf::from(flag(args, "--omp").unwrap_or_else(|| DEFAULT_OMP.into()));
    let omp_sha = sha_file(&omp);
    let contract = flag(args, "--contract").map(PathBuf::from);
    let contract_sha = contract.as_deref().and_then(sha_file);
    let exe_sha = std::env::current_exe().ok().as_deref().and_then(sha_file);
    let report = json!({
        "omp": omp, "omp_sha256": omp_sha, "omp_pin_ok": omp_sha.as_deref() == Some(policy::OMP_SHA256),
        "sandbox_exec_present": std::path::Path::new(pilot_capture::host::SANDBOX_EXEC).is_file(),
        "contract_sha256": contract_sha,
        "contract_ok": contract_sha.as_deref().map(|s| s == policy::CONTRACT_SHA256),
        "bridge_sha256": pilot_capture::sha256_hex(pilot_capture::host::BRIDGE_SOURCE),
        "implementation_sha256": exe_sha,
        "boot_identity_ok": pilot_capture::boot_identity().is_ok(),
        "continuous_clock_ok": pilot_capture::continuous_nanos().is_ok(),
        "network_used": false, "credential_read": false,
    });
    println!("{report}");
    if report["omp_pin_ok"] == true && report["sandbox_exec_present"] == true {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn render(args: &[String]) -> ExitCode {
    let (Some(run_dir), Some(workspace), Some(prompt_file)) = (
        flag(args, "--run-dir"),
        flag(args, "--workspace"),
        flag(args, "--prompt-file"),
    ) else {
        return usage();
    };
    let omp = PathBuf::from(flag(args, "--omp").unwrap_or_else(|| DEFAULT_OMP.into()));
    let Ok(prompt) = read_private(Path::new(&prompt_file), "prompt_unreadable").and_then(|bytes| {
        String::from_utf8(bytes).map_err(|_| authority_fault("prompt_unreadable"))
    }) else {
        return refuse(CaptureFault::new(
            pilot_capture::protocol::FaultClass::Bootstrap,
            "prompt_unreadable",
        ));
    };
    let Ok(workspace) = std::fs::canonicalize(&workspace) else {
        return refuse(CaptureFault::new(
            pilot_capture::protocol::FaultClass::Bootstrap,
            "workspace_missing",
        ));
    };
    let mut sources = Vec::new();
    for rel in policy::READ_SET {
        match read_private(&workspace.join(rel), "slot_sources_missing") {
            Ok(bytes) => sources.push((rel.to_string(), pilot_capture::sha256_hex(&bytes))),
            Err(_) => {
                return refuse(CaptureFault::new(
                    pilot_capture::protocol::FaultClass::Pins,
                    "slot_sources_missing",
                ));
            }
        }
    }
    let slot = PreparedSlot {
        slot_id: "render".into(),
        workspace,
        initial_user_text: prompt,
        expected_sources: sources,
    };
    match pilot_capture::render_binding(
        &PreviewAdmission {
            run_dir: run_dir.into(),
            omp,
        },
        &slot,
    ) {
        Ok(binding) => {
            println!("{}", json!(binding));
            ExitCode::SUCCESS
        }
        Err(f) => refuse(f),
    }
}

fn authority_fault(code: &'static str) -> CaptureFault {
    CaptureFault::new(FaultClass::Authority, code)
}

fn private_fault(code: &'static str) -> CaptureFault {
    CaptureFault::new(FaultClass::Persistence, code)
}

// The CLI owns these named private documents; shared executable/contract pins
// continue through their existing readers and are never chmodded here.
fn private_parent(path: &Path) -> Result<File, CaptureFault> {
    let parent = path
        .parent()
        .ok_or(private_fault("private_output_parent_invalid"))?;
    use std::os::unix::ffi::OsStrExt;
    let mut file = File::open(if parent.is_absolute() { "/" } else { "." })
        .map_err(|_| private_fault("private_output_parent_invalid"))?;
    for component in parent.components() {
        match component {
            std::path::Component::RootDir | std::path::Component::CurDir => continue,
            std::path::Component::Normal(name) => {
                let name = std::ffi::CString::new(name.as_bytes())
                    .map_err(|_| private_fault("private_output_parent_invalid"))?;
                // SAFETY: one component relative to this held directory; only
                // a fresh successful descriptor is transferred to File.
                let fd = unsafe {
                    libc::openat(
                        file.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    return Err(private_fault("private_output_parent_invalid"));
                }
                file = unsafe { File::from_raw_fd(fd) };
            }
            _ => return Err(private_fault("private_output_parent_invalid")),
        }
    }
    let meta = file
        .metadata()
        .map_err(|_| private_fault("private_output_parent_invalid"))?;
    if !meta.is_dir() || meta.uid() != unsafe { libc::getuid() } || meta.mode() & 0o7777 != 0o700 {
        return Err(private_fault("private_output_parent_invalid"));
    }
    Ok(file)
}

fn private_name(path: &Path) -> Result<std::ffi::CString, CaptureFault> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(
        path.file_name()
            .ok_or(private_fault("private_output_path_invalid"))?
            .as_bytes(),
    )
    .map_err(|_| private_fault("private_output_path_invalid"))
}

fn read_private(path: &Path, missing: &'static str) -> Result<Vec<u8>, CaptureFault> {
    let parent = private_parent(path)?;
    let name = private_name(path)?;
    // SAFETY: this is one component relative to the held private parent fd.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(authority_fault(missing));
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let meta = file.metadata().map_err(|_| authority_fault(missing))?;
    if !meta.is_file() || meta.uid() != unsafe { libc::getuid() } || meta.mode() & 0o7777 != 0o600 {
        return Err(CaptureFault::new(
            FaultClass::Privacy,
            "private_artifact_not_private",
        ));
    }
    let mut bytes = Vec::new();
    file.take((1 << 20) + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| authority_fault(missing))?;
    if bytes.len() > 1 << 20 {
        return Err(authority_fault("evidence_shape_invalid"));
    }
    Ok(bytes)
}

fn read_json<T: serde::de::DeserializeOwned>(
    path: &str,
    missing: &'static str,
    invalid: &'static str,
) -> Result<T, CaptureFault> {
    serde_json::from_slice(&read_private(Path::new(path), missing)?)
        .map_err(|_| authority_fault(invalid))
}

fn write_private(path: &Path, value: &impl serde::Serialize) -> Result<(), CaptureFault> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|_| private_fault("private_output_encode_failed"))?;
    let parent = private_parent(path)?;
    let name = private_name(path)?;
    // SAFETY: exclusive creation cannot overwrite an existing entry; the new
    // owned fd alone is normalized, including under a restrictive child mask.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(private_fault("private_output_create_failed"));
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(|_| private_fault("private_output_mode_failed"))?;
    let meta = file
        .metadata()
        .map_err(|_| private_fault("private_output_mode_failed"))?;
    if !meta.is_file() || meta.uid() != unsafe { libc::getuid() } || meta.mode() & 0o7777 != 0o600 {
        return Err(private_fault("private_output_mode_failed"));
    }
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| parent.sync_all())
        .map_err(|_| private_fault("private_output_write_failed"))
}

/// Both persistence and finalization must succeed; the caller has already
/// consumed the session, so an error never reopens a terminal journal.
fn probe_publication_result(
    persisted: Result<(), CaptureFault>,
    finished: Result<pilot_capture::CaptureReceipt, CaptureFault>,
) -> Result<pilot_capture::CaptureReceipt, CaptureFault> {
    let receipt = finished?;
    persisted?;
    Ok(receipt)
}

fn publish_coding_report(path: &Path, report: &apparatus::PilotReport) -> ExitCode {
    if let Err(fault) = write_private(path, report) {
        return refuse(fault);
    }
    println!(
        "{}",
        json!({"report": path, "written": true, "pairs": report.pairs, "arms": report.arms, "halt": report.receipt.halt})
    );
    if report.receipt.halt.is_some() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn verified_inputs(args: &[String]) -> Result<(PathBuf, apparatus::Verified), CaptureFault> {
    let (Some(root), Some(inputs)) = (flag(args, "--pilot-root"), flag(args, "--inputs")) else {
        return Err(authority_fault("usage"));
    };
    let inputs: EvidenceInputs = read_json(&inputs, "inputs_missing", "inputs_invalid")?;
    Ok((PathBuf::from(root), inputs.verify()?))
}

fn materialize(args: &[String]) -> ExitCode {
    let result = verified_inputs(args).and_then(|(root, verified)| {
        let root = PilotRoot::create(&root)?;
        apparatus::install_seed(&root, &verified)?;
        let views = apparatus::materialize_views(&root, &verified)?;
        Ok((root, views))
    });
    match result {
        Ok((root, views)) => {
            println!(
                "{}",
                json!({"pilot_root": root.root, "views": views.iter().map(|v| json!({
                "pair": v.pair, "sha256": v.view_sha256, "bytes": v.view_bytes,
                "delivered_o200k_tokens": v.delivered_o200k_tokens, "header": v.header,
                "prepare_ms": v.prepare_ms, "context_ms": v.context_ms})).collect::<Vec<_>>()})
            );
            ExitCode::SUCCESS
        }
        Err(f) if f.code == "usage" => usage(),
        Err(f) => refuse(f),
    }
}

fn manifests(args: &[String]) -> ExitCode {
    let Some(run_dir) = flag(args, "--run-dir") else {
        return usage();
    };
    let result = verified_inputs(args).and_then(|(root, verified)| {
        let root = PilotRoot::open(&root)?;
        apparatus::build_manifests(&root, &verified, std::path::Path::new(&run_dir))
            .map(|m| (root, m))
    });
    match result {
        Ok((root, (probe, coding))) => {
            println!(
                "{}",
                json!({
                    "run_dir": probe.run_dir,
                    "probe_manifest": root.manifest(Scope::Probe),
                    "coding_manifest": root.manifest(Scope::Coding),
                    "probe_manifest_sha256": sha_file(&root.manifest(Scope::Probe)),
                    "coding_manifest_sha256": sha_file(&root.manifest(Scope::Coding)),
                    "slots": coding.slots.len() + probe.slots.len(),
                    "pins": coding.pins.len(),
                })
            );
            ExitCode::SUCCESS
        }
        Err(f) if f.code == "usage" => usage(),
        Err(f) => refuse(f),
    }
}

/// A live admission file names evidence paths only; it never contains a
/// credential. `probe_seal` is required for coding and refused for probe.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmissionFile {
    pilot_root: PathBuf,
    inputs: PathBuf,
    run_dir: PathBuf,
    contract: PathBuf,
    authority: PathBuf,
    ratecard: PathBuf,
    credential_file: PathBuf,
    probe_seal: Option<PathBuf>,
}

fn live_admission(
    file: &AdmissionFile,
    scope: Scope,
    root: &PilotRoot,
    omp: PathBuf,
) -> LiveAdmission {
    LiveAdmission {
        run_dir: file.run_dir.clone(),
        omp,
        scope,
        contract: file.contract.clone(),
        authority: file.authority.clone(),
        launch_manifest: root.manifest(scope),
        ratecard: file.ratecard.clone(),
        compatibility: file.probe_seal.clone(),
        credential_file: file.credential_file.clone(),
    }
}

fn load_admission(
    args: &[String],
    scope: Scope,
) -> Result<(AdmissionFile, PilotRoot, apparatus::Verified), CaptureFault> {
    let Some(path) = flag(args, "--admission") else {
        return Err(authority_fault("usage"));
    };
    let file: AdmissionFile = read_json(&path, "admission_missing", "admission_invalid")?;
    if (scope == Scope::Probe) == file.probe_seal.is_some() {
        return Err(authority_fault("admission_scope_invalid"));
    }
    let inputs: EvidenceInputs = read_json(
        file.inputs.to_str().unwrap_or_default(),
        "inputs_missing",
        "inputs_invalid",
    )?;
    let root = PilotRoot::open(&file.pilot_root)?;
    let verified = inputs.verify()?;
    Ok((file, root, verified))
}

fn probe(args: &[String]) -> ExitCode {
    let (file, root, verified) = match load_admission(args, Scope::Probe) {
        Ok(v) => v,
        Err(f) if f.code == "usage" => return usage(),
        Err(f) => return refuse(f),
    };
    let admission = live_admission(&file, Scope::Probe, &root, verified.inputs.omp.clone());
    let session = match CaptureSession::open(LaunchAdmission::Live(admission)) {
        Ok(s) => s,
        Err(f) => return refuse(f),
    };
    probe_session(session, &root)
}

fn probe_session(mut session: CaptureSession, root: &PilotRoot) -> ExitCode {
    let (capture, seal) = match apparatus::run_probe(&mut session, root) {
        Ok(v) => v,
        Err(f) => {
            let receipt = match session.finish() {
                Ok(receipt) => receipt,
                Err(finish_fault) => return refuse(finish_fault),
            };
            println!(
                "{}",
                json!({"probe": null, "seal_file": null, "seal_refusal": f.code, "receipt": receipt})
            );
            return refuse(f);
        }
    };
    // A sealed probe is kept privately in its run (0600, exclusive) so an
    // explicit coding authorization can cite it as compatibility evidence.
    let seal_path = session.run_dir().join("probe-seal.json");
    let persisted = match &seal {
        Ok(seal) => write_private(&seal_path, seal),
        Err(fault) => Err(*fault),
    };
    let receipt = match probe_publication_result(persisted, session.finish()) {
        Ok(receipt) => receipt,
        Err(fault) => return refuse(fault),
    };
    println!(
        "{}",
        json!({"probe": capture.disposition,
        "seal_file": {"path": seal_path, "written": true},
        "seal_refusal": null, "receipt": receipt})
    );
    ExitCode::SUCCESS
}

fn coding(args: &[String]) -> ExitCode {
    let (file, root, verified) = match load_admission(args, Scope::Coding) {
        Ok(v) => v,
        Err(f) if f.code == "usage" => return usage(),
        Err(f) => return refuse(f),
    };
    let seal_path = file.probe_seal.clone().unwrap_or_default();
    let seal: ProbeSeal = match read_json(
        seal_path.to_str().unwrap_or_default(),
        "probe_seal_missing",
        "probe_seal_invalid",
    ) {
        Ok(s) => s,
        Err(f) => return refuse(f),
    };
    let admission = live_admission(&file, Scope::Coding, &root, verified.inputs.omp.clone());
    let session = match CaptureSession::open_after_probe(&seal, LaunchAdmission::Live(admission)) {
        Ok(s) => s,
        Err(f) => return refuse(f),
    };
    coding_session(session, &root, &verified)
}

fn coding_session(
    session: CaptureSession,
    root: &PilotRoot,
    verified: &apparatus::Verified,
) -> ExitCode {
    let report_path = session.run_dir().join("coding-report.json");
    match apparatus::run_coding(session, root, verified) {
        Ok(report) => publish_coding_report(&report_path, &report),
        Err(f) => refuse(f),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("preflight") => preflight(&args[1..]),
        Some("render") => render(&args[1..]),
        Some("materialize") => materialize(&args[1..]),
        Some("manifests") => manifests(&args[1..]),
        Some("probe") => probe(&args[1..]),
        Some("coding") => coding(&args[1..]),
        _ => usage(),
    }
}

// These tests enter the same post-admission CLI path with Verification mode.
// They cannot authorize a live launch or qualify real-provider compatibility.
#[cfg(all(test, target_os = "macos"))]
#[allow(dead_code)]
#[path = "../../tests/pilot_capture/fixture.rs"]
mod restore_fixture;

#[cfg(all(test, target_os = "macos"))]
mod restore_tests {
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

    fn fixture_private_dir(path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(path.as_ref())?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
    }

    fn fixture_private_dirs(path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let mut missing = Vec::new();
        let mut cursor = path.as_ref();
        while !cursor.try_exists()? {
            missing.push(cursor.to_path_buf());
            cursor = cursor
                .parent()
                .ok_or_else(|| std::io::Error::other("owned fixture has no parent"))?;
        }
        for directory in missing.into_iter().rev() {
            fixture_private_dir(directory)?;
        }
        Ok(())
    }

    fn fixture_private_write(
        path: impl AsRef<std::path::Path>,
        bytes: impl AsRef<[u8]>,
    ) -> std::io::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let existed = path.as_ref().symlink_metadata().is_ok();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path.as_ref())?;
        if !existed {
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(bytes.as_ref())
    }

    use super::*;
    use context_foundry::pilot_capture::{Counters, VerificationAdmission};
    use restore_fixture::{Fixture, Reply};
    use serde_json::Value;
    use std::path::Path;

    const OMP: &str = match option_env!("CF_PILOT_TEST_OMP") {
        Some(path) => path,
        None => DEFAULT_OMP,
    };
    const CANARY: &str = "sk-ant-verify-restore-halt-7f3c1e9a5b2d4c6e8f0a1b3c5d7e9f11";

    fn admission(run_dir: &Path, fixture: &Fixture, scope: Scope) -> LaunchAdmission {
        LaunchAdmission::Verification(VerificationAdmission {
            run_dir: run_dir.to_path_buf(),
            omp: OMP.into(),
            scope,
            fixture_addr: fixture.addr,
            fixture_ca_pem: fixture.ca_pem.clone(),
            credential: CANARY.into(),
            faults: Default::default(),
            manifest: None,
        })
    }

    fn journal(run_dir: &Path) -> Vec<Value> {
        std::fs::read_to_string(run_dir.join("journal.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn assert_terminal(run_dir: &Path, code: &str, counters: Counters) -> Vec<Value> {
        let records = journal(run_dir);
        let halts: Vec<_> = records.iter().filter(|r| r["kind"] == "halt").collect();
        assert_eq!(halts.len(), 1);
        assert_eq!(halts[0]["class"], FaultClass::Pins.as_str());
        assert_eq!(halts[0]["code"], code);
        let last = records.last().unwrap();
        assert_eq!(last["kind"], "finished");
        assert_eq!(last["halt"], json!([FaultClass::Pins.as_str(), code]));
        assert_eq!(last["counters"], json!(counters));
        assert!(
            !std::fs::read_to_string(run_dir.join("journal.jsonl"))
                .unwrap()
                .contains(CANARY)
        );
        records
    }

    #[test]
    fn probe_restore_failure_finishes_halted_without_http() {
        for invalid_contents in [false, true] {
            let temp = fixture_private_tempdir().unwrap();
            let base = std::fs::canonicalize(temp.path()).unwrap();
            let root = PilotRoot::create(&base.join("pilot")).unwrap();
            if invalid_contents {
                fixture_private_dir(root.seed().join("src")).unwrap();
                for (rel, _) in apparatus::pins::SEED {
                    fixture_private_write(root.seed().join(rel), b"synthetic invalid seed\n")
                        .unwrap();
                }
            }
            let code = if invalid_contents {
                "seed_changed"
            } else {
                "seed_tree_not_exact"
            };
            let run_dir = base.join("run");
            let fixture = Fixture::start(vec![]);
            let mut session =
                CaptureSession::open(admission(&run_dir, &fixture, Scope::Probe)).unwrap();
            let fault = apparatus::run_probe(&mut session, &root).err().unwrap();
            assert_eq!(fault.code, code);
            assert_eq!(session.halted().unwrap().code, code);
            // Preparation failed before any host. Even a direct caller cannot
            // bypass that halt by submitting the next otherwise valid slot id.
            assert_eq!(
                session
                    .run_slot(PreparedSlot {
                        slot_id: policy::PROBE_SLOT.into(),
                        workspace: root.seed(),
                        initial_user_text: policy::PROBE_INSTRUCTION.into(),
                        expected_sources: Vec::new(),
                    })
                    .err()
                    .unwrap()
                    .code,
                "session_halted"
            );
            assert_eq!(probe_session(session, &root), ExitCode::from(2));
            let records = assert_terminal(&run_dir, code, Counters::default());
            assert_eq!(records[0]["kind"], "admitted");
            assert_eq!(
                records.last().unwrap()["slots"],
                json!([[policy::PROBE_SLOT, "not_started"]])
            );
            assert_eq!(std::fs::read_dir(run_dir.join("hosts")).unwrap().count(), 0);
            assert!(fixture.seen().is_empty());
            assert_eq!(fixture.remaining_script(), 0);
            assert!(!run_dir.join("probe-seal.json").exists());
        }
    }

    #[test]
    fn coding_restore_failure_after_fixture_probe_retains_all_rows_and_fails_cli() {
        let temp = fixture_private_tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        let root = PilotRoot::create(&base.join("pilot")).unwrap();
        let workspace = base.join("synthetic-probe");
        fixture_private_dirs(workspace.join("src")).unwrap();
        for (rel, bytes) in [
            (
                "Cargo.toml",
                "[package]\nname = \"records\"\nversion = \"0.0.0\"\n",
            ),
            (
                "src/lib.rs",
                "pub fn decode(input: &[u8]) -> Vec<u8> { input.to_vec() }\n",
            ),
            ("src/ascii_set.rs", "pub struct AsciiSet;\n"),
            ("LICENSE-MIT", "MIT synthetic\n"),
            ("LICENSE-APACHE", "Apache synthetic\n"),
        ] {
            fixture_private_write(workspace.join(rel), bytes).unwrap();
        }
        let fixture = Fixture::start(vec![
            Reply::Count(2048),
            restore_fixture::text_reply("msg_restore_probe", "PILOT_TRANSPORT_OK"),
        ]);
        let run_dir = base.join("run");
        let mut probe = CaptureSession::open(admission(&run_dir, &fixture, Scope::Probe)).unwrap();
        let capture = probe
            .run_slot(PreparedSlot {
                slot_id: policy::PROBE_SLOT.into(),
                workspace: workspace.clone(),
                initial_user_text: policy::PROBE_INSTRUCTION.into(),
                expected_sources: policy::READ_SET
                    .iter()
                    .map(|rel| {
                        (
                            rel.to_string(),
                            pilot_capture::sha256_hex(&std::fs::read(workspace.join(rel)).unwrap()),
                        )
                    })
                    .collect(),
            })
            .unwrap();
        if probe.halted().is_some() {
            // Failed owned fixture: retain only bounded metadata diagnostics.
            let mut todo = vec![base.clone()];
            let mut inspected = 0;
            while let Some(path) = todo.pop() {
                inspected += 1;
                assert!(inspected <= 16_384);
                let meta = std::fs::symlink_metadata(&path).unwrap();
                let expected = if meta.is_dir() { 0o700 } else { 0o600 };
                if meta.mode() & 0o7777 != expected || (!meta.is_dir() && !meta.is_file()) {
                    eprintln!(
                        "owned probe output metadata: {} mode={:04o} directory={} regular={}",
                        path.strip_prefix(&base).unwrap().display(),
                        meta.mode() & 0o7777,
                        meta.is_dir(),
                        meta.is_file()
                    );
                }
                if meta.is_dir() {
                    todo.extend(
                        std::fs::read_dir(&path)
                            .unwrap()
                            .map(|entry| entry.unwrap().path()),
                    );
                }
            }
            eprintln!(
                "owned failed probe: {:?}; evidence={}",
                probe.halted(),
                base.display()
            );
            std::mem::forget(temp);
        }
        let seal = probe.seal_probe(&capture).unwrap();
        let probe_receipt = probe.finish().unwrap();
        let sealed_prefix = std::fs::read(run_dir.join("journal.jsonl")).unwrap();
        assert_eq!(
            pilot_capture::sha256_hex(&sealed_prefix),
            seal.journal_sha256
        );
        assert_eq!(fixture.seen().len(), 2);
        assert_eq!(fixture.remaining_script(), 0);
        // Same actual sealed probe and consumed finish receipt, now exercising
        // the real CLI sidecar/result owners without another host invocation.
        let seal_collision = run_dir.join("probe-seal.json");
        fixture_private_write(&seal_collision, b"owned seal-sidecar sentinel").unwrap();
        let persisted = write_private(&seal_collision, &seal);
        let fault = probe_publication_result(persisted, Ok(probe_receipt)).unwrap_err();
        assert_eq!(fault.code, "private_output_create_failed");
        assert_eq!(refuse(fault), ExitCode::from(2));
        assert_eq!(
            std::fs::read(&seal_collision).unwrap(),
            b"owned seal-sidecar sentinel"
        );
        assert_eq!(
            std::fs::read(run_dir.join("journal.jsonl")).unwrap(),
            sealed_prefix
        );
        let coding =
            CaptureSession::open_after_probe(&seal, admission(&run_dir, &fixture, Scope::Coding))
                .unwrap();
        // Deliberately unavailable downstream inputs. Restore must halt before
        // reading a view, building an arm prompt, or executing an oracle.
        // This value is synthetic, not a claim that frozen evidence verified.
        let absent = base.join("must-not-be-used");
        let verified = apparatus::Verified {
            inputs: EvidenceInputs {
                paired_root: absent.clone(),
                freeze_root: absent.clone(),
                foundry: absent.clone(),
                profile: absent.clone(),
                omp: OMP.into(),
            },
            pins: Vec::new(),
            task: String::new(),
            arm_a_prompt: String::new(),
            oracle: absent,
            seed: Vec::new(),
        };
        assert_eq!(coding_session(coding, &root, &verified), ExitCode::from(1));
        let report: apparatus::PilotReport =
            serde_json::from_slice(&std::fs::read(run_dir.join("coding-report.json")).unwrap())
                .unwrap();
        let records = assert_terminal(&run_dir, "seed_tree_not_exact", seal.counters);
        assert_eq!(report.receipt.counters, seal.counters);
        assert_eq!(
            report.receipt.halt,
            Some((
                FaultClass::Pins.as_str().into(),
                "seed_tree_not_exact".into()
            ))
        );
        assert_eq!(
            report
                .slots
                .iter()
                .map(|s| s.slot_id.as_str())
                .collect::<Vec<_>>(),
            policy::SLOT_ORDER
        );
        assert!(
            report
                .slots
                .iter()
                .all(|s| s.state == "not_started" && s.disposition.is_none() && s.check.is_none())
        );
        assert_eq!(
            report
                .receipt
                .slots
                .iter()
                .map(|s| s.slot_id.as_str())
                .collect::<Vec<_>>(),
            policy::SLOT_ORDER
        );
        assert!(
            report
                .receipt
                .slots
                .iter()
                .all(|s| s.state == "not_started" && s.disposition.is_none() && s.check.is_none())
        );
        assert!(report.pairs.iter().all(|p| !p.valid
            && p.a_pass.is_none()
            && p.b_pass.is_none()
            && p.b_minus_a.is_none()));
        assert!(report.arms.iter().all(|a| a.checked == 0 && a.passed == 0));
        let final_journal = std::fs::read(run_dir.join("journal.jsonl")).unwrap();
        assert!(final_journal.starts_with(&sealed_prefix));
        assert_eq!(
            report.receipt.journal_sha256,
            pilot_capture::sha256_hex(&final_journal)
        );
        assert_eq!(report.receipt.journal_records as usize, records.len());
        assert_eq!(std::fs::read_dir(run_dir.join("hosts")).unwrap().count(), 1);
        assert_eq!(
            fixture.seen().len(),
            2,
            "restore failure must add no HTTP request"
        );
        assert_eq!(fixture.remaining_script(), 0);
        assert_eq!(
            CaptureSession::open_after_probe(&seal, admission(&run_dir, &fixture, Scope::Coding))
                .err()
                .unwrap()
                .code,
            "journal_digest_mismatch"
        );
        assert_eq!(fixture.seen().len(), 2);
    }
    #[test]
    fn privacy_cli_writer_is_exact_and_reports_encoding_creation_and_parent_failures() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
        struct InvalidEncoding;
        impl serde::Serialize for InvalidEncoding {
            fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom(
                    "owned deterministic encoding refusal",
                ))
            }
        }
        let temp = fixture_private_tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        let value = serde_json::json!({"owned":"publication fixture"});
        let fresh = base.join("fresh.json");
        write_private(&fresh, &value).unwrap();
        assert_eq!(std::fs::metadata(&fresh).unwrap().mode() & 0o7777, 0o600);
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(&fresh).unwrap()).unwrap(),
            value
        );
        let before = std::fs::read(&fresh).unwrap();
        assert_eq!(
            write_private(&fresh, &serde_json::json!({"replacement":true}))
                .unwrap_err()
                .code,
            "private_output_create_failed"
        );
        assert_eq!(std::fs::read(&fresh).unwrap(), before);
        let link = base.join("link.json");
        symlink(&fresh, &link).unwrap();
        assert_eq!(
            write_private(&link, &value).unwrap_err().code,
            "private_output_create_failed"
        );
        assert_eq!(std::fs::read(&fresh).unwrap(), before);
        let encoding = base.join("encoding.json");
        assert_eq!(
            write_private(&encoding, &InvalidEncoding).unwrap_err().code,
            "private_output_encode_failed"
        );
        assert!(!encoding.exists());
        let container = base.join("container");
        fixture_private_dir(&container).unwrap();
        let alias = base.join("parent-alias");
        symlink(&container, &alias).unwrap();
        assert!(write_private(&alias.join("must-not-exist.json"), &value).is_err());
        assert!(!container.join("must-not-exist.json").exists());
        for mode in [0o750, 0o1700] {
            std::fs::set_permissions(&container, std::fs::Permissions::from_mode(mode)).unwrap();
            assert_eq!(
                write_private(&container.join("must-not-exist.json"), &value)
                    .unwrap_err()
                    .code,
                "private_output_parent_invalid"
            );
            assert_eq!(std::fs::metadata(&container).unwrap().mode() & 0o7777, mode);
        }
    }

    #[test]
    fn privacy_cli_writer_child_masks_and_real_write_failure_are_not_false_success() {
        use std::os::unix::fs::MetadataExt;
        use std::os::unix::process::CommandExt;
        use std::process::Command;
        const CHILD: &str = "CF_PRIVATE_CLI_WRITER_TEST_CHILD";
        if let Some(path) = std::env::var_os(CHILD) {
            let root = PathBuf::from(path);
            std::fs::write(root.join("ambient-before"), []).unwrap();
            let ambient_mode = std::fs::metadata(root.join("ambient-before"))
                .unwrap()
                .mode()
                & 0o7777;
            let output = root.join("result.json");
            let zero_file_limit = std::env::var_os("CF_PRIVATE_CLI_FSIZE_ZERO").is_some();
            if zero_file_limit {
                // This already-execed test child is the sole affected process.
                let limit = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &limit) }, 0);
                unsafe {
                    libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
                }
            }
            let result = write_private(&output, &serde_json::json!({"owned":"nonempty"}));
            if zero_file_limit {
                assert_eq!(result.unwrap_err().code, "private_output_write_failed");
                assert_eq!(std::fs::metadata(&output).unwrap().len(), 0);
            } else {
                result.unwrap();
            }
            assert_eq!(std::fs::metadata(output).unwrap().mode() & 0o7777, 0o600);
            std::fs::write(root.join("ambient-after"), []).unwrap();
            assert_eq!(
                std::fs::metadata(root.join("ambient-after"))
                    .unwrap()
                    .mode()
                    & 0o7777,
                ambient_mode,
                "product operation changed its caller mask"
            );
            return;
        }
        let temp = fixture_private_tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        std::fs::write(base.join("parent-before"), b"owned parent mask witness").unwrap();
        let before = std::fs::metadata(base.join("parent-before"))
            .unwrap()
            .mode()
            & 0o7777;
        for (label, mask, zero_file_limit) in [
            ("permissive", 0, false),
            ("restrictive", 0o777, false),
            ("write-failure", 0, true),
        ] {
            let root = base.join(label);
            fixture_private_dir(&root).unwrap();
            let mut child = Command::new(std::env::current_exe().unwrap());
            child.args(["--exact","restore_tests::privacy_cli_writer_child_masks_and_real_write_failure_are_not_false_success","--nocapture"]).env(CHILD,&root);
            if zero_file_limit {
                child.env("CF_PRIVATE_CLI_FSIZE_ZERO", "1");
            }
            // All changes apply only to this owned test child before exec.
            unsafe {
                child.pre_exec(move || {
                    libc::umask(mask);
                    Ok(())
                });
            }
            // RLIMIT_FSIZE applies to inherited regular log descriptors too.
            // Pipes let the zero-limit child finish libtest's own reporting;
            // the unaffected parent retains both output streams afterward.
            let output = child.output().unwrap();
            std::io::stdout().write_all(&output.stdout).unwrap();
            std::io::stderr().write_all(&output.stderr).unwrap();
            assert!(output.status.success(), "owned CLI writer child {label}");
        }
        std::fs::write(base.join("parent-after"), b"owned parent mask witness").unwrap();
        assert_eq!(
            std::fs::metadata(base.join("parent-after")).unwrap().mode() & 0o7777,
            before
        );
    }

    #[test]
    fn privacy_cli_report_publication_requires_a_real_write_and_preserves_finished_history() {
        let temp = fixture_private_tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        let fixture = Fixture::start(vec![]);
        let run = base.join("run");
        let receipt = CaptureSession::open(admission(&run, &fixture, Scope::Coding))
            .unwrap()
            .finish()
            .unwrap();
        let history = std::fs::read(run.join("journal.jsonl")).unwrap();
        let report = apparatus::PilotReport {
            kind: "owned-publication-fixture".into(),
            mode: pilot_capture::Mode::Verification,
            slots: Vec::new(),
            pairs: Vec::new(),
            arms: Vec::new(),
            receipt,
            claim_ceiling: "publication result only; no coding slots executed".into(),
        };
        let collision = run.join("coding-report.json");
        fixture_private_write(&collision, b"owned report sentinel").unwrap();
        assert_eq!(
            publish_coding_report(&collision, &report),
            ExitCode::from(2)
        );
        assert_eq!(std::fs::read(&collision).unwrap(), b"owned report sentinel");
        assert_eq!(std::fs::read(run.join("journal.jsonl")).unwrap(), history);
        let success = run.join("new-report.json");
        assert_eq!(publish_coding_report(&success, &report), ExitCode::SUCCESS);
        let stored: apparatus::PilotReport =
            serde_json::from_slice(&std::fs::read(success).unwrap()).unwrap();
        assert_eq!(
            stored.receipt.journal_sha256,
            pilot_capture::sha256_hex(&history)
        );
        assert_eq!(std::fs::read(run.join("journal.jsonl")).unwrap(), history);
        assert!(fixture.seen().is_empty());
    }

    #[test]
    fn privacy_cli_probe_publication_does_not_hide_real_finish_failure() {
        let temp = fixture_private_tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        let fixture = Fixture::start(vec![]);
        let run = base.join("run");
        let mut launch = admission(&run, &fixture, Scope::Probe);
        let LaunchAdmission::Verification(ref mut verification) = launch else {
            unreachable!()
        };
        verification.faults.journal_fail_after = Some(1);
        let session = CaptureSession::open(launch).unwrap();
        let before = std::fs::read(run.join("journal.jsonl")).unwrap();
        let persisted = write_private(
            &run.join("owned-sidecar.json"),
            &serde_json::json!({"owned":"sidecar"}),
        );
        let finished = session.finish();
        assert_eq!(finished.as_ref().unwrap_err().code, "journal_append_failed");
        let fault = probe_publication_result(persisted, finished).unwrap_err();
        assert_eq!(fault.code, "journal_append_failed");
        assert_eq!(refuse(fault), ExitCode::from(2));
        assert_eq!(std::fs::read(run.join("journal.jsonl")).unwrap(), before);
        assert!(fixture.seen().is_empty());
    }
}
