//! 003 T004 behavioral verification of the owned model gateway: the real
//! `foundry gateway` and `foundry gateway-omp` binaries against a local fake
//! upstream that writes raw bytes (so frame splits are controllable). No real
//! credentials and no network: the upstream key is a synthetic canary that
//! must never appear in client-visible bytes, stdout/stderr or receipts.
//!
//! Tests defend behavior (codes, statuses, bytes, counts), never wording.
//!
//! NOT RUN during implementation (mid-flight builds/tests are forbidden);
//! the captain's single post-settlement run executes them.

use std::{
    collections::VecDeque,
    io::{BufRead as _, Read as _},
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use context_foundry::receipts::{Delivery, Outcome, Receipt};
use futures_util::StreamExt as _;
use rmcp::{ServiceExt, model::CallToolRequestParams, transport::TokioChildProcess};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::Notify,
};

const BIN: &str = env!("CARGO_BIN_EXE_foundry");
const FAULTS_BIN: &str = env!("CARGO_BIN_EXE_foundry-faults");
const KEY_CANARY: &str = "zai-canary-3f9a1c7e5b2d4a60";
const KEY_ENV: &str = "FOUNDRY_TEST_GATEWAY_KEY";
const PINNED: &str = "https://api.z.ai/api/coding/paas/v4";
const WAIT: Duration = Duration::from_secs(20);
const TURN: &str = "omp-18.6.0-turn.json";

// ---------------------------------------------------------------------------
// Request fixtures and SSE builders
// ---------------------------------------------------------------------------

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/gateway")
            .join(name),
    )
    .unwrap()
}

/// The turn fixture with one edit applied (re-serialized JSON).
fn mutated(edit: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(&fixture(TURN)).unwrap();
    edit(&mut value);
    serde_json::to_vec(&value).unwrap()
}

/// A valid request whose JSON nesting is exactly `4 + levels` containers deep.
fn deep_request(levels: usize) -> Vec<u8> {
    let mut nested = String::from("1");
    for _ in 0..levels {
        nested = format!("{{\"a\":{nested}}}");
    }
    format!(
        r#"{{"model":"glm-5.3-flash","messages":[{{"role":"user","content":"x"}}],"stream":true,"tools":[{{"type":"function","function":{{"name":"f","parameters":{nested}}}}}]}}"#
    )
    .into_bytes()
}

fn data_event(value: &Value) -> Vec<u8> {
    format!("data: {value}\n\n").into_bytes()
}

fn delta_event(text: &str) -> Vec<u8> {
    data_event(&json!({
        "id": "chatcmpl-1",
        "choices": [{"index": 0, "delta": {"content": text}}],
    }))
}

fn usage_object(input: u64, output: u64, cached: Option<u64>) -> Value {
    let mut usage = json!({
        "prompt_tokens": input,
        "completion_tokens": output,
        "total_tokens": input + output,
    });
    if let Some(cached) = cached {
        usage["prompt_tokens_details"] = json!({"cached_tokens": cached});
    }
    usage
}

fn final_chunk(usage: Option<Value>) -> Vec<u8> {
    let mut chunk = json!({
        "id": "chatcmpl-1",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
    });
    if let Some(usage) = usage {
        chunk["usage"] = usage;
    }
    data_event(&chunk)
}

fn usage_only_chunk(usage: Value) -> Vec<u8> {
    data_event(&json!({"id": "chatcmpl-1", "choices": [], "usage": usage}))
}

fn done_event() -> Vec<u8> {
    b"data: [DONE]\n\n".to_vec()
}

fn simple_stream() -> Vec<Vec<u8>> {
    vec![
        delta_event("hello"),
        final_chunk(Some(usage_object(10, 5, None))),
        done_event(),
    ]
}

// ---------------------------------------------------------------------------
// Fake upstream: a local tokio server writing raw bytes
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum Step {
    Write(Vec<u8>),
    Sleep(u64),
    /// Wait until the test releases the gate.
    Gate(Arc<Notify>),
    /// Keep the connection open until the peer closes it.
    Hold,
}

struct Reply {
    steps: Vec<Step>,
}

impl Reply {
    fn sse_head(extra_headers: &str) -> Step {
        Step::Write(
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\nconnection: close\r\n{extra_headers}\r\n"
            )
            .into_bytes(),
        )
    }

    fn sse(extra_headers: &str, parts: Vec<Vec<u8>>) -> Self {
        let mut steps = vec![Self::sse_head(extra_headers)];
        steps.extend(parts.into_iter().map(Step::Write));
        Self { steps }
    }

    fn sse_steps(extra_headers: &str, rest: Vec<Step>) -> Self {
        let mut steps = vec![Self::sse_head(extra_headers)];
        steps.extend(rest);
        Self { steps }
    }

    fn status(code: u16, reason: &str, extra_headers: &str, body: &str) -> Self {
        Self {
            steps: vec![Step::Write(
                format!(
                    "HTTP/1.1 {code} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n{extra_headers}\r\n{body}",
                    body.len()
                )
                .into_bytes(),
            )],
        }
    }

    fn raw(steps: Vec<Step>) -> Self {
        Self { steps }
    }
}

struct Received {
    head: String,
    body: Vec<u8>,
    peer_closed: Arc<AtomicBool>,
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn header_of(head: &str, name: &str) -> Option<String> {
    head.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_owned())
    })
}

impl Received {
    fn header(&self, name: &str) -> Option<String> {
        header_of(&self.head, name)
    }
}

type ReceivedLog = Arc<Mutex<Vec<Arc<Received>>>>;
type ReplyQueue = Arc<Mutex<VecDeque<Reply>>>;

struct FakeUpstream {
    port: u16,
    received: ReceivedLog,
    replies: ReplyQueue,
}

impl FakeUpstream {
    async fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let received: ReceivedLog = Arc::default();
        let replies: ReplyQueue = Arc::default();
        let (log, queue) = (Arc::clone(&received), Arc::clone(&replies));
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(serve_upstream(stream, Arc::clone(&log), Arc::clone(&queue)));
            }
        });
        Self {
            port,
            received,
            replies,
        }
    }

    fn queue(&self, reply: Reply) {
        self.replies.lock().unwrap().push_back(reply);
    }

    fn received(&self) -> Vec<Arc<Received>> {
        self.received.lock().unwrap().clone()
    }

    fn count(&self) -> usize {
        self.received.lock().unwrap().len()
    }
}

async fn serve_upstream(mut stream: TcpStream, received: ReceivedLog, replies: ReplyQueue) {
    let _ = stream.set_nodelay(true);
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];
    let head_end = loop {
        if let Some(position) = find(&buffer, b"\r\n\r\n") {
            break position + 4;
        }
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buffer.extend_from_slice(&chunk[..n]),
        }
    };
    let head = String::from_utf8_lossy(&buffer[..head_end]).into_owned();
    let length = header_of(&head, "content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    while buffer.len() < head_end + length {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buffer.extend_from_slice(&chunk[..n]),
        }
    }
    let record = Arc::new(Received {
        head,
        body: buffer[head_end..head_end + length].to_vec(),
        peer_closed: Arc::new(AtomicBool::new(false)),
    });
    received.lock().unwrap().push(Arc::clone(&record));
    let reply = replies.lock().unwrap().pop_front();
    let Some(reply) = reply else {
        let _ = stream
            .write_all(
                b"HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
            )
            .await;
        return;
    };
    for step in reply.steps {
        match step {
            Step::Write(bytes) => {
                if stream.write_all(&bytes).await.is_err() {
                    return;
                }
                let _ = stream.flush().await;
            }
            Step::Sleep(millis) => tokio::time::sleep(Duration::from_millis(millis)).await,
            Step::Gate(gate) => gate.notified().await,
            Step::Hold => {
                loop {
                    match stream.read(&mut chunk).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
                record.peer_closed.store(true, Ordering::SeqCst);
                return;
            }
        }
    }
    let _ = stream.shutdown().await;
}

/// Upstream-visible request shape: exactly the pinned headers, the original
/// bytes' path, and neither the local token nor any client header.
fn assert_upstream_request(received: &Received, token: &str) {
    assert!(
        received
            .head
            .starts_with("POST /api/coding/paas/v4/chat/completions HTTP/1.1"),
        "the request goes to <upstream>/chat/completions"
    );
    assert_eq!(
        received.header("authorization").as_deref(),
        Some(format!("Bearer {KEY_CANARY}").as_str()),
        "upstream authentication is the gateway's own"
    );
    assert_eq!(
        received.header("content-type").as_deref(),
        Some("application/json")
    );
    assert_eq!(
        received.header("accept").as_deref(),
        Some("text/event-stream")
    );
    assert_eq!(
        received.header("user-agent").as_deref(),
        Some("omp/18.6.0"),
        "the client user-agent is the one forwarded header"
    );
    let allowed = [
        "host",
        "content-length",
        "content-type",
        "accept",
        "authorization",
        "user-agent",
    ];
    for line in received
        .head
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
    {
        let name = line.split(':').next().unwrap().trim().to_ascii_lowercase();
        assert!(allowed.contains(&name.as_str()), "unexpected header {name}");
    }
    assert!(
        !received.head.contains(token),
        "the local token never reaches upstream"
    );
}

// ---------------------------------------------------------------------------
// Process helpers
// ---------------------------------------------------------------------------

fn run_with_deadline(mut command: Command, limit: Duration) -> std::process::Output {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + limit;
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the process kept running past {limit:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    child.wait_with_output().unwrap()
}

/// The code of the last stderr line (the bounded error JSON).
fn error_code(output: &std::process::Output) -> String {
    let text = String::from_utf8_lossy(&output.stderr).into_owned();
    let line = text.lines().last().unwrap_or_else(|| panic!("no stderr"));
    serde_json::from_str::<Value>(line)
        .unwrap_or_else(|_| panic!("not a bounded error line: {line}"))["code"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn write_config(dir: &Path, run_dir: &Path, log_bytes: Option<u64>) -> PathBuf {
    let mut config = json!({
        "v": 1,
        "port": 0,
        "upstream": PINNED,
        "model": "glm-5.3-flash",
        "mode": "meter",
        "credential_env": KEY_ENV,
        "run_dir": run_dir,
    });
    if let Some(bytes) = log_bytes {
        config["log_bytes"] = json!(bytes);
    }
    let path = dir.join(format!(
        "{}.json",
        run_dir.file_name().unwrap().to_string_lossy()
    ));
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    path
}

#[derive(Default)]
struct Spawn {
    upstream: Option<u16>,
    log_bytes: Option<u64>,
    idle_ms: Option<u64>,
    deadline_ms: Option<u64>,
    run_name: Option<&'static str>,
    /// The global `--store` flag, which the gateway must never open.
    store: Option<PathBuf>,
}

struct Gateway {
    child: Child,
    ready: Value,
    token: String,
    port: u16,
    run_dir: PathBuf,
    config: PathBuf,
    stdout: Arc<Mutex<Vec<String>>>,
    stderr: Arc<Mutex<String>>,
    threads: Vec<std::thread::JoinHandle<()>>,
    client: reqwest::Client,
}

struct Finished {
    stdout: Vec<String>,
    stderr: String,
    receipts_text: String,
    status: std::process::ExitStatus,
    elapsed: Duration,
    token: String,
}

impl Finished {
    fn final_line(&self) -> Value {
        serde_json::from_str(self.stdout.last().expect("a final summary line")).unwrap()
    }

    fn assert_no_secrets(&self) {
        for (what, text) in [
            ("stdout", self.stdout.join("\n")),
            ("stderr", self.stderr.clone()),
            ("receipts", self.receipts_text.clone()),
        ] {
            assert!(
                !text.contains(KEY_CANARY),
                "the upstream key leaked into {what}"
            );
            assert!(
                !text.contains(&self.token),
                "the local token leaked into {what}"
            );
        }
    }
}

impl Gateway {
    fn start(dir: &Path, spawn: Spawn) -> Self {
        let run_dir = dir.join(spawn.run_name.unwrap_or("run"));
        let config = write_config(dir, &run_dir, spawn.log_bytes);
        let mut command = Command::new(BIN);
        if let Some(store) = &spawn.store {
            command.arg("--store").arg(store);
        }
        command
            .arg("gateway")
            .arg("--config")
            .arg(&config)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env(KEY_ENV, KEY_CANARY)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(port) = spawn.upstream {
            command.env(
                "FOUNDRY_GATEWAY_TEST_UPSTREAM",
                format!("http://127.0.0.1:{port}/api/coding/paas/v4"),
            );
        }
        if let Some(millis) = spawn.idle_ms {
            command.env("FOUNDRY_GATEWAY_TEST_IDLE_MS", millis.to_string());
        }
        if let Some(millis) = spawn.deadline_ms {
            command.env("FOUNDRY_GATEWAY_TEST_DEADLINE_MS", millis.to_string());
        }
        let mut child = command.spawn().unwrap();
        let stdout: Arc<Mutex<Vec<String>>> = Arc::default();
        let stderr: Arc<Mutex<String>> = Arc::default();
        let out_pipe = child.stdout.take().unwrap();
        let mut err_pipe = child.stderr.take().unwrap();
        let mut threads = Vec::new();
        let lines = Arc::clone(&stdout);
        threads.push(std::thread::spawn(move || {
            for line in std::io::BufReader::new(out_pipe)
                .lines()
                .map_while(Result::ok)
            {
                lines.lock().unwrap().push(line);
            }
        }));
        let text = Arc::clone(&stderr);
        threads.push(std::thread::spawn(move || {
            let mut buffer = String::new();
            let _ = err_pipe.read_to_string(&mut buffer);
            text.lock().unwrap().push_str(&buffer);
        }));
        let deadline = Instant::now() + Duration::from_secs(15);
        let ready_line = loop {
            let first = stdout.lock().unwrap().first().cloned();
            if let Some(line) = first {
                break line;
            }
            if let Ok(Some(status)) = child.try_wait() {
                for thread in threads.drain(..) {
                    let _ = thread.join();
                }
                panic!(
                    "the gateway exited early ({status}): {}",
                    stderr.lock().unwrap()
                );
            }
            assert!(
                Instant::now() < deadline,
                "the gateway never reported readiness"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        let ready: Value = serde_json::from_str(&ready_line).unwrap();
        let port = ready["port"].as_u64().unwrap() as u16;
        let token = std::fs::read_to_string(ready["token_file"].as_str().unwrap())
            .unwrap()
            .trim()
            .to_owned();
        Self {
            child,
            ready,
            token,
            port,
            run_dir,
            config,
            stdout,
            stderr,
            threads,
            client: reqwest::Client::builder()
                .no_proxy()
                .pool_max_idle_per_host(0)
                .build()
                .unwrap(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    fn bearer(&self) -> String {
        format!("Bearer {}", self.token)
    }

    fn session_id(&self) -> String {
        self.ready["session_id"].as_str().unwrap().to_owned()
    }

    /// An unauthenticated request builder.
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.client.request(method, self.url(path))
    }

    async fn post(&self, body: Vec<u8>) -> reqwest::Response {
        self.request(reqwest::Method::POST, "/v1/chat/completions")
            .header("authorization", self.bearer())
            .header("content-type", "application/json")
            .header("user-agent", "omp/18.6.0")
            .body(body)
            .send()
            .await
            .unwrap()
    }

    async fn health(&self) -> Value {
        self.request(reqwest::Method::GET, "/health")
            .header("authorization", self.bearer())
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    /// Only complete lines: a receipt being appended is never half-read.
    fn receipts(&self) -> Vec<Receipt> {
        let text = std::fs::read_to_string(self.run_dir.join("receipts.jsonl")).unwrap_or_default();
        let end = text.rfind('\n').map_or(0, |index| index + 1);
        text[..end]
            .lines()
            .map(|line| {
                Receipt::parse(line.as_bytes()).expect("a receipt the strict reader accepts")
            })
            .collect()
    }

    async fn wait_receipts(&self, count: usize) -> Vec<Receipt> {
        let deadline = Instant::now() + WAIT;
        loop {
            let receipts = self.receipts();
            if receipts.len() >= count {
                return receipts;
            }
            assert!(
                Instant::now() < deadline,
                "expected {count} receipts, found {}; stderr: {}",
                receipts.len(),
                self.stderr.lock().unwrap()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    fn terminate(&mut self) -> Finished {
        let signalled = Instant::now();
        // SAFETY: kill(2) on this test's own live child.
        unsafe { libc::kill(self.child.id() as i32, libc::SIGTERM) };
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(
                signalled.elapsed() < Duration::from_secs(8),
                "the gateway did not exit after SIGTERM"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        let elapsed = signalled.elapsed();
        for thread in self.threads.drain(..) {
            thread.join().unwrap();
        }
        Finished {
            stdout: self.stdout.lock().unwrap().clone(),
            stderr: self.stderr.lock().unwrap().clone(),
            receipts_text: std::fs::read_to_string(self.run_dir.join("receipts.jsonl"))
                .unwrap_or_default(),
            status,
            elapsed,
            token: self.token.clone(),
        }
    }
}

impl Drop for Gateway {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

// ---------------------------------------------------------------------------
// Client-side helpers
// ---------------------------------------------------------------------------

/// Whole response body plus whether the stream ended cleanly.
async fn read_all(response: reqwest::Response) -> (Vec<u8>, bool) {
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) => body.extend_from_slice(&chunk),
            Err(_) => return (body, false),
        }
    }
    (body, true)
}

struct RawReply {
    status: Option<u16>,
    body: Vec<u8>,
}

fn parse_reply(data: &[u8]) -> RawReply {
    let (head, body) = match find(data, b"\r\n\r\n") {
        Some(position) => (&data[..position], data[position + 4..].to_vec()),
        None => (data, Vec::new()),
    };
    let head = String::from_utf8_lossy(head).into_owned();
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok());
    RawReply { status, body }
}

/// Read until the peer closes (a reset counts as the end), within 10 s.
async fn read_to_end(stream: &mut (impl tokio::io::AsyncRead + Unpin)) -> Vec<u8> {
    let mut data = Vec::new();
    let mut chunk = [0u8; 8192];
    let _ = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => data.extend_from_slice(&chunk[..n]),
            }
        }
    })
    .await;
    data
}

async fn raw_exchange(port: u16, request: &[u8]) -> RawReply {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let _ = stream.write_all(request).await;
    parse_reply(&read_to_end(&mut stream).await)
}

/// A request head with `Connection: close`; extra lines are `Name: value`.
fn raw_head(port: u16, method: &str, path: &str, extra: &[String]) -> Vec<u8> {
    let mut lines = vec![
        format!("{method} {path} HTTP/1.1"),
        format!("Host: 127.0.0.1:{port}"),
        "Connection: close".to_owned(),
    ];
    lines.extend(extra.iter().cloned());
    format!("{}\r\n\r\n", lines.join("\r\n")).into_bytes()
}

/// POST the body on a raw socket and leave the stream open for reading.
async fn open_stream(gateway: &Gateway, body: &[u8]) -> TcpStream {
    open_stream_with(gateway, body, "").await
}

/// `extra` is zero or more complete `Name: value\r\n` header lines.
async fn open_stream_with(gateway: &Gateway, body: &[u8], extra: &str) -> TcpStream {
    let mut stream = TcpStream::connect(("127.0.0.1", gateway.port))
        .await
        .unwrap();
    let head = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nUser-Agent: omp/18.6.0\r\n{extra}Connection: close\r\n\r\n",
        gateway.port,
        gateway.bearer(),
        body.len()
    );
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();
    stream
}

async fn read_until(stream: &mut TcpStream, needle: &[u8]) -> Vec<u8> {
    let mut data = Vec::new();
    let mut chunk = [0u8; 8192];
    let deadline = Instant::now() + WAIT;
    while find(&data, needle).is_none() {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(
            !left.is_zero(),
            "never saw {:?}",
            String::from_utf8_lossy(needle)
        );
        match tokio::time::timeout(left, stream.read(&mut chunk)).await {
            Ok(Ok(0)) | Ok(Err(_)) => panic!(
                "the stream ended before {:?}: {:?}",
                String::from_utf8_lossy(needle),
                String::from_utf8_lossy(&data)
            ),
            Ok(Ok(n)) => data.extend_from_slice(&chunk[..n]),
            Err(_) => panic!(
                "timed out waiting for {:?}",
                String::from_utf8_lossy(needle)
            ),
        }
    }
    data
}

fn assert_no_cors(response: &reqwest::Response) {
    for name in response.headers().keys() {
        assert!(
            !name.as_str().starts_with("access-control-"),
            "CORS header {name} must never be sent"
        );
    }
}

fn assert_counts(receipt: &Receipt, input: Option<u64>, cached: Option<u64>, output: Option<u64>) {
    assert_eq!(receipt.input_tokens, input, "input");
    assert_eq!(receipt.cached_input_tokens, cached, "cached input");
    assert_eq!(receipt.output_tokens, output, "output");
}

fn delivery_of(receipt: &Receipt) -> Option<Delivery> {
    receipt.observation.as_ref().and_then(|o| o.delivery)
}

async fn error_of(response: reqwest::Response) -> (u16, String) {
    let status = response.status().as_u16();
    let body: Value = response.json().await.unwrap();
    (status, body["error"]["code"].as_str().unwrap().to_owned())
}

// ---------------------------------------------------------------------------
// Accepted input
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_pinned_fixtures_forward_byte_identical_bodies() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    let expected = simple_stream().concat();
    for (index, name) in [
        TURN,
        "omp-18.6.0-tool-continuation.json",
        "omp-18.6.0-auto-judge.json",
    ]
    .into_iter()
    .enumerate()
    {
        let sent = fixture(name);
        upstream.queue(Reply::sse("x-request-id: up-req-1\r\n", simple_stream()));
        let response = gateway.post(sent.clone()).await;
        assert_eq!(response.status(), 200, "{name}");
        let headers = response.headers().clone();
        assert_eq!(
            headers["content-type"].to_str().unwrap(),
            "text/event-stream"
        );
        assert_eq!(headers["cache-control"].to_str().unwrap(), "no-cache");
        assert_eq!(headers["x-request-id"].to_str().unwrap(), "up-req-1");
        assert_no_cors(&response);
        let (body, clean) = read_all(response).await;
        assert!(clean, "{name}: the stream ends cleanly");
        assert_eq!(body, expected, "{name}: the forwarded stream is byte-exact");

        let receipts = gateway.wait_receipts(index + 1).await;
        let receipt = &receipts[index];
        assert_eq!(receipt.adapter_id, "foundry-gateway");
        assert_eq!(receipt.model_id, "glm-5.3-flash");
        assert_eq!(receipt.session_id, gateway.session_id());
        assert_eq!(receipt.outcome, Outcome::Complete);
        assert_counts(receipt, Some(10), None, Some(5));
        assert_eq!(delivery_of(receipt), Some(Delivery::Delivered));
        let observation = receipt.observation.as_ref().unwrap();
        assert_eq!(observation.provider_request_id.as_deref(), Some("up-req-1"));
        assert_eq!(
            observation.provider_response_id.as_deref(),
            Some("chatcmpl-1")
        );

        let seen = upstream.received();
        assert_eq!(
            seen[index].body, sent,
            "{name}: the original bytes reach upstream"
        );
        assert_upstream_request(&seen[index], &gateway.token);
    }
    let ids: std::collections::HashSet<_> = gateway
        .receipts()
        .iter()
        .map(|receipt| receipt.request_id.clone())
        .collect();
    assert_eq!(ids.len(), 3, "each attempt has its own request id");
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nesting_depth_64_is_accepted_and_65_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    upstream.queue(Reply::sse("", simple_stream()));
    let accepted = gateway.post(deep_request(60)).await;
    assert_eq!(accepted.status(), 200, "depth 64 is inside the bound");
    assert!(read_all(accepted).await.1);
    gateway.wait_receipts(1).await;
    let refused = gateway.post(deep_request(61)).await;
    assert_eq!(error_of(refused).await, (400, "invalid_argument".into()));
    assert_eq!(upstream.count(), 1, "only the accepted request was sent");
    gateway.terminate().assert_no_secrets();
}

// ---------------------------------------------------------------------------
// Refusals: nothing reaches upstream
// ---------------------------------------------------------------------------

struct Case {
    name: &'static str,
    body: Vec<u8>,
    headers: Vec<(&'static str, &'static str)>,
    content_type: &'static str,
    status: u16,
    code: &'static str,
}

fn case(name: &'static str, body: Vec<u8>, status: u16, code: &'static str) -> Case {
    Case {
        name,
        body,
        headers: Vec::new(),
        content_type: "application/json",
        status,
        code,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_refused_request_reaches_nothing_upstream() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    let unsupported = "gateway_feature_unsupported";
    let invalid = "invalid_argument";
    let duplicate = br#"{"model":"glm-5.3-flash","model":"glm-5.3-flash","messages":[{"role":"user","content":"x"}],"stream":true}"#;
    let mut cases = vec![
        case(
            "extra top-level field",
            mutated(|v| v["extra"] = json!(1)),
            400,
            unsupported,
        ),
        case(
            "image part",
            mutated(|v| {
                v["messages"][2]["content"] = json!([{"type": "image_url", "image_url": {"url": "data:image/png;base64,AA=="}}]);
            }),
            400,
            unsupported,
        ),
        case(
            "wrong model",
            mutated(|v| v["model"] = json!("glm-5.3")),
            400,
            unsupported,
        ),
        case(
            "stream false",
            mutated(|v| v["stream"] = json!(false)),
            400,
            unsupported,
        ),
        case(
            "missing stream",
            mutated(|v| {
                v.as_object_mut().unwrap().remove("stream");
            }),
            400,
            unsupported,
        ),
        case(
            "max_tokens over the window",
            mutated(|v| v["max_tokens"] = json!(131_073)),
            400,
            unsupported,
        ),
        case("duplicate keys", duplicate.to_vec(), 400, invalid),
        case("nesting depth 65", deep_request(61), 400, invalid),
        case(
            "invalid UTF-8",
            b"{\"model\":\"\xff\"}".to_vec(),
            400,
            invalid,
        ),
        case("not JSON", b"not json".to_vec(), 400, invalid),
    ];
    let mut gzip = case("gzip content-encoding", fixture(TURN), 415, unsupported);
    gzip.headers.push(("content-encoding", "gzip"));
    cases.push(gzip);
    let mut plain = case("wrong content-type", fixture(TURN), 415, invalid);
    plain.content_type = "text/plain";
    cases.push(plain);
    let mut bad_ids = case("invalid context ids", fixture(TURN), 400, invalid);
    bad_ids
        .headers
        .push(("x-foundry-context-ids", "not-a-uuid"));
    cases.push(bad_ids);

    for case in cases {
        let mut request = gateway
            .request(reqwest::Method::POST, "/v1/chat/completions")
            .header("authorization", gateway.bearer())
            .header("content-type", case.content_type)
            .body(case.body.clone());
        for (name, value) in &case.headers {
            request = request.header(*name, *value);
        }
        let response = request.send().await.unwrap();
        let (status, code) = error_of(response).await;
        assert_eq!(
            (status, code.as_str()),
            (case.status, case.code),
            "{}",
            case.name
        );
    }

    // Over 4 MiB by Content-Length: refused before the body is read.
    let head = raw_head(
        gateway.port,
        "POST",
        "/v1/chat/completions",
        &[
            format!("Authorization: {}", gateway.bearer()),
            "Content-Type: application/json".to_owned(),
            format!("Content-Length: {}", 4 * 1024 * 1024 + 1),
        ],
    );
    let reply = raw_exchange(gateway.port, &head).await;
    assert_eq!(reply.status, Some(413));
    let body: Value = serde_json::from_slice(&reply.body).unwrap();
    assert_eq!(body["error"]["code"], "request_too_large");

    // A streamed body is cut at 4 MiB: the reply is 413, or the connection
    // is reset before it is read; either way nothing is sent upstream.
    let head = raw_head(
        gateway.port,
        "POST",
        "/v1/chat/completions",
        &[
            format!("Authorization: {}", gateway.bearer()),
            "Content-Type: application/json".to_owned(),
            "Transfer-Encoding: chunked".to_owned(),
        ],
    );
    let stream = TcpStream::connect(("127.0.0.1", gateway.port))
        .await
        .unwrap();
    let (mut reader, mut writer) = stream.into_split();
    writer.write_all(&head).await.unwrap();
    let pusher = tokio::spawn(async move {
        let block = vec![b'a'; 64 * 1024];
        for _ in 0..70 {
            let framing = format!("{:x}\r\n", block.len());
            if writer.write_all(framing.as_bytes()).await.is_err()
                || writer.write_all(&block).await.is_err()
                || writer.write_all(b"\r\n").await.is_err()
            {
                return;
            }
        }
        let _ = writer.write_all(b"0\r\n\r\n").await;
    });
    let reply = parse_reply(&read_to_end(&mut reader).await);
    pusher.abort();
    assert!(
        matches!(reply.status, Some(413) | None),
        "a streamed oversize body is refused: {:?}",
        reply.status
    );

    assert_eq!(upstream.count(), 0, "no refused request reached upstream");
    assert_eq!(
        gateway.health().await["attempts"],
        0,
        "no attempt was recorded"
    );
    assert!(gateway.receipts().is_empty());
    gateway.terminate().assert_no_secrets();
}

// ---------------------------------------------------------------------------
// Usage placement and SSE handling
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn usage_is_taken_from_the_provider_chunk_wherever_it_sits() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    let nested_usage = data_event(&json!({
        "id": "chatcmpl-1",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop",
                     "usage": usage_object(300, 50, Some(0))}],
    }));
    // (parts, outcome, input, cached, output)
    type Scenario = (Vec<Vec<u8>>, Outcome, Option<u64>, Option<u64>, Option<u64>);
    let scenarios: Vec<Scenario> = vec![
        (
            vec![
                delta_event("a"),
                final_chunk(Some(usage_object(100, 20, Some(60)))),
                done_event(),
            ],
            Outcome::Complete,
            Some(100),
            Some(60),
            Some(20),
        ),
        (
            vec![
                delta_event("a"),
                final_chunk(None),
                usage_only_chunk(usage_object(200, 40, Some(150))),
                done_event(),
            ],
            Outcome::Complete,
            Some(200),
            Some(150),
            Some(40),
        ),
        (
            vec![delta_event("a"), nested_usage, done_event()],
            Outcome::Complete,
            Some(300),
            Some(0),
            Some(50),
        ),
        (
            vec![delta_event("a"), final_chunk(None), done_event()],
            Outcome::Unknown,
            None,
            None,
            None,
        ),
        (
            vec![
                delta_event("a"),
                final_chunk(Some(usage_object(400, 80, None))),
                done_event(),
            ],
            Outcome::Complete,
            Some(400),
            None,
            Some(80),
        ),
    ];
    for (index, (parts, outcome, input, cached, output)) in scenarios.into_iter().enumerate() {
        let expected = parts.concat();
        upstream.queue(Reply::sse("x-request-id: up-req-usage\r\n", parts));
        let response = gateway.post(fixture(TURN)).await;
        assert_eq!(response.status(), 200, "scenario {index}");
        let (body, clean) = read_all(response).await;
        assert!(clean, "scenario {index}");
        assert_eq!(
            body, expected,
            "scenario {index}: bytes forwarded unchanged"
        );
        let receipts = gateway.wait_receipts(index + 1).await;
        let receipt = &receipts[index];
        assert_eq!(receipt.outcome, outcome, "scenario {index}");
        assert_counts(receipt, input, cached, output);
        assert_eq!(
            delivery_of(receipt),
            Some(Delivery::Delivered),
            "scenario {index}"
        );
        assert_eq!(
            receipt
                .observation
                .as_ref()
                .unwrap()
                .provider_request_id
                .as_deref(),
            Some("up-req-usage")
        );
    }
    let finished = gateway.terminate();
    let summary = finished.final_line();
    assert_eq!(summary["attempts"], 5);
    assert_eq!(summary["complete"], 4);
    assert_eq!(summary["unknown"], 1);
    assert_eq!(
        summary["input_tokens"],
        100 + 200 + 300 + 400,
        "unknown stays out of totals"
    );
    assert_eq!(
        summary["cached_input_tokens"],
        60 + 150,
        "an absent category adds nothing"
    );
    assert_eq!(summary["coverage"], "whole_run_unverified");
    finished.assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn events_split_across_arbitrary_writes_arrive_byte_exact() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    // Mixed terminators: LF, CRLF, a comment-only event and a bare-CR end.
    let crlf = {
        let mut event = delta_event("beta");
        event.truncate(event.len() - 2);
        event.extend_from_slice(b"\r\n\r\n");
        event
    };
    let cr_usage = {
        let mut event = final_chunk(Some(usage_object(50, 7, Some(10))));
        event.truncate(event.len() - 2);
        event.extend_from_slice(b"\r\r");
        event
    };
    let events = [
        delta_event("alpha"),
        crlf,
        b": keep-alive\n\n".to_vec(),
        cr_usage,
        done_event(),
    ];
    let expected = events.concat();
    let mut steps = Vec::new();
    for (index, byte) in expected.iter().enumerate() {
        steps.push(Step::Write(vec![*byte]));
        if index % 4 == 0 {
            steps.push(Step::Sleep(1));
        }
    }
    upstream.queue(Reply::sse_steps("", steps));
    let response = gateway.post(fixture(TURN)).await;
    assert_eq!(response.status(), 200);
    let (body, clean) = read_all(response).await;
    assert!(clean);
    assert_eq!(
        body, expected,
        "byte-by-byte delivery is forwarded byte-exact"
    );
    let receipts = gateway.wait_receipts(1).await;
    assert_eq!(receipts[0].outcome, Outcome::Complete);
    assert_counts(&receipts[0], Some(50), Some(10), Some(7));
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tool_call_deltas_are_forwarded_in_order_with_done() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    let fragments = ["{\"pa", "th\":\"he", "llo.txt\"}"];
    let mut parts: Vec<Vec<u8>> = fragments
        .iter()
        .enumerate()
        .map(|(index, fragment)| {
            let (id, name) = if index == 0 {
                (json!("call_1"), json!("read"))
            } else {
                (Value::Null, Value::Null)
            };
            data_event(&json!({
                "id": "chatcmpl-1",
                "choices": [{"index": 0, "delta": {"tool_calls": [{
                    "index": 0,
                    "id": id,
                    "type": "function",
                    "function": {"name": name, "arguments": fragment},
                }]}}],
            }))
        })
        .collect();
    parts.push(data_event(&json!({
        "id": "chatcmpl-1",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}],
        "usage": usage_object(90, 12, Some(64)),
    })));
    parts.push(done_event());
    let expected = parts.concat();
    upstream.queue(Reply::sse("", parts));
    let response = gateway
        .post(fixture("omp-18.6.0-tool-continuation.json"))
        .await;
    let (body, clean) = read_all(response).await;
    assert!(clean);
    assert_eq!(body, expected, "events arrive unchanged and in order");
    assert!(body.ends_with(b"data: [DONE]\n\n"), "[DONE] is forwarded");
    let text = String::from_utf8(body).unwrap();
    let mut arguments = String::new();
    for event in text.split("\n\n") {
        let Some(payload) = event.strip_prefix("data: ") else {
            continue;
        };
        let Ok(chunk) = serde_json::from_str::<Value>(payload) else {
            continue;
        };
        if let Some(fragment) =
            chunk["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"].as_str()
        {
            arguments.push_str(fragment);
        }
    }
    assert_eq!(
        arguments, "{\"path\":\"hello.txt\"}",
        "fragments keep their order"
    );
    let receipts = gateway.wait_receipts(1).await;
    assert_counts(&receipts[0], Some(90), Some(64), Some(12));
    gateway.terminate().assert_no_secrets();
}

// ---------------------------------------------------------------------------
// Upstream errors
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn upstream_error_bodies_are_replaced_by_a_gateway_body() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    let statuses = [
        (400u16, "Bad Request"),
        (401, "Unauthorized"),
        (429, "Too Many Requests"),
        (500, "Internal Server Error"),
        (503, "Service Unavailable"),
    ];
    for (index, (code, reason)) in statuses.into_iter().enumerate() {
        let body = format!(r#"{{"error":{{"message":"rejected {KEY_CANARY}","code":"{code}"}}}}"#);
        upstream.queue(Reply::status(
            code,
            reason,
            &format!("x-request-id: up-err-{code}\r\n"),
            &body,
        ));
        let response = gateway.post(fixture(TURN)).await;
        assert_eq!(
            response.status().as_u16(),
            code,
            "the status is relayed so retries behave"
        );
        assert!(
            !response.headers().contains_key("x-request-id"),
            "no upstream header is relayed"
        );
        assert!(
            response.headers().get("rate_limit_type").is_none(),
            "an upstream 429 must stay retryable"
        );
        let text = response.text().await.unwrap();
        assert!(
            !text.contains(KEY_CANARY),
            "the upstream body is never forwarded"
        );
        let reply: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(reply["error"]["code"], "upstream_error");
        assert_eq!(reply["error"]["upstream_status"], code);
        assert_eq!(
            reply["error"]["provider_request_id"],
            format!("up-err-{code}")
        );
        let receipts = gateway.wait_receipts(index + 1).await;
        let receipt = &receipts[index];
        assert_eq!(receipt.outcome, Outcome::Failed);
        assert_counts(receipt, None, None, None);
        assert_eq!(
            receipt
                .observation
                .as_ref()
                .unwrap()
                .provider_request_id
                .as_deref(),
            Some(format!("up-err-{code}").as_str())
        );
    }
    // An unapproved provider request id is dropped, not echoed.
    upstream.queue(Reply::status(
        500,
        "Internal Server Error",
        "x-request-id: bad id with spaces\r\n",
        "{}",
    ));
    let response = gateway.post(fixture(TURN)).await;
    let reply: Value = response.json().await.unwrap();
    assert!(reply["error"].get("provider_request_id").is_none());
    let receipts = gateway.wait_receipts(statuses.len() + 1).await;
    assert!(
        receipts
            .last()
            .unwrap()
            .observation
            .as_ref()
            .unwrap()
            .provider_request_id
            .is_none()
    );
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_redirect_is_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let decoy = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    upstream.queue(Reply::raw(vec![Step::Write(
        format!(
            "HTTP/1.1 302 Found\r\nlocation: http://127.0.0.1:{}/steal\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
            decoy.port
        )
        .into_bytes(),
    )]));
    let response = gateway.post(fixture(TURN)).await;
    let status = response.status();
    let headers = response.headers().clone();
    let reply: Value = response.json().await.unwrap();
    assert_eq!(
        status,
        reqwest::StatusCode::FOUND,
        "the original status is relayed"
    );
    assert_eq!(reply["error"]["code"], "upstream_error");
    assert_eq!(reply["error"]["upstream_status"], 302);
    assert!(
        !headers.contains_key("location"),
        "no redirect target is ever relayed"
    );
    assert_eq!(decoy.count(), 0, "the redirect target was never contacted");
    assert_eq!(upstream.count(), 1);
    let receipts = gateway.wait_receipts(1).await;
    assert_eq!(receipts[0].outcome, Outcome::Failed);
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_upstream_sse_error_event_closes_the_stream_unforwarded() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    let before = delta_event("before");
    upstream.queue(Reply::sse(
        "",
        vec![
            before.clone(),
            data_event(
                &json!({"error": {"message": format!("boom {KEY_CANARY}"), "code": "1234"}}),
            ),
            delta_event("after"),
            final_chunk(Some(usage_object(9, 9, None))),
            done_event(),
        ],
    ));
    let response = gateway.post(fixture(TURN)).await;
    assert_eq!(response.status(), 200, "headers were already sent");
    let (body, clean) = read_all(response).await;
    assert_eq!(body, before, "only what preceded the error is forwarded");
    assert!(
        !clean,
        "the stream is aborted, never terminated like a success"
    );
    let receipts = gateway.wait_receipts(1).await;
    assert_eq!(receipts[0].outcome, Outcome::Failed);
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_event_over_one_mib_is_a_local_failure() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    // A valid 900 KiB chunk is inside the bound and passes through unchanged.
    let event_of = |bytes: usize| {
        let mut event = b"data: ".to_vec();
        event.extend_from_slice(
            &serde_json::to_vec(&json!({
                "id": "chatcmpl-big",
                "choices": [{"index": 0, "delta": {"content": "x".repeat(bytes)}}],
            }))
            .unwrap(),
        );
        event.extend_from_slice(b"\n\n");
        event
    };
    let big = event_of(900 * 1024);
    let mut parts = vec![big.clone()];
    parts.push(final_chunk(Some(usage_object(5, 5, None))));
    parts.push(done_event());
    let expected = parts.concat();
    upstream.queue(Reply::sse("", parts));
    let (body, clean) = read_all(gateway.post(fixture(TURN)).await).await;
    assert!(clean);
    assert_eq!(body, expected);
    gateway.wait_receipts(1).await;

    // 1.2 MiB is not: the stream is cut and the attempt fails locally.
    let before = delta_event("before");
    let oversize = event_of(1200 * 1024);
    let mut steps = vec![Step::Write(before.clone())];
    steps.extend(
        oversize
            .chunks(64 * 1024)
            .map(|chunk| Step::Write(chunk.to_vec())),
    );
    steps.push(Step::Write(final_chunk(Some(usage_object(1, 1, None)))));
    upstream.queue(Reply::sse_steps("", steps));
    let (body, clean) = read_all(gateway.post(fixture(TURN)).await).await;
    assert_eq!(body, before, "the oversize event is never forwarded");
    assert!(!clean);
    let receipts = gateway.wait_receipts(2).await;
    assert_eq!(receipts[1].outcome, Outcome::Failed);
    assert_eq!(delivery_of(&receipts[1]), Some(Delivery::LocalFailure));
    assert_counts(&receipts[1], None, None, None);
    gateway.terminate().assert_no_secrets();
}

// ---------------------------------------------------------------------------
// Auth and transport
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn auth_origin_host_query_and_route_refusals() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            ..Spawn::default()
        },
    );
    let port = gateway.port;
    let token = gateway.token.clone();
    let post = |gateway: &Gateway| {
        gateway
            .request(reqwest::Method::POST, "/v1/chat/completions")
            .header("content-type", "application/json")
            .body(fixture(TURN))
    };

    // Missing or wrong bearer: 401, decided before the body is read.
    let wrong = [
        None,
        Some("Bearer wrong".to_owned()),
        Some(format!("Bearer {token}x")),
        Some(format!("Bearer {}", &token[1..])),
        Some(format!("Basic {token}")),
    ];
    for authorization in wrong {
        let mut request = post(&gateway);
        if let Some(value) = &authorization {
            request = request.header("authorization", value);
        }
        let response = request.send().await.unwrap();
        assert_no_cors(&response);
        assert_eq!(
            error_of(response).await,
            (401, "gateway_unauthorized".into()),
            "{authorization:?}"
        );
    }
    let head = raw_head(
        port,
        "POST",
        "/v1/chat/completions",
        &[
            "Authorization: Bearer wrong".to_owned(),
            "Content-Type: application/json".to_owned(),
            "Content-Length: 1000000".to_owned(),
        ],
    );
    let reply = raw_exchange(port, &head).await;
    assert_eq!(
        reply.status,
        Some(401),
        "the bearer is checked before any body arrives"
    );

    // Origin is refused first, with or without valid auth.
    for authorized in [false, true] {
        let mut request = post(&gateway).header("origin", "http://127.0.0.1");
        if authorized {
            request = request.header("authorization", gateway.bearer());
        }
        let response = request.send().await.unwrap();
        assert_no_cors(&response);
        assert_eq!(
            error_of(response).await,
            (403, "gateway_forbidden_origin".into())
        );
    }

    // Only the exact bound authority is accepted as Host.
    for host in [
        format!("localhost:{port}"),
        "127.0.0.1".to_owned(),
        "127.0.0.1:1".to_owned(),
        format!("127.0.0.1:0{port}"),
        "evil.example".to_owned(),
    ] {
        let response = post(&gateway)
            .header("host", host.clone())
            .header("authorization", gateway.bearer())
            .send()
            .await
            .unwrap();
        assert_eq!(
            error_of(response).await,
            (403, "gateway_bad_host".into()),
            "{host}"
        );
    }

    // Query strings, unknown paths, wrong methods and upgrades.
    let response = gateway
        .request(reqwest::Method::POST, "/v1/chat/completions?model=other")
        .header("authorization", gateway.bearer())
        .header("content-type", "application/json")
        .body(fixture(TURN))
        .send()
        .await
        .unwrap();
    assert_eq!(error_of(response).await, (400, "invalid_argument".into()));
    let response = gateway
        .request(reqwest::Method::GET, "/health?x=1")
        .header("authorization", gateway.bearer())
        .send()
        .await
        .unwrap();
    assert_eq!(error_of(response).await, (400, "invalid_argument".into()));
    let routes = [
        (reqwest::Method::POST, "/v1/other", 404),
        (reqwest::Method::POST, "/v1/chat/completions/extra", 404),
        (reqwest::Method::GET, "/v1/chat/completions", 405),
        (reqwest::Method::POST, "/health", 405),
        (reqwest::Method::DELETE, "/v1/chat/completions", 405),
        (reqwest::Method::OPTIONS, "/v1/chat/completions", 405),
    ];
    for (method, path, status) in routes {
        let response = gateway
            .request(method.clone(), path)
            .header("authorization", gateway.bearer())
            .send()
            .await
            .unwrap();
        assert_no_cors(&response);
        assert_eq!(
            error_of(response).await,
            (status, "gateway_feature_unsupported".into()),
            "{method} {path}"
        );
    }
    let head = raw_head(
        port,
        "GET",
        "/health",
        &[
            format!("Authorization: {}", gateway.bearer()),
            "Upgrade: websocket".to_owned(),
        ],
    );
    let reply = raw_exchange(port, &head).await;
    assert_eq!(reply.status, Some(400), "an Upgrade header is refused");

    // Health requires the bearer and never carries credentials.
    let response = gateway
        .request(reqwest::Method::GET, "/health")
        .send()
        .await
        .unwrap();
    assert_eq!(
        error_of(response).await,
        (401, "gateway_unauthorized".into())
    );
    let health = gateway.health().await;
    assert_eq!(health["v"], 1);
    assert_eq!(health["ready"], true);
    assert_eq!(health["mode"], "meter");
    assert_eq!(health["admission"], "open");
    assert_eq!(health["attempts"], 0);
    assert_eq!(health["session_id"], gateway.session_id());
    let text = health.to_string();
    assert!(!text.contains(KEY_CANARY) && !text.contains(&token));

    assert_eq!(upstream.count(), 0, "no refusal reached upstream");
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_ninth_connection_is_closed_unread() {
    let dir = tempfile::tempdir().unwrap();
    let mut gateway = Gateway::start(dir.path(), Spawn::default());
    let mut held = Vec::new();
    for _ in 0..8 {
        held.push(
            TcpStream::connect(("127.0.0.1", gateway.port))
                .await
                .unwrap(),
        );
    }
    let mut ninth = TcpStream::connect(("127.0.0.1", gateway.port))
        .await
        .unwrap();
    let mut buffer = [0u8; 16];
    let read = tokio::time::timeout(Duration::from_secs(3), ninth.read(&mut buffer))
        .await
        .expect("the excess connection is closed promptly, not left waiting");
    assert!(matches!(read, Ok(0) | Err(_)), "closed without a reply");
    drop(held);
    drop(ninth);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        gateway.health().await["ready"],
        true,
        "freed connections admit again"
    );
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn oversized_headers_and_slow_headers_are_cut() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            ..Spawn::default()
        },
    );
    let head = raw_head(
        gateway.port,
        "POST",
        "/v1/chat/completions",
        &[
            format!("Authorization: {}", gateway.bearer()),
            format!("X-Padding: {}", "a".repeat(20 * 1024)),
            "Content-Type: application/json".to_owned(),
            "Content-Length: 2".to_owned(),
        ],
    );
    let reply = raw_exchange(gateway.port, &head).await;
    assert!(
        reply
            .status
            .is_none_or(|status| (400..500).contains(&status)),
        "oversized headers fail before any body: {:?}",
        reply.status
    );

    // A partial head never completes: the connection is cut near 5 s.
    let started = Instant::now();
    let mut stream = TcpStream::connect(("127.0.0.1", gateway.port))
        .await
        .unwrap();
    stream
        .write_all(b"POST /v1/chat/completions HTTP/1.1\r\nHost: 127.0.0.1")
        .await
        .unwrap();
    let _ = read_to_end(&mut stream).await;
    let waited = started.elapsed();
    assert!(
        waited >= Duration::from_secs(4) && waited < Duration::from_secs(9),
        "the header deadline is about five seconds, was {waited:?}"
    );
    assert_eq!(upstream.count(), 0);
    gateway.terminate().assert_no_secrets();
}

// ---------------------------------------------------------------------------
// One generation slot
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_second_concurrent_post_is_busy_and_never_sent_upstream() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    let gate = Arc::new(Notify::new());
    upstream.queue(Reply::sse_steps(
        "",
        vec![
            Step::Write(delta_event("first-delta")),
            Step::Gate(Arc::clone(&gate)),
            Step::Write(final_chunk(Some(usage_object(70, 8, Some(30))))),
            Step::Write(done_event()),
        ],
    ));
    let mut first = open_stream(&gateway, &fixture(TURN)).await;
    read_until(&mut first, b"first-delta").await;

    let response = gateway.post(fixture(TURN)).await;
    assert_eq!(response.status(), 429);
    assert_eq!(
        response.headers()["rate_limit_type"].to_str().unwrap(),
        "max_parallel_requests",
        "OMP 18.6.0's no-retry marker"
    );
    assert_no_cors(&response);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "gateway_busy");
    assert_eq!(
        upstream.count(),
        1,
        "the busy refusal sent nothing upstream"
    );
    assert_eq!(
        gateway.health().await["attempts"],
        1,
        "and is not billed usage"
    );

    gate.notify_one();
    let rest = read_to_end(&mut first).await;
    assert!(
        find(&rest, b"[DONE]").is_some(),
        "the first stream completes"
    );
    let receipts = gateway.wait_receipts(1).await;
    assert_eq!(receipts[0].outcome, Outcome::Complete);
    assert_counts(&receipts[0], Some(70), Some(30), Some(8));

    upstream.queue(Reply::sse("", simple_stream()));
    let response = gateway.post(fixture(TURN)).await;
    assert_eq!(
        response.status(),
        200,
        "the slot is free again after the receipt"
    );
    assert!(read_all(response).await.1);
    gateway.wait_receipts(2).await;
    gateway.terminate().assert_no_secrets();
}

// ---------------------------------------------------------------------------
// Disconnects and slow clients
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_disconnect_before_usage_cancels_upstream_and_records_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    upstream.queue(Reply::sse_steps(
        "",
        vec![Step::Write(delta_event("partial")), Step::Hold],
    ));
    let mut client = open_stream(&gateway, &fixture(TURN)).await;
    read_until(&mut client, b"partial").await;
    drop(client);
    let receipts = gateway.wait_receipts(1).await;
    assert_eq!(receipts[0].outcome, Outcome::Unknown);
    assert_counts(&receipts[0], None, None, None);
    assert_eq!(delivery_of(&receipts[0]), Some(Delivery::ClientClosed));
    let record = upstream.received()[0].clone();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !record.peer_closed.load(Ordering::SeqCst) {
        assert!(
            Instant::now() < deadline,
            "the upstream connection was not cancelled"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_disconnect_after_usage_keeps_the_counts() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    upstream.queue(Reply::sse_steps(
        "",
        vec![
            Step::Write(delta_event("partial")),
            Step::Write(final_chunk(Some(usage_object(1000, 400, Some(900))))),
            Step::Hold,
        ],
    ));
    let mut client = open_stream(&gateway, &fixture(TURN)).await;
    read_until(&mut client, b"prompt_tokens").await;
    drop(client);
    let receipts = gateway.wait_receipts(1).await;
    assert_eq!(
        receipts[0].outcome,
        Outcome::Complete,
        "terminal usage was observed"
    );
    assert_counts(&receipts[0], Some(1000), Some(900), Some(400));
    assert_eq!(delivery_of(&receipts[0]), Some(Delivery::ClientClosed));
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_slow_client_hits_the_idle_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            idle_ms: Some(400),
            ..Spawn::default()
        },
    );
    // 48 MiB of 64 KiB events: far past every kernel buffer and the 256 KiB
    // forwarding bound, so a client that never reads stalls the gateway.
    let event = {
        // A valid chunk (object with a choices array), just large.
        let mut event = b"data: ".to_vec();
        event.extend_from_slice(
            &serde_json::to_vec(&json!({
                "id": "chatcmpl-fill",
                "choices": [{"index": 0, "delta": {"content": "x".repeat(64 * 1024 - 200)}}],
            }))
            .unwrap(),
        );
        event.extend_from_slice(b"\n\n");
        event
    };
    let mut steps = Vec::new();
    for _ in 0..768 {
        steps.push(Step::Write(event.clone()));
    }
    upstream.queue(Reply::sse_steps("", steps));
    let client = open_stream(&gateway, &fixture(TURN)).await;
    let receipts = gateway.wait_receipts(1).await;
    assert_eq!(
        receipts[0].outcome,
        Outcome::Failed,
        "no usage was observed"
    );
    assert_eq!(delivery_of(&receipts[0]), Some(Delivery::LocalFailure));
    assert_counts(&receipts[0], None, None, None);
    drop(client);
    gateway.terminate().assert_no_secrets();
}

// ---------------------------------------------------------------------------
// Capacity
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_full_log_closes_admission_before_the_send_that_cannot_fit() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(65_536),
            ..Spawn::default()
        },
    );
    // 64 context ids make each receipt about 2.8 KiB, so the 64 KiB log
    // fills after a few dozen attempts.
    let ids: Vec<String> = (0..64).map(|_| uuid::Uuid::new_v4().to_string()).collect();
    let header = ids.join(",");
    let mut admitted = 0usize;
    loop {
        upstream.queue(Reply::sse("", simple_stream()));
        let response = gateway
            .request(reqwest::Method::POST, "/v1/chat/completions")
            .header("authorization", gateway.bearer())
            .header("content-type", "application/json")
            .header("x-foundry-context-ids", header.clone())
            .body(fixture(TURN))
            .send()
            .await
            .unwrap();
        if response.status() == 403 {
            assert_eq!(error_of(response).await.1, "gateway_admission_closed");
            break;
        }
        assert_eq!(response.status(), 200);
        assert!(read_all(response).await.1);
        admitted += 1;
        gateway.wait_receipts(admitted).await;
        assert!(admitted < 200, "a 64 KiB log must fill");
    }
    assert!(admitted > 1, "admission stayed open while receipts fit");
    assert_eq!(
        upstream.count(),
        admitted,
        "the refused request never reached upstream"
    );
    let health = gateway.health().await;
    assert_eq!(health["admission"], "closed");
    assert_eq!(health["attempts"], admitted);
    let size = std::fs::metadata(gateway.run_dir.join("receipts.jsonl"))
        .unwrap()
        .len();
    assert!(size <= 65_536, "the log never outgrows its cap: {size}");
    let receipts = gateway.receipts();
    assert_eq!(receipts.len(), admitted);
    assert_eq!(
        receipts[0].context_ids, ids,
        "context ids are recorded as supplied"
    );
    let finished = gateway.terminate();
    assert_eq!(finished.final_line()["attempts"], admitted);
    finished.assert_no_secrets();
}

// The 10,000-attempt bound (`session_full`) cannot be driven through the
// process without a test-only cap; its cap logic and status mapping are unit
// tested in `src/gateway.rs` (`the_ten_thousandth_attempt_closes_admission_as_session_full`).

// ---------------------------------------------------------------------------
// Shutdown and restart scope
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sigterm_during_an_active_stream_finishes_receipts_within_five_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    upstream.queue(Reply::sse(
        "",
        vec![
            delta_event("a"),
            final_chunk(Some(usage_object(10, 3, Some(4)))),
            done_event(),
        ],
    ));
    assert!(read_all(gateway.post(fixture(TURN)).await).await.1);
    gateway.wait_receipts(1).await;

    upstream.queue(Reply::sse_steps(
        "",
        vec![Step::Write(delta_event("mid-stream")), Step::Hold],
    ));
    let mut active = open_stream(&gateway, &fixture(TURN)).await;
    read_until(&mut active, b"mid-stream").await;
    let run_dir = gateway.run_dir.clone();
    let finished = gateway.terminate();

    assert!(
        finished.status.success(),
        "a signalled gateway exits cleanly"
    );
    assert!(
        finished.elapsed < Duration::from_secs(5),
        "exit within five seconds of the signal, took {:?}",
        finished.elapsed
    );
    let summary = finished.final_line();
    assert_eq!(summary["attempts"], 2);
    assert_eq!(summary["complete"], 1);
    assert_eq!(summary["failed"], 1, "the cut stream failed before usage");
    assert_eq!(summary["unknown"], 0);
    assert_eq!(summary["input_tokens"], 10);
    assert_eq!(summary["cached_input_tokens"], 4);
    assert_eq!(summary["output_tokens"], 3);
    assert_eq!(summary["coverage"], "whole_run_unverified");
    let receipts: Vec<Receipt> = finished
        .receipts_text
        .lines()
        .map(|line| Receipt::parse(line.as_bytes()).unwrap())
        .collect();
    assert_eq!(receipts.len(), 2, "receipts survive the shutdown");
    assert_eq!(receipts[1].outcome, Outcome::Failed);
    assert_eq!(delivery_of(&receipts[1]), Some(Delivery::LocalFailure));
    assert!(
        !run_dir.join("token").exists(),
        "the token is removed on exit"
    );
    assert!(!run_dir.join("gateway.json").exists());
    assert!(run_dir.join("receipts.jsonl").exists(), "receipts are kept");
    finished.assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_existing_run_dir_is_refused_and_a_fresh_run_has_a_new_identity() {
    let dir = tempfile::tempdir().unwrap();
    let mut first = Gateway::start(
        dir.path(),
        Spawn {
            log_bytes: Some(65_536),
            run_name: Some("run-a"),
            ..Spawn::default()
        },
    );
    let (session_a, token_a) = (first.session_id(), first.token.clone());
    let config_a = first.config.clone();
    let run_a = first.run_dir.clone();
    let finished = first.terminate();
    assert!(finished.status.success());
    std::fs::write(run_a.join("evidence.txt"), "keep").unwrap();
    let receipts_before = std::fs::read(run_a.join("receipts.jsonl")).unwrap();

    let mut command = Command::new(BIN);
    command
        .arg("gateway")
        .arg("--config")
        .arg(&config_a)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env(KEY_ENV, KEY_CANARY)
        .stdin(Stdio::null());
    let output = run_with_deadline(command, Duration::from_secs(10));
    assert!(!output.status.success(), "an existing run_dir refuses");
    assert_eq!(error_code(&output), "invalid_argument");
    assert_eq!(
        std::fs::read_to_string(run_a.join("evidence.txt")).unwrap(),
        "keep"
    );
    assert_eq!(
        std::fs::read(run_a.join("receipts.jsonl")).unwrap(),
        receipts_before,
        "nothing was cleared"
    );

    let mut second = Gateway::start(
        dir.path(),
        Spawn {
            run_name: Some("run-b"),
            ..Spawn::default()
        },
    );
    assert_ne!(
        second.session_id(),
        session_a,
        "a fresh run has a fresh session id"
    );
    assert_ne!(second.token, token_a, "and a fresh token");
    assert_eq!(second.token.len(), 64);
    assert!(
        second
            .token
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    );
    second.terminate().assert_no_secrets();
}

// ---------------------------------------------------------------------------
// Config refusals
// ---------------------------------------------------------------------------

fn base_config(run_dir: &Path) -> Value {
    json!({
        "v": 1,
        "port": 0,
        "upstream": PINNED,
        "model": "glm-5.3-flash",
        "mode": "meter",
        "credential_env": KEY_ENV,
        "run_dir": run_dir,
    })
}

fn run_refused(dir: &Path, name: &str, config: &Value, key: Option<&str>) -> std::process::Output {
    let path = dir.join(format!("{name}.json"));
    std::fs::write(&path, serde_json::to_vec(config).unwrap()).unwrap();
    let mut command = Command::new(BIN);
    command
        .arg("gateway")
        .arg("--config")
        .arg(&path)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null());
    if let Some(key) = key {
        command.env(KEY_ENV, key);
    }
    run_with_deadline(command, Duration::from_secs(10))
}

#[test]
fn config_refusals_name_their_codes_and_create_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let run_dir = dir.path().join("never-created");
    let unsupported = "gateway_feature_unsupported";
    let invalid = "invalid_argument";
    type Edit<'a> = (&'a str, Box<dyn Fn(&mut Value)>, &'a str);
    let edits: Vec<Edit> = vec![
        (
            "enforce",
            Box::new(|c: &mut Value| c["mode"] = json!("enforce")),
            unsupported,
        ),
        (
            "mode other",
            Box::new(|c: &mut Value| c["mode"] = json!("observe")),
            invalid,
        ),
        (
            "limits",
            Box::new(|c: &mut Value| c["limits"] = json!({"input_tokens": 1})),
            unsupported,
        ),
        (
            "unknown model",
            Box::new(|c: &mut Value| c["model"] = json!("glm-5.3")),
            unsupported,
        ),
        (
            "unknown upstream",
            Box::new(|c: &mut Value| c["upstream"] = json!("https://example.com/v1")),
            unsupported,
        ),
        (
            "log too small",
            Box::new(|c: &mut Value| c["log_bytes"] = json!(65_535)),
            invalid,
        ),
        (
            "log too large",
            Box::new(|c: &mut Value| c["log_bytes"] = json!(16 * 1024 * 1024 + 1)),
            invalid,
        ),
        (
            "log null",
            Box::new(|c: &mut Value| c["log_bytes"] = Value::Null),
            invalid,
        ),
        (
            "unknown key",
            Box::new(|c: &mut Value| c["extra"] = json!(1)),
            invalid,
        ),
        (
            "version",
            Box::new(|c: &mut Value| c["v"] = json!(2)),
            invalid,
        ),
        (
            "port type",
            Box::new(|c: &mut Value| c["port"] = json!("80")),
            invalid,
        ),
        (
            "port range",
            Box::new(|c: &mut Value| c["port"] = json!(65_536)),
            invalid,
        ),
        (
            "relative run_dir",
            Box::new(|c: &mut Value| c["run_dir"] = json!("relative/run")),
            invalid,
        ),
        (
            "bad env name",
            Box::new(|c: &mut Value| c["credential_env"] = json!("1-bad")),
            invalid,
        ),
    ];
    for (name, edit, code) in edits {
        let mut config = base_config(&run_dir);
        edit(&mut config);
        let output = run_refused(dir.path(), name, &config, Some(KEY_CANARY));
        assert!(!output.status.success(), "{name}");
        assert_eq!(error_code(&output), code, "{name}");
        assert!(
            !run_dir.exists(),
            "{name}: a refused config creates nothing"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains(KEY_CANARY),
            "{name}: the key never reaches stderr"
        );
    }
    for (name, key) in [
        ("missing credential", None),
        ("empty credential", Some("")),
        ("blank credential", Some("  ")),
    ] {
        let output = run_refused(dir.path(), name, &base_config(&run_dir), key);
        assert!(!output.status.success(), "{name}");
        assert_eq!(error_code(&output), "gateway_credential_missing", "{name}");
        assert!(!run_dir.exists(), "{name}");
    }
    // More than 64 KiB of config refuses.
    let mut oversized = base_config(&run_dir);
    oversized["padding"] = json!("x".repeat(70 * 1024));
    let output = run_refused(dir.path(), "oversized", &oversized, Some(KEY_CANARY));
    assert_eq!(error_code(&output), invalid);
}

// ---------------------------------------------------------------------------
// Launcher `gateway-omp`
// ---------------------------------------------------------------------------

const GLOBAL_MODELS: &str = "# the owner's global OMP models\nproviders: {}\n";

struct Launcher {
    home: tempfile::TempDir,
    work: tempfile::TempDir,
}

impl Launcher {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let agent = home.path().join(".omp/agent");
        std::fs::create_dir_all(&agent).unwrap();
        std::fs::write(agent.join("models.yml"), GLOBAL_MODELS).unwrap();
        let other = home.path().join(".omp/profiles/other/agent");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("models.yml"), "other: keep\n").unwrap();
        Self { home, work }
    }

    fn profile_dir(&self, name: &str) -> PathBuf {
        self.home.path().join(".omp/profiles").join(name)
    }

    fn run_dir(&self, name: &str) -> PathBuf {
        self.work.path().join(name)
    }

    fn config(&self, run_name: &str) -> PathBuf {
        write_config(self.work.path(), &self.run_dir(run_name), Some(65_536))
    }

    fn key_file(&self, content: &str) -> PathBuf {
        let path = self.work.path().join(format!("key-{}", content.len()));
        std::fs::write(&path, content).unwrap();
        path
    }

    /// A fake `omp` that records argv, env, the profile's models.yml and the
    /// gateway token, adds a session file and exits with `exit_code`.
    fn fake_omp(&self, out: &Path, run_name: &str, profile: &str, exit_code: i32) -> PathBuf {
        let path = self.work.path().join(format!("omp-{profile}"));
        let script = format!(
            "#!/bin/sh\nout='{out}'\nmkdir -p \"$out\"\nprintf '%s\\n' \"$@\" > \"$out/argv\"\n/usr/bin/env | /usr/bin/sort > \"$out/env\"\ncp \"$HOME/.omp/profiles/{profile}/agent/models.yml\" \"$out/models.yml\"\ncp '{run}/token' \"$out/token\"\nprintf 'session\\n' > \"$HOME/.omp/profiles/{profile}/agent/session.jsonl\"\nexit {exit_code}\n",
            out = out.display(),
            run = self.run_dir(run_name).display(),
        );
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn launch(&self, bin: &str, args: &[&str], fault: Option<&str>) -> std::process::Output {
        let mut command = Command::new(bin);
        command
            .arg("gateway-omp")
            .args(args)
            .env_clear()
            .env("HOME", self.home.path())
            .env("PATH", "/usr/bin:/bin")
            .env("FOUNDRY_TEST_SENTINEL", "must-not-reach-omp")
            .stdin(Stdio::null());
        if let Some(fault) = fault {
            command.env("FOUNDRY_TEST_FAULT", fault);
        }
        run_with_deadline(command, Duration::from_secs(60))
    }

    fn assert_global_state_untouched(&self) {
        assert_eq!(
            std::fs::read_to_string(self.home.path().join(".omp/agent/models.yml")).unwrap(),
            GLOBAL_MODELS,
            "the global models.yml is never touched"
        );
        assert_eq!(
            std::fs::read_to_string(self.profile_dir("other").join("agent/models.yml")).unwrap(),
            "other: keep\n",
            "other profiles are never touched"
        );
    }
}

fn env_map(text: &str) -> std::collections::HashMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
}

fn path_str(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn the_launcher_routes_omp_through_a_fresh_profile_and_cleans_up() {
    let launcher = Launcher::new();
    let config = launcher.config("run1");
    let key = launcher.key_file(&format!("{KEY_CANARY}\n"));
    let out = launcher.work.path().join("omp-out");
    let omp = launcher.fake_omp(&out, "run1", "demo", 0);
    let output = launcher.launch(
        BIN,
        &[
            "--config",
            path_str(&config),
            "--key-file",
            path_str(&key),
            "--profile",
            "demo",
            "--omp",
            path_str(&omp),
            "--",
            "extra-1",
            "--flag",
        ],
        None,
    );
    assert!(
        output.status.success(),
        "launcher failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "the launcher prints nothing on stdout"
    );

    let argv: Vec<String> = std::fs::read_to_string(out.join("argv"))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        argv,
        [
            "--profile",
            "demo",
            "--model",
            "zai/glm-5.3-flash",
            "--thinking",
            "max",
            "extra-1",
            "--flag",
        ],
        "an explicit thinking level, so OMP never runs auto"
    );
    let env_text = std::fs::read_to_string(out.join("env")).unwrap();
    let env = env_map(&env_text);
    let token = std::fs::read_to_string(out.join("token"))
        .unwrap()
        .trim()
        .to_owned();
    assert_eq!(token.len(), 64);
    assert_eq!(env.get("PI_NO_TITLE").map(String::as_str), Some("1"));
    assert_eq!(
        env.get("FOUNDRY_GATEWAY_TOKEN"),
        Some(&token),
        "OMP holds this run's token"
    );
    assert!(
        !env_text.contains(KEY_CANARY),
        "no upstream key reaches OMP"
    );
    assert!(!env.contains_key(KEY_ENV) && !env.contains_key("ZAI_API_KEY"));
    assert!(
        !env.contains_key("FOUNDRY_TEST_SENTINEL"),
        "OMP's environment is an allowlist"
    );
    assert!(
        !argv.iter().any(|arg| arg.contains("api-key")),
        "never --api-key"
    );

    // models.yml was exact while OMP ran, with this run's loopback port.
    let models = std::fs::read_to_string(out.join("models.yml")).unwrap();
    let port = models
        .split("127.0.0.1:")
        .nth(1)
        .and_then(|rest| rest.split("/v1").next())
        .unwrap();
    assert_eq!(
        models,
        format!(
            "providers:\n  zai:\n    baseUrl: \"http://127.0.0.1:{port}/v1\"\n    apiKey: FOUNDRY_GATEWAY_TOKEN\n"
        )
    );

    // After OMP: only the generated file is gone; evidence stays.
    let agent = launcher.profile_dir("demo").join("agent");
    assert!(
        !agent.join("models.yml").exists(),
        "the generated models.yml is removed"
    );
    assert_eq!(
        std::fs::read_to_string(agent.join("session.jsonl")).unwrap(),
        "session\n"
    );
    let run_dir = launcher.run_dir("run1");
    assert!(run_dir.join("receipts.jsonl").exists(), "receipts are kept");
    assert!(
        !run_dir.join("token").exists(),
        "the gateway removed its token"
    );
    assert!(!run_dir.join("gateway.json").exists());
    launcher.assert_global_state_untouched();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains(KEY_CANARY) && !stderr.contains(&token));
    assert!(
        stderr.contains("whole_run_unverified"),
        "the gateway's final summary is surfaced"
    );
}

#[test]
fn the_launcher_exits_with_omps_status() {
    let launcher = Launcher::new();
    let config = launcher.config("run7");
    let key = launcher.key_file(&format!("{KEY_CANARY}\n"));
    let out = launcher.work.path().join("omp-out7");
    let omp = launcher.fake_omp(&out, "run7", "seven", 7);
    let output = launcher.launch(
        BIN,
        &[
            "--config",
            path_str(&config),
            "--key-file",
            path_str(&key),
            "--profile",
            "seven",
            "--thinking",
            "high",
            "--omp",
            path_str(&omp),
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(7));
    let argv = std::fs::read_to_string(out.join("argv")).unwrap();
    assert!(argv.contains("--thinking\nhigh\n"));
    assert!(
        !launcher
            .profile_dir("seven")
            .join("agent/models.yml")
            .exists()
    );
}

#[test]
fn the_launcher_refuses_an_existing_profile_and_bad_names() {
    let launcher = Launcher::new();
    let config = launcher.config("never");
    let key = launcher.key_file(&format!("{KEY_CANARY}\n"));
    let out = launcher.work.path().join("omp-out-refused");
    let omp = launcher.fake_omp(&out, "never", "taken", 0);
    let taken = launcher.profile_dir("taken").join("agent");
    std::fs::create_dir_all(&taken).unwrap();
    std::fs::write(taken.join("models.yml"), "mine: true\n").unwrap();

    let attempt = |profile: &str| {
        // `--profile=NAME` keeps a dash-led name a value, so the launcher's
        // own validation (not clap) refuses it.
        let profile = format!("--profile={profile}");
        launcher.launch(
            BIN,
            &[
                "--config",
                path_str(&config),
                "--key-file",
                path_str(&key),
                &profile,
                "--omp",
                path_str(&omp),
            ],
            None,
        )
    };
    let output = attempt("taken");
    assert!(!output.status.success());
    assert_eq!(error_code(&output), "profile_exists");
    assert_eq!(
        std::fs::read_to_string(taken.join("models.yml")).unwrap(),
        "mine: true\n",
        "never overwritten"
    );

    // A dangling symlink is "any form" too.
    std::os::unix::fs::symlink("/nonexistent-target", launcher.profile_dir("dangling")).unwrap();
    assert_eq!(error_code(&attempt("dangling")), "profile_exists");

    let too_long = "a".repeat(65);
    for name in [
        "Bad_Name",
        "default",
        "..",
        "con",
        "nul.txt",
        "lpt0",
        "a.",
        "-x",
        too_long.as_str(),
    ] {
        let output = attempt(name);
        assert!(!output.status.success(), "{name}");
        assert_eq!(error_code(&output), "invalid_argument", "{name}");
    }
    assert!(!out.exists(), "OMP never ran");
    assert!(
        !launcher.run_dir("never").exists(),
        "no gateway was started"
    );
    launcher.assert_global_state_untouched();
}

#[test]
fn the_launcher_refuses_a_bad_key_file() {
    let launcher = Launcher::new();
    let config = launcher.config("never");
    let out = launcher.work.path().join("omp-out-key");
    let omp = launcher.fake_omp(&out, "never", "keyed", 0);
    let missing = launcher.work.path().join("no-such-key");
    let empty = launcher.key_file("");
    let multiline = launcher.key_file("line-one\nline-two\n");
    for (name, key) in [
        ("missing", &missing),
        ("empty", &empty),
        ("multiline", &multiline),
    ] {
        let output = launcher.launch(
            BIN,
            &[
                "--config",
                path_str(&config),
                "--key-file",
                path_str(key),
                "--profile",
                "keyed",
                "--omp",
                path_str(&omp),
            ],
            None,
        );
        assert!(!output.status.success(), "{name}");
        assert_eq!(error_code(&output), "invalid_argument", "{name}");
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("line-one"),
            "{name}: the key is never echoed"
        );
    }
    assert!(!launcher.run_dir("never").exists());
    assert!(!out.exists());
}

/// `gateway_cleaned_up`: false when the gateway was killed by the fault, so
/// it never got to remove its own token and run description.
fn assert_failed_launch_cleaned_up(
    launcher: &Launcher,
    output: &std::process::Output,
    code: &str,
    out: &Path,
    run_name: &str,
    profile: &str,
    gateway_cleaned_up: bool,
) {
    assert!(!output.status.success());
    assert_eq!(error_code(output), code);
    assert!(!out.exists(), "OMP never ran after a refused verification");
    assert!(
        !launcher.profile_dir(profile).exists(),
        "the generated profile is removed"
    );
    if gateway_cleaned_up {
        let run_dir = launcher.run_dir(run_name);
        assert!(
            !run_dir.join("token").exists(),
            "the gateway was stopped and removed its token"
        );
        assert!(!run_dir.join("gateway.json").exists());
    }
    launcher.assert_global_state_untouched();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains(KEY_CANARY), "no key in the refusal");
}

#[test]
fn the_launcher_refuses_a_corrupt_profile() {
    let launcher = Launcher::new();
    let config = launcher.config("corrupt");
    let key = launcher.key_file(&format!("{KEY_CANARY}\n"));
    let out = launcher.work.path().join("omp-out-corrupt");
    let omp = launcher.fake_omp(&out, "corrupt", "corrupt", 0);
    let output = launcher.launch(
        FAULTS_BIN,
        &[
            "--config",
            path_str(&config),
            "--key-file",
            path_str(&key),
            "--profile",
            "corrupt",
            "--omp",
            path_str(&omp),
        ],
        Some("ctxfoundry-fault/gateway.launch.after_models_write=fail"),
    );
    assert_failed_launch_cleaned_up(
        &launcher,
        &output,
        "profile_invalid",
        &out,
        "corrupt",
        "corrupt",
        true,
    );
}

#[test]
fn the_launcher_refuses_an_empty_token() {
    let launcher = Launcher::new();
    let config = launcher.config("emptytoken");
    let key = launcher.key_file(&format!("{KEY_CANARY}\n"));
    let out = launcher.work.path().join("omp-out-token");
    let omp = launcher.fake_omp(&out, "emptytoken", "emptytoken", 0);
    let output = launcher.launch(
        FAULTS_BIN,
        &[
            "--config",
            path_str(&config),
            "--key-file",
            path_str(&key),
            "--profile",
            "emptytoken",
            "--omp",
            path_str(&omp),
        ],
        Some("ctxfoundry-fault/gateway.launch.before_token_read=fail"),
    );
    assert_failed_launch_cleaned_up(
        &launcher,
        &output,
        "token_missing",
        &out,
        "emptytoken",
        "emptytoken",
        true,
    );
}

#[test]
fn the_launcher_refuses_a_gateway_that_is_down_before_health() {
    let launcher = Launcher::new();
    let config = launcher.config("down");
    let key = launcher.key_file(&format!("{KEY_CANARY}\n"));
    let out = launcher.work.path().join("omp-out-down");
    let omp = launcher.fake_omp(&out, "down", "down", 0);
    let output = launcher.launch(
        FAULTS_BIN,
        &[
            "--config",
            path_str(&config),
            "--key-file",
            path_str(&key),
            "--profile",
            "down",
            "--omp",
            path_str(&omp),
        ],
        Some("ctxfoundry-fault/gateway.launch.before_health=fail"),
    );
    assert_failed_launch_cleaned_up(
        &launcher,
        &output,
        "gateway_unavailable",
        &out,
        "down",
        "down",
        false,
    );
}

// ---------------------------------------------------------------------------
// The gateway-only receipt field
// ---------------------------------------------------------------------------

fn receipt_bytes(observation: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "v": 1,
        "session_id": "s",
        "request_id": "r",
        "adapter_id": "foundry-gateway",
        "model_id": "glm-5.3-flash",
        "outcome": "complete",
        "observation": observation,
    }))
    .unwrap()
}

#[test]
fn the_receipt_delivery_field_is_strict_and_optional() {
    for delivery in ["delivered", "client_closed", "local_failure"] {
        let parsed = Receipt::parse(&receipt_bytes(
            json!({"mode": "meter", "elapsed_ms": 5, "delivery": delivery}),
        ))
        .unwrap();
        assert_eq!(parsed.to_json()["observation"]["delivery"], delivery);
        let again =
            Receipt::parse(serde_json::to_string(&parsed.to_json()).unwrap().as_bytes()).unwrap();
        assert_eq!(again, parsed, "serialize then parse is lossless");
    }
    let without =
        Receipt::parse(&receipt_bytes(json!({"mode": "meter", "elapsed_ms": 5}))).unwrap();
    assert!(
        without.to_json()["observation"].get("delivery").is_none(),
        "receipts without the field stay valid and do not gain one"
    );
    for bad in [json!("bogus"), Value::Null, json!(1), json!(true)] {
        let parsed = Receipt::parse(&receipt_bytes(
            json!({"mode": "meter", "elapsed_ms": 5, "delivery": bad}),
        ));
        assert!(parsed.is_err(), "{bad} is not a delivery value");
    }
}

// ---------------------------------------------------------------------------
// Coexistence with a store owner
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_gateway_runs_beside_an_mcp_store_owner_without_opening_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("workspace");
    let store = dir.path().join("store");
    std::fs::create_dir_all(&root).unwrap();
    for index in 0..2 {
        std::fs::write(
            root.join(format!("mod_{index}.rs")),
            format!("pub fn parse_{index}(input: &str) -> Option<(&str, &str)> {{ input.split_once('=') }}\n"),
        )
        .unwrap();
    }
    let bootstrap = Command::new(BIN)
        .arg("--store")
        .arg(&store)
        .args(["bootstrap", "--root"])
        .arg(&root)
        .arg("--apply")
        .output()
        .unwrap();
    assert!(
        bootstrap.status.success(),
        "{}",
        String::from_utf8_lossy(&bootstrap.stderr)
    );

    let mut owner = tokio::process::Command::new(BIN);
    owner
        .arg("--store")
        .arg(&store)
        .arg("mcp")
        .arg("--root")
        .arg(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let client = ().serve(TokioChildProcess::new(owner).unwrap()).await.unwrap();
    let status = client
        .call_tool(CallToolRequestParams::new("status"))
        .await
        .unwrap();
    assert_eq!(status.is_error, Some(false), "the owner serves");

    let upstream = FakeUpstream::start().await;
    // The store is the same one the owner holds exclusively: a gateway that
    // opened it would fail with a busy store instead of starting.
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            store: Some(store.clone()),
            ..Spawn::default()
        },
    );
    upstream.queue(Reply::sse("", simple_stream()));
    let response = gateway.post(fixture(TURN)).await;
    assert_eq!(
        response.status(),
        200,
        "the gateway forwards beside the owner"
    );
    assert!(read_all(response).await.1);
    let during = client
        .call_tool(CallToolRequestParams::new("status"))
        .await
        .unwrap();
    assert_eq!(
        during.is_error,
        Some(false),
        "the owner still serves while the gateway runs"
    );
    gateway.wait_receipts(1).await;
    let finished = gateway.terminate();
    assert!(finished.status.success());
    finished.assert_no_secrets();
    client.cancel().await.unwrap();
}

// ---------------------------------------------------------------------------
// Review round: C1, F1, F2 and the M findings
// ---------------------------------------------------------------------------

/// A fake `omp` that records its inputs, then waits with a SIGINT trap so a
/// forwarded interrupt is observable.
fn fake_omp_waiting(launcher: &Launcher, out: &Path, run_name: &str, profile: &str) -> PathBuf {
    let path = launcher.work.path().join(format!("omp-wait-{profile}"));
    let script = format!(
        "#!/bin/sh\nout='{}'\nmkdir -p \"$out\"\nprintf '%s\\n' \"$@\" > \"$out/argv\"\n/usr/bin/env | /usr/bin/sort > \"$out/env\"\ncp '{}/token' \"$out/token\"\ntrap 'printf sigint > \"$out/signal\"; kill $pid 2>/dev/null; exit 130' INT\nsleep 30 &\npid=$!\nwait $pid\nexit 0\n",
        out.display(),
        launcher.run_dir(run_name).display(),
    );
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bom_or_unknown_shape_never_hides_an_upstream_error_or_secret() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    // The BOM leads the STREAM: the first complete event carries it, so a
    // BOM-prefixed error is the very first thing the observer sees.
    let bom_error = {
        let mut bytes = b"\xef\xbb\xbf".to_vec();
        bytes.extend_from_slice(
            data_event(&json!({"error": {"message": format!("bom {KEY_CANARY}"), "code": "9"}}))
                .as_slice(),
        );
        bytes
    };
    let after = delta_event("never-delivered");
    let first = delta_event("first");
    // A whole leading BOM before an error event: nothing is forwarded.
    upstream.queue(Reply::sse_steps(
        "",
        vec![Step::Write(bom_error.clone()), Step::Write(after.clone())],
    ));
    let (body, clean) = read_all(gateway.post(fixture(TURN)).await).await;
    assert!(body.is_empty(), "the error event is withheld: {body:?}");
    assert!(!clean);
    let receipts = gateway.wait_receipts(1).await;
    assert_eq!(
        receipts[0].outcome,
        Outcome::Failed,
        "whole-BOM error observed"
    );
    assert!(!String::from_utf8_lossy(&body).contains(KEY_CANARY));

    // The same leading BOM split across two reads behaves identically.
    upstream.queue(Reply::sse_steps(
        "",
        vec![
            Step::Write(bom_error[..2].to_vec()),
            Step::Sleep(40),
            Step::Write(bom_error[2..].to_vec()),
            Step::Write(after.clone()),
        ],
    ));
    let (body, _) = read_all(gateway.post(fixture(TURN)).await).await;
    assert!(
        body.is_empty(),
        "a split BOM hides nothing either: {body:?}"
    );
    gateway.wait_receipts(2).await;

    // Unknown shapes, a duplicate-key error hidden before `"error":null`,
    // and a valid data line followed by a BOM-prefixed error line in the
    // SAME event are each withheld and fail the stream.
    let payloads: Vec<Vec<u8>> = vec![
        b"data: hello\n\n".to_vec(),
        b"data: {\"no_choices\":true}\n\n".to_vec(),
        format!(
            "data: {{\"error\":{{\"message\":\"dup {KEY_CANARY}\"}},\"choices\":[],\"error\":null}}\n\n"
        )
        .into_bytes(),
        {
            let mut event = b"data: {\"choices\":[{\"delta\":{}}]}\n\xef\xbb\xbf".to_vec();
            event.extend_from_slice(
                format!("data: {{\"error\":{{\"message\":\"split {KEY_CANARY}\"}}}}\n\n")
                    .as_bytes(),
            );
            event
        },
    ];
    for (offset, payload) in payloads.iter().enumerate() {
        let expected_count = 3 + offset;
        upstream.queue(Reply::sse_steps(
            "",
            // Give the first frame a moment to reach the client before the
            // stream is cut, then tolerate either observable outcome: the
            // contract requires withholding the bad event, not that an
            // already-raced frame beat the abort.
            vec![
                Step::Write(first.clone()),
                Step::Sleep(60),
                Step::Write(payload.clone()),
            ],
        ));
        let (body, clean) = read_all(gateway.post(fixture(TURN)).await).await;
        assert_eq!(
            body,
            first,
            "the event is never forwarded: {}",
            String::from_utf8_lossy(payload)
        );
        assert!(!String::from_utf8_lossy(&body).contains(KEY_CANARY));
        assert!(!clean);
        let receipts = gateway.wait_receipts(expected_count).await;
        assert_eq!(receipts[expected_count - 1].outcome, Outcome::Failed);
    }

    // `error: null` beside `choices` is a normal chunk, forwarded verbatim,
    // and a BOM-prefixed valid stream forwards its original bytes.
    let mut bom_stream = b"\xef\xbb\xbf".to_vec();
    bom_stream.extend(delta_event("bom").clone());
    bom_stream.extend(final_chunk(Some(usage_object(8, 2, None))));
    bom_stream.extend(done_event());
    let expected = bom_stream.clone();
    let mut steps = vec![Step::Write(bom_stream[..4].to_vec()), Step::Sleep(30)];
    steps.push(Step::Write(bom_stream[4..].to_vec()));
    upstream.queue(Reply::sse_steps("", steps));
    let (body, clean) = read_all(gateway.post(fixture(TURN)).await).await;
    assert!(clean);
    assert_eq!(
        body, expected,
        "the BOM bytes are forwarded unchanged while still being observed"
    );
    let receipts = gateway.wait_receipts(3 + payloads.len()).await;
    let last = &receipts[2 + payloads.len()];
    assert_eq!(last.outcome, Outcome::Complete);
    assert_counts(last, Some(8), None, Some(2));
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_deadline_bounds_every_forwarding_wait_even_when_idle_is_long() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    // idle stays at the 60 s default: only the 350 ms deadline may cut this.
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            deadline_ms: Some(350),
            ..Spawn::default()
        },
    );
    let event = {
        let mut event = b"data: ".to_vec();
        event.extend_from_slice(
            &serde_json::to_vec(&json!({
                "id": "chatcmpl-fill",
                "choices": [{"index": 0, "delta": {"content": "x".repeat(64 * 1024 - 200)}}],
            }))
            .unwrap(),
        );
        event.extend_from_slice(b"\n\n");
        event
    };
    let mut steps = Vec::new();
    for _ in 0..768 {
        steps.push(Step::Write(event.clone()));
    }
    upstream.queue(Reply::sse_steps("", steps));
    let started = Instant::now();
    let client = open_stream(&gateway, &fixture(TURN)).await;
    let receipts = gateway.wait_receipts(1).await;
    let waited = started.elapsed();
    drop(client);
    assert!(
        waited < Duration::from_secs(3),
        "the receipt exists at the deadline, not after the idle window: {waited:?}"
    );
    assert_eq!(receipts[0].outcome, Outcome::Failed);
    assert_eq!(delivery_of(&receipts[0]), Some(Delivery::LocalFailure));
    assert!(
        receipts[0].observation.as_ref().unwrap().elapsed_ms < 3000,
        "elapsed_ms reflects the deadline cut"
    );
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_silent_upstream_before_headers_fails_within_the_idle_bound() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            idle_ms: Some(250),
            ..Spawn::default()
        },
    );
    // Reads the request, then never sends response headers.
    upstream.queue(Reply::raw(vec![Step::Hold]));
    let started = Instant::now();
    let (status, code) = error_of(gateway.post(fixture(TURN)).await).await;
    assert_eq!(status, 504);
    assert_eq!(code, "deadline_exceeded");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "the idle bound applies before response headers"
    );
    let receipts = gateway.wait_receipts(1).await;
    assert_eq!(receipts[0].outcome, Outcome::Failed);
    assert_eq!(delivery_of(&receipts[0]), Some(Delivery::LocalFailure));
    assert_eq!(upstream.count(), 1, "the stalled connection was cancelled");
    // The slot is free again.
    upstream.queue(Reply::sse("", simple_stream()));
    assert_eq!(gateway.post(fixture(TURN)).await.status(), 200);
    gateway.wait_receipts(2).await;
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stalled_clients_do_not_exhaust_the_connection_cap() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            idle_ms: Some(200),
            ..Spawn::default()
        },
    );
    let event = {
        let mut event = b"data: ".to_vec();
        event.extend_from_slice(
            &serde_json::to_vec(&json!({
                "id": "chatcmpl-fill",
                "choices": [{"index": 0, "delta": {"content": "x".repeat(64 * 1024 - 200)}}],
            }))
            .unwrap(),
        );
        event.extend_from_slice(b"\n\n");
        event
    };
    // Eight sequential non-reading clients: each fails locally and each
    // connection permit must come back, or the ninth connection hangs.
    for index in 0..8 {
        let mut steps = Vec::new();
        for _ in 0..768 {
            steps.push(Step::Write(event.clone()));
        }
        upstream.queue(Reply::sse_steps("", steps));
        let client = open_stream(&gateway, &fixture(TURN)).await;
        let receipts = gateway.wait_receipts(index + 1).await;
        assert_eq!(delivery_of(&receipts[index]), Some(Delivery::LocalFailure));
        drop(client);
    }
    let health = gateway.health().await;
    assert_eq!(health["ready"], true, "connection capacity is reclaimed");
    gateway.terminate().assert_no_secrets();
}

/// A COMPLETE stream (final chunk with usage, then `[DONE]`) to clients that
/// never read. Depending on kernel buffering the response is absorbed
/// (delivered), blocks the forwarding buffer first (failed locally) or has
/// its clean end go undrained (complete, then cut): in every regime the
/// connection permit must come back. The sizes sweep those regimes; the
/// undrained-clean-end branch itself is pinned deterministically by the
/// unit test `a_clean_end_the_host_never_drains_cuts_the_connection_and_keeps_usage`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn complete_streams_to_non_reading_clients_reclaim_their_connections() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            idle_ms: Some(200),
            ..Spawn::default()
        },
    );
    let chunk = {
        let mut event = b"data: ".to_vec();
        event.extend_from_slice(
            &serde_json::to_vec(&json!({
                "id": "chatcmpl-fill",
                "choices": [{"index": 0, "delta": {"content": "x".repeat(32 * 1024)}}],
            }))
            .unwrap(),
        );
        event.extend_from_slice(b"\n\n");
        event
    };
    let mut clients = Vec::new();
    for index in 0..8usize {
        // 128 KiB up to 1 MiB of deltas, then the final chunk and [DONE].
        let mut steps = Vec::new();
        for _ in 0..(4 * (index + 1)) {
            steps.push(Step::Write(chunk.clone()));
        }
        steps.push(Step::Write(final_chunk(Some(usage_object(10, 5, None)))));
        steps.push(Step::Write(done_event()));
        upstream.queue(Reply::sse_steps("", steps));
        clients.push(open_stream(&gateway, &fixture(TURN)).await);
        let receipts = gateway.wait_receipts(index + 1).await;
        let receipt = &receipts[index];
        assert!(
            matches!(
                delivery_of(receipt),
                Some(Delivery::Delivered | Delivery::LocalFailure)
            ),
            "client {index}: {:?}",
            delivery_of(receipt)
        );
        if receipt.outcome == Outcome::Complete {
            // Terminal usage already observed is kept whichever way the
            // delivery ended.
            assert_counts(receipt, Some(10), None, Some(5));
        }
    }
    // All eight client sockets are still open and unread: only reclaimed
    // connection permits let this authenticated health check through.
    let health = gateway.health().await;
    assert_eq!(health["ready"], true, "connection capacity is reclaimed");
    drop(clients);
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn admission_cannot_use_a_stale_capacity_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(65_536),
            ..Spawn::default()
        },
    );
    let ids: Vec<String> = (0..64).map(|_| uuid::Uuid::new_v4().to_string()).collect();
    let header = ids.join(",");
    let receipts_path = gateway.run_dir.join("receipts.jsonl");
    // Fill the log until exactly one more same-sized receipt closes it.
    let mut fillers = 0usize;
    loop {
        upstream.queue(Reply::sse("", simple_stream()));
        let response = gateway
            .request(reqwest::Method::POST, "/v1/chat/completions")
            .header("authorization", gateway.bearer())
            .header("content-type", "application/json")
            .header("x-foundry-context-ids", header.clone())
            .body(fixture(TURN))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert!(read_all(response).await.1);
        fillers += 1;
        gateway.wait_receipts(fillers).await;
        let written = std::fs::metadata(&receipts_path).unwrap().len();
        let last_line = std::fs::read_to_string(&receipts_path)
            .unwrap()
            .lines()
            .next_back()
            .unwrap()
            .len() as u64;
        if written + last_line > 65_536 - 16_385 {
            // One more receipt of this size cannot fit: A closes admission.
            break;
        }
        assert!(fillers < 100, "the log never reaches its close window");
    }
    assert_eq!(
        gateway.health().await["admission"],
        "open",
        "still open before A"
    );

    // A streams under the slot; its receipt is the one that closes
    // admission. While A holds the slot every concurrent B is busy; after A
    // finalizes, B's admission snapshot must be re-read under the slot.
    let gate = Arc::new(Notify::new());
    upstream.queue(Reply::sse_steps(
        "",
        vec![
            Step::Write(delta_event("a-stream")),
            Step::Gate(Arc::clone(&gate)),
            Step::Write(final_chunk(Some(usage_object(30, 3, None)))),
            Step::Write(done_event()),
        ],
    ));
    // A carries the same 64 context ids, so its receipt is as large as the
    // fillers' and is the one that closes the log.
    let ids_header = format!("X-Foundry-Context-Ids: {header}\r\n");
    let mut a = open_stream_with(&gateway, &fixture(TURN), &ids_header).await;
    read_until(&mut a, b"a-stream").await;

    // Reaching past the loop already proves the 403 was observed.
    let deadline = Instant::now() + Duration::from_secs(10);
    gate.notify_one();
    loop {
        assert!(
            Instant::now() < deadline,
            "B never observed the closed admission"
        );
        let response = gateway.post(fixture(TURN)).await;
        let status = response.status().as_u16();
        let body: Value = response.json().await.unwrap();
        assert_ne!(
            status, 200,
            "a request that reached a closed log is never sent"
        );
        if status == 403 {
            assert_eq!(body["error"]["code"], "gateway_admission_closed");
            break;
        }
        assert_eq!(status, 429, "busy while A holds the slot");
    }
    assert_eq!(gateway.health().await["admission"], "closed");
    assert!(
        gateway.health().await["attempts"] == (fillers as u64) + 1,
        "B was never recorded as an attempt"
    );
    drop(a);
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn provisional_usage_without_a_finish_is_not_terminal() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    // A delta that carries usage but no finish_reason, then a held stream.
    upstream.queue(Reply::sse_steps(
        "",
        vec![
            Step::Write(data_event(&json!({
                "id": "chatcmpl-1",
                "choices": [{"index": 0, "delta": {"content": "part"}}],
                "usage": usage_object(12, 1, None),
            }))),
            Step::Hold,
        ],
    ));
    let mut client = open_stream(&gateway, &fixture(TURN)).await;
    read_until(&mut client, b"prompt_tokens").await;
    drop(client);
    let receipts = gateway.wait_receipts(1).await;
    assert_eq!(
        receipts[0].outcome,
        Outcome::Unknown,
        "no finish was observed, so nothing is terminal"
    );
    assert_counts(&receipts[0], None, None, None);
    assert_eq!(delivery_of(&receipts[0]), Some(Delivery::ClientClosed));
    gateway.terminate().assert_no_secrets();
}

#[test]
fn a_directed_sigint_reaches_omp_even_when_stdin_is_a_tty() {
    use std::os::fd::FromRawFd as _;
    let launcher = Launcher::new();
    let config = launcher.config("sigint");
    let key = launcher.key_file(&format!("{KEY_CANARY}\n"));
    let out = launcher.work.path().join("omp-out-sigint");
    let omp = fake_omp_waiting(&launcher, &out, "sigint", "sigint");
    let mut master: libc::c_int = 0;
    let mut slave: libc::c_int = 0;
    // SAFETY: openpty(3) allocates a pseudo-terminal pair; success is checked.
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    let mut command = Command::new(BIN);
    command
        .arg("gateway-omp")
        .args([
            "--config",
            path_str(&config),
            "--key-file",
            path_str(&key),
            "--profile",
            "sigint",
            "--omp",
            path_str(&omp),
        ])
        .env_clear()
        .env("HOME", launcher.home.path())
        .env("PATH", "/usr/bin:/bin")
        // SAFETY: the raw fd is a fresh pty slave this test owns.
        .stdin(unsafe { Stdio::from_raw_fd(slave) })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    // The fake OMP records its inputs immediately, then waits.
    std::thread::sleep(Duration::from_millis(1500));
    assert!(
        out.join("argv").exists(),
        "OMP is running behind the gateway"
    );
    // A DIRECTED SIGINT: only the launcher is signalled. stdin is a tty, so
    // the old isatty heuristic would have swallowed it.
    // SAFETY: kill(2) on this test's own child.
    unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "the launcher did not exit after the forwarded interrupt"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    // SAFETY: conventional close(2) of the master side this test allocated.
    let _ = unsafe { libc::close(master) };
    assert_eq!(
        status.code(),
        Some(130),
        "OMP's 128+SIGINT status is relayed"
    );
    assert_eq!(
        std::fs::read_to_string(out.join("signal")).unwrap_or_default(),
        "sigint",
        "the directed interrupt was forwarded to OMP"
    );
    let run_dir = launcher.run_dir("sigint");
    assert!(!run_dir.join("token").exists(), "the gateway was stopped");
    assert!(!launcher.profile_dir("sigint").exists(), "cleanup ran");
}

#[test]
fn owned_and_credential_omp_arguments_refuse_before_anything_starts() {
    let launcher = Launcher::new();
    let config = launcher.config("never");
    let key = launcher.key_file(&format!("{KEY_CANARY}\n"));
    let out = launcher.work.path().join("omp-out-args");
    let omp = launcher.fake_omp(&out, "never", "argtest", 0);
    for refused in [
        vec!["--api-key", "secret"],
        vec!["--profile", "default"],
        vec!["--thinking", "auto"],
        vec!["--thinking=auto"],
        vec!["--model", "glm-5.3"],
        vec!["--models=other"],
        vec!["--provider", "openai"],
        vec!["--alias=x"],
        vec!["--config=/etc/other.yml"],
        vec!["--extension", "tools"],
        vec!["--extension=tools"],
        vec!["-e", "tools"],
        vec!["--external-thinking"],
        vec!["--service-tier=flex"],
        vec!["--hook", "/x/hook.ts"],
        vec!["--hook=/x/hook.ts"],
        vec!["--plugin-dir", "/x/plugins"],
        vec!["--plugin-dir=/x/plugins"],
        vec!["--smol", "other-model"],
        vec!["--smol=other-model"],
        vec!["--slow", "other-model"],
        vec!["--slow=other-model"],
        vec!["--plan"],
        vec!["--plan=other-model"],
        vec!["--prewalk"],
        vec!["--prewalk-into=other-model"],
        vec!["--plan-yolo"],
        vec!["--plan-yolo-into", "other-model"],
        vec!["--plan-yolo-into=other-model"],
    ] {
        let mut args: Vec<&str> = vec![
            "--config",
            path_str(&config),
            "--key-file",
            path_str(&key),
            "--profile",
            "argtest",
            "--omp",
            path_str(&omp),
        ];
        args.push("--");
        args.extend(refused.iter().copied());
        let output = launcher.launch(BIN, &args, None);
        assert!(!output.status.success(), "{refused:?}");
        assert_eq!(error_code(&output), "invalid_argument", "{refused:?}");
        assert!(
            !launcher.profile_dir("argtest").exists(),
            "{refused:?}: nothing was written"
        );
        assert!(
            !launcher.run_dir("never").exists(),
            "{refused:?}: no gateway was started"
        );
        assert!(!out.exists(), "{refused:?}: OMP never ran");
    }
    launcher.assert_global_state_untouched();
}

#[test]
fn a_failed_agent_creation_still_cleans_up_the_created_profile() {
    let launcher = Launcher::new();
    let config = launcher.config("partial1");
    let key = launcher.key_file(&format!("{KEY_CANARY}\n"));
    let out = launcher.work.path().join("omp-out-partial1");
    let omp = launcher.fake_omp(&out, "partial1", "partial1", 0);
    let output = launcher.launch(
        FAULTS_BIN,
        &[
            "--config",
            path_str(&config),
            "--key-file",
            path_str(&key),
            "--profile",
            "partial1",
            "--omp",
            path_str(&omp),
        ],
        Some("ctxfoundry-fault/gateway.launch.block_agent_dir=fail"),
    );
    assert_eq!(error_code(&output), "gateway_unavailable");
    assert!(
        !launcher.profile_dir("partial1").exists(),
        "the profile directory created before the failure is removed"
    );
    assert!(!out.exists(), "OMP never ran");
    assert!(!launcher.run_dir("partial1").join("token").exists());
    launcher.assert_global_state_untouched();
}

#[test]
fn a_failed_models_write_still_cleans_up_and_does_not_block_the_next_launch() {
    let launcher = Launcher::new();
    let config = launcher.config("partial2");
    let key = launcher.key_file(&format!("{KEY_CANARY}\n"));
    let out = launcher.work.path().join("omp-out-partial2");
    let omp = launcher.fake_omp(&out, "partial2", "partial2", 0);
    let failed = launcher.launch(
        FAULTS_BIN,
        &[
            "--config",
            path_str(&config),
            "--key-file",
            path_str(&key),
            "--profile",
            "partial2",
            "--omp",
            path_str(&omp),
        ],
        Some("ctxfoundry-fault/gateway.launch.after_models_create=fail"),
    );
    assert_eq!(error_code(&failed), "gateway_unavailable");
    assert!(
        !launcher.profile_dir("partial2").exists(),
        "the created-but-unwritten models.yml and its directories are removed"
    );
    assert!(!out.exists(), "OMP never ran");
    launcher.assert_global_state_untouched();

    // The cleanup was complete: the same profile name launches cleanly now
    // (a fresh run_dir: the old run's receipts stay by design).
    let config_b = launcher.config("partial2b");
    let output = launcher.launch(
        BIN,
        &[
            "--config",
            path_str(&config_b),
            "--key-file",
            path_str(&key),
            "--profile",
            "partial2",
            "--omp",
            path_str(&omp),
        ],
        None,
    );
    assert!(
        output.status.success(),
        "a relaunch is never blocked by leftover artifacts: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out.join("argv").exists(), "OMP ran on the retry");
    assert!(
        !launcher
            .profile_dir("partial2")
            .join("agent/models.yml")
            .exists()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn local_refusals_are_counted_by_code_in_health_and_the_summary() {
    let dir = tempfile::tempdir().unwrap();
    let upstream = FakeUpstream::start().await;
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            log_bytes: Some(1 << 20),
            ..Spawn::default()
        },
    );
    for _ in 0..2 {
        let response = gateway
            .request(reqwest::Method::POST, "/v1/chat/completions")
            .header("authorization", "Bearer wrong")
            .header("content-type", "application/json")
            .body(fixture(TURN))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
    }
    let response = gateway
        .request(reqwest::Method::POST, "/v1/chat/completions?x=1")
        .header("authorization", gateway.bearer())
        .header("content-type", "application/json")
        .body(fixture(TURN))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    let response = gateway
        .request(reqwest::Method::POST, "/v1/nope")
        .header("authorization", gateway.bearer())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    let health = gateway.health().await;
    assert_eq!(health["refused"]["gateway_unauthorized"], 2);
    assert_eq!(health["refused"]["invalid_argument"], 1);
    assert_eq!(health["refused"]["gateway_feature_unsupported"], 1);
    assert_eq!(health["attempts"], 0, "refusals are not attempts");
    assert_eq!(upstream.count(), 0);

    let finished = gateway.terminate();
    let summary = finished.final_line();
    assert_eq!(summary["refused"]["gateway_unauthorized"], 2);
    assert_eq!(summary["refused"]["invalid_argument"], 1);
    assert_eq!(summary["refused"]["gateway_feature_unsupported"], 1);
    assert_eq!(summary["attempts"], 0);
    finished.assert_no_secrets();
}
