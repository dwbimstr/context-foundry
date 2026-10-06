//! 013 T002 learning worker (`learning-worker` feature): the shared learning
//! serve loop over the LibTorch model ([`context_foundry::decision_model::net`]).
//! A numerical engine only: it loads the granted checkpoint, runs the
//! updates and logits the owner asks for, writes `head.safetensors` into its
//! scratch run directory and reloads it. It never reads the store, the
//! dataset files, the network or anything outside its grants.
//!
//! Before LibTorch is touched, `--probe <name>` runs one named
//! development-isolation check inside the sandboxed bundle and prints a JSON
//! verdict (the probes shared with 009's worker); those exist for the
//! profile's negative tests with positive controls.
#[cfg(target_os = "macos")]
fn main() {
    use context_foundry::learning::worker::{self, WorkerArgs};

    let argv: Vec<String> = std::env::args().collect();
    if let Some(index) = argv.iter().position(|arg| arg == "--probe") {
        let name = argv.get(index + 1).cloned().unwrap_or_default();
        let rest = argv[index + 2..].to_vec();
        context_foundry::neural::probes::run(&name, &rest);
        return;
    }
    let (args, rest) = match WorkerArgs::parse(argv.into_iter().skip(1)) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("foundry-learn: {message}");
            std::process::exit(64);
        }
    };
    if !rest.is_empty() {
        eprintln!("foundry-learn: unknown arguments {rest:?}");
        std::process::exit(64);
    }
    let mut backend = real::Torch::default();
    std::process::exit(worker::serve(args, &mut backend));
}

#[cfg(target_os = "macos")]
mod real {
    use context_foundry::decision_model::net::Model;
    use context_foundry::decision_model::trainable;
    use context_foundry::learning::ipc::ParameterCounts;
    use context_foundry::learning::worker::{Backend, LoadReport, Refusal};
    use std::path::Path;

    #[derive(Default)]
    pub struct Torch {
        model: Option<Model>,
        interop_set: bool,
    }

    impl Torch {
        fn model(&mut self) -> Result<&mut Model, Refusal> {
            self.model
                .as_mut()
                .ok_or_else(|| Refusal::new("frame_invalid", "no checkpoint is loaded"))
        }
    }

    impl Backend for Torch {
        fn load(
            &mut self,
            dir: &Path,
            head: Option<&[u8]>,
            threads: u32,
            seed: u64,
        ) -> Result<LoadReport, Refusal> {
            // The inter-op pool can be sized once per process, before any
            // parallel work.
            if !self.interop_set {
                tch::set_num_interop_threads(1);
                self.interop_set = true;
            }
            tch::set_num_threads(threads as i32);
            tch::manual_seed(seed as i64);
            self.model = None;
            let mut model = Model::load(dir)?;
            if let Some(bytes) = head {
                model.load_head(bytes)?;
            }
            let report = LoadReport {
                source_dtype: model.source_dtype.as_str().to_owned(),
                weights_sha256: model.weights_sha256.clone(),
                encoder_config_sha256: model.encoder_config_sha256.clone(),
                counts: ParameterCounts {
                    encoder: model.encoder_parameters as u64,
                    trainable: model.trainable_parameters as u64,
                    frozen_other: model.frozen_other_parameters as u64,
                },
                trainable: trainable().into_iter().map(|(name, _)| name).collect(),
                frozen_encoder_sha256: model.frozen_encoder_sha256(),
            };
            self.model = Some(model);
            Ok(report)
        }

        fn step(
            &mut self,
            ids: &[u32],
            markers: [usize; 2],
            target: usize,
        ) -> Result<(f64, f64), Refusal> {
            Ok(self.model()?.train_step(ids, markers, target)?)
        }

        fn logits(&mut self, ids: &[u32], markers: [usize; 2]) -> Result<[f32; 2], Refusal> {
            Ok(self.model()?.logits(ids, markers)?)
        }

        fn head_bytes(&mut self) -> Result<Vec<u8>, Refusal> {
            Ok(self.model()?.head_bytes())
        }

        fn reload(
            &mut self,
            bytes: &[u8],
            ids: &[u32],
            markers: [usize; 2],
        ) -> Result<(f64, [f32; 2]), Refusal> {
            let model = self.model()?;
            let before = model.logits(ids, markers)?;
            let current = model.head_bytes();
            model.load_head(bytes)?;
            let after = model.logits(ids, markers)?;
            // Bytes are compared exactly; the logits difference is reported.
            if current != bytes {
                return Err(Refusal::new(
                    "artifact_invalid",
                    "the head read back from disk is not the head in memory",
                ));
            }
            let diff = f64::from(
                (before[0] - after[0])
                    .abs()
                    .max((before[1] - after[1]).abs()),
            );
            Ok((diff, after))
        }

        fn frozen_hash(&mut self) -> Result<String, Refusal> {
            Ok(self.model()?.frozen_encoder_sha256())
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("foundry-learn: the learning worker targets macOS");
    std::process::exit(78);
}
