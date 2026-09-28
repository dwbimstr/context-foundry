use context_foundry::{
    Engine, digest,
    graph::{Edge, Endpoint, GraphBundle},
    laya::{Feedback, Strategy},
};

fn drain(engine: &mut Engine) {
    while engine.refresh_index().unwrap() != 0 {}
}

fn endpoint(path: &str, body: &str) -> Endpoint {
    Endpoint {
        path: path.into(),
        line: 1,
        symbol: path.into(),
        hash: digest(body.as_bytes()),
    }
}

#[test]
fn changes_are_durable_before_search_and_old_search_hits_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut engine = Engine::open(dir.path()).unwrap();
        engine.replace_source("a.rs", "fn oldname() {}\n").unwrap();
        drain(&mut engine);
        assert_eq!(engine.search("oldname", 10).unwrap().hits.len(), 1);
        engine.replace_source("a.rs", "fn newname() {}\n").unwrap();
        let stale = engine.search("oldname", 10).unwrap();
        assert!(stale.hits.is_empty());
        assert_eq!(stale.stale_candidates, 1);
        assert_eq!(stale.pending_sources, 1);
    }
    let mut engine = Engine::open(dir.path()).unwrap();
    assert_eq!(engine.pending().unwrap(), 1);
    drain(&mut engine);
    assert_eq!(engine.search("newname", 10).unwrap().hits.len(), 1);
    assert!(!engine.replace_source("a.rs", "fn newname() {}\n").unwrap());
    engine.delete_source("a.rs").unwrap();
    assert!(engine.search("newname", 10).unwrap().hits.is_empty());
    drain(&mut engine);
    assert_eq!(engine.pending().unwrap(), 0);
}

#[test]
fn missing_search_index_is_rebuilt_from_authoritative_sources() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut engine = Engine::open(dir.path()).unwrap();
        engine.replace_source("a.rs", "durable source\n").unwrap();
        drain(&mut engine);
    }
    std::fs::remove_dir_all(dir.path().join("search")).unwrap();
    {
        let engine = Engine::open(dir.path()).unwrap();
        assert_eq!(engine.pending().unwrap(), 1);
        // Restart after index creation, before the rebuild has run.
        assert!(engine.search("durable", 5).unwrap().hits.is_empty());
    }
    let mut engine = Engine::open(dir.path()).unwrap();
    assert_eq!(engine.pending().unwrap(), 1);
    drain(&mut engine);
    assert_eq!(engine.search("durable", 5).unwrap().hits.len(), 1);
}

#[test]
fn graph_preserves_producers_checks_freshness_and_reports_bounds() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    for p in ["a.rs", "b.rs", "c.rs"] {
        engine.replace_source(p, "source\n").unwrap();
    }
    let bundle = |provider: &str, to: &str| GraphBundle {
        provider: provider.into(),
        revision: "fixture-1".into(),
        edges: vec![Edge {
            from: endpoint("a.rs", "source\n"),
            to: endpoint(to, "source\n"),
            kind: "calls".into(),
            evidence: "manual".into(),
        }],
    };
    engine.import_graph(&bundle("one", "b.rs")).unwrap();
    engine.import_graph(&bundle("two", "c.rs")).unwrap();
    assert_eq!(engine.graph("a.rs", false, 1, 10).unwrap().edges.len(), 2);
    assert!(engine.graph("a.rs", false, 1, 1).unwrap().truncated);
    assert_eq!(engine.graph("b.rs", true, 1, 10).unwrap().edges.len(), 1);
    engine
        .import_graph(&GraphBundle {
            provider: "one".into(),
            revision: "fixture-2".into(),
            edges: vec![],
        })
        .unwrap();
    assert_eq!(engine.graph("a.rs", false, 1, 10).unwrap().edges.len(), 1);
    engine.replace_source("c.rs", "changed\n").unwrap();
    let stale = engine.graph("a.rs", false, 1, 10).unwrap();
    assert!(stale.edges.is_empty());
    assert_eq!(stale.stale_edges, 1);
    assert!(engine.import_graph(&bundle("two", "c.rs")).is_err());
}

#[test]
fn context_counts_citations_and_omission_metadata_and_preserves_utf8() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    engine
        .replace_source("a.rs", &"café 東京 source retrieval\n".repeat(200))
        .unwrap();
    drain(&mut engine);
    let bundle = engine.context("source", 256).unwrap();
    assert!(bundle.tokens <= 256);
    assert_eq!(
        bundle.tokens,
        tiktoken_rs::o200k_base_singleton()
            .encode_ordinary(&bundle.text)
            .len()
    );
    assert!(bundle.text.contains("omitted candidates"));
    assert!(bundle.omitted > 0);
    assert!(engine.context("source", 0).is_err());
}

#[test]
fn feedback_requires_explicit_training_opt_in_and_keeps_tasks_in_one_split() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    let mut feedback = Feedback {
        task_id: "task-1".into(),
        query: "who calls parse".into(),
        correct_strategy: Strategy::Graph,
        label_source: "operator".into(),
        allow_training: false,
    };
    engine.record_feedback(&feedback).unwrap();
    assert!(engine.training_examples().unwrap().is_empty());
    feedback.allow_training = true;
    engine.record_feedback(&feedback).unwrap();
    feedback.query = "what depends on parse".into();
    engine.record_feedback(&feedback).unwrap();
    let examples = engine.training_examples().unwrap();
    assert_eq!(examples.len(), 2);
    assert_eq!(examples[0]["split"], examples[1]["split"]);
    feedback.allow_training = false;
    engine.record_feedback(&feedback).unwrap();
    assert_eq!(engine.training_examples().unwrap().len(), 1);
    feedback.label_source = "model_prediction".into();
    assert!(engine.record_feedback(&feedback).is_err());
}

#[test]
fn workspace_identity_and_invalid_paths_fail_without_mutating_sources() {
    let dir = tempfile::tempdir().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    engine.bind_workspace(first.path()).unwrap();
    assert!(engine.bind_workspace(second.path()).is_err());
    assert!(engine.replace_source("../escape", "content").is_err());
    assert!(engine.paths().unwrap().is_empty());
}
