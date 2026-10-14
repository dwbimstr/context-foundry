//! The semantic paths of the owner and the CLI: dense-or-lexical planning of
//! the primary root's search and context, the launch-time `SemanticServing`,
//! and the semantic slot the owner and a CLI command reserve, with the CLI
//! slot's tests. `SemanticSlot` and the shared server state stay in the parent
//! module.
use super::*;

/// What the semantic path decided for one request against the primary root.
#[cfg(feature = "semantic")]
enum SemanticPlan {
    Off,
    Dense(crate::neural::query::DenseWindow),
    Fallback(String),
}

/// The shared admission predicate. A query with an anchor
/// (the `anchors` a multi-root owner chose, else this store's own choice)
/// never calls the model and is served exactly as without a profile (009
/// T004 "Placement"); an error choosing them is left to the baseline call,
/// which makes the same choice.
#[cfg(feature = "semantic")]
fn semantic_anchored(
    engine: &Engine,
    query: &str,
    path: Option<&str>,
    control: &Control,
    anchors: Option<&[crate::store::AnchorCandidate]>,
    prose: bool,
) -> bool {
    let anchored = |chosen: &[crate::store::AnchorCandidate]| {
        chosen
            .iter()
            .any(|anchor| !prose || !crate::store::incidental_prose_anchor(query, anchor))
    };
    match anchors {
        Some(chosen) => anchored(chosen),
        None => crate::roots::select_anchors(&[engine], query, path, control)
            .map_or(true, |chosen| anchored(&chosen)),
    }
}

/// The dense window under the request's read deadline, or a named fallback.
#[cfg(feature = "semantic")]
fn plan_semantic(
    slot: &SemanticSlot,
    engine: &Engine,
    query: &str,
    path: Option<&str>,
    control: &Control,
    anchors: Option<&[crate::store::AnchorCandidate]>,
    prose: bool,
) -> SemanticPlan {
    if slot.is_none() || semantic_anchored(engine, query, path, control, anchors, prose) {
        return SemanticPlan::Off;
    }
    match slot {
        None => SemanticPlan::Off,
        Some(Err(word)) => SemanticPlan::Fallback(word.clone()),
        Some(Ok(runtime)) => {
            let deadline = control
                .deadline()
                .unwrap_or_else(|| Instant::now() + READ_DEADLINE);
            match runtime.window(engine, query, deadline, control) {
                Ok(window) => SemanticPlan::Dense(window),
                Err(fallback) => SemanticPlan::Fallback(crate::neural::query::fallback_word(
                    &fallback.to_string(),
                )),
            }
        }
    }
}

/// The primary root's search candidates with the semantic path applied;
/// `anchors` as in [`Engine::search_candidates_with`].
pub fn search_primary(
    slot: &SemanticSlot,
    engine: &Engine,
    query: &str,
    path: Option<&str>,
    limit: usize,
    control: &Control,
    anchors: Option<&[crate::store::AnchorCandidate]>,
) -> FResult<crate::store::CandidateBatch> {
    #[cfg(feature = "semantic")]
    match plan_semantic(slot, engine, query, path, control, anchors, false) {
        SemanticPlan::Off => {}
        SemanticPlan::Dense(window) => {
            return engine
                .search_candidates_semantic(query, path, limit, control, &window, anchors);
        }
        SemanticPlan::Fallback(word) => {
            let mut batch = engine.search_candidates_with(query, path, limit, control, anchors)?;
            batch.semantic = Some(word);
            return Ok(batch);
        }
    }
    #[cfg(not(feature = "semantic"))]
    let _ = slot;
    engine.search_candidates_with(query, path, limit, control, anchors)
}

/// The primary root's context candidates (and 008 memory hits when asked)
/// with the semantic path applied and, when a policy is configured, its
/// routing of an `auto` strategy (013 T003): after the 009 merge, before
/// graph expansion, under the same read deadline as the query embedding.
/// `anchors` as in [`Engine::context_candidates_with`].
#[allow(clippy::too_many_arguments)]
pub fn context_primary(
    slot: &SemanticSlot,
    policy: Option<&crate::policy::Policy>,
    engine: &Engine,
    query: &str,
    strategy: Strategy,
    control: &Control,
    memory: bool,
    anchors: Option<&[crate::store::AnchorCandidate]>,
) -> FResult<crate::memory::MemoryContext> {
    context_primary_with_options(
        slot,
        engine,
        query,
        strategy,
        control,
        crate::store::ContextOptions {
            memory,
            policy,
            anchors,
            ..crate::store::ContextOptions::default()
        },
    )
}

/// The same production path with explicitly configured experimental packing.
/// The default wrapper above remains available to existing library callers.
pub fn context_primary_with_options(
    slot: &SemanticSlot,
    engine: &Engine,
    query: &str,
    strategy: Strategy,
    control: &Control,
    requested: crate::store::ContextOptions<'_>,
) -> FResult<crate::memory::MemoryContext> {
    if let Some(task) = requested.task {
        engine.check_task_scope(task)?;
    }
    let policy = requested.policy.filter(|policy| policy.routes());
    #[cfg(feature = "semantic")]
    let plan = plan_semantic(
        slot,
        engine,
        query,
        None,
        control,
        requested.anchors,
        requested.experimental_behavior
            && crate::store::prose_context_query(query, strategy, policy.is_some()),
    );
    #[cfg(feature = "semantic")]
    if let Some(trace) = control.trace() {
        let state = match &plan {
            SemanticPlan::Off => serde_json::json!({"state":"off"}),
            SemanticPlan::Dense(window) => {
                serde_json::json!({"state":"dense", "hits":window.hits.len(), "complete_at":window.complete_at()})
            }
            SemanticPlan::Fallback(reason) => {
                serde_json::json!({"state":"fallback", "reason":reason})
            }
        };
        trace.record(serde_json::json!({"stage":"semantic", "query_sha256":crate::digest(query.as_bytes()), "decision":state}));
    }
    #[cfg(not(feature = "semantic"))]
    let _ = slot;
    let options = crate::store::ContextOptions {
        task: requested.task,
        compose_evidence: requested.compose_evidence,
        compact_evidence: requested.compact_evidence,
        boundary_config: requested.boundary_config,
        memory: requested.memory,
        briefs: requested.briefs,
        experimental_behavior: requested.experimental_behavior,
        #[cfg(feature = "semantic")]
        dense: match &plan {
            SemanticPlan::Dense(window) => Some(window),
            _ => None,
        },
        policy,
        anchors: requested.anchors,
    };
    #[cfg_attr(not(feature = "semantic"), allow(unused_mut))]
    let mut context = engine.context_candidates_with(query, strategy, control, &options)?;
    #[cfg(feature = "semantic")]
    if let SemanticPlan::Fallback(word) = plan {
        context.batch.semantic = Some(word);
    }
    Ok(context)
}

/// The launch-time semantic configuration (`mcp --semantic-profile FILE
/// [--development-isolation] [--semantic-cache-cap BYTES]`).
pub struct SemanticServing {
    /// Explicit continuous maintenance, resumed only by this launch option.
    pub maintain: bool,
    pub profile: PathBuf,
    pub development: bool,
    /// The owner's preparation cache cap; `None` is the 2 GiB default.
    pub cache_cap_bytes: Option<u64>,
    /// Tests only: build the provider here instead of launching the
    /// supervised worker.
    #[cfg(all(feature = "test-faults", feature = "semantic"))]
    pub provider: Option<crate::neural::query::MakeProvider>,
}

impl SemanticServing {
    /// The production configuration: the supervised worker.
    pub fn new(profile: PathBuf, development: bool) -> Self {
        Self {
            profile,
            development,
            cache_cap_bytes: None,
            maintain: false,
            #[cfg(all(feature = "test-faults", feature = "semantic"))]
            provider: None,
        }
    }

    /// Tests only: serve through a caller-built provider.
    #[cfg(all(feature = "test-faults", feature = "semantic"))]
    pub fn with_provider(profile: PathBuf, make: crate::neural::query::MakeProvider) -> Self {
        Self {
            profile,
            development: true,
            cache_cap_bytes: None,
            maintain: false,
            provider: Some(make),
        }
    }
}

/// The refusal of a semantic profile by a build without semantic support.
#[cfg(not(feature = "semantic"))]
pub(super) fn refuse_unsupported_semantic() -> FoundryError {
    FoundryError::Semantic {
        code: "semantic_unavailable",
        message: "this build has no semantic retrieval support".into(),
    }
}

/// Start the semantic runtime of one owner (MCP) or command (CLI): the
/// resident worker, or the named `fallback:<reason>` word of a refused or
/// failed start. `None` without a configured profile. Bounded by the
/// profile's load timeout; nothing loads behind a query afterwards.
pub fn semantic_slot(launch: Option<SemanticServing>) -> FResult<SemanticSlot> {
    semantic_slot_reserved(launch, None)
}

/// A command owns its runtime until the provider thread has stopped and reaped
/// its worker. Dropping the last Arc alone only closes the job channel; the CLI
/// could exit before that detached thread finishes, leaving an orphaned child.
/// MCP keeps its existing explicit asynchronous owner-shutdown path.
pub(crate) struct CliSemanticSlot(SemanticSlot);

impl std::ops::Deref for CliSemanticSlot {
    type Target = SemanticSlot;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for CliSemanticSlot {
    fn drop(&mut self) {
        #[cfg(feature = "semantic")]
        if let Some(Ok(runtime)) = &self.0 {
            runtime.shutdown();
        }
    }
}

/// A single CLI request can establish that it needs no model before opening
/// artifacts or reserving/loading a worker. Resident MCP startup is unchanged.
/// This shares the actual request's anchor/prose predicate; it is not a second
/// routing heuristic. Query/index errors remain the baseline reader's concern.
pub(crate) fn cli_semantic_slot(
    launch: Option<SemanticServing>,
    engine: &Engine,
    query: &str,
    path: Option<&str>,
    prose: bool,
    control: &Control,
) -> FResult<CliSemanticSlot> {
    #[cfg(feature = "semantic")]
    if launch.is_some() && semantic_anchored(engine, query, path, control, None, prose) {
        if let Some(trace) = control.trace() {
            trace.record(serde_json::json!({"stage":"semantic_startup", "state":"skipped", "reason":"anchor_or_baseline_error"}));
        }
        return Ok(CliSemanticSlot(None));
    }
    #[cfg(not(feature = "semantic"))]
    let _ = (engine, query, path, prose, control);
    semantic_slot(launch).map(CliSemanticSlot)
}

pub(super) fn semantic_slot_reserved(
    launch: Option<SemanticServing>,
    resources: Option<&Arc<crate::model_resources::Resources>>,
) -> FResult<SemanticSlot> {
    #[cfg(feature = "semantic")]
    {
        use crate::neural::query::{QueryRuntime, fallback_word};
        let Some(launch) = launch else {
            return Ok(None);
        };
        let fallback =
            |error: crate::neural::provider::ProviderError| fallback_word(&error.to_string());
        let profile = match crate::neural::profile::SemanticProfile::load(&launch.profile) {
            Ok(profile) => Arc::new(profile),
            Err(error) => return Ok(Some(Err(fallback(error)))),
        };
        let reservation = match resources
            .map(|resources| {
                resources.reserve(
                    crate::model_resources::Lane::Semantic,
                    profile.memory_ceiling_bytes,
                )
            })
            .transpose()
        {
            Ok(reservation) => reservation,
            Err(reason) => return Ok(Some(Err(fallback_word(reason.code())))),
        };
        let cap = launch.cache_cap_bytes;
        let capped = move |runtime: QueryRuntime| match cap {
            Some(bytes) => runtime.with_cache_cap(bytes),
            None => runtime,
        };
        #[cfg(feature = "test-faults")]
        if let Some(make) = launch.provider {
            // Tests only: the provider is built by the caller instead of the
            // supervised worker; everything else is the production path.
            let started = QueryRuntime::start_reserved(profile, make, reservation);
            return Ok(Some(started.map(capped).map(Arc::new).map_err(fallback)));
        }
        Ok(Some(
            QueryRuntime::acquire_profile(profile, launch.development, reservation)
                .map(capped)
                .map(Arc::new)
                .map_err(fallback),
        ))
    }
    #[cfg(not(feature = "semantic"))]
    {
        let _ = resources;
        match launch {
            Some(_) => Err(refuse_unsupported_semantic()),
            None => Ok(None),
        }
    }
}

#[cfg(all(test, feature = "semantic", feature = "test-faults"))]
mod tests {
    use super::*;

    #[cfg(all(feature = "semantic", feature = "test-faults"))]
    #[test]
    fn cli_slot_waits_for_provider_cleanup_on_success_and_error() {
        use crate::neural::{provider, query::QueryRuntime};
        use std::sync::mpsc;
        struct Cleanup {
            descriptor: provider::FunctionDescriptor,
            entered: mpsc::Sender<()>,
            release: mpsc::Receiver<()>,
        }
        impl provider::EmbeddingProvider for Cleanup {
            fn descriptor(&self) -> &provider::FunctionDescriptor {
                &self.descriptor
            }
            fn embed_documents(
                &mut self,
                _batch: &[provider::TokenizedInput],
                _control: &Control,
            ) -> Result<Vec<Vec<f32>>, provider::ProviderError> {
                unreachable!("cleanup needs no inference")
            }
            fn embed_query(
                &mut self,
                _input: &provider::TokenizedInput,
                _deadline: Instant,
            ) -> Result<Vec<f32>, provider::ProviderError> {
                unreachable!("cleanup needs no inference")
            }
        }
        impl Drop for Cleanup {
            fn drop(&mut self) {
                self.entered.send(()).unwrap();
                self.release.recv_timeout(Duration::from_secs(10)).unwrap();
            }
        }
        for fail in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = crate::testkit::write_semantic_profile(dir.path(), "cleanup", |_| {});
            let profile = Arc::new(crate::neural::profile::SemanticProfile::load(&path).unwrap());
            let (entered_tx, entered) = mpsc::channel();
            let (release, release_rx) = mpsc::channel();
            let provider = Cleanup {
                descriptor: profile.descriptor.clone(),
                entered: entered_tx,
                release: release_rx,
            };
            let runtime = Arc::new(
                QueryRuntime::start(profile, Box::new(move || Ok(Box::new(provider)))).unwrap(),
            );
            let (done_tx, done) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                let result = (|| -> Result<(), ()> {
                    let _slot = CliSemanticSlot(Some(Ok(runtime)));
                    if fail {
                        return Err(());
                    }
                    Ok(())
                })();
                done_tx.send(result).unwrap();
            });
            entered.recv_timeout(Duration::from_secs(10)).unwrap();
            let before_cleanup = done.recv_timeout(Duration::from_millis(50));
            release.send(()).unwrap();
            assert_eq!(before_cleanup, Err(mpsc::RecvTimeoutError::Timeout));
            assert_eq!(
                done.recv_timeout(Duration::from_secs(10)).unwrap(),
                if fail { Err(()) } else { Ok(()) }
            );
            worker.join().unwrap();
        }
    }

    #[cfg(all(feature = "semantic", feature = "test-faults"))]
    #[test]
    fn cli_semantic_startup_uses_request_anchor_scope_before_acquiring_a_provider() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let mut engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
        engine
            .replace_source("names.rs", "fn Named() {}\nfn Go() {}\n")
            .unwrap();
        engine.refresh(&Control::unbounded()).unwrap();
        let profile = crate::testkit::write_semantic_profile(dir.path(), "preflight", |_| {});
        let acquisitions = Arc::new(AtomicUsize::new(0));
        for (query, path, prose, skip) in [
            ("call Named", None, false, true),
            ("call `missing`", None, false, true),
            ("convert a Go string", None, true, false),
            ("convert a Go string", None, false, true),
            ("latent conceptual behavior", None, true, false),
            ("call Named", Some("elsewhere"), false, false),
        ] {
            let before = acquisitions.load(Ordering::SeqCst);
            let calls = acquisitions.clone();
            let launch = SemanticServing::with_provider(
                profile.clone(),
                Box::new(move || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Err(crate::neural::provider::ProviderError::Busy)
                }),
            );
            let slot = cli_semantic_slot(
                Some(launch),
                &engine,
                query,
                path,
                prose,
                &Control::unbounded(),
            )
            .unwrap();
            assert_eq!(slot.is_none(), skip, "{query}, {path:?}, {prose}");
            assert_eq!(
                acquisitions.load(Ordering::SeqCst) - before,
                usize::from(!skip)
            );
        }
        let skipped = cli_semantic_slot(
            Some(SemanticServing::new(dir.path().join("absent.json"), true)),
            &engine,
            "`Named`",
            None,
            false,
            &Control::unbounded(),
        )
        .unwrap();
        assert!(skipped.is_none(), "an unused profile is not opened");
    }
}
