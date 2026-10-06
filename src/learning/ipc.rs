//! 013 T002 learning-worker IPC: the owner (the core) and the numerical
//! worker exchange frames in 009's shared format
//! ([`crate::neural::protocol::read_frame_as`]), with this header type. The
//! worker is a numerical engine only; the core drives the data, the order,
//! the clock and every decision.
//!
//! Every request carries a monotonic `request_id` and the identity the
//! owner believes is loaded (the model function, the starting head and the
//! number of updates since that load). Every reply echoes both; a `stepped`,
//! `logits` or `saved` reply also echoes [`input_sha256`] of the token IDs
//! and markers it computed on. A reply with any other ID or identity, a
//! duplicate, an unsolicited or out-of-order reply is a terminal worker
//! failure: the worker is stopped and nothing it produced is used.
//!
//! Token IDs travel as `u32 LE` in the payload (at most 1024); every number
//! in a header is finite JSON. Owner EOF ends the worker.
use crate::decision_model::{CheckpointPin, MAX_TOTAL_TOKENS};
use crate::neural::protocol::FrameHeader;
use serde::{Deserialize, Serialize};

pub const LEARN_PROTOCOL: u32 = 1;
/// The largest payload: one maximal input's token IDs.
pub const MAX_PAYLOAD_BYTES: usize = MAX_TOTAL_TOKENS * 4;

/// What the owner believes is loaded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub model_function_sha256: String,
    /// SHA-256 of the starting head (a base or an incumbent candidate's
    /// `head.safetensors`); `None` for the pinned initial checkpoint.
    pub head_sha256: Option<String>,
    /// Updates since that load.
    pub steps: u64,
}

/// Which starting head a `load` installs from the worker's scratch run
/// directory (fixed basenames; no path ever comes from the owner).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadSlot {
    /// `base-head.safetensors`.
    Base,
    /// `incumbent-head.safetensors`.
    Incumbent,
}

impl HeadSlot {
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Base => "base-head.safetensors",
            Self::Incumbent => "incumbent-head.safetensors",
        }
    }
}

/// Parameter counts the worker loaded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterCounts {
    pub encoder: u64,
    pub trainable: u64,
    /// `type_emb` rows 1-2, `act_head.*` and `temperature`: loaded,
    /// validated, never used or trained.
    pub frozen_other: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Message {
    /// Owner → worker: (re)load the checkpoint from the granted checkpoint
    /// directory, then the starting head when `head` names one. `seed`
    /// seeds the head dropout; `threads` caps LibTorch's intra-op pool.
    Load {
        checkpoint: CheckpointPin,
        head: Option<HeadSlot>,
        threads: u32,
        seed: u64,
    },
    /// Worker → owner: what it verified and loaded.
    Loaded {
        source_dtype: String,
        weights_sha256: String,
        encoder_config_sha256: String,
        counts: ParameterCounts,
        trainable: Vec<String>,
        frozen_encoder_sha256: String,
    },
    /// Owner → worker: one training update (train mode, head dropout on)
    /// on the payload's IDs. `target` indexes the row-order markers.
    Step {
        markers: [u32; 2],
        target: u32,
    },
    Stepped {
        input_sha256: String,
        loss: f64,
        grad_norm_before_clip: f64,
    },
    /// Owner → worker: evaluation-mode logits for the payload's IDs.
    Logits {
        markers: [u32; 2],
    },
    LogitsOut {
        input_sha256: String,
        values: [f32; 2],
    },
    /// Owner → worker: write `head.safetensors` (the trainable set,
    /// float32) into the scratch run directory, reload it, and report its
    /// digest and the reload check on the payload's probe input.
    Save {
        markers: [u32; 2],
    },
    Saved {
        input_sha256: String,
        sha256: String,
        bytes: u64,
        reload_max_abs_diff: f64,
        values: [f32; 2],
    },
    /// Owner → worker: the frozen encoder's digest.
    FrozenHash,
    FrozenHashOut {
        sha256: String,
    },
    /// Worker → owner: a named refusal or failure. Always terminal.
    Error {
        code: String,
        message: String,
    },
}

impl Message {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Load { .. } => "load",
            Self::Loaded { .. } => "loaded",
            Self::Step { .. } => "step",
            Self::Stepped { .. } => "stepped",
            Self::Logits { .. } => "logits",
            Self::LogitsOut { .. } => "logits_out",
            Self::Save { .. } => "save",
            Self::Saved { .. } => "saved",
            Self::FrozenHash => "frozen_hash",
            Self::FrozenHashOut { .. } => "frozen_hash_out",
            Self::Error { .. } => "error",
        }
    }
}

/// One frame header: version, correlation, identity, message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearnHeader {
    pub protocol: u32,
    /// Monotonic from 1; 0 only on a worker error that answers no request.
    pub request_id: u64,
    pub identity: Identity,
    pub message: Message,
}

impl FrameHeader for LearnHeader {
    const PROTOCOL: u32 = LEARN_PROTOCOL;
    const MAX_PAYLOAD_BYTES: usize = MAX_PAYLOAD_BYTES;
    fn protocol(&self) -> u32 {
        self.protocol
    }
}

/// The digest a reply echoes: SHA-256 over the IDs (`u32 LE`) and then
/// both markers (`u32 LE`). This is the frame input's identity, not the
/// contract's state `input_sha256`.
pub fn input_sha256(ids: &[u32], markers: [u32; 2]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for id in ids {
        hasher.update(id.to_le_bytes());
    }
    for marker in markers {
        hasher.update(marker.to_le_bytes());
    }
    format!("{:x}", hasher.finalize())
}

pub fn encode_ids(ids: &[u32]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(ids.len() * 4);
    for id in ids {
        payload.extend_from_slice(&id.to_le_bytes());
    }
    payload
}

/// Decode and check an input: 1..=1024 IDs below the vocabulary, two
/// distinct in-range markers. Returns the IDs and the markers as indices.
pub fn decode_input(payload: &[u8], markers: [u32; 2]) -> Result<(Vec<u32>, [usize; 2]), String> {
    if payload.is_empty() || !payload.len().is_multiple_of(4) || payload.len() > MAX_PAYLOAD_BYTES {
        return Err(format!(
            "payload of {} bytes is not 1..={MAX_TOTAL_TOKENS} u32 token IDs",
            payload.len()
        ));
    }
    let ids: Vec<u32> = payload
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let vocab = crate::decision_model::arch::VOCAB as u32;
    if let Some(bad) = ids.iter().find(|id| **id >= vocab) {
        return Err(format!(
            "token ID {bad} is outside the vocabulary of {vocab}"
        ));
    }
    let len = ids.len() as u32;
    if markers[0] >= len || markers[1] >= len || markers[0] == markers[1] {
        return Err(format!(
            "markers {markers:?} are not two distinct positions below {len}"
        ));
    }
    Ok((ids, [markers[0] as usize, markers[1] as usize]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neural::protocol::{FrameError, read_frame_as, write_frame_as};

    fn identity() -> Identity {
        Identity {
            model_function_sha256: "f".repeat(64),
            head_sha256: None,
            steps: 3,
        }
    }

    #[test]
    fn frames_round_trip_through_the_shared_format() {
        let header = LearnHeader {
            protocol: LEARN_PROTOCOL,
            request_id: 7,
            identity: identity(),
            message: Message::Logits { markers: [1, 2] },
        };
        let mut wire = Vec::new();
        write_frame_as(&mut wire, &header, &encode_ids(&[5, 6, 7])).unwrap();
        let (read, payload): (LearnHeader, Vec<u8>) = read_frame_as(&mut wire.as_slice()).unwrap();
        assert_eq!(read, header);
        assert_eq!(decode_input(&payload, [1, 2]).unwrap().0, [5, 6, 7]);
    }

    #[test]
    fn the_learning_payload_cap_and_version_are_enforced() {
        let header = br#"{"protocol":1,"request_id":1,"identity":{"model_function_sha256":"a","head_sha256":null,"steps":0},"message":{"kind":"frozen_hash"}}"#;
        let frame = |header: &[u8], payload_len: u32| {
            let mut bytes = (header.len() as u32).to_le_bytes().to_vec();
            bytes.extend_from_slice(header);
            bytes.extend_from_slice(&payload_len.to_le_bytes());
            bytes
        };
        let bytes = frame(header, MAX_PAYLOAD_BYTES as u32 + 1);
        assert!(matches!(
            read_frame_as::<LearnHeader, _>(&mut bytes.as_slice()),
            Err(FrameError::TooLarge(_))
        ));
        let old = String::from_utf8(header.to_vec())
            .unwrap()
            .replace("\"protocol\":1", "\"protocol\":2");
        let bytes = frame(old.as_bytes(), 0);
        assert!(matches!(
            read_frame_as::<LearnHeader, _>(&mut bytes.as_slice()),
            Err(FrameError::Malformed(_))
        ));
        // A nonfinite number cannot travel: JSON has no NaN.
        let nan = br#"{"protocol":1,"request_id":1,"identity":{"model_function_sha256":"a","head_sha256":null,"steps":0},"message":{"kind":"logits_out","input_sha256":"x","values":[null,0.5]}}"#;
        let bytes = frame(nan, 0);
        assert!(matches!(
            read_frame_as::<LearnHeader, _>(&mut bytes.as_slice()),
            Err(FrameError::Malformed(_))
        ));
    }

    #[test]
    fn inputs_are_checked_for_size_vocabulary_and_markers() {
        assert!(decode_input(&[], [0, 1]).is_err());
        assert!(decode_input(&[0, 0, 0], [0, 1]).is_err());
        assert!(decode_input(&encode_ids(&[1, 2]), [0, 0]).is_err());
        assert!(decode_input(&encode_ids(&[1, 2]), [0, 2]).is_err());
        assert!(decode_input(&encode_ids(&[1, 50368]), [0, 1]).is_err());
        assert!(decode_input(&encode_ids(&vec![1; 1024]), [0, 1]).is_ok());
        assert!(decode_input(&encode_ids(&vec![1; 1025]), [0, 1]).is_err());
    }

    #[test]
    fn the_input_digest_binds_ids_and_markers() {
        let a = input_sha256(&[1, 2, 3], [0, 1]);
        assert_ne!(a, input_sha256(&[1, 2, 3], [1, 0]));
        assert_ne!(a, input_sha256(&[1, 2, 4], [0, 1]));
        assert_eq!(a, input_sha256(&[1, 2, 3], [0, 1]));
    }
}
