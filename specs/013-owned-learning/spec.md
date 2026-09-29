# 013 — Owned, isolated continuous learning

Status: Proposed, revised 2026-09-29. The owner replaced an external Laya integration
with a Foundry-owned system. Laya is a research reference, not a runtime, API, checkpoint
or repository dependency. All first-party implementation is Rust. No new code, model
run, sandbox installation or deployment is implied by this design.

## Outcome and boundaries

From explicitly approved feedback, prepare reproducible features, train a small
decision model, evaluate it, select or reject it, deploy it in an isolated worker,
and repeat with new examples. It improves a bounded choice in the context workflow;
it cannot authorize new roots, execute tools, spend money or expand a token budget.
The first choice remains `search` versus `graph` expansion. Deterministic budgeting
and actual adapter usage accounting belong to 003, not learned confidence.

The proposed first model is a CPU decision head over the query embedding already
needed by 009. It avoids another encoder, tokenizer, foundation-model download and
GPU resident model. This is an owned model trained and subsequently fine-tuned across
rounds; the Nemotron encoder stays frozen. It does not claim full Laya feature parity,
encoder fine-tuning or a demonstrated quality advantage. A failed learning hypothesis
leaves deterministic routing enabled and does not block source/agent releases.

Dependencies: 001 for feedback ownership; 003 for agent feedback/economics; 009's pinned
query feature function for actual feature preparation; 005 before claiming useful graph
selection. Data admission can be implemented before those optional integrations run.

## Required behavior

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

**Replace:** external Laya commands, `Agent(local_path)`, Laya tokenizer/512-token
sequence, GRPO notebook recipe and HTTP System One schema. Porting those mechanisms
would retain an extra encoder/runtime and installation/deployment ownership.
**Retain:** typed choices, calibrated abstention, grouped feedback, actual gradient
updates, immutable artifacts, repeated rounds and measured task-level selection.

Choose one maintained Rust training library; Burn with its CPU backend/autodiff is
the first candidate to verify, not a tested or locked dependency. D001 records the
exact compatible library/version/features/MSRV, tensor serialization, numerical parity,
and one working packaged isolation profile. Preserve the current Rust 1.90 floor
unless an explicit repository decision changes it. Do not build a tensor/autodiff
engine or offer multiple training backends. Official [Burn documentation](https://docs.rs/burn/latest/burn/)
describes training/autodiff and a Rust CPU backend; compatibility and deployment still
require a small real train→save→load→predict check. No long model comparison is needed.

One crate may produce `foundry` and a feature-gated `foundry-worker` binary. The latter
is justified by resource/failure/access isolation, not as another daemon. Keep ML and
isolation dependencies out of the default core build. Ordinary modules own policy,
learning and worker supervision; no general job/plugin framework. The worker's platform
contract and deployment gates are in [deployment](../../docs/deployment.md).

Current `src/laya.rs`, `--laya-port` and `tests/laya_protocol.rs` are legacy prototype
surfaces. Their actual evidence remains historical. T003 removes that inference path
when the owned one works; do not retain two providers behind a compatibility router.
V1 feedback stays stored/exportable but cannot gain missing rights/group fields.
Legacy Laya datasets/checkpoints fail `recipe_incompatible`; no checkpoint converter.

## Tasks and acceptance

### T001 — Prepare approved examples and reusable features

- **Depends:** 001; 003 only for MCP feedback; 009 for real query embeddings.
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

SC-001..SC-004 remain unexecuted. This amendment selects owned Rust learning and its
small initial responsibility; D001 still requires actual library/platform integration
evidence before dependent implementation claims can be made.
