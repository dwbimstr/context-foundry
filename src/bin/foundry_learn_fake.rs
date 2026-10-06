//! 013 T002 fault-test learning worker: the shared learning serve loop
//! with a deterministic two-number fake model and named fault hooks. Built
//! only with the non-default `test-faults` feature (mirroring
//! `foundry-embed-fake`); it links no LibTorch. Its head is a real-shaped
//! `head.safetensors` (the exact trainable set, float32, zeros except the
//! two fake parameters), so the core's candidate validation runs unchanged.
//!
//! `--shim` serves the owner-death tests exactly as 009's fake does.
//!
//! 013 T003 serving hooks, keyed by the 1-based prediction count:
//! `--predict-delays-ms A,B,…` sleeps before each prediction's logits,
//! `--predict-die-at N` aborts during prediction N, and
//! `--predict-tamper N:KIND,…` rewrites (or duplicates, or withholds)
//! prediction N's reply header (see [`fake::tamper`]).
#[cfg(target_os = "macos")]
fn main() {
    use context_foundry::learning::worker::{self, WorkerArgs};
    use context_foundry::neural::worker_runtime;

    let argv: Vec<String> = std::env::args().collect();
    if argv.iter().any(|arg| arg == "--shim") {
        worker_runtime::run_shim(argv[1..].to_vec());
    }
    let (mut args, rest) = match WorkerArgs::parse(argv.into_iter().skip(1)) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("foundry-learn-fake: {message}");
            std::process::exit(64);
        }
    };
    let hooks = match fake::Hooks::parse(&rest) {
        Ok(hooks) => hooks,
        Err(message) => {
            eprintln!("foundry-learn-fake: {message}");
            std::process::exit(64);
        }
    };
    if hooks.ignore_term {
        unsafe {
            libc::signal(libc::SIGTERM, libc::SIG_IGN);
        }
    }
    if let Some(path) = &hooks.pid_file
        && let Err(e) = std::fs::write(path, std::process::id().to_string())
    {
        eprintln!("cannot write the PID file {path}: {e}");
        std::process::exit(1);
    }
    args.faults = hooks.reply.clone();
    if !hooks.predict_tamper.is_empty() {
        let _ = fake::TAMPER.set(hooks.predict_tamper.clone());
        args.faults.serve_reply = Some(fake::tamper);
    }
    let mut backend = fake::Fake::new(hooks);
    std::process::exit(worker::serve(args, &mut backend));
}

#[cfg(target_os = "macos")]
mod fake {
    use context_foundry::decision_model::{self, safetensors};
    use context_foundry::learning::ipc::ParameterCounts;
    use context_foundry::learning::worker::{Backend, LoadReport, Refusal, ReplyFaults};
    use std::path::Path;

    /// The two fake parameters live in these tensors' first element.
    const PARAM_A: &str = "scorer.3.bias";
    const PARAM_B: &str = "scorer.0.bias";

    #[derive(Clone, Debug, Default)]
    pub struct Hooks {
        pub reply: ReplyFaults,
        pub pid_file: Option<String>,
        pub ignore_term: bool,
        pub crash_at_step: Option<u64>,
        pub hang_at_step: Option<u64>,
        pub slow_step_ms: u64,
        pub load_ms: u64,
        pub nonfinite_loss_at: Option<u64>,
        pub nonfinite_grad_at: Option<u64>,
        pub nonfinite_logits_at: Option<u64>,
        pub head_missing_tensor: bool,
        pub head_bad_shape: bool,
        pub head_nonfinite: bool,
        pub head_tamper: bool,
        pub save_extra_mb: u64,
        pub save_bloat_mb: u64,
        pub frozen_drift: bool,
        pub env_report: Option<String>,
        pub predict_delays_ms: Vec<u64>,
        pub predict_die_at: Option<u64>,
        pub predict_tamper: Vec<(u64, String)>,
    }

    impl Hooks {
        pub fn parse(rest: &[String]) -> Result<Self, String> {
            let mut hooks = Self::default();
            let mut i = 0;
            let number = |i: usize, name: &str| -> Result<u64, String> {
                rest.get(i + 1)
                    .ok_or(format!("{name} needs a value"))?
                    .parse::<u64>()
                    .map_err(|e| format!("{name}: {e}"))
            };
            let text = |i: usize, name: &str| -> Result<String, String> {
                rest.get(i + 1)
                    .cloned()
                    .ok_or(format!("{name} needs a value"))
            };
            while i < rest.len() {
                let name = rest[i].as_str();
                let mut takes_value = true;
                match name {
                    "--pid-file" => hooks.pid_file = Some(text(i, name)?),
                    "--request-log" => hooks.reply.request_log = Some(text(i, name)?.into()),
                    "--env-report" => hooks.env_report = Some(text(i, name)?),
                    "--crash-at-step" => hooks.crash_at_step = Some(number(i, name)?),
                    "--hang-at-step" => hooks.hang_at_step = Some(number(i, name)?),
                    "--slow-step-ms" => hooks.slow_step_ms = number(i, name)?,
                    "--load-ms" => hooks.load_ms = number(i, name)?,
                    "--nonfinite-loss-at" => hooks.nonfinite_loss_at = Some(number(i, name)?),
                    "--nonfinite-grad-at" => hooks.nonfinite_grad_at = Some(number(i, name)?),
                    "--nonfinite-logits-at" => hooks.nonfinite_logits_at = Some(number(i, name)?),
                    "--duplicate-logits" => hooks.reply.duplicate_logits = Some(number(i, name)?),
                    "--stale-logits" => hooks.reply.stale_logits = Some(number(i, name)?),
                    "--save-extra-mb" => hooks.save_extra_mb = number(i, name)?,
                    "--save-bloat-mb" => hooks.save_bloat_mb = number(i, name)?,
                    "--predict-die-at" => hooks.predict_die_at = Some(number(i, name)?),
                    "--predict-delays-ms" => {
                        hooks.predict_delays_ms = text(i, name)?
                            .split(',')
                            .map(|ms| ms.parse::<u64>().map_err(|e| format!("{name}: {e}")))
                            .collect::<Result<_, _>>()?;
                    }
                    "--predict-tamper" => {
                        for entry in text(i, name)?.split(',') {
                            let (n, kind) = entry
                                .split_once(':')
                                .ok_or(format!("{name} entries are N:KIND"))?;
                            let n = n.parse::<u64>().map_err(|e| format!("{name}: {e}"))?;
                            hooks.predict_tamper.push((n, kind.to_owned()));
                        }
                    }
                    _ => {
                        takes_value = false;
                        match name {
                            "--ignore-term" => hooks.ignore_term = true,
                            "--drift-after-save" => hooks.reply.drift_after_save = true,
                            "--duplicate-saved" => hooks.reply.duplicate_saved = true,
                            "--head-missing-tensor" => hooks.head_missing_tensor = true,
                            "--head-bad-shape" => hooks.head_bad_shape = true,
                            "--head-nonfinite" => hooks.head_nonfinite = true,
                            "--head-tamper" => hooks.head_tamper = true,
                            "--frozen-drift" => hooks.frozen_drift = true,
                            other => return Err(format!("unknown flag {other}")),
                        }
                    }
                }
                i += if takes_value { 2 } else { 1 };
            }
            Ok(hooks)
        }
    }

    pub struct Fake {
        hooks: Hooks,
        a: f32,
        b: f32,
        steps: u64,
        logits_calls: u64,
    }

    fn refusal(code: &'static str, message: impl Into<String>) -> Refusal {
        Refusal::new(code, message)
    }

    /// Deterministic per-input features in [-2, 2].
    fn features(ids: &[u32], markers: [usize; 2]) -> [f64; 2] {
        let sum: u64 = ids.iter().map(|id| u64::from(*id)).sum();
        let f = |mul: u64, at: usize, modulus: u64| {
            ((sum.wrapping_mul(mul) + at as u64) % modulus) as f64 / modulus as f64 * 4.0 - 2.0
        };
        [f(31, markers[0], 997), f(17, markers[1], 991)]
    }

    fn softmax(z: [f64; 2]) -> [f64; 2] {
        let m = z[0].max(z[1]);
        let e = [(z[0] - m).exp(), (z[1] - m).exp()];
        [e[0] / (e[0] + e[1]), e[1] / (e[0] + e[1])]
    }

    /// The real-shaped head: the exact trainable set, float32, zeros except
    /// the two parameters.
    fn head(a: f32, b: f32, hooks: &Hooks) -> Vec<u8> {
        let mut table = decision_model::trainable();
        if hooks.head_missing_tensor {
            table.retain(|(name, _)| name != "head.layers.1.norm2.bias");
        }
        if hooks.head_bad_shape {
            for (name, shape) in &mut table {
                if name == "scorer.1.bias" {
                    *shape = vec![1023];
                }
            }
        }
        let (mut out, ranges) = safetensors::encode_f32_header(&table);
        let data_start = out.len();
        let total: u64 = ranges.iter().map(|(_, end)| *end).max().unwrap_or(0);
        out.resize(data_start + total as usize, 0);
        for ((name, _), (start, _)) in table.iter().zip(&ranges) {
            let at = data_start + *start as usize;
            let value = match name.as_str() {
                PARAM_A => Some(a),
                PARAM_B => Some(b),
                "scorer.1.weight" if hooks.head_nonfinite => Some(f32::NAN),
                _ => None,
            };
            if let Some(value) = value {
                out[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        out
    }

    /// Read the two parameters back from a head file's bytes, validating
    /// the exact trainable set as the real worker does.
    fn params_of(bytes: &[u8]) -> Result<(f32, f32), Refusal> {
        let invalid = |m: String| refusal("artifact_invalid", m);
        if bytes.len() < 8 {
            return Err(invalid("short head".into()));
        }
        let len = safetensors::header_len(bytes[..8].try_into().unwrap(), "artifact_invalid")?;
        let start = 8 + len as usize;
        let entries = safetensors::parse_header(
            &bytes[8..start],
            (bytes.len() - start) as u64,
            "artifact_invalid",
        )?;
        safetensors::check_tensors(
            &entries,
            &decision_model::trainable(),
            safetensors::Dtype::F32,
            "artifact_invalid",
        )?;
        let read = |name: &str| -> Result<f32, Refusal> {
            let entry = entries
                .iter()
                .find(|e| e.name == name)
                .ok_or_else(|| invalid(format!("{name} missing")))?;
            let at = start + entry.start as usize;
            let value = f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            if !value.is_finite() {
                return Err(refusal("nonfinite_weight", format!("{name} is nonfinite")));
            }
            Ok(value)
        };
        Ok((read(PARAM_A)?, read(PARAM_B)?))
    }

    fn sha_of(path: &Path) -> Result<String, Refusal> {
        std::fs::read(path)
            .map(|bytes| context_foundry::digest(&bytes))
            .map_err(|e| refusal("checkpoint_invalid", format!("{}: {e}", path.display())))
    }

    impl Fake {
        pub fn new(hooks: Hooks) -> Self {
            Self {
                hooks,
                a: 0.0,
                b: 0.0,
                steps: 0,
                logits_calls: 0,
            }
        }

        fn report_environment(&self) {
            let Some(path) = &self.hooks.env_report else {
                return;
            };
            let mut names: Vec<String> = std::env::vars().map(|(k, _)| k).collect();
            names.sort();
            let fds: Vec<i32> = (3..=256)
                .filter(|fd| unsafe { libc::fcntl(*fd, libc::F_GETFD) } != -1)
                .collect();
            let spawn = std::process::Command::new("/usr/bin/true")
                .status()
                .map(|_| 0)
                .unwrap_or_else(|e| e.raw_os_error().unwrap_or(-1));
            let mut limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            let mut limits = Vec::new();
            for resource in [libc::RLIMIT_NPROC, libc::RLIMIT_CPU, libc::RLIMIT_FSIZE] {
                unsafe { libc::getrlimit(resource, &mut limit) };
                limits.push(limit.rlim_max);
            }
            let report = serde_json::json!({
                "env": names,
                "fds": fds,
                "spawn_errno": spawn,
                "limits": {"nproc": limits[0], "cpu": limits[1], "fsize": limits[2]},
                "cwd": std::env::current_dir().ok(),
            });
            let _ = std::fs::write(path, report.to_string());
        }
    }

    impl Backend for Fake {
        fn load(
            &mut self,
            dir: &Path,
            head: Option<&[u8]>,
            _threads: u32,
            _seed: u64,
        ) -> Result<LoadReport, Refusal> {
            self.report_environment();
            if self.hooks.load_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(self.hooks.load_ms));
            }
            let weights_sha256 = sha_of(&dir.join("model.safetensors"))?;
            let encoder_config_sha256 = sha_of(&dir.join("encoder").join("config.json"))?;
            (self.a, self.b) = match head {
                Some(bytes) => params_of(bytes)?,
                None => (0.0, 0.0),
            };
            self.steps = 0;
            let trainable = decision_model::trainable();
            let elements = |set: &[(String, Vec<usize>)]| -> u64 {
                set.iter()
                    .map(|(_, s)| s.iter().product::<usize>() as u64)
                    .sum()
            };
            let checkpoint = decision_model::checkpoint_tensors();
            let encoder: Vec<(String, Vec<usize>)> = checkpoint
                .iter()
                .filter(|(n, _)| n.starts_with("encoder."))
                .cloned()
                .collect();
            Ok(LoadReport {
                source_dtype: "F16".into(),
                weights_sha256,
                encoder_config_sha256,
                counts: ParameterCounts {
                    encoder: elements(&encoder),
                    trainable: elements(&trainable),
                    frozen_other: elements(&checkpoint) - elements(&encoder) - elements(&trainable)
                        + 1024,
                },
                trainable: trainable.into_iter().map(|(name, _)| name).collect(),
                frozen_encoder_sha256: self.frozen_hash()?,
            })
        }

        fn step(
            &mut self,
            ids: &[u32],
            markers: [usize; 2],
            target: usize,
        ) -> Result<(f64, f64), Refusal> {
            let step = self.steps + 1;
            if self.hooks.crash_at_step == Some(step) {
                std::process::abort();
            }
            if self.hooks.hang_at_step == Some(step) {
                loop {
                    std::thread::park();
                }
            }
            if self.hooks.slow_step_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(self.hooks.slow_step_ms));
            }
            let f = features(ids, markers);
            let p = softmax([f[0] + f64::from(self.a), f[1] + f64::from(self.b)]);
            let loss = -p[target].ln();
            let grad = [
                p[0] - f64::from(u8::from(target == 0)),
                p[1] - f64::from(u8::from(target == 1)),
            ];
            let norm = 4.0 * (grad[0] * grad[0] + grad[1] * grad[1]).sqrt();
            let scale = if norm > 1.0 { 1.0 / norm } else { 1.0 };
            self.a -= (0.05 * grad[0] * scale) as f32;
            self.b -= (0.05 * grad[1] * scale) as f32;
            self.steps = step;
            if self.hooks.nonfinite_loss_at == Some(step) {
                return Ok((f64::NAN, norm));
            }
            if self.hooks.nonfinite_grad_at == Some(step) {
                return Ok((loss, f64::INFINITY));
            }
            Ok((loss, norm))
        }

        fn logits(&mut self, ids: &[u32], markers: [usize; 2]) -> Result<[f32; 2], Refusal> {
            self.logits_calls += 1;
            if self.hooks.predict_die_at == Some(self.logits_calls) {
                std::process::abort();
            }
            if let Some(ms) = self
                .hooks
                .predict_delays_ms
                .get(self.logits_calls as usize - 1)
                .filter(|ms| **ms > 0)
            {
                std::thread::sleep(std::time::Duration::from_millis(*ms));
            }
            if self.hooks.nonfinite_logits_at == Some(self.logits_calls) {
                return Ok([f32::NAN, 0.0]);
            }
            let f = features(ids, markers);
            Ok([
                (f[0] + f64::from(self.a)) as f32,
                (f[1] + f64::from(self.b)) as f32,
            ])
        }

        fn head_bytes(&mut self) -> Result<Vec<u8>, Refusal> {
            let mut bytes = head(self.a, self.b, &self.hooks);
            if self.hooks.save_bloat_mb > 0 {
                bytes.resize(bytes.len() + (self.hooks.save_bloat_mb << 20) as usize, 0);
            }
            if self.hooks.save_extra_mb > 0 {
                // Many 10 MiB files: each under the per-file limit, their
                // total over the supervised scratch total.
                let chunk = vec![1u8; 10 << 20];
                for i in 0..self.hooks.save_extra_mb.div_ceil(10) {
                    std::fs::write(format!("extra-{i}.bin"), &chunk)
                        .map_err(|e| refusal("output_write", e.to_string()))?;
                }
                // Stay alive across several monitor polls.
                std::thread::sleep(std::time::Duration::from_millis(1500));
            }
            Ok(bytes)
        }

        fn reload(
            &mut self,
            bytes: &[u8],
            ids: &[u32],
            markers: [usize; 2],
        ) -> Result<(f64, [f32; 2]), Refusal> {
            let before = self.logits(ids, markers)?;
            self.logits_calls -= 1;
            let (a, b) = if self.hooks.head_missing_tensor
                || self.hooks.head_bad_shape
                || self.hooks.head_nonfinite
            {
                // The malformed head is the fault under test: report it as
                // reloaded so the core's own validation must catch it.
                (self.a, self.b)
            } else {
                params_of(bytes)?
            };
            let diff = f64::from((a - self.a).abs().max((b - self.b).abs()));
            (self.a, self.b) = (a, b);
            let after = self.logits(ids, markers)?;
            self.logits_calls -= 1;
            if before != after {
                return Ok((f64::from((before[0] - after[0]).abs()), after));
            }
            if self.hooks.head_tamper {
                // Change the file on disk after its digest was taken.
                let mut tampered = bytes.to_vec();
                if let Some(last) = tampered.last_mut() {
                    *last ^= 0x01;
                }
                let _ = std::fs::write("head.safetensors", tampered);
            }
            Ok((diff, after))
        }

        fn frozen_hash(&mut self) -> Result<String, Refusal> {
            let drift = if self.hooks.frozen_drift && self.steps > 0 {
                "drifted"
            } else {
                "frozen"
            };
            Ok(context_foundry::digest(
                format!("fake encoder {drift}").as_bytes(),
            ))
        }
    }

    /// The configured reply rewrites (`--predict-tamper`).
    pub static TAMPER: std::sync::OnceLock<Vec<(u64, String)>> = std::sync::OnceLock::new();

    /// Rewrite prediction `count`'s reply header. Kinds: `legacy_confidence`
    /// and `extra_field` add a field; `missing_graph` and `extra_option`
    /// break the probability keys; `null_choice`; `sum`, `range`,
    /// `minority_choice`, `tie_search` and `confidence` break the
    /// distribution or its consistency; `wrong_candidate`, `wrong_model`,
    /// `wrong_input` and `wrong_id` break the identity; `duplicate_key`
    /// repeats `choice` in the raw JSON; `twice` sends the reply twice;
    /// `silent` sends nothing; `probs=S/G[/C]` sets a CONSISTENT vector
    /// (choice by the maximum/tie rule, confidence the maximum or `C`).
    pub fn tamper(count: u64, header: Vec<u8>) -> Vec<Vec<u8>> {
        use serde_json::{Value, json};
        let Some(kind) = TAMPER
            .get()
            .and_then(|list| list.iter().find(|(n, _)| *n == count))
            .map(|(_, kind)| kind.clone())
        else {
            return vec![header];
        };
        let mut value: Value = serde_json::from_slice(&header).expect("the worker's own reply");
        let digest = |c: char| json!(c.to_string().repeat(64));
        match kind.as_str() {
            "legacy_confidence" => value["confidence"] = json!(0.99),
            "extra_field" => value["act"] = json!(1),
            "missing_graph" => {
                value["probabilities"]
                    .as_object_mut()
                    .expect("probabilities")
                    .remove("graph");
            }
            "extra_option" => value["probabilities"]["delete_workspace"] = json!(0.0),
            "null_choice" => value["choice"] = Value::Null,
            "sum" => {
                value["probabilities"] = json!({"search": 0.7, "graph": 0.2});
                value["choice"] = json!("search");
                value["answer_confidence"] = json!(0.7);
            }
            "range" => {
                value["probabilities"] = json!({"search": 1.2, "graph": -0.2});
                value["choice"] = json!("search");
                value["answer_confidence"] = json!(1.2);
            }
            "minority_choice" => {
                let search = value["probabilities"]["search"].as_f64().unwrap_or(0.0);
                let graph = value["probabilities"]["graph"].as_f64().unwrap_or(0.0);
                value["choice"] = json!(if search > graph { "graph" } else { "search" });
                value["answer_confidence"] = json!(search.min(graph));
            }
            "tie_search" => {
                value["probabilities"] = json!({"search": 0.5, "graph": 0.5});
                value["choice"] = json!("search");
                value["answer_confidence"] = json!(0.5);
            }
            "confidence" => {
                let confidence = value["answer_confidence"].as_f64().unwrap_or(0.0);
                value["answer_confidence"] = json!(confidence - 0.01);
            }
            "wrong_candidate" => value["candidate_sha256"] = digest('0'),
            "wrong_model" => value["model_function_sha256"] = digest('1'),
            "wrong_input" => value["input_sha256"] = digest('2'),
            "wrong_id" => {
                let id = value["request_id"].as_u64().unwrap_or(0);
                value["request_id"] = json!(id + 1);
            }
            "twice" => return vec![header.clone(), header],
            "silent" => return Vec::new(),
            "duplicate_key" => {
                let text = String::from_utf8(header).expect("JSON is UTF-8");
                let choice = value["choice"].to_string();
                let doubled = text.replacen(
                    &format!("\"choice\":{choice}"),
                    &format!("\"choice\":{choice},\"choice\":{choice}"),
                    1,
                );
                return vec![doubled.into_bytes()];
            }
            other => {
                let numbers: Vec<f64> = other
                    .strip_prefix("probs=")
                    .unwrap_or_else(|| panic!("unknown tamper kind {other}"))
                    .split('/')
                    .map(|n| n.parse().expect("a probability"))
                    .collect();
                let (search, graph) = (numbers[0], numbers[1]);
                value["probabilities"] = json!({"search": search, "graph": graph});
                value["choice"] = json!(if search > graph { "search" } else { "graph" });
                value["answer_confidence"] =
                    json!(numbers.get(2).copied().unwrap_or(search.max(graph)));
            }
        }
        vec![serde_json::to_vec(&value).expect("JSON")]
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("foundry-learn-fake: the learning supervisor targets macOS");
    std::process::exit(78);
}
