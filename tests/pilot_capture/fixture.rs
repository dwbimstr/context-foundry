//! Owned, scripted, numeric-loopback HTTPS fixture for pilot-capture
//! verification. A throwaway CA and an `api.anthropic.com` leaf are minted
//! per fixture with the system `openssl` into a private temporary directory;
//! nothing is committed. Requests are recorded (method, path, headers, body)
//! and answered from a script in order. Any unscripted request gets a 500 and
//! is recorded, so a test can prove that no retry or extra request happened.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, StreamBody, combinators::BoxBody};
use hyper::body::{Frame, Incoming};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls;

/// One scripted answer.
#[derive(Clone, Debug)]
pub enum Reply {
    /// 200 application/json count result.
    Count(u64),
    /// Arbitrary status, headers and body (for count or messages failures).
    Status(u16, Vec<(String, String)>, Vec<u8>),
    /// 200 text/event-stream body delivered as these chunks, with a short
    /// pause between chunks.
    Sse(Vec<Vec<u8>>),
}

#[derive(Clone, Debug)]
pub struct Seen {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

pub struct Fixture {
    pub addr: SocketAddr,
    pub ca_pem: Vec<u8>,
    seen: Arc<Mutex<Vec<Seen>>>,
    script: Arc<Mutex<VecDeque<Reply>>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    _dir: tempfile::TempDir,
}

fn openssl(dir: &Path, args: &[&str]) {
    let out = Command::new("/usr/bin/openssl")
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()
        .expect("system openssl is required to mint the fixture CA");
    assert!(out.status.success(), "openssl {args:?} failed");
}

/// Named-curve P-256 keys (LibreSSL's `-pkeyopt ec_paramgen_curve` writes
/// explicit curve parameters, which rustls rejects), PKCS#8-encoded.
fn mint(dir: &Path) {
    for name in ["ca", "leaf"] {
        let sec1 = format!("{name}.sec1");
        let key = format!("{name}.key");
        openssl(
            dir,
            &[
                "ecparam",
                "-name",
                "prime256v1",
                "-genkey",
                "-noout",
                "-out",
                &sec1,
            ],
        );
        openssl(
            dir,
            &["pkcs8", "-topk8", "-nocrypt", "-in", &sec1, "-out", &key],
        );
    }
    openssl(
        dir,
        &[
            "req",
            "-new",
            "-x509",
            "-sha256",
            "-key",
            "ca.key",
            "-out",
            "ca.pem",
            "-days",
            "2",
            "-subj",
            "/CN=cf-pilot-verification-ca",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
            "-addext",
            "keyUsage=critical,keyCertSign",
        ],
    );
    openssl(
        dir,
        &[
            "req",
            "-new",
            "-sha256",
            "-key",
            "leaf.key",
            "-out",
            "leaf.csr",
            "-subj",
            "/CN=api.anthropic.com",
        ],
    );
    std::fs::write(
        dir.join("ext.cnf"),
        "subjectAltName=DNS:api.anthropic.com\nbasicConstraints=CA:FALSE\nextendedKeyUsage=serverAuth\nkeyUsage=digitalSignature\n",
    )
    .unwrap();
    openssl(
        dir,
        &[
            "x509",
            "-req",
            "-sha256",
            "-in",
            "leaf.csr",
            "-CA",
            "ca.pem",
            "-CAkey",
            "ca.key",
            "-CAcreateserial",
            "-out",
            "leaf.pem",
            "-days",
            "2",
            "-extfile",
            "ext.cnf",
        ],
    );
}

type Body = BoxBody<Bytes, Infallible>;

fn full(status: u16, headers: Vec<(String, String)>, body: Vec<u8>) -> Response<Body> {
    let mut b = Response::builder().status(status);
    for (k, v) in headers {
        b = b.header(k, v);
    }
    b.body(Full::new(Bytes::from(body)).boxed()).unwrap()
}

impl Fixture {
    pub fn start(script: Vec<Reply>) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        mint(dir.path());
        let ca_pem = std::fs::read(dir.path().join("ca.pem")).unwrap();
        let chain: Vec<CertificateDer<'static>> =
            CertificateDer::pem_file_iter(dir.path().join("leaf.pem"))
                .unwrap()
                .map(|c| c.unwrap())
                .collect();
        let key = PrivateKeyDer::from_pem_file(dir.path().join("leaf.key")).unwrap();
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(chain, key)
        .unwrap();
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        std_listener.set_nonblocking(true).unwrap();
        let addr = std_listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let script = Arc::new(Mutex::new(VecDeque::from(script)));
        let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();
        let (seen2, script2) = (seen.clone(), script.clone());
        let thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move {
                let listener = tokio::net::TcpListener::from_std(std_listener).unwrap();
                loop {
                    tokio::select! {
                        _ = &mut rx => break,
                        accepted = listener.accept() => {
                            let Ok((tcp, _)) = accepted else { continue };
                            let acceptor = acceptor.clone();
                            let (seen, script) = (seen2.clone(), script2.clone());
                            tokio::spawn(async move {
                                let Ok(tls) = acceptor.accept(tcp).await else { return };
                                let service = hyper::service::service_fn(move |req: Request<Incoming>| {
                                    let (seen, script) = (seen.clone(), script.clone());
                                    async move { Ok::<_, Infallible>(answer(req, seen, script).await) }
                                });
                                let _ = hyper::server::conn::http1::Builder::new()
                                    .serve_connection(TokioIo::new(tls), service)
                                    .await;
                            });
                        }
                    }
                }
            });
        });
        Fixture {
            addr,
            ca_pem,
            seen,
            script,
            shutdown: Some(tx),
            thread: Some(thread),
            _dir: dir,
        }
    }

    pub fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }

    pub fn paths(&self) -> Vec<String> {
        self.seen().into_iter().map(|s| s.path).collect()
    }

    pub fn remaining_script(&self) -> usize {
        self.script.lock().unwrap().len()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

async fn answer(
    req: Request<Incoming>,
    seen: Arc<Mutex<Vec<Seen>>>,
    script: Arc<Mutex<VecDeque<Reply>>>,
) -> Response<Body> {
    let method = req.method().to_string();
    let path = req
        .uri()
        .path_and_query()
        .map(|p| p.to_string())
        .unwrap_or_default();
    let headers = req
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_string(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect();
    let body = req
        .into_body()
        .collect()
        .await
        .map(|b| b.to_bytes().to_vec())
        .unwrap_or_default();
    seen.lock().unwrap().push(Seen {
        method,
        path,
        headers,
        body,
    });
    let next = script.lock().unwrap().pop_front();
    match next {
        Some(Reply::Count(n)) => full(
            200,
            vec![
                ("content-type".into(), "application/json".into()),
                ("request-id".into(), format!("req_count_{n}")),
            ],
            format!("{{\"input_tokens\":{n}}}").into_bytes(),
        ),
        Some(Reply::Status(status, headers, body)) => full(status, headers, body),
        Some(Reply::Sse(chunks)) => {
            let stream = futures_util::stream::unfold(chunks.into_iter(), |mut it| async move {
                let chunk = it.next()?;
                tokio::time::sleep(Duration::from_millis(15)).await;
                Some((Ok::<_, Infallible>(Frame::data(Bytes::from(chunk))), it))
            });
            Response::builder()
                .status(200)
                .header("content-type", "text/event-stream; charset=utf-8")
                .header("request-id", "req_fixture_messages")
                .body(BodyExt::boxed(StreamBody::new(stream)))
                .unwrap()
        }
        None => full(
            500,
            vec![("content-type".into(), "application/json".into())],
            b"{}".to_vec(),
        ),
    }
}

// ---------------------------------------------------------------------------
// SSE script builders (shapes the pinned host's own Anthropic client accepts).
// ---------------------------------------------------------------------------

pub fn event(name: &str, data: &serde_json::Value) -> Vec<u8> {
    format!("event: {name}\ndata: {data}\n\n").into_bytes()
}

pub fn message_start(id: &str, input: u64, cache_write: u64, cache_read: u64) -> Vec<u8> {
    event(
        "message_start",
        &serde_json::json!({"type":"message_start","message":{"id":id,"type":"message","role":"assistant",
            "model":"claude-sonnet-5-5","content":[],"stop_reason":null,"stop_sequence":null,
            "usage":{"input_tokens":input,"cache_creation_input_tokens":cache_write,
                     "cache_read_input_tokens":cache_read,"output_tokens":1,"service_tier":"standard"}}}),
    )
}

pub fn text_block(index: u64, text: &str) -> Vec<u8> {
    let mut out = event(
        "content_block_start",
        &serde_json::json!({"type":"content_block_start","index":index,"content_block":{"type":"text","text":""}}),
    );
    out.extend(event(
        "content_block_delta",
        &serde_json::json!({"type":"content_block_delta","index":index,"delta":{"type":"text_delta","text":text}}),
    ));
    out.extend(event(
        "content_block_stop",
        &serde_json::json!({"type":"content_block_stop","index":index}),
    ));
    out
}

pub fn tool_block(index: u64, id: &str, name: &str, input: &serde_json::Value) -> Vec<u8> {
    let mut out = event(
        "content_block_start",
        &serde_json::json!({"type":"content_block_start","index":index,
            "content_block":{"type":"tool_use","id":id,"name":name,"input":{}}}),
    );
    out.extend(event(
        "content_block_delta",
        &serde_json::json!({"type":"content_block_delta","index":index,
            "delta":{"type":"input_json_delta","partial_json":input.to_string()}}),
    ));
    out.extend(event(
        "content_block_stop",
        &serde_json::json!({"type":"content_block_stop","index":index}),
    ));
    out
}

pub fn message_end(
    stop_reason: &str,
    input: u64,
    cache_write: u64,
    cache_read: u64,
    output: u64,
) -> Vec<u8> {
    let mut out = event(
        "message_delta",
        &serde_json::json!({"type":"message_delta","delta":{"stop_reason":stop_reason,"stop_sequence":null},
            "usage":{"input_tokens":input,"cache_creation_input_tokens":cache_write,
                     "cache_read_input_tokens":cache_read,"output_tokens":output}}),
    );
    out.extend(event(
        "message_stop",
        &serde_json::json!({"type":"message_stop"}),
    ));
    out
}

/// A complete successful text reply as separate chunks.
pub fn text_reply(id: &str, text: &str) -> Reply {
    Reply::Sse(vec![
        message_start(id, 1200, 40, 0),
        text_block(0, text),
        message_end("end_turn", 1200, 40, 0, 12),
    ])
}

/// A complete tool_use reply.
pub fn tool_reply(id: &str, tool_id: &str, name: &str, input: serde_json::Value) -> Reply {
    Reply::Sse(vec![
        message_start(id, 1300, 0, 40),
        tool_block(0, tool_id, name, &input),
        message_end("tool_use", 1300, 0, 40, 30),
    ])
}
