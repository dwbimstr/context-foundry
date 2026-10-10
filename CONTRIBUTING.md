# Contributing to Context Foundry

Thank you for helping. This guide covers how to set up, what a change needs, and how
design decisions are made. Everyone who takes part follows the
[Code of Conduct](CODE_OF_CONDUCT.md). Security problems go through private reporting,
not issues: see [SECURITY](SECURITY.md).

## Ways to contribute

- **Report a bug** with the issue form: the exact command, what you expected, what
  happened, and `foundry status` output for the store. A minimal public or synthetic
  reproduction is the most useful thing you can attach.
- **Improve the docs.** Corrections to the README, `docs/` and the specs are welcome;
  keep claims tied to evidence in [validation](docs/validation.md) or the code.
- **Fix a bug** with a regression test that fails before the fix.
- **Improve a language.** Unit kinds, names, addresses and import keys per language are
  specified in [context-v2 § Languages](specs/001-source-state-recovery/contracts/context-v2.md#languages);
  a mis-parsed construct with a small public fixture is a good first contribution.
- **Propose a feature** with the feature form. Behavior or format changes start as a
  spec discussion (below), before code.

## Development setup

- [rustup](https://rustup.rs) with the stable toolchain and the 1.90.0 toolchain (the
  minimum supported Rust version):
  `rustup toolchain install stable 1.90.0 --component rustfmt --component clippy`.
- A C and C++ compiler (Xcode command-line tools on macOS). The first build fetches
  crates and three grammar forks from GitHub.
- `node` for the documentation link check.
- Only for the optional embedding worker (`--features embed-worker`, macOS with Metal):
  `cmake` and a llama.cpp git checkout at the pinned commit `b9acf138`, named by
  `LLAMA_CPP_DIR`. The frozen learning worker (`--features learning-worker`) needs
  LibTorch 2.13.0 in `LIBTORCH`, paired with `tch`/`torch-sys` 0.26.0.
  Neither is needed for the core. This migration is under validation; the accepted
  historical macOS pairing was 0.24.0/2.11.0. See the
  [compatibility review](docs/review/tch-026.md) before updating a deployed worker.

The historical learning acceptance ran on macOS arm64; CI runs the core checks on
Linux and macOS. The separate optional-worker workflow tests the paired runtime;
its weight-free fixtures do not replace macOS checkpoint and bundle acceptance.

## Build and the fast loop

```sh
cargo build --locked
cargo test --locked --test syntax          # one test file under tests/
cargo test --locked --test cli -- some_test_name
```

Run the focused tests for what you changed while you work. The full suite takes 16–20
minutes, mostly `tests/policy.rs`; run it once before you open the pull request, not
after every edit.

## Full checks

CI runs, on Rust 1.90.0, Linux and macOS ([workflow](.github/workflows/rust.yml)):

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Maintainers run the merge gates once per merge with
[`scripts/gates.sh`](scripts/gates.sh):

```sh
scripts/gates.sh TREE OUTDIR
```

It runs fmt, clippy and the full suite on the default toolchain, then `cargo check` and
clippy under the installed 1.90.0 toolchain, and prints one `GATE <name> exit=<n>` line
per gate, with logs in `OUTDIR`. You may run it yourself; it is not required for a pull
request.

Documentation changes need the link check instead of a Rust build. Run it from the
repository root; it checks every relative link and heading anchor in `README.md`,
`docs/` and `specs/`:

```sh
node scripts/check-links.mjs
```

## Tests

- **Every bug fix has a regression test** that fails without the fix.
- **Test behavior, not wiring or wording.** Assert what a user or an agent observes:
  output, named errors, durable state. Do not pin log text, help text or internal call
  order, and do not add a long benchmark where an acceptance test can decide.
- **Faults.** Crash, interruption and stall tests use named fault points
  ([`src/fault.rs`](src/fault.rs)) compiled only under the non-default `test-faults`
  feature, which `cargo test` enables through a self dev-dependency; environment arming
  lives in the test-only `foundry-faults` binary. Never arm faults from the shipped
  `foundry` binary or a release build: check with `cargo build --locked --release` and
  `strings target/release/foundry` (no `ctxfoundry-fault` or `FOUNDRY_TEST_FAULT`).
  Target boundaries by name and assert the durable state that identifies them, not by
  counting checkpoints.
- Use small original or public fixtures with clear rights, labeled as fixtures.

## Design process

Read the [constitution](.specify/memory/constitution.md) (KISS with the product's
capabilities intact) and the [spec portfolio](specs/README.md). Each numbered spec owns
one user outcome: its behavior, design, tasks and acceptance. The
[modified Spec Kit workflow](.specify/README.md) describes the optional commands and
templates; no stage is mandatory.

- **A small repair amends its existing contract.** It does not get a new numbered spec.
- **Behavior or format changes need a spec discussion first.** Open an issue before
  writing code for anything that changes source identity, durable state, the store or
  search schema, the wire format, the MCP tools or their limits. Name the user failure,
  the simplest alternative, and what state or protocol the change would add.
- Every change to what a search document contains bumps the search schema
  (context-v2 § Index version gate), and older stores then need `repair-index`.

## Pull requests

- Keep each pull request small and about one behavior.
- Describe the user-visible effect and the evidence: the tests you added or ran, and
  any output that shows the change. Say what you did not verify.
- CI must be green.
- Maintainers request a cross-lab review (a reviewer from a different model lab than
  the author) for substantial changes, and may ask for changes before merging.
- Update the docs and the owning spec's status when behavior changes.

## What not to submit

- Private source code, datasets, transcripts, model weights or real credentials, in a
  patch, a fixture or an issue. Neither a filter nor the MIT license establishes the
  right to publish data.
- Benchmarks or measurements without an acceptance question: state the decision a
  number would change before adding it.
- New services, daemons, protocols or databases without a spec discussion.

## Measurement apparatus

The G1 and G2 measurements in [validation](docs/validation.md) use private task sets,
corpora stores and models that live outside Git
([measurement handoff](docs/measurement-handoff.md)). Contributors are not expected
to run them; maintainers do, when a change could move their results.

## Grammar forks

Three tree-sitter grammars resolve from forks pinned by git revision in `Cargo.toml`
`[patch.crates-io]` (Ruby, Perl and Rust, under `github.com/dwbimstr`); the comments
there say what each fork fixes. A grammar change alters what is indexed: it needs
fixture tests and a search schema bump. See [dependencies](docs/dependencies.md).

## License

Context Foundry is [MIT licensed](LICENSE). By submitting a contribution you agree that
it is licensed under the MIT license, the same terms as the project (inbound = outbound).
There is no separate contributor agreement. Dependencies keep their own licenses.
