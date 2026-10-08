//! 009 test worker: the shared worker runtime with a deterministic fake
//! model and named fault hooks. Built only with the non-default
//! `test-faults` feature; the real worker never reads these flags.
//!
// Deterministic vectors come from `worker_runtime::deterministic_vector`,
// keyed only by the ID sequence. The `--shim` mode serves the owner-death
// tests: this process becomes the owner of one child copy of itself while
// a separate `sleep` process holds the liveness pipe open, so killing this
// process exercises the kqueue owner-exit path alone. `--notices DIR` is the
// real worker's packaging flag with `worker_runtime::FAKE_NOTICE` as text.
#[cfg(target_os = "macos")]
fn main() {
    use context_foundry::neural::worker_runtime::{self, FAKE_VOCAB, Hooks, WorkerArgs};
    use std::io::Write;

    let argv: Vec<String> = std::env::args().collect();
    if argv.iter().any(|arg| arg == "--shim") {
        worker_runtime::run_shim(argv[1..].to_vec());
    }
    if let [_, flag, dir] = argv.as_slice()
        && flag == "--notices"
    {
        let code =
            worker_runtime::write_notices(std::path::Path::new(dir), worker_runtime::FAKE_NOTICE);
        let _ = std::io::stdout().flush();
        std::process::exit(code);
    }
    let (mut args, rest) = match WorkerArgs::parse(argv.into_iter().skip(1)) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("foundry-embed-fake: {message}");
            std::process::exit(64);
        }
    };
    let hooks = match Hooks::parse(&rest) {
        Ok(hooks) => hooks,
        Err(message) => {
            eprintln!("foundry-embed-fake: {message}");
            std::process::exit(64);
        }
    };
    if hooks.ignore_term {
        unsafe {
            libc::signal(libc::SIGTERM, libc::SIG_IGN);
        }
    }
    if let Some(path) = &hooks.phase_file {
        worker_runtime::set_phase_file(path);
    }
    args.hooks = hooks;
    let code = worker_runtime::serve(args, |engine| {
        let hooks = engine.args.hooks.clone();
        let expected = &engine.args.expected;
        if *expected != worker_runtime::fake_descriptor_with(expected.dimensions) {
            engine.fail_load(
                "descriptor_mismatch",
                "the fake worker serves only its own descriptor (at any supported dimension)",
            );
            return 1;
        }
        let dims = expected.dims();
        if let Some(path) = &hooks.pid_file
            && let Err(e) = std::fs::write(path, std::process::id().to_string())
        {
            eprintln!("cannot write the PID file {path}: {e}");
            return 1;
        }
        if hooks.stray_stdout {
            // Exactly what a chatty library does: plain text on fd 1. The
            // runtime points fd 1 at stderr, so this must never reach the
            // frame channel.
            println!("stray stdout line from a library");
            let _ = std::io::stdout().flush();
        }
        if hooks.stderr_flood > 0 {
            let chunk = [b'f'; 4096];
            let mut written = 0;
            let mut stderr = std::io::stderr();
            while written < hooks.stderr_flood {
                let want = (hooks.stderr_flood - written).min(chunk.len());
                if stderr.write_all(&chunk[..want]).is_err() {
                    break;
                }
                written += want;
            }
            let _ = stderr.flush();
        }
        // Hook allocations are held for the life of the process.
        let mut held: Vec<Vec<u8>> = Vec::new();
        if hooks.alloc_mb > 0 {
            held.push(worker_runtime::allocate_touching(hooks.alloc_mb));
            std::hint::black_box(&held);
        }
        if hooks.load_ms > 0 {
            worker_runtime::mark_phase("load");
            worker_runtime::sleep_ms(hooks.load_ms);
        }
        if !engine.send_ready(FAKE_VOCAB) {
            return 1;
        }
        while let Some(job) = engine.next_job() {
            // The admitted call has entered the worker's compute phase.
            worker_runtime::mark_phase("call");
            if hooks.die_in_call {
                // SAFETY: kill(2) on this very process; nothing runs after.
                unsafe {
                    libc::kill(libc::getpid(), libc::SIGKILL);
                }
            }
            if hooks.alloc_call_mb > 0 {
                held.push(worker_runtime::allocate_touching(hooks.alloc_call_mb));
                std::hint::black_box(&held);
            }
            if hooks.slow_ms > 0 {
                worker_runtime::sleep_ms(hooks.slow_ms);
            }
            if let Some(path) = &hooks.hold_file {
                while std::path::Path::new(path).exists() {
                    worker_runtime::sleep_ms(5);
                }
            }
            let vectors: Vec<Vec<f32>> = job
                .inputs
                .iter()
                .map(|input| worker_runtime::deterministic_vector(&input.ids, dims))
                .collect();
            if !engine.finish_job(job, vectors) {
                return 1;
            }
        }
        0
    });
    std::process::exit(code);
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("foundry-embed-fake: the worker supervisor targets macOS");
    std::process::exit(78);
}
