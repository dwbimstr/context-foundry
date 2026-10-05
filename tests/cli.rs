use context_foundry::testkit::{
    self, CORRUPT_INDEX_BYTES, KEPT_BODY, V2Response, corrupt_search_index, craft_v1_store,
    new_fixture, parse_v2, quarantine_dirs, schema_marker,
};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn foundry(store: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_foundry"))
        .arg("--store")
        .arg(store)
        .args(args)
        .output()
        .unwrap()
}

fn ok(store: &Path, args: &[&str]) -> Output {
    let out = foundry(store, args);
    assert!(
        out.status.success(),
        "command {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn expect_code(store: &Path, args: &[&str], code: i32) -> Output {
    let out = foundry(store, args);
    assert_eq!(
        out.status.code(),
        Some(code),
        "command {args:?} exit {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn error_json(out: &Output) -> serde_json::Value {
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(stderr.len() <= 1024, "error output exceeds 1024 bytes");
    // The error line is the last stderr line.
    let line = stderr.lines().last().unwrap_or_default();
    serde_json::from_str(line).unwrap_or_else(|_| panic!("not JSON: {line}"))
}

/// A v2 stdout parsed by the shared testkit parser.
fn v2(stdout: &[u8]) -> V2Response {
    let text = std::str::from_utf8(stdout).unwrap();
    parse_v2(text).unwrap_or_else(|e| panic!("not a v2 text ({e}):\n{text}"))
}

/// The v2 handle of the first search hit for `query`.
fn search_handle(store: &Path, query: &str) -> String {
    let out = ok(store, &["search", query]);
    let parsed = v2(&out.stdout);
    assert_eq!(parsed.header[0], "foundry search");
    parsed.items[0].handle.clone()
}

#[test]
fn cli_index_initializes_searches_and_reports_status() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("workspace");
    let store = fixture.path().join("store");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("main.rs"), "fn original_identifier() {}\n").unwrap();
    std::fs::write(root.join("image.bin"), [0, 1, 2]).unwrap();

    let indexed = ok(&store, &["index", root.to_str().unwrap()]);
    let report: serde_json::Value = serde_json::from_slice(&indexed.stdout).unwrap();
    assert_eq!(report["changed"], 1);
    assert_eq!(report["excluded"], 1);
    assert_eq!(report["scan_complete"], true);
    assert_eq!(report["partial"], false);

    let status: serde_json::Value =
        serde_json::from_slice(&ok(&store, &["status"]).stdout).unwrap();
    assert_eq!(status["schema"], 5);
    assert_eq!(status["source_count"], 1);
    assert_eq!(status["pending_count"], 0);
    assert_eq!(status["index_state"], "ready");
    assert_eq!(status["scan_state"], "complete");
    assert_eq!(status["source_revision"], 1);
    assert!(status["workspace_id"].as_str().is_some());

    let search = v2(&ok(&store, &["search", "original_identifier"]).stdout);
    assert!(
        search.header.contains(&"budget:1024".to_owned()),
        "{search:?}"
    );
    let handle = search.items[0].handle.clone();
    assert!(handle.starts_with("main.rs#0-27@"), "{handle}");

    let context = ok(
        &store,
        &["context", "original_identifier", "--tokens", "256"],
    );
    let parsed = v2(&context.stdout);
    assert!(parsed.header.contains(&"budget:256".to_owned()));
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.body.contains("fn original_identifier"))
    );
    let text = String::from_utf8(context.stdout).unwrap();
    assert!(
        tiktoken_rs::o200k_base_singleton()
            .encode_ordinary(&text)
            .len()
            <= 256,
        "CLI context stdout must stay within the token budget"
    );

    let retrieved = v2(&ok(&store, &["retrieve", "--handle", &handle]).stdout);
    assert_eq!(retrieved.items[0].body, "fn original_identifier() {}");
    assert!(retrieved.next.is_none());
}

#[test]
fn cli_edit_invalidates_handles_and_delete_is_not_found() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("workspace");
    let store = fixture.path().join("store");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("main.rs"), "fn first_version() {}\n").unwrap();
    ok(&store, &["index", root.to_str().unwrap()]);
    let handle = search_handle(&store, "first_version");

    std::fs::write(root.join("main.rs"), "fn second_version() {}\n").unwrap();
    ok(&store, &["index", root.to_str().unwrap()]);
    let out = expect_code(&store, &["retrieve", "--handle", &handle], 1);
    assert_eq!(error_json(&out)["code"], "stale_handle");
    assert!(out.stdout.is_empty());

    let handle = search_handle(&store, "second_version");
    ok(&store, &["retrieve", "--handle", &handle]);

    std::fs::remove_file(root.join("main.rs")).unwrap();
    ok(&store, &["index", root.to_str().unwrap()]);
    let out = expect_code(&store, &["retrieve", "--handle", &handle], 1);
    assert_eq!(error_json(&out)["code"], "not_found");
    let status: serde_json::Value =
        serde_json::from_slice(&ok(&store, &["status"]).stdout).unwrap();
    assert_eq!(status["source_count"], 0);
    assert_eq!(
        status["source_revision"], 3,
        "add, change and delete each bump once"
    );
}

#[test]
fn cli_exit_codes_and_bounded_errors() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("workspace");
    let store = fixture.path().join("store");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("main.rs"), "fn budget_probe() {}\n").unwrap();
    ok(&store, &["index", root.to_str().unwrap()]);

    // Missing store: named error, no filesystem state created.
    let missing = fixture.path().join("missing-store");
    let out = expect_code(&missing, &["status"], 1);
    assert_eq!(error_json(&out)["code"], "store_not_found");
    assert!(!missing.exists());

    // Invalid arguments and unsupported modes exit 2.
    let out = expect_code(&store, &["search", "query", "--limit", "0"], 2);
    assert_eq!(error_json(&out)["code"], "invalid_argument");
    let out = expect_code(&store, &["search", "   ", "--limit", "5"], 2);
    assert_eq!(error_json(&out)["code"], "invalid_argument");
    let out = expect_code(&store, &["search", "query", "--tokens", "0"], 2);
    assert_eq!(error_json(&out)["code"], "invalid_argument");
    for bad in ["0", "32769"] {
        let out = expect_code(&store, &["context", "query", "--tokens", bad], 2);
        assert_eq!(error_json(&out)["code"], "invalid_argument", "{bad}");
    }
    let out = expect_code(
        &store,
        &["retrieve", "--handle", "a.rs#0-1@0", "--view", "skeleton"],
        2,
    );
    assert_eq!(error_json(&out)["code"], "invalid_argument");
    let out = expect_code(
        &store,
        &["context", "query", "--strategy", "verify_current"],
        2,
    );
    assert_eq!(error_json(&out)["code"], "unsupported_mode");

    // Budget failure exits nonzero with empty stdout, on every budgeted read.
    let out = expect_code(&store, &["context", "budget_probe", "--tokens", "1"], 1);
    assert_eq!(error_json(&out)["code"], "budget_too_small");
    assert!(out.stdout.is_empty());
    let out = expect_code(&store, &["search", "budget_probe", "--tokens", "1"], 1);
    assert_eq!(error_json(&out)["code"], "budget_too_small");
    assert!(out.stdout.is_empty());
    let handle = search_handle(&store, "budget_probe");
    let out = expect_code(
        &store,
        &["retrieve", "--handle", &handle, "--tokens", "1"],
        1,
    );
    assert_eq!(error_json(&out)["code"], "budget_too_small");
    assert!(out.stdout.is_empty());
    let out = expect_code(
        &store,
        &["retrieve", "--handle", &handle, "--lines", "0"],
        2,
    );
    assert_eq!(error_json(&out)["code"], "invalid_argument");

    // Wrong root refuses before any mutation.
    let other = fixture.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let revision: serde_json::Value =
        serde_json::from_slice(&ok(&store, &["status"]).stdout).unwrap();
    let out = expect_code(&store, &["index", other.to_str().unwrap()], 1);
    assert_eq!(error_json(&out)["code"], "wrong_workspace");
    let after: serde_json::Value = serde_json::from_slice(&ok(&store, &["status"]).stdout).unwrap();
    assert_eq!(after["source_revision"], revision["source_revision"]);
}

#[test]
fn cli_store_busy_exits_three_for_competing_owner() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("workspace");
    let store = fixture.path().join("store");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("main.rs"), "fn busy_probe() {}\n").unwrap();
    ok(&store, &["index", root.to_str().unwrap()]);
    // This process is the competing owner: deterministic, no polling or sleeps.
    let owner = context_foundry::Engine::open_existing(&store).unwrap();
    let out = expect_code(&store, &["status"], 3);
    let error = error_json(&out);
    assert_eq!(error["code"], "store_busy");
    assert_eq!(error["retryable"], true);
    assert!(out.stdout.is_empty());
    // Mutating commands are refused the same way; no lock stealing or retry.
    let out = expect_code(&store, &["index", root.to_str().unwrap()], 3);
    assert_eq!(error_json(&out)["code"], "store_busy");
    drop(owner);
    ok(&store, &["status"]);
}

#[test]
fn cli_upgrade_and_repair_flows() {
    let fixture = tempfile::tempdir().unwrap();
    let store = fixture.path().join("v1store");
    craft_v1_store(&store, Some(&fixture.path().join("ws")));
    assert_eq!(schema_marker(&store), "1");
    // v1 stores are refused on ordinary commands until upgraded.
    let out = expect_code(&store, &["status"], 2);
    assert_eq!(error_json(&out)["code"], "upgrade_required");
    let out = expect_code(&store, &["upgrade-store", "--to", "2"], 2);
    assert_eq!(error_json(&out)["code"], "unsupported_mode");
    let out = expect_code(&store, &["upgrade-store", "--to", "3"], 2);
    assert_eq!(error_json(&out)["code"], "unsupported_mode");
    assert_eq!(
        schema_marker(&store),
        "1",
        "a refused upgrade changes nothing"
    );
    ok(&store, &["upgrade-store", "--to", "5"]);
    let status: serde_json::Value =
        serde_json::from_slice(&ok(&store, &["status"]).stdout).unwrap();
    assert_eq!(status["schema"], 5);
    // Upgrade preserves the committed source, its unfinished index work and
    // feedback, and initializes revision/scan metadata.
    assert_eq!(status["source_count"], 1);
    assert_eq!(status["pending_count"], 1);
    // Its derived index predates search schema v2: status names the explicit
    // repair and nothing is rebuilt on open.
    assert_eq!(status["index_state"], "repair_required");
    assert!(
        status["index_reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("search_schema")),
        "{status}"
    );
    assert_eq!(status["source_revision"], 0);
    assert_eq!(status["scan_state"], "never");
    let training = ok(&store, &["export-training"]);
    assert_eq!(
        String::from_utf8(training.stdout).unwrap().lines().count(),
        1
    );
    // The explicit repair quarantines the v1 index, publishes schema v2 and
    // drains the preserved pending work.
    let repaired = ok(&store, &["repair-index"]);
    let report: serde_json::Value = serde_json::from_slice(&repaired.stdout).unwrap();
    assert_eq!(report["repaired"], true);
    let upgraded = quarantine_dirs(&store);
    assert_eq!(upgraded.len(), 1, "the v1 index is quarantined");
    assert_eq!(report["quarantined_to"], upgraded[0].to_str().unwrap());
    let status: serde_json::Value =
        serde_json::from_slice(&ok(&store, &["status"]).stdout).unwrap();
    assert_eq!(status["index_state"], "ready");
    assert_eq!(status["pending_count"], 0);

    // Corrupt the derived index; repair-index restores it.
    let root = fixture.path().join("ws");
    std::fs::write(root.join("kept.rs"), KEPT_BODY).unwrap();
    ok(&store, &["index", root.to_str().unwrap()]);
    corrupt_search_index(&store);
    let out = expect_code(&store, &["search", "kept"], 1);
    assert_eq!(error_json(&out)["code"], "repair_required");
    let repaired = ok(&store, &["repair-index"]);
    let report: serde_json::Value = serde_json::from_slice(&repaired.stdout).unwrap();
    assert_eq!(report["repaired"], true);
    let quarantines: Vec<_> = quarantine_dirs(&store)
        .into_iter()
        .filter(|dir| !upgraded.contains(dir))
        .collect();
    assert_eq!(quarantines.len(), 1, "exactly one new quarantine");
    assert_eq!(report["quarantined_to"], quarantines[0].to_str().unwrap());
    assert_eq!(
        std::fs::read(quarantines[0].join("meta.json")).unwrap(),
        CORRUPT_INDEX_BYTES
    );
    // After the repair the rebuilt index serves the source again.
    let search = v2(&ok(&store, &["search", "kept"]).stdout);
    assert_eq!(search.items.len(), 1);
    assert!(search.items[0].handle.starts_with("kept.rs#"));
}

#[test]
fn cli_partial_index_report_on_stdout_with_nonzero_exit() {
    if unsafe { libc::geteuid() } == 0 {
        return; // chmod-based failure injection is not effective for root.
    }
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("workspace");
    let store = fixture.path().join("store");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("good.rs"), "fn good_probe() {}\n").unwrap();
    let locked = root.join("locked.rs");
    std::fs::write(&locked, "locked probe\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&locked).unwrap().permissions();
        perms.set_mode(0o000);
        std::fs::set_permissions(&locked, perms).unwrap();
    }
    let out = expect_code(&store, &["index", root.to_str().unwrap()], 1);
    let report: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|_| panic!("partial report must be on stdout"));
    assert_eq!(report["failures"], 1);
    assert_eq!(report["deletions_deferred"], true);
    assert_eq!(report["partial"], true);
    assert_eq!(report["changed"], 1);
    let error = error_json(&out);
    assert_eq!(error["code"], "index_incomplete");
    assert!(error["partial"]["failed"] == 1);
}

#[test]
fn cli_query_text_naming_foreign_paths_reads_nothing() {
    let fixture = tempfile::tempdir().unwrap();
    let root_a = fixture.path().join("a");
    let root_b = fixture.path().join("b");
    let store = fixture.path().join("store-a");
    std::fs::create_dir_all(&root_a).unwrap();
    std::fs::create_dir_all(&root_b).unwrap();
    // Markers share no subtoken of two or more characters.
    std::fs::write(root_a.join("a.rs"), "alpha quokka_apricot\n").unwrap();
    std::fs::write(root_b.join("b.rs"), "beta walrus_banana\n").unwrap();
    ok(&store, &["index", root_a.to_str().unwrap()]);
    // Absolute paths in query text are ordinary search text, not reads.
    let query = format!("find walrus_banana in {}", root_b.join("b.rs").display());
    let search = v2(&ok(&store, &["search", &query]).stdout);
    assert!(search.items.is_empty(), "{search:?}");
    let status: serde_json::Value =
        serde_json::from_slice(&ok(&store, &["status"]).stdout).unwrap();
    assert_eq!(status["source_count"], 1);
    // A valid handle from workspace B is rejected as wrong_workspace.
    let store_b = fixture.path().join("store-b");
    ok(&store_b, &["index", root_b.to_str().unwrap()]);
    let handle = search_handle(&store_b, "walrus_banana");
    let out = expect_code(&store, &["retrieve", "--handle", &handle], 1);
    assert_eq!(error_json(&out)["code"], "wrong_workspace");
}

#[test]
fn cli_search_path_restricts_both_tiers_to_a_normalized_subtree() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let store = fixture.path().join("store");
    for path in ["src/a/x.rs", "src/b/x.rs", "src/ab.rs"] {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "fn probe_fn() {}\n\nfn user() { probe_fn(); }\n").unwrap();
    }
    ok(&store, &["index", root.to_str().unwrap()]);
    // Tier 1 `fn probe_fn` (bytes 0-16, L1), then tier 2 `fn user` (18-43, L3).
    for (filter, file) in [("./src/a/", "src/a/x.rs"), ("src/b", "src/b/x.rs")] {
        let search = v2(&ok(&store, &["search", "probe_fn", "--path", filter]).stdout);
        let shown: Vec<(&str, Option<&str>, Option<&str>)> = search
            .items
            .iter()
            .map(|item| {
                (
                    item.handle.split_once('@').unwrap().0,
                    item.lines.as_deref(),
                    item.label.as_deref(),
                )
            })
            .collect();
        let first = format!("{file}#0-16");
        let second = format!("{file}#18-43");
        assert_eq!(
            shown,
            [
                (first.as_str(), Some("L1"), Some("fn probe_fn")),
                (second.as_str(), Some("L3"), Some("fn user")),
            ],
            "{filter}"
        );
    }
    for bad in ["../x", "/abs", "a//b", ""] {
        let out = expect_code(&store, &["search", "probe_fn", "--path", bad], 2);
        assert_eq!(error_json(&out)["code"], "invalid_argument", "{bad:?}");
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn cli_retrieve_view_outline_elides_interiors_and_refuses_unmapped_languages() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("ws");
    let store = fixture.path().join("store");
    std::fs::create_dir_all(&root).unwrap();
    let lets: String = (0..10).map(|j| format!("    let v{j} = {j};\n")).collect();
    let body = format!("fn outlined_probe() {{\n{lets}}}\n");
    std::fs::write(root.join("o.rs"), &body).unwrap();
    std::fs::write(root.join("notes.txt"), "alpha_note\n").unwrap();
    ok(&store, &["index", root.to_str().unwrap()]);
    let unit = search_handle(&store, "outlined_probe");
    // The unit's handle (bytes 0-183: a 22-byte signature line, ten 16-byte
    // lines and `}`, without the final LF) in the outline form: the 10-line
    // interior unfolds within the outline form's 60-line target.
    assert!(unit.starts_with("o.rs#0-183@"), "{unit}");
    let out = ok(
        &store,
        &[
            "retrieve", "--handle", &unit, "--view", "outline", "--lines", "1-12",
        ],
    );
    let parsed = v2(&out.stdout);
    assert!(parsed.next.is_none());
    assert_eq!(parsed.items[0].form.as_deref(), Some("outline"));
    assert_eq!(parsed.items[0].lines.as_deref(), Some("L1-12"));
    assert_eq!(parsed.items[0].body, body);
    // A 1-token budget is refused with empty stdout; an unmapped language
    // has no outline.
    let out = expect_code(
        &store,
        &[
            "retrieve", "--handle", &unit, "--view", "outline", "--tokens", "1",
        ],
        1,
    );
    assert_eq!(error_json(&out)["code"], "budget_too_small");
    assert!(out.stdout.is_empty());
    let note = search_handle(&store, "alpha_note");
    let out = expect_code(
        &store,
        &["retrieve", "--handle", &note, "--view", "outline"],
        2,
    );
    assert_eq!(error_json(&out)["code"], "unsupported_mode");
    assert!(out.stdout.is_empty());
}

/// Run the CLI with a wall-clock limit so a regression that hangs fails the
/// test instead of stalling the suite.
fn run_with_timeout(store: &std::path::Path, args: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_foundry"))
        .arg("--store")
        .arg(store)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            panic!("foundry {args:?} did not finish within 20 seconds");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn minimum_in(error: &serde_json::Value) -> usize {
    error["message"]
        .as_str()
        .unwrap()
        .rsplit(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn cli_long_diagnostics_return_a_bounded_error_instead_of_hanging() {
    let mut fx = new_fixture();
    fx.add(&[("a.rs", "fn long_probe() {}\n")]);
    let (_dir, store, _root) = fx.close();
    // 600 ASCII bytes once looped forever in error rendering.
    for length in [600usize, 5000, 100_000] {
        let strategy = "x".repeat(length);
        let out = run_with_timeout(&store, &["context", "long_probe", "--strategy", &strategy]);
        assert_eq!(out.status.code(), Some(2), "length {length}");
        assert!(out.stdout.is_empty());
        let error = error_json(&out);
        assert_eq!(error["code"], "unsupported_mode");
        assert_eq!(error["retryable"], false);
    }
}

#[test]
fn cli_stdout_budget_matrix_counts_actual_stdout_and_hints_succeed() {
    let mut fx = new_fixture();
    let body: String = (0..60)
        .map(|i| format!("fn matrix_probe_{i}() {{ \"quoted\" }}\n"))
        .collect();
    fx.add(&[("m.rs", &body)]);
    let handle = fx
        .engine
        .search("matrix_probe_3", 1)
        .unwrap()
        .hits
        .remove(0)
        .handle;
    let (_dir, store, _root) = fx.close();
    let handle = handle.to_v2();
    let count = |text: &str| {
        tiktoken_rs::o200k_base_singleton()
            .encode_ordinary(text)
            .len()
    };
    let commands: [(&str, Vec<&str>); 3] = [
        ("search", vec!["search", "matrix_probe_3"]),
        ("context", vec!["context", "matrix_probe_3"]),
        ("retrieve", vec!["retrieve", "--handle", &handle]),
    ];
    let mut succeeded_at_some_budget = false;
    for (name, base) in commands {
        for budget in [1usize, 32, 64, 256, 1024, 32768] {
            let tokens = budget.to_string();
            let mut args = base.clone();
            args.extend(["--tokens", &tokens]);
            let out = foundry(&store, &args);
            let stdout = String::from_utf8(out.stdout.clone()).unwrap();
            if out.status.success() {
                succeeded_at_some_budget = true;
                assert!(
                    count(&stdout) <= budget,
                    "{name} budget {budget}: {}",
                    count(&stdout)
                );
                assert!(stdout.len() <= 256 * 1024);
                let parsed = v2(&out.stdout);
                assert_eq!(parsed.header[0], format!("foundry {name}"));
                assert!(parsed.header.contains(&format!("budget:{budget}")));
                for item in parsed.items.iter().filter(|item| item.lines.is_some()) {
                    if item.kind != testkit::V2Kind::Source {
                        continue;
                    }
                    let range = context_foundry::store::HandleRef::parse(&item.handle).unwrap();
                    assert_eq!(
                        item.body.as_bytes(),
                        &body.as_bytes()[range.start as usize..range.end as usize]
                    );
                }
            } else {
                // Budget failure: bounded named error, stdout stays empty, and
                // the advertised minimum succeeds when retried.
                assert!(stdout.is_empty(), "{name} budget {budget}: partial stdout");
                let error = error_json(&out);
                assert_eq!(error["code"], "budget_too_small", "{name} {budget}");
                let minimum = minimum_in(&error);
                let again = minimum.to_string();
                let mut retry = base.clone();
                retry.extend(["--tokens", &again]);
                let retried = ok(&store, &retry);
                assert!(count(&String::from_utf8(retried.stdout).unwrap()) <= minimum);
            }
        }
    }
    assert!(succeeded_at_some_budget);
}

#[test]
fn cli_launched_from_another_workspace_keeps_binding_and_state() {
    let mut a = new_fixture();
    a.add(&[("a.rs", "fn alpha_only_probe() {}\n")]);
    let mut b = new_fixture();
    fs::write(b.root.join("b_only.rs"), "fn B_UNIQUE_MARKER_QQQ() {}\n").unwrap();
    let b_root = b.root.clone();
    b.engine
        .index(&b_root, &context_foundry::Control::unbounded())
        .unwrap();
    let b_handle = b
        .engine
        .search("B_UNIQUE_MARKER_QQQ", 5)
        .unwrap()
        .hits
        .remove(0)
        .handle;
    let (b_dir, _b_store, _) = b.close();
    let (_a_dir, a_store, _a_root) = a.close();
    let before = testkit::snapshot(&a_store);
    let b_cwd = b_dir.path().join("ws");
    let query = format!(
        "find B_UNIQUE_MARKER_QQQ in {}",
        b_cwd.join("b_only.rs").display()
    );
    let from_b = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_foundry"))
            .current_dir(&b_cwd)
            .arg("--store")
            .arg(&a_store)
            .args(args)
            .output()
            .unwrap()
    };
    // Query text naming B's absolute path is ordinary text, never a read.
    let search = from_b(&["search", &query]);
    assert!(search.status.success());
    assert!(v2(&search.stdout).items.is_empty());
    let context = from_b(&["context", &query, "--tokens", "2048"]);
    assert!(context.status.success());
    assert!(!String::from_utf8_lossy(&context.stdout).contains("B_UNIQUE_MARKER_QQQ()"));
    // B's valid handle is wrong_workspace against A's bound store.
    let retrieve = from_b(&["retrieve", "--handle", &b_handle.to_v2()]);
    assert_eq!(retrieve.status.code(), Some(1));
    assert_eq!(error_json(&retrieve)["code"], "wrong_workspace");
    // Binding, sources, pending and revision: every row exactly as before.
    assert_eq!(testkit::snapshot(&a_store), before);
    // Without --store the default is relative to B and nothing is created.
    let default = Command::new(env!("CARGO_BIN_EXE_foundry"))
        .current_dir(&b_cwd)
        .arg("status")
        .output()
        .unwrap();
    assert_eq!(default.status.code(), Some(1));
    assert_eq!(error_json(&default)["code"], "store_not_found");
    assert!(!b_cwd.join(".context-foundry").exists());
}

#[test]
fn cli_four_thousand_ninety_six_byte_escaped_path_round_trips_through_retrieve() {
    // `#`, `@`, `.` and JSON-special characters, unescaped in v2 handles.
    let component = "\"\\#@.".repeat(40);
    let mut parts: Vec<String> = Vec::new();
    while parts.len() * 201 + 200 < 4096 {
        parts.push(component.clone());
    }
    let mut path = parts.join("/");
    path.push('/');
    path.push_str(&"q".repeat(4096 - path.len()));
    assert_eq!(path.len(), 4096);
    let mut fx = new_fixture();
    fx.add(&[(&path, "fn cli_escaped_path_probe() {}\n")]);
    let (_dir, store, _root) = fx.close();
    let search = ok(
        &store,
        &["search", "cli_escaped_path_probe", "--tokens", "32768"],
    );
    let handle = v2(&search.stdout).items[0].handle.clone();
    let parsed = context_foundry::store::HandleRef::parse(&handle).unwrap();
    assert_eq!(parsed.path, path);
    assert!(handle.len() <= 4200, "{}", handle.len());
    let retrieved = ok(
        &store,
        &["retrieve", "--handle", &handle, "--tokens", "32768"],
    );
    let retrieved = v2(&retrieved.stdout);
    assert_eq!(retrieved.items[0].body, "fn cli_escaped_path_probe() {}\n");
    assert_eq!(retrieved.items[0].handle, handle);
    // A tiny budget is a named refusal whose hint succeeds when retried.
    let refused = expect_code(
        &store,
        &["retrieve", "--handle", &handle, "--tokens", "1"],
        1,
    );
    assert!(refused.stdout.is_empty());
    let error = error_json(&refused);
    assert_eq!(error["code"], "budget_too_small");
    let hint = minimum_in(&error).to_string();
    let at_hint = ok(
        &store,
        &["retrieve", "--handle", &handle, "--tokens", &hint],
    );
    assert!(!v2(&at_hint.stdout).items.is_empty());
    // A handle over the 4200-byte input cap is refused as invalid.
    let out = expect_code(&store, &["retrieve", "--handle", &"x".repeat(4201)], 2);
    assert_eq!(error_json(&out)["code"], "invalid_argument");
}

#[test]
fn cli_cooperative_cancellation_exits_130_with_a_partial_report() {
    // The fault-arming twin of the CLI interrupts itself at the named
    // boundary between the search commit and the pending clear.
    let fx = new_fixture();
    for i in 0..3 {
        fs::write(
            fx.root.join(format!("c{i}.rs")),
            format!("fn cancel_probe_{i}() {{}}\n"),
        )
        .unwrap();
    }
    let (_dir, store, root) = fx.close();
    let cancelled = Command::new(env!("CARGO_BIN_EXE_foundry-faults"))
        .arg("--store")
        .arg(&store)
        .arg("index")
        .arg(&root)
        .env(
            "FOUNDRY_TEST_FAULT",
            "ctxfoundry-fault/index.after_search_commit=cancel",
        )
        .output()
        .unwrap();
    assert_eq!(cancelled.status.code(), Some(130));
    // The partial report is on stdout; the bounded error names the code.
    let report: serde_json::Value =
        serde_json::from_slice(&cancelled.stdout).unwrap_or_else(|_| {
            panic!(
                "partial report missing: {}",
                String::from_utf8_lossy(&cancelled.stdout)
            )
        });
    assert_eq!(report["partial"], true);
    assert_eq!(report["reason_code"], "cancelled");
    assert_eq!(report["changed"], 3);
    let error = error_json(&cancelled);
    assert_eq!(error["code"], "cancelled");
    assert_eq!(error["retryable"], true);
    // The committed work survives; a clean rerun completes the scan.
    let status: serde_json::Value =
        serde_json::from_slice(&ok(&store, &["status"]).stdout).unwrap();
    assert_eq!(status["source_count"], 3);
    let finished: serde_json::Value =
        serde_json::from_slice(&ok(&store, &["index", root.to_str().unwrap()]).stdout).unwrap();
    assert_eq!(finished["partial"], false);
    assert_eq!(finished["unchanged"], 3);
}

#[test]
fn cli_search_stdout_including_the_final_newline_stays_within_its_caps() {
    let mut fx = new_fixture();
    // 64 quote-dense hits under long paths: at the 32768-token maximum the
    // complete stdout, final newline included, stays within the budget and
    // the 256 KiB cap; hits that do not fit are omitted and counted, the
    // highest-ranked retained first.
    let line = "\"".repeat(60) + "\\\\big_probe\\\\\n";
    for i in 0..64 {
        let body = format!("fn big_probe_{i}() {{\n{}\n}}\n", line.repeat(115));
        fx.engine
            .replace_source(&format!("big{i:02}_{}.rs", "padding_".repeat(400)), &body)
            .unwrap();
    }
    fx.drain();
    let (_dir, store, _root) = fx.close();
    let out = ok(
        &store,
        &["search", "big_probe", "--limit", "64", "--tokens", "32768"],
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.ends_with('\n'));
    assert!(
        stdout.len() <= 256 * 1024,
        "stdout including the final newline is {} bytes",
        stdout.len()
    );
    assert!(
        tiktoken_rs::o200k_base_singleton()
            .encode_ordinary(&stdout)
            .len()
            <= 32768
    );
    let parsed = v2(stdout.as_bytes());
    let omitted = parsed
        .header
        .iter()
        .find_map(|segment| segment.strip_prefix("omitted:"))
        .map_or(0, |n| n.parse::<usize>().unwrap());
    assert_eq!(parsed.items.len() + omitted, 64, "{:?}", parsed.header);
    let one = v2(&ok(
        &store,
        &["search", "big_probe", "--limit", "1", "--tokens", "32768"],
    )
    .stdout);
    assert_eq!(
        parsed.items[0].handle, one.items[0].handle,
        "highest-ranked hits are retained first"
    );
}

/// 001 T004 CLI smoke on the shipped example and the agent-task fixture:
/// `lines` selects whole absolute file lines inside the handle and never
/// widens it.
#[test]
fn cli_retrieve_lines_select_whole_file_lines_and_never_widen_the_handle() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = tempfile::tempdir().unwrap();
    let store = fixture.path().join("cf-smoke");
    ok(
        &store,
        &[
            "index",
            manifest.join("examples/workspace").to_str().unwrap(),
        ],
    );
    let handle = search_handle(&store, "parse_record");
    // The handle is the whole `parse_record` unit, bytes 0-86: lines 1-3
    // without the file's final LF.
    assert!(handle.starts_with("src/parser.rs#0-86@"), "{handle}");
    let out = ok(&store, &["retrieve", "--handle", &handle, "--lines", "1-3"]);
    let parsed = v2(&out.stdout);
    assert_eq!(parsed.header[0], "foundry retrieve");
    assert_eq!(
        parsed.items[0].body,
        "pub fn parse_record(input: &str) -> Option<(&str, &str)> {\n    input.split_once('=')\n}",
        "exactly lines 1-3"
    );
    assert_eq!(parsed.items[0].lines.as_deref(), Some("L1-3"));
    let out = expect_code(
        &store,
        &["retrieve", "--handle", &handle, "--lines", "4-6"],
        2,
    );
    assert_eq!(error_json(&out)["code"], "invalid_range");
    assert!(out.stdout.is_empty());

    // records.rs: `parse_record` occupies lines 5-7 after a three-line `//!`
    // comment and a blank line. A handle covering exactly lines 5-7 asked
    // for lines 4-6 returns lines 5-6.
    let store = fixture.path().join("agent-task");
    let root = manifest.join("tests/fixtures/agent-task");
    ok(&store, &["index", root.to_str().unwrap()]);
    let records = fs::read_to_string(root.join("src/records.rs")).unwrap();
    let line_start = |n: usize| -> usize {
        records
            .split_inclusive('\n')
            .take(n - 1)
            .map(str::len)
            .sum()
    };
    let ws16 = context_foundry::store::HandleRef::parse(&search_handle(&store, "parse_record"))
        .unwrap()
        .ws16;
    let lines_5_to_7 = format!(
        "src/records.rs#{}-{}@{}.{ws16}",
        line_start(5),
        records.len(),
        &context_foundry::digest(records.as_bytes())[..32]
    );
    let out = ok(
        &store,
        &["retrieve", "--handle", &lines_5_to_7, "--lines", "4-6"],
    );
    let parsed = v2(&out.stdout);
    assert_eq!(
        parsed.items[0].body,
        &records[line_start(5)..line_start(7)],
        "lines 5-6, never widened to line 4"
    );
    assert_eq!(parsed.items[0].lines.as_deref(), Some("L5-6"));
}
