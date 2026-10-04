//! 007 T001 verification: multi-root admission, aliases, coverage, merged
//! search/context, per-root headers and status, root-scoped identity.
//!
//! Three temporary repositories per combined fixture, one identifier defined
//! in two roots, a reference store held by another (test) process, admission
//! refusals before serving (including a `ws16` collision through the
//! test-faults seam), one budget and one charge per response, and a stalled
//! second root failing the whole request while holding the single engine
//! slot. Identifiers in different roots share no `foundry_code` subtoken of
//! 2+ characters except the deliberately shared `shared_anchor`.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use context_foundry::store::HandleRef;
use context_foundry::testkit::parse_v2;

use rmcp::{ServiceExt, model::CallToolRequestParams, transport::TokioChildProcess};
use tokio::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_foundry");
const FAULTS_BIN: &str = env!("CARGO_BIN_EXE_foundry-faults");
/// Longer than the 5000 ms read deadline: the library call returns only
/// after the deadline passed, leaving a window in which the slot is held.
const STALL_MS: u64 = 7_000;
const READ_DEADLINE_MS: u64 = 5_000;

const PRIMARY_LIB: &str = "pub fn shared_anchor(input: &str) -> Option<&str> {\n    input.split_once('=').map(|(_, v)| v)\n}\n\npub fn primary_grip_wide(n: u8) -> u8 {\n    n.wrapping_add(1)\n}\n";
const REF1_LIB: &str = "pub fn shared_anchor(input: &str) -> Option<&str> {\n    input.split_once(':').map(|(_, v)| v)\n}\n\npub fn delta_signal_pad() -> usize {\n    41\n}\n";
const REF2_LIB: &str = "pub fn kappa_probe_quiet() -> usize {\n    97\n}\n";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The `ws16` of a canonical root, as the store computes it.
fn ws16_of(root: &Path) -> String {
    context_foundry::workspace_id_for_root(root).unwrap()[..16].to_owned()
}

fn bootstrap_apply(store: &Path, root: &Path) {
    let output = std::process::Command::new(BIN)
        .arg("--store")
        .arg(store)
        .arg("bootstrap")
        .arg("--root")
        .arg(root)
        .arg("--apply")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "bootstrap apply failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// One temporary repository with `src/lib.rs`, bootstrapped into its own
/// store under the fixture directory.
struct Repo {
    root: PathBuf,
    store: PathBuf,
}

fn repo(parent: &Path, name: &str, lib: &str) -> Repo {
    let root = parent.join(name);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), lib).unwrap();
    let store = parent.join(format!("{name}-store"));
    bootstrap_apply(&store, &root);
    Repo { root, store }
}

/// A repo whose store is never created (a `missing_store` reference).
fn unbootstrapped_repo(parent: &Path, name: &str, lib: &str) -> Repo {
    let root = parent.join(name);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), lib).unwrap();
    Repo {
        store: parent.join(format!("{name}-store")),
        root,
    }
}

fn reference_arg(root: &Path, store: &Path) -> String {
    format!("{}={}", root.display(), store.display())
}

/// A real SDK stdio client against a multi-root owner.
async fn stdio_owner(
    store: &Path,
    root: &Path,
    references: &[String],
    extra_args: &[String],
) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let mut command = Command::new(BIN);
    command
        .arg("--store")
        .arg(store)
        .args(["mcp", "--root"])
        .arg(root)
        .args(
            references
                .iter()
                .flat_map(|reference| ["--reference".to_owned(), reference.clone()]),
        )
        .args(extra_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let transport = TokioChildProcess::new(command).unwrap();
    ().serve(transport).await.unwrap()
}

async fn call(
    peer: &rmcp::service::Peer<rmcp::RoleClient>,
    tool: &'static str,
    arguments: Option<serde_json::Value>,
) -> rmcp::model::CallToolResult {
    let mut request = CallToolRequestParams::new(tool);
    if let Some(arguments) = arguments {
        request = request.with_arguments(arguments.as_object().unwrap().clone());
    }
    peer.call_tool(request).await.unwrap()
}

fn text_of(result: &rmcp::model::CallToolResult) -> String {
    let rmcp::model::ContentBlock::Text(text) = &result.content[0] else {
        panic!("expected one text block");
    };
    text.text.to_string()
}

fn assert_success(result: &rmcp::model::CallToolResult) -> String {
    assert_eq!(
        result.is_error,
        Some(false),
        "unexpected tool error: {}",
        text_of(result)
    );
    assert_eq!(result.content.len(), 1);
    text_of(result)
}

fn bounded_error(result: &rmcp::model::CallToolResult) -> (String, String) {
    assert_eq!(result.is_error, Some(true), "expected isError:true");
    let value: serde_json::Value = serde_json::from_str(&text_of(result)).unwrap();
    (
        value["code"].as_str().unwrap().to_owned(),
        value["message"].as_str().unwrap().to_owned(),
    )
}

fn count_tokens(text: &str) -> u64 {
    tiktoken_rs::o200k_base_singleton()
        .encode_ordinary(text)
        .len() as u64
}

/// The ` · `-joined header segments of a v2 text.
fn header_segments(text: &str) -> Vec<String> {
    text.lines()
        .next()
        .expect("a header line")
        .split(" · ")
        .map(str::to_owned)
        .collect()
}

/// The per-root segments (between the operation and the `budget:` segment).
fn root_segments(text: &str) -> Vec<String> {
    let segments = header_segments(text);
    let budget_at = segments
        .iter()
        .position(|segment| segment.starts_with("budget:"))
        .expect("a budget segment");
    segments[1..budget_at].to_vec()
}

/// The v2 handles of a response's items, parsed.
fn item_handles(text: &str) -> Vec<HandleRef> {
    parse_v2(text)
        .unwrap_or_else(|e| panic!("not a v2 text ({e}):\n{text}"))
        .items
        .into_iter()
        .filter(|item| !item.handle.is_empty())
        .filter_map(|item| HandleRef::parse(&item.handle).ok())
        .collect()
}

/// A budget policy file granting a session allowance above the requests used.
fn session_budget_file(dir: &Path, allowance: u64) -> PathBuf {
    let file = dir.join("budget.json");
    std::fs::write(
        &file,
        serde_json::json!({
            "foundry_budget": {
                "v": 1,
                "max_context_tokens": 32768,
                "session_context_tokens": allowance
            }
        })
        .to_string(),
    )
    .unwrap();
    file
}

/// The standard three-root fixture: `shared_anchor` lives in the primary and
/// ref1 (`foo`); ref2 (`foo~2`) has its own symbol.
struct Fixture {
    primary: Repo,
    ref1: Repo,
    ref2: Repo,
}

fn fixture(dir: &Path) -> Fixture {
    Fixture {
        primary: repo(dir, "ws0", PRIMARY_LIB),
        ref1: repo(dir, "foo", REF1_LIB),
        ref2: repo(dir, "foo~2", REF2_LIB),
    }
}

impl Fixture {
    fn references(&self) -> Vec<String> {
        vec![
            reference_arg(&self.ref1.root, &self.ref1.store),
            reference_arg(&self.ref2.root, &self.ref2.store),
        ]
    }
}

/// Run `foundry mcp` to a startup outcome and return `(exit code, error JSON)`.
fn launch_refusal(store: &Path, root: &Path, references: &[String]) -> (i32, serde_json::Value) {
    let mut command = std::process::Command::new(BIN);
    command
        .arg("--store")
        .arg(store)
        .args(["mcp", "--root"])
        .arg(root)
        .args(
            references
                .iter()
                .flat_map(|reference| ["--reference".to_owned(), reference.clone()]),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = command.output().unwrap();
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap_or_else(|e| {
        panic!(
            "stderr is error JSON: {e}: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output.status.code().unwrap(), error)
}

// ---------------------------------------------------------------------------
// Admission refusals before serving
// ---------------------------------------------------------------------------

#[test]
fn admission_refuses_before_serving_with_named_codes() {
    let dir = tempfile::tempdir().unwrap();
    let primary = unbootstrapped_repo(dir.path(), "ws0", PRIMARY_LIB);
    let a = unbootstrapped_repo(dir.path(), "a", REF1_LIB);
    let b = unbootstrapped_repo(dir.path(), "b", REF2_LIB);
    let nested = {
        let root = primary.root.join("inner");
        std::fs::create_dir_all(root.join("src")).unwrap();
        Repo {
            store: dir.path().join("inner-store"),
            root,
        }
    };

    let (code, error) = launch_refusal(
        &primary.store,
        &primary.root,
        &[
            reference_arg(&a.root, &a.store),
            reference_arg(&a.root, &a.store),
        ],
    );
    assert_eq!(code, 2, "{error}");
    assert_eq!(error["code"], "duplicate_root", "{error}");
    assert_eq!(error["retryable"], false);

    let (code, error) = launch_refusal(
        &primary.store,
        &primary.root,
        &[reference_arg(&nested.root, &nested.store)],
    );
    assert_eq!(code, 2);
    assert_eq!(error["code"], "nested_root", "{error}");

    let nine: Vec<String> = (0..9)
        .map(|i| {
            let root = dir.path().join(format!("r{i}"));
            std::fs::create_dir_all(&root).unwrap();
            reference_arg(&root, &dir.path().join(format!("s{i}")))
        })
        .collect();
    let (code, error) = launch_refusal(&primary.store, &primary.root, &nine);
    assert_eq!(code, 2);
    assert_eq!(error["code"], "too_many_roots", "{error}");

    // A component boundary, not a byte prefix: `a` and `ab` coexist, so
    // admission passes and the refusal is the (nonexistent) primary store.
    let sibling = unbootstrapped_repo(dir.path(), "ab", REF1_LIB);
    let (code, error) = launch_refusal(
        &primary.store,
        &primary.root,
        &[
            reference_arg(&a.root, &a.store),
            reference_arg(&b.root, &b.store),
            reference_arg(&sibling.root, &sibling.store),
        ],
    );
    assert_ne!(code, 2, "admission passed: {error}");
    assert_eq!(error["code"], "store_not_found", "{error}");
}

/// Two distinct roots forced to share `ws16` through the test-faults seam;
/// the refusal happens in-process, before anything is opened or served.
#[tokio::test]
async fn a_ws16_collision_is_refused_through_the_test_seam() {
    let dir = tempfile::tempdir().unwrap();
    let primary = unbootstrapped_repo(dir.path(), "ws0", PRIMARY_LIB);
    let ref1 = unbootstrapped_repo(dir.path(), "foo", REF1_LIB);
    let forced = format!("{}{}", "deadbeefdeadbeef", "0".repeat(48));
    let primary_canonical = primary.root.canonicalize().unwrap();
    let ref1_canonical = ref1.root.canonicalize().unwrap();
    context_foundry::fault::override_workspace_id(
        &primary_canonical.display().to_string(),
        &forced,
    );
    context_foundry::fault::override_workspace_id(&ref1_canonical.display().to_string(), &forced);
    let error = context_foundry::mcp::serve_stdio(context_foundry::mcp::ServerOptions {
        store: primary.store.clone(),
        root: primary.root.clone(),
        references: vec![
            context_foundry::roots::parse_reference(&reference_arg(&ref1.root, &ref1.store))
                .unwrap(),
        ],
        budget: context_foundry::config::BudgetConfig::default(),
    })
    .await
    .unwrap_err();
    context_foundry::fault::clear_workspace_overrides();
    assert_eq!(error.code(), "root_id_collision");
    assert_eq!(error.exit_code(), 2);
    // The seam is inert once cleared: distinct roots open normally (every
    // serving test below proves it), and no override leaks past this test.
    assert!(
        context_foundry::fault::workspace_id_override(&primary_canonical.display().to_string())
            .is_none()
    );
}

// ---------------------------------------------------------------------------
// Aliases, labels and the multi-root header
// ---------------------------------------------------------------------------

/// Labels come from basenames with control characters replaced by `?`; two
/// roots named `foo` keep the same label under different aliases, `foo~2`
/// stays whole, and a control-character basename renders one `?`.
#[tokio::test]
async fn aliases_and_labels_render_for_foo_foo_foo2_and_a_control_name() {
    let dir = tempfile::tempdir().unwrap();
    let primary = repo(dir.path(), "ws0", PRIMARY_LIB);
    let refs = ["foo", "sub/foo", "foo~2", "ba\u{7}d"]
        .map(|name| unbootstrapped_repo(dir.path(), name, REF1_LIB));
    let references: Vec<String> = refs
        .iter()
        .map(|repo| reference_arg(&repo.root, &repo.store))
        .collect();
    let client = stdio_owner(&primary.store, &primary.root, &references, &[]).await;

    let search = call(
        client.peer(),
        "search",
        Some(serde_json::json!({"query": "shared_anchor"})),
    )
    .await;
    let text = assert_success(&search);
    let segments = root_segments(&text);
    assert_eq!(segments.len(), 5, "{text}");
    assert!(
        segments[0].starts_with("primary(") && segments[0].contains(") r"),
        "{text}"
    );
    assert_eq!(segments[1], "ref1(foo) missing_store", "{text}");
    assert_eq!(segments[2], "ref2(foo) missing_store", "{text}");
    assert_eq!(segments[3], "ref3(foo~2) missing_store", "{text}");
    assert_eq!(segments[4], "ref4(ba?d) missing_store", "{text}");
    client.cancel().await.unwrap();
}

// ---------------------------------------------------------------------------
// Combined search and context
// ---------------------------------------------------------------------------

/// One `context` cites at least two roots with distinct `ws16`, under one
/// budget and one charge: a later probe on the same session sees exactly the
/// allowance minus the first delivery's counted tokens.
#[tokio::test]
async fn one_context_cites_two_roots_under_one_budget_and_one_charge() {
    const ALLOWANCE: u64 = 3_000;
    let dir = tempfile::tempdir().unwrap();
    let fixture = fixture(dir.path());
    let budget = session_budget_file(dir.path(), ALLOWANCE);
    let extra = vec!["--budget".to_owned(), budget.display().to_string()];
    let client = stdio_owner(
        &fixture.primary.store,
        &fixture.primary.root,
        &fixture.references(),
        &extra,
    )
    .await;
    let expected: Vec<String> = vec![
        ws16_of(&fixture.primary.root),
        ws16_of(&fixture.ref1.root),
        ws16_of(&fixture.ref2.root),
    ];

    let context = call(
        client.peer(),
        "context",
        Some(serde_json::json!({"query": "shared_anchor", "tokens": 1024})),
    )
    .await;
    let text = assert_success(&context);
    let segments = root_segments(&text);
    assert_eq!(segments.len(), 3, "{text}");
    for (segment, alias) in segments.iter().zip(["primary", "ref1", "ref2"]) {
        assert!(
            segment.starts_with(&format!("{alias}(")) && segment.contains(") r"),
            "{text}"
        );
    }
    assert!(
        header_segments(&text)
            .iter()
            .any(|segment| segment == "budget:1024"),
        "one effective budget for the whole merged response: {text}"
    );
    let distinct: std::collections::BTreeSet<String> = item_handles(&text)
        .into_iter()
        .map(|handle| handle.ws16)
        .filter(|ws16| expected.contains(ws16))
        .collect();
    assert!(
        distinct.len() >= 2,
        "items from at least two roots with distinct ws16: {text}"
    );

    // One charge: the session sees exactly the allowance minus the counted
    // tokens of the first delivery (a per-root charge would subtract more).
    let charged = count_tokens(&text);
    let probe = call(
        client.peer(),
        "context",
        Some(serde_json::json!({"query": "shared_anchor", "tokens": 32768})),
    )
    .await;
    let probe_text = assert_success(&probe);
    let budget_segment = header_segments(&probe_text)
        .into_iter()
        .find(|segment| segment.starts_with("budget:"))
        .expect("a budget segment");
    assert_eq!(
        budget_segment,
        format!("budget:{}(session)", ALLOWANCE - charged),
        "the remaining allowance is one delivery's charge away: {probe_text}"
    );
    client.cancel().await.unwrap();
}

/// A busy reference (its store held by another process) shows its coverage in
/// the header while the other roots serve; status names every root with
/// nulls for the unopened one; selecting only unavailable roots fails with
/// `roots_unavailable` listing each coverage; and re-indexing a root whose
/// coverage is not ok is `root_unavailable`.
#[tokio::test]
async fn a_busy_reference_shows_coverage_while_others_serve() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = fixture(dir.path());
    // Another live owner of ref2's store: never stopped by this owner.
    let holder = context_foundry::Engine::open_existing(&fixture.ref2.store).unwrap();

    let client = stdio_owner(
        &fixture.primary.store,
        &fixture.primary.root,
        &fixture.references(),
        &[],
    )
    .await;
    let expected: Vec<String> = vec![ws16_of(&fixture.primary.root), ws16_of(&fixture.ref1.root)];

    let search = call(
        client.peer(),
        "search",
        Some(serde_json::json!({"query": "shared_anchor"})),
    )
    .await;
    let text = assert_success(&search);
    let segments = root_segments(&text);
    assert_eq!(segments.len(), 3, "{text}");
    assert!(segments[0].starts_with("primary(") && segments[0].contains(") r"));
    assert!(segments[1].starts_with("ref1(") && segments[1].contains(") r"));
    assert_eq!(segments[2], "ref2(foo~2) busy", "{text}");
    // The busy root served nothing: every hit names an open root.
    assert!(
        item_handles(&text)
            .iter()
            .all(|handle| expected.contains(&handle.ws16)),
        "{text}"
    );

    // status names every root; fields needing an open store are null.
    let status = call(client.peer(), "status", None).await;
    let status: serde_json::Value = serde_json::from_str(&assert_success(&status)).unwrap();
    let roots = status["roots"].as_array().expect("a roots array");
    assert_eq!(roots.len(), 3, "{status}");
    assert_eq!(roots[0]["alias"], "primary");
    assert_eq!(roots[0]["coverage"], "ok");
    assert!(roots[0]["source_revision"].is_u64());
    assert_eq!(roots[1]["alias"], "ref1");
    assert_eq!(roots[1]["label"], "foo");
    assert_eq!(roots[2]["alias"], "ref2");
    assert_eq!(roots[2]["label"], "foo~2");
    assert_eq!(roots[2]["coverage"], "busy");
    assert_eq!(roots[2]["workspace_id"], serde_json::Value::Null);
    assert_eq!(roots[2]["source_revision"], serde_json::Value::Null);
    assert_eq!(roots[2]["scan_state"], serde_json::Value::Null);

    // Selecting only the busy root fails and lists its coverage.
    let refusal = call(
        client.peer(),
        "search",
        Some(serde_json::json!({"query": "shared_anchor", "roots": ["ref2"]})),
    )
    .await;
    let (code, message) = bounded_error(&refusal);
    assert_eq!(code, "roots_unavailable");
    assert!(message.contains("ref2(foo~2) busy"), "{message}");
    assert!(message.len() < 1024);

    // Re-indexing a root whose coverage is not ok is `root_unavailable`.
    let refusal = call(
        client.peer(),
        "index",
        Some(serde_json::json!({"root": "ref2"})),
    )
    .await;
    let (code, message) = bounded_error(&refusal);
    assert_eq!(code, "root_unavailable");
    assert!(message.contains("busy"), "{message}");

    client.cancel().await.unwrap();
    drop(holder);
}

/// Every selected root unavailable (a missing store and a busy store) fails
/// with `roots_unavailable` listing both coverages inside the bound.
#[tokio::test]
async fn all_selected_roots_unavailable_fails_roots_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = fixture(dir.path());
    // ref1's store directory does not exist: `missing_store`.
    let mut references = fixture.references();
    references[0] = reference_arg(&fixture.ref1.root, &dir.path().join("nowhere-store"));
    let holder = context_foundry::Engine::open_existing(&fixture.ref2.store).unwrap();
    let client = stdio_owner(
        &fixture.primary.store,
        &fixture.primary.root,
        &references,
        &[],
    )
    .await;

    let refusal = call(
        client.peer(),
        "context",
        Some(serde_json::json!({"query": "shared_anchor", "roots": ["ref1", "ref2"]})),
    )
    .await;
    let (code, message) = bounded_error(&refusal);
    assert_eq!(code, "roots_unavailable");
    assert!(message.contains("ref1(foo) missing_store"), "{message}");
    assert!(message.contains("ref2(foo~2) busy"), "{message}");
    assert!(message.len() < 1024);
    client.cancel().await.unwrap();
    drop(holder);
}

// ---------------------------------------------------------------------------
// Root-scoped identity
// ---------------------------------------------------------------------------

/// After `index {root:"ref1"}`, a pre-edit ref1 handle is `stale_handle`
/// while primary and ref2 handles (unchanged roots) still retrieve through
/// their own roots.
#[tokio::test]
async fn indexing_ref1_makes_only_its_pre_edit_handles_stale() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = fixture(dir.path());
    let client = stdio_owner(
        &fixture.primary.store,
        &fixture.primary.root,
        &fixture.references(),
        &[],
    )
    .await;

    let handle_of = |search_text: String| -> String {
        item_handles(&search_text)
            .first()
            .unwrap_or_else(|| panic!("a hit:\n{search_text}"))
            .to_string()
    };
    let primary_handle = handle_of(assert_success(
        &call(
            client.peer(),
            "search",
            Some(serde_json::json!({"query": "shared_anchor"})),
        )
        .await,
    ));
    let ref1_handle = handle_of(assert_success(
        &call(
            client.peer(),
            "search",
            Some(serde_json::json!({"query": "delta_signal_pad"})),
        )
        .await,
    ));
    let ref2_handle = handle_of(assert_success(
        &call(
            client.peer(),
            "search",
            Some(serde_json::json!({"query": "kappa_probe_quiet"})),
        )
        .await,
    ));
    assert_eq!(
        HandleRef::parse(&ref1_handle).unwrap().ws16,
        ws16_of(&fixture.ref1.root)
    );
    assert_ne!(
        HandleRef::parse(&primary_handle).unwrap().ws16,
        HandleRef::parse(&ref2_handle).unwrap().ws16
    );

    // Edit ref1's source, then re-index ONLY ref1 through its own store.
    std::fs::write(
        fixture.ref1.root.join("src/lib.rs"),
        format!("{REF1_LIB}// edited\n"),
    )
    .unwrap();
    let report = call(
        client.peer(),
        "index",
        Some(serde_json::json!({"root": "ref1"})),
    )
    .await;
    let report_text = assert_success(&report);
    let report: serde_json::Value = serde_json::from_str(&report_text).unwrap();
    assert!(report.get("changed").is_some(), "{report_text}");

    let stale = call(
        client.peer(),
        "retrieve",
        Some(serde_json::json!({"handle": ref1_handle, "tokens": 512})),
    )
    .await;
    let (code, _) = bounded_error(&stale);
    assert_eq!(code, "stale_handle");

    for (label, handle, root) in [
        ("primary", primary_handle, &fixture.primary.root),
        ("ref2", ref2_handle, &fixture.ref2.root),
    ] {
        let fresh = call(
            client.peer(),
            "retrieve",
            Some(serde_json::json!({"handle": handle, "tokens": 512})),
        )
        .await;
        let text = assert_success(&fresh);
        let retrieved = item_handles(&text);
        assert_eq!(
            retrieved.len(),
            1,
            "{label} still retrieves one item: {text}"
        );
        assert_eq!(
            retrieved[0].ws16,
            ws16_of(root),
            "{label} reads through its own root"
        );
    }
    client.cancel().await.unwrap();
}

/// Identical graph rows in two roots stay distinct: both edges appear, each
/// carrying its seed root's alias; no cross-root deduplication or edges.
#[tokio::test]
async fn identical_graph_rows_in_two_roots_stay_distinct() {
    let lib = "pub fn graph_seed_port() -> u8 {\n    7\n}\n\npub fn graph_dock_rail() -> u8 {\n    9\n}\n";
    let dir = tempfile::tempdir().unwrap();
    let primary = repo(dir.path(), "ws0", lib);
    let ref1 = repo(dir.path(), "foo", lib);
    let hash = context_foundry::digest(lib.as_bytes());
    let bundle = context_foundry::graph::GraphBundle {
        provider: "fixture".to_owned(),
        revision: "1".to_owned(),
        edges: vec![context_foundry::graph::Edge {
            from: context_foundry::graph::Endpoint {
                path: "src/lib.rs".to_owned(),
                line: 1,
                symbol: "graph_seed_port".to_owned(),
                hash: hash.clone(),
            },
            to: context_foundry::graph::Endpoint {
                path: "src/lib.rs".to_owned(),
                line: 5,
                symbol: "graph_dock_rail".to_owned(),
                hash: hash.clone(),
            },
            kind: "calls".to_owned(),
            evidence: "manual".to_owned(),
        }],
    };
    for store in [&primary.store, &ref1.store] {
        let engine = context_foundry::Engine::open_existing(store).unwrap();
        engine.import_graph(&bundle).unwrap();
    }
    drop(bundle);

    let client = stdio_owner(
        &primary.store,
        &primary.root,
        &[reference_arg(&ref1.root, &ref1.store)],
        &[],
    )
    .await;
    let context = call(
        client.peer(),
        "context",
        Some(serde_json::json!({
            "query": "graph_seed_port",
            "strategy": "graph",
            "tokens": 2048
        })),
    )
    .await;
    let text = assert_success(&context);
    let parsed = parse_v2(&text).unwrap_or_else(|e| panic!("not a v2 text ({e}):\n{text}"));
    let mut edges: Vec<String> = parsed
        .items
        .iter()
        .filter(|item| item.kind == context_foundry::testkit::V2Kind::Edge)
        .map(|item| item.body.clone())
        .collect();
    edges.sort();
    let edge_text = "src/lib.rs:1 (graph_seed_port) --calls--> src/lib.rs:5 (graph_dock_rail) [manual; provider=fixture@1]";
    assert_eq!(
        edges,
        vec![format!("primary {edge_text}"), format!("ref1 {edge_text}")],
        "identical rows stay distinct, one per root, aliased:\n{text}"
    );
    client.cancel().await.unwrap();
}

// ---------------------------------------------------------------------------
// Selection and admission scope
// ---------------------------------------------------------------------------

/// Unknown or duplicate aliases are `invalid_argument`; a query naming a
/// fourth, unadmitted repository's path admits nothing.
#[tokio::test]
async fn unknown_aliases_refuse_and_query_text_admits_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = fixture(dir.path());
    let fourth = unbootstrapped_repo(dir.path(), "fourth", "pub fn orphan_zephyr_spin() {}\n");
    let client = stdio_owner(
        &fixture.primary.store,
        &fixture.primary.root,
        &fixture.references(),
        &[],
    )
    .await;

    for roots in [
        serde_json::json!(["ref9"]),
        serde_json::json!(["primary", "primary"]),
        serde_json::json!([]),
        serde_json::json!(["primary", 7]),
    ] {
        let refusal = call(
            client.peer(),
            "search",
            Some(serde_json::json!({"query": "shared_anchor", "roots": roots})),
        )
        .await;
        let (code, _) = bounded_error(&refusal);
        assert_eq!(code, "invalid_argument", "roots {roots}");
    }
    let refusal = call(
        client.peer(),
        "index",
        Some(serde_json::json!({"root": "ref9"})),
    )
    .await;
    let (code, _) = bounded_error(&refusal);
    assert_eq!(code, "invalid_argument");

    // A query naming the fourth repository's path is ordinary search text.
    let naming = format!("orphan_zephyr_spin in {}", fourth.root.display());
    let search = call(
        client.peer(),
        "search",
        Some(serde_json::json!({"query": naming})),
    )
    .await;
    let text = assert_success(&search);
    assert!(
        item_handles(&text).is_empty(),
        "the fourth repository is not admitted by query text: {text}"
    );
    let status = call(client.peer(), "status", None).await;
    let status: serde_json::Value = serde_json::from_str(&assert_success(&status)).unwrap();
    assert_eq!(status["roots"].as_array().unwrap().len(), 3, "{status}");
    client.cancel().await.unwrap();
}

// ---------------------------------------------------------------------------
// The shared deadline: a stall in the second root fails the whole request
// ---------------------------------------------------------------------------

/// A test-faults stall beyond the 5000 ms read deadline in the SECOND root
/// (the fault fires from the second hit onward; the primary root produced
/// its hits first) returns `deadline_exceeded`; the single engine slot stays
/// busy (a concurrent `status` is refused `busy`) until the stall returns,
/// and is free the moment it does.
#[tokio::test]
async fn a_stall_in_the_second_root_fails_the_request_and_holds_the_slot() {
    let dir = tempfile::tempdir().unwrap();
    let primary = repo(dir.path(), "ws0", PRIMARY_LIB);
    let ref1 = repo(dir.path(), "foo", REF1_LIB);
    let mut command = Command::new(FAULTS_BIN);
    command
        .env(
            "FOUNDRY_TEST_FAULT",
            format!(
                "{}=delay:{STALL_MS}:1",
                context_foundry::fault::names::ROOTS_BEFORE_ROOT
            ),
        )
        .arg("--store")
        .arg(&primary.store)
        .args(["mcp", "--root"])
        .arg(&primary.root)
        .arg("--reference")
        .arg(reference_arg(&ref1.root, &ref1.store))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let transport = TokioChildProcess::new(command).unwrap();
    let client: rmcp::service::RunningService<rmcp::RoleClient, ()> =
        ().serve(transport).await.unwrap();

    let deadline = Duration::from_millis(READ_DEADLINE_MS);
    let started = std::time::Instant::now();
    let peer = client.peer().clone();
    let stalled = tokio::spawn({
        let peer = peer.clone();
        async move {
            call(
                &peer,
                "context",
                Some(serde_json::json!({"query": "shared_anchor", "tokens": 1024})),
            )
            .await
        }
    });
    // Give the stalled call a moment to be admitted before probing: a probe
    // that won the slot first would return busy for the stalled call itself.
    tokio::time::sleep(Duration::from_millis(250)).await;

    let (mut busy_before, mut busy_after) = (false, false);
    while !stalled.is_finished() {
        let probe = call(client.peer(), "status", None).await;
        let at = started.elapsed();
        if probe.is_error == Some(true) {
            let (code, _) = bounded_error(&probe);
            assert_eq!(code, "busy", "a held slot refuses, never queues");
            if at < deadline {
                busy_before = true;
            } else {
                busy_after = true;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let result = stalled.await.unwrap();
    let elapsed = started.elapsed();
    let (code, _) = bounded_error(&result);
    assert_eq!(
        code,
        "deadline_exceeded",
        "the whole request fails at the shared deadline: {}",
        text_of(&result)
    );
    assert!(
        elapsed >= Duration::from_millis(STALL_MS),
        "the slot is not abandoned at the deadline; it returned after {elapsed:?}"
    );
    assert!(busy_before, "the slot was held within the deadline");
    assert!(
        busy_after,
        "the slot stayed held after the deadline until the stall returned"
    );
    let free = call(client.peer(), "status", None).await;
    assert_eq!(
        free.is_error,
        Some(false),
        "the slot is free the moment the stalled call returns: {}",
        text_of(&free)
    );
    client.cancel().await.unwrap();
}

// ---------------------------------------------------------------------------
// Review fixes (round 2)
// ---------------------------------------------------------------------------

/// M1: an unknown alias is `invalid_argument` before dispatch on a
/// NO-REFERENCE owner too; `primary` selects the bound root and the header
/// keeps its single-root `r<rev>` form.
#[tokio::test]
async fn unknown_alias_is_invalid_argument_even_without_references() {
    let dir = tempfile::tempdir().unwrap();
    let primary = repo(dir.path(), "ws0", PRIMARY_LIB);
    let client = stdio_owner(&primary.store, &primary.root, &[], &[]).await;
    for tool in ["search", "context"] {
        let refusal = call(
            client.peer(),
            tool,
            Some(serde_json::json!({"query": "shared_anchor", "roots": ["ref1"]})),
        )
        .await;
        let (code, _) = bounded_error(&refusal);
        assert_eq!(code, "invalid_argument", "tool {tool}");
    }
    let ok = call(
        client.peer(),
        "search",
        Some(serde_json::json!({"query": "shared_anchor", "roots": ["primary"]})),
    )
    .await;
    let text = assert_success(&ok);
    assert!(
        header_segments(&text)[1].starts_with("r"),
        "single-root rendering unchanged: {text}"
    );
    client.cancel().await.unwrap();
}

/// M2: the request's `roots` order never reorders the merged result:
/// execution, RRF tie-breaks and header segments follow admission order.
/// Tier-1 cutoff at `limit` 1, and a tier-2 tie between two identical files.
#[tokio::test]
async fn roots_selection_order_never_reorders_the_merged_result() {
    // Tier-1 cutoff: `shared_anchor` is defined in the primary and ref1.
    let dir = tempfile::tempdir().unwrap();
    let fixture = fixture(dir.path());
    let client = stdio_owner(
        &fixture.primary.store,
        &fixture.primary.root,
        &fixture.references(),
        &[],
    )
    .await;
    let primary_ws16 = ws16_of(&fixture.primary.root);
    let ask = |roots: serde_json::Value| {
        let peer = client.peer();
        async move {
            call(
                peer,
                "search",
                Some(serde_json::json!({"query": "shared_anchor", "limit": 1, "roots": roots})),
            )
            .await
        }
    };
    let reverse = assert_success(&ask(serde_json::json!(["ref1", "primary"])).await);
    let forward = assert_success(&ask(serde_json::json!(["primary", "ref1"])).await);
    assert_eq!(reverse, forward, "request order changes nothing");
    assert_eq!(item_handles(&reverse)[0].ws16, primary_ws16, "{reverse}");
    assert_eq!(root_segments(&reverse), root_segments(&forward));
    client.cancel().await.unwrap();

    // Tier-2 tie: identical files rank equal in both roots (RRF 1/61 each),
    // so admission order decides; the header lists ref1 second either way.
    let tie_lib = "pub fn tie_home_grip() {}\n\n// kite_tail_extra marker\n";
    let dir = tempfile::tempdir().unwrap();
    let primary = repo(dir.path(), "ws0", tie_lib);
    let ref1 = repo(dir.path(), "foo", tie_lib);
    let client = stdio_owner(
        &primary.store,
        &primary.root,
        &[reference_arg(&ref1.root, &ref1.store)],
        &[],
    )
    .await;
    let ask = |roots: serde_json::Value| {
        let peer = client.peer();
        async move {
            call(
                peer,
                "search",
                Some(serde_json::json!({"query": "kite_tail_extra", "limit": 1, "roots": roots})),
            )
            .await
        }
    };
    let reverse = assert_success(&ask(serde_json::json!(["ref1", "primary"])).await);
    let forward = assert_success(&ask(serde_json::json!(["primary", "ref1"])).await);
    assert_eq!(reverse, forward, "a tied tier-2 breaks by admission order");
    assert_eq!(item_handles(&reverse)[0].ws16, ws16_of(&primary.root));
    client.cancel().await.unwrap();
}

/// M3: with `roots` omitted and nothing able to serve, `roots_unavailable`
/// names EVERY admitted root with its coverage; an explicit selection names
/// the selected roots only.
#[tokio::test]
async fn omitted_roots_name_every_admitted_root_when_none_serves() {
    let dir = tempfile::tempdir().unwrap();
    // Primary: authoritative store opens, derived index needs repair.
    let primary = repo(dir.path(), "ws0", PRIMARY_LIB);
    context_foundry::testkit::corrupt_search_index(&primary.store);
    let ref1 = unbootstrapped_repo(dir.path(), "foo", REF1_LIB);
    let ref2 = repo(dir.path(), "foo~2", REF2_LIB);
    let holder = context_foundry::Engine::open_existing(&ref2.store).unwrap();
    let client = stdio_owner(
        &primary.store,
        &primary.root,
        &[
            reference_arg(&ref1.root, &ref1.store),
            reference_arg(&ref2.root, &ref2.store),
        ],
        &[],
    )
    .await;

    for tool in ["search", "context"] {
        let refusal = call(
            client.peer(),
            tool,
            Some(serde_json::json!({"query": "shared_anchor"})),
        )
        .await;
        let (code, message) = bounded_error(&refusal);
        assert_eq!(code, "roots_unavailable", "tool {tool}");
        assert!(
            message.contains("primary(ws0) repair_required"),
            "{message}"
        );
        assert!(message.contains("ref1(foo) missing_store"), "{message}");
        assert!(message.contains("ref2(foo~2) busy"), "{message}");
        assert!(message.len() < 1024, "{message}");
    }
    let refusal = call(
        client.peer(),
        "search",
        Some(serde_json::json!({"query": "shared_anchor", "roots": ["ref1"]})),
    )
    .await;
    let (code, message) = bounded_error(&refusal);
    assert_eq!(code, "roots_unavailable");
    assert!(message.contains("ref1(foo) missing_store"), "{message}");
    assert!(
        !message.contains("primary"),
        "only the selected roots: {message}"
    );
    client.cancel().await.unwrap();
    drop(holder);
}

/// M4: nine admissions with JSON-special labels — every alias:coverage pair
/// survives inside the 1024-byte serialized error; labels drop out instead
/// of letting generic truncation eat a pair.
#[tokio::test]
async fn every_coverage_pair_survives_the_error_bound() {
    let dir = tempfile::tempdir().unwrap();
    let primary = repo(dir.path(), "ws0", PRIMARY_LIB);
    context_foundry::testkit::corrupt_search_index(&primary.store);
    let slashes = "\\".repeat(32);
    let refs: Vec<Repo> = (0..8)
        .map(|index| unbootstrapped_repo(dir.path(), &format!("{slashes}{index}"), REF1_LIB))
        .collect();
    let references: Vec<String> = refs
        .iter()
        .map(|repo| reference_arg(&repo.root, &repo.store))
        .collect();
    let client = stdio_owner(&primary.store, &primary.root, &references, &[]).await;

    let refusal = call(
        client.peer(),
        "search",
        Some(serde_json::json!({"query": "shared_anchor"})),
    )
    .await;
    let (code, message) = bounded_error(&refusal);
    assert_eq!(code, "roots_unavailable");
    let mut pairs = vec!["primary:repair_required".to_owned()];
    for index in 1..=8 {
        pairs.push(format!("ref{index}:missing_store"));
    }
    for pair in &pairs {
        assert!(message.contains(pair.as_str()), "missing {pair}: {message}");
    }
    assert!(
        message.ends_with("restart the owner to retry"),
        "nothing was truncated away: {message}"
    );
    assert!(message.len() < 1024, "{message}");
    client.cancel().await.unwrap();
}

/// M5: the first three distinct (root, path) files consume the outline
/// slots even when their outlines are suppressed (each file is fully
/// spanned by its unit), so a fourth file in ref1 renders no outline.
#[tokio::test]
async fn fully_spanned_files_consume_outline_slots() {
    let spanned = "pub fn slot_berth_vane() -> u8 {\n    3\n}\n";
    let dir = tempfile::tempdir().unwrap();
    let primary = dir.path().join("ws0");
    std::fs::create_dir_all(primary.join("src")).unwrap();
    for name in ["a", "b", "c"] {
        std::fs::write(primary.join(format!("src/{name}.rs")), spanned).unwrap();
    }
    let primary_store = dir.path().join("ws0-store");
    bootstrap_apply(&primary_store, &primary);
    let ref1 = dir.path().join("foo");
    std::fs::create_dir_all(ref1.join("src")).unwrap();
    std::fs::write(
        ref1.join("src/d.rs"),
        "pub fn slot_berth_vane() -> u8 {\n    3\n}\n// keel_note_line tail\n",
    )
    .unwrap();
    let ref1_store = dir.path().join("foo-store");
    bootstrap_apply(&ref1_store, &ref1);
    let client = stdio_owner(
        &primary_store,
        &primary,
        &[reference_arg(&ref1, &ref1_store)],
        &[],
    )
    .await;
    let context = call(
        client.peer(),
        "context",
        Some(serde_json::json!({"query": "slot_berth_vane", "tokens": 2048})),
    )
    .await;
    let text = assert_success(&context);
    let parsed = parse_v2(&text).unwrap_or_else(|e| panic!("not a v2 text ({e}):\n{text}"));
    let outlines: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.form.as_deref() == Some("outline"))
        .collect();
    assert!(
        outlines.is_empty(),
        "three suppressed files consume the slots; ref1's fourth file never gets one:\n{text}"
    );
    // All four units are still delivered, from both roots.
    let distinct: std::collections::BTreeSet<String> = item_handles(&text)
        .into_iter()
        .map(|handle| handle.ws16)
        .collect();
    assert_eq!(distinct.len(), 2, "{text}");
    client.cancel().await.unwrap();
}
