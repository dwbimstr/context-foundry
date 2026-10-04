//! The command-line surface. This module is ordinary release code and never
//! reads fault-environment variables.
use crate::{
    Control, Engine, FResult, FoundryError, Strategy,
    adapter_error::AResult,
    graph::GraphBundle,
    laya::Feedback,
    response::{self, Budget},
    store::check_token_budget,
};
use clap::{Parser, Subcommand};
use std::{
    io::{self, BufRead, Read},
    path::PathBuf,
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
enum Command {
    /// Index a workspace. Respect ignore files; do not execute repository code.
    /// Initializes a store explicitly when none exists.
    Index {
        root: PathBuf,
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
    },
    /// Print only the token-budgeted evidence text to stdout.
    Context {
        query: String,
        #[arg(long, default_value_t = 2048)]
        tokens: usize,
        #[arg(long, default_value = "auto")]
        strategy: String,
        /// 008: append compact `mem:` lines for validated memory hits.
        #[arg(long = "include-memory")]
        include_memory: bool,
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
    /// Accept one explicitly labeled feedback JSON object from stdin.
    Feedback,
    /// Emit opted-in local examples as JSONL. Redirect outside the source checkout.
    ExportTraining,
    /// Resume pending derived-index work after an interrupted indexing command.
    Refresh,
    /// Store metadata; never touches the filesystem on a missing store.
    Status,
    /// Explicit v1|v2 -> v3 schema upgrade under exclusive ownership.
    UpgradeStore {
        #[arg(long)]
        to: u32,
    },
    /// Explicit derived-index repair: durable marker, one quarantine, paged rebuild.
    RepairIndex,
    /// 008 explicit project memory: put, update, get, forget, search, export.
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    #[command(flatten)]
    Adapter(crate::adapter_cli::AdapterCommand),
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

fn live_control() -> Control {
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
    let matches = Cli::command().get_matches();
    // Bootstrap/connect default the store to `<canonical-root>/.context-foundry`
    // unless --store was actually supplied.
    let store_explicit =
        matches.value_source("store") != Some(clap::parser::ValueSource::DefaultValue);
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    match cli.command {
        Command::Adapter(command) => crate::adapter_cli::run(command, cli.store, store_explicit)?,
        Command::Memory { action } => memory_run(&cli.store, action)?,
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
        } => {
            check_token_budget(tokens)?;
            let engine = Engine::open_existing(&cli.store)?;
            let outcome = engine.search_in(&query, path.as_deref(), limit)?;
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
            strategy,
            include_memory,
        } => {
            let strategy = parse_strategy(&strategy)?;
            check_token_budget(tokens)?;
            let engine = Engine::open_existing(&cli.store)?;
            // 008: memory candidates validate in the SAME final read as the
            // source and graph items; one snapshot governs the response.
            let (batch, memory) = if include_memory {
                let combined =
                    engine.context_candidates_memory(&query, strategy, &Control::unbounded())?;
                (combined.batch, combined.hits)
            } else {
                (
                    engine.context_candidates(&query, strategy, &Control::unbounded())?,
                    Vec::new(),
                )
            };
            let packed = if memory.is_empty() {
                response::pack_context(&batch, Budget::request(tokens), &response::stdout_bytes)?
            } else {
                response::pack_context_with_memory(
                    &batch,
                    &memory,
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
                    "omitted": packed.omitted,
                    "strategy": if batch.counters.graph.is_some() { "graph" } else { "search" },
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
        Command::Feedback => {
            let engine = Engine::open_existing(&cli.store)?;
            let mut line = String::new();
            io::stdin().lock().take(8193).read_line(&mut line)?;
            if line.len() > 8192 {
                return Err(
                    FoundryError::InvalidArgument("feedback exceeds 8192 bytes".into()).into(),
                );
            }
            let feedback: Feedback = serde_json::from_str(&line)?;
            println!(
                "{}",
                serde_json::json!({"id": engine.record_feedback(&feedback)?})
            );
        }
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
        Command::Status => {
            let engine = Engine::open_existing(&cli.store)?;
            println!("{}", serde_json::to_string_pretty(&engine.status()?)?);
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
