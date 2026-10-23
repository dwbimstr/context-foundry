//! The two transports: stdio (bounded decode, pre-spawn admission) and HTTP
//! (bearer auth, the pre-dispatch admission gate, the live-session cap), with
//! their tests. Server options, owner startup and the engine slot stay in
//! the parent module.
use super::*;

// ---------------------------------------------------------------------------
// stdio transport: bounded decode + pre-spawn admission
// ---------------------------------------------------------------------------

// Admission rule shared by both transports: a request takes one of 16
// permits BEFORE the SDK sees it (so a refused request never gets a handler
// task), and the permit rides inside the request's `Extensions` as an
// `Arc<AdmissionGuard>`. The SDK moves those extensions into the handler's
// `RequestContext`, so the permit is released only when the handler really
// finishes (and detached engine work holds its own clone). It is NOT tied to
// the outbound response: the SDK drops responses of cancelled requests
// before the transport sink.

pub async fn serve_stdio(options: ServerOptions) -> AResult<()> {
    serve_streams(options, tokio::io::stdin(), tokio::io::stdout()).await
}

/// The stdio transport over one inbound and one outbound byte stream: the
/// process's stdin/stdout in production, an in-process pipe in tests. EOF on
/// `input` ends the session exactly as stdin EOF does.
pub async fn serve_streams<R, W>(options: ServerOptions, input: R, output: W) -> AResult<()>
where
    R: tokio::io::AsyncRead + Send + Unpin + 'static,
    W: tokio::io::AsyncWrite + Send + Unpin + 'static,
{
    // The delivery-only capability is validated before any store is opened.
    options.budget.require_delivery()?;
    // 007 launch-time admission (refused before serving) and the one-time
    // open of every root.
    let state = open_owner(options, CancellationToken::new())?;
    let server = FoundryMcp::new(Arc::clone(&state));

    let permits = Arc::new(tokio::sync::Semaphore::new(MAX_HANDLER_ADMISSION));
    let eof_state = Arc::clone(&state);
    let read = FramedRead::new(
        input,
        JsonRpcMessageCodec::<RxJsonRpcMessage<RoleServer>>::new_with_max_length(MAX_INBOUND_BYTES),
    )
    .scan(permits, |permits, item| {
        ready(admit_stdio_item(permits, item))
    })
    .chain(
        futures_util::stream::once(async move {
            // Inbound EOF, refused frame or admission close: stop admission
            // and request cancellation NOW, before the SDK's response-drain
            // interval lets further batches commit.
            eof_state.cancel_active();
        })
        .filter_map(|()| ready(None::<RxJsonRpcMessage<RoleServer>>)),
    )
    // The SDK's sink/stream adapter needs an `Unpin` stream.
    .boxed();
    // The encoder side carries no size bound: only INCOMING frames are
    // limited to 64 KiB; outputs follow the 256 KiB result cap.
    let write = FramedWrite::new(
        output,
        JsonRpcMessageCodec::<TxJsonRpcMessage<RoleServer>>::default(),
    );

    let running = match server.serve((write, read)).await {
        Ok(running) => running,
        // EOF/closed before initialization is a clean exit, not a failure;
        // the resident worker is still stopped before the store is released.
        Err(rmcp::service::ServerInitializeError::ConnectionClosed(_)) => {
            state.shut_down().await;
            return Ok(());
        }
        Err(e) => {
            state.shut_down().await;
            return Err(FoundryError::Internal(e.into()).into());
        }
    };
    // Serve until the transport closes (EOF/disconnect). Only THEN stop
    // admission, request cancellation of anything still running, wait for
    // the current engine transaction (bounded above the maximum index
    // timeout), for the preparation driver to record its stop, and for the
    // resident worker to be stopped and reaped, before exiting.
    let quit = running.waiting().await;
    state.shut_down().await;
    quit.map_err(|e| FoundryError::Internal(e.into()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// HTTP transport: bearer auth on every method, pre-dispatch admission,
// 16 live sessions, IPv4 loopback only, Host/Origin rejection
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum CappedSessionManagerError {
    Inner(LocalSessionManagerError),
    TooManySessions,
}

impl std::fmt::Display for CappedSessionManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inner(e) => write!(f, "session manager error: {e}"),
            Self::TooManySessions => write!(f, "too many live sessions"),
        }
    }
}

impl std::error::Error for CappedSessionManagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inner(e) => Some(e),
            Self::TooManySessions => None,
        }
    }
}

impl From<LocalSessionManagerError> for CappedSessionManagerError {
    fn from(e: LocalSessionManagerError) -> Self {
        Self::Inner(e)
    }
}

/// `LocalSessionManager` wrapper enforcing the live-session cap exactly:
/// creation is serialized and refuses at `MAX_LIVE_SESSIONS` before any
/// session worker is allocated. Session expiry and DELETE release the
/// connection-local allowance with the session.
struct CappedSessionManager {
    inner: LocalSessionManager,
    create_lock: tokio::sync::Mutex<()>,
    shared: Arc<Shared>,
}

impl SessionManager for CappedSessionManager {
    type Error = CappedSessionManagerError;
    type Transport = <LocalSessionManager as SessionManager>::Transport;

    async fn create_session(&self) -> Result<(SessionId, Self::Transport), Self::Error> {
        let _guard = self.create_lock.lock().await;
        if self.inner.sessions.read().await.len() >= MAX_LIVE_SESSIONS {
            return Err(CappedSessionManagerError::TooManySessions);
        }
        self.inner.create_session().await.map_err(Into::into)
    }

    async fn initialize_session(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> Result<ServerJsonRpcMessage, Self::Error> {
        self.inner
            .initialize_session(id, message)
            .await
            .map_err(Into::into)
    }

    async fn has_session(&self, id: &SessionId) -> Result<bool, Self::Error> {
        self.inner.has_session(id).await.map_err(Into::into)
    }

    async fn close_session(&self, id: &SessionId) -> Result<(), Self::Error> {
        self.shared.drop_session(id.as_ref());
        self.inner.close_session(id).await.map_err(Into::into)
    }

    async fn create_stream(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> Result<impl futures_util::Stream<Item = ServerSseMessage> + Send + 'static, Self::Error>
    {
        self.inner
            .create_stream(id, message)
            .await
            .map_err(Into::into)
    }

    async fn accept_message(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> Result<(), Self::Error> {
        self.inner
            .accept_message(id, message)
            .await
            .map_err(Into::into)
    }

    async fn create_standalone_stream(
        &self,
        id: &SessionId,
    ) -> Result<impl futures_util::Stream<Item = ServerSseMessage> + Send + 'static, Self::Error>
    {
        self.inner
            .create_standalone_stream(id)
            .await
            .map_err(Into::into)
    }

    async fn resume(
        &self,
        id: &SessionId,
        last_event_id: String,
    ) -> Result<impl futures_util::Stream<Item = ServerSseMessage> + Send + 'static, Self::Error>
    {
        self.inner
            .resume(id, last_event_id)
            .await
            .map_err(Into::into)
    }
}

pub struct HttpOptions {
    /// IPv4 loopback port; 0 selects a free local port.
    pub port: u16,
    /// Environment variable NAME carrying the bearer secret. Read before the
    /// store is opened; never printed.
    pub token_env: String,
    /// Idle-session expiry (pinned SDK default 300 s; shorter for tests).
    pub keep_alive: Duration,
    /// Owner shutdown signal.
    pub shutdown: CancellationToken,
}

fn bearer_matches(header: Option<&http::HeaderValue>, expected: &str) -> bool {
    let Some(value) = header.and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let Some(token) = value.strip_prefix("Bearer ") else {
        return false;
    };
    // Constant-time-ish comparison; this bounds accidental use, not a
    // hostile same-user process (per the gateway threat note).
    let a = token.as_bytes();
    let b = expected.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[derive(Clone)]
struct GateState {
    secret: Arc<String>,
    permits: Arc<tokio::sync::Semaphore>,
    /// The exact `Host` header value the bound listener must be addressed
    /// by: `127.0.0.1:PORT`, compared byte for byte (no port normalization).
    host: Arc<str>,
}

/// Newest protocol the adapter negotiates. Anything later takes the SDK's
/// stateless, session-less path, which would defeat session caps,
/// connection-local allowances and DELETE cancellation.
const NEWEST_SESSION_PROTOCOL: &str = "2025-11-25";

fn jsonrpc_is_initialize(value: &serde_json::Value) -> bool {
    value
        .get("method")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|method| method == "initialize")
}

/// A request naming a newer protocol that is not an `initialize` asks for
/// the stateless path: refused before it can allocate anything. (An
/// `initialize` naming a newer version is ordinary MCP negotiation and is
/// answered with the newest session-bearing version.)
fn stateless_protocol_refusal() -> http::Response<axum::body::Body> {
    http::Response::builder()
        .status(http::StatusCode::BAD_REQUEST)
        .body(axum::body::Body::from(
            "{\"error\":\"stateless_protocol_unsupported\"}",
        ))
        .expect("static 400 response")
}

/// A JSON-RPC message that needs a handler task is a REQUEST (it has both a
/// `method` and an `id`). Notifications (`notifications/cancelled`,
/// `initialized`) and client responses never take handler capacity, so
/// cancellation and overload relief stay serviceable at saturation.
fn count_jsonrpc_requests(value: &serde_json::Value) -> usize {
    fn is_request(object: &serde_json::Map<String, serde_json::Value>) -> bool {
        object.contains_key("method") && object.contains_key("id")
    }
    match value {
        serde_json::Value::Object(object) => usize::from(is_request(object)),
        serde_json::Value::Array(items) => items
            .iter()
            .filter(|item| item.as_object().is_some_and(is_request))
            .count(),
        _ => 0,
    }
}

fn refusal(status: http::StatusCode, body: &'static str) -> http::Response<axum::body::Body> {
    http::Response::builder()
        .status(status)
        .header(http::header::RETRY_AFTER, "1")
        .body(axum::body::Body::from(body))
        .expect("static refusal response")
}

/// Bearer authentication on EVERY MCP HTTP method (before protocol/session
/// allocation) plus pre-dispatch handler admission. Runs as axum middleware
/// in front of the SDK service: refused requests never reach SDK dispatch,
/// so no SDK handler task is spawned for them.
///
/// Only JSON-RPC REQUESTS take permits. GET (standalone SSE streams) and
/// DELETE take none; a POST carrying only notifications/responses takes
/// none. The POST body is read here under the 64 KiB cap (chunked input
/// included), classified, then rebuilt for the SDK. The permit guard rides
/// the request extensions into the SDK, which moves `http::request::Parts`
/// into `RequestContext.extensions`; it is released only when that
/// handler's context (and any detached engine work holding a clone) drops.
async fn auth_admission_gate(
    axum::extract::State(state): axum::extract::State<GateState>,
    request: http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> http::Response<axum::body::Body> {
    if !bearer_matches(
        request.headers().get(http::header::AUTHORIZATION),
        &state.secret,
    ) {
        return http::Response::builder()
            .status(http::StatusCode::UNAUTHORIZED)
            .header(http::header::WWW_AUTHENTICATE, "Bearer")
            .body(axum::body::Body::empty())
            .expect("static 401 response");
    }
    // Exact authority equality and no browser Origin: refused after
    // authentication and before any protocol or session allocation.
    let host_matches = request
        .headers()
        .get(http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| host == &*state.host);
    if !host_matches || request.headers().contains_key(http::header::ORIGIN) {
        return http::Response::builder()
            .status(http::StatusCode::FORBIDDEN)
            .body(axum::body::Body::empty())
            .expect("static 403 response");
    }
    let newer_protocol = request
        .headers()
        .get("mcp-protocol-version")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|version| version > NEWEST_SESSION_PROTOCOL);
    let (parts, body) = request.into_parts();
    if parts.method != http::Method::POST {
        if newer_protocol {
            return stateless_protocol_refusal();
        }
        return next.run(http::Request::from_parts(parts, body)).await;
    }
    let bytes = match axum::body::to_bytes(body, MAX_INBOUND_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return http::Response::builder()
                .status(http::StatusCode::PAYLOAD_TOO_LARGE)
                .body(axum::body::Body::empty())
                .expect("static 413 response");
        }
    };
    // One parse serves both classifications; malformed JSON classifies as
    // `null` (not an initialize, no requests).
    let requests = {
        let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        if newer_protocol && !jsonrpc_is_initialize(&value) {
            return stateless_protocol_refusal();
        }
        count_jsonrpc_requests(&value)
    };
    let mut request = http::Request::from_parts(parts, axum::body::Body::from(bytes));
    if requests > 0 {
        let Ok(permit) = Arc::clone(&state.permits).try_acquire_many_owned(requests as u32) else {
            return refusal(
                http::StatusCode::SERVICE_UNAVAILABLE,
                "{\"error\":\"handler_admission_exceeded\"}",
            );
        };
        request
            .extensions_mut()
            .insert(Arc::new(AdmissionGuard { _permit: permit }));
    }
    next.run(request).await
}

pub async fn serve_http(options: ServerOptions, http: HttpOptions) -> AResult<HttpServe> {
    // The delivery-only capability is validated before the secret is read
    // and before any store is opened.
    options.budget.require_delivery()?;
    // Nonempty secret from the named environment variable, before opening
    // the store; never printed or logged.
    let secret = std::env::var(&http.token_env)
        .map_err(|_| {
            FoundryError::InvalidArgument(format!(
                "HTTP bearer token environment variable `{}` is not set",
                http.token_env
            ))
        })
        .and_then(|value| {
            if value.trim().is_empty() {
                Err(FoundryError::InvalidArgument(format!(
                    "HTTP bearer token environment variable `{}` is empty",
                    http.token_env
                )))
            } else {
                Ok(value)
            }
        })?;
    // Bind before starting an owned watcher/runtime. A bind failure must not
    // leave background work retaining a store whose HTTP owner never started.
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", http.port))
        .await
        .map_err(|e| FoundryError::InvalidArgument(format!("bind 127.0.0.1:{}: {e}", http.port)))?;
    let address = listener
        .local_addr()
        .map_err(|e| FoundryError::Internal(e.into()))?;
    // 007 launch-time admission (refused before serving) and the one-time
    // open of every root; the owner-level shutdown token is the HTTP one.
    let state = open_owner(options, http.shutdown.clone())?;
    let mut inner_manager = LocalSessionManager::default();
    // `SessionConfig` is #[non_exhaustive]: start from the SDK default and
    // pin only the documented fields.
    inner_manager.session_config.keep_alive = Some(http.keep_alive);
    inner_manager.session_config.completed_cache_ttl = COMPLETED_CACHE_TTL;
    let session_manager = Arc::new(CappedSessionManager {
        inner: inner_manager,
        create_lock: tokio::sync::Mutex::new(()),
        shared: Arc::clone(&state),
    });
    let config = StreamableHttpServerConfig::default()
        // Sessions are required: connection-local allowances, the session
        // cap and DELETE-based cancellation all depend on them.
        .with_legacy_session_mode(true)
        // Exact IPv4 loopback authority; any other Host is rejected.
        .with_allowed_hosts([format!("127.0.0.1:{}", address.port())])
        // Empty allowlist plus enforcement rejects every present Origin.
        .enforce_origin_validation()
        // Inbound request body bound (including chunked input); oversized
        // bodies get 413 for that request only. Output uses 256 KiB.
        .with_max_request_body_bytes(MAX_INBOUND_BYTES)
        .with_cancellation_token(http.shutdown.clone());
    let factory_state = Arc::clone(&state);
    let service = StreamableHttpService::new(
        move || Ok(FoundryMcp::new(Arc::clone(&factory_state))),
        session_manager,
        config,
    );
    let router = gated(
        axum::Router::new().nest_service("/mcp", service),
        GateState {
            secret: Arc::new(secret),
            permits: Arc::new(tokio::sync::Semaphore::new(MAX_HANDLER_ADMISSION)),
            host: Arc::from(format!("127.0.0.1:{}", address.port())),
        },
    );

    let shutdown_signal = http.shutdown.clone();
    let serve_state = Arc::clone(&state);
    let (done_tx, done_rx) = tokio::sync::watch::channel(());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                shutdown_signal.cancelled().await;
            })
            .await;
        // Shared-owner shutdown: cancel active operations and SDK sessions;
        // exit waits for the current engine transaction (bounded above the
        // maximum index timeout), for the preparation driver to record its
        // stop and for the resident worker to be stopped and reaped;
        // in-flight replies can be lost. `done` fires only after those waits
        // so a foreground owner can exit.
        serve_state.shut_down().await;
        let _ = done_tx.send(());
    });
    Ok(HttpServe {
        address,
        done: done_rx,
    })
}

/// A running shared HTTP owner. `address` is the bound IPv4 loopback
/// listener; `done` completes after shutdown has waited for the current
/// engine transaction.
pub struct HttpServe {
    pub address: std::net::SocketAddr,
    pub done: tokio::sync::watch::Receiver<()>,
}

/// Wrap `inner` with bearer authentication and pre-dispatch admission. This
/// is the one place the HTTP gate is attached, for production and tests.
fn gated(inner: axum::Router, gate: GateState) -> axum::Router {
    inner.layer(axum::middleware::from_fn_with_state(
        gate,
        auth_admission_gate,
    ))
}

/// Pre-spawn admission for ONE decoded stdio frame; the real function the
/// stdio `scan` runs. A request takes one of the 16 permits (carried in the
/// request's extensions until its handler finishes). Returning `None` ends
/// the stream, which closes the session: at capacity, or on an oversized or
/// malformed frame, each with one bounded stderr diagnostic. Notifications
/// and responses never take permits, so cancellation stays serviceable.
fn admit_stdio_item(
    permits: &Arc<tokio::sync::Semaphore>,
    item: Result<RxJsonRpcMessage<RoleServer>, JsonRpcMessageCodecError>,
) -> Option<RxJsonRpcMessage<RoleServer>> {
    let mut message = match item {
        Ok(message) => message,
        Err(error) => {
            eprintln!("foundry-mcp: frame refused before decode: {error:.200}");
            return None;
        }
    };
    if let JsonRpcMessage::Request(request) = &mut message {
        let Ok(permit) = Arc::clone(permits).try_acquire_owned() else {
            eprintln!(
                "foundry-mcp: admission: {MAX_HANDLER_ADMISSION} in-flight request handlers; closing session"
            );
            return None;
        };
        request
            .request
            .extensions_mut()
            .insert(Arc::new(AdmissionGuard { _permit: permit }));
    }
    Some(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_message(id: i64) -> RxJsonRpcMessage<RoleServer> {
        serde_json::from_value(
            serde_json::json!({"jsonrpc": "2.0", "id": id, "method": "tools/list"}),
        )
        .expect("a valid JSON-RPC request")
    }

    fn notification_message() -> RxJsonRpcMessage<RoleServer> {
        serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0", "method": "notifications/cancelled",
            "params": {"requestId": 1, "reason": "test"}
        }))
        .expect("a valid JSON-RPC notification")
    }

    #[test]
    fn stdio_admission_closes_at_the_seventeenth_request_and_passes_notifications() {
        let permits = Arc::new(tokio::sync::Semaphore::new(MAX_HANDLER_ADMISSION));
        let mut in_flight = Vec::new();
        for id in 0..MAX_HANDLER_ADMISSION as i64 {
            in_flight.push(
                admit_stdio_item(&permits, Ok(request_message(id)))
                    .expect("requests under the cap are admitted"),
            );
        }
        assert_eq!(
            permits.available_permits(),
            0,
            "16 requests hold 16 permits"
        );
        assert!(
            admit_stdio_item(&permits, Ok(request_message(99))).is_none(),
            "the 17th request closes the session before any handler task exists"
        );
        assert!(
            admit_stdio_item(&permits, Ok(notification_message())).is_some(),
            "cancellation notifications stay serviceable at capacity"
        );
        // Capacity returns only when an admitted request's handler context
        // (the message carrying the guard) is dropped.
        drop(in_flight.pop());
        assert!(admit_stdio_item(&permits, Ok(request_message(100))).is_some());
    }

    #[test]
    fn request_classification_counts_only_handler_bearing_messages() {
        let count = |text: &str| {
            count_jsonrpc_requests(
                &serde_json::from_slice(text.as_bytes()).unwrap_or(serde_json::Value::Null),
            )
        };
        assert_eq!(
            count(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call"}"#),
            1
        );
        assert_eq!(
            count(r#"{"jsonrpc":"2.0","method":"notifications/cancelled"}"#),
            0
        );
        assert_eq!(count(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#), 0);
        assert_eq!(
            count(
                r#"[{"jsonrpc":"2.0","id":1,"method":"a"},{"jsonrpc":"2.0","method":"n"},{"jsonrpc":"2.0","id":2,"method":"b"}]"#
            ),
            2
        );
        assert_eq!(count("not json"), 0);
    }

    async fn gate_server(permits: Arc<tokio::sync::Semaphore>) -> String {
        // Stand-in for the SDK service: reports whether the admission guard
        // rode the request into it.
        async fn reached(
            request: http::Request<axum::body::Body>,
        ) -> http::Response<axum::body::Body> {
            let guarded = request.extensions().get::<Arc<AdmissionGuard>>().is_some();
            http::Response::builder()
                .status(http::StatusCode::OK)
                .body(axum::body::Body::from(if guarded {
                    "guarded"
                } else {
                    "open"
                }))
                .expect("static response")
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let router = gated(
            axum::Router::new().route("/mcp", axum::routing::any(reached)),
            GateState {
                secret: Arc::new("s3cret".to_owned()),
                permits,
                host: Arc::from(addr.to_string()),
            },
        );
        let url = format!("http://{addr}/mcp");
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        url
    }

    #[tokio::test]
    async fn http_gate_refuses_requests_at_capacity_but_passes_control_traffic() {
        let permits = Arc::new(tokio::sync::Semaphore::new(MAX_HANDLER_ADMISSION));
        let url = gate_server(Arc::clone(&permits)).await;
        let http = reqwest::Client::new();
        let send = |method: reqwest::Method, body: &'static str| {
            http.request(method, &url)
                .header("authorization", "Bearer s3cret")
                .body(body)
        };
        let request =
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"status"}}"#;
        let held = Arc::clone(&permits)
            .try_acquire_many_owned(MAX_HANDLER_ADMISSION as u32)
            .expect("test holds all 16 permits");

        // A JSON-RPC request and a batch of requests are refused with 503.
        assert_eq!(
            send(reqwest::Method::POST, request)
                .send()
                .await
                .unwrap()
                .status(),
            503
        );
        let batch =
            r#"[{"jsonrpc":"2.0","id":1,"method":"a"},{"jsonrpc":"2.0","id":2,"method":"b"}]"#;
        assert_eq!(
            send(reqwest::Method::POST, batch)
                .send()
                .await
                .unwrap()
                .status(),
            503
        );

        // Control traffic still passes at saturation: notifications, client
        // responses, GET streams and DELETE take no permit.
        let cancelled =
            r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#;
        for (method, body) in [
            (reqwest::Method::POST, cancelled),
            (
                reqwest::Method::POST,
                r#"{"jsonrpc":"2.0","id":1,"result":{}}"#,
            ),
            (reqwest::Method::GET, ""),
            (reqwest::Method::DELETE, ""),
        ] {
            let label = format!("{method} {body}");
            let response = send(method, body).send().await.unwrap();
            assert_eq!(
                response.status(),
                200,
                "{label} stays serviceable at capacity"
            );
        }

        // Authentication precedes admission on every method; the body cap
        // rejects the request alone.
        for method in [
            reqwest::Method::POST,
            reqwest::Method::GET,
            reqwest::Method::DELETE,
        ] {
            let unauthorized = http.request(method, &url).send().await.unwrap();
            assert_eq!(unauthorized.status(), 401);
        }
        let oversize = http
            .post(&url)
            .header("authorization", "Bearer s3cret")
            .body(vec![b'x'; MAX_INBOUND_BYTES + 1])
            .send()
            .await
            .unwrap();
        assert_eq!(oversize.status(), 413);

        // With capacity back, a request is admitted with its guard attached,
        // and the permit returns when the request is done.
        // Exact Host equality (no port normalization) and no Origin at all:
        // each negative carries a valid bearer, so ONLY that check refuses.
        let port = url
            .trim_end_matches("/mcp")
            .rsplit(':')
            .next()
            .expect("port")
            .to_owned();
        for host in [
            format!("127.0.0.1:0{port}"),
            format!("localhost:{port}"),
            "127.0.0.1".to_owned(),
        ] {
            let refused = http
                .post(&url)
                .header("authorization", "Bearer s3cret")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("host", host.clone())
                .body(cancelled)
                .send()
                .await
                .unwrap();
            assert_eq!(
                refused.status(),
                403,
                "Host {host} is not the exact bound authority"
            );
        }
        let with_origin = http
            .post(&url)
            .header("authorization", "Bearer s3cret")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("origin", "http://127.0.0.1")
            .body(cancelled)
            .send()
            .await
            .unwrap();
        assert_eq!(with_origin.status(), 403, "any Origin is refused");

        // A request naming the stateless 2026 protocol that is not an
        // `initialize` is refused before it can allocate anything.
        let stateless = send(reqwest::Method::POST, request)
            .header("mcp-protocol-version", "2026-07-28")
            .send()
            .await
            .unwrap();
        assert_eq!(stateless.status(), 400);
        let stateless_get = send(reqwest::Method::GET, "")
            .header("mcp-protocol-version", "2026-07-28")
            .send()
            .await
            .unwrap();
        assert_eq!(stateless_get.status(), 400);

        drop(held);
        // An `initialize` naming 2026-07-28 is ordinary negotiation and is
        // passed to the SDK, which answers with the newest session version.
        let initialize = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2026-07-28","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#;
        let negotiated = send(reqwest::Method::POST, initialize)
            .header("mcp-protocol-version", "2026-07-28")
            .send()
            .await
            .unwrap();
        assert_eq!(negotiated.status(), 200);
        let admitted = send(reqwest::Method::POST, request).send().await.unwrap();
        assert_eq!(admitted.status(), 200);
        assert_eq!(admitted.text().await.unwrap(), "guarded");
        assert_eq!(permits.available_permits(), MAX_HANDLER_ADMISSION);
    }
}
