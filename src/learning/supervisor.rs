//! 013 T002 owner side of the learning worker (macOS): verify the profile
//! and the bundle executable before launch, start the worker under the
//! development isolation profile, police its wall clock, memory and output,
//! correlate every reply, and stop it with EOF → TERM → 5 s → KILL, reaping
//! before anything is reported.
//!
//! Reused from 009 rather than copied: the scratch-root rules
//! ([`crate::neural::supervisor::prepare_run_scratch_at`]), the pre-exec
//! child setup ([`crate::neural::supervisor::child_setup_limited`]: own
//! process group, extra hard limits, `RLIMIT_NPROC=0`, inheritable
//! descriptors closed), the footprint measurement and its fault point, the
//! TERM/KILL/reap sequence and the frame format.
//!
//! Enforcement (the matrix in [`super::profile::PROVIDED`]):
//! * `RLIMIT_NPROC=0`, `RLIMIT_CPU = wall_seconds × cpu_threads` and
//!   `RLIMIT_FSIZE = output_bytes` are set before exec, soft = hard;
//! * a monitor thread polls the worker's physical footprint and the total
//!   bytes of its scratch run directory every 250 ms. A breach, or a
//!   footprint that cannot be measured while the worker lives, kills the
//!   worker with a named reason: measurement fails CLOSED.
//!
//! Every reply must carry the request's ID and the identity the owner
//! holds; a `logits`/`step`/`save` reply must also echo the input digest.
//! Anything else — another ID, a duplicate, an unsolicited frame, an
//! identity mismatch, a worker `error` — stops the worker; its output is
//! never consumed.
use super::ipc::{self, HeadSlot, Identity, LEARN_PROTOCOL, LearnHeader, Message, ParameterCounts};
use super::profile::LearnProfile;
use super::{LearningPolicy, fail};
use crate::control::Control;
use crate::error::{FResult, FoundryError};
use crate::neural::anchor::Dir;
use crate::neural::profile::hash_regular_file_until;
use crate::neural::protocol::{self, FrameError, read_frame_as, write_frame_as};
use crate::neural::provider::ProviderError;
use crate::neural::supervisor::{
    child_setup_limited, physical_footprint, prepare_run_scratch_at, terminate_and_reap,
};
use std::io::{BufReader, Read as _};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const WAIT_SLICE: Duration = Duration::from_millis(50);
/// How long a worker whose run completed gets to exit on owner EOF before
/// TERM/KILL (which then fails the completion).
const EXIT_GRACE: Duration = Duration::from_secs(5);
/// The reader's last frame when the worker's output ended cleanly.
const END_OF_OUTPUT: &str = "the worker's output ended";

#[cfg(feature = "test-faults")]
thread_local! {
    static TEST_WORKER_ARGS: std::cell::RefCell<Vec<String>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Tests only: extra argv for workers this thread launches (the fake
/// worker's fault hooks). Production launches never carry any.
#[cfg(feature = "test-faults")]
pub fn set_test_worker_args(args: Vec<String>) {
    TEST_WORKER_ARGS.with(|slot| *slot.borrow_mut() = args);
}

fn test_worker_args() -> Vec<String> {
    #[cfg(feature = "test-faults")]
    {
        TEST_WORKER_ARGS.with(|slot| slot.borrow().clone())
    }
    #[cfg(not(feature = "test-faults"))]
    {
        Vec::new()
    }
}

/// A stop the monitor imposed: the named code wins over the symptom.
pub(super) type Breach = Arc<Mutex<Option<(&'static str, String)>>>;

enum Frame {
    Header(Box<LearnHeader>, Vec<u8>),
    End(String),
}

/// The supervised ceilings, measured by the monitor every poll and once more
/// at completion: the same checks and the same fault point.
#[derive(Clone)]
struct Limits {
    memory: u64,
    output: u64,
    run: PathBuf,
    staged: Arc<std::sync::atomic::AtomicU64>,
    fault_detail: String,
}

impl Limits {
    /// The named stop for live worker `pid`, if any. A footprint or scratch
    /// total that cannot be read fails closed.
    fn measure(&self, pid: u32) -> Option<(&'static str, String)> {
        let (memory, output) = (self.memory, self.output);
        match physical_footprint(pid, &self.fault_detail) {
            Err(why) => Some((
                "memory_unmeasurable",
                format!(
                    "the live worker's physical footprint could not be measured ({why}); the \
                     {memory}-byte ceiling cannot be enforced"
                ),
            )),
            Ok(footprint) if footprint > memory => Some((
                "memory_limit",
                format!("physical footprint {footprint} bytes exceeds the {memory}-byte ceiling"),
            )),
            Ok(_) => match tree_bytes(&self.run) {
                Ok(bytes) if bytes.saturating_sub(self.staged.load(Ordering::SeqCst)) > output => {
                    Some((
                        "output_limit",
                        format!(
                            "the scratch run directory holds {bytes} bytes; the output ceiling \
                             is {output}"
                        ),
                    ))
                }
                Ok(_) => None,
                Err(e) => Some((
                    "output_limit",
                    format!(
                        "the scratch run directory cannot be measured ({e}); the output \
                         ceiling cannot be enforced"
                    ),
                )),
            },
        }
    }
}

/// What a `load` verified, as the owner checked it.
pub struct Loaded {
    pub frozen_encoder_sha256: String,
    pub counts: ParameterCounts,
}

/// What a `save` reported.
pub struct Saved {
    pub sha256: String,
    pub bytes: u64,
    pub values: [f32; 2],
}

/// The supervised worker.
pub struct LearnWorker {
    child: Arc<Mutex<Option<Child>>>,
    pid: u32,
    stdin: Option<ChildStdin>,
    frames: mpsc::Receiver<Frame>,
    stderr_tail: Arc<Mutex<Vec<u8>>>,
    breach: Breach,
    stopping: Arc<AtomicBool>,
    /// Bytes the core itself placed in the run directory (starting heads):
    /// inputs, not worker output, so the supervised total excludes them.
    staged: Arc<std::sync::atomic::AtomicU64>,
    limits: Limits,
    handles: Vec<std::thread::JoinHandle<()>>,
    liveness: Option<OwnedFd>,
    scratch_path: Option<PathBuf>,
    scratch: Dir,
    next_id: u64,
    identity: Identity,
    wall_deadline: Instant,
    load_timeout: Duration,
    status: Option<std::process::ExitStatus>,
    stopped: bool,
}

fn provider_error(error: ProviderError) -> FoundryError {
    match error {
        ProviderError::IsolationUnavailable(m) => fail("isolation_unavailable", m),
        ProviderError::Timeout => FoundryError::DeadlineExceeded(None),
        ProviderError::Cancelled => FoundryError::Cancelled(None),
        other => fail("profile_invalid", other.to_string()),
    }
}

/// Total bytes of regular files under `dir`, never following a link.
fn tree_bytes(dir: &std::path::Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next)? {
            let entry = entry?;
            let meta = std::fs::symlink_metadata(entry.path())?;
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

/// A worker `error` code as the owner names it.
pub(super) fn worker_code(code: &str) -> &'static str {
    const KNOWN: [&str; 8] = [
        "nonfinite_loss",
        "nonfinite_gradient",
        "nonfinite_logits",
        "nonfinite_weight",
        "checkpoint_invalid",
        "artifact_invalid",
        "output_write",
        "output_limit",
    ];
    KNOWN
        .iter()
        .find(|known| **known == code)
        .copied()
        .unwrap_or("worker_failed")
}

/// A launched, supervised worker before its first frame: the one launch
/// convention training ([`LearnWorker`]) and serving ([`super::serve`])
/// share. Its stderr drains into a bounded tail and ONE monitor polls
/// [`Limits::measure`] every [`POLL_INTERVAL`], killing on a breach.
pub(super) struct Spawned {
    pub child: Arc<Mutex<Option<Child>>>,
    pub pid: u32,
    pub stdin: Option<ChildStdin>,
    pub stdout: Option<std::process::ChildStdout>,
    pub stderr_tail: Arc<Mutex<Vec<u8>>>,
    pub breach: Breach,
    pub stopping: Arc<AtomicBool>,
    pub staged: Arc<std::sync::atomic::AtomicU64>,
    limits: Limits,
    pub handles: Vec<std::thread::JoinHandle<()>>,
    pub liveness: OwnedFd,
    pub run: PathBuf,
    pub scratch: Dir,
}

/// The resources one launch requests: LibTorch threads, the hard
/// `RLIMIT_CPU` (`None` for a serving worker, which lives as long as its
/// owner and is bounded per prediction instead), and the supervised memory
/// and output ceilings (output is also the hard `RLIMIT_FSIZE`).
pub(super) struct Request {
    pub threads: u32,
    pub cpu_seconds: Option<u64>,
    pub memory_bytes: u64,
    pub output_bytes: u64,
    /// The fake worker's test hooks; production launches pass none.
    pub extra_args: Vec<String>,
}

/// Verify the profile and the bundle executable, create the scratch run
/// directory, and start the worker with its stderr drain and monitor.
pub(super) fn spawn(
    profile: &LearnProfile,
    request: Request,
    control: &Control,
) -> FResult<Spawned> {
    control.check()?;
    profile.validate()?;
    let executable = profile.executable();
    let (sha, _) = hash_regular_file_until(&executable, control).map_err(provider_error)?;
    if sha != profile.worker.executable_sha256 {
        return Err(fail(
            "profile_invalid",
            format!(
                "worker executable {} has SHA-256 {sha}, the profile expects {}",
                executable.display(),
                profile.worker.executable_sha256
            ),
        ));
    }
    let run = prepare_run_scratch_at(&profile.worker.scratch_root, &|| {
        profile.check_scratch_disjoint()
    })
    .map_err(provider_error)?;
    let scratch = match Dir::open_path(&run) {
        Ok(dir) => dir,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&run);
            return Err(fail(
                "isolation_unavailable",
                format!("scratch run directory: {e}"),
            ));
        }
    };
    let (liveness_reader, liveness_writer) = match std::io::pipe() {
        Ok(pipe) => pipe,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&run);
            return Err(fail("isolation_unavailable", format!("liveness pipe: {e}")));
        }
    };
    let read_fd = liveness_reader.as_raw_fd();
    let mut command = Command::new(&executable);
    command
        .arg("--owner-pid")
        .arg(std::process::id().to_string())
        .arg("--liveness-fd")
        .arg(read_fd.to_string())
        .arg("--checkpoint-dir")
        .arg(&profile.checkpoint_dir)
        .args(request.extra_args)
        .current_dir(&run)
        // `env -i` style: no inherited credentials, proxies or sockets.
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", &run)
        .env("TMPDIR", run.join("tmp"))
        .env("OMP_NUM_THREADS", request.threads.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut hard = Vec::with_capacity(2);
    if let Some(cpu_seconds) = request.cpu_seconds {
        hard.push((libc::RLIMIT_CPU, cpu_seconds as libc::rlim_t));
    }
    hard.push((libc::RLIMIT_FSIZE, request.output_bytes as libc::rlim_t));
    // SAFETY: the setup calls only async-signal-safe functions between
    // fork and exec and allocates nothing there.
    unsafe { command.pre_exec(child_setup_limited(read_fd, hard)) };
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&run);
            return Err(fail(
                "profile_invalid",
                format!("launch {}: {e}", executable.display()),
            ));
        }
    };
    drop(liveness_reader);
    let pid = child.id();
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child = Arc::new(Mutex::new(Some(child)));
    let stderr_tail = Arc::new(Mutex::new(Vec::new()));
    let breach: Breach = Arc::new(Mutex::new(None));
    let stopping = Arc::new(AtomicBool::new(false));
    let staged = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let mut handles = Vec::new();
    if let Some(stderr) = stderr {
        let tail = Arc::clone(&stderr_tail);
        handles.push(std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut chunk = [0u8; 4096];
            while let Ok(n) = reader.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                let mut tail = tail.lock().unwrap_or_else(|p| p.into_inner());
                let room = protocol::MAX_STDERR_BYTES.saturating_sub(tail.len());
                tail.extend_from_slice(&chunk[..n.min(room)]);
            }
        }));
    }
    let limits = Limits {
        memory: request.memory_bytes,
        output: request.output_bytes,
        run: run.clone(),
        staged: Arc::clone(&staged),
        // The measurement fault point names the worker by its scratch
        // root (009's convention).
        fault_detail: profile.worker.scratch_root.display().to_string(),
    };
    {
        let child = Arc::clone(&child);
        let breach = Arc::clone(&breach);
        let stopping = Arc::clone(&stopping);
        let limits = limits.clone();
        handles.push(std::thread::spawn(move || {
            loop {
                std::thread::sleep(POLL_INTERVAL);
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                let mut guard = child.lock().unwrap_or_else(|p| p.into_inner());
                let Some(live) = guard.as_mut() else {
                    break;
                };
                // Only an unreaped child is measured, so a reused PID
                // is never read; a child that ended is the reader's.
                if !matches!(live.try_wait(), Ok(None)) {
                    break;
                }
                if let Some(stop) = limits.measure(live.id()) {
                    *breach.lock().unwrap_or_else(|p| p.into_inner()) = Some(stop);
                    unsafe { libc::kill(live.id() as libc::pid_t, libc::SIGKILL) };
                    break;
                }
            }
        }));
    }
    Ok(Spawned {
        child,
        pid,
        stdin,
        stdout,
        stderr_tail,
        breach,
        stopping,
        staged,
        limits,
        handles,
        liveness: OwnedFd::from(liveness_writer),
        run,
        scratch,
    })
}

impl LearnWorker {
    /// Verify the profile and the bundle executable, create the scratch run
    /// directory, and start the worker. `wall_deadline` bounds the whole
    /// run; `policy` supplies the ceilings.
    pub fn launch(
        profile: &LearnProfile,
        policy: &LearningPolicy,
        wall_deadline: Instant,
        control: &Control,
    ) -> FResult<Self> {
        let threads = policy.cpu_threads;
        let Spawned {
            child,
            pid,
            stdin,
            stdout,
            stderr_tail,
            breach,
            stopping,
            staged,
            limits,
            mut handles,
            liveness,
            run,
            scratch,
        } = spawn(
            profile,
            Request {
                threads,
                cpu_seconds: Some(policy.wall_seconds.saturating_mul(u64::from(threads))),
                memory_bytes: policy.memory_bytes,
                output_bytes: policy.output_bytes,
                extra_args: test_worker_args(),
            },
            control,
        )?;
        let (tx, frames) = mpsc::channel::<Frame>();
        if let Some(stdout) = stdout {
            handles.push(std::thread::spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    match read_frame_as::<LearnHeader, _>(&mut reader) {
                        Ok((header, payload)) => {
                            if tx.send(Frame::Header(Box::new(header), payload)).is_err() {
                                break;
                            }
                        }
                        Err(FrameError::Eof) => {
                            let _ = tx.send(Frame::End(END_OF_OUTPUT.into()));
                            break;
                        }
                        Err(e) => {
                            let _ = tx.send(Frame::End(format!("worker IPC: {e}")));
                            break;
                        }
                    }
                }
            }));
        }
        Ok(Self {
            child,
            pid,
            stdin,
            frames,
            stderr_tail,
            breach,
            stopping,
            staged,
            limits,
            handles,
            liveness: Some(liveness),
            scratch_path: Some(run),
            scratch,
            next_id: 1,
            identity: Identity {
                model_function_sha256: String::new(),
                head_sha256: None,
                steps: 0,
            },
            wall_deadline,
            load_timeout: Duration::from_secs(profile.load_timeout_seconds),
            status: None,
            stopped: false,
        })
    }

    /// The worker's PID while it lives.
    pub fn pid(&self) -> Option<u32> {
        (!self.stopped).then_some(self.pid)
    }

    /// The scratch run directory, held by descriptor: starting heads go in,
    /// `head.safetensors` comes out.
    pub fn scratch(&self) -> &Dir {
        &self.scratch
    }

    /// Place a verified candidate's head in the run directory as `slot`'s
    /// file, through both descriptors, and count it as staged input. The
    /// allowance is reserved BEFORE the copy, so a poll during the copy never
    /// counts the input as worker output. Returns the copy's SHA-256 (the
    /// caller compares it with the manifest's).
    pub fn stage_head(&self, from: &Dir, slot: HeadSlot) -> FResult<String> {
        let bytes = from
            .open_file(super::candidate::HEAD)
            .and_then(|file| file.metadata())
            .map_err(|e| fail("output_write", format!("{}: {e}", slot.file_name())))?
            .len();
        self.staged.fetch_add(bytes, Ordering::SeqCst);
        super::candidate::copy_head_into(from, &self.scratch, slot.file_name())
    }

    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    fn stderr_excerpt(&self) -> String {
        let tail = self.stderr_tail.lock().unwrap_or_else(|p| p.into_inner());
        String::from_utf8_lossy(&tail).chars().take(2000).collect()
    }

    /// Stop the worker, then name the failure. A monitor breach and a
    /// kernel limit (`SIGXCPU`, `SIGXFSZ`) win over the symptom they caused.
    fn fail(&mut self, code: &'static str, message: String) -> FoundryError {
        self.shutdown();
        if let Some((code, message)) = self
            .breach
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
        {
            return fail(code, message);
        }
        match self.status.and_then(|status| status.signal()) {
            Some(libc::SIGXCPU) => {
                return fail(
                    "cpu_limit",
                    "the worker exhausted its hard CPU-time limit (wall_seconds × cpu_threads)",
                );
            }
            Some(libc::SIGXFSZ) => {
                return fail(
                    "output_limit",
                    "the worker exceeded its hard per-file output limit (output_bytes)",
                );
            }
            _ => {}
        }
        let excerpt = self.stderr_excerpt();
        if excerpt.is_empty() || code != "worker_failed" {
            fail(code, message)
        } else {
            fail(code, format!("{message}; worker stderr: {excerpt}"))
        }
    }

    /// One request and its reply. The reply must answer exactly this
    /// request: same ID, same identity, the expected kind.
    fn request(
        &mut self,
        message: Message,
        payload: &[u8],
        expect: &'static str,
        control: &Control,
        bound: Instant,
    ) -> FResult<Message> {
        if self.stopped {
            return Err(fail("worker_failed", "the worker was already stopped"));
        }
        // A frame waiting before the request is sent answers nothing: a
        // duplicate, a late or an unsolicited reply.
        match self.frames.try_recv() {
            Ok(Frame::Header(header, _)) => {
                return Err(self.fail(
                    "worker_failed",
                    format!(
                        "unsolicited {} frame for request {}",
                        header.message.kind(),
                        header.request_id
                    ),
                ));
            }
            Ok(Frame::End(why)) => return Err(self.fail("worker_failed", why)),
            Err(_) => {}
        }
        if let Err(stop) = control.check() {
            self.shutdown();
            return Err(stop);
        }
        let id = self.next_id;
        self.next_id += 1;
        let header = LearnHeader {
            protocol: LEARN_PROTOCOL,
            request_id: id,
            identity: self.identity.clone(),
            message,
        };
        let written = match self.stdin.as_mut() {
            Some(stdin) => write_frame_as(stdin, &header, payload).is_ok(),
            None => false,
        };
        if !written {
            return Err(self.fail(
                "worker_failed",
                "the request could not be written to the worker".into(),
            ));
        }
        loop {
            self.check_stops(bound, control)?;
            // The receive never outlasts the earliest applicable stop: the
            // wall clock, this request's bound (the load timeout for a
            // load) and the caller's deadline.
            let mut until = self.wall_deadline.min(bound);
            if let Some(deadline) = control.deadline() {
                until = until.min(deadline);
            }
            let slice = until
                .saturating_duration_since(Instant::now())
                .min(WAIT_SLICE);
            match self.frames.recv_timeout(slice) {
                Ok(Frame::Header(reply, payload)) => {
                    if let Err(e) = learning_fault!(
                        REPLY_RECEIVED,
                        control,
                        &format!(
                            "{} {}",
                            reply.message.kind(),
                            until.saturating_duration_since(Instant::now()).as_millis()
                        )
                    ) {
                        self.shutdown();
                        return Err(e);
                    }
                    // A reply that crossed a stop while it was received is
                    // refused, never consumed.
                    self.check_stops(bound, control)?;
                    if !payload.is_empty() {
                        return Err(self.fail(
                            "worker_failed",
                            format!("a {} reply carried a payload", reply.message.kind()),
                        ));
                    }
                    if reply.request_id != id {
                        return Err(self.fail(
                            "worker_failed",
                            format!(
                                "a {} reply for request {} arrived while {id} was outstanding",
                                reply.message.kind(),
                                reply.request_id
                            ),
                        ));
                    }
                    if reply.identity != self.identity {
                        return Err(self.fail(
                            "worker_failed",
                            format!(
                                "the reply to request {id} names another loaded identity \
                                 ({:?}) than the owner's ({:?})",
                                reply.identity, self.identity
                            ),
                        ));
                    }
                    if let Message::Error { code, message } = &reply.message {
                        let named = worker_code(code);
                        return Err(self.fail(named, format!("worker {code}: {message}")));
                    }
                    if reply.message.kind() != expect {
                        return Err(self.fail(
                            "worker_failed",
                            format!(
                                "a {} reply answered a request expecting {expect}",
                                reply.message.kind()
                            ),
                        ));
                    }
                    return Ok(reply.message);
                }
                Ok(Frame::End(why)) => return Err(self.fail("worker_failed", why)),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(
                        self.fail("worker_failed", "the worker's reply channel closed".into())
                    );
                }
            }
        }
    }

    fn check_input(&mut self, echoed: &str, ids: &[u32], markers: [u32; 2]) -> FResult<()> {
        if echoed != ipc::input_sha256(ids, markers) {
            return Err(self.fail(
                "worker_failed",
                "the reply was computed on another input than the request's".into(),
            ));
        }
        Ok(())
    }

    /// (Re)load the checkpoint and, with `head`, a starting head that the
    /// caller already placed in the scratch run directory. The owner's
    /// identity becomes `{model_function, head digest, 0}`.
    pub fn load(
        &mut self,
        policy: &LearningPolicy,
        model_function_sha256: &str,
        head: Option<(HeadSlot, String)>,
        seed: u64,
        control: &Control,
    ) -> FResult<Loaded> {
        self.identity = Identity {
            model_function_sha256: model_function_sha256.to_owned(),
            head_sha256: head.as_ref().map(|(_, sha)| sha.clone()),
            steps: 0,
        };
        let bound = Instant::now() + self.load_timeout;
        let reply = self.request(
            Message::Load {
                checkpoint: policy.model.clone(),
                head: head.map(|(slot, _)| slot),
                threads: policy.cpu_threads,
                seed,
            },
            &[],
            "loaded",
            control,
            bound,
        )?;
        let Message::Loaded {
            source_dtype,
            weights_sha256,
            encoder_config_sha256,
            counts,
            trainable,
            frozen_encoder_sha256,
        } = reply
        else {
            unreachable!("the kind was checked");
        };
        let names: Vec<String> = crate::decision_model::trainable()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        let elements: u64 = crate::decision_model::trainable()
            .iter()
            .map(|(_, shape)| shape.iter().product::<usize>() as u64)
            .sum();
        if source_dtype != policy.model.source_dtype
            || weights_sha256 != policy.model.weights_sha256
            || encoder_config_sha256 != policy.model.encoder_config_sha256
        {
            return Err(self.fail(
                "checkpoint_invalid",
                "the worker loaded another checkpoint than the policy pins".into(),
            ));
        }
        if trainable != names || counts.trainable != elements {
            return Err(self.fail(
                "worker_failed",
                "the worker's trainable set is not the contract's".into(),
            ));
        }
        Ok(Loaded {
            frozen_encoder_sha256,
            counts,
        })
    }

    /// One training update; the owner's step count follows.
    pub fn step(
        &mut self,
        ids: &[u32],
        markers: [u32; 2],
        target: u32,
        control: &Control,
    ) -> FResult<(f64, f64)> {
        let reply = self.request(
            Message::Step { markers, target },
            &ipc::encode_ids(ids),
            "stepped",
            control,
            self.wall_deadline,
        )?;
        let Message::Stepped {
            input_sha256,
            loss,
            grad_norm_before_clip,
        } = reply
        else {
            unreachable!("the kind was checked");
        };
        self.check_input(&input_sha256, ids, markers)?;
        if !loss.is_finite() || !grad_norm_before_clip.is_finite() {
            return Err(self.fail(
                "nonfinite_loss",
                "the worker reported a nonfinite loss or gradient norm".into(),
            ));
        }
        self.identity.steps += 1;
        Ok((loss, grad_norm_before_clip))
    }

    pub fn logits(
        &mut self,
        ids: &[u32],
        markers: [u32; 2],
        control: &Control,
    ) -> FResult<[f32; 2]> {
        let reply = self.request(
            Message::Logits { markers },
            &ipc::encode_ids(ids),
            "logits_out",
            control,
            self.wall_deadline,
        )?;
        let Message::LogitsOut {
            input_sha256,
            values,
        } = reply
        else {
            unreachable!("the kind was checked");
        };
        self.check_input(&input_sha256, ids, markers)?;
        if !values.iter().all(|v| v.is_finite()) {
            return Err(self.fail("nonfinite_logits", format!("logits {values:?}")));
        }
        Ok(values)
    }

    pub fn save(&mut self, ids: &[u32], markers: [u32; 2], control: &Control) -> FResult<Saved> {
        let reply = self.request(
            Message::Save { markers },
            &ipc::encode_ids(ids),
            "saved",
            control,
            self.wall_deadline,
        )?;
        let Message::Saved {
            input_sha256,
            sha256,
            bytes,
            reload_max_abs_diff,
            values,
        } = reply
        else {
            unreachable!("the kind was checked");
        };
        self.check_input(&input_sha256, ids, markers)?;
        if reload_max_abs_diff != 0.0 || !values.iter().all(|v| v.is_finite()) {
            return Err(self.fail(
                "artifact_invalid",
                format!(
                    "the saved head does not reload exactly (max difference {reload_max_abs_diff})"
                ),
            ));
        }
        Ok(Saved {
            sha256,
            bytes,
            values,
        })
    }

    pub fn frozen_hash(&mut self, control: &Control) -> FResult<String> {
        let reply = self.request(
            Message::FrozenHash,
            &[],
            "frozen_hash_out",
            control,
            self.wall_deadline,
        )?;
        let Message::FrozenHashOut { sha256 } = reply else {
            unreachable!("the kind was checked");
        };
        Ok(sha256)
    }

    /// Every stop that ends a wait: a monitor breach, the wall clock, the
    /// request's bound and the caller's control. A stop shuts the worker
    /// down and names itself.
    fn check_stops(&mut self, bound: Instant, control: &Control) -> FResult<()> {
        let breached = self
            .breach
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some((code, message)) = breached {
            return Err(self.fail(code, message));
        }
        let now = Instant::now();
        if now >= self.wall_deadline {
            return Err(self.fail(
                "worker_timeout",
                "the run's wall clock (wall_seconds) expired".into(),
            ));
        }
        if now >= bound {
            return Err(self.fail(
                "worker_timeout",
                "the worker did not load within the profile's load timeout".into(),
            ));
        }
        if let Err(stop) = control.check() {
            self.shutdown();
            return Err(stop);
        }
        Ok(())
    }

    /// The checked end of a SUCCESSFUL run, required before anything the
    /// worker produced is published (plain [`Self::shutdown`] is error
    /// cleanup only):
    ///
    /// 1. one last measurement of the live worker, fail closed, and every
    ///    stop ([`Self::check_stops`]);
    /// 2. owner EOF, then the worker's own exit (TERM/KILL after
    ///    [`EXIT_GRACE`]), reaping, and joining the reader and the monitor;
    /// 3. a breach the monitor recorded meanwhile, or a resource limit in
    ///    the exit status, is the run's named failure;
    /// 4. anything the worker sent after the last reply — an extra or
    ///    duplicate frame, a malformed one — refuses the run;
    /// 5. the worker must have exited 0 on its own.
    pub fn finish(&mut self, control: &Control) -> FResult<()> {
        if self.stopped {
            return Err(fail("worker_failed", "the worker was already stopped"));
        }
        // Measured while the handle is held and the child is unreaped, so a
        // reused PID is never read (the monitor's own rule).
        let measured = {
            let mut guard = self.child.lock().unwrap_or_else(|p| p.into_inner());
            match guard.as_mut() {
                Some(child) => match child.try_wait() {
                    Ok(None) => self.limits.measure(child.id()),
                    _ => None,
                },
                None => None,
            }
        };
        if let Some((code, message)) = measured {
            return Err(self.fail(code, message));
        }
        self.check_stops(self.wall_deadline, control)?;
        self.stop(true);
        let breached = self
            .breach
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some((code, message)) = breached {
            return Err(fail(code, message));
        }
        match self.status.and_then(|status| status.signal()) {
            Some(libc::SIGXCPU) => {
                return Err(fail(
                    "cpu_limit",
                    "the worker exhausted its hard CPU-time limit (wall_seconds × cpu_threads)",
                ));
            }
            Some(libc::SIGXFSZ) => {
                return Err(fail(
                    "output_limit",
                    "the worker exceeded its hard per-file output limit (output_bytes)",
                ));
            }
            _ => {}
        }
        let mut clean_end = false;
        while let Ok(frame) = self.frames.try_recv() {
            match frame {
                Frame::Header(header, _) => {
                    return Err(fail(
                        "worker_failed",
                        format!(
                            "an extra {} frame for request {} followed the last reply; nothing \
                             the worker produced is used",
                            header.message.kind(),
                            header.request_id
                        ),
                    ));
                }
                Frame::End(why) if why == END_OF_OUTPUT => clean_end = true,
                Frame::End(why) => return Err(fail("worker_failed", why)),
            }
        }
        if !clean_end {
            return Err(fail(
                "worker_failed",
                "the worker's output did not end cleanly after the last reply",
            ));
        }
        match self.status {
            Some(status) if status.success() => Ok(()),
            other => Err(fail(
                "worker_failed",
                format!("the worker did not exit cleanly on owner EOF ({other:?})"),
            )),
        }
    }

    /// Close stdin (owner EOF), then TERM, 5 s, KILL, reap; join the helper
    /// threads; remove the scratch run directory. Reports nothing before
    /// the process is gone. Error cleanup: it judges nothing.
    pub fn shutdown(&mut self) {
        self.stop(false);
    }

    /// Stop and reap the worker. With `wait_for_exit` it first gets
    /// [`EXIT_GRACE`] to exit on owner EOF by itself.
    fn stop(&mut self, wait_for_exit: bool) {
        if self.stopped {
            return;
        }
        self.stdin = None;
        if wait_for_exit {
            let started = Instant::now();
            while started.elapsed() < EXIT_GRACE {
                let mut guard = self.child.lock().unwrap_or_else(|p| p.into_inner());
                match guard.as_mut().map(Child::try_wait) {
                    Some(Ok(Some(status))) => {
                        self.status = Some(status);
                        *guard = None;
                        break;
                    }
                    Some(Ok(None)) => {}
                    _ => break,
                }
                drop(guard);
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        self.status = terminate_and_reap(&self.child).or(self.status);
        self.liveness = None;
        self.stopping.store(true, Ordering::SeqCst);
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
        self.stopped = true;
        if let Some(run) = self.scratch_path.take() {
            let _ = std::fs::remove_dir_all(run);
        }
    }
}

impl Drop for LearnWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}
