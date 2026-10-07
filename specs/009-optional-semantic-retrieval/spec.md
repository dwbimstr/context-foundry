# 009 — Neural context that becomes useful and stays prepared

Status: T001 (preparation, cache, index and the supervised development worker), T002
(semantic context within the existing budget) and T003 (progressive preparation in the
MCP owner) implemented and accepted locally on 2026-10-05, under development isolation
only (see validation). T003's real lifecycle exercise passed its predeclared bounds in
the measurement phase (2026-10-06); package acceptance needs signing. D001 recorded 2026-09-29; its chosen values were
recorded 2026-10-03 (below). Reactivated for planning by the owner's preparation-cost
concern. Dependencies:
001; 003 for preparation during agent use. Implementation requires D001 below. Beyond
the owner's 2026-10-04 answers below, no model downloads, long experiments, predecessor
migration or external-repo changes are authorized.
External prerequisites (owner): authorization to run the pinned local weights, a
USearch C++ build on the Rust 1.90 floor, and isolation/package acceptance.

Owner answers, 2026-10-04:
- **Downloads and installs.** Authorized: the pinned artifact (downloaded and
  hash-verified that day) and pinned MLX/mlx-lm in a private venv.
- **USearch.** 2.26.2 builds on Rust 1.90.0 with Apple clang 21 and passes an F16
  2048-d persistence/update smoke.
- **Signing/notarization, 2026-10-06.** Signing is the final step, once the package is
  complete and working. Install, upgrade, rollback and uninstall are completed and
  checked with ad-hoc-signed development bundles first.
- **Package, 2026-10-06 (implementation; see
  [deployment](../../docs/deployment.md#lifecycle-and-installation)).**
  `scripts/package.sh --with-semantic --semantic-profile FILE` packages
  `libexec/foundry-embed` and `scripts/embed-worker-bundle.sh`. `PACKAGE.json` records
  pyo3, the linked Python library and the profile's runtime closure (Python, MLX,
  frozen-requirements SHA-256); the runtime, site-packages, model and profile are never
  packaged. `scripts/install.sh install|upgrade --semantic-profile FILE
  [--semantic-extra-read DIR]...` builds the ad-hoc-signed `FoundryEmbed.app` from the
  installed worker. It writes an installed profile copy with the bundle path and
  executable SHA-256 filled in; an upgrade rebuilds it for the new version.
  `disable-semantic` removes only those files. Caches and stores stay through disable,
  rollback and uninstall. Package acceptance with the real bundle remains open with
  signing.
- **Model comparison, 2026-10-07 (owner-authorized exception to the measurement
  freeze).** Google EmbeddingGemma 2 (`914f7f89`, run through llama.cpp because no MLX
  release supports it) was compared with the pinned Nemotron on the frozen checker tasks
  over a library-only rust-lang/rust store. It was within noise for `search` and
  delivered significantly less for `graph` on the development tasks, so the switch rule
  ("at least as much") failed and Nemotron stays. Both models, fused as specified, delivered
  less required evidence for `search` than lexical retrieval alone on these
  identifier-named tasks, mostly on usage questions. The task set does not cover
  natural-language queries without an identifier, which is where dense retrieval is
  meant to help. [Validation](../../docs/validation.md) has the numbers and the
  evaluation-only harness.

The semantic-item line form was decided 2026-10-04 (§ Documents, embedding units and
returned evidence).

Selected model: **`nvidia/Nemotron-3-Embed-1B-BF16`**, explicitly chosen by the owner.
Selected local artifact: **`mlx-community/Nemotron-3-Embed-1B-BF16-4bit`**, subsequently
supplied by the owner. Output: 2048 dimensions, cached as float32 vectors; the search
index stores float16 (D001 chosen values).
The earlier Llama Nemotron suggestion is superseded. D001 records pinned artifact/runtime
identities and a bounded executed scratch bridge. Production integration, the complete
recipe and installed-package acceptance remain open; that probe is not dense retrieval.

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
passed a synthetic persistence/update smoke and is the selected index library
(third-party C++ core); its build on the Rust 1.90 floor is an external prerequisite.
These pins stand. The chosen values below settle the recipe parameters; executing their
limit checks, quality, cancellation, aggregate resources and distributed package
acceptance remain open. The 512-token probe cap is not a selected chunk size; do not
prepare the corpus from this smoke.

### D001 chosen values (2026-10-03)

| Parameter | Chosen value |
| --- | --- |
| Code and Markdown embedding units | 001 delivery units (001 T005; Markdown sections from `pulldown-cmark` 0.13) combined greedily in source order up to 1024 model tokens, the rendered prefix and special tokens included |
| Other files | blank-line paragraphs, then lines, combined the same way up to 1024 model tokens |
| Serving limit to test | 2048 model tokens including overhead, checked at 2048 (at limit) and 2049 (limit + 1) |
| Batches | document batch of 8 inputs; query batch of 1 |
| Query-embedding ceiling | min(1500 ms, remaining read deadline) |
| Candidate merge | 001 tier-1 exact definitions first, then reciprocal-rank fusion with k = 60 over the lexical top 256 and the dense top 64 |
| Vector encoding | cache stays float32 (settled); USearch index `ScalarKind::F16` |
| Cache cap | 2 GiB per workspace by default |
| Matryoshka (MRL) prefix slicing | a later explicit profile change, not in this tranche |

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
Documented prefix slicing and re-normalization remain a later explicit profile change
(a new embedding profile and index rebuild), not an initial task. In the selected MLX
artifact, BF16 identifies the source checkpoint; the deployed weights are 4-bit. Weight
precision and cached vector encoding differ.
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

The inspected [loader](https://huggingface.co/mlx-community/Nemotron-3-Embed-1B-BF16-4bit/blob/d0408b94c50fc327b6ea37dce7409c51e020a4d8/nemotron3_embed_mlx.py)
defaults to 4096 input tokens and silently truncates. The adapter must tokenize with
the exact prefix/special-token recipe and split documents or refuse oversized queries
before encoding. Treat 4096 as the loader's bootstrap setting, not Foundry's permanent
document size. The serving limit to test is 2048 tokens including that overhead (at
most 32768); do not inherit the loader default. Check it with a bounded local run at
2048 and 2049 tokens, recording peak memory and deadlines, before corpus preparation.
Parameter count does not establish a usable sequence limit.
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
preparation throughput and storage. Include the document/section cases below. If the
1024-token unit misses required evidence or exceeds the declared resource bound, report
it; changing the unit limit is an explicit recipe change, not a tuning step. Fewer
vectors do not establish faster preparation: longer inputs change inference cost, and
batching pads to the longest item. A cross-model bakeoff or full-corpus chunk-size
sweep is not a prerequisite.
Freeze artifact hashes and the serving/chunk recipe before full-corpus preparation.

Completion: the chosen values above fix the unit and serving limits, batch sizes,
query deadline, merge policy, vector encoding and default cache cap. D001 still records
exact versions and artifact hashes as pinned, provider identity verification, the
executed serving-limit check, wire/schema additions and compatibility with 001. T001
cannot guess these in code.
The runtime admission proof includes one active embedding request and no waiting
inference queue shared by document and query calls, including work still executing
after a client timeout. A busy provider names the refusal before enqueueing work.
Record how the actual chosen runtime/adapter enforces this; a client semaphore alone
does not bound abandoned server-side work. If unsupported, D001 remains incomplete
for background preparation; do not silently add a retry/job service.
A run over ten minutes needs its decision, budget and stop rule first. Missing local
artifacts/runtime leave D001 incomplete. This is not a baseline-release prerequisite.

**Executed 2026-10-04**
([evidence](../../docs/review/009-d001-2026-10-04.md),
[build checks](../../docs/review/prerequisites-2026-10-04.md)). Every model call ran
through a Rust PyO3 0.29.2 bridge in an ad-hoc-signed App Sandbox bundle with a hard
`RLIMIT_NPROC=0` set before exec. In that profile, fork/spawn, outbound TCP, loopback
listen and outside reads/writes were denied.

- **Runtime and artifact.** Runtime: MLX 0.32.3, mlx-lm 0.31.3, transformers 5.18.0 (the
  dependency closure, not a pin). Artifact `d0408b94…` with its hashes verified.
- **Serving limit.** A 2048-token input (prefix included) encodes: 2048-d, finite,
  norm 1, peak footprint about 1.6 GiB. A 2049-token input is refused by the bridge
  before encode. Run alone at `max_length` 2048, the loader silently drops the extra
  token, so the Foundry-side count-and-refuse is required.
- **Token overhead.** The tokenizer adds no special tokens, so overhead is the prefix
  only: 2 tokens for `query: `, 3 for `passage: `.
- **YaRN.** MLX applies YaRN whatever the `apply_yarn_scaling: false` config key says.
  Against plain RoPE, cosine is 0.99714 at 2048 tokens. The upstream torch path
  dispatches on `rope_type` and ignores that key too, so YaRN is the intended behavior.
  The MLX port stays as shipped.
- **Admission.** An abandoned call keeps executing and concurrent calls are not
  serialized. Only a worker-held slot, released when the work truly ends, bounds it.
- **Reference comparison.** Owner-authorized: torch 2.14.1 and sentence-transformers
  6.1.0 in the scratch venv. The reference is the local upstream BF16 checkpoint
  `a5e0f804…`, hash-verified, with byte-identical tokenizer and token streams; the
  torch side ran outside the sandbox as development evidence.
  - MLX 4-bit against BF16: cosine median 0.981, minimum 0.978, over 13 inputs of up to
    2048 tokens.
  - The own passage ranks first on both sides for all three pairs.
  - The publisher's unchanged `compare_backends.py` gives this artifact cosine 0.986
    against torch.
  - The owner-selected 4-bit artifact stands. This is a fidelity record, not a quality
    benchmark.

Still open:
- process-kill and owner-death cleanup, part of the T001/T003 supervisor;
- package acceptance, which needs signing/notarization.

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
search index. The cache holds float32 vectors; the USearch index stores
`ScalarKind::F16` copies derived from them. The index scalar kind belongs to the
derived index, not the document function digest, so changing it rebuilds the index
from cache with zero document calls. Repair preserves source, feedback and cached
vectors; rebuilding an index replays vectors without document inference. Never define
rebuild as deleting the whole workspace state. Explicitly deleting all workspace state
also loses its cache. Cache corruption disables affected semantic data by name and
requires explicit repair/preparation; it never silently starts a full re-embedding
campaign. Source access remains available where authoritative storage is healthy.

### Documents, embedding units and returned evidence

Keep three units separate: the complete authoritative source, the text encoded into
one vector, and the source span returned to the agent. Existing storage blocks are
an internal representation; neither their 2048-byte size nor symbol-occurrence counts
dictate model calls. A vector hit identifies its entire input span, not an inferred
answer location within it. No generated summary is needed to bridge these units.

The document-unit limit is 1024 model tokens, including the rendered `passage: ` prefix
and special tokens. It differs from the 2048-token serving limit: serving capacity is a
ceiling; useful evidence delivery and edit cost determine the unit limit. Do not choose
large units merely because inference fits, then require a second model to find
passages inside them. Partition each admitted UTF-8 file in source order and greedily
combine adjacent pieces while the rendered input fits; a file that fits in one unit is
encoded once. A file in a language 001 maps (001 T005) starts from its outermost
delivery units: top-level units and the blocks between them. Markdown is such a
language; its units are `pulldown-cmark` 0.13 heading sections, which exclude
heading-like text inside code fences. A delivery unit that alone exceeds the limit is
replaced by its child delivery units and its residual regions (its bytes minus its
direct children's ranges) in source order, recursively; children and residuals cover
the parent's bytes exactly once, so no byte is encoded twice. Whitespace between
pieces joins the following piece; a trailing tail joins the last. Other files use
blank-line paragraphs, then lines. A piece that is still oversized splits at line
boundaries, then into token-bounded UTF-8 spans. Combining rather than emitting one
vector per short heading or function is the point of the greedy step. Pin tokenizer
and 001 grammar versions and tie-breaking in D001's recipe. This is a retrieval
partition, not an assertion of compiler scope or semantic completeness.

This partition is derived from 001's unit forest deliberately, so a semantic hit names
the same units lexical search and context deliver. It is not 001's search-document
partition: 001's byte-based part split of regions over 8192 bytes is a search-index
rule and does not apply here; oversized pieces follow the token rule above.

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
return a budget-fitting prefix explicitly labeled as a preview. Reuse 001's
bounded prefix trials; do not add query-time document inference, summaries or another
retrieval service. A neural evidence item carries the matched handle for the whole
matched unit, the ordinary handle for bytes actually returned, and a selection equal to
`whole_unit`, `lexical_span` or `preview`. A preview also carries a continuation for
its remaining range. Here the remaining range means the selected lexical span when one
exists, otherwise the whole matched unit; no continuation appears when exhausted. An
oversized lexical span uses the same bounded prefix rule and is labeled `preview`.
All these fields count toward the existing budget; if none fits, report an omission.
Search returns v2 locator lines without bodies, so a semantic search hit names its
matched unit's handle and never claims `whole_unit`.

**Decision — semantic item line form (2026-10-04; revised after cross-lab
refutation).** Recorded in context-v2 § Evidence items.

- The selection tag sits immediately after `L<a>-<b>`, before the optional label:
  `[whole_unit]`, `[lexical_span <matched handle>]` or `[preview <matched handle>]`.
- The item handle names the returned bytes. `whole_unit` does not repeat it.
- Bodies are verbatim. The packer tries the whole unit, then the lexical span, then a
  preview; `[signature]`/`[outline]` forms do not participate.
- After a preview's fence, `next: <handle>` names its remaining range. Item, fence and
  continuation pack as one indivisible rendering.
- Only neural candidates carry a tag.
- T002 extends the renderer and the parser together:
  - the selection is not an outline form;
  - a preview continuation belongs to its item, not to the response;
  - parser success alone is not semantic verification.

Preview text is a navigation aid, not proof that the answer was localized. Graph
expansion may use whole-unit or lexical-span evidence under its existing bounds, but
not a preview as a localized seed. A relevant filename or prefix alone fails a test
whose required evidence is later in the document. If that happens on the fixed target
questions, report it and decide an explicit recipe change before preparing the corpus;
do not declare a quality pass from document recall alone.

Use one configured profile and one searchable vector index. Changing the profile is
an explicit stop/configure/restart operation, not concurrent candidate preparation or
a live promotion manager. A mismatched index is unavailable by name until explicit
preparation rebuilds it for that profile; baseline retrieval continues. Keep completed
cache entries under their original function/input keys. Rollback restores the prior
configuration and rebuilds that profile's derived index from retained cache; no instant
rollback or zero calls for inputs never cached is promised. Old cached inputs
may support branch reversions until explicit cleanup. At the disk cap (2 GiB of cache
per workspace by default), preparation stops with `cache_full`, without a silent
eviction/recompute cycle.
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

Only one batch is outstanding: a document batch holds at most 8 inputs, and a query
embeds one input. Compute remaining work from current source inputs
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
behavior remains. The bounded lexical/dense merge chosen in D001 is checked for
candidate starvation before graph expansion/packing. The owned policy's search/graph choice controls
graph expansion, not whether semantic candidates are allowed. Query embeddings remain
per-request model work with a ceiling of min(1500 ms, remaining read deadline);
document reuse is not zero calls
per query. The 013 ModernBERT decision model has its own joint input and encoding;
it cannot reuse a Nemotron vector as that input. Semantic failure alone does not
disable a separately available policy. Its exact input/cost contract remains 013 D001.
Preparation does not await policy training, generated summaries or graph completion.

### Ordering without another required model

Candidate generation, ordering, graph expansion and packing have one retrieval owner.
The D001 merge is deterministic: 001's tier-1 exact-definition candidates first, then
reciprocal-rank fusion with k = 60, score 1/(60 + rank), over the lexical top 256 and
the dense top 64, ties ordered by path then start as in 001's tier-2 order. This is
ordinary ranking, not a learned reranker. Exact locator behavior and candidate
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

SC-00N is the acceptance of T00N: SC-001 is T001's outcome, SC-002 T002's and SC-003
T003's, each passing only with that task's verification.

### T001 — Prepare once and preserve expensive work

- **Depends:** 001 T001–T003, 001 T005 (delivery units for the code partition) and
  009 D001's recorded concrete decisions.
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
  fenced heading, a delivery unit over 1024 tokens that descends to its children and
  residual regions without encoding any byte twice, oversized section/line, Unicode,
  CRLF, empty source, 1024/1025-token unit inputs and 2048/2049-token serving inputs.
  Assert complete nonoverlapping byte coverage, exact model input length including
  prefixes, document batches of at most 8 inputs, and no unit identity inherited from
  storage ordinals. Editing
  one section reuses every unchanged rendered input; whole-file edits recompute that
  file's unit. Graph arrival and storage-block changes make zero new document calls.
  Rebuilding the F16 index from the f32 cache makes zero document calls.
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
- **Implementation decisions (captain, 2026-10-04; amended after a GPT-6 Astra refutation
  the same day, which found the architecture sound but the identity, admission,
  isolation and owner-death wording insufficient).**
  - *Tokenizer boundary.* Foundry tokenizes in Rust with `tokenizers` =0.23.2 (`onig`,
    no default features), loading the artifact's `tokenizer.json`. On 34 inputs (the
    D001 texts plus Unicode, CRLF and the 2048/2049-token cases) its IDs equal the
    publisher stack's: Python `tokenizers` 0.23.2 under transformers 5.18.0. Foundry
    alone applies the prefix, counts exact model input, partitions and refuses. The
    worker receives token IDs, never text. It loads through the unchanged publisher
    `load` and calls the unchanged `NemotronEmbedModel.__call__`; the first-party adapter
    only builds the input-ID and attention-mask arrays (right padding, pad ID 11, the
    dtypes `encode` produces), forces evaluation, copies the float32 output and clears
    the MLX cache. Pooling and normalization happen only inside the publisher model. The
    loader's `encode` is not called because it truncates silently.
  - *Parity before corpus preparation.* With tolerances declared before running
    (cosine ≥ 0.9999 and max absolute difference ≤ 1e-3 per vector), compare the
    direct-ID path with publisher `encode(input_type=None)` on identical rendered
    inputs: batch sizes 1 and 8, heterogeneous lengths (a short input beside a
    1024-token one), reordered batch members, Unicode/CRLF and the limit boundaries.
    Token parity alone does not establish batch invariance. A failure is reported and
    becomes an explicit recipe decision; batchmates never enter the cache key to hide it.
  - *Document-function descriptor.* A versioned descriptor, hashed with the exact
    rendered input bytes under an unambiguous length-prefixed encoding, gives the cache
    key. It covers:
    - the SHA-256 of every artifact file the loader reads (weights, configs, tokenizer
      files, loader source) and the quantization;
    - the Rust tokenizer version and effective configuration (special tokens on,
      right padding, pad ID 11);
    - the first-party adapter revision: array construction, input/mask dtypes and
      output materialization;
    - pooling/normalization (publisher mean + L2), 2048 dimensions, float32 output;
    - the verified numerical runtime closure: Python, MLX, mlx-metal, mlx-lm,
      transformers and NumPy versions, plus the hash of the venv's frozen requirements.

    Production rendering is exactly `passage: ` followed by the unit's source bytes,
    with no path, title or language. Worker location, signing identity, labels, the
    query recipe, ranking/packing, partition grammar and limits, and ANN scalar settings
    are excluded: changing them re-embeds nothing whose rendered input is unchanged. The
    worker's `hello` must equal the expected descriptor fields; it never selects them.
  - *Crate shape.* One crate, two features:
    - `semantic`, default on, gates the core's `tokenizers` and `usearch` =2.26.2 (both
      built on Rust 1.90). `--no-default-features` builds a lexical-only core, and the
      gates check that build.
    - `embed-worker`, default off, builds the second binary `foundry-embed`, the only
      code linking `pyo3` =0.29.2.

    The core never embeds Python. Semantic rows survive a build without `semantic`.
  - *Isolation admission.* Normal admission requires an accepted platform profile and
    passing startup enforcement checks. Until signing/notarization and package
    acceptance close, normal semantic execution is `isolation_unavailable` and source
    access is untouched.

    The development profile is a script-built, ad-hoc-signed App Sandbox bundle around
    `foundry-embed`, with:
    - read-only grants for the model directory and the operator's Python runtime only;
    - soft and hard `RLIMIT_NPROC=0` set by the supervisor before exec;
    - an `env -i`-style offline environment (`HF_HUB_OFFLINE`, `TRANSFORMERS_OFFLINE`,
      `PYTHONNOUSERSITE`, `PYTHONDONTWRITEBYTECODE`, isolated `sys.path`), with no
      credentials in its environment, argv or IPC;
    - descriptors closed beyond stdio and the liveness pipe.

    It runs only behind an explicit `--development-isolation` flag under the owner's
    existing authorization, and it is never advertised as production isolation. Its
    negative tests have positive controls:
    - reads and writes outside the grants and of the live store, and writes into the
      read-only grants;
    - symlink substitution;
    - inherited environment credentials and descriptors;
    - TCP, UDP, DNS and IPv6, outbound and listening;
    - fork/spawn and limit restoration;
    - oversized or malformed IPC;
    - memory breach, timeout and owner death.
  - *Admission and IPC.* Admission is enforced inside the worker. A nonblocking
    try-acquire takes the one shared document/query slot before any inference
    allocation. The slot is held through MLX evaluation, vector copy and the bounded
    write of the reply; client timeout or cancellation never releases it. A dedicated
    reader thread stays responsive during inference and refuses further `embed`
    requests with `provider_busy`, never queueing them. A replacement worker starts only
    after the old process is confirmed gone.

    Frames carry a protocol version, a request ID and the descriptor digest. Header and
    frame sizes are checked before allocation, and length arithmetic is checked. Limits
    apply per input as well as per batch (≤8 documents of ≤1024 IDs; one query of ≤2048
    IDs), and every ID must be below the vocabulary size. Replies are checked for an
    exact payload byte count, the expected dimension and finite values. Stderr is
    drained continuously into a bounded buffer. A late reply with a stale request ID is
    discarded. The capacity-one handoff stays occupied until a result is consumed or
    discarded.
  - *Owner death.* Before importing Python, the worker starts a native thread that
    watches a dedicated liveness pipe and a kqueue `NOTE_EXIT` on the owner PID. The
    supervisor passes that PID; the worker checks it equals `getppid()` at start and
    refuses otherwise. On either signal it calls `_exit` without the GIL, destructors or
    locks.

    Normal shutdown closes stdin, sends TERM, waits 5 seconds, sends KILL and reaps
    through the verified child handle. Ownership is released only after the process is
    gone. Tests kill the owner with SIGKILL during model load, during a long native call
    and while IPC is blocked, then assert the worker disappears within 2 s; a separate
    test kills a worker that ignores TERM.

    Named limitation: a worker stopped by SIGSTOP cannot exit itself. Package
    acceptance therefore needs an OS-level guardian (a launch guardian or service
    profile), which stays open with signing.
  - *Resources.* Process count is `hard` (`RLIMIT_NPROC=0`). Memory is `supervised`:
    the supervisor polls the worker's physical footprint (`proc_pid_rusage`) every
    250 ms against a 3 GiB ceiling (D001 measured 1.82 GiB for 8 × 1024 tokens). A
    breach kills the worker and stops preparation with `resource_limit`; so does a
    failed footprint measurement of a live worker, which is stopped like a breach. A
    job that requires a hard memory bound is refused.

    `--budget-seconds` counts from argument parsing, so profile verification, worker
    start, loading and partitioning are all included. At expiry no new batch is
    admitted. An in-flight call gets at most 30 seconds before the worker is stopped,
    and the report names `budget_exhausted` with committed counts.
  - *Storage.* Store schema 5 (`upgrade-store --to 5` from v1–v4) keeps every earlier
    table, including disabled features', and adds three tables:
    - partitions: path → source hash, recipe ID, and unit ranges with input keys, or a
      completed empty partition. The bound store supplies workspace identity;
    - the f32 vector cache keyed by input key;
    - semantic state: profile, stopped/paused/running, last error and committed counts.

    Cache commits precede derived publication. Mapping acceptance and every serving
    lookup check the current source hash, recipe and range. The USearch F16 index and
    its label map under `<store>/semantic/<profile id>/` form one validated generation:
    a missing, partial or mismatched pair is unavailable and is rebuilt from the cache,
    never served by directory name. Since generation format v2 (T002) the label map
    lists, per label, every unit location `{path, start, end, source_sha256}` its key
    had at publication, and the manifest records the `source_revision` those were read
    at and `coverage` (`complete` only when every admitted source had a current
    partition and every eligible unit's vector is in the index); a v1 generation is
    unavailable by name until preparation republishes it from the cache.

    Startup never resumes preparation. Purge commits removal and the stopped state
    under sole ownership; leftover derived files are ineligible and never repopulate
    the cache.

    Decisions made during review, 2026-10-05:
    - *Publication shares the budget.* `--budget-seconds` bounds the whole command,
      including publication. New batches stop when the remaining time falls below a
      publication reserve of max(5 s, budget/10). Publication then runs under the run
      control. If it cannot finish, the run reports `budget_exhausted`, the committed
      vectors stay cached, and `index_published` is false with the reason. Every run
      publishes pending committed coverage before admitting new inference.
    - *Status trusts committed payloads.* Status counts come from committed metadata:
      mappings, row lengths and stored function digests. It does not scan vector
      payloads. A payload tampered with after commit is caught by the lookup that uses
      it, which disables that vector by name and reclassifies it at the next
      preparation or repair. Every vector is validated (length, finite values) before it
      is committed.
    - *Destructive filesystem work is descriptor-anchored.* Purge, generation
      publication and removal open the store and `semantic/` directories without
      following symlinks. They then work through descriptor-relative calls that do not
      follow links, so renaming or substituting an ancestor after a check cannot
      redirect them.
  - *Profile and CLI.* There are three commands:
    - `foundry semantic prepare --profile FILE --budget-seconds N
      [--development-isolation]`;
    - `foundry semantic status`, which never loads Python, the tokenizer or a broken
      profile;
    - `foundry semantic purge`.

    The profile file is bounded (≤64 KiB) and versioned, and is parsed before any
    worker exists. It names the model directory, worker bundle, runtime paths and
    expected hashes. Every named object is resolved and verified, with symlink
    substitution refused, before publisher `load` runs. A missing, malformed or
    mismatched profile, worker, runtime or artifact gives a named semantic failure with
    no download, install, fallback execution or cache reset.

### T002 — Deliver useful semantic context within the existing budget

- **Depends:** T001; 003 for MCP delivery, 005 for graph-backed claims.
- **Scope:** provider adapter, bounded candidate merge and existing packer;
  `tests/neural.rs` plus relevant CLI/MCP cases. No reranker or owned policy trainer.
- **Decision first:** close the open semantic-item line form above, recording it here
  and in context-v2, before implementation; it is reviewed with 001's contract.
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
  Assert the merge order (exact definitions first, then k = 60 fusion over the lexical
  256 and dense 64 windows, deterministic ties) and a query embedding cut off at
  min(1500 ms, remaining read deadline) with baseline fallback.
  The normal path makes no reranker call. Exercise the semantic request deadline and
  deterministic fallback without requiring 013. When owned policy is implemented,
  013's inference integration adds the combined-model deadline case to these same
  fixtures; independent provider ceilings cannot accumulate. That later case does
  not gate the earlier semantic-only release.
- **Review/cutover:** advertise only the observed corpus/model/hardware scope. No
  task/token uplift from one fixture. Disable semantics without losing source/cache.
- **Decisions made during review, 2026-10-05:**
  - *Embedding wait.* The query embedding waits at most min(1500 ms, half the remaining
    read deadline), inside the ceiling above. Waiting the full remaining deadline would
    end the wait exactly at the request deadline, leaving no time for the baseline
    retrieval and final read, so the named fallback could never be delivered; half
    leaves the fallback at least as much time as the embedding had. Normal requests
    under the 5 s read deadline still wait 1500 ms.

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
- **Decisions made during implementation, 2026-10-05:**
  - *One worker, one slot (D1).* The MCP owner's resident runtime is the only model
    worker. It runs query embeddings and document batches of at most 8 inputs, one
    call at a time, behind one busy flag that clears only when the provider call
    really ends; a late reply is dropped but holds the slot until then. A query the
    supervised worker abandoned at its deadline counts as holding the slot until its
    late reply really arrives; every admission claims the slot before it checks for
    that late call, and releases a claim that finds one. A query while the slot is
    held gets `provider_busy` and baseline results (amended 2026-10-06: one query may
    wait for a document batch, see D5). A document batch is admitted
    only while the slot is free and no foreground query is being dispatched; query
    registration and that admission are one decision under a short lock never held
    during inference. Otherwise preparation pauses with `provider_busy`, keeps
    committed work and queues nothing.
  - *Shared steps (D2).* The CLI run and the MCP driver call the same functions in
    `neural::prepare`: partition a page of sources, select the next batch of
    missing inputs, validate and commit a batch, publish, and record the run's
    start and stop in the state row. Vectors are keyed by exact input, so a late
    result for an edited or deleted source may enter the cache, but eligibility is
    always recomputed from current sources. The CLI keeps exclusive ownership and
    its budget and publication reserve.
  - *The driver (D3).* One background thread per owner prepares the primary root
    only. Every store step takes the engine slot only while no foreground operation
    is in flight and gives it back; inference holds no slot and no transaction. A
    request arriving during a store step gets the usual retryable `busy` (amended
    2026-10-06: it waits for the step, see D7). Pause
    admits no new batch (the final stop check and the admission are one decision
    under the lock pause takes) and lets the in-flight batch commit; owner exit or
    EOF discards the uncommitted batch, also one that waits for the engine slot,
    and shutdown completes only after the worker is stopped and reaped, before the
    owner releases its store; each document call has 60 s plus the
    supervisor's in-flight grace, and expiry pauses with `provider_timeout`; a
    failed, malformed or wrong-function reply stops by name without publishing that
    batch; completion is `stopped` with no reason. A pass that saw the source
    revision change walks again. Publication happens at most once per committed
    batch: after the first, then when the vectors not yet published reach the size
    of the last published generation, and at every stop except exit, so partial
    coverage arrives early and rebuild work stays linear. There are no hidden
    retries, job journal, leases, retry queue or watcher, and startup never resumes.
  - *Control and status (D4).* `index` gains `semantic: "prepare" | "pause"` and then
    does nothing else; combined with `root` or `scip` it is `invalid_argument`.
    `prepare` starts or resumes and returns at once with the state and reason; it is
    refused `provider_busy` while an earlier call still holds the slot, before
    anything starts. Without a profile, or after a refused start, it is
    `semantic_unavailable` with the fallback reason. `status` adds a `semantic`
    object: T001's metadata-only census, the live state over the committed row, the
    reason, the last error, the provider observation with its time and the resident
    runtime word. The census gets half the read deadline and is named
    `deadline_exceeded` instead of failing status. The catalog grew from 977 to 986
    o200k tokens without changing any description.
  - *Measurement deferred (owner, 2026-10-05).* The real lifecycle exercise on a
    permitted declared corpus is the ignored test
    `real_lifecycle_exercise_on_a_permitted_declared_corpus` in
    `tests/neural_retrieval.rs`, with its run budget and bounds declared in it. It
    has not been run; no timing or real-model result is claimed.
- **Decisions after the measurement phase, 2026-10-06 (captain):**
  - *Queries during preparation (D5).* Measured: back-to-back batches of 8 inputs
    (about 3.2 s each with the real model) kept the one slot busy, so every query
    during preparation got `fallback:provider_busy`. Two changes. First, the owner
    records the time of every tool call it serves (search, context, retrieve,
    index — its `semantic` prepare and pause included — status, memory,
    references) at the tool boundary, before the tool runs, and while one arrived in
    the last 60 s each document call carries at most 2 inputs; otherwise up to 8.
    The driver reads that
    time when it admits a call, and a selected batch of 8 is then embedded 2 at a
    time. Second, a query that finds the slot held by a document batch of this
    owner's driver may wait for that batch to end, up to its own ceiling (min(1500
    ms, half the remaining read deadline)). Only one query waits: any other query
    gets `provider_busy` at once, and while a query waits the driver admits no new
    batch. A slot held by another query or by an abandoned late call still gives
    `provider_busy` at once, and a query whose ceiling ends first gets
    `provider_busy` and baseline results. The waiting query reads its ceiling again
    after every wake and right before it claims the slot, so a batch that ended after
    the ceiling is never claimed. The worker still runs one call and keeps
    no queue; the wait is in the owner and bounded by the request's deadline. Every
    admission still claims the slot before it checks for a late call, under the
    same admission lock.
  - *Dead owners' scratch (D6).* Measured: an owner killed by SIGKILL left its
    scratch run directory (`w-<pid>-<nanos>`, 100 MB for 013) and later runs never
    removed it. Every 009 and 013 launch now first removes, under its profile's
    scratch root, each run directory whose pid is no process. Only real directories
    owned by this user and named exactly that way are removed; nothing else under
    the root is touched. A candidate is validated on its own `O_NOFOLLOW`
    descriptor, then claimed by a no-replace rename through the root's descriptor to
    a fresh quarantine name of the live reclaiming process, `.reclaim-<pid>-<nanos>`.
    That name is outside the run namespace, so no reclamation ever takes it as a
    candidate. Only the pass that made the claim removes the tree, through
    descriptors, and only if the claimed entry is the validated directory; anything
    that took its name meanwhile gets the name back untouched. When that name was
    taken again, the entry stays under its quarantine name and is reported on
    stderr, never removed. Reclamation runs under the launch's control, checked
    before every name read and every kind check of every listing, and stops at it,
    leaving the rest for a later launch; the launch checks the control again right
    before it starts the worker. *Leak for safety (amended 2026-10-06, review M6):* a
    claimed tree whose removal stops (the launch's control stopped, an error, or the
    reclaiming process died) stays under its `.reclaim-` name for good, reported when
    the process lives to report it. A later launch cannot tell it from a directory
    someone else put there, so nothing removes it automatically; the owner of the
    scratch root removes it by hand. That trades a leak for never removing a
    directory that was not validated.
  - *Foreground requests during store steps (D7).* Measured: on a 60,739-file store
    every context query during the first ~12 minutes of cold partitioning failed
    `busy`, because back-to-back partition steps and whole-generation rebuilds held
    the engine slot. A foreground request that finds the slot held by a driver store
    step now waits for that step, bounded by its own deadline (and cancellation); a
    slot held by another foreground operation is still `busy` at once. While a
    request waits the driver starts no new step: it re-checks the in-flight count
    after it took the slot. The request classifies the slot's holder by one mark set
    while the slot is held, so a slot the driver just released is taken, never
    refused; and it checks its own control right after it took the slot, so a
    request stopped while it waited never starts its operation. Steps are short: a
    partition step ends after 8 sources
    or at its first source boundary after 100 ms. A publication reads its mapping a
    page of sources per step and its cached vectors a chunk per step (each ending at
    its first boundary after 100 ms), validates the old generation and builds and
    stages the new one with the slot free, and takes the slot again only for the
    final renames.
  - *Runtime word after a worker failure (D8).* `status` reported the resident
    runtime `ready` after its worker died. The runtime now keeps the last terminal
    failure a model call returned (`provider_exited`, `resource_limit`), cleared by a
    later call that succeeds, and `status` names it as `fallback:<code>` without
    calling the model.

D001's model choice and chosen values are settled; executing their checks, the cache
schema, the open semantic-item line form and isolated Rust-bridge integration remain
incomplete, and the external prerequisites above are the owner's. The owned learning
head cannot substitute for this proof. All tasks are proposed; no model/runtime
performance or implementation acceptance above has been measured in Foundry.
