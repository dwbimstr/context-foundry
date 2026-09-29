# Predecessor spec crosswalk

All 71 discovered predecessor `spec.md` files are mapped below. Numbers 017, 028
and 033 are absent in that inventory; 069b/069c/070b are distinct entries. This is
an architectural disposition, not a promise of one-for-one feature parity or proof
that every old requirement was exhaustively reviewed. Original spec text, private
measurements and runtime data are not copied here. Only references and newly written
extraction decisions are included.

Destination numbers below preserve the original audit mapping. The [current portfolio](../specs/README.md)
has six active workflows; old destination IDs now resolve to explicit merged/deferred
dispositions. Follow those dispositions, not the old row, when planning work. This
crosswalk is not a feature backlog or a promise to build any listed mechanism.
The rows retain the original audit dispositions. The 2026-09-29 owner correction
supersedes their external-Laya and deferred-proxy assumptions: 013 now owns Rust
learning, and 003 includes the optional request gateway. Follow those current specs.

| Predecessor ID and capability | Destination | Extraction decision |
| --- | --- | --- |
| 001 — nugget-store-and-vector-core | [001](../specs/001-source-state-recovery/spec.md), [003](../specs/003-agent-retrieval-context/spec.md) | Rebuild durable source/knowledge and lexical retrieval with libraries. |
| 002 — setu-graph-and-hybrid-retrieval | [005](../specs/005-graph-evidence-lifecycle/spec.md), [009](../specs/009-optional-semantic-retrieval/spec.md) | Keep typed graph evidence; separate optional approximate retrieval. |
| 003 — context-delivery-and-compression | [003](../specs/003-agent-retrieval-context/spec.md) | Keep cited budgeted delivery; defer bespoke compression and hotload caches. |
| 004 — incremental-extraction | [004](../specs/004-workspace-freshness-scale/spec.md), [006](../specs/006-semantic-producer-adapters/spec.md) | Keep raw coverage independent of optional semantic extraction. |
| 005 — dwar-mcp-gateway | [002](../specs/002-single-owner-serving/spec.md), [003](../specs/003-agent-retrieval-context/spec.md) | Use a thin MCP adapter over shared application operations. |
| 006 — outcome-benchmark-suite | [011](../specs/011-outcome-token-economics/spec.md) | Keep real task evidence; do not treat proxy recall as task success. |
| 007 — cache-aware-governance | [011](../specs/011-outcome-token-economics/spec.md) | Do not recreate the cache governor; retain honest usage accounting. |
| 008 — cache-aware-routing-gateway | [011](../specs/011-outcome-token-economics/spec.md) | Use consuming-host receipts; defer a provider-routing proxy. |
| 009 — enrichment-worker | [010](../specs/010-source-bound-derived-knowledge/spec.md) | Optional source-bound derivation after a demonstrated concept-query need. |
| 010 — prakarana-app-server | [002](../specs/002-single-owner-serving/spec.md) | Keep one store owner; keep model work outside storage transactions. |
| 011 — knowledge-visibility | [003](../specs/003-agent-retrieval-context/spec.md), [015](../specs/015-open-source-release-lifecycle/spec.md) | Keep inspectable state and limits; defer a dashboard. |
| 012 — workspace-watcher | [004](../specs/004-workspace-freshness-scale/spec.md) | Reconcile missed events with bounded complete-scan semantics. |
| 013 — visibility-overhaul | [015](../specs/015-open-source-release-lifecycle/spec.md) | CLI/status first; broad dashboard remains deferred. |
| 014 — precise-extraction | [005](../specs/005-graph-evidence-lifecycle/spec.md), [006](../specs/006-semantic-producer-adapters/spec.md) | Preserve extraction precision as producer evidence, not built-in authority. |
| 015 — target-split | [015](../specs/015-open-source-release-lifecycle/spec.md) | One crate first; packaging follows real consumers. |
| 016 — correctness-fixes | [001](../specs/001-source-state-recovery/spec.md), [003](../specs/003-agent-retrieval-context/spec.md) | Carry focused correctness/accounting contracts into normal repairs. |
| 018 — okf-export | [014](../specs/014-migration-coexistence/spec.md) | Interchange only for a real migration/consumer; broad format deferred. |
| 019 — reranker-lane | [009](../specs/009-optional-semantic-retrieval/spec.md) | Reranking is optional after candidate coverage is understood. |
| 020 — text2mem-adapter | [008](../specs/008-scoped-durable-memory/spec.md) | Start with explicit scoped memory; defer a broad compatibility IR. |
| 021 — v9-control-plane | [001](../specs/001-source-state-recovery/spec.md), [008](../specs/008-scoped-durable-memory/spec.md) | Metadata shares transactional ownership; no independent persistence planes. |
| 022 — store-integrity-and-downgrade-safety | [001](../specs/001-source-state-recovery/spec.md), [015](../specs/015-open-source-release-lifecycle/spec.md) | Keep schema refusal/recovery; delegate physical database formats. |
| 023 — session-bridge-claude-code | [003](../specs/003-agent-retrieval-context/spec.md) | Exercise the actual host; do not assume hooks prove integration. |
| 024 — claude-md-context-spine | [003](../specs/003-agent-retrieval-context/spec.md) | Explicit retrieval first; automatic context-spine injection deferred. |
| 025 — mcp-retrieval-displacement | [003](../specs/003-agent-retrieval-context/spec.md), [011](../specs/011-outcome-token-economics/spec.md) | Compact evidence and complete payload accounting; no chatty graph ritual. |
| 026 — embedding-lane-v2 | [009](../specs/009-optional-semantic-retrieval/spec.md) | Optional version-bound dense index. |
| 027 — lora-distill-lane | [012](../specs/012-learning-data-contract/spec.md), [013](../specs/013-owned-learning/spec.md) | Narrow learning to Laya decisions; defer a multi-adapter research program. |
| 029 — memory-layers-bitemporal-episodic | [008](../specs/008-scoped-durable-memory/spec.md) | Keep explicit memory; defer bitemporal history and salience machinery. |
| 030 — consolidation-engine | [008](../specs/008-scoped-durable-memory/spec.md), [010](../specs/010-source-bound-derived-knowledge/spec.md) | Explicit memory/derivations first; autonomous consolidation deferred. |
| 031 — install-lifecycle-envelope | [015](../specs/015-open-source-release-lifecycle/spec.md) | Keep real install/upgrade/remove ownership and evidence. |
| 032 — neural-by-default | [009](../specs/009-optional-semantic-retrieval/spec.md) | Optional incremental models; no mandatory whole-corpus re-embedding. |
| 034 — session-memory-substrate | [008](../specs/008-scoped-durable-memory/spec.md), [012](../specs/012-learning-data-contract/spec.md), [014](../specs/014-migration-coexistence/spec.md) | Preserve memory/privacy/import goals; automatic transcript capture deferred. |
| 035 — consolidation-live-reflection | [010](../specs/010-source-bound-derived-knowledge/spec.md), [012](../specs/012-learning-data-contract/spec.md) | Operator/checker labels first; autonomous reflection remains deferred. |
| 036 — reflection-to-weights | [012](../specs/012-learning-data-contract/spec.md), [013](../specs/013-owned-learning/spec.md) | Keep train/serve separation, held-out evaluation and explicit promotion. |
| 037 — pas-loop | [002](../specs/002-single-owner-serving/spec.md), [013](../specs/013-owned-learning/spec.md) | Simple serving and external bounded timer; no generic orchestration layer. |
| 038 — config-control-plane | [002](../specs/002-single-owner-serving/spec.md), [015](../specs/015-open-source-release-lifecycle/spec.md) | One explicit configuration surface with preserved user settings. |
| 039 — llama-cpp-only | [013](../specs/013-owned-learning/spec.md) | Reuse optional external Laya; do not add several mandatory model runtimes. |
| 040 — ask-native | [003](../specs/003-agent-retrieval-context/spec.md) | Return evidence; a duplicate autonomous answer agent is deferred. |
| 041 — text-extraction-fidelity | [003](../specs/003-agent-retrieval-context/spec.md), [004](../specs/004-workspace-freshness-scale/spec.md) | Keep verbatim source identity, coordinates and encoding limits. |
| 042 — derived-work-ledger | [001](../specs/001-source-state-recovery/spec.md), [010](../specs/010-source-bound-derived-knowledge/spec.md) | Use transactional pending rows for concrete derived work. |
| 043 — drain-throughput | [001](../specs/001-source-state-recovery/spec.md), [004](../specs/004-workspace-freshness-scale/spec.md) | Avoid whole-store updates before tuning drain policy. |
| 044 — envelope-config-integrity | [001](../specs/001-source-state-recovery/spec.md), [007](../specs/007-multi-workspace-context/spec.md), [015](../specs/015-open-source-release-lifecycle/spec.md) | Keep root/config identity and exact owned configuration changes. |
| 045 — enrich-honesty | [004](../specs/004-workspace-freshness-scale/spec.md), [010](../specs/010-source-bound-derived-knowledge/spec.md) | Name partial, failed and unsupported work; no false complete coverage. |
| 046 — r3-retrieval-tally | [011](../specs/011-outcome-token-economics/spec.md) | Keep read-side outcome accounting separate from source truth. |
| 047 — delivery-envelope | [003](../specs/003-agent-retrieval-context/spec.md), [011](../specs/011-outcome-token-economics/spec.md) | Budget the full owned result and observe actual host/provider overhead. |
| 048 — upsert-relink | [005](../specs/005-graph-evidence-lifecycle/spec.md) | Logical graph identity must not depend on vector slots. |
| 049 — compact-reclaim | [001](../specs/001-source-state-recovery/spec.md), [004](../specs/004-workspace-freshness-scale/spec.md) | Delegate reclamation; retain operational bounds and explicit repair. |
| 050 — watcher-delete-floor | [004](../specs/004-workspace-freshness-scale/spec.md) | Only completed reconciliation proves source absence. |
| 051 — deser-bounds | [001](../specs/001-source-state-recovery/spec.md), [002](../specs/002-single-owner-serving/spec.md), [005](../specs/005-graph-evidence-lifecycle/spec.md) | Keep bounded decoding at every external/durable boundary. |
| 052 — utf8-producer-boundaries | [003](../specs/003-agent-retrieval-context/spec.md), [004](../specs/004-workspace-freshness-scale/spec.md), [006](../specs/006-semantic-producer-adapters/spec.md) | Keep UTF-8 and coordinate correctness through producer conversion. |
| 053 — secret-quoted-key | [008](../specs/008-scoped-durable-memory/spec.md), [012](../specs/012-learning-data-contract/spec.md) | Keep consent and scrub limitations; no secret-removal guarantee. |
| 054 — ingest-identity | [004](../specs/004-workspace-freshness-scale/spec.md), [005](../specs/005-graph-evidence-lifecycle/spec.md) | Do not revive withdrawn identity machinery; keep source ownership simple. |
| 055 — sanchalaka-operator | [003](../specs/003-agent-retrieval-context/spec.md), [013](../specs/013-owned-learning/spec.md) | Bounded evidence/strategy choice; full autonomous agent loop deferred. |
| 056 — code-retrieval-qrels | [003](../specs/003-agent-retrieval-context/spec.md), [006](../specs/006-semantic-producer-adapters/spec.md), [011](../specs/011-outcome-token-economics/spec.md) | Retain code-shaped relevance fixtures and actual outcome distinction. |
| 057 — composable-retrieval-plans | [003](../specs/003-agent-retrieval-context/spec.md) | A few explicit operations; generic retrieval-plan language deferred. |
| 058 — graph-publication-economics | [005](../specs/005-graph-evidence-lifecycle/spec.md), [006](../specs/006-semantic-producer-adapters/spec.md), [011](../specs/011-outcome-token-economics/spec.md) | Track real graph coverage and usefulness, not edge count as success. |
| 059 — rrf-tie-canon | [003](../specs/003-agent-retrieval-context/spec.md), [009](../specs/009-optional-semantic-retrieval/spec.md) | Stable ties and explicit ranking recipes; no unsupported determinism claim. |
| 060 — live-reclamation-lane | [001](../specs/001-source-state-recovery/spec.md), [004](../specs/004-workspace-freshness-scale/spec.md) | Do not port generation/reclamation architecture; keep recovery and bounds. |
| 061 — spine-tree | [003](../specs/003-agent-retrieval-context/spec.md) | Spine-tree caching remains deferred until a real consumer requires it. |
| 062 — structured-doc-facts | [010](../specs/010-source-bound-derived-knowledge/spec.md) | Optional source-bound structured facts; no mandatory whole-corpus lane. |
| 063 — service-boundary-edges | [005](../specs/005-graph-evidence-lifecycle/spec.md), [007](../specs/007-multi-workspace-context/spec.md) | Evidence-backed service/dependency relations; unsupported cases unresolved. |
| 064 — workspace-discovery | [007](../specs/007-multi-workspace-context/spec.md) | Explicit workspace registry, not silent discovery of private roots. |
| 065 — native-tool-mount | [002](../specs/002-single-owner-serving/spec.md), [003](../specs/003-agent-retrieval-context/spec.md) | Real host mount through a thin adapter. |
| 066 — code-shaped-retrieval | [003](../specs/003-agent-retrieval-context/spec.md), [006](../specs/006-semantic-producer-adapters/spec.md) | Exact paths/identifiers and semantic evidence are distinct contracts. |
| 067 — spec-authoring-adoption | [011](../specs/011-outcome-token-economics/spec.md), [015](../specs/015-open-source-release-lifecycle/spec.md) | Adoption experiments inform a claim, not every functional release. |
| 068 — duplicate-live-id-retirement | [001](../specs/001-source-state-recovery/spec.md), [005](../specs/005-graph-evidence-lifecycle/spec.md) | Transactional logical keys replace live-slot retirement machinery. |
| 069 — flush-write-amplification | [001](../specs/001-source-state-recovery/spec.md) | Library transactions replace application whole-checkpoint fallback tuning. |
| 069b — derived-record-wal-frame | [001](../specs/001-source-state-recovery/spec.md) | Do not port mutation-specific WAL frame taxonomy. |
| 069c — uncovered-mutation-frames | [001](../specs/001-source-state-recovery/spec.md), [011](../specs/011-outcome-token-economics/spec.md) | Preserve close correctness; cause counts alone do not identify roots. |
| 070 — degraded-read-only-serve | [001](../specs/001-source-state-recovery/spec.md), [015](../specs/015-open-source-release-lifecycle/spec.md) | Named corruption/refusal and explicit recovery; degraded repair is not silent. |
| 070b — auto-heal-duplicate-live-id | [001](../specs/001-source-state-recovery/spec.md) | Checked recovery before any auto-heal; slot-repair state machine rejected. |
| 071 — neural-section-order | [009](../specs/009-optional-semantic-retrieval/spec.md) | Delegate physical vector serialization/order to a maintained index library. |
