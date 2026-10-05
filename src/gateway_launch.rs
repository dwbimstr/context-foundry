//! `foundry gateway-omp` (003 T004): the supported OMP launcher. It owns a
//! fresh, non-default OMP profile whose only configuration is this run's
//! loopback gateway, verifies the routed setup before launching OMP, and
//! passes no upstream credential route to the host. A failed launch never
//! falls back to a direct upstream route.
//!
//! OMP 18.6.0 exposes no query for a profile's effective `baseUrl`
//! (`omp models --json` omits it), so the pre-launch verification is
//! structural: the fresh profile must hold exactly the generated
//! `models.yml`, the token file must hold this run's token, and an
//! authenticated `/health` must answer. A silently dropped registry override
//! fails closed: the fresh profile carries no auth and OMP's environment
//! carries no upstream credential, so a direct request has nothing to send.

use std::{
    io::{BufRead as _, Read as _, Write as _},
    os::unix::process::{CommandExt as _, ExitStatusExt as _},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicI32, Ordering},
        mpsc::{Receiver, channel},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use serde_json::Value;

use crate::{
    adapter_error::{AResult, AdapterError},
    gateway::{MODEL_ID, TOKEN_ENV, parse_config},
};

pub struct Options {
    pub config: PathBuf,
    pub key_file: PathBuf,
    pub profile: String,
    /// `low|high|max`; explicit so OMP never runs `auto` judge side requests.
    pub thinking: String,
    pub omp: PathBuf,
    pub omp_args: Vec<String>,
}

fn invalid(message: &str) -> AdapterError {
    AdapterError::named("invalid_argument", message)
}

fn unavailable(message: impl Into<String>) -> AdapterError {
    AdapterError::runtime("gateway_unavailable", message)
}

// ---------------------------------------------------------------------------
// Signals: SIGINT/SIGTERM reach OMP first; the launcher then cleans up
// ---------------------------------------------------------------------------

static OMP_PID: AtomicI32 = AtomicI32::new(0);
/// A signal that arrived before OMP existed (0 = none).
static PENDING_SIGNAL: AtomicI32 = AtomicI32::new(0);

/// A terminal's Ctrl-C reaches every process in the foreground group, and
/// OMP shares the launcher's group (it needs interactive stdio), so
/// forwarding a tty-generated SIGINT again would double-deliver the
/// interrupt. kill(2)-sent signals name one process, and the launcher is
/// the one that must pass them on. siginfo tells them apart:
/// - macOS: a kill(2) reports `SI_USER` with the sender's nonzero `si_pid`;
///   tty/kernel delivery carries `si_pid == 0`.
/// - Linux: user-space senders use `si_code <= 0` (`SI_USER`, `SI_QUEUE`,
///   `SI_TKILL`); tty/kernel delivery is a positive code (`SI_KERNEL`).
///
/// The terminal case itself is not covered by a test: reproducing it needs
/// a session-leading pty and a real foreground-group interrupt. The
/// directed case (kill(2) to the launcher with stdin on a pty) is tested;
/// it is exactly the case the previous `isatty` heuristic swallowed.
extern "C" fn on_signal(
    signal: libc::c_int,
    info: *mut libc::siginfo_t,
    _context: *mut libc::c_void,
) {
    // Async-signal-safe only: atomics, one siginfo read and kill(2).
    // SAFETY: siginfo is kernel-provided and valid inside its own handler.
    #[cfg(target_os = "macos")]
    let directed_by_kill = unsafe { (*info).si_pid != 0 };
    // SAFETY: as above.
    #[cfg(not(target_os = "macos"))]
    let directed_by_kill = unsafe { (*info).si_code <= 0 };
    let forward = signal != libc::SIGINT || directed_by_kill;
    let pid = OMP_PID.load(Ordering::SeqCst);
    if pid > 0 {
        if forward {
            // SAFETY: kill(2) on the child pid this process spawned.
            unsafe { libc::kill(pid, signal) };
        }
    } else {
        PENDING_SIGNAL.store(signal, Ordering::SeqCst);
    }
}

fn install_signal_forwarding() {
    // SAFETY: a zeroed sigaction is a valid empty starting point (no
    // handler, empty mask); the fields are then set explicitly.
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    action.sa_sigaction = on_signal
        as extern "C" fn(libc::c_int, *mut libc::siginfo_t, *mut libc::c_void)
        as libc::sighandler_t;
    action.sa_flags = libc::SA_SIGINFO;
    // SAFETY: conventional sigaction(2) registration of an
    // async-signal-safe handler; the old disposition is not needed.
    unsafe {
        libc::sigaction(libc::SIGINT, &action, std::ptr::null_mut());
        libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut());
    }
}

// ---------------------------------------------------------------------------
// Profile name validation: OMP's own rule plus never `default`
// ---------------------------------------------------------------------------

const WINDOWS_RESERVED: [&str; 24] = [
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
    "COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// `^[a-z0-9][a-z0-9._-]{0,63}$`, not `.`/`..`, not ending in `.`, not a
/// Windows reserved device name (optionally with an extension) and not
/// `default`.
fn valid_profile_name(name: &str) -> bool {
    if name == "default" || name == "." || name == ".." || name.ends_with('.') {
        return false;
    }
    let mut bytes = name.bytes();
    let first_ok = bytes
        .next()
        .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    let rest_ok = bytes
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'));
    if !first_ok || !rest_ok || name.len() > 64 {
        return false;
    }
    let device = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    !WINDOWS_RESERVED.contains(&device.as_str())
}

/// Options the launcher owns on OMP's command line: the profile, model,
/// thinking and configuration, every credential route, the loaders that
/// bring in other code or configuration (hooks, plugins, extensions), and
/// the model-role switches (`--smol`, `--slow`, the plan and prewalk
/// family) that would send traffic for a different model through the
/// gateway or past it. Overriding any of them through trailing arguments
/// would invalidate the verified profile or hand OMP a direct upstream
/// credential, so they refuse before the gateway is started or anything is
/// written. Both `--flag value` and `--flag=value` forms carry the same name.
const REFUSED_OMP_FLAGS: [&str; 20] = [
    "api-key",
    "profile",
    "alias",
    "model",
    "models",
    "provider",
    "thinking",
    "config",
    "extension",
    "external-thinking",
    "service-tier",
    "hook",
    "plugin-dir",
    "smol",
    "slow",
    "plan",
    "prewalk",
    "prewalk-into",
    "plan-yolo",
    "plan-yolo-into",
];

fn validate_omp_args(args: &[String]) -> AResult<()> {
    for arg in args {
        let name = if arg == "-e" {
            return Err(invalid(
                "`-e`/`--extension` is owned by the launcher profile",
            ));
        } else {
            let Some(long) = arg.strip_prefix("--") else {
                continue;
            };
            long.split('=').next().unwrap_or(long).to_ascii_lowercase()
        };
        if REFUSED_OMP_FLAGS.contains(&name.as_str()) {
            return Err(invalid(&format!(
                "`--{name}` is owned by the launcher or is a credential route"
            )));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The gateway child
// ---------------------------------------------------------------------------

/// The spawned gateway plus a reader that keeps draining its stdout so the
/// gateway's final summary line can never block on a full pipe.
struct GatewayChild {
    child: Child,
    lines: Receiver<String>,
    reader: Option<JoinHandle<()>>,
}

impl GatewayChild {
    fn spawn(mut command: Command) -> AResult<Self> {
        let mut child = command
            .spawn()
            .map_err(|e| unavailable(format!("cannot spawn the gateway: {e}")))?;
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(unavailable("the gateway child has no stdout"));
        };
        let (sender, lines) = channel();
        let reader = std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            lines,
            reader: Some(reader),
        })
    }

    /// The gateway's one JSON ready line within 10 s.
    fn ready(&self) -> AResult<Value> {
        let line = self
            .lines
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| unavailable("the gateway did not report readiness within 10 s"))?;
        let value: Value = serde_json::from_str(&line)
            .map_err(|_| unavailable("the gateway ready line is not JSON"))?;
        if value.get("v").and_then(Value::as_u64) != Some(1)
            || value.get("token_env").and_then(Value::as_str) != Some(TOKEN_ENV)
        {
            return Err(unavailable(
                "the gateway ready line is not this launcher's protocol",
            ));
        }
        Ok(value)
    }

    /// SIGTERM, up to 10 s of grace, then SIGKILL. The gateway's final
    /// summary line (counts and known totals; no secrets) goes to stderr.
    fn stop(&mut self) {
        // An already reaped child must not be signalled: its pid may be reused.
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            // SAFETY: kill(2) on our own live, unreaped child.
            unsafe { libc::kill(self.child.id() as i32, libc::SIGTERM) };
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                match self.child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    _ => {
                        let _ = self.child.kill();
                        let _ = self.child.wait();
                        break;
                    }
                }
            }
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        for line in self.lines.try_iter() {
            eprintln!("{line}");
        }
    }
}

/// Everything this launch created and must undo on every exit path: the
/// gateway, the generated `models.yml`, and the directories it made (removed
/// only while empty, so OMP's session files are never touched).
struct LaunchGuard {
    gateway: Option<GatewayChild>,
    profile_dir: Option<PathBuf>,
    models_yml: Option<PathBuf>,
}

impl Drop for LaunchGuard {
    fn drop(&mut self) {
        // The host is already stopped; the gateway goes first so no request
        // can arrive while the profile is being cleaned up.
        if let Some(gateway) = self.gateway.as_mut() {
            gateway.stop();
        }
        if let Some(models_yml) = &self.models_yml {
            let _ = std::fs::remove_file(models_yml);
        }
        if let Some(profile_dir) = &self.profile_dir {
            let _ = std::fs::remove_dir(profile_dir.join("agent"));
            let _ = std::fs::remove_dir(profile_dir);
        }
    }
}

/// The allowlisted child environment: nothing ambient, no inherited
/// credential routes.
fn env_allowlist(names: &[&str]) -> Vec<(String, String)> {
    names
        .iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| (name.to_string(), value))
        })
        .collect()
}

fn read_key_file(path: &Path) -> AResult<String> {
    let mut raw = String::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(64 * 1024).read_to_string(&mut raw))
        .map_err(|e| invalid(&format!("cannot read the key file: {e}")))?;
    let line = raw.strip_suffix('\n').unwrap_or(&raw);
    let line = line.strip_suffix('\r').unwrap_or(line);
    if line.is_empty() || line.contains(['\n', '\r']) {
        return Err(invalid("the key file must hold exactly one nonempty line"));
    }
    Ok(line.to_owned())
}

/// The shared `~/.omp/profiles` root (recursive; shared, never removed).
fn create_profiles_root(profiles_root: &Path) -> AResult<()> {
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .recursive(true)
        .create(profiles_root)
        .map_err(|e| unavailable(format!("cannot create the OMP profiles root: {e}")))
}

/// Exclusive owner-private directory creation; `profile_exists` if a profile
/// appeared since the pre-check.
fn create_dir_exclusive(path: &Path, what: &str) -> AResult<()> {
    use std::os::unix::fs::DirBuilderExt as _;
    let refused = |e: std::io::Error| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            AdapterError::named(
                "profile_exists",
                "the OMP profile already exists; refusing to overwrite it",
            )
        } else {
            unavailable(format!("cannot create the {what}: {e}"))
        }
    };
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .map_err(refused)
}

/// Create models.yml exclusively (0600) and close it: the caller owns the
/// path for cleanup the moment this returns, before any byte is written.
fn create_models_yml(path: &Path) -> AResult<()> {
    use std::os::unix::fs::OpenOptionsExt as _;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| unavailable(format!("cannot create models.yml: {e}")))?;
    Ok(())
}

/// Write the generated text through a fresh handle: a file whose permissions
/// changed between creation and this write fails here for real.
fn write_models_yml(path: &Path, text: &[u8]) -> AResult<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| unavailable(format!("cannot write models.yml: {e}")))?;
    file.write_all(text)
        .map_err(|e| unavailable(format!("cannot write models.yml: {e}")))
}

/// The profile tree must hold exactly the generated file: the profile was
/// refused if it existed, so anything else means a race or a misdirected
/// write. A corrupt or missing GENERATED profile is `profile_invalid`,
/// distinct from the gateway being unavailable.
fn verify_profile(profile_dir: &Path, models_yml: &Path, expected: &[u8]) -> AResult<()> {
    let invalid = |message: &str| AdapterError::named("profile_invalid", message);
    let on_disk = std::fs::read(models_yml)
        .map_err(|e| invalid(&format!("cannot re-read models.yml: {e}")))?;
    if on_disk != expected {
        return Err(invalid(
            "the profile's models.yml differs from the gateway's printed configuration",
        ));
    }
    let names = |dir: &Path| -> AResult<Vec<String>> {
        std::fs::read_dir(dir)
            .map_err(|e| invalid(&format!("cannot list the profile: {e}")))?
            .map(|entry| {
                entry
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .map_err(|e| invalid(&format!("cannot list the profile: {e}")))
            })
            .collect()
    };
    if names(profile_dir)? != ["agent"] || names(&profile_dir.join("agent"))? != ["models.yml"] {
        return Err(invalid(
            "the fresh profile must contain only the generated models.yml",
        ));
    }
    Ok(())
}

fn read_token(token_file: &Path) -> AResult<String> {
    let raw = std::fs::read_to_string(token_file).unwrap_or_default();
    let token = raw.trim();
    if token.len() == 64
        && token
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        Ok(token.to_owned())
    } else {
        Err(AdapterError::runtime(
            "token_missing",
            "the gateway token file is absent, empty or malformed",
        ))
    }
}

/// An authenticated `GET /health` over loopback that names this run.
fn verify_health(ready: &Value, token: &str) -> AResult<()> {
    let url = ready
        .get("url")
        .and_then(Value::as_str)
        .and_then(|url| url.strip_suffix("/v1"))
        .ok_or_else(|| unavailable("the gateway ready line lacks its URL"))?;
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .max_redirects(0)
        .proxy(None)
        .build()
        .new_agent();
    let mut response = agent
        .get(format!("{url}/health"))
        .header("authorization", format!("Bearer {token}"))
        .call()
        .map_err(|_| unavailable("the gateway health check failed"))?;
    let body: Value = response
        .body_mut()
        .with_config()
        .limit(64 * 1024)
        .read_json()
        .map_err(|_| unavailable("the gateway health reply is not JSON"))?;
    if body.get("ready").and_then(Value::as_bool) != Some(true)
        || body.get("session_id") != ready.get("session_id")
    {
        return Err(unavailable("the gateway is not ready for this run"));
    }
    Ok(())
}

/// Returns OMP's exit status for the launcher to exit with.
pub fn run(options: &Options) -> AResult<i32> {
    install_signal_forwarding();
    if !valid_profile_name(&options.profile) {
        return Err(invalid(
            "profile names match ^[a-z0-9][a-z0-9._-]{0,63}$ and are not default, ., .. or a reserved device name",
        ));
    }
    validate_omp_args(&options.omp_args)?;
    let home = std::env::var("HOME")
        .map_err(|_| invalid("HOME is not set; the OMP profile root is unknown"))?;
    let profiles_root = Path::new(&home).join(".omp").join("profiles");
    let profile_dir = profiles_root.join(&options.profile);
    // Any existing form (file, directory, dangling symlink) refuses.
    if std::fs::symlink_metadata(&profile_dir).is_ok() {
        return Err(AdapterError::named(
            "profile_exists",
            "the OMP profile already exists; refusing to overwrite it",
        ));
    }
    let key = read_key_file(&options.key_file)?;
    let mut config_bytes = Vec::new();
    std::fs::File::open(&options.config)
        .and_then(|file| file.take(64 * 1024 + 1).read_to_end(&mut config_bytes))
        .map_err(|e| invalid(&format!("cannot read the gateway config: {e}")))?;
    let config = parse_config(&config_bytes)?;

    let mut guard = LaunchGuard {
        gateway: None,
        profile_dir: None,
        models_yml: None,
    };

    // The gateway gets a minimal environment: the upstream key reaches this
    // child and nothing else. Its own process group keeps a terminal's
    // Ctrl-C away from it; the launcher stops it after OMP.
    let exe = std::env::current_exe().map_err(|e| unavailable(format!("current_exe: {e}")))?;
    let mut command = Command::new(exe);
    command
        .arg("gateway")
        .arg("--config")
        .arg(&options.config)
        .env_clear()
        .envs(env_allowlist(&["PATH", "HOME", "TMPDIR", "LANG", "LC_ALL"]))
        .env(&config.credential_env, &key)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .process_group(0);
    #[cfg(feature = "test-faults")]
    {
        // Test overrides (fake upstream, short timeouts) travel to the
        // gateway; release builds contain none of this.
        for (name, value) in
            std::env::vars().filter(|(name, _)| name.starts_with("FOUNDRY_GATEWAY_TEST_"))
        {
            command.env(name, value);
        }
    }
    // The guard owns the child before anything can fail, so every refusal
    // below stops the gateway.
    let gateway = GatewayChild::spawn(command)?;
    let ready = gateway.ready();
    guard.gateway = Some(gateway);
    let ready = ready?;
    let printed_yml = ready
        .get("omp_models_yml")
        .and_then(Value::as_str)
        .ok_or_else(|| unavailable("the gateway ready line lacks omp_models_yml"))?
        .to_owned();
    let token_file = ready
        .get("token_file")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| unavailable("the gateway ready line lacks token_file"))?;

    // The generated profile: only its own models.yml, owner-private. Each
    // exclusive creation is registered with the cleanup guard BEFORE the
    // next fallible step, so a partial profile never outlives the launch.
    create_profiles_root(&profiles_root)?;
    create_dir_exclusive(&profile_dir, "OMP profile")?;
    guard.profile_dir = Some(profile_dir.clone());
    #[cfg(feature = "test-faults")]
    {
        // An armed failure here makes creating `agent` fail (read-only
        // profile dir): the just-created profile must still be cleaned up.
        if fault!(GATEWAY_LAUNCH_BLOCK_AGENT_DIR, None, None, "").is_err() {
            use std::os::unix::fs::PermissionsExt as _;
            let _ = std::fs::set_permissions(&profile_dir, std::fs::Permissions::from_mode(0o500));
        }
    }
    create_dir_exclusive(&profile_dir.join("agent"), "OMP profile agent directory")?;
    let models_yml = profile_dir.join("agent").join("models.yml");
    create_models_yml(&models_yml)?;
    guard.models_yml = Some(models_yml.clone());
    #[cfg(feature = "test-faults")]
    {
        // An armed failure here makes the first write fail (read-only
        // file): the created-but-unwritten models.yml must still be cleaned
        // up rather than blocking the next launch as a corrupt profile.
        if fault!(GATEWAY_LAUNCH_AFTER_MODELS_CREATE, None, None, "").is_err() {
            use std::os::unix::fs::PermissionsExt as _;
            let _ = std::fs::set_permissions(&models_yml, std::fs::Permissions::from_mode(0o400));
        }
    }
    write_models_yml(&models_yml, printed_yml.as_bytes())?;

    #[cfg(feature = "test-faults")]
    {
        // An armed failure here corrupts the freshly written file so the
        // verification below runs against a real mismatch.
        if fault!(GATEWAY_LAUNCH_AFTER_MODELS_WRITE, None, None, "").is_err() {
            let _ = std::fs::write(&models_yml, b"corrupt\n");
        }
    }
    verify_profile(&profile_dir, &models_yml, printed_yml.as_bytes())?;

    #[cfg(feature = "test-faults")]
    {
        // An armed failure here empties the token file: the refusal for a
        // missing token runs against a real empty file.
        if fault!(GATEWAY_LAUNCH_BEFORE_TOKEN_READ, None, None, "").is_err() {
            let _ = std::fs::write(&token_file, b"");
        }
    }
    let token = read_token(&token_file)?;

    #[cfg(feature = "test-faults")]
    {
        // An armed failure here stops the gateway, so the health check faces
        // a really absent listener.
        if fault!(GATEWAY_LAUNCH_BEFORE_HEALTH, None, None, "").is_err()
            && let Some(gateway) = guard.gateway.as_mut()
        {
            let _ = gateway.child.kill();
            let _ = gateway.child.wait();
        }
    }
    verify_health(&ready, &token)?;

    if PENDING_SIGNAL.load(Ordering::SeqCst) != 0 {
        return Err(AdapterError::runtime(
            "cancelled",
            "interrupted before OMP started",
        ));
    }
    // OMP: routed through this gateway only. No ZAI_API_KEY, no auth broker,
    // never --api-key; the local token is the only credential it holds.
    let mut omp = Command::new(&options.omp);
    omp.args([
        "--profile",
        options.profile.as_str(),
        "--model",
        &format!("zai/{MODEL_ID}"),
        "--thinking",
        options.thinking.as_str(),
    ])
    .args(&options.omp_args)
    .env_clear()
    .envs(env_allowlist(&[
        "PATH",
        "HOME",
        "TMPDIR",
        "TERM",
        "COLORTERM",
        "LANG",
        "LC_ALL",
        "USER",
        "LOGNAME",
        "SHELL",
    ]))
    .env(TOKEN_ENV, &token)
    .env("PI_NO_TITLE", "1")
    .stdin(Stdio::inherit())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit());
    let mut omp = omp
        .spawn()
        .map_err(|e| AdapterError::runtime("omp_unavailable", format!("cannot spawn OMP: {e}")))?;
    let omp_pid = omp.id() as i32;
    OMP_PID.store(omp_pid, Ordering::SeqCst);
    let pending = PENDING_SIGNAL.swap(0, Ordering::SeqCst);
    if pending != 0 {
        // SAFETY: kill(2) on the OMP child just spawned.
        unsafe { libc::kill(omp_pid, pending) };
    }
    let status = loop {
        match omp.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                OMP_PID.store(0, Ordering::SeqCst);
                return Err(AdapterError::runtime(
                    "omp_unavailable",
                    format!("cannot wait for OMP: {e}"),
                ));
            }
        }
    };
    OMP_PID.store(0, Ordering::SeqCst);
    // OMP is stopped; dropping the guard stops the gateway and removes only
    // the generated models.yml (the gateway removes its own token). Session
    // files and receipts stay.
    drop(guard);
    Ok(status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::{REFUSED_OMP_FLAGS, valid_profile_name, validate_omp_args};

    #[test]
    fn profile_names_follow_omp_and_exclude_default_and_device_names() {
        for good in ["a", "dev-1", "0abc", "foundry.gw_2", &"a".repeat(64)] {
            assert!(valid_profile_name(good), "{good}");
        }
        for bad in [
            "",
            "default",
            ".",
            "..",
            "a.",
            "-x",
            "_x",
            ".x",
            "Upper",
            "has space",
            "a/b",
            "con",
            "nul.txt",
            "COM1",
            "lpt9.log",
            "aux",
            "com0",
            "lpt0.x",
            &"a".repeat(65),
        ] {
            assert!(!valid_profile_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn owned_and_credential_omp_arguments_refuse_in_both_forms() {
        // Every owned or credential flag refuses in both forms.
        for name in REFUSED_OMP_FLAGS {
            for form in [format!("--{name}"), format!("--{name}=value")] {
                let args = [form.clone()];
                let code = validate_omp_args(&args).err().unwrap().code();
                assert_eq!(code, "invalid_argument", "{form}");
            }
        }
        // The named set, spelled out so the list cannot silently shrink.
        for refused in [
            "--api-key",
            "--api-key=secret",
            "--profile",
            "--profile=default",
            "--alias=x",
            "--model",
            "--model=glm-5.3",
            "--models=other",
            "--provider",
            "--thinking=auto",
            "--config=/etc/x",
            "--extension",
            "--extension=tools",
            "-e",
            "--external-thinking",
            "--service-tier=flex",
            "--hook",
            "--hook=/x.ts",
            "--plugin-dir",
            "--plugin-dir=/plugins",
            "--smol",
            "--smol=x",
            "--slow",
            "--slow=x",
            "--plan",
            "--plan=x",
            "--prewalk",
            "--prewalk=x",
            "--prewalk-into",
            "--prewalk-into=x",
            "--plan-yolo",
            "--plan-yolo=x",
            "--plan-yolo-into",
            "--plan-yolo-into=x",
        ] {
            let args = [refused.to_owned()];
            let code = validate_omp_args(&args).err().unwrap().code();
            assert_eq!(code, "invalid_argument", "{refused}");
        }
        // A value that merely looks like a flag, unrelated flags and
        // ordinary OMP arguments pass through.
        let kept = [
            "--verbose",
            "--continue-flag",
            "extra-file.txt",
            "--modelish",
            "--",
        ];
        validate_omp_args(&kept.iter().map(|arg| arg.to_string()).collect::<Vec<_>>()).unwrap();
    }
}
