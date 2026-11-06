//! `foundry-pilot-capture`: thin entrypoint for the bounded pilot v1 capture
//! boundary (opt-in `pilot-capture` feature).
//!
//! - `preflight`: local pin and capability checks; no network, no credential.
//! - `render`: the actual host's request commitments for one slot, stopped
//!   before any count or provider request (no credential, no network).
//! - `probe`: the future authorized transport-only probe. It validates every
//!   live gate and refuses with a named missing-evidence code until an
//!   explicit owner authorization, launch manifest, ratecard and credential
//!   exist. It never authorizes coding.
//!
//! The coding path is a library API (`CaptureSession`); there is no full
//! pilot executor here.

use std::path::PathBuf;
use std::process::ExitCode;

use context_foundry::pilot_capture::{
    self, CaptureFault, CaptureSession, LaunchAdmission, LiveAdmission, PreparedSlot,
    PreviewAdmission, Scope, policy,
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
         foundry-pilot-capture probe --admission FILE"
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
    let Ok(prompt) = std::fs::read_to_string(&prompt_file) else {
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
        match std::fs::read(workspace.join(rel)) {
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

/// The probe admission file names evidence paths only; it never contains a
/// credential.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeAdmissionFile {
    run_dir: PathBuf,
    omp: PathBuf,
    contract: PathBuf,
    authority: PathBuf,
    launch_manifest: PathBuf,
    ratecard: PathBuf,
    credential_file: PathBuf,
}

fn probe(args: &[String]) -> ExitCode {
    let Some(path) = flag(args, "--admission") else {
        return usage();
    };
    let fault = |code| CaptureFault::new(pilot_capture::protocol::FaultClass::Authority, code);
    let Ok(bytes) = std::fs::read(&path) else {
        return refuse(fault("probe_admission_missing"));
    };
    let Ok(file) = serde_json::from_slice::<ProbeAdmissionFile>(&bytes) else {
        return refuse(fault("probe_admission_invalid"));
    };
    let admission = LiveAdmission {
        run_dir: file.run_dir,
        omp: file.omp,
        scope: Scope::Probe,
        contract: file.contract,
        authority: file.authority,
        launch_manifest: file.launch_manifest.clone(),
        ratecard: file.ratecard,
        compatibility: None,
        credential_file: file.credential_file,
    };
    let manifest_bytes = std::fs::read(&file.launch_manifest).ok();
    let mut session = match CaptureSession::open(LaunchAdmission::Live(admission)) {
        Ok(s) => s,
        Err(f) => return refuse(f),
    };
    // The manifest was validated by `open`; it names the probe slot.
    let Some(manifest) = manifest_bytes
        .and_then(|b| serde_json::from_slice::<pilot_capture::LaunchManifest>(&b).ok())
    else {
        return refuse(fault("launch_manifest_invalid"));
    };
    let Some(entry) = manifest.slots.first() else {
        return refuse(fault("launch_manifest_invalid"));
    };
    let slot = PreparedSlot {
        slot_id: policy::PROBE_SLOT.into(),
        workspace: entry.workspace.clone(),
        initial_user_text: policy::PROBE_INSTRUCTION.into(),
        expected_sources: entry.sources.clone(),
    };
    let capture = match session.run_slot(slot) {
        Ok(c) => c,
        Err(f) => return refuse(f),
    };
    let seal = session.seal_probe(&capture);
    // A sealed probe is kept privately in its run (0600, exclusive) so an
    // explicit coding authorization can cite it as compatibility evidence.
    let seal_file = seal.as_ref().ok().map(|s| {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let path = session.run_dir().join("probe-seal.json");
        let written = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .and_then(|mut f| {
                f.write_all(&serde_json::to_vec_pretty(s).unwrap_or_default())?;
                f.sync_all()
            });
        (path, written.is_ok())
    });
    let receipt = session.finish();
    println!(
        "{}",
        json!({
            "probe": capture.disposition,
            "seal": seal.as_ref().ok(),
            "seal_file": seal_file.as_ref().map(|(p, ok)| json!({"path": p, "written": ok})),
            "seal_refusal": seal.as_ref().err().map(|f| f.code),
            "receipt": receipt.as_ref().ok(),
        })
    );
    if seal.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("preflight") => preflight(&args[1..]),
        Some("render") => render(&args[1..]),
        Some("probe") => probe(&args[1..]),
        _ => usage(),
    }
}
