# Experimental ecosystem checkpoint — 2026-10-11

## Current status — 2026-11-06

Updated 2026-11-11. This section owns current status; other documents link here,
and dated paragraphs below are history.

- **Measurement.** G2 (388 questions over rust-lang/rust and oh-my-pi, run with
  [`scripts/g2.sh`](../scripts/g2.sh) on the owner's Mac) is now development evidence.
  G1 runs on every candidate. The eight Foundry questions are a smoke check only.
  The working set lives in `datasets/context-foundry-g2-work`, because a macOS
  cleanup of `/private/tmp` deleted the earlier stores
  ([handoff](measurement-handoff.md)).
- **Enable rule (owner, 2026-10-19).** Significantly more located definitions (exact
  one-sided McNemar, p < 0.05/K), no net loss of verbatim bodies, and every response
  within 2,048 tokens. Mean tokens are reported only.
- **Default recipe: complete hierarchy (F4, 2026-10-22).** F4 ran once, as
  pre-registered, on a fresh sealed set of 300 questions: hierarchy located 130
  against cards' 100 (+64/−34, p = 0.0016; verbatim bodies 97 → 128). With G2
  (169 against 136), G1 (PASS on all sets) and the review of its 17 newly lost G1 tasks
  ([evidence](validation.md#f4-and-the-hierarchy-default--2026-10-22)), a profile
  without `representation` now selects `hierarchy-v1`. Address cards stay selectable
  as `cards-v1`.
- **What it costs.** 1.8–2.1 times the units, 5.9 hours of Metal preparation for
  rust-lang/rust against 72 minutes, and a 3.24 GB serving peak against 1.95 GB.
  Existing card stores serve lexical results until `semantic prepare` runs for the new
  recipe.
- **Not candidates.** One score with fusion (185/165 on G2, p = 0.057 against hierarchy)
  is an unmerged experiment that would need its own pre-registration. Its serving
  owner peaks at 4.91 GB on rust, against 3.34 GB for hierarchy and 1.97 GB for cards
  measured the same way ([validation](validation.md#f4-and-the-hierarchy-default--2026-10-22)).
- **Losses now.** Ranking loses 79 answers to the caps and 44 to the packer; recall
  loses 80 never retrieved. Hierarchy's 17 newly lost G1 tasks are mostly tier-1
  lexical hits placed behind every dense unit.
- **Scale.** Publication streams cached vectors (no 2 GiB vector bound) and
  generation v6 interns label-map paths and hashes (rust map 191 → 69 MB). At rust's
  density the next bounds are the mapping-view estimate near 120k files, then the label
  map near 240k and the ANN near 253k. A v5 store needs `semantic prepare` once. That
  rebuilds its ANN, which on G2 located 166 against the original build's 169.
- **Since then.** 48 refactor commits with no intended retrieval change, and six
  dependency updates ([validation](validation.md#architecture-refactor-and-dependency-updates--2026-10-25)).
  usearch 2.26.5 builds a different ANN graph than 2.26.4 from the same cache, so the
  next republish can move G2 as the v6 rebuild did; G2 was not rerun.
  The held Scala, PowerShell and pulldown-cmark updates now go in one schema bump
  gated on a unit diff (owner, 2026-11-06).
- **Open.** Local gates are the receipt; pushes from this line start no CI. Since
  2026-10-12, hosted runs came only from Dependabot and from the 2026-10-25 fix
  branches and their pushes to main. The 6,000-file watcher test missed its deadline on
  the M5 Max when disk syncs were slow; `65fd799` batches the watcher's commits
  and it passes in about 35 s ([receipt](validation.md#watcher-test-repair-f18--2026-11-10)).
  The reread symptom seen on the earlier M3 Pro (F18) has not recurred. After a reboot, repeat G2 runs differ in 36 of 388
  rows with equal totals; compare arms within one session.
- **Evidence/profile repairs, 2026-10-25.** Brief enrichment preserves retrieved
  spans/order and the 32-source cap; delivered containing bodies/excerpts can support
  a brief citation. Host configuration forwards explicit semantic/knowledge profiles
  and development admission without enabling maintenance. See the
  [scoped repair receipt](validation.md#preserve-retrieved-evidence-and-selected-profiles--2026-10-25).
  These are correctness repairs; retrieval quality, economics and real-model readiness
  are not inferred from their fixture checks.
- **Freshness and diagnostics, 2026-10-25.** Compact memory lines expose linked-source
  freshness without deleting durable decisions. Existing private traces explain final
  packing decisions, excerpt ranges, unsupported citations and keyed proof outcomes.
  Task-composed memory uses the composition's final snapshot. See the
  [receipt](validation.md#memory-freshness-and-selection-diagnostics--2026-10-25).
  Ranking and caps are unchanged. The later recommendations and pivots are deferred
  with re-entry triggers (owner, 2026-11-06).
- **Dependency cleanup, 2026-10-26.** PR #43 merged the active probe's security pins
  with an immutable pre-refresh manifest reference; historical model receipts stay
  unchanged. The rmcp/Tokio/ureq maintenance subset is included. See the
  [cleanup receipt](validation.md#dependency-and-branch-cleanup--2026-10-26).
- **Parser security repair, 2026-10-26.** `2651fdb` updates pulldown-cmark to 0.13.5.
  Existing stores must explicitly run `repair-index` for search schema 8, then
  `semantic prepare` to refit source-unit schema 6. Old semantic state/cache is
  retained, not served or labeled current; store schema 12 and generation v6 stay
  unchanged. Scala 0.26.3 was deferred, and on 2026-11-06 the owner put Scala,
  PowerShell and pulldown-cmark into one schema bump gated on a unit diff; rmcp 3.7.0
  and uuid 1.29.0 remain deferred. The owner
  authorized this security-only exception to G2 promotion measurement; no quality
  or performance gain is claimed. See the
  [migration and live CLI/MCP receipt](validation.md#markdown-parser-security-and-unit-migration--2026-10-26).
- **Task calibration and local activation, 2026-10-28.** This checkout now has its
  own prepared `.context-foundry` store; release CLI queries with the existing
  explicit profile and development-isolation flag were exercised, including
  exact citation retrieval. The public upstream coding-task pair produced two
  externally correct repairs and actual host usage. Extra native-tool exposure,
  missing reasoning categories and uncontrolled hosted cache prevent clean paired
  economic acceptance; T006 remains open. See the
  [dated calibration receipt](validation.md#paired-coding-task-and-actual-host-usage--2026-10-28).
- **Apparatus-only continuation, 2026-10-29.** The Anthropic-only author restriction
  is removed. The full actual-host no-provider preflight now passes after strict
  delivery, request-accounting, lossless-counter and enforcement-boundary repairs;
  both failed attempts and original calibration evidence are retained. See the
  [dated receipt](validation.md#no-provider-apparatus-preflight--2026-10-29).
  The guard remains fixture-only, not an authorized live-capture adapter; no new
  coding trial is authorized. The pilot's planning choices now have the single
  owner linked below, while live implementation, evidence and authorization remain gated.
- **Pilot contract freeze, 2026-10-31.** The
  [bounded pilot v1 contract](../specs/003-agent-retrieval-context/contracts/bounded-pilot-v1.md)
  owns the frozen policy and input pins; [T006](../specs/003-agent-retrieval-context/spec.md#t006--validate-retrieval-repairs-in-real-coding-tasks)
  records its external digest. This freeze authorizes no pilot implementation,
  materialization or provider calls. Live-capture acceptance, compatibility,
  account/ratecard/launch evidence and explicit scoped owner authorization are still required.
  T006 and economic acceptance remain open.
- **Live-capture implementation, 2026-11-05.** The approved Rust-owned capture
  boundary exists behind the opt-in `pilot-capture` feature and passes its
  actual-host no-provider qualification (14 real-host and 58 unit tests); an
  in-session review's 26 surviving findings are fixed. The owner removed the
  independent-review gate the same day, so missing gate 1 is complete. No provider
  call, credential, view materialization or coding trial occurred; live execution
  stays locked behind gates 2–5. See the
  [receipt](validation.md#live-capture-implementation-and-no-provider-qualification--2026-11-05).
- **Pilot ready for paid launch, 2026-11-06.** The views, manifests,
  contained checker and executor are built and frozen (gate 4), and a
  full-pipeline run without any provider sealed the probe and all eight slots.
  Only owner inputs remain: the credential file, account identity, rights
  approval and explicit spend authorization (probe, then coding). See the
  [receipt](validation.md#pilot-apparatus-and-frozen-launch-inputs--2026-11-06).
- **Bounded pilot v1 completed, 2026-11-06.** With the owner's credential,
  workspace identity, rights approval and two scoped authorizations, the paid
  probe sealed and all eight coding slots ran without a halt.
  - **Apparatus verdict:** a completed, interpretable pilot.
  - **Coding verdict:** all 8 slots passed the independent oracle (A 4/4,
    B 4/4, B−A 0 in every pair). Both arms made the same repair, so the task
    cannot show an effect of the view on correctness.
  - **Tokens and cost:** B used 13% fewer inclusive input tokens in total, but
    pair 1 reverses this, and slot order and cache confound cost. The
    calculated charge at the pinned ratecard was about USD 0.35, against a
    budget-rate spend of 36% of the USD 10 envelope.
  - **Classification:** two arm-A reads of `.` were blocked. The owner kept
    them as constraint violations (2026-11-06), so A stays 4/4. T006 remains
    unaccepted, and the claim ceiling holds. See the
    [receipt](validation.md#paid-transport-probe-and-bounded-coding-pilot--2026-11-06).
- **Owner decisions, 2026-11-06.** The owner settled all 31 pending decisions in
  one pass; the [record](validation.md#owner-decisions--2026-11-06) lists each with
  its result.
  - **Release path:** the signed-package checks (row 4) block release; 010's
    real-model acceptance and the Z.ai gateway leave the release; 005 T005 is
    accepted at its written scope using the `65fd799` gates receipt
    ([acceptance](validation.md#boundary-scope-acceptance-005-t005--2026-11-10)); 009 scale is
    accepted through the Mac Rust preparation and completed replay/change checks
    ([receipt](validation.md#semantic-scale-replay-and-changed-input-acceptance--2026-11-10)); the ad-hoc-signed native script run passed,
    accepting the selected device checks for 009 T008/T009 and 010 T002
    ([receipt](validation.md#native-macos-acceptance--2026-11-10)); it must repeat on
    the signed release candidate. 001 T011 is accepted by the native Mac watch run on pinned rust
    ([receipt](validation.md#native-mac-watcher-acceptance-001-t011--2026-11-10));
    001 T010 is accepted at its written lexical retrieval scope: behavioral selection
    now defaults on with `--experimental-behavior-context=false` opt-out. Supplied
    dense windows and semantic model admission retain their ordinary behavior
    ([receipt](validation.md#t010-singleton-qualification--2026-11-11)). The first
    failed default attempt and the test-only gate continuation remain recorded;
    no semantic-quality or real-agent benefit is claimed. The retained cleanup and
    bounded store-lock repair pass the local gates
    ([receipt](validation.md#derived-writer-contention-after-fork--2026-11-11)).
    The remaining release-path items are to do.
  - **T006:** the [unfunded pilot v2 draft](../specs/003-agent-retrieval-context/contracts/bounded-pilot-v2.md)
    now records the arms, capture requirements and v1 gap repairs. It is not frozen:
    concrete held-out tasks, power/sample analysis, arm readiness, launch pins and a
    costed schedule remain. The
    task-context tasks (003 T007–T009, 008 T003–T005) are to be measured in it. Nothing is spent until the
    owner authorizes a key, rights and a budget.
    Credential-after-claim refusal journaling passes its selected local no-provider
    checks on an owned copy of the exact frozen OMP artifact
    ([record](validation.md#pilot-credential-refusal-durability--2026-11-11)). This
    does not qualify the current installed host or accept pilot v2/T006; other
    apparatus gaps and freeze requirements remain.
    Workspace restoration failures halt at the selected local fixture boundary;
    healthy journal finalization retains unstarted rows, and a failed halt
    append prevents a later success receipt
    ([record](validation.md#pilot-workspace-restoration-and-halt-persistence--2026-11-11)).
    This does not qualify live admission, the current installed host or pilot
    v2/T006; the other apparatus and freeze requirements remain.
    Authority/run-path binding passes its selected local no-provider checks
    ([record](validation.md#pilot-authority-and-run-path-binding--2026-11-11)).
    New launch metadata binds the canonical capture root and chains the
    validated authority digest; historical v1 evidence remains immutable.
    Pilot v2/T006 and the remaining freeze requirements stay open.
    Checker completion and artifact binding pass the selected consumer checks and
    real checker run ([record](validation.md#pilot-checker-completion-and-artifact-binding--2026-11-11)).
    This does not accept the full private pipeline or pilot v2/T006; other gaps remain.
    Checker provenance and retained diagnostics pass the selected local checks
    ([record](validation.md#pilot-checker-provenance-and-retained-diagnostics--2026-11-11)).
    Owned-output permissions and truthful seal/report publication pass their
    selected local no-provider checks on `c399c1e`
    ([record](validation.md#pilot-private-output-permissions-and-terminal-publication--2026-11-11)).
    The receipt separates the originally unqualified stage from its completed
    gate-only continuation, and distinguishes current lexical stores, pinned-host
    fixtures and actual checker outputs. These permission checks do not qualify
    semantic-worker output modes, the original private full pipeline or current
    installed-host compatibility; pilot v2/T006 remain open.
    Startup-network observation remains open: matching-release source exposes
    partial diagnostics but no established complete early observation/barrier
    ([source finding](validation.md#pilot-startup-network-source-feasibility--2026-11-11)).
  - **Hosts and evidence:** the owner's Mac is the work host. The 2026-10-11
    sandbox and its archive are declared lost; F2, F6, F8, F10 and F11 retire.
    Five encrypted backups are built and checksum-verified in
    `~/VSC_DEV/backups/foundry-20261111-retry1`
    ([receipt](validation.md#encrypted-evidence-backups--2026-11-11)). The owner
    retains the recovery private key and handles off-machine copying and authenticated
    restoration; neither is claimed done. The pilot key file is deleted; the owner
    revokes the key.
  - **Dependencies:** rustls 0.23.46 merged to main as `d94b013` (done); zip alerts
    dismissed as not used and Dart 0.3 declined (done); Tantivy 0.27 is accepted
    on experimental after store compatibility, exact G1 neutrality and all eight
    local gates ([receipt](validation.md#tantivy-027-store-compatibility-and-g1-neutrality--2026-11-11)).
    The combined parser update is accepted on `68ed776`: complete corpus/fixture
    snapshots are identical, focused migration checks and all eight local gates
    pass, and the conditional G2 run is not triggered
    ([receipt](validation.md#parser-bundle-and-complete-unit-comparison--2026-11-11)).
    Existing indexes require explicit repair for search9/units7; old semantic
    state remains history until explicit preparation.
  - **Closed or kept:** rows 5 and 6 and the historical CI re-run retire; the
    Nemotron comparison closes (its files are kept); `overview-v1` stays; query-time
    parse cost is a named limitation.


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

Repository: `/Users/satishlomte/VSC_DEV/context-foundry` on the owner's Mac (the
2026-10-11 checkpoint used `/workspace/context-foundry` on the sandbox, now lost);
branch: `experimental/context-ecosystem`.
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
| Host tasks | Historical Qwen3-4B pair fails both unchanged external oracles. The managed-runtime public seeded repair passes in both arms; provider usage is unavailable. | Representative correct completions and complete provider accounting, now owned by 003 T006 and pilot v2. The private stronger-model protocol (F2) was retired on 2026-11-06, and F3 moved to T006. |
| Representative scale | Exact overview fitting covers 67,663 admitted Rust/Go sources; 60,735–65,959 distinct inputs / 14.65–15.86 million tokens. | Owner decision, 2026-11-06 ("Accept the Mac rust run"): scale is accepted through the complete hierarchy-v1 Rust preparation and the [completed Mac replay/change check](validation.md#semantic-scale-replay-and-changed-input-acceptance--2026-11-10). No complete 67,663-source model run exists; that corpus and Linux x86 scale are a named limitation. |
| Economics | Identical-source CLI delivery improves by 25% fewer calls and 6.48% fewer delivered tokens at 512 tokens. Provider/task benefit remains unproved. Failed attempts and unknown usage are retained. Selected history totals 71 requests, including the latest 13; 66 complete and five unknown. | Demonstrated completion benefit with bootstrap, acquisition, retrieval, maintenance and failed attempts included. Smaller failed prompts are not a benefit. |
| Evidence | Bounded source call postings, conservative Python aliases, router dependency proofs and revision-bound execution snapshots; eight real framework/execution checks pass. | 005 T005 is accepted at its written scope ([receipt](validation.md#boundary-scope-acceptance-005-t005--2026-11-10)); broader collectors, frameworks and cross-language resolution are named limitations, and realistic task outcomes belong to pilot v2. Unsupported/dynamic cases remain explicitly incomplete. |
| Mac/GPU | Earlier runner evidence includes CPU/Metal parity and native CPU pressure (runs 38207948408, 38209502529). The owner's physical Mac now passes the selected ad-hoc-signed native CPU, embedding, generation, calibration/replay and cleanup checks ([2026-11-10 receipt](validation.md#native-macos-acceptance--2026-11-10)); Spotlight was unchanged. | Repeat on the signed release candidate; signed installed-package acceptance (row 4) still blocks release. Global GPU pressure, combined residency, thermal/battery and eviction remain named limitations (owner, 2026-11-06, "Script checks only"). |

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

Owner decision, 2026-11-06 ("Declare both lost"): V11, the capsule and the portable
archive below were on the 2026-10-11 sandbox and are lost. This section records what
they held; it is not a restore procedure.

Set `V11=/workspace/.context-foundry-tools/validation-runs/realistic-delivery-v11`.
The private `V11/resume-capsule/` held handoff scripts, patches, hashes, source
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

The portable `/workspace/context-foundry-resume-20261011.tar.gz` preserved the v11
receipts/stores/binaries, handoff work, source corpora/manifests, selected historical
host receipts and an offline Git bundle. Its adjacent `.sha256` file verifies the
archive. Read `RESTORE.md` inside before extraction. Public model weights, general
build caches and installed OMP/Bun/Rust dependencies are excluded. Restore original
absolute corpus paths to preserve source/store identity; do not casually rewrite
profiles or claim cache equivalence across different worker descriptors/hardware.
The archive is private handoff material, not a Git/release artifact.

## Where to continue

Owner decisions, 2026-11-06 ("Mac only, drop sandbox"; "Declare both lost"): continue
on the owner's Mac (Apple M5 Max, 64 GiB) in `/Users/satishlomte/VSC_DEV/context-foundry`.
Private corpora, datasets and stores stay at their absolute paths under
`/Users/satishlomte/VSC_DEV` ([CLAUDE.md](../CLAUDE.md)). The 2026-10-11 sandbox and its
archive are lost; do not try to reach either. `scripts/cloud/sandbox.sh` and
`fetch-resume.sh` are retired.

If the Mac is unavailable, use a Claude Code cloud environment as a fallback. It has
no private evidence, so G1, G2 and pilot runs wait for the Mac. It is versioned in
`scripts/cloud`:

| File | Use |
| --- | --- |
| [`sandbox.sh`](../scripts/cloud/sandbox.sh) | Retired 2026-11-06 (the sandbox is lost); kept as the record. Do not run it. |
| [`setup.sh`](../scripts/cloud/setup.sh) | The environment's setup script. It installs Rust 1.90, libclang/CMake, the pinned llama.cpp object store, the pinned public models and 013 files, OMP/Bun, and the locked Cargo dependencies. |
| [`environment.env`](../scripts/cloud/environment.env) | The environment variables. They hold paths only; there are no secrets. |
| [`allowed-domains.txt`](../scripts/cloud/allowed-domains.txt) | Hosts added under Custom network access, alongside the default package managers. |
| [`doctor.sh`](../scripts/cloud/doctor.sh) | Checks a session: the environment variables, host limits, `avx512_bf16`, toolchains and pinned inputs. |
| [`gates.sh`](../scripts/cloud/gates.sh) | Runs the merge gates (`scripts/gates.sh`) with `environment.env` applied, an optional trusted `CF_GATE_ENV` file, and `RUSTUP_TOOLCHAIN` unset; `bash scripts/cloud/gates.sh OUTDIR`. |
| [`fetch-resume.sh`](../scripts/cloud/fetch-resume.sh) | Retired 2026-11-06 (the archive is lost); kept as the record of how a cloud session was meant to fetch it. Do not run it. |

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

An Anthropic-hosted VM has about 4 vCPUs, 16 GB RAM, 30 GB of disk and no private
evidence. Steps 3–5 below (F10, F11, F2) were retired on 2026-11-06, so none of them
needs a larger host. Revision-5 (`avx512_bf16`) jobs stay parked until the owner
provides a Linux host.

[`CLAUDE.md`](../CLAUDE.md) gives agents the session rules for both places.

## Resume in this order

This is the order as of the 2026-10-11 checkpoint; it is superseded. Take next steps
from [Current status](#current-status--2026-11-06). What became of each step:

- Step 1 now runs on the owner's Mac (M5 Max, 64 GiB), not the sandbox.
- Step 2 was measured as F9 on 2026-10-14; its capsule is lost with the archive.
- Steps 3 and 4 (F10, F11) were retired on 2026-11-06 ("Accept the Mac rust run").
- Step 5 (F2) was retired on 2026-11-06 ("Retire F2; F3 to T006").
- Step 6 is superseded by F4, which passed on 2026-10-22.
- Step 7's capsule patch was retired with the archive (F8).

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
