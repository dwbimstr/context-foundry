# Workflows worth building

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
decided 2026-10-04 in context-v2) or external prerequisites (below); those specs stay
proposed. The owner requested
thoughtful subtraction, not a Rust translation of the old system. The other active
goals remain.

The [validated implementation](../docs/validation.md) is the baseline. The user
outcomes below retain the product goals. All active tasks specify behavior and
acceptance, including 013's replacement ModernBERT/head contract. The token-economics
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
| [001](001-source-state-recovery/spec.md) | Find trustworthy, cited context after source updates or restart, in compact text with exact definitions first and deterministic outlines | None | Useful CLI on its own; no agent or model required |
| [003](003-agent-retrieval-context/spec.md) | Bootstrap a repository, use Foundry before eligible grep/ripgrep and exploratory reads, deliver budgeted context at the fewest delivered tokens and optionally forward/meter model requests | 001 for MCP; gateway owns no store | Real agent tool-order/fallback acceptance; separately verified host/gateway protocol and accounting boundary |
| [005](005-graph-evidence-lifecycle/spec.md) | Follow real code relationships in a large workspace | 001; 003 for agent demonstration | One language and real semantic producer, bounded ingest and delivery |
| [007](007-multi-workspace-context/spec.md) | Ask one question across explicitly admitted repositories and receive one cited, budgeted response | 001 T004–T006; 003 T001–T002 | Admission at owner launch only; no cross-root edges, global snapshot or registry |
| [008](008-scoped-durable-memory/spec.md) | Remember, correct and forget explicit project knowledge | 001 | Independent extension; no transcript harvesting |
| [009](009-optional-semantic-retrieval/spec.md) | Prepare neural context progressively and retain expensive work | 001; 003 for foreground coexistence | Nemotron 3 Embed 1B, local MLX 4-bit profile; useful partial coverage, warm restart and delta updates |
| [013](013-owned-learning/spec.md) | Improve decisions through repeated owned Rust fine-tuning of ModernBERT with a decision head | 001/003 for data; 005 for useful graph selection; separate from 009 retrieval vectors | Contract v4; initial head adaptation, explicit selection and independently accepted package lifecycle |

A release waits for all of them (owner policy 2026-10-04): every task of 001, 003, 005,
007, 008, 009 and 013 implemented and accepted, then the ordinary
[release checklist](../docs/release.md). Deferred and superseded specs are excluded.

009 is now planned alongside source ownership, before full-corpus model preparation.
Its earlier blanket deferral missed first-use and repeated preparation costs.
Lexical retrieval works without it, but a release includes 009.
Neural readiness does not depend on policy training or generating summaries.

The normal retrieval plan uses deterministic candidate ordering. A learned reranker
is not selected; 009 names the concrete ordering failure needed to revisit it. The owned
policy learns optional graph expansion, not passage relevance. Its repeated-training workflow
remains active while normal inference can stay disabled without proven task benefit.
The [decision-ecosystem map](../docs/references/laya-decision-ecosystem.md) preserves
the broader typed-input/learning/calibration/deployment scope and review references;
the first search/graph family is not a claim of full Laya parity.

## Task detail and remaining execution inputs

| Spec | Detailed work | State and prerequisite that cannot be silently waived |
| --- | --- | --- |
| 001 | D001 resolved: retain redb/Tantivy; T001 recovery; T002 reconciliation; T003 exact retrieval; T004 compact v2 wire and atomic allowance; T005 syntax-unit search with exact definitions first; T006 outlines, context ladder and retrieve views | T001–T003 verified 2026-10-01; T004–T006 accepted locally at SHIP and committed in `5edf32c`; the T005 leading-run amendment accepted 2026-10-04 and committed in `bd1d890`. Release checklist remains |
| 003 | T001 bootstrap/MCP; T002 budgets/receipts; T003 actual agent task; T004 owned request gateway; T005 catalog/instructions, opt-in OMP hook, usage import and economics evidence | T001–T003 verified with OMP and Codex 2026-10-01. T004 (OMP on Z.ai `glm-5.3-flash`, meter only) implemented and accepted 2026-10-04 after three cross-lab review rounds (BLOCK, BLOCK, SHIP). Real OMP 18.6.0 passed the fixture checks, and 2 of 3 authorized live runs reconciled exactly with `usage import`. T005 implemented and accepted 2026-10-04, committed in `5e99ffd`; the hook is committed in the team-kit (`e65f8cf`) and installed default-off; real-host runbook recorded in validation |
| 005 | T001 real SCIP; T002 scoped publication; T003 large-workspace workflow | Spec-pass decisions recorded 2026-10-03. The `references` header segments were decided 2026-10-04 (context-v2 segments 12–14). T001 and T002 implemented and accepted 2026-10-04 (store schema 4; CLI `import-scip`/`references`) after five cross-lab review rounds (four REVISE, then SHIP). T003's agent surface (MCP `references`, `index {scip}`, compiler graph context) accepted 2026-10-05 after four rounds. T003's producer run, preselected questions and source check are recorded (`4f564f2`); the measured run is next. See validation |
| 007 | T001 admit, query and cite several roots in one budgeted response | T001 implemented and accepted 2026-10-04, committed in `cc402e0`; unreleased |
| 008 | T001 record lifecycle; T002 export/forget | Spec-pass decisions recorded 2026-10-03 (one `memory` tool). T001 and T002 implemented 2026-10-04 with store schema 3, after cross-lab review; acceptance and validation are recorded in validation. Unreleased |
| 009 | D001 runtime/index disposition; T001 durable preparation; T002 retrieval; T003 progressive preparation | D001 chosen values recorded 2026-10-03; pinned artifact/runtime stand. The semantic-item line form was decided 2026-10-04 (context-v2 § Evidence items). USearch builds on Rust 1.90. D001 serving-limit, isolation, admission and reference-comparison evidence was executed 2026-10-04: 4-bit against BF16 cosine median 0.981, and YaRN confirmed intended. Kill/owner-death and package acceptance (signing) remain. External prerequisites below |
| 013 | D001 actual recipe/package acceptance; T001 joint-input data; T002 real fitting; T003 inference/rollback; T004 repeat/deploy | v4 pins model/input/backend; spec-pass decisions recorded 2026-10-03; scratch gradients pass. tch 0.24.0 + LibTorch 2.11.0 and `tokenizers` 0.23.2 build and run on Rust 1.90 (2026-10-04). The task-checker task set was approved and the decision checkpoint downloaded on 2026-10-04 (`d1f609c`). External prerequisites below |

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
| [013 D001/T002–T004](013-owned-learning/spec.md) | Approved labeled rows; the LibTorch package; signing/notarization; an aggregate residency run with 009 | LibTorch 2.11.0 downloaded and smoke-built 2026-10-04. Task set approved and checkpoint downloaded 2026-10-04; rows are produced after 005 T003. Signing open (decide later). Residency open |
| [Release](../docs/release.md) | Every active spec complete, then a release destination (owner policy 2026-10-04) | Open; the local `v0.1.0` tag of 2026-10-04 was withdrawn |

## What was subtracted

- **002**: removed the custom socket carrier and forwarding shim. Default direct
  stdio remains; the 2026-10-01 owner-approved shared SDK MCP mode lives in 003.
  It adds no protocol dialect, federation coordinator or automatic daemon fleet.
- **004 + 006**: merged source bounds into 001 and the actual scale/producer workflow
  into 005. A watcher and general producer framework are not prerequisites.
- **011**: moved deterministic delivery budgets and adapter usage receipts into 003,
  with exact packing in 001. Complete-request control requires real host hooks; there
  is no standalone cost-governance service. The owner's latest answer adds an optional
  explicit request gateway in 003 T004; it is separate from MCP delivery and source storage.
- **012**: merged consent, grouping and dataset handoff into 013. Learning owns its
  input contract; no separate learning-data platform.
- **015**: replaced a packaging capability with the release checklist.
- **010, 014**: deferred generated knowledge and migration until a concrete task
  justifies each. Their stubs state re-entry criteria.
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
