@AGENTS.md

## Where the work runs

For now the ecosystem work is bound to **the sandbox**: the machine that made the
[ecosystem checkpoint](docs/resume-context-ecosystem.md). It holds the checkout at
`/workspace/context-foundry`, the restored evidence under
`/workspace/.context-foundry-tools` and the archive, and has a 4-CPU quota, 32 GiB
of RAM and no swap. Sessions there are driven over Remote Control:
`bash scripts/cloud/sandbox.sh check`, then `start`. Run one heavy build, index or
model job at a time, and keep the existing absolute paths.

## Cloud sessions

These rules apply when `CLAUDE_CODE_REMOTE=true` (Claude Code on the web), the fallback
if the sandbox is unavailable. The environment is defined in
[scripts/cloud](scripts/cloud/setup.sh).

- **Check first.** Run `bash scripts/cloud/doctor.sh` at the start of every session. Do
  not start a build or model run while it reports FAIL.
- **Private evidence.** `bash scripts/cloud/fetch-resume.sh` downloads and verifies the
  resume archive into `/workspace` and prints its `RESTORE.md`; restore by following
  that file. Corpora and stores go back to their original absolute paths as real
  directories, never symlinks, because stores are bound to canonical paths. Nothing
  from the archive, no model and no transcript enters Git.
- **Host limits.** About 4 vCPUs, 16 GB RAM, 30 GB disk, Ubuntu 24.04 x86_64. Run one
  heavy build, index or model job at a time, and never time a model while compiling.
  - The prepared Qwen3-Coder-30B-A3B host pair needs an 18.6 GB model in a sealed
    memfd. It cannot run here, so report it as blocked; do not change the model
    without the owner.
  - The SIMD revision-5 Gemma function needs `avx512_bf16` (the doctor reports it).
    Without it, stop and report; do not switch to another embedding function, because
    its cache keys and vectors differ.
  - Sessions run as root. Two `tests/gateway.rs` fault tests,
    `a_failed_agent_creation_still_cleans_up_the_created_profile` and
    `a_failed_models_write_still_cleans_up_and_does_not_block_the_next_launch`, simulate
    a failure with read-only permissions, which root bypasses, so they fail here. Rerun
    the built test binary as `nobody`, with a readable copy of the CA bundle in
    `SSL_CERT_FILE`; do not change or skip the tests.
- **Long jobs.** Start them with `run_in_background` and an explicit `timeout` (up to
  4 hours). Write all output under
  `/workspace/.context-foundry-tools/validation-runs/<run>/`. Keep the session active
  until they end by checking their logs, for example `sleep 540; tail -n 20 LOG`: an
  idle session's VM pauses, and that interrupts the job. A run interrupted by a pause,
  restart or budget is INVALID.
- **GitHub.** The proxy rejects GraphQL, so `gh pr` and `gh issue` fail; use REST
  through `gh api`.
- **Git.** Work on `experimental/context-ecosystem`. Never push `main`, tags or
  releases. Run `node scripts/check-links.mjs` before committing Markdown.

## GitHub Actions is unavailable

Owner decision, 2026-10-12: GitHub Actions is out of budget for the foreseeable
future. Treat it as unavailable, not broken. This applies everywhere, not only to
cloud sessions.

- Do not dispatch, re-run or wait on any workflow, and do not schedule check-ins
  for one. End commit subjects with `[skip ci]` so pushes start no runs.
- Leave `.github/workflows` intact.
- **Rust CI is replaced by local gates.** Run `scripts/gates.sh . OUTDIR` once per
  merge and record a receipt in [validation](docs/validation.md). The receipt gives the
  commit, the toolchains, each `GATE` line, the test totals and the SHA-256 of each
  log. Cite it wherever a CI run URL used to go, and name any gate not run.
- **Native macOS acceptance has no Linux substitute.** Run
  `scripts/native-macos-acceptance.py` on a Mac. Otherwise record it as an explicit
  gap; never mark it pending, and never let it block a merge.
- Write new acceptance criteria as local checks from the start. If an existing
  criterion names a workflow run, amend it in place to say what local check now
  satisfies it and why.
