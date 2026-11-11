//! Native OMP host lifecycle for the capture session (implementer contract
//! §5, §8): a fresh private host root per launch, the pinned binary under a
//! network-denied `sandbox-exec` profile, the bridge loaded as the only
//! trusted extension, private inherited pipes on fds 3 and 4, RPC on
//! stdin/stdout, tool admission, normal shutdown and native session evidence.
//!
//! The host never receives a real credential, never has network access and
//! cannot read the evidence directory, other arms' state or the credential
//! file. Forced termination is a cleanup failure, never a manufactured
//! success.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::mpsc;

use super::protocol::{
    ChannelIdentity, FaultClass, Inbound, InboundState, OutboundState, ParentMessage, ProtocolFault,
};
use super::{
    CaptureFault, PrivateArtifact, PrivateTreeBudget, create_private_dir, open_private_file,
    policy, private_child, require_private_directory, sha256_hex, verify_private_tree,
    write_private_new,
};

/// The bridge source, embedded from the repository and materialized with
/// its exact bytes into each private host root.
pub const BRIDGE_SOURCE: &[u8] = include_bytes!("bridge.ts");
pub const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
/// Exact owned extraction path of the pinned OMP 18.13.2 macOS arm64 host.
/// This declares private executable metadata, not a per-request content hash.
pub(crate) const NATIVE_MODULE_RELATIVE_PATH: &str =
    "home/.omp/natives/18.13.2/pi_natives.darwin-arm64.node";

/// Match a bounded walk's inventoried path at hosts/<one-root>/<exact module>.
/// No workspace, store, other host version, or arbitrary-depth path is admitted.
pub(crate) fn is_run_native_module(relative: &Path) -> bool {
    let mut parts = relative.components();
    matches!(parts.next(), Some(Component::Normal(name)) if name == "hosts")
        && matches!(parts.next(), Some(Component::Normal(_)))
        && parts.as_path().as_os_str() == std::ffi::OsStr::new(NATIVE_MODULE_RELATIVE_PATH)
}

/// Longest RPC stdout line kept for parsing (agent_end carries messages).
const MAX_RPC_LINE_BYTES: usize = 32 * 1024 * 1024;
/// Largest native session file read back for identity reconciliation.
const MAX_NATIVE_SESSION_BYTES: u64 = 64 * 1024 * 1024;
const EVENT_QUEUE: usize = 16;
const WRITE_QUEUE: usize = 8;

const MODELS_YML: &str = "providers:\n  anthropic:\n    auth: none\n";
/// Per-host settings: no agent retry, model fallback, compaction, cache
/// warming, loop guard, unexpected-stop continuation or read summaries;
/// replace edits without fuzzy matching; short cache retention; no host
/// stream timers (Rust's generation deadline bounds every attempt, and a
/// host timer abort before response start would trigger a host retry).
const CONFIG_YML: &str = "retry:\n  enabled: false\n  maxRetries: 0\n  modelFallback: false\n\
compaction:\n  enabled: false\n  idleEnabled: false\n\
providers:\n  cacheRetention: short\n  cacheWarming: \"off\"\n  streamFirstEventTimeoutSeconds: 0\n  streamIdleTimeoutSeconds: 0\n\
model:\n  loopGuard:\n    enabled: false\n\
features:\n  unexpectedStopDetection: none\n\
read:\n  summarize:\n    enabled: false\n\
edit:\n  mode: replace\n  fuzzyMatch: false\n";

fn fault(class: FaultClass, code: &'static str) -> CaptureFault {
    CaptureFault::new(class, code)
}

/// Everything needed to launch one fresh native host.
#[derive(Clone, Debug)]
pub struct HostSpec {
    /// Pinned OMP executable (its hash is verified by the session).
    pub omp: PathBuf,
    /// Fresh host root to create (must not exist; parent must exist).
    pub root: PathBuf,
    /// Canonical owned workspace (must exist).
    pub workspace: PathBuf,
    /// The owner's real home: denied for reading and writing.
    pub real_home: PathBuf,
    /// Additional subpaths the host must not read or write (the run
    /// directory with its evidence and other arms; checker directories).
    pub deny: Vec<PathBuf>,
    /// Literal files the host must not read (the credential file).
    pub deny_literals: Vec<PathBuf>,
    pub identity: ChannelIdentity,
}

/// Paths of a launched host root.
#[derive(Clone, Debug)]
pub struct HostPaths {
    pub root: PathBuf,
    pub home: PathBuf,
    pub agent: PathBuf,
    pub sessions: PathBuf,
    pub bridge: PathBuf,
    pub tmp: PathBuf,
}

/// Non-secret launch evidence.
#[derive(Clone, Debug)]
pub struct LaunchRecord {
    pub pid: u32,
    pub argv_sha256: String,
    pub policy_sha256: String,
    pub bridge_sha256: String,
    pub models_yml_sha256: String,
    pub config_yml_sha256: String,
    pub env_names: Vec<String>,
}

/// Summary of a native session `get_state` response.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostState {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api: Option<String>,
    pub max_tokens: Option<u64>,
    pub thinking_level: Option<String>,
    pub auto_compaction: Option<bool>,
    pub session_file: Option<String>,
}

/// One RPC stdout frame, summarized. Message text, thinking and tool
/// content are never retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RpcEvent {
    Ready,
    Response {
        id: Option<String>,
        command: String,
        success: bool,
        state: Option<HostState>,
    },
    PromptResult {
        id: Option<String>,
        status: String,
    },
    SessionSettled,
    AgentEnd,
    /// A host behaviour the pilot forbids (retry, compaction, cache warming,
    /// fallback, model change, extension error).
    Forbidden(&'static str),
    Other,
}

/// Everything the session driver receives from a running host.
#[derive(Debug)]
pub enum HostEvent {
    Bridge(Inbound),
    BridgeFault(ProtocolFault),
    BridgeEof,
    Rpc(RpcEvent),
    RpcFault(&'static str),
    RpcEof,
    BridgeWriteFailed(&'static str),
}

/// How a host ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostExit {
    /// Exited on its own after RPC stdin EOF.
    pub normal: bool,
    pub code: Option<i32>,
    pub signal: Option<i32>,
    /// The process group had to be killed.
    pub forced: bool,
    /// No member of the process group remained afterwards.
    pub group_gone: bool,
    pub stderr_bytes: u64,
    pub stderr_sha256: String,
}

pub struct Host {
    pub paths: HostPaths,
    pub launch: LaunchRecord,
    child: Child,
    pgid: i32,
    bridge_out: Option<mpsc::Sender<(ParentMessage, Vec<u8>)>>,
    rpc_out: Option<mpsc::Sender<Vec<u8>>>,
    threads: Vec<JoinHandle<()>>,
    stderr: std::sync::Arc<std::sync::Mutex<StderrDigest>>,
}

#[derive(Default)]
struct StderrDigest {
    bytes: u64,
    hasher: Option<sha2::Sha256>,
    done: bool,
}

/// Reject paths the SBPL profile cannot quote safely.
fn sbpl_path(path: &Path) -> Result<String, CaptureFault> {
    let text = path
        .to_str()
        .ok_or(fault(FaultClass::Bootstrap, "sandbox_path_not_utf8"))?;
    if !path.is_absolute()
        || text.is_empty()
        || text
            .chars()
            .any(|c| c == '"' || c == '\\' || c.is_control())
    {
        return Err(fault(FaultClass::Bootstrap, "sandbox_path_unquotable"));
    }
    Ok(text.to_string())
}

/// Compose the host's sandbox profile. Later rules win in SBPL: denials of
/// the real home and the run directory come first, the host's own root,
/// workspace and executable are re-allowed, then credential literals are
/// denied again last.
pub fn sandbox_policy(
    omp: &Path,
    root: &Path,
    workspace: &Path,
    real_home: &Path,
    deny: &[PathBuf],
    deny_literals: &[PathBuf],
) -> Result<String, CaptureFault> {
    let mut p = String::from("(version 1)(allow default)(deny network*)");
    let home = sbpl_path(real_home)?;
    p.push_str(&format!(
        "(deny file-read-data (subpath \"{home}\"))(deny file-write* (subpath \"{home}\"))"
    ));
    for d in deny {
        let d = sbpl_path(d)?;
        p.push_str(&format!(
            "(deny file-read-data (subpath \"{d}\"))(deny file-write* (subpath \"{d}\"))"
        ));
    }
    let (omp, root, ws) = (sbpl_path(omp)?, sbpl_path(root)?, sbpl_path(workspace)?);
    p.push_str(&format!(
        "(allow file-read-data (literal \"{omp}\") (subpath \"{root}\") (subpath \"{ws}\"))\
         (allow file-write* (subpath \"{root}\") (subpath \"{ws}\"))"
    ));
    for l in deny_literals {
        let l = sbpl_path(l)?;
        p.push_str(&format!(
            "(deny file-read-data (literal \"{l}\"))(deny file-write* (literal \"{l}\"))"
        ));
    }
    p.push_str(
        "(deny mach-lookup (global-name \"com.apple.securityd\") (global-name \"com.apple.securityd.xpc\"))",
    );
    Ok(p)
}

/// The OMP argv (after the sandbox wrapper).
pub fn omp_args(paths: &HostPaths) -> Vec<String> {
    let model = format!("{}/{}", policy::PROVIDER, policy::MODEL);
    vec![
        "--mode".into(),
        "rpc".into(),
        "--model".into(),
        model,
        "--thinking".into(),
        policy::THINKING_LEVEL.into(),
        "--tools".into(),
        "read,edit".into(),
        "--no-extensions".into(),
        "--no-skills".into(),
        "--no-rules".into(),
        "--no-context-files".into(),
        "--no-title".into(),
        "--no-lsp".into(),
        "--no-pty".into(),
        "--no-prewalk".into(),
        "--session-dir".into(),
        paths.sessions.to_string_lossy().into_owned(),
        "--trusted-extension".into(),
        paths.bridge.to_string_lossy().into_owned(),
    ]
}

fn host_env(
    paths: &HostPaths,
    identity: &ChannelIdentity,
    tx_ino: u64,
    rx_ino: u64,
) -> Vec<(String, String)> {
    let s = |p: &Path| p.to_string_lossy().into_owned();
    vec![
        ("HOME".into(), s(&paths.home)),
        ("TMPDIR".into(), format!("{}/", s(&paths.tmp))),
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("LANG".into(), "C".into()),
        ("XDG_CONFIG_HOME".into(), s(&paths.home.join(".config"))),
        ("XDG_DATA_HOME".into(), s(&paths.home.join(".local/share"))),
        ("PI_CONFIG_DIR".into(), ".omp".into()),
        ("PI_CODING_AGENT_DIR".into(), s(&paths.agent)),
        ("PI_EDIT_VARIANT".into(), "replace".into()),
        ("PI_EDIT_FUZZY".into(), "0".into()),
        ("PI_REQUEST_COMPRESSION".into(), "0".into()),
        ("PI_NO_TITLE".into(), "1".into()),
        ("PI_CACHE_RETENTION".into(), "short".into()),
        ("PI_NO_THINKING_LOOP_GUARD".into(), "1".into()),
        ("CF_PILOT_SESSION".into(), identity.session.clone()),
        ("CF_PILOT_HOST".into(), identity.host.clone()),
        ("CF_PILOT_TX_FD".into(), "3".into()),
        ("CF_PILOT_RX_FD".into(), "4".into()),
        ("CF_PILOT_TX_INO".into(), tx_ino.to_string()),
        ("CF_PILOT_RX_INO".into(), rx_ino.to_string()),
        ("CF_PILOT_API".into(), policy::BRIDGE_API.into()),
    ]
}

impl Host {
    /// Create the fresh private root, materialize configuration and the
    /// bridge, and launch the pinned host with fds 3 (child → Rust) and 4
    /// (Rust → child).
    /// Returns the host and the receiver of its events (kept separate so
    /// the driver can await events while sending frames).
    pub fn launch(spec: &HostSpec) -> Result<(Host, mpsc::Receiver<HostEvent>), CaptureFault> {
        let workspace = fs::canonicalize(&spec.workspace)
            .map_err(|_| fault(FaultClass::Bootstrap, "workspace_missing"))?;
        if workspace != spec.workspace {
            return Err(fault(FaultClass::Bootstrap, "workspace_not_canonical"));
        }
        verify_private_tree(&workspace, &[], &mut PrivateTreeBudget::new(None))?;
        if workspace.join(".omp").symlink_metadata().is_ok() {
            return Err(fault(
                FaultClass::Bootstrap,
                "workspace_has_project_settings",
            ));
        }
        create_private_dir(&spec.root)?;
        let root = fs::canonicalize(&spec.root)
            .map_err(|_| fault(FaultClass::Bootstrap, "host_root_unresolvable"))?;
        let paths = HostPaths {
            home: root.join("home"),
            agent: root.join("agent"),
            sessions: root.join("sessions"),
            bridge: root.join("bridge").join("bridge.ts"),
            tmp: root.join("tmp"),
            root: root.clone(),
        };
        for dir in [
            &paths.home,
            &paths.agent,
            &paths.sessions,
            &paths.tmp,
            &root.join("bridge"),
        ] {
            create_private_dir(dir)?;
        }
        write_private_new(&paths.agent.join("models.yml"), MODELS_YML.as_bytes())?;
        write_private_new(&paths.agent.join("config.yml"), CONFIG_YML.as_bytes())?;
        write_private_new(&paths.bridge, BRIDGE_SOURCE)?;
        let written = fs::read(&paths.bridge)
            .map_err(|_| fault(FaultClass::Bootstrap, "bridge_unreadable"))?;
        let bridge_sha256 = sha256_hex(&written);
        if bridge_sha256 != sha256_hex(BRIDGE_SOURCE) {
            return Err(fault(FaultClass::Bootstrap, "bridge_bytes_mismatch"));
        }

        let policy_text = sandbox_policy(
            &spec.omp,
            &root,
            &workspace,
            &spec.real_home,
            &spec.deny,
            &spec.deny_literals,
        )?;
        let args = omp_args(&paths);

        let (c2r_read, c2r_write) =
            std::io::pipe().map_err(|_| fault(FaultClass::Bootstrap, "pipe_create_failed"))?;
        let (r2c_read, r2c_write) =
            std::io::pipe().map_err(|_| fault(FaultClass::Bootstrap, "pipe_create_failed"))?;
        let c2r_write: OwnedFd = c2r_write.into();
        let r2c_read: OwnedFd = r2c_read.into();
        let tx_ino = fstat_ino(c2r_write.as_raw_fd())?;
        let rx_ino = fstat_ino(r2c_read.as_raw_fd())?;
        let env = host_env(&paths, &spec.identity, tx_ino, rx_ino);

        let mut cmd = Command::new(SANDBOX_EXEC);
        cmd.arg("-p")
            .arg(&policy_text)
            .arg(&spec.omp)
            .args(&args)
            .current_dir(&workspace)
            .env_clear()
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        verify_private_tree(
            &root,
            &[Path::new(NATIVE_MODULE_RELATIVE_PATH)],
            &mut PrivateTreeBudget::new(None),
        )?;
        private_child(&mut cmd);
        let child_write = c2r_write.as_raw_fd();
        let child_read = r2c_read.as_raw_fd();
        // SAFETY: only async-signal-safe libc calls (fcntl, dup2, close)
        // run between fork and exec; nothing allocates.
        unsafe {
            cmd.pre_exec(move || place_channel_fds(child_write, child_read));
        }
        let mut child = cmd
            .spawn()
            .map_err(|_| fault(FaultClass::Bootstrap, "host_spawn_failed"))?;
        drop(c2r_write);
        drop(r2c_read);
        let pid = child.id();
        let pgid =
            i32::try_from(pid).map_err(|_| fault(FaultClass::Bootstrap, "host_pid_invalid"))?;

        let launch = LaunchRecord {
            pid,
            argv_sha256: sha256_hex(args.join("\0").as_bytes()),
            policy_sha256: sha256_hex(policy_text.as_bytes()),
            bridge_sha256,
            models_yml_sha256: sha256_hex(MODELS_YML.as_bytes()),
            config_yml_sha256: sha256_hex(CONFIG_YML.as_bytes()),
            env_names: env.iter().map(|(k, _)| k.clone()).collect(),
        };

        let (event_tx, events) = mpsc::channel(EVENT_QUEUE);
        let mut threads = Vec::new();
        let stdout = child
            .stdout
            .take()
            .ok_or(fault(FaultClass::Bootstrap, "host_stdout_missing"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or(fault(FaultClass::Bootstrap, "host_stdin_missing"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(fault(FaultClass::Bootstrap, "host_stderr_missing"))?;

        // Bridge reader: fd 3 frames → events.
        {
            let tx = event_tx.clone();
            let identity = spec.identity.clone();
            let mut reader = c2r_read;
            threads.push(std::thread::spawn(move || {
                let mut state = InboundState::new(identity);
                loop {
                    let event = match state.read(&mut reader) {
                        Ok(Some(inbound)) => HostEvent::Bridge(inbound),
                        Ok(None) => HostEvent::BridgeEof,
                        Err(f) => HostEvent::BridgeFault(f),
                    };
                    let stop = !matches!(event, HostEvent::Bridge(_));
                    if tx.blocking_send(event).is_err() || stop {
                        break;
                    }
                }
            }));
        }
        // Bridge writer: bounded queue → fd 4 frames.
        let (bridge_tx, mut bridge_rx) = mpsc::channel::<(ParentMessage, Vec<u8>)>(WRITE_QUEUE);
        {
            let tx = event_tx.clone();
            let identity = spec.identity.clone();
            let mut writer = r2c_write;
            threads.push(std::thread::spawn(move || {
                let mut state = OutboundState::new(identity);
                while let Some((msg, payload)) = bridge_rx.blocking_recv() {
                    if let Err(ProtocolFault(code)) = state.write(&mut writer, msg, &payload) {
                        let _ = tx.blocking_send(HostEvent::BridgeWriteFailed(code));
                        break;
                    }
                }
                drop(writer);
            }));
        }
        // RPC stdout reader.
        {
            let tx = event_tx.clone();
            threads.push(std::thread::spawn(move || {
                let mut reader = BufReader::with_capacity(64 * 1024, stdout);
                loop {
                    match read_bounded_line(&mut reader, MAX_RPC_LINE_BYTES) {
                        Ok(Some(line)) => {
                            let event = match parse_rpc_line(&line) {
                                // Streaming updates and other frames are
                                // summarized away here, never queued.
                                Ok(RpcEvent::Other) => continue,
                                Ok(e) => HostEvent::Rpc(e),
                                Err(code) => HostEvent::RpcFault(code),
                            };
                            let stop = matches!(event, HostEvent::RpcFault(_));
                            if tx.blocking_send(event).is_err() || stop {
                                // Keep draining so the host never blocks on a full pipe.
                                drain(&mut reader);
                                break;
                            }
                        }
                        Ok(None) => {
                            let _ = tx.blocking_send(HostEvent::RpcEof);
                            break;
                        }
                        Err(code) => {
                            let _ = tx.blocking_send(HostEvent::RpcFault(code));
                            drain(&mut reader);
                            break;
                        }
                    }
                }
            }));
        }
        // RPC stdin writer: dropping the sender closes stdin (EOF).
        let (rpc_tx, mut rpc_rx) = mpsc::channel::<Vec<u8>>(WRITE_QUEUE);
        {
            let tx = event_tx.clone();
            let mut stdin = stdin;
            threads.push(std::thread::spawn(move || {
                while let Some(line) = rpc_rx.blocking_recv() {
                    if stdin.write_all(&line).and_then(|()| stdin.flush()).is_err() {
                        let _ = tx.blocking_send(HostEvent::RpcFault("rpc_stdin_write_failed"));
                        break;
                    }
                }
                drop(stdin);
            }));
        }
        // stderr: count and hash only; content is never retained.
        let digest = std::sync::Arc::new(std::sync::Mutex::new(StderrDigest {
            hasher: Some(<sha2::Sha256 as sha2::Digest>::new()),
            ..StderrDigest::default()
        }));
        {
            let digest = digest.clone();
            let mut stderr = stderr;
            threads.push(std::thread::spawn(move || {
                let mut buf = [0u8; 16 * 1024];
                loop {
                    match stderr.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if let Ok(mut d) = digest.lock() {
                                d.bytes = d.bytes.saturating_add(n as u64);
                                if let Some(h) = d.hasher.as_mut() {
                                    sha2::Digest::update(h, &buf[..n]);
                                }
                            }
                        }
                    }
                }
                if let Ok(mut d) = digest.lock() {
                    d.done = true;
                }
            }));
        }
        drop(event_tx);
        let host = Host {
            paths,
            launch,
            child,
            pgid,
            bridge_out: Some(bridge_tx),
            rpc_out: Some(rpc_tx),
            threads,
            stderr: digest,
        };
        Ok((host, events))
    }

    /// Queue one frame to the bridge (bounded; awaits backpressure).
    pub async fn send_bridge(
        &self,
        msg: ParentMessage,
        payload: Vec<u8>,
    ) -> Result<(), CaptureFault> {
        let out = self
            .bridge_out
            .as_ref()
            .ok_or(fault(FaultClass::Protocol, "bridge_channel_closed"))?;
        out.send((msg, payload))
            .await
            .map_err(|_| fault(FaultClass::Protocol, "bridge_channel_closed"))
    }

    /// Send one RPC command (a JSON object) as a JSONL line.
    pub async fn send_rpc(&self, command: &Value) -> Result<(), CaptureFault> {
        let out = self
            .rpc_out
            .as_ref()
            .ok_or(fault(FaultClass::Bootstrap, "rpc_stdin_closed"))?;
        let mut line = serde_json::to_vec(command)
            .map_err(|_| fault(FaultClass::Bootstrap, "rpc_encode_failed"))?;
        line.push(b'\n');
        out.send(line)
            .await
            .map_err(|_| fault(FaultClass::Bootstrap, "rpc_stdin_closed"))
    }

    /// Close the private channel's write side after a final `Close`.
    pub fn close_bridge(&mut self) {
        self.bridge_out = None;
    }

    /// Close RPC stdin: the host disposes its session and exits.
    pub fn close_rpc(&mut self) {
        self.rpc_out = None;
    }

    /// Wait for the host to exit on its own within `grace`, draining events
    /// so no producer blocks. Returns the drained events (summarized) and
    /// the exit. If the host does not exit in time, its whole process group
    /// is killed and the exit is marked forced.
    pub async fn finish(
        mut self,
        mut events: mpsc::Receiver<HostEvent>,
        grace: Duration,
    ) -> (Vec<HostEvent>, HostExit) {
        self.bridge_out = None;
        self.rpc_out = None;
        let deadline = tokio::time::Instant::now() + grace;
        let mut drained = Vec::new();
        let mut status = None;
        let mut producers_done = false;
        loop {
            if status.is_none()
                && let Ok(Some(s)) = self.child.try_wait()
            {
                status = Some(s);
            }
            if status.is_some() && producers_done {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            if producers_done {
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
            match tokio::time::timeout(Duration::from_millis(50), events.recv()).await {
                Ok(Some(e)) => drained.push(e),
                Ok(None) => producers_done = true,
                Err(_) => {}
            }
        }
        let mut forced = false;
        if status.is_none() {
            forced = true;
            // SAFETY: signalling our own child's process group.
            unsafe {
                libc::kill(-self.pgid, libc::SIGKILL);
            }
            status = self.child.wait().ok();
        }
        // Reap any remaining group members and confirm the group is gone.
        let mut group_gone = false;
        for _ in 0..40 {
            // SAFETY: probing our own child's process group.
            let r = unsafe { libc::kill(-self.pgid, 0) };
            if r != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                group_gone = true;
                break;
            }
            if !forced {
                // A leftover member after normal exit is still ours to stop.
                unsafe {
                    libc::kill(-self.pgid, libc::SIGKILL);
                }
                forced = true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        while let Ok(e) = events.try_recv() {
            drained.push(e);
        }
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        let (stderr_bytes, stderr_sha256) = match self.stderr.lock() {
            Ok(mut d) => {
                let hex = d
                    .hasher
                    .take()
                    .map(|h| {
                        let out = sha2::Digest::finalize(h);
                        out.iter().map(|b| format!("{b:02x}")).collect::<String>()
                    })
                    .unwrap_or_default();
                (d.bytes, hex)
            }
            Err(_) => (0, String::new()),
        };
        let (code, signal) = match status {
            Some(s) => {
                use std::os::unix::process::ExitStatusExt;
                (s.code(), s.signal())
            }
            None => (None, None),
        };
        let exit = HostExit {
            normal: !forced && code == Some(0),
            code,
            signal,
            forced,
            group_gone,
            stderr_bytes,
            stderr_sha256,
        };
        (drained, exit)
    }
}

impl Drop for Host {
    /// A host dropped without `finish` (an unwinding caller) is never left
    /// running: its whole process group is killed and reaped.
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            // SAFETY: signalling our own child's process group.
            unsafe {
                libc::kill(-self.pgid, libc::SIGKILL);
            }
            let _ = self.child.wait();
        }
    }
}

fn fstat_ino(fd: i32) -> Result<u64, CaptureFault> {
    // SAFETY: fstat on an fd we own into a zeroed stat buffer.
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let r = unsafe { libc::fstat(fd, &mut st) };
    if r != 0 || (st.st_mode & libc::S_IFMT) != libc::S_IFIFO {
        return Err(fault(FaultClass::Bootstrap, "pipe_identity_unavailable"));
    }
    Ok(st.st_ino as u64)
}

/// In the forked child: place the channel ends on fds 3 and 4 regardless
/// of their current numbers and mark every other fd >= 5 close-on-exec.
fn place_channel_fds(child_write: i32, child_read: i32) -> std::io::Result<()> {
    // SAFETY: async-signal-safe calls on fds valid in the child.
    unsafe {
        let a = libc::fcntl(child_write, libc::F_DUPFD, 16);
        let b = libc::fcntl(child_read, libc::F_DUPFD, 16);
        if a < 0 || b < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if libc::dup2(a, 3) < 0 || libc::dup2(b, 4) < 0 {
            return Err(std::io::Error::last_os_error());
        }
        libc::close(a);
        libc::close(b);
        for fd in 5..1024 {
            libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
        }
    }
    Ok(())
}

fn drain<R: Read>(r: &mut R) {
    let mut buf = [0u8; 64 * 1024];
    while matches!(r.read(&mut buf), Ok(n) if n > 0) {}
}

/// Read one LF-terminated line of at most `max` bytes. `Ok(None)` at EOF
/// with nothing buffered; a final unterminated line is returned as is.
fn read_bounded_line<R: BufRead>(r: &mut R, max: usize) -> Result<Option<Vec<u8>>, &'static str> {
    let mut line = Vec::new();
    loop {
        let buf = r.fill_buf().map_err(|_| "rpc_stdout_read_failed")?;
        if buf.is_empty() {
            return Ok(if line.is_empty() { None } else { Some(line) });
        }
        match buf.iter().position(|&b| b == b'\n') {
            Some(i) => {
                if line.len() + i > max {
                    return Err("rpc_line_too_large");
                }
                line.extend_from_slice(&buf[..i]);
                r.consume(i + 1);
                return Ok(Some(line));
            }
            None => {
                let n = buf.len();
                if line.len() + n > max {
                    return Err("rpc_line_too_large");
                }
                line.extend_from_slice(buf);
                r.consume(n);
            }
        }
    }
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Summarize one RPC stdout line. Unknown frame types are `Other`; frames
/// the pilot forbids are flagged; nothing textual is retained.
pub fn parse_rpc_line(line: &[u8]) -> Result<RpcEvent, &'static str> {
    if line.iter().all(u8::is_ascii_whitespace) {
        return Ok(RpcEvent::Other);
    }
    let v: Value = serde_json::from_slice(line).map_err(|_| "rpc_line_not_json")?;
    let kind = v
        .get("type")
        .and_then(Value::as_str)
        .ok_or("rpc_line_untyped")?;
    Ok(match kind {
        "ready" => RpcEvent::Ready,
        "response" => {
            let command = str_field(&v, "command").ok_or("rpc_response_shape")?;
            let success = v
                .get("success")
                .and_then(Value::as_bool)
                .ok_or("rpc_response_shape")?;
            let state = if command == "get_state" && success {
                v.get("data").map(state_from)
            } else {
                None
            };
            RpcEvent::Response {
                id: str_field(&v, "id"),
                command,
                success,
                state,
            }
        }
        "prompt_result" => RpcEvent::PromptResult {
            id: str_field(&v, "id"),
            status: str_field(&v, "status").ok_or("rpc_prompt_result_shape")?,
        },
        "session_settled" => RpcEvent::SessionSettled,
        "agent_end" => RpcEvent::AgentEnd,
        "auto_retry_start" | "auto_retry_end" => RpcEvent::Forbidden("host_auto_retry"),
        "auto_compaction_start" | "auto_compaction_end" => RpcEvent::Forbidden("host_compaction"),
        "cache_warming_start" | "cache_warming_end" => RpcEvent::Forbidden("host_cache_warming"),
        "retry_fallback_applied" => RpcEvent::Forbidden("host_model_fallback"),
        "model_changed" => RpcEvent::Forbidden("host_model_changed"),
        "extension_error" => RpcEvent::Forbidden("host_extension_error"),
        _ => RpcEvent::Other,
    })
}

fn state_from(data: &Value) -> HostState {
    let model = data.get("model");
    HostState {
        provider: model.and_then(|m| str_field(m, "provider")),
        model: model.and_then(|m| str_field(m, "id")),
        api: model.and_then(|m| str_field(m, "api")),
        max_tokens: model
            .and_then(|m| m.get("maxTokens"))
            .and_then(Value::as_u64),
        thinking_level: str_field(data, "thinkingLevel"),
        auto_compaction: data.get("autoCompactionEnabled").and_then(Value::as_bool),
        session_file: str_field(data, "sessionFile"),
    }
}

// ---------------------------------------------------------------------------
// Tool admission (ported from the accepted apparatus guard predicates).
// ---------------------------------------------------------------------------

/// The admitted tool scope of one owned workspace.
#[derive(Clone, Debug)]
pub struct ToolPolicy {
    /// Canonical workspace (realpath).
    pub workspace: PathBuf,
    /// Admitted read files, workspace-relative.
    pub read: Vec<String>,
    /// Admitted edit files, workspace-relative.
    pub edit: Vec<String>,
}

impl ToolPolicy {
    pub fn pilot(workspace: PathBuf) -> Self {
        Self {
            workspace,
            read: policy::READ_SET.iter().map(|s| s.to_string()).collect(),
            edit: policy::EDIT_SET.iter().map(|s| s.to_string()).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolVerdict {
    Allow,
    Refuse(&'static str),
}

const READ_KEYS: &[&str] = &["path", "i", "intent"];
const EDIT_KEYS: &[&str] = &[
    "path",
    "old_string",
    "new_string",
    "replace_all",
    "i",
    "intent",
];

/// Decide one native tool call before it runs.
pub fn admit_tool(policy: &ToolPolicy, name: &str, args: &[u8]) -> ToolVerdict {
    let is_read = match name {
        "read" => true,
        "edit" => false,
        _ => return ToolVerdict::Refuse("tool_not_allowed"),
    };
    let Ok(Value::Object(input)) = serde_json::from_slice::<Value>(args) else {
        return ToolVerdict::Refuse("input_not_object");
    };
    let allowed = if is_read { READ_KEYS } else { EDIT_KEYS };
    for key in input.keys() {
        if !allowed.contains(&key.as_str()) {
            return ToolVerdict::Refuse(if is_read {
                "read_key_forbidden"
            } else {
                "edit_key_forbidden"
            });
        }
    }
    for meta in ["i", "intent"] {
        if let Some(v) = input.get(meta) {
            match v.as_str() {
                Some(s) if s.len() <= 1024 => {}
                _ => return ToolVerdict::Refuse("intent_metadata_invalid"),
            }
        }
    }
    if !is_read {
        for key in ["old_string", "new_string"] {
            if let Some(v) = input.get(key)
                && !v.is_string()
            {
                return ToolVerdict::Refuse("edit_field_invalid");
            }
        }
        if let Some(v) = input.get("replace_all")
            && !v.is_boolean()
        {
            return ToolVerdict::Refuse("edit_field_invalid");
        }
    }
    let Some(raw) = input.get("path").and_then(Value::as_str) else {
        return ToolVerdict::Refuse("path_missing");
    };
    match admit_path(policy, raw, is_read) {
        Ok(()) => ToolVerdict::Allow,
        Err(code) => ToolVerdict::Refuse(code),
    }
}

fn is_safe_numeric_selector(sel: &str) -> bool {
    if sel.is_empty() || sel.len() > 128 {
        return false;
    }
    let num = |s: &str| !s.is_empty() && s.len() <= 9 && s.bytes().all(|b| b.is_ascii_digit());
    sel.split(',').all(|item| {
        if let Some(rest) = item.strip_prefix('-') {
            return num(rest);
        }
        if let Some((a, b)) = item.split_once('+') {
            return num(a) && num(b);
        }
        if let Some((a, b)) = item.split_once('-') {
            return num(a) && (b.is_empty() || num(b));
        }
        num(item)
    })
}

/// `posix.normalize(p) == p` for the path forms the guard admitted.
fn is_normalized(p: &str) -> bool {
    if p.is_empty() || p.contains("//") || (p.len() > 1 && p.ends_with('/')) {
        return false;
    }
    !p.split('/')
        .enumerate()
        .any(|(i, seg)| seg == ".." || seg == "." || (seg.is_empty() && i != 0))
}

fn admit_path(policy: &ToolPolicy, raw: &str, is_read: bool) -> Result<(), &'static str> {
    if raw.is_empty() || raw.len() > 1024 {
        return Err("path_length_invalid");
    }
    if raw.chars().any(char::is_control) {
        return Err("path_control_character");
    }
    if raw.contains('\\') {
        return Err("path_backslash");
    }
    if raw.starts_with('~') {
        return Err("path_home_relative");
    }
    let (file_part, has_selector) = if is_read {
        match raw.rfind(':') {
            Some(i) => {
                if !is_safe_numeric_selector(&raw[i + 1..]) {
                    return Err("selector_unsafe");
                }
                (&raw[..i], true)
            }
            None => (raw, false),
        }
    } else {
        (raw, false)
    };
    if file_part.is_empty() || file_part.contains(':') {
        return Err("path_uri_or_device_forbidden");
    }
    if !is_normalized(file_part) {
        return Err("path_not_canonical");
    }
    let abs = if file_part.starts_with('/') {
        PathBuf::from(file_part)
    } else {
        policy.workspace.join(file_part)
    };
    let set = if is_read { &policy.read } else { &policy.edit };
    let admitted = set.iter().any(|rel| policy.workspace.join(rel) == abs);
    if !admitted {
        return Err("path_not_admitted");
    }
    if has_selector {
        match fs::symlink_metadata(policy.workspace.join(raw)) {
            Ok(_) => return Err("selector_literal_shadow"),
            Err(e) => match e.raw_os_error() {
                Some(libc::ENOENT) | Some(libc::ENOTDIR) | Some(libc::ENAMETOOLONG) => {}
                _ => return Err("selector_probe_unknown"),
            },
        }
    }
    verify_admitted_file(&policy.workspace, &abs)
}

fn verify_admitted_file(workspace: &Path, abs: &Path) -> Result<(), &'static str> {
    match fs::canonicalize(workspace) {
        Ok(real) if real == workspace => {}
        _ => return Err("workspace_not_canonical"),
    }
    let rel = abs
        .strip_prefix(workspace)
        .map_err(|_| "path_escapes_workspace")?;
    let mut cur = workspace.to_path_buf();
    let comps: Vec<Component<'_>> = rel.components().collect();
    for (i, comp) in comps.iter().enumerate() {
        let Component::Normal(seg) = comp else {
            return Err("path_escapes_workspace");
        };
        cur.push(seg);
        let meta = fs::symlink_metadata(&cur).map_err(|_| "path_missing_on_disk")?;
        if meta.file_type().is_symlink() {
            return Err("path_symlink");
        }
        let last = i + 1 == comps.len();
        if !last && !meta.is_dir() {
            return Err("parent_not_directory");
        }
        if last && !meta.is_file() {
            return Err("not_regular_file");
        }
    }
    match fs::canonicalize(abs) {
        Ok(real) if real == abs => Ok(()),
        _ => Err("path_realpath_mismatch"),
    }
}

// ---------------------------------------------------------------------------
// Native session evidence.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeAssistant {
    pub entry_id: String,
    pub response_id: Option<String>,
    pub stop_reason: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NativeSession {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
    pub assistant: Vec<NativeAssistant>,
    /// Out-of-transcript `model_usage` entries (cache warming, helpers).
    pub model_usage_entries: u32,
    pub unparsed_lines: u32,
}

/// Read back the single native session file the host wrote, without
/// modifying it. `Ok(None)` when no session file exists (no assistant
/// message was produced).
pub fn read_native_session(sessions: &Path) -> Result<Option<NativeSession>, CaptureFault> {
    require_private_directory(sessions)?;
    verify_private_tree(sessions, &[], &mut PrivateTreeBudget::new(None))?;
    let mut files = Vec::new();
    for entry in fs::read_dir(sessions)
        .map_err(|_| fault(FaultClass::Usage, "native_sessions_unreadable"))?
    {
        let entry = entry.map_err(|_| fault(FaultClass::Usage, "native_sessions_unreadable"))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".jsonl") && !name.starts_with('.') {
            files.push(entry.path());
        }
    }
    match files.len() {
        0 => return Ok(None),
        1 => {}
        _ => return Err(fault(FaultClass::Usage, "native_session_ambiguous")),
    }
    let path = files.remove(0);
    let file = open_private_file(&path, PrivateArtifact::File)?;
    let meta = file
        .metadata()
        .map_err(|_| fault(FaultClass::Usage, "native_session_unreadable"))?;
    if meta.len() > MAX_NATIVE_SESSION_BYTES {
        return Err(fault(FaultClass::Usage, "native_session_shape"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_NATIVE_SESSION_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fault(FaultClass::Usage, "native_session_unreadable"))?;
    if bytes.len() as u64 > MAX_NATIVE_SESSION_BYTES {
        return Err(fault(FaultClass::Usage, "native_session_shape"));
    }
    let mut assistant = Vec::new();
    let mut model_usage_entries = 0u32;
    let mut unparsed_lines = 0u32;
    for line in bytes.split(|&b| b == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<Value>(line) else {
            unparsed_lines = unparsed_lines.saturating_add(1);
            continue;
        };
        match v.get("type").and_then(Value::as_str) {
            Some("message") => {
                let msg = v.get("message");
                if msg.and_then(|m| m.get("role")).and_then(Value::as_str) == Some("assistant") {
                    assistant.push(NativeAssistant {
                        entry_id: str_field(&v, "id").unwrap_or_default(),
                        response_id: msg.and_then(|m| str_field(m, "responseId")),
                        stop_reason: msg.and_then(|m| str_field(m, "stopReason")),
                    });
                }
            }
            Some("model_usage") => model_usage_entries = model_usage_entries.saturating_add(1),
            _ => {}
        }
    }
    Ok(Some(NativeSession {
        sha256: sha256_hex(&bytes),
        bytes: bytes.len() as u64,
        path,
        assistant,
        model_usage_entries,
        unparsed_lines,
    }))
}

/// File-system identity of a path (device, inode) for ownership checks.
pub fn file_identity(path: &Path) -> Option<(u64, u64)> {
    fs::symlink_metadata(path).ok().map(|m| (m.dev(), m.ino()))
}

#[cfg(test)]
mod tests {
    // Compatibility setup for NEW owned synthetic fixtures only. Existing
    // entries keep their modes, so deliberate negative mutations stay negative.
    // These helpers do not participate in product permission qualification.
    fn fixture_private_tempdir() -> std::io::Result<tempfile::TempDir> {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::Builder::new()
            .prefix("cf-owned-pilot-fixture-")
            .tempdir_in("/private/tmp")?;
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700))?;
        Ok(temp)
    }

    fn fixture_private_dir(path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(path.as_ref())?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
    }

    fn fixture_private_dirs(path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let mut missing = Vec::new();
        let mut cursor = path.as_ref();
        while !cursor.try_exists()? {
            missing.push(cursor.to_path_buf());
            cursor = cursor
                .parent()
                .ok_or_else(|| std::io::Error::other("owned fixture has no parent"))?;
        }
        for directory in missing.into_iter().rev() {
            fixture_private_dir(directory)?;
        }
        Ok(())
    }

    fn fixture_private_write(
        path: impl AsRef<std::path::Path>,
        bytes: impl AsRef<[u8]>,
    ) -> std::io::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let existed = path.as_ref().symlink_metadata().is_ok();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path.as_ref())?;
        if !existed {
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(bytes.as_ref())
    }

    use super::*;

    fn workspace() -> (tempfile::TempDir, ToolPolicy) {
        let dir = fixture_private_tempdir().unwrap();
        let ws = fs::canonicalize(dir.path()).unwrap();
        fixture_private_dirs(ws.join("src")).unwrap();
        for f in policy::READ_SET {
            fixture_private_write(ws.join(f), b"x").unwrap();
        }
        (dir, ToolPolicy::pilot(ws))
    }

    #[test]
    fn admits_only_the_frozen_read_and_edit_sets() {
        let (_d, p) = workspace();
        assert_eq!(
            admit_tool(&p, "read", br#"{"path":"src/lib.rs","i":"look"}"#),
            ToolVerdict::Allow
        );
        assert_eq!(
            admit_tool(&p, "read", br#"{"path":"LICENSE-MIT"}"#),
            ToolVerdict::Allow
        );
        let abs = format!(
            r#"{{"path":"{}"}}"#,
            p.workspace.join("Cargo.toml").display()
        );
        assert_eq!(admit_tool(&p, "read", abs.as_bytes()), ToolVerdict::Allow);
        assert_eq!(
            admit_tool(
                &p,
                "edit",
                br#"{"path":"src/lib.rs","old_string":"a","new_string":"b"}"#
            ),
            ToolVerdict::Allow
        );
        assert_eq!(
            admit_tool(
                &p,
                "edit",
                br#"{"path":"Cargo.toml","old_string":"a","new_string":"b"}"#
            ),
            ToolVerdict::Refuse("path_not_admitted")
        );
        assert_eq!(
            admit_tool(&p, "bash", br#"{"command":"ls"}"#),
            ToolVerdict::Refuse("tool_not_allowed")
        );
        assert_eq!(
            admit_tool(&p, "read", br#"{"path":"../x"}"#),
            ToolVerdict::Refuse("path_not_canonical")
        );
        assert_eq!(
            admit_tool(&p, "read", br#"{"path":"src/./lib.rs"}"#),
            ToolVerdict::Refuse("path_not_canonical")
        );
        assert_eq!(
            admit_tool(&p, "read", br#"{"path":"file:///etc/passwd"}"#),
            ToolVerdict::Refuse("selector_unsafe")
        );
        assert_eq!(
            admit_tool(&p, "read", br#"{"path":"~/.ssh/id"}"#),
            ToolVerdict::Refuse("path_home_relative")
        );
        assert_eq!(
            admit_tool(&p, "read", br#"{"path":"src/lib.rs","limit":3}"#),
            ToolVerdict::Refuse("read_key_forbidden")
        );
        assert_eq!(
            admit_tool(&p, "edit", br#"{"path":"src/lib.rs","replace_all":"yes"}"#),
            ToolVerdict::Refuse("edit_field_invalid")
        );
        assert_eq!(
            admit_tool(&p, "read", b"[1]"),
            ToolVerdict::Refuse("input_not_object")
        );
    }

    #[test]
    fn selectors_and_shadows_follow_the_guard() {
        let (_d, p) = workspace();
        assert_eq!(
            admit_tool(&p, "read", br#"{"path":"src/lib.rs:230-470"}"#),
            ToolVerdict::Allow
        );
        assert_eq!(
            admit_tool(&p, "read", br#"{"path":"src/lib.rs:1,5-,-3,7+2"}"#),
            ToolVerdict::Allow
        );
        assert_eq!(
            admit_tool(&p, "read", br#"{"path":"src/lib.rs:a"}"#),
            ToolVerdict::Refuse("selector_unsafe")
        );
        assert_eq!(
            admit_tool(&p, "edit", br#"{"path":"src/lib.rs:1"}"#),
            ToolVerdict::Refuse("path_uri_or_device_forbidden")
        );
        fixture_private_write(p.workspace.join("src/lib.rs:1"), b"shadow").unwrap();
        assert_eq!(
            admit_tool(&p, "read", br#"{"path":"src/lib.rs:1"}"#),
            ToolVerdict::Refuse("selector_literal_shadow")
        );
    }

    #[test]
    fn symlinks_and_non_files_are_refused() {
        let (_d, p) = workspace();
        fs::remove_file(p.workspace.join("LICENSE-MIT")).unwrap();
        std::os::unix::fs::symlink("/etc/hosts", p.workspace.join("LICENSE-MIT")).unwrap();
        assert_eq!(
            admit_tool(&p, "read", br#"{"path":"LICENSE-MIT"}"#),
            ToolVerdict::Refuse("path_symlink")
        );
        fs::remove_file(p.workspace.join("Cargo.toml")).unwrap();
        fixture_private_dir(p.workspace.join("Cargo.toml")).unwrap();
        assert_eq!(
            admit_tool(&p, "read", br#"{"path":"Cargo.toml"}"#),
            ToolVerdict::Refuse("not_regular_file")
        );
    }

    #[test]
    fn rpc_lines_are_summarized_without_content() {
        let state = br#"{"id":"s","type":"response","command":"get_state","success":true,"data":{"model":{"provider":"anthropic","id":"claude-sonnet-5-5","api":"cf-pilot-capture-v1","maxTokens":4096},"thinkingLevel":"xhigh","autoCompactionEnabled":false}}"#;
        match parse_rpc_line(state).unwrap() {
            RpcEvent::Response { state: Some(s), .. } => {
                assert_eq!(s.api.as_deref(), Some(policy::BRIDGE_API));
                assert_eq!(s.max_tokens, Some(4096));
                assert_eq!(s.auto_compaction, Some(false));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            parse_rpc_line(br#"{"type":"auto_retry_start","attempt":1}"#).unwrap(),
            RpcEvent::Forbidden("host_auto_retry")
        );
        assert_eq!(
            parse_rpc_line(
                br#"{"type":"prompt_result","id":"p1","status":"completed","agentInvoked":true}"#
            )
            .unwrap(),
            RpcEvent::PromptResult {
                id: Some("p1".into()),
                status: "completed".into()
            }
        );
        assert!(parse_rpc_line(b"not json").is_err());
    }

    #[test]
    fn bounded_lines_refuse_oversize() {
        let mut r = std::io::Cursor::new(b"abc\ndef".to_vec());
        assert_eq!(read_bounded_line(&mut r, 8).unwrap().unwrap(), b"abc");
        assert_eq!(read_bounded_line(&mut r, 8).unwrap().unwrap(), b"def");
        assert!(read_bounded_line(&mut r, 8).unwrap().is_none());
        let mut r = std::io::Cursor::new(vec![b'x'; 20]);
        assert_eq!(
            read_bounded_line(&mut r, 8).unwrap_err(),
            "rpc_line_too_large"
        );
    }

    #[test]
    fn sandbox_policy_denies_home_and_run_then_allows_host_root() {
        let p = sandbox_policy(
            Path::new("/Users/u/.local/bin/omp"),
            Path::new("/Users/u/run/hosts/h1"),
            Path::new("/Users/u/run/ws/pair1"),
            Path::new("/Users/u"),
            &[PathBuf::from("/Users/u/run")],
            &[PathBuf::from("/Users/u/cred")],
        )
        .unwrap();
        let deny_run = p
            .find("(deny file-read-data (subpath \"/Users/u/run\"))")
            .unwrap();
        let allow_root = p.find("(subpath \"/Users/u/run/hosts/h1\")").unwrap();
        let deny_cred = p
            .find("(deny file-read-data (literal \"/Users/u/cred\"))")
            .unwrap();
        assert!(p.starts_with("(version 1)(allow default)(deny network*)"));
        assert!(deny_run < allow_root && allow_root < deny_cred);
        assert!(!p.contains("network-outbound"));
        assert!(
            sandbox_policy(
                Path::new("/a\"b"),
                Path::new("/r"),
                Path::new("/w"),
                Path::new("/h"),
                &[],
                &[]
            )
            .is_err()
        );
    }

    #[test]
    fn native_session_identity_is_read_back() {
        let dir = fixture_private_tempdir().unwrap();
        assert!(read_native_session(dir.path()).unwrap().is_none());
        fixture_private_write(
            dir.path().join("2026_x.jsonl"),
            b"{\"type\":\"session\",\"id\":\"s\"}\n{\"type\":\"message\",\"id\":\"e1\",\"message\":{\"role\":\"assistant\",\"responseId\":\"msg_1\",\"stopReason\":\"stop\"}}\n",
        )
        .unwrap();
        let s = read_native_session(dir.path()).unwrap().unwrap();
        assert_eq!(s.assistant.len(), 1);
        assert_eq!(s.assistant[0].response_id.as_deref(), Some("msg_1"));
        assert_eq!(s.model_usage_entries, 0);
    }
    #[test]
    fn privacy_native_session_readback_requires_private_regular_files() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let bytes=b"{\"type\":\"session\",\"id\":\"owned\"}\n{\"type\":\"message\",\"id\":\"owned-entry\",\"message\":{\"role\":\"assistant\",\"responseId\":\"msg_owned\",\"stopReason\":\"stop\"}}\n";
        for case in ["private", "wide", "executable", "special", "symlink"] {
            eprintln!("owned native-session privacy case: {case}");
            let temp = fixture_private_tempdir().unwrap();
            let root = fs::canonicalize(temp.path()).unwrap();
            let sessions = root.join("sessions");
            create_private_dir(&sessions).unwrap();
            let path = sessions.join("owned.jsonl");
            write_private_new(&path, bytes).unwrap();
            match case {
                "wide" => fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap(),
                "executable" => {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap()
                }
                "special" => {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o1600)).unwrap()
                }
                "symlink" => {
                    let target = root.join("owned-target");
                    fs::rename(&path, &target).unwrap();
                    symlink(target, &path).unwrap();
                }
                "private" => {}
                _ => unreachable!(),
            }
            let result = read_native_session(&sessions);
            if case == "private" {
                assert_eq!(
                    result.unwrap().unwrap().assistant[0].response_id.as_deref(),
                    Some("msg_owned")
                );
            } else {
                let fault = result.unwrap_err();
                assert_eq!(
                    (fault.class, fault.code),
                    (FaultClass::Privacy, "private_artifact_not_private")
                );
            }
        }
    }
}
