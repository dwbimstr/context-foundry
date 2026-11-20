//! Actual pinned-runtime controls for startup refusal receipt ordering.
use context_foundry::neural::protocol::read_frame_as;
use context_foundry::pilot_capture::{host, policy, sha256_hex, startup};
use serde_json::json;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::{fs::MetadataExt, process::CommandExt};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;

const DRIVER: &str = r#"
const refuse=globalThis[Symbol.for('omp.connection-prewarm.refusal.v1')];
const metadata={owner:'model_connection_warmup',method:'HEAD',origin:'https://api.anthropic.com'};
const first=refuse(metadata), second=refuse(metadata), third=refuse(metadata);
Promise.all([first,second,third]).then(()=>{
 const channel=getCaptureChannel();
 if(!channel.send({type:'owner_settled',owner:'model_connection_warmup'}))process.exit(74);
 channel.close();setTimeout(()=>process.exit(0),25);
});
await new Promise(()=>{});
"#;

#[tokio::test]
async fn startup_refusals_reject_stale_digest_sequence_and_reordered_attempts() {
    let mut mismatches = Vec::new();
    for (kind, replies, expected_exit, expected_frames) in [
        ("clean", [(1, 3, 'a'), (2, 5, 'b'), (3, 7, 'c')], 0, 4),
        (
            "duplicate_seq",
            [(1, 3, 'a'), (2, 3, 'b'), (3, 7, 'c')],
            75,
            3,
        ),
        (
            "decreasing_seq",
            [(1, 5, 'a'), (2, 3, 'b'), (3, 7, 'c')],
            75,
            3,
        ),
        (
            "duplicate_digest",
            [(1, 3, 'a'), (2, 5, 'a'), (3, 7, 'c')],
            75,
            3,
        ),
        (
            "repeated_digest",
            [(1, 3, 'a'), (2, 5, 'b'), (3, 7, 'a')],
            75,
            3,
        ),
        ("reordered", [(2, 3, 'a'), (1, 5, 'b'), (3, 7, 'c')], 75, 3),
    ] {
        let dir = super::fixture_private_tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let mut preload = host::STARTUP_OBSERVER_SOURCE.to_vec();
        preload.extend_from_slice(DRIVER.as_bytes());
        super::fixture_private_write(root.join("preload.ts"), &preload).unwrap();
        let omp = std::path::Path::new(super::OMP);
        assert_eq!(sha256_hex(&std::fs::read(omp).unwrap()), policy::OMP_SHA256);
        let sandbox = host::sandbox_policy(
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
            // SAFETY: zeroed storage is valid for fstat, which initializes it.
            let mut m: libc::stat = unsafe { std::mem::zeroed() };
            // SAFETY: fd is a live pipe and m is writable stat storage.
            assert_eq!(unsafe { libc::fstat(fd, &mut m) }, 0);
            m.st_ino as u64
        };
        let tx = child_write.as_raw_fd();
        let rx = child_read.as_raw_fd();
        let mut command = std::process::Command::new(host::SANDBOX_EXEC);
        command
            .args(["-p", &sandbox])
            .arg(omp)
            .arg("--help")
            .current_dir(&root)
            .env_clear()
            .env("HOME", &root)
            .env("TMPDIR", &root)
            .env("PATH", "/usr/bin:/bin")
            .env("BUN_OPTIONS", "--preload ./preload.ts")
            .env("BUN_RUNTIME_TRANSPILER_CACHE_PATH", "0")
            .env("CF_PILOT_API", "cf-pilot-startup-v2")
            .env("CF_PILOT_SESSION", "refusal-control")
            .env("CF_PILOT_HOST", kind)
            .env("CF_PILOT_TX_FD", "3")
            .env("CF_PILOT_RX_FD", "4")
            .env("CF_PILOT_TX_INO", inode(tx).to_string())
            .env("CF_PILOT_RX_INO", inode(rx).to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        // SAFETY: only async-signal-safe descriptor operations run before exec.
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
            let mut result = Vec::new();
            loop {
                match read_frame_as::<startup::ChildHeader, _>(&mut child_output) {
                    Ok((h, p)) => {
                        assert!(p.is_empty());
                        assert!(result.len() < 5);
                        result.push(h)
                    }
                    Err(context_foundry::neural::protocol::FrameError::Eof) => break,
                    Err(e) => panic!("bad child frame: {e:?}"),
                }
            }
            result
        });
        let stderr = child.stderr.take().unwrap();
        let err = tokio::spawn(async move {
            let mut b = Vec::new();
            stderr.take(65537).read_to_end(&mut b).await.unwrap();
            b
        });
        let stdout = child.stdout.take().unwrap();
        let out = tokio::spawn(async move {
            let mut b = Vec::new();
            stdout.take(65537).read_to_end(&mut b).await.unwrap();
            b
        });
        let mut bytes = Vec::new();
        for (seq, (attempt, journal_seq, digest)) in replies.into_iter().enumerate() {
            let h=serde_json::to_vec(&json!({"v":2,"session":"refusal-control","host":kind,"seq":seq+1,"msg":{"type":"startup_refused","attempt":attempt,"journal_seq":journal_seq,"journal_sha256":digest.to_string().repeat(64)}})).unwrap();
            bytes.extend((h.len() as u32).to_le_bytes());
            bytes.extend(h);
            bytes.extend(0u32.to_le_bytes());
        }
        child_input.write_all(&bytes).unwrap();
        let status = match tokio::time::timeout(Duration::from_secs(10), child.wait()).await {
            Ok(v) => v.unwrap(),
            Err(_) => {
                // SAFETY: this is the process group created by this test.
                unsafe { libc::kill(-pid, libc::SIGKILL) };
                child.wait().await.unwrap();
                panic!("{kind}: timeout")
            }
        };
        drop(child_input);
        let stderr = err.await.unwrap();
        assert!(out.await.unwrap().len() <= 65536);
        let actual = status.code();
        if actual != Some(expected_exit) {
            mismatches.push(format!(
                "{kind}: expected {expected_exit}, got {actual:?}; stderr={}",
                String::from_utf8_lossy(&stderr)
            ));
        }
        let frames = frames.await.unwrap();
        assert!(
            actual == Some(0) || actual == Some(75),
            "{kind}: unexpected exit {actual:?}"
        );
        assert_eq!(
            frames.len(),
            if actual == Some(0) { 4 } else { 3 },
            "{kind}"
        );
        assert!(matches!(
            frames[0].msg,
            startup::ChildMessage::StartupAttempt { attempt: 1, .. }
        ));
        assert!(matches!(
            frames[1].msg,
            startup::ChildMessage::StartupAttempt { attempt: 2, .. }
        ));
        assert!(matches!(
            frames[2].msg,
            startup::ChildMessage::StartupAttempt { attempt: 3, .. }
        ));
        if actual == Some(0) {
            assert!(matches!(
                frames[3].msg,
                startup::ChildMessage::OwnerSettled { .. }
            ));
        }
        // SAFETY: signal zero only checks the reaped test process group.
        assert_eq!(unsafe { libc::kill(-pid, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
        assert_eq!(std::fs::metadata(&root).unwrap().mode() & 0o077, 0);
        assert_eq!(expected_frames, if expected_exit == 0 { 4 } else { 3 });
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("; "));
}
