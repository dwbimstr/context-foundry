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
//! llama.cpp (Metal, embedded shader library) that this script builds from
//! the pinned commit's tree with fixed flags; see [`llama`].
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

/// 009 T004: the `foundry-embed` link against a static llama.cpp built here.
///
/// `LLAMA_CPP_DIR` names a llama.cpp git repository that contains
/// [`llama::COMMIT`]. Nothing of its working tree, its index or any earlier
/// build is used: the blobs of that commit's library tree ([`llama::TREE`])
/// are read from the object store (`git ls-tree` and `git cat-file`, replace
/// refs off, so no checkout state, attribute or filter applies) into
/// `OUT_DIR`, and CMake builds them there into static archives with the
/// fixed configuration [`llama::CMAKE_FLAGS`] (static, Metal with the
/// embedded shader library, Accelerate BLAS, no OpenMP, no host-specific CPU
/// tuning), the system compilers and a cleared environment. Each run of this
/// script starts from an empty source and build directory. Bindings are
/// generated from that tree's `llama.h`; the archives, the system C++ library
/// and the Metal, Foundation and Accelerate frameworks are linked into the
/// `foundry-embed` binary only (never the library, `foundry` or the tests),
/// so it needs no llama.cpp or ggml dylib at run time. The tree's `LICENSE`
/// (which covers the vendored ggml; there is no `ggml/LICENSE` at this
/// commit) is copied into `OUT_DIR` and embedded in the worker, which writes
/// it on `--notices DIR`: packaging needs no checkout.
#[cfg(feature = "embed-worker")]
mod llama {
    use std::ffi::OsString;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    /// Must equal `neural::worker_runtime::LLAMA_CPP_COMMIT`; the worker
    /// binary asserts it at compile time from `FOUNDRY_LLAMA_CPP_COMMIT`.
    const COMMIT: &str = "b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea";
    const BIN: &str = "foundry-embed";

    /// The parts of the commit's tree the library build reads.
    const TREE: [&str; 7] = [
        "CMakeLists.txt",
        "LICENSE",
        "cmake",
        "ggml",
        "include",
        "src",
        "vendor",
    ];

    /// The fixed CMake configuration: static archives, Metal with its shader
    /// library embedded, Apple Accelerate BLAS, no OpenMP (so no OpenMP
    /// runtime notice), CPU kernels for the architecture's baseline rather
    /// than the build host, no compiler cache, and the library alone. The
    /// build commit and number are fixed too, so nothing outside the tree
    /// reaches the archives.
    const CMAKE_FLAGS: [&str; 21] = [
        "-DCMAKE_BUILD_TYPE=Release",
        "-DCMAKE_C_COMPILER=/usr/bin/cc",
        "-DCMAKE_CXX_COMPILER=/usr/bin/c++",
        "-DBUILD_SHARED_LIBS=OFF",
        "-DGGML_METAL=ON",
        "-DGGML_METAL_EMBED_LIBRARY=ON",
        "-DGGML_BLAS=ON",
        "-DGGML_BLAS_VENDOR=Apple",
        "-DGGML_ACCELERATE=ON",
        "-DGGML_NATIVE=OFF",
        "-DGGML_OPENMP=OFF",
        "-DGGML_BACKEND_DL=OFF",
        "-DGGML_CCACHE=OFF",
        "-DLLAMA_BUILD_COMMON=OFF",
        "-DLLAMA_BUILD_TESTS=OFF",
        "-DLLAMA_BUILD_TOOLS=OFF",
        "-DLLAMA_BUILD_EXAMPLES=OFF",
        "-DLLAMA_BUILD_SERVER=OFF",
        "-DLLAMA_BUILD_APP=OFF",
        "-DLLAMA_OPENSSL=OFF",
        "-DLLAMA_BUILD_NUMBER=0",
    ];

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
        for name in [
            "LLAMA_CPP_DIR",
            "MACOSX_DEPLOYMENT_TARGET",
            "DEVELOPER_DIR",
            "SDKROOT",
        ] {
            println!("cargo:rerun-if-env-changed={name}");
        }
        let Some(repository) = std::env::var_os("LLAMA_CPP_DIR").map(PathBuf::from) else {
            panic!(
                "the embed-worker feature needs LLAMA_CPP_DIR (a llama.cpp git repository \
                 containing {COMMIT})"
            );
        };
        let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
        let source = out.join("llama.cpp");
        let build = out.join("llama.cpp-build");
        for dir in [&source, &build] {
            if dir.exists() {
                std::fs::remove_dir_all(dir)
                    .unwrap_or_else(|e| panic!("remove {}: {e}", dir.display()));
            }
        }
        export(&repository, &source);
        compile(&source, &build, &out);
        println!("cargo:rustc-env=FOUNDRY_LLAMA_CPP_COMMIT={COMMIT}");
        let license = source.join("LICENSE");
        std::fs::copy(&license, out.join("llama.cpp-LICENSE"))
            .unwrap_or_else(|e| panic!("copy {}: {e}", license.display()));

        for (dir, name) in ARCHIVES {
            let archive = build.join(dir).join(format!("lib{name}.a"));
            assert!(
                archive.is_file(),
                "the llama.cpp build produced no {}",
                archive.display()
            );
            println!("cargo:rustc-link-arg-bin={BIN}={}", archive.display());
        }
        for framework in ["Metal", "MetalKit", "Foundation", "Accelerate"] {
            println!("cargo:rustc-link-arg-bin={BIN}=-Wl,-framework,{framework}");
        }
        println!("cargo:rustc-link-arg-bin={BIN}=-lc++");

        let mut clang_args = vec![
            format!("-I{}", source.join("include").display()),
            format!("-I{}", source.join("ggml/include").display()),
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

    /// `git` in `repository` with replace refs off, so every object read is
    /// the one its id names.
    fn git(repository: &Path) -> Command {
        let mut command = Command::new("git");
        command
            .arg("--no-replace-objects")
            .arg("-C")
            .arg(repository);
        command
    }

    /// Write the blobs of [`TREE`] at [`COMMIT`] under `dest`, straight from
    /// the object store: no working tree, index, attribute or filter.
    fn export(repository: &Path, dest: &Path) {
        let listing = git(repository)
            .args(["ls-tree", "-r", "-z", "--full-tree", COMMIT, "--"])
            .args(TREE)
            .output()
            .unwrap_or_else(|e| panic!("git in {}: {e}", repository.display()));
        assert!(
            listing.status.success(),
            "{} does not contain llama.cpp {COMMIT}: {}",
            repository.display(),
            String::from_utf8_lossy(&listing.stderr).trim()
        );
        let mut entries = Vec::new();
        for record in listing.stdout.split(|&b| b == 0).filter(|r| !r.is_empty()) {
            let record = std::str::from_utf8(record).expect("a UTF-8 tree entry");
            let (meta, path) = record.split_once('\t').expect("a tree entry");
            let fields: Vec<&str> = meta.split(' ').collect();
            assert!(
                matches!(fields.as_slice(), ["100644" | "100755", "blob", _]),
                "{path}: unexpected {meta} in llama.cpp {COMMIT}"
            );
            entries.push((fields[0] == "100755", fields[2].to_owned(), path.to_owned()));
        }
        for part in TREE {
            assert!(
                entries
                    .iter()
                    .any(|(_, _, path)| path == part || path.starts_with(&format!("{part}/"))),
                "llama.cpp {COMMIT} has no {part}"
            );
        }
        let mut child = git(repository)
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("git cat-file in {}: {e}", repository.display()));
        let mut stdin = child.stdin.take().expect("git cat-file stdin");
        let requests: String = entries.iter().map(|(_, id, _)| format!("{id}\n")).collect();
        let writer = std::thread::spawn(move || stdin.write_all(requests.as_bytes()));
        let mut objects = BufReader::new(child.stdout.take().expect("git cat-file stdout"));
        for (executable, id, path) in &entries {
            let mut header = String::new();
            objects
                .read_line(&mut header)
                .expect("a git cat-file header");
            let fields: Vec<&str> = header.split_whitespace().collect();
            let size: usize = match fields.as_slice() {
                [answered, "blob", size] if answered == id => size.parse().expect("a blob size"),
                _ => panic!("git cat-file answered {header:?} for {path}"),
            };
            let mut content = vec![0u8; size + 1];
            objects.read_exact(&mut content).expect("a blob");
            assert_eq!(content.pop(), Some(b'\n'), "git cat-file framing");
            let file = dest.join(path);
            std::fs::create_dir_all(file.parent().expect("a parent directory"))
                .unwrap_or_else(|e| panic!("create {}: {e}", file.display()));
            std::fs::write(&file, &content)
                .unwrap_or_else(|e| panic!("write {}: {e}", file.display()));
            if *executable {
                std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755))
                    .unwrap_or_else(|e| panic!("chmod {}: {e}", file.display()));
            }
        }
        writer
            .join()
            .expect("the object request writer")
            .expect("write the object requests");
        assert!(
            child.wait().expect("git cat-file").success(),
            "git cat-file failed in {}",
            repository.display()
        );
    }

    /// Configure and build the library from `source` into `build` with
    /// [`CMAKE_FLAGS`], under a cleared environment (only the system `PATH`,
    /// the Xcode selection, and a git ceiling at `out` so the build stamps
    /// no commit of an enclosing repository). CMake's own output goes to
    /// this script's stderr, never to the stdout cargo reads.
    fn compile(source: &Path, build: &Path, out: &Path) {
        let cmake = std::env::var_os("PATH")
            .and_then(|path| {
                std::env::split_paths(&path)
                    .map(|dir| dir.join("cmake"))
                    .find(|candidate| candidate.is_file())
            })
            .expect("the embed-worker feature builds llama.cpp with CMake; no cmake on PATH");
        let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
            Ok("aarch64") => "arm64",
            Ok("x86_64") => "x86_64",
            other => panic!("foundry-embed has no llama.cpp build for target arch {other:?}"),
        };
        // The deployment target rustc links the worker for.
        let deployment =
            std::env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "11.0".to_owned());
        let jobs = std::env::var("NUM_JOBS").unwrap_or_else(|_| "1".to_owned());
        let run = |args: Vec<OsString>, what: &str| {
            let mut command = Command::new(&cmake);
            command
                .env_clear()
                .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
                .env("GIT_CEILING_DIRECTORIES", out)
                .args(&args)
                .stdout(std::io::stderr());
            for name in ["DEVELOPER_DIR", "SDKROOT"] {
                if let Some(value) = std::env::var_os(name) {
                    command.env(name, value);
                }
            }
            let status = command
                .status()
                .unwrap_or_else(|e| panic!("{}: {e}", cmake.display()));
            assert!(status.success(), "llama.cpp {what} failed ({status})");
        };
        let mut configure: Vec<OsString> = vec![
            "-S".into(),
            source.into(),
            "-B".into(),
            build.into(),
            "-G".into(),
            "Unix Makefiles".into(),
            format!("-DCMAKE_OSX_ARCHITECTURES={arch}").into(),
            format!("-DCMAKE_OSX_DEPLOYMENT_TARGET={deployment}").into(),
            format!("-DLLAMA_BUILD_COMMIT={}", &COMMIT[..8]).into(),
        ];
        configure.extend(CMAKE_FLAGS.iter().map(OsString::from));
        run(configure, "configuration");
        run(
            vec![
                "--build".into(),
                build.into(),
                "--target".into(),
                "llama".into(),
                "--parallel".into(),
                jobs.into(),
            ],
            "build",
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
