# Team handoff simulation — 2026-09-29

Historical paper-review baseline. The subsequent [executed feasibility pass](feasibility.md)
replaces H1's superseded contract and narrows H3/H4/H5/H7 with actual probe evidence.
Read that disposition for current readiness; this report preserves what was known before execution.
The 2026-10-01 owner approval selects all 001/003 T001–T003 and optional standard
shared-owner MCP for concurrent OMP/Codex clients. The
[003 amendment](../../specs/003-agent-retrieval-context/spec.md#optional-shared-owner--approved-amendment-2026-10-01)
supersedes this walkthrough's earlier simultaneous-client limitation; it is not
permission to revive a custom socket/shim or treat unexecuted host proof as a pass.


**Verdict: hand off the source/CLI work in stages; do not hand off the complete
neural/learning ecosystem as implementation-ready.** The ModernBERT decision-head
target is preserved, but its implementable contract is missing. Several runtime and
deployment choices need evidence before a team can commit to delivering them.

Review base: `f51e6c64a5094b57654d81619b259b15e5e46117`. The small documentation
corrections alongside this report remove contradictions; they do not implement the
missing features. This report is a handoff aid, not another mandatory spec stage.

## Method and coverage

This is a paper execution: follow each task as its implementer, supply a concrete
input/failure, identify the next owner and stop where the contract requires guessing.
It is not a program simulation, model benchmark or independent review. No application
code, tests, dependencies, model weights, host configuration or services were changed.
No build, training, producer, provider or sandbox execution was performed.

Read all fifteen current spec dispositions, the six active specs, their three wire/data
contracts, architecture, learning, deployment, release, dependency inventory and CI.
Traced adjacent source owners in `main.rs`, `store.rs`, `ingest.rs`, `laya.rs` and
`graph.rs`; inspected test entry points and the Laya transport fixture. This is not an
exhaustive source audit or a rereview of the predecessor's entire history.

Rechecked the selected upstream MLX loader as source, without executing it, at revision
`d0408b94c50fc327b6ea37dce7409c51e020a4d8`; the pinned Rust training-path evidence is
in [the ModernBERT assessment](../learning.md#modernbert-and-the-cost-of-matching-laya--2026-09-29).
Local paths, corpora, credentials and weights are not included in this report.

Evidence labels below distinguish a **verified contract conflict**, an **unresolved
decision**, and an **unexecuted integration check**. None implies an observed runtime
failure. “Startable” means sufficiently directed for implementation, not implemented
or accepted. Team role names below are proposed responsibility assignments.

## Walkthrough: where the team reaches a stop

| Scenario | Paper execution and expected state | Handoff result |
| --- | --- | --- |
| Fresh checkout; no models or credentials | Implement 001 initialization, source commit, replay, retrieval and explicit repair. CLI can ship before other features. | Start with 001 T001. Existing `Engine::open` creates state and requires Tantivy, so do not treat the prototype as already satisfying the new contract. |
| Crash after source commit, before search commit | New source is durable; stale hits are rejected; pending work remains; explicit refresh repairs derived search. | Ownership/order are specified. Child-process fault tests are implementation work, not a missing architectural decision. |
| Agent indexes A, then mentions a path in B | Text mentioning B admits nothing. An explicit separate bootstrap creates B's store. A's reconciliation cannot delete B's records. | Specified. No watcher, shared ledger, automatic training or cross-repo graph is implied. |
| A second host opens A while its MCP session runs | The second owner receives `store_busy`; it cannot steal the lock or launch another writer. | An accepted first-release limitation, not a promised multi-agent service. Do not assign concurrent-serving work under 003. |
| Agent edits a source and asks for references | Re-index changes source revision; old compiler snapshot becomes stale; import a separately produced current snapshot through the same owner. | Import path is specified. Actual producer/version/snapshot fixture must be pinned; whole-revision invalidation trades simplicity for refresh cost. |
| Cold semantic bootstrap | Baseline source works; semantic setup needs exact assets, runtime bridge, profile, partition recipe and index library. | Stops at 009 D001, before full-corpus preparation. An MLX model name is not those inputs. |
| Document embedding times out; query arrives | Old runtime work still occupies its slot; query uses bounded baseline fallback; no second hidden job is queued. | Runtime admission/termination needs real integration proof. A client timeout alone cannot close this condition. |
| Semantics unavailable; ModernBERT policy available | Lexical candidates may still support a decision and current graph expansion, subject to the shared deadline. | Old query-vector dependency was contradictory and is removed in this review. Actual policy IPC still awaits 013. |
| Same task/query, different state or option order | The decision input, label meaning and replay identity must remain reconstructible. | Current feedback and superseded v3 cannot express this. Stop before training-data implementation; see H1. |
| First training round; then withdrawal and second round | Only permitted exact inputs and valid base contributions may train. Calibration/evaluation stay separate; rejected candidates are valid outcomes. | Principles are retained, but ModernBERT data/model/lineage/artifact fields and acceptance need replacement. No current training implementation exists. |
| Codex streams a tool request through the gateway | Host request must fit the declared Responses subset; enforce mode counts before generation; lost terminal usage stays unknown. | T004 must pin actual host/API behavior. A configured endpoint or HTTP fixture alone cannot pass. |
| Install on an Apple laptop; supervisor dies | Optional workers must remain confined and be contained/reaped; baseline must still work without their runtime. | Package/signing/MLX/LibTorch profile and owner-death behavior are unproven. A CPU microVM is not assumed to support the selected MLX runtime. |
| Upgrade schema after graph/memory work lands | One integrator allocates the next schema step; preserve all implemented owners' records; rollback uses a compatible binary or prior backup. | Specified ownership, but shared files must be integrated deliberately; feature teams cannot allocate versions independently. |

## Findings in impact order

### H1 — Learning is not yet a buildable team contract

**Strong; unresolved decisions; blocks 013 data/trainer/inference implementation.**
[013](../../specs/013-owned-learning/spec.md) explicitly marks T001–T004 and
v3 superseded (the linked contract is now replaced by v4). D001 had
not selected the exact ModernBERT checkpoint/head, trainable parameter set, joint
input schema, pretrained initialization, artifact layout or runtime bounds.

The failure is concrete: [current feedback](../../src/laya.rs) contains `task_id`,
`query` and `correct_strategy`; its key hashes task/query (base lines 100–127). Two
different states/options under that pair overwrite the same example. The old v3
contract likewise stores query vectors and identifies duplicate queries, rather
than complete decisions. Replaying those rows cannot reconstruct the requested model
input. Base encoder weights alone also cannot recreate a pretrained decision head.

**Owner: learning lead. Exit:** replace the existing 013 contract and four task entries,
without adding a second backlog. Specify exact state/question/ordered options and
stable choice-label mapping; consent for the complete input; identity/correction and
grouped split rules; tokenizer/masking/overflow; checkpoint/head shape and starting
weights; loss/optimizer/trainable parameters; calibration/abstention; repeated-round
lineage; bounded IPC and publication/rollback. Include worked examples for an option
reorder, changed state, oversized input, withdrawal and save/load. Head-only and
encoder-plus-head adaptation must be named distinctly. Do not recover the old small
classifier just because its contract is more detailed.

### H2 — Independent teams would implement incompatible policy interfaces

**Strong; verified conflict; corrected in documentation during this review.**
At the review base, 001's shared contract (then `context-v1.md` lines 150–153, now
superseded by [context v2](../../specs/001-source-state-recovery/contracts/context-v2.md))
and [009](../../specs/009-optional-semantic-retrieval/spec.md) lines
312–313 required reusing Nemotron's query vector and forbade another encoding. The
architecture and deployment prose also inherited that assumption. ModernBERT's
joint-input model cannot satisfy it.

Removed that coupling across the active contracts, architecture, deployment and
dependency inventory. Nemotron owns retrieval; ModernBERT owns its separate decision
input/encoding. Semantic failure alone does not disable an otherwise usable policy.
The existing graph-availability and explicit-strategy bypasses remain. Hard token
budgets and source authority remain deterministic.

**Owner: learning lead with retrieval maintainer. Exit:** H1's single bounded
prediction contract supplies the missing implementation interface. The documentation
correction does not establish that interface or measured latency.

### H3 — The selected MLX artifact has no verified Rust execution boundary

**Strong; verified loader shape plus unexecuted integration; blocks 009.**
The inspected [76-line publisher loader](https://huggingface.co/mlx-community/Nemotron-3-Embed-1B-BF16-4bit/blob/d0408b94c50fc327b6ea37dce7409c51e020a4d8/nemotron3_embed_mlx.py)
exposes Python `load` and `encode`, using MLX, mlx-lm, Transformers and NumPy.
It has no server/CLI entry point in that file. `load` can download on a nonlocal
path; `encode` enables truncation and defaults to 4096 tokens. These are source facts,
not evidence that Rust integration is impossible or that no other wrapper exists.

**Owner: semantic runtime lead. Exit in 009 D001:** pin one callable Rust-to-publisher
boundary and its transitive runtime/assets; establish local-only loading, exact input
length handling, one-active-call behavior after timeout, cleanup and numerical output
identity. Select the vector-search library, merge rule, partition/serving limits and
cache/resource caps already assigned to D001. All first-party glue stays Rust;
third-party runtime dependencies remain explicit. Do not write a new tensor engine,
silently substitute a different model or create a second daemon merely to bypass this
decision. Paper review cannot establish Metal execution or its confinement.

### H4 — Rust infrastructure does not yet supply the verified trained model

**Strong; source-supported gap plus unexecuted parity checks; blocks 013 model claims.**
Candle's inspected ModernBERT Q/K rotary path uses a no-backward operation; some
changing weights or falling loss would not prove correct encoder adaptation.
`tch` supplies mature LibTorch autograd/optimizers, but still needs the exact model/head
definition, weight mapping, tokenizer and train/eval behavior. See the pinned
[source analysis](../learning.md#rust-routes-and-remaining-ownership).

**Owner: learning lead. Exit:** after H1 pins the recipe, one bounded load/forward/
gradient/save/load comparison against a fixed reference. Head-only mode proves the
encoder unchanged and intended head updates; encoder adaptation proves intended
gradients including Q/K positional paths. Keep the existing small execution budget;
preparation/porting time is separate and cannot be called a ten-minute implementation.
If a bridge needs code, that is a later explicit feasibility implementation, not proof
obtained by this no-code review. Do not build several backends before resolving one.

### H5 — Deployment and total model residency are not determined

**Strong; unexecuted integration and capacity decisions; blocks advertised model packages.**
[Deployment](../deployment.md) proposes a signed macOS App Sandbox worker and a Linux
profile, but no selected model package demonstrates them. MLX and LibTorch also have
different loading/native-library needs. Per-worker limits plus serialized calls do
not bound the sum of two resident models, runtime caches and training allocations.
Startup, disabled-feature behavior and supervisor-death containment need an actual
target package. Generic CI does not exercise that boundary.

Capacity illustration only: assuming 400 million trainable parameters and float32
parameters, gradients and two AdamW moments, those four arrays alone total
`400,000,000 × 4 × 4 = 6.4 GB` (about 5.96 GiB). Activations, head differences, temporary
copies, allocator/backend storage and Nemotron are additional. This is arithmetic
under stated assumptions, not a chosen model size or measured hardware requirement.
The former small-head memory/startup limits cannot be reused.

**Owner: runtime/package lead, jointly with 009/013. Exit:** name the first hardware/OS
target, complete runtime inventory, supported resident/overlap states and numerical
load/inference/training budgets; demonstrate one packaged grant/cleanup profile there.
Choose frozen/head-only versus encoder adaptation for capability reasons, not to hide
an unmeasured memory ceiling. If training needs a different machine, record an explicit
offline artifact handoff and supported deployment scope; do not invent a remote job
platform. Libkrun remains conditional. Core-only packaging does not wait for this.

### H6 — An optional acceptance condition had become a release dependency

**Strong; verified dependency conflict; corrected in documentation during this review.**
At the review base, 009 T002 required a combined owned-policy/semantic deadline test
despite the portfolio saying neural readiness does not depend on learning. That could
hold a working semantic release until superseded 013 tasks were rebuilt.

009 now closes its own deadline/fallback behavior with deterministic routing. When
013 exists, its inference integration adds the combined-model case to the same
fixtures. No acceptance guarantee was dropped: the case is owned by the feature that
introduces the second model. No new measurement campaign or all-feature gate was added.

**Owner: release/integration maintainer. Exit:** retain these feature boundaries in
task assignment and release notes. A test requiring an unimplemented optional feature
must not become a prerequisite for an earlier advertised scope.

### H7 — Adapter/producer compatibility needs concrete inputs, not more framework design

**Strong; explicit but unresolved handoff inputs; blocks the corresponding integrations.**
003 T001 still needs an SDK release compatible with Rust 1.90 and its framing/handler
limits. T004 needs an exact host/API request subset and counting projection, including
tool/reasoning/streaming behavior and retry settings. A custom endpoint configuration
does not prove the host emits that subset. 005 needs the pinned real SCIP producer,
snapshot fixture and, for its large-workspace claim, a declared corpus and numeric
bounds. These are named gaps in the specs, not proven incompatibilities.

**Owners: adapter lead and graph lead respectively. Exit:** attach one pinned
consumer/producer example to each existing task before its dependent implementation;
prove the real exchange when execution is selected. Keep gateway delivery separate
from MCP, and import separate from compiler execution. An unavailable large corpus
blocks the scale claim, not the accepted small reference workflow. No generic proxy,
producer manager or compatibility matrix is required.

### H8 — Parallel feature work converges on one schema and command owner

**Worth exploring as a delivery risk; code-supported shared ownership.**
The source module owns all schema initialization; command dispatch currently opens
the engine before every operation. Graph, memory, semantic cache and feedback work
all converge on `store.rs`, `main.rs`, and later the MCP worker. Independent feature
branches can each pass local tests and still choose conflicting next schema steps or
restore automatic initialization. Existing specs correctly assign one schema owner;
task assignment must preserve that ownership.

**Owner: core integrator. Direction:** land 001 first, then allocate schema changes
when integrating each feature. Teams use separate branches/worktrees and nominate one
reviewer for shared store/dispatch changes. Hand off domain modules/fixtures in parallel
only after their required contract exists. No schema registry, migration framework or
new governance service is needed. This is a coordination risk, not an observed merge failure.

## Work that can proceed without waiting for everything

| Owner | Next concrete work | Dependency or stop condition |
| --- | --- | --- |
| Core integrator | 001 T001 recovery/upgrade, then T002 scan and T003 exact retrieval | Current accepted direction is sufficient to start when implementation is assigned; no ML gate. |
| Adapter lead | Pin SDK/host behavior; implement 003 against the accepted 001 surface | Real MCP acceptance follows 001. Gateway compatibility is a separately scoped T004. |
| Graph lead | Pin producer and a small immutable reference fixture | Implement 005 after 001 identity/revision; scale acceptance needs its declared corpus. |
| Memory owner | Implement 008 lifecycle after 001 | Coordinate the next schema step; no 009/013 dependency. |
| Semantic runtime lead | Close 009 D001's bridge/profile/index/recipe choices | No full-corpus preparation until the concrete model path works. |
| Learning lead | Replace 013 contract/tasks, then verify one ModernBERT/head path | Do not implement the retained v3 vector-head tasks. |
| Runtime/package lead | One actual target profile for optional model workers | Can investigate alongside core; cannot certify isolation by reading platform docs. |

002, 004, 006, 011, 012 and 015 stay merged/superseded. 007 federation, 010 generated
knowledge and 014 legacy migration stay deferred with their existing re-entry rules.
Explicit refresh, one store owner, manual repeated training and explicit selection are
retained limitations. Do not silently promise watchers, concurrent hosts, autonomous
training or cross-repo joins. No new feature spec or full-system release gate is needed.

## What this review establishes

The immediate code handoff is 001. The immediate ML handoff is bounded contract and
integration work, not a promise that a turnkey Rust Laya equivalent exists. The KISS
failures found here were incomplete propagation of a changed model boundary and an
optional test turned into a dependency. Both can be corrected without weakening the
ModernBERT decision-head goal or replacing ordinary source/graph/storage owners.

Documentation checks and Git verification are recorded with this change. They do
not prove gradient correctness, supported GPU memory, sandbox enforcement, real host
compatibility, retrieval quality, token savings or deployment readiness. Those proofs
remain with the existing owning tasks rather than another blanket audit.
