use context_foundry::{
    Control, Engine, FoundryError, Strategy,
    fault::{self, Action, names},
    response::{self, Budget},
    store::ContextOptions,
    task_context,
};
use serde_json::{Value, json};
use std::process::Command;
const BIN: &str = env!("CARGO_BIN_EXE_foundry");
const SERVICE: &str = "from fastapi import FastAPI\napp = FastAPI()\n@app.get('/v1/items')\ndef items():\n    return {'items': ['book']}\n";
const CLIENT: &str = "import httpx\ndef fetch_items():\n    return httpx.get('http://catalog/v1/items').json()['products']\n";
const VERIFY: &str = "import requests\ndef test_items():\n    assert requests.get('http://catalog/v1/items').json()['products'] == ['book']\n";
struct Env {
    dir: tempfile::TempDir,
    engine: Engine,
    revision: u64,
    wid: String,
}
fn task(e: &Engine, c: Value) -> Result<Value, FoundryError> {
    e.task_context(
        &task_context::parse(
            json!({"workspace_id":e.workspace_id().unwrap(),"task_id":"contract","command":c}),
        )
        .unwrap(),
        &Control::unbounded(),
    )
}
fn fixture() -> Env {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("root")).unwrap();
    let mut engine =
        Engine::initialize(&dir.path().join("store"), &dir.path().join("root")).unwrap();
    for (p, s) in [
        ("service.py", SERVICE),
        ("client.py", CLIENT),
        ("test_contract.py", VERIFY),
        (
            "billing.py",
            "from fastapi import FastAPI\napp = FastAPI()\n@app.get('/v1/items')\ndef items():\n    return {'total': 0}\n",
        ),
    ] {
        engine.replace_source(p, s).unwrap();
        std::fs::write(dir.path().join("root").join(p), s).unwrap();
    }
    let config=json!({"v":1,"services":[{"id":"catalog","providers":["service.py"],"origins":["http://catalog"]},{"id":"billing","providers":["billing.py"],"origins":["http://billing"]}],"consumers":["client.py","test_contract.py"]}).to_string();
    engine.replace_source("boundary.json", &config).unwrap();
    std::fs::write(dir.path().join("root/boundary.json"), config).unwrap();
    engine
        .index_boundaries("boundary.json", &[], &Control::unbounded())
        .unwrap();
    let r = task(
        &engine,
        json!({"op":"create","title":"Repair catalog response contract"}),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    let edits = json!([
        {"op":"add","entry":{"id":"shape","kind":"requirement","text":"fetch_items expects products in the catalog response.","author":"operator","provenance":"sandbox task","source_links":[]}},
        {"op":"add","entry":{"id":"keep","kind":"constraint","text":"Keep GET /v1/items and the billing service unchanged.","author":"operator","provenance":"sandbox task","source_links":[]}},
        {"op":"add","entry":{"id":"tests","kind":"question","text":"test_items verifies the products response contract.","author":"operator","provenance":"sandbox task","source_links":[]}}
    ]);
    let revision = task(
        &engine,
        json!({"op":"apply","expected_revision":r,"edits":edits}),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    engine.refresh(&Control::unbounded()).unwrap();
    let wid = engine.workspace_id().unwrap();
    Env {
        dir,
        engine,
        revision,
        wid,
    }
}
fn compose(e: &Env) -> context_foundry::memory::MemoryContext {
    e.engine
        .context_candidates_with(
            "items",
            Strategy::Auto,
            &Control::unbounded(),
            &ContextOptions {
                task: Some(task_context::Scope {
                    workspace_id: &e.wid,
                    task_id: "contract",
                    expected_revision: e.revision,
                }),
                compose_evidence: true,
                boundary_config: Some("boundary.json"),
                ..Default::default()
            },
        )
        .unwrap()
}
fn report(text: &str) -> Value {
    serde_json::from_str(
        text.lines()
            .find_map(|l| l.strip_prefix("task-evidence: "))
            .unwrap(),
    )
    .unwrap()
}
#[test]
fn observed_execution_requires_fresh_proof_and_all_delivered_endpoints() {
    let e = fixture();
    let endpoint = |path: &str, body: &str, line: usize| json!({"path":path,"line":line,"symbol":"producer-label","hash":context_foundry::digest(body.as_bytes())});
    let artifact = json!({"v":1,"producer":"synthetic-test-trace","run":"case-1","command":["fixture-only"],"completed":true,"exit_code":0,"duration_ms":1,
    "test":endpoint("test_contract.py",VERIFY,2),"observations":[
        {"from":endpoint("test_contract.py",VERIFY,3),"to":endpoint("service.py",SERVICE,4),"kind":"calls","count":1},
        {"from":endpoint("test_contract.py",VERIFY,2),"to":endpoint("service.py",SERVICE,5),"kind":"covers","count":1}
    ]});
    let bytes = serde_json::to_vec(&artifact).unwrap();
    assert_eq!(e.engine.import_execution(&bytes).unwrap(), 2);
    let c = compose(&e);
    let pack = |tokens| {
        response::pack_enriched_context(&c, None, Budget::request(tokens), &response::stdout_bytes)
            .unwrap()
    };
    let full = pack(12000);
    assert_eq!(report(&full.text)["execution"]["delivered"], 2);
    assert!(full.text.contains("task-execution:"));
    assert!(full.text.contains(&context_foundry::digest(&bytes)));
    for budget in [1536, 2048, 4096] {
        let packed = pack(budget);
        assert!(packed.tokens <= budget);
        let delivered = packed
            .text
            .lines()
            .filter(|line| line.starts_with("task-execution:"))
            .count();
        assert_eq!(report(&packed.text)["execution"]["delivered"], delivered);
    }
    let mut invalid = artifact.clone();
    invalid["observations"][0]["count"] = 0.into();
    assert!(
        e.engine
            .import_execution(&serde_json::to_vec(&invalid).unwrap())
            .is_err()
    );
    assert_eq!(
        e.engine
            .graph("service.py", true, 1, 32)
            .unwrap()
            .edges
            .len(),
        2,
        "invalid replacement is atomic"
    );
    e.engine
        .replace_source("test_contract.py", &(VERIFY.to_owned() + "# changed\n"))
        .unwrap();
    assert!(
        e.engine
            .graph("service.py", true, 1, 32)
            .unwrap()
            .edges
            .is_empty()
    );
    let after = compose(&e);
    let packed = response::pack_enriched_context(
        &after,
        None,
        Budget::request(12000),
        &response::stdout_bytes,
    )
    .unwrap();
    assert!(!packed.text.contains("task-execution:"));
    assert_eq!(report(&packed.text)["execution"]["delivered"], 0);
}

#[test]
fn composition_keeps_caller_bodies_instead_of_expanding_file_outlines() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let mut engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
    engine
        .replace_source(
            "src/records.rs",
            include_str!("fixtures/agent-task/src/records.rs"),
        )
        .unwrap();
    let mut caller = include_str!("fixtures/agent-task/src/main.rs").to_owned();
    for i in 0..64 {
        caller.push_str(&format!(
            "\nfn unrelated_{i}() {{ println!(\"unrelated display configuration\"); }}\n"
        ));
    }
    engine.replace_source("src/main.rs", &caller).unwrap();
    engine.refresh(&Control::unbounded()).unwrap();
    let revision = task(
        &engine,
        json!({"op":"create","title":"Preserve parser callers"}),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    let revision = task(&engine, json!({"op":"apply","expected_revision":revision,"edits":[
        {"op":"add","entry":{"id":"goal","kind":"requirement","text":"Trim whitespace on both sides of the first = and reject an empty key. Preserve the borrowed Option interface and additional delimiters in values.","author":"operator","provenance":"fixture","source_links":[]}},
        {"op":"add","entry":{"id":"caller","kind":"question","text":"What do callers require from parse_record?","author":"operator","provenance":"fixture","source_links":[]}}
    ]})).unwrap()["revision"].as_u64().unwrap();
    let wid = engine.workspace_id().unwrap();
    let c = engine
        .context_candidates_with(
            "parse_record",
            Strategy::Auto,
            &Control::unbounded(),
            &ContextOptions {
                task: Some(task_context::Scope {
                    workspace_id: &wid,
                    task_id: "contract",
                    expected_revision: revision,
                }),
                compose_evidence: true,
                compact_evidence: true,
                ..Default::default()
            },
        )
        .unwrap();
    let packed =
        response::pack_enriched_context(&c, None, Budget::request(2048), &response::stdout_bytes)
            .unwrap();
    assert!(packed.tokens <= 2048);
    assert!(packed.text.contains("fn check()"), "{}", packed.text);
    assert!(packed.text.contains("Some((\"mode\", \"a=b\"))"));
    assert!(!packed.text.contains("unrelated_"));
    let source_text = packed
        .text
        .lines()
        .filter(|line| !line.starts_with("task-view:") && !line.starts_with("task-evidence:"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    for item in context_foundry::testkit::parse_v2(&source_text)
        .unwrap()
        .items
    {
        let h = context_foundry::store::HandleRef::parse(&item.handle).unwrap();
        if h.path == "src/main.rs" && item.form.is_none() {
            assert_eq!(
                item.body.trim_end_matches('\n'),
                caller[h.start as usize..h.end as usize].trim_end_matches('\n')
            );
        }
    }
    // Without imported compiler/execution evidence, source text does not assert
    // resolved callers, observed coverage or a satisfied task requirement.
    assert_eq!(report(&packed.text)["complete"], false);
    assert_eq!(report(&packed.text)["execution"]["delivered"], 0);
}

#[test]
fn compact_accounting_preserves_sources_relationships_and_protected_state() {
    let e = fixture();
    let verbose = compose(&e);
    let compact = e
        .engine
        .context_candidates_with(
            "items",
            Strategy::Auto,
            &Control::unbounded(),
            &ContextOptions {
                task: Some(task_context::Scope {
                    workspace_id: &e.wid,
                    task_id: "contract",
                    expected_revision: e.revision,
                }),
                compose_evidence: true,
                compact_evidence: true,
                boundary_config: Some("boundary.json"),
                ..Default::default()
            },
        )
        .unwrap();
    let pack = |c: &_| {
        response::pack_enriched_context(c, None, Budget::request(12000), &response::stdout_bytes)
            .unwrap()
    };
    let (verbose, compact) = (pack(&verbose), pack(&compact));
    let (v, c) = (report(&verbose.text), report(&compact.text));
    assert_eq!(c["v"], 2);
    assert_eq!(c["complete"], false);
    assert_eq!(c["constraints"], v["constraints"]);
    assert_eq!(c["verification"], v["verification"]);
    assert!(compact.tokens < verbose.tokens);
    // The report alone changes; original cited bodies, task view and both-end
    // relationship evidence remain byte-identical at this nonbinding allowance.
    let without_report = |text: &str| {
        text.lines()
            .filter(|l| !l.starts_with("task-evidence: "))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(without_report(&compact.text), without_report(&verbose.text));
    for (facet, value) in v["facets"].as_object().unwrap() {
        assert_eq!(c["facets"][facet], value["state"]);
    }
    for (i, source) in c["sources"].as_array().unwrap().iter().enumerate() {
        assert_eq!(source["item"], i + 1);
        assert!(source.get("handle").is_none());
    }
    assert!(
        e.engine
            .context_candidates_with(
                "items",
                Strategy::Auto,
                &Control::unbounded(),
                &ContextOptions {
                    compact_evidence: true,
                    ..Default::default()
                }
            )
            .is_err()
    );
}

#[test]
fn composed_mounted_routes_require_declaration_and_mount_bodies() {
    let e = fixture();
    let router = "from fastapi import FastAPI, APIRouter\napp = FastAPI()\nrouter = APIRouter(prefix='/v1')\n@router.get('/items')\ndef items():\n    return {'items': ['book']}\napp.include_router(router)\n";
    e.engine.replace_source("service.py", router).unwrap();
    e.engine
        .index_boundaries("boundary.json", &[], &Control::unbounded())
        .unwrap();
    let c = compose(&e);
    let mut delivered = false;
    for budget in [1500, 2500, 4000, 12000] {
        let Ok(p) = response::pack_enriched_context(
            &c,
            None,
            Budget::request(budget),
            &response::stdout_bytes,
        ) else {
            continue;
        };
        assert!(p.tokens <= budget);
        for line in p
            .text
            .lines()
            .filter_map(|l| l.strip_prefix("task-relationship: "))
        {
            let relationship: Value = serde_json::from_str(line).unwrap();
            assert_eq!(relationship["dependencies"].as_array().unwrap().len(), 2);
            assert!(p.text.contains("router = APIRouter(prefix='/v1')"));
            assert!(p.text.contains("app.include_router(router)"));
            delivered = true;
        }
    }
    assert!(delivered);
}

#[test]
fn cross_file_mount_proof_is_delivered_whole_or_omitted() {
    let e = fixture();
    e.engine.replace_source("service.py","from fastapi import FastAPI\nfrom routes import router\napp = FastAPI()\napp.include_router(router, prefix='/v1')\n").unwrap();
    e.engine.replace_source("routes.py","from fastapi import APIRouter\nrouter = APIRouter()\n@router.get('/items')\ndef items(): return {'items':['book']}\n").unwrap();
    e.engine.replace_source("boundary.json", &json!({"v":1,"services":[{"id":"catalog","providers":["service.py","routes.py"],"origins":["http://catalog"]}],"consumers":["client.py","test_contract.py"]}).to_string()).unwrap();
    e.engine
        .index_boundaries("boundary.json", &[], &Control::unbounded())
        .unwrap();
    let c = compose(&e);
    let mut count = 0;
    for tokens in [1500, 2500, 4000, 12000] {
        let Ok(p) = response::pack_enriched_context(
            &c,
            None,
            Budget::request(tokens),
            &response::stdout_bytes,
        ) else {
            continue;
        };
        let r = report(&p.text);
        for line in p
            .text
            .lines()
            .filter_map(|l| l.strip_prefix("task-relationship: "))
        {
            let relationship: Value = serde_json::from_str(line).unwrap();
            let deps = relationship["dependencies"].as_array().unwrap();
            assert_eq!(deps.len(), 3);
            for dependency in deps {
                let wanted =
                    context_foundry::store::HandleRef::parse(dependency.as_str().unwrap()).unwrap();
                assert!(r["sources"].as_array().unwrap().iter().any(|s| {
                    let h = context_foundry::store::HandleRef::parse(s["handle"].as_str().unwrap())
                        .unwrap();
                    s["representation"] == "body"
                        && h.path == wanted.path
                        && h.sha32 == wanted.sha32
                        && h.start <= wanted.start
                        && h.end >= wanted.end
                }));
            }
            count += 1;
        }
        assert!(p.tokens <= tokens);
    }
    assert!(count > 0);
}
#[test]
fn composes_task_and_multiservice_evidence_with_exact_citations_and_no_duplicate_spans() {
    let e = fixture();
    let c = compose(&e);
    let p =
        response::pack_enriched_context(&c, None, Budget::request(12000), &response::stdout_bytes)
            .unwrap();
    let r = report(&p.text);
    assert_eq!(r["complete"], false);
    assert_eq!(r["constraints"]["caller_assertions"], 1);
    for f in [
        "implementation",
        "consumers",
        "configuration",
        "verification",
    ] {
        assert_eq!(
            r["facets"][f]["state"], "source_candidates_delivered",
            "{f}: {r}"
        );
    }
    assert!(p.text.contains("task-relationship:"));
    assert!(r["boundary"]["delivered_relationships"].as_u64().unwrap() > 0);
    assert!(p.text.contains("\"exact_key\""));
    assert!(p.text.contains("Keep GET /v1/items"));
    let source = r["sources"].as_array().unwrap();
    let mut handles = std::collections::BTreeSet::new();
    for s in source {
        assert!(handles.insert(s["handle"].as_str().unwrap()));
    }
    assert_eq!(r["verification"], "test_candidates_only_not_executed");
    let relation = c.evidence.as_ref().unwrap().boundaries.as_ref().unwrap();
    assert!(
        relation
            .relations
            .iter()
            .all(|r| r.providers.len() == 1 && r.providers[0].evidence.path == "service.py")
    );
}
#[test]
fn one_allowance_includes_reports_and_partial_delivery_never_claims_coverage() {
    let e = fixture();
    let c = compose(&e);
    // Escaped JSON envelope is the MCP boundary, not raw text bytes.
    let boundary = |s: &str| {
        serde_json::to_vec(&json!({"content":[{"type":"text","text":s}]}))
            .unwrap()
            .len()
    };
    let mut successes = 0;
    let mut saw_partial = false;
    for tokens in [1, 512, 1000, 1500, 2000, 4000, 12000] {
        match response::pack_enriched_context(&c, None, Budget::request(tokens), &boundary) {
            Ok(p) => {
                successes += 1;
                assert!(p.tokens <= tokens);
                assert_eq!(p.tokens, response::count_tokens(&p.text));
                assert!(boundary(&p.text) <= response::BYTE_CAP);
                let r = report(&p.text);
                assert_eq!(r["complete"], false);
                saw_partial |= r["facets"]
                    .as_object()
                    .unwrap()
                    .values()
                    .any(|v| v["state"] == "omitted_budget" || v["state"] == "addresses_only");
            }
            Err(FoundryError::BudgetTooSmall { minimum_tokens }) => {
                assert!(minimum_tokens > tokens)
            }
            Err(e) => panic!("{e:?}"),
        }
    }
    assert!(successes > 0 && saw_partial);
}
#[test]
fn ordinary_context_is_unchanged_and_composition_requires_explicit_task() {
    let e = fixture();
    let before = e
        .engine
        .context_candidates_with(
            "items",
            Strategy::Auto,
            &Control::unbounded(),
            &ContextOptions::default(),
        )
        .unwrap();
    let _ = compose(&e);
    let after = e
        .engine
        .context_candidates_with(
            "items",
            Strategy::Auto,
            &Control::unbounded(),
            &ContextOptions::default(),
        )
        .unwrap();
    let pack = |c| {
        response::pack_enriched_context(c, None, Budget::request(2048), &response::stdout_bytes)
            .unwrap()
            .text
    };
    assert_eq!(pack(&before), pack(&after));
    assert!(!pack(&after).contains("task-evidence:"));
    assert!(
        e.engine
            .context_candidates_with(
                "items",
                Strategy::Auto,
                &Control::unbounded(),
                &ContextOptions {
                    compose_evidence: true,
                    ..Default::default()
                }
            )
            .is_err()
    );
}
#[test]
fn final_read_revalidates_source_and_task_revision_after_discovery() {
    let e = fixture();
    fault::arm(
        names::TASK_EVIDENCE_BEFORE_FINAL,
        0,
        Action::Call(Box::new(|ctx| {
            ctx.engine
                .unwrap()
                .replace_source("service.py", "# changed after discovery\n")
                .unwrap();
        })),
    );
    let c = compose(&e);
    fault::disarm_all();
    assert!(
        !c.batch
            .items
            .iter()
            .any(|i| i.handle.as_ref().unwrap().path == "service.py")
    );
    assert!(
        c.evidence
            .as_ref()
            .unwrap()
            .boundaries
            .as_ref()
            .unwrap()
            .relations
            .iter()
            .all(|r| r.resolution == "incomplete")
    );
    let revision = e.revision;
    fault::arm(
        names::TASK_EVIDENCE_BEFORE_FINAL,
        0,
        Action::Call(Box::new(move |ctx| {
            task(ctx.engine.unwrap(),json!({"op":"apply","expected_revision":revision,"edits":[{"op":"add","entry":{"id":"changed","kind":"constraint","text":"New constraint","author":"operator","provenance":"concurrent edit","source_links":[]}}]})).unwrap();
        })),
    );
    let result = e.engine.context_candidates_with(
        "items",
        Strategy::Auto,
        &Control::unbounded(),
        &ContextOptions {
            task: Some(task_context::Scope {
                workspace_id: &e.wid,
                task_id: "contract",
                expected_revision: e.revision,
            }),
            compose_evidence: true,
            ..Default::default()
        },
    );
    fault::disarm_all();
    assert!(matches!(result, Err(FoundryError::Conflict(_))));
}
#[test]
fn ambiguous_relationships_are_exposed_and_missing_tests_remain_missing() {
    let mut e = fixture();
    e.engine
        .replace_source(
            "service.py",
            &format!("{SERVICE}\n@app.get('/v1/items')\ndef duplicate():\n    return {{}}\n"),
        )
        .unwrap();
    e.engine.delete_source("test_contract.py").unwrap();
    e.engine
        .index_boundaries("boundary.json", &[], &Control::unbounded())
        .unwrap();
    e.engine.refresh(&Control::unbounded()).unwrap();
    let c = compose(&e);
    let p =
        response::pack_enriched_context(&c, None, Budget::request(12000), &response::stdout_bytes)
            .unwrap();
    let r = report(&p.text);
    assert_eq!(r["facets"]["verification"]["state"], "missing");
    assert_eq!(r["boundary"]["resolution"]["ambiguous"], 1);
}
#[test]
fn cli_composition_supports_a_sandbox_contract_patch_and_test() {
    let e = fixture();
    let root = e.dir.path().join("root");
    let store = e.dir.path().join("store");
    let revision = e.revision;
    drop(e.engine);
    let out = Command::new(BIN)
        .arg("--store")
        .arg(&store)
        .args([
            "context",
            "items",
            "--task",
            "contract",
            "--task-revision",
            &revision.to_string(),
            "--compose-evidence",
            "--compact-evidence",
            "--boundary-config",
            "boundary.json",
            "--tokens",
            "12000",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(report(&text)["v"], 2);
    assert!(text.contains("['products']"));
    assert!(text.contains("'items': ['book']"));
    assert!(text.contains("test_contract.py#"));
    // Execute the selected provider, consumer and test with a deterministic
    // in-process transport adapter. This verifies a contract patch, not HTTP
    // deployment or an LLM's autonomous patch quality.
    let harness = r#"import sys, types, runpy
routes = {}
class App:
    def get(self, path):
        def register(f):
            routes[path] = f
            return f
        return register
sys.modules['fastapi'] = types.SimpleNamespace(FastAPI=App)
runpy.run_path('service.py')
class Response:
    def __init__(self, value): self.value = value
    def json(self): return self.value
def get(url): return Response(routes[url.removeprefix('http://catalog')]())
sys.modules['httpx'] = types.SimpleNamespace(get=get)
sys.modules['requests'] = types.SimpleNamespace(get=get)
assert runpy.run_path('client.py')['fetch_items']() == ['book']
runpy.run_path('test_contract.py')['test_items']()
"#;
    let run = || {
        Command::new("python3")
            .args(["-c", harness])
            .current_dir(&root)
            .output()
            .unwrap()
    };
    assert!(!run().status.success());
    let billing = std::fs::read(root.join("billing.py")).unwrap();
    std::fs::write(
        root.join("service.py"),
        SERVICE.replace("'items':", "'products':"),
    )
    .unwrap();
    assert!(run().status.success());
    assert_eq!(billing, std::fs::read(root.join("billing.py")).unwrap());
}
#[tokio::test]
async fn mcp_composition_is_advertised_only_when_enabled_and_uses_real_result_budget() {
    use rmcp::{ServiceExt, model::CallToolRequestParams, transport::TokioChildProcess};
    let e = fixture();
    let store = e.dir.path().join("store");
    let root = e.dir.path().join("root");
    let revision = e.revision;
    drop(e.engine);
    for enabled in [false, true] {
        let mut cmd = tokio::process::Command::new(BIN);
        cmd.arg("--store")
            .arg(&store)
            .arg("mcp")
            .arg("--root")
            .arg(&root);
        if enabled {
            cmd.arg("--experimental-task-context");
        }
        let client = ().serve(TokioChildProcess::new(cmd).unwrap()).await.unwrap();
        let catalog = client.list_tools(None).await.unwrap();
        let tool = catalog.tools.iter().find(|t| t.name == "context").unwrap();
        assert_eq!(
            tool.input_schema["properties"]
                .get("compose_evidence")
                .is_some(),
            enabled
        );
        let indexed = client
            .call_tool(
                CallToolRequestParams::new("index").with_arguments(
                    json!({"boundary":{"config":"boundary.json"}})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
            )
            .await
            .unwrap();
        assert_eq!(indexed.is_error == Some(true), !enabled, "{indexed:?}");
        assert_eq!(
            tool.input_schema["properties"]
                .get("compact_evidence")
                .is_some(),
            enabled
        );
        let args = json!({"query":"items","task_id":"contract","task_revision":revision,"compose_evidence":true,"compact_evidence":true,"boundary_config":"boundary.json","tokens":4000});
        let result = client
            .call_tool(
                CallToolRequestParams::new("context")
                    .with_arguments(args.as_object().unwrap().clone()),
            )
            .await
            .unwrap();
        assert_eq!(result.is_error == Some(true), !enabled, "{result:?}");
        if enabled {
            let rmcp::model::ContentBlock::Text(t) = &result.content[0] else {
                panic!()
            };
            assert!(response::count_tokens(&t.text) <= 4000);
            assert!(serde_json::to_vec(&result).unwrap().len() <= response::BYTE_CAP);
            assert_eq!(report(&t.text)["complete"], false);
            assert_eq!(report(&t.text)["v"], 2);
        }
        client.cancel().await.unwrap();
    }
}

#[test]
fn derived_vocabulary_discovers_source_and_boundary_neighbors_without_promoting_claims() {
    let mut e = fixture();
    let handle = context_foundry::SourceHandle {
        workspace_id: e.wid.clone(),
        path: "service.py".into(),
        sha256: context_foundry::digest(SERVICE.as_bytes()),
        start: 0,
        end: SERVICE.len() as u64,
    }
    .to_v2();
    let brief = json!({"cited_spans":[handle],"recipe_sha256":"a".repeat(64),"model":"synthetic/no-provider","artifact_sha256":"b".repeat(64),"prompt_sha256":"c".repeat(64),"generated_at_unix_ms":1,"purpose":[{"text":"quartzfulfillment","citations":[0],"identifiers":[]}],"behavior":[{"text":"Return catalog items.","citations":[0],"identifiers":[]}],"provides":[],"consumes":[],"constraints":[],"tests":[],"unknowns":["Synthetic discovery fixture"]});
    let request = context_foundry::brief::parse(
        &serde_json::to_vec(&json!({"workspace_id":e.wid,"command":{"op":"put","brief":brief}}))
            .unwrap(),
    )
    .unwrap();
    e.engine.brief(&request, &Control::unbounded()).unwrap();
    // A separate discovery-only task leaves no lexical requirement seeds.
    task(
        &e.engine,
        json!({"op":"forget","expected_revision":e.revision}),
    )
    .unwrap();
    e.revision = task(
        &e.engine,
        json!({"op":"create","title":"quartzfulfillment"}),
    )
    .unwrap()["revision"]
        .as_u64()
        .unwrap();
    e.engine.refresh(&Control::unbounded()).unwrap();
    let c = e
        .engine
        .context_candidates_with(
            "quartzfulfillment",
            Strategy::Search,
            &Control::unbounded(),
            &ContextOptions {
                task: Some(task_context::Scope {
                    workspace_id: &e.wid,
                    task_id: "contract",
                    expected_revision: e.revision,
                }),
                compose_evidence: true,
                boundary_config: Some("boundary.json"),
                briefs: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(c.briefs.len(), 1);
    assert!(
        c.batch
            .items
            .iter()
            .any(|i| i.handle.as_ref().unwrap().path == "client.py")
    );
    let p =
        response::pack_enriched_context(&c, None, Budget::request(12000), &response::stdout_bytes)
            .unwrap();
    assert_eq!(report(&p.text)["orientation"]["derived_briefs"], 1);
    assert!(p.text.contains("\"kind\":\"derived\""));
    assert_eq!(report(&p.text)["complete"], false);
    e.engine
        .replace_source("service.py", "# source replaced\n")
        .unwrap();
    let c = e
        .engine
        .context_candidates_with(
            "quartzfulfillment",
            Strategy::Search,
            &Control::unbounded(),
            &ContextOptions {
                task: Some(task_context::Scope {
                    workspace_id: &e.wid,
                    task_id: "contract",
                    expected_revision: e.revision,
                }),
                compose_evidence: true,
                boundary_config: Some("boundary.json"),
                briefs: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(c.briefs.is_empty());
}

#[tokio::test]
async fn watched_boundary_maintenance_refreshes_one_changed_file_and_pauses_explicitly() {
    use rmcp::{ServiceExt, model::CallToolRequestParams, transport::TokioChildProcess};
    use std::time::{Duration, Instant};
    let e = fixture();
    let store = e.dir.path().join("store");
    let root = e.dir.path().join("root");
    drop(e.engine);
    let mut cmd = tokio::process::Command::new(BIN);
    cmd.arg("--store")
        .arg(&store)
        .arg("mcp")
        .arg("--root")
        .arg(&root)
        .args(["--experimental-task-context", "--watch"]);
    let client = ().serve(TokioChildProcess::new(cmd).unwrap()).await.unwrap();
    let decode = |result: rmcp::model::CallToolResult| -> serde_json::Value {
        assert_ne!(result.is_error, Some(true), "{result:?}");
        let rmcp::model::ContentBlock::Text(t) = &result.content[0] else {
            panic!()
        };
        serde_json::from_str(&t.text).unwrap()
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let status = decode(
            client
                .call_tool(CallToolRequestParams::new("status"))
                .await
                .unwrap(),
        );
        assert!(
            status["watch"].get("boundary").is_none(),
            "watching alone never enables extraction"
        );
        if status["watch"]["phase"] == "idle" {
            break;
        }
        assert!(Instant::now() < deadline, "{status}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let request = |maintain| {
        CallToolRequestParams::new("index").with_arguments(
            json!({"boundary":{"config":"boundary.json","maintain":maintain}})
                .as_object()
                .unwrap()
                .clone(),
        )
    };
    decode(client.call_tool(request(true)).await.unwrap());
    std::fs::write(
        root.join("client.py"),
        CLIENT.replace("/v1/items", "/v1/missing"),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let status = decode(
            client
                .call_tool(CallToolRequestParams::new("status"))
                .await
                .unwrap(),
        );
        let b = &status["watch"]["boundary"];
        if b["phase"] == "waiting"
            && b["parsed"].as_u64().unwrap_or(0) == 1
            && b["completed_source_revision"] == status["source_revision"]
        {
            break;
        }
        assert!(Instant::now() < deadline, "{status}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let once = || {
        CallToolRequestParams::new("index").with_arguments(
            json!({"boundary":{"config":"boundary.json"}})
                .as_object()
                .unwrap()
                .clone(),
        )
    };
    assert_eq!(
        decode(client.call_tool(once()).await.unwrap())["parsed"],
        0,
        "watcher already published the edit"
    );
    decode(client.call_tool(request(false)).await.unwrap());
    let before = decode(
        client
            .call_tool(CallToolRequestParams::new("status"))
            .await
            .unwrap(),
    )["source_revision"]
        .clone();
    std::fs::write(root.join("client.py"), CLIENT).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let status = decode(
            client
                .call_tool(CallToolRequestParams::new("status"))
                .await
                .unwrap(),
        );
        assert_eq!(status["watch"]["boundary"]["phase"], "paused");
        if status["source_revision"] != before && status["watch"]["phase"] == "idle" {
            break;
        }
        assert!(Instant::now() < deadline, "{status}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(decode(client.call_tool(once()).await.unwrap())["parsed"], 1);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn admitted_root_composition_uses_one_primary_task_and_preserves_root_identity() {
    use rmcp::{ServiceExt, model::CallToolRequestParams, transport::TokioChildProcess};
    let e = fixture();
    let reference = tempfile::tempdir().unwrap();
    let root = reference.path().join("root");
    let store = reference.path().join("store");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("client.py"), CLIENT).unwrap();
    let mut engine = Engine::initialize(&store, &root).unwrap();
    engine.index(&root, &Control::unbounded()).unwrap();
    let reference_id = engine.workspace_id().unwrap();
    task(
        &engine,
        json!({"op":"create","title":"FOREIGN_TASK_MUST_NOT_APPEAR"}),
    )
    .unwrap();
    drop(engine);
    drop(e.engine);
    let mut cmd = tokio::process::Command::new(BIN);
    cmd.arg("--store")
        .arg(e.dir.path().join("store"))
        .arg("mcp")
        .arg("--root")
        .arg(e.dir.path().join("root"))
        .arg("--reference")
        .arg(format!("{}={}", root.display(), store.display()))
        .arg("--experimental-task-context");
    let client = ().serve(TokioChildProcess::new(cmd).unwrap()).await.unwrap();
    for roots in [json!(["primary", "ref1"]), json!(["ref1"])] {
        let result=client.call_tool(CallToolRequestParams::new("context").with_arguments(json!({"query":"fetch_items","task_id":"contract","task_revision":e.revision,"compose_evidence":true,"roots":roots,"tokens":12000}).as_object().unwrap().clone())).await.unwrap();
        if roots.as_array().unwrap().len() == 1 {
            assert_eq!(result.is_error, Some(true));
            continue;
        }
        assert_ne!(result.is_error, Some(true), "{result:?}");
        let rmcp::model::ContentBlock::Text(t) = &result.content[0] else {
            panic!()
        };
        let r = report(&t.text);
        assert_eq!(r["scope"], "admitted_roots_primary_task");
        assert_eq!(r["roots"], json!([e.wid, reference_id]));
        assert_eq!(t.text.matches("task-view: ").count(), 1);
        assert!(!t.text.contains("FOREIGN_TASK_MUST_NOT_APPEAR"));
        let sources = r["sources"].as_array().unwrap();
        for wid in [&e.wid, &reference_id] {
            assert!(
                sources.iter().any(|s| s["handle"]
                    .as_str()
                    .is_some_and(|h| h.contains(&wid[..16]) && h.contains("client.py"))),
                "{r}"
            );
        }
        assert!(response::count_tokens(&t.text) <= 12000);
        assert!(serde_json::to_vec(&result).unwrap().len() <= response::BYTE_CAP);
    }
    client.cancel().await.unwrap();
}
