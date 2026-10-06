//! 009 real embedding worker: the shared worker runtime driving the
//! UNCHANGED publisher loader (`nemotron3_embed_mlx.py` from the verified
//! model directory) through pyo3. Built only with the non-default
//! `embed-worker` feature; `foundry` never links Python.
//!
//! Before any Python exists, `--probe <name>` runs one named development
//! isolation check inside the sandboxed bundle and prints a JSON verdict;
//! those probes exist for the profile's negative tests with positive
//! controls and never load the model.
#[cfg(target_os = "macos")]
fn main() {
    use std::io::Write;

    let argv: Vec<String> = std::env::args().collect();
    if let Some(index) = argv.iter().position(|arg| arg == "--probe") {
        let name = argv.get(index + 1).cloned().unwrap_or_default();
        let rest = argv[index + 2..].to_vec();
        probes::run(&name, &rest);
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

/// Load the publisher model once inside one Python attach, report `ready`,
/// then serve admitted jobs: build right-padded `int32` input-ID and mask
/// arrays, call the unchanged `NemotronEmbedModel.__call__`, force
/// evaluation, copy float32 vectors and clear the MLX cache per batch.
///
/// Order is fixed. The descriptor is checked against what this adapter
/// actually implements first; then CPython is initialized in ISOLATED mode
/// with site import disabled and an explicit search path; only then does any
/// Python code run.
#[cfg(target_os = "macos")]
fn real_run(engine: &mut context_foundry::neural::worker_runtime::Engine) -> i32 {
    use context_foundry::neural::worker_runtime::{REAL_PAD_ID, check_real_descriptor};
    use pyo3::prelude::*;

    if let Err(message) = check_real_descriptor(&engine.args.expected) {
        engine.fail_load("descriptor_unsupported", &message);
        return 1;
    }
    let paths = match search_paths(&engine.args.python_home, &engine.args.site_packages) {
        Ok(paths) => paths,
        Err(message) => {
            engine.fail_load("load_failed", &message);
            return 1;
        }
    };
    if let Err(message) = init_isolated_python(&engine.args.python_home, &engine.args.site_packages)
    {
        engine.fail_load("load_failed", &message);
        return 1;
    }

    Python::attach(|py| {
        let (model, vocab) = match setup(py, engine, &paths) {
            Ok(loaded) => loaded,
            Err(message) => {
                engine.fail_load("load_failed", &message);
                return 1;
            }
        };
        let mx: Py<PyModule> = match py.import("mlx.core") {
            Ok(module) => module.unbind(),
            Err(e) => {
                engine.fail_load("load_failed", &format!("mlx.core import: {e}"));
                return 1;
            }
        };
        if !engine.send_ready(vocab) {
            return 1;
        }
        while let Some(job) = py.detach(|| engine.next_job()) {
            let mx = mx.bind(py);
            match embed_batch(model.bind(py), mx, &job.inputs, REAL_PAD_ID as i32) {
                Ok(vectors) => {
                    if !engine.finish_job(job, vectors) {
                        return 1;
                    }
                }
                Err(message) => {
                    if !engine.fail_job(job, "inference_failed", &message) {
                        return 1;
                    }
                }
            }
        }
        0
    })
}

/// The interpreter's complete module search path, in order: the standard
/// library, its extension modules and the profile's site-packages. Nothing
/// else: not the environment, not the working directory, not a user or
/// global site directory.
#[cfg(target_os = "macos")]
fn search_paths(
    python_home: &std::path::Path,
    site_packages: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, String> {
    let lib = python_home.join("lib");
    let mut stdlib = None;
    let entries =
        std::fs::read_dir(&lib).map_err(|e| format!("python home lib {}: {e}", lib.display()))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("python3.") && entry.path().join("os.py").is_file() {
            if stdlib.is_some() {
                return Err(format!(
                    "python home {} holds more than one standard library",
                    python_home.display()
                ));
            }
            stdlib = Some(entry.path());
        }
    }
    let stdlib = stdlib.ok_or_else(|| {
        format!(
            "python home {} holds no lib/python3.N/os.py",
            python_home.display()
        )
    })?;
    Ok(vec![
        stdlib.join("lib-dynload"),
        stdlib,
        site_packages.to_path_buf(),
    ])
}

/// Start CPython from an explicit isolated configuration: the interpreter
/// ignores the environment and the working directory, imports no `site`
/// (so no global `.pth` line and no `sitecustomize`/`usercustomize` ever
/// runs), runs in UTF-8 mode with unbuffered stdio and no bytecode writes,
/// installs no signal handlers, and takes `sys.path` exactly from
/// [`search_paths`]. This is the ONLY place the interpreter starts: pyo3's
/// `auto-initialize` is off, so any other first use fails instead of
/// silently starting an unisolated interpreter.
#[cfg(target_os = "macos")]
fn init_isolated_python(
    python_home: &std::path::Path,
    site_packages: &std::path::Path,
) -> Result<(), String> {
    use pyo3::ffi;
    use std::ffi::{CStr, CString};
    use std::os::unix::ffi::OsStrExt;

    fn check(what: &str, status: ffi::PyStatus) -> Result<(), String> {
        // SAFETY: `status` is a plain value returned by CPython.
        if unsafe { ffi::PyStatus_Exception(status) } == 0 {
            return Ok(());
        }
        let text = |ptr: *const std::ffi::c_char| {
            if ptr.is_null() {
                String::new()
            } else {
                // SAFETY: CPython's status strings are static C strings.
                unsafe { CStr::from_ptr(ptr) }
                    .to_string_lossy()
                    .into_owned()
            }
        };
        Err(format!(
            "python {what}: {} (in {})",
            text(status.err_msg),
            text(status.func)
        ))
    }

    let paths = search_paths(python_home, site_packages)?;
    let c_home = CString::new(python_home.as_os_str().as_bytes())
        .map_err(|_| "python home contains a NUL byte".to_string())?;
    let c_executable = CString::new(python_home.join("bin/python3").as_os_str().as_bytes())
        .map_err(|_| "python home contains a NUL byte".to_string())?;

    // SAFETY: called once on the main thread before any other CPython use;
    // every pointer handed to CPython outlives the call that reads it, and
    // `PyConfig_Clear` releases what the configuration allocated.
    unsafe {
        if ffi::Py_IsInitialized() != 0 {
            return Err("python was already initialized".into());
        }
        let mut preconfig = std::mem::MaybeUninit::<ffi::PyPreConfig>::uninit();
        ffi::PyPreConfig_InitIsolatedConfig(preconfig.as_mut_ptr());
        let mut preconfig = preconfig.assume_init();
        // UTF-8 mode: `open()` defaults, filesystem and stdio encodings are
        // UTF-8 whatever the (empty) environment's locale says.
        preconfig.utf8_mode = 1;
        check("pre-initialization", ffi::Py_PreInitialize(&preconfig))?;

        let mut config = std::mem::MaybeUninit::<ffi::PyConfig>::uninit();
        ffi::PyConfig_InitIsolatedConfig(config.as_mut_ptr());
        let mut config = config.assume_init();
        let outcome = (|| {
            config.isolated = 1;
            config.use_environment = 0;
            config.site_import = 0;
            config.user_site_directory = 0;
            config.install_signal_handlers = 0;
            config.write_bytecode = 0;
            config.buffered_stdio = 0;
            config.safe_path = 1;
            config.parse_argv = 0;
            config.pathconfig_warnings = 0;
            check(
                "home",
                ffi::PyConfig_SetBytesString(
                    &mut config,
                    std::ptr::addr_of_mut!(config.home),
                    c_home.as_ptr(),
                ),
            )?;
            check(
                "executable",
                ffi::PyConfig_SetBytesString(
                    &mut config,
                    std::ptr::addr_of_mut!(config.executable),
                    c_executable.as_ptr(),
                ),
            )?;
            config.module_search_paths_set = 1;
            for dir in &paths {
                let wide: Vec<libc::wchar_t> = dir
                    .to_string_lossy()
                    .chars()
                    .map(|c| c as libc::wchar_t)
                    .chain(std::iter::once(0))
                    .collect();
                check(
                    "module search path",
                    ffi::PyWideStringList_Append(
                        std::ptr::addr_of_mut!(config.module_search_paths),
                        wide.as_ptr(),
                    ),
                )?;
            }
            check("initialization", ffi::Py_InitializeFromConfig(&config))
        })();
        ffi::PyConfig_Clear(&mut config);
        outcome?;
        // Release the GIL the initialization left held, as pyo3's own
        // initialization does, so `Python::attach` takes it normally.
        ffi::PyEval_SaveThread();
    }
    Ok(())
}

/// The interpreter's startup state as JSON: the startup flags, `sys.path`
/// and whether `site`, `sitecustomize` or `usercustomize` were imported.
#[cfg(target_os = "macos")]
fn describe_interpreter(py: pyo3::Python<'_>) -> Result<serde_json::Value, String> {
    use pyo3::prelude::*;

    let sys = py.import("sys").map_err(|e| format!("sys import: {e}"))?;
    let flags = sys
        .getattr("flags")
        .map_err(|e| format!("sys.flags: {e}"))?;
    let flag = |name: &str| -> Result<i64, String> {
        flags
            .getattr(name)
            .and_then(|value| value.extract::<i64>())
            .map_err(|e| format!("sys.flags.{name}: {e}"))
    };
    let path: Vec<String> = sys
        .getattr("path")
        .and_then(|path| path.extract())
        .map_err(|e| format!("sys.path: {e}"))?;
    let modules = sys
        .getattr("modules")
        .map_err(|e| format!("sys.modules: {e}"))?;
    let imported = |name: &str| -> Result<bool, String> {
        modules
            .contains(name)
            .map_err(|e| format!("sys.modules lookup of {name}: {e}"))
    };
    Ok(serde_json::json!({
        "isolated": flag("isolated")?,
        "no_site": flag("no_site")?,
        "ignore_environment": flag("ignore_environment")?,
        "no_user_site": flag("no_user_site")?,
        "utf8_mode": flag("utf8_mode")?,
        "path": path,
        "site_imported": imported("site")?,
        "sitecustomize_imported": imported("sitecustomize")?,
        "usercustomize_imported": imported("usercustomize")?,
    }))
}

/// Refuse to continue unless the running interpreter really is the isolated
/// one [`init_isolated_python`] asked for.
#[cfg(target_os = "macos")]
fn verify_isolation(
    py: pyo3::Python<'_>,
    expected_paths: &[std::path::PathBuf],
) -> Result<(), String> {
    let state = describe_interpreter(py)?;
    let flag = |name: &str| state[name].as_i64().unwrap_or(-1);
    let imported = |name: &str| state[name].as_bool().unwrap_or(true);
    if flag("isolated") != 1
        || flag("no_site") != 1
        || flag("ignore_environment") != 1
        || flag("no_user_site") != 1
        || flag("utf8_mode") != 1
    {
        return Err(format!("the interpreter is not isolated: {state}"));
    }
    if imported("site_imported")
        || imported("sitecustomize_imported")
        || imported("usercustomize_imported")
    {
        return Err(format!("a startup hook module was imported: {state}"));
    }
    let expected: Vec<String> = expected_paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let actual: Vec<String> = state["path"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if actual != expected {
        return Err(format!(
            "sys.path is {actual:?}, expected exactly {expected:?}"
        ));
    }
    Ok(())
}

/// Verify the isolation and the observed runtime closure against the
/// expected descriptor, verify the loader's files exist, then call the
/// unchanged publisher `load`.
#[cfg(target_os = "macos")]
fn setup(
    py: pyo3::Python<'_>,
    engine: &context_foundry::neural::worker_runtime::Engine,
    paths: &[std::path::PathBuf],
) -> Result<(pyo3::Py<pyo3::PyAny>, u32), String> {
    use context_foundry::neural::worker_runtime::REAL_LOADER_INPUTS;
    use pyo3::prelude::*;

    let expected = &engine.args.expected;
    let model_dir = engine.args.model_dir.display().to_string();

    verify_isolation(py, paths)?;

    // The observed runtime closure must equal the expected one, field by
    // field; the worker never selects these values.
    let platform = py
        .import("platform")
        .map_err(|e| format!("platform import: {e}"))?;
    let observed_python: String = platform
        .getattr("python_version")
        .map_err(|e| format!("platform.python_version: {e}"))?
        .call0()
        .and_then(|v| v.extract())
        .map_err(|e| format!("python_version: {e}"))?;
    if observed_python != expected.runtime.python {
        return Err(format!(
            "python is {observed_python}, profile expects {}",
            expected.runtime.python
        ));
    }
    let metadata = py
        .import("importlib.metadata")
        .map_err(|e| format!("importlib.metadata import: {e}"))?;
    let version_of = |dist: &str| -> Result<String, String> {
        metadata
            .getattr("version")
            .map_err(|e| format!("importlib.metadata.version: {e}"))?
            .call1((dist,))
            .and_then(|v| v.extract::<String>())
            .map_err(|e| format!("version of {dist}: {e}"))
    };
    for (dist, want) in [
        ("mlx", &expected.runtime.mlx),
        ("mlx-metal", &expected.runtime.mlx_metal),
        ("mlx-lm", &expected.runtime.mlx_lm),
        ("transformers", &expected.runtime.transformers),
        ("numpy", &expected.runtime.numpy),
    ] {
        let observed = version_of(dist)?;
        if &observed != want {
            return Err(format!("{dist} is {observed}, profile expects {want}"));
        }
    }

    // Named files must exist before `load` may run; a missing model
    // directory would otherwise fall into an implicit Hub download.
    for name in REAL_LOADER_INPUTS {
        if !engine.args.model_dir.join(name).is_file() {
            return Err(format!("model file missing: {name}"));
        }
    }

    // Import the UNCHANGED publisher module from the verified model dir.
    let importlib = py
        .import("importlib.util")
        .map_err(|e| format!("importlib.util import: {e}"))?;
    let loader_path = engine
        .args
        .model_dir
        .join("nemotron3_embed_mlx.py")
        .display()
        .to_string();
    let spec = importlib
        .getattr("spec_from_file_location")
        .map_err(|e| format!("spec_from_file_location: {e}"))?
        .call1(("nemotron3_embed_mlx", loader_path))
        .map_err(|e| format!("spec_from_file_location call: {e}"))?;
    let module = importlib
        .getattr("module_from_spec")
        .map_err(|e| format!("module_from_spec: {e}"))?
        .call1((&spec,))
        .map_err(|e| format!("module_from_spec call: {e}"))?;
    let loader = spec
        .getattr("loader")
        .map_err(|e| format!("spec loader: {e}"))?;
    loader
        .getattr("exec_module")
        .map_err(|e| format!("exec_module lookup: {e}"))?
        .call1((&module,))
        .map_err(|e| format!("publisher module exec: {e}"))?;

    // The unchanged publisher `load`.
    let loaded = module
        .getattr("load")
        .map_err(|e| format!("publisher load: {e}"))?
        .call1((model_dir.as_str(),))
        .map_err(|e| format!("publisher load call: {e}"))?;
    let model = loaded
        .get_item(0)
        .map_err(|e| format!("load result model: {e}"))?;
    // The embedding table's row count is the hard bound on every token ID;
    // the tokenizer's own length can exceed it.
    let vocab = model
        .getattr("args")
        .and_then(|args| args.getattr("vocab_size"))
        .and_then(|size| size.extract::<usize>())
        .map_err(|e| format!("model vocab_size: {e}"))?;
    if vocab == 0 || vocab > u32::MAX as usize {
        return Err(format!("implausible vocabulary bound {vocab}"));
    }
    Ok((model.unbind(), vocab as u32))
}

/// One batch through the publisher model: right-padded `int32` input IDs
/// (pad ID from the descriptor) and `int32` mask arrays, the unchanged
/// `NemotronEmbedModel.__call__`, forced evaluation, float32 copy and
/// `mx.clear_cache()`. Pooling and normalization stay inside the model.
#[cfg(target_os = "macos")]
fn embed_batch(
    model: &pyo3::Bound<'_, pyo3::PyAny>,
    mx: &pyo3::Bound<'_, pyo3::types::PyModule>,
    inputs: &[context_foundry::neural::provider::TokenizedInput],
    pad_id: i32,
) -> Result<Vec<Vec<f32>>, String> {
    use pyo3::prelude::*;

    let width = inputs
        .iter()
        .map(|input| input.ids.len())
        .max()
        .unwrap_or(0);
    if width == 0 {
        return Err("empty batch".into());
    }
    let mut ids: Vec<Vec<i32>> = Vec::with_capacity(inputs.len());
    let mut mask: Vec<Vec<i32>> = Vec::with_capacity(inputs.len());
    for input in inputs {
        let mut row = Vec::with_capacity(width);
        let mut bits = Vec::with_capacity(width);
        for id in &input.ids {
            row.push(*id as i32);
            bits.push(1);
        }
        row.resize(width, pad_id);
        bits.resize(width, 0);
        ids.push(row);
        mask.push(bits);
    }
    let int32 = mx.getattr("int32").map_err(|e| format!("mx.int32: {e}"))?;
    let float32 = mx
        .getattr("float32")
        .map_err(|e| format!("mx.float32: {e}"))?;
    let array = mx.getattr("array").map_err(|e| format!("mx.array: {e}"))?;
    let input_ids = typed_array(&array, ids, &int32, "input_ids")?;
    let attention_mask = typed_array(&array, mask, &int32, "attention_mask")?;
    // Phase marks exist only in `test-faults` builds: a test kills the owner
    // once the worker has really entered the model call and the evaluation.
    context_foundry::neural::worker_runtime::mark_phase("call");
    let output = model
        .call1((input_ids, attention_mask))
        .map_err(|e| format!("NemotronEmbedModel.__call__: {e}"))?;
    context_foundry::neural::worker_runtime::mark_phase("eval");
    mx.getattr("eval")
        .map_err(|e| format!("mx.eval: {e}"))?
        .call1((&output,))
        .map_err(|e| format!("mx.eval: {e}"))?;
    context_foundry::neural::worker_runtime::mark_phase("evaluated");
    let output = output
        .call_method("astype", (float32,), None)
        .map_err(|e| format!("astype float32: {e}"))?;
    let rows: Vec<Vec<f32>> = output
        .call_method0("tolist")
        .and_then(|v| v.extract())
        .map_err(|e| format!("tolist: {e}"))?;
    mx.getattr("clear_cache")
        .map_err(|e| format!("mx.clear_cache: {e}"))?
        .call0()
        .map_err(|e| format!("mx.clear_cache: {e}"))?;
    Ok(rows)
}

/// `mx.array(rows, dtype=dtype)` with an explicit int32 dtype, exactly the
/// array construction the publisher's `encode` performs.
#[cfg(target_os = "macos")]
fn typed_array<'py>(
    array: &pyo3::Bound<'py, pyo3::PyAny>,
    rows: Vec<Vec<i32>>,
    dtype: &pyo3::Bound<'py, pyo3::PyAny>,
    what: &str,
) -> Result<pyo3::Bound<'py, pyo3::PyAny>, String> {
    use pyo3::prelude::*;
    use pyo3::types::PyDict;

    let py = array.py();
    let kwargs = PyDict::new(py);
    kwargs
        .set_item("dtype", dtype)
        .map_err(|e| format!("{what} dtype kwarg: {e}"))?;
    array
        .call((rows,), Some(&kwargs))
        .map_err(|e| format!("{what} array: {e}"))
}

/// One named development-isolation probe. The file, descriptor,
/// environment, network and process probes are shared with 013's learning
/// worker ([`context_foundry::neural::probes`]); the interpreter probes are
/// this worker's own. Each prints one JSON line and exits 0; the verdict
/// (`allowed`, `errno`) is the evidence.
#[cfg(target_os = "macos")]
mod probes {
    use context_foundry::neural::probes::report;

    /// Initialize CPython the way the worker does, then print its startup
    /// state. `isolated` takes the profile's python home and site-packages.
    fn python(mode: &str, rest: &[String]) {
        use std::path::Path;
        let outcome = match (mode, rest) {
            ("isolated", [home, site]) => {
                super::init_isolated_python(Path::new(home), Path::new(site))
            }
            // The positive control: the interpreter as `Py_InitializeEx(0)`
            // started it before the isolation fix (environment, global
            // site, `sitecustomize` and `.pth` all honored).
            ("default", []) => {
                unsafe {
                    pyo3::ffi::Py_InitializeEx(0);
                    pyo3::ffi::PyEval_SaveThread();
                }
                Ok(())
            }
            _ => Err("python probe: isolated <home> <site-packages> | default".to_string()),
        };
        match outcome {
            Err(message) => report(&format!("python-{mode}"), false, message, 22),
            Ok(()) => pyo3::Python::attach(|py| match super::describe_interpreter(py) {
                Ok(state) => report(&format!("python-{mode}"), true, state.to_string(), 0),
                Err(message) => report(&format!("python-{mode}"), false, message, 22),
            }),
        }
    }

    /// Run one probe by name with its arguments.
    pub fn run(name: &str, rest: &[String]) {
        match name {
            "python-isolated" => python("isolated", rest),
            "python-default" => python("default", rest),
            other => context_foundry::neural::probes::run(other, rest),
        }
        let _ = std::io::Write::flush(&mut std::io::stdout());
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("foundry-embed: the embedding worker targets macOS");
    std::process::exit(78);
}
