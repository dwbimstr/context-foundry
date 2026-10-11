//! An explicitly run, owned fixture. Source indexing itself never starts Python.
use context_foundry::{Control, Engine};
use serde_json::Value;
use std::process::Command;

#[test]
#[ignore = "explicit Python 3.11+ runtime trace and CLI import cross-check"]
fn real_dynamic_dispatch_and_observed_lines_import_with_source_identity() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let service = "class Calculator:\n    def refund(self, amount):\n        if amount < 0:\n            raise ValueError('negative')\n        return amount * 2\n";
    let test = "from service import Calculator\ndef test_refund():\n    assert getattr(Calculator(), 'refund')(4) == 8\ntest_refund()\n";
    for (path, body) in [("service.py", service), ("test_refund.py", test)] {
        std::fs::write(root.join(path), body).unwrap();
    }
    let store = dir.path().join("store");
    let mut engine = Engine::initialize(&store, &root).unwrap();
    engine.index(&root, &Control::unbounded()).unwrap();
    let bytes = collect(
        &root,
        &["service.py", "test_refund.py"],
        "dynamic-refund",
        &engine,
    );
    let artifact: Value = serde_json::from_slice(&bytes).unwrap();
    let observations = artifact["observations"].as_array().unwrap();
    assert!(
        observations
            .iter()
            .any(|o| o["kind"] == "calls" && o["to"]["symbol"] == "Calculator.refund")
    );
    assert!(
        observations.iter().any(|o| o["kind"] == "covers"
            && o["to"]["path"] == "service.py"
            && o["to"]["line"] == 5)
    );
    assert!(
        !observations.iter().any(|o| o["kind"] == "covers"
            && o["to"]["path"] == "service.py"
            && o["to"]["line"] == 4),
        "unexecuted error branch is not observed coverage"
    );
    drop(engine);
    let path = dir.path().join("execution.json");
    std::fs::write(&path, &bytes).unwrap();
    let imported = Command::new(env!("CARGO_BIN_EXE_foundry"))
        .arg("--store")
        .arg(&store)
        .arg("import-execution")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let engine = Engine::open_existing(&store).unwrap();
    let graph = engine.graph("service.py", true, 1, 128).unwrap();
    assert!(
        graph
            .edges
            .iter()
            .any(|e| e.edge.kind == "calls" && e.edge.to.symbol == "Calculator.refund")
    );
    assert!(
        graph
            .edges
            .iter()
            .all(|e| e.execution.as_ref().unwrap().artifact_sha256
                == context_foundry::digest(&bytes))
    );
    println!(
        "EXECUTION_RESULT {}",
        serde_json::json!({"runtime":"CPython sys.settrace","observations":observations.len(),"imported":serde_json::from_slice::<Value>(&imported.stdout).unwrap(),"artifact_sha256":context_foundry::digest(&bytes),"dynamic_callee_observed":true,"unexecuted_branch_absent":true})
    );
}

fn collect(root: &std::path::Path, names: &[&str], run: &str, engine: &Engine) -> Vec<u8> {
    let script = r#"
import sys, pathlib, hashlib, json, time, runpy
root=pathlib.Path(sys.argv[1]).resolve()
names=json.loads(sys.argv[2])
original={name:(root/name).read_bytes() for name in names}
hashes={name:hashlib.sha256(body).hexdigest() for name,body in original.items()}
test={'path':'test_refund.py','line':2,'symbol':'test_refund','hash':hashes['test_refund.py']}
events={}
def endpoint(frame, line):
    p=pathlib.Path(frame.f_code.co_filename)
    if p.parent != root or p.name not in hashes: return None
    return {'path':p.name,'line':line,'symbol':frame.f_code.co_qualname,'hash':hashes[p.name]}
def record(frm,to,kind):
    key=json.dumps({'from':frm,'to':to,'kind':kind},sort_keys=True)
    events[key]=events.get(key,0)+1
def trace(frame,event,arg):
    target=endpoint(frame,frame.f_lineno)
    if target:
        if event=='line': record(test,target,'covers')
        if event=='call' and frame.f_back:
            caller=endpoint(frame.f_back,frame.f_back.f_lineno)
            if caller: record(caller,target,'calls')
    return trace
sys.path.insert(0,str(root))
started=time.monotonic()
sys.settrace(trace)
runpy.run_path(str(root/'test_refund.py'),run_name='__main__')
sys.settrace(None)
assert all((root/name).read_bytes()==body for name,body in original.items())
observations=[dict(json.loads(key),count=count) for key,count in sorted(events.items())]
print(json.dumps({'v':2,'producer':'cpython-sys-settrace','scope':test['path']+'::'+test['symbol'],'workspace_id':sys.argv[4],'source_revision':int(sys.argv[5]),'inputs':[{'path':name,'sha256':hashes[name]} for name in sorted(hashes)],'run':sys.argv[3],'command':[sys.executable,'-I','-B',*sys.argv], 'completed':True,'exit_code':0,'duration_ms':int((time.monotonic()-started)*1000),'test':test,'observations':observations}))
"#;
    let collector = root.parent().unwrap().join("collector.py");
    std::fs::write(&collector, script).unwrap();
    let output = Command::new(
        std::env::var("FOUNDRY_FRAMEWORK_PYTHON").unwrap_or_else(|_| "python3".into()),
    )
    .args(["-I", "-B"])
    .arg(&collector)
    .arg(root)
    .arg(serde_json::to_string(names).unwrap())
    .arg(run)
    .arg(engine.workspace_id().unwrap())
    .arg(engine.source_revision().unwrap().to_string())
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
#[ignore = "explicit Flask/CPython execution fixture; no workspace code runs during indexing"]
fn real_flask_request_observes_dynamic_callback_and_only_executed_test_paths() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let service = "class Calculator:\n    def refund(self, amount):\n        if amount < 0:\n            raise ValueError('negative')\n        return amount * 2\n";
    let routes = "from flask import Flask\nfrom service import Calculator\napp = Flask(__name__)\n@app.get('/refund/<int:amount>')\ndef refund(amount):\n    return {'value': getattr(Calculator(), 'refund')(amount)}\n";
    let test = "from routes import app\ndef test_refund():\n    response = app.test_client().get('/refund/4')\n    assert response.status_code == 200 and response.json == {'value': 8}\ntest_refund()\n";
    for (name, body) in [
        ("service.py", service),
        ("routes.py", routes),
        ("test_refund.py", test),
    ] {
        std::fs::write(root.join(name), body).unwrap();
    }
    let store = dir.path().join("store");
    let mut engine = Engine::initialize(&store, &root).unwrap();
    engine.index(&root, &Control::unbounded()).unwrap();
    let bytes = collect(
        &root,
        &["service.py", "routes.py", "test_refund.py"],
        "flask-dynamic-refund",
        &engine,
    );
    let artifact: Value = serde_json::from_slice(&bytes).unwrap();
    let observations = artifact["observations"].as_array().unwrap();
    assert!(observations.iter().any(|o| o["kind"] == "calls"
        && o["from"]["path"] == "routes.py"
        && o["to"]["symbol"] == "Calculator.refund"));
    assert!(
        observations.iter().any(|o| o["kind"] == "covers"
            && o["to"]["path"] == "routes.py"
            && o["to"]["line"] == 6)
    );
    assert!(
        !observations.iter().any(|o| o["kind"] == "covers"
            && o["to"]["path"] == "service.py"
            && o["to"]["line"] == 4)
    );
    engine.import_execution(&bytes).unwrap();
    drop(engine);
    let engine = Engine::open_existing(&store).unwrap();
    let graph = engine.graph("service.py", true, 1, 128).unwrap();
    let dynamic = graph
        .edges
        .iter()
        .find(|e| {
            e.edge.kind == "calls"
                && e.edge.from.path == "routes.py"
                && e.edge.to.symbol == "Calculator.refund"
        })
        .unwrap();
    let receipt = dynamic.execution.as_ref().unwrap();
    assert_eq!(receipt.test.path, "test_refund.py");
    assert_eq!(receipt.artifact_sha256, context_foundry::digest(&bytes));
    engine
        .replace_source("routes.py", "def refund(amount): return {'value': 0}\n")
        .unwrap();
    assert!(
        engine
            .graph("service.py", true, 1, 128)
            .unwrap()
            .edges
            .is_empty(),
        "changing an intermediate route invalidates observations whose endpoints did not change"
    );
    engine.replace_source("routes.py", routes).unwrap();
    let fresh = collect(
        &root,
        &["service.py", "routes.py", "test_refund.py"],
        "flask-refreshed",
        &engine,
    );
    engine.import_execution(&fresh).unwrap();
    assert!(
        !engine
            .graph("service.py", true, 1, 128)
            .unwrap()
            .edges
            .is_empty()
    );
    engine
        .replace_source("test_refund.py", &format!("{test}# changed test\n"))
        .unwrap();
    assert!(
        engine
            .graph("service.py", true, 1, 128)
            .unwrap()
            .edges
            .is_empty(),
        "changed observing test invalidates all observations after restart"
    );
    println!(
        "EXECUTION_RESULT {}",
        serde_json::json!({"runtime":"Flask TestClient + CPython sys.settrace","observations":observations.len(),"artifact_sha256":context_foundry::digest(&bytes),"dynamic_callee_observed":true,"unexecuted_branch_absent":true,"changed_test_suppresses_evidence":true})
    );
}

// Synthetic import lifecycle checks; these never run the command in a receipt.
#[test]
fn snapshot_execution_collections_coexist_reject_atomically_and_expire_after_any_source_edit() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let store = dir.path().join("store");
    let mut engine = Engine::initialize(&store, &root).unwrap();
    let bodies = [
        ("service.py", "def refund():\n    return 8\n"),
        ("routes.py", "from service import refund\n"),
        ("test_a.py", "def test_a():\n    assert run() == 8\n"),
        ("test_b.py", "def test_b():\n    assert run() == 8\n"),
    ];
    for (path, body) in bodies {
        engine.replace_source(path, body).unwrap();
    }
    let endpoint = |path: &str| {
        let body = bodies.iter().find(|(p, _)| *p == path).unwrap().1;
        json!({"path":path,"line":2,"symbol":"reported","hash":context_foundry::digest(body.as_bytes())})
    };
    let artifact = |scope: &str, test: &str| {
        json!({
            "v":2,"producer":"synthetic-collector","scope":scope,"run":"run-1",
            "command":["fixture-not-executed"],"completed":true,"exit_code":0,"duration_ms":1,
            "workspace_id":engine.workspace_id().unwrap(),"source_revision":engine.source_revision().unwrap(),
            "inputs":bodies.iter().map(|(path,body)|json!({"path":path,"sha256":context_foundry::digest(body.as_bytes())})).collect::<Vec<_>>(),
            "test":endpoint(test),"observations":[{"from":endpoint(test),"to":endpoint("service.py"),"kind":"covers","count":1}]
        })
    };
    let a = artifact("suite/test-a", "test_a.py");
    let b = artifact("suite/test-b", "test_b.py");
    for value in [&a, &b] {
        assert_eq!(
            engine
                .import_execution(&serde_json::to_vec(value).unwrap())
                .unwrap(),
            1
        );
    }
    let graph = || engine.graph("service.py", true, 1, 128).unwrap();
    let prior = serde_json::to_value(graph()).unwrap();
    assert_eq!(prior["edges"].as_array().unwrap().len(), 2);
    for edge in prior["edges"].as_array().unwrap() {
        assert_eq!(edge["execution"]["freshness"], "workspace_revision");
        assert_eq!(edge["execution"]["snapshot"]["input_count"], 4);
        assert!(edge["execution"]["snapshot"]["inputs_sha256"].is_string());
    }
    for problem in [
        "workspace",
        "revision",
        "hash",
        "duplicate",
        "missing",
        "empty_bad_test",
        "legacy_claim",
    ] {
        let mut invalid = a.clone();
        match problem {
            "workspace" => invalid["workspace_id"] = json!("0".repeat(64)),
            "revision" => invalid["source_revision"] = json!(0),
            "hash" => invalid["inputs"][0]["sha256"] = json!("0".repeat(64)),
            "duplicate" => {
                let duplicate = invalid["inputs"][0].clone();
                invalid["inputs"].as_array_mut().unwrap().push(duplicate);
            }
            "missing" => {
                invalid["inputs"].as_array_mut().unwrap().remove(0);
            }
            "empty_bad_test" => {
                invalid["observations"] = json!([]);
                invalid["test"]["line"] = json!(0);
            }
            "legacy_claim" => invalid["v"] = json!(1),
            _ => unreachable!(),
        }
        assert!(
            engine
                .import_execution(&serde_json::to_vec(&invalid).unwrap())
                .is_err(),
            "{problem}"
        );
        assert_eq!(serde_json::to_value(graph()).unwrap(), prior, "{problem}");
    }
    let mut empty_a = a.clone();
    empty_a["observations"] = json!([]);
    assert_eq!(
        engine
            .import_execution(&serde_json::to_vec(&empty_a).unwrap())
            .unwrap(),
        0
    );
    assert_eq!(
        graph().edges.len(),
        1,
        "only the selected collection is cleared"
    );
    assert_eq!(
        graph().edges[0]
            .execution
            .as_ref()
            .unwrap()
            .scope
            .as_deref(),
        Some("suite/test-b")
    );
    engine
        .import_execution(&serde_json::to_vec(&a).unwrap())
        .unwrap();
    drop(engine);
    engine = Engine::open_existing(&store).unwrap();
    assert_eq!(
        engine
            .graph("service.py", true, 1, 128)
            .unwrap()
            .edges
            .len(),
        2
    );
    // This dependency is neither caller, target nor observing test.
    engine
        .replace_source("routes.py", "def run():\n    return 0\n")
        .unwrap();
    let stale = engine.graph("service.py", true, 1, 128).unwrap();
    assert!(stale.edges.is_empty());
    assert_eq!(stale.stale_edges, 2);
    assert!(
        engine
            .import_execution(&serde_json::to_vec(&a).unwrap())
            .is_err()
    );
    drop(engine);
    engine = Engine::open_existing(&store).unwrap();
    assert!(
        engine
            .graph("service.py", true, 1, 128)
            .unwrap()
            .edges
            .is_empty()
    );
    engine.replace_source("routes.py", bodies[1].1).unwrap();
    assert!(
        engine
            .graph("service.py", true, 1, 128)
            .unwrap()
            .edges
            .is_empty(),
        "reverting bytes never revives an old execution snapshot"
    );
}

#[test]
fn legacy_execution_receipts_explicitly_retain_endpoint_only_freshness() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
    let body = "run()\n";
    engine.replace_source("test.py", body).unwrap();
    engine.replace_source("helper.py", "before\n").unwrap();
    let endpoint = json!({"path":"test.py","line":1,"symbol":"test","hash":context_foundry::digest(body.as_bytes())});
    let artifact = json!({"v":1,"producer":"legacy","run":"one","command":["fixture"],"completed":true,"exit_code":0,"duration_ms":1,
        "test":endpoint,"observations":[{"from":endpoint,"to":endpoint,"kind":"covers","count":1}]});
    engine
        .import_execution(&serde_json::to_vec(&artifact).unwrap())
        .unwrap();
    engine.replace_source("helper.py", "after\n").unwrap();
    let graph = serde_json::to_value(engine.graph("test.py", true, 1, 128).unwrap()).unwrap();
    assert_eq!(graph["edges"][0]["execution"]["freshness"], "endpoint_only");
    assert!(graph["edges"][0]["execution"].get("snapshot").is_none());
}
