//! Opt-in free-form host calibration. No patch candidates or scripted replies.
use super::*;
use context_foundry::{Control, Engine, Strategy, response, store::ContextOptions, task_context};

const TASK: &str = "Edit only src/records.rs so parse_record trims Unicode whitespace on both sides of the first = and rejects an empty key. Preserve the borrowed Option<(&str, &str)> interface, allow empty values, preserve additional = characters in the value, and return None without =. Keep all other files unchanged. Use the write tool to save the repair; a separate checker will verify it. Do not just describe a patch.";
const OVERFLOW_TASK: &str = "Edit only src/records.rs so checked_total(unit_price: u64, quantity: u64, fee: u64) -> Option<u64> returns None if either multiplication or addition overflows. Never panic or wrap. Preserve exact representable totals, including zero quantities and u64::MAX. Keep the public interface and all other files unchanged. Use the write tool to save the repair; a separate checker will verify it. Do not just describe a patch.";

fn check(workspace: &Path, output: &Path, rustc: &Path, scenario: &str) -> bool {
    let assertions = if scenario == "overflow" {
        r#"for (price,quantity,fee,expected) in [
            (12,3,4,Some(40)), (u64::MAX,1,0,Some(u64::MAX)),
            (u64::MAX,2,0,None), (u64::MAX,1,1,None),
            (u64::MAX,0,7,Some(7)), (0,u64::MAX,u64::MAX,Some(u64::MAX)),
            (u64::MAX/2,2,1,Some(u64::MAX)), (u64::MAX/2,2,2,None),
        ] { assert_eq!(records::checked_total(price,quantity,fee),expected); }"#
    } else {
        PARSER_ASSERTIONS
    };
    let source = format!(
        "#[path = {:?}] mod records;\nfn main() {{\n{}\n}}\n",
        workspace.join("src/records.rs"),
        assertions
    );
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
    let compiled = Command::new("timeout")
        .arg("15s")
        .arg(rustc)
        .args(["--edition=2024", "--crate-name", "host_task_oracle"])
        .arg(&path)
        .arg("-o")
        .arg(output)
        .output()
        .unwrap();
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
        _ => panic!("unsupported explicitly pinned task model"),
    };
    let (task_seconds, output_limit) = if backend == "qwen-coder-7b" {
        (420, 768)
    } else {
        (180, 512)
    };
    let scenario = std::env::var("FOUNDRY_HOST_TASK_SCENARIO").unwrap_or_else(|_| "parser".into());
    assert!(["parser", "overflow"].contains(&scenario.as_str()));
    let task = if scenario == "overflow" {
        OVERFLOW_TASK
    } else {
        TASK
    };
    let symbol = if scenario == "overflow" {
        "checked_total"
    } else {
        "parse_record"
    };
    let scale =
        std::env::var("FOUNDRY_HOST_TASK_SOURCE_SCALE").unwrap_or_else(|_| "original".into());
    assert!(["original", "expanded"].contains(&scale.as_str()));
    let output =
        PathBuf::from(std::env::var_os("FOUNDRY_HOST_TASK_DIR").expect("new output directory"));
    assert!(output.is_absolute() && !output.starts_with(env!("CARGO_MANIFEST_DIR")));
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&output)
        .unwrap();
    // Positive control stays outside both model-visible workspaces/prompts.
    let control = output.join("oracle-control");
    std::fs::create_dir_all(control.join("src")).unwrap();
    let correct = if scenario == "overflow" {
        "pub fn checked_total(unit_price: u64, quantity: u64, fee: u64) -> Option<u64> { unit_price.checked_mul(quantity)?.checked_add(fee) }\n"
    } else {
        "pub fn parse_record(input: &str) -> Option<(&str, &str)> { let (key, value) = input.split_once('=')?; let key = key.trim(); (!key.is_empty()).then_some((key, value.trim())) }\n"
    };
    std::fs::write(control.join("src/records.rs"), correct).unwrap();
    assert!(
        check(
            &control,
            &output.join("positive-control"),
            &rustc,
            &scenario
        ),
        "checker must accept a correct borrowed repair"
    );
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join(if scenario == "overflow" {
        "tests/fixtures/agent-task-overflow"
    } else {
        "tests/fixtures/agent-task"
    });
    let files = ["Cargo.toml", "src/records.rs", "src/main.rs"];
    let mut rows = Vec::new();
    for arm in ["full_files", "foundry_composed"] {
        let dir = output.join(arm);
        let workspace = dir.join("workspace");
        let agent = dir.join("omp/agent");
        let sessions = dir.join("sessions");
        for path in [workspace.join("src"), agent.clone(), sessions.clone()] {
            std::fs::create_dir_all(path).unwrap();
        }
        for file in files {
            std::fs::copy(fixture.join(file), workspace.join(file)).unwrap();
        }
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
        let protected: Vec<_> = ["Cargo.toml", "src/main.rs"]
            .iter()
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
        let prompt = format!("{task}\n\nInitial repository evidence:\n{context}");
        std::fs::write(dir.join("prompt.txt"), &prompt).unwrap();
        let mut gateway = Gateway::start(
            &dir,
            Spawn {
                upstream: Some(port),
                log_bytes: Some(8_388_608),
                idle_ms: Some(120_000),
                deadline_ms: Some(task_seconds * 1000),
                ..Default::default()
            },
        );
        std::fs::write(
            agent.join("models.yml"),
            format!("{}    modelOverrides:\n      glm-5.3-flash:\n        contextWindow: 16384\n        maxTokens: {output_limit}\n        reasoning: false\n", gateway.ready["omp_models_yml"].as_str().unwrap()),
        )
        .unwrap();
        let config_dir = format!(
            "{}{}",
            "../".repeat(32),
            dir.join("omp").strip_prefix("/").unwrap().display()
        );
        let mut command = tokio::process::Command::new(&bun);
        command.arg(&cli).args([
            "--print","--mode","json","--model","zai/glm-5.3-flash","--thinking","off",
            "--tools","read,write","--no-lsp","--no-pty","--no-extensions","--no-skills",
            "--no-rules","--no-title","--system-prompt","Complete the requested code repair using read and write. Preserve user constraints.",
            "--max-time",&format!("{task_seconds}s"),"--approval-mode","yolo",
        ]).arg("--session-dir").arg(&sessions).arg(&prompt)
            .env_clear().env("PATH","/usr/bin:/bin").env("PI_CONFIG_DIR",config_dir)
            .env("PI_CODING_AGENT_DIR",&agent).env("FOUNDRY_GATEWAY_TOKEN",&gateway.token)
            .current_dir(&workspace).stdin(Stdio::null()).kill_on_drop(true);
        let model_started = Instant::now();
        let ran =
            tokio::time::timeout(Duration::from_secs(task_seconds + 15), command.output()).await;
        let elapsed_ms = model_started.elapsed().as_millis();
        let (host_success, timed_out) = match ran {
            Ok(Ok(result)) => {
                std::fs::write(dir.join("host.stdout.jsonl"), &result.stdout).unwrap();
                std::fs::write(dir.join("host.stderr.log"), &result.stderr).unwrap();
                (result.status.success(), false)
            }
            Ok(Err(error)) => {
                std::fs::write(dir.join("host.error"), error.to_string()).unwrap();
                (false, false)
            }
            Err(_) => (false, true),
        };
        let finished = gateway.terminate();
        finished.assert_no_secrets();
        let receipts = gateway.receipts();
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
        let changed = std::fs::read(fixture.join("src/records.rs")).unwrap()
            != std::fs::read(workspace.join("src/records.rs")).unwrap();
        rows.push(json!({"arm":arm,"host_success":host_success,"timeout":timed_out,"oracle_pass":checked,"protected_files_unchanged":preserved,"source_changed":changed,"verified_task":checked&&preserved,
            "preparation_ms":preparation_ms,"host_elapsed_ms":elapsed_ms,"total_measured_task_ms":preparation_ms+elapsed_ms,"requests":receipts.len(),"input_tokens":sum(|r|r.input_tokens),"output_tokens":sum(|r|r.output_tokens),"cached_input_tokens":sum(|r|r.cached_input_tokens),
            "complete_requests":receipts.iter().filter(|r|r.outcome==Outcome::Complete).count(),"gateway":finished.final_line()}));
        std::fs::write(
            output.join("progress.json"),
            serde_json::to_vec_pretty(&rows).unwrap(),
        )
        .unwrap();
    }
    let report = json!({"scope":"public_single_task_free_form_host_calibration","scenario":scenario,"held_out":false,"host":"OMP 18.6.0","actual_backend":model,"model_sha256":model_sha,"source_scale":scale,"backend_identity":"operator_launched_local_server_not_remote_attestation","transport_alias":"glm-5.3-flash","llama_cpp":"b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea","attempts_per_arm":1,"model_output_limit":output_limit,"task_seconds":task_seconds,"cost_boundary":"context preparation plus all gateway requests; common model acquisition/loading and energy accounted separately, never inferred from source tokens","rows":rows});
    std::fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{report}");
}
