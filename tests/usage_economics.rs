use context_foundry::usage::economics::report;
use serde_json::{Value, json};
use std::path::Path;

fn receipt(request: &str, input: u64, output: u64, cached: u64, cost: u64) -> Value {
    json!({"v":1,"session_id":"session","request_id":request,"adapter_id":"fixture","model_id":"fixture-model","context_ids":[],
        "input_tokens":input,"output_tokens":output,"cached_input_tokens":cached,
        "cost_microunits":cost,"currency":"USD","cost_basis":"reported","outcome":"complete"})
}
fn write(dir: &Path, name: &str, rows: &[Value]) {
    std::fs::write(
        dir.join(name),
        rows.iter().map(|r| format!("{r}\n")).collect::<String>(),
    )
    .unwrap();
}
fn save(dir: &Path, manifest: &Value) -> std::path::PathBuf {
    let path = dir.join("manifest.json");
    std::fs::write(&path, serde_json::to_vec(manifest).unwrap()).unwrap();
    path
}
fn attempt(id: &str, outcome: &str, log: &str) -> Value {
    json!({"id":id,"task":"repair","outcome":outcome,"verification_sha256":"a".repeat(64),"usage":{"receipts":log,"expected_requests":1}})
}
fn manifest() -> Value {
    json!({"v":1,"reuse_completions":10,"attempts":[attempt("failed-first","failed","failed.jsonl"),attempt("retry","passed","passed.jsonl")],
        "shared":[{"phase":"bootstrap","usage":{"receipts":"bootstrap.jsonl","expected_requests":1}}]})
}
fn fixture(dir: &Path) {
    write(dir, "failed.jsonl", &[receipt("a", 100, 10, 80, 20)]);
    write(dir, "passed.jsonl", &[receipt("b", 50, 5, 20, 10)]);
    write(dir, "bootstrap.jsonl", &[receipt("c", 1000, 0, 0, 100)]);
}

#[test]
fn economics_counts_failures_and_fixed_cost_once_and_separates_projection() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let path = save(dir.path(), &manifest());
    let result = report(&path).unwrap();
    assert_eq!(result["reported_completed_tasks"], 1);
    assert_eq!(result["failed_attempts"], 1);
    assert_eq!(result["tokens_per_reported_completion"], 1165.0);
    assert_eq!(result["uncached_input_tokens"], 1050);
    assert_eq!(result["costs"]["USD"]["known_total_microunits"], 130);
    assert_eq!(
        result["costs"]["USD"]["observed_per_reported_completion"],
        130.0
    );
    assert_eq!(
        result["costs"]["USD"]["projected_per_reported_completion"],
        40.0
    );
    assert_eq!(result["comparison"], "not_performed");
    assert_eq!(
        result["logs"][0]["summary"]["input_sha256"],
        context_foundry::digest(&std::fs::read(dir.path().join("failed.jsonl")).unwrap())
    );
    let cli = std::process::Command::new(env!("CARGO_BIN_EXE_foundry"))
        .args(["usage", "economics", "--manifest"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&cli.stdout).unwrap(),
        result
    );
}

#[test]
fn partial_usage_unknown_requests_and_failed_tasks_cannot_claim_completion_economics() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let mut m = manifest();
    m["attempts"][1]["outcome"] = "failed".into();
    let result = report(&save(dir.path(), &m)).unwrap();
    assert!(result["tokens_per_reported_completion"].is_null());
    assert!(result["costs"]["USD"]["observed_per_reported_completion"].is_null());
    m = manifest();
    m["attempts"][1]["usage"]["expected_requests"] = 2.into();
    let result = report(&save(dir.path(), &m)).unwrap();
    assert_eq!(result["usage_complete_for_declared_requests"], false);
    assert!(result["tokens_per_reported_completion"].is_null());
    assert!(result["costs"]["USD"]["observed_per_reported_completion"].is_null());
    let mut partial = receipt("b", 50, 5, 20, 10);
    for field in [
        "input_tokens",
        "cached_input_tokens",
        "cost_microunits",
        "currency",
        "cost_basis",
    ] {
        partial.as_object_mut().unwrap().remove(field);
    }
    write(dir.path(), "passed.jsonl", &[partial]);
    let result = report(&save(dir.path(), &manifest())).unwrap();
    assert_eq!(
        result["logs"][1]["summary"]["receipts_missing_usage"], 0,
        "old any-category subtotal remains compatible"
    );
    assert_eq!(result["logs"][1]["summary"]["missing"]["input_tokens"], 1);
    assert!(result["tokens_per_reported_completion"].is_null());
    assert!(result["uncached_input_tokens"].is_null());
    assert!(result["costs"]["USD"]["projected_per_reported_completion"].is_null());
    assert_eq!(
        result["attempt_subtotals"]["output_tokens"], 15,
        "known failed/partial usage remains visible"
    );
}

#[test]
fn duplicate_request_attribution_invalid_outcomes_and_overflows_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let duplicated = receipt("a", 100, 10, 80, 20);
    write(
        dir.path(),
        "failed.jsonl",
        &[duplicated.clone(), duplicated.clone()],
    );
    assert_eq!(
        report(&save(dir.path(), &manifest())).unwrap()["logs"][0]["summary"]["duplicate_retries_ignored"],
        1
    );
    write(dir.path(), "passed.jsonl", &[duplicated]);
    assert!(report(&save(dir.path(), &manifest())).is_err());
    fixture(dir.path());
    let mut m = manifest();
    m["attempts"][1]["verification_sha256"] = Value::Null;
    assert!(report(&save(dir.path(), &m)).is_err());
    m["attempts"][1]["outcome"] = "unknown".into();
    assert!(report(&save(dir.path(), &m)).unwrap()["tokens_per_reported_completion"].is_null());
    write(
        dir.path(),
        "bootstrap.jsonl",
        &[receipt("c", u64::MAX, 0, 0, 100)],
    );
    assert!(report(&save(dir.path(), &manifest())).is_err());
}
