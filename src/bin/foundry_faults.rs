//! Test-only twin of the `foundry` CLI that arms named fault points from the
//! `FOUNDRY_TEST_FAULT` environment variable before running the same CLI.
//! `required-features` keeps this binary out of every default and release
//! build, so the shipped `foundry` binary never reads that variable.
use context_foundry::fault::{self, GlobalAction};

fn main() {
    arm_from_env();
    context_foundry::cli::main();
}

/// `name=abort[:skip]`, `name=cancel[:skip]`, `name=fail[:skip]` or
/// `name=delay:<millis>[:skip]` (stall the boundary for `<millis>`), separated
/// by `;`.
fn arm_from_env() {
    let Ok(spec) = std::env::var("FOUNDRY_TEST_FAULT") else {
        return;
    };
    for entry in spec.split(';').filter(|e| !e.is_empty()) {
        let Some((name, rest)) = entry.split_once('=') else {
            continue;
        };
        let mut parts = rest.split(':');
        let kind = parts.next().unwrap_or("fail");
        let second = parts.next().and_then(|v| v.parse::<u64>().ok());
        let (millis, skip) = match kind {
            // delay:<millis>[:skip]
            "delay" => (
                second.unwrap_or(0),
                parts
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0) as usize,
            ),
            _ => (0, second.unwrap_or(0) as usize),
        };
        let action = match kind {
            "abort" => GlobalAction::Abort,
            "cancel" => GlobalAction::Cancel,
            "delay" => GlobalAction::Delay(std::time::Duration::from_millis(millis)),
            _ => GlobalAction::Fail(format!("injected failure at {name}")),
        };
        // Process-global: engine calls may run on worker threads.
        fault::arm_global(name, skip, action);
    }
}
