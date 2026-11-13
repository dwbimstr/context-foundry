//! Bounded pilot v1 capture boundary: actual-host no-provider qualification
//! (implementer contract §10) and the bounded suite.
//!
//! Every test that launches a host uses the real pinned OMP 18.13.2 binary,
//! the embedded bridge, a network-denied sandbox and an owned numeric-loopback
//! TLS fixture with an owned CA and an owned synthetic credential. No real
//! credential is read and no provider request is made. The pinned host is a
//! hard requirement: a missing or mismatching binary fails the test rather
//! than skipping it.
//!
//! Run: `cargo test --features pilot-capture --test pilot_capture`.
//! `CF_PILOT_TEST_OMP` may select an archived host at compile time; its bytes
//! must still match the frozen host pin. The installed-host default is unchanged.
#![cfg(all(feature = "pilot-capture", target_os = "macos"))]

#[path = "pilot_capture/fixture.rs"]
mod fixture;

use std::path::{Path, PathBuf};

use context_foundry::pilot_capture::{
    self, CaptureSession, CheckStep, Disposition, IndependentCheckReceipt, LaunchAdmission,
    LiveAdmission, PreparedSlot, PreviewAdmission, Scope, VerificationAdmission,
    VerificationFaults, policy, protocol::FaultClass, wire,
};
use fixture::{Fixture, Reply};
use serde_json::{Value, json};

const OMP: &str = match option_env!("CF_PILOT_TEST_OMP") {
    Some(path) => path,
    None => "/Users/satishlomte/.local/bin/omp",
};
const CANARY: &str = "sk-ant-verify-canary-7f3c1e9a5b2d4c6e8f0a1b3c5d7e9f11";
const TASK: &str = "Repair malformed percent escapes in src/lib.rs so no bytes are dropped.";

fn require_host() {
    let bytes = std::fs::read(OMP).expect("pinned OMP host is required for this qualification");
    assert_eq!(
        pilot_capture::sha256_hex(&bytes),
        policy::OMP_SHA256,
        "pinned OMP hash mismatch"
    );
}

struct Env {
    _root: tempfile::TempDir,
    run_dir: PathBuf,
    workspace: PathBuf,
}

/// A fresh synthetic five-file workspace plus an unclaimed run path.
fn env() -> Env {
    let root = fixture_private_tempdir().unwrap();
    let base = std::fs::canonicalize(root.path()).unwrap();
    let ws = base.join("ws");
    fixture_private_dirs(ws.join("src")).unwrap();
    fixture_private_write(
        ws.join("Cargo.toml"),
        "[package]\nname = \"records\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    fixture_private_write(
        ws.join("src/lib.rs"),
        "pub fn decode(input: &[u8]) -> Vec<u8> { input.to_vec() }\n",
    )
    .unwrap();
    fixture_private_write(ws.join("src/ascii_set.rs"), "pub struct AsciiSet;\n").unwrap();
    fixture_private_write(ws.join("LICENSE-MIT"), "MIT synthetic\n").unwrap();
    fixture_private_write(ws.join("LICENSE-APACHE"), "Apache synthetic\n").unwrap();
    Env {
        run_dir: base.join("run"),
        workspace: ws,
        _root: root,
    }
}

fn sources(ws: &Path) -> Vec<(String, String)> {
    policy::READ_SET
        .iter()
        .map(|rel| {
            (
                rel.to_string(),
                pilot_capture::sha256_hex(&std::fs::read(ws.join(rel)).unwrap()),
            )
        })
        .collect()
}

fn slot(env: &Env, id: &str, prompt: &str) -> PreparedSlot {
    PreparedSlot {
        slot_id: id.into(),
        workspace: env.workspace.clone(),
        initial_user_text: prompt.into(),
        expected_sources: sources(&env.workspace),
    }
}

fn verification(
    env: &Env,
    fx: &Fixture,
    scope: Scope,
    faults: VerificationFaults,
) -> LaunchAdmission {
    LaunchAdmission::Verification(VerificationAdmission {
        run_dir: env.run_dir.clone(),
        omp: OMP.into(),
        scope,
        fixture_addr: fx.addr,
        fixture_ca_pem: fx.ca_pem.clone(),
        credential: CANARY.into(),
        faults,
        manifest: None,
    })
}

fn journal(run_dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(run_dir.join("journal.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn kinds(run_dir: &Path) -> Vec<String> {
    journal(run_dir)
        .iter()
        .map(|r| r["kind"].as_str().unwrap().to_string())
        .collect()
}

/// No file under `dir` contains `needle` (credential-echo privacy).
fn assert_absent_under(dir: &Path, needle: &str) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                let bytes = std::fs::read(&path).unwrap_or_default();
                let found = bytes.windows(needle.len()).any(|w| w == needle.as_bytes());
                assert!(!found, "{needle} leaked into {}", path.display());
            }
        }
    }
}

// Expected values are independently spelled out for the producer/consumer tests.
// No ambient environment values, private credential or private source is read.
fn checker_expected_environment(root: &Path) -> std::collections::BTreeMap<String, String> {
    let root = root.to_str().unwrap();
    [
        ("HOME", format!("{root}/home")),
        ("LANG", "C".into()),
        ("PATH", "/usr/bin:/bin".into()),
        ("TMPDIR", format!("{root}/tmp/")),
        ("XDG_CONFIG_HOME", format!("{root}/home/.config")),
        ("XDG_DATA_HOME", format!("{root}/home/.local/share")),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect()
}

fn checker_account_home() -> PathBuf {
    // SAFETY: caller-owned buffers, with all pointers checked before use.
    unsafe {
        let mut pwd: libc::passwd = std::mem::zeroed();
        let mut buf = vec![0 as libc::c_char; 16 * 1024];
        let mut out: *mut libc::passwd = std::ptr::null_mut();
        assert_eq!(
            libc::getpwuid_r(
                libc::getuid(),
                &mut pwd,
                buf.as_mut_ptr(),
                buf.len(),
                &mut out
            ),
            0
        );
        assert!(!out.is_null() && !pwd.pw_dir.is_null());
        let home = std::ffi::CStr::from_ptr(pwd.pw_dir).to_str().unwrap();
        std::fs::canonicalize(home).unwrap()
    }
}

fn checker_expected_profile(root: &Path) -> String {
    let root = root.to_str().unwrap();
    let home = checker_account_home();
    let home = home.to_str().unwrap();
    format!(
        "(version 1)(allow default)(deny network*)\
         (deny file-read-data (subpath \"{home}\"))(deny file-write* (subpath \"{home}\"))\
         (deny file-write* (subpath \"/private/tmp\"))(deny file-write* (subpath \"/private/var/folders\"))\
         (allow file-read-data (subpath \"{root}\"))(allow file-write* (subpath \"{root}\"))"
    )
}

fn checker_assert_diagnostics(receipt: &IndependentCheckReceipt) {
    use std::os::unix::fs::MetadataExt;
    let root = &receipt.checker_root;
    assert_eq!(receipt.kind, "bounded-pilot-v1-check-v2");
    assert_eq!(receipt.environment, checker_expected_environment(root));
    assert_eq!(receipt.sandbox_profile, checker_expected_profile(root));
    assert_eq!(receipt.compiler, Path::new(policy::RUSTC));
    assert_eq!(receipt.compiler_sha256, policy::RUSTC_SHA256);
    for directory in [root.clone(), root.join("diagnostics")] {
        let meta = std::fs::symlink_metadata(directory).unwrap();
        assert!(meta.is_dir());
        assert_eq!(meta.mode() & 0o7777, 0o700);
        assert_eq!(meta.uid(), unsafe { libc::getuid() });
    }
    for step in &receipt.steps {
        assert_eq!(
            step.stderr_path,
            format!("diagnostics/{}.stderr", step.step)
        );
        let path = root.join(&step.stderr_path);
        let meta = std::fs::symlink_metadata(&path).unwrap();
        assert!(meta.is_file() && !meta.file_type().is_symlink());
        assert_eq!(meta.mode() & 0o7777, 0o600);
        assert_eq!(meta.uid(), unsafe { libc::getuid() });
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.len() <= 1_048_576);
        assert_eq!(step.stderr_bytes, bytes.len() as u64);
        assert_eq!(step.stderr_sha256, pilot_capture::sha256_hex(&bytes));
        let executable = if step.step == "oracle_run" {
            assert_eq!(step.artifacts.len(), 1);
            step.artifacts[0].1.as_str()
        } else {
            policy::RUSTC_SHA256
        };
        assert_eq!(step.executable_sha256, executable);
    }
}

// The receipt fixture owns this fresh directory; create it with the same
// declared privacy as the real checker instead of inheriting an ambient mask.
fn private_checker_tempdir() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;
    let directory = fixture_private_tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    directory
}

fn fixture_receipt(capture: &pilot_capture::SlotCapture, root: &Path) -> IndependentCheckReceipt {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    let root_s = root.to_str().unwrap();
    let diagnostics = root.join("diagnostics");
    if !diagnostics.exists() {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&diagnostics)
            .unwrap();
    }
    assert_eq!(
        std::fs::symlink_metadata(&diagnostics).unwrap().mode() & 0o7777,
        0o700
    );
    for (name, _) in policy::CHECK_RECIPE {
        let path = diagnostics.join(format!("{name}.stderr"));
        if path.exists() {
            assert_eq!(std::fs::read(path).unwrap(), b"");
        } else {
            private_fixture_file(&path, b"");
        }
    }
    let steps = policy::CHECK_RECIPE
        .iter()
        .map(|(name, argv)| CheckStep {
            step: name.to_string(),
            argv: argv.iter().map(|a| a.replace("<ROOT>", root_s)).collect(),
            exit: Some(0),
            signal: None,
            timed_out: false,
            // Synthetic consumer receipt: expected metadata, not an observed
            // execution of the private oracle. Root verified this fixed digest.
            stdout_sha256: if *name == "oracle_run" {
                "aac03c0ded8403eb8bd3ca53e9880313cde11fa658db4fc7444d737eef33127d".into()
            } else {
                pilot_capture::sha256_hex(name.as_bytes())
            },
            executable_sha256: if *name == "oracle_run" {
                pilot_capture::sha256_hex(b"owned synthetic checker image; never executed")
            } else {
                policy::RUSTC_SHA256.into()
            },
            stderr_path: format!("diagnostics/{name}.stderr"),
            stderr_bytes: 0,
            stderr_sha256: pilot_capture::sha256_hex(b""),
            artifacts: match *name {
                "noalloc_compile" => vec![(
                    "out/librecords-noalloc.rlib".into(),
                    pilot_capture::sha256_hex(b"owned synthetic noalloc object"),
                )],
                "alloc_compile" => vec![(
                    "out/librecords-alloc.rlib".into(),
                    pilot_capture::sha256_hex(b"owned synthetic alloc object"),
                )],
                "oracle_compile" | "oracle_run" => vec![(
                    "out/check".into(),
                    pilot_capture::sha256_hex(b"owned synthetic checker image; never executed"),
                )],
                _ => unreachable!("frozen checker step"),
            },
            wall_ms: 100,
        })
        .collect();
    IndependentCheckReceipt {
        kind: "bounded-pilot-v1-check-v2".into(),
        slot_id: capture.slot_id.clone(),
        checker_root: root.to_path_buf(),
        candidate: capture.candidate_inventory.clone(),
        oracle_sha256: policy::ORACLE_SHA256.into(),
        compiler: policy::RUSTC.into(),
        compiler_sha256: policy::RUSTC_SHA256.into(),
        compiler_version: "rustc 1.97.1 (fixture)".into(),
        environment: checker_expected_environment(root),
        sandbox_profile: checker_expected_profile(root),
        steps,
        fixture: true,
    }
}

// ---------------------------------------------------------------------------
// Stage 1: actual-host no-provider qualification matrix.
// ---------------------------------------------------------------------------

/// Native bootstrap, Ready before the sole prompt, keyless/no bypass,
/// exactly one count then one unchanged generation, the native response
/// path, identity join and normal exit; then the clean probe-to-coding
/// handoff preserves counters and the paid deadline.
#[test]
fn stage1_probe_end_to_end_and_clean_handoff() {
    require_host();
    let e = env();
    let fx = Fixture::start(vec![
        Reply::Count(2048),
        fixture::text_reply("msg_probe01", "PILOT_TRANSPORT_OK"),
    ]);
    let mut session =
        CaptureSession::open(verification(&e, &fx, Scope::Probe, Default::default())).unwrap();
    let capture = session
        .run_slot(slot(&e, policy::PROBE_SLOT, policy::PROBE_INSTRUCTION))
        .unwrap();
    assert!(
        matches!(capture.disposition, Disposition::Completed { .. }),
        "{:?}",
        capture.disposition
    );
    assert_eq!(capture.prompt_status.as_deref(), Some("completed"));
    let seen = fx.seen();
    assert_eq!(
        seen.iter()
            .map(|s| (s.method.as_str(), s.path.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("POST", "/v1/messages/count_tokens"),
            ("POST", "/v1/messages")
        ]
    );
    assert_eq!(fx.remaining_script(), 0);
    for s in &seen {
        assert_eq!(s.header("x-api-key"), Some(CANARY));
        assert_eq!(
            s.header("anthropic-version"),
            Some(policy::ANTHROPIC_VERSION)
        );
        assert_eq!(s.header("accept-encoding"), Some("identity"));
        assert_eq!(s.header("x-claude-code-session-id"), None);
        assert_eq!(s.header("content-encoding"), None);
        assert!(!String::from_utf8_lossy(&s.body).contains(policy::BRIDGE_SYNTHETIC_KEY));
    }
    // The Messages body is the host's exact serialized body.
    let records = journal(session.run_dir());
    assert_eq!(records[0]["kind"], "admitted");
    let validated = records
        .iter()
        .find(|r| r["kind"] == "request_validated")
        .unwrap();
    assert_eq!(
        validated["body_sha256"],
        pilot_capture::sha256_hex(&seen[1].body)
    );
    let messages: Value = serde_json::from_slice(&seen[1].body).unwrap();
    assert_eq!(messages["max_tokens"], 4096);
    assert_eq!(messages["service_tier"], "standard_only");
    assert_eq!(messages["output_config"]["effort"], "xhigh");
    let count: Value = serde_json::from_slice(&seen[0].body).unwrap();
    for omitted in [
        "max_tokens",
        "stream",
        "service_tier",
        "context_management",
        "metadata",
    ] {
        assert!(
            count.get(omitted).is_none(),
            "{omitted} in count projection"
        );
    }
    assert!(count["thinking"].get("block_binding").is_none());
    // Native response and identity join.
    let g = &capture.generations[0];
    assert_eq!(g.message_id.as_deref(), Some("msg_probe01"));
    assert_eq!(g.outcome, "settled");
    let usage = g.usage.as_ref().unwrap();
    assert!(usage.complete);
    assert_eq!(usage.inclusive_input, Some(1240));
    let native = capture.native.as_ref().unwrap();
    assert_eq!(
        native.joins,
        vec![("msg_probe01".to_string(), native.joins[0].1.clone())]
    );
    assert_eq!(native.joins[0].1.len(), 1);
    assert!(native.unmatched_native_response_ids.is_empty());
    let exit = capture.host_exit.as_ref().unwrap();
    assert!(exit.normal && exit.group_gone && !exit.forced, "{exit:?}");
    // Ready preceded the prompt; reservation and attempt preceded HTTP.
    let order = kinds(session.run_dir());
    let pos = |k: &str| {
        order
            .iter()
            .position(|x| x == k)
            .unwrap_or_else(|| panic!("{k} missing from {order:?}"))
    };
    assert!(pos("bridge_ready") < pos("prompt_sent"));
    assert!(pos("reservation") < pos("http_begin"));
    assert!(pos("paid_phase_started") < pos("http_begin"));
    let counters = session.counters();
    assert_eq!(counters.outstanding_micro_usd, 0);
    assert_eq!(counters.spent_micro_usd, 1240 * 10 + 12 * 25);
    assert_eq!(
        (
            counters.generation_permits,
            counters.count_calls,
            counters.messages_attempts
        ),
        (1, 1, 1)
    );
    // No credential reached the host's private root.
    assert_absent_under(&session.run_dir().join("hosts"), CANARY);
    // The host's own network attempts (local model discovery) were denied by
    // the sandbox; it never reached the fixture or any provider directly.
    let logs = std::fs::read_dir(session.run_dir().join("hosts"))
        .unwrap()
        .map(|h| h.unwrap().path().join("home/.omp/logs"))
        .filter(|d| d.is_dir())
        .flat_map(|d| {
            std::fs::read_dir(d)
                .unwrap()
                .map(|f| std::fs::read_to_string(f.unwrap().path()).unwrap())
        })
        .collect::<String>();
    let discovery: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("model discovery failed"))
        .collect();
    assert!(
        discovery.iter().all(|l| l.contains("EPERM")),
        "{discovery:?}"
    );
    // Check the pinned host's expected provider-named prewarm diagnostic.
    // Its reported ENOTFOUND does not establish OS-level denial ordering or
    // a complete inventory of startup network attempts.
    let provider_lines: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("api.anthropic.com"))
        .collect();
    assert!(
        provider_lines
            .iter()
            .all(|l| l.contains("Model connection prewarm failed") && l.contains("ENOTFOUND")),
        "host reached for the provider outside the relay: {provider_lines:?}"
    );

    let seal = session.seal_probe(&capture).unwrap();
    let receipt = session.finish().unwrap();
    // The seal is the journal's terminal record: finishing a sealed probe
    // appends nothing, so the seal stays valid for the handoff.
    assert_eq!(receipt.journal_records, seal.journal_records);
    assert_eq!(receipt.journal_sha256, seal.journal_sha256);
    assert_eq!(seal.attempt_seq, 2);
    let sealed_bytes = std::fs::read(e.run_dir.join("journal.jsonl")).unwrap();

    // Verification never accepts a credential it does not own, also not
    // through the handoff.
    let not_owned = LaunchAdmission::Verification(VerificationAdmission {
        credential: "sk-ant-api03-not-an-owned-verification-key".into(),
        ..match verification(&e, &fx, Scope::Coding, Default::default()) {
            LaunchAdmission::Verification(v) => v,
            LaunchAdmission::Live(_) => unreachable!(),
        }
    });
    assert_eq!(
        CaptureSession::open_after_probe(&seal, not_owned)
            .err()
            .unwrap()
            .code,
        "verification_credential_not_owned"
    );
    // Seal fields the journal does not state are refused.
    let inflated = pilot_capture::ProbeSeal {
        paid_deadline_ns: seal.paid_deadline_ns + 1_000_000_000,
        ..seal.clone()
    };
    assert_eq!(
        CaptureSession::open_after_probe(
            &inflated,
            verification(&e, &fx, Scope::Coding, Default::default())
        )
        .err()
        .unwrap()
        .code,
        "probe_seal_not_journaled"
    );
    let tampered = pilot_capture::ProbeSeal {
        journal_sha256: "0".repeat(64),
        ..seal.clone()
    };
    assert_eq!(
        CaptureSession::open_after_probe(
            &tampered,
            verification(&e, &fx, Scope::Coding, Default::default())
        )
        .err()
        .unwrap()
        .code,
        "journal_digest_mismatch"
    );

    // Clean handoff: counters, identity sequences and the paid deadline
    // carry over and the journal continues after the sealed digest.
    let fx3 = Fixture::start(vec![]);
    let coding = CaptureSession::open_after_probe(
        &seal,
        verification(&e, &fx3, Scope::Coding, Default::default()),
    )
    .unwrap();
    assert_eq!(coding.counters(), seal.counters);
    let after = journal(coding.run_dir());
    let handoff = after.last().unwrap();
    assert_eq!(handoff["kind"], "coding_handoff");
    assert_eq!(after.len() as u64, seal.journal_records + 1);
    assert!(
        std::fs::read(coding.run_dir().join("journal.jsonl"))
            .unwrap()
            .starts_with(&sealed_bytes)
    );
    assert_owned_private_run_tree(&e.run_dir);
    assert_owned_private_tree(&e.workspace, None);
}

/// An owned malformed key reaches the same claimed-run boundary as live
/// credential loading. No host is launched and the TLS fixture sees no HTTP.
#[test]
fn claimed_credential_refusal_is_durable_and_append_failure_stays_closed() {
    require_host();
    for (fail_after, expected) in [
        (None, "credential_shape_invalid"),
        (Some(0), "journal_append_failed"),
    ] {
        let e = env();
        let fx = Fixture::start(vec![]);
        let malformed = format!("{CANARY} invalid");
        let mut admission = match verification(
            &e,
            &fx,
            Scope::Probe,
            VerificationFaults {
                journal_fail_after: fail_after,
            },
        ) {
            LaunchAdmission::Verification(v) => v,
            LaunchAdmission::Live(_) => unreachable!(),
        };
        admission.credential = malformed.clone();
        let fault = CaptureSession::open(LaunchAdmission::Verification(admission))
            .err()
            .unwrap();
        assert_eq!(fault.code, expected);
        assert!(!fault.to_string().contains(CANARY));
        assert!(e.run_dir.join("claim.json").is_file());
        assert!(
            fx.seen().is_empty(),
            "credential refusal sent an HTTP request"
        );
        assert_eq!(
            std::fs::read_dir(e.run_dir.join("hosts")).unwrap().count(),
            0
        );
        if fail_after.is_none() {
            let records = journal(&e.run_dir);
            assert_eq!(records.len(), 1);
            assert_eq!(records[0]["kind"], "halt");
            assert_eq!(records[0]["phase"], "credential_admission");
            assert_eq!(records[0]["class"], "authority");
            assert_eq!(records[0]["code"], "credential_shape_invalid");
            assert_eq!(records[0]["seq"], 1);
            assert_eq!(records[0]["prev"], "");
        } else {
            assert_eq!(fault.class, FaultClass::Persistence);
            assert!(
                std::fs::read(e.run_dir.join("journal.jsonl"))
                    .unwrap()
                    .is_empty()
            );
        }
        assert_absent_under(&e.run_dir, CANARY);
        assert_absent_under(&e.run_dir, &malformed);
        assert_eq!(
            CaptureSession::open(verification(&e, &fx, Scope::Probe, Default::default()))
                .err()
                .unwrap()
                .code,
            "run_dir_claim_failed",
            "credential refusal must not release the consumed run claim"
        );
        assert!(fx.seen().is_empty());
    }
}

#[test]
fn coding_credential_refusal_preserves_the_seal_and_prevents_continuation() {
    require_host();
    let e = env();
    let fx = Fixture::start(vec![
        Reply::Count(2048),
        fixture::text_reply("msg_refusal_probe", "PILOT_TRANSPORT_OK"),
    ]);
    let mut probe =
        CaptureSession::open(verification(&e, &fx, Scope::Probe, Default::default())).unwrap();
    let capture = probe
        .run_slot(slot(&e, policy::PROBE_SLOT, policy::PROBE_INSTRUCTION))
        .unwrap();
    let seal = probe.seal_probe(&capture).unwrap();
    probe.finish().unwrap();
    let journal_path = e.run_dir.join("journal.jsonl");
    let prefix = std::fs::read(&journal_path).unwrap();
    assert_eq!(pilot_capture::sha256_hex(&prefix), seal.journal_sha256);
    assert_eq!(journal(&e.run_dir)[0]["kind"], "admitted");
    assert_eq!(fx.seen().len(), 2);

    let malformed = format!("{CANARY} invalid");
    let mut admission = match verification(&e, &fx, Scope::Coding, Default::default()) {
        LaunchAdmission::Verification(v) => v,
        LaunchAdmission::Live(_) => unreachable!(),
    };
    admission.credential = malformed.clone();
    let fault = CaptureSession::open_after_probe(&seal, LaunchAdmission::Verification(admission))
        .err()
        .unwrap();
    assert_eq!(fault.code, "credential_shape_invalid");
    assert!(!fault.to_string().contains(CANARY));
    let bytes = std::fs::read(&journal_path).unwrap();
    assert!(bytes.starts_with(&prefix));
    let records = journal(&e.run_dir);
    assert_eq!(records.len() as u64, seal.journal_records + 1);
    let refusal = records.last().unwrap();
    assert_eq!(refusal["kind"], "halt");
    assert_eq!(refusal["phase"], "credential_admission");
    assert_eq!(refusal["code"], "credential_shape_invalid");
    let previous = prefix
        .split(|b| *b == b'\n')
        .rfind(|line| !line.is_empty())
        .unwrap();
    assert_eq!(refusal["prev"], pilot_capture::sha256_hex(previous));
    assert_eq!(refusal["seq"], seal.journal_records + 1);
    assert_absent_under(&e.run_dir, CANARY);
    assert_absent_under(&e.run_dir, &malformed);
    assert_eq!(
        fx.seen().len(),
        2,
        "coding refusal sent another HTTP request"
    );

    // Verification admission must refuse before opening even a missing
    // caller-selected journal. An unowned credential is an admission failure,
    // not an admitted credential-shape refusal to append to a run.
    let root = e.run_dir.parent().unwrap();
    let missing_journal = pilot_capture::ProbeSeal {
        run_dir: root.join("missing-probe-run"),
        ..seal.clone()
    };
    assert!(!missing_journal.run_dir.exists());
    let mut unowned = match verification(&e, &fx, Scope::Coding, Default::default()) {
        LaunchAdmission::Verification(v) => v,
        LaunchAdmission::Live(_) => unreachable!(),
    };
    unowned.credential = "not-owned-and-must-not-be-examined".into();
    assert_eq!(
        CaptureSession::open_after_probe(&missing_journal, LaunchAdmission::Verification(unowned))
            .err()
            .unwrap()
            .code,
        "verification_credential_not_owned"
    );
    // An owned-prefix credential passes admission but has invalid wire shape.
    // The terminal journal must refuse first, without appending another halt.
    let mut owned_malformed = match verification(&e, &fx, Scope::Coding, Default::default()) {
        LaunchAdmission::Verification(v) => v,
        LaunchAdmission::Live(_) => unreachable!(),
    };
    owned_malformed.credential = malformed.clone();
    assert_eq!(
        CaptureSession::open_after_probe(&seal, LaunchAdmission::Verification(owned_malformed))
            .err()
            .unwrap()
            .code,
        "journal_digest_mismatch"
    );
    // Live cannot inspect a verification journal, even when its path is missing.
    let unopened = root.join("unopened-evidence");
    assert_eq!(
        CaptureSession::open_after_probe(
            &missing_journal,
            LaunchAdmission::Live(LiveAdmission {
                run_dir: e.run_dir.clone(),
                omp: unopened.clone(),
                scope: Scope::Coding,
                contract: unopened.clone(),
                authority: root.join("unused-authority.json"),
                launch_manifest: unopened.clone(),
                ratecard: unopened.clone(),
                compatibility: Some(unopened.clone()),
                credential_file: unopened,
            })
        )
        .err()
        .unwrap()
        .code,
        "probe_handoff_mode_mismatch"
    );
    assert!(!root.join("unused-authority.json.claim").exists());
    // Replacing the supplied seal's digest/count cannot legitimize the halt.
    let rewritten = pilot_capture::ProbeSeal {
        journal_sha256: pilot_capture::sha256_hex(&bytes),
        journal_records: seal.journal_records + 1,
        ..seal.clone()
    };
    assert_eq!(
        CaptureSession::open_after_probe(
            &rewritten,
            verification(&e, &fx, Scope::Coding, Default::default()),
        )
        .err()
        .unwrap()
        .code,
        "probe_seal_not_journaled"
    );
    assert_eq!(std::fs::read(&journal_path).unwrap(), bytes);
    assert_eq!(fx.seen().len(), 2);
}

/// Retry refusal: a failed generation is never re-sent, including by the
/// host's own recovery paths.
#[test]
fn stage1_messages_failure_halts_without_any_second_request() {
    require_host();
    let e = env();
    let fx = Fixture::start(vec![
        Reply::Count(2048),
        Reply::Status(
            529,
            vec![("content-type".into(), "application/json".into())],
            b"{\"type\":\"error\"}".to_vec(),
        ),
    ]);
    let mut session =
        CaptureSession::open(verification(&e, &fx, Scope::Probe, Default::default())).unwrap();
    let capture = session
        .run_slot(slot(&e, policy::PROBE_SLOT, policy::PROBE_INSTRUCTION))
        .unwrap();
    assert!(
        matches!(capture.disposition, Disposition::Halted { .. }),
        "{:?}",
        capture.disposition
    );
    assert_eq!(
        fx.paths(),
        vec!["/v1/messages/count_tokens", "/v1/messages"]
    );
    // The lost generation keeps its full reservation.
    assert_eq!(
        session.counters().outstanding_micro_usd,
        policy::FULL_RESERVATION_MICRO_USD
    );
    assert_eq!(session.halted().unwrap().class, FaultClass::Transport);
    // A halted session admits nothing further.
    assert_eq!(
        session
            .run_slot(slot(&e, policy::PROBE_SLOT, policy::PROBE_INSTRUCTION))
            .unwrap_err()
            .code,
        "session_halted"
    );
    assert!(session.seal_probe(&capture).is_err());
}

/// Credential-echo privacy: non-success bodies and headers, and split or
/// coalesced SSE error events carrying the owned canary, never reach the
/// bridge, the native session or capture evidence; stream failure is not a
/// clean EOF.
#[test]
fn stage1_upstream_errors_carrying_the_credential_never_reach_the_host() {
    require_host();
    // Non-success with the canary in body and headers.
    let e = env();
    let fx = Fixture::start(vec![
        Reply::Count(2048),
        Reply::Status(
            401,
            vec![
                ("content-type".into(), "application/json".into()),
                ("x-echo".into(), CANARY.into()),
                ("request-id".into(), CANARY.into()),
            ],
            format!("{{\"error\":{{\"message\":\"bad key {CANARY}\"}}}}").into_bytes(),
        ),
    ]);
    let mut session =
        CaptureSession::open(verification(&e, &fx, Scope::Probe, Default::default())).unwrap();
    let capture = session
        .run_slot(slot(&e, policy::PROBE_SLOT, policy::PROBE_INSTRUCTION))
        .unwrap();
    assert!(matches!(capture.disposition, Disposition::Halted { .. }));
    assert_eq!(fx.paths().len(), 2);
    assert_absent_under(session.run_dir(), CANARY);

    // A valid event coalesced with an error event, and an error split
    // across chunks.
    let e = env();
    let error = fixture::event(
        "error",
        &json!({"type":"error","error":{"type":"overloaded_error","message": format!("echo {CANARY}")}}),
    );
    let mut coalesced = fixture::message_start("msg_err01", 900, 0, 0);
    coalesced.extend(fixture::event("ping", &json!({"type":"ping"})));
    coalesced.extend(&error[..error.len() / 2]);
    let fx = Fixture::start(vec![
        Reply::Count(1024),
        Reply::Sse(vec![coalesced, error[error.len() / 2..].to_vec()]),
    ]);
    let mut session =
        CaptureSession::open(verification(&e, &fx, Scope::Probe, Default::default())).unwrap();
    let capture = session
        .run_slot(slot(&e, policy::PROBE_SLOT, policy::PROBE_INSTRUCTION))
        .unwrap();
    let halt = session.halted().unwrap();
    assert_eq!(
        (halt.class, halt.code),
        (FaultClass::Transport, "upstream_error_event")
    );
    assert!(matches!(capture.disposition, Disposition::Halted { .. }));
    assert_ne!(capture.prompt_status.as_deref(), Some("completed"));
    assert_eq!(fx.paths().len(), 2, "no retry after a withheld error event");
    assert_absent_under(session.run_dir(), CANARY);
    assert_absent_under(session.run_dir(), "overloaded_error");
}

/// A coding slot: an admitted read tool call, a legitimate followup with a
/// fresh permit, an independent check seal and frozen slot order.
#[test]
fn coding_slot_tool_followup_seal_and_order() {
    require_host();
    let e = env();
    let fx = Fixture::start(vec![
        Reply::Count(3000),
        fixture::tool_reply(
            "msg_c1",
            "toolu_01read",
            "read",
            json!({"path": "src/lib.rs", "i": "inspect"}),
        ),
        Reply::Count(3400),
        fixture::text_reply("msg_c2", "Done."),
    ]);
    let mut session =
        CaptureSession::open(verification(&e, &fx, Scope::Coding, Default::default())).unwrap();
    // Out of order slots are refused before any launch.
    assert_eq!(
        session.run_slot(slot(&e, "1A", TASK)).unwrap_err().code,
        "slot_out_of_order"
    );
    let capture = session.run_slot(slot(&e, "1B", TASK)).unwrap();
    assert!(
        matches!(capture.disposition, Disposition::Completed { .. }),
        "{:?}",
        capture.disposition
    );
    assert_eq!(fx.paths().len(), 4);
    assert_eq!(capture.generations.len(), 2);
    assert!(capture.generations[1].followup);
    assert_eq!(capture.generations[0].tool_uses, vec!["read".to_string()]);
    assert_eq!(capture.tools.len(), 1);
    assert!(capture.tools[0].admitted);
    assert_eq!(capture.tools[0].outcome.as_deref(), Some("ok"));
    let native = capture.native.as_ref().unwrap();
    assert_eq!(native.joins.len(), 2);
    assert!(native.joins.iter().all(|(_, entries)| entries.len() == 1));
    // The next slot waits for the independent check.
    assert_eq!(
        session.run_slot(slot(&e, "1A", TASK)).unwrap_err().code,
        "previous_slot_unsealed"
    );
    let checker = private_checker_tempdir();
    let root = std::fs::canonicalize(checker.path()).unwrap();
    let sealed = session
        .seal_slot(&capture, &fixture_receipt(&capture, &root))
        .unwrap();
    assert_eq!(sealed.outcome, pilot_capture::CheckOutcome::Pass);
    assert_eq!(session.counters().outstanding_micro_usd, 0);
    let receipt = session.finish().unwrap();
    assert_eq!(receipt.slots.len(), 8);
    assert_eq!(receipt.slots[0].state, "sealed");
    assert!(receipt.slots[1..].iter().all(|s| s.state == "not_started"));
    assert_owned_private_run_tree(&e.run_dir);
    assert_owned_private_tree(&e.workspace, None);
}

/// A forbidden path is blocked before execution (behaviour failure, not a
/// halt); protected files stay unchanged.
#[test]
fn forbidden_edit_is_blocked_and_recorded_without_effect() {
    require_host();
    let e = env();
    let before = sources(&e.workspace);
    let fx = Fixture::start(vec![
        Reply::Count(3000),
        fixture::tool_reply(
            "msg_f1",
            "toolu_01edit",
            "edit",
            json!({"path": "Cargo.toml", "old_string": "records", "new_string": "pwned", "i": "edit"}),
        ),
        Reply::Count(3400),
        fixture::text_reply("msg_f2", "Stopped."),
    ]);
    let mut session =
        CaptureSession::open(verification(&e, &fx, Scope::Coding, Default::default())).unwrap();
    let capture = session.run_slot(slot(&e, "1B", TASK)).unwrap();
    assert!(session.halted().is_none(), "{:?}", session.halted());
    assert_eq!(capture.tools.len(), 1);
    assert!(!capture.tools[0].admitted);
    assert_eq!(
        capture.tools[0].refusal.as_deref(),
        Some("path_not_admitted")
    );
    assert_eq!(capture.tools[0].outcome.as_deref(), Some("blocked"));
    assert_eq!(sources(&e.workspace), before);
    // An independent check whose recipe drifted is a provenance fault that
    // halts the session; it is never a correctness result.
    let checker = private_checker_tempdir();
    let root = std::fs::canonicalize(checker.path()).unwrap();
    let mut drifted = fixture_receipt(&capture, &root);
    drifted.steps[1].argv.retain(|a| a != "--cfg");
    let refused = session.seal_slot(&capture, &drifted).unwrap_err();
    assert_eq!(
        (refused.class, refused.code),
        (FaultClass::CheckerProvenance, "check_recipe_mismatch")
    );
    assert_eq!(session.halted().unwrap().code, "check_recipe_mismatch");
}

/// Counter missingness: a final usage delta without output tokens leaves
/// usage unknown (never zero), retains the full reservation and halts.
#[test]
fn missing_final_output_counter_retains_the_reservation_and_halts() {
    require_host();
    let e = env();
    let mut end = fixture::event(
        "message_delta",
        &json!({"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},
                "usage":{"input_tokens":900,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}),
    );
    end.extend(fixture::event(
        "message_stop",
        &json!({"type":"message_stop"}),
    ));
    let fx = Fixture::start(vec![
        Reply::Count(1024),
        Reply::Sse(vec![
            fixture::message_start("msg_u1", 900, 0, 0),
            fixture::text_block(0, "x"),
            end,
        ]),
    ]);
    let mut session =
        CaptureSession::open(verification(&e, &fx, Scope::Probe, Default::default())).unwrap();
    let capture = session
        .run_slot(slot(&e, policy::PROBE_SLOT, policy::PROBE_INSTRUCTION))
        .unwrap();
    let halt = session.halted().unwrap();
    assert_eq!(
        (halt.class, halt.code),
        (FaultClass::Usage, "usage_incomplete")
    );
    let usage = capture.generations[0].usage.as_ref().unwrap();
    assert!(!usage.complete);
    assert_eq!(usage.output, None);
    assert_eq!(
        session.counters().outstanding_micro_usd,
        policy::FULL_RESERVATION_MICRO_USD
    );
    assert_eq!(session.counters().spent_micro_usd, 0);
    assert_eq!(fx.paths().len(), 2);
}

/// A failed count is a global counting fault: no Messages request, the
/// provisional reservation is released, the permit stays consumed.
#[test]
fn count_failure_halts_before_messages() {
    require_host();
    let e = env();
    let fx = Fixture::start(vec![Reply::Status(500, vec![], b"{}".to_vec())]);
    let mut session =
        CaptureSession::open(verification(&e, &fx, Scope::Coding, Default::default())).unwrap();
    let capture = session.run_slot(slot(&e, "1B", TASK)).unwrap();
    assert_eq!(fx.paths(), vec!["/v1/messages/count_tokens"]);
    assert_eq!(session.halted().unwrap().class, FaultClass::Counting);
    assert_eq!(session.counters().outstanding_micro_usd, 0);
    assert_eq!(session.counters().generation_permits, 1);
    assert!(matches!(capture.disposition, Disposition::Halted { .. }));
}

/// An estimate above 30,720 is a pre-Messages input-cap refusal that closes
/// the coding slot as partial; for the probe it halts.
#[test]
fn input_cap_refusal_closes_the_slot_without_messages() {
    require_host();
    let e = env();
    let fx = Fixture::start(vec![Reply::Count(30_721)]);
    let mut session =
        CaptureSession::open(verification(&e, &fx, Scope::Coding, Default::default())).unwrap();
    let capture = session.run_slot(slot(&e, "1B", TASK)).unwrap();
    assert_eq!(fx.paths(), vec!["/v1/messages/count_tokens"]);
    assert_eq!(
        capture.disposition,
        Disposition::Partial {
            reason: "input_cap_refusal".into()
        }
    );
    assert!(session.halted().is_none());
    assert_eq!(session.counters().outstanding_micro_usd, 0);

    let e = env();
    let fx = Fixture::start(vec![Reply::Count(30_721)]);
    let mut probe =
        CaptureSession::open(verification(&e, &fx, Scope::Probe, Default::default())).unwrap();
    probe
        .run_slot(slot(&e, policy::PROBE_SLOT, policy::PROBE_INSTRUCTION))
        .unwrap();
    assert_eq!(probe.halted().unwrap().code, "probe_input_cap_refusal");
}

/// Capture/append failure before transmission: nothing is sent.
#[test]
fn journal_failure_prevents_any_transmission() {
    require_host();
    let e = env();
    let fx = Fixture::start(vec![Reply::Count(2048), fixture::text_reply("msg_j1", "x")]);
    // admitted, host_launched, bridge_ready, prompt_sent, request_validated
    // succeed; paid_phase_started (the 6th record) fails.
    let faults = VerificationFaults {
        journal_fail_after: Some(5),
    };
    let mut session = CaptureSession::open(verification(&e, &fx, Scope::Probe, faults)).unwrap();
    let result = session.run_slot(slot(&e, policy::PROBE_SLOT, policy::PROBE_INSTRUCTION));
    assert!(
        fx.paths().is_empty(),
        "no provider request after a journal failure"
    );
    match result {
        Ok(capture) => assert!(matches!(capture.disposition, Disposition::Halted { .. })),
        Err(f) => assert_eq!(f.class, FaultClass::Persistence),
    }
}

/// Preview rendering stops before counting: no request at all, and the two
/// arms of a pair differ only in the initial-user evidence slot.
#[test]
fn preview_render_pairs_match_without_any_request() {
    require_host();
    let e = env();
    let a = pilot_capture::render_binding(
        &PreviewAdmission {
            run_dir: e.run_dir.clone(),
            omp: OMP.into(),
        },
        &slot(&e, "1A", TASK),
    )
    .unwrap();
    let b_prompt = format!("{TASK}\n\nInitial repository evidence:\nsrc/lib.rs:1 pub fn decode");
    let b = pilot_capture::render_binding(
        &PreviewAdmission {
            run_dir: e.run_dir.with_file_name("run-b"),
            omp: OMP.into(),
        },
        &slot(&e, "1B", &b_prompt),
    )
    .unwrap();
    a.matches_pair(&b).unwrap();
    assert_ne!(a.initial_user_sha256, b.initial_user_sha256);
    assert_eq!(
        a.bridge_sha256,
        pilot_capture::sha256_hex(pilot_capture::host::BRIDGE_SOURCE)
    );
}

// ---------------------------------------------------------------------------
// Admission separation (no host launch).
// ---------------------------------------------------------------------------

#[test]
fn live_admission_refuses_by_name_before_credentials_or_claims() {
    let e = env();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let live = |contract: PathBuf, authority: PathBuf| {
        LaunchAdmission::Live(LiveAdmission {
            run_dir: e.run_dir.clone(),
            omp: OMP.into(),
            scope: Scope::Probe,
            contract,
            authority,
            launch_manifest: e.run_dir.with_file_name("manifest.json"),
            ratecard: e.run_dir.with_file_name("ratecard.json"),
            compatibility: None,
            credential_file: e.run_dir.with_file_name("no-such-credential"),
        })
    };
    let contract = repo.join("specs/003-agent-retrieval-context/contracts/bounded-pilot-v1.md");
    let missing_authority = CaptureSession::open(live(
        contract.clone(),
        e.run_dir.with_file_name("authority.json"),
    ));
    assert_eq!(
        missing_authority.err().unwrap().code,
        "authority_receipt_missing"
    );
    let wrong_contract = CaptureSession::open(live(
        repo.join("Cargo.toml"),
        e.run_dir.with_file_name("authority.json"),
    ));
    assert_eq!(
        wrong_contract.err().unwrap().code,
        "contract_digest_mismatch"
    );
    assert!(!e.run_dir.exists(), "a refused admission claims nothing");
}

#[test]
fn verification_cannot_target_anything_but_numeric_loopback() {
    let e = env();
    let refused = CaptureSession::open(LaunchAdmission::Verification(VerificationAdmission {
        run_dir: e.run_dir.clone(),
        omp: OMP.into(),
        scope: Scope::Probe,
        fixture_addr: "203.0.113.7:443".parse().unwrap(),
        fixture_ca_pem: vec![],
        credential: CANARY.into(),
        faults: Default::default(),
        manifest: None,
    }));
    assert_eq!(
        refused.err().unwrap().code,
        "verification_endpoint_not_loopback"
    );
    let not_owned = CaptureSession::open(LaunchAdmission::Verification(VerificationAdmission {
        run_dir: e.run_dir.clone(),
        omp: OMP.into(),
        scope: Scope::Probe,
        fixture_addr: "127.0.0.1:9".parse().unwrap(),
        fixture_ca_pem: vec![],
        credential: "sk-ant-api03-real-looking".into(),
        faults: Default::default(),
        manifest: None,
    }));
    assert_eq!(
        not_owned.err().unwrap().code,
        "verification_credential_not_owned"
    );
}

/// The verification transport reaches only the owned fixture: its own CA,
/// its numeric loopback address, logical `api.anthropic.com` URLs.
#[test]
fn verification_transport_speaks_only_to_the_owned_fixture() {
    let fx = Fixture::start(vec![Reply::Count(77), fixture::text_reply("msg_t1", "ok")]);
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let endpoint = wire::Endpoint::Verification {
            addr: fx.addr,
            ca_pem: fx.ca_pem.clone(),
        };
        let transport =
            wire::Transport::new(&endpoint, wire::Credential::new(CANARY).unwrap()).unwrap();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        let beta =
            "effort-2025-11-24,thinking-binding-controls-2026-08-01,context-management-2025-06-27";
        let count = transport
            .count(
                br#"{"model":"claude-sonnet-5-5","messages":[]}"#,
                beta,
                deadline,
            )
            .await
            .unwrap();
        assert_eq!(
            (count.input_tokens, count.input_tokens_lexeme.as_str()),
            (77, "77")
        );
        let mut stream = transport
            .messages(br#"{"model":"claude-sonnet-5-5"}"#, beta, deadline)
            .await
            .unwrap();
        let mut events = 0;
        while let Some(_e) = stream.next_event(deadline).await.unwrap() {
            events += 1;
        }
        let obs = stream.observation();
        assert!(obs.terminal);
        assert_eq!(obs.message_id.as_deref(), Some("msg_t1"));
        assert!(obs.usage.complete());
        assert_eq!(events, 6);
    });
    let seen = fx.seen();
    assert_eq!(seen[0].header("host"), Some("api.anthropic.com"));
    assert_eq!(seen[1].header("accept"), Some("text/event-stream"));
    assert_eq!(seen[1].header("x-api-key"), Some(CANARY));
    // A different CA is not trusted.
    let other = Fixture::start(vec![Reply::Count(1)]);
    rt.block_on(async {
        let endpoint = wire::Endpoint::Verification {
            addr: other.addr,
            ca_pem: fx.ca_pem.clone(),
        };
        let transport =
            wire::Transport::new(&endpoint, wire::Credential::new(CANARY).unwrap()).unwrap();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        let beta =
            "effort-2025-11-24,thinking-binding-controls-2026-08-01,context-management-2025-06-27";
        let failure = transport.count(b"{}", beta, deadline).await.unwrap_err();
        assert_eq!(failure.fault.code, "count_transport_failed");
    });
    assert!(other.seen().is_empty());
}

/// A check receipt that stops early although every present step succeeded
/// is missing provenance: a halt, never a behaviour `Fail`.
#[test]
fn truncated_all_success_check_receipt_is_a_provenance_fault() {
    require_host();
    let e = env();
    let fx = Fixture::start(vec![Reply::Count(30_721)]);
    let mut session =
        CaptureSession::open(verification(&e, &fx, Scope::Coding, Default::default())).unwrap();
    let capture = session.run_slot(slot(&e, "1B", TASK)).unwrap();
    let checker = private_checker_tempdir();
    let root = std::fs::canonicalize(checker.path()).unwrap();
    let mut truncated = fixture_receipt(&capture, &root);
    truncated.steps.truncate(2);
    let refused = session.seal_slot(&capture, &truncated).unwrap_err();
    assert_eq!(
        (refused.class, refused.code),
        (FaultClass::CheckerProvenance, "check_steps_truncated")
    );
    // A receipt that stops at a failing step is a genuine behaviour result.
    let e = env();
    let fx = Fixture::start(vec![Reply::Count(30_721)]);
    let mut session =
        CaptureSession::open(verification(&e, &fx, Scope::Coding, Default::default())).unwrap();
    let capture = session.run_slot(slot(&e, "1B", TASK)).unwrap();
    let mut failed = fixture_receipt(&capture, &root);
    failed.steps.truncate(1);
    failed.steps[0].exit = Some(1);
    let sealed = session.seal_slot(&capture, &failed).unwrap();
    assert_eq!(sealed.outcome, pilot_capture::CheckOutcome::Fail);
}

// ---------------------------------------------------------------------------
// Full pipeline qualification (private evidence; no provider).
// ---------------------------------------------------------------------------

/// The whole pilot apparatus end to end without any provider: pinned-input
/// verification, seed install, the four real view materializations, real
/// renders and frozen manifests, the probe and its seal, the journal-verified
/// handoff, all eight coding slots through the real host and sandbox, the
/// real contained checker on each candidate, seals and the descriptive
/// report. Only the model is scripted: arm B applies the upstream repair
/// with the native edit tool; arm A stops without editing. Needs the owner's
/// private evidence roots, named by `CF_PILOT_EVIDENCE_INPUTS` (an
/// `EvidenceInputs` JSON file).
#[test]
#[ignore = "needs private pilot evidence roots; run with --ignored and CF_PILOT_EVIDENCE_INPUTS"]
fn full_pilot_pipeline_qualifies_without_provider() {
    use context_foundry::pilot_capture::apparatus::{self, EvidenceInputs, PilotRoot};
    require_host();
    let inputs_path = std::env::var("CF_PILOT_EVIDENCE_INPUTS").expect("CF_PILOT_EVIDENCE_INPUTS");
    let inputs: EvidenceInputs =
        serde_json::from_slice(&std::fs::read(&inputs_path).unwrap()).unwrap();
    let verified = inputs.verify().unwrap();
    // Retain this private run on success or failure. An explicit fresh parent
    // lets the owner keep the full proof outside temporary-directory cleanup.
    let evidence = std::env::var_os("CF_PILOT_PIPELINE_EVIDENCE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| fixture_private_tempdir().unwrap().keep());
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::symlink_metadata(&evidence).unwrap();
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    assert_eq!(metadata.uid(), unsafe { libc::getuid() });
    assert_eq!(metadata.mode() & 0o7777, 0o700);
    let root = PilotRoot::create(&std::fs::canonicalize(evidence).unwrap().join("pilot")).unwrap();
    eprintln!("retained full-pipeline evidence: {}", root.root.display());
    apparatus::install_seed(&root, &verified).unwrap();
    let views = apparatus::materialize_views(&root, &verified).unwrap();
    assert_eq!(views.len(), 4);
    assert!(
        views
            .iter()
            .all(|v| v.header.contains("semantic:ready") && v.delivered_o200k_tokens <= 2048)
    );
    PilotRoot::open(&root.root).unwrap();
    let capture_run = root.root.join("runs/pilot");
    let (probe_manifest, coding_manifest) =
        apparatus::build_manifests(&root, &verified, &capture_run).unwrap();

    // The scripted model: the upstream repair is one replaced hunk.
    let seed = std::fs::read_to_string(inputs.paired_root.join("inputs/seed-lib.rs")).unwrap();
    let fixed =
        std::fs::read_to_string(inputs.paired_root.join("oracle/control/src/lib.rs")).unwrap();
    let (a, b): (Vec<&str>, Vec<&str>) = (
        seed.split_inclusive('\n').collect(),
        fixed.split_inclusive('\n').collect(),
    );
    let pre = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let post = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let old = a[pre..a.len() - post].concat();
    let new = b[pre..b.len() - post].concat();
    let mut script = vec![
        Reply::Count(4000),
        fixture::text_reply("msg_probe", "PILOT_TRANSPORT_OK"),
    ];
    for slot_id in policy::SLOT_ORDER {
        if slot_id.ends_with('B') {
            script.push(Reply::Count(9000));
            script.push(fixture::tool_reply(
                &format!("msg_{slot_id}_1"),
                &format!("toolu_{slot_id}_edit"),
                "edit",
                json!({"path": "src/lib.rs", "old_string": old, "new_string": new, "i": "repair escape"}),
            ));
            script.push(Reply::Count(9400));
            script.push(fixture::text_reply(
                &format!("msg_{slot_id}_2"),
                "Repaired.",
            ));
        } else {
            script.push(Reply::Count(3000));
            script.push(fixture::text_reply(
                &format!("msg_{slot_id}_1"),
                "No change made.",
            ));
        }
    }
    let fx = Fixture::start(script);
    let admission = |scope: Scope, run: &str, manifest| {
        LaunchAdmission::Verification(VerificationAdmission {
            run_dir: root.root.join("runs").join(run),
            omp: verified.inputs.omp.clone(),
            scope,
            fixture_addr: fx.addr,
            fixture_ca_pem: fx.ca_pem.clone(),
            credential: CANARY.into(),
            faults: Default::default(),
            manifest: Some(manifest),
        })
    };
    let mut probe = CaptureSession::open(admission(Scope::Probe, "pilot", probe_manifest)).unwrap();
    let (probe_capture, seal) = apparatus::run_probe(&mut probe, &root).unwrap();
    let seal =
        seal.unwrap_or_else(|f| panic!("probe not sealed: {f:?} {:?}", probe_capture.disposition));
    probe.finish().unwrap();
    assert_owned_private_run_tree(&capture_run);
    PilotRoot::open(&root.root).unwrap();
    let sealed_bytes = std::fs::read(capture_run.join("journal.jsonl")).unwrap();
    let request_count = fx.seen().len();
    assert_eq!(
        CaptureSession::open_after_probe(
            &seal,
            admission(Scope::Coding, "unused", coding_manifest.clone()),
        )
        .err()
        .unwrap()
        .code,
        "run_dir_authority_mismatch"
    );
    assert_eq!(
        std::fs::read(capture_run.join("journal.jsonl")).unwrap(),
        sealed_bytes
    );
    assert_eq!(fx.seen().len(), request_count);
    assert!(!root.root.join("runs/unused").exists());
    let coding =
        CaptureSession::open_after_probe(&seal, admission(Scope::Coding, "pilot", coding_manifest))
            .unwrap();
    assert_eq!(coding.counters(), seal.counters);
    let report = apparatus::run_coding(coding, &root, &verified).unwrap();
    // Test-owned archival output, not a claim about the production CLI writer.
    fixture_private_write(
        root.root.join("renders/full-pipeline-report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();

    assert_eq!(report.receipt.halt, None, "{:?}", report.slots);
    assert!(
        report.slots.iter().all(|r| r.state == "sealed"),
        "{:?}",
        report.slots
    );
    let arm = |name: &str| report.arms.iter().find(|a| a.arm == name).unwrap().clone();
    assert_eq!((arm("A").checked, arm("A").passed), (4, 0));
    assert_eq!((arm("B").checked, arm("B").passed), (4, 4));
    assert!(
        report
            .pairs
            .iter()
            .all(|p| p.valid && p.b_minus_a == Some(1))
    );
    let c = report.receipt.counters;
    assert_eq!(
        (c.generation_permits, c.count_calls, c.messages_attempts),
        (13, 13, 13)
    );
    assert_eq!(c.outstanding_micro_usd, 0);
    assert_eq!(fx.remaining_script(), 0);
    // B arms carry the frozen view verbatim; A arms carry none.
    let requests = fx.seen();
    let firsts: Vec<&fixture::Seen> = requests
        .iter()
        .filter(|s| s.path == "/v1/messages")
        .collect();
    assert_eq!(firsts.len(), 13);
    assert_absent_under(&root.root, CANARY);
    // Every planned arm left its archived workspace or its live one.
    assert!(root.root.join("archive/after-1B").is_dir());
    assert_owned_private_run_tree(&capture_run);
    PilotRoot::open(&root.root).unwrap();
    inputs
        .verify()
        .expect("frozen input hashes must remain unchanged");
}

/// Every live gate refuses by name, in order, before any credential is
/// read; an authority receipt admits one run only. Synthetic evidence only.
#[test]
fn live_gates_refuse_in_order_and_claim_the_authority_once() {
    use pilot_capture::{
        AuthorityReceipt, LaunchManifest, ManifestSlot, ManifestView, PinEntry, Ratecard,
        RenderCommitments,
    };
    require_host();
    let e = env();
    let dir = e.run_dir.parent().unwrap().to_path_buf();
    let write = |name: &str, bytes: &[u8]| {
        let p = dir.join(name);
        fixture_private_write(&p, bytes).unwrap();
        p
    };
    let sha = |p: &Path| pilot_capture::sha256_hex(&std::fs::read(p).unwrap());
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let contract = repo.join("specs/003-agent-retrieval-context/contracts/bounded-pilot-v1.md");
    let pinned = write("pinned.txt", b"pinned input");
    let views: Vec<ManifestView> = (1..=4u8)
        .map(|pair| {
            let p = write(
                &format!("view-{pair}.txt"),
                format!("view {pair}").as_bytes(),
            );
            ManifestView {
                pair,
                sha256: sha(&p),
                path: p,
                delivered_o200k_tokens: 3,
                store_revision: "r5".into(),
                citations_sha256: "0".repeat(64),
                recipe_id: "recipe".into(),
            }
        })
        .collect();
    let manifest = LaunchManifest {
        kind: pilot_capture::LAUNCH_MANIFEST_KIND.into(),
        run_dir: dir.join("run-5"),
        contract_sha256: policy::CONTRACT_SHA256.into(),
        omp_sha256: policy::OMP_SHA256.into(),
        slots: vec![ManifestSlot {
            slot_id: policy::PROBE_SLOT.into(),
            arm: "probe".into(),
            workspace: e.workspace.clone(),
            prompt_sha256: pilot_capture::sha256_hex(policy::PROBE_INSTRUCTION.as_bytes()),
            sources: sources(&e.workspace),
            render: RenderCommitments {
                system_sha256: String::new(),
                tools_sha256: String::new(),
                settings_sha256: String::new(),
                stable_prefix_sha256: String::new(),
                beta: String::new(),
                cache_layout: vec![],
                initial_user_sha256: String::new(),
            },
        }],
        pins: vec![PinEntry {
            name: "pinned".into(),
            sha256: sha(&pinned),
            path: pinned,
        }],
        allocation_sha256: "0".repeat(64),
        views,
        isolation_root: dir.clone(),
    };
    let manifest_path = write("manifest.json", &serde_json::to_vec(&manifest).unwrap());
    let card = |account: &str| Ratecard {
        kind: "bounded-pilot-v1-ratecard".into(),
        model: policy::MODEL.into(),
        service: "standard".into(),
        input_usd_per_mtok: "2".into(),
        cache_write_usd_per_mtok: "2.5".into(),
        cache_read_usd_per_mtok: "0.1".into(),
        output_usd_per_mtok: "10".into(),
        account_identity_sha256: account.into(),
        evidence_sha256: "a".repeat(64),
    };
    let exe = sha(&std::env::current_exe().unwrap());
    let receipt = |rights: bool, implementation: &str, ratecard: &Path| AuthorityReceipt {
        kind: "bounded-pilot-v1-authority".into(),
        contract_sha256: policy::CONTRACT_SHA256.into(),
        implementation_sha256: implementation.into(),
        launch_manifest_sha256: sha(&manifest_path),
        ratecard_sha256: sha(ratecard),
        scope: Scope::Probe,
        authorized_by: "synthetic test owner".into(),
        authorized_at: "2026-11-06".into(),
        rights_approved: rights,
    };
    let open = |authority: &Path, ratecard: &Path, run: &str| {
        CaptureSession::open(LaunchAdmission::Live(LiveAdmission {
            run_dir: dir.join(run),
            omp: OMP.into(),
            scope: Scope::Probe,
            contract: contract.clone(),
            authority: authority.to_path_buf(),
            launch_manifest: manifest_path.clone(),
            ratecard: ratecard.to_path_buf(),
            compatibility: None,
            credential_file: dir.join("no-such-credential"),
        }))
        .err()
        .unwrap()
        .code
    };
    let unbound = write(
        "ratecard-unbound.json",
        &serde_json::to_vec(&card("")).unwrap(),
    );
    let bound = write(
        "ratecard.json",
        &serde_json::to_vec(&card(&"b".repeat(64))).unwrap(),
    );
    let a1 = write(
        "authority-1.json",
        &serde_json::to_vec(&receipt(false, &exe, &bound)).unwrap(),
    );
    assert_eq!(open(&a1, &bound, "run-5"), "rights_not_approved");
    assert!(!dir.join("authority-1.json.claim").exists());
    assert!(!dir.join("run-5").exists());
    let a2 = write(
        "authority-2.json",
        &serde_json::to_vec(&receipt(true, &"c".repeat(64), &bound)).unwrap(),
    );
    assert_eq!(open(&a2, &bound, "run-5"), "implementation_digest_mismatch");
    assert!(!dir.join("authority-2.json.claim").exists());
    assert!(!dir.join("run-5").exists());
    let a3 = write(
        "authority-3.json",
        &serde_json::to_vec(&receipt(true, &exe, &unbound)).unwrap(),
    );
    assert_eq!(open(&a3, &unbound, "run-5"), "ratecard_account_unbound");
    assert!(!dir.join("authority-3.json.claim").exists());
    assert!(!dir.join("run-5").exists());
    fixture_private_write(dir.join("view-2.txt"), b"tampered").unwrap();
    let a4 = write(
        "authority-4.json",
        &serde_json::to_vec(&receipt(true, &exe, &bound)).unwrap(),
    );
    assert_eq!(open(&a4, &bound, "run-5"), "launch_view_changed");
    assert!(!dir.join("authority-4.json.claim").exists());
    assert!(!dir.join("run-5").exists());
    fixture_private_write(dir.join("view-2.txt"), b"view 2").unwrap();
    // Every gate passes; the credential is the last thing read and is
    // absent, after the authority and the run were claimed exclusively.
    assert_eq!(open(&a4, &bound, "run-5"), "credential_missing");
    assert!(dir.join("authority-4.json.claim").is_file());
    assert!(dir.join("run-5/claim.json").is_file());
    let refused = journal(&dir.join("run-5"));
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0]["kind"], "halt");
    assert_eq!(refused[0]["phase"], "credential_admission");
    assert_eq!(refused[0]["class"], "privacy");
    assert_eq!(refused[0]["code"], "credential_missing");
    assert_eq!(refused[0]["prev"], "");
    assert_eq!(refused[0]["seq"], 1);
    assert_eq!(open(&a4, &bound, "run-6"), "run_dir_authority_mismatch");

    // A distinct authority is consumed by an owned malformed file as well.
    // These bytes are synthetic, private and outside the claimed run.
    use std::os::unix::fs::PermissionsExt;
    let malformed = format!("{CANARY} invalid");
    let key_path = write("no-such-credential", malformed.as_bytes());
    std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    // A new positive claim has its own manifest-bound target. Earlier owned
    // fixture bytes are preserved; the open closure below selects this path.
    let malformed_manifest_path = write(
        "malformed-key-manifest.json",
        &serde_json::to_vec(&LaunchManifest {
            run_dir: dir.join("run-7"),
            ..manifest.clone()
        })
        .unwrap(),
    );
    let a5 = write(
        "authority-5.json",
        &serde_json::to_vec(&AuthorityReceipt {
            launch_manifest_sha256: sha(&malformed_manifest_path),
            ..receipt(true, &exe, &bound)
        })
        .unwrap(),
    );
    let malformed_open = |run: &str| {
        CaptureSession::open(LaunchAdmission::Live(LiveAdmission {
            run_dir: dir.join(run),
            omp: OMP.into(),
            scope: Scope::Probe,
            contract: contract.clone(),
            authority: a5.clone(),
            launch_manifest: malformed_manifest_path.clone(),
            ratecard: bound.clone(),
            compatibility: None,
            credential_file: key_path.clone(),
        }))
        .err()
        .unwrap()
        .code
    };
    assert_eq!(malformed_open("run-7"), "credential_shape_invalid");
    assert!(dir.join("authority-5.json.claim").is_file());
    assert!(dir.join("run-7/claim.json").is_file());
    let refused = journal(&dir.join("run-7"));
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0]["kind"], "halt");
    assert_eq!(refused[0]["class"], "authority");
    assert_eq!(refused[0]["code"], "credential_shape_invalid");
    assert_absent_under(&dir.join("run-7"), CANARY);
    assert_absent_under(&dir.join("run-7"), &malformed);
    assert_eq!(malformed_open("run-8"), "run_dir_authority_mismatch");
    assert!(!dir.join("run-8").exists());
    // Single-use claim behavior is covered directly by the library test;
    // a differently named root must now fail identity before claim reuse.
    for run in ["run-6", "run-8"] {
        assert!(!dir.join(run).exists(), "{run} claimed despite refusal");
    }

    // A synthetic Live-shaped coding admission must reach the journal gate,
    // not stop at mode/manifest/authority validation. The owned verification
    // probe supplies current boot/clock/bridge fields only: relabeling this
    // evidence does not establish an actual Live-compatible probe history.
    // This auxiliary Verification probe is a distinct run. Keep its real host
    // outputs outside the Live fixture's isolation root, whose executable
    // declaration belongs only to the run named by its authority manifest.
    let probe_env = env();
    let fx = Fixture::start(vec![
        Reply::Count(2048),
        fixture::text_reply("msg_live_shape_probe", "PILOT_TRANSPORT_OK"),
    ]);
    let mut probe = CaptureSession::open(verification(
        &probe_env,
        &fx,
        Scope::Probe,
        Default::default(),
    ))
    .unwrap();
    let capture = probe
        .run_slot(slot(
            &probe_env,
            policy::PROBE_SLOT,
            policy::PROBE_INSTRUCTION,
        ))
        .unwrap();
    let verification_seal = probe.seal_probe(&capture).unwrap();
    probe.finish().unwrap();
    assert_eq!(fx.seen().len(), 2);

    let terminal_run = dir.join("run-5");
    let terminal_path = terminal_run.join("journal.jsonl");
    let terminal_bytes = std::fs::read(&terminal_path).unwrap();
    let live_shape = pilot_capture::ProbeSeal {
        mode: pilot_capture::Mode::Live,
        run_dir: terminal_run.clone(),
        journal_sha256: pilot_capture::sha256_hex(&terminal_bytes),
        journal_records: journal(&terminal_run).len() as u64,
        ..verification_seal
    };
    let cited = write(
        "synthetic-live-seal.json",
        &serde_json::to_vec(&live_shape).unwrap(),
    );
    let coding_manifest = LaunchManifest {
        run_dir: terminal_run.clone(),
        slots: policy::SLOT_ORDER
            .iter()
            .map(|id| ManifestSlot {
                slot_id: (*id).into(),
                arm: id.chars().last().unwrap().to_string(),
                ..manifest.slots[0].clone()
            })
            .collect(),
        ..manifest.clone()
    };
    let coding_manifest_path = write(
        "coding-manifest.json",
        &serde_json::to_vec(&coding_manifest).unwrap(),
    );
    let coding_authority = write(
        "unused-coding-authority.json",
        &serde_json::to_vec(&AuthorityReceipt {
            scope: Scope::Coding,
            launch_manifest_sha256: sha(&coding_manifest_path),
            ..receipt(true, &exe, &bound)
        })
        .unwrap(),
    );
    let missing_key = dir.join("coding-key-must-not-be-read");
    assert!(!missing_key.exists());
    let fault = CaptureSession::open_after_probe(
        &live_shape,
        LaunchAdmission::Live(LiveAdmission {
            run_dir: terminal_run,
            omp: OMP.into(),
            scope: Scope::Coding,
            contract: contract.clone(),
            authority: coding_authority,
            launch_manifest: coding_manifest_path,
            ratecard: bound.clone(),
            compatibility: Some(cited),
            credential_file: missing_key,
        }),
    )
    .err()
    .unwrap();
    assert_eq!(
        fault.code, "probe_seal_not_journaled",
        "validated Live coding admission must reject the terminal journal before claim/key"
    );
    assert!(
        !dir.join("unused-coding-authority.json.claim").exists(),
        "terminal evidence must not consume fresh coding authority"
    );
    assert_eq!(std::fs::read(&terminal_path).unwrap(), terminal_bytes);
    assert_eq!(
        fx.seen().len(),
        2,
        "no additional fixture request is permitted"
    );
}

// Owned JSON files intentionally keep this test-only addition compilable on
// the preceding capture format. The format/path failures must be runtime results.
struct AuthorityFiles {
    root: PathBuf,
    scope: Scope,
    manifest: Value,
    authority: Value,
}

fn private_fixture_file(path: &Path, bytes: &[u8]) {
    use std::os::unix::fs::PermissionsExt;
    fixture_private_write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

impl AuthorityFiles {
    fn new(e: &Env, label: &str, scope: Scope, run_dir: &Path) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let root = e.run_dir.parent().unwrap().join(label);
        fixture_private_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let pinned = root.join("pinned.txt");
        private_fixture_file(&pinned, b"owned admission fixture");
        let views: Vec<Value> = (1..=4u8)
            .map(|pair| {
                let path = root.join(format!("view-{pair}.txt"));
                private_fixture_file(&path, format!("owned view {pair}").as_bytes());
                json!({"pair":pair, "path":path,
                    "sha256":pilot_capture::sha256_hex(&std::fs::read(&path).unwrap()),
                    "delivered_o200k_tokens":3, "store_revision":"owned-fixture",
                    "citations_sha256":"0".repeat(64), "recipe_id":"owned-fixture"})
            })
            .collect();
        let ids = match scope {
            Scope::Probe => vec![policy::PROBE_SLOT],
            Scope::Coding => policy::SLOT_ORDER.to_vec(),
        };
        let slots: Vec<Value> = ids.into_iter().map(|id| json!({
            "slot_id":id, "arm":if id == policy::PROBE_SLOT { "probe".to_string() } else { id.chars().last().unwrap().to_string() },
            "workspace":e.workspace, "prompt_sha256":"0".repeat(64), "sources":sources(&e.workspace),
            "render":{"system_sha256":"", "tools_sha256":"", "settings_sha256":"", "stable_prefix_sha256":"",
                      "beta":"", "cache_layout":[], "initial_user_sha256":""}
        })).collect();
        let card = json!({"kind":"bounded-pilot-v1-ratecard", "model":policy::MODEL,
            "service":"standard", "input_usd_per_mtok":"2", "cache_write_usd_per_mtok":"2.5",
            "cache_read_usd_per_mtok":"0.1", "output_usd_per_mtok":"10",
            "account_identity_sha256":"b".repeat(64), "evidence_sha256":"a".repeat(64)});
        let card_bytes = serde_json::to_vec(&card).unwrap();
        private_fixture_file(&root.join("ratecard.json"), &card_bytes);
        let mut files = Self {
            root,
            scope,
            manifest: json!({"kind":"bounded-pilot-v1-launch-v2", "run_dir":run_dir,
                "contract_sha256":policy::CONTRACT_SHA256, "omp_sha256":policy::OMP_SHA256,
                "slots":slots, "pins":[{"name":"owned input", "path":pinned,
                    "sha256":pilot_capture::sha256_hex(b"owned admission fixture")}],
                "allocation_sha256":"0".repeat(64), "views":views,
                "isolation_root":e.run_dir.parent().unwrap()}),
            authority: json!({"kind":"bounded-pilot-v1-authority", "contract_sha256":policy::CONTRACT_SHA256,
                "implementation_sha256":pilot_capture::sha256_hex(&std::fs::read(std::env::current_exe().unwrap()).unwrap()),
                "launch_manifest_sha256":"", "ratecard_sha256":pilot_capture::sha256_hex(&card_bytes),
                "scope":scope, "authorized_by":"owned synthetic test", "authorized_at":"2026-11-11",
                "rights_approved":true}),
        };
        files.save_manifest();
        files
    }

    fn save_manifest(&mut self) {
        let bytes = serde_json::to_vec(&self.manifest).unwrap();
        private_fixture_file(&self.root.join("manifest.json"), &bytes);
        self.authority["launch_manifest_sha256"] = json!(pilot_capture::sha256_hex(&bytes));
        private_fixture_file(
            &self.root.join("authority.json"),
            &serde_json::to_vec(&self.authority).unwrap(),
        );
    }

    fn admission(&self, run_dir: &Path, compatibility: Option<PathBuf>) -> LaunchAdmission {
        LaunchAdmission::Live(LiveAdmission {
            run_dir: run_dir.to_path_buf(),
            omp: OMP.into(),
            scope: self.scope,
            contract: Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("specs/003-agent-retrieval-context/contracts/bounded-pilot-v1.md"),
            authority: self.root.join("authority.json"),
            launch_manifest: self.root.join("manifest.json"),
            ratecard: self.root.join("ratecard.json"),
            compatibility,
            credential_file: self.root.join("synthetic-key"),
        })
    }

    fn authority_sha256(&self) -> String {
        pilot_capture::sha256_hex(&std::fs::read(self.root.join("authority.json")).unwrap())
    }

    fn assert_unclaimed(&self) {
        assert!(!self.root.join("authority.json.claim").exists());
    }
}

#[test]
fn authority_missing_or_legacy_run_binding_refuses_before_claim() {
    require_host();
    for (label, kind, remove_run) in [
        ("legacy", "bounded-pilot-v1-launch", true),
        ("missing", "bounded-pilot-v1-launch-v2", true),
        ("null", "bounded-pilot-v1-launch-v2", false),
    ] {
        let e = env();
        let mut files = AuthorityFiles::new(&e, label, Scope::Probe, &e.run_dir);
        files.manifest["kind"] = json!(kind);
        if remove_run {
            files.manifest.as_object_mut().unwrap().remove("run_dir");
        } else {
            files.manifest["run_dir"] = Value::Null;
        }
        files.save_manifest();
        let before = std::fs::read(files.root.join("manifest.json")).unwrap();
        let fault = CaptureSession::open(files.admission(&e.run_dir, None))
            .err()
            .unwrap();
        assert_eq!(fault.code, "launch_manifest_invalid", "{label}");
        files.assert_unclaimed();
        assert!(!e.run_dir.exists());
        assert!(!files.root.join("synthetic-key").exists());
        assert_eq!(
            std::fs::read(files.root.join("manifest.json")).unwrap(),
            before
        );
    }
}

#[test]
fn authority_run_mismatch_and_noncanonical_binding_refuse_before_claim() {
    require_host();
    let e = env();
    let mut files = AuthorityFiles::new(&e, "path-authority", Scope::Probe, &e.run_dir);
    let other = e.run_dir.with_file_name("other-run");
    let fault = CaptureSession::open(files.admission(&other, None))
        .err()
        .unwrap();
    assert_eq!(fault.code, "run_dir_authority_mismatch");
    files.assert_unclaimed();
    assert!(!e.run_dir.exists() && !other.exists());
    // Equivalent filesystem resolution must not silently canonicalize the
    // authorized bytes: the manifest itself must already name the canonical root.
    let alias_parent = e.run_dir.with_file_name("parent-alias");
    std::os::unix::fs::symlink(e.run_dir.parent().unwrap(), &alias_parent).unwrap();
    // Path equality can normalize away '.', so retain the exact JSON spelling.
    for stored in [
        "run".to_string(),
        format!("{}/./run", e.run_dir.parent().unwrap().display()),
        alias_parent.join("run").to_str().unwrap().to_string(),
    ] {
        files.manifest["run_dir"] = json!(stored);
        files.save_manifest();
        let fault = CaptureSession::open(files.admission(&e.run_dir, None))
            .err()
            .unwrap();
        assert_eq!(fault.code, "launch_manifest_run_not_canonical", "{stored}");
        files.assert_unclaimed();
        assert!(!e.run_dir.exists());
        assert!(!files.root.join("synthetic-key").exists());
    }
}

#[test]
fn authority_probe_admission_is_chained_before_a_session_is_returned() {
    require_host();
    let e = env();
    let files = AuthorityFiles::new(&e, "positive-authority", Scope::Probe, &e.run_dir);
    // Only this owned dummy is read. open/finish construct and close a client;
    // run_slot/count/messages are never called, and no host is launched.
    private_fixture_file(&files.root.join("synthetic-key"), CANARY.as_bytes());
    let session = CaptureSession::open(files.admission(&e.run_dir, None)).unwrap();
    let records = journal(session.run_dir());
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["kind"], "admitted");
    assert_eq!(
        records[0]["authority_receipt_sha256"],
        files.authority_sha256()
    );
    assert_eq!(
        records[0]["run_dir"],
        json!(std::fs::canonicalize(&e.run_dir).unwrap())
    );
    assert_eq!(records[0]["scope"], json!(Scope::Probe));
    assert_eq!(records[0]["prev"], "");
    assert_eq!(records[0]["seq"], 1);
    assert_eq!(session.counters().count_calls, 0);
    assert_eq!(session.counters().messages_attempts, 0);
    assert_eq!(
        std::fs::read_dir(e.run_dir.join("hosts")).unwrap().count(),
        0
    );
    assert_absent_under(&e.run_dir, CANARY);
    let receipt = session.finish().unwrap();
    assert!(!receipt.paid_started);
}

#[test]
fn authority_journal_admission_append_failure_returns_no_session() {
    require_host();
    let e = env();
    let fx = Fixture::start(vec![]);
    let fault = CaptureSession::open(verification(
        &e,
        &fx,
        Scope::Probe,
        VerificationFaults {
            journal_fail_after: Some(0),
        },
    ))
    .err()
    .unwrap();
    assert_eq!(fault.code, "journal_append_failed");
    assert_eq!(fault.class, FaultClass::Persistence);
    assert!(e.run_dir.join("claim.json").is_file());
    assert!(
        std::fs::read(e.run_dir.join("journal.jsonl"))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        std::fs::read_dir(e.run_dir.join("hosts")).unwrap().count(),
        0
    );
    assert!(fx.seen().is_empty());
}

#[test]
fn authority_handoff_path_mismatch_preserves_sealed_bytes_and_requests() {
    require_host();
    let e = env();
    let fx = Fixture::start(vec![
        Reply::Count(2048),
        fixture::text_reply("msg_path_probe", "PILOT_TRANSPORT_OK"),
    ]);
    let mut probe =
        CaptureSession::open(verification(&e, &fx, Scope::Probe, Default::default())).unwrap();
    let capture = probe
        .run_slot(slot(&e, policy::PROBE_SLOT, policy::PROBE_INSTRUCTION))
        .unwrap();
    let seal = probe.seal_probe(&capture).unwrap();
    probe.finish().unwrap();
    let bytes = std::fs::read(e.run_dir.join("journal.jsonl")).unwrap();
    let other = e.run_dir.with_file_name("other-owned-run");
    fixture_private_dir(&other).unwrap();
    let alias = e.run_dir.with_file_name("wrong-run-alias");
    std::os::unix::fs::symlink(&other, &alias).unwrap();
    for declared in [e.run_dir.with_file_name("unused"), alias] {
        let mut admission = match verification(&e, &fx, Scope::Coding, Default::default()) {
            LaunchAdmission::Verification(v) => v,
            LaunchAdmission::Live(_) => unreachable!(),
        };
        admission.run_dir = declared;
        assert_eq!(
            CaptureSession::open_after_probe(&seal, LaunchAdmission::Verification(admission))
                .err()
                .unwrap()
                .code,
            "probe_run_dir_mismatch"
        );
        assert_eq!(
            std::fs::read(e.run_dir.join("journal.jsonl")).unwrap(),
            bytes
        );
        assert_eq!(fx.seen().len(), 2);
    }
    let coding = CaptureSession::open_after_probe(
        &seal,
        verification(&e, &fx, Scope::Coding, Default::default()),
    )
    .unwrap();
    assert_eq!(coding.counters(), seal.counters);
    let rows = journal(coding.run_dir());
    let last = rows.last().unwrap();
    assert_eq!(last["kind"], "coding_handoff");
    assert_eq!(last["run_dir"], json!(seal.run_dir));
    assert_eq!(last["scope"], json!(Scope::Coding));
    assert!(last.get("authority_receipt_sha256").unwrap().is_null());
    assert_eq!(fx.seen().len(), 2);
}

#[test]
fn authority_journal_handoff_append_failure_returns_no_session() {
    require_host();
    let e = env();
    let fx = Fixture::start(vec![
        Reply::Count(2048),
        fixture::text_reply("msg_append_probe", "PILOT_TRANSPORT_OK"),
    ]);
    let mut probe =
        CaptureSession::open(verification(&e, &fx, Scope::Probe, Default::default())).unwrap();
    let capture = probe
        .run_slot(slot(&e, policy::PROBE_SLOT, policy::PROBE_INSTRUCTION))
        .unwrap();
    let seal = probe.seal_probe(&capture).unwrap();
    probe.finish().unwrap();
    let bytes = std::fs::read(e.run_dir.join("journal.jsonl")).unwrap();
    let hosts = std::fs::read_dir(e.run_dir.join("hosts")).unwrap().count();
    let fault = CaptureSession::open_after_probe(
        &seal,
        verification(
            &e,
            &fx,
            Scope::Coding,
            VerificationFaults {
                journal_fail_after: Some(seal.journal_records),
            },
        ),
    )
    .err()
    .unwrap();
    assert_eq!(fault.code, "journal_append_failed");
    assert_eq!(fault.class, FaultClass::Persistence);
    assert_eq!(
        std::fs::read(e.run_dir.join("journal.jsonl")).unwrap(),
        bytes
    );
    assert_eq!(
        std::fs::read_dir(e.run_dir.join("hosts")).unwrap().count(),
        hosts
    );
    assert_eq!(fx.seen().len(), 2);
    // No permanent burn is claimed for an injected Verification append failure.
}

#[test]
fn authority_live_shaped_handoff_is_chained_to_exact_coding_authority() {
    require_host();
    let e = env();
    // This auxiliary Verification probe is a distinct run. Keep its real host
    // outputs outside the Live fixture's isolation root, whose executable
    // declaration belongs only to the run named by its authority manifest.
    let probe_env = env();
    let fx = Fixture::start(vec![
        Reply::Count(2048),
        fixture::text_reply("msg_synthetic_live_probe", "PILOT_TRANSPORT_OK"),
    ]);
    let mut probe = CaptureSession::open(verification(
        &probe_env,
        &fx,
        Scope::Probe,
        Default::default(),
    ))
    .unwrap();
    let capture = probe
        .run_slot(slot(
            &probe_env,
            policy::PROBE_SLOT,
            policy::PROBE_INSTRUCTION,
        ))
        .unwrap();
    let verification_seal = probe.seal_probe(&capture).unwrap();
    probe.finish().unwrap();

    let live_run = e.run_dir.with_file_name("synthetic-live-run");
    let probe_files = AuthorityFiles::new(&e, "live-probe-authority", Scope::Probe, &live_run);
    private_fixture_file(&probe_files.root.join("synthetic-key"), CANARY.as_bytes());
    let live_constructor = CaptureSession::open(probe_files.admission(&live_run, None)).unwrap();
    let first = journal(&live_run).remove(0);
    drop(live_constructor); // No run_slot, paid attempt or real Live probe.
    assert_eq!(
        first["authority_receipt_sha256"],
        probe_files.authority_sha256()
    );

    // Explicitly synthetic handoff input: the real constructor's admission
    // plus re-chained verification records. This is not Live-compatible history.
    let mut rows = journal(&probe_env.run_dir);
    rows[0] = first;
    rows.last_mut().unwrap()["run_dir"] = json!(live_run);
    let mut bytes = Vec::new();
    let mut prev = String::new();
    for (index, row) in rows.iter_mut().enumerate() {
        row["seq"] = json!(index + 1);
        row["prev"] = json!(prev);
        let line = serde_json::to_vec(row).unwrap();
        prev = pilot_capture::sha256_hex(&line);
        bytes.extend(line);
        bytes.push(b'\n');
    }
    private_fixture_file(&live_run.join("journal.jsonl"), &bytes);
    let seal = pilot_capture::ProbeSeal {
        mode: pilot_capture::Mode::Live,
        run_dir: live_run.clone(),
        journal_sha256: pilot_capture::sha256_hex(&bytes),
        journal_records: rows.len() as u64,
        ..verification_seal
    };
    let files = AuthorityFiles::new(&e, "live-coding-authority", Scope::Coding, &live_run);
    let cited = files.root.join("synthetic-seal.json");
    private_fixture_file(&cited, &serde_json::to_vec(&seal).unwrap());
    private_fixture_file(&files.root.join("synthetic-key"), CANARY.as_bytes());
    let wrong_run = e.run_dir.with_file_name("different-existing-run");
    fixture_private_dir(&wrong_run).unwrap();
    assert_eq!(
        CaptureSession::open_after_probe(&seal, files.admission(&wrong_run, Some(cited.clone())))
            .err()
            .unwrap()
            .code,
        "run_dir_authority_mismatch"
    );
    files.assert_unclaimed();
    assert_eq!(
        std::fs::read(live_run.join("journal.jsonl")).unwrap(),
        bytes
    );
    assert_eq!(fx.seen().len(), 2);
    let coding =
        CaptureSession::open_after_probe(&seal, files.admission(&live_run, Some(cited))).unwrap();
    assert_eq!(coding.counters(), seal.counters);
    let rows = journal(&live_run);
    let last = rows.last().unwrap();
    assert_eq!(last["kind"], "coding_handoff");
    assert_eq!(last["scope"], json!(Scope::Coding));
    assert_eq!(last["run_dir"], json!(live_run));
    assert_eq!(last["authority_receipt_sha256"], files.authority_sha256());
    assert_ne!(
        last["authority_receipt_sha256"],
        probe_files.authority_sha256()
    );
    let previous_line = bytes
        .split(|b| *b == b'\n')
        .rfind(|line| !line.is_empty())
        .unwrap();
    assert_eq!(last["prev"], pilot_capture::sha256_hex(previous_line));
    assert_eq!(last["seq"], seal.journal_records + 1);
    assert!(
        std::fs::read(live_run.join("journal.jsonl"))
            .unwrap()
            .starts_with(&bytes)
    );
    assert_eq!(fx.seen().len(), 2);
    coding.finish().unwrap();
}

#[test]
fn authority_manifests_cli_requires_explicit_run_dir_before_inputs() {
    let tmp = fixture_private_tempdir().unwrap();
    let root = tmp.path().join("must-not-be-created");
    let inputs = tmp.path().join("nonexistent-inputs.json");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_foundry-pilot-capture"))
        .args(["manifests", "--pilot-root"])
        .arg(&root)
        .arg("--inputs")
        .arg(&inputs)
        .current_dir(tmp.path())
        .env_clear()
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with("usage:"), "{stderr}");
    assert!(
        stderr
            .lines()
            .any(|line| line.contains("foundry-pilot-capture manifests")
                && line.contains("--run-dir DIR")),
        "{stderr}"
    );
    assert!(output.stdout.is_empty());
    assert!(!root.exists() && !inputs.exists());
}

// Independent expected metadata recovered and verified by the owner from the
// pinned oracle's existing successful evidence. No private output bytes are here.
const FIXTURE_ORACLE_COMPLETION_SHA256: &str =
    "aac03c0ded8403eb8bd3ca53e9880313cde11fa658db4fc7444d737eef33127d";

/// Synthetic receipt only: these inert byte identities are not compiler output
/// or proof of running the private oracle. Keep every other new binding valid
/// when mutating one assertion's field, so the refusal reaches its intended gate.
fn oracle_bound_fixture_receipt(
    capture: &pilot_capture::SlotCapture,
    root: &Path,
) -> IndependentCheckReceipt {
    let mut receipt = fixture_receipt(capture, root);
    receipt.steps[0].artifacts = vec![(
        "out/librecords-noalloc.rlib".into(),
        pilot_capture::sha256_hex(b"owned synthetic noalloc object"),
    )];
    receipt.steps[1].artifacts = vec![(
        "out/librecords-alloc.rlib".into(),
        pilot_capture::sha256_hex(b"owned synthetic alloc object"),
    )];
    let checker = pilot_capture::sha256_hex(b"owned synthetic checker image; never executed");
    receipt.steps[2].artifacts = vec![("out/check".into(), checker.clone())];
    receipt.steps[3].artifacts = vec![("out/check".into(), checker)];
    receipt.steps[3].stdout_sha256 = FIXTURE_ORACLE_COMPLETION_SHA256.into();
    receipt
}

/// The existing partial-slot pattern reaches the real seal_slot consumer using
/// exactly one owned CountTokens fixture response, without a Messages request.
fn oracle_awaiting_capture(e: &Env, fx: &Fixture) -> (CaptureSession, pilot_capture::SlotCapture) {
    let mut session =
        CaptureSession::open(verification(e, fx, Scope::Coding, Default::default())).unwrap();
    let capture = session.run_slot(slot(e, "1B", TASK)).unwrap();
    assert!(matches!(capture.disposition, Disposition::Partial { .. }));
    assert_eq!(fx.paths(), vec!["/v1/messages/count_tokens"]);
    assert!(session.halted().is_none());
    (session, capture)
}

fn oracle_assert_terminal_refusal(
    mut session: CaptureSession,
    capture: &pilot_capture::SlotCapture,
    receipt: &IndependentCheckReceipt,
    e: &Env,
    fx: &Fixture,
    code: &str,
) {
    let request_count = fx.seen().len();
    let fault = session.seal_slot(capture, receipt).unwrap_err();
    assert_eq!(fault.class, FaultClass::CheckerProvenance);
    assert_eq!(fault.code, code);
    assert_eq!(session.halted(), Some(fault));
    assert_eq!(
        session.run_slot(slot(e, "1A", TASK)).unwrap_err().code,
        "session_halted"
    );
    assert_eq!(fx.seen().len(), request_count);
    let report = session.finish().unwrap();
    assert_eq!(
        report.halt,
        Some((FaultClass::CheckerProvenance.as_str().into(), code.into()))
    );
    assert!(report.slots[1..].iter().all(|s| s.state == "not_started"));
    let rows = journal(&e.run_dir);
    assert_eq!(rows.iter().filter(|r| r["kind"] == "halt").count(), 1);
    assert!(!rows.iter().any(|r| r["kind"] == "slot_sealed"));
    let refused = rows.iter().find(|r| r["kind"] == "check_refused").unwrap();
    assert_eq!(
        refused["receipt_sha256"],
        pilot_capture::sha256_hex(&serde_json::to_vec(receipt).unwrap())
    );
    assert_eq!(rows.last().unwrap()["kind"], "finished");
    assert_eq!(fx.seen().len(), request_count);
}

#[test]
fn oracle_zero_exit_without_exact_completion_is_a_terminal_provenance_fault() {
    require_host();
    for stdout in [b"".as_slice(), b"owned wrong completion\n".as_slice()] {
        let e = env();
        let fx = Fixture::start(vec![Reply::Count(30_721)]);
        let (session, capture) = oracle_awaiting_capture(&e, &fx);
        let checker = private_checker_tempdir();
        let mut receipt =
            oracle_bound_fixture_receipt(&capture, &std::fs::canonicalize(checker.path()).unwrap());
        receipt.steps[3].stdout_sha256 = pilot_capture::sha256_hex(stdout);
        assert!(receipt.steps.iter().all(|step| step.exit == Some(0)));
        oracle_assert_terminal_refusal(
            session,
            &capture,
            &receipt,
            &e,
            &fx,
            "check_oracle_completion_mismatch",
        );
    }
}

#[test]
fn oracle_successful_steps_require_the_exact_named_artifact() {
    require_host();
    for (index, wrong_name) in [(0, false), (1, false), (2, false), (3, false), (2, true)] {
        let e = env();
        let fx = Fixture::start(vec![Reply::Count(30_721)]);
        let (session, capture) = oracle_awaiting_capture(&e, &fx);
        let checker = private_checker_tempdir();
        let mut receipt =
            oracle_bound_fixture_receipt(&capture, &std::fs::canonicalize(checker.path()).unwrap());
        if wrong_name {
            receipt.steps[index].artifacts[0].0 = "out/not-the-executed-checker".into();
        } else {
            receipt.steps[index].artifacts.clear();
        }
        oracle_assert_terminal_refusal(
            session,
            &capture,
            &receipt,
            &e,
            &fx,
            "check_artifact_unbound",
        );
    }
}

#[test]
fn oracle_executed_checker_must_match_the_compiled_checker() {
    require_host();
    let e = env();
    let fx = Fixture::start(vec![Reply::Count(30_721)]);
    let (session, capture) = oracle_awaiting_capture(&e, &fx);
    let checker = private_checker_tempdir();
    let mut receipt =
        oracle_bound_fixture_receipt(&capture, &std::fs::canonicalize(checker.path()).unwrap());
    receipt.steps[3].artifacts[0].1 =
        pilot_capture::sha256_hex(b"different owned synthetic executable");
    oracle_assert_terminal_refusal(
        session,
        &capture,
        &receipt,
        &e,
        &fx,
        "check_oracle_executable_mismatch",
    );
}

#[test]
fn oracle_bound_success_and_observed_nonzero_keep_distinct_outcomes() {
    require_host();
    for nonzero in [false, true] {
        let e = env();
        let fx = Fixture::start(vec![Reply::Count(30_721)]);
        let (mut session, capture) = oracle_awaiting_capture(&e, &fx);
        let checker = private_checker_tempdir();
        let mut receipt =
            oracle_bound_fixture_receipt(&capture, &std::fs::canonicalize(checker.path()).unwrap());
        if nonzero {
            receipt.steps[3].exit = Some(101);
            receipt.steps[3].stdout_sha256 =
                pilot_capture::sha256_hex(b"owned failing-program output");
        }
        let expected = if nonzero {
            pilot_capture::CheckOutcome::Fail
        } else {
            pilot_capture::CheckOutcome::Pass
        };
        assert_eq!(
            session.seal_slot(&capture, &receipt).unwrap().outcome,
            expected
        );
        assert!(session.halted().is_none());
        let report = session.finish().unwrap();
        assert_eq!(report.halt, None);
        assert_eq!(report.slots[0].check, Some(expected));
        assert_eq!(report.slots[0].state, "sealed");
        assert!(report.slots[1..].iter().all(|s| s.state == "not_started"));
        assert_eq!(fx.paths(), vec!["/v1/messages/count_tokens"]);
        let rows = journal(&e.run_dir);
        let sealed = rows.iter().find(|r| r["kind"] == "slot_sealed").unwrap();
        assert_eq!(sealed["outcome"], json!(expected));
        assert_eq!(
            sealed["receipt_sha256"],
            pilot_capture::sha256_hex(&serde_json::to_vec(&receipt).unwrap())
        );
        assert!(!rows.iter().any(|r| r["kind"] == "halt"));
    }
}

/// Actual changed checker producer only: no CaptureSession, host, model, view
/// materialization or provider. Explicit private paths are supplied by the owner;
/// missing inputs fail rather than skipping or weakening the frozen oracle pin.
#[test]
#[ignore = "needs explicit CF_PILOT_ORACLE_SOURCE and CF_PILOT_POSITIVE_CANDIDATE; actual fixed compiler/oracle on fresh owned copies"]
fn oracle_actual_producer_binds_completion_binary_and_owned_sources() {
    use context_foundry::pilot_capture::apparatus::run_independent_check;
    use std::os::unix::fs::DirBuilderExt;
    use std::os::unix::fs::PermissionsExt;

    const EXPECTED_ORACLE: &str =
        "f5ac14d370d852090c49ead61e23f4d9185eed491b33b123ba6e6ff2dec09bb9";
    // Independently recovered historical success identity, not derived from
    // this execution or read from the new production completion constant.
    const EXPECTED_COMPLETION: &str =
        "aac03c0ded8403eb8bd3ca53e9880313cde11fa658db4fc7444d737eef33127d";
    let oracle = PathBuf::from(
        std::env::var_os("CF_PILOT_ORACLE_SOURCE")
            .expect("CF_PILOT_ORACLE_SOURCE must name the owner-verified pinned oracle"),
    );
    let source =
        PathBuf::from(std::env::var_os("CF_PILOT_POSITIVE_CANDIDATE").expect(
            "CF_PILOT_POSITIVE_CANDIDATE must name a newly owned copied positive candidate",
        ));
    assert!(
        oracle.is_absolute() && source.is_absolute(),
        "explicit absolute inputs required"
    );
    assert!(
        std::fs::symlink_metadata(&oracle).unwrap().is_file(),
        "oracle must be a regular file"
    );
    assert!(
        std::fs::symlink_metadata(&source).unwrap().is_dir(),
        "candidate must be a real directory"
    );
    assert!(
        std::fs::symlink_metadata(source.join("src"))
            .unwrap()
            .is_dir(),
        "candidate src must be a real directory"
    );
    let oracle = std::fs::canonicalize(oracle).unwrap();
    let source = std::fs::canonicalize(source).unwrap();
    let oracle_before = std::fs::read(&oracle).unwrap();
    assert_eq!(pilot_capture::sha256_hex(&oracle_before), EXPECTED_ORACLE);
    assert_eq!(policy::ORACLE_SHA256, EXPECTED_ORACLE);
    let source_before: Vec<(String, Vec<u8>)> = policy::READ_SET
        .iter()
        .map(|rel| {
            let path = source.join(rel);
            assert!(
                std::fs::symlink_metadata(&path).unwrap().is_file(),
                "nonregular candidate source: {rel}"
            );
            ((*rel).to_string(), std::fs::read(path).unwrap())
        })
        .collect();
    let source_identity: Vec<(String, String)> = source_before
        .iter()
        .map(|(rel, bytes)| (rel.clone(), pilot_capture::sha256_hex(bytes)))
        .collect();

    // Retain this new private root on red, green or panic, so the owner can
    // inspect complete producer outputs. Only this owned root is written.
    let owned_temp = tempfile::Builder::new()
        .prefix("cf-oracle-producer-")
        .tempdir()
        .unwrap();
    std::fs::set_permissions(owned_temp.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        std::fs::metadata(owned_temp.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o700
    );
    let owned = std::fs::canonicalize(owned_temp.path()).unwrap();
    std::mem::forget(owned_temp);
    eprintln!("owned oracle producer evidence: {}", owned.display());
    private_fixture_file(&owned.join("input-identities.json"), &serde_json::to_vec_pretty(&json!({
        "oracle_sha256":EXPECTED_ORACLE, "positive_candidate":source_identity,
        "completion_stdout_sha256":EXPECTED_COMPLETION,
        "completion_bytes":106, "completion_bytes_source":"historical owner-verified pin, not an observed receipt field"
    })).unwrap());
    let copy_candidate = |name: &str, fail_compile: bool| {
        let target = owned.join(name);
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&target)
            .unwrap();
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(target.join("src"))
            .unwrap();
        for (rel, bytes) in &source_before {
            let mut bytes = bytes.clone();
            if fail_compile && rel == "src/lib.rs" {
                bytes.extend_from_slice(
                    b"\ncompile_error!(\"owned checker-producer regression\");\n",
                );
            }
            private_fixture_file(&target.join(rel), &bytes);
        }
        target
    };
    let inventory = |workspace: &Path| -> Vec<(String, String)> {
        policy::READ_SET
            .iter()
            .map(|rel| {
                (
                    (*rel).to_string(),
                    pilot_capture::sha256_hex(&std::fs::read(workspace.join(rel)).unwrap()),
                )
            })
            .collect()
    };
    let unchanged_inputs = || {
        assert!(
            std::fs::read(&oracle).unwrap() == oracle_before,
            "original oracle bytes changed; raw private bytes withheld"
        );
        for (rel, bytes) in &source_before {
            assert!(
                &std::fs::read(source.join(rel)).unwrap() == bytes,
                "input candidate changed: {rel}; raw private bytes withheld"
            );
        }
    };
    let order: Vec<String> = policy::READ_SET
        .iter()
        .map(|rel| (*rel).to_string())
        .collect();
    let positive = copy_candidate("positive-candidate", false);
    let positive_before = inventory(&positive);
    assert_eq!(positive_before, source_identity);
    let positive_checker = owned.join("positive-checker");
    assert!(!positive_checker.exists());
    let positive_receipt =
        run_independent_check("1B", &positive, &order, &oracle, &positive_checker).unwrap();
    private_fixture_file(
        &owned.join("positive-receipt.json"),
        &serde_json::to_vec_pretty(&positive_receipt).unwrap(),
    );
    // Check input identity before the provenance/diagnostic assertions, including
    // on a deliberately disabled-guard or producer-observation control.
    unchanged_inputs();
    assert_eq!(inventory(&positive), positive_before);
    assert_eq!(inventory(&positive_checker), positive_before);
    assert_eq!(positive_receipt.candidate, positive_before);
    assert_eq!(
        positive_receipt.checker_root,
        std::fs::canonicalize(&positive_checker).unwrap()
    );
    assert_eq!(positive_receipt.oracle_sha256, EXPECTED_ORACLE);
    assert_eq!(
        pilot_capture::sha256_hex(&std::fs::read(positive_checker.join("check.rs")).unwrap()),
        EXPECTED_ORACLE
    );
    assert_eq!(positive_receipt.compiler, Path::new(policy::RUSTC));
    assert_eq!(positive_receipt.compiler_sha256, policy::RUSTC_SHA256);
    assert_eq!(
        pilot_capture::sha256_hex(&std::fs::read(policy::RUSTC).unwrap()),
        policy::RUSTC_SHA256
    );
    checker_assert_diagnostics(&positive_receipt);
    assert!(!positive_receipt.fixture);
    assert_eq!(positive_receipt.steps.len(), 4);
    let checker_text = positive_checker.to_str().unwrap();
    for (step, (name, argv)) in positive_receipt.steps.iter().zip(policy::CHECK_RECIPE) {
        assert_eq!(step.step, name);
        assert_eq!(
            step.argv,
            argv.iter()
                .map(|arg| arg.replace("<ROOT>", checker_text))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            step.exit,
            Some(0),
            "actual checker step failed: {}",
            step.step
        );
        assert_eq!(step.signal, None);
        assert!(!step.timed_out);
    }
    assert_eq!(positive_receipt.steps[3].stdout_sha256, EXPECTED_COMPLETION);
    // Preserve the accepted actual pre-execution observation and its binding
    // to the compile output while extending diagnostic/provenance coverage.
    let executed = &positive_receipt.steps[3].artifacts;
    assert_eq!(
        executed.len(),
        1,
        "oracle_run must bind its actual pre-execution out/check image"
    );
    assert_eq!(executed[0].0, "out/check");
    let expected_artifacts = [
        "out/librecords-noalloc.rlib",
        "out/librecords-alloc.rlib",
        "out/check",
        "out/check",
    ];
    for (step, name) in positive_receipt.steps.iter().zip(expected_artifacts) {
        let actual_digest =
            pilot_capture::sha256_hex(&std::fs::read(positive_checker.join(name)).unwrap());
        assert_eq!(
            step.artifacts,
            vec![(name.to_string(), actual_digest)],
            "{} artifact differs",
            step.step
        );
    }
    assert_eq!(
        positive_receipt.steps[2].artifacts,
        positive_receipt.steps[3].artifacts
    );

    // A distinct new owned candidate fails during compilation, without
    // executing the oracle or rerunning the historical seeded-negative control.
    let negative = copy_candidate("compile-error-candidate", true);
    let negative_before = inventory(&negative);
    assert_ne!(negative_before, positive_before);
    let negative_checker = owned.join("compile-error-checker");
    assert!(!negative_checker.exists());
    let negative_receipt =
        run_independent_check("1B", &negative, &order, &oracle, &negative_checker).unwrap();
    private_fixture_file(
        &owned.join("compile-error-receipt.json"),
        &serde_json::to_vec_pretty(&negative_receipt).unwrap(),
    );
    assert_eq!(negative_receipt.candidate, negative_before);
    assert_eq!(inventory(&negative), negative_before);
    assert_eq!(inventory(&negative_checker), negative_before);
    assert_eq!(negative_receipt.oracle_sha256, EXPECTED_ORACLE);
    assert_eq!(
        pilot_capture::sha256_hex(&std::fs::read(negative_checker.join("check.rs")).unwrap()),
        EXPECTED_ORACLE
    );
    checker_assert_diagnostics(&negative_receipt);
    let negative_stderr =
        std::fs::read(negative_checker.join(&negative_receipt.steps[0].stderr_path)).unwrap();
    let marker = b"owned checker-producer regression";
    assert!(
        negative_stderr.windows(marker.len()).any(|w| w == marker),
        "the actual compiler stderr must retain the owned compile_error marker"
    );
    assert!(!negative_receipt.fixture);
    assert_eq!(negative_receipt.steps.len(), 1);
    let failed = &negative_receipt.steps[0];
    assert_eq!(failed.step, "noalloc_compile");
    assert!(failed.exit.is_some_and(|code| code != 0));
    assert_eq!(failed.signal, None);
    assert!(!failed.timed_out);
    assert!(!negative_checker.join("out/check").exists());
    // Ok(receipt) with this observed nonzero exit remains an ordinary producer
    // result. The separate seal_slot tests cover its consumer Fail disposition.
    unchanged_inputs();
    private_fixture_file(
        &owned.join("input-readback.json"),
        &serde_json::to_vec_pretty(&json!({
            "unchanged_input_bytes":true, "positive_candidate":inventory(&source),
            "oracle_sha256":pilot_capture::sha256_hex(&std::fs::read(&oracle).unwrap())
        }))
        .unwrap(),
    );
    assert_owned_private_tree(&positive_checker, Some(Path::new("out/check")));
    assert_owned_private_tree(&negative_checker, Some(Path::new("out/check")));
}

#[test]
fn checker_stderr_tampering_is_terminal_before_another_fixture_request() {
    require_host();
    let e = env();
    let fx = Fixture::start(vec![Reply::Count(30_721)]);
    let (session, capture) = oracle_awaiting_capture(&e, &fx);
    let checker = private_checker_tempdir();
    let root = std::fs::canonicalize(checker.path()).unwrap();
    let receipt = fixture_receipt(&capture, &root);
    checker_assert_diagnostics(&receipt);
    private_fixture_file(
        &root.join(&receipt.steps[0].stderr_path),
        b"owned changed diagnostic",
    );
    oracle_assert_terminal_refusal(session, &capture, &receipt, &e, &fx, "check_stderr_unbound");
}

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

#[test]
fn privacy_named_admission_documents_refuse_before_claim_without_normalizing_inputs() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    require_host();
    for case in [
        "authority",
        "manifest",
        "ratecard",
        "view",
        "container",
        "manifest-symlink",
    ] {
        eprintln!("owned private admission case: {case}");
        let e = env();
        let files = AuthorityFiles::new(&e, "private-admission", Scope::Probe, &e.run_dir);
        let path = files.root.join(match case {
            "authority" => "authority.json",
            "manifest" | "manifest-symlink" => "manifest.json",
            "ratecard" => "ratecard.json",
            "view" => "view-1.txt",
            "container" => "authority.json",
            _ => unreachable!(),
        });
        let before = std::fs::read(&path).unwrap();
        if case == "container" {
            std::fs::set_permissions(&files.root, std::fs::Permissions::from_mode(0o750)).unwrap();
        } else if case == "manifest-symlink" {
            let target = files.root.join("owned-manifest-target");
            std::fs::rename(&path, &target).unwrap();
            symlink(target, &path).unwrap();
        } else {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        }
        let fault = CaptureSession::open(files.admission(&e.run_dir, None))
            .err()
            .unwrap();
        assert_eq!(
            (fault.class, fault.code),
            (FaultClass::Privacy, "private_artifact_not_private")
        );
        files.assert_unclaimed();
        assert!(!e.run_dir.exists());
        assert!(!files.root.join("synthetic-key").exists());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        if case == "container" {
            assert_eq!(
                std::fs::metadata(&files.root).unwrap().mode() & 0o7777,
                0o750
            );
        } else if case == "manifest-symlink" {
            assert!(
                std::fs::symlink_metadata(path)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
        } else {
            assert_eq!(std::fs::metadata(path).unwrap().mode() & 0o7777, 0o640);
        }
    }
}

#[test]
fn privacy_prelaunch_refusal_consumes_no_fixture_http_or_permit() {
    use std::os::unix::fs::PermissionsExt;
    require_host();
    let e = env();
    let fx = Fixture::start(vec![]);
    let mut session =
        CaptureSession::open(verification(&e, &fx, Scope::Coding, Default::default())).unwrap();
    let file = e.workspace.join("src/lib.rs");
    let before = std::fs::read(&file).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o640)).unwrap();
    let fault = session.run_slot(slot(&e, "1B", TASK)).unwrap_err();
    assert_eq!(
        (fault.class, fault.code),
        (FaultClass::Privacy, "private_artifact_not_private")
    );
    assert_eq!(session.halted(), Some(fault));
    assert_eq!(session.counters(), pilot_capture::Counters::default());
    assert!(fx.seen().is_empty());
    assert_eq!(std::fs::read(&file).unwrap(), before);
    let report = session.finish().unwrap();
    assert_eq!(report.counters, pilot_capture::Counters::default());
    let rows = journal(&e.run_dir);
    assert_eq!(rows.iter().filter(|r| r["kind"] == "halt").count(), 1);
    assert!(!rows.iter().any(|r| r["kind"] == "http_begin"
        || r["kind"] == "reservation"
        || r["kind"] == "host_launched"));
    assert_eq!(rows.last().unwrap()["kind"], "finished");
}

#[test]
fn privacy_send_gates_preserve_observed_http_and_refuse_the_next_send() {
    for after_count in [true, false] {
        eprintln!(
            "owned privacy send-gate case: {}",
            if after_count {
                "after-count"
            } else {
                "before-followup-count"
            }
        );
        require_host();
        let e = env();
        let changed = e.workspace.join("src/lib.rs");
        let before = std::fs::read(&changed).unwrap();
        let mutate = |reply| Reply::ModeThen {
            path: changed.clone(),
            mode: 0o640,
            reply: Box::new(reply),
        };
        let fx = Fixture::start(if after_count {
            vec![
                mutate(Reply::Count(3000)),
                fixture::text_reply("msg_owned_privacy_unexpected_message", "Done."),
            ]
        } else {
            vec![
                Reply::Count(3000),
                mutate(fixture::tool_reply(
                    "msg_owned_privacy_tool",
                    "toolu_owned_privacy_read",
                    "read",
                    json!({"path":"src/lib.rs", "i":"inspect"}),
                )),
                Reply::Count(3400),
                fixture::text_reply("msg_owned_privacy_unexpected_followup", "Done."),
            ]
        });
        let mut session =
            CaptureSession::open(verification(&e, &fx, Scope::Coding, Default::default())).unwrap();
        let capture = session.run_slot(slot(&e, "1B", TASK)).unwrap();
        let fault = session.halted().expect("privacy refusal must be terminal");
        assert_eq!(
            (fault.class, fault.code),
            (FaultClass::Privacy, "private_artifact_not_private")
        );
        assert!(matches!(capture.disposition, Disposition::Halted { .. }));
        assert!(
            capture.native.is_none(),
            "unsafe native readback must not be accepted after the privacy failure"
        );
        assert!(
            capture.candidate_inventory.is_empty(),
            "unsafe candidate inventory must not be published"
        );
        assert!(capture.host_exit.as_ref().unwrap().group_gone);
        assert_eq!(capture.generations.len(), 1);
        let generation = &capture.generations[0];
        assert_eq!(generation.count_status, Some(200));
        assert_eq!(generation.estimate_lexeme.as_deref(), Some("3000"));
        let counters = session.counters();
        assert_eq!(counters.count_calls, 1);
        assert_eq!(
            counters.generation_permits, 1,
            "an unsent followup must not consume a permit"
        );
        assert_eq!(counters.outstanding_micro_usd, 0);
        if after_count {
            assert_eq!(fx.paths(), ["/v1/messages/count_tokens"]);
            assert_eq!(counters.messages_attempts, 0);
            assert_eq!(counters.provider_http_attempts, 1);
            assert_eq!(counters.spent_micro_usd, 0);
            assert_eq!(generation.messages_attempt, None);
            assert_eq!(generation.outcome, "privacy_before_messages");
        } else {
            assert_eq!(fx.paths(), ["/v1/messages/count_tokens", "/v1/messages"]);
            assert_eq!(counters.messages_attempts, 1);
            assert_eq!(counters.provider_http_attempts, 2);
            assert_eq!(
                generation.message_id.as_deref(),
                Some("msg_owned_privacy_tool")
            );
            assert!(generation.usage.as_ref().unwrap().complete);
            assert_eq!(counters.spent_micro_usd, generation.cost_micro_usd.unwrap());
            assert!(counters.spent_micro_usd > 0);
        }
        assert_eq!(std::fs::read(&changed).unwrap(), before);
        let observed_requests = fx.seen().len();
        assert_eq!(
            session.run_slot(slot(&e, "1A", TASK)).unwrap_err().code,
            "session_halted"
        );
        let report = session.finish().unwrap();
        assert_eq!(report.counters, counters);
        assert_eq!(report.slots[0].state, "closed");
        assert_eq!(
            report.slots[0].disposition.as_ref(),
            Some(&capture.disposition)
        );
        assert!(report.slots[1..].iter().all(|s| s.state == "not_started"));
        assert_eq!(fx.seen().len(), observed_requests);
        let rows = journal(&e.run_dir);
        assert_eq!(rows.iter().filter(|r| r["kind"] == "halt").count(), 1);
        assert_eq!(
            rows.iter().filter(|r| r["kind"] == "http_begin").count(),
            observed_requests
        );
        assert_eq!(
            rows.iter().filter(|r| r["kind"] == "reservation").count(),
            1
        );
        let closed: Vec<_> = rows.iter().filter(|r| r["kind"] == "slot_closed").collect();
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0]["generations"], 1);
        assert_eq!(
            closed[0]["disposition"],
            serde_json::to_value(&capture.disposition).unwrap()
        );
        assert_eq!(
            closed[0]["counters"],
            serde_json::to_value(counters).unwrap()
        );
        assert_eq!(rows.last().unwrap()["kind"], "finished");
    }
}

// Actual successful run roots only. Workspaces and checker roots continue
// through the ordinary helper and receive no host-native exception.
fn assert_owned_private_run_tree(run: &Path) {
    const NATIVE: &str = "home/.omp/natives/18.13.2/pi_natives.darwin-arm64.node";
    let hosts = std::fs::read_dir(run.join("hosts"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(
        !hosts.is_empty(),
        "successful actual host flow must retain its host root"
    );
    for entry in hosts {
        let metadata = std::fs::symlink_metadata(entry.path().join(NATIVE)).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    }
    assert_owned_private_tree_in_run(run, None, Some(run));
}

fn assert_owned_private_tree(root: &Path, executable: Option<&Path>) {
    assert_owned_private_tree_in_run(root, executable, None);
}

fn assert_owned_private_tree_in_run(root: &Path, executable: Option<&Path>, run: Option<&Path>) {
    if let Some(run) = run {
        assert_eq!(root, run, "native exception requires the actual run root");
    }
    use std::os::unix::fs::MetadataExt;
    let mut pending = vec![root.to_path_buf()];
    let mut seen = 0;
    while let Some(path) = pending.pop() {
        seen += 1;
        assert!(seen <= 16_384, "owned test output tree exceeded its bound");
        let meta = std::fs::symlink_metadata(&path).unwrap();
        assert_eq!(meta.uid(), unsafe { libc::getuid() });
        assert!(
            !meta.file_type().is_symlink(),
            "owned output contains symlink"
        );
        if meta.is_dir() {
            assert_eq!(meta.mode() & 0o7777, 0o700, "owned directory mode");
            pending.extend(std::fs::read_dir(path).unwrap().map(|e| e.unwrap().path()));
        } else {
            assert!(meta.is_file(), "owned output is not a regular file");
            let native = run.is_some_and(|run| {
                let hosts = run.join("hosts");
                let Ok(relative) = path.strip_prefix(&hosts) else {
                    return false;
                };
                let mut components = relative.components();
                matches!(components.next(), Some(std::path::Component::Normal(_)))
                    && components.as_path()
                        == Path::new("home/.omp/natives/18.13.2/pi_natives.darwin-arm64.node")
            });
            let expected = if native || executable.is_some_and(|rel| path == root.join(rel)) {
                0o700
            } else {
                0o600
            };
            assert_eq!(meta.mode() & 0o7777, expected, "owned file mode");
        }
    }
}
