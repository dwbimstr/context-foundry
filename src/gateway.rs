//! Owned model gateway (003 T004): a loopback-only SSE forwarder for the
//! pinned OMP 18.6.0 / Z.ai `glm-5.3-flash` chat-completions profile, with
//! usage receipts. It never opens the source store: source and gateway
//! processes have different side effects and credentials.
//!
//! Intentional difference from the usual SSE crates (`sse-stream`): events
//! are split from raw bytes here. Lines end LF, CRLF or CR, an event ends at
//! a blank line and is bounded to 1 MiB, and every observed event is
//! forwarded unchanged (byte-exact, in order, terminating blank line
//! included). That also lets an upstream error event be withheld from the
//! host. A parsing crate yields events without a per-event bound and would
//! force re-serialization.

use std::{
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use bytes::{Bytes, BytesMut};
use futures_util::{StreamExt as _, stream::BoxStream};
use http_body_util::{BodyExt as _, Full, combinators::UnsyncBoxBody};
use hyper::{
    Method, StatusCode,
    body::{Body as HttpBody, Frame, Incoming},
    header::{
        AUTHORIZATION, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HOST, HeaderMap,
        HeaderValue, ORIGIN, UPGRADE,
    },
    service::service_fn,
};
use hyper_util::rt::{TokioIo, TokioTimer};
use serde::Deserializer as _;
use serde_json::{Value, json};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Mutex as AsyncMutex, OwnedMutexGuard, OwnedSemaphorePermit, Semaphore, mpsc, oneshot},
    task::JoinSet,
    time::{Instant, sleep, sleep_until, timeout, timeout_at},
};
use tokio_util::sync::CancellationToken;

use crate::{
    adapter_error::{AResult, AdapterError},
    receipts::{
        CONTEXT_IDS_MAX, Delivery, Mode, Observation, Outcome, RECEIPT_MAX_BYTES, Receipt,
        ReceiptDedup, ReceiptLog,
    },
};

pub const ADAPTER_ID: &str = "foundry-gateway";
pub const MODEL_ID: &str = "glm-5.3-flash";
pub const PINNED_UPSTREAM: &str = "https://api.z.ai/api/coding/paas/v4";
/// Pinned catalog windows for `zai.glm-5.3-flash` (OMP 18.6.0): the input
/// context and the largest `max_tokens` the request profile accepts. Meter
/// mode makes no preflight cap decision from the input window.
pub const PINNED_INPUT_WINDOW_TOKENS: u64 = 1_000_000;
pub const PINNED_MAX_OUTPUT_TOKENS: u64 = 131_072;
/// The environment variable NAME OMP reads the local bearer token from; the
/// value is generated per run and never printed.
pub const TOKEN_ENV: &str = "FOUNDRY_GATEWAY_TOKEN";

const CONFIG_MAX_BYTES: usize = 64 * 1024;
const MAX_CONNECTIONS: usize = 8;
const HEADER_MAX_BYTES: usize = 16 * 1024;
const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(5);
const BODY_MAX_BYTES: usize = 4 * 1024 * 1024;
const ATTEMPT_DEADLINE: Duration = Duration::from_secs(600);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const EVENT_MAX_BYTES: usize = 1024 * 1024;
const RESPONSE_MAX_BYTES: u64 = 64 * 1024 * 1024;
/// Unread bytes toward the client; permits are taken per forwarded chunk and
/// returned once hyper has consumed the frame.
const BACKPRESSURE_BYTES: usize = 256 * 1024;
/// Chunks stay small so one event can never demand more credit than exists.
const FORWARD_CHUNK_BYTES: usize = 32 * 1024;
const CHANNEL_CAPACITY: usize = 1024;
const MAX_ATTEMPTS: u64 = 10_000;
const LOG_MIN_BYTES: u64 = 65_536;
const LOG_MAX_BYTES: u64 = 16 * 1024 * 1024;
const CONTEXT_IDS_MAX_BYTES: usize = 4096;
const UPSTREAM_ERROR_DRAIN_BYTES: usize = 64 * 1024;
const PROVIDER_ID_MAX_BYTES: usize = 128;
/// Leaves headroom inside the contractual five seconds after the signal.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(4);

fn invalid(message: &str) -> AdapterError {
    AdapterError::named("invalid_argument", message)
}

fn unsupported(message: &str) -> AdapterError {
    AdapterError::named("gateway_feature_unsupported", message)
}

/// Request bounds with test-only overrides (`FOUNDRY_GATEWAY_TEST_IDLE_MS`,
/// `FOUNDRY_GATEWAY_TEST_DEADLINE_MS`); release builds use the contract
/// defaults and read no environment.
struct Limits {
    idle: Duration,
    deadline: Duration,
}

impl Limits {
    #[cfg(not(feature = "test-faults"))]
    fn load() -> Self {
        Self {
            idle: IDLE_TIMEOUT,
            deadline: ATTEMPT_DEADLINE,
        }
    }

    #[cfg(feature = "test-faults")]
    fn load() -> Self {
        let millis = |name: &str| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(Duration::from_millis)
        };
        Self {
            idle: millis("FOUNDRY_GATEWAY_TEST_IDLE_MS").unwrap_or(IDLE_TIMEOUT),
            deadline: millis("FOUNDRY_GATEWAY_TEST_DEADLINE_MS").unwrap_or(ATTEMPT_DEADLINE),
        }
    }
}

/// The upstream base URL and whether the client is HTTPS-only. Under
/// `test-faults`, `FOUNDRY_GATEWAY_TEST_UPSTREAM` replaces the pinned origin
/// with a plain-http loopback fake; release builds contain none of this.
#[cfg(not(feature = "test-faults"))]
fn upstream_target() -> AResult<(String, bool)> {
    Ok((PINNED_UPSTREAM.to_owned(), true))
}

#[cfg(feature = "test-faults")]
fn upstream_target() -> AResult<(String, bool)> {
    let Ok(raw) = std::env::var("FOUNDRY_GATEWAY_TEST_UPSTREAM") else {
        return Ok((PINNED_UPSTREAM.to_owned(), true));
    };
    let url = reqwest::Url::parse(&raw)
        .map_err(|_| invalid("FOUNDRY_GATEWAY_TEST_UPSTREAM is not a URL"))?;
    if url.scheme() != "http" || !matches!(url.host_str(), Some("127.0.0.1" | "localhost")) {
        return Err(invalid(
            "FOUNDRY_GATEWAY_TEST_UPSTREAM must be plain http on loopback",
        ));
    }
    Ok((raw.trim_end_matches('/').to_owned(), false))
}

// ---------------------------------------------------------------------------
// Config v1
// ---------------------------------------------------------------------------

/// Gateway config v1 holds names and paths, never secret values. Only the
/// pinned profile exists: other upstreams/models and `mode: enforce` are
/// `gateway_feature_unsupported`; every other deviation is `invalid_argument`.
pub(crate) struct GatewayConfig {
    pub(crate) port: u16,
    pub(crate) credential_env: String,
    pub(crate) run_dir: PathBuf,
    pub(crate) log_bytes: Option<u64>,
}

const CONFIG_KEYS: [&str; 8] = [
    "v",
    "port",
    "upstream",
    "model",
    "mode",
    "credential_env",
    "run_dir",
    "log_bytes",
];

fn valid_env_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    name.len() <= 256
        && bytes
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

pub(crate) fn parse_config(bytes: &[u8]) -> AResult<GatewayConfig> {
    if bytes.len() > CONFIG_MAX_BYTES {
        return Err(invalid("gateway config exceeds 64 KiB"));
    }
    let value = strict_parse(bytes).map_err(|message| invalid(&message))?;
    let Value::Object(fields) = value else {
        return Err(invalid("gateway config must be a JSON object"));
    };
    if fields.contains_key("limits") {
        return Err(unsupported("`limits` are not supported by the gateway"));
    }
    if let Some(key) = fields
        .keys()
        .find(|key| !CONFIG_KEYS.contains(&key.as_str()))
    {
        return Err(invalid(&format!("unknown gateway config key `{key}`")));
    }
    let field = |name: &str| {
        fields
            .get(name)
            .ok_or_else(|| invalid(&format!("`{name}` is required")))
    };
    if field("v")?.as_u64() != Some(1) {
        return Err(invalid("`v` must be 1"));
    }
    let port = field("port")?
        .as_u64()
        .and_then(|port| u16::try_from(port).ok())
        .ok_or_else(|| invalid("`port` must be an integer in 0..=65535"))?;
    let upstream = field("upstream")?
        .as_str()
        .ok_or_else(|| invalid("`upstream` must be a string"))?;
    let model = field("model")?
        .as_str()
        .ok_or_else(|| invalid("`model` must be a string"))?;
    let mode = field("mode")?
        .as_str()
        .ok_or_else(|| invalid("`mode` must be a string"))?;
    match mode {
        "meter" => {}
        "enforce" => {
            return Err(unsupported(
                "`mode: enforce` needs a verified provider counting API",
            ));
        }
        _ => return Err(invalid("`mode` must be `meter`")),
    }
    if upstream != PINNED_UPSTREAM {
        return Err(unsupported(
            "`upstream` must be the pinned Z.ai coding-plan origin",
        ));
    }
    if model != MODEL_ID {
        return Err(unsupported("`model` must be the pinned glm-5.3-flash"));
    }
    let credential_env = field("credential_env")?
        .as_str()
        .ok_or_else(|| invalid("`credential_env` must be a string"))?;
    if !valid_env_name(credential_env) {
        return Err(invalid(
            "`credential_env` must match [A-Za-z_][A-Za-z0-9_]* within 256 bytes",
        ));
    }
    let run_dir = field("run_dir")?
        .as_str()
        .ok_or_else(|| invalid("`run_dir` must be a string"))?;
    if run_dir.len() > 4096 || !Path::new(run_dir).is_absolute() {
        return Err(invalid(
            "`run_dir` must be an absolute path of at most 4096 bytes",
        ));
    }
    // Omitted disables the receipt file; null and out-of-range values refuse.
    let log_bytes = match fields.get("log_bytes") {
        None => None,
        Some(value) => {
            let cap = value
                .as_u64()
                .filter(|cap| (LOG_MIN_BYTES..=LOG_MAX_BYTES).contains(cap))
                .ok_or_else(|| invalid("`log_bytes` must be an integer in 65536..=16777216"))?;
            Some(cap)
        }
    };
    Ok(GatewayConfig {
        port,
        credential_env: credential_env.to_owned(),
        run_dir: PathBuf::from(run_dir),
        log_bytes,
    })
}

// ---------------------------------------------------------------------------
// Strict JSON: duplicate keys and nesting depth
// ---------------------------------------------------------------------------

/// Parse UTF-8 JSON, rejecting duplicate object keys, trailing data and
/// nesting deeper than 64 containers. serde_json's own `Value` keeps the
/// last duplicate silently, which would let two readers disagree about a
/// request.
fn strict_parse(bytes: &[u8]) -> Result<Value, String> {
    std::str::from_utf8(bytes).map_err(|_| "input is not valid UTF-8".to_owned())?;
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = (&mut deserializer)
        .deserialize_any(StrictVisitor { depth: 1 })
        .map_err(|e| format!("invalid JSON: {e}"))?;
    deserializer
        .end()
        .map_err(|e| format!("invalid JSON: {e}"))?;
    Ok(value)
}

const MAX_DEPTH: u32 = 64;

struct StrictVisitor {
    depth: u32,
}

impl<'de> serde::de::Visitor<'de> for StrictVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::from(v))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::from(v))
    }

    fn visit_f64<E>(self, v: f64) -> Result<Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::from(v))
    }

    fn visit_str<E>(self, v: &str) -> Result<Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::from(v))
    }

    fn visit_unit<E>(self) -> Result<Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        if self.depth > MAX_DEPTH {
            return Err(serde::de::Error::custom("nesting depth exceeds 64"));
        }
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(StrictVisitor {
            depth: self.depth + 1,
        })? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        if self.depth > MAX_DEPTH {
            return Err(serde::de::Error::custom("nesting depth exceeds 64"));
        }
        let mut object = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if object.contains_key(&key) {
                return Err(serde::de::Error::custom("duplicate object key"));
            }
            let value = map.next_value_seed(StrictVisitor {
                depth: self.depth + 1,
            })?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}

impl<'de> serde::de::DeserializeSeed<'de> for StrictVisitor {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

// ---------------------------------------------------------------------------
// Pinned request profile (OMP 18.6.0 capture)
// ---------------------------------------------------------------------------

/// A static reason: the refusal text never carries request content.
type Refusal = &'static str;

const TOP_LEVEL_KEYS: [&str; 12] = [
    "model",
    "messages",
    "stream",
    "stream_options",
    "tools",
    "tool_choice",
    "max_tokens",
    "reasoning_effort",
    "temperature",
    "top_p",
    "tool_stream",
    "thinking",
];

/// Validate a parsed request against the pinned subset. Anything outside it
/// is refused as `gateway_feature_unsupported` so hidden charges (another
/// model, images, extra fields, a non-streaming reply) are not mislabeled as
/// metered traffic.
pub(crate) fn validate_request(value: &Value) -> Result<(), Refusal> {
    let Value::Object(fields) = value else {
        return Err("the request must be a JSON object");
    };
    if fields
        .keys()
        .any(|key| !TOP_LEVEL_KEYS.contains(&key.as_str()))
    {
        return Err("a top-level field is outside the pinned profile");
    }
    if fields.get("model").and_then(Value::as_str) != Some(MODEL_ID) {
        return Err("`model` must be the pinned model");
    }
    if fields.get("stream") != Some(&Value::Bool(true)) {
        return Err("`stream` must be true");
    }
    if let Some(options) = fields.get("stream_options") {
        match options {
            Value::Object(keys)
                if keys.len() == 1 && keys.get("include_usage").is_some_and(Value::is_boolean) => {}
            _ => return Err("`stream_options` must be {include_usage: bool}"),
        }
    }
    if let Some(max) = fields.get("max_tokens")
        && !max
            .as_u64()
            .is_some_and(|max| (1..=PINNED_MAX_OUTPUT_TOKENS).contains(&max))
    {
        return Err("`max_tokens` must be an integer in 1..=131072");
    }
    if let Some(effort) = fields.get("reasoning_effort")
        && !matches!(effort.as_str(), Some("low" | "high" | "max"))
    {
        return Err("`reasoning_effort` must be low, high or max");
    }
    for key in ["temperature", "top_p"] {
        if fields.get(key).is_some_and(|value| !value.is_number()) {
            return Err("`temperature` and `top_p` must be numbers");
        }
    }
    if fields
        .get("tool_stream")
        .is_some_and(|value| !value.is_boolean())
    {
        return Err("`tool_stream` must be a bool");
    }
    if let Some(thinking) = fields.get("thinking") {
        match thinking {
            Value::Object(keys)
                if keys.len() == 1
                    && matches!(
                        keys.get("type").and_then(Value::as_str),
                        Some("enabled" | "disabled")
                    ) => {}
            _ => return Err("`thinking` must be {type: enabled|disabled}"),
        }
    }
    if let Some(choice) = fields.get("tool_choice") {
        validate_tool_choice(choice)?;
    }
    validate_messages(fields)?;
    validate_tools(fields)
}

fn validate_tool_choice(choice: &Value) -> Result<(), Refusal> {
    match choice {
        Value::String(mode) if matches!(mode.as_str(), "auto" | "none" | "required") => Ok(()),
        Value::Object(keys)
            if keys.len() == 2
                && keys.get("type").and_then(Value::as_str) == Some("function")
                && matches!(
                    keys.get("function"),
                    Some(Value::Object(function))
                        if function.len() == 1 && function.get("name").is_some_and(Value::is_string)
                ) =>
        {
            Ok(())
        }
        _ => Err("`tool_choice` is outside the pinned profile"),
    }
}

fn validate_messages(fields: &serde_json::Map<String, Value>) -> Result<(), Refusal> {
    let Some(Value::Array(messages)) = fields.get("messages") else {
        return Err("`messages` must be an array");
    };
    if messages.is_empty() {
        return Err("`messages` must not be empty");
    }
    for message in messages {
        let Value::Object(keys) = message else {
            return Err("each message must be an object");
        };
        let Some(role) = keys.get("role").and_then(Value::as_str) else {
            return Err("each message needs a string role");
        };
        match role {
            "system" | "user" => {
                if keys.len() != 2 {
                    return Err("system and user messages allow only role and content");
                }
                validate_content(keys.get("content"))?;
            }
            "assistant" => validate_assistant(keys)?,
            "tool" => {
                if keys.len() != 3 || !keys.get("tool_call_id").is_some_and(Value::is_string) {
                    return Err("tool messages allow only role, content and tool_call_id");
                }
                validate_content(keys.get("content"))?;
            }
            _ => return Err("the message role is outside the pinned profile"),
        }
    }
    Ok(())
}

fn validate_assistant(keys: &serde_json::Map<String, Value>) -> Result<(), Refusal> {
    if keys.keys().any(|key| {
        !matches!(
            key.as_str(),
            "role" | "content" | "reasoning_content" | "tool_calls"
        )
    }) {
        return Err("an assistant message field is outside the pinned profile");
    }
    match keys.get("content") {
        None if !keys.contains_key("tool_calls") => {
            return Err("assistant `content` may be absent only beside tool_calls");
        }
        None | Some(Value::Null) => {}
        content => validate_content(content)?,
    }
    if keys
        .get("reasoning_content")
        .is_some_and(|value| !value.is_string())
    {
        return Err("`reasoning_content` must be a string");
    }
    if let Some(calls) = keys.get("tool_calls") {
        let Value::Array(calls) = calls else {
            return Err("`tool_calls` must be an array");
        };
        for call in calls {
            validate_tool_call(call)?;
        }
    }
    Ok(())
}

/// Text content is a string or an array of `{type:"text", text}` parts; any
/// other part (image, file, audio, ...) is refused.
fn validate_content(content: Option<&Value>) -> Result<(), Refusal> {
    match content {
        Some(Value::String(_)) => Ok(()),
        Some(Value::Array(parts)) => {
            for part in parts {
                match part {
                    Value::Object(keys)
                        if keys.len() == 2
                            && keys.get("type").and_then(Value::as_str) == Some("text")
                            && keys.get("text").is_some_and(Value::is_string) => {}
                    Value::Object(keys) if keys.contains_key("type") => {
                        return Err("a content part type is outside the pinned profile");
                    }
                    _ => return Err("content parts must be {type:\"text\", text}"),
                }
            }
            Ok(())
        }
        _ => Err("message content must be a string or text parts"),
    }
}

fn validate_tool_call(call: &Value) -> Result<(), Refusal> {
    let well_formed = match call {
        Value::Object(keys) => {
            keys.len() == 3
                && keys.get("id").is_some_and(Value::is_string)
                && keys.get("type").and_then(Value::as_str) == Some("function")
                && matches!(
                    keys.get("function"),
                    Some(Value::Object(function))
                        if function.len() == 2
                            && function.get("name").is_some_and(Value::is_string)
                            && function.get("arguments").is_some_and(Value::is_string)
                )
        }
        _ => false,
    };
    if well_formed {
        Ok(())
    } else {
        Err("tool_calls items must be {id, type:function, function:{name, arguments}}")
    }
}

fn validate_tools(fields: &serde_json::Map<String, Value>) -> Result<(), Refusal> {
    let Some(tools) = fields.get("tools") else {
        return Ok(());
    };
    let Value::Array(tools) = tools else {
        return Err("`tools` must be an array");
    };
    for tool in tools {
        let Value::Object(keys) = tool else {
            return Err("each tool must be an object");
        };
        let Some(Value::Object(function)) = keys.get("function") else {
            return Err("each tool needs a function object");
        };
        if keys.len() != 2
            || keys.get("type").and_then(Value::as_str) != Some("function")
            || !function.get("name").is_some_and(Value::is_string)
        {
            return Err("tools items must be {type:function, function:{name, ...}}");
        }
        if function.keys().any(|key| {
            !matches!(
                key.as_str(),
                "name" | "description" | "parameters" | "strict"
            )
        }) {
            return Err("a tool function field is outside the pinned profile");
        }
        if function
            .get("description")
            .is_some_and(|value| !value.is_string())
            || function
                .get("parameters")
                .is_some_and(|value| !value.is_object())
            || function
                .get("strict")
                .is_some_and(|value| !value.is_boolean())
        {
            return Err("a tool function field has the wrong type");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// In-memory accounting
// ---------------------------------------------------------------------------

#[derive(Default, Clone, Copy)]
struct Usage {
    input: Option<u64>,
    cached: Option<u64>,
    output: Option<u64>,
}

enum Admission {
    Open,
    /// Log full or log write failure.
    Closed,
    /// The 10,000-attempt bound; there is no eviction.
    Full,
}

/// A known total with checked arithmetic: an unrepresentable sum becomes
/// unknown (and is flagged) rather than wrapping or saturating.
#[derive(Default)]
struct Total {
    value: Option<u64>,
    overflowed: bool,
}

impl Total {
    fn add(&mut self, count: Option<u64>) {
        let Some(count) = count else { return };
        if self.overflowed {
            return;
        }
        match self.value.unwrap_or(0).checked_add(count) {
            Some(sum) => self.value = Some(sum),
            None => {
                self.value = None;
                self.overflowed = true;
            }
        }
    }
}

struct Book {
    attempts: u64,
    complete: u64,
    failed: u64,
    unknown: u64,
    in_flight: u64,
    input: Total,
    cached: Total,
    output: Total,
    dedup: ReceiptDedup,
    log: Option<ReceiptLog>,
    log_cap: u64,
    log_written: u64,
    log_failed: bool,
    /// Local refusals by code: every request that reached the gateway and
    /// was refused before an attempt was recorded (not receipts).
    refused: std::collections::BTreeMap<String, u64>,
}

impl Book {
    fn new(run_dir: &Path, log_bytes: Option<u64>) -> AResult<Self> {
        let log = log_bytes
            .map(|cap| ReceiptLog::open(&run_dir.join("receipts.jsonl"), cap))
            .transpose()?;
        Ok(Self {
            attempts: 0,
            complete: 0,
            failed: 0,
            unknown: 0,
            in_flight: 0,
            input: Total::default(),
            cached: Total::default(),
            output: Total::default(),
            dedup: ReceiptDedup::default(),
            log,
            log_cap: log_bytes.unwrap_or(0),
            log_written: 0,
            log_failed: false,
            refused: std::collections::BTreeMap::new(),
        })
    }

    /// Decided before any body is read. A log that cannot fit another
    /// <=16 KiB receipt plus its newline, a prior write failure and the
    /// attempt bound are all permanent for this run.
    fn admission(&self) -> Admission {
        let log_full =
            self.log.is_some() && self.log_written + RECEIPT_MAX_BYTES as u64 + 1 > self.log_cap;
        if self.log_failed || log_full {
            Admission::Closed
        } else if self.attempts >= MAX_ATTEMPTS {
            Admission::Full
        } else {
            Admission::Open
        }
    }

    /// The attempt is counted in memory BEFORE the upstream send, so a later
    /// logging failure keeps it.
    fn record_attempt(&mut self) {
        self.attempts += 1;
        self.in_flight += 1;
    }

    fn record_refusal(&mut self, code: &str) {
        *self.refused.entry(code.to_owned()).or_insert(0) += 1;
    }

    fn finalize(&mut self, receipt: &Receipt) {
        self.in_flight = self.in_flight.saturating_sub(1);
        // The key is a fresh UUID per attempt: a repeat or a conflict would
        // be an internal invariant break and must not double count.
        if !matches!(self.dedup.admit(receipt), Ok(true)) {
            return;
        }
        match receipt.outcome {
            Outcome::Complete => self.complete += 1,
            Outcome::Failed => self.failed += 1,
            Outcome::Unknown => self.unknown += 1,
        }
        self.input.add(receipt.input_tokens);
        self.cached.add(receipt.cached_input_tokens);
        self.output.add(receipt.output_tokens);
        if let Some(log) = self.log.as_mut() {
            match log.append(receipt) {
                Ok(()) => {
                    let line = serde_json::to_string(&receipt.to_json()).unwrap_or_default();
                    self.log_written += line.len() as u64 + 1;
                }
                // Counts stay in memory; the failure closes admission and is
                // recorded for the operator without any request content.
                Err(error) => {
                    self.log_failed = true;
                    eprintln!("{}", error.bounded_json());
                }
            }
        }
    }

    /// Whole-run coverage is unverified: a crash can lose an in-memory
    /// attempt entirely, and nothing here survives a restart.
    fn final_line(&self, session_id: &str) -> String {
        json!({
            "v": 1,
            "session_id": session_id,
            "attempts": self.attempts,
            "complete": self.complete,
            "failed": self.failed,
            "unknown": self.unknown + self.in_flight,
            "input_tokens": self.input.value,
            "cached_input_tokens": self.cached.value,
            "output_tokens": self.output.value,
            "totals_overflowed": self.input.overflowed
                || self.cached.overflowed
                || self.output.overflowed,
            "log_failed": self.log_failed,
            "refused": self.refused,
            "coverage": "whole_run_unverified",
        })
        .to_string()
    }
}

fn book_lock(book: &Mutex<Book>) -> std::sync::MutexGuard<'_, Book> {
    // The critical sections never panic and never span an `.await`; a
    // poisoned guard is recovered instead of unwrapped.
    book.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ---------------------------------------------------------------------------
// Shared server state
// ---------------------------------------------------------------------------

struct Shared {
    session_id: String,
    token: String,
    /// `Bearer <upstream key>`: the key is read once, marked sensitive and
    /// never printed, logged or echoed.
    upstream_auth: HeaderValue,
    client: reqwest::Client,
    upstream: String,
    port: u16,
    book: Mutex<Book>,
    /// The single generation slot: acquired after header authentication and
    /// held through body read, validation, upstream send, streaming and
    /// receipt finalization.
    slot: Arc<AsyncMutex<()>>,
    shutdown: CancellationToken,
    limits: Limits,
}

/// One admitted generation: the identity recorded before the upstream send
/// plus the generation slot, released last (after the receipt).
struct Attempt {
    id: String,
    started: Instant,
    context_ids: Vec<String>,
    _slot: OwnedMutexGuard<()>,
}

struct Finish {
    outcome: Outcome,
    delivery: Delivery,
    usage: Usage,
    provider_request_id: Option<String>,
    provider_response_id: Option<String>,
}

impl Finish {
    fn bare(outcome: Outcome, delivery: Delivery) -> Self {
        Self {
            outcome,
            delivery,
            usage: Usage::default(),
            provider_request_id: None,
            provider_response_id: None,
        }
    }
}

fn finalize(shared: &Shared, attempt: Attempt, finish: Finish) {
    // A cached subset larger than the reported input cannot be represented
    // by the strict receipt schema; it stays unknown rather than corrupting
    // the receipt.
    let cached = match (finish.usage.cached, finish.usage.input) {
        (Some(cached), Some(input)) if cached > input => None,
        (cached, _) => cached,
    };
    let receipt = Receipt {
        session_id: shared.session_id.clone(),
        request_id: attempt.id.clone(),
        adapter_id: ADAPTER_ID.to_owned(),
        model_id: MODEL_ID.to_owned(),
        context_ids: attempt.context_ids.clone(),
        input_tokens: finish.usage.input,
        output_tokens: finish.usage.output,
        cached_input_tokens: cached,
        cost_microunits: None,
        currency: None,
        cost_basis: None,
        ratecard_id: None,
        outcome: finish.outcome,
        observation: Some(Observation {
            mode: Mode::Meter,
            elapsed_ms: u64::try_from(attempt.started.elapsed().as_millis()).unwrap_or(u64::MAX),
            count_elapsed_ms: None,
            provider_request_id: finish.provider_request_id,
            provider_response_id: finish.provider_response_id,
            delivery: Some(finish.delivery),
        }),
    };
    book_lock(&shared.book).finalize(&receipt);
    // The slot is released only now, after the receipt is final.
    drop(attempt);
}

fn sanitize_provider_id(value: Option<&HeaderValue>) -> Option<String> {
    let raw = value?.to_str().ok()?;
    let allowed = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-');
    (!raw.is_empty() && raw.len() <= PROVIDER_ID_MAX_BYTES && raw.bytes().all(allowed))
        .then(|| raw.to_owned())
}

fn canonical_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value)
        .map(|id| id.get_version_num() == 4 && id.hyphenated().to_string() == value)
        .unwrap_or(false)
}

/// `X-Foundry-Context-Ids`: <=4 KiB, <=64 distinct canonical UUIDv4 values,
/// comma separated. `None` is invalid; an absent header is an empty list.
fn parse_context_ids(headers: &HeaderMap) -> Option<Vec<String>> {
    let mut values = headers.get_all("x-foundry-context-ids").iter();
    let Some(value) = values.next() else {
        return Some(Vec::new());
    };
    if values.next().is_some() {
        return None;
    }
    let text = value.to_str().ok()?;
    if text.len() > CONTEXT_IDS_MAX_BYTES {
        return None;
    }
    let ids: Vec<String> = text.split(',').map(|id| id.trim().to_owned()).collect();
    let mut distinct = ids.clone();
    distinct.sort();
    distinct.dedup();
    (ids.len() <= CONTEXT_IDS_MAX
        && distinct.len() == ids.len()
        && ids.iter().all(|id| canonical_uuid(id)))
    .then_some(ids)
}

/// Constant-time bearer comparison; a length difference also folds into the
/// accumulator so neither content nor length short-circuits.
fn bearer_ok(headers: &HeaderMap, token: &str) -> bool {
    let Some(supplied) = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
    else {
        return false;
    };
    let supplied = supplied.as_bytes();
    let expected = token.as_bytes();
    let mut diff = (supplied.len() ^ expected.len()) as u8;
    for index in 0..supplied.len().max(expected.len()) {
        diff |=
            supplied.get(index).copied().unwrap_or(0) ^ expected.get(index).copied().unwrap_or(0);
    }
    diff == 0
}

// ---------------------------------------------------------------------------
// Command entry: startup order is normative
// ---------------------------------------------------------------------------

fn emit(line: &str) {
    // A closed stdout must not abort the shutdown path.
    let _ = writeln!(std::io::stdout().lock(), "{line}");
}

fn read_config(path: &Path) -> AResult<GatewayConfig> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| {
            file.take(CONFIG_MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|e| invalid(&format!("cannot read the gateway config: {e}")))?;
    parse_config(&bytes)
}

/// Read the upstream key once and remove it from the process environment so
/// no child or later library can inherit it. Only the pre-built
/// `Authorization` value survives, marked sensitive.
fn take_credential(name: &str) -> AResult<HeaderValue> {
    let missing = || {
        AdapterError::named(
            "gateway_credential_missing",
            format!("credential environment variable `{name}` is not set or empty"),
        )
    };
    let key = std::env::var(name).map_err(|_| missing())?;
    if key.trim().is_empty() {
        return Err(missing());
    }
    // SAFETY: `remove_var` races only with other threads touching the
    // environment; this runs on the main thread before the tokio runtime
    // (and any other thread of this process) exists.
    unsafe { std::env::remove_var(name) };
    let mut value = HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| {
        AdapterError::named(
            "gateway_credential_missing",
            "the credential value is not a valid HTTP header value",
        )
    })?;
    value.set_sensitive(true);
    Ok(value)
}

/// Fresh owner-private run directory; an existing one is refused rather than
/// cleared, and the parent must already exist.
fn create_run_dir(path: &Path) -> AResult<()> {
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .map_err(|e| {
            invalid(&format!(
                "cannot create `run_dir` (it must not already exist): {e}"
            ))
        })
}

/// 256-bit local bearer token from the OS CSPRNG, lowercase hex (64 chars).
fn generate_token() -> AResult<String> {
    let mut raw = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut raw))
        .map_err(|e| AdapterError::runtime("gateway_unavailable", format!("/dev/urandom: {e}")))?;
    Ok(raw.iter().map(|b| format!("{b:02x}")).collect())
}

fn write_private_exclusive(path: &Path, bytes: &[u8]) -> AResult<()> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| {
            AdapterError::runtime(
                "gateway_unavailable",
                format!("cannot create run file: {e}"),
            )
        })?;
    file.write_all(bytes).map_err(|e| {
        AdapterError::runtime("gateway_unavailable", format!("cannot write run file: {e}"))
    })
}

/// Only this run's generated files; receipts are never touched.
fn remove_run_files(run_dir: &Path) {
    let _ = std::fs::remove_file(run_dir.join("gateway.json"));
    let _ = std::fs::remove_file(run_dir.join("token"));
}

/// A failed startup leaves nothing behind: the directory was created by this
/// run, so an empty receipts file and the directory itself go with it.
fn remove_failed_start(run_dir: &Path) {
    remove_run_files(run_dir);
    let _ = std::fs::remove_file(run_dir.join("receipts.jsonl"));
    let _ = std::fs::remove_dir(run_dir);
}

pub fn run(config_path: &Path) -> AResult<()> {
    let config = read_config(config_path)?;
    let upstream_auth = take_credential(&config.credential_env)?;
    create_run_dir(&config.run_dir)?;
    let token = match generate_token().and_then(|token| {
        write_private_exclusive(&config.run_dir.join("token"), token.as_bytes())?;
        Ok(token)
    }) {
        Ok(token) => token,
        Err(error) => {
            remove_failed_start(&config.run_dir);
            return Err(error);
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            remove_failed_start(&config.run_dir);
            return Err(AdapterError::runtime(
                "gateway_unavailable",
                error.to_string(),
            ));
        }
    };
    let session_id = uuid::Uuid::new_v4().to_string();
    let server = match runtime.block_on(start(&config, upstream_auth, token, session_id)) {
        Ok(server) => server,
        Err(error) => {
            remove_failed_start(&config.run_dir);
            runtime.shutdown_timeout(Duration::from_millis(500));
            return Err(error);
        }
    };
    runtime.block_on(server.serve());
    remove_run_files(&config.run_dir);
    // A bounded wait: a resolver thread stuck in a blocking call must not
    // push the exit past the shutdown deadline.
    runtime.shutdown_timeout(Duration::from_millis(500));
    Ok(())
}

struct Server {
    listener: TcpListener,
    shared: Arc<Shared>,
}

async fn start(
    config: &GatewayConfig,
    upstream_auth: HeaderValue,
    token: String,
    session_id: String,
) -> AResult<Server> {
    let (upstream, https_only) = upstream_target()?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .https_only(https_only)
        .build()
        .map_err(|e| AdapterError::runtime("gateway_unavailable", e.to_string()))?;
    let book = Book::new(&config.run_dir, config.log_bytes)?;
    let listener = TcpListener::bind(("127.0.0.1", config.port))
        .await
        .map_err(|e| invalid(&format!("cannot bind 127.0.0.1:{}: {e}", config.port)))?;
    let port = listener
        .local_addr()
        .map_err(|e| AdapterError::runtime("gateway_unavailable", e.to_string()))?
        .port();
    // Signals are registered before the ready line so a launcher that reads
    // it and signals at once is never missed.
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|e| AdapterError::runtime("gateway_unavailable", e.to_string()))?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .map_err(|e| AdapterError::runtime("gateway_unavailable", e.to_string()))?;
    let shared = Arc::new(Shared {
        session_id: session_id.clone(),
        token,
        upstream_auth,
        client,
        upstream,
        port,
        book: Mutex::new(book),
        slot: Arc::new(AsyncMutex::new(())),
        shutdown: CancellationToken::new(),
        limits: Limits::load(),
    });
    let shutdown = shared.shutdown.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = sigterm.recv() => {}
            _ = sigint.recv() => {}
        }
        shutdown.cancel();
    });

    let url = format!("http://127.0.0.1:{port}/v1");
    let token_file = config.run_dir.join("token");
    write_private_exclusive(
        &config.run_dir.join("gateway.json"),
        json!({
            "v": 1,
            "url": url,
            "port": port,
            "session_id": session_id,
            "token_file": token_file,
            "pid": std::process::id(),
        })
        .to_string()
        .as_bytes(),
    )?;
    // Names and paths only; the token is never printed.
    emit(
        &json!({
            "v": 1,
            "url": url,
            "port": port,
            "session_id": session_id,
            "run_dir": config.run_dir,
            "token_file": token_file,
            "token_env": TOKEN_ENV,
            "omp_models_yml": format!(
                "providers:\n  zai:\n    baseUrl: \"http://127.0.0.1:{port}/v1\"\n    apiKey: {TOKEN_ENV}\n"
            ),
        })
        .to_string(),
    );
    Ok(Server { listener, shared })
}

impl Server {
    async fn serve(self) {
        let Server { listener, shared } = self;
        // At most eight connections, decided at accept: an excess socket is
        // closed before a single byte is read.
        let permits = Arc::new(Semaphore::new(MAX_CONNECTIONS));
        let mut connections: JoinSet<()> = JoinSet::new();
        loop {
            tokio::select! {
                biased;
                _ = shared.shutdown.cancelled() => break,
                // Reap finished connections so a long run never accumulates results.
                Some(_) = connections.join_next(), if !connections.is_empty() => {}
                accepted = listener.accept() => match accepted {
                    Ok((stream, _peer)) => {
                        let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
                            drop(stream);
                            continue;
                        };
                        let shared = Arc::clone(&shared);
                        connections.spawn(async move {
                            let _permit = permit;
                            serve_connection(stream, shared).await;
                        });
                    }
                    Err(_) => sleep(Duration::from_millis(10)).await,
                },
            }
        }
        // Admission is closed (the token is cancelled). An active attempt
        // sees the same token, cancels its upstream work, records its
        // receipt and then releases the slot; acquiring the slot therefore
        // means no attempt is left. Waiting is bounded inside the five
        // seconds after the signal and extends no earlier deadline.
        let grace = Instant::now() + SHUTDOWN_GRACE;
        let _idle = timeout_at(grace, Arc::clone(&shared.slot).lock_owned()).await;
        connections.abort_all();
        let line = book_lock(&shared.book).final_line(&shared.session_id);
        emit(&line);
    }
}

async fn serve_connection(stream: TcpStream, shared: Arc<Shared>) {
    // One cancellation per socket: an attempt that must cut a stalled
    // delivery closes the connection here, reclaiming its permit without
    // waiting for the peer to drain (the body's abort error may otherwise
    // never be polled past a blocked write).
    let cut = CancellationToken::new();
    let service_cut = cut.clone();
    let service_shared = Arc::clone(&shared);
    let service = service_fn(move |request: hyper::Request<Incoming>| {
        let shared = Arc::clone(&service_shared);
        let cut = service_cut.clone();
        async move { Ok::<_, std::convert::Infallible>(handle(request, shared, cut).await) }
    });
    let connection = hyper::server::conn::http1::Builder::new()
        // Headers (and the read buffer) are bounded to 16 KiB, so oversized
        // headers fail before any body allocation.
        .max_buf_size(HEADER_MAX_BYTES)
        .timer(TokioTimer::new())
        .header_read_timeout(HEADER_READ_TIMEOUT)
        .serve_connection(TokioIo::new(stream), service);
    tokio::select! {
        _ = connection => {}
        _ = cut.cancelled() => {}
        _ = shared.shutdown.cancelled() => {}
    }
}

// ---------------------------------------------------------------------------
// HTTP handling
// ---------------------------------------------------------------------------

type ResponseBody = UnsyncBoxBody<Bytes, std::io::Error>;
type Response = hyper::Response<ResponseBody>;

fn full_body(bytes: Bytes) -> ResponseBody {
    Full::new(bytes)
        .map_err(|never: std::convert::Infallible| -> std::io::Error { match never {} })
        .boxed_unsync()
}

fn json_response(
    status: StatusCode,
    extra: &[(&'static str, &'static str)],
    body: &Value,
) -> Response {
    let mut builder = hyper::Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json");
    for (name, value) in extra {
        builder = builder.header(*name, *value);
    }
    builder
        .body(full_body(Bytes::from(body.to_string())))
        .unwrap_or_else(|_| {
            let mut response = hyper::Response::new(full_body(Bytes::new()));
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            response
        })
}

/// `{"error":{"code":..,"message":<fixed text>}}`: never request bodies,
/// upstream bodies, the local token or the upstream key.
fn refusal(status: StatusCode, code: &str, message: &str) -> Response {
    json_response(
        status,
        &[],
        &json!({"error": {"code": code, "message": message}}),
    )
}

impl Shared {
    /// A local refusal of a request that reached the gateway: counted by
    /// code in the summary and `/health`, never as a receipt (nothing was
    /// sent upstream, so there is no attempt to bill).
    fn refuse(&self, status: StatusCode, code: &str, message: &str) -> Response {
        book_lock(&self.book).record_refusal(code);
        refusal(status, code, message)
    }
}

/// The permanent admission refusals as a spec: the caller counts and builds
/// it through `Shared::refuse`. 403, not 5xx/408/429: OMP 18.6.0 retries
/// only those statuses, and these refusals are permanent for the run.
fn admission_refusal(
    shutting_down: bool,
    admission: &Admission,
) -> Option<(StatusCode, &'static str, &'static str)> {
    if shutting_down || matches!(admission, Admission::Closed) {
        Some((
            StatusCode::FORBIDDEN,
            "gateway_admission_closed",
            "gateway admission is closed",
        ))
    } else if matches!(admission, Admission::Full) {
        Some((
            StatusCode::FORBIDDEN,
            "session_full",
            "the session attempt bound was reached",
        ))
    } else {
        None
    }
}

enum Route {
    Chat,
    Health,
}

async fn handle(
    request: hyper::Request<Incoming>,
    shared: Arc<Shared>,
    // Cancelled by the attempt when the gateway must cut this connection
    // (delivery timeout or deadline): the socket closes promptly, so its
    // connection permit is reclaimed even if the peer never drains.
    connection: CancellationToken,
) -> Response {
    let started = Instant::now();
    let (parts, body) = request.into_parts();
    if parts.headers.contains_key(UPGRADE) {
        return shared.refuse(
            StatusCode::BAD_REQUEST,
            "gateway_feature_unsupported",
            "protocol upgrades are not supported",
        );
    }
    if parts.uri.query().is_some() {
        return shared.refuse(
            StatusCode::BAD_REQUEST,
            "invalid_argument",
            "query strings are not accepted",
        );
    }
    let route = match (&parts.method, parts.uri.path()) {
        (&Method::POST, "/v1/chat/completions") => Route::Chat,
        (&Method::GET, "/health") => Route::Health,
        (_, "/v1/chat/completions" | "/health") => {
            return shared.refuse(
                StatusCode::METHOD_NOT_ALLOWED,
                "gateway_feature_unsupported",
                "the method is not supported on this path",
            );
        }
        _ => {
            return shared.refuse(
                StatusCode::NOT_FOUND,
                "gateway_feature_unsupported",
                "only POST /v1/chat/completions and GET /health are exposed",
            );
        }
    };
    // Origin, then exact Host, then the bearer token: all before any body.
    if parts.headers.contains_key(ORIGIN) {
        return shared.refuse(
            StatusCode::FORBIDDEN,
            "gateway_forbidden_origin",
            "browser origins are not accepted",
        );
    }
    let host_ok = parts
        .headers
        .get(HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| host == format!("127.0.0.1:{}", shared.port));
    if !host_ok {
        return shared.refuse(
            StatusCode::FORBIDDEN,
            "gateway_bad_host",
            "host must be the exact bound loopback authority",
        );
    }
    if !bearer_ok(&parts.headers, &shared.token) {
        return shared.refuse(
            StatusCode::UNAUTHORIZED,
            "gateway_unauthorized",
            "missing or invalid bearer token",
        );
    }
    match route {
        Route::Health => health(&shared),
        Route::Chat => chat(parts.headers, body, shared, started, connection).await,
    }
}

fn health(shared: &Shared) -> Response {
    let (admission_open, attempts, refused) = {
        let book = book_lock(&shared.book);
        (
            matches!(book.admission(), Admission::Open),
            book.attempts,
            book.refused.clone(),
        )
    };
    let open = admission_open && !shared.shutdown.is_cancelled();
    json_response(
        StatusCode::OK,
        &[],
        &json!({
            "v": 1,
            "ready": true,
            "session_id": shared.session_id,
            "mode": "meter",
            "admission": if open { "open" } else { "closed" },
            "attempts": attempts,
            "refused": refused,
        }),
    )
}

/// Why a request could not take the generation slot.
enum SlotRefusal {
    Busy,
    Closed(StatusCode, &'static str, &'static str),
}

/// Permanent-refusal check, then the single generation slot, then the same
/// check again UNDER the slot. The first check keeps refusal ordering (a
/// closed gateway answers closed, not busy); the second is authoritative: a
/// receipt finalized between the first check and acquiring the slot may have
/// closed admission, and while this request holds the slot nothing can
/// change that answer again.
fn admit_slot(shared: &Shared) -> Result<OwnedMutexGuard<()>, SlotRefusal> {
    admit_slot_with(shared, || {})
}

/// `admit_slot` with a seam between the first check and taking the slot, so
/// a test can run another request's receipt finalization and slot release
/// at exactly the point where a stale snapshot would otherwise be used.
fn admit_slot_with(
    shared: &Shared,
    after_first_check: impl FnOnce(),
) -> Result<OwnedMutexGuard<()>, SlotRefusal> {
    let check = |shared: &Shared| {
        let admission = book_lock(&shared.book).admission();
        admission_refusal(shared.shutdown.is_cancelled(), &admission)
    };
    if let Some((status, code, message)) = check(shared) {
        return Err(SlotRefusal::Closed(status, code, message));
    }
    after_first_check();
    let slot = Arc::clone(&shared.slot)
        .try_lock_owned()
        .map_err(|_| SlotRefusal::Busy)?;
    match check(shared) {
        Some((status, code, message)) => Err(SlotRefusal::Closed(status, code, message)),
        None => Ok(slot),
    }
}

async fn chat(
    headers: HeaderMap,
    body: Incoming,
    shared: Arc<Shared>,
    started: Instant,
    connection: CancellationToken,
) -> Response {
    let slot = match admit_slot(&shared) {
        Ok(slot) => slot,
        Err(SlotRefusal::Closed(status, code, message)) => {
            return shared.refuse(status, code, message);
        }
        Err(SlotRefusal::Busy) => {
            // One active generation, zero queued; nothing is sent upstream
            // and nothing is billed. The marker is OMP 18.6.0's no-retry
            // signal.
            let mut response = shared.refuse(
                StatusCode::TOO_MANY_REQUESTS,
                "gateway_busy",
                "another generation is active",
            );
            response.headers_mut().insert(
                "rate_limit_type",
                HeaderValue::from_static("max_parallel_requests"),
            );
            return response;
        }
    };
    if headers.get(CONTENT_ENCODING).is_some_and(|value| {
        !value
            .to_str()
            .is_ok_and(|text| text.trim().eq_ignore_ascii_case("identity"))
    }) {
        return shared.refuse(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "gateway_feature_unsupported",
            "only identity content-encoding is supported",
        );
    }
    let json_content = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"))
        });
    if !json_content {
        return shared.refuse(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "invalid_argument",
            "content-type must be application/json",
        );
    }
    if headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > BODY_MAX_BYTES as u64)
    {
        return shared.refuse(
            StatusCode::PAYLOAD_TOO_LARGE,
            "request_too_large",
            "the request body exceeds the 4 MiB bound",
        );
    }
    let Some(context_ids) = parse_context_ids(&headers) else {
        return shared.refuse(
            StatusCode::BAD_REQUEST,
            "invalid_argument",
            "X-Foundry-Context-Ids must be <=64 distinct canonical UUIDv4 values",
        );
    };
    let deadline = started + shared.limits.deadline;
    let body = match read_body(body, &shared, deadline).await {
        Ok(body) => body,
        Err(refused) => return refused,
    };
    let parsed = match strict_parse(&body) {
        Ok(value) => value,
        Err(_) => {
            return shared.refuse(
                StatusCode::BAD_REQUEST,
                "invalid_argument",
                "the request body is not strict UTF-8 JSON",
            );
        }
    };
    if let Err(reason) = validate_request(&parsed) {
        return shared.refuse(
            StatusCode::BAD_REQUEST,
            "gateway_feature_unsupported",
            &format!("the request is outside the pinned OMP 18.6.0 profile: {reason}"),
        );
    }
    drop(parsed);
    // Re-check: shutdown may have closed admission while the body was read.
    if shared.shutdown.is_cancelled() {
        return shared.refuse(
            StatusCode::FORBIDDEN,
            "gateway_admission_closed",
            "gateway admission is closed",
        );
    }
    book_lock(&shared.book).record_attempt();
    let attempt = Attempt {
        id: uuid::Uuid::new_v4().to_string(),
        started,
        context_ids,
        _slot: slot,
    };
    let job = Job {
        body,
        user_agent: headers.get("user-agent").cloned(),
        deadline,
        connection,
    };
    // The attempt runs in its own task: a client that disconnects while the
    // upstream is being awaited drops this handler, never the attempt, so
    // every recorded attempt still ends in a receipt.
    let (head_tx, head_rx) = oneshot::channel();
    tokio::spawn(run_attempt(shared, attempt, job, head_tx));
    head_rx.await.unwrap_or_else(|_| {
        refusal(
            StatusCode::BAD_GATEWAY,
            "upstream_error",
            "the upstream attempt ended without a reply",
        )
    })
}

async fn read_body(
    mut body: Incoming,
    shared: &Shared,
    deadline: Instant,
) -> Result<Vec<u8>, Response> {
    let mut collected = Vec::new();
    loop {
        let wait = shared
            .limits
            .idle
            .min(deadline.saturating_duration_since(Instant::now()));
        let timed_out = || {
            shared.refuse(
                StatusCode::REQUEST_TIMEOUT,
                "deadline_exceeded",
                "the request body was not received in time",
            )
        };
        if wait.is_zero() {
            return Err(timed_out());
        }
        let frame = tokio::select! {
            biased;
            _ = shared.shutdown.cancelled() => {
                return Err(shared.refuse(
                    StatusCode::FORBIDDEN,
                    "gateway_admission_closed",
                    "gateway admission is closed",
                ));
            }
            frame = timeout(wait, body.frame()) => frame,
        };
        match frame {
            Err(_) => return Err(timed_out()),
            Ok(None) => return Ok(collected),
            Ok(Some(Err(_))) => {
                return Err(shared.refuse(
                    StatusCode::BAD_REQUEST,
                    "invalid_argument",
                    "the request body could not be read",
                ));
            }
            Ok(Some(Ok(frame))) => {
                if let Ok(data) = frame.into_data() {
                    collected.extend_from_slice(&data);
                    if collected.len() > BODY_MAX_BYTES {
                        return Err(shared.refuse(
                            StatusCode::PAYLOAD_TOO_LARGE,
                            "request_too_large",
                            "the request body exceeds the 4 MiB bound",
                        ));
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Upstream attempt
// ---------------------------------------------------------------------------

struct Job {
    body: Vec<u8>,
    user_agent: Option<HeaderValue>,
    deadline: Instant,
    connection: CancellationToken,
}

async fn discard_error_body(response: reqwest::Response, shared: &Shared, deadline: Instant) {
    let mut stream = response.bytes_stream().boxed();
    let drain = async {
        let mut drained = 0usize;
        while drained < UPSTREAM_ERROR_DRAIN_BYTES {
            match timeout(shared.limits.idle, stream.next()).await {
                Ok(Some(Ok(chunk))) => drained += chunk.len(),
                _ => break,
            }
        }
    };
    // Read at most 64 KiB and discard it: an upstream error body is never
    // forwarded, logged or embedded in a gateway error.
    tokio::select! {
        _ = shared.shutdown.cancelled() => {}
        _ = timeout_at(deadline, drain) => {}
    }
}

async fn run_attempt(
    shared: Arc<Shared>,
    attempt: Attempt,
    job: Job,
    mut head: oneshot::Sender<Response>,
) {
    let Job {
        body,
        user_agent,
        deadline,
        connection,
    } = job;
    // Exactly the pinned wire headers. Client authorization, Foundry
    // correlation headers, cookies and everything else are never forwarded.
    let mut request = shared
        .client
        .post(format!("{}/chat/completions", shared.upstream))
        .header("content-type", "application/json")
        .header("accept", "text/event-stream")
        .header("authorization", shared.upstream_auth.clone())
        .body(body);
    if let Some(agent) = user_agent {
        request = request.header("user-agent", agent);
    }
    // The connect/send/response-header phase is bounded by the idle limit
    // and the remaining absolute deadline, like every other wait.
    let phase_cap = deadline.min(Instant::now() + shared.limits.idle);
    let sent = tokio::select! {
        biased;
        _ = shared.shutdown.cancelled() => {
            finalize(&shared, attempt, Finish::bare(Outcome::Failed, Delivery::LocalFailure));
            return;
        }
        _ = head.closed() => {
            finalize(&shared, attempt, Finish::bare(Outcome::Unknown, Delivery::ClientClosed));
            return;
        }
        _ = sleep_until(phase_cap) => {
            finalize(&shared, attempt, Finish::bare(Outcome::Failed, Delivery::LocalFailure));
            let _ = head.send(refusal(
                StatusCode::GATEWAY_TIMEOUT,
                "deadline_exceeded",
                "the upstream stalled before its response headers",
            ));
            return;
        }
        sent = request.send() => sent,
    };
    let upstream = match sent {
        Ok(response) => response,
        Err(_) => {
            finalize(
                &shared,
                attempt,
                Finish::bare(Outcome::Failed, Delivery::LocalFailure),
            );
            let _ = head.send(refusal(
                StatusCode::BAD_GATEWAY,
                "upstream_error",
                "the upstream request failed",
            ));
            return;
        }
    };
    let status = upstream.status();
    let provider_request_id = sanitize_provider_id(upstream.headers().get("x-request-id"));
    if !status.is_success() {
        discard_error_body(upstream, &shared, deadline).await;
        // The ORIGINAL non-success status keeps OMP's retry semantics for
        // every class (3xx included); redirects are never followed and no
        // upstream header (Location least of all) is relayed.
        let relayed = status;
        let mut error = json!({
            "code": "upstream_error",
            "message": "the upstream returned a non-success status",
            "upstream_status": status.as_u16(),
        });
        if let Some(id) = &provider_request_id {
            error["provider_request_id"] = json!(id);
        }
        let delivery = if head.is_closed() {
            Delivery::ClientClosed
        } else {
            Delivery::Delivered
        };
        // Receipt first, slot released, then the reply: a host that retries
        // at once must not find the slot still held.
        finalize(
            &shared,
            attempt,
            Finish {
                outcome: Outcome::Failed,
                delivery,
                usage: Usage::default(),
                provider_request_id,
                provider_response_id: None,
            },
        );
        let _ = head.send(json_response(relayed, &[], &json!({"error": error})));
        return;
    }
    let event_stream = upstream
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("text/event-stream")
        });
    let encoded = upstream
        .headers()
        .get(CONTENT_ENCODING)
        .is_some_and(|value| {
            !value
                .to_str()
                .is_ok_and(|text| text.trim().eq_ignore_ascii_case("identity"))
        });
    if !event_stream || encoded {
        // Not the pinned SSE wire: forwarding it would hide a reply the
        // observer cannot meter.
        finalize(
            &shared,
            attempt,
            Finish {
                outcome: Outcome::Failed,
                delivery: Delivery::LocalFailure,
                usage: Usage::default(),
                provider_request_id,
                provider_response_id: None,
            },
        );
        let _ = head.send(json_response(
            StatusCode::BAD_GATEWAY,
            &[],
            &json!({"error": {
                "code": "upstream_error",
                "message": "the upstream reply is not the pinned event stream",
                "upstream_status": status.as_u16(),
            }}),
        ));
        return;
    }

    let (sender, receiver) = mpsc::channel(CHANNEL_CAPACITY);
    let (drained_tx, drained_rx) = oneshot::channel();
    let aborted = Arc::new(AtomicBool::new(false));
    let mut builder = hyper::Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream")
        .header("cache-control", "no-cache");
    // Only the allowlisted `x-request-id` response header is forwarded.
    if let Some(id) = provider_request_id
        .as_deref()
        .and_then(|id| HeaderValue::from_str(id).ok())
    {
        builder = builder.header("x-request-id", id);
    }
    let response = builder
        .body(
            ForwardBody {
                receiver,
                held: None,
                aborted: Arc::clone(&aborted),
                flushed_before_abort: false,
                drained: Some(drained_tx),
            }
            .boxed_unsync(),
        )
        .unwrap_or_else(|_| {
            refusal(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "the response could not be built",
            )
        });
    if head.send(response).is_err() {
        // The client left before any byte was sent: cancel upstream (the
        // stream is dropped with this task) and record unknown usage.
        finalize(
            &shared,
            attempt,
            Finish {
                outcome: Outcome::Unknown,
                delivery: Delivery::ClientClosed,
                usage: Usage::default(),
                provider_request_id,
                provider_response_id: None,
            },
        );
        return;
    }
    let end = pump(
        upstream.bytes_stream().boxed(),
        Forwarder {
            sender,
            credit: Arc::new(Semaphore::new(BACKPRESSURE_BYTES)),
            idle: shared.limits.idle,
            deadline,
            shutdown: shared.shutdown.clone(),
            seen: Seen::default(),
        },
        PumpControl {
            aborted,
            drained: drained_rx,
            shutdown: shared.shutdown.clone(),
            deadline,
            connection,
        },
    )
    .await;
    finalize(
        &shared,
        attempt,
        Finish {
            outcome: end.outcome,
            delivery: end.delivery,
            usage: end.usage,
            provider_request_id,
            provider_response_id: end.provider_response_id,
        },
    );
}

// ---------------------------------------------------------------------------
// SSE observation and forwarding
// ---------------------------------------------------------------------------

/// Incremental event splitter. Lines end LF, CRLF or CR; an event ends at a
/// blank line. The scan keeps its position between pushes, so byte-by-byte
/// delivery stays linear.
#[derive(Default)]
struct EventSplitter {
    buf: BytesMut,
    /// Start of the line being scanned; everything before it is complete
    /// non-blank lines of the current event.
    line_start: usize,
    pos: usize,
}

impl EventSplitter {
    fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    fn buffered(&self) -> usize {
        self.buf.len()
    }

    /// The next complete event through its blank line. A trailing CR may be
    /// the first half of CRLF, so it waits for the next byte unless `eof`.
    fn next_event(&mut self, eof: bool) -> Option<Bytes> {
        while self.pos < self.buf.len() {
            let (content_end, next) = match self.buf[self.pos] {
                b'\n' => (self.pos, self.pos + 1),
                b'\r' => {
                    if self.pos + 1 < self.buf.len() {
                        let width = if self.buf[self.pos + 1] == b'\n' {
                            2
                        } else {
                            1
                        };
                        (self.pos, self.pos + width)
                    } else if eof {
                        (self.pos, self.pos + 1)
                    } else {
                        return None;
                    }
                }
                _ => {
                    self.pos += 1;
                    continue;
                }
            };
            if content_end == self.line_start {
                let event = self.buf.split_to(next).freeze();
                self.line_start = 0;
                self.pos = 0;
                return Some(event);
            }
            self.line_start = next;
            self.pos = next;
        }
        None
    }
}

/// Visit each line of a complete event (LF, CRLF or CR terminated).
fn for_each_line(event: &[u8], mut visit: impl FnMut(&[u8])) {
    let mut start = 0;
    let mut index = 0;
    while index < event.len() {
        let (end, next) = match event[index] {
            b'\n' => (index, index + 1),
            b'\r' if event.get(index + 1) == Some(&b'\n') => (index, index + 2),
            b'\r' => (index, index + 1),
            _ => {
                index += 1;
                continue;
            }
        };
        visit(&event[start..end]);
        start = next;
        index = next;
    }
}

/// What a complete event contains, judged line by line.
enum EventFields {
    /// Every line is blank, a comment or a standard SSE field; carries the
    /// `data` values joined with LF (the SSE field rule), `None` without a
    /// data line.
    Allowed(Option<Vec<u8>>),
    /// At least one line names a field outside `data`/`event`/`id`/`retry`.
    /// A stray byte (a mid-event BOM, say) can turn a `data:` line into an
    /// unknown field a lenient client then skips or reads differently, so
    /// the whole event is judged unforwardable.
    Foreign,
}

/// Judge EVERY line of the event, including events that also carry valid
/// `data:` lines: a line without a colon is a field name with an empty
/// value, a leading `:` is a comment, and one optional space after the
/// colon is not part of the value.
fn event_fields(event: &[u8]) -> EventFields {
    let mut data: Option<Vec<u8>> = None;
    let mut foreign = false;
    for_each_line(event, |line| {
        if line.is_empty() || line.starts_with(b":") {
            return;
        }
        let (name, value) = match line.iter().position(|byte| *byte == b':') {
            Some(colon) => {
                let value = &line[colon + 1..];
                (&line[..colon], value.strip_prefix(b" ").unwrap_or(value))
            }
            None => (line, &line[line.len()..]),
        };
        match name {
            b"data" => match data.as_mut() {
                Some(joined) => {
                    joined.push(b'\n');
                    joined.extend_from_slice(value);
                }
                None => data = Some(value.to_vec()),
            },
            b"event" | b"id" | b"retry" => {}
            _ => foreign = true,
        }
    });
    if foreign {
        EventFields::Foreign
    } else {
        EventFields::Allowed(data)
    }
}

/// Usage from top-level `usage`, else `choices[0].usage`. The reported
/// `prompt_tokens` is the normalized input (it includes cached input), the
/// cached subset stays unknown when absent, and nothing is counted from
/// deltas.
fn usage_from(chunk: &Value) -> Option<Usage> {
    let usage = chunk
        .get("usage")
        .filter(|value| value.is_object())
        .or_else(|| {
            chunk
                .get("choices")?
                .as_array()?
                .first()?
                .get("usage")
                .filter(|value| value.is_object())
        })?;
    let input = usage.get("prompt_tokens").and_then(Value::as_u64);
    let cached = usage
        .get("prompt_tokens_details")
        .and_then(|details| details.get("cached_tokens"))
        .and_then(Value::as_u64);
    let output = usage.get("completion_tokens").and_then(Value::as_u64);
    (input.is_some() || cached.is_some() || output.is_some()).then_some(Usage {
        input,
        cached,
        output,
    })
}

#[derive(Default)]
struct Seen {
    /// Whether a non-null `choices[0].finish_reason` was observed. Terminal
    /// usage requires it: finish-bearing usage, or usage arriving after the
    /// finish. Provisional mid-delta usage is never promoted to completion.
    finish_seen: bool,
    /// Terminal usage only (the final usage object is authoritative).
    usage: Option<Usage>,
    response_id: Option<String>,
}

enum Observed {
    Plain,
    Done,
    UpstreamError,
}

/// A stream-leading UTF-8 BOM is valid SSE framing (WHATWG §9.2.5). It is
/// stripped here for OBSERVATION only: the original bytes, including a BOM
/// split across reads, are still forwarded unchanged.
const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";

/// Observe one complete event. Every line must be blank, a comment or a
/// standard SSE field; a forwarded data event must be `[DONE]` or a strict
/// JSON object (duplicate keys refuse, as for requests) carrying a
/// `choices` array (a trailing usage-only `choices: []` chunk included). A
/// non-null top-level `error`, unparsable data, a duplicate key, a foreign
/// field or any other shape is an upstream stream failure and the event is
/// withheld. `error: null` beside `choices` is a normal chunk, and events
/// without a `data:` line forward unchanged.
fn observe(event: &[u8], seen: &mut Seen, stream_start: bool) -> Observed {
    let observed = if stream_start {
        event.strip_prefix(UTF8_BOM).unwrap_or(event)
    } else {
        event
    };
    let data = match event_fields(observed) {
        EventFields::Foreign => return Observed::UpstreamError,
        EventFields::Allowed(None) => return Observed::Plain,
        EventFields::Allowed(Some(data)) => data,
    };
    if data.as_slice() == b"[DONE]" {
        return Observed::Done;
    }
    // The same strict parser the request side uses: serde_json's own
    // `Value` keeps the LAST duplicate key, so an `error` hidden before a
    // later `"error":null` would pass unjudged.
    let Ok(chunk) = strict_parse(&data) else {
        return Observed::UpstreamError;
    };
    if chunk.get("error").is_some_and(|error| !error.is_null()) {
        return Observed::UpstreamError;
    }
    if chunk
        .get("choices")
        .is_none_or(|choices| !choices.is_array())
    {
        return Observed::UpstreamError;
    }
    if seen.response_id.is_none()
        && let Some(id) = chunk.get("id").and_then(Value::as_str)
        && !id.trim().is_empty()
        && id.len() <= 256
    {
        seen.response_id = Some(id.to_owned());
    }
    let finished_before = seen.finish_seen;
    let finish = chunk["choices"]
        .as_array()
        .filter(|choices| !choices.is_empty())
        .and_then(|choices| choices[0].get("finish_reason"))
        .is_some_and(|reason| !reason.is_null());
    if finish {
        seen.finish_seen = true;
    }
    if let Some(usage) = usage_from(&chunk)
        && (finish || finished_before)
    {
        seen.usage = Some(usage);
    }
    Observed::Plain
}

type Item = (Bytes, OwnedSemaphorePermit);

/// The response body: a bounded channel with byte accounting. A permit is
/// returned only when hyper asks for the next frame, i.e. after the previous
/// one was consumed, so unread bytes toward the client stay bounded.
struct ForwardBody {
    receiver: mpsc::Receiver<Item>,
    held: Option<OwnedSemaphorePermit>,
    /// Set before the sender drops when the gateway cut the stream: the body
    /// then ends with an error, so hyper aborts the chunked reply instead of
    /// terminating it cleanly (never a forged completion).
    aborted: Arc<AtomicBool>,
    /// hyper drops its unflushed write buffer when a body errors on the very
    /// next poll, which would lose frames the host was already owed. One
    /// self-woken `Pending` lets the dispatcher flush them before the error.
    flushed_before_abort: bool,
    /// Signalled when the host consumed the clean end of the stream.
    drained: Option<oneshot::Sender<()>>,
}

impl HttpBody for ForwardBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, std::io::Error>>> {
        let this = self.get_mut();
        match this.receiver.poll_recv(cx) {
            Poll::Ready(Some((bytes, permit))) => {
                this.held = Some(permit);
                Poll::Ready(Some(Ok(Frame::data(bytes))))
            }
            Poll::Ready(None) => {
                this.held = None;
                if this.aborted.load(Ordering::SeqCst) {
                    if !this.flushed_before_abort {
                        this.flushed_before_abort = true;
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                    // The one flush opportunity has passed: tell the cut
                    // watcher the body ended (hyper closes the socket itself).
                    if let Some(done) = this.drained.take() {
                        let _ = done.send(());
                    }
                    return Poll::Ready(Some(Err(std::io::Error::other(
                        "stream aborted by the gateway",
                    ))));
                }
                if let Some(done) = this.drained.take() {
                    let _ = done.send(());
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

enum Cause {
    Done,
    UpstreamEof,
    UpstreamError,
    ClientClosed,
    LocalFailure,
}

struct Forwarder {
    sender: mpsc::Sender<Item>,
    credit: Arc<Semaphore>,
    idle: Duration,
    /// The whole-attempt deadline: every forwarding wait is bounded by it,
    /// never only by a fresh idle window.
    deadline: Instant,
    shutdown: CancellationToken,
    seen: Seen,
}

impl Forwarder {
    /// Observe one COMPLETE event, then forward its raw bytes. `Some` ends
    /// the stream: an upstream error event is withheld from the host.
    async fn deliver(&mut self, event: Bytes, stream_start: bool) -> Option<Cause> {
        if event.len() > EVENT_MAX_BYTES {
            return Some(Cause::LocalFailure);
        }
        let observed = observe(&event, &mut self.seen, stream_start);
        if matches!(observed, Observed::UpstreamError) {
            return Some(Cause::UpstreamError);
        }
        if let Err(cause) = self.push(event).await {
            return Some(cause);
        }
        matches!(observed, Observed::Done).then_some(Cause::Done)
    }

    async fn push(&self, mut event: Bytes) -> Result<(), Cause> {
        while !event.is_empty() {
            let size = event.len().min(FORWARD_CHUNK_BYTES);
            let chunk = event.split_to(size);
            // A stalled client must not hold the slot past a shutdown.
            tokio::select! {
                biased;
                _ = self.shutdown.cancelled() => return Err(Cause::LocalFailure),
                queued = self.queue(chunk, size) => queued?,
            }
        }
        Ok(())
    }

    /// A client that does not drain within the idle timeout, or an attempt
    /// that reaches its deadline, is a local failure; byte credit is the
    /// real bound on unread data.
    async fn queue(&self, chunk: Bytes, size: usize) -> Result<(), Cause> {
        let cap = self.deadline.min(Instant::now() + self.idle);
        let permit = match timeout_at(
            cap,
            Arc::clone(&self.credit).acquire_many_owned(size as u32),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) | Err(_) => return Err(Cause::LocalFailure),
        };
        match timeout_at(cap, self.sender.send((chunk, permit))).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(Cause::ClientClosed),
            Err(_) => Err(Cause::LocalFailure),
        }
    }
}

struct PumpControl {
    aborted: Arc<AtomicBool>,
    drained: oneshot::Receiver<()>,
    shutdown: CancellationToken,
    deadline: Instant,
    /// The owning socket's cut token (see `serve_connection`).
    connection: CancellationToken,
}

/// How long a cut response may keep flushing already-forwarded frames to a
/// peer before the socket is closed regardless.
const CUT_FLUSH_GRACE: Duration = Duration::from_secs(1);

/// A cut response keeps flushing its already-forwarded frames for a bounded
/// grace; if the body has not reported its abort by then (a peer that stopped
/// draining leaves hyper blocked on the socket write), the socket is closed
/// so its connection permit comes back. Detached, so the attempt's receipt
/// and slot release never wait on a stalled peer.
fn cut_after_flush(drained: oneshot::Receiver<()>, connection: CancellationToken, grace: Duration) {
    tokio::spawn(async move {
        if timeout(grace, drained).await.is_err() {
            connection.cancel();
        }
    });
}

struct StreamEnd {
    outcome: Outcome,
    delivery: Delivery,
    usage: Usage,
    provider_response_id: Option<String>,
}

async fn pump(
    mut upstream: BoxStream<'static, reqwest::Result<Bytes>>,
    mut forwarder: Forwarder,
    control: PumpControl,
) -> StreamEnd {
    let mut splitter = EventSplitter::default();
    let mut received = 0u64;
    let idle = forwarder.idle;
    // The first complete event is where a stream-leading BOM can sit.
    let mut stream_start = true;
    let cause = 'stream: loop {
        while let Some(event) = splitter.next_event(false) {
            let at_start = stream_start;
            stream_start = false;
            if let Some(cause) = forwarder.deliver(event, at_start).await {
                break 'stream cause;
            }
        }
        if splitter.buffered() > EVENT_MAX_BYTES {
            break Cause::LocalFailure;
        }
        let wait = idle.min(control.deadline.saturating_duration_since(Instant::now()));
        if wait.is_zero() {
            break Cause::LocalFailure;
        }
        let next = tokio::select! {
            biased;
            _ = control.shutdown.cancelled() => break Cause::LocalFailure,
            // The host went away: stop reading (and so cancel) the upstream.
            _ = forwarder.sender.closed() => break Cause::ClientClosed,
            next = timeout(wait, upstream.next()) => next,
        };
        match next {
            Err(_) | Ok(Some(Err(_))) => break Cause::LocalFailure,
            Ok(None) => {
                // A trailing CR ends the last line; an unterminated tail is
                // an incomplete event and is never forwarded.
                while let Some(event) = splitter.next_event(true) {
                    let at_start = stream_start;
                    stream_start = false;
                    if let Some(cause) = forwarder.deliver(event, at_start).await {
                        break 'stream cause;
                    }
                }
                break Cause::UpstreamEof;
            }
            Ok(Some(Ok(chunk))) => {
                received += chunk.len() as u64;
                if received > RESPONSE_MAX_BYTES {
                    break Cause::LocalFailure;
                }
                splitter.push(&chunk);
            }
        }
    };
    // Stop upstream work before waiting on the host.
    drop(upstream);
    let Forwarder { sender, seen, .. } = forwarder;
    let usage_seen = seen.usage.is_some();
    let (outcome, delivery) = match cause {
        Cause::Done | Cause::UpstreamEof => {
            drop(sender);
            // The clean end counts as delivered once the host consumed it;
            // the wait is bounded by the attempt deadline, not just idle.
            let drain_cap = control.deadline.min(Instant::now() + idle);
            let delivery = tokio::select! {
                _ = control.shutdown.cancelled() => Delivery::LocalFailure,
                drained = timeout_at(drain_cap, control.drained) => match drained {
                    Ok(Ok(())) => Delivery::Delivered,
                    Ok(Err(_)) => Delivery::ClientClosed,
                    // The host did not drain the clean end in time: close
                    // the socket now, exactly as the failure branches do, so
                    // a stalled peer cannot hold its connection permit. The
                    // terminal usage already observed is kept below.
                    Err(_) => {
                        control.connection.cancel();
                        Delivery::LocalFailure
                    }
                },
            };
            let outcome = if usage_seen {
                Outcome::Complete
            } else {
                Outcome::Unknown
            };
            (outcome, delivery)
        }
        // The provider's own failure, withheld from the host and recorded
        // as failed; terminal usage seen earlier is kept in the counts.
        Cause::UpstreamError => {
            control.aborted.store(true, Ordering::SeqCst);
            drop(sender);
            cut_after_flush(
                control.drained,
                control.connection,
                CUT_FLUSH_GRACE.min(idle),
            );
            (Outcome::Failed, Delivery::Delivered)
        }
        Cause::LocalFailure => {
            control.aborted.store(true, Ordering::SeqCst);
            drop(sender);
            cut_after_flush(
                control.drained,
                control.connection,
                CUT_FLUSH_GRACE.min(idle),
            );
            let outcome = if usage_seen {
                Outcome::Complete
            } else {
                Outcome::Failed
            };
            (outcome, Delivery::LocalFailure)
        }
        Cause::ClientClosed => {
            let outcome = if usage_seen {
                Outcome::Complete
            } else {
                Outcome::Unknown
            };
            (outcome, Delivery::ClientClosed)
        }
    };
    StreamEnd {
        outcome,
        delivery,
        usage: seen.usage.unwrap_or_default(),
        provider_response_id: seen.response_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TURN: &str = include_str!("../tests/fixtures/gateway/omp-18.6.0-turn.json");
    const TOOL: &str = include_str!("../tests/fixtures/gateway/omp-18.6.0-tool-continuation.json");
    const JUDGE: &str = include_str!("../tests/fixtures/gateway/omp-18.6.0-auto-judge.json");

    fn parsed(text: &str) -> Value {
        strict_parse(text.as_bytes()).unwrap()
    }

    fn nested(levels: usize) -> String {
        format!("{}1{}", "{\"x\":".repeat(levels), "}".repeat(levels))
    }

    #[test]
    fn strict_parse_rejects_duplicates_trailing_data_and_depth_over_64() {
        assert!(strict_parse(br#"{"a":1,"a":1}"#).is_err());
        assert!(strict_parse(br#"{"a":{"b":1,"b":2}}"#).is_err());
        assert!(strict_parse(br#"{"a":1} {"b":2}"#).is_err());
        assert!(strict_parse(b"{\"a\":\"\xff\"}").is_err(), "invalid UTF-8");
        assert!(strict_parse(nested(64).as_bytes()).is_ok(), "depth 64");
        assert!(strict_parse(nested(65).as_bytes()).is_err(), "depth 65");
        assert!(strict_parse(b"[[[[1]]]]").is_ok());
    }

    #[test]
    fn pinned_fixtures_are_inside_the_request_profile() {
        for fixture in [TURN, TOOL, JUDGE] {
            validate_request(&parsed(fixture)).unwrap();
        }
    }

    #[test]
    fn profile_refuses_everything_outside_the_pin() {
        let refuse = |edit: &dyn Fn(&mut Value)| {
            let mut value = parsed(TOOL);
            edit(&mut value);
            assert!(validate_request(&value).is_err());
        };
        refuse(&|v| v["extra"] = json!(1));
        refuse(&|v| {
            v["messages"][2]["content"] = json!([{"type": "image_url", "image_url": {"url": "x"}}]);
        });
        refuse(&|v| v["model"] = json!("glm-5.3"));
        refuse(&|v| v["stream"] = json!(false));
        refuse(&|v| {
            v.as_object_mut().unwrap().remove("stream");
        });
        refuse(&|v| v["max_tokens"] = json!(0));
        refuse(&|v| v["max_tokens"] = json!(131_073));
        refuse(&|v| v["reasoning_effort"] = json!("medium"));
        refuse(&|v| v["stream_options"] = json!({"include_usage": true, "x": 1}));
        refuse(&|v| v["thinking"] = json!({"type": "auto"}));
        refuse(&|v| v["tool_choice"] = json!("sometimes"));
        refuse(&|v| v["messages"][3]["name"] = json!("n"));
        refuse(&|v| {
            let assistant = v["messages"][3].as_object_mut().unwrap();
            assistant.remove("content");
            assistant.remove("tool_calls");
        });
        refuse(&|v| v["messages"][4]["tool_call_id"] = json!(7));
        refuse(&|v| v["tools"][0]["function"]["extra"] = json!(1));
        refuse(&|v| v["messages"] = json!([]));
    }

    #[test]
    fn profile_accepts_the_documented_optional_shapes() {
        let mut value = parsed(TURN);
        value["temperature"] = json!(0.2);
        value["top_p"] = json!(1);
        value["tool_stream"] = json!(true);
        value["thinking"] = json!({"type": "enabled"});
        value["tool_choice"] = json!("auto");
        value["messages"] = json!([
            {"role": "user", "content": "hi"},
            {"role": "assistant", "content": null, "tool_calls": [
                {"id": "c1", "type": "function", "function": {"name": "f", "arguments": "{}"}}
            ]},
            {"role": "assistant"},
        ]);
        // The last assistant message has neither content nor tool_calls.
        assert!(validate_request(&value).is_err());
        value["messages"][2] = json!({"role": "assistant", "content": ""});
        validate_request(&value).unwrap();
    }

    fn config(extra: &str) -> Vec<u8> {
        format!(
            r#"{{"v":1,"port":0,"upstream":"https://api.z.ai/api/coding/paas/v4","model":"glm-5.3-flash","mode":"meter","credential_env":"ZAI_KEY","run_dir":"/tmp/gw"{extra}}}"#
        )
        .into_bytes()
    }

    #[test]
    fn config_accepts_the_pinned_profile_and_refuses_everything_else() {
        let parsed = parse_config(&config("")).unwrap();
        assert_eq!(parsed.port, 0);
        assert_eq!(parsed.log_bytes, None);
        assert_eq!(
            parse_config(&config(r#","log_bytes":65536"#))
                .unwrap()
                .log_bytes,
            Some(65_536)
        );
        let code = |extra: &str| parse_config(&config(extra)).err().unwrap().code();
        assert_eq!(code(r#","limits":{}"#), "gateway_feature_unsupported");
        assert_eq!(code(r#","log_bytes":65535"#), "invalid_argument");
        assert_eq!(code(r#","log_bytes":16777217"#), "invalid_argument");
        assert_eq!(code(r#","log_bytes":null"#), "invalid_argument");
        assert_eq!(code(r#","extra":1"#), "invalid_argument");
        let swapped = |from: &str, to: &str| {
            let text = String::from_utf8(config("")).unwrap().replace(from, to);
            parse_config(text.as_bytes()).err().unwrap().code()
        };
        assert_eq!(
            swapped("\"meter\"", "\"enforce\""),
            "gateway_feature_unsupported"
        );
        assert_eq!(swapped("\"meter\"", "\"other\""), "invalid_argument");
        assert_eq!(
            swapped("glm-5.3-flash", "glm-5.3"),
            "gateway_feature_unsupported"
        );
        assert_eq!(swapped("paas/v4", "paas/v5"), "gateway_feature_unsupported");
        assert_eq!(swapped("\"v\":1", "\"v\":2"), "invalid_argument");
        assert_eq!(swapped("\"port\":0", "\"port\":65536"), "invalid_argument");
        assert_eq!(swapped("/tmp/gw", "relative/gw"), "invalid_argument");
        assert_eq!(swapped("ZAI_KEY", "1BAD"), "invalid_argument");
    }

    fn receipt(index: u64, input: Option<u64>) -> Receipt {
        Receipt {
            session_id: "s".into(),
            request_id: format!("req-{index}"),
            adapter_id: ADAPTER_ID.into(),
            model_id: MODEL_ID.into(),
            context_ids: Vec::new(),
            input_tokens: input,
            output_tokens: None,
            cached_input_tokens: None,
            cost_microunits: None,
            currency: None,
            cost_basis: None,
            ratecard_id: None,
            outcome: Outcome::Complete,
            observation: None,
        }
    }

    #[test]
    fn the_ten_thousandth_attempt_closes_admission_as_session_full() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = Book::new(dir.path(), None).unwrap();
        for index in 0..MAX_ATTEMPTS {
            assert!(matches!(book.admission(), Admission::Open));
            book.record_attempt();
            book.finalize(&receipt(index, Some(1)));
        }
        assert!(matches!(book.admission(), Admission::Full));
        assert_eq!(book.attempts, MAX_ATTEMPTS);
        assert_eq!(book.input.value, Some(MAX_ATTEMPTS));
        let (status, code, _) = admission_refusal(false, &book.admission()).unwrap();
        assert_eq!((status, code), (StatusCode::FORBIDDEN, "session_full"));
        assert!(admission_refusal(false, &Admission::Open).is_none());
        assert!(
            admission_refusal(true, &Admission::Open).is_some(),
            "shutdown closes admission"
        );
    }

    fn test_shared(dir: &Path, log_bytes: Option<u64>) -> Shared {
        Shared {
            session_id: "s".into(),
            token: "t".into(),
            upstream_auth: HeaderValue::from_static("Bearer test"),
            client: reqwest::Client::new(),
            upstream: "http://127.0.0.1:1".into(),
            port: 1,
            book: Mutex::new(Book::new(dir, log_bytes).unwrap()),
            slot: Arc::new(AsyncMutex::new(())),
            shutdown: CancellationToken::new(),
            limits: Limits::load(),
        }
    }

    /// The constructed interleaving, with a deterministic barrier. A's first
    /// check passes while B runs and A is paused right after it; B then
    /// finalizes the receipt that closes admission and releases the slot; A
    /// resumes, takes the slot with its now-stale snapshot and must be
    /// refused by the SECOND (slot-protected) check. Without that recheck A
    /// would be admitted and a send beyond the closed log would follow.
    #[test]
    fn admission_is_rechecked_under_the_slot() {
        let dir = tempfile::tempdir().unwrap();
        let shared = test_shared(dir.path(), Some(LOG_MIN_BYTES));
        // One receipt away from a log that cannot fit another receipt.
        book_lock(&shared.book).log_written = LOG_MIN_BYTES - RECEIPT_MAX_BYTES as u64 - 1;
        let b_slot = admit_slot(&shared).ok().expect("B is admitted");
        book_lock(&shared.book).record_attempt();

        let shared_ref = &shared;
        let result = admit_slot_with(&shared, move || {
            // A's first check has just passed on an open gateway.
            assert!(matches!(
                book_lock(&shared_ref.book).admission(),
                Admission::Open
            ));
            // B finalizes a receipt that fills the log, then releases the slot.
            book_lock(&shared_ref.book).finalize(&receipt(1, Some(5)));
            drop(b_slot);
            assert!(matches!(
                book_lock(&shared_ref.book).admission(),
                Admission::Closed
            ));
        });
        match result {
            Err(SlotRefusal::Closed(status, code, _)) => {
                assert_eq!(
                    (status, code),
                    (StatusCode::FORBIDDEN, "gateway_admission_closed")
                );
            }
            Err(SlotRefusal::Busy) => panic!("the slot was free: A must see the closed log"),
            Ok(_) => panic!("A was admitted on a stale snapshot"),
        }
        // The refused request left the slot free for the next one.
        assert!(shared.slot.try_lock().is_ok());
    }

    /// The same barrier around attempt 10,000: B records the bound-reaching
    /// attempt after A's first check, and the slot-protected recheck refuses
    /// A (attempt 10,001) as `session_full`.
    #[test]
    fn the_attempt_bound_is_rechecked_under_the_slot() {
        let dir = tempfile::tempdir().unwrap();
        let shared = test_shared(dir.path(), None);
        book_lock(&shared.book).attempts = MAX_ATTEMPTS - 1;
        let b_slot = admit_slot(&shared)
            .ok()
            .expect("attempt 10,000 is admitted");

        let shared_ref = &shared;
        let result = admit_slot_with(&shared, move || {
            assert!(matches!(
                book_lock(&shared_ref.book).admission(),
                Admission::Open
            ));
            book_lock(&shared_ref.book).record_attempt();
            drop(b_slot);
        });
        match result {
            Err(SlotRefusal::Closed(status, code, _)) => {
                assert_eq!((status, code), (StatusCode::FORBIDDEN, "session_full"));
            }
            Err(SlotRefusal::Busy) => panic!("the slot was free: A must see the bound"),
            Ok(_) => panic!("attempt 10,001 was admitted on a stale snapshot"),
        }
    }

    #[test]
    fn a_repeated_receipt_key_counts_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = Book::new(dir.path(), None).unwrap();
        book.record_attempt();
        book.finalize(&receipt(1, Some(5)));
        book.finalize(&receipt(1, Some(5)));
        assert_eq!(book.complete, 1);
        assert_eq!(book.input.value, Some(5));
    }

    #[test]
    fn totals_use_checked_arithmetic() {
        let mut total = Total::default();
        total.add(Some(u64::MAX));
        assert_eq!(total.value, Some(u64::MAX));
        total.add(Some(1));
        assert!(
            total.value.is_none() && total.overflowed,
            "never wraps or saturates"
        );
        total.add(Some(1));
        assert!(total.value.is_none());
    }

    #[test]
    fn a_log_that_cannot_fit_another_receipt_closes_admission() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = Book::new(dir.path(), Some(LOG_MIN_BYTES)).unwrap();
        let mut count = 0u64;
        while matches!(book.admission(), Admission::Open) {
            book.record_attempt();
            book.finalize(&receipt(count, Some(10)));
            count += 1;
            assert!(
                count < MAX_ATTEMPTS,
                "a 64 KiB log cannot hold 10,000 receipts"
            );
        }
        assert!(matches!(book.admission(), Admission::Closed));
        let written = std::fs::metadata(dir.path().join("receipts.jsonl"))
            .unwrap()
            .len();
        assert!(written + RECEIPT_MAX_BYTES as u64 + 1 > LOG_MIN_BYTES);
        assert!(written <= LOG_MIN_BYTES);
        // Counts survive and the log stays valid receipts.
        assert_eq!(book.complete, count);
    }

    /// The joined `data` value of an event whose every line is allowed.
    fn event_data(event: &[u8]) -> Option<Vec<u8>> {
        match event_fields(event) {
            EventFields::Allowed(data) => data,
            EventFields::Foreign => panic!("unexpected foreign field in a test event"),
        }
    }

    #[test]
    fn every_line_of_an_event_is_judged_by_its_field_name() {
        let foreign = |event: &[u8]| matches!(event_fields(event), EventFields::Foreign);
        // Standard fields, comments, blank lines and colon-less names pass.
        for allowed in [
            &b"data: x\n\n"[..],
            b"event: message\nid: 7\nretry: 5\ndata: x\n\n",
            b": comment\ndata: x\n\n",
            b"data\n\n",
            b"data:no-space\n\n",
        ] {
            assert!(!foreign(allowed), "{:?}", String::from_utf8_lossy(allowed));
        }
        // A valid data line does not excuse a later foreign one.
        assert!(foreign(
            b"data: {\"choices\":[]}\n\xef\xbb\xbfdata: {\"error\":{}}\n\n"
        ));
        assert!(foreign(b"data: x\nunknown: y\n\n"));
        assert!(
            foreign(b"data: x\nDATA: y\n\n"),
            "field names are case-sensitive"
        );
        assert!(foreign(b" data: x\n\n"));
        // The data rule: one optional space, LF-joined lines, colon-less is empty.
        assert_eq!(
            event_data(b"data:  two-spaces\n\n").unwrap(),
            b" two-spaces"
        );
        assert_eq!(event_data(b"data\ndata: x\n\n").unwrap(), b"\nx");
    }

    #[test]
    fn duplicate_keys_and_split_event_bom_withhold_the_event() {
        let mut seen = Seen::default();
        let hidden_error = br#"data: {"error":{"message":"CANARY"},"choices":[],"error":null}

"#;
        assert!(matches!(
            observe(hidden_error, &mut seen, false),
            Observed::UpstreamError
        ));
        let nested = br#"data: {"choices":[{"delta":{},"delta":{}}]}

"#;
        assert!(matches!(
            observe(nested, &mut seen, false),
            Observed::UpstreamError
        ));
        // A valid data line followed by a BOM-prefixed error line in the
        // SAME event hides nothing; only a stream-LEADING BOM is stripped.
        let split = b"data: {\"choices\":[{\"delta\":{}}]}\n\xef\xbb\xbfdata: {\"error\":{\"message\":\"CANARY\"}}\n\n";
        for stream_start in [false, true] {
            assert!(matches!(
                observe(split, &mut seen, stream_start),
                Observed::UpstreamError
            ));
        }
    }

    /// A finished upstream: one delta, the final chunk with usage, `[DONE]`.
    fn clean_upstream() -> BoxStream<'static, reqwest::Result<Bytes>> {
        let events = vec![
            Bytes::from_static(
                b"data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n",
            ),
            Bytes::from_static(
                b"data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":4}}\n\n",
            ),
            Bytes::from_static(b"data: [DONE]\n\n"),
        ];
        futures_util::StreamExt::boxed(futures_util::stream::iter(events.into_iter().map(Ok)))
    }

    fn test_forwarder(idle: Duration) -> (Forwarder, mpsc::Receiver<Item>) {
        let (sender, receiver) = mpsc::channel(CHANNEL_CAPACITY);
        (
            Forwarder {
                sender,
                credit: Arc::new(Semaphore::new(BACKPRESSURE_BYTES)),
                idle,
                deadline: Instant::now() + Duration::from_secs(60),
                shutdown: CancellationToken::new(),
                seen: Seen::default(),
            },
            receiver,
        )
    }

    fn test_control(drained: oneshot::Receiver<()>, connection: CancellationToken) -> PumpControl {
        PumpControl {
            aborted: Arc::new(AtomicBool::new(false)),
            drained,
            shutdown: CancellationToken::new(),
            deadline: Instant::now() + Duration::from_secs(60),
            connection,
        }
    }

    /// The whole stream (and `[DONE]`) is accepted by the gateway's buffers
    /// but the host never consumes the clean end: the drain times out, the
    /// owning connection is cut and the known terminal usage is kept.
    #[tokio::test]
    async fn a_clean_end_the_host_never_drains_cuts_the_connection_and_keeps_usage() {
        let (forwarder, _receiver) = test_forwarder(Duration::from_millis(100));
        let (_drained_tx, drained_rx) = oneshot::channel::<()>();
        let connection = CancellationToken::new();
        let end = pump(
            clean_upstream(),
            forwarder,
            test_control(drained_rx, connection.clone()),
        )
        .await;
        assert!(
            connection.is_cancelled(),
            "a drain timeout reclaims the connection permit"
        );
        assert_eq!(end.outcome, Outcome::Complete, "terminal usage was seen");
        assert_eq!(end.delivery, Delivery::LocalFailure);
        assert_eq!(end.usage.input, Some(10));
        assert_eq!(end.usage.output, Some(4));
    }

    #[tokio::test]
    async fn a_host_that_drains_the_clean_end_is_delivered_without_a_cut() {
        let (forwarder, _receiver) = test_forwarder(Duration::from_secs(5));
        let (drained_tx, drained_rx) = oneshot::channel::<()>();
        let connection = CancellationToken::new();
        tokio::spawn(async move {
            sleep(Duration::from_millis(20)).await;
            let _ = drained_tx.send(());
        });
        let end = pump(
            clean_upstream(),
            forwarder,
            test_control(drained_rx, connection.clone()),
        )
        .await;
        assert!(!connection.is_cancelled());
        assert_eq!(end.outcome, Outcome::Complete);
        assert_eq!(end.delivery, Delivery::Delivered);
    }

    #[test]
    fn events_split_across_any_boundary_and_terminator_style() {
        let stream = b"data: a\n\ndata: b\r\n\r\ndata: c\r\r: keep-alive\n\ndata: [DONE]\n\n";
        for step in [1usize, 2, 3, 7, stream.len()] {
            let mut splitter = EventSplitter::default();
            let mut events: Vec<Bytes> = Vec::new();
            for piece in stream.chunks(step) {
                splitter.push(piece);
                while let Some(event) = splitter.next_event(false) {
                    events.push(event);
                }
            }
            while let Some(event) = splitter.next_event(true) {
                events.push(event);
            }
            let joined: Vec<u8> = events.iter().flat_map(|e| e.iter().copied()).collect();
            assert_eq!(joined, stream, "step {step}: forwarded bytes are exact");
            assert_eq!(events.len(), 5, "step {step}");
            assert_eq!(event_data(&events[0]).unwrap(), b"a");
            assert_eq!(event_data(&events[2]).unwrap(), b"c");
            assert_eq!(event_data(&events[3]), None, "a comment-only event");
            assert_eq!(event_data(&events[4]).unwrap(), b"[DONE]");
        }
        let mut splitter = EventSplitter::default();
        splitter.push(b"data: x\n");
        assert!(
            splitter.next_event(true).is_none(),
            "an incomplete event never completes"
        );
        let mut cr = EventSplitter::default();
        cr.push(b"data: z\r\r");
        assert!(
            cr.next_event(false).is_none(),
            "a trailing CR may begin CRLF"
        );
        assert_eq!(&cr.next_event(true).unwrap()[..], b"data: z\r\r");
        assert_eq!(event_data(b": only a comment\n\n"), None);
        assert_eq!(event_data(b"data: 1\ndata: 2\n\n").unwrap(), b"1\n2");
    }

    #[test]
    fn usage_finish_and_stream_shape_observation() {
        let mut seen = Seen::default();
        // A mid-delta usage without finish is provisional: never terminal.
        let provisional = br#"data: {"id":"chat-1","choices":[{"index":0,"delta":{"content":"hi"}}],"usage":{"prompt_tokens":12,"completion_tokens":1}}

"#;
        assert!(matches!(
            observe(provisional, &mut seen, false),
            Observed::Plain
        ));
        assert!(seen.usage.is_none(), "no finish yet, so no terminal usage");
        assert_eq!(seen.response_id.as_deref(), Some("chat-1"));
        // The final chunk bears finish and usage: terminal.
        let final_chunk = br#"data: {"id":"chat-1","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":4,"prompt_tokens_details":{"cached_tokens":6}}}

"#;
        assert!(matches!(
            observe(final_chunk, &mut seen, false),
            Observed::Plain
        ));
        let usage = seen.usage.unwrap();
        assert_eq!(
            (usage.input, usage.cached, usage.output),
            (Some(10), Some(6), Some(4))
        );
        // A trailing usage-only chunk after the finish: the last one wins.
        let trailing = br#"data: {"id":"chat-1","choices":[],"usage":{"prompt_tokens":3,"completion_tokens":1}}

"#;
        assert!(matches!(
            observe(trailing, &mut seen, false),
            Observed::Plain
        ));
        let usage = seen.usage.unwrap();
        assert_eq!(
            (usage.input, usage.cached, usage.output),
            (Some(3), None, Some(1)),
            "last terminal usage wins; absent cache stays unknown"
        );
        // Shapes: null error beside choices is normal; comment-only events
        // have no data; anything else is an upstream stream failure.
        assert!(matches!(
            observe(
                br#"data: {"choices":[{"delta":{}}],"error":null}

"#,
                &mut seen,
                false
            ),
            Observed::Plain
        ));
        assert!(matches!(
            observe(b": keep-alive\n\n", &mut seen, false),
            Observed::Plain
        ));
        assert!(
            matches!(
                observe(b"data: not json\n\n", &mut seen, false),
                Observed::UpstreamError
            ),
            "unparsable data is withheld, not forwarded"
        );
        assert!(matches!(
            observe(b"data: {\"no_choices\":1}\n\n", &mut seen, false),
            Observed::UpstreamError
        ));
        assert!(matches!(
            observe(
                br#"data: {"choices":{"not":"an array"}}

"#,
                &mut seen,
                false
            ),
            Observed::UpstreamError
        ));
        assert!(matches!(
            observe(
                b"data: {\"error\":{\"message\":\"x\"}}\n\n",
                &mut seen,
                false
            ),
            Observed::UpstreamError
        ));
        assert!(matches!(
            observe(b"data: [DONE]\n\n", &mut seen, false),
            Observed::Done
        ));
    }

    #[test]
    fn a_stream_leading_bom_is_observed_but_forwarded_verbatim() {
        let bom = b"\xef\xbb\xbf";
        let error = {
            let mut event = bom.to_vec();
            event.extend_from_slice(b"data: {\"error\":{\"message\":\"x\"}}\n\n");
            event
        };
        let mut seen = Seen::default();
        assert!(
            matches!(observe(&error, &mut seen, true), Observed::UpstreamError),
            "the BOM must not hide an error event from the observer"
        );
        // A BOM is only ever stripped once, at the stream start. Mid-stream
        // it glues onto the first field name (an unknown field, no `data:`
        // line at all); such an event is neither a comment nor a standard
        // field, so it is withheld rather than forwarded unjudged.
        assert!(matches!(
            observe(&error, &mut seen, false),
            Observed::UpstreamError
        ));
        for benign in [
            &b"event: message\nid: 7\nretry: 1000\n\n"[..],
            b": keep-alive\n\n",
        ] {
            assert!(matches!(observe(benign, &mut seen, false), Observed::Plain));
        }
        assert!(matches!(
            observe(b"unknown: field\n\n", &mut seen, false),
            Observed::UpstreamError
        ));
        let done = {
            let mut event = bom.to_vec();
            event.extend_from_slice(b"data: [DONE]\n\n");
            event
        };
        assert!(matches!(observe(&done, &mut seen, true), Observed::Done));
        let normal = {
            let mut event = bom.to_vec();
            event.extend_from_slice(
                br#"data: {"id":"c1","choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":2}}

"#
                .as_slice(),
            );
            event
        };
        let mut seen = Seen::default();
        assert!(matches!(observe(&normal, &mut seen, true), Observed::Plain));
        assert_eq!(seen.usage.unwrap().output, Some(2));
    }

    #[test]
    fn bearer_comparison_checks_content_and_length() {
        let token = "ab".repeat(32);
        let mut headers = HeaderMap::new();
        assert!(!bearer_ok(&headers, &token));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );
        assert!(bearer_ok(&headers, &token));
        for wrong in [
            format!("Bearer {}x", token),
            format!("Bearer {}", &token[1..]),
            format!("Basic {token}"),
            "Bearer ".to_owned(),
        ] {
            headers.insert(AUTHORIZATION, HeaderValue::from_str(&wrong).unwrap());
            assert!(!bearer_ok(&headers, &token), "{wrong}");
        }
    }

    #[test]
    fn context_ids_are_bounded_distinct_canonical_uuids() {
        let one = uuid::Uuid::new_v4().to_string();
        let two = uuid::Uuid::new_v4().to_string();
        let parse = |text: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(
                "x-foundry-context-ids",
                HeaderValue::from_str(text).unwrap(),
            );
            parse_context_ids(&headers)
        };
        assert_eq!(parse_context_ids(&HeaderMap::new()), Some(Vec::new()));
        assert_eq!(parse(&format!("{one}, {two}")).unwrap().len(), 2);
        assert!(parse(&format!("{one},{one}")).is_none(), "duplicates");
        assert!(parse("not-a-uuid").is_none());
        assert!(
            parse(&one.to_uppercase()).is_none(),
            "canonical lowercase only"
        );
        let many = (0..65)
            .map(|_| uuid::Uuid::new_v4().to_string())
            .collect::<Vec<_>>()
            .join(",");
        assert!(parse(&many).is_none(), "more than 64");
    }

    #[test]
    fn provider_ids_are_sanitized() {
        let value = |text: &str| HeaderValue::from_str(text).ok();
        assert_eq!(
            sanitize_provider_id(value("req-1.a:b_c").as_ref()).as_deref(),
            Some("req-1.a:b_c")
        );
        assert!(sanitize_provider_id(value("has space").as_ref()).is_none());
        assert!(sanitize_provider_id(value(&"a".repeat(129)).as_ref()).is_none());
        assert!(sanitize_provider_id(None).is_none());
    }
}
