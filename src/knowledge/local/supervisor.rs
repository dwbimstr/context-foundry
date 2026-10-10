use super::{Config, Device, Observation, Profile, Telemetry, protocol, resources};
use crate::knowledge::{fail, provider::Reply};
use crate::{Control, FResult};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::os::unix::{fs::OpenOptionsExt, process::CommandExt};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_millis(20);
fn io_failure(_: std::io::Error) -> crate::FoundryError {
    fail("knowledge_worker_failed", "owned worker I/O failed")
}
/// A private, unlinked, read-only snapshot. It cannot follow later path changes;
/// the only write descriptor is closed before the object reaches the worker.
/// File-backed snapshots avoid keeping a second weight-sized Rust allocation.
fn snapshot(
    path: &Path,
    expected: &str,
    max: u64,
    executable: bool,
    control: &Control,
) -> FResult<(File, u64)> {
    let mut source = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    let meta = source.metadata()?;
    if !meta.is_file() || meta.len() == 0 || meta.len() > max {
        return Err(fail(
            "knowledge_artifact_invalid",
            "artifact must be a bounded nonempty regular file",
        ));
    }
    let name = std::env::temp_dir().join(format!("foundry-generation-{}", uuid::Uuid::new_v4()));
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&name)?;
    let cleanup = Cleanup(name.clone());
    let mut digest = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        control.check()?;
        let n = source.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if bytes > max {
            return Err(fail(
                "knowledge_artifact_invalid",
                "artifact exceeds its snapshot bound",
            ));
        }
        digest.update(&buffer[..n]);
        output.write_all(&buffer[..n])?;
    }
    if bytes != meta.len() || format!("{:x}", digest.finalize()) != expected {
        return Err(fail(
            "knowledge_artifact_invalid",
            "artifact hash/size mismatch",
        ));
    }
    use std::os::unix::fs::PermissionsExt;
    output.set_permissions(std::fs::Permissions::from_mode(if executable {
        0o500
    } else {
        0o400
    }))?;
    drop(output);
    let input = File::open(&name)?;
    drop(cleanup);
    Ok((input, bytes))
}
pub struct Provider {
    config: Config,
    executable: File,
    model: File,
    worker: Option<Worker>,
    next_id: u64,
    last_use: Instant,
    telemetry: Telemetry,
    model_name: String,
    model_hash: String,
}
struct Worker {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
    _liveness: std::io::PipeWriter,
}
impl Drop for Worker {
    fn drop(&mut self) {
        // No descendant execution is admitted. Reap before releasing snapshots
        // or admitting a replacement, including cancellation during model load.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Provider {
    pub fn new(
        profile: &Profile,
        control: &Control,
        observed: &mut Observation,
        telemetry: Telemetry,
    ) -> FResult<Self> {
        let config = profile.local.as_ref().expect("local profile").clone();
        config.platform()?;
        let bound = control.bounded_by(Instant::now() + Duration::from_secs(config.load_seconds));
        let (executable, exe_bytes) = snapshot(
            &config.executable,
            &config.executable_sha256,
            512 * 1024 * 1024,
            true,
            &bound,
        )?;
        let (model, model_bytes) = snapshot(
            &config.gguf,
            &profile.artifact_sha256,
            config.memory_bytes,
            false,
            &bound,
        )?;
        observed.verified_bytes = exe_bytes + model_bytes;
        telemetry.publish(observed);
        Ok(Self {
            config,
            executable,
            model,
            worker: None,
            next_id: 1,
            last_use: Instant::now(),
            telemetry,
            model_name: profile.model.clone(),
            model_hash: profile.artifact_sha256.clone(),
        })
    }
    pub fn matches(&self, profile: &Profile) -> bool {
        profile.local.as_ref() == Some(&self.config)
            && profile.model == self.model_name
            && profile.artifact_sha256 == self.model_hash
    }
    fn spawn(&mut self, profile: &Profile, observed: &mut Observation) -> FResult<Vec<u8>> {
        // Inherited descriptors share their offset across worker launches.
        self.model.seek(SeekFrom::Start(0))?;
        let (reader, writer) = std::io::pipe()?;
        let model_fd = self.model.as_raw_fd();
        let executable_fd = self.executable.as_raw_fd();
        let life_fd = reader.as_raw_fd();
        #[cfg(target_os = "linux")]
        let path = PathBuf::from(format!("/proc/self/fd/{executable_fd}"));
        #[cfg(not(target_os = "linux"))]
        let path = {
            // A macOS bundle keeps its signed path. Reverify before each launch;
            // copied executable files do not establish App Sandbox packaging.
            let (hash, _) = crate::neural::profile::hash_regular_file(&self.config.executable)
                .map_err(|_| fail("knowledge_artifact_invalid", "worker verification failed"))?;
            if hash != self.config.executable_sha256 {
                return Err(fail("knowledge_artifact_invalid", "worker changed"));
            }
            self.config.executable.clone()
        };
        let mut command = Command::new(path);
        command
            .env_clear()
            .env("LANG", "C")
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", "/nonexistent")
            .env("TMPDIR", "/nonexistent")
            .current_dir("/")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let owner = std::process::id();
        unsafe {
            command.pre_exec(move || child_setup(owner, &[model_fd, executable_fd, life_fd]));
        }
        let mut child = command.spawn().map_err(io_failure)?;
        let input = child.stdin.take().expect("piped input");
        let output = child.stdout.take().expect("piped output");
        // From here on every error drops the child guard and reaps it.
        self.worker = Some(Worker {
            child,
            input,
            output,
            _liveness: writer,
        });
        let worker = self.worker.as_ref().expect("spawned");
        observed.pid = Some(worker.child.id());
        observed.launches += 1;
        observed.last_stop = None;
        observed.isolation = None;
        self.telemetry.publish(observed);
        for fd in [worker.input.as_raw_fd(), worker.output.as_raw_fd()] {
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
            {
                return Err(io_failure(std::io::Error::last_os_error()));
            }
        }
        protocol::encode(&protocol::Hello {
            v: 1,
            config: self.config.clone(),
            model: profile.model.clone(),
            artifact_sha256: profile.artifact_sha256.clone(),
            owner_pid: owner,
            model_fd,
            liveness_fd: life_fd,
        })
        .map_err(io_failure)
    }
    async fn exchange(
        &mut self,
        frame: Vec<u8>,
        control: &Control,
        observed: &mut Observation,
    ) -> FResult<protocol::Answer> {
        let worker = self.worker.as_mut().expect("spawned worker");
        let mut written = 0;
        let mut received = Vec::new();
        loop {
            control.check()?;
            let mut progress = false;
            let mut complete = false;
            if written < frame.len() {
                match worker.input.write(&frame[written..]) {
                    Ok(0) => return Err(fail("knowledge_worker_exited", "worker input closed")),
                    Ok(n) => {
                        written += n;
                        progress = true;
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(e) => return Err(io_failure(e)),
                }
            }
            let mut buffer = [0u8; 8192];
            match worker.output.read(&mut buffer) {
                Ok(0) => {
                    if worker
                        .child
                        .try_wait()
                        .ok()
                        .flatten()
                        .is_some_and(|status| status.code() == Some(74))
                    {
                        return Err(fail(
                            "knowledge_resource_limit",
                            "worker resource monitor stopped execution",
                        ));
                    }
                    return Err(fail(
                        "knowledge_worker_exited",
                        "worker exited before a complete reply",
                    ));
                }
                Ok(n) => {
                    received.extend_from_slice(&buffer[..n]);
                    progress = true;
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(e) => return Err(io_failure(e)),
            }
            if received.len() >= 4 {
                let size =
                    u32::from_le_bytes(received[..4].try_into().expect("length prefix")) as usize;
                if size == 0 || size > protocol::MAX_FRAME || received.len() > size + 4 {
                    return Err(fail(
                        "knowledge_provider_malformed",
                        "invalid worker frame size",
                    ));
                }
                if received.len() == size + 4 {
                    if written != frame.len() {
                        return Err(fail(
                            "knowledge_provider_malformed",
                            "premature worker reply",
                        ));
                    }
                    complete = true;
                }
            }
            let memory = resources::memory(worker.child.id()).map_err(|_| {
                fail(
                    "knowledge_resource_unavailable",
                    "worker memory observation failed",
                )
            })?;
            observed.peak_memory_bytes = observed.peak_memory_bytes.max(memory);
            self.telemetry.publish(observed);
            if memory > self.config.memory_bytes {
                return Err(fail(
                    "knowledge_resource_limit",
                    "worker exceeded its memory ceiling",
                ));
            }
            if complete {
                control.check()?;
                return serde_json::from_slice(&received[4..])
                    .map_err(|_| fail("knowledge_provider_malformed", "invalid worker reply"));
            }
            if !progress {
                tokio::time::sleep(POLL).await;
            }
        }
    }
    pub async fn generate(
        &mut self,
        profile: &Profile,
        body: Vec<u8>,
        control: &Control,
        observed: &mut Observation,
    ) -> FResult<Reply> {
        let result = self.generate_inner(profile, body, control, observed).await;
        self.last_use = Instant::now();
        if let Err(error) = &result {
            self.unload(error.code(), observed);
        }
        result
    }
    async fn generate_inner(
        &mut self,
        profile: &Profile,
        body: Vec<u8>,
        control: &Control,
        observed: &mut Observation,
    ) -> FResult<Reply> {
        if self.worker.is_none() {
            let bound =
                control.bounded_by(Instant::now() + Duration::from_secs(self.config.load_seconds));
            bound.check()?;
            let frame = self.spawn(profile, observed)?;
            match self.exchange(frame, &bound, observed).await? {
                protocol::Answer::Ready {
                    identity,
                    isolation,
                } if identity == crate::knowledge::hash(&self.config)?
                    && isolation
                        == if cfg!(target_os = "linux") {
                            "linux-seccomp-fd-v1"
                        } else {
                            "macos-app-sandbox-development-v1"
                        } =>
                {
                    observed.isolation = Some(isolation)
                }
                protocol::Answer::Failed { id: 0, code } => return Err(worker_failure(&code)),
                _ => {
                    return Err(fail(
                        "knowledge_provider_malformed",
                        "worker readiness identity mismatch",
                    ));
                }
            }
        }
        let bound = control.bounded_by(Instant::now() + Duration::from_secs(profile.call_seconds));
        let id = self.next_id;
        self.next_id = id
            .checked_add(1)
            .ok_or_else(|| fail("knowledge_worker_failed", "request identity exhausted"))?;
        let frame = protocol::encode(&protocol::Request {
            id,
            body: serde_json::from_slice(&body)?,
        })
        .map_err(io_failure)?;
        observed.calls += 1;
        self.telemetry.publish(observed);
        match self.exchange(frame, &bound, observed).await? {
            protocol::Answer::Completed {
                id: got,
                content,
                usage,
                gpu_bytes,
            } if got == id => {
                let gpu_valid = match self.config.device {
                    Device::Cpu => gpu_bytes == 0,
                    Device::Metal {
                        allocation_bytes, ..
                    } => gpu_bytes <= allocation_bytes,
                };
                if !gpu_valid
                    || content.len() > crate::knowledge::MAX_RESPONSE_BYTES
                    || usage
                        .prompt_tokens
                        .is_none_or(|n| n > self.config.context_tokens.into())
                    || usage
                        .completion_tokens
                        .is_none_or(|n| n > profile.output_tokens.into())
                {
                    return Err(fail(
                        "knowledge_provider_malformed",
                        "worker output/usage exceeds its admitted bounds",
                    ));
                }
                observed.peak_gpu_bytes = observed.peak_gpu_bytes.max(gpu_bytes);
                self.telemetry.publish(observed);
                Ok(Reply { content, usage })
            }
            protocol::Answer::Failed { id: got, code } if got == id => Err(worker_failure(&code)),
            _ => Err(fail(
                "knowledge_provider_malformed",
                "worker response identity mismatch",
            )),
        }
    }
    pub fn unload(&mut self, reason: &str, observed: &mut Observation) {
        if self.worker.take().is_some() {
            observed.last_stop = Some(reason.into());
        }
        observed.pid = None;
        self.telemetry.publish(observed);
    }
    pub fn idle(&mut self, observed: &mut Observation) {
        if let Some(worker) = &mut self.worker {
            let reason = match worker.child.try_wait() {
                Ok(Some(_)) => Some("knowledge_worker_exited"),
                Err(_) => Some("knowledge_worker_failed"),
                Ok(None) => match resources::memory(worker.child.id()) {
                    Ok(bytes) => {
                        observed.peak_memory_bytes = observed.peak_memory_bytes.max(bytes);
                        if bytes > self.config.memory_bytes {
                            Some("knowledge_resource_limit")
                        } else if self.last_use.elapsed()
                            >= Duration::from_secs(self.config.idle_seconds)
                        {
                            Some("idle")
                        } else {
                            None
                        }
                    }
                    Err(_) => Some("knowledge_resource_unavailable"),
                },
            };
            self.telemetry.publish(observed);
            if let Some(reason) = reason {
                self.unload(reason, observed);
            }
        }
    }
}
fn worker_failure(code: &str) -> crate::FoundryError {
    let code = match code {
        "knowledge_isolation_unavailable" => "knowledge_isolation_unavailable",
        "knowledge_resource_limit" => "knowledge_resource_limit",
        "knowledge_resource_unavailable" => "knowledge_resource_unavailable",
        "knowledge_input_limit" => "knowledge_input_limit",
        "knowledge_output_limit" => "knowledge_output_limit",
        "knowledge_artifact_invalid" => "knowledge_artifact_invalid",
        _ => "knowledge_worker_failed",
    };
    fail(
        code,
        "owned generation failed; private worker details are not echoed",
    )
}

fn child_setup(owner: u32, keep: &[i32]) -> std::io::Result<()> {
    // This is pre_exec: no allocation, locks, filesystem parsing or Rust I/O.
    if unsafe { libc::setpgid(0, 0) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    #[cfg(target_os = "linux")]
    {
        if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } != 0
            || unsafe { libc::getppid() } as u32 != owner
            || unsafe { libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 4u32) } != 0
        {
            return Err(std::io::Error::last_os_error());
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = owner;
        for fd in 3..unsafe { libc::getdtablesize() } {
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
            if flags >= 0 {
                unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
            }
        }
    }
    for &fd in keep {
        if unsafe { libc::fcntl(fd, libc::F_SETFD, 0) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod pressure_tests {
    use super::*;
    use crate::model_resources::{self, Lane, Resources};
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };

    #[test]
    fn pressure_reaps_idle_child_before_releasing_reservation_and_keeps_snapshots() {
        // A sleeping process tests the owner's residency lifecycle, not model
        // execution/isolation (those are tested with foundry-generate-fake).
        let mut child = Command::new("/bin/sleep")
            .arg("60")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let pid = child.id();
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let (_, liveness) = std::io::pipe().unwrap();
        let memory = Arc::new(AtomicU64::new(2000));
        let resources = Resources::fixture(
            model_resources::Config {
                resident_bytes: 1000,
                reserve_bytes: 100,
                resume_margin_bytes: 50,
                background_cpu: None,
            },
            memory.clone(),
        );
        let reservation = resources.reserve(Lane::Generation, 1000).unwrap();
        let config: Config = serde_json::from_value(serde_json::json!({
            "development":true,"executable":"/unused","executable_sha256":"a".repeat(64),
            "gguf":"/unused-model","llama_cpp":super::super::LLAMA_CPP,"chat_template":"chatml",
            "device":{"kind":"cpu"},"context_tokens":4096,"batch_tokens":128,"threads":1,
            "memory_bytes":536870912,"memory_enforcement":"hard_address_space",
            "cpu_seconds":30,"load_seconds":10,"idle_seconds":300
        }))
        .unwrap();
        let telemetry = Telemetry::default();
        let mut session = super::super::Session {
            provider: Some(Provider {
                config,
                executable: tempfile::tempfile().unwrap(),
                model: tempfile::tempfile().unwrap(),
                worker: Some(Worker {
                    child,
                    input,
                    output,
                    _liveness: liveness,
                }),
                next_id: 1,
                last_use: Instant::now(),
                telemetry: telemetry.clone(),
                model_name: "fixture".into(),
                model_hash: "a".repeat(64),
            }),
            refused: None,
            observation: Observation {
                pid: Some(pid),
                ..Default::default()
            },
            telemetry,
            resources: Some(resources.clone()),
            reservation: Some(reservation),
        };
        memory.store(99, Ordering::SeqCst);
        resources.refresh();
        session.idle();
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "must reap before returning capacity"
        );
        assert_eq!(resources.status()["reserved_generation_bytes"], 0);
        assert_eq!(
            session.observation.last_stop.as_deref(),
            Some("model_memory_pressure")
        );
        let provider = session.provider.as_ref().unwrap();
        assert!(provider.worker.is_none());
        assert!(provider.executable.metadata().is_ok() && provider.model.metadata().is_ok());
        memory.store(150, Ordering::SeqCst);
        resources.refresh();
        assert!(resources.background().is_ok());
    }
}
