//! The 003 adapter commands (`mcp`, `bootstrap`, `connect`, `usage`,
//! `gateway`, `gateway-omp`). The core CLI flattens [`AdapterCommand`] into
//! its one `clap` parser and hands the parsed command to [`run`]; nothing
//! here opens a store except `mcp` and `bootstrap --apply`, through the
//! library modules. The gateway commands never open a store.
use crate::{FResult, FoundryError, adapter_error::AResult};
use clap::Subcommand;
use std::{io::Read, path::PathBuf};

#[derive(Subcommand)]
pub enum AdapterCommand {
    /// Serve the seven MCP tools
    /// (search/context/retrieve/index/status/memory/references): stdio by
    /// default, or one shared owner at /mcp with --transport
    /// streamable-http.
    Mcp {
        /// Repository root; canonicalized and bound once.
        #[arg(long)]
        root: PathBuf,
        /// `stdio` (default) or `streamable-http`.
        #[arg(long, default_value = "stdio")]
        transport: String,
        /// IPv4 loopback bind for streamable-http, e.g. 127.0.0.1:9633 (port 0 = free port).
        #[arg(long)]
        bind: Option<String>,
        /// Environment variable NAME carrying the HTTP bearer secret (never the secret itself).
        #[arg(long = "auth-token-env")]
        auth_token_env: Option<String>,
        /// Budget policy file: {"foundry_budget": {"v":1, ...}} (config v1).
        #[arg(long)]
        budget: Option<PathBuf>,
        /// Admit an outside repository as `ROOT=STORE` (repeat for up to 8
        /// references); validated and opened once before serving.
        #[arg(long = "reference", value_name = "ROOT=STORE")]
        reference: Vec<String>,
        /// Omit the `memory` tool and refuse `include_memory`; records are
        /// never touched by disabling.
        #[arg(long = "no-memory")]
        no_memory: bool,
    },
    /// Inspect repository bootstrap; `--apply` creates/opens the store and indexes baseline source.
    Bootstrap {
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        apply: bool,
        #[arg(long, value_delimiter = ',', default_value = "lexical")]
        components: Vec<String>,
        #[arg(long = "graph-index")]
        graph_index: Option<PathBuf>,
        #[arg(long = "graph-snapshot")]
        graph_snapshot: Option<PathBuf>,
        #[arg(long)]
        profile: Option<PathBuf>,
    },
    /// Print per-project host MCP configuration; never edits host files unless
    /// `--apply-config FILE` authorizes the positively owned block.
    Connect {
        #[arg(long)]
        host: String,
        #[arg(long)]
        root: PathBuf,
        #[arg(long = "print-config")]
        print_config: bool,
        /// Authorize inserting the owned block into this host config file.
        #[arg(long = "apply-config")]
        apply_config: Option<PathBuf>,
        /// Print shared-HTTP configuration for this IPv4 loopback port.
        #[arg(long = "http-port")]
        http_port: Option<u16>,
        /// Environment variable NAME the host reads the bearer token from.
        #[arg(long = "token-env")]
        token_env: Option<String>,
        /// Budget policy file: {"foundry_budget": {"v":1, ...}} (config v1).
        #[arg(long)]
        budget: Option<PathBuf>,
        /// Admit an outside repository as `ROOT=STORE` on the launched owner
        /// (repeat for up to 8 references); recorded in the printed argv.
        #[arg(long = "reference", value_name = "ROOT=STORE")]
        reference: Vec<String>,
    },
    /// Foreground owned-model gateway (003 T004): a loopback-only SSE
    /// forwarder for the pinned OMP 18.6.0 / glm-5.3-flash profile, with
    /// usage receipts; never opens the source store.
    Gateway {
        /// Strict gateway config v1 JSON file (names and paths, never secrets).
        #[arg(long)]
        config: PathBuf,
    },
    /// Run OMP through the gateway in a fresh, dedicated, non-default profile.
    GatewayOmp {
        /// Gateway config v1 JSON file (same file as `foundry gateway`).
        #[arg(long)]
        config: PathBuf,
        /// File holding the single-line upstream key; never printed or logged.
        #[arg(long = "key-file")]
        key_file: PathBuf,
        /// Fresh profile name; an existing profile is refused, never overwritten.
        #[arg(long)]
        profile: String,
        /// low|high|max. Explicit so OMP never runs `auto` judge side requests.
        #[arg(long, default_value = "max", value_parser = ["low", "high", "max"])]
        thinking: String,
        /// OMP executable; defaults to `omp` on PATH.
        #[arg(long)]
        omp: Option<PathBuf>,
        /// Extra arguments passed to OMP after `--`.
        #[arg(last = true)]
        omp_args: Vec<String>,
    },
    /// Offline usage-receipt tools; open no store and no provider connection.
    Usage {
        #[command(subcommand)]
        action: UsageAction,
    },
}

#[derive(Subcommand)]
pub enum UsageAction {
    /// Summarize known totals plus missing/conflicting coverage (<=16 MiB, <=10,000 rows).
    Summarize {
        #[arg(long)]
        input: PathBuf,
    },
    /// Import one host session record (offline counters only; no store).
    Import {
        /// Host session format: `omp` or `codex`.
        #[arg(long)]
        host: crate::usage::UsageHost,
        /// The host's own session JSONL file.
        #[arg(long)]
        session: PathBuf,
    },
}

fn read_budget(path: Option<&std::path::Path>) -> FResult<crate::config::BudgetConfig> {
    let Some(path) = path else {
        return Ok(crate::config::BudgetConfig::default());
    };
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(crate::config::CONFIG_MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    crate::config::BudgetConfig::parse(&bytes)
}

/// Strict IPv4 loopback authority: `127.0.0.1:PORT`. Anything else
/// (localhost, IPv6, wildcard, LAN) is invalid; there is no public listener.
fn parse_loopback_bind(raw: &str) -> FResult<u16> {
    let invalid = || {
        FoundryError::InvalidArgument(
            "--bind must be an explicit IPv4 loopback address 127.0.0.1:PORT".into(),
        )
    };
    let (host, port) = raw.rsplit_once(':').ok_or_else(invalid)?;
    if host != "127.0.0.1" {
        return Err(invalid());
    }
    port.parse::<u16>().map_err(|_| invalid())
}

#[allow(clippy::too_many_arguments)]
fn serve_mcp(
    store: PathBuf,
    root: PathBuf,
    transport: &str,
    bind: Option<String>,
    auth_token_env: Option<String>,
    budget: Option<PathBuf>,
    references: Vec<String>,
    no_memory: bool,
) -> AResult<()> {
    let budget = read_budget(budget.as_deref())?;
    let mut parsed = Vec::with_capacity(references.len());
    for raw in &references {
        parsed.push(crate::roots::parse_reference(raw)?);
    }
    let references = parsed;
    let options = crate::mcp::ServerOptions {
        store,
        root,
        references,
        budget,
        no_memory,
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| FoundryError::Internal(e.into()))?;
    match transport {
        "stdio" => {
            if bind.is_some() || auth_token_env.is_some() {
                return Err(FoundryError::InvalidArgument(
                    "--bind and --auth-token-env apply only to --transport streamable-http".into(),
                )
                .into());
            }
            runtime.block_on(crate::mcp::serve_stdio(options))
        }
        "streamable-http" => {
            let bind = bind.ok_or_else(|| {
                FoundryError::InvalidArgument("--bind is required for streamable-http".into())
            })?;
            let token_env = auth_token_env.ok_or_else(|| {
                FoundryError::InvalidArgument(
                    "--auth-token-env is required for streamable-http".into(),
                )
            })?;
            let port = parse_loopback_bind(&bind)?;
            runtime.block_on(async move {
                let shutdown = tokio_util::sync::CancellationToken::new();
                let serve = crate::mcp::serve_http(
                    options,
                    crate::mcp::HttpOptions {
                        port,
                        token_env,
                        keep_alive: crate::mcp::SESSION_KEEP_ALIVE,
                        shutdown: shutdown.clone(),
                    },
                )
                .await?;
                // One machine-readable line so a launcher can learn a port-0 bind.
                println!(
                    "{}",
                    serde_json::json!({"listening": format!("http://{}/mcp", serve.address)})
                );
                let mut done = serve.done;
                let stop = shutdown.clone();
                tokio::spawn(async move {
                    #[cfg(unix)]
                    {
                        let mut term = tokio::signal::unix::signal(
                            tokio::signal::unix::SignalKind::terminate(),
                        )
                        .ok();
                        tokio::select! {
                            _ = tokio::signal::ctrl_c() => {},
                            _ = async {
                                match term.as_mut() {
                                    Some(term) => { term.recv().await; }
                                    None => std::future::pending::<()>().await,
                                }
                            } => {},
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        let _ = tokio::signal::ctrl_c().await;
                    }
                    stop.cancel();
                });
                let _ = done.changed().await;
                Ok(())
            })
        }
        other => Err(FoundryError::UnsupportedMode(format!(
            "unsupported transport {other:?}; expected stdio or streamable-http"
        ))
        .into()),
    }
}

/// `store` is the global `--store`; `store_explicit` is whether the operator
/// actually supplied it (bootstrap/connect otherwise default the store to
/// `<canonical-root>/.context-foundry`).
pub fn run(command: AdapterCommand, store: PathBuf, store_explicit: bool) -> AResult<()> {
    match command {
        AdapterCommand::Mcp {
            root,
            transport,
            bind,
            auth_token_env,
            budget,
            reference,
            no_memory,
        } => serve_mcp(
            store,
            root,
            &transport,
            bind,
            auth_token_env,
            budget,
            reference,
            no_memory,
        )?,
        AdapterCommand::Bootstrap {
            root,
            apply,
            components,
            graph_index,
            graph_snapshot,
            profile,
        } => {
            let store = store_explicit.then_some(store.as_path());
            let report = if apply {
                crate::bootstrap::apply(
                    &root,
                    store,
                    &components,
                    graph_index.as_deref(),
                    graph_snapshot.as_deref(),
                    profile.as_deref(),
                )?
            } else {
                crate::bootstrap::inspect(
                    &root,
                    store,
                    &components,
                    graph_index.as_deref(),
                    graph_snapshot.as_deref(),
                    profile.as_deref(),
                    &std::env::current_exe()?,
                    None,
                )?
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
            // Exit 1 on incomplete application; inspection can exit 0 with setup requirements.
            if apply && !report.complete {
                return Err(FoundryError::Internal(anyhow::anyhow!(
                    "bootstrap application incomplete; see the report above"
                ))
                .into());
            }
        }
        AdapterCommand::Connect {
            host,
            root,
            print_config,
            apply_config,
            http_port,
            token_env,
            budget,
            reference,
        } => {
            if !print_config && apply_config.is_none() {
                return Err(FoundryError::InvalidArgument(
                    "connect requires --print-config or --apply-config FILE".into(),
                )
                .into());
            }
            // The printed argv carries the references exactly as given; only
            // the ROOT=STORE shape is checked here. Admission (duplicates,
            // nesting, `ws16` collisions, count) is the launched owner's.
            for raw in &reference {
                crate::roots::parse_reference(raw)?;
            }
            let canonical_root = root.canonicalize()?;
            let store = if store_explicit {
                std::path::absolute(&store)?
            } else {
                canonical_root.join(".context-foundry")
            };
            let budget_file = budget.as_deref().map(std::path::absolute).transpose()?;
            let printed = crate::bootstrap::connect(&crate::bootstrap::ConnectInfo {
                host,
                binary: std::env::current_exe()?,
                root: canonical_root,
                store,
                budget: read_budget(budget.as_deref())?,
                budget_file,
                http_port,
                token_env,
                references: reference,
            })?;
            if let Some(file) = apply_config {
                crate::bootstrap::apply_owned_block(
                    &file,
                    &printed.owned_block,
                    printed.json_native,
                )?;
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "host": printed.host,
                    "config": printed.config_text,
                    "launch": printed.launch,
                    "instructions": printed.instructions,
                    "capability": printed.capability_note,
                    "json_native": printed.json_native,
                }))?
            );
        }
        AdapterCommand::Gateway { config } => {
            #[cfg(unix)]
            crate::gateway::run(&config)?;
            #[cfg(not(unix))]
            {
                let _ = config;
                return Err(FoundryError::PlatformUnsupported(
                    "the model gateway needs a unix host".into(),
                )
                .into());
            }
        }
        AdapterCommand::GatewayOmp {
            config,
            key_file,
            profile,
            thinking,
            omp,
            omp_args,
        } => {
            #[cfg(unix)]
            {
                // OMP's own exit status is the launcher's; cleanup already ran.
                let code = crate::gateway_launch::run(&crate::gateway_launch::Options {
                    config,
                    key_file,
                    profile,
                    thinking,
                    omp: omp.unwrap_or_else(|| PathBuf::from("omp")),
                    omp_args,
                })?;
                if code != 0 {
                    std::process::exit(code);
                }
            }
            #[cfg(not(unix))]
            {
                let _ = (config, key_file, profile, thinking, omp, omp_args);
                return Err(FoundryError::PlatformUnsupported(
                    "the model gateway needs a unix host".into(),
                )
                .into());
            }
        }
        AdapterCommand::Usage { action } => match action {
            UsageAction::Summarize { input } => println!(
                "{}",
                serde_json::to_string_pretty(&crate::receipts::summarize(&input)?)?
            ),
            UsageAction::Import { host, session } => {
                let summary = crate::usage::import_session(host, &session)?;
                println!("{}", summary.to_json());
            }
        },
    }
    Ok(())
}
