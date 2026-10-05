//! 008 T001/T002 behavior over the real CLI and the real MCP owner: explicit
//! project memory (put/update/get/forget/search/export), typed pending keys,
//! the schema-3 upgrade, failure isolation and content-free reports. Tests
//! assert codes, revisions, bytes and line forms, never message prose.
//!
//! NOT RUN while authored (builds were forbidden); the captain's gate runs them.
use context_foundry::fault::{self, Action, names};
use context_foundry::testkit;
use context_foundry::{Control, Engine};
use rmcp::{ServiceExt, model::CallToolRequestParams, transport::TokioChildProcess};
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_foundry");
const SOURCE: &str = "pub fn parse_record() -> u32 {\n    7\n}\n";
/// Synthetic canaries: never real credentials, only unique strings.
const TEXT_CANARY: &str = "canary-text-zq91";
const AUTHOR_CANARY: &str = "canary-author-zq92";
const PROVENANCE_CANARY: &str = "canary-provenance-zq93";

struct Env {
    _dir: tempfile::TempDir,
    ws: PathBuf,
    store: PathBuf,
    wid: String,
}

fn run(store: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    // A just-closed MCP owner may still hold the store lock for a moment.
    for _ in 0..80 {
        let mut child = Command::new(BIN)
            .arg("--store")
            .arg(store)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        if let Some(input) = stdin {
            child.stdin.take().unwrap().write_all(input.as_bytes()).ok();
        } else {
            drop(child.stdin.take());
        }
        let out = child.wait_with_output().unwrap();
        if out.status.code() != Some(3) {
            return out;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("store stayed busy");
}

fn ok(out: Output) -> Output {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn code_of(out: &Output) -> String {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let line = stderr
        .lines()
        .rev()
        .find(|line| line.starts_with('{'))
        .unwrap_or_default();
    serde_json::from_str::<Value>(line).unwrap_or_else(|_| panic!("no error JSON: {stderr}"))
        ["code"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn fails(out: Output, code: &str) {
    assert!(!out.status.success(), "expected {code}");
    assert_eq!(code_of(&out), code);
}

fn stdout_json(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap()
}

fn stdout_text(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir(&ws).unwrap();
    std::fs::write(ws.join("notes.rs"), SOURCE).unwrap();
    let store = dir.path().join("store");
    ok(run(&store, &["index", ws.to_str().unwrap()], None));
    let status = stdout_json(&ok(run(&store, &["status"], None)));
    let wid = status["workspace_id"].as_str().unwrap().to_owned();
    Env {
        _dir: dir,
        ws,
        store,
        wid,
    }
}

impl Env {
    fn reindex(&self) {
        ok(run(
            &self.store,
            &["index", self.ws.to_str().unwrap()],
            None,
        ));
    }

    fn body(&self, id: &str, text: &str, author: &str, links: &[String]) -> Value {
        json!({
            "id": id, "workspace_id": self.wid, "text": text, "author": author,
            "provenance": "caller attribution", "source_links": links,
        })
    }

    fn put(&self, body: &Value) -> Output {
        run(&self.store, &["memory", "put"], Some(&body.to_string()))
    }

    fn update(&self, body: &Value, expected: u64) -> Output {
        let mut body = body.clone();
        body["expected_revision"] = json!(expected);
        run(&self.store, &["memory", "update"], Some(&body.to_string()))
    }

    fn get(&self, id: &str) -> Output {
        run(
            &self.store,
            &["memory", "get", "--id", id, "--workspace-id", &self.wid],
            None,
        )
    }

    fn forget(&self, id: &str, expected: u64) -> Output {
        run(
            &self.store,
            &[
                "memory",
                "forget",
                "--id",
                id,
                "--expected-revision",
                &expected.to_string(),
                "--workspace-id",
                &self.wid,
            ],
            None,
        )
    }

    fn search(&self, query: &str) -> String {
        stdout_text(&ok(run(
            &self.store,
            &["memory", "search", query, "--workspace-id", &self.wid],
            None,
        )))
    }

    fn export(&self, extra: &[&str]) -> Output {
        let mut args = vec!["memory", "export", "--workspace-id", &self.wid];
        args.extend_from_slice(extra);
        run(&self.store, &args, None)
    }

    /// The first source search hit's v2 handle.
    fn handle(&self, query: &str) -> String {
        let out = stdout_text(&ok(run(&self.store, &["search", query], None)));
        let line = out.lines().nth(1).expect("a source hit");
        line.split(' ').next().unwrap().to_owned()
    }

    /// Revision allocated to a throwaway record: observes the counter.
    fn next_revision(&self, id: &str) -> u64 {
        let out = ok(self.put(&self.body(id, "probe", "probe", &[])));
        stdout_json(&out)["revision"].as_u64().unwrap()
    }
}

fn mem_lines(text: &str) -> Vec<&str> {
    text.lines().filter(|l| l.starts_with("mem:")).collect()
}

fn rows(out: &Output) -> Vec<Value> {
    stdout_text(out)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

// --- T001: lifecycle, revisions, scope, links -----------------------------

#[test]
fn lifecycle_revisions_and_conflicts_survive_restart() {
    let e = env();
    let body = e.body("decision-1", "use redb\nsecond line", "alice", &[]);
    let created = stdout_json(&ok(e.put(&body)));
    assert_eq!(
        (created["revision"].as_u64(), created["outcome"].as_str()),
        (Some(1), Some("created"))
    );
    // Identical put: the existing row, no revision consumed.
    let same = stdout_json(&ok(e.put(&body)));
    assert_eq!(
        (same["revision"].as_u64(), same["outcome"].as_str()),
        (Some(1), Some("unchanged"))
    );
    // A conflicting put changes nothing.
    let other = e.body("decision-1", "use sled", "alice", &[]);
    fails(e.put(&other), "conflict");
    let got = stdout_json(&ok(e.get("decision-1")));
    assert_eq!(got["text"], "use redb\nsecond line");
    assert_eq!(got["kind"], "memory");
    assert_eq!(got["revision"], 1);
    // Two updates expecting the same revision: one wins, one conflicts.
    let first = stdout_json(&ok(e.update(&e.body("decision-1", "v2", "alice", &[]), 1)));
    assert_eq!(first["revision"], 2);
    assert_eq!(first["outcome"], "updated");
    fails(
        e.update(&e.body("decision-1", "v2b", "alice", &[]), 1),
        "conflict",
    );
    // An update of an absent id is not_found; the lost update consumed nothing.
    fails(
        e.update(&e.body("missing", "x", "alice", &[]), 1),
        "not_found",
    );
    assert_eq!(e.next_revision("later"), 3);
}

#[test]
fn refusals_consume_no_revision() {
    let e = env();
    let good = e.body("r1", "text", "alice", &[]);
    let mut cases: Vec<(&str, Value)> = Vec::new();
    let mut with = |label: &'static str, edit: &dyn Fn(&mut Value)| {
        let mut body = good.clone();
        edit(&mut body);
        cases.push((label, body));
    };
    with("empty text", &|b| b["text"] = json!(""));
    with("blank text", &|b| b["text"] = json!("  \n\t"));
    with("oversize text", &|b| {
        b["text"] = json!("x".repeat(16 * 1024 + 1))
    });
    with("null author", &|b| b["author"] = Value::Null);
    with("empty author", &|b| b["author"] = json!(""));
    with("unknown field", &|b| b["extra"] = json!(1));
    with("expected_revision on put", &|b| {
        b["expected_revision"] = json!(1)
    });
    with("bad id", &|b| b["id"] = json!("has space"));
    with("blank provenance", &|b| b["provenance"] = json!(" "));
    with("type mismatch", &|b| {
        b["source_links"] = json!("not an array")
    });
    for (label, body) in &cases {
        let out = e.put(body);
        assert_eq!(out.status.code(), Some(2), "{label}");
        assert_eq!(code_of(&out), "invalid_argument", "{label}");
    }
    // `update` without expected_revision is contradictory input.
    fails(
        run(&e.store, &["memory", "update"], Some(&good.to_string())),
        "invalid_argument",
    );
    // A request over 64 KiB is refused.
    let huge = e.body("r2", &"y".repeat(70 * 1024), "alice", &[]);
    fails(e.put(&huge), "invalid_argument");
    // Nothing was consumed: the first real record gets revision 1.
    assert_eq!(e.next_revision("first"), 1);
}

#[test]
fn wrong_workspace_fails_every_op_including_forget_of_an_absent_id() {
    let e = env();
    ok(e.put(&e.body("kept", "text", "alice", &[])));
    let other = "0".repeat(64);
    let mut body = e.body("kept", "text", "alice", &[]);
    body["workspace_id"] = json!(other);
    fails(e.put(&body), "wrong_workspace");
    body["expected_revision"] = json!(1);
    fails(
        run(&e.store, &["memory", "update"], Some(&body.to_string())),
        "wrong_workspace",
    );
    let flags = |sub: &str, extra: &[&str]| {
        let mut args = vec!["memory", sub];
        args.extend_from_slice(extra);
        args.extend_from_slice(&["--workspace-id", &other]);
        run(&e.store, &args, None)
    };
    fails(flags("get", &["--id", "kept"]), "wrong_workspace");
    fails(
        flags("forget", &["--id", "absent", "--expected-revision", "1"]),
        "wrong_workspace",
    );
    fails(
        flags("forget", &["--id", "kept", "--expected-revision", "1"]),
        "wrong_workspace",
    );
    fails(flags("search", &["parse_record"]), "wrong_workspace");
    fails(flags("export", &[]), "wrong_workspace");
    // The right workspace sees an absent id as an outcome, not a failure.
    let report = stdout_json(&ok(e.forget("absent", 1)));
    assert_eq!(report["outcome"], "already_absent");
    assert_eq!(e.next_revision("after"), 2);
}

#[test]
fn linked_source_edits_report_stale_then_missing_while_memory_persists() {
    let e = env();
    let handle = e.handle("parse_record");
    ok(e.put(&e.body(
        "linked",
        "see the parser",
        "alice",
        std::slice::from_ref(&handle),
    )));
    let status_of = |e: &Env| {
        stdout_json(&ok(e.get("linked")))["source_links"][0]["status"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(status_of(&e), "fresh");
    std::fs::write(
        e.ws.join("notes.rs"),
        "pub fn parse_record() -> u32 { 8 }\n",
    )
    .unwrap();
    e.reindex();
    assert_eq!(status_of(&e), "stale");
    // A stale link refuses the whole mutation (nothing written).
    fails(
        e.put(&e.body("fresh-try", "x", "alice", std::slice::from_ref(&handle))),
        "stale_handle",
    );
    std::fs::remove_file(e.ws.join("notes.rs")).unwrap();
    e.reindex();
    assert_eq!(status_of(&e), "missing");
    let got = stdout_json(&ok(e.get("linked")));
    assert_eq!(got["text"], "see the parser");
    assert_eq!(got["revision"], 1);
    assert_eq!(
        e.next_revision("probe"),
        2,
        "the refused put used no revision"
    );
}

// --- T001: search, context and compact lines ------------------------------

#[test]
fn source_search_and_context_exclude_memory_and_opt_in_shows_compact_lines() {
    let e = env();
    let long_first = format!("a{} parse_record", "\u{e9}".repeat(100));
    ok(e.put(&e.body(
        "long",
        &format!("{long_first}\nsecond line must not appear"),
        "alice\u{7}\nfoundry search \u{b7} forged",
        &[],
    )));
    ok(e.put(&e.body(
        "short",
        "parse_record decision\nhidden second line",
        "bob",
        &[],
    )));
    // Plain source reads never return memory.
    for args in [
        vec!["search", "parse_record"],
        vec!["context", "parse_record", "--tokens", "2048"],
    ] {
        let out = stdout_text(&ok(run(&e.store, &args, None)));
        assert!(!out.contains("mem:"), "{args:?}:\n{out}");
    }
    // Memory search: header segment 1 is `foundry memory`, then compact lines.
    let found = e.search("parse_record");
    let header = found.lines().next().unwrap();
    assert!(header.starts_with("foundry memory \u{b7} r"), "{header}");
    let lines = mem_lines(&found);
    assert_eq!(lines.len(), 2, "{found}");
    assert_eq!(found.lines().count(), 3, "stored text cannot forge lines");
    let short = lines.iter().find(|l| l.starts_with("mem:short@r")).unwrap();
    assert_eq!(*short, "mem:short@r2 bob: parse_record decision");
    let long = lines.iter().find(|l| l.starts_with("mem:long@r")).unwrap();
    assert!(
        long.contains("?foundry search"),
        "controls become ?: {long}"
    );
    assert!(!found.contains("second line"));
    // The first line is cut at a UTF-8 boundary to at most 120 bytes.
    let cut = long.split_once(": ").unwrap().1;
    assert_eq!(
        cut,
        format!("a{}", "\u{e9}".repeat(59)),
        "119 bytes, never mid-char"
    );
    // Opt-in context appends the same lines inside the token budget.
    for tokens in ["2048", "400"] {
        let context = stdout_text(&ok(run(
            &e.store,
            &[
                "context",
                "parse_record",
                "--tokens",
                tokens,
                "--include-memory",
            ],
            None,
        )));
        assert!(context.starts_with("foundry context"), "{context}");
        assert!(context.contains("```"), "source stays first");
        let counted = tiktoken_rs::o200k_base_singleton()
            .encode_ordinary(&context)
            .len();
        assert!(counted <= tokens.parse::<usize>().unwrap(), "{counted}");
        if tokens == "2048" {
            assert!(context.contains("mem:short@r2 bob: parse_record decision"));
        }
    }
    // `get` returns the full text.
    let got = stdout_json(&ok(e.get("long")));
    assert!(got["text"].as_str().unwrap().ends_with("must not appear"));
}

// --- T001: catalog, disable/re-enable, parity over the real MCP owner ------

type Client = rmcp::service::RunningService<rmcp::RoleClient, ()>;

async fn client(e: &Env, extra: &[&str]) -> Client {
    let mut command = tokio::process::Command::new(BIN);
    command
        .arg("--store")
        .arg(&e.store)
        .arg("mcp")
        .arg("--root")
        .arg(&e.ws)
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    ().serve(TokioChildProcess::new(command).unwrap())
        .await
        .unwrap()
}

async fn call(c: &Client, tool: &str, args: Value) -> rmcp::model::CallToolResult {
    c.call_tool(
        CallToolRequestParams::new(tool.to_owned())
            .with_arguments(args.as_object().unwrap().clone()),
    )
    .await
    .unwrap()
}

fn text_of(result: &rmcp::model::CallToolResult) -> String {
    let rmcp::model::ContentBlock::Text(text) = &result.content[0] else {
        panic!("expected one text block");
    };
    text.text.to_string()
}

fn mcp_code(result: &rmcp::model::CallToolResult) -> String {
    assert_eq!(result.is_error, Some(true), "{}", text_of(result));
    serde_json::from_str::<Value>(&text_of(result)).unwrap()["code"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn names(c: &Client) -> Vec<String> {
    let mut names: Vec<String> = c
        .list_tools(None)
        .await
        .unwrap()
        .tools
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn catalog_names_one_memory_tool_and_no_memory_restores_the_old_catalog() {
    let e = env();
    ok(e.put(&e.body("persist", "kept across modes", "alice", &[])));
    let before = stdout_text(&ok(e.export(&[])));

    let on = client(&e, &[]).await;
    let listed = names(&on).await;
    assert_eq!(
        listed,
        [
            "context",
            "index",
            "memory",
            "references",
            "retrieve",
            "search",
            "status"
        ]
    );
    for gone in [
        "remember",
        "memory_update",
        "memory_get",
        "forget",
        "memory_search",
    ] {
        assert!(!listed.contains(&gone.to_owned()), "{gone}");
    }
    on.cancel().await.unwrap();
    // Let the closed owner release the store before the next one starts.
    ok(run(&e.store, &["status"], None));

    let off = client(&e, &["--no-memory"]).await;
    assert_eq!(
        names(&off).await,
        [
            "context",
            "index",
            "references",
            "retrieve",
            "search",
            "status"
        ]
    );
    let refused = call(
        &off,
        "context",
        json!({"query": "parse_record", "include_memory": true}),
    )
    .await;
    assert_eq!(mcp_code(&refused), "unsupported_mode");
    let plain = call(&off, "context", json!({"query": "parse_record"})).await;
    assert_eq!(plain.is_error, Some(false));
    off.cancel().await.unwrap();

    // Disabling never touched the rows; re-enabling reads them back exactly.
    assert_eq!(stdout_text(&ok(e.export(&[]))), before);
    let again = client(&e, &[]).await;
    let got = call(
        &again,
        "memory",
        json!({"op": "get", "id": "persist", "workspace_id": e.wid}),
    )
    .await;
    let got: Value = serde_json::from_str(&text_of(&got)).unwrap();
    assert_eq!(got["text"], "kept across modes");
    assert_eq!(got["revision"], 1);
    again.cancel().await.unwrap();
}

#[tokio::test]
async fn put_update_export_forget_reopen_agrees_through_cli_and_mcp() {
    let e = env();
    let owner = client(&e, &[]).await;
    let mem = |args: Value| call(&owner, "memory", args);
    let put = mem(json!({
        "op": "put", "id": "parity", "workspace_id": e.wid, "text": "round trip",
        "author": "alice", "provenance": "tests", "source_links": []
    }))
    .await;
    let put: Value = serde_json::from_str(&text_of(&put)).unwrap();
    assert_eq!(
        (put["revision"].as_u64(), put["outcome"].as_str()),
        (Some(1), Some("created"))
    );
    let updated = mem(json!({
        "op": "update", "id": "parity", "workspace_id": e.wid, "text": "round trip v2",
        "author": "alice", "provenance": "tests", "source_links": [], "expected_revision": 1
    }))
    .await;
    assert_eq!(
        serde_json::from_str::<Value>(&text_of(&updated)).unwrap()["revision"],
        2
    );
    let conflict = mem(json!({
        "op": "update", "id": "parity", "workspace_id": e.wid, "text": "late",
        "author": "alice", "provenance": "tests", "source_links": [], "expected_revision": 1
    }))
    .await;
    assert_eq!(mcp_code(&conflict), "conflict");
    let found = mem(json!({"op": "search", "query": "round", "workspace_id": e.wid})).await;
    assert!(
        text_of(&found).starts_with("foundry memory"),
        "{}",
        text_of(&found)
    );
    assert!(text_of(&found).contains("mem:parity@r2 alice: round trip v2"));
    let unknown =
        mem(json!({"op": "get", "id": "parity", "workspace_id": e.wid, "limit": 3})).await;
    assert_eq!(mcp_code(&unknown), "invalid_argument");
    let foreign = mem(json!({"op": "get", "id": "parity", "workspace_id": "0".repeat(64)})).await;
    assert_eq!(mcp_code(&foreign), "wrong_workspace");
    owner.cancel().await.unwrap();

    // The CLI reads exactly what the MCP owner committed (export is CLI-only).
    let exported = rows(&ok(e.export(&[])));
    assert_eq!(exported.len(), 1);
    assert_eq!(
        (
            exported[0]["revision"].as_u64(),
            exported[0]["text"].as_str()
        ),
        (Some(2), Some("round trip v2"))
    );

    let owner = client(&e, &[]).await;
    let forgot = call(
        &owner,
        "memory",
        json!({"op": "forget", "id": "parity", "workspace_id": e.wid, "expected_revision": 2}),
    )
    .await;
    let forgot: Value = serde_json::from_str(&text_of(&forgot)).unwrap();
    assert_eq!(
        (
            forgot["outcome"].as_str(),
            forgot["removed_revision"].as_u64()
        ),
        (Some("deleted"), Some(2))
    );
    owner.cancel().await.unwrap();
    fails(e.get("parity"), "not_found");
    assert!(rows(&ok(e.export(&[]))).is_empty());
}

// --- T001/T002: failure isolation -------------------------------------------

#[test]
fn broken_lexical_index_still_reads_and_exports_and_a_corrupt_row_is_named() {
    let e = env();
    ok(e.put(&e.body("alive", "still readable", "alice", &[])));
    testkit::corrupt_search_index(&e.store);
    assert_eq!(stdout_json(&ok(e.get("alive")))["text"], "still readable");
    assert_eq!(rows(&ok(e.export(&[]))).len(), 1);
    fails(
        run(&e.store, &["search", "parse_record"], None),
        "repair_required",
    );
    ok(run(&e.store, &["repair-index"], None));

    // A malformed row is named while healthy source retrieval still works.
    testkit::write_raw_memory_row(&e.store, "broken-row", "{not json");
    let named = e.get("broken-row");
    assert!(!named.status.success());
    assert_eq!(code_of(&named), "corrupt_memory");
    assert!(String::from_utf8_lossy(&named.stderr).contains("broken-row"));
    let handle = e.handle("parse_record");
    ok(run(&e.store, &["retrieve", "--handle", &handle], None));
    // Neither forget nor export hides it (export aborts at the first bad row).
    fails(e.forget("broken-row", 1), "corrupt_memory");
    fails(e.export(&[]), "corrupt_memory");
}

// --- T001: schema-3 steps inside the schema-4 upgrade ------------------------

#[test]
fn v2_upgrade_preserves_rows_migrates_pending_keys_and_is_all_or_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    let store = dir.path().join("store");
    testkit::craft_v2_store(&store, Some(&ws));
    fails(run(&store, &["status"], None), "upgrade_required");
    let before = testkit::snapshot(&store);

    // Interrupted before commit: wholly v2, row for row.
    fault::arm(
        names::UPGRADE_BEFORE_COMMIT,
        0,
        Action::Fail("injected".into()),
    );
    Engine::upgrade_store(&store, 4, &Control::unbounded()).unwrap_err();
    fault::disarm_all();
    assert_eq!(testkit::schema_marker(&store), "2");
    assert_eq!(testkit::snapshot(&store), before);
    // `--to 2` and `--to 3` are no longer targets; nothing changed.
    fails(
        run(&store, &["upgrade-store", "--to", "2"], None),
        "unsupported_mode",
    );
    fails(
        run(&store, &["upgrade-store", "--to", "3"], None),
        "unsupported_mode",
    );

    // Interrupted after commit: wholly v4.
    fault::arm(
        names::UPGRADE_AFTER_COMMIT,
        0,
        Action::Fail("injected".into()),
    );
    Engine::upgrade_store(&store, 4, &Control::unbounded()).unwrap_err();
    fault::disarm_all();
    assert_eq!(testkit::schema_marker(&store), "4");
    let after = testkit::snapshot(&store);
    for table in [
        "sources",
        "chunks",
        "feedback",
        "provider_bundles",
        "edges_out",
        "edges_in",
    ] {
        assert_eq!(after[table], before[table], "{table} preserved");
    }
    // The prefix chain migrates losslessly: a raw `source:a` is not
    // overwritten by `a`'s typed destination.
    let sorted = |rows: &Vec<(String, String)>| {
        let mut rows = rows.clone();
        rows.sort();
        rows
    };
    let before_pending = sorted(&before["pending_index"]);
    let after_pending = sorted(&after["pending_index"]);
    assert!(before_pending.contains(&("a".to_owned(), "A".to_owned())));
    assert!(before_pending.contains(&("source:a".to_owned(), "B".to_owned())));
    assert!(before_pending.contains(&("source:source:a".to_owned(), "C".to_owned())));
    assert!(after_pending.contains(&("source:a".to_owned(), "A".to_owned())));
    assert!(after_pending.contains(&("source:source:a".to_owned(), "B".to_owned())));
    assert!(after_pending.contains(&("source:source:source:a".to_owned(), "C".to_owned())));
    assert_eq!(after_pending.len(), before_pending.len());
    assert!(
        !after_pending.iter().any(|(k, _)| k == "a"),
        "no raw key remains"
    );
    // Populated graph and scan_seen rows survive (not vacuously empty).
    assert!(
        !after["edges_out"].is_empty(),
        "a populated graph row exists"
    );
    assert_eq!(after["edges_out"], before["edges_out"]);
    assert_eq!(after["edges_in"], before["edges_in"]);
    assert_eq!(after["scan_seen"], before["scan_seen"]);
    assert!(after["memory"].is_empty());
    for table in testkit::COMPILER_TABLES {
        assert!(after[table].is_empty(), "{table} starts empty");
    }
    assert_eq!(
        testkit::meta_value(&store, "memory_revision").as_deref(),
        Some("0")
    );
    // Re-running the upgrade is a no-op; the status reports schema 4.
    ok(run(&store, &["upgrade-store", "--to", "4"], None));
    assert_eq!(
        stdout_json(&ok(run(&store, &["status"], None)))["schema"],
        4
    );
}

// --- T001/T002: namespaces and the derived index ------------------------------

#[test]
fn a_source_named_memory_x_and_a_memory_record_x_never_affect_each_other() {
    let e = env();
    let colliding = e.ws.join("memory:x");
    std::fs::write(&colliding, "pub fn colliding_source_symbol() {}\n").unwrap();
    e.reindex();
    ok(e.put(&e.body("x", "zebra_marker note", "alice", &[])));
    let source_hits = |e: &Env| {
        stdout_text(&ok(run(
            &e.store,
            &["search", "colliding_source_symbol"],
            None,
        )))
        .lines()
        .count()
    };
    assert_eq!(source_hits(&e), 2, "header + the source hit");
    assert_eq!(mem_lines(&e.search("zebra_marker")).len(), 1);

    // Deleting and re-indexing the source leaves the memory record alone.
    std::fs::remove_file(&colliding).unwrap();
    e.reindex();
    assert_eq!(source_hits(&e), 1, "only the header remains");
    assert_eq!(mem_lines(&e.search("zebra_marker")).len(), 1);
    assert_eq!(stdout_json(&ok(e.get("x")))["revision"], 1);

    // Forgetting the record leaves a re-created source untouched.
    std::fs::write(&colliding, "pub fn colliding_source_symbol() {}\n").unwrap();
    e.reindex();
    ok(e.forget("x", 1));
    assert_eq!(source_hits(&e), 2);
    assert!(mem_lines(&e.search("zebra_marker")).is_empty());
}

#[test]
fn reindex_and_full_repair_preserve_records_and_restore_their_documents() {
    let e = env();
    ok(e.put(&e.body("durable", "findable_term here", "alice", &[])));
    let exported = stdout_text(&ok(e.export(&[])));
    std::fs::write(
        e.ws.join("notes.rs"),
        "pub fn parse_record() -> u32 { 9 }\n",
    )
    .unwrap();
    e.reindex();
    assert_eq!(mem_lines(&e.search("findable_term")).len(), 1);
    testkit::corrupt_search_index(&e.store);
    ok(run(&e.store, &["repair-index"], None));
    let found = e.search("findable_term");
    assert_eq!(
        mem_lines(&found),
        ["mem:durable@r1 alice: findable_term here"]
    );
    assert_eq!(
        stdout_text(&ok(e.export(&[]))),
        exported,
        "records are byte-identical"
    );
}

// --- T002: export pages, forget, recreate, exhaustion --------------------------

#[test]
fn export_pages_concatenate_losslessly_across_restart() {
    let e = env();
    for id in ["a", "b", "c"] {
        ok(e.put(&e.body(id, &format!("text {id}"), "alice", &[])));
    }
    let whole = stdout_text(&ok(e.export(&[])));
    let first = ok(e.export(&["--limit", "2"]));
    let meta: Value = serde_json::from_str(String::from_utf8_lossy(&first.stderr).trim()).unwrap();
    assert_eq!(meta["rows"], 2);
    assert_eq!(meta["next_after_id"], "b");
    let second = ok(e.export(&["--after-id", "b", "--limit", "2"]));
    let meta: Value = serde_json::from_str(String::from_utf8_lossy(&second.stderr).trim()).unwrap();
    assert_eq!(
        (meta["rows"].as_u64(), meta["next_after_id"].is_null()),
        (Some(1), true)
    );
    assert_eq!(
        format!("{}{}", stdout_text(&first), stdout_text(&second)),
        whole
    );
    let ids: Vec<String> = whole
        .lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(ids, ["a", "b", "c"], "sorted by id");
    fails(e.export(&["--limit", "129"]), "invalid_argument");
}

#[test]
fn forget_recreate_and_exhaustion_keep_revisions_monotonic_and_reports_content_free() {
    let e = env();
    let body = |id: &str, author: &str| {
        json!({
            "id": id, "workspace_id": e.wid, "text": format!("{TEXT_CANARY} {id}"),
            "author": author, "provenance": PROVENANCE_CANARY, "source_links": [],
        })
    };
    ok(e.put(&body("a", AUTHOR_CANARY)));
    ok(e.put(&body("b", AUTHOR_CANARY)));
    let mismatch = e.forget("a", 99);
    let mismatch = stdout_json(&ok(mismatch));
    assert_eq!(mismatch["outcome"], "conflict");
    let forgot = ok(e.forget("b", 2));
    let rendered = stdout_text(&forgot);
    assert_eq!(stdout_json(&forgot)["outcome"], "deleted");
    assert_eq!(stdout_json(&forgot)["removed_revision"], 2);
    let exported = ok(e.export(&[]));
    for canary in [TEXT_CANARY, AUTHOR_CANARY, PROVENANCE_CANARY] {
        assert!(!rendered.contains(canary), "forget result carries {canary}");
        assert!(
            !String::from_utf8_lossy(&exported.stderr).contains(canary),
            "export metadata carries {canary}"
        );
    }
    assert_eq!(rows(&exported).len(), 1, "forgotten rows leave export");
    fails(e.get("b"), "not_found");
    assert!(
        mem_lines(&e.search(TEXT_CANARY))
            .iter()
            .all(|l| !l.starts_with("mem:b@"))
    );
    // Recreate with the same text but a different author: a strictly greater
    // revision; the old update and forget are conflicts.
    let recreated = stdout_json(&ok(e.put(&body("b", "someone-else"))));
    assert_eq!(recreated["revision"], 3);
    fails(e.update(&body("b", "someone-else"), 2), "conflict");
    assert_eq!(stdout_json(&ok(e.forget("b", 2)))["outcome"], "conflict");
    // Delete the last rows, restart, recreate: the counter is never reused.
    ok(e.forget("a", 1));
    ok(e.forget("b", 3));
    assert!(rows(&ok(e.export(&[]))).is_empty());
    assert_eq!(e.next_revision("fresh"), 4);
    // Exhaustion changes nothing.
    ok(e.forget("fresh", 4));
    testkit::set_meta(&e.store, "memory_revision", Some(&u64::MAX.to_string()));
    let before = testkit::snapshot(&e.store);
    fails(e.put(&body("never", "alice")), "revision_exhausted");
    assert_eq!(testkit::snapshot(&e.store), before);
}

#[test]
fn interrupted_forget_and_stale_documents_never_leak() {
    let e = env();
    ok(e.put(&e.body("hold", "interrupt_term text", "alice", &[])));
    let mut engine = Engine::open_existing(&e.store).unwrap();
    let input = context_foundry::memory::ForgetInput {
        id: "hold".into(),
        workspace_id: e.wid.clone(),
        expected_revision: 1,
    };
    // Before the commit the exact prior row stays.
    fault::arm(
        names::MEMORY_FORGET_BEFORE_COMMIT,
        0,
        Action::Fail("injected".into()),
    );
    engine
        .memory_forget(&input, &Control::unbounded())
        .unwrap_err();
    fault::disarm_all();
    assert_eq!(engine.memory_get("hold", &e.wid).unwrap().revision, 1);
    // After the commit get is not_found and search/export omit it even though
    // the derived document is still in the index (the drain has not run).
    fault::arm(
        names::MEMORY_FORGET_AFTER_COMMIT,
        0,
        Action::Fail("injected".into()),
    );
    engine
        .memory_forget(&input, &Control::unbounded())
        .unwrap_err();
    fault::disarm_all();
    assert_eq!(
        engine.memory_get("hold", &e.wid).unwrap_err().code(),
        "not_found"
    );
    let search = context_foundry::memory::SearchInput {
        query: "interrupt_term".into(),
        workspace_id: e.wid.clone(),
        limit: 10,
        tokens: 1024,
    };
    let outcome = engine.memory_search(&search).unwrap();
    assert!(outcome.hits.is_empty());
    assert_eq!(
        outcome.stale_candidates, 1,
        "the stale document failed validation"
    );
    assert!(
        engine
            .memory_export(&e.wid, None, 128)
            .unwrap()
            .rows
            .is_empty()
    );
    // Enabled context validates memory in the SAME final read: the stale
    // document is dropped there and counted in the header's `stale:`. The
    // engine holds the store, so this reads through the same engine.
    let combined = engine
        .context_candidates_memory(
            "interrupt_term",
            context_foundry::laya::Strategy::Auto,
            &Control::unbounded(),
        )
        .unwrap();
    let context = context_foundry::response::pack_context_with_memory(
        &combined.batch,
        &combined.hits,
        context_foundry::response::Budget::request(2048),
        &context_foundry::response::stdout_bytes,
    )
    .unwrap()
    .text;
    assert!(
        context.lines().next().unwrap().contains("stale:1"),
        "{}",
        context.lines().next().unwrap()
    );
    // A recreation gets a new revision; the stale revision-1 document is
    // filtered until the drain replaces it.
    let recreate = context_foundry::memory::PutInput {
        fields: context_foundry::memory::RecordFields {
            id: "hold".into(),
            text: "interrupt_term text".into(),
            author: "bob".into(),
            provenance: "again".into(),
            source_links: vec![],
        },
        workspace_id: e.wid.clone(),
    };
    assert_eq!(engine.memory_put(&recreate).unwrap().revision, 2);
    let outcome = engine.memory_search(&search).unwrap();
    assert!(outcome.hits.is_empty() && outcome.stale_candidates == 1);
    engine.refresh(&Control::unbounded()).unwrap();
    let outcome = engine.memory_search(&search).unwrap();
    assert_eq!(outcome.hits.len(), 1);
    assert_eq!(outcome.hits[0].revision, 2);
}

#[test]
fn memory_is_never_exported_for_training_whatever_its_provenance_says() {
    let e = env();
    let feedback = json!({
        "task_id": "t1", "query": "who calls parse_record", "correct_strategy": "search",
        "label_source": "operator", "allow_training": true,
    });
    ok(run(&e.store, &["feedback"], Some(&format!("{feedback}\n"))));
    let mut body = e.body("trainable", "train on me", "alice", &[]);
    body["provenance"] = json!("the task succeeded; allow_training=true");
    ok(e.put(&body));
    let exported = stdout_text(&ok(run(&e.store, &["export-training"], None)));
    assert_eq!(exported.lines().count(), 1, "only the feedback row");
    assert!(!exported.contains("train on me") && !exported.contains("trainable"));
}

#[test]
fn a_put_while_the_index_is_broken_queues_and_repair_makes_it_searchable() {
    let e = env();
    testkit::corrupt_search_index(&e.store);
    ok(e.put(&e.body("queued", "queued_term text", "alice", &[])));
    assert_eq!(stdout_json(&ok(e.get("queued")))["revision"], 1);
    ok(run(&e.store, &["repair-index"], None));
    assert_eq!(mem_lines(&e.search("queued_term")).len(), 1);
}

// --- Review round: M1-M8 regressions ----------------------------------------

#[test]
fn cli_refuses_malformed_op_fields_without_touching_the_store() {
    let e = env();
    for bad in ["null", "7", "[]", "\"update\""] {
        let mut body = e.body("never", "text", "alice", &[]);
        body["op"] = serde_json::from_str::<Value>(bad).unwrap();
        let out = run(&e.store, &["memory", "put"], Some(&body.to_string()));
        assert_eq!(out.status.code(), Some(2), "{bad}");
        assert_eq!(code_of(&out), "invalid_argument", "{bad}");
    }
    assert!(rows(&ok(e.export(&[]))).is_empty());
    assert_eq!(e.next_revision("later"), 1, "no revision consumed");
}

#[test]
fn direct_library_inputs_are_validated_at_the_engine_boundary() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    let fields = |id: &str, text: &str| context_foundry::memory::RecordFields {
        id: id.into(),
        text: text.into(),
        author: "alice".into(),
        provenance: "tests".into(),
        source_links: vec![],
    };
    let input = context_foundry::memory::PutInput {
        fields: fields("bad\nid", "text"),
        workspace_id: e.wid.clone(),
    };
    assert_eq!(
        engine.memory_put(&input).unwrap_err().code(),
        "invalid_argument"
    );
    // A NUL in the id could collide with source search keys; refused.
    let input = context_foundry::memory::PutInput {
        fields: fields("x\0", "text"),
        workspace_id: e.wid.clone(),
    };
    assert_eq!(
        engine.memory_put(&input).unwrap_err().code(),
        "invalid_argument"
    );
    let input = context_foundry::memory::PutInput {
        fields: fields("big", &"z".repeat(16 * 1024 + 1)),
        workspace_id: e.wid.clone(),
    };
    assert_eq!(
        engine.memory_put(&input).unwrap_err().code(),
        "invalid_argument"
    );
    // Too many links is refused before the store is touched.
    let links: Vec<String> = (0..9)
        .map(|i| {
            let ws16 = format!("{i:016x}");
            format!(
                "notes.rs#0-1@{}.{}",
                &context_foundry::digest(b"x")[..32],
                ws16
            )
        })
        .collect();
    let input = context_foundry::memory::PutInput {
        fields: context_foundry::memory::RecordFields {
            id: "many".into(),
            text: "text".into(),
            author: "alice".into(),
            provenance: "tests".into(),
            source_links: links,
        },
        workspace_id: e.wid.clone(),
    };
    assert_eq!(
        engine.memory_put(&input).unwrap_err().code(),
        "invalid_argument"
    );
    drop(engine);
    assert_eq!(e.next_revision("fine"), 1, "nothing was written");
}

#[test]
fn retries_after_link_changes_keep_their_specified_outcomes() {
    let e = env();
    let handle = e.handle("parse_record");
    let body = e.body(
        "retry",
        "linked note",
        "alice",
        std::slice::from_ref(&handle),
    );
    ok(e.put(&body));
    std::fs::write(
        e.ws.join("notes.rs"),
        "pub fn parse_record() -> u32 { 8 }\n",
    )
    .unwrap();
    e.reindex();
    // Identical put after the linked source changed: unchanged, not stale.
    let again = stdout_json(&ok(e.put(&body)));
    assert_eq!(
        (again["revision"].as_u64(), again["outcome"].as_str()),
        (Some(1), Some("unchanged"))
    );
    // Consume revision 1 with a real (linkless) update, delete the linked
    // source, then retry the ORIGINAL update (expected 1, stale link): the
    // consumed revision conflicts before any link is examined.
    let update = e.body("retry", "v2", "alice", &[]);
    let updated = stdout_json(&ok(e.update(&update, 1)));
    assert_eq!(updated["revision"], 2);
    std::fs::remove_file(e.ws.join("notes.rs")).unwrap();
    e.reindex();
    let mut retry = e.body("retry", "v3", "alice", &[handle]);
    retry["expected_revision"] = json!(1);
    fails(
        run(&e.store, &["memory", "update"], Some(&retry.to_string())),
        "conflict",
    );
}

#[test]
fn memory_window_reports_candidates_full_and_stale_filled_windows() {
    let mut fx = testkit::new_fixture();
    fx.add(&[("notes.rs", SOURCE)]);
    let ws_id = fx.engine.workspace_id().unwrap();
    let put = |fx: &mut testkit::Fixture, id: &str| {
        let fields = context_foundry::memory::RecordFields {
            id: id.into(),
            text: format!("window_probe note {id}"),
            author: "alice".into(),
            provenance: "tests".into(),
            source_links: vec![],
        };
        fx.engine
            .memory_put(&context_foundry::memory::PutInput {
                fields,
                workspace_id: ws_id.clone(),
            })
            .unwrap();
    };
    for i in 0..255 {
        put(&mut fx, &format!("m{i:03}"));
    }
    fx.drain();
    let search = |engine: &Engine| {
        engine
            .memory_search(&context_foundry::memory::SearchInput {
                query: "window_probe".into(),
                workspace_id: ws_id.clone(),
                limit: 10,
                tokens: 1024,
            })
            .unwrap()
    };
    let outcome = search(&fx.engine);
    assert_eq!(outcome.hits.len(), 10);
    assert!(!outcome.candidates_full, "255 candidates fit the window");
    put(&mut fx, "m255");
    fx.drain();
    assert!(search(&fx.engine).candidates_full, "256 fills the window");
    put(&mut fx, "m256");
    fx.drain();
    assert!(
        search(&fx.engine).candidates_full,
        "257 overflows the window"
    );
    // A window filled with stale documents reports stale coverage, not silence.
    let (_dir, store, _root) = fx.close();
    {
        let engine = Engine::open_existing(&store).unwrap();
        let input = context_foundry::memory::ForgetInput {
            id: "m000".into(),
            workspace_id: ws_id.clone(),
            expected_revision: 1,
        };
        // Forget through the core so the derived documents stay stale.
        context_foundry::fault::arm(
            names::MEMORY_FORGET_AFTER_COMMIT,
            0,
            Action::Fail("x".into()),
        );
        let _ = engine.memory_forget(&input, &Control::unbounded());
        context_foundry::fault::disarm_all();
        let outcome = search(&engine);
        assert_eq!(outcome.hits.len(), 10, "live records still fill the page");
        assert_eq!(
            outcome.stale_candidates, 1,
            "the drop is reported, not silent"
        );
        assert!(outcome.candidates_full, "the window is still full");
        drop(engine);
    }
    let _ = store;
}

#[test]
fn pending_and_drain_counts_stay_per_namespace() {
    let e = env();
    let mut engine = Engine::open_existing(&e.store).unwrap();
    // One queued source key and one queued memory key at once: status counts
    // the total, each header counts only its own namespace.
    engine
        .replace_source("notes.rs", "pub fn parse_record() -> u32 { 8 }\n")
        .unwrap();
    let put = context_foundry::memory::PutInput {
        fields: context_foundry::memory::RecordFields {
            id: "lagging-note".into(),
            text: "lagging_term".into(),
            author: "alice".into(),
            provenance: "tests".into(),
            source_links: vec![],
        },
        workspace_id: e.wid.clone(),
    };
    engine.memory_put(&put).unwrap();
    assert_eq!(engine.status().unwrap().pending_count, 2);
    let header = |text: &str| text.lines().next().unwrap().to_owned();
    let source_header = header(&stdout_text_local(&engine, "parse_record"));
    assert!(source_header.contains("pending:1"), "{source_header}");
    let memory = engine
        .memory_search(&context_foundry::memory::SearchInput {
            query: "lagging_term".into(),
            workspace_id: e.wid.clone(),
            limit: 10,
            tokens: 1024,
        })
        .unwrap();
    let packed = context_foundry::response::pack_memory_search(
        &memory,
        context_foundry::response::Budget::request(1024),
        &context_foundry::response::stdout_bytes,
    )
    .unwrap();
    assert!(
        header(&packed.text).contains("pending:1"),
        "{}",
        header(&packed.text)
    );
    // After the drain both namespaces are quiet.
    engine.refresh(&Control::unbounded()).unwrap();
    assert_eq!(engine.status().unwrap().pending_count, 0);
    drop(engine);
}

fn stdout_text_local(engine: &Engine, query: &str) -> String {
    let outcome = engine.search_in(query, None, 10).unwrap();
    context_foundry::response::pack_search(
        &outcome,
        context_foundry::response::Budget::request(1024),
        &context_foundry::response::stdout_bytes,
    )
    .unwrap()
    .text
}

#[test]
fn empty_tail_packing_is_byte_identical_to_plain_context() {
    let mut fx = testkit::new_fixture();
    fx.add(&[("notes.rs", SOURCE)]);
    let batch = fx
        .engine
        .context_candidates(
            "parse_record",
            context_foundry::laya::Strategy::Auto,
            &Control::unbounded(),
        )
        .unwrap();
    let plain = context_foundry::response::pack_context(
        &batch,
        context_foundry::response::Budget::request(2048),
        &context_foundry::response::stdout_bytes,
    )
    .unwrap();
    let with_empty = context_foundry::response::pack_context_with_memory(
        &batch,
        &[],
        context_foundry::response::Budget::request(2048),
        &context_foundry::response::stdout_bytes,
    )
    .unwrap();
    assert_eq!(plain.text, with_empty.text);
    assert_eq!(plain.tokens, with_empty.tokens);
}

#[tokio::test]
async fn multi_root_memory_follows_primary_selection_and_never_fails_the_response() {
    let primary = env();
    ok(primary.put(&primary.body("primary-note", "parse_record note", "alice", &[])));
    let reference = env();
    let args = vec![
        "--reference".to_owned(),
        format!("{}={}", reference.ws.display(), reference.store.display()),
    ];
    let owner = client_with(&primary, &args).await;
    // Reference-only selection: no primary memory lines under its header.
    let ref_only = call(
        &owner,
        "context",
        json!({"query": "parse_record", "roots": ["ref1"], "include_memory": true}),
    )
    .await;
    assert_eq!(ref_only.is_error, Some(false), "{}", text_of(&ref_only));
    assert!(
        !text_of(&ref_only).contains("mem:"),
        "{}",
        text_of(&ref_only)
    );
    owner.cancel().await.unwrap();
    ok(run(&primary.store, &["status"], None));

    // A broken primary index must not fail an include_memory response: the
    // reference serves and the primary keeps its coverage segment.
    testkit::corrupt_search_index(&primary.store);
    let owner = client_with(&primary, &args).await;
    let served = call(
        &owner,
        "context",
        json!({"query": "parse_record", "roots": ["ref1"], "include_memory": true}),
    )
    .await;
    assert_eq!(served.is_error, Some(false), "{}", text_of(&served));
    assert!(!text_of(&served).contains("mem:"));
    owner.cancel().await.unwrap();
}

async fn client_with(e: &Env, extra: &[String]) -> Client {
    let mut command = tokio::process::Command::new(BIN);
    command
        .arg("--store")
        .arg(&e.store)
        .arg("mcp")
        .arg("--root")
        .arg(&e.ws)
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    ().serve(TokioChildProcess::new(command).unwrap())
        .await
        .unwrap()
}

// --- Review round 2: M8-M12 and the export byte cut -------------------------

#[test]
fn m9_links_validate_against_verified_source_bytes() {
    let mut fx = testkit::new_fixture();
    // The source ends in a two-byte `é`, so `len - 1` falls inside it.
    const NOTES: &str = "pub const NOTE: &str = \"x\"; // café";
    fx.add(&[("notes.rs", NOTES)]);
    let ws = fx.engine.workspace_id().unwrap();
    let body = NOTES.as_bytes();
    let len = body.len();
    // A handle ending mid-codepoint: valid grammar, invalid range.
    let mid = context_foundry::store::HandleRef::parse(&format!(
        "notes.rs#0-{}@{}.{}",
        len + 1,
        &context_foundry::digest(body)[..32],
        &ws[..16]
    ));
    // The digest matches, so this reaches the range check only if the
    // endOffset lands inside the final multi-byte character.
    let mid = {
        let parsed = mid.unwrap();
        let shifted = context_foundry::store::HandleRef {
            path: parsed.path.clone(),
            start: 0,
            end: len as u64 - 1,
            sha32: parsed.sha32.clone(),
            ws16: parsed.ws16.clone(),
        };
        shifted.to_string()
    };
    let input = context_foundry::memory::PutInput {
        fields: context_foundry::memory::RecordFields {
            id: "linked".into(),
            text: "note".into(),
            author: "alice".into(),
            provenance: "tests".into(),
            source_links: vec![mid],
        },
        workspace_id: ws.clone(),
    };
    let err = fx.engine.memory_put(&input).unwrap_err();
    assert_eq!(err.code(), "invalid_range", "mid-codepoint: {err}");
    // A missing chunk refuses the mutation as corrupt_source. The raw edit
    // needs the database closed, so the engine is reopened afterwards.
    let (_dir, store, _root) = fx.close();
    testkit::remove_chunk(&store, "notes.rs", 0);
    let engine = context_foundry::Engine::open_existing(&store).unwrap();
    let good = context_foundry::store::SourceHandle {
        workspace_id: ws.clone(),
        path: "notes.rs".into(),
        sha256: context_foundry::digest(body),
        start: 0,
        end: len as u64,
    }
    .to_v2();
    let input = context_foundry::memory::PutInput {
        fields: context_foundry::memory::RecordFields {
            id: "linked".into(),
            text: "note".into(),
            author: "alice".into(),
            provenance: "tests".into(),
            source_links: vec![good.clone()],
        },
        workspace_id: ws.clone(),
    };
    assert_eq!(
        engine.memory_put(&input).unwrap_err().code(),
        "corrupt_source"
    );
    // Nothing was written: row, counter and pending are unchanged.
    let status = engine.status().unwrap();
    assert_eq!((status.pending_count, status.source_count), (0, 1));
    let fields = context_foundry::memory::RecordFields {
        id: "clean".into(),
        text: "note".into(),
        author: "alice".into(),
        provenance: "tests".into(),
        source_links: vec![],
    };
    let input = context_foundry::memory::PutInput {
        fields,
        workspace_id: ws.clone(),
    };
    assert_eq!(engine.memory_put(&input).unwrap().revision, 1);
    // The same refusal applies to update.
    let update = context_foundry::memory::UpdateInput {
        fields: context_foundry::memory::RecordFields {
            id: "clean".into(),
            text: "v2".into(),
            author: "alice".into(),
            provenance: "tests".into(),
            source_links: vec![good],
        },
        workspace_id: ws.clone(),
        expected_revision: 1,
    };
    assert_eq!(
        engine.memory_update(&update).unwrap_err().code(),
        "corrupt_source"
    );
}

#[test]
fn m10_corrupt_memory_rows_fail_search_and_context_never_succeed() {
    let e = env();
    ok(e.put(&e.body("victim", "findable_term note", "alice", &[])));
    // Overwrite the live row with valid JSON naming a DIFFERENT id: the
    // derived document still matches, but validation must name corruption.
    let mut wrong_id = e.body("victim", "findable_term note", "alice", &[]);
    wrong_id["id"] = json!("other");
    wrong_id["revision"] = json!(1);
    testkit::write_raw_memory_row(&e.store, "victim", &wrong_id.to_string());
    fails(
        run(
            &e.store,
            &[
                "memory",
                "search",
                "findable_term",
                "--workspace-id",
                &e.wid,
            ],
            None,
        ),
        "corrupt_memory",
    );
    // Enabled context fails the whole request on the corrupt row: no success
    // text is produced (context-v2 § Failure scope).
    fails(
        run(
            &e.store,
            &["context", "findable_term", "--include-memory"],
            None,
        ),
        "corrupt_memory",
    );
    // An LF in the stored id is the same corruption.
    let mut lf_id = wrong_id.clone();
    lf_id["id"] = json!("vic\ntim");
    testkit::write_raw_memory_row(&e.store, "victim", &lf_id.to_string());
    fails(e.get("victim"), "corrupt_memory");
}

#[tokio::test]
async fn m11_multi_root_memory_propagates_corruption_and_keeps_coverage() {
    let primary = env();
    ok(primary.put(&primary.body("primary-note", "parse_record note", "alice", &[])));
    let reference = env();
    let args = vec![
        "--reference".to_owned(),
        format!("{}={}", reference.ws.display(), reference.store.display()),
    ];
    // Corruption in the primary's matching memory fails the request.
    let mut wrong = primary.body("primary-note", "parse_record note", "alice", &[]);
    wrong["id"] = json!("mismatch");
    wrong["revision"] = json!(1);
    testkit::write_raw_memory_row(&primary.store, "primary-note", &wrong.to_string());
    let owner = client_with(&primary, &args).await;
    let corrupted = call(
        &owner,
        "context",
        json!({"query": "parse_record", "include_memory": true}),
    )
    .await;
    assert_eq!(
        mcp_code(&corrupted),
        "corrupt_memory",
        "{}",
        text_of(&corrupted)
    );
    owner.cancel().await.unwrap();
    ok(run(&primary.store, &["status"], None));

    // An unavailable primary with no selector: the reference still serves,
    // without memory lines, and the primary's coverage stays in the header.
    testkit::corrupt_search_index(&primary.store);
    let owner = client_with(&primary, &args).await;
    let served = call(
        &owner,
        "context",
        json!({"query": "parse_record", "include_memory": true}),
    )
    .await;
    assert_eq!(served.is_error, Some(false), "{}", text_of(&served));
    let header = text_of(&served).lines().next().unwrap().to_owned();
    assert!(header.contains("primary"), "{header}");
    assert!(!text_of(&served).contains("mem:"), "{header}");
    owner.cancel().await.unwrap();
}

#[test]
fn m12_public_search_inputs_are_bounded() {
    let e = env();
    let engine = Engine::open_existing(&e.store).unwrap();
    for limit in [0usize, 65] {
        let err = engine.memory_hits("parse_record", limit).unwrap_err();
        assert_eq!(err.code(), "invalid_argument", "limit {limit}");
    }
    for tokens in [0usize, 32769] {
        let input = context_foundry::memory::SearchInput {
            query: "parse_record".into(),
            workspace_id: e.wid.clone(),
            limit: 10,
            tokens,
        };
        let err = engine.memory_search(&input).unwrap_err();
        assert_eq!(err.code(), "invalid_argument", "tokens {tokens}");
    }
}

#[test]
fn m8_source_readers_count_only_source_pending() {
    let e = env();
    // A broken derived index blocks the put's best-effort self-drain, so the
    // memory key is the only pending work.
    testkit::corrupt_search_index(&e.store);
    ok(e.put(&e.body("lag-note", "lagging_term", "alice", &[])));
    {
        let engine = Engine::open_existing(&e.store).unwrap();
        assert_eq!(engine.pending_source_work().unwrap(), 0);
        assert_eq!(engine.pending().unwrap(), 1);
    }
    // A scan whose drain stays blocked reports source work only.
    let indexed = run(&e.store, &["index", e.ws.to_str().unwrap()], None);
    assert_eq!(
        stdout_json(&indexed)["pending_sources"],
        0,
        "scan counts source work only"
    );
    // Repair rebuilds both namespaces and reports the split.
    let repaired = stdout_json(&ok(run(&e.store, &["repair-index"], None)));
    assert_eq!(
        (
            repaired["drained_sources"].as_u64(),
            repaired["drained_memory"].as_u64()
        ),
        // Repair re-enqueues the live record under the same `memory:` key it
        // was already pending on: one source and one memory key drain.
        (Some(1), Some(1)),
        "{repaired}"
    );
    // With a healthy index, a library put (no self-drain) leaves only memory
    // work: the source header ignores it, the memory header counts it, and
    // the drain returns the split.
    {
        let engine = Engine::open_existing(&e.store).unwrap();
        let input = context_foundry::memory::PutInput {
            fields: context_foundry::memory::RecordFields {
                id: "lag-two".into(),
                text: "lagging_term again".into(),
                author: "alice".into(),
                provenance: "tests".into(),
                source_links: vec![],
            },
            workspace_id: e.wid.clone(),
        };
        engine.memory_put(&input).unwrap();
        assert_eq!(engine.pending_source_work().unwrap(), 0);
        assert_eq!(engine.pending().unwrap(), 1);
    }
    let source_header = stdout_text(&ok(run(&e.store, &["search", "parse_record"], None)))
        .lines()
        .next()
        .unwrap()
        .to_owned();
    assert!(!source_header.contains("pending:"), "{source_header}");
    let memory_header = e.search("lagging_term").lines().next().unwrap().to_owned();
    assert!(memory_header.contains("pending:1"), "{memory_header}");
    let mut engine = Engine::open_existing(&e.store).unwrap();
    assert_eq!(engine.refresh(&Control::unbounded()).unwrap(), (0, 1));
}

#[test]
fn export_stops_at_the_4mib_byte_cut_and_resumes_losslessly() {
    let mut fx = testkit::new_fixture();
    // A source with a ~2.5 KiB path: each full handle is ~2.6 KiB, so eight
    // links plus a 16 KiB text make every row ~38 KiB and 128 rows exceed
    // the 4 MiB page cap long before the row cap.
    let long_path = format!("{}.rs", "p".repeat(2500));
    fx.add(&[(long_path.as_str(), SOURCE)]);
    let ws = fx.engine.workspace_id().unwrap();
    let handle = context_foundry::store::SourceHandle {
        workspace_id: ws.clone(),
        path: long_path.clone(),
        sha256: context_foundry::digest(SOURCE.as_bytes()),
        start: 0,
        end: SOURCE.len() as u64,
    }
    .to_v2();
    let links: Vec<String> = std::iter::repeat_n(handle, 8).collect();
    for i in 0..128 {
        let fields = context_foundry::memory::RecordFields {
            id: format!("big{i:03}"),
            text: "t".repeat(16 * 1024 - 32),
            author: "alice".into(),
            provenance: "tests".into(),
            source_links: links.clone(),
        };
        fx.engine
            .memory_put(&context_foundry::memory::PutInput {
                fields,
                workspace_id: ws.clone(),
            })
            .unwrap();
    }
    let mut collected = Vec::new();
    let mut after: Option<String> = None;
    let mut pages = Vec::new();
    loop {
        let page = fx.engine.memory_export(&ws, after.as_deref(), 128).unwrap();
        assert!(page.bytes <= 4 * 1024 * 1024, "{}", page.bytes);
        let rendered: usize = page.rows.iter().map(String::len).sum();
        assert_eq!(page.bytes, rendered, "bytes are the rendered rows' sum");
        pages.push((page.rows.len(), page.next_after_id.is_some()));
        collected.extend(page.rows.clone());
        match page.next_after_id {
            Some(cursor) => after = Some(cursor),
            None => break,
        }
    }
    // 128 rows of ~37 KiB exceed 4 MiB, so the byte cut (not the 128-row
    // cap) ends the first page, with a cursor, and a second page follows.
    assert!(pages.len() >= 2, "{pages:?}");
    assert!(
        pages[0].0 > 0 && pages[0].0 < 128 && pages[0].1,
        "{pages:?}"
    );
    assert_eq!(collected.len(), 128, "every row is exported exactly once");
    // The raw read needs the database closed.
    let (_dir, store, _root) = fx.close();
    let whole: Vec<String> = testkit::memory_rows(&store)
        .into_iter()
        .map(|(_, v)| format!("{v}\n"))
        .collect();
    assert_eq!(collected, whole, "pages concatenate losslessly");
}
