# Legacy Laya prototype boundary

Status: historical/current-prototype notes, revised 2026-09-29. The owner selected
[Foundry-owned Rust learning](learning.md). This page is not the target architecture.

The existing `src/laya.rs` and `--laya-port` flag can contact a separately managed
loopback Laya HTTP server for graph/search strategy selection. The prototype bounds
that request to two seconds and 64 KiB and falls back to its deterministic rule.
Local HTTP fixtures exercised this wire boundary; no real Laya model, training round
or custom checkpoint was executed in this project.

The existing feedback CLI accepts caller-provided task/query labels and explicit
permission. V1 export groups by task; it is not a complete training/consent/lineage
system. Export does not imply that a trainer exists or that learning helps a task.

The new [013 contract](../specs/013-owned-learning/contracts/learning-loop.md) replaces
the previously planned external Laya trainer, `Agent(local_path)` serving adapter,
512-token recipe and HTTP protocol. It keeps grouped consented examples, repeated
fine-tuning, calibration, identity and rollback, implemented as owned Rust components.
T003 removes the legacy inference path when its replacement is accepted; the source
has not yet changed. There is no Laya runtime/install/repository prerequisite.

Earlier versions of the [legacy format examples](../specs/013-owned-learning/contracts/eval.public.jsonl)
were checked with Laya's evaluation parser; the current fixture text was checked only
for JSON shape. Neither is acceptance evidence for the new policy-head data format.
The inspected local Laya revision was `4066d5d5fbf08b66c6757ddeedbd797bd7655bc0`.
It remains a read-only design reference. No source is copied/relicensed into Foundry.

See [implemented validation](validation.md) for the actual prototype evidence and
[deployment](deployment.md) for the proposed owned worker lifecycle.
