//! The shared packer/renderer seam at its FINAL boundary: every trial, cap and
//! hint is computed on the exact emitted bytes, not on application JSON.
use context_foundry::response::{self, BYTE_CAP, count_tokens};
use context_foundry::testkit::{mcp_error, mcp_success, new_fixture};
use context_foundry::{Control, FoundryError, PartialIndexCounts, SourceHandle, Strategy};

fn escaped_source() -> String {
    // Backslashes, quotes, newlines and multibyte text all grow when the MCP
    // text block escapes the application JSON a second time.
    "let text = \"quoted \\\\ back\\\\slash \\\"nested\\\" 東京\";\n".repeat(40)
}

fn minimum_of(error: FoundryError) -> usize {
    match error {
        FoundryError::BudgetTooSmall { minimum_tokens } => minimum_tokens,
        other => panic!("expected budget_too_small, got {other:?}"),
    }
}

#[test]
fn context_packing_counts_the_escaped_final_result_for_every_trial() {
    let mut fx = new_fixture();
    fx.add(&[("esc.rs", &escaped_source())]);
    let mut discriminated = false;
    for budget in [700usize, 1024, 2048, 4096] {
        let outcome = fx
            .engine
            .context("nested", budget, Strategy::Search, &Control::unbounded())
            .unwrap();
        let packed =
            response::pack_context_application(&outcome, None, &mcp_success, BYTE_CAP).unwrap();
        // Emitted bytes ARE the final serialization, and they are what is counted.
        assert_eq!(packed.emitted, mcp_success(&packed.application_json));
        assert_eq!(packed.tokens, count_tokens(&packed.emitted));
        assert!(
            packed.tokens <= budget,
            "budget {budget}: {}",
            packed.tokens
        );
        assert!(packed.emitted.len() <= BYTE_CAP);
        // The same content packed against the application text alone would
        // overshoot once wrapped: proof the final boundary is what decides.
        let naive = response::pack_context_application(
            &outcome,
            None,
            &|application| application.to_owned(),
            BYTE_CAP,
        )
        .unwrap();
        if count_tokens(&mcp_success(&naive.application_json)) > budget {
            discriminated = true;
        }
    }
    assert!(
        discriminated,
        "no budget distinguished final-boundary packing"
    );
}

#[test]
fn adapter_metadata_is_counted_inside_the_envelope() {
    let mut fx = new_fixture();
    fx.add(&[("a.rs", "fn meta_probe() {}\n")]);
    let outcome = fx
        .engine
        .context("meta_probe", 4096, Strategy::Search, &Control::unbounded())
        .unwrap();
    let extra = serde_json::json!({"context_id": "11111111-2222-3333-4444-555555555555"});
    let packed =
        response::pack_context_application(&outcome, Some(&extra), &mcp_success, BYTE_CAP).unwrap();
    let value: serde_json::Value = serde_json::from_str(&packed.application_json).unwrap();
    assert_eq!(value["context_id"], "11111111-2222-3333-4444-555555555555");
    assert_eq!(packed.tokens, count_tokens(&packed.emitted));
    // Adapter metadata reduces what fits: a budget that fits the bare result
    // by exactly its size no longer fits with the extra fields.
    let bare = response::pack_context_application(&outcome, None, &mcp_success, BYTE_CAP).unwrap();
    assert!(packed.tokens > bare.tokens);
}

#[test]
fn budget_too_small_hint_is_sufficient_at_the_final_boundary() {
    let mut fx = new_fixture();
    fx.add(&[("esc.rs", &escaped_source())]);
    for budget in [1usize, 8, 32] {
        let outcome = fx
            .engine
            .context("nested", budget, Strategy::Search, &Control::unbounded())
            .unwrap();
        let minimum = minimum_of(
            response::pack_context_application(&outcome, None, &mcp_success, BYTE_CAP).unwrap_err(),
        );
        assert!(minimum > budget);
        // The hint is a budget that succeeds: no hint that fails when used.
        let retry = fx
            .engine
            .context("nested", minimum, Strategy::Search, &Control::unbounded())
            .unwrap();
        let packed =
            response::pack_context_application(&retry, None, &mcp_success, BYTE_CAP).unwrap();
        assert!(packed.tokens <= minimum);
        // And the CLI boundary reports its own sufficient hint.
        let cli_minimum = minimum_of(response::pack_context_cli(&outcome).unwrap_err());
        let cli_retry = fx
            .engine
            .context(
                "nested",
                cli_minimum,
                Strategy::Search,
                &Control::unbounded(),
            )
            .unwrap();
        assert!(response::pack_context_cli(&cli_retry).unwrap().tokens <= cli_minimum);
    }
}

#[test]
fn empty_source_tiny_budget_returns_budget_too_small_without_panicking() {
    let mut fx = new_fixture();
    fx.add(&[("empty.txt", "")]);
    let workspace = fx.engine.workspace_id().unwrap();
    let handle = SourceHandle {
        v: 1,
        workspace_id: workspace,
        path: "empty.txt".into(),
        sha256: context_foundry::digest(b""),
        start: 0,
        end: 0,
    };
    let tiny = fx.engine.retrieve(&handle.to_json(), 1).unwrap();
    let cli_minimum = minimum_of(response::pack_retrieve_cli(&tiny).unwrap_err());
    let app_minimum = minimum_of(
        response::pack_retrieve_application(&tiny, None, &mcp_success, BYTE_CAP).unwrap_err(),
    );
    // The empty-span result succeeds at its own hint, with no continuation.
    let ok_cli = fx.engine.retrieve(&handle.to_json(), cli_minimum).unwrap();
    let packed = response::pack_retrieve_cli(&ok_cli).unwrap();
    assert!(packed.tokens <= cli_minimum && packed.text.contains("next: null"));
    let ok_app = fx.engine.retrieve(&handle.to_json(), app_minimum).unwrap();
    let packed =
        response::pack_retrieve_application(&ok_app, None, &mcp_success, BYTE_CAP).unwrap();
    let value: serde_json::Value = serde_json::from_str(&packed.application_json).unwrap();
    assert_eq!(value["text"], "");
    assert!(value["next"].is_null());
}

#[test]
fn retrieve_packing_is_exact_at_the_final_boundary_and_continues() {
    let mut fx = new_fixture();
    let body = escaped_source();
    fx.add(&[("esc.rs", &body)]);
    let hit = fx.engine.search("nested", 5).unwrap().hits.remove(0);
    let minimum = minimum_of(
        response::pack_retrieve_application(
            &fx.engine.retrieve(&hit.handle.to_json(), 1).unwrap(),
            None,
            &mcp_success,
            BYTE_CAP,
        )
        .unwrap_err(),
    );
    // Retrying at the advertised hint succeeds (the hint is computed on the
    // final rendering, never a separate wrapper reservation).
    let at_hint = fx.engine.retrieve(&hit.handle.to_json(), minimum).unwrap();
    let packed =
        response::pack_retrieve_application(&at_hint, None, &mcp_success, BYTE_CAP).unwrap();
    assert!(
        packed.tokens <= minimum,
        "{} > hint {minimum}",
        packed.tokens
    );
    assert_eq!(packed.emitted, mcp_success(&packed.application_json));
    let budget = minimum + 120;
    let mut handle = hit.handle.clone();
    let mut collected = String::new();
    for _ in 0..200 {
        let out = fx.engine.retrieve(&handle.to_json(), budget).unwrap();
        let packed =
            response::pack_retrieve_application(&out, None, &mcp_success, BYTE_CAP).unwrap();
        assert_eq!(packed.emitted, mcp_success(&packed.application_json));
        assert_eq!(packed.tokens, count_tokens(&packed.emitted));
        assert!(packed.tokens <= budget);
        let value: serde_json::Value = serde_json::from_str(&packed.application_json).unwrap();
        let text = value["text"].as_str().unwrap();
        assert!(!text.is_empty());
        collected.push_str(text);
        if value["next"].is_null() {
            break;
        }
        let next: SourceHandle = serde_json::from_value(value["next"].clone()).unwrap();
        assert_eq!(next.start, handle.start + text.len() as u64);
        handle = next;
    }
    assert_eq!(
        collected.as_bytes(),
        body.as_bytes()[hit.handle.start as usize..hit.handle.end as usize].to_vec()
    );
}

#[test]
fn error_cap_applies_to_the_final_rendering_and_always_terminates() {
    // M1: a 600-byte diagnostic once looped forever at 515 bytes.
    let long = FoundryError::UnsupportedMode("x".repeat(600));
    let plain = long.bounded_json();
    assert!(plain.len() <= 1024);
    let value: serde_json::Value = serde_json::from_str(&plain).unwrap();
    assert_eq!(value["code"], "unsupported_mode");
    assert_eq!(value["retryable"], false);

    // C2: 450 backslashes fit 1024 bytes as application JSON but not once the
    // tool-result wrapper escapes them a second time.
    let heavy = FoundryError::InvalidArgument("\\".repeat(450));
    assert!(heavy.bounded_json().len() <= 1024);
    let wrapped = heavy.bounded_rendered(&mcp_error);
    assert!(wrapped.len() <= 1024, "{} bytes", wrapped.len());
    let outer: serde_json::Value = serde_json::from_str(&wrapped).unwrap();
    assert_eq!(outer["isError"], true);
    let inner: serde_json::Value =
        serde_json::from_str(outer["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(inner["code"], "invalid_argument");

    // Multi-byte text never splits a codepoint, and partial counts survive.
    let wide = FoundryError::CorruptStore("東京".repeat(400));
    assert!(wide.bounded_rendered(&mcp_error).len() <= 1024);
    let counts = PartialIndexCounts {
        changed: u64::MAX,
        unchanged: u64::MAX,
        deleted: u64::MAX,
        excluded: u64::MAX,
        failed: u64::MAX,
        pending_sources: u64::MAX,
        scan_complete: false,
        deletions_deferred: true,
    };
    for error in [
        FoundryError::Cancelled(Some(counts.clone())),
        FoundryError::DeadlineExceeded(Some(counts.clone())),
        FoundryError::IndexIncomplete(counts.clone()),
    ] {
        let wrapped = error.bounded_rendered(&mcp_error);
        assert!(
            wrapped.len() <= 1024,
            "{} bytes for {}",
            wrapped.len(),
            error.code()
        );
        let outer: serde_json::Value = serde_json::from_str(&wrapped).unwrap();
        let inner: serde_json::Value =
            serde_json::from_str(outer["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(inner["partial"]["changed"], u64::MAX);
        assert!(inner["message"].as_str().unwrap().len() <= 256);
    }
}

#[test]
fn cli_context_cites_line_numbers_and_renders_named_limits() {
    let mut fx = new_fixture();
    let multi: String = (1..=120)
        .map(|i| format!("line {i} marker_cite payload\n"))
        .collect();
    fx.add(&[("one.rs", "fn one_liner_cite() {}\n"), ("many.rs", &multi)]);
    let outcome = fx
        .engine
        .context(
            "one_liner_cite",
            2048,
            Strategy::Search,
            &Control::unbounded(),
        )
        .unwrap();
    let text = response::pack_context_cli(&outcome).unwrap().text;
    // One-line file: a one-based inclusive LINE citation, not byte offsets.
    assert!(text.contains("one.rs:1-1 [sha256:"), "{text}");
    assert!(text.contains("bytes 0-23]"));
    // Every cited line range of a multi-chunk file names the real lines.
    let outcome = fx
        .engine
        .context("marker_cite", 8192, Strategy::Search, &Control::unbounded())
        .unwrap();
    let text = response::pack_context_cli(&outcome).unwrap().text;
    let lines: Vec<&str> = multi.lines().collect();
    let mut checked = 0;
    for citation in text.lines().filter(|l| l.starts_with("many.rs:")) {
        let range = citation["many.rs:".len()..].split(' ').next().unwrap();
        let (first, last) = range.split_once('-').unwrap();
        let (first, last): (usize, usize) = (first.parse().unwrap(), last.parse().unwrap());
        assert!(
            first >= 1 && last >= first && last <= lines.len(),
            "{citation}"
        );
        let body_start = text.find(citation).unwrap() + citation.len() + 1;
        assert!(
            text[body_start..].starts_with(&format!("line {first} ")),
            "{citation}"
        );
        checked += 1;
    }
    assert!(
        checked >= 2,
        "expected several chunk citations, saw {checked}"
    );
    // Named degradation and limits sit inside the counted envelope.
    for needle in [
        "stale_candidates: 0",
        "candidate_limit: 256",
        "candidate_limit_reached: false",
        "search_truncated:",
        "graph: not_requested",
        "omitted_candidates:",
    ] {
        assert!(text.contains(needle), "missing {needle}: {text}");
    }
    let graph = fx
        .engine
        .context(
            "one_liner_cite",
            2048,
            Strategy::Graph,
            &Control::unbounded(),
        )
        .unwrap();
    assert!(
        response::pack_context_cli(&graph)
            .unwrap()
            .text
            .contains("graph: graph_unavailable")
    );
}

#[test]
fn retrieve_prefix_respects_the_128kib_cap_and_two_mib_source_limit() {
    let fx = new_fixture();
    let word = "internationalization ";
    let body = word.repeat((2 * 1024 * 1024) / word.len());
    let body = format!("{body}{}", "x".repeat(2 * 1024 * 1024 - body.len()));
    assert_eq!(body.len(), 2 * 1024 * 1024);
    fx.engine.replace_source("big.txt", &body).unwrap();
    let too_big = format!("{body}y");
    let err = fx
        .engine
        .replace_source("bigger.txt", &too_big)
        .unwrap_err();
    assert_eq!(err.code(), "invalid_argument");
    let workspace = fx.engine.workspace_id().unwrap();
    let whole = SourceHandle {
        v: 1,
        workspace_id: workspace,
        path: "big.txt".into(),
        sha256: context_foundry::digest(body.as_bytes()),
        start: 0,
        end: body.len() as u64,
    };
    let mut handle = whole.clone();
    for hop in 0..2 {
        let out = fx.engine.retrieve(&handle.to_json(), 32768).unwrap();
        let packed = response::pack_retrieve_cli(&out).unwrap();
        assert!(packed.tokens <= 32768);
        let (meta, delivered) = packed.text.split_once("---\n").unwrap();
        assert!(
            delivered.len() <= 128 * 1024,
            "hop {hop}: {}",
            delivered.len()
        );
        assert!(!delivered.is_empty());
        let next = meta.lines().find_map(|l| l.strip_prefix("next: ")).unwrap();
        let next = SourceHandle::from_json(next).unwrap();
        assert_eq!(next.start, handle.start + delivered.len() as u64);
        assert_eq!(next.end, whole.end);
        assert_eq!(
            delivered.as_bytes(),
            &body.as_bytes()[handle.start as usize..next.start as usize]
        );
        handle = next;
    }
}
