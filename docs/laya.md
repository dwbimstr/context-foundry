# Legacy Laya prototype boundary

Status: historical, revised 2026-10-05 for 013 T003. The owner selected
[Foundry-owned Rust learning](learning.md). This page is not the target architecture.

The prototype's `src/laya.rs` (`decide`/`predict`) and its `/v1/systemone` HTTP client
are **removed** (013 T003). They once contacted a separately managed loopback Laya
server for graph/search strategy selection, bounded to two seconds and 64 KiB with a
deterministic fallback; local HTTP fixtures (`tests/laya_protocol.rs`, also removed)
exercised that wire. No real Laya model, training round or custom checkpoint ran in
this project. Git history at `2b9a0b2` retains the code.

`--laya-port` was dropped from `foundry context` earlier; it is now refused by name
(`unsupported_mode`, exit 2) with migration guidance: select a trained candidate with
`foundry learning select` and start with `--policy-config CONFIG
--development-isolation` (see [learning](learning.md#serving-selection-and-rollback-013-t003)).
Foundry never rewrites a host configuration that still carries the flag; remove it there.
There is no dual provider router.

The legacy feedback CLI (`foundry feedback` with no subcommand) and `export-training`
are unchanged byte for byte; the `Feedback` row type and its store code now live in
`src/learning.rs`, and `Strategy` beside `strategy_for_query` in `src/response.rs`.
Legacy rows stay exportable and are never trainable: they lack the exact inputs and
rights the [013 contract](../specs/013-owned-learning/contracts/learning-loop.md) requires.

Earlier versions of the [legacy format examples](../specs/013-owned-learning/contracts/eval.public.jsonl)
were checked with Laya's evaluation parser; the current fixture text was checked only
for JSON shape. Neither is acceptance evidence for the policy-head data format.
The inspected local Laya revision was `4066d5d5fbf08b66c6757ddeedbd797bd7655bc0`.
It remains a read-only design reference. No source is copied/relicensed into Foundry.
