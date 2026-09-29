# Subtraction review

Current disposition: [executed feasibility](feasibility.md) replaces the obsolete
013 contract with v4 and selects a concrete Rust model path. The entries below are
historical source reviews; their unexecuted/undecided statements describe those passes.

## ModernBERT follow-up — capability is not library support

Owner clarification: the target is ModernBERT with a decision head. The earlier
vector-only head and its v3 contract/task details are superseded, not a smaller
implementation of the same behavior. D001 must replace those clauses before code.
The options below assess implementation/adaptation modes within the clarified target.

2026-09-29. Scope: current 013/v3 design against Laya revision
`4066d5d5fbf08b66c6757ddeedbd797bd7655bc0`, its typed-decision training notebook,
and primary Candle/Burn/tch source. This is a focused in-session review, not a new
whole-portfolio audit, model benchmark or independently validated backend selection.
The [learning assessment](../learning.md#modernbert-and-the-cost-of-matching-laya--2026-09-29)
records pinned source links and the alternatives.

| Strength | Concrete suffering and evidence | Disposition / correction | Proof limit |
| --- | --- | --- | --- |
| Strong | A maintainer implements the precise 131k-parameter head and considers the owned Laya-like capability delivered. Laya `build_sequence`/`DecisionModel` instead jointly encode state, question and variable candidate descriptions | Supersede the vector-head contract; D001 pins the clarified ModernBERT plus decision-head recipe and trainable parameters before backend choice | A simple head might suffice for fixed search/graph labels but does not meet the clarified architecture |
| Strong | A developer loads Candle ModernBERT, sees changing weights/lower loss, and calls encoder fine-tuning correct. The inspected Q/K RoPE path uses `apply_op3_no_bwd`, whose result has no backprop operation | Require reference-gradient checks on intended encoder paths; do not treat a forward implementation or one changed tensor as training support | Static code proof of a graph break, not an executed training failure; a differentiable alternative exists but the full training path is unverified |
| Strong | A platform plan inherits CPU-head startup/time limits and query-feature cache reuse for a full typed transformer | Scope the old limits/IPC to v3. A selected transformer requires joint-input identity, explicit encoder cost, tensor bounds and a verified package/jail profile | No added model, download, runtime, feature cache or gateway coupling is selected by this review |
| Worth exploring | A pure Rust framework port becomes a new ML-maintenance project before the product needs it | Investigate Rust over maintained LibTorch if encoder updates are required; test a frozen encoder/typed head first when that satisfies the task | Third-party native packaging remains real work; `tch` does not automatically instantiate ModernBERT or supply Laya's head |

The notebook updates encoder and head, so describing the actual reference recipe as
head-only would be wrong. Preserving the useful outcome does not require copying its
DDP, noisy-gradient objective, multilingual router or every integration. Nor does a
generic model loader recreate the checkpoint's pretrained decision capability.
The all-first-party-Rust requirement remains; third-party native libraries are a
separate dependency choice. No backend is installed or selected solely from this review.

Updated the existing 013 D001, v3 scope, architecture/deployment, roadmap and portfolio.
No new spec number or implementation task was added. Source, weights and external
repositories remain unchanged; planned gradient/parity checks have not run.

## Current amendment — ownership through deployment, 2026-09-29

The owner explicitly selected all first-party code in Rust, a Foundry-owned learning
system inspired by Laya, process isolation (libkrun only if needed), and both context
budgeting and forwarding/metering of model requests. These decisions supersede the
external Laya/no-gateway assumptions in the historical passes below. Review and plan
correction stayed in this session; no independent model review or live acceptance ran.

| Strength / affected story | Evidence and mechanism | Disposition and simpler direction | Tradeoff / proof limit |
| --- | --- | --- | --- |
| Strong — contributors must operate another project to improve Foundry | Earlier 013 required Laya commands/tokenizer/checkpoints; current `src/laya.rs::predict` posts to a separately operated HTTP server | Replace with Foundry-owned Rust training, head artifacts and private worker IPC; retire the legacy path in T003 | Rust ML library/MSRV and real train/save/load parity remain 013 D001 |
| Worth exploring — another encoder adds preparation and deployment work | 009 already computes a 2048-dimensional query vector; the strategy choice has two labels | Reuse that feature in a small CPU head with a frozen encoder; no extra query encoding or reranker | Semantic embeddings may not separate strategy labels well. This is a testable hypothesis, not full Laya parity or proven task benefit; deterministic routing remains available |
| Strong — process separation is mistaken for access isolation | Platform documentation distinguishes sandbox grants from launch; libkrun's own security model includes the host/VMM rights | Verify one native profile per advertised target; use a confined CPU VM only for a concrete unmet boundary | Packaged macOS signing/grants, Linux kernel controls and MLX GPU compatibility remain actual integration work; no fallback to an unrestricted worker |
| Strong — a context cap is presented as control of the agent's bill | MCP owns a tool response, not the host's complete request; a lost stream can hide billed work | 003 distinguishes delivery, host hooks and the explicitly requested Rust gateway, with provider count/usage and unknown-attempt handling | One supported API/host first; preflight adds a call and time. No invisible-traffic, subscription/OAuth, monthly-cap or guaranteed-savings claim |
| Strong — setup/release becomes a chain of model prerequisites | Prior learning tasks ended at checkpoint serving and left package/bootstrap ownership implicit | Explicit inspect/apply/connect, private grants, shutdown, upgrade/rollback and preservation on uninstall belong to existing feature owners | Optional jail/model/gateway failure blocks only that feature; lexical/MCP cuts remain independently releasable |

The full route is now explicit: install core → inspect/apply a named repository →
connect the host → prepare only selected optional capabilities → deliver budgeted
context → optionally forward supported model requests → approve examples → isolated
Rust training → evaluate/select/restart → repeat. Receipt observation never grants
training rights; outside-path mentions never add stores or watchers. The gateway
opens no store and grants no credentials to ML workers. Its additional T004 is real
new user-requested work, not an attempt to hold the original eighteen-task count.

No new numbered spec or universal stage was added. Current ownership is 001 truth,
003 bootstrap/adapter/gateway economics, 005 graph, 008 memory, 009 embeddings and
013 learning. Conditional VM images and gateway processes have real lifecycle cost;
they are not called simple merely because they use libraries. A generic jail factory,
model fleet, automatic promotion, provider retry layer and request ledger remain out.

The narrow next work is still 001 T001. 009/013 D001 and 003 T004 name concrete
dependency/host proofs for their optional features, not new blockers on the core.
The current [validation record](../planning-validation.md) lists checks and gaps.

Date: 2026-09-28. Scope: the proposed Context Foundry ecosystem and adjacent current
Rust/Laya boundaries. In-session review as requested; no independent audit or new
runtime measurement. This record supersedes the prior fifteen-spec readiness framing.

Follow-up: the owner's preparation-cost correction reactivated 009 for planning.
There are now six proposed workflows. The five-workflow disposition below records
the earlier subtraction pass; it does not override the current portfolio. Preserving
expensive vector work and useful partial readiness is a product need; its old custom
ledger and orchestration machinery are still not requirements.

The later whole-ecosystem pass below resolves 001 D001 and further narrows 009/013.
Earlier references to an unresolved storage choice describe the original review.

## Finding

We made the inherited architecture more detailed before sufficiently challenging
whether it belonged. Fifteen uniformly shaped spec/plan/task bundles turned optional
subsystems into apparently committed work. Structural checks passed, but did not prove
that this was the smallest viable product. The correction removes mechanisms and
release dependencies while retaining concrete safety and user-outcome contracts.

## Concrete complexity and dispositions

| Finding and user consequence | Evidence / mechanism | Simpler disposition and accepted limitation |
| --- | --- | --- |
| Two application protocols before one working agent integration | Former 002 specified custom framed socket traffic and an MCP relay; source currently has only CLI ownership | 003 uses direct SDK stdio and ordinary engine calls; simultaneous clients cannot share this first store owner |
| Persistence complexity could survive the language rewrite | [Source/pending commit and index replay](../../src/store.rs) span redb and Tantivy; search validation is additional application policy | 001 explicitly reopens the single-transaction alternative before hardening; current pair remains working but provisional, with no invented performance advantage |
| Graph correctness and a real producer were separate programs | Former 004/005/006 split source scale, graph publication and semantic production; a graph could pass fixtures without answering real code questions | 005 owns one actual producer, relation and large-workspace workflow; fewer languages and unsupported relations are explicit |
| Learning planned a control plane before any custom checkpoint had served | Former 012/013 specified dataset management, trigger states and live selection; actual [Rust boundary](../../src/laya.rs) only provides HTTP decisions/feedback | One external bounded batch, immutable artifact and manual selection/restart; automatic deployment and uninterrupted model swaps are absent |
| Measurement became a universal close stage | Previous `/measure` also closed functional specs; 011/015 grew separate evidence/release programs | Focused acceptance closes functional work; experiments answer numerical decisions; release uses one checklist |
| Deferred capabilities still looked executable | Fifteen plans and forty-five task slices included unselected models, federation and migration consumers | Five active workflows, ten disposition stubs; no implementation tasks for parked ideas |
| Large-workspace promises exceeded the prototype | [Ingestion](../../src/ingest.rs) holds all observed paths and reports in memory; current graph imports a whole capped producer bundle | 001 bounds reconciliation, 005 adds scoped producer updates and a real workload witness; no arbitrary file-count guarantee |

These are source/design conclusions about maintenance obligations, not causal proof
that every mechanism caused Prakarana's historical release delays. The previous private
audit remains the evidence for predecessor-specific findings. Its pending-cause
histograms do not establish ordering: `tags` may be emitted in an uncovered insertion
fallback and cannot be equated to independent roots.

## Two meaningful interface choices

**Agent ownership.** Chosen `foundry mcp` over stdio → ordinary engine methods → one
store owner. The discarded shape was host shim → custom socket request → shared daemon
→ engine. Both can deliver cited context. The former deletes framing/relay/socket
lifecycle obligations; the latter's actual advantage is simultaneous clients. That
advantage has not yet been needed by the first user task. SDK APIs/host behavior must
be verified during 003, not assumed from a diagram.

**Durable context.** Current `replace_source` commits source plus pending work in redb;
`refresh` commits Tantivy and clears matching work. Alternative `replace_source`
commits source, adjacency and FTS in one SQLite transaction; `context` reads one state.
The alternative removes replay/stale-index coordination but changes search semantics
and introduces a C dependency. We have not tested feature parity, scale or migration.
001 must record a concrete keep/replace decision; neither Rust purity nor a simpler
diagram proves the correct answer. No storage-plugin abstraction is justified.

**Learning.** A repeatable `train(manifest, output_dir)` external job plus explicit
`serve(checkpoint)` has one dataset consumer and one inference consumer. The discarded
shape added a Rust round scheduler, durable stages and live model-selection state.
Repeated batches meet the learning goal with fewer runtime states. The cost is manual
selection/restarts and no exact optimizer-resume claim. Actual Laya entry points still
need implementation and live evidence; the prototype does not already train.

Dependencies stay concrete: storage/search libraries are internal implementation
dependencies; MCP and compiler artifacts are real external integration boundaries;
Laya is an optional external runtime. Deterministic routing is the real fallback, not
an invented mock service. No new layer is added merely to make tests convenient.

## Review passes and limits

1. Necessity pass removed protocols, future frameworks and generic release programs.
2. Ownership pass retained recovery, source hashes, consent, held-out splits and model
   identity, while combining their rules with the consuming workflow.
3. Delivery pass allows a CLI release after 001, an agent release after 003, then
   independently useful graph, memory and learning. Optional work never gates earlier cuts.

The former specs/plan/tasks were hash-verified into an ignored local archive before
removal. The public historical refinement note is labeled superseded; it does not own
current contracts. Current checks are recorded in [planning validation](../planning-validation.md).
No code, model, dependency, live service or publication changed in this pass.

## Whole-ecosystem pass: ranking, learning and release

Scope: reread all 15 current spec files and both shared contracts, including the
post-Nemotron chunk policy; checked architecture, portfolio, release/workflow guidance
and current Rust source/store/search/packing, graph, ingest and Laya entry points.
Sampled adjacent tests and the local Laya Agent/package interfaces at
`4066d5d5fbf08b66c6757ddeedbd797bd7655bc0`. This is an in-session plan/source review,
not another exhaustive audit of Prakarana's 71 implementations or live runtime proof.

Intent stays: useful cited context in large repositories, trustworthy graph evidence,
bounded consumption, durable explicit memory and repeated Laya fine-tuning. Ordinary
retrieval must remain useful without training, a third model or a control service.

1. **Strong — replace simultaneous embedding-profile preparation.** The prior 009
   allowed a candidate to prepare while an incumbent served, adding two profiles'
   runtime/index readiness and a live selection pointer before any replacement use
   case. One configured profile with explicit restart and cache-backed rebuild removes
   that coordination. Tradeoff: a model change can temporarily use baseline retrieval;
   instant semantic rollback is not promised. Cache preservation remains mandatory.
2. **Strong — correct routing and dataset assumptions before adding a model.**
   [`laya::decide`](../../src/laya.rs) uses a substring heuristic that omits references,
   although 005 first supports definitions/references. It may send `calls_tracker` to
   graph because of a substring, and caller-oriented format examples overstate 005.
   001 now owns an explicit whole-token rule; format examples name references. The
   old 013 handoff also rejected different labels anywhere within a task group: a
   source-location step followed by a graph step would reject a legitimate task.
   Grouping now controls split membership; only contradictory duplicate queries fail.
   These are proposed behavior fixes, not changed Rust or executed training tests.
3. **Strong — retain one retrieval owner and separate learned responsibilities.**
   [`context_with_strategy`](../../src/store.rs) searches in both routes and adds graph
   evidence for one; [`predict`](../../src/laya.rs) returns one of two strategy labels,
   not document relevance. Do not reuse that confidence as a ranking score. Core
   ordering remains deterministic; 009 defers a learned reranker until required
   passages are present but wrongly ordered after merge/packing defects are fixed.
   A coarse whole-document hit does not establish passage presence or localization.
   Tradeoff: some ordering failures may remain until that bounded evidence justifies
   a scorer. No Foundry candidate-quality comparison has yet run.
4. **Strong — bypass unavailable decisions and share the request deadline.**
   [`main.rs`](../../src/main.rs) currently calls Laya before context construction.
   In the proposed ecosystem, querying it when no graph is current buys no route
   difference; independent provider timeouts could also accumulate beyond MCP's read
   deadline. 001/003/009/013 now specify zero router calls for unavailable graph or
   explicit strategies and one remaining-time allowance. Partial graph coverage is
   still disclosed. This reduces work without inventing a learned availability model.
5. **Worth exploring — require workflow benefit before recommending normal Laya
   activation.** The handoff's classifier screen permits a non-regressing candidate;
   it does not prove its extra model call improves a checked agent task or total cost.
   Keep the real two-round lifecycle and isolated serving tests, but leave normal
   routing off until the existing T003 comparison justifies it. Record the result with
   existing evidence, without a new promotion registry, scorer dataset or release gate.
   A rejected optimization remains a valid learning-lifecycle result.

The [retrieve/rerank distinction](https://sbert.net/examples/sentence_transformer/applications/retrieve_rerank/README.html)
supports a candidate scorer as a separate role; it does not establish that Foundry
needs one. The exact model's larger input ceiling likewise does not select its best
unit size. 009 keeps serving limit and unit limit separate and checks useful tail
evidence before preparing the corpus; a new reranker is not its truncation workaround.

| Spec | Disposition after this pass |
| --- | --- |
| 001 | Retain; D001 resolved for redb/Tantivy, exact accounting and one routing rule |
| 002 | Remains superseded by direct MCP; no second transport |
| 003 | Retain one owner; optional providers share its read deadline |
| 004 | Remains merged; no automatic watcher dependency |
| 005 | Retain real definition/reference facts and conservative freshness; do not manufacture callers |
| 006 | Remains merged into the actual graph workflow; no producer framework |
| 007 | Remains deferred; outside path mentions never enroll another repository |
| 008 | Retain explicit memory; no automatic reflection or training admission |
| 009 | Retain preparation/cache; one profile, useful units, deterministic ranking; no required reranker |
| 010 | Remains deferred; no generated-fact factory to compensate for retrieval defects |
| 011 | Remains merged; exact delivery and actual usage evidence, no governor |
| 012 | Remains merged into 013; no learning-data service |
| 013 | Retain external repeated fine-tuning; corrected groups and optional narrow inference |
| 014 | Remains deferred; no predecessor store reader or automatic migration |
| 015 | Remains the release checklist; no all-roadmap gate |

001's existing one-table replay and planned one-marker repair satisfy D001's bounded
retention condition at design level. Replacing the whole storage backend is not needed
to resolve the neural questions. The next implementation owner can take 001 T001;
009 D001 remains limited to concrete runtime/library/recipe inputs. Add no new spec
number or recurring architecture/measurement stage from this review.

## Adjacent-pattern pass: ownership, lifecycle and failure

The neural/reranker examples are instances of wider problems, not the review boundary.
This pass checked the six active specs, their shared contracts and nine inactive
dispositions for interactions across creation, normal reads/writes, interrupted work,
restart, disabling, deletion and upgrade. Traced the relevant Rust entry points and
Laya sequence helper; reviewed workflow/release guidance. This is a source/design
self-review. It neither reruns the historical 71-spec audit nor proves every possible
interaction safe. No live model, compiler, service or fault test ran.

The following findings are **Strong** because each has an explicit reachable failure
in current code or the preceding proposed contract. Planned behavior is identified as
such; it is not presented as an observed production failure.

| General pattern and concrete failure | Evidence | Correction in the existing owner; accepted cost |
| --- | --- | --- |
| A convenience entry point makes reads into maintenance | [`run`](../../src/main.rs) opens the engine before command-specific root validation; [`Engine::open`](../../src/store.rs) creates directories/database/tables and enqueues a missing index | 001 T001 distinguishes explicit initialization, existing-state reads and explicit repair. Reads do not add access history or training work; library locking/recovery I/O is still possible |
| Optional failure blocks unrelated capabilities | Current `Engine::open` requires a working Tantivy writer; the prior 013 config contract rejected startup | 001/003/008 preserve healthy authoritative reads/exports when lexical search is broken. 013 disables invalid learned routing visibly. Database-level corruption still fails; this is not permission to serve cached truth |
| One successful retry is mistaken for bounded lifecycle cost | The proposed repair previously quarantined each partial replacement; neural cache deletion did not exclude late worker writes | 001 T001 retains one original quarantine per repair and checks replacement identity; 009 T001 makes purge exclusive/offline. Operator-kept diagnostics can still consume disk; no automatic retention or purge epoch |
| Shared version numbers conceal different identities | Proposed 005 allowed per-document publication from different artifacts/configurations at one source revision | 005 T002 selects one compiler snapshot tuple and filters facts against it. Interrupted imports expose partial coverage, not a mixture of builds; no whole-run atomic graph swap |
| Validation hashes bytes other than those consumed | The proposed two-pass artifact importer could reopen a changed caller pathname after hashing | 005 T001 freezes bounded owned copies once for hashing and both passes. Explicit scratch budget/cancellation; copying has real I/O cost and a between-batch budget is not an OS disk quota |
| A feature exists but cannot be reached through its owner | 003 holds the store while 005 originally offered only a CLI writer for graph refresh | 005 T003 adds `import_scip` to that same worker using explicitly staged files. Edit/re-index/import works in one MCP session; compiler execution remains external and long imports still occupy the serialized operation slot |
| Independently planned features multiply upgrade paths | 005/008/009/013 each add durable state without a shared version-allocation rule | 001 owns the schema integer and explicit upgrade steps. Disabling features preserves their rows; model recipes and protocol identities remain distinct. No migration framework or implicit table creation |
| Correct parts can produce an incoherent combined answer | Candidates/provider decisions can predate source, memory or graph changes before response construction | The shared 001 contract validates eligible evidence in one final authoritative read transaction after model work. Stale items are omitted visibly; no model wait inside that transaction, global publication epoch or automatic retry loop |
| Input parity and nominal limits hide semantic loss | Inspected Laya `common.py::build_sequence`, revision `4066d5d5fbf08b66c6757ddeedbd797bd7655bc0`, slices state to the room remaining inside 512 tokens | 013 T001–T003 check full sanitized input before fitting/inference. Oversized labeled input fails the batch; serving returns a named fallback without model execution. No summarizer or larger-model retry |
| Runtime readiness becomes protocol/configuration churn | Optional graph/models become ready or fail independently of an MCP client session | 003 registers the implemented catalog once per process and reports readiness in data/status. Tool discovery performs no model load or maintenance; clients must inspect operation results |

These corrections extend focused acceptance in existing tasks. The portfolio still
has six active specs and eighteen implementation tasks; no new service, queue,
watcher, durability plane, spec number or acceptance stage is introduced. The shared
failure/snapshot rules live in [001's contract](../../specs/001-source-state-recovery/contracts/context-v1.md),
not separate feature-specific variants. Workflow prompts now ask for adjacent failure
patterns only where the changed boundary makes them relevant.

Retained limits remain deliberate: one stdio store owner, separate workspace stores,
conservative whole-source-revision compiler invalidation, offline learning preparation
and purge, explicit memory/feedback consent, and manual checkpoint selection. Deferred
federation, watchers, generated facts and migration remain dispositions, not hidden
release dependencies. The import tool resolves the actual graph-update dead end;
it does not justify a second transport or shared daemon.

Implementation is still required to establish recovery, bounded memory, real producer
coverage, MCP behavior and useful neural/learning outcomes. Per-item limits, completed
jobs, classifier scores and status counters are not substitutes for those results.
The next implementation slice remains 001 T001; optional model work does not gate it.

## Second adjacent-pattern pass: reuse, composition and hidden work

Scope: all six active contracts and nine inactive dispositions, with emphasis on
001 ingestion/handles, 005 publication/cardinality, 008 lifecycle, 009 work admission
and 013 dataset selection. Rechecked current `ingest.rs`, `store::validate_path`,
feedback replacement and the local inference client; reviewed shared ownership,
architecture and release boundaries. This is another requested in-session review,
not independent validation or a new mandatory stage. The following recommendations
are **Strong**; code evidence and proposed-contract failures are distinguished.

| Priority / pattern | Concrete failure and evidence | Existing owner and simpler correction |
| --- | --- | --- |
| High — identity reuse defeats stale-write protection | Proposed 008 reset recreated IDs to revision 1. A delayed update/forget from the old record could match the new record | 008 T001/T002 allocate revisions from one persistent memory counter in existing metadata. No per-ID tombstone history; a late create can still recreate an absent ID as explicitly documented |
| High — checking scope before use leaves a replacement race | Current [`ingest::sync`](../../src/ingest.rs) skips symlinks while walking, then calls `File::open` by pathname. A file/ancestor replacement can redirect the read | 001 T002 enforces root-relative no-follow opening at the ingestion boundary and refuses unsafe replacement. No second filesystem service or claim of an atomic live-tree snapshot |
| High — grouping is confused with novelty | Proposed 013 admitted only new training groups, ignoring new queries inside an existing group; `no_new_data` could also hide an invalid inherited base | 013 T001/T004 compare exact eligible rows with the selected valid base's contributions. Existing groups keep their splits; new rows count, unchanged rows supply bounded replay, invalid bases refuse first |
| Medium — individually bounded inputs multiply downstream | Proposed 005 bounded occurrence counts but left multiple-definition expansion ambiguous; duplicate document paths could replace the same scope twice | 005 T001/T002 store each symbol occurrence once, reject duplicate document/manifest paths before selection, bound definition lookup and preserve ambiguity. Stable public symbol/occurrence IDs avoid inventing resolved edges or private caller state |
| Medium — status conceals expensive preparation | Proposed 009 demanded exact unit totals even before the selected tokenizer/partition recipe had processed sources | 009 T001 records partition completeness with existing mappings and reports unknown totals. Status reads metadata only; context does not perform a full census. Metadata scans still obey deadlines rather than claiming constant-time counts |
| Medium — client cancellation is mistaken for completed remote work | 009 acknowledged that provider work can outlive a timeout, but a resumed client could submit more while the server remained occupied | 009 D001/T003 require runtime-side one-active/no-waiting admission, including abandoned work. Query busy falls back; preparation busy pauses. The chosen runtime must prove this; no retry/job controller is added |
| Medium — field bounds do not compose through serialization | The shared contract allowed 4096-byte paths but only 8192-byte CLI handle JSON. A valid escaped path produces a larger handle | 001 T003 accepts up to 32768 serialized handle bytes and tests search→retrieve round-trip. The ordinary path bound remains; this does not increase source size or response budget |

The serialization counterexample was checked with standard JSON only: a valid
4096-byte path containing escaped form-feed characters produces an 8353-byte handle.
That exceeds the old input cap. This is a contract arithmetic check, not execution
of Foundry's proposed CLI. The ingestion race is established by source ordering;
its deterministic interleaving test remains to be implemented. The other findings
correct proposed behavior and do not assert observed production incidents.

Retained tradeoffs were checked too. Failed scans preserve unobserved prior sources;
the engine promises an indexed snapshot, not immediate reflection of disk/ignore
changes. Consent checks occur at explicit handoffs; withdrawal neither untrains nor
automatically stops an already running provider. Learning keeps its group floors,
held-out evaluation and bounded replay. The single engine worker, separate workspace
stores, external compiler/model runtimes and staged releases remain intentional.
The nine inactive dispositions gain no executable work. Existing review prompts
already cover these interactions and need no additional checklist.

These are refinements of the same eighteen tasks. No new spec, worker, queue, retry
manager or observability ledger is introduced. The memory revision counter is the
small added state needed to distinguish reused IDs; it is neither a source revision
nor a new durability mechanism. Runtime acceptance and actual model/producer evidence
remain with the feature that makes each claim.
