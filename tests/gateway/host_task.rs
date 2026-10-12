//! Opt-in free-form host calibration. No patch candidates or scripted replies.
use super::*;
use context_foundry::{Control, Engine, Strategy, response, store::ContextOptions, task_context};

const TASK: &str = "Edit only src/records.rs so parse_record trims Unicode whitespace on both sides of the first = and rejects an empty key. Preserve the borrowed Option<(&str, &str)> interface, allow empty values, preserve additional = characters in the value, and return None without =. Keep all other files unchanged. Use the write tool to save the repair; a separate checker will verify it. Do not just describe a patch.";
const OVERFLOW_TASK: &str = "Edit only src/records.rs so checked_total(unit_price: u64, quantity: u64, fee: u64) -> Option<u64> returns None if either multiplication or addition overflows. Never panic or wrap. Preserve exact representable totals, including zero quantities and u64::MAX. Keep the public interface and all other files unchanged. Use the write tool to save the repair; a separate checker will verify it. Do not just describe a patch.";
const PERCENT_TASK: &str = "Repair after_percent_sign in src/lib.rs: incomplete or invalid percent escapes must remain byte-for-byte unchanged, while valid upper/lowercase hex escapes decode normally. The helper is used by both the iterator and allocation/UTF-8 conversion paths; preserve their behavior, public APIs, no_std/no-alloc support and borrowing when nothing decodes. Edit only src/lib.rs and keep all other files unchanged. Use the edit tool to save the repair; a separate checker will verify it.";

const PERCENT_ASSERTIONS: &str = r#"
    use std::borrow::Cow;
    for input in ["%", "%A", "%GG", "%1G", "%G1", "%20%", "x%Qz%20y", "%0%41", "%%41", "é%zz"] {
        let mut expected = Vec::new(); let bytes = input.as_bytes(); let mut i = 0;
        while i < bytes.len() {
            let hex = |b:u8| match b { b'0'..=b'9'=>Some(b-b'0'), b'a'..=b'f'=>Some(b-b'a'+10), b'A'..=b'F'=>Some(b-b'A'+10), _=>None };
            if bytes[i] == b'%' && i+2 < bytes.len() {
                if let (Some(a),Some(b)) = (hex(bytes[i+1]),hex(bytes[i+2])) { expected.push(a*16+b);i+=3;continue; }
            }
            expected.push(bytes[i]);i+=1;
        }
        assert_eq!(records::percent_decode_str(input).collect::<Vec<_>>(), expected, "{input}");
        let cow: Cow<'_, [u8]> = records::percent_decode_str(input).into();
        assert_eq!(cow.as_ref(), expected, "{input}");
    }
    for b in 0..=255u8 {
        for input in [format!("%{b:02X}"),format!("%{b:02x}")] {
            assert_eq!(records::percent_decode_str(&input).collect::<Vec<_>>(), vec![b]);
        }
    }
    assert_eq!(records::percent_decode_str("%E2%82%AC").decode_utf8().unwrap(), "€");
    assert!(records::percent_decode_str("%FF").decode_utf8().is_err());
    assert_eq!(records::percent_decode_str("%FF").decode_utf8_lossy(), "�");
    for input in ["unchanged", "%", "%G1", "é%zz"] {
        assert!(matches!(records::percent_decode_str(input).decode_utf8().unwrap(), Cow::Borrowed(_)));
    }
    assert_eq!(records::utf8_percent_encode("a b", records::NON_ALPHANUMERIC).to_string(), "a%20b");
"#;

// This reads a stopped gateway, unlike the shared live-reader helper. Preserve
// usable receipts, but an incomplete or malformed tail cannot establish coverage.
fn stopped_receipts(bytes: &[u8]) -> (Vec<Receipt>, bool) {
    let mut rows = Vec::new();
    let mut complete = true;
    for line in bytes.split_inclusive(|&byte| byte == b'\n') {
        if !line.ends_with(b"\n") {
            complete = false;
            continue;
        }
        match Receipt::parse(line) {
            Ok(row) => rows.push(row),
            Err(_) => complete = false,
        }
    }
    (rows, complete)
}

fn receipt_accounting_complete(rows: &[Receipt], log_complete: bool, gateway: &Value) -> bool {
    log_complete
        && gateway["log_failed"] == false
        && gateway["totals_overflowed"] == false
        && gateway["attempts"].as_u64() == Some(rows.len() as u64)
        && [
            ("complete", Outcome::Complete),
            ("failed", Outcome::Failed),
            ("unknown", Outcome::Unknown),
        ]
        .iter()
        .all(|(key, outcome)| {
            gateway[*key].as_u64()
                == Some(rows.iter().filter(|row| row.outcome == *outcome).count() as u64)
        })
}

#[test]
fn stopped_receipt_damage_and_gateway_disagreement_refuse_complete_accounting() {
    let row = br#"{"v":1,"adapter_id":"test","session_id":"session","request_id":"request","model_id":"model","outcome":"complete","input_tokens":10,"output_tokens":2,"cached_input_tokens":0}"#;
    let mut bytes = row.to_vec();
    bytes.push(b'\n');
    let (rows, complete) = stopped_receipts(&bytes);
    let gateway = json!({"attempts":1,"complete":1,"failed":0,"unknown":0,"log_failed":false,"totals_overflowed":false});
    assert_eq!(rows.len(), 1);
    assert!(receipt_accounting_complete(&rows, complete, &gateway));
    for tail in [
        row.as_slice(),
        b"{\"v\":".as_slice(),
        b"invalid\n".as_slice(),
    ] {
        let mut damaged = bytes.clone();
        damaged.extend_from_slice(tail);
        let (retained, complete) = stopped_receipts(&damaged);
        assert_eq!(retained.len(), 1, "known paid work remains visible");
        assert!(!receipt_accounting_complete(&retained, complete, &gateway));
    }
    for (key, value) in [
        ("attempts", json!(2)),
        ("complete", json!(0)),
        ("unknown", json!(1)),
        ("log_failed", json!(true)),
        ("totals_overflowed", json!(true)),
    ] {
        let mut inconsistent = gateway.clone();
        inconsistent[key] = value;
        assert!(!receipt_accounting_complete(&rows, true, &inconsistent));
    }
    assert!(!receipt_accounting_complete(&rows, false, &gateway));
}

/// A smaller failed response is not an economic win. Keep raw attempt costs,
/// but only compare completion costs when both independently checked repairs
/// pass and every gateway request has known input/output/cache usage.
fn completion_comparison(rows: &[Value]) -> Value {
    let eligible = rows.len() == 2
        && rows.iter().all(|r| {
            r["verified_task"] == true
                && r["receipt_accounting_complete"] == true
                && r["host_success"] == true
                && r["timeout"] == false
                && r["requests"].as_u64().is_some_and(|n| n > 0)
                && r["requests"] == r["complete_requests"]
                && [
                    "input_tokens",
                    "output_tokens",
                    "cached_input_tokens",
                    "total_measured_task_ms",
                ]
                .iter()
                .all(|k| r[k].as_u64().is_some())
                && r["cached_input_tokens"].as_u64() <= r["input_tokens"].as_u64()
        });
    if !eligible {
        return json!({"status":"not_established","reason":"requires_two_completed_verified_repairs_and_complete_usage","monetary_cost":null});
    }
    let ratio = |key: &str| {
        let denominator = rows[0][key].as_f64().unwrap();
        (denominator > 0.0).then(|| rows[1][key].as_f64().unwrap() / denominator)
    };
    let uncached = |i: usize| {
        rows[i]["input_tokens"].as_u64().unwrap() - rows[i]["cached_input_tokens"].as_u64().unwrap()
    };
    let uncached_ratio = (uncached(0) > 0).then(|| uncached(1) as f64 / uncached(0) as f64);
    json!({"status":"paired_calibration_only","input_token_ratio":ratio("input_tokens"),"uncached_input_token_ratio":uncached_ratio,"output_token_ratio":ratio("output_tokens"),"measured_wall_ratio":ratio("total_measured_task_ms"),"monetary_cost":null,"amortized_total_cost":null})
}

#[test]
fn failed_or_incompletely_metered_repairs_never_establish_completion_savings() {
    let row = json!({"receipt_accounting_complete":true,"verified_task":true,"host_success":true,"timeout":false,"requests":2,"complete_requests":2,"input_tokens":100,"output_tokens":10,"cached_input_tokens":0,"total_measured_task_ms":200});
    assert_eq!(
        completion_comparison(&[row.clone(), row.clone()])["status"],
        "paired_calibration_only"
    );
    for (key, value) in [
        ("receipt_accounting_complete", json!(false)),
        ("receipt_accounting_complete", Value::Null),
        ("verified_task", json!(false)),
        ("host_success", json!(false)),
        ("input_tokens", Value::Null),
        ("cached_input_tokens", Value::Null),
        ("cached_input_tokens", json!(101)),
        ("complete_requests", json!(1)),
        ("timeout", json!(true)),
    ] {
        let mut failed = row.clone();
        failed[key] = value;
        assert_eq!(
            completion_comparison(&[row.clone(), failed])["status"],
            "not_established"
        );
    }
    let mut cached_control = row.clone();
    cached_control["cached_input_tokens"] = json!(60);
    let mut cached_candidate = row;
    cached_candidate["input_tokens"] = json!(200);
    cached_candidate["cached_input_tokens"] = json!(180);
    let comparison = completion_comparison(&[cached_control, cached_candidate]);
    assert_eq!(comparison["input_token_ratio"], 2.0);
    assert_eq!(comparison["uncached_input_token_ratio"], 0.5);
    assert!(comparison["monetary_cost"].is_null());
}

fn check(workspace: &Path, output: &Path, rustc: &Path, scenario: &str) -> bool {
    let percent = scenario == "percent_decode";
    let library = output.with_file_name(format!(
        "librecords_{}.rlib",
        output.file_name().unwrap().to_str().unwrap()
    ));
    if percent {
        // Compile both advertised configurations; the model cannot satisfy the
        // runtime oracle by quietly introducing a std/alloc requirement.
        for alloc in [false, true] {
            let mut command = Command::new("timeout");
            command
                .arg("15s")
                .arg(rustc)
                .args([
                    "--edition=2018",
                    "--crate-type=rlib",
                    "--crate-name=records",
                ])
                .arg(workspace.join("src/lib.rs"))
                .arg("-o")
                .arg(&library);
            if alloc {
                command.args(["--cfg", "feature=\"alloc\""]);
            }
            let compiled = command.output().unwrap();
            std::fs::write(
                output.with_extension(if alloc { "alloc.log" } else { "noalloc.log" }),
                &compiled.stderr,
            )
            .unwrap();
            if !compiled.status.success() {
                return false;
            }
        }
    }
    let assertions = if percent {
        PERCENT_ASSERTIONS
    } else if scenario == "overflow" {
        r#"for (price,quantity,fee,expected) in [
            (12,3,4,Some(40)), (u64::MAX,1,0,Some(u64::MAX)),
            (u64::MAX,2,0,None), (u64::MAX,1,1,None),
            (u64::MAX,0,7,Some(7)), (0,u64::MAX,u64::MAX,Some(u64::MAX)),
            (u64::MAX/2,2,1,Some(u64::MAX)), (u64::MAX/2,2,2,None),
        ] { assert_eq!(records::checked_total(price,quantity,fee),expected); }"#
    } else {
        PARSER_ASSERTIONS
    };
    let import = if percent {
        "extern crate records;".to_owned()
    } else {
        format!(
            "#[path = {:?}] mod records;",
            workspace.join("src/records.rs")
        )
    };
    let source = format!("{import}\nfn main() {{\n{assertions}\n}}\n");
    const PARSER_ASSERTIONS: &str = r#"for (input, expected) in [
            (" mode = local ", Some(("mode", "local"))),
            (" = x", None), ("mode", None), ("", None),
            ("mode=a=b", Some(("mode", "a=b"))),
            ("mode=  ", Some(("mode", ""))),
            ("\u{2003}mode\u{2003}=\tlocal\n", Some(("mode", "local"))),
            ("a==", Some(("a", "="))), ("=", None),
        ] { assert_eq!(records::parse_record(input), expected); }
        let owned = String::from(" key = value ");
        let (key, value) = records::parse_record(&owned).unwrap();
        assert_eq!(key.as_ptr(), owned[1..].as_ptr());
        assert_eq!(value.as_ptr(), owned[7..].as_ptr());"#;
    let path = output.with_extension("rs");
    std::fs::write(&path, source).unwrap();
    let mut command = Command::new("timeout");
    command
        .arg("15s")
        .arg(rustc)
        .args(["--edition=2024", "--crate-name", "host_task_oracle"])
        .arg(&path)
        .arg("-o")
        .arg(output);
    if percent {
        command
            .arg("--extern")
            .arg(format!("records={}", library.display()));
    }
    let compiled = command.output().unwrap();
    std::fs::write(output.with_extension("compile.log"), &compiled.stderr).unwrap();
    if !compiled.status.success() {
        return false;
    }
    let ran = Command::new("timeout")
        .arg("5s")
        .arg(output)
        .output()
        .unwrap();
    std::fs::write(output.with_extension("check.log"), &ran.stderr).unwrap();
    ran.status.success()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real OMP 18.6.0 and local llama-server; see docs/validation.md"]
async fn free_form_parser_repair_with_full_files_and_foundry_context() {
    let bun = std::env::var_os("FOUNDRY_OMP_BUN").expect("Bun executable");
    let cli = std::env::var_os("FOUNDRY_OMP_CLI").expect("OMP dist/cli.js");
    let port: u16 = std::env::var("FOUNDRY_HOST_TASK_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let rustc = PathBuf::from(std::env::var_os("FOUNDRY_HOST_RUSTC").expect("Rust 1.90 rustc"));
    let backend =
        std::env::var("FOUNDRY_HOST_TASK_MODEL").unwrap_or_else(|_| "qwen-coder-0.5b".into());
    let (model, model_sha) = match backend.as_str() {
        "qwen-coder-0.5b" => (
            "Qwen2.5-Coder-0.5B-Instruct Q4_K_M CPU",
            "1d9614638d18024d0fbb36575a15f1302a3adf044df10345688ec4f6e1c4ff32",
        ),
        "qwen3-1.7b" => (
            "Qwen3-1.7B Q8_0 CPU, thinking disabled",
            "061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a",
        ),
        "qwen-coder-7b" => (
            "Qwen2.5-Coder-7B-Instruct Q4_K_M CPU",
            "509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c",
        ),
        "qwen3-4b-instruct-2507" => (
            "Qwen3-4B-Instruct-2507 Q4_K_M CPU",
            "3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597",
        ),
        "qwen3-coder-30b-a3b-instruct" => (
            "Qwen3-Coder-30B-A3B-Instruct Q4_K_M CPU",
            "fadc3e5f8d42bf7e894a785b05082e47daee4df26680389817e2093056f088ad",
        ),
        _ => panic!("unsupported explicitly pinned task model"),
    };
    let public_check = std::env::var("FOUNDRY_HOST_TASK_PUBLIC_CHECK").as_deref() == Ok("1");
    let (task_seconds, output_limit) = if public_check {
        (900, 768)
    } else if [
        "qwen-coder-7b",
        "qwen3-4b-instruct-2507",
        "qwen3-coder-30b-a3b-instruct",
    ]
    .contains(&backend.as_str())
    {
        (420, 768)
    } else {
        (180, 512)
    };
    let configured_limit = |name: &str, default: u64, low: u64, high: u64| {
        let value = match std::env::var(name) {
            Ok(value) => value
                .parse::<u64>()
                .expect("host task limit must be an integer"),
            Err(std::env::VarError::NotPresent) => default,
            Err(error) => panic!("invalid {name}: {error}"),
        };
        assert!(
            (low..=high).contains(&value),
            "{name} must be in {low}..={high}"
        );
        value
    };
    let output_limit = configured_limit("FOUNDRY_HOST_TASK_OUTPUT_LIMIT", output_limit, 1, 4096);
    let context_window = configured_limit("FOUNDRY_HOST_TASK_CONTEXT_WINDOW", 16384, 4096, 131072);
    assert!(
        output_limit < context_window,
        "output limit must leave room for input"
    );
    let scenario = std::env::var("FOUNDRY_HOST_TASK_SCENARIO").unwrap_or_else(|_| "parser".into());
    assert!(["parser", "overflow", "percent_decode"].contains(&scenario.as_str()));
    assert!(
        !public_check
            || (["parser", "percent_decode"].contains(&scenario.as_str())
                && [
                    "qwen3-4b-instruct-2507",
                    "qwen-coder-7b",
                    "qwen3-coder-30b-a3b-instruct"
                ]
                .contains(&backend.as_str()))
    );
    let percent = scenario == "percent_decode";
    // Whole upstream source can take over two minutes to prefill on this CPU;
    // share its declared task ceiling instead of the small fixture's idle cap.
    let idle_seconds = if public_check {
        420
    } else if percent {
        task_seconds
    } else {
        120
    };
    let task = if percent {
        PERCENT_TASK
    } else if scenario == "overflow" {
        OVERFLOW_TASK
    } else {
        TASK
    };
    let symbol = if percent {
        "after_percent_sign"
    } else if scenario == "overflow" {
        "checked_total"
    } else {
        "parse_record"
    };
    let scale =
        std::env::var("FOUNDRY_HOST_TASK_SOURCE_SCALE").unwrap_or_else(|_| "original".into());
    assert!(["original", "expanded"].contains(&scale.as_str()));
    assert!(
        !percent || scale == "original",
        "real source is not synthetically padded"
    );
    let fixture = if percent {
        PathBuf::from(
            std::env::var_os("FOUNDRY_HOST_TASK_UPSTREAM")
                .expect("verified percent-encoding 2.3.2 source"),
        )
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(if scenario == "overflow" {
            "tests/fixtures/agent-task-overflow"
        } else {
            "tests/fixtures/agent-task"
        })
    };
    let files: &[&str] = if percent {
        &[
            "Cargo.toml",
            "src/lib.rs",
            "src/ascii_set.rs",
            "LICENSE-MIT",
            "LICENSE-APACHE",
        ]
    } else {
        &["Cargo.toml", "src/records.rs", "src/main.rs"]
    };
    let edited_file = if percent {
        "src/lib.rs"
    } else {
        "src/records.rs"
    };
    if percent {
        // Published crate 2.3.2, archive SHA in Cargo.lock. No network, source
        // generation or local registry mutation is performed by this test.
        for (file, hash) in files.iter().zip([
            "e4ea043c47bbd5551fc5fcf8151eab94428b4be6f2337834a0f9020a4392fb19",
            "641903d385581e76f31ef8c991d83c2a5d7fbbc2879d93909c539037e4e1e3e2",
            "f75552e4f446ed240cef64c1367e848cf3ae65f77602085459566d835a1d8587",
            "b38f11f6096706e6de553dabe2a7ed142d59b6fa8c97e290c67496154745cdd5",
            "a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2",
        ]) {
            assert_eq!(
                context_foundry::digest(&std::fs::read(fixture.join(file)).unwrap()),
                hash,
                "upstream source changed: {file}"
            );
        }
    }
    let output =
        PathBuf::from(std::env::var_os("FOUNDRY_HOST_TASK_DIR").expect("new output directory"));
    assert!(output.is_absolute() && !output.starts_with(env!("CARGO_MANIFEST_DIR")));
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&output)
        .unwrap();
    // Use the host's supported source-body reads instead of its independent
    // declaration-only projection. This fixed overlay is identical for both arms.
    let host_config = "read:\n  summarize:\n    enabled: false\n";
    let host_config_path = output.join("host-config.yml");
    std::fs::write(&host_config_path, host_config).unwrap();
    let host_config_sha = context_foundry::digest(host_config.as_bytes());
    // Preserve the installed OMP engineering workflow. A custom --system-prompt
    // replaces that workflow; an explicit bundled template avoids discovery of
    // unrelated SYSTEM.md overrides while allowing scoped additive constraints.
    let system_template = Path::new(&cli)
        .parent()
        .and_then(Path::parent)
        .expect("OMP dist/cli.js package directory")
        .join("src/prompts/system/system-prompt.md");
    let system_template_bytes = std::fs::read(&system_template).unwrap();
    let system_template_sha = context_foundry::digest(&system_template_bytes);
    assert_eq!(
        system_template_sha, "0c02a5913988f0bd374926ccfa2961a95c8ba00c1b445041a435d5ef86e33736",
        "pinned OMP 18.6.0 bundled system template changed"
    );
    let recorded_system_template = output.join("system-template.md");
    std::fs::write(&recorded_system_template, system_template_bytes).unwrap();
    // Positive control stays outside both model-visible workspaces/prompts.
    let control = output.join("oracle-control");
    std::fs::create_dir_all(control.join("src")).unwrap();
    let correct = if scenario == "overflow" {
        "pub fn checked_total(unit_price: u64, quantity: u64, fee: u64) -> Option<u64> { unit_price.checked_mul(quantity)?.checked_add(fee) }\n"
    } else {
        "pub fn parse_record(input: &str) -> Option<(&str, &str)> { let (key, value) = input.split_once('=')?; let key = key.trim(); (!key.is_empty()).then_some((key, value.trim())) }\n"
    };
    std::fs::write(control.join("src/records.rs"), correct).unwrap();
    if percent {
        for file in ["src/lib.rs", "src/ascii_set.rs"] {
            std::fs::copy(fixture.join(file), control.join(file)).unwrap();
        }
    }
    assert!(
        check(
            &control,
            &output.join("positive-control"),
            &rustc,
            &scenario
        ),
        "checker must accept a correct borrowed repair"
    );
    let mut rows = Vec::new();
    let reset_slot = std::env::var("FOUNDRY_HOST_TASK_RESET_SLOT").as_deref() == Ok("1");
    for arm in ["full_files", "foundry_composed"] {
        let dir = output.join(arm);
        let workspace = dir.join("workspace");
        let agent = dir.join("omp/agent");
        let sessions = dir.join("sessions");
        for path in [workspace.join("src"), agent.clone(), sessions.clone()] {
            std::fs::create_dir_all(path).unwrap();
        }
        // Explicit local llama-server calibration only. With --cache-ram 0,
        // clearing its sole slot prevents cross-arm prefix reuse while allowing
        // ordinary caching within each task. Preserve the upstream receipt.
        let cache_reset = if reset_slot {
            let reset: Value = reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap()
                .post(format!(
                    "http://127.0.0.1:{port}/api/coding/paas/v4/slots/0?action=erase"
                ))
                .json(&json!({}))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();
            assert_eq!(reset["id_slot"], 0);
            assert!(reset["n_erased"].as_u64().is_some());
            std::fs::write(
                dir.join("cache-reset.json"),
                serde_json::to_vec_pretty(&reset).unwrap(),
            )
            .unwrap();
            Some(reset)
        } else {
            None
        };
        for file in files {
            std::fs::copy(fixture.join(file), workspace.join(file)).unwrap();
        }
        if percent {
            let path = workspace.join(edited_file);
            let source = std::fs::read_to_string(&path).unwrap();
            let original = "let mut cloned_iter = iter.clone();\n    let h = char::from(*cloned_iter.next()?).to_digit(16)?;\n    let l = char::from(*cloned_iter.next()?).to_digit(16)?;\n    *iter = cloned_iter;";
            assert_eq!(source.matches(original).count(), 1);
            std::fs::write(path, source.replace(original, "let h = char::from(*iter.next()?).to_digit(16)?;\n    let l = char::from(*iter.next()?).to_digit(16)?;")).unwrap();
        }
        let before_source = std::fs::read(workspace.join(edited_file)).unwrap();
        if scale == "expanded" {
            use std::io::Write as _;
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(workspace.join("src/main.rs"))
                .unwrap();
            // Explicit synthetic unrelated source, identical across arms. This
            // tests the effect of evidence volume, not realistic task coverage.
            for i in 0..64 {
                writeln!(file, "\n#[allow(dead_code)]\nfn display_colour_{i:03}() -> (&'static str, u32) {{ (\"shade {i:03}\", {}) }}", i * 257).unwrap();
            }
        }
        let protected: Vec<_> = files
            .iter()
            .filter(|&&f| f != edited_file)
            .map(|f| (*f, std::fs::read(workspace.join(f)).unwrap()))
            .collect();
        assert!(
            !check(&workspace, &dir.join("before"), &rustc, &scenario),
            "original must fail"
        );
        let started = Instant::now();
        let context = if arm == "full_files" {
            files
                .iter()
                .filter(|f| !f.starts_with("LICENSE-"))
                .map(|f| {
                    format!(
                        "File {f}:\n{}\n",
                        std::fs::read_to_string(workspace.join(f)).unwrap()
                    )
                })
                .collect::<String>()
        } else {
            let mut engine = Engine::initialize(&dir.join("store"), &workspace).unwrap();
            engine.index(&workspace, &Control::unbounded()).unwrap();
            let wid = engine.workspace_id().unwrap();
            let mutate = |command: Value| {
                engine
                    .task_context(
                        &task_context::parse(
                            json!({"workspace_id":wid,"task_id":"repair","command":command}),
                        )
                        .unwrap(),
                        &Control::unbounded(),
                    )
                    .unwrap()
            };
            let revision = mutate(
                json!({"op":"create","title":"Repair source behavior while preserving callers"}),
            )["revision"]
                .as_u64()
                .unwrap();
            let revision = mutate(json!({"op":"apply","expected_revision":revision,"edits":[
                {"op":"add","entry":{"id":"goal","kind":"requirement","text":task,"author":"operator","provenance":"public calibration task","source_links":[]}},
                {"op":"add","entry":{"id":"caller","kind":"question","text":format!("What do callers require from {symbol}?"),"author":"operator","provenance":"public calibration task","source_links":[]}}
            ]}))["revision"].as_u64().unwrap();
            let candidates = engine
                .context_candidates_with(
                    symbol,
                    Strategy::Auto,
                    &Control::unbounded(),
                    &ContextOptions {
                        task: Some(task_context::Scope {
                            workspace_id: &wid,
                            task_id: "repair",
                            expected_revision: revision,
                        }),
                        compose_evidence: true,
                        compact_evidence: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            response::pack_enriched_context(
                &candidates,
                None,
                response::Budget::request(2048),
                &response::stdout_bytes,
            )
            .unwrap()
            .text
        };
        let preparation_ms = started.elapsed().as_millis();
        std::fs::write(dir.join("context.txt"), &context).unwrap();
        let public_command = if public_check {
            // Ordinary visible fixture checks, distinct from the external oracle.
            // Put build output outside the protected workspace in both arms.
            let quote =
                |path: &Path| format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"));
            let script = dir.join("public-check.sh");
            let binary = dir.join("public-check-bin");
            let source = if percent {
                format!(
                    "#!/bin/sh\nset -eu\n{} --edition=2018 --test --cfg 'feature=\"alloc\"' --cfg 'feature=\"std\"' {} -o {}\nexec {}\n",
                    quote(&rustc),
                    quote(&workspace.join("src/lib.rs")),
                    quote(&binary),
                    quote(&binary)
                )
            } else {
                format!(
                    "#!/bin/sh\nset -eu\n{} --edition=2024 {} -o {}\nexec {} check\n",
                    quote(&rustc),
                    quote(&workspace.join("src/main.rs")),
                    quote(&binary),
                    quote(&binary)
                )
            };
            std::fs::write(&script, source).unwrap();
            Some(format!("/bin/sh {}", quote(&script)))
        } else {
            None
        };
        let check_instruction = public_command.as_ref().map(|command| format!("\nUse bash to run the existing public checks with {command}. Inspect their output and correct failures before finishing. Keep compilation output outside the repository.")).unwrap_or_default();
        let prompt =
            format!("{task}{check_instruction}\n\nInitial repository evidence:\n{context}");
        std::fs::write(dir.join("prompt.txt"), &prompt).unwrap();
        let mut gateway = Gateway::start(
            &dir,
            Spawn {
                upstream: Some(port),
                log_bytes: Some(8_388_608),
                idle_ms: Some(idle_seconds * 1000),
                deadline_ms: Some(task_seconds * 1000),
                ..Default::default()
            },
        );
        std::fs::write(
            agent.join("models.yml"),
            format!("{}    modelOverrides:\n      glm-5.3-flash:\n        contextWindow: {context_window}\n        maxTokens: {output_limit}\n        reasoning: false\n", gateway.ready["omp_models_yml"].as_str().unwrap()),
        )
        .unwrap();
        let config_dir = format!(
            "{}{}",
            "../".repeat(32),
            dir.join("omp").strip_prefix("/").unwrap().display()
        );
        let mut command = tokio::process::Command::new(&bun);
        let host_tools = if public_check && percent {
            "read,edit,bash"
        } else if public_check {
            "read,write,bash"
        } else if percent {
            "read,edit"
        } else {
            "read,write"
        };
        let system = if public_check && percent {
            "Complete the requested code repair using read, edit and bash. Preserve user constraints and verify the edit using the public checks."
        } else if public_check {
            "Complete the requested code repair using read, write and bash. Preserve user constraints and verify the edit using the public checks."
        } else if percent {
            "Complete the requested code repair using read and edit. Preserve user constraints."
        } else {
            "Complete the requested code repair using read and write. Preserve user constraints."
        };
        let system_append = format!(
            "{system} Explicit user restrictions on editable files take precedence over general instructions to update tests, documentation, or changelogs. Keep provided public checks unchanged. Keep temporary verification and build outputs outside the workspace."
        );
        std::fs::write(dir.join("system-append.txt"), &system_append).unwrap();
        command
            .arg(&cli)
            .args([
                "--print",
                "--mode",
                "json",
                "--model",
                "zai/glm-5.3-flash",
                "--thinking",
                "off",
                "--tools",
                host_tools,
                "--no-lsp",
                "--no-pty",
                "--no-extensions",
                "--no-skills",
                "--no-rules",
                "--no-title",
                "--config",
                host_config_path.to_str().unwrap(),
                "--system-prompt-template",
                recorded_system_template.to_str().unwrap(),
                "--append-system-prompt",
                &system_append,
                "--max-time",
                &format!("{task_seconds}s"),
                "--approval-mode",
                "yolo",
            ])
            .arg("--session-dir")
            .arg(&sessions)
            .arg(&prompt)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("PI_CONFIG_DIR", config_dir)
            .env("PI_CODING_AGENT_DIR", &agent)
            .env("FOUNDRY_GATEWAY_TOKEN", &gateway.token)
            .current_dir(&workspace)
            .stdin(Stdio::null())
            .kill_on_drop(true);
        if percent {
            // Pin the host's exact-text edit variant in both arms. No
            // model-specific hashline read or fuzzy edit is required.
            command
                .env("PI_EDIT_VARIANT", "replace")
                .env("PI_EDIT_FUZZY", "0");
        }
        let host_argv = serde_json::to_vec_pretty(&json!({
            "program": Path::new(&bun).display().to_string(),
            "args": command.as_std().get_args().map(|arg| arg.to_string_lossy().into_owned()).collect::<Vec<_>>()
        })).unwrap();
        std::fs::write(dir.join("host-argv.json"), &host_argv).unwrap();
        let host_settings_sha =
            context_foundry::digest(&std::fs::read(agent.join("models.yml")).unwrap());
        let model_started = Instant::now();
        // File-backed logs survive outer cancellation and preserve the partial
        // tool transcript; output() would discard it when its future is dropped.
        command.stdout(std::fs::File::create(dir.join("host.stdout.jsonl")).unwrap());
        command.stderr(std::fs::File::create(dir.join("host.stderr.log")).unwrap());
        let ran =
            tokio::time::timeout(Duration::from_secs(task_seconds + 15), command.status()).await;
        let elapsed_ms = model_started.elapsed().as_millis();
        let (host_success, timed_out) = match ran {
            Ok(Ok(status)) => (status.success(), false),
            Ok(Err(error)) => {
                std::fs::write(dir.join("host.error"), error.to_string()).unwrap();
                (false, false)
            }
            Err(_) => (false, true),
        };
        let finished = gateway.terminate();
        finished.assert_no_secrets();
        let (receipts, receipt_log_complete) = std::fs::read(dir.join("run/receipts.jsonl"))
            .map(|bytes| stopped_receipts(&bytes))
            .unwrap_or_else(|_| (Vec::new(), false));
        let gateway_summary = finished.final_line();
        let receipt_accounting_complete =
            receipt_accounting_complete(&receipts, receipt_log_complete, &gateway_summary);
        assert!(
            receipts
                .iter()
                .all(|r| r.output_tokens.is_none_or(|n| n <= output_limit))
        );
        let sum = |field: fn(&Receipt) -> Option<u64>| -> Option<u64> {
            (!receipts.is_empty())
                .then(|| receipts.iter().map(field).sum::<Option<u64>>())
                .flatten()
        };
        let observed = |field: fn(&Receipt) -> Option<u64>| {
            let values: Vec<_> = receipts.iter().filter_map(field).collect();
            json!({"known_tokens":(!values.is_empty()).then(||values.iter().sum::<u64>()),
                "known_requests":values.len(),"unknown_requests":receipts.len()-values.len()})
        };
        let checked = check(&workspace, &dir.join("after"), &rustc, &scenario);
        let mut observed_files = Vec::new();
        fn paths(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    paths(root, &path, out);
                } else {
                    out.push(path.strip_prefix(root).unwrap().to_path_buf());
                }
            }
        }
        paths(&workspace, &workspace, &mut observed_files);
        let preserved = observed_files.len() == files.len()
            && observed_files
                .iter()
                .all(|p| files.iter().any(|f| p == Path::new(f)))
            && protected
                .iter()
                .all(|(file, bytes)| *bytes == std::fs::read(workspace.join(file)).unwrap());
        let changed = before_source != std::fs::read(workspace.join(edited_file)).unwrap();
        let total_ms = started.elapsed().as_millis();
        rows.push(json!({"arm":arm,"host_success":host_success,"timeout":timed_out,"oracle_pass":checked,"protected_files_unchanged":preserved,"source_changed":changed,"verified_task":checked&&preserved,
            "receipt_accounting_complete":receipt_accounting_complete,"receipt_log_complete":receipt_log_complete,
            "public_check_command":public_command,
            "host_config_sha256":host_config_sha,
            "system_template_sha256":system_template_sha,
            "system_append_sha256":context_foundry::digest(system_append.as_bytes()),
            "host_argv_sha256":context_foundry::digest(&host_argv),
            "host_model_settings_sha256":host_settings_sha,
            "cache_reset_before_task":cache_reset,
            "preparation_ms":preparation_ms,"host_elapsed_ms":elapsed_ms,"orchestration_and_check_ms":total_ms-preparation_ms-elapsed_ms,"total_measured_task_ms":total_ms,"requests":receipts.len(),"input_tokens":sum(|r|r.input_tokens),"output_tokens":sum(|r|r.output_tokens),"cached_input_tokens":sum(|r|r.cached_input_tokens),
            "observed_usage":{"input":observed(|r|r.input_tokens),"output":observed(|r|r.output_tokens),"cached_input":observed(|r|r.cached_input_tokens)},
            "complete_requests":receipts.iter().filter(|r|r.outcome==Outcome::Complete).count(),"gateway":gateway_summary}));
        std::fs::write(
            output.join("progress.json"),
            serde_json::to_vec_pretty(&rows).unwrap(),
        )
        .unwrap();
    }
    let report = json!({"version":4,"system_prompt_recipe":"omp_18_6_0_bundled_template_with_scope_append_and_body_reads","scope":"public_single_task_free_form_host_calibration","scenario":scenario,"held_out":false,"host":"OMP 18.6.0","actual_backend":model,"model_sha256":model_sha,"source_scale":scale,"upstream_crate":percent.then_some("percent-encoding 2.3.2 seeded regression, archive 9b4f627cb1b25917193a259e49bdad08f671f8d9708acfd5fe0a8c1455d87220"),"tools":if public_check && percent {"read,edit,bash; replace mode; fuzzy=false"} else if public_check {"read,write,bash"} else if percent {"read,edit; replace mode; fuzzy=false"} else {"read,write"},"public_check_enabled":public_check,"backend_identity":"operator_launched_local_server_not_remote_attestation","transport_alias":"glm-5.3-flash","llama_cpp":"b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea","attempts_per_arm":1,"model_output_limit":output_limit,"model_context_window":context_window,"task_seconds":task_seconds,"request_idle_seconds":idle_seconds,"cost_boundary":"context preparation, gateway/host startup, all requests, teardown and oracle; common fixture setup, model acquisition/loading and energy excluded and separately recorded, never inferred from source tokens","completion_comparison":completion_comparison(&rows),"rows":rows});
    std::fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{report}");
}
