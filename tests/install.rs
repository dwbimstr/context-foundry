//! The installable package end to end (deployment § Lifecycle and
//! installation; 013 T004's packaged lifecycle): `scripts/package.sh`
//! packages the debug binaries, `scripts/install.sh` installs them into a
//! temporary prefix and the installed `foundry` serves a temporary
//! repository. Upgrade, rollback, the refusals (a running owner, before and
//! at the cutover; a live lock; a foreign launcher; a linked layout; unsafe
//! path arguments), an interrupted switch, the optional worker bundles
//! (around the fake workers), disable and uninstall run against real files,
//! processes and host-config bytes. The release-build install with the real
//! worker bundles is a separate measurement. The package target is macOS
//! arm64 (`scripts/package.sh` refuses any other host), so this file builds
//! only there.
#![cfg(all(target_os = "macos", target_arch = "aarch64"))]

use context_foundry::bootstrap::{OWNED_BEGIN, OWNED_END};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Output, Stdio};
use std::time::{Duration, Instant};

const FOUNDRY: &str = env!("CARGO_BIN_EXE_foundry");
const VERSION: &str = env!("CARGO_PKG_VERSION");
const SOURCE: &str = "pub fn parse_record() -> u32 {\n    7\n}\n";

/// Start the installed `foundry mcp` owner and wait until it has served an
/// `initialize`: from then on it certainly runs the installed executable.
fn start_owner(installed: &Path, store: &Path, root: &Path) -> (Child, ChildStdin) {
    let mut owner = Command::new(installed)
        .arg("--store")
        .arg(store)
        .arg("mcp")
        .arg("--root")
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = owner.stdin.take().unwrap();
    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
               "params": {"protocolVersion": "2025-11-25", "capabilities": {},
                          "clientInfo": {"name": "install-test", "version": "0"}}})
    )
    .unwrap();
    let mut reply = String::new();
    BufReader::new(owner.stdout.take().unwrap())
        .read_line(&mut reply)
        .unwrap();
    assert!(
        reply.contains("\"result\""),
        "the installed owner serves: {reply}"
    );
    (owner, stdin)
}

/// Wait for a test barrier file `waiter` creates; fail as soon as `waiter`
/// exits without creating it.
fn wait_for(path: &Path, waiter: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(600);
    while !path.exists() {
        assert!(
            waiter.try_wait().unwrap().is_none(),
            "the installer exited before {}",
            path.display()
        );
        assert!(
            Instant::now() < deadline,
            "{} never appeared",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn script(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join(name)
}

fn ok(out: Output) -> Output {
    assert!(
        out.status.success(),
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn sha256(path: &Path) -> String {
    context_foundry::digest(&std::fs::read(path).unwrap())
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// A canonical temporary directory: `lsof` and the installer report
/// physical paths.
fn scratch() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().canonicalize().unwrap();
    (dir, base)
}

/// Package the executables in `bin_dir`; `label` relabels the package the
/// way only tests may (`CF_TEST_VERSION_LABEL`). No llama.cpp checkout is
/// named: a packaged worker supplies its own notices.
fn package(bin_dir: &Path, out: &Path, label: Option<&str>, extra: &[&str]) -> PathBuf {
    ok(package_command(bin_dir, out, label, extra)
        .output()
        .unwrap());
    out.join(format!(
        "context-foundry-{}-macos-arm64.tar.gz",
        label.unwrap_or(VERSION)
    ))
}

fn package_command(bin_dir: &Path, out: &Path, label: Option<&str>, extra: &[&str]) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .arg(script("package.sh"))
        .arg("--out")
        .arg(out)
        .arg("--bin-dir")
        .arg(bin_dir)
        .args(extra)
        .env("CARGO", env!("CARGO"))
        .env_remove("CF_TEST_VERSION_LABEL")
        .env_remove("LLAMA_CPP_DIR");
    if let Some(label) = label {
        command.env("CF_TEST_VERSION_LABEL", label);
    }
    command
}

/// One member of a package, read without unpacking it.
fn package_member(package: &Path, member: &str) -> Vec<u8> {
    let name = package.file_name().unwrap().to_str().unwrap();
    let top = name.strip_suffix(".tar.gz").unwrap();
    ok(Command::new("tar")
        .arg("-xOzf")
        .arg(package)
        .arg(format!("{top}/{member}"))
        .output()
        .unwrap())
    .stdout
}

fn installer(args: &[&dyn AsRef<std::ffi::OsStr>]) -> Output {
    let mut command = Command::new("/bin/sh");
    command.arg(script("install.sh"));
    for arg in args {
        command.arg(arg);
    }
    command.output().unwrap()
}

/// Run the installer with the test hook killing it at `point`; it must die
/// of SIGKILL there.
fn killed_at(point: &str, args: &[&dyn AsRef<std::ffi::OsStr>]) {
    let mut command = Command::new("/bin/sh");
    command
        .arg(script("install.sh"))
        .env("CF_TEST_HOOK", format!("kill:{point}"));
    for arg in args {
        command.arg(arg);
    }
    let out = command.output().unwrap();
    assert_eq!(
        out.status.signal(),
        Some(libc::SIGKILL),
        "killed at {point}: {}",
        text(&out.stderr)
    );
}

fn foundry(binary: &Path, store: &Path, args: &[&dyn AsRef<std::ffi::OsStr>]) -> Output {
    foundry_with_stdin(binary, store, args, None)
}

fn foundry_with_stdin(
    binary: &Path,
    store: &Path,
    args: &[&dyn AsRef<std::ffi::OsStr>],
    stdin: Option<&str>,
) -> Output {
    let mut command = Command::new(binary);
    command.arg("--store").arg(store);
    for arg in args {
        command.arg(arg);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut pipe = child.stdin.take().unwrap();
    if let Some(input) = stdin {
        pipe.write_all(input.as_bytes()).unwrap();
    }
    drop(pipe);
    child.wait_with_output().unwrap()
}

/// Every regular file and link under `dir` with its bytes (a link maps to
/// its target text).
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            if meta.is_dir() {
                pending.push(path);
            } else if meta.file_type().is_symlink() {
                let target = std::fs::read_link(&path).unwrap();
                files.insert(path, target.into_os_string().into_encoded_bytes());
            } else {
                let bytes = std::fs::read(&path).unwrap();
                files.insert(path, bytes);
            }
        }
    }
    files
}

/// The versions `foundry bootstrap` inspection reports for `binary`.
fn versions(binary: &Path, root: &Path) -> Value {
    let out = ok(Command::new(binary)
        .arg("bootstrap")
        .arg("--root")
        .arg(root)
        .output()
        .unwrap());
    serde_json::from_slice::<Value>(&out.stdout).unwrap()["versions"].clone()
}

/// Every file installed.json owns still has its recorded hash.
fn assert_owned_files_match(prefix: &Path) {
    let state = read_json(&prefix.join("lib/context-foundry/installed.json"));
    let files = state["files"].as_object().unwrap();
    assert!(!files.is_empty());
    for (path, sha) in files {
        assert_eq!(
            &Value::String(sha256(&prefix.join(path))),
            sha,
            "owned file {path}"
        );
    }
}

fn current_link(prefix: &Path) -> PathBuf {
    std::fs::read_link(prefix.join("lib/context-foundry/current")).unwrap()
}

#[test]
fn package_install_upgrade_rollback_uninstall_keep_user_data() {
    let (_dir, base) = scratch();
    let bin = base.join("bin");
    std::fs::create_dir(&bin).unwrap();
    std::os::unix::fs::symlink(FOUNDRY, bin.join("foundry")).unwrap();
    let packages = base.join("packages");
    let second_label = format!("{VERSION}-test.2");
    let first = package(&bin, &packages, None, &[]);
    let second = package(&bin, &packages, Some(&second_label), &[]);

    // The manifest is strict JSON: identity, versions and every file's hash.
    let manifest: Value = serde_json::from_slice(&package_member(&first, "PACKAGE.json")).unwrap();
    assert_eq!(manifest["version"], VERSION);
    assert_eq!(manifest["target"], "aarch64-apple-darwin");
    assert_eq!(manifest["components"], json!(["core"]));
    assert_eq!(manifest["versions"]["core"], VERSION);
    assert_eq!(
        manifest["versions"]["store_schema"],
        context_foundry::SCHEMA_VERSION
    );
    let commit = manifest["git_commit"].as_str().unwrap();
    assert!(commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()));
    let files = manifest["files"].as_object().unwrap();
    assert_eq!(files["bin/foundry"], sha256(Path::new(FOUNDRY)));
    for member in [
        "README.md",
        "LICENSE",
        "scripts/install.sh",
        "THIRD-PARTY/INDEX.json",
    ] {
        assert!(files.contains_key(member), "{member} is packaged");
    }
    assert!(
        !files
            .keys()
            .any(|path| path.starts_with("libexec/") || path.contains("worker-bundle")),
        "a core package carries no worker: {files:?}"
    );
    // Every runtime dependency has its notice directory and index row.
    let index: Value =
        serde_json::from_slice(&package_member(&first, "THIRD-PARTY/INDEX.json")).unwrap();
    let rows = index.as_array().unwrap();
    assert_eq!(
        rows.len() as u64,
        manifest["dependencies"]["third_party_packages"]
            .as_u64()
            .unwrap()
    );
    let redb = rows
        .iter()
        .find(|row| row["name"] == "redb")
        .expect("redb is a runtime dependency");
    let redb_files = redb["files"].as_array().unwrap();
    assert!(!redb_files.is_empty());
    let first_notice = format!(
        "THIRD-PARTY/redb-{}/{}",
        redb["version"].as_str().unwrap(),
        redb_files[0].as_str().unwrap()
    );
    assert!(files.contains_key(&first_notice), "{first_notice}");
    for row in rows {
        assert!(
            !row["files"].as_array().unwrap().is_empty(),
            "{row}: every package has a license file or a supplied notice"
        );
    }

    // The relabel is honoured only for prebuilt test binaries, in test form.
    let refused = Command::new("/bin/sh")
        .arg(script("package.sh"))
        .arg("--out")
        .arg(&packages)
        .env("CF_TEST_VERSION_LABEL", format!("{VERSION}-test.3"))
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(64), "{}", text(&refused.stderr));
    let refused = Command::new("/bin/sh")
        .arg(script("package.sh"))
        .arg("--out")
        .arg(&packages)
        .arg("--bin-dir")
        .arg(&bin)
        .env("CF_TEST_VERSION_LABEL", "9.9.9")
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(64), "{}", text(&refused.stderr));

    // Install: the link resolves into the version directory; every owned
    // file is recorded with its hash.
    let prefix = base.join("prefix");
    let lib = prefix.join("lib/context-foundry");
    ok(installer(&[
        &"install",
        &"--package",
        &first,
        &"--prefix",
        &prefix,
    ]));
    let installed = prefix.join("bin/foundry");
    assert_eq!(
        installed.canonicalize().unwrap(),
        lib.join(VERSION).join("bin/foundry")
    );
    assert_eq!(sha256(&installed), sha256(Path::new(FOUNDRY)));
    let state = read_json(&lib.join("installed.json"));
    assert_eq!(
        (state["current"].as_str(), state["previous"].as_str()),
        (Some(VERSION), Some(""))
    );
    assert!(
        state["files"]
            .as_object()
            .unwrap()
            .contains_key(&format!("lib/context-foundry/{VERSION}/bin/foundry"))
    );
    assert_owned_files_match(&prefix);
    let again = installer(&[&"install", &"--package", &first, &"--prefix", &prefix]);
    assert_eq!(again.status.code(), Some(73), "a second install is refused");
    // An operator directory under the installation is never pruned.
    let operator_dir = lib.join("operator-cache");
    std::fs::create_dir(&operator_dir).unwrap();

    // A launcher the installer did not make is never replaced: the command
    // refuses before any change.
    let wrapper = b"#!/bin/sh\nexec /usr/bin/true\n";
    std::fs::remove_file(&installed).unwrap();
    std::fs::write(&installed, wrapper).unwrap();
    let state_bytes = std::fs::read(lib.join("installed.json")).unwrap();
    let refused = installer(&[&"upgrade", &"--package", &second, &"--prefix", &prefix]);
    assert_eq!(refused.status.code(), Some(65), "{}", text(&refused.stderr));
    assert_eq!(std::fs::read(&installed).unwrap(), wrapper);
    assert_eq!(
        std::fs::read(lib.join("installed.json")).unwrap(),
        state_bytes
    );
    assert!(!lib.join(&second_label).exists());
    std::fs::remove_file(&installed).unwrap();
    std::os::unix::fs::symlink("../lib/context-foundry/current/bin/foundry", &installed).unwrap();

    // One lifecycle command at a time: a live holder's lock refuses.
    let lock = lib.join(".lock");
    std::fs::create_dir(&lock).unwrap();
    let started = ok(Command::new("ps")
        .args(["-o", "lstart=", "-p"])
        .arg(std::process::id().to_string())
        .output()
        .unwrap())
    .stdout;
    std::fs::write(
        lock.join("owner"),
        [format!("{}\n", std::process::id()).as_bytes(), &started].concat(),
    )
    .unwrap();
    let locked = installer(&[&"rollback", &"--prefix", &prefix]);
    assert_eq!(locked.status.code(), Some(75), "{}", text(&locked.stderr));
    assert_eq!(
        std::fs::read(lib.join("installed.json")).unwrap(),
        state_bytes
    );
    std::fs::remove_dir_all(&lock).unwrap();

    // The installed foundry indexes, searches and keeps a memory.
    let ws = base.join("ws");
    std::fs::create_dir(&ws).unwrap();
    std::fs::write(ws.join("lib.rs"), SOURCE).unwrap();
    let store = base.join("store");
    ok(foundry(&installed, &store, &[&"index", &ws]));
    let hits = text(&ok(foundry(&installed, &store, &[&"search", &"parse_record"])).stdout);
    assert!(hits.contains("lib.rs"), "{hits}");
    let status: Value =
        serde_json::from_slice(&ok(foundry(&installed, &store, &[&"status"])).stdout).unwrap();
    let wid = status["workspace_id"].as_str().unwrap().to_owned();
    let record = json!({
        "id": "install-note", "workspace_id": wid, "text": "survives upgrades",
        "author": "tester", "provenance": "install test", "source_links": [],
    });
    ok(foundry_with_stdin(
        &installed,
        &store,
        &[&"memory", &"put"],
        Some(&record.to_string()),
    ));
    let memory_get: [&dyn AsRef<std::ffi::OsStr>; 6] = [
        &"memory",
        &"get",
        &"--id",
        &"install-note",
        &"--workspace-id",
        &wid,
    ];
    let memory = ok(foundry(&installed, &store, &memory_get)).stdout;

    // The installed binary reports its versions.
    let reported = versions(&installed, &ws);
    assert_eq!(reported["core"], VERSION);
    assert_eq!(reported["store_schema"], context_foundry::SCHEMA_VERSION);
    assert_eq!(
        reported["embed_protocol"],
        context_foundry::neural::protocol::PROTOCOL_VERSION
    );
    assert_eq!(
        reported["learn_protocol"],
        context_foundry::learning::ipc::LEARN_PROTOCOL
    );
    assert_eq!(
        reported["predict_protocol"],
        context_foundry::learning::ipc::PREDICT_PROTOCOL
    );
    assert_eq!(reported["foundry_embed"], Value::Null);
    assert_eq!(reported["foundry_learn"], Value::Null);

    // Upgrade refuses while an installed owner runs, found by executable
    // path; nothing changes. The barrier is a served `initialize`.
    let (mut owner, owner_stdin) = start_owner(&installed, &store, &ws);
    let busy = installer(&[&"upgrade", &"--package", &second, &"--prefix", &prefix]);
    assert_eq!(busy.status.code(), Some(75), "{}", text(&busy.stderr));
    assert!(
        text(&busy.stderr).contains(&format!("pid {}:", owner.id())),
        "{}",
        text(&busy.stderr)
    );
    assert_eq!(
        std::fs::read(lib.join("installed.json")).unwrap(),
        state_bytes
    );
    assert_eq!(current_link(&prefix), Path::new(VERSION));
    assert!(!lib.join(&second_label).exists());
    owner.kill().unwrap();
    owner.wait().unwrap();
    drop(owner_stdin);

    // The cutover checks again: an owner started while the upgrade
    // prepared (held at the test barrier before its cutover) still refuses
    // it, and the prepared version is removed.
    let barrier = base.join("barrier");
    std::fs::create_dir(&barrier).unwrap();
    let mut upgrade = Command::new("/bin/sh")
        .arg(script("install.sh"))
        .args(["upgrade", "--package"])
        .arg(&second)
        .arg("--prefix")
        .arg(&prefix)
        .env(
            "CF_TEST_HOOK",
            format!("wait:before-cutover:{}", barrier.display()),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for(&barrier.join("arrived"), &mut upgrade);
    // The prepared version is still private: nothing is placed before the
    // cutover check.
    assert!(!lib.join(&second_label).exists());
    let (mut owner, owner_stdin) = start_owner(&installed, &store, &ws);
    std::fs::write(barrier.join("go"), "").unwrap();
    let late = upgrade.wait_with_output().unwrap();
    assert_eq!(late.status.code(), Some(75), "{}", text(&late.stderr));
    assert!(text(&late.stderr).contains(&format!("pid {}:", owner.id())));
    assert_eq!(
        std::fs::read(lib.join("installed.json")).unwrap(),
        state_bytes
    );
    assert_eq!(current_link(&prefix), Path::new(VERSION));
    assert!(!lib.join(&second_label).exists());
    owner.kill().unwrap();
    owner.wait().unwrap();
    drop(owner_stdin);

    // Upgrade beside the old version; the store and memory are intact, and
    // the installer says which stores need upgrade-store without running it.
    let old_store = base.join("old-store");
    let old_root = base.join("old-root");
    context_foundry::testkit::craft_v4_store(&old_store, &old_root);
    let upgraded = ok(installer(&[
        &"upgrade",
        &"--package",
        &second,
        &"--prefix",
        &prefix,
        &"--store",
        &store,
        &"--store",
        &old_store,
    ]));
    let said = text(&upgraded.stdout);
    assert!(said.contains("store schema unchanged"), "{said}");
    assert!(
        said.contains(&format!("store {}: schema", store.display()))
            && said.contains("no upgrade-store needed"),
        "{said}"
    );
    assert!(
        said.contains(&format!(
            "store {}: needs 'foundry --store {} upgrade-store --to {}'",
            old_store.display(),
            old_store.display(),
            context_foundry::SCHEMA_VERSION
        )),
        "{said}"
    );
    assert_eq!(
        context_foundry::testkit::schema_marker(&old_store),
        "4",
        "upgrade-store is not run"
    );
    assert_eq!(current_link(&prefix), Path::new(&second_label));
    assert_eq!(
        installed.canonicalize().unwrap(),
        lib.join(&second_label).join("bin/foundry")
    );
    assert!(
        lib.join(VERSION).join("bin/foundry").is_file(),
        "the old version stays"
    );
    let state = read_json(&lib.join("installed.json"));
    assert_eq!(
        (state["current"].as_str(), state["previous"].as_str()),
        (Some(second_label.as_str()), Some(VERSION))
    );
    assert_owned_files_match(&prefix);
    assert_eq!(ok(foundry(&installed, &store, &memory_get)).stdout, memory);
    assert_eq!(
        text(&ok(foundry(&installed, &store, &[&"search", &"parse_record"])).stdout),
        hits
    );

    // Roll back to the previous version; it serves the same store.
    ok(installer(&[&"rollback", &"--prefix", &prefix]));
    assert_eq!(current_link(&prefix), Path::new(VERSION));
    let state = read_json(&lib.join("installed.json"));
    assert_eq!(
        (state["current"].as_str(), state["previous"].as_str()),
        (Some(VERSION), Some(second_label.as_str()))
    );
    assert_eq!(ok(foundry(&installed, &store, &memory_get)).stdout, memory);

    // A switch killed between its links and its record: the next command
    // reclaims the dead lock, completes the switch its pending.json records
    // and proceeds.
    killed_at("after-links", &[&"rollback", &"--prefix", &prefix]);
    assert!(lib.join("pending.json").is_file());
    assert_eq!(
        current_link(&prefix),
        Path::new(&second_label),
        "the link switched"
    );
    let state = read_json(&lib.join("installed.json"));
    assert_eq!(state["current"], VERSION, "the record did not");
    assert!(lib.join(".lock").is_dir(), "the dead command's lock stays");
    let recovered = ok(installer(&[&"rollback", &"--prefix", &prefix]));
    let said = text(&recovered.stdout);
    assert!(said.contains("reclaimed the lock"), "{said}");
    assert!(
        said.contains("recovered the interrupted rollback: completed it"),
        "{said}"
    );
    assert_eq!(current_link(&prefix), Path::new(VERSION));
    let state = read_json(&lib.join("installed.json"));
    assert_eq!(
        (state["current"].as_str(), state["previous"].as_str()),
        (Some(VERSION), Some(second_label.as_str()))
    );
    assert!(!lib.join(".lock").exists());
    assert!(!lib.join("pending.json").exists());
    assert_owned_files_match(&prefix);

    // A managed directory replaced by a link is never followed: uninstall
    // refuses, and the link's target keeps every file.
    let third_party = lib.join(VERSION).join("THIRD-PARTY");
    let cache = base.join("license-cache");
    std::fs::rename(&third_party, &cache).unwrap();
    std::os::unix::fs::symlink(&cache, &third_party).unwrap();
    let cache_bytes = snapshot(&cache);
    let state_bytes = std::fs::read(lib.join("installed.json")).unwrap();
    let refused = installer(&[&"uninstall", &"--prefix", &prefix]);
    assert_eq!(refused.status.code(), Some(65), "{}", text(&refused.stderr));
    assert_eq!(snapshot(&cache), cache_bytes);
    assert_eq!(
        std::fs::read(lib.join("installed.json")).unwrap(),
        state_bytes
    );
    assert!(installed.is_file());
    std::fs::remove_file(&third_party).unwrap();
    std::fs::rename(&cache, &third_party).unwrap();

    // An applied host block, a user-modified owned file, then uninstall.
    let host_config = base.join("config.toml");
    let original =
        b"model = \"o3\"\n# operator bytes \xff\xfe\napproval_policy = \"never\"".to_vec();
    std::fs::write(&host_config, &original).unwrap();
    ok(foundry(
        &installed,
        &store,
        &[
            &"connect",
            &"--host",
            &"codex",
            &"--root",
            &ws,
            &"--apply-config",
            &host_config,
        ],
    ));
    let applied = std::fs::read(&host_config).unwrap();
    assert!(applied.starts_with(&original) && applied.len() > original.len());
    let modified = lib.join(VERSION).join("README.md");
    let mut note = std::fs::read(&modified).unwrap();
    note.extend_from_slice(b"\noperator note\n");
    std::fs::write(&modified, &note).unwrap();
    let store_bytes = snapshot(&store);

    let removed = ok(installer(&[
        &"uninstall",
        &"--prefix",
        &prefix,
        &"--host-config",
        &host_config,
    ]));
    let said = text(&removed.stdout);
    assert_eq!(
        std::fs::read(&host_config).unwrap(),
        original,
        "host bytes restored"
    );
    assert!(
        said.contains(&format!(
            "kept (modified since install): {}",
            modified.display()
        )),
        "{said}"
    );
    assert_eq!(std::fs::read(&modified).unwrap(), note);
    assert!(
        std::fs::symlink_metadata(&installed).is_err(),
        "the link is gone"
    );
    let left: Vec<PathBuf> = snapshot(&prefix).into_keys().collect();
    assert_eq!(left, vec![modified.clone()], "only the modified file stays");
    assert!(
        operator_dir.is_dir(),
        "the operator's directory is never pruned"
    );
    assert_eq!(snapshot(&store), store_bytes, "the store is untouched");
    assert_eq!(
        ok(foundry(Path::new(FOUNDRY), &store, &memory_get)).stdout,
        memory,
        "the memory survives uninstall"
    );
}

/// Path arguments that canonicalization or splitting could rewrite, and a
/// prefix whose `lib` or `bin` is a link, are refused before anything is
/// created or read.
#[test]
fn unsafe_arguments_and_linked_layouts_are_refused_before_any_change() {
    let (_dir, base) = scratch();
    let package = base.join("context-foundry-0.0.0-macos-arm64.tar.gz");
    std::fs::write(&package, "never read").unwrap();
    let block_file = base.join("a");
    let block = format!("{OWNED_BEGIN}\nx = 1\n{OWNED_END}\n");
    std::fs::write(&block_file, &block).unwrap();
    let refused = |args: &[&dyn AsRef<std::ffi::OsStr>]| {
        let out = installer(args);
        assert_eq!(out.status.code(), Some(64), "{}", text(&out.stderr));
    };
    let newline = base.join("new\nline");
    refused(&[&"install", &"--package", &package, &"--prefix", &newline]);
    assert!(!newline.exists() && !base.join("new").exists());
    let slash = format!("{}/", base.join("trailing").display());
    refused(&[&"install", &"--package", &package, &"--prefix", &slash]);
    assert!(!base.join("trailing").exists());
    // A byte outside printable ASCII: lsof would print it escaped, so the
    // running-owner check could not match it.
    let accented = base.join("caf\u{e9}");
    refused(&[&"install", &"--package", &package, &"--prefix", &accented]);
    assert!(!accented.exists());
    // One --host-config value naming the same file twice across a newline.
    let split = format!("{}\n{}", block_file.display(), block_file.display());
    refused(&[&"uninstall", &"--prefix", &base, &"--host-config", &split]);
    assert_eq!(std::fs::read_to_string(&block_file).unwrap(), block);
    refused(&[
        &"upgrade",
        &"--package",
        &package,
        &"--prefix",
        &base,
        &"--store",
        &"store\twith-tab",
    ]);
    let profile = format!("{}\n", base.join("profile.json").display());
    refused(&[
        &"install",
        &"--package",
        &package,
        &"--prefix",
        &base.join("p"),
        &"--semantic-profile",
        &profile,
    ]);
    assert!(!base.join("p").exists());
    for (option, value) in [
        ("--out", base.join("out\nx").display().to_string()),
        ("--out", base.join("out-caf\u{e9}").display().to_string()),
        ("--bin-dir", format!("{}/", base.display())),
    ] {
        let out = Command::new("/bin/sh")
            .arg(script("package.sh"))
            .arg(option)
            .arg(&value)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(64), "{}", text(&out.stderr));
    }
    assert!(!base.join("out").exists() && !base.join("out-caf\u{e9}").exists());

    // A prefix whose lib or bin is a link: install refuses before creating
    // or writing anything through it.
    for linked in ["lib", "bin"] {
        let prefix = base.join(format!("linked-{linked}"));
        let target = base.join(format!("operator-{linked}"));
        std::fs::create_dir_all(&prefix).unwrap();
        std::fs::create_dir(&target).unwrap();
        std::os::unix::fs::symlink(&target, prefix.join(linked)).unwrap();
        let out = installer(&[&"install", &"--package", &package, &"--prefix", &prefix]);
        assert_eq!(out.status.code(), Some(65), "{}", text(&out.stderr));
        assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(&prefix).unwrap().count(), 1);
    }
}

/// Each kill point of a command leaves its pending.json; the next command
/// finalizes a committed record, completes or rolls back the rest exactly as
/// recorded, and refuses while an installed owner runs, when `current` names
/// no recorded endpoint, or when unrecorded host cleanup lacks the foundry.
/// An edited target file is kept, a lock the installer did not make is
/// refused, an operator directory named like a temporary is never touched,
/// and uninstall removes everything owned and nothing else.
#[test]
fn interrupted_commands_recover_from_their_recorded_intent() {
    let (_dir, base) = scratch();
    let bin = base.join("bin");
    std::fs::create_dir(&bin).unwrap();
    std::os::unix::fs::symlink(FOUNDRY, bin.join("foundry")).unwrap();
    let packages = base.join("packages");
    let first_label = format!("{VERSION}-test.2");
    let second_label = format!("{VERSION}-test.3");
    let first = package(&bin, &packages, Some(&first_label), &[]);
    let second = package(&bin, &packages, Some(&second_label), &[]);
    let prefix = base.join("prefix");
    let lib = prefix.join("lib/context-foundry");
    let operator = lib.join(".staging.learning");
    std::fs::create_dir_all(&operator).unwrap();
    std::fs::write(operator.join("data"), "operator data").unwrap();
    let launcher = prefix.join("bin/foundry");
    let consistent = |current: &str| {
        assert!(!lib.join("pending.json").exists(), "the intent is dropped");
        let state = read_json(&lib.join("installed.json"));
        assert_eq!(state["current"], current);
        assert_eq!(current_link(&prefix), Path::new(current));
        assert_eq!(
            std::fs::read_link(&launcher).unwrap(),
            Path::new("../lib/context-foundry/current/bin/foundry")
        );
        assert_owned_files_match(&prefix);
    };
    let install: [&dyn AsRef<std::ffi::OsStr>; 5] =
        [&"install", &"--package", &first, &"--prefix", &prefix];
    let upgrade: [&dyn AsRef<std::ffi::OsStr>; 5] =
        [&"upgrade", &"--package", &second, &"--prefix", &prefix];
    let rollback: [&dyn AsRef<std::ffi::OsStr>; 3] = [&"rollback", &"--prefix", &prefix];
    let uninstall: [&dyn AsRef<std::ffi::OsStr>; 3] = [&"uninstall", &"--prefix", &prefix];

    // A first install killed after placing its version, before `current`:
    // the next install rolls it back from the record, then installs.
    killed_at("after-place", &install);
    assert!(lib.join("pending.json").is_file() && lib.join(&first_label).is_dir());
    assert!(!lib.join("current").exists() && !launcher.exists());
    let said = text(&ok(installer(&install)).stdout);
    assert!(
        said.contains("recovered the interrupted install: rolled it back"),
        "{said}"
    );
    consistent(&first_label);
    ok(installer(&uninstall));

    // Killed after `current`, before the launcher: the next command
    // completes the install and makes the missing launcher.
    killed_at("after-current", &install);
    assert_eq!(current_link(&prefix), Path::new(&first_label));
    assert!(!launcher.exists() && !lib.join("installed.json").exists());
    let again = installer(&install);
    assert_eq!(again.status.code(), Some(73), "{}", text(&again.stderr));
    assert!(
        text(&again.stdout).contains("recovered the interrupted install: completed it"),
        "{}",
        text(&again.stdout)
    );
    consistent(&first_label);

    // An upgrade killed after placing, then an operator edit in its target:
    // rolled back from the record, the edited file kept and reported.
    killed_at("after-place", &upgrade);
    // With that record pending, a `current` link to a directory it does not
    // name is refused before recovery touches anything.
    let foreign = lib.join("user-data");
    std::fs::create_dir(&foreign).unwrap();
    std::fs::write(foreign.join("checkpoint"), "operator checkpoint").unwrap();
    std::fs::remove_file(lib.join("current")).unwrap();
    std::os::unix::fs::symlink("user-data", lib.join("current")).unwrap();
    let out = installer(&rollback);
    assert_eq!(out.status.code(), Some(65), "{}", text(&out.stderr));
    assert_eq!(current_link(&prefix), Path::new("user-data"));
    assert_eq!(
        std::fs::read_to_string(foreign.join("checkpoint")).unwrap(),
        "operator checkpoint"
    );
    assert!(lib.join("pending.json").is_file() && lib.join(&second_label).is_dir());
    std::fs::remove_file(lib.join("current")).unwrap();
    std::os::unix::fs::symlink(&first_label, lib.join("current")).unwrap();
    let edited = lib.join(&second_label).join("README.md");
    std::fs::write(&edited, "operator edit\n").unwrap();
    let out = installer(&rollback);
    assert_eq!(out.status.code(), Some(69), "no previous version yet");
    let said = text(&out.stdout);
    assert!(
        said.contains("recovered the interrupted upgrade: rolled it back"),
        "{said}"
    );
    assert!(
        said.contains(&format!(
            "kept (modified since install): {}",
            edited.display()
        )),
        "{said}"
    );
    assert_eq!(std::fs::read_to_string(&edited).unwrap(), "operator edit\n");
    consistent(&first_label);
    // The operator clears what it kept before upgrading again.
    std::fs::remove_dir_all(lib.join(&second_label)).unwrap();

    // An upgrade killed after its links, before its record: completed by
    // the next command, which then rolls back.
    killed_at("after-links", &upgrade);
    assert_eq!(current_link(&prefix), Path::new(&second_label));
    let said = text(&ok(installer(&rollback)).stdout);
    assert!(
        said.contains("recovered the interrupted upgrade: completed it"),
        "{said}"
    );
    consistent(&first_label);
    assert_eq!(
        read_json(&lib.join("installed.json"))["previous"],
        second_label.as_str()
    );

    // A switch killed after committing installed.json, then an edit in its
    // target: the next command finalizes the record, keeping and reporting
    // the edit, and never undoes the switch.
    killed_at("after-commit", &rollback);
    assert_eq!(current_link(&prefix), Path::new(&second_label));
    assert!(lib.join("pending.json").is_file());
    let target_readme = lib.join(&second_label).join("README.md");
    let readme = std::fs::read(&target_readme).unwrap();
    std::fs::write(&target_readme, "operator edit\n").unwrap();
    let said = text(&ok(installer(&rollback)).stdout);
    assert!(
        said.contains(
            "recovered the interrupted rollback: installed.json was committed; finalized it"
        ),
        "{said}"
    );
    assert!(
        said.contains(&format!(
            "kept (modified since install): {}",
            target_readme.display()
        )),
        "{said}"
    );
    assert_eq!(
        std::fs::read_to_string(&target_readme).unwrap(),
        "operator edit\n"
    );
    std::fs::write(&target_readme, &readme).unwrap();
    consistent(&first_label);

    // Without a record, too, a `current` link to a directory no record names
    // is refused, never adopted.
    std::fs::remove_file(lib.join("current")).unwrap();
    std::os::unix::fs::symlink("user-data", lib.join("current")).unwrap();
    let out = installer(&uninstall);
    assert_eq!(out.status.code(), Some(65), "{}", text(&out.stderr));
    assert_eq!(
        std::fs::read_to_string(foreign.join("checkpoint")).unwrap(),
        "operator checkpoint"
    );
    std::fs::remove_file(lib.join("current")).unwrap();
    std::os::unix::fs::symlink(&first_label, lib.join("current")).unwrap();

    // A lock that is a link is refused before anything is read or removed.
    let operator_lock = base.join("operator-lock");
    std::fs::create_dir(&operator_lock).unwrap();
    std::fs::write(operator_lock.join("owner"), "99999999\nstale\n").unwrap();
    std::os::unix::fs::symlink(&operator_lock, lib.join(".lock")).unwrap();
    let out = installer(&uninstall);
    assert_eq!(out.status.code(), Some(65), "{}", text(&out.stderr));
    assert_eq!(
        std::fs::read_to_string(operator_lock.join("owner")).unwrap(),
        "99999999\nstale\n"
    );
    std::fs::remove_file(lib.join(".lock")).unwrap();

    // Host cleanup needs the installed foundry: without it, uninstall
    // refuses, naming the host file, and changes nothing.
    let ws = base.join("ws");
    std::fs::create_dir(&ws).unwrap();
    std::fs::write(ws.join("lib.rs"), SOURCE).unwrap();
    let store = base.join("store");
    ok(foundry(&launcher, &store, &[&"index", &ws]));
    let host_config = base.join("config.toml");
    let original = b"model = \"o3\"\n".to_vec();
    std::fs::write(&host_config, &original).unwrap();
    let apply_block = || {
        ok(foundry(
            &launcher,
            &store,
            &[
                &"connect",
                &"--host",
                &"codex",
                &"--root",
                &ws,
                &"--apply-config",
                &host_config,
            ],
        ));
        assert_ne!(std::fs::read(&host_config).unwrap(), original);
    };
    apply_block();
    let with_block = std::fs::read(&host_config).unwrap();
    let core = |label: &str| lib.join(label).join("bin/foundry");
    let set_mode = |path: &Path, mode: u32| {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    };
    let uninstall_hosts: [&dyn AsRef<std::ffi::OsStr>; 5] = [
        &"uninstall",
        &"--prefix",
        &prefix,
        &"--host-config",
        &host_config,
    ];
    set_mode(&core(&first_label), 0o644);
    let out = installer(&uninstall_hosts);
    assert_eq!(out.status.code(), Some(66), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains(&host_config.display().to_string()));
    assert!(!lib.join("pending.json").exists());
    assert_eq!(std::fs::read(&host_config).unwrap(), with_block);
    set_mode(&core(&first_label), 0o755);
    consistent(&first_label);

    // An uninstall interrupted once its intent is recorded: while an
    // installed owner runs, recovery refuses before doing anything.
    killed_at("after-pending", &uninstall_hosts);
    let (mut owner, owner_stdin) = start_owner(&launcher, &store, &ws);
    let out = installer(&rollback);
    assert_eq!(out.status.code(), Some(75), "{}", text(&out.stderr));
    assert!(lib.join("pending.json").is_file());
    assert_owned_files_match(&prefix);
    assert_eq!(std::fs::read(&host_config).unwrap(), with_block);
    owner.kill().unwrap();
    owner.wait().unwrap();
    drop(owner_stdin);
    // Its host cleanup is not recorded, so without the foundry recovery
    // refuses, naming the host file, and changes nothing.
    set_mode(&core(&first_label), 0o644);
    let out = installer(&rollback);
    assert_eq!(out.status.code(), Some(66), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains(&host_config.display().to_string()));
    assert!(lib.join("pending.json").is_file());
    assert_owned_files_match(&prefix);
    assert_eq!(std::fs::read(&host_config).unwrap(), with_block);
    set_mode(&core(&first_label), 0o755);
    // With it, the next command completes the uninstall.
    let out = installer(&uninstall);
    assert_eq!(out.status.code(), Some(66), "nothing is left to uninstall");
    assert!(
        text(&out.stdout).contains("recovered the interrupted uninstall: completed it"),
        "{}",
        text(&out.stdout)
    );
    assert_eq!(std::fs::read(&host_config).unwrap(), original);

    // An uninstall killed after it recorded its host cleanup: recovery
    // trusts that record and completes even without an executable foundry.
    ok(installer(&install));
    apply_block();
    killed_at("after-hosts", &uninstall_hosts);
    assert_eq!(std::fs::read(&host_config).unwrap(), original);
    assert_eq!(read_json(&lib.join("pending.json"))["phase"], "hosts-done");
    set_mode(&core(&first_label), 0o644);
    let out = installer(&uninstall);
    assert_eq!(out.status.code(), Some(66), "{}", text(&out.stderr));
    assert!(
        text(&out.stdout).contains("recovered the interrupted uninstall: completed it"),
        "{}",
        text(&out.stdout)
    );
    assert_eq!(std::fs::read(&host_config).unwrap(), original);

    // Everything owned is gone and nothing else.
    let left: Vec<PathBuf> = snapshot(&prefix).into_keys().collect();
    assert_eq!(
        left,
        vec![operator.join("data"), foreign.join("checkpoint")],
        "only operator files stay"
    );
    assert_eq!(
        std::fs::read_to_string(operator.join("data")).unwrap(),
        "operator data"
    );
}

/// Optional workers: the fake workers are packaged as `foundry-embed` and
/// `foundry-learn`, installed with operator profiles into ad-hoc-signed
/// bundles, carried into an upgrade, disabled one by one and uninstalled;
/// the supplied profiles and inputs never change.
#[cfg(all(feature = "semantic", target_os = "macos"))]
#[test]
fn worker_components_install_as_bundles_carry_forward_and_disable() {
    use context_foundry::learning::profile::LearnProfile;
    use context_foundry::neural::profile::SemanticProfile;

    let (_dir, base) = scratch();
    let inputs = base.join("inputs");
    std::fs::create_dir(&inputs).unwrap();
    let semantic_profile =
        context_foundry::testkit::write_semantic_profile(&inputs, "fake", |_| {});
    let checkpoint = inputs.join("checkpoint");
    std::fs::create_dir(&checkpoint).unwrap();
    std::fs::write(checkpoint.join("model.safetensors"), "fixture weights\n").unwrap();
    let libtorch = inputs.join("libtorch").join("lib");
    std::fs::create_dir_all(&libtorch).unwrap();
    std::fs::write(libtorch.join("libtorch.dylib"), "fixture library\n").unwrap();
    // The learning scratch root sits under the installation, as an operator
    // may place it: no prune may take it, empty or not.
    let prefix = base.join("prefix");
    let lib = prefix.join("lib/context-foundry");
    let learning_scratch = lib.join("learning-scratch");
    let learning_profile = inputs.join("learn-profile.json");
    std::fs::write(
        &learning_profile,
        serde_json::to_vec_pretty(&json!({
            "v": 1,
            "name": "fake-learn",
            "worker": {
                "bundle": "/nonexistent/FoundryLearn.app",
                "executable_sha256": "0".repeat(64),
                "scratch_root": learning_scratch,
            },
            "checkpoint_dir": checkpoint,
            "libtorch_dir": libtorch,
            "load_timeout_seconds": 60,
            "ceilings": {"memory_bytes": 1u64 << 30, "wall_seconds": 60,
                         "output_bytes": 1u64 << 20, "cpu_threads": 1},
        }))
        .unwrap(),
    )
    .unwrap();
    let supplied = snapshot(&inputs);

    let bin = base.join("bin");
    std::fs::create_dir(&bin).unwrap();
    for (name, exe) in [
        ("foundry", FOUNDRY),
        ("foundry-embed", env!("CARGO_BIN_EXE_foundry-embed-fake")),
    ] {
        std::os::unix::fs::symlink(exe, bin.join(name)).unwrap();
    }
    // The learning worker built against a LibTorch tree whose path has a
    // space: the manifest records the whole directory and its version.
    let build_libtorch = base.join("lib torch").join("lib");
    std::fs::create_dir_all(&build_libtorch).unwrap();
    std::fs::write(build_libtorch.join("libtorch.dylib"), "fixture library\n").unwrap();
    std::fs::write(base.join("lib torch").join("build-version"), "2.11.0\n").unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_foundry-learn-fake"),
        bin.join("foundry-learn"),
    )
    .unwrap();
    ok(Command::new("install_name_tool")
        .arg("-add_rpath")
        .arg(&build_libtorch)
        .arg(bin.join("foundry-learn"))
        .output()
        .unwrap());
    let packages = base.join("packages");
    let profile_arg = semantic_profile.to_str().unwrap();
    let options = [
        "--with-semantic",
        "--semantic-profile",
        profile_arg,
        "--with-learning",
    ];
    let second_label = format!("{VERSION}-test.2");
    let first = package(&bin, &packages, None, &options);
    let second = package(&bin, &packages, Some(&second_label), &options);

    let manifest: Value = serde_json::from_slice(&package_member(&first, "PACKAGE.json")).unwrap();
    assert_eq!(
        manifest["components"],
        json!(["core", "semantic", "learning"])
    );
    let files = manifest["files"].as_object().unwrap();
    assert_eq!(
        files["libexec/foundry-embed"],
        sha256(Path::new(env!("CARGO_BIN_EXE_foundry-embed-fake")))
    );
    assert!(files.contains_key("libexec/foundry-learn"));
    assert!(files.contains_key("scripts/embed-worker-bundle.sh"));
    assert!(files.contains_key("scripts/learn-worker-bundle.sh"));
    assert!(
        !files.keys().any(|path| path.ends_with(".safetensors")
            || (path.ends_with(".json") && path.contains("profile"))),
        "no weights or profiles are packaged: {files:?}"
    );
    let supplied_profile = SemanticProfile::load(&semantic_profile).unwrap();
    let dependencies = &manifest["dependencies"];
    assert_eq!(
        dependencies["semantic"]["llama_cpp"],
        supplied_profile.descriptor.llama_cpp
    );
    assert!(dependencies["semantic"].get("runtime").is_none());
    assert!(dependencies["semantic"].get("pyo3").is_none());
    // The llama.cpp notice is the one the packaged worker wrote
    // (`--notices`), counted with the crates' rows.
    let commit = supplied_profile.descriptor.llama_cpp.as_str();
    assert_eq!(
        package_member(&first, &format!("THIRD-PARTY/llama.cpp-{commit}/LICENSE")),
        context_foundry::neural::worker_runtime::FAKE_NOTICE.as_bytes()
    );
    let index: Value =
        serde_json::from_slice(&package_member(&first, "THIRD-PARTY/INDEX.json")).unwrap();
    let rows = index.as_array().unwrap();
    assert_eq!(
        rows.len() as u64,
        dependencies["third_party_packages"].as_u64().unwrap()
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row["name"] == "llama.cpp")
            .cloned()
            .collect::<Vec<_>>(),
        [json!({"name": "llama.cpp", "version": commit, "license": "MIT", "files": ["LICENSE"]})]
    );
    assert_eq!(dependencies["learning"]["libtorch_required"], "2.11.0");
    assert_eq!(
        dependencies["learning"]["libtorch_build_dir"],
        build_libtorch.display().to_string()
    );
    assert_eq!(dependencies["learning"]["libtorch_build_version"], "2.11.0");
    assert!(
        dependencies["learning"]["tch"]
            .as_str()
            .is_some_and(|v| !v.is_empty())
    );

    // Install both components: signed bundles in the version directory and
    // installed profile copies naming them.
    ok(installer(&[
        &"install",
        &"--package",
        &first,
        &"--prefix",
        &prefix,
        &"--semantic-profile",
        &semantic_profile,
        &"--learning-profile",
        &learning_profile,
    ]));
    let check_bundles = |version: &str| {
        let vdir = lib.join(version);
        for app in ["FoundryEmbed.app", "FoundryLearn.app"] {
            ok(Command::new("codesign")
                .args(["--verify", "--strict"])
                .arg(vdir.join(app))
                .output()
                .unwrap());
        }
        let semantic =
            SemanticProfile::load(&lib.join("current/profiles/semantic-profile.json")).unwrap();
        assert_eq!(semantic.worker.bundle, vdir.join("FoundryEmbed.app"));
        assert_eq!(
            semantic.worker.executable_sha256,
            sha256(&vdir.join("FoundryEmbed.app/Contents/MacOS/foundry-embed"))
        );
        assert_eq!(semantic.descriptor, supplied_profile.descriptor);
        let learning =
            LearnProfile::load(&lib.join("current/profiles/learning-profile.json")).unwrap();
        assert_eq!(learning.worker.bundle, vdir.join("FoundryLearn.app"));
        assert_eq!(
            learning.worker.executable_sha256,
            sha256(&learning.executable())
        );
        assert_eq!(learning.libtorch_dir, libtorch);
    };
    check_bundles(VERSION);
    assert_owned_files_match(&prefix);
    let installed = prefix.join("bin/foundry");
    let reported = versions(&installed, &base);
    assert_eq!(
        reported["foundry_embed"],
        lib.join(VERSION)
            .join("libexec/foundry-embed")
            .display()
            .to_string()
    );
    assert_eq!(
        reported["foundry_learn"],
        lib.join(VERSION)
            .join("libexec/foundry-learn")
            .display()
            .to_string()
    );

    // An upgrade without profile options rebuilds the bundles the previous
    // version had, for the new version.
    ok(installer(&[
        &"upgrade",
        &"--package",
        &second,
        &"--prefix",
        &prefix,
    ]));
    check_bundles(&second_label);
    assert_owned_files_match(&prefix);

    // Disable one component, then the other: only their files go, from
    // every version, and they stay disabled.
    ok(installer(&[&"disable-semantic", &"--prefix", &prefix]));
    for version in [VERSION, second_label.as_str()] {
        let vdir = lib.join(version);
        assert!(!vdir.join("libexec/foundry-embed").exists());
        assert!(!vdir.join("FoundryEmbed.app").exists());
        assert!(!vdir.join("profiles/semantic-profile.json").exists());
        assert!(vdir.join("FoundryLearn.app").exists());
        assert!(vdir.join("bin/foundry").is_file());
    }
    let reported = versions(&installed, &base);
    assert_eq!(reported["foundry_embed"], Value::Null);
    assert!(reported["foundry_learn"].is_string());
    assert_eq!(
        read_json(&lib.join("installed.json"))["disabled"],
        "semantic"
    );
    assert_owned_files_match(&prefix);
    ok(installer(&[&"disable-learning", &"--prefix", &prefix]));
    assert!(!lib.join(VERSION).join("FoundryLearn.app").exists());
    assert_eq!(versions(&installed, &base)["foundry_learn"], Value::Null);
    assert_eq!(
        read_json(&lib.join("installed.json"))["disabled"],
        "semantic learning"
    );
    assert!(learning_scratch.is_dir(), "the scratch root stays");

    ok(installer(&[&"uninstall", &"--prefix", &prefix]));
    assert!(snapshot(&prefix).is_empty(), "every owned file is gone");
    assert_eq!(
        snapshot(&inputs),
        supplied,
        "supplied profiles and inputs are untouched"
    );
    assert!(inputs.join("scratch").is_dir() && learning_scratch.is_dir());
}

/// 009 T004 package cutover. Packaging reads the llama.cpp commit from the
/// worker and refuses a profile that pins another. An upgrade over a version
/// installed with a descriptor v1 (MLX worker) profile, as the pre-T004
/// installer left it, keeps the core upgrade, disables semantic retrieval
/// by name and asks for a v2 (llama.cpp) profile; the previous version keeps
/// its bundle and v1 profile, so rollback is that binary with its profile.
#[cfg(all(feature = "semantic", target_os = "macos"))]
#[test]
fn an_upgrade_over_a_v1_semantic_profile_keeps_the_core_and_disables_semantic_by_name() {
    use context_foundry::neural::profile::SemanticProfile;

    let (_dir, base) = scratch();
    let inputs = base.join("inputs");
    std::fs::create_dir(&inputs).unwrap();
    let v2 = context_foundry::testkit::write_semantic_profile(&inputs, "fake", |_| {});
    let other_pin = context_foundry::testkit::write_semantic_profile(&inputs, "other-pin", |d| {
        d.llama_cpp = "0".repeat(40)
    });
    // The pre-T004 shape: profile and descriptor v1, no llama.cpp pin.
    let mut profile = read_json(&v2);
    profile["v"] = json!(1);
    profile["descriptor"]["v"] = json!(1);
    profile["descriptor"]
        .as_object_mut()
        .unwrap()
        .remove("llama_cpp");
    let v1 = inputs.join("profile-mlx.json");
    std::fs::write(&v1, serde_json::to_vec_pretty(&profile).unwrap()).unwrap();
    assert_eq!(
        SemanticProfile::load(&v1).unwrap_err().code(),
        "profile_unsupported"
    );

    let bin = base.join("bin");
    std::fs::create_dir(&bin).unwrap();
    for (name, exe) in [
        ("foundry", FOUNDRY),
        ("foundry-embed", env!("CARGO_BIN_EXE_foundry-embed-fake")),
    ] {
        std::os::unix::fs::symlink(exe, bin.join(name)).unwrap();
    }
    let packages = base.join("packages");
    let refused = package_command(
        &bin,
        &packages,
        None,
        &[
            "--with-semantic",
            "--semantic-profile",
            other_pin.to_str().unwrap(),
        ],
    )
    .output()
    .unwrap();
    assert_eq!(refused.status.code(), Some(65), "{}", text(&refused.stderr));
    assert!(
        text(&refused.stderr).contains(&format!(
            "the profile pins llama.cpp {}; the worker is built from {}",
            "0".repeat(40),
            context_foundry::neural::provider::LLAMA_CPP_COMMIT
        )),
        "{}",
        text(&refused.stderr)
    );
    let options = [
        "--with-semantic",
        "--semantic-profile",
        v2.to_str().unwrap(),
    ];
    let second_label = format!("{VERSION}-test.2");
    let first = package(&bin, &packages, None, &options);
    let second = package(&bin, &packages, Some(&second_label), &options);

    let prefix = base.join("prefix");
    let lib = prefix.join("lib/context-foundry");
    ok(installer(&[
        &"install",
        &"--package",
        &first,
        &"--prefix",
        &prefix,
        &"--semantic-profile",
        &v1,
    ]));
    let old = lib.join(VERSION);
    let old_profile = std::fs::read(old.join("profiles/semantic-profile.json")).unwrap();

    let upgraded = ok(installer(&[
        &"upgrade",
        &"--package",
        &second,
        &"--prefix",
        &prefix,
    ]));
    let said = text(&upgraded.stdout);
    assert!(
        said.contains(&format!(
            "semantic: disabled: {VERSION}'s installed profile is descriptor v1 (the MLX worker), \
             which {second_label} refuses (profile_unsupported)"
        )) && said.contains("give a v2 (llama.cpp) profile with --semantic-profile FILE"),
        "{said}"
    );
    assert!(
        said.contains(&format!("upgraded {VERSION} -> {second_label}")),
        "{said}"
    );
    assert_eq!(current_link(&prefix), Path::new(&second_label));
    let new = lib.join(&second_label);
    assert!(new.join("bin/foundry").is_file());
    for absent in [
        "libexec/foundry-embed",
        "scripts/embed-worker-bundle.sh",
        "FoundryEmbed.app",
        "profiles/semantic-profile.json",
    ] {
        assert!(!new.join(absent).exists(), "{absent} is not placed");
    }
    assert_eq!(
        read_json(&lib.join("installed.json"))["disabled"],
        "semantic"
    );
    let installed = prefix.join("bin/foundry");
    assert_eq!(versions(&installed, &base)["foundry_embed"], Value::Null);
    assert_owned_files_match(&prefix);

    // The previous version is untouched: rollback returns the old binary
    // with its bundle and its own v1 profile.
    ok(installer(&[&"rollback", &"--prefix", &prefix]));
    assert_eq!(current_link(&prefix), Path::new(VERSION));
    assert!(old.join("FoundryEmbed.app").is_dir());
    assert_eq!(
        std::fs::read(lib.join("current/profiles/semantic-profile.json")).unwrap(),
        old_profile
    );
    assert_eq!(
        versions(&installed, &base)["foundry_embed"],
        old.join("libexec/foundry-embed").display().to_string()
    );
    assert_owned_files_match(&prefix);
}

/// THIRD-PARTY reads each package from the source Cargo resolved (001 T008
/// review R1): the tree-sitter-ruby fork pinned by git comes from its
/// checkout, even in a Cargo home with no unpacked registry copy of that
/// name and version (one that shares the real home's index, crate cache and
/// git checkouts, and links every other unpacked registry crate).
#[test]
fn third_party_reads_a_git_pinned_package_from_its_checkout() {
    let (_dir, base) = scratch();
    let real = std::env::var_os("CARGO_HOME").map_or_else(
        || PathBuf::from(std::env::var_os("HOME").unwrap()).join(".cargo"),
        PathBuf::from,
    );
    let home = base.join("cargo-home");
    std::fs::create_dir_all(home.join("registry/src")).unwrap();
    for shared in ["git", "registry/index", "registry/cache"] {
        std::os::unix::fs::symlink(real.join(shared), home.join(shared)).unwrap();
    }
    for index in std::fs::read_dir(real.join("registry/src")).unwrap() {
        let index = index.unwrap().path();
        let copy = home.join("registry/src").join(index.file_name().unwrap());
        std::fs::create_dir(&copy).unwrap();
        for entry in std::fs::read_dir(&index).unwrap() {
            let entry = entry.unwrap();
            if !entry
                .file_name()
                .to_string_lossy()
                .starts_with("tree-sitter-ruby-")
            {
                std::os::unix::fs::symlink(entry.path(), copy.join(entry.file_name())).unwrap();
            }
        }
    }
    let bin = base.join("bin");
    std::fs::create_dir(&bin).unwrap();
    std::os::unix::fs::symlink(FOUNDRY, bin.join("foundry")).unwrap();
    let out = base.join("packages");
    ok(Command::new("/bin/sh")
        .arg(script("package.sh"))
        .arg("--out")
        .arg(&out)
        .arg("--bin-dir")
        .arg(&bin)
        .env("CARGO", env!("CARGO"))
        .env("CARGO_HOME", &home)
        .env_remove("CF_TEST_VERSION_LABEL")
        .output()
        .unwrap());
    let package = out.join(format!("context-foundry-{VERSION}-macos-arm64.tar.gz"));
    let index: Value =
        serde_json::from_slice(&package_member(&package, "THIRD-PARTY/INDEX.json")).unwrap();
    let ruby = index
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "tree-sitter-ruby")
        .unwrap();
    assert_eq!(ruby["version"], "0.23.1");
    assert_eq!(ruby["files"], json!(["LICENSE"]));
    assert!(!package_member(&package, "THIRD-PARTY/tree-sitter-ruby-0.23.1/LICENSE").is_empty());
}
