//! Real current CLI store outputs under the pilot-selected child mask.
//! Separate from the private run_bounded launcher witness: no public override,
//! original store/model/worker/provider input, or semantic qualification.
#![cfg(all(feature = "pilot-capture", target_os = "macos"))]

use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Output};

fn private_dir(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
}

fn private_file(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

fn assert_private_tree(root: &Path) {
    let mut pending = vec![root.to_path_buf()];
    let mut count = 0;
    while let Some(path) = pending.pop() {
        count += 1;
        assert!(count <= 16_384);
        let meta = fs::symlink_metadata(&path).unwrap();
        assert_eq!(meta.uid(), unsafe { libc::getuid() });
        assert!(!meta.file_type().is_symlink());
        if meta.is_dir() {
            assert_eq!(meta.mode() & 0o7777, 0o700, "directory {}", path.display());
            pending.extend(fs::read_dir(path).unwrap().map(|e| e.unwrap().path()));
        } else {
            assert!(meta.is_file());
            assert_eq!(meta.mode() & 0o7777, 0o600, "file {}", path.display());
        }
    }
}

fn cli(base: &Path, workspace: &Path, store: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_foundry"));
    command
        .arg("--store")
        .arg(store)
        .args(args)
        .current_dir(workspace)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", base.join("home"))
        .env("TMPDIR", base.join("tmp"));
    // Only the just-forked child mask changes. The parent test process is untouched.
    unsafe {
        command.pre_exec(|| {
            libc::umask(0o077);
            Ok(())
        });
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "owned tiny-store CLI failed; exit={:?}",
        output.status.code()
    );
    output
}

#[test]
fn privacy_current_cli_creates_four_closed_lexical_stores_with_exact_private_modes() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::Builder::new()
        .prefix("cf-private-cli-stores-")
        .tempdir()
        .unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let base = fs::canonicalize(temp.path()).unwrap();
    std::mem::forget(temp);
    eprintln!("owned tiny-store evidence: {}", base.display());
    for name in ["home", "tmp"] {
        private_dir(&base.join(name));
    }
    let before_mask_probe = base.join("parent-before");
    fs::write(&before_mask_probe, b"owned parent-mask witness").unwrap();
    let parent_mode = fs::metadata(&before_mask_probe).unwrap().mode() & 0o7777;
    let mut receipts = Vec::new();
    for pair in 1..=4 {
        let workspace = base.join(format!("workspace-{pair}"));
        let store = base.join(format!("store-{pair}"));
        private_dir(&workspace);
        let symbol = format!("owned_privacy_probe_{pair}");
        let source = format!("pub fn {symbol}() -> u32 {{ {pair} }}\n");
        private_file(&workspace.join("lib.rs"), source.as_bytes());
        let before_mode = fs::metadata(workspace.join("lib.rs")).unwrap().mode();
        let indexed = cli(
            &base,
            &workspace,
            &store,
            &["index", workspace.to_str().unwrap()],
        );
        let index: serde_json::Value = serde_json::from_slice(&indexed.stdout).unwrap();
        assert_eq!(index["changed"], 1);
        assert_eq!(index["scan_complete"], true);
        assert_eq!(index["partial"], false);
        assert_private_tree(&store);
        // Each invocation is a separate exited process. Status and context
        // actually reopen the store; no active writer is inferred from metadata.
        let status = cli(&base, &workspace, &store, &["status"]);
        let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
        assert_eq!(status["source_count"], 1);
        assert_eq!(status["index_state"], "ready");
        assert_eq!(status["pending_count"], 0);
        assert_private_tree(&store);
        let context = cli(
            &base,
            &workspace,
            &store,
            &["context", &symbol, "--tokens", "512"],
        );
        let parsed =
            context_foundry::testkit::parse_v2(std::str::from_utf8(&context.stdout).unwrap())
                .unwrap();
        assert!(
            parsed
                .items
                .iter()
                .any(|item| item.body.contains(&format!("fn {symbol}")))
        );
        assert_private_tree(&store);
        assert_eq!(
            fs::read(workspace.join("lib.rs")).unwrap(),
            source.as_bytes()
        );
        assert_eq!(
            fs::metadata(workspace.join("lib.rs")).unwrap().mode(),
            before_mode
        );
        receipts.push(serde_json::json!({"pair":pair,"index":index,"status":status,"context_sha256":context_foundry::pilot_capture::sha256_hex(&context.stdout)}));
    }
    let after_mask_probe = base.join("parent-after");
    fs::write(&after_mask_probe, b"owned parent-mask witness").unwrap();
    assert_eq!(
        fs::metadata(&after_mask_probe).unwrap().mode() & 0o7777,
        parent_mode
    );
    private_file(
        &base.join("receipt.json"),
        &serde_json::to_vec_pretty(&receipts).unwrap(),
    );
}
