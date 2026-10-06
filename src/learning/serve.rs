//! 013 T003 owner side of the serving worker (macOS): the SAME `foundry-learn`
//! executable T002 trains with, launched through T002's supervised spawn
//! ([`super::supervisor::spawn`]: bundle and executable digest, scratch run
//! directory, pre-exec limits with zero descendants, the bounded stderr
//! drain, the fail-closed footprint and output monitor), loaded once with the
//! selected candidate's head (`serve`, evaluation mode, its fitted
//! temperature) within [`LOAD_CEILING`], then serving IPC-v2 predictions.
//!
//! Slot rules (contract § Serving, selection and rollback):
//! * one active prediction and zero waiting: a request that finds the slot
//!   occupied is [`Refused::Busy`] BEFORE anything is dispatched;
//! * each prediction waits at most min([`PREDICT_CEILING`], HALF the time
//!   left to the request's read deadline), so a cut-off prediction always
//!   leaves the deterministic fallback at least as much time as it took
//!   (009's rule); a wait below [`PREDICT_FLOOR`] is
//!   [`Refused::InsufficientTime`] without a dispatch;
//! * a timed-out request falls back alone, but its slot stays occupied until
//!   the worker's late reply actually arrives (it is validated and then
//!   discarded, never delivered) or the worker is terminated, so later
//!   requests see busy;
//! * ONE linearization point per request decides success versus timeout:
//!   its completion state, changed only under the slot mutex. The reader's
//!   publication stamps its instant INSIDE that critical section; a reply
//!   is within the ceiling when it is published to the core's per-request
//!   state before the ceiling (reading bytes earlier does not count: the
//!   core cannot act on a reply it has not received). The waiter expires
//!   only by winning the same arbitration at or after the ceiling. Exactly
//!   one of {on-time success, timeout} happens per request; a late
//!   publication only frees the slot. Only an on-time reply resets the
//!   consecutive-timeout count; busy and the other fallbacks neither count
//!   nor reset it; [`TIMEOUT_LIMIT`] consecutive timeouts terminate the
//!   worker;
//! * an unsolicited, duplicate, malformed or wrong-identity reply, and a
//!   worker that dies, are terminal the same way: the owned worker is
//!   terminated and the policy stays unavailable until the owner restarts.
//!   There is no restart loop.
//!
//! Every reply is validated by the core ([`crate::policy::validate_reply`])
//! before anything is stored; the worker cannot declare a route accepted.
use super::ipc::{
    self, HeadSlot, Identity, LEARN_PROTOCOL, LearnHeader, Message, PREDICT_PROTOCOL, PredictReply,
    PredictRequest,
};
use super::profile::LearnProfile;
use super::supervisor::{self, Breach, Request, Spawned};
use crate::control::Control;
use crate::decision_model::{self, CheckpointPin, Rendered};
use crate::error::{FResult, FoundryError};
use crate::neural::anchor::Dir;
use crate::neural::protocol::{FrameError, read_frame_as, write_frame_as};
use crate::neural::supervisor::terminate_and_reap;
use crate::policy::{Expected, Prediction, validate_reply};
use std::io::{BufReader, Write as _};
use std::os::fd::OwnedFd;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, mpsc};
use std::time::{Duration, Instant};

/// The serving load ceiling, outside every request (contract 261). A profile
/// with a shorter `load_timeout_seconds` bounds it further.
pub const LOAD_CEILING: Duration = Duration::from_secs(30);
/// The per-prediction ceiling before the read deadline cuts it shorter.
pub const PREDICT_CEILING: Duration = Duration::from_millis(2000);
/// The shortest prediction wait worth dispatching: below it the request is
/// routed deterministically without a model call.
pub const PREDICT_FLOOR: Duration = Duration::from_millis(50);
/// Consecutive prediction timeouts that terminate the worker.
pub const TIMEOUT_LIMIT: u32 = 3;
const WAIT_SLICE: Duration = Duration::from_millis(50);

/// Why one request is routed deterministically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The one slot is occupied; nothing was dispatched.
    Busy,
    /// The ceiling passed before a reply was read.
    Timeout,
    /// Half the time left to the read deadline is below [`PREDICT_FLOOR`];
    /// nothing was dispatched.
    InsufficientTime,
    /// The worker is terminated: the policy is unavailable until restart.
    Unavailable,
    /// The request does not fit the frame caps; nothing was dispatched.
    Oversize,
}

/// The prediction wait for `remaining` time to the read deadline:
/// min([`PREDICT_CEILING`], half of it), or `None` below [`PREDICT_FLOOR`].
pub fn prediction_wait(remaining: Duration) -> Option<Duration> {
    let wait = PREDICT_CEILING.min(remaining / 2);
    (wait >= PREDICT_FLOOR).then_some(wait)
}

/// Tests only: how the waiter of a prediction made on this thread waits.
#[cfg(feature = "test-faults")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestWait {
    /// Production: wake on publication, expire at the ceiling.
    Normal,
    /// Sleep past the ceiling WITHOUT looking, then arbitrate.
    PastCeiling,
    /// Wait for the publication however long it takes, then arbitrate.
    UntilPublished,
}

#[cfg(feature = "test-faults")]
thread_local! {
    static TEST_WAIT: std::cell::Cell<TestWait> = const { std::cell::Cell::new(TestWait::Normal) };
    static TEST_HOLD_PUBLICATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Tests only: the waiting mode of predictions this thread makes.
#[cfg(feature = "test-faults")]
pub fn set_test_wait(wait: TestWait) {
    TEST_WAIT.with(|cell| cell.set(wait));
}

/// Tests only: workers this thread starts hold each reply, once read, until
/// its waiter has arbitrated (the reader paused between reading and
/// publishing).
#[cfg(feature = "test-faults")]
pub fn set_test_hold_publication(hold: bool) {
    TEST_HOLD_PUBLICATION.with(|cell| cell.set(hold));
}

#[cfg(feature = "test-faults")]
fn test_wait() -> TestWait {
    TEST_WAIT.with(std::cell::Cell::get)
}

fn test_hold_publication() -> bool {
    #[cfg(feature = "test-faults")]
    {
        TEST_HOLD_PUBLICATION.with(std::cell::Cell::get)
    }
    #[cfg(not(feature = "test-faults"))]
    {
        false
    }
}

/// What a start needs: the verified profile, candidate and identities.
pub struct ServeLaunch<'a> {
    pub profile: &'a LearnProfile,
    pub checkpoint: &'a CheckpointPin,
    pub model_function_sha256: &'a str,
    pub candidate_sha256: &'a str,
    pub temperature: f64,
    /// The selected candidate's directory, held by descriptor since its
    /// read-back.
    pub candidate: &'a Dir,
    /// The head digest the candidate's manifest binds.
    pub head_sha256: &'a str,
    /// The fake worker's test hooks; production launches pass none.
    pub extra_args: Vec<String>,
}

/// A dispatched prediction's completion: the ONE arbiter of its outcome,
/// changed only under the slot mutex.
enum Completion {
    /// Neither published nor expired.
    Pending,
    /// Published before the ceiling: an on-time success.
    Replied(Prediction),
    /// Published at or after the ceiling before the waiter arbitrated: the
    /// work has ended; the waiter counts the timeout and frees the slot.
    Late,
    /// The waiter arbitrated a timeout while the work runs; its later
    /// publication only frees the slot.
    Expired,
}

/// The one dispatched prediction.
struct Outstanding {
    request_id: u64,
    input_sha256: String,
    /// A publication at or after this instant is late.
    ceiling: Instant,
    completion: Completion,
}

struct Slot {
    next_id: u64,
    outstanding: Option<Outstanding>,
    consecutive_timeouts: u32,
    /// Set once: why the worker was terminated. Nothing is dispatched or
    /// consumed afterwards.
    terminal: Option<(&'static str, String)>,
}

struct Shared {
    slot: Mutex<Slot>,
    changed: Condvar,
    /// Tests only: the reader holds a read reply until its waiter has
    /// arbitrated.
    hold_publication: bool,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Slot> {
        self.slot.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// What status reports about the worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerState {
    pub consecutive_timeouts: u32,
    /// A prediction (possibly a timed-out one) still occupies the slot.
    pub busy: bool,
    pub terminal: Option<(&'static str, String)>,
}

/// The resident serving worker of one owner (MCP) or one command (CLI).
pub struct PolicyWorker {
    shared: Arc<Shared>,
    child: Arc<Mutex<Option<Child>>>,
    candidate_sha256: String,
    model_function_sha256: String,
    /// Requests to the writer thread, which owns the worker's stdin;
    /// dropping it is owner EOF.
    requests: Option<mpsc::Sender<Vec<u8>>>,
    handles: Vec<std::thread::JoinHandle<()>>,
    stopping: Arc<AtomicBool>,
    liveness: Option<OwnedFd>,
    run: Option<PathBuf>,
    stderr_tail: Arc<Mutex<Vec<u8>>>,
    breach: Breach,
    stopped: bool,
}

/// Mark the worker terminal and end it, without blocking the caller: the
/// TERM → grace → KILL → reap sequence runs on its own thread.
fn terminate(
    slot: &mut Slot,
    child: &Arc<Mutex<Option<Child>>>,
    code: &'static str,
    message: String,
) {
    if slot.terminal.is_some() {
        return;
    }
    slot.terminal = Some((code, message));
    let child = Arc::clone(child);
    std::thread::spawn(move || {
        terminate_and_reap(&child);
    });
}

fn excerpt(tail: &Mutex<Vec<u8>>) -> String {
    let tail = tail.lock().unwrap_or_else(|p| p.into_inner());
    String::from_utf8_lossy(&tail).chars().take(512).collect()
}

/// The named stop a monitor breach imposed, else the observed failure with
/// the worker's bounded stderr.
fn named(
    breach: &Breach,
    tail: &Mutex<Vec<u8>>,
    code: &'static str,
    message: String,
) -> (&'static str, String) {
    if let Some(stop) = breach.lock().unwrap_or_else(|p| p.into_inner()).clone() {
        return stop;
    }
    let excerpt = excerpt(tail);
    if excerpt.trim().is_empty() {
        (code, message)
    } else {
        (
            code,
            format!("{message}; worker stderr: {}", excerpt.trim()),
        )
    }
}

/// Publish one reply to its request's completion state; called with the
/// slot mutex held, which makes this the request's linearization point.
/// The reply must answer exactly the outstanding request, once, from the
/// identity this owner loaded, with a valid distribution. Its publication
/// instant is taken HERE: before the ceiling it is an on-time reply; at or
/// after it, late (the waiter counts the timeout); after the waiter
/// expired the request, it only frees the slot.
fn publish(
    slot: &mut Slot,
    reply: &PredictReply,
    candidate_sha256: &str,
    model_function_sha256: &str,
) -> Result<(), (&'static str, String)> {
    let Some(outstanding) = slot.outstanding.as_mut() else {
        return Err((
            "reply_invalid",
            format!("an unsolicited reply for request {}", reply.request_id),
        ));
    };
    let answered = matches!(
        outstanding.completion,
        Completion::Replied(_) | Completion::Late
    );
    if reply.request_id != outstanding.request_id || answered {
        return Err((
            "reply_invalid",
            format!(
                "a reply for request {} arrived while {} was outstanding{}",
                reply.request_id,
                outstanding.request_id,
                if answered {
                    " and already answered"
                } else {
                    ""
                }
            ),
        ));
    }
    let prediction = validate_reply(
        reply,
        &Expected {
            candidate_sha256,
            model_function_sha256,
            input_sha256: &outstanding.input_sha256,
        },
    )?;
    let published = Instant::now();
    match outstanding.completion {
        Completion::Expired => slot.outstanding = None,
        _ if published < outstanding.ceiling => {
            outstanding.completion = Completion::Replied(prediction);
        }
        _ => outstanding.completion = Completion::Late,
    }
    Ok(())
}

impl PolicyWorker {
    /// Launch the worker, stage the candidate's head, and load it with
    /// `serve` within the load ceiling. Any failure stops and reaps the
    /// worker before it is reported; nothing serves.
    pub fn start(launch: ServeLaunch<'_>, control: &Control) -> FResult<Self> {
        let ceilings = &launch.profile.ceilings;
        let Spawned {
            child,
            stdin,
            stdout,
            stderr_tail,
            breach,
            stopping,
            staged,
            handles,
            liveness,
            run,
            scratch,
            ..
        } = supervisor::spawn(
            launch.profile,
            Request {
                threads: ceilings.cpu_threads,
                cpu_seconds: None,
                memory_bytes: ceilings.memory_bytes,
                output_bytes: ceilings.output_bytes,
                extra_args: launch.extra_args,
            },
            control,
        )?;
        let mut worker = Self {
            shared: Arc::new(Shared {
                slot: Mutex::new(Slot {
                    // The load is request 1; predictions follow it.
                    next_id: 2,
                    outstanding: None,
                    consecutive_timeouts: 0,
                    terminal: None,
                }),
                changed: Condvar::new(),
                hold_publication: test_hold_publication(),
            }),
            child,
            candidate_sha256: launch.candidate_sha256.to_owned(),
            model_function_sha256: launch.model_function_sha256.to_owned(),
            requests: None,
            handles,
            stopping,
            liveness: Some(liveness),
            run: Some(run),
            stderr_tail,
            breach,
            stopped: false,
        };
        // The head is the core's input, not worker output: reserved before
        // the copy, and checked against the manifest's digest.
        let bytes = launch
            .candidate
            .open_file(super::candidate::HEAD)
            .and_then(|file| file.metadata())
            .map_err(|e| worker.failure("artifact_invalid", format!("candidate head: {e}")))?
            .len();
        staged.fetch_add(bytes, Ordering::SeqCst);
        let copied = super::candidate::copy_head_into(
            launch.candidate,
            &scratch,
            HeadSlot::Serve.file_name(),
        )?;
        if copied != launch.head_sha256 {
            return Err(worker.failure(
                "artifact_invalid",
                "the candidate's head changed while it was staged".into(),
            ));
        }
        drop(scratch);
        let identity = Identity {
            model_function_sha256: launch.model_function_sha256.to_owned(),
            head_sha256: Some(copied),
            steps: 0,
        };
        let load = LearnHeader {
            protocol: LEARN_PROTOCOL,
            request_id: 1,
            identity: identity.clone(),
            message: Message::Serve {
                checkpoint: launch.checkpoint.clone(),
                candidate_sha256: launch.candidate_sha256.to_owned(),
                temperature: launch.temperature,
                threads: ceilings.cpu_threads,
            },
        };
        let (Some(mut stdin), Some(stdout)) = (stdin, stdout) else {
            return Err(worker.failure("worker_failed", "the worker has no frame channel".into()));
        };
        if write_frame_as(&mut stdin, &load, &[]).is_err() {
            return Err(worker.failure(
                "worker_failed",
                "the serve load could not be written to the worker".into(),
            ));
        }
        let (loaded_tx, loaded_rx) = mpsc::channel();
        let reader = worker.reader(stdout, loaded_tx);
        worker.handles.push(reader);
        let (requests, queue) = mpsc::channel::<Vec<u8>>();
        worker
            .handles
            .push(std::thread::spawn(move || write_requests(stdin, queue)));
        worker.requests = Some(requests);
        let ceiling = LOAD_CEILING.min(Duration::from_secs(launch.profile.load_timeout_seconds));
        let bound = Instant::now() + ceiling;
        let first = loop {
            control.check()?;
            match loaded_rx.recv_timeout(WAIT_SLICE) {
                Ok(first) => break first,
                Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() < bound => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(worker.failure(
                        "worker_timeout",
                        format!("the worker did not load within {} s", ceiling.as_secs()),
                    ));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(
                        worker.failure("worker_failed", "the worker's reply channel closed".into())
                    );
                }
            }
        };
        let (reply, payload) = first.map_err(|why| worker.failure("worker_failed", why))?;
        worker.check_loaded(&reply, &payload, &identity, launch.checkpoint)?;
        Ok(worker)
    }

    /// The loaded reply: request 1, this identity, no payload, the pinned
    /// checkpoint and the contract's trainable set.
    fn check_loaded(
        &mut self,
        reply: &LearnHeader,
        payload: &[u8],
        identity: &Identity,
        pin: &CheckpointPin,
    ) -> FResult<()> {
        if reply.request_id != 1 || reply.identity != *identity || !payload.is_empty() {
            return Err(self.failure(
                "worker_failed",
                "the load reply does not answer the serve load".into(),
            ));
        }
        let (source_dtype, weights_sha256, encoder_config_sha256, counts, trainable) =
            match &reply.message {
                Message::Loaded {
                    source_dtype,
                    weights_sha256,
                    encoder_config_sha256,
                    counts,
                    trainable,
                    ..
                } => (
                    source_dtype,
                    weights_sha256,
                    encoder_config_sha256,
                    counts,
                    trainable,
                ),
                Message::Error { code, message } => {
                    let named = supervisor::worker_code(code);
                    return Err(self.failure(named, format!("worker {code}: {message}")));
                }
                other => {
                    return Err(self.failure(
                        "worker_failed",
                        format!("a {} reply answered the serve load", other.kind()),
                    ));
                }
            };
        if *source_dtype != pin.source_dtype
            || *weights_sha256 != pin.weights_sha256
            || *encoder_config_sha256 != pin.encoder_config_sha256
        {
            return Err(self.failure(
                "checkpoint_invalid",
                "the worker loaded another checkpoint than the candidate pins".into(),
            ));
        }
        let set = decision_model::trainable();
        let names: Vec<&str> = set.iter().map(|(name, _)| name.as_str()).collect();
        let elements: u64 = set
            .iter()
            .map(|(_, shape)| shape.iter().product::<usize>() as u64)
            .sum();
        if *trainable != names || counts.trainable != elements {
            return Err(self.failure(
                "worker_failed",
                "the worker's trainable set is not the contract's".into(),
            ));
        }
        Ok(())
    }

    /// The reader thread: the load reply first, then every prediction
    /// reply, each validated against the slot under its lock. Anything
    /// invalid, and the end of the worker's output, is terminal.
    fn reader(
        &self,
        stdout: ChildStdout,
        loaded: mpsc::Sender<Result<(LearnHeader, Vec<u8>), String>>,
    ) -> std::thread::JoinHandle<()> {
        let shared = Arc::clone(&self.shared);
        let child = Arc::clone(&self.child);
        let breach = Arc::clone(&self.breach);
        let tail = Arc::clone(&self.stderr_tail);
        let candidate = self.candidate_sha256.clone();
        let function = self.model_function_sha256.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let first = read_frame_as::<LearnHeader, _>(&mut reader).map_err(|e| match e {
                FrameError::Eof => "the worker exited before it loaded".to_owned(),
                other => format!("worker IPC: {other}"),
            });
            let ok = first.is_ok();
            let _ = loaded.send(first);
            if !ok {
                return;
            }
            loop {
                let frame = read_frame_as::<PredictReply, _>(&mut reader);
                let mut slot = shared.lock();
                if shared.hold_publication && frame.is_ok() {
                    // Tests only: paused between reading and publishing
                    // until the waiter has arbitrated.
                    slot = shared
                        .changed
                        .wait_while(slot, |s| {
                            s.terminal.is_none()
                                && s.outstanding
                                    .as_ref()
                                    .is_some_and(|o| matches!(o.completion, Completion::Pending))
                        })
                        .unwrap_or_else(|p| p.into_inner());
                }
                if slot.terminal.is_some() {
                    // Nothing a terminated worker sends is consumed.
                    break;
                }
                let verdict = match frame {
                    Ok((reply, _)) => publish(&mut slot, &reply, &candidate, &function),
                    Err(FrameError::Eof) => Err(named(
                        &breach,
                        &tail,
                        "worker_failed",
                        "the serving worker exited".into(),
                    )),
                    Err(e) => Err(("reply_invalid", format!("worker IPC: {e}"))),
                };
                if let Err((code, message)) = verdict {
                    terminate(&mut slot, &child, code, message);
                }
                shared.changed.notify_all();
                if slot.terminal.is_some() {
                    break;
                }
            }
        })
    }

    /// Stop the worker, then name the failure (a monitor breach wins).
    fn failure(&mut self, code: &'static str, message: String) -> FoundryError {
        self.shutdown();
        let (code, message) = named(&self.breach, &self.stderr_tail, code, message);
        FoundryError::Learning { code, message }
    }

    /// One prediction for `state` with `option_ids` in that order, rendered
    /// by the core's preflight as `rendered`, under the request's read
    /// `deadline`. `Ok` is a reply read within its ceiling that passed the
    /// core's validation; the caller applies its threshold.
    pub fn predict(
        &self,
        state: &str,
        option_ids: [&str; 2],
        rendered: &Rendered,
        deadline: Instant,
    ) -> Result<Prediction, Refused> {
        let mut slot = self.shared.lock();
        if slot.terminal.is_some() {
            return Err(Refused::Unavailable);
        }
        if slot.outstanding.is_some() {
            return Err(Refused::Busy);
        }
        let now = Instant::now();
        let Some(wait) = prediction_wait(deadline.saturating_duration_since(now)) else {
            return Err(Refused::InsufficientTime);
        };
        let ceiling = now + wait;
        let request_id = slot.next_id;
        let request = PredictRequest {
            v: PREDICT_PROTOCOL,
            request_id,
            candidate_sha256: self.candidate_sha256.clone(),
            model_function_sha256: self.model_function_sha256.clone(),
            family: decision_model::FAMILY.to_owned(),
            state: state.to_owned(),
            option_ids: [option_ids[0].to_owned(), option_ids[1].to_owned()],
        };
        let markers = [rendered.markers[0] as u32, rendered.markers[1] as u32];
        let payload = ipc::encode_predict_payload(&rendered.ids, markers);
        let mut frame = Vec::new();
        if write_frame_as(&mut frame, &request, &payload).is_err() {
            // Over the 64 KiB header cap (or the payload cap): nothing sent.
            return Err(Refused::Oversize);
        }
        slot.next_id += 1;
        let sent = self
            .requests
            .as_ref()
            .is_some_and(|requests| requests.send(frame).is_ok());
        if !sent {
            terminate(
                &mut slot,
                &self.child,
                "worker_failed",
                "the request channel to the worker is closed".into(),
            );
            self.shared.changed.notify_all();
            return Err(Refused::Unavailable);
        }
        slot.outstanding = Some(Outstanding {
            request_id,
            input_sha256: decision_model::input_sha256(state, option_ids),
            ceiling,
            completion: Completion::Pending,
        });
        let pending = |s: &Slot| {
            s.terminal.is_none()
                && s.outstanding.as_ref().is_some_and(|o| {
                    o.request_id == request_id && matches!(o.completion, Completion::Pending)
                })
        };
        // Tests only: a forced waiting order; production always waits
        // normally.
        #[cfg(feature = "test-faults")]
        let (slot, normal) = match test_wait() {
            TestWait::Normal => (slot, true),
            TestWait::PastCeiling => {
                // Look only once the ceiling has passed.
                drop(slot);
                std::thread::sleep(
                    (ceiling + Duration::from_millis(100))
                        .saturating_duration_since(Instant::now()),
                );
                (self.shared.lock(), false)
            }
            TestWait::UntilPublished => (
                self.shared
                    .changed
                    .wait_while(slot, |s| pending(s))
                    .unwrap_or_else(|p| p.into_inner()),
                false,
            ),
        };
        #[cfg(not(feature = "test-faults"))]
        let normal = true;
        let mut slot = slot;
        // Wake on publication; expire only at or after the ceiling.
        while normal && pending(&slot) && Instant::now() < ceiling {
            let left = ceiling.saturating_duration_since(Instant::now());
            slot = self
                .shared
                .changed
                .wait_timeout(slot, left)
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
        // Nothing a terminated worker sent is consumed.
        if slot.terminal.is_some() {
            return Err(Refused::Unavailable);
        }
        // The arbitration, under the same mutex as every publication.
        let mine = slot
            .outstanding
            .as_mut()
            .filter(|o| o.request_id == request_id);
        match mine.map(|o| std::mem::replace(&mut o.completion, Completion::Expired)) {
            Some(Completion::Replied(prediction)) => {
                slot.outstanding = None;
                slot.consecutive_timeouts = 0;
                return Ok(prediction);
            }
            // Published late: the work has ended, so the slot is free.
            Some(Completion::Late) => slot.outstanding = None,
            // Not published by the ceiling: expired here; the work keeps the
            // slot until its publication (discarded) or the worker ends.
            Some(Completion::Pending) | Some(Completion::Expired) | None => {}
        }
        self.shared.changed.notify_all();
        slot.consecutive_timeouts += 1;
        if slot.consecutive_timeouts >= TIMEOUT_LIMIT {
            terminate(
                &mut slot,
                &self.child,
                "prediction_timeouts",
                format!("{TIMEOUT_LIMIT} consecutive predictions exceeded their ceiling"),
            );
            self.shared.changed.notify_all();
        }
        Err(Refused::Timeout)
    }

    pub fn state(&self) -> WorkerState {
        let slot = self.shared.lock();
        WorkerState {
            consecutive_timeouts: slot.consecutive_timeouts,
            busy: slot.outstanding.is_some(),
            terminal: slot.terminal.clone(),
        }
    }

    /// Owner EOF (the request channel closes, so the writer drops the
    /// worker's stdin), then TERM → 5 s → KILL → reap; join the helper
    /// threads; remove the scratch run directory.
    pub fn shutdown(&mut self) {
        if self.stopped {
            return;
        }
        self.requests = None;
        {
            let mut slot = self.shared.lock();
            if slot.terminal.is_none() {
                slot.terminal = Some(("stopped", "the owner stopped the policy worker".into()));
            }
            self.shared.changed.notify_all();
        }
        terminate_and_reap(&self.child);
        self.liveness = None;
        self.stopping.store(true, Ordering::SeqCst);
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
        self.stopped = true;
        if let Some(run) = self.run.take() {
            let _ = std::fs::remove_dir_all(run);
        }
    }
}

impl Drop for PolicyWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// The writer thread: one frame per request, in order. A failed write ends
/// it; the reader then sees the worker's death.
fn write_requests(mut stdin: ChildStdin, queue: mpsc::Receiver<Vec<u8>>) {
    while let Ok(frame) = queue.recv() {
        if stdin
            .write_all(&frame)
            .and_then(|()| stdin.flush())
            .is_err()
        {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wait_leaves_the_fallback_half_the_remaining_time_or_is_skipped() {
        let ms = Duration::from_millis;
        assert_eq!(prediction_wait(ms(5000)), Some(PREDICT_CEILING));
        assert_eq!(prediction_wait(ms(4000)), Some(PREDICT_CEILING));
        assert_eq!(prediction_wait(ms(800)), Some(ms(400)));
        assert_eq!(prediction_wait(ms(100)), Some(PREDICT_FLOOR));
        assert_eq!(
            prediction_wait(ms(99)),
            None,
            "below the floor: no model call"
        );
        assert_eq!(prediction_wait(Duration::ZERO), None);
    }
}
