//! 009 worker supervisor: verifies the profile before launch, starts the
//! bundle executable in a jailed environment, polices readiness, memory and
//! liveness, and serves [`EmbeddingProvider`] over worker IPC.
//!
//! Launch happens only under an accepted profile and the development
//! isolation flag; otherwise [`acquire_until`] refuses with `isolation_unavailable`
//! and source access stays untouched. Shutdown is close-stdin, TERM, a 5 s
//! grace, KILL and reap through the child handle; ownership is released only
//! after the process is gone, and no replacement starts before that.
use super::profile::{SemanticProfile, control_stop, hash_regular_file_until};
use super::protocol::{self, Header, Purpose};
use super::provider::{
    EmbeddingProvider, FunctionDescriptor, ProviderError, TokenizedInput, check_document_batch,
    check_query,
};
use super::worker_runtime;
use crate::control::Control;
use std::io::{BufReader, Read};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// How often the supervisor polls the worker's physical footprint.
const MEMORY_POLL_INTERVAL: Duration = Duration::from_millis(250);
/// Grace between TERM and KILL at shutdown (spec: 5 s).
const TERM_GRACE: Duration = Duration::from_secs(5);
/// Grace an in-flight call gets after its caller gives up (budget rule);
/// at expiry the worker is stopped and the call reports a timeout.
const IN_FLIGHT_GRACE: Duration = Duration::from_secs(30);
/// Slice used when waiting on a reply while checking deadlines.
const WAIT_SLICE: Duration = Duration::from_millis(50);
/// A `busy` answer to a request sent while no earlier request is in flight
/// is the worker still releasing its slot after its last reply; retry for at
/// most this long, every [`BUSY_RETRY_STEP`], before reporting it.
const BUSY_RETRY_WINDOW: Duration = Duration::from_millis(500);
const BUSY_RETRY_STEP: Duration = Duration::from_millis(5);

/// The readiness receive slice. Tests widen it (through
/// [`set_launch_wait_slice`]) so a `ready` that crosses the acquisition's
/// stop inside one slice is deterministic instead of a scheduling race.
fn launch_wait_slice() -> Duration {
    #[cfg(feature = "test-faults")]
    {
        let ms = LAUNCH_WAIT_SLICE_MS.load(Ordering::Relaxed);
        if ms != 0 {
            return Duration::from_millis(ms);
        }
    }
    WAIT_SLICE
}

/// Tests only: widen the readiness receive slice for the whole process.
/// The slice is still bounded by the acquisition's deadline, so a widened
/// slice cannot make an acquisition overshoot its bound.
#[cfg(feature = "test-faults")]
pub fn set_launch_wait_slice(slice: Duration) {
    LAUNCH_WAIT_SLICE_MS.store(slice.as_millis() as u64, Ordering::Relaxed);
}

#[cfg(feature = "test-faults")]
static LAUNCH_WAIT_SLICE_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The seam the preparation path calls. Normal admission stays closed until
/// platform isolation acceptance; `development` runs the script-built,
/// ad-hoc-signed profile under the owner's explicit authorization.
///
/// Worker start-up, the hello/ready exchange and the model load wait at most
/// the profile's `load_timeout_seconds` and only while `control` has neither
/// passed its deadline (`Timeout`) nor been cancelled (`Cancelled`); the
/// control is checked every wait slice, so cancellation is prompt. A
/// half-started worker is stopped and reaped (TERM, 5 s, KILL) before this
/// returns, so nothing replaces it before it is gone.
pub fn acquire_until(
    profile: &SemanticProfile,
    development: bool,
    control: &Control,
) -> Result<Box<dyn EmbeddingProvider>, ProviderError> {
    if !development {
        return Err(ProviderError::IsolationUnavailable(
            "normal semantic admission stays closed until platform isolation acceptance; \
             pass --development-isolation under the owner's authorization"
                .into(),
        ));
    }
    let provider = WorkerProvider::launch_with(profile, &[], control)?;
    Ok(Box::new(provider))
}

/// Create this run's private scratch directory under `root`, the profile's
/// scratch root, which the signed bundle grants read-write and nothing else
/// writable. The root is created owner-private when absent; a pre-existing
/// root that is a symlink, not a directory, owned by another user or
/// writable by group or others is refused, because the worker's HOME and
/// TMPDIR live below it. The run directory and its `tmp` are mode 0700
/// whatever the umask, and removed after the worker is reaped.
fn prepare_run_scratch(profile: &SemanticProfile) -> Result<PathBuf, ProviderError> {
    let root = profile.worker.scratch_root.clone();
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    let refuse = |why: String| {
        ProviderError::IsolationUnavailable(format!("scratch root {}: {why}", root.display()))
    };
    let private = |dir: &Path| -> std::io::Result<()> {
        std::fs::DirBuilder::new().mode(0o700).create(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
    };
    // Canonical separation first, with the leaf allowed to be absent:
    // NOTHING is created inside or beside a read-only tree.
    profile
        .check_scratch_disjoint()
        .map_err(|e| ProviderError::ProfileInvalid(e.to_string()))?;
    match std::fs::symlink_metadata(&root) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(refuse("is a symlink".into()));
            }
            if !meta.is_dir() {
                return Err(refuse("is not a directory".into()));
            }
            let owner = unsafe { libc::geteuid() };
            if meta.uid() != owner {
                return Err(refuse(format!(
                    "is owned by uid {}, not by this process's uid {owner}",
                    meta.uid()
                )));
            }
            if meta.mode() & 0o022 != 0 {
                return Err(refuse(format!(
                    "is writable by group or others (mode {:o})",
                    meta.mode() & 0o7777
                )));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            private(&root)
                .map_err(|e| refuse(format!("cannot be created (its parent must exist): {e}")))?;
        }
        Err(e) => return Err(refuse(e.to_string())),
    }
    // The launch-time recheck, on the root as it now exists, before
    // anything is created inside it.
    profile
        .check_scratch_disjoint()
        .map_err(|e| ProviderError::ProfileInvalid(e.to_string()))?;
    let run = root.join(format!(
        "w-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    private(&run).map_err(|e| refuse(format!("cannot create the run directory: {e}")))?;
    if let Err(e) = private(&run.join("tmp")) {
        let _ = std::fs::remove_dir_all(&run);
        return Err(refuse(format!(
            "cannot create the run's tmp directory: {e}"
        )));
    }
    // Sandbox grants name physical paths, so the worker gets the canonical one.
    Ok(std::fs::canonicalize(&run).unwrap_or(run))
}

/// The child's pre-exec setup: own process group, soft and hard
/// `RLIMIT_NPROC=0`, the liveness read end inheritable and every other
/// inheritable descriptor beyond stdio closed. Descriptors with
/// `FD_CLOEXEC` close at exec by themselves and are left alone, so the
/// standard library's own exec-error pipe keeps reporting failures. Only
/// async-signal-safe calls run between fork and exec. Public so the
/// development isolation tests launch their probes under exactly this setup.
pub fn child_setup(read_fd: i32) -> impl FnMut() -> std::io::Result<()> + Send + Sync + 'static {
    move || {
        if unsafe { libc::setpgid(0, 0) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if unsafe { libc::setrlimit(libc::RLIMIT_NPROC, &limit) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        if unsafe { libc::fcntl(read_fd, libc::F_SETFD, 0) } == -1 {
            return Err(std::io::Error::last_os_error());
        }
        let table = unsafe { libc::getdtablesize() };
        for fd in 3..table {
            if fd == read_fd {
                continue;
            }
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
            if flags != -1 && flags & libc::FD_CLOEXEC == 0 {
                unsafe { libc::close(fd) };
            }
        }
        Ok(())
    }
}

/// Physical footprint in bytes of `pid` (`proc_pid_rusage`, flavor V0), the
/// figure Activity Monitor reports as memory. `None` when the call fails.
fn physical_footprint(pid: u32) -> Option<u64> {
    let mut info = unsafe { std::mem::zeroed::<libc::rusage_info_v0>() };
    let rc = unsafe {
        libc::proc_pid_rusage(
            pid as libc::pid_t,
            libc::RUSAGE_INFO_V0,
            std::ptr::addr_of_mut!(info).cast(),
        )
    };
    (rc == 0).then_some(info.ri_phys_footprint)
}

enum LaunchEvent {
    Ready(Box<FunctionDescriptor>, u32),
    Failed(String),
}

enum Reply {
    Vectors(Vec<Vec<f32>>),
    Busy,
    Failed { code: String, message: String },
    Broken(String),
}

/// The capacity-one handoff for the reply to the current request. It stays
/// occupied until the waiting call consumes the reply or, for an abandoned
/// request, until the late reply arrives and is discarded.
enum Handoff {
    Idle,
    /// A caller waits for the reply and expects this many vectors.
    Waiting(mpsc::Sender<Reply>, usize),
    /// The caller gave up (a query past its deadline) while the request still
    /// runs in the worker, which holds its slot until the work ends.
    Abandoned,
}

struct Shared {
    pid: u32,
    current_id: AtomicU64,
    breach: AtomicBool,
    breach_info: Mutex<String>,
    broken: Mutex<Option<String>>,
    /// The handoff for the reply to the current request (see [`Handoff`]).
    handoff: Mutex<Handoff>,
    stderr_tail: Mutex<Vec<u8>>,
    stopped: AtomicBool,
    child_gone: AtomicBool,
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    liveness: Mutex<Option<OwnedFd>>,
}

impl Shared {
    fn stderr_excerpt(&self) -> String {
        let tail = match self.stderr_tail.lock() {
            Ok(tail) => tail,
            Err(poisoned) => poisoned.into_inner(),
        };
        format!(
            "worker stderr ({} of {} bytes retained): {}",
            tail.len(),
            protocol::MAX_STDERR_BYTES,
            String::from_utf8_lossy(&tail)
                .chars()
                .take(4000)
                .collect::<String>()
        )
    }

    fn set_broken(&self, message: String) {
        if let Ok(mut slot) = self.broken.lock()
            && slot.is_none()
        {
            *slot = Some(message.clone());
        }
        self.notify(Reply::Broken(message));
    }

    fn handoff(&self) -> std::sync::MutexGuard<'_, Handoff> {
        match self.handoff.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Deliver the reply for the current request. A waiting caller receives
    /// it; the late reply of an abandoned request is consumed and discarded,
    /// releasing the handoff; with no request in flight it is ignored.
    fn notify(&self, reply: Reply) {
        let previous = std::mem::replace(&mut *self.handoff(), Handoff::Idle);
        if let Handoff::Waiting(tx, _) = previous {
            let _ = tx.send(reply);
        }
    }

    /// How many vectors the waiting caller expects, if one waits.
    fn expected_count(&self) -> Option<usize> {
        match &*self.handoff() {
            Handoff::Waiting(_, count) => Some(*count),
            _ => None,
        }
    }

    /// True while an abandoned request's reply is still outstanding.
    fn abandoned(&self) -> bool {
        matches!(&*self.handoff(), Handoff::Abandoned)
    }

    /// The caller gave up on a waiting request that is still running in the
    /// worker: keep the handoff occupied until its late reply is discarded.
    fn abandon(&self) {
        let mut state = self.handoff();
        if matches!(&*state, Handoff::Waiting(..)) {
            *state = Handoff::Abandoned;
        }
    }

    /// Run `f` with the child's PID only while the process is unreaped, so
    /// the PID cannot have been reused: PID names alone are not ownership.
    /// `None` once the child was reaped or never existed.
    fn with_live_pid<T>(&self, f: impl FnOnce(u32) -> T) -> Option<T> {
        let mut guard = match self.child.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let child = guard.as_mut()?;
        match child.try_wait() {
            Ok(None) => Some(f(child.id())),
            _ => None,
        }
    }

    /// Signal only the verified, still-unreaped child; never a broad kill.
    fn signal_child(&self, signal: i32) {
        self.with_live_pid(|pid| unsafe {
            libc::kill(pid as libc::pid_t, signal);
        });
    }
}

/// Who waits on a call and when it stops waiting: a query carries its own
/// deadline, a document call its budget [`Control`] (deadline and
/// cancellation).
#[derive(Clone, Copy)]
struct Caller<'a> {
    deadline: Option<Instant>,
    control: Option<&'a Control>,
}

impl Caller<'_> {
    /// Why the caller no longer waits, if it does not: its control's
    /// deadline (`Timeout`) or cancellation (`Cancelled`), or the query
    /// deadline (`Timeout`); `None` while it waits.
    fn stop(&self) -> Option<ProviderError> {
        if let Some(control) = self.control
            && let Some(stop) = control_stop(control)
        {
            return Some(stop);
        }
        if self.deadline.is_some_and(|d| Instant::now() >= d) {
            return Some(ProviderError::Timeout);
        }
        None
    }

    /// Time left before the query deadline; `None` without one.
    fn remaining(&self) -> Option<Duration> {
        self.deadline
            .map(|d| d.saturating_duration_since(Instant::now()))
    }
}

/// The supervised worker behind one admission slot.
pub struct WorkerProvider {
    shared: Arc<Shared>,
    descriptor: Arc<FunctionDescriptor>,
    /// Vocabulary bound the worker reported at readiness.
    pub vocab: u32,
    /// The per-run scratch directory, removed at shutdown.
    scratch: Option<std::path::PathBuf>,
    handles: Vec<Option<std::thread::JoinHandle<()>>>,
    /// Receive slice and busy-retry step; tests shorten or stretch them to
    /// make a timing-dependent path deterministic.
    wait_slice: Duration,
    busy_retry_step: Duration,
}

impl WorkerProvider {
    /// [`Self::launch_until`] with no caller bound; tests only.
    #[cfg(feature = "test-faults")]
    pub fn launch(
        profile: &SemanticProfile,
        extra_args: Vec<String>,
    ) -> Result<Self, ProviderError> {
        Self::launch_until(profile, extra_args, &Control::unbounded())
    }

    /// Launch whatever executable the profile's bundle holds, with
    /// `extra_args` appended to the worker's argv. Tests only: the test
    /// bundle's `foundry-embed-fake` arms its fault hooks from them, while
    /// production reaches the same code through [`acquire_until`] with no
    /// extra arguments.
    #[cfg(feature = "test-faults")]
    pub fn launch_until(
        profile: &SemanticProfile,
        extra_args: Vec<String>,
        control: &Control,
    ) -> Result<Self, ProviderError> {
        Self::launch_with(profile, &extra_args, control)
    }

    /// Verify the profile, every artifact, the requirements listing and the
    /// bundle executable hash BEFORE launch, then start the worker and wait
    /// for a `ready` whose descriptor equals the profile's.
    ///
    /// The whole acquisition (verification, spawn, hello, model load) runs
    /// under ONE absolute bound fixed here: the earlier of the caller's
    /// control deadline and now + the profile's `load_timeout_seconds`, with
    /// the caller's cancellation shared. Every stage checks it, including the
    /// streamed hashing.
    fn launch_with(
        profile: &SemanticProfile,
        extra_args: &[String],
        control: &Control,
    ) -> Result<Self, ProviderError> {
        let load_bound =
            Instant::now().checked_add(Duration::from_secs(profile.load_timeout_seconds));

        let Some(load_bound) = load_bound else {
            // Unrepresentable even after validation clamps the profile's
            // value: refuse rather than wrap.
            return Err(ProviderError::ProfileInvalid(
                "the profile's load timeout is not representable as a deadline".into(),
            ));
        };
        let bound = control.bounded_by(load_bound);
        // The receive cap comes from the BOUNDED control: the earlier of the
        // caller's own deadline and the load bound, never the load bound
        // alone, so a caller's earlier deadline wakes the receive at that
        // deadline.
        let bound_deadline = bound.deadline();
        let launch_slice = launch_wait_slice();
        let control = &bound;
        if let Some(stop) = control_stop(control) {
            return Err(stop);
        }
        profile.validate()?;
        worker_runtime::check_real_descriptor(&profile.descriptor)
            .map_err(ProviderError::ProfileInvalid)?;
        profile.verify_artifacts_until(control)?;
        let executable = profile
            .worker
            .bundle
            .join("Contents/MacOS")
            .join("foundry-embed");
        let (sha, _) = hash_regular_file_until(&executable, control)?;
        if sha != profile.worker.executable_sha256 {
            return Err(ProviderError::ProfileInvalid(format!(
                "worker executable {} has SHA-256 {sha}, profile expects {}",
                executable.display(),
                profile.worker.executable_sha256
            )));
        }
        // Nothing is spawned for a caller that stopped meanwhile.
        if let Some(stop) = control_stop(control) {
            return Err(stop);
        }

        // The worker's private scratch is a per-run directory under exactly
        // the profile's scratch root, which the signed bundle grants.
        let scratch_dir = prepare_run_scratch(profile)?;
        let (liveness_reader, liveness_writer) = std::io::pipe().map_err(|e| {
            let _ = std::fs::remove_dir_all(&scratch_dir);
            ProviderError::IsolationUnavailable(format!("liveness pipe: {e}"))
        })?;
        let read_fd = liveness_reader.as_raw_fd();
        let descriptor_json = serde_json::to_string(&profile.descriptor)
            .map_err(|e| ProviderError::ProfileInvalid(format!("descriptor JSON: {e}")))?;

        let mut command = Command::new(&executable);
        command
            .arg("--owner-pid")
            .arg(std::process::id().to_string())
            .arg("--liveness-fd")
            .arg(read_fd.to_string())
            .arg("--descriptor")
            .arg(&descriptor_json)
            .arg("--model-dir")
            .arg(&profile.model_dir)
            .arg("--python-home")
            .arg(&profile.runtime.python_home)
            .arg("--site-packages")
            .arg(&profile.runtime.site_packages);
        command.args(extra_args);
        command
            .current_dir(&scratch_dir)
            // `env -i` style: no inherited credentials, proxies or agent
            // sockets reach the worker.
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &scratch_dir)
            .env("TMPDIR", scratch_dir.join("tmp"))
            .env("PYTHONHOME", &profile.runtime.python_home)
            .env("PYTHONNOUSERSITE", "1")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .env("PYTHONUNBUFFERED", "1")
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("TOKENIZERS_PARALLELISM", "false")
            .env("OMP_NUM_THREADS", "2")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: `child_setup` calls only async-signal-safe functions
        // between fork and exec and takes no lock or allocation.
        unsafe { command.pre_exec(child_setup(read_fd)) };
        let mut child = command.spawn().map_err(|e| {
            let _ = std::fs::remove_dir_all(&scratch_dir);
            ProviderError::ProfileInvalid(format!("launch {}: {e}", executable.display()))
        })?;
        // The parent keeps only the write end; the read end belongs to the
        // child now.
        drop(liveness_reader);
        let pid = child.id();
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let shared = Arc::new(Shared {
            pid,
            current_id: AtomicU64::new(0),
            breach: AtomicBool::new(false),
            breach_info: Mutex::new(String::new()),
            broken: Mutex::new(None),
            handoff: Mutex::new(Handoff::Idle),
            stderr_tail: Mutex::new(Vec::new()),
            stopped: AtomicBool::new(false),
            child_gone: AtomicBool::new(false),
            child: Mutex::new(Some(child)),
            stdin: Mutex::new(stdin),
            liveness: Mutex::new(Some(OwnedFd::from(liveness_writer))),
        });

        // `hello` is the supervisor's first frame and `ready` answers it. A
        // worker that already died is reported through its closed stdout.
        let hello_sent = match shared.stdin.lock() {
            Ok(mut guard) => guard.as_mut().is_some_and(|stdin| {
                protocol::write_frame(
                    stdin,
                    &Header::Hello {
                        protocol: protocol::PROTOCOL_VERSION,
                    },
                    &[],
                )
                .is_ok()
            }),
            Err(_) => false,
        };
        if !hello_sent {
            shared.set_broken("the hello frame could not be written".into());
        }

        let mut handles = Vec::new();
        // Stderr is drained continuously into a bounded buffer.
        if let Some(stderr) = stderr {
            let shared = Arc::clone(&shared);
            handles.push(Some(std::thread::spawn(move || {
                let mut reader = BufReader::new(stderr);
                let mut chunk = [0u8; 4096];
                loop {
                    match reader.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let mut tail = match shared.stderr_tail.lock() {
                                Ok(tail) => tail,
                                Err(poisoned) => poisoned.into_inner(),
                            };
                            let room = protocol::MAX_STDERR_BYTES.saturating_sub(tail.len());
                            tail.extend_from_slice(&chunk[..n.min(room)]);
                        }
                    }
                }
            })));
        }
        // Memory: poll the physical footprint against the ceiling.
        {
            let shared = Arc::clone(&shared);
            let ceiling = profile.memory_ceiling_bytes;
            handles.push(Some(std::thread::spawn(move || {
                loop {
                    std::thread::sleep(MEMORY_POLL_INTERVAL);
                    if shared.stopped.load(Ordering::SeqCst) {
                        break;
                    }
                    // Measure only while the child is unreaped, so a reused
                    // PID is never read.
                    let Some(measured) = shared.with_live_pid(physical_footprint) else {
                        break;
                    };
                    let Some(footprint) = measured else {
                        continue;
                    };
                    if footprint > ceiling {
                        let detail = format!(
                            "physical footprint {footprint} bytes exceeds the {ceiling} byte ceiling"
                        );
                        if let Ok(mut slot) = shared.breach_info.lock() {
                            *slot = detail.clone();
                        }
                        shared.breach.store(true, Ordering::SeqCst);
                        shared.signal_child(libc::SIGKILL);
                        shared.notify(Reply::Broken(detail));
                        break;
                    }
                }
            })));
        }
        // Reply reader: validates every frame and routes by request ID.
        let (launch_tx, launch_rx) = mpsc::channel::<LaunchEvent>();
        if let Some(stdout) = stdout {
            let shared = Arc::clone(&shared);
            let launch_tx = launch_tx.clone();
            handles.push(Some(std::thread::spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    match protocol::read_frame(&mut reader) {
                        Ok((
                            Header::Ready {
                                descriptor,
                                vocab_size,
                                ..
                            },
                            _,
                        )) => {
                            if launch_tx
                                .send(LaunchEvent::Ready(descriptor, vocab_size))
                                .is_err()
                            {
                                // Already launched; an unsolicited repeat breaks the stream.
                                shared.set_broken("unsolicited ready frame".into());
                                break;
                            }
                        }
                        Ok((
                            Header::Error {
                                id: None,
                                code,
                                message,
                                ..
                            },
                            _,
                        )) => {
                            if launch_tx
                                .send(LaunchEvent::Failed(format!("{code}: {message}")))
                                .is_err()
                            {
                                shared.set_broken(format!("worker reported {code}: {message}"));
                            }
                        }
                        Ok((
                            Header::Vectors {
                                id, count, dims, ..
                            },
                            payload,
                        )) => {
                            let current = shared.current_id.load(Ordering::SeqCst);
                            if id != current {
                                continue; // stale: discarded
                            }
                            match shared.expected_count() {
                                Some(expected) => {
                                    match protocol::decode_vectors(count, dims, &payload, expected)
                                    {
                                        Ok(vectors) => shared.notify(Reply::Vectors(vectors)),
                                        Err(e) => shared.notify(Reply::Failed {
                                            code: "provider_malformed".into(),
                                            message: e.to_string(),
                                        }),
                                    }
                                }
                                // Nobody waits: an abandoned request's late
                                // reply is consumed and discarded, which
                                // releases the handoff.
                                None => shared.notify(Reply::Busy),
                            }
                        }
                        Ok((Header::Busy { id, .. }, _)) => {
                            if id == shared.current_id.load(Ordering::SeqCst) {
                                shared.notify(Reply::Busy);
                            }
                        }
                        Ok((
                            Header::Error {
                                id: Some(id),
                                code,
                                message,
                                ..
                            },
                            _,
                        )) => {
                            if id == shared.current_id.load(Ordering::SeqCst) {
                                shared.notify(Reply::Failed { code, message });
                            }
                        }
                        Ok((other, _)) => {
                            shared.set_broken(format!(
                                "unexpected {} frame from the worker",
                                protocol_kind(&other)
                            ));
                            break;
                        }
                        Err(protocol::FrameError::Eof) => {
                            shared.set_broken("worker stdout ended".into());
                            let _ = launch_tx.send(LaunchEvent::Failed("worker exited".into()));
                            break;
                        }
                        Err(e) => {
                            shared.set_broken(format!("worker IPC: {e}"));
                            let _ = launch_tx.send(LaunchEvent::Failed(format!("worker IPC: {e}")));
                            break;
                        }
                    }
                }
            })));
        }
        drop(launch_tx);

        // Wait for readiness under the acquisition's one bound.
        let mut stopped: Option<ProviderError> = None;
        let event = loop {
            // One bound for the whole acquisition: the caller's control and
            // the profile's load timeout, fixed at entry.
            if let Some(stop) = control_stop(control) {
                stopped = Some(stop);
                break None;
            }
            let slice = bound_deadline
                .and_then(|at| at.checked_duration_since(Instant::now()))
                .map_or(launch_slice, |left| left.min(launch_slice));
            match launch_rx.recv_timeout(slice) {
                Ok(event) => break Some(event),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if shared.breach.load(Ordering::SeqCst) {
                        break Some(LaunchEvent::Failed(
                            "the worker breached the memory ceiling while loading".into(),
                        ));
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break Some(LaunchEvent::Failed(
                        "the worker's output ended before it was ready".into(),
                    ));
                }
            }
        };
        let (descriptor, vocab) = match event {
            // A `ready` that crossed the acquisition's stop inside one receive
            // slice is discarded, not accepted: the worker is shut down and
            // reaped, and the caller's stop reason stands.
            Some(LaunchEvent::Ready(descriptor, vocab)) => {
                if let Some(stop) = control_stop(control) {
                    let mut provider =
                        Self::assemble(shared, &profile.descriptor, 0, scratch_dir, handles);
                    provider.shutdown();
                    return Err(stop);
                }
                (*descriptor, vocab)
            }
            Some(LaunchEvent::Failed(message)) => {
                let mut provider =
                    Self::assemble(shared, &profile.descriptor, 0, scratch_dir, handles);
                provider.shutdown();
                if let Some(breach) = provider.breach_message() {
                    return Err(ProviderError::ResourceLimit(breach));
                }
                return Err(ProviderError::WorkerExited(format!(
                    "{message}; {}",
                    provider.shared.stderr_excerpt()
                )));
            }
            None => {
                let mut provider =
                    Self::assemble(shared, &profile.descriptor, 0, scratch_dir, handles);
                provider.shutdown();
                if let Some(breach) = provider.breach_message() {
                    return Err(ProviderError::ResourceLimit(breach));
                }
                return Err(stopped.unwrap_or(ProviderError::Timeout));
            }
        };
        if descriptor != profile.descriptor {
            let mut provider = Self::assemble(shared, &profile.descriptor, 0, scratch_dir, handles);
            provider.shutdown();
            return Err(ProviderError::ProfileInvalid(
                "the worker reported a different function descriptor than the profile".into(),
            ));
        }
        if vocab == 0 {
            let mut provider = Self::assemble(shared, &profile.descriptor, 0, scratch_dir, handles);
            provider.shutdown();
            return Err(ProviderError::Malformed(
                "the worker reported a zero vocabulary bound".into(),
            ));
        }
        // The full control again before the provider is handed out: a stop
        // that landed while the reply checks ran never yields a worker.
        if let Some(stop) = control_stop(control) {
            let mut provider = Self::assemble(shared, &profile.descriptor, 0, scratch_dir, handles);
            provider.shutdown();
            return Err(stop);
        }
        Ok(Self::assemble(
            shared,
            &profile.descriptor,
            vocab,
            scratch_dir,
            handles,
        ))
    }

    fn assemble(
        shared: Arc<Shared>,
        descriptor: &FunctionDescriptor,
        vocab: u32,
        scratch: std::path::PathBuf,
        handles: Vec<Option<std::thread::JoinHandle<()>>>,
    ) -> Self {
        Self {
            shared,
            descriptor: Arc::new(descriptor.clone()),
            vocab,
            scratch: Some(scratch),
            handles,
            wait_slice: WAIT_SLICE,
            busy_retry_step: BUSY_RETRY_STEP,
        }
    }

    /// The verified child PID while the worker lives.
    pub fn worker_pid(&self) -> Option<u32> {
        (!self.shared.child_gone.load(Ordering::SeqCst)).then_some(self.shared.pid)
    }

    /// True only after shutdown completed and the process was reaped.
    pub fn stopped(&self) -> bool {
        self.shared.stopped.load(Ordering::SeqCst) && self.shared.child_gone.load(Ordering::SeqCst)
    }

    /// Retained worker stderr, bounded by [`protocol::MAX_STDERR_BYTES`].
    pub fn stderr_excerpt(&self) -> String {
        self.shared.stderr_excerpt()
    }

    /// Tests only: set the receive slice and the busy-retry step, so a
    /// timing-dependent path (a reply crossing a deadline inside one slice,
    /// a retry sleep outlasting the budget) is deterministic, not a race.
    #[cfg(feature = "test-faults")]
    pub fn set_timing(&mut self, wait_slice: Duration, busy_retry_step: Duration) {
        self.wait_slice = wait_slice;
        self.busy_retry_step = busy_retry_step;
    }

    fn breach_message(&self) -> Option<String> {
        (self.shared.breach.load(Ordering::SeqCst)).then(|| {
            let info = match self.shared.breach_info.lock() {
                Ok(info) => info.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            };
            format!("memory ceiling breach: {info}")
        })
    }

    fn broken_reason(&self) -> Option<String> {
        match self.shared.broken.lock() {
            Ok(slot) => slot.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// The worker ended or broke the protocol: discard the handoff, stop and
    /// reap it, then name the cause. A memory breach wins over the symptom
    /// it produced.
    fn broken_error(&mut self, message: String) -> ProviderError {
        self.discard_handoff();
        self.shutdown();
        if let Some(breach) = self.breach_message() {
            return ProviderError::ResourceLimit(breach);
        }
        ProviderError::WorkerExited(format!("{message}; {}", self.shared.stderr_excerpt()))
    }

    /// One embed call with the supervisor's benign-race rule: the supervisor
    /// is single-threaded behind `&mut self`, so every earlier request is
    /// final when a new one starts, and a `busy` answer can only be the
    /// worker still releasing the slot after the reply it just wrote. That
    /// is retried for a short bounded window; the worker still never queues.
    ///
    /// The caller's whole stop condition (deadline AND cancellation) is
    /// checked before every dispatch, so no request is sent after the
    /// caller gave up, and again when a reply is consumed.
    fn request(
        &mut self,
        purpose: Purpose,
        inputs: &[TokenizedInput],
        caller: Caller<'_>,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        if let Some(stop) = caller.stop() {
            return Err(stop);
        }
        // An abandoned query's request is still running in the worker, which
        // holds its slot until the work ends: nothing is sent, and the call
        // is busy until the late reply is discarded.
        if self.shared.abandoned() {
            return Err(ProviderError::Busy);
        }
        let started = Instant::now();
        loop {
            match self.request_once(purpose, inputs, caller) {
                Err(ProviderError::Busy) => {
                    if let Some(stop) = caller.stop() {
                        return Err(stop);
                    }
                    if started.elapsed() >= BUSY_RETRY_WINDOW {
                        return Err(ProviderError::Busy);
                    }
                    std::thread::sleep(self.busy_retry_step);
                }
                other => return other,
            }
        }
    }

    fn request_once(
        &mut self,
        purpose: Purpose,
        inputs: &[TokenizedInput],
        caller: Caller<'_>,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        if let Some(breach) = self.breach_message() {
            self.shutdown();
            return Err(ProviderError::ResourceLimit(breach));
        }
        if let Some(broken) = self.broken_reason() {
            return Err(self.broken_error(broken));
        }
        // Nothing is dispatched for a caller that stopped waiting, whether
        // this is the first attempt or a retry after a sleep.
        if let Some(stop) = caller.stop() {
            return Err(stop);
        }
        let id = self.shared.current_id.fetch_add(1, Ordering::SeqCst) + 1;
        let (tx, rx) = mpsc::channel::<Reply>();
        *self.shared.handoff() = Handoff::Waiting(tx, inputs.len());
        let (lengths, payload) = protocol::encode_ids(inputs);
        let header = Header::Embed {
            protocol: protocol::PROTOCOL_VERSION,
            id,
            descriptor_digest: self.descriptor.digest(),
            purpose,
            lengths,
        };
        let written = match self.shared.stdin.lock() {
            Ok(mut stdin) => match stdin.as_mut() {
                Some(stdin) => protocol::write_frame(stdin, &header, &payload).is_ok(),
                None => false,
            },
            Err(poisoned) => match poisoned.into_inner().as_mut() {
                Some(stdin) => protocol::write_frame(stdin, &header, &payload).is_ok(),
                None => false,
            },
        };
        if !written {
            let detail = self
                .broken_reason()
                .unwrap_or_else(|| "the embed request could not be written".into());
            return Err(self.broken_error(detail));
        }
        loop {
            // The top of every iteration re-checks the caller, so the stop
            // is noticed within one receive slice (a query's slice never
            // overshoots its deadline).
            if let Some(stop) = caller.stop() {
                return match purpose {
                    Purpose::Document => self.in_flight_grace(&rx, stop),
                    Purpose::Query => self.abandon_query(&rx, stop),
                };
            }
            let slice = caller
                .remaining()
                .map_or(self.wait_slice, |left| left.min(self.wait_slice));
            match rx.recv_timeout(slice) {
                Ok(reply) => {
                    self.discard_handoff();
                    return match reply {
                        // A reply that crossed the caller's stop within the
                        // receive slice is discarded with the caller's stop
                        // reason, never returned as success.
                        Reply::Vectors(vectors) => match caller.stop() {
                            Some(stop) => Err(stop),
                            None => Ok(vectors),
                        },
                        Reply::Busy => Err(ProviderError::Busy),
                        Reply::Failed { code, message } => {
                            Err(ProviderError::Malformed(format!("{code}: {message}")))
                        }
                        Reply::Broken(message) => Err(self.broken_error(message)),
                    };
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let message = self
                        .broken_reason()
                        .unwrap_or_else(|| "the worker reply channel closed".into());
                    return Err(self.broken_error(message));
                }
            }
        }
    }

    /// A document call's caller gave up (budget expiry or cancellation)
    /// while its request is still in flight. The worker's slot belongs to
    /// the real work: wait at most [`IN_FLIGHT_GRACE`] for the reply, then
    /// stop the worker (TERM/5 s/KILL/reap) and time out. Nothing is
    /// replaced before the process is gone. Queries never come here; they
    /// obey their deadline (see `abandon_query`).
    fn in_flight_grace(
        &mut self,
        rx: &mpsc::Receiver<Reply>,
        caller_error: ProviderError,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        match rx.recv_timeout(IN_FLIGHT_GRACE) {
            Ok(Reply::Broken(message)) => Err(self.broken_error(message)),
            // The real work ended inside the grace; the caller still gave
            // up first, so its error stands and the reply is discarded.
            Ok(_) => {
                self.discard_handoff();
                Err(caller_error)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.discard_handoff();
                self.shutdown();
                Err(ProviderError::Timeout)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let message = self
                    .broken_reason()
                    .unwrap_or_else(|| "the worker reply channel closed".into());
                Err(self.broken_error(message))
            }
        }
    }

    /// A query's deadline passed with its request still in flight. Queries
    /// obey their deadline: `Timeout` now, without waiting and without
    /// stopping the worker. The request stays pending in the handoff, so
    /// every later call is `Busy` until its late reply arrives and is
    /// discarded.
    fn abandon_query(
        &mut self,
        rx: &mpsc::Receiver<Reply>,
        stop: ProviderError,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        match rx.try_recv() {
            Ok(Reply::Broken(message)) => Err(self.broken_error(message)),
            // The reply landed at the deadline: the handoff already
            // released, but the deadline still stands.
            Ok(_) => Err(stop),
            Err(_) => {
                self.shared.abandon();
                Err(stop)
            }
        }
    }

    /// Give up on the current reply: the capacity-one handoff is discarded,
    /// so a late frame is dropped by request ID and never blocks the reader.
    fn discard_handoff(&self) {
        *self.shared.handoff() = Handoff::Idle;
    }

    /// Close stdin, TERM, wait 5 s, KILL, reap. Reports stopped only after
    /// the process is gone; no replacement may start before that.
    pub fn shutdown(&mut self) {
        if self.shared.stopped.load(Ordering::SeqCst) {
            return;
        }
        // EOF first: a compliant worker exits on its own.
        if let Ok(mut slot) = self.shared.stdin.lock() {
            *slot = None;
        }
        self.shared.signal_child(libc::SIGTERM);
        let term_at = Instant::now();
        let mut reaped = None;
        while Instant::now().duration_since(term_at) < TERM_GRACE {
            let mut guard = match self.shared.child.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            match guard.as_mut() {
                Some(child) => match child.try_wait() {
                    Ok(Some(status)) => {
                        reaped = Some(status);
                        *guard = None;
                        break;
                    }
                    Ok(None) => {}
                    Err(_) => {
                        reaped = None;
                        *guard = None;
                        break;
                    }
                },
                None => break,
            }
            drop(guard);
            std::thread::sleep(Duration::from_millis(50));
        }
        if reaped.is_none() {
            self.shared.signal_child(libc::SIGKILL);
            let mut guard = match self.shared.child.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Some(child) = guard.as_mut() {
                let _ = child.wait();
                *guard = None;
            }
        }
        // The liveness write end survives until the worker is reaped.
        if let Ok(mut slot) = self.shared.liveness.lock() {
            *slot = None;
        }
        self.shared.child_gone.store(true, Ordering::SeqCst);
        self.shared.stopped.store(true, Ordering::SeqCst);
        for handle in self.handles.iter_mut() {
            if let Some(handle) = handle.take() {
                let _ = handle.join();
            }
        }
        // The per-run scratch directory goes only after the process is
        // gone; the root itself stays for the next run.
        if let Some(scratch) = self.scratch.take() {
            let _ = std::fs::remove_dir_all(scratch);
        }
    }
}

impl Drop for WorkerProvider {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl EmbeddingProvider for WorkerProvider {
    fn descriptor(&self) -> &FunctionDescriptor {
        &self.descriptor
    }

    fn embed_documents(
        &mut self,
        batch: &[TokenizedInput],
        control: &Control,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        check_document_batch(batch)?;
        self.request(
            Purpose::Document,
            batch,
            Caller {
                deadline: None,
                control: Some(control),
            },
        )
    }

    fn embed_query(
        &mut self,
        input: &TokenizedInput,
        deadline: Instant,
    ) -> Result<Vec<f32>, ProviderError> {
        check_query(input)?;
        let mut vectors = self.request(
            Purpose::Query,
            std::slice::from_ref(input),
            Caller {
                deadline: Some(deadline),
                control: None,
            },
        )?;
        if vectors.len() != 1 {
            return Err(ProviderError::Malformed(format!(
                "{} vectors for one query",
                vectors.len()
            )));
        }
        Ok(vectors.remove(0))
    }
}

fn protocol_kind(header: &Header) -> &'static str {
    match header {
        Header::Hello { .. } => "hello",
        Header::Ready { .. } => "ready",
        Header::Embed { .. } => "embed",
        Header::Vectors { .. } => "vectors",
        Header::Busy { .. } => "busy",
        Header::Error { .. } => "error",
    }
}
