# Workflows worth building

Status: revised proposals, 2026-09-29. This portfolio replaces the fifteen-bundle
plan. The owner requested thoughtful subtraction, not a Rust translation of the old
system. Planning changes are authorized; these documents do not claim new runtime
features, implementation approval, or an independent review.

The [working CLI](../docs/validation.md) is the baseline. Six user outcomes retain
the product goals. All active tasks now specify behavior and acceptance, including
013's replacement ModernBERT/head contract. The next
step is 001 T001: D001 now retains the existing redb/Tantivy pair for bounded recovery;
implementation and external execution prerequisites remain explicit.
The [team handoff simulation](../docs/review/handoff-simulation.md) traces startable
work, unresolved integration inputs and corrected cross-spec dependencies. It is a
source-backed paper walkthrough, not runtime acceptance or another required stage.
[Executed feasibility](../docs/review/feasibility.md) now provides bounded model,
isolation and protocol evidence, with unresolved checks assigned to their owning tasks.

KISS applies to the machinery, not the ambition. These specs are an organizational
choice, not proof of simplicity. Preserve the [capability commitments](../docs/architecture.md#sophisticated-behavior-through-simple-ownership)
and their failure guarantees while reducing independent state, protocols and operator
work. A narrower first release does not redefine the intended product as basic search.

| Spec | User outcome | Needs | Shipping boundary |
| --- | --- | --- | --- |
| [001](001-source-state-recovery/spec.md) | Find trustworthy, cited context after source updates or restart | None | Useful CLI release; no agent or model required |
| [003](003-agent-retrieval-context/spec.md) | Bootstrap a repository, use budgeted context and optionally forward/meter model requests | 001 for MCP; gateway owns no store | Real MCP client; separately verified host/gateway protocol and accounting boundary |
| [005](005-graph-evidence-lifecycle/spec.md) | Follow real code relationships in a large workspace | 001; 003 for agent demonstration | One language and real semantic producer, bounded ingest and delivery |
| [008](008-scoped-durable-memory/spec.md) | Remember, correct and forget explicit project knowledge | 001 | Independent extension; no transcript harvesting |
| [009](009-optional-semantic-retrieval/spec.md) | Prepare neural context progressively and retain expensive work | 001; 003 for foreground coexistence | Nemotron 3 Embed 1B, local MLX 4-bit profile; useful partial coverage, warm restart and delta updates |
| [013](013-owned-learning/spec.md) | Improve decisions through repeated owned Rust fine-tuning of ModernBERT with a decision head | 001/003 for data; 005 for useful graph selection; separate from 009 retrieval vectors | Contract v4; initial head adaptation, explicit selection and independently accepted package lifecycle |

Do not wait for all six to release. The first CLI release requires 001 and the
ordinary [release checklist](../docs/release.md). The agent release adds 003. A
compiler-backed graph claim adds 005. Memory and learning ship when useful in their
own right; they cannot hold the first usable releases.

009 is now planned alongside source ownership, before full-corpus model preparation.
Its earlier blanket deferral missed first-use and repeated preparation costs. The
lexical release can ship first; a release advertised as semantic must satisfy 009.
Neural readiness does not depend on policy training or generating summaries.

The normal retrieval plan uses deterministic candidate ordering. A learned reranker
is not selected; 009 names the concrete ordering failure needed to revisit it. The owned
policy learns optional graph expansion, not passage relevance. Its repeated-training workflow
remains active while normal inference can stay disabled without proven task benefit.

## Task detail and remaining execution inputs

| Spec | Detailed work | Prerequisite that cannot be silently waived |
| --- | --- | --- |
| 001 | D001 resolved: retain redb/Tantivy; T001 recovery; T002 reconciliation; T003 exact retrieval | Implement and verify the selected recovery contract |
| 003 | T001 bootstrap/MCP; T002 budgets/receipts; T003 actual agent task; T004 owned request gateway | Locked SDK and actual host; gateway additionally needs pinned Responses schema/counting and permitted API access |
| 005 | T001 real SCIP; T002 scoped publication; T003 large-workspace workflow | Real producer/snapshot; predeclared corpus, hardware and numeric run limits |
| 008 | T001 record lifecycle; T002 export/forget | Accepted 001 owner/schema contract; 003 only for advertised MCP surface |
| 009 | D001 runtime/index disposition; T001 durable preparation; T002 retrieval; T003 progressive preparation | Model selected; pinned artifact/runtime, Rust bridge, verified isolation and numeric execution bounds |
| 013 | D001 actual recipe/package acceptance; T001 joint-input data; T002 real fitting; T003 inference/rollback; T004 repeat/deploy | v4 pins model/input/backend; scratch gradients pass, complete recipe and actual package still need acceptance |

Nineteen implementation task entries and three decision tasks are recorded. 001 D001 is
resolved. 009 D001 and 013 D001 have partial executed feasibility evidence; their remaining
recipe/runtime/package checks are explicit in the disposition above. A missing runtime
input causes the named prerequisite failure in its spec; it is not an implementer's
invitation to guess or a passed acceptance criterion. [Current review/evidence](../docs/planning-validation.md)
records what was inspected and checked. No implementation acceptance has run merely
because task detail is now present.

## What was subtracted

- **002**: removed the custom socket carrier and MCP forwarding process. Use direct
  stdio MCP; multi-client serving is an unproven future need.
- **004 + 006**: merged source bounds into 001 and the actual scale/producer workflow
  into 005. A watcher and general producer framework are not prerequisites.
- **011**: moved deterministic delivery budgets and adapter usage receipts into 003,
  with exact packing in 001. Complete-request control requires real host hooks; there
  is no standalone cost-governance service. The owner's latest answer adds an optional
  explicit request gateway in 003 T004; it is separate from MCP delivery and source storage.
- **012**: merged consent, grouping and dataset handoff into 013. Learning owns its
  input contract; no separate learning-data platform.
- **015**: replaced a packaging capability with the release checklist.
- **007, 010, 014**: parked federation, generated knowledge and
  migration until a concrete task justifies each. Their stubs state re-entry criteria.
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
