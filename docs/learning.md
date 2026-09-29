# Owned continuous learning

Status: Proposed, 2026-09-29. [013](../specs/013-owned-learning/spec.md) owns the
design and acceptance. The implementation is still the original Rust prototype;
it does not yet train or launch isolated model workers.

Foundry owns its policy model, Rust trainer, feature/data contract, worker protocol,
calibration, checkpoint loading, deployment and rollback. Laya supplied a useful
reference for small typed decisions with abstention. It is not the product runtime,
an API to emulate or a repository contributors must install/edit.

The first model is deliberately small: a 2048→64→2 decision head over the already
available Nemotron query embedding. It chooses whether to add graph evidence. The
head is trained and repeatedly fine-tuned; the embedding encoder stays frozen.
Inference adds no second encoder/query embedding call, and can run in a CPU jail.
When compatible features/graph/checkpoint are unavailable, use the deterministic route.
This is a testable starting architecture, not a demonstrated quality result or full
replacement of every Laya capability.

The continuous loop is explicit and repeatable:

1. A user/trusted checker supplies labels and permission for exact examples. Ordinary
   context use, usage receipts, memory and outside path mentions create no training data.
2. Freeze grouped data and compatible query features; reuse unchanged approved features.
   A new query in an old group is new work; the group only controls split membership.
3. Train/calibrate/evaluate an owned Rust checkpoint in a worker with restricted access
   and declared limits. A malformed/partial result cannot become a selected model.
4. Compare with deterministic routing and a compatible incumbent. A rejected candidate
   is a valid completed learning round. Actual task/economic benefit is a separate claim.
5. Explicitly select an eligible artifact and restart the store-owning process. Keep
   the prior config/checkpoint for rollback, then repeat on new examples and bounded replay.

[The contract](../specs/013-owned-learning/contracts/learning-loop.md) defines fields,
limits, fitting, calibration, lineage and IPC. [Deployment](deployment.md) defines
jails, conditional libkrun use, packaging, startup, upgrade and removal. Burn is the
first Rust-library candidate to verify; no library has been installed or tensor engine
written. The exact library/MSRV and packaged platform isolation remain D001 inputs.

The model never authorizes repository enrollment, tool execution, provider spend or
larger budgets. [Adapter economics](../specs/003-agent-retrieval-context/contracts/adapter-economics.md)
owns those deterministic boundaries. Learning remains off until actual task benefit
justifies it; source/agent releases can ship earlier. Model preparation and training
costs are reported separately and amortized only over observed use.

The existing `src/laya.rs`/`--laya-port` path is a legacy prototype interface scheduled
for removal in 013 T003. [Legacy notes](laya.md) describe what currently exists; they
do not override the owned design. No Laya checkpoint conversion or dual-provider router
is planned. Preserve old feedback without silently granting new permissions.
