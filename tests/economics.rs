//! 003 T005 § Payload measurement (adapter-economics): the frozen corpus at
//! `tests/fixtures/.economics` is the exact bytes of the nine files at
//! `6bb81e6`; tests copy it to a temporary root, index it once and then
//! measure and assert through the real MCP stdio boundary with raw JSON-RPC
//! (the minimal `RawStdio` helper pattern of `tests/mcp.rs`, in-file).
//!
//! Expectations are NOT tuned to pass: each assertion names the contract's
//! expected unit; a miss is root-caused on the rendered output, never tuned
//! away. The `#[ignore] payload_report` prints the per-query payload table
//! for `docs/validation.md`:
//! `cargo test --locked --test economics -- --ignored --nocapture payload_report`.
//!
//! NOT RUN during adapter implementation (mid-flight builds/tests are
//! forbidden); the captain's single post-settlement run executes them.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use context_foundry::store::HandleRef;
use context_foundry::testkit::{V2Response, parse_v2};

const BIN: &str = env!("CARGO_BIN_EXE_foundry");
const CORPUS_DIR: &str = "tests/fixtures/.economics";

/// One identifier query and its INDEPENDENTLY FROZEN expected unit: the
/// byte range extended backward over its leading run of `///` docs and
/// `#[...]` attributes, no trailing line terminator — so the assertions
/// cannot pass against a server-chosen narrower slice. `marker` is the
/// definition line hit #1's locator shows; `head_line` is that line (the
/// unit's head, after the leading run); `first_line`/`last_line` are the
/// whole unit's 1-based lines. Ranges were derived by hand from the 6bb81e6
/// corpus bytes.
struct Identifier {
    query: &'static str,
    path: &'static str,
    marker: &'static str,
    head_line: u64,
    start: u64,
    end: u64,
    first_line: u64,
    last_line: u64,
}

const IDENTIFIERS: [Identifier; 6] = [
    Identifier {
        query: "reconstruct_verified",
        path: "src/store.rs",
        marker: "fn reconstruct_verified",
        head_line: 445,
        start: 14954,
        end: 17238,
        first_line: 442,
        last_line: 497,
    },
    Identifier {
        query: "pack_ordered",
        path: "src/response.rs",
        marker: "fn pack_ordered",
        head_line: 135,
        start: 3947,
        end: 5655,
        first_line: 132,
        last_line: 181,
    },
    Identifier {
        query: "BudgetConfig",
        path: "src/config.rs",
        marker: "struct BudgetConfig",
        head_line: 38,
        start: 1381,
        end: 1689,
        first_line: 37,
        last_line: 45,
    },
    Identifier {
        query: "native_discovery_block",
        path: "src/bootstrap.rs",
        marker: "fn native_discovery_block",
        head_line: 436,
        start: 17382,
        end: 18499,
        first_line: 433,
        last_line: 447,
    },
    Identifier {
        query: "fit_prefix",
        path: "src/response.rs",
        marker: "fn fit_prefix",
        head_line: 376,
        start: 12181,
        end: 14762,
        first_line: 369,
        last_line: 432,
    },
    Identifier {
        query: "take_context_id",
        path: "src/mcp.rs",
        marker: "fn take_context_id",
        head_line: 972,
        start: 40352,
        end: 40656,
        first_line: 972,
        last_line: 978,
    },
];

/// Subsystem questions: `context` at 2048 must surface the expected unit,
/// verbatim or as a signature. Each marker is the unit's definition line and
/// is unique to it in the corpus (checked when frozen).
const QUESTIONS: [(&str, &str); 6] = [
    (
        "stale handle rejected on retrieve",
        "pub fn retrieve(&self, handle_json: &str, tokens: usize)",
    ),
    ("repair index quarantine marker", "fn repair_index("),
    ("inbound frame byte limit", "const MAX_INBOUND_BYTES"),
    ("session allowance refund", "fn refund("),
    ("receipt deduplication conflict", "struct ReceiptDedup"),
    (
        "sweep unseen sources after complete scan",
        "fn sweep_unseen(",
    ),
];

fn count_tokens(text: &str) -> usize {
    tiktoken_rs::o200k_base_singleton()
        .encode_ordinary(text)
        .len()
}

/// A v2 text parsed by the shared testkit parser.
fn v2(text: &str) -> V2Response {
    parse_v2(text).unwrap_or_else(|e| panic!("not a v2 text ({e}):\n{text}"))
}

fn server_args(store: &Path, root: &Path) -> Vec<String> {
    vec![
        "--store".into(),
        store.display().to_string(),
        "mcp".into(),
        "--root".into(),
        root.display().to_string(),
    ]
}

/// Index the copied corpus once through the CLI, exactly as a bootstrap
/// would (mirrors `bootstrap_apply` of `tests/mcp.rs`).
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
        "bootstrap --apply failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A real `foundry mcp` child spoken to over raw JSON-RPC lines, so tests
/// see the exact result bytes the server emitted.
struct RawStdio {
    _child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    lines: tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>,
    next_id: u64,
}

struct RawCall {
    /// The exact bytes of the JSON-RPC `result` value, as emitted.
    raw: String,
    parsed: serde_json::Value,
}

impl RawCall {
    fn is_error(&self) -> bool {
        self.parsed["isError"] == true
    }

    /// The single text block exactly as emitted.
    fn text(&self) -> String {
        self.parsed["content"][0]["text"]
            .as_str()
            .expect("one text block")
            .to_owned()
    }
}

impl RawStdio {
    async fn start(store: &Path, root: &Path) -> Self {
        let mut command = tokio::process::Command::new(BIN);
        command
            .args(server_args(store, root))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = command.spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        use tokio::io::AsyncBufReadExt as _;
        let mut raw = Self {
            _child: child,
            stdin,
            lines: tokio::io::BufReader::new(stdout).lines(),
            next_id: 2,
        };
        raw.send(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2025-11-25", "capabilities": {},
                       "clientInfo": {"name": "economics-test", "version": "0"}}
        }))
        .await;
        let _ = raw.read_result(1).await;
        raw.send(serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .await;
        raw
    }

    async fn send(&mut self, value: serde_json::Value) {
        use tokio::io::AsyncWriteExt as _;
        self.stdin
            .write_all(value.to_string().as_bytes())
            .await
            .unwrap();
        self.stdin.write_all(b"\n").await.unwrap();
        self.stdin.flush().await.unwrap();
    }

    async fn read_result(&mut self, id: u64) -> String {
        #[derive(serde::Deserialize)]
        struct Envelope<'a> {
            id: Option<u64>,
            #[serde(borrow)]
            result: Option<&'a serde_json::value::RawValue>,
        }
        loop {
            let line = tokio::time::timeout(Duration::from_secs(180), self.lines.next_line())
                .await
                .expect("the server answers in time")
                .unwrap()
                .expect("the server keeps the stream open");
            let envelope: Envelope = serde_json::from_str(&line).unwrap();
            if envelope.id == Some(id) {
                return envelope
                    .result
                    .expect("a result, not a protocol error")
                    .get()
                    .to_owned();
            }
        }
    }

    async fn rpc(&mut self, method: &str, params: serde_json::Value) -> String {
        let id = self.next_id;
        self.next_id += 1;
        self.send(serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params
        }))
        .await;
        self.read_result(id).await
    }

    async fn call(&mut self, tool: &str, arguments: serde_json::Value) -> RawCall {
        let id = self.next_id;
        self.next_id += 1;
        self.send(serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": {"name": tool, "arguments": arguments}
        }))
        .await;
        let raw = self.read_result(id).await;
        let parsed = serde_json::from_str(&raw).unwrap();
        RawCall { raw, parsed }
    }
}

/// One measured corpus: a temp copy of the frozen bytes, indexed once.
struct Measured {
    _fixture: tempfile::TempDir,
    root: PathBuf,
    store: PathBuf,
}

/// The nine prescribed corpus files (adapter-economics § Payload
/// measurement) with their SHA-256 at 6bb81e6, computed once with
/// `git show 6bb81e6:<path> | shasum -a 256` and frozen here. The corpus is
/// verified against EXACTLY this set of paths and digests before anything
/// is copied or indexed.
const CORPUS_SHA256: [(&str, &str); 9] = [
    (
        "src/store.rs",
        "ea0d59ba7ddff8bad58fad343a086048018d8a17ff62a4b7c241f460296bb566",
    ),
    (
        "src/response.rs",
        "20cea410bcbb96b43bd4d0152b5115351e7aaf6944edeb8483cac653ecb9471b",
    ),
    (
        "src/mcp.rs",
        "748f25f139d927ad405a5229096bab98cb6960a7c4f092b7e5ada102f6131406",
    ),
    (
        "src/ingest.rs",
        "9e3366765488b984757ac26f85207dd0f0abf03b228052bd63e85b3eca12d419",
    ),
    (
        "src/bootstrap.rs",
        "c4c686853282300c19a38eb48c58a40931a6d5f127059556f6c5c6f64471b12c",
    ),
    (
        "src/receipts.rs",
        "82b460a1ccff0939f4456befd4cb4f380741b5ec7665109cbdc223c8ec5dd1d1",
    ),
    (
        "src/config.rs",
        "abc006a5ae40bd6ca9873554f0817acfe6fd4544604ce1408789c79ca4b3846f",
    ),
    (
        "src/error.rs",
        "daa4988b0719aad422a5d4e3a25a8e63698aa41f8d5a0c3673d34ee93f81fc49",
    ),
    (
        "docs/architecture.md",
        "9faebe456f5d314ee6b6affa15c372014f740d9177d5805c1574088b85e2d8b1",
    ),
];

/// Every file below `from` as `/`-joined relative paths.
fn relative_files(from: &Path, prefix: &str, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().to_string();
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if entry.file_type().unwrap().is_dir() {
            relative_files(&entry.path(), &relative, out);
        } else {
            out.push(relative);
        }
    }
}

fn measured_corpus() -> Measured {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("corpus");
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS_DIR);
    // The frozen corpus is exactly the nine prescribed files with their
    // 6bb81e6 bytes; any drift refuses before anything is indexed.
    let mut files = Vec::new();
    relative_files(&from, "", &mut files);
    files.sort();
    let mut expected: Vec<&str> = CORPUS_SHA256.iter().map(|(path, _)| *path).collect();
    expected.sort_unstable();
    assert_eq!(
        files, expected,
        "the frozen corpus holds exactly the nine prescribed files"
    );
    for (path, digest) in CORPUS_SHA256 {
        assert_eq!(
            sha256_hex(&from.join(path)),
            digest,
            "{path} no longer holds its 6bb81e6 bytes"
        );
    }
    copy_tree(&from, &root);
    let store = fixture.path().join("store");
    bootstrap_apply(&store, &root);
    Measured {
        _fixture: fixture,
        root,
        store,
    }
}

/// The hex SHA-256 of one file's bytes.
fn sha256_hex(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(std::fs::read(path).unwrap());
    format!("{:x}", hasher.finalize())
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn corpus_bytes(root: &Path, relative: &str) -> Vec<u8> {
    std::fs::read(root.join(relative)).unwrap()
}

/// Search at limit 10 (the default 1024-token budget) for one query.
async fn search10(server: &mut RawStdio, query: &str) -> RawCall {
    server
        .call("search", serde_json::json!({"query": query, "limit": 10}))
        .await
}

#[tokio::test]
async fn identifier_definitions_are_search_hit1_and_retrieve_whole_units() {
    let corpus = measured_corpus();
    let mut server = RawStdio::start(&corpus.store, &corpus.root).await;

    for Identifier {
        query,
        path,
        marker,
        head_line,
        start,
        end,
        first_line,
        last_line,
    } in IDENTIFIERS
    {
        let search = search10(&mut server, query).await;
        assert!(
            !search.is_error(),
            "`{query}` search failed: {}",
            search.raw
        );
        let text = search.text();
        assert!(
            count_tokens(&text) <= 1024,
            "`{query}`: search stays in budget"
        );
        let parsed = v2(&text);
        assert_eq!(parsed.header[0], "foundry search");
        assert!(
            !parsed.items.is_empty(),
            "`{query}`: the definition is a search hit"
        );
        let hit = &parsed.items[0];
        assert!(
            hit.handle.starts_with(&format!("{path}#")),
            "`{query}`: hit #1 is the definition in {path}, got {}",
            hit.handle
        );
        assert!(
            hit.label
                .as_deref()
                .is_some_and(|label| label.contains(query)),
            "`{query}`: hit #1 is labeled its definition, got {:?}",
            hit.label
        );
        assert!(
            hit.body.contains(marker),
            "`{query}`: hit #1's locator line shows the definition, got: {}",
            hit.body
        );
        // The hit's handle names the FROZEN whole unit, not a server-chosen
        // slice: exact path, byte range and unit line numbers.
        let hit_range = HandleRef::parse(&hit.handle).unwrap();
        assert_eq!(
            (hit_range.path.as_str(), hit_range.start, hit_range.end),
            (path, start, end),
            "`{query}`: hit #1's handle is the frozen unit range"
        );
        assert_eq!(
            hit.lines.as_deref(),
            Some(format!("L{head_line}").as_str()),
            "`{query}`: hit #1 locates the unit's head (definition) line"
        );

        let retrieve = server
            .call(
                "retrieve",
                serde_json::json!({"handle": hit.handle, "tokens": 2048}),
            )
            .await;
        assert!(
            !retrieve.is_error(),
            "`{query}`: retrieve of hit #1 failed: {}",
            retrieve.raw
        );
        let text = retrieve.text();
        assert!(
            count_tokens(&text) <= 2048,
            "`{query}`: the whole unit fits 2048 tokens"
        );
        let unit = v2(&text);
        assert_eq!(unit.header[0], "foundry retrieve");
        assert!(
            unit.next.is_none(),
            "`{query}`: no continuation — the WHOLE unit arrived within 2048"
        );
        let item = &unit.items[0];
        assert!(
            item.form.is_none(),
            "`{query}`: the unit is verbatim, not {:#?}",
            item.form
        );
        assert!(
            item.body.contains(marker),
            "`{query}`: the retrieved unit contains its definition"
        );
        // The retrieved item's handle and bytes equal the FROZEN unit, not
        // whatever slice the server chose: exact byte range, exact line
        // numbers, and the corpus bytes at the frozen offsets.
        let range = HandleRef::parse(&item.handle).unwrap();
        assert_eq!(
            (range.path.as_str(), range.start, range.end),
            (path, start, end),
            "`{query}`: the retrieved handle is the frozen unit range"
        );
        assert_eq!(
            item.lines.as_deref(),
            Some(format!("L{first_line}-{last_line}").as_str()),
            "`{query}`: the retrieved item spans the frozen unit lines"
        );
        let file = corpus_bytes(&corpus.root, path);
        assert_eq!(
            item.body.as_bytes(),
            &file[start as usize..end as usize],
            "`{query}`: the retrieved bytes are the frozen corpus unit exactly"
        );
    }
}

#[tokio::test]
async fn question_contexts_surface_their_expected_units() {
    let corpus = measured_corpus();
    let mut server = RawStdio::start(&corpus.store, &corpus.root).await;

    for (question, marker) in QUESTIONS {
        let call = server
            .call(
                "context",
                serde_json::json!({"query": question, "tokens": 2048}),
            )
            .await;
        assert!(!call.is_error(), "`{question}` failed: {}", call.raw);
        let text = call.text();
        assert!(
            count_tokens(&text) <= 2048,
            "`{question}`: context stays within 2048 tokens"
        );
        let parsed = v2(&text);
        assert_eq!(parsed.header[0], "foundry context");
        assert!(
            text.contains(marker),
            "`{question}` does not surface its expected unit ({marker}):\n{text}"
        );
        // Every fenced verbatim body is exactly its handle's corpus bytes.
        for item in &parsed.items {
            if item.kind != context_foundry::testkit::V2Kind::Source
                || item.form.is_some()
                || item.handle.is_empty()
            {
                continue;
            }
            let range = HandleRef::parse(&item.handle).unwrap();
            let file = corpus_bytes(&corpus.root, &range.path);
            assert_eq!(
                item.body.as_bytes(),
                &file[range.start as usize..range.end as usize],
                "`{question}`: a verbatim context body is exact corpus bytes"
            );
        }
    }
}

/// The per-query payload table (adapter-economics § Payload measurement):
/// exact o200k tokens of the emitted text blocks of `search` (limit 10),
/// `search` + unit `retrieve` of hit #1 (2048), `context` (2048), and the
/// serialized `tools/list` result.
#[tokio::test]
#[ignore = "measurement report; run with --ignored --nocapture"]
async fn payload_report() {
    let corpus = measured_corpus();
    let mut server = RawStdio::start(&corpus.store, &corpus.root).await;

    println!("query | search10 | search10+retrieve2048 | context2048");
    let mut queries: Vec<&str> = IDENTIFIERS.iter().map(|unit| unit.query).collect();
    queries.extend(QUESTIONS.iter().map(|(q, _)| *q));
    for query in queries {
        let search = search10(&mut server, query).await;
        assert!(!search.is_error(), "{query}: {}", search.raw);
        let search_tokens = count_tokens(&search.text());
        let hit = v2(&search.text())
            .items
            .first()
            .unwrap_or_else(|| panic!("{query}: search returned no hits"))
            .handle
            .clone();
        let retrieve = server
            .call(
                "retrieve",
                serde_json::json!({"handle": hit, "tokens": 2048}),
            )
            .await;
        assert!(!retrieve.is_error(), "{query}: {}", retrieve.raw);
        let retrieve_tokens = count_tokens(&retrieve.text());
        let context = server
            .call(
                "context",
                serde_json::json!({"query": query, "tokens": 2048}),
            )
            .await;
        assert!(!context.is_error(), "{query}: {}", context.raw);
        let context_tokens = count_tokens(&context.text());
        println!(
            "{query} | {search_tokens} | {} | {context_tokens}",
            search_tokens + retrieve_tokens
        );
    }
    let listed = server.rpc("tools/list", serde_json::json!({})).await;
    println!(
        "tools/list serialized result: {} o200k tokens",
        count_tokens(&listed)
    );
}
