//! Offline diagnostics only. Labels never enter ranking, packing or production.
use context_foundry::{
    Control, Engine, SourceHandle, Strategy, response, retrieval_trace::RetrievalTrace,
    store::ContextOptions, testkit,
};
use serde_json::{Value, json};

fn covers(unit: &Value, expected: &SourceHandle) -> bool {
    unit["workspace_id"] == expected.workspace_id
        && unit["path"] == expected.path
        && unit["sha256"] == expected.sha256
        && unit["start"].as_u64().is_some_and(|n| n <= expected.start)
        && unit["end"].as_u64().is_some_and(|n| n >= expected.end)
}

fn attribute(record: &Value, query: &str, expected: &SourceHandle) -> Value {
    let unknown = |reason: &str| json!({"result":"unknown","reason":reason});
    if record["version"] != 1
        || record["operation"] != "context"
        || record["trace"]["truncated"] != false
    {
        return unknown("missing_invalid_or_truncated_trace");
    }
    let Some(events) = record["trace"]["events"].as_array() else {
        return unknown("events_missing");
    };
    let queries: Vec<_> = events.iter().filter(|e| e["stage"] == "query").collect();
    if queries.len() != 1 || queries[0]["query_sha256"] != context_foundry::digest(query.as_bytes())
    {
        return unknown("query_correlation_missing_or_conflicting");
    }
    if record["outcome"]["status"] != "success" {
        return unknown("request_failed");
    }
    let Some(text) = record["outcome"]["text"].as_str() else {
        return unknown("delivery_missing");
    };
    if record["outcome"]["sha256"] != context_foundry::digest(text.as_bytes()) {
        return unknown("delivery_hash_mismatch");
    }
    let Ok(delivered) = testkit::parse_v2(text) else {
        return unknown("delivery_format_invalid");
    };
    let matching: Vec<_> = delivered
        .items
        .iter()
        .filter(|i| {
            context_foundry::store::HandleRef::parse(&i.handle).is_ok_and(|h| {
                expected.workspace_id.starts_with(&h.ws16)
                    && expected.sha256.starts_with(&h.sha32)
                    && expected.path == h.path
                    && h.start <= expected.start
                    && h.end >= expected.end
            })
        })
        .collect();
    if matching
        .iter()
        .any(|i| i.form.is_none() && !i.body.is_empty())
    {
        return json!({"result":"delivered","boundary":"foundry_delivery","host_consumption":"unknown"});
    }
    let packs: Vec<_> = events
        .iter()
        .filter(|e| e["stage"] == "packing_input")
        .collect();
    if packs.len() != 1 {
        return unknown("final_packing_snapshot_missing_or_conflicting");
    }
    let pack = packs[0];
    let in_pack = pack["items"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|i| covers(&i["unit"], expected))
        || pack["anchors"].as_array().into_iter().flatten().any(|a| {
            a["units"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|u| covers(u, expected))
        });
    if in_pack {
        return json!({"result":"loss","stage":if matching.is_empty(){"packing_omission"}else{"representation_reduction"}});
    }
    let seen: Vec<_> = events
        .iter()
        .filter(|e| covers(&e["unit"], expected))
        .collect();
    // A deduplicated copy is not a lost unit. Multiple competing observed
    // causes remain unknown; absence from a bounded window proves no cause.
    let causes: std::collections::BTreeSet<_> = seen
        .iter()
        .filter_map(|e| match e["outcome"].as_str()? {
            "stale" => Some("freshness"),
            "path_filter" => Some("path_filter"),
            "per_file_cap" => Some("per_file_cap"),
            "limit" => Some("candidate_limit"),
            _ => None,
        })
        .collect();
    if causes.len() == 1 && !seen.iter().any(|e| e["outcome"] == "kept") {
        json!({"result":"loss","stage":causes.first().unwrap()})
    } else {
        unknown(if seen.is_empty() {
            "expected_span_not_observed_in_bounded_candidate_window"
        } else {
            "no_unique_observed_loss"
        })
    }
}

fn record(trace: &RetrievalTrace, text: &str) -> Value {
    json!({"version":1,"operation":"context","trace":trace.snapshot(),"outcome":{"status":"success","text":text,"sha256":context_foundry::digest(text.as_bytes())}})
}

#[test]
fn attribution_requires_identity_correlation_and_actual_body_delivery() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let mut engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
    engine
        .replace_source(
            "lib.rs",
            "pub fn retain() { println!(\"preserve bytes\"); }\n",
        )
        .unwrap();
    engine.refresh(&Control::unbounded()).unwrap();
    let query = "retain";
    let trace = RetrievalTrace::default();
    let c = engine
        .context_candidates_with(
            query,
            Strategy::Search,
            &Control::unbounded().with_trace(Some(trace.clone())),
            &ContextOptions::default(),
        )
        .unwrap();
    let expected = c.batch.items[0].handle.clone().unwrap();
    let p = response::pack_enriched_context(
        &c,
        None,
        response::Budget::request(2048),
        &response::stdout_bytes,
    )
    .unwrap();
    let r = record(&trace, &p.text);
    assert_eq!(attribute(&r, query, &expected)["result"], "delivered");
    assert_eq!(
        attribute(&r, "another query", &expected)["result"],
        "unknown"
    );
    let mut bad = r.clone();
    bad["trace"]["truncated"] = json!(true);
    assert_eq!(attribute(&bad, query, &expected)["result"], "unknown");
    let mut bad = r.clone();
    bad["outcome"]["text"] = json!("tampered");
    assert_eq!(
        attribute(&bad, query, &expected)["reason"],
        "delivery_hash_mismatch"
    );
    let mut old = expected.clone();
    old.sha256 = "f".repeat(64);
    assert_eq!(attribute(&r, query, &old)["result"], "unknown");
    let empty = "foundry context · 0 items\n";
    let mut lost = r.clone();
    lost["outcome"] =
        json!({"status":"success","text":empty,"sha256":context_foundry::digest(empty.as_bytes())});
    assert_eq!(
        attribute(&lost, query, &expected)["stage"],
        "packing_omission"
    );
    lost["trace"]["events"]
        .as_array_mut()
        .unwrap()
        .retain(|e| e["stage"] != "packing_input");
    assert_eq!(attribute(&lost, query, &expected)["result"], "unknown");
}

/// Freeze these development labels before executing either arm. They are public
/// repository diagnostics, not held-out G2 questions or task-quality acceptance.
const CASES: &[(&str, &str, &str)] = &[
    (
        "reject route publication when an imported module changes before commit",
        "src/boundary.rs",
        "boundary dependency changed during extraction",
    ),
    (
        "commit available files without waiting for the next disk read",
        "src/ingest.rs",
        "match pending.result.try_recv()",
    ),
    (
        "recall an earlier failed attempt relevant to the changed file",
        "src/task_context.rs",
        "Kind::RejectedAttempt if relevant =>",
    ),
    (
        "mark references inside attributed tests as verification evidence",
        "src/task_evidence.rs",
        "compiler_reference_test_attribute",
    ),
    (
        "restrict which CPUs contribute to the utilization sample",
        "src/model_resources.rs",
        "fn affinity_ticks(",
    ),
    (
        "refuse to overwrite a previous diagnostic run directory",
        "src/retrieval_trace.rs",
        "builder.create(directory)?",
    ),
    (
        "keep CPU-heavy models out of source transactions",
        "src/neural/prepare.rs",
        "pub fn run(",
    ),
    (
        "follow callers of `index_boundaries`",
        "src/boundary.rs",
        "pub fn index_boundaries(",
    ),
];

#[test]
#[ignore = "explicit repository diagnostic; requires new FOUNDRY_ATTRIBUTION_DIR, preserves traces outside Git"]
fn repository_development_attribution() {
    let output = std::path::PathBuf::from(
        std::env::var_os("FOUNDRY_ATTRIBUTION_DIR").expect("new output directory"),
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(!output.starts_with(root));
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&output)
        .unwrap();
    let mut engine = Engine::initialize(&output.join("store"), root).unwrap();
    let deadline =
        || Control::with_deadline(std::time::Instant::now() + std::time::Duration::from_secs(300));
    let indexed = engine.index(root, &deadline()).unwrap();
    // The diagnostic's literal questions/expected snippets are evaluator data.
    // Indexing this file would rank those labels as repository evidence.
    let excluded = "tests/retrieval_attribution.rs";
    engine.delete_source(excluded).unwrap();
    engine.refresh(&deadline()).unwrap();
    let wid = engine.workspace_id().unwrap();
    let frozen: Vec<_> = CASES
        .iter()
        .map(|&(query, path, needle)| {
            let body = std::fs::read_to_string(root.join(path)).unwrap();
            let at = body.find(needle).expect("frozen label exists");
            let start = body[..at].rfind('\n').map_or(0, |i| i + 1);
            let end = body[at..].find('\n').map_or(body.len(), |i| at + i + 1);
            (
                query,
                SourceHandle {
                    workspace_id: wid.clone(),
                    path: path.into(),
                    sha256: context_foundry::digest(body.as_bytes()),
                    start: start as u64,
                    end: end as u64,
                },
            )
        })
        .collect();
    std::fs::write(
        output.join("labels.json"),
        serde_json::to_vec_pretty(&frozen).unwrap(),
    )
    .unwrap();
    let mut rows = Vec::new();
    for (id, (query, expected)) in frozen.iter().enumerate() {
        for behavior in [false, true] {
            let trace = RetrievalTrace::default();
            let started = std::time::Instant::now();
            let c = engine
                .context_candidates_with(
                    query,
                    Strategy::Search,
                    &deadline().with_trace(Some(trace.clone())),
                    &ContextOptions {
                        experimental_behavior: behavior,
                        ..Default::default()
                    },
                )
                .unwrap();
            let packed = response::pack_enriched_context(
                &c,
                None,
                response::Budget::request(2048),
                &response::stdout_bytes,
            )
            .unwrap();
            let elapsed = started.elapsed().as_millis();
            let r = record(&trace, &packed.text);
            let plain = engine
                .context_candidates_with(
                    query,
                    Strategy::Search,
                    &deadline(),
                    &ContextOptions {
                        experimental_behavior: behavior,
                        ..Default::default()
                    },
                )
                .unwrap();
            let plain = response::pack_enriched_context(
                &plain,
                None,
                response::Budget::request(2048),
                &response::stdout_bytes,
            )
            .unwrap();
            assert_eq!(
                plain.text, packed.text,
                "diagnostics cannot change delivery"
            );
            let name = format!(
                "{id}-{}.json",
                if behavior { "behavior" } else { "baseline" }
            );
            std::fs::write(output.join(&name), serde_json::to_vec_pretty(&r).unwrap()).unwrap();
            rows.push(json!({"id":id,"query":query,"expected":expected,"behavior":behavior,"tokens":packed.tokens,"traced_elapsed_ms":elapsed,"record":name,"attribution":attribute(&r,query,expected)}));
        }
    }
    let report = json!({"scope":"repository_development_lexical_diagnostic","historical_g2":false,"excluded_evaluator_sources":[excluded],"indexed":indexed,"budget":2048,"rows":rows});
    std::fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{}", serde_json::to_string(&report).unwrap());
}
