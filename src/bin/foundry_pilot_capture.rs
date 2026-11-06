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

use std::path::PathBuf;
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
         foundry-pilot-capture manifests --pilot-root DIR --inputs FILE\n  \
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

fn authority_fault(code: &'static str) -> CaptureFault {
    CaptureFault::new(FaultClass::Authority, code)
}

fn read_json<T: serde::de::DeserializeOwned>(
    path: &str,
    missing: &'static str,
    invalid: &'static str,
) -> Result<T, CaptureFault> {
    let bytes = std::fs::read(path).map_err(|_| authority_fault(missing))?;
    serde_json::from_slice(&bytes).map_err(|_| authority_fault(invalid))
}

fn write_private(path: &std::path::Path, value: &impl serde::Serialize) -> bool {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut f| {
            f.write_all(&serde_json::to_vec_pretty(value).unwrap_or_default())?;
            f.sync_all()
        })
        .is_ok()
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
    let result = verified_inputs(args).and_then(|(root, verified)| {
        let root = PilotRoot::open(&root)?;
        apparatus::build_manifests(&root, &verified).map(|m| (root, m))
    });
    match result {
        Ok((root, (probe, coding))) => {
            println!(
                "{}",
                json!({
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
    let mut session = match CaptureSession::open(LaunchAdmission::Live(admission)) {
        Ok(s) => s,
        Err(f) => return refuse(f),
    };
    let (capture, seal) = match apparatus::run_probe(&mut session, &root) {
        Ok(v) => v,
        Err(f) => return refuse(f),
    };
    // A sealed probe is kept privately in its run (0600, exclusive) so an
    // explicit coding authorization can cite it as compatibility evidence.
    let seal_path = session.run_dir().join("probe-seal.json");
    let seal_written = seal.as_ref().ok().map(|s| write_private(&seal_path, s));
    let receipt = session.finish();
    println!(
        "{}",
        json!({
            "probe": capture.disposition,
            "seal_file": seal_written.map(|ok| json!({"path": seal_path, "written": ok})),
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
    let report_path = session.run_dir().join("coding-report.json");
    match apparatus::run_coding(session, &root, &verified) {
        Ok(report) => {
            let written = write_private(&report_path, &report);
            println!(
                "{}",
                json!({"report": report_path, "written": written, "pairs": report.pairs, "arms": report.arms, "halt": report.receipt.halt})
            );
            ExitCode::SUCCESS
        }
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
