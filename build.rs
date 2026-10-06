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
//! The default build emits nothing.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
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
