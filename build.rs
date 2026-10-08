//! Build-time linking for the two optional workers. The default build emits
//! nothing.
//!
//! 013 T002: with the `learning-worker` feature, make every linked target
//! (the `foundry-learn` worker, the tests and the parity suite) load
//! LibTorch without `DYLD_LIBRARY_PATH`, which SIP strips:
//!
//! * `$LIBTORCH/lib` becomes an rpath (`LIBTORCH` is the variable torch-sys
//!   builds against; it fails the build when unset);
//! * the official LibTorch 2.11.0 macOS arm64 tree names its OpenMP runtime
//!   by an absolute path that does not exist (`libtorch_cpu.dylib` →
//!   `/opt/llvm-openmp/lib/libomp.dylib`; the file ships in `lib/` with that
//!   install name). The binary therefore also loads `@rpath/libomp.dylib`
//!   itself, through a symbol-free text stub, so the runtime is already
//!   loaded under its install name when `libtorch_cpu` asks for it. The
//!   vendor tree is never modified.
//!
//! 009 T004: with the `embed-worker` feature, `foundry-embed` links a static
//! llama.cpp (Metal, embedded shader library) built from the pinned commit;
//! see [`llama`].
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(feature = "embed-worker")]
    llama::link();
    learning_worker();
}

fn learning_worker() {
    println!("cargo:rerun-if-env-changed=LIBTORCH");
    if std::env::var_os("CARGO_FEATURE_LEARNING_WORKER").is_none() {
        return;
    }
    let Some(libtorch) = std::env::var_os("LIBTORCH") else {
        return;
    };
    let lib = std::path::Path::new(&libtorch).join("lib");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos")
        && lib.join("libomp.dylib").exists()
    {
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
        let stub = out.join("libomp-rpath.tbd");
        std::fs::write(
            &stub,
            "--- !tapi-tbd\ntbd-version: 4\ntargets: [ arm64-macos, x86_64-macos ]\n\
             install-name: '@rpath/libomp.dylib'\ncurrent-version: 5\n\
             compatibility-version: 5\n...\n",
        )
        .expect("write the libomp stub");
        println!(
            "cargo:rustc-link-arg=-Wl,-needed_library,{}",
            stub.display()
        );
    }
}

/// 009 T004: the `foundry-embed` link against a static llama.cpp.
///
/// `LLAMA_CPP_DIR` names a llama.cpp checkout whose `HEAD` is exactly
/// [`llama::COMMIT`] with no tracked modification; `LLAMA_BUILD_DIR` (default
/// `$LLAMA_CPP_DIR/build-static`) its CMake build with `BUILD_SHARED_LIBS=OFF`,
/// `GGML_METAL=ON` and `GGML_METAL_EMBED_LIBRARY=ON`. Bindings are generated
/// from that checkout's `llama.h`; the archives, the system C++ library and
/// the Metal, Foundation and Accelerate frameworks are linked into the
/// `foundry-embed` binary only (never the library, `foundry` or the tests),
/// so it needs no llama.cpp or ggml dylib at run time. The checkout's
/// `LICENSE` (which covers the vendored ggml; there is no `ggml/LICENSE` at
/// this commit) is copied into `OUT_DIR` and embedded in the worker, which
/// writes it on `--notices DIR`: packaging needs no checkout.
#[cfg(feature = "embed-worker")]
mod llama {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    /// Must equal `neural::worker_runtime::LLAMA_CPP_COMMIT`; the worker
    /// binary asserts it at compile time from `FOUNDRY_LLAMA_CPP_COMMIT`.
    const COMMIT: &str = "b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea";
    const BIN: &str = "foundry-embed";

    /// Static archives in dependency order (dependents first), each with its
    /// directory relative to the build directory.
    const ARCHIVES: [(&str, &str); 6] = [
        ("src", "llama"),
        ("ggml/src", "ggml"),
        ("ggml/src", "ggml-cpu"),
        ("ggml/src/ggml-metal", "ggml-metal"),
        ("ggml/src/ggml-blas", "ggml-blas"),
        ("ggml/src", "ggml-base"),
    ];

    pub fn link() {
        println!("cargo:rerun-if-env-changed=LLAMA_CPP_DIR");
        println!("cargo:rerun-if-env-changed=LLAMA_BUILD_DIR");
        let Some(source) = std::env::var_os("LLAMA_CPP_DIR").map(PathBuf::from) else {
            panic!(
                "the embed-worker feature needs LLAMA_CPP_DIR (a llama.cpp checkout at {COMMIT})"
            );
        };
        let build = std::env::var_os("LLAMA_BUILD_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| source.join("build-static"));
        check_commit(&source);
        println!("cargo:rustc-env=FOUNDRY_LLAMA_CPP_COMMIT={COMMIT}");
        let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
        let license = source.join("LICENSE");
        println!("cargo:rerun-if-changed={}", license.display());
        std::fs::copy(&license, out.join("llama.cpp-LICENSE"))
            .unwrap_or_else(|e| panic!("copy {}: {e}", license.display()));

        for (dir, name) in ARCHIVES {
            let archive = build.join(dir).join(format!("lib{name}.a"));
            assert!(
                archive.is_file(),
                "missing static archive {} (build llama.cpp with BUILD_SHARED_LIBS=OFF first)",
                archive.display()
            );
            println!("cargo:rerun-if-changed={}", archive.display());
            println!("cargo:rustc-link-arg-bin={BIN}={}", archive.display());
        }
        for framework in ["Metal", "MetalKit", "Foundation", "Accelerate"] {
            println!("cargo:rustc-link-arg-bin={BIN}=-Wl,-framework,{framework}");
        }
        println!("cargo:rustc-link-arg-bin={BIN}=-lc++");

        let include = source.join("include");
        let ggml_include = source.join("ggml/include");
        println!(
            "cargo:rerun-if-changed={}",
            include.join("llama.h").display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            ggml_include.join("ggml.h").display()
        );
        let mut clang_args = vec![
            format!("-I{}", include.display()),
            format!("-I{}", ggml_include.display()),
        ];
        if let Some(sdk) = sdk_path() {
            clang_args.push("-isysroot".into());
            clang_args.push(sdk);
        }
        let bindings = bindgen::Builder::default()
            .rust_edition(bindgen::RustEdition::Edition2024)
            .header_contents("foundry_llama.h", "#include \"llama.h\"\n")
            .clang_args(&clang_args)
            .allowlist_function("llama_.*")
            .allowlist_type("llama_.*")
            .allowlist_type("ggml_log_level")
            .allowlist_type("ggml_type")
            .default_enum_style(bindgen::EnumVariation::Consts)
            .prepend_enum_name(false)
            .layout_tests(true)
            .derive_default(false)
            .generate_comments(false)
            .generate()
            .expect("bindgen over llama.h");
        bindings
            .write_to_file(out.join("llama_bindings.rs"))
            .expect("write the llama.cpp bindings");
    }

    /// Refuse any checkout other than [`COMMIT`] with clean tracked files.
    fn check_commit(source: &Path) {
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(source)
                .args(args)
                .output()
                .unwrap_or_else(|e| panic!("git in {}: {e}", source.display()))
        };
        let head = git(&["rev-parse", "HEAD"]);
        let head = String::from_utf8_lossy(&head.stdout);
        assert_eq!(
            head.trim(),
            COMMIT,
            "{} is not llama.cpp {COMMIT}",
            source.display()
        );
        assert!(
            git(&["diff", "--quiet", "HEAD", "--"]).status.success(),
            "{} has tracked modifications; build from a clean {COMMIT}",
            source.display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            source.join(".git/HEAD").display()
        );
    }

    /// The macOS SDK root, so libclang finds the C headers llama.h includes.
    fn sdk_path() -> Option<String> {
        let output = Command::new("xcrun").arg("--show-sdk-path").output().ok()?;
        if !output.status.success() {
            return None;
        }
        let path = String::from_utf8(output.stdout).ok()?.trim().to_owned();
        (!path.is_empty()).then_some(path)
    }
}
