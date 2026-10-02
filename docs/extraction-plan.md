# Extract outcomes, not the predecessor architecture

The prior plan divided a broad feature inventory into fifteen implementation bundles.
That preserved too many assumptions about how the product had to be built. The owner
asked for a KISS redesign; rewritten prose and Rust syntax are insufficient.

## Keep, simplify, leave out

| Desired outcome | Keep | Simplify or remove |
| --- | --- | --- |
| Trustworthy context after edits/restart | Transactional source identity, stale checks, recoverable derived data | No custom WAL/checkpoint/flush-predicate framework; reassess the two-library store choice |
| Useful coding-agent integration | Cited bounded evidence and real consuming-client checks | Direct stdio MCP, without custom socket carrier or relay |
| Effective graph for large codebases | Real producer provenance, logical identities, bounded navigation | One producer and task together; no generic provider framework or separate graph-program gate |
| Durable project knowledge | Explicit remember/correct/forget with export | No transcript harvesting, reflection, temporal memory or automatic consolidation |
| Honest token economics | Adapter budgets plus the requested optional Rust forwarding/metering gateway | One verified protocol, no fleet governor or universal campaign; MCP delivery is not whole-request control |
| Neural context that stays useful | Progressive preparation, current coverage and reusable document vectors | Ordinary cache table, rebuildable search index and one bounded model worker; no custom ledger or repeated full warm-up |
| Owned continuous learning inspired by Laya | Opted-in grouped joint inputs, ModernBERT with a decision head, actual Rust head adaptation, checkpoint identity, isolated deployment and rollback | Freeze the encoder initially; no Nemotron-vector substitute, Laya runtime, HTTP server or automatic promotion. Encoder adaptation retains its separate acceptance |
| Open-source adoption | MIT, license hygiene, public fixtures and install/recovery instructions | Ordinary release checklist; source re-index first, no speculative migration platform |

The [workflow specs](../specs/README.md) are the active direction. The [71-entry
crosswalk](extraction-map.md) is an inventory of source lessons, not a coverage mandate.
Its original review mixed full spec reads with scoped sampling; it is not exhaustive
verification of every predecessor requirement or implementation path.

## Three useful passes, each with a different question

1. **Necessity:** What actual user failure is this intended to fix? If no current
   consumer needs it, park it. Preserve the outcome, not every former subsystem.
2. **Ownership and failure:** Read the adjacent code. Who commits the fact, what can
   become stale, and what happens after interruption? Prefer deleting a state owner
   or protocol over adding diagnostics to make its interactions understandable.
3. **Delivery:** Follow one real user task through the proposed design. Can it ship
   without the next feature or another open-ended experiment? Remove circular gates.

These are review lenses, not three mandatory review meetings. Apply them to selected
work and changed assumptions. Do not keep re-auditing the entire portfolio to postpone
implementation. The [subtraction record](review/subtraction.md) applies them to the
current plan and names its accepted limitations.

## First implementation

001 D001 retains the current redb/Tantivy pair after comparing the simpler
single-transaction alternative. Implement that bounded recovery contract, prove
preservation/recovery, then bound indexing and verify the rendered
CLI output. That is a usable release by itself. The agent adapter follows; the real
graph workflow and owned policy loop must earn their extra complexity through actual use.
003 owns explicit bootstrap and adapter budgets; the shared [deployment contract](deployment.md)
places isolation and package lifecycle checks with the feature making each claim.

Do not copy C++ source, physical stores, private specs, transcripts or measurements.
Reimplement permitted ideas with attribution where needed. Prakarana and Laya stay
read-only research references; neither is a deployed dependency. New source indexing
uses an independent store. Foundry owns its Rust trainer/worker; third-party ML and
platform libraries keep their own licenses. Operating a predecessor is separate work.
