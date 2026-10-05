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
pub mod cli;
pub mod config;
pub mod control;
pub mod error;
#[cfg(feature = "test-faults")]
pub mod fault;
#[cfg(unix)]
pub mod gateway;
#[cfg(unix)]
pub mod gateway_launch;
pub mod graph;
pub mod ingest;
pub mod laya;
pub mod mcp;
pub mod memory;
pub mod receipts;
pub mod response;
pub mod roots;
pub mod scip;
pub mod store;
pub mod syntax;
#[cfg(feature = "test-faults")]
pub mod testkit;
pub mod usage;

pub use control::Control;
pub use error::{FResult, FoundryError, PartialIndexCounts};
pub use ingest::IndexReport;
pub use laya::Strategy;
pub use response::FinalRender;
pub use store::{
    Engine, RepairReport, SCHEMA_VERSION, SourceHandle, SourceMeta, StoreStatus,
    workspace_id_for_root,
};

pub fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

/// The engine moves across threads (tokio spawn_blocking behind a mutex in
/// MCP); keep this guarantee at compile time.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<Engine>();
    assert_send::<Control>();
    assert_send::<FoundryError>();
};
