# Experimental ecosystem checkpoint — 2026-10-11

The owner requested a pause, a portable handoff and continuation in a new context.
**This is a checkpoint, not completion of the six acceptance gates.** No new model
experiment was started after that request. Start here, then read `AGENTS.md`, the
constitution, [validation](validation.md#realistic-acceptance-continuation--2026-10-11)
and the [current inventory](context-ecosystem-plan.md#pending-work-inventory--2026-10-10).
Use the strongest available reasoning/coding model and inspect actual artifacts.

## Code and working state

Repository: `/workspace/context-foundry`; branch: `experimental/context-ecosystem`.
The tested implementation checkpoint is `6ac60d8`; the subsequent documentation
checkpoint is branch HEAD. All intended repository changes are committed. Relevant
commits, in order:

- `bc10986`: held-descriptor Mac model loading and native lifecycle fixes.
- `f75f14c`: independent native CPU/model checks and 128-file calibration.
- `8ef3acc`: bounded hierarchy resume plans and structural-fragment roles.
- `c2337f7`: bounded full-corpus native driver; this driver has **not run** at scale.
- `e7a1b59`: foreground-release notification, shutdown and watcher restart handling.
- `6ac60d8`: current owner scopes and coverage-independent experimental interleaving.

Preserve file-first hierarchy publication, exact-input cache identity, zero-call
unchanged replay, freshness-checked router/caller/test evidence, coarse excerpts,
acronym matching and CLI worker reaping. Defaults remain unchanged. Hierarchy/body
role metadata uses `kinds2`; overview/address recipes retain `kinds1`.

## Actual acceptance state

| Gate | Verified now | Still required |
| --- | --- | --- |
| Retrieval | Complete real hierarchy: 274 admitted sources, 13,822 vectors, 14,453 locations. Complete original, role-only and owner-scope arms deliver 1/8; coverage-independent interleaving restores 2/8. Lexical is 2/8. | Effective general repair and independent acceptance. All eight stay in the denominator; one historical source is excluded by unchanged private-key policy, so admitted ceiling is 7/8. Do not bypass admission or relabel outlines as body evidence. |
| Host tasks | Body reads and native tool transport work; latest Qwen3-4B pair still fails both unchanged external oracles. | Run the prepared stronger-model pair, obtain correct task completion, then assess economics. |
| Representative scale | Exact overview fitting covers 67,663 admitted Rust/Go sources; 60,735–65,959 distinct inputs / 14.65–15.86 million tokens. | Full real inference, audit/replay, serving and changed-input verification. No complete 67,663-source model run exists. |
| Economics | Failed attempts and unknown usage are retained. Selected history totals 71 requests, including the latest 13; 66 complete and five unknown. | Demonstrated completion benefit with bootstrap, acquisition, retrieval, maintenance and failed attempts included. Smaller failed prompts are not a benefit. |
| Evidence | Bounded source call postings, conservative Python aliases, router dependency proofs and revision-bound execution snapshots; eight real framework/execution checks pass. | Broader collectors, cross-language resolution and realistic task outcomes. Unsupported/dynamic cases remain explicitly incomplete. |
| Mac/GPU | Real Metal embeddings, 8,192-token input, owned generation, cleanup and CPU/Metal parity pass. Actual 128-file calibration and zero-call replay pass. | Native CPU-pressure acceptance, physical-device performance/global GPU pressure, combined residency and signed installed-package acceptance. |

The complete small-repository preparation accounts for 3,608,991 tokens and
2,710,537 ms of preparation (2,413,715 ms provider time). Unchanged replay takes
3,977 ms with zero calls, tokens, partitioning and worker acquisition. Every
role/scope/interleave comparison preserves all current input keys and spans.

Latest focused checks: 252 library + 93 semantic-retrieval + two attribution + four
watcher tests pass, along with all-target Clippy, formatting and documentation links.
The unchanged 6,000-file Linux watcher test passes in the 29.89-second four-test run.
These checks do not close retrieval, host-task or scale acceptance.

[Native run 38176235770](https://github.com/dwbimstr/context-foundry/actions/runs/38176235770)
at `f75f14c` uses macOS 15 arm64 / Apple Paravirtual Metal. Five matched CPU/Metal
inputs, including 8,192 tokens, have minimum cosine 0.9999949405. Owned generation
records real GPU allocation and initial/change/restart/revert reuse. The overall job
**fails** CPU-pressure acceptance: its initial idle threshold never reaches 30%.
The cause is unresolved, not proven environmental; no threshold was relaxed.
The 128-file sample needs 744.804 provider seconds for 30,726 tokens (41.254 tokens/s).
Do not launch the proposed full Mac batch-one run within a two/three-hour budget.

[Default Rust CI 38178644843](https://github.com/dwbimstr/context-foundry/actions/runs/38178644843)
at `e7a1b59` **passed** on both Ubuntu and macOS (completed 22:32:49 UTC), including the
Mac watcher test that failed at 5,806/6,000 files in the earlier `bc10986` run. That is
one passing Mac run, not a repeated measurement. It does not include `6ac60d8`;
[CI 38205296934](https://github.com/dwbimstr/context-foundry/actions/runs/38205296934) at `1b5f0b1`
covers that code and passes on Ubuntu (1,147 tests) and macOS (1,248 tests), watcher included.

The native CPU-pressure test passed in native runs at `4dd9da9` and `4dab8e4`, with idle
at 9%. It failed at `bc10986` and `f75f14c`, with idle at 84–100%. All four used the same
runner image and stage order, and the commits between them touch neither the sampler
nor the test. Native run 38205301244 records process snapshots: Spotlight indexing
the job's own build output occupies the host (idle 0–2%). The workflow now stops
Spotlight on the ephemeral runner before building, and
[native run 38207948408](https://github.com/dwbimstr/context-foundry/actions/runs/38207948408)
passes the whole native job at unchanged thresholds (idle 2%, recovered 27%). See
[validation](validation.md#realistic-acceptance-continuation--2026-10-11).

## Evidence and portable package

Set `V11=/workspace/.context-foundry-tools/validation-runs/realistic-delivery-v11`.
The private `V11/resume-capsule/` holds handoff scripts, patches, hashes, source
snapshots and sealed evaluation data. None of these unrun patches is silently applied.
`preserved-files.json` identifies their exact bytes. Important evidence directories:

- `foundry-hierarchy-complete-01/`: original complete baseline, index and frozen binaries.
- `foundry-hierarchy-roleonly-01/`, `foundry-hierarchy-ownerscope-01/`,
  `foundry-hierarchy-fusion-01/`: separate immutable comparisons and attribution traces.
- `native-macos-38176235770/`: raw native artifacts, numerical comparison and assessment.
- `rust-ci-38178644843/`: pending CI snapshot; fetch final results on resume.
- `qwen3-body-reads-pair-2048/`: latest negative host pair and accounting.
- `evidence-framework-checks/report.json`: eight actual framework/execution checks.

The portable `/workspace/context-foundry-resume-20261011.tar.gz` preserves the v11
receipts/stores/binaries, handoff work, source corpora/manifests, selected historical
host receipts and an offline Git bundle. Its adjacent `.sha256` file verifies the
archive. Read `RESTORE.md` inside before extraction. Public model weights, general
build caches and installed OMP/Bun/Rust dependencies are excluded. Restore original
absolute corpus paths to preserve source/store identity; do not casually rewrite
profiles or claim cache equivalence across different worker descriptors/hardware.
The archive is private handoff material, not a Git/release artifact.

## Where to continue

For now, continue on the sandbox that made this checkpoint. It already holds the
checkout, the restored evidence and the archive, and it has 32 GiB of RAM. Drive it
through Claude Code Remote Control, which needs a claude.ai Pro/Max login on the
sandbox:

1. Run `git pull --ff-only` in `/workspace/context-foundry`.
2. Run `bash scripts/cloud/sandbox.sh check`. It reports PASS/FAIL for the Claude CLI,
   the login, network access to Anthropic, tmux, the repository and the evidence.
3. Run `bash scripts/cloud/sandbox.sh start`. It starts one Remote Control session in
   tmux. Answer the trust and enable prompts once in `tmux attach -t cf-remote-control`,
   then open the session at claude.ai/code.

If the sandbox is unavailable, use a Claude Code cloud environment instead. It is
versioned in `scripts/cloud`:

| File | Use |
| --- | --- |
| [`sandbox.sh`](../scripts/cloud/sandbox.sh) | Checks, starts, shows and stops the sandbox's Remote Control session. |
| [`setup.sh`](../scripts/cloud/setup.sh) | The environment's setup script. It installs Rust 1.90, libclang/CMake, the pinned llama.cpp object store, the pinned public models and 013 files, OMP/Bun, and the locked Cargo dependencies. |
| [`environment.env`](../scripts/cloud/environment.env) | The environment variables. They hold paths only; there are no secrets. |
| [`allowed-domains.txt`](../scripts/cloud/allowed-domains.txt) | Hosts added under Custom network access, alongside the default package managers. |
| [`doctor.sh`](../scripts/cloud/doctor.sh) | Checks a session: host limits, `avx512_bf16`, toolchains and pinned inputs. |
| [`fetch-resume.sh`](../scripts/cloud/fetch-resume.sh) | Downloads and verifies the private archive and prints its `RESTORE.md`. It takes the archive from a private GitHub release attached to the session, or from an HTTPS URL. |

An Anthropic-hosted VM has about 4 vCPUs, 16 GB RAM and 30 GB of disk. That is half
the memory of the checkpoint host, which affects two steps below:

- Step 5's 18.6 GB Qwen3-Coder-30B-A3B pair cannot run in this environment. It needs
  a larger host.
- Steps 3 and 4 need `avx512_bf16` to run the same SIMD revision-5 function.

[`CLAUDE.md`](../CLAUDE.md) gives agents the session rules for both places.

## Resume in this order

1. Verify branch/HEAD/clean status and fetch the final remote CI result. Restore
   prerequisites from [resume instructions](resume.md) and the environment setup;
   this machine uses Rust 1.90, four CPU quota, 32 GiB RAM, no swap. Run only one
   heavy local build/index/model experiment at a time. Do not include compilation
   or competing inference in model throughput measurements.
2. Complete/review the separate document/body RRF experiment from
   `resume-capsule/foundry-document-body-rrf/RESUME-WIP.md`. Its patch is explicitly
   **NOT READY TO APPLY**, uncompiled and incomplete. The helper fragment is also
   uncompiled. It needs plumbing, type fixes, focused tests and contract updates.
   Proposed fixed policy: RRF constant 60, one-based ranks, exact current lexical
   document overlap plus independently scored Body evidence, deterministic ties,
   ordinary/refined deduplication, before existing destructive quotas. Preserve
   ordinary Body candidates outside refinement scopes; do not manufacture ownership,
   inherit parent cosine as body cosine, or increase existing work/output bounds.
   Current traces identify Q0 excerpt/selection loss, Q3 packing loss, Q4 a called
   helper outside the selected owner, Q5 missing constructor and Q6 unobserved target.
   Read the preserved actual trace analyses; do not tune thresholds on expected labels.
3. Review `resume-capsule/linux-batch-resume-handoff.md`. Compile its small read-only
   cache exporter against the current semantic rlib, then run the prospective
   batch-one/eight calibration on the same 128 source files. **Neither exporter
   compilation nor calibration ran.** The fixed selection requires identical keys,
   tokens, locations and zero-call replay; cosine >=0.9999, normalized finite vectors,
   at least 1.10x provider throughput, no more than 1.05x preparation wall, four threads,
   8 GiB worker ceiling, 600 seconds per arm / 1,500 seconds total. Eight pure control
   mocks pass, not real-model acceptance. The batch-aware full driver is a separate
   reviewed-but-unrun file; original `V11/run-full-overview.py` remains unchanged.
4. Declare and actually run the full Linux overview job after calibration. Use a
   fresh store, 512-token overviews, explicit 512 MiB cache, same SIMD revision-5
   function and the empirically selected scheduling batch. The original batch-one
   estimate was roughly three provider-hours plus overhead; declare a realistic wall
   budget, preserve partial phases and stop below the disk reserve. It needs at least
   2.75 GiB free before starting. Full overview completion does not imply body coverage.
5. Run the prepared stronger host pair from `V11/stronger-model-research/next-run.md`.
   The Qwen3-Coder-30B-A3B Q4_K_M runner streams the pinned 18,556,689,568-byte model
   into a sealed memfd, then uses one CPU server/32K context. **No acquisition or pair
   ran.** Its complete outer budget is 4,200 seconds, each task 900 seconds; memory
   admission, OOM/swap/pressure, exact model/template and genuine native tool calls
   must pass. Preserve all older negative/unknown costs. Both unchanged oracles,
   real edits and complete accounting must pass before comparing completion costs.
6. Freeze final candidates explicitly before releasing the sealed independent
   48-question set. Root has not read its questions/labels; **no evaluation ran**.
   Preserve the fixed 2,048-token budget, at most three comparisons, paired test and
   simultaneous interval, token/resource/latency guards. Its conservative interval
   needs at least 13 gains and zero losses out of 48. Do not alter the protocol to
   manufacture acceptance. These questions do not replace unavailable private G2/G1.
7. Review the unrun native CPU diagnostic patch under
   `resume-capsule/foundry-native-cpu-diagnostics/`. It logs actual admission counters,
   self CPU and bounded process-name snapshots in explicit tests only. Investigate
   the failed prerequisite without weakening policy. Separately, the committed driver now
   records bounded `top` snapshots, process names only, just before and after the
   CPU-pressure test. They identify Spotlight indexing as the busy host activity, and
   the workflow now stops Spotlight on the runner. The capsule patch is still unapplied.

Sealed identities (hash verification does not authorize inspecting labels):
manifest `01a9ad47473c0b99f82bee5f2fabf67cf96c86134e9627c63bd50cb67d80d429`;
seal `5d599fb9d13a9950233491dd8c4881ef44cbcdbb06287ba64cd0bcc0c836da22`;
protocol `7bb60b43908269410cfdb29bc35086f9da684d9c0d480086907ee3c97c9d02a0`.
No local live model, indexing or experiment process was left running. Pre-existing
PID-1-owned zombie entries are not live workers and were not claimed or signaled.
