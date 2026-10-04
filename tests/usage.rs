//! 003 T005 § Usage import: `foundry usage import` and
//! `usage::import_session` against synthetic host session records.
//!
//! Expected values are frozen independently of the implementation: token
//! totals and byte counts are hand-computed from the fixture bytes, and the
//! four o200k estimates were measured with the reference Python `tiktoken`
//! `o200k_base` encoder over the exact result strings. No expectation is
//! derived from the code under test.
//!
//! NOT RUN during adapter implementation (mid-flight builds/tests are
//! forbidden); the captain's single post-settlement run executes them.

use std::path::{Path, PathBuf};

use context_foundry::adapter_error::AdapterError;
use context_foundry::usage::{UsageHost, import_session};

const BIN: &str = env!("CARGO_BIN_EXE_foundry");
/// A unique string planted in fixture content (tool arguments, user text and
/// tool results). Counters-only output must never contain it.
const MARKER: &str = "Zq7vXk2MwPt9";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/usage")
        .join(name)
}

fn foundry(store: &Path, args: &[&str]) -> std::process::Output {
    std::process::Command::new(BIN)
        .arg("--store")
        .arg(store)
        .args(args)
        .output()
        .unwrap()
}

/// The `{code,message,retryable}` JSON of the last stderr line.
fn error_json(out: &std::process::Output) -> serde_json::Value {
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(stderr.len() <= 1024, "error output exceeds 1024 bytes");
    let line = stderr.lines().last().unwrap_or_default();
    serde_json::from_str(line).unwrap_or_else(|_| panic!("not JSON: {line}"))
}

fn expect_exit(store: &Path, args: &[&str], code: i32) -> std::process::Output {
    let out = foundry(store, args);
    assert_eq!(
        out.status.code(),
        Some(code),
        "command {args:?} exit {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn code_of(error: &AdapterError) -> &str {
    error.code()
}

fn sha256_hex(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(std::fs::read(path).unwrap());
    format!("{:x}", hasher.finalize())
}

#[test]
fn omp_synthetic_session_imports_exact_normalized_totals() {
    let path = fixture("omp-session.jsonl");
    let summary = import_session(UsageHost::Omp, &path).unwrap();

    // Scalars: 7 assistant messages (a1, a2, a3, a4 usage-free, a5's direct
    // Foundry call, a6/a7's blocked and repeated grep; the user line, the
    // toolResults and the malformed a8 never count), 3 usage records (a4, a5,
    // a6, a7 carry no usage), the identical a1 repeat deduplicated.
    assert_eq!(summary.v, 1);
    assert_eq!(summary.host, "omp");
    assert_eq!(
        summary.host_version, None,
        "OMP sessions record no host version"
    );
    assert_eq!(summary.recipe, "omp-v1");
    assert_eq!(summary.session_sha256, sha256_hex(&path));
    assert_eq!(summary.models, ["fixture/model-a", "fixture/model-b"]);
    assert_eq!(summary.assistant_messages, 7);

    // Hand-computed from the fixture usage objects: per record
    // input = input + cacheRead + cacheWrite; subsets never re-added.
    // a1: 1000+300+50 / a2: 500+0+10 / a3: 700+120+0.
    let usage = &summary.provider_usage;
    assert_eq!(usage.input_tokens, 2680);
    assert_eq!(usage.cached_input_tokens, 420);
    assert_eq!(usage.cache_write_tokens, 60);
    assert_eq!(usage.output_tokens, 370);
    assert_eq!(usage.reasoning_tokens, 50);
    assert_eq!(usage.total_tokens, 2680 + 370);

    // Only a2 lacks reasoningTokens.
    assert_eq!(
        serde_json::to_value(&summary.missing).unwrap(),
        serde_json::json!({"reasoning_tokens": 1})
    );
    assert!(!summary.complete);
    // The xd:// Foundry device write classifies once as foundry.search; the
    // ordinary write stays write; the DIRECT mcp__context_foundry_search
    // call keeps its own name; the hook-blocked grep and its repeated
    // attempt both attribute to grep. Result texts: "foundry search ok
    // Zq7vXk2MwPt9" (30 bytes, 14 tokens), "wrote /tmp/plain.rs" (19/6),
    // "grep ran" (8/2), measured with the reference Python tiktoken
    // o200k_base encoder.
    assert_eq!(
        summary.unattributed_results, 1,
        "call_missing joins nothing"
    );
    assert_eq!(
        serde_json::to_value(&summary.tools).unwrap(),
        serde_json::json!({
            "foundry.search": {"calls": 1, "result_bytes": 30, "result_o200k_estimate": 14},
            "grep": {"calls": 2, "result_bytes": 31, "result_o200k_estimate": 6},
            "mcp__context_foundry_search": {
                "calls": 1,
                "result_bytes": 20,
                "result_o200k_estimate": 3
            },
            "write": {"calls": 1, "result_bytes": 19, "result_o200k_estimate": 6},
        })
    );

    // The not-JSON line and the recognized-but-malformed a8 usage; every
    // other non-message line is ignored, not unparsed.
    assert_eq!(summary.unparsed_lines, 2);

    let json = summary.to_json();
    assert!(!json.contains(MARKER), "no content bytes in output");
    assert!(
        !json.contains("first answer"),
        "no assistant text in output"
    );
    assert!(!json.contains("fit_prefix"), "no tool arguments in output");
}

#[test]
fn codex_synthetic_rollout_imports_exact_normalized_totals() {
    let path = fixture("codex-rollout.jsonl");
    let summary = import_session(UsageHost::Codex, &path).unwrap();

    assert_eq!(summary.v, 1);
    assert_eq!(summary.host, "codex");
    assert_eq!(summary.host_version.as_deref(), Some("0.158.0"));
    assert_eq!(summary.recipe, "codex-v1");
    assert_eq!(summary.session_sha256, sha256_hex(&path));
    assert_eq!(summary.models, ["fixture-gpt-x"]);
    assert_eq!(summary.assistant_messages, 2, "msg_a1 and msg_a2");
    assert_eq!(
        summary.usage_records, 1,
        "the last cumulative record, never summed"
    );

    // The LAST token_count with non-null total_token_usage (ordinal 12) wins
    // over ordinal 9, the null ordinal 10 and the malformed ordinal 11; its
    // absent reasoning and explicit-null cache-write categories count in
    // `missing`; input already includes cached.
    let usage = &summary.provider_usage;
    assert_eq!(usage.input_tokens, 3000);
    assert_eq!(usage.cached_input_tokens, 1200);
    assert_eq!(usage.cache_write_tokens, 0);
    assert_eq!(usage.output_tokens, 150);
    assert_eq!(usage.reasoning_tokens, 0);
    assert_eq!(usage.total_tokens, 3000 + 150);

    assert_eq!(
        serde_json::to_value(&summary.missing).unwrap(),
        serde_json::json!({"cache_write_tokens": 1, "reasoning_tokens": 1})
    );
    assert!(!summary.complete);

    // "hi\nZq7vXk2MwPt9" is 15 bytes / 12 tokens; "lookup result bytes" is
    // 19 bytes / 3 tokens (reference Python tiktoken o200k_base).
    assert_eq!(summary.unattributed_results, 1, "call_nobody joins nothing");
    assert_eq!(
        serde_json::to_value(&summary.tools).unwrap(),
        serde_json::json!({
            "exec": {"calls": 1, "result_bytes": 15, "result_o200k_estimate": 12},
            "lookup": {"calls": 1, "result_bytes": 19, "result_o200k_estimate": 3},
        })
    );
    // The not-JSON line and the recognized-but-malformed token_count.
    assert_eq!(summary.unparsed_lines, 2);

    let json = summary.to_json();
    assert!(!json.contains(MARKER), "no content bytes in output");
}

#[test]
fn conflicting_omp_usage_under_one_entry_id_refuses() {
    let path = fixture("omp-conflict.jsonl");
    let error = import_session(UsageHost::Omp, &path).unwrap_err();
    assert_eq!(code_of(&error), "usage_conflict");

    let store = tempfile::tempdir().unwrap();
    let out = expect_exit(
        store.path(),
        &[
            "usage",
            "import",
            "--host",
            "omp",
            "--session",
            path.to_str().unwrap(),
        ],
        1,
    );
    assert_eq!(error_json(&out)["code"], "usage_conflict");
    assert!(
        String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        "a refusal prints no summary; the error is on stderr"
    );
}

#[test]
fn u64_overflow_refuses_with_usage_overflow() {
    let path = fixture("omp-overflow.jsonl");
    let error = import_session(UsageHost::Omp, &path).unwrap_err();
    assert_eq!(code_of(&error), "usage_overflow");

    let store = tempfile::tempdir().unwrap();
    let out = expect_exit(
        store.path(),
        &[
            "usage",
            "import",
            "--host",
            "omp",
            "--session",
            path.to_str().unwrap(),
        ],
        1,
    );
    assert_eq!(error_json(&out)["code"], "usage_overflow");
}

#[test]
fn usage_free_sessions_refuse_with_usage_unavailable() {
    for (host, name) in [
        (UsageHost::Omp, "omp-no-usage.jsonl"),
        (UsageHost::Codex, "codex-no-usage.jsonl"),
    ] {
        let path = fixture(name);
        let error = import_session(host, &path).unwrap_err();
        assert_eq!(code_of(&error), "usage_unavailable", "{name}");

        let store = tempfile::tempdir().unwrap();
        let out = expect_exit(
            store.path(),
            &[
                "usage",
                "import",
                "--host",
                if matches!(host, UsageHost::Omp) {
                    "omp"
                } else {
                    "codex"
                },
                "--session",
                path.to_str().unwrap(),
            ],
            1,
        );
        assert_eq!(error_json(&out)["code"], "usage_unavailable", "{name}");
    }
}

#[test]
fn oversize_line_refuses_with_usage_input_too_large() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("oversize-line.jsonl");
    // One valid line, then one line of 32 MiB + 1 bytes (no newline inside).
    let mut bytes = br#"{"type":"session","version":3,"id":"x"}"#.to_vec();
    bytes.push(b'\n');
    bytes.extend(std::iter::repeat_n(b'a', 32 * 1024 * 1024 + 1));
    bytes.push(b'\n');
    std::fs::write(&path, bytes).unwrap();

    let error = import_session(UsageHost::Omp, &path).unwrap_err();
    assert_eq!(code_of(&error), "usage_input_too_large");

    let store = tempfile::tempdir().unwrap();
    let out = expect_exit(
        store.path(),
        &[
            "usage",
            "import",
            "--host",
            "omp",
            "--session",
            path.to_str().unwrap(),
        ],
        2,
    );
    assert_eq!(error_json(&out)["code"], "usage_input_too_large");
}

#[test]
fn oversize_file_refuses_with_usage_input_too_large() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("oversize-file.jsonl");
    // A sparse 512 MiB + 1 byte file: refused by bound, never read in full.
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(512 * 1024 * 1024 + 1).unwrap();
    drop(file);

    let error = import_session(UsageHost::Omp, &path).unwrap_err();
    assert_eq!(code_of(&error), "usage_input_too_large");

    let store = tempfile::tempdir().unwrap();
    let out = expect_exit(
        store.path(),
        &[
            "usage",
            "import",
            "--host",
            "omp",
            "--session",
            path.to_str().unwrap(),
        ],
        2,
    );
    assert_eq!(error_json(&out)["code"], "usage_input_too_large");
}

#[test]
fn cli_import_prints_one_counter_json_object_per_host() {
    let store = tempfile::tempdir().unwrap();
    for (host, name, records, total) in [
        ("omp", "omp-session.jsonl", 3u64, 3050u64),
        ("codex", "codex-rollout.jsonl", 1, 3150),
    ] {
        let path = fixture(name);
        let out = expect_exit(
            store.path(),
            &[
                "usage",
                "import",
                "--host",
                host,
                "--session",
                path.to_str().unwrap(),
            ],
            0,
        );
        let stdout = String::from_utf8(out.stdout.clone()).unwrap();
        assert_eq!(
            stdout.lines().count(),
            1,
            "{host}: exactly one JSON object on stdout"
        );
        assert!(!stdout.contains(MARKER), "{host}: no content bytes");

        let summary: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
        assert_eq!(summary["v"], 1);
        assert_eq!(summary["host"], host);
        assert_eq!(summary["recipe"], format!("{host}-v1"));
        assert_eq!(summary["usage_records"], records);
        assert_eq!(summary["provider_usage"]["total_tokens"], total);
    }
}

/// One synthetic OMP assistant usage record line with the given counters.
fn omp_usage_line(id: &str, usage: serde_json::Value) -> String {
    serde_json::json!({
        "type": "message", "id": id,
        "message": {"role": "assistant", "content": [], "usage": usage}
    })
    .to_string()
}

fn omp_usage(
    input: u64,
    cache_read: u64,
    cache_write: u64,
    output: u64,
    reasoning: u64,
) -> serde_json::Value {
    serde_json::json!({
        "input": input, "output": output, "cacheRead": cache_read,
        "cacheWrite": cache_write, "totalTokens": 0, "reasoningTokens": reasoning
    })
}

/// Every u64 addition the recipes perform refuses with `usage_overflow`
/// instead of panicking or wrapping: both OMP per-record additions, the
/// aggregate output/reasoning accumulators, and the final input+output total
/// (which is the only one Codex's single cumulative record can reach).
#[test]
fn overflow_refuses_at_every_addition_for_both_recipes() {
    let dir = tempfile::tempdir().unwrap();
    let max = u64::MAX;
    let cases: [(&str, Vec<String>); 5] = [
        (
            "per-record first addition (input + cacheRead)",
            vec![omp_usage_line("a1", omp_usage(max, 1, 0, 0, 0))],
        ),
        (
            "per-record second addition (+ cacheWrite)",
            vec![omp_usage_line("a1", omp_usage(max, 0, 1, 0, 0))],
        ),
        (
            "aggregate output accumulation",
            vec![
                omp_usage_line("a1", omp_usage(1, 0, 0, max, 0)),
                omp_usage_line("a2", omp_usage(1, 0, 0, max, 0)),
            ],
        ),
        (
            "aggregate reasoning accumulation",
            vec![
                omp_usage_line("a1", omp_usage(1, 0, 0, 0, max)),
                omp_usage_line("a2", omp_usage(1, 0, 0, 0, max)),
            ],
        ),
        (
            "final input + output total",
            vec![omp_usage_line("a1", omp_usage(max, 0, 0, 1, 0))],
        ),
    ];
    for (what, lines) in &cases {
        let path = dir.path().join("overflow-case.jsonl");
        std::fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();
        let error = match import_session(UsageHost::Omp, &path) {
            Err(error) => error,
            Ok(summary) => panic!("{what}: expected usage_overflow, got {summary:?}"),
        };
        assert_eq!(code_of(&error), "usage_overflow", "{what}");
    }

    // The first case also refuses through the CLI with exit 1.
    let path = dir.path().join("overflow-cli.jsonl");
    std::fs::write(
        &path,
        format!("{}\n", omp_usage_line("a1", omp_usage(max, 1, 0, 0, 0))),
    )
    .unwrap();
    let store = tempfile::tempdir().unwrap();
    let out = expect_exit(
        store.path(),
        &[
            "usage",
            "import",
            "--host",
            "omp",
            "--session",
            path.to_str().unwrap(),
        ],
        1,
    );
    assert_eq!(error_json(&out)["code"], "usage_overflow");

    // Codex reaches the final input+output addition through its single
    // cumulative record.
    let path = dir.path().join("codex-overflow.jsonl");
    std::fs::write(
        &path,
        format!(
            "{}\n",
            serde_json::json!({
                "type": "event_msg",
                "payload": {"type": "token_count", "info": {"total_token_usage": {
                    "input_tokens": max, "cached_input_tokens": 0,
                    "cache_write_input_tokens": 0, "output_tokens": 1,
                    "reasoning_output_tokens": 0}}}
            })
        ),
    )
    .unwrap();
    let error = import_session(UsageHost::Codex, &path).unwrap_err();
    assert_eq!(code_of(&error), "usage_overflow", "codex final total");
}

/// A conflicting entry id is session-controlled: the refusal stays bounded
/// and fast even for a 1 MiB id, with no summary on stdout.
#[test]
fn conflicting_long_entry_ids_render_a_bounded_fast_refusal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("long-id-conflict.jsonl");
    let long_id = "i".repeat(1024 * 1024);
    let first = omp_usage_line(&long_id, omp_usage(100, 0, 0, 10, 0));
    let second = omp_usage_line(&long_id, omp_usage(101, 0, 0, 10, 0));
    std::fs::write(&path, format!("{first}\n{second}\n")).unwrap();

    let error = import_session(UsageHost::Omp, &path).unwrap_err();
    assert_eq!(code_of(&error), "usage_conflict");
    let rendered = error.bounded_json();
    assert!(
        rendered.len() <= 1024,
        "bounded error: {} bytes",
        rendered.len()
    );
    assert!(
        !rendered.contains(&long_id[..64]),
        "the id is not echoed back"
    );

    let store = tempfile::tempdir().unwrap();
    let started = std::time::Instant::now();
    let out = expect_exit(
        store.path(),
        &[
            "usage",
            "import",
            "--host",
            "omp",
            "--session",
            path.to_str().unwrap(),
        ],
        1,
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(30),
        "the bounded refusal completes quickly"
    );
    assert_eq!(error_json(&out)["code"], "usage_conflict");
    assert!(out.stderr.len() <= 1024, "bounded stderr");
    assert!(
        String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        "no summary on refusal"
    );
}
