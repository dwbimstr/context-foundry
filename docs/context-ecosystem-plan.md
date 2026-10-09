# Context Foundry ecosystem plan

Planning direction, 2026-10-09. Start with retrieval, then extend the same evidence
and measurement discipline through task state, working context and durable memory.
The owner authorized execution on 2026-10-09 (“go for 1 then 2 and onwards”).
This sequence is not implementation or acceptance evidence.
The numbered specs own behavior and tasks; this document owns their cross-spec order.
The first three tasks are **active, in progress**. The owner's subsequent instruction
authorizes experimental sandbox implementation of later milestones without waiting
for measurements or gates. Their concrete contracts are refined in the owning specs.
Existing acceptance, signing and
release requirements remain in force; 013 remains frozen off.

The product objective is **verified task completion with faithful evidence and user
constraints, at an acceptable total cost and latency**. Foundry should find the
necessary evidence, keep the current task coherent, recover details when needed,
and make useful knowledge available in later work. Smaller prompts alone do not
establish success.

Execution update: [2026-10-09 validation](validation.md#retrieval-execution--2026-10-09-partial-milestone)
records implemented bounded traces and an opt-in mixed-intent packing candidate,
with unchanged defaults. The preserved G1/G2/Gemma apparatus and live host runs are
still needed before quality acceptance, but do not block experimental implementation. Host inspection
and the matched-run procedure are prepared in 003 T006. The sandbox implementation
now also exposes [task state, reversible views, reminders and explicit promotion](experimental-context.md).
The implemented continuation (008 T005 / 003 T008) connects the task view to source
retrieval under one allowance and adds content search over retained observations.
These are local capabilities, not measured host outcomes.

## Architecture correction: useful context across 60,000–80,000 files

Owner direction, 2026-10-09: task quality and practical bootstrap/maintenance are
primary. Bounded tool output and recoverable task memory are necessary but do not
solve repository understanding or total token economics. The next work returns to
these foundations. This direction explicitly reactivates the narrowed source-derived
knowledge work in [010](../specs/010-source-bound-derived-knowledge/spec.md).

`rg` remains an efficient exact-match primitive. The expensive failure is asking an
agent to discover an unfamiliar system through repeated broad searches and reads,
with no retained architecture or behavioral model. Foundry must reduce that discovery
work while supplying the evidence needed for a correct change. A smaller answer that
omits a caller, configuration dependency or test can make the complete task cost more.

### What exists and what is missing

| Capability | Current evidence | Required change |
| --- | --- | --- |
| Exact source/structural graph | Versioned source, lexical search, 23-language extraction, compiler imports, approximate doors | Preserve it as the evidence base; add evidence about behavior and process boundaries. |
| CPU parallelism | Source-index document construction already uses at most eight threads and a 64 MiB input bound (001 T009) | Semantic partitioning was serial under the writer and MCP engine slot. 009 T006 separates snapshot, parallel computation and conditional publication. |
| Embedding bootstrap | Latest recorded Gemma runs: 112,549 cards in 1,256 s; 332,732 cards in 4,437 s. MCP prefetches one batch; model work has one slot. | Make useful partial coverage available early, overlap independent stages, remove whole-corpus barriers, and tune device batches separately from CPU worker counts. |
| Repository knowledge | Address cards contain heads/docs; 010 was deferred. Explicit task/project memory is caller-authored. | Add source-bound LLM summaries and area knowledge, indexed for conceptual discovery and linked back to exact source. |
| Continuous maintenance | Explicit re-index and semantic resume exist; no watcher | Add event-driven changed-path reconciliation, bounded coalescing, overflow recovery and an idle resource policy. |
| Task-quality economics | Exact delivery accounting and older host trials exist; recent features have fixture evidence | Compare verified multi-file tasks and complete cost, including bootstrap amortization, maintenance, retries and auxiliary generation. |

The recorded preparation times are complete historical runs, not profiles of where
time was spent. The source inspection proves serialization, not the percentage of
the 4,437 seconds attributable to it. The old Nemotron 845-second/2,099-unit run is
not the current embedding baseline. Memory figures in the record name the process
measured and must not be treated as whole-host RAM.

### Repository knowledge and task evidence

Keep three complementary representations under existing owners:

1. **Exact evidence:** immutable source versions, definitions, references, manifests,
   tests and configuration. AST/tree-sitter/SCIP establish declared structural facts
   within their coverage; they cannot alone explain intent, business constraints or
   runtime wiring.
2. **Derived repository knowledge:** short behavioral glosses, structured source
   briefs and area summaries. Record purpose, entry points, inputs/outputs,
   dependencies, failure behavior, constraints, relevant tests and explicit unknowns.
   Each assertion cites source spans and carries model/prompt/input identity. Matching
   an identifier to bytes validates the reference, not the truth of the model's prose.
   An area summary inherits the dependency hashes and derived classification of its
   children. Start with source briefs; materialize higher levels only when useful.
3. **Task state and evidence selection:** use the retained task requirements and open
   questions to request implementation, callers, invariants and verification evidence.
   Return compact summaries for orientation and exact source where an edit depends
   on it. Recover details on demand; expose missing/conflicting evidence instead of
   implying that top-k relevance means the task is answerable.

Conceptual search should retrieve a source *through* its generated vocabulary without
allowing several summaries of that source to consume the whole candidate budget.
Group candidates by source/evidence identity; preserve exact-name behavior; budget
orientation, source bodies and tests according to the question. Embedding a longer
summary by itself does not solve candidate selection or complete-evidence packing.
Generation is off the query path and optional at startup. Lexical retrieval over
briefs can help before any new vectors are ready. No generation model or paid endpoint
is implicitly selected by this plan.

The graph needs both observed structure and explicitly derived relationships.
Routes, OpenAPI/protobuf contracts, configuration, queue topics and command tables
can establish joins that a compiler index does not contain. Runtime-assembled links
remain hypotheses unless supported. Never convert an LLM's suggested `consumes`
entry into a compiler-confirmed call edge. 005 owns this distinction and the joins;
010 owns the generated claims. Cross-repository joins still need explicit admission
and a separate scope amendment.

### Execution and memory design

Use a bounded pipeline: source change hints → verified snapshots → parallel CPU
parsing/card preparation → controlled device batches → conditional durable commits
→ searchable publication. Each stage hands off owned, bounded data; no write
transaction or serving slot spans tokenization or inference. The writer remains one
owner, and freshness is rechecked at acceptance. More CPU threads are useful only
for independent CPU work. Replicating a model process on one saturated GPU can make
throughput and memory worse; first fill appropriate batches and overlap CPU work.

Bootstrap becomes progressive: fast lexical/structural readiness; partial semantic
coverage as bounded waves finish; useful briefs for requested areas and entry points;
broader background coverage while idle. File count is not a workload model: also
track bytes, definitions/cards, input tokens and changed dependency fanout. Full
partitioning and full ANN publication currently remain barriers; T006 removes only
the partition computation lock/serialization. T007 removes those remaining barriers
without losing committed cache work or publishing stale vectors.

Bound retained source bytes, queued token IDs, completed-but-uncommitted outputs,
model/device residency and publication scratch separately. Avoid repeated body
copies, parse/tokenize passes and full-store materialization. Use owned buffers and
existing redb/USearch facilities before introducing a new allocator or custom memory
mapping. A byte bound on source inputs is not a claim about total process memory.
One reusable CPU pool, token-shaped device batching and segmented/delta publication
are candidates for subsequent slices; do not add a generic scheduler or a new store.

Watching supplies hints to the existing ingestion owner. Coalesce repeated events;
reconcile admitted paths with the same no-follow, ignore and deletion rules; handle
renames, event overflow and restart through bounded reconciliation. Missed events
must not become certified freshness. Quiet operation must avoid repeated full-body
reads. Changed sources invalidate affected vectors, briefs and area dependencies
before their replacements are ready. An idle policy should consider foreground work,
CPU pressure, memory/device headroom and battery where available, with explicit
operator limits and hysteresis. Unknown host signals remain unknown.

### Revised build order

| Order | Concrete increment | Owner/status |
| --- | --- | --- |
| A | Snapshot/compute/commit partition pipeline with bounded parallel CPU work and source-version refusal | 009 T006: implemented and tested in this sandbox increment; actual throughput gain unmeasured |
| B | First source-bound structured brief: strict import/generation seam, lexical discovery, exact citations, stale/forget behavior | 010 T001: explicit import, lexical discovery and lifecycle implemented/tested; generation remains T002 |
| C | Event-driven incremental source refresh and idle maintenance within the admitted owner | 001 T011: selected, not implemented |
| D | First useful semantic coverage before a full pass; amortized publication and device-batch tuning | 009 T007: selected, not implemented |
| E | Governed model generation, area rollups, boundary joins and task-oriented evidence composition | 010 T002, 005 T005, 003 T009: selected, not implemented |

B and C need not wait for real-model throughput or held-out quality measurements.
Contract tests and a small sandbox task fixture accompany each implementation.
Larger evaluations assess usefulness rather than blocking the owner's experimental
build authorization. Continue on `experimental/context-ecosystem` with local commits;
no default promotion, release, training unfreeze or predecessor deployment is implied.

Use tasks such as tracing a request through configuration and a worker, repairing a
behavior while preserving its callers, and a multi-service contract change with its
tests. Compare a native-tools baseline, current Foundry and the candidate with the
same host/model and task constraints. The primary outcome is a correct, verified
change without violated requirements; record wrong edits and incomplete evidence.
For monetary economics, divide all attributable bootstrap, maintenance, retrieval,
model and retry costs by verified completions over a stated reuse horizon. Report
latency, RAM and energy separately; add failed attempts to cost rather than dropping
them. These outcomes remain unmeasured for the new architecture.

### Prakarana reference and intentional differences

Read-only inspection pinned to
[`3143213d63c22a097c77fb229bfc910e08c6e01e`](https://github.com/dwbimstr/prakarana/tree/3143213d63c22a097c77fb229bfc910e08c6e01e).
No predecessor binaries, services or tests were executed. Its source and tests inform
contracts; historical results are not Foundry results.

- **010 vocabulary/briefs:** `src/internal/enrich_worker.cpp::process_one` and
  `src/internal/enrich_structured.cpp::process_structured`; contract tests
  `test_structured_docfact_{parse,lane,retire,delivery,feeder}.cpp` and
  `test_enrich_queue.cpp`. Keep source revalidation, derived provenance, strict
  bounded output, resumability and salience. Foundry uses existing transactional
  storage and maintained Rust integrations, not the predecessor's custom sidecar,
  C ABI, HTTP client or full queue framework. Its M9 recall@3 result of 25%→50% was
  only two added hits on a small opaque-function fixture; another tiny corpus lost
  recall through competition. Neither establishes coding-task improvement.
- **010 area knowledge:** `src/internal/spine_tree.cpp::{render_card,write_spine_tree}`,
  spec 061 and `tests/contract/test_door_wiring.cpp`. Retain dependency identity and
  the rule that composing derived text does not turn it into observed fact. The
  module's `provides/consumes` rollups explicitly say deterministic 063 joins are
  future work; spec 063 is a draft, not proof of an implemented cross-service loader.
- **001 continuous maintenance:** `src/internal/workspace.cpp`,
  `docs/design/watcher-efficiency.md`, `test_watch_scan_efficiency.cpp` and
  `test_watcher_delete_floor.cpp`. Preserve liveness and safe reconciliation. Avoid
  the documented failure where each scan retained too much transient work, refused
  the tail, and repeated expensive scans without useful publication.
- **009 resource control:** `src/internal/policy_gate.cpp::{gate_decide,embed_gate_step}`,
  `test_embed_governor.cpp` and `test_embed_governor_live.cpp`. Retain bounded idle
  work and hysteresis; map them to the actual CPU/device bottlenecks instead of
  copying constants or creating one resident model per CPU thread.
- **005 publication cost:** spec 058 and `src/internal/setu_engine.cpp` describe
  batching and work proportional to touched graph regions. Use that cost question
  for Foundry publication; retain redb/USearch rather than inheriting custom graph
  storage. The benefit in Foundry remains to be demonstrated.

## Starting evidence

The [2026-10-08 measurements](validation.md#measurements-on-the-development-host--2026-10-08)
are the starting point. They establish a useful retrieval core and substantial gaps.

| Observation | Planning consequence |
| --- | --- |
| Gemma G2 delivers the required definition on 139/388 questions (35.8%): Rust 45/197, the harness's `bun` set 94/191. That second corpus is oh-my-pi. There are 249 misses. | Account for losses through the retrieval stages before choosing a repair. Compare new work with current Gemma, not just the old 0/388 no-profile baseline. |
| G1 passes its historical guards, while paired checker first-call passes change 3,741 → 3,716 and `bun` 971 → 969. | Preserve the original acceptance record and also expose current-baseline wins and losses, including wording and ambiguity strata. |
| Address cards contain signatures/heads and leading documentation, normally within 128 model tokens; dense queries retrieve 64 candidates. | Test whether the representation expresses the behavior and whether the answer enters the candidate set. Neither limitation alone proves the cause of a miss. |
| An anchor disables dense retrieval, and anchored packing restricts delivery to the resolved definitions/directories and selected doors. | Separate identifying a symbol from finding evidence about its behavior. A question naming a type may need a method body or a caller. |
| Older host trials sometimes used more tokens/turns with Foundry. They predate the latest retrieval work and include small samples and prompt asymmetry. | Re-measure actual tasks with matched prompts and current binaries; do not present those trials as the current regression rate. |
| Gemma's development checks passed; installed semantic checks still need the signed macOS package. Nemotron was closed after invalid runs. | Keep enablement and packaging restrictions. Neither a new profile sweep nor Linux-only tests resolve the installed-artifact requirement. |

The mixed-query code path is a concrete limitation. How much it matters in real tasks,
and what causes the 249 anchorless misses, remain measurement questions.

## Milestones and dependencies

| Order | User-visible outcome | Existing owner and work | Exit evidence |
| --- | --- | --- | --- |
| 1. Retrieve the required evidence | Behavioral and mixed symbol/behavior questions return useful implementation evidence within the requested budget. | [009 T005](../specs/009-optional-semantic-retrieval/spec.md#t005--diagnose-and-improve-anchorless-evidence-retrieval) diagnoses and repairs anchorless retrieval; [001 T010](../specs/001-source-state-recovery/spec.md#t010--retrieve-behavioral-evidence-for-a-named-symbol) repairs mixed intent. | Stage-attributed losses, paired improvement over current Gemma on fresh held-out cases, preserved source/budget invariants and G1 guards. |
| 2. Improve actual coding tasks | Agents reach the right evidence and a verified patch with fewer unnecessary discovery steps. | [003 T006](../specs/003-agent-retrieval-context/spec.md#t006--validate-retrieval-repairs-in-real-coding-tasks) tests current versus repaired retrieval at the real host boundary. | Verified task outcomes, full-task usage and latency, visible failures, and an inspected host capability boundary for the next milestone. |
| 3. Preserve task state and observations | A restart retains the user constraint, rejected approach, unresolved question and supporting observation. | Experimental 003 T007 delivery and 008 T003 explicitly scoped retained records; 001 continues to own source identity. | Atomic updates, conflict handling, bounded opt-in capture, exact retained-observation recovery, and no inferred root admission or training permission. |
| 4. Edit and recover working context | The agent can fold a large observation, keep the task coherent, and recover exact details before acting. | Experimental 003 T007 / 008 T003 reversible views using source identities and milestone 3 records. | A real host applies an identified view revision; round-trip recovery, valid tool linkage, interrupted-commit/restart behavior and measured task economics pass. |
| 5. Reactivate memory when it helps | Relevant constraints and failed attempts influence the next action without constant reminders. | Experimental 008 T004 exact-failure reminders; broader triggers remain proposed. | Fewer repeated failures and missed obligations than passive memory on held-out tasks, with acceptable total cost and a measured unnecessary-intervention rate. |
| 6. Grow reliable durable knowledge | Useful decisions survive across tasks, corrections supersede stale beliefs, and forgetting reaches retained derivatives. | Experimental 008 T004 explicit decision promotion and tracked-copy forgetting. Reactivated 010 owns source-derived repository knowledge under the revised build order above. | Capture/update/retrieve/use evaluated separately; scope, contradiction, export and forget behavior verified through the same lifecycle suite. |

Milestone 1's diagnostic checkpoint unlocks its two repairs; they need not wait for
each other. Milestone 2 evaluates their combined benefit. Host capability inspection
and lifecycle fixture design can proceed during retrieval work, but implementing
task-state/context mutation waits for their concrete contracts. Moving on requires a
useful retrieval increment for the experimental branch; external measurement gates
do not block building and testing the later stages in the sandbox. Unexecuted quality
checks remain pending and never imply default promotion or release acceptance.

## First execution tranche

The three linked task sections are the authoritative backlog and acceptance detail.
Execute them in this order:

1. **Freeze and explain the baseline — 009 T005 diagnostic checkpoint.** Pin the
   current binary, Gemma profile, corpora, prepared generations, harness and budgets.
   Inspect all 388 results, including the 139 successes. For each miss record the
   first observed loss: absent/ineligible unit, candidate retrieval, ordering/caps,
   packing, or transport. Keep representation explanations as hypotheses until an
   ablation supports them. Join expected answers to diagnostic traces offline; labels
   never influence retrieval. Unknowns retain a named missing input. Do not infer
   host consumption from an MCP response.
2. **Repair mixed intent — 001 T010.** Keep symbol resolution and pure locator behavior
   deterministic. For a behavior question such as “How does `QueryRuntime` load the
   dense index on the first query?”, use the symbol as an identity constraint and
   seek the supporting implementation. Begin with bounded lexical/structural and
   current graph evidence. Amend the existing packing contract explicitly; do not
   merely turn dense retrieval on for every anchored query.
3. **Repair the largest demonstrated anchorless loss — 009 T005 candidate checkpoint.**
   Choose one mechanism from the attribution: richer source-derived cards, candidate
   coverage, deterministic ordering, or packing. Hold the selected embedding model
   fixed. Evaluate at most three predeclared variants in the first tranche; use the
   simplest variant that clears the gate. Change a representation recipe only with
   its version/cache invalidation and preparation costs accounted for.
4. **Verify the integrated result — 003 T006.** Compare current Foundry, each repair
   alone and both together on matched real tasks. Report first useful evidence,
   complete supporting evidence, exploratory calls, verified completion, total
   provider usage, latency and invalid runs. A retrieval score improvement that
   worsens task success does not promote the default.

The exit artifact is a concise decision record: loss counts, chosen repair and
rejected alternatives, changed contracts, paired results and regressions, runtime
limits, and the next unresolved bottleneck. Raw corpora, labels and transcripts stay
outside Git; public aggregate counts and reproducible apparatus identities belong
in [validation](validation.md).

Full G2 attribution needs the preserved private responses/questions and the prepared
Gemma apparatus described in the [measurement handoff](measurement-handoff.md).
The cloud workspace can implement trace plumbing, public mixed-intent fixtures and
contract tests. Real-model acceptance uses the existing Metal-capable measurement
host, with one job at a time and adequate memory; do not copy large private stores or
download another model as an implicit prerequisite. Missing inputs block the relevant
measurement, not unrelated fixture work. No new benchmark has run as part of this plan.

## Task state before context compression

Milestone 3 introduces only the records needed by the first lifecycle workflow:

- Requirements and constraints with attribution and scope.
- Observations with recoverable payload identity and source/tool revision.
- Hypotheses, decisions and rejected attempts, distinguished from observed facts.
- Open knowledge questions, remaining work and produced artifacts.

Use versioned atomic deltas and compare-and-swap updates. Record explicit supersession
or unresolved conflict; a later model assertion does not silently replace evidence.
Separate session work from durable project knowledge. A model's “done” is a claim;
task completion still needs the task's external oracle.

008 T003/T004 now authorize explicit caller-written task state and selected decision
promotion; automatic transcript harvesting remains excluded. Capture is scoped, opt-in
and bounded by retention and size; retrieval consent never implies training consent. Persist an observation before
removing its only working copy. Exact recovery is promised only while the payload is
retained; deletion and expiry must invalidate references honestly. Reuse the existing
transactional owner and storage unless the selected workflow demonstrates a concrete
failure that requires another mechanism.

## Working-view and host contract

Milestone 4 starts with selection, pinning, a reversible fold and bounded peek/expand.
Represent a proposed edit against a specific view revision; validate it, then commit
atomically. The runtime owns IDs, admission, budgets, protected instructions and tool
call/result linkage. The model can propose what to retain, not rewrite source truth
or grant itself authority. Restore rechecks source freshness and never implies that
external tool side effects were undone.

Test fixed folding against more flexible staged edits on the same tasks. Cheap peeks
should append evidence without rewriting an unrelated warm prefix; batch mutations
when useful, and measure the cache consequences. Add a derivation/fold parent link
when recovery needs it, without making all knowledge a new graph subsystem.

The host capability check determines what Foundry can honestly enforce:

| Boundary | What can be claimed |
| --- | --- |
| CLI/MCP tool response | Foundry delivered bounded cited evidence. It cannot claim to have edited the host's conversation or measured hidden provider additions. |
| Verified pre-send host hook | The hook can acknowledge the actual serialized view revision/digest and included evidence IDs it applied. Supported mutations and protected regions must be tested on the pinned host. |
| Explicit owned request path | Foundry can control only the configured protocol and traffic. Today's gateway is meter-only; context rewriting requires an amendment and separate compatibility proof. |

Planning assumption, pending host preference: inspect **Oh My Pi first**, because
Foundry already has a measured integration. This is a capability-check candidate, not
a claim that its current hook can replace context. Keep retrieval available through
the existing CLI and MCP surfaces. If the host lacks the necessary boundary, report
that limit and choose a supported integration before promising context control.

The first complete lifecycle demonstration combines milestones 3 and 4: an agent
investigates a bug, records a user constraint and a rejected approach, folds an
oversized tool observation, restarts, recovers the exact observation, and produces a
verified patch without repeating the failed attempt or violating the constraint.
At least one needed fact must come from the interaction and be unavailable by simply
re-reading the repository. Add a changed constraint and stale-source case to expose
incorrect retention, not just successful static replay.

## Selective intervention and durable knowledge

Milestone 5 begins with observable events: repeating an unchanged failed action,
violating a retained constraint, encountering a contradictory observation, or dropping
an unresolved obligation. Each intervention cites its evidence and records why it
fired. Deduplication, cooldown, an explicit budget and silence are normal outcomes.
Compare no intervention, passive retrieved memory and selective intervention before
adding an always-running model. Evaluate behavior change through the external task
oracle; a persuasive reminder or a self-reported improvement is insufficient.

Milestone 6 promotes only policy-eligible, attributed knowledge into an explicit
scope. Test stable facts separately from changing facts and distinguish historical
truth from current applicability. Conflicting claims remain visible until resolved;
an unsupported consolidation cannot certify itself. Forget/export must follow
applicable copies, summaries, embeddings and retained versions, with limits named.
Removing stored text alone does not prove erasure of a live provider's cached state.
Derived records need source/version links and invalidation behavior before broader use.

## One shared measurement spine

Keep G1/G2 unchanged as historical regression instruments. Once their misses are used
to design a repair, they are development evidence for that repair. Create fresh
held-out questions before tuning candidates, with leakage checks for copied wording,
paths and identifiers, and freeze mixed-intent cases separately from pure locators.
Score both single required definitions and complete evidence sets; one relevant hit
does not establish that a question is answerable.

For retrieval, predeclare candidate count, paired comparison, multiplicity correction,
per-corpus/intent guards and token/latency limits before opening the holdout. The
proposed promotion rule is a positive lower bound on paired delivered-evidence gain
at 95% confidence over current Gemma, at no greater mean delivered tokens under the
same request cap, with the existing G1 gates preserved. Report absolute coverage and
every loss stratum as well. Diagnose the attainable coverage ceiling before selecting
an additional absolute coverage target; 35.8% is the baseline, not a product goal.

Extend one small lifecycle suite through milestones 2–6, keeping development and
evaluation tasks separate. Pin host/model/settings, prompts, repository revisions,
tool schemas, task constraints and success oracles. Use repeated paired runs for
stochastic behavior and choose sample size/budget from a separate pilot before final
evaluation. Inconclusive results do not count as wins. Compare retrieval alone, then
task state, then working-view changes, then intervention, so attribution stays clear.

Measure capture, correct update, complete retrieval, applied view, and successful use
independently. Include interruption, restart, stale evidence, contradiction and forget
in the shared scenarios. Report invalid/refused/retried/missing runs explicitly; keep
the frozen harness's invalid-run rules. Include all paid attempts in cost accounting,
even if a run is invalid for quality acceptance.

Full-task economics includes uncached input, reported cache reads/writes, output and
reasoning categories where available, auxiliary calls, preparation/maintenance costs,
turns and wall time. Report one-time and amortized costs separately with the reuse
assumption. Unknown usage remains unknown; token estimates are not verified dollars.
Before any experiment exceeding ten minutes, record its decision, inputs, resource
and time budget, and stop rule. Use the existing [economics contract](../specs/003-agent-retrieval-context/contracts/adapter-economics.md).

## Readiness, adoption and later research

Fix cold preparation, cancellation, stale graph publication and multi-root budget
allocation when the selected tasks expose them, under 009/005/007 respectively.
Keep source-hash provenance intact; a comment-only edit is not permission to guess
that compiler evidence remains valid. Multi-root improvements retain explicit
admission, per-root identity and visible partial results. Cross-root graph resolution
would need a separately justified amendment. Avoid extra workers as a default response
to memory pressure.

Each accepted milestone includes CLI/MCP compatibility, status/diagnostics, scoped
bootstrap guidance and recovery instructions for its actual user workflow. Complete
the signed installed-artifact checks before advertising semantic retrieval in a
release. Future roadmap phases do not silently reopen accepted tasks or activate
deferred specs; select their concrete scope in the owning spec when entering them.

After these milestones, consider learned selection, memory-program evolution or a
serving-backend experiment only when a measured residual failure warrants it. Record
the simpler alternative, maintenance cost and held-out task benefit first. Reopening
013 requires an explicit owner decision; a better classifier label score is still
not a task outcome. KV-state/latent-memory research belongs to a backend that actually
controls inference, not a capability claim for an MCP server.

## Research decisions behind the sequence

These sources motivate experiments, not Foundry performance claims. No outside
benchmark result is substituted for local acceptance.

| Source | Adopt or test | Limit carried into the plan |
| --- | --- | --- |
| [CLM](https://arxiv.org/html/2609.37725v1) and [AutoCompact](https://arxiv.org/html/2610.02163v1) | Editable working context, protected regions, task-aware compaction boundaries (milestone 4). | Training, harness and backend changes are distinct contributors; neither paper proves an untrained Foundry adapter will obtain the same gain. |
| [FlowState](https://arxiv.org/html/2609.34565v1) and [PoS](https://arxiv.org/html/2610.01415v1) | Typed task state and explicit knowledge/work gaps (milestone 3). | Index growth and sidecar costs matter; PoS reports a setting with 5.06× total tokens despite lower action-model usage. |
| [Proactive Memory Agent](https://arxiv.org/html/2607.08716v1) | Separate bank updates from intervention; silence is a valid action (milestone 5). | An extra model on every step is not a default cost assumption; gains vary by domain/model. |
| [Dyad](https://arxiv.org/html/2610.03020v1) and [DynamicMem](https://arxiv.org/html/2606.22877v1) | Separate capture/update/retrieve/use and test evolving facts (milestones 3–6). | Gold-memory performance is not end-to-end memory performance; retrieval hits need not supply all required support. |
| [MemCodex](https://arxiv.org/html/2609.39765v1) | Consider evidence-channel choice and evaluated offline policy search later. | Account for evolution/maintenance and distinguish API-only results from latent access. No automatic self-promotion. |

Two implementation references inform proposed contracts. They are pinned references,
not vendored dependencies or claims of Foundry acceptance:

- **001 T010 evidence coverage:** [Trove at c4e4003](https://github.com/misterdas/hermes-trove/tree/c4e4003f94422908ec5f2086114df3ca70c22e4d),
  `evidence_compiler.py::compile_evidence`; `tests/test_evidence_compiler.py` tests
  `test_semantic_facets_cannot_erase_deterministic_facets_or_claim_coverage`,
  `test_duplicate_grounded_ref_cannot_certify_finite_coverage` and
  `test_conflict_and_unknown_are_explicit_states`. Retain explicit unknown/conflict and
  grounded support. Foundry's first amendment uses source spans and bounded intent;
  it does not claim arbitrary exhaustive coverage or adopt the Python subsystem.
- **Proposed 003 milestone 4 recovery:** [Pi-fold at 0fbb4b8](https://github.com/shaneconner/fold/tree/0fbb4b848ddebcea40cc11dba73bcd6cea028df4),
  `extensions/lib/folding.ts::{preparedFoldError,assertProjectionPreservesToolLinkage,recoverFoldMessages,peekFoldSource}`;
  `tests/verify.mjs::{gatePeekAndFoldIndex,gateSchedulingWireRoundTrip,gatePeekIsAppendOnly}`.
  Retain recoverability, tool linkage and append-only peeks. Foundry needs its own
  Rust ownership, deletion policy and real-host application proof. The published
  fixed-workload cost result does not establish agent-selected folds or recovery.
