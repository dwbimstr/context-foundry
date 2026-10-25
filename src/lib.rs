//! Local source knowledge with one durable owner and a disposable search index.

/// Reach a named fault point. With the non-default `test-faults` feature this
/// is a real hook; without it the macro expands to `Ok(())` and the point
/// name, hook code and test-only strings are absent from the build.
#[cfg(feature = "test-faults")]
macro_rules! fault {
    ($name:ident, $engine:expr, $control:expr, $detail:expr) => {
        $crate::fault::hit(
            $crate::fault::names::$name,
            &$crate::fault::Ctx {
                engine: $engine,
                control: $control,
                detail: $detail,
            },
        )
    };
}

#[cfg(not(feature = "test-faults"))]
macro_rules! fault {
    ($name:ident, $engine:expr, $control:expr, $detail:expr) => {
        Ok::<(), $crate::FoundryError>(())
    };
}

pub mod adapter_cli;
pub mod adapter_error;
pub mod bootstrap;
pub mod boundary;
pub mod brief;
pub mod cli;
pub mod config;
pub mod control;
pub mod decision_model;
pub mod error;
#[cfg(feature = "test-faults")]
pub mod fault;
#[cfg(unix)]
pub mod gateway;
#[cfg(unix)]
pub mod gateway_launch;
pub mod graph;
pub mod ingest;
pub mod knowledge;
pub mod learning;
pub mod mcp;
pub mod memory;
pub mod model_resources;
pub mod neural;
pub mod policy;
pub mod receipts;
pub mod response;
pub mod retrieval_trace;
pub mod roots;
pub mod scip;
pub mod store;
pub mod syntax;
pub mod task_context;
pub mod task_evidence;
pub mod task_host;
#[cfg(feature = "test-faults")]
pub mod testkit;
pub mod usage;
mod watch;

pub use control::Control;
pub use error::{FResult, FoundryError, PartialIndexCounts};
pub use ingest::IndexReport;
pub use response::{FinalRender, Strategy};
pub use store::{
    Engine, RepairReport, SCHEMA_VERSION, SourceHandle, SourceMeta, StoreStatus,
    workspace_id_for_root,
};

pub fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    lower_hex(&Sha256::digest(bytes))
}

/// Two lowercase hex digits per byte, in order: the encoding of every stored
/// SHA-256 string (source hashes, cache keys, artifact pins). It does not
/// depend on a digest type's `LowerHex` implementation.
pub fn lower_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// An `io::Write` that keeps only the number of bytes written: serializing
/// into it measures the exact compact `serde_json` length without building it.
pub(crate) struct ByteCount(pub(crate) u64);

impl std::io::Write for ByteCount {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 += bytes.len() as u64;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The exact compact `serde_json` length of `value`, without building it. A
/// serialization error is the one `serde_json::to_vec` would return.
pub(crate) fn json_len(value: &impl serde::Serialize) -> serde_json::Result<usize> {
    let mut count = ByteCount(0);
    serde_json::to_writer(&mut count, value)?;
    Ok(usize::try_from(count.0).expect("a serialized length fits usize"))
}

/// The engine moves across threads (tokio spawn_blocking behind a mutex in
/// MCP); keep this guarantee at compile time.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<Engine>();
    assert_send::<Control>();
    assert_send::<FoundryError>();
};

#[cfg(test)]
mod hex_tests {
    use super::*;
    use sha2::{Digest, Sha256};

    /// Pinned SHA-256 hex strings: FIPS 180-2 vectors plus a long input.
    /// They pin the stored encoding independently of the digest crate.
    #[test]
    fn digest_hex_matches_pinned_sha256_vectors() {
        assert_eq!(
            digest(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            digest(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            digest(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        let mut streamed = Sha256::new();
        for chunk in vec![b'a'; 1_000_000].chunks(4096) {
            streamed.update(chunk);
        }
        assert_eq!(
            lower_hex(&streamed.finalize()),
            digest(&vec![b'a'; 1_000_000])
        );
        assert_eq!(lower_hex(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
        assert_eq!(lower_hex(&[]), "");
    }
}
