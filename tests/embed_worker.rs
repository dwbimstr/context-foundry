//! 009 supervised embedding worker tests: admission (the one slot belongs
//! to the worker and outlives client timeouts), frame validation in both
//! directions, the stderr bound, owner-death containment on both signals,
//! TERM/5 s/KILL shutdown with no replacement before disappearance, the
//! supervised memory ceiling, and admission refusal without the development
//! flag or with a mismatched profile. All use `foundry-embed-fake`; the
//! real-model development run is the `#[ignore]` tests at the bottom.
#![cfg(target_os = "macos")]

use context_foundry::neural::profile::{SemanticProfile, WorkerSpec};
use context_foundry::neural::protocol::{self, Header, Purpose};
use context_foundry::neural::provider::{EmbeddingProvider, ProviderError, TokenizedInput};
use context_foundry::neural::supervisor::{self, WorkerProvider};
use context_foundry::neural::worker_runtime::{self, fake_descriptor};
use std::io::{BufReader, Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// The fake descriptor's output dimension.
const DIMENSIONS: usize = 768;

fn fake_exe() -> &'static str {
    env!("CARGO_BIN_EXE_foundry-embed-fake")
}

/// A liveness pipe: the reader (keep it open until after the spawn), its
/// descriptor number for the worker's argv, and the write end the owner
/// holds. macOS has no `pipe2`; `std::io::pipe` closes both ends on exec.
fn liveness_pipe() -> (std::io::PipeReader, i32, OwnedFd) {
    let (reader, writer) = std::io::pipe().expect("liveness pipe");
    let read_fd = reader.as_raw_fd();
    (reader, read_fd, OwnedFd::from(writer))
}

/// `pre_exec` body: clear `FD_CLOEXEC` on `fd` so the exec'd worker
/// inherits it.
fn inherit(fd: i32) -> impl FnMut() -> std::io::Result<()> + Send + Sync + 'static {
    move || {
        if unsafe { libc::fcntl(fd, libc::F_SETFD, 0) } == -1 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
}

fn sha256_of(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(std::fs::read(path).expect("read file"));
    format!("{:x}", hasher.finalize())
}

/// A minimal unsigned .app bundle around the fake worker; the supervisor
/// resolves `<bundle>/Contents/MacOS/foundry-embed` and verifies its hash.
fn fake_bundle(dir: &Path) -> (PathBuf, String) {
    let app = dir.join("FoundryEmbedFake.app");
    let macos = app.join("Contents/MacOS");
    std::fs::create_dir_all(&macos).expect("create bundle");
    let exe = macos.join("foundry-embed");
    std::fs::copy(fake_exe(), &exe).expect("copy fake worker");
    std::fs::write(
        app.join("Contents/Info.plist"),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\
         <key>CFBundleIdentifier</key><string>org.context-foundry.embed.fake</string>\
         <key>CFBundleExecutable</key><string>foundry-embed</string>\
         <key>CFBundlePackageType</key><string>APPL</string></dict></plist>\n",
    )
    .expect("write Info.plist");
    let sha = sha256_of(&exe);
    (app, sha)
}

/// A v2 profile whose artifacts are the descriptor's files, all empty
/// (their digests are pinned in the fake descriptor), whose worker is
/// `bundle`, and whose scratch root is `dir/scratch-root`. The root is NOT
/// created: the supervisor creates it owner-private on first use.
fn fake_profile(
    dir: &Path,
    bundle: &Path,
    exe_sha: &str,
    ceiling: u64,
    load_timeout: u64,
) -> SemanticProfile {
    let model_dir = dir.join("model");
    std::fs::create_dir_all(&model_dir).expect("create model dir");
    let descriptor = fake_descriptor();
    for file in &descriptor.artifact_files {
        std::fs::write(model_dir.join(&file.name), b"").expect("artifact file");
    }
    SemanticProfile {
        v: context_foundry::neural::profile::PROFILE_VERSION,
        name: "fake-worker".into(),
        model_dir,
        worker: WorkerSpec {
            bundle: bundle.to_path_buf(),
            executable_sha256: exe_sha.into(),
            scratch_root: dir.join("scratch-root"),
        },
        descriptor,
        query_template: "query: {text}".into(),
        card_tokens: context_foundry::neural::profile::DEFAULT_CARD_TOKENS,
        batch: context_foundry::neural::profile::DEFAULT_BATCH,
        memory_ceiling_bytes: ceiling,
        load_timeout_seconds: load_timeout,
    }
}

fn launch(profile: &SemanticProfile, hooks: &[&str]) -> Result<WorkerProvider, ProviderError> {
    let extra: Vec<String> = hooks.iter().map(|h| h.to_string()).collect();
    WorkerProvider::launch(profile, extra)
}

/// The error of a call that must be refused. `WorkerProvider` and
/// `dyn EmbeddingProvider` are deliberately not `Debug`, so `expect_err`
/// is unavailable for launch results.
fn refused<T>(result: Result<T, ProviderError>, what: &str) -> ProviderError {
    match result {
        Ok(_) => panic!("{what}: the call must be refused"),
        Err(error) => error,
    }
}

fn input(ids: &[u32]) -> TokenizedInput {
    TokenizedInput { ids: ids.to_vec() }
}

/// A manually spawned fake worker the test owns (the test PID is the
/// worker's parent), with a liveness pipe whose write end the test holds.
struct ManualWorker {
    child: Child,
    liveness_write: OwnedFd,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

fn spawn_fake(hooks: &[&str]) -> ManualWorker {
    let dir = tempfile::tempdir().expect("tempdir");
    let (reader, read_fd, liveness_write) = liveness_pipe();
    let descriptor = serde_json::to_string(&fake_descriptor()).expect("descriptor JSON");
    let mut command = Command::new(fake_exe());
    command
        .arg("--owner-pid")
        .arg(std::process::id().to_string())
        .arg("--liveness-fd")
        .arg(read_fd.to_string())
        .arg("--descriptor")
        .arg(&descriptor)
        .arg("--model-dir")
        .arg(dir.path())
        .args(hooks)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: `inherit` runs one async-signal-safe `fcntl` between fork and
    // exec.
    unsafe { command.pre_exec(inherit(read_fd)) };
    let mut child = command.spawn().expect("spawn fake worker");
    drop(reader);
    let stdin = child.stdin.take().expect("stdin");
    let stdout = BufReader::new(child.stdout.take().expect("stdout"));
    ManualWorker {
        child,
        liveness_write,
        stdin,
        stdout,
    }
}

impl ManualWorker {
    fn send(&mut self, header: &Header, payload: &[u8]) {
        protocol::write_frame(&mut self.stdin, header, payload).expect("send frame");
    }

    fn hello(&mut self) {
        self.send(
            &Header::Hello {
                protocol: protocol::PROTOCOL_VERSION,
            },
            &[],
        );
    }

    fn recv(&mut self) -> (Header, Vec<u8>) {
        protocol::read_frame(&mut self.stdout).expect("receive frame")
    }

    fn expect_ready(&mut self) {
        match self.recv() {
            (Header::Ready { .. }, _) => {}
            other => panic!("expected ready, got {other:?}"),
        }
    }

    fn embed(&mut self, id: u64, digest: &str, purpose: Purpose, inputs: &[TokenizedInput]) {
        let (lengths, payload) = protocol::encode_ids(inputs);
        self.send(
            &Header::Embed {
                protocol: protocol::PROTOCOL_VERSION,
                id,
                descriptor_digest: digest.into(),
                purpose,
                lengths,
            },
            &payload,
        );
    }

    fn write_raw(&mut self, bytes: &[u8]) {
        self.stdin.write_all(bytes).expect("raw write");
        self.stdin.flush().expect("raw flush");
    }
}

fn pid_gone(pid: u32, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    loop {
        let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
        if rc == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn acquire_embeds_deterministic_documents_and_queries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let mut provider =
        supervisor::acquire_until(&profile, true, &context_foundry::Control::unbounded())
            .expect("acquire");
    let batch: Vec<TokenizedInput> = [vec![1, 2, 3], vec![4], vec![5, 6], vec![7, 8, 9, 10, 11]]
        .iter()
        .map(|ids| input(ids))
        .collect();
    let vectors = provider
        .embed_documents(&batch, &context_foundry::Control::unbounded())
        .expect("embed documents");
    assert_eq!(vectors.len(), batch.len());
    for (vector, tokenized) in vectors.iter().zip(&batch) {
        assert_eq!(
            vector,
            &worker_runtime::deterministic_vector(&tokenized.ids, DIMENSIONS)
        );
        assert_eq!(vector.len(), DIMENSIONS);
    }
    // Deterministic: the same IDs give bitwise-identical vectors.
    let again = provider
        .embed_documents(&batch, &context_foundry::Control::unbounded())
        .expect("embed again");
    assert_eq!(vectors, again);
    let query = provider
        .embed_query(
            &input(&[500, 501]),
            Instant::now() + Duration::from_secs(10),
        )
        .expect("embed query");
    assert_eq!(
        query,
        worker_runtime::deterministic_vector(&[500, 501], DIMENSIONS)
    );
}

#[test]
fn worker_admission_slot_refuses_second_embeds_and_frees_on_completion() {
    // Worker-level admission (spec: admission is enforced inside the
    // worker): a second embed while the slot is held gets `busy`
    // immediately, never queued; the slot frees only when the work truly
    // ends; a client that stops reading changes nothing.
    let mut worker = spawn_fake(&["--slow-ms", "2500"]);
    worker.hello();
    worker.expect_ready();
    let digest = fake_descriptor().digest();
    worker.embed(1, &digest, Purpose::Document, &[input(&[100, 101])]);
    // The second request arrives while the first truly runs.
    worker.embed(2, &digest, Purpose::Document, &[input(&[200])]);
    match worker.recv() {
        (Header::Busy { id: 2, .. }, _) => {}
        other => panic!("expected busy for id 2, got {other:?}"),
    }
    // The first reply lands only when the work ends (~2.5 s), not before.
    let started = Instant::now();
    match worker.recv() {
        (
            Header::Vectors {
                id: 1, count: 1, ..
            },
            _,
        ) => {}
        other => panic!("expected vectors for id 1, got {other:?}"),
    }
    assert!(
        started.elapsed() >= Duration::from_millis(1500),
        "the reply left the worker before the work truly ended"
    );
    // The slot frees once the work truly ended: a fresh request is admitted
    // and answered with its own deterministic vector. The slot is released
    // just after the reply write returns, so a client that resends the
    // instant it read the reply may see one transient `busy`; retry.
    let mut id = 3u64;
    let vectors = loop {
        worker.embed(id, &digest, Purpose::Document, &[input(&[200])]);
        match worker.recv() {
            (Header::Busy { id: busy, .. }, _) if busy == id => {
                assert!(id < 100, "the slot never freed after the work ended");
                id += 1;
                std::thread::sleep(Duration::from_millis(5));
            }
            (
                Header::Vectors {
                    id: got,
                    count: 1,
                    dims,
                    ..
                },
                payload,
            ) if got == id => {
                break protocol::decode_vectors(1, dims, &payload, 1, DIMENSIONS).expect("decode");
            }
            other => panic!("expected busy or vectors for id {id}, got {other:?}"),
        }
    };
    assert_eq!(
        vectors[0],
        worker_runtime::deterministic_vector(&[200], DIMENSIONS)
    );
}

#[test]
fn caller_timeout_waits_for_the_in_flight_reply_and_discards_it() {
    // Supervisor-level: the caller's deadline does not return early and
    // never releases the slot; the call resolves when the work truly ends,
    // the caller's error stands, the late reply is discarded, and the next
    // call gets its own vector back.
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let mut provider = launch(&profile, &["--slow-ms", "1500"]).expect("acquire");
    let started = Instant::now();
    let error = provider.embed_documents(
        &[input(&[100, 101])],
        &context_foundry::Control::with_deadline(Instant::now() + Duration::from_millis(200)),
    );
    let elapsed = started.elapsed();
    assert!(
        matches!(error, Err(ProviderError::Timeout)),
        "got {error:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(1000) && elapsed <= Duration::from_secs(10),
        "the caller waited for the in-flight reply, took {elapsed:?}"
    );
    let next = provider
        .embed_documents(&[input(&[200])], &context_foundry::Control::unbounded())
        .expect("slot freed and stale reply discarded");
    assert_eq!(
        next[0],
        worker_runtime::deterministic_vector(&[200], DIMENSIONS)
    );
}

#[test]
fn query_past_its_deadline_times_out_promptly_and_the_slot_stays_busy_until_the_late_reply() {
    // Queries obey their deadline: `Timeout` right away, the worker neither
    // waited for nor stopped. The request stays pending in the capacity-one
    // handoff, so every later call is `Busy` until the late reply arrives
    // and is discarded; then the next call succeeds with ITS vectors.
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let mut provider = launch(&profile, &["--slow-ms", "2000"]).expect("acquire");
    let pid = provider.worker_pid().expect("worker pid");

    let started = Instant::now();
    let deadline = started + Duration::from_millis(300);
    let error = provider
        .embed_query(&input(&[700]), deadline)
        .expect_err("the slow query must time out");
    let late = started.elapsed().saturating_sub(Duration::from_millis(300));
    assert!(matches!(error, ProviderError::Timeout), "got {error:?}");
    assert!(
        late < Duration::from_millis(500),
        "the query returned {late:?} after its deadline instead of promptly"
    );

    // Still occupied: documents and queries alike are busy, with nothing
    // sent to the worker and no worker stopped or replaced.
    let busy_documents =
        provider.embed_documents(&[input(&[1])], &context_foundry::Control::unbounded());
    assert!(
        matches!(busy_documents, Err(ProviderError::Busy)),
        "got {busy_documents:?}"
    );
    let busy_query = provider.embed_query(&input(&[2]), Instant::now() + Duration::from_secs(1));
    assert!(
        matches!(busy_query, Err(ProviderError::Busy)),
        "got {busy_query:?}"
    );
    assert_eq!(
        provider.worker_pid(),
        Some(pid),
        "the worker was not touched"
    );

    // The slow reply lands about 2 s after the query started and is
    // discarded; afterwards the call is admitted and gets its own vectors,
    // not the stale ones.
    std::thread::sleep(Duration::from_millis(2600).saturating_sub(started.elapsed()));
    let next = provider
        .embed_documents(&[input(&[800])], &context_foundry::Control::unbounded())
        .expect("the slot freed once the late reply was discarded");
    assert_eq!(
        next[0],
        worker_runtime::deterministic_vector(&[800], DIMENSIONS)
    );
    assert_ne!(
        next[0],
        worker_runtime::deterministic_vector(&[700], DIMENSIONS)
    );
    assert_eq!(provider.worker_pid(), Some(pid), "still the same worker");
}

/// 009 T003 review M1: the resident runtime over the supervised worker
/// counts the supervisor's abandoned query as an occupied slot until its late
/// reply really arrives, although the provider call returned `Timeout` at
/// the ceiling. A query and an explicit `prepare` are refused `provider_busy`
/// (before anything starts) until the worker released the call. Barrier: the
/// fake worker holds the query while the hold file exists.
#[cfg(feature = "semantic")]
#[test]
fn an_abandoned_query_keeps_the_runtime_slot_and_refuses_prepare_until_its_late_reply() {
    use context_foundry::neural::driver::{Owner, Preparation};
    use context_foundry::neural::provider::{FunctionDescriptor, LateCall};
    use context_foundry::neural::query::QueryRuntime;
    use std::sync::Arc;

    /// The supervised fake worker under the runtime profile's function.
    struct Relabeled {
        worker: WorkerProvider,
        descriptor: FunctionDescriptor,
    }
    impl EmbeddingProvider for Relabeled {
        fn descriptor(&self) -> &FunctionDescriptor {
            &self.descriptor
        }
        fn embed_documents(
            &mut self,
            batch: &[TokenizedInput],
            control: &context_foundry::Control,
        ) -> Result<Vec<Vec<f32>>, ProviderError> {
            self.worker.embed_documents(batch, control)
        }
        fn embed_query(
            &mut self,
            input: &TokenizedInput,
            deadline: Instant,
        ) -> Result<Vec<f32>, ProviderError> {
            self.worker.embed_query(input, deadline)
        }
        fn late_call(&self) -> Option<LateCall> {
            self.worker.late_call()
        }
    }
    /// An owner with no store: `prepare` is decided before any store step.
    struct NoStore;
    impl Owner for NoStore {
        fn try_primary(
            &self,
            _step: &mut dyn FnMut(&context_foundry::Engine),
        ) -> context_foundry::FResult<bool> {
            Err(context_foundry::FoundryError::InvalidArgument(
                "this test owner has no store".into(),
            ))
        }
        fn closing(&self) -> bool {
            false
        }
    }
    let wait_until = |what: &str, done: &dyn Fn() -> bool| {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !done() {
            assert!(Instant::now() < deadline, "never {what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    };

    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let worker_profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let runtime_profile = Arc::new(
        SemanticProfile::load(&context_foundry::testkit::write_semantic_profile(
            &dir.path().join("runtime"),
            "runtime",
            |_| {},
        ))
        .expect("the runtime profile"),
    );
    let hold = dir.path().join("hold");
    let phases = dir.path().join("phases");
    std::fs::write(&hold, b"").expect("hold file");
    let hooks = vec![
        "--hold-file".to_owned(),
        hold.display().to_string(),
        "--phase-file".to_owned(),
        phases.display().to_string(),
    ];
    let descriptor = runtime_profile.descriptor.clone();
    let runtime = Arc::new(
        QueryRuntime::start(
            Arc::clone(&runtime_profile),
            Box::new(move || {
                let worker = WorkerProvider::launch(&worker_profile, hooks)?;
                Ok(Box::new(Relabeled { worker, descriptor }) as Box<dyn EmbeddingProvider>)
            }),
        )
        .expect("the runtime starts"),
    );

    // The caller gives up at the ceiling while the worker still runs the
    // call: the supervisor keeps the abandoned handoff.
    assert_eq!(
        runtime.embed("twilight onset", Instant::now() + Duration::from_secs(30)),
        Err(ProviderError::Timeout)
    );
    wait_until("the worker runs the abandoned query", &|| {
        std::fs::read_to_string(&phases)
            .unwrap_or_default()
            .lines()
            .any(|phase| phase == "call")
    });
    assert!(runtime.occupied(), "the late call holds the slot");
    assert_eq!(
        runtime.embed("dusk", Instant::now() + Duration::from_secs(30)),
        Err(ProviderError::Busy)
    );
    let preparation = Arc::new(Preparation::default());
    let refused = preparation
        .prepare(Arc::new(NoStore), Arc::clone(&runtime))
        .expect_err("prepare is refused while the late call runs");
    assert_eq!(refused.code(), "provider_busy");
    assert!(preparation.idle(), "nothing was started");

    // The worker finishes; the late reply is discarded and the slot frees.
    std::fs::remove_file(&hold).expect("release the call");
    wait_until("the late reply arrived", &|| !runtime.occupied());
    preparation
        .prepare(Arc::new(NoStore), Arc::clone(&runtime))
        .expect("prepare proceeds once the late call ended");
    wait_until("the driver stopped", &|| preparation.idle());
    runtime.shutdown();
}

/// 009 T003 review M6: both admission paths claim the model slot BEFORE they
/// look for the provider's late call. Barrier (`semantic.slot_claimed`):
/// right after a claim, an older call on the same supervised worker is
/// driven past its ceiling while the fake worker holds it (`--hold-file`),
/// so the supervisor marks it abandoned. Each admission then finds that late
/// call, releases its claim and is refused `Busy`; no job reaches the
/// provider thread.
#[cfg(feature = "semantic")]
#[test]
fn an_admission_that_claimed_the_slot_still_refuses_a_call_abandoned_meanwhile() {
    use context_foundry::fault::{self, Action};
    use context_foundry::neural::fault_names::SLOT_CLAIMED;
    use context_foundry::neural::provider::{FunctionDescriptor, LateCall};
    use context_foundry::neural::query::QueryRuntime;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// The supervised fake worker, shared so the test can drive an older call
    /// on it directly; calls that come through the runtime are counted.
    struct Direct {
        worker: Arc<Mutex<WorkerProvider>>,
        descriptor: FunctionDescriptor,
        jobs: Arc<AtomicUsize>,
    }
    impl EmbeddingProvider for Direct {
        fn descriptor(&self) -> &FunctionDescriptor {
            &self.descriptor
        }
        fn embed_documents(
            &mut self,
            batch: &[TokenizedInput],
            control: &context_foundry::Control,
        ) -> Result<Vec<Vec<f32>>, ProviderError> {
            self.jobs.fetch_add(1, Ordering::SeqCst);
            self.worker.lock().unwrap().embed_documents(batch, control)
        }
        fn embed_query(
            &mut self,
            input: &TokenizedInput,
            deadline: Instant,
        ) -> Result<Vec<f32>, ProviderError> {
            self.jobs.fetch_add(1, Ordering::SeqCst);
            self.worker.lock().unwrap().embed_query(input, deadline)
        }
        fn late_call(&self) -> Option<LateCall> {
            self.worker.lock().unwrap().late_call()
        }
    }
    let wait_until = |what: &str, done: &dyn Fn() -> bool| {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !done() {
            assert!(Instant::now() < deadline, "never {what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    };

    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let worker_profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let runtime_profile = Arc::new(
        SemanticProfile::load(&context_foundry::testkit::write_semantic_profile(
            &dir.path().join("runtime"),
            "runtime",
            |_| {},
        ))
        .expect("the runtime profile"),
    );
    let hold = dir.path().join("hold");
    let log = dir.path().join("requests.log");
    let worker = Arc::new(Mutex::new(
        launch(
            &worker_profile,
            &[
                "--hold-file",
                &hold.display().to_string(),
                "--request-log",
                &log.display().to_string(),
            ],
        )
        .expect("acquire"),
    ));
    let jobs = Arc::new(AtomicUsize::new(0));
    let runtime = QueryRuntime::start(Arc::clone(&runtime_profile), {
        let (worker, jobs) = (Arc::clone(&worker), Arc::clone(&jobs));
        let descriptor = runtime_profile.descriptor.clone();
        Box::new(move || {
            Ok(Box::new(Direct {
                worker,
                descriptor,
                jobs,
            }) as Box<dyn EmbeddingProvider>)
        })
    })
    .expect("the runtime starts");

    // At the barrier: an older call on the same worker, past its 200 ms
    // ceiling while the worker holds it, so the supervisor abandons it.
    let hits = Arc::new(AtomicUsize::new(0));
    {
        let (older, hits) = (Arc::clone(&worker), Arc::clone(&hits));
        fault::arm(
            SLOT_CLAIMED,
            0,
            Action::Call(Box::new(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
                let abandoned = older
                    .lock()
                    .unwrap()
                    .embed_query(&input(&[7]), Instant::now() + Duration::from_millis(200));
                assert_eq!(abandoned, Err(ProviderError::Timeout));
            })),
        );
    }

    // The document path.
    std::fs::write(&hold, b"").expect("hold file");
    assert!(!runtime.occupied(), "the slot is free before the claim");
    let refused = runtime.dispatch_documents(
        vec![input(&[1, 2, 3])],
        context_foundry::Control::unbounded(),
    );
    assert!(
        matches!(refused, Err(ProviderError::Busy)),
        "the claim must find the late call"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert_eq!(jobs.load(Ordering::SeqCst), 0, "no job was sent");
    assert!(runtime.occupied(), "the late call holds the slot");
    std::fs::remove_file(&hold).expect("release the older call");
    wait_until("the late reply arrived", &|| !runtime.occupied());

    // The query path, at the same barrier.
    std::fs::write(&hold, b"").expect("hold file");
    assert_eq!(
        runtime.embed("dusk", Instant::now() + Duration::from_secs(30)),
        Err(ProviderError::Busy)
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    assert_eq!(jobs.load(Ordering::SeqCst), 0, "no job was sent");
    std::fs::remove_file(&hold).expect("release the older call");
    wait_until("the late reply arrived", &|| !runtime.occupied());

    assert_eq!(
        request_ids(&log),
        vec![1, 2],
        "only the two older calls reached the worker"
    );
    runtime.shutdown();
}

#[test]
fn in_flight_call_gets_30s_grace_then_the_worker_is_stopped() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let mut provider = launch(&profile, &["--slow-ms", "60000"]).expect("acquire");
    let pid = provider.worker_pid().expect("worker pid");
    let started = Instant::now();
    // The caller gives up at 300 ms while the work truly runs for 60 s.
    let error = provider.embed_documents(
        &[input(&[300])],
        &context_foundry::Control::with_deadline(Instant::now() + Duration::from_millis(300)),
    );
    let elapsed = started.elapsed();
    assert!(
        matches!(error, Err(ProviderError::Timeout)),
        "got {error:?}"
    );
    assert!(
        elapsed >= Duration::from_secs(30) && elapsed <= Duration::from_secs(40),
        "grace took {elapsed:?}"
    );
    // The worker was stopped for real, and nothing replaces it before the
    // process is gone.
    assert!(provider.stopped(), "stopped after the grace kill");
    assert!(
        pid_gone(pid, Duration::from_secs(1)),
        "worker {pid} was not reaped"
    );
    let replacement = launch(&profile, &[]).expect("replacement acquire");
    assert_ne!(replacement.worker_pid(), Some(pid));
}

#[test]
fn reply_faults_are_refused_as_malformed() {
    for (fault, hook) in [
        ("wrong-dims", "wrong-dims"),
        ("wrong-count", "wrong-count"),
        ("nan", "nan"),
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        let (bundle, sha) = fake_bundle(dir.path());
        let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
        let mut provider = launch(&profile, &["--reply-fault", hook]).expect("acquire");
        let result = provider.embed_documents(
            &[input(&[1]), input(&[2])],
            &context_foundry::Control::unbounded(),
        );
        assert!(
            matches!(result, Err(ProviderError::Malformed(_))),
            "{fault}: got {result:?}"
        );
    }
}

#[test]
fn oversize_reply_breaks_and_stops_the_worker() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let mut provider = launch(&profile, &["--reply-fault", "oversize"]).expect("acquire");
    let first = provider.embed_documents(&[input(&[1])], &context_foundry::Control::unbounded());
    assert!(
        matches!(
            first,
            Err(ProviderError::WorkerExited(_) | ProviderError::Malformed(_))
        ),
        "got {first:?}"
    );
    let second = provider.embed_documents(&[input(&[1])], &context_foundry::Control::unbounded());
    assert!(
        matches!(second, Err(ProviderError::WorkerExited(_))),
        "got {second:?}"
    );
}

#[test]
fn worker_rejects_invalid_frames_from_the_supervisor() {
    // Stale descriptor digest: a named refusal, worker stays alive.
    let mut worker = spawn_fake(&[]);
    worker.hello();
    worker.expect_ready();
    let digest = "0".repeat(64);
    worker.embed(1, &digest, Purpose::Document, &[input(&[1, 2])]);
    match worker.recv() {
        (
            Header::Error {
                id: Some(1),
                code,
                message,
                ..
            },
            _,
        ) => {
            assert_eq!(code, "descriptor_mismatch");
            assert!(!message.is_empty());
        }
        other => panic!("expected descriptor_mismatch, got {other:?}"),
    }
    // Token ID outside the reported vocabulary: input_invalid.
    worker.embed(
        2,
        &fake_descriptor().digest(),
        Purpose::Document,
        &[input(&[0x0fff_ffff])],
    );
    match worker.recv() {
        (Header::Error { code, .. }, _) => assert_eq!(code, "input_invalid"),
        other => panic!("expected input_invalid, got {other:?}"),
    }
    // Still serving: a valid embed gets vectors.
    worker.embed(
        3,
        &fake_descriptor().digest(),
        Purpose::Document,
        &[input(&[7])],
    );
    match worker.recv() {
        (
            Header::Vectors {
                id: 3,
                count: 1,
                dims,
                ..
            },
            _,
        ) => assert_eq!(dims as usize, DIMENSIONS),
        other => panic!("expected vectors for id 3, got {other:?}"),
    }
    drop(worker);

    // Oversize declared header: refused before allocation, stream ends.
    let mut worker = spawn_fake(&[]);
    worker.hello();
    worker.expect_ready();
    worker.write_raw(&0xffff_ffffu32.to_le_bytes());
    match worker.recv() {
        (Header::Error { code, .. }, _) => assert_eq!(code, "frame_invalid"),
        other => panic!("expected frame_invalid, got {other:?}"),
    }
    let status = worker.child.wait().expect("wait");
    assert_eq!(status.code(), Some(worker_runtime::FRAME_EXIT));
    drop(worker);

    // Wrong protocol version: refused, stream ends.
    let mut worker = spawn_fake(&[]);
    worker.send(
        &Header::Hello {
            protocol: protocol::PROTOCOL_VERSION + 1,
        },
        &[],
    );
    match worker.recv() {
        (Header::Error { code, .. }, _) => assert_eq!(code, "frame_invalid"),
        other => panic!("expected frame_invalid, got {other:?}"),
    }
    assert_eq!(
        worker.child.wait().expect("wait").code(),
        Some(worker_runtime::FRAME_EXIT)
    );
    drop(worker);

    // Malformed header JSON: refused, stream ends.
    let mut worker = spawn_fake(&[]);
    worker.write_raw(&4u32.to_le_bytes());
    worker.write_raw(b"junk");
    worker.write_raw(&0u32.to_le_bytes());
    match worker.recv() {
        (Header::Error { code, .. }, _) => assert_eq!(code, "frame_invalid"),
        other => panic!("expected frame_invalid, got {other:?}"),
    }
    assert_eq!(
        worker.child.wait().expect("wait").code(),
        Some(worker_runtime::FRAME_EXIT)
    );
    drop(worker);

    // Oversize declared payload on an embed: refused, stream ends.
    let mut worker = spawn_fake(&[]);
    worker.hello();
    worker.expect_ready();
    let (lengths, _) = protocol::encode_ids(&[input(&[1])]);
    let header = Header::Embed {
        protocol: protocol::PROTOCOL_VERSION,
        id: 9,
        descriptor_digest: fake_descriptor().digest(),
        purpose: Purpose::Document,
        lengths,
    };
    let header_bytes = serde_json::to_vec(&header).expect("header JSON");
    worker.write_raw(&(header_bytes.len() as u32).to_le_bytes());
    worker.write_raw(&header_bytes);
    worker.write_raw(&0x00ff_ffffu32.to_le_bytes());
    worker.write_raw(b"part");
    match worker.recv() {
        (Header::Error { code, .. }, _) => assert_eq!(code, "frame_invalid"),
        other => panic!("expected frame_invalid, got {other:?}"),
    }
    assert_eq!(
        worker.child.wait().expect("wait").code(),
        Some(worker_runtime::FRAME_EXIT)
    );
}

#[test]
fn stray_stdout_writes_never_corrupt_the_frame_channel() {
    // A library that prints to fd 1 (the real model stack does: a lazy
    // import error from huggingface_hub did exactly that). The runtime moves
    // the frame channel to a private descriptor and points fd 1 at stderr,
    // so launch and calls still work and the text lands in the bounded
    // stderr excerpt instead.
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let mut provider = launch(&profile, &["--stray-stdout"]).expect("acquire despite stray stdout");
    let vectors = provider
        .embed_documents(&[input(&[9])], &context_foundry::Control::unbounded())
        .expect("embed after stray stdout");
    assert_eq!(
        vectors[0],
        worker_runtime::deterministic_vector(&[9], DIMENSIONS)
    );
    assert!(
        provider
            .stderr_excerpt()
            .contains("stray stdout line from a library"),
        "the stray line must be diverted to stderr: {}",
        provider.stderr_excerpt()
    );
}

#[test]
fn stderr_is_drained_continuously_into_a_bounded_buffer() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let mut provider = launch(&profile, &["--stderr-flood", "1048576"]).expect("acquire");
    let vectors = provider
        .embed_documents(&[input(&[42])], &context_foundry::Control::unbounded())
        .expect("embed after flood");
    assert_eq!(
        vectors[0],
        worker_runtime::deterministic_vector(&[42], DIMENSIONS)
    );
    let excerpt = provider.stderr_excerpt();
    let retained = excerpt
        .split('(')
        .nth(1)
        .and_then(|rest| rest.split(' ').next())
        .and_then(|count| count.parse::<usize>().ok())
        .expect("retained byte count in the excerpt");
    assert!(
        retained <= protocol::MAX_STDERR_BYTES,
        "{retained} retained"
    );
    assert!(excerpt.contains('f'), "the flood text was captured");
}

#[test]
fn owner_death_on_liveness_pipe_eof_ends_the_worker() {
    let mut worker = spawn_fake(&[]);
    worker.hello();
    worker.expect_ready();
    let pid = worker.child.id();
    // Closing the write end is the owner-gone signal; no SIGKILL needed.
    drop(worker.liveness_write);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Ok(Some(_)) = worker.child.try_wait() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "worker {pid} outlived the pipe EOF by 2 s"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// A `--shim` owner: the fake binary becomes the owner of one worker child
/// (a copy of itself, or `--shim-exe`), keeps that child's liveness pipe
/// open through a separate `sleep` process so only kqueue can report the
/// owner's death, and prints the child's PID. The shim's stdin and stdout
/// are the child's IPC. The worker appends every phase it really enters to
/// `phase_file` (`--phase-file`), so a test kills the owner only once the
/// intended phase is running, never after a fixed delay.
struct Shim {
    child: Child,
    worker_pid: u32,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    phase_file: PathBuf,
}

fn fake_worker_args(dir: &Path, hooks: &[&str]) -> Vec<String> {
    let dir = dir.display().to_string();
    let mut args = vec![
        "--descriptor".to_string(),
        serde_json::to_string(&fake_descriptor()).expect("descriptor JSON"),
        "--model-dir".to_string(),
        dir,
    ];
    args.extend(hooks.iter().map(|hook| hook.to_string()));
    args
}

fn start_shim(
    worker_exe: Option<&Path>,
    worker_args: &[String],
    env: Option<&[(&'static str, String)]>,
    phase_file: &Path,
) -> Shim {
    let mut command = Command::new(fake_exe());
    command.arg("--shim");
    if let Some(exe) = worker_exe {
        command.arg("--shim-exe").arg(exe);
    }
    command
        .args(worker_args)
        .arg("--phase-file")
        .arg(phase_file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(env) = env {
        command.env_clear();
        for (key, value) in env {
            command.env(key, value);
        }
    }
    let mut child = command.spawn().expect("spawn shim");
    let mut stdout = child.stdout.take().expect("shim stdout");
    let stdin = child.stdin.take().expect("shim stdin");
    // The shim prints the worker PID before anything else; read it byte by
    // byte so the framed reader starts exactly at the first frame.
    let mut line = String::new();
    let mut byte = [0u8; 1];
    loop {
        assert_eq!(
            stdout.read(&mut byte).expect("read shim stdout"),
            1,
            "shim stdout closed"
        );
        if byte[0] == b'\n' {
            break;
        }
        line.push(byte[0] as char);
    }
    let worker_pid: u32 = line
        .strip_prefix("shim-worker-pid ")
        .and_then(|pid| pid.parse().ok())
        .unwrap_or_else(|| panic!("shim printed {line:?}"));
    Shim {
        child,
        worker_pid,
        stdin,
        stdout: BufReader::new(stdout),
        phase_file: phase_file.to_path_buf(),
    }
}

/// `ps` for one PID: its state and elapsed time while it lingers.
fn ps_line(pid: u32) -> String {
    Command::new("/bin/ps")
        .args(["-o", "pid,ppid,stat,etime,command", "-p"])
        .arg(pid.to_string())
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .last()
                .unwrap_or("")
                .trim()
                .to_string()
        })
        .unwrap_or_else(|e| format!("ps failed: {e}"))
}

/// What happened between the owner's SIGKILL and the worker's disappearance.
#[derive(Debug)]
struct OwnerDeath {
    /// SIGKILL until `kill(pid, 0)` reported `ESRCH`; `None` if the worker
    /// was still there when the observation gave up.
    gone_after: Option<Duration>,
    /// SIGKILL until the worker's own `_exit` call (its `exit` phase line):
    /// how fast the watcher noticed, whatever the main thread was doing.
    exit_called_after: Option<Duration>,
    /// The worker's `_exit` call until its disappearance: the kernel's
    /// teardown of the process.
    exit_to_gone: Option<Duration>,
    /// `ps` of the worker while it lingered: state, elapsed time.
    ps: Vec<(Duration, String)>,
}

impl Shim {
    fn send(&mut self, header: &Header, payload: &[u8]) {
        protocol::write_frame(&mut self.stdin, header, payload).expect("send frame");
    }

    fn hello(&mut self) {
        self.send(
            &Header::Hello {
                protocol: protocol::PROTOCOL_VERSION,
            },
            &[],
        );
    }

    fn expect_ready(&mut self) {
        match protocol::read_frame(&mut self.stdout).expect("ready frame") {
            (Header::Ready { .. }, _) => {}
            other => panic!("expected ready, got {other:?}"),
        }
    }

    fn embed_batch(&mut self, id: u64, digest: &str, purpose: Purpose, inputs: &[TokenizedInput]) {
        let (lengths, payload) = protocol::encode_ids(inputs);
        self.send(
            &Header::Embed {
                protocol: protocol::PROTOCOL_VERSION,
                id,
                descriptor_digest: digest.into(),
                purpose,
                lengths,
            },
            &payload,
        );
    }

    fn embed(&mut self, id: u64, digest: String, purpose: Purpose, input: &TokenizedInput) {
        self.embed_batch(id, &digest, purpose, std::slice::from_ref(input));
    }

    /// The reply to request `id`: `count` vectors.
    fn expect_vectors(&mut self, id: u64, count: usize) -> Vec<Vec<f32>> {
        match protocol::read_frame(&mut self.stdout).expect("reply frame") {
            (
                Header::Vectors {
                    id: got,
                    count: n,
                    dims,
                    ..
                },
                payload,
            ) if got == id => protocol::decode_vectors(n, dims, &payload, count, DIMENSIONS)
                .expect("decode vectors"),
            other => panic!("expected vectors for {id}, got {other:?}"),
        }
    }

    /// The phases the worker has entered, oldest first.
    fn phases(&self) -> Vec<String> {
        std::fs::read_to_string(&self.phase_file)
            .map(|text| text.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    /// Block until the worker's most recent phase is `phase`.
    fn wait_phase(&self, phase: &str, within: Duration) {
        let deadline = Instant::now() + within;
        loop {
            if self.phases().last().map(String::as_str) == Some(phase) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the worker never entered phase {phase:?}; phases so far: {:?}",
                self.phases()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// SIGKILL the owner and watch the worker until it is gone (or until
    /// `give_up`), recording the actual timing and, while it lingers, its
    /// process state.
    fn kill_owner(mut self, give_up: Duration) -> OwnerDeath {
        let wall = |at: std::time::SystemTime| {
            at.duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        };
        let killed_wall = wall(std::time::SystemTime::now());
        let killed = Instant::now();
        unsafe {
            libc::kill(self.child.id() as libc::pid_t, libc::SIGKILL);
        }
        let mut ps = Vec::new();
        let mut next_ps = Duration::from_millis(50);
        let gone_after = loop {
            let elapsed = killed.elapsed();
            let rc = unsafe { libc::kill(self.worker_pid as libc::pid_t, 0) };
            if rc == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                break Some(elapsed);
            }
            if elapsed >= give_up {
                break None;
            }
            if elapsed >= next_ps {
                ps.push((elapsed, ps_line(self.worker_pid)));
                next_ps = elapsed
                    + if elapsed < Duration::from_secs(2) {
                        Duration::from_millis(450)
                    } else {
                        Duration::from_secs(2)
                    };
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let exit_ns = self.phases().iter().rev().find_map(|line| {
            line.strip_prefix("exit ")
                .and_then(|ns| ns.parse::<u128>().ok())
        });
        let exit_called_after =
            exit_ns.map(|ns| Duration::from_nanos(ns.saturating_sub(killed_wall) as u64));
        let exit_to_gone = match (exit_ns, gone_after) {
            (Some(ns), Some(after)) => Some(Duration::from_nanos(
                (killed_wall + after.as_nanos()).saturating_sub(ns) as u64,
            )),
            _ => None,
        };
        let _ = self.child.wait();
        OwnerDeath {
            gone_after,
            exit_called_after,
            exit_to_gone,
            ps,
        }
    }

    /// SIGKILL the owner and assert the worker is gone within 2 s. A miss
    /// still waits for the actual disappearance, so the failure carries the
    /// real time, the worker's own `_exit` timing and its process state.
    fn kill_owner_and_expect_worker_gone(self, what: &str) {
        let pid = self.worker_pid;
        let death = self.kill_owner(Duration::from_secs(60));
        match death.gone_after {
            Some(after) if after <= Duration::from_secs(2) => {}
            _ => panic!(
                "worker {pid} outlived owner death ({what}) by more than 2 s: {death:?}; process \
                 state while it lingered (elapsed, ps): {:?}",
                death.ps
            ),
        }
    }
}

/// Owner death observed only through kqueue (the liveness pipe stays open
/// in a separate `sleep` process the shim starts). Phases: during load,
/// during a long admitted call, and while the worker is blocked writing
/// its reply. The kill waits for the worker's own record that the phase is
/// running.
fn shim_owner_death(hooks: &[&str], phase: &str) {
    let dir = tempfile::tempdir().expect("tempdir");
    let phase_file = dir.path().join("phase");
    let mut shim = start_shim(
        None,
        &fake_worker_args(dir.path(), hooks),
        None,
        &phase_file,
    );
    shim.hello();
    match phase {
        // The worker is inside its artificial load delay.
        "load" => {}
        "call" | "blocked" => {
            shim.expect_ready();
            shim.embed(
                1,
                fake_descriptor().digest(),
                Purpose::Document,
                &input(&[1]),
            );
        }
        other => panic!("unknown phase {other}"),
    }
    shim.wait_phase(phase, Duration::from_secs(20));
    shim.kill_owner_and_expect_worker_gone(phase);
}

#[test]
fn owner_death_during_load_ends_the_worker() {
    shim_owner_death(&["--load-ms", "9000"], "load");
}

#[test]
fn owner_death_during_long_call_ends_the_worker() {
    shim_owner_death(&["--slow-ms", "9000"], "call");
}

#[test]
fn owner_death_while_ipc_blocked_ends_the_worker() {
    shim_owner_death(&["--stdout-block"], "blocked");
}

#[test]
fn the_worker_records_its_own_exit_so_the_teardown_can_be_timed() {
    // The measurement the 2 s contract rests on: the worker's `_exit` call is
    // timestamped in its phase file, so a miss splits into "the watcher was
    // late" and "the kernel took long to reap". Here both are small.
    let dir = tempfile::tempdir().expect("tempdir");
    let phase_file = dir.path().join("phase");
    let mut shim = start_shim(
        None,
        &fake_worker_args(dir.path(), &["--slow-ms", "9000"]),
        None,
        &phase_file,
    );
    shim.hello();
    shim.expect_ready();
    shim.embed(
        1,
        fake_descriptor().digest(),
        Purpose::Document,
        &input(&[1]),
    );
    shim.wait_phase("call", Duration::from_secs(20));
    let death = shim.kill_owner(Duration::from_secs(30));
    let exit_called = death
        .exit_called_after
        .expect("the worker recorded its exit");
    let to_gone = death.exit_to_gone.expect("the worker disappeared");
    assert!(
        exit_called < Duration::from_secs(1),
        "the watcher took {exit_called:?} to end the process: {death:?}"
    );
    assert!(
        to_gone < Duration::from_secs(1),
        "the process took {to_gone:?} to disappear after _exit: {death:?}"
    );
}

#[test]
fn term_ignoring_worker_is_reaped_after_the_5s_kill() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let mut provider = launch(&profile, &["--ignore-term", "--ignore-eof"]).expect("acquire");
    let pid = provider.worker_pid().expect("worker pid");
    let started = Instant::now();
    provider.shutdown();
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_secs(4) && elapsed <= Duration::from_secs(8),
        "shutdown took {elapsed:?}"
    );
    assert!(provider.stopped(), "stopped only after the process is gone");
    assert!(
        pid_gone(pid, Duration::from_secs(1)),
        "worker {pid} was not reaped"
    );
}

#[test]
fn no_replacement_worker_before_the_old_one_disappeared() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let first = launch(&profile, &[]).expect("first acquire");
    let first_pid = first.worker_pid().expect("first pid");
    drop(first);
    assert!(
        pid_gone(first_pid, Duration::from_secs(2)),
        "first worker not reaped"
    );
    let second = launch(&profile, &[]).expect("second acquire");
    let second_pid = second.worker_pid().expect("second pid");
    assert_ne!(first_pid, second_pid);
    assert!(second.worker_pid().is_some());
}

#[test]
fn memory_ceiling_breach_during_load_stops_with_resource_limit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 32 << 20, 30);
    let error = refused(
        launch(&profile, &["--alloc-mb", "256", "--load-ms", "1500"]),
        "breach during load",
    );
    assert!(
        matches!(&error, ProviderError::ResourceLimit(m) if m.contains("ceiling")),
        "got {error:?}"
    );
}

#[test]
fn memory_ceiling_breach_during_a_call_stops_with_resource_limit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 32 << 20, 30);
    let mut provider = launch(&profile, &["--alloc-call-mb", "256", "--slow-ms", "1200"])
        .expect("acquire under the ceiling");
    let error = provider
        .embed_documents(&[input(&[1])], &context_foundry::Control::unbounded())
        .expect_err("breach during call");
    assert!(
        matches!(&error, ProviderError::ResourceLimit(m) if m.contains("ceiling")),
        "got {error:?}"
    );
    assert!(provider.stopped());
}

#[test]
fn without_the_development_flag_admission_is_isolation_unavailable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let started = Instant::now();
    let error = refused(
        supervisor::acquire_until(&profile, false, &context_foundry::Control::unbounded()),
        "refused without development",
    );
    assert!(
        matches!(error, ProviderError::IsolationUnavailable(_)),
        "got {error:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "refusal was not immediate"
    );
}

#[test]
fn executable_hash_mismatch_is_refused_before_launch() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, _) = fake_bundle(dir.path());
    let wrong = "a".repeat(64);
    let profile = fake_profile(dir.path(), &bundle, &wrong, 3 << 30, 60);
    let started = Instant::now();
    let error = refused(launch(&profile, &[]), "hash mismatch");
    assert!(
        matches!(&error, ProviderError::ProfileInvalid(m) if m.contains("SHA-256")),
        "got {error:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "refused before launch work"
    );
}

#[test]
fn artifact_hash_mismatch_is_refused_before_launch() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    // Corrupt the pinned artifact: its digest no longer matches.
    std::fs::write(profile.model_dir.join("model.gguf"), b"tampered").expect("tamper");
    let error = refused(launch(&profile, &[]), "artifact mismatch");
    assert!(
        matches!(&error, ProviderError::ProfileInvalid(m) if m.contains("SHA-256")),
        "got {error:?}"
    );
}

/// The PID the fake worker wrote (`--pid-file`) must be a process that is
/// gone: the aborted start stopped and reaped it before returning, so no
/// replacement can precede its disappearance.
fn assert_worker_reaped(pid_file: &Path) {
    let pid: u32 = std::fs::read_to_string(pid_file)
        .expect("the worker wrote its PID before the start was aborted")
        .trim()
        .parse()
        .expect("PID file content");
    assert!(
        pid_gone(pid, Duration::from_millis(200)),
        "worker {pid} survived an aborted start"
    );
}

#[test]
fn acquire_until_stops_a_slow_starting_worker_at_the_control_deadline() {
    let _guard = LAUNCH_TIMING_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // The profile allows 60 s to load; the caller's control allows 500 ms.
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let pid_file = dir.path().join("worker.pid");
    // Headroom for the worker to reach its PID write under a parallel suite;
    // the 9 s load hook keeps it far from ready.
    let control =
        context_foundry::Control::with_deadline(Instant::now() + Duration::from_millis(3000));
    let started = Instant::now();
    let error = refused(
        WorkerProvider::launch_until(
            &profile,
            vec![
                "--load-ms".into(),
                "9000".into(),
                "--pid-file".into(),
                pid_file.display().to_string(),
            ],
            &control,
        ),
        "a start past the control deadline",
    );
    let late = started
        .elapsed()
        .saturating_sub(Duration::from_millis(3000));
    assert!(matches!(error, ProviderError::Timeout), "got {error:?}");
    assert!(
        late < Duration::from_secs(1),
        "the start returned {late:?} after the control deadline"
    );
    assert_worker_reaped(&pid_file);
}

#[test]
fn cancellation_aborts_a_starting_worker_promptly() {
    let _guard = LAUNCH_TIMING_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let pid_file = dir.path().join("worker.pid");
    let control = context_foundry::Control::unbounded();
    let flag = control.cancel_flag();
    // Cancel only once the worker is really running (its PID file exists),
    // so the abort is of a started worker whatever the machine load.
    let cancelled_at = std::sync::Arc::new(std::sync::Mutex::new(None::<Instant>));
    let marker = std::sync::Arc::clone(&cancelled_at);
    let watched = pid_file.clone();
    std::thread::spawn(move || {
        let give_up = Instant::now() + Duration::from_secs(20);
        while !watched.exists() && Instant::now() < give_up {
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(100));
        *marker.lock().expect("marker") = Some(Instant::now());
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let error = refused(
        WorkerProvider::launch_until(
            &profile,
            vec![
                "--load-ms".into(),
                "9000".into(),
                "--pid-file".into(),
                pid_file.display().to_string(),
            ],
            &control,
        ),
        "a cancelled start",
    );
    assert!(matches!(error, ProviderError::Cancelled), "got {error:?}");
    let promptness = cancelled_at
        .lock()
        .expect("marker")
        .expect("the cancel thread ran")
        .elapsed();
    assert!(
        promptness < Duration::from_secs(1),
        "the cancelled start returned {promptness:?} after the cancellation"
    );
    assert_worker_reaped(&pid_file);
}

#[test]
fn acquire_until_refuses_a_stopped_caller_before_spawning_anything() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let cancelled = refused(
        supervisor::acquire_until(&profile, true, &context_foundry::Control::cancelled()),
        "a cancelled caller",
    );
    assert!(
        matches!(cancelled, ProviderError::Cancelled),
        "got {cancelled:?}"
    );
    let expired = refused(
        supervisor::acquire_until(
            &profile,
            true,
            &context_foundry::Control::with_deadline(Instant::now()),
        ),
        "an expired caller",
    );
    assert!(matches!(expired, ProviderError::Timeout), "got {expired:?}");
    // The isolation refusal still comes first for a normal (non-development)
    // admission, whatever the control says.
    let normal = refused(
        supervisor::acquire_until(&profile, false, &context_foundry::Control::unbounded()),
        "normal admission",
    );
    assert!(
        matches!(normal, ProviderError::IsolationUnavailable(_)),
        "got {normal:?}"
    );
    // A live control still acquires normally through the same entry point.
    let mut provider = supervisor::acquire_until(
        &profile,
        true,
        &context_foundry::Control::with_deadline(Instant::now() + Duration::from_secs(30)),
    )
    .unwrap_or_else(|e| panic!("a live control must acquire: {e}"));
    let vectors = provider
        .embed_documents(&[input(&[5])], &context_foundry::Control::unbounded())
        .expect("embed");
    assert_eq!(
        vectors[0],
        worker_runtime::deterministic_vector(&[5], DIMENSIONS)
    );
}

#[test]
fn load_deadline_expires_and_the_worker_is_stopped() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 1);
    let started = Instant::now();
    let error = refused(launch(&profile, &["--load-ms", "8000"]), "load timeout");
    assert!(matches!(error, ProviderError::Timeout), "got {error:?}");
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "deadline enforced promptly"
    );
}

#[test]
fn ready_descriptor_mismatch_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let error = refused(launch(&profile, &["--ready-mutate"]), "ready mismatch");
    assert!(
        matches!(&error, ProviderError::ProfileInvalid(m) if m.contains("descriptor")),
        "got {error:?}"
    );
}

// ---------------------------------------------------------------------------
// Review round 2. Each test below fails on the code it replaced:
//   M2 arming window · M3/M4 real-adapter admission · M5 one absolute
//   acquisition bound · M6 the full control at every dispatch and reply ·
//   M7 isolated interpreter · m1 scratch-root ownership.
// ---------------------------------------------------------------------------

/// An exited child that has NOT been reaped (a zombie): its PID still names
/// it, and a `NOTE_EXIT` registration on it fires at once.
fn exited_unreaped_child() -> (Child, u32) {
    let child = Command::new("/usr/bin/true")
        .spawn()
        .expect("spawn /usr/bin/true");
    let pid = child.id();
    let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
    let rc = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOWAIT,
        )
    };
    assert_eq!(rc, 0, "waitid: {}", std::io::Error::last_os_error());
    (child, pid)
}

#[test]
fn an_owner_gone_before_the_watcher_armed_is_found_by_the_registration_itself() {
    use worker_runtime::{Armed, arm_owner_death_watcher};
    // The liveness pipe's write end stays open in this process throughout,
    // so the exit notification (or the missing process) is the ONLY thing
    // that can say the owner is gone: the registration call must act on what
    // it returns, because a process-exit notification is one-shot.
    let (reader, read_fd, _held_open) = liveness_pipe();
    let (mut zombie, pid) = exited_unreaped_child();
    assert_eq!(
        arm_owner_death_watcher(pid, read_fd),
        Ok(Armed::OwnerGone),
        "an owner that exited and is not yet reaped"
    );
    zombie.wait().expect("reap the owner");
    assert_eq!(
        arm_owner_death_watcher(pid, read_fd),
        Ok(Armed::OwnerGone),
        "an owner that no longer exists at all"
    );
    // The pipe-EOF half of the pair, with a live owner.
    let (eof_reader, eof_fd, eof_writer) = liveness_pipe();
    drop(eof_writer);
    let mut live = Command::new("/bin/sleep")
        .arg("30")
        .spawn()
        .expect("spawn sleep");
    assert_eq!(
        arm_owner_death_watcher(live.id(), eof_fd),
        Ok(Armed::OwnerGone),
        "a liveness pipe already at EOF"
    );
    let _ = live.kill();
    let _ = live.wait();
    drop((reader, eof_reader));
    // (A live owner with an open pipe arms the watcher thread and returns
    // `Watching`; the shim tests above cover that path end to end, since a
    // watcher armed here would end this very test process with its owner.)
}

/// The claims the real worker implements, mutated one at a time by
/// [`mutate_adapter_field`].
const ADAPTER_FIELDS: [&str; 5] = [
    "adapter_revision",
    "llama_cpp",
    "pooling",
    "output",
    "dimensions",
];

/// Change `field` to a value the real adapter does not implement; returns
/// the text the refusal names and whether `validate` already refuses it.
fn mutate_adapter_field(
    descriptor: &mut context_foundry::neural::provider::FunctionDescriptor,
    field: &str,
) -> (&'static str, bool) {
    match field {
        "adapter_revision" => {
            descriptor.adapter_revision += 1;
            ("adapter_revision", false)
        }
        "llama_cpp" => {
            descriptor.llama_cpp = "0".repeat(40);
            ("llama.cpp", false)
        }
        "pooling" => {
            descriptor.pooling = "max".into();
            ("pooling", true)
        }
        "output" => {
            descriptor.output = "f16".into();
            ("output", true)
        }
        "dimensions" => {
            descriptor.dimensions = 1000;
            ("dimensions", true)
        }
        other => panic!("unknown adapter field {other}"),
    }
}

#[test]
fn the_real_adapter_check_refuses_each_claim_it_does_not_implement() {
    worker_runtime::check_real_descriptor(&fake_descriptor())
        .expect("the unmutated descriptor is admitted (positive control)");
    for field in ADAPTER_FIELDS {
        let mut descriptor = fake_descriptor();
        let (named, _) = mutate_adapter_field(&mut descriptor, field);
        let message = worker_runtime::check_real_descriptor(&descriptor)
            .expect_err("a foreign adapter claim must be refused");
        assert!(message.contains(named), "{field}: {message}");
    }
}

#[test]
fn the_supervisor_refuses_a_foreign_adapter_recipe_before_launching_anything() {
    for field in ADAPTER_FIELDS {
        let dir = tempfile::tempdir().expect("tempdir");
        let (bundle, sha) = fake_bundle(dir.path());
        let mut profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
        let (named, _) = mutate_adapter_field(&mut profile.descriptor, field);
        let error = refused(
            supervisor::acquire_until(&profile, true, &context_foundry::Control::unbounded()),
            field,
        );
        assert!(
            matches!(&error, ProviderError::ProfileInvalid(m) if m.contains(named)),
            "{field}: got {error:?}"
        );
        assert!(
            !profile.worker.scratch_root.exists(),
            "{field}: the refusal came after launch work began"
        );
    }
}

/// The files the real worker reads: the GGUF it loads and the core's
/// tokenizer the IDs come from.
const REAL_INPUTS: [&str; 2] = ["model.gguf", "tokenizer.json"];

#[test]
fn every_file_the_worker_reads_must_be_in_the_verified_inventory() {
    for name in REAL_INPUTS {
        // The check itself.
        let mut descriptor = fake_descriptor();
        descriptor.artifact_files.retain(|file| file.name != name);
        let message = worker_runtime::check_real_descriptor(&descriptor)
            .expect_err("an omitted input must be refused");
        assert!(message.contains(name), "{name}: {message}");
        // The supervisor, with the file still on disk: it would be loaded
        // without ever having been hashed.
        let dir = tempfile::tempdir().expect("tempdir");
        let (bundle, sha) = fake_bundle(dir.path());
        let mut profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
        profile
            .descriptor
            .artifact_files
            .retain(|file| file.name != name);
        assert!(profile.model_dir.join(name).is_file());
        let error = refused(
            supervisor::acquire_until(&profile, true, &context_foundry::Control::unbounded()),
            name,
        );
        assert!(
            matches!(&error, ProviderError::ProfileInvalid(m) if m.contains(name)),
            "{name}: got {error:?}"
        );
        assert!(
            !profile.worker.scratch_root.exists(),
            "{name}: the refusal came after launch work began"
        );
    }
}

/// Run the REAL `foundry-embed` with `descriptor` against an empty model
/// directory, send `hello`, and report the first frame it answers with and
/// its exit code. Nothing here can load a model.
#[cfg(feature = "embed-worker")]
fn real_worker_first_answer(
    descriptor: &context_foundry::neural::provider::FunctionDescriptor,
) -> (Option<Header>, Option<i32>) {
    real_worker_answer_over(descriptor, &[])
}

/// [`real_worker_first_answer`] with `files` written into the model
/// directory first.
#[cfg(feature = "embed-worker")]
fn real_worker_answer_over(
    descriptor: &context_foundry::neural::provider::FunctionDescriptor,
    files: &[(&str, &[u8])],
) -> (Option<Header>, Option<i32>) {
    let dir = tempfile::tempdir().expect("tempdir");
    for (name, bytes) in files {
        std::fs::write(dir.path().join(name), bytes).expect("model file");
    }
    let (reader, read_fd, _liveness_write) = liveness_pipe();
    let mut command = Command::new(env!("CARGO_BIN_EXE_foundry-embed"));
    command
        .arg("--owner-pid")
        .arg(std::process::id().to_string())
        .arg("--liveness-fd")
        .arg(read_fd.to_string())
        .arg("--descriptor")
        .arg(serde_json::to_string(descriptor).expect("descriptor JSON"))
        .arg("--model-dir")
        .arg(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // SAFETY: `inherit` runs one async-signal-safe `fcntl` between fork and
    // exec.
    unsafe { command.pre_exec(inherit(read_fd)) };
    let mut child = command.spawn().expect("spawn the real worker");
    drop(reader);
    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));
    let _ = protocol::write_frame(
        &mut stdin,
        &Header::Hello {
            protocol: protocol::PROTOCOL_VERSION,
        },
        &[],
    );
    let header = protocol::read_frame(&mut stdout)
        .ok()
        .map(|(header, _)| header);
    let code = child.wait().expect("wait for the real worker").code();
    (header, code)
}

#[cfg(feature = "embed-worker")]
#[test]
fn the_real_worker_refuses_a_foreign_descriptor_before_the_model() {
    // Positive control: a descriptor the real recipe admits gets PAST the
    // check and fails later, at the (absent) GGUF, under another code.
    let (header, code) = real_worker_first_answer(&fake_descriptor());
    match header {
        Some(Header::Error {
            id: None,
            code: named,
            ..
        }) => assert_eq!(named, "load_failed"),
        other => panic!("the admitted descriptor must reach the load: {other:?}"),
    }
    assert_eq!(code, Some(1));

    for field in ADAPTER_FIELDS {
        let mut descriptor = fake_descriptor();
        let (named_text, structural) = mutate_adapter_field(&mut descriptor, field);
        let (header, code) = real_worker_first_answer(&descriptor);
        if structural {
            // Structurally invalid: refused while parsing the argv.
            assert!(header.is_none(), "{field}: no frame before refusal");
            assert_eq!(code, Some(64), "{field}");
            continue;
        }
        match header {
            Some(Header::Error {
                id: None,
                code: named,
                message,
                ..
            }) => {
                assert_eq!(named, "descriptor_unsupported", "{field}");
                assert!(message.contains(named_text), "{field}: {message}");
            }
            other => panic!("{field}: expected descriptor_unsupported, got {other:?}"),
        }
        assert_eq!(code, Some(1), "{field}");
    }

    // The GGUF outside the inventory is structurally invalid; the tokenizer
    // outside it is a recipe the worker refuses.
    for (name, structural) in [("model.gguf", true), ("tokenizer.json", false)] {
        let mut descriptor = fake_descriptor();
        descriptor.artifact_files.retain(|file| file.name != name);
        let (header, code) = real_worker_first_answer(&descriptor);
        if structural {
            assert!(header.is_none(), "{name}: no frame before refusal");
            assert_eq!(code, Some(64), "{name}");
            continue;
        }
        match header {
            Some(Header::Error {
                id: None,
                code: named,
                message,
                ..
            }) => {
                assert_eq!(named, "descriptor_unsupported", "{name}");
                assert!(message.contains(name), "{name}: {message}");
            }
            other => panic!("{name}: expected descriptor_unsupported, got {other:?}"),
        }
        assert_eq!(code, Some(1), "{name}");
    }
}

/// A GGUF v3 header with no tensors whose key/value pairs are `pairs`
/// (each a key, a GGUF value type and the value's bytes).
#[cfg(feature = "embed-worker")]
fn gguf_header(pairs: &[(&str, u32, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = b"GGUF".to_vec();
    bytes.extend_from_slice(&3u32.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.extend_from_slice(&(pairs.len() as u64).to_le_bytes());
    for (key, kind, value) in pairs {
        bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
        bytes.extend_from_slice(key.as_bytes());
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(value);
    }
    bytes
}

/// Review M5: the pinned llama.cpp loader opens the sibling shards a
/// GGUF's `split.count` names, files the descriptor never hashed. Given the
/// first shard of two (the only GGUF the descriptor lists) with the second
/// beside it in the granted model directory, the real worker refuses before
/// llama.cpp loads anything.
#[cfg(feature = "embed-worker")]
#[test]
fn the_real_worker_refuses_a_split_gguf_before_loading() {
    const FIRST: &str = "model-00001-of-00002.gguf";
    let shard = |no: u16| {
        let mut architecture = (6u64).to_le_bytes().to_vec();
        architecture.extend_from_slice(b"gemma3");
        gguf_header(&[
            ("general.architecture", 8, architecture),
            ("split.no", 2, no.to_le_bytes().to_vec()),
            ("split.count", 2, 2u16.to_le_bytes().to_vec()),
        ])
    };
    let (first, second) = (shard(0), shard(1));
    let mut descriptor = fake_descriptor();
    for file in &mut descriptor.artifact_files {
        if file.name == descriptor.gguf {
            file.name = FIRST.into();
        }
    }
    descriptor.gguf = FIRST.into();
    let (answer, code) = real_worker_answer_over(
        &descriptor,
        &[(FIRST, &first), ("model-00002-of-00002.gguf", &second)],
    );
    match answer {
        Some(Header::Error {
            id: None,
            code: named,
            message,
            ..
        }) => {
            assert_eq!(named, "load_failed");
            assert!(message.contains("split.count 2"), "{message}");
        }
        other => panic!("a split GGUF must be refused before loading: {other:?}"),
    }
    assert_eq!(code, Some(1));
}

/// A profile whose descriptor also names `zz-big.bin`, an 8 GiB sparse file:
/// hashing it takes seconds, and its digest cannot match.
fn big_artifact_profile(dir: &Path, load_timeout: u64) -> SemanticProfile {
    let (bundle, sha) = fake_bundle(dir);
    let mut profile = fake_profile(dir, &bundle, &sha, 3 << 30, load_timeout);
    let file = std::fs::File::create(profile.model_dir.join("zz-big.bin")).expect("big artifact");
    file.set_len(8 << 30).expect("sparse 8 GiB artifact");
    profile
        .descriptor
        .artifact_files
        .push(context_foundry::neural::provider::ArtifactFile {
            name: "zz-big.bin".into(),
            sha256: "0".repeat(64),
        });
    profile
}

#[test]
fn verifying_a_large_artifact_stops_at_the_control_deadline() {
    let dir = tempfile::tempdir().expect("tempdir");
    let profile = big_artifact_profile(dir.path(), 600);
    let control =
        context_foundry::Control::with_deadline(Instant::now() + Duration::from_millis(300));
    let started = Instant::now();
    let error = refused(
        supervisor::acquire_until(&profile, true, &control),
        "verification past the control deadline",
    );
    assert!(matches!(error, ProviderError::Timeout), "got {error:?}");
    assert!(
        started.elapsed() < Duration::from_millis(1500),
        "verification ran {:?} past a 300 ms deadline",
        started.elapsed()
    );
    assert!(
        !profile.worker.scratch_root.exists(),
        "nothing was launched"
    );
}

#[test]
fn cancelling_stops_the_verification_of_a_large_artifact() {
    let dir = tempfile::tempdir().expect("tempdir");
    let profile = big_artifact_profile(dir.path(), 600);
    let control = context_foundry::Control::unbounded();
    let flag = control.cancel_flag();
    let cancelled_at = std::sync::Arc::new(std::sync::Mutex::new(None::<Instant>));
    let marker = std::sync::Arc::clone(&cancelled_at);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        *marker.lock().expect("marker") = Some(Instant::now());
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let error = refused(
        supervisor::acquire_until(&profile, true, &control),
        "a cancelled verification",
    );
    assert!(matches!(error, ProviderError::Cancelled), "got {error:?}");
    let promptness = cancelled_at
        .lock()
        .expect("marker")
        .expect("the cancel thread ran")
        .elapsed();
    assert!(
        promptness < Duration::from_millis(1500),
        "verification kept running {promptness:?} after the cancellation"
    );
    assert!(
        !profile.worker.scratch_root.exists(),
        "nothing was launched"
    );
}

#[test]
fn the_profile_load_timeout_bounds_verification_from_the_entry_of_the_acquisition() {
    // An unbounded caller control: only the profile's 1 s load timeout, which
    // starts at entry and so covers the hashing, can stop an 8 GiB read.
    let dir = tempfile::tempdir().expect("tempdir");
    let profile = big_artifact_profile(dir.path(), 1);
    let started = Instant::now();
    let error = refused(
        supervisor::acquire_until(&profile, true, &context_foundry::Control::unbounded()),
        "verification past the load timeout",
    );
    assert!(matches!(error, ProviderError::Timeout), "got {error:?}");
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "a 1 s acquisition bound took {:?}",
        started.elapsed()
    );
    assert!(
        !profile.worker.scratch_root.exists(),
        "nothing was launched"
    );
}

/// The embed request IDs a worker received (`--request-log`), oldest first.
fn request_ids(log: &Path) -> Vec<u64> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

/// A provider whose worker answers its first request `busy` (without running
/// it) and logs every request it receives.
fn busy_once_provider(dir: &Path) -> (WorkerProvider, PathBuf) {
    let (bundle, sha) = fake_bundle(dir);
    let profile = fake_profile(dir, &bundle, &sha, 3 << 30, 60);
    let log = dir.join("requests.log");
    let provider = launch(
        &profile,
        &[
            "--busy-count",
            "1",
            "--request-log",
            &log.display().to_string(),
        ],
    )
    .expect("acquire");
    (provider, log)
}

#[test]
fn a_budget_that_expires_between_busy_and_the_retry_sends_nothing_more() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut provider, log) = busy_once_provider(dir.path());
    // The retry sleep (600 ms) outlasts the 250 ms budget.
    provider.set_timing(Duration::from_millis(50), Duration::from_millis(600));
    let control =
        context_foundry::Control::with_deadline(Instant::now() + Duration::from_millis(250));
    let result = provider.embed_documents(&[input(&[1])], &control);
    assert!(
        matches!(result, Err(ProviderError::Timeout)),
        "got {result:?}"
    );
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        request_ids(&log),
        vec![1],
        "a second request was dispatched after the budget expired"
    );
    // The worker is untouched and serves the next caller.
    let next = provider
        .embed_documents(&[input(&[2])], &context_foundry::Control::unbounded())
        .expect("the next call is served");
    assert_eq!(
        next[0],
        worker_runtime::deterministic_vector(&[2], DIMENSIONS)
    );
}

#[test]
fn a_query_deadline_that_passes_during_the_busy_retry_sends_nothing_more() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut provider, log) = busy_once_provider(dir.path());
    provider.set_timing(Duration::from_millis(50), Duration::from_millis(600));
    let result = provider.embed_query(&input(&[1]), Instant::now() + Duration::from_millis(250));
    assert!(
        matches!(result, Err(ProviderError::Timeout)),
        "got {result:?}"
    );
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        request_ids(&log),
        vec![1],
        "a second request was dispatched after the deadline passed"
    );
}

#[test]
fn a_cancellation_between_busy_and_the_retry_sends_nothing_more() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut provider, log) = busy_once_provider(dir.path());
    provider.set_timing(Duration::from_millis(50), Duration::from_millis(600));
    let control = context_foundry::Control::unbounded();
    let flag = control.cancel_flag();
    std::thread::spawn(move || {
        // Lands inside the 600 ms retry sleep that follows the first `busy`.
        std::thread::sleep(Duration::from_millis(150));
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let result = provider.embed_documents(&[input(&[1])], &control);
    assert!(
        matches!(result, Err(ProviderError::Cancelled)),
        "got {result:?}"
    );
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        request_ids(&log),
        vec![1],
        "a second request was dispatched after the cancellation"
    );
}

#[test]
fn a_reply_that_crosses_the_deadline_inside_one_receive_slice_is_discarded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let mut provider = launch(&profile, &["--slow-ms", "400"]).expect("acquire");
    // One receive slice (3 s) spans the 150 ms deadline AND the 400 ms reply.
    provider.set_timing(Duration::from_secs(3), Duration::from_millis(5));
    let control =
        context_foundry::Control::with_deadline(Instant::now() + Duration::from_millis(150));
    let result = provider.embed_documents(&[input(&[1])], &control);
    assert!(
        matches!(result, Err(ProviderError::Timeout)),
        "a reply landing after the deadline must not be returned: {result:?}"
    );
    // The reply was consumed and discarded: the slot is free and the next
    // call gets its own vectors.
    provider.set_timing(Duration::from_millis(50), Duration::from_millis(5));
    let next = provider
        .embed_documents(&[input(&[2])], &context_foundry::Control::unbounded())
        .expect("the next call is served");
    assert_eq!(
        next[0],
        worker_runtime::deterministic_vector(&[2], DIMENSIONS)
    );
}

#[test]
fn a_reply_that_crosses_a_cancellation_inside_one_receive_slice_is_discarded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let mut provider = launch(&profile, &["--slow-ms", "400"]).expect("acquire");
    provider.set_timing(Duration::from_secs(3), Duration::from_millis(5));
    let control = context_foundry::Control::unbounded();
    let flag = control.cancel_flag();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let result = provider.embed_documents(&[input(&[1])], &control);
    assert!(
        matches!(result, Err(ProviderError::Cancelled)),
        "a reply landing after the cancellation must not be returned: {result:?}"
    );
}

/// Unix permission bits of `path` (no symlink following).
fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::symlink_metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o7777
}

#[test]
fn the_scratch_root_and_run_directories_are_owner_private_whatever_the_umask() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let root = profile.worker.scratch_root.clone();
    assert!(!root.exists());
    // A wide-open umask: only explicit modes can make these directories
    // private.
    let previous = unsafe { libc::umask(0) };
    let provider = launch(&profile, &[]);
    unsafe { libc::umask(previous) };
    let provider = provider.expect("acquire");
    assert_eq!(mode_of(&root), 0o700, "the created scratch root");
    let runs: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("scratch root")
        .map(|entry| entry.expect("entry").path())
        .collect();
    assert_eq!(runs.len(), 1, "one run directory per live worker: {runs:?}");
    assert_eq!(mode_of(&runs[0]), 0o700, "the run directory");
    assert_eq!(mode_of(&runs[0].join("tmp")), 0o700, "the run's tmp");
    drop(provider);
    assert!(root.is_dir(), "the root stays for the next run");
    assert_eq!(
        std::fs::read_dir(&root).expect("scratch root").count(),
        0,
        "the run directory is removed once the worker is reaped"
    );
    // A pre-existing owner-private root is accepted unchanged.
    let again = launch(&profile, &[]).expect("a private pre-existing root");
    assert_eq!(mode_of(&root), 0o700);
    drop(again);
}

#[test]
fn an_unsafe_scratch_root_is_refused_before_anything_starts() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    type Prepare = fn(&Path, &Path);
    let cases: Vec<(&str, Prepare)> = vec![
        ("a symlink to a private directory", |root, dir| {
            let real = dir.join("real-root");
            std::fs::create_dir(&real).expect("real root");
            std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o700)).expect("chmod");
            symlink(&real, root).expect("symlink root");
        }),
        ("a group-writable directory", |root, _| {
            std::fs::create_dir(root).expect("root");
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o770)).expect("chmod");
        }),
        ("a world-writable directory", |root, _| {
            std::fs::create_dir(root).expect("root");
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o777)).expect("chmod");
        }),
        ("a regular file", |root, _| {
            std::fs::write(root, b"not a directory").expect("file");
        }),
    ];
    for (what, prepare) in cases {
        let dir = tempfile::tempdir().expect("tempdir");
        let (bundle, sha) = fake_bundle(dir.path());
        let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
        prepare(&profile.worker.scratch_root, dir.path());
        let pid_file = dir.path().join("worker.pid");
        let error = refused(
            launch(&profile, &["--pid-file", &pid_file.display().to_string()]),
            what,
        );
        assert!(
            matches!(&error, ProviderError::IsolationUnavailable(m) if m.contains("scratch root")),
            "{what}: got {error:?}"
        );
        assert!(!pid_file.exists(), "{what}: a worker was started anyway");
    }

    // A root another user owns (the system's own directory), unless this
    // test runs as that user.
    if unsafe { libc::geteuid() } != 0 {
        let dir = tempfile::tempdir().expect("tempdir");
        let (bundle, sha) = fake_bundle(dir.path());
        let mut profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
        profile.worker.scratch_root = PathBuf::from("/usr");
        let error = refused(launch(&profile, &[]), "a root owned by another user");
        assert!(
            matches!(&error, ProviderError::IsolationUnavailable(m) if m.contains("owned by")),
            "got {error:?}"
        );
    }

    // A root whose parent does not exist is not created.
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let mut profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    profile.worker.scratch_root = dir.path().join("missing-parent/scratch-root");
    let error = refused(launch(&profile, &[]), "a root without a parent");
    assert!(
        matches!(&error, ProviderError::IsolationUnavailable(_)),
        "got {error:?}"
    );
    assert!(!dir.path().join("missing-parent").exists());
}

/// A SIGKILLed owner never removes its scratch run directory; the next
/// launch under the same scratch root reclaims it (009 and 013 share the
/// rule). Only a real directory named exactly `w-<pid>-<nanos>` whose pid
/// is no process goes, contents and all; a live owner's run, any other
/// name, a regular file and a symlink (and its target) stay untouched.
#[test]
fn a_launch_reclaims_run_directories_of_dead_owners_and_nothing_else() {
    use std::collections::BTreeSet;
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let root = profile.worker.scratch_root.clone();
    std::fs::create_dir(&root).expect("scratch root");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    // A pid that names no process: a child that exited and was reaped.
    let mut exited = Command::new("/usr/bin/true").spawn().expect("spawn");
    let dead = exited.id();
    exited.wait().expect("reap");
    // A live owner other than this process.
    let mut live = Command::new("/bin/sleep").arg("60").spawn().expect("spawn");
    let dead_run = root.join(format!("w-{dead}-1759730000000000000"));
    std::fs::create_dir_all(dead_run.join("tmp/nested")).expect("dead run");
    std::fs::write(dead_run.join("head.safetensors"), vec![0u8; 1 << 20]).expect("staged head");
    std::fs::write(dead_run.join("tmp/nested/file"), b"x").expect("nested file");
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).expect("outside");
    std::fs::write(outside.join("kept"), b"kept").expect("outside file");
    let kept_dirs = [
        format!("w-{}-1759730000000000000", live.id()),
        format!("w-{}-1", std::process::id()),
        format!("w-{dead}"),
        format!("w-{dead}-1-2"),
        format!("w-0{dead}-1"),
        format!("w-{dead}-1x"),
        format!("x-{dead}-1"),
    ];
    for name in &kept_dirs {
        std::fs::create_dir(root.join(name)).expect("kept directory");
        std::fs::write(root.join(name).join("file"), b"kept").expect("kept file");
    }
    let file = format!("w-{dead}-2");
    std::fs::write(root.join(&file), b"a regular file").expect("regular file");
    let link = format!("w-{dead}-3");
    symlink(&outside, root.join(&link)).expect("symlink");
    let names = || -> BTreeSet<String> {
        std::fs::read_dir(&root)
            .expect("scratch root")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .into_string()
                    .expect("UTF-8")
            })
            .collect()
    };
    let planted = names();

    let provider = launch(&profile, &[]).expect("acquire");
    let during = names();
    drop(provider);
    let after = names();
    let _ = live.kill();
    let _ = live.wait();

    assert!(
        !dead_run.exists(),
        "the dead owner's run directory was reclaimed"
    );
    let mut expected = planted.clone();
    expected.remove(&format!("w-{dead}-1759730000000000000"));
    assert_eq!(after, expected, "nothing else under the root was touched");
    assert_eq!(
        during.difference(&expected).count(),
        1,
        "the launch's own run directory: {during:?}"
    );
    for name in &kept_dirs {
        assert_eq!(
            std::fs::read(root.join(name).join("file")).expect("kept file"),
            b"kept"
        );
    }
    assert_eq!(
        std::fs::read(root.join(&file)).expect("regular file"),
        b"a regular file"
    );
    assert!(
        std::fs::symlink_metadata(root.join(&link))
            .expect("symlink")
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        std::fs::read(outside.join("kept")).expect("the link's target"),
        b"kept"
    );
}

/// A dead owner's run directory under the scratch root of `profile`, with
/// one file inside; returns its path. The pid is a reaped child's.
fn plant_dead_run(profile: &SemanticProfile, nanos: u64) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let root = &profile.worker.scratch_root;
    if !root.exists() {
        std::fs::create_dir(root).expect("scratch root");
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    }
    let mut exited = Command::new("/usr/bin/true").spawn().expect("spawn");
    let dead = exited.id();
    exited.wait().expect("reap");
    let run = root.join(format!("w-{dead}-{nanos}"));
    std::fs::create_dir_all(run.join("tmp")).expect("dead run");
    std::fs::write(run.join("head.safetensors"), b"staged").expect("staged head");
    run
}

/// Review M5: reclamation runs under the launch's control. Barrier: the
/// `supervisor.scratch_reclaim_validated` fault point cancels the launch
/// right after the first dead run was validated. Nothing more is claimed or
/// removed, no run directory is created and no worker is started: the dead
/// runs stay for a later launch.
#[test]
fn a_launch_stopped_during_reclamation_leaves_the_rest_and_starts_nothing() {
    use context_foundry::fault::{self, Action};
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let first = plant_dead_run(&profile, 1);
    let second = plant_dead_run(&profile, 2);
    let pid_file = dir.path().join("worker.pid");
    fault::arm(
        supervisor::fault_names::SCRATCH_RECLAIM_VALIDATED,
        0,
        Action::Cancel,
    );
    let error = refused(
        launch(&profile, &["--pid-file", &pid_file.display().to_string()]),
        "a launch cancelled during reclamation",
    );
    fault::disarm_all();
    assert!(matches!(error, ProviderError::Cancelled), "got {error:?}");
    assert!(!pid_file.exists(), "no worker was started");
    assert!(
        first.join("head.safetensors").exists(),
        "nothing was removed"
    );
    assert!(
        second.join("head.safetensors").exists(),
        "nothing was removed"
    );
    assert_eq!(
        std::fs::read_dir(&profile.worker.scratch_root)
            .expect("scratch root")
            .count(),
        2,
        "no run directory was created"
    );
}

/// The names under `profile`'s scratch root, sorted.
fn scratch_names(profile: &SemanticProfile) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(&profile.worker.scratch_root)
        .expect("scratch root")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("UTF-8")
        })
        .collect();
    names.sort();
    names
}

/// Review M5, round 3: the scratch root's listing runs under the launch's
/// control step by step, not only between candidates. Barrier: the
/// `supervisor.scratch_reclaim_entry` fault point cancels the launch at one
/// step of that listing (32 entries), once while its names are read and
/// once while their kinds are checked. The listing ends at that very step,
/// nothing is claimed or removed, no run directory is created and no worker
/// is started.
#[test]
fn a_launch_cancelled_while_listing_the_scratch_root_stops_at_that_step() {
    use context_foundry::fault::{self, Action};
    const OTHERS: usize = 30;
    // The names pass reads 32 names and the end of the listing (plus `.`
    // and `..`); the kinds pass follows with 32 checks.
    for (pass, step) in [("names", 1), ("kinds", 2 * OTHERS)] {
        let dir = tempfile::tempdir().expect("tempdir");
        let (bundle, sha) = fake_bundle(dir.path());
        let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
        let runs = [plant_dead_run(&profile, 1), plant_dead_run(&profile, 2)];
        for n in 0..OTHERS {
            let other = profile.worker.scratch_root.join(format!("other-{n}"));
            std::fs::write(other, b"kept").expect("another entry");
        }
        let planted = scratch_names(&profile);
        let pid_file = dir.path().join("worker.pid");
        fault::arm(
            supervisor::fault_names::SCRATCH_RECLAIM_ENTRY,
            step,
            Action::Cancel,
        );
        let error = refused(
            launch(&profile, &["--pid-file", &pid_file.display().to_string()]),
            "a launch cancelled while listing the scratch root",
        );
        let steps = fault::reached(supervisor::fault_names::SCRATCH_RECLAIM_ENTRY);
        fault::disarm_all();
        assert!(
            matches!(error, ProviderError::Cancelled),
            "{pass}: got {error:?}"
        );
        assert_eq!(
            steps,
            step + 1,
            "{pass}: no step ran after the cancelled one"
        );
        assert!(!pid_file.exists(), "{pass}: no worker was started");
        for run in &runs {
            assert_eq!(
                std::fs::read(run.join("head.safetensors")).expect("the dead run"),
                b"staged",
                "{pass}: nothing was claimed or removed"
            );
        }
        assert_eq!(
            scratch_names(&profile),
            planted,
            "{pass}: no run directory was created"
        );
    }
}

/// Review M5, round 3: a claimed dead run's tree is listed under the
/// launch's control too. Barrier: the `supervisor.scratch_reclaim_entry`
/// fault point cancels the launch at one step of the claimed tree's listing
/// (scope `run`, 32 entries), once while its names are read and once while
/// their kinds are checked. The listing ends at that very step, nothing in
/// the tree is removed and no worker is started. Review M6: the claimed tree
/// stays under its quarantine name `.reclaim-<pid>-<nanos>`, never under a
/// run name, and a later launch leaves it alone.
#[test]
fn a_launch_cancelled_while_listing_a_claimed_run_stops_at_that_step() {
    use context_foundry::fault::{self, Action, Ctx};
    use std::{cell::Cell, rc::Rc};
    const FILES: usize = 30;
    for (pass, step) in [("names", 1), ("kinds", 2 * FILES)] {
        let dir = tempfile::tempdir().expect("tempdir");
        let (bundle, sha) = fake_bundle(dir.path());
        let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
        // `tmp`, `head.safetensors` and FILES files.
        let run = plant_dead_run(&profile, 1);
        for n in 0..FILES {
            std::fs::write(run.join(format!("file-{n}")), b"kept").expect("a run file");
        }
        let pid_file = dir.path().join("worker.pid");
        let seen = Rc::new(Cell::new(0usize));
        fault::arm(
            supervisor::fault_names::SCRATCH_RECLAIM_ENTRY,
            0,
            Action::Call(Box::new({
                let seen = Rc::clone(&seen);
                move |ctx: &Ctx<'_>| {
                    if ctx.detail != "run" {
                        return;
                    }
                    if seen.get() == step {
                        ctx.control.expect("the launch's control").cancel();
                    }
                    seen.set(seen.get() + 1);
                }
            })),
        );
        let error = refused(
            launch(&profile, &["--pid-file", &pid_file.display().to_string()]),
            "a launch cancelled while listing a claimed run",
        );
        fault::disarm_all();
        assert!(
            matches!(error, ProviderError::Cancelled),
            "{pass}: got {error:?}"
        );
        assert_eq!(
            seen.get(),
            step + 1,
            "{pass}: no step ran after the cancelled one"
        );
        assert!(!pid_file.exists(), "{pass}: no worker was started");
        assert!(!run.exists(), "{pass}: the dead run was claimed");
        let names = scratch_names(&profile);
        let [claimed] = names.as_slice() else {
            panic!("{pass}: only the claimed tree: {names:?}");
        };
        assert!(
            claimed.starts_with(&format!(".reclaim-{}-", std::process::id())),
            "{pass}: {claimed}"
        );
        let claimed = profile.worker.scratch_root.join(claimed);
        let intact = || {
            std::fs::read_dir(&claimed)
                .expect("the claimed tree")
                .count()
                == FILES + 2
                && std::fs::read(claimed.join("head.safetensors")).expect("staged head")
                    == b"staged"
        };
        assert!(intact(), "{pass}: nothing in the claimed tree was removed");
        drop(launch(&profile, &[]).expect("a later launch"));
        assert_eq!(
            scratch_names(&profile),
            names,
            "{pass}: the later launch left the quarantined tree"
        );
        assert!(
            intact(),
            "{pass}: the later launch left the quarantined tree"
        );
    }
}

/// Review M6: the dead run that passed validation is replaced, under its
/// name, by an empty foreign directory before it is claimed. Barrier: the
/// `supervisor.scratch_reclaim_validated` fault point moves the validated
/// directory out of the root and puts the foreign one in its place. The
/// claim finds another inode than the validated one and gives the name back:
/// the foreign directory survives, and the moved original is untouched.
#[test]
fn a_dead_run_replaced_after_validation_is_never_removed() {
    use context_foundry::fault::{self, Action};
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let run = plant_dead_run(&profile, 1);
    let moved = dir.path().join("moved-away");
    fault::arm(
        supervisor::fault_names::SCRATCH_RECLAIM_VALIDATED,
        0,
        Action::Call(Box::new({
            let (run, moved) = (run.clone(), moved.clone());
            move |_| {
                std::fs::rename(&run, &moved).expect("move the validated run away");
                std::fs::create_dir(&run).expect("the foreign replacement");
            }
        })),
    );
    let provider = launch(&profile, &[]);
    fault::disarm_all();
    drop(provider.expect("acquire"));
    assert!(run.is_dir(), "the foreign replacement survived");
    assert_eq!(
        std::fs::read_dir(&run).expect("replacement").count(),
        0,
        "the foreign replacement is untouched"
    );
    assert_eq!(
        std::fs::read(moved.join("head.safetensors")).expect("the moved original"),
        b"staged",
        "the moved original is untouched"
    );
    let names: Vec<_> = std::fs::read_dir(&profile.worker.scratch_root)
        .expect("scratch root")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(names, [run.file_name().unwrap()], "{names:?}");
}

/// Review M6, round 3: as above, but once the claim moved the foreign
/// directory (with data) to its quarantine name, the run's name is taken
/// again, so the claim cannot be given back. Barriers: the
/// `supervisor.scratch_reclaim_validated` fault point swaps the validated run
/// for the foreign directory; `supervisor.scratch_reclaim_claimed` puts a new
/// directory under the run's name. The foreign directory stays, with its
/// data, under `.reclaim-<pid>-<nanos>`, outside the run namespace: a later
/// launch leaves it alone, and so it does a `.reclaim-` entry whose claiming
/// process is gone (planted with a dead pid). That later launch does reclaim
/// the directory under the dead run's name.
#[test]
fn a_claim_that_cannot_be_given_back_is_never_removed() {
    use context_foundry::fault::{self, Action};
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let root = profile.worker.scratch_root.clone();
    let run = plant_dead_run(&profile, 1);
    let moved = dir.path().join("moved-away");
    fault::arm(
        supervisor::fault_names::SCRATCH_RECLAIM_VALIDATED,
        0,
        Action::Call(Box::new({
            let (run, moved) = (run.clone(), moved.clone());
            move |_| {
                std::fs::rename(&run, &moved).expect("move the validated run away");
                std::fs::create_dir(&run).expect("the foreign replacement");
                std::fs::write(run.join("data"), b"foreign").expect("the foreign data");
            }
        })),
    );
    fault::arm(
        supervisor::fault_names::SCRATCH_RECLAIM_CLAIMED,
        0,
        Action::Call(Box::new({
            let run = run.clone();
            move |_| std::fs::create_dir(&run).expect("the run's name taken again")
        })),
    );
    let provider = launch(&profile, &[]);
    fault::disarm_all();
    drop(provider.expect("acquire"));
    let names = scratch_names(&profile);
    let [held, taken] = names.as_slice() else {
        panic!("the quarantined foreign directory and the run's name: {names:?}");
    };
    assert!(
        held.starts_with(&format!(".reclaim-{}-", std::process::id())),
        "{names:?}"
    );
    assert_eq!(root.join(taken), run, "{names:?}");
    assert_eq!(
        std::fs::read(root.join(held).join("data")).expect("the foreign data"),
        b"foreign"
    );
    // The claiming process is gone: a dead pid's quarantine entry.
    let mut exited = Command::new("/usr/bin/true").spawn().expect("spawn");
    let dead = exited.id();
    exited.wait().expect("reap");
    let orphan = format!(".reclaim-{dead}-1");
    std::fs::create_dir(root.join(&orphan)).expect("an orphaned claim");
    std::fs::write(root.join(&orphan).join("data"), b"orphan").expect("its data");

    drop(launch(&profile, &[]).expect("a later launch"));
    assert!(
        !run.exists(),
        "the later launch reclaimed what held the dead run's name"
    );
    let mut kept = vec![held.clone(), orphan.clone()];
    kept.sort();
    assert_eq!(scratch_names(&profile), kept);
    assert_eq!(
        std::fs::read(root.join(held).join("data")).expect("the foreign data"),
        b"foreign"
    );
    assert_eq!(
        std::fs::read(root.join(&orphan).join("data")).expect("the orphan's data"),
        b"orphan"
    );
    assert_eq!(
        std::fs::read(moved.join("head.safetensors")).expect("the moved original"),
        b"staged",
        "the moved original is untouched"
    );
}

/// Gaps review M3: the resident worker dies during a query. The query
/// falls back by name (`fallback:provider_exited`), and the owner's status
/// names that failure as its runtime word without calling the model.
/// Barrier: the fake worker reports its `call` phase and holds the call
/// (`--hold-file`) until it is killed.
#[cfg(feature = "semantic")]
#[test]
fn a_worker_dying_during_a_query_is_named_by_the_owners_status() {
    use context_foundry::neural::provider::{FunctionDescriptor, LateCall};
    use context_foundry::neural::query::{Fallback, QueryRuntime, fallback_word};
    use std::sync::Arc;

    struct Relabeled {
        worker: WorkerProvider,
        descriptor: FunctionDescriptor,
    }
    impl EmbeddingProvider for Relabeled {
        fn descriptor(&self) -> &FunctionDescriptor {
            &self.descriptor
        }
        fn embed_documents(
            &mut self,
            batch: &[TokenizedInput],
            control: &context_foundry::Control,
        ) -> Result<Vec<Vec<f32>>, ProviderError> {
            self.worker.embed_documents(batch, control)
        }
        fn embed_query(
            &mut self,
            input: &TokenizedInput,
            deadline: Instant,
        ) -> Result<Vec<f32>, ProviderError> {
            self.worker.embed_query(input, deadline)
        }
        fn late_call(&self) -> Option<LateCall> {
            self.worker.late_call()
        }
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let worker_profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let runtime_profile = Arc::new(
        SemanticProfile::load(&context_foundry::testkit::write_semantic_profile(
            &dir.path().join("runtime"),
            "runtime",
            |_| {},
        ))
        .expect("the runtime profile"),
    );
    let (hold, phases, pid_file) = (
        dir.path().join("hold"),
        dir.path().join("phases"),
        dir.path().join("worker.pid"),
    );
    std::fs::write(&hold, b"").expect("hold file");
    let hooks = vec![
        "--hold-file".to_owned(),
        hold.display().to_string(),
        "--phase-file".to_owned(),
        phases.display().to_string(),
        "--pid-file".to_owned(),
        pid_file.display().to_string(),
    ];
    let descriptor = runtime_profile.descriptor.clone();
    let runtime = Arc::new(
        QueryRuntime::start(
            Arc::clone(&runtime_profile),
            Box::new(move || {
                let worker = WorkerProvider::launch(&worker_profile, hooks)?;
                Ok(Box::new(Relabeled { worker, descriptor }) as Box<dyn EmbeddingProvider>)
            }),
        )
        .expect("the runtime starts"),
    );
    assert_eq!(runtime.status_word(), "ready");
    let query = {
        let runtime = Arc::clone(&runtime);
        std::thread::spawn(move || {
            runtime.embed("twilight onset", Instant::now() + Duration::from_secs(30))
        })
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    while !std::fs::read_to_string(&phases)
        .unwrap_or_default()
        .lines()
        .any(|phase| phase == "call")
    {
        assert!(Instant::now() < deadline, "the worker never ran the query");
        std::thread::sleep(Duration::from_millis(2));
    }
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .expect("worker pid")
        .trim()
        .parse()
        .expect("pid");
    unsafe { libc::kill(pid, libc::SIGKILL) };
    let error = query
        .join()
        .unwrap()
        .expect_err("the query fails with the worker");
    assert!(
        matches!(error, ProviderError::WorkerExited(_)),
        "got {error:?}"
    );
    assert!(
        fallback_word(&Fallback::from(error).to_string()).starts_with("fallback:provider_exited"),
        "the request's named fallback"
    );
    assert_eq!(runtime.status_word(), "fallback:provider_exited");
    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).expect("workspace");
    let engine =
        context_foundry::Engine::initialize(&dir.path().join("store"), &workspace).expect("store");
    let status = context_foundry::neural::driver::status_object(
        &engine,
        None,
        &runtime.status_word(),
        &context_foundry::Control::unbounded(),
    )
    .expect("status");
    assert_eq!(status["runtime"], "fallback:provider_exited", "{status}");
    runtime.shutdown();
}

/// Serializes tests that widen the readiness receive slice (or assert
/// launch promptness against it), so the process-wide override cannot land
/// in another test's launch.
static LAUNCH_TIMING_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The readiness receive is capped by the BOUNDED control (the earlier of
/// the caller's deadline and the profile's 60 s load bound): the
/// acquisition wakes at the caller's deadline, not at the worker's ready
/// (500 ms after the deadline at the earliest) and not at the end of the
/// 30 s receive slice, and that ready is never accepted.
///
/// The claim needs the deadline to land INSIDE that receive, with the
/// worker started. The same deadline also covers the verification before
/// the spawn: the profile, its artifacts and the SHA-256 of the 5 MB debug
/// fake, measured 2026-10-06 at 200-260 ms unloaded and past 400 ms under a
/// parallel suite. Only an attempt whose launch ENTERED the readiness
/// receive with time left (the `supervisor.ready_receive_entered` fault
/// point) and whose worker wrote its PID proves the claim; any other attempt
/// proves nothing about the receive, and the next one gives the start four
/// times the headroom. Every attempt must still time out with nothing
/// accepted.
#[test]
fn a_ready_crossing_the_deadline_inside_one_receive_slice_is_never_accepted() {
    let _guard = LAUNCH_TIMING_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    use context_foundry::fault::{self, Action};
    use std::{cell::Cell, rc::Rc};
    let mut reached_the_receive = false;
    for deadline in [400, 1600, 6400].map(Duration::from_millis) {
        let entered = Rc::new(Cell::new(false));
        fault::arm(
            supervisor::fault_names::READY_RECEIVE_ENTERED,
            0,
            Action::Call(Box::new({
                let entered = Rc::clone(&entered);
                move |_| entered.set(true)
            })),
        );
        let dir = tempfile::tempdir().expect("tempdir");
        let (bundle, sha) = fake_bundle(dir.path());
        let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
        let pid_file = dir.path().join("worker.pid");
        let load = deadline + Duration::from_millis(500);
        supervisor::set_launch_wait_slice(Duration::from_secs(30));
        let control = context_foundry::Control::with_deadline(Instant::now() + deadline);
        let started = Instant::now();
        let error = refused(
            WorkerProvider::launch_until(
                &profile,
                vec![
                    "--load-ms".into(),
                    load.as_millis().to_string(),
                    "--pid-file".into(),
                    pid_file.display().to_string(),
                ],
                &control,
            ),
            "a ready past the deadline",
        );
        let took = started.elapsed();
        supervisor::set_launch_wait_slice(Duration::ZERO);
        fault::disarm_all();
        assert!(matches!(error, ProviderError::Timeout), "got {error:?}");
        if !entered.get() || !pid_file.exists() {
            continue;
        }
        // It wakes at its deadline: not before it, and before the ready
        // (`load`, 500 ms later) and the 30 s slice end. Scheduler delay on a
        // loaded host stays inside that window.
        assert!(
            took + Duration::from_millis(50) >= deadline && took < load,
            "the acquisition must wake at its {deadline:?} deadline, took {took:?} \
             (the ready comes {load:?} after the worker's start, the slice ends at 30 s)"
        );
        assert_worker_reaped(&pid_file);
        reached_the_receive = true;
        break;
    }
    assert!(
        reached_the_receive,
        "no attempt started the worker before its deadline, the last one 6.4 s"
    );
}

#[test]
fn a_ready_crossing_a_cancellation_inside_one_receive_slice_is_discarded() {
    let _guard = LAUNCH_TIMING_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    let pid_file = dir.path().join("worker.pid");
    // No deadline: the receive cannot be bounded by time, so only the check
    // at CONSUMING the ready frame can notice the cancellation that landed
    // while the (wide) receive slept.
    supervisor::set_launch_wait_slice(Duration::from_secs(5));
    let control = context_foundry::Control::unbounded();
    let flag = control.cancel_flag();
    let watched = pid_file.clone();
    std::thread::spawn(move || {
        let give_up = Instant::now() + Duration::from_secs(20);
        while !watched.exists() && Instant::now() < give_up {
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(50));
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let error = refused(
        WorkerProvider::launch_until(
            &profile,
            vec![
                "--load-ms".into(),
                "900".into(),
                "--pid-file".into(),
                pid_file.display().to_string(),
            ],
            &control,
        ),
        "a ready past a cancellation",
    );
    supervisor::set_launch_wait_slice(Duration::ZERO);
    assert!(
        matches!(error, ProviderError::Cancelled),
        "a ready consumed after the caller cancelled must not be accepted: {error:?}"
    );
    assert_worker_reaped(&pid_file);
}

/// Review M7: the acquisition's deadline passes between the readiness
/// wait's control check and its receive-slice computation. Barrier: the
/// `supervisor.ready_slice` fault point holds the launch there, after the
/// worker wrote its PID, until the deadline has passed. The slice is then
/// zero: the acquisition times out at once, never enters the receive (30 s
/// slice; the worker's ready would come only after 60 s) and reaps the
/// worker. An attempt whose launch stopped before the readiness wait (its
/// verification overran the deadline under load) never reaches the fault
/// point and proves nothing; the next one gives the start four times the
/// headroom.
#[test]
fn a_deadline_passing_before_the_receive_slice_enters_no_receive() {
    let _guard = LAUNCH_TIMING_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    use context_foundry::fault::{self, Action, Ctx};
    let mut crossed = false;
    for deadline in [400, 1600, 6400].map(Duration::from_millis) {
        let dir = tempfile::tempdir().expect("tempdir");
        let (bundle, sha) = fake_bundle(dir.path());
        let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
        let pid_file = dir.path().join("worker.pid");
        fault::arm(
            supervisor::fault_names::READY_SLICE,
            0,
            Action::Call(Box::new({
                let pid_file = pid_file.clone();
                move |ctx: &Ctx<'_>| {
                    let at = ctx
                        .control
                        .and_then(context_foundry::Control::deadline)
                        .expect("the acquisition's bound");
                    let give_up = Instant::now() + Duration::from_secs(20);
                    while !pid_file.exists() && Instant::now() < give_up {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    while Instant::now() < at {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                }
            })),
        );
        supervisor::set_launch_wait_slice(Duration::from_secs(30));
        let control = context_foundry::Control::with_deadline(Instant::now() + deadline);
        let started = Instant::now();
        let error = refused(
            WorkerProvider::launch_until(
                &profile,
                vec![
                    "--load-ms".into(),
                    "60000".into(),
                    "--pid-file".into(),
                    pid_file.display().to_string(),
                ],
                &control,
            ),
            "a deadline passing before the receive slice",
        );
        let took = started.elapsed();
        supervisor::set_launch_wait_slice(Duration::ZERO);
        let held = fault::reached(supervisor::fault_names::READY_SLICE);
        let received = fault::reached(supervisor::fault_names::READY_RECEIVE_ENTERED);
        fault::disarm_all();
        assert!(matches!(error, ProviderError::Timeout), "got {error:?}");
        if held == 0 {
            continue;
        }
        assert_eq!(held, 1, "one pass of the readiness wait");
        assert_eq!(
            received, 0,
            "no receive is entered once the deadline passed"
        );
        assert!(
            took < Duration::from_secs(20),
            "the acquisition ends at its {deadline:?} deadline, not at the 30 s slice: \
             took {took:?}"
        );
        assert_worker_reaped(&pid_file);
        crossed = true;
        break;
    }
    assert!(
        crossed,
        "no attempt reached the readiness wait before its deadline, the last one 6.4 s"
    );
}

#[test]
fn the_scratch_root_may_not_overlap_any_read_only_grant() {
    use std::os::unix::fs::symlink;
    for (what, root_of) in [
        ("equal to the model directory", 0u8),
        ("an ancestor of the model directory", 1),
        ("inside the model directory", 2),
        ("a symlinked alias of the model directory", 3),
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        let (bundle, sha) = fake_bundle(dir.path());
        let mut profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
        match root_of {
            0 => profile.worker.scratch_root = profile.model_dir.clone(),
            1 => profile.worker.scratch_root = dir.path().to_path_buf(),
            2 => profile.worker.scratch_root = profile.model_dir.join("scratch"),
            _ => {
                let alias = dir.path().join("alias");
                symlink(&profile.model_dir, &alias).expect("alias");
                profile.worker.scratch_root = alias;
            }
        }
        assert!(
            profile.validate().is_err(),
            "{what}: the profile itself must be invalid"
        );
        let pid_file = dir.path().join("worker.pid");
        let error = refused(
            launch(&profile, &["--pid-file", &pid_file.display().to_string()]),
            what,
        );
        assert!(
            matches!(&error, ProviderError::ProfileInvalid(m) if m.contains("overlap")),
            "{what}: got {error:?}"
        );
        assert!(!pid_file.exists(), "{what}: a worker was started anyway");
    }
    // A scratch leaf that does not exist yet, beneath a chain of absolute
    // symlink aliases of the model directory (a -> absolute b -> model
    // dir): refused by resolution, not by the leaf existing.
    {
        let dir = tempfile::tempdir().expect("tempdir");
        let (bundle, sha) = fake_bundle(dir.path());
        let mut profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
        let b = dir.path().join("b");
        symlink(&profile.model_dir, &b).expect("absolute b -> model dir");
        let a = dir.path().join("a");
        symlink(&b, &a).expect("absolute a -> b");
        profile.worker.scratch_root = a.join("new-root");
        assert!(!profile.model_dir.join("new-root").exists());
        assert!(
            profile.validate().is_err(),
            "the alias chain must be refused at validation"
        );
        let pid_file = dir.path().join("worker.pid");
        let error = refused(
            launch(&profile, &["--pid-file", &pid_file.display().to_string()]),
            "an absent leaf beneath an alias chain",
        );
        assert!(
            matches!(&error, ProviderError::ProfileInvalid(m) if m.contains("overlap")),
            "got {error:?}"
        );
        assert!(
            !profile.model_dir.join("new-root").exists(),
            "nothing was created inside the read-only tree"
        );
        assert!(!pid_file.exists(), "no worker was started");
    }
    // Positive control: a disjoint private root launches (and the root the
    // helper picks is disjoint from every grant by construction).
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 60);
    assert!(profile.validate().is_ok());
    drop(launch(&profile, &[]).expect("a disjoint scratch root launches"));
}

#[test]
fn an_overlarge_load_timeout_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (bundle, sha) = fake_bundle(dir.path());
    let mut profile = fake_profile(dir.path(), &bundle, &sha, 3 << 30, 3601);
    let error = refused(launch(&profile, &[]), "an over-large load timeout");
    assert!(
        matches!(&error, ProviderError::ProfileInvalid(m) if m.contains("3600")),
        "got {error:?}"
    );
    // The documented maximum itself is fine.
    profile.load_timeout_seconds = 3600;
    assert!(profile.validate().is_ok());
}

/// The bundle script refuses to sign when the scratch root overlaps ANY
/// read-only grant, including every canonical `--extra-read` directory
/// (equal, ancestor or descendant); nothing is created or signed. The
/// disjoint case proceeds to a signed bundle.
#[test]
fn the_bundle_script_refuses_a_scratch_grant_overlapping_an_extra_read() {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("embed-worker-bundle.sh");
    let run_script = |profile_json: &str, extra_read: &Path, out: &Path| -> Option<i32> {
        Command::new(&script)
            .arg("--binary")
            .arg("/usr/bin/true")
            .arg("--out")
            .arg(out)
            .arg("--profile")
            .arg(profile_json)
            .arg("--extra-read")
            .arg(extra_read)
            .stderr(Stdio::inherit())
            .output()
            .expect("run the bundle script")
            .status
            .code()
    };
    // One refusal case: a profile and an `--extra-read` directory, both in
    // one tempdir, with the scratch root placed to overlap the extra-read
    // the named way.
    let build_case = |what: &str, place: fn(&Path) -> (PathBuf, PathBuf)| {
        let dir = tempfile::tempdir_in("/private/tmp").expect("tempdir");
        let (scratch, extra) = place(dir.path());
        let mut profile = fake_profile(dir.path(), Path::new("/nonexistent.app"), "a", 3 << 30, 60);
        profile.worker.scratch_root = scratch;
        let json = dir.path().join("profile.json");
        std::fs::write(
            &json,
            serde_json::to_string(&profile).expect("profile JSON"),
        )
        .expect("write profile");
        let out = dir.path().join("Out.app");
        let code = run_script(&json.display().to_string(), &extra, &out);
        assert_eq!(code, Some(66), "{what}: the script must refuse (exit 66)");
        assert!(!out.exists(), "{what}: nothing may be signed");
    };
    // extra-read EQUAL to the scratch root.
    build_case("extra-read equal to the scratch root", |dir| {
        let same = dir.join("extra-read");
        std::fs::create_dir(&same).expect("dir");
        (same.clone(), same)
    });
    // extra-read an ANCESTOR of the scratch root (the leaf does not exist).
    build_case("extra-read an ancestor of the scratch root", |dir| {
        let extra = dir.join("extra-read");
        std::fs::create_dir(&extra).expect("dir");
        (extra.join("scratch"), extra)
    });
    // extra-read a DESCENDANT of the scratch root.
    build_case("extra-read a descendant of the scratch root", |dir| {
        let scratch = dir.join("scratch-root");
        std::fs::create_dir(&scratch).expect("dir");
        let extra = scratch.join("extra-read");
        std::fs::create_dir(&extra).expect("dir");
        (scratch, extra)
    });

    // Positive control: a disjoint extra-read proceeds to a signed bundle.
    let dir = tempfile::tempdir_in("/private/tmp").expect("tempdir");
    let extra = dir.path().join("extra-read");
    std::fs::create_dir(&extra).expect("extra-read dir");
    let profile = fake_profile(dir.path(), Path::new("/nonexistent.app"), "a", 3 << 30, 60);
    let json = dir.path().join("profile.json");
    std::fs::write(
        &json,
        serde_json::to_string(&profile).expect("profile JSON"),
    )
    .expect("write profile");
    let out = dir.path().join("Out.app");
    let code = run_script(&json.display().to_string(), &extra, &out);
    assert_eq!(
        code,
        Some(0),
        "a disjoint extra-read must sign a bundle: {code:?}"
    );
    assert!(out.join("Contents/MacOS/foundry-embed").is_file());
}

// ---------------------------------------------------------------------------
// Development run with the real model: `--ignored`, never CI. Requires the
// bundle built by scripts/embed-worker-bundle.sh and the profile at
// CF_EMBED_DEV_PROFILE (default /private/tmp/cf-009-dev/profile.json with
// worker.executable_sha256 filled from the script's output).
// ---------------------------------------------------------------------------

/// The supervisor's worker environment (names and values), with private
/// probe scratch for HOME and TMPDIR.
fn worker_env(_profile: &SemanticProfile) -> Vec<(&'static str, String)> {
    vec![
        ("PATH", "/usr/bin:/bin".to_string()),
        ("HOME", "/private/tmp/cf-embed-probe-home".to_string()),
        ("TMPDIR", "/private/tmp/cf-embed-probe-tmp".to_string()),
    ]
}

/// The real worker's argv minus the owner PID and liveness descriptor,
/// which whoever owns the worker supplies.
fn real_worker_args(profile: &SemanticProfile) -> Vec<String> {
    vec![
        "--descriptor".to_string(),
        serde_json::to_string(&profile.descriptor).expect("descriptor JSON"),
        "--model-dir".to_string(),
        profile.model_dir.display().to_string(),
    ]
}

fn dev_profile() -> SemanticProfile {
    let path = std::env::var("CF_EMBED_DEV_PROFILE")
        .unwrap_or_else(|_| "/private/tmp/cf-009-dev/profile.json".into());
    SemanticProfile::load(std::path::Path::new(&path)).expect("load the development profile")
}

/// Text whose rendering with the profile's document template is exactly
/// `target` model tokens (template and any special tokens included), built
/// by appending words and trimming to the exact count with the same Rust
/// tokenizer the core uses.
#[cfg(feature = "semantic")]
fn text_with_tokens(target: usize, profile: &SemanticProfile) -> String {
    let tokenizer = tokenizers::Tokenizer::from_file(profile.model_dir.join("tokenizer.json"))
        .expect("tokenizer.json");
    let template = &profile.descriptor.document_template;
    let count = |text: &str| {
        tokenizer
            .encode(
                context_foundry::neural::provider::render(template, text),
                profile.descriptor.add_special_tokens,
            )
            .expect("encode")
            .get_ids()
            .len()
    };
    let mut text = String::new();
    for word in std::iter::repeat("embedding retrieval partition grammar supplies boundary ")
        .flat_map(|line| line.split(' '))
    {
        if count(&format!("{text}{word}")) >= target {
            break;
        }
        text.push_str(word);
        text.push(' ');
    }
    // Trim whole words, then drop trailing characters one at a time until
    // the count is exact (single ASCII characters are one token each).
    while count(&text) > target {
        text.pop();
    }
    while count(&text) < target {
        text.push('x');
        // Guard against the rare multi-token character inflating the count.
        while count(&text) > target {
            text.pop();
            text.push(' ');
        }
    }
    assert_eq!(count(&text), target);
    text
}

/// The profile's exact model input for `text`: rendered with the document
/// template and tokenized like the core does.
#[cfg(feature = "semantic")]
fn dev_ids(profile: &SemanticProfile, text: &str) -> TokenizedInput {
    let tokenizer = tokenizers::Tokenizer::from_file(profile.model_dir.join("tokenizer.json"))
        .expect("tokenizer.json");
    let rendered =
        context_foundry::neural::provider::render(&profile.descriptor.document_template, text);
    TokenizedInput {
        ids: tokenizer
            .encode(rendered, profile.descriptor.add_special_tokens)
            .expect("encode")
            .get_ids()
            .to_vec(),
    }
}

/// Batched against single-sequence vectors (spec 009 T004): cosine
/// >= 0.9999 per input.
#[cfg(feature = "semantic")]
fn assert_agree(name: &str, batched: &[Vec<f32>], single: &[Vec<f32>]) {
    assert_eq!(batched.len(), single.len(), "{name}: row count");
    for (i, (a, b)) in batched.iter().zip(single).enumerate() {
        assert_eq!(a.len(), b.len(), "{name} row {i}: dimension");
        let (mut dot, mut na, mut nb) = (0.0f64, 0.0f64, 0.0f64);
        for (x, y) in a.iter().zip(b) {
            dot += *x as f64 * *y as f64;
            na += *x as f64 * *x as f64;
            nb += *y as f64 * *y as f64;
        }
        let cosine = dot / (na.sqrt() * nb.sqrt());
        println!("agreement {name} row {i}: cosine={cosine:.7} (tolerance: >= 0.9999)");
        assert!(
            cosine >= 0.9999,
            "{name} row {i}: cosine {cosine:.6} < 0.9999"
        );
    }
}

/// One call per input: the single-sequence reference.
#[cfg(feature = "semantic")]
fn singly(provider: &mut dyn EmbeddingProvider, inputs: &[TokenizedInput]) -> Vec<Vec<f32>> {
    inputs
        .iter()
        .map(|input| {
            provider
                .embed_documents(
                    std::slice::from_ref(input),
                    &context_foundry::Control::unbounded(),
                )
                .expect("single-sequence call")
                .remove(0)
        })
        .collect()
}

#[cfg(feature = "semantic")]
#[test]
#[ignore = "development run: real model and sandbox bundle required"]
fn dev_real_worker_batched_and_single_sequence_vectors_agree() {
    let profile = dev_profile();
    let mut provider =
        supervisor::acquire_until(&profile, true, &context_foundry::Control::unbounded())
            .expect("acquire the real worker");

    // A batch of 8 with Unicode, CRLF and tiny inputs, the reordered batch
    // and a short input beside one exactly at the card limit, each against
    // the same inputs embedded one per call.
    let short = "Where does the parser read configuration records?";
    let unicode = "设置检索边界 — emoji 🧩 and combining márks";
    let crlf = "first line\r\nsecond line\r\n";
    let batch8: Vec<TokenizedInput> = [short, unicode, crlf, "a", "b c", "dd", "e f g", "h"]
        .iter()
        .map(|text| dev_ids(&profile, text))
        .collect();
    let reference = singly(provider.as_mut(), &batch8);
    let batched = provider
        .embed_documents(&batch8, &context_foundry::Control::unbounded())
        .expect("batch 8");
    assert_agree("batch-8", &batched, &reference);
    let reordered: Vec<TokenizedInput> = batch8.iter().rev().cloned().collect();
    let mut expected = reference.clone();
    expected.reverse();
    let batched = provider
        .embed_documents(&reordered, &context_foundry::Control::unbounded())
        .expect("reordered");
    assert_agree("reordered", &batched, &expected);
    let card = text_with_tokens(profile.card_tokens as usize, &profile);
    let hetero = vec![dev_ids(&profile, short), dev_ids(&profile, &card)];
    assert_eq!(hetero[1].ids.len(), profile.card_tokens as usize);
    let reference = singly(provider.as_mut(), &hetero);
    let batched = provider
        .embed_documents(&hetero, &context_foundry::Control::unbounded())
        .expect("heterogeneous");
    assert_agree("heterogeneous", &batched, &reference);

    // Serving boundary: exactly 2048 tokens as one query input; 2049 is
    // refused before any model call.
    let limit = context_foundry::neural::provider::SERVING_LIMIT_TOKENS;
    let full = dev_ids(&profile, &text_with_tokens(limit, &profile));
    assert_eq!(full.ids.len(), limit);
    provider
        .embed_query(&full, Instant::now() + Duration::from_secs(120))
        .expect("2048-token query");
    let mut over = full.clone();
    over.ids.push(over.ids[0]);
    let refused = provider.embed_query(&over, Instant::now() + Duration::from_secs(5));
    assert!(
        matches!(refused, Err(ProviderError::InputTooLarge(_))),
        "2049 must be refused before any model call, got {refused:?}"
    );
    // The supervised worker is released (reaped) before the test ends.
    drop(provider);
}

/// The profile's scratch root, created owner-private when absent (what the
/// supervisor itself does); the signed bundle's one write grant.
fn dev_scratch_root(profile: &SemanticProfile) -> PathBuf {
    use std::os::unix::fs::DirBuilderExt;
    let root = &profile.worker.scratch_root;
    if !root.exists() {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(root)
            .expect("create the profile's scratch root");
    }
    assert_eq!(
        mode_of(root) & 0o022,
        0,
        "the profile's scratch root must not be group- or world-writable"
    );
    std::fs::canonicalize(root).expect("canonical scratch root")
}

/// The warm-up of the agreement run, as real tokens from the profile's own
/// tokenizer: a batch of 8 (Unicode, CRLF, tiny inputs), a batch of 1, the
/// reordered batch, a short input beside one exactly at the card limit, and
/// the exactly-2048-token serving query.
#[cfg(feature = "semantic")]
fn real_token_workload(profile: &SemanticProfile) -> (Vec<Vec<TokenizedInput>>, TokenizedInput) {
    use context_foundry::neural::provider::SERVING_LIMIT_TOKENS;
    let short = "Where does the parser read configuration records?";
    let unicode = "设置检索边界 — emoji 🧩 and combining márks";
    let crlf = "first line\r\nsecond line\r\n";
    let texts = [short, unicode, crlf, "a", "b c", "dd", "e f g", "h"];
    let batch8: Vec<TokenizedInput> = texts.iter().map(|text| dev_ids(profile, text)).collect();
    let batch1 = batch8[..1].to_vec();
    let reordered: Vec<TokenizedInput> = batch8.iter().rev().cloned().collect();
    let card = text_with_tokens(profile.card_tokens as usize, profile);
    let hetero = vec![dev_ids(profile, short), dev_ids(profile, &card)];
    let full = dev_ids(profile, &text_with_tokens(SERVING_LIMIT_TOKENS, profile));
    assert_eq!(full.ids.len(), SERVING_LIMIT_TOKENS);
    (vec![batch8, batch1, reordered, hetero], full)
}

/// The executable of the development bundle built WITH `test-faults` so the
/// real worker records its phases (`--phase-file`): `foundry-embed` inside
/// `FoundryEmbedPhase.app`, signed with the same entitlements from the same
/// profile. The production bundle carries no phase code.
fn dev_phase_executable() -> PathBuf {
    std::env::var("CF_EMBED_DEV_PHASE_EXE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(
                "/private/tmp/cf-009-dev/FoundryEmbedPhase.app/Contents/MacOS/foundry-embed",
            )
        })
}

/// A sandboxed real worker owned by a shim, past `hello`/`ready` and warmed
/// (`warm`) with the whole real-token workload, with its phase file in the
/// bundle's write grant. Returns the shim, the warm-up batches and the
/// exactly-2048-token real-token query for the target call.
#[cfg(feature = "semantic")]
fn shim_worker_at_ready(
    profile: &SemanticProfile,
    warm: bool,
) -> (Shim, Vec<Vec<TokenizedInput>>, TokenizedInput) {
    let (batches, full_query) = real_token_workload(profile);
    let scratch = dev_scratch_root(profile);
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let phase_file = scratch.join(format!("phase-{}-{unique}", std::process::id()));
    let _ = std::fs::remove_file(&phase_file);
    let mut shim = start_shim(
        Some(dev_phase_executable().as_path()),
        &real_worker_args(profile),
        Some(worker_env(profile).as_slice()),
        &phase_file,
    );
    shim.hello();
    shim.expect_ready();
    if warm {
        let digest = profile.descriptor.digest();
        for (id, batch) in batches.iter().enumerate() {
            shim.embed_batch(id as u64 + 1, &digest, Purpose::Document, batch);
            let vectors = shim.expect_vectors(id as u64 + 1, batch.len());
            assert!(vectors.iter().all(|v| v.len() == DIMENSIONS));
        }
        assert_eq!(
            shim.phases().last().map(String::as_str),
            Some("evaluated"),
            "the warm-up calls finished; phases: {:?}",
            shim.phases()
        );
    }
    (shim, batches, full_query)
}

/// Send the target call and wait until the worker's own record says the
/// evaluation of THAT call is running and has been for 150 ms. False means
/// the call already finished: not an in-flight kill.
#[cfg(feature = "semantic")]
fn target_call_in_evaluation(
    shim: &mut Shim,
    profile: &SemanticProfile,
    input: &TokenizedInput,
) -> bool {
    shim.embed(4096, profile.descriptor.digest(), Purpose::Query, input);
    shim.wait_phase("eval", Duration::from_secs(60));
    std::thread::sleep(Duration::from_millis(150));
    shim.phases().last().map(String::as_str) == Some("eval")
}

/// A warmed sandboxed real worker mid-evaluation of the real-token
/// 2048-token query (the helper the owner-death gate uses).
#[cfg(feature = "semantic")]
fn real_worker_mid_evaluation(profile: &SemanticProfile, warm: bool) -> Shim {
    let (mut shim, _batches, full_query) = shim_worker_at_ready(profile, warm);
    assert!(
        target_call_in_evaluation(&mut shim, profile, &full_query),
        "the 2048-token call ended before the kill; it is not an in-flight kill: {:?}",
        shim.phases()
    );
    shim
}

/// Owner SIGKILL during a real model call, on the ORIGINAL warmed
/// real-token workload: a resident supervised worker has run the parity
/// batches, a second (shim-owned) sandboxed worker runs them too, and only
/// when that worker records that it is evaluating the 2048-token call is its
/// owner killed. The shim holds the liveness pipe open elsewhere, so only
/// kqueue can report the death. The worker must be gone within 2 s; the
/// printed timing separates the watcher's reaction (`exit_called_after`)
/// from the kernel's teardown (`exit_to_gone`), and a miss keeps waiting for
/// the true disappearance and carries `ps` state.
#[cfg(feature = "semantic")]
#[test]
#[ignore = "development run: real model, phase bundle, sandbox and venv required"]
fn dev_real_worker_owner_death_during_a_model_call() {
    let profile = dev_profile();
    let (batches, _) = real_token_workload(&profile);
    // The parity run's supervised worker stays loaded and warmed alongside.
    let mut resident =
        supervisor::acquire_until(&profile, true, &context_foundry::Control::unbounded())
            .expect("acquire the resident real worker");
    for batch in &batches {
        resident
            .embed_documents(batch, &context_foundry::Control::unbounded())
            .expect("resident warm-up");
    }
    let shim = real_worker_mid_evaluation(&profile, true);
    let pid = shim.worker_pid;
    let state_before = ps_line(pid);
    let death = shim.kill_owner(Duration::from_secs(120));
    println!("owner death of worker {pid}: {death:?}");
    println!("worker state before the kill: {state_before}");
    match death.gone_after {
        Some(after) if after <= Duration::from_secs(2) => {}
        _ => panic!(
            "worker {pid} outlived owner death by more than 2 s (state before the kill: \
             {state_before}): {death:?}"
        ),
    }
    drop(resident);
}

/// Diagnosis for the owner-death bound: list the mid-evaluation worker's
/// threads and sample their stacks, showing the watcher thread is separate
/// from the evaluating one and parked in `kevent`. The sampled call is one
/// full context (the profile's batch of cards at the card limit), long
/// enough that the evaluation still spans the 1 s sample; the phase file
/// is RE-CHECKED after the sample, and the attempt is repeated when the
/// call finished under it.
#[cfg(feature = "semantic")]
#[test]
#[ignore = "development run: real model, phase bundle and sandbox required"]
fn dev_real_worker_threads_during_a_model_call() {
    let profile = dev_profile();
    let (mut shim, _batches, _full) = shim_worker_at_ready(&profile, true);
    let pid = shim.worker_pid;
    let card = text_with_tokens(profile.card_tokens as usize, &profile);
    let one = dev_ids(&profile, &card);
    let batch: Vec<TokenizedInput> = (0..profile.batch).map(|_| one.clone()).collect();

    let out = std::path::Path::new("/private/tmp/cf-009-dev").join(format!("sample-{pid}.txt"));
    let mut sampled_inside_eval = false;
    for _ in 0..3 {
        shim.embed_batch(
            8192,
            &profile.descriptor.digest(),
            Purpose::Document,
            &batch,
        );
        shim.wait_phase("eval", Duration::from_secs(60));
        let threads = Command::new("/bin/ps")
            .args(["-M", "-o", "pid,tid,stat,pri,time,command", "-p"])
            .arg(pid.to_string())
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_else(|e| format!("ps -M failed: {e}"));
        println!("threads of worker {pid} mid-evaluation:\n{threads}");
        let sample = Command::new("/usr/bin/sample")
            .arg(pid.to_string())
            .args(["1", "10", "-file"])
            .arg(&out)
            .output();
        match sample {
            Ok(o) if o.status.success() => {
                let text = std::fs::read_to_string(&out).unwrap_or_default();
                let kevent = text.lines().filter(|l| l.contains("kevent")).count();
                println!(
                    "sample of worker {pid} written to {}: {kevent} kevent frames",
                    out.display()
                );
            }
            Ok(o) => println!(
                "sample unavailable ({}): {}{}",
                o.status,
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ),
            Err(e) => println!("sample could not run: {e}"),
        }
        // Reconfirm: the sampled call must still be evaluating.
        if shim.phases().last().map(String::as_str) == Some("eval") {
            sampled_inside_eval = true;
            break;
        }
        println!("the sampled call finished under the sample; retrying");
    }
    assert!(
        sampled_inside_eval,
        "no sample landed inside an evaluation; phases: {:?}",
        shim.phases()
    );
    let death = shim.kill_owner(Duration::from_secs(120));
    println!("owner death of worker {pid}: {death:?}");
}

/// Two busy children per core: CPU saturation for the whole teardown.
#[cfg(feature = "semantic")]
fn start_cpu_pressure() -> Vec<Child> {
    let cores = std::thread::available_parallelism().map_or(8, |n| n.get());
    (0..cores * 2)
        .map(|_| {
            Command::new("/usr/bin/yes")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn a busy child")
        })
        .collect()
}

/// A second real worker that keeps the GPU busy: it loops 2048-token queries
/// until told to stop. Returns the stop flag, the completed-call counter and
/// the thread (the provider lives and dies on it).
#[cfg(feature = "semantic")]
fn start_gpu_pressure(
    profile: &SemanticProfile,
) -> (
    std::sync::Arc<std::sync::atomic::AtomicBool>,
    std::sync::Arc<std::sync::atomic::AtomicU64>,
    std::thread::JoinHandle<()>,
) {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    let stop = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicU64::new(0));
    let (flag, counter, profile) = (Arc::clone(&stop), Arc::clone(&calls), profile.clone());
    let thread = std::thread::spawn(move || {
        let (_, full) = real_token_workload(&profile);
        let mut provider =
            supervisor::acquire_until(&profile, true, &context_foundry::Control::unbounded())
                .expect("acquire the pressure worker");
        while !flag.load(Ordering::SeqCst) {
            let outcome = provider.embed_query(&full, Instant::now() + Duration::from_secs(120));
            match outcome {
                Ok(_) => {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
                Err(ProviderError::Busy) => std::thread::sleep(Duration::from_millis(5)),
                Err(other) => panic!("the pressure worker failed: {other}"),
            }
        }
    });
    (stop, calls, thread)
}

/// One teardown experiment, the acceptance gate for the 2 s bound: a
/// sandboxed real worker whose target call is evaluating, killed through its
/// owner while the named pressure is active. Pressure starts BEFORE the
/// target call (GPU pressure before the worker even loads, CPU pressure
/// before the call is sent), and both the phase file (`eval`) and the
/// pressure are confirmed active immediately before the kill. Every
/// scenario asserts the worker is gone within 2 s, and prints the timing
/// split (kill -> `_exit`, `_exit` -> gone) and the `ps` state.
#[cfg(feature = "semantic")]
#[allow(clippy::too_many_arguments)]
fn teardown_scenario(
    profile: &SemanticProfile,
    name: &str,
    warm: bool,
    repeated_ids: bool,
    cpu: bool,
    gpu: bool,
    resident: bool,
) -> OwnerDeath {
    use std::sync::atomic::Ordering;
    // The original condition: the parity run's supervised worker stays
    // resident (loaded and warmed) beside the shim-owned one.
    let mut resident_provider = resident.then(|| {
        let (batches, _) = real_token_workload(profile);
        let mut provider =
            supervisor::acquire_until(profile, true, &context_foundry::Control::unbounded())
                .expect("acquire the resident real worker");
        for batch in &batches {
            provider
                .embed_documents(batch, &context_foundry::Control::unbounded())
                .expect("resident warm-up");
        }
        provider
    });
    let gpu_load = gpu.then(|| start_gpu_pressure(profile));
    let gpu_calls_at_start = gpu_load.as_ref().map(|(_, calls, _)| {
        // Steady GPU load before the experiment starts.
        let give_up = Instant::now() + Duration::from_secs(180);
        while calls.load(Ordering::SeqCst) < 2 {
            assert!(Instant::now() < give_up, "the pressure worker never ran");
            std::thread::sleep(Duration::from_millis(100));
        }
        calls.load(Ordering::SeqCst)
    });
    let (mut shim, _batches, full_query) = shim_worker_at_ready(profile, warm);
    // CPU pressure is running before the target call is sent.
    let mut busy_children = cpu.then(start_cpu_pressure);
    if cpu {
        std::thread::sleep(Duration::from_millis(200));
    }
    let target = if repeated_ids {
        // The original run's call: one 2048-token input of a repeated ID.
        TokenizedInput {
            ids: vec![100; context_foundry::neural::provider::SERVING_LIMIT_TOKENS],
        }
    } else {
        full_query
    };
    assert!(
        target_call_in_evaluation(&mut shim, profile, &target),
        "{name}: the target call ended before the kill; not an in-flight kill; phases: {:?}",
        shim.phases()
    );
    // The pressure is still active immediately before the kill.
    for child in busy_children.iter_mut().flatten() {
        assert!(
            child.try_wait().expect("cpu pressure child").is_none(),
            "{name}: a CPU pressure child exited early"
        );
    }
    if let (Some((_, calls, _)), Some(start)) = (&gpu_load, gpu_calls_at_start) {
        assert!(
            calls.load(Ordering::SeqCst) > start,
            "{name}: the GPU pressure worker went idle before the kill"
        );
    }
    assert_eq!(
        shim.phases().last().map(String::as_str),
        Some("eval"),
        "{name}: the worker is not inside the evaluation at the kill: {:?}",
        shim.phases()
    );
    let pid = shim.worker_pid;
    let state_before = ps_line(pid);
    let death = shim.kill_owner(Duration::from_secs(120));
    for mut child in busy_children.into_iter().flatten() {
        let _ = child.kill();
        let _ = child.wait();
    }
    if let Some((stop, calls, thread)) = gpu_load {
        stop.store(true, Ordering::SeqCst);
        thread.join().expect("the pressure thread");
        println!(
            "scenario {name}: pressure worker completed {} calls",
            calls.load(Ordering::SeqCst)
        );
    }
    drop(resident_provider.take());
    println!("STRESS-RESULT scenario={name} worker={pid} {death:?} state_before={state_before}");
    death
}

/// The 2 s bound under every condition the original miss may have run
/// under: a COLD first inference under CPU saturation and under GPU
/// contention, the ORIGINAL condition (a resident supervised worker beside a
/// shim-owned worker whose cold first call is the repeated-ID 2048-token
/// query), and the warmed cases. Every scenario must show the worker gone
/// within 2 s of the owner's death; a miss is reported with the timing split
/// and `ps` state, never relaxed.
#[cfg(feature = "semantic")]
#[test]
#[ignore = "development run: real model, phase bundle, sandbox and venv required"]
fn dev_real_worker_teardown_under_pressure() {
    let profile = dev_profile();
    let mut worst = Duration::ZERO;
    for (name, warm, repeated_ids, cpu, gpu, resident) in [
        (
            "cold-first-call-cpu-saturated",
            false,
            false,
            true,
            false,
            false,
        ),
        (
            "cold-first-call-gpu-contended",
            false,
            false,
            false,
            true,
            false,
        ),
        (
            "original-resident-and-repeated-id-cold-call",
            false,
            true,
            false,
            false,
            true,
        ),
        ("warm-cpu-saturated", true, false, true, false, false),
        ("warm-gpu-contended", true, false, false, true, false),
        ("warm-cpu-and-gpu", true, false, true, true, false),
    ] {
        let death = teardown_scenario(&profile, name, warm, repeated_ids, cpu, gpu, resident);
        let called = death
            .exit_called_after
            .unwrap_or_else(|| panic!("{name}: the worker never recorded its exit: {death:?}"));
        assert!(
            called < Duration::from_secs(1),
            "{name}: the watcher took {called:?} to end the process: {death:?}"
        );
        let gone = death
            .gone_after
            .unwrap_or_else(|| panic!("{name}: the worker never disappeared: {death:?}"));
        assert!(
            gone <= Duration::from_secs(2),
            "{name}: the worker outlived owner death by {gone:?} (over the 2 s bound)"
        );
        worst = worst.max(gone);
    }
    println!("STRESS-WORST gone_after={worst:?}");
}

/// Negative isolation tests with positive controls against the sandboxed
/// bundle, plus unsandboxed controls through the raw (unsigned) binary.
#[test]
#[ignore = "development run: sandbox bundle required (scripts/embed-worker-bundle.sh)"]
fn dev_sandbox_negative_probes() {
    let profile = dev_profile();
    let executable = profile.worker.bundle.join("Contents/MacOS/foundry-embed");
    let scratch = dev_scratch_root(&profile);
    let sentinel_dir = tempfile::tempdir_in("/private/tmp").expect("outside tempdir");
    let sentinel = sentinel_dir.path().join("outside-sentinel");
    std::fs::write(&sentinel, b"sentinel").expect("sentinel");
    let store_dir = tempfile::tempdir_in("/private/tmp").expect("store tempdir");
    std::fs::write(store_dir.path().join("store.db"), b"store").expect("store file");
    let scratch_probe = scratch.join("probe-write");
    let _ = std::fs::remove_file(&scratch_probe);

    // The supervisor's environment; probes must see nothing else.
    let env = worker_env(&profile);

    let run_probe = |name: &str, args: &[&str]| -> serde_json::Value {
        let (reader, read_fd, _owner) = liveness_pipe();
        let mut command = Command::new(&executable);
        command
            .arg("--probe")
            .arg(name)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .env_clear();
        for (key, value) in &env {
            command.env(key, value);
        }
        // The supervisor's own child setup, so a probe runs under exactly
        // the worker's process group, RLIMIT_NPROC=0 and descriptor table.
        // SAFETY: that setup calls only async-signal-safe functions.
        unsafe { command.pre_exec(supervisor::child_setup(read_fd)) };
        let output = command.output().expect("probe output");
        drop(reader);
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        serde_json::from_str(&stdout).unwrap_or_else(|_| panic!("probe {name} printed {stdout:?}"))
    };

    // A denial counts as enforcement only with the errno the sandbox (or
    // the resource limit) produces: `ENOENT` or `EINVAL` mean the probe
    // never reached the check.
    let denied_with = |name: &str, verdict: &serde_json::Value, errnos: &[i32]| {
        assert!(
            verdict["allowed"] == serde_json::Value::Bool(false),
            "{name} must be denied inside the sandbox, got {verdict}"
        );
        let errno = verdict["errno"].as_i64().unwrap_or(-1) as i32;
        assert!(
            errnos.contains(&errno),
            "{name} was denied with errno {errno}, expected one of {errnos:?}: {verdict}"
        );
    };
    let denied = |name: &str, verdict: &serde_json::Value| {
        denied_with(name, verdict, &[libc::EPERM, libc::EACCES]);
    };

    // Outside reads and writes, live-store access, writes into read-only
    // grants: denied, with granted reads as the positive control.
    denied(
        "outside read",
        &run_probe("read", &[&sentinel.display().to_string()]),
    );
    denied(
        "outside write",
        &run_probe(
            "write",
            &[&sentinel_dir.path().join("escape").display().to_string()],
        ),
    );
    denied(
        "store read",
        &run_probe(
            "read",
            &[&store_dir.path().join("store.db").display().to_string()],
        ),
    );
    denied(
        "store write",
        &run_probe(
            "write",
            &[&store_dir.path().join("store.db").display().to_string()],
        ),
    );
    denied(
        "grant write",
        &run_probe(
            "write",
            &[&profile.model_dir.join("escape").display().to_string()],
        ),
    );
    let granted = run_probe(
        "read",
        &[&profile.model_dir.join("config.json").display().to_string()],
    );
    assert_eq!(
        granted["allowed"],
        serde_json::Value::Bool(true),
        "granted read must work"
    );
    // The private scratch root is the one read-write grant.
    let scratch_write = run_probe("write", &[&scratch_probe.display().to_string()]);
    assert_eq!(
        scratch_write["allowed"],
        serde_json::Value::Bool(true),
        "scratch write must work: {scratch_write}"
    );

    // Symlinks, created and verified HERE in the unsandboxed parent before
    // any probe runs (a probe that cannot create its own link proves
    // nothing). Each link is followed by the probe's `open`; the sandbox
    // judges the resolved target, never the link's name:
    //  - escape-file: inside the read-write grant, pointing OUT at a file;
    //  - escape-dir: inside the grant, a directory component substituted by
    //    a link to an outside directory;
    //  - granted-via-outside: OUTSIDE every grant, naming a granted file
    //    (the link's location neither grants nor revokes anything).
    let outside_dir = sentinel_dir.path().join("outside-dir");
    std::fs::create_dir(&outside_dir).expect("outside dir");
    let inner_sentinel = outside_dir.join("inner-sentinel");
    std::fs::write(&inner_sentinel, b"inner").expect("inner sentinel");
    let granted_file = profile.model_dir.join("config.json");
    let escape_file = scratch.join("escape-file");
    let escape_dir = scratch.join("escape-dir");
    let granted_via_outside = sentinel_dir.path().join("granted-via-outside");
    for (link, target) in [
        (&escape_file, &sentinel),
        (&escape_dir, &outside_dir),
        (&granted_via_outside, &granted_file),
    ] {
        let _ = std::fs::remove_file(link);
        std::os::unix::fs::symlink(target, link).expect("create the link in the parent");
        assert_eq!(
            std::fs::read_link(link).expect("read the link back"),
            *target,
            "the link must exist and point where this test says"
        );
        std::fs::metadata(link).expect("the link resolves in the parent");
    }
    let through_dir = escape_dir.join("inner-sentinel");
    let created_through_dir = escape_dir.join("created-by-probe");
    let _ = std::fs::remove_file(outside_dir.join("created-by-probe"));
    denied(
        "read through escape-file",
        &run_probe("read", &[&escape_file.display().to_string()]),
    );
    denied(
        "write through escape-file",
        &run_probe("write", &[&escape_file.display().to_string()]),
    );
    assert_eq!(
        std::fs::read(&sentinel).expect("sentinel"),
        b"sentinel",
        "the sandboxed write must not have reached the link's target"
    );
    denied(
        "read through escape-dir",
        &run_probe("read", &[&through_dir.display().to_string()]),
    );
    denied(
        "create through escape-dir",
        &run_probe("write", &[&created_through_dir.display().to_string()]),
    );
    assert!(
        !outside_dir.join("created-by-probe").exists(),
        "the sandboxed create must not have reached the outside directory"
    );
    let via_outside = run_probe("read", &[&granted_via_outside.display().to_string()]);
    assert_eq!(
        via_outside["allowed"], granted["allowed"],
        "a link outside the grants must read exactly like its granted target: {via_outside}"
    );

    // Inherited environment: every name the supervisor set, and no extra
    // name beyond those the OS itself injects into a sandboxed process; in
    // particular nothing that looks like a credential, proxy or agent socket.
    let env_probe = run_probe("env", &[]);
    let names: Vec<String> =
        serde_json::from_str(env_probe["detail"].as_str().expect("env detail"))
            .expect("env names JSON");
    for (key, _) in &env {
        assert!(
            names.iter().any(|n| n == key),
            "{key} must reach the worker"
        );
    }
    let os_injected = [
        "APP_SANDBOX_",
        "XPC_",
        "CFFIXED_USER_HOME",
        "__CF_",
        "__CFBundle",
    ];
    let extras: Vec<&String> = names
        .iter()
        .filter(|n| !env.iter().any(|(k, _)| *k == n.as_str()))
        .collect();
    for extra in &extras {
        assert!(
            os_injected.iter().any(|prefix| extra.starts_with(prefix)),
            "unexpected environment name {extra} reached the worker (all extras: {extras:?})"
        );
    }

    // Inherited descriptors: only the liveness fd beyond stdio.
    let fds_probe = run_probe("fds", &[]);
    let open: Vec<i64> =
        serde_json::from_str(fds_probe["detail"].as_str().expect("fds detail")).unwrap_or_default();
    assert_eq!(
        open.len(),
        1,
        "only the liveness fd may survive, got {open:?}"
    );

    // Network: outbound and listening, TCP and UDP over IPv4 and IPv6, plus
    // DNS. The targets are loopback sockets this test owns, so the
    // unsandboxed controls need no internet: a connect or send to them
    // succeeds there, and the sandbox must still deny it.
    let tcp4 = std::net::TcpListener::bind("127.0.0.1:0").expect("IPv4 TCP target");
    let udp4 = std::net::UdpSocket::bind("127.0.0.1:0").expect("IPv4 UDP target");
    let tcp6 = std::net::TcpListener::bind("[::1]:0").ok();
    let udp6 = std::net::UdpSocket::bind("[::1]:0").ok();
    let mut network: Vec<(&'static str, String)> = vec![
        ("tcp-connect", tcp4.local_addr().expect("addr").to_string()),
        ("udp-send", udp4.local_addr().expect("addr").to_string()),
        ("tcp-listen", "127.0.0.1:0".to_string()),
        ("udp-bind", "127.0.0.1:0".to_string()),
    ];
    if let (Some(tcp6), Some(udp6)) = (&tcp6, &udp6) {
        network.push(("tcp-connect", tcp6.local_addr().expect("addr").to_string()));
        network.push(("udp-send", udp6.local_addr().expect("addr").to_string()));
        network.push(("tcp-listen", "[::1]:0".to_string()));
        network.push(("udp-bind", "[::1]:0".to_string()));
    }
    for (probe, target) in &network {
        denied(
            &format!("{probe} {target}"),
            &run_probe(probe, &[target.as_str()]),
        );
    }
    // The resolver's failure carries no stable errno; the positive control
    // below shows the same lookup succeeds outside the sandbox.
    let dns = run_probe("dns", &["example.com"]);
    assert_eq!(
        dns["allowed"],
        serde_json::Value::Bool(false),
        "dns must fail inside the sandbox: {dns}"
    );

    // Process limits: fork and spawn denied (RLIMIT_NPROC=0 answers
    // EAGAIN, the sandbox EPERM), limit restoration denied.
    let no_process = [libc::EAGAIN, libc::EPERM, libc::EACCES];
    denied_with("fork", &run_probe("fork", &[]), &no_process);
    denied_with("spawn", &run_probe("spawn", &[]), &no_process);
    denied("nproc restore", &run_probe("nproc-restore", &[]));

    // Oversized/malformed IPC against the real worker: manual spawn (the
    // test is the owner), one bad frame, expect refusal and exit.
    let descriptor = serde_json::to_string(&profile.descriptor).expect("descriptor JSON");
    let (reader, read_fd, liveness_owner) = liveness_pipe();
    let mut command = Command::new(&executable);
    command
        .arg("--owner-pid")
        .arg(std::process::id().to_string())
        .arg("--liveness-fd")
        .arg(read_fd.to_string())
        .arg("--descriptor")
        .arg(&descriptor)
        .arg("--model-dir")
        .arg(&profile.model_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env_clear();
    for (key, value) in &env {
        command.env(key, value);
    }
    // SAFETY: the supervisor's child setup calls only async-signal-safe
    // functions between fork and exec.
    unsafe { command.pre_exec(supervisor::child_setup(read_fd)) };
    let mut child = command.spawn().expect("spawn real worker");
    drop(reader);
    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));
    protocol::write_frame(
        &mut stdin,
        &Header::Hello {
            protocol: protocol::PROTOCOL_VERSION,
        },
        &[],
    )
    .expect("hello");
    let (header, _) = protocol::read_frame(&mut stdout).expect("ready");
    assert!(matches!(header, Header::Ready { .. }));
    stdin
        .write_all(&0xffff_ffffu32.to_le_bytes())
        .expect("oversize frame");
    match protocol::read_frame(&mut stdout).expect("error frame") {
        (Header::Error { code, .. }, _) => assert_eq!(code, "frame_invalid"),
        other => panic!("expected frame_invalid, got {other:?}"),
    }
    assert_eq!(
        child.wait().expect("wait").code(),
        Some(worker_runtime::FRAME_EXIT)
    );
    drop(liveness_owner);

    // Unsandboxed positive controls through the raw, unsigned binary: the
    // same probes must be permitted (or fail for non-sandbox reasons), so a
    // sandbox "denied" is evidence of enforcement, not of a broken probe.
    let Some(raw) = option_env!("CARGO_BIN_EXE_foundry-embed") else {
        panic!("build with --features embed-worker for the control binary");
    };
    let control = |name: &str, args: &[&str]| -> serde_json::Value {
        let output = Command::new(raw)
            .arg("--probe")
            .arg(name)
            .args(args)
            .output()
            .expect("control probe");
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        serde_json::from_str(&stdout)
            .unwrap_or_else(|_| panic!("control {name} printed {stdout:?}"))
    };
    assert_eq!(
        control("read", &[&sentinel.display().to_string()])["allowed"],
        serde_json::Value::Bool(true),
        "unsandboxed read must succeed"
    );
    // The same links, unsandboxed: each resolves and is reachable, so the
    // sandbox's denials above are enforcement, not broken links.
    for (what, path) in [
        ("escape-file", &escape_file),
        ("escape-dir", &through_dir),
        ("granted-via-outside", &granted_via_outside),
    ] {
        let verdict = control("read", &[&path.display().to_string()]);
        assert_eq!(
            verdict["allowed"],
            serde_json::Value::Bool(true),
            "unsandboxed read through {what} must succeed: {verdict}"
        );
    }
    let verdict = control("write", &[&escape_file.display().to_string()]);
    assert_eq!(
        verdict["allowed"],
        serde_json::Value::Bool(true),
        "unsandboxed write through escape-file must succeed: {verdict}"
    );
    assert!(
        std::fs::read(&sentinel).expect("sentinel").ends_with(b"x"),
        "the unsandboxed write must have reached the link's target"
    );
    let verdict = control("write", &[&created_through_dir.display().to_string()]);
    assert_eq!(
        verdict["allowed"],
        serde_json::Value::Bool(true),
        "unsandboxed create through escape-dir must succeed: {verdict}"
    );
    assert!(
        outside_dir.join("created-by-probe").exists(),
        "the unsandboxed create must have reached the outside directory"
    );
    assert_eq!(
        control("fork", &[])["allowed"],
        serde_json::Value::Bool(true),
        "unsandboxed fork must succeed"
    );
    assert_eq!(
        control("nproc-restore", &[])["allowed"],
        serde_json::Value::Bool(true),
        "unsandboxed limit raise must succeed"
    );
    for (probe, target) in &network {
        let verdict = control(probe, &[target.as_str()]);
        assert_eq!(
            verdict["allowed"],
            serde_json::Value::Bool(true),
            "unsandboxed {probe} {target} must be permitted: {verdict}"
        );
    }
    let dns = control("dns", &["example.com"]);
    assert_eq!(
        dns["allowed"],
        serde_json::Value::Bool(true),
        "unsandboxed DNS must resolve (the host needs outbound access for this control): {dns}"
    );
}
