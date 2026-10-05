# 013 — Owned, isolated continuous learning

Status: Proposed implementation contract v4, revised 2026-09-29 after bounded Rust
feasibility probes. Production learning remains unimplemented. The owner selected
ModernBERT with a decision head and all first-party code in Rust. Laya is a reference,
not a deployed service. [Evidence and remaining blockers](../../docs/review/feasibility.md)
are scoped by probe; this document does not declare a model package ready to release.
Spec-pass decisions recorded 2026-10-03 in [contract v4](contracts/learning-loop.md):
the core-composed `state` (query, graph coverage and top-3 lexical locator lines)
within the 1024-token total, float16 checkpoint tensors upcast to float32 at load, a
per-request timeout fallback with the policy disabled after three consecutive
prediction timeouts, refusal of inherited per-option temperatures or a fitted
temperature below 0.5, and enablement only after a usage-import comparison. External
prerequisites (owner): approved labeled rows, the LibTorch package,
signing/notarization and an aggregate residency run.

Owner answers, 2026-10-04:
- **LibTorch.** Download authorized and done (2.11.0 CPU, macOS arm64). tch 0.24.0 and
  `tokenizers` 0.23.2 build and run on Rust 1.90; evidence in
  [prerequisites](../../docs/review/prerequisites-2026-10-04.md).
- **Rows.** Labels come from mechanically checked coding tasks (`task_checker`).
  Approved 2026-10-04 as proposed:
  - Corpus: rust-lang/rust 1.99.0 (`b940084d`) with the 005 T003 `library/` SCIP
    artifact. The rights assertion is the upstream MIT OR Apache-2.0 license.
  - Tasks: about 900, over about 400 groups.
    - Symbols are single-definition globals under `library/{core,alloc,std}/src`,
      chosen in SHA-256(symbol) order, excluding T003's 20 question symbols.
    - Eight phrasings mix definition and usage intents.
    - `task_group_id` is the defining module path.
  - Checker: `context` runs at 2048 tokens with `--strategy search` and with
    `--strategy graph`.
    - Required evidence is the definition's delivery unit, or min(3, n)
      reference-site units.
    - Label: the strategy that delivers all of it. If both do, the one with fewer
      delivered tokens (a tie goes to `search`). If neither does, no row is written.
  - Rows become trainable only through the trusted operator CLI. Labeling runs after
    005 T003's graph context lands.
- **Decision checkpoint.** Download authorized and done 2026-10-04:
  - Source: `convaiinnovations/laya-typed-decisions@1a793eb5`, Hub card license
    apache-2.0, 7 files, 846,203,578 bytes.
  - Location: `~/VSC_DEV/models/laya-typed-decisions-1a793eb5`.
  - Verification: every file matches its LFS SHA-256 or git blob ID. `model.safetensors`
    has SHA-256 `4fa56de7…a24e`.
  - Fixture: its `rl_agent_config.json` carries the inherited
    `temperature_by_options` (`choice:11+` = 0.1006) that T002 must refuse.
- **Signing/notarization.** Decide later.

## Outcome and requirements

Repeatedly prepare permitted feedback, adapt the model, evaluate, select or reject,
serve in isolation and roll back. First decision: `search` versus `graph` on joint
state/question/ordered options. Neural inference cannot grant roots, tools, spending
or larger token budgets. Nemotron remains the independent retrieval model.

- **FR-001:** Only externally labeled, explicitly approved exact inputs may train.
  Preserve correction, withdrawal, group splits and contributing lineage between rounds.
- **FR-002:** Training and serving use the same ModernBERT/tokenizer/choice-head mapping,
  rendering, label identity and calibrated probabilities. Reject oversize/incompatible
  inputs; no vector-only substitute, hidden truncation or random replacement head.
- **FR-003:** A Rust worker trains within a demonstrated filesystem/network/process and
  resource profile. No store/home/credential access. Incomplete output cannot select itself.
- **FR-004:** Compare against deterministic routing and compatible incumbent on held-out
  groups. Label accuracy differs from task benefit and complete token/provider cost.
- **FR-005:** Serve one selected immutable checkpoint through bounded private IPC;
  explicit config/restart selects or rolls back. Failure uses named deterministic fallback.
- **FR-006:** Repeat with new permitted inputs and bounded replay from a valid base;
  withdrawal/correction cannot be hidden by `no_new_data`. No autonomous scheduler.

[Contract v4](contracts/learning-loop.md) owns exact schemas, rendering, tensors,
training, bounds and errors. It replaces v3 entirely; history retains superseded details.
Dependencies: 001 for source/schema ownership; 003 for MCP feedback, bounded serving
and the usage import that gates enablement; 005 for graph usefulness. Neither
tokenized learning inputs nor policy inference depend on 009 query vectors. Explicit
strategies and unavailable graph bypass policy.

## Decision ecosystem and review traceability

The required ecosystem includes typed joint inputs, pretrained decision behavior,
answer probabilities/abstention, externally checked feedback, repeatable adaptation,
calibration/evaluation, immutable artifacts, explicit selection and rollback. Passing
one tensor-head fixture does not complete this workflow. Search/graph is its first
bounded consumer; it does not redefine all typed decisions as a fixed binary classifier.
Variable-choice, boolean (`noul`) and ordinal-score decisions remain extension targets.
Before enabling another family, name its actual Foundry consumer, exact input/output
semantics, external gold labels, calibration identity and focused acceptance here.
Use the same data/model/worker lifecycle; no extra service or generic model framework.

The [source-to-contract map](../../docs/references/laya-decision-ecosystem.md) links
immutable upstream modules, functions, regression tests and notebook cells to the
following tasks and existing/proposed Foundry modules. L04/L08 explicitly distinguish
our first fitting recipe from Laya's encoder-plus-head/noisy-logit objective; L05
defines the answer-probability semantics to preserve; L09–L12 state deliberate limits.
Review a touched behavior against its source row, contract and actual Rust acceptance.
When production code lands, attach its real symbol/test to that row; do not present
planned paths, upstream tests or scratch probes as implemented product behavior.

## D001 — Concrete path and remaining feasibility

Select `tch` 0.24.0 / LibTorch 2.11.0, CPU float32 and the pinned pretrained
ModernBERT-large/typed-choice checkpoint in v4. First implementation freezes encoder
and adapts head/type/scorer. Preserve encoder adaptation as a later supported capability
only after complete gradient/update and resource acceptance; do not label head fitting
encoder fine-tuning. No Burn/Candle/backend-selection framework or custom autodiff.

The sandboxed scratch fixture proved one full Rust encoder/head forward, head gradients,
a real update and save/load. It also probes first global/local QKV gradients. Those are
integration evidence, not complete model, quality, optimizer or deployment acceptance.
Rust code still needs mask/permutation/train-eval/tokenizer fixtures and exact package
assembly. The public reference supplied test outputs only; no Laya module enters runtime.

D001 remains **partially open for packaging and full recipe acceptance**. App Sandbox
allowed child execution, so the current macOS profile does not meet zero-descendant
requirements alone. Adding a hard pre-exec process limit denied tested spawn/fork
while the actual models still ran; full installed-profile acceptance remains open.
Package lead must demonstrate the required profile or leave the feature
unavailable. Do not silently relax rights, label a subprocess a jail, or assume libkrun
runs MLX. Preserve Rust 1.90 unless a documented decision changes the repository floor;
only the MCP probe has demonstrated that floor so far.

No model preparation on the full corpus until its small fixture and target profile pass.
One crate may build core plus feature-gated worker; ML dependencies stay out of default
core. No new numbered spec, job platform or model registry.

## Tasks and acceptance

SC-00N is the acceptance of T00N: SC-001 is T001's outcome, SC-002 T002's, SC-003
T003's and SC-004 T004's, each passing only with that task's verification.

### T001 — Prepare exact permitted ModernBERT examples

- **Depends:** 001 schema/feedback ownership and v4 input contract. **Scope:**
  `src/learning.rs`, operator feedback/prepare/check commands, `tests/learning_data.rs`;
  shared rendering/tokenization in proposed `src/decision_model.rs` (no weight load).
  Review references: L01/L03/L08 and the module map linked above.
- **Outcome (FR-001/FR-002, SC-001):** immutable schema-4 grouped/tokenized dataset;
  same renderer at train and predict; no model load merely to freeze/tokenize data.
- **Verification:** exact IDs/markers against pinned reference; 1024 and 1025 tokens;
  a state over 16 KiB refused before tokenization and a state under 16 KiB refused at
  the 1024-token total; state composition with zero to three locator lines and both
  coverage values; mask-token literal, Unicode, reversed options, state change with same
  query, wrong label/duplicate option; all byte limits and unknown/null/duplicate keys.
  Corrections, rights denial/withdrawal, historical split conflicts, ancestor missing,
  invalid base before no-op, new input in old group, interruption and output
  ownership/read-back. No outside path traversal, transcript/memory admission or 009
  feature call.
- **Review/cutover:** preserve/export legacy feedback, do not invent missing state or consent.
  One schema integrator reviews durable changes; no separate learning database.

### T002 — Train and evaluate the actual model in one accepted profile

- **Depends:** T001; D001 model mapping and demonstrated package rights/resources.
  **Scope:** feature-gated Rust worker, ordinary model/supervisor modules and
  `tests/learning_worker.rs`; no Laya service, training notebook or first-party Python.
  `src/decision_model.rs` owns numerical model/parameter operations; `src/learning.rs`
  owns fitting/calibration/evaluation/artifact workflow. Review references: L02–L08/L12.
- **Outcome (FR-002/FR-003/FR-004, SC-002):** real AdamW head adaptation, separate
  calibration/evaluation, immutable readable candidate and complete contribution lineage.
- **Verification:** float32 reference logits and gradients (atol 1e-5, rtol 1e-4),
  full/local attention boundaries, padding if introduced, option permutation/marker
  masking, deterministic eval and seeded train dropout. Float16 checkpoint tensors load
  as float32, and changing the source dtype changes `model_function_sha256`. Hash frozen
  encoder before/after;
  all intended trainable groups receive gradients/change. Compare training-eval and
  loaded logits. Test 1/limit/limit+1, nonfinite loss, malformed/missing tensors,
  calibration failure, timeout, crash, disk full and owner death; no eligible partial.
  Compare calibrated probability vectors before/after export, including conflicting
  upstream bucket temperatures; no inherited override of the fitted scalar. Refuse a
  candidate carrying an inherited `temperature_by_options` and one whose fitted scalar
  is below 0.5, using Laya's `choice:11+` temperature 0.1006 (issue #394) as the
  fixture. Test per-case/group/error denominators and 15-bin ECE against known inputs;
  diagnostics do not create a new benchmark campaign.
  Deny outside/symlink reads/writes, network, inherited secrets/FDs and child execution.
- **Review/cutover:** a rejected candidate is valid lifecycle evidence, not quality success.
  Head mode cannot certify encoder adaptation; selected package limitations are explicit.

### T003 — Serve, select, roll back and retire the old HTTP path

- **Depends:** T002 and 003; current 005 graph. **Scope:** `src/policy.rs`, worker IPC,
  config/status and real MCP tests. Remove `src/laya.rs`/`--laya-port` after owned path
  acceptance; preserve feedback/export. No dual provider router.
  Review references: L03/L05/L07/L11/L12; core grants remain outside model output.
- **Outcome (FR-004/FR-005, SC-003):** joint input predicts a stable option with exact
  candidate/input identity; config/restart selection and rollback preserve all user state.
- **Verification:** two candidate identities, malformed/oversized/late reply, wrong
  tokenizer/weights, invalid config, startup failure, timeout/death/EOF and busy slot.
  One prediction timeout falls back for that request only; later requests see busy
  while the timed-out work runs, and its late reply is never delivered. Only a valid
  reply within the ceiling resets the count (busy fallback neither counts nor resets);
  three consecutive prediction timeouts terminate the worker and disable the policy
  until restart. Malformed, wrong-identity and dead-worker cases stay terminal.
  Explicit strategy/missing graph bypasses policy. Missing Nemotron does not disable a
  usable policy; neither model extends 003 deadline. Combined aggregate resource test
  belongs here only when both features are enabled. Count extra encoder work honestly.
  Verify exact-threshold acceptance, below-threshold abstention before rounding,
  entropy/answer-probability distinction, option-probability consistency and stable
  tie handling. Malformed distribution/legacy ambiguous confidence cannot grant a route.
  A real adapter comparison on the same checked tasks, with provider usage read by
  003's `foundry usage import`, checks task correctness, latency and total provider
  tokens. The policy may be enabled only when correctness is equal and total provider
  tokens are lower; otherwise default policy stays off.
- **Review/cutover:** reject removed option with migration guidance, no host config rewrite.
  Rollback restores prior supported binary/config, not two simultaneous model paths.

### T004 — Repeat and deploy without growing an orchestration platform

- **Depends:** T002/T003, new permitted rows meeting grouped floors and the selected
  target package. **Scope:** same commands, second round, release/install/stop/rollback.
  Review references: L03/L07/L08/L11; Foundry owns consent and lineage beyond the notebook.
- **Outcome (FR-006 and FR-001–FR-005, SC-004):** second real round uses permitted new
  rows/replay; can be accepted/rejected without changing historical splits. Installed
  artifact survives restart, fallback and rollback; uninstall preserves user data.
- **Verification:** represented input→no_new_data, withdrawal/correction→base refusal,
  interruption retains incumbent, exact artifacts and lineage for both runs. Exercise
  actual distributed profile, process cleanup, install/upgrade/disable/uninstall and
  selected adapter. Fresh improvement needs fresh confirmation groups; regression data
  does not establish new uplift. Record resource and dependency identities.
- **Review/cutover:** unavailable isolation blocks this learning package alone. CLI, MCP,
  graph, memory and independently accepted semantic releases continue on their own gates.

SC-001–SC-004 remain unexecuted. Scratch feasibility is evidence toward D001, not
completion of these product tasks. Remaining checks have owners and bounded exits in
[the feasibility disposition](../../docs/review/feasibility.md); do not repeat a blanket
review or add a slow measurement requirement to every implementation phase.
