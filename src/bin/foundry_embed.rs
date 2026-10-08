//! 009 T004 real embedding worker: the shared worker runtime around a
//! pinned, statically linked llama.cpp (Metal, embedded shader library), no
//! Python. Built only with the non-default `embed-worker` feature (build.rs
//! links llama.cpp into this binary alone); `foundry` never links it.
//!
//! The core renders every card or query with the profile's template and
//! tokenizes it with the profile's `tokenizer.json`; this worker feeds those
//! IDs to llama.cpp unchanged. One context serves every call: n_ctx =
//! n_batch = n_ubatch = 2048 tokens (the protocol's per-call total) and at
//! most 32 sequences, so one call is ONE llama.cpp evaluation. Each input is
//! its own sequence, placed in the batch grouped by length; the pooled
//! vector of each sequence (the descriptor's pinned pooling) is cut to the
//! descriptor's dimension and renormalized. A split GGUF is refused before
//! llama.cpp opens it: its other shards are outside the verified inventory.
//!
//! `--probe <name>` runs one named development isolation check inside the
//! sandboxed bundle and prints a JSON verdict; probes never load the model.
//! `--notices DIR` writes the license text of the linked llama.cpp (embedded
//! by build.rs from the pinned commit's tree it built; it covers the vendored
//! ggml) to `DIR/LICENSE` and prints the llama.cpp commit, for package.sh.
#[cfg(target_os = "macos")]
fn main() {
    use std::io::Write;

    let argv: Vec<String> = std::env::args().collect();
    if let [_, flag, dir] = argv.as_slice()
        && flag == "--notices"
    {
        let dir = std::path::Path::new(dir);
        let code = context_foundry::neural::worker_runtime::write_notices(dir, llama::LICENSE);
        let _ = std::io::stdout().flush();
        std::process::exit(code);
    }
    if let Some(index) = argv.iter().position(|arg| arg == "--probe") {
        let name = argv.get(index + 1).cloned().unwrap_or_default();
        let rest = argv[index + 2..].to_vec();
        context_foundry::neural::probes::run(&name, &rest);
        let _ = std::io::stdout().flush();
        return;
    }
    use context_foundry::neural::worker_runtime::{self, WorkerArgs};
    let (args, rest) = match WorkerArgs::parse(argv.into_iter().skip(1)) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("foundry-embed: {message}");
            std::process::exit(64);
        }
    };
    match rest.as_slice() {
        [] => {}
        #[cfg(feature = "test-faults")]
        [flag, path] if flag == "--phase-file" => worker_runtime::set_phase_file(path),
        _ => {
            eprintln!("foundry-embed: unknown arguments {rest:?}");
            std::process::exit(64);
        }
    }
    let code = worker_runtime::serve(args, real_run);
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}

/// Check the descriptor against what this adapter implements, load the
/// verified GGUF, report `ready` with the model's vocabulary bound, then
/// serve admitted jobs one at a time. Every failure after `ready` is a named
/// job failure, never a panic.
#[cfg(target_os = "macos")]
fn real_run(engine: &mut context_foundry::neural::worker_runtime::Engine) -> i32 {
    use context_foundry::neural::worker_runtime::check_real_descriptor;

    if let Err(message) = check_real_descriptor(&engine.args.expected) {
        engine.fail_load("descriptor_unsupported", &message);
        return 1;
    }
    let gguf = engine.args.model_dir.join(&engine.args.expected.gguf);
    let mut embedder = match llama::Embedder::load(&gguf, &engine.args.expected) {
        Ok(embedder) => embedder,
        Err(message) => {
            engine.fail_load("load_failed", &message);
            return 1;
        }
    };
    if !engine.send_ready(embedder.vocab_bound()) {
        return 1;
    }
    while let Some(job) = engine.next_job() {
        let alive = match embedder.embed(&job.inputs) {
            Ok(vectors) => engine.finish_job(job, vectors),
            Err(message) => engine.fail_job(job, "inference_failed", &message),
        };
        if !alive {
            return 1;
        }
    }
    0
}

#[cfg(target_os = "macos")]
mod llama {
    use context_foundry::neural::provider::{
        FunctionDescriptor, LLAMA_CPP_COMMIT, MAX_DOCUMENT_BATCH, SERVING_LIMIT_TOKENS,
        TokenizedInput,
    };
    use context_foundry::neural::worker_runtime::{mark_phase, refuse_split_gguf};
    use std::ffi::{CStr, CString, c_char, c_void};
    use std::io::Write as _;
    use std::os::unix::ffi::OsStrExt as _;
    use std::path::Path;
    use std::ptr::NonNull;
    use std::sync::Once;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Exact bindgen output over the pinned `llama.h` (with its compile-time
    /// layout assertions for every struct passed by value).
    #[allow(
        non_upper_case_globals,
        non_camel_case_types,
        non_snake_case,
        dead_code,
        unsafe_op_in_unsafe_fn,
        clippy::all
    )]
    mod ffi {
        include!(concat!(env!("OUT_DIR"), "/llama_bindings.rs"));
    }

    /// The commit build.rs built and linked is the commit descriptors must
    /// name.
    const _: () = assert!(
        same(env!("FOUNDRY_LLAMA_CPP_COMMIT"), LLAMA_CPP_COMMIT),
        "build.rs and worker_runtime pin different llama.cpp commits"
    );

    /// The pinned commit's `LICENSE` (MIT, "The ggml authors"), which
    /// build.rs copied into `OUT_DIR` from the tree it built; at this commit
    /// ggml has no license file of its own, so this one text covers
    /// llama.cpp and its ggml.
    pub const LICENSE: &str = include_str!(concat!(env!("OUT_DIR"), "/llama.cpp-LICENSE"));

    const fn same(a: &str, b: &str) -> bool {
        let (a, b) = (a.as_bytes(), b.as_bytes());
        if a.len() != b.len() {
            return false;
        }
        let mut i = 0;
        while i < a.len() {
            if a[i] != b[i] {
                return false;
            }
            i += 1;
        }
        true
    }

    /// The descriptor's pooling as llama.cpp's pooling type.
    fn pooling_type(pooling: &str) -> Result<ffi::llama_pooling_type, String> {
        match pooling {
            "mean" => Ok(ffi::LLAMA_POOLING_TYPE_MEAN),
            "cls" => Ok(ffi::LLAMA_POOLING_TYPE_CLS),
            "last" => Ok(ffi::LLAMA_POOLING_TYPE_LAST),
            other => Err(format!("pooling {other:?} has no llama.cpp pooling type")),
        }
    }

    /// Keep the leading `dims` values and L2-normalize them in f64. Refuses a
    /// nonfinite value or a zero norm.
    pub fn truncate_normalize(values: &[f32], dims: usize) -> Result<Vec<f32>, String> {
        let head = values
            .get(..dims)
            .ok_or_else(|| format!("the embedding has {} values, {dims} needed", values.len()))?;
        let mut sum = 0f64;
        for (index, &value) in head.iter().enumerate() {
            if !value.is_finite() {
                return Err(format!("embedding value {index} is not finite ({value})"));
            }
            sum += f64::from(value) * f64::from(value);
        }
        let norm = sum.sqrt();
        if !(norm.is_finite() && norm > 0.0) {
            return Err(format!("the embedding prefix has norm {norm}"));
        }
        Ok(head.iter().map(|&v| (f64::from(v) / norm) as f32).collect())
    }

    static LAST_LEVEL: AtomicU32 = AtomicU32::new(0);
    static BACKEND: Once = Once::new();

    /// llama.cpp/ggml log sink: stderr only, never stdout (stdout is the
    /// supervisor's frame channel). WARN and ERROR only, so the load chatter
    /// does not flood the supervisor's bounded stderr tail; CONT lines follow
    /// the level they continue.
    unsafe extern "C" fn log_to_stderr(
        level: ffi::ggml_log_level,
        text: *const c_char,
        _user: *mut c_void,
    ) {
        let level = level as u32;
        let effective = if level == ffi::GGML_LOG_LEVEL_CONT as u32 {
            LAST_LEVEL.load(Ordering::Relaxed)
        } else {
            LAST_LEVEL.store(level, Ordering::Relaxed);
            level
        };
        if effective < ffi::GGML_LOG_LEVEL_WARN as u32 || text.is_null() {
            return;
        }
        // SAFETY: ggml passes a NUL-terminated string valid for this call.
        let bytes = unsafe { CStr::from_ptr(text) }.to_bytes();
        let _ = std::io::stderr().write_all(bytes);
    }

    fn init_backend() {
        BACKEND.call_once(|| unsafe {
            // Before any other llama.cpp call, so no line uses the default sink.
            ffi::llama_log_set(Some(log_to_stderr), std::ptr::null_mut());
            ffi::llama_backend_init();
        });
    }

    /// A loaded GGUF embedding model and its one context: every layer on
    /// Metal, embeddings on, the descriptor's pooling pinned, one unified
    /// context of [`SERVING_LIMIT_TOKENS`] for up to [`MAX_DOCUMENT_BATCH`]
    /// sequences, F32 K/V, flash attention off.
    pub struct Embedder {
        model: NonNull<ffi::llama_model>,
        ctx: NonNull<ffi::llama_context>,
        batch: ffi::llama_batch,
        n_vocab: u32,
        n_embd_out: usize,
        dims: usize,
        use_encode: bool,
    }

    impl Embedder {
        pub fn load(path: &Path, descriptor: &FunctionDescriptor) -> Result<Self, String> {
            let pooling = pooling_type(&descriptor.pooling)?;
            init_backend();
            if !unsafe { ffi::llama_supports_gpu_offload() } {
                return Err("this llama.cpp build cannot offload to a GPU (Metal)".into());
            }
            if !path.is_file() {
                return Err(format!("{} is not a file", path.display()));
            }
            // Before llama.cpp opens anything: a split GGUF would make it
            // open shards outside the verified inventory.
            refuse_split_gguf(path)?;
            let c_path = CString::new(path.as_os_str().as_bytes())
                .map_err(|_| format!("{} contains a NUL byte", path.display()))?;
            let mut model_params = unsafe { ffi::llama_model_default_params() };
            model_params.n_gpu_layers = -1; // every layer (and the output) on the GPU
            model_params.progress_callback = None;
            let model = NonNull::new(unsafe {
                ffi::llama_model_load_from_file(c_path.as_ptr(), model_params)
            })
            .ok_or_else(|| format!("llama.cpp could not load {}", path.display()))?;
            let has_encoder = unsafe { ffi::llama_model_has_encoder(model.as_ptr()) };
            let has_decoder = unsafe { ffi::llama_model_has_decoder(model.as_ptr()) };
            if has_encoder && has_decoder {
                unsafe { ffi::llama_model_free(model.as_ptr()) };
                return Err("encoder-decoder models are not supported for embeddings".into());
            }
            let n_ctx = SERVING_LIMIT_TOKENS as u32;
            let mut ctx_params = unsafe { ffi::llama_context_default_params() };
            ctx_params.n_ctx = n_ctx;
            ctx_params.n_batch = n_ctx;
            ctx_params.n_ubatch = n_ctx;
            ctx_params.n_seq_max = MAX_DOCUMENT_BATCH as u32;
            // One context for every sequence of a call: each may use all of it.
            ctx_params.kv_unified = true;
            ctx_params.embeddings = true;
            ctx_params.pooling_type = pooling;
            ctx_params.flash_attn_type = ffi::LLAMA_FLASH_ATTN_TYPE_DISABLED;
            ctx_params.type_k = ffi::GGML_TYPE_F32;
            ctx_params.type_v = ffi::GGML_TYPE_F32;
            ctx_params.offload_kqv = true;
            ctx_params.no_perf = true;
            let Some(ctx) =
                NonNull::new(unsafe { ffi::llama_init_from_model(model.as_ptr(), ctx_params) })
            else {
                unsafe { ffi::llama_model_free(model.as_ptr()) };
                return Err("llama.cpp could not create the embedding context".into());
            };
            // From here on `Self` owns the model, the context and the batch;
            // a later failure frees them.
            let batch = unsafe { ffi::llama_batch_init(n_ctx as i32, 0, 1) };
            let mut embedder = Self {
                model,
                ctx,
                batch,
                n_vocab: 0,
                n_embd_out: 0,
                dims: descriptor.dims(),
                use_encode: has_encoder && !has_decoder,
            };
            embedder.check(pooling)?;
            Ok(embedder)
        }

        /// Refuse a context or model that is not what the descriptor pins.
        fn check(&mut self, pooling: ffi::llama_pooling_type) -> Result<(), String> {
            if self.batch.token.is_null()
                || self.batch.pos.is_null()
                || self.batch.n_seq_id.is_null()
                || self.batch.seq_id.is_null()
                || self.batch.logits.is_null()
            {
                return Err("llama_batch_init returned an incomplete batch".into());
            }
            let ctx = self.ctx.as_ptr();
            let actual = unsafe { ffi::llama_pooling_type(ctx) };
            if actual != pooling {
                return Err(format!(
                    "the context pools with type {actual}, not the pinned {pooling}"
                ));
            }
            let (n_ctx, n_batch, n_ubatch, n_seq_max) = unsafe {
                (
                    ffi::llama_n_ctx(ctx),
                    ffi::llama_n_batch(ctx),
                    ffi::llama_n_ubatch(ctx),
                    ffi::llama_n_seq_max(ctx),
                )
            };
            let want = SERVING_LIMIT_TOKENS as u32;
            if n_ctx.min(n_batch).min(n_ubatch) < want || n_seq_max < MAX_DOCUMENT_BATCH as u32 {
                return Err(format!(
                    "llama.cpp gave n_ctx {n_ctx}, n_batch {n_batch}, n_ubatch {n_ubatch}, \
                     n_seq_max {n_seq_max}; {want} tokens and {MAX_DOCUMENT_BATCH} sequences needed"
                ));
            }
            let vocab = unsafe { ffi::llama_model_get_vocab(self.model.as_ptr()) };
            if vocab.is_null() {
                return Err("the model has no vocabulary".into());
            }
            let n_vocab = unsafe { ffi::llama_vocab_n_tokens(vocab) };
            self.n_vocab = u32::try_from(n_vocab)
                .ok()
                .filter(|&n| n > 0)
                .ok_or_else(|| format!("the model reports {n_vocab} tokens"))?;
            let n_embd_out = unsafe { ffi::llama_model_n_embd_out(self.model.as_ptr()) };
            if n_embd_out <= 0 || (n_embd_out as usize) < self.dims {
                return Err(format!(
                    "the model embeds {n_embd_out} values; dimensions {} needs at least that many",
                    self.dims
                ));
            }
            self.n_embd_out = n_embd_out as usize;
            Ok(())
        }

        /// One past the largest token ID the model accepts: the runtime
        /// refuses any request ID at or above it.
        pub fn vocab_bound(&self) -> u32 {
            self.n_vocab
        }

        /// Embed one call's inputs (already within the protocol's caps) in
        /// ONE llama.cpp evaluation: input `i` is sequence `i`, positions
        /// from 0, sequences placed shortest first (grouped by length); the
        /// pooled vector of each sequence is read by its ID, so placement
        /// never changes which vector answers which input.
        pub fn embed(&mut self, inputs: &[TokenizedInput]) -> Result<Vec<Vec<f32>>, String> {
            mark_phase("call");
            let total: usize = inputs.iter().map(|input| input.ids.len()).sum();
            if inputs.is_empty() || inputs.len() > MAX_DOCUMENT_BATCH {
                return Err(format!(
                    "{} inputs; 1..={MAX_DOCUMENT_BATCH} allowed",
                    inputs.len()
                ));
            }
            if total > SERVING_LIMIT_TOKENS || inputs.iter().any(|input| input.ids.is_empty()) {
                return Err(format!(
                    "{total} tokens (or an empty input); 1..={SERVING_LIMIT_TOKENS} allowed"
                ));
            }
            let mut order: Vec<usize> = (0..inputs.len()).collect();
            order.sort_by_key(|&index| inputs[index].ids.len());
            // SAFETY: the batch was allocated for SERVING_LIMIT_TOKENS tokens
            // with one sequence-ID slot each (`check` verified every array),
            // and `total` is at most that.
            unsafe {
                let token = std::slice::from_raw_parts_mut(self.batch.token, total);
                let pos = std::slice::from_raw_parts_mut(self.batch.pos, total);
                let n_seq_id = std::slice::from_raw_parts_mut(self.batch.n_seq_id, total);
                let seq_id = std::slice::from_raw_parts_mut(self.batch.seq_id, total);
                let logits = std::slice::from_raw_parts_mut(self.batch.logits, total);
                let mut at = 0;
                for &index in &order {
                    for (position, &id) in inputs[index].ids.iter().enumerate() {
                        if id >= self.n_vocab {
                            return Err(format!("input {index}: token ID {id} is out of range"));
                        }
                        if seq_id[at].is_null() {
                            return Err("llama_batch_init returned a null sequence-ID slot".into());
                        }
                        token[at] = id as ffi::llama_token;
                        pos[at] = position as ffi::llama_pos;
                        n_seq_id[at] = 1;
                        *seq_id[at] = index as ffi::llama_seq_id;
                        logits[at] = 1;
                        at += 1;
                    }
                }
            }
            self.batch.n_tokens = total as i32;
            let ctx = self.ctx.as_ptr();
            unsafe { ffi::llama_memory_clear(ffi::llama_get_memory(ctx), true) };
            mark_phase("eval");
            let (call, status) = if self.use_encode {
                ("llama_encode", unsafe {
                    ffi::llama_encode(ctx, self.batch)
                })
            } else {
                ("llama_decode", unsafe {
                    ffi::llama_decode(ctx, self.batch)
                })
            };
            mark_phase("evaluated");
            if status != 0 {
                return Err(format!(
                    "{call} returned {status} for {} sequences of {total} tokens",
                    inputs.len()
                ));
            }
            let mut vectors = Vec::with_capacity(inputs.len());
            for index in 0..inputs.len() {
                let pooled = unsafe { ffi::llama_get_embeddings_seq(ctx, index as i32) };
                if pooled.is_null() {
                    return Err(format!(
                        "llama.cpp returned no pooled embedding for input {index}"
                    ));
                }
                // SAFETY: a pooled sequence embedding has n_embd_out floats.
                let pooled = unsafe { std::slice::from_raw_parts(pooled, self.n_embd_out) };
                vectors.push(
                    truncate_normalize(pooled, self.dims)
                        .map_err(|e| format!("input {index}: {e}"))?,
                );
            }
            Ok(vectors)
        }
    }

    impl Drop for Embedder {
        fn drop(&mut self) {
            unsafe {
                ffi::llama_batch_free(self.batch);
                ffi::llama_free(self.ctx.as_ptr());
                ffi::llama_model_free(self.model.as_ptr());
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn truncation_normalizes_the_prefix_only() {
            let v = truncate_normalize(&[3.0, 4.0, 100.0], 2).unwrap();
            assert_eq!(v, vec![0.6, 0.8]);
            assert!(truncate_normalize(&[0.0, 0.0], 2).is_err());
            assert!(truncate_normalize(&[f32::NAN, 1.0], 2).is_err());
            assert!(truncate_normalize(&[1.0], 2).is_err());
        }

        #[test]
        fn every_descriptor_pooling_maps_to_llama_cpp() {
            for pooling in context_foundry::neural::provider::POOLINGS {
                assert!(pooling_type(pooling).is_ok(), "{pooling}");
            }
            assert!(pooling_type("max").is_err());
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("foundry-embed: the embedding worker targets macOS");
    std::process::exit(78);
}
