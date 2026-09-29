# 009 — Neural context that becomes useful and stays prepared

Status: Proposed, 2026-09-28. Reactivated for planning by the owner's preparation-cost
concern. No dense retrieval or preparation worker exists in the prototype. Dependencies:
001; 003 for preparation during agent use. Implementation requires D001 below. No model
downloads, long experiments, predecessor migration or external-repo changes are authorized.
Selected model: **`nvidia/Nemotron-3-Embed-1B-BF16`**, explicitly chosen by the owner.
Selected local artifact: **`mlx-community/Nemotron-3-Embed-1B-BF16-4bit`**, subsequently
supplied by the owner. Initial planned output: 2048 dimensions, float32 cached vectors.
The earlier Llama Nemotron suggestion is superseded. Artifact revisions and runtime
versions remain to be pinned; no weights were downloaded or executed.

## Outcome and evidence

Explicitly enroll a workspace, get baseline context immediately, prepare semantic
retrieval progressively, retain that work across sessions and update changed inputs.
A semantic-enabled release must demonstrate that lifecycle; earlier lexical releases
remain independently useful and accurately advertised. Distinguish missing candidates
from bad ordering; a reranker cannot recover an absent candidate.

The predecessor M42 result reports 32,707 embeddings and approximately 2,920 generated
facts reattached with zero model calls in 30.2 seconds, compared with approximately a
day of prior model processing. Preserve that reuse outcome through ordinary libraries.
M26 supports semantic retrieval and exposes fusion losses. These historical records
do not select Foundry's model or prove its quality. Reuse them instead of repeating a
long attribution campaign to establish whether semantic retrieval is useful at all.

## Required behavior

- **FR-001:** Explicit workspace enrollment follows 001. Outside paths mentioned in
  sessions never admit sources. Vector caches remain workspace-scoped; an operator-
  managed runtime may share immutable model weights across workspaces.
- **FR-002:** Cache document vectors by exact rendered input and embedding-function
  identity. Unchanged restart, re-scan and derived-index repair repeat zero completed
  document calls. A lost uncommitted batch may repeat.
- **FR-003:** Source commits never depend on the model. Changed/deleted source versions
  immediately invalidate their old retrieval mappings. Query/document profiles match.
- **FR-004:** Preparation is bounded and resumable from committed cache entries.
  Queries neither trigger corpus preparation nor wait for corpus completion.
- **FR-005:** Semantic candidates pass current-source checks and the existing output
  budget. Partial coverage, query availability and preparation progress are distinct.
- **FR-006:** Validate the selected model/runtime recipe before full-corpus preparation. Real
  cold/warm/edit/restart behavior is required; owned policy training and generated summaries
  are not prerequisites. Coverage/dimension alone proves neither quality nor savings.

## Selection decision — D001

2026-09-29 [feasibility evidence](../../docs/review/feasibility.md): pinned MLX revision
`d0408b94c50fc327b6ea37dce7409c51e020a4d8` ran through Rust PyO3 0.29.2 with publisher
loader/MLX 0.32.3/mlx-lm 0.31.3 in a macOS App Sandbox bundle. A pre-exec hard process
limit retained model execution while denying tested process creation. USearch 2.26.2
passed a synthetic persistence/update smoke and is the initial index integration
candidate (third-party C++ core). Full recipe, max input, quality, MSRV, cancellation,
aggregate resources and distributed package acceptance remain open. The 512-token
probe cap is not a selected chunk size; do not prepare the corpus from this smoke.

The model family/checkpoint choice is resolved by the owner. After 001 D001, pin
the selected model's immutable artifact revision and compatible local runtime; its
tokenizer/chunk recipe; and one maintained Rust vector-search library with verified
persistence/update APIs. Keep Rust application code; no custom ANN engine or provider
framework. Record target hardware and model-runtime ownership. No assumed GPU/Apple
compatibility. Inspect existing local artifacts before selecting any download/run.

Use [NVIDIA Nemotron 3 Embed 1B BF16](https://huggingface.co/nvidia/Nemotron-3-Embed-1B-BF16).
The official card documents 2048-dimensional output, a 32768-token model input ceiling,
average pooling and L2 normalization, with distinct `query: ` and `passage: ` prefixes.
The selected adapter must apply those recipes exactly once; provider-managed prompts
must not be duplicated. The input ceiling is not a target chunk size or a guarantee
that a serving endpoint accepts that length. D001 records the actual runtime limit.

Start with the full 2048-dimensional representation; no dimension sweep is required.
Documented prefix slicing and re-normalization remain a later storage tradeoff, not
an initial task. In the selected MLX artifact, BF16 identifies the source checkpoint;
the deployed weights are 4-bit. Weight precision and cached vector encoding differ.
The checkpoint's OpenMDW-1.1 license is separate from Foundry's MIT code license;
weights remain separately obtained. Neither NVFP4 nor the older Llama Nemotron
checkpoint is an interchangeable default. No runtime/platform support is assumed
from the model name or the CUDA examples in the card.

For local execution use the owner's [MLX conversion](https://huggingface.co/mlx-community/Nemotron-3-Embed-1B-BF16-4bit).
Its card identifies affine 4-bit/group-size 64 and a bundled embedding implementation;
ordinary language-model loading is insufficient. Keep this external model runtime
behind the Rust provider boundary. Pin/review the loader and dependencies together
with weights/tokenizer; do not introduce a Rust tensor-engine rewrite or vendor this
code under Foundry's MIT license. All first-party bridge/supervisor code stays Rust;
the publisher's Python/MLX implementation remains an external dependency, not a
Foundry Python service to write or maintain. D001 must establish a supported callable
boundary for the pinned loader, or report `runtime_bridge_unavailable`. Do not assume
a CLI/API exists merely because the model repository includes a loader.
Production runtime integration remains unimplemented; the scratch bridge is proven
only at the boundary described above.

Apply the [deployment isolation contract](../../docs/deployment.md) to actual model
execution, including asset grants, no downloads/network, process cleanup and declared
hard/supervised resource bounds. D001 proves one usable native profile on the selected
hardware. An MLX artifact is not proof of Linux/libkrun compatibility; if native access
cannot be confined as required, semantic execution remains unavailable pending a
specific supported profile. Baseline source retrieval remains usable. This does not
authorize switching the selected model, downloading weights or installing a runtime.

The inspected [loader](https://huggingface.co/mlx-community/Nemotron-3-Embed-1B-BF16-4bit/blob/main/nemotron3_embed_mlx.py)
defaults to 4096 input tokens and silently truncates. The adapter must tokenize with
the exact prefix/special-token recipe and split documents or refuse oversized queries
before encoding. Treat 4096 as the loader's bootstrap setting, not Foundry's permanent
document size. D001 selects and passes an explicit tested serving limit, at most 32768,
including that overhead; do not inherit the loader default. Check the proposed limit
with a bounded local run, including at-limit/cap+1 behavior, peak memory and deadlines,
before corpus preparation. Parameter count does not establish a usable sequence limit.
Load an existing pinned local directory; a missing artifact fails by
name rather than taking the loader's implicit Hub-download branch. Quantization and
loader identity participate in the document function digest. BF16/4-bit/8-bit outputs
must never mix in one searchable space; document and query calls use the same profile.

The publisher's tests show a smaller weight footprint but slower 4-bit throughput
than BF16. Treat that as third-party evidence, not a local performance result or a
reason to change the owner's selected artifact silently. D001 measures actual bounded
preparation/query behavior before a large drain; a memory reduction alone cannot
justify a faster-preparation claim.

Validate this chosen model on a small fixed permitted source/query set with expected
spans: correct prompt handling, candidate coverage, delivered evidence, query latency,
preparation throughput and storage. Include the document/section cases below; compare
a finer split only when the whole-document policy misses required evidence or exceeds
the declared resource bound. Fewer vectors do not establish faster preparation: longer
inputs change inference cost, and batching pads to the longest item. A cross-model
bakeoff or full-corpus chunk-size sweep is not a prerequisite.
Freeze artifact hashes and the serving/chunk recipe before full-corpus preparation.

Completion: record exact versions, artifact hashes, input/chunk limits, provider
identity verification, cache disk cap, batch and deadline limits, query merge policy,
wire/schema additions and compatibility with 001. T001 cannot guess these in code.
The runtime admission proof includes one active embedding request and no waiting
inference queue shared by document and query calls, including work still executing
after a client timeout. A busy provider names the refusal before enqueueing work.
Record how the actual chosen runtime/adapter enforces this; a client semaphore alone
does not bound abandoned server-side work. If unsupported, D001 remains incomplete
for background preparation; do not silently add a retry/job service.
A run over ten minutes needs its decision, budget and stop rule first. Missing local
artifacts/runtime leave D001 incomplete. This is not a baseline-release prerequisite.

## Identity and reusable work

The embedding profile owns model/weights revision and quantization, tokenizer and
query/document recipe, pooling/normalization, output dimension and numeric encoding.
Dimension is model output metadata, independent of source identity, context-token
budget and the owned strategy head. No arbitrary dimension requirement, padding
short vectors, or unsupported truncation. Validate finite vectors and the profile's
exact output length; same-length vectors from different profiles are not compatible.
Keep one active profile per derived index. Model replacement rebuilds derived vectors
from admitted source versions and refuses profile mixing; it does not rewrite source
truth, expand watched roots, change training consent or require a custom durable log.
Failure leaves baseline retrieval usable with semantic availability stated explicitly.
The document-cache key hashes the document embedding function and exact rendered
document input, including any path, title or language text sent to the model. Its
function digest includes only fields affecting document vectors. The retrieval profile
also identifies the query recipe and merge policy, but changing only query prompting,
ranking, output packing or a display label must not re-embed unchanged documents.
Different model weights/tokenization/pooling/normalization/output space do invalidate
document reuse. Identical body bytes alone cannot establish reuse. Endpoints are not
function identity. No operator "assume equivalent" restamp path is required.

Keep a vector-cache table in the existing transactional library and a rebuildable
search index. Repair preserves source, feedback and cached vectors; rebuilding an
index replays vectors without document inference. Never define rebuild as deleting
the whole workspace state. Explicitly deleting all workspace state also loses its
cache. Cache corruption disables affected semantic data by name and requires explicit
repair/preparation; it never silently starts a full re-embedding campaign. Source
access remains available where authoritative storage is healthy.

### Documents, embedding units and returned evidence

Keep three units separate: the complete authoritative source, the text encoded into
one vector, and the source span returned to the agent. Existing storage blocks are
an internal representation; neither their 2048-byte size nor symbol-occurrence counts
dictate model calls. A vector hit identifies its entire input span, not an inferred
answer location within it. No generated summary is needed to bridge these units.

Use whole-document-first preparation. D001 pins a document-unit token limit no greater
than the tested serving limit, including rendered prefix/special tokens. These are two
different limits: serving capacity is a ceiling; useful evidence delivery and edit cost
determine the unit limit. Do not choose 32768-token units merely because inference fits,
then require a second model to find passages inside them. If a complete
admitted UTF-8 file fits, encode it once. If it does not, partition it in source order:
prefer Markdown heading sections, then blank-line paragraphs, then line boundaries,
then token-bounded UTF-8 spans for an oversized paragraph/line. Markdown headings come
from a maintained CommonMark parser, excluding heading-like text inside code fences;
other source types use the paragraph/line fallback. Greedily combine adjacent complete
sections/paragraphs that fit, rather than emitting one vector per short heading or
function. Pin parser/tokenizer versions and tie-breaking in D001's recipe. This is a
retrieval partition, not an assertion of compiler scope or semantic completeness.

Every nonempty source byte belongs to exactly one embedding unit: contiguous ranges,
zero overlap, no omitted whitespace or tail, and no duplicate whole-file vector beside
its sections. Count the exact final model input; never recover an oversized input by
silently encoding only its beginning. Empty files remain source records with zero
embedding units. Graph availability must not change this partition; graph preparation
cannot trigger re-chunking or become a neural prerequisite. No extra vector per symbol
occurrence, edge, generated gloss or transcript.

Current `(workspace,path,source_hash,start,end)` ranges map to document-input keys.
Identical rendered inputs share a vector within the workspace; offsets and ordinal
changes alone cannot invalidate it. Repartition the changed file and reuse exact-input
cache hits. A one-byte edit to a whole-file unit requires re-encoding that unit; a
sectioned file can reuse unchanged sections. Boundary movement can change several
inputs. Raising an input limit does not automatically repartition an existing profile;
the operator selects a changed recipe explicitly. Recipe identity belongs in retrieval
readiness and policy outcome records, while unchanged document inputs retain their cache
keys when the embedding function is unchanged. Neither a storage-block change nor a
packing change justifies deleting the vector cache.

Partitioning/tokenization is preparation work, not a source-commit, query or status
side effect. The existing input-mapping owner records whether partitioning finished
for a given source hash and partition recipe, including a completed empty file.
Missing or stale partition metadata means unit count unknown for that source, not
zero units and not complete coverage. Build from bounded immutable source inputs
outside the engine slot; only the owner accepts current-hash mapping results. A
restart can repeat unfinished partition work, but completed mappings are reusable.
No separate corpus-count job, automatic tokenizer load or new truth ledger is needed.

For context delivery, try the matched unit in full under 001's exact output budget.
If it cannot fit, use the highest-ranked already-retrieved lexical span intersecting
that unit, clipped to its bounds; ties use source byte start. If there is no such span,
return a budget-fitting prefix explicitly labeled `selection:preview`. Reuse 001's
bounded prefix trials; do not add query-time document inference, summaries or another
retrieval service. Neural evidence carries `match_handle` for the whole matched unit,
the normal `handle` for bytes actually returned, and `selection` equal to `whole_unit`,
`lexical_span` or `preview`. A preview also carries `next` for its remaining range.
Here the remaining range means the selected lexical span when one exists, otherwise
the whole matched unit; `next` is null when exhausted. An oversized lexical span uses
the same bounded prefix rule and is labeled `preview`.
All these fields count toward the existing budget; if none fits, report an omission.
Direct search retains its byte cap and cannot label a shortened hit `whole_unit`.

Preview text is a navigation aid, not proof that the answer was localized. Graph
expansion may use whole-unit or lexical-span evidence under its existing bounds, but
not a preview as a localized seed. A relevant filename or prefix alone fails a test
whose required evidence is later in the document. If that happens on the fixed target
questions, revise the unit limit/partition before freezing the recipe and preparing
the corpus; do not declare a quality pass from document recall alone.

Use one configured profile and one searchable vector index. Changing the profile is
an explicit stop/configure/restart operation, not concurrent candidate preparation or
a live promotion manager. A mismatched index is unavailable by name until explicit
preparation rebuilds it for that profile; baseline retrieval continues. Keep completed
cache entries under their original function/input keys. Rollback restores the prior
configuration and rebuilds that profile's derived index from retained cache; no instant
rollback or zero calls for inputs never cached is promised. Old cached inputs
may support branch reversions until explicit cleanup. At the declared disk cap,
preparation stops with `cache_full`, without a silent eviction/recompute cycle.
Deletion immediately removes search eligibility; retained orphan vectors are disclosed
as cache retention, with explicit workspace-cache purge, not secure-erasure claims.
Purge is offline maintenance under sole store ownership, with no in-flight worker
or pending result handoff. A serving owner causes `store_busy`; do not implement a
live purge epoch to outrun inference. Purge removes vector/input mappings and the
derived vector index, preserves source/graph/memory/feedback, and leaves preparation
stopped until an explicit prepare command. An orphan result from a closed owner has
no writer/response channel into the new process. Resume cannot undo a completed purge
without an explicit new preparation request. Return actual remaining cache/index state.

## Bounded preparation under one owner

CLI preparation runs under exclusive ownership with explicit elapsed/work budgets.
During MCP serving, the existing owner selects one bounded batch of immutable inputs.
One model worker performs inference outside the engine slot, holding no transaction,
and returns through a capacity-one handoff. The owner validates profile, finite vector
length and current source eligibility, commits cache results, then publishes the index.
Publication failure leaves reusable cache entries. A late result after edit/deletion
can populate the cache but never restore stale source eligibility.

Only one batch is outstanding. Compute remaining work from current source inputs
minus valid cached results in bounded pages; no job journal, leases or retry queue.
Publish at most one bounded batch between foreground operations. Admit no document
batch while a foreground model request is being dispatched; do not queue model work.
An in-flight provider call may
be unpreemptible: queries obey their deadline and return baseline fallback. The local
runtime must enforce D001's admission bound; this does not promise contention-free
latency or bound other clients outside Foundry's control.

Pause stops new batches. Exit/cancellation preserves commits and discards uncommitted
work. Transport timeout stops preparation with a named resumable reason. Wrong-profile
or malformed output stops without publishing that batch. Explicit resume retries
missing inputs; no hidden retries, host CPU governor or system-service installation.
003's source operations remain serialized; provider inference does not occupy its
single engine slot. Another CLI writer still gets `store_busy`. External provider
work may continue after client cancellation. Owned policy training is a separate explicit job;
no cross-service scheduler is added here.

A timed-out client request is not proof that server inference stopped. The same
runtime admission slot stays occupied until that work actually ends. Query refusal
uses `provider_busy` and baseline retrieval; a document refusal pauses preparation
with that named reason and preserves committed work. Explicit resume while old work
is running must still be refused before new inference allocation. Do not create a
new connection/job to bypass the bound. Unrelated external clients are outside this
contract; claiming shared-runtime support requires the runtime to enforce the same
admission rule across its clients.

## Readiness and retrieval

Status names workspace/source revision and profile; exact known eligible, cached-current,
searchable-current and missing unit counts from current committed mappings; and
`unpartitioned_sources`, whose unit total is unknown. Report preparation running/
paused/stopped, last error and committed progress separately. Counts describe that
read snapshot, not lifetime job totals. No complete percentage while any source is
unpartitioned; zero known units is not an empty corpus. `empty` requires every admitted
source to have a current completed zero-unit partition, or no admitted sources.
Full coverage of an incomplete scan is not whole-workspace coverage.

Status reads mapping/cache metadata in bounded pages; it does not open source bodies,
tokenize them, load/probe a model or populate missing mappings. Ordinary metadata
enumeration still obeys the read deadline; large status scans can return a named
deadline failure. Context does not run this full census per request: report known
coverage or `unknown`, never infer corpus readiness from the candidate set. Provider
availability is separately `unknown` or the last observed state with observation time
and profile, not a live health guarantee. A cold model and missing corpus vectors are
different conditions. No additional persistent telemetry ledger is required.

Context may use current partial coverage and reports it. Baseline exact path/identifier
behavior remains. The bounded lexical/dense merge is pinned in D001 and checked for
candidate starvation before graph expansion/packing. The owned policy's search/graph choice controls
graph expansion, not whether semantic candidates are allowed. Query embeddings remain
per-request model work under the selected deadline; document reuse is not zero calls
per query. The 013 ModernBERT decision model has its own joint input and encoding;
it cannot reuse a Nemotron vector as that input. Semantic failure alone does not
disable a separately available policy. Its exact input/cost contract remains 013 D001.
Preparation does not await policy training, generated summaries or graph completion.

### Ordering without another required model

Candidate generation, ordering, graph expansion and packing have one retrieval owner.
009 D001 pins one deterministic lexical/dense merge and its bounded candidate window;
this is ordinary ranking, not a learned reranker. Exact locator behavior and candidate
retention are checked in T002. Do not add a model-selection router, query-rewriting loop,
reranker endpoint, second candidate journal or reranker configuration in this scope.
The owned policy under 013 can choose graph expansion; its confidence is not a passage
relevance score and must never reorder source candidates.

A learned reranker is a future option only after a fixed example shows that a required
passage is present in the bounded candidate set, but ordering loses it before packing.
First correct merge/deduplication/packing defects. Missing candidates belong to source
admission, preparation or retrieval; an unlocalized whole-document hit belongs to the
unit/delivery policy; stale graph facts belong to 005. Adding a ranking model does not
repair those failures. Use the existing T002 fixture to decide whether an optional
bounded query/passage scorer warrants a later amendment, with its added latency and
fallback stated. This creates no new spec, task or release prerequisite.

All inference for one context request shares 003's overall read deadline. A policy-worker or
query-embedding timeout is not a fresh time allowance for the next provider; pass the
remaining time and return the named baseline fallback while time remains. Background
preparation has its own explicit budget and is never started to answer the query.

## Tasks and acceptance

### T001 — Prepare once and preserve expensive work

- **Depends:** 001 T001–T003 and 009 D001's recorded concrete decisions.
- **Scope:** new `src/neural.rs`, cache/input mappings in the selected store library,
  CLI preparation/status/purge and `tests/neural.rs`; no MCP background worker.
- **Outcome/acceptance (FR-001, FR-002, FR-003, FR-004 / SC-001):** bounded preparation commits exact-
  profile vectors and resumes from cache; source/search repair preserves them.
  Unknown cache schema is a named error, not an authoritative reset.
- **Verification:** count document calls for first preparation, unchanged restart
  (zero), edited input, rename with/without title in input, retained-cache revert,
  different document function and query/ranking/display-only changes (zero new document
  calls). Interrupt before/after cache commit;
  repair index with zero document calls. Wrong-profile/nonfinite/short vectors and
  disk cap preserve valid data. Fixtures prove ordering; an actual permitted model
  must produce vectors that are durably read back.
  Assert no implicit download on missing local artifacts, no double prefix, refusal/
  splitting before the explicitly selected loader limit, and different keys for BF16
  versus the selected MLX 4-bit function. Loader/dependency changes cannot hide behind
  the unchanged upstream model name.
  Cover a small file (one unit), many tiny headings/functions (combined units), a
  fenced heading, oversized section/line, Unicode, CRLF, empty source and at-limit/
  cap+1 input. Assert complete nonoverlapping byte coverage, exact model input length
  including prefixes, and no unit identity inherited from storage ordinals. Editing
  one section reuses every unchanged rendered input; whole-file edits recompute that
  file's unit. Graph arrival and storage-block changes make zero new document calls.
  Change profile only across restart: refuse the old derived index, preserve its
  cache, and rebuild the selected one; restore the prior profile from retained cache.
  At no point may mismatched vectors serve or two profile preparation loops run.
  Attempt purge beside serving (busy, unchanged), then exit, purge and reopen: no
  source/memory/graph/feedback loss and no spontaneous re-embedding or repopulation.
  Direct source reads stay usable after a malformed cache/profile is refused.
  Cold status with unpartitioned sources reports unknown totals, not empty/ready;
  repeated status/context performs zero corpus tokenization and document inference.
  Complete empty-file partitions are distinguishable from missing metadata. Change
  one source and reject its old mapping/count eligibility without retokenizing others.
- **Review/cutover:** verify identity, source eligibility and repair preservation.
  Disabling semantics leaves baseline/source state intact. No predecessor-cache import.

### T002 — Deliver useful semantic context within the existing budget

- **Depends:** T001; 003 for MCP delivery, 005 for graph-backed claims.
- **Scope:** provider adapter, bounded candidate merge and existing packer;
  `tests/neural.rs` plus relevant CLI/MCP cases. No reranker or owned policy trainer.
- **Outcome/acceptance (FR-003, FR-005, FR-006 / SC-002):** actual model retrieves the
  frozen expected source spans for a stated vocabulary-gap fixture through the final
  response, with source freshness and exact output budget. Keep exact locator fixtures.
- **Verification:** real cold/warm query, provider deadline/fallback, profile mismatch,
  source edit/delete, partial coverage and neural candidates crowded by other evidence.
  Inspect candidate membership and delivered spans; count the full response. A protocol
  fixture is not a retrieval-quality pass. Preparation and query costs are separate.
  Include a whole document that fits the response and a long document whose relevant
  evidence is near its end, plus a code file containing unrelated features. Check final
  expected byte spans under the normal 2048-token response budget, not just filename
  hits. Separately force an unlocalized oversized hit and assert `preview`, truthful
  matched/returned handles and forward-progress continuation. Record units, input
  tokens, actual document calls, elapsed preparation and edit cost, vector bytes and
  delivered tokens on this same bounded fixture; no additional measurement campaign.
  Separately cover absent candidate, candidate demoted by merge, oversized unlocalized
  unit and correctly ranked evidence omitted by packing; attribute each to its owner.
  The normal path makes no reranker call. Exercise the semantic request deadline and
  deterministic fallback without requiring 013. When owned policy is implemented,
  013's inference integration adds the combined-model deadline case to these same
  fixtures; independent provider ceilings cannot accumulate. That later case does
  not gate the earlier semantic-only release.
- **Review/cutover:** advertise only the observed corpus/model/hardware scope. No
  task/token uplift from one fixture. Disable semantics without losing source/cache.

### T003 — Prepare progressively during real agent use

- **Depends:** T001/T002, accepted 003 owner/protocol, concrete bounds from D001.
- **Scope:** one model worker and owner handoff, prepare/pause/resume/status on the
  existing MCP adapter, fault cases and one real lifecycle exercise.
- **Outcome/acceptance (all FRs / SC-003):** baseline context during preparation,
  semantic evidence after coverage arrives, fresh baseline after an indexed edit,
  then preparation of only missing inputs. Source access cannot wait for full warm-up.
- **Verification:** slow provider while source/status operations execute; prove no
  engine slot/transaction held by inference. Exercise pause, EOF, full handoff, second
  writer, late result after deletion and restart. Read back exact cache/source counts.
  Let server inference outlive a client timeout, then send query and explicit resume:
  both respect the occupied runtime slot, no second job is queued, and baseline
  retrieval still works. Retry can proceed only after the old job actually ends.
  Unchanged restart makes zero document calls. Separate B-store preparation leaves A
  unchanged; a path mention starts no work. On a permitted declared corpus record cold
  preparation, useful partial query, steady query, edit catch-up and restart with actual
  model calls/timestamps. Predeclare the run budget and relevant quality/latency bounds.
- **Review/cutover:** name foreground stalls and missing proof; model failure cannot
  wedge the source worker. Remove any blocking preparation path from the MCP engine
  slot. Explicit index events are sufficient; watchers are not required by this task.

D001's model choice is settled; runtime/library/schema/bounds and isolated Rust-bridge
integration remain incomplete. The owned learning head cannot substitute for this proof.
All tasks are proposed; no model/runtime performance or
implementation acceptance above has been measured in Foundry.
