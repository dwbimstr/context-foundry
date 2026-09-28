# Release scope

The first handoff is an architecture, a working Rust core, and a Laya integration
boundary. It is not a claim of complete feature parity or a stable public release.

| Cut | User-visible outcome | Completion evidence |
| --- | --- | --- |
| Current first slice | Index a UTF-8 workspace snapshot; search, import relationships and obtain cited bounded context; export opted-in routing feedback | CLI edit/delete/reopen test; source/index recovery; graph isolation and staleness; token accounting; local Laya protocol tests |
| Agent preview | A real agent can use one long-lived owner over stdio MCP; inspect freshness and retrieve more evidence without reopening the store per call | One end-to-end agent task on a public fixture; protocol bounds; restart and cancellation; clear index freshness |
| Large-repository graph | Paged reconciliation, per-source graph updates and one real semantic producer with stable symbol identity and source-bound resolution | Correct callers/dependencies on a public codebase, bounded import/query memory and update time; errors and unsupported languages named |
| Memory and conceptual retrieval | Explicit scoped remember/forget, separate user knowledge ownership, and optional dense retrieval where lexical/graph coverage is insufficient | Persistence/deletion tests; scoped retrieval; quality comparison on the missed questions |
| Learned routing | Repeated Laya training rounds can improve a bounded retrieval decision and safely return to the prior candidate | Task-grouped datasets; a real training round; held-out calibration/evaluation; actual candidate load and rollback |

The cuts preserve the full product direction. They keep unfinished optional work
from silently blocking a smaller usable release. No date or unmeasured scale
promise is attached to them.

## What gates a release

For the declared surface: installation and a real CLI/agent interaction must work;
update/delete/reopen and freshness must be correct; resource and output bounds
must be honest; local data must stay outside the source distribution. Run formatting,
lint and the relevant tests. The published artifact must match the checked source.
The full Rust suite belongs to a release candidate, not every documentation edit.

A benchmark is required before claiming faster queries, better task success or
lower cost. Name the decision and time budget, reuse unchanged evidence, and stop
when that decision has an answer. A failed quality/cost experiment can lead to
deferring the optimization; it does not invalidate a separately working lexical
CLI. Conversely, a favorable benchmark cannot excuse lost data or a broken close.

## One economic objective

Reduce the complete cost of correctly completing a coding task. Observe correctness,
tool calls, complete model input/output, cache-read/write tokens where supplied,
latency and local compute. Keep context-text token counts as a useful local budget
measure. Do not equate fewer indexed bytes, fewer tool calls or a smaller context
with lower dollars or better answers without the corresponding evidence.

No provider proxy, billing engine or token-price catalog is required in this core.
The consuming agent can provide usage receipts. Missing provider usage remains
unknown, not zero.
