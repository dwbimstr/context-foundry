use std::process::{Command, Output};

fn run(store: &std::path::Path, args: &[&str]) -> Output {
    let out = Command::new(env!("CARGO_BIN_EXE_foundry"))
        .arg("--store")
        .arg(store)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

#[test]
fn cli_indexes_updates_excludes_binary_and_deletes_across_processes() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("workspace");
    let store = fixture.path().join("store");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("main.rs"), "fn original_identifier() {}\n").unwrap();
    std::fs::write(root.join("image.bin"), [0, 1, 2]).unwrap();
    let indexed = run(&store, &["index", root.to_str().unwrap()]);
    let report: serde_json::Value = serde_json::from_slice(&indexed.stdout).unwrap();
    assert_eq!(report["changed"], 1);
    assert_eq!(report["skipped"].as_array().unwrap().len(), 1);
    assert_eq!(report["deletions_deferred"], false);
    let hit = run(&store, &["search", "original_identifier"]);
    let hit: serde_json::Value = serde_json::from_slice(&hit.stdout).unwrap();
    assert_eq!(hit["hits"][0]["path"], "main.rs");

    std::fs::write(root.join("main.rs"), "fn replacement_identifier() {}\n").unwrap();
    run(&store, &["index", root.to_str().unwrap()]);
    let bundle = run(
        &store,
        &["context", "replacement_identifier", "--tokens", "256"],
    );
    let text = String::from_utf8(bundle.stdout).unwrap();
    assert!(text.contains("fn replacement_identifier"));
    assert!(!text.contains("fn original_identifier"));
    assert!(
        tiktoken_rs::o200k_base_singleton()
            .encode_ordinary(&text)
            .len()
            <= 256
    );

    std::fs::remove_file(root.join("main.rs")).unwrap();
    run(&store, &["index", root.to_str().unwrap()]);
    let status = run(&store, &["status"]);
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["sources"], 0);
    assert_eq!(status["pending_sources"], 0);
}
