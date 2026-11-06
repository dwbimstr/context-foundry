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

const OMP: &str = "/Users/satishlomte/.local/bin/omp";
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
    let root = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(root.path()).unwrap();
    let ws = base.join("ws");
    std::fs::create_dir_all(ws.join("src")).unwrap();
    std::fs::write(
        ws.join("Cargo.toml"),
        "[package]\nname = \"records\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    std::fs::write(
        ws.join("src/lib.rs"),
        "pub fn decode(input: &[u8]) -> Vec<u8> { input.to_vec() }\n",
    )
    .unwrap();
    std::fs::write(ws.join("src/ascii_set.rs"), "pub struct AsciiSet;\n").unwrap();
    std::fs::write(ws.join("LICENSE-MIT"), "MIT synthetic\n").unwrap();
    std::fs::write(ws.join("LICENSE-APACHE"), "Apache synthetic\n").unwrap();
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

fn fixture_receipt(capture: &pilot_capture::SlotCapture, root: &Path) -> IndependentCheckReceipt {
    let root_s = root.to_str().unwrap();
    let steps = policy::CHECK_RECIPE
        .iter()
        .map(|(name, argv)| CheckStep {
            step: name.to_string(),
            argv: argv.iter().map(|a| a.replace("<ROOT>", root_s)).collect(),
            exit: Some(0),
            signal: None,
            timed_out: false,
            stdout_sha256: pilot_capture::sha256_hex(name.as_bytes()),
            stderr_sha256: pilot_capture::sha256_hex(b""),
            artifacts: vec![],
            wall_ms: 100,
        })
        .collect();
    IndependentCheckReceipt {
        slot_id: capture.slot_id.clone(),
        checker_root: root.to_path_buf(),
        candidate: capture.candidate_inventory.clone(),
        oracle_sha256: policy::ORACLE_SHA256.into(),
        compiler: policy::RUSTC.into(),
        compiler_sha256: policy::RUSTC_SHA256.into(),
        compiler_version: "rustc 1.97.1 (fixture)".into(),
        env_names: vec!["PATH".into(), "HOME".into()],
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
    // OMP 18.13.2 unconditionally prewarms a connection to its initial
    // model's origin (an unauthenticated HEAD through its own transport)
    // while creating the session. The sandbox denies it before name
    // resolution; that is the only provider-named host line allowed.
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
    let checker = tempfile::tempdir().unwrap();
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
    let checker = tempfile::tempdir().unwrap();
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
    let checker = tempfile::tempdir().unwrap();
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
    let tmp = tempfile::tempdir().unwrap();
    let root =
        PilotRoot::create(&std::fs::canonicalize(tmp.path()).unwrap().join("pilot")).unwrap();
    apparatus::install_seed(&root, &verified).unwrap();
    let views = apparatus::materialize_views(&root, &verified).unwrap();
    assert_eq!(views.len(), 4);
    assert!(
        views
            .iter()
            .all(|v| v.header.contains("semantic:ready") && v.delivered_o200k_tokens <= 2048)
    );
    let (probe_manifest, coding_manifest) = apparatus::build_manifests(&root, &verified).unwrap();

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
    let coding = CaptureSession::open_after_probe(
        &seal,
        admission(Scope::Coding, "unused", coding_manifest),
    )
    .unwrap();
    let report = apparatus::run_coding(coding, &root, &verified).unwrap();

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
}
