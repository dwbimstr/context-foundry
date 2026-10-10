//! Synthetic generation behind the REAL worker isolation and bounded IPC.
use context_foundry::knowledge::local::{
    protocol::Hello,
    runtime::{self, Generator},
};
use context_foundry::knowledge::provider::{Reply, Usage};
use serde_json::{Value, json};
struct Fixture {
    memory: u64,
    model_fd: i32,
}
impl Generator for Fixture {
    fn generate(&mut self, body: &Value) -> Result<(Reply, u64), &'static str> {
        let system = body["messages"][0]["content"].as_str().unwrap_or("");
        let command = system
            .rsplit_once("\nfixture:")
            .map(|(_, command)| command)
            .unwrap_or("");
        if command == "hang" {
            std::thread::sleep(std::time::Duration::from_secs(120));
        }
        if command == "spin" {
            loop {
                std::hint::black_box(17u64.wrapping_mul(23));
            }
        }
        if command == "oversize" || command == "wrong_id" {
            let bytes = if command == "oversize" {
                ((context_foundry::knowledge::local::protocol::MAX_FRAME + 1) as u32)
                    .to_le_bytes()
                    .to_vec()
            } else {
                context_foundry::knowledge::local::protocol::encode(
                    &context_foundry::knowledge::local::protocol::Answer::Failed {
                        id: 999,
                        code: "knowledge_resource_limit".into(),
                    },
                )
                .unwrap()
            };
            unsafe {
                libc::write(1, bytes.as_ptr().cast(), bytes.len());
            }
            std::thread::sleep(std::time::Duration::from_secs(120));
        }
        if command == "allocate" {
            let size = self.memory as usize * 2;
            let address = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    size,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_PRIVATE | libc::MAP_ANON,
                    -1,
                    0,
                )
            };
            if address == libc::MAP_FAILED {
                return Err("knowledge_resource_limit");
            }
            unsafe { libc::munmap(address, size) };
            return Err("knowledge_fixture_limit_missing");
        }
        let mut unknowns = vec![];
        if let Some(path) = command.strip_prefix("probe:") {
            let outside = std::fs::read(path)
                .is_err_and(|e| matches!(e.raw_os_error(), Some(libc::EACCES | libc::EPERM)));
            let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
            let network =
                fd < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM);
            if fd >= 0 {
                unsafe { libc::close(fd) };
            }
            let exec = std::process::Command::new("/bin/true")
                .status()
                .is_err_and(|e| e.raw_os_error() == Some(libc::EPERM));
            let mut byte = [0u8; 1];
            let model = unsafe { libc::pread(self.model_fd, byte.as_mut_ptr().cast(), 1, 0) } == 1;
            let thread = std::thread::spawn(|| 42).join().unwrap() == 42;
            let async_owner = unsafe { libc::fcntl(1, libc::F_SETOWN, libc::getppid()) } < 0
                && std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM);
            let env = std::env::vars()
                .all(|(key, _)| matches!(key.as_str(), "HOME" | "TMPDIR" | "PATH" | "LANG"));
            unknowns.push(format!("probe outside={outside} network={network} exec={exec} model={model} thread={thread} env={env} async_owner={async_owner}"));
        }
        let claim =
            json!({"text":"fixture capacity eviction behavior", "citations":[0], "identifiers":[]});
        let content = json!({"purpose":[claim], "behavior":[claim], "provides":[], "consumes":[], "constraints":[], "tests":[], "unknowns":unknowns}).to_string();
        Ok((
            Reply {
                content,
                usage: Usage {
                    prompt_tokens: Some(if command == "usage" { u64::MAX } else { 37 }),
                    completion_tokens: Some(13),
                },
            },
            0,
        ))
    }
}
fn main() {
    let code = runtime::serve(|hello: &Hello| {
        Ok(Box::new(Fixture {
            memory: hello.config.memory_bytes,
            model_fd: hello.model_fd,
        }))
    });
    std::process::exit(code);
}
