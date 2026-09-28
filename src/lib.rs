//! Local source knowledge with one durable owner and a disposable search index.
pub mod graph;
pub mod ingest;
pub mod laya;
pub mod store;

pub use store::{Engine, SourceMeta};

pub fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
