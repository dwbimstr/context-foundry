# Validation

## Cloud continuation — 2026-10-12

A Claude Code cloud session continued the checkpoint from public inputs only. It did
not use the private resume archive, and GitHub Actions is unavailable by owner decision.
The host had 4 vCPUs, 15.7 GiB of RAM and no swap. Its VM restarted mid-session and
came back on a different CPU: first a Xeon @ 2.10 GHz with `avx512_bf16`, then a Xeon
@ 2.80 GHz without it. Remaining work is listed in [FOLLOWUP.md](../FOLLOWUP.md).

**Owner decision, 2026-10-12.** The owner authorized merging
`experimental/context-ecosystem` into `main` while the acceptance gates in
[FOLLOWUP.md](../FOLLOWUP.md) stay open. Retrieval remains 2/8. Scale, host tasks,
economics, the sealed evaluation and the native macOS re-check remain unaccepted. The
merge marks none of them passed and is not a release; the release checklist still
applies. No Rust, Cargo or build file differs from `b432f46`, so its local gates receipt
below covers the merged code, and the gates were not rerun. Native macOS acceptance was
not rerun either, because Actions is unavailable.

### Local gates receipts

`scripts/gates.sh` replaces Rust CI. The default toolchain was Rust 1.97.0, and the
MSRV gates ran on Rust 1.90.0. Each receipt directory held the logs and the starting
`git status`, under the session's ephemeral `/workspace`, so the digests below identify
the logs. The `test` gate runs as root, which bypasses the read-only
permissions two gateway fault tests rely on, so those tests are rerun unprivileged.

| Commit | fmt | clippy 1.97 | test 1.97 | msrv-check | msrv-clippy |
| --- | --- | --- | --- | --- | --- |
| `d6903f7` | 0 | 101: three new lints | 101: 1,145 passed, only the 2 root-only failures | 0 | 0 |
| `ef524b9` | 0 | 0 | 101: 1,145 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `b432f46` | 0 | 0 | 101: 1,145 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |

Log SHA-256 digests:

| Commit | clippy | test | msrv-check | msrv-clippy |
| --- | --- | --- | --- | --- |
| `d6903f7` | `712708a6…` | `d37d829e…` | `437b9d06…` | `368e057b…` |
| `ef524b9` | `fcc7dfca…` | `ea77740f…` | `0d73feed…` | `243e2afe…` |
| `b432f46` | `38c4c123…` | `276e988f…` | `b0f11c35…` | `45f25865…` |

The `ef524b9` run started with only the new `FOLLOWUP.md` untracked. Its counts equal
`d6903f7`'s because `ef524b9` reverts the RRF candidate. `b432f46` started from a clean
tree. It applies Dependabot updates #10, #8 and #7 to this line (see
[dependencies](dependencies.md)); `sha2` 0.11 (#6) was left out because it fails to
compile. Each update was also checked alone on Rust 1.90. With `bindgen` 0.73 both
llama.cpp workers build, and the regenerated bindings differ only in 20 opaque handle
types losing `Copy`/`Clone`.

`72c4319` fixes the three Clippy 1.97 lints (`byte_char_slices`, a manual `Option::zip`,
`while_let_loop`) without changing behavior. `f491821` fixes a regression from
`8ef3acc`: `cargo build --no-default-features --lib` had stopped compiling, because
`syntax::units_with_certainty` was gated on the `semantic` feature while a caller
outside that feature used it. The build now compiles without warnings. Native macOS
acceptance did not run: there is no Mac here, and Actions is unavailable.

Two timing-bound integration tests failed once each in a full parallel run of
`tests/neural_retrieval.rs` on this 4-vCPU host and passed on rerun:
`progressive_bootstrap_serves_priority_before_the_unprepared_tail_and_resumes` and
`continuous_semantics_progresses_during_filesystem_traffic_and_converges`. Neither
touches placement. Their 15-second wall-clock deadlines were not changed.

### Document/body RRF (resume step 2)

**Frozen corpus.** Commit `ebf28f8` is the frozen Foundry snapshot. The eight labelled
needles of `CASES` in `tests/retrieval_attribution.rs` occur there exactly once each,
and its `src/ingest.rs` still contains the literal private-key marker. Extracting it
with `git archive ebf28f8` gives an uncompressed tar stream with SHA-256 `31ff389a…`.

**Lexical baseline.** A fresh index changes 275 sources and excludes two:
`src/ingest.rs` (private-key marker) and the binary SCIP fixture. The harness then
deletes the evaluator source, leaving the recorded 274 admitted sources. At the fixed
2,048-token budget, default delivery is **1/8** (Q7) and experimental lexical delivery
is **2/8** (Q2, Q7). Both match the frozen record. Raw rows are in
`validation-runs/rrf-step2/lexical-01/`.

**Candidate.** `9ccefcc` implements the stated policy independently; the capsule's
unfinished patch was not available. Experimental anchorless prose over body or
hierarchy recipes fuses current lexical documents and Body locations by reciprocal
rank (k = 60, one-based ranks), before the duplicate, per-file and limit cuts.
- **Lists:** the lexical list keeps its order. The Body list keeps the dense order,
  refined owner-scoped passages before ordinary hits, and counts a location found
  both ways once.
- **Support:** a document and a Body location add each other's best reciprocal rank
  only when their byte ranges overlap in the same current source.
- **Ties and coarse locations:** exact ties keep the previous experimental order.
  Parent, file and container-fragment locations follow in dense order.
- **Bounds:** no window, slot or output bound grows.

The first draft ranked Body evidence by raw distance. The existing callable-scope
regression rejected it: an ordinary sibling took an owner-scoped passage's per-file
slot. The draft was corrected to keep the dense order.

Verification: five new unit tests pass, the semantic-retrieval suite passes (93, 1
ignored), Clippy 1.90/1.97 passes and contracts 009 and context-v2 are updated. The
placement regression shows the mechanism directly: in the hierarchy case the lexical
document leads because a refined passage overlapping it supports it
(`supported_lexical: 1`).

**Not measured.** The dense comparison needs the revision-5 AVX-512 BF16 worker. After
the VM moved, that worker refused to load with "AVX-512 BF16 embedding worker is
unsupported on this CPU". A different embedding function was not substituted, so no
dense result exists. `ef524b9` reverts the candidate, keeping the measured 2/8
interleave arm as the experimental placement. [FOLLOWUP F9](../FOLLOWUP.md) has the
exact measurement and decision rule. Retrieval therefore remains **2/8**.

### Steps 3 to 7

- **Steps 3 and 4** (Linux calibration and full overview) did not run. They need the
  revision-5 worker, and the proxy refused the pinned public Rust and Go archives
  (`codeload.github.com` 403 for `rust-lang/rust` and `golang/go`). See F10 and F11.
- **Step 5** cannot run on a 15.7 GiB host. **Step 6** and the private G1/G2 and Rust/Go
  label sets need the archive. See F2 to F6.
- **Step 7's** cause is resolved: Spotlight indexing (see below). Physical-device and
  signed-package acceptance remain in F7.

## Realistic acceptance continuation — 2026-10-11

Work continues on `experimental/context-ecosystem`; the open acceptance conditions
are not closed by the checks below. Private receipts and executable identities are
under `/workspace/.context-foundry-tools/validation-runs/realistic-delivery-v11/`.

**Completed hierarchy diagnostic.** The pinned SIMD revision-5 Gemma worker now
prepares all 274 admitted frozen Foundry sources: 13,822 distinct vectors,
14,453 current searchable locations and 3,608,991 input tokens. The three
preparation phases take 2,710,537 ms in total; provider calls account for
2,413,715 ms. Initial whole-file views precede refinement. Unchanged replay takes
3,977 ms with no embedding calls, token inputs, partitioning or worker acquisition.
Complete coverage nevertheless delivers only **1/8** frozen development answers,
versus lexical **2/8**: coverage-dependent dense precedence displaces the previously
delivered failed-attempt recall. All eight remain in the denominator, including
the source excluded by the unchanged private-key-marker policy.

The first controlled correction classifies 984 parsed container fragments separately
from implementation bodies. It preserves every input key/source span and reuses all
13,822 vectors with zero migration or replay inference. Delivery remains **1/8**;
removing structural fragments does not resolve legitimate sibling-function ranking.
These are exposed development results, not held-out acceptance. The completed
baseline and role-only reports remain in `foundry-hierarchy-complete-01/` and
`foundry-hierarchy-roleonly-01/`. Focused boundary checks pass: 245 library,
91 semantic-retrieval and two attribution tests, plus all-target Clippy. They cover
the shared 16 MiB hierarchy resume cache, freshness/eviction, CLI/MCP pending work,
file-first publication and zero-call replay; larger-scale throughput is still open.

The separate owner-scope comparison also delivers **1/8**. The third fixed
comparison keeps hybrid placement independent of coverage and restores the displaced
lexical answer, reaching **2/8**; it does not establish the required general repair.
All three preserve every current input key/span and perform zero migration/replay
embedding calls. Owner proofs and lexical relevance must survive subsequent ranking:
traces still show global/per-file cuts and packing losses. These completed reports
are in `foundry-hierarchy-ownerscope-01/` and `foundry-hierarchy-fusion-01/`.
The combined focused run passes 252 library, 93 semantic-retrieval, two attribution
and four watcher tests, including the unchanged 6,000-file deadline. All-target
Clippy and formatting pass. Default retrieval remains unchanged.

Actual [native run 38176235770](https://github.com/dwbimstr/context-foundry/actions/runs/38176235770)
on macOS 15 arm64 passes Metal embedding at 8,192 tokens, owned generation,
worker cleanup and 128-file preparation/publication with zero-call replay.
CPU/Metal comparison across five identical inputs passes with minimum cosine
0.9999949405. The device is Apple Paravirtual Metal, not a physical-Mac performance
receipt. The run still **fails native CPU-pressure acceptance**: its initial idle
prerequisite never reaches 30%; raw causes need further observation. No threshold
was relaxed. The same test passed in native runs
[38172126296](https://github.com/dwbimstr/context-foundry/actions/runs/38172126296) (`4dd9da9`)
and [38172614756](https://github.com/dwbimstr/context-foundry/actions/runs/38172614756) (`4dab8e4`):
idle 9%, loaded 100% and recovered 2%. It failed in
[38174697974](https://github.com/dwbimstr/context-foundry/actions/runs/38174697974) (`bc10986`)
and 38176235770, where host CPU stayed at 84–100% for the whole 15-second idle
window. All four runs used runner image `macos-15-arm64` 20260907.0337.1 and the
same stage order. The commits between them change neither the sampler nor this
test. The driver now records a bounded `top` snapshot of process names just before
and after this test, outside its measured window. In
[38205301244](https://github.com/dwbimstr/context-foundry/actions/runs/38205301244) (`1b5f0b1`)
the test fails again with idle samples of 81–100%. Both snapshots show Spotlight
indexing: `mds_stores` at 45–50% and `mds` at 12–23%, plus six or seven
`mdworker_shared` workers. Host idle is 0–2% at a load average of 15 on three CPUs,
after the job has written about 8 GB. The test process does not appear among the 12
busiest processes. The cause is therefore Spotlight indexing the job's own output,
not the sampler or the policy. All model and lifecycle checks in that run pass.
The workflow now stops Spotlight on the ephemeral runner before building, and the
driver records `mdutil -a -s`, without changing a threshold. With that change,
[38207948408](https://github.com/dwbimstr/context-foundry/actions/runs/38207948408) (`e0a1458`)
passes the whole native job for the first time: idle 2%, loaded 100% with three
threads, then recovered samples of 31%, 35%, 35% and 27% at the unchanged 60%/30%
thresholds. The host is 96% idle just before the test. Metal embedding and owned
generation pass in the same run. This is one passing run on Apple Paravirtual
hardware; physical-device, global GPU and signed-package acceptance remain open.
The 41.254 tokens/second calibration below ran with Spotlight enabled.
[38209502529](https://github.com/dwbimstr/context-foundry/actions/runs/38209502529) (`7e7176b`)
repeats it with Spotlight stopped, using the same function digest, recipe and sample
manifest: 128 calls / 30,726 tokens in 738.899 provider seconds (41.584
tokens/second, +0.8%), with zero-call unchanged replay. Indexing was not the
throughput limit, so the full-corpus Mac budget remains rejected. The same run
passes CPU policy again (idle 14%, loaded 100%, recovered 3%).
Calibration performs 128 calls / 30,726 tokens in 744.804 provider seconds
(41.254 tokens/second). This rejects the proposed full-corpus Mac runtime budget;
it is not 67,663-source acceptance. Generation records actual GPU allocation
and successful initial/change/restart/revert behavior. Signed installed-package,
combined residency and global GPU-pressure acceptance remain separate.

The full default Ubuntu job at `bc10986` passes (1,131 tests). The corresponding
Mac job passes 1,231 tests and fails one: the 6,000-source watcher reaches 5,806
sources at its unchanged 180-second reconciliation deadline. This is retained as
a failure; native model acceptance and the foreground-release investigation are
separate ongoing checks. The next full default run,
[38178644843](https://github.com/dwbimstr/context-foundry/actions/runs/38178644843) at `e7a1b59`,
passes on both Ubuntu and macOS, including the unchanged Mac watcher deadline.
That is one passing Mac run after the foreground-release change, not a repeated
measurement. It does not cover `6ac60d8`, which was pushed under a `[skip ci]` head.
[38205296934](https://github.com/dwbimstr/context-foundry/actions/runs/38205296934) at `1b5f0b1`
(code at `6ac60d8`) passes on Ubuntu (1,147 tests) and macOS (1,248 tests), including
the Mac watcher, which makes two consecutive passing Mac runs.
A local Linux run of the checkpoint (code at `6ac60d8`) with Rust 1.90 in a cloud
session passes fmt, all-target Clippy and 1,145 tests. Two gateway fault tests fail
there only because the session runs as root, which bypasses their read-only
permissions; run unprivileged, the whole gateway target passes.

**Implementation.** Retrieval now distinguishes representation roles and scores
current resident body vectors within bounded candidate files. A scored vector can
deliver only the exact current locations that passed its partition/generation join.
File-first hierarchy publication, unchanged-input cache keys, coarse excerpts and
anchor startup avoidance remain. Call-name postings cover admitted source ingestion
and expose unsupported/incomplete parsing; search schema 7 adds conservative Python
import aliases without changing source-unit schema 5. Alias delivery requires fresh
definition, import and call bodies together. Execution-v2 imports record a complete
producer snapshot and suppress it after an indexed source revision changes; syntax
candidates remain unresolved and are never labeled observed calls.

**Linux checks.** The final focused set has 773 passing tests across the library and
13 integration targets, plus a passing real CPython alias check. The new alias
fixture initially exhausted Python's existing parser budget with 2,000 consecutive
comments; it now explicitly verifies parsing and retains over 32 KiB between import
and call. No parser limit or discovery/proof assertion was weakened. Installed OMP
18.6.0 passes a differential read check: its default structural projection omits a
body marker, while the supported body-read setting exposes it and preserves live-view
synchronization and resumed history. `gateway-omp --body-reads` exposes that setting
through a private fixed configuration; arbitrary host configuration remains refused.
Clippy with warnings denied, the no-default-features library build, formatting and
documentation links pass. See `focused-v11-acceptance.log`,
`native-and-evidence-checks.log`, `real-cpython-alias.log` and `omp-body-read-check.log`.

**Current negative host evidence.** The default-workflow Qwen3-4B pair reaches both
900-second task limits. Full files makes no edit; composed context makes an
uncompilable edit, then repeatedly receives outlines while asking for line ranges
only in intent text. Known input/output is 65,628/187 versus 118,220/1,168 tokens,
with one interrupted request of unknown usage in each arm. Protected files and public
tests remain unchanged. The complete runner takes 1,819.714 seconds. These costs
remain in historical accounting; no completion benefit is claimed. The next matched
recipe uses the same supported body-read configuration for both arms, preserving the
task, oracle, model, tools and limits.

**Native execution.** GitHub-hosted macOS 15 arm64 has usable Metal: an actual compute
kernel returns 42. Native CPU sampling and six worker lifecycle fixtures pass after
fixing missing/deprecated Mach bindings. Native admission then exposes a separate
VM-statistics revision mismatch: the code required the newest libc structure's exact
word count. It now accepts a successful initialized prefix containing `free_count`.
The corrected native run, full-window embedding comparison and owned Metal generation
lifecycle are pending. Earlier claims that no macOS/GPU runner was available describe
the previous environment state, not this continuation. Runs and retained failures:
[first](https://github.com/dwbimstr/context-foundry/actions/runs/38169264424),
[second](https://github.com/dwbimstr/context-foundry/actions/runs/38170097978).

Full frozen retrieval, complete representative inference, verified completion
economics and native model acceptance remain open until their actual runs finish.
The [portable scale commands](measurement-handoff.md) reproduce the pinned corpus,
preparation, complete publication audit and zero-call replay without private paths.

## Initial file coverage and task evidence — 2026-10-11

Continued from `57b4ff5` on `experimental/context-ecosystem`. Private artifacts:
`/workspace/.context-foundry-tools/validation-runs/realistic-delivery-v10/`.

**Task evidence.** Composition discovers unresolved call-name candidates in up to
eight already selected Rust, Python, Go and JavaScript/TypeScript files. Rust test
attributes, supported assertion macros and up to four tests matching task terms
add bounded verification candidates. Canonical definition spelling comes from the
final source snapshot, including when the query uses different capitalization.
Source edits/deletion invalidate the old evidence; exact spans and the existing
delivery allowance remain. These candidates create no compiler edges or observed
coverage. The real upstream percent-decoder context now delivers both consumer and
verification bodies under the same 2,048-token allowance; its earlier context
reported both facets missing.

**Preparation.** An exhaustive body-v1 trial on the unchanged pinned Foundry
snapshot stops within its 900-second command budget: 665 vectors / 269,121 tokens,
32 partitioned sources, 242 unpartitioned, 819,042 ms wall time and 805,269 ms in
provider calls. The command reserves 90 seconds for publication. Seven eligible
queries use actual dense serving and the anchor bypasses the model; delivery stays
2/8. This partial run does not evaluate complete body-vector retrieval. Short
compilation/tests overlapped preparation, so its timing is not an isolated throughput
comparison. Raw failures and traces remain in `body-preparation.json` and
`foundry-body-semantic/report.json`.

The resulting code repair makes hierarchy preparation visit one representation
spanning each selected file before body refinement, in both CLI and resident paths.
Publication separates those phases, including cached inputs and foreground-sized
leftovers. Existing recipe IDs, exact-input reuse, change scope, quotas and source
freshness remain. A new cold real-model hierarchy run, batch one, publishes initial
views for **all 274 admitted files**, observed and source-hash/range verified by
**288,795 ms**. Its published manifest correctly remains partial: the full hierarchy
contains **14,453 representation locations**; identical inputs share vectors. This
observation is an upper bound on when broad coverage was available, not the exact
first-publication timestamp. The saved manifest already
has 548 vectors, including subsequent refinement. See `hierarchy-first-views.json`
and its immutable manifest/location snapshots. The bounded run finishes in 378,819 ms,
with 783 vectors / 277,320 input tokens, 784 searchable locations and every source
partitioned. First searchable publication takes 12,053 ms. These are small-repository
results; all body vectors are not prepared. Fitting every hierarchy partition and
exhaustive refinement still cost work; automatic selective refinement is not implemented.
The same eight frozen questions still deliver **2/8** after this preparation:
seven eligible queries use actual dense serving without fallback, and the anchored
query skips the worker (`foundry-hierarchy-semantic/report.json`). The real CLI
snapshot predates the final resident prefetch-drain correction; the resident path
is verified separately with an instrumented provider, not claimed as a native run.

**Host outcome.** A new pinned Qwen2.5-Coder-7B Q4_K_M/OMP 18.6.0 pair uses the same
upstream percent-encoding 2.3.2 seeded regression, external checker, 18 public tests,
2,048-token context allowance, 768-token response limit and one attempt per arm.
Prompt caching is enabled and reset between arms. Both arms return fenced JSON
resembling tool calls, execute no edits and fail the external checker. Complete
gateway usage is 8,171/4,016 input and 102/170 output tokens; cached input is zero.
Measured task times are 264,321/129,288 ms, including 0/636 ms preparation. These are
failed-attempt costs, **not completion savings**. The different model is a new recipe,
not a causal comparison with earlier 4B trials. Model acquisition is recorded
separately; energy and amortized total cost remain unknown. No text-shaped command
is promoted to an executable tool call.

**Rejected retrieval trial.** Weak query-only inflection alternatives preserve all
prior delivered spans but add none: Foundry 2/8, Rust 3/6 and 5/8, Go 5/8. The scoring
change was removed. The original questions, hashes and budgets remain unchanged.

**Attribution correction.** Lexical materialization traces now include the indexed
document range as well as its enclosing function and delivered excerpt. The diagnostic
previously blamed an excerpt when another fragment of the same function had been
selected. It now checks the actual document at selection/materialization; missing
coordinates leave the cause unknown. Final lexical comparisons retain every previous
delivery: Foundry 2/8, Rust 3/6 and 5/8, Go 5/8 (`retrieval-final-*/report.json`).
Foundry question 3 is now correctly attributed to the candidate limit. No label,
budget or old raw report was changed. The native reports above predate this correction
and retain their original loss classifications.

**CLI worker ownership.** The final process inspection found normal CLI embedding
workers orphaned as zombies after command exit. Dropping the last runtime reference
closed the job channel but did not join its provider thread. The per-command slot now
waits for existing shutdown on success/error, allowing the supervisor to reap the
child before exit. MCP's owner shutdown and provider grace remain. A blocking-provider
test exercises both return paths. The real Linux worker then passes both a dense
context command and an intentional missing-task error: each observed child PID/start
identity is absent when its parent returns, and successful delivery is byte-identical
to the preceding dense result. See `cli-reaping-v3-report.json` (12,447/11,474 ms).
Two earlier monitoring attempts failed because this kernel omits per-thread `children`
files; their records are retained and establish no reaping result. The corrected
monitor reads PPID and process identity from `/proc` instead. Cleanup can outlast the
query read deadline, under the existing supervisor grace.

**Checks.** Focused library, core, MCP, multi-root, response, task-evidence, neural
retrieval and attribution suites pass, including source freshness, exact budget
accounting, restart reuse and initial-view publication before any body call. The
resident test includes foreground-sized leftovers; no duplicate model inputs are
submitted. Gateway completion-cost accounting also passes. Clippy with warnings denied,
the no-default-features library build, formatting and documentation links pass.

Full representative model inference, general retrieval quality, reliable host-task
completion/economics and native macOS/Metal/GPU acceptance remain open. The first
file-view pass is useful progressive coverage, not full retrieval acceptance.

## Named re-exports and current fitting — 2026-10-11

Continued from `b54ab8a` on `experimental/context-ecosystem`. Private artifacts:
`/workspace/.context-foundry-tools/validation-runs/prose-aliases-v8/` and
`/workspace/.context-foundry-tools/validation-runs/router-reexports-v9/`.

**Retrieval.** Plural uppercase acronyms now match both ordinary source forms
(`CPUs` matches `cpu` or `cpus`). The alternatives share one concept score and one
proximity bonus, including repeated singular/plural query forms. Explicit symbols
and the stored tokenizer are unchanged. Preserved lexical delivery remains Foundry
2/8, inspected Rust 3/6, additional Rust 5/8 and Go 5/8. No previously delivered span
is lost, and this matching fix alone establishes no general quality gain.
The final quiet real-model comparison also remains **2/8**: seven eligible queries
use dense serving without fallbacks and the anchored query skips the worker
(`router-reexports-v9/foundry-semantic-accepted/report.json`). The questions, source
hashes, deadlines and 2,048-token allowance are unchanged. Five cases lose evidence
at materialization and one frozen source remains excluded; those failures remain open.

**Boundary evidence.** `python-http-v7` follows named router re-exports through
configured modules/package initializers, retaining exact import-hop dependencies.
Pure forwarding modules need no app of their own; arbitrary code, unsupported imports,
cycles, ambiguous layouts, escaped/shadowed aliases and multiple mounts remain
incomplete. Resolution stays inside the configured service's existing 64-file bound.
Dependency edits, deletion and restart suppress stale relationships; changed-path
publication refreshes the admitted service, and unchanged replay parses zero files.
Task composition delivers every dependency body or omits the relationship under its
existing allowance. Old boundary recipe rows require republication.

Real FastAPI **0.141.1**, Starlette **1.6.0**, httpx **0.28.1** passes a generated
request through a package re-export. The four explicitly invoked FastAPI fixtures
pass. The first Flask invocation selected a Python without Flask and failed import;
using the already installed framework environment, the Flask **3.1.2** fixture passes.
An initial implementation test caught forwarding modules being marked incomplete
solely for lacking an app; the fix permits only the restricted forwarding syntax.
Failures remain recorded alongside corrected runs. Indexing executes no Python.

**Current representative fitting.** The same pinned Rust/Go manifest has 69,565
regular files and **67,663 admitted sources**. All now fit with the current
`overview-v1+overview3` recipe and real tokenizer, producing **67,585 card locations**;
78 sources produce no card. The 600-second phase stops at 64,112 files as declared,
retaining its partial report. Verified continuation finishes the remaining manifest
in **69,091 ms**, including 4,429 ms checking the prefix's source hashes. Both phases
use the same fitter executable, recorded in `inventory-provenance.json`.

Distinct keys are not retained between phases, so the combined obligation is a bound:
**60,735–65,959 inputs**, **14,651,529–15,857,818 input tokens**, and **177.93–193.24 MiB**
of raw float vectors before metadata/ANN storage. Total tokens including duplicate
inputs are 16,163,103. This token obligation exceeds the preceding recipe's 12,999,181
distinct tokens; distributed extracts retain more source at a preparation cost.
Compilation/retrieval checks overlapped parts of fitting, so wall times are not an
isolated throughput comparison. This completes current fitting, **not full model
inference or representative retrieval acceptance**. The small repository's completed
274-vector model check remains separately scoped below.

The inventory diagnostic now also records and checks the versioned fitter recipe
before accepting a continuation. A matching profile alone cannot authorize mixing
old/new extraction rules. Missing/older recipe identities are refused before indexing;
the two completed phases above precede this guard and have separate same-binary
provenance. Their raw reports were not relabeled.

**Checks.** Focused library, core, MCP, multi-root, neural retrieval, response,
attribution, boundary and task-evidence suites: **556 passed, 8 explicitly ignored**.
The extended task packing check also passes for both direct and re-exported imports.
Five framework checks were invoked separately as described above. Clippy with warnings
denied, no-default-features library build, formatting and documentation links pass.

**Remaining.** General retrieval quality and independent acceptance; selective body
preparation alongside coarse coverage; full current-recipe representative inference,
serving and sustained churn; further frameworks/import forms/dynamic collectors;
reliable host tasks and complete amortized cost benefits; native macOS/Metal/GPU and
installed-package acceptance. These are still active, and several remain Linux-feasible.
013 stays frozen off; no release or default promotion is claimed.

## Evidence delivery and unused startup — 2026-10-11

Continued from `e282d4a` on `experimental/context-ecosystem`. Private artifacts are
in `/workspace/.context-foundry-tools/validation-runs/evidence-delivery-v7/`.
The existing Rust 1.90/Linux toolchain and pinned Gemma CPU worker were reused;
no default promotion, release or native Mac/GPU acceptance is claimed.

**Implemented.** Long overview/hierarchy hits in experimental prose context can
return a current, exact local passage rather than the beginning of a large file.
Dense-only selection counts distinct concepts across adjacent lines, caps the excerpt
at 1,536 bytes and keeps UTF-8 boundaries. Traces retain the dense unit and actual
excerpt separately; the excerpt is not independently embedded or similarity-scored.
Complete `overview-v1` coverage now interleaves lexical matches first, preserving
precise matches when every file has only a coarse representation. Existing discovery,
per-file and output limits remain. Plural uppercase acronyms such as `CPUs` now query
`cpu`, rather than CamelCase fragments `CP` and `Us`; explicit symbols retain their
existing rules. No whole-repository source scan was added to retrieval.

Overview preparation skips parsing when the complete input fits. Files without
parsed declarations use up to 16 distributed text samples (256 bytes each before
fitting), with a nonblank fallback for sparse files. Empty extracts are refused.
Both file-capable recipes include `+overview3`: old partitions refit, while unchanged
exact inputs reuse cached vectors. Body/address recipes retain their identities.
The previous 67,663-source inventory is evidence for its prior recipe, not a newly
executed inventory of this revision. Full representative inference remains open.

**Preserved retrieval.** All frozen development queries retain their earlier delivered
spans under the same 2,048-token allowance (`retrieval-final-*/report.json`):

| Corpus | Previous experimental | Current lexical |
| --- | --- | --- |
| Foundry | 2/8 | 2/8 |
| Inspected Rust | 3/6 | 3/6 |
| Additional Rust | 5/8 | 5/8 |
| Mixed-corpus Go | 5/8 | 5/8 |

The acronym fix brings the CPU case into selection but still loses its required
span during excerpt selection. This is not a delivered-answer gain. A broad lexical
passage change lost one Rust answer and was removed; a length-normalized scoring
trial also regressed delivery and was removed. Raw failures remain under
`retrieval-passage-*` and `retrieval-length-v2-*`. The earlier `retrieval-length-*`
launch is explicitly invalid: a failed build left the old executable in place and
the accidental run was stopped. It contributes no result above.

**Complete small-repository model check.** A fresh index of the pinned Foundry
snapshot excludes the evaluator and retains the admission policy. The ordinary
Gemma CPU worker prepares all **274 admitted sources / 274 independent inputs**,
with 84,927 input tokens, in **160,618 ms**. All 274 vectors/locations are searchable,
zero sources remain unpartitioned, and there are no provider/publication errors.
An unchanged fresh-process preparation finishes in 2,665 ms with **zero document
calls**. These are debug command observations; some checks overlapped, so they are
not an isolated throughput comparison or 60–80k acceptance.

The attribution diagnostic now optionally runs the real CLI/model path using
`FOUNDRY_ATTRIBUTION_SEMANTIC_PROFILE`, preserving the frozen questions, exact spans,
source identity and 2,048-token budget. The initial dense-first coarse run delivers
only **1/8**, losing one lexical success. The final interleave delivers **2/8**, retaining
both lexical successes but establishing no general semantic gain
(`foundry-semantic-accepted/report.json`): all seven eligible queries use the real
dense path, and the anchored eighth skips it. The preceding final trial overlapped
CPU-heavy checks and had five timeout fallbacks; it remains preserved separately.
The repeated trial changed no query/deadline/token budget and ran with other heavy
checks stopped. Per-row semantic decisions now make those fallbacks explicit.
Whole-file coverage is insufficient evidence of useful retrieval. No new host-task benefit is claimed.

**Unused model startup.** The real explicit-symbol probe previously spent 13,712 ms
starting a worker it never queried. CLI search/context now share the request's
anchor/prose predicate before opening an unused profile, hashing artifacts, loading
a tokenizer or acquiring its model. Eligible queries keep the normal startup/fallback
path; resident MCP startup is unchanged. The same real-profile anchored command now
finishes in **1,535 ms with byte-identical output**. Its trace records skipped startup;
a provider-factory test separately proves no acquisition and verifies path/prose
admission. Scoped CLI search also stays byte-identical. This single observation
(`startup-report.json`) is an avoided-startup result, not a controlled latency
benchmark or total task-cost benefit. The unanchored
follow-up reaches real dense serving and delivers the expected source span. Its
initial byte-equality check failed because the earlier comparison had used a timeout
fallback; that difference is retained in `startup-unanchored-report.json`, not labeled
a production failure or byte-identical result. The final eight-query run above
confirms the intended behavior.

**Source admission.** The fresh frozen index explains one persistent absent source:
`src/ingest.rs` contains the detector's own private-key marker literal. Its current
source now constructs the identical runtime constant from pieces, so it no longer
excludes itself. Both actual markers remain rejected. A fresh current-checkout scan
admits **279 sources**, reports zero failures and excludes only the binary SCIP
fixture (`self-index.json`). The frozen corpus and its eight-question denominator
were not edited or relabeled.

**Checks.** Focused library, core, MCP, multi-root, neural retrieval, response and
attribution suites pass: **519 passed, 3 explicitly ignored**. Lifecycle checks cover exact-input reuse, recipe expansion,
changes/reverts/deletion, stale coarse generations, UTF-8 excerpts and source-empty
fallbacks. CLI startup/refusal and real anchored/unanchored consumers pass. The
explicit representative-scale tests remain ignored in the ordinary suite; the
small real-model run above is recorded separately. Clippy with warnings denied, the no-default-features library build, formatting and
documentation links pass. This is focused validation, not a full feature/platform suite.

**Remaining work.** General ranking/excerpt delivery and independent acceptance;
automatic selective body preparation alongside the coarse index; complete current
representative inference and sustained semantic/knowledge churn; broader boundary
and execution collectors; useful free-form/memory-sensitive tasks and complete
amortized cost benefits; native Mac/Metal/GPU resource/isolation/package acceptance.
These remain active TODOs, with hardware-dependent work separated from Linux-feasible
implementation. No measurements or TODOs were waived to make the branch look complete.

## Ecosystem code improvements — 2026-10-11

Continued from `6637414` on `experimental/context-ecosystem`, using the existing
Rust 1.90/Linux setup. Artifacts are outside Git in
`/workspace/.context-foundry-tools/validation-runs/ecosystem-code-v6/`.
No default promotion, release, new model, host-task success or native Mac/GPU
acceptance is claimed. The code focus is useful preparation with fewer inputs,
retrieval intent, evidence traceability and complete declared-scope accounting.

**Retrieval.** The final conservative repair prefers an explicit language before the
fixed candidate window, removes that qualifier from behavioral term scoring and
treats unmarked “Unicode whitespace” as prose. Ordinary/marked symbol behavior,
source validation, per-file caps and the 2,048-token probe allowance remain intact.
Frozen development results (`retrieval-final-*/report.json`):

| Corpus | Preceding experimental | Final candidate |
| --- | --- | --- |
| Foundry, 8 questions | 2/8 | 2/8 |
| Inspected Rust, 6 questions | 3/6 | 3/6 |
| Additional Rust, 8 questions | 5/8 | 5/8 |
| Mixed-corpus Go, 8 questions | 1/8 | 5/8 |

No previously delivered span is lost. The four Go gains are Cut, ParseBool,
ToValidUTF8 and Clone; TrimSpace/Join still reach the candidate cutoff and ReadAll
is unknown in the bounded window. Foundry still has excerpt and discovery/coverage
losses; these are not held-out acceptance results. The initial language-preference-only
variant did not improve these counts. A broader inflection variant reached Foundry
3/8 and Go 6/8 but lost one delivered answer in each Rust set (2/6 and 4/8); it was
removed. Its failures remain in `retrieval-inflections-*`.

**Preparation.** `overview-v1` computes at most one source input per nonblank file
without body/parent fitting. It shares exact file inputs with hierarchy-v1 and the
existing bounded area lane. The file fitter now uses bounded distributed extracts and
logarithmic prefix fitting; the hierarchy recipe ID includes `+overview2` to invalidate
old partitions while retaining exact-input vector reuse. Contract checks cover UTF-8,
bounded tokenizer work, retained-card fallback, expansion, changes, revert, restart
and deletion. Overviews are extracts that may omit source, not complete behavioral
representations or automatic query-triggered body expansion.

The same pinned Rust 1.90/Go 1.25 manifest has 69,565 regular files and 67,663 admitted
sources. With the real tokenizer and the unchanged 512-token allowance, complete
inventory (`overview-inventory-v2/workload.json`) records **67,585 card locations,
63,621 distinct inputs, 12,999,181 distinct input tokens and 195,443,712 raw vector
bytes (186.39 MiB)**; 78 sources have no card. The body recipe's prior complete
inventory required 782,716–831,067 distinct inputs and 2.24–2.38 GiB of raw vectors.
This measures a smaller representation obligation, not equal quality or completed
inference. The debug inventory finishes in 390,792 ms, including 9,198 ms of unchanged
index reconciliation; other checks overlapped parts of these diagnostics, so timings
are not an isolated throughput comparison. The original greedy-fitter inventory
stopped at 600 seconds after 35,388 admitted sources; its partial result is retained.
A reuse-harness attempt initially called the initializer on an existing store; it
refused without fitting and was corrected to open the existing store.

The unchanged ordinary Gemma CPU worker (revision 3, four threads, 8 GiB ceiling)
then prepares this representation under 120/60-second budgets, with two explicit Go
priorities. Cold: 404 embeddings / 78,588 tokens / 108,675 ms, publishing 404 vectors
and 441 current locations. Resume: 188 new embeddings / 33,646 tokens / 54,461 ms,
446 cached-unit reuses, publishing **592 vectors and 635 current locations**.
Both stop `budget_exhausted` with no provider/publication error. **67,021 sources
remain unpartitioned**; this is bounded real inference, not complete semantic scale.
The actual partial serving check returns the requested sort.Search declaration/body,
validates three current source handles and trace/stdout agreement within 2,048 tokens
(`representative-serving-fixed/report.json`). Fresh worker acquisition is included
in its 15,836 ms; this is not a resident-host latency claim. Exact executable/profile
hashes live in `artifacts.json`; the semantic snapshot precedes a later defensive
requirement-link validation change, which does not affect semantic preparation.

**Evidence and economics.** Task execution proofs now carry requirement IDs only for
fresh explicit source links overlapping their observed target. Source ranges, hashes
and UTF-8 boundaries are checked; the observing test alone and similar prose do not
establish a join. The proof still requires all endpoint/test bodies and says satisfaction
is not established. `usage economics` reuses production receipt parsing, counts failed
attempts, refuses cross-log request attribution, reports missing categories/requests,
separates cached input and keeps observed costs apart from an explicit reuse projection.
It reports external checker identities, not execution attestation or hidden traffic.
Replaying all four preserved percent-decoder failures gives zero completed tasks,
127,095 known input tokens and 6,147 known output tokens, incomplete usage and unknown
prices. Completion economics stays null (`host-economics-report.json`). No new host
model run or success is claimed.

**Resources and checks.** The macOS host-CPU sampler is implemented using existing
Mach bindings and the existing admission hysteresis. Portable arithmetic/reset tests
pass; native compilation/observation and pressure behavior remain unrun. The focused
suite passes **540 tests** (three explicit checks remain ignored): 219 library,
82 core, 90 MCP, 20 multiroot, 83 neural retrieval, 16 response, two attribution,
15 task evidence, ten usage and three task-economics tests. A later retained-overview
fallback check and all 15 task-evidence tests pass after final link validation. The
test first tried inserting an invalid link and correctly received `InvalidRange`;
the repaired test now asserts that refusal. All-target Clippy with denied warnings,
the genuine no-default-features library check, formatting and Markdown links pass.

**Remaining:** general anchorless/behavioral retrieval and independent quality
acceptance; simultaneous coarse/selective body preparation and better overview content
where needed; full representative inference, useful serving and sustained maintenance/
compaction; broader static frameworks/runtime collectors and requirement satisfaction;
reliable free-form and memory-sensitive tasks with total/amortized cost benefits;
native Mac/Metal CPU/GPU parity, pressure, thermal/battery and signed-package acceptance.
Global GPU policy and optional embedding eviction remain implementation work.
These are recorded in the [pending inventory](context-ecosystem-plan.md#pending-work-inventory--2026-10-10),
not silently closed as environment-only TODOs. Learning remains frozen off.

## Broader evidence and representative workload — 2026-10-11

Continued on `experimental/context-ecosystem` with Rust 1.90, Linux x86_64,
a four-CPU quota and no exposed GPU. Foundry/test executables are debug builds;
model work uses the pinned ordinary CPU worker. These are diagnostic timings, not
a release throughput claim. The existing onboarding setup was reused.
Plans, exact source manifests, model receipts, failed launches and task transcripts
remain outside Git under
`/workspace/.context-foundry-tools/validation-runs/quality-scale-economics-v5/`.
Nothing here promotes experimental retrieval, selects a generally useful model or
releases a package. Full repository/model coverage and native hardware acceptance
are separate from the completed implementation checks below.

**Evidence and retrieval implementation.** `python-http-v6` adds direct router
definitions in explicitly configured Python package initializers, including relative
`from . import router`. Competing module/package layouts, re-exports, cycles and
escaped aliases remain incomplete. Package changes invalidate the service proof.
The real FastAPI package request passes with FastAPI 0.141.1, Starlette 1.6.0 and
httpx 0.28.1. The Flask 3.1.2 execution fixture imports **19 observed relationships**,
including a dynamically selected callback, and excludes the unexecuted error branch.
Changing the observing test after reopening the store suppresses its observations.
The earlier direct CPython fixture also passes (12 observations). The first Flask
attempt lacked the framework on system Python; the retained successful run uses the
existing explicit `FOUNDRY_FRAMEWORK_PYTHON` environment. Indexing never runs Python.
These are real consumers of owned fixtures, not general collectors or inferred
requirement coverage.

Experimental prose retrieval keeps up to two disjoint matching documents from a
long unit within the existing 256-document discovery window, four-item file cap and
response budget. Exact duplicate rendered handles are removed, dense locations keep
their ranges, and ordinary/anchored search remains unchanged. New regressions require
two separated passages, exact source bytes and partial semantic evidence under one
budget. A reproduced language ambiguity is also repaired: “a Go string” previously
anchored an unrelated function named `Go`. In experimental prose mode an unmarked
language name modifying a data/source noun is now treated as a qualifier. Explicit
call/name requests and marked/shaped identifiers retain symbol precedence. This
changes intent, not repository admission or language filtering. The real serving
check also exposed inconsistent admission: lexical selection suppressed the `Go`
anchor while the CLI/MCP semantic gate still skipped the model. Both paths now use
the same intent/filter rules. A failing-before/passing-after regression counts actual
fixture-provider calls with local and supplied root anchors; explicit symbol anchors
and ordinary search retain their existing no-call behavior.

| Frozen public questions | Ordinary delivery | Previous experimental (`6f2ad67`) | Final candidate |
|---|---:|---:|---:|
| Foundry development set | 1/8 | 2/8 | 2/8 |
| Rust inspected set | 0/6 | 3/6 | 3/6 |
| Rust additional set | 0/8 | 5/8 | 5/8 |
| Go in the combined corpus | 0/8 | 0/8 | 1/8 |

Every previous delivered span is retained. The new Go success is `sort.Search`;
other Go misses include two candidate-limit losses, one per-file-cap loss, two
packing omissions and two unresolved discovery cases. These are source-span checks
under the unchanged 2,048-token budget, not proof that an agent can complete a task.
General anchorless/held-out acceptance remains open. Traced/untraced outputs match;
sequential debug runs and filesystem warming do not support latency claims.

Traces now distinguish the matching index document from the delivered excerpt. The
attribution test caught a diagnostic error: whole-file outline handles were counted
as evidence that source bodies reached packing. Outlines are now excluded from that
inference, and a verified unit whose excerpt misses the label is reported as
`evidence_window`. Missing or inconsistent trace evidence stays unknown. Labels do
not enter ranking or delivery. Eight additional public Go questions are frozen before
querying the new combined corpus; they are source-inspected development probes, not
an independent holdout.

**Representative corpus.** The pinned combined source tree contains **69,565 regular
files, 68,056 distinct file contents and 324,780,308 bytes**. It combines Rust 1.90.0
(`1159e78c4747b02ef996e55082b704c09b970588`) with Go 1.25.0
(`6e676ab2b809d46623acb5988248d95d1eb7939c`). Go's archive SHA-256 is
`7e5813e23f221dc193dec557379fe22607baa6fc2e321a1b8e39b658848a5f5f`;
the exact combined manifest is
`b4c3e6a6c5d89d8ef2c05be8397c7b8434026d30483c38db417a9b6066389b80`.
There is no synthetic content repetition. The new ignored
`representative_corpus_workload_inventory` checks source admission and current hashes,
then serially fits admitted sources with the production body-v1 recipe (512-token cards,
64-token parents) and real tokenizer. Its report separates complete fitting from
embedding execution, distinct inputs from source locations, and current counts from
partial progress. Its serial offline fitting time is not a production bootstrap
throughput measurement. Whole-corpus inference is not established by the corpus file count.

The initial scan indexes **67,663 sources in 219,827 ms**, with no pending source
work. Of the 69,565 manifest files, 1,902 are not admitted; the scan's 816 explicit
exclusion events are a different measure from all ignored/not-admitted manifest
files. The 1,800-second inventory then reaches 65,783 files and stops at its declared
deadline (failure retained). A separately recorded continuation reuses the same
store/profile/manifest, rechecks the fitted prefix's current hashes and fits only the
remaining 3,782 files. It passes in **82,046 ms**, including 9,653 ms indexing and
4,559 ms prefix verification. No model or build runs concurrently with either phase.

Every admitted source now fits: **944,387 source-card locations**, 150,527,648 tokens
including duplicate inputs, and 78 admitted sources without cards. Rust contributes
53,955 sources / 501,682 cards; Go contributes 13,708 sources / 442,705 cards. Since
the first phase did not persist its full key set, combined distinct-input counts are
honest bounds: **782,716–831,067 inputs** and **131,313,896–141,600,550 tokens**.
Raw float vector payload is **2,404,503,552–2,553,037,824 bytes (2.24–2.38 GiB)**,
already above the default 2 GiB cache cap before metadata or ANN storage. Completing
this recipe therefore needs an explicitly larger cache allocation and sufficient disk,
as well as inference time. No default capacity or model choice changes. Fitting is
complete; full semantic preparation/serving and representative churn remain open.
The failed first phase and successful continuation are separate artifacts, with a
hash link to the previous report; neither the original deadline nor its outcome is
rewritten.

**Real preparation and reuse on that store.** The normal CPU worker uses the same
pinned Gemma BF16/tokenizer, body-v1 512/64 recipe, four CPU threads, single-input
calls and an 8 GiB worker ceiling. Its CPU lifetime ceiling is explicitly raised to
7,200 CPU-seconds for the 1,800-second wall budget; this does not change function or
cache identity. The corpus and OS file cache are already warm from the inventory;
“cold” here means an empty semantic cache, not a clean machine/install benchmark.
No other model or build runs during either phase.

| Phase | Budget / measured ms | New vectors / reused cached units | Model input tokens | Published vectors | Unpartitioned sources |
|---|---:|---:|---:|---:|---:|
| Empty semantic cache | 1,800 s / 1,620,928 | 2,678 / 0 | 1,332,355 | 2,678 | 67,639 |
| Resume | 120 s / 108,491 | 142 / 2,678 | 71,089 | 2,820 | 67,631 |

Both pause with `budget_exhausted` while preserving the publication reserve; neither
reports a provider/publication error or hits the cache cap. First searchable output
arrives after **14,636 ms**. Cold provider calls take **1,569,857 ms (96.85% of elapsed)**,
versus 8,157 ms fitting, 9,371 ms rendering and 1,523 ms publication. The worker's
process counters show about 3.9 CPU cores used. This recipe's bottleneck is inference.
The resume adds only new inputs and publishes one delta with zero base-index rebuilds.

After resume, only **32 sources have partitions**: 3,775 current eligible locations,
2,820 cached/searchable, and 955 still missing, plus 67,631 unpartitioned sources.
A partition is not proof that all of a file's evidence is ready. At the cold phase's
849 input tokens/second, a straight token-rate extrapolation of the complete workload
is about **43–46 provider-hours**. That is an illustration, not measured completion:
input-length mix, per-call overhead and later maintenance/publication costs differ.
Full representative semantic coverage and sustained churn remain unvalidated. More
file-parsing threads alone would not remove the measured model-call cost.

The first real serving check retains its failure: the semantic decision was `off`
because of that incidental `Go` anchor. After the admission repair, the same query,
profile, store and 2,048-token budget pass. The trace records a real dense decision
with 64 hits and partial coverage; the final response delivers `sort.Search` in
**2,042 o200k tokens**. All five returned source handles match current file hashes
and bounds, and the trace's delivery equals stdout. The fresh CLI process takes
15,823 ms including worker startup; this is not resident-host latency or general
semantic-quality acceptance. See `representative-serving-fixed/report.json`.

**Uncached upstream host pair.** Both arms use the same pinned percent-encoding 2.3.2
seeded repair, unchanged external oracle, protected-file checks, Qwen3-4B-Instruct-2507
Q4_K_M model, OMP 18.6.0, four CPU threads, 768 output tokens per request and 900-second
task budget. The new recipe exposes the package's **18 unchanged upstream unit tests**
through a public checker; tools are read/edit/bash with exact replacement edits.
The original upstream source passes those tests. No other model or build runs during
the pair. The raw uncached report mistakenly labels the tools read/write/bash; the
actual launcher and tool transcript use edit, and the harness label is corrected.

| Arm | Complete / unknown requests | Known input / output / cached tokens | Measured task ms | Result |
|---|---:|---:|---:|---|
| Full files | 3 / 1 | 27,000 / 1,216 / 0 | 901,222 | Host deadline; external oracle fails |
| Composed | 4 / 0 | 22,673 / 1,196 / 0 | 591,139 | Host completes; external oracle fails |

Both preserve protected files. The composed patch passes the upstream unit tests but
turns `%A` into `%` in the external oracle; later oracle assertions are not reached.
Public-test success is therefore insufficient evidence of task completion. `completion_comparison` remains
`not_established`; incomplete usage is never replaced with its known subtotal.
Model acquisition, common fixture setup, energy and amortized maintenance remain
explicit exclusions from the paired task clock. Per-arm fixture verification and cache
reset are experiment setup outside that clock.

A separately frozen pair enables ordinary prompt caching and erases the server's
sole slot before each arm. `--cache-ram 0` prevents saved in-memory slot snapshots;
the operator supplies an empty `--slot-save-path` solely to enable erase actions.
The first launch returned HTTP 501 before any task request because that option was
missing; it is retained as setup failure, then corrected. No save/restore is called.
Erase receipts and observed cached tokens are retained. The task/oracle/budgets remain
fixed. Completion accounting now distinguishes total input from uncached input and
refuses cached totals greater than input; neither ratio establishes monetary cost.

| Cached arm | Complete / unknown requests | Total input / cached / uncached input | Output | Task ms | Result |
|---|---:|---:|---:|---:|---|
| Full files | 7 / 0 | 73,462 / 61,914 / 11,548 | 2,967 | 784,812 | Host completes; patch does not compile |
| Composed | 1 / 0 | 3,960 / 0 / 3,960 | 768 | 145,715 | Host completes without tool calls or edits; original bug fails |

The erase receipts record zero tokens before the first arm and 12,699 erased before
the second. Both preserve protected files. All four arms' tool transcripts pass the
workspace/checker audit, and all preserve the upstream test module byte for byte
(`public-test-integrity.json`); no hidden oracle feedback is supplied. The composed cached
arm stops at its output limit without attempting the repair. Complete metering of a
failed task does not earn a cost-per-completion comparison. These fixed-order, single
public repair attempts do not establish causal, multi-file or memory-sensitive
benefit. Cached processing is a separate measure from total supplied context, and
OMP's uncached input count must not replace the gateway's total input count.

**Native validation handoff.** The ordinary Linux CPU worker emits a portable receipt
for four literal short inputs (single calls versus a four-input batch) and one exact
8,192-token input. It passes in **54,052 ms** under the 8 GiB ceiling, with finite,
normalized 768-dimensional outputs and matching short-input vectors. The receipt
records worker/model/tokenizer/geometry identities and exact token IDs. The comparator
requires identical inputs and descriptors except the intentional CPU/Metal adapter
revision, normalization error at most 0.0001 and cosine at least 0.9999. Its rejection
fixtures pass. The CPU receipt is `cpu-embedding-receipt.json` in the artifact directory.
There is no macOS runner or exposed GPU here, so **Metal comparison has not run**.
[Exact handoff commands](measurement-handoff.md) cover native capture/comparison;
existing batch/repeat/limit/isolation/resource and signed-install checks remain owed.


**Implementation checks.** Core (80), multi-root (20), semantic retrieval (82),
boundary (20), response (16), task evidence (15), attribution (2), receipt comparison
(1) and the completion-cost accounting guard pass. The explicit FastAPI package,
CPython/Flask execution and real CPU receipt checks also pass. The full inventory
continuation and four frozen retrieval comparisons pass with the limitations above.
All-target Clippy with warnings denied, the genuine library build without default
features, formatting and Markdown links pass. Native Metal comparison remains ignored
until a real receipt is available. No whole-suite or signed-release pass is claimed.


## Fair preparation and evidence delivery — 2026-10-11

Continued on `experimental/context-ecosystem`, Rust 1.90, the same four-CPU Linux
quota and no GPU. Frozen plans, raw reports, failed attempts and model/tool logs
remain outside Git under
`/workspace/.context-foundry-tools/validation-runs/quality-scale-economics-v4/`.
The existing onboarding environment and tool activation were reused; no environment
configuration change or publication was needed. This checkpoint is experimental.

**Task composition and real host.** The composer incorrectly promoted file-outline
handles to full source bodies. Those ranges swallowed the selected caller functions
during coalescing, and the large resulting unit degraded to an address under the
2,048-token budget. Outline-only candidates now stay out of the source-seed window.
Ordinary outlines, source identity, protected task state and final-read validation
are unchanged. The new regression delivers the caller's public checks without the
64 unrelated functions. All 15 task-evidence tests pass.

One frozen matched pair repeats the expanded parser task with OMP 18.6.0 and the
same pinned Qwen3-4B-Instruct-2507 Q4_K_M model, four CPU threads, public checks,
768 output tokens per request and 900 seconds per arm. No other model or build ran
during the pair. The exact composition-only executable and test binary are recorded
in `host-artifacts.json`; the later retrieval/scheduler edits were built afterward.
The final source in **both arms passes the unchanged external oracle**, including
Unicode trimming, additional delimiters and borrowed slices. Protected files stay
unchanged; `host-tool-audit.json` confirms only the repair file and visible checker
were used. This does not establish observed requirement coverage in the product.

| Arm | Requests complete / unknown | Known input / output / cached tokens | Measured task ms | Host completion |
|---|---:|---:|---:|---|
| Full files | 4 / 0 | 19,474 / 794 / 0 | 451,453 | Completed |
| Composed | 10 / 1 | 40,844 / 1,349 / 0 | 901,814 | Task deadline; final patch passes |

The composed run needs several compiler/test feedback cycles and reaches its limit
during a subsequent request. Its complete usage remains unknown, while its known
input already exceeds twice the successful control's. `completion_comparison` stays
`not_established`. The prior negative run is retained below. Passing this patch is
useful new evidence, not proof that this fix caused a general task-quality gain or
reduced cost. The fixture is artificially expanded and the helper is named; this is
neither realistic multi-file acceptance nor anchorless retrieval acceptance. Model
acquisition/loading, shared fixture setup, energy and amortization remain outside
the paired task clock. Monetary benefit is unestablished.

**Retrieval candidate rejected.** A bounded passage-locality reranker used the same
256-document discovery window, 2,048-token responses and frozen source/span labels.
It added no model, index schema or source scan. The previous experimental executable
is `fe37eff3` (SHA-256 `a11838c6578dd780e53c7b73c66c1e01a8f7c48f66328ceee43850f870bf0f39`).

| Frozen public probes | Legacy delivery | Previous experimental | Passage candidate |
|---|---:|---:|---:|
| Foundry, eight development questions | 1/8 | 2/8 | 2/8 |
| Rust, six inspected questions | 0/6 | 3/6 | 2/6 |
| Rust, eight additional questions | 0/8 | 5/8 | 5/8 |

The candidate loses `binary_search_by` at packing and adds no delivered spans. It
was removed; the previous experimental ranking remains. `retrieval-summary.json`
and the three `retrieval-*/` directories retain every miss and unknown attribution;
unknown is not counted as delivered. `rejected-locality.patch`, executable and
`rejected-locality-decision.json` preserve the rejected implementation. The eight
additional questions were frozen before execution but are public, author-visible
library probes, not an independently reviewed holdout. There is no general
anchorless acceptance or task-quality result here. Traced and untraced responses
match. Sequential debug executables and filesystem warming do not support a
ranking-latency claim.

The rejected design consulted Tantivy **0.26.2**,
`postings/segment_postings.rs::SegmentPostings::seek` and
`postings/postings.rs::Postings::positions`; the upstream
`block_segment_postings.rs::test_block_segment_postings_seek` was inspected, not run.
The prototype used nondecreasing seeks over the bounded candidates for 009 T005;
no upstream change or continued Foundry passage reranking is shipped.

**Bounded preparation turns.** The CLI and MCP preparation paths now yield a
contended source wave after eight new inputs. This prevents one large file from
consuming every inference turn before later priorities are visited. Queued cursors
retain only source bounds, and resume by checking current partitions/cache keys;
MCP also excludes in-flight and prefetched inputs. No new persisted state or model
slot is introduced. Replays cost cache lookups and occasional source rendering.
The limit counts inputs, not wall time, and does not increase inference throughput.

The frozen Rust store is copied into two experiment-owned stores and semantic state
is purged in each copy. The original 53,955-source store stays untouched. Both arms
use the same 16 frozen size/area priority paths, ordinary CPU worker, two threads,
batch eight and 180-second budget; copy/purge and existing lexical indexing are
excluded. Admission stops early when insufficient time remains for another call.
The candidate reaches six requested files with current cached vectors versus one
for the previous executable; three requested files are fully cached versus one.
It publishes 87 current vectors in 166.8 s versus 70 in 174.7 s. First searchable
coverage is 15.1 s versus 14.7 s. Distinct input lengths differ between arms, so vector
counts are coverage observations rather than a controlled inference-speed ratio.

Candidate partition work is 0.9 s and replay rendering is 1.9 s, compared with 0.4 s
and zero for the baseline; inference still dominates at 149.8 s versus 160.6 s.
The tradeoff improves priority coverage in this bounded run, with the anticipated
CPU replay overhead. CLI cancellation/resume and MCP prefetch regression tests
also verify that accepted inputs are not inferred twice. Complete source/corpus
coverage and retrieval recall are separate requirements. Raw preparation, source-
hash-checked cache coverage and binary identities live in `priority-plan.json`,
`priority-report.json`, `priority-artifacts.json` and the `priority-*/` directories.

A separate 90-second candidate resume adds 24 new vectors (111 total) in 81.6 s,
with 23 partitioned sources and 53,932 still unpartitioned. Existing progress becomes
searchable after 4.0 s; one delta is published and no generation is rebuilt. The
same six priority files have vectors and the same three are fully cached. This
resume preserves progress but does not improve priority breadth in its short budget.
Run counters include repeated cache lookups; they are not distinct corpus coverage.

**CPU threads and batch size.** The ordinary isolated Linux worker embeds the
middle body card from each of 16 frozen Rust files across library/compiler/tools/tests
and approximately 1/4/16/64 KiB source sizes. These are 16 distinct inputs totalling
3,452 tokens, using body-v1, 64 parent tokens and a 512-token card cap. The exact
Gemma BF16 model, tokenizer and worker are those pinned in the preceding checkpoint;
function digest is `701e4ff37f617ee634c299754b745180deb20d65455852aaca6194f44d75d61f`.
The plan allows 900 seconds, uses two reversed rounds and stops on an error or
vector mismatch. All eight arms finish in 135.9 seconds without a concurrent build
or model. Every vector is finite, normalized and 768-dimensional. Minimum cosine
against the first arm is 0.9999952; maximum absolute coordinate difference is
0.0003921. Changing only threads with batch one is identical in this sample.

| CPU threads | Inputs per call | Inference ms, two rounds |
|---|---:|---:|
| 2 | 1 | 7,379 / 6,569 |
| 4 | 1 | 3,898 / 4,001 |
| 2 | 8 | 11,288 / 11,076 |
| 4 | 8 | 6,527 / 6,340 |

Each acquisition separately takes 8,954–9,365 ms, including verification and worker
startup/loading. Four threads help this sample; larger batches hurt it. These are
real inference observations, not an explanation of the backend's bottleneck, a
full-scale throughput result or evidence of retrieval quality. The diagnostic is
`tests/semantic_scale.rs::representative_cpu_batch_comparison`; frozen selection,
inputs, vectors and each progress row are in `cpu-batch-plan.json` and `cpu-batch/`.
No global thread/batch default changes. GPU/Metal behavior remains untested here.

The separately frozen follow-up applies **four CPU threads and batch one** to a
fresh semantic-only copy of the same Rust store, keeping the candidate, priorities,
model, representation and 180-second budget unchanged. It publishes **305 distinct
vectors covering 326 current source units** in 162.9 s. All **16/16 priority files**
have current vectors; **5/16** are fully cached. First searchable coverage is 13.7 s.
Input volume is 113,988 tokens, with 136.2 s provider wait, 2.3 s partition work,
8.7 s replay rendering and 0.2 s publication. Identical inputs can cover multiple
source locations, hence the distinct vector/source-unit counts.

This supports the selected runtime settings for this standalone CPU workload and
shows the replay overhead becoming more visible as inference speeds up. The
scheduling-only result remains the separate 1/16 to 6/16 comparison above; the
16/16 result also changes CPU/batch settings. It is one follow-up, not a randomized
whole-repository trial. Only 24 sources have partitions; **53,931 remain
unpartitioned**, and no required-span recall or complete-file coverage is inferred
from a file having some vectors. `priority-tuned-plan.json`,
`priority-tuned-profile.json`, `priority-tuned-report.json` and `priority-tuned/`
retain the inputs and outputs. No model/profile default was promoted.
The final candidate executable SHA-256 is
`3a95367d6f2e90e1673119788bd5d776977cea28108d2e4180c1fd5ad5b9b3a0`.

**Checks and remaining scope.** Focused checks pass: 70 preparation integration
checks, 80 neural retrieval checks (one ignored), 85 neural unit checks, 15 task-
evidence checks, 16 response checks and 20 multiroot checks. After removing the
rejected ranking candidate, all 78 core checks pass. Rust 1.90 formatting, default
and no-default-feature all-target Clippy with warnings denied, Markdown links and
`git diff --check` pass. The real CPU comparison and priority coverage diagnostics
were explicitly executed; the real OMP harness records outcomes rather than turning
its execution into a task-quality pass. This was self-review, not independent review.

The [pending inventory](context-ecosystem-plan.md#pending-work-inventory--2026-10-10)
retains general retrieval/held-out acceptance, complete unique-content 60–80k model
coverage, broader observed relationships/test evidence, useful generated knowledge
and realistic task/memory economics. These remain Linux-capable work, not all hardware
blockers. Native macOS/Metal/GPU/package acceptance still requires another runner.
013 remains frozen off. No release, push, hosted publication or default promotion
was performed.

## Retrieval, scale and host economics — 2026-10-11

Continued on `experimental/context-ecosystem`, Rust 1.90, Linux with a four-CPU
quota and no GPU. Artifacts and frozen plans are outside Git under
`/workspace/.context-foundry-tools/validation-runs/quality-scale-economics-v3/`.
This checkpoint does not promote defaults or establish release acceptance.
The saved final CLI executable is pinned by `candidate-artifact.json`, SHA-256
`a11838c6578dd780e53c7b73c66c1e01a8f7c48f66328ceee43850f870bf0f39`.

**Retrieval.** Final development delivery is **2/8 versus 1/8** on frozen Foundry
source and **3/6 versus 0/6** on the inspected Rust questions, under unchanged budgets.
General retrieval acceptance remains open. The existing opt-in scores distinct non-stopword coverage before
bounded adjacent-term proximity. Repeated or rare single terms cannot outweigh an
additional covered term. Alphabetic hyphenated prose is admitted while marked/shaped
symbols retain their normal behavior. Candidate windows, source checks and budgets
are unchanged. Regression fixtures cover repetition, proximity, compounds, global
anchor selections and explicit symbols.

The frozen `ebf28f8` source archive was indexed once for a same-store comparison.
The saved previous executable (SHA-256
`81b80c9d1cfd554771bcc6a69602d90ef7d48854a8856369967fa71f05897010`)
delivers **1/8** expected spans there; the candidate delivers **2/8**. This is a new
corpus/binary comparison, not a reproduction of the earlier checkpoint's 2/8 result.
Six public Rust-library questions were frozen before candidate evaluation. The
initial scoring-only comparison delivers **0/6** required spans in all three arms.
Some discovery losses move to packing, but final delivery remains inadequate.
The public probes are neither blind G2 nor task-completion evidence; the missing
preserved G2 inputs remain missing. No general retrieval improvement is established.
The diagnostic now supports pinned external corpora/labels and previous executables,
checks label hashes/ranges and distinguishes source-admission observations from
trace-attributed ranking losses. The existing private-key marker exclusion stays.

A follow-up guard, selected after the real partial-coverage probe, alternates lexical
and dense placement (lexical first) for experimental anchorless prose when semantic
coverage is partial. It keeps current unprepared-source evidence early instead of
letting a small prepared subset occupy every initial slot. Complete coverage,
explicit anchors and ordinary search retain dense precedence. The six-question
lexical comparison above predates this separate partial-coverage guard and is not
claimed as its acceptance result.

The next packing candidate follows the observed rank-2 losses: a callable hit in
long leading documentation now exposes up to 1,536 exact bytes from its declaration
head when the matching window would otherwise omit that head. Deep body matches
still retain their focused excerpt. This uses the existing source coordinates and
budget, with no new form or fabricated union range. The follow-up comparison uses
the same frozen sources/labels; having inspected the misses, it is development
evidence rather than a new holdout.

The first follow-up (`*-declaration-v4/`) exposed the intended declarations but
still delivered **0/6** required whole-line spans: handles began after indentation.
The indented-declaration regression now requires the whole line, clipped to the
original unit. With that correction (`*-declaration-v5/`), the candidate delivers
**3/6**, versus **0/6** from both legacy and saved previous executable. The recovered
spans are `from_utf8_lossy`, `make_contiguous` and `binary_search_by`. Foundry remains
**2/8 versus 1/8**. All traced/untraced responses match and fit 2,048 tokens. These
results neither relax labels nor establish complete behavioral support; remaining
Rust losses include low rank, per-file capping and a macro-contained definition.

The real Gemma comparison reuses the representative run's store and vectors, with
one query per executable and no additional preparation. Both requests succeed at
2,048 tokens and retain `semantic:partial`. The previous executable begins with
`RELEASES.md`; the candidate begins with a current lexical implementation in
`compiler/rustc_infer/src/infer/canonical/canonicalizer.rs`, followed by dense evidence.
The trace records `partial_prose_interleave_v1`. A separate final-binary repeat also
succeeds in 1,978 tokens. This verifies the placement policy, not relevance or recall
superiority on that query. Artifacts:
`partial-dense-plan.json`, `partial-dense-comparison/`.

**Representative scale.** Acquired the public `rust-lang/rust` 1.90.0 snapshot at
`1159e78c4747b02ef996e55082b704c09b970588`, archive SHA-256
`8dc476ed3ccd405304236e3ee737b365b49168f1cfebd4da537ee0b4380e96bf`.
It contains **55,075 regular files, 53,897 distinct file contents and 191,000,975
bytes**. File sizes p50/p90/p99 are 593/4,504/41,124 bytes. This is not a 60–80k
completion claim. No repository code ran during indexing. The ordinary walker admits
**53,955 sources in 138.070 s**, with 62 reported exclusions, zero scan failures and
zero parser fallbacks; ignore policy also affects admission.

Real Gemma CPU revision 3 used the existing verified worker, two threads, 8 GiB,
body-v1/parent64/card512, and 16 frozen size/area priority files. The four preparation
budgets total 1,140 seconds. Indexing plus preparation, query worker acquisitions
and checks took 1,260.971 seconds. The profile and raw phases are retained.

| Phase | Preparation seconds | Newly embedded inputs | Active index entries | Partial |
|---|---:|---:|---:|---|
| Cold | 556.129 | 230 | 230 | yes |
| Resume | 273.804 | 176 | 406 | yes |
| Edit/delete plus backlog | 113.255 | 81 | 480 | yes |
| Revert plus backlog | 114.069 | 96 | 582 | yes |

First searchable coverage arrived after **14.399 s** of cold preparation. Cold
partition computation took 0.533 s, provider wait 542.122 s and publication 0.139 s;
inference dominates this workload. Resume reused 17 partitions and 223 cached inputs.
Six prior vectors survived edit/delete and seven survived revert with unchanged
values. Source edits affect only the experiment's store snapshot, then both files
are restored. The delta phases also prepare backlog; their inference totals are not
pure changed-input costs. All four real query/ANN/current-source delivery checks pass
at 2,048 tokens; query plus delivery takes 0.582–0.825 s after separate 10.772–11.031 s
worker acquisitions. These checks establish serving integrity, not retrieval recall.

Final recorded coverage is **42 partitioned sources, 2,198 discovered cards, 582
searchable cards, 583 cache entries and 53,913 unpartitioned sources**. A release-notes
passage heads the last serving probe. Searchable partial coverage is not demonstrated
useful coverage of the whole repository. Representative bootstrap remains expensive;
full unique-content scale, task-relevant coverage and scheduling/backend improvements
remain open. The earlier repeated-family 80k success cannot substitute for this result.

The run exposed an accounting ambiguity: preparation's `sources`, `eligible_units`
and `missing_units` count examined work, not the whole corpus. `PrepareReport` now
also exposes `coverage`, the last publication's corpus counts only when source
revision/function/recipe are current. Stale or absent observations return `null`;
this is not a new disk audit. The real run predates this report-only addition; its
final corpus counts were read separately through ordinary `semantic status`.

**Host protocol and repair quality.** The retained Qwen2.5-Coder-7B backend fails
three native tool-call probes (`auto`, required, explicit formatting): replies contain
tool-shaped text rather than protocol calls. A pinned
`unsloth/Qwen3-4B-Instruct-2507-GGUF` revision
`a06e946bb6b655725eafa393f4a9745d460374c9`, Q4_K_M, passes the native call probe.
Artifact SHA-256 is
`3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597`,
2,497,281,120 bytes, Apache-2.0. A 300-second serial download stopped incomplete;
the bounded eight-range resume took 93.479 seconds and the final digest passes.
Those acquisition attempts are retained, not charged as zero. The ordinary local
llama.cpp server uses the existing pinned source, four CPU threads, zero GPU layers,
16,384 context, temperature/seed zero, thinking off and prompt caching disabled.
It is an operator-launched server, not an isolated production generation worker.

OMP 18.6.0 now makes actual file edits in both expanded-fixture arms. With read/write
tools, a 420-second task ceiling and 768 output tokens per request, all four repairs
still fail the unchanged external Rust checker. Each uses three completed requests,
with zero reported cached input. Protected files are unchanged.

| Task / arm | Input / output tokens | Measured task ms | Verified task |
| --- | ---: | ---: | --- |
| Parser / full files | 12,524 / 455 | 277,382 | Failed; rejects additional `=` |
| Parser / composed | 6,887 / 417 | 150,878 | Failed; rejects additional `=` |
| Overflow / full files | 11,381 / 496 | 252,744 | Failed; multiplication panic |
| Overflow / composed | 6,697 / 500 | 153,611 | Failed; multiplication panic |

Reports now calculate a paired completion comparison only when **both** independent
repairs pass and every request has complete input/output/cache usage. These failures
therefore return `not_established`. Measured task wall time includes context preparation,
gateway/host startup, requests, teardown and the oracle, with orchestration separated.
Common fixture setup, model acquisition/loading, energy and amortized maintenance are
outside that number. Monetary cost remains unknown. File-backed transcripts survive
outer cancellation. Public calibration tasks do not establish held-out task benefit.
These named-helper repairs compare initial evidence composition; they do not test
anchorless discovery or demonstrate that the retrieval repair caused task success.

The upstream repair uses published `percent-encoding` **2.3.2** (MIT OR Apache-2.0),
archive SHA-256 `9b4f627cb1b25917193a259e49bdad08f671f8d9708acfd5fe0a8c1455d87220`.
Five source/manifest/license hashes are checked before private workspace copies are
made; source is not vendored. The seed makes `src/lib.rs::after_percent_sign` consume
the live iterator before validation. The original passes the external positive
control; the seed fails it. The oracle compiles alloc/no-alloc and checks all 256 byte
escapes, malformed/incomplete mixtures, UTF-8 behavior and unchanged-input borrowing.
Both arms use exact-text `read,edit`, fuzzy edits off, 420 seconds and 768 output
tokens. Full source is 21,863 bytes across two Rust files, without synthetic padding.

Both upstream arms fail. Full files make an incorrect edit that loses `A` from `%A`;
composed context makes four completed reading requests but no edit. Both hosts exit
unsuccessfully at their own time limit (the separate outer timeout flag is false).
Measured task wall is **421,235 / 422,613 ms**. The full-file arm has one complete
request (**7,689 input / 636 output**) and one unknown request; the composed arm has
four complete requests (**17,712 / 287**) and one unknown request. These are known
subtotals, not complete attempt costs. Both recorded cache subtotals are zero with
one unknown request. The harness now exposes such known/unknown accounting directly;
this run's subtotals were recovered separately from its retained receipts.
No verified completion or economic advantage is established on the upstream task.
Artifacts: `upstream-task-plan.json`, `upstream-host-recipe.json`, `qwen3-percent/`,
`percent-known-usage.json`. An earlier build failed because a test deadline variable
was placed in the checker; that harness error was fixed before either model arm ran.

The separately frozen parser recipe enables `read,write,bash` and gives both arms
the existing visible fixture-check command, 900 seconds, a 420-second request-idle
ceiling and 768 output tokens. External assertions remain hidden and unchanged;
build output stays outside the protected workspace. The full-file arm fixes a Rust
type error and then an untrimmed value through actual public feedback. Its final
patch passes the independent oracle and preserves all protected files: **7 complete
requests, 37,848 input / 1,374 output tokens, zero cached input, 892,260 ms**. This
establishes one completed public calibration task, not representative task acceptance.
Artifacts: `public-check-host-plan.json`, `qwen3-parser-public-check/`.
One readiness attempt used the wrong health-route prefix and expired before any
host request; the server was stopped and the corrected route admitted the run.
The successful model readiness observation is 1,831 ms, outside both task clocks.

The composed arm **fails** the external oracle and exits at its own 900-second
limit after **12 requests** (11 complete, one unknown). Known usage alone is
**40,654 input / 1,802 output tokens**, already above the successful full-file arm;
the complete totals remain unknown. Known cached input is zero with one unknown
request. Measured wall is **901,755 ms**, including 458 ms context preparation.
Repeated edits retain semantic mistakes and introduce compiler errors; one write
tool invocation fails. Both arms preserve protected files. The tool transcript audit
shows reads/writes of `src/records.rs` and execution of the provided public checker,
without external-oracle feedback. `completion_comparison` correctly returns
`not_established`. This is a negative matched outcome for the composed-context
recipe, not evidence of completion-cost savings. The visible-test loop enables a
successful control but does not establish reliable Foundry-assisted task completion.

**Remaining work.** Linux work still includes better ranking/per-file selection,
macro-contained evidence, fresh retrieval acceptance, complete unique-content
60–80k preparation with useful coverage and inference/scheduling improvements,
and reliable multi-file/memory-sensitive host tasks with all bootstrap, maintenance
and request costs. The public repair failures remain evidence against accepting the
current model/recipe. Native macOS/Metal/GPU and signed-package checks require other
hardware; they are not the only unfinished work. The
[pending inventory](context-ecosystem-plan.md#pending-work-inventory--2026-10-10)
retains the broader ecosystem TODOs.

**Code checks.** 263 focused source, multi-root, semantic preparation/retrieval and
response tests pass (78 + 20 + 69 + 80 + 16), with the changed excerpt suites repeated
after the range correction. The completion-accounting guard passes; the external
attribution and representative-scale harnesses compile and their explicit runs are
reported above. Default and no-default-feature all-target Clippy pass with warnings
denied. These functional checks do not turn a failed model task into a pass.

## Linux worker, representations, evidence and scale — 2026-10-11

Continued on `experimental/context-ecosystem`, Rust 1.90, four quota CPUs and no
GPU. These results supersede the implementation gaps in the preceding checkpoint;
they do not establish release or macOS/Metal acceptance. Raw artifacts are under
`/workspace/.context-foundry-tools/validation-runs/`, outside Git. 013 stays off.

**Normal embedding worker.** The exact-source-checked Gemma unused-logit allocation
repair now applies to ordinary `embed-worker` builds. The retained diagnostic feature
is an alias. Pinned llama.cpp remains `b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea`;
the native recipe is `gemma2-pooled-output-v1`. CPU adapter revision **3** and Metal
revision **4** give the corrected workers distinct vector identities. Old profiles
and cache rows remain available with their original verified binaries; a new binary
does not pretend to implement the old identity. The new Metal identity is untested.

The ordinary Linux supervisor now starts the real worker from verified executable
and model descriptors. Before model/IPC threads, the worker enforces seccomp syscall
restrictions, affinity, hard address-space and CPU-time limits, and owner-pipe death
handling, reusing the local-generation resource implementation. No workspace paths
or network access are granted. Explicit development admission is still required.
Fake-worker integration checks exercise syscall denials, limits, malformed replies,
changed artifacts and owner death. The Linux packaging branch copies the executable
and pinned native notice; this is not a signed macOS installation check.

The real Gemma BF16 artifact remains SHA-256
`68bae29d62fb8c7d23e98d21fd4662753ddd636e6b62b8a70dfe059a9844f216`.
The actual tokenizer is `google/embeddinggemma-2@914f7f89142e33e77833254d9c9b90c3cef7303b`,
SHA-256 `4d777ef5bdc1aa36227abdfb77c3e49e7b9c892d16e1b6bda41c393504828be4`.
Eight short strings have identical tokenizer.json/GGUF token IDs. Single/batch and
retained-stock-vector comparisons pass cosine ≥0.9999 (observed minimum 0.99999528).
Through the normal supervisor and IPC path, two admitted **8,192-token** evaluations,
short single/batch comparisons and limit-plus-one refusal pass in **175,159 ms** under
8 GiB and two CPU threads. Peak RSS was not separately measured in that IPC run;
the earlier 2.72 GiB peak belongs to the diagnostic. An initial harness attempt
correctly refused 8,192 tokens against a 512-token card allowance; its log is retained.
Artifacts: `production-embedding-real-8192-admitted.log`,
`production-tokenizer-parity.{log,json}`, `production-embedding-linux-tests.log`,
and `production-linux-package.log`. Tested worker SHA-256:
`8bf85740d78f8c5b7212bc30a25e42596b6491e392f892f24e7db329dd3bf5ff`.

**Independent representations.** Opt-in `hierarchy-v1` keeps exact body-v1 inputs
and adds separately embedded parent and file inputs. Short scopes retain all source;
long scopes use bounded extractive declaration overviews. These are not averages of
child vectors or claims that omitted source is represented. Equal source ranges can
carry distinct input keys, in deterministic order. Tests cover migration, unchanged
restart, changed bodies/parents/files, revert, deletion and bounded retained-card
fallback. Existing body vectors remain reusable.

Current multi-source briefs also have independent vectors in the existing cache.
Their source dependencies, brief revision and transitive lineage are checked before
binding and again before delivery. CLI and continuous MCP preparation share this
path; brief changes wake an overview pass without forcing a whole-source rescan.
The overview layer scans at most 256 bindings rather than adding another ANN owner;
preparation refuses more than 4,096 brief rows or 256 eligible area briefs by name.
Area counts are separate from source ANN coverage. Exact input reuse is tested across
changed dependencies; stale/deleted source or replaced briefs cannot retain eligibility.
Real-model superiority of this recipe over body-v1 is still unaccepted.

**Relationships and observed evidence.** `python-http-v5` adds literal Flask routes
and directly registered same-file Blueprints, prefix overrides, method lists and
implicit HEAD/OPTIONS/static-route declarations. Source/configuration freshness and
whole dependency proof still apply. Dynamic/escaped bindings, unsupported converters,
nested/cross-file Blueprints and unrecognized registrations remain incomplete.
The explicit Flask 3.1.2 / Werkzeug 3.1.9 runtime cross-check passes.

`import-execution` accepts a bounded, completed producer artifact with positive
`calls`/`covers` observations, source hashes, command/test identity and an exact
artifact digest. It executes nothing. These are labeled `observed_by_producer`,
not compiler facts, full coverage or execution attestation. Task composition emits
an observation only when its caller, target and test source spans all survive the
same delivery budget; changed endpoint/test sources suppress it. A separately run
CPython fixture captures an actual `getattr` dispatch and 12 observations, imports
them through the CLI, and excludes the unexecuted error branch. Static, budget and
staleness checks also pass. Artifacts: `flask-runtime-test.log`,
`observed-runtime-test.log`, `observed-task-test.log`.

**Real-model 80,000-file lifecycle.** The admitted CPU worker, actual tokenizer,
768-dimensional vectors, cache and ANN publication completed a 43,235,460-byte
mixed-size corpus. It has **512 distinct content families repeated across 80,000
paths**, so 86,250 source windows share **528 distinct model inputs**. This validates
real-model integration and large provenance mappings; it is not 80,000 unique-body
inference throughput or a representative repository distribution.

| Operation | Elapsed | New inputs / calls | Result |
| --- | ---: | ---: | --- |
| Source index | 97,481 ms | — | 80,000 sources |
| Cold semantic preparation | 469,389 ms | 528 / 72 | Complete; 80,464 model-input tokens |
| Unchanged restart | 8,649 ms | 0 / 0 | Zero repartitioning |
| One stored-source edit plus deletion | 18,836 ms | 1 / 1 | One repartitioned source |
| Revert | 8,579 ms | 0 / 0 | Cached vector reused |

First searchable semantic publication arrived at 28,916 ms. Cold provider time was
270,731 ms and partition computation 70,671 ms; these are observed counters, not an
isolated CPU-utilization comparison. Other sandbox builds ran during parts of the
experiment. The separate real-query check loads the published index, embeds a query
in 436 ms, and returns verified source in 1,965 tokens; query plus context delivery
takes 1,148 ms, after 11,578 ms worker acquisition. It checks serving and source
identity, not recall quality on this repetitive corpus. The 512-file pilot and all
80,000-file reports are retained in `real-scale-pilot-512/`, `real-scale-80000/` and
their logs. Budget: two preparation rounds of at most 900 seconds; one round sufficed.

**Anchorless repair.** The opt-in candidate now uses two disjoint 128-document
windows for code and prose, interleaves them within the existing 256-document cap,
removes common prose stop words, and returns exact bounded excerpts around matching
body lines. The starvation/late-body regression passes with unchanged freshness and
budget checks. On the eight existing development labels, baseline delivery is **1/8**
and candidate delivery **2/8**; traced/untraced bytes match. Artifact:
`anchorless-diversified-v2/`. This is a measured local gain, not general or held-out
acceptance. One expected source, `src/ingest.rs`, is excluded by the existing private-key
marker policy; the policy was not weakened to increase this score. Historical G2
inputs remain unavailable, and the six other missing deliveries are still open.

**Paired 7B host tasks: negative.** OMP 18.6.0 runs the parser and overflow repairs
once per arm, with 64 unrelated functions, an immutable external Rust oracle and
protected harness files. Model: `Qwen/Qwen2.5-Coder-7B-Instruct-GGUF` revision
`13fb94bfda8c8cf22497dc57b78f391a9acb426a`, Q4_K_M,
SHA-256 `509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c`,
4,683,073,536 bytes. The pinned llama.cpp server uses four CPU threads, no GPU,
16,384 context, temperature/seed zero and 768 output tokens. Each arm has a 420-second
limit. No retries, manual patches or oracle relaxation are counted as task success.

| Task / arm | Input / output tokens | Context preparation + host | Verified task |
| --- | ---: | ---: | --- |
| Parser / full files | 4,003 / 157 | 0 + 128,789 ms | Failed; no edit |
| Parser / composed | 2,128 / 164 | 451 + 74,132 ms | Failed; no edit |
| Overflow / full files | 3,633 / 154 | 0 + 117,255 ms | Failed; no edit |
| Overflow / composed | 2,059 / 159 | 483 + 72,034 ms | Failed; no edit |

Each arm has one complete gateway request and zero cached-input tokens. The model
emits JSON write requests as assistant text, so the host applies no patch. Host exit
success is not task success. The baseline parser proposal would also fail compilation.
Smaller prompts/latency do not establish benefit per verified completion. This is a
public calibration, not held-out acceptance or a realistic multi-step benchmark.
Gateway coverage remains `whole_run_unverified`; complete local receipts do not prove
visibility into arbitrary host/provider traffic. Server identity is operator-observed,
not remotely attested. The transport alias's prices do not describe this local model.

The table includes context preparation and all recorded requests, but excludes common
model acquisition/loading, oracle validation and energy. Loading took 2.48 s; acquisition
includes a partial sequential download plus 234.97 s of resumed parallel transfer.
No total monetary or amortized cost claim is made. Artifacts:
`coder7-{parser,overflow}-v2/`, their logs, `coder7-task-server.log`,
`../generation-fixture/coder7-artifact.json` and download logs. The earlier harness attempt used an invalid
Rust path, failed its positive control before any model request and is retained
separately. The server was stopped after the four arms completed.

macOS/Metal/signing, representative unique-content scale, held-out retrieval and
realistic multi-step task/memory economics remain distinct acceptance work; they
are not all macOS blockers.

**Code checks.** Rust 1.90 default all-target Clippy, no-default-feature Clippy and
native `embed-worker` all-target Clippy pass with warnings denied. Across the ordinary
suite and focused continuation runs, 1,064 tests pass (latest result per target,
without counting reruns twice), plus the separate 21-case crash/recovery harness.
The native embedding binary adds two passing unit tests; its three Linux supervisor
checks also pass again. Explicit real-model, Flask and CPython checks are reported
above, separately from fixture counts. Formatting, shell syntax and Markdown links
pass. macOS-only tests and signed installation checks did not execute on Linux.

The first broad run and a continuation exposed a race in the existing HTTP
cancellation fixture: all 6,000 files could finish before DELETE. Its original
isolated pass was insufficient. A test-only, workspace-scoped cancellation hold now
keeps an admitted scan at a known boundary without blocking other workspaces or
fault points. The full MCP target passes **90/90**, and semantic retrieval passes
**79/79**. The library passes **216 tests** including brief-only area maintenance,
change/forget wakeups without source reselection, exact duplicate-input reuse and
refusal to create an area vector containing only its generic prefix. This fixes the
fixture's synchronization without weakening stream-loss, overload, cancellation or
partial-scan assertions.

Logs: `ecosystem-default-suite.log`, `ecosystem-remaining-suite.log`,
`final-area-tests.log`, `final-lib.log`, `final-integration-suite.log`,
`final-{default,no-default,native}-clippy.log`, `final-native-tests.log`,
`final-recovery.log`, `final-doctests.log` and `final-links.log`. This is sandbox
implementation/validation, not independent cross-lab review or release acceptance.

## Feasible Linux repairs and task comparison — 2026-10-10

Continued on `experimental/context-ecosystem`, Rust 1.90, with four visible quota
CPUs and no GPU. Raw results remain outside Git under
`/workspace/.context-foundry-tools/validation-runs/`; weights stay in the sibling
`embedding-fixture` and `generation-fixture` directories. These are sandbox results,
not release, macOS or production Linux embedding acceptance. 013 remains frozen off.

**8,192-token allocation repair.** The pinned llama.cpp
`b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea` unconditionally reserves a vocabulary-sized
logit output buffer. Its `src/models/gemma-embedding2.cpp::graph` produces embeddings
and no logits. The new Linux-only `embed-worker-pooled-diagnostic` feature applies
one exact-source-checked replacement in `build.rs`, suppressing that unused allocation
only for Gemma embedding mode. It reports the separate recipe
`gemma2-pooled-output-v1`. Ordinary native builds and production admission are unchanged;
this is an experimental overlay, not an upstream fix or a silently reused cache identity.

The real Gemma BF16 artifact is SHA-256
`68bae29d62fb8c7d23e98d21fd4662753ddd636e6b62b8a70dfe059a9844f216`, from
`ggml-org/embeddinggemma-2-GGUF@bfcd298762cc34d0357ece5ebdd31791a3a374d8`.
The CPU diagnostic runs under `linux-seccomp-fd-v1`, two CPU threads, an 8 GiB
address-space ceiling and fixed cosine ≥0.9999 / unit-norm tolerance 0.0001.
All three completed runs pass eight short sequences singly, batched and reversed,
Unicode/CRLF inputs, full-window repeatability, a near-full heterogeneous batch,
finite/unit-norm outputs and limit-plus-one refusal.

| Runtime recipe / token window | Diagnostic wall time | Peak process RSS | Minimum cosine |
| --- | ---: | ---: | ---: |
| Unmodified upstream / 2,048 | 30,950 ms | 3,073,216,512 B (2.86 GiB) | 0.99999528 |
| Pooled-output experiment / 2,048 | 29,136 ms | 918,056,960 B (0.85 GiB) | 0.99999528 |
| Pooled-output experiment / 8,192 | 330,477 ms | 2,916,052,992 B (2.72 GiB) | 0.99999528 |

The two patched runs additionally compare all eight short-input vectors against the
retained unmodified 2,048-token run, checking model and token-input hashes. There is
no stock 8,192-token reference under this ceiling: full-window checks establish
repeatability and single/batch agreement, not full-window cross-runtime parity.
The first patched 8,192 attempt exhausted its 600 CPU-second allowance; the final
predeclared attempt passed with 900 CPU seconds and a 480-second outer wall limit.
The memory ceiling and numerical tolerances were unchanged. Logs retain both attempts.
Model downloading overlapped part of the final run; timings are observations, not an
isolated speed comparison. The 2,048-token peak-RSS reduction is approximately 70%.

Artifacts: `pooled-output-v1/{baseline-2048,pooled-2048,pooled-8192-cpu900}.{log,json}`
and the failed `pooled-8192.log`. To reproduce after the native build prerequisites
in [CONTRIBUTING](../CONTRIBUTING.md), set `LLAMA_CPP_DIR` to the pinned object store,
`FOUNDRY_EMBED_CPU_GGUF` to the verified model, and `FOUNDRY_EMBED_CPU_CONTEXT` to
the desired window. Run the ignored `cpu_pinned_gemma_batch_geometry` binary test
at checkpoint `5b93332` with `embed-worker` for the stock reference and
`embed-worker-pooled-diagnostic` for the overlay. Current ordinary builds include
the repair; use the retained stock artifact for comparison. Extract the `CPU_EMBED_RESULT` JSON from the first log and supply
it as `FOUNDRY_EMBED_CPU_REFERENCE` for the comparisons. Pipe output to a parent-owned
log: the confined test process cannot write regular files. Keep the 480-second outer
timeout for the 8,192 run. Profile `tokenizer.json` parity, independent parent/file/area
vectors, long-context retrieval quality and worker/device/package adoption remain open.
Prefer a pinned upstream repair for production and bind its runtime identity first.

**Anchorless retrieval.** One conservative candidate was selected before evaluation:
with the existing experimental behavior flag, anchorless alphabetic prose no longer
promotes incidental exact-name runs ahead of lexical body matches. A regression with
48 unrelated `read` definitions reproduces eviction of the relevant body from the
32-slot context window; the repair delivers it under the existing budget and freshness
rules. Locator/code/path/graph queries, admitted anchors and ordinary search retain
their contracts. Dense evidence retains precedence. No cap, model or score formula
was increased. This is a repair for that specific failure, not general acceptance.

The repository attribution harness now excludes its own evaluator source before
freezing identities, avoiding retrieval of literal labels as repository evidence.
Both arms still deliver **1/8** expected bodies on the eight public development labels:
three candidate-limit losses, two packing omissions and two unknowns per arm. Traced
and untraced bytes match. Raw labels, exact source hashes, traces and outputs are in
`anchorless-prose-v1/`. These labels are neither held out nor the unavailable historical
G2 set. The candidate stays opt-in; broader anchorless usefulness is still unproven.

**Stronger-model host comparison.** The existing free-form OMP harness now accepts
the explicitly pinned Qwen3-1.7B Q8_0 model and an `expanded` fixture mode. The latter
adds 64 unrelated Rust functions, identically in both arms. This is synthetic evidence
volume, not realistic task diversity. Model artifact: `Qwen/Qwen3-1.7B-GGUF` revision
`90862c4b9d2787eaed51d12237eafdfe7c5f6077`, SHA-256
`061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a`
(1,834,426,016 bytes); Apache-2.0 metadata/license retained outside Git. OMP 18.6.0 / Bun
1.3.14 used the unmodified pinned llama.cpp server, launched with four CPU threads,
16,384 context, one slot, temperature zero, thinking off, prompt cache disabled and 512 output tokens
per request. The `glm-5.3-flash` name is only the transport alias. No paid API was used.

One attempt per arm, each capped at 180 seconds, edited a fresh public parser fixture
using real `write` calls. The separate Rust checker accepted a positive control and
rejected the original before either task. Actual tool calls touched only the allowed
parser path; protected files remained unchanged. These are ordinary local host
processes, not a claim of execution isolation or remote backend attestation.

| Arm | Verified task | Preparation | Host elapsed | Requests | Observed input / output tokens |
| --- | --- | ---: | ---: | ---: | ---: |
| Full files | Failed: missing `=` panics via `unwrap()` | <1 ms | 180,821 ms | 4 | ≥12,821 / ≥896; one request unknown |
| Foundry composed, 2,048-token allowance | Failed: Rust does not compile | 874 ms | 61,360 ms | 2 | 4,488 / 346; complete |

The full-file run reached the host time limit after an output-truncated, unexecuted
write and two executed writes. Three model receipts are complete; cancellation leaves
the fourth request's usage unknown, so full-run totals remain null in the report.
The composed arm completed its host process after one write, but its patch failed the
compiler. Every complete receipt reported zero cached input tokens. Retain the lower
observed token count as an observation, **not cost per verified completion**: both tasks
failed, one total is incomplete, and model acquisition/loading, energy and amortized
bootstrap/maintenance costs are not included. This comparison does not establish a
memory benefit, a useful model/recipe, representative quality or total-cost savings.

Artifacts: `qwen3-expanded-host/report.json`, per-arm prompts/sessions/receipts, patches
and checker logs, plus `qwen3-task-server.log`. Reproduction uses the existing ignored
`host_task::free_form_parser_repair_with_full_files_and_foundry_context` gateway test,
the documented OMP/Bun/rustc paths, `FOUNDRY_HOST_TASK_MODEL=qwen3-1.7b` and
`FOUNDRY_HOST_TASK_SOURCE_SCALE=expanded`; choose a new outside-Git output directory.
The local server was stopped after the run. Earlier 0.5B negative results remain below.

**Code checks.** All 413 focused default checks pass (`lib`, `core`, `multiroot`,
`neural_retrieval`, `response`, `retrieval_attribution`, `task_evidence`), as do the
two ordinary native embedding tests. Default and pooled-diagnostic all-target Clippy
pass with `-D warnings`. The real-model diagnostics and repository attribution run
above are additional opt-in checks, not included in the 413. The prior full-suite
baseline remains below; no unavailable platform or zero-test target is counted.
The added prose-plus-dense precedence assertion also passes, followed by final default
all-target Clippy, the build without default features, formatting and documentation links.

**Remaining from the requested list:** real-model 60,000–80,000-file semantic validation;
independent parent/file/area vectors; broader framework/dynamic callers and observed
test coverage; demonstrated representative task-quality and total-cost benefits; an
effective general anchorless repair. The allocation failure is closed for the Linux
diagnostic, while its production adoption and macOS/GPU acceptance remain open.

## Ecosystem closeout audit — 2026-10-10

Reviewed the implementation through `1cbb6bd` on `experimental/context-ecosystem`
against the active specs, source modules, CLI/MCP wiring and tests. This is an
in-session audit, not independent review or release acceptance. The
[capability map](context-ecosystem-plan.md#what-exists-and-what-is-missing) now links
implemented behavior to code/tests and distinguishes missing capabilities, pending
quality/scale/platform acceptance and conditional extensions. Corrected stale claims
that progressive publication, matching changed-input restart reuse, live host-view
application and Linux CPU admission were still wholly missing. Production code,
defaults, model pins and numerical tolerances are unchanged by this closeout.

**Fresh verification:** restored Linux checkout, Rust/Cargo 1.90, offline locked
dependencies, two test threads. **170 checks passed, none failed:**

| Executed checks | Passed | Boundary established |
| --- | ---: | --- |
| `boundary`, `task_evidence`, `semantic`, `retrieval_attribution` integration targets | 135 | Source/graph freshness, dependency publication, bounded whole-proof delivery and diagnostic identity checks |
| `model_resources::tests` and `task_evidence::reference::tests` ordinary library tests | 9 | Resource admission/hysteresis and callee/test syntax classification |
| `task_context` integration target | 21 | Task recovery, protection, promotion/forgetting, grounded review and restart/fault behavior |
| Gateway `live_` ordinary checks and `exported_view_applies` | 3 | Owner synchronization, revocation/restart refusal, deadlines and exported-view replay |
| Opt-in live Linux CPU observation | 1 | Five affinity CPUs and a visible 400,000/100,000 quota (four CPUs); stable sampled identity |
| Opt-in installed OMP 18.6.0/Bun 1.3.14 live-view check | 1 | Resumed tool history, post-export owner edit, applied revision and final-request receipt with a scripted upstream |

The real-host check launches only its fixture workspace and loopback services; it uses
scripted replies, no model inference or paid provider. It does not re-establish task
quality or savings. Formatting and documentation links also pass. The prior full
1,050-test/default-and-native-Clippy results below remain the implementation baseline;
they were not rerun for this documentation-only closeout. Expensive model, scale and
free-form task diagnostics were not repeated because their inputs/code did not change.
Their earlier positive and negative outcomes remain historical, with their stated limits.

**Evidence availability after restart:** the checkout, toolchain and installed OMP/Bun
survived; the earlier `/tmp/foundry-repository-attribution-v1`,
`/tmp/foundry-semantic-scale-8192-v1`, `/tmp/foundry-freeform-host-v1` reports and CPU
embedding logs did not. Raw paths in earlier entries describe the prior runtime, not
currently available files. Their aggregate results, artifact identities and executable
harnesses are committed; this audit did not reanalyse unavailable raw records. For a
future rerun, choose a new outside-Git directory on a retained workspace path (for
example `/workspace/.context-foundry-tools/validation-runs/RUN`), retain the declared
identities and stop rules, and treat it as a new run. Checkpoint restoration does not
verify installation from the saved setup draft on a clean machine.

**At this audit checkpoint:** no effective anchorless repair or representative task-quality/cost
gain; no selected-model semantic acceptance at 60,000–80,000 files; the tested
8,192-token/8 GiB embedding allocation failure (subsequently repaired in the opt-in
Linux diagnostic above); independent parent/file/area vectors,
broader relationships and observed test coverage; native macOS/GPU/resource/package
acceptance. The [pending inventory](context-ecosystem-plan.md#pending-work-inventory--2026-10-10)
keeps these obligations and the conditional extensions explicit. macOS remains deferred
by the owner, and 013 remains frozen off. Closing this audit does not close those tasks.

## Linux relationships, attribution and semantic diagnostics — 2026-10-10

Work continues on `experimental/context-ecosystem` with Rust 1.90. macOS validation
remains deferred by the owner; 013 learning stays frozen off. No model weights, raw
host sessions, stores or retrieval traces are added to Git.

**Verification.** All **1,050** default Rust tests pass (11 opt-in tests ignored),
including the 6,000-file watcher/foreground/idle-read check. Default and `embed-worker`
all-target Clippy pass with
`-D warnings`; formatting and documentation links pass. The native embedding binary's
two ordinary tests pass. The first contended full-suite run hit the existing CPU-limit
test's eight-second wall assertion while native compilation competed for CPU; its
worker-exit and retained-work assertions passed. The isolated rerun passes that test
without changing its assertions; the full rerun exits successfully. No macOS-only
suite or zero-test target is counted as hardware acceptance.

**Relationship/evidence implementation.** `python-http-v4` adds bounded same-file
nested routers and direct imports from configured provider modules of the same service.
Import/declaration/registration spans must all be current and delivered for a task
relationship. Importer edits immediately suppress dependent evidence; publication
rechecks the service inputs atomically. Query dependency checks share one source-metadata
read per configured file. Unchanged scopes reuse their rows. Cycles, re-exports,
ambiguous module layouts, escaping aliases and early/multiple mounts remain incomplete.
Both opt-in real FastAPI 0.141.1 / Starlette 1.6.0 tests pass, including the cross-file
nested-router request. Indexing executes no workspace Python.

Compiler references additionally carry source-grounded Rust/Python callee-expression
spans and Rust test attributes. Function pointers/arguments are not classified as calls;
name-only tests and nested functions do not acquire an enclosing test's attribution.
These are still compiler references with runtime calls **unverified** and execution
**not observed**, not call-graph or test-coverage claims. The final delivery requires
all cited spans. Focused tests cover stale sources, fitting and whole-proof omission.

**Repository retrieval attribution.** Eight public Foundry development labels were
frozen before running either lexical arm, with the same 2,048-token budget. The
retained records bind query, exact source ranges/hashes, candidate events, final packing
inputs and exact delivered bytes. Traced and untraced delivery was identical.

| Mode | Required body delivered | Candidate-limit loss | Packing omission | Unknown |
| --- | ---: | ---: | ---: | ---: |
| Baseline lexical | 1/8 | 3 | 2 | 2 |
| Experimental behavior | 1/8 | 3 | 2 | 2 |

No improvement is established. Missing expected spans in a bounded candidate window
remain unknown. The diagnostic rejects mismatched hashes/queries, truncated traces and
missing/conflicting packing snapshots; duplicate removal alone is not a loss. These
labels are development observations, not held-out cases or the preserved Gemma G2 set.
The historical 249 G2 misses remain unattributed. Next retrieval work should inspect
ranking/packing at the observed losses, then evaluate a repair on fresh held-out inputs.
Raw records: `/tmp/foundry-repository-attribution-v1`, outside Git.

**Mixed-size semantic lifecycle.** 8,192 synthetic Rust files, 4,445,012 source bytes,
128-dimensional fixture vectors, fixture byte tokenizer, body windows of 512 tokens
and a 64-token parent prefix. This exercises production fitting, partition/cache reuse,
ANN and transaction paths, not selected-model segmentation or inference throughput.
Source indexing took 27,852 ms; the first semantic preparation finished in 216,950 ms,
with 15,872 eligible units, 9,226 unique embedded units and 6,646 cache reuses. Its first
semantic publication appeared at 417 ms after preparation started. The complete
index/cold/restart/edit/delete/revert sequence took 253,552 ms.

| Restart/maintenance step | Partitioned sources | New embedded units | Document calls | Elapsed |
| --- | ---: | ---: | ---: | ---: |
| Unchanged | 0 | 0 | 0 | 1,552 ms |
| One edit plus one deletion | 1 | 1 | 1 | 2,872 ms |
| Revert edited input | 1 | 0 | 0 | 2,932 ms |

All generations published successfully. The mixed-size test has a bounded preparation
budget and retains progress on failure; it does not claim 60,000–80,000-file semantic
acceptance. Native/Rust compilation overlapped this run on a four-CPU quota, so timings
are observations under contention, not an isolated performance comparison. Report:
`/tmp/foundry-semantic-scale-8192-v1/report.json`. Real-model scale, overflow/compaction
and continuous semantic/knowledge maintenance at the target corpus size remain open.

**Real embedding CPU numerics.** The selected EmbeddingGemma 2 BF16 artifact was
fetched at `ggml-org/embeddinggemma-2-GGUF@bfcd298762cc34d0357ece5ebdd31791a3a374d8`:
557,950,176 bytes, SHA-256
`68bae29d62fb8c7d23e98d21fd4662753ddd636e6b62b8a70dfe059a9844f216`.
The native runtime remains pinned to llama.cpp
`b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea`. The embedding binary now exposes its shared
numerical implementation in an ignored Linux-only test; the production Linux embedding
entry point still refuses. The diagnostic uses inherited read-only model descriptors,
`linux-seccomp-fd-v1`, two affinity CPUs and an 8 GiB hard address-space ceiling.

At **2,048 tokens**, eight sequences, single-versus-batch/reversed-order agreement,
heterogeneous lengths, full-window repeatability, finite/unit-normalized 768-vectors
and over-limit refusal all pass. Minimum cosine is **0.9999952796** against a 0.9999
threshold. Diagnostic inference elapsed 73,787 ms; this overlapped some repository
indexing and is not isolated throughput. At **8,192 tokens**, the run fails: pinned
`llama_context::output_reserve` tries to allocate **8,216 MiB** for all-token output
buffers. `decode` requires all tokens for pooled embeddings; merely disabling logits
flags does not fix this version. No runtime patch or larger resource limit was silently
substituted. This allocation is now a concrete long-context efficiency TODO.

An initial direct-to-file stdout/stderr invocation died with SIGXFSZ because the
sandbox installs a zero file-write-size limit. Repeating with parent-owned pipe capture
corrected the harness; the 2,048-token pass and 8,192-token failure above use that capture.
GGUF tokenization does not certify profile tokenizer.json parity. Production worker IPC,
macOS App Sandbox/signed bundles, Metal/GPU numerics, resource pressure and long-context
retrieval quality remain unverified. No GPU device is visible in this sandbox.
Logs: `/tmp/foundry-cpu-embedding-2048-piped.log` and
`/tmp/foundry-cpu-embedding-8192.log`.

**Free-form host calibration.** Installed OMP 18.6.0/Bun 1.3.14 used real local
Qwen2.5-Coder-0.5B-Instruct Q4_K_M replies through the existing gateway. The upstream
was the pinned llama.cpp server, CPU-only, two threads, 16,384-token context, temperature
default 0, one slot; host model overrides capped each request at 512 output tokens.
`glm-5.3-flash` is only the transport alias, **not the model tested**. GGUF SHA-256:
`1d9614638d18024d0fbb36575a15f1302a3adf044df10345688ec4f6e1c4ff32`.

One public T003 parser task, two fresh workspaces/sessions, matched goal and read/write
tools, one attempt per arm, 180 seconds per task. No candidate patches were supplied.
The immutable external checker compiles the resulting Rust and verifies whitespace,
empty keys/values, embedded `=`, Unicode and borrowed slices; protected files must stay
unchanged and no extra workspace files may appear. It rejects the original and accepts
a separate correct control. Model failures are retained, not repaired by the evaluator.

| Initial evidence | Verified tasks | Gateway input tokens | Of which cached | Output tokens | Host elapsed |
| --- | ---: | ---: | ---: | ---: | ---: |
| Full fixture files | 0/1 | 1,544 | 0 | 67 | 38,443 ms |
| Foundry compact composition | 0/1 | 2,483 | 858 | 141 | 51,051 ms |

Both returned inert JSON text naming an unavailable `parse_record` tool and made **no
file edit**. Each host exited successfully and each gateway recorded one complete
request, but each task failed its checker. Foundry context preparation added 1,525 ms.
The server retained a prompt-cache prefix across arms; cached tokens above are included
in total input, not additional tokens. Compiler work overlapped part of inference.
These latencies are not isolated causal comparisons. Gateway summaries retain
`whole_run_unverified`: the table accounts for observed requests, not a claim about
unobserved auxiliary host traffic. Monetary cost, energy, model build/load and amortized
bootstrap economics were not measured. There is no cost-per-verified-completion result
with zero completions, no demonstrated benefit, and no held-out task acceptance. A
capable tool-using model and broader matched tasks remain necessary. The server was
stopped after this run; raw sessions and receipts remain at
`/tmp/foundry-freeform-host-v1` outside Git.

**Reproduction.** Ordinary tests need no model or Python framework. Explicit diagnostics
refuse an existing output directory; keep it outside the checkout. The real FastAPI
checks require installed FastAPI/Starlette/httpx. Activate the retained Rust toolchain
and use fresh paths:

```bash
cargo test --locked --offline --test boundary real_fastapi -- --ignored --nocapture
FOUNDRY_ATTRIBUTION_DIR=/tmp/new-attribution cargo test --locked --offline --test retrieval_attribution repository_development_attribution -- --ignored --nocapture
FOUNDRY_SEMANTIC_SCALE_DIR=/tmp/new-scale FOUNDRY_SEMANTIC_SCALE_FILES=8192 timeout 720s cargo test --locked --offline --test semantic_scale -- --ignored --nocapture
```

For native tests supply CMake, libclang and the pinned `LLAMA_CPP_DIR` as documented in
[deployment](deployment.md), plus `BINDGEN_EXTRA_CLANG_ARGS="-isystem $(cc -print-file-name=include)"`
on this Linux compiler setup. Pipe both output streams to a parent-owned log:

```bash
set -o pipefail
FOUNDRY_EMBED_CPU_GGUF=/path/to/embeddinggemma-2-BF16.gguf FOUNDRY_EMBED_CPU_CONTEXT=2048 timeout 480s cargo test --locked --offline --features embed-worker --bin foundry-embed cpu_pinned_gemma_batch_geometry -- --ignored --nocapture --test-threads=1 2>&1 | tee /tmp/new-cpu-embedding.log
```


For the free-form host diagnostic, build the same upstream server outside the checkout:
export the pinned Git tree, configure CMake with `BUILD_SHARED_LIBS=OFF`,
`GGML_NATIVE=OFF`, `GGML_OPENMP=OFF`, `GGML_METAL=OFF`, `GGML_CUDA=OFF`,
`LLAMA_BUILD_COMMON=ON`, **both** `LLAMA_BUILD_TOOLS=ON` and `LLAMA_BUILD_SERVER=ON`,
`LLAMA_BUILD_TESTS=OFF`, `LLAMA_BUILD_EXAMPLES=OFF`, `LLAMA_BUILD_UI=OFF`, and
`LLAMA_OPENSSL=OFF`; build target `llama-server` with two jobs. TOOLS=OFF does not
create the server target. Use the hash-verified Qwen artifact above, then start:

```bash
llama-server -m /path/to/qwen2.5-coder-0.5b-instruct-q4_k_m.gguf --alias glm-5.3-flash --host 127.0.0.1 --port 18137 --api-prefix /api/coding/paas/v4 --ctx-size 16384 --parallel 1 --threads 2 --threads-batch 2 --n-gpu-layers 0 --batch-size 512 --ubatch-size 512 --predict 512 --temp 0 --jinja --no-webui --no-context-shift
```

Require `/api/coding/paas/v4/health` to return HTTP 200. Set `FOUNDRY_OMP_BUN` and
`FOUNDRY_OMP_CLI` to the installed pinned executables, `FOUNDRY_HOST_RUSTC` to Rust
1.90's rustc, `FOUNDRY_HOST_TASK_PORT=18137`, and `FOUNDRY_HOST_TASK_DIR` to a new
absolute directory outside Git. Run `cargo test --locked --offline --test gateway
host_task::free_form_parser_repair -- --ignored --nocapture --test-threads=1`. The
harness declares this exact backend identity; pointing it at another model invalidates
that identity and comparison. Stop this task-owned server afterward. No hosted key is
required and no new production model endpoint is introduced.


## Expanded ecosystem continuation — 2026-10-10

On `experimental/context-ecosystem`, main's coordinated tch/LibTorch migration is
integrated. This continuation implements additional experimental behavior; it does
not finish release acceptance or reactivate frozen 013 learning. macOS validation
remains explicitly deferred by the owner.

- **Compact evidence:** opt-in `--compact-evidence` / `compact_evidence:true` keeps
  exact source bodies/handles, protected task state and relationship citations while
  replacing repeated handles in the report with delivered-item ordinals. Facet states,
  stale/limited discovery, compiler coverage and unverified constraints remain visible.
  The verbose v1 report is still the default; compact accounting is v2.
- **Boundary coverage:** `python-http-v3` adds built-in typed/terminal-path converters
  and direct same-file static `APIRouter` registration after route declarations.
  Declaration and registration dependencies must fit before a relationship is emitted.
  Unknown/dynamic/nested/cross-file/multiple mounts remain incomplete. Recipe identity
  forces republication of old scopes. Overlap stays ambiguous, with no runtime-priority
  inference. No workspace Python runs during indexing.
- **Grounded review:** explicit revision-bound `review` recalls source-linked
  constraints/failures for changed paths and selected obligations/questions before
  finishing. It returns complete original text with attribution, visible omission/stale
  counts and no claim of semantic conflict detection or host consumption. Existing
  task state, selection and rejection dismissal are reused without a schema change.
- **CPU admission:** Linux sampling now combines affinity utilization with every
  visible finite cgroup-v2 ancestor's quota-normalized usage. It includes competing
  siblings, resets on changed sample identity, preserves hysteresis and idle residency,
  and refuses unavailable observations. This does not enforce a CPU quota or observe
  ancestors hidden outside the mount; macOS/GPU/thermal/battery controls remain open.
- **Bootstrap overlap:** the full suite exposed that fixed four-file commit gathering
  could withhold an available prefix while waiting on the next read. Ingestion now
  groups up to four ready neighbors and commits the ready prefix immediately when the
  next receiver is pending. Order, source revalidation and the eight-read bound remain.
  The existing overlap barrier test is retained unchanged; this repairs the behavior
  rather than relaxing its assertion. The earlier 80,000-file timing predates this
  opportunistic batching change and is not a fresh throughput result.
- **Generation framing:** adapter `foundry-generate-v2` returns a grammar-constrained
  JSON object as soon as it is complete, including on the last permitted token. Escapes
  and token-byte splits are handled incrementally, with one final JSON parse. Partial
  outputs remain failures and the owner still validates source/schema claims. Function
  identity changes invalidate old completion reuse; actual sampled tokens are reported.

**Focused and live checks.** The 23 ingestion fault/overlap tests pass without changing
their assertions, including exact early-publication counts after variable-size groups. A direct four-source
transaction test verifies complete rollback on failure/cancellation; successful scan
group counts now allow one to four commits according to readiness.
The boundary, task-context and evidence suites pass (15, 21 and 12 tests respectively),
including CLI/MCP opt-in delivery, source changes, budgets and restart. The framing
and seven resource-policy unit checks pass. A separately invoked live CPU observation
reports five affinity CPUs and one visible quota of 400,000/100,000 microseconds
(four CPUs), with a 1% sampled busy interval. This verifies observation, not sustained
pressure control. Nine independently executed ASGI requests pass against FastAPI
0.141.1 / Starlette 1.6.0, including UUID separator variants and router prefixes.
The runtime check is opt-in and installs/downloads nothing.

**Real CPU repair-selection calibration.** Two fixed synthetic tasks, three input
modes, six cold calls, same Qwen2.5-Coder-0.5B-Instruct Q4_K_M GGUF, greedy sampler,
256-output-token ceiling, 90-second call deadline, two threads and 2 GiB address-space
limit. Recipe v2 adds a short JSON example. Both recipe and adapter changed, so this
is not an isolated causal test of output termination. Compiler/model builds were not
run during the six calls. The earlier output-limit failures below remain evidence.

| Input mode | FIFO correct | Contract correct | Actual prompt tokens (two calls) | Actual completion tokens | Generation time (two calls) |
| --- | --- | --- | ---: | ---: | ---: |
| All fixture files | No | Yes | 666 | 291 | 55.70 s |
| Verbose Foundry context | No | Yes | 3,271 | 241 | 84.22 s |
| Compact Foundry context | No | Yes | 2,412 | 238 | 76.14 s |

All six calls completed JSON with known token usage. **All chose `repair_a`**, which is
wrong for FIFO and correct for the contract. The example itself contains that label;
this establishes a usable output shape, not a model that reliably reasons about repairs.
The independent Python oracles reject the original and every wrong candidate patch.
Compact input cuts actual prompt tokens by **26.3% versus verbose context**, with the
same results on these two cases. It still costs **3.62×** the full-file prompt tokens
on these tiny fixtures. There is no demonstrated quality improvement or large-repository
economic gain. Total tokens including failed-task attempts are 957 / 3,512 / 2,650 for
the three modes; no calls were dropped or retried. There were no auxiliary generation
calls. The reuse horizon is one task; this experiment does not measure amortized semantic
bootstrap, monetary cost, free-form patches or host-agent work.

**Neutral-example follow-up (recipe v3).** One further predeclared six-call run replaces
the example's repair label with `SELECTED_REPAIR`; model, worker, fixtures and limits
are unchanged. All calls complete within 256 tokens with known usage. Every FIFO reply
lists multiple repair identifiers and is rejected as ambiguous. Every contract reply
selects the correct `repair_a`. No failed arm is retried within either run.

| Input mode | Verified repairs | Actual prompt tokens | Actual completion tokens | Generation time |
| --- | ---: | ---: | ---: | ---: |
| All fixture files | 1/2 | 698 | 265 | 59.14 s |
| Verbose Foundry context | 1/2 | 3,372 | 277 | 93.31 s |
| Compact Foundry context | 1/2 | 2,446 | 273 | 81.07 s |

Compact context uses **27.5% fewer prompt tokens** than verbose context, but still
**3.50×** the full-file prompt cost on these tiny cases. There is **no task-quality
improvement**. Including the rejected FIFO attempts gives 963 / 3,649 / 2,719 total
tokens; all twelve calls across v2 and v3 cost 14,450 observed tokens. This is calibration,
not held-out acceptance or a representative cost per completion. The useful task-model
and recipe decision remains open. Detailed v3 observations are local in
`/tmp/foundry-task-quality-v3.json`; the previous v2 run is retained separately.

The harness now defaults to v3; `FOUNDRY_TASK_COMPARISON_RECIPE=v2` reproduces the seeded
recipe. Future reports also retain the exact serialized-request value beside its hash,
so ephemeral workspace identities do not prevent input reconstruction. These two
recorded runs predate that reporting-only addition and retain hashes rather than full
requests; their exact transient workspace inputs were not retained. No extra model
run was spent merely to replace that historical limitation.

**Real generation and reuse.** The revised native adapter also passes the independent
source-bound knowledge smoke: initial generation reports 377 prompt / 123 completion
tokens, changed input 381 / 127, then restart/revert reuses the original output with
zero calls, launches or verified artifact bytes while the model file is absent. Peak
resident observations are 726,237,184 and 725,336,064 bytes under the 2 GiB ceiling.
These two lifecycle calls add 1,008 observed tokens separately from the repair comparison.
This verifies acceptance/cache lifecycle, not the semantic quality of every brief.

**Final checks.** Rust 1.90 default suite: **1,043 passed, 0 failed, 7 opt-in tests
ignored**. Default all-target Clippy and optional `generate-worker` Clippy pass with
warnings denied. The opt-in CPU, FastAPI, repair-selection and real generation/reuse
checks above ran separately; a passing comparison harness does not make its model
choices correct. Markdown link checks and whitespace checks pass. No macOS/Metal,
real embedding weights or free-form hosted agent acceptance is claimed.

GGUF SHA-256: `1d9614638d18024d0fbb36575a15f1302a3adf044df10345688ec4f6e1c4ff32`.
Worker SHA-256: `4be2a7867b3984e0250c8f1904b220f25c2e22de984a7274891773369b2a6197`.
llama.cpp: `b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea`.
The executable fixture is [`tests/task_quality.rs`](../tests/task_quality.rs);
local detailed output is `/tmp/foundry-task-quality-v2.json` with each input/request
hash, actual usage, worker observation, response and behavioral result. Weights and
host-local outputs are not committed. The [remaining inventory](context-ecosystem-plan.md#pending-work-inventory--2026-10-10)
retains model selection, broader evidence/host tasks, retrieval attribution, scale and
hardware acceptance rather than marking the ecosystem complete.


## Live views, parameterized evidence and CPU task comparisons — 2026-10-10

Experimental continuation on `experimental/context-ecosystem`. No release, default
promotion, free-form agent-quality claim or GPU acceptance. The revised
[pending inventory](context-ecosystem-plan.md#pending-work-inventory--2026-10-10)
retains all unfinished work.

**Live host views.** The gateway now uses the existing authenticated loopback MCP
owner's atomic `render` operation before each model request, anchored to an explicit
exported task/entry/original digest. No extra store owner, file projection or host
extension is introduced. Changed folds and new protected entries reach the next
request. Deselection, expansion, forgetting, revision rollback, missing owner and
resolution deadline refuse before upstream; no stale snapshot fallback occurs. A
restarted owner reconnects. Receipts record the revision and exact serialized body,
with `synchronization:"owner_snapshot"`. Source freshness is indexed state at the read;
concurrent edits after that read can race the send. See the
[contract](../specs/003-agent-retrieval-context/contracts/adapter-economics.md#live-owner-synchronization--2026-10-10).

Installed OMP **18.6.0**, Bun **1.3.14**, resumed a real read-tool session through the
Rust gateway in `real_omp_applies_live_view_on_resumed_tool_history`. An owner edit after export changed revision 2 to 3; the resumed outgoing
request used revision 3, its receipt digest matched the body, and the original saved
history remained intact. **12,757 incoming bytes → 3,803 forwarded bytes**, 17.01 s.
The upstream was scripted: this is host/request/history acceptance, not provider-token
or task-quality evidence. Frozen export/replay remains covered separately.

**Broader evidence.** `python-http-v2` joins whole-segment FastAPI string parameters
with literal `requests`/`httpx` consumers. Both traversal directions, overlapping
literal/template providers, empty segments, unsupported converters/mixed segments,
source freshness and restart are checked. Unique matches are `route_template`, not
runtime call proof. Existing scope, posting and delivery caps remain. A composed
parameterized-contract fixture delivers provider and consumer evidence inside 4,000
admission tokens; old recipe scopes require republication.

**Paired task experiment.** `tests/task_quality.rs` fixes two tasks and three candidate
patches per task before generation: FIFO removal preserving empty-queue behavior, and
a parameterized catalog response repair preserving the published route and billing.
Independent Python behavioral oracles reject the original and every wrong patch.
The real model selects a patch identifier through the existing structured decoder;
this is constrained repair selection, not autonomous patch writing or an OMP agent
loop. Full-file input reads every file in each tiny fixture (two/five files); Foundry
uses task evidence composition with a 4,000-token delivery allowance. Both receive the
same task, options, model, greedy decoding and 256-token output ceiling. Arm order is
counterbalanced. Each call gets a cold isolated worker without retries or output reuse.

Explicit model: Qwen2.5-Coder-0.5B-Instruct Q4_K_M, GGUF SHA-256
`1d9614638d18024d0fbb36575a15f1302a3adf044df10345688ec4f6e1c4ff32`.
Accounted-run worker SHA-256
`521f80cc240dbcd10083501a81c589efa72eb2dd6ba780f35ced0ccd1d16a804`, llama.cpp
`b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea`. CPU, 2 threads, ChatML, 8,192 context,
batch 128, 2 GiB hard address-space ceiling, 180 lifetime CPU seconds, 90 s load/call
bounds; observed `linux-seccomp-fd-v1`. No embedding model or remote provider was used.

| Task / input | Context bytes | Request o200k admission | Actual prompt / output tokens | Cold generation seconds | Verified repair |
| --- | ---: | ---: | ---: | ---: | --- |
| FIFO / full files | 195 | 257 | 206 / 256 | 36.003 | No; output limit |
| FIFO / Foundry | 2,979 | 1,209 | 1,273 / 256 | 58.579 | No; output limit |
| Contract / Foundry | 4,402 | 1,776 | 1,905 / 256 | 68.964 | No; output limit |
| Contract / full files | 576 | 393 | 320 / 256 | 42.311 | No; output limit |

**Neither arm completed either task.** Foundry's full evidence envelope increased
prompt tokens 6.18× and 5.95× on these tiny fixtures. This does not predict behavior
at 60,000–80,000 files, but it rejects a blanket token-savings claim. The selected
model/prompt/output recipe has no earned task acceptance. Next work is a useful task
recipe and a compact small-task context path, followed by held-out free-form tasks.
Four calls consumed **3,704 observed prompt + 1,024 output tokens**. Cost/tokens per
verified completion are undefined with zero completions; money, energy and amortized
large-repository bootstrap/maintenance costs were not measured. No auxiliary inference
ran. Foundry preparation took 791/125 ms (cold tiny store, source acceptance, boundary
publication when applicable, lexical refresh, task setup and packing); baseline file
reads/assembly were below 1 ms. These are fixture observations, not isolated throughput
measurements. Cold generation includes worker artifact verification/load; common test
setup and construction of artifact pins are outside those times. Peak worker memory was
776,400,896–778,260,480 bytes. Total comparison execution was 232.88 s.

The **first four-call run is also retained**, not silently dropped: all four attempts
hit the same output ceiling with **unknown usage** (34.537, 47.049, 63.529 and 39.565 s;
212.19 s total test). That exposed a local accounting gap. The decoder now returns
actual counters after exhausting a completed output allowance; bounded failure usage
is included in run/pass totals, without accepting or caching incomplete prose. Invalid
counters refuse, and interrupted/killed/incomplete computation stays unknown. The
accounted run repeated the fixed source/tasks/recipe after this repair; generated
workspace/source handles differ across temporary stores, so Foundry requests are not
byte-identical across runs. Each run records actual context/request/worker/model digests.
Across these two experiments there were **eight failed calls**, four with unknown usage;
the second run's complete counters do not retroactively fill the first run's gaps.
Raw synthetic reports/logs remain outside Git at `/tmp/foundry-task-quality.json`,
`/tmp/foundry-task-quality-before-accounting.json` and the corresponding task-quality
logs. Reproduce with explicit `FOUNDRY_GENERATION_SMOKE_WORKER`,
`FOUNDRY_GENERATION_SMOKE_GGUF`, optional `FOUNDRY_TASK_COMPARISON_REPORT`, and:

```sh
cargo test --locked --test task_quality -- --ignored \
  --exact real_cpu_paired_repair_selection_reports_all_attempts --nocapture
```

**Embedding/hardware scope.** This host is Linux x86_64, kernel 6.18.44, five CPU
affinity slots, a four-CPU quota (`400000 100000`), 32 GiB memory and no `/dev/nvidia*`
or `/dev/dri/render*` devices. No selected embedding weights or macOS execution target
are configured. The existing macOS parity harness now checks finite/unit-norm vectors,
explicit eight-sequence admission, printed geometry and full-window query repeatability.
Those additions are **not target-typechecked or executed** on this Linux host. Portable
geometry/body/parent fixtures and Linux Metal-refusal checks are separate from real
embedding numerical, GPU/Metal resource, combined-model and signed-package acceptance.
The [hardware handoff](measurement-handoff.md) records the concrete command and scope.

Verification: **578 affected automated checks and 21 subprocess recovery scenarios
passed**, including real MCP transports, gateway, task state/evidence, boundary
publication, knowledge accounting and portable neural geometry/maintenance. An actual
`--no-default-features --features test-faults --bin foundry` build passed **99 consumer
harness checks** and the real OMP live-view test again (10.66 s; same request sizes).
Harness libraries/fake auxiliaries retain default features; this verifies the actual
lexical-only application binary, not a claim that `cargo test --no-default-features`
disables the self dev-dependency's semantic feature. Default all-target and actual
no-default-feature library/binary Clippy pass with warnings denied. The native
`generate-worker` binary also passes Clippy; the normal default-feature application
build is restored, and formatting/local documentation links are checked. The model comparisons
above are executed experiments with failed task outcomes, not passing quality acceptance.

## Evidence coverage, host application and real CPU maintenance — 2026-10-10

Experimental branch continuation; no release/default promotion, real-provider cost
claim or production model selection. The revised
[pending inventory](context-ecosystem-plan.md#pending-work-inventory--2026-10-10)
separates implemented behavior from remaining acceptance.

**Evidence.** Task composition reuses selected compiler reference publications in the
final source/task read, retaining original definition-unit seeds before body coalescing.
Non-definition/changed seeds are visible skips; unavailable/stale/ambiguous/capped
windows cannot imply absence or unique calls. Compiler lines require a unique target
and both delivered endpoint bodies. A constructed SCIP fixture checks publication,
source invalidation and delivery budgets. Real SDK MCP composition across two admitted
roots preserves separate identities for identical paths/bytes, includes one primary task
view, excludes foreign task state and refuses reference-only task selection. These are
source/reference candidates; tests are not executed automatically and constraint
conflicts are not inferred.

**Host boundary.** Installed npm `@oh-my-pi/pi-coding-agent@18.6.0` with Bun 1.3.14
(`@oven/bun-linux-x64`) ran through the actual Rust gateway in
`tests/gateway.rs::real_omp_applies_exported_view_on_resumed_tool_history`. Packages,
configuration and sessions were isolated outside Git; no global host configuration was
changed. The actual dist runtime, not the older inspected source tree, supplied the
consumer evidence. Three scripted loopback completions exercised a real read tool,
saved history and a fresh process resuming that history. The bound result's original
text remained in the host session; the outbound result contained the complete exported
view. Final request SHA-256 matched `observation.applied_view`. The resumed request was
**12,757 incoming bytes → 3,787 forwarded bytes**. This single fixture's byte reduction
is not provider-token savings, task-quality evidence or an automatic fold-quality result.
The real-host test completed in 20.76 s.

Gateway fixtures additionally verify exact preservation of all other request values,
wrong-content/duplicate/unpaired/out-of-order refusals before upstream, original recovery,
stale export revision refusal and idempotent replay across restart. Packet tests check
final serialized byte limits, unchanged absent-carrier bytes, altered digests and
protected-entry representations. Ordinary meter requests remain byte-exact. The gateway
loads an explicitly selected frozen packet once; source freshness at send, live revision
synchronization and provider consumption remain unknown. Applied receipts describe the
body serialized for an attempted send, including failures. See the
[application contract](../specs/003-agent-retrieval-context/contracts/adapter-economics.md#experimental-exported-task-view-application--2026-10-10)
and [operator workflow](experimental-context.md#exported-task-views-in-omp-requests).

**Real model and available hardware.** The expanded ignored test
`tests/knowledge.rs::local_real_cpu_generation_accepts_a_cited_brief_and_reuses_it`
used explicitly supplied Qwen2.5-Coder-0.5B-Instruct Q4_K_M, GGUF SHA-256
`1d9614638d18024d0fbb36575a15f1302a3adf044df10345688ec4f6e1c4ff32`, and the existing
native worker SHA-256 `25c6978579780973f0563742d87d2d30cf9fe2e5e34f680a98c4cd8892319740`
linked to llama.cpp `b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea`. Parameters: CPU,
2 threads, ChatML, 4,096 context tokens, batch 128, output allowance 512, 2 GiB hard
address-space ceiling, 180 worker CPU seconds, 90 s load/call deadlines. Observed
isolation was `linux-seccomp-fd-v1`. Linux x86_64 exposes five CPUs/affinity slots,
a four-CPU cgroup quota (`400000 100000`), a 32 GiB memory limit, and no GPU device.

| Synthetic source state | Generated | Calls | Observed prompt/output tokens | Worker peak bytes |
| --- | ---: | ---: | ---: | ---: |
| `q.remove(0)` | 1 | 1 | 376 / 124 | 726,601,728 |
| Changed to `q.remove(1)` | 1 | 1 | 380 / 128 | 726,933,504 |
| Reopen store, revert source, GGUF absent | 0; cached reuse 1 | 0 | 0 / 0 | 0; no launch |

Both model calls had complete usage observations and accepted source-bound brief
publication. A current replay also made no calls/launches with its model absent.
Restart/revert verified zero artifact bytes and made no HTTP requests. Total test time
was 77.01 s. This verifies generation/maintenance/cache lifecycle, not the semantic
correctness or coding-task benefit of the generated prose. It does not compare thread
counts or long-context model quality. Real embedding/combined-model pressure,
macOS CPU/Metal, GPU allocation/thermal/battery and signed-package acceptance remain
unexecuted on this hardware. The existing resource/worker lifecycle fixtures remain
separate from this real-model run.

Verification: **537 affected automated tests** passed across library, compiler evidence,
root dispatch, task state/retrieval, gateway, knowledge, MCP and economics suites, plus
**21 subprocess crash/recovery cases**. The two real-consumer/model tests above were
run explicitly despite their default ignored status. Default all-target Clippy and
actual no-default-feature library/binary Clippy passed with warnings denied. A real
`--no-default-features --features test-faults --bin foundry` build then passed **73**
task-evidence/task-state/gateway harness checks, and the real OMP test passed again
(15.98 s; same request sizes). Harness libraries and auxiliary fault binaries retain
default features; this is consumer verification of the actual lexical-only `foundry`,
not a claim that Cargo's self-dev-dependency disables semantic in every test library.
Formatting, diff whitespace and Markdown links passed (47 files, 770 links, 202 anchors).
The default-feature binary was restored after the lexical-only checks.

The setup draft now records isolated pinned OMP/Bun installation and test commands;
its save was confirmed. Publication and fresh-environment restoration remain pending.

## Cold verification and changed-input publication — 2026-10-10

Experimental schema 12 adds a redb publication lookup and bounded transactional
invalidation frontier. Tests exercise ordinary and lexical-only writers, explicit derived
repair, lazy model acquisition, source batching, background CPU admission and maintained
boundary relationships. Real-model lifecycle measurements remain explicitly ignored.
This uses synthetic vectors and real storage/parser/ANN/owner boundaries; it establishes work selection and recovery, not device throughput or task quality.

A 260-file addition fixture makes an offline location edit, adds one prepared source and
one zero-card source, and deletes another source. Restart publication reads exactly three
extant dirty partition rows in one shared metadata snapshot, with **zero full mapping pages**;
its locations match an independent authoritative audit. Cache-only completion after a
partial publication refreshes both paths sharing that key without a census. Exact counters,
ordinary replay, overflow, profile changes, cancellation and independent audits remain covered.
Three real subprocess aborts around lookup commit and manifest publication recover without
losing edits or re-embedding. An interrupted schema-11 upgrade remains old; explicit upgrade
preserves sources, cache and the existing generation. File substitution and corruption still
fail verification; receipts do not survive restart.

Cache-only CLI replay succeeds with the unused GGUF absent. A missing input still
refuses before acquisition, and tokenizer substitution cannot promote a partial receipt.
Three older assertions expected all model verification before any partitioning; they
were revised for lazy acquisition while preserving zero-call, cache and deadline checks.
Independent profile files hash using at most four joined scoped workers; a single GGUF
still needs a full sequential SHA-256. ANN loading hashes while filling the exact restore
buffer, with cancellation checks per MiB. Initial/overflow/repair lookup construction,
cold metadata decoding, required byte verification and compaction remain corpus-sized.
No semantic throughput or task-quality improvement is inferred from these fixtures.

Cold ingestion now commits up to four prepared sources atomically while retaining the
eight-read/16 MiB admission limit. Each changed source keeps its own source revision and
derivation obligation; reporting follows the group commit. Failure, cancellation and real
process deaths prove whole-group rollback/replay. Unchanged replay makes no source commit.
Before a reusable mapping checkpoint exists, source writes no longer populate an unusable
publication frontier. A source edit during mapping construction cannot drop a previously
processed row from the next durable checkpoint; an explicit race test covers that retry.

A bounded scale fixture uses **80,000 tiny synthetic Rust files**, 200 directories of
400 files: `d{n//400:03}/f{n:05}.rs`, containing `pub fn scale_{n:05}() -> usize { n }`.
On this Linux x86_64 sandbox (five available CPUs, Rust 1.90, unoptimized dev build,
debug information disabled), indexing exceeded the 360-second stop before the final
ingestion changes. A fresh store after those changes completed in **164.893 seconds**,
with 80,000 changed sources, no failures, a complete scan and zero pending source work.
The same corpus was reused; neither run is a cold OS-cache or optimized-release claim.
The earlier observation is censored, so no precise speedup ratio is inferred.

A real stdio MCP watcher then reconciled its 80,000-file startup baseline in **32.907
seconds**. Replacing `d199/f79999.rs` with `updated_scale_sentinel` became searchable at
revision 80,001 in **0.176 seconds**, with **one source read and zero full reconciliations**
after that baseline. Timing includes client polling. The complete fixture took 200.498
seconds; maximum child RSS was 170,076 KiB (about 166 MiB). The tested default binary SHA-256
was `1996c0609a00c72ea95d083829ea987d6aa845cb0c24e0dc9ec485f35a30ec94`.
These are single-run lexical/watcher observations; mixed-size repositories, semantic
80,000-file preparation, real models, task quality and other platforms remain unverified.

The existing watcher can maintain one explicitly enabled boundary configuration, one file
per turn, including replay, source/config changes and pause. A real MCP consumer checks the
full enable/edit/pause cycle. Optional Linux host aggregate CPU admission samples with
pause/resume hysteresis and defers on unknown observations without evicting an idle generator.
This is not a cgroup/affinity CPU quota, GPU, thermal, battery or macOS CPU implementation.

Across the affected suites, **648 tests pass**, with two explicitly ignored real-model
measurements, plus all **21 scenarios** in the separate recovery runner. This covers
library, core, neural storage/preparation/retrieval, boundary, brief, CLI, knowledge,
MCP, memory, task-context and task-evidence tests. A real build without default features,
followed by direct integration-harness execution, passes 22 boundary/task/publication
checks, including MCP and a lexical-only CLI edit followed by semantic publication.
Harness libraries retain their semantic test feature; the disabled-feature claim is for
the spawned binary. All-target Clippy, Clippy for the binary without default features,
formatting, Markdown links and diff checks pass.

The continuous-semantic fixture now includes stable pending work alongside two constantly
changing files; otherwise every candidate could correctly fail freshness checks. The HTTP
timeout fixture now starts the tested deadline at provider admission, avoiding an assumption
that preprocessing always finishes within 80 ms. Both pass with their original safety
requirements intact.

## Boundary relationships and task evidence — 2026-10-10

Experimental [005 T005](../specs/005-graph-evidence-lifecycle/spec.md#t005--connect-repository-behavior-across-process-boundaries)
and [003 T009](../specs/003-agent-retrieval-context/spec.md#t009--compose-evidence-for-a-repository-task)
are implemented on `experimental/context-ecosystem`. The environment is Linux x86_64,
Rust 1.90.0, offline cached dependencies; no GPU, downloaded model or external provider
is involved. Source fixtures are synthetic; redb/Tantivy, parsers, CLI and SDK MCP
consumers are real. The database is schema 11, upgraded explicitly with preserved data.

The boundary fixture exercises exact source spans, replay with zero parses/writes,
one-file replacement, changed and deleted dependencies, configuration changes, two
providers, dynamic calls, logging strings, shadowed/mutated bindings, parser scanner
limits and extraction caps. Named faults cover changed inputs after extraction,
cancellation, transaction failure, and real process death before/after commit followed
by restart. A schema-10 store refuses ordinary open, survives a failed upgrade and
upgrades explicitly to schema 11 with source identity preserved.

The task fixture covers implementation/consumer/configuration/test/constraint evidence,
range deduplication, actual delivered-facet accounting, token and serialized-result
byte caps, task/source races during discovery, ambiguous providers, missing tests,
ordinary-context compatibility, and derived-vocabulary discovery with source-first
boundary expansion. MCP checks the opt-in catalog, owner publication and actual result
budget. Brief records in these tests are declared synthetic, not generated by a model.

A two-service sandbox task delivers a catalog provider returning `items`, its HTTP
consumer requiring `products`, the caller's constraint to retain `/v1/items` and billing,
and the relevant test. A one-line provider repair changes the payload key. A deterministic
Python in-process transport executes the selected provider/consumer/test: it fails before
and passes after, while billing bytes stay identical. This establishes a reproducible
contract-repair fixture, not deployed HTTP behavior or autonomous model patch quality.
The response itself correctly continues to say its discovered tests are unexecuted.

Verification across the affected suites: **465 tests pass**, including all 20 new
boundary/composition tests, plus all **20 scenarios** in the separate recovery runner.
The broad run covers library, boundary, brief, CLI, core, MCP, memory, task-context and
task-evidence suites; the schema-9 knowledge-upgrade test is also checked separately.
The new brief-only fixture initially violated existing required-behavior and protected
entry rules; corrected fixture inputs pass without weakening either production rule.

- `cargo test --offline --test boundary --test task_evidence` passes on the final code.
- The affected library/CLI/MCP/storage/task/brief suites and recovery runner pass;
  `cargo test --offline --test knowledge schema_nine_requires_explicit_atomic_upgrade_without_losing_sources`
  passes after the schema-target update.
- `cargo clippy --offline --all-targets -- -D warnings`, formatting, Markdown links
  and `git diff --check` pass.
- A real `cargo build --offline --no-default-features --features test-faults --bin foundry`
  is followed by direct execution of the already compiled boundary/task-evidence test
  harnesses: all 20 pass, including CLI and MCP subprocesses against that binary.
  This avoids Cargo's self dev-dependency re-enabling the default semantic feature
  during `cargo test --no-default-features`. Library portions of those harnesses still
  use their ordinary test build; the disabled-feature claim is for the spawned binary.

Real-host outcomes, provider usage/economics, full-repository scale, other framework
patterns, parameterized routes, and multi-root task composition remain unverified or
outside this first slice. No default promotion, release or 013 learning unfreeze follows.

## Pipelined source ingestion — 2026-10-09

The next E1 increment in [001 T009/T011](../specs/001-source-state-recovery/spec.md)
overlaps bounded source reads/hash with ordered, freshness-checked source commits.
One Engine-owned Rayon pool serves reads and lexical builds across pages/calls;
ingestion drains one lexical page after 128 accepted candidates, then finishes the
existing final drain. No schema, source transaction, pending-acknowledgment or model
recipe changes. [Usage and limits](experimental-context.md#pipelined-source-ingestion)
name the separate payload budgets and unchanged explicit MCP engine-slot policy.

Linux/Rust 1.90.0 sandbox tests exercise these boundaries:

- Deliberate out-of-order reads overlap the single writer, retain walker-order
  commits and occupy at most eight active/completed result slots. Lexical builds
  run on the same three observed worker threads. A single worker is reused for
  261 lexical builds across three pages and another refresh call. Serial and
  eight-worker ingestion produce identical source tables and logical search
  documents for 260 Rust files, an empty source and an excluded binary file.
- Buffered in-place edits, atomic replacements, symlinks, removal and a changed
  exclusion decision preserve the accepted source and defer unseen deletions.
  Sensitive filenames are checked by descriptor without reading their contents;
  replacement before deletion preserves the accepted record until a clean retry.
  A read-worker panic is named, preserves prior data and permits the pool to recover.
- Cancellation after the first lexical commit in a 257-file scan leaves exactly
  128 committed sources searchable and retains an unseen old source. Reopen/retry
  reports 128 unchanged, 129 changed and one verified deletion, with no pending work.
- A real SIGABRT at the same early-publication boundary leaves 128 sources and
  their conditional pending acknowledgments. Restart reuses those versions,
  accepts the remaining 129 and verifies every final hash. The existing child-process
  source, parallel-index, repair and upgrade recovery scenarios also pass.
- Parse failures from early pages and the final drain are counted together once;
  a later unchanged scan does not inherit the previous scan's failures.

`cargo test --locked --no-fail-fast` passes on the final implementation:
**899 passed, zero failed, two existing measurements ignored**, plus all scenarios
in the separate recovery runner. This includes the CLI/MCP consumers, semantic
integration and the 6,000-file watcher fixture with foreground service and no idle
body rescans. The strengthened read/commit overlap barrier also passes separately.
`cargo clippy --locked --all-targets -- -D warnings`,
`cargo check --locked --all-targets`, library/CLI Clippy without default features,
`cargo fmt --all --check`, Markdown links/anchors and `git diff --check` pass.

These are synthetic source fixtures with real storage, parsers and lexical indexing.
They establish overlap, reuse and recovery, not a numerical throughput improvement,
80,000-file scalability or coding-task quality. Source transactions remain individual;
explicit re-indexing still reads the tree. Remaining E1 artifact handoff/cache accounting
and per-card fitting, E2/E3 incremental semantic maintenance/publication, E4 representations
and automatic generation remain pending. Self-review only; no release or deployment.

## Bootstrap work reuse — 2026-10-09

The first efficiency increment in [009 T006/T007](../specs/009-optional-semantic-retrieval/spec.md#t006t007-follow-up--remove-repeated-preparation-and-startup-work)
reuses bounded semantic CPU workers and exact tokenized cards across pipeline
stages. CLI model acquisition is lazy; source replacement no longer allocates
replacement storage chunks for unchanged content. Source/cache schemas, input
recipes, model geometry and vector meaning are unchanged.

Focused Linux/Rust 1.90.0 preparation evidence: all eight unit tests pass. A
40-definition source produces identical keys/token IDs across five batches with
one rendering/tokenization pass; retained-payload overflow takes the original
on-demand path and preserves every input. Source replacement invalidates the
handoff, differing stored tuples are refused, serial/parallel mappings match,
workers overlap, and the same CPU thread handles multiple waves and recovers
after an injected tokenizer panic. Empty/cancelled work starts no pool. Existing
partition cancellation, source-step limits and no-publication-on-error tests pass.

The full `cargo test --locked --no-fail-fast` run on the final code passes:
**891 passed, zero failed, two existing measurements ignored**, plus all scenarios
in the separate recovery runner. The 61 semantic-preparation and 56 semantic/MCP
integration tests include these new boundary checks:

- Cache-only restart and deletion-only mapping publication never call the acquisition
  seam; accepted vectors remain searchable and the previous provider observation is
  preserved. Empty/uncarded work also starts no model.
- A cold 25-source run acquires exactly once across multiple waves. A run inside
  its publication reserve never attempts startup. An acquisition that outlasts its
  deadline retains verified partitions, commits no vectors and reports the named
  budget stop. Existing provider refusal/malformed-output tests still pass.
- Existing pause/shutdown, stale-source rejection, prefetch, partial publication,
  cache corruption and child-process recovery cases pass. The watcher suite's
  6,000-file fixture retains foreground service, targeted edit updates and zero
  idle body rescans.

`cargo clippy --locked --all-targets -- -D warnings`,
`cargo check --locked --all-targets`, Clippy for the library/CLI without default
features, `cargo fmt --all --check`, Markdown links/anchors and `git diff --check`
pass. Rust 1.90.0 throughout; self-review only.

These fixtures use synthetic embeddings with real Rust tokenization and storage;
they demonstrate removed work and preserved inputs, not device throughput or
coding-task quality. Subsequent E1 ingestion/lexical work is recorded above;
artifact handoff/cache accounting and per-card fitting, T008 incremental
maintenance/publication, T009 representations and 010 generation remain pending.
No release, deployment or model-provider execution is part of this slice.

## Efficiency and incremental-maintenance audit — 2026-10-09

Inspected `62c6c97` on `experimental/context-ecosystem`: ingestion/storage, lexical
and semantic CPU work, profiles, worker acquisition/geometry, cache commits,
preparation scheduling, USearch publication, status, watching and brief lifecycle.
The [audit and model-window sources](context-ecosystem-plan.md#efficiency-audit-after-progressive-bootstrap--2026-10-09)
separate implemented reuse from pending work. Publisher metadata was read for the
pinned EmbeddingGemma 2 and Nemotron revisions and official Mistral/Codestral/Gemini
contracts. No model weights, repository uploads or provider inference were involved.

Findings: exact-input embedding reuse already works; discovery still walks source
metadata, stopped preparation does not wake on watcher edits, publication rebuilds
the complete ANN generation, status performs broad validation/censuses, and briefs
are imported rather than automatically regenerated. Cold ingestion and preparation
retain serial/repeated work. Current 128-token cards omit multiline implementation
bodies; the worker's 2,048-token total geometry is narrower than the selected model's
advertised 8,192-token input window. These are code/contract findings, not a profile
attributing elapsed time or a demonstration of long-context quality.

The revised plan selects existing 001/009 CPU/startup follow-ups, new 009 T008/T009
tasks within the existing spec, and dependency-driven 010 generation afterward.
Runtime behavior is unchanged by this documentation revision. Documentation
consistency, Markdown links/anchors and whitespace are checked; no additional Rust
tests or performance runs are warranted for this change. Prior executed evidence
below remains scoped to the implementation that produced it.

## Progressive semantic bootstrap — 2026-10-09

[009 T007](../specs/009-optional-semantic-retrieval/spec.md#t007--make-semantic-bootstrap-useful-before-a-complete-corpus-pass)
now composes bounded partition/select/embed waves in CLI and the existing MCP owner.
Explicit priority scopes alternate with the general cursor; one in-flight batch and
one bounded prefetch preserve model ownership. Publication visits prepared rows and
rebuilds the existing USearch generation at geometric growth boundaries and final
coverage changes. Cache-only pauses also publish changed locations. No schema,
model geometry, segment format or additional process changes. [Usage and limits](experimental-context.md#progressive-semantic-bootstrapping)
describe priorities, stage accounting, coverage and payload bounds.

Executed Linux/Rust 1.90.0 sandbox evidence:

- A real MCP SDK/client/server fixture with 86 indexed sources serves the explicitly
  requested semantic document while at least 77 sources remain unpartitioned. The
  general walk also progresses. A zero-card tail cannot make prefetch traverse the
  whole corpus before the first publication. A new owner resumes without repeating
  the accepted input; a completed preparation adds zero document calls on replay.
- An overlapping priority/background fixture counts one document input per eligible
  unit, including overlap with the first in-flight call. No concurrent provider calls
  are added. Invalid priorities and priorities on non-prepare operations are refused.
- CLI cancellation after two nonempty publications leaves 16 searchable inputs from
  priority and ordinary waves, with 48 of 64 sources unpartitioned. Resume embeds
  exactly those 48 and uses at most three further generation builds.
- An oversized generation request returns `resource_limit` before replacing the
  previously validated generation. A pause after source deletion republishes the
  remaining 11 locations without additional document work.
- Focused semantic checks: 48 unit tests and 114 integration tests pass, with one
  existing real-model measurement ignored. Existing malformed-provider, edit,
  cancellation, bounded prefetch, foreground admission and recovery assertions pass.

`cargo clippy --locked --all-targets -- -D warnings`,
`cargo check --locked --all-targets`, the library/CLI Clippy build without default
features, `cargo fmt --all --check`, Markdown links/anchors and `git diff --check`
pass. CLI help exposes the repeatable `--priority PATH` option.

The full `cargo test --locked --no-fail-fast` run completed with 884 passed, one
catalog-budget failure and two existing model/hardware measurements ignored; the
separate recovery runner also passed. Adding priority metadata took `tools/list` to
1,021 o200k tokens. The only subsequent code change shortened three tool descriptions,
preserving their guidance and schema constraints. The exact catalog contract now
passes at **997 tokens** under its unchanged 1,000-token ceiling. The final complete
MCP suite passes **90/90**, resolving the only failure: **885 unique tests passed**
across the full run and affected-suite rerun, with two measurements ignored. The
full run's other checks were not repeated for this wording-only correction. All
targets and the build without default features pass Clippy/check again afterward.
These tests use synthetic embedding functions with real Rust tokenization, redb,
USearch and MCP delivery; they establish scheduling/freshness/reuse behavior, not
real-model speed or task-quality gains. Stage times overlap and are wall accounting,
not device profiling. Full USearch generation builds and status coverage censuses
still scale with prepared/source metadata respectively. Device tuning, CPU pool
reuse, host-pressure admission and 80,000-file performance remain unverified.
Self-review only; no release, model download, external provider or deployment ran.
At this checkpoint the next selected increment was generation/area knowledge
(010 T002). The later efficiency audit above puts CPU/startup reuse, changed-work
maintenance, vector deltas and model-aware inputs before that work. Boundary joins
and task evidence composition remain selected.

## Incremental primary-root watching — 2026-10-09

[001 T011](../specs/001-source-state-recovery/spec.md#t011--maintain-the-admitted-workspace-from-filesystem-change-hints)
adds explicit `mcp --watch` on the existing stdio/shared-HTTP owner. Native `notify`
events feed a bounded, coalescing queue in [watch.rs](../src/watch.rs); the held-root
reader and streaming reconciler in [ingest.rs](../src/ingest.rs) publish through the
existing source transactions, seen marks and pending lexical index. No schema change
or additional service is required. [Usage and recovery](experimental-context.md#incremental-workspace-watching)
describe source delay, status, startup reconciliation and platform limits.

Executed Linux/Rust 1.90.0 sandbox evidence:

- Ten watcher unit tests cover 10,000 duplicate hints, path/byte overflow, read and
  store-event noise, ignore precedence, subtree deletion boundaries, incomplete and
  superseded scans, edits during reads, cancellation, symlink/root replacement,
  deliberately lost hints followed by rescan/restart, visible error backoff, and
  handle/brief invalidation before lexical drain. A busy known-file queue publishes
  searchable changes before its hints drain. A deterministic MCP owner test
  holds a watcher step while a foreground request waits and verifies source work
  yields to foreground admission.
- Four real SDK integration tests in [tests/watch.rs](../tests/watch.rs) pass. Native
  events drive atomic saves, new files, directory moves/deletes and ignore edits.
  Prior handles become stale. A killed stdio owner recovers edits, deletion and new
  files made offline. Default-off behavior, HTTP primary-only maintenance, reference
  isolation, bind-failure cleanup and clean store release pass.
- A synthetic **6,000-file** workspace starts from an empty store. Foreground status
  succeeds during partial reconciliation; after convergence the watcher reports
  exactly 6,000 source reads. A known-file edit needs at most three additional body
  reads (allowing duplicate native hints), causes no new root reconciliation, and is
  searchable. Both observed quiet intervals add zero source reads. This fixture
  verifies work selection and coexistence, not 80,000-file throughput or a speedup.
- The shared drain now checks pending work before treating a zero source/memory
  count as completion: briefs can occupy an entire page. A regression with 129
  pending briefs followed by source work drains completely and preserves public
  source/memory counters. All 18 brief tests pass.
- Final `cargo test --locked --no-fail-fast`: **880 passed, zero failed, two existing
  model/hardware measurements ignored**, plus all scenarios in the separate recovery
  runner. Source stayed fixed during this full run; subsequent warning cleanup only
  renamed a fault-hook parameter and gated the semantic-only status helper.
- `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo check --locked --all-targets`, the build without default features,
  `cargo fmt --all --check`, Markdown link/anchor checks and `git diff --check` pass.

Self-review only. No embedding/generation provider, real-host task-quality trial,
macOS/native-network-filesystem validation or release promotion ran. Native watch
registration memory is outside the queue bound. Notifications silently lost without
a rescan signal and ignore-policy files outside the root need explicit indexing or
owner restart; there is no periodic all-source polling. Progressive semantic bootstrap
(009 T007) follows in the newer record above.

## Source-bound brief import and conceptual discovery — 2026-10-09

[010 T001](../specs/010-source-bound-derived-knowledge/spec.md) is implemented on the
experimental branch in [brief.rs](../src/brief.rs), the shared store/index and the
source-first response packer. Schema 8 retains one structured derived brief per
source/recipe, with generator attribution, exact cited spans and full dependency
hashes. Import, get/search, explicit per-ID historical export and CAS forget use the
same authoritative owner. `context --include-briefs` maps generated vocabulary to
source for anchorless queries and can deliver derived orientation within the same
allowance as supporting code, a selected task view and explicit memory.

Executed Linux/Rust 1.90.0 sandbox evidence:

- **17 tests in [tests/brief.rs](../tests/brief.rs)** pass against real redb, Tantivy,
  response packing and independent CLI processes. The synthetic `capacity eviction`
  fixture has no baseline source match; a brief finds the opaque routine and exact
  source/test bytes are recovered. Exact-name context remains byte-identical in the
  fixture. A deliberately false summary is still only `kind:derived`; its primary
  source must fit before any summary is delivered.
- Source/dependency edits and deletes invalidate delivery before index cleanup;
  tests force edits during generation and final query validation and forget after
  candidate selection. Replay, replacement CAS, monotonic revisions, malformed
  citations/identifiers, UTF-8/JSON/text limits, escaped dependency paths, corruption,
  namespace separation and user-memory preservation pass. Restart, full repair and
  pending replacement during derived drain converge on current rows.
- Real child processes abort before/after put and forget commits. Reopen/replay
  proves row, revision and pending work are atomic at those boundaries. This is
  application recovery evidence, not a hardware power-loss claim.
- The previous schema-7 binary creates source plus task state in an isolated
  temporary store; the current binary upgrades it to schema 8 and preserves both.
  The previous binary then returns `unsupported_schema`. A separate test interrupts
  schema-7 upgrade before commit and verifies retry; old source/memory/task data is
  retained. Existing repository indexes were not upgraded.
- `cargo test --locked --no-fail-fast` exercised the full suite: 860 passed,
  four older migration assertions still expected schema 7, and two existing model/
  hardware measurements were ignored. Only those test expectations were then
  corrected; rerunning both affected suites (`--test neural --test semantic`)
  passed all 159 tests. **864 tests now pass, zero unresolved failures, two ignored**,
  plus every scenario in the separate recovery runner. Source was not edited during
  the full run; the rerun changed only the four schema assertions.
- `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo check --locked --no-default-features --lib --bin foundry`,
  `cargo fmt --all --check`, Markdown link/anchor checks and `git diff --check` pass.

These inputs are synthetic generator outputs. No generation provider, area rollup,
MCP brief tool or automatic background generation ran. A citation establishes which
bytes were used, not that model prose is correct. Freshness is relative to committed
source state; the watcher is the next build-order increment. No real-model quality,
80,000-file throughput or full-task token/cost improvement is claimed. Self-review
only; no release/default promotion or change to 013's freeze.

## Large-repository architecture and parallel partitioning — 2026-10-09

The owner redirected the experimental work toward verified task quality, practical
bootstrap at 60,000–80,000 files, LLM-derived repository knowledge, parallel execution
and continuous incremental maintenance. The
[revised design and build order](context-ecosystem-plan.md#architecture-correction-useful-context-across-6000080000-files)
records the comparison with Prakarana, read at commit
`3143213d63c22a097c77fb229bfc910e08c6e01e`. Its sources, contract tests and historical
reports were inspected; no predecessor code, service or test suite was executed.
Prakarana's draft cross-service graph spec is not treated as an implemented loader.

Findings from current Foundry source: source-index construction already parallelizes;
semantic partition computation held the redb write transaction and, in MCP, the
serving engine slot. Source watching is absent and 010 was deferred. The latest
recorded full Gemma preparation took 4,437 s for 332,732 cards; that historical elapsed
time does not identify the share caused by partitioning. Recent task-state and exact
delivery-budget features do not establish repository-understanding or task-cost gains.

[009 T006](../specs/009-optional-semantic-retrieval/spec.md#t006--bound-and-parallelize-semantic-partition-computation)
is implemented in [cache.rs](../src/neural/cache.rs),
[prepare.rs](../src/neural/prepare.rs) and [driver.rs](../src/neural/driver.rs):

- Snapshot and verify source bodies under short read transactions. Compute card
  tuples outside all store transactions and the MCP engine slot, with up to
  min(available parallelism, eight) CPU workers and at most eight source bodies per
  wave (16 MiB input bound, not a whole-process memory guarantee).
- Join workers and commit mappings in source order with a current source-hash check.
  Changed/deleted sources refuse publication; cancellation, pause and shutdown keep
  unpublished work out of storage. A worker panic is a named persistent error.
- CLI and MCP share the computation and acceptance path. Existing card recipes,
  vector identities, schema 7, model admission, caching and source outputs stay intact.
  This changes CPU preparation concurrency, not model/device batch parallelism.

Linux/Rust 1.90.0 sandbox evidence:

- Five partition tests and seven driver tests passed, including serial/parallel tuple
  equality, bounded admission, actual overlapping CPU calls, cancellation/panic,
  foreground retrieval and writes while computation has no engine slot, and pause/
  shutdown discarding unpublished work. Four of those are new tests.
- The neural lifecycle suite passed 57 tests, including the new source-change/delete
  during-computation test and restart preservation. Neural retrieval passed 54 tests
  with one real-model measurement ignored. The separate recovery runner passed all
  scenarios. Inputs/providers here are synthetic; no generation/embedding model ran.
- Final `cargo test --locked --no-fail-fast`: **847 passed, zero failed, two ignored**,
  plus every scenario in the separate recovery runner. Ignored hardware/model
  measurements are not counted as passed. `cargo clippy --locked --all-targets --
  -D warnings`, `cargo check --locked --no-default-features --lib --bin foundry`,
  `cargo fmt --check`, the repository Markdown link/anchor checker and
  `git diff --check` pass. No source was edited during the final regression run.

010 T001/T002, 001 T011, 009 T007, 005 T005 and 003 T009 now have selected contracts;
their LLM briefs, watcher, progressive publication, boundary joins and task composition
are **not implemented by this increment**. No real bootstrap speedup, 80,000-file
throughput, live coding-task improvement or provider-dollar saving is claimed.
013 stays frozen; the owner's instruction permits sandbox implementation without
waiting for external measurement gates. Evidence is self-review, not independent review.

## Task-aware retrieval and recovery search — 2026-10-09

The next sandbox increment on `experimental/context-ecosystem` implements
[008 T005](../specs/008-scoped-durable-memory/spec.md#t005--recover-evidence-by-content-and-compose-task-aware-retrieval)
and [003 T008](../specs/003-agent-retrieval-context/spec.md#t008--deliver-task-aware-source-context-under-one-allowance):

- Source `context` can include the complete selected task view. Task revision,
  original/summary representations and linked-source freshness validate inside the
  primary source query's final read transaction. A changed or forgotten task refuses
  the whole response. Multi-root retrieval includes primary task state once and
  retains the existing separate source snapshot for each root.
- The task line and ordinary evidence share one exact o200k token allowance and the
  existing byte cap. Selected entries cannot silently disappear or become summaries.
  Oversized views require explicit edits; conflicts and packing refusals refund MCP
  reservations, and successful replies charge the entire delivered text.
- Read-only task search finds retained originals, including folded/deselected
  observations, with byte-bounded pages, exact UTF-8 excerpts, hashes, attribution,
  revision checks and source freshness. Superseded history requires explicit opt-in.
- The existing schema-7 table, source transaction and packing ladder own the work.
  No new schema, index, background worker or automatic capture is introduced. The
  default MCP catalog remains unchanged; task-aware fields require the experimental
  task-context flag. See the [usage guide](experimental-context.md).

Verification on Linux, Rust/Cargo 1.90.0, using synthetic fixtures:

- `cargo test --locked --no-fail-fast`: **842 passed, zero failed, two ignored**,
  plus every scenario in the separate recovery runner. The ignored hardware/model
  measurements are not counted as passed. This complete run also verifies the
  schema assertions repaired in the preceding increment.
- [task_retrieval.rs](../tests/task_retrieval.rs) adds **11 integration tests**:
  real CLI/SDK MCP delivery, opt-in schema, exact shared budgets and MCP refunds,
  single/multi-root scope, source staleness and task-change/forget fault hooks,
  restart recovery, Unicode offsets, history and byte-limited search pagination.
  The byte-cap edge fixture was then strengthened to keep tokens below their cap
  while the mandatory context header makes the bytes overflow; all 11 tests pass
  again. The existing 19 task-context tests pass in the complete run.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and
  `cargo check --locked --no-default-features --lib --bin foundry` pass. Local
  Markdown targets and `git diff --check` are checked before commit.
- Evidence is self-review and local execution. Tool delivery is verified; actual
  host application, held-out retrieval improvement, coding-task outcomes and provider
  economics remain unmeasured. These remain future acceptance evidence and do not
  block the owner's authorized experimental sandbox work.

## Experimental context ecosystem — 2026-10-09

The owner explicitly selected an experimental Git branch, local commits and sandbox
building/testing without waiting for external measurements. Work is on
`experimental/context-ecosystem`; the retrieval increment is committed as `556b434`.
This instruction supersedes the later-milestone implementation stop in the earlier
retrieval record below. No release, default promotion or publication is claimed.

[008 T003/T004](../specs/008-scoped-durable-memory/spec.md) and
[003 T007](../specs/003-agent-retrieval-context/spec.md) are implemented experimentally
in [task_context.rs](../src/task_context.rs), the existing store and CLI/MCP owners:

- Typed task records retain immutable observations and attributed constraints,
  requirements, hypotheses, decisions, failed attempts, questions, work and artifacts.
  Updates compare revisions and commit complete batches atomically.
- Selection, pinning, folding, expansion, checkpoints and restoration edit a working
  view. Protected constraints remain visible; old views cannot revive superseded
  constraints. UTF-8 peeks recover exact original bytes and hashes after restart.
- Exact failed-action matching produces grounded advice or explicit silence within a
  byte budget. Explicit dismissal survives restart. There is no automatic polling,
  semantic constraint detector, auxiliary model call or claimed host consumption.
- Explicit decision promotion atomically writes ordinary project memory, its replay
  key and task lineage. Forget removes unchanged retained copies and invalidates
  stale search hits; independently edited copies cause an atomic conflict. Deleted
  promotion IDs cannot be reassigned within the same task's lineage.
- Schema 7 adds one table and one revision counter to the existing redb owner. Earlier
  schemas require explicit upgrade; current-schema corruption is never auto-repaired.
  The preserved schema-6 baseline binary was run against a disposable schema-7 store
  and refused with `unsupported_schema` (exit 2). The repository's existing store was
  not upgraded. Old-binary rollback requires a pre-upgrade closed-store backup.
- CLI `task-context` and MCP `--experimental-task-context` share engine operations.
  The default catalog is unchanged. MCP retains its existing frame/result bounds;
  large observations and exact exports use CLI. See the
  [usage guide](experimental-context.md) for request shapes and limits.

Verification on Linux, Rust/Cargo 1.90.0, with synthetic fixtures:

- The broad `cargo test --locked --no-fail-fast` run executed 830 ordinary tests:
  829 passed, one obsolete schema-6 assertion failed, two measurements were ignored.
  The separate recovery runner exposed a second obsolete schema-6 assertion. Both
  assertions were updated to the current schema without weakening their preservation
  checks. All other suites passed in that run.
- On the final implementation after the lineage-reuse repair,
  `cargo test --locked --test task_context --test memory --test recovery` passed:
  **19 task-context tests, 29 memory tests, and every recovery-runner scenario**.
  This includes real CLI process restarts, a real SDK MCP client, simultaneous MCP
  edits, stale-source refusal, protected entries, UTF-8 paging, exact export,
  capacity/refusal behavior, and real abrupt exits before/after apply, promotion and
  cascading-forget commits. One interaction-only observation is absent from the
  repository and still recovered exactly after folding and restart.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and
  `cargo check --locked --no-default-features --lib --bin foundry` pass. Local Markdown
  targets and `git diff --check` are checked before commit.
- The checks are self-review and executable sandbox evidence. No model solved a live
  coding task using this working view. Applied host replacement, provider economics,
  held-out retrieval benefit and broad intervention quality remain unmeasured. They
  remain future acceptance evidence, not blockers to experimental implementation.

## Retrieval execution — 2026-10-09 (partial milestone)

The owner authorized the ecosystem sequence (“go for 1 then 2 and onwards”).
001 T010, 009 T005 and 003 T006 are active; none is fully accepted by this record.
Implementation and self-review ran in the Linux cloud workspace on Rust/Cargo 1.90.0.

- [Request-local diagnostics](../src/retrieval_trace.rs) are available through
  `context --retrieval-trace-dir DIR` and the same MCP startup flag. The directory
  must be new. Records have explicit truncation and run limits, full source identity,
  selection losses, packing inputs, and the actual Foundry result. Host consumption
  remains unknown. Source/memory payloads and raw traces stay outside Git.
- [Behavioral packing](../specs/001-source-state-recovery/contracts/context-v2.md#experimental-behavioral-support--001-t010-2026-10-09)
  is implemented behind `--experimental-behavior-context`, **default off**. It uses
  existing candidates and changes neither embeddings nor candidate recall. Locator,
  unknown-intent, ambiguous-name and graph behavior retain the default path.
- On the preserved local Foundry index at source revision 222, the development query
  “How does `QueryRuntime` load the dense index on the first query?” exposed a packing
  loss: `QueryRuntime::dense_index` was candidate rank 2 (zero based), survived the
  four-per-file and 32-unit limits, and reached packing, but the baseline delivered
  only the struct. The candidate delivers the method body and the owner's address.
  At the same 1,024-token request cap, delivered tokens are **437 → 357**, and shown
  items **1 → 2**. Both use the same indexed source identity and no semantic profile.
  This is one local development example, not a retrieval-quality rate, provider-cost
  result or live coding-task result. A separate public Rust fixture exercises
  the same candidate-kept/packing-omitted mechanism.
- Fixtures cover stale and capped candidates, dense/lexical placement tracing,
  incomplete traces and private file permissions, unchanged traced response bytes,
  actual stdio success/refusal records, Rust/Python/TypeScript methods, budget sweeps,
  locator/unknown/graph controls and cross-root ambiguity. The actual stdio test
  confirms that the opt-in flag reaches the packing policy without a tool-schema
  change.
- Rust validation: `cargo fmt --check`, `cargo check --locked --all-targets`,
  `cargo clippy --locked --all-targets -- -D warnings`, and
  `cargo check --locked --no-default-features --lib --bin foundry` pass on 1.90.0.
  The final `cargo test --locked --no-fail-fast` run after review changes passed
  **812 tests, zero failed, two ignored**, plus the recovery runner. The ignored
  hardware/lifecycle measurements are not counted as passed. This is local contract
  verification, not the unexecuted retrieval-quality or live-host gates below.
- [003 T006](../specs/003-agent-retrieval-context/spec.md#t006--validate-retrieval-repairs-in-real-coding-tasks)
  records the host capability inspection and matched-run procedure. Codex CLI
  `0.159.0-alpha.3` parses the command-local MCP configuration; no model task or
  provider-usage measurement ran. Pinned OMP source exposes context/payload hooks,
  but that corpus pin is package 18.5.0, distinct from the previously measured 18.6.0
  host. Upstream source/tests were inspected, not executed or treated as host proof.

**Remaining gates:** the private G1/G2 questions/responses, prepared Gemma apparatus
and Metal-capable measurement host are absent here. The 249 historical G2 misses
remain unattributed; no anchorless repair has been selected and no fresh holdout
comparison has run. OMP/Bun are not installed here, and matched live host tasks,
complete provider economics and applied-view acknowledgment are unexecuted. The
candidate stays opt-in. Milestones 3–6 retain their dependency gates and are not
implemented by this change. Existing signed-package requirements and the 013 freeze
remain in force.

## Experimental LibTorch dependency migration — 2026-10-10

The [PR #9 / PR #2 compatibility review](review/tch-026.md) records Rust 1.90
checks against actual LibTorch 2.13 CPU on Linux: 145 feature-enabled library
tests, 142 default library tests, both all-target Clippy configurations, and
compilation of the feasibility decision probe passed. Three numerical-operation
tests execute LibTorch; the Linux worker binary itself is an unsupported-platform
stub. macOS arm64 loading, package/bundle behavior and full checkpoint parity
remain unexecuted for the new pairing. Learning remains frozen off; historical
0.24.0/2.11.0 results below are not relabeled as new-runtime acceptance.
The owner authorized merging the coordinated changes on 2026-10-10 with macOS
validation deferred. That deferral does not mark the pending checks as passed.

## v0.1.0 local release — 2026-10-04 (withdrawn)

**Withdrawn the same day.** The owner decided that a version is released only when
every active spec is complete (constitution 0.5.0), so the local tag `v0.1.0` and its
artifacts were deleted; nothing had been published or pushed. This record remains as
history of the checklist run against the scope implemented at the time.

Owner decision 2026-10-04, since withdrawn: a local tag `v0.1.0` and a macOS arm64
artifact; nothing published or pushed. Checklist (`docs/release.md`):

1. **Scope and evidence:** 001 T001–T006 (with the T005 leading-run amendment), 003
   T001–T003 and T005, 007 T001; acceptance evidence and limitations are the sections
   below. Advertised integrations were exercised with their real consumers: OMP 18.6.0
   and Codex 0.159.2 ran against this release binary in the 2026-10-04 runbook.
2. **Checks:** the integrated gates (fmt, clippy `-D warnings`, 258 passing Rust tests
   plus 16 custom-harness recovery scenarios with one payload report ignored, Rust 1.90
   check and clippy) ran on build inputs identical to the tag's `src/`, `tests/`,
   `Cargo.toml` and `Cargo.lock`. Install smoke:
   `cargo install --path . --locked` into a scratch root, then 18 README steps on fresh
   stores (missing store, index, search, context, status, graph import and traversal,
   retrieve, re-index, bootstrap inspect/apply, connect for OMP and Codex, usage help)
   all exited as documented; the installed binary equals the release binary.
3. **Compatibility and data:** stores are schema 2 (`upgrade-store --to 2` from v1);
   search schema `"3"` makes older indexes report `repair_required` until
   `repair-index`, preserving source, revisions and feedback. No automatic migration or
   downgrade; rollback is restoring a store copy and the previous binary. Removing the
   binary never deletes stores.
4. **Contents and licenses:** 154 tracked files, no credential or private data patterns
   (the four private-key-marker matches are deny-rule string literals at
   `src/ingest.rs:489–490` and `tests/fixtures/.economics/src/ingest.rs:489–490`). The
   artifact carries `LICENSE` (MIT)
   and `THIRD-PARTY/` with the license/notice files of the 287 runtime dependency
   packages; 20 crate packages ship no license file, so a supplied notice records each
   declared license with canonical texts. The binary links only system libraries and
   contains no fault-hook strings.
5. **Artifact:** `context-foundry-v0.1.0-macos-arm64.tar.gz` (binary, README, LICENSE,
   release notes, THIRD-PARTY) and `context-foundry-v0.1.0-source.tar.gz` (`git archive`
   of the tag), with `SHA256SUMS`; binary SHA-256 `e32204fb…6cee7`. Local preparation
   only.

## Measurements on the development host — 2026-10-08

The owner reversed the cloud handoff below because the baseline stores are too large to copy. The remaining rows ran on this host (Mac15,6, 12 cores, 18 GiB), one job at a time, using the frozen apparatus (`FROZEN.txt` of 11:31:14Z, with addenda at 14:45Z and 18:48Z).

Every result below comes from a run with 0 retried, refused, missing or duplicate tasks. INVALID runs are named and not used.

A timing run counts only if it had no swapouts and the 1-minute load was below 6 before it started. The owner's resident services keep the idle load at 4–7, so the usual idle rule (load below 2) could not be met. The owner stopped three resident model servers for the timing rows.

**Result:** EmbeddingGemma 2 is the selected profile. Semantic retrieval stays off by default and unadvertised until the installed-artifact checks pass on the Developer ID-signed package.

- **G2 (row 1).** Foundry `8035faa` (`0ca21c8e…`); K = 2; pass requires one-sided p < 0.025.
  - **No-profile baseline:** delivered evidence on 0 of 388 questions. The result is identical on `90a82cd`. Two checks confirm the scoring:
    - A positive control of 6 questions that name the definition passed 6 of 6.
    - The 21 baseline responses that name the answer's file deliver other units from that file.
  - **EmbeddingGemma 2** (`gemma.json` `bb1c9d08…`): **ENABLE.**
    - 139 of 388 gained (rust 45/197, bun 94/191), 0 lost; p = 1.43e-42.
    - Mean delivered tokens 2,019.7 → 2,018.1.
  - **Nemotron 3 Embed 1B** (`nemotron.json` `619205ad…`): **INVALID twice.**
    - In both runs the first rust question was refused once and retried.
    - The refusal code was not captured. [inference] The likely cause is the first dense query loading the 2.74 GB rust index past the read deadline.
    - For information only: 147 gained, 0 lost; mean tokens 2,019.7 → 2,015.2.
    - The owner closed Nemotron: it is not enabled and will not be measured further.
- **G1 with EmbeddingGemma 2 (row 2): PASS.**
  - Sets: checker (6,881 tasks), qualified (2,551) and bun (999), via `g1quick.sh … 1/1 1`.
  - The run used one shard. Six shards, each with its own worker, swapped this host and produced 44 retries (INVALID).
  - No target or guard that passes without the profile fails with it.
  - First-call passes, task by task against the no-profile run:
    - **checker:** 3,741 → 3,716. Every change is on an unmarked wording: 26 definition and 7 usage tasks lost; 3 definition and 5 usage tasks gained.
    - **qualified:** unchanged.
    - **bun:** 971 → 969.
  - The largest guard drop is checker definition/unmarked first-call, 55.0% → 51.7%. It stays above its frozen baseline of 37.4%.
- **Nemotron conversion.**
  - Converted offline from the local BF16 safetensors with llama.cpp `b9acf138`'s converter (`mistral3.attention.causal = False`); GGUF `e0cd7ff3…`.
  - Worker against the torch BF16 bidirectional reference, 12 inputs: min cosine 0.999691. A causal reference scores only 0.62–0.90.
- **Batched against single-sequence vectors (row 3), EmbeddingGemma 2: PASS.**
  - The ignored `embed_worker` test on the frozen bundle: all 18 rows have cosine ≥ 0.9999. The lowest is 0.9999878; a 128-token heterogeneous batch scores 0.9999962.
  - Max absolute difference: 5.79e-4.
  - A 2,048-token query embeds; a 2,049-token query is refused.
- **T009 index time (row 6).** Release builds, fresh stores, `/usr/bin/time -l`.

  | Corpus | Build | Valid runs | Wall time | Peak footprint |
  | --- | --- | --- | --- | --- |
  | oh-my-pi (8,436 sources) | before T009 (`fb4511c`) | 3 | 56.1–57.2 s | 328–350 MB |
  | oh-my-pi | T009 (`89a8c54`) | 1 | 54.9 s | 368 MB |
  | oh-my-pi | main (`d61f1da`) | 2 | 53.2–54.4 s | 364–375 MB |
  | rust-lang/rust (60,739 sources) | before T009 | 0 | none valid | — |
  | rust-lang/rust | T009 | 1 | 363.5 s | 703 MB |
  | rust-lang/rust | main | 2 | 361.2–365.5 s | 731–733 MB |

  - On oh-my-pi, comparing the fastest valid runs, T009 takes 0.978× and main 0.949× the before-T009 time.
  - Every rust before-T009 run swapped (4 attempts, wall 361.6–390.1 s). In total, 15 runs swapped and are not used.
  - In the valid runs, user plus system time is about 18–51% of wall time. [inference] Indexing on this host is bound by I/O under memory pressure, so T009's eight build threads give no visible wall-time gain here.
- **Door latency (row 7).** Main `d61f1da`, `context` at 2,048 tokens, 3 passes, 0 swapouts. Every response text equals its G1 response.

  | Corpus | Group | Queries | p50 | p95 |
  | --- | --- | --- | --- | --- |
  | rust-lang/rust with SCIP | tie group (`doors:each`) | 30 | 50.2 ms | 125.7 ms |
  | rust-lang/rust with SCIP | resolved anchor (`doors:exact`) | 30 | 35.4 ms | 113.7 ms |
  | rust-lang/rust with SCIP | approximate (`doors:approx`) | 11 | 28.4 ms | 85.0 ms |
  | oh-my-pi | tie group, approximate doors | 14 | 61.3 ms | 170.5 ms |
  | oh-my-pi | approximate (`doors:approx`) | 30 | 53.3 ms | 80.6 ms |

**Preparation (record; rows 1 and 5).** `semantic prepare --budget-seconds 3600 --development-isolation`, repeated until `partial` is false. Peak is the foundry process's footprint.

| Store | Profile | Rounds | Cards | Embedded | Input tokens | Elapsed | Peak |
| --- | --- | --- | --- | --- | --- | --- | --- |
| oh-my-pi | EmbeddingGemma 2 | 1 | 112,549 | 111,239 | 6,798,414 | 1,256 s | 1.97 GB |
| rust-lang/rust | EmbeddingGemma 2 | 2 | 332,732 | 331,556 | 19,303,357 | 4,437 s | 4.72 GB |
| oh-my-pi | Nemotron | 2 | 112,549 | 111,240 | 5,848,851 | 4,129 s | 3.60 GB |
| rust-lang/rust | Nemotron, `--cache-cap 4294967296` | 4 | 332,732 | 331,556 | 16,213,239 | 12,884 s | 7.63 GB |

- **Library-only rust-lang/rust store, EmbeddingGemma 2 (row 5; the 2,255 files of `library/`).**
  - 62,023 cards; 61,674 embedded; 4,681,997 input tokens; 7,710 document calls.
  - Cache 194 MB; foundry peak 1.11–1.12 GB across attempts; worker peak 1.27 GB.
  - Its time is not accepted: all three attempts swapped (65,628–953,714 swapouts). They took 694–747 s, against 2,139 s for 15,026 body units with EmbeddingGemma at 768 dimensions on 2026-10-07.
- **Nemotron's rust index.** The 2,048-dimension index needed 2.74 GB of cache, hence the cap setting below. Publishing it at about 292k rows overran one 3,600 s round.

## Measurement handoff and cache cap — 2026-10-08

- **Handoff.** The owner moved the remaining real-model and timing measurements (G2, G1 with a semantic profile, vector parity, the installed artifact, preparation cost, T009 timing, tie-group door latency, rollback re-embedding) to the [measurement handoff](measurement-handoff.md) for a cloud agent: this 18 GiB host swapped under six G1 shards with a resident worker each, and the resulting deadline refusals made that run INVALID. The frozen inputs that lived only under `/private/tmp` (the G1 baseline stores, SCIP artifact, baseline binary and token counter) were cloned into the G1 dataset directory outside Git. Semantic retrieval stays off by default and unadvertised until the handoff's rows 1–4 pass.
- **Cache cap setting.** G2 preparation of the full rust-lang/rust store found 332,732 cards, 2.74 GB at 2,048 dimensions, above 009's fixed 2 GiB cap, so preparation could never complete. The owner made the cap an operator setting: `semantic prepare --cache-cap BYTES` and `mcp --semantic-cache-cap BYTES` (default 2 GiB, not part of the profile or its function digest); the run records it and `semantic status` reports it, and a run under a larger cap resumes from the kept rows. Cross-lab review by OpenAI GPT-6.1 Sol: SHIP with one minor (the older-binary test fixture now omits the new field). Author: Anthropic Opus 5.5.

## 009 T004 address cards on a llama.cpp worker — 2026-10-08

Merged at `fd01489`; unreleased. Its real-model and timing checks (G2, batched against single-sequence vectors, the installed artifact's isolation, resource and numerical checks, preparation time and footprint) are in the [measurement handoff](measurement-handoff.md); nothing here ran a model.
- **Shape.** One card per definition and per Markdown section (address, signature or head lines, leading documentation; 128 tokens by default under the profile's template), recipe `cards-v1+units5`, so a unit change re-partitions; acceptance takes only the cards the verified body renders. Dense retrieval serves only anchor-less queries, its units first in similarity order and tagged `[semantic]`, then the lexical candidates in today's order; an anchored query never calls the model and its output is byte-identical to no profile. RRF and the semantic preview ladder are removed. Descriptor and profile v2 own the dimension, templates, pooling and card limit; cache rows are self-describing and each run reconciles the byte total from the actual rows; generation v3 records its geometry; one batch is prefetched and a failed prefetch still publishes the committed batch. `foundry-embed` links llama.cpp `b9acf138` statically (Metal), built by `build.rs` from the pinned commit's tree with fixed flags (`GGML_NATIVE=OFF`, portable across Apple silicon), refuses split GGUFs, and writes llama.cpp's license for THIRD-PARTY (`--notices`). pyo3, the MLX worker and its Python closure are gone; a descriptor v1 profile is refused with `profile_unsupported`, and an upgrade over one keeps the core and disables semantic retrieval by name.
- **Review.** Cross-lab by OpenAI GPT-6.1 Sol over four rounds: REVISE (6 major: card provenance not bound to its input, cache-cap accounting after a binary rollback, a prefetch failure hiding the committed batch, whole-file re-tokenization per batch, unhashed GGUF shards, linked archives not tied to the pinned source; 1 minor), REVISE (card partitions not invalidated by 001 T008's unit change), SHIP, SHIP on the gate fixes. Author: Anthropic Opus 5.5 (four continuation sessions).
- **Gates.** fmt, clippy (stable and Rust 1.90), the full test suite and Rust 1.90 check passed at `fd01489`; the gate found two tests whose expectations predated T004 (an anchored query's header has no `semantic:` word; the prefetch takes the engine slot for one selection step during an inference) and two lints.
- **G1 unchanged.** Without a profile, G1 on `fd01489` (binary `b9b01e93…`) matches `483810a` on every task of all three sets (0 of 10,431 differ in pass, body or tokens).
- **Rollback check (no model).** No release exists, so the 005 T004 release build (its semantic code equals `7da1125`) read a store written by T004 with the fake worker: rows counted corrupt, the v3 generation unavailable by name, source status and search intact, `semantic purge` removing every row. Re-embedding under its own v1 profile needs the MLX model and is in the measurement handoff, needed only if a pre-T004 build was ever distributed.

## G1 city-map verdict — 2026-10-08: PASS

On main `483810a` (001 T007, T008, T009; 005 T004 with its tie-group amendment), release binary SHA-256 `cd99e35a…`, deterministic full sets through the frozen apparatus (`g1quick.sh` at SAMPLE=1/1, its acceptance mode), stores copied from the frozen baselines and brought to schema `"5"` by `repair-index` (rust-lang/rust 52 s, 580 MB peak). Every set has 0 duplicate, missing, retried or apparatus-error tasks.

| Set | Targets (baseline → city map) | Tokens |
| --- | --- | --- |
| checker | D1 89.1 → 97.8%, D2 82.1 → 98.0%, D3 77.1 → 92.8%, U1 8.2 → 86.6% | median to pass 806 → 234 (grep 682); mean 2,056 → 1,440 |
| qualified | D4 45.7 → 94.0%, U2 8.0 → 81.2% | median 2,017 → 234 (grep 674); mean 2,313 → 1,153 |
| bun | B1 93.1 → 99.1%, B2 16.2 → 96.8% | median 1,994 → 184 (grep 647); mean 1,718 → 455 |

Every guard passes; the closest are definitions with more than 16 names (pass@1 20.9 → 25.0%, body 20.5 → 21.7%) and usage with more than 16 names (6.3 → 6.8%, allowed drop 0.6). Usage with 2–4 names went 54.3 → 86.4% (50.5% before the amendment) and with 5–16 names 38.3 → 48.9%. Results, responses and the per-task comparisons stay outside Git.

## 005 T004 amendment: doors for a tie group — 2026-10-08

Merged at `483810a`; unreleased. When a first anchor is ambiguous and its tie group (the window entries the resolver cannot tell apart) has at most 4 definitions, each gets its own exact door group of at most 4 lines right after its entry (`doors:each`), packed after the address pass and before signature upgrades; an entry without exact doors gets none, and with none for every entry the name's approximate doors follow the list once, joined from every tie root. A stale entry promotes nothing, in one root or across roots. At most 4 references windows per root's final read. A Head form for ambiguous lists was rejected: both refuters (Anthropic Opus 5.5, OpenAI GPT-6 Astra) showed it would be a directory line credited only by the scorer's fence rule.
- **Review.** Cross-lab by OpenAI GPT-6.1 Sol over two rounds: REVISE (final backtracking could split a door group; the multi-root approximate fallback took only the first root's group), SHIP. Author: Anthropic Opus 5.5.
- **Gates.** fmt, the full test suite and Rust 1.90 check passed at `70b97e6`; two clippy findings (a redundant closure, a test type) were fixed in `483810a` and clippy then passed on stable and Rust 1.90.
- **Latency.** Exact-door latency for common names with 2–4 namesakes (up to 4 windows per root) was recorded on 2026-10-08, under *Measurements on the development host* above.

## 001 T008 languages — 2026-10-08

Merged at `3e0a117`; unreleased. G1's final verdict is recorded on the city-map binary with the 005 T004 tie-group amendment.
- **Shape.** Runtime `tree-sitter` 0.26 with 15 more grammars (C#, F#, VB.NET, PHP, Perl, shell, PowerShell, Ruby, Kotlin, Swift, Scala, Lua, Dart, Elixir, Haskell; release binary 44.6 → 92.4 MB); search schema `"5"`. Every parse reads one character per input call under a 2^25-unit work budget, so a stop is deterministic; a stopped or panicking parse indexes as plain blocks (`parse_stopped`, `parse_panicked`). A scanner audit of all 22 external scanners (call graphs, loops, serializers, about 400,000 fuzzed parses) found five native hazards, guarded before parsing (F# comment nesting and Perl bracket nesting above 8,192, Perl heredoc words over 512 bytes, Python sources with over 256 indent widths, a Kotlin `@` with no line break after it). Three grammars resolve from owner-approved forks pinned by git rev: Ruby `1a594bf` (heredoc serialization bound), Perl `0686313` (scanner frees its heredoc queues; it leaked about 1 KB per heredoc per parse: +7.3 MB over 2,100 parses before, +16 KiB after) and Rust `d43cece` (rustc 1.99 nightly syntax: `const impl`, `[const]` bounds, const/auto/impl-restricted traits, macros 2.0, `safe` extern items, trait aliases; error files under `library/` 188 → 1 and `compiler/` 63 → 1; 34,707 of 34,723 error-free corpus files keep a byte-identical tree, the other 16 were silent mis-parses). Packaging reads license files from the source Cargo resolved (`cargo metadata`), so git pins package. A unit keeps at most 16 qualifiers (nested one-letter modules: 864 → 197 MB peak indexing a 200 KB source).
- **Review.** Cross-lab by OpenAI GPT-6.1 Sol over four rounds: REVISE (6 major), REVISE (packaging from a git pin, the Perl scanner leak, a quadratic leading-comment walk, PowerShell operand alignment), REVISE (`safe`/`auto` type names in the Rust fork, interpolated PowerShell strings), SHIP with one minor (a bare PowerShell token containing `$` can still give a key; fixed with the next merge). Captain findings fixed in review: a Perl `package` statement dropped its last `sub` (the grammar's last block runs over trailing whitespace), qualifier memory. Author: Anthropic Opus 5.5.
- **Gates.** fmt, clippy (stable and Rust 1.90), the full test suite and Rust 1.90 check passed at `3e0a117`.
- **G1** (deterministic, full sets, against `0882766`): checker D1 97.8%, D2 98.0%, D3 92.8%, U1 84.2% and every guard but usage on 2–4 names (50.5% against 54.3%, 50.1% before T008); qualified PASS (D4 94.0%, U2 60.0%); bun unchanged. Tasks whose result changed: checker 209 of 6,881 (definitions 135 gained, 22 lost, 9 otherwise changed; usage 32 gained, 5 lost, 6 otherwise), qualified 119 of 2,551 (111 gained, 3 lost, 5 otherwise), bun 0 of 999. The losses are definitions with more than 4 namesakes, which newly parsed files add.

## 005 T004 doors — 2026-10-08

Merged at `d00fc19`; unreleased. G1 judges it with 001 T007; the verdict is recorded once on the final city-map binary (001 T008's Rust grammar fork changes the checker set's definitions).
- **Shape.** A context request that asks who uses a resolved anchor gets doors: exact doors from the definition's stored name range through the same references window as `references {handle}` (shared file window and record allowance, inside the response's final read), else approximate doors from `ident` and index-time import keys; one door line per file, 16 lines, then `⋯`. Header states `doors:exact|approx|ambiguous|none`; `candidates:full` and `stale:` cover door windows; `graph:` is removed with the span-seeded graph expansion. `references` takes a handle alone. A target that goes stale before the final read gives `doors:none` and never promotes a runner-up, in one root or across roots (pre-validation heads carried through the merge); later roots follow the first serving root's decided strategy.
- **Review.** Cross-lab by OpenAI GPT-6.1 Sol over two rounds: REVISE (5 major: per-scope hash check of the file cache, the primary's doors request reaching other roots, stale-target promotion, a lost exhausted-scan flag, the door window not shared with `references`; 1 minor docs), SHIP. Each major has a regression test that failed before the fix. Author: Anthropic Opus 5.5.
- **Gates.** fmt, clippy (stable and Rust 1.90), the full test suite and Rust 1.90 check passed at `d00fc19`.
- **Integrated G1 before the review fixes** (T007 + T004, deterministic, full sets): bun PASS (B1 99.1%, B2 96.8%); qualified PASS (D4 89.7%, U2 58.1%); checker FAIL on D2 93.6%, D3 89.1% and the usage guards for 2–4 names (50.1% against 54.3%) and 5–16 names (35.1% against 38.3%). Most checker definition misses (27 of 35 D2, 39 of 77 D3, 17 of 24 D1) had the definition inside a tree-sitter-rust parse error on nightly syntax; the owner chose to fork the grammar (001 T008).

## 001 T007 city-map core — 2026-10-08

Merged at `4e23dee`; unreleased. Its G1 verdict is recorded with 005 T004 (G1 judges them together).
- **Shape.** Search schema `"4"`: one definition document per unit (the one holding its name node) with role, exact-case name hash, address hashes, name range and per-file import keys. Queries take up to four anchors; each anchor gets a resolver window (qualifier hits, exact case, role, then today's tier-1 order). A resolved anchor shows its definition through verbatim, signature and `[address]` plus up to 8 namesakes; an ambiguous one lists up to 16 in three passes (address lines for all, then signature, then verbatim upgrades). Compact context and its pointers are removed. In a multi-root owner the anchors are chosen once over every root. New definition units after G1 samples exposed unanswerable names: Rust trait/extern declarations and enum variants, module-level bindings in TS/JS, Python and Go, enum members in TS, Java, C and C++, named Go type specs; Rust `impl` blocks are containers. Address qualifiers come from the syntax tree, never from qname text.
- **Review.** Cross-lab by OpenAI GPT-6.1 Sol over three rounds: REVISE (4 major: cross-root anchor selection, `->` in generic stripping, quoted TS enum members, grouped Go specs; 3 minor), REVISE (comments and C++ shifts in generics, escaped quoted names: fixed by reading qualifiers from the tree), SHIP. A greedy packing pass that could drop an ambiguous candidate while showing another's body was found through a nondeterministic test (7 of 12 runs failed) and replaced by the three-pass rule. Author: Anthropic Opus 5.5.
- **Gates.** fmt, clippy (stable and Rust 1.90), the full test suite and Rust 1.90 check passed. Ten tests whose queries now anchor were reworded to keep their intent (plain-word queries for graph, dense and two-tier paths; expected bytes in anchor order for 013's composition).
- **Named limitations.** Definitions generated by macros (`define!`, `cfg_select!` bodies) have no unit. Constructs tree-sitter-rust 0.24.2 does not parse (the `[const]` bound, `const impl`, macros 2.0) had none either; 001 T008's grammar fork parses them.

## 001 T009 parallel indexing — 2026-10-08

Accepted locally at `89a8c54`; unreleased.
- **Shape.** `src/store.rs` refresh hands each page's sources out in key order to min(available parallelism, 8) scoped threads; at most 64 MiB of source bytes are out and not yet added. The single writer adds documents in key order as builds finish, then commits and reloads before the version-matched pending clear. A parse panic (`catch_unwind` per source) gives a named scan failure and the plain blocks of an unmapped source whose first document carries kind `unparsed`; `status` reports `parse_failures` by counting those documents and samples at most 20. A worker panic outside the parse guard, a writer-side panic and a corrupt recorded length end the page with an error, never a hang. Cancellation is checked at each hand-out and after the commit. No new dependency, table, META key or schema change.
- **Tests.** 1 versus 8 threads give equal documents and tables with the earliest key finishing last; panic fallback, status counting and persistence until reparse or `repair-index`; the hand-out bound counting built-but-unadded sources (at a bound reduced through a test hook); add, commit, reload and cancellation faults with replay; two crash stages, one with a build still in flight. The T009 tests take about 11 s in total.
- **G1 identity.** Copies of the G1 baseline stores (rust-lang/rust, oh-my-pi) rebuilt by the T009 release binary's `repair-index` gave the same result on all 10,431 G1 tasks (pass, calls, tokens and body), with no repetition, exclusion, duplicate or missing task.
- **Review.** Cross-lab by OpenAI GPT-6.1 Sol over four rounds: REVISE (M1–M5, m1–m2; M5 rejected by the captain and the contract amended), REVISE (m1), SHIP, then SHIP on the `unparsed` census changes made after cross-lab refutation by GPT-6 Astra. Author: Anthropic Opus 5.5.
- **Gates.** fmt, clippy (stable and Rust 1.90) and Rust 1.90 check passed. The test suite passed except one load-sensitive test, `tests/mcp.rs::http_dropped_stream_keeps_slot_and_overload_keeps_control_traffic_serviceable`: it drops an `index` request after 300 ms and expects the engine to be busy, and at host load 20–150 the request had not started yet. It passed 3 of 3 runs alone. It does not touch T009's code and is a pre-existing timing assumption.
- **Not measured:** index time and peak memory before and after, in the [measurement handoff](measurement-handoff.md) as a record.

## City-map evidence — 2026-10-07

Deterministic runs and arithmetic on recorded responses; no model calls. They motivate the owner-approved city map (001 T007–T009, 005 T004, 009 T004) and are not acceptance of it.
- **One call by name ambiguity** (library-only rust-lang/rust store, lexical, the frozen checker tasks, required evidence from `search` or `graph`): names with one library definition (SCIP name counts, a proxy for Foundry's) 74% dev / 74% held-out; 2–4 definitions 68% / 65%; 5–16 59% / 49%; 17–64 32% / 21%; more than 64 10% / 6%. On the full-repository store the comparable `search`-only definition figures were 77% / 71% and 7% / 2% (2026-10-06, below).
- **A second, scripted call recovers usage questions when the definition is known.** 400 failing tasks sampled per kind (seed 11), library-only lexical store:
  - definition failures: the definition was within the first 10 `search` locators for 8% (dev) / 2% (held-out) and within 64 for 27% / 32%; most of the rest have more than 64 same-name definitions;
  - usage failures: `search` (limit 64) listed the definition for 68% / 64%; given that definition (the script picked the correct one among the hits, which was hit #1 in 92 of 400), `references` covered the required sites for 265 of 265, median about 185 delivered tokens; with a single library definition, 68 of 78 failures recovered.
- **Path-seeded graph expansion expands the wrong symbols.** ``context "callers of `sleep_ms`" --strategy graph`` on the full store returned `fn sleep_ms` and then `Duration` constants (`Duration::SECOND`, `MILLISECOND`, …): `context_graph_units` seeds from every occurrence inside the top units, not from the queried symbol.
- **A Bun monorepo** (oh-my-pi `5b8d5b8a15`, 8,436 files indexed in 65 s, 348 MB peak, single-threaded): `interface ToolSession` (`packages/coding-agent/src/tools/index.ts` lines 207–555, 19,500 bytes) is the first `search` hit for `ToolSession`, and the first context item at 8,192 tokens, but absent at 2,048: neither its verbatim nor its signature form fits and units have no smaller form, so the response delivered test helpers `function toolSession` (seven test files; tier 1's `def_name` is case-folded) instead. `bun.lock` and 217 `.snap` files are indexed as ordinary text.
- **Languages.** Foundry and Prakarana both parse the same eight code languages (Rust, Python, TypeScript/TSX, JavaScript, Go, C, C++, Java); every other UTF-8 file is plain-text only.
- **Grammar spike for 001 T008** (outside the repository): all 23 grammars build and parse on `tree-sitter` 0.26.13 with Rust 1.90; the original eight gave identical units on 7,357 of 7,358 real files and their syntax tests pass unchanged; the grammar archives add about 48 MB to a release binary (F# 14.1 MB); deeply nested Haskell and F# inputs parse in tens of seconds (a 68 KB Haskell file in 42 s), so T008 bounds every parse by a deterministic work budget; unparsed constructs are named in the contract's § Languages.
- **G1 apparatus and baselines** (001 § G1; frozen 2026-10-08 00:22Z before any city-map code, refrozen 00:51Z after two cross-lab refutations: apparatus, name-count buckets and four targets). The first baselines (G1 v2/v3, 2026-10-07) were discarded: two runs on the same tasks differed because ripgrep's parallel walk reorders the grep agent's output before its 50 KiB cut (7 of 21 grep responses equal), and on a host at load 120–150 the Foundry agent's calls hit the 5 s read deadline (49 of 1,000 bun tasks). G1 v4 sorts grep output, records repeated refusals, and its verdict fails closed (INVALID) on any repetition, exclusion, duplicate or missing task. Two independent full Foundry baseline runs agree on all 10,431 scored tasks (pass, calls, tokens); the second, on a quiet host, needed 0 repetitions. The first bucket regex miscounted `const fn` and `static mut` definitions (`rg -o` takes the leftmost match) and was corrected before any code. On an uncontended host the Foundry agent ran all 10,432 tasks in 228 s, and paired scoring of all three sets took 18 s. Baseline (current release binary): checker 6,881 tasks, Foundry pass@1 35.6%, pass≤4 45.1%, 2,056 tokens per task; grep pass≤4 52.6%, 7,491 tokens. Qualified 2,551 tasks: Foundry 35.0% / 40.8% / 2,313; grep 47.4% / 7,228. Bun 999 scored tasks: Foundry 61.3% / 61.6% / 1,718; grep 96.9% / 836. Per-group baselines and the targets are in 001 § G1.
- **G2 question set** (009 T004; outside the repository; frozen 2026-10-08 03:03Z before any T004 code). 388 identifier-free questions (197 over rust-lang/rust `library/`, answers SCIP definitions; 191 over oh-my-pi, answers top-level declarations or direct methods of top-level classes), written from code by Anthropic Opus agents without running any retrieval. A mechanical verifier checks offsets and scope and rejects answers inside macro invocations, 5-word overlaps with the answer's file, the answer's string literals, inflected name pieces and meaningful 3-letter pieces (46-case self-test). Cross-lab refutation by OpenAI GPT-6 Astra took three rounds: REFUTED (ambiguous answers, a macro-generated answer, copied comment text, misdescribed behavior), REPAIR (10 wordings, one scope clause), ACCEPT. The scoring harness (one `context` call per question, the frozen G1 evidence parser, fail-closed integrity, exact one-sided McNemar divided by the number of candidate profiles) was frozen with it; repeated runs were identical and a positive control passed 6 of 6. No G2 measurement has been run.

## Embedding model comparison: Nemotron 3 Embed 1B vs EmbeddingGemma 2 — 2026-10-07

Owner-authorized exception to the measurement freeze, before signing; nothing in the repository changed except this record and the 009 status note. Decision rule (owner): switch only if EmbeddingGemma 2 delivers at least as much required evidence. **Result: rule not met; Nemotron stays.**
- **Runtime.** No released MLX package supports EmbeddingGemma 2 (mlx-vlm 0.7.6 has no `embedding_gemma2`; support is an open pull request), so it ran through llama.cpp at build `b9acf138` (includes EmbeddingGemma 2 support, PR #30054), statically linked with Metal, using the ggml-org GGUF `bfcd2987` (BF16, converted from `google/embeddinggemma-2@914f7f89`, Apache-2.0). Parity against the reference sentence-transformers 6.1.0 float32 implementation: cosine ≥ 0.99999 on 20 queries and 40 code windows (Q8_0: ≥ 0.99982).
- **Evaluation-only harness** (outside the repository, not product code). Foundry's embedding path is fixed at 2048 dimensions and the Nemotron `passage: `/`query: ` recipe, so a llama.cpp worker speaks Foundry's unchanged worker protocol inside the same App Sandbox bundle machinery. It decodes Foundry's token IDs, re-renders them with Gemma's prompts (`task: code retrieval | query: `, `title: none | text: `), keeps the first 768 or 256 dimensions, renormalizes and zero-pads to 2048. Zero padding leaves cosine similarity unchanged. Both arms embed the identical 15,026 units because Foundry partitions with the Nemotron tokenizer in both; decoding round-trips exactly on 200 of 200 sampled code windows.
- **Protocol.** Library-only store (the 2,255 files of rust-lang/rust 1.99.0 `library/`, where every checker answer lives) with the 005 SCIP graph; the frozen 4,597 development and 2,284 held-out tasks (checker v3 `e190b7dd…f6c8`, pass logic unchanged); `context` at 2048 tokens, `search` and `graph`, through `foundry mcp --semantic-profile` (every response `semantic:ready`).

| Required evidence delivered | dev search | dev graph | held-out search | held-out graph |
| --- | ---: | ---: | ---: | ---: |
| Lexical only | 49.3% | 22.2% | 44.1% | 19.5% |
| Nemotron (MLX 4-bit, 2048-d) | 48.3% | 22.8% | 40.9% | 20.0% |
| EmbeddingGemma 2, 768-d | 48.0% | 22.4% | 41.5% | 19.6% |
| EmbeddingGemma 2, 256-d | 47.8% | 22.3% | 41.2% | 19.5% |

- **Paired comparisons** (tasks gained / lost, exact McNemar):
  - Gemma 768 vs Nemotron: search dev 70 / 83 (p = 0.33) and held-out 45 / 30 (p = 0.11), so within noise; graph dev 17 / 35 (p = 0.018) and held-out 6 / 16 (p = 0.052), so Gemma is worse. Gemma 256 is worse than Nemotron on graph (dev p = 0.001, held-out p = 0.043).
  - Both models vs lexical only: search loses significantly (Nemotron dev 89 / 134, held-out 28 / 101; Gemma 768 dev 58 / 116, held-out 17 / 75; all p ≤ 0.003). Nemotron gains on graph for dev (51 / 21, p = 0.001).
  - The loss is in usage questions (dev search: lexical 33.3%, Nemotron 30.7%, Gemma 29.9%; held-out 35.3%, 27.5%, 28.6%): fused dense candidates displace reference-site units. Definition questions are unchanged (58.2%, 58.1%, 58.1%).
  - These tasks name an identifier; the set has no natural-language query without one, which is the case dense retrieval exists for. Delivered tokens were equal across arms (mean 1,807–1,812 for search).
- **Cost** (same machine; MLX 4-bit vs llama.cpp BF16, so runtime and model both differ):
  - Full preparation of the 15,026 units: Nemotron 6,889 s in three bounded rounds; Gemma 768 2,139 s in one, alone (3.2× faster). Gemma 256 took 2,098 s while two checker runs shared the machine.
  - Worker peak footprint: Gemma about 1.62 GiB (eight 120-line windows, 4,096-token context); Nemotron about 1.6 GiB at a 2,048-token input (009 D001).
  - Weights: 629 MB (Nemotron 4-bit artifact) vs 532 MB (Gemma BF16 GGUF).
  - Index and cache size are computed, not measured, because the harness pads every vector to 2048: an F16 index of 15,026 vectors would be 61.5 MB at 2048-d, 23.1 MB at 768-d and 7.7 MB at 256-d.
- **What a switch would have needed.** Profile-owned dimensions and prompts (today `DIMENSIONS`, the prefixes, the cache row layout and the frame size are constants), a llama.cpp worker in place of pyo3 and MLX, and a new signed bundle. The harness worker shows the llama.cpp path works under the existing protocol and sandbox, and it is the template if a future model without MLX support wins.

## Learned-router economics and tier-1 ranking — 2026-10-06

Owner-approved after the 013 corpus analysis: (1) account for the learned router by economics, (2) fix the deterministic retrieval causes first, (3) decide a second learned decision from the rerun. Offline arithmetic and deterministic `context` runs only: no model call, no timing claim.
- **Router counterfactual** (recorded round-1/round-2 evaluation cases joined with each row's task-checker evidence; no new runs). Round 1, 71 held-out rows: deterministic routing and the served router both delivered the required evidence on 64 rows (143,142 vs 143,136 delivered tokens); the label would deliver 71. The router changed one route, on a row where both strategies passed. All 7 deterministic misses chose `search` where `graph` was needed (mostly "what would break if `X` changed"); the model chose `search` on all 7. Round 2 (101 rows): the same single change, 92 both ways. Label accuracy rose 59 → 60/71 without any evidence gain.
- **Enablement gate** (013 contract amendment, `24abb11`). Evaluation reports `economics` (evidence and delivered tokens for baseline, routed and label; changed, gained and lost routes). `learning select` and owner startup require `gained > lost` (`economics_unknown`, `candidate_no_benefit`); token-only savings never enable. `learning select --lifecycle-check` is the visible, status-marked override for T004/D001 package acceptance only. Round 1 would be refused.
- **Tier-1 cause and fix** (context-v2 amendment, `c3437e6`). Tier 1 matched every identifier run of a query, English words included (`find`, `is`, `of`), kept a key_hash-arbitrary 64 and sorted them by path: `find \`sleep_ms\`` returned `fn find` definitions and never `fn sleep_ms`; `callers of \`sleep_ms\`` seeded graph expansion from `fn of`. Tier 1 now uses backtick-marked runs when present, ranks runs by exact filtered definition count and fills its 64 slots per run. Also accepted: the MCP `lines` array form and an empty-selection refusal naming the handle's lines. tools/list measures 995 of the 1000-token ceiling.
- **Rerun protocol.** `econ_check.py` (SHA-256 `fafba7ee…ac2ac2`, frozen 19:35Z before any fix) regenerates the 4,597 labeling tasks exactly and adds 2,284 held-out tasks in eight wordings never used for development (four marked, four unmarked). `context` at 2048 tokens, `search` and `graph`, on a schema-6 copy of the rust-lang/rust 1.99.0 store. The baseline binary (`77bf120`) reproduced every labeling-run pass flag and search token count.

| Required evidence delivered | search | graph | either | `auto` |
| --- | ---: | ---: | ---: | ---: |
| Dev 4,597, before | 22.2% | 13.0% | 23.7% | 22.2% |
| Dev 4,597, after | 42.1% | 23.1% | 44.1% | 40.6% |
| Held-out 2,284, before | 30.1% | 16.2% | 32.2% | 30.1% |
| Held-out 2,284, after | 34.3% | 17.6% | 36.5% | 34.5% |

- **Per task** (search / graph): dev gained 930 / 566 and lost 16 / 99; held-out gained 155 / 101 and lost 59 / 70. Paired dev wordings now match: "find `X`" 12.5% → 53.2%, "where is `X` defined" 42.7% → 53.0%.
- **Route keywords withdrawn.** Adding `uses`, `used`, `break`, `breaks` and `referenced` lowered `auto` evidence after the fix (dev 40.6% → 38.0%, held-out 34.5% → 34.2%): graph context places path-seeded neighbors, not the queried symbol's references, and delivered less than search for every usage wording except held-out unmarked "callers of X". The shipped list is unchanged.
- **Residual causes.**
  - Ambiguous names: definitions whose name has more than 64 library definitions pass 7% (dev) and 2% (held-out); unique names pass 77% and 71%. The query does not say which definition is meant.
  - Unmarked queries rank English words by the same specificity rule: held-out "where is X defined" fell 43.6% → 38.6%, while marked wordings rose.
  - Usage questions: at most 37.8% even for the best wording; `context` is not a reference listing.
  - Tokens: every response still fills the budget (mean 2,014–2,019 of 2,048).
- **Compact sufficiency** (`search` at 512 tokens, same tasks, fix binary). 56% of dev and 58% of held-out definition passes at 2048 also pass at 512 (46% / 38% for usage), at about 480 instead of 2,015 delivered tokens; three tasks passed only at 512. A compact decision saving about 1,530 delivered tokens must be weighed against a miss costing about one agent turn (12.4–13k tokens per OMP request, 003 T005 root cause).
- **013 note.** Composed route states (query, coverage, top-3 locator lines) change under the new tier-1 order; states composed before `c3437e6` do not match serving composition, so a next round recomposes and relabels.
- **Gates** (`24abb11` with the keyword withdrawal): fmt; clippy `-D warnings` on stable and 1.90; full suite 871 passed, 0 failed, 8 ignored.
- **Review.** Anthropic Opus 5.5 wrote both slices; OpenAI GPT-6.1 Sol reviewed them: tier-1/lines SHIP; economics gate REVISE (1 major: the lifecycle mark was lost on failed startup and inspection), then SHIP.
- **Compact context** (owner chose the fixed rule over a learned decision; `844796c`). When every backticked identifier with a definition has exactly one, `context` delivers that definition plus at most 8 one-line pointers and marks the header `compact`; other responses are byte-identical. Same frozen tasks (checker v2 `038182f6…53adb4` adds body/pointer and header fields; pass logic unchanged), compared per task with the pre-compact run:
  - Applied to 14.3% of dev tasks and 7.0% of held-out tasks (half the held-out wordings are unmarked and never compact).
  - On those rows: mean delivered tokens 2,012 → 824 (search); required evidence dev 458 → 458 (search) and 401 → 399 (graph), held-out 119 → 114 and 100 → 97. The held-out losses are usage tasks whose third reference site lay beyond the 8 pointers. Definitions arrive as bodies in 317 of 324 dev passes.
  - All tasks: delivered tokens −8.5% (dev) and −4.1% (held-out); evidence 42.1% → 42.1% (dev search), 34.3% → 34.1% (held-out search).
  - Arithmetic, not a host measurement: on the held-out compact rows, 5 extra misses at about one turn each (~62k tokens) against 160 × ~1,190 fewer delivered tokens (~190k) on the first delivery alone.
  - Remaining room (non-compact tasks; `search` at 512 vs 2048 on the pre-compact binary, whose non-compact output is byte-identical; arithmetic on recorded runs, one turn taken as 12.7k tokens, replay factor = later turns that re-send a response):
    - Dev 3,938 / held-out 2,124 tasks: short enough 16.5% / 15.5%; needs the full budget 21.0% / 15.8%; fails at both sizes 62.4% / 68.7%. Shortening saves about 1,535 delivered tokens per response.
    - Net delivered tokens per task (dev; held-out): shorten when the name has more than 64 definitions +178 / +910 / +1,641 at replay ×1 / ×3 / ×5 (+282 / +978 / +1,675); more than 16 definitions +55 / +1,267 / +2,479 (+325 / +1,477 / +2,629); always 512 −1,128 / +1,943 / +5,014 (−475 / +2,594 / +5,662), break-even at replay 1.73 (1.31); a perfect decision +1,213 / +3,640 / +6,067 (+1,290 / +3,871 / +6,452). Definition counts here are the library SCIP counts, a proxy for Foundry's tier-1 counts.
    - The needs-full tasks are mostly definitions (dev 652 of 826; held-out 264 of 336) of names with 2–16 definitions (69% / 78%), delivered at positions 1–9 (82% / 87%) among same-named definitions, which tier 1 orders by path. The query does not say which one is meant, and a learned decision sees only the query, coverage and top-3 locator lines, so most of the gap to a perfect decision is not in its input.
    - Not measured: `graph` at 512 and the replay factor of real host sessions (host compaction drops old tool results).
  - Gates (`844796c`): fmt; clippy `-D warnings` on stable and 1.90; full suite 877 passed, 0 failed, 8 ignored. Review: Anthropic Opus 5.5 wrote it; OpenAI GPT-6.1 Sol reviewed it: SHIP.

## 009 runtime fixes after measurement — 2026-10-06

Implementation only; measured in the end-of-implementation batch (owner, 2026-10-06).
- **Why.** The 2026-10-06 measurement found two problems. During continuous preparation every query fell back with `provider_busy`. On the 60,739-file corpus, every request in the first ~12 minutes failed with engine `busy`, because back-to-back driver store steps held the engine slot.
- **Shape** (spec 009 T003 decisions D5–D8):
  - Batches of at most 2 inputs while foreground activity happened in the last 60 s, otherwise 8.
  - One query may wait for this owner's document batch, up to its own ceiling.
  - Requests wait for a driver store step, bounded by their deadline. A single FREE/DRIVER/REQUEST holder mark decides who waits.
  - Time-bounded partition steps.
  - Generations are built and staged without the engine slot.
  - Status names a dead worker (`fallback:provider_exited`) instead of `ready`.
  - Dead owners' scratch is reclaimed at launch under the launch control. It is first claimed into a non-run `.reclaim-*` quarantine and deleted only if it is the validated inode.
  - The readiness wait never enters a receive once the deadline has passed.
  - The `embed_worker` and `learning_worker` load-timing flakes were fixed at the root cause.
- **Review.** Anthropic Opus 5.5 wrote it; OpenAI GPT-6.1 Sol reviewed it: REVISE (6 major, 2 minor), then REVISE (3), then SHIP. Each fix has a barrier or fault-point test.
- **Gates** (`b1466ca`, all slices integrated): fmt; clippy `-D warnings` on stable and 1.90; full suite 858 passed, 0 failed, 8 ignored, no flake.

## Installable package — 2026-10-06

Unsigned (ad-hoc-signed development bundles). Signing is the last step, by owner decision (2026-10-06).
- **Shape.**
  - `scripts/package.sh` builds `context-foundry-<version>-macos-arm64.tar.gz`: the core, optional `foundry-embed`/`foundry-learn` with their bundle scripts, README, LICENSE, THIRD-PARTY (331 packages) and `PACKAGE.json` (every file's SHA-256 plus the dependency identities). No weights, datasets, profiles or credentials.
  - `scripts/install.sh` supports `install`, `upgrade`, `rollback`, `disable-semantic`, `disable-learning` and `uninstall`.
  - Versions install side by side, selected by an atomically switched `current` link.
  - Worker bundles are built at install time from the operator's profiles, which are written back as installed copies.
  - One lock and one recorded intent (`pending.json`) make every interrupted command recoverable: it completes or rolls back from recorded hashes only.
  - Refusals happen before any change: running owners, symlinked layout, a foreign launcher, non-ASCII or control-character paths.
  - Uninstall removes only owned files that still match their hashes, plus the `connect` host-config block (byte-exact restore through `connect --remove-config`).
  - `bootstrap` reports a `versions` object.
- **Review.** Anthropic Opus 5.5 implemented it; OpenAI GPT-6.1 Sol reviewed it over four rounds (REVISE ×3, then SHIP). The findings fixed were:
  - deletion through replaced symlinks;
  - pruning of unowned empty directories;
  - overwriting an operator launcher;
  - unrecoverable interruptions;
  - ownership recovered by rehashing;
  - glob-based temp cleanup;
  - a symlinked lock;
  - busy-check gaps;
  - marker substrings in host configs;
  - recovery bypassing the busy gate;
  - post-commit rollback;
  - unproven host cleanup;
  - foreign `current` links.
  
  The remaining limits are documented in deployment.md: a check-to-delete TOCTOU window, an owner starting after the final busy check, and recovery's own temporaries.
- **Tests.** `tests/install.rs` (5): package contents and manifest; install, run, upgrade, rollback and uninstall with user data kept; busy refusal; worker bundles with fake workers; byte-exact host-config removal; interrupted-command recovery at every recorded point; refusal cases. Since 2026-10-08 the host-config removal test is in `tests/mcp.rs`, and `tests/install.rs` builds only on macOS arm64, the package target.
- **Real smoke** (release build with both real workers):
  - Install builds both ad-hoc bundles. Semantic context is `ready` through the installed Nemotron bundle; context is `route:policy` through the installed learning bundle.
  - `connect --apply-config` works.
  - Upgrade to a relabeled package rebuilds both bundles and serves `ready`.
  - Rollback and `disable-learning` work.
  - Uninstall leaves the store byte-identical, the host config byte-exact, the supplied profiles intact and nothing under the prefix.

## Measurement phase — 2026-10-06

The owner deferred every measurement until the implementable spec tasks were done (2026-10-05). All runs used release builds and the development bundles; nothing ran under production isolation. Evidence: [009 T003 lifecycle](review/009-t003-lifecycle-2026-10-06.json), [013 rounds](review/013-measurement-2026-10-06.json).

- **009 T003 real lifecycle** (Nemotron 4-bit, production `foundry mcp`, this repository as the corpus: 193 sources, 2,099 units). It passed its predeclared bounds in 872 s:
  - cold preparation took about 845 s (about 2.5 units/s);
  - the query during preparation got baseline results with `fallback:provider_busy`, because continuous preparation keeps the one model slot nearly always busy;
  - steady queries took 1.25 s for the first (index load), then 184–215 ms;
  - after an edit, baseline `partial` came back in 195 ms; catch-up embedded exactly 1 unit; restart embedded 0.
- **013 rows.** 1,088 labeled tasks were composed into states by the core in 52 s.
  - 32 rows were recorded `allow_training:false`. 27 composed states recur across groups (the same symbol name in different modules yields the same query and locators), and one state carried conflicting labels. The contract refuses both as `duplicate_conflict`.
  - Rounds follow the owner's split: 770 rows in round 1, 318 in round 2.
- **Defect found and fixed.** The first sandboxed training run failed: App Sandbox moves the worker's working directory into its container, so the relative `head.safetensors` was written there.
  - The supervisor now passes `--run-dir`; the worker re-enters it and confirms it by device and inode.
  - A regression test starts the worker elsewhere.
  - Fake-worker tests could not see this; it is the reason the sandboxed run is in this phase.
- **013 round 1** (555 train, 124 calibration, 71 evaluation rows): 569 steps in 195 s, worker peak RSS 2.3 GB, temperature 3.0 (the grid's upper edge). Coverage 0.56 and accepted accuracy 0.90 (36/40), so it is eligible. Fallback-inclusive accuracy was 60/71 against 59/71 for deterministic routing, a one-row difference. Eligibility permits a trial; it is not an improvement claim.
- **013 round 2** (306 new plus 217 replay train rows; the evaluation reuses round 1's held-out groups): 434 steps in 200 s, 2.6 GB. Not eligible: accepted accuracy 0.8625 is below 0.9. It equals the incumbent (84/101) against 83/101 for baseline. This is a valid rejected candidate; the incumbent stays.
- **013 lifecycle (T004 verification items):**
  - represented input → `no_new_data`;
  - withdrawing an inherited contributor → `base_permission_changed` at prepare and `contribution_changed` before any update at train;
  - owner SIGKILL during training → worker gone in 41 ms, no candidate, incumbent unchanged;
  - select writes a config for round 1 and refuses round 2 (`candidate_ineligible`) and any overwrite;
  - rollback is restarting without the config.
  - Limitation: a killed owner's scratch run directory (100 MB staged base head) is not reclaimed by later runs.
- **013 serving.**
  - Resident MCP owner with the selected candidate: start 13.2 s, first routed call 1.3 s, steady 265–435 ms, against 66–160 ms without a policy.
  - A CLI call with `--policy-config` pays the model load each time (about 8.6–9 s).
  - The learned router stays off by default. The owner declined the paid enablement comparison.
- **013 numerics.** The exhaustive comparison holds for every element of the full gradients (34- and 1024-token cases) and of the post-sequence parameters, within atol 1e-5 / rtol 1e-4. The signed-bundle negative probes pass.
- **Not measured:** packaged install/upgrade/disable/uninstall and the distributed profile (013 T004, 009/013 package acceptance) need signing. The aggregate 009+013 residency was not run as a joint test; observed peaks were about 2.6 GB for the training worker, and the 009 worker's ceiling is 3 GiB.

## 013 T003 serve, select, roll back and retire the legacy HTTP path — 2026-10-05

Development isolation only. Deferred to the measurement phase (owner, 2026-10-05), not run: the enablement comparison (the same checked tasks with `foundry usage import`), the aggregate 009+013 residency test, and latency figures. The learned policy stays off by default. (Superseded 2026-10-06: enablement now requires the economics gate above.)
- **Shape.**
  - `src/policy.rs`: config v2 validated at owner startup. An invalid config is `policy_config_invalid`, with baseline retrieval intact.
  - `learning select`: validates the candidate and writes a new config by no-replace publication. Rollback is starting with the prior config or none.
  - Serving: `foundry-learn` loads the candidate and answers contract IPC-v2 requests over the shared frame. The core validates the reply and thresholds the unrounded maximum probability.
  - Slot: one prediction, no waiting. The wait is min(2 s, half the remaining read deadline); below 50 ms the model is skipped. A request is on time only if its reply is published to the core before the ceiling. Three consecutive timeouts, or any malformed, wrong-identity or dead reply, terminate the worker and make the policy unavailable.
  - Routing: only for `auto` with a current graph, after the 009 merge and before graph expansion. The `route:` header segment (context-v2 amendment) appears only when a policy is configured; otherwise output is byte-identical.
  - `src/laya.rs` and its HTTP client are deleted. Legacy feedback/export is unchanged. `--laya-port` is refused with migration guidance.
- **Review.** Anthropic Opus 5.5 implemented it; OpenAI GPT-6.1 Sol reviewed it. Round 1, REVISE (3 major): a late reply granting a route and resetting the count; a policy timeout consuming the whole read deadline; evaluation rows' rights not checked at selection. Round 2, REVISE: an on-time reply could still be counted as a timeout. That was resolved by one publication-based linearization point per request, a captain decision recorded in the contract. Round 3, SHIP. Real prediction parity against T002's evaluation probabilities: 21 rows, bit-exact (release build).
- **Gates** (`5585a6e`): fmt; clippy `-D warnings` on stable and 1.90 (default, `--no-default-features`, `learning-worker`); full suite 820 passed, 0 failed, 8 ignored.

## 013 T002 tch training worker, calibration, evaluation and candidates — 2026-10-05

Development isolation only. Deferred to the measurement phase (owner, 2026-10-05), not run: training and evaluation on the real labeled rust-lang/rust rows, the exhaustive elementwise comparison against `reference-t002-full.safetensors`, and the signed-bundle negative probes. They are ignored tests and documented commands in [learning](learning.md).
- **Shape.**
  - Optional `learning-worker` feature (tch 0.24.0 / LibTorch 2.11.0), outside the default build. `build.rs` sets the rpath and works around LibTorch's libomp install name.
  - The `foundry-learn` worker is a numerical engine only. The core owns read-back, the pre-fit permission gate under exclusive ownership, seeded order, `max_steps`, the clock, calibration, evaluation, candidate validation and publication.
  - Worker IPC rides on the shared frame format with a request ID and the loaded identity. A duplicate, stale or late reply is terminal.
  - Trainable: the head, the scorer and type-embedding row 0 only.
  - AdamW with clip 1.0. Temperature grid 0.5–3.0. 15-bin ECE. Query-only deterministic routing as the comparator.
  - Candidates: float32 head plus manifest, published through anchored no-replace publication. Inherited per-option temperatures and a fitted scalar below 0.5 are refused.
  - Enforcement matrix: hard process count, output and CPU; supervised memory, failing closed.
- **Numerical parity** with a reference generated from unchanged Laya @4066d5d5 (transformers 5.18.0, torch 2.14.1). It covers 5 cases (34 to 1024 tokens) and a 3-step AdamW sequence with two clipped steps:
  - logits and losses exact;
  - gradients ≤1.9e-13 absolute;
  - post-sequence parameters ≤2.5e-10;
  - post-sequence logits ≤2.4e-7.
  
  The tolerance is atol 1e-5 / rtol 1e-4.
- **Design and review.** The captain's brief was refuted cross-lab by GPT-6 Astra (refuter-b) before implementation. Findings folded in: the pre-fit consent gate; type row 0 only; query-only comparator; IPC identity; full validation before adopt; base versus candidate temperatures; fail-closed measurement; real ENOSPC paths; no encoder cache; a clipped reference step. Anthropic Opus 5.5 implemented it. OpenAI GPT-6.1 Sol reviewed: REVISE (4 major: duplicate final reply, breach after the last reply, reply crossing a deadline, manifest outside the output total), then SHIP.
- **Gates** (`390c282`, rebased on 009 T003): fmt; clippy `-D warnings` on stable and 1.90, default and with `learning-worker`; full suite 796 passed. The only failure was the known `embed_worker` launch-timing flake, which passed alone.

## 009 T003 progressive preparation in the MCP owner — 2026-10-05

Development isolation only. The real lifecycle exercise is deferred to the measurement phase (owner, 2026-10-05) as an ignored test (`real_lifecycle_exercise_on_a_permitted_declared_corpus`).
- **Shape:**
  - The resident `QueryRuntime` is the one model worker for query embeddings and document batches of up to 8, behind one slot. Admission claims the slot, then checks for an abandoned late call; a query being dispatched wins over a document admission. A refused document admission pauses preparation with `provider_busy`. Nothing is queued.
  - The shared preparation steps in prepare.rs serve both the CLI and the MCP driver.
  - The driver takes the engine slot only to select and to commit. Inference holds no slot and no transaction.
  - `index {semantic: prepare|pause}` controls preparation; `status` reports a `semantic` object. The catalog is 986 o200k tokens, within the 1000 limit.
  - Shutdown joins the provider, so the worker is reaped before the store is released.
- **Authors and review.** Anthropic Opus 5.5 wrote it; cross-lab review by OpenAI GPT-6.1 Sol. The review went REVISE (5 major: hidden late-query occupancy, commit after EOF, ownership released before the worker was gone, an admission race, pause racing admission), then REVISE (the active-to-abandoned transition), then SHIP. Each fix has a deterministic barrier test.
- **Gates** (`75cd840`): fmt; clippy `-D warnings` on stable and 1.90 (default, `--no-default-features` and `embed-worker`); full suite 742 passed, 0 failed, 7 ignored.

## 009 T002 semantic context within the existing budget — 2026-10-05

Development isolation only. [Evidence](review/009-t002-2026-10-05.json).
- **Shape.**
  - D001 merge: exact definitions first, then RRF k=60 over the lexical top 256 and dense top 64. Each arm votes once per delivery unit.
  - Stale lexical documents are dropped in the final read before fusion.
  - Dense-only hits never seed graph or compiler expansion.
  - The `semantic:<word>` header segment and the whole/lexical-span/preview forms ladder; neural batches deduplicate by full identity.
  - Generation format v2: the label map carries unit locations plus `source_revision` and `coverage`, so no request walks partitions. A v1 generation is unavailable by name and republished from the cache with zero document calls.
  - The index bytes restored are the bytes that were hash-verified.
  - `ready` only when the generation is complete at the response's own final-read revision.
  - The query embedding waits min(1500 ms, half the remaining deadline). A failed footprint measurement of a live worker stops it like a breach.
  - MCP starts one resident worker (`mcp --semantic-profile FILE --development-isolation`); the CLI takes the same flags.
- **Authors and review.** Z.ai GLM-5.3 wrote the first version; after the owner retired GLM, Anthropic Opus 5.5 completed it and wrote both fix rounds. Cross-lab review by OpenAI GPT-6.1 Sol: REVISE (8 major), then REVISE (one residual coverage case), then SHIP. Each fix has a dedicated test.
- **Real model** (Nemotron-3-Embed-1B 4-bit, production development bundle):
  - All three frozen vocabulary-gap spans are delivered at 2048 tokens (2,023 / 2,023 / 1,947 tokens); the lexical baseline delivers none of them.
  - No-profile output is byte-identical.
  - Preparation: 8 units, 1 document call, 2,969 input tokens. A one-file edit embeds 1 unit (140 tokens) and reuses 7.
  - After a delete without re-preparation: `stale:1`, `semantic:partial`, and the deleted source is never served.
  - A profile mismatch falls back by name.
  - Resident MCP: worker load 10.6 s, first query 314 ms, warm queries 24–32 ms.
  - The index is 34,032 bytes.
- **Gates** (`451033b`): fmt; clippy `-D warnings` stable and 1.90, default, `--no-default-features` and `embed-worker`; full suite 720 passed. The only failure is `embed_worker::a_ready_crossing_the_deadline_inside_one_receive_slice_is_never_accepted`, a 400 ms launch-timing test that already flakes on main (1 in 6 solo binary runs; the worker's PID file is not yet written at the deadline). It passes alone.
- **Known gaps, closed 2026-10-06** (`121eba2`). The owner had accepted them on 2026-10-05; each now has a behavior test that GPT-6.1 Sol reviewed (REVISE → SHIP). No product defect was found.
  - Semantic evidence crowded by graph/compiler evidence; dense-only hits seed no expansion.
  - Multi-root:
    - the wording says primary root only;
    - a secondary-only request spends no embedding and returns the plain bytes;
    - no secondary item leaks.
  - The no-profile byte-identity matrix with positive hits: CLI and MCP × search and context × single and multi root.
  - A worker dying mid-query: named `provider_exited`, then a stable named fallback.
  - A localized lexical-span preview continues to the span's end.
  - CRLF, multibyte and fence-like bytes stay exact in every semantic form.
  - The MCP byte cap. It is measured at the packer with the owner's exact measure; end to end it takes 1.3 s in release, against 10 s in debug.
  - Exact session-allowance charging, including a refused request charged 0.

## 013 T001 exact permitted ModernBERT examples — 2026-10-05

Locally implemented and verified; no model is loaded and nothing is trained.
- **Shape.**
  - Store schema 6 adds `learning_feedback`, `learning_history` and `learning_datasets`. The explicit `upgrade-store --to 6` runs from v1–v5 in one transaction; every earlier table is preserved, the 009 vector cache included.
  - `feedback v4` is operator-only. MCP offers no feedback or consent path.
  - `learning prepare|check|compose-state`. `check` takes `--policy`, because the policy pins the tokenizer the exact re-render needs (contract amended).
  - The renderer (`src/decision_model.rs`) matches unchanged Laya `build_sequence` @4066d5d5 on 9 pinned cases, including the 1024/1025 boundary.
- **Authors and review.** Z.ai GLM-5.3 wrote the first version. After the owner retired GLM for erroring, Anthropic Opus 5.5 wrote both fix rounds. Cross-lab review by OpenAI GPT-6.1 Sol: REVISE (10 major, 2 minor), REVISE (one new major), then SHIP. Findings fixed:
  - inherited contributions forgotten across rounds, now cumulative coverage plus a recorded lineage table;
  - a rights-assertion change not detected;
  - a split conflict hidden by the no-op path;
  - a hand-written JSON scanner that could loop, replaced by strict serde parsing;
  - reads before bounds checks;
  - pathname-only output containment, now judged through descriptors;
  - replace-on-publish;
  - forged token arrays accepted on read-back;
  - group ownership not checked;
  - empty bases accepted;
  - publication of a substituted staging source, now staged inside a held private directory with source and destination identity checks.
  
  A lost publication response is verified with a full read-back and adopted.
- **Gates** (`9e1c6bc`): fmt; clippy `-D warnings`, stable and 1.90, default and `--no-default-features`; the full suite, 679 passed / 0 failed / 6 ignored, with the recovery harness passing. `learning_data` has 67 tests.
- **Not yet done:** feedback v4 rows from the accepted labeling run (4,597 tasks, 1,088 labeled) are built with `compose-state` against the refreshed corpus store when T002 needs them.

## 009 T001 semantic preparation and supervised worker — 2026-10-05

Locally implemented and verified, **pushed 2026-10-05, unreleased**. Development isolation
only; normal admission stays `isolation_unavailable` until signing/notarization and
package acceptance. [Evidence](review/009-t001-2026-10-05.json).
- **Authors and shape.** Z.ai GLM-5.3 wrote two slices; the captain wrote the shared
  boundary (`provider`, `protocol`, `profile`). Slice A covers partition, the schema-5
  tables, cache, USearch generations, bounded prepare, status and purge. Slice B
  covers the worker runtime, the `foundry-embed` worker, the supervisor and the bundle
  script.
- **Review:** cross-lab by OpenAI GPT-6.1 Sol, four rounds per slice.
  - Slice A: BLOCK ×3, then SHIP. Findings fixed:
    - symlink and ancestor substitution in purge and publication, now anchored to
      a store descriptor held for the Engine's life;
    - the run deadline not reaching the provider or artifact verification;
    - duplicate-input generations;
    - partial runs never publishing, now under a publication reserve;
    - mapping ranges and exact-input key identity;
    - whitespace direction, and a depth cutoff now replaced by a work stack;
    - durable progress counters;
    - retained-profile and nonfinite cache classification;
    - status deadlines.
  - Slice B: BLOCK ×2, REVISE, then SHIP. Findings fixed:
    - an unexplained owner-death miss, now behind a pressure gate;
    - a lost exit event during watcher arming;
    - the worker's descriptor and inventory not being validated;
    - acquisition not bounded by the caller's deadline;
    - late replies and retries after the deadline;
    - site hooks running before isolation, now an isolated `PyConfig`;
    - a vacuous symlink probe;
    - scratch-root binding and overlap with read-only grants, aliases included;
    - timeout overflow.

  The captain recorded three decisions in spec 009: publication shares the budget,
  status trusts committed payloads, and destructive filesystem work is
  descriptor-anchored.
- **Gates** on `te-009` over main `59dcfa0`:
  - fmt, clippy `-D warnings` and the Rust 1.90.0 check and clippy pass, including the
    `embed-worker` and `--no-default-features` builds;
  - full suite **598 passed, 0 failed, 6 ignored**: five `--ignored` real-model
    development gates plus the existing economics test;
  - `tests/neural.rs` 51, `tests/embed_worker.rs` 46 (+5).
- **Real model.** A release `foundry` drove the production worker bundle (`3cb10e27…`,
  no test hooks) with the pinned Nemotron 4-bit profile (function digest
  `5e0e9cb8…`):
  - first preparation: 5 sources → 17 units, 3 document calls, 17 cached f32 vectors,
    an F16 generation of 17 entries, 15.8 s, 657 MiB peak (CLI process);
  - unchanged restart: 0 calls, 17 reused;
  - one-file edit: the old mapping was rejected immediately; re-preparation made 1 call
    and reused 16;
  - `repair-index`: rebuilt the semantic index from the cache with 0 calls;
  - purge: removed partitions, cache (including the orphan) and the generation, while
    lexical search kept working;
  - without `--development-isolation`: `isolation_unavailable`.
- **Development worker runs.**
  - Parity: the direct-ID path equals publisher `encode` (cosine 1.0000000, max
    difference 0) for batch 1 and 8, reordered and heterogeneous batches, and exact
    1024- and 2048-token inputs; 2049 is refused before any model call.
  - Owner death: across six pressure scenarios (cold and warm, CPU and GPU), the worker
    was gone within 2 s in every one, worst 102.7 ms. One earlier unacknowledged run
    (pid 95942) exceeded 2 s; it was not reproduced, and its cause is unknown.
  - Sandbox: negative probes, each with an unsandboxed positive control.
- **Limits.**
  - A SIGSTOPped worker cannot exit itself; package acceptance needs an OS guardian.
  - Memory is supervised, not hard.
  - The ad-hoc signed bundle is not a distribution package.
  - Semantic search delivery and MCP preparation are T002 and T003.

## 005 T003 measured run — rust-lang/rust 1.99.0, 2026-10-05

One run, executed once, against the declared profile. Every acceptance criterion
passed, with no failed check (50 checks).
[Evidence](review/005-t003-run-2026-10-05.json);
[preselected questions and source check](review/005-t003-questions-2026-10-04.json).
- **Inputs.**
  - Corpus: rust-lang/rust 1.99.0 (`b940084d`), clean before the run. Foundry admitted
    60,739 sources: 75 named exclusions, about 179 MB by the admission rule.
  - Host: Apple M3 Pro, macOS 27.
  - Binary: release `eac5d97b…8775` from main `f7a7233`, with no test-hook strings.
  - Producer artifact: `db306662…0dc6`.
  - Questions: preselected and source-checked before timing (`4f564f2`).
  - Profile, fixed before the run: `max_index_seconds=900`, `max_peak_rss_bytes=2 GiB`,
    `max_query_p95_ms=250`, `max_run_seconds=3600`.
- **Foundry (measured).**
  - Index: 429.6 s, 556 MiB peak.
  - Import: 30.0 s, 469 MiB; `complete: true`, 0 failed, `coverage: partial` (external
    references unresolved).
  - Queries: the 20 timed MCP `references` calls (tokens 4096) had p95 105.3 ms
    (nearest rank, sorted sample 19) and a maximum of 132.8 ms.
  - Session: 861.3 s, with the timer paused during the producer rerun.
- **Producer (disclosed separately).** Jailed rust-analyzer 2026-08-31 over the
  filtered `library/` snapshot:
  - first run 1,128 s at 1.07 GiB (2026-10-04);
  - edit-cycle rerun 73.3 s at 1.10 GiB (a warm scratch target directory);
  - end to end 934.6 s.
- **Answers.**
  - The 15 supported questions returned exactly their expected references on page 1,
    with `coverage:complete` and no continuation.
  - The five high-degree questions returned a correct nonempty first page with
    `next: after=`. Their full pagination equals the expected sets: 2,633 records.
  - All 2,780 records match on exact byte ranges, read back through single-record
    pages.
  - Controls held: a deep position seed, a corrupted handle, a foreign workspace, an
    unknown symbol and a same-name different symbol.
- **Edit cycle through one MCP owner.** An appended comment in
  `library/std/src/os/unix/net/datagram.rs` was picked up by MCP `index`
  (revision 60739 → 60740), and q02 then answered `coverage:stale` with no items. The
  producer was rerun on an edited snapshot copy and `index {scip}` re-imported. The
  same q02 then answered `coverage:complete` with its expected set. There was no restart
  and no competing writer; a competing CLI import was refused `store_busy`.
- **Protocol checks.**
  - Traversal, path and `.` names are `invalid_argument`.
  - Missing and symlinked staged files are `artifact_unavailable`.
  - A 65 MiB manifest is `manifest_too_large` (untimed supplement).
  - A 2 s deadline mid-import returned `complete: false` with committed counts.
  - Staged caller files were untouched.
- **Limits of this result.**
  - The producer input is a filtered, incomplete `library/` snapshot (named omissions in
    spec 005).
  - The independent source check found one reference rust-analyzer did not emit (q19:
    `library/compiler-builtins/compiler-builtins/build.rs:594`); the expected sets follow
    the artifact.
  - SCIP identities stay as produced: the `core 0.0.0` workspace instance is not merged
    into the sysroot instance (counts are in the questions record).
  - This tests one named corpus, not universal capacity. It is not a token-savings or
    cost claim.

## 005 T003 agent surface — MCP `references`, `index {scip}`, graph context, 2026-10-05

Locally implemented and verified, **pushed 2026-10-05, unreleased**. The measured
large-workspace run is recorded separately.
- **Authors:** Z.ai GLM-5.3 (implementer). The captain made the contract rulings during
  review (spec 005, "Amended 2026-10-05").
- **Review:** cross-lab by OpenAI GPT-6.1 Sol in four rounds:
  - round 1, REVISE with 10 Majors:
    - context units lacked snapshot provenance;
    - staged files were checked and then reopened by path;
    - ambiguous symbols expanded;
    - compiler units bypassed the 32-unit bound, including the multi-root merge;
    - corruption in the final read failed source context;
    - seed ranges went unvalidated;
    - deduplication turned a used graph into `graph_unavailable`;
    - field validation ran after routing and admission;
    - multi-root `references` headers were missing;
    - the budget floor hint could be too low;
  - round 2, REVISE: a same-snapshot completion could make a witnessed definition
    ambiguous, and the final read did not reverify definition source chunks;
  - round 3, REVISE: the final pass did not charge witness membership reads to its
    allowance;
  - round 4, SHIP.

  Each round's fixes were mutation-checked: disabling a fix makes its regression test
  fail. Midway, the implementer accidentally overwrote `src/mcp.rs` and rebuilt it. The
  captain confirmed only 2 removed lines against the base, and the reviewer read the
  whole rebuilt diff.
- **Behaviour.**
  - `references` is the seventh MCP tool (catalog 977 o200k tokens; ceiling 1000). It
    validates fields before routing and admission, routes handles by `ws16` like
    `retrieve`, uses the root-aware header, and answers while the lexical index is
    `repair_required`.
  - `index {scip}` imports files staged in `<store>/imports`. It opens and freezes
    them by descriptor (no-follow), inside the engine slot, with the timeout and
    cancellation of `index`.
  - `context` with the graph strategy expands only uniquely resolved symbols. One
    32-unit bound covers lexical and compiler units in single- and multi-root
    responses. The final read re-proves each expansion's snapshot, witness rows,
    uniqueness and source bodies within a separate ≤256-record allowance.
- **Gates** (worktree on `3a81f8c`): fmt, clippy `-D warnings`, Rust 1.90.0 `check`
  and clippy pass; full suite **474 passed, 0 failed, 1 ignored**.
  `http_dropped_stream_keeps_slot_and_overload_keeps_control_traffic_serviceable` (an
  existing test) failed once under concurrent build load: its 300 ms client timeout
  dropped the request before it reached the server. It passed 3/3 in isolation.
- **Integration notes.** `src/graph.rs` changes are additions only: lines 1–1928 are
  byte-identical to `3a81f8c`. Every removed line elsewhere was audited: the
  index schema and allow-list, catalog assertions, graph-state branches, the multi-root
  merge, the references header prologue and the `scip.rs` freeze signature.

## 005 compiler references — T001 and T002, 2026-10-04

Locally implemented and verified, **pushed 2026-10-05, unreleased**.
- **Authors:** Z.ai GLM-5.3 (implementer); captain integration (the `fault.rs` point
  union and the `Cargo.lock` three-way merge with the gateway commit).
- **Review:** cross-lab by OpenAI GPT-6.1 Sol:
  - round 1, REVISE with 8 Majors: same-start pagination bypassed `limit`; the narrowest
    position rule was wrong; no import `coverage`; failed documents counted toward
    resolution; failed runs retired scopes; key parsing could underflow; FIFO inputs
    blocked; the occurrence cap ran after allocation;
  - round 2, REVISE: a reachable panic on a missing definition source; non-atomic
    retirement under cancellation; a budget cliff and masked staleness; key decode
    before the examined cap; outside-scope inputs degrading coverage; ignored SCIP
    metadata still materialized; an ordinal label treated as a path. The captain added
    M10: on the real artifact, position seeds deep in a file exhausted the scan because
    rust-analyzer's whole-file module occurrence defeated the early stop;
  - round 3, REVISE:
    - a stored seed-key range not checked against the verified source body;
    - an oversized document identified by its first rather than last path, which let a
      duplicate slip past preflight.

    In flight, the captain added two Majors, both closed by round 4: definition lookup
    could consume the record reserved for reference progress, and the new decoder
    accepted field number 0 and unterminated groups;
  - round 4, REVISE: the rewritten oversized-message scan accepted a document truncated
    at physical end of file;
  - round 5, SHIP: the truncation guard closed it, and the reviewer checked that no
    other early-EOF path accepts a truncated message.

  Round 2 found no remaining issue with the FIFO refusal or with failed documents
  counting toward resolution. Each later round's fixes were mutation-checked: disabling
  a fix makes its regression test fail.
- **Gates on the integrated main tree.**
  - Source manifest `8ba6fa5a…4a00`, the SHA-256 of sorted file hashes over `src`,
    `tests` (minus the frozen economics corpus), `Cargo.toml` and `Cargo.lock`.
  - `cargo fmt --check` and `cargo clippy --all-targets -D warnings` pass.
  - The full suite: **435 passed, 0 failed, 1 ignored**; all recovery scenarios passed.
  - Rust 1.90.0 `check` and `clippy -D warnings` pass.
- **`tests/semantic.rs` (83 tests).** These run on the T001 fixture artifact
  (`tests/fixtures/semantic/`) and on crafted SCIP:
  - `a::parse_record` versus `b::parse_record`, Unicode before an occurrence, an
    indirect use, and stale-to-fresh after a third-file edit and reimport;
  - frozen copies, FIFO refusal, hash mismatch before selection, duplicate
    documents and inputs;
  - every import limit at its real value, and the counting-allocator proofs that 4
    million ignored symbols or diagnostics are skipped without being materialized;
  - strict wire decoding (field 0, groups, wire types 6/7, overruns) at every
    nesting level;
  - atomic finalization under cancellation;
  - the narrowest-range and tie rules, deep seeds, pagination and budget reserves
    (a property loop over seed costs 250–255);
  - corrupted rows, short IDs and missing definition sources named without panics;
  - oversized documents identified by their last path, and truncated messages
    refused before selection.
- **Real artifact.** The 005 T003 producer run (spec 005 § Owner answers, T003 producer run;
  [questions record](review/005-t003-questions-2026-10-04.json)) produced a 56.9 MB
  rust-lang/rust `library/` artifact (958 documents, 436,829 occurrences). Imported into
  the whole-repository store (60,739 sources):
  - `complete: true`, 0 failed, `coverage: partial` (unresolved external references);
  - 22.5 s at 480 MiB peak RSS (round 2); with the final release binary
    `fd3c166b…6e3a` (no test-hook strings), 35.3 s at 465 MiB;
  - all 20 preselected questions match their expected sets on exact byte ranges
    (2,780 records), read back through single-record pages;
  - the controls hold: a deep position seed, a corrupted handle, a foreign workspace,
    an unknown ID, and a same-name symbol that differs.

  This is functional evidence, not the T003 measured run.
- **Compatibility.**
  - Store schema 4: `upgrade-store --to 4` from v1, v2 or v3 in one transaction; v3
    memory and typed pending keys are preserved.
  - Older readers refuse. Rollback means restoring a pre-upgrade store copy and the
    previous binary.
  - New dependencies `scip` =0.10.0 and `protobuf` =3.7.2 (Rust 1.90 verified).
- **Integration notes.** Every removed line was audited: 87 lines, all schema 3→4
  edits, visibility widenings and test assertions; nothing was removed from
  `graph.rs`, `scip.rs` or `response.rs`.

## 003 T004 owned model gateway — 2026-10-04

Locally implemented and verified, **pushed 2026-10-05, unreleased**.
- **Authors:** Z.ai GLM-5.3 (implementer), plus captain fixes: the flush-before-abort
  in the response body, test-only fixes, and a Rust 1.90 clippy rewrite.
- **Review:** cross-lab by OpenAI GPT-6.1 Sol in three rounds:
  - BLOCK: a credential-bearing SSE error event could pass, plus 9 Majors;
  - BLOCK: C1, M1 and M4 reopened by new counterexamples;
  - SHIP: the reviewer replayed the counterexamples against a fresh build.
- **Evidence:** [t004-gateway-2026-10-04.json](review/t004-gateway-2026-10-04.json).

- **Request pin.** OMP 18.6.0's request profile was pinned before code by running
  actual OMP (the `dist/cli.js` bundle, not the 18.1.11 TypeScript sources beside it)
  against a credential-free capture endpoint; see adapter-economics § Pinned OMP 18.6.0
  request profile. Sanitized fixtures are in `tests/fixtures/gateway/`.
- **Gates on the integrated main tree** (with 008):
  - source manifest `87aea7ff…07d3`;
  - `cargo fmt --check` and `cargo clippy --all-targets -D warnings` pass;
  - the full suite: **352 passed, 0 failed, 1 ignored**; all recovery scenarios passed;
  - Rust 1.90.0 `check` and `clippy -D warnings` pass;
  - `tests/gateway.rs` is 42/42 on three consecutive runs.
- **Release binary.** `53d43b0c…2662` contains none of the test-hook strings
  (`FOUNDRY_GATEWAY_TEST`, `ctxfoundry-fault`, `FOUNDRY_TEST_FAULT`).
- **Real OMP 18.6.0 against a loopback fake upstream** (test build, throwaway `HOME`,
  synthetic key canary):
  - A tool call and its continuation reconciled exactly with `usage import`: 2 attempts,
    2100/1000/38.
  - OMP stopped reading 4–158 ms after the trailing usage chunk: `complete`,
    `client_closed`, counts kept.
  - A finish without usage was aborted by OMP at 2,506–2,684 ms (the pinned 2,500 ms
    grace) and recorded `unknown`.
  - Empty completions produced 12 recorded attempts while OMP persisted none.
  - Upstream HTTP 500 produced 35 attempts in 90 s, with the canary never forwarded.
  - With the slot held by another client, OMP took 10 `gateway_busy` refusals,
    counted in `refused`, with no in-transport retries, then succeeded.
  - Launcher refusals were `profile_exists`, `invalid_argument` (a `default` profile;
    `--hook`), `profile_invalid`, `token_missing` and `gateway_unavailable`. OMP never
    started, nothing reached the upstream, and the global `models.yml` was never
    created.
- **Live runs against Z.ai.** Owner-authorized for 3 runs within 15 minutes; 2 were used
  and finished in 77 s.
  - Run 1, a tool turn, matched `usage import` exactly: 2 attempts and 2 assistant
    messages, input 13,263, cached 6,592, output 57.
  - Run 2, SIGTERM 4 s in, produced one `unknown`/`client_closed` attempt with no
    counts. OMP recorded zeros, its default for absent usage. That attempt may still
    have been billed.
  - While live, `run_dir` was 0700, the token 0600, `gateway.json` 0600 and
    `models.yml` 0600. Afterwards the token and `models.yml` were removed, while
    receipts and sessions remained.
  - A key scan (`grep -F -f` over 35 text files) found 0 matches.
- **Limits of this evidence.** Meter mode only. Receipts are in-memory with an optional
  bounded log, so whole-run coverage stays unverified across crashes. A terminal-
  generated SIGINT is documented, not tested. No savings claim follows from
  forwarding.

## 008 explicit memory — T001 and T002, 2026-10-04

Locally implemented and verified, **pushed 2026-10-05, unreleased**.
- **Authors:** Z.ai GLM-5.3 (implementer), plus captain integration fixes.
- **Review:** cross-lab by OpenAI GPT-6.1 Sol in three rounds:
  - REVISE: 8 Majors, including pending-key migration loss and enabled context reading
    two snapshots;
  - REVISE: 5 Majors (full link validation, memory-ID check in the combined read, a
    swallowed multi-root error, remaining source/memory count consumers, public search
    bounds), and an export byte-cut test that had to actually cut;
  - SHIP.

- **Gates on the integrated main tree.**
  - Source manifest `f6151644…5cdf`, the SHA-256 of sorted file hashes over `src`,
    `tests` (minus the frozen economics corpus), `Cargo.toml` and `Cargo.lock`.
  - `cargo fmt --check` and `cargo clippy --all-targets -D warnings` pass.
  - The full suite: **287 passed, 0 failed, 1 ignored** (the existing payload report),
    and all recovery scenarios passed.
  - Rust 1.90.0 `check` and `clippy -D warnings` pass.
- **`tests/memory.rs` (29 tests).** It covers every T001/T002 Verification bullet and
  the review regressions:
  - put/update/conflict/idempotent revisions; refusals consume no revision;
  - wrong workspace on every op;
  - stale/missing links, with retrieve-grade validation inside the write transaction;
  - source-only search/context exclusion; compact `mem:` lines cut at a UTF-8 boundary
    within the budget;
  - `--no-memory`;
  - a broken lexical index with get/export still working;
  - `corrupt_memory` isolation, including an ID mismatch in the combined read;
  - the v2→v3 upgrade, including a `a`/`source:a`/`source:source:a` prefix chain and
    populated graph/scan rows, interrupted before and after commit;
  - a `memory:x` path versus record `x`;
  - re-index and repair;
  - export paging, including a 4 MiB byte cut with cursor resume;
  - forget/recreate without revision reuse; `revision_exhausted`;
  - content-free reports; training exclusion;
  - multi-root selection and corruption propagation;
  - namespace-specific pending counts; CLI/MCP parity.
- **Catalog.** Measured on the integrated debug binary with `tiktoken-rs` 0.12.1
  `o200k_base`: `tools/list` is **793** tokens for six tools and **669** with
  `--no-memory`. The ceiling is 800.
- **Compatibility.**
  - Store schema 3: `upgrade-store --to 3` from v1 or v2 in one transaction; `--to 2`
    and older readers refuse.
  - Rollback means restoring a pre-upgrade store copy and the previous binary.
  - The search schema is unchanged, so no forced repair.
- **Integration notes.** The implementer repeatedly replaced existing lines instead of
  inserting. The captain restored `pub mod laya`, `use crate::graph`, the
  `RETRIEVE_BEFORE_FINAL_READ` fault point, `scan_status` initialization,
  `open_table(PENDING)` and a displaced doc comment. The captain also routed
  `#[tool_handler]` through the instance router, so `--no-memory` really removes the
  tool. Every removed line in the final diff was audited by the captain and the
  reviewer.

## Token-economics remainder — 003 T005 and 007 T001, 2026-10-04

Locally implemented and verified; committed on main as `bd1d890` (001 T005 amendment),
`5e99ffd` (003 T005) and `cc402e0` (007 T001), **pushed 2026-10-05, unreleased**. Authors: Z.ai
`glm-5.3` (usage import and economics tests, multi-root, OMP hook) and Anthropic Claude
Opus 5.5 (captain: 001 T005 leading-run amendment and integration). Reviewer: OpenAI
`gpt-6.1-sol:xhigh`, one reviewer per slice on a different lab from both authors; not an
independent quorum.

| Slice | First review | Fixes | Final |
| --- | --- | --- | --- |
| 001 T005 leading-run amendment (`src/syntax.rs`, `src/store.rs`) | REVISE: ASCII-only whitespace detached docs (NBSP, form feed); `/**/` broke a JSDoc/Javadoc run; inner-doc test masked | One Unicode whitespace predicate; `/**/` kept; boundary test, which failed on the old predicates | SHIP (delta) |
| 003 T005 usage import and economics tests | REVISE: unchecked per-record OMP input sum; full conflicting id fed a quadratic bounded-error renderer; whole-unit check compared a self-selected slice; fixture gaps; corpus identity unguarded | Checked arithmetic; bounded id in the message; hand-frozen unit spans; extended fixtures; nine frozen corpus digests | SHIP (delta) |
| 007 T001 multi-root | REVISE: unknown alias ignored on a single-root owner; request order replaced admission order; empty-serving refusal listed no coverage; truncation could drop coverage pairs; outline slots backfilled | Alias validation before dispatch; admission-order selection; listed-root refusal; bounded compact pairs; key-counted outline slots | SHIP (delta) |
| OMP first-call hook (team-kit) | REVISE in three rounds: per-command flag arity, relative `input.cwd`, stdio without args, `/` root, `rg` encoding clusters, explicit stdio with a stray `url`, empty `command` | Each fixed with a behavioral case; 21 cases | SHIP (delta) |

Integration found one regression before review: with leading runs inside units, a
tier-1 locator showed `#[derive(…)]` instead of `pub struct BudgetConfig`. The locator
now uses the unit's head (its own node or wrapper start), separate from the outline's
signature start. Units with leading runs need search schema `"3"`; older stores report
`repair_required` until `repair-index`.

**Gates** on the integrated tree (32-file manifest SHA-256 `b885606c…161c`), rustc
1.97.1 and Rust 1.90.0:

| Check | Exit |
| --- | ---: |
| `cargo fmt --check` | 0 |
| Locked all-targets clippy, warnings denied | 0 |
| Locked full test run, no fail-fast: 258 passed, 1 ignored (the payload report) | 0 |
| Rust 1.90.0 check and clippy (warnings denied) | 0 |
| `cargo test --locked --test economics -- --ignored --nocapture payload_report` | 0 |
| `bun ~/.omp/team-kit/bin/check-foundry.ts`: 21 cases | 0 |

**Economics on the frozen corpus** (exact `o200k_base` text-block tokens; one
observation each): 12/12 expected units (six identifiers as search hit #1 retrieved
whole within 2048 tokens; six questions surface their unit within `context` 2048).
`tools/list` is 661 tokens (598 before 007 added `root`/`roots`; ceiling 800).

| Query | v2 search (10) | v1 search | Ratio | v2 search + retrieve | v1 pair | Ratio | v2 context 2048 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `reconstruct_verified` | 253 | 2651 | 9.5% | 854 | 3399 | 25.1% | 1555 |
| `pack_ordered` | 167 | 2056 | 8.1% | 627 | 2806 | 22.3% | 1352 |
| `BudgetConfig` | 440 | 4640 | 9.5% | 570 | 5434 | 10.5% | 2032 |
| `native_discovery_block` | 119 | 1337 | 8.9% | 392 | 2123 | 18.5% | 1356 |
| `fit_prefix` | 192 | 2079 | 9.2% | 865 | 2880 | 30.0% | 1309 |
| `take_context_id` | 133 | 1382 | 9.6% | 261 | 2162 | 12.1% | 1020 |
| `stale handle rejected on retrieve` | 601 | 6483 | — | 1294 | — | — | 2030 |
| `repair index quarantine marker` | 592 | 6069 | — | 1253 | — | — | 2011 |
| `inbound frame byte limit` | 639 | 6478 | — | 739 | — | — | 1979 |
| `session allowance refund` | 606 | 5934 | — | 731 | — | — | 1980 |
| `receipt deduplication conflict` | 602 | 6803 | — | 848 | — | — | 2012 |
| `sweep unseen sources after complete scan` | 607 | 6328 | — | 685 | — | — | 2019 |

The search (≤20%) and search-plus-unit (≤35%) targets hold for all six identifier
queries. The ≤40-token header bound is asserted on the v2 success fixtures of
`tests/response.rs` (`assert_v2_success`) and `tests/mcp.rs` (`assert_v2_delivery`);
this report does not measure it separately. v2 `context` ranges 1020–2032
tokens against v1's 1439–1853 at the same 2048 request; there is no context-size
target.

**Latency of the two T006 risks** is recorded under the 001 T005/T006 acceptance below.

**Real hosts (003 T005 runbook)**, counters only in
[`real-host-te-2026-10-04.json`](review/real-host-te-2026-10-04.json): release binary
SHA-256 `e32204fb…6cee7` built from the gated tree; OMP 18.6.0 with `zai/glm-5.3`;
Codex CLI 0.159.2 with `gpt-6.1-sol`. Eight runs in 358.7 s of the 2400 s cap, no
timeout or harness failure, plus one owner-authorized rerun of run 8 (run 9, 39.4 s;
398.1 s in total). Label: **bundled Foundry adoption (v2 + instructions +
optional hook), n=1 per task and arm**, not an isolated v2 effect or a general savings
result. Every OMP import is `complete:false` because the session usage records omit a
reasoning-token category; totals are the reported input-plus-output sums, and the
reasoning breakdown is unknown.

| Run | Oracle | Wall s | Provider tokens (input / cached / output) | Tool calls | Foundry / host payload estimate |
| --- | --- | ---: | --- | --- | --- |
| 1 hook probe, F | pass: step 1 refused with the hook reason; repeat, regex and `/tmp` greps ran; Foundry search via `xd://` then its grep ran | 69.2 | 80,013 (76,740 / 61,440 / 3,273) | grep 5, foundry.search 1, read 1 | 368 / 738 |
| 2 A1-H | pass | 31.8 | 114,208 (113,129 / 83,200 / 1,079) | grep 3, bash 1, read 1, context 1 | 0 / 16,075 |
| 3 A1-F | pass | 46.3 | 90,464 (89,081 / 84,736 / 1,383) | foundry.search 2, foundry.retrieve 3 | 2,668 / 0 |
| 4 A2-H | pass | 27.4 | 79,360 (78,282 / 71,936 / 1,078) | grep 2, read 2 | 0 / 5,752 |
| 5 A2-F | pass | 70.9 | 97,623 (94,323 / 87,424 / 3,300) | foundry.search 2, foundry.retrieve 4 | 4,686 / 0 |
| 6 A3-H | pass: check exit 1 → 0 | 25.6 | 66,163 (65,655 / 53,312 / 508) | bash 2, read 2, edit 1 | 0 / 883 |
| 7 A3-F | pass: check 1 → 0; pre-edit handle now `stale_handle` | 68.5 | 163,876 (162,335 / 158,016 / 1,541) | foundry.search 2, foundry.retrieve 4, foundry.index 1, read 2, bash 1, edit 1 | 1,082 / 736 |
| 8 Codex T003 discovery | **miss**: first discovery was shell `rg`; the answer was correct (`src/records.rs:5`, callers `src/main.rs:13`, `:33`) | 19.0 | not imported (`--ephemeral`); stdout turn usage 50,729 (50,493 / 44,928 / 236) | shell 2 (`rg`, `nl`) | — |
| 9 Codex rerun with instructions | pass: first discovery Foundry `search`; the answer cites `src/records.rs:5` and both callers | 39.4 | not imported (`--ephemeral`); stdout turn usage 105,511 (104,919 / 87,552 / 592) | context-foundry search 2, retrieve 5 | — |

Findings, reported without prompt or threshold tuning:

- A1-F used 21% fewer provider tokens than A1-H; A2-F used 23% more and A3-F 2.5×
  more (A3-F's prompt adds the index refresh and citation, a recorded asymmetry
  against F). Total imported tool-result payload estimates fell from 16,075 (H) to
  2,668 (F) for A1 and from 5,752 (H) to 4,686 (F) for A2. These opened a follow-up
  decision; the owner chose a root-cause analysis of the retained sessions (below).
- **Cost root cause** (retained transcripts, arithmetic on the recorded turns; no new
  runs or tuning). OMP re-sends the fixed prefix and task prompt plus accumulated
  history on each turn. The fixed prefix plus initial user content is about
  12.4–13.0k tokens per request; complete request input also includes replayed
  history. F's fixed prefix is 11,672 tokens (11,671 in A3-F), versus 11,130 in H: a
  bundled-setup difference of 542 tokens per request (541 in A3-F).
  A1: one unscoped whole-repository grep in A1-H returned 51 KB, re-sent on three
  later turns; H's accumulated history outweighs F's extra turn and catalog. (Turn-1
  provider-cache warmth differed between arms but cannot move these totals, which
  count cached tokens one for one.) A2: the extra F turn came from a failed
  `retrieve`; removing that round trip leaves F 2.5% above H. A3: 11 turns against 5;
  removing the runbook's refresh and citation turns would save about 47.7k input
  tokens and, on top of that, the turn spent on a failed retrieve about 14.6k (each
  estimate is arithmetic on the recorded run and they do not add exactly); a failed
  retrieve inside another turn re-sent about 1.9k; F also fell back to two plain-read
  turns. Three of eleven in-session retrieves failed: twice the agent sent `lines` as
  a JSON array (`[1630, 1800]`), which OMP rejected against the schema's `a-b` string
  pattern before calling Foundry, echoing the arguments in about 870 bytes; once it
  copied a handle's byte range (`#650-1459`) as line numbers (`invalid_range`).
  Accepting the array form and separating byte ranges from line numbers in handles
  are Foundry-side options for the owner. Additional turns and replayed history are
  observed costs; avoiding refusals and redundant calls is one option, but these n=1
  runs do not establish that fewer turns are necessary for further savings.
- Run 8: no MCP call and no MCP error item. A later local probe of the same release
  owner on the run's store (no model) listed all five tools. Unlike the passing
  2026-10-01 Codex run, which shared the OMP F arm's copy and its Foundry `AGENTS.md`,
  this copy carried no project instructions (the runbook then named none). The owner
  amended the runbook to write the printed Codex instructions into the copy and
  authorized one rerun (run 9): Codex then called Foundry `search` first and answered
  correctly, using about twice run 8's provider tokens (two searches and five
  retrieves against two shell calls on this two-file fixture).
- The A/B F arms (runs 3, 5 and 7) first called Foundry search through the `xd://`
  device and ran no grep or `rg`, so the hook blocked nothing in runs 2–7; run 1 shows
  one refusal followed by an allowed identical repeat.
- A user-level custom Prakarana `context` tool stays loaded under `--no-extensions` in
  both arms; A1-H's one `context` call returned a no-daemon error.

Deviations: OMP 18.6.0 instead of the runbook's 18.4.10 (run 1 confirmed the hook
matchers); the hook loaded from the team-kit source path, not an installed copy,
because `./install.sh --sync` would also install 42 unrelated uncommitted team-kit
edits; the binary and the A3 checks used private `CARGO_TARGET_DIR`s.

Committed after the runs (owner decision): the team-kit hook as team-kit `e65f8cf`, only
its own files (the 42 unrelated kit edits untouched), installed by copying the one
extension file into `~/.omp/agent/extensions` (inactive until `TEAM_KIT_FOUNDRY_ROUTE=1`;
not in the kit's install manifest). Nothing is pushed or released. Residual: the
long-id usage regression test measures elapsed time but has no watchdog, so a
reintroduced quadratic renderer would hang it rather than fail it (reviewer minor,
accepted).

This section and the real-host record were reviewed by the same OpenAI
`gpt-6.1-sol:xhigh` lane: REVISE (a private hook digest and home path in the record,
Codex stdout usage reported as absent, an unsupported reasoning-exclusion claim, three
wording scopes), then SHIP on the corrections.

## 001 T005/T006 local acceptance — 2026-10-04

001 T005 (syntax units, search index v2, two-tier ranking, `path` filter) and T006
(outlines, forms ladder, candidate seam, retrieve `view`; `following_chunks` removed)
are locally implemented and **unreleased**. Each was accepted at the r3 **SHIP** of the
same existing reviewer, `T004CutoverReview` (OpenAI `gpt-6.1-sol:xhigh`), cross-lab
to the Anthropic author; one reviewer, not an independent quorum. Both were committed
with T004 in `5edf32c`, whose 19 code files matched the accepted T006 hashes. Receipts,
review history and the gate script are retained outside Git in the owner's handoff
folder (`handoffs/context-foundry-001/artifacts/`); this record restates them and did
not rerun the gates.

| r3 gate (each task) | Exit |
| --- | ---: |
| `cargo fmt --check` | 0 |
| Locked all-targets clippy, warnings denied (rustc 1.97.1) | 0 |
| Locked full test run, no fail-fast | 0 |
| Rust 1.90.0 `cargo check --all-targets` (toolchain first on `PATH`) | 0 |
| Rust 1.90.0 clippy, warnings denied | 0 |
| Freeze recheck: 19/19 files unchanged during the run | 0 |

T006 test counts: lib 11, cli 16, core 36, integrity 15, laya 3, mcp 73, recovery all
passed (custom harness), repair 9, response 15, scan_faults 15, syntax 28; 0 warnings.
Review rounds: T005 r1 REVISE (C/C++ parenthesized declarator names; native recursion
and a quadratic parent walk), r2 REVISE (quadratic qualified-name content), r3 SHIP;
T006 r1 REVISE (mandatory member signatures, outline refusal hints, graph window,
Python span ends, skip-versus-stop), r2 REVISE (C++ template wrapper on members), r3
SHIP. Each fix had RED captured on the frozen pre-fix tree before GREEN. Owner decisions
taken in review — the 256-byte qualified-name tail, metadata-only upgrade with explicit
`repair-index`, and the two outline refusal policies — are written into the
[v2 contract](../specs/001-source-state-recovery/contracts/context-v2.md). No RED was
captured for T005's path-filter tests or limit-cut change.

**Catalog re-measured, 2026-10-04.** At `c430997` (clean source; debug `foundry`
SHA-256 `2f455d16…`) a throwaway stdio probe, mirroring `tests/mcp.rs` `RawStdio`,
counted the raw `tools/list` result: 5 tools, 2401 bytes, **598** o200k tokens (568 at
T004; v1 711; ceiling 800). T005/T006 grew the `search` (`path`) and `retrieve`
(`view`) schemas. The probe was outside the repository and was removed.

**Latency of the two T006 risks, measured 2026-10-04.** Release `foundry` built at
`c430997` (SHA-256 `7e85efff…`); CLI wall time including process start and store
open, median of 3 runs, local macOS arm64; disposable fixtures, removed afterwards.

| Corpus | context `tokens` 2048 | context `tokens` 32768 | search |
| --- | ---: | ---: | ---: |
| Real: `git archive` of prakarana `ae159e85` (5,481 sources; index 36.5 s), four queries (one with no hits) | 0.08–0.27 s | 0.08–0.35 s | 0.08–0.12 s |
| Synthetic worst case: 32 Rust files of about 1 MiB, each holding one matching unit | 4.19 s | 4.24 s | 0.22 s |

Increasing the context budget from 2048 to 32768 changed the reported CLI wall time by
at most 0.1 s. The synthetic 32-file corpus took about 4.2 s for context versus 0.22 s
for search. Context additionally constructs syntax-based forms and ladder-packs them;
this probe did not time parsing, form rendering and packing separately, so their
individual costs are not established. The approximately 4.2 s CLI wall time is about
84% of the MCP owner's 5,000 ms read-deadline duration, but is not an MCP latency
measurement. 007 plans to share that deadline sequentially across roots, so these
results identify a deadline risk, not a measured multi-root failure. Whether to bound
or cache query-time parsing before release remains an owner decision; the measured
real corpus's CLI times were substantially below five seconds.

## 001 T004 local closure — final4

001 T004 is locally implemented and verified, **unreleased**. The owner reports that
all six final4 gates exited 0 on the unchanged source manifest retained at
`/private/tmp/cf-t004-gates-final/manifest.txt`; logs are in that directory. The same
existing `T004CutoverReview` (OpenAI `gpt-6.1-sol:xhigh`) returned **SHIP** after its
reported finding/delta closures. The author lane is Anthropic. This is the existing
in-session cross-lab review, not a fresh independent review. This docs-only closure
does not rerun or independently re-certify the six checks.

| Reported final4 gate | Exit |
| --- | ---: |
| `cargo fmt --check` | 0 |
| Locked all-targets clippy, warnings denied | 0 |
| Locked full test run, no fail-fast | 0 |
| Rust 1.90.0 check | 0 |
| Rust 1.90.0 clippy | 0 |
| CLI smoke | 0 |

The retained MSRV version log names rustc/cargo 1.90.0 and clippy 0.1.90. The retained
smoke shows index/search and `retrieve --lines 1-3` succeeding with v2 text and exact
`parse_record` bytes. Its negative `--lines 4-6` returns `invalid_range`, exit 2,
as invalid input under the existing CLI contract; that is not a changed exit rule
or a failed smoke gate.

| Measurement | T004 v2 | v1 comparison | Boundary and scope |
| --- | ---: | ---: | --- |
| Handle on `examples/workspace` | 33 o200k tokens; 68 bytes | 94 tokens; 207 bytes | Isolated v2 handle versus v1 JSON handle; fixture-specific, superseding the 29/85 estimate |
| Five-tool `tools/list` | 568 o200k tokens (598 after T005/T006, above) | Historical baseline: 711 | Serialized catalog; the 800-token ceiling is met |

These are handle/catalog measurements, not the complete twelve-query payload
comparison, provider usage, a universal per-handle cost or host-session savings.
T005, T006 and their measurements are recorded above; the 003 T005 remainder and
007 T001 are recorded in the 2026-10-04 token-economics section at the top.

**Reviewed ambiguity limitation:** a valid path embedding a complete handle suffix
and valid item tail can give an item line two complete readings. The test parser
`testkit::parse_v2` refuses both without attributing either handle; production
rendering and direct retrieve are unaffected. No universal item-line round trip is
claimed. The required 4096-byte special-path CLI/MCP/core cases passed in final4.
Zero-allowance `budget_exhausted` uses an outcome-free sufficient budget, not a
proved minimal budget, as the existing v2 refusal-floor contract allows.

## Token-economics v1 payload baseline — 2026-10-03

This is historical v1-era evidence. Its frozen queries and byte ranges describe
`6bb81e6`, including the subsequently deleted `take_context_id`; they are not
claims about current symbol availability.

Phase 0 capture for the approved token-economics tranche (001 T004–T006, 003 T005,
007 T001), taken on baseline `6bb81e6` before any code change, on the local macOS
arm64 workstation. One run of a temporary `#[ignore]` test appended to `tests/mcp.rs`,
reusing that suite's real rmcp client helpers:
`cargo test --locked --test mcp -- --ignored --nocapture v1_payload_report` → 1 passed,
65 filtered out, 2.93 s. The test was then removed byte-exactly (`tests/mcp.rs` is back
to its 4686 baseline lines at capture time, with no diff against `6bb81e6` then).
One observation per cell; no repetition or variance claim.

- **Corpus:** the frozen nine-file economics corpus, exact bytes at `6bb81e6` of
  `src/{store,response,mcp,ingest,bootstrap,receipts,config,error}.rs` and
  `docs/architecture.md`, indexed as one workspace from a temporary copy.
- **Boundary:** exact `o200k_base` `encode_ordinary` count of the MCP text-block
  content Foundry emits, which the
  [v2 contract](../specs/001-source-state-recovery/contracts/context-v2.md) counts.
  It is the delivered payload, not host or provider cost: hosts forward it unchanged
  only below their own size limits.
  v1 itself budgets the whole serialized `CallToolResult`, including a second JSON
  escaping, so a 2048-token v1 request delivers fewer text-block tokens than 2048.
- **Requests:** `search` with `limit` 10; `retrieve` of search hit #1's handle with
  `tokens` 2048; `context` with `tokens` 2048. No request returned an error.
- **Catalog:** serialized `tools/list` catalog, 5 tools: 3044 bytes, 711 tokens.

| Query | Kind | Search hits | Search tokens | Retrieve hit #1 tokens | Context tokens | Hit #1 v1 block (bytes) |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| `reconstruct_verified` | identifier | 4 | 2651 | 748 | 1457 | `src/store.rs` 52648–54672 |
| `pack_ordered` | identifier | 3 | 2056 | 750 | 1498 | `src/response.rs` 8077–10069 |
| `BudgetConfig` | identifier | 7 | 4640 | 794 | 1567 | `src/config.rs` 0–2028 |
| `native_discovery_block` | identifier | 2 | 1337 | 786 | 1439 | `src/bootstrap.rs` 20047–22048 |
| `fit_prefix` | identifier | 3 | 2079 | 801 | 1808 | `src/response.rs` 14136–16142 |
| `take_context_id` | identifier | 2 | 1382 | 780 | 1483 | `src/mcp.rs` 38555–40586 |
| `stale handle rejected on retrieve` | question | 10 | 6483 | 748 | 1478 | `src/store.rs` 58701–60738 |
| `repair index quarantine marker` | question | 10 | 6069 | 689 | 1853 | `src/store.rs` 64784–66820 |
| `inbound frame byte limit` | question | 10 | 6478 | 796 | 1836 | `src/mcp.rs` 4061–6076 |
| `session allowance refund` | question | 10 | 5934 | 732 | 1784 | `src/mcp.rs` 12188–14200 |
| `receipt deduplication conflict` | question | 10 | 6803 | 767 | 1460 | `src/receipts.rs` 14222–16240 |
| `sweep unseen sources after complete scan` | question | 10 | 6328 | 800 | 1445 | `src/store.rs` 34366–36411 |

Byte ranges are half-open. Every hit #1 is one v1 storage block of at most 2048 bytes,
whatever the size of the matched code; this probe did not check whether that block
holds the expected definition. These rows are the comparison basis for the targets in
the [adapter economics contract](../specs/003-agent-retrieval-context/contracts/adapter-economics.md):
v2 search at most 20% of the identifier rows' search tokens, v2 search plus unit
retrieve at most 35% of search plus hit-#1 retrieve, a context header of at most 40
tokens against v1's 199-token minimum envelope (2026-10-01 allowance run below),
12/12 expected units and `tools/list` at most 800 tokens. Full-tranche payload/host
comparisons await the remaining tasks; T004's handle/catalog measurements are
recorded above. The historical v1 table remains unchanged.

## 001 + 003 first implementation — 2026-10-01

This section records the historical v1 wire and counting boundary; its envelope
fields and measurements are not descriptions of current T004 output.

Owner-approved scope: 001 T001–T003 and 003 T001–T003, including the optional shared
MCP owner for concurrent OMP/Codex sessions. Verified locally on macOS 26.5 arm64
(rustc 1.97.1) and in a Rust 1.90.0 Linux aarch64 container. Not published, signed or
packaged; no hosted CI run. No predecessor store, service or global host configuration
was touched; real-host runs used temporary public fixtures and fixture-scoped config.

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Pass |
| `cargo clippy --locked --all-targets -- -D warnings` | Pass |
| `cargo build --locked --all-targets` | Pass, 0 warnings |
| `cargo test --locked --no-fail-fast` | Pass: 157 tests (unit 10, cli 13, core 20, integrity 15, laya_protocol 3, mcp 65, repair 8, response 8, scan_faults 15) plus 12 child-process recovery scenarios |
| `cargo build --locked --release` | Pass; SHA-256 `c36d0fb3975168b115309946954f8f673dc8252e2682420fae4865373c62c996`; 0 fault-hook strings |
| Rust 1.90.0, Linux aarch64 (`rust:1.90-slim-bookworm`) | `cargo check --all-targets --locked` pass; core 20, integrity 15, recovery, repair 8 and scan_faults 15 pass, including the non-UTF-8 filename branch APFS cannot create |
| README commands on the release binary | Pass, including `store_not_found` on a read before `index` |

Fault and recovery tests use named fault points behind the non-default `test-faults`
feature (enabled for tests by a self dev-dependency) and a separate `foundry-faults`
test binary. The shipped `foundry` binary has no arming path in any build; release
builds contain no fault-point strings. `--all-targets` debug builds contain four inert
fault-point names because Cargo unifies the test feature there.

**Real hosts (003 T003),** shared HTTP owner, fixture `tests/fixtures/agent-task`, host
instructions printed by `connect`. The host runs below used release `8968e70b…`; the
only later v1-era change added the counted `budget_limited_by` JSON-envelope field
and limiter-named refusals, rechecked by the mcp suite and the allowance run on `c36d0fb3…`:

- OMP 18.4.9 (model `zai/glm-5.3`), five runs: every first eligible discovery call was
  Foundry `search`, with no grep or ripgrep before it; the agent edited `parse_record`,
  the checker passed 4/4, it re-indexed through MCP and the pre-edit handle returned
  `stale_handle` (revision 6). An exact byte-pattern request used host `grep`
  (permitted). With the owner stopped, OMP's own diagnostic named `failed to connect`
  and the agent fell back to host tools. In both fallback cases the agent's reply did
  not restate the reason. Instruction-based preference only; no enforced routing hook.
- Codex CLI 0.159.2 (`gpt-6.1-sol`): native Streamable HTTP attachment (negotiated
  2025-06-18), tool listing, `search` and `retrieve` succeed. Alone, the discovery task
  listed the directory, then used Foundry `search`/`retrieve` and cited
  `src/records.rs:5`. Codex requires MCP tool approval by default; without the printed
  approval configuration non-interactive runs hang or refuse every call.
- Concurrent, same shared owner and fixture store: OMP performed the edit task while
  Codex answered a read-only discovery task. Their Foundry calls interleaved (OMP
  `search` 24.4 s, Codex `search` 31.1 s, OMP `retrieve` 35.5 s, Codex `retrieve` 44.9 s,
  OMP `index` 88.4 s); both exited 0, the owner exited 0 and the checker passed. Codex
  used only Foundry tools and cited `src/records.rs:5` and both callers. Three earlier
  concurrent attempts stalled Codex because the captain's harness never drained
  Codex's verbose stderr pipe; with stderr sent to a file the stall did not recur.
  Product code was unchanged by that harness fix. Evidence:
  [real-host record](review/real-host-t003-2026-10-01.json).
- Delivery allowance on a real host (release `c36d0fb3…`, OMP, owner launched from the
  printed `--budget` with `session_context_tokens: 500`): the first context request
  for 2000 tokens delivered with the v1 JSON fields `requested_budget: 500` and
  `budget_limited_by: session_allowance`; the next two were refused with
  `budget_exhausted` ("cannot fit within 73 tokens (limited by session_allowance);
  minimum 199 tokens"), and the unchanged 73 shows refusals are not charged.
  Host-request mode stays `budget_scope_unsupported`; neither host offers the hooks.

**Reviews.** Recorded session events, not self-reports: source review round 1 and
adapter review round 1 ran on OpenAI `gpt-6.1-sol`, cross-lab to the Z.ai GLM-5.3 and
Anthropic Sonnet 5.5 authors; source round 2 fell back to Sonnet 5.5, same lab as that
revision, so a final OpenAI pass (`codex exec`, `gpt-6.1-sol`) covered the integrated
tree. Its three new defects were fixed; it could not bind HTTP sockets in its sandbox,
so HTTP behavior rests on the native test run above.

**Accepted limitations.** Names are enumerated by path while bytes are read through
the held root: a same-user process that swaps and restores a directory during one scan
can retire records of still-present files until the next scan (001 records the
re-entry). The 16-handler cap is proven at its admission owners, not with 16 truly
stalled handlers (one engine slot makes that unreachable). Cancellation-registration
ordering is fixed by construction without an interleaving test. Several MCP tests
observe timing and fail closed on an unusually loaded machine. One continuation test
was made full-suite-safe after a single unreproduced load failure.

## First prototype slice — 2026-09-28

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

## Artifact, accounting and card reuse — 2026-10-09

Experimental E1 continuation: verified artifact receipts, transactional cache totals and
exact per-card normalized-segment reuse. Linux sandbox: 165 library tests and 65 neural
integration tests passed; Clippy all targets passed. Receipt tests count artifact reads,
refuse replacement/in-place modification and descriptor mismatch; cache tests cover warm
runs, stale finalizers, duplicate keys, mixed retained rows and restart. Card tests compare
exact IDs/offsets/keys/spans with the ordinary tokenizer and bounded overflow fallback.
These fixtures establish work reuse and equivalence, not model speed, cost or task quality.
macOS supervised `/dev/fd` loading remains unexecuted here.

## Continuous semantic maintenance — 2026-10-09

Experimental E2 implements schema-9 source-transaction obligations, independent bounded
semantic/knowledge lanes, exact conditional acknowledgments, fair changed-path selection,
explicit continuous preparation and cheap publication observations. Fixtures run the
real redb, tokenizer, USearch and MCP/watch paths with a counted fake embedding provider.
They cover edit/revert/rename/delete, inference races, pause/restart, explicit startup,
queue overflow and late acknowledgments, hot-path rotation, schema upgrade, status
polling without census/model work or batch throttling, and progress during filesystem
traffic followed by convergence. Recovery snapshots now include both obligation tables.

Focused checks passed: 170 library, 65 neural, 61 neural retrieval/MCP, 18 brief and
19 task-context tests (one real-model lifecycle test remains ignored). The full
`cargo test --locked --all-targets --no-fail-fast` run passed **915 tests**, with two
intentional ignores, plus the custom crash-recovery runner. The 6,000-file watcher
fixture passed. Subsequent review checks reran all 170 library and 61 neural
retrieval tests, including explicit mode switching, and the five work-queue tests
with owner/lane fences. `cargo clippy --locked --all-targets -- -D warnings`,
`cargo fmt --all --check`, `cargo check --locked --no-default-features --lib` and the
Markdown link check passed. The real MCP catalog measured **996 o200k tokens**, inside
its unchanged 1,000-token limit. A CLI smoke check verified cheap status, explicit
`--audit` and refusal of `--semantic-maintain` without a profile. These tests establish work
selection and recovery; publication still walks prepared mappings and can rebuild the
full ANN. E3 vector/location deltas and E4 model-aware inputs remain pending. No
80,000-file throughput, real-provider token economics or task-quality result is claimed.

## ANN delta publication — 2026-10-09

Experimental 009 T008/E3 introduces generation v4: one immutable USearch base, one
bounded delta, independent active locations and a single atomic manifest switch.
The actual tokenizer/redb/USearch/MCP paths use counted fixture providers here.

Focused tests verify zero vector decodes/builds for location-only changes, unchanged
base/delta file identities, one-vector delta work, duplicate input suppression,
reactivation of reverted base keys, compaction thresholds, empty generations,
malformed membership refusal and bounded orphan reclamation. An 80-tombstone fixture
places retired vectors ahead of fresh base hits in the 64-candidate window; filtered
ANN selection still returns a full active window merged with the new delta hit.
Real child-process aborts before/after the manifest switch cover both delta publication
and compaction: reopen keeps a readable generation and pending obligations, then
publishes from committed cache without a provider. These establish application ordering,
not hardware power-loss immunity.

The driver fixture exercises three visible publication retries, serviceable foreground
reads during backoff, exhaustion without a second finalization retry cycle, durable
pending work and explicit recovery without more embedding. The real MCP maintenance
fixture verifies that an edit increments delta publications without replacing the base
ANN, then covers revert, rename, deletion, pause and restart.

Focused checks passed: nine index tests, the four child-process crash boundaries, all
62 neural library tests and the continuous-maintenance MCP case. Clippy all targets
and the build without default features passed. Across the full all-targets run,
**919 regular tests passed**, with two intentional ignores, including the 6,000-file
watcher fixture. A concurrent test rebuild replaced the self-executing recovery binary
mid-run, causing five harness restart failures (`ENOENT`); after builds finished, the
complete recovery runner passed **all 20 cases** separately. This was test orchestration,
not a product recovery failure. Final Clippy, formatting, build without default features
and Markdown link checks passed (47 files, 694 links, 153 heading anchors).

At this checkpoint, full mapping scans, cache-layout probes, referenced-file hashing,
complete location-map writes and resident-generation reload remained T008 costs; the
warm-reuse continuation below addresses three of these. E4 model-aware inputs remain
pending. This is no real-device speed, provider-token-cost or task-quality claim.


## Warm publication reuse — 2026-10-10

Experimental 009 T008 now keeps a disposable mapping view per Engine, refreshes paths
and input-key dependents recorded with committed source/partition/cache changes, and
shares bounded file-verification receipts and immutable resident USearch segments
between publication and serving. No durable format or schema changed.

The mapping fixtures use 260 admitted sources and count actual partition reads,
full-mapping pages and cache probes. Quiet replay does no such work; completing a
shared cached input refreshes its two paths without a source revision change. Source
edits/deletion/sweep remove stale locations, rejected commits leave the view clean,
and changes between pages remain pending after an incomplete result. Cancellation,
overflow, profile changes, purge and reopen rebuild safely; a subsequent warm pass
stays incremental. Explicit audit still walks authoritative rows.

The retrieval fixture uses the actual renderer, redb store, publisher, USearch loader
and query runtime with deterministic concept vectors. A warmed location-only change
reads one affected partition and performs zero ANN hashes/restores. A new delta
hashes/restores only that segment; explicit generation refresh and base reversion reuse
resident segments. Crossing the compaction threshold restores the new base once.
Held old query windows preserve their generation revision. Reopen invalidates owner
reuse, and profile/manifest changes are detected without an invalidation hint.
Same-size in-place mutation, replacement and symlink substitution refuse on refresh;
changed-during-read and cancellation cases cannot mint a receipt. Receipt-cache waiting
obeys control, retention stays at two segments, and explicit audit rehashes independently.

The full regression run passed **928 regular tests**, with two intentional ignores,
and **all 20 child-process recovery cases**, including delta/compaction publication.
The 6,000-file watcher fixture passed. Final Clippy (all targets, warnings denied),
formatting, the build without default features and Markdown link checks passed
(47 files, 696 links, 153 heading anchors).

These fixtures establish work selection, identity checks and recovery, not model quality,
provider-token savings or 80,000-file throughput. At this checkpoint, full location
serialization, membership/label metadata work and a restart baseline remained; the
following continuation addresses those repeated costs.

## Compact publication metadata — 2026-10-10

Experimental 009 T008 generation v5 now publishes immutable base membership and a
location base with one bounded replacement/tombstone overlay. Persistent ordered maps
share unchanged nodes across the mapping view, publication and serving. Metadata receipts
reuse decoded artifacts; a redb mapping revision lets an unchanged restart reuse verified
publication coverage without a database mapping scan. Schema remains 9.

Focused fixtures exercise real redb, publication, metadata validation and USearch:

- A location edit writes one overlay; serving reads/hashes/decodes only that new metadata
  file. An empty source then changes coverage without location comparisons or metadata
  writes/reads. A failed manifest switch retries from the warm view without a full scan.
- One ANN-delta addition plus 4,095 shifted inputs fits the 4,096-key location overlay.
  The next shift rebases locations while ANN base/delta and membership inodes remain
  identical. Failed rebase leaves the prior generation readable; retry converges, cleanup
  stays bounded, and edits/reverts operate against the new location base. The fixture
  manifest remains below 2 KiB with over 4,000 base keys.
- Cold clean publication returns Current with zero mapping pages, partition reads or
  cache-layout probes; it still reads and verifies the immutable artifacts. Same-source-
  revision partition changes and offline source edits invalidate this reuse and rebuild
  the baseline. Failed source commits leave the durable mapping revision unchanged.
- Duplicate membership/overlay keys, malformed units, redundant replacements, excess
  overlay entries and unbound active keys refuse even with matching content hashes.
  Same-size in-place edits and replacements invalidate metadata receipts; cancellation,
  changes during verification and byte limits cannot mint/reuse an invalid receipt.
- Older derived formats republish from cache without inference; generation v4 has an
  explicit migration fixture in addition to the existing v1 case. Process recovery adds
  before/after location-overlay manifest switches to the ANN delta/compaction scenarios.

Regression verification passed **934 regular tests** (two intentional measurement
ignores) and **all 20 child-process recovery cases**, including six semantic crash
boundaries across delta, ANN compaction and location publication. The 6,000-file watcher
fixture passed in the regression run (89.43 s for its suite). One prior-format file-count
assertion was updated from six to seven; the complete affected suite and remaining
regression suites then passed. Earlier successful suites were retained. Final all-target
Clippy with warnings denied, formatting, the build without default features and Markdown
link checks passed (47 files, 700 links, 154 heading anchors).

These fixtures establish bounded work, integrity and recovery. They do not measure
large-repository throughput, actual provider-token savings or task quality. Initial
bootstrap, cold artifact verification, changed-input restart baselines and compaction
remain corpus-sized. The first changed mapping after a clean restart still needs its
baseline; the revision shortcut applies to publication, not preparation's source walk.


## Model-aware body/parent inputs — 2026-10-10

Experimental 009 T009 adds strict versioned descriptor geometry and an explicit
body-v1 representation. Address profiles serialize identically without the new fields.
Both CLI and MCP preparation pack actual token lengths within independent per-input,
call-total and sequence bounds. The macOS adapter checks configured geometry against
the loaded GGUF and actual backend capacities; no default model window is raised.

Executed fixtures use the real tokenizer, parser, redb store, cache, ANN publication,
query runtime and MCP stdio consumer with deterministic provider vectors:

- Complete short units, long windows, Unicode/CRLF and unsupported syntax retain exact,
  non-overlapping source spans. Candidate re-encoding verifies total input limits;
  an impossible template refuses instead of dropping body characters.
- Body-only behavior reaches final packed context with the child's exact span. A body
  edit embeds one changed input; included parent-header edits invalidate their child
  context. Unchanged preparation, same-language rename, body/parent revert and return
  to the address recipe issue zero document calls. Old cache rows remain available.
- Explicit 8,192-token geometry passes documents longer than 2,048 tokens intact, while
  short inputs share batches beyond the old worst-case-size cap. Observed call tokens
  equal the preparation report. Continuous MCP maintenance catches a body edit and
  preserves actual call bounds alongside foreground context retrieval.
- Retained-card and forced-fallback paths produce identical keys and token IDs across
  multiple selection batches for both recipes. Fresh handoff avoids a second encoding;
  body fallback builds one lazy span plan, then re-encodes only selected windows.
- IPC round-trips preserve each sequence at 2,048, 8,192 and 32,768 total-token
  boundaries; excess input length, total tokens and sequence count refuse. Unknown
  geometry versions and invalid limits refuse; explicit geometry changes vector identity.

The full regression run passed **943 regular tests** (two intentional measurement
ignores) and **all 20 child-process recovery cases**. The 6,000-file watcher suite
passed in 87.46 s. After the final query-boundary assertion and test cleanup, all
77 neural unit tests and 70 retrieval integration tests passed again (one intentional
measurement ignore). Final all-target Clippy with warnings denied, formatting, the
build without default features and Markdown link checks passed (47 files, 710 links,
159 heading anchors). The explicit-geometry query fixture passes an input longer
than the legacy limit intact and refuses an oversize query before a provider call.

These establish representation, work selection, token admission and provenance, not
embedding quality or device throughput. Linux cannot build/run the selected macOS
Metal adapter. Pinned llama.cpp API/example/operator-test references were inspected;
real-model single/batched numerical parity for new geometries remains unexecuted. The
existing ignored real-worker parity fixture now reads the selected descriptor's input
limit. Parent/header text is a bounded child prefix; independent parent/file/area vectors,
contextual token-state late chunking and 010 T002 generated knowledge remain future work.


## Selective knowledge generation

Experimental 010 T002 now has explicit `knowledge prepare`, `status` and `purge-cache`
CLI operations with a strict configured HTTP provider/source scope. Schema 10 adds a
bounded completion cache and optional transitive area lineage to the existing brief
owner. Current inputs bypass body reconstruction, tokenization and provider calls;
selected source/area results remain derived, with exact original evidence recoverable.

`tests/knowledge.rs` uses a real local TCP/HTTP fixture and the actual CLI, tokenizer,
redb transactions, lexical index and context packer. All completions and reported
provider usage in these tests are **synthetic**. The executed boundaries cover:

- Partial two-call preparation followed by restart generates only the two missing
  ancestors. A complete unchanged replay sends zero requests, including when a fault
  would refuse source-body preparation. Independent embedding obligations stay pending.
- A source edit regenerates the changed child and two ancestors, reusing the other
  child. An immediate stale read excludes affected ancestors. Reverting source inputs
  reuses three cached results without a call, even when child prose remained identical
  across the source edit. Unselected source changes require no generation.
- Model/deployment changes rekey inputs; return to the prior configuration reuses cached
  completions. Recipe changes and primary-source reassignment cannot reuse the wrong
  node/area identity. If an evicted child input is regenerated with different prose,
  parents regenerate even after its original source input identity returns.
- Exact final source bytes accompany derived orientation. Oversized source/request
  inputs and run budgets refuse before a provider call; smaller explicit spans fit.
  Strict claim/citation/identifier checks refuse invalid model output before publication.
- Malformed replies, a wrong response model, incomplete/oversized output, HTTP 429 and
  redirects refuse without leaking response text. Persistent backoff suppresses retries
  after restart. Deadline cancellation closes the client request; absent usage stays
  unknown. Observed prompt/completion totals remain separate from o200k admission counts.
- Seventy-two retained outputs exercise bounded cache storage and transactional counts.
  Lowering the cap from 2 MiB to 1 MiB evicts history while all current briefs still replay
  with zero calls. Cache purge preserves current briefs. Forget removes a selected child,
  generated ancestors and cached copies in the same transaction, preserving user memory.
- A source edit during generation refuses the stale result. Faults and actual child-process
  aborts immediately before/after brief commit prove the cache and live brief publish
  atomically; restart performs four or three calls respectively. Schema-9 upgrade is
  explicit, preserves sources and remains wholly old after a pre-commit failure.

The full regression run plus the final affected-suite rerun passed **957 regular tests**
(two intentional measurement ignores) and **all 20 existing child-process recovery cases**.
The first full run found one old schema-9 upgrade target assertion; after updating it,
all 189 library tests, 18 brief tests and 14 knowledge tests passed. Other successful
regression suites were retained. The 6,000-file watcher suite passed in 84.37 seconds.
The new knowledge suite additionally runs two real process-abort boundaries internally.
Final all-target Clippy with warnings denied, formatting, the build without default
features and Markdown link checks passed (47 files, 719 links, 165 heading anchors).

No actual generation model was selected or called, and no task-quality, billing or
throughput improvement is inferred. HTTP cannot attest deployed weights or stop external
computation after cancellation. This CLI requires exclusive store ownership; continuous
MCP knowledge scheduling, shared foreground/device admission and an owned isolated local
generator remain open. Preparation scans a bounded selected plan, not a corpus-wide
reverse-dependency index. Knowledge acknowledgments describe that selected profile only.

## Continuous MCP knowledge maintenance

010 T002 now composes the existing generation pipeline inside the MCP owner. An
explicit profile enables generation controls and derived context; `maintain` or the
launch flag enables ongoing work. Source snapshot/acceptance steps use the shared
foreground-prioritized engine slot. JSON construction, tokenization and HTTP hold no
engine slot or source transaction. Schema 10 remains unchanged.

Executed checks use synthetic HTTP completions and embedding vectors, with real Rust
providers/transports, source storage, generation cache, native watching, lexical/dense
indexes, context packing and MCP SDK consumers:

- Real stdio watching/profile admission alone sends zero requests. Explicit maintenance
  generates four source/area nodes, an edit generates the changed child and two ancestors,
  and revert/restart reuse sends zero further requests. Context delivers labeled derived
  orientation together with the original implementation under the same token allowance.
- A gated HTTP call leaves foreground context and watcher source acceptance available.
  Editing a cited source during that call rejects its completion, then regenerates only
  the missing child and ancestors. Accepted transitive dependencies use the new full hash.
- Pause during a call accepts that one current result and admits no next call. Resume
  preserves it. An eight-call window stops a nine-node plan; rapid pause/resume cannot
  reset the allowance, and a fresh owner generates only the ninth missing result.
  Invalid output on the eighth call pauses explicitly even when a later node exhausts
  the window, preventing an unintended provider retry when a new window opens.
- A quiet driver waiting for a missing source performs no store steps across repeated
  notification timeouts. A prepare action wakes the waiting driver without a source edit.
  Publication preserves unrelated pending source indexing rather than draining the corpus.
- Shared admission excludes generation and embedding requests. A selected embedding batch
  waits for generation to release admission and then completes all 12 fixture inputs.
  Dispatched/waiting dense queries get priority; a late embedding call still blocks new
  generation. These exercise the actual runtime slot, not a mock scheduling counter.
- Shutdown releases the owner store in under two seconds while a synthetic external service
  still takes four seconds to answer; the uncommitted result is absent and work stays pending.
  This proves client cancellation and store lifecycle, not termination of external compute.
- A shared HTTP owner returns primary-root derived context alongside reference-root headers;
  excluding the primary refuses derived delivery. Both transports preserve exact source
  recovery. The HTTP teardown fixture waits for final SDK ownership to release the store
  after its completion notification, rather than assuming those two events are atomic.

The full regression plus final affected-suite reruns passed **967 regular tests**
(two intentional measurement ignores) and **all 20 child-process recovery cases**.
The full run passed 966 tests; the final library/knowledge rerun passed all 192 library
and 21 knowledge tests, including the added mixed provider-failure/window-exhaustion case.
The 6,000-file watcher suite passed in 127.20 seconds; this is a fixture result, not a
throughput claim. A build without default features exposed a watcher admission method
that was unnecessarily gated on embeddings; after removing that gate, the build and all
21 knowledge integration tests also passed under a `--no-default-features` invocation.
Correction recorded 2026-10-10: the self dev-dependency re-enables semantic in Cargo test,
so that invocation alone does not establish disabled-feature runtime coverage. Final all-target
Clippy with warnings denied, formatting and Markdown link checks passed (47 files,
721 links, 166 heading anchors). No schema migration or dependency changes were added.

The fixtures establish lifecycle, bounded admission, reuse, provenance and foreground
availability. They do not establish real-model quality, server CPU/GPU headroom, weights,
provider billing or large-repository throughput. Usage is reported at completed-pass
boundaries and is owner-local; a process restart is not a durable account-wide quota.
Actual isolated local generation and hardware resource enforcement remain open.

## Owned local generation and resource controls

010 T002 adds an explicit owned decoder to the existing CLI/MCP pipeline, with lazy
pinned artifacts, per-worker CPU/RAM/device limits and model unload/reuse. Schema 10
and existing HTTP cache identities are unchanged. This is experimental Linux CPU work
plus an unexecuted macOS App Sandbox/Metal path; it is not installed-package acceptance.

The native lifecycle fixtures use synthetic completions behind the **actual Rust worker
entry, supervisor, seccomp and OS resource limits**. They exercise:

- Denied access to an existing private file, socket creation, subprocess execution and
  `fcntl` ownership that could request asynchronous signals to another process,
  while model-descriptor reads and in-process threads remain available. The child sees
  only the explicit environment. CPU affinity, hard address-space/lifetime CPU limits
  and `no_new_privs` are inspected on the live process through MCP status.
- Actual address-space allocation refusal and CPU-time termination preserve pending
  source work and publish no output. Unsupported GPU/memory combinations refuse before
  reading an absent model, rather than falling back to unrestricted execution.
- MCP startup/profile admission alone launches nothing. A four-node pass uses one
  worker, idle unload removes its PID, and a changed child plus two ancestors reload
  from retained verified snapshots even after the original artifact paths are removed.
  A fresh owner with current briefs performs zero calls/launches without those paths.
- A blocked native call leaves foreground exact-source context available. Shutdown
  terminates/reaps the child before store release and leaves no uncommitted brief.
  Abrupt owner death stops computation; reopening recovers the retained obligation.
  The container may retain an orphan zombie under PID 1, so the death check distinguishes
  a stopped zombie from a live process; normal shutdown separately requires reaping.
- Oversized frames, wrong reply identity and token-usage overflow kill the worker and
  refuse publication. Bad artifact pins fail before a call and acquisition failure is
  memoized across selected nodes; a fresh explicit session can retry repaired artifacts.

The cloud instance is Linux x86_64, kernel 6.18.44, five available CPUs and no GPU.
Landlock reports unavailable and cgroup v2 is not delegated. The implemented CPU profile
therefore needs neither: a seccomp allowlist denies all new path opens, and the pinned
loader consumes a read-only `FILE*` model descriptor. This profile was exercised here;
aarch64, macOS filesystem/process denials, Metal load/inference, GPU pressure and signed
distribution remain unexecuted. Sampled footprint/Metal allocation is not a hard quota.

The opt-in real CPU smoke uses upstream
`Qwen/Qwen2.5-Coder-0.5B-Instruct-GGUF/qwen2.5-coder-0.5b-instruct-q4_k_m.gguf`, SHA-256
`1d9614638d18024d0fbb36575a15f1302a3adf044df10345688ec4f6e1c4ff32`, kept outside Git.
The worker is built from llama.cpp `b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea`, using the
explicit `chatml` template, 4,096 context tokens, batch 128, two CPU threads, 2 GiB hard
address space and 180 lifetime CPU seconds. The ignored integration test requires explicit
worker/GGUF paths and never downloads weights. Its input is a tiny synthetic Rust function;
it does not establish coding-task quality, corpus throughput or economic improvement.

Early smoke attempts revealed two separate model-output problems: generic JSON decoding
allowed incorrect brief field types, and the small model emitted unsupported identifiers
and misleading prose. The worker now constrains the brief structure with a grammar; source
acceptance continues to reject unsupported identifiers. The bounded smoke profile asks for
two short claims and empty optional arrays. Such integration evidence does not select this
model for production, make citations proof of truth or solve model quality. Terminal usage
from successful calls is actual GGUF token usage; interrupted calls remain unknown.

The final real CPU run passed: one accepted/cached brief, **377 prompt / 124 completion tokens**,
one launch, zero unknown-usage calls, and 725,540,864 bytes sampled peak worker RSS under
the 2 GiB virtual-address ceiling. The test's 40.90 seconds include hash/copy/setup work;
there is no throughput comparison. Replaying with an absent model path used zero calls
and zero launches. All nine new native fixture tests also passed. The first abrupt-death
fixture assumed the optional procfs child-list file; matching ordinary process PPid
instead made it portable to this kernel without weakening the stopped-process assertion.

The full regression executed **975 passes and one timeout**, with three intentional ignores
(two existing measurements and the separately executed real-model smoke), plus **all 20
child-process recovery cases**. The existing
`continuous_semantics_progresses_during_filesystem_traffic_and_converges` case exceeded its
12-second progress deadline. It passed unchanged in isolation in 2.99 seconds, then the
entire affected neural-retrieval suite passed **70/70** in 45.19 seconds with its measurement
case still ignored. Thus all **976 regular tests** have passing evidence across the full
run and affected-suite rerun; the one transient timeout remains recorded rather than
reported as an uninterrupted green run. No test deadlines or production scheduling were
changed to obtain the rerun. The 6,000-file watcher suite passed in 109.53 seconds.

The no-default-features build and a 30-test knowledge invocation also passed. The self
dev-dependency re-enables semantic in Cargo test, so the latter does not establish
disabled-feature integration; the shared-admission checks below exercise those binaries
directly. Native worker
build and Clippy, all-target Clippy with warnings denied, formatting, bundle-script syntax
and Markdown link checks passed (47 files, 729 links, 172 heading anchors). No dependencies,
schema changes or weights were added. The cloud setup draft records the additional pinned
CMake/libclang prerequisites and optional native-worker activation; publication/fresh-task
restoration is not claimed by these current-instance checks.

## Shared model memory admission — 2026-10-10

010 T002 / 009 maintenance now share opt-in worker-ceiling reservations and observed
memory headroom under the MCP owner. The policy has no schema, dependency or model/cache
identity change. Tests exercise the public `--model-resources` option, native synthetic
generation worker, source store and MCP status/context/pause/shutdown paths. The embedding
provider and deterministic pressure sampler are fixtures; combined real-model performance
and macOS memory/Metal behavior are not claimed.

New checks cover combined ceilings, refusal before embedding startup, retention through
provider shutdown, headroom for loading, pressure hysteresis, unavailable observations,
and status reads that do not sample. Background embedding waits under pressure while a
foreground query succeeds, then commits all 12 fixture rows after recovery. A synthetic
sleeping child verifies that idle eviction reaps before releasing capacity and preserves
artifact descriptors; existing native denial/resource tests retain their separate scope.

CLI/MCP fixtures verify generation reservation while resident, zero reservation after idle
unload, current-brief replay under pressure with missing artifacts and zero calls/launches,
zero usage on pressure/budget refusal, foreground source access, responsive pause and durable
pending work. Restarting after budget refusal completes the four-node plan.

The initial real-container checks exposed an inherited cgroup mount whose root is `/..`,
while `/proc/self/cgroup` names `/`. Treating that as an ordinary path correctly refused
observation but prevented admission. A bounded lookup of the owner's live PID now locates
the actual `init` group and includes its visible ancestor limits. A regression fixture
checks this namespace case alongside nested `memory.high`/`memory.max` limits and malformed
observations. No cgroup files or host settings were changed.

Admission is conservative rather than an OS quota: worker ceilings are reserved, while
host/container availability is sampled. In-flight work keeps existing native limits;
embedding residency stays available for queries. Owner/index memory, artifact storage,
other owners/processes and hidden cgroup ancestors are outside the reservation sum.
CPU utilization, global GPU pressure, thermal/battery policy and live macOS validation
remain pending in the [inventory](context-ecosystem-plan.md#pending-work-inventory--2026-10-10).

Affected suites have passing evidence for **392 tests**: 199 library, 33 knowledge,
90 MCP and 70 neural-retrieval tests, including ten new tests. The real-model generation
smoke and semantic measurement remain intentionally ignored in this run. After the cgroup
fix, one rerun of the existing 80 ms HTTP deadline fixture expired before call admission
(zero unknown-usage calls versus the fixture's expected one); the full knowledge suite
then passed unchanged in 34.88 seconds. That transient fixture failure is retained here;
no production/test deadlines were relaxed. The existing filesystem-traffic semantic case
passed in the 70-test affected-suite run. This is focused verification, not a claim that
the entire repository suite was rerun for this increment.

The core and synthetic generator additionally built with `--no-default-features
--features test-faults`. Invoking the existing knowledge test harness directly against
those rebuilt executables passed all three new CLI/MCP memory-policy cases in 6.80 seconds,
without Cargo rebuilding/re-enabling semantic. All-target Clippy with warnings denied,
formatting and relative Markdown link checks passed (47 files, 741 links, 181 anchors).
No real-model, macOS/GPU or large-repository performance experiment ran for this increment.
