//! 009 worker IPC: length-prefixed frames on the worker's stdin/stdout.
//!
//! Frame = `u32 LE header length` ‖ compact JSON [`Header`] ‖ `u32 LE payload
//! length` ‖ payload. Both lengths are checked against fixed caps before any
//! allocation. Token IDs travel as `u32 LE`, vectors as `f32 LE`. Every
//! request carries an ID and the expected descriptor digest; a reply for any
//! other ID is stale and discarded by the supervisor.
//!
//! The frame format is shared: 013's learning worker frames its own header
//! type through [`read_frame_as`] / [`write_frame_as`] with the same caps and
//! the same refusals (see [`FrameHeader`]); there is no second framing.
use super::provider::{
    DIMENSIONS, DOCUMENT_BATCH, DOCUMENT_UNIT_TOKENS, FunctionDescriptor, SERVING_LIMIT_TOKENS,
    TokenizedInput,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

pub const PROTOCOL_VERSION: u32 = 1;
/// A header never needs more: the largest is `ready` with its descriptor.
pub const MAX_HEADER_BYTES: usize = 64 * 1024;
/// The largest payload is a full document batch of vectors.
pub const MAX_PAYLOAD_BYTES: usize = DOCUMENT_BATCH * DIMENSIONS * 4;
/// Bound on worker stderr retained by the supervisor (drained continuously).
pub const MAX_STDERR_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Document,
    Query,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Header {
    /// Supervisor → worker, first frame. No payload.
    Hello { protocol: u32 },
    /// Worker → supervisor, the reply to `hello`: what it verified and loaded.
    Ready {
        protocol: u32,
        descriptor: Box<FunctionDescriptor>,
        vocab_size: u32,
    },
    /// Supervisor → worker. Payload: the inputs' IDs concatenated, `lengths`
    /// giving each input's ID count in order.
    Embed {
        protocol: u32,
        id: u64,
        descriptor_digest: String,
        purpose: Purpose,
        lengths: Vec<u32>,
    },
    /// Worker → supervisor. Payload: `count × dims` f32 values.
    Vectors {
        protocol: u32,
        id: u64,
        count: u32,
        dims: u32,
    },
    /// Worker → supervisor: the one admission slot is held; nothing queued.
    Busy { protocol: u32, id: u64 },
    /// Worker → supervisor: a named refusal or failure. No payload.
    Error {
        protocol: u32,
        id: Option<u64>,
        code: String,
        message: String,
    },
}

impl Header {
    pub fn protocol(&self) -> u32 {
        match self {
            Self::Hello { protocol }
            | Self::Ready { protocol, .. }
            | Self::Embed { protocol, .. }
            | Self::Vectors { protocol, .. }
            | Self::Busy { protocol, .. }
            | Self::Error { protocol, .. } => *protocol,
        }
    }
}

/// A header type carried by the shared frame format: its protocol version,
/// the version a frame must declare, and its payload cap. The header cap
/// ([`MAX_HEADER_BYTES`]) is common to every protocol.
pub trait FrameHeader: Serialize + DeserializeOwned {
    /// The only version a frame of this type may declare.
    const PROTOCOL: u32;
    /// The largest payload a frame of this type may declare.
    const MAX_PAYLOAD_BYTES: usize;
    /// The version this header declares.
    fn protocol(&self) -> u32;
}

impl FrameHeader for Header {
    const PROTOCOL: u32 = PROTOCOL_VERSION;
    const MAX_PAYLOAD_BYTES: usize = MAX_PAYLOAD_BYTES;
    fn protocol(&self) -> u32 {
        Header::protocol(self)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FrameError {
    /// Clean end of stream before a frame began.
    Eof,
    Io(String),
    /// A declared length exceeds its cap; nothing was allocated for it.
    TooLarge(String),
    Malformed(String),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Eof => f.write_str("end of stream"),
            Self::Io(m) => write!(f, "io: {m}"),
            Self::TooLarge(m) => write!(f, "frame too large: {m}"),
            Self::Malformed(m) => write!(f, "malformed frame: {m}"),
        }
    }
}

fn read_len<R: Read>(r: &mut R, first: bool) -> Result<u32, FrameError> {
    let mut buf = [0u8; 4];
    let mut got = 0;
    while got < 4 {
        match r.read(&mut buf[got..]) {
            Ok(0) if got == 0 && first => return Err(FrameError::Eof),
            Ok(0) => return Err(FrameError::Malformed("truncated length prefix".into())),
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(FrameError::Io(e.to_string())),
        }
    }
    Ok(u32::from_le_bytes(buf))
}

fn read_exact_vec<R: Read>(r: &mut R, len: usize, what: &str) -> Result<Vec<u8>, FrameError> {
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).map_err(|e| match e.kind() {
        io::ErrorKind::UnexpectedEof => FrameError::Malformed(format!("truncated {what}")),
        _ => FrameError::Io(e.to_string()),
    })?;
    Ok(buf)
}

/// Read one frame, checking both caps before allocating, the protocol
/// version and strict JSON (unknown fields refused).
pub fn read_frame<R: Read>(r: &mut R) -> Result<(Header, Vec<u8>), FrameError> {
    read_frame_as(r)
}

/// [`read_frame`] for any [`FrameHeader`]: the same caps (the header's own
/// payload cap), checked before allocating, and the same refusals.
pub fn read_frame_as<H: FrameHeader, R: Read>(r: &mut R) -> Result<(H, Vec<u8>), FrameError> {
    let header_len = read_len(r, true)? as usize;
    if header_len == 0 || header_len > MAX_HEADER_BYTES {
        return Err(FrameError::TooLarge(format!(
            "header of {header_len} bytes; 1..={MAX_HEADER_BYTES} allowed"
        )));
    }
    let header_bytes = read_exact_vec(r, header_len, "header")?;
    let payload_len = read_len(r, false)? as usize;
    if payload_len > H::MAX_PAYLOAD_BYTES {
        return Err(FrameError::TooLarge(format!(
            "payload of {payload_len} bytes; at most {} allowed",
            H::MAX_PAYLOAD_BYTES
        )));
    }
    let header: H = serde_json::from_slice(&header_bytes)
        .map_err(|e| FrameError::Malformed(format!("header: {e}")))?;
    if header.protocol() != H::PROTOCOL {
        return Err(FrameError::Malformed(format!(
            "protocol {} is not {}",
            header.protocol(),
            H::PROTOCOL
        )));
    }
    let payload = read_exact_vec(r, payload_len, "payload")?;
    Ok((header, payload))
}

/// Write one frame and flush. Caps are enforced on the sending side too.
pub fn write_frame<W: Write>(w: &mut W, header: &Header, payload: &[u8]) -> io::Result<()> {
    write_frame_as(w, header, payload)
}

/// [`write_frame`] for any [`FrameHeader`], under its own payload cap.
pub fn write_frame_as<H: FrameHeader, W: Write>(
    w: &mut W,
    header: &H,
    payload: &[u8],
) -> io::Result<()> {
    let header_bytes = serde_json::to_vec(header).map_err(io::Error::other)?;
    if header_bytes.is_empty() || header_bytes.len() > MAX_HEADER_BYTES {
        return Err(io::Error::other("header exceeds the frame cap"));
    }
    if payload.len() > H::MAX_PAYLOAD_BYTES {
        return Err(io::Error::other("payload exceeds the frame cap"));
    }
    w.write_all(&(header_bytes.len() as u32).to_le_bytes())?;
    w.write_all(&header_bytes)?;
    w.write_all(&(payload.len() as u32).to_le_bytes())?;
    w.write_all(payload)?;
    w.flush()
}

/// Encode inputs for an `embed` request: per-input lengths and the payload.
pub fn encode_ids(inputs: &[TokenizedInput]) -> (Vec<u32>, Vec<u8>) {
    let lengths = inputs.iter().map(|i| i.ids.len() as u32).collect();
    let mut payload = Vec::with_capacity(inputs.iter().map(|i| i.ids.len() * 4).sum());
    for input in inputs {
        for id in &input.ids {
            payload.extend_from_slice(&id.to_le_bytes());
        }
    }
    (lengths, payload)
}

/// Decode and check an `embed` payload: batch size and per-input limits for
/// the purpose, the exact byte count, and every ID below `vocab_size`.
pub fn decode_ids(
    purpose: Purpose,
    lengths: &[u32],
    payload: &[u8],
    vocab_size: u32,
) -> Result<Vec<TokenizedInput>, FrameError> {
    let (max_inputs, max_ids) = match purpose {
        Purpose::Document => (DOCUMENT_BATCH, DOCUMENT_UNIT_TOKENS),
        Purpose::Query => (1, SERVING_LIMIT_TOKENS),
    };
    if lengths.is_empty() || lengths.len() > max_inputs {
        return Err(FrameError::Malformed(format!(
            "{} inputs; 1..={max_inputs} allowed",
            lengths.len()
        )));
    }
    let mut total: usize = 0;
    for (i, &len) in lengths.iter().enumerate() {
        let len = len as usize;
        if len == 0 || len > max_ids {
            return Err(FrameError::Malformed(format!(
                "input {i} has {len} IDs; 1..={max_ids} allowed"
            )));
        }
        total = total
            .checked_add(len)
            .ok_or_else(|| FrameError::Malformed("ID count overflow".into()))?;
    }
    let expected = total
        .checked_mul(4)
        .ok_or_else(|| FrameError::Malformed("payload size overflow".into()))?;
    if payload.len() != expected {
        return Err(FrameError::Malformed(format!(
            "payload has {} bytes, lengths require {expected}",
            payload.len()
        )));
    }
    let mut ids = payload
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]));
    let mut inputs = Vec::with_capacity(lengths.len());
    for &len in lengths {
        let chunk: Vec<u32> = ids.by_ref().take(len as usize).collect();
        if let Some(bad) = chunk.iter().find(|&&id| id >= vocab_size) {
            return Err(FrameError::Malformed(format!(
                "token ID {bad} is outside the vocabulary of {vocab_size}"
            )));
        }
        inputs.push(TokenizedInput { ids: chunk });
    }
    Ok(inputs)
}

/// Encode vectors for a `vectors` reply.
pub fn encode_vectors(vectors: &[Vec<f32>]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(vectors.iter().map(|v| v.len() * 4).sum());
    for vector in vectors {
        for value in vector {
            payload.extend_from_slice(&value.to_le_bytes());
        }
    }
    payload
}

/// Decode and check a `vectors` reply against the request it answers: the
/// exact count, the profile dimension, the exact byte count, finite values.
pub fn decode_vectors(
    count: u32,
    dims: u32,
    payload: &[u8],
    expected_count: usize,
) -> Result<Vec<Vec<f32>>, FrameError> {
    if count as usize != expected_count {
        return Err(FrameError::Malformed(format!(
            "{count} vectors for {expected_count} inputs"
        )));
    }
    if dims as usize != DIMENSIONS {
        return Err(FrameError::Malformed(format!(
            "dimension {dims} is not {DIMENSIONS}"
        )));
    }
    let expected = expected_count * DIMENSIONS * 4;
    if payload.len() != expected {
        return Err(FrameError::Malformed(format!(
            "payload has {} bytes, expected {expected}",
            payload.len()
        )));
    }
    let values: Vec<f32> = payload
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    if values.iter().any(|v| !v.is_finite()) {
        return Err(FrameError::Malformed("nonfinite vector value".into()));
    }
    Ok(values
        .chunks_exact(DIMENSIONS)
        .map(<[f32]>::to_vec)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(header_len: u32, header: &[u8], payload_len: u32, payload: &[u8]) -> Vec<u8> {
        let mut bytes = header_len.to_le_bytes().to_vec();
        bytes.extend_from_slice(header);
        bytes.extend_from_slice(&payload_len.to_le_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    #[test]
    fn a_request_round_trips_and_is_checked_against_the_purpose_limits() {
        let inputs = vec![
            TokenizedInput { ids: vec![5, 6, 7] },
            TokenizedInput { ids: vec![9] },
        ];
        let (lengths, payload) = encode_ids(&inputs);
        let header = Header::Embed {
            protocol: PROTOCOL_VERSION,
            id: 42,
            descriptor_digest: "d".repeat(64),
            purpose: Purpose::Document,
            lengths: lengths.clone(),
        };
        let mut wire = Vec::new();
        write_frame(&mut wire, &header, &payload).unwrap();
        let (read, read_payload) = read_frame(&mut wire.as_slice()).unwrap();
        assert_eq!(read, header);
        assert_eq!(
            decode_ids(Purpose::Document, &lengths, &read_payload, 10).unwrap(),
            inputs
        );
        // Two inputs are not a query; an ID at the vocabulary size is refused.
        assert!(decode_ids(Purpose::Query, &lengths, &read_payload, 10).is_err());
        assert!(decode_ids(Purpose::Document, &lengths, &read_payload, 9).is_err());
        // A payload one byte short of the declared lengths is refused.
        assert!(decode_ids(Purpose::Document, &lengths, &read_payload[1..], 10).is_err());
    }

    #[test]
    fn per_input_limits_apply_not_just_the_batch_total() {
        // One 1025-ID document input stays refused even though eight
        // inputs could carry 8192 IDs in total.
        let lengths = [1025u32];
        let payload = vec![0u8; 1025 * 4];
        assert!(decode_ids(Purpose::Document, &lengths, &payload, 100).is_err());
        let lengths = [1024u32];
        assert!(decode_ids(Purpose::Document, &lengths, &payload[..1024 * 4], 100).is_ok());
        let lengths = [2049u32];
        assert!(decode_ids(Purpose::Query, &lengths, &vec![0u8; 2049 * 4], 100).is_err());
        assert!(decode_ids(Purpose::Document, &[1; 9], &[0u8; 36], 100).is_err());
    }

    #[test]
    fn oversized_declared_lengths_are_refused_before_allocation() {
        // A header length above the cap is refused from the 4-byte prefix
        // alone; the reader holds no more bytes than that.
        let bytes = (MAX_HEADER_BYTES as u32 + 1).to_le_bytes();
        assert!(matches!(
            read_frame(&mut bytes.as_slice()),
            Err(FrameError::TooLarge(_))
        ));
        let header = br#"{"kind":"hello","protocol":1}"#;
        let bytes = frame(
            header.len() as u32,
            header,
            MAX_PAYLOAD_BYTES as u32 + 1,
            &[],
        );
        assert!(matches!(
            read_frame(&mut bytes.as_slice()),
            Err(FrameError::TooLarge(_))
        ));
        let bytes = frame(0, &[], 0, &[]);
        assert!(matches!(
            read_frame(&mut bytes.as_slice()),
            Err(FrameError::TooLarge(_))
        ));
    }

    #[test]
    fn eof_truncation_versions_and_unknown_fields_are_distinguished() {
        assert_eq!(read_frame(&mut [].as_slice()), Err(FrameError::Eof));
        assert!(matches!(
            read_frame(&mut [1u8, 0].as_slice()),
            Err(FrameError::Malformed(_))
        ));
        let header = br#"{"kind":"hello","protocol":1}"#;
        let full = frame(header.len() as u32, header, 4, &[1, 2, 3, 4]);
        assert!(matches!(
            read_frame(&mut &full[..full.len() - 1]),
            Err(FrameError::Malformed(_))
        ));
        let old = br#"{"kind":"hello","protocol":0}"#;
        let bytes = frame(old.len() as u32, old, 0, &[]);
        assert!(matches!(
            read_frame(&mut bytes.as_slice()),
            Err(FrameError::Malformed(_))
        ));
        let extra = br#"{"kind":"hello","protocol":1,"x":1}"#;
        let bytes = frame(extra.len() as u32, extra, 0, &[]);
        assert!(matches!(
            read_frame(&mut bytes.as_slice()),
            Err(FrameError::Malformed(_))
        ));
    }

    #[test]
    fn vectors_are_checked_for_count_dimension_length_and_finiteness() {
        let good = vec![vec![0.5f32; DIMENSIONS]; 2];
        let payload = encode_vectors(&good);
        assert_eq!(
            decode_vectors(2, DIMENSIONS as u32, &payload, 2).unwrap(),
            good
        );
        assert!(decode_vectors(2, DIMENSIONS as u32, &payload, 3).is_err());
        assert!(decode_vectors(2, 1024, &payload, 2).is_err());
        assert!(decode_vectors(2, DIMENSIONS as u32, &payload[4..], 2).is_err());
        let mut bad = good.clone();
        bad[1][7] = f32::NAN;
        assert!(decode_vectors(2, DIMENSIONS as u32, &encode_vectors(&bad), 2).is_err());
    }
}
