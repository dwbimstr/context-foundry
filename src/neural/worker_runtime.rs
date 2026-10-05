//! 009 worker runtime shared by the real embedding worker (`foundry-embed`)
//! and the test worker (`foundry-embed-fake`): owner-death containment, the
//! single-slot admission protocol loop and reply integrity. No Python here.
//!
//! Containment order is fixed: the native owner-death watcher starts before
//! the run closure may import or initialize anything heavy, and it exits the
//! process with `_exit` (no GIL, no destructors, no locks) when either the
//! liveness pipe reaches EOF or kqueue reports the owner PID exited. The
//! admission slot is a nonblocking try-acquire taken before any inference
//! allocation, held through inference, the vector copy and the bounded reply
//! write, and never released by a client timeout.
use super::protocol::{self, FrameError, Header, Purpose};
use super::provider::{DIMENSIONS, FunctionDescriptor, TokenizedInput};
#[cfg(feature = "test-faults")]
use sha2::{Digest, Sha256};
use std::io;
#[cfg(feature = "test-faults")]
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Exit code used when the watcher ends the process for owner death.
pub const OWNER_DEATH_EXIT: i32 = 75;
/// Exit code for an argv/identity refusal before the loop starts.
pub const CONFIG_EXIT: i32 = 78;
/// Exit code when the owner-death watcher cannot be armed.
pub const WATCHER_EXIT: i32 = 79;
/// Exit code for a stream that broke framing (cannot be resynchronized).
pub const FRAME_EXIT: i32 = 101;

/// Vocabulary bound the fake worker reports; test token IDs stay far below.
pub const FAKE_VOCAB: u32 = 1 << 20;

/// Revision of the first-party array-construction and output code in
/// `foundry-embed`. The descriptor's `adapter_revision` must equal it: the
/// worker computes exactly this recipe and never anything a profile merely
/// claims.
pub const REAL_ADAPTER_REVISION: u32 = 1;
/// Input and mask array element type the real adapter builds.
pub const REAL_INPUT_DTYPE: &str = "int32";
/// Padding side and pad ID the real adapter applies.
pub const REAL_PADDING_SIDE: &str = "right";
pub const REAL_PAD_ID: u32 = 11;
/// Pooling and normalization the publisher model performs (never the adapter).
pub const REAL_POOLING: &str = "publisher mean + l2";
/// Output element type the real adapter copies out.
pub const REAL_OUTPUT: &str = "f32";
/// Every file the publisher loader (and the tokenizer it builds) reads. All
/// must be named, and therefore hashed, in the descriptor's inventory: a
/// loader input outside the inventory would execute unverified.
pub const REAL_LOADER_INPUTS: [&str; 5] = [
    "config.json",
    "model.safetensors",
    "nemotron3_embed_mlx.py",
    "tokenizer.json",
    "tokenizer_config.json",
];

/// Refuse a descriptor that claims a numerical recipe other than the one the
/// real adapter implements. Checked before the model loads and before
/// `ready`, by the supervisor before launch and by the worker itself.
pub fn check_real_adapter(descriptor: &FunctionDescriptor) -> Result<(), String> {
    fn same<T: PartialEq + std::fmt::Display>(
        field: &str,
        claimed: &T,
        implemented: &T,
    ) -> Result<(), String> {
        if claimed == implemented {
            Ok(())
        } else {
            Err(format!(
                "descriptor claims {field} {claimed}, but the real adapter implements {implemented}"
            ))
        }
    }
    same(
        "adapter_revision",
        &descriptor.adapter_revision,
        &REAL_ADAPTER_REVISION,
    )?;
    same(
        "input_dtype",
        &descriptor.input_dtype.as_str(),
        &REAL_INPUT_DTYPE,
    )?;
    same(
        "mask_dtype",
        &descriptor.mask_dtype.as_str(),
        &REAL_INPUT_DTYPE,
    )?;
    same(
        "padding_side",
        &descriptor.padding_side.as_str(),
        &REAL_PADDING_SIDE,
    )?;
    same("pad_id", &descriptor.pad_id, &REAL_PAD_ID)?;
    same("pooling", &descriptor.pooling.as_str(), &REAL_POOLING)?;
    same("output", &descriptor.output.as_str(), &REAL_OUTPUT)?;
    same("dimensions", &(descriptor.dimensions as usize), &DIMENSIONS)
}

/// Require the real loader's complete input inventory in the descriptor's
/// verified artifact list (extra files are allowed, missing ones are not).
pub fn check_real_inventory(descriptor: &FunctionDescriptor) -> Result<(), String> {
    for required in REAL_LOADER_INPUTS {
        if !descriptor
            .artifact_files
            .iter()
            .any(|file| file.name == required)
        {
            return Err(format!(
                "the descriptor's artifact inventory omits the loader input {required}, which \
                 would load unverified"
            ));
        }
    }
    Ok(())
}

/// Both real-worker descriptor checks, adapter claims first.
pub fn check_real_descriptor(descriptor: &FunctionDescriptor) -> Result<(), String> {
    check_real_adapter(descriptor)?;
    check_real_inventory(descriptor)
}

/// Parsed worker argv as the supervisor writes it.
#[derive(Clone, Debug)]
pub struct WorkerArgs {
    pub owner_pid: u32,
    pub liveness_fd: i32,
    pub expected: FunctionDescriptor,
    pub model_dir: PathBuf,
    pub python_home: PathBuf,
    pub site_packages: PathBuf,
    /// Test hooks, armed only by `foundry-embed-fake`.
    #[cfg(feature = "test-faults")]
    pub hooks: Hooks,
}

impl WorkerArgs {
    /// Parse the known flags; unknown flags are returned for the caller to
    /// interpret (the fake worker's hooks, the real worker's probes).
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<(Self, Vec<String>), String> {
        let mut owner_pid = None;
        let mut liveness_fd = None;
        let mut descriptor: Option<FunctionDescriptor> = None;
        let mut model_dir = None;
        let mut python_home = None;
        let mut site_packages = None;
        let mut rest = Vec::new();
        let mut raw = args.into_iter().peekable();
        while let Some(flag) = raw.next() {
            let mut value = |name: &str| -> Result<String, String> {
                raw.next().ok_or_else(|| format!("{name} needs a value"))
            };
            match flag.as_str() {
                "--owner-pid" => {
                    owner_pid = Some(
                        value("--owner-pid")?
                            .parse()
                            .map_err(|_| "--owner-pid needs a PID")?,
                    )
                }
                "--liveness-fd" => {
                    liveness_fd = Some(
                        value("--liveness-fd")?
                            .parse()
                            .map_err(|_| "--liveness-fd needs an FD")?,
                    )
                }
                "--descriptor" => {
                    let json = value("--descriptor")?;
                    descriptor = Some(
                        serde_json::from_str(&json)
                            .map_err(|e| format!("--descriptor JSON: {e}"))?,
                    );
                }
                "--model-dir" => model_dir = Some(PathBuf::from(value("--model-dir")?)),
                "--python-home" => python_home = Some(PathBuf::from(value("--python-home")?)),
                "--site-packages" => site_packages = Some(PathBuf::from(value("--site-packages")?)),
                other => rest.push(other.to_string()),
            }
        }
        let expected = descriptor.ok_or("--descriptor is required")?;
        expected
            .validate()
            .map_err(|e| format!("descriptor: {e}"))?;
        Ok((
            Self {
                owner_pid: owner_pid.ok_or("--owner-pid is required")?,
                liveness_fd: liveness_fd.ok_or("--liveness-fd is required")?,
                expected,
                model_dir: model_dir.ok_or("--model-dir is required")?,
                python_home: python_home.ok_or("--python-home is required")?,
                site_packages: site_packages.ok_or("--site-packages is required")?,
                #[cfg(feature = "test-faults")]
                hooks: Hooks::default(),
            },
            rest,
        ))
    }
}

/// Named deterministic fault hooks of the test worker. Default values are
/// inert; `foundry-embed` never arms them and the build without
/// `test-faults` contains none of this code.
#[cfg(feature = "test-faults")]
#[derive(Clone, Debug, Default)]
pub struct Hooks {
    /// Hold the admission slot sleeping this many milliseconds per call.
    pub slow_ms: u64,
    /// Sleep this many milliseconds before reporting `ready`.
    pub load_ms: u64,
    /// Write this many bytes to stderr before reporting `ready`.
    pub stderr_flood: usize,
    /// Allocate and touch this many MiB before reporting `ready`.
    pub alloc_mb: usize,
    /// Allocate and touch this many MiB inside every call (never freed).
    pub alloc_call_mb: usize,
    /// Ignore SIGTERM (the supervisor must reach KILL).
    pub ignore_term: bool,
    /// Keep serving after stdin EOF (blocks normal shutdown).
    pub ignore_eof: bool,
    /// Report a deliberately mutated descriptor in `ready`.
    pub ready_mutate: bool,
    /// Fill stdout with junk during the first call until the write blocks.
    pub stdout_block: bool,
    /// Print a stray line to stdout before `ready`, as a library might.
    pub stray_stdout: bool,
    /// Write this process's PID to the named file as the first load step, so
    /// a test can prove a worker whose launch failed was stopped and reaped.
    pub pid_file: Option<String>,
    /// Corrupt the `vectors` reply in a named way.
    pub reply_fault: ReplyFault,
    /// Write the named phase (`load`, `call`, `blocked`) to the file at the
    /// moment the worker enters it, so a test kills the owner only once the
    /// intended phase is really running.
    pub phase_file: Option<String>,
    /// Append every received `embed` request ID to the file.
    pub request_log: Option<String>,
    /// Answer the first N `embed` requests `busy` without running them.
    pub busy_count: u32,
}

/// Reply corruption modes used by [`Hooks`].
#[cfg(feature = "test-faults")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReplyFault {
    #[default]
    None,
    /// Claim 2047 dimensions.
    WrongDims,
    /// Claim one vector more than the batch.
    WrongCount,
    /// Make the first value NaN.
    Nan,
    /// Hand-write a frame declaring an oversized payload.
    Oversize,
    /// Write raw non-frame bytes.
    Malformed,
}

#[cfg(feature = "test-faults")]
impl Hooks {
    /// Parse the fake worker's extra flags from the leftover argv.
    pub fn parse(rest: &[String]) -> Result<Self, String> {
        let mut hooks = Self::default();
        let mut i = 0;
        let value = |i: &mut usize, name: &str, rest: &[String]| -> Result<String, String> {
            *i += 1;
            rest.get(*i)
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        while i < rest.len() {
            match rest[i].as_str() {
                "--slow-ms" => {
                    hooks.slow_ms = value(&mut i, "--slow-ms", rest)?
                        .parse()
                        .map_err(|_| "--slow-ms needs a number")?
                }
                "--load-ms" => {
                    hooks.load_ms = value(&mut i, "--load-ms", rest)?
                        .parse()
                        .map_err(|_| "--load-ms needs a number")?
                }
                "--stderr-flood" => {
                    hooks.stderr_flood = value(&mut i, "--stderr-flood", rest)?
                        .parse()
                        .map_err(|_| "--stderr-flood needs a number")?
                }
                "--alloc-mb" => {
                    hooks.alloc_mb = value(&mut i, "--alloc-mb", rest)?
                        .parse()
                        .map_err(|_| "--alloc-mb needs a number")?
                }
                "--alloc-call-mb" => {
                    hooks.alloc_call_mb = value(&mut i, "--alloc-call-mb", rest)?
                        .parse()
                        .map_err(|_| "--alloc-call-mb needs a number")?
                }
                "--ignore-term" => hooks.ignore_term = true,
                "--ignore-eof" => hooks.ignore_eof = true,
                "--ready-mutate" => hooks.ready_mutate = true,
                "--stdout-block" => hooks.stdout_block = true,
                "--stray-stdout" => hooks.stray_stdout = true,
                "--pid-file" => {
                    hooks.pid_file = Some(value(&mut i, "--pid-file", rest)?);
                }
                "--phase-file" => {
                    hooks.phase_file = Some(value(&mut i, "--phase-file", rest)?);
                }
                "--request-log" => {
                    hooks.request_log = Some(value(&mut i, "--request-log", rest)?);
                }
                "--busy-count" => {
                    hooks.busy_count = value(&mut i, "--busy-count", rest)?
                        .parse()
                        .map_err(|_| "--busy-count needs a number")?
                }
                "--reply-fault" => {
                    hooks.reply_fault = match value(&mut i, "--reply-fault", rest)?.as_str() {
                        "wrong-dims" => ReplyFault::WrongDims,
                        "wrong-count" => ReplyFault::WrongCount,
                        "nan" => ReplyFault::Nan,
                        "oversize" => ReplyFault::Oversize,
                        "malformed" => ReplyFault::Malformed,
                        other => return Err(format!("unknown reply fault {other:?}")),
                    };
                }
                other => return Err(format!("unknown test hook {other:?}")),
            }
            i += 1;
        }
        Ok(hooks)
    }
}

/// The test worker's fixed descriptor: the identity the supervisor's test
/// profile pins, so `ready` equality can be checked end to end. It is a
/// descriptor the REAL recipe admits (the same adapter claims and the
/// loader's complete input inventory), so the supervisor runs one check for
/// every worker and the tests exercise exactly that check; only the model
/// behind it differs. Every artifact digest is the SHA-256 of the empty
/// file, matching the zero-byte files the tests create.
#[cfg(feature = "test-faults")]
pub fn fake_descriptor() -> FunctionDescriptor {
    let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    FunctionDescriptor {
        v: super::provider::DESCRIPTOR_VERSION,
        model: "foundry-fake/deterministic-embed-v1".into(),
        artifact_files: REAL_LOADER_INPUTS
            .iter()
            .map(|name| super::provider::ArtifactFile {
                name: (*name).into(),
                sha256: empty.into(),
            })
            .collect(),
        quantization: "none".into(),
        tokenizer: "fake-ids 1".into(),
        add_special_tokens: true,
        padding_side: REAL_PADDING_SIDE.into(),
        pad_id: REAL_PAD_ID,
        adapter_revision: REAL_ADAPTER_REVISION,
        input_dtype: REAL_INPUT_DTYPE.into(),
        mask_dtype: REAL_INPUT_DTYPE.into(),
        pooling: REAL_POOLING.into(),
        dimensions: DIMENSIONS as u32,
        output: REAL_OUTPUT.into(),
        runtime: super::provider::RuntimeClosure {
            python: "none".into(),
            mlx: "none".into(),
            mlx_metal: "none".into(),
            mlx_lm: "none".into(),
            transformers: "none".into(),
            numpy: "none".into(),
            requirements_sha256: empty.into(),
        },
        document_prefix: super::provider::DOCUMENT_PREFIX.into(),
    }
}

/// Deterministic unit-length-free vector keyed only by the ID sequence: the
/// same IDs always yield the same finite 2048 values. Shared by the test
/// worker and the supervisor tests so expectations never drift.
#[cfg(feature = "test-faults")]
pub fn deterministic_vector(ids: &[u32]) -> Vec<f32> {
    let mut hasher = Sha256::new();
    for id in ids {
        hasher.update(id.to_le_bytes());
    }
    let seed = u64::from_le_bytes(hasher.finalize()[..8].try_into().expect("8 bytes"));
    let mut state = seed | 1;
    let mut vector = Vec::with_capacity(DIMENSIONS);
    for _ in 0..DIMENSIONS {
        // xorshift64* then map the high 24 bits into [-1, 1).
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let bits = state.wrapping_mul(0x2545F4914F6CDD1D) >> 40;
        vector.push((bits as f32 / 8_388_608.0) - 1.0);
    }
    vector
}

/// One admitted embedding request handed from the reader thread to the
/// inference thread.
pub struct Job {
    pub id: u64,
    pub purpose: Purpose,
    pub inputs: Vec<TokenizedInput>,
}

struct Shared {
    /// The one admission slot: false free, true held. Taken before any
    /// inference allocation; released only after the reply is written.
    slot: AtomicBool,
    /// Vocabulary bound set by `send_ready`; requests before it fail decode.
    vocab: AtomicU32,
    /// Set once the supervisor's `hello` arrived; `ready` answers it.
    hello: AtomicBool,
    /// The private frame channel (see [`take_ipc_stdout`]).
    out: Mutex<std::fs::File>,
    #[cfg(feature = "test-faults")]
    hooks: Hooks,
    /// Embed requests still to be answered `busy` without running
    /// (`--busy-count`), a deterministic stand-in for the transient slot
    /// release race.
    #[cfg(feature = "test-faults")]
    busy_left: AtomicU32,
    /// Set when stdout broke; the engine stops after the current job.
    out_broken: AtomicBool,
}

impl Shared {
    fn write_frame(&self, header: &Header, payload: &[u8]) -> bool {
        let mut out = match self.out.lock() {
            Ok(out) => out,
            Err(poisoned) => poisoned.into_inner(),
        };
        let ok = protocol::write_frame(&mut *out, header, payload).is_ok();
        if !ok {
            self.out_broken.store(true, Ordering::SeqCst);
        }
        ok
    }

    fn write_error(&self, id: Option<u64>, code: &str, message: &str) {
        let _ = self.write_frame(
            &Header::Error {
                protocol: protocol::PROTOCOL_VERSION,
                id,
                code: code.into(),
                message: message.into(),
            },
            &[],
        );
    }
}

/// The inference side of the worker: sends `ready`, receives admitted jobs
/// and writes replies, holding the slot until each reply is on the wire.
pub struct Engine {
    shared: Arc<Shared>,
    jobs: mpsc::Receiver<Job>,
    pub args: WorkerArgs,
}

impl Engine {
    /// Report readiness with the verified descriptor and vocabulary bound.
    /// Returns false when stdout already broke.
    pub fn send_ready(&mut self, vocab: u32) -> bool {
        if vocab == 0 {
            self.fail_load("load_failed", "vocabulary size is zero");
            return false;
        }
        // `ready` answers `hello`: wait (bounded) for the supervisor's first
        // frame; the model has usually taken far longer to load.
        let hello_deadline = Instant::now() + Duration::from_secs(10);
        while !self.shared.hello.load(Ordering::Acquire) {
            if Instant::now() >= hello_deadline {
                self.fail_load("hello_missing", "the supervisor sent no hello");
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        self.shared.vocab.store(vocab, Ordering::SeqCst);
        #[cfg_attr(not(feature = "test-faults"), allow(unused_mut))]
        let mut descriptor = self.args.expected.clone();
        #[cfg(feature = "test-faults")]
        if self.shared.hooks.ready_mutate {
            descriptor.model.push_str("-mutated");
        }
        self.shared.write_frame(
            &Header::Ready {
                protocol: protocol::PROTOCOL_VERSION,
                descriptor: Box::new(descriptor),
                vocab_size: vocab,
            },
            &[],
        )
    }

    /// A named load failure before any `ready`; best effort on a broken pipe.
    pub fn fail_load(&mut self, code: &str, message: &str) {
        self.shared.write_error(None, code, message);
    }

    /// Block until the next admitted job; `None` is shutdown (stdin EOF).
    pub fn next_job(&mut self) -> Option<Job> {
        self.jobs.recv().ok()
    }

    /// Reply with vectors for `job`, then release the slot. Returns false
    /// when stdout broke and the worker should exit.
    pub fn finish_job(
        &mut self,
        job: Job,
        #[cfg_attr(not(feature = "test-faults"), allow(unused_mut))] mut vectors: Vec<Vec<f32>>,
    ) -> bool {
        let shape_ok = vectors.len() == job.inputs.len()
            && vectors
                .iter()
                .all(|v| v.len() == DIMENSIONS && v.iter().all(|c| c.is_finite()));
        if !shape_ok {
            self.shared.write_error(
                Some(job.id),
                "malformed_vectors",
                "inference produced a wrong shape or nonfinite value",
            );
            self.shared.slot.store(false, Ordering::Release);
            return !self.shared.out_broken.load(Ordering::SeqCst);
        }
        #[cfg(feature = "test-faults")]
        {
            let hooks = &self.shared.hooks;
            if hooks.stdout_block {
                // Fill the pipe with non-blocking writes until it is full
                // (EAGAIN), and only then record the phase and return to a
                // blocking write that cannot complete: `blocked` is on
                // record only once the next write is certain to block. The
                // owner-death watcher is what ends this process.
                let junk = [b'x'; 4096];
                let fd = {
                    use std::os::fd::AsRawFd;
                    match self.shared.out.lock() {
                        Ok(out) => out.as_raw_fd(),
                        Err(poisoned) => poisoned.into_inner().as_raw_fd(),
                    }
                };
                let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
                unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
                while unsafe { libc::write(fd, junk.as_ptr().cast(), junk.len()) } > 0 {}
                unsafe { libc::fcntl(fd, libc::F_SETFL, flags) };
                mark_phase("blocked");
                while unsafe { libc::write(fd, junk.as_ptr().cast(), junk.len()) } > 0 {}
            }
            match hooks.reply_fault {
                ReplyFault::None => {}
                ReplyFault::WrongDims | ReplyFault::WrongCount => {
                    let dims = if hooks.reply_fault == ReplyFault::WrongDims {
                        (DIMENSIONS - 1) as u32
                    } else {
                        DIMENSIONS as u32
                    };
                    let count = if hooks.reply_fault == ReplyFault::WrongCount {
                        vectors.len() as u32 + 1
                    } else {
                        vectors.len() as u32
                    };
                    let ok = self.shared.write_frame(
                        &Header::Vectors {
                            protocol: protocol::PROTOCOL_VERSION,
                            id: job.id,
                            count,
                            dims,
                        },
                        &protocol::encode_vectors(&vectors),
                    );
                    self.shared.slot.store(false, Ordering::Release);
                    return ok && !self.shared.out_broken.load(Ordering::SeqCst);
                }
                ReplyFault::Nan => vectors[0][0] = f32::NAN,
                ReplyFault::Oversize => {
                    let header = format!(
                        "{{\"kind\":\"vectors\",\"protocol\":1,\"id\":{},\"count\":1,\"dims\":{DIMENSIONS}}}",
                        job.id
                    );
                    let ok = self.raw_frame(&header, 0x00ff_ffff, b"partial");
                    self.shared.slot.store(false, Ordering::Release);
                    return ok;
                }
                ReplyFault::Malformed => {
                    let mut out = match self.shared.out.lock() {
                        Ok(out) => out,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    let ok = out
                        .write_all(b"not a frame at all")
                        .and_then(|()| out.flush())
                        .is_ok();
                    self.shared.slot.store(false, Ordering::Release);
                    return ok;
                }
            }
        }
        let ok = self.shared.write_frame(
            &Header::Vectors {
                protocol: protocol::PROTOCOL_VERSION,
                id: job.id,
                count: vectors.len() as u32,
                dims: DIMENSIONS as u32,
            },
            &protocol::encode_vectors(&vectors),
        );
        self.shared.slot.store(false, Ordering::Release);
        ok && !self.shared.out_broken.load(Ordering::SeqCst)
    }

    /// Reply with a named failure for `job`, then release the slot.
    pub fn fail_job(&mut self, job: Job, code: &str, message: &str) -> bool {
        self.shared.write_error(Some(job.id), code, message);
        self.shared.slot.store(false, Ordering::Release);
        !self.shared.out_broken.load(Ordering::SeqCst)
    }

    /// Hand-write a frame declaring `declared_len` payload bytes while
    /// sending `payload` bytes; used only by the oversize reply fault.
    #[cfg(feature = "test-faults")]
    fn raw_frame(&self, header: &str, declared_len: u32, payload: &[u8]) -> bool {
        let mut out = match self.shared.out.lock() {
            Ok(out) => out,
            Err(poisoned) => poisoned.into_inner(),
        };
        let ok = out
            .write_all(&(header.len() as u32).to_le_bytes())
            .and_then(|()| out.write_all(header.as_bytes()))
            .and_then(|()| out.write_all(&declared_len.to_le_bytes()))
            .and_then(|()| out.write_all(payload))
            .and_then(|()| out.flush())
            .is_ok();
        if !ok {
            self.shared.out_broken.store(true, Ordering::SeqCst);
        }
        ok
    }
}

/// Take the frame channel private. The original stdout descriptor moves to a
/// new close-on-exec descriptor owned by the runtime, and descriptor 1
/// becomes a copy of stderr: a stray `print` from any library (a Python
/// warning, a C `printf`) then lands in the supervisor's bounded stderr
/// drain instead of corrupting a frame.
fn take_ipc_stdout() -> io::Result<std::fs::File> {
    use std::os::fd::FromRawFd;
    let ipc = unsafe { libc::fcntl(1, libc::F_DUPFD_CLOEXEC, 3) };
    if ipc < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `ipc` is a fresh descriptor this function now owns.
    let file = unsafe { std::fs::File::from_raw_fd(ipc) };
    if unsafe { libc::dup2(2, 1) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(file)
}

/// Run one worker: verify ownership, arm the owner-death watcher, start the
/// reader thread, then hand the engine to `run` (the model side). Returns
/// the process exit code.
pub fn serve(args: WorkerArgs, run: impl FnOnce(&mut Engine) -> i32) -> i32 {
    let ppid = unsafe { libc::getppid() } as u32;
    if ppid != args.owner_pid {
        eprintln!(
            "owner PID {} does not match parent {ppid}; refusing to run",
            args.owner_pid
        );
        return CONFIG_EXIT;
    }
    match arm_owner_death_watcher(args.owner_pid, args.liveness_fd) {
        Ok(Armed::Watching) => {}
        // The owner died between the parent check and the registration:
        // end now, silently (its stderr pipe may be gone with it).
        Ok(Armed::OwnerGone) => exit_now(OWNER_DEATH_EXIT),
        Err(message) => {
            eprintln!("cannot arm owner-death watcher: {message}");
            return WATCHER_EXIT;
        }
    }
    let ipc_out = match take_ipc_stdout() {
        Ok(file) => file,
        Err(e) => {
            eprintln!("cannot take the frame channel private: {e}");
            return CONFIG_EXIT;
        }
    };
    let shared = Arc::new(Shared {
        slot: AtomicBool::new(false),
        vocab: AtomicU32::new(0),
        hello: AtomicBool::new(false),
        out: Mutex::new(ipc_out),
        #[cfg(feature = "test-faults")]
        hooks: args.hooks.clone(),
        out_broken: AtomicBool::new(false),
        #[cfg(feature = "test-faults")]
        busy_left: AtomicU32::new(args.hooks.busy_count),
    });
    let (tx, rx) = mpsc::channel::<Job>();
    let digest = args.expected.digest();

    // Dedicated reader thread: stays responsive during inference, refuses a
    // second embed with `busy` and never queues.
    {
        let shared = Arc::clone(&shared);
        let tx = tx;
        std::thread::spawn(move || {
            let stdin = io::stdin();
            let mut lock = stdin.lock();
            loop {
                match protocol::read_frame(&mut lock) {
                    Ok((header, payload)) => {
                        if !shared.hello.load(Ordering::Acquire)
                            && !matches!(header, Header::Hello { .. })
                        {
                            shared.write_error(
                                None,
                                "frame_invalid",
                                "the first frame must be hello",
                            );
                            exit_now(FRAME_EXIT);
                        }
                        match header {
                            Header::Hello { .. } => {
                                if shared.hello.swap(true, Ordering::AcqRel) {
                                    shared.write_error(None, "frame_invalid", "second hello");
                                    exit_now(FRAME_EXIT);
                                }
                            }
                            Header::Embed {
                                id,
                                descriptor_digest,
                                purpose,
                                lengths,
                                ..
                            } => {
                                #[cfg(feature = "test-faults")]
                                {
                                    log_request(&shared.hooks, id);
                                    if shared.busy_left.load(Ordering::SeqCst) > 0 {
                                        shared.busy_left.fetch_sub(1, Ordering::SeqCst);
                                        shared.write_frame(
                                            &Header::Busy {
                                                protocol: protocol::PROTOCOL_VERSION,
                                                id,
                                            },
                                            &[],
                                        );
                                        continue;
                                    }
                                }
                                if shared
                                    .slot
                                    .compare_exchange(
                                        false,
                                        true,
                                        Ordering::Acquire,
                                        Ordering::Relaxed,
                                    )
                                    .is_err()
                                {
                                    shared.write_frame(
                                        &Header::Busy {
                                            protocol: protocol::PROTOCOL_VERSION,
                                            id,
                                        },
                                        &[],
                                    );
                                    continue;
                                }
                                if descriptor_digest != digest {
                                    shared.slot.store(false, Ordering::Release);
                                    shared.write_error(
                                        Some(id),
                                        "descriptor_mismatch",
                                        "request names a stale function descriptor",
                                    );
                                    continue;
                                }
                                let vocab = shared.vocab.load(Ordering::SeqCst);
                                match protocol::decode_ids(purpose, &lengths, &payload, vocab) {
                                    Ok(inputs) => {
                                        if tx
                                            .send(Job {
                                                id,
                                                purpose,
                                                inputs,
                                            })
                                            .is_err()
                                        {
                                            shared.slot.store(false, Ordering::Release);
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        shared.slot.store(false, Ordering::Release);
                                        shared.write_error(
                                            Some(id),
                                            "input_invalid",
                                            &e.to_string(),
                                        );
                                    }
                                }
                            }
                            other => {
                                shared.write_error(
                                    None,
                                    "frame_invalid",
                                    &format!(
                                        "unexpected {} frame on worker input",
                                        kind_name(&other)
                                    ),
                                );
                                exit_now(FRAME_EXIT);
                            }
                        }
                    }
                    Err(FrameError::Eof) => {
                        #[cfg(feature = "test-faults")]
                        if shared.hooks.ignore_eof {
                            // Keep the channel open forever: only KILL ends us.
                            loop {
                                std::thread::park();
                            }
                        }
                        drop(tx);
                        break;
                    }
                    Err(e) => {
                        shared.write_error(None, "frame_invalid", &e.to_string());
                        exit_now(FRAME_EXIT);
                    }
                }
            }
        });
    }

    let mut engine = Engine {
        shared,
        jobs: rx,
        args,
    };
    run(&mut engine)
}

fn kind_name(header: &Header) -> &'static str {
    match header {
        Header::Hello { .. } => "hello",
        Header::Ready { .. } => "ready",
        Header::Embed { .. } => "embed",
        Header::Vectors { .. } => "vectors",
        Header::Busy { .. } => "busy",
        Header::Error { .. } => "error",
    }
}

/// What arming the owner-death watcher found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Armed {
    /// The watcher thread is running and will end the process on owner death.
    Watching,
    /// The owner was already gone (exit notification or liveness-pipe EOF
    /// pending) when the registration call returned. No thread was started:
    /// the caller must end the process now.
    OwnerGone,
}

/// What one kqueue event says about the owner.
enum Verdict {
    /// The owner is gone: the process must end.
    Gone,
    /// Not about the owner's death.
    Ignore,
    /// The kernel refused the registration or the queue broke.
    Failed(String),
}

/// One classification shared by the registration call and the watcher
/// loop, so a notification retrieved while arming can never be lost:
/// `NOTE_EXIT` on the owner PID, an owner that no longer exists (`ESRCH`),
/// or end-of-file on the liveness pipe.
fn classify_event(event: &libc::kevent, owner_pid: u32, liveness_fd: i32) -> Verdict {
    if event.flags & libc::EV_ERROR != 0 {
        let errno = event.data as i32;
        if errno == 0 {
            return Verdict::Ignore;
        }
        if event.filter == libc::EVFILT_PROC
            && event.ident == owner_pid as usize
            && errno == libc::ESRCH
        {
            return Verdict::Gone;
        }
        return Verdict::Failed(format!(
            "kevent change failed: {}",
            io::Error::from_raw_os_error(errno)
        ));
    }
    if event.filter == libc::EVFILT_PROC && event.fflags & libc::NOTE_EXIT != 0 {
        return Verdict::Gone;
    }
    if event.filter == libc::EVFILT_READ {
        if event.flags & libc::EV_EOF != 0 {
            return Verdict::Gone;
        }
        if event.data > 0 {
            // Stray bytes on a pipe that carries none: drain one so the
            // level-triggered filter does not spin, and re-check for EOF.
            let mut byte = [0u8; 1];
            let got = unsafe { libc::read(liveness_fd, byte.as_mut_ptr().cast(), 1) };
            if got == 0 {
                return Verdict::Gone;
            }
        }
    }
    Verdict::Ignore
}

/// Arm one native thread that exits the process when the owner disappears.
/// Both signals are required: kqueue `EVFILT_PROC`/`NOTE_EXIT` on the owner
/// PID and EOF on the inherited liveness pipe. The thread never touches
/// Python or any lock and sits in `kevent` only; on either signal it calls
/// `_exit`. Events already pending at registration are handled here, with
/// the watcher loop's own classification, before the thread starts.
pub fn arm_owner_death_watcher(owner_pid: u32, liveness_fd: i32) -> Result<Armed, String> {
    let kq = unsafe { libc::kqueue() };
    if kq < 0 {
        return Err(format!("kqueue: {}", io::Error::last_os_error()));
    }
    let changes = [
        libc::kevent {
            ident: owner_pid as usize,
            filter: libc::EVFILT_PROC,
            flags: libc::EV_ADD,
            fflags: libc::NOTE_EXIT,
            data: 0,
            udata: std::ptr::null_mut(),
        },
        libc::kevent {
            ident: liveness_fd as usize,
            filter: libc::EVFILT_READ,
            flags: libc::EV_ADD,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        },
    ];
    let mut events = [unsafe { std::mem::zeroed::<libc::kevent>() }; 4];
    // Register with a zero timeout. The call also returns events that are
    // ALREADY pending (an owner that exited, a pipe already at EOF); a
    // process-exit notification is one-shot, so retrieving it here consumes
    // it and the watcher loop below would never see it.
    let n = unsafe {
        libc::kevent(
            kq,
            changes.as_ptr(),
            changes.len() as i32,
            events.as_mut_ptr(),
            events.len() as i32,
            &libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
        )
    };
    if n < 0 {
        let error = io::Error::last_os_error();
        unsafe { libc::close(kq) };
        return Err(format!("kevent register: {error}"));
    }
    for event in &events[..n as usize] {
        match classify_event(event, owner_pid, liveness_fd) {
            Verdict::Gone => {
                unsafe { libc::close(kq) };
                return Ok(Armed::OwnerGone);
            }
            Verdict::Failed(message) => {
                unsafe { libc::close(kq) };
                return Err(message);
            }
            Verdict::Ignore => {}
        }
    }
    std::thread::spawn(move || {
        loop {
            let mut events = [unsafe { std::mem::zeroed::<libc::kevent>() }; 4];
            let n = unsafe {
                libc::kevent(
                    kq,
                    std::ptr::null(),
                    0,
                    events.as_mut_ptr(),
                    events.len() as i32,
                    std::ptr::null(),
                )
            };
            if n < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                // A broken watcher must never outlive its owner.
                exit_now(OWNER_DEATH_EXIT);
            }
            for event in &events[..n as usize] {
                match classify_event(event, owner_pid, liveness_fd) {
                    Verdict::Gone | Verdict::Failed(_) => exit_now(OWNER_DEATH_EXIT),
                    Verdict::Ignore => {}
                }
            }
        }
    });
    Ok(Armed::Watching)
}

/// Leave the process now: no destructors, no atexit handlers, no locks. The
/// only exit the watcher and the frame-error paths use, because an MLX
/// evaluation may be running on another thread.
pub fn exit_now(code: i32) -> ! {
    #[cfg(feature = "test-faults")]
    mark_phase(&format!("exit {}", unix_nanos()));
    unsafe { libc::_exit(code) }
}

/// `pre_exec` body for the shim spawns: clear `FD_CLOEXEC` on `fd` so the
/// exec'd image inherits it.
#[cfg(feature = "test-faults")]
fn inherit_fd(fd: i32) -> impl FnMut() -> io::Result<()> + Send + Sync + 'static {
    move || {
        if unsafe { libc::fcntl(fd, libc::F_SETFD, 0) } == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

/// The fake worker's `--shim` helper for owner-death tests: become the owner
/// of one child copy of this executable, keep the child's liveness pipe open
/// from a separate `sleep` process (so only kqueue can report the shim's
/// death), print the child's PID and park forever. Killing the shim with
/// SIGKILL must end the child within 2 s.
#[cfg(feature = "test-faults")]
pub fn run_shim(argv: Vec<String>) -> ! {
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let (reader, writer) = match io::pipe() {
        Ok(pipe) => pipe,
        Err(e) => {
            eprintln!("shim: cannot create the liveness pipe: {e}");
            exit_now(WATCHER_EXIT)
        }
    };
    let read_fd = reader.as_raw_fd();
    let write_fd = writer.as_raw_fd();
    // A `sleep` process holds the write end open, so the shim's death is
    // observable only through kqueue on the shim PID.
    let mut holder = Command::new("/bin/sleep");
    holder
        .arg("20")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe { holder.pre_exec(inherit_fd(write_fd)) };
    if let Err(e) = holder.spawn() {
        eprintln!("shim: cannot start the pipe holder: {e}");
        exit_now(WATCHER_EXIT)
    }
    // Strip any stale owner/liveness pairs and the shim flags; the shim
    // supplies its own owner PID and liveness descriptor. `--shim-exe PATH`
    // makes the shim own that executable (the real worker) instead of a
    // copy of this one.
    let mut cleaned = Vec::new();
    let mut worker_exe: Option<String> = None;
    let mut args = argv.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--shim" => {}
            "--liveness-fd" | "--owner-pid" => {
                args.next();
            }
            "--shim-exe" => worker_exe = args.next().cloned(),
            _ => cleaned.push(arg.clone()),
        }
    }
    let mut worker = Command::new(match worker_exe {
        Some(path) => PathBuf::from(path),
        None => std::env::current_exe().expect("current exe"),
    });
    worker
        .args(&cleaned)
        .arg("--owner-pid")
        .arg(std::process::id().to_string())
        .arg("--liveness-fd")
        .arg(read_fd.to_string())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    unsafe { worker.pre_exec(inherit_fd(read_fd)) };
    let child = match worker.spawn() {
        Ok(child) => child,
        Err(e) => {
            eprintln!("shim: cannot spawn worker: {e}");
            exit_now(WATCHER_EXIT)
        }
    };
    println!("shim-worker-pid {}", child.id());
    let _ = io::stdout().flush();
    loop {
        std::thread::park();
    }
}

/// The file this process appends its phase lines to (test builds only).
#[cfg(feature = "test-faults")]
static PHASE_FILE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Name the phase file. A test passes `--phase-file PATH` to the worker; the
/// worker records each phase it really enters, and the instant it calls
/// `_exit`, so a test acts on the phase that is running rather than on a
/// fixed delay and can measure `_exit`-to-disappearance.
#[cfg(feature = "test-faults")]
pub fn set_phase_file(path: &str) {
    let _ = PHASE_FILE.set(PathBuf::from(path));
}

/// Record that the worker has just entered `phase`: one appended line in the
/// phase file, written before the phase's first action. Compiled out of
/// production builds.
pub fn mark_phase(phase: &str) {
    #[cfg(feature = "test-faults")]
    {
        if let Some(path) = PHASE_FILE.get()
            && let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
        {
            let _ = writeln!(file, "{phase}");
        }
    }
    #[cfg(not(feature = "test-faults"))]
    let _ = phase;
}

/// Nanoseconds since the Unix epoch, for the `exit` phase line.
#[cfg(feature = "test-faults")]
fn unix_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// Append one received `embed` request ID to the `--request-log`, if any.
#[cfg(feature = "test-faults")]
fn log_request(hooks: &Hooks, id: u64) {
    if let Some(path) = &hooks.request_log
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        let _ = writeln!(file, "{id}");
    }
}

/// Sleep helper for the fake worker's timing hooks.
#[cfg(feature = "test-faults")]
pub fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// Allocate `mib` MiB and touch every page so the footprint is real. The
/// caller keeps the buffer alive: a freed spike can fall between two
/// supervisor polls, which would test nothing.
#[cfg(feature = "test-faults")]
pub fn allocate_touching(mib: usize) -> Vec<u8> {
    let bytes = mib.saturating_mul(1024 * 1024);
    let mut buffer = vec![0u8; bytes];
    for chunk in buffer.chunks_mut(4096) {
        chunk[0] = 1;
    }
    buffer
}
