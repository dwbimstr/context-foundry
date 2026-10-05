//! Authoritative integrity: verified reconstruction, strict counters, path
//! syntax, graph degradation, truncation metadata, binding, read purity and
//! mutation INSIDE one response.
use context_foundry::fault::{self, Action, names};
use context_foundry::graph::{Edge, Endpoint, GraphBundle};
use context_foundry::laya::{Feedback, Strategy as LayaStrategy};
use context_foundry::store::{CandidateBatch, HandleRef, RenderedForm};
use context_foundry::testkit;
use context_foundry::testkit::{
    craft_v1_store, insert_raw_edge, knowledge, new_fixture, remove_chunk, schema_marker, set_meta,
    snapshot, tamper_chunk_body,
};
use context_foundry::{Control, Engine, FoundryError, Strategy, digest};

/// Whether any candidate's first form (a unit's bytes, an outline or a graph
/// line) contains `needle`.
fn mentions(batch: &CandidateBatch, needle: &str) -> bool {
    batch.items.iter().any(|item| {
        item.forms.first().is_some_and(|form| match form {
            RenderedForm::Verbatim(text)
            | RenderedForm::Signature(text)
            | RenderedForm::Outline(text)
            | RenderedForm::OutlineMin(text)
            | RenderedForm::Line(text) => text.contains(needle),
        })
    })
}

fn endpoint(path: &str, body: &str) -> Endpoint {
    Endpoint {
        path: path.into(),
        line: 1,
        symbol: path.into(),
        hash: digest(body.as_bytes()),
    }
}

fn edge_bundle(provider: &str, from: (&str, &str), to: (&str, &str)) -> GraphBundle {
    GraphBundle {
        provider: provider.into(),
        revision: "r1".into(),
        edges: vec![Edge {
            from: endpoint(from.0, from.1),
            to: endpoint(to.0, to.1),
            kind: "calls".into(),
            evidence: "manual".into(),
        }],
    }
}

fn feedback(task: &str) -> Feedback {
    Feedback {
        task_id: task.into(),
        query: format!("query for {task}"),
        correct_strategy: LayaStrategy::Graph,
        label_source: "operator".into(),
        allow_training: true,
    }
}

#[test]
fn inconsistent_chunks_are_corrupt_source_for_search_context_and_retrieve() {
    const BODY: &str = "fn parse_record() {}\n";
    for (case, tamper) in [
        ("modified body", 0u8),
        ("missing chunk", 1),
        ("other hash recorded", 2),
        ("same-length modification", 3),
    ] {
        let mut fx = new_fixture();
        fx.add(&[("lib.rs", BODY)]);
        let handle = fx
            .engine
            .search("parse_record", 5)
            .unwrap()
            .hits
            .remove(0)
            .handle;
        let (_dir, store, _root) = fx.close();
        match tamper {
            0 => tamper_chunk_body(&store, "lib.rs", 0, "TAMPERED_BYTES_NOT_MATCHING_HASH\n"),
            1 => remove_chunk(&store, "lib.rs", 0),
            2 => testkit::retag_chunk_hash(&store, "lib.rs", 0, &digest(b"another version")),
            // Same byte length, different bytes: only the hash can catch it.
            _ => tamper_chunk_body(&store, "lib.rs", 0, "fn parse_recorD() {}\n"),
        }
        let engine = Engine::open_existing(&store).unwrap();
        let search = engine.search("parse_record", 5).unwrap_err();
        assert_eq!(search.code(), "corrupt_source", "search: {case}");
        let context = engine
            .context_candidates("parse_record", Strategy::Search, &Control::unbounded())
            .unwrap_err();
        assert_eq!(context.code(), "corrupt_source", "context: {case}");
        let retrieve = engine.retrieve(&handle.to_v2(), None, 4096).unwrap_err();
        assert_eq!(retrieve.code(), "corrupt_source", "retrieve: {case}");
    }
}

/// Run one operation on a freshly opened engine, then release the store.
fn op<R>(store: &std::path::Path, f: impl FnOnce(&mut Engine) -> R) -> R {
    let mut engine = Engine::open_existing(store).unwrap();
    f(&mut engine)
}

#[test]
fn malformed_or_missing_counters_are_corrupt_store_and_never_reset() {
    for (key, value) in [
        ("source_revision", Some("garbage")),
        ("source_revision", None),
        ("scan_id", Some("-1")),
        ("scan_id", None),
        ("scan_status", Some("bogus")),
        ("scan_status", None),
    ] {
        let mut fx = new_fixture();
        fx.add(&[("a.rs", "fn counters() {}\n")]);
        let root = fx.root.clone();
        let (_dir, store, _root) = fx.close();
        set_meta(&store, key, value);
        let case = format!("{key}={value:?}");
        let revision = key == "source_revision";
        let scan_status = key == "scan_status";
        // status reads revision and scan state strictly.
        let status = op(&store, |e| e.status());
        if revision || scan_status {
            assert_eq!(status.unwrap_err().code(), "corrupt_store", "status {case}");
        } else {
            status.unwrap();
        }
        let before = snapshot(&store);
        if revision {
            // A source mutation must not substitute zero and commit.
            let replace = op(&store, |e| e.replace_source("b.rs", "fn other() {}\n"));
            assert_eq!(
                replace.unwrap_err().code(),
                "corrupt_store",
                "replace {case}"
            );
            let delete = op(&store, |e| e.delete_source("a.rs"));
            assert_eq!(delete.unwrap_err().code(), "corrupt_store", "delete {case}");
            assert_eq!(snapshot(&store), before, "source rows changed for {case}");
        }
        // Every corrupt required value stops indexing before any mutation.
        let index = op(&store, |e| e.index(&root, &Control::unbounded()));
        assert_eq!(index.unwrap_err().code(), "corrupt_store", "index {case}");
        assert_eq!(snapshot(&store), before, "index rewrote {case}");
    }
}

#[test]
fn path_syntax_is_validated_raw_before_workspace_and_never_committed() {
    let mut fx = new_fixture();
    fx.add(&[("good.rs", "fn good() {}\n")]);
    let foreign = digest(b"another workspace root");
    let sha = digest(b"anything");
    for bad in [
        "a//b.rs",
        "a/./b.rs",
        "a/b.rs/",
        "/a.rs",
        "a/../b.rs",
        "..",
        ".",
        "",
        "a\nb.rs",
        "a\rb.rs",
        "a\0b.rs",
        "a/",
        "//",
    ] {
        let handle = format!("{bad}#0-1@{}.{}", &sha[..32], &foreign[..16]);
        // Field validation precedes the workspace match.
        let err = fx.engine.retrieve(&handle, None, 2048).unwrap_err();
        assert_eq!(err.code(), "invalid_argument", "retrieve {bad:?}");
        let err = fx.engine.replace_source(bad, "x\n").unwrap_err();
        assert_eq!(err.code(), "invalid_argument", "replace {bad:?}");
        assert!(fx.engine.source(bad).unwrap().is_none());
    }
    let too_long = format!("{}.rs", "a".repeat(4094));
    assert_eq!(too_long.len(), 4097);
    assert_eq!(
        fx.engine
            .replace_source(&too_long, "x\n")
            .unwrap_err()
            .code(),
        "invalid_argument"
    );
    fx.engine.replace_source(&too_long[1..], "x\n").unwrap();
    assert_eq!(fx.engine.status().unwrap().source_count, 2);
}

#[test]
fn scan_never_commits_a_path_the_handle_rules_reject() {
    let mut fx = new_fixture();
    std::fs::write(fx.root.join("ok.rs"), "fn ok() {}\n").unwrap();
    // A newline is legal in a unix file name and forbidden in a source path.
    std::fs::write(fx.root.join("bad\nname.rs"), "fn bad() {}\n").unwrap();
    let report = fx
        .engine
        .index(&fx.root.clone(), &Control::unbounded())
        .unwrap();
    assert_eq!(report.failures, 1);
    assert!(report.failure_samples[0].contains("invalid_source_path"));
    assert!(report.deletions_deferred && report.partial);
    assert_eq!(report.reason_code, Some("scan_failures"));
    assert_eq!(fx.engine.status().unwrap().source_count, 1);
    assert!(fx.engine.source("bad\nname.rs").unwrap().is_none());
}

#[test]
fn graph_context_names_invalid_stale_and_unavailable_without_losing_source() {
    const A: &str = "fn alpha_probe() { beta_probe(); }\n";
    const B: &str = "fn beta_probe() {}\n";
    // Unavailable: no edges at all.
    let mut fx = new_fixture();
    fx.add(&[("a.rs", A), ("b.rs", B)]);
    let outcome = fx
        .engine
        .context_candidates("alpha_probe", Strategy::Graph, &Control::unbounded())
        .unwrap();
    assert_eq!(outcome.counters.graph, Some("graph_unavailable"));
    assert!(mentions(&outcome, "alpha_probe"));
    // Healthy: a fresh edge, no reason.
    fx.engine
        .import_graph(&edge_bundle("p", ("a.rs", A), ("b.rs", B)))
        .unwrap();
    let outcome = fx
        .engine
        .context_candidates("alpha_probe", Strategy::Graph, &Control::unbounded())
        .unwrap();
    assert_eq!(outcome.counters.graph, Some("ok"));
    assert!(mentions(&outcome, "--calls-->"));
    // Stale only: every stored edge no longer matches its source hashes.
    fx.engine
        .replace_source("b.rs", "fn beta_probe() { changed(); }\n")
        .unwrap();
    fx.drain();
    let outcome = fx
        .engine
        .context_candidates("alpha_probe", Strategy::Graph, &Control::unbounded())
        .unwrap();
    assert_eq!(outcome.counters.graph, Some("graph_stale"));
    assert!(!mentions(&outcome, "--calls-->"));
    assert!(mentions(&outcome, "alpha_probe"));
    // Invalid: an undecodable edge row degrades only the graph component.
    let (_dir, store, _root) = fx.close();
    insert_raw_edge(&store, "a.rs", "{\"not\": \"an edge\"");
    let engine = Engine::open_existing(&store).unwrap();
    let outcome = engine
        .context_candidates("alpha_probe", Strategy::Graph, &Control::unbounded())
        .unwrap();
    assert_eq!(outcome.counters.graph, Some("graph_invalid"));
    assert!(mentions(&outcome, "alpha_probe"));
    // The direct graph request names the same component-local failure.
    assert_eq!(
        engine.graph("a.rs", false, 1, 8).unwrap_err().code(),
        "graph_invalid"
    );
}

#[test]
fn search_reports_truncation_and_orders_ties_deterministically() {
    let mut fx = new_fixture();
    fx.add(&[
        ("t1.rs", "needle_match\n"),
        ("t2.rs", "needle_match\n"),
        ("t3.rs", "needle_match\n"),
    ]);
    let one = fx.engine.search("needle_match", 1).unwrap();
    assert_eq!(one.hits.len(), 1);
    assert!(one.truncated, "a dropped current hit must set truncated");
    assert!(!one.candidate_limit_reached);
    let all = fx.engine.search("needle_match", 3).unwrap();
    assert_eq!(all.hits.len(), 3);
    assert!(!all.truncated);
    let paths: Vec<&str> = all.hits.iter().map(|h| h.path.as_str()).collect();
    assert_eq!(paths, ["t1.rs", "t2.rs", "t3.rs"], "ties sort by path");
    // A filled 256-candidate window is a limitation, whether or not hits remain.
    let bulk: Vec<(String, String)> = (0..300)
        .map(|i| (format!("w{i:03}.rs"), "window_match\n".to_owned()))
        .collect();
    let refs: Vec<(&str, &str)> = bulk.iter().map(|(p, b)| (p.as_str(), b.as_str())).collect();
    fx.add(&refs);
    let window = fx.engine.search("window_match", 64).unwrap();
    assert!(window.candidate_limit_reached && window.truncated);
    assert_eq!(window.hits.len(), 64);
    let keys: Vec<(&str, u64)> = window
        .hits
        .iter()
        .map(|h| (h.path.as_str(), h.handle.start))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "equal scores order by path then byte start");
    sorted.dedup();
    assert_eq!(sorted.len(), 64);
    // The context packer carries both limitations into its outcome.
    let outcome = fx
        .engine
        .context_candidates("window_match", Strategy::Search, &Control::unbounded())
        .unwrap();
    assert!(outcome.counters.candidates_full && outcome.counters.truncated);
}

#[test]
fn unbound_upgraded_store_binds_and_serves_queries_on_the_same_owner() {
    let fixture = tempfile::tempdir().unwrap();
    let v1 = fixture.path().join("v1store");
    craft_v1_store(&v1, None);
    Engine::upgrade_store(&v1, 4, &Control::unbounded()).unwrap();
    // The upgraded index predates search schema v2: the explicit repair
    // publishes it before the store serves queries.
    let engine = Engine::open_existing(&v1).unwrap();
    let status = engine.status().unwrap();
    assert_eq!(status.index_state, "repair_required");
    assert!(
        status
            .index_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("search_schema")),
        "{status:?}"
    );
    drop(engine);
    assert!(
        Engine::repair_index(&v1, &Control::unbounded())
            .unwrap()
            .repaired
    );
    let mut engine = Engine::open_existing(&v1).unwrap();
    assert!(engine.workspace_id().is_none());
    assert_eq!(
        engine.search("kept", 5).unwrap_err().code(),
        "workspace_unbound"
    );
    let root = fixture.path().join("ws");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("kept.rs"), testkit::KEPT_BODY).unwrap();
    let report = engine.index(&root, &Control::unbounded()).unwrap();
    assert!(!report.partial, "{report:?}");
    // No reopen: the live owner now carries the binding it just committed.
    let expected = context_foundry::workspace_id_for_root(&root).unwrap();
    assert_eq!(engine.workspace_id().as_deref(), Some(expected.as_str()));
    assert_eq!(
        engine.status().unwrap().workspace_id.as_deref(),
        Some(expected.as_str())
    );
    let hit = engine.search("kept", 5).unwrap().hits.remove(0);
    assert_eq!(hit.handle.workspace_id, expected);
    let outcome = engine.retrieve(&hit.handle.to_v2(), None, 2048).unwrap();
    assert_eq!(outcome.span, testkit::KEPT_BODY.as_bytes());
    // A second root is refused after the bind.
    let other = fixture.path().join("other");
    std::fs::create_dir(&other).unwrap();
    assert_eq!(
        engine
            .index(&other, &Control::unbounded())
            .unwrap_err()
            .code(),
        "wrong_workspace"
    );
}

#[test]
fn repeated_reads_leave_every_authoritative_row_and_pending_unchanged() {
    const A: &str = "fn read_probe() { helper(); }\n";
    const B: &str = "fn helper() {}\n";
    let mut fx = new_fixture();
    fx.add(&[("a.rs", A), ("b.rs", B)]);
    fx.engine
        .import_graph(&edge_bundle("p", ("a.rs", A), ("b.rs", B)))
        .unwrap();
    fx.engine.record_feedback(&feedback("t1")).unwrap();
    // Unindexed work: pending stays nonzero, so reads cannot "drain" it.
    fx.engine
        .replace_source("c.rs", "fn pending_probe() {}\n")
        .unwrap();
    let hit = fx.engine.search("read_probe", 5).unwrap().hits.remove(0);
    let (_dir, store, _root) = fx.close();
    let before = snapshot(&store);
    let pending_before = before["pending_index"].clone();
    assert!(!pending_before.is_empty());
    for round in 0..3 {
        let engine = Engine::open_existing(&store).unwrap();
        engine.status().unwrap();
        engine.search("read_probe", 5).unwrap();
        engine
            .context_candidates(
                "references to read_probe",
                Strategy::Auto,
                &Control::unbounded(),
            )
            .unwrap();
        engine.retrieve(&hit.handle.to_v2(), None, 4096).unwrap();
        engine.graph("a.rs", false, 1, 16).unwrap();
        engine.training_examples().unwrap();
        // A missing-handle read and a failed read mutate nothing either.
        assert!(engine.retrieve("{\"v\":1}", None, 8).is_err());
        drop(engine);
        assert_eq!(
            snapshot(&store),
            before,
            "round {round}: a read changed state"
        );
    }
}

#[test]
fn broken_search_still_serves_status_retrieve_graph_and_feedback_exports() {
    const A: &str = "fn serve_probe() { dep(); }\n";
    const B: &str = "fn dep() {}\n";
    let mut fx = new_fixture();
    fx.add(&[("a.rs", A), ("b.rs", B)]);
    fx.engine
        .import_graph(&edge_bundle("p", ("a.rs", A), ("b.rs", B)))
        .unwrap();
    fx.engine.record_feedback(&feedback("t1")).unwrap();
    let handle = fx
        .engine
        .search("serve_probe", 5)
        .unwrap()
        .hits
        .remove(0)
        .handle;
    let expected_examples = fx.engine.training_examples().unwrap();
    let (_dir, store, _root) = fx.close();
    testkit::corrupt_search_index(&store);
    let before = knowledge(&snapshot(&store));
    let engine = Engine::open_existing(&store).unwrap();
    let status = engine.status().unwrap();
    assert_eq!(status.index_state, "repair_required");
    assert_eq!(
        engine.search("serve_probe", 5).unwrap_err().code(),
        "repair_required"
    );
    assert_eq!(
        engine
            .context_candidates("serve_probe", Strategy::Search, &Control::unbounded())
            .unwrap_err()
            .code(),
        "repair_required"
    );
    // The handle is the whole `serve_probe` unit, without the final LF.
    assert_eq!(
        engine.retrieve(&handle.to_v2(), None, 4096).unwrap().span,
        b"fn serve_probe() { dep(); }"
    );
    let graph = engine.graph("a.rs", false, 1, 8).unwrap();
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(engine.training_examples().unwrap(), expected_examples);
    drop(engine);
    assert_eq!(knowledge(&snapshot(&store)), before);
}

#[test]
fn unknown_schema_is_refused_and_changes_no_record() {
    let fixture = tempfile::tempdir().unwrap();
    let store = fixture.path().join("future");
    craft_v1_store(&store, Some(&fixture.path().join("ws")));
    set_meta(&store, "schema", Some("5"));
    let before = snapshot(&store);
    assert_eq!(
        Engine::open_existing(&store).unwrap_err().code(),
        "unsupported_schema"
    );
    assert_eq!(
        Engine::upgrade_store(&store, 4, &Control::unbounded())
            .unwrap_err()
            .code(),
        "unsupported_schema"
    );
    assert_eq!(
        Engine::repair_index(&store, &Control::unbounded())
            .unwrap_err()
            .code(),
        "unsupported_schema"
    );
    assert_eq!(schema_marker(&store), "5");
    assert_eq!(snapshot(&store), before);
}

fn two_source_fixture() -> testkit::Fixture {
    let mut fx = new_fixture();
    fx.add(&[
        ("keep.rs", "fn mutation_probe() { shared(); }\n"),
        ("churn.rs", "fn mutation_probe_two() { shared(); }\n"),
    ]);
    fx
}

#[test]
fn edit_between_candidate_collection_and_final_validation_is_omitted_not_relabeled() {
    let fx = two_source_fixture();
    let revision_before = fx.engine.source_revision().unwrap();
    fault::arm(
        names::CONTEXT_BEFORE_FINAL_VALIDATION,
        0,
        Action::Call(Box::new(|ctx| {
            ctx.engine
                .unwrap()
                .replace_source("churn.rs", "fn totally_different() {}\n")
                .unwrap();
        })),
    );
    let outcome = fx
        .engine
        .context_candidates("mutation_probe", Strategy::Search, &Control::unbounded())
        .unwrap();
    fault::disarm_all();
    // The response reports the FINAL read transaction's revision, and carries
    // only evidence valid in that snapshot.
    assert_eq!(outcome.freshness.source_revision, revision_before + 1);
    assert!(mentions(&outcome, "mutation_probe() "));
    assert!(!mentions(&outcome, "mutation_probe_two"));
    assert!(outcome.counters.stale >= 1);
}

#[test]
fn delete_between_candidate_collection_and_final_validation_is_omitted() {
    let fx = two_source_fixture();
    fault::arm(
        names::CONTEXT_BEFORE_FINAL_VALIDATION,
        0,
        Action::Call(Box::new(|ctx| {
            assert!(ctx.engine.unwrap().delete_source("churn.rs").unwrap());
        })),
    );
    let outcome = fx
        .engine
        .context_candidates("mutation_probe", Strategy::Search, &Control::unbounded())
        .unwrap();
    fault::disarm_all();
    assert!(!mentions(&outcome, "mutation_probe_two"));
    assert!(outcome.counters.stale >= 1);
    assert!(mentions(&outcome, "mutation_probe() "));
}

#[test]
fn graph_replacement_between_candidate_collection_and_final_validation_is_omitted() {
    const A: &str = "fn graph_inject() { dep_inject(); }\n";
    const B: &str = "fn dep_inject() {}\n";
    let mut fx = new_fixture();
    fx.add(&[("a.rs", A), ("b.rs", B)]);
    fx.engine
        .import_graph(&edge_bundle("p", ("a.rs", A), ("b.rs", B)))
        .unwrap();
    // The graph row still matches its source hashes; only the producer's
    // bundle is replaced inside the response.
    fault::arm(
        names::CONTEXT_BEFORE_FINAL_VALIDATION,
        0,
        Action::Call(Box::new(|ctx| {
            ctx.engine
                .unwrap()
                .import_graph(&GraphBundle {
                    provider: "p".into(),
                    revision: "r2".into(),
                    edges: vec![],
                })
                .unwrap();
        })),
    );
    let outcome = fx
        .engine
        .context_candidates("graph_inject", Strategy::Graph, &Control::unbounded())
        .unwrap();
    fault::disarm_all();
    assert!(!mentions(&outcome, "--calls-->"));
    assert!(mentions(&outcome, "graph_inject"));
    assert!(outcome.counters.stale >= 1);
    assert_eq!(outcome.counters.graph, Some("graph_stale"));
}

#[test]
fn four_thousand_ninety_six_byte_escaped_path_round_trips_through_search_and_handles() {
    // `#`, `@`, `.` and JSON-special characters appear unescaped in a v2
    // handle, so a 4096-byte path stays within the 4200-byte handle cap.
    let component = "\"\\#@.".repeat(40);
    let mut parts = Vec::new();
    while parts.len() * 201 + 200 < 4096 {
        parts.push(component.clone());
    }
    let mut path = parts.join("/");
    path.push('/');
    path.push_str(&"q".repeat(4096 - path.len()));
    assert_eq!(path.len(), 4096);
    let mut fx = new_fixture();
    fx.add(&[(&path, "fn escaped_path_probe() {}\n")]);
    let hit = fx
        .engine
        .search("escaped_path_probe", 5)
        .unwrap()
        .hits
        .remove(0);
    assert_eq!(hit.path, path);
    let handle = hit.handle.to_v2();
    assert!(handle.len() <= 4200, "{}", handle.len());
    let out = fx.engine.retrieve(&handle, None, 4096).unwrap();
    assert_eq!(out.span, b"fn escaped_path_probe() {}\n");
    // The search wire round trip preserves the exact path.
    let outcome = fx.engine.search("escaped_path_probe", 5).unwrap();
    let text = context_foundry::response::pack_search(
        &outcome,
        context_foundry::response::Budget::request(32768),
        &context_foundry::response::stdout_bytes,
    )
    .unwrap()
    .text;
    let located = testkit::parse_v2(&text).unwrap().items.remove(0).handle;
    assert_eq!(located, handle);
    assert_eq!(HandleRef::parse(&located).unwrap().path, path);
    // Over the 4200-byte input cap is an invalid argument, not a read.
    for over in [format!("{handle}{}", " ".repeat(64)), "x".repeat(4201)] {
        assert_eq!(
            fx.engine.retrieve(&over, None, 4096).unwrap_err().code(),
            "invalid_argument"
        );
    }
    let _ = FoundryError::NotFound;
}

#[test]
fn delay_stalls_read_boundaries_for_external_deadline_testing() {
    let mut fx = new_fixture();
    fx.add(&[("d.rs", "fn delay_probe() {}\n")]);
    let handle = fx
        .engine
        .search("delay_probe", 5)
        .unwrap()
        .hits
        .remove(0)
        .handle;
    // Boundary between validation and the final authoritative read.
    fault::arm(
        names::RETRIEVE_BEFORE_FINAL_READ,
        0,
        Action::Delay(std::time::Duration::from_millis(120)),
    );
    let started = std::time::Instant::now();
    let out = fx.engine.retrieve(&handle.to_v2(), None, 2048).unwrap();
    fault::disarm_all();
    assert!(started.elapsed() >= std::time::Duration::from_millis(120));
    assert_eq!(out.span, b"fn delay_probe() {}");
    // And between candidate collection and the final context validation.
    let mut fx = new_fixture();
    fx.add(&[("d.rs", "fn delay_probe() {}\n")]);
    fault::arm(
        names::CONTEXT_BEFORE_FINAL_VALIDATION,
        0,
        Action::Delay(std::time::Duration::from_millis(120)),
    );
    let started = std::time::Instant::now();
    let outcome = fx
        .engine
        .context_candidates("delay_probe", Strategy::Search, &Control::unbounded())
        .unwrap();
    fault::disarm_all();
    assert!(started.elapsed() >= std::time::Duration::from_millis(120));
    assert!(mentions(&outcome, "delay_probe"));
}
