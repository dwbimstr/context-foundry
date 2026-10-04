//! Deterministic NAMED fault points for recovery tests.
//!
//! This module is compiled only with the non-default `test-faults` cargo
//! feature (enabled for the crate's own tests through a self dev-dependency).
//! Release builds contain no hook code and no fault-name strings: every call
//! site goes through the `fault!` macro, which expands to `Ok(())` without
//! the feature.
//!
//! A point is a stable boundary in production code ("after the search commit,
//! before the pending clear"). Tests arm a point by name on the current
//! thread; points are never identified by how many checkpoints precede them.
use crate::{Control, Engine, FResult, FoundryError};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Stable point names. The shared prefix is what the release-build audit
/// searches for.
pub mod names {
    macro_rules! point {
        ($ident:ident, $name:literal) => {
            pub const $ident: &str = concat!("ctxfoundry-fault/", $name);
        };
    }
    point!(SOURCE_BEFORE_COMMIT, "source.before_commit");
    point!(SOURCE_AFTER_COMMIT, "source.after_commit");
    point!(INDEX_AFTER_SEARCH_COMMIT, "index.after_search_commit");
    point!(SCAN_BEFORE_ROOT_OPEN, "scan.before_root_open");
    point!(SCAN_AFTER_ROOT_OPEN, "scan.after_root_open");
    point!(SCAN_BEFORE_WALK, "scan.before_walk");
    point!(SCAN_BEFORE_OPEN, "scan.before_open");
    point!(SWEEP_PAGE, "sweep.page");
    point!(SEEN_FLUSH, "scan.seen_flush");
    point!(REPAIR_AFTER_MARKER, "repair.after_marker");
    point!(
        REPAIR_BEFORE_QUARANTINE_RENAME,
        "repair.before_quarantine_rename"
    );
    point!(
        REPAIR_AFTER_QUARANTINE_RENAME,
        "repair.after_quarantine_rename"
    );
    point!(REPAIR_AFTER_ENQUEUE_PAGE, "repair.after_enqueue_page");
    point!(
        REPAIR_BEFORE_REPLACEMENT_INDEX,
        "repair.before_replacement_index"
    );
    point!(
        REPAIR_AFTER_REPLACEMENT_INDEX,
        "repair.after_replacement_index"
    );
    point!(REPAIR_BEFORE_MARKER_CLEAR, "repair.before_marker_clear");
    point!(
        REPAIR_AFTER_SCHEMA_PUBLICATION,
        "repair.after_schema_publication"
    );
    point!(UPGRADE_BEFORE_COMMIT, "upgrade.before_commit");
    point!(UPGRADE_AFTER_COMMIT, "upgrade.after_commit");
    point!(
        CONTEXT_BEFORE_FINAL_VALIDATION,
        "context.before_final_validation"
    );
    point!(RETRIEVE_BEFORE_FINAL_READ, "retrieve.before_final_read");
    point!(MEMORY_FORGET_BEFORE_COMMIT, "memory.forget_before_commit");
    point!(MEMORY_FORGET_AFTER_COMMIT, "memory.forget_after_commit");
    point!(ROOTS_BEFORE_ROOT, "roots.before_root");
}

/// What a point sees when it is reached.
pub struct Ctx<'a> {
    pub engine: Option<&'a Engine>,
    pub control: Option<&'a Control>,
    /// Point-specific detail: the relative path about to be opened, or the
    /// size of the page/batch just formed for the paged walks.
    pub detail: &'a str,
}

pub enum Action {
    /// `std::process::abort()`: a real abrupt process exit at this boundary.
    Abort,
    /// Return an injected failure from this boundary (write/disk failure).
    Fail(String),
    /// Fail every hit whose detail contains `detail_contains`.
    FailWhen {
        detail_contains: String,
        message: String,
    },
    /// Cancel the active `Control`, exactly as an external cancel would.
    Cancel,
    /// Stall at this boundary, holding the engine's thread, so an external
    /// read deadline can elapse end-to-end. Test-only (feature-gated module).
    Delay(std::time::Duration),
    /// Run test code at this boundary, with the engine when the point has one.
    Call(Box<dyn Fn(&Ctx<'_>)>),
}

struct Armed {
    skip: usize,
    seen: usize,
    action: Action,
}

thread_local! {
    static ARMED: RefCell<HashMap<String, Rc<RefCell<Armed>>>> = RefCell::new(HashMap::new());
    static REACHED: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::new());
}

/// The env-expressible subset of [`Action`], storable process-wide: spawned
/// CLI/MCP tests arm from a main thread while engine calls run on worker
/// threads, so a thread-local table would never fire.
#[derive(Clone, Debug)]
pub enum GlobalAction {
    Abort,
    Cancel,
    Fail(String),
    Delay(std::time::Duration),
}

struct GlobalArmed {
    skip: usize,
    seen: u64,
    action: GlobalAction,
}

static ARMED_GLOBAL: std::sync::OnceLock<std::sync::Mutex<HashMap<String, GlobalArmed>>> =
    std::sync::OnceLock::new();

fn armed_global() -> &'static std::sync::Mutex<HashMap<String, GlobalArmed>> {
    ARMED_GLOBAL.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Arm `name` for EVERY thread in this process (env-armed spawned tests).
/// Fires on every hit from the `skip`-th onward, like [`arm`].
pub fn arm_global(name: &str, skip: usize, action: GlobalAction) {
    armed_global().lock().unwrap().insert(
        name.to_owned(),
        GlobalArmed {
            skip,
            seen: 0,
            action,
        },
    );
}

pub fn disarm_all() {
    ARMED.with(|armed| armed.borrow_mut().clear());
    REACHED.with(|reached| reached.borrow_mut().clear());
    armed_global().lock().unwrap().clear();
}

/// Arm `name` on this thread. The action fires on every hit from the
/// `skip`-th (0-based) onward; `FailWhen` fires on every matching hit.
pub fn arm(name: &str, skip: usize, action: Action) {
    ARMED.with(|armed| {
        armed.borrow_mut().insert(
            name.to_owned(),
            Rc::new(RefCell::new(Armed {
                skip,
                seen: 0,
                action,
            })),
        );
    });
}

/// How many times `name` was reached on this thread since the last disarm.
pub fn reached(name: &str) -> usize {
    let local = REACHED.with(|reached| reached.borrow().get(name).copied().unwrap_or(0));
    let global = armed_global()
        .lock()
        .unwrap()
        .get(name)
        .map(|state| state.seen as usize)
        .unwrap_or(0);
    local + global
}

pub(crate) fn hit(name: &str, ctx: &Ctx<'_>) -> FResult<()> {
    REACHED.with(|reached| *reached.borrow_mut().entry(name.to_owned()).or_insert(0) += 1);
    let Some(armed) = ARMED.with(|armed| armed.borrow().get(name).cloned()) else {
        // Nothing armed on this thread: the process-wide table armed for
        // spawned-process tests may still decide this hit.
        return hit_global(name, ctx);
    };
    let index = {
        let mut state = armed.borrow_mut();
        let index = state.seen;
        state.seen += 1;
        index
    };
    // Run outside any borrow so a callback may re-enter other points.
    let state = armed.borrow();
    match &state.action {
        Action::FailWhen {
            detail_contains,
            message,
        } => {
            if ctx.detail.contains(detail_contains.as_str()) {
                return Err(FoundryError::Internal(anyhow::anyhow!(message.clone())));
            }
            Ok(())
        }
        _ if index < state.skip => Ok(()),
        Action::Abort => std::process::abort(),
        Action::Fail(message) => Err(FoundryError::Internal(anyhow::anyhow!(message.clone()))),
        Action::Cancel => {
            if let Some(control) = ctx.control {
                control.cancel();
            }
            Ok(())
        }
        Action::Delay(duration) => {
            std::thread::sleep(*duration);
            Ok(())
        }
        Action::Call(call) => {
            call(ctx);
            Ok(())
        }
    }
}

fn hit_global(name: &str, ctx: &Ctx<'_>) -> FResult<()> {
    let mut global = armed_global().lock().unwrap();
    let Some(state) = global.get_mut(name) else {
        return Ok(());
    };
    let index = state.seen as usize;
    state.seen += 1;
    if index < state.skip {
        return Ok(());
    }
    match state.action.clone() {
        GlobalAction::Abort => std::process::abort(),
        GlobalAction::Fail(message) => Err(FoundryError::Internal(anyhow::anyhow!(message))),
        GlobalAction::Cancel => {
            if let Some(control) = ctx.control {
                control.cancel();
            }
            Ok(())
        }
        GlobalAction::Delay(duration) => {
            std::thread::sleep(duration);
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// ws16 collision seam (007 T001)
// ---------------------------------------------------------------------------

/// Test-only override of a canonical root's workspace identity, keyed by the
/// canonical root path. `workspace_id_for_root` consults this table so a test
/// can force two distinct roots to share `ws16` (a natural collision needs
/// ~2^32 hash evaluations). Nothing is expressed through the environment:
/// the collision refusal is checked before serving, so an in-process test
/// arms the table directly and clears it immediately after.
static WORKSPACE_OVERRIDES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, String>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

fn workspace_overrides() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
    &WORKSPACE_OVERRIDES
}

/// Make `workspace_id_for_root(canonical_root)` return `id` in this process.
pub fn override_workspace_id(canonical_root: &str, id: &str) {
    workspace_overrides()
        .lock()
        .unwrap()
        .insert(canonical_root.to_owned(), id.to_owned());
}

/// The overridden identity of `canonical_root`, if any.
pub fn workspace_id_override(canonical_root: &str) -> Option<String> {
    workspace_overrides()
        .lock()
        .unwrap()
        .get(canonical_root)
        .cloned()
}

/// Drop every workspace override (tests must not leak them into later opens).
pub fn clear_workspace_overrides() {
    workspace_overrides().lock().unwrap().clear();
}
