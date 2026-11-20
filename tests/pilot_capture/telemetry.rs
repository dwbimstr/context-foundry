//! Independent bounded OTLP collector for the pinned native SDK controls.
//! Hyper consumes content-length or chunked framing; no payload/header is retained.
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::thread::JoinHandle;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{Request, Response, body::Incoming};
use hyper_util::rt::TokioIo;
use serde_json::{Value, json};

pub(super) struct Collector {
    pub addr: SocketAddr,
    requests: Arc<Mutex<Vec<Value>>>,
    connections: Arc<AtomicUsize>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Collector {
    pub fn new() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let (rows, accepted) = (requests.clone(), connections.clone());
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                loop {
                    tokio::select! {
                        _ = &mut rx => break,
                        next = listener.accept() => {
                            let (tcp, _) = next.unwrap();
                            assert!(accepted.fetch_add(1, Ordering::SeqCst) < 32, "OTLP connection cap");
                            let rows = rows.clone();
                            tokio::spawn(async move {
                                let service = hyper::service::service_fn(move |req| answer(req, rows.clone()));
                                let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(tcp), service).await;
                            });
                        }
                    }
                }
            });
        });
        Self {
            addr,
            requests,
            connections,
            shutdown: Some(tx),
            thread: Some(thread),
        }
    }

    pub fn finish(&mut self) -> Value {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        self.thread.take().unwrap().join().unwrap();
        assert!(self.connections.load(Ordering::SeqCst) < 32);
        assert!(self.requests.lock().unwrap().len() < 32);
        json!({"connections":self.connections.load(Ordering::SeqCst),"requests":self.requests.lock().unwrap().clone()})
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

async fn answer(
    req: Request<Incoming>,
    rows: Arc<Mutex<Vec<Value>>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let method = req.method().to_string();
    let path = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or("")
        .to_owned();
    let chunked = req
        .headers()
        .get("transfer-encoding")
        .is_some_and(|v| v == "chunked");
    let marker: Option<&[u8]> = match path.as_str() {
        "/v1/traces" => Some(b"cf-owned-native-registration"),
        "/v1/logs" => Some(b"cf-owned-native-log"),
        "/v1/metrics" => Some(b"cf_owned_native_metric"),
        _ => None,
    };
    let body = tokio::time::timeout(
        Duration::from_secs(2),
        Limited::new(req.into_body(), 1024 * 1024).collect(),
    )
    .await;
    let mut row = json!({"method":method,"path":path,"chunked":chunked,"accepted":false});
    if method == "POST"
        && let (Some(marker), Ok(Ok(body))) = (marker, body)
    {
        let body = body.to_bytes();
        row["accepted"] = json!(true);
        row["bytes"] = json!(body.len());
        row["sha256"] = json!(context_foundry::pilot_capture::sha256_hex(&body));
        row["marker"] = json!(body.windows(marker.len()).any(|part| part == marker));
    }
    let status = if row["accepted"] == true { 200 } else { 400 };
    let mut rows = rows.lock().unwrap();
    assert!(rows.len() < 32, "OTLP request cap");
    rows.push(row);
    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/x-protobuf")
        .body(Full::new(Bytes::new()))
        .unwrap())
}

pub(super) fn configuration(config: &[u8], kind: &str) -> Vec<u8> {
    let mut config = std::str::from_utf8(config).unwrap().to_owned();
    let flag = match kind {
        "telemetry-enabled" => Some(true),
        "telemetry-disabled" => Some(false),
        "telemetry-owned" => None,
        _ => panic!("unknown telemetry control"),
    };
    if let Some(flag) = flag {
        let disabled = "\ntelemetry:\n  otlpExportEnabled: false\n";
        let enabled = "\ntelemetry:\n  otlpExportEnabled: true\n";
        if config.contains(disabled) {
            config = config.replace(disabled, if flag { enabled } else { disabled });
        } else {
            assert!(!config.contains("telemetry:"));
            config.push_str(if flag { enabled } else { disabled });
        }
    }
    config.into_bytes()
}

pub(super) fn preload() -> Vec<u8> {
    let mut source =
        std::str::from_utf8(context_foundry::pilot_capture::host::QUALIFICATION_STARTUP_PROBE)
            .unwrap()
            .to_owned();
    let capture = "if (typeof prototype.addRequest !== \"function\") fail();";
    assert_eq!(source.matches(capture).count(), 1);
    source = source.replace(
        capture,
        &format!("{capture}\n const originalAddRequest = prototype.addRequest;"),
    );
    let refusal = "\t\t\tif (typeof request.destroy !== \"function\" || typeof request.emit !== \"function\") fail();";
    assert_eq!(source.matches(refusal).count(), 1);
    source = source.replace(refusal, &format!("\t\t\tif (scheme === \"http:\" && `${{host}}${{port}}` === process.env.CF_OWNED_OTLP_AUTHORITY && request.method === \"POST\" && [\"/v1/traces\", \"/v1/logs\", \"/v1/metrics\"].includes(request.path)) return Reflect.apply(originalAddRequest, this, [request, options]);\n{refusal}"));
    source.into_bytes()
}
