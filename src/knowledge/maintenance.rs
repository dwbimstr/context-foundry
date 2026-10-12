//! Explicit primary-root MCP maintenance. Reuses the CLI generation pipeline;
//! owner steps never hold the engine across tokenization or HTTP. No job ledger.
use super::*;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

const POLL: Duration = Duration::from_millis(20);
const QUIET: Duration = Duration::from_millis(500);
const WINDOW: Duration = Duration::from_secs(60);
const LIMITS: RunLimits = RunLimits {
    calls: 8,
    request_tokens: 32768,
    output_tokens: 16384,
};

pub struct Serving {
    pub profile: std::path::PathBuf,
    pub maintain: bool,
}
/// Shared with embedding admission when the owner has a semantic runtime.
pub(crate) struct Permit {
    #[cfg(feature = "semantic")]
    pub _runtime: Option<crate::neural::query::KnowledgeCall>,
    pub _model: crate::neural::work::ModelPermit,
}
pub(crate) trait Owner: Send + Sync {
    fn try_step(&self, step: &mut dyn FnMut(&mut Engine)) -> FResult<bool>;
    fn try_model(&self) -> FResult<Option<Permit>>;
    fn closing(&self) -> bool;
}
#[derive(Default, Serialize)]
struct Totals {
    calls: u64,
    request_tokens_o200k: u64,
    requested_output_tokens: u64,
    observed_prompt_tokens: u64,
    observed_completion_tokens: u64,
    usage_unknown_calls: u64,
}
impl Totals {
    fn add(&mut self, report: &Report) -> FResult<()> {
        // Compute every addition before changing accounting.
        let checked = |a: u64, b: u64| {
            a.checked_add(b)
                .ok_or_else(|| fail("knowledge_usage_invalid", "maintenance usage overflow"))
        };
        let next = Self {
            calls: checked(self.calls, report.calls.into())?,
            request_tokens_o200k: checked(self.request_tokens_o200k, report.request_tokens_o200k)?,
            requested_output_tokens: checked(
                self.requested_output_tokens,
                report.requested_output_tokens,
            )?,
            observed_prompt_tokens: checked(
                self.observed_prompt_tokens,
                report.observed_prompt_tokens,
            )?,
            observed_completion_tokens: checked(
                self.observed_completion_tokens,
                report.observed_completion_tokens,
            )?,
            usage_unknown_calls: checked(
                self.usage_unknown_calls,
                report.usage_unknown_calls.into(),
            )?,
        };
        *self = next;
        Ok(())
    }
}
struct State {
    revision: u64,
    running: bool,
    enabled: bool,
    continuous: bool,
    phase: &'static str,
    reason: Option<String>,
    control: Option<Arc<Control>>,
    foreground: Option<Instant>,
    report: Option<Report>,
    totals: Totals,
    window: Instant,
    spent: RunLimits,
}
pub(crate) struct Maintenance {
    profile: Arc<Profile>,
    local: local::Telemetry,
    resources: Option<Arc<crate::model_resources::Resources>>,
    state: Mutex<State>,
}
impl Maintenance {
    pub(crate) fn new(profile: Profile) -> Self {
        Self {
            profile: Arc::new(profile),
            local: local::Telemetry::default(),
            resources: None,
            state: Mutex::new(State {
                revision: 0,
                running: false,
                enabled: false,
                continuous: false,
                phase: "paused",
                reason: None,
                control: None,
                foreground: None,
                report: None,
                totals: Totals::default(),
                window: Instant::now(),
                spent: RunLimits {
                    calls: 0,
                    request_tokens: 0,
                    output_tokens: 0,
                },
            }),
        }
    }
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub(crate) fn with_resources(
        mut self,
        resources: Option<Arc<crate::model_resources::Resources>>,
    ) -> Self {
        self.resources = resources;
        self
    }
    pub(crate) fn start(self: &Arc<Self>, owner: Arc<dyn Owner>, continuous: bool) -> FResult<()> {
        if owner.closing() {
            return Err(FoundryError::Cancelled(None));
        }
        let mut state = self.lock();
        state.revision = state.revision.wrapping_add(1);
        state.continuous = continuous;
        state.enabled = true;
        state.reason = None;
        if state.running {
            return Ok(());
        }
        let control = Arc::new(Control::unbounded());
        state.control = Some(control.clone());
        state.running = true;
        state.phase = "running";
        let this = self.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("foundry-knowledge".into())
            .spawn(move || {
                loop {
                    let revision = this.lock().revision;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        drive(owner.as_ref(), &this, &control)
                    }))
                    .unwrap_or_else(|_| {
                        Err(fail("knowledge_failed", "maintenance thread panicked"))
                    });
                    let mut state = this.lock();
                    if state.enabled && state.revision != revision && !control.is_cancelled() {
                        continue;
                    }
                    if let Err(error) = result {
                        state.reason = Some(error.code().into());
                        state.phase = if control.is_cancelled() {
                            "cancelled"
                        } else {
                            "paused"
                        };
                    }
                    state.running = false;
                    state.enabled = false;
                    state.control = None;
                    break;
                }
            })
        {
            state.running = false;
            state.enabled = false;
            state.control = None;
            return Err(FoundryError::Internal(error.into()));
        }
        Ok(())
    }
    pub(crate) fn pause(&self) {
        let mut state = self.lock();
        state.enabled = false;
        state.continuous = false;
        state.phase = if state.running { "pausing" } else { "paused" };
    }
    pub(crate) fn cancel(&self) {
        let mut state = self.lock();
        state.enabled = false;
        if let Some(control) = &state.control {
            control.cancel();
        }
    }
    pub(crate) fn idle(&self) -> bool {
        !self.lock().running
    }
    pub(crate) fn foreground(&self) {
        self.lock().foreground = Some(Instant::now());
    }
    pub(crate) fn status(&self) -> Value {
        let state = self.lock();
        let last = state.report.as_ref().map(|r| {
            json!({
                "complete":r.complete,"planned":r.planned,"generated":r.generated,
                "reused_current":r.reused_current,"reused_cached":r.reused_cached,
                "pending":r.nodes.iter().filter(|n| n.state == "pending").count(),
                "publication_error":r.publication_error,
            })
        });
        json!({"state":state.phase,"continuous":state.continuous && state.enabled,
            "reason":state.reason,"last_run":last,"completed_pass_usage":state.totals,
            "local_worker":self.profile.local.as_ref().map(|_| self.local.snapshot()),
            "window_seconds":WINDOW.as_secs(),"window_calls":state.spent.calls,
            "window_request_tokens_o200k":state.spent.request_tokens,
            "window_requested_output_tokens":state.spent.output_tokens,
            "limits":{"calls":LIMITS.calls,"request_tokens_o200k":LIMITS.request_tokens,"requested_output_tokens":LIMITS.output_tokens}})
    }
}
struct Background<'a> {
    owner: &'a dyn Owner,
    maintenance: &'a Maintenance,
}
impl Access for Background<'_> {
    type Permit = Permit;
    fn step<T>(
        &mut self,
        control: &Control,
        f: impl FnOnce(&mut Engine) -> FResult<T>,
    ) -> FResult<T> {
        let mut f = Some(f);
        loop {
            if self.owner.closing() {
                control.cancel();
            }
            control.check()?;
            let mut result = None;
            if self.owner.try_step(&mut |engine| {
                result = Some(f.take().expect("one admitted step")(engine));
            })? {
                return result.expect("owner executed step");
            }
            std::thread::sleep(POLL);
        }
    }
    fn check(&self) -> FResult<()> {
        if !self.maintenance.lock().enabled {
            return Err(fail("knowledge_paused", "maintenance was paused"));
        }
        Ok(())
    }
    fn admit(&mut self, control: &Control, provider: &mut provider::Generation) -> FResult<Permit> {
        loop {
            if self.owner.closing() {
                control.cancel();
            }
            control.check()?;
            // Reap idle/pressured residency before taking the pause/admission
            // lock. Verified snapshots remain reusable through this wait.
            provider.local.idle();
            // Pause and admission are one decision. This lock is released before
            // HTTP or waiting for an engine; pause never waits for a provider.
            let mut state = self.maintenance.lock();
            if !state.enabled {
                return Err(fail("knowledge_paused", "maintenance was paused"));
            }
            if state.foreground.is_none_or(|at| at.elapsed() >= QUIET)
                && let Some(permit) = self.owner.try_model()?
            {
                match provider.local.admit_resources(&self.maintenance.profile) {
                    Ok(()) => {
                        state.phase = "generating";
                        state.reason = None;
                        return Ok(permit);
                    }
                    Err(reason @ crate::model_resources::Deferred::Budget) => {
                        return Err(fail(
                            reason.code(),
                            "configured worker ceilings exceed the owner's memory budget",
                        ));
                    }
                    Err(reason) => {
                        state.phase = "resource_wait";
                        state.reason = Some(reason.code().into());
                    }
                }
            }
            drop(state);
            std::thread::sleep(POLL);
        }
    }
}
fn drive(owner: &dyn Owner, maintenance: &Maintenance, control: &Control) -> FResult<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let profile = &maintenance.profile;
    let mut access = Background { owner, maintenance };
    let wake = access.step(control, |engine| Ok(engine.semantic_wake.clone()))?;
    let identity = hash(profile)?;
    let mut initial = true;
    let mut provider = provider::Generation::default();
    provider.local.observation = maintenance.local.snapshot();
    provider.local.telemetry = maintenance.local.clone();
    provider.local.resources = maintenance.resources.clone();
    loop {
        if owner.closing() {
            control.cancel();
        }
        control.check()?;
        {
            let mut state = maintenance.lock();
            if !state.enabled {
                state.phase = "paused";
                return Ok(());
            }
            if state.window.elapsed() >= WINDOW {
                state.window = Instant::now();
                state.spent = RunLimits {
                    calls: 0,
                    request_tokens: 0,
                    output_tokens: 0,
                };
            }
        }
        let stamp = wake.stamp();
        let pending = access.step(control, |engine| engine.derivation_pending(Lane::Knowledge))?;
        if !initial && pending.paths == 0 && !pending.reconcile_required {
            if !maintenance.lock().continuous {
                maintenance.lock().phase = "stopped";
                return Ok(());
            }
            wait_changes(maintenance, owner, control, &wake, stamp, &mut provider)?;
            continue;
        }
        let limits = {
            let state = maintenance.lock();
            RunLimits {
                calls: LIMITS.calls - state.spent.calls,
                request_tokens: LIMITS.request_tokens - state.spent.request_tokens,
                output_tokens: LIMITS.output_tokens - state.spent.output_tokens,
            }
        };
        if limits.calls == 0
            || limits.request_tokens == 0
            || limits.output_tokens < profile.output_tokens.into()
        {
            wait_budget(maintenance, owner, control, &mut provider)?;
            continue;
        }
        initial = false;
        let frontier = access.step(control, |engine| {
            engine.derivation_reconcile(Lane::Knowledge, &identity)
        })?;
        maintenance.lock().phase = "running";
        // Each pass is bounded; one in-flight request still finishes on pause.
        // Owner shutdown cancels its transport immediately at a cooperative poll.
        let pass = control.bounded_by(Instant::now() + Duration::from_secs(300));
        let report = runtime.block_on(prepare_access(
            &mut access,
            profile,
            limits,
            &pass,
            Some(frontier),
            &mut provider,
        ))?;
        let budget = report
            .nodes
            .iter()
            .any(|n| n.reason.as_deref() == Some("knowledge_budget_exhausted"));
        // A provider/input failure must pause even when a later node also
        // exhausts the window; otherwise every window could retry bad output.
        let terminal = |code: &str| {
            !matches!(
                code,
                "knowledge_child_pending"
                    | "knowledge_budget_exhausted"
                    | "stale_handle"
                    | "not_found"
                    | "conflict"
            )
        };
        let failure = report
            .publication_error
            .as_ref()
            .filter(|code| terminal(code))
            .cloned()
            .or_else(|| {
                report
                    .nodes
                    .iter()
                    .filter_map(|n| n.reason.as_ref())
                    .find(|code| terminal(code))
                    .cloned()
            });
        let reason = failure
            .clone()
            .or_else(|| report.publication_error.clone())
            .or_else(|| {
                report
                    .nodes
                    .iter()
                    .filter_map(|n| n.reason.as_ref())
                    .find(|code| code.as_str() != "knowledge_child_pending")
                    .cloned()
            });
        let complete = report.complete;
        {
            let mut state = maintenance.lock();
            state.spent.calls += report.calls;
            state.spent.request_tokens += report.request_tokens_o200k;
            state.spent.output_tokens += report.requested_output_tokens;
            state.totals.add(&report)?;
            state.reason = reason.clone();
            state.report = Some(report);
            if !state.enabled {
                state.phase = "paused";
                return Ok(());
            }
            if !state.continuous {
                state.phase = if complete { "stopped" } else { "paused" };
                return Ok(());
            }
        }
        if failure.is_some() {
            maintenance.lock().phase = "paused";
            return Ok(());
        }
        if budget {
            wait_budget(maintenance, owner, control, &mut provider)?;
        } else if !complete {
            match reason.as_deref() {
                Some("stale_handle" | "not_found" | "conflict") => {
                    wait_changes(maintenance, owner, control, &wake, stamp, &mut provider)?
                }
                _ => {
                    maintenance.lock().phase = "paused";
                    return Ok(());
                }
            }
        }
    }
}
fn wait_changes(
    maintenance: &Maintenance,
    owner: &dyn Owner,
    control: &Control,
    wake: &crate::neural::work::Wake,
    stamp: u64,
    provider: &mut provider::Generation,
) -> FResult<()> {
    maintenance.lock().phase = "waiting";
    loop {
        if owner.closing() {
            control.cancel();
        }
        control.check()?;
        provider.local.idle();
        let mut state = maintenance.lock();
        if !state.enabled || !state.continuous {
            return Ok(());
        }
        state.phase = "waiting";
        drop(state);
        if wake.wait(stamp) {
            return Ok(());
        }
    }
}
fn wait_budget(
    maintenance: &Maintenance,
    owner: &dyn Owner,
    control: &Control,
    provider: &mut provider::Generation,
) -> FResult<()> {
    provider.local.unload("budget_wait");
    maintenance.lock().phase = "budget_wait";
    loop {
        if owner.closing() {
            control.cancel();
        }
        control.check()?;
        let mut state = maintenance.lock();
        if !state.enabled || state.window.elapsed() >= WINDOW {
            return Ok(());
        }
        state.phase = "budget_wait";
        drop(state);
        std::thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct TestOwner {
        engine: Mutex<Engine>,
        steps: AtomicUsize,
    }
    impl Owner for TestOwner {
        fn try_step(&self, step: &mut dyn FnMut(&mut Engine)) -> FResult<bool> {
            self.steps.fetch_add(1, Ordering::SeqCst);
            step(&mut self.engine.lock().unwrap());
            Ok(true)
        }
        fn try_model(&self) -> FResult<Option<Permit>> {
            panic!("missing source cannot call a provider")
        }
        fn closing(&self) -> bool {
            false
        }
    }
    fn until(test: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !test() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(POLL);
        }
    }
    #[test]
    fn missing_sources_wait_without_store_polling_and_prepare_resumes_a_waiting_driver() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
        let profile: Profile = serde_json::from_value(json!({
            "v":1,"workspace_id":engine.workspace_id().unwrap(),"recipe":"fixture","model":"fixture",
            "artifact_sha256":"a".repeat(64),"endpoint":"http://127.0.0.1:9/v1/chat/completions",
            "source_scope":["missing.rs"],"instructions":"","max_request_tokens":1024,"max_request_bytes":8192,
            "output_tokens":128,"call_seconds":1,"cache_bytes":1048576,
            "nodes":[{"id":"missing","sources":[{"path":"missing.rs"}]}]
        })).unwrap();
        let maintenance = Arc::new(Maintenance::new(profile));
        let owner = Arc::new(TestOwner {
            engine: Mutex::new(engine),
            steps: AtomicUsize::new(0),
        });
        maintenance.start(owner.clone(), true).unwrap();
        until(|| maintenance.status()["state"] == "waiting");
        let steps = owner.steps.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(600));
        assert_eq!(
            owner.steps.load(Ordering::SeqCst),
            steps,
            "quiet waits read no store rows"
        );
        maintenance.start(owner, false).unwrap();
        until(|| maintenance.idle());
        assert_eq!(maintenance.status()["reason"], "not_found");
    }
}
