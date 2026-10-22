# Experimental ecosystem checkpoint — 2026-10-11

## Current status — 2026-10-21

This section owns current status; other documents link here, and dated paragraphs
below are history.

- **Measurement.** Retrieval is judged on the frozen G2 set (388 questions over
  rust-lang/rust and oh-my-pi) with [`scripts/g2.sh`](../scripts/g2.sh) on the owner's
  Mac. The 2026-10-08 result reproduces exactly. The eight Foundry questions are a
  smoke check only; from 4/8 they cannot show a significant gain.
- **Enable rule (owner, 2026-10-19).** No mean-token clause. A candidate must locate
  significantly more definitions (p < 0.025), lose no verbatim definition bodies net,
  and stay within the 2,048-token cap; mean tokens are reported only.
- **Scores at 2,048 tokens (located / verbatim body, of 388).** Default lexical 0/0;
  experimental lexical 13/10; Gemma address cards (default recipe) 136/120, unchanged
  from 2026-10-08 (139/123); complete hierarchy **169/151** (rust 54/47, bun 115/104).
  File overviews (bun only) 77/45 of 191.
- **Decided.** Breadth-first packing is rejected: +14 located but −95 verbatim
  bodies. Experimental lexical passes G2 but stays off for 001 T010's anchored and
  agent-task gates; it also lowers hierarchy (169 → 155).
- **Recipe decision pending (owner).** Complete hierarchy passes the G2 rule against
  cards (+78/−45, p = 0.0019) at 1.8–2.1 times the units, 5.9 hours of Metal time for
  rust (cards: 72 minutes) and 1.3 GB more serving memory. Default promotion still
  needs the G1 gates and held-out questions.
- **Fixed.** Semantic publication refused large repositories through worst-case size
  estimates (about 100k cards since 2026-10-09; a 685,623-unit hierarchy on
  2026-10-21). It now counts exact JSON and fences old checkpoints; the vector bound
  (89% used by rust's hierarchy) is the next scale limit.
- **Next engineering targets, by G2 loss stage (cards):** target unit never retrieved
  (120 = 45 file absent + 75 unit absent), cut by placement limits (95), omitted by
  the packer (37).
- **Open.** The 6,000-file watcher test is intermittent on this Mac (FOLLOWUP F18).
  Acceptance needs held-out questions (F4). Evidence:
  [rust hierarchy](validation.md#complete-hierarchy-on-rust--2026-10-21),
  [rule change](validation.md#g2-rule-without-the-token-clause--2026-10-19),
  [G2 results](validation.md#g2-restored-as-the-retrieval-instrument--2026-10-17).

**Representation repair continuation, 2026-10-17:** body-window contexts keep the
region's own declaration when over budget (`+context2`), and long overviews keep
each declaration's documentation summary and the file opening (`+overview4`). The
eight public questions were rerun with real EmbeddingGemma 2 on Apple M3 Pro Metal:
overview and complete hierarchy both stay at **4/8** before and after; experimental
lexical stays 4/8 and default 1/8. Dense target ranks move both ways, so no retrieval
gain is claimed. Lexical ranking variants that gained Q4–Q6 each lost Q0 and were
reverted. Owner-scope refinement ordering is live (the 4,096-vector cap bound twice)
and stays. See the [repair receipt](validation.md#representation-repair-and-lexical-ablation--2026-10-17).

**Runtime repair continuation, 2026-10-17:** code `81c20ad` preserves independently
scored file bodies alongside lexical owner hints and admits task relationships with
their source dependencies under one allowance. `d72f3bb` keeps store ownership until
index-writer shutdown finishes and strengthens the two-relationship regression.
The real-model fixture passes all four cases in both arms, with actual owner/file
competition, exact bytes, bounded output, worker cleanup and zero-call replay.
All five final local gates pass: 1,174 tests, zero failures, 28 ignored; seven real
framework/runtime checks also pass. Earlier failed gates remain in the
[repair receipt](validation.md#evidence-admission-and-owner-cleanup--2026-10-17).
The public eight-question benchmark was not rerun; broader retrieval and economics
remain open. Physical Mac/GPU acceptance is a gap. FOLLOWUP protocols stay parked.

**Callable evidence continuation, 2026-10-16:** runtime code `f782dda` improves
both public experimental lexical delivery and real Gemma 2 file-overview delivery
from **3/8 to 4/8 at 2,048 tokens**, preserving Q0, Q2 and Q7 and adding Q3. The
repair keeps already discovered, disjoint documents of the same exact callable
together before lexical cuts and after dense interleaving. It adds no model,
dependency, candidate slots or output allowance. Default lexical remains 1/8;
historical full-hierarchy delivery remains 2/8 and was not rerun.

The final public lexical check delivers 16,151 tokens versus 16,177 across eight
questions; the real overview check delivers 16,242 versus 16,289. These are source-
evidence counts, not coding-task success rates or provider savings. Unchanged
real-model replay reuses all 274 inputs and partitions in 4.551 seconds with zero
embedding calls/tokens. Verbatim source hashes/ranges, response budgets and worker cleanup
pass. The final model comparison reuses verified baseline observations from the
preceding paired run; no latency benefit is claimed. Read the
[retrieval receipt](validation.md#callable-passage-preservation--2026-10-16) and
[machine-readable results](review/owner-retrieval-2026-10-16.json) for local gate
results, exact identities and reproduction commands.

A bounded coarse-vector callable localizer was also tried and removed: the real
model stayed at 3/8. Its patches, traces, negative results and failed development
checks remain under
`/workspace/.context-foundry-tools/validation-runs/owner-retrieval-2026-10-16/`.
`merged-model/` holds the retained candidate; `final-lexical2/` holds its lexical
comparison. The focused suite passes 445 tests; all five full local gates pass on
Rust 1.97.0/1.90.0, with 1,173 tests passed, zero failed and 28 ignored. All 54
gateway tests pass under uid 1000. Native Mac/GPU acceptance is a gap.
No private archive or parked FOLLOWUP protocol was used.

Next retrieval work is Q4–Q6: finding the correct file does not reliably identify
the needed implementation, and stemmed term coverage did not fix it. Q1 remains
excluded by unchanged admission, while all eight remain in the denominator. Broader
retrieval acceptance, representative inference, task quality and total provider
costs remain open. Partial overlaps and greedy full-form packing also remain.
Unrelated [FOLLOWUP](../FOLLOWUP.md) protocols stay parked.

The preceding budget repair (`c258026`, diagnostic correction `1f0827c`) established
2/8 → 3/8 and improved identical-source delivery: 27 instead of 36 calls, and
6.48% fewer delivered tokens, at a 512-token allowance. Those results and the
274-vector preparation receipt remain in
[validation](validation.md#budget-allocation-and-evidence-preservation--2026-10-16).
Neither this continuation nor the previous one establishes complete task economics
or a release.

**Earlier managed-host continuation, 2026-10-16:** the published Codex sandbox passes the portable
Linux setup checks. The owner selected the existing managed agent runtime as the
coding host; Foundry stays local and is callable through its CLI. A public seeded
repair passes the external checker in both baseline and Foundry arms, but provider
tokens/billing are unavailable and no economic benefit is established. OMP and a
separate Codex CLI login are not prerequisites for this managed path. Read the
[current receipt](validation.md#managed-host-and-gateway-continuation--2026-10-16)
before the historical checkpoint below. Work remains on
`experimental/context-ecosystem`; the parked items in [FOLLOWUP](../FOLLOWUP.md)
and the native macOS/GPU gap remain. Do not fetch the private resume archive.

At the 2026-10-11 checkpoint, the owner requested a pause, a portable handoff and
continuation in a new context. **This is not completion of the six acceptance
gates.** That session stopped model experiments; later authorized runs are recorded
above. Start here, then read `AGENTS.md`, the
constitution, [validation](validation.md#realistic-acceptance-continuation--2026-10-11)
and the [current inventory](context-ecosystem-plan.md#pending-work-inventory--2026-10-10).
Use the strongest available reasoning/coding model and inspect actual artifacts.

## Cloud continuation — 2026-10-12

A Claude Code cloud session continued from `16d7f0a` on branch
`claude/festive-cannon-ladw9u`. Its work was merged into `experimental/context-ecosystem`
through [PR #11](https://github.com/dwbimstr/context-foundry/pull/11) (`ea86fc6`) and
[PR #12](https://github.com/dwbimstr/context-foundry/pull/12). By owner decision the line
was then merged into `main` through [PR #13](https://github.com/dwbimstr/context-foundry/pull/13)
(`a691ab3`); experimental remains the working branch. It used public inputs only. GitHub Actions is now unavailable by owner decision, so a local
`scripts/gates.sh` receipt replaces CI (see [CLAUDE.md](../CLAUDE.md)).

- **Step 1** is closed.
- **Step 2:**
  - The frozen corpus is commit `ebf28f8`, and the lexical baseline reproduces (1/8,
    and 2/8 experimental).
  - The RRF candidate is implemented and tested as `9ccefcc`, then reverted by
    `ef524b9` until it is measured.
  - The dense measurement needed `avx512_bf16`, which the session's VM lost when it
    restarted. A 2026-10-14 cloud session with `avx512_bf16` ran it: the candidate and
    the interleave arm both deliver 2/8, so the candidate stays reverted.
- **Steps 3 and 4** also need public archives that the session's proxy refused.
- **Step 7's** cause, Spotlight indexing, is fixed.

Every remaining item is in [FOLLOWUP.md](../FOLLOWUP.md) with its blocker and resume
commands: F10 and F11 for steps 3 and 4, F2 for step 5,
F4 for step 6, and F7 and F8 for the native and capsule work. F9 records step 2's
measurement. Results are in [validation](validation.md#cloud-continuation--2026-10-12).

## Code and working state

Repository: `/workspace/context-foundry`; branch: `experimental/context-ecosystem`.
The 2026-10-11 checkpoint's tested implementation was `6ac60d8`, with documentation
commits after it. The line has since gained code changes from the 2026-10-12 cloud
session:
- Clippy 1.97 fixes (`72c4319`);
- the no-default-features build fix (`f491821`);
- dependency updates (`b432f46` and later);
- the `sha2` 0.11 migration.

The current tested code is the latest commit with a local gates receipt in
[validation](validation.md#local-gates-receipts). Relevant 2026-10-11 commits, in order:

- `bc10986`: held-descriptor Mac model loading and native lifecycle fixes.
- `f75f14c`: independent native CPU/model checks and 128-file calibration.
- `8ef3acc`: bounded hierarchy resume plans and structural-fragment roles.
- `c2337f7`: bounded full-corpus native driver; this driver has **not run** at scale.
- `e7a1b59`: foreground-release notification, shutdown and watcher restart handling.
- `6ac60d8`: current owner scopes and coverage-independent experimental interleaving.

Preserve file-first hierarchy publication, exact-input cache identity, zero-call
unchanged replay, freshness-checked router/caller/test evidence, coarse excerpts,
acronym matching and CLI worker reaping. Default context requests remain 2,048 tokens; the budget repair raises the implicit
ceiling for explicit requests to 32,768. Hierarchy/body
role metadata uses `kinds2`; overview/address recipes retain `kinds1`.

## Actual acceptance state

| Gate | Verified now | Still required |
| --- | --- | --- |
| Retrieval | Complete real hierarchy: 274 admitted sources, 13,822 vectors, 14,453 locations. Complete original, role-only and owner-scope arms deliver 1/8; coverage-independent interleaving restores 2/8. Historical experimental lexical was 2/8; the 2026-10-16 budget repair reached 3/8, and callable passage preservation now reaches 4/8 at the same 2,048 tokens. The separate real-model overview comparison also reaches 4/8. Default lexical remains 1/8. | Effective general repair and independent acceptance. All eight stay in the denominator; one historical source is excluded by unchanged private-key policy, so admitted ceiling is 7/8. Do not bypass admission or relabel outlines as body evidence. |
| Host tasks | Historical Qwen3-4B pair fails both unchanged external oracles. The managed-runtime public seeded repair passes in both arms; provider usage is unavailable. | Representative correct completions and complete provider accounting. The private stronger-model protocol remains parked. |
| Representative scale | Exact overview fitting covers 67,663 admitted Rust/Go sources; 60,735–65,959 distinct inputs / 14.65–15.86 million tokens. | Full real inference, audit/replay, serving and changed-input verification. No complete 67,663-source model run exists. |
| Economics | Identical-source CLI delivery improves by 25% fewer calls and 6.48% fewer delivered tokens at 512 tokens. Provider/task benefit remains unproved. Failed attempts and unknown usage are retained. Selected history totals 71 requests, including the latest 13; 66 complete and five unknown. | Demonstrated completion benefit with bootstrap, acquisition, retrieval, maintenance and failed attempts included. Smaller failed prompts are not a benefit. |
| Evidence | Bounded source call postings, conservative Python aliases, router dependency proofs and revision-bound execution snapshots; eight real framework/execution checks pass. | Broader collectors, cross-language resolution and realistic task outcomes. Unsupported/dynamic cases remain explicitly incomplete. |
| Mac/GPU | Real Metal embeddings, 8,192-token input, owned generation, cleanup and CPU/Metal parity pass. Actual 128-file calibration and zero-call replay pass. With Spotlight stopped on the runner, native CPU-pressure acceptance passes twice at unchanged thresholds (runs 38207948408, 38209502529). | Physical-device performance/global GPU pressure, combined residency and signed installed-package acceptance. |

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
The 128-file sample needs 744.804 provider seconds for 30,726 tokens (41.254 tokens/s);
a repeat with Spotlight stopped (run 38209502529) measures 41.584 tokens/s.
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
- `rust-ci-38178644843/`: a snapshot taken while run 38178644843 was still running. The
  run finished successfully on both platforms, as recorded above and in validation.
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
| [`doctor.sh`](../scripts/cloud/doctor.sh) | Checks a session: the environment variables, host limits, `avx512_bf16`, toolchains and pinned inputs. |
| [`gates.sh`](../scripts/cloud/gates.sh) | Runs the merge gates (`scripts/gates.sh`) with `environment.env` applied, an optional trusted `CF_GATE_ENV` file, and `RUSTUP_TOOLCHAIN` unset; `bash scripts/cloud/gates.sh OUTDIR`. |
| [`fetch-resume.sh`](../scripts/cloud/fetch-resume.sh) | Downloads and verifies the private archive and prints its `RESTORE.md`. It takes the archive from a private GitHub release attached to the session, or from an HTTPS URL. |

For a prepared environment with tools outside `/opt/cf`, source `environment.env`
first, then apply the local path overrides. The doctor honors `LIBCLANG_PATH`
(a directory or library file); an invalid explicit override fails instead of
falling back to another installed library. Set `CF_SETUP_RECEIPT` to the regular,
nonempty receipt from that environment's actual preparation. Its default remains
`/opt/cf/setup-receipt.txt`; a receipt is not a substitute for the other checks.

An indexing/retrieval setup that deliberately omits generation weights can set
`CF_GENERATION_REQUIRED=0`. The doctor then reports generation as unconfigured and
unvalidated. The default still requires `CF_GENERATION_GGUF`, and invalid policy
values fail. Restore `CF_GENERATION_REQUIRED=1` before any generation validation.
These overrides do not change worker profiles, model identities or acceptance
requirements.

For full gates in a different layout, set `CF_GATE_ENV` to a trusted, non-secret
shell environment file that applies the local paths after `environment.env`.
Prepend the actual Rust 1.97 bin directory to `PATH` there; the gate runner then
prepends the actual Rust 1.90 directory for its MSRV checks. The wrapper unsets
`RUSTUP_TOOLCHAIN` after both files. Incremental compilation and dev/test debuginfo
must remain `0`; invalid overrides fail before any gate starts. Missing, unreadable,
empty or non-file overrides also fail. `OUTDIR/env.txt` records the override path
and SHA-256, selected effective settings and compiler paths, excluding private
archive configuration and arbitrary exports. Never place credentials in the file.

An Anthropic-hosted VM has about 4 vCPUs, 16 GB RAM and 30 GB of disk. That is half
the memory of the checkpoint host, which affects two steps below:

- Step 5's 18.6 GB Qwen3-Coder-30B-A3B pair cannot run in this environment. It needs
  a larger host.
- Steps 3 and 4 need `avx512_bf16` to run the same SIMD revision-5 function.

[`CLAUDE.md`](../CLAUDE.md) gives agents the session rules for both places.

## Resume in this order

1. Verify branch/HEAD/clean status and the latest local gates receipt (CI is unavailable). Restore
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
