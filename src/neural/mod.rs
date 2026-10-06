//! 009 optional semantic retrieval. `provider` and `protocol` are the shared
//! boundary between core preparation and the supervised embedding worker;
//! neither loads a model, a tokenizer or Python.
//!
//! T001 core (slice A): `partition`, `tokenize`, `cache`, `index`, `prepare`,
//! `status`, `anchor` (descriptor-anchored filesystem work) and the
//! deterministic test provider `fake`. `partition`, `cache`, `anchor` and
//! `status` are compiled in every build so semantic rows survive a build
//! without the `semantic` feature; `tokenize`, `prepare` and the `index`
//! build path carry the pinned `tokenizers` and `usearch` dependencies.
//! T003: `driver`, the MCP owner's progressive preparation over the same
//! steps as `prepare`.

/// Named fault points shared by the cache and the index (test-faults only;
/// release builds carry no hook code and no fault-name strings).
#[cfg(feature = "test-faults")]
pub mod fault_names {
    macro_rules! point {
        ($ident:ident, $name:literal) => {
            pub const $ident: &str = concat!("ctxfoundry-fault/", $name);
        };
    }
    // Purge: the plan is made and the rows are removed; the descriptor-
    // relative deletion has not started.
    point!(PURGE_AFTER_PLAN, "semantic.purge_after_plan");
    // Publication: the semantic root and generation directory are open;
    // nothing has been written yet.
    point!(PUBLISH_AFTER_CHECK, "semantic.publish_after_check");
    // Serving load: the index bytes were read once and matched the manifest;
    // the USearch header check and restore from those bytes come next.
    point!(LOAD_AFTER_VERIFY, "semantic.load_after_verify");
    // Supervisor memory poll: one footprint measurement of a live worker;
    // the detail is the worker's scratch root.
    point!(FOOTPRINT_MEASURE, "supervisor.footprint_measure");
    // Every read of one stored partition row (tests count them).
    point!(PARTITION_READ, "semantic.partition_read");
    // 009 T003 admission: a document admission has begun (before its one
    // decision under the admission lock); a foreground query has registered
    // (before it claims the model slot); either path has claimed the slot
    // and not yet checked the provider's late call.
    point!(DOCUMENT_ADMISSION, "semantic.document_admission");
    point!(QUERY_REGISTERED, "semantic.query_registered");
    point!(SLOT_CLAIMED, "semantic.slot_claimed");
}

/// The fault hook entry (test-faults only).
#[cfg(feature = "test-faults")]
pub(crate) fn hit_fault(
    name: &str,
    control: Option<&crate::Control>,
    detail: &str,
) -> crate::FResult<()> {
    crate::fault::hit(
        name,
        &crate::fault::Ctx {
            engine: None,
            control,
            detail,
        },
    )
}

#[cfg(feature = "test-faults")]
macro_rules! neural_fault {
    ($name:ident, $control:expr, $detail:expr) => {
        $crate::neural::hit_fault($crate::neural::fault_names::$name, $control, $detail)
    };
}

#[cfg(not(feature = "test-faults"))]
macro_rules! neural_fault {
    ($name:ident, $control:expr, $detail:expr) => {
        Ok::<(), $crate::FoundryError>(())
    };
}

pub mod anchor;
pub mod cache;
#[cfg(feature = "semantic")]
pub mod driver;
#[cfg(all(feature = "test-faults", feature = "semantic"))]
pub mod fake;
pub mod index;
pub mod merge;
pub mod partition;
#[cfg(feature = "semantic")]
pub mod prepare;
#[cfg(target_os = "macos")]
pub mod probes;
pub mod profile;
pub mod protocol;
pub mod provider;
#[cfg(feature = "semantic")]
pub mod query;
pub mod status;
#[cfg(target_os = "macos")]
pub mod supervisor;
#[cfg(feature = "semantic")]
pub mod tokenize;
#[cfg(target_os = "macos")]
pub mod worker_runtime;
