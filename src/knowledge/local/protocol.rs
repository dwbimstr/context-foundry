//! Length-prefixed bounded JSON over private pipes, never a listener.
use super::{Config, Observation};
use crate::knowledge::provider::{Reply, Usage};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

pub const MAX_FRAME: usize = 512 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub v: u32,
    pub config: Config,
    pub model: String,
    pub artifact_sha256: String,
    pub owner_pid: u32,
    pub model_fd: i32,
    pub liveness_fd: i32,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Answer {
    Ready {
        identity: String,
        isolation: String,
    },
    Completed {
        id: u64,
        content: String,
        usage: Usage,
        gpu_bytes: u64,
    },
    Failed {
        id: u64,
        code: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        usage: Option<Usage>,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: u64,
    pub body: serde_json::Value,
}
pub fn encode(value: &impl Serialize) -> io::Result<Vec<u8>> {
    let body = serde_json::to_vec(value)?;
    if body.len() > MAX_FRAME {
        return Err(io::Error::other("worker frame limit"));
    }
    let mut frame = Vec::with_capacity(4 + body.len());
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend(body);
    Ok(frame)
}
pub fn read<T: serde::de::DeserializeOwned>(reader: &mut impl Read) -> io::Result<T> {
    let mut size = [0u8; 4];
    reader.read_exact(&mut size)?;
    let size = u32::from_le_bytes(size) as usize;
    if size == 0 || size > MAX_FRAME {
        return Err(io::Error::other("worker frame limit"));
    }
    let mut body = vec![0; size];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body).map_err(io::Error::other)
}
pub fn write(writer: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    writer.write_all(&encode(value)?)?;
    writer.flush()
}
pub fn completed(id: u64, reply: Reply, observation: &Observation) -> Answer {
    Answer::Completed {
        id,
        content: reply.content,
        usage: reply.usage,
        gpu_bytes: observation.peak_gpu_bytes,
    }
}
