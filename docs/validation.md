# First-slice validation

Local validation on 2026-09-28, macOS, Rust 1.97.1. No predecessor build, benchmark
or live-store experiment was part of this validation.

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Pass |
| `cargo clippy --locked --all-targets -- -D warnings` | Pass |
| `cargo test --locked` | Pass: 10 integration tests |
| `cargo build --locked --release` | Pass |
| Release CLI help → index → graph import → context on the bundled fixture | Pass: 2 sources, 1 manual edge, 439 context-text tokens under a 1024-token budget |
| Public documentation relative links | Pass |

Tests cover source replacement/deletion and stale-search rejection, reopen with
pending index work, rebuilding a missing search index across another restart,
graph producer isolation/hash validity/traversal bounds, UTF-8 token-budget
accounting, workspace binding, feedback consent/splitting, the CLI lifecycle, and
Laya response parsing/fallback through a real local HTTP transport fixture.

The HTTP fixture is not a model. No Laya inference, fine-tuning, candidate promotion,
provider billing or task-quality experiment ran. No million-file scale, power-loss
fault injection or cross-platform result is claimed. The configured Linux/macOS
CI and Rust 1.90 minimum-toolchain jobs have not run on a hosted runner yet.

Release executable SHA-256 for this local build:
`5589bc3018c4149e9bd62804a0eab13e27a5d91ba966fefea54ed69efc16da78`.
This is a local build fingerprint, not a signed distribution artifact.

## Subsequent feasibility, 2026-09-29

[Bounded feasibility results](review/feasibility.md) add actual sandboxed model,
gradient, vector-index and Rust 1.90 MCP probes. They live in a separate scratch-probe
crate and do not change the root product or retroactively complete its proposed specs.
No production test suite rerun was needed for these documentation/probe-only edits.
