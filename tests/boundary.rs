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
fn flask_blueprint_prefix_override_methods_and_source_freshness() {
    let (_dir, e) = fixture();
    let body = "from flask import Flask, Blueprint\napp = Flask(__name__)\napi = Blueprint('api', __name__, url_prefix='/old')\n@api.route('/items', methods=['GET', 'POST'])\ndef items(): return {'items': []}\napp.register_blueprint(api, url_prefix='/v1')\n";
    e.replace_source("service.py", body).unwrap();
    publish(&e);
    let relations = read(&e);
    assert!(relations.incomplete_paths.is_empty(), "{relations:?}");
    assert!(
        relations
            .relations
            .iter()
            .all(|r| r.resolution == "exact_key" && r.providers.len() == 1)
    );
    let proof = &relations.relations[0].providers[0];
    assert_eq!(
        proof.dependencies.len(),
        3,
        "blueprint, registration and app declaration"
    );
    assert_eq!(publish(&e).parsed, 0);
    for changed in [body.replace("url_prefix='/v1'","url_prefix=prefix"),
        body.replace("methods=['GET', 'POST']","methods=verbs"),
        format!("{body}configure(api)\n"),
        body.replace("'/items'","'/items/<uuid:id>'"),
        body.replace("app.register_blueprint(api, url_prefix='/v1')", "app.register_blueprint(api, url_prefix='/v1')\napp.add_url_rule('/dynamic', view_func=items)")] {
        e.replace_source("service.py",&changed).unwrap();
        assert!(read(&e).relations.iter().all(|r|r.resolution=="incomplete"));
        publish(&e);
        assert!(read(&e).incomplete_paths.iter().any(|p|p=="service.py"));
    }
    e.replace_source("service.py", body).unwrap();
    publish(&e);
    assert!(
        read(&e)
            .relations
            .iter()
            .all(|r| r.resolution == "exact_key")
    );
}

#[test]
#[ignore = "explicit runtime cross-check requires Flask 3.1.2; indexing never executes this"]
fn real_flask_blueprint_contract() {
    let (_dir, e) = fixture();
    let body = "from flask import Flask, Blueprint\napp = Flask(__name__)\napi = Blueprint('api', __name__, url_prefix='/old')\n@api.route('/items/<int:item>', methods=['GET', 'POST'])\ndef items(item): return {'item': item}\napp.register_blueprint(api, url_prefix='/v1')\n";
    e.replace_source("service.py", body).unwrap();
    e.replace_source(
        "client.py",
        "import requests\ndef fetch(): return requests.get('http://catalog/v1/items/42')\n",
    )
    .unwrap();
    publish(&e);
    let r = read(&e);
    assert!(r.relations.iter().any(|r| {
        r.consumer
            .key
            .as_ref()
            .is_some_and(|k| k.path == "/v1/items/42")
            && r.resolution == "route_template"
    }));
    let program = format!(
        "{body}\nfrom importlib.metadata import version\nc=app.test_client()\nassert c.get('/v1/items/42').json == {{'item':42}}\nassert c.post('/v1/items/42').status_code == 200\nassert c.head('/v1/items/42').status_code == 200\nassert c.get('/old/items/42').status_code == 404\nassert c.get('/v1/items/-1').status_code == 404\nprint(version('flask'))\n"
    );
    let output = Command::new(
        std::env::var("FOUNDRY_FRAMEWORK_PYTHON").unwrap_or_else(|_| "python3".into()),
    )
    .args(["-I", "-B", "-c", &program])
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    println!("Flask {}", String::from_utf8_lossy(&output.stdout).trim());
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
    e.replace_source(
        "service.py",
        &PROVIDER.replace("/v1/items", "/v1/{item:custom}"),
    )
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
    assert!(
        Engine::upgrade_store(
            &store,
            context_foundry::SCHEMA_VERSION,
            &Control::unbounded()
        )
        .is_err()
    );
    fault::disarm_all();
    assert!(matches!(
        Engine::open_existing(&store),
        Err(context_foundry::FoundryError::UpgradeRequired { .. })
    ));
    Engine::upgrade_store(
        &store,
        context_foundry::SCHEMA_VERSION,
        &Control::unbounded(),
    )
    .unwrap();
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

#[test]
fn template_routes_discover_literal_consumers_from_either_end_without_hiding_overlap() {
    let (dir, e) = fixture();
    let route = PROVIDER.replace("/v1/items", "/v1/{item}");
    e.replace_source("service.py", &route).unwrap();
    publish(&e);
    for seed in ["service.py", "client.py"] {
        let r = e
            .boundary_relations("boundary.json", &[seed.into()], &Control::unbounded())
            .unwrap();
        assert_eq!(
            r.relations.len(),
            2,
            "provider seeds must discover literal consumers"
        );
        assert!(
            r.relations
                .iter()
                .all(|r| r.resolution == "route_template" && r.providers.len() == 1)
        );
        assert!(r.examined <= 512);
    }
    // A literal and a template can overlap: declaration/runtime precedence is
    // not inferred from this static evidence.
    e.replace_source("other.py", PROVIDER).unwrap();
    publish(&e);
    assert!(
        read(&e)
            .relations
            .iter()
            .all(|r| r.resolution == "ambiguous" && r.providers.len() == 2)
    );
    e.replace_source("other.py", "from fastapi import FastAPI\napp = FastAPI()\n")
        .unwrap();
    publish(&e);
    e.replace_source("client.py", &CLIENT.replace("/v1/items?limit=3", "/v1/"))
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
        "unresolved"
    );
    drop(e);
    let e = Engine::open_existing(&dir.path().join("store")).unwrap();
    assert_eq!(
        read(&e)
            .relations
            .iter()
            .find(|r| r.consumer.evidence.path == "test_contract.py")
            .unwrap()
            .resolution,
        "route_template"
    );
    // Unsupported converters and partial-segment syntax invalidate certainty.
    for path in ["/v1/{item:custom}", "/v1/prefix-{item}", "/{item}/{item}"] {
        e.replace_source("service.py", &PROVIDER.replace("/v1/items", path))
            .unwrap();
        publish(&e);
        let r = read(&e);
        assert!(r.incomplete_paths.contains(&"service.py".to_string()));
        assert!(r.relations.iter().all(|r| r.resolution == "incomplete"));
    }
}

#[test]
fn builtin_converters_match_only_their_declared_path_language() {
    let (_dir, e) = fixture();
    for (route, value, matched) in [
        ("{id:int}", "42", true),
        ("{id:int}", "-42", false),
        ("{id:int}", "word", false),
        ("{n:float}", "12.5", true),
        ("{n:float}", "12", true),
        ("{n:float}", "1e3", false),
        ("{id:uuid}", "A0123456-7890-abcd-ef01-234567890abc", true),
        ("{id:uuid}", "A01234567890abcdef01234567890abc", true),
        ("{id:uuid}", "A01234567890-abcd-ef01234567890abc", true),
        ("{id:uuid}", "A0123456-7890-abcd-ef01-234567890abz", false),
        ("{tail:path}", "a/b/c", true),
        ("{tail:path}", "", true),
        ("{name:str}", "", false),
        ("{name:str}", "a/b", false),
    ] {
        e.replace_source(
            "service.py",
            &PROVIDER.replace("/v1/items", &format!("/v1/{route}")),
        )
        .unwrap();
        e.replace_source(
            "client.py",
            &CLIENT.replace("/v1/items?limit=3", &format!("/v1/{value}")),
        )
        .unwrap();
        publish(&e);
        let r = e
            .boundary_relations(
                "boundary.json",
                &["client.py".into()],
                &Control::unbounded(),
            )
            .unwrap();
        let row = r
            .relations
            .iter()
            .find(|r| r.consumer.evidence.path == "client.py")
            .unwrap();
        assert_eq!(
            row.resolution,
            if matched {
                "route_template"
            } else {
                "unresolved"
            },
            "{route} / {value}"
        );
    }
    for route in ["/{tail:path}/suffix", "/{id:custom}"] {
        e.replace_source("service.py", &PROVIDER.replace("/v1/items", route))
            .unwrap();
        assert_eq!(publish(&e).incomplete, 1);
    }
    e.replace_source("service.py", PROVIDER).unwrap();
    e.replace_source(
        "client.py",
        &CLIENT.replace("/v1/items?limit=3", "/v1/%69tems"),
    )
    .unwrap();
    publish(&e);
    assert!(
        read(&e)
            .relations
            .iter()
            .any(|r| r.consumer.evidence.path == "client.py" && r.resolution == "unsupported")
    );
}

#[test]
fn static_router_mounts_cite_prefixes_and_refuse_dynamic_or_early_registration() {
    let (dir, e) = fixture();
    let router = "from fastapi import FastAPI, APIRouter\napp = FastAPI()\nrouter = APIRouter(prefix='/v1')\n@router.get('/items')\ndef items():\n    return {'items': []}\napp.include_router(router)\n";
    e.replace_source("service.py", router).unwrap();
    assert_eq!(publish(&e).incomplete, 0);
    let r = read(&e);
    assert_eq!(r.relations.len(), 2);
    for row in &r.relations {
        assert_eq!(row.resolution, "exact_key");
        let dependencies = &row.providers[0].dependencies;
        assert_eq!(dependencies.len(), 2);
        let text: Vec<_> = dependencies
            .iter()
            .map(|s| &router[s.start as usize..s.end as usize])
            .collect();
        assert!(text[0].contains("APIRouter(prefix='/v1')"));
        assert_eq!(text[1], "app.include_router(router)");
    }
    let prefixed = router.replace("prefix='/v1'", "prefix=''").replace(
        "include_router(router)",
        "include_router(router, prefix='/v1')",
    );
    e.replace_source("service.py", &prefixed).unwrap();
    assert!(
        read(&e)
            .relations
            .iter()
            .all(|r| r.resolution == "incomplete")
    );
    assert_eq!(publish(&e).parsed, 1);
    assert!(
        read(&e)
            .relations
            .iter()
            .all(|r| r.resolution == "exact_key")
    );
    for changed in [
        router.replace("prefix='/v1'", "prefix=PREFIX"),
        router.replace("@router.get('/items')", "@router.get('items')"),
        router.replace(
            "include_router(router)",
            "include_router(router, prefix=PREFIX)",
        ),
        router.replace("@router.get", "app.include_router(router)\n@router.get"),
        router.replace("app.include_router(router)\n", ""),
        router.replace("include_router(router)", "mount('/v1', router)"),
        format!("{router}app = FastAPI()\n"),
        format!("{router}app.add_api_route('/dynamic', items)\n"),
        router.replace(
            "@router.get",
            "router = APIRouter(prefix='/other')\n@router.get",
        ),
    ] {
        e.replace_source("service.py", &changed).unwrap();
        assert_eq!(publish(&e).incomplete, 1, "{changed}");
        assert!(
            read(&e)
                .relations
                .iter()
                .all(|r| r.resolution == "incomplete")
        );
    }
    e.replace_source("service.py", router).unwrap();
    publish(&e);
    drop(e);
    let e = Engine::open_existing(&dir.path().join("store")).unwrap();
    assert_eq!(read(&e).relations[0].providers[0].dependencies.len(), 2);
    assert_eq!(publish(&e).parsed, 0);
}

#[test]
#[ignore = "requires explicitly installed FastAPI/Starlette/httpx; no dependency downloads"]
fn real_fastapi_converter_and_mount_fixture_matches_published_evidence() {
    let (_dir, e) = fixture();
    for (route, path, matched) in [
        ("{id:int}", "42", true),
        ("{id:int}", "-42", false),
        ("{n:float}", "1.25", true),
        ("{n:float}", "1e3", false),
        ("{id:uuid}", "0123456789abcdef0123456789abcdef", true),
        ("{id:uuid}", "01234567-89abcdef-0123456789abcdef", true),
        ("{id:uuid}", "bad", false),
        ("{tail:path}", "a/b/c", true),
        ("{tail:path}", "", true),
    ] {
        let source = format!(
            "from fastapi import FastAPI, APIRouter\napp = FastAPI()\nrouter = APIRouter(prefix='/v1')\n@router.get('/{route}')\ndef endpoint(): return {{'ok': True}}\napp.include_router(router, prefix='/api')\n"
        );
        let url = format!("/api/v1/{path}");
        e.replace_source("service.py", &source).unwrap();
        e.replace_source(
            "client.py",
            &format!("import httpx\nhttpx.get('http://catalog{url}')\n"),
        )
        .unwrap();
        assert_eq!(publish(&e).incomplete, 0);
        let row = e
            .boundary_relations(
                "boundary.json",
                &["client.py".into()],
                &Control::unbounded(),
            )
            .unwrap();
        assert_eq!(
            row.relations[0].resolution,
            if matched {
                "route_template"
            } else {
                "unresolved"
            }
        );
        let script = format!(
            "{source}\nfrom fastapi.testclient import TestClient\nimport importlib.metadata as m\nresponse = TestClient(app).get({url:?}, follow_redirects=False)\nassert response.status_code == {}, response.status_code\nprint(m.version('fastapi'), m.version('starlette'))\n",
            if matched { 200 } else { 404 }
        );
        let out = Command::new("python3")
            .args(["-c", &script])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{route} / {path}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        eprintln!(
            "FastAPI/Starlette runtime: {}",
            String::from_utf8_lossy(&out.stdout).trim()
        );
    }
}

#[test]
fn nested_router_prefixes_require_ordered_complete_registration_chains() {
    let (_dir, e) = fixture();
    let source = "from fastapi import FastAPI, APIRouter\napp = FastAPI()\nouter = APIRouter(prefix='/v1')\ninner = APIRouter(prefix='/items')\n@inner.get('')\ndef items(): return {'items': []}\nouter.include_router(inner)\napp.include_router(outer)\n";
    e.replace_source("service.py", source).unwrap();
    assert_eq!(publish(&e).incomplete, 0);
    for row in read(&e).relations {
        assert_eq!(row.resolution, "exact_key");
        assert_eq!(row.providers[0].dependencies.len(), 4);
        for dependency in &row.providers[0].dependencies {
            assert_eq!(
                dependency.sha256,
                context_foundry::digest(source.as_bytes())
            );
        }
    }
    for changed in [
        source.replace(
            "outer.include_router(inner)\napp.include_router(outer)",
            "app.include_router(outer)\nouter.include_router(inner)",
        ),
        source.replace("app.include_router(outer)", "inner.include_router(outer)"),
        format!("{source}app.include_router(inner)\n"),
        source.replace(
            "outer.include_router(inner)",
            "outer.include_router(inner, prefix=prefix)",
        ),
    ] {
        e.replace_source("service.py", &changed).unwrap();
        assert_eq!(publish(&e).incomplete, 1, "{changed}");
        assert!(
            read(&e)
                .relations
                .iter()
                .all(|r| r.resolution == "incomplete")
        );
    }
}

#[test]
fn cross_file_router_imports_invalidate_and_publish_the_dependent_service() {
    let (dir, e) = fixture();
    let importing = "from fastapi import FastAPI\nfrom other import router as api\napp = FastAPI()\napp.include_router(api, prefix='/v1')\n";
    let routes = "from fastapi import APIRouter\nrouter = APIRouter()\n@router.get('/items')\ndef items(): return {'items': []}\n";
    e.replace_source("service.py", importing).unwrap();
    e.replace_source("other.py", routes).unwrap();
    assert_eq!(publish(&e).incomplete, 0);
    for row in read(&e).relations {
        assert_eq!(row.resolution, "exact_key");
        assert_eq!(row.providers[0].evidence.path, "other.py");
        assert_eq!(row.providers[0].dependencies.len(), 3);
        assert!(
            row.providers[0]
                .dependencies
                .iter()
                .any(|s| s.path == "service.py"
                    && importing[s.start as usize..s.end as usize]
                        .starts_with("from other import"))
        );
    }
    assert_eq!(publish(&e).parsed, 0);
    e.replace_source("service.py", &importing.replace("'/v1'", "'/v2'"))
        .unwrap();
    assert!(
        read(&e)
            .relations
            .iter()
            .all(|r| r.resolution == "incomplete" && r.providers.is_empty())
    );
    let report = e
        .index_boundaries(
            "boundary.json",
            &["service.py".into()],
            &Control::unbounded(),
        )
        .unwrap();
    assert_eq!(report.parsed, 2);
    assert!(
        read(&e)
            .relations
            .iter()
            .all(|r| r.resolution == "unresolved")
    );
    e.replace_source("service.py", importing).unwrap();
    e.index_boundaries(
        "boundary.json",
        &["service.py".into()],
        &Control::unbounded(),
    )
    .unwrap();
    drop(e);
    let e = Engine::open_existing(&dir.path().join("store")).unwrap();
    assert!(
        read(&e)
            .relations
            .iter()
            .all(|r| r.resolution == "exact_key")
    );
    for changed in [
        format!("{importing}use(api)\n"),
        format!("{importing}app.include_router(api)\n"),
        importing.replace("router as api", "missing as api"),
        importing.replace("prefix='/v1'", "prefix=PREFIX"),
        importing.replace("app.include_router", "unknown.include_router"),
        importing.replace("app = FastAPI()\n", "") + "app = FastAPI()\n",
    ] {
        e.replace_source("service.py", &changed).unwrap();
        publish(&e);
        assert!(
            read(&e)
                .relations
                .iter()
                .all(|r| r.resolution == "incomplete"),
            "{changed}"
        );
    }
    e.delete_source("service.py").unwrap();
    assert!(
        read(&e)
            .relations
            .iter()
            .all(|r| r.resolution == "incomplete")
    );
}

#[test]
#[ignore = "requires installed FastAPI/Starlette/httpx; runs only a generated fixture"]
fn real_fastapi_cross_file_nested_router_contract() {
    let (dir, e) = fixture();
    let app = "from fastapi import FastAPI\nfrom other import outer\napp = FastAPI()\napp.include_router(outer, prefix='/v1')\n";
    let routes = "from fastapi import APIRouter\nouter = APIRouter()\ninner = APIRouter(prefix='/items')\n@inner.get('')\ndef items(): return {'items':['book']}\nouter.include_router(inner)\n";
    for (path, body) in [("service.py", app), ("other.py", routes)] {
        e.replace_source(path, body).unwrap();
        std::fs::write(dir.path().join("root").join(path), body).unwrap();
    }
    assert_eq!(publish(&e).incomplete, 0);
    for r in read(&e).relations {
        assert_eq!(r.resolution, "exact_key");
        assert_eq!(r.providers[0].dependencies.len(), 5);
    }
    let out=Command::new("python3").args(["-B","-c","from service import app\nfrom fastapi.testclient import TestClient\nimport importlib.metadata as m\nr=TestClient(app).get('/v1/items',follow_redirects=False)\nassert r.status_code==200 and r.json()=={'items':['book']}\nprint(m.version('fastapi'),m.version('starlette'))"]).current_dir(dir.path().join("root")).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("runtime {}", String::from_utf8_lossy(&out.stdout));
}

#[test]
fn relative_router_imports_require_a_configured_unambiguous_package_path() {
    let (_dir, e) = fixture();
    let config = |providers: &[&str]| {
        json!({"v":1,"services":[{"id":"catalog","providers":providers,"origins":["http://catalog"]}],"consumers":["client.py"]}).to_string()
    };
    e.replace_source("boundary.json", &config(&["pkg/app.py", "pkg/routes.py"]))
        .unwrap();
    let app = "from fastapi import FastAPI\nfrom .routes import router\napp = FastAPI()\napp.include_router(router, prefix='/v1')\n";
    let routes = "from fastapi import APIRouter\nrouter = APIRouter()\n@router.get('/items')\ndef items(): return {}\n";
    e.replace_source("pkg/app.py", app).unwrap();
    e.replace_source("pkg/routes.py", routes).unwrap();
    publish(&e);
    assert_eq!(read(&e).relations[0].resolution, "exact_key");
    e.replace_source("pkg/app.py", &app.replace("from .routes", "from ..routes"))
        .unwrap();
    publish(&e);
    assert_eq!(read(&e).relations[0].resolution, "incomplete");
    e.replace_source("pkg/app.py", app).unwrap();
    e.replace_source(
        "pkg/routes.py",
        &(String::from("from .app import app\n") + routes),
    )
    .unwrap();
    publish(&e);
    assert_eq!(
        read(&e).relations[0].resolution,
        "incomplete",
        "cyclic imports cannot certify the join"
    );
    e.replace_source("pkg/routes.py", routes).unwrap();
    e.replace_source("pkg/routes/__init__.py", PROVIDER)
        .unwrap();
    e.replace_source(
        "boundary.json",
        &config(&["pkg/app.py", "pkg/routes.py", "pkg/routes/__init__.py"]),
    )
    .unwrap();
    publish(&e);
    assert_eq!(
        read(&e).relations[0].resolution,
        "incomplete",
        "competing module layouts cannot certify the join"
    );
}
