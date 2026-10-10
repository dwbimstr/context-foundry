//! Owned decoder worker. The core never links llama.cpp or loads model weights.
use context_foundry::knowledge::local::{
    Device,
    protocol::Hello,
    runtime::{self, Generator},
};
use context_foundry::knowledge::provider::{Reply, Usage};
use serde_json::Value;
use std::ffi::CString;
use std::ptr::NonNull;

#[allow(
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    dead_code,
    unsafe_op_in_unsafe_fn,
    unnecessary_transmutes,
    clippy::all
)]
mod ffi {
    include!(concat!(env!("OUT_DIR"), "/llama_bindings.rs"));
}

// Brief grammar prevents schema drift and tool/thought framing; source/citation validation stays
// in the existing source owner. The grammar is part of the adapter revision.
const JSON_GRAMMAR: &str = r#"
root ::= "{" ws "\"purpose\"" ws ":" ws required "," ws "\"behavior\"" ws ":" ws required "," ws "\"provides\"" ws ":" ws claims "," ws "\"consumes\"" ws ":" ws claims "," ws "\"constraints\"" ws ":" ws claims "," ws "\"tests\"" ws ":" ws claims "," ws "\"unknowns\"" ws ":" ws strings "}" ws
required ::= "[" ws claim ("," ws claim)* "]" ws
claims ::= "[" ws (claim ("," ws claim)*)? "]" ws
claim ::= "{" ws "\"text\"" ws ":" ws string "," ws "\"citations\"" ws ":" ws "[" ws index ("," ws index)* "]" ws "," ws "\"identifiers\"" ws ":" ws strings "}" ws
index ::= ([0-9] | [12] [0-9] | "3" [01]) ws
strings ::= "[" ws (string ("," ws string)*)? "]" ws
string ::= "\"" ([^"\\\x00-\x1F] | "\\" (["\\/bfnrt] | "u" [0-9a-fA-F]{4}))* "\"" ws
ws ::= [ \t\n\r]*
"#;

struct Decoder {
    model: NonNull<ffi::llama_model>,
    ctx: NonNull<ffi::llama_context>,
    config: context_foundry::knowledge::local::Config,
    gpu: ffi::ggml_backend_dev_t,
    peak_gpu: u64,
    monitor: Option<GpuMonitor>,
}
impl Drop for Decoder {
    fn drop(&mut self) {
        drop(self.monitor.take());
        unsafe {
            ffi::llama_free(self.ctx.as_ptr());
            ffi::llama_model_free(self.model.as_ptr());
        }
    }
}
struct GpuMonitor {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl GpuMonitor {
    fn start(gpu: ffi::ggml_backend_dev_t, device: &Device) -> Result<Option<Self>, &'static str> {
        if gpu.is_null() {
            return Ok(None);
        }
        let device = device.clone();
        let address = gpu as usize;
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done = stop.clone();
        let thread = std::thread::Builder::new()
            .name("generation-gpu".into())
            .spawn(move || {
                while !done.load(std::sync::atomic::Ordering::SeqCst) {
                    if gpu_usage(address as ffi::ggml_backend_dev_t, &device).is_err() {
                        // Metal observation is process-local and thread-safe at this
                        // pin. Stop allocations/compute even during a blocked load.
                        unsafe { libc::_exit(74) };
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            })
            .map_err(|_| "knowledge_resource_unavailable")?;
        Ok(Some(Self {
            stop,
            thread: Some(thread),
        }))
    }
}
impl Drop for GpuMonitor {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
unsafe extern "C" fn quiet_log(
    _: ffi::ggml_log_level,
    _: *const libc::c_char,
    _: *mut libc::c_void,
) {
}
impl Decoder {
    fn load(hello: &Hello) -> Result<Self, &'static str> {
        if hello.config.llama_cpp != env!("FOUNDRY_LLAMA_CPP_COMMIT") {
            return Err("knowledge_artifact_invalid");
        }
        unsafe {
            ffi::llama_log_set(Some(quiet_log), std::ptr::null_mut());
            ffi::llama_backend_init();
        }
        let mut params = unsafe { ffi::llama_model_default_params() };
        params.load_mode = ffi::LLAMA_LOAD_MODE_MMAP;
        let mut gpu = std::ptr::null_mut();
        let mut devices = [std::ptr::null_mut(); 2];
        match hello.config.device {
            Device::Cpu => {
                params.n_gpu_layers = 0;
                params.devices = devices.as_mut_ptr();
            }
            Device::Metal { layers, .. } => {
                if !cfg!(target_os = "macos") || !unsafe { ffi::llama_supports_gpu_offload() } {
                    return Err("knowledge_isolation_unavailable");
                }
                gpu = unsafe { ffi::ggml_backend_dev_by_type(ffi::GGML_BACKEND_DEVICE_TYPE_GPU) };
                if gpu.is_null() {
                    return Err("knowledge_resource_unavailable");
                }
                devices[0] = gpu;
                params.devices = devices.as_mut_ptr();
                params.n_gpu_layers = layers as i32;
                params.split_mode = ffi::LLAMA_SPLIT_MODE_NONE;
                gpu_usage(gpu, &hello.config.device)?;
            }
        }
        let monitor = GpuMonitor::start(gpu, &hello.config.device)?;
        // The verified, single-file snapshot is the only model-readable object.
        // Missing split shards cannot be opened through the isolation profile.
        let fd = unsafe { libc::dup(hello.model_fd) };
        if fd < 0 {
            return Err("knowledge_artifact_invalid");
        }
        let file = unsafe { libc::fdopen(fd, c"rb".as_ptr()) };
        if file.is_null() {
            unsafe {
                libc::close(fd);
            }
            return Err("knowledge_artifact_invalid");
        }
        let loaded = unsafe { ffi::llama_model_load_from_file_ptr(file.cast(), params) };
        unsafe {
            libc::fclose(file);
        }
        let model = NonNull::new(loaded).ok_or("knowledge_worker_failed")?;
        let create = (|| {
            if unsafe { ffi::llama_model_has_encoder(model.as_ptr()) }
                || !unsafe { ffi::llama_model_has_decoder(model.as_ptr()) }
                || unsafe { ffi::llama_model_n_ctx_train(model.as_ptr()) }
                    < hello.config.context_tokens as i32
            {
                return Err("knowledge_artifact_invalid");
            }
            gpu_usage(gpu, &hello.config.device)?;
            let mut params = unsafe { ffi::llama_context_default_params() };
            params.n_ctx = hello.config.context_tokens;
            params.n_batch = hello.config.batch_tokens;
            params.n_ubatch = hello.config.batch_tokens;
            params.n_seq_max = 1;
            params.n_threads = hello.config.threads as i32;
            params.n_threads_batch = hello.config.threads as i32;
            params.offload_kqv = !gpu.is_null();
            params.op_offload = !gpu.is_null();
            params.no_perf = true;
            let ctx = NonNull::new(unsafe { ffi::llama_init_from_model(model.as_ptr(), params) })
                .ok_or("knowledge_worker_failed")?;
            Ok(ctx)
        })();
        let ctx = match create {
            Ok(ctx) => ctx,
            Err(error) => {
                unsafe {
                    ffi::llama_model_free(model.as_ptr());
                }
                return Err(error);
            }
        };
        let mut result = Self {
            model,
            ctx,
            config: hello.config.clone(),
            gpu,
            peak_gpu: 0,
            monitor,
        };
        if unsafe { ffi::llama_n_ctx(result.ctx.as_ptr()) } < result.config.context_tokens
            || unsafe { ffi::llama_n_batch(result.ctx.as_ptr()) } < result.config.batch_tokens
            || unsafe { ffi::llama_n_threads(result.ctx.as_ptr()) } != result.config.threads as i32
        {
            return Err("knowledge_resource_unavailable");
        }
        result.check_gpu()?;
        Ok(result)
    }
    fn check_gpu(&mut self) -> Result<(), &'static str> {
        self.peak_gpu = self.peak_gpu.max(gpu_usage(self.gpu, &self.config.device)?);
        Ok(())
    }
    fn prompt(&self, body: &Value) -> Result<Vec<ffi::llama_token>, &'static str> {
        let messages = body["messages"]
            .as_array()
            .filter(|m| m.len() == 2)
            .ok_or("knowledge_input_limit")?;
        let mut strings = Vec::new();
        for (message, role) in messages.iter().zip(["system", "user"]) {
            if message["role"] != role {
                return Err("knowledge_input_limit");
            }
            strings.push((
                CString::new(role).unwrap(),
                CString::new(message["content"].as_str().ok_or("knowledge_input_limit")?)
                    .map_err(|_| "knowledge_input_limit")?,
            ));
        }
        let chat: Vec<_> = strings
            .iter()
            .map(|(role, content)| ffi::llama_chat_message {
                role: role.as_ptr(),
                content: content.as_ptr(),
            })
            .collect();
        let template = CString::new(self.config.chat_template.as_str()).unwrap();
        let mut prompt = vec![0u8; 256 * 1024];
        let size = unsafe {
            ffi::llama_chat_apply_template(
                template.as_ptr(),
                chat.as_ptr(),
                chat.len(),
                true,
                prompt.as_mut_ptr().cast(),
                prompt.len() as i32,
            )
        };
        if size <= 0 || size as usize > prompt.len() {
            return Err("knowledge_input_limit");
        }
        let vocab = unsafe { ffi::llama_model_get_vocab(self.model.as_ptr()) };
        let mut tokens = vec![0; self.config.context_tokens as usize];
        let n = unsafe {
            ffi::llama_tokenize(
                vocab,
                prompt.as_ptr().cast(),
                size,
                tokens.as_mut_ptr(),
                tokens.len() as i32,
                true,
                true,
            )
        };
        if n <= 0 || n as usize > tokens.len() {
            return Err("knowledge_input_limit");
        }
        tokens.truncate(n as usize);
        Ok(tokens)
    }
    fn decode(&mut self, tokens: &mut [ffi::llama_token]) -> Result<(), &'static str> {
        self.check_gpu()?;
        let batch = unsafe { ffi::llama_batch_get_one(tokens.as_mut_ptr(), tokens.len() as i32) };
        if unsafe { ffi::llama_decode(self.ctx.as_ptr(), batch) } != 0 {
            return Err("knowledge_worker_failed");
        }
        unsafe {
            ffi::llama_synchronize(self.ctx.as_ptr());
        }
        self.check_gpu()
    }
}
fn gpu_usage(gpu: ffi::ggml_backend_dev_t, config: &Device) -> Result<u64, &'static str> {
    let Device::Metal {
        allocation_bytes,
        reserve_bytes,
        ..
    } = config
    else {
        return Ok(0);
    };
    let (mut free, mut total) = (0usize, 0usize);
    if gpu.is_null() {
        return Err("knowledge_resource_unavailable");
    }
    unsafe {
        ffi::ggml_backend_dev_memory(gpu, &mut free, &mut total);
    }
    // At this pin Metal reports recommendedMaxWorkingSetSize minus this
    // process's currentAllocatedSize. This is NOT global free VRAM telemetry.
    if total == 0 || free > total {
        return Err("knowledge_resource_unavailable");
    }
    let used = (total - free) as u64;
    if used > *allocation_bytes || (free as u64) < *reserve_bytes {
        return Err("knowledge_resource_limit");
    }
    Ok(used)
}
impl Generator for Decoder {
    fn generate(&mut self, body: &Value) -> Result<(Reply, u64), &'static str> {
        let max = body["max_tokens"]
            .as_u64()
            .filter(|n| (1..=8192).contains(n))
            .ok_or("knowledge_input_limit")?;
        let mut tokens = self.prompt(body)?;
        if tokens.len() as u64 + max > self.config.context_tokens as u64 {
            return Err("knowledge_input_limit");
        }
        let prompt_tokens = tokens.len() as u64;
        unsafe {
            ffi::llama_memory_clear(ffi::llama_get_memory(self.ctx.as_ptr()), true);
        }
        for batch in tokens.chunks_mut(self.config.batch_tokens as usize) {
            self.decode(batch)?;
        }
        struct Sampler(NonNull<ffi::llama_sampler>);
        impl Drop for Sampler {
            fn drop(&mut self) {
                unsafe {
                    ffi::llama_sampler_free(self.0.as_ptr());
                }
            }
        }
        let sampler = Sampler(
            NonNull::new(unsafe {
                ffi::llama_sampler_chain_init(ffi::llama_sampler_chain_default_params())
            })
            .ok_or("knowledge_worker_failed")?,
        );
        let vocab = unsafe { ffi::llama_model_get_vocab(self.model.as_ptr()) };
        let grammar = CString::new(JSON_GRAMMAR).unwrap();
        let grammar = NonNull::new(unsafe {
            ffi::llama_sampler_init_grammar(vocab, grammar.as_ptr(), c"root".as_ptr())
        })
        .ok_or("knowledge_worker_failed")?;
        unsafe {
            ffi::llama_sampler_chain_add(sampler.0.as_ptr(), grammar.as_ptr());
        }
        let greedy = NonNull::new(unsafe { ffi::llama_sampler_init_greedy() })
            .ok_or("knowledge_worker_failed")?;
        unsafe {
            ffi::llama_sampler_chain_add(sampler.0.as_ptr(), greedy.as_ptr());
        }
        let mut content = Vec::new();
        for completion in 1..=max {
            let token =
                unsafe { ffi::llama_sampler_sample(sampler.0.as_ptr(), self.ctx.as_ptr(), -1) };
            if token < 0 {
                return Err("knowledge_worker_failed");
            }
            if unsafe { ffi::llama_vocab_is_eog(vocab, token) } {
                return Ok((
                    Reply {
                        content: String::from_utf8(content)
                            .map_err(|_| "knowledge_worker_failed")?,
                        usage: Usage {
                            prompt_tokens: Some(prompt_tokens),
                            completion_tokens: Some(completion),
                        },
                    },
                    self.peak_gpu,
                ));
            }
            let mut piece = [0u8; 32768];
            let n = unsafe {
                ffi::llama_token_to_piece(
                    vocab,
                    token,
                    piece.as_mut_ptr().cast(),
                    piece.len() as i32,
                    0,
                    false,
                )
            };
            if n < 0 || n as usize > piece.len() || content.len() + n as usize > 256 * 1024 {
                return Err("knowledge_output_limit");
            }
            content.extend_from_slice(&piece[..n as usize]);
            self.decode(&mut [token])?;
        }
        Err("knowledge_output_limit")
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if let [_, flag, dir] = args.as_slice()
        && flag == "--notices"
    {
        let dir = std::path::Path::new(dir);
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("LICENSE"),
            include_str!(concat!(env!("OUT_DIR"), "/llama.cpp-LICENSE")),
        )
        .unwrap();
        println!("{}", env!("FOUNDRY_LLAMA_CPP_COMMIT"));
        return;
    }
    if args.len() != 1 {
        std::process::exit(64);
    }
    let code = runtime::serve(|hello| Ok(Box::new(Decoder::load(hello)?)));
    std::process::exit(code);
}
