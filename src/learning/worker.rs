//! 013 T002 worker side: the serve loop shared by the real `foundry-learn`
//! (LibTorch) and the fault-test `foundry-learn-fake`. It reuses 009's
//! worker runtime for ownership: the parent-PID check, the owner-death
//! watcher (kqueue `NOTE_EXIT` plus liveness-pipe EOF, `_exit` on either)
//! and the private frame channel. The numerical work is the [`Backend`]'s.
//!
//! The loop is strictly request/reply: one frame in, one frame out. It
//! refuses a request ID that does not increase, an identity other than the
//! one loaded, a malformed input, and any nonfinite number it would send;
//! every refusal is an `error` frame and the end of the process. Owner EOF
//! ends the process normally.
//!
//! 013 T003: a `serve` load (the selected candidate's head, evaluation
//! mode, its fitted temperature) switches the loop to IPC-v2 predictions
//! for the rest of the process: one [`PredictRequest`] in, one
//! [`PredictReply`] out, nothing else accepted.
use super::ipc::{
    self, HeadSlot, Identity, LEARN_PROTOCOL, LearnHeader, Message, PREDICT_PROTOCOL,
    ParameterCounts, PredictReply, PredictRequest, Probabilities,
};
use crate::decision_model::CheckpointPin;
use crate::neural::protocol::{FrameError, read_frame_as, write_frame_as};
use crate::neural::worker_runtime::{
    self, Armed, CONFIG_EXIT, FRAME_EXIT, OWNER_DEATH_EXIT, WATCHER_EXIT,
};
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// A named refusal from the backend; it ends the process.
#[derive(Debug)]
pub struct Refusal {
    pub code: &'static str,
    pub message: String,
}

impl Refusal {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<crate::FoundryError> for Refusal {
    fn from(error: crate::FoundryError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

/// What a load verified.
pub struct LoadReport {
    pub source_dtype: String,
    pub weights_sha256: String,
    pub encoder_config_sha256: String,
    pub counts: ParameterCounts,
    pub trainable: Vec<String>,
    pub frozen_encoder_sha256: String,
}

/// The numerical engine behind the loop.
pub trait Backend {
    /// (Re)load the checkpoint from `dir`, then `head` (a starting head's
    /// exact bytes) when given.
    fn load(
        &mut self,
        dir: &Path,
        head: Option<&[u8]>,
        threads: u32,
        seed: u64,
    ) -> Result<LoadReport, Refusal>;
    /// One training update: the loss and the gradient norm before clipping.
    fn step(
        &mut self,
        ids: &[u32],
        markers: [usize; 2],
        target: usize,
    ) -> Result<(f64, f64), Refusal>;
    fn logits(&mut self, ids: &[u32], markers: [usize; 2]) -> Result<[f32; 2], Refusal>;
    /// The exact `head.safetensors` bytes of the current trainable set.
    fn head_bytes(&mut self) -> Result<Vec<u8>, Refusal>;
    /// Reload the trainable set from `bytes` (as read back from disk) and
    /// return the largest absolute difference from the in-memory weights
    /// and the probe's logits computed with the reloaded weights.
    fn reload(
        &mut self,
        bytes: &[u8],
        ids: &[u32],
        markers: [usize; 2],
    ) -> Result<(f64, [f32; 2]), Refusal>;
    fn frozen_hash(&mut self) -> Result<String, Refusal>;
}

/// A serving reply rewrite: the prediction count and the reply header, to
/// the headers actually sent (none, one or several).
pub type ServeReplyHook = fn(u64, Vec<u8>) -> Vec<Vec<u8>>;

/// Reply-side fault hooks of the fake worker (test-faults builds parse
/// them; the real worker always runs with none).
#[derive(Clone, Debug, Default)]
pub struct ReplyFaults {
    /// Send the n-th (1-based) `logits` reply twice.
    pub duplicate_logits: Option<u64>,
    /// Answer the n-th `logits` request with the previous request's ID.
    pub stale_logits: Option<u64>,
    /// After a `save`, report one more update than happened.
    pub drift_after_save: bool,
    /// Send the `saved` reply twice.
    pub duplicate_saved: bool,
    /// Append `<kind> <request_id>` for every request received.
    pub request_log: Option<PathBuf>,
    /// Serving: rewrite the n-th (1-based) prediction reply.
    pub serve_reply: Option<ServeReplyHook>,
}

pub struct WorkerArgs {
    pub owner_pid: u32,
    pub liveness_fd: i32,
    pub checkpoint_dir: PathBuf,
    /// The absolute scratch run directory. App Sandbox moves a sandboxed
    /// process's working directory into its container at start-up, so the
    /// worker re-enters this directory itself before it touches any file.
    pub run_dir: PathBuf,
    pub faults: ReplyFaults,
}

impl WorkerArgs {
    /// Parse the supervisor's flags; anything else is returned for the
    /// caller (the fake worker's hooks).
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<(Self, Vec<String>), String> {
        let mut owner_pid = None;
        let mut liveness_fd = None;
        let mut checkpoint_dir = None;
        let mut run_dir = None;
        let mut rest = Vec::new();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
            match arg.as_str() {
                "--owner-pid" => {
                    owner_pid = Some(
                        value("--owner-pid")?
                            .parse::<u32>()
                            .map_err(|e| format!("--owner-pid: {e}"))?,
                    );
                }
                "--liveness-fd" => {
                    liveness_fd = Some(
                        value("--liveness-fd")?
                            .parse::<i32>()
                            .map_err(|e| format!("--liveness-fd: {e}"))?,
                    );
                }
                "--checkpoint-dir" => {
                    checkpoint_dir = Some(PathBuf::from(value("--checkpoint-dir")?));
                }
                "--run-dir" => {
                    run_dir = Some(PathBuf::from(value("--run-dir")?));
                }
                _ => rest.push(arg),
            }
        }
        Ok((
            Self {
                owner_pid: owner_pid.ok_or("--owner-pid is required")?,
                liveness_fd: liveness_fd.ok_or("--liveness-fd is required")?,
                checkpoint_dir: checkpoint_dir.ok_or("--checkpoint-dir is required")?,
                run_dir: run_dir.ok_or("--run-dir is required")?,
                faults: ReplyFaults::default(),
            },
            rest,
        ))
    }
}

struct Channel {
    out: std::fs::File,
}

impl Channel {
    fn send(&mut self, request_id: u64, identity: &Identity, message: Message) -> bool {
        let header = LearnHeader {
            protocol: LEARN_PROTOCOL,
            request_id,
            identity: identity.clone(),
            message,
        };
        write_frame_as(&mut self.out, &header, &[]).is_ok()
    }

    /// Send a refusal and end the process: every worker error is terminal.
    fn refuse(&mut self, request_id: u64, identity: &Identity, refusal: Refusal) -> ! {
        let _ = self.send(
            request_id,
            identity,
            Message::Error {
                code: refusal.code.to_owned(),
                message: refusal.message,
            },
        );
        worker_runtime::exit_now(1)
    }
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// The digest the owner expects after `save`.
fn sha256(bytes: &[u8]) -> String {
    crate::digest(bytes)
}

/// Make `dir` (absolute, a real directory, not a link) the working
/// directory, and confirm by device and inode that the working directory
/// now is that directory.
fn enter_run_dir(dir: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt as _;
    if !dir.is_absolute() {
        return Err(format!("{} is not an absolute path", dir.display()));
    }
    let named = std::fs::symlink_metadata(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if !named.is_dir() {
        return Err(format!("{} is not a directory", dir.display()));
    }
    std::env::set_current_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let here = std::fs::metadata(".").map_err(|e| format!("working directory: {e}"))?;
    if (here.dev(), here.ino()) != (named.dev(), named.ino()) {
        return Err(format!(
            "the working directory is not {} after entering it",
            dir.display()
        ));
    }
    Ok(())
}

/// Write `bytes` as `head.safetensors` in the working directory (the
/// scratch run directory), never through a link, fsync it, and read it back
/// from disk.
fn write_head(bytes: &[u8]) -> Result<Vec<u8>, Refusal> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let name = "head.safetensors";
    // `EFBIG` is the hard per-file output limit (`RLIMIT_FSIZE`) refusing
    // the write when `SIGXFSZ` does not end the process first.
    let io = |what: &str, e: std::io::Error| {
        let code = if e.raw_os_error() == Some(libc::EFBIG) {
            "output_limit"
        } else {
            "output_write"
        };
        Refusal::new(code, format!("{what} {name}: {e}"))
    };
    match std::fs::symlink_metadata(name) {
        Ok(meta) if meta.is_file() => std::fs::remove_file(name).map_err(|e| io("replace", e))?,
        Ok(_) => {
            return Err(Refusal::new(
                "output_write",
                format!("{name} is not a regular file"),
            ));
        }
        Err(_) => {}
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(name)
        .map_err(|e| io("create", e))?;
    file.write_all(bytes).map_err(|e| io("write", e))?;
    file.sync_all().map_err(|e| io("fsync", e))?;
    drop(file);
    let back = std::fs::read(name).map_err(|e| io("read back", e))?;
    Ok(back)
}

/// Read a starting head from the working directory without following a
/// link.
fn read_head(slot: HeadSlot) -> Result<Vec<u8>, Refusal> {
    use std::io::Read as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let name = slot.file_name();
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(name)
        .map_err(|e| Refusal::new("artifact_invalid", format!("{name}: {e}")))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| Refusal::new("artifact_invalid", format!("{name}: {e}")))?;
    Ok(bytes)
}

fn log_request(faults: &ReplyFaults, kind: &str, id: u64) {
    log_line(faults, &format!("{kind} {id}"));
}

fn log_line(faults: &ReplyFaults, line: &str) {
    if let Some(path) = &faults.request_log
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

/// What a serving worker loaded (013 T003).
struct Serving {
    candidate_sha256: String,
    model_function_sha256: String,
    temperature: f64,
}

/// A serving refusal: the IPC-v2 reply has no error form, so the reason
/// goes to stderr (which the owner keeps bounded) and the process ends.
fn refuse_serving(code: &str, message: impl std::fmt::Display) -> ! {
    eprintln!("{code}: {message}");
    worker_runtime::exit_now(1)
}

fn write_raw_frame(out: &mut std::fs::File, header: &[u8]) -> std::io::Result<()> {
    out.write_all(&(header.len() as u32).to_le_bytes())?;
    out.write_all(header)?;
    out.write_all(&0u32.to_le_bytes())?;
    out.flush()
}

/// Serve IPC-v2 predictions until owner EOF: the request must follow the
/// last ID, name exactly the loaded candidate and model function, the
/// family and a permutation of the stable options, and carry a decodable
/// rendering; then evaluation-mode logits, the contract's stable softmax at
/// the candidate's fitted temperature, and the maximum/tie rule.
fn serve_predictions(
    out: &mut std::fs::File,
    input: &mut impl std::io::Read,
    backend: &mut dyn Backend,
    serving: &Serving,
    faults: &ReplyFaults,
    mut last_id: u64,
) -> i32 {
    use crate::decision_model::{FAMILY, OPTIONS, STATE_MAX_BYTES};
    let mut count = 0u64;
    loop {
        let (request, payload) = match read_frame_as::<PredictRequest, _>(input) {
            Ok(frame) => frame,
            Err(FrameError::Eof) => return 0,
            Err(e) => {
                eprintln!("frame_invalid: {e}");
                return FRAME_EXIT;
            }
        };
        count += 1;
        log_line(
            faults,
            &format!(
                "predict {} {}",
                request.request_id, request.candidate_sha256
            ),
        );
        if request.request_id <= last_id {
            refuse_serving(
                "request_out_of_order",
                format!("request {} does not follow {last_id}", request.request_id),
            );
        }
        last_id = request.request_id;
        if request.candidate_sha256 != serving.candidate_sha256
            || request.model_function_sha256 != serving.model_function_sha256
        {
            refuse_serving(
                "identity_mismatch",
                "the request names another candidate or model function than this worker loaded",
            );
        }
        let order = [
            request.option_ids[0].as_str(),
            request.option_ids[1].as_str(),
        ];
        if request.family != FAMILY
            || request.state.len() > STATE_MAX_BYTES
            || order[0] == order[1]
            || !order
                .iter()
                .all(|id| OPTIONS.iter().any(|def| def.id == *id))
        {
            refuse_serving(
                "input_invalid",
                "the family, state or options are not a valid input",
            );
        }
        let (ids, markers) = ipc::decode_predict_payload(&payload)
            .unwrap_or_else(|m| refuse_serving("input_invalid", m));
        let logits = backend
            .logits(&ids, markers)
            .unwrap_or_else(|r| refuse_serving(r.code, r.message));
        if !logits.iter().all(|v| v.is_finite()) {
            refuse_serving("nonfinite_logits", format!("logits {logits:?}"));
        }
        let p = super::eval::softmax(logits, serving.temperature);
        let decision = super::eval::decide(p, order, 0.0);
        let at = |id: &str| p[usize::from(order[1] == id)];
        let reply = PredictReply {
            v: PREDICT_PROTOCOL,
            request_id: request.request_id,
            candidate_sha256: serving.candidate_sha256.clone(),
            model_function_sha256: serving.model_function_sha256.clone(),
            input_sha256: crate::decision_model::input_sha256(&request.state, order),
            choice: order[decision.selected].to_owned(),
            probabilities: Probabilities {
                search: at("search"),
                graph: at("graph"),
            },
            answer_confidence: decision.confidence,
        };
        let header =
            serde_json::to_vec(&reply).unwrap_or_else(|e| refuse_serving("frame_invalid", e));
        let headers = match faults.serve_reply {
            Some(tamper) => tamper(count, header),
            None => vec![header],
        };
        for header in headers {
            if write_raw_frame(out, &header).is_err() {
                return FRAME_EXIT;
            }
        }
    }
}

/// Run one learning worker: verify ownership, arm the owner-death watcher,
/// take the frame channel private, then serve until owner EOF. Returns the
/// process exit code.
pub fn serve(args: WorkerArgs, backend: &mut dyn Backend) -> i32 {
    let ppid = unsafe { libc::getppid() } as u32;
    if ppid != args.owner_pid {
        eprintln!(
            "owner PID {} does not match parent {ppid}; refusing to run",
            args.owner_pid
        );
        return CONFIG_EXIT;
    }
    match worker_runtime::arm_owner_death_watcher(args.owner_pid, args.liveness_fd) {
        Ok(Armed::Watching) => {}
        Ok(Armed::OwnerGone) => worker_runtime::exit_now(OWNER_DEATH_EXIT),
        Err(message) => {
            eprintln!("cannot arm owner-death watcher: {message}");
            return WATCHER_EXIT;
        }
    }
    // Every head the worker reads or writes is a fixed name in the run
    // directory, opened relative to the working directory. Re-enter the
    // run directory the supervisor named (absolute, granted read-write) and
    // confirm the working directory IS it, so no file lands in, or is read
    // from, the sandbox container.
    if let Err(message) = enter_run_dir(&args.run_dir) {
        eprintln!("cannot enter the scratch run directory: {message}");
        return CONFIG_EXIT;
    }
    let out = match worker_runtime::take_ipc_stdout() {
        Ok(file) => file,
        Err(e) => {
            eprintln!("cannot take the frame channel private: {e}");
            return CONFIG_EXIT;
        }
    };
    let mut channel = Channel { out };
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let faults = args.faults;
    let mut last_id = 0u64;
    let mut current: Option<Identity> = None;
    let mut logits_calls = 0u64;
    loop {
        let (header, payload) = match read_frame_as::<LearnHeader, _>(&mut input) {
            Ok(frame) => frame,
            Err(FrameError::Eof) => return 0,
            Err(e) => {
                let identity = current.clone().unwrap_or(Identity {
                    model_function_sha256: String::new(),
                    head_sha256: None,
                    steps: 0,
                });
                let _ = channel.send(
                    0,
                    &identity,
                    Message::Error {
                        code: "frame_invalid".into(),
                        message: e.to_string(),
                    },
                );
                return FRAME_EXIT;
            }
        };
        let id = header.request_id;
        let identity = header.identity.clone();
        log_request(&faults, header.message.kind(), id);
        if id <= last_id {
            channel.refuse(
                id,
                &identity,
                Refusal::new(
                    "request_out_of_order",
                    format!("request {id} does not follow {last_id}"),
                ),
            );
        }
        last_id = id;
        if !matches!(header.message, Message::Load { .. } | Message::Serve { .. })
            && current.as_ref() != Some(&identity)
        {
            channel.refuse(
                id,
                &identity,
                Refusal::new(
                    "identity_mismatch",
                    "the request names another loaded identity than this worker's",
                ),
            );
        }
        // Every refusal answers this request, then ends the process.
        macro_rules! refused {
            ($refusal:expr) => {
                channel.refuse(id, &identity, $refusal)
            };
        }
        let decode = |markers: [u32; 2]| {
            ipc::decode_input(&payload, markers).map_err(|m| Refusal::new("input_invalid", m))
        };
        // The identity a reply echoes: the request's (the drift hook of the
        // fake worker misreports it on a `saved` reply).
        let mut echo = identity.clone();
        let reply = match header.message {
            Message::Load {
                checkpoint,
                head,
                threads,
                seed,
            } => {
                if identity.steps != 0 || head.is_some() != identity.head_sha256.is_some() {
                    refused!(Refusal::new(
                        "identity_mismatch",
                        "a load starts at step 0 of its head"
                    ));
                }
                let head_bytes = match head {
                    Some(slot) => {
                        let bytes = read_head(slot).unwrap_or_else(|r| refused!(r));
                        if identity.head_sha256.as_deref() != Some(sha256(&bytes).as_str()) {
                            refused!(Refusal::new(
                                "artifact_invalid",
                                format!("{} is not the named head", slot.file_name()),
                            ));
                        }
                        Some(bytes)
                    }
                    None => None,
                };
                let report = backend
                    .load(&args.checkpoint_dir, head_bytes.as_deref(), threads, seed)
                    .unwrap_or_else(|r| refused!(r));
                if let Err(refusal) = check_pin(&report, &checkpoint) {
                    refused!(refusal);
                }
                current = Some(identity.clone());
                Message::Loaded {
                    source_dtype: report.source_dtype,
                    weights_sha256: report.weights_sha256,
                    encoder_config_sha256: report.encoder_config_sha256,
                    counts: report.counts,
                    trainable: report.trainable,
                    frozen_encoder_sha256: report.frozen_encoder_sha256,
                }
            }
            Message::Serve {
                checkpoint,
                candidate_sha256,
                temperature,
                threads,
            } => {
                let Some(head) = identity.head_sha256.clone() else {
                    refused!(Refusal::new(
                        "identity_mismatch",
                        "a serve load names the candidate's head"
                    ));
                };
                if identity.steps != 0 {
                    refused!(Refusal::new(
                        "identity_mismatch",
                        "a serve load starts at step 0 of its head"
                    ));
                }
                if !super::hex64(&candidate_sha256)
                    || !super::hex64(&identity.model_function_sha256)
                    || !super::eval::valid_temperature(temperature)
                {
                    refused!(Refusal::new(
                        "input_invalid",
                        "a serve load needs a candidate and model digest and a grid temperature"
                    ));
                }
                let bytes = read_head(HeadSlot::Serve).unwrap_or_else(|r| refused!(r));
                if sha256(&bytes) != head {
                    refused!(Refusal::new(
                        "artifact_invalid",
                        format!("{} is not the named head", HeadSlot::Serve.file_name()),
                    ));
                }
                let report = backend
                    .load(&args.checkpoint_dir, Some(&bytes), threads, 0)
                    .unwrap_or_else(|r| refused!(r));
                if let Err(refusal) = check_pin(&report, &checkpoint) {
                    refused!(refusal);
                }
                let loaded = Message::Loaded {
                    source_dtype: report.source_dtype,
                    weights_sha256: report.weights_sha256,
                    encoder_config_sha256: report.encoder_config_sha256,
                    counts: report.counts,
                    trainable: report.trainable,
                    frozen_encoder_sha256: report.frozen_encoder_sha256,
                };
                if !channel.send(id, &identity, loaded) {
                    return FRAME_EXIT;
                }
                let serving = Serving {
                    candidate_sha256,
                    model_function_sha256: identity.model_function_sha256,
                    temperature,
                };
                return serve_predictions(
                    &mut channel.out,
                    &mut input,
                    backend,
                    &serving,
                    &faults,
                    id,
                );
            }
            Message::Step { markers, target } => {
                let (ids, at) = decode(markers).unwrap_or_else(|r| refused!(r));
                if target > 1 {
                    refused!(Refusal::new("input_invalid", "target must be 0 or 1"));
                }
                let (loss, norm) = backend
                    .step(&ids, at, target as usize)
                    .unwrap_or_else(|r| refused!(r));
                if !finite(&[loss]) {
                    refused!(Refusal::new("nonfinite_loss", format!("loss {loss}")));
                }
                if !finite(&[norm]) {
                    refused!(Refusal::new(
                        "nonfinite_gradient",
                        format!("gradient norm {norm}")
                    ));
                }
                if let Some(current) = current.as_mut() {
                    current.steps += 1;
                }
                Message::Stepped {
                    input_sha256: ipc::input_sha256(&ids, markers),
                    loss,
                    grad_norm_before_clip: norm,
                }
            }
            Message::Logits { markers } => {
                let (ids, at) = decode(markers).unwrap_or_else(|r| refused!(r));
                let values = backend.logits(&ids, at).unwrap_or_else(|r| refused!(r));
                if !values.iter().all(|v| v.is_finite()) {
                    refused!(Refusal::new(
                        "nonfinite_logits",
                        format!("logits {values:?}")
                    ));
                }
                logits_calls += 1;
                let message = Message::LogitsOut {
                    input_sha256: ipc::input_sha256(&ids, markers),
                    values,
                };
                if faults.duplicate_logits == Some(logits_calls) {
                    let _ = channel.send(id, &echo, message.clone());
                }
                if faults.stale_logits == Some(logits_calls) {
                    let _ = channel.send(id - 1, &echo, message);
                    continue;
                }
                message
            }
            Message::Save { markers } => {
                let (ids, at) = decode(markers).unwrap_or_else(|r| refused!(r));
                let bytes = backend.head_bytes().unwrap_or_else(|r| refused!(r));
                let written = write_head(&bytes).unwrap_or_else(|r| refused!(r));
                let (diff, values) = backend
                    .reload(&written, &ids, at)
                    .unwrap_or_else(|r| refused!(r));
                if !finite(&[diff]) || !values.iter().all(|v| v.is_finite()) {
                    refused!(Refusal::new(
                        "nonfinite_weight",
                        "the reloaded head is not finite"
                    ));
                }
                if faults.drift_after_save {
                    echo.steps += 1;
                }
                let message = Message::Saved {
                    input_sha256: ipc::input_sha256(&ids, markers),
                    sha256: sha256(&written),
                    bytes: written.len() as u64,
                    reload_max_abs_diff: diff,
                    values,
                };
                if faults.duplicate_saved {
                    let _ = channel.send(id, &echo, message.clone());
                }
                message
            }
            Message::FrozenHash => {
                let sha = backend.frozen_hash().unwrap_or_else(|r| refused!(r));
                Message::FrozenHashOut { sha256: sha }
            }
            other => refused!(Refusal::new(
                "frame_invalid",
                format!("unexpected {} frame on worker input", other.kind()),
            )),
        };
        if !channel.send(id, &echo, reply) {
            return FRAME_EXIT;
        }
    }
}

/// The load must be exactly the checkpoint the owner pinned.
fn check_pin(report: &LoadReport, pin: &CheckpointPin) -> Result<(), Refusal> {
    if report.weights_sha256 != pin.weights_sha256
        || report.encoder_config_sha256 != pin.encoder_config_sha256
        || report.source_dtype != pin.source_dtype
    {
        return Err(Refusal::new(
            "checkpoint_invalid",
            format!(
                "the checkpoint is {} / {} / {}, the owner pinned {} / {} / {}",
                report.weights_sha256,
                report.encoder_config_sha256,
                report.source_dtype,
                pin.weights_sha256,
                pin.encoder_config_sha256,
                pin.source_dtype
            ),
        ));
    }
    Ok(())
}
