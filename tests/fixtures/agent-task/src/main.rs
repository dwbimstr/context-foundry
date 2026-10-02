//! Parser under edit. TASK: make `parse_record` trim whitespace on both
//! sides of `=` and reject an empty key, while preserving the
//! `Option<(&str, &str)>` interface exactly.

mod records;

fn main() {
    let argument = std::env::args().nth(1);
    match argument.as_deref() {
        Some("check") => check(),
        Some("run") => {
            for input in ["mode=local", " mode = local ", "mode=a=b"] {
                println!("{input:?} -> {:?}", records::parse_record(input));
            }
        }
        _ => {
            eprintln!("usage: agent-task-fixture [check|run]");
            std::process::exit(2);
        }
    }
}

/// Checker for the T003 task. Exit code 0 is the only pass signal.
fn check() {
    let cases: &[(&str, Option<(&str, &str)>)] = &[
        (" mode = local ", Some(("mode", "local"))),
        (" = x", None),
        ("mode", None),
        ("mode=a=b", Some(("mode", "a=b"))),
    ];
    let mut failures = 0;
    for (input, expected) in cases {
        let actual = records::parse_record(input);
        let ok = actual == *expected;
        println!(
            "{:<14} {:?} => {:?} {}",
            input,
            expected,
            actual,
            if ok { "ok" } else { "FAIL" }
        );
        if !ok {
            failures += 1;
        }
    }
    if failures > 0 {
        println!("{failures} case(s) failing");
        std::process::exit(1);
    }
    println!("all cases pass");
}
