//! Worker entry shared by the separately linked backend and the fixture binary.
use super::{Observation, protocol, resources};
use crate::knowledge::provider::{Reply, Usage};
use std::io;

/// Incremental framing for one grammar-constrained JSON object. Recognize an
/// object completed on the last allowed token without waiting for model EOS or
/// sampling unbounded trailing whitespace. Semantic validation stays in the owner.
#[derive(Default)]
pub struct JsonCompletion {
    depth: usize,
    quoted: bool,
    escaped: bool,
    started: bool,
    complete: bool,
    invalid: bool,
}
impl JsonCompletion {
    pub fn push(&mut self, bytes: &[u8]) -> bool {
        for &byte in bytes {
            if self.invalid {
                break;
            }
            if self.complete {
                self.invalid |= !byte.is_ascii_whitespace();
            } else if self.quoted {
                if self.escaped {
                    self.escaped = false;
                } else if byte == b'\\' {
                    self.escaped = true;
                } else if byte == b'"' {
                    self.quoted = false;
                }
            } else if !self.started {
                if byte == b'{' {
                    self.started = true;
                    self.depth = 1;
                } else {
                    self.invalid |= !byte.is_ascii_whitespace();
                }
            } else {
                match byte {
                    b'"' => self.quoted = true,
                    b'{' | b'[' => self.depth += 1,
                    b'}' | b']' => {
                        self.depth -= 1;
                        self.complete = self.depth == 0;
                    }
                    _ => {}
                }
            }
        }
        self.complete && !self.invalid
    }
}

pub trait Generator {
    fn generate(&mut self, body: &serde_json::Value) -> Result<(Reply, u64), &'static str>;
    /// Actual counters for a completed computation that failed to produce an
    /// admissible result. Abrupt exits and interrupted computation stay unknown.
    fn failed_usage(&self) -> Option<Usage> {
        None
    }
}
pub fn serve(
    load: impl FnOnce(&protocol::Hello) -> Result<Box<dyn Generator>, &'static str>,
) -> i32 {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let Ok(hello) = protocol::read::<protocol::Hello>(&mut input) else {
        return 65;
    };
    let failed = |output: &mut io::StdoutLock<'_>, id, code: &str| {
        let _ = protocol::write(
            output,
            &protocol::Answer::Failed {
                id,
                code: code.into(),
                usage: None,
            },
        );
    };
    if hello.v != 1
        || hello.model_fd < 3
        || hello.liveness_fd < 3
        || hello.model_fd == hello.liveness_fd
        || hello.owner_pid == 0
        || hello.model.len() > 256
        || !crate::knowledge::digest(&hello.artifact_sha256)
        || hello.config.validate().is_err()
    {
        failed(&mut output, 0, "knowledge_profile_invalid");
        return 78;
    }
    let isolation = match resources::install(&hello.config, hello.model_fd) {
        Ok(isolation) => isolation,
        Err(_) => {
            failed(&mut output, 0, "knowledge_isolation_unavailable");
            return 79;
        }
    };
    if arm_owner(hello.owner_pid, hello.liveness_fd).is_err() {
        failed(&mut output, 0, "knowledge_isolation_unavailable");
        return 79;
    }
    let mut generator = match load(&hello) {
        Ok(generator) => generator,
        Err(code) => {
            failed(&mut output, 0, code);
            return 1;
        }
    };
    let Ok(identity) = crate::knowledge::hash(&hello.config) else {
        return 78;
    };
    if protocol::write(
        &mut output,
        &protocol::Answer::Ready {
            identity,
            isolation: isolation.into(),
        },
    )
    .is_err()
    {
        return 1;
    }
    let mut last = 0u64;
    loop {
        let request: protocol::Request = match protocol::read(&mut input) {
            Ok(request) => request,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return 0,
            Err(_) => return 65,
        };
        if request.id <= last
            || request.body["model"] != hello.model
            || serde_json::to_vec(&request.body).is_ok_and(|body| body.len() > 131072)
        {
            failed(&mut output, request.id, "knowledge_provider_malformed");
            return 65;
        }
        last = request.id;
        let answer = match generator.generate(&request.body) {
            Ok((reply, peak_gpu_bytes)) => protocol::completed(
                request.id,
                reply,
                &Observation {
                    peak_gpu_bytes,
                    ..Observation::default()
                },
            ),
            Err(code) => protocol::Answer::Failed {
                id: request.id,
                code: code.into(),
                usage: generator.failed_usage(),
            },
        };
        if protocol::write(&mut output, &answer).is_err() {
            return 1;
        }
        if matches!(answer, protocol::Answer::Failed { .. }) {
            return 1;
        }
    }
}
fn arm_owner(owner: u32, fd: i32) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        // A pipe, not PID polling: PID reuse cannot keep an orphan alive.
        let mut stat = unsafe { std::mem::zeroed::<libc::stat>() };
        if unsafe { libc::fstat(fd, &mut stat) } != 0
            || stat.st_mode & libc::S_IFMT != libc::S_IFIFO
            || unsafe { libc::getppid() } as u32 != owner
        {
            return Err(io::Error::other("invalid owner liveness"));
        }
        std::thread::Builder::new()
            .name("generation-owner".into())
            .spawn(move || {
                let mut byte = [0u8];
                loop {
                    let result = unsafe { libc::read(fd, byte.as_mut_ptr().cast(), 1) };
                    if result == -1
                        && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted
                    {
                        continue;
                    }
                    // No data is ever expected; an invalid liveness channel also ends work.
                    unsafe { libc::_exit(75) };
                }
            })?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        use crate::neural::worker_runtime::{Armed, arm_owner_death_watcher};
        match arm_owner_death_watcher(owner, fd).map_err(io::Error::other)? {
            Armed::Watching => Ok(()),
            Armed::OwnerGone => Err(io::Error::other("owner exited")),
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (owner, fd);
        Err(io::Error::other("owner containment unavailable"))
    }
}

#[cfg(test)]
mod completion_tests {
    use super::JsonCompletion;

    #[test]
    fn framing_survives_token_boundaries_inside_strings_escapes_and_utf8() {
        let bytes = br#"{"text":"a } [ \" \\ \u2603","nested":[{"v":"ok"}]}"#;
        for split in 0..bytes.len() {
            let mut frame = JsonCompletion::default();
            assert!(!frame.push(&bytes[..split]));
            assert!(frame.push(&bytes[split..]));
            assert!(frame.push(b"\n "));
            assert!(
                !frame.push(b"{}"),
                "multiple objects are not one completion"
            );
        }
        let mut frame = JsonCompletion::default();
        for &byte in "{\"text\":\"☃\"}".as_bytes().split_last().unwrap().1 {
            assert!(!frame.push(&[byte]));
        }
        assert!(frame.push(b"}"));
        assert!(!JsonCompletion::default().push(b"[]"));
        assert!(!JsonCompletion::default().push(b"{}oops"));
    }
}
