# 013 — Owned, isolated continuous learning

Status: Proposed, revised 2026-09-29. The owner replaced an external Laya integration
with a Foundry-owned system. Laya is a research reference, not a runtime, API, checkpoint
or repository dependency. All first-party implementation is Rust. No new code, model
run, sandbox installation or deployment is implied by this design.

## Outcome and boundaries

From explicitly approved feedback, prepare reproducible inputs, adapt ModernBERT with
a decision head, evaluate it, select or reject it, deploy it in an isolated worker,
and repeat with new examples. It improves a bounded choice in the context workflow;
it cannot authorize new roots, execute tools, spend money or expand a token budget.
The first choice remains `search` versus `graph` expansion. Deterministic budgeting
and actual adapter usage accounting belong to 003, not learned confidence.

The owner clarified **ModernBERT with a decision head**, not a fixed classifier on
Nemotron vectors. Preserve joint state/question/candidate encoding. D001 compares
head-only adaptation with head-plus-encoder adaptation within this target, then pins
one supported recipe. Nemotron remains the retrieval model. A failed learning
hypothesis leaves deterministic routing enabled and does not block source/agent releases.
The [ModernBERT review](../../docs/learning.md#modernbert-and-the-cost-of-matching-laya--2026-09-29)
records the source evidence and Rust integration work.

**Planning status:** the earlier vector-only v3 contract and T001–T004 details below
are superseded and retained for review, not implementation authority. D001 must replace
their input, model, cache, IPC, artifact, resource and acceptance clauses for the
clarified target. Passing those old tests would not deliver the requested capability.

Dependencies: 001 for feedback ownership; 003 for agent feedback/economics; 005 before
claiming useful graph selection. ModernBERT policy inputs do not depend on 009 query
vectors; 009 retains retrieval ownership. Data admission needs its own corrected schema.

## Prior v3 behavior — retained for contract replacement

Consent, grouped evaluation, isolated execution, explicit selection and repeatability
remain outcome requirements. Vector-specific fields and dependencies in this section
and the task section are superseded; D001 owns their replacement.

- **FR-001:** Only explicitly permitted, externally labeled examples enter fitting.
  Preserve task-group splits, corrections, withdrawal and base lineage as defined in
  the [owned learning contract](contracts/learning-loop.md). Memory/transcripts are
  not automatically training data; embedding consent is not training consent.
- **FR-002:** Train and serve exactly the same feature function, dimensions, head,
  label order and calibration. A different embedding function invalidates policy
  compatibility, even with the same dimension. Do not silently truncate query input.
- **FR-003:** One owned Rust worker trains under the deployment isolation contract,
  with explicit CPU/memory/output/time bounds. It receives only frozen permitted
  inputs and has no store, repository, credential or network access. Complete output
  requires verified read-back and a final manifest; partial output never selects itself.
- **FR-004:** Evaluate candidate, deterministic routing and compatible incumbent on
  the same held-out data; report rejection/inconclusive results honestly. Strategy-label
  accuracy is distinct from checked task benefit, total tokens and monetary cost.
- **FR-005:** The store owner supervises one isolated inference worker, validates
  every reply, and falls back on failure. Explicit config/restart selects an immutable
  checkpoint; rollback preserves all source/cache/user records. No public inference
  port, Python/Laya runtime, model fleet or automatic promotion is required.
- **FR-006:** A second real round consumes new permitted rows, including queries in
  existing training groups, with bounded replay from a valid base. Unknown platform
  isolation or unavailable features are named prerequisite failures, not passed tests.

## Architecture disposition and D001

**Replace:** external Laya service ownership and the vector-only v3 model proposal.
**Retain:** ModernBERT plus a decision head, joint candidate-conditioned inputs, typed
choices, calibrated abstention, grouped feedback, actual gradient updates, immutable
artifacts, repeated rounds and measured task-level selection. Laya's exact tokenizer,
head and checkpoint behavior are reference evidence, not mechanisms to discard merely
to avoid another encoder. No generic model fleet or full Laya feature clone is required.

**D001 pins the ModernBERT plus decision-head recipe and trainable parameters.**
Resolve head-only versus head-plus-encoder updates. The fixed query-vector head is
not a substitute or a selectable implementation of the clarified goal. Specify one
concrete task family and its exact state/question/option inputs before choosing a backend.
Pin permitted starting encoder/head weights, tokenizer, checkpoint mapping and maximum
sequence/option sizes; a new random head cannot claim pretrained decision behavior.
Retain deterministic routing
as the fallback; no capability claim from library names or parameter count.

Choose one maintained Rust training library. Burn's tensor/autodiff infrastructure
does not establish a ModernBERT fine-tuner. Candle's inspected
ModernBERT path supports forward computation but its RoPE uses a no-backward operation;
stock inference support does not prove encoder gradients. For actual encoder tuning,
investigate `tch`/LibTorch as the first alternative to a custom training port. That
retains Rust first-party code with a third-party native dependency; do not confuse it
with a ready-made Rust equivalent of Python's AutoModel or full Laya parity.
The review links pinned upstream evidence and the work each path leaves to Foundry.
D001 records the exact compatible library/version/features/MSRV, tensor serialization, numerical parity,
and one working packaged isolation profile. Preserve the current Rust 1.90 floor
unless an explicit repository decision changes it. Do not build a tensor/autodiff
engine or offer multiple training backends. Official [Burn documentation](https://docs.rs/burn/latest/burn/)
describes training/autodiff and a Rust CPU backend; compatibility and deployment still
require a small real train→save→load→predict check. No long model comparison is needed.

The check must prove the selected scope: frozen encoders keep identical weight hashes
while intended head weights receive gradients and change. Encoder fine-tuning instead
needs correct Q/K/V gradients through both local and global attention, positional
rotations and masks, plus intended parameter updates. Compare a deterministic float32
fixture against reference logits and gradients under declared tolerances (initial
proposal atol=1e-5, rtol=1e-4); loss reduction alone is insufficient. Include label
order, train/eval behavior and checkpoint read-back; typed inputs additionally cover
option permutation and masking. Pin the recipe
before running, use permitted local artifacts, at most 20 update steps and ten minutes
of execution for this feasibility check. Missing artifacts/reference or a timeout is
incomplete evidence, not a reason to install or run an unrestricted fallback.

Decision: if the selected capability and packaged profile pass, implement that one
path; if not, keep that learning capability unavailable and record the narrow missing
piece. Do not build a general backend framework or tune the entire corpus to decide.
The ModernBERT/typed-head contract must first replace the v3 input/cache/IPC/artifact
and resource clauses, including 009-independent feature identity and actual encoder
cost; it cannot inherit the small head's 100 ms/5-second/CPU-only assumptions. The
existing Laya checkpoint remains incompatible with v3; reference-model evaluation
does not silently grant a production import/conversion path.

One crate may produce `foundry` and a feature-gated `foundry-worker` binary. The latter
is justified by resource/failure/access isolation, not as another daemon. Keep ML and
isolation dependencies out of the default core build. Ordinary modules own policy,
learning and worker supervision; no general job/plugin framework. The worker's platform
contract and deployment gates are in [deployment](../../docs/deployment.md).

Current `src/laya.rs`, `--laya-port` and `tests/laya_protocol.rs` are legacy prototype
surfaces. Their actual evidence remains historical. T003 removes that inference path
when the owned one works; do not retain two providers behind a compatibility router.
V1 feedback stays stored/exportable but cannot gain missing rights/group fields.
Legacy Laya artifacts cannot be relabeled as the owned format. D001 must decide exact
checkpoint mapping for the chosen permitted model; no generic conversion framework.

## Superseded v3 tasks and acceptance — D001 must replace before implementation

### T001 — Prepare approved examples and reusable features

- **Depends:** 001; D001's explicit fixed-head scope before feature-specific code;
  003 only for MCP feedback; 009 for real query embeddings under the current v3 recipe.
- **Scope:** owned `src/learning.rs`, feedback/CLI and existing worker integration,
  `tests/learning_data.rs`. No source ingestion from session text or automatic fitting.
- **Outcome (FR-001, FR-002 / SC-001):** exact contract-v3 dataset/lineage and feature
  files, immutable publication, current permission checks and reproducible row ordering.
- **Verification:** correction/withdrawal, invalid rights, duplicate/group conflicts,
  held-out exclusion, interrupted export, exact bounds, unchanged feature reuse and
  changed embedding-function rejection. New queries in an existing training group are
  new work; invalid base refuses before `no_new_data`. An outside path remains literal
  text; memory and another store's feedback are absent. Actual permitted encoder output
  must be durably read back; fake vectors prove schema only. Fitting/inference receive
  byte-identical feature vectors with the same label order; no Laya tokenizer is used.
- **Review/cutover:** preserve legacy feedback without inventing approval. V2 Laya
  artifacts remain incompatible rather than being relabeled. Follow 001 schema ownership.

### T002 — Train and evaluate one isolated Rust checkpoint

- **Depends:** T001, D001 library/isolation proof, permitted inputs and explicit limits.
- **Scope:** feature-gated owned worker, supervisor/limits, training/calibration and
  focused `tests/learning_worker.rs`; no edits to Laya or third-party model code.
- **Outcome (FR-002, FR-003, FR-004 / SC-002):** one real gradient run changes policy
  weights, calibrates on the separate split and publishes a loadable immutable result.
- **Verification:** record actual steps/loss/weight digests; compare loaded-model logits
  with training evaluation; inject nonfinite data, calibration failure, output full,
  timeout/crash and malformed artifacts. Exercise deployment deny tests before training:
  no outside-file access, network, store mutation or surviving owned children. A process
  launch alone is not jail acceptance. No eligible manifest on any failed boundary.
- **Review/cutover:** keep input and prior selected artifacts; remove only owned partial
  outputs. A rejected candidate is a valid lifecycle result, never a quality pass. No
  exact optimizer resume or fallback to an unrestricted/foreign-language trainer.

### T003 — Serve, select, roll back and retire the legacy inference path

- **Depends:** T002, 003 worker owner and actual 009 features; 005 for graph claims.
- **Scope:** `src/policy.rs`, bounded private worker IPC, config/status, relevant CLI/MCP
  cases and removal of the prototype Laya HTTP inference path after parity of outcomes.
- **Outcome (FR-004, FR-005 / SC-003):** exact selected head serves typed decisions with
  identity/confidence; timeout, malformed reply, incompatible features or failed jail
  yields deterministic context. Explicit selection/restart and rollback work.
- **Verification:** two checkpoint identities; mismatch/oversize/late reply; worker
  death and owner EOF; no extra embedding or encoder call for routing; no worker call
  for explicit strategy or unavailable graph/features. Invalid config disables policy
  visibly. Real adapter task comparison includes context, provider usage when observable,
  preparation/training cost and end-to-end latency. No demonstrated benefit means off.
- **Review/cutover:** reject old `--laya-port` with migration guidance once removed;
  preserve feedback/export. No automatic host config rewrite or old checkpoint conversion.
  Restore the old supported binary/config backup for rollback, not both live paths.

### T004 — Repeat and deploy the selected learning scope

- **Depends:** T002/T003, another permitted dataset satisfying group floors and the
  [release/deployment contract](../../docs/deployment.md).
- **Scope:** the same commands and owned binaries, second real round, packaging/start/
  stop/upgrade/rollback checks on each advertised target. No scheduler or model registry.
- **Outcome (FR-006 and FR-001–FR-005 / SC-004):** a second round learns new examples
  with unchanged splits and permitted replay; selection can accept or reject. Install
  the actual built artifact, bootstrap a fixture, connect the real adapter and exercise
  selected policy/fallback after restart, then uninstall without deleting user data.
- **Verification:** exact dataset/base/recipe/candidate digests; already represented
  inputs→`no_new_data`; withdrawn contributor→base refusal; no in-place selection writes;
  interruption preserves incumbent. Record two real runs, package identities and deny/
  rollback outcomes. Reused evaluation groups cannot establish fresh improvement.
- **Review/rollback:** a platform without proven isolation ships baseline capabilities
  only. Optional workers cannot delay earlier release cuts. Local package preparation
  does not publish a repository, release or model.

SC-001..SC-004 remain unexecuted and describe the superseded vector-head proposal.
The accepted target is owned Rust learning with ModernBERT and a decision head; D001
must replace the affected contract/tasks and establish library/platform integration
before dependent implementation. This review does not claim a build-ready model plan.
