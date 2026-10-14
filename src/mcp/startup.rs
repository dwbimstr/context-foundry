//! Owner startup (007 § Admission at launch): admission of the launch roots,
//! the one-time open of every root, and the shared state the transports
//! serve. `Shared`, the engine slot and the operations stay in the parent
//! module.
use super::*;

/// Launch-time admission and the one-time open of every root (007 §
/// Admission at launch). Admission is refused — invalid-argument exit 2 —
/// before anything is opened: `too_many_roots`, `duplicate_root`,
/// `nested_root` (path-component boundary) and `root_id_collision` (equal
/// `ws16`). The primary keeps today's fail-fast open (busy, wrong workspace
/// or unsupported schema stops startup; a broken lexical index permits
/// authoritative-only startup with coverage `repair_required`). Each
/// reference is opened exactly once; the outcome is its coverage for the
/// whole session, with no retries, and another live owner is never stopped.
pub(super) fn open_owner(
    mut options: ServerOptions,
    shutdown: CancellationToken,
) -> AResult<Arc<Shared>> {
    let model_resources = options
        .model_resources
        .take()
        .map(crate::model_resources::Resources::new)
        .transpose()?;
    let admitted = roots::validate_admission(&options.root, &options.references)
        .map_err(|error| AdapterError::named(error.code(), error.message()))?;
    let primary = &admitted[0];
    #[cfg(feature = "semantic")]
    let maintain = options
        .semantic
        .as_ref()
        .is_some_and(|launch| launch.maintain);
    let knowledge_launch = options.knowledge.take();
    let knowledge_maintain = knowledge_launch
        .as_ref()
        .is_some_and(|launch| launch.maintain);
    let knowledge = knowledge_launch
        .map(|launch| crate::knowledge::Profile::load(&launch.profile))
        .transpose()?;
    if knowledge
        .as_ref()
        .is_some_and(|profile| profile.workspace_id != primary.workspace_id)
    {
        return Err(FoundryError::WrongWorkspace.into());
    }
    let semantic_launch = options.semantic.take();
    let policy_launch = options.policy.take();
    #[cfg(not(feature = "semantic"))]
    if semantic_launch.is_some() {
        return Err(refuse_unsupported_semantic().into());
    }
    let engine = Engine::open_existing(&options.store)?;
    let bound = engine
        .workspace_id()
        .ok_or(FoundryError::WorkspaceUnbound)?;
    if bound != primary.workspace_id {
        return Err(FoundryError::WrongWorkspace.into());
    }
    let watch_reader = options
        .watch
        .then(|| crate::ingest::ChangeReader::new(&engine, &primary.root).map(Arc::new))
        .transpose()?;
    let watch = options
        .watch
        .then(|| crate::watch::Watch::new(primary.root.clone(), engine.directory().to_owned()));
    let primary_coverage = match engine.status() {
        Ok(status) if status.index_state == "repair_required" => Coverage::RepairRequired,
        Ok(_) => Coverage::Ok,
        // The store opened and bound; a status read failing here is a named
        // startup failure, not a coverage.
        Err(error) => return Err(error.into()),
    };
    let meta_of = |root: &AdmittedRoot, coverage: Coverage| RootMeta {
        alias: root.alias.clone(),
        label: root.label.clone(),
        root: root.root.clone(),
        workspace_id: root.workspace_id.clone(),
        coverage,
    };
    let mut meta = vec![meta_of(primary, primary_coverage)];
    let mut engines = vec![Some(engine)];
    for (root, reference) in admitted[1..].iter().zip(&options.references) {
        let (coverage, engine) = open_reference(root, &reference.store);
        meta.push(meta_of(root, coverage));
        engines.push(engine);
    }
    // 009 T002: ONE resident worker starts here, before serving, when a
    // semantic profile is configured; the owner never loads behind a query.
    // A refused or failed start is the named fallback every later request
    // reports, with baseline results intact. Only explicit continuous mode
    // starts preparation at launch; otherwise an index action is required.
    let semantic = semantic_slot_reserved(semantic_launch, model_resources.as_ref())?;
    // 013 T003: the config is validated and its worker loaded HERE, once,
    // before serving (bounded by the load ceiling). An invalid config or a
    // failed start leaves the policy unavailable by name; baseline
    // retrieval serves either way.
    let policy = Arc::new(crate::policy::Policy::start(
        policy_launch,
        Some(&primary.workspace_id),
    ));
    let state = Arc::new(Shared {
        experimental_behavior_context: options.experimental_behavior_context,
        trace_sink: options
            .retrieval_trace_dir
            .as_deref()
            .map(crate::retrieval_trace::TraceSink::create)
            .transpose()?
            .map(Arc::new),
        engines: Arc::new(Mutex::new(engines)),
        meta: Arc::new(meta),
        budget: options.budget,
        sessions: Mutex::new(HashMap::new()),
        in_flight_engine: AtomicUsize::new(0),
        slot_holder: std::sync::atomic::AtomicU8::new(SLOT_FREE),
        #[cfg(test)]
        slot_hooks: SlotHooks::default(),
        shutdown,
        no_memory: options.no_memory,
        experimental_task_context: options.experimental_task_context,
        watch: watch.clone(),
        #[cfg(feature = "semantic")]
        semantic_turn: Mutex::new(None),
        #[cfg(feature = "semantic")]
        source_steps: AtomicUsize::new(0),
        semantic,
        knowledge: knowledge
            .map(|profile| {
                crate::knowledge::maintenance::Maintenance::new(profile)
                    .with_resources(model_resources.clone())
            })
            .map(Arc::new),
        model_resources,
        background_model: Arc::default(),
        #[cfg(feature = "semantic")]
        preparation: Arc::default(),
        policy,
    });
    #[cfg(feature = "semantic")]
    if maintain {
        let runtime = match &state.semantic {
            Some(Ok(runtime)) => Arc::clone(runtime),
            _ => {
                return Err(FoundryError::Semantic {
                    code: "semantic_unavailable",
                    message: "continuous maintenance requires an available semantic profile".into(),
                }
                .into());
            }
        };
        state.preparation.maintain(state.clone(), runtime, None)?;
    }
    if let (Some(watch), Some(reader)) = (watch, watch_reader) {
        watch.start(state.clone(), reader)?;
    }
    if knowledge_maintain
        && let Err(error) = state
            .knowledge
            .as_ref()
            .expect("configured profile")
            .start(state.clone(), true)
    {
        state.cancel_active();
        return Err(error.into());
    }
    Ok(state)
}

/// Open one reference store once; the outcome is that root's coverage for
/// the whole session. A store bound to another (or no) root never serves
/// this one and is dropped unread.
fn open_reference(root: &AdmittedRoot, store: &Path) -> (Coverage, Option<Engine>) {
    let coverage = match Engine::open_existing(store) {
        Err(FoundryError::StoreNotFound) => Coverage::MissingStore,
        Err(FoundryError::StoreBusy) => Coverage::Busy,
        Err(FoundryError::UnsupportedSchema { .. } | FoundryError::UpgradeRequired { .. }) => {
            Coverage::UnsupportedSchema
        }
        // An unreadable authoritative database and every other open failure
        // read as corruption; a coverage never invents state.
        Err(_) => Coverage::Corrupt,
        Ok(engine) => match engine.workspace_id() {
            Some(bound) if bound == root.workspace_id => match engine.status() {
                Ok(status) if status.index_state == "repair_required" => {
                    return (Coverage::RepairRequired, Some(engine));
                }
                Ok(_) => return (Coverage::Ok, Some(engine)),
                Err(_) => Coverage::Corrupt,
            },
            _ => Coverage::WrongWorkspace,
        },
    };
    (coverage, None)
}
