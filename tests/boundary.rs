use context_foundry::{
    Control, Engine, boundary,
    fault::{self, Action, names},
};
use serde_json::json;
use std::process::Command;
const BIN: &str = env!("CARGO_BIN_EXE_foundry");
const PROVIDER: &str = "from fastapi import FastAPI\napp = FastAPI()\n@app.get('/v1/items')\ndef items():\n    return {'items': []}\n";
const CLIENT: &str =
    "import httpx\ndef fetch():\n    return httpx.get('http://catalog/v1/items?limit=3')\n";
fn fixture() -> (tempfile::TempDir, Engine) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("root")).unwrap();
    let e = Engine::initialize(&dir.path().join("store"), &dir.path().join("root")).unwrap();
    e.replace_source("boundary.json", &json!({"v":1,"services":[{"id":"catalog","providers":["service.py","other.py"],"origins":["http://catalog"]}],"consumers":["client.py","test_contract.py"]}).to_string()).unwrap();
    e.replace_source("service.py", PROVIDER).unwrap();
    e.replace_source("other.py", "from fastapi import FastAPI\napp = FastAPI()\n")
        .unwrap();
    e.replace_source("client.py", CLIENT).unwrap();
    e.replace_source("test_contract.py","import requests\ndef test_items():\n    assert requests.get('http://catalog/v1/items').status_code == 200\n").unwrap();
    (dir, e)
}
fn publish(e: &Engine) -> boundary::Publication {
    e.index_boundaries("boundary.json", &[], &Control::unbounded())
        .unwrap()
}
fn read(e: &Engine) -> boundary::Relations {
    e.boundary_relations(
        "boundary.json",
        &["service.py".into(), "client.py".into()],
        &Control::unbounded(),
    )
    .unwrap()
}
#[test]
fn publishes_both_spans_reuses_and_recovers_after_restart() {
    let (dir, e) = fixture();
    assert_eq!(publish(&e).parsed, 4);
    let r = read(&e);
    assert_eq!(r.relations.len(), 2);
    for r in r.relations {
        assert_eq!(r.resolution, "exact_key");
        assert_eq!(r.providers.len(), 1);
        assert_eq!(
            r.providers[0].evidence.sha256,
            context_foundry::digest(PROVIDER.as_bytes())
        );
        assert!(r.consumer.context.start < r.consumer.evidence.start);
    }
    let reused = publish(&e);
    assert_eq!(reused.parsed, 0);
    assert_eq!(reused.reused, 4);
    drop(e);
    let e = Engine::open_existing(&dir.path().join("store")).unwrap();
    assert_eq!(read(&e).relations[0].resolution, "exact_key");
}
#[test]
fn changed_unpublished_provider_cannot_prove_uniqueness_and_incremental_replaces() {
    let (_dir, e) = fixture();
    publish(&e);
    e.replace_source("other.py", PROVIDER).unwrap();
    assert!(
        read(&e)
            .relations
            .iter()
            .all(|r| r.resolution == "incomplete")
    );
    let p = e
        .index_boundaries("boundary.json", &["other.py".into()], &Control::unbounded())
        .unwrap();
    assert_eq!(p.parsed, 1);
    assert!(
        read(&e)
            .relations
            .iter()
            .all(|r| r.resolution == "ambiguous" && r.providers.len() == 2)
    );
    e.replace_source("other.py", &PROVIDER.replace("/v1/items", "/v2/items"))
        .unwrap();
    publish(&e);
    assert!(
        read(&e)
            .relations
            .iter()
            .all(|r| r.resolution == "exact_key")
    );
    e.replace_source("service.py", &PROVIDER.replace("/v1/items", "/v2/items"))
        .unwrap();
    publish(&e);
    assert!(
        read(&e)
            .relations
            .iter()
            .all(|r| r.resolution == "unresolved")
    );
}
#[test]
fn source_and_configuration_changes_suppress_stale_links_without_republication() {
    let (_dir, e) = fixture();
    publish(&e);
    e.delete_source("client.py").unwrap();
    assert_eq!(read(&e).relations.len(), 1);
    e.replace_source("boundary.json", &json!({"v":1,"services":[{"id":"catalog","providers":["service.py"],"origins":["https://catalog"]}],"consumers":["test_contract.py"]}).to_string()).unwrap();
    assert!(read(&e).relations.is_empty());
    publish(&e);
    let r = read(&e);
    assert!(r.relations.is_empty());
    let r = e
        .boundary_relations(
            "boundary.json",
            &["test_contract.py".into()],
            &Control::unbounded(),
        )
        .unwrap();
    assert_eq!(r.relations[0].resolution, "unsupported");
}
#[test]
fn syntax_distinguishes_calls_from_logs_dynamic_urls_shadowing_and_template_routes() {
    let (_dir, e) = fixture();
    e.replace_source("client.py", "import httpx as h\ndef fetch(url):\n    print(\"h.get('http://catalog/v1/items')\")\n    h.get(url)\n    h.get('http://catalog/v1/items')\n").unwrap();
    publish(&e);
    let r = e
        .boundary_relations(
            "boundary.json",
            &["client.py".into()],
            &Control::unbounded(),
        )
        .unwrap();
    let calls: Vec<_> = r
        .relations
        .iter()
        .filter(|r| r.consumer.evidence.path == "client.py")
        .collect();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].resolution, "unsupported");
    assert_eq!(calls[1].resolution, "exact_key");
    e.replace_source(
        "client.py",
        "import httpx\ndef fetch(httpx):\n    return httpx.get('http://catalog/v1/items')\n",
    )
    .unwrap();
    publish(&e);
    assert_eq!(
        e.boundary_relations(
            "boundary.json",
            &["client.py".into()],
            &Control::unbounded()
        )
        .unwrap()
        .relations[0]
            .resolution,
        "unsupported"
    );
    e.replace_source("service.py", &PROVIDER.replace("/v1/items", "/v1/{item}"))
        .unwrap();
    publish(&e);
    assert!(
        read(&e)
            .incomplete_paths
            .contains(&"service.py".to_string())
    );
}
#[test]
fn cancellation_and_commit_failure_preserve_prior_postings() {
    let (dir, e) = fixture();
    publish(&e);
    e.replace_source("client.py", &CLIENT.replace("/v1/items", "/v2/items"))
        .unwrap();
    assert!(
        e.index_boundaries("boundary.json", &[], &Control::cancelled())
            .is_err()
    );
    fault::arm(
        names::BOUNDARY_BEFORE_COMMIT,
        0,
        Action::Fail("injected".into()),
    );
    assert!(
        e.index_boundaries("boundary.json", &[], &Control::unbounded())
            .is_err()
    );
    fault::disarm_all();
    drop(e);
    let e = Engine::open_existing(&dir.path().join("store")).unwrap();
    assert!(read(&e).incomplete_paths.contains(&"client.py".into()));
    e.replace_source("client.py", CLIENT).unwrap();
    assert_eq!(read(&e).relations.len(), 2);
}
#[test]
fn cli_publication_traversal_and_budget_are_real_boundaries() {
    let (dir, e) = fixture();
    drop(e);
    let store = dir.path().join("store");
    let out = Command::new(BIN)
        .arg("--store")
        .arg(&store)
        .args(["boundary", "index", "--config", "boundary.json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = Command::new(BIN)
        .arg("--store")
        .arg(&store)
        .args([
            "boundary",
            "relations",
            "--config",
            "boundary.json",
            "--path",
            "service.py",
            "--tokens",
            "1300",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(context_foundry::response::count_tokens(&text) <= 1300);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["evidence_class"], "syntactic");
}

#[test]
fn publication_rejects_changed_dependencies_after_extraction() {
    let (_dir, e) = fixture();
    publish(&e);
    e.replace_source("client.py", &CLIENT.replace("/v1/items", "/v2/items"))
        .unwrap();
    fault::arm(
        names::BOUNDARY_BEFORE_ADMISSION,
        0,
        Action::Call(Box::new(|ctx| {
            ctx.engine
                .unwrap()
                .replace_source("client.py", "# concurrent replacement\n")
                .unwrap();
        })),
    );
    let result = e.index_boundaries("boundary.json", &[], &Control::unbounded());
    fault::disarm_all();
    assert!(matches!(
        result,
        Err(context_foundry::FoundryError::Conflict(_))
    ));
    assert!(read(&e).incomplete_paths.contains(&"client.py".into()));
}
#[test]
fn unsafe_bindings_and_scanner_limits_do_not_become_unique_matches() {
    let (_dir, e) = fixture();
    e.replace_source(
        "service.py",
        &format!("{PROVIDER}\napp.get = replacement\n"),
    )
    .unwrap();
    publish(&e);
    assert!(
        read(&e)
            .relations
            .iter()
            .all(|r| r.resolution == "incomplete")
    );
    let body = (0..300)
        .map(|i| format!("{}if True:\n", " ".repeat(i)))
        .collect::<String>();
    e.replace_source("service.py", &body).unwrap();
    assert!(publish(&e).incomplete > 0);
    assert!(
        read(&e)
            .relations
            .iter()
            .all(|r| r.resolution == "incomplete")
    );
}
#[test]
fn boundary_abort_child() {
    let Ok(path) = std::env::var("CF_BOUNDARY_ABORT_STORE") else {
        return;
    };
    let e = Engine::open_existing(std::path::Path::new(&path)).unwrap();
    let name = if std::env::var("CF_BOUNDARY_ABORT_AFTER").unwrap() == "true" {
        names::BOUNDARY_AFTER_COMMIT
    } else {
        names::BOUNDARY_BEFORE_COMMIT
    };
    fault::arm(name, 0, Action::Abort);
    let _ = e.index_boundaries("boundary.json", &[], &Control::unbounded());
    panic!("abort point was not reached");
}
#[test]
fn process_death_leaves_publication_wholly_old_or_new() {
    for after in [false, true] {
        let (dir, e) = fixture();
        publish(&e);
        e.replace_source("client.py", &CLIENT.replace("/v1/items", "/v2/items"))
            .unwrap();
        drop(e);
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "boundary_abort_child"])
            .env("CF_BOUNDARY_ABORT_STORE", dir.path().join("store"))
            .env("CF_BOUNDARY_ABORT_AFTER", after.to_string())
            .output()
            .unwrap();
        assert!(!out.status.success());
        let e = Engine::open_existing(&dir.path().join("store")).unwrap();
        let r = read(&e);
        assert_eq!(
            r.incomplete_paths.contains(&"client.py".to_string()),
            !after
        );
        if after {
            assert!(
                r.relations.iter().any(
                    |r| r.consumer.evidence.path == "client.py" && r.resolution == "unresolved"
                )
            );
        } else {
            e.replace_source("client.py", CLIENT).unwrap();
            assert_eq!(read(&e).relations.len(), 2);
        }
    }
}
#[test]
fn schema_ten_requires_atomic_explicit_upgrade() {
    use redb::{MultimapTableDefinition, TableDefinition};
    let (dir, e) = fixture();
    let hash = e.source("service.py").unwrap().unwrap().hash;
    drop(e);
    let store = dir.path().join("store");
    let db = redb::Database::open(store.join("knowledge.redb")).unwrap();
    let tx = db.begin_write().unwrap();
    tx.delete_table(TableDefinition::<&str, &str>::new("boundary_scopes"))
        .unwrap();
    tx.delete_multimap_table(MultimapTableDefinition::<&str, &str>::new("boundary_keys"))
        .unwrap();
    tx.open_table(TableDefinition::<&str, &str>::new("meta"))
        .unwrap()
        .insert("schema", "10")
        .unwrap();
    tx.commit().unwrap();
    drop(db);
    assert!(matches!(
        Engine::open_existing(&store),
        Err(context_foundry::FoundryError::UpgradeRequired { .. })
    ));
    fault::arm(
        names::UPGRADE_BEFORE_COMMIT,
        0,
        Action::Fail("injected".into()),
    );
    assert!(Engine::upgrade_store(&store, 11, &Control::unbounded()).is_err());
    fault::disarm_all();
    assert!(matches!(
        Engine::open_existing(&store),
        Err(context_foundry::FoundryError::UpgradeRequired { .. })
    ));
    Engine::upgrade_store(&store, 11, &Control::unbounded()).unwrap();
    let e = Engine::open_existing(&store).unwrap();
    assert_eq!(e.source("service.py").unwrap().unwrap().hash, hash);
    assert_eq!(publish(&e).parsed, 4);
}

#[test]
fn capped_provider_extraction_never_certifies_a_unique_match() {
    let (_dir, e) = fixture();
    let mut source = PROVIDER.to_string();
    for n in 0..130 {
        source.push_str(&format!(
            "\n@app.get('/v1/extra{n}')\ndef extra{n}():\n    return {{}}\n"
        ));
    }
    e.replace_source("service.py", &source).unwrap();
    let p = publish(&e);
    assert_eq!(p.incomplete, 1);
    let r = read(&e);
    assert!(r.relations.iter().all(|r| r.resolution == "incomplete"));
    assert!(r.examined <= 512);
}
