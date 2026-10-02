//! The command-line surface. This module is ordinary release code and never
//! reads fault-environment variables.
use crate::{
    Control, Engine, FResult, FoundryError, Strategy, adapter_error::AResult, graph::GraphBundle,
    laya::Feedback, response,
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
    Search {
        query: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Print only the token-budgeted evidence text to stdout.
    Context {
        query: String,
        #[arg(long, default_value_t = 2048)]
        tokens: usize,
        #[arg(long, default_value = "auto")]
        strategy: String,
    },
    /// Retrieve one exact span addressed by a source-handle JSON object.
    Retrieve {
        #[arg(long)]
        handle: String,
        #[arg(long, default_value_t = 2048)]
        tokens: usize,
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
    /// Explicit v1 -> v2 schema upgrade under exclusive ownership.
    UpgradeStore {
        #[arg(long)]
        to: u32,
    },
    /// Explicit derived-index repair: durable marker, one quarantine, paged rebuild.
    RepairIndex,
    #[command(flatten)]
    Adapter(crate::adapter_cli::AdapterCommand),
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
        Command::Search { query, limit } => {
            let engine = Engine::open_existing(&cli.store)?;
            let mut outcome = engine.search(&query, limit)?;
            println!("{}", response::search_json(&mut outcome));
        }
        Command::Context {
            query,
            tokens,
            strategy,
        } => {
            let strategy = parse_strategy(&strategy)?;
            let engine = Engine::open_existing(&cli.store)?;
            let outcome = engine.context(&query, tokens, strategy, &Control::unbounded())?;
            let packed = response::pack_context_cli(&outcome)?;
            print!("{}", packed.text);
            eprintln!(
                "{}",
                serde_json::json!({
                    "stdout_tokens": packed.tokens,
                    "tokenizer": response::TOKENIZER,
                    "omitted": packed.omitted,
                    "strategy": outcome.strategy.to_string(),
                })
            );
        }
        Command::Retrieve { handle, tokens } => {
            let engine = Engine::open_existing(&cli.store)?;
            let outcome = engine.retrieve(&handle, tokens)?;
            let packed = response::pack_retrieve_cli(&outcome)?;
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
            println!("{}", serde_json::json!({"refreshed_sources": n}));
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
