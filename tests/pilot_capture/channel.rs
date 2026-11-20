//! Real pinned-runtime framing controls. These do not qualify startup owners.
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::{fs::MetadataExt, process::CommandExt};
use std::process::Stdio;
use std::time::Duration;

use context_foundry::pilot_capture::{host, protocol, sha256_hex};
use serde_json::json;
use tokio::io::AsyncReadExt;

// Physically distinct modules reproduce the early preload / concatenated bridge
// loaders. Early writes must survive the late loader without a sequence reset.
const PRELOAD: &str = r#"
const { getCaptureChannel: early } = await import('./early.ts');
const first = early();
if (!first.send({type:'fault',code:'config'})) throw Error('early write');
const { getCaptureChannel: late } = await import('./late.ts');
if (first !== late()) throw Error('channel owner changed');
const original = process.env.CF_PILOT_SESSION;
process.env.CF_PILOT_SESSION = 'different-launch';
let refused = false;
try { late(); } catch { refused = true; }
process.env.CF_PILOT_SESSION = original;
if (!refused || first !== late()) throw Error('identity reuse accepted');
const onFrame = (message, payload) => {
 if (payload.length) throw Error('unexpected payload');
 if (message.type === 'tool_decision') first.send({type:'fault',code:'model_selection'});
 else if (message.type === 'close') { first.close(); setTimeout(() => process.exit(0), 25); }
 else throw Error('unexpected command');
};
first.attach(onFrame, () => {});
let duplicate = false;
try { late().attach(onFrame, () => {}); } catch { duplicate = true; }
if (!duplicate || !first.send({type:'fault',code:'internal'})) throw Error('late attachment/write');
await new Promise(() => {});
"#;

#[tokio::test]
async fn two_loaders_share_sequence_and_reject_truncated_or_replayed_input() {
    for (kind, expected_exit, codes) in [
        (
            "clean",
            0,
            vec!["config", "internal", "model_selection", "model_selection"],
        ),
        (
            "replay",
            72,
            vec!["config", "internal", "model_selection", "protocol"],
        ),
        ("truncated", 72, vec!["config", "internal"]),
        ("early-eof", 70, vec!["config", "internal"]),
        ("stale", 72, vec!["config", "internal", "protocol"]),
        ("oversized", 72, vec!["config", "internal", "protocol"]),
        ("version", 72, vec!["config", "internal", "protocol"]),
    ] {
        let dir = super::fixture_private_tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        for (name, bytes) in [
            ("early.ts", host::CHANNEL_SOURCE),
            ("late.ts", host::CHANNEL_SOURCE),
            ("preload.ts", PRELOAD.as_bytes()),
        ] {
            super::fixture_private_write(root.join(name), bytes).unwrap();
        }
        let omp = std::path::Path::new(super::OMP);
        assert_eq!(
            sha256_hex(&std::fs::read(omp).unwrap()),
            context_foundry::pilot_capture::policy::OMP_SHA256
        );
        let policy = host::sandbox_policy(
            omp,
            &root,
            &root,
            &std::path::PathBuf::from(std::env::var_os("HOME").unwrap()),
            &[],
            &[],
        )
        .unwrap();
        let (mut child_output, child_write) = std::io::pipe().unwrap();
        let (child_read, mut child_input) = std::io::pipe().unwrap();
        let inode = |fd: i32| {
            // SAFETY: fstat only reads the live pipe descriptor into our buffer.
            let mut metadata: libc::stat = unsafe { std::mem::zeroed() };
            assert_eq!(unsafe { libc::fstat(fd, &mut metadata) }, 0);
            metadata.st_ino as u64
        };
        let tx = child_write.as_raw_fd();
        let rx = child_read.as_raw_fd();
        let mut command = std::process::Command::new(host::SANDBOX_EXEC);
        command
            .args(["-p", &policy])
            .arg(omp)
            .arg("--help")
            .current_dir(&root)
            .env_clear()
            .env("HOME", &root)
            .env("TMPDIR", &root)
            .env("PATH", "/usr/bin:/bin")
            .env("BUN_OPTIONS", "--preload ./preload.ts")
            .env("BUN_RUNTIME_TRANSPILER_CACHE_PATH", "0")
            .env("CF_PILOT_API", "cf-pilot-capture-v1")
            .env("CF_PILOT_SESSION", "channel-control")
            .env("CF_PILOT_HOST", kind)
            .env("CF_PILOT_TX_FD", "3")
            .env("CF_PILOT_RX_FD", "4")
            .env("CF_PILOT_TX_INO", inode(tx).to_string())
            .env("CF_PILOT_RX_INO", inode(rx).to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        // SAFETY: async-signal-safe descriptor setup, with temporary duplicates
        // so the original descriptor numbers cannot collide with the targets.
        unsafe {
            command.pre_exec(move || {
                let a = libc::fcntl(tx, libc::F_DUPFD_CLOEXEC, 16);
                let b = libc::fcntl(rx, libc::F_DUPFD_CLOEXEC, 16);
                if a < 0 || b < 0 || libc::dup2(a, 3) < 0 || libc::dup2(b, 4) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                libc::close(a);
                libc::close(b);
                for fd in 5..1024 {
                    libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
                }
                Ok(())
            });
        }
        let mut command = tokio::process::Command::from(command);
        command.kill_on_drop(true);
        let mut child = command.spawn().unwrap();
        let pid = child.id().unwrap() as i32;
        drop(child_write);
        drop(child_read);
        let frames = tokio::task::spawn_blocking(move || {
            let mut state = protocol::InboundState::new(
                protocol::ChannelIdentity::new("channel-control".into(), kind.into()).unwrap(),
            );
            let mut result = Vec::new();
            while let Some(frame) = state.read(&mut child_output).unwrap() {
                assert!(result.len() < 8);
                result.push(frame);
            }
            result
        });
        let errors = child.stderr.take().unwrap();
        let stderr = tokio::spawn(async move {
            let mut bytes = Vec::new();
            errors.take(65537).read_to_end(&mut bytes).await.unwrap();
            bytes
        });
        let stdout = child.stdout.take().unwrap();
        let output = tokio::spawn(async move {
            let mut bytes = Vec::new();
            stdout.take(65537).read_to_end(&mut bytes).await.unwrap();
            bytes
        });
        let frame = |seq, message| {
            let header = serde_json::to_vec(
                &json!({"v":1,"session":"channel-control","host":kind,"seq":seq,"msg":message}),
            )
            .unwrap();
            let mut bytes = (header.len() as u32).to_le_bytes().to_vec();
            bytes.extend(header);
            bytes.extend(0u32.to_le_bytes());
            bytes
        };
        let decision = json!({"type":"tool_decision","call":"owned-call","allow":false});
        let close = json!({"type":"close","reason":"completed"});
        match kind {
            "clean" => {
                // A fragmented header and coalesced later frames exercise the
                // same decoder after both loaders have obtained its owner.
                let bytes = frame(1, decision.clone());
                child_input.write_all(&bytes[..3]).unwrap();
                child_input.write_all(&bytes[3..]).unwrap();
                child_input
                    .write_all(&[frame(2, decision), frame(3, close)].concat())
                    .unwrap();
            }
            "replay" => child_input
                .write_all(&[frame(1, decision.clone()), frame(1, decision)].concat())
                .unwrap(),
            "truncated" => child_input
                .write_all(&[frame(1, close), vec![5, 0]].concat())
                .unwrap(),
            "stale" => {
                let bytes = frame(1, decision);
                let text = String::from_utf8(bytes[4..bytes.len() - 4].to_vec()).unwrap();
                let header = text
                    .replace("\"host\":\"stale\"", "\"host\":\"other\"")
                    .into_bytes();
                child_input
                    .write_all(
                        &[
                            (header.len() as u32).to_le_bytes().as_slice(),
                            &header,
                            &0u32.to_le_bytes(),
                        ]
                        .concat(),
                    )
                    .unwrap();
            }
            "oversized" => child_input.write_all(&65537u32.to_le_bytes()).unwrap(),
            "version" => {
                let mut bytes = frame(1, decision);
                let index = bytes.windows(5).position(|w| w == b"\"v\":1").unwrap();
                bytes[index + 4] = b'2';
                child_input.write_all(&bytes).unwrap();
            }
            "early-eof" => {}
            _ => unreachable!(),
        }
        drop(child_input);
        let status = match tokio::time::timeout(Duration::from_secs(10), child.wait()).await {
            Ok(status) => status.unwrap(),
            Err(_) => {
                // SAFETY: only our explicitly created process group is killed.
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                }
                child.wait().await.unwrap();
                panic!("{kind}: channel control timed out");
            }
        };
        let stderr = stderr.await.unwrap();
        assert!(output.await.unwrap().len() <= 65536);
        assert_eq!(
            status.code(),
            Some(expected_exit),
            "{kind}: {}",
            String::from_utf8_lossy(&stderr)
        );
        let messages = frames.await.unwrap();
        let actual: Vec<_> = messages
            .iter()
            .map(|frame| {
                serde_json::to_value(&frame.msg).unwrap()["code"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(actual, codes, "{kind}");
        assert_eq!(
            messages.iter().map(|m| m.seq).collect::<Vec<_>>(),
            (1..=messages.len() as u32).collect::<Vec<_>>()
        );
        assert!(stderr.is_empty(), "{kind}: unexpected stderr");
        // SAFETY: signal zero only checks our reaped process group.
        assert_eq!(unsafe { libc::kill(-pid, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
        assert!(std::fs::metadata(&root).unwrap().mode() & 0o077 == 0);
    }
}
