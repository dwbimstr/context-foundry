//! The command-line surface. This module is ordinary release code and never
//! reads fault-environment variables.
use crate::graph::{REFERENCES_DEFAULT_LIMIT, ReferencesRequest, ReferencesSeed};
use crate::learning;
use crate::{
    Control, Engine, FResult, FoundryError, Strategy,
    adapter_error::AResult,
    graph::GraphBundle,
    learning::Feedback,
    response::{self, Budget},
    store::check_token_budget,
};
use clap::{Parser, Subcommand};
use std::{
    io::{self, BufRead, Read},
    path::PathBuf,
    time::Instant,
};

#[derive(Parser)]
#[command(version, about = "Local source search and bounded graph context")]
struct Cli {
    #[arg(long, global = true, default_value = ".context-foundry")]
    store: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum BoundaryCommand {
    Index {
        #[arg(long)]
        config: String,
        #[arg(long)]
        path: Vec<String>,
    },
    Relations {
        #[arg(long)]
        config: String,
        #[arg(long, required = true)]
        path: Vec<String>,
        #[arg(long, default_value_t = 4096)]
        tokens: usize,
    },
}

#[derive(Subcommand)]
enum Command {
    /// Index a workspace. Respect ignore files; do not execute repository code.
    /// Initializes a store explicitly when none exists.
    Index {
        root: PathBuf,
    },
    /// Experimental indexed Python HTTP boundary publication and traversal.
    Boundary {
        #[command(subcommand)]
        command: BoundaryCommand,
    },
    /// Print v2 locator lines (one per hit) within the token budget.
    Search {
        query: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long, default_value_t = 1024)]
        tokens: usize,
        /// Restrict both search tiers to one file or directory subtree.
        #[arg(long)]
        path: Option<String>,
        /// 009 T002: fuse dense evidence from this semantic profile into
        /// the lexical candidates. Only eligible queries load the worker.
        #[arg(long = "semantic-profile", value_name = "FILE")]
        semantic_profile: Option<PathBuf>,
        /// Required to run the development-isolated worker.
        #[arg(long = "development-isolation", requires = "semantic_profile")]
        development_isolation: bool,
    },
    /// Print only the token-budgeted evidence text to stdout.
    #[command(group(
        clap::ArgGroup::new("isolated_workers")
            .multiple(true)
            .args(["semantic_profile", "policy_config"])
    ))]
    Context {
        query: String,
        #[arg(long, default_value_t = 2048)]
        tokens: usize,
        /// Write bounded private diagnostics in a NEW directory; never changes evidence.
        #[arg(long, value_name = "DIR")]
        retrieval_trace_dir: Option<PathBuf>,
        /// Test behavior/prose selection; default off pending retrieval/host validation.
        #[arg(long)]
        experimental_behavior_context: bool,
        #[arg(long, default_value = "auto")]
        strategy: String,
        /// Include this experimental task's complete selected view.
        #[arg(long, requires = "task_revision")]
        task: Option<String>,
        /// Expected task revision; refuses changed or forgotten task state.
        #[arg(long, requires = "task")]
        task_revision: Option<u64>,
        /// Compose implementation, boundary consumers, configuration and tests for the selected task.
        #[arg(long, requires = "task")]
        compose_evidence: bool,
        /// Deliver compact evidence accounting without repeating source handles.
        #[arg(long, requires = "compose_evidence")]
        compact_evidence: bool,
        /// Indexed boundary configuration used for task evidence traversal.
        #[arg(long, requires = "compose_evidence")]
        boundary_config: Option<String>,
        /// 008: append compact `mem:` lines for validated memory hits.
        #[arg(long = "include-memory")]
        include_memory: bool,
        /// Experimental source-bound derived vocabulary (anchorless queries only).
        #[arg(long)]
        include_briefs: bool,
        /// 009 T002: fuse dense evidence from this semantic profile into
        /// the lexical candidates. Only eligible queries load the worker.
        #[arg(long = "semantic-profile", value_name = "FILE")]
        semantic_profile: Option<PathBuf>,
        /// 013 T003: route `--strategy auto` with the learned policy this
        /// config v2 file selects. Validated and loaded at command start;
        /// an invalid config or a failed start keeps deterministic routing
        /// with the reason in the header.
        #[arg(long = "policy-config", value_name = "FILE")]
        policy_config: Option<PathBuf>,
        /// Required to run the development-isolated workers.
        #[arg(long = "development-isolation", requires = "isolated_workers")]
        development_isolation: bool,
        /// Removed in 013 T003 (the legacy Laya HTTP router); refused with
        /// migration guidance.
        #[arg(long = "laya-port", value_name = "PORT", hide = true)]
        laya_port: Option<String>,
    },
    /// Retrieve one exact span addressed by a v2 handle string
    /// (`path#start-end@sha32.ws16`), optionally narrowed to whole file lines.
    Retrieve {
        #[arg(long)]
        handle: String,
        #[arg(long, default_value_t = 2048)]
        tokens: usize,
        #[arg(long)]
        lines: Option<String>,
        /// `text` (bounded prefix with `next:`) or `outline` (never paginated).
        #[arg(long, default_value = "text")]
        view: String,
    },
    /// Traverse imported file relationships; symbol labels are producer-supplied.
    Graph {
        path: String,
        #[arg(long)]
        reverse: bool,
        #[arg(long, default_value_t = 1)]
        depth: usize,
        #[arg(long, default_value_t = 64)]
        edges: usize,
    },
    ImportGraph {
        bundle: PathBuf,
    },
    /// Import a completed producer execution artifact; never runs its command.
    ImportExecution {
        artifact: PathBuf,
    },
    /// 005: import one completed SCIP artifact bound by a snapshot manifest
    /// (explicit, CLI-only; the importer runs no compiler). Exits 1 for a
    /// partial import, 2 for an invalid invocation, 130 when cancelled.
    ImportScip {
        /// The completed SCIP artifact (a rust-analyzer `scip` output).
        #[arg(long)]
        index: PathBuf,
        /// The snapshot manifest v1 (JSON) that binds the artifact.
        #[arg(long)]
        snapshot: PathBuf,
    },
    /// 005: budgeted compiler references of one symbol, as v2 text. Seed with
    /// `--symbol-id` (16 hex), with `--handle` plus `--byte-offset`, or with
    /// `--handle` alone: the symbols of its definition unit (context-v2
    /// § Doors).
    #[command(group(clap::ArgGroup::new("seed").required(true).args(["symbol_id", "handle"])))]
    References {
        #[arg(long)]
        symbol_id: Option<String>,
        #[arg(long)]
        handle: Option<String>,
        /// Absolute byte offset inside the handle's range.
        #[arg(long, requires = "handle")]
        byte_offset: Option<u64>,
        #[arg(long, default_value_t = REFERENCES_DEFAULT_LIMIT)]
        limit: usize,
        #[arg(long, default_value_t = 1024)]
        tokens: usize,
        /// Continue strictly after `<path>#<start>-<end>` (the `next: after=` value).
        #[arg(long)]
        after: Option<String>,
    },
    /// Accept one explicitly labeled feedback JSON object from stdin. The
    /// default is the legacy v1 shape (kept exportable, never trainable);
    /// `feedback v4` reads one 013 v4 row with exact inputs and rights.
    Feedback {
        #[command(subcommand)]
        action: Option<FeedbackAction>,
    },
    /// 013: prepare and check the exact permitted dataset; train a candidate.
    Learning {
        #[command(subcommand)]
        action: LearningAction,
    },
    /// Emit opted-in local examples as JSONL. Redirect outside the source checkout.
    ExportTraining,
    /// Resume pending derived-index work after an interrupted indexing command.
    Refresh,
    /// Store metadata; never touches the filesystem on a missing store.
    /// `policy` reports the learned routing policy: `disabled`, or the
    /// `--policy-config` file validated as an owner would (no worker starts).
    Status {
        #[arg(long = "policy-config", value_name = "FILE")]
        policy_config: Option<PathBuf>,
    },
    /// Explicit schema upgrade under exclusive ownership; target is schema 12.
    UpgradeStore {
        #[arg(long)]
        to: u32,
    },
    /// Explicit derived-index repair: durable marker, one quarantine, paged rebuild.
    RepairIndex,
    /// 009: optional semantic retrieval — prepare, status and purge.
    Semantic {
        #[command(subcommand)]
        action: SemanticAction,
    },
    /// Selectively generate source briefs and area knowledge under an explicit profile.
    Knowledge {
        #[command(subcommand)]
        action: KnowledgeAction,
    },
    /// Experimental task state: one scoped JSON request on stdin.
    TaskContext,
    /// Experimental source brief: one bounded JSON request on stdin.
    Brief,
    /// 008 explicit project memory: put, update, get, forget, search, export.
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    #[command(flatten)]
    Adapter(crate::adapter_cli::AdapterCommand),
}

#[derive(Subcommand)]
enum KnowledgeAction {
    Prepare {
        #[arg(long)]
        profile: PathBuf,
        #[arg(long, default_value_t = 300)]
        budget_seconds: u64,
        #[arg(long, default_value_t = 8)]
        max_calls: u32,
        /// o200k tokens of serialized requests, distinct from provider usage.
        #[arg(long, default_value_t = 32768)]
        request_tokens: u64,
        /// Sum of requested provider completion ceilings.
        #[arg(long, default_value_t = 16384)]
        output_tokens: u64,
    },
    Status,
    PurgeCache,
}

#[derive(Subcommand)]
enum SemanticAction {
    /// Bounded preparation under exclusive ownership. The budget counts
    /// from argument parsing; preparation resumes from committed cache
    /// entries and commits per batch.
    Prepare {
        /// The bounded versioned profile naming the model, worker and
        /// recipe (templates, card limit, dimension), verified before any
        /// worker starts.
        #[arg(long)]
        profile: PathBuf,
        /// Work budget in seconds.
        #[arg(long)]
        budget_seconds: u64,
        /// Run the owner-authorized development isolation profile; never
        /// advertised as production isolation.
        #[arg(long)]
        development_isolation: bool,
        /// Bytes of vector cache this store may hold (default 2 GiB); at
        /// the cap preparation stops `cache_full`. Recorded in `status`.
        #[arg(
            long = "cache-cap",
            value_name = "BYTES",
            value_parser = clap::value_parser!(u64).range(1..)
        )]
        cache_cap: Option<u64>,
        /// Prioritize an admitted source file or subtree; repeat up to 16 times.
        #[arg(long = "priority", value_name = "PATH")]
        priority_paths: Vec<String>,
    },
    /// Metadata-only readiness: never loads the tokenizer, the worker, the
    /// model or a broken profile.
    Status {
        /// Explicit full source/cache census and generation integrity validation.
        #[arg(long)]
        audit: bool,
    },
    /// Offline removal of every semantic partition, cache entry and index
    /// generation under sole ownership. Sources, graph, memory and feedback
    /// stay; nothing repopulates.
    Purge,
}

#[derive(Subcommand)]
enum FeedbackAction {
    /// Record one 013 v4 row (exact state/options/consent, <=24 KiB,
    /// strict). Consent for training can be granted ONLY here — the
    /// operator CLI — never over MCP.
    V4,
}

#[derive(Subcommand)]
enum LearningAction {
    /// Prepare the immutable schema-4 dataset under exclusive ownership:
    /// renders every permitted row through the exact pinned renderer,
    /// enforces duplicate/split/lineage rules and group floors, and writes
    /// the output through an owned partial sibling (manifest last).
    Prepare {
        /// Output directory; must not exist and must not sit under the
        /// admitted source root.
        #[arg(long)]
        out: PathBuf,
        /// The v2 run policy pinning recipe, seed and the tokenizer.
        #[arg(long)]
        policy: PathBuf,
        /// A prior dataset manifest this round builds on (base replay).
        #[arg(long)]
        parent: Option<PathBuf>,
    },
    /// Verify a prepared dataset read-back through the exact pinned
    /// renderer: manifest bytes, bounds, sort order, dataset identity, group
    /// ownership, coverage floors and every row's token IDs and markers. The
    /// policy's tokenizer is required; there is no structural-only pass.
    Check {
        #[arg(long)]
        manifest: PathBuf,
        /// The v2 run policy pinning the tokenizer the rows are re-rendered
        /// with.
        #[arg(long)]
        policy: PathBuf,
    },
    /// 013 T002: fit the head on the dataset's train rows, calibrate one
    /// temperature, evaluate against deterministic routing (and an
    /// incumbent), and publish an immutable candidate, under exclusive
    /// store ownership for the whole run. Exits 0 completed (eligible or
    /// not, named), 2 invalid/preflight, 3 busy, 1 execution or artifact
    /// failure, 130 cancelled.
    Train {
        /// The prepared dataset's manifest.
        #[arg(long)]
        input: PathBuf,
        /// The same v2 run policy the dataset was prepared under.
        #[arg(long)]
        policy: PathBuf,
        /// The candidate directory; must not exist (or hold exactly this
        /// run's candidate) and must not sit under the source root.
        #[arg(long)]
        out: PathBuf,
        /// The accepted candidate the policy pins as the base.
        #[arg(long)]
        base: Option<PathBuf>,
        /// A compatible candidate to compare against on the same rows.
        #[arg(long)]
        incumbent: Option<PathBuf>,
        /// Run the owner-authorized development isolation profile; never
        /// advertised as production isolation.
        #[arg(long = "development-isolation")]
        development_isolation: bool,
    },
    /// Print the core's composed `state` for a query: the query, LF,
    /// `graph: <complete|partial>` from the current compiler graph, then up
    /// to three lexical locator lines. Exactly the state, one trailing LF,
    /// so a `feedback v4` row stores the state the policy would see. Refuses
    /// `graph_unavailable` / `graph_stale` when no current graph exists.
    ComposeState {
        #[arg(long)]
        query: String,
    },
    /// 013 T003: validate a published candidate (full read-back, identity,
    /// report digest, the fitted scalar and no inherited temperature,
    /// eligibility, the CURRENT consent of every example it used, and a net
    /// task-checker evidence gain over deterministic routing: the economics
    /// gate, `economics_unknown` / `candidate_no_benefit`) and write a NEW
    /// config v2 file naming it; an existing file is never
    /// overwritten (`output_exists`). Install it with `mcp --policy-config
    /// FILE` (or `context --policy-config FILE`) and a restart; roll back by
    /// restarting with the prior file or none. Exits 0 selected, 2 invalid
    /// or refused, 3 busy, 1 write failure.
    Select {
        #[arg(long)]
        candidate: PathBuf,
        /// The learning-worker isolation profile the policy will serve under.
        #[arg(long = "isolation-profile")]
        isolation_profile: PathBuf,
        /// The new config file.
        #[arg(long)]
        out: PathBuf,
        /// Lifecycle and package verification only (013 T004, D001): select an
        /// otherwise valid candidate that fails the economics gate. The config
        /// records `"lifecycle_check": true`; it is not enablement and must
        /// never ship as a default config.
        #[arg(long = "lifecycle-check")]
        lifecycle_check: bool,
    },
}

#[derive(Subcommand)]
enum MemoryAction {
    /// Create a record from one JSON object on stdin (<=64 KiB, strict).
    Put,
    /// Replace a record (with expected_revision) from one JSON object on stdin.
    Update,
    /// Print the live record with exact text and per-link freshness.
    Get {
        #[arg(long)]
        id: String,
        #[arg(long)]
        workspace_id: String,
    },
    /// Remove a record; prints a content-free outcome report.
    Forget {
        #[arg(long)]
        id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        workspace_id: String,
    },
    /// v2 text: the header plus one compact `mem:` line per validated hit.
    Search {
        query: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long, default_value_t = 1024)]
        tokens: usize,
        #[arg(long)]
        workspace_id: String,
    },
    /// JSONL page of live records on stdout; counts-only metadata on stderr.
    Export {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        after_id: Option<String>,
        #[arg(long, default_value_t = 128)]
        limit: usize,
    },
}

#[cfg(unix)]
mod cancel {
    use std::ptr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

    static FLAG: AtomicPtr<AtomicBool> = AtomicPtr::new(ptr::null_mut());

    extern "C" fn on_sigint(_: libc::c_int) {
        let flag = FLAG.load(Ordering::SeqCst);
        if !flag.is_null() {
            // SAFETY: the pointer came from Arc::into_raw and is never freed.
            unsafe { (*flag).store(true, Ordering::SeqCst) };
        }
    }

    /// SIGINT cooperatively cancels the active operation; exit is 130.
    pub fn install(flag: Arc<AtomicBool>) {
        let raw = Arc::into_raw(flag) as *mut AtomicBool;
        FLAG.store(raw, Ordering::SeqCst);
        // SAFETY: conventional signal(2) registration.
        unsafe {
            libc::signal(
                libc::SIGINT,
                on_sigint as extern "C" fn(libc::c_int) as libc::sighandler_t,
            )
        };
    }
}

#[cfg(not(unix))]
mod cancel {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    pub fn install(_flag: Arc<AtomicBool>) {}
}

pub(crate) fn live_control() -> Control {
    let control = Control::unbounded();
    cancel::install(control.cancel_flag());
    control
}

fn parse_strategy(raw: &str) -> FResult<Strategy> {
    match raw {
        "auto" => Ok(Strategy::Auto),
        "search" => Ok(Strategy::Search),
        "graph" => Ok(Strategy::Graph),
        other => Err(FoundryError::UnsupportedMode(format!(
            "unknown strategy {other:?}; expected auto, search or graph"
        ))),
    }
}

/// `--laya-port` (and the legacy HTTP router it enabled) was removed in 013
/// T003. Host configurations are never rewritten; the message says what to
/// run instead.
fn removed_laya_port() -> FoundryError {
    FoundryError::UnsupportedMode(
        "--laya-port was removed with the legacy Laya HTTP router (013 T003). Context \
         routing is deterministic by default; to serve a learned policy, select a trained \
         candidate with `foundry learning select --candidate DIR --isolation-profile FILE \
         --out CONFIG` and pass `--policy-config CONFIG --development-isolation` (see \
         docs/learning.md). Remove --laya-port from your host configuration; Foundry does \
         not edit it."
            .into(),
    )
}

/// The `--semantic-profile FILE [--development-isolation]` flags as the
/// launch configuration the semantic path starts from.
fn semantic_launch(
    profile: Option<PathBuf>,
    development: bool,
) -> Option<crate::mcp::SemanticServing> {
    profile.map(|profile| crate::mcp::SemanticServing::new(profile, development))
}

/// 008 memory CLI. Mutating record inputs are one bounded JSON object on
/// stdin (<=64 KiB, strict); get/forget/search/export take flags. Every
/// command parses through the same strict request parser as the MCP tool.
/// Memory ops never need the lexical index and open authoritative-only, so
/// they work while search is broken (search itself needs the index).
fn memory_run(store: &std::path::Path, action: MemoryAction) -> AResult<()> {
    use crate::memory::MemoryRequest;
    use serde_json::Value;
    let object = match &action {
        MemoryAction::Put | MemoryAction::Update => {
            let mut bytes = Vec::new();
            io::stdin()
                .lock()
                .take(crate::memory::MAX_REQUEST_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > crate::memory::MAX_REQUEST_BYTES {
                return Err(FoundryError::InvalidArgument(format!(
                    "memory request exceeds {} bytes",
                    crate::memory::MAX_REQUEST_BYTES
                ))
                .into());
            }
            let text = String::from_utf8(bytes).map_err(|_| {
                FoundryError::InvalidArgument("memory request must be UTF-8".into())
            })?;
            let value: Value = serde_json::from_str(&text).map_err(|e| {
                FoundryError::InvalidArgument(format!("memory request is not one JSON object: {e}"))
            })?;
            let Value::Object(mut object) = value else {
                return Err(FoundryError::InvalidArgument(
                    "memory request must be one JSON object".into(),
                )
                .into());
            };
            let op = match &action {
                MemoryAction::Put => "put",
                _ => "update",
            };
            // `op` is injected only when absent; a present value must be the
            // exact subcommand string — null, other types and mismatches are
            // refused before the store is opened.
            match object.get("op") {
                None => {
                    object.insert("op".into(), Value::String(op.into()));
                }
                Some(Value::String(named)) if named == op => {}
                Some(other) => {
                    return Err(FoundryError::InvalidArgument(format!(
                        "memory request names op {other:?} but the subcommand is {op:?}"
                    ))
                    .into());
                }
            }
            object
        }
        MemoryAction::Get { id, workspace_id } => {
            let mut object = serde_json::Map::new();
            object.insert("op".into(), Value::String("get".into()));
            object.insert("id".into(), Value::String(id.clone()));
            object.insert("workspace_id".into(), Value::String(workspace_id.clone()));
            object
        }
        MemoryAction::Forget {
            id,
            expected_revision,
            workspace_id,
        } => {
            let mut object = serde_json::Map::new();
            object.insert("op".into(), Value::String("forget".into()));
            object.insert("id".into(), Value::String(id.clone()));
            object.insert(
                "expected_revision".into(),
                Value::Number((*expected_revision).into()),
            );
            object.insert("workspace_id".into(), Value::String(workspace_id.clone()));
            object
        }
        MemoryAction::Search {
            query,
            limit,
            tokens,
            workspace_id,
        } => {
            let mut object = serde_json::Map::new();
            object.insert("op".into(), Value::String("search".into()));
            object.insert("query".into(), Value::String(query.clone()));
            object.insert("limit".into(), Value::Number((*limit).into()));
            object.insert("tokens".into(), Value::Number((*tokens).into()));
            object.insert("workspace_id".into(), Value::String(workspace_id.clone()));
            object
        }
        MemoryAction::Export {
            workspace_id,
            after_id,
            limit,
        } => {
            // Export is CLI-only: a sorted JSONL page on stdout, counts-only
            // metadata on stderr, authoritative-only, one read snapshot.
            let engine = Engine::open_authoritative(store)?;
            let page = engine.memory_export(workspace_id, after_id.as_deref(), *limit)?;
            for row in &page.rows {
                print!("{row}");
            }
            eprintln!(
                "{}",
                serde_json::json!({
                    "rows": page.rows.len(),
                    "bytes": page.bytes,
                    "next_after_id": page.next_after_id,
                })
            );
            return Ok(());
        }
    };
    match crate::memory::parse_request(&object)? {
        MemoryRequest::Put(input) => {
            // The healthy-index open lets the mutation apply its own pending
            // key afterwards; a broken index still opens and leaves it queued.
            let mut engine = Engine::open_existing(store)?;
            let report = engine.memory_put(&input)?;
            engine.drain_memory_key(&report.id);
            println!("{}", serde_json::to_string(&report)?);
        }
        MemoryRequest::Update(input) => {
            let mut engine = Engine::open_existing(store)?;
            let report = engine.memory_update(&input)?;
            engine.drain_memory_key(&report.id);
            println!("{}", serde_json::to_string(&report)?);
        }
        MemoryRequest::Get { id, workspace_id } => {
            let engine = Engine::open_authoritative(store)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&engine.memory_get(&id, &workspace_id)?)?
            );
        }
        MemoryRequest::Forget(input) => {
            let control = live_control();
            let mut engine = Engine::open_existing(store)?;
            let report = engine.memory_forget(&input, &control)?;
            engine.drain_memory_key(&report.id);
            println!("{}", serde_json::to_string(&report)?);
        }
        MemoryRequest::Search(input) => {
            let engine = Engine::open_existing(store)?;
            let outcome = engine.memory_search(&input)?;
            let packed = response::pack_memory_search(
                &outcome,
                Budget::request(input.tokens),
                &response::stdout_bytes,
            )?;
            print!("{}", packed.text);
            eprintln!(
                "{}",
                serde_json::json!({
                    "stdout_tokens": packed.tokens,
                    "tokenizer": response::TOKENIZER,
                    "omitted": packed.omitted,
                })
            );
        }
    }
    Ok(())
}

/// `index` is the explicit initializer for a missing store; every other
/// command opens existing state only.
fn open_or_initialize(store: &std::path::Path, root: &std::path::Path) -> FResult<Engine> {
    if store.join("knowledge.redb").is_file() {
        Engine::open_existing(store)
    } else {
        Engine::initialize(store, root)
    }
}

/// The CLI entry point, shared by the `foundry` binary and (behind the
/// `test-faults` feature) the fault-arming test twin.
pub fn main() {
    if let Err(error) = run() {
        eprintln!("{}", error.bounded_json());
        std::process::exit(error.exit_code());
    }
}

fn run() -> AResult<()> {
    use clap::{CommandFactory, FromArgMatches};
    // 009: the preparation budget counts from argument parsing.
    let semantic_started = std::time::Instant::now();
    let matches = Cli::command().get_matches();
    // Bootstrap/connect default the store to `<canonical-root>/.context-foundry`
    // unless --store was actually supplied.
    let store_explicit =
        matches.value_source("store") != Some(clap::parser::ValueSource::DefaultValue);
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    match cli.command {
        Command::Semantic { action } => semantic_run(&cli.store, action, semantic_started)?,
        Command::Adapter(command) => crate::adapter_cli::run(command, cli.store, store_explicit)?,
        Command::Memory { action } => memory_run(&cli.store, action)?,
        Command::Knowledge { action } => match action {
            KnowledgeAction::Prepare {
                profile,
                budget_seconds,
                max_calls,
                request_tokens,
                output_tokens,
            } => {
                if !(1..=3600).contains(&budget_seconds) {
                    return Err(crate::knowledge::fail(
                        "knowledge_budget_invalid",
                        "budget-seconds must be 1..3600",
                    )
                    .into());
                }
                let control = live_control()
                    .bounded_by(semantic_started + std::time::Duration::from_secs(budget_seconds));
                let profile = crate::knowledge::Profile::load(&profile)?;
                let mut engine = Engine::open_existing(&cli.store)?;
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                let report = runtime.block_on(crate::knowledge::prepare(
                    &mut engine,
                    &profile,
                    crate::knowledge::RunLimits {
                        calls: max_calls,
                        request_tokens,
                        output_tokens,
                    },
                    &control,
                ))?;
                println!("{}", serde_json::to_string(&report)?);
                if !report.complete {
                    return Err(crate::knowledge::fail(
                        "knowledge_incomplete",
                        "generation is incomplete; see the report; committed results are retained",
                    )
                    .into());
                }
            }
            KnowledgeAction::Status => println!(
                "{}",
                Engine::open_authoritative(&cli.store)?.knowledge_status()?
            ),
            KnowledgeAction::PurgeCache => println!(
                "{}",
                Engine::open_authoritative(&cli.store)?.knowledge_purge_cache()?
            ),
        },
        Command::Brief => {
            let mut bytes = Vec::new();
            io::stdin()
                .lock()
                .take(crate::brief::MAX_REQUEST_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            let request = crate::brief::parse(&bytes)?;
            let mut engine = if matches!(
                request.command,
                crate::brief::Command::Get { .. } | crate::brief::Command::Export { .. }
            ) {
                Engine::open_authoritative(&cli.store)?
            } else {
                Engine::open_existing(&cli.store)?
            };
            let result = engine.brief(&request, &live_control())?;
            if matches!(
                request.command,
                crate::brief::Command::Put { .. } | crate::brief::Command::Forget { .. }
            ) && let Some(id) = result["id"].as_str()
            {
                engine.drain_brief_key(id);
            }
            println!("{}", serde_json::to_string(&result)?);
        }
        Command::TaskContext => {
            let mut bytes = Vec::new();
            io::stdin()
                .lock()
                .take(crate::task_context::MAX_REQUEST_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > crate::task_context::MAX_REQUEST_BYTES {
                return Err(
                    FoundryError::InvalidArgument("task request exceeds 1 MiB".into()).into(),
                );
            }
            let value = serde_json::from_slice(&bytes).map_err(|_| {
                FoundryError::InvalidArgument("expected one task JSON request".into())
            })?;
            let request = crate::task_context::parse(value)?;
            let mut engine = if matches!(
                request.command,
                crate::task_context::Command::Promote { .. }
                    | crate::task_context::Command::Forget { .. }
            ) {
                Engine::open_existing(&cli.store)?
            } else {
                Engine::open_authoritative(&cli.store)?
            };
            let result = engine.task_context(&request, &live_control())?;
            engine.drain_task_memory(&result);
            println!("{}", serde_json::to_string(&result)?);
        }
        Command::Index { root } => {
            let control = live_control();
            let mut engine = open_or_initialize(&cli.store, &root)?;
            let report = engine.index(&root, &control)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if report.partial
                && let Some(error) = report.index_error()
            {
                return Err(error.into());
            }
        }
        Command::Search {
            query,
            limit,
            tokens,
            path,
            semantic_profile,
            development_isolation,
        } => {
            check_token_budget(tokens)?;
            let engine = Engine::open_existing(&cli.store)?;
            let semantic = semantic_launch(semantic_profile, development_isolation);
            let outcome = if let Some(launch) = semantic {
                let slot = crate::mcp::cli_semantic_slot(
                    Some(launch),
                    &engine,
                    &query,
                    path.as_deref(),
                    false,
                    &Control::with_deadline(Instant::now() + crate::mcp::READ_DEADLINE),
                )?;
                let control = Control::with_deadline(Instant::now() + crate::mcp::READ_DEADLINE);
                Engine::search_outcome(crate::mcp::search_primary(
                    &slot,
                    &engine,
                    &query,
                    path.as_deref(),
                    limit,
                    &control,
                    None,
                )?)
            } else {
                engine.search_in(&query, path.as_deref(), limit)?
            };
            let packed =
                response::pack_search(&outcome, Budget::request(tokens), &response::stdout_bytes)?;
            print!("{}", packed.text);
            eprintln!(
                "{}",
                serde_json::json!({
                    "stdout_tokens": packed.tokens,
                    "tokenizer": response::TOKENIZER,
                    "omitted": packed.omitted,
                })
            );
        }
        Command::Context {
            query,
            tokens,
            retrieval_trace_dir,
            experimental_behavior_context,
            strategy,
            include_memory,
            include_briefs,
            task,
            task_revision,
            compose_evidence,
            compact_evidence,
            boundary_config,
            semantic_profile,
            policy_config,
            development_isolation,
            laya_port,
        } => {
            if laya_port.is_some() {
                return Err(removed_laya_port().into());
            }
            let strategy = parse_strategy(&strategy)?;
            check_token_budget(tokens)?;
            let trace_sink = retrieval_trace_dir
                .as_deref()
                .map(crate::retrieval_trace::TraceSink::create)
                .transpose()?;
            let trace = trace_sink
                .as_ref()
                .map(|_| crate::retrieval_trace::RetrievalTrace::default());
            let result = (|| -> AResult<_> {
                let engine = Engine::open_existing(&cli.store)?;
                // 008: memory candidates validate in the SAME final read as the
                // source and graph items; one snapshot governs the response.
                // The shared anchor/prose rule can skip unused semantic startup.
                // Otherwise loading keeps its separate profile timeout and a
                // refusal leaves baseline results with the named fallback.
                let semantic = semantic_launch(semantic_profile, development_isolation);
                // 013 T003: the policy config is validated and its worker loaded
                // at command start, outside the request's read deadline; an
                // invalid config or a failed start keeps deterministic routing
                // with the reason in the header. No config, or a disabled one,
                // leaves the output byte-identical.
                let policy = policy_config.map(|config| {
                    crate::policy::Policy::start(
                        Some(crate::policy::PolicyServing::new(
                            config,
                            development_isolation,
                        )),
                        engine.workspace_id().as_deref(),
                    )
                });
                let routed = policy.as_ref().filter(|policy| policy.routes());
                let bounded_query = semantic.is_some() || routed.is_some();
                // Worker admission/load stays outside the query deadline.
                let slot = crate::mcp::cli_semantic_slot(
                    semantic,
                    &engine,
                    &query,
                    None,
                    experimental_behavior_context
                        && crate::store::prose_context_query(&query, strategy, routed.is_some()),
                    &Control::with_deadline(Instant::now() + crate::mcp::READ_DEADLINE)
                        .with_trace(trace.clone()),
                )?;
                let control = if bounded_query {
                    Control::with_deadline(Instant::now() + crate::mcp::READ_DEADLINE)
                } else {
                    Control::unbounded()
                }
                .with_trace(trace.clone());
                let workspace_id = engine
                    .workspace_id()
                    .ok_or(FoundryError::WorkspaceUnbound)?;
                let task_scope = task.as_deref().map(|task_id| crate::task_context::Scope {
                    workspace_id: &workspace_id,
                    task_id,
                    expected_revision: task_revision.expect("clap requires paired task revision"),
                });
                let combined = crate::mcp::context_primary_with_options(
                    &slot,
                    &engine,
                    &query,
                    strategy,
                    &control,
                    crate::store::ContextOptions {
                        task: task_scope,
                        compose_evidence,
                        compact_evidence,
                        boundary_config: boundary_config.as_deref(),
                        memory: include_memory,
                        briefs: include_briefs,
                        policy: routed,
                        experimental_behavior: experimental_behavior_context,
                        ..crate::store::ContextOptions::default()
                    },
                )?;
                let packed = response::pack_enriched_context(
                    &combined,
                    None,
                    Budget::request(tokens),
                    &response::stdout_bytes,
                )?;
                Ok((packed, combined.batch.doors.is_some()))
            })();
            if let (Some(sink), Some(trace)) = (&trace_sink, &trace) {
                let outcome = match &result {
                    Ok((packed, _)) => serde_json::json!({"status":"success", "text":packed.text,
                        "tokens":packed.tokens, "sha256":crate::digest(packed.text.as_bytes())}),
                    Err(error) => {
                        serde_json::json!({"status":"error", "error":error.bounded_json()})
                    }
                };
                sink.save("context", trace, outcome)?;
            }
            let (packed, graph) = result?;
            print!("{}", packed.text);
            eprintln!(
                "{}",
                serde_json::json!({
                    "stdout_tokens": packed.tokens,
                    "tokenizer": response::TOKENIZER,
                    "omitted": packed.omitted,
                    "strategy": if graph { "graph" } else { "search" },
                })
            );
        }
        Command::Retrieve {
            handle,
            tokens,
            lines,
            view,
        } => {
            let outline = match view.as_str() {
                "text" => false,
                "outline" => true,
                other => {
                    return Err(FoundryError::InvalidArgument(format!(
                        "--view must be text or outline, not {other:?}"
                    ))
                    .into());
                }
            };
            let engine = Engine::open_existing(&cli.store)?;
            let packed = if outline {
                response::pack_retrieve_outline(
                    &engine.retrieve_outline(&handle, lines.as_deref(), tokens)?,
                    Budget::request(tokens),
                    &response::stdout_bytes,
                )?
            } else {
                response::pack_retrieve(
                    &engine.retrieve(&handle, lines.as_deref(), tokens)?,
                    Budget::request(tokens),
                    &response::stdout_bytes,
                )?
            };
            print!("{}", packed.text);
            eprintln!(
                "{}",
                serde_json::json!({
                    "stdout_tokens": packed.tokens,
                    "tokenizer": response::TOKENIZER,
                    "truncated": packed.truncated,
                })
            );
        }
        Command::Boundary { command } => {
            let engine = Engine::open_existing(&cli.store)?;
            let control = live_control();
            match command {
                BoundaryCommand::Index { config, path } => println!(
                    "{}",
                    serde_json::to_string(&engine.index_boundaries(&config, &path, &control)?)?
                ),
                BoundaryCommand::Relations {
                    config,
                    path,
                    tokens,
                } => {
                    check_token_budget(tokens)?;
                    let report = engine.boundary_relations(&config, &path, &control)?;
                    print!(
                        "{}",
                        response::pack_boundaries(
                            &report,
                            Budget::request(tokens),
                            &response::stdout_bytes
                        )?
                        .text
                    );
                }
            }
        }
        Command::Graph {
            path,
            reverse,
            depth,
            edges,
        } => {
            let engine = Engine::open_existing(&cli.store)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&engine.graph(&path, reverse, depth, edges)?)?
            );
        }
        Command::ImportGraph { bundle } => {
            let engine = Engine::open_existing(&cli.store)?;
            let mut bytes = Vec::new();
            std::fs::File::open(&bundle)?
                .take(64 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 64 * 1024 * 1024 {
                return Err(
                    FoundryError::InvalidArgument("graph bundle exceeds 64 MiB".into()).into(),
                );
            }
            let bundle: GraphBundle = serde_json::from_slice(&bytes)?;
            println!(
                "{}",
                serde_json::json!({"imported_edges": engine.import_graph(&bundle)?})
            );
        }
        Command::ImportExecution { artifact } => {
            let engine = Engine::open_existing(&cli.store)?;
            let mut bytes = Vec::new();
            std::fs::File::open(artifact)?
                .take(4 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            println!(
                "{}",
                serde_json::json!({"imported_edges":engine.import_execution(&bytes)?})
            );
        }
        Command::ImportScip { index, snapshot } => {
            let control = live_control();
            let engine = Engine::open_existing(&cli.store)?;
            let report = engine.import_scip(&index, &snapshot, &control)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            match report.interrupted.as_deref() {
                Some("cancelled") => return Err(FoundryError::Cancelled(None).into()),
                Some("deadline_exceeded") => {
                    return Err(FoundryError::DeadlineExceeded(None).into());
                }
                _ => {}
            }
            if !report.complete {
                return Err(crate::scip::fail(
                    crate::scip::code::IMPORT_INCOMPLETE,
                    format!(
                        "the import finished partial: {} sources committed, {} failed",
                        report.completed + report.accepted_empty + report.unresolved,
                        report.failed
                    ),
                )
                .into());
            }
        }
        Command::References {
            symbol_id,
            handle,
            byte_offset,
            limit,
            tokens,
            after,
        } => {
            check_token_budget(tokens)?;
            let seed = match (symbol_id, handle, byte_offset) {
                (Some(id), None, None) => ReferencesSeed::SymbolId(id),
                (None, Some(handle), Some(byte_offset)) => ReferencesSeed::Position {
                    handle,
                    byte_offset,
                },
                (None, Some(handle), None) => ReferencesSeed::Handle(handle),
                _ => {
                    return Err(FoundryError::InvalidArgument(
                        "exactly one seed form: --symbol-id, or --handle with an optional --byte-offset"
                            .into(),
                    )
                    .into());
                }
            };
            let engine = Engine::open_existing(&cli.store)?;
            let outcome = engine.references(&ReferencesRequest { seed, limit, after })?;
            let packed = response::pack_references(
                &outcome,
                Budget::request(tokens),
                &response::stdout_bytes,
            )?;
            print!("{}", packed.text);
            eprintln!(
                "{}",
                serde_json::json!({
                    "stdout_tokens": packed.tokens,
                    "tokenizer": response::TOKENIZER,
                    "omitted": packed.omitted,
                })
            );
        }
        Command::Feedback { action } => {
            let engine = Engine::open_existing(&cli.store)?;
            match action {
                None => {
                    let mut line = String::new();
                    io::stdin().lock().take(8193).read_line(&mut line)?;
                    if line.len() > 8192 {
                        return Err(FoundryError::InvalidArgument(
                            "feedback exceeds 8192 bytes".into(),
                        )
                        .into());
                    }
                    let feedback: Feedback = serde_json::from_str(&line)?;
                    println!(
                        "{}",
                        serde_json::json!({"id": engine.record_feedback(&feedback)?})
                    );
                }
                Some(FeedbackAction::V4) => {
                    // The ONLY path that may set allow_training=true.
                    let mut line = String::new();
                    io::stdin()
                        .lock()
                        .take(24 * 1024 + 1)
                        .read_line(&mut line)?;
                    if line.len() > 24 * 1024 {
                        return Err(FoundryError::InvalidArgument(
                            "feedback v4 exceeds 24576 bytes".into(),
                        )
                        .into());
                    }
                    let (example_id, status) = engine.record_learning_feedback(line.trim_end())?;
                    println!(
                        "{}",
                        serde_json::json!({
                            "example_id": example_id,
                            "status": status,
                        })
                    );
                }
            }
        }
        Command::Learning { action } => learning_run(&cli.store, action)?,
        Command::ExportTraining => {
            let engine = Engine::open_existing(&cli.store)?;
            for row in engine.training_examples()? {
                println!("{}", serde_json::to_string(&row)?);
            }
        }
        Command::Refresh => {
            let control = live_control();
            let mut engine = Engine::open_existing(&cli.store)?;
            let n = engine.refresh(&control)?;
            println!(
                "{}",
                serde_json::json!({"refreshed_sources": n.0, "refreshed_memory": n.1})
            );
        }
        Command::Status { policy_config } => {
            let engine = Engine::open_existing(&cli.store)?;
            /// The store status plus the policy object, store fields first.
            #[derive(serde::Serialize)]
            struct WithPolicy<'a> {
                #[serde(flatten)]
                status: &'a crate::StoreStatus,
                policy: serde_json::Value,
            }
            let status = engine.status()?;
            let policy = match policy_config {
                Some(config) => {
                    crate::policy::Policy::inspect(&config, engine.workspace_id().as_deref())
                }
                None => crate::policy::Policy::off().status(),
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&WithPolicy {
                    status: &status,
                    policy,
                })?
            );
        }
        Command::UpgradeStore { to } => {
            let control = live_control();
            Engine::upgrade_store(&cli.store, to, &control)?;
            println!("{}", serde_json::json!({"schema": to}));
        }
        Command::RepairIndex => {
            let control = live_control();
            let report = Engine::repair_index(&cli.store, &control)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.repaired {
                return Err(FoundryError::RepairRequired(
                    report
                        .reason
                        .unwrap_or_else(|| "repair did not complete".into()),
                )
                .into());
            }
        }
    }
    Ok(())
}

/// 013 operator commands. `prepare` and `train` require exclusive store
/// ownership where they read feedback/permission (`train` holds it for the
/// whole run); a named `no_new_data` outcome is success (exit 0), and
/// nothing here grants MCP any consent path.
fn learning_run(store: &std::path::Path, action: LearningAction) -> AResult<()> {
    let control = live_control();
    match action {
        LearningAction::Prepare {
            out,
            policy,
            parent,
        } => {
            let engine = Engine::open_existing(store)?;
            match learning::prepare(&engine, &out, &policy, parent.as_deref(), &control)? {
                learning::PrepareOutcome::Completed(prepared) => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "outcome": "completed",
                            "dataset_id": prepared.dataset_id,
                            "manifest": prepared.manifest_path,
                            "new_rows": prepared.new_rows,
                            "replay_rows": prepared.replay_rows,
                            "train_rows": prepared.train_rows,
                            "calibration_rows": prepared.calibration_rows,
                            "evaluation_rows": prepared.evaluation_rows,
                            "groups": prepared.groups,
                            // An earlier run's identical, unrecorded output
                            // was verified and recorded (lost response).
                            "adopted": prepared.adopted,
                        })
                    );
                }
                learning::PrepareOutcome::NoNewData {
                    parent_manifest_sha256,
                } => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "outcome": "no_new_data",
                            "parent_manifest_sha256": parent_manifest_sha256,
                        })
                    );
                }
            }
        }
        LearningAction::Check { manifest, policy } => {
            let report = learning::check(&manifest, &policy, &control)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        LearningAction::Train {
            input,
            policy,
            out,
            base,
            incumbent,
            development_isolation,
        } => {
            let engine = Engine::open_existing(store)?;
            let trained = learning::train::train(
                &engine,
                &learning::train::TrainRequest {
                    input: &input,
                    policy: &policy,
                    out: &out,
                    base: base.as_deref(),
                    incumbent: incumbent.as_deref(),
                    development_isolation,
                },
                &control,
            )?;
            println!("{}", serde_json::to_string(&trained)?);
        }
        LearningAction::ComposeState { query } => {
            let engine = Engine::open_existing(store)?;
            let state = engine.compose_route_state(&query, &control)?;
            // Exactly the state plus the one LF `println!` adds.
            println!("{state}");
        }
        LearningAction::Select {
            candidate,
            isolation_profile,
            out,
            lifecycle_check,
        } => {
            let engine = Engine::open_existing(store)?;
            let selected = crate::policy::select(
                &engine,
                &crate::policy::SelectRequest {
                    candidate: &candidate,
                    isolation_profile: &isolation_profile,
                    out: &out,
                    lifecycle_check,
                },
                &control,
            )?;
            println!("{}", serde_json::to_string(&selected)?);
        }
    }
    Ok(())
}

/// 009 semantic CLI: prepare (semantic feature only; a lexical-only build
/// reports `semantic_unavailable`), status and purge.
fn semantic_run(
    store: &std::path::Path,
    action: SemanticAction,
    started: std::time::Instant,
) -> AResult<()> {
    match action {
        SemanticAction::Status { audit } => {
            let engine = Engine::open_existing(store)?;
            let status = if audit {
                serde_json::to_value(engine.semantic_status(&live_control())?)?
            } else {
                engine.semantic_progress(&live_control())?
            };
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        SemanticAction::Purge => {
            let engine = Engine::open_existing(store)?;
            let report = engine.semantic_purge()?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        SemanticAction::Prepare {
            profile,
            budget_seconds,
            development_isolation,
            cache_cap,
            priority_paths,
        } => {
            #[cfg(feature = "semantic")]
            {
                let control = live_control();
                let report = crate::neural::prepare::cli_prepare_with_priorities(
                    store,
                    &profile,
                    budget_seconds,
                    development_isolation,
                    cache_cap.unwrap_or(crate::neural::cache::DEFAULT_CACHE_CAP_BYTES),
                    started,
                    &control,
                    &priority_paths,
                )?;
                println!("{}", serde_json::to_string_pretty(&report)?);
                if let Some(error) = report.error() {
                    return Err(error.into());
                }
            }
            #[cfg(not(feature = "semantic"))]
            {
                let _ = (
                    profile,
                    budget_seconds,
                    development_isolation,
                    cache_cap,
                    priority_paths,
                    started,
                );
                return Err(FoundryError::Semantic {
                    code: "semantic_unavailable",
                    message: "this build has no semantic retrieval; rebuild with \
                              the `semantic` feature"
                        .into(),
                }
                .into());
            }
        }
    }
    Ok(())
}
