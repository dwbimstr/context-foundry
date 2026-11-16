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
    let config = if telemetry {
        super::telemetry::configuration(config, kind)
    } else {
        config.to_vec()
    };
    let restart = matches!(kind, "auth-restart" | "memory-restart");
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
            } else if telemetry {
                include_bytes!("telemetry_witness.mjs").as_slice()
            } else {
                include_bytes!("startup_witness.mjs").as_slice()
            },
        ),
    ] {
        super::fixture_private_write(root.join(path), bytes).unwrap();
    }
    let probe = if telemetry {
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
        + &if auth || memory {
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
    let captured = tokio::time::timeout(Duration::from_secs(20), async {
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
                tokio::time::sleep_until(
                    started + Duration::from_secs(if telemetry { 3 } else { 7 }),
                )
                .await;
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
    assert!(
        !forced
            && group_gone
            && exit.code() == Some(if restart { 75 } else { 0 })
            && stderr.is_empty(),
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
        if (auth || memory || telemetry)
            && let Some(path) = std::env::var_os("CF_PILOT_CURRENT_EVIDENCE_ROOT")
        {
            let path = std::path::PathBuf::from(format!(
                "{}-{}-controls",
                std::path::PathBuf::from(path).display(),
                if auth {
                    "auth"
                } else if memory {
                    "memory"
                } else {
                    "telemetry"
                }
            ));
            super::fixture_private_dirs(&path).unwrap();
            super::fixture_private_write(path.join(format!("{kind}-startup.jsonl")), &bytes)
                .unwrap();
        }
        let trace = super::current_startup_probe_exit(&bytes, if restart { 75 } else { 0 });
        assert_eq!(trace["valid"], true);
        trace
    };
    json!({"case":kind,"connections":connections,"witness":witness,
        "otlp":otlp,"config_sha256":pilot_capture::sha256_hex(&config),
        "auth_sha256":auth.then(|| pilot_capture::sha256_hex(host::QUALIFICATION_STARTUP_AUTH)),
        "memory_sha256":memory.then(|| pilot_capture::sha256_hex(host::QUALIFICATION_STARTUP_MEMORY)),
        "native_model_sha256":pilot_capture::sha256_hex(&serde_json::to_vec(&state["model"]).unwrap()),
        "probe_sha256":pilot_capture::sha256_hex(&probe),"trace":trace,
        "normal_exit":exit.success(),"exit_code":exit.code(),"group_gone":group_gone,"forced":false,"no_prompt":true,"provider_calls":0})
}
