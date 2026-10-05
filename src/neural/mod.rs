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
#[cfg(all(feature = "test-faults", feature = "semantic"))]
pub mod fake;
pub mod index;
pub mod partition;
#[cfg(feature = "semantic")]
pub mod prepare;
pub mod profile;
pub mod protocol;
pub mod provider;
pub mod status;
#[cfg(target_os = "macos")]
pub mod supervisor;
#[cfg(feature = "semantic")]
pub mod tokenize;
#[cfg(target_os = "macos")]
pub mod worker_runtime;
