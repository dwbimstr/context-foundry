use anyhow::Result;
use clap::{Parser, Subcommand};
use context_foundry::{
    Engine,
    graph::GraphBundle,
    ingest,
    laya::{self, Feedback},
};
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
        #[arg(long)]
        laya_port: Option<u16>,
        #[arg(long, default_value_t = 0.8)]
        confidence: f64,
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
    Status,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("foundry: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let mut engine = Engine::open(&cli.store)?;
    match cli.command {
        Command::Index { root } => {
            let report = ingest::sync(&mut engine, &root)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            anyhow::ensure!(
                report.failures.is_empty(),
                "indexing incomplete; inspect failures above"
            );
        }
        Command::Search { query, limit } => println!(
            "{}",
            serde_json::to_string_pretty(&engine.search(&query, limit)?)?
        ),
        Command::Context {
            query,
            tokens,
            laya_port,
            confidence,
        } => {
            let decision = laya::decide(&query, laya_port, confidence);
            let bundle = engine.context_with_strategy(&query, tokens, decision.strategy)?;
            print!("{}", bundle.text);
            eprintln!(
                "{}",
                serde_json::json!({"stdout_tokens": bundle.tokens, "tokenizer": bundle.tokenizer, "decision": decision})
            );
        }
        Command::Graph {
            path,
            reverse,
            depth,
            edges,
        } => println!(
            "{}",
            serde_json::to_string_pretty(&engine.graph(&path, reverse, depth, edges)?)?
        ),
        Command::ImportGraph { bundle } => {
            let mut bytes = Vec::new();
            std::fs::File::open(bundle)?
                .take(64 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            anyhow::ensure!(
                bytes.len() <= 64 * 1024 * 1024,
                "graph bundle exceeds 64 MiB"
            );
            let bundle: GraphBundle = serde_json::from_slice(&bytes)?;
            println!(
                "{}",
                serde_json::json!({"imported_edges": engine.import_graph(&bundle)?})
            );
        }
        Command::Feedback => {
            let mut line = String::new();
            io::stdin().lock().take(8193).read_line(&mut line)?;
            anyhow::ensure!(line.len() <= 8192, "feedback exceeds 8192 bytes");
            let feedback: Feedback = serde_json::from_str(&line)?;
            println!(
                "{}",
                serde_json::json!({"id": engine.record_feedback(&feedback)?})
            );
        }
        Command::ExportTraining => {
            for row in engine.training_examples()? {
                println!("{}", serde_json::to_string(&row)?);
            }
        }
        Command::Refresh => {
            let mut n = 0;
            loop {
                let batch = engine.refresh_index()?;
                n += batch;
                if batch == 0 {
                    break;
                }
            }
            println!("{}", serde_json::json!({"refreshed_sources": n}));
        }
        Command::Status => println!(
            "{}",
            serde_json::json!({"sources": engine.paths()?.len(), "pending_sources": engine.pending()?, "schema": 1})
        ),
    }
    Ok(())
}
