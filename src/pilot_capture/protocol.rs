//! Private bridge protocol between the Rust capture session and the native
//! OMP bridge (bounded pilot v1 §5, approved implementer contract §4).
//!
//! Frames reuse [`crate::neural::protocol`]: `u32` LE header length, compact
//! JSON header, `u32` LE payload length, raw payload bytes. The header cap is
//! the shared 64 KiB; the capture payload cap is 1 MiB. Every header carries
//! the protocol version, the Rust-allocated session and host identities and a
//! checked per-direction `u32` sequence starting at 1. Message types are a
//! closed, versioned set; unknown fields, unknown types, out-of-order or
//! duplicate frames, stale identities, oversized lengths and truncation are
//! refused. Metadata-only messages carry no payload. Raw request bodies and
//! accepted response bytes cross only as transient binary payloads; nothing
//! here logs them.

use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

use crate::neural::protocol::{FrameError, FrameHeader, read_frame_as, write_frame_as};

/// The only capture protocol version (independent of the embedding worker's).
pub const CAPTURE_PROTOCOL: u32 = 1;
/// Largest payload either direction may carry.
pub const MAX_CAPTURE_PAYLOAD_BYTES: usize = 1024 * 1024;
/// Header list bounds for a relayed fetch (ported from the accepted guard).
pub const MAX_RELAY_HEADERS: usize = 64;
pub const MAX_HEADER_NAME_BYTES: usize = 128;
pub const MAX_HEADER_VALUE_BYTES: usize = 8192;
/// Longest logical URL a fetch may name.
pub const MAX_URL_BYTES: usize = 2048;
/// Longest identity string (session, host, tool-call id, model fields).
pub const MAX_IDENTITY_BYTES: usize = 128;
/// Most tool names a Ready frame may report.
pub const MAX_READY_TOOLS: usize = 64;

/// Closed failure classes (implementer contract §4). Diagnostic detail is a
/// separate static code chosen by Rust; neither ever carries upstream bodies,
/// headers, credentials or prompt material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultClass {
    Authority,
    Pins,
    Bootstrap,
    Protocol,
    RequestBinding,
    Route,
    Counting,
    Usage,
    Retry,
    ProviderLimitBreach,
    Privacy,
    Persistence,
    Deadline,
    CheckerProvenance,
    Cleanup,
    /// A provider attempt failed (non-success status, transport error,
    /// stream error or withheld upstream error event).
    Transport,
    /// A tool call outside the admitted set was attempted.
    Containment,
    /// Expected slot-local exhaustion (budget, permits, wall or input cap).
    SlotLimit,
}

impl FaultClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Authority => "authority",
            Self::Pins => "pins",
            Self::Bootstrap => "bootstrap",
            Self::Protocol => "protocol",
            Self::RequestBinding => "request_binding",
            Self::Route => "route",
            Self::Counting => "counting",
            Self::Usage => "usage",
            Self::Retry => "retry",
            Self::ProviderLimitBreach => "provider_limit_breach",
            Self::Privacy => "privacy",
            Self::Persistence => "persistence",
            Self::Deadline => "deadline",
            Self::CheckerProvenance => "checker_provenance",
            Self::Cleanup => "cleanup",
            Self::Transport => "transport",
            Self::Containment => "containment",
            Self::SlotLimit => "slot_limit",
        }
    }
}

/// Why the bridge abandoned a fetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelReason {
    /// The native request signal aborted.
    Aborted,
    /// The native consumer cancelled the response stream.
    StreamCancelled,
}

/// Closed bridge-side fault codes. The bridge never reports exception text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeFault {
    /// Bridge configuration (fds, identities, env) was missing or invalid.
    Config,
    /// The host-bundled delegate could not be imported.
    DelegateImport,
    /// The catalog model was missing or its clone was not accepted.
    ModelSelection,
    /// Thinking level or active tools did not settle to the frozen values.
    Settings,
    /// A Rust frame could not be decoded or arrived out of order.
    Protocol,
    /// The native host invoked the custom API with an unexpected model or
    /// options shape.
    Invocation,
    /// The payload callback produced a conflicting service tier.
    ServiceTier,
    /// The fetch input could not be relayed (non-string URL, stream body...).
    FetchShape,
    /// The private channel failed while writing.
    ChannelWrite,
    /// Any other internal bridge failure (no text is carried).
    Internal,
}

/// Outcome of a native tool call as the bridge observed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolOutcome {
    Ok,
    Error,
    Blocked,
}

/// Why Rust closed the private channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseReason {
    Completed,
    Halted,
}

/// Model identity the bridge observed after its public `setModel`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadyModel {
    pub provider: String,
    pub id: String,
    pub api: String,
    pub max_tokens: u64,
}

/// Child → Rust messages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChildMessage {
    /// Loaded bridge, delegate, model, API, settings and tool identities.
    Ready {
        /// Custom API identifier the bridge registered.
        api: String,
        /// `typeof` of the imported host-bundled delegate (must be "function").
        delegate: String,
        model: ReadyModel,
        thinking_level: String,
        active_tools: Vec<String>,
    },
    /// A native fetch: method, logical URL, bounded headers; payload = raw
    /// final body bytes.
    FetchBegin {
        invocation: u32,
        fetch: u32,
        method: String,
        url: String,
        headers: Vec<(String, String)>,
    },
    FetchCancel {
        fetch: u32,
        reason: CancelReason,
    },
    /// A native tool call awaiting admission; payload = JSON arguments.
    ToolCheck {
        call: String,
        name: String,
    },
    ToolDone {
        call: String,
        outcome: ToolOutcome,
        result_sha256: String,
        result_bytes: u64,
    },
    Fault {
        code: BridgeFault,
    },
}

impl ChildMessage {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Ready { .. } => "ready",
            Self::FetchBegin { .. } => "fetch_begin",
            Self::FetchCancel { .. } => "fetch_cancel",
            Self::ToolCheck { .. } => "tool_check",
            Self::ToolDone { .. } => "tool_done",
            Self::Fault { .. } => "fault",
        }
    }

    /// Whether this message type carries a payload (all others must not).
    pub fn carries_payload(&self) -> bool {
        matches!(self, Self::FetchBegin { .. } | Self::ToolCheck { .. })
    }
}

/// Rust → child messages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ParentMessage {
    /// Status and explicitly permitted, validated response headers.
    ResponseStart {
        fetch: u32,
        status: u16,
        headers: Vec<(String, String)>,
    },
    /// Payload = one accepted, unchanged SSE event's bytes.
    ResponseData {
        fetch: u32,
    },
    /// The stream settled successfully.
    ResponseEnd {
        fetch: u32,
    },
    /// Closed failure; the bridge errors the native response.
    ResponseError {
        fetch: u32,
        class: FaultClass,
    },
    ToolDecision {
        call: String,
        allow: bool,
    },
    Close {
        reason: CloseReason,
    },
}

impl ParentMessage {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ResponseStart { .. } => "response_start",
            Self::ResponseData { .. } => "response_data",
            Self::ResponseEnd { .. } => "response_end",
            Self::ResponseError { .. } => "response_error",
            Self::ToolDecision { .. } => "tool_decision",
            Self::Close { .. } => "close",
        }
    }

    pub fn carries_payload(&self) -> bool {
        matches!(self, Self::ResponseData { .. })
    }
}

/// Header of a child → Rust frame.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildHeader {
    pub v: u32,
    pub session: String,
    pub host: String,
    pub seq: u32,
    pub msg: ChildMessage,
}

/// Header of a Rust → child frame.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParentHeader {
    pub v: u32,
    pub session: String,
    pub host: String,
    pub seq: u32,
    pub msg: ParentMessage,
}

impl FrameHeader for ChildHeader {
    const PROTOCOL: u32 = CAPTURE_PROTOCOL;
    const MAX_PAYLOAD_BYTES: usize = MAX_CAPTURE_PAYLOAD_BYTES;
    fn protocol(&self) -> u32 {
        self.v
    }
}

impl FrameHeader for ParentHeader {
    const PROTOCOL: u32 = CAPTURE_PROTOCOL;
    const MAX_PAYLOAD_BYTES: usize = MAX_CAPTURE_PAYLOAD_BYTES;
    fn protocol(&self) -> u32 {
        self.v
    }
}

/// A protocol refusal: a closed static code, never peer-supplied text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolFault(pub &'static str);

impl std::fmt::Display for ProtocolFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// A validated inbound child message with its payload.
#[derive(Debug)]
pub struct Inbound {
    pub seq: u32,
    pub msg: ChildMessage,
    pub payload: Vec<u8>,
}

/// The identities both directions must carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelIdentity {
    pub session: String,
    pub host: String,
}

impl ChannelIdentity {
    pub fn new(session: String, host: String) -> Result<Self, ProtocolFault> {
        if !identity_ok(&session) || !identity_ok(&host) {
            return Err(ProtocolFault("identity_invalid"));
        }
        Ok(Self { session, host })
    }
}

/// Inbound validation state: identity and strictly increasing sequence.
#[derive(Debug)]
pub struct InboundState {
    identity: ChannelIdentity,
    next: u32,
}

impl InboundState {
    pub fn new(identity: ChannelIdentity) -> Self {
        Self { identity, next: 1 }
    }

    /// Decode one frame from `r` and validate it. `Ok(None)` is a clean EOF
    /// before any byte of a new frame.
    pub fn read<R: Read>(&mut self, r: &mut R) -> Result<Option<Inbound>, ProtocolFault> {
        let (header, payload) = match read_frame_as::<ChildHeader, R>(r) {
            Ok(frame) => frame,
            Err(FrameError::Eof) => return Ok(None),
            Err(FrameError::TooLarge(_)) => return Err(ProtocolFault("frame_too_large")),
            Err(FrameError::Malformed(_)) => return Err(ProtocolFault("frame_malformed")),
            Err(FrameError::Io(_)) => return Err(ProtocolFault("channel_read_failed")),
        };
        self.accept(header, payload).map(Some)
    }

    /// Validate an already decoded frame (identity, order, payload rules,
    /// field bounds).
    pub fn accept(
        &mut self,
        header: ChildHeader,
        payload: Vec<u8>,
    ) -> Result<Inbound, ProtocolFault> {
        if header.session != self.identity.session || header.host != self.identity.host {
            return Err(ProtocolFault("identity_stale"));
        }
        if header.seq != self.next {
            return Err(ProtocolFault("sequence_out_of_order"));
        }
        self.next = self
            .next
            .checked_add(1)
            .ok_or(ProtocolFault("sequence_exhausted"))?;
        if matches!(header.msg, ChildMessage::FetchBegin { .. }) && payload.is_empty() {
            return Err(ProtocolFault("fetch_body_missing"));
        }
        if header.msg.carries_payload() == payload.is_empty() {
            return Err(ProtocolFault("payload_presence_invalid"));
        }
        validate_child(&header.msg)?;
        Ok(Inbound {
            seq: header.seq,
            msg: header.msg,
            payload,
        })
    }
}

/// Outbound framing state: identity and the next sequence number.
#[derive(Debug)]
pub struct OutboundState {
    identity: ChannelIdentity,
    next: u32,
}

impl OutboundState {
    pub fn new(identity: ChannelIdentity) -> Self {
        Self { identity, next: 1 }
    }

    /// Frame and write one message, then flush.
    pub fn write<W: Write>(
        &mut self,
        w: &mut W,
        msg: ParentMessage,
        payload: &[u8],
    ) -> Result<(), ProtocolFault> {
        if msg.carries_payload() == payload.is_empty() {
            return Err(ProtocolFault("payload_presence_invalid"));
        }
        if payload.len() > MAX_CAPTURE_PAYLOAD_BYTES {
            return Err(ProtocolFault("frame_too_large"));
        }
        let seq = self.next;
        self.next = self
            .next
            .checked_add(1)
            .ok_or(ProtocolFault("sequence_exhausted"))?;
        let header = ParentHeader {
            v: CAPTURE_PROTOCOL,
            session: self.identity.session.clone(),
            host: self.identity.host.clone(),
            seq,
            msg,
        };
        write_frame_as(w, &header, payload).map_err(|_| ProtocolFault("channel_write_failed"))
    }
}

fn identity_ok(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTITY_BYTES
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
}

fn bounded_text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

/// Header-name grammar ported from the accepted apparatus guard.
pub fn header_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_HEADER_NAME_BYTES
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

/// Header values: bounded, no control characters.
pub fn header_value_ok(value: &str) -> bool {
    value.len() <= MAX_HEADER_VALUE_BYTES && !value.chars().any(char::is_control)
}

/// Validate a relayed header list: bounds, grammar and no duplicate names
/// after lowercasing.
pub fn validate_headers(headers: &[(String, String)]) -> Result<(), ProtocolFault> {
    if headers.len() > MAX_RELAY_HEADERS {
        return Err(ProtocolFault("header_count_exceeded"));
    }
    let mut seen: Vec<String> = Vec::with_capacity(headers.len());
    for (name, value) in headers {
        if !header_name_ok(name) {
            return Err(ProtocolFault("header_name_invalid"));
        }
        if !header_value_ok(value) {
            return Err(ProtocolFault("header_value_invalid"));
        }
        let lower = name.to_ascii_lowercase();
        if seen.contains(&lower) {
            return Err(ProtocolFault("header_duplicate"));
        }
        seen.push(lower);
    }
    Ok(())
}

fn validate_child(msg: &ChildMessage) -> Result<(), ProtocolFault> {
    match msg {
        ChildMessage::Ready {
            api,
            delegate,
            model,
            thinking_level,
            active_tools,
        } => {
            let fields = [
                api.as_str(),
                delegate.as_str(),
                model.provider.as_str(),
                model.id.as_str(),
                model.api.as_str(),
                thinking_level.as_str(),
            ];
            if !fields.iter().all(|f| bounded_text(f, MAX_IDENTITY_BYTES))
                || active_tools.len() > MAX_READY_TOOLS
                || !active_tools
                    .iter()
                    .all(|t| bounded_text(t, MAX_IDENTITY_BYTES))
            {
                return Err(ProtocolFault("ready_field_invalid"));
            }
        }
        ChildMessage::FetchBegin {
            fetch,
            method,
            url,
            headers,
            ..
        } => {
            if *fetch == 0 || !bounded_text(method, 16) || !bounded_text(url, MAX_URL_BYTES) {
                return Err(ProtocolFault("fetch_field_invalid"));
            }
            validate_headers(headers)?;
        }
        ChildMessage::FetchCancel { fetch, .. } => {
            if *fetch == 0 {
                return Err(ProtocolFault("fetch_field_invalid"));
            }
        }
        ChildMessage::ToolCheck { call, name } => {
            if !bounded_text(call, MAX_IDENTITY_BYTES) || !bounded_text(name, MAX_IDENTITY_BYTES) {
                return Err(ProtocolFault("tool_field_invalid"));
            }
        }
        ChildMessage::ToolDone {
            call,
            result_sha256,
            ..
        } => {
            if !bounded_text(call, MAX_IDENTITY_BYTES) || !is_sha256_hex(result_sha256) {
                return Err(ProtocolFault("tool_field_invalid"));
            }
        }
        ChildMessage::Fault { .. } => {}
    }
    Ok(())
}

/// Lowercase 64-digit hex.
pub fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn ident() -> ChannelIdentity {
        ChannelIdentity::new("s-1".into(), "h-1".into()).unwrap()
    }

    fn child(seq: u32, msg: ChildMessage) -> ChildHeader {
        ChildHeader {
            v: CAPTURE_PROTOCOL,
            session: "s-1".into(),
            host: "h-1".into(),
            seq,
            msg,
        }
    }

    fn encode(header: &ChildHeader, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        write_frame_as(&mut out, header, payload).unwrap();
        out
    }

    fn cancel(fetch: u32) -> ChildMessage {
        ChildMessage::FetchCancel {
            fetch,
            reason: CancelReason::Aborted,
        }
    }

    #[test]
    fn frames_round_trip_in_order_and_eof_is_clean() {
        let mut bytes = encode(&child(1, cancel(1)), b"");
        bytes.extend(encode(
            &child(
                2,
                ChildMessage::ToolCheck {
                    call: "toolu_1".into(),
                    name: "read".into(),
                },
            ),
            br#"{"path":"src/lib.rs"}"#,
        ));
        let mut state = InboundState::new(ident());
        let mut r = Cursor::new(bytes);
        let first = state.read(&mut r).unwrap().unwrap();
        assert_eq!(first.seq, 1);
        let second = state.read(&mut r).unwrap().unwrap();
        assert_eq!(second.payload, br#"{"path":"src/lib.rs"}"#);
        assert!(state.read(&mut r).unwrap().is_none());
    }

    #[test]
    fn out_of_order_duplicate_and_stale_frames_are_refused() {
        let mut state = InboundState::new(ident());
        assert_eq!(
            state.accept(child(2, cancel(1)), vec![]).unwrap_err(),
            ProtocolFault("sequence_out_of_order")
        );
        let mut state = InboundState::new(ident());
        state.accept(child(1, cancel(1)), vec![]).unwrap();
        assert_eq!(
            state.accept(child(1, cancel(1)), vec![]).unwrap_err(),
            ProtocolFault("sequence_out_of_order")
        );
        let mut stale = child(2, cancel(1));
        stale.host = "h-0".into();
        assert_eq!(
            state.accept(stale, vec![]).unwrap_err(),
            ProtocolFault("identity_stale")
        );
    }

    #[test]
    fn unknown_fields_types_and_payload_misuse_are_refused() {
        let unknown = br#"{"v":1,"session":"s-1","host":"h-1","seq":1,"msg":{"type":"fetch_cancel","fetch":1,"reason":"aborted","extra":1}}"#;
        assert!(serde_json::from_slice::<ChildHeader>(unknown).is_err());
        let unknown_type =
            br#"{"v":1,"session":"s-1","host":"h-1","seq":1,"msg":{"type":"usage","n":1}}"#;
        assert!(serde_json::from_slice::<ChildHeader>(unknown_type).is_err());
        let top = br#"{"v":1,"session":"s-1","host":"h-1","seq":1,"x":0,"msg":{"type":"fault","code":"internal"}}"#;
        assert!(serde_json::from_slice::<ChildHeader>(top).is_err());
        let mut state = InboundState::new(ident());
        assert_eq!(
            state
                .accept(child(1, cancel(1)), b"x".to_vec())
                .unwrap_err(),
            ProtocolFault("payload_presence_invalid")
        );
        let mut state = InboundState::new(ident());
        let begin = ChildMessage::FetchBegin {
            invocation: 1,
            fetch: 1,
            method: "POST".into(),
            url: "https://api.anthropic.com/v1/messages".into(),
            headers: vec![],
        };
        assert_eq!(
            state.accept(child(1, begin), vec![]).unwrap_err(),
            ProtocolFault("fetch_body_missing")
        );
    }

    #[test]
    fn truncation_and_oversize_fail_closed() {
        let bytes = encode(&child(1, cancel(1)), b"");
        let mut state = InboundState::new(ident());
        let mut r = Cursor::new(bytes[..bytes.len() - 3].to_vec());
        assert_eq!(
            state.read(&mut r).unwrap_err(),
            ProtocolFault("frame_malformed")
        );
        let mut huge = Vec::new();
        let header = serde_json::to_vec(&child(1, cancel(1))).unwrap();
        huge.extend((header.len() as u32).to_le_bytes());
        huge.extend(&header);
        huge.extend(((MAX_CAPTURE_PAYLOAD_BYTES + 1) as u32).to_le_bytes());
        let mut state = InboundState::new(ident());
        assert_eq!(
            state.read(&mut Cursor::new(huge)).unwrap_err(),
            ProtocolFault("frame_too_large")
        );
    }

    /// The exact header shapes `bridge.ts` serializes with `JSON.stringify`.
    #[test]
    fn bridge_serialized_headers_are_accepted() {
        let frames: [&[u8]; 6] = [
            br#"{"v":1,"session":"s-1","host":"h-1","seq":1,"msg":{"type":"ready","api":"cf-pilot-capture-v1","delegate":"function","model":{"provider":"anthropic","id":"claude-sonnet-5-5","api":"cf-pilot-capture-v1","max_tokens":4096},"thinking_level":"xhigh","active_tools":["read","edit"]}}"#,
            br#"{"v":1,"session":"s-1","host":"h-1","seq":2,"msg":{"type":"fetch_begin","invocation":1,"fetch":1,"method":"POST","url":"https://api.anthropic.com/v1/messages","headers":[["accept","text/event-stream"],["x-api-key","k"]]}}"#,
            br#"{"v":1,"session":"s-1","host":"h-1","seq":3,"msg":{"type":"fetch_cancel","fetch":1,"reason":"stream_cancelled"}}"#,
            br#"{"v":1,"session":"s-1","host":"h-1","seq":4,"msg":{"type":"tool_check","call":"toolu_1","name":"read"}}"#,
            br#"{"v":1,"session":"s-1","host":"h-1","seq":5,"msg":{"type":"tool_done","call":"toolu_1","outcome":"blocked","result_sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","result_bytes":0}}"#,
            br#"{"v":1,"session":"s-1","host":"h-1","seq":6,"msg":{"type":"fault","code":"service_tier"}}"#,
        ];
        let payloads: [&[u8]; 6] = [b"", b"{}", b"", b"{\"path\":\"x\"}", b"", b""];
        let mut state = InboundState::new(ident());
        for (frame, payload) in frames.iter().zip(payloads) {
            let header: ChildHeader = serde_json::from_slice(frame).unwrap();
            state.accept(header, payload.to_vec()).unwrap();
        }
        // And the parent shapes the bridge validates with exact key sets.
        let parent = serde_json::to_value(ParentHeader {
            v: 1,
            session: "s".into(),
            host: "h".into(),
            seq: 1,
            msg: ParentMessage::ResponseStart {
                fetch: 1,
                status: 200,
                headers: vec![("content-type".into(), "text/event-stream".into())],
            },
        })
        .unwrap();
        assert_eq!(
            parent["msg"],
            serde_json::json!({"type":"response_start","fetch":1,"status":200,"headers":[["content-type","text/event-stream"]]})
        );
        let err = serde_json::to_value(ParentMessage::ResponseError {
            fetch: 2,
            class: FaultClass::ProviderLimitBreach,
        })
        .unwrap();
        assert_eq!(
            err,
            serde_json::json!({"type":"response_error","fetch":2,"class":"provider_limit_breach"})
        );
    }

    #[test]
    fn header_lists_are_bounded_and_unique() {
        let ok = vec![("content-type".to_string(), "application/json".to_string())];
        assert!(validate_headers(&ok).is_ok());
        let dup = vec![
            ("X-Api-Key".to_string(), "a".to_string()),
            ("x-api-key".to_string(), "b".to_string()),
        ];
        assert_eq!(
            validate_headers(&dup).unwrap_err(),
            ProtocolFault("header_duplicate")
        );
        let ctl = vec![("x".to_string(), "a\nb".to_string())];
        assert_eq!(
            validate_headers(&ctl).unwrap_err(),
            ProtocolFault("header_value_invalid")
        );
    }

    #[test]
    fn outbound_frames_carry_identity_and_sequence() {
        let mut out = OutboundState::new(ident());
        let mut buf = Vec::new();
        out.write(&mut buf, ParentMessage::ResponseEnd { fetch: 1 }, b"")
            .unwrap();
        out.write(
            &mut buf,
            ParentMessage::ResponseData { fetch: 1 },
            b"event: ping\n\n",
        )
        .unwrap();
        assert_eq!(
            out.write(&mut buf, ParentMessage::ResponseData { fetch: 1 }, b"")
                .unwrap_err(),
            ProtocolFault("payload_presence_invalid")
        );
        let mut r = Cursor::new(buf);
        let (h1, p1): (ParentHeader, Vec<u8>) = read_frame_as(&mut r).unwrap();
        let (h2, p2): (ParentHeader, Vec<u8>) = read_frame_as(&mut r).unwrap();
        assert_eq!((h1.seq, h2.seq), (1, 2));
        assert!(p1.is_empty());
        assert_eq!(p2, b"event: ping\n\n");
        assert_eq!(h2.session, "s-1");
    }
}
