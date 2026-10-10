# Workflows worth building

Restarting this work on another host or with a fresh agent: start with the
[context packet](../docs/context-packet.md).

## Next planning direction — 2026-10-09

The [context ecosystem plan](../docs/context-ecosystem-plan.md) starts with retrieval
and then sequences real-host task results, persistent task state, reversible context
editing, selective intervention and durable knowledge. The owner authorized execution on 2026-10-09. Its first active tasks are
[009 T005](009-optional-semantic-retrieval/spec.md#t005--diagnose-and-improve-anchorless-evidence-retrieval)
(miss attribution and anchorless repair),
[001 T010](001-source-state-recovery/spec.md#t010--retrieve-behavioral-evidence-for-a-named-symbol)
(mixed symbol/behavior retrieval), and
[003 T006](003-agent-retrieval-context/spec.md#t006--validate-retrieval-repairs-in-real-coding-tasks)
(real-host outcomes and economics). They are in progress. The owner's subsequent
instruction authorizes sandbox implementation without waiting for measurements or
gates; 008 T003/T004 own experimental task state, recoverable working context, grounded
reminders and explicit promotion; 003 T007 owns CLI/opt-in MCP delivery. 008 T005
and 003 T008 connect task state to source retrieval under one budget and add recovery
search over retained originals. The owner's subsequent large-repository direction
reactivates 010 for source-bound LLM knowledge and selects 001 T011 (watching),
009 T006/T007 (parallel/progressive bootstrap), 005 T005 (boundary relationships)
and 003 T009 (task evidence). 001 T011 now implements opt-in primary-root watching,
bounded change reconciliation and foreground priority; 009 T007 now adds progressive
waves and priority paths. The subsequent efficiency audit selects CPU/startup reuse
first, then [009 T008](009-optional-semantic-retrieval/spec.md#t008--maintain-and-publish-only-changed-semantic-work)
(durable changed work, continuous maintenance, delta publication and cheap status)
and [009 T009](009-optional-semantic-retrieval/spec.md#t009--use-model-owned-geometry-and-contextual-representations)
(model-aware geometry and body/parent representations). The first CPU/startup slice
now reuses semantic workers and verified cards, defers CLI model acquisition when
vectors are cached and avoids unchanged-source chunk allocation. The next slice adds
bounded source reads/hash, ordered acceptance, periodic lexical publication and a
reusable ingestion/lexical pool. Verified artifact handoff, transactional cache accounting
and exact per-card fitting reuse now implement the remaining E1 work. T008 E2 adds
schema-9 durable changed-work queues, explicit continuous maintenance and cheap status.
T008 E3 now adds bounded ANN deltas, location-only publication, atomic manifest recovery
and visible publication backoff. Warm publication now refreshes affected locations and
reuses verified/resident segments. Generation v5 adds bounded location overlays, shared
metadata snapshots and clean-restart publication reuse. Cold verification, unmatched/overflow restart reconciliation and compaction remain
corpus-sized; matching changed-input checkpoints use bounded transactional lookups. T009 now implements opt-in body windows
with bounded parent context, versioned input/call geometry and actual-token batching.
Address profiles/caches remain usable; real-device parity/quality and independent
file/area representations remain open. 010 T002 now adds explicit selective HTTP
generation, atomic completion reuse and source-bound area summaries in schema 10.
Opt-in continuous MCP scheduling, shared client admission and source-first derived
context now work on both transports. Owned local generation adds pinned snapshot reuse,
idle unload and per-worker CPU/RAM/Metal controls. Shared memory admission is implemented; native CPU/global GPU pressure policies, Metal
hardware acceptance and real-model task acceptance remain open. See the
[revised build order](../docs/context-ecosystem-plan.md#revised-build-order).
005 T005 and 003 T009 now implement a scoped static Python HTTP boundary producer and
explicit primary-task evidence composition introduced in schema 11. Schema 12 adds
a transactional publication lookup for changed-input restart. Changed-source/config freshness,
ambiguous/unsupported/incomplete joins, atomic incremental publication, CLI/opt-in MCP,
source-bound brief discovery and delivered-facet accounting are verified in sandbox
fixtures. The next continuation adds admitted-root/selected-compiler evidence,
explicit exported task-view application in the Rust gateway, and installed OMP 18.6.0
continuation/history checks against a scripted upstream. Real Qwen CPU generation now
covers changed input and restart/revert cache reuse. Broader frameworks, live host-view
synchronization, real-device embedding/Metal and paired task quality/economics remain. See the [updated pending inventory](../docs/context-ecosystem-plan.md#pending-work-inventory--2026-10-10).
Other deferred specs remain deferred; 013 stays frozen off. The task
table below records the existing implementation baseline and remaining obligations.

## Implementation baseline

Status: revised portfolio. 001 and 003 T001–T003, including the optional shared MCP
owner for concurrent OMP/Codex sessions, were implemented and verified locally on
2026-10-01 ([validation](../docs/validation.md)); they are not yet released. On
2026-10-03 the owner selected the token-economics tranche — 001 T004–T006, 003 T005
and 007 T001 — and reactivated 007. 001 T004–T006 are locally implemented, accepted
at the reviewer's SHIP and committed (`5edf32c`), unreleased. 003 T005 (usage import,
economics tests, operator hook) and 007 T001 were implemented and accepted locally on
2026-10-04 and committed locally (unreleased); the real-host runbook is recorded in validation.
The same spec pass classified the remaining specified unknowns
of 003 T004, 005, 008, 009 and 013 as settled decisions, named open owned decisions
(005 T002's `references` header segments; 009 T002's semantic-item line form, both
decided 2026-10-04 in context-v2) or external prerequisites (below). Their subsequent
acceptance and 013's freeze are recorded in the task table. The owner requested
thoughtful subtraction, not a Rust translation of the old system. The other active
goals remain.

The [validated implementation](../docs/validation.md) is the baseline. The user
outcomes below retain the product goals. All active tasks specify behavior and
acceptance; 013's replacement ModernBERT/head contract is retained as frozen research.
The token-economics
tranche is implemented locally; a release waits until every active spec is complete,
then runs the ordinary [release checklist](../docs/release.md) (owner policy 2026-10-04).
The [team handoff simulation](../docs/review/handoff-simulation.md) traces startable
work, unresolved integration inputs and corrected cross-spec dependencies. It is a
source-backed paper walkthrough, not runtime acceptance or another required stage.
[Executed feasibility](../docs/review/feasibility.md) now provides bounded model,
isolation and protocol evidence, with unresolved checks assigned to their owning tasks.

KISS applies to the machinery, not the ambition. These specs are an organizational
choice, not proof of simplicity. Preserve the [capability commitments](../docs/architecture.md#sophisticated-behavior-through-simple-ownership)
and their failure guarantees while reducing independent state, protocols and operator
work. A release delivers the whole intended product, not basic search alone.

| Spec | User outcome | Needs | Completion boundary |
| --- | --- | --- | --- |
| [001](001-source-state-recovery/spec.md) | Find trustworthy, cited context after source updates or restart, in compact text with exact definitions first and deterministic outlines; from 2026-10-07 the city map: every definition has an address, a query resolves to the building or a short directory, in any of 23 languages | None | Useful CLI on its own; no agent or model required |
| [003](003-agent-retrieval-context/spec.md) | Bootstrap a repository, use Foundry before eligible grep/ripgrep and exploratory reads, deliver budgeted context at the fewest delivered tokens and optionally forward/meter model requests | 001 for MCP; gateway owns no store | Real agent tool-order/fallback acceptance; separately verified host/gateway protocol and accounting boundary |
| [005](005-graph-evidence-lifecycle/spec.md) | Follow real code relationships in a large workspace | 001; 003 for agent demonstration | One language and real semantic producer, bounded ingest and delivery |
| [007](007-multi-workspace-context/spec.md) | Ask one question across explicitly admitted repositories and receive one cited, budgeted response | 001 T004–T006; 003 T001–T002 | Admission at owner launch only; no cross-root edges, global snapshot or registry |
| [008](008-scoped-durable-memory/spec.md) | Remember, correct and forget explicit project knowledge | 001 | Independent extension; no transcript harvesting |
| [009](009-optional-semantic-retrieval/spec.md) | Prepare neural context progressively and retain expensive work | 001; 003 for foreground coexistence | Optional; from 2026-10-07 address cards on a llama.cpp worker, used only for queries that name nothing, enabled only after G2 shows a gain |
| [010](010-source-bound-derived-knowledge/spec.md) | Find behavior through source-bound LLM briefs and area knowledge | 001 source identity; 003 delivery; 009 resource admission | Experimental T001 and explicit T002 HTTP/CLI generation, completion reuse and area lineage implemented locally; continuous MCP scheduling/shared client admission implemented; owned local generation/per-worker controls implemented; native CPU/global GPU pressure policies, Metal acceptance and real-model task utility remain |
| [013](013-owned-learning/spec.md) | Improve decisions through repeated owned Rust fine-tuning of ModernBERT with a decision head | 001/003 for data; 005 for useful graph selection; separate from 009 retrieval vectors | **Frozen off 2026-10-07**: implemented tasks stand; no further work; signed-package items are not release conditions |

A release waits for all of them (owner policy 2026-10-04): every task of 001, 003, 005,
007, 008, 009 and 010 implemented and accepted, and 013's tasks as frozen on 2026-10-07,
then the ordinary [release checklist](../docs/release.md). Deferred and superseded
specs are excluded.

009 was planned alongside source ownership and is now implemented under development
isolation, with installed-artifact acceptance still pending. Its earlier blanket
deferral missed first-use and repeated preparation costs.
Lexical retrieval works without it, but a release includes 009.
Neural readiness does not depend on policy training or generating summaries.

The normal retrieval plan uses deterministic candidate ordering; from 2026-10-07 the
city map (001 T007, 005 T004) resolves addresses and doors deterministically. A learned
reranker is not selected; 009 names the concrete ordering failure needed to revisit it.
013 is frozen off (2026-10-07): its implemented commands, data and code remain as
research tooling under development isolation, are not built into the release package
(no `--with-learning`), and its routing is not advertised.
The [decision-ecosystem map](../docs/references/laya-decision-ecosystem.md) preserves
the broader typed-input/learning/calibration/deployment scope and review references;
the first search/graph family is not a claim of full Laya parity.

## Task detail and remaining execution inputs

| Spec | Detailed work | State and prerequisite that cannot be silently waived |
| --- | --- | --- |
| 001 | D001 resolved: retain redb/Tantivy; T001 recovery; T002 reconciliation; T003 exact retrieval; T004 compact v2 wire and atomic allowance; T005 syntax-unit search with exact definitions first; T006 outlines, context ladder and retrieve views; T007 addresses, anchors and anchored context; T008 languages; T009 parallel indexing | T001–T003 verified 2026-10-01; T004–T006 accepted locally at SHIP and committed in `5edf32c`; the T005 leading-run amendment accepted 2026-10-04 and committed in `bd1d890`; the 2026-10-06 tier-1 and compact amendments committed in `c3437e6` and `844796c`. T007–T009 approved 2026-10-07; G1 frozen 2026-10-08 before their code. T009 accepted locally 2026-10-08 (`89a8c54`); T007 merged 2026-10-08 (`4e23dee`) and 005 T004 merged (`d00fc19`, tie-group amendment `483810a`); G1 PASS 2026-10-08 on the city-map binary; T008 merged 2026-10-08 (`3e0a117`) with owner-approved forks of the Ruby, Perl and Rust grammars. T009's timing was recorded on 2026-10-08 for oh-my-pi; a valid rust-lang/rust run with the binary before T009 is still owed ([measurement handoff](../docs/measurement-handoff.md)). Release checklist remains |
| 003 | T001 bootstrap/MCP; T002 budgets/receipts; T003 actual agent task; T004 owned request gateway; T005 catalog/instructions, opt-in OMP hook, usage import and economics evidence | T001–T003 verified with OMP and Codex 2026-10-01. T004 (OMP on Z.ai `glm-5.3-flash`, meter only) implemented and accepted 2026-10-04 after three cross-lab review rounds (BLOCK, BLOCK, SHIP). Real OMP 18.6.0 passed the fixture checks, and 2 of 3 authorized live runs reconciled exactly with `usage import`. T005 implemented and accepted 2026-10-04, committed in `5e99ffd`; the hook is committed in the team-kit (`e65f8cf`) and installed default-off; real-host runbook recorded in validation. T007/T008 explicit task delivery and T009 bounded primary-task evidence composition are experimental implementations; real-host application and paired outcomes remain |
| 005 | T001 real SCIP; T002 scoped publication; T003 large-workspace workflow; T004 doors; T005 boundary relationships | T001–T003 complete and locally accepted, unreleased. T001 and T002 accepted 2026-10-04 (store schema 4; CLI `import-scip`/`references`) after five cross-lab review rounds. T003's agent surface (MCP `references`, `index {scip}`, compiler graph context) accepted 2026-10-05 after four rounds. Its measured run on rust-lang/rust 1.99.0 passed every criterion: index 430 s, import 30 s, query p95 105 ms against 250, session 861 s. T004 approved 2026-10-07; merged 2026-10-08 (`d00fc19`, cross-lab SHIP, gates green), amended for tie groups (`483810a`); G1 PASS with 001 T007 on 2026-10-08. Tie-group door latency recorded 2026-10-08 (rust-lang/rust `doors:each` p50 50 ms, p95 126 ms). T005 static Python HTTP producer implemented experimentally 2026-10-10; broader coverage remains. See validation |
| 007 | T001 admit, query and cite several roots in one budgeted response | T001 implemented and accepted 2026-10-04, committed in `cc402e0`; unreleased |
| 008 | T001 record lifecycle; T002 export/forget | Spec-pass decisions recorded 2026-10-03 (one `memory` tool). T001 and T002 implemented 2026-10-04 with store schema 3, after cross-lab review; acceptance and validation are recorded in validation. Unreleased |
| 009 | D001 runtime/index disposition; T001 durable preparation; T002 retrieval; T003 progressive preparation; T004 address cards on a llama.cpp worker | D001 chosen values recorded 2026-10-03; pinned artifact/runtime stand until T004. The semantic-item line form was decided 2026-10-04 (context-v2 § Evidence items). D001 serving-limit, isolation, admission and reference-comparison evidence was executed 2026-10-04 (4-bit against BF16 cosine median 0.981; YaRN intended). T001, T002 and T003 implemented and accepted 2026-10-05 under development isolation. The T003 real lifecycle passed its bounds on 2026-10-06: 2,099 units cold in about 845 s, steady queries 184–215 ms, edit catch-up 1 unit, restart 0. The 2026-10-07 model comparison kept Nemotron and showed dense fusion below lexical retrieval on identifier tasks. T004 merged 2026-10-08 (`fd01489`, cross-lab SHIP, gates green). G2 on 2026-10-08 selected EmbeddingGemma 2 (139 of 388 gained, 0 lost); G1 with the profile and vector parity also pass. Nemotron was INVALID and closed by the owner. Semantic retrieval stays off and unadvertised until the installed-artifact checks pass on the signed package ([measurement handoff](../docs/measurement-handoff.md)). The cache cap became an operator setting (2026-10-08). Package acceptance (signing) remains. See validation |
| 010 | T001 source-bound derived briefs; T002 governed generation and area knowledge | Experimental T001 implemented/tested locally on 2026-10-09: schema 8, strict CLI/library import, lexical conceptual discovery, exact citations, source-hash invalidation, replay/CAS/forget/recovery and source-first bounded context. T002 adds selective HTTP/CLI generation, transactional completion reuse and bounded area lineage in schema 10. HTTP fixtures use synthetic outputs; continuous MCP scheduling/shared client admission implemented; owned local generation/per-worker controls and shared memory admission implemented; native CPU/global GPU pressure policies, Metal acceptance and real-model task utility remain. See validation |
| 013 | D001 actual recipe/package acceptance; T001 joint-input data; T002 real fitting; T003 inference/rollback; T004 repeat/deploy | **Frozen off 2026-10-07 (owner).** T001, T002 and T003 accepted 2026-10-05 under development isolation; `src/laya.rs` retired. Measurement phase 2026-10-06: real two-round training on the 1,088 labeled rust-lang/rust rows (round 1 eligible on 71 held-out rows, round 2 a rejected candidate); T004's repeat-lifecycle checks passed; resident serving adds about 200–300 ms per routed request; exhaustive numerical comparison passes. The router changed one route of 71 with identical evidence and stays off; T004's signed profile and D001's package acceptance are no longer release conditions |

001 D001 is resolved. 009 D001's chosen values are recorded; its limit checks and
runtime/package acceptance remain. 013 D001 is recipe/MSRV/package acceptance, not a
new model/backend selection. Their partial executed feasibility does not close these
checks. A missing runtime input causes the named prerequisite failure in its spec; it
is not an invitation to guess or a passed criterion. The two owned decisions were
closed on 2026-10-04 and refuted cross-lab against 001's v2 contract before any
implementation.
[Current review/evidence](../docs/planning-validation.md) records what was inspected
and checked, including the 2026-10-03 spec-pass ledger. Task detail alone is not
implementation acceptance.

## External prerequisites

Recorded in the 2026-10-03 spec pass. Each is supplied or authorized by the owner; none
is implied by writing a task, and a missing one fails by name in its spec.

| Owning task | Prerequisite | Status |
| --- | --- | --- |
| [003 T004](003-agent-retrieval-context/spec.md#t004--forward-and-meter-an-actual-supported-model-workflow) | The owner's Z.ai key (a private file read into the gateway's environment) and the GLM Coding Plan terms acceptance; live check authorized at 3 runs and 15 minutes | Supplied 2026-10-04 |
| [005 T001/T003](005-graph-evidence-lifecycle/spec.md) | A permitted jailed rust-analyzer run (it executes build scripts and proc macros); the scale corpus and numeric profile for T003 | Producer authorized and run 2026-10-04 (installed 2026-08-31 build, no-network jail). Corpus: rust-lang/rust 1.99.0, cloned 2026-10-04. Numeric profile proposed at run selection |
| [009 D001/T001](009-optional-semantic-retrieval/spec.md) | Authorization to run the pinned local weights; a USearch C++ build on the Rust 1.90 floor; isolation and package acceptance | Weights and runtime authorized 2026-10-04; artifact downloaded and hash-verified. USearch 1.90 build passed. Package acceptance still needs signing/notarization (owner: decide later) |
| [013 D001/T002–T004](013-owned-learning/spec.md) | Approved labeled rows; the LibTorch package; signing/notarization; an aggregate residency run with 009 | LibTorch 2.11.0 downloaded and smoke-built 2026-10-04. Task set approved and checkpoint downloaded 2026-10-04; rows produced and used 2026-10-05/06. Frozen off 2026-10-07: signing and residency for 013 are no longer release conditions |
| [009 T004](009-optional-semantic-retrieval/spec.md) and [001 T008](001-source-state-recovery/spec.md) | G2: the source of at least 300 identifier-free questions with ground-truth definitions; the owner's acceptance of T008's binary-size change | Resolved 2026-10-08: G2 questions are written from code and frozen before 009 T004 code; all 15 T008 grammars accepted (about +48 MB) |
| [Release](../docs/release.md) | Every active spec complete, then a release destination (owner policy 2026-10-04) | Open; the local `v0.1.0` tag of 2026-10-04 was withdrawn |

## What was subtracted

- **002**: removed the custom socket carrier and forwarding shim. Default direct
  stdio remains; the 2026-10-01 owner-approved shared SDK MCP mode lives in 003.
  It adds no protocol dialect, federation coordinator or automatic daemon fleet.
- **004 + 006**: merged source bounds into 001 and the actual scale/producer workflow
  into 005. The new watcher scope is 001 T011; no general producer framework is required.
- **011**: moved deterministic delivery budgets and adapter usage receipts into 003,
  with exact packing in 001. Complete-request control requires real host hooks; there
  is no standalone cost-governance service. The owner's latest answer adds an optional
  explicit request gateway in 003 T004; it is separate from MCP delivery and source storage.
- **012**: merged consent, grouping and dataset handoff into 013. Learning owns its
  input contract; no separate learning-data platform.
- **015**: replaced a packaging capability with the release checklist.
- **014**: deferred migration until a concrete task
  justifies it. Its stub states re-entry criteria.
- **007**: reactivated on 2026-10-03 as launch-time multi-root context; federation,
  registries and cross-root edges stay out of scope.
- **009**: reactivated after the owner's preparation-cost correction; retain a narrow
  cache/preparation lifecycle without a custom ledger, generic scheduler or model fleet.

Old IDs remain as short disposition notes so historical references resolve. They
have no executable plans or task lists. Full prior drafts were preserved in an
ignored local review archive; they are not public requirements or a second backlog.

## How a spec earns more detail

Each active spec contains behavior, a small design, ordered work and acceptance.
Use a separate plan, task file or wire contract only when it helps a real implementer
or consumer. Do not generate three documents or three tasks for every idea.

Before adding a mechanism, ask: which current user failure needs it; what is the
simplest working alternative; what state, protocol or operator obligation would it
add; and what can be deleted? Record the answer where the decision lives. This is a
design question, not another gate or audit artifact.

The [subtraction review](../docs/review/subtraction.md) explains the decisions and
remaining tradeoffs. The [predecessor crosswalk](../docs/extraction-map.md) is a
historical inventory of lessons, not a requirements generator.
