# Validation

## Worker scratch creation defaults — 2026-11-13

**PASS for the selected direct-launch creation defaults.** The
shared 009/013 pre-exec setup now sets umask 0077 only in the forked worker child.
Ordinary scratch writers cannot inherit a permissive caller mask. The existing
launcher, descriptor handling, resource limits and cleanup owners remain; no
parent mask or preexisting object is changed.

The retained red on `a1a2d9d` completes an actual fake-worker embedding and observes
its phase/request files while the producer is live. Under caller masks 0000 and
0022, the files are 0666 and 0644; both children are reaped and the caller witnesses
retain their original modes. The command naturally exits 101 in 25.094 seconds.
The private wrapper then references a nonexistent receipt field; its original
failure is preserved. A separate readback validates the expected red and existing
cleanup fields, and the wrapper is corrected without rewriting that receipt.
The repaired focused test on `4f28854` passes in 18.171 seconds with both files
0600 under both masks and unchanged caller witnesses.

The first native attempt fails in 9.535 seconds because the pinned production
worker rejects the test-only phase-file hook before inference. It is retained as
a failed prerequisite attempt, with all selected inputs and scratch unchanged.
The next test uses the same pinned worker's existing native write probe under
`supervisor::child_setup`, while a normally launched model worker owns its run.
The probe creates one byte in a fresh scratch file; a normal real embedding call
also completes. Probe coverage is separate from inference-internal phase writes.
On committed candidate `dd4d34b33b4275bec9edc03a880114ca9c76b3d3`, both caller-mask
cases pass in 16.695 seconds: the named probe files are 0600, caller witnesses stay
0666/0644, both model workers are reaped, and two observed native worker identities
are absent. The owned scratch runs are removed. Worker/profile/model pins are
unchanged. Sampled worker
RSS peaks at 768,589,824 bytes; samples do not establish the true peak.

All eight gates pass on committed `dd4d34b` in 771.404 seconds: **1,312 passed,
0 failed, 35 ignored** in the default suite; **101 passed, 0 failed, 1 ignored**
in the pilot outer suite; 21 recovery checks separately. Nested child summaries
are excluded. The final fake-worker regression runs again in those gates; the
ignored native check passes separately. Cargo inputs, model/worker artifacts,
profile identities, pilot request construction and frozen v1 pins are unchanged.
All observed processes are gone; selected input bytes/modes and the prior scratch
inventory/sibling metadata match their before/after readbacks.

The stage uses a 3,600-second shared bound, a 120-second cleanup reserve, a 20 GiB
disk floor and an 8 GiB sampled RSS ceiling per observed process, with one heavy
job at a time. No paid request, provider credential, training, download, worker rebuild,
host change or holdout query occurs. A mask constrains default creation; explicit
creation modes, later chmod and every transient library output are not qualified
by these named writes. Closed producer-tree checks remain separate. Pilot v2/T006,
current installed-host compatibility, complete startup observation and signed
package acceptance remain open. Private receipts and failed attempts are retained
in `runs/worker-scratch-permissions-2026-11-13/`; the
[handoff](resume-context-ecosystem.md#current-status--2026-11-06) owns current status.

## Private full pipeline and preview archival — 2026-11-13

**PASS for the frozen-host, no-provider apparatus regression.** The full private
pipeline now passes on repair `b959b707eb0c498b67f34356538ac79211af782d`, with the
existing task, outcome, counter and request assertions intact. This exercises
actual local Gemma preparation, the pinned host, capture/journal handoff and the
contained compiler/oracle. Model replies are scripted through an owned loopback
TLS fixture with a synthetic credential. This is neither another paid coding trial
nor evidence of retrieval benefit; the original paid-v1 results and dispositions
are unchanged. Pilot v2/T006 and release remain unaccepted.

The earlier supplied-path mismatch is resolved for this run by finding the exact
frozen Foundry executable in retained Cargo release artifacts. A new private copy
matches SHA-256 `9dcbec8fdfff79d491410c82ab6e3ea922d3c19d5c9397f9d231fc08713c8614`
(100,278,224 bytes). A new input manifest points to that copy; the original file,
artifact and frozen pins are preserved. The host remains the exact archived
`38c36452…` artifact. The currently installed host hashes to `ce80099f…`, so current
installed-host compatibility is still unqualified.

**A real consumer failure is retained.** The first fresh regression on `d6bce4c`
exited 101 after 100.920 seconds: all four real view materializations and nine
previews completed, but capture admission refused `private_artifact_not_private`.
The previews retained valid 0700 native modules outside the active capture-run
exception. Ordinary isolation-root admission correctly requires those closed trees
to be data. No input pin, mode check, expected result or request assertion was
weakened to obtain a pass.

`render_binding` now confirms the host group is gone, validates its newly owned
preview tree, and archives only its exact native-module copies as 0600 data via
checked descriptors. Shared inodes, wrong modes and unknown executables refuse.
Bytes are preserved; file and parent are synced; the closed tree passes ordinary
data validation, and `preview_archived` is appended before returning the binding.
Active capture and checker exceptions are unchanged. Partial or failed archival
does not produce a successful binding. Historical artifacts are not normalized.

Four focused checks pass for bytes/modes, shutdown-before-change, hard-link refusal
and wrong-mode/unknown-executable refusal. The fresh full regression passes in
142.119 seconds: the probe seals, its journal prefix survives the checked handoff,
and all eight coding slots seal. The actual checker produces the expected
**scripted controls A 0/4, B 4/4**. There are 13 fixture generations, 13 Count calls
and 13 Messages attempts (26 fixture HTTP attempts), with no outstanding
reservations. Fixture ledger values are not provider charges. Eight embedding
workers are observed and absent after natural completion; no owned processes remain.
All selected input bytes/modes and the prior scratch inventory/sibling metadata
match their readbacks.

All nine archived native copies match the bytes from the retained failed run;
their modes change from 0700 to 0600. Each preview journal ends with
`preview_archived`; the capture journal ends with `finished` and eight slot seals.
A double readback verifies 493 directories at 0700 and 710 files, 1,984,937,844
bytes: 693 data files at 0600, nine capture-native and eight checker executables
at their declared 0700 paths. The report is archived by the test, not the
production CLI writer. This is no power-loss or transient-scratch-mode claim.

All eight gates pass on committed `b959b70` in 759.343 seconds: **1,311 passed,
0 failed, 34 ignored** in the default suite; **101 passed, 0 failed, 1 ignored**
in the pilot outer suite; 21 recovery checks separately. Nested child summaries
are not added to the outer count. Four archival unit tests and the ignored full
pipeline also passed separately. The v1 contract/policy/pins, Cargo inputs, host
bridge and request-content construction are unchanged.

The stage retained its original 3,600-second total, 120-second cleanup reserve,
20 GiB disk floor and 8 GiB sampled owned-process RSS ceiling. Each full/gate phase
had at most 1,800 seconds within the shared deadline; no extension was used. No
paid request, provider credential, training, download, host installation or new
model search occurred. Current installed-host compatibility, complete startup
observation, transient worker scratch-file modes and v2's declared task/arm/host
qualification remain open. Private receipts and the original failure are retained
in `runs/pilot-full-pipeline-2026-11-13/`; the
[handoff](resume-context-ecosystem.md#current-status--2026-11-06) owns current status.

## Pilot semantic-producer permissions — 2026-11-13

**PASS at the closed semantic-output boundary.** The existing bounded launcher
passes a real local-Gemma producer check on the owner's Mac. Test addition
`00b2806` and receipt-label clarification `e08aaf5` add one ignored test in the
existing apparatus module; every non-test byte of that module matches `d4109c4`.
The capture owner, Cargo inputs, frozen v1 policy/pins and production paths are
unchanged. No retrieval-quality change or new public interface is claimed.

The test invokes production `run_bounded` with the retained current release
Foundry (`60aff102…`), the frozen default profile (`cd3a5326…`), existing Gemma
worker (`b0f7e020…`), BF16 model and tokenizer. A fresh PilotRoot holds public
synthetic Rust/Markdown sources. Real indexing, semantic preparation and context
delivery complete: all five eligible units are embedded and searchable, and the
context stays within 512 tokens. `PilotRoot::open` validates produced trees after
each command. The parent starts and finishes with umask 0022; children receive
0077 through the actual launcher.

The negative control disables that child-mask operation and exits 101 at the
intended post-index privacy assertion, before any embedding worker runs. Source
is restored. The fresh green exits zero in 40.030 seconds including compilation;
two real workers are observed and absent after natural exit. All selected input
hashes and synthetic source bytes match their readbacks. The scratch entry
inventory and prior-sibling metadata match before and after. A separate double
readback verifies **16 directories at 0700 and 23 regular files at 0600**,
1,081,444 bytes total. These are closed output trees; transient worker scratch
file modes are not qualified. Sampled worker RSS peaks at 757,989,376 bytes;
this is not a continuous peak, GPU measurement or wider-workload limit.

All eight gates pass on committed `e08aaf549253e7a0bca94cba5086ec1e22f60bd7`
in 765.327 seconds: default suite **1,311 passed, 0 failed, 34 ignored**;
pilot unit suite **97 passed, 0 failed, 1 ignored**; 21 recovery checks pass
separately. The new ignored test passes separately with explicit local inputs.
Default and actual Rust 1.90 checks both pass with SDK 26.5. The supervisor exits
naturally without owned processes left; input and scratch-inventory readbacks
also pass after the gates.

Bounds were selected before implementation/execution: 3,600 seconds total with
a 120-second cleanup reserve, 20 GiB disk floor and 8 GiB sampled owned-process
RSS ceiling; 600 seconds for the producer, with 60/180/90-second command limits,
and 1,800 seconds for the gates within that stage. No paid request, provider
credential, training or download is involved. Local call/token economics do not
follow from the positive embedded-unit count.

Historical preflight; the later [full-pipeline regression](#private-full-pipeline-and-preview-archival--2026-11-13)
uses a recovered exact artifact without changing the original input file or pins.

**The unchanged private full pipeline was not admitted in this slice.** Its supplied
evidence-input file names mutable `target/release/foundry`: those bytes are now
`60aff102…`, while the frozen v1 apparatus requires `9dcbec8f…`. The preflight
mismatch is retained; the pipeline was not launched and no pin was relaxed.
This is not a global artifact-absence claim. Current installed-host compatibility,
complete startup-network observation, the broader private pipeline, pilot v2/T006
and release remain open. Private protocol, control, producer, gate and readback
receipts are in `runs/pilot-semantic-permissions-2026-11-13/`; the
[handoff](resume-context-ecosystem.md#current-status--2026-11-06) owns current status.

## T005 compact candidate: no promotion — 2026-11-11

**Reject the selected K=1 compact preference and preserve the current baseline.**
Candidate `0de75ff` (binary SHA-256
`67b8d6426acc9dfcd7e37445230c69d71a24e3faaa23dc3e78d62cacefee8f5e`)
passed all eight local gates in 772.810 seconds: 1,317 default-suite tests passed,
0 failed, 34 ignored; the separate pilot unit suite passed 97 tests. Both toolchains
and the opt-in pilot checks exited zero on the unchanged committed source. The
release build passed. This establishes implementation correctness, not quality.

The unchanged `scripts/g2.sh` ran one baseline/candidate pair against the pinned
current Gemma profile and fresh copies of the prepared units7 masters. Both arms
captured diagnostics; the previously qualified off/on baseline establishes trace
parity separately. All 388 questions per arm have valid wire, score, source-identity
and dense-readiness accounting, with the original 2,048-token cap.

| G2 corpus | Located baseline → candidate | Located gains / losses | Verbatim bodies baseline → candidate | Body gains / losses |
|---|---:|---:|---:|---:|
| Rust (197) | 52 → 53 | +1 / −0 | 47 → 33 | +1 / −15 |
| bun (191) | 117 → 121 | +4 / −0 | 107 → 79 | +2 / −30 |
| Total (388) | 169 → 174 | +5 / −0 | 154 → 112 | +3 / −45 |

Exact one-sided McNemar p = 0.03125, but **42 fewer verbatim bodies fail the
conjunctive enable rule**. The unchanged comparator exits 1, `KEEP OFF`, agreeing
with the stricter validator. Mean delivered tokens are 2,022.925 → 2,022.461 and are
reported only. The pair completed in 114.376 seconds; all 21 native commands exited
naturally and were reaped. Original/prepared-master/input readbacks and corpus/repo
identity checks pass. Sampled peak individual RSS was 3,049,750,528 bytes; 34 raced
samples were missed. This is neither a latency distribution nor a true continuous
memory peak or provider/host cost measurement.

The required current-semantic G1 comparison then completed in 677.573 seconds.
All three unchanged frozen guards exit zero over 10,432 raw tasks, 10,431 scored
after the existing one-task exclusion. First-call success, success within four calls
and verbatim-body success are identical in each group: **zero gains and zero losses**,
so there are no newly lost cases to review. Twenty-seven task records differ in
usage fields, including one fewer checker call: total calls are 14,056 → 14,055,
and delivered tokens are 13,206,239 → 13,205,241.
All 31 native commands exit naturally, with complete retained-reference/source/store
readbacks and an empty owned scratch directory. No per-query embedding count or
host-consumption claim follows from those token records.

A separate fixed-row CLI check passes in 16.984 seconds. It selected physical row
zero of the frozen G2 file before launch, used two fresh store copies, and required
literal same-arm CLI/MCP response-byte equality. Both outputs have 1,991 tokens;
the candidate trace records one actual allocation preference, versus zero in the
baseline. Both owners and workers were observed and naturally reaped; all eight
native commands and final input/master/scratch checks pass. This proves the launch
path, not another quality result.

The runtime flag and packing preference were removed after these observations;
the candidate commit, binary, control failures and private evidence are retained.
Source, tests, Cargo inputs and build scripts return exactly to the previously
gated `c399c1e` state; a fresh run of all 22 ordinary response tests also passes.
Formatting and link checks pass. The earlier full gates remain applicable to the
identical restored runtime/test/build inputs; they are not represented as a second
fresh eight-gate run. No alternative fraction, additional candidate or final held-out
query was tried. The sealed 400-question set stays unused and private. This closes
the selected one-variant evaluation with no promotion; it does not accept the broader
T005 quality goal. A new repair requires a new bounded selection.

Retirement commit `d9e0f04` rebuilt offline in 58.098 seconds. Its release binary
is byte-for-byte identical to the retained baseline (SHA-256
`60aff1025ba6515caa70b0854f233a1d2e2b4f8f83cfed290f143626e8c14011`).
Both `context --help` and `mcp --help` exit zero without the compact flag. The
build exited zero and was reaped; tracked inputs and the retained candidate binary
are unchanged. This restores the measured baseline executable; it is not another
G1/G2 or native acceptance run. The build and readback receipt is retained in
`runs/t005-restored-baseline-2026-11-11/`.

Private receipts: `runs/t005-packing-candidate-2026-11-11/`,
`runs/t005-compact-g2-2026-11-11/`, `runs/t005-compact-g1-2026-11-11/` and
`runs/t005-compact-cli-2026-11-11/`. G2 runtime receipt SHA-256:
`bbb06ebf5bc27ed250a4605bc5de6fbf08013ca0be1091fd1ef1605deef52451`;
G1 runtime receipt SHA-256:
`f9ff337583e4f54efa22e211f7408a4a646ca69667bc9ef1211ba349199669aa`.
These were local, no-spend measurements; signed-package and task/economic acceptance
remain separate. The [handoff](resume-context-ecosystem.md#current-status--2026-11-06)
owns current status.

## T005 sealed holdout encrypted retention — 2026-11-11

The new sealed holdout has a local encrypted archive in
`~/VSC_DEV/backups/foundry-t005-heldout-20261111/`, using the owner's selected
`~/.ssh/id_ed25519.pub`. All three tar/zstd/age pipeline exits are zero. Complete
source/clone byte-and-mode manifests match before and after, the private staging
copy is removed, and all five ciphertext/recovery-file checksum entries pass a fresh
readback. Archive size: 10,216,691 bytes; SHA-256:
`9b8e900bf01169d171476c8d06de9934e4106ea22b1ffe8029af3f1745a0b6de`.

The job was bounded at 300 seconds with a 20 GiB free-space floor and no network
or model/provider calls. It read only the selected public recovery key. The packet
was handled as opaque bytes; no fresh questions, labels or rationale were exposed
to implementation. Header syntax and ciphertext integrity are verified;
authenticated decryption/restoration and off-machine copying remain owner tasks.
Private receipts are in `runs/t005-holdout-backup-2026-11-11/`.

## T005 default-off compact preference implementation — 2026-11-11

Historical candidate `0de75ff`; the subsequent
[no-promotion result](#t005-compact-candidate-no-promotion--2026-11-11) records its removal.

The single variant selected in `7c5776d` is implemented behind the default-off
`--experimental-compact-context` flag on CLI context and the MCP owner. The shared
response packer owns the preference after normal candidate selection/root merging;
there is no store, recipe, model, schema, candidate-order or cap change. The selected
[contract](../specs/001-source-state-recovery/contracts/context-v2.md#experimental-compact-preference--009-t005)
keeps the one-eighth soft share and ordinary-ladder fallback fixed.

Six focused tests pass: four renderer fixtures, a real CLI/stdio-MCP flag matrix on
lexical input, and single/multi-root MCP owners using the existing concept embedding
fixture. They check actual semantic eligibility, default-byte parity, exact delivered
source, allocation reasons, later-body delivery, duplicate compact-form fallback,
token and serialized-byte refusal accounting, and lexical-only secondary-root parity.
The fixture provider establishes delivery plumbing, not Gemma quality. Positive
production CLI semantic delivery still requires the native runtime comparison.

Two mutation controls fail at their intended assertions: disabling the preference
loses the later body; disabling speculative source coverage duplicates an excerpt
and suppresses its richer parent. Both are restored before the fresh six-test pass.
An earlier control attempt hit an SDK linker error before tests; those logs remain
unqualified and preserved. The corrected checks explicitly use macOS SDK 26.5 and
verify the assertion failure, not merely exit 101.

`cargo check --offline`, formatting and diff checks pass. The focused command is
`cargo test --offline --test response --test mcp --test neural_retrieval compact_`.
Private logs, control patches and the frozen selection are retained in
`runs/t005-packing-candidate-2026-11-11/`. Full local gates and the unchanged G1/G2
comparisons remain to run. The fresh held-out set is still sealed; no candidate
quality result, default enablement, promotion or T005 acceptance is claimed.

## T005 fresh holdout seal and evaluation guards — 2026-11-11

**The fresh holdout is sealed before candidate implementation or tuning:** exactly
400 questions, 200 each over the pinned Rust and bun corpora. Four isolated authors
used the frozen module partitions and candidate order. Four separate in-session
checkers reviewed every answer against source and plausible alternatives, retaining
1,176 source-inspection records and 95 same-identity wording repairs. All 400 pass;
no answer identity was replaced. This is not an independent or cross-lab review.
The author ledgers retain 257 ordered skips, 408 wording attempts (8 failed), and
7 inspection-only quota stops, with no inspected candidate left without a disposition.

The final set has no duplicate normalized question text or overlap with the selected
historical exclusions: G2/F4 answers, one recorded retired answer, and retained old
wordings. The original three-per-file/six-per-module limits hold. The unchanged
`build.py -o` and full `verify.py`, without `--partial`, both pass. All original
verifier rules run, including six literal-bearing-unit mutations.

**A receipt-wrapper failure is retained.** The first wrapper expected the historical
G2 set's 46 self-tests, although its original builder and verifier both exited zero.
The unchanged verifier runs 37 fixed checks plus a data-dependent five-gram sample:
388 questions exercise nine samples; 400 exercise eight. The correct receipt is
therefore **45/45**, not 46/46. The corrected wrapper had already repeated the
mechanical check before the instruction to reuse its outputs arrived. Both actual
attempts exited naturally and were reaped, with identical engine logs and question
bytes. The original failure, script and logs remain intact. An offline receipt-parser
qualification passes 12 cases, including 11 rejection controls. Final qualification
took 260.565 seconds from the original 600-second mechanical-stage start; no question
or original-harness change, coverage waiver or retrieval rerun occurred.

Question SHA-256: `78b46d7cc8ba50282728419246043a0626db545ba9b3e56ae71473ad1d44e5bd`
(137,837 bytes). Metadata-only qualified seal SHA-256:
`71ee60e304938d1d0534151a30d6a508154ee3b94db2149cee42124ea5f2ae0c`.
After all eight author/checker children and the custodian were interrupted, an opaque
copy to a new private datasets directory passed complete byte/mode readback:
79 files, 45,344,937 bytes, source unchanged, process exit zero and reaped.
Transport-manifest SHA-256:
`48614417bd96c64cfd36494f24b9f6c15c82cfe5dd392604c4c790bda796f841`.
Implementation received only metadata; questions, labels, source rationales and
per-case outcomes remain private. The copy is local retention, not an authenticated
encrypted restore or off-machine backup.

**Evaluation statistics and outer guards are selected before evaluation.** K = 1,
with the fixed 200/200 sample. The lower bound averages the two corpus-specific
differences between an exact binomial lower bound for gains and upper bound for
losses. Four one-sided tails at 0.0125 give a conservative 95% model-based bound by
Bonferroni; this is a derivation from the
[NIST binomial limits](https://itl.nist.gov/div898/software/dataplot/refman2/auxillar/exacbino.htm)
and [Bonferroni inequality](https://www.itl.nist.gov/div898/handbook/prc/section4/prc473.htm),
not a claim that NIST endorses this experiment. Within-corpus binomial/independence
assumptions are not established by authoring: source clustering and curation limit
population inference. The dyadic arithmetic rounds outward and decides positivity
before display rounding; ten synthetic tests pass. The strict count adapter passes
five additional synthetic tests, including invalid token types/ranges, duplicate or
missing IDs, inconsistent body scores and invalid receipt bindings. These fixtures
do not qualify actual wire, source, runtime or promotion evidence.

The fixed sample gives 0.25 percentage-point overall resolution. Numerical planning
under gain/loss probabilities 0.15/0.05 in both corpora gives a 54.20% chance of
clearing the gain-bound and observed located guards; 0.25/0.10 gives 82.60%. These
are imposed-model calculations, exclude the other gates, and are not actual
candidate power or a justification to increase the sample after results.

The selected final pair keeps tracing off and runs baseline then candidate, Rust
then bun within each arm, once. Promotion still requires a positive lower bound,
exact one-sided McNemar p < 0.05, observed located and verbatim-body nonregression
in **each** corpus, valid complete accounting, the same 2,048-token/byte caps, all
unchanged G1/G2 gates, and review of new G1 losses. Mean tokens remain reported only.
For each corpus, complete MCP-owner elapsed time must be at most
1.25 × paired baseline + 2 seconds; sampled peak individual RSS at most
1.20 × baseline + 256 MiB and at most 8 GiB absolutely. The existing development
worker limit remains 4 GiB. This does not accept the installed 3 GiB profile.
There must be at least 20 GiB free, at most 40 GiB allocated growth, and one heavy
job. The whole pair is bounded at 3,600 seconds, reserving 240 seconds for final
readback and 120 for cleanup; each owner has a 600-second cap and 30-second cleanup.
Natural reaping and verifiable required measurements remain mandatory. Raced RSS
samples are reported; sampled peaks are not continuous maxima. These are observed
one-pair operating limits, not causal overhead or per-request cold/warm latency.

Private receipts, selected guards and synthetic helper qualifications are retained
in `runs/t005-holdout-preparation-2026-11-11/`. No retrieval/model-worker/provider
experiment or store API was invoked for authoring, checking or sealing. The exact
candidate algorithm and its executable manifests still need to be frozen. No
candidate, held-out retrieval result, quality promotion or T005 acceptance is claimed.

## T005 fresh holdout authoring selection — 2026-11-11

The completed diagnostic review is published as `cf2923c`; its private evidence
snapshot passed full readback (39 files, 3,343,063 bytes; manifest SHA-256
`5b52e78a41a2488733709464bc7635e4ef1ebda23105269215bd5632ab658036`).
No usable unused holdout is designated by the checked records. The old 48-question
archive was declared lost; F4's 300 questions were already evaluated. Neither is a
fresh T005 holdout.

Before any fresh question drafting, the new authoring protocol selects **400
questions, 200 per corpus**, at the existing source pins. This exceeds the unchanged
verifier's 180-per-corpus/360-total floors; it is not yet a power claim. One candidate
family may change packing allocation over already placed units, holding Gemma,
representation, selection, ordering, placement and delivery limits fixed. The exact
algorithm, sample-size/power rationale, paired confidence statistic, multiplicity,
per-corpus and latency/resource guards must be frozen before tuning/evaluation.

Authors work in fresh contexts without retrieval outputs, traces or candidate
design, using disjoint modules and deterministic candidate order. Historical G2/F4
answers and duplicate question text are excluded. Every visited candidate and skip
reason is retained; the original limits of three questions per file and six per
module remain. A separate label-side checker inspects every accepted answer and
plausible alternatives. The unchanged builder and full verifier, including all
self-tests, remain mandatory. This is an in-session source quality check, not an
independent review gate. Only metadata reaches implementation; questions, labels,
source excerpts, responses and case outcomes stay private.

The selected authoring/checking bound is three hours, followed by at most ten
minutes for mechanical validation and two minutes for cleanup. There are no paid
provider calls, retrieval calls or store opens, and at most one heavy job. Failure
to reach the exact counts with valid identity, leakage and answer-quality checks
retains the partial evidence and stops tuning/evaluation; no floor is silently
lowered. The private selection is in
`runs/t005-holdout-preparation-2026-11-11/authoring-selection.json`. No set is sealed,
candidate implemented, improvement measured or T005 acceptance claimed here.

## T005 complete loss review and refined trace attribution — 2026-11-11

**All 52 newly lost G1 tasks have been reviewed**, taking the union of first-call,
four-call and verbatim-body losses: 50 checker tasks and 2 bun tasks. The unchanged
qualified set has no losses. Full result and decompressed response hashes bind the
review to the [current G1 comparison](#t005-current-semantic-g1-comparison--2026-11-11).
Checker expectations are bound through the frozen occurrence metadata and task-ID
rule. The two bun expectations are explicitly reconstructed from accepted baseline
verbatim bytes, locators and the frozen generator's ID rule; they are not presented
as separately serialized labels or a fresh source/compiler-graph export.

The checker review observes 26 tasks with the expected file absent from delivered
units, 8 with the file returned but the required definition line omitted, 13 with
insufficient distinct usage files, 1 with the definition visible but its verbatim
body lost, and 2 recovered only after the first call. Both bun losses omit the
expected file from every returned unit. All reviewed outcomes agree with the
retained scorer. The checker comparison's valid error replies are all
`symbol_not_found`; among its reviewed loss cases, two new reference calls return
that error. Neither that histogram nor a `semantic:ready` response header proves
an earlier cause. G1 has no retrieval traces, so candidate generation, ordering,
cap and packing explanations remain unobserved for these cases.

**Offline G2 refinement accounts for all 388 retained cases using complete target
name ranges and frozen/current source hashes.** It verifies 330 distinct target
files and separates lexical document ranges from enclosing units, source inputs
from whole-file outlines, and exact placement/materialization/packing paths.
Twelve synthetic boundary tests pass. Missing joins remain unknown by contract;
none is missing in this retained run. This reuses validated delivery scores and
does not rescore, run retrieval, call a model or open a store.

The 219 misses have these sets of observed loss paths:

| Observed path set | Rust | bun | Total |
| --- | --- | --- | --- |
| Target file absent from the returned selection window | 24 | 9 | 33 |
| File seen, no source document/unit covering the complete target name | 47 | 15 | 62 |
| Global placement limit | 29 | 17 | 46 |
| Per-file placement cap | 24 | 13 | 37 |
| Both placement limits | 0 | 2 | 2 |
| Token-budget packing drop | 12 | 12 | 24 |
| Token-budget packing drop and global placement limit | 6 | 3 | 9 |
| Token-budget packing drop and per-file placement cap | 2 | 3 | 5 |
| Token-budget packing drop and both placement limits | 1 | 0 | 1 |

These are sets of observations, not exclusive causes or predicted repair gains.
Four cases move from the initial coarse placement category to noncovering lexical
documents: their enclosing units covered the name, but the recorded documents did
not. The original coarse records remain intact. Of 169 successful deliveries, 79
also have a cut or drop on another candidate path; counting such events as failed
queries would overstate loss. All 39 misses retaining a non-outline source range
covering the full target name at packing input have an explicit token-budget drop;
15 also have placement losses on other candidates. No name-losing materialization
excerpt is observed among the 208 cases with materialized target ranges.

Eight deterministic examples, the first two IDs from each original coarse miss
group, were followed through their exact trace/output and target-source bytes.
All eight are bun, so they are illustrations rather than a representative sample.
Two show an available dense source range retained through materialization, followed
by a token-budget drop of its signature form after earlier verbatim selections.
The **selected hypothesis for the next bounded contract is packing allocation**:
spending less of the allowance on early full bodies may leave room for existing
compact forms of later already-placed definitions. Keep the model, recipe, ANN
window, selection, placement, source checks, forms and caps fixed. No concrete
variant is selected or evaluated here. Rejected-form costs and counterfactual
allocations are unavailable; body regression remains a material risk. Absence from
a bounded retrieval window does not establish missing behavioral meaning or justify
an embedding/ranking replacement. Fresh held-out pre-registration and the existing
G1/G2, body, cap and resource gates remain required before any promotion.

The first offline analyzer produced complete results, but its outer process sampler
was sandbox-denied after launch; direct child exit/reaping was not recorded. That
failed supervision and the command-absence check remain preserved. A separately
selected native recheck of the unchanged analyzer then exited naturally in
**1.944 s**, with matching input pins, unchanged analyzer source, no observed
remaining owned process or model worker, and exactly identical case bytes. Reports differ only in elapsed
time and self-reported RSS. Bounds were 240 seconds including 120 for cleanup,
1 GiB sampled RSS and USD 0 spend. This repeats only offline analysis, not the
model/harness run, and does not retroactively repair the first lifecycle receipt.
The private review and both analysis attempts are retained in
`runs/t005-diagnostic-review-2026-11-11/`. T005 remains unaccepted; no product code,
ranking policy or profile enablement changed.

## T005 current semantic G1 comparison — 2026-11-11

**All three frozen G1 aggregate guards pass; the current semantic profile still
loses cases against the current no-profile implementation.** On `e5e0516`, one full
semantic run per set used the archived `f19d353` normal binary and current units7
masters. The no-profile side reuses the accepted T010 **candidate** runs, whose
result bytes and decompressed response hashes match before and after pairing.
Their starting whole-store manifests match the lineage of the prepared masters.
Compiler preservation through semantic-only preparation is a source-boundary
inference over that verified lineage, not a fresh compiler-table equality export.

| Set | Scored tasks | Pass on first call, no-profile → semantic | Pass within four calls, no-profile → semantic | Verbatim bodies, no-profile → semantic |
| --- | --- | --- | --- | --- |
| bun | 999 | 971 → 969 (+0/−2) | 976 → 974 (+0/−2) | 572 → 570 (+0/−2) |
| checker | 6,881 | 3,741 → 3,706 (+6/−41) | 3,868 → 3,831 (+10/−47) | 2,526 → 2,495 (+4/−35) |
| qualified | 2,551 | 2,315 → 2,315 (+0/−0) | 2,334 → 2,334 (+0/−0) | 1,750 → 1,750 (+0/−0) |
| Total | 10,431 | 7,027 → 6,990 (+6/−43) | 7,178 → 7,139 (+10/−49) | 4,848 → 4,815 (+4/−37) |

Body accounting applies to the 6,881 definition tasks. The frozen scorer's one
external-only bun usage exclusion is unchanged: 10,432 raw tasks become 10,431
scored tasks. No task is otherwise missing, duplicated, retried or excluded as an
apparatus error. The new runs contain 14,056 terminal tool calls, against 14,284 in
the retained no-profile runs. Their 950 valid tool-error replies (928 checker,
22 qualified) remain in scoring, against 999 in the reference; they are not silently
dropped or treated as measurement failures. Every recorded reply stays within its
frozen tool allowance. Scored tool-text tokens total 13,206,239 against 13,309,894;
these are not provider input, host consumption or cost savings.

The unchanged `g1score.py` supplies paired outcomes. Separate unmasked
`g1compare.py` calls use its fixed historical Foundry/grep references and all exit 0.
Those aggregate passes do not erase the newly lost current-comparison cases or
create a zero-individual-loss rule. The
[complete loss review](#t005-complete-loss-review-and-refined-trace-attribution--2026-11-11)
retains all of them. This is a baseline diagnostic, not a new quality promotion.

The new owner wrapper preserves bytes and records actual owner exit, complete
request/response IDs, tool arguments, text and error bits. Its checks account for
each new run's complete transport against its frozen harness records. Historical
no-profile artifacts do not acquire retroactive full-wire proof. G1 tracing stays
off, so per-query semantic decisions and complete provider-call counters are not
available; an observed worker alone does not prove which requests called it.
Twelve synthetic transport/accounting tests pass. The separately pinned
[G2 pair](#t005-current-g2-baseline-and-trace-parity--2026-11-11) supplies the live
dense-serving and trace-parity evidence.

The stage completed in **677.230 s**, with semantic group phases of 74.907 s for
bun, 389.097 s for checker and 149.290 s for qualified. These are current instrumented
run times, not a timing comparison with the retained reference. All 30 native
commands exited naturally, with no forced cleanup or observed owned descendants
remaining. One embedding worker was observed in each group. Original and immutable
master manifests, retained results and selected artifact/helper/input hashes match
on readback. Root's final checks confirm unchanged repository/corpus HEADs and
tracked files, unchanged existing scratch sibling metadata and an empty new scratch
child. No original store was opened through Foundry.

The preselected envelope was 3,600 seconds, with 120 each for final readback and
cleanup; group ceilings were 360/1,200/600 seconds, one heavy job and USD 0 spend.
Bounds: 20 GiB free space, 40 GiB apparent growth and 8 GiB sampled individual RSS.
Observed owned-process RSS peak: 3,167,977,472 B; driver: 215,302,144 B;
apparent growth: 19,669,063,201 B; minimum free space: 259,895,570,432 B;
35 missed process samples. Sampling does not establish transient/GPU peaks.
Private evidence remains in `runs/t005-g1-baseline-2026-11-11/` and
`datasets/context-foundry-t005-g1-z1wa3aqv.noindex`; runtime receipt SHA-256
`a0a3f10a1930e1b6d72b62d87fcc1694d79f0ccfb710d43ff75f690da12a85dd`.
No ranking variant, installed-package acceptance, paid trial or T005 acceptance is
established by this run.

## T005 current G2 baseline and trace parity — 2026-11-11

**The single traced/untraced baseline pair is valid.** On `28567ed`, the unchanged
`scripts/g2.sh run` executed the frozen 388 questions once with tracing off and
once with tracing on. Both arms used the archived normal `f19d353` binary
(SHA-256 `60aff1025ba6515caa70b0854f233a1d2e2b4f8f83cfed290f143626e8c14011`),
the accepted Gemma hierarchy profile and fresh query copies of the
[prepared units7 masters](#t005-units7-baseline-preparation--2026-11-11).
The [fresh corpus capture](#t005-corpus-identity-and-diagnostic-helper-checks--2026-11-11)
is separately pinned. No ranking variant or retrieval-code change was selected.

All **388 traced decisions are dense at the pinned source revisions**, with zero
off, fallback, missing or malformed decisions. Both arms have exactly the expected
197 Rust and 191 bun IDs, with no missing/duplicate/retried/refused/error rows and
every response within 2,048 tokens. Complete raw protocol stdout, parsed envelopes,
delivered text and frozen result rows match exactly between arms. The offline
validator accounts for every trace and reports no unknown rows or global errors.

| Corpus | Queries | Located definitions | Verbatim bodies | Mean delivered tokens |
| --- | --- | --- | --- | --- |
| Rust | 197 | 52 | 47 | 2,020.924 |
| bun | 191 | 117 | 107 | 2,024.990 |
| Total | 388 | 169 | 154 | 2,022.925 |

Both arms have these same scores. They are a fresh baseline measurement, not an
improvement over a candidate or retroactive evidence for the historical 249 misses.
The current 219 misses partition by first observed stage as follows:

| Recorded boundary | Queries |
| --- | --- |
| Expected file absent from selection events | 33 |
| Expected file present, target-covering unit absent from selection events | 58 |
| Target-covering placement events all rejected, with duplicate/limit/per-file-cap outcomes | 89 |
| Target-covering unit reaches packing input, but the frozen scorer does not verify the definition in the delivered response | 39 |
| Unknown | 0 |

These are initial observed boundaries, not established causes. The private
per-query records retain exact placement outcomes and ranks. The subsequent
[refinement and case review](#t005-complete-loss-review-and-refined-trace-attribution--2026-11-11)
distinguishes enclosing units from source documents and retains overlapping paths;
these initial counts alone select no representation or ranking replacement.

The untraced script phase took **37.651 s** and the traced phase **38.607 s**, a
0.956-second difference in this one sequential pair. Those phase times include
store-copy, wrapper and harness work; they do not establish a latency distribution
or isolate tracing as the cause of the difference. The whole bounded stage took
**103.950 s**. All 19 native commands exited naturally, with no forced cleanup or
observed owned processes remaining. Two embedding workers were independently
observed in each arm, with native memory samples. Query embedding-call and input-token
scalars are not exposed by this apparatus; 776 validated context responses and the
frozen delivered-token counts do not establish provider or host consumption.

Original and retained master byte/metadata manifests match before and after.
Binary, model, tokenizer, worker, helper, instrument and profile pins match on
readback; strict worker signature verification passes. The copied profile changes
only its private scratch child, which finishes empty; existing scratch sibling
metadata is unchanged. The selected HEAD and tracked files remain unchanged.
The qualified wrapper/validator sources were unchanged; 13 additional pure synthetic
checks passed for the separate full-dense readiness expectation. An observed off
decision would require review under existing anchor admission, not establish a
product defect by itself.

Selected bounds: 3,600 seconds total, 120 reserved for cleanup, 240 for offline
validation, 1,300 per script phase, 600 per owner plus its 30-second cleanup grace,
one heavy job and USD 0 spend. Resource bounds were 20 GiB free space, 40 GiB
observed apparent growth and 8 GiB sampled individual RSS. Observed peaks:
3,087,745,024 B owned-process RSS, 210,780,160 B driver RSS and 12,562,913,761 B
apparent growth; minimum free space 261,389,533,184 B; 30 missed process samples.
These observations do not establish true transient/GPU peaks or an atomic source
snapshot. Private evidence is in `runs/t005-baseline-pair-2026-11-11/` and
`datasets/context-foundry-t005-baseline-2026-11-11-9ptm180p.noindex`.
The runtime receipt SHA-256 is
`d7e5afb1e46f065d91facec98b1c9da6e0b9e919152789239f1a15c970956b26`.
T005 remains unaccepted; no quality promotion, installed-package acceptance or
paid-provider result is claimed.

## T005 corpus identity and diagnostic helper checks — 2026-11-11

**The current admitted corpora match the frozen parser manifests.** A separate
600-second, no-model stage used the archived parser candidate from `68ed776`
(SHA-256 `9853f8afebd457bd694362c6d92f829976a195e4285d85263994d33000d4bcf8`)
to capture each corpus into a new owned store and source bundle. It did not open
an original G2 store. The command's complete production admission and reconstructed
source-byte checks pass; new manifests exactly match the frozen ordered path,
SHA-256, length and language-override records:

| Corpus | Sources | Source bytes | Manifest SHA-256 |
| --- | --- | --- | --- |
| Rust | 60,739 | 177,769,746 | `2b36b53aef11ee64ef0b9cf6cba2110192efc0a15039a33948e96e6c3e3a1542` |
| bun | 8,436 | 99,926,130 | `f855b1b80c316cbac73efd0e3a5712868bea54c92dfe45fb12a24cd50187b5ab` |

Binding checks rehash all 59,387 unique Rust and 8,346 unique bun source blobs,
then repeat the complete bundle readback. Frozen input bytes and metadata match
before and after. Both corpus HEADs match their frozen commits and tracked files
are clean on both observations. Git excludes untracked files; the fresh capture,
rather than that Git check, establishes the complete admitted-set comparison.
This is separate fresh-store evidence, not a row-by-row export of the prepared
semantic store or an atomic filesystem snapshot under concurrent edits.

Rust capture took 108.489 s and bun 18.803 s; the stage completed in **135.983 s**.
All 10 native commands exited naturally, with no observed model workers or owned
processes remaining. Bounds: 120 seconds for cleanup, one heavy job, USD 0 spend,
300/120-second native capture ceilings with 270/90-second internal budgets,
20 GiB free-space and apparent-growth bounds, and 8 GiB sampled individual RSS.
Observed RSS peak: 1,269,481,472 B; minimum free space: 262,207,369,216 B;
apparent growth: 1,337,574,430 B; 10 missed process samples and three disappearing
temporary entries during growth observations. These are sampled observations.
Private stores and bundles remain under the new
`datasets/context-foundry-t005-corpus-freshness-a9rz0a1k.noindex` directory;
receipts and pins are in `runs/t005-corpus-identity-2026-11-11/`.

**Diagnostic helper checks are synthetic qualification only.** The private stdout
wrapper and offline validator first passed 25 tests. A narrow subsequent correction
validates supplied source identities, half-open ranges and stage-specific outcomes
before assigning an observed pipeline stage; malformed evidence remains unknown.
It also removes the inference that a locator's range proves visible definition
content. The final helper passes **34 tests in 1.954 s** (2.219 s including native
supervision), with unchanged source pins and no observed remaining owned processes.
The wrapper itself is byte-identical between these versions. Raw logs and both
source versions are retained in `runs/t005-trace-qualification-2026-11-11/`.

Tests exercise exact forwarding under backpressure, unchanged stdin, output/line
limits, refusal, deadline cleanup and malformed/missing trace or identity rejection.
Transport fixtures substitute a synthetic manifest in test-process memory; they do
not qualify real manifest admission, model calls, source freshness or the full
388-question accounting. Each check has a separately selected 180-second envelope,
60-second test limit and 120-second cleanup reserve. No private questions, real
stores, model or provider were used by these checks.

The frozen G2 harness can retry a busy/deadline response; zero retries is a later
run-validity condition, not a guarantee made by the transparent wrapper. The
qualified helper preserves exact text and full parsed envelopes, checks complete
request/notification accounting and records raw stdout equality separately.
Delivered-token counts come from the frozen harness; the offline validator does
not recount them or infer host/provider consumption. Actual G2 parity, trace
completeness, current scores and T005 acceptance remain to establish.

## T005 units7 baseline preparation — 2026-11-11

**Both owned copies pass current units7 metadata readiness; no query-quality
result is claimed.** Following the
[obsolete-recipe preflight](#t005-current-baseline-identity-preflight--2026-11-11),
a separately selected 3,600-second stage on `6831dbb` prepared fresh retained
copies of the singleton masters. It used the same archived `f19d353` normal
binary, accepted Gemma hierarchy profile and existing development worker. Only
the copied profile's scratch path changed, to a new private child inside the
worker's existing signed grant. Model, tokenizer, worker, original profile and
binary hashes match before and after; strict worker signature verification passes.
The bundle was not changed or re-signed.

| Corpus | Sources / revision | Current eligible, cached and searchable units | Unique ANN entries | Prepare elapsed |
| --- | --- | --- | --- | --- |
| bun | 8,436 | 191,161 | 183,311 | 168.725 s |
| Rust | 60,739 | 685,623 | 624,325 | 684.782 s |

Each command reports `partial:false`, a published index, stopped state and no
stop/publication reason. Subsequent full audits report complete partition coverage,
no unpartitioned or missing units, no corrupt/legacy/orphan cache entries and an
available generation. Source counts, revisions and workspace identities remain
unchanged. The recipe is
`hierarchy-v1+overview4+context2+kinds2+units7:512+parent64+tokenizers 0.23.2 onig`,
dimension 768, with unchanged function digest
`01dcfe577222256b4e08f9783e973fddb80240be53fe96c7454a6956c100e912`.
Unique ANN entries deduplicate exact inputs; they need not equal eligible units.

Both preparations report **0 document calls, 0 embedded units and 0 input tokens**;
cached vectors were reused. Run work counters revisit sources/units and are not
corpus totals. This proves no current query capability or quality result, and the
new generation cannot inherit an old G2 score. No model worker was observed.

The whole stage completed in **879.817 s**. All 22 native commands exited naturally,
with no observed owned processes remaining. Original stores were never opened
through Foundry and their complete before/after byte and metadata manifests match.
Complete final manifests pin both prepared copies, including their generation
files. Existing scratch sibling metadata is unchanged and the new scratch child
is empty. The copies remain in the new owned directory
`datasets/context-foundry-t005-units7-2026-11-11-sa2l0xjn.noindex` for the diagnostic.

Selected limits: one heavy job, USD 0 spend, 120 seconds reserved for cleanup,
another 120 seconds for final audits, at most 900 seconds for bun and 2,400 for Rust,
4 GiB cache per store, 20 GiB free-space floor, 40 GiB observed apparent-growth
ceiling and 8 GiB sampled individual-process RSS ceiling. Observed owned-process
RSS peak: 4,160,634,880 B; driver: 205,766,656 B; minimum free space:
264,595,890,176 B; apparent growth: 7,103,069,537 B; 22 owned-process samples missed.
Sampled values do not establish transient or GPU peaks. A separate lightweight
synthetic trace-helper check ran for 2.434 seconds during Rust preparation; no
comparative preparation-performance claim is made.

Private evidence is under `runs/t005-units7-preparation-2026-11-11/`, including the
preselected bounds, exact commands, reports, audits, artifact readbacks and complete
store manifests. This stage performed no repair-index, cache purge, download,
query, provider request or workspace-program execution. Current corpus contents
and full traced/untraced response parity still require verification. Stable store
metadata is not a fresh row-by-row source-table equality proof. T005 remains
unaccepted; no ranking variant is selected.

## T005 current-baseline identity preflight — 2026-11-11

**Metadata preflight passed; semantic query readiness is not established.** On
`5bef761`, the archived normal binary from `f19d353` (SHA-256
`60aff1025ba6515caa70b0854f233a1d2e2b4f8f83cfed290f143626e8c14011`)
opened only verified disposable copies of the T010 singleton Rust and bun masters.
Normal runtime, build and dependency files are unchanged since that binary's
source commit; intervening runtime changes are confined to the pilot feature.
This is not a claim that the binary was rebuilt at the current documentation head.

Both ordinary indexes report ready, complete scans, no pending sources and no
parse failures: 60,739 Rust sources and 8,436 bun sources, at source schema 12.
Both semantic progress reports are stale. Full audits refuse their recorded
`hierarchy-v1+overview4+context2+kinds2+units6:512+parent64+tokenizers 0.23.2 onig`
recipe with `unit_schema_obsolete`; every source is unpartitioned for the current
recipe and no current semantic generation is available. Retained caches contain
624,325 and 183,311 entries respectively, with zero corrupt entries observed.
The audit's zero missing units accompanies zero eligible current units; it does
not mean complete coverage. Recorded provider-ready observations and bootstrap
usage are historical, not calls or readiness established by this preflight.

Opaque byte checks pin the profile, 388-question instrument, G1/G2 harnesses,
counter, binary and Cargo lock. Corpus HEADs match their frozen commits. Corpus
working-tree contents, model/worker artifact bytes, current local-model capability,
full response parity and retrieval quality remain unchecked at this stage.
The original profile and both original stores were never opened through Foundry.
Complete original file-byte and metadata manifests match before and after each
attempt; all quarantines, caches, scratch and failed copies remain retained.

**Copy failures and bounded continuation.** The first attempt stopped before a
store open because `cp -c -R` inherited group 0 instead of 20. Its retry with
`cp -c -p -R` preserved modes but still changed the group of 322 regular files.
Both failures remain recorded; the retry used the original absolute deadline and
closed at 431.670 s elapsed (10.570 s in the retry itself). A tiny owned fixture
then verified descriptor-bound group preservation without changing bytes, mode
or inode. A separately selected 600-second continuation applies that correction
only to fresh copies and requires exact byte, kind, mode, UID and GID identity
before opening them. It completed in 16.033 s, with all 13 native commands exiting
naturally, no observed model workers and no observed owned processes remaining.

Both selections reserve 120 seconds for cleanup, keep one heavy job, a 20 GiB
free-space floor, 20 GiB apparent-growth ceiling, 8 GiB sampled individual-process
RSS ceiling and USD 0 spend. The successful continuation observed 1,434,107,904 B
peak owned-process RSS, 203,358,208 B driver RSS, 266,857,476,096 B minimum free
space and 7,102,255,589 B apparent growth; 13 owned-process samples were missed.
Sampling does not establish transient or GPU peaks. No model, query, preparation,
repair, provider request or source program ran.

Private evidence is retained under `runs/t005-baseline-preflight-2026-11-11/`,
including both failed attempts, copy fixture, selected bounds, raw status/audits,
input pins and complete original readbacks. The next runtime dependency is a
separately bounded local `semantic prepare` on new owned copies for units7,
followed by current identity, coverage and query checks; no ranking variant is
selected. [T005](../specs/009-optional-semantic-retrieval/spec.md#t005--diagnose-and-improve-anchorless-evidence-retrieval)
remains unaccepted. Fresh traces cannot retroactively explain historical misses.

## Pilot private-output permissions and terminal publication — 2026-11-11

**Qualified at the selected local boundaries on `c399c1e`.** The
[existing v2 requirement](../specs/003-agent-retrieval-context/contracts/bounded-pilot-v2.md#required-v1-gap-repairs-and-qualification)
now enforces exact 0700 owned directories and 0600 private files, including special
bits. Descriptor-relative checks preserve ownership, type and inode identity;
one logical pass shares 16,384 inspected entries, depth 64 and 8 MiB of OS-path
bytes, including both inventory and validation. Child launchers set umask 0077
without changing their caller. Existing wrong-mode objects are refused, not repaired.

Checks run before count/Messages and after producers. A privacy failure after a
count retains that observation, releases only its reservation and prevents Messages.
Post-send failures retain observed usage and halted slot capture/closure while
refusing unsafe readback. CLI success additionally requires actual exclusive,
durable seal/report persistence and successful finalization; terminal journals
are not reopened to hide sidecar failure.

The only declared 0700 executables are checker `out/check` and pinned OMP 18.13.2's
`home/.omp/natives/18.13.2/pi_natives.darwin-arm64.node`, limited to the declared
run's `hosts/<host>` or fixed `runs/<run>/hosts/<host>` layout. No workspace/store
or arbitrary-suffix exception exists. This records the observed extraction and
checks metadata; it adds no per-request native-content hash or host re-pin.
The archived host remains `38c3645283ceba06f66462830ae15f27270cc7f84575e40a4c6bd68452a63d0f`.
Frozen v1 contract, policy module and apparatus pins are byte-identical.

**Final focused checks.** Counts are passed / failed / ignored; nested isolated
children are not added to outer counts.

| Boundary | Counts | Elapsed |
| --- | --- | --- |
| Pilot units | 97 / 0 / 0 | 34.886 s |
| CLI units | 6 / 0 / 0 | 17.342 s |
| Four lexical stores / process reopen | 1 / 0 / 0 | 6.805 s |
| Complete pinned-host fixture suite | 33 / 0 / 2 | 279.922 s |
| Actual compiler/oracle producer | 1 / 0 / 0 | 3.664 s |

The host suite uses the same production code but predates the ignored producer's
test-only container-mode correction; it is not an identical full test-source claim.
That producer was separately rerun on the final source. Safe readback confirms
private checker artifacts, expected completion/binary bindings, and an ordinary
compile-error candidate exiting 1 without executing an oracle. The four stores
are actual current CLI lexical stores, not semantic-worker qualification.

**All 15 selected controls executed unchanged failing assertions and restored
exact source:** mode, component-links, creation-dir-mode, creation-file-mode,
child-mask-bounded, child-mask-checker, parent-mask, host-mask, path-byte-budget,
send-count, send-messages, post-host-closure, cli-report, cli-probe-persist and
checker-mode. Each inner test returned 101 with its runtime marker and no compile
failure. `control-boundaries.json` preserves the precise reached assertion and
unreached checks. In particular, send-count failed the generations-length assertion;
it is **not an observed extra-HTTP proof**. Send-messages reached a fixture-path
mismatch without disclosing its exact vector. Host-mask reached non-completion,
not a zero-HTTP assertion. Creation controls failed prerequisites before metadata
assertions; checker-mode reached only its successful-step receipt branch.
Parent-mask affects only the isolated caller. CLI probe persistence uses the real
finished fixture-probe receipt; post-host-closure directly exposes lost returned
capture, not independently observed later closure rows.

**Failures retained:** 8 earlier unexpected-exit commands remain in safe
metadata, including missing test metadata/permission imports, restrictive-mask
creation, APFS filename setup, CLI fixtures and the first full-host suite.
macOS refused O_EVTONLY on mode 000: exclusive mkdir now uses nonfollowing stat,
fchmodat, then opened-inode comparison and child/parent sync. APFS rejected invalid
UTF8 filenames; the raw-byte test directly charges an OsString and separately
walks an accepted multibyte filename. Two synthetic Live fixtures moved auxiliary
Verification probes to separate owned environments. The RLIMIT_FSIZE child uses
pipes for libtest reporting. The ignored producer's newly owned 0755 container
was corrected to 0700 after readback and rerun. These fixture corrections preserve the production guards.

**Separate bounded phases.** The original 3,600-second stage closed after
2931.612 s **unqualified because full gates were absent**;
its deadline was not extended. A separately selected 1,800-second gate-only
continuation tested the same committed candidate. All eight actual gate exits are
0 (fmt, clippy, tests, pilot clippy/tests and three Rust 1.90 gates), completing in
614.765 s; continuation elapsed is
625.829 s, within its original deadline. Default tests:
1311 passed, 0 failed, 34 ignored;
recovery: 21 checks; pilot outer: 97 passed,
0 failed, 0 ignored, with
1 intact nested summary. Two child summaries interleave with parallel test output
in both the gate and final focused logs; each log retains three child 1/0/0 count
fragments plus the outer 97/0/0. The original parser mismatch is preserved and
explained in `nested-summary-inspection.json`; children are never added to outer
totals. Rust/Cargo 1.97.1 and 1.90.0 use SDK 26.5.

Both phases retain 120 seconds for cleanup, one heavy job, a 20 GiB free-space floor,
8 GiB sampled individual-process ceiling and USD 0 spend. Focused sampled RSS/free
minimum: 2,192,277,504 /
266,261,848,064 bytes, with
225 missed samples; gates:
1,418,739,712 / 263,419,609,088 bytes, with 1382 misses. Receipts report natural exit,
no forced cleanup and no observed owned identities remaining. These are sampled
observations, not true peaks or GPU/global-memory guarantees.

No real provider request, real credential, original private full-pipeline rerun,
semantic-worker output acceptance or current-installed-host qualification is
claimed. Startup-network completeness, arbitrary same-user race protection,
deterministic fchmod/fsync fault injection and power-loss durability remain
unproved. Pilot v2 stays unfunded/unfrozen; T006 and task-context outcomes remain open.
Private receipts remain under `runs/pilot-private-permissions-2026-11-11/`:
`commit-pin.json`, `stage-closure.json`, safe command/control metadata, readbacks,
`control-boundaries.json`, and `gate-continuation/{protocol,gates-result,safe-gate-metadata}.json`.

## Parser bundle and complete unit comparison — 2026-11-11

Owner decision 21 is implemented in `68ed776`: tree-sitter-scala 0.26.2 →
0.26.4, tree-sitter-powershell 0.26.4 → 0.26.6 and pulldown-cmark 0.13.5 →
0.13.6 move together. **Accepted after all eight local gates passed on this commit.**
The final lock changes only those three package blocks. Resolution initially
rewrote unrelated bindgen/syn and tempfile/getrandom edges; those rewrites were
restored, with the intermediate lock and exact comparison retained.

**Complete input/output comparison: PASS.** Baseline `7f1c0d3` and candidate
`68ed776` used the same Rust 1.97.1 compiler and snapshot helper, and the same
captured bytes for every production-admitted source in both pinned G2 corpus
roots plus all 23 frozen fixture files. Fresh owned scratch indexing established
admission; neither git-tracked files alone nor answer paths selected the inputs.
The snapshots retain complete ordered syntax forests, effective delivery units,
index documents, imports/calls, coverage and parse-stop/fallback results. Source
identity and build provenance are bound separately. Exact JSONL comparison
ignores no output fields; all three sets are byte-identical, with zero changed
sources and zero stopped parses in either arm.

| Input | Sources | Admitted bytes | Identical snapshot SHA-256 |
| --- | ---: | ---: | --- |
| Rust `b940084d7eb6a299eb4bfeb8e34901bc051e7ac4` | 60,739 | 177,769,746 | `6922b248d5fd38f6fb5857574ca67843279fb5cb552477afaf818061f590a9e1` |
| bun `5b8d5b8a15ab1711597584aadbdc112c55befed1` | 8,436 | 99,926,130 | `c505eccbc9791e0a7fee9a2af7e8f3cddcd667ba390b883b2b04bdc85df54567` |
| Frozen parser fixtures | 23 | 218,058 | `4285ead755493c89c997fe8d6618bf35ef5f2ae62e7a181624f4ae9b48fd87d4` |

Raw and effective units in both G2 corpora are unchanged, so the owner's
conditional fresh 388-question G2 run is **not triggered**. This is no new G2
score, retrieval-quality result or coding-task benefit. It does not establish
equivalence on unmeasured inputs.

**Migration remains explicit.** The selected bundle advances derived search
schema 8 → 9 and `SOURCE_UNIT_SCHEMA` 6 → 7 even though these measured bytes are
unchanged: the dependency/grammar identity changes beyond this finite comparison.
Authoritative store schema 12 and semantic generation schema 6 stay fixed.
Under the [existing repair contract](../specs/001-source-state-recovery/contracts/context-v2.md#parser-bundle--2026-11-11),
old indexes require explicit `repair-index`; authoritative sources, hashes,
revisions, memories and tasks remain. Old semantic partitions/state/generation
metadata remain inspectable history, never current. Explicit `semantic prepare`
refits every source under units7, including zero-unit sources, reusing only exact
cached model inputs. Ordinary open/status does not repair or republish old state.
No original measurement store was opened, repaired or semantically migrated by
the corpus comparison; migration checks use test stores.

**Executed local checks.** The 58 syntax tests and one test each for recipe
eligibility, search repair, retained semantic-state migration and changed-range
refitting all pass: 62 tests, zero failures, in five focused commands on `68ed776`.
Baseline preparation took 323.33 s; candidate build/snapshot/comparison took
210.83 s; focused checks took 54.79 s. These phases perform different work and
are elapsed receipts, not a performance comparison. Each arm's eight native
phases and all five focused commands exited 0 naturally, with no forced cleanup
or observed owned process remaining. No provider/model calls, training or corpus
code execution occurred.

**Clock correction.** The initial shared deadline passed Python 3.9
`time.monotonic()` values across processes on this Mac, where their origins
differed; that shared guard was ineffective for the focused phase. Its local
900-second cap remained effective, and the actual 54.79-second run finished
before the originally selected UTC end, `2026-11-11T17:11:46.981248Z`. Subsequent
candidate/gate admission uses the shared `CLOCK_MONOTONIC` clock, preserving
that original end without an extension. This correction does not retroactively
claim that the earlier shared guard enforced the bound.

**Full local gates: PASS on `68ed776`.** All eight stages exit 0: fmt,
clippy, default tests, pilot clippy/tests, actual Rust 1.90 check/clippy and
Rust 1.90 pilot check. The default suite has 1,307 passed, zero failed and
34 ignored; the custom recovery binary adds 21 passing crash checks, and the
pilot unit suite has 62 passed. The four watch tests finish in 26.97 s.
The complete gate run takes 738.89 s, exits naturally, requests no cleanup signal
and leaves no observed owned process. Both Rust 1.97.1 and 1.90 use SDK 26.5.
The sampled maximum among owned roots/descendants is 2,135,736,320 bytes RSS
(includes compilers), with 1,475 missed process samples; this is not a true peak.
The observed free-space minimum is 276,552,712,192 bytes, above the 20 GiB floor;
no sampled process crosses 8 GiB. These are development gates, not signed-package
acceptance or actual-host pilot qualification.

**Upstream source inspected, not upstream tests executed.** Scala 0.26.4
(`8c0b6d44bfd66bf7bc9a1fc15084ef17cef2f97e`) changes declaration/layout ownership
in `grammar.js::_function_declaration`/`_function_constructor` and scanner
`scan_impl`; its packaged `bindings/rust/lib.rs:53::test_can_load_grammar` was
inspected. PowerShell 0.26.6 (`65ea6919e816a7ac142acec7ee72757f89db8439`)
changes `grammar.js::command_parameter`/`_command_argument` and
`src/scanner.c::scan_bare_dashes`; its corresponding Rust binding test at line 54
was inspected. Neither grammar crate packages its upstream grammar corpus tests.
Cmark 0.13.6 (`014b61d38d235f382935bb676bab872b7680afa2`) changes
`src/parse.rs::handle_wikilink`; `tests/suite/wikilinks.rs::wikilinks_test_15`
was inspected. Foundry's [`markdown_sections`](../src/syntax/markdown.rs) uses
`Parser::new` with wikilinks off. Foundry owns bounded syntax units/import hints
under 001 T008, not upstream editor queries or compiler-proven relationships.
These references explain the selected risk; the measured comparison supplies
the output evidence. No upstream performance or security improvement is claimed.

Private evidence: `runs/parser-bump-2026-11-11/` contains
`baseline-receipts/final-receipt.json`, `candidate-receipts/final-receipt.json`,
their native command receipts, `focused/result.json`, `candidate-clock.json`,
`lock-diff.json`, `lock-after-resolution.toml` and `upstream-inspection/NOTES.md`.
Complete captured inputs and snapshots remain in the owned parser baseline and
candidate `.noindex` directories. The selection/inspection documents' earlier
"not started"/"not run" labels remain preparation history; they are not substitutes
for these later executed receipts or the completed full-gate receipt.

## Encrypted evidence backups — 2026-11-11

Owner decisions 7/30 are complete at the selected local-disk boundary. Five
archives are built under `~/VSC_DEV/backups/foundry-20261111-retry1`, totaling
1,225,156,744 encrypted bytes. The owner selected `~/.ssh/id_ed25519.pub` and
retains its private key; the pinned recipient fingerprint is
`SHA256:nsuaEpeRmr4BvJH1gfJkqkPuoiAz9uaQFmkLlL0rzsQ`. No private key was read.
The owner still handles off-machine copying and authenticated recovery.

| Archive | Source directory under `datasets/` | Ciphertext SHA-256 |
| --- | --- | --- |
| `01.tar.zst.age` | `context-foundry-g2` | `e81cfbcb320b280eacc9d370b9b89d335a8cc8cdd45740f4a5059a6fac78122d` |
| `02.tar.zst.age` | `context-foundry-f4` | `bd7688af71838712d09fe0b87b15ddc3ae86eaf3197f577ad658872085f82ff0` |
| `03.tar.zst.age` | `context-foundry-g2-harness` | `6a7155a79f7873177518361182f40523a2222e8dcd705d6a3878e1a6785c91fc` |
| `04.tar.zst.age` | `context-foundry-citymap` | `bfbdfd33cc9784ba0f6fc83a5af49b47bbd78a6b08f535bcbdf9075b179f7133` |
| `05.tar.zst.age` | `context-foundry-task-validation` | `d9468d818e3ac20bfde7018930d0601317c7fd929c69ecc45ce35bf2e753748e` |

The native-supervised job passed in 20.66 s within the declared 1,800 s budget
and 20 GiB free-disk floor. Each source received a complete non-following
byte/type/mode/symlink manifest. Per-file APFS clones matched those manifests
before and after the archive stream, and the originals matched again at the end.
All 15 BSDtar/zstd/age pipeline exits were zero. There was no plaintext tar at rest;
file-level manifests and source mappings are inside encryption. Ciphertext files
were fsynced, freshly hashed, header-inspected and published without overwriting.
A separate `shasum -a 256 -c SHA256SUMS` readback passed for all five archives and
four recovery sidecars/helpers. The checksum list's own SHA-256 is
`40edae75f9e9b7852e34717391594196224444852c4c84ce74f5bce9f4b4cc32`.
The supervisor recorded natural exit, no forced cleanup and no owned process
remaining. Only this job's staging copies were removed; original data is retained.

The first preflight failed before any dataset read: `diskutil info` was given the
staging subdirectory rather than its containing mount point. Its failed receipt
and empty destination remain. The corrected driver queries the observed APFS
mount point and checks its device against staging, then uses fresh work/output
names. No source, recipient, archive rule or resource bound changed.

Age 1.3.2's official macOS arm64 archive and executable were checksum-verified;
no tool was installed system-wide. Nine synthetic manifest rejection checks and a
small APFS/BSDtar restore check passed, including executable/read-only modes and
symlinks. Those are apparatus checks, not decryption of the real archives.
**Authenticated decryption/restoration and an off-machine copy are unexecuted.**
The backup preserves file bytes, types, modes and symlink targets; ACLs, xattrs,
resource forks, flags, ownership, timestamps, sparse layout and hardlink topology
are not claimed. These are per-file clones with repeated manifest checks, not an
atomic volume snapshot or a live-database consistency proof. No paid calls ran.

The private apparatus, original/corrected protocols, failed/successful native
receipts and complete manifests are retained in `runs/backups-2026-11-11/`.
[Build, checksum and owner-only recovery commands](resume.md#encrypted-backups)
travel with the existing resume instructions. Pilot evidence stays retained until
T006 is accepted, retired or superseded.

## Tantivy 0.27 store compatibility and G1 neutrality — 2026-11-11

Owner decision 22 (2026-11-06, “Apply only if neutral”) selects one dependency
candidate: Tantivy 0.26.2 → 0.27.0. Candidate `292976929836a65ad8aecc5a82f205209903fda2`
changes only Cargo.toml and Cargo.lock; no Foundry API adaptation was required.
Default features remain off; `mmap` and `lz4-compression` remain selected.
No Foundry ranking, parser, corpus, scoring or schema change is included.

**Pins.** The measured baseline is `f5afd2b9a71acdf1956aca9a6da48f7dacfde7f7`
(Tantivy 0.26.2), binary SHA-256
`767910da6960905cc788634297521d043ccb6f294d7a18245126494f866e1389`.
The candidate binary SHA-256 is
`874ee3c79624f6e14e442877ecd6ea1b07dd12e765604353e684138f116729ef`.
The exact 0.27.0 crate archive is
`ad1ebacf6bc124b741ffbfb088f9835cb55e1e2ff57e182e9219e3efa1377b97`;
its declared Rust floor is 1.88. The selection named source commit `fef81c6`;
`f5afd2b` is its documentation-only successor and is the actual frozen baseline.

**Executed on the owner's Mac, with no spend.** Each arm opened its own APFS clone
of the preexisting 0.26.2 store. All six opening/post-query status pairs matched the frozen
ready/complete/zero-pending state; no repair or indexing command ran.
Rust's store had 60,739 sources at `b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`;
oh-my-pi had 8,436 at `5b8d5b8a15ab1711597584aadbdc112c55befed1`.
Full original-store manifests and corpus pins remained unchanged. Original stores
were not opened. Store schema 12, search schema 8, source-unit schema 6 and semantic
generation 6 remain unchanged; this is compatibility at the tested lexical-store
boundary, not a semantic preparation or release acceptance run.

| Full G1 group | Tasks per arm | Response records per arm | Unmasked guards | Paired bytes |
| --- | ---: | ---: | --- | --- |
| bun | 1,000 | 1,029 | PASS in both arms | Identical |
| checker | 6,881 | 10,476 | PASS in both arms | Identical |
| qualified | 2,551 | 2,779 | PASS in both arms | Identical |
| Total | 10,432 | 14,284 | All pass | Identical |

The frozen runner/scorer used SAMPLE 1/1 and one shard; the comparator did not mask
its exit. Complete result JSONL and decompressed response JSONL matched literally,
including identities, arguments, evidence and order; no normalization was used.
Gzip container timestamps were excluded. Nine synthetic comparator checks passed,
including rejection of missing/truncated/duplicate or mismatched records.

The predeclared comparison bound was 3,600 s, with a 20 GiB disk floor and an 8 GiB
sampled Foundry-owner RSS ceiling. The run completed in 1,297.74 s. All 27 native
phase receipts passed with natural exit, no forced cleanup and no owned process
remaining. Peak sampled owner RSS was 567,296,000 bytes; minimum observed free disk
was 293,290,606,592 bytes. Seven owner samples missed processes that had already
exited. These are sampled observations, not true process/GPU peaks or hard resource
limits. No provider/model calls, training or corpus-code execution occurred.
No quality or performance improvement is claimed; G2 was not rerun for this neutral
maintenance candidate.

**Upstream inspection, separately from execution.** Exact Tantivy 0.27.0 source:
`src/lib.rs:240` (`INDEX_FORMAT_VERSION = 7`, oldest supported 4);
`src/compat_tests.rs:42` / `:53` (`test_format_6`, `test_format_7`);
`src/query/const_score_query.rs:42` (`ConstScoreQuery::weight`, child scoring disabled)
and `:192` (`test_const_score_query_disables_child_scoring`). Those upstream tests
were inspected, not executed. Same format numbers were a hypothesis, not the
compatibility proof above. `src/directory/mmap_directory/mod.rs:310–318`
(`ReleaseLockFile::drop`) still relies on descriptor closure; the separate Foundry
inherited-lock containment repair remains in both arms. These references inform
001's existing derived-index owner and decision 22; no upstream ownership or
new source schema is imported.

**Local gates: PASS on `2929769`.** All eight stages exited 0 in 791.38 s:
formatting, Clippy, default tests, pilot-feature Clippy and unit tests, and actual
Rust 1.90 check/Clippy/pilot check. The default suite passed 1,307 tests, with zero
failures and 34 ignored; all 21 custom child-process recovery checks also passed.
The separate pilot unit gate passed 62 tests. All four watcher tests passed in
27.30 s. Rust/Cargo 1.97.1, Rust 1.90.0 and the macOS 26.5 SDK were used on the
owner's M5 Max, with the tracked candidate clean. The 1,800 s gate deadline,
20 GiB disk floor and 8 GiB sampled owner RSS ceiling were respected. The supervisor
recorded natural exit, no forced cleanup and no owned process remaining.

Decision 22 is accepted for the experimental line. This does not accept T010
default promotion or constitute a release, signed-package or semantic-quality result.

Private protocols, binaries, status/identity reports and command receipts remain in
`runs/tantivy-neutral-2026-11-11/`; complete private query outputs and clones remain
in `datasets/context-foundry-tantivy-027-2026-11-11.noindex/`.
Protocol SHA-256: `ab19377fd3270edeabfaac8a68ae19d6777a3e3a1cb01ced9021ef38b2c07ab9`.
Driver SHA-256: `4a7296be6993cfab29fdc92a439df0e6e558dc504fefdf7d32e4bd16896f4ab7`.
Key receipts: `measurement/result.json`, `measurement/{bun,checker,qualified}-identity.json`,
`measurement/commands/`, `binary-{baseline,candidate}.json`, `source-inspection.json`,
`dependency-diff.json`, `comparator-checks.json`, `gates-result.json` and `gates-supervisor/000-full-gates.receipt.json`.
The earlier source-inspection/README “unexecuted” statuses are preparation history;
the measurement/result receipt supplies the later executed result.

## Derived writer contention after fork — 2026-11-11

The retained-cleanup gates on `85369dd` completed all eight stages in 795.88 s:
seven exited 0, but the default test gate exited 101. Its totals were 1,304 passed,
one failed and 34 ignored; the separate pilot unit gate passed all 62 tests.
`policy::the_cli_keeps_output_identical_without_a_policy_and_reports_it` failed
at its first plain CLI context call with Tantivy `LockBusy`, incorrectly reported
as `repair_required`. All observed owned processes exited naturally, without
forced cleanup. This failed run is retained and does not count as acceptance.

The existing `d72f3bb` field-order repair is present: redb ownership outlives the
parent's search-writer teardown. A separate real-process witness identifies a
remaining lifetime mismatch. Tantivy 0.26.2's `ReleaseLockFile::drop`
(`src/directory/mmap_directory/mod.rs`) releases its BSD flock only by closing its
file; its `src/directory/tests.rs::test_lock_non_blocking` and `test_lock_blocking`
cover same-process/thread lock release, not this inherited-descriptor boundary.
Those upstream tests were inspected, not rerun. redb 4.4.0's optimized file backend
explicitly unlocks in `release_all_locks`/`unlock_whole_file`. A child held between
fork and exec retains the inherited Tantivy descriptor. After the parent drops
its Engine, a new Engine acquires redb but reports `repair_required/LockBusy`;
releasing and reaping the child restores `ready` and the original search result,
without repair. The native supervisor recorded natural exit and no remaining
owned child. This establishes the mechanism; the original policy-test failure
did not instrument its fork interval, so that specific occurrence is not causally
attributed beyond the matching lock symptom.

The repair preserves the typed lock error. After acquiring redb ownership, open
retries only the derived writer's `LockBusy`, bounded to one second. Persistent
contention returns `store_busy`; corruption and schema mismatches still require
explicit repair. Competing redb owners are refused immediately. No lock is stolen,
index rebuilt or source record changed. This local containment avoids a dependency
fork or a new process-coordination layer; it does not change Tantivy's lock lifetime.

Both new tests in `tests/store_lock.rs` fail before the repair and pass after it
(2 passed in 1.23 s). The first uses real forked descriptors and releases the child
only after the actual lock attempt fails; the second holds a real Tantivy writer,
requires bounded `store_busy`, unchanged index metadata and a ready reopen with
the original source revision/result. A native focused run passed all 88 core,
22 policy and two lock tests in 87.82 s, including the original failing policy case;
it exited naturally with no observed owned processes left. In-session review added
a deadline check before starting any retry, so an expired wait cannot start another
reader construction; both lock regressions passed again after that correction.

**Fresh full gates: PASS on `fef81c64f75b41ce5379174d82fa1b3e4888b461`.** All eight
stages exited 0 in 741.87 s: formatting, Clippy, default tests, pilot-feature Clippy
and unit tests, and Rust 1.90 check/Clippy/pilot check. The default suite passed
1,307 tests with zero failures and 34 ignored; all 21 named child-process recovery
checks also passed. The separate pilot unit gate passed 62 tests. The watcher file
passed all four tests in 26.52 s. This used Rust/Cargo 1.97.1, actual Rust 1.90.0
binaries and the macOS 26.5 SDK on the owner's M5 Max. The tracked tree was clean.
The 1,800 s outer deadline, 20 GiB disk floor and 8 GiB sampled Foundry-owner RSS
ceiling were unchanged. The supervisor exited naturally, without forced cleanup,
and found no owned processes remaining. No paid provider, training or hosted CI ran.

This accepts the retained semantic isolation, supervisor repair and store-lock
containment at their tested boundaries. It does **not** qualify T010's rejected
default-promotion candidate. Private evidence, including the pre-repair executable,
red/green logs, final gate receipt and a 340-file SHA-256 evidence snapshot, is in
`runs/store-lock-2026-11-11/`; the original
gate logs and receipt remain in `runs/t010-lexical-2026-11-11/gates-retained*`.

## Pilot startup-network source feasibility — 2026-11-11

**Source feasibility finding; startup-network qualification remains open.** Owning
requirement: 003 T006's [existing v2 startup row](../specs/003-agent-retrieval-context/contracts/bounded-pilot-v2.md#required-v1-gap-repairs-and-qualification).
Capture denied startup attempts with destination/classification before any paid
request and disallow unobserved routes. This in-session source inspection neither
implements that outcome nor establishes runtime coverage or OS-denial causality.

The official [v18.13.2 release metadata](https://api.github.com/repos/can1357/oh-my-pi/releases/tags/v18.13.2)
associates `omp-darwin-arm64` asset `682434495` (217,260,176 bytes) with
[asset SHA-256](https://api.github.com/repos/can1357/oh-my-pi/releases/assets/682434495)
`38c3645283ceba06f66462830ae15f27270cc7f84575e40a4c6bd68452a63d0f`, exactly the frozen host pin.
The [tag reference](https://api.github.com/repos/can1357/oh-my-pi/git/ref/tags/v18.13.2)
resolves to immutable source commit `e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e`;
all source/test links below use that commit. Release metadata reports `immutable:false`.
This establishes release association, not reproducible-build/source-to-artifact
correspondence. No artifact was downloaded/rehashed, built or executed in this inspection.

| Inspected source owner | Finding and implication |
| --- | --- |
| [`ModelRegistry` constructor](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/coding-agent/src/config/model-registry.ts#L518), [trusted-extension load](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/coding-agent/src/main.ts#L2450) | Private fetch is captured before the extension factory; a later global-fetch wrapper cannot cover it. |
| [`prewarmModelConnection`](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/ai/src/providers/connection-prewarm.ts#L28), [`sendCoworkRequest`](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/ai/src/providers/cowork-fetch.ts#L185), [SDK startup](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/coding-agent/src/sdk.ts#L2177) | Official Anthropic prewarm can use direct Node HTTPS; startup discards its promise. The inference relay and registry await do not cover it. |
| [`ExtensionAPI.pi`](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/coding-agent/src/extensibility/extensions/types.ts#L1720) / [`registerLogSink`](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/utils/src/logger.ts#L28), [`awaitBackgroundRefresh`](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/coding-agent/src/config/model-registry.ts#L615) | Useful partial diagnostics/current registry settlement; neither supplies every attempt, prior events or a complete startup barrier. |
| [Discovery fallback](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/coding-agent/src/config/model-discovery.ts#L604), [silent metadata](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/catalog/src/provider-models/openai-compat.ts#L4378), [timeout wrapper](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/coding-agent/src/config/model-discovery.ts#L79) | Provider warnings can omit fallback/metadata destinations; wrapper timeout can settle while fetch ignores abort. Quiet logs or registry settlement cannot certify a complete inventory. |

Relevant upstream tests were **read, not run**: [prewarm injected-fetch HEAD](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/ai/test/connection-prewarm.test.ts#L50),
[awaited/coalesced discovery](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/coding-agent/test/model-discovery.test.ts#L239),
[8080→9931 fallback](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/coding-agent/test/model-discovery.test.ts#L1635)
and [fetch ignoring abort](https://github.com/can1357/oh-my-pi/blob/e996e1c7ac476f60e02b8ed9764cf30fa8c00a8e/packages/coding-agent/test/model-discovery.test.ts#L1764).
These test sources support the individual behavior boundaries; they do not prove a
compiled host exposes the injected logger singleton or complete startup observation.

Foundry's [bridge Ready](../src/pilot_capture/bridge.ts), inference-only [`FetchBegin`](../src/pilot_capture/protocol.rs)
and [`Host::launch` sandbox policy](../src/pilot_capture/host.rs) do
not supply that guarantee. Preserve the frozen inference routes and reuse the
owned bridge/channel and durable journal if a complete early host seam is built.
Concrete missing seam: inject observation/refusal before `ModelRegistry` captures
fetch, retain/await prewarm and every other allowed startup owner, then acknowledge
the complete durable observation prefix before count/messages admission. Prove
non-bypass coverage and failure/cleanup at the actual host boundary; a logger sink,
late fetch hook or quiet timer alone is insufficient. No host re-pin is selected.

Private `runs/pilot-private-permissions-2026-11-11/startup-source-plan.md` and
`startup-local-interface-plan.md` retain module/blob identities and Foundry interface pointers;
no host launch, test, provider call, private-input read or runtime acceptance is
claimed here. Possible source destinations are not a measured inventory. Host
ENOTFOUND/EPERM text is not independent OS-denial proof. This is a narrow source
finding, not a repository-wide absence claim; T006 and pilot v2 remain unaccepted.

## Pilot checker provenance and retained diagnostics — 2026-11-11

**PASS at the selected local checker boundary.** Pilot v2/T006 remain unfunded and unaccepted.
Production `351f638dce5cc2aa245653ad969df2357b6eed87` follows `ac0850cba7409bdb13914461a07ac645c1266bad`;
final gate candidate `1f434cae92bd48e60f5dad0b235b8a3ca637e59f` changes only the declared test cleanup conditional.
Exact frozen policy/pins modules and v1 contract are unchanged. Required metadata
`bounded-pilot-v1-check-v2` binds exact sanitized environment/SBPL, fresh executable observations
and complete private stderr files/lengths/digests. Legacy fields receive no defaults. Capped partial
capture faults halt; complete nonzero checks remain ordinary Fail. One deadline covers process/readers.

| Original focused command | Passed / failed / ignored | Actual / expected exit | Seconds | Evidence |
| --- | ---: | ---: | ---: | --- |
| `green-actual-producer` | 1/0/0 | 0/0 | 4.257 | green |
| `green-actual-producer-final` | 1/0/0 | 0/0 | 4.123 | green |
| `green-full-host-suite` | 22/8/2 | 101/0 | 221.768 | retained failure |
| `green-full-host-suite-retry` | 30/0/2 | 0/0 | 250.458 | green |
| `green-unit` | 76/1/0 | 101/0 | 35.005 | retained failure |
| `green-unit-final` | 77/0/0 | 0/0 | 34.359 | green |
| `green-unit-retry` | 77/0/0 | 0/0 | 34.999 | green |
| `red-compiler` | 0/1/0 | 101/101 | 30.388 | expected red |
| `red-environment` | 0/1/0 | 101/101 | 31.871 | expected red |
| `red-kind` | 0/1/0 | 101/101 | 31.563 | expected red |
| `red-oracle-executable` | 0/1/0 | 101/101 | 31.821 | expected red |
| `red-pipe-deadline` | 0/1/0 | 101/101 | 35.320 | expected red |
| `red-producer-environment` | 0/1/0 | 101/101 | 17.787 | expected red |
| `red-profile` | 0/1/0 | 101/101 | 33.822 | expected red |
| `red-special-mode` | 0/1/0 | 101/101 | 30.699 | expected red |
| `red-stderr-cap` | 0/1/0 | 101/101 | 31.218 | expected red |
| `red-stderr-file` | 0/1/0 | 101/101 | 31.087 | expected red |
| `red-stderr-read` | 0/1/0 | 101/101 | 31.284 | expected red |

Initial unit failure passed deadline/group cleanup but retained 15-byte stderr with a complete
filename after cleanup EOF; stopped-group output is now conservatively partial, with assertions unchanged.
Initial host failures reached unadapted fixture roots (0755 versus 0700), before intended assertions;
eight fresh-directory sites now establish 0700, with production/assertions unchanged. Both failures remain.
All eleven selected reds reach assertions, restore exact source and precede final greens. The pipe
control gets err() None before its later two-second assertion; producer-environment completes four
zero-exit steps then rejects the emptied receipt map, before the negative phase. Neither proves a
child-environment syscall trace or compiler replacement. Expected-red success is not acceptance.
Consumer/file, owned-process and actual compiler/oracle evidence remain distinct. The producer uses
fresh owned five-file copies; final readback retains 34 regular private files. Four positive steps exit zero;
one new compile-error step exits 1 without a checker. Complete stderr: 0/0/457/0 positive / 230
negative bytes, all 0600, with the public negative marker. Exact environment/profile/digest metadata matches.
Private diagnostic inspection found no unexpected error headers; raw diagnostics remain private.
Inputs retain original candidate bytes/mode/mtime/inode, oracle/historical stdout bytes, owned bytes/modes
and compiler pin. Original oracle mode was not compared; no new stdout length is claimed. The full host
file ignores actual-producer/private-full-pipeline tests; actual producer is separately selected. No calibration rerun.

Original gates on `351f638` FAILED: only pilot-clippy exit 101 (two collapsible_if findings in
test cleanup); seven exits zero. Tests 1,311/0/34, 21 recovery checks, 77 pilot pass. Gates 619.161 s;
original stage closed failed at 1941.598/2,400 s, with no extension. A separate 1,800-second
lint continuation preserves exact reverse-replacement equality, production prefix, other three source files
and all assertions. No new focused run was required; the fresh pilot-unit gate exercises cleanup again.
Fresh eight gates PASS in 638.859 s; default 1311/0/34, pilot 77/0/0
passed/failed/ignored plus 21 recovery checks. Continuation shared time 755.163/1,800 s.
Both stages retain 120-second cleanup reserves, one heavy job and $0 spend; Rust/Cargo 1.97.1 and
actual 1.90.0 use SDK 26.5. Every observed command exits naturally, without forced cleanup or owned survivors.
Focused sampled individual RSS max 2,146,254,848 B, free min 268,272,418,816 B,
109 misses. Original gates: RSS 1,407,909,888 B, free 265,131,134,976 B, 1415 misses;
continuation: RSS 2,114,453,504 B, free 265,360,961,536 B, 1387 misses. Bounds held; not true peaks.

Private owner: `runs/pilot-checker-provenance-2026-11-11/`, including immutable original pins/results,
safe command/readback/control metadata, both corrections and `lint-continuation/` protocol/equivalence/gates.
Prehash/exec remains non-atomic; dynamic libraries/sysroot, compiler-version preflight bounds,
broader permissions and startup-network capture remain open.
No current-host/provider compatibility, private full-pipeline/agent/economics or pilot-v2/T006 acceptance follows.

## Pilot checker completion and artifact binding — 2026-11-11

**PASS at the selected local checker boundary.** Pilot v2/T006 remain unfunded and unaccepted.
Candidate `85479736a55c3b6982e88adb2a4b8a013a062b5f` follows `13c0a63e1e8282986d50444632e13ffc27a6b9db`; frozen v1 policy/history are unchanged.
The existing consumer requires the pinned completion digest and exact compile/executed-image
lineage. Complete observed nonzero results remain ordinary Fail; incomplete provenance halts.
The producer hashes the exact copied oracle bytes, refuses read-error-as-EOF capture, and routes
capture/receipt-write/sealing errors through the existing halt, keeping later slots unstarted.

| Retained focused command | Passed / failed / ignored | Actual / expected exit | Seconds | Disposition |
| --- | ---: | ---: | ---: | --- |
| `green-actual-producer` | 1 / 0 / 0 | 0 / 0 | 4.108 | green pass |
| `green-consumer` | 4 / 0 / 1 | 0 / 0 | 66.552 | green pass |
| `green-full-host-suite` | 29 / 0 / 2 | 0 / 0 | 242.703 | green pass |
| `green-helpers` | no test summary | 101 / 0 | 9.798 | retained other outcome; inspect metadata/log |
| `green-helpers-final` | 2 / 0 / 0 | 0 / 0 | 31.625 | green pass |
| `green-helpers-retry` | 2 / 0 / 0 | 0 / 0 | 31.787 | green pass |
| `red-actual-producer` | 0 / 1 / 0 | 101 / 101 | 1.346 | expected-red failure witness |
| `red-check-halt` | 0 / 1 / 0 | 101 / 101 | 32.517 | expected-red failure witness |
| `red-consumer` | 1 / 3 / 1 | 101 / 101 | 44.974 | expected-red failure witness |
| `red-pipe` | 0 / 1 / 0 | 101 / 101 | 31.849 | expected-red failure witness |

Expected-red `passed:true` matches a failure witness, not implementation acceptance. Actual
assertion metadata and exact restored-source controls remain with each command. Tests run on
retained working-tree patches; all eight gates below bind the final committed candidate.
The first `green-helpers` command failed compilation before tests: its unqualified `json!`
was corrected to `serde_json::json!` in test code only; production was unchanged and the failure is retained.
The pipe red accepts a prefix hash; the halt red fails on None versus check_output_unbound at its
first producer-error case, before its receipt-write branch. Final green exercises both helper branches.
Synthetic consumer receipts use inert artifact identities and do not execute the private oracle.
The actual producer executes the fixed local compiler/oracle on new owned copies of the five
SHA256SUMS-verified files and fresh outputs. It uses no host/model/provider or original calibration harness.
Its baseline red completes four zero-exit steps and checks completion before finding no oracle_run
executed-image artifact. Green separately passes the positive chain and a new owned compile-error case.
That negative producer phase is an observed nonzero receipt; ordinary Fail consumption is tested separately.
Readback retains 29 private green artifacts and proves five original files unchanged in bytes/mode/
mtime/inode, owned input bytes/mode 0600 and compiler pin unchanged. Original oracle and historical
stdout bytes are unchanged; original oracle mode was not compared.
The serial synthetic-host file is distinct; both the private full pipeline and actual-producer test
remain ignored there. Actual producer execution is only its separately selected ignored-test command.

**All eight gates pass** in 616.327 s; default tests 1,311/0/34 passed/failed/ignored,
21 custom recovery checks pass separately, and pilot units 66/0/0. Rust/Cargo 1.97.1 and
actual Rust/Cargo 1.90.0 use macOS SDK 26.5. Every retained command exits naturally, without
forced cleanup or observed owned processes remaining. Shared elapsed: 1529.531/2,400 s;
no extension, 120-second cleanup reserve and spend $0. Each producer recipe retains its 60-second bound.
Focused sampled individual RSS maximum 2,165,719,040 B; minimum free
270,814,244,864 B; 74 missed samples. Gates: 1,401,208,832 B RSS,
267,521,388,544 B free, 1,381 misses. Bounds held; these are not true peaks or aggregate/GPU memory.

Expected successful stdout SHA-256: `aac03c0ded8403eb8bd3ca53e9880313cde11fa658db4fc7444d737eef33127d`.
Its 106-byte length is historical input metadata; the new receipt records a digest, not a byte counter.
Oracle SHA-256: `f5ac14d370d852090c49ead61e23f4d9185eed491b33b123ba6e6ff2dec09bb9`.
Private owners: `runs/pilot-oracle-completion-2026-11-11/` protocol, owned-input/commit pins,
all focused receipts/control metadata, `producer-red-readback.json`, `producer-readback.json`,
`input-final-readback.json`, `helper-compile-correction.json`,
`gates-result.json` and gate logs. Raw private source/oracle/output bytes stay outside Git.

No current installed-host/provider compatibility, private full-pipeline/agent/economics acceptance,
power-loss durability or retrieval gain is claimed. Environment/sandbox/compiler receipt expansion,
retained stderr bytes and permissions remain separate gaps; broader v2 freeze requirements stay open.

## Pilot authority and run-path binding — 2026-11-11

**PASS at the selected local no-provider boundary.** Pilot v2/T006 remain unfunded and unaccepted.
Authority code `87fe845a3e4612996f14370a2dfc6e2a8732cb2b` follows base `bd9ecbf19f6ca529aa51d17c2fa923e88a76def0`.
Test-only continuation `4c659f72f3d5bb9ade51e4502dc9fe986d0dc79b` changes only `tests/store_lock.rs`; the four authority-source hashes remain exact.
Launch metadata `bounded-pilot-v1-launch-v2` requires canonical `run_dir` and explicit
`manifests --run-dir`; the unchanged authority shape binds that root through the manifest digest.
Existing synced `admitted`/`coding_handoff` records bind the digest of validated authority bytes, scope
and actual root before a runnable session returns; Verification records null authority.
Legacy unbound metadata and unbound Live probe history refuse without rewriting historical evidence.

| Retained command | Passed / failed / ignored | Actual / expected exit | Seconds | Disposition |
| --- | ---: | ---: | ---: | --- |
| `green-authority` | 9 / 0 / 0 | 0 / 0 | 96.354 | green pass |
| `green-chain-final` | 2 / 0 / 0 | 0 / 0 | 34.223 | green pass |
| `green-full-host-suite` | 25 / 0 / 1 | 0 / 0 | 195.484 | green pass |
| `green-restore-cli` | 2 / 0 / 0 | 0 / 0 | 13.949 | green pass |
| `green-seal-binding` | 1 / 0 / 0 | 0 / 0 | 18.535 | green pass |
| `red-baseline-authority` | 2 / 7 / 0 | 101 / 101 | 80.778 | expected-red failure witness |
| `red-coding-chain` | 0 / 1 / 0 | 101 / 101 | 30.368 | expected-red failure witness |
| `red-probe-chain` | 0 / 1 / 0 | 101 / 101 | 21.798 | expected-red failure witness |
| `red-seal-binding` | 0 / 1 / 0 | 101 / 101 | 18.174 | expected-red failure witness |
| `fixture-continuation/green-store-lock` | 2 / 0 / 0 | 0 / 0 | 4.442 | green pass |
| `fixture-continuation/mechanism-witness` | 1 / 0 / 0 | 0 / 0 | 6.226 | green pass |

Expected-red `passed:true` only means the declared failure occurred. Actual panic/assertion
excerpts and every command identity/lifecycle are retained in the private collection and logs.
Baseline chain-positive tests stopped at new-format rejection; those are not causal chain witnesses.
`red-probe-chain` and `red-coding-chain` omit only their producer fields, retain format/path
validation, and restore all four selected source hashes. Their exact failing assertions remain recorded.
The probe control fails on null admitted authority; the coding control reaches handoff and fails
on null run path. Its later authority assertion was not reached, so it is not an authority-specific red.
The seal-binding red stops at the first missing-authority case; it does not independently witness
all later invalid-binding branches. The unchanged final green tests exercise their full assertions.
Focused runs retain their actual working-tree HEAD/patch identity; gate receipts bind their respective commits.
The ignored private-evidence pipeline was adapted before execution (`fixture-correction.txt`),
including its earlier manifest-bound path refusal; it was never run or qualified.

**First gates FAILED on `87fe845`:** seven exits 0, default test 101. The store-lock test failed
at direct Tantivy writer setup (`tests/store_lock.rs:120`, `LockBusy`), before `Engine::open_existing`.
Default totals: 1,310/1/34 passed/failed/ignored; 21 recovery checks and 64 pilot units pass.
It took 634.366 s; original shared stage 1354.647/1,800 s concluded failed without extension.
Its sampled individual RSS was 2,079,424,512 B, minimum free 267,518,402,560 B, with 1,413 missed samples.
The separate two-store witness shows held-child `LockBusy` becoming writer-acquired after reap,
while the other owner stays ready; it demonstrates a mechanism, not the original concurrent schedule.
The continuation adds one shared Mutex as the first local in both tests. Strip-equality confirms
unchanged original bodies/assertions and removal of the temporary witness; product lock policy is unchanged.
This applies the [prior descriptor-lifetime finding](#derived-writer-contention-after-fork--2026-11-11) to fixture isolation (Tantivy 0.27 `directory::mmap_directory::ReleaseLockFile::drop`).

**Fresh continuation gates pass** in 584.887 s: fmt, Clippy, default tests, pilot Clippy/tests,
actual Rust 1.90 check/Clippy and pilot check. Rust/Cargo 1.97.1 and 1.90.0 use macOS SDK 26.5.
Default totals: 1,311 passed, 0 failed, 34 ignored; 21 custom recovery checks pass separately.
Pilot unit totals: 64 passed, 0 failed, 0 ignored. All retained commands exit naturally,
with no forced cleanup or observed owned process remaining. The separate continuation completes in
724.857 s of its own 1,800 s, with no extension and spend $0; the original failure remains unchanged.
Focused sampled individual RSS maximum: 2,110,259,200 B;
minimum free disk: 271,239,192,576 B; missed memory samples: 76.
Gates: 859,127,808 B sampled individual RSS, 267,593,863,168 B minimum free,
1,338 missed samples. The 8 GiB process/20 GiB disk bounds held; these are not true peaks or aggregate/GPU memory.

Private evidence: `runs/pilot-authority-path-2026-11-11/`: `protocol.json`, `commit-pin.json`,
`candidate-hashes.json`, all `focused/*/result.json` with named stdout/stderr/native receipts,
`red-*-chain-control.json`, `fixture-correction.txt`, the failed `gates-result.json` and `stage-disposition.json`.
Fresh continuation owners are `fixture-continuation/{protocol,commit-pin,gates-result,fixture-isolation-check}.json`, its focused receipts and gates logs.
Both pins/receipts bind unchanged authority sources, frozen policy/contract and host; the original failed receipt is hash-preserved.
Archived host SHA-256: `38c3645283ceba06f66462830ae15f27270cc7f84575e40a4c6bd68452a63d0f`.

This is metadata revision 2 under frozen v1 policy, not new pilot authorization. No current installed
host or provider compatibility, actual Live-compatible probe history, private oracle outcomes,
power-loss/hostile-filesystem campaign, retrieval gain or real-agent/economics benefit is claimed.
Other apparatus and pilot task/sample/arm/launch prerequisites retain their existing status.

## Pilot workspace restoration and halt persistence — 2026-11-11

**PASS at the selected local no-provider repair boundary.** Focused red/green,
serial pinned-host checks and all eight local gates pass. This does not accept
pilot v2 or 003 T006. The selected combined repair
starts from `ff75a5fdb74b8b77a06637cc42919fa8a52f7ec5`, after the separate accepted
[credential-refusal repair](#pilot-credential-refusal-durability--2026-11-11).
The completed focused/host checks below ran on retained working-tree patches over
that base. The exact combined sources are now committed as
`e320614f2aca7376f481d140b40d6dc78f73a79f`; all eight gates pass on that
commit. The retained working-patch checks and committed gate run are distinct
evidence; private source preparation is not substituted for either.
003 T006 and pilot v2 remain unfunded, unfrozen and unaccepted.

**Selected repair.** Workspace restoration failures in the existing
`apparatus::run_probe` and `run_coding` enter the same monotonic session halt that
blocks subsequent slots. With healthy journal persistence, the existing finalizer
retains the terminal halt and `not_started` rows. Probe's post-admission CLI function
finishes the failed session, prints its existing receipt and returns exit 2. A
halted coding run retains its descriptive report and returns exit 1. No next host
is launched at the failed restore boundary; prior slot cleanup continues to use
the existing owner. The focused tests below exercise these selected behaviors.

The original private restore patch alone could hide a halt append failure:
`Inner::enter_halt` discarded that persistence error, and a later successful
`finish` could append `finished` and return a receipt. The selected amendment
retains the first append error in the existing `Inner` owner. `finish` drops its
transport and returns that error before any new finished record or success receipt,
even if the journal becomes writable again. The original causal halt still blocks
admission. If capture persistence fails, partial existing evidence remains; the
repair does not manufacture completed rows or a durable success report. The CLI
reports the persistence refusal. There is no new journal/schema, retry, recovery
policy or process manager.

**Written selection and bounds.** The selected source packet is
`runs/pilot-v2-2026-11-11/restore-halt-draft/journal-amendment/`; its combined patch
supersedes the original private restore-only candidate. Both packets and their
static preparation history remain retained. Their README/pins use source base
`7f5ba75`; the later execution protocol names the accepted record head `ff75a5f`
and pins the actual source bytes. Preparation statements such as “not executed”
remain historical and do not substitute for runtime results.

`runs/pilot-restore-halt-2026-11-11/protocol.json` declares one heavy job, spend $0,
one shared 1,800-second stage with 120 seconds reserved for cleanup, a 20 GiB
free-disk floor and 8 GiB sampled per-owned-process RSS bound. Focused commands are
capped at 180 seconds and the full host file at 600 seconds, further constrained
by the remaining shared deadline. All eight gates must run on the final committed
candidate within the declared allowance; an incomplete run remains incomplete.

The real-file unit test constructs the existing session/journal over private
temporary files with no transport, host, credential or live admission. Its healthy
control requires `admitted`, `halt`, `finished`, an unstarted probe row and matching
receipt digest. Its negative control fails the halt append, clears that injection
so a later append could succeed, and requires finalization to return
`Persistence/journal_append_failed` without changing the journal. It also checks
that a later fault cannot replace the original halt. This is deterministic
real-owner/file evidence, not a power-loss, partial-write or fsync-failure campaign.

The binary tests call the exact post-admission CLI functions using Verification
mode and owned synthetic TLS fixtures. They reuse the prior run's read-only archived
OMP artifact through compile-time `CF_PILOT_TEST_OMP`, with required SHA-256
`38c3645283ceba06f66462830ae15f27270cc7f84575e40a4c6bd68452a63d0f`.
The installed host remains untouched. The probe cases select empty seed inventory
and changed seed bytes; the coding case performs a local fixture probe/handoff,
then fails restoration before unavailable downstream inputs or any oracle are used.
It must retain the original probe counters, journal prefix and fixture-request count,
with every coding row unstarted and no correctness result or completed pair.
These do not qualify process-argument/live-front-door admission, actual-provider
compatibility, private evidence, oracle execution or the current installed host.

**Completed focused receipts.** All seven commands record base HEAD `ff75a5f` with
their retained working-tree patches. Expected-red exit 101 is a failure witness;
the result's `passed:true` means it matched that expected failure, not that the
implementation passed. Times below are command-receipt elapsed time, including
setup/build overhead, rather than just the Rust test duration.

| Command under `focused/` | Tests passed / failed / ignored | Exit | Elapsed seconds |
| --- | ---: | ---: | ---: |
| `red-restore-cli` | 0 / 2 / 0 | 101, expected | 25.436 |
| `red-halt-persistence` | 0 / 1 / 0 | 101, expected | 32.901 |
| `green-halt-persistence` | 1 / 0 / 0 | 0 | 32.271 |
| `green-restore-cli` | 2 / 0 / 0 | 0 | 14.163 |
| `red-cli-only` | 0 / 2 / 0 | 101, expected | 11.118 |
| `green-restore-cli-final` | 2 / 0 / 0 | 0 | 14.629 |
| `green-full-host-suite` | 17 / 0 / 1 | 0 | 142.952 |

The original CLI red run found an absent probe halt and coding exit 2 instead of
the required retained-report exit 1. The halt-persistence counterfactual failed at
`unwrap_err()`: after the halt append failed and injection was cleared, `finish`
returned a successful `CaptureReceipt` with two journal records. The setup's
`admitted` and later `finished` record omitted the terminal halt. The restored
amendment passes the unchanged real-file unit: a later writable journal does not
erase the retained persistence error or permit a new success receipt.

The first green CLI run passes both restoration cases. The separate CLI-only
counterfactual keeps repaired halt/apparatus behavior but restores the old CLI
handling. It fails both unchanged tests: coding returns exit 0 instead of 1, and
the probe journal's last record is `halt` instead of `finished`. All three selected
production source hashes were restored before the final green CLI run, which
passes both tests again. These controls separately exercise session halting,
truthful persistence finalization and CLI failure classification.

The complete serial pinned-host fixture file passed with 17 tests, no failures
and one ignored test in 142.951550541 command seconds. It uses the exact archived
pin and `--test-threads=1`. The ignored
`full_pilot_pipeline_qualifies_without_provider` requires private evidence roots;
that evidence-backed pipeline was not run or qualified here. This serial elapsed
time is a run receipt, not a performance comparison with a prior parallel run.

All seven completed commands exit naturally, with no forced cleanup or observed
owned process remaining. Their largest sampled individual owned-process RSS is
2,138,996,736 bytes; minimum sampled free disk is 272,238,710,784 bytes; 34 owned-
process memory samples were missed. The 8 GiB process bound and 20 GiB free-disk
floor hold for these commands. These are sampled observations, not true peaks,
aggregate process memory or GPU memory. Every completed result records spend $0.

**Full gates: PASS on `e320614`.** All eight named stages exit 0 in 604.639
seconds: formatting, Clippy, default tests, pilot-feature Clippy and unit
tests, actual Rust 1.90 check/Clippy and the Rust 1.90 pilot check. The default
suite reports 1,311 passed, 0 failed and 34 ignored; 21 custom recovery
checks pass separately. The pilot unit gate reports 63 passed, 0 failed and 0
ignored. Rust/Cargo 1.97.1 and actual Rust/Cargo 1.90.0 use macOS SDK 26.5.
The opt-in binary regressions retain their separate focused evidence.

The gate command exits naturally, with no forced cleanup or observed owned
process remaining. Its largest sampled individual owned-process RSS is
1,396,670,464 bytes, with 1,360 missed owned-process memory samples;
minimum free disk is 268,764,352,512 bytes. These are sampled process
measurements, not true peaks, aggregate memory or GPU memory. The recorded
resource bounds held. The completed shared stage occupied 1241.485 seconds of
the original 1,800-second bound, with no extension. The result records unchanged
source, frozen v1 contract and pinned host; the commit pin retains the unchanged
policy prefix. Spend is $0. All red/counterfactual failures remain retained.
The final owners are `gates-result.json`, `gates/` and
`gates-supervisor/000-full-gates.receipt.json` in the private run directory.

The existing healthy-probe finish-error classification and report-write failure
classification remain outside this selected restore/halt-capture repair, as the
private packet states. These checks do not qualify every capture-persistence
failure, arbitrary journal corruption or recovery policy.

Private evidence owner: `runs/pilot-restore-halt-2026-11-11/`, including each
`focused/{red-restore-cli,red-halt-persistence,green-halt-persistence,green-restore-cli,red-cli-only,green-restore-cli-final,green-full-host-suite}/result.json`
and its named command receipt/stdout/stderr, plus the retained command patches.
`commit-pin.json` binds the exact final source hashes and records
`policy_prefix_unchanged:true`; `gates-protocol.json` names the committed gate run.
Selected combined
patch SHA-256:
`4384cb21562235b36ab23b44853129ab6bb192bec6f1f592bc73cf947dbbc9a2`.
The protocol and completed commit pin agree on the final source SHA-256 values:

- `src/pilot_capture.rs`: `b8395f43f3a4fd3662902eea171229c2e5eeb501a5933d4699adadcb2d0e0737`.
- `src/pilot_capture/apparatus.rs`: `ea2ba430bad448d2f9b12f2a96a37313f549c9e8a0a20a6092519563952ab058`.
- `src/bin/foundry_pilot_capture.rs`: `b063d09cf4e63f62f5f69a9f9bcd033df695ebf4252be2c82a196ecc9160ab95`.

The commit pin verifies these exact selected sources in `e320614` and the unchanged
policy prefix. The frozen v1 contract's recorded SHA-256 is
`1a00758ffdafbfa18a59dd69e0b9912ef8beca9f7010f375bab8e553c554502d`.
No corpus, owner credential/private key or actual provider is selected. Other v1
gap rows, harder held-out tasks, sample/power analysis, arm readiness and costed
owner authorization retain their existing status.

## Pilot credential-refusal durability — 2026-11-11

**PASS at the selected local repair boundary.** Focused red/green checks, the
pinned-host file and all eight gates pass. This is one no-provider
repair of the disclosed credential-after-claim failure path, based on
`031989862defcb4602cd32fb7567cd072a9af39a`. It does not accept 003 T006, freeze or
fund pilot v2, certify the current installed host, or repair the other v1 gaps.
The repaired source/tests are now committed as
`7f5ba75c9810a18f1acbc741afb5642cf5c869fc`; all eight gates passed on that commit.
T010's preceding publication is recorded separately in private
`runs/t010-singleton-2026-11-11/publication.json`; its acceptance is unchanged.

**Preflight refusal, retained.** The installed `/Users/satishlomte/.local/bin/omp`
was refused before testing: SHA-256
`ce80099f10c5ea4367523767230cb5b920f30cc139b9cf8758c1cc4b2f1f9409`
did not match the required frozen host
`38c3645283ceba06f66462830ae15f27270cc7f84575e40a4c6bd68452a63d0f`.
The shell continued after the failed preflight assertion and applied only the test
patch; that patch was retained and exactly reversed without executing tests or the
host. No production patch was applied in that refused attempt. Its receipt records
zero provider requests and spend $0; it is not a successful test run.

The exact pinned artifact was then found at
`~/.local/bin/omp.1794156108095.11575.0.bak` (217,260,176 bytes) and copied into the
owned run's `bin/omp`. The integration test now accepts the compile-time
`CF_PILOT_TEST_OMP` path override while retaining its installed-path default and
strict frozen SHA-256 check. This selects an exact archived artifact for the local
fixture tests; it does not install or certify the current installed host. Frozen v1 policy and
contract bytes are unchanged.

**Qualified implementation.** Existing `CaptureSession::open`
and `open_after_probe` create or reopen the existing journal after branch admission
and before admitted credential loading. A refusal after the durable claim appends
`halt` with `phase:credential_admission`, the fault class and stable code, never the
credential. The consumed claim remains consumed. The existing journal's append
failure is returned as a persistence fault rather than claiming a durable refusal.
The selected handoff ordering admits the branch before journal access, verifies
Live's cited journal before consuming fresh authority, and permits appends only
after the claim. The journal format/chain and existing session owners are reused.

The revised private source plan is
`runs/pilot-v2-2026-11-11/credential-refusal-ordering-draft/`; the earlier ordering
draft remains at its sibling `credential-refusal-draft/`. Their revision history
is source preparation, not runtime evidence. The earlier fabricated cross-mode
Live case establishes mode refusal only. The subsequently declared
`additional-live-guard.json` extends the existing Live admission test with a
valid-shaped synthetic coding manifest, authority and cited seal pointing at an
owned terminal journal. It retains the original deadline and repair rule. Its
local branch-ordering evidence is recorded below; no actual Live-compatible probe
history, provider compatibility or owner credential is supplied by this fixture.

**Declared run.** `runs/pilot-credential-refusal-2026-11-11/protocol.json` freezes
one heavy job at a time, a shared 1,800-second stage with 120 seconds reserved for
cleanup, 20 GiB free-disk floor, an 8 GiB sampled per-owned-process RSS bound and
spend $0. Only owned synthetic credentials and local TLS fixtures are selected.
No owner credential/private-key file is read and no actual provider call is
selected. A local fixture request is not a provider call; refusal tests must show
no additional fixture transmission across the refusal boundary.

**Completed focused evidence.** All nine commands ran on retained working-tree
patches over `031989862defcb4602cd32fb7567cd072a9af39a`; the repair was uncommitted
when they ran. Their command records bind `CF_PILOT_TEST_OMP` to the owned archived
artifact. Expected-red exit 101 is a failure witness, not an implementation pass;
the step result's `passed:true` means that its expected failure was observed.

| Command under `focused/` | Tests passed / failed / ignored | Exit | Receipt elapsed seconds |
| --- | ---: | ---: | ---: |
| `red-credential-refusal` | 0 / 2 / 0 | 101, expected | 30.955 |
| `red-live-gates` | 0 / 1 / 0 | 101, expected | 10.635 |
| `green-credential-refusal` | 2 / 0 / 0 | 0 | 32.107 |
| `green-live-gates` | 1 / 0 / 0 | 0 | 16.223 |
| `green-handoff` | 1 / 0 / 0 | 0 | 14.846 |
| `green-full-host-suite` | 17 / 0 / 1 | 0 | 19.832 |
| `red-live-terminal` | 0 / 1 / 0 | 101, expected | 40.710 |
| `green-live-terminal` | 1 / 0 / 0 | 0 | 39.356 |
| `green-full-host-suite-final` | 17 / 0 / 1 | 0 | 26.649 |

The two new red tests failed at the intended boundaries: the fresh refusal lacked
`claim.json`, and the coding refusal kept 17 journal records where the terminal
record required 18. The existing Live admission test failed because the consumed
authority's refused run lacked its own claim. After the selected repair, the same
cases pass: claims remain consumed, the refusal is chained without credential
bytes, refusal sends no additional local fixture request, and injected append
failure returns the persistence fault. The positive probe/handoff passes its
initial `admitted` ordering and unchanged sealed-prefix assertions.

**Additional synthetic Live ordering guard: red/green PASS.** The counterfactual
temporarily restored only the old `open_after_probe` method, retaining the
fresh-open repair so the extended assertion was reached. It failed exactly there:
`credential_missing` was returned instead of `probe_seal_not_journaled`. The fault
assertion failed before the subsequent claim-absence assertion, so that run did
not directly report whether the old path left a fresh claim; no such runtime
observation is claimed. The exact repaired source was restored with SHA-256
`596f8cb316c00e7ddd2d09f87ab0c7fe3028e02828f547dd6dbe9fc2d3197d79`.

The unchanged extended test's valid-shaped synthetic Live coding admission passes
validation, then rejects the terminal journal with `probe_seal_not_journaled`
before the missing-credential fault. It asserts no new coding-authority claim file,
a byte-identical terminal journal and zero additional local fixture requests.
This establishes the selected synthetic branch
ordering, while its relabeled verification-derived seal does not establish an
actual Live-compatible probe history. The final complete pinned-artifact test
file also passed after this extension.

The complete non-ignored pinned-host file passes. Its one ignored case is
`full_pilot_pipeline_qualifies_without_provider`, which requires private pilot
evidence roots and was not selected or executed. These results do not qualify that
full evidence-backed pipeline. The totals above come from Rust summaries; elapsed
times come from the command receipts and include command setup/build overhead.

All nine commands exited naturally with no forced cleanup or observed owned process
remaining. Across these completed receipts, the largest sampled individual owned
process RSS was 3,116,990,464 bytes, minimum sampled free disk was 271,579,947,008
bytes, and 76 owned-process memory samples were missed. The sampled 8 GiB process
bound and 20 GiB free-disk floor held for these commands. These are sampled maxima,
not true peaks, aggregate process memory or GPU/global memory. Every completed
result records spend $0. No provider-transmission conclusion is inferred from exit
status alone; the refusal tests' owned fixture assertions establish their stated
local transmission boundary.

**Committed-candidate gates: PASS.** All eight named stages exit 0 on `7f5ba75`
in 741.560 seconds. The default suite reports 1,311 passed,
0 failed and 34 ignored, with 21 additional passing custom recovery
checks. The pilot unit gate reports 62 passed, 0 failed and 0 ignored.
The toolchains are Rust/Cargo 1.97.1 and actual Rust/Cargo 1.90.0 with macOS SDK 26.5.
The gate command exits naturally, with no forced cleanup or observed owned process
remaining. Its largest sampled individual owned-process RSS is
2,060,304,384 bytes, with 1,430 missed memory samples;
minimum sampled free disk is 268,721,946,624 bytes. These are sampled process bounds,
not true peaks or GPU/global-memory measurements.

The completed shared stage takes 1579.356 seconds of its original 1,800-second
bound, including the added synthetic Live guard; the deadline was not extended.
Source identity, the exact owned host and frozen v1 contract are unchanged across
the gate run. `commit-pin.json` also binds the unchanged policy prefix and focused
patches. The selected repair is complete at these local boundaries; the original
preflight refusal, red failures and counterfactual remain evidence, not green runs.
Final owners: `gates-result.json`, `gates-supervisor/000-full-gates.receipt.json`,
the eight gate logs and toolchain logs. No provider spend occurred.

Private evidence: `runs/pilot-credential-refusal-2026-11-11/`, including
`preflight-refusal.json`, `local-backup-host-pins.json`, `unexecuted-red-test.patch`,
`protocol.json`, `additional-live-guard.json`, `commit-pin.json`,
`gates-protocol.json`, the owned host, retained per-command patches, and each completed
`focused/{red-credential-refusal,red-live-gates,green-credential-refusal,green-live-gates,green-handoff,green-full-host-suite}/result.json`
and `focused/{red-live-terminal,green-live-terminal,green-full-host-suite-final}/result.json`
with its named command receipt/stdout/stderr.
Source plan pins: implementation patch
`9a7dd2b9767e20d332d079b456c5c16f969339fd584cd507613d855055c7ab31`;
red-test patch
`cea56c02387c6447ad4b7d38717acf369fadc843e5ab8877f16e461cd7c35f1e`.
These are the selected draft patch pins; the compiled host-path override is an
additional recorded integration-harness amendment. The final commit pin includes
the extended test SHA-256
`f971715fa0e89b1ffce7365fdbe099f5e8e025ae52521bfe11f5827190a546c9`,
the restored source SHA-256 above and `policy_prefix_unchanged:true`.
The recorded unchanged v1 contract SHA-256 is
`1a00758ffdafbfa18a59dd69e0b9912ef8beca9f7010f375bab8e553c554502d`.

The unfunded [pilot v2 draft](../specs/003-agent-retrieval-context/contracts/bounded-pilot-v2.md)
and 003 T006 remain open. Startup network-denial journaling, checker environment/
sandbox/compiler binding, retained compile stderr, coding-authority chain/path
binding, private-artifact modes, workspace-restore halts and `seal_slot` completion
binding retain their existing requirements and status. This repair supplies no
harder held-out task set, powered sample, arm-readiness proof, paid authority or
provider-economics result.

## T010 singleton qualification — 2026-11-11

**PASS: T010 is accepted at its written lexical retrieval scope.** The actual-default
singleton candidate passes the frozen G1/G2 measurement and the corrected full
neural file plus all eight gates under the declared test-only continuation.
Lexical behavioral selection defaults on with explicit opt-out. The first full
gates on `f19d353` failed and remain recorded below; the earlier default candidate's
failed economics guard and all of its receipts remain in
[the first-attempt record](#t010-lexical-default--2026-11-11).

The second candidate was selected before its tests. It is implementation commit
`029b95281c4f8e2c38f1575d1f3aaaa7b0a9a839`, followed only by the option-comment
correction `f19d35325d4f34f74b208eed816d328de569b9d0`. The complete isolated trial
already contains the proposed default in the library, CLI context and MCP owner,
with bare-flag compatibility and `--experimental-behavior-context=false` opt-out.
That actual default was committed and built before broad measurement; the candidate
MCP arguments contain no enabling flag. No ranking/default code change after the
frozen measurements is part of this candidate.

The accepted comparison baseline is parser/Tantivy code `68ed776` (record-only head
`1b4cf0e`), with behavior selection off by default. Baseline binary SHA-256:
`8b6b55d7dd7013cdebc9c6e3dec701c58ad2c6929857fc05e828c1ac0cb0e0ae`.
Final candidate binary SHA-256:
`60aff1025ba6515caa70b0854f233a1d2e2b4f8f83cfed290f143626e8c14011`.
The source/search/unit/semantic-generation schemas remain 12/9/7/6. This retrieval
repair does not change the parser bundle, persistent schema or semantic recipe.

**Selected repair.** Eligible anchorless lexical prose may retain exact-run priority
only when the complete existing restricted Count reports one indexed definition
document. Stopwords and suppressed incidental descriptors stay excluded; no role,
case or qualifier shortcut is added to the count. This is per-index uniqueness,
not compiler or global-root identity. Ambiguous common-word definitions keep the
existing prose ranking. Long prose candidates use the existing indexed-document
bounds at both cross-tier deduplication and final admission, including tier 1, so
promotion does not erase a later disjoint body match. The two-passage admission,
source freshness, candidate/delivery caps and response budgets remain. Ordinary
search and non-prose selection retain their rules. Any supplied dense window,
including partial/empty, still suppresses the flag's effects; named fallback remains
lexical. No new model call, scoring weight, corpus-specific exception or threshold
variant is selected.

**Focused red/green evidence.** Before the production repair, the frozen real-Engine
singleton-delivery regression failed at the final packed-body assertion, with the
other fourteen selected prose tests passing. The enabled economics boundary also
failed its unchanged expected-unit assertion. Both expected-red commands exited
101; these are retained failure witnesses, not passing implementation gates.
After the repair, all 92 core tests and both active economics tests passed (one
measurement report ignored); the four selected semantic-isolation tests passed.
The economics harness now runs the same six frozen questions through default and
explicit-enabled MCP owners. Its fixture bytes, markers, parser, body-identity
checks, assertions and 2,048-token cap are unchanged. This is a harness extension,
not a claim that the test file was untouched.

The initial actual-default boundary run failed two legacy ordinary-control arms:
`prose_diversity_backfills_independent_same_file_evidence_without_hiding_neighbors`
and `prose_plural_acronyms_match_the_acronym_without_changing_symbol_requests`.
Their default calls had become enabled under the proposed default; their explicit
experimental assertions passed. Only those ordinary calls were changed to explicit
`experimental_behavior:false`. Assertions, fixture source, caps and ranking stayed
unchanged, and the original focused deadline was not extended. The failed attempt
is retained. The complete corrected run passed 92 core, two active economics and
93 MCP tests; the default-on semantic-isolation run passed its four selected tests.
This exercises actual default, bare enabling flag, explicit opt-out and CLI/MCP
parity at the tested boundary. Focused all-target Clippy also passed.

The focused commands above ran on retained working-tree patches over `1b4cf0e`
before the candidate commit; their logs preserve that distinction. After committing
`f19d353` and building the final binary, the exact-default release-binary mixed
fixture run passed: complete required method support was 0/6 for the accepted
baseline and 6/6 for the candidate, all twelve locator/unknown-intent controls were
identical, and every cap held. The same mixed protocol was used without new tasks
or ground truth. These synthetic cases establish their stated retrieval outcomes,
not representative coding-task benefit. Each inspected focused-command receipt
recorded natural exit, no forced cleanup and no observed owned process remaining.
The completed broad-run lifecycle and resource receipts are recorded below;
the completed full-gates failure is retained separately below.

**Frozen family and bounds.** K=2 covers the previous lexical/default candidate and
this singleton candidate in the continuing promotion decision. The prior K=1
G1/G2 result remains historical evidence; its failed default gate does not erase
that attempt. The rule is exact one-sided McNemar p < 0.025, no net loss of verbatim
bodies and every response within 2,048 tokens; mean tokens are reported only.
This is repeated use of a frozen set, not a newly untouched holdout or a claim that
all historical retrieval development was covered by two comparisons. No variant
is authorized after measured failure.

One heavy job runs at a time with no spend or provider work. The selected phase
budgets are 1,800 seconds for builds/focused checks, 3,600 seconds for measurement
and 1,800 seconds for full gates, with a 120-second cleanup reserve, 20 GiB free-disk
floor and 30 GiB observed growth ceiling. The broad runner uses an 8 GiB aggregate
sampled owned-process RSS cap, stricter than the selected per-process cap. Only
owned copies of the pinned stores may be opened; source corpora and originals stay
read-only. The fresh baseline/candidate comparison uses compatible parser-derived
units, and no semantic preparation, training or corpus-code execution is selected.

| Required evidence | Baseline | Actual-default candidate | Verdict |
| --- | ---: | ---: | --- |
| Full G1 checker / qualified / bun tasks | 6,881 / 2,551 / 1,000 | Same | All three groups pass in both arms |
| G1 complete response records | 14,284 | 14,284 | Complete result, uncompressed response and source-identity hashes match in every group |
| G2 located definitions, 388 questions | 0 | 10 | +10 / −0; exact one-sided McNemar p = 0.000977 < 0.025, K = 2 |
| G2 verbatim definition bodies | 0 | 8 | +8 / −0; no net loss |
| G2 mean delivered tokens | 2,023.2 | 2,023.0 | Reported only; all 388 responses in each arm stay within 2,048 |
| All eight local gates on `f19d353` | — | FAILED | Seven exits 0; default test gate exits 101 with two neural-suite failures |
| Corrected full neural file and all eight gates on test-only continuation | — | PASS | Full file passes on the retained patch; all eight gates exit 0 on `8e88a35` |
| Final default-promotion/T010 disposition | — | ACCEPTED | Lexical default on, explicit opt-out; supplied dense windows and semantic model admission unchanged |

G1 totals are 10,432 tasks per arm; checker, qualified and bun respectively have
10,476, 2,779 and 1,029 response records per arm. Complete-file hashes compare the
uncompressed response bytes, not gzip container bytes or normalized text. The G2
split is Rust 0 → 4 located / 0 → 3 bodies on 197 questions, and bun 0 → 6 located /
0 → 5 bodies on 191. The literal unchanged `scripts/g2.sh compare` returns ENABLE
for this measurement. Overall acceptance additionally rests on the completed
test-only gate continuation below; the failed local run is not relabeled.
The first attempt's 13 located definitions and 10 bodies remain in its historical
record with its failed default guard. This run establishes improvement over the
declared current baseline, not superiority over that earlier candidate.

**Measurement lifecycle and retained inputs: PASS.** All 33 phases exited naturally
in 1,449.296 seconds, with no forced cleanup and no observed owned process remaining.
The tracked candidate stayed clean at `f19d353` before and after; the pinned source
corpora retained their heads and tracked bytes. Full original source-store manifests
and both repaired master inputs remained unchanged. The measured layout retains
three owned copies per corpus: one repaired lexical master shared by both G2 arms,
and the G1 baseline/candidate pair. The Rust G1 pair serves checker and qualified.
The literal frozen `scripts/g2.sh` creates and removes its own transient query
copies; those query copies are not retained. Complete query outputs and the named
retained stores remain private.

Peak sampled aggregate RSS for the driver and observed owned processes was
1,779,056,640 bytes, with 44 missed owned-process memory samples. This is a sampled
lower bound on a true aggregate process peak, with shared mappings potentially
counted repeatedly; it is not a GPU/global-memory measurement. Minimum free disk was
274,779,361,280 bytes and maximum observed allocated bytes were 26,950,246,400.
The recorded 8 GiB aggregate RSS, 20 GiB free-disk and 30 GiB allocation bounds held.
The completed receipt records no resume/additional variant, native lifecycle complete
and spend $0. These conclusions belong to this measurement, not a claim that the
subsequent full gates passed.

**First full gates: FAILED; retained separately from the passing continuation.**
All eight stages completed on `f19d353` in 781.6757805 seconds. Formatting, Clippy,
pilot-feature Clippy, pilot unit tests, actual Rust 1.90 check/Clippy and the Rust
1.90 pilot check exited 0; the default test stage exited 101. The default suite
reported 1,309 passed, two failed and 34 ignored. Its 21 custom recovery checks
passed separately. The pilot unit gate passed all 62 tests. The failing neural
suite itself reported 94 passed, two failed and one ignored:

- `neural_candidates_crowded_by_other_evidence_are_still_delivered_in_budget`,
  assertion at `tests/neural_retrieval.rs:1984`.
- `without_a_usable_profile_cli_and_mcp_answers_are_the_plain_baseline_byte_for_byte`,
  assertion at `tests/neural_retrieval.rs:5277`.

The receipt records unchanged candidate source and unchanged measurement binary.
It used Rust/Cargo 1.97.1, actual Rust/Cargo 1.90.0 and the macOS 26.5 SDK. The outer
gate command exited naturally with no requested signals, forced cleanup or observed
owned process remaining; the supervisor's process-exit status is not the verdict
for the individual Rust test stage. `gates-result.json` correctly reports passed=false.
The gate receipt's sampled owned-process RSS peak was 2,069,790,720 bytes with
1,452 missed owned-process memory samples; minimum free disk was 270,781,730,816
bytes. These are sampled process measurements, not true peaks or GPU/global memory.
The 1,800-second stage limit and recorded resource bounds held. Test worker
subprocesses were observed during this full suite; the measurement's no-worker
scope must not be copied onto the gate run.

**Confirmed fixture diagnosis; test-only continuation: PASS.** The two failing
comparisons selected different behavior-option modes. The no-profile MCP helper
explicitly selected false while CLI/default comparison owners inherited true;
the mismatch was present before fallback. The crowding fixture compared a newly
prose baseline with intentionally ordinary dense selection and failed before its
delivery assertions. These are the confirmed fixture-option causes of the retained
failures, not evidence that the delivery assertions passed.

Only `tests/neural_retrieval.rs` changed: the crowding test's local baseline now
sets `ContextOptions.experimental_behavior=false`, without changing the shared
baseline helper, and `owner_with` inherits `ContextOptions::default`. All assertions,
queries, fixture bytes and caps are preserved. Production source, ranking/default
behavior and the frozen G1/G2 measured binary/receipts are unchanged.

A separately declared 1,800-second shared continuation covers the full neural file
and all eight gates, with a 120-second cleanup reserve, 20 GiB free-disk floor,
8 GiB sampled owned-process RSS bound, one heavy job and spend $0. It does not
extend or relabel the failed run. Protocol:
`gate-fixture-continuation/protocol.json`; base `f19d353`, recorded production-source
tree `b3bd8925e3e9ef289fe6ed1028285f291fbeb919`, and the same final measured binary
SHA-256 above. The corrected full neural file passed on the retained test-only
working-tree patch over `f19d353`: 96 passed, 0 failed, 1 ignored, in 44.409 seconds.
The patch was then committed as `8e88a3599e7dee4675a207ac919aaa049faf9980`.
Its only changed path from `f19d353` is `tests/neural_retrieval.rs`; the
production-source tree and retained measured binary are unchanged. The full
eight-gate continuation on `8e88a35` passed in 578.644 seconds,
with formatting, Clippy, default tests, pilot-feature Clippy, pilot unit tests,
actual Rust 1.90 check/Clippy and the Rust 1.90 pilot check all exiting 0.
The default suite reported 1,311 passed, 0 failed and 34 ignored,
plus 21 passing custom recovery checks. The pilot gate reported
62 passed, 0 failed and 0 ignored. It used Rust/Cargo 1.97.1,
actual Rust/Cargo 1.90.0 and the macOS 26.5 SDK. The completed shared stage
occupied 680.205 seconds of its declared 1,800-second bound.

Both commands exited naturally, with no forced cleanup or observed owned
process remaining. The full-gate receipt records a maximum sampled individual
owned-process RSS of 884,113,408 bytes, 1,375 missed owned-process
memory samples and minimum free disk of 270,720,831,488 bytes. The corrected
neural command separately recorded 625,229,824 bytes of sampled owned-process RSS,
7 missed memory samples and 274,152,775,680 bytes minimum free disk.
These sampled process maxima are not true peaks, aggregate process memory or
GPU/global-memory measurements. The recorded resource bounds held; spend was
$0. This successful continuation does not alter the failed gate receipt or
rerun/tune the frozen retrieval measurement. A matching documentation
consistency correction removes the obsolete semantic-prose exception from
the context contract; it describes the already-tested dense-window exclusion
and adds no runtime rule.

No semantic-quality gain, real-agent benefit, provider savings, signed-package
acceptance or release publication is established by this record. Git publication
is a separate action from release publication.

Private evidence: `runs/t010-singleton-2026-11-11/` and
`datasets/context-foundry-t010-singleton-2026-11-11.noindex/`. Key focused owners are
`selection.json`, `frozen-test-source-pin.json`, `focused/{red-prose,red-economics,green-core-economics,default-boundaries,default-boundaries-corrected,default-dense-isolation,final-default-mixed}/`,
`default-boundaries-correction.json`, `candidate-binary.json`, `candidate-source-check.md`
and `default-mixed/mixed-fixture/result.json`. Completed measurement owners are
`measurement/result.json`, `measurement-summary.json`, `g1-cross-arm.json` and
`measurement/commands/032-g2-compare-k2.stdout.txt`. The failed full-gate owners are
`gates-result.json`, `gates/test.log`, `gates/pilot-test.log`, the two toolchain logs
and `gates-supervisor/000-full-gates.receipt.json`. The completed continuation owners
are `gate-fixture-continuation/{protocol,commit-pin,gates-protocol,gates-result}.json`,
`gate-fixture-continuation/focused/corrected-neural-suite/`, the continuation
`gates/` logs and its `gates-supervisor/000-full-gates.receipt.json`. The narrow in-session source check
found no functional mismatch to the fixed rule; its one stale option comment was
corrected in `f19d353`. That check was not independent certification or runtime proof.

The input-freeze receipt passed without opening original stores. Recorded protocol
SHA-256: `cc313a8bec1b79b5d77954956c2e71696af71bb028c24482037a4e06fad6d1bc`;
recorded measurement-driver SHA-256:
`2a64efdad550b98987d1f5b341820b6c875f6e2e14380595eba1e042d7b80cd1`.
Selected-rule SHA-256:
`a39cb7d1fcef9961092d3b74eec71b57d6101fb2c60a998464fcf1ef88d0ba4e`.
Mixed-protocol SHA-256:
`e2f8da77eacc76666d5de98a1178c9312c1eade23cb5c49bdcef66bdceb0c7af`.
Completed measurement-result SHA-256:
`3e9c4faa52f80133fae5cae7ba8182239c69b69a4fd4ae6829da84d9093c38a7`.
These identities and metrics are copied from completed small receipts, not newly
hashed or recomputed from question/response bodies for this draft.

## T010 lexical default — 2026-11-11

**HOLD: default promotion failed a frozen local guard.** One
fixed candidate, `ceca2d8`, removes the behavioral flag's semantic effects and keeps
the existing lexical algorithm. No ranking was tuned from these results. The
comparison ran on the owner's M5 Max against `a456c53`, with no provider calls,
training or corpus-code execution. The default-switch candidate was tested through
the library, CLI context and MCP owner, then withdrawn after the failure below.
The flag remains **off by default**. Its semantic cleanup is retained: a supplied dense window, including
partial/empty windows, ignores the option. Ordinary semantic admission is unchanged
by the flag; named fallback remains lexical.

| Frozen measurement | Baseline | Candidate | Verdict |
| --- | ---: | ---: | --- |
| G2 located definitions, 388 questions | 0 | 13 | +13 / −0; exact one-sided McNemar p = 0.000122, K = 1 |
| G2 verbatim definition bodies | 0 | 10 | +10 / −0 |
| G2 mean delivered tokens | 2,023.2 | 2,023.7 | Reported only; every response within 2,048 |
| G1 checker / qualified / bun tasks | 6,881 / 2,551 / 1,000 | Same | All existing guards pass in both arms |
| G1 complete response records | 14,284 | 14,284 | Byte-identical, including arguments, evidence and order; no normalization |
| Fresh mixed-intent fixtures, complete method support | 0/6 | 6/6 | All twelve locator/unknown-intent controls identical; all caps pass |

The G2 corpus split is Rust 0 → 3 located / 0 → 3 bodies, bun 0 → 10 / 0 → 7.
These are absolute lexical results on the frozen identifier-free set, not the
hierarchy semantic scores or a general retrieval-success rate. The six new
synthetic mixed-intent cases and exact required spans were frozen before their
first run (`mixed-protocol.json`, SHA-256
`e2f8da77eacc76666d5de98a1178c9312c1eade23cb5c49bdcef66bdceb0c7af`). They supplement
the existing ambiguity, stale support, graph, scope and budget fixtures; they are
not representative coding tasks or a separate statistical promotion test. No
real-agent benefit or provider savings is claimed.

The predeclared measurement bound was one hour including setup and diagnosis,
20 GiB free disk and 8 GiB sampled owner RSS. G1/G2 completed at 1,778.82 s; the
mixed-intent check took another 5.17 s. Maximum sampled owner RSS was 578,224,128
bytes; minimum free disk was 306,294,603,776 bytes. No embedding worker was observed.
Samples are lower bounds on true process peaks, not GPU/global memory measurements.
Both original stores retained identical full file-hash manifests; only owned copies
were opened. Corpus heads and frozen harness/question/counter pins were checked.

**Retained failures and local checks.** A new semantic-admission regression failed
before the repair and passed after it. The first neural suite had obsolete semantic
experiment expectations and sandbox-denied HTTP/watch fixtures; the native rerun
passed 96 tests, with one ignored. Existing source identity, freshness and resource
checks remain. The process-observation race and its red/green tests are recorded
below; the fresh qualified baseline reproduced all 2,779 responses from the failed
apparatus attempt. Default promotion exposed two core fixtures implicitly relying
on the former default; that trial made their baseline calls explicitly set `false`,
with assertions unchanged. The trial boundary suites passed all 88 core and 93 MCP tests,
including library defaults, bare-flag compatibility, explicit opt-out and exact
CLI/MCP parity. Default binary `aac40dd` reproduced all 388 G2 response records and
result rows exactly; its SHA-256 is
`2f15591cc93064a7b9cce91f36f6a1ee250b7e2e34f159f10a21640e32a624ad`.
The first full gate attempt stopped after Clippy rejected a literal-boolean
`assert_eq!` in a semantic regression. The assertion now uses `is_none()` without
changing its condition. The supervisor's 60-second interrupt grace expired during
the test suite, then bounded cleanup removed all observed owned processes. That
partial run is retained and does not count as acceptance. The next full attempt
passed formatting and Clippy but failed
`economics::question_contexts_surface_their_expected_units`: “session allowance
refund” did not deliver the independently frozen `fn refund(` marker at 2,048 tokens.
The other five frozen subsystem questions pass in a separate CLI reproduction;
the baseline passes all six. That gate attempt was also stopped and its observed
owned processes were cleaned up. Neither partial run counts as acceptance.

The untruncated trace shows the same fresh `Shared::refund` unit at bytes
13,528–13,841 of the frozen `src/mcp.rs`: baseline selection/packing rank 0, delivered
verbatim; the candidate disables exact-run promotion, keeps it at selection rank 18,
then drops its signature at packing rank 23 for the token budget. This establishes
ordering and packing loss, not missing recall. Scores and term contributions are
not traced. The fixture's marker, source bytes, cap and assertions are unchanged.

Candidate family 1 therefore **does not qualify for default promotion**, despite
its G1/G2 results. No new ranking rule was tuned to the failed fixture. The
default-switch code and its dependent test changes are withdrawn; semantic isolation,
the direct admission regression and the native supervisor repair remain. T010 is
not accepted. A future lexical repair needs a separately declared candidate and
fresh local/frozen measurements. The first complete implementation-gate run on the
retained cleanup failed the separate writer-lock case [recorded above](#derived-writer-contention-after-fork--2026-11-11).
After its repair, all eight gates passed on `fef81c6`; default promotion stays held.

Private protocols, binaries, complete outputs, failed receipts and resource samples
are retained in `runs/t010-lexical-2026-11-11/` and
`datasets/context-foundry-t010-2026-11-11.noindex`. The baseline binary SHA-256 is
`c9e497a74e02fd8d12f2b16b6334fe5f442c40af511df81d8a0885b419806e94`; the measured
explicit-flag candidate is
`1fd668cff5bc2c564e1e43c54967a0aae63eddd5d1da330ea4c2f1ff64db250a`.

## Wrap-up continuation — 2026-11-11

The signed-package preflight found zero valid code-signing identities in the
owner's current keychain search list (`security find-identity -v -p codesigning`,
checked outside the execution sandbox). Source inspection also confirms two
implementation prerequisites: `scripts/embed-worker-bundle.sh` signs ad hoc when
called by the installer, and `src/neural/supervisor.rs` refuses normal admission
without the development flag. A certificate alone would not complete row 4.
Signing remains deferred until product readiness; no key was accessed, package
installed, host setting changed or publication attempted. These are metadata and
source findings, not signed-runtime acceptance.

001 T010's fixed candidate and no-spend bounds were recorded before measurement in
private `runs/t010-lexical-2026-11-11/protocol.json`; see the receipt above.

The [pilot v2 draft](../specs/003-agent-retrieval-context/contracts/bounded-pilot-v2.md)
records the selected comparison, task-context outcomes, capture/accounting boundary,
v1 gap repairs and execution locks. It is deliberately not labeled frozen or
powered: concrete held-out tasks, sample calculation, implemented arm matrix and
costed launch schedule remain to be prepared. This is documentation work with no
provider calls or credential access; v1's frozen bytes are unchanged.

The G1 measurement supervisor initially refused the baseline qualified set after
its wrapper exited 0: its process snapshot preceded the exit poll, so a descendant
could finish between the two observations. The final identity check found none
remaining, with no forced cleanup. `NativeFullCommand.run` now refreshes descendant
liveness after observing owner exit. `python3 tests/native_supervisor.py` passes two
real-process regressions: a forced snapshot/exit race and a genuinely live orphan
that must still fail and be cleaned up. The race test fails against the prior
script. This changes observation ordering, not the lifecycle acceptance rule.
The failed receipt is retained; a fresh qualified baseline and the remaining G1
sets passed with the same binaries/scoring and the original one-hour wall bound.

## Native macOS acceptance — 2026-11-10

**PASS at the owner's selected script scope, on `01a1152`.** The fresh retry ran
all 33 stages of `scripts/native-macos-acceptance.py --generation --scale-calibration`
successfully in 382.25 s on the physical M5 Max. This accepts the ad-hoc-signed
development-device checks for 009 T008/T009 and 010 T002 under decisions 12/13;
the signed release-candidate run and installed-package checks remain open.
Only local inference and pinned public downloads were used: no paid provider,
training or corpus-code execution. This was an in-session verification.

**Retained failure and repair.**
The first native run on `ec234b3` failed at the embedding-worker build. It ran
`scripts/native-macos-acceptance.py --generation --scale-calibration` on the owner's
M5 Max, with a two-hour outer budget, 20 GiB disk floor, ad-hoc signing and fresh
`.noindex` work storage. Task-local launchers select the actual installed Rust 1.90
binaries because the host's Cargo does not dispatch `+toolchain`; SDKROOT explicitly
selects the same macOS 26.5 SDK as the gates. No host configuration changed.

Before that build failure, both native CPU checks, both held-GGUF checks, all six
synthetic lifecycle checks and a real Metal compute dispatch passed. The pinned
public model/tokenizer downloads passed their hashes. No real embedding, generation
or calibration ran in that attempt, so it did not accept the native checks. The outer
supervisor observed normal exit, reaped its child and confirmed no owned descendants
remained. Private evidence is under `datasets/context-foundry-native-2026-11-10/out`
and this checkout's `runs/native-macos-2026-11-10/`.

The compiler error is a concrete AST-type mismatch: `bindgen 0.73.2` resolved
`syn 2.0.119`, while `prettyplease 0.3.0` uses `syn 3.0.6`. The `8600fc0` lock update
had changed that bindgen edge from syn 3 to syn 2. `cargo update --offline -p bindgen
--precise 0.73.2` restores the previous syn 3 edge; no package version, source,
license, application code or default-build dependency changes. The one-edge repair
is committed as `01a1152d463a09b48ee97ceb384dd19d80778b30`. The fresh retry below
verifies it; the failed attempt is retained, not relabeled as passed.

**Fresh retry, unchanged bounds and assertions.** Rust 1.90.0, macOS 26.5 SDK,
the pinned llama.cpp `b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea`, Gemma BF16
adapter 4 and Qwen 0.5B smoke artifact were built in new `.noindex` storage.
Spotlight and host configuration were not changed. The 7,200 s outer deadline,
20 GiB disk floor, script deadlines and 4 GiB embedding / 2 GiB generation ceilings
were unchanged. The script itself is unchanged, SHA-256
`6d7b1cd6cf5129005671b103dd81d828847f3976b300c334f957268247b239fc`.

| Selected check | Observed result |
| --- | --- |
| CPU admission | Both native observation and actual pressure/pause/resume tests passed for semantic and generation lanes. |
| Held models and synthetic lifecycle | Both held-GGUF tests and all six owner-death/resource-limit/forced-reap fixtures passed. |
| Metal embedding | Real compute dispatch; five 768-dimensional vectors, including an 8,192-token input; observed worker gone and scratch empty. |
| Owned Metal generation | Initial and changed source each generated one cited brief: 375/123 and 379/122 prompt/completion tokens. Current and restart/revert reuse each made zero calls and acquired no model resources; HTTP calls zero. Both workers were observed and gone. |
| Fixed overview calibration | 128 sources, 128 distinct inputs/calls, 33,754 input tokens; 1.582 s supervised provider time, 13.968 s preparation, 19.839 s total test harness; unchanged replay made zero calls. |
| Final lifecycle | Supervisor exited 0 naturally, without forced cleanup; no observed owned processes remained. Calibration worker reaped and scratch empty. |

Calibration memory was sampled before/after each successful call: maximum observed
physical footprint 321,734,072 bytes, excluding acquisition and between-sample peaks.
The generation receipts observed at most 407,602,496 bytes physical footprint and
617,201,664 bytes process GPU allocation, within their configured limits. Neither
measurement is global GPU headroom or a combined-model residency result. The calibration
uses the fixed public Rust/Go `overview-v1` sample; it is not a population-weighted
estimate or completion of the 67,663-source corpus. Mac Rust hierarchy scale is
accepted separately by the replay/change receipt below.

**Evidence limits.** This run does not establish cross-host CPU/Metal numerical
parity, real-model owner death under pressure, generator adversarial isolation,
global GPU pressure, combined residency, thermal/battery policy, embedding eviction,
retrieval/task utility, or signed installation. The generator remains an experimental
smoke model, not the deferred 010 production provider/recipe selection. Existing
default-feature gates on `65fd799` are reused: application code and their dependency
graph are unchanged. The optional worker features were freshly built and exercised
here; no hosted CI or release was run.

Private retry evidence is in `datasets/context-foundry-native-2026-11-10-retry1/out`
and this checkout's `runs/native-macos-2026-11-10-retry1/`. Both attempts retain
the exact script/launcher, predeclared protocol, stage logs, process observations,
final receipts and `SHA256SUMS`; no private artifacts are committed.

| Retry evidence | SHA-256 |
| --- | --- |
| Protocol | `d99f7faf649905a549fb5d5f0678205f47a9b11bc72db945bb80c051423c79a3` |
| Native result | `08826d4b0d859fc4470d079b7b49ec41942a4341761b94b71a2d1cf9e4760911` |
| Metal receipt | `f9bca1279fcd778bed25be0ff7c684b30478503707d5c78c2f0cb53c92187687` |
| Generation receipt | `d0b285aaabe6cd11da2bdc015e00c9e72a0a8f0879cc04f71c814bda0d6a5a4f` |
| Calibration receipt | `836674dbd360bcad7ed4d5a5d329023a9027268338f5de9916ce8fef9ad931cb` |
| Supervisor and acceptance result | `62896fc7d44070e3993d9fec13346a8b3ecf6cd43bc23ebeb0e18ebcd5ff3ce3` |
| Evidence manifest | `73f1b01bafce96d2cc9ae274b7828d77a211673b1b631f7c07630582e3839972` |

## Semantic scale replay and changed-input acceptance — 2026-11-10

**PASS for 009 T006–T009 scale under decision 11.** The owner's M5 Max ran the
existing complete Rust hierarchy store through replay, add/edit/revert/delete,
full audits and actual dense serving. This accepts the selected Mac Rust scale
boundary together with the preserved complete preparation; it does not accept
retrieval quality, task benefit, Linux x86/the 67,663-source corpus, native device
checks or signed installation.

**Pins and bounds, declared before launch.** Code at `d52e903` (Rust sources unchanged
from `65fd799`), Rust corpus `b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`, release
CLI SHA-256 `c9e497a74e02fd8d12f2b16b6334fe5f442c40af511df81d8a0885b419806e94`.
The existing `repair-units6-9WjnwU/rust` store was APFS-cloned before opening;
function `01dcfe57…`, `hierarchy-v1+overview4+context2+kinds2+units6:512+parent64+tokenizers 0.23.2 onig`,
768 dimensions and the pinned Gemma BF16 Metal worker were unchanged. A unique
scratch subdirectory stayed inside the signed development bundle's existing grant.
Budget: 1,800 s total, 300 s per preparation, 4 GiB cache cap, 20 GiB disk reserve,
sampled owner/tree RSS stops at 8/12 GiB, fewer than 64 embedded units per probe
change. Only local inference; no paid provider, training or corpus-code execution.

**Observed, 46 checks passed in 104.4 s.**

| Phase | Wall time | Document calls / embedded units / input tokens |
| --- | --- | --- |
| Unchanged replay | 4.83 s | 0 / 0 / 0; no repartitioning |
| Add one owned probe | 5.80 s preparation | 2 / 2 / 172; one partitioned source |
| Edit that probe | 5.73 s preparation | 2 / 2 / 172; one partitioned source |
| Revert to original probe bytes | 4.50 s preparation | 0 / 0 / 0; retained vectors reused |
| Remove the probe | 4.82 s preparation | 0 / 0 / 0 |
| Final unchanged replay | 4.50 s | 0 / 0 / 0; no repartitioning |

Every preparation completed and published. Full audits returned to 60,739 sources,
685,623 eligible/cached/searchable current units and 624,325 ANN entries, with zero
unpartitioned sources, missing units or corrupt cache entries. The four new vectors
remain cached (624,329 cache entries); source revision advances normally. Per-run
selection counters revisit sources/units and are not corpus-size counts.
The old edited handle was rejected as stale; new and reverted declaration bytes and
hashes matched, and deletion removed the probe from search. The final real context
request used dense serving (64 hits), delivered 1,996 tokens under 2,048, and all
26 delivered citation hashes/ranges matched disk. This is a serving/freshness check,
not a retrieval-quality comparison. Peak sampled RSS: owner 2.83 GiB, owned tree
3.54 GiB; no global GPU/true-peak claim. Three workers were observed and absent at
completion, scratch was empty, the probe was removed, and byte/hash manifests of
both the original corpus and original store were unchanged.

**Retained preparation failures.** The initial release build selected incompatible
macOS 27 SDK stubs; explicitly selecting the gates' macOS 26.5 SDK completed the
build. The first private driver run passed replay and add/preparation, then failed
because it omitted CLI `retrieve --handle`. It removed its probe and verified both
original manifests and worker cleanup. The corrected driver passed the entire
sequence on a fresh store copy; no product code or acceptance threshold changed.
The existing green Rust gates are reused, not rerun for these documentation changes.

Private evidence: `runs/semantic-scale-2026-11-10/` in this checkout; `run-1` preserves
the failed attempt, `run-2` the accepted run. The latter contains the exact driver,
predeclared protocol, before manifests, raw reports, serving trace, process samples
summarized per phase, final receipt and `SHA256SUMS`.

| Evidence | SHA-256 |
| --- | --- |
| Protocol | `90b07a2749adc01101372e140970ee4dd369fb7cbd39a362002eaa9cba1a92d5` |
| Driver | `0d85325d18978916f38cebd006e0746b87f99f0e480028aacb74452bcf570ff3` |
| Result | `647b1f79250c8500162bf88cb7a8e7066a15c2b51c136800e13d8630540e5ded` |
| Original-store manifest | `4afee773e03f633a86eb445bfd1bdc625281e99416c48b910c01b2965d1383e5` |

## Native Mac watcher acceptance (001 T011) — 2026-11-10

**PASS at the owner-selected native scope.** Self-review on the owner's Apple M5 Max,
64 GiB Mac; no provider, model, corpus-code execution or paid call. The production
stdio `mcp --watch` binary is `target/gates/debug/foundry`, built from code `65fd799`
under Rust 1.97.1; source/tests/lockfile are unchanged at `eda1edf`. Its SHA-256 is
`c4396172c3be1057ebbc5da3f82a485549476f294f5272b61ebdb4951286f201`.
The existing eight-gate receipt below is reused, not rerun.

**Inputs and count correction, established before launch.** The Rust 1.99.0 corpus
is still pinned to `b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`. The earlier
"62,116-file" description is not the current inventory: Git has 62,035 entries
(62,018 regular files, five symlinks and 12 submodule entries); the filesystem has
62,023 file/symlink entries outside `.git` and `.context-foundry`. The run used an
APFS copy, preserving relative paths, ignore rules and bytes, with a fresh store.
No files were added to pad the corpus. Its manifest SHA-256 is
`45c2f2111b8395a3389e230ebbe25c7743073aa27851b8dd149c26692bbecd56`.
The complete admitted count, 60,739, matches the prior pinned-corpus record.

**Predeclared decision and bounds.** This run decides T011's native watching
acceptance, not semantic preparation or retrieval quality. Total wall budget 3,600 s;
cold reconciliation 1,800 s; restart reconciliation 900 s; each edit phase 60 s;
each MCP request 35 s; owner sampled RSS 8 GiB; free-disk floor 20 GiB; clean
shutdown 15 s. A failed bound leaves T011 open. Search requests asked for 2,048
tokens. The protocol was written and hashed before the full run; no acceptance
threshold changed during it.

| Check | Observed |
| --- | --- |
| Whole run | 697.9 s; 65 checks passed, none failed |
| Cold reconciliation | 381.2 s after MCP initialization; 60,739 accepted, 75 excluded, 60,814 body reads; one full reconciliation, zero events/overflow/errors, no pending index work |
| Foreground service | Status and search served during reconciliation; 313 tool calls over the run, p95 184.7 ms, maximum 1,161.2 ms |
| Owner memory | Peak sampled RSS 541.8 MiB; all sampled disk reserves above the floor |
| Idle operation | Two 120-second windows, before edits and after recovery; zero extra body reads in each |
| Atomic edit | New bytes and hash delivered, old handle rejected as stale; one additional body read and no extra full reconciliation |
| Burst, rename, delete | 64 files created, directory renamed, all probes deleted; each phase converged by the next two-second poll (2.04–2.05 s), with current paths and no deleted hits |
| Abrupt restart | Only the owned MCP child killed; offline edit recovered in 27.5 s including reopening (22.6 s reconciliation); new declaration/hash delivered |
| Cleanup and preservation | Both owned children reaped; clean final exit and store reopen; final admitted count restored; copied and original corpus manifests unchanged |

The two-second polling interval is an observation bound, not measured event latency.
RSS sampling is not a hard memory-enforcement proof. Foreground timings cover this
serial status/search workload; no prior-version latency comparison, universal
contention bound or economic benefit is claimed. Native scale did not inject queue
overflow or ignore-policy changes; those remain covered by the existing synthetic
contracts. The M3 Pro duplicate-read observation in F18 remains unresolved.
T011 is accepted under decision 15. This does not accept semantic scale, the native
model/device script, signed-package checks, or T006/real-host task outcomes.

**Private evidence and reproduction.** In this checkout, all raw files remain ignored
under `runs/t011-native-2026-11-10/`. `watch_run.py --out <fresh-absolute-directory>`
runs the same no-provider driver against the pinned corpus; the binary is required
at `target/gates/debug/foundry`. `rust-1/` contains the exact `harness.py`, protocol,
corpus manifest, request/response log, statuses, latency samples, stderr, final receipt,
response-byte/hash audit and `SHA256SUMS`. The fixture preflight passed before the
Rust run. Its first two failures are retained: a 100-file fixture completed before
observing contention, then a driver assertion incorrectly assumed lexical search
returned only an exact name. Both were corrected before freezing the full-run driver;
no product code changed.

| Evidence | SHA-256 |
| --- | --- |
| Protocol | `1c6f09c13ad82e6638b69d93d75e9a431ae89a43ee7b629b39a024bbb2db7c06` |
| Driver | `37ce2ee1ec3e2a2ab4b17ac46c1b0d1d5d34c6ab4aada92218a358645458bfca` |
| Raw events | `fc40d8d78a5be5150ce7b2def67cf1cb4cf68cbe6dd137a82f8a34db22c67911` |
| Result | `1e277cae7b7bf2def213a3654be029d58e8fd3a20dd36601599ab6819cc4edfa` |

## Boundary scope acceptance (005 T005) — 2026-11-10

Decision 16 (2026-11-06, "Accept at written scope") is fulfilled by the existing
`65fd799` local gates receipt below. This is an in-session readback of recorded
checks, not a new test run or independent review. The code, tests and lockfile at
`eda1edf` are identical to `65fd799`.

The preserved `test.log` contains 21 passing boundary tests and 24 passing task-evidence
tests, including `composes_task_and_multiservice_evidence_with_exact_citations_and_no_duplicate_spans`,
`ambiguous_relationships_are_exposed_and_missing_tests_remain_missing`, the real
CLI/MCP delivery fixtures, source/configuration invalidation, interrupted publication,
restart and watched maintenance. All eight local gates exit 0. Test-log SHA-256:
`e97514d8c7c30cb64e9bc2159a760d46166d122d3b2599b12d6a14ad5cd664f3`.

T005 is accepted at the written multi-service fixture scope and no longer blocks
release. The five optional real-framework boundary tests and one CPython task-evidence
test are ignored in that receipt; their older runtime records were not rerun here.
Other frameworks, brokers, broader imports, cross-language identity and dynamic
collectors remain named limitations. This acceptance does not establish 003 T009
real-host outcomes, T006 benefit or release readiness.

## Watcher test repair (F18) — 2026-11-10

Self-review, on the owner's Mac (Apple M5 Max, 64 GiB). Private logs:
`datasets/context-foundry-gates/f18-2026-11-10/` and
`datasets/context-foundry-gates/65fd799-2026-11-10/`.

**Observed before the change, at `07b7b1d`.** Three runs of
`six_thousand_files_allow_foreground_calls_and_no_idle_body_rescans` passed in 103, 111
and 122 seconds against the test's 180-second deadline; the two failures
recorded on 2026-11-06 ran past it. In every run with a captured status the watcher
reports `events` 0, `full_reconciliations` 1 and `source_reads` 6,000, so on this
Mac the test does not reread sources: it is slow. A phase probe (temporary, not
committed) put the walk at 60 s and the remaining index drain of 4,504 keys at 56 s.
A 10-second `sample` of the owner during the walk found 73% of the watcher thread
inside redb's durable commit, almost all of it in `File::sync_all`. The walk made
one forced commit per file, and each eight-key index page made a search commit and a
store commit: about 6,750 forced commits for 6,000 files.

**Change, `65fd799`.** A watcher walk publishes the files it reads in atomic batches
of at most four through the existing foreground batch commit, rechecking each file
by name just before the commit, and a background index page holds 32 keys instead
of eight. A single hinted path still commits alone. The test's assertions and
deadline are unchanged; its read-count assertion now prints the watcher status.

**Checked.**

- The probed test at the change: walk and drain complete in 27 s, test 31 s.
- `tests/watch.rs`, all four tests, five consecutive runs: 5 of 5 pass in 29–43 s,
  each with `events` 0 and `source_reads` 6,000.
- New unit test `a_walk_never_publishes_a_file_rewritten_after_its_read`: a file
  rewritten between its read and its batch commit fails the walk and is never
  published with the earlier bytes. With the recheck disabled the test fails.
- Local gates on clean `65fd799` (20:47–21:05 UTC), Rust 1.97.1 and 1.90.0: all
  eight gates exit 0; **1,315 passed, 0 failed, 34 ignored**; `pilot-test` 62
  passed. The watch file took 50 s inside the full suite. Log SHA-256 prefixes:
  Clippy `8ca4fb7d…`, test `e97514d8…`, pilot Clippy `5799c3ab…`, pilot tests
  `7109aa4e…`, MSRV check `975eb1f3…`, MSRV Clippy `fd6e45c3…`, MSRV pilot check
  `5b2a6c17…`.

**Not established.** The 2026-10-17 symptom on the M3 Pro, 6,076–7,658 reads
against 6,000, did not occur in any of these runs, so its late-hint hypothesis is
neither confirmed nor repaired. No test measures foreground latency behind a
32-key page or a four-file batch. Not run: CI (Actions unavailable), native macOS
acceptance (F7) and the 62,116-file watch run that accepts 001 T011.

## Owner decisions — 2026-11-06

After the pilot, an in-session sweep (self-review, not independent) searched the
repository, GitHub and the private pilot files for decisions waiting on the owner.
It found 113 raw candidates, 57 of them distinct, and a completeness pass added 7
more. Verifiers dropped 32 of those 64 as already decided, the agent's own call or
not real, kept 32, and grouped those into 29 questions. The owner answered all of them, plus two follow-ups, in one
multiple-choice dialog. "Done" means it happened on 2026-11-06; everything else
is work to do.

**Release path**

| # | Item | Owner's choice | Result |
|---|---|---|---|
| 10 | 009 signed-package checks (handoff row 4) | "Row 4 blocks release" | The first release ships signed semantic retrieval; a failing row 4 is fixed first. |
| 14 | 010 real-model acceptance | "Defer 010 real-model acceptance" | The release no longer waits on 010's task utility; the code stays experimental and unadvertised. |
| 20 | 003 T004 meter-only Z.ai gateway | "Release it separately" | CLI and MCP release without advertising it; it gets its own release once proven live. |
| 16 | 005 T005 boundary relationships | "Accept at written scope" | Accepted using the `65fd799` local gates receipt ([2026-11-10](#boundary-scope-acceptance-005-t005--2026-11-10)); other producers remain named limitations. |
| 11 | 009 T006–T009 scale | "Accept the Mac rust run" | Done: the existing Mac hierarchy-v1 preparation plus the [replay and changed-input check](#semantic-scale-replay-and-changed-input-acceptance--2026-11-10) accepts scale; F10/F11 retire; Linux x86 and the 67,663-source corpus are a named limitation. |
| 12 | Device acceptance scope | "Script checks only" | What `scripts/native-macos-acceptance.py` checks decides; unbuilt GPU-pressure, residency, thermal/battery and eviction policies are named limitations. |
| 13 | Native macOS acceptance (F7) | "Once now, unsigned" | Done: the ad-hoc-signed run on the owner's Mac passed ([2026-11-10](#native-macos-acceptance--2026-11-10)); repeat on the signed release candidate. |
| 15 | 001 T011 watching | "Native Mac run on rust" | Done: F18's deadline repair and the bounded native `mcp --watch` run on pinned rust-lang/rust passed ([2026-11-10](#native-mac-watcher-acceptance-001-t011--2026-11-10)); the receipt corrects the earlier file-count description. |
| 9 | 001 T010 and `--experimental-behavior-context` | "Lexical-only, drop T006 gate" | Semantic effects remain removed. The fixed singleton candidate passes its required retrieval and local gates; lexical behavioral selection is accepted and defaults on with explicit opt-out ([receipt](#t010-singleton-qualification--2026-11-11)). The first failed default attempt remains recorded; no 003 T006 or semantic-quality acceptance is implied. |
| 24 | Query-time parse cost (001 T006, 007) | "Accept as named limitation" | No code change; reopen only after a measured failure on a real corpus. |

**T006 and task context**

| # | Item | Owner's choice | Result |
|---|---|---|---|
| 1 | 003 T006 after bounded pilot v1 | "Draft pilot v2, unfunded" | The [unfunded v2 draft](../specs/003-agent-retrieval-context/contracts/bounded-pilot-v2.md) records the design and v1 gap requirements; task/sample/launch inputs and apparatus repairs remain before freeze. No spend until the owner authorizes a key, rights and a budget. |
| 8 | 003 T007–T009 and 008 T003–T005 | "Measure them in pilot v2" | Their outcome tasks join the T006 suite; 008 T004's semantic triggers get built; all six stay open until an authorized run passes. |
| 17 | `pilot-capture` apparatus and its embedded TypeScript bridge | "Keep, with an exception" | A narrow architecture exception for embedded host-extension source in an opt-in feature; the local gates gained a `pilot-capture` step with the main merge (see below). |
| 19 | F2 local 30B pair and F3 economics | "Retire F2; F3 to T006" | F2 is retired; F3's economics gate points to T006 and pilot v2. |

**Hosts, evidence and credentials**

| # | Item | Owner's choice | Result |
|---|---|---|---|
| 3 | Where the work runs | "Mac only, drop sandbox" | The owner's Mac (Apple M5 Max, 64 GiB) is the work host; cloud sessions remain a fallback; Linux-only jobs stay parked until the owner provides a Linux host. |
| 6 | The 2026-10-11 sandbox and its resume archive | "Declare both lost" | Items that needed the archive are retired or resettled (F2, F6, F8, F10, F11). |
| 18 | F6 frozen Rust/Go label sets | "Retire F6 as a gate" | G1, G2 and F4-style sealed sets are the only retrieval instruments. |
| 7, 30 | Backups of the G2/F4/G1 sets and pilot evidence | "Encrypted off-machine copy", "Encrypted, on disk for now" | Done on local disk: five encrypted archives and checksum readback pass ([receipt](#encrypted-evidence-backups--2026-11-11)). The owner copies them off the Mac and performs authenticated recovery. Pilot evidence is kept until T006 is accepted, retired or superseded. |
| 4 | Pilot v1 API key | "Revoke it, delete the file" | Done: the local key file and its empty directory were deleted. The owner revokes the key in the Console. |

**Dependencies**

| # | Item | Owner's choice | Result |
|---|---|---|---|
| 2 | rustls 0.23.45 alerts, PR #49 and the main merge | "Bump rustls, then merge" | rustls 0.23.46 on experimental, one gates run, one PR to main merged by the owner with a merge commit. Done: PR #52 merged as `d94b013` on 2026-11-07; alerts #9–#12 closed and PR #49 closed as superseded. |
| 5 | zip 0.6.6 alerts via the optional tch worker | "Dismiss as unused" | Done: Dependabot alerts #13–#16 dismissed as not used, with a recheck if 013 is unfrozen or npz loading is added. |
| 21 | tree-sitter-scala 0.26.4, tree-sitter-powershell 0.26.6, pulldown-cmark 0.13.6 | "One bump, gated on unit diff" | Done on `68ed776`, with search schema 9 and units7. Complete fixture and both G2-corpus snapshots are identical; the conditional fresh G2 run is not triggered. Focused checks and all eight local gates pass ([receipt](#parser-bundle-and-complete-unit-comparison--2026-11-11)). |
| 22 | Tantivy 0.27.0 (PR #47) | "Apply only if neutral" | Done: existing 0.26 stores open without repair, all G1 result/response bytes are identical, and all eight local gates pass on `2929769` ([receipt](#tantivy-027-store-compatibility-and-g1-neutrality--2026-11-11)). Accepted for experimental. |
| 23 | tree-sitter-dart 0.3.0 (PR #48) | "Close; stay on 0.2.0" | Done: PR #48 closed with the reason. |

**Retired, closed or kept**

| # | Item | Owner's choice | Result |
|---|---|---|---|
| 28 | Handoff rows 5 and 6 (record-only timings) | "Retire both" | No compute is spent; the handoff stays open for row 4 and newer checks. |
| 26 | Nemotron versus EmbeddingGemma comparison | "Close it; Gemma only" | EmbeddingGemma 2 only. |
| 31 | Foundry's 10.4 GB of Nemotron stores and GGUF | "Keep them" | Kept as historical evidence. |
| 27 | `overview-v1` recipe | "Keep as the low-cost recipe" | No change. |
| 29 | F1 historical CI re-run | "Drop the historical re-run" | When Actions returns: one run on the then-current heads plus the two action bumps. |
| 25 | 2026-10-25 audit items 3–5 and three pivots | "Defer, with triggers" | See below. |

**Re-entry triggers for the deferred audit items.**

- Item 3, explaining which changed dependency triggered regeneration: when a
  measured regeneration cost or an operator question needs that explanation.
- Item 4, cross-host resume: when a concrete host needs a task resumed on
  another host; then an adapter for that host only.
- Item 5, stable prompt prefixes: only inside a paid authorization, such as
  pilot v2, that measures the hosted cache benefit.
- The pivots (resume companion, task capsules, demand-driven enrichment): only
  after T006 shows task benefit in an authorized run.

**Done on 2026-11-06, checked.**

- GitHub: alerts #13–#16 read back as `dismissed` (`not_used`); PR #48 reads
  back `closed`. Open alerts are now the four rustls ones (#9–#12).
- The pilot key file is gone: `~/.config/cf-pilot` no longer exists.
- Pilot evidence: 357 files and directories readable by group or others were
  tightened to 0700/0600, as contract §10 requires; none remain.
- The offline oracle rerun (1A, 1B, 3A, seed) and the replay script were copied
  from the session scratchpad into the private pilot folder, with a SHA-256
  list of 74 files, before macOS cleanup could delete them.

**Main merge preparation (decisions 2 and 17).** `8600fc0` bumps rustls to
0.23.46; its lockfile change is identical to Dependabot PR #49 (cargo's MSRV-aware
resolution also moves bindgen to syn 2.0.119 and tempfile to getrandom 0.3.4).
`854cf88` adds three `pilot-capture` gates to `scripts/gates.sh` and fixes the two
Clippy lints they found in `src/pilot_capture/wire.rs` test code; the earlier
qualification receipt had not run Clippy on that feature's test targets. Gates ran
on the owner's Mac (Apple M5 Max) with Rust 1.97.1 and 1.90.0, the macOS 26.5 SDK,
on clean trees; logs are private, under
`datasets/context-foundry-gates/<commit>-2026-11-06/`.

- `8600fc0` (01:13–01:30 UTC): the five gates then in the script all exit 0;
  **1,314 passed, 0 failed, 34 ignored**. Log SHA-256 prefixes: Clippy `2d948dd2…`,
  test `dfb51fd8…`, MSRV check `37b27708…`, MSRV Clippy `cd0f5a0a…`. Run separately
  afterward, `pilot-capture` Clippy failed on the two lints, and its unit tests
  passed 62.
- `854cf88` (01:33–01:52 UTC): fmt, Clippy, `pilot-clippy`, `pilot-test` (62
  passed), MSRV check, MSRV Clippy and `msrv-pilot-check` exit 0. `test` exits 101:
  **1,312 passed, 2 failed, 34 ignored**. Log SHA-256 prefixes: Clippy `35843436…`,
  test `8bbfca51…`, pilot Clippy `b18a1c35…`, pilot tests `2f6725bb…`, MSRV check
  `d79e2fea…`, MSRV Clippy `32d47f79…`, MSRV pilot check `2974fd84…`.
- The two failures are timing tests. `artifact_verification_obeys_caller_cancellation`
  passed 3 of 3 isolated reruns. The 6,000-file watcher test (F18) left 2,680, then
  3,248 files pending index after its 180-second deadline, in the gate run and in an
  isolated rerun. The default build is the same as at `8600fc0`, where it passed:
  since then only feature-gated test code changed. Neither test was loosened; F18
  stays open ("Native Mac run on rust" requires fixing it first).
- Not run: CI (Actions unavailable), native macOS acceptance (F7, to do) and the
  `pilot-capture` actual-host suite (not a gate).
- Merged 2026-11-07: the owner asked for PR #52 to be merged, and it landed on `main`
  as merge commit `d94b013` (parents `69c3bb0` and `98adfe5`; its tree equals
  `98adfe5`). No workflow run started. Dependabot then closed alerts #9–#12, and
  PR #49 was closed as superseded.

One observation for the owner, with no change made: Dependabot pull requests
still start hosted workflow runs on their own branches (for example on
2026-11-05 and 2026-11-06), although pushes from this line use `[skip ci]`.

## Paid transport probe and bounded coding pilot — 2026-11-06

**Verdicts, kept separate as contract §12 requires.**

- **Apparatus:** a completed, interpretable pilot. All eight slots reached a
  terminal outcome and were retained. All 60 provider HTTP attempts are
  accounted for, primary usage is complete for all 30 generations, and the
  source and oracle bindings are valid. No halt occurred and no capture,
  containment or provenance breach is unresolved. The binding gaps listed
  below are disclosed rather than claimed closed.
- **Coding:** all 8 slots passed the independent oracle, arm A 4/4 and arm B
  4/4, so B−A = 0 in every pair. The model made essentially the same repair in
  both arms, so this task is too easy to show any effect of the Foundry view on
  correctness.
- **Claim ceiling:** descriptive only. One task was repeated four times per
  arm. There is no significance, interval, noninferiority, causal savings,
  generalization, T006 acceptance, default promotion or invoiced-cost claim.

Coding provider and model: Anthropic `claude-sonnet-5-5` through OMP 18.13.2,
with adaptive thinking, `xhigh` effort, a 4,096-token output cap and standard
service. Native sessions record `resolvedModelIsFallback=false`.

The implementation and this record are by Claude Code (Opus 5.5). Every check
below was done by in-session agents (self-review), not independently.

### Authorization and probe

The owner provided:
- the credential file;
- the billing identity, an Anthropic workspace that is pinned in the ratecard
  only by SHA-256;
- confirmation that the personal key is scoped to that workspace. The frozen
  transport sends no `anthropic-workspace-id` header, so an unscoped key would
  have failed.

The owner then approved the rights and two scoped authorizations, each naming
its pins:

| Scope | Pins |
| --- | --- |
| Probe only | contract `1a00758f…502d`, helper `5ed9a163…64b3`, probe manifest `a15d9654…5474`, ratecard `507fe2f0…841e` |
| Coding | the same contract, helper and ratecard, plus coding manifest `50184eb7…1cdd` and the probe seal `dcf8869d…d8f0` |

Before any spend, in-session agents replayed every admission gate against the
helper's code and files (92 of 92 passed). The helper claims a receipt before it
reads the credential, so the credential's shape was checked locally first.

The probe ran 13:14:57–13:15:06 UTC. It made two HTTP attempts, one CountTokens
and one Messages, with no retry. It ended `end_turn` with 17 output tokens and
no tool use. Its 5,239 input tokens matched the count exactly: 4 were uncached
and 5,235 were five-minute cache writes. The seal binds:
- the 17-record journal (`6d6b8ab2…d076`);
- the returned model and settings;
- the request and message identities;
- the boot identity;
- the paid clock.

### Coding run

The coding run took 170 s, from 13:31:51 to 13:34:41 UTC. It continued the
probe's hash-chained journal (331 records, `801c76e7…`) through the verified
handoff. Slot order was 1B, 1A, 2B, 2A, 3A, 3B, 4A, 4B.

Every final generation ended `end_turn`. No slot hit a cap, and no request was
truncated or refused for its input size.

| Arm | Passed | Inclusive input, median (range) | Output, median (range) | Host wall, median (range) | Generations by pair | Tool calls by pair |
| --- | ---: | --- | --- | --- | --- | --- |
| A | 4/4 | 43,837.5 (43,494–44,126) | 1,521.5 (1,273–1,650) | 21.0 s (18.8–22.5) | 4 / 4 / 4 / 4 | 3 / 3 / 4 / 4 |
| B | 4/4 | 32,837 (31,429–55,155) | 1,251.5 (1,207–1,590) | 16.8 s (14.4–22.5) | 4 / 3 / 3 / 3 | 3 / 2 / 2 / 2 |

The denominator is 4 in every cell, with no missing or partial observations.

- **Totals.** Inclusive input was 175,295 tokens for A and 152,258 for B, 13%
  lower for B. Pair 1 reverses this: 1B used 55,155 tokens against 1A's 43,628.
- **Cache counters.** A had 40 uncached, 37,951 cache-write and 137,304
  cache-read tokens; B had 34, 39,857 and 112,367.
  - The provider applied five-minute writes: native usage records them as
    `ephemeral5m`, and the transport omits the TTL.
  - Every request shared the same stable prefix, system, tools and settings
    hashes.
- **Usage scope.** These usage figures are the pilot's own live aggregate
  observation of the wire. They are not invoice data.
- **Money.** Budget-rate spend was 3,609,995 microUSD, 36.1% of the USD 10
  envelope, including the probe's 52,815.
  - The calculated charge at the pinned ratecard is about USD 0.346: A 0.168,
    B 0.164 and probe 0.013. It is not an invoice and excludes taxes, fees and
    discounts.
  - OMP's native cost fields agree with it to 1e-6 USD.
- **Limits.** Totals stayed within every cap:
  - 30 permits, 30 counts and 60 HTTP attempts against caps of 49, 49 and 107;
  - at most 4 generations in any slot;
  - a highest slot spend of 591,300 against 1,000,000;
  - at most 818 output tokens in a generation, against 4,096;
  - a largest count of 17,164 tokens, against 30,720.
- **Paid wall.** 1,181 s of 7,200 s were used. The 1,005 s between the probe
  seal and the coding handoff mixes the owner's approval wait with operator
  work, and the journal does not record the split.
- **B's allocated subtotal.** Preparing each pair's view took 3.8–4.1 s
  (index, prepare and context, with 112 embedded document units per pair). The
  query-embedding count was not recorded and is unknown. Adding the preparation
  time brings B's median wall to about 20.6 s, against A's 21.0 s.

### How to read the arm differences

- **Slot order and cache.** The probe did not warm the cache for 1B: the
  1,005 s gap exceeded the five-minute TTL, and 1B's first generation read 0
  cached tokens. 1B instead wrote the 4,849-token shared prefix that every later
  slot read. That costs B about USD 0.0116 at the ratecard, more than the
  A−B gap of USD 0.0044. No cache-independent effect is claimed.
- **B is not one treatment.** Views 1–3 differ only in store-identity citation
  suffixes. View 4 adds a tenth block, `Cargo.toml` (2,041 against about
  1,730 tokens).
- **Arm A's evidence slot** holds the pinned placeholder that tells the model
  to inspect the workspace with its file tools.
- **Budget headroom.** B's injected context limited its headroom: 1B could not
  have admitted a fifth generation, which every A slot could have.
- **Repairs.** Seven candidates use the upstream clone-then-commit fix, and all
  seven compile to a byte-identical oracle binary. 3A peeks instead, which is
  equivalent.
- **The oracle is not vacuous.**
  - It fails the seed with exit 101 at input `%A`.
  - Every pass printed the full 131,598-case PASS line.
  - An offline rerun of 1A, 1B and 3A reproduced their check binaries.
  - Replaying each slot's single edit onto the seed reproduces its checked
    source, which confirms each slot was restored from the seed.

**Two blocked reads.** In 3A and 4A, the model's first generation issued
`read` of `.`, a listing of the workspace root, alongside its read of
`src/lib.rs`. Canonical read admission covers only the five corpus files, so the
guard refused both calls, with no effect. The prompt does not forbid the read.

- Only arm A saw these refusals, which plausibly follows from its "inspect the
  workspace" placeholder.
- **Owner decision, 2026-11-06:** "Keep as recorded". They are constraint
  violations under §10. The tool was the advertised `read`, so they are not
  "forbidden-tool attempts" under §9/§11, and the oracle outcomes stand
  (A 4/4).
- **Stricter reading, not applied:** counting them as forbidden-tool attempts
  would have made arm A 2/4 and B−A +1 in pairs 3 and 4.

One journaled consumer cancellation (2A, final generation) was drained to the
terminal event and reconciled with complete usage.

### Containment and disclosed gaps

**Host startup traffic.** Each OMP host, at startup and outside any model tool
call, tried:
- an `api.anthropic.com` connection prewarm, which failed with `ENOTFOUND`;
- loopback model discovery on ports 11434, 1234 and 8080, which failed with
  `EPERM`.

The sandbox denied these before any bytes were sent, and the journal does not
record them. The host profile is `(allow default)(deny network*)` plus path
denials. It is not a jail.

**Disclosed gaps:**
- **Checker receipts bind environment-variable names only.** They omit the
  values and the `sandbox-exec` profile. They take the oracle and compiler
  hashes from policy constants; the copies on disk were rehashed separately and
  match.
- **Every compile step wrote some stderr, and only its hash was kept.** The
  cause is unknown; the compiled binaries are byte-identical to clean rebuilds.
- **The coding authority digest sits outside the hash chain.** It is recorded
  only in its claim marker. `coding-admission.json` names a `runs/coding`
  directory that the handoff never uses; the run continued in `runs/probe`.
- **Some files are not 0600/0700.** Store files and OMP-created files have
  wider permissions; the pilot root is 0700.
- **Failure paths not exercised.**
  - A credential rejected after the receipt claim leaves only the claim marker.
  - A workspace-restore failure exits without journaling a halt.
- **`seal_slot` decides pass from exit codes alone.** It does not check the
  PASS line, which matched here.

**Key leak scan.** No file under the runs, checks, archives, workspaces, helper
logs or `docs/` contains the key or any of its overlapping 24-character
fragments: 410 files, about 1 GB, were scanned.

**Private evidence:** `pilot-run-v1-69f88b`, including `runs/probe`, `checks`,
`archive` and `authority`, plus `live-capture-v1-ztmPDT/{probe,coding}-run-2026-11-06`.

## Pilot apparatus and frozen launch inputs — 2026-11-06

**Verdict: everything before the first paid request is built, qualified and
frozen.** Contract gate 4 of the
[bounded pilot v1 contract](../specs/003-agent-retrieval-context/contracts/bounded-pilot-v1.md)
is met; gates 2, 3 and 5 wait only on owner-held inputs (credential, account
identity, rights approval and an explicit spend authorization). No provider
request, real credential or coding trial occurred.

[`pilot_capture::apparatus`](../src/pilot_capture/apparatus.rs) and the
`materialize`, `manifests`, `probe` and `coding` commands of
[`foundry-pilot-capture`](../src/bin/foundry_pilot_capture.rs) add:

- verification of every pinned input, including the profile-declared embedding
  worker, model and tokenizer, which the freeze had declared but not rehashed
  (all three match);
- the four pair views, each from a fresh seed workspace and isolated store with
  the pinned release Foundry, profile and development isolation;
- real renders of all nine slots and the frozen probe and coding manifests
  (17 re-verified pins, the views, per-slot render commitments, and the pilot
  root as the host's isolation root);
- a contained independent checker: the accepted four-step recipe on a copied
  candidate under `sandbox-exec` (no network, owner home unreadable, minimal
  environment, 60 s). The accepted positive control passes and the seeded
  negative fails with the exact recorded assertion;
- the executor (probe, then the eight frozen slots with workspace restore,
  archiving, checks and seals) and a descriptive report with no inference.

Live admission now also requires the billed account and pricing evidence pinned
by SHA-256 in the ratecard, and `rights_approved` in the owner receipt; a test
walks every live gate in order and confirms one receipt admits one run.

**Full-pipeline qualification (no provider).** One run used real views,
renders, host, sandbox and checker with only the model scripted: the probe
sealed, the journal-verified handoff continued it, and all eight slots sealed.
Arm B applied the upstream repair through the native edit tool and passed 4/4;
arm A made no edit and failed 0/4; B−A was +1 in every pair; 13 permits.

**Frozen inputs (private, `pilot-run-v1-69f88b`).** Each view took about 2.5 s
to prepare and 1.4 s to deliver, all `semantic:ready`:

| Pair | Bytes | o200k tokens | Shown items |
| ---: | ---: | ---: | ---: |
| 1 | 6,428 | 1,729 | 9 |
| 2 | 6,428 | 1,738 | 9 |
| 3 | 6,428 | 1,738 | 9 |
| 4 | 7,577 | 2,041 | 10 |

The views differ by pair only through the workspace identity in each evidence
handle. That changes handle token lengths, and pair 4's packing fits one more
item. The contract treats pair views as setup-derived facts; report this when
interpreting arm B.

Pricing evidence, from the official pricing and token-counting pages: Claude
Sonnet 5.5 costs $2 input, $2.50 five-minute cache write, $0.10 cache read and
$10 output per million tokens, all within the frozen $10/$25 budget rates.
CountTokens is free, separately rate-limited, an estimate, and never uses the
prompt cache. The release helper is frozen in the pilot root
(`5ed9a163…64b3`); the release `foundry` binary is unchanged.

Gates for this change: `cargo fmt --check` and both clippy gates pass; 62 unit
and 16 actual-host tests pass, including the full pipeline (`29a2bcf`).

## Live-capture implementation and no-provider qualification — 2026-11-05

**Verdict: the approved capture boundary is implemented and passes its actual-host
no-provider qualification; missing gate 1 of the
[bounded pilot v1 contract](../specs/003-agent-retrieval-context/contracts/bounded-pilot-v1.md)
(SHA-256 `1a00758f…502d`, byte-unchanged) is complete by owner decision.** It made
no provider request, read no real credential, materialized no Foundry view and ran
no coding trial. Gates 2–5 (real compatibility, ratecard/account/rights, launch
manifest, owner authorization) remain open, so live execution is still locked.

The opt-in `pilot-capture` feature adds one module and one helper; the default
build and the release `foundry` binary are unchanged:

- [`src/pilot_capture.rs`](../src/pilot_capture.rs): the non-cloneable
  `CaptureSession` (strictly separate verification and live admissions, live
  gates refused by name before any credential read, one-run authority claim,
  hash-chained fsynced journal, checked microUSD reservations, permits,
  deadlines, monotonic halt, slot driver, preview `render_binding`, independent
  check seals, probe seal and clean probe-to-coding handoff).
- [`protocol.rs`](../src/pilot_capture/protocol.rs) (closed frames over the
  existing `neural::protocol` framing), [`host.rs`](../src/pilot_capture/host.rs)
  (network-denied `sandbox-exec` launch, inode-bound pipes on fds 3/4, RPC, tool
  admission ported from the accepted apparatus guard, native session read-back),
  [`wire.rs`](../src/pilot_capture/wire.rs) (final-body validation, CountTokens
  projection, direct HTTPS without proxy/redirect/retry, whole-event SSE judging
  with error withholding, lossless `u64` usage) and the thin
  [`bridge.ts`](../src/pilot_capture/bridge.ts) (credential-free, delegates to the
  host-bundled `streamAnthropic`).
- [`foundry-pilot-capture`](../src/bin/foundry_pilot_capture.rs): `preflight`,
  `render` and the future `probe`, which today refuses with
  `authority_receipt_missing` before claiming anything.

Qualification used the pinned OMP 18.13.2 (`38c36452…3d0f`), the embedded bridge,
a fresh private host root per slot and an owned numeric-loopback TLS fixture
(throwaway CA minted per test) with an owned synthetic credential. Results:

| Stage 1 case | Evidence |
| --- | --- |
| Native bootstrap | Bundled import, keyless catalog clone (custom API, `max_tokens` 4096, `xhigh`, exactly `read`/`edit`) and settings verified through `get_state`; a bridge load failure aborts host startup before RPC ready (`--trusted-extension`) |
| Ready and private descriptors | Ready precedes the sole prompt; fds 3/4 bound to Rust's pipe inodes; EOF/truncation fail closed |
| Keyless / no bypass | No registry credential; the host's own network attempts (local-model discovery and an unconditional connection prewarm `HEAD` to the model origin) are denied by the sandbox before name resolution |
| Count → Messages | Exactly one CountTokens then one unchanged Messages body per permit; projection omits output/transport controls and the thinking binding |
| Native response path | Native SDK consumes accepted bytes; `responseId` joins the SSE message id exactly once |
| Retry refusal | Failed status, withheld error event, failed count and input-cap refusal each produce no second request |
| Credential-echo privacy | Canary in non-success body/headers/`request-id` and in split or coalesced SSE error events never reaches the host root, journal or capture; failure is never a clean EOF |
| Normal exit and containment | RPC EOF → normal exit, process group gone, no forced kill |

The bounded suite also covers a legitimate tool followup with a fresh permit, a
blocked out-of-scope edit (behaviour failure, protected files unchanged), missing
final usage (reservation retained, halt), journal failure before transmission,
preview rendering with no request, pair render matching, independent check seals
and recipe/truncation provenance faults, and the sealed handoff (tampered or
inflated seals and non-owned verification keys refused).

Pinned-host behaviours the implementation now handles explicitly:
request bodies are gzipped unless `PI_REQUEST_COMPRESSION=0` (disabled; any
`content-encoding` is refused, which also removes the host's silent uncompressed
re-send); the native client cancels the body right after `message_stop` (Rust
keeps reading upstream to a verified terminal); blocked tool calls get no
`tool_result` hook (Rust's refusal completes them); the reminder shows the
working directory without macOS's `/private` prefix (exact display mapping).

An in-session four-lens adversarial review raised 34 findings; 26 survived
per-finding refutation and all 26 are fixed, including tool completions erased at
settlement, unbounded reuse of one authority receipt, a probe seal invalidated by a
later record, non-halting pin/launch faults, the probe wall and a
credential-shaped upstream `request-id`. That review is self-review, not
independent review, and it is the implementation's review: on 2026-11-05 the owner
decided "no independent reviews needed - go complete and remove the gate of independent reviews". Gate 1 (implementation and review of the capture boundary) is
therefore complete; no cross-lab review ran or is pending.

Receipt (Rust 1.97.1 Homebrew; source as committed on base `957aacb`):

| Gate | Result | Log SHA-256 |
| --- | --- | --- |
| `cargo fmt --check` | pass | `e3b0c442…b855` |
| `cargo clippy --locked --all-targets -- -D warnings` | pass | `f2066ca2…49ec` |
| `cargo clippy --locked --features pilot-capture … -- -D warnings` | pass | `fecede7c…2ffe` |
| Unit tests (`--features pilot-capture --lib pilot_capture`) | 58 passed | `1fe1d804…84ce` |
| Actual-host suite (`--test pilot_capture`) | 14 passed | `705087d1…2b6b` |

Not run: `scripts/gates.sh` (no merge), the Rust 1.90 MSRV clippy and the full
default test suite; native macOS acceptance is out of scope. The private
implementation receipt and logs are in the live-capture evidence root.

## No-provider apparatus preflight — 2026-10-29

**Verdict: the full no-provider fixture preflight passes.** These are
installed-consumer apparatus runs with fixed scripted loopback responses—not
model-written coding trials or measurements of provider consumption/economics.
The owner removed the Anthropic-only author restriction; scoped corrections used
GLM-5.3, with OpenAI Sol/Astra review. Those reviewers share one provider and are
independent of the Z.ai author.

| Attempt | Passed / declared cases | Wall seconds | Outcome |
| --- | ---: | ---: | --- |
| First | 6 / 25 | 1,276.32 | Undeclared SSE encoder; startup catalog-control accounting |
| Corrected | 23 / 25 | 268.42 | JavaScript-safe integer/import domain and native-refusal attribution |
| Final | 25 / 25 | 268.14 | Full declared inventory and final reconciliation pass |

The pinned OMP 18.13.2, release Foundry and Rust 1.97.1 binaries ran with fresh
private HOME/XDG/TMP directories, a cleared child environment and dummy credentials.
The process-birth macOS policy allowed only the fixture TCP port, denied other
network destinations and restricted real-home data access. Live fixture/decoy and
harmless canary controls passed; no actual credential contents were opened.
The policy includes named securityd denials, but no dynamic keychain proof is claimed.
This controlled apparatus is not a production jail.

All three runs passed the four controller checks: network isolation, home-data isolation,
real launch-pin tamper refusal, and externally compiled positive/seeded-negative
oracle controls. The host passed later-payload-mutation and wrong-role-prompt
rejection before transport. Admitted request bytes showed exactly `read`/`edit`,
the selected model, adaptive thinking and `output_config.effort: "xhigh"`.

The first run's fixture writer referenced an undeclared encoder. Startup
`GET /v1/models` discovery was also recorded as an unexpected path. The corrected
run delivered SSE and exercised actual read/edit effects, selector-shadow refusal,
usage missingness, split frames and open-body completion. Late/cancelled Messages
attempts are now checked against the immutable inventory used by each case, again
after bounded handler shutdown; final catalog-auth and persistence state is reconciled.
The open-body case completed through independent client cancellation before normal
host exit; it does not prove that the HTTP body remained open until exit.

The second run exposed two apparatus mismatches. The final controller losslessly
parses Rust's valid `u64` import aggregates through original JSON integer lexemes,
checks exact BigInt arithmetic and retains oversized evidence as tagged decimal
strings plus complete raw stdout. Guard normalization still remains unknown at
its JavaScript-safe overflow boundary; production accounting is unchanged.
The clean host's identity-bound `Tool locate not found` result is accepted only
with the exact single-plain-text-block shape and refusal digest. The recorded
boundary is the host registry, not a fictional guard hook; the out-of-scope read
is separately vetoed by the guard. No fake native tool or ambient plugin was added.

In the successful baseline fixture, the actual wire thinking count is captured,
but the native host still omits `reasoningTokens`; production import correctly
remains `complete:false`. A separate, explicitly derived fixture session imports
as complete only with strictly correlated observed wire counters. Native JSONL is
unchanged; missing native reasoning is not a measured zero, and reasoning is not
added again to inclusive output. This synthetic proof does not establish what
the provider emitted in the earlier live calibration.
Native usage-record counts are not request counts: the overflow fixture retained
two usage-bearing native entries for one endpoint response. Their arithmetic is
imported as recorded, not rewritten into request parity or billed consumption.

The final receipt reconciles all Messages attempts with no pending handlers,
late inventory drift, unexpected catalog auth or evidence-append failures. The
operator additionally verified the hashes of all 29 complete raw import-output
artifacts. Native and derived imports remain separate.
A post-run Sol review inspected the manifest, selected raw transport/native/tool
and oracle proofs, and accepted only this bounded no-provider apparatus scope.

All cleanups reported no unresolved owned groups and stopped listeners. The
operator confirmed all three canary paths absent with `lstat` and all 135 original
calibration file pins unchanged. Evidence and exact executed source snapshots remain
outside Git under `datasets/context-foundry-task-validation/apparatus-v2-DkGJgT/`;
run directories are `preflight-runs/run-mvt80879-mfwlf/`,
`preflight-runs/run-mvtgh5lg-2ayg4z/` and
`preflight-runs/run-mvthtmxg-vpxs7/`. The final controller/guard SHA-256 prefixes
are `112d1aa1…` / `d8999597…`. Failed attempts remain failed and retained.
Elapsed times are diagnostic apparatus time, not coding latency.

This closes the approved apparatus-only preflight, not T006 or a clean coding
comparison. The guard remains fixture-only, with dummy auth and live origins
rejected; it is not an authorized live-capture adapter. A live comparison still
needs an approved capture/claim boundary, cache treatment, sampling/repetition and
uncertainty plan, complete phase/usage accounting, spend/stop rules and explicit
authorization. No real inference, forwarding, production accounting change or
coding-comparison authorization follows from these fixture runs.

## Paired coding task and actual host usage — 2026-10-28

**Verdict:** both model-written repairs pass a frozen, hash-pinned external
correctness oracle, and actual host input/output/cache usage is captured and reconciled.
This is one public seeded calibration, not held-out task-quality acceptance.
The frozen read/edit-only tool recipe was not enforced by the host, and its
reasoning-token breakdown is missing. T006 and economic acceptance remain open.

**Apparatus.** OMP 18.13.2 (`38c36452…`) ran Anthropic
`claude-sonnet-5-5` at `xhigh` in both arms, with no model fallback. The task reuses
the public `percent-encoding` 2.3.2 scenario from
[`tests/gateway/host_task.rs`](../tests/gateway/host_task.rs): malformed escapes
must not consume bytes, while valid escapes, no-alloc support, UTF-8 conversion
and unchanged borrowing remain correct. The pinned upstream archive is
`9b4f627cb1b25917193a259e49bdad08f671f8d9708acfd5fe0a8c1455d87220`.
Only private copies were seeded or edited; the registry and project code were not.

The operator recorded protocol `3e55bde1…` before model execution, with matching
seeded files, task, model, settings and checker; the recorded random draw selected
baseline then assisted. Freeze chronology and the draw procedure are operator
provenance, not independently attested by the saved runner, which rechecks seed
hashes but not every launch pin. Each arm had one attempt, a 600-second host limit
and a USD 5 host-reported admission ceiling, not a guarantee about invoiced or
in-flight cost. The intended baseline was ordinary file tools without injected
evidence. Assisted received the release CLI's 2,031-`o200k_base`-token semantic view.
Both actually exposed the larger catalog described below. The operator reports no
synthetic source expansion, manual arm retry, post-outcome query retuning or ranking change.

The external oracle (`f5ac14d3…`) passed the untouched upstream control and
rejected both seeded inputs before the runs. Afterward, both candidates compiled
with no-alloc and alloc, passed 131,598 byte-case invocations plus UTF-8/encoding checks, and
kept every protected file unchanged. Actual model reads/edits stayed within the
five-file workspace; neither model saw the checker or another repository's source.
The host's filesystem scope was instruction-based and audited, not an OS jail.
The compilation JSON retains results, not compiler argv/version or an explicit
candidate/linkage manifest; those invocations are operator execution evidence,
not a self-contained compiler replay bundle.


| Observation | Baseline (intended file tools) | Assisted (injected context) |
| --- | ---: | ---: |
| External correctness oracle | PASS | PASS |
| Completed assistant usage records | 5 | 4 |
| Source read calls | 2 | 1 |
| Input tokens, including cache | 70,724 | 60,576 |
| Cache-read tokens, input subset | 52,062 | 51,744 |
| Cache-write tokens, input subset | 18,650 | 8,822 |
| Output tokens | 1,595 | 1,287 |
| Input plus output tokens | 72,319 | 61,863 |
| Host wall time, seconds | 16.743 | 12.739 |
| Known execution subtotal, seconds | 17.065 | 16.991 |
| Host-catalog estimated USD, not billed charges | 0.0957802 | 0.0533524 |

The execution subtotal adds the external after-check and, for assisted, 2.666 s
of fresh indexing/preparation and 1.364 s of retrieval. Experiment construction,
positive/negative controls, operator model work and energy are outside that subtotal,
not zero. Local preparation separately embedded 112 vectors through 15 document
calls and 18,841 input tokens. These are not Anthropic tokens or equivalent costs.
Query-embedding token usage and local energy are not exposed by this CLI and remain
unknown. No model acquisition was performed.

**Receipt reconciliation.** All nine completed assistant records have actual
input/output/cache counts; native totals match their sums and the usage objects
in persisted sessions match those in the final JSON events. Complete messages
are not field-identical: persisted messages also contain `contextSnapshot`.
Production `usage import --host omp` returns the same normalized totals with no
unparsed or unattributed results. Reasoning-token counts—not reasoning activity—
are absent from the retained host usage objects: import correctly reports
`complete:false` with 5/4 missing reasoning categories. Its numeric zero is not a
measured zero, and the current completeness contract is unchanged.
Anthropic [documents inclusive output usage](https://platform.claude.com/docs/en/build-with-claude/thinking-steering-and-cost#pricing)
and an `output_tokens_details.thinking_tokens` subset on the final streaming
event. Missing host breakdown does not itself prove missing aggregate output or
provider omission; no raw provider stream was captured in this calibration.
Host-catalog prices and the one-hour cache-write TTL do not establish billed charges.
Hosted cache state was not reset; first-request cache conditions differed.
The exact provenance and causal contribution of cache reuse are not identified.
The apparent differences therefore do not establish causal or general savings.

**Protocol deviation.** Despite `--tools read,edit`, OMP advertised those tools
plus native `locate`, `context`, `status`, `remember`, `retrieve` and `history`.
Both models attempted `locate` once; each returned a no-daemon error and no source.
Those failed tool turns and their usage remain included. Effective catalogs matched
between arms, but neither run satisfies the frozen read/edit-only recipe. The host
filtering issue was reported; the pair was not rerun to erase that result.

**Context boundary.** Release Foundry (`9dcbec8f…`), the existing pinned local
profile and `--development-isolation` produced `semantic:ready`; its trace records
64 dense candidates at revision 5. The injected view matches the assisted host's
persisted user message. It includes the iterator caller, but does not cover the
repair helper at bytes `8535..8747`; the model still read source. This is operator-
injected CLI context, not proof of autonomous tool adoption, enforced host routing
or provider-wire application hooks.

**Post-repair freshness diagnostic.** The pre-edit handle still retrieves the
accepted revision-5 snapshot before re-indexing, as
[001 FR-003](../specs/001-source-state-recovery/spec.md) requires; an initial
diagnostic incorrectly expected live-file invalidation. After explicit indexing
accepts revision 6, that handle returns `stale_handle` with no source bytes.
Semantic maintenance then completes with one new vector/197 document tokens,
retaining the old cached input. This additional re-index/probe/refit work took
1.299 s and is reported separately, not hidden in the primary paired subtotal.

Raw prompts, sessions, patches, controls, protocol, checker, traces, imports and
`report.json` are retained outside Git under
`datasets/context-foundry-task-validation/paired-lFCMwZ/`. Session SHA-256 prefixes
are baseline `3c5b0a54…` and assisted `01241341…`. No credentials, raw transcripts,
weights or upstream sources were added to Git. No product default, persistent/global
host configuration, product code or acceptance threshold changed; command-local
overlay settings and flags were used as recorded.

## Markdown parser security and unit migration — 2026-10-26

The owner approved a targeted security follow-up to the cleanup below, not the
whole dependency group in PR #44. Accepted source commit: `2651fdb`, based on
`bb79aee`. Only pulldown-cmark moves, from 0.13.4 to 0.13.5. Scala 0.26.3,
rmcp 3.7.0 and uuid 1.29.0 remain deferred. GLM authoring was authorized only for
this follow-up; GPT-6.1 Sol independently reviewed its source and regressions.
GPT-6 Astra additionally reviewed the parser exposure and migration boundaries.
Sol and Astra share a provider; neither is counted as a second independent lab.

**Exposure.** The exact 13-byte input `- [o]:u\n    \t` makes the upstream default
offset iterator and the old production Foundry outline retrieve exit 101.
Indexing's per-source panic catch retained the source with a parse failure, but
does not protect later outline/context reparses. Source review identified the
shared owner's engine-lock poisoning risk; this was not an observed before-fix
MCP exploit. The upstream fixes also address default-parser complexity; no local
complexity timing or disabled-wikilink exploit is claimed.
Upstream references: [panic report #1179](https://github.com/pulldown-cmark/pulldown-cmark/issues/1179)
and [fix #1181](https://github.com/pulldown-cmark/pulldown-cmark/pull/1181).

**Migration.** The offset change observed in the preceding cleanup requires
`SEARCH_SCHEMA` **7 → 8** and `SOURCE_UNIT_SCHEMA` **5 → 6**. Authoritative store
schema **12** and generation format **6** do not change. Existing stores refuse
lexical use until explicit `foundry --store <store> repair-index`. Old semantic
recipes remain retained but ineligible until `semantic prepare` refits them.
Profileless replay and cheap progress also reject obsolete unit identities rather
than replaying or reporting them current. Exact matching embedding inputs remain
reusable; no cache purge, source rewrite, automatic repair or new table is added.
See the [unit-grammar contract](../specs/001-source-state-recovery/contracts/context-v2.md#unit-grammar-repair--2026-10-26).

**Checks.** All five `scripts/gates.sh` gates pass: formatting, all-target Clippy
and the full test suite on Rust 1.97.1, then all-target check and Clippy on Rust
1.90.0 with the macOS 26.5 SDK. Totals: **1,314 passed, 0 failed, 34 ignored**.
Log SHA-256 prefixes: Clippy `5530ef72…`, test `24d6d52d…`,
MSRV check `fd3d5c5f…`, MSRV Clippy `5c40836f…`. Lexical-only
`cargo check --locked --no-default-features --lib` also passes.
Migration regressions use explicitly aged publications, not an old binary:
they retain stale semantic state, reject its replay, preserve unchanged cache
rows and submit exactly one changed input. An unchanged-source offset fixture
refits its unit range without an embedding request.

The rebuilt production CLI separately opened an owned store created by the old
binary: `repair_required` before repair, `ready` afterward, parse failures **1 → 0**.
Source revision **2**, exact source citation, memory revision **1**, provenance,
and exported task revision **2** remained unchanged. A real stdio MCP owner then
served hostile outline retrieve, status, context, memory get and final status,
followed by clean EOF; source count stayed **2**, parse failures **0**, and
`[links:fresh]` reached context. The first throwaway MCP check incorrectly compared
JSON key order; structural comparison confirmed identical records and the corrected
same-owner sequence passed. No production change was made for that harness error.

The owner explicitly waived G2 promotion measurement for this security-only fix.
No G1/G2, retrieval-quality, real-model, cache-economics or performance improvement
is claimed. Historical measurement receipts and private corpora are untouched.

## Dependency and branch cleanup — 2026-10-26

The owner authorized refreshing the active probe dependencies while preserving the
pre-refresh reproduction manifest, and merging the reviewed experimental line into
`main` through a pull request with a merge commit. The candidate starts at `90e6c30`.
No direct push to `main`, alert dismissal as "unused", model download or old-result
relabeling is authorized by this receipt.

| Dependency | Before | Candidate |
| --- | --- | --- |
| filelock | 4.0.6 | 4.2.1 |
| setuptools | 81.0.0 | 84.1.0 |
| certifi | 2026.7.22 | 2026.10.19 |
| rmcp | 3.5.1 | 3.6.0 |
| rmcp-macros | 3.5.1 | 3.7.0 |
| tokio | 1.53.3 | 1.54.1 |
| ureq | 3.4.2 | 3.5.0 |
| ureq-proto | 0.6.4 | 0.6.5 |

The three Python updates are PRs #38–#40. The five Cargo package updates are the
accepted maintenance subset of PR #37; the only added dependency edge is rmcp to
the already locked libc. Source and store schemas are unchanged.

**Initial parser holds.** The cleanup initially held both Scala 0.26.3 and
pulldown-cmark 0.13.5: the upstream default-parser CLI changes the setext heading
range of `[a]: /url\n    \nTitle\n===\n` from `14..25` to `15..25`. Foundry uses
those offsets for Markdown section identity. The security follow-up above resolves
the pulldown hold with the explicit source-unit/search version gate; Scala remains held.

**Python security proof.** All five currently open alerts are in the probe manifest:
setuptools advisories GHSA-h35f-9h28-mq5c and GHSA-xmj3-9gf7-h52w, and filelock
advisories GHSA-v4x8-3wr5-3wq8, GHSA-567q-5qvw-pf92 and GHSA-9vhv-jxww-xfm5.
The candidate pins cover their published fixed ranges. REST and GraphQL both report
five open alerts and no critical alert; the earlier six-alert push banner was not
used as the authoritative inventory.

A private Python 3.12.11 environment installed the complete 42-pin manifest from
wheels; `uv pip check` passed. Imports resolved the actual ModernBERT model/config,
AutoTokenizer, torch, safetensors.torch, mlx.core and mlx_lm consumers; a CPU tensor
calculation passed. No model weights were loaded. A real setuptools `sdist` included
a synthetic NFD-named excluded file under 81.0.0 and excluded it under 84.1.0, while
retaining the public control file.

FileLock, SoftFileLock, SQLite ReadWriteLock, Hugging Face WeakFileLock and the torch
FileLock consumer acquired/released locks. Separate processes verified exclusive
file locks, concurrent readers, reader/writer exclusion and acquisition after release.
A symlink lock in the private fixture was refused with its canary's bytes and mode
unchanged. The removed emSign root fingerprint was absent from certifi's bundle;
an explicit verifying context with proxies disabled fetched PyPI's filelock metadata
over HTTPS. These are macOS checks, not Windows exploit or cross-account race proof.

The [probe README](../tools/feasibility/README.md#pins-and-preparation) links the
immutable pre-refresh manifest and preserves the original 2.11.0/0.24.0 measurement
disclaimer. Model parity, sandbox and native-package acceptance do not transfer.
Scoped in-session GPT-6 Astra reviews found no remaining static blocker in the
selected Cargo subset or Python pins; runtime acceptance remains a separate check.

**Initial Rust gate result.** Formatting, Clippy, MSRV check and MSRV Clippy passed.
The full test run passed 1,305 tests with 34 ignored and one offline attribution
failure. An actual CLI trace established the cause: the selected lexical document
ended at byte 4,084, but a final packing outline named the whole file and was
mistaken for evidence that the late `10577..10589` span had been discovered.
The consumer now restricts candidate observations to selection, materialization,
placement and validation stages; packing and unknown-stage metadata cannot
manufacture discovery or freshness loss. The original expected attribution stays
unchanged, with an additional regression over misleading metadata. This corrective
follow-up belongs to the preceding selection-diagnostics feature, not retrieval
ranking or dependency behavior.

**Accepted local receipt, `16b1e39`.** All five `scripts/gates.sh` gates pass:
formatting, all-target Clippy and the full test suite on Rust 1.97.1, then all-target
check and Clippy on Rust 1.90.0 against Xcode's macOS 26.5 SDK. Final totals:
**1,307 passed, 0 failed, 34 ignored**. Log SHA-256 prefixes: Clippy `619e18e9…`,
test `32315f0e…`, MSRV check `f41a6db4…`, MSRV Clippy `00628e2b…`.
The focused attribution target also passes 3 tests with its 2 explicit diagnostics
ignored; Sol's cross-lab review accepts the GLM corrective follow-up under the
original task-specific author approval.

The rebuilt production CLI and a real stdio MCP owner completed initialize, memory
get, context and clean EOF against an owned synthetic workspace. Source bytes,
memory revision/provenance and `[links:none]` were delivered correctly; CLI
trace-on/off bytes matched. No model profile or weights were used.
Only this receipt changed after the accepted source tree was checked. No G1/G2,
retrieval-quality, model-parity, Windows-exploit or performance claim is made.

## Memory freshness and selection diagnostics — 2026-10-25

Local work on `experimental/context-ecosystem`, based on `add7627`, which merges
latest `main` (`1d9a58a`) and has exactly its source tree. The incoming
evidence/profile and MCP/worker-startup repairs are preserved.

**Memory.** Compact search/context lines carry `[links:...]`: `fresh`, `stale`,
`missing`, `none`, or mixed states joined in fresh+stale+missing order. The marker
describes linked indexed sources, not whether the decision is true or trusted.
Full reads and hits share one evaluator. Context, including task-evidence
composition, projects statuses in its actual final read; record text, author,
provenance and revision remain unchanged. Stale or missing links do not prune memory.

**Selection diagnostics.** Existing opt-in private traces now include settled
`stage:"packing"` decisions. They identify selected/shortened/dropped source forms,
actual excerpt ranges and their parents, memory IDs/revisions, unsupported brief
citations, and individually keyed proofs with required source handles. Budget events
distinguish token and serialized-byte refusals. Existing selection/validation events
retain stale-source, per-file and global candidate-cap causes. Proof retries and
independently eligible source evidence remain unchanged; a refused proof does not
make its source bodies ineligible.

**Runtime proof.** A temporary synthetic workspace exercised the actual CLI and a
real stdio MCP owner. Full reads and compact lines followed fresh → stale → missing;
the retained decision stayed at revision 1 with identical text and provenance.
Final saved CLI traces reported token-driven signature shortening, eight global
limit exclusions in a forty-file fixture, and the selected memory's ID/revision.
Trace-on and trace-off responses matched byte for byte, and saved response hashes
matched the delivered text. The trace sink's existing bounds and privacy stay in force.

**Checks.**

- `cargo test --locked --lib --test memory --test response --test brief
  --test task_evidence --test task_retrieval --no-fail-fast`: 383 passed, 0 failed,
  3 ignored. `cargo test --locked --test core retrieval_trace`: 1 passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo check --locked --no-default-features --lib`: passed.
- Rust 1.90.0 `cargo check --locked --all-targets`, with SDKROOT set to the selected
  Xcode's macOS 26.5 SDK: passed. The ordinary production CLI was rebuilt afterward.
- GPT-6.1 Sol cross-lab review of the user-authorized GLM-5.3 changes: SHIP after
  corrections. Initial review caught the composing-consumer snapshot gap,
  premature/unkeyed proof verdicts, parent-range excerpt events and silent brief
  exclusions. Regressions cover each. Initial fixture/type/lint failures were
  corrected without changing ranking, caps or proof retries.

This is not a retrieval-quality, performance or provider-cache claim. No G1/G2,
full gates, real-model experiment or CI run was performed for this work. Invalidation
explanations, cross-host resume distinctions, stable-prefix experiments and the larger
pivots remain proposed. Store schemas, source identities, citations, reversible task
views, model choices and instruction-trust boundaries are unchanged.

## CI boundary repairs — 2026-10-25

The profile-forwarding fixture compares absolute paths under the canonical working
directory; macOS may report `/private/var` for a temporary directory created under
`/var`. The profiles remain nonexistent, so configuration generation still proves
it does not load them.

The MCP escaped-byte fixture's operation deadline was reproduced with two competing
processes on one CPU. A local Rust 1.90 diagnostic with the production tokenizer
measured packing at 2.93 seconds before and 0.43 seconds after separating byte-only
prefix fitting from token fitting. These are single fixture observations, not task
economics. Final text still receives exact token and serialized-byte checks; the
five-second deadline and 256 KiB cap are unchanged. A mixed Unicode/escaping test
checks both token-limited and byte-limited continuation.

A concurrent fork can retain a snapshot's writable descriptor until exec and cause
`ETXTBSY` after the owner closes its writer. This failure was reproduced separately;
worker startup now retries only that errno within the existing load/cancellation
bound. Regressions cover eventual startup, deadline exhaustion and unrelated errors.
Resource-limit reply classification is unchanged. The original CI log recorded only
`knowledge_worker_failed`, so it does not prove this race caused that particular
failure. The memory-limit test passed in isolation, 18 stress repetitions and the
ordinary full knowledge suite. A deliberately oversubscribed 16-thread diagnostic
instead hit the separate CPU-limit test's enclosing deadline; that run is not a pass
and did not justify changing its production limits.

## Preserve retrieved evidence and selected profiles — 2026-10-25

The fixes are isolated on a branch from `main` at `2f6b4ea`. Its source tree is
identical to the earlier experimental base `4954fab`, so no experimental commits
or additional feature prerequisites are included.

- Brief enrichment retains retrieved spans and their order. Exact source-handle
  duplicates are suppressed; disjoint same-file spans remain. A missing primary
  citation is appended only while the existing 32-source window has room.
- Brief orientation requires its cited bytes in a delivered body or excerpt,
  including when a containing retrieved span provides support at a full window.
  Signatures, outlines, addresses, partial coverage and different source identities
  cannot provide that support. Freshness and delivery budgets remain authoritative.
- Generated OMP/Codex launch arguments forward explicit semantic and knowledge
  profiles and semantic development admission. Profile paths are absolute and
  escaped; printing does not load profiles/models or enable maintenance. Invalid
  development/removal combinations are refused, and no-profile launches are preserved.

The regressions exercise the real store, production tokenizer and delivery packer,
including oversized orientation, exact duplicates, disjoint ranges, a full candidate
window and rejected forms/identities. Generated OMP JSON and Codex TOML round-trip
spaces, quotes, backslashes and Unicode; the full generated argv is launched through
MCP to verify named fallback, knowledge capability without generation, and owner/store/
scratch cleanup. These use synthetic fixtures and an unusable embedding worker;
they do not establish real-model readiness, retrieval-quality or economic gains.

The PR and its local validation receipt record the tested commit, focused checks,
full gate results and ignored-test counts. No ranking, invalidation, model or
dependency policy is changed by these repairs.
Focused checks on this `main`-based worktree pass: 211 tests, 0 failures, 3 ignored,
plus all 5 connection tests. The lexical-only library check and documentation link
check also pass. The install target has no runnable tests on Linux.

## Architecture refactor and dependency updates — 2026-10-25

`4475f9f..38dc370` on `experimental/context-ecosystem`. By owner instruction the
refactor ran without measurements or long tests, so nothing here measures retrieval,
latency or memory.

**Refactor, 2026-10-23 and 24.** 48 commits in six review rounds give duplicated
logic one owner, remove forwarding layers, borrow instead of cloning and stream
instead of buffering. Examples: one placement stage for lexical and dense search, one
source-admission rule, one f32 vector codec, one JSON byte counter, one lowercase-hex
predicate, and borrowed views for knowledge, graph and SCIP output. Each commit passed
Clippy with `-D warnings` in the default, `test-faults` and no-default-features
library configurations, Rust 1.90 check and Clippy, and its focused test binaries,
and had a GPT-6.1 Sol cross-lab review that ended in SHIP (author: Anthropic Opus
5.5). Behavior is unchanged except:

- `a35926f`: decoded locations share their path and hash strings, so the 1 GiB
  expansion cap of [§ Interned label map](#interned-label-map-generation-v6--2026-10-22)
  is gone;
- `1c1895f`: a poisoned knowledge-maintenance lock reports "engine state was poisoned
  by an earlier panic";
- `da6c5d2`, in `test-faults` builds only: each preparation-selection and status page
  reads one snapshot, so a `PARTITION_READ` fault callback that writes the store is
  not seen by the rest of its page.

`f270c9e` links the Rust 1.90 gates against the selected Xcode's macOS SDK. Rust 1.90
passes `cc` no SDK root, and Xcode 26.6's linker rejects the `arm64e.x1` entries of
the Command Line Tools' macOS 27 SDK.

**Dependencies, `38dc370`.** Six of Dependabot #35's seven updates: libc 0.2.191,
redb 4.4.0, serde_json 1.0.154, tokio 1.53.3, usearch 2.26.5 and uuid 1.28.0
([dependencies](dependencies.md)). Their upstream diffs were read against our use,
and a GPT-6.1 Sol review returned SHIP:

- redb keeps file format 3; its new niche encodings need flags that are not enabled;
- serde_json's serializer is unchanged, so output bytes are identical;
- usearch keeps its file format and filtered search, so published generations load and
  search as before. It now draws HNSW node levels from fixed-seed splitmix64 streams
  that survive `reserve` and copies, so the next republish builds a different graph
  than 2.26.4 would from the same cache. The last such rebuild located 166 against 169
  on G2 (above); this one was not measured.

tree-sitter-scala 0.26.3 is held: it changes Scala parse trees, and
[§ Index version gate](../specs/001-source-state-recovery/contracts/context-v2.md#index-version-gate)
ties unit changes to schema bumps, an owner decision.

**Gates.** `scripts/gates.sh` at `38dc370` on a clean tree, Rust 1.97.1 and 1.90.0
(SDKROOT: Xcode's macOS 26.5 SDK). All five exit 0: 1,287 tests passed, 0 failed, 34
ignored, and the gateway target passes 54. Log SHA-256: clippy `2c0892a7…`, test
`a975a789…`, msrv-check `98bf8871…`, msrv-clippy `d7ff6ad4…`. Not run: the worker
features (`embed-worker`, `generate-worker`, `learning-worker`), whose binaries import
no item the refactor changed, and loading a 2.26.4-built segment under 2.26.5. No CI
ran.

## Interned label map (generation v6) — 2026-10-22

Step 2 of the [scale plan](../specs/009-optional-semantic-retrieval/spec.md) is in.
The base label map stores each distinct `[path, source sha256]` once and each
location as `[file, kind, start, end]`. Publication counts the exact bytes before
writing. Because one stored string can stand for many locations, publisher and
reader also cap the path and hash bytes locations expand to at 1 GiB. Rust's expand
to 73.7 MiB, so the cap binds near 860k files (cross-lab review finding). Since
2026-10-23 decoded locations share each stored path and hash (`Arc<str>`), so they
can no longer outgrow the file and this cap is gone.

| Republished from the same cache | v5 label map | v6 label map | ANN / members |
| --- | --- | --- | --- |
| rust-lang/rust hierarchy (624,325 keys) | 191,404,275 bytes | **69,390,990** | identical |
| oh-my-pi hierarchy (183,311 keys) | 53,313,825 | **18,805,169** | identical |

The 256 MiB label-map bound now binds near 240k files at rust's density, after the
mapping-view estimate (~120k) and before the ANN file (~253k). Publisher peaks moved
within run-to-run noise (rust 5.24 → 5.00 GB, oh-my-pi 2.29 → 2.32 GB). Serving
shows no clear change either: 3.25 → 3.17 GB peak, 3.48 → 3.54 GB maximum RSS.

**Serving is unchanged.** With the same ANN, G2 under v6 matches v5 byte for byte,
388/388 rows and responses. **Cutover:** a v5 store reports `generation format v5
is not v6` and serves lexical results. `semantic prepare` republishes it in 90 s
(oh-my-pi) and 325 s (rust) with zero embeddings. The replaced v5 files stay until
the next publication reclaims them (1.24 GB on rust), as after any full rebuild.

**The republish rebuilds the ANN, and the rebuilt graph scores differently.** The
head store's original ANN (`1d12fcb8…`, built late in the 5.9-hour preparation
process) differs from every fresh rebuild (`6ffafcd3…`); two fresh rebuilds agree
byte for byte. On G2 (both corpora) the rebuilt graphs locate 166 against 169 (+1/−4,
p = 1; rust 52 against 54; bodies 152 → 150). That is within noise and does not touch F4's margin, but the 169 is a
property of that one build, not of the recipe alone. [INFERENCE] HNSW level
assignment depends on the building process's earlier index builds; the cause was
not investigated.

**Checks.** Unit tests fix the round trip, the refusal of damaged tables and the
string budget at its boundary. Local gates on Rust 1.97.1 and 1.90.0 pass all five:
1,284 tests passed, 0 failed, 34 ignored. A GPT-6.1 Sol review found that an
interned path could expand without bound; with the budget, its second review
returned SHIP. The budgeted build again served 388/388 identical G2 responses. No CI
ran.

## Streamed index build — 2026-10-22

Step 1 of the [scale plan](../specs/009-optional-semantic-retrieval/spec.md) is in. In
every rebuild, publication decodes cached vectors in pages of 1,024, adds each page to
the f16 index under construction and drops it, rather than holding all of a
generation's f32 vectors. That removes the 2 GiB vector bound, which rust-lang/rust's
hierarchy used to 89%.

Each generation's published files were deleted from a store clone and republished
from the unchanged vector cache with zero document calls, by the previous build
(`c27c511`) and this one:

| Republish | Entries | ANN / labels / members | Publisher peak footprint | Wall |
| --- | --- | --- | --- | --- |
| rust-lang/rust hierarchy, before | 624,325 | `6ffafcd3` / `52966a91` / `3a1372c4` | 7.35 GB | 341 s |
| rust-lang/rust hierarchy, after | 624,325 | identical | **4.87 GB** | 332 s |
| oh-my-pi hierarchy, before | 183,311 | `7f31dce4` / `f810a623` / `c8703eb9` | 2.79 GB | 94 s |
| oh-my-pi hierarchy, after | 183,311 | identical | **2.23 GB** | 94 s |

`repair-index`, the other publication path, republished oh-my-pi with the same three
hashes. Generations are byte-identical, so retrieval is unchanged and no G2 run is
needed. A new unit test fixes the page contract: every cached key once, in key
order, across page boundaries. The label map (256 MiB, 71% used) is the next bound,
near 87k files.

**Checks.** A GPT-6.1 Sol review found that location-only publications constructed
a native index they never used. The index is now created on the first vector, and
the second review returned SHIP. After that fix oh-my-pi republished with the same
hashes again, peaking at 2.28 GB. Local gates on Rust 1.97.1 and 1.90.0 pass all
five: 1,283 tests passed, 0 failed, 34 ignored. No CI ran.

## F4 and the hierarchy default — 2026-10-22

**F4 passes; complete hierarchy is the default.** The sealed 48-question set was
unreachable (see [FOLLOWUP F4](../FOLLOWUP.md#f4-sealed-independent-evaluation--passed-2026-10-22)).
The owner chose a fresh sealed set, pre-registered in `d29314d` before any question
existed.

- **Authoring.** Six agents (three Opus 5.5, three GPT-6 Astra) wrote 300 questions,
  150 over rust-lang/rust `library/` and 150 over oh-my-pi. Each author had a disjoint
  third of the modules, walked candidates in SHA-256 order and skipped 57 of 357 (54
  over the per-file or per-module cap, 3 not unique).
- **Checks.** None of the answers is a G2 answer. G2's unmodified `verify.py` passes
  the merged set, self-test included (46/46). Its self-test cannot run on a single
  50-row part, so authors checked their parts with it stubbed.
- **Seal.** A GPT-6 Astra sealer froze `questions.jsonl` (`0083fd2a…`); its
  report-only audit found 29 of 30 sampled rows correct and unique and 1 rust row
  wrong, which stays in. The captain read no question, answer or author transcript.

Both arms ran `f06ec74` with EmbeddingGemma 2 at 2,048 tokens, a minute apart, and
`scripts/g2.sh compare` applied the pre-registered rule (K = 1):

| F4, 300 questions | Cards | Complete hierarchy |
| --- | --- | --- |
| Located | 100 | **130** (+64/−34, p = 0.0016) |
| Verbatim bodies | 97 | **128** (+64/−33) |
| rust: located / bodies | 44 / 43 | 51 / 50 |
| oh-my-pi: located / bodies | 56 / 54 | 79 / 78 |
| Mean delivered tokens | 2022.7 | 2021.5 |

No row errored, retried or exceeded the cap. The archive's resource and latency
guards were lost with it; the serving peak is reported, not gated. Remeasured in one
session as the MCP owner's peak footprint over rust's 197 G2 questions (`/usr/bin/time
-l`; the embedding worker is a separate process), it is 1.97 GB for cards, 3.34 GB for
hierarchy and 4.91 GB for the unmerged one score with fusion (maximum RSS 5.18 GB).
The earlier 1.95 and 3.24 GB used the same method. One score with fusion sits at the
owner's ~5 GB line for a design document, but it is not significant against the new
default (p = 0.057), so it stays a lab result and is not built.

**Promotion.** A profile without `representation` now selects `hierarchy-v1` with a
64-token parent and 512-token inputs ([spec 009](../specs/009-optional-semantic-retrieval/spec.md)).
With such a profile the promoted build reproduces explicit hierarchy row for row on
F4 (300/300) and on G2 (388/388, same session). Smoke checks of the cutover:

- a cards store under the default profile reports `profile_mismatch` and serves
  lexical results;
- the frozen 2026-10-08 cards profile, which states 128 tokens but no representation,
  is refused at load with the `cards-v1` instruction;
- an explicit `cards-v1` profile still serves `semantic:ready`.

**Instrument note.** After a host reboot, the unchanged `f06ec74` hierarchy arm
differs from its morning G2 run in 36 of 388 rows: 35 token counts and 2 pass bits,
one each way, so totals stay 169. Stores, worker and model are unchanged. Runs within
one session are identical, and every comparison above is same-session. The cause was
not investigated. Receipt: [f4-2026-10-22.json](review/f4-2026-10-22.json).

**Checks.** Local gates on Rust 1.97.1 and 1.90.0 pass all five: 1,282 tests passed,
0 failed, 34 ignored. A new test fixes the default, the explicit-cards recipe and the
legacy-profile boundary. A GPT-6.1 Sol review returned SHIP; its one minor finding
is now documented in spec 009 and covered by that test: implicit-cards profiles of 256 tokens or more
now load as hierarchy. No CI ran.

## G1 on hierarchy, and fusion — 2026-10-22

**G1 passes for complete hierarchy.** Frozen G1v4 runs every task (SAMPLE 1/1), with
HEAD `f06ec74` serving no profile, cards and hierarchy. All three sets PASS in every
arm. No target or guard that passes without a profile fails with one. Hierarchy
does cost unmarked rust tasks: checker pass≤4 goes from 3,878 to 3,843 (+13/−48);
cards goes to 3,845. Apparatus: the rust stores need the frozen `index.scip`
imported. Without it every `references` call is `coverage:unavailable` and U1/U2
fail (74.6%, 38.2%, in a run whose files were not kept). See
[measurement handoff](measurement-handoff.md).

**The 17 G1 tasks hierarchy newly loses against cards** (16 checker, 1 bun; it
newly gains 14). All 17 are unmarked one-word queries for names with many
definitions (`finish`, `len`, `flush`, `map`). Each hierarchy context was replayed
with a trace; the replay matched the G1 response byte for byte in 17 of 17. Causes:

- **10: tier-1 lexical hit cut by dense-first placement.** Lexical tier 1 found the
  definition or use site at rank 0–47. Placement puts every dense unit first, so it
  landed at rank 62–211 and was cut by the 32-unit limit or the per-file cap.
- **2: tier-2 or low-ranked hit cut by the limit** (`finish`, `flush`).
- **4: packed but omitted by the budget** (rank 18–33 of the packing input).
- **1: never retrieved** (`map`: the use site's file is absent).

The cards arm was not traced. 15 of the 17 pass under one score with fusion, whose
single ranking removes dense-first placement (`port` and `map` still fail); that arm
is not a candidate.

**Fusion, then one score** (G2, 2,048 tokens, located / verbatim body):

| Arm (K = 3 against hierarchy) | Located | Bodies | Against hierarchy |
| --- | --- | --- | --- |
| Hierarchy | 169 | 151 | — |
| Fusion: cards and hierarchy dense lists, RRF | 169 | 145 | +31/−31 |
| One score: hierarchy, lexical tier 1, tier 2, RRF | 168 | 154 | +33/−34 |
| One score with fusion (four lists) | **185** | **165** | +53/−37, p = 0.057 |
| …and no per-file cap, 64-unit limit | 185 | 166 | +1/−1 against the row above |

One score with fusion is not significant against hierarchy (p < 0.0167 needed). It
would pass the rule against the current default, cards (+66/−17, p = 2.8 × 10⁻⁸).
It passes G1 too, recovering most of hierarchy's unmarked losses: checker 3,871
(+15/−22 against no profile). It needs both generations prepared. The upper bound
(either recipe answers) is 214.

**Caps are not the lever now.** Under cards, 75 of the 95 cut answers came only
from lexical tier 2, 18 of them as its top hit (traces since deleted). Placement put
every dense unit first, and tier 1's 64 definitions came before tier 2. With one
score the caps still exclude 79 answers, but removing them changed one question each
way. Only 2 of those 79 rank in the top 13, about as many items as fit in 2,048
tokens; that the rest would not fit is an inference, not an ablation. What remains is
ranking (79 cut; 44 packed but omitted, 10th–90th percentile at zero-based ranks
8–26) and recall (27 files and 53 units never retrieved). The environment-gated
experiment is not merged: [patch](review/one-score-fusion-2026-10-22.patch), measured
with experimental behavior off. With it on, prose interleaving silently takes
precedence over one score (review M1). Receipt:
[g1-fusion-2026-10-22.json](review/g1-fusion-2026-10-22.json). The scale plan is in
[spec 009](../specs/009-optional-semantic-retrieval/spec.md). A GPT-6.1 Sol review
verified the scores, comparisons and scale arithmetic.

## Complete hierarchy on rust — 2026-10-21

Complete `hierarchy-v1` now has G2 results on both corpora. Under the 2026-10-19 rule
it beats the address-card default (`scripts/g2.sh compare`, K = 2):

| 2,048 tokens | Cards (default) | Complete hierarchy |
| --- | --- | --- |
| Located, all 388 | 136 | **169** (+78/−45, p = 0.0019) |
| Verbatim bodies, all 388 | 120 | **151** (+78/−47) |
| rust: located / bodies | 43 / 42 | 54 / 47 |
| oh-my-pi: located / bodies | 93 / 78 | 115 / 104 |
| Mean delivered tokens | 2021.9 | 2023.3 |
| Serving owner peak footprint, rust | 1.95 GB | 3.24 GB |
| Indexed units, rust / oh-my-pi | 332,732 / 112,549 | 685,623 / 199,592 |

Rust alone is +35/−24 (p = 0.096): same direction as oh-my-pi, not significant on
its own. Experimental behavior again lowers hierarchy, to 155 (+3/−17 against
hierarchy). The gain churns: 45 questions that cards answer, hierarchy loses.

**Cost.** Embedding and publication took 5.9 hours of Metal time for 685,623 units
(624,325 distinct vectors, 101.6 million tokens), against the 8–9 hours estimated.
Rust's cards take 72 minutes. A `library/`-only index was considered and rejected:
all 197 rust answers are in `library/` (4% of files), but non-library code supplies
57% of today's dense candidates and delivered items, appearing in 181 of 197
answers, so dropping it would flatter both recipes.

**Two more worst-case estimates blocked publication.** After 5.5 hours and 597,282
vectors, publication refused: its pre-check counted the label map at 277 MiB against
the reader's 256 MiB limit, while the real file is 182 MiB (the fixed 160 bytes per
location assumed 20-digit offsets). The mapping-view estimate would also have reached
1.03 GiB against its 1 GiB bound. Publication now counts the exact compact JSON it
writes (a regression test bounds the count between the written size and the label
ordinals at full width), and the view estimate bound is 2 GiB. The resumed pass
reused all cached vectors and embedded the remaining 27,043 in 23 minutes; 17
publication retries had cost 20 minutes. The 2 GiB vector bound is next: this
repository uses 89% of it.

**Decision for the owner.** Hierarchy passes the G2 rule and is the better measured
recipe, at 1.8–2.1 times the units, 5× the rust preparation time and 1.3 GB more
serving memory. Promoting it to the default still needs the G1 gates and held-out
questions (F4), which spec 009's promotion rule requires. Receipt:
[g2-hierarchy-rust-2026-10-21.json](review/g2-hierarchy-rust-2026-10-21.json). Local
gates on Rust 1.97.1 and 1.90.0 pass all five: 1,281 tests passed, 0 failed, 34
ignored. A GPT-6.1 Sol cross-lab review returned SHIP; its two minor findings (vector
counting in the docs, an inexact test tolerance) were fixed. No CI ran for this change.

## G2 rule without the token clause — 2026-10-19

The owner dropped the frozen rule's "no more mean delivered tokens" clause: it
refused gains that only filled the same 2,048-token cap more fully. Before applying
that, every held candidate was rechecked with a second score from the same frozen G1
parser: whether the definition arrives as a **verbatim body** (no form tag, nothing
elided), not only as a located name line. The frozen G2 score counts a name line in
any delivered body, so a signature counts as much as the implementation.

| Candidate (base) | Located | Verbatim body | Mean tokens |
| --- | --- | --- | --- |
| Breadth-first packing (current cards) | 136 → 150 (+14/−0) | 120 → **25** (+0/−95) | 2021.9 → 2033.9 |
| Experimental lexical (default lexical) | 0 → 13 (+13/−0) | 0 → 10 (+10/−0) | 2023.2 → 2023.7 |
| Experimental behavior with cards | 136 → 136 | 120 → 120 | 2021.9 → 2022.2 |
| Complete hierarchy (cards, bun only) | 93 → 115 (+43/−21) | 78 → 104 (+47/−21) | 2021.2 → 2025.7 |
| Experimental behavior with hierarchy (bun only) | 115 → 108 (+2/−9) | 104 → 100 (+2/−6) | 2025.7 → 2023.6 |
| File overviews (cards, bun only) | 93 → 77 (+29/−45) | 78 → 45 (+21/−54) | 2021.2 → 1984.6 |
| Current cards (2026-10-08 cards) | 139 → 136 (+5/−8) | 123 → 120 (+4/−7) | 2018.1 → 2021.9 |

Breadth-first packing gained located names by turning most bodies into signatures:
it would have cut verbatim definitions from 120 to 25. The token clause had blocked
it, but for the wrong reason. [`scripts/g2.sh`](../scripts/g2.sh) `compare` therefore
replaces the clause with a quality guard instead of dropping it alone. A candidate is
enabled when it locates significantly more definitions (exact one-sided McNemar,
p < 0.05/K, K = 2 as frozen), loses no verbatim bodies net, and keeps every response
within the request cap. Mean tokens are reported, not gated. The frozen harness,
questions and parser are unchanged. `compare` fails closed (INVALID, exit 2) unless
every question has exactly one valid result and response: duplicate, missing,
unknown, malformed, retried, refused or over-cap records, and recorded pass bits that
disagree with their own response, all invalidate the run. All 2026-10-17 runs pass.

Under this rule:

- **Breadth-first packing is rejected** (−95 verbatim bodies). The patch stays only as
  the [measured artifact](review/breadth-first-packing-2026-10-17.patch).
- **Experimental lexical now passes G2** for lexical-only use (p = 1.2 × 10⁻⁴, no body
  loss). It changes nothing with cards and costs hierarchy 7 located definitions.
  It stays off: 001 T010 also requires the anchored G1 gates and an agent-task
  result (003 T006), which G2's anchorless questions cannot supply.
- **Complete hierarchy passes on bun** (p = 0.0041, +26 bodies net). Rust was not
  prepared; a recipe decision needs it.
- Current cards remain the default; against 2026-10-08 they are unchanged within noise.

Comparisons ran with `scripts/g2.sh compare` on the 2026-10-17 runs; bun-only rows
used the same metrics over 191 questions. No Rust code changed.

## G2 restored as the retrieval instrument — 2026-10-17

From 2026-10-10 to 2026-10-17, retrieval work was judged on eight Foundry questions.
From 4/8, three wins and no losses give a one-sided McNemar p of 0.125, so that set
cannot show a gain, and its wording favors lexical matching. The frozen G2 set (388
identifier-free questions over rust-lang/rust `library/` and oh-my-pi, with harness
and stores) was on the owner's Mac throughout; only the cloud session lacked it
(FOLLOWUP F5). [`scripts/g2.sh`](../scripts/g2.sh) now runs it.

**The instrument reproduces.** The archived `8035faa` binary and 2026-10-08 Gemma
stores give 139/388 again, with all 388 pass bits and token counts identical.
Each arm takes about a minute once its stores are prepared.

**Current code (`e9c5ca0` plus the publication fix below)**, Apple M3 Pro, real
EmbeddingGemma 2 BF16 on the Metal worker (adapter revision 4), 2,048 tokens:

| Arm | rust (197) | bun (191) | Total | Mean tokens |
| --- | --- | --- | --- | --- |
| Lexical, default | 0 | 0 | 0/388 | 2023.2 |
| Lexical, `--experimental-behavior-context` | 3 | 10 | 13/388 | 2023.7 |
| Gemma address cards, 2026-10-08 (`8035faa`, revision 1) | 45 | 94 | 139/388 | 2018 |
| Gemma address cards, current default | 43 | 93 | 136/388 | 2021.9 |
| Gemma address cards, current experimental | 43 | 93 | 136/388 | 2022.2 |
| Gemma `overview-v1` (bun only) | — | 77 | — | 1984.6 |
| Gemma complete `hierarchy-v1`, default (bun only) | — | 115 | — | 2025.7 |
| Gemma complete `hierarchy-v1`, experimental (bun only) | — | 108 | — | 2023.6 |

Nine days of retrieval commits did not change the default semantic result: against
2026-10-08 the current cards gain 5 and lose 8 (not significant). With address cards,
experimental behavior changes no question; without a profile it gains 13 and loses none
(p = 1.2 × 10⁻⁴) but delivers 0.5 more mean tokens, so the frozen rule ("no more
mean delivered tokens") keeps it off. File overviews prepare 2.6 million instead of
6.8 million bun tokens but deliver 77 against 93 (29 gained, 45 lost).

**Complete hierarchy is the best measured recipe, at a cost.** On bun it delivers 115
against the cards' 93: 43 gained, 21 lost, p = 0.0041, at 4.5 more mean tokens.
Experimental behavior lowers it to 108 (2 gained, 9 lost). It needs 199,592 vectors
against 112,549 cards. An earlier preparation committed 81,121 of them; 11 hours after
its last publication its worker was still waiting on a Metal command buffer, across
battery sleep periods, and the process was later lost with its report. The completing
pass embedded 102,190 more
(23.9 million tokens) in 91 minutes on AC power. Rust hierarchy was not prepared
(about three times bun's cards); this is a one-corpus result.

**A publication regression blocked mid-size repositories.** Since `5890ebd`/`65eb571`
(2026-10-09/10), publication charged every path and hash byte as a worst-case
six-byte JSON escape against 256 MiB. Bun's 112,549 cards embedded and then failed
with `publication mapping view exceeds 268435456 bytes`, leaving 67,629 entries
live. The real label map for rust's 332,732 cards is 90.5 MB. Publication now counts
exact `serde_json` escapes for the files the reader caps at 256 MiB and bounds the
in-memory mapping view separately at 1 GiB. Checkpoints of the new accounting use a
new key and the old key is removed on write, so neither binary restores a lookup
summed under the other's accounting (a cross-lab review found that an earlier binary
would otherwise fail its next edit as a corrupt store). After the fix, bun
published all 112,549 entries from cache in 74 s with zero model calls. Rust
prepared 332,732 cards (19.3 million tokens) in 72 minutes and published them.

**Where the 252 misses happen** (current cards; a traced rerun delivers identically):

| Stage | Questions |
| --- | --- |
| Target file never retrieved | 45 |
| File retrieved, target unit not | 75 |
| Unit retrieved, cut by placement limit, per-file cap or duplicates | 95 |
| Unit packed, omitted by the 2,048-token packer | 37 |

The packer shows about 14 of 35 packed items; in omitted cases the three largest take
a median 1,090 tokens. Packing the smallest rung of every candidate first, then
upgrading in rank order, delivers 150/388: 14 gained, none lost, p = 6.1 × 10⁻⁵. It
fills the same cap more fully (mean 2,034 against 2,022 tokens), so the frozen token
clause keeps it off; the [patch](review/breadth-first-packing-2026-10-17.patch) is not
in the tree.

These p values are development evidence over many paired comparisons on one set. The
frozen 2026-10-08 family used α = 0.05/2; a Bonferroni bound over up to ten
comparisons reported here (α = 0.005) still holds for both zero-loss gains.

Owner decisions needed:

- **Token clause.** The frozen rule's "no more mean delivered tokens" is the only thing
  keeping three gains off: experimental lexical (+13/−0, +0.5 tokens), breadth-first
  packing (+14/−0, +12) and complete hierarchy (+43/−21 on bun, +4.5). Each differs by
  how fully it fills the same 2,048-token cap.
- **Recipes.** Complete hierarchy wins on bun but needs about 1.8 times the cards'
  vectors and was not measured on rust. File overviews are cheapest and lose to cards.
  Recommendation: keep cards as the default, make hierarchy the recipe to develop and
  measure on rust, and decide whether overview-only stays.
- **Experimental behavior.** It helps only lexical-only installs, is neutral with
  cards and hurts hierarchy (−7 net). Recommendation: do not promote it; remove its
  semantic-path effects or keep it lexical-only.

Corrections to the earlier review: the neural model is the product's retrieval lever
(0 → 136/388), not a no-gain addition; and "Actions unavailable" reflects the
owner's budget decision (F1), not a contradiction with the push-triggered workflow.

Final local gates on Rust 1.97.1 and 1.90.0 pass all five: **1,280 tests passed, 0
failed, 34 ignored**. An earlier full run on the first publication fix had one
failure, `six_thousand_files_allow_foreground_calls_and_no_idle_body_rescans`
(6,076–7,658 source reads against 6,000); unchanged `e9c5ca0` also fails it in 2 of 3
runs on this host, so it is intermittent and predates this change
([FOLLOWUP F18](../FOLLOWUP.md#f18-the-6000-file-watcher-test-rereads-sources-on-the-owners-mac-open-2026-10-17)).
A GPT-6.1 Sol cross-lab review returned REVISE (checkpoint downgrade, wrapper quoting,
run-name traversal, the frozen K, two minor gaps), then SHIP after the fixes. G2 is now
development evidence; acceptance still needs held-out questions (F4). No CI ran for
this change. Receipt: [g2-head-2026-10-17.json](review/g2-head-2026-10-17.json).

## Representation repair and lexical ablation — 2026-10-17

Runtime commit `92d8e6a` repairs two semantic representation defects; this entry also
records lexical ranking experiments that were rejected. It establishes **no retrieval
gain**: every arm below delivers the same **4/8** public expected spans (Q0, Q2, Q3,
Q7) before (`11d61e0`) and after the change, at 2,048 tokens on the `ebf28f8` snapshot.

Body-v1/hierarchy body windows carry a context prefix: the file preamble or enclosing
header, then the region's own header. When that exceeded `parent_tokens`, it was halved
from its end, so a long `//!` preamble displaced the function's own signature in the
context of its later windows. The region's own declaration now wins, then its signature
alone when its documentation is too long; in-budget contexts are byte-identical to
before. Long overviews kept only a file's first line and each declaration's signature,
not the opening documentation their own comment promised. They now keep the file
opening and each declaration's leading documentation (whole lines within 256 bytes)
beside its signature, without repeating source bytes. Recipes become `+context2` and
`+overview4`; old partitions refit and unchanged exact inputs reuse vectors. Three new
unit tests fail on `11d61e0` and pass now.

Real EmbeddingGemma 2 BF16 ran on an Apple M3 Pro through the Metal worker (adapter
revision 4, ad-hoc signed development bundle, 512-token cards):

| Arm | Baseline `11d61e0` | Repair |
| --- | --- | --- |
| overview-v1: delivered | 4/8, 16,203 tokens | 4/8, 16,228 tokens |
| overview-v1: preparation | 274 inputs, 84,927 tokens, 39.6 s | 274 inputs, 88,959 tokens, 37.3 s |
| hierarchy-v1 (`parent_tokens` 64): delivered | 4/8, 16,197 tokens | 4/8, 16,250 tokens |
| hierarchy-v1: preparation | 13,822 inputs, 3,608,991 tokens, 981 s | 13,840 inputs, 3,504,074 tokens, 941 s |

Target-file dense ranks move both ways. Overview: Q0 5→3, Q3 26→16, Q6 7→4, while Q2
falls from 33 out of the 64 hits. Hierarchy: Q3 8→3, while Q0 3→6, Q2 15→23 and Q6
8→11. Q4 and Q5 rank first in every arm yet remain lost (overview `evidence_window`,
hierarchy `candidate_limit`); Q1's source is excluded. The earlier recorded 2/8 for
complete hierarchy is superseded: this host measures 4/8 at `11d61e0` too. Each CLI
question loads the model (about 11–13.5 s here). Unchanged replay was not rerun.

The candidate hierarchy run reaches the 4,096-vector refinement cap for Q1 and Q2, so
owner-scope ordering in refinement is live. An earlier review called it a no-op; that
claim was wrong, and the code stays.

Lexical ranking changes were tried on the same store and reverted. Each gain lost Q0,
and the module-documentation redirect existed only to reach Q6 (default stays 1/8):

| Variant | Delivered |
| --- | --- |
| Baseline | 0, 2, 3, 7 |
| All tried heuristics (module-doc redirect, module-header boost, plural forms, sort tie-breaks, struct-to-method extension, adjacent-function companion) | 2, 3, 4, 5, 6, 7 |
| Without redirect and header boost | 2, 3, 4, 5, 7 |
| Also without plural forms | 2, 3, 4, 7 |
| Also without tie-breaks | 0, 2, 3, 7 |
| That plus plural forms | 2, 3, 5, 7 |

Local gates on Rust 1.97.1 and 1.90.0: fmt, clippy, MSRV check and MSRV clippy pass.
The full suite reports 1,277 passed, 1 failed and 34 ignored (library 264, core 87,
neural_retrieval 95, task_evidence 21). The failure is `install`'s packaging test: under
`--offline` it could not obtain `tch 0.26.0`, which this host's Cargo cache lacked.
After `cargo fetch --locked`, that target passes 6/6; the full suite was not rerun.

A GPT-6.1 Sol cross-lab review first found a partial-overlap signature loss (fixed,
with its regression) and documentation gaps; its final verdict is SHIP. The eight
questions are author-visible; no held-out or private set, task-quality comparison or
provider accounting ran. The [receipt](review/representation-repair-2026-10-17.json)
lists hashes and arms. No CI ran for this change.

## Evidence admission and owner cleanup — 2026-10-17

Runtime commit `81c20ad` fixes two losses before delivery. Hierarchy refinement
now treats lexical/parent owners as priorities: owner bodies and the remaining
published file bodies alternate into the existing bounded working set and compete
using their own vectors. Exact partition/source/generation joins and all caps stay
in force. Task evidence now admits a relationship and its covering verbatim source
bodies as one budget decision, reusing shared dependencies before filling spare
capacity with standalone sources, briefs and memory.

The regression comparison against `8580bb5` exposes the old behavior: hierarchy
refinement admits 31 of 39 published body spans, excluding eight competing spans.
The repaired test requires all 39 and independently checks their ranking and source
bytes. At a 3,072-token allowance, the old task allocator delivers one exact-key
client/service relationship; the repair delivers both client/service and test/service
relationships with complete source support. Both already delivered the test source:
the improvement is the second explicit connection, not new test discovery or observed
execution. The strengthened regression requires both relationships and rejects stale
provider support. Fixture token observations are not provider economics.

Commit `d72f3bb` also fixes a teardown race found by the full suite: `Engine` used
to release redb ownership before Tantivy's writer finished shutting down. A new
owner could enter that window and falsely report `repair_required`. The database
field now drops last, keeping ownership through derived-resource cleanup.

Real EmbeddingGemma 2 BF16 preparation completes 52 vectors across six public
fixture files in seven document calls; unchanged replay makes zero calls/tokens.
The initial short-query control is 3/3 in both arms but bypasses prose refinement.
A separate traced follow-up delivers 4/4 expected spans in both arms. Its explicit
owner-competition case admits one owner body plus 25 disjoint same-file bodies to
refinement, versus only the owner body before the fix. The ordinary ANN window
already contains those siblings in this small corpus, so this is changed-path
integration evidence, not a repository-score gain. Exact bytes, 2,048-token limits,
current revision, frozen identities, worker cleanup and zero-call replay pass.

Local verification on `d72f3bb` passes all five gates on Rust 1.97.0/1.90.0:
**1,174 tests passed, zero failed, 28 ignored**, including all 54 gateway tests
under uid 1000. The no-default-features library check has zero warnings, and seven
real framework/runtime checks pass. The first full run on `81c20ad` had 1,172 passed
and two failures: the ownership race above and an MCP byte-cap deadline. The latter
passes separately with its original deadline. The final full run uses two concurrent
test cases on the four-CPU quota; no additional test is skipped and no deadline,
cap or assertion is loosened.
The initial failed gate logs and model preflight failure remain recorded.

The [receipt](review/evidence-selection-2026-10-17.json) names code commits, log
SHA-256s, commands, model identities and retained failures. Raw artifacts remain at
`/workspace/.context-foundry-tools/validation-runs/evidence-selection-2026-10-16/`.
The public eight-question benchmark was not rerun: its last experimental lexical
and file-overview result remains 4/8; historical complete-hierarchy delivery remains
2/8. Coarse-file excerpt loss, broader ranking/packing, representative inference,
provider-accounted task benefits and physical Mac/GPU acceptance remain gaps.
Unrelated FOLLOWUP protocols stay parked. No CI ran: GitHub Actions is unavailable
(owner decision 2026-10-12).

## Callable passage preservation — 2026-10-16

The experimental lexical path now delivers **4/8** public expected source spans at
2,048 tokens, up from **3/8** at `d8248d5`. It retains Q0, Q2 and Q7 and adds Q3.
Default context remains 1/8. Across all eight responses, delivered tokens fall from
16,177 to 16,151; no allowance or candidate window was increased. The questions
are the unchanged, author-visible development set on public source `ebf28f8`, not
independent retrieval acceptance or a coding-task success rate. Q1 stays in the
denominator despite its source being excluded by the existing admission policy.

The lost Q3 passage belonged to the same callable as the rank-zero document but
ranked 64 on its own, outside the 32-unit cut. Experimental code-lane selection now
keeps the next disjoint matching document of that exact owner nearby before the
code/document lanes alternate. Path, source hash, owner start/head/end and kind must
all agree. Final freshness, two-document owner selection, diversity, candidate and
delivery limits still apply. The new regression places forty competing files
between two matching passages and checks stale-source suppression.

The first model experiment tested bounded syntax-owner localization of coarse
file vectors as well. It delivered **3/8**, unchanged from the baseline, so that
localizer and its stemming dependency were removed. Its full results, source patch,
failed development checks and model commands remain in the run directory. Passing
source-boundary tests did not establish a relevance improvement. The retained repair
instead preserves exact-owner lexical pairs after dense interleaving, where the
first experiment still separated Q3's matching documents before packing.

Implementation `f782dda` applies that grouping through the semantic merge. The
final real EmbeddingGemma 2 BF16 file-overview comparison also improves **3/8 →
4/8**, preserving Q0, Q2 and Q7 and adding Q3:

| Arm, 2,048 tokens per question | Delivered spans | Delivered tokens, eight questions | Query wall time, eight commands |
| --- | --- | ---: | ---: |
| `d8248d5` baseline | 3/8: Q0, Q2, Q7 | 16,289 | 204.983 s |
| `f782dda` candidate | 4/8: Q0, Q2, Q3, Q7 | 16,242 | 184.037 s |

Both arms use the same previously completed 274-vector overview store, model,
worker, profile, source hashes and revision 276. The final candidate reuses the
baseline observations from the preceding paired experiment after verifying their
binary/output hashes and exact source spans. These single observations, taken at
different times, establish no latency benefit. Every returned verbatim source body
matches its source hash and byte range, every response fits 2,048 tokens, and all seven prose
queries report current dense coverage with 64 hits. Q7 keeps the model off.
Unchanged replay takes 4.551 seconds with zero document calls, zero input tokens
and all 274 cached inputs/partitions reused. Doctor checks pass before every
model command with `avx512_bf16`; compilation and inference never overlap. Every
command leaves an empty scratch root and no new worker. Start/end boot IDs and
wrapper exit receipts agree.

The rejected localization experiment is retained at 3/8 in both arms. Two earlier
runner preflight errors (a build-toolchain environment and a wrong driver filename)
stopped before model invocation. Development failures are retained too; the final
focused suite passes 445 tests. One hierarchy regression now verifies both the full
lexical document and its exact fallback range because grouping exercises that
existing packing form. It still requires preferred independently scored body
ranges and current ownership. All five local gates pass on Rust 1.97.0/1.90.0:
formatting, Clippy, the full suite (1,173 passed, zero failed, 28 ignored), MSRV
compilation and MSRV Clippy. All 54 gateway tests pass directly under uid 1000.
The no-default-features library check passes with zero warnings, and documentation
links pass. Toolchain identities and each log's SHA-256 are in the committed receipt.

For the public corpus/store setup from the preceding budget receipt, reproduce
lexical/default comparisons with a new output directory:

```sh
FOUNDRY_ATTRIBUTION_ROOT=/abs/frozen-foundry \
FOUNDRY_ATTRIBUTION_STORE=/abs/frozen-store \
FOUNDRY_ATTRIBUTION_BASELINE=/abs/d8248d5-foundry \
FOUNDRY_ATTRIBUTION_DIR=/abs/new-owner-comparison \
cargo test --locked --test retrieval_attribution -- \
  --exact repository_development_attribution --ignored --nocapture
```

Run either saved CLI against the same completed overview profile for a real-model
question; the other unchanged questions are in `tests/retrieval_attribution.rs`:

```sh
foundry --store /abs/frozen-store context \
  'mark references inside attributed tests as verification evidence' \
  --tokens 2048 --strategy search --experimental-behavior-context \
  --semantic-profile /abs/overview-profile.json --development-isolation \
  --retrieval-trace-dir /abs/new-query-trace
```

Raw commands, outputs, traces, rejected patches and logs live under
`/workspace/.context-foundry-tools/validation-runs/owner-retrieval-2026-10-16/`;
`merged-model/` holds the retained model candidate. The
[committed receipt](review/owner-retrieval-2026-10-16.json) records their identities.

Q4–Q6 remain unresolved. Correct file discovery alone does not identify the needed
implementation; callable localization by stemmed term coverage did not repair it.
Historical complete-hierarchy delivery remains 2/8 and was not rerun. General
retrieval acceptance, full representative inference, broader evidence, provider-
accounted task benefits and physical Mac/GPU acceptance remain open. Unrelated
FOLLOWUP protocols stay parked. No CI ran: GitHub Actions is unavailable by owner
decision. These checks are local, and this checkpoint is not a release.

## Budget allocation and evidence preservation — 2026-10-16

The owner reopened budget and retrieval repair. Implementation `c258026` improves
experimental lexical delivery on the unchanged, public eight-question Foundry set
from **2/8 to 3/8 at 2,048 tokens**, retaining both previous hits. Default context
remains **1/8** at that budget. These are exact-source-span delivery counts on an
author-visible development set, not coding-task success rates or independent
retrieval acceptance. Historical complete-hierarchy delivery remains 2/8; it was
not rerun. No private archive or parked evaluation was used.

The failure crossed several boundaries. The implicit MCP configuration ceiling
capped explicit requests at the default 2,048 tokens. Candidate construction clipped away matching
source before packing knew the allowance. Hard per-file caps discarded additional
useful candidates even when the overall window had room. Source reads repeatedly
halved until they fit, leaving avoidable continuation overhead.

The repair keeps the default request at 2,048 while allowing explicit requests up
to the existing 32,768 protocol limit; configured ceilings and session allowances
still apply. Matching indexed documents survive to packing, with exact, bounded
excerpt fallbacks and representation-specific source handles. Experimental prose
selection preserves diverse first choices, then backfills spare slots; refinement
retains the 320-location provenance ceiling. Packing suppresses wholly covered
source ranges, counts the final omission header and checks brief support against
any delivered exact body. Source reads refine the known fit bracket with bounded
UTF-8-safe trials, accepting only responses that pass exact token and byte counts.
Partial overlaps and greedy full-form preference remain; no optimal-packing claim
is made.

The old CLI (`8f0b94c`) and candidate used the same public `ebf28f8` source snapshot,
source-only store, labels and requested allowances. Traced and untraced candidate
outputs agreed. All eight questions stay in the denominator; the source excluded
by the existing private-key marker policy stays excluded.

| Allowance per question | Old hits | New hits | Old delivered tokens, eight questions | New delivered tokens, eight questions |
| --- | ---: | ---: | ---: | ---: |
| 2,048 | 2/8 | 3/8 | 16,123 | 16,177 |
| 4,096 | 2/8 | 3/8 | 30,479 | 30,535 |
| 8,192 | 2/8 | 3/8 | 59,151 | 59,194 |
| 32,768 | 2/8 | 3/8 | 134,980 | 209,085 |

The added hit is Q0: the matching rank-zero document now retains the publication
race check instead of clipping it away. Q2 and Q7 remain delivered. Larger budgets
produce no additional hits here and can substantially increase delivered tokens. They are not
a substitute for ranking and discovery repairs. At 2,048, Q3's expected document
ranks 64 and Q4's ranks 132, both outside the 32-unit cutoff. Another document of
Q3's same function ranks zero. Q5 and Q6's expected spans are absent from the bounded
256-candidate window; their exact discovery failures remain unestablished. Q1 is
the excluded source. Owner-local continuation and ranking before the global cut
are the next concrete work; these traces do not prove downstream packing losses
for those misses.

A separate CLI comparison reconstructed all **52,059 bytes** of `src/boundary.rs`
byte-for-byte in each arm. Counts include every delivered response and continuation
header, using the production o200k tokenizer:

| Allowance | Old calls | New calls | Old delivered tokens | New delivered tokens | Old / new wall time |
| --- | ---: | ---: | ---: | ---: | --- |
| 512 | 36 | 27 | 14,654 | 13,705 | 32.067 / 24.819 s |
| 2,048 | 7 | 6 | 11,651 | 11,540 | 6.216 / 5.224 s |

At 512 this is 25% fewer calls and 6.48% fewer delivered tokens for identical source.
Wall times are single observations. This proves a tool-delivery improvement, not
provider-accounted task economics: host requests, repeated conversation input,
task correctness, bootstrap and maintenance are not included in that comparison.

A separate real EmbeddingGemma 2 BF16 run completed **274/274 file-overview
vectors**, with 84,927 input tokens and 35 document calls in 298.563 seconds wall
(268.450 seconds provider time). First searchable publication took 27.505 seconds.
The profile uses overview-v1, 512-token cards, batch eight, adapter revision 5,
four CPU threads, an 8 GiB worker ceiling and development isolation. The doctor
passed before each model command, with `avx512_bf16`; no compilation ran alongside
inference. The unchanged replay completed in 4.777 seconds, reusing all 274 inputs
and partitions with **zero document calls and zero input tokens**.

Both CLIs then queried the same completed overview store at 2,048 tokens:

| Real-model overview arm | Exact-span hits | Delivered tokens, eight questions | Query wall time, eight commands |
| --- | --- | ---: | ---: |
| Old CLI | 2/8: Q2, Q7 | 16,104 | 184.404 s |
| Candidate | 3/8: Q0, Q2, Q7 | 16,289 | 187.064 s |

All fourteen unanchored commands reported dense retrieval with 64 hits and current
source revision 276. Both anchored Q7 commands kept semantic retrieval off. Every
returned exact body matched its source hash and byte range, and every response was
within the requested allowance. This confirms the delivery gain with a real model;
it establishes no latency improvement. Baseline ran first for each pair, so OS
cache state was not randomized. Fresh-process dense queries took roughly 25–28
seconds each, including acquisition, serving and shutdown.

This is file-overview validation on a small public repository, not the historical
complete-hierarchy comparison or 60–80k acceptance. The first driver stopped after
successful preparation because its cleanup check included an unrelated, pre-existing
defunct worker. That failure is retained. The corrected check compares worker PIDs
before and after each command; the resumed driver verified the preparation's exit
and output hash and reused it without another preparation inference pass.
Every successful command left an empty scratch root and no new worker behind.
Initial preparation's owned worker was also absent after exit. The detached resume
shell did not leave its final wrapper receipt. The complete comparison report,
all command exit codes and final RESULT line were present; the timeout process's
retained `/proc` exit status was zero, and the observed ending boot ID matched the
start. The committed receipt identifies this observed completion evidence and the
missing wrapper receipt.

With the retained profile (model and worker hashes are in the committed receipt):

```sh
foundry --store /abs/frozen-store bootstrap --root /abs/frozen-foundry \
  --apply --components semantic --profile /abs/overview-profile.json \
  --semantic-budget-seconds 600 --development-isolation
foundry --store /abs/frozen-store context 'QUERY' --tokens 2048 \
  --strategy search --experimental-behavior-context \
  --semantic-profile /abs/overview-profile.json --development-isolation \
  --retrieval-trace-dir /abs/new-query-trace
```

Use the unchanged public `CASES` in `tests/retrieval_attribution.rs`, and run each
query through both compiled CLIs against the same completed store. Only verified
exact source bodies count as delivered evidence; outlines and addresses do not.

Full **local** gates for `c258026`, run with `bash scripts/cloud/gates.sh OUTDIR`,
passed on actual Rust 1.97.0/default and 1.90.0/MSRV:

```text
GATE fmt exit=0
GATE clippy exit=0
GATE test exit=0
GATE msrv-check exit=0
GATE msrv-clippy exit=0
```

Cargo reported **1,171 passed, zero failed, 28 ignored**; the separate recovery
harness passed. Gateway's 54 enabled tests passed as uid 1000, including both
permission-sensitive tests. The additional
`cargo check --locked --no-default-features --lib` passed with zero warnings.
The focused suite passed 569 tests with three ignored. The separate real-framework
diagnostics were not rerun. Native macOS/GPU acceptance remains an explicit gap.

`1f0827c` fixes only the new ignored comparison driver's missing `--handle` CLI
argument. That real CLI comparison and target Clippy passed after the fix; full
gates were not repeated for it. Earlier failed focused assertions, the first gate
attempt's missing exhaustive test-helper arm and the first diagnostic invocation
are retained in the raw logs. No CI ran: GitHub Actions is unavailable (owner
decision 2026-10-12).

Raw commands, traces, binaries, public corpus, profiles and logs are retained under
`/workspace/.context-foundry-tools/validation-runs/budget-repair-2026-10-16/`.
The [committed receipt](review/budget-repair-2026-10-16.json) records exact identities,
per-query counts, local gate log SHA-256s and failed attempts. Gates and comparisons
had explicit timeouts and matching start/end boot IDs. The shared source store
subsequently gained overview vectors for the separate real-model check; the lexical
diagnostic does not enable that profile. The recorded lexical traces remain fixed.

The public paired drivers are reproducible with a source-indexed frozen checkout
and the retained or rebuilt old CLI. Keep the fixture outside the current checkout
and preserve its canonical path when reusing a store:

```sh
export FOUNDRY_ATTRIBUTION_ROOT=/abs/frozen-foundry
export FOUNDRY_ATTRIBUTION_STORE=/abs/frozen-store
export FOUNDRY_ATTRIBUTION_BASELINE=/abs/baseline-foundry
export FOUNDRY_ATTRIBUTION_DIR=/abs/new-comparison-output
export FOUNDRY_ATTRIBUTION_TOKENS=2048
cargo test --locked --test retrieval_attribution -- \
  --exact repository_development_attribution --ignored --nocapture
# Use a fresh output directory for the separate identical-source read comparison.
export FOUNDRY_ATTRIBUTION_DIR=/abs/new-source-read-output
cargo test --locked --test retrieval_attribution -- \
  --exact repository_retrieve_budget_comparison --ignored --nocapture
```

The source fixture is `git archive ebf28f8`. The diagnostic deletes
`tests/retrieval_attribution.rs` from the index before querying, keeping literal
evaluator questions out of source evidence. Keep the model runs separate
from compilation. General retrieval, representative 60–80k inference, provider
cost benefit, broader evidence and native acceptance remain open. Unrelated
[FOLLOWUP](../FOLLOWUP.md) protocols remain parked.

## Bounded semantic bootstrap on the published VM — 2026-10-16

Bootstrap now calls the existing semantic preparation owner instead of reporting
the obsolete "009 pending" setup stub. `--semantic-budget-seconds` explicitly
selects inference after source indexing; profile existence alone stays unverified.
One engine owns indexing and preparation. Retry commands retain budget, cache cap
and isolation selection. Semantic-only application requires a completed source scan,
including a legitimately empty indexed root. Failed optional preparation preserves
lexical source. No new worker, model, dependency, download or store format was added.

The retained EmbeddingGemma 2 BF16 worker passed the real Linux numerical check:
single and batched vectors agreed under the existing tolerance, and the full
8,192-token input completed in **66.878 seconds**. The test took 92.63 seconds,
including acquisition and shutdown. This uses adapter revision 5, the pinned
llama.cpp commit, four CPU threads, an 8 GiB worker memory ceiling and explicit
development isolation. The doctor passed with `avx512_bf16`, 33.3 GiB visible RAM,
no swap and 26 GiB free disk. There is no GPU on this VM.

The bootstrap command then ran against the five pinned files of the public
`percent-encoding` 2.3.2 crate, using hierarchy-v1, 512-token cards, 64-token parents,
batch eight and a 300-second semantic budget. This is a small integration check,
not representative scale validation. Corpus coverage was five sources and 113
current units; deduplication yielded 106 ANN entries. Initial searchable coverage
appeared after 30.414 seconds, before body preparation finished.

| Local CLI run | Wall time | Embedded vectors | Document calls | Result |
| --- | ---: | ---: | ---: | --- |
| Fresh bootstrap | 73.294 s | 106 | 14 | complete |
| Unchanged replay | 4.630 s | 0 | 0 | complete; provider not acquired |
| Comment appended to `src/ascii_set.rs` | 25.706 s | 1 | 1 | complete; other inputs reused |
| Original file restored | 5.596 s | 0 | 0 | complete; original vectors reused |
| New source with missing worker bundle | 24.151 s | not reported | not reported | exit 1; lexical ready, semantic `profile_invalid` |

Each command left its scratch root empty. No live worker remained after the run.
The comment edit checks incremental input identity; it does not establish arbitrary
semantic-change coverage. The missing-worker failure returned no preparation receipt,
so its inference counters remain unknown rather than invented zeroes. Raw command
arguments, reports, profile and the driver remain in
`/workspace/.context-foundry-tools/validation-runs/bootstrap-semantic-2026-10-16/`.
The [committed counter record](review/bootstrap-semantic-2026-10-16.json) contains
input/model identities, counts and artifact hashes. The numerical vectors and test
log remain in the sibling `native-published-2026-10-16/` directory. Neither run was
interrupted; starting and ending boot IDs agree. A wrapper attempt failed before
starting Foundry because `/usr/bin/time` was absent; its logs are retained with
`first-` names. Timing uses the Python monotonic clock and includes process startup.

With a pinned local profile, the reproducible application command is:

```sh
foundry --store /abs/store bootstrap --root /abs/percent-encoding-2.3.2 \
  --apply --components lexical,semantic --profile /abs/profile.json \
  --semantic-budget-seconds 300 --development-isolation
```

Rerun the same command and inspect `semantic_preparation.document_calls`; unchanged
replay must be zero. The retained profile and public file hashes are recorded above.
The native check uses `FOUNDRY_EMBED_PROFILE` and `FOUNDRY_EMBED_RECEIPT` with
`cargo test --locked --test embedding_parity real_native_embedding_receipt -- --ignored --nocapture`.

The managed host's previous composition errors were also traced to missing task
metadata and a compact flag without composition. A fresh public copy with explicit
task ID/revision successfully returned composed evidence at 2,048 and 4,096 tokens.
The smaller response delivered the helper, caller and test bodies plus another
caller address; the larger delivered all eight selected bodies. Both correctly
reported incomplete evidence: syntactic callers are unresolved, compiler coverage
is unavailable, tests are candidates rather than observed execution, and host
application is unknown. This was a CLI composition diagnostic, not another agent
repair or an economics comparison. Raw captures are in `managed-context-2026-10-16/`.

Frozen retrieval remains **2/8**. Representative 60–80k inference, demonstrated
provider-accounted economics, the parked FOLLOWUP protocols and native macOS/GPU
acceptance remain open. The managed runtime exposes no per-agent provider usage;
no token or dollar savings are inferred from these local timings or output sizes.

Six new CLI-boundary regressions cover read-only inspection, explicit admission,
the never-scanned versus empty-indexed store distinction, lexical survival after
profile failure, budget-limited partial preparation, exact retry limits, unchanged
cache replay without a usable worker and workspace binding. The first focused run
failed because the new assertion expected `workspace_mismatch`; it was corrected to
the existing `wrong_workspace` contract. The production error behavior was unchanged.

The final rebuilt CLI also retrieved `newly_admitted` after the missing-worker
failure, then completed a clean replay in 4.442 seconds and semantic-only replay in
4.622 seconds, both with zero document calls and full current coverage. The final
CLI identity is included in the committed counter record; these checks include the
last admission-guard corrections.

Full **local** gates for `b202a9c` passed through `bash scripts/cloud/gates.sh OUTDIR`:
all five `GATE` results were exit 0 on actual Rust 1.97.0/default and Rust 1.90.0/MSRV.
Cargo reported **1,167 passed, zero failed, 27 ignored**; the separate recovery
harness passed. Gateway's 54 enabled tests passed as uid 1000, including both
permission-sensitive cases, without a root workaround. The additional
`cargo check --locked --no-default-features --lib` passed with zero warnings.
The detached gates had a 2,400-second timeout, a 2 GiB free-disk stop guard and a
stable boot ID. Logs are under the run directory's `full/` subdirectory.
No CI ran: GitHub Actions is unavailable (owner decision 2026-10-12).

| Local log | SHA-256 |
| --- | --- |
| `fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `clippy.log` | `b24eff294f76ed4fc92739f6de3f45e1da72a56e9bd8e4487ebc436907681a6d` |
| `test.log` | `ebf52af157055db5789b61643aa5c0c39ad113a761f02054f78879645a1756da` |
| `msrv-check.log` | `be81b1411cd4e66fe5d1f36e57af081ae6cd92d0df36dc71513d476ccb6ff6a1` |
| `msrv-clippy.log` | `7704b9649f089249b9dcffa812138428f553dd9d2cb52d5152324e447c06b980` |
| `lexical-check.log` | `1f8e8c9e5032bb292cb8f811f81ef8df568f67f493e0cab6bf65084e94d03036` |

## Managed host and gateway continuation — 2026-10-16

The owner's selected host is the **existing managed Codex agent runtime**. It can
call the local Foundry CLI without OMP, a generation-model download, another CLI
login or a working Z.ai subscription. This session's managed-agent tools expose no
per-agent provider usage or billing records; task execution can be checked, but
tool-output bytes cannot establish token or dollar savings.

Two fresh managed agents, both requested as `gpt-6.1-sol` with `xhigh` reasoning and
no inherited conversation, repaired identical seeded copies of the published
`percent-encoding` 2.3.2 crate. This is one public, author-visible calibration with
a named helper, not a held-out or representative task evaluation. The existing
source hashes and `PERCENT_ASSERTIONS` in
[`tests/gateway/host_task.rs`](../tests/gateway/host_task.rs) define the inputs and
external checker. The original source passes; the preserved pre-edit logs reject
the seeded bug at `%A`. Both final repairs pass the same external checker,
including malformed escapes, all 256 byte values in both hex cases, UTF-8,
borrowing and compilation with and without allocation support. Only the permitted
`src/lib.rs` changed. The baseline also added one visible test within that file.

The candidate actually used lexical `foundry context after_percent_sign --tokens
2048` against its admitted copy. It received the faulty helper's complete body,
then read surrounding source to check callers. Its initial composed-context
instruction omitted required task metadata; the resulting errors and fallback
remain in the totals. No semantic inference or native MCP attachment was exercised.

| Recorded boundary | Baseline | Foundry |
| --- | ---: | ---: |
| External correctness and protected files | pass | pass |
| Captured commands | 5 | 10 |
| Subprocess stdout + stderr | 12,545 bytes | 11,993 bytes |
| Capture-wrapper output, including receipt footers | 12,961 bytes | 12,834 bytes |
| Sum of subprocess durations | 209 ms | 957 ms |
| Separate lexical bootstrap | none | 175 ms |
| Provider input/output/cache tokens and charges | unknown | unknown |

The roughly 1% wrapper-byte difference establishes **no economic benefit**. These
counts exclude platform envelopes, prompts, agent reasoning/output and operator
preparation/verification; subprocess durations are not whole-task latency. Four
candidate commands returned nonzero: two invalid context invocations, a Git query
in the non-Git fixture, and an AGENTS search with no matches. No output was truncated.
The first baseline attempt is retained as invalid for comparison: its wrapper
replaced PATH, hid installed `rg`, and failed to record that command. Its provider
cost remains unknown. The corrected pair did not reuse that agent's conversation.
Model selection is recorded configuration, not provider attestation; complete host
prompts and platform transcripts were not frozen. These are integration observations,
not a sealed economics experiment or acceptance of T006/F2/F3.

The committed [counter record](review/managed-host-calibration-2026-10-16.json)
retains the metrics and artifact identities. Raw captures, hashes, oracle commands,
repairs and control copies remain under
`/workspace/.context-foundry-tools/validation-runs/managed-host-2026-10-16-v2/`;
the invalid first attempt remains in `managed-host-2026-10-16/`. To repeat the
calibration, use the public crate hashes, seed and external assertions in the
existing host-task test, give identical tasks to fresh managed agents, and keep
the checker outside their instructed workspaces. Use basic lexical context unless
an explicit task/revision has been created. Record all errors and wrapper output;
the instructions are not an enforced filesystem jail. Provider-accounted economic
acceptance still requires host-supplied usage and all preparation/failed-attempt costs.

`c4c095f` separately fixes the existing Z.ai gateway: it accepts the two exact
Coding Plan/standard API bases, supports the published `credential_env` binding
without a key file, and honors configured proxy/CA settings only in the gateway
child. Loopback health bypasses proxies; the coding host receives neither upstream
credentials nor transport settings. Reserved environment names refuse. Ready
metadata identifies the configured billing route; missing usage/charges stay unknown.

Local focused checks passed **315 tests** (261 library and 54 gateway), with four
ignored entries, including the subprocess-only fixture. Actual gateway/launcher
binaries exercise proxy routing, HTTPS CONNECT failure without direct fallback,
NO_PROXY, environment credentials, host credential exclusion and cleanup. These
are local provider fixtures. The first focused run failed because the new test
supplied a nonexistent CA bundle; correcting that fixture to use actual configured
trust made it pass without weakening TLS verification.

The live Coding Plan probe returned HTTP 400/code 1210 when thinking was disabled;
with `reasoning_effort: low`, it returned HTTP 429/code 1113, "Insufficient balance
or no resource package." Neither returned usage. Standard-API entitlement and
successful live generation are unverified. Separately, installed Codex CLI
`0.159.0-alpha.3` failed its smoke request with stale OAuth credentials. Neither
failure blocks the working managed runtime, and neither is counted as free usage.

Full local gates for `c4c095f` passed through `bash scripts/cloud/gates.sh OUTDIR`:
`GATE fmt exit=0`, `GATE clippy exit=0`, `GATE test exit=0`,
`GATE msrv-check exit=0`, `GATE msrv-clippy exit=0`. The actual default toolchain
was Rust 1.97.0; MSRV checks used actual Rust 1.90.0. Cargo reported **1,161 passed,
zero failed, 27 ignored**, and the separate recovery harness passed. All 54 enabled
gateway tests passed under uid 1000, including the permission-sensitive cases; no
root workaround was needed. The additional Rust 1.97.0
`cargo check --locked --no-default-features --lib` passed with zero warnings.

The detached gates ran from 18:39:58 to 18:46:59 UTC, with a 2,400-second timeout
and a 2 GiB free-disk stop guard. Every individual gate status was checked, boot
IDs match and no pause/restart was observed. The lexical check followed serially.
The doctor passed with the local activation, `avx512_bf16` present and no FAIL
lines. About 26 GiB remained free after the gates. No setup reinstall or new model
download was needed. Receipts are under
`/workspace/.context-foundry-tools/validation-runs/zai-integration-2026-10-16/`:

| Log | SHA-256 |
| --- | --- |
| `doctor.log` | `ffadd7cb68e54d7959a1ed3c374eb40ba93073fdfe0e3863a08b379b1fe5d9ea` |
| `focused.log` (invalid CA fixture) | `e2b74984f42fa5bb2e1707afa1b0bbbc300447b5312488ef363a0b9ad81486bd` |
| `focused-v2.log` | `86438472fe201b32c7119579e628d8a16fa324a91761ba7f18848f3253c9d860` |
| `coding-probe-result.json` | `966d7ed7b2bdc7c637f2545df03400827c4f50698ea33847f12a9238de12606a` |
| `coding-probe-low-result.json` | `c6bf69bf2c0bdede3436bb79f8848d9cbf911177d3c1f613ff49457f69f1e256` |
| `full/gates.log` | `cbb0c5169f3b54868bddcbc7db13a8d2c150e29a94daaae41fab7136d82a120f` |
| `full/env.txt` | `56802d37a494690521946c3d18829bfe730abe137c974e14f6bd3210912c3ab0` |
| `full/default-versions.log` | `b5d809c65162ea93252e9d6f929b07d2a551ca36dd5cb4361f65359132bfe85a` |
| `full/msrv-versions.log` | `cc250b5865c44041711cb5a7796974fe5f60f38b3d8d1c5fc2a0e088589cf532` |
| `full/fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `full/clippy.log` | `1bf4a8a536f8987c26e11616814ee284fc525ca0114a2d85839c57808476f5a6` |
| `full/test.log` | `08c9b22041d4fbfc1c419f616c4063032a3dba73b6a6d4ede0e7ea3df22a085f` |
| `full/msrv-check.log` | `f1a1371c556a743024812a43afec745848b6a7e0ea9f2c3b53253d2d1c67c457` |
| `full/msrv-clippy.log` | `f669e3f52135665e84e7a8466682e60d4099e5ffe6516ce2b0049e60be8706f5` |
| `full/lexical-check.log` | `9b0f53e629ef2eef449ac20efc7adea6eba0c27b1458a4bc0e4b2d9060c55c46` |

Frozen retrieval remains **2/8**. Full representative semantic inference, broad
evidence acceptance and task economics remain open; parked FOLLOWUP protocols were
not run. Native macOS/GPU acceptance is an explicit gap. No CI ran: GitHub Actions
is unavailable (owner decision 2026-10-12).

## Portable full Linux gates — 2026-10-16

`c88b83d` makes the cloud gates wrapper accept a trusted, non-secret `CF_GATE_ENV`
file after the repository defaults. It preserves the local tokenizer, compiler and
native-tool paths while requiring disabled incremental compilation and dev/test
debuginfo. Receipts identify the override by path and SHA-256, exclude private
archive URLs, and omit arbitrary environment variables. No Rust application,
dependency or lockfile changed.

Twelve wrapper boundary cases passed: default settings, an override path containing
spaces, four invalid-file cases, and nonzero/unset cases for each of the three
build settings. The receipt checks include exclusion of synthetic credentials and
private archive URLs. The actual doctor also passed with the local activation.

`bash scripts/cloud/gates.sh OUTDIR` ran locally with actual Rust 1.97.0 for the
default gates and actual Rust 1.90.0 for the MSRV gates. All five reported exit 0:
`fmt`, `clippy`, `test`, `msrv-check`, `msrv-clippy`. Cargo reported **1,155 passed,
zero failed, 26 ignored**; the separate recovery harness also passed. The gateway
target passed all 49 enabled tests (two ignored), including both permission-sensitive
fault tests, under uid 1000. No root workaround or unprivileged rerun was needed.
The additional Rust 1.97.0 `cargo check --locked --no-default-features --lib` passed
with zero warnings.

The single detached gates job ran from 14:50:49 to 14:58:05 UTC with a 2,400-second
timeout and a stop guard at 2 GiB free disk. Its exit receipt is 0; every individual
`GATE` line was checked because the underlying runner's final status alone does not
prove success. Boot IDs match and no pause/restart was observed. No CI ran: GitHub
Actions is unavailable (owner decision 2026-10-12). Native macOS/GPU acceptance is
an explicit gap. No model inference, hosted GLM request or parked evaluation ran;
retrieval, representative scale, task quality and economics remain unaccepted.

Receipts are under
`/workspace/.context-foundry-tools/validation-runs/gates-portability-2026-10-16/`.
SHA-256:

| Log | SHA-256 |
| --- | --- |
| `wrapper-cases.json` | `dec50b67c547930f15387db499696aedfc228fe6b6f3cf373bba0466bdf52af5` |
| `doctor.log` | `1f3f1c1c1753b923d22a293106e56b01d16ebd1bb311688995faf8efdc40c417` |
| `full/gates.log` | `cbb0c5169f3b54868bddcbc7db13a8d2c150e29a94daaae41fab7136d82a120f` |
| `full/env.txt` | `56802d37a494690521946c3d18829bfe730abe137c974e14f6bd3210912c3ab0` |
| `full/default-versions.log` | `b5d809c65162ea93252e9d6f929b07d2a551ca36dd5cb4361f65359132bfe85a` |
| `full/msrv-versions.log` | `cc250b5865c44041711cb5a7796974fe5f60f38b3d8d1c5fc2a0e088589cf532` |
| `full/fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `full/clippy.log` | `5e746282ede447e6214cc5d0be25edcfdc453ce0be851d6368d432c4dd6fa25e` |
| `full/test.log` | `7584034569723bd4f4ca5950ea96f2a32aa07579f43b7d671b67d3c844a9d3ed` |
| `full/msrv-check.log` | `192f167b65190262e9e74cd6851775a358821210dd7b0f078364cd51a20a7709` |
| `full/msrv-clippy.log` | `93a6bda1f2aa02804b87c0102b876569131858c7f7396d7e08a618e493e09475` |
| `full/lexical-check.log` | `3b0667c88a03bc60ae5719dc5e0f188825e99d2bba89f4921f26dd6f7b9cffb9` |

## Portable Linux setup — 2026-10-16

`7be514e` changes the cloud doctor and its documentation; no Rust application,
dependency or lockfile changes. This Codex sandbox runs as uid 1000, with a
four-CPU quota, 32 GiB memory limit, no swap and `avx512_bf16`. It reuses retained
Rust 1.90.0, CMake 4.4.4, system libclang 19, pinned llama.cpp and checksum-verified
EmbeddingGemma 2/tokenizer assets. Rust 1.97.0 was installed for formatting and
future gates. No generation weights were downloaded.

The doctor passes with explicit local paths, the actual local preparation receipt
and `CF_GENERATION_REQUIRED=0`. Generation is reported unconfigured/unvalidated;
the default still requires weights. Twelve actual doctor cases passed, including
refusal of invalid policy values, missing/empty/directory receipts, invalid or empty
libclang overrides, and a directory containing only `libclang-cpp`.

Local Rust 1.90.0 verification:

- `cargo test --locked --offline --test cli --test task_retrieval --test learning_data`:
  **94 passed**, zero failed/ignored (16 CLI, 11 task retrieval, 67 learning data).
- `cargo check --locked --no-default-features --lib`: exit 0, zero warnings.
- `cargo build --locked --features embed-worker-avx512-bf16 --bin foundry-embed`:
  exit 0, zero warnings. Running `foundry-embed --notices DIR` also exited 0 and
  emitted pinned llama.cpp commit `b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea`
  and its license. This verifies native compilation/linking, not model inference.
- Rust 1.97.0 `cargo fmt --check`, shell syntax and documentation links passed.

The initial offline attempt failed on a missing locked `toml_parser` dependency.
An online locked retry fetched dependencies and printed 94 passes, but its shell
was killed before writing the final exit receipt, so that attempt is not the
accepted run. A detached, explicitly timed offline replay completed with exit 0;
its log is the accepted receipt. No VM pause/restart was observed. The first native
check launcher refused the missing test-exit receipt without building anything;
the detached retry completed both build checks and the executable smoke check.

Full merge gates and clippy were **not run** at this focused setup checkpoint.
The wrapper's local-path mismatch was subsequently resolved and the full gates
passed in the [next receipt](#portable-full-linux-gates--2026-10-16).
No CI ran: GitHub Actions is unavailable (owner decision 2026-10-12).
Native macOS/GPU acceptance is an explicit
gap. No parked evaluation, model inference or hosted GLM request ran; retrieval,
scale, task quality and economics acceptance remain unchanged.

Receipts are under `validation-runs/doctor-portability-2026-10-16/`, except the
doctor baseline in `validation-runs/setup-2026-10-16/doctor-final.log`. SHA-256:

| Log | SHA-256 |
| --- | --- |
| `doctor-final.log` | `22a1f4ab7cb4ec266edb20d8539b71083b822165b4e77e409095530b31eb9eeb` |
| `doctor-cases.json` | `80580bf236a6794bbe866f4cb22e58cc9c1569084c2cef88c99882ad53dfc775` |
| `versions.log` | `dbecaed9b9656f8c482b076a066ff5ebe97c71f99588ff4a2577b03849979c2e` |
| `focused-tests.log` (offline refusal) | `2577bc83742d80a5d152663392fcb7b80a0f18af619a0e50c315098609e98fb7` |
| `focused-tests-online.log` (no exit receipt) | `d772f9fe25b268fbe059b182a853b09e9a89aa678ac27ada42a506cbf1f06289` |
| `focused-tests-detached.log` (accepted) | `ec18076491bff9247dbbcbc030aa937099fcbdcf65e9a73f6db21918628b11df` |
| `lexical-check.log` | `d8d5f5858615db0d257aaf642a358804aa8a1fa754835ec2c060870b58dfa074` |
| `embed-worker-build.log` | `4e7c832712b5dc70ab6b1417485d3a84953597947257398dc5726277669b7e24` |
| `embed-worker-notices.log` | `aed2eb35fa32059951692386e2195197424a5fc82bb3e5883634fb5f3b1888f8` |
| `fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

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
the logs. Earlier Claude Code sessions ran the `test` gate as root, which bypasses
the read-only permissions two gateway fault tests rely on, so those tests were
rerun unprivileged. The 2026-10-16 Codex sandbox runs as uid 1000 and passes them
directly.

| Commit | fmt | clippy 1.97 | test 1.97 | msrv-check | msrv-clippy |
| --- | --- | --- | --- | --- | --- |
| `65fd799` | 0 | 0 | 0: 1,315 passed, 34 ignored; pilot-capture gates 0 ([receipt](#watcher-test-repair-f18--2026-11-10)) | 0 | 0 |
| `854cf88` | 0 | 0 | 101: 1,312 passed, 2 timing failures (watcher F18, cancellation), 34 ignored; pilot-capture gates 0 ([receipt](#owner-decisions--2026-11-06)) | 0 | 0 |
| `8600fc0` | 0 | 0 | 0: 1,314 passed, 34 ignored ([receipt](#owner-decisions--2026-11-06)) | 0 | 0 |
| `16b1e39` | 0 | 0 | 0: 1,307 passed, 34 ignored ([dependency cleanup receipt and log digests](#dependency-and-branch-cleanup--2026-10-26)) | 0 | 0 |
| `38dc370` | 0 | 0 | 0: 1,287 passed, 34 ignored; gateway 54 passed ([receipt and log digests](#architecture-refactor-and-dependency-updates--2026-10-25)) | 0 | 0 |
| `d72f3bb` | 0 | 0 | 0: 1,174 passed, 28 ignored; gateway 54 passed; two test threads ([receipt and log digests](review/evidence-selection-2026-10-17.json)) | 0 | 0 |
| `81c20ad` | 0 | 0 | 101: 1,172 passed, 2 failed, 28 ignored; ownership race and MCP deadline, retained in the same receipt | 0 | 0 |
| `f782dda` | 0 | 0 | 0: 1,173 passed, 28 ignored; gateway 54 passed under uid 1000 ([receipt and log digests](review/owner-retrieval-2026-10-16.json)) | 0 | 0 |
| `c258026` | 0 | 0 | 0: 1,171 passed, 28 ignored; gateway 54 passed under uid 1000 ([receipt](#budget-allocation-and-evidence-preservation--2026-10-16)) | 0 | 0 |
| `1f0827c` | 0 | targeted attribution Clippy: 0 | ignored paired source-read diagnostic: 0; full gates not repeated | not repeated | not repeated |
| `b202a9c` | 0 | 0 | 0: 1,167 passed, 27 ignored; gateway 54 passed under uid 1000 ([receipt](#bounded-semantic-bootstrap-on-the-published-vm--2026-10-16)) | 0 | 0 |
| `c4c095f` | 0 | 0 | 0: 1,161 passed, 27 ignored; gateway 54 passed under uid 1000 ([receipt](#managed-host-and-gateway-continuation--2026-10-16)) | 0 | 0 |
| `c88b83d` | 0 | 0 | 0: 1,155 passed, 26 ignored; gateway 49 passed under uid 1000 ([receipt](#portable-full-linux-gates--2026-10-16)) | 0 | 0 |
| `7be514e` | 0 | not run | not run (94 focused tests on 1.90; [receipt](#portable-linux-setup--2026-10-16)) | not run (lexical library check: 0) | not run |
| `d6903f7` | 0 | 101: three new lints | 101: 1,145 passed, only the 2 root-only failures | 0 | 0 |
| `ef524b9` | 0 | 0 | 101: 1,145 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `b432f46` | 0 | 0 | 101: 1,145 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `87f4d08` (#15) | 0 | 0 | 101: 1,145 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `84108a1` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `14729de` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `9a86665` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `1c924a1` (#17 head) | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `94b9219` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `7353251` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `e3c7b08` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `7dff09b` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `9d65e9d` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `17697da` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `e814da2` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `57ae9de` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `1cb3173` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `7c25def` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `385a593` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `3beac37` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `1cced0f` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `973548d` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `f072a11` | 0 | 0 | 101: 1,146 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `1f29ec2` | 0 | 0 | 101: 1,150 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `b32ec73` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `a780ceb` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `7a4f0e9` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `ae6bc38` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `27603cf` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `b4dda7c` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `89ac751` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `19271cd` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `014870d` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `3a62678` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `201232d` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `efd769d` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `0e5f1bf` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `105a208` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `ed7a501` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `edb6a20` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `3b02871` | 0 | 0 | 101: 1,152 passed, the 2 root-only failures and one budget-boundary failure ([FOLLOWUP F17](../FOLLOWUP.md#f17-teststask_evidencers-budget-depended-on-the-random-workspace-id-fixed-2026-10-14)); unprivileged rerun of the gateway target: 49 passed | 0 | 0 |
| `b4eca2c` | 0 | 0 | 101: 1,153 passed, only the 2 root-only failures; unprivileged rerun of the gateway target: 49 passed | 0 | 0 |

Log SHA-256 digests:

| Commit | clippy | test | msrv-check | msrv-clippy |
| --- | --- | --- | --- | --- |
| `d6903f7` | `712708a6…` | `d37d829e…` | `437b9d06…` | `368e057b…` |
| `ef524b9` | `fcc7dfca…` | `ea77740f…` | `0d73feed…` | `243e2afe…` |
| `b432f46` | `38c4c123…` | `276e988f…` | `b0f11c35…` | `45f25865…` |
| `87f4d08` (#15) | `906f36d9…` | `86080107…` | `e47338e5…` | `45d5668c…` |
| `84108a1` | `65c78323…` | `302dc862…` | `fd1c818b…` | `879e530f…` |
| `14729de` | `93e65aae…` | `0bfde1e7…` | `1f5d1f30…` | `fe368598…` |
| `9a86665` | `afb5e4c6…` | `e0db4974…` | `f164437c…` | `03feb756…` |
| `1c924a1` (#17 head) | `8f63fb23…` | `26dfc62c…` | `d0a1eb9d…` | `d9b291e6…` |
| `94b9219` | `1f788b66…` | `01605187…` | `248e1366…` | `2f8d3209…` |
| `7353251` | `e7db2afc…` | `0fcd3c66…` | `8482a812…` | `1ad0e205…` |
| `e3c7b08` | `7577aa7e…` | `9b6ce778…` | `ef13260c…` | `4e61ede7…` |
| `7dff09b` | `24476047…` | `714347e9…` | `5655425f…` | `25af98a4…` |
| `9d65e9d` | `158a7e90…` | `1231a989…` | `d6da8bc6…` | `b824705a…` |
| `17697da` | `43ddda3f…` | `56c8b70e…` | `865a112a…` | `952dc801…` |
| `e814da2` | `703e7eca…` | `15378f34…` | `e02f19e9…` | `db5fd4b2…` |
| `57ae9de` | `661da8b8…` | `ef26e109…` | `fc3f61ec…` | `7667c9d0…` |
| `1cb3173` | `d7ba34e2…` | `b375c223…` | `90caeffd…` | `7ab1b402…` |
| `7c25def` | `7577aa7e…` | `95dfe823…` | `7409415a…` | `44c679a6…` |
| `385a593` | `03329197…` | `330a4300…` | `9594fa71…` | `f15bfc3e…` |
| `3beac37` | `661da8b8…` | `5afd6241…` | `7259fa1b…` | `8be0bcc9…` |
| `1cced0f` | `a71c43d3…` | `ef9d0bdc…` | `ae5cbb26…` | `acdcbb20…` |
| `973548d` | `24476047…` | `875386bf…` | `aa8b8db9…` | `5b3cefa8…` |
| `f072a11` | `6d982f82…` | `039e7109…` | `eab84e95…` | `b1d7d4d3…` |
| `1f29ec2` | `703e7eca…` | `9bde0db8…` | `36ac8f23…` | `56e85a59…` |
| `b32ec73` | `970c7429…` | `dcdacf71…` | `0d550f41…` | `ff1c8aea…` |
| `a780ceb` | `1f788b66…` | `56e23611…` | `eb05a7e3…` | `513da650…` |
| `7a4f0e9` | `703e7eca…` | `d18fe10f…` | `a85516af…` | `a34eb198…` |
| `ae6bc38` | `661da8b8…` | `0f742bc1…` | `97d21d24…` | `b1967fae…` |
| `27603cf` | `d374a05e…` | `e4304a55…` | `40de387e…` | `0956084c…` |
| `b4dda7c` | `d7ba34e2…` | `11e8e726…` | `96a87cd3…` | `2ae49b33…` |
| `89ac751` | `1f788b66…` | `ac6d6dac…` | `e03ac533…` | `12b3e74e…` |
| `19271cd` | `b88a7994…` | `e6707c83…` | `d59def12…` | `b824705a…` |
| `014870d` | `6d982f82…` | `f74b8658…` | `030fb810…` | `daf14ee3…` |
| `3a62678` | `da5f1937…` | `7aa475f3…` | `fc014764…` | `363989cc…` |
| `201232d` | `1d4d60a0…` | `3d153e44…` | `0c2f76e7…` | `fc1721c9…` |
| `efd769d` | `661da8b8…` | `2c31a6a4…` | `b4b30c20…` | `586783d5…` |
| `0e5f1bf` | `f581e6dc…` | `2357b750…` | `0e884b33…` | `c6e01741…` |
| `105a208` | `24476047…` | `121a9d92…` | `dcc5ff13…` | `1402d137…` |
| `ed7a501` | `d7ba34e2…` | `a6c5d8f8…` | `f789f14b…` | `9a0e434c…` |
| `edb6a20` | `703e7eca…` | `b0864715…` | `3eca7dc7…` | `7e32a6df…` |
| `3b02871` | `703e7eca…` | `9ff155a1…` | `24476047…` | `d7ba34e2…` |
| `b4eca2c` | `3173a02b…` | `8304e7a2…` | `0d94049b…` | `f0b3a92a…` |

The `ef524b9` run started with only the new `FOLLOWUP.md` untracked. Its counts equal
`d6903f7`'s because `ef524b9` reverts the RRF candidate. `b432f46` started from a clean
tree. It applies Dependabot updates #10, #8 and #7 to this line (see
[dependencies](dependencies.md)); `sha2` 0.11 (#6) was left out because it fails to
compile. Each update was also checked alone on Rust 1.90. With `bindgen` 0.73 both
llama.cpp workers build, and the regenerated bindings differ only in 20 opaque handle
types losing `Copy`/`Clone`.

On 2026-10-13, `87f4d08` (Dependabot #15, merged as `6ba885a`) moved `tokio-util` from
0.7.19 to 0.7.20. It is a lock-only change: Dependabot's resolver also re-points existing
`windows-sys` and `getrandom` edges, adding and removing no packages. `84108a1`, on a
clean tree, covers the `sha2` 0.11 migration, the progressive-bootstrap test fix and the
doc corrections; its extra passing test is the pinned SHA-256 vector test. Both receipts
are local only: GitHub Actions did not run.

`14729de` and `9a86665` are the two god-module splits described
[below](#god-module-splits--2026-10-13). Each started from a clean tree and was gated
before the next commit, with `scripts/cloud/environment.env` sourced and
`RUSTUP_TOOLCHAIN` unset so the gates pick their own toolchains. Two earlier attempts on
`14729de` are invalid and not counted. The first ran without that file's debuginfo-off
profiles and ran out of disk while linking the tests. The second ran without its
tokenizer path, so 52 `learning_data` tests could not read their fixture; in that run
`continuous_semantics_progresses_during_filesystem_traffic_and_converges` also stopped
on the [F13](../FOLLOWUP.md) race, and it passed in both counted runs. The receipts are
local only: GitHub Actions did not run.

`1c924a1` is the head of PR #17, gated again before it merged as `1593c2c`. The eleven
commits after it are the second batch of god-module splits
([below](#god-module-splits--2026-10-13)), each gated the same way before the next
split started. Each run also reran the gateway target as `nobody`, ran
`cargo check --locked --no-default-features --lib` (0 warnings every time) and compared
rustdoc. All of it is local only: GitHub Actions did not run.

`72c4319` fixes the three Clippy 1.97 lints (`byte_char_slices`, a manual `Option::zip`,
`while_let_loop`) without changing behavior. `f491821` fixes a regression from
`8ef3acc`: `cargo build --no-default-features --lib` had stopped compiling, because
`syntax::units_with_certainty` was gated on the `semantic` feature while a caller
outside that feature used it. The build now compiles without warnings. Native macOS
acceptance did not run: there is no Mac here, and Actions is unavailable.

Two integration tests in `tests/neural_retrieval.rs` failed once each in a full parallel
run on this 4-vCPU host and passed on rerun. A follow-up investigation on 2026-10-13
found that neither failure is a deadline problem:

- `progressive_bootstrap_serves_priority_before_the_unprepared_tail_and_resumes`: the
  driver publishes a generation before it snapshots bootstrap progress
  (`src/neural/driver.rs`, `publish` then `sync_progress`). A status read in between sees
  the searchable unit before `first_searchable_ms` is set. The test now waits, within the
  existing 60-second helper bound, until both are visible; its assertions are unchanged.
- `continuous_semantics_progresses_during_filesystem_traffic_and_converges`: reproduced
  1 in 6 full-suite runs. The semantic lane stops in `paused` with `partition_invalid`
  ("the mapping names a stale source version") after a source changes between
  partitioning and accept, so it cannot converge at any deadline. This is a product race,
  parked as [FOLLOWUP F13](../FOLLOWUP.md) and fixed since `1f29ec2`
  ([below](#f13-superseded-partitions--2026-10-13)); the test is unchanged.

### sha2 0.11 migration — 2026-10-13

`sha2` 0.11 returns a digest type without `LowerHex`, and 18 sites wrote stored SHA-256
strings with `format!("{:x}", ...)`: source hashes, cache keys and artifact pins. The
migration ran in two steps so byte identity is proven, not assumed:

1. **`16093de`, still on 0.10:** all sites move to `context_foundry::lower_hex`, which
   writes two lowercase hex digits per byte in order. The 3 test oracles encode
   inline. A test asserts `lower_hex(d) == format!("{:x}", d)` for every input length
   0..=300 and passes, together with the pinned FIPS 180-2 vectors (`""`, `"abc"`, the
   448-bit message, one million `a`) and a streamed hash.
2. **The bump to 0.11.1:** the `{:x}` comparison can no longer compile and is removed.
   The pinned vectors pass unchanged on 0.11, so the stored encoding is identical.
   All targets check on Rust 1.90, including `foundry-embed` with `embed-worker`.
   sha2 0.10.9 remains only under the optional `learning-worker` feature.

### God-module splits — 2026-10-13

An audit ranked the source files by production size, function count, concerns,
fan-in, churn and hot-path use; the table and the remaining plan are in
[FOLLOWUP F14](../FOLLOWUP.md#f14-god-module-splits-audit-2026-10-13). Two splits that
were purely mechanical ran, one commit each:

- **`14729de`:** `src/mcp.rs` lines 1471–3718 at `a088969` (the `FoundryMcp` router and tools,
  delivery and multi-root fan-out, the `ServerHandler`) moved to `src/mcp/tools.rs`.
  The parent gained `mod tools;` and `pub use tools::FoundryMcp;`.
- **`9a86665`:** `src/store.rs` lines 1550–1630 at `a088969` (the behavior-intent heuristic) and
  6243–6711 (`Engine::context_candidates`, `context_candidates_memory`,
  `context_candidates_with`) moved to `src/store/context.rs`, the methods inside one
  `impl Engine` block. The parent gained `mod context;`.

Evidence for each, all local:

- **Verbatim:** the removed line ranges equal the new file's body byte for byte, and
  the parent differs only by the added `mod`/`use` lines. No body, signature or
  visibility changed, and no test file changed.
- **Public API:** `cargo doc --no-deps --lib --features test-faults` before and after.
  With source links stripped, all 1,011 pages of the baseline are unchanged except as
  noted below. Rustdoc adds a redirect page for the private child path
  (`mcp/tools/struct.FoundryMcp.html`); it is not a public path.
  On the `Engine` page the moved methods now sit in a separate `impl Engine` block, so
  the page gains one block header; its 114 documented members (94 methods) keep
  identical signatures and docs.
- **Feature builds:** `cargo check --no-default-features --lib` passes with no warnings,
  as before. The gates compile the default build and, through the crate's self
  dev-dependency, `test-faults`.
- **Gates:** the receipts above, run after each commit.

**Second batch.** Eleven more splits, one commit each. Each was gated before the next
started, with the same evidence as above plus two checks: the gateway target rerun as
`nobody` and a rustdoc comparison per commit. All of it is local only.

| Commit | Split | Parent lines | New file (moved lines) |
| --- | --- | --- | --- |
| `94b9219` | `owners`: the three `Owner` impls on `Shared` | `src/mcp.rs` 4,260 → 4,090 | `src/mcp/owners.rs` 176 (170 moved) |
| `7353251` | `transport`: stdio, HTTP gate and session cap, with 3 tests | `src/mcp.rs` 4,090 → 3,313 | `src/mcp/transport.rs` 789 (776 moved) |
| `e3c7b08` | `retrieve`: `Engine::retrieve`, `retrieve_outline`, the verified handle read | `src/store.rs` 7,104 → 6,939 | `src/store/retrieve.rs` 173 (165 moved) |
| `7dff09b` | `index_hooks`: inline test-seam module to a file | `src/store.rs` 6,939 → 6,828 | `src/store/index_hooks.rs` 110 (110 moved) |
| `9d65e9d` | `sources`: scans, source writes, the pending queue, the drains | `src/store.rs` 6,828 → 6,412 | `src/store/sources.rs` 424 (416 moved) |
| `17697da` | `indexing`: the parallel refresh | `src/store.rs` 6,412 → 5,748 | `src/store/indexing.rs` 673 (664 moved) |
| `e814da2` | `lifecycle`: initialize, open, upgrade, status, repair | `src/store.rs` 5,748 → 4,967 | `src/store/lifecycle.rs` 788 (780 moved) |
| `57ae9de` | `refinement`: owner scopes and `DenseWindow::refine`, with 7 tests | `src/neural/query.rs` 2,358 → 1,439 | `src/neural/query/refinement.rs` 933 (921 moved) |
| `1cb3173` | `runtime`: the provider-thread runtime, with the remaining test module | `src/neural/query.rs` 1,439 → 449 | `src/neural/query/runtime.rs` 999 (992 moved) |
| `7c25def` | `references`: the references query and exact doors | `src/graph.rs` 2,954 → 1,372 | `src/graph/references.rs` 1,595 (1,589 moved) |
| `385a593` | `edges`: the manual edge graph | `src/graph.rs` 1,372 → 777 | `src/graph/edges.rs` 606 (599 moved) |

- **Verbatim:** a script compares every moved range of the parent commit with the new
  file, byte for byte. It also checks that the new parent equals the old one minus those
  ranges plus the inserted `mod`/`use` lines. A child's only other lines are its `//!`
  summary, `use super::*;`, an `impl Engine { … }` wrapper around moved methods, and a
  `#[cfg(…)] mod tests { use super::*; … }` wrapper around moved tests. Three commits
  differ slightly:
  - `7dff09b` turns an inline module into a file, so each of its 110 lines equals the
    original minus exactly four leading spaces.
  - `1cb3173` moves its test module whole, header included.
  - `57ae9de` and `1cb3173` add `use super::cache;` and `use super::area;` to the parent:
    the moved code reaches those modules by the relative paths `super::cache` and
    `super::area`, which now resolve through the parent.
- **Moved tests still run:** the 7 refinement, 6 runtime and 3 transport tests ran and
  passed under their new module paths, and the total stayed at 1,146.
- **Public API:** rustdoc was built into an empty `doc/` directory at `1593c2c` and at
  every commit, and each pair of neighbouring builds was compared page by page with
  source links stripped, member by member (signature and docs).
  - No page was removed. All 16,173 members and all 31 rustdoc warnings are unchanged.
  - The 29 added pages are redirect stubs for the private child paths of re-exported
    items (1,012 → 1,041 pages).
  - The `Engine` page gains four `impl Engine` block headers (retrieve, sources,
    indexing, lifecycle; 50 → 54) and its blocks are reordered; its 114 members are
    unchanged.
  - One rustdoc-internal attribute changed, on `neural/query/type.MakeProvider.html`.
    Its `data-self-path`, which the page's script uses to load the impls of the aliased
    `Box`, now names the definition path `neural::query::runtime::MakeProvider`. The
    rendered impls are the same.
  - The first per-commit comparisons reused one output directory, and rustdoc never
    deletes stale pages; every comparison above was redone from clean builds.
- **Revert:** reverting all eleven in reverse order restores `1593c2c`'s tree exactly.
  Seven also revert cleanly alone. `e3c7b08`, `9d65e9d`, `57ae9de` and `7c25def` conflict
  alone, where a later split of the same file inserted its `mod`/`use` lines next to
  theirs or removed the code next to their range. The conflicts are positional; no line
  is claimed by both sides.
- **Compiler split, first attempt:** its mapped boundary needed two visibility changes
  (E0624 on `OccurrenceKind::tag` and `from_tag`), so it was discarded before any commit.

**Follow-up, `3beac37`.** The `src/graph.rs` compiler split ran with `OccurrenceKind` and
its `impl` kept in the parent, next to the storage key codec that calls them. Three
ranges moved verbatim: the tables, the identities and rows, and the compiler
`impl Engine`, 536 lines in all. `src/graph.rs` went from 777 to 247 lines, and the new
`src/graph/compiler.rs` has 544. The parent gained `mod compiler;` and two re-export
blocks: 12 public items and 8 crate-internal items.

The commit passed the same local gates as the batch: fmt, Clippy 1.97, MSRV check and
Clippy 1.90, and 1,146 tests with only the 2 root-only failures, with the gateway target
passing 49 as `nobody`. `--no-default-features` gave 0 warnings. A clean rustdoc
comparison against `c23279e` showed pages 1,041 → 1,053 (12 redirect stubs for the re-exported items), all members unchanged and the 31
warnings unchanged. GitHub Actions did not run.

**`syntax.rs` self-sample, `1cced0f`.** This applies the owner's decision on
[FOLLOWUP F15](../FOLLOWUP.md): the Rust input of
`per_character_reads_give_the_whole_buffer_tree` is now
`tests/fixtures/syntax/syntax-rs-sample.txt`, a byte-identical copy of `src/syntax.rs`
at `5d9596d` (202,038 bytes, SHA-256 `6d909735…7b33035`), instead of
`include_str!("syntax.rs")`. At this commit the test parses exactly the bytes it parsed
before, and it passed. The commit passed the same local gates: fmt, Clippy 1.97, MSRV
check and Clippy 1.90, 1,146 tests with only the 2 root-only failures, the gateway target
passing 49 as `nobody`, and `--no-default-features` with 0 warnings. Rustdoc showed pages
1,053 → 1,053 unchanged, with all members and the 31 warnings unchanged. GitHub Actions did not run.

**`syntax.rs` splits, `973548d` and `f072a11`.** These are the two splits of
`src/syntax.rs` that need no visibility change, run after the test sample was frozen.
Each moved its ranges verbatim (proof as above) and was gated before the next:

| Commit | Split | `src/syntax.rs` lines | New file (moved lines) | Rustdoc |
| --- | --- | --- | --- | --- |
| `973548d` | `outliner`: `ELIDE_LINES`, `Segment`, `outline`, `render_outline`, `Outliner` | 5,075 → 4,690 | `src/syntax/outliner.rs` 397 (391) | 1,053 → 1,057 pages, 4 redirect stubs |
| `f072a11` | `tokens`: `code_subtokens`, `identifier_runs`, `path_segments`, `address_segments` | 4,690 → 4,584 | `src/syntax/tokens.rs` 112 (106) | 1,057 → 1,061 pages, 4 redirect stubs |

`COMMENT_LINES` stays in the parent because unit extraction also uses it. The second
child is `tokens` because `src/syntax.rs` already has a private `fn text`. Both commits
passed the same local gates:

- fmt, Clippy 1.97, and MSRV check and Clippy 1.90;
- 1,146 tests with only the 2 root-only failures, the gateway target passing 49 as
  `nobody`;
- `--no-default-features` with 0 warnings;
- clean rustdoc comparisons, with all members and the 31 warnings unchanged.

GitHub Actions did not run.

### F13 superseded partitions — 2026-10-13

`1f29ec2` fixes the stale-source partition race recorded in
[FOLLOWUP F13](../FOLLOWUP.md#f13-continuous-semantics-stopped-on-a-stale-source-partition-race-fixed-2026-10-13),
as designed there. A partition whose source was replaced or deleted while it was
computed is now superseded: the driver drops it with its cards, the source's newer
version stays pending, and a later batch derives it. Before, accept refused it with
`partition_invalid` or `not_found`, and the continuous driver stopped in `paused`
without converging. The public `semantic_record_partition` still refuses a stale hash,
and the CLI `run`, which opens the store exclusively and so cannot meet the race, keeps
the old error.

- **Red first:** four tests in `src/neural/driver.rs` drive the race through the fault
  points, with replace and delete variants (the mid-wave test replaces only). On the
  parent `6dff64a` they fail with
  `partition_invalid` when the source is replaced and `not_found` when it is deleted;
  on `1f29ec2` they pass.
- **Contract unchanged:** `tests/neural.rs` passes without changes (70 tests), including
  the stale-hash refusal and the structural `partition_invalid` checks.
- **Soak:** 12 consecutive runs of `cargo test --locked --test neural_retrieval`, the
  target where the race showed about 1 run in 6 of the full parallel suite, all passed
  (93 passed, 1 ignored each) with its deadlines unchanged. They ran the target alone,
  on the code of `1f29ec2` before a test-only type alias was added, so they show the
  target still passes; the tests above are the proof of the fix.
- **Local gates on `1f29ec2`:** in the receipts [above](#local-gates-receipts), from a
  clean tree.
  - fmt, Clippy 1.97, and MSRV check and Clippy 1.90 exit 0.
  - 1,150 tests passed: the earlier 1,146 plus the 4 new ones. The only failures are the
    2 root-only gateway tests, and the gateway target passed 49 as `nobody`.
  - `--no-default-features` builds with 0 warnings.
  - Rustdoc keeps all 1,061 pages and the 31 warnings. The only changed page is
    `neural::driver`, whose module doc gained the sentence on superseded partitions; no
    member or signature changed. `PartitionCommit` is crate-internal.

  An earlier run, on the commit before an amend, stopped at Clippy 1.97's
  `type_complexity` on the new test owner's queue. The amend added a type alias, and
  that run is not counted. Native macOS acceptance did not run (no Mac), and GitHub
  Actions did not run.

**`stale_area`, `b32ec73`.** On the owner's next request, the driver's `area_pass` gives
the same treatment to an overview area that `area::bind` refuses: it skips the area
instead of stopping with `stale_area`. Since the pass's snapshot, that area's brief
changed or was forgotten, or one of its sources changed. A brief write marks overviews
pending and a source write records its path, in the same transaction, so a later pass
binds the current area or drops it. The CLI `run` keeps `stale_area`, which it cannot
reach.

- **Red first:** three tests in `src/neural/driver.rs` make a foreground write during
  an area's model call, which holds no engine slot, in three variants: the brief
  revised, the brief forgotten, a source replaced. On the parent `a126eac` every variant
  stops with `stale_area`; on `b32ec73` the pass skips that area and binds the next,
  `maintain` converges to `waiting` with nothing pending, and a one-shot pass completes
  with a changed brief still pending as an overview obligation.
- **Soak:** `neural_retrieval` alone passed 12 of 12 runs again (93 passed, 1 ignored
  each), deadlines unchanged.
- **Local gates on `b32ec73`:** in the receipts [above](#local-gates-receipts), from a
  clean tree. Every gate gave the same result as on `1f29ec2`, now with 1,153 tests
  passed (3 more). Rustdoc again changed only the `neural::driver` module page, by one
  sentence on skipped areas. Native macOS acceptance did not run (no Mac), and GitHub
  Actions did not run.

### God-module splits with visibility changes — 2026-10-14

The owner resumed [FOLLOWUP F14](../FOLLOWUP.md#f14-god-module-splits-audit-2026-10-13)
for the planned splits that need crate-internal visibility changes: planned bumps only,
and park anything that needs more. All 15 ran, one commit each, every split gated before
the next started:

| Commit | Split | Bumps | Rustdoc |
| --- | --- | --- | --- |
| `a780ceb` | `src/store.rs` → `store/doors.rs` | 1 | 1,061 pages unchanged |
| `7a4f0e9` | `src/store.rs` → `store/semantic_search.rs` | 1 | unchanged; `Engine` gains one impl section, members identical |
| `ae6bc38` | `src/store.rs` → `store/lexical.rs` | 2 | the same |
| `27603cf` | `src/mcp.rs` → `mcp/startup.rs` | 1 | unchanged |
| `b4dda7c` | `src/mcp.rs` → `mcp/semantic.rs` | 2 | 1,061 → 1,066: 5 redirect stubs |
| `89ac751` | `src/syntax.rs` → `syntax/tiling.rs` | 1 | unchanged |
| `19271cd` | `src/syntax.rs` → `syntax/forest.rs` | 1 | unchanged |
| `014870d` | `src/syntax.rs` → `syntax/markdown.rs` | 1 | unchanged |
| `3a62678` | `src/syntax.rs` → `syntax/imports.rs` | 1 | unchanged |
| `201232d` | `src/syntax.rs` → `syntax/extract.rs` | 1 | unchanged |
| `efd769d` | `src/syntax.rs` → `syntax/parser.rs` | 2 | 1,066 → 1,070: 4 redirect stubs |
| `0e5f1bf` | `src/syntax.rs` → `syntax/calls.rs` | 5 | unchanged |
| `105a208` | `src/neural/prepare.rs` → `prepare/order.rs` | 0 | unchanged |
| `ed7a501` | `src/neural/prepare.rs` → `prepare/cli.rs` | 0 | 1,070 → 1,073: 3 redirect stubs |
| `edb6a20` | `src/neural/prepare.rs` → `prepare/steps.rs` | 1 | unchanged |

- **Proof per split:** a script compared every moved range at the parent commit with
  the new child, byte for byte after applying the declared bumps, which only insert
  `pub(super)`. It also checked that the new parent equals the old one minus those
  ranges plus the inserted `mod`, `use` and re-export lines. In 5 splits, rustfmt
  wrapped a bumped signature that no longer fit on one line (`open_owner`, `forest`,
  `import_keys`, `tree_units`, `observe`). There the child was checked against rustfmt
  of the verbatim move plus the bumps, and that wrapping was the only difference.
- **Public API:** every page that existed before is unchanged in every split, compared
  member by member. The only additions are redirect stubs for re-exported items' private
  child paths. `syntax::parse_hooks`, a public module now re-exported from
  `syntax/parser.rs`, keeps its page at the same path.
- **Rustdoc warnings:** 31 in every split. In `ed7a501` one changed wording: the
  `neural::prepare` module doc links to `publish`, which was a private item and is now
  in the private `cli` child with no re-export, because nothing outside it calls
  `publish`. "Links to private item `publish`" became "unresolved link to `publish`".
  The rendered page is unchanged.
- **Local gates on every commit:** fmt, Clippy 1.97, and MSRV check and Clippy 1.90
  exit 0. 1,153 tests passed with only the 2 root-only gateway failures, and the gateway
  target passed 49 as `nobody`. `--no-default-features` builds with 0 warnings. Each run
  started from a clean tree; the receipts and log digests are
  [above](#local-gates-receipts).
- **Records commit:** `3b02871` changes only docs. Its gates run had one extra
  failure: `observed_execution_requires_fresh_proof_and_all_delivered_endpoints` in
  `tests/task_evidence.rs`, at a budget boundary. That test passed in every split's
  run and in 130 reruns on the same code. Its cause, found later: the minimum budget
  depends on how the tokenizer splits the random workspace id, which the report carries
  11 times ([FOLLOWUP F17](../FOLLOWUP.md#f17-teststask_evidencers-budget-depended-on-the-random-workspace-id-fixed-2026-10-14)).
- **Not run:** native macOS acceptance (no Mac), so the `src/learning.rs` splits stay
  parked. GitHub Actions did not run.

### F17 task_evidence budget floor — 2026-10-14

`observed_execution_requires_fresh_proof_and_all_delivered_endpoints`
(`tests/task_evidence.rs`) failed once, in the `3b02871` run above. The cause, recorded in
[FOLLOWUP F17](../FOLLOWUP.md#f17-teststask_evidencers-budget-depended-on-the-random-workspace-id-fixed-2026-10-14), is that the test's smallest budget, 1,536 tokens, sometimes fell
below the packer's minimum. That minimum is an `o200k_base` token count of a report
carrying the random workspace id 11 times, and it ranged from 1,435 to 1,551 over 301
runs.

`b4eca2c` makes the test read the minimum from a 1-token request's `BudgetTooSmall` and
check the budgets `[minimum, minimum + 512, 4096]`. No product code changed.

- **Before and after:** with a scratch fixture pinned to the root path whose minimum is
  1,547, the old test failed 5 times out of 5 and the new one passes 5 out of 5. It
  also passes at the root whose minimum is 1,459.
- **Repeated runs:** 300 runs of the new test with random roots passed.
- **Local gates on `b4eca2c`:** in the receipts [above](#local-gates-receipts), from a clean
  tree. 1,153 tests passed with only the 2 root-only failures, and the gateway target
  passed 49 as `nobody`. fmt, Clippy 1.97, and MSRV check and Clippy 1.90 exit 0, and
  `--no-default-features` builds with 0 warnings. Rustdoc is unchanged.
- **Audit of the other fixed-budget tests:** the full suite ran with every workspace
  id forced into the most expensive and then the cheapest realistic token layout.
  These are the extremes of 200,000 random ids (54 and 88 tokens for one full id plus
  one handle prefix, median 69), set by a temporary patch that was not committed. Both
  runs gave 1,153 passed with only the 2 root-only failures, so no other test depends
  on how the id tokenizes. The pre-fix test, run against the same patch, failed 5
  times out of 5 with expensive ids and passed with cheap ones. The run logs' SHA-256
  are `ac49a348…` (expensive) and `e4f27281…` (cheap).
- **Not run:** native macOS acceptance (no Mac). GitHub Actions did not run.

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

**Not measured on 2026-10-12.** The dense comparison needs the revision-5 AVX-512 BF16
worker. After the VM moved, that worker refused to load with "AVX-512 BF16 embedding
worker is unsupported on this CPU". A different embedding function was not substituted.
`ef524b9` reverted the candidate, keeping the measured 2/8 interleave arm as the
experimental placement until the comparison could run.

**Dense measurement — 2026-10-14.** A later cloud session's VM had `avx512_bf16`
(Intel Xeon at 2.10 GHz, 4 vCPUs), and [FOLLOWUP F9](../FOLLOWUP.md) ran there. All of
it is local; raw rows, traces and binary hashes are in `validation-runs/rrf-step2/`.
- **Inputs.** The corpus tar (`31ff389a…`), model (`68bae29d…`), tokenizer
  (`4d777ef5…`) and bundled revision-5 worker (`c31f4aa5…`) match their pins. The
  baseline `foundry` (`dfc1c28e…`) was built at `72c4319` on 2026-10-12. Its source
  differs from `ef524b9` only by three `#[cfg(feature = "semantic")]` attributes, which
  the default features make no-ops.
- **Preparation.** Two `semantic prepare` passes, each with a 7,200 s budget, ran with
  nothing else heavy. The first stopped with `budget_exhausted` after committing 8,384
  units. The second completed: 14,453 of 14,453 units searchable, 13,822 vectors and no
  pending paths. Together they embedded 3,608,991 input tokens, the same count as the
  [2026-10-11 preparation](#realistic-acceptance-continuation--2026-10-11). Provider time
  was 12,421,624 ms, against 2,413,715 ms there.
- **Arms.** Two release builds from a worktree, with Rust 1.97.0: `ef524b9` for
  interleave (`foundry` `db486388…`), and `ef524b9` with `9ccefcc` cherry-picked for RRF
  (`foundry` `001480df…`; its tree `f97335e…` equals `9ccefcc`'s). Each ran the
  attribution test against the same store with the semantic profile and, as a lexical
  control, the baseline binary. Budget 2,048 tokens; questions numbered from 0.

  | Arm | Delivered |
  | --- | --- |
  | Default lexical (in-process, both runs) | 1/8 (Q7) |
  | Experimental lexical (in-process and baseline binary, both runs) | 2/8 (Q2, Q7) |
  | Semantic interleave (`ef524b9`) | 2/8 (Q2, Q7) |
  | Semantic RRF (`9ccefcc`) | 2/8 (Q2, Q7) |

- **Differences.** RRF changes the delivered text of Q0 to Q6, the seven questions that
  use the dense lane; Q7 runs without it and is byte-identical. Only Q0's loss stage
  moves: from `evidence_window` (its unit was materialized, but the excerpt missed the
  span) to `packing_omission` (the unit reached the final packing input but was not
  delivered). In both arms Q3 is lost at packing, Q4 and Q5 at the per-file cap, and Q1
  and Q6 never enter the bounded candidate window.
- **Repeatability.** Rerunning the eight semantic questions from each arm's saved
  `foundry` gives byte-identical output, 8 of 8 per arm. The lexical outputs are
  byte-identical between the two runs.
- **Decision.** The candidate delivers no more expected spans than the interleave arm,
  so by F9's rule it stays reverted: `ef524b9` stands and no code changed. Retrieval
  remains **2/8**. These are exposed development questions, not held-out acceptance; F6
  still gates any retrieval acceptance claim. (Superseded 2026-11-06: F6 is retired as
  a gate, and G1, G2 and F4-style sealed sets are the only retrieval instruments; see
  the [owner decisions](#owner-decisions--2026-11-06).)

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
