# 010 — Source-bound derived repository knowledge

Status: Reactivated for experimental implementation, 2026-10-09. T001 and T002 are
selected contracts, not implemented capabilities. The owner's large-repository
architecture direction supersedes the earlier blanket deferral. No generation model
has been selected or run in this increment. 013 remains frozen off.

## Outcome and ownership

An agent should find behavior and understand a subsystem without repeatedly reading
large files merely because the user's vocabulary differs from the code's identifiers.
LLM-generated source briefs and area summaries complement exact source and graph
facts. They are distinct from caller-authored task state or durable project memory.
The [ecosystem plan](../../docs/context-ecosystem-plan.md#architecture-correction-useful-context-across-6000080000-files)
records the architecture, Prakarana pin and build order.

Existing source transactions remain authoritative. A generated assertion is always
labeled derived and attributed to its generator; citations prove which source was
used, not that the assertion is true. Unlinked claims and unsupported fields stay
unknown. Generation never runs inside source commits or foreground queries.

### T001 — Store, find and invalidate a structured source brief

First implement an explicit CLI/library import seam for a bounded generator result,
using synthetic outputs to exercise the lifecycle before choosing a real generator.
This is the same acceptance path the later worker uses; importing a fixture is not
LLM runtime evidence. Example target: find a capacity-eviction routine with opaque
identifiers from a behavioral question, then recover its implementation and tests.
Record the baseline result and exact expected evidence in the implementation fixture;
do not claim current G2 attribution from that constructed example.

- One current brief per source and generation recipe. Schema fields: purpose,
  behavior, provides, consumes, constraints, tests and unknowns. Assertions carry
  exact source handles and an explicit derived classification. Use bounded JSON,
  at most 16 KiB of text and 32 cited spans per brief; reject malformed/extra fields.
- Record workspace, source/dependency hashes, model/artifact identity, prompt/recipe
  digest and generation time. The same accepted input identity is a replay no-op;
  replacing it atomically retires the old searchable representation.
- Reconstruct/hash-check the cited source snapshot on input. Before acceptance,
  compare every dependency with current source state. Unsupported identifiers are
  refused or reported as unknown; byte presence is not semantic verification.
- Use a small source-bound record table inside the existing redb owner and the
  existing derived lexical index. A separate table is justified because automatic
  source invalidation must not delete explicit user-authored project memory. No
  standalone knowledge database, custom journal or generic orchestration framework.
  Implementation must include the ordinary explicit schema upgrade and recovery tests.
- Source edits/deletion make a brief ineligible immediately, before index cleanup or
  regeneration. Its old text cannot be returned as current evidence from lexical or
  dense lookup. Explicit forget removes retained brief text and schedules derived
  cleanup; exports/backups remain outside live-store deletion guarantees.
- Search through generated vocabulary maps to the cited source identity. Group
  multiple forms of one source so summaries do not crowd out independent evidence.
  Keep exact-name retrieval unchanged. Task context may deliver a brief as labeled
  orientation plus supporting source; it cannot substitute for required edit evidence
  solely because it is shorter. Count all delivered text in the same allowance.
- Verify conceptual discovery, stale dependencies, edit-during-generation, deletion,
  malformed claims, replay, restart, output bounds and exact-source recovery. Include
  a misleading-summary fixture so a lexical hit cannot certify answer completeness.

### T002 — Generate useful briefs and area knowledge within idle resources

After T001, select one accountable generation provider and a pinned recipe. First-
party orchestration stays Rust; local generation uses an isolated, bounded model
worker. A remote provider requires an explicitly configured endpoint and data scope.
Neither this spec nor retrieval admission sends source to an unspecified service.
Use existing maintained transports and the shared source acceptance seam.

- Source replacement only records durable pending work; it never waits for a model.
  Resume from missing/stale input identities and preserve completed results across
  crash/restart. Deduplicate in-flight source/recipe work; late results must recheck
  dependencies, never overwrite a newer source version.
- Prioritize requested areas, entry points and changed dependencies within explicit
  work/token/time limits. Generate useful behavioral briefs selectively; generating
  one description for every file is not a bootstrap prerequisite. Do not continually
  regenerate identical inputs or recursively summarize summaries without dependency
  identities and a bounded level count.
- Derive area overviews from current child briefs plus exact manifests/entry points
  and tests. Retire affected ancestors on dependency changes. Composing model-written
  content preserves the derived label. Keep original source recoverable at every level.
- Share resource admission with 009: foreground priority, CPU and device headroom,
  bounded queued bytes/tokens, and hysteresis for idle work. Account for model
  generation separately from embedding; increasing one lane's work must not starve
  source freshness or foreground evidence delivery.
- Test with a real configured provider at its actual boundary before claiming model
  integration. Compare verified coding tasks and complete generation/maintenance/task
  cost; the owner's sandbox build does not wait on a held-out promotion gate.

## Pinned reference and differences

Prakarana at `3143213d63c22a097c77fb229bfc910e08c6e01e`:
`src/internal/enrich_worker.cpp::process_one`,
`src/internal/enrich_structured.cpp::process_structured`,
`src/internal/spine_tree.cpp::{render_card,write_spine_tree}`;
`tests/contract/test_structured_docfact_{parse,lane,retire,delivery,feeder}.cpp` and
`test_door_wiring.cpp`. Source/tests inspected, not executed here. Its doc-fact lane
and structured tier motivate this scope; its sidecar queue, C ABI and custom HTTP
client are not dependencies. See the pinned links and small-fixture result limits
in the ecosystem plan. Existing L3/L4 research continues to inform task state and
recoverability; it does not replace this repository-knowledge work.
