//! Actual pinned SDK and independent TCP controls for the embedded preload.
//! This fixture does not qualify full transport coverage or capture admission.
use std::io::ErrorKind;
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::process::Stdio;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::Duration;

use context_foundry::pilot_capture::{self, host};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

struct TcpCounter {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<usize>>,
}

impl TcpCounter {
    fn new() -> Self {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut count = 0;
            loop {
                match listener.accept() {
                    Ok((stream, _)) => {
                        count += 1;
                        drop(stream);
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        if stopped.load(Ordering::SeqCst) {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("owned TCP listener failed: {error}"),
                }
            }
            count
        });
        Self {
            addr,
            stop,
            thread: Some(thread),
        }
    }

    fn finish(&mut self) -> usize {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap()
    }
}

impl Drop for TcpCounter {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) async fn control(
    omp: &Path,
    root: &Path,
    workspace: &Path,
    config: &[u8],
    models: &[u8],
    kind: &str,
) -> Value {
    use std::os::unix::process::CommandExt;
    let auth = kind.starts_with("auth-");
    let memory = kind.starts_with("memory-");
    let telemetry = kind.starts_with("telemetry-");
    let warmup = kind.starts_with("warmup-");
    let logging = kind.starts_with("logging-");
    let marketplace = kind.starts_with("marketplace-");
    let watchdog = kind.starts_with("watchdog-");
    let resolver = kind.starts_with("resolver-");
    let config = if telemetry {
        super::telemetry::configuration(config, kind)
    } else if marketplace && kind != "marketplace-owned" {
        let mut config = std::str::from_utf8(config).unwrap().to_owned();
        let off = "\nmarketplace:\n  autoUpdate: off\n";
        let notify = "\nmarketplace:\n  autoUpdate: notify\n";
        let selected = if kind == "marketplace-disabled" {
            off
        } else {
            notify
        };
        if config.contains(off) {
            config = config.replace(off, selected);
        } else {
            assert!(!config.contains("marketplace:"));
            config.push_str(selected);
        }
        config.into_bytes()
    } else {
        config.to_vec()
    };
    let restart = matches!(
        kind,
        "auth-restart" | "memory-restart" | "warmup-restart" | "warmup-repeat-seal"
    );
    for dir in [
        root.to_path_buf(),
        root.join("home"),
        root.join("agent"),
        root.join("sessions"),
        root.join("tmp"),
        root.join("bridge"),
    ] {
        super::fixture_private_dirs(dir).unwrap();
    }
    for (path, bytes) in [
        ("agent/config.yml", config.as_slice()),
        ("agent/models.yml", models),
        (
            "bridge/witness.mjs",
            if auth {
                include_bytes!("auth_witness.mjs").as_slice()
            } else if memory {
                include_bytes!("memory_witness.mjs").as_slice()
            } else if logging {
                include_bytes!("logging_witness.mjs").as_slice()
            } else if marketplace {
                include_bytes!("marketplace_witness.mjs").as_slice()
            } else if watchdog {
                include_bytes!("watchdog_witness.mjs").as_slice()
            } else if resolver {
                include_bytes!("resolver_witness.mjs").as_slice()
            } else if warmup {
                include_bytes!("warmup_witness.mjs").as_slice()
            } else if telemetry {
                include_bytes!("telemetry_witness.mjs").as_slice()
            } else {
                include_bytes!("startup_witness.mjs").as_slice()
            },
        ),
    ] {
        super::fixture_private_write(root.join(path), bytes).unwrap();
    }
    let probe = if resolver {
        let mut source = host::QUALIFICATION_STARTUP_PROBE.to_vec();
        source.extend_from_slice(include_bytes!("resolver_probe.ts"));
        source
    } else if watchdog {
        let mut source = host::QUALIFICATION_STARTUP_PROBE.to_vec();
        source.extend_from_slice(include_bytes!("watchdog_probe.ts"));
        source
    } else if logging {
        let mut source = host::QUALIFICATION_STARTUP_PROBE.to_vec();
        source.extend_from_slice(include_bytes!("logging_probe.ts"));
        source
    } else if warmup {
        let mut source = std::str::from_utf8(host::QUALIFICATION_STARTUP_PROBE)
            .unwrap()
            .to_owned();
        if kind == "warmup-missing" {
            let marker = "args[0] !== 10000";
            assert_eq!(source.matches(marker).count(), 1);
            source = source.replace(marker, "args[0] !== 10001");
        }
        if kind == "warmup-unsettled" {
            source = format!(
                "const actualTimeout = AbortSignal.timeout; AbortSignal.timeout = function(ms) {{ return ms === 10000 ? new AbortController().signal : Reflect.apply(actualTimeout, this, [ms]); }};\n{source}"
            );
        }
        source.into_bytes()
    } else if telemetry {
        super::telemetry::preload()
    } else if kind == "stock" {
        b"process.chdir(process.env.CF_PILOT_PROBE_WORKSPACE); globalThis.fetch = async () => { throw new Error('CF_QUALIFICATION_STARTUP_REFUSED'); };\n".to_vec()
    } else {
        host::QUALIFICATION_STARTUP_PROBE.to_vec()
    };
    super::fixture_private_write(root.join("bridge/startup_probe.ts"), &probe).unwrap();
    if auth {
        super::fixture_private_write(
            root.join("bridge/startup_auth.ts"),
            host::QUALIFICATION_STARTUP_AUTH,
        )
        .unwrap();
    }
    if memory {
        super::fixture_private_write(
            root.join("bridge/startup_memory.ts"),
            host::QUALIFICATION_STARTUP_MEMORY,
        )
        .unwrap();
    }
    if warmup || logging {
        super::fixture_private_write(
            root.join("bridge/startup_logging.ts"),
            host::QUALIFICATION_STARTUP_LOGGING,
        )
        .unwrap();
    }
    let mut counter = (!telemetry).then(TcpCounter::new);
    let mut collector = telemetry.then(super::telemetry::Collector::new);
    let addr = collector
        .as_ref()
        .map(|c| c.addr)
        .unwrap_or_else(|| counter.as_ref().unwrap().addr);
    let policy = host::sandbox_policy(
        omp,
        root,
        workspace,
        &std::path::PathBuf::from(std::env::var_os("HOME").unwrap()),
        &[],
        &[],
    )
    .unwrap()
        + &if auth || memory || warmup || logging || marketplace || watchdog || resolver {
            String::new()
        } else {
            format!(
                "(allow network-outbound (remote ip \"localhost:{}\"))",
                addr.port()
            )
        };
    let mut command = tokio::process::Command::new(host::SANDBOX_EXEC);
    command
        .arg("-p")
        .arg(&policy)
        .arg(omp)
        .args([
            "--mode",
            "rpc",
            "--model",
            "anthropic/claude-sonnet-5-5",
            "--thinking",
            "off",
            "--tools",
            "read,edit",
            "--no-extensions",
            "--no-skills",
            "--no-rules",
            "--no-context-files",
            "--no-title",
            "--no-lsp",
            "--no-pty",
            "--no-prewalk",
            "--session-dir",
        ])
        .arg(root.join("sessions"))
        .arg("--trusted-extension")
        .arg(root.join("bridge/witness.mjs"))
        .current_dir(root)
        .env_clear()
        .env("HOME", root.join("home"))
        .env("TMPDIR", root.join("tmp"))
        .env("XDG_CONFIG_HOME", root.join("home"))
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C")
        .env("NO_COLOR", "1")
        .env("DO_NOT_TRACK", "1")
        .env("BUN_RUNTIME_TRANSPILER_CACHE_PATH", "0")
        .env("PI_CONFIG_DIR", ".omp")
        .env("PI_CODING_AGENT_DIR", root.join("agent"))
        .env("PI_NO_TITLE", "1")
        .env("BUN_OPTIONS", "--preload ./bridge/startup_probe.ts")
        .env("CF_PILOT_PROBE_WORKSPACE", workspace)
        .env("CF_PILOT_PROBE_TRACE", root.join("startup.jsonl"))
        .env("CF_STARTUP_WITNESS_KIND", kind)
        .env(
            "CF_STARTUP_WITNESS_URL",
            format!("https://{addr}/owned-startup-witness"),
        )
        .env("CF_STARTUP_WITNESS_RESULT", root.join("witness.json"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true);
    if telemetry {
        command
            .env("CF_OWNED_OTLP_AUTHORITY", addr.to_string())
            .env("OTEL_EXPORTER_OTLP_ENDPOINT", format!("http://{addr}"))
            .env("OTEL_TRACES_EXPORTER", "otlp")
            .env("OTEL_METRICS_EXPORTER", "otlp")
            .env("OTEL_LOGS_EXPORTER", "otlp")
            .env("OTEL_EXPORTER_OTLP_PROTOCOL", "http/protobuf")
            .env("OTEL_BSP_SCHEDULE_DELAY", "500")
            .env("OTEL_BLRP_SCHEDULE_DELAY", "500");
    }
    // SAFETY: umask is async-signal-safe and runs only in the child before exec.
    unsafe {
        command.as_std_mut().pre_exec(|| {
            libc::umask(0o077);
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    let pid = child.id().unwrap() as i32;
    let stderr = child.stderr.take().unwrap();
    let errors = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stderr.take(65537).read_to_end(&mut bytes).await.unwrap();
        bytes
    });
    let mut lines = BufReader::new(child.stdout.take().unwrap().take(1024 * 1024 + 1)).lines();
    let started = tokio::time::Instant::now();
    let mut watchdog_initial = None;
    let mut resolver_initial = None;
    let state_timeout = Duration::from_secs(if watchdog || resolver { 45 } else { 20 });
    let captured = tokio::time::timeout(state_timeout, async {
        let mut ready = false;
        let mut total = 0;
        loop {
            let line = lines
                .next_line()
                .await
                .map_err(|_| "RPC read")?
                .ok_or("RPC EOF")?;
            total += line.len() + 1;
            if total > 1024 * 1024 {
                return Err("RPC cap");
            }
            let value: Value = serde_json::from_str(&line).map_err(|_| "RPC JSON")?;
            if value["type"] == "ready" {
                if ready {
                    return Err("duplicate Ready");
                }
                ready = true;
                child
                    .stdin
                    .as_mut()
                    .unwrap()
                    .write_all(b"{\"id\":\"startup-state\",\"type\":\"get_state\"}\n")
                    .await
                    .map_err(|_| "RPC write")?;
            }
            if value["type"] == "response" && value["id"] == "startup-state" {
                if !ready || value["success"] != true {
                    return Err("state refused");
                }
                if watchdog {
                    watchdog_initial = Some(value["data"].clone());
                    tokio::time::sleep(Duration::from_secs(22)).await;
                    child
                        .stdin
                        .as_mut()
                        .unwrap()
                        .write_all(b"{\"id\":\"watchdog-after\",\"type\":\"get_state\"}\n")
                        .await
                        .map_err(|_| "RPC write")?;
                    continue;
                }
                if resolver {
                    resolver_initial = Some(value["data"].clone());
                    tokio::time::sleep(Duration::from_secs(12)).await;
                    child
                        .stdin
                        .as_mut()
                        .unwrap()
                        .write_all(b"{\"id\":\"resolver-after\",\"type\":\"get_state\"}\n")
                        .await
                        .map_err(|_| "RPC write")?;
                    continue;
                }
                if warmup {
                    while !root.join("witness.json").is_file() {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                }
                tokio::time::sleep_until(
                    started
                        + Duration::from_secs(if warmup {
                            14
                        } else if telemetry {
                            3
                        } else {
                            7
                        }),
                )
                .await;
                return Ok(value["data"].clone());
            }
            if watchdog && value["type"] == "response" && value["id"] == "watchdog-after" {
                if value["success"] != true
                    || watchdog_initial.as_ref().map(|state| &state["model"])
                        != Some(&value["data"]["model"])
                {
                    return Err("watchdog state changed");
                }
                return Ok(value["data"].clone());
            }
            if resolver && value["type"] == "response" && value["id"] == "resolver-after" {
                if value["success"] != true
                    || resolver_initial.as_ref().map(|state| &state["model"])
                        != Some(&value["data"]["model"])
                {
                    return Err("resolver state changed");
                }
                return Ok(value["data"].clone());
            }
        }
    })
    .await;
    drop(child.stdin.take());
    let mut forced = false;
    let exit = match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
        Ok(result) => result.unwrap(),
        Err(_) => {
            forced = true;
            // SAFETY: the child was launched as its own process group.
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
            child.wait().await.unwrap()
        }
    };
    let otlp = collector
        .as_mut()
        .map(|c| c.finish())
        .unwrap_or(Value::Null);
    let connections = counter
        .as_mut()
        .map(TcpCounter::finish)
        .unwrap_or_else(|| otlp["connections"].as_u64().unwrap() as usize);
    // SAFETY: signal zero only checks the owned process group after wait/reap.
    let group_gone = unsafe { libc::kill(-pid, 0) } == -1
        && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
    let stderr = errors.await.unwrap();
    super::fixture_private_write(root.join("stderr.log"), &stderr).unwrap();
    let watchdog_notices = if watchdog {
        assert!(stderr.len() <= 65536);
        let text = std::str::from_utf8(&stderr).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len() % 2, 0);
        let log_prefix = format!("  logs: {}/omp.", root.join("home/.omp/logs").display());
        for pair in lines.chunks_exact(2) {
            let (seconds, phase) = pair[0]
                .strip_prefix("Still starting after ")
                .unwrap()
                .split_once("s — phase: ")
                .unwrap();
            assert!(
                (10..=44).contains(&seconds.parse::<u64>().unwrap())
                    && !phase.is_empty()
                    && phase.len() <= 512
            );
            assert!(
                pair[1].starts_with(&log_prefix)
                    && pair[1].ends_with(
                        ".log · re-run with PI_DEBUG_STARTUP=1 for streaming phase markers"
                    )
            );
        }
        lines.len() / 2
    } else {
        0
    };
    assert!(
        !forced
            && group_gone
            && exit.code() == Some(if restart { 75 } else { 0 })
            && (watchdog || stderr.is_empty()),
        "native startup exit: {exit:?}"
    );
    let state = captured
        .expect("native state deadline")
        .expect("native state capture");
    assert_eq!(state["model"]["provider"], "anthropic");
    assert_eq!(state["model"]["id"], "claude-sonnet-5-5");
    assert_eq!(state["messageCount"], 0);
    assert_eq!(state["isStreaming"], false);
    let witness: Value =
        serde_json::from_slice(&std::fs::read(root.join("witness.json")).unwrap()).unwrap();
    let trace = if kind == "stock" {
        Value::Null
    } else {
        let bytes = std::fs::read(root.join("startup.jsonl")).unwrap();
        if (auth || memory || telemetry || warmup || logging || marketplace || watchdog || resolver)
            && let Some(path) = std::env::var_os("CF_PILOT_CURRENT_EVIDENCE_ROOT")
        {
            let path = std::path::PathBuf::from(format!(
                "{}-{}-controls",
                std::path::PathBuf::from(path).display(),
                if auth {
                    "auth"
                } else if memory {
                    "memory"
                } else if warmup {
                    "warmup"
                } else if logging {
                    "logging"
                } else if marketplace {
                    "marketplace"
                } else if watchdog {
                    "watchdog"
                } else if resolver {
                    "resolver"
                } else {
                    "telemetry"
                }
            ));
            super::fixture_private_dirs(&path).unwrap();
            if watchdog {
                super::fixture_private_write(path.join(format!("{kind}-stderr.log")), &stderr)
                    .unwrap();
            }
            super::fixture_private_write(path.join(format!("{kind}-startup.jsonl")), &bytes)
                .unwrap();
        }
        let trace = super::current_startup_probe_exit(&bytes, if restart { 75 } else { 0 });
        assert_eq!(trace["valid"], true);
        trace
    };
    let logging_io = if logging {
        let files: Vec<_> = std::fs::read_dir(root.join("owned-logs"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(files.len(), 1);
        let path = &files[0];
        let metadata = std::fs::symlink_metadata(path).unwrap();
        assert!(metadata.is_file() && metadata.len() <= 1024 * 1024);
        let bytes = std::fs::read(path).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        json!({"files":1,"bytes":bytes.len(),"sha256":pilot_capture::sha256_hex(&bytes),
            "before_marker":text.contains("CF_OWNED_LOG_BEFORE_RETIRE"),"after_marker":text.contains("CF_OWNED_LOG_AFTER_RETIRE")})
    } else {
        Value::Null
    };
    // Each serial case owns this recoverable extraction. Verify it before
    // retiring the copy once its only child is reaped, rather than retaining
    // several GiB of identical libraries until the outer TempDir is dropped.
    let native_module_cleanup = {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let path = root.join("home/.omp/natives/18.18.1/pi_natives.darwin-arm64.node");
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) => {
                assert!(metadata.is_file() && std::fs::canonicalize(&path).unwrap() == path);
                assert_eq!(metadata.uid(), unsafe { libc::getuid() });
                assert_eq!(metadata.permissions().mode() & 0o7777, 0o700);
                assert_eq!(metadata.len(), 111110320);
                let digest = pilot_capture::sha256_hex(&std::fs::read(&path).unwrap());
                assert_eq!(
                    digest,
                    "ee0bf26f4e88b0905611f761efcd6e73a0f2e55a9355e293b0251542dc8b900b"
                );
                std::fs::remove_file(&path).unwrap();
                json!({"removed_after_reap":true,"bytes":metadata.len(),"mode":0o700,"sha256":digest})
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Value::Null,
            Err(error) => panic!("native extraction metadata: {error}"),
        }
    };
    json!({"case":kind,"connections":connections,"witness":witness,
        "otlp":otlp,"logging_io":logging_io,"watchdog_notices":watchdog_notices,"watchdog_second_state":watchdog_initial.is_some(),
        "watchdog_stderr_bytes":watchdog.then_some(stderr.len()),"watchdog_stderr_sha256":watchdog.then(|| pilot_capture::sha256_hex(&stderr)),
        "watchdog_witness_sha256":watchdog.then(|| pilot_capture::sha256_hex(&std::fs::read(root.join("bridge/witness.mjs")).unwrap())),
        "resolver_second_state":resolver_initial.is_some(),
        "resolver_witness_sha256":resolver.then(|| pilot_capture::sha256_hex(&std::fs::read(root.join("bridge/witness.mjs")).unwrap())),
        "native_module_cleanup":native_module_cleanup,
        "config_sha256":pilot_capture::sha256_hex(&config),
        "auth_sha256":auth.then(|| pilot_capture::sha256_hex(host::QUALIFICATION_STARTUP_AUTH)),
        "memory_sha256":memory.then(|| pilot_capture::sha256_hex(host::QUALIFICATION_STARTUP_MEMORY)),
        "logging_sha256":(warmup || logging).then(|| pilot_capture::sha256_hex(host::QUALIFICATION_STARTUP_LOGGING)),
        "native_model_sha256":pilot_capture::sha256_hex(&serde_json::to_vec(&state["model"]).unwrap()),
        "probe_sha256":pilot_capture::sha256_hex(&probe),"trace":trace,
        "normal_exit":exit.success(),"exit_code":exit.code(),"group_gone":group_gone,"forced":false,"no_prompt":true,"provider_calls":0})
}
