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

// A lexical unit can be a long function with many indexed documents. Selecting
// one document does not establish that every byte of its enclosing unit reached
// excerpt construction. Dense locations and placement events already name their
// actual selected ranges. Older lexical traces without document ranges cannot
// establish this boundary and remain unknown.
fn selected_covers(event: &Value, expected: &SourceHandle) -> bool {
    if !covers(&event["unit"], expected) {
        return false;
    }
    if event["origin"] == "lexical"
        && matches!(
            event["stage"].as_str(),
            Some("selection" | "materialization")
        )
    {
        return event["document"]["start"]
            .as_u64()
            .is_some_and(|start| start <= expected.start)
            && event["document"]["end"]
                .as_u64()
                .is_some_and(|end| end >= expected.end);
    }
    true
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
        .filter(|i| i.form.as_deref() != Some("outline"))
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
        .filter(|i| i["tier"] != context_foundry::store::TIER_OUTLINE)
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
    let materialized: Vec<_> = events
        .iter()
        .filter(|e| e["stage"] == "materialization" && selected_covers(e, expected))
        .collect();
    if !materialized.is_empty() {
        if materialized.iter().all(|e| {
            let excerpt = &e["excerpt"];
            let (Some(start), Some(end)) = (excerpt["start"].as_u64(), excerpt["end"].as_u64())
            else {
                return false;
            };
            excerpt["workspace_id"] == expected.workspace_id
                && excerpt["path"] == expected.path
                && excerpt["sha256"] == expected.sha256
                && start < end
                && e["unit"]["start"].as_u64().is_some_and(|n| n <= start)
                && e["unit"]["end"].as_u64().is_some_and(|n| n >= end)
                && !covers(excerpt, expected)
        }) {
            return json!({"result":"loss","stage":"evidence_window"});
        }
        return unknown("materialization_and_packing_not_correlated");
    }
    let seen: Vec<_> = events
        .iter()
        .filter(|e| selected_covers(e, expected))
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
fn attribution_distinguishes_excerpt_selection_from_missing_discovery() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let mut engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
    let body = format!(
        "pub fn process() {{\n    retain_expected_records();\n{}    restore_catalog();\n{}    restore_late();\n}}\n",
        "    ordinary_step();\n".repeat(100),
        "    ordinary_step();\n".repeat(400)
    );
    engine.replace_source("lib.rs", &body).unwrap();
    engine.refresh(&Control::unbounded()).unwrap();
    let query = "retain expected records";
    let trace = RetrievalTrace::default();
    let candidates = engine
        .context_candidates_with(
            query,
            Strategy::Search,
            &Control::unbounded().with_trace(Some(trace.clone())),
            &ContextOptions {
                experimental_behavior: true,
                ..Default::default()
            },
        )
        .unwrap();
    let start = body.find("restore_catalog").unwrap();
    let expected = SourceHandle {
        workspace_id: engine.workspace_id().unwrap(),
        path: "lib.rs".into(),
        sha256: context_foundry::digest(body.as_bytes()),
        start: start as u64,
        end: (start + "restore_catalog".len()) as u64,
    };
    let packed = response::pack_enriched_context(
        &candidates,
        None,
        response::Budget::request(2048),
        &response::stdout_bytes,
    )
    .unwrap();
    let r = record(&trace, &packed.text);
    assert_eq!(attribute(&r, query, &expected)["stage"], "evidence_window");
    let start = body.find("restore_late").unwrap() as u64;
    let outside = SourceHandle {
        start,
        end: start + "restore_late".len() as u64,
        ..expected.clone()
    };
    assert_eq!(
        attribute(&r, query, &outside)["reason"],
        "expected_span_not_observed_in_bounded_candidate_window",
        "a selected function is not evidence that its unselected documents were materialized"
    );
    let mut legacy = r.clone();
    for event in legacy["trace"]["events"].as_array_mut().unwrap() {
        if event["stage"] == "materialization" {
            event.as_object_mut().unwrap().remove("document");
        }
    }
    assert_eq!(attribute(&legacy, query, &expected)["result"], "unknown");
    let mut incomplete = r.clone();
    for event in incomplete["trace"]["events"].as_array_mut().unwrap() {
        if event["stage"] == "materialization" {
            event["excerpt"] = Value::Null;
        }
    }
    assert_eq!(
        attribute(&incomplete, query, &expected)["result"],
        "unknown"
    );
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
    let root = std::env::var_os("FOUNDRY_ATTRIBUTION_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| env!("CARGO_MANIFEST_DIR").into());
    assert!(!output.starts_with(env!("CARGO_MANIFEST_DIR")));
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&output)
        .unwrap();
    let store = std::env::var_os("FOUNDRY_ATTRIBUTION_STORE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| output.join("store"));
    let existing = store.exists();
    let mut engine = if existing {
        Engine::open_existing(&store)
    } else {
        Engine::initialize(&store, &root)
    }
    .unwrap();
    let deadline =
        || Control::with_deadline(std::time::Instant::now() + std::time::Duration::from_secs(300));
    let indexed = if existing {
        None
    } else {
        Some(engine.index(&root, &deadline()).unwrap())
    };
    // The diagnostic's literal questions/expected snippets are evaluator data.
    // Indexing this file would rank those labels as repository evidence.
    let excluded = "tests/retrieval_attribution.rs";
    engine.delete_source(excluded).unwrap();
    engine.refresh(&deadline()).unwrap();
    let wid = engine.workspace_id().unwrap();
    let source_revision = engine.status().unwrap().source_revision;
    let external: Option<Value> = std::env::var_os("FOUNDRY_ATTRIBUTION_LABELS")
        .map(|path| serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap());
    let cases: Vec<_> = external.as_ref().map_or_else(
        || CASES.to_vec(),
        |v| {
            v["labels"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    (
                        v["query"].as_str().unwrap(),
                        v["path"].as_str().unwrap(),
                        v["needle"].as_str().unwrap(),
                    )
                })
                .collect()
        },
    );
    let frozen: Vec<_> = cases
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
    if let Some(external) = &external {
        for ((_, expected), label) in frozen.iter().zip(external["labels"].as_array().unwrap()) {
            assert_eq!(label["sha256"], expected.sha256, "frozen corpus changed");
            assert_eq!(label["start"], expected.start, "frozen label moved");
            assert_eq!(label["end"], expected.end, "frozen label moved");
        }
    }
    std::fs::write(
        output.join("labels.json"),
        serde_json::to_vec_pretty(&frozen).unwrap(),
    )
    .unwrap();
    let mut rows = Vec::new();
    for (id, (query, expected)) in frozen.iter().enumerate() {
        let admission = engine.source(&expected.path).unwrap();
        let exclusion = indexed.as_ref().and_then(|report| {
            report
                .exclusion_samples
                .iter()
                .find(|s| s.starts_with(&format!("{}:", expected.path)))
        });
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
            rows.push(json!({"id":id,"query":query,"expected":expected,"behavior":behavior,"tokens":packed.tokens,"traced_elapsed_ms":elapsed,"record":name,"source_admission":{"source_revision":source_revision,"present":admission.is_some(),"sha256":admission.as_ref().map(|s|&s.hash),"exclusion_sample":exclusion},"attribution":attribute(&r,query,expected)}));
        }
    }
    drop(engine);
    let mut variants = Vec::new();
    if let Some(binary) = std::env::var_os("FOUNDRY_ATTRIBUTION_BASELINE") {
        variants.push((
            "previous_experimental",
            std::path::PathBuf::from(binary),
            None,
        ));
    }
    if let Some(profile) = std::env::var_os("FOUNDRY_ATTRIBUTION_SEMANTIC_PROFILE") {
        variants.push((
            "candidate_semantic",
            std::path::PathBuf::from(env!("CARGO_BIN_EXE_foundry")),
            Some(std::path::PathBuf::from(profile)),
        ));
    }
    for (id, (query, expected)) in frozen.iter().enumerate() {
        for (variant, binary, profile) in &variants {
            let directory = output.join(format!("{id}-{variant}"));
            let started = std::time::Instant::now();
            // The external binary is explicitly supplied by the operator. No
            // indexed source selects or runs it. Both use the same store bytes.
            let mut command = std::process::Command::new("timeout");
            command
                .arg(if profile.is_some() { "90s" } else { "30s" })
                .arg(binary)
                .arg("--store")
                .arg(&store)
                .arg("context")
                .arg(query)
                .args([
                    "--tokens",
                    "2048",
                    "--strategy",
                    "search",
                    "--experimental-behavior-context",
                    "--retrieval-trace-dir",
                ])
                .arg(&directory);
            if let Some(profile) = profile {
                command
                    .arg("--semantic-profile")
                    .arg(profile)
                    .arg("--development-isolation");
            }
            let result = command.output().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let files: Vec<_> = std::fs::read_dir(&directory)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            assert_eq!(files.len(), 1);
            let r: Value = serde_json::from_slice(&std::fs::read(&files[0]).unwrap()).unwrap();
            assert_eq!(
                r["outcome"]["text"].as_str().unwrap().as_bytes(),
                result.stdout
            );
            let semantic_decisions: Vec<_> = r["trace"]["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|event| event["stage"] == "semantic")
                .map(|event| &event["decision"])
                .collect();
            rows.push(json!({"id":id,"query":query,"expected":expected,"variant":variant,"profile":profile,"semantic_decisions":semantic_decisions,"elapsed_ms":started.elapsed().as_millis(),"record":files[0],"attribution":attribute(&r,query,expected)}));
        }
    }
    let report = json!({"scope":"public_retrieval_diagnostic_not_blind_acceptance","historical_g2":false,"root":root,"external_labels":external,"excluded_evaluator_sources":[excluded],"indexed":indexed,"budget":2048,"rows":rows});
    std::fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{}", serde_json::to_string(&report).unwrap());
}
