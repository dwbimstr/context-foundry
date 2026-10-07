# Validation

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
- **Tests.** `tests/install.rs` (5): package contents and manifest; install, run, upgrade, rollback and uninstall with user data kept; busy refusal; worker bundles with fake workers; byte-exact host-config removal; interrupted-command recovery at every recorded point; refusal cases.
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
