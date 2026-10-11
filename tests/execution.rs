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
    let bytes = collect(&root, &["service.py", "test_refund.py"], "dynamic-refund");
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
    let store = dir.path().join("store");
    let mut engine = Engine::initialize(&store, &root).unwrap();
    engine.index(&root, &Control::unbounded()).unwrap();
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

fn collect(root: &std::path::Path, names: &[&str], run: &str) -> Vec<u8> {
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
print(json.dumps({'v':1,'producer':'cpython-sys-settrace','run':sys.argv[3],'command':[sys.executable,'-I','-B',*sys.argv], 'completed':True,'exit_code':0,'duration_ms':int((time.monotonic()-started)*1000),'test':test,'observations':observations}))
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
    let bytes = collect(
        &root,
        &["service.py", "routes.py", "test_refund.py"],
        "flask-dynamic-refund",
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
    let store = dir.path().join("store");
    let mut engine = Engine::initialize(&store, &root).unwrap();
    engine.index(&root, &Control::unbounded()).unwrap();
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
