@AGENTS.md

## Where the work runs

Owner decision, 2026-11-06: "Mac only, drop sandbox". The work runs on **the owner's
Mac**, which reports an Apple M5 Max with 64 GiB of RAM (receipts before 2026-11-06
name an 18 GiB Apple M3 Pro). The checkout is `/Users/satishlomte/VSC_DEV/context-foundry`;
the private corpora, datasets and stores stay at their existing absolute paths under
`/Users/satishlomte/VSC_DEV`. Keep stores, profiles and worker bundles outside
`/private/tmp`, which macOS cleans ([handoff](docs/measurement-handoff.md#inputs)).
Run one heavy build, index or model job at a time. Current status is in the
[ecosystem handoff](docs/resume-context-ecosystem.md).

Owner decision, 2026-11-06: "Declare both lost". The 2026-10-11 checkpoint sandbox and
its private resume archive are given up; do not try to reach either. The frozen
measurement sets and the pilot evidence have no copy off the Mac until the owner copies
the [encrypted backups](docs/resume.md#encrypted-backups) off it; other private
datasets have none. Any job that needs Linux, such as one using the SIMD revision-5
function on `avx512_bf16`, stays parked until the owner provides a Linux host; F2's
sealed-memfd runner is retired (below).

## Cloud sessions

These rules apply when `CLAUDE_CODE_REMOTE=true` (Claude Code on the web), the fallback
when the owner's Mac is unavailable. The environment is defined in
[scripts/cloud](scripts/cloud/setup.sh). A cloud session has no private evidence, so
G1, G2 and pilot runs stay on the Mac.

- **Check first.** Run `bash scripts/cloud/doctor.sh` at the start of every session. Do
  not start a build or model run while it reports FAIL.
- **Environment variables.** `FAIL environment` from the doctor means the session did
  not get [environment.env](scripts/cloud/environment.env): test builds then keep
  debuginfo and fill the disk, and the learning tests cannot find their tokenizer. Ask
  the owner to paste the file into the environment's variables (environment menu,
  Edit); only new sessions get them. Until then, run
  `set -a; . scripts/cloud/environment.env; set +a` in each shell, and run the gates as
  `bash scripts/cloud/gates.sh OUTDIR`, which applies the file itself.
- **Private evidence.** The resume archive is lost (owner, 2026-11-06), so do not run
  `scripts/cloud/fetch-resume.sh`. Anything private that is restored goes back to its
  original absolute path as a real directory, never a symlink, because stores are bound
  to canonical paths. No private dataset, model or transcript enters Git.
- **Host limits.** About 4 vCPUs, 16 GB RAM, 30 GB disk, Ubuntu 24.04 x86_64. Run one
  heavy build, index or model job at a time, and never time a model while compiling.
  - The Qwen3-Coder-30B-A3B host pair (F2), which needed an 18.6 GB model in a sealed
    memfd and a runner from the lost archive, is retired (owner, 2026-11-06, "Retire
    F2; F3 to T006"). Do not attempt it, and do not change the model without the owner.
  - The SIMD revision-5 Gemma function needs `avx512_bf16` (the doctor reports it).
    Any revision-5 job stays parked until the owner provides a Linux host; F10 and F11
    are retired (owner, 2026-11-06, "Accept the Mac rust run"). Without `avx512_bf16`,
    stop and report; do not switch to another embedding function, because its cache
    keys and vectors differ.
  - The VM can restart onto a different CPU during a session. Re-run the doctor before
    every model run; a session that started with `avx512_bf16` may lose it.
  - The proxy refuses GitHub archives of repositories outside the session
    (`codeload.github.com` 403), including the pinned Rust and Go corpora. Report it;
    do not route around it.
  - Sessions run as root. Two `tests/gateway.rs` fault tests,
    `a_failed_agent_creation_still_cleans_up_the_created_profile` and
    `a_failed_models_write_still_cleans_up_and_does_not_block_the_next_launch`, simulate
    a failure with read-only permissions, which root bypasses, so they fail here. Rerun
    the built test binary as `nobody`, with `SSL_CERT_FILE` pointing at a copy of the CA
    bundle that `nobody` can read. `/tmp/claude-0` is private, so put the copy elsewhere,
    for example `/tmp/cf-nobody/ca.crt`. Do not change or skip the tests.
- **Long jobs.** Start them with `run_in_background` and an explicit `timeout` (up to
  4 hours). Write all output under
  `/workspace/.context-foundry-tools/validation-runs/<run>/`. Keep the session active
  until they end by checking their logs, for example `sleep 540; tail -n 20 LOG`: an
  idle session's VM pauses, and that interrupts the job. A run interrupted by a pause,
  restart or budget is INVALID.
- **GitHub.** The proxy rejects GraphQL, so `gh pr` and `gh issue` fail; use REST
  through `gh api`.
- **Git.** Work on `experimental/context-ecosystem`. Never push tags or releases, and
  never push `main` directly: `main` receives the experimental line only through an
  owner-approved pull request merged with a merge commit (first done 2026-10-12). Run
  `node scripts/check-links.mjs` before committing Markdown.

## Independent review is not a gate

Owner decision, 2026-11-05: "no independent reviews needed - go complete and remove
the gate of independent reviews". Cross-lab or other independent review is not
required before acceptance, merge or any contract gate, and no work waits for one.
Review proportionally in-session, fix what survives, label self-review honestly
(AGENTS.md) and check consequential evidence yourself. Request an independent review
only when the owner asks for one.

## GitHub Actions is unavailable

Owner decision, 2026-10-12: GitHub Actions is out of budget for the foreseeable
future. Treat it as unavailable, not broken. This applies everywhere, not only to
cloud sessions.

- Do not dispatch, re-run or wait on any workflow, and do not schedule check-ins
  for one. End commit subjects with `[skip ci]` so pushes start no runs.
- Leave `.github/workflows` intact. Owner decision, 2026-11-06: "Drop the historical
  re-run". When the owner says Actions is back, [F1](FOLLOWUP.md) needs one CI run on
  the then-current heads plus its two workflow action bumps, not a re-run of every
  merged head.
- **Rust CI is replaced by local gates.** Run `scripts/gates.sh . OUTDIR` once per
  merge (in a cloud session, `bash scripts/cloud/gates.sh OUTDIR`) and record a receipt
  in [validation](docs/validation.md). The receipt gives the
  commit, the toolchains, each `GATE` line, the test totals and the SHA-256 of each
  log. Cite it wherever a CI run URL used to go, and name any gate not run.
- **Native macOS acceptance has no Linux substitute.** Run
  `scripts/native-macos-acceptance.py` on a Mac. Otherwise record it as an explicit
  gap; never mark it pending, and never let it block a merge. Owner decision,
  2026-11-06: "Once now, unsigned". F7 is unparked for one run on the owner's Mac
  with ad-hoc signing. That run passed on 2026-11-10
  ([receipt](docs/validation.md#native-macos-acceptance--2026-11-10)). The script must
  run again on the signed release candidate.
- Write new acceptance criteria as local checks from the start. If an existing
  criterion names a workflow run, amend it in place to say what local check now
  satisfies it and why.
