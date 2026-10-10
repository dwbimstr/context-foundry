# 009 — Neural context that becomes useful and stays prepared

Planning addition, 2026-10-09: [T005](#t005--diagnose-and-improve-anchorless-evidence-retrieval)
is authorized for execution by the owner on 2026-10-09 under the [ecosystem plan](../../docs/context-ecosystem-plan.md).
It does not change T001–T004 acceptance or semantic enablement/package restrictions.

Experimental update, 2026-10-09: T006/T007 implement bounded parallel partitions and
progressive bootstrap. The subsequent efficiency audit selects their CPU/startup
follow-up, T008 incremental maintenance/publication and T009 model-aware input
geometry before broad 010 generation. CPU/startup reuse (E1) and durable continuous
maintenance/cheap status (E2), bounded delta publication/reuse (E3) and opt-in
model-aware body/parent inputs (E4) are implemented experimentally. Real-device
long-context parity, quality and throughput remain unexecuted. See the
[revised build order](../../docs/context-ecosystem-plan.md#revised-build-order).

Status: T001 (preparation, cache, index and the supervised development worker), T002
(semantic context within the existing budget) and T003 (progressive preparation in the
MCP owner) implemented and accepted locally on 2026-10-05, under development isolation
only (see validation). T003's real lifecycle exercise passed its predeclared bounds in
the measurement phase (2026-10-06); package acceptance needs signing. T004 (address
cards on a llama.cpp worker, city map) was approved on 2026-10-07 and implemented and
merged on 2026-10-08 (`fd01489`, cross-lab SHIP, gates green); it replaces the MLX worker
and the fused merge. Its real-model checks ran on 2026-10-08
([validation](../../docs/validation.md)):
- EmbeddingGemma 2 passed G2 (139 of 388 gained, 0 lost), G1 with the profile, and the
  batched-against-single-sequence check, so it is the selected profile.
- Nemotron 3 Embed 1B's G2 runs were INVALID, and the owner closed it.

The installed-artifact checks wait for Developer ID signing
([measurement handoff](../../docs/measurement-handoff.md)). Until they pass, semantic
retrieval stays off by default and a release advertises none.
D001 recorded 2026-09-29; its chosen values were
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
  signing. T004 replaces the Python closure: `PACKAGE.json` records the llama.cpp
  commit, and THIRD-PARTY takes llama.cpp's license from the worker
  (`foundry-embed --notices DIR`).
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
| Cache cap | 2 GiB per workspace by default; an operator sets another per run with `semantic prepare --cache-cap BYTES` or `mcp --semantic-cache-cap BYTES` (owner, 2026-10-08: T004's one card per definition gives the full rust-lang/rust store 332,732 cards, 2.74 GB at 2,048 dimensions), recorded in the state row and reported by `status` |
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

(Superseded by T004's cards and placement for the embedded text, the returned items
and their line form; the rest of this section stays as history of T001–T003.)

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
per workspace by default, or the operator's per-run setting), preparation stops with
`cache_full`, without a silent eviction/recompute cycle; a later run under a larger
cap resumes from the kept rows.
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

(T004 replaces the D001 merge below with placement: dense candidates only for queries
without an anchor, ahead of the lexical ones.)

Candidate generation, ordering, graph expansion and packing have one retrieval owner.
The D001 merge is deterministic: 001's tier-1 exact-definition candidates first, then
reciprocal-rank fusion with k = 60, score 1/(60 + rank), over the lexical top 256 and
the dense top 64, ties ordered by path then start as in 001's tier-2 order. This is
ordinary ranking, not a learned reranker. Exact locator behavior and candidate
retention are checked in T002. When T004 is accepted, this fusion, D001's "Candidate
merge" row and T002's merge checks are replaced by T004's placement: dense candidates
only for queries without an anchor, ahead of the lexical ones. Do not add a model-selection router, query-rewriting loop,
reranker endpoint, second candidate journal or reranker configuration in this scope.
The owned policy under 013 (frozen off, 2026-10-07) chose graph expansion; its
confidence is not a passage relevance score and must never reorder source candidates.

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

### T004 — Address cards on a llama.cpp worker (city map)

**Status:** approved 2026-10-07 (owner: "llama.cpp worker + cards"), revised after
cross-lab refutation the same day; implemented and merged 2026-10-08 (`fd01489`;
[validation](../../docs/validation.md)); accepted on its implementation evidence, with
its real-model and timing checks deciding enablement, not this task's acceptance.
Those checks ran on 2026-10-08:
- EmbeddingGemma 2 passed G2, G1 with the profile, and batched against
  single-sequence vectors, and is the selected profile.
- Nemotron was INVALID in G2 and closed by the owner.
- The installed artifact waits for signing
  ([measurement handoff](../../docs/measurement-handoff.md)).

Earlier evidence ([validation](../../docs/validation.md)):
fused into the final ranking, both Nemotron and EmbeddingGemma 2 delivered less
required evidence than lexical retrieval alone on the identifier-named checker tasks,
mostly on usage questions; Nemotron's only gain was on `graph` for dev (51 / 21), the
path-seeded expansion that 005 T004 replaces. Full preparation of 15,026 units took
6,889 s with Nemotron on MLX and 2,139 s with EmbeddingGemma 2 on a single-sequence
llama.cpp worker (model and runtime both differ); the average embedded unit was about
822 tokens.

- **Depends:** 001 T007 (definitions, addresses, anchors). **Scope:** `src/neural/*`
  (profile-owned dimension, templates and card limit replacing the `DIMENSIONS`,
  `DOCUMENT_PREFIX`, `QUERY_PREFIX` and `DOCUMENT_UNIT_TOKENS` constants; descriptor v2;
  self-describing cache rows; generation v3; card partition), `src/bin/foundry_embed.rs`
  rebuilt on a pinned, statically linked llama.cpp (Metal) with no Python, which
  `build.rs` builds from the pinned commit's tree (read from the git object store of
  `LLAMA_CPP_DIR`) with CMake and fixed flags (static, embedded Metal library,
  Accelerate, no OpenMP, `GGML_NATIVE=OFF` so the build is portable across Apple
  silicon, rustc's deployment target), never from an external build; the pyo3/MLX
  worker and its runtime closure removed, `scripts/{package,install,embed-worker-bundle}.sh`,
  THIRD-PARTY (llama.cpp; at the pinned commit its LICENSE also covers the vendored
  ggml, which has no license file of its own), tests in
  `tests/{embed_worker,neural_retrieval}.rs`.
- **Outcome/acceptance:**
  - *Cards, not bodies.* One card per definition (001 T007's one definition document per
    unit) and per Markdown section: its address, its signature or head lines and its
    leading documentation, at most the profile's card limit (128 tokens by default),
    rendered with the profile's document template. A card's cache key is its exact
    rendered input; its provenance is the existing partition row (path, unit range,
    source hash), accepted only as the cards the verified body renders (every range
    and key) and checked again against a fresh rendering for every card rendered into
    a batch, so eligibility is recomputed from current sources and an edit re-embeds
    only the cards whose text changed. The partition recipe names 001's search index
    version, which changes whenever units can, so a source partitioned before a unit
    change (even into no cards) is partitioned again; unchanged card inputs keep their
    cached vectors.
  - *Placement: only for queries without an anchor.* For such a query, dense retrieval
    returns the nearest cards; their units come first, in similarity order, each
    through the ordinary unit ladder (verbatim, then signature) and tagged as semantic
    candidates under the existing `semantic:<state>` header; the lexical candidates
    follow in today's order. A query with an anchor never calls the model. This
    replaces reciprocal rank fusion (D001's "Candidate merge" row, § Bounded candidate
    merge and T002's merge verification) and the semantic preview ladder; both are
    removed with this task.
  - *One inference, one prefetched batch.* The worker embeds up to the profile's batch
    of sequences per call in one llama.cpp context (non-causal pooling, sequences
    grouped by length). While one inference runs, the core may render and tokenize at
    most one next batch without holding the engine slot or a transaction; admission
    of that batch is decided afresh after the call ends, under T003's slot,
    foreground-wait, pause and late-call rules, which are unchanged. Only that batch's
    cards are rendered (a partly consumed source is parsed once per pass); a failed
    prefetch stops the run after the in-flight batch commits, and that batch is
    published at the stop.
  - *Data cutover.* Cache rows become self-describing: digest, dimension (u32 LE), then
    that many f32 values; a 2048-value row without a dimension is read as a descriptor
    v1 row; accounting sums actual row lengths, and each run reconciles the recorded
    total from the actual rows when it begins (a pre-T004 binary records 8,256 bytes
    per row; nothing is purged). A descriptor v1 (MLX) profile is refused
    at load with `profile_unsupported`, semantic retrieval stays off, and its retained
    cache rows and generations are kept until an explicit `semantic purge`; a profile
    change never purges. Generation v3 records its geometry; a v2 generation is never
    served by a v2 profile and is rebuilt from cache under the new one. Source, memory,
    feedback and graph are untouched by every refusal. Rollback across this task is
    binary plus profile: the pre-T004 binary with its v1 profile. That binary reads a
    self-describing row as a corrupt cache row (counted, re-embedded under its own
    profile or removed by `semantic purge`), never as source damage. No release exists,
    so T004 verified this on 2026-10-08 with the 005 T004 release build (its semantic
    code equals `7da1125`) on a store written by T004 with the fake worker: four rows
    counted corrupt, the v3 generation unavailable by name, source status and search
    intact, `semantic purge` removing every row (it reports four fixed 8,256-byte rows,
    not their actual bytes); re-embedding under its own v1 profile needs the MLX model
    and is in the [measurement handoff](../../docs/measurement-handoff.md), needed only
    if a pre-T004 build was ever distributed. § Identity and reusable work's rollback
    promise holds within one binary generation.
  - *Package cutover.* `install.sh upgrade` with an installed descriptor v1 profile
    keeps the core upgrade, disables semantic retrieval by name and asks for a v2
    profile; the bundle's Python and site-packages grants and `PACKAGE.json`'s Python
    closure are removed. `package.sh` takes the llama.cpp commit and license from the
    worker (`foundry-embed --notices DIR`, embedded at build), refuses a profile that
    pins another commit, and needs no llama.cpp checkout for a prebuilt worker. Signing
    remains the final step (sign, hash the executable, write the installed profile).
  - *Model.* The profile pins a single-file GGUF (SHA-256; the worker refuses a GGUF
    declaring `split.count` above 1 before llama.cpp loads it, since its other shards
    lie outside the verified inventory), the llama.cpp commit, pooling, dimension (a
    supported Matryoshka truncation, renormalized) and the query and document
    templates. G2 chooses between EmbeddingGemma 2
    (`ggml-org/embeddinggemma-2-GGUF@bfcd2987`, Apache-2.0) and a Nemotron 3 Embed 1B
    GGUF, on cards.
  - *G2 and enablement.* G2 is a frozen set of at least 300 questions that name no
    identifier, each with one ground-truth definition: written from the code and
    documentation of rust-lang/rust `library/` (answers are SCIP definitions of the
    frozen 005 artifact) and oh-my-pi (answers are top-level declarations, or direct
    methods of top-level classes, at its pinned commit; TypeScript has no SCIP artifact
    here) (owner, 2026-10-08), frozen before any
    T004 code, and checked by a cross-lab refuter for leaked identifiers, paths, name
    pieces and copied text and for answers another definition fits equally well. Its
    scoring harness (delivered evidence as G1 scores it, paired per question, delivered
    tokens) and the candidate profiles are frozen before the first G2 run. Semantic
    retrieval stays optional and off by default; a profile is enabled for anchor-less
    queries only when, on G2, its delivered evidence exceeds the same queries without
    it (exact McNemar p < 0.05, divided by the number of candidate profiles) at no more
    delivered tokens per query, and G1 with the profile enabled passes every target and
    guard that it passes without it.
- **Verification:** card rendering and the limit on a fixture per language; dimensions
  768/512/256/128 end to end (cache, generation geometry, frame size) with a wrong
  length refused; mixed retained 2048/768/256 rows open and account correctly; a v1
  profile is refused with its cache intact, and an interrupted cutover or publication
  restarts cleanly; a template change re-keys cards; the prefetched batch is dropped
  on pause, owner death and EOF and re-admitted only after a foreground query that
  arrived first; card acceptance refuses any tuple the body does not render; a run's
  cap accounting starts from the actual rows; a failed prefetch still publishes the
  committed batch; a partly consumed source renders only its selected cards; a split
  GGUF is refused before loading. Measured 2026-10-08
  ([validation](../../docs/validation.md)):
  - Batched and single-sequence vectors agree (EmbeddingGemma 2, min cosine 0.9999878).
  - Preparation counts and footprints on the library-only rust-lang/rust store are
    recorded. Its time is not accepted, because every attempt swapped on the 18 GiB
    host.

  Still in the [measurement handoff](../../docs/measurement-handoff.md): the installed
  worker links only system libraries and frameworks, and passes the isolation,
  resource and numerical checks on the installed artifact.

D001's model choice and chosen values were settled on 2026-10-03; T001–T003 were
accepted on 2026-10-05 under development isolation and measured on 2026-10-06 (see
validation). T004 (merged 2026-10-08) reopened the model choice through G2, which
selected EmbeddingGemma 2, and replaces the MLX worker. Signing and installed-package
acceptance remain open in the measurement handoff. The owned learning head (013,
frozen off) cannot substitute
for this proof.

### T005 — Diagnose and improve anchorless evidence retrieval

**Status:** authorized 2026-10-09 (owner: “go for 1 then 2 and onwards”); in progress, not accepted. T004's
selected Gemma profile and current implementation are the comparison baseline.
The original G2 verdict remains valid for its original question: gain over no profile.
This task asks which current losses can be repaired at the same delivery budget.

**Implementation in progress:** `src/retrieval_trace.rs` owns opt-in request-local
diagnostics, shared through `Control` by the ordinary selection/final-read paths.
`context --retrieval-trace-dir DIR` and `mcp --retrieval-trace-dir DIR` require a new
run directory; existing paths, including symlinks, are refused. Unix directories are
0700 and files are created exclusively as 0600. No automatic capture occurs.
Events contain source identities, candidate origin/rank, stale/filter/dedup/cap/cut
outcomes and packing inputs. Dense events identify the card input key. Context queries
are correlated by SHA-256; MCP records also retain their request ID. The actual
Foundry response is retained privately; it is not a host-consumption acknowledgment.

Limits per request: 8,192 events and 2 MiB of serialized event data, with `truncated`
set on overflow, and 4 MiB per persisted record. One owner writes at most 4,096
records / 256 MiB, including failed-write reservations. CLI diagnostic failures fail
the explicit measurement command before successful stdout. MCP diagnostic failures
emit `retrieval_trace_unavailable: run is incomplete` on stderr without changing an
already settled delivery. A missing, malformed, truncated or uncorrelated report
invalidates attribution; no missing event proves absence. Invalid requests rejected
before the delivery path still require the harness's request/refusal accounting.
Diagnostics are outside the evidence allowance but their I/O/latency is measured
separately. Their payloads can contain private source/memory; keep the directory
outside indexed source and Git, and explicitly remove it when its retention ends.

The private G1/G2 questions/results, prepared Gemma stores and macOS measurement host
are absent from this cloud workspace. That blocks full miss attribution and quality
acceptance. Local traced examples and fixture results must remain separately labelled;
they do not assign causes to the 249 historical G2 misses.

- **Depends:** T004 implementation and preserved G1/G2 apparatus from the
  [measurement handoff](../../docs/measurement-handoff.md). The diagnostic checkpoint
  below unlocks 001 T010; completion of one repair is not a prerequisite for starting
  the other. 003 T006 owns integrated real-host acceptance.
- **Scope:** opt-in bounded retrieval traces through `src/neural/{partition,query}.rs`,
  `src/store.rs`, `src/response.rs` and `src/mcp.rs`; measurement harness outside Git;
  regression fixtures in existing retrieval tests. Reuse ordinary source validation,
  deterministic ordering and the existing worker. No new model, service or mandatory
  reranker. Query traces are retrieval diagnostics, never automatic training records.
- **Diagnostic checkpoint:**
  - Pin binary, profile, source commits, source/index generations, prepared coverage,
    query set, harness, token cap and runtime configuration. Trace both successful and
    unsuccessful queries. Preserve the original response bytes when tracing is off;
    diagnostics stay off protocol stdout and outside the evidence/token allowance.
    Measure trace overhead separately from normal latency.
  - Trace unit identity and eligibility, candidate origin/rank, exclusion by
    freshness/deduplication/per-file/global caps, packed evidence/form and adapter
    delivery. Join expected answers offline in the evaluator; ground-truth labels
    must never affect candidate generation, ranking or packing. Persist only bounded,
    explicitly requested diagnostics in the private
    run directory. Never infer provider consumption from server delivery.
  - Account for all 388 queries and partition the 249 recorded misses by first
    observed loss. Retain `unknown` with its missing input where historical results
    cannot support attribution. A new traced run is a new run, not retroactive proof
    of the historical path. Report reproduction differences rather than forcing its
    counts to match. A claim that a card omits behavioral meaning requires a labelled
    example and a controlled representation comparison, not just low similarity.
  - Deliver stage counts, examples, current-baseline G1 wins/losses and one selected
    repair hypothesis. If dominant losses remain unexplained, improve diagnostics
    before selecting an embedding or ranking replacement.
- **Candidate checkpoint:**
  - Hold Gemma fixed and predeclare at most three variants targeting the demonstrated
    loss: representation, candidate coverage, ordering or packing. Keep other stages
    fixed for each ablation. If a source representation changes, update its recipe
    identity and test invalidation/reuse, interrupted preparation and retained cache
    safety; report preparation time, footprint and query cold/warm latency.
  - Reuse the existing G1/G2 files unchanged as regression/development evidence.
    Freeze fresh held-out anchorless questions and their required definitions before
    candidate tuning, with per-corpus results and the original leakage/answer-quality
    checks. Exposed old misses cannot serve as a fresh holdout. Keep labels private
    from implementation/tuning and do not repeatedly select against the final set.
  - Before evaluation freeze candidate count, sample-size rationale, paired statistic,
    multiplicity correction, per-corpus guards and latency/resource bounds. Proposed
    promotion requires a positive lower bound on paired delivered-evidence gain at
    95% confidence over current Gemma, at no more mean delivered tokens under the same
    cap, and all existing G1 gates. Publish absolute coverage, gained/lost task counts,
    refusals, missing/retried/duplicate tasks and uncertainty. Preserve frozen-run
    invalidity rules. If no candidate clears the gate, retain the baseline and record
    a no-promotion result instead of opening an unbounded model search.
  - A successful amendment names exactly which T004 representation/placement rule it
    replaces in this spec and the shared context contract; no parallel undocumented
    ranking path. Every newly lost current-baseline G1 case is reviewed and counted;
    passing historical guards alone does not erase those losses.
- **Verification:** fixtures establish trace attribution for absent candidates,
  stale units, rank/cap exclusion and packing omission; invalid traces cannot certify
  coverage. Test the selected repair's relevant source/budget/cache invariants and run
  the repository's required implementation checks when code changes. Linux fixtures
  do not substitute for the real Gemma/Metal measurements or signed installed checks.
  Record aggregate results in validation; raw labels, traces and source payloads stay
  outside Git. Validation records the executed diagnostic fixtures; the full G1/G2,
  held-out quality and installed-artifact checks are not claimed by those fixtures.

### T006 — Bound and parallelize semantic partition computation

Status: implemented experimentally in the shared CLI/MCP pipeline; sandbox evidence
is in [validation](../../docs/validation.md#large-repository-architecture-and-parallel-partitioning--2026-10-09).
Real-model throughput and task benefit remain unmeasured.

Selected 2026-10-09 for the experimental branch, following the owner's large-repository
architecture correction. Current `partition_page` tokenizes serially inside a redb
write transaction and, in MCP, the engine slot. This blocks unrelated source work
even though card computation only needs an immutable, verified source body.

Split partition acceptance into three ordinary operations: copy and hash-verify a
bounded source snapshot; compute card tuples outside all store transactions and the
MCP engine slot; reacquire the writer and compare the current source hash before
publication. A private prepared value binds the computed tuples to their verified
input. The public claim-validation API still renders the actual source and refuses
invented tuples. Source changes/deletion during computation refuse stale publication.

Share the pipeline between CLI and MCP. Keep at most eight source bodies (at most
16 MiB under the existing per-source bound) in one computation wave, and use at most
min(available CPU parallelism, eight) scoped workers. Commit in source order, one
atomic source mapping at a time. Check cancellation before work and before commits;
join workers on error/panic and surface a named failure. No unbounded channel, new
database, schema, model process or provider concurrency is introduced. MCP pause
and shutdown discard unpublished results when observed before commit admission.
Existing model/query admission stays intact.

Verify serial/parallel tuple equality, actual overlapping CPU work, bounded admission,
cancellation, source change/deletion before commit, restart reuse, panic propagation
and foreground engine-slot availability while card computation runs. Local fixtures
prove those contracts; they do not establish real-model throughput or task outcomes.
Full-source partition barriers, progressive ANN publication, larger device batches,
watching and LLM enrichment remain separate work in the revised ecosystem plan.

### T007 — Make semantic bootstrap useful before a complete corpus pass

Selected 2026-10-09; progressive scheduling/publication implemented experimentally,
with device tuning and real throughput still unverified. T006 removes CPU computation
from serving and writer locks. Replace the whole-source partition and initial
generation barriers with bounded waves of
partition/select/embed/commit and amortized searchable publication, preserving exact
source/input identity and resumability. Do not rebuild a full ANN generation after
every small batch; justify segmented or delta publication against the current USearch
generation owner before adding another persisted representation.

Prioritize explicitly requested areas and entry points, retain fair background
progress, and expose partial coverage. Keep lexical retrieval immediately usable.
Account independently for CPU preparation, provider/device work, cached reuse and
publication. Reuse CPU workers and buffers when their measured overhead warrants it;
keep queue bytes, token IDs, completed outputs and publication scratch bounded.

Tune model batches by tokens/sequence geometry and actual device capacity. Additional
model workers are not inferred from available CPU cores. Any batching change must
preserve single-versus-batched vector parity and foreground latency; any change to
vector meaning re-keys the descriptor/cache. Foreground/idle resource admission also
governs 010 generation, with explicit CPU/memory/device limits and hysteresis.

Verify that the first bounded wave is searchable while later sources remain
unprepared, edits invalidate immediately, restarts reuse accepted work, and background
progress neither starves queries nor spins while paused. Record real throughput when
the selected runtime/device is available; do not extrapolate a fake provider's speed.


T007 implementation (experimental): [prepare.rs](../../src/neural/prepare.rs) and
[driver.rs](../../src/neural/driver.rs) compose up to eight new partitions with
selection/embedding, alternate at most 16 priority cursors with a general cursor,
and keep one provider call plus one bounded prefetch. Priority revisits use the
cache or pending-key exclusion, not duplicate inference. No durable cursor is needed:
restart replays accepted partition/cache state. [index.rs](../../src/neural/index.rs)
walks prepared keys and compares current-partition count/source revision for complete
coverage, publishing first coverage and then geometric growth plus final scope.
Retaining the existing generation format avoids a second persisted owner, query
fusion and recovery protocol; bounded logical metadata/vector/serialization payloads
fail with `resource_limit` while accepted work stays. A full generation still needs
memory and rebuild time. Segmentation is not ruled out if later device/scale evidence
requires it. [Usage and accounting](../../docs/experimental-context.md#progressive-semantic-bootstrapping)
separate coverage from live/last-run stage counters. Existing token geometry, vector
meaning, model ownership and foreground hysteresis remain unchanged. Reusable CPU
pools, different model batches, device-pressure admission and 010 generation controls
need their own runtime evidence; this increment does not claim them completed.

### T006/T007 follow-up — Remove repeated preparation and startup work

Selected after the 2026-10-09 efficiency audit; first increment implemented
experimentally, with the remainder explicitly pending below. Keep the
implemented bounded waves and one resident model. Reuse bounded CPU workers and
verified intermediates across partition and batch selection: source bytes, parsed
ranges and token IDs must carry source/recipe identity, have a byte limit, and be
discardable after a wave. A corpus-wide AST/token cache is unnecessary. Avoid
repeated prefix tokenization when fitting an input; preserve exact rendered bytes
and token limits. Coordinate ingestion pipelining with 001 T009/T011.

Acquire the CLI model lazily when inference is actually needed. Avoid verifying the
same artifact twice within one acquisition by passing a private verified handle
through the same trust boundary; path or mtime equality alone is insufficient.
Do not remove worker-boundary verification or allow substitution between checking
and opening. Maintain cache-byte totals transactionally; full reconciliation remains
available for repair/upgrade rather than running on every ordinary warm replay.
Cancellation, pause, panic and shutdown must reclaim bounded workers/buffers and
leave accepted work reusable. Verify work counts and vector/input equivalence in
sandbox fixtures; real-device speed remains a separate claim.

Implemented increment in [prepare.rs](../../src/neural/prepare.rs),
[driver.rs](../../src/neural/driver.rs) and 001's `replace_source`:

- A lazy Rayon 1.12.0 pool reuses at most `min(available_parallelism, 8)` CPU workers
  for a CLI run and across MCP prepare/pause/resume runs for the owner. Rayon already
  exists in Cargo.lock through maintained dependencies; using it directly avoids
  a custom worker/channel/shutdown implementation. 001's subsequent ingestion
  increment makes it available in lexical-only builds as well. Each
  wave joins before returning, preserves input order and reports tokenizer panics
  without poisoning subsequent work. Empty/current waves start no CPU threads.
- Fresh verified partitions retain exact cards for selection, capped at 2 MiB per
  source (16 MiB per eight-source wave), including ids, key strings and array capacity.
  This is a retained-payload bound, not a process RAM limit. Sources that exceed it
  discard retained cards and use the existing on-demand rendering path. Retained
  cards are private in-memory values bound to source hash, recipe and function;
  both paths compare every selected range/key to its accepted partition. No new
  persisted cache, recipe, schema or vector meaning is introduced. Input reconstruction,
  parsing and tokenization need not repeat for a retained source in that wave.
- CLI acquisition occurs only after selecting missing input and checking the run
  budget/reserve. Cache-only replay, mapping-only republishing and zero-card sources
  need no model. Startup uses the existing run control, and admission rechecks the
  reserve afterward. A startup refusal/timeout can leave verified partitions and
  prior cached coverage published; it cannot commit vectors from a refused worker.
  Provider observations change only when acquisition is actually attempted; a
  cache-only run preserves the previous observation. MCP's query runtime remains
  resident under its existing ownership rule.
- Source replacement checks the current hash before allocating copied storage
  chunks, preserving the transaction and unchanged-source result.

001's subsequent E1 increment implements bounded initial ingestion and reusable
lexical workers; see its [contract](../001-source-state-recovery/contracts/context-v2.md#parallel-indexing).
The remaining E1 work now implements verified artifact handoff, transactional cache
accounting and per-card fitting:

- `VerifiedArtifacts` retains verified open regular files and the tokenizer bytes read
  while hashing. Acquisition consumes the same receipt; descriptor/path identity and
  descriptor metadata are checked before use and after model load/inference. The macOS
  worker inherits the verified GGUF descriptor and loads `/dev/fd`, avoiding pathname
  substitution. This is not an immutable filesystem snapshot. Inventory is bounded at
  64 files and tokenizer bytes at 64 MiB. Worker-executable verification remains.
- Versioned cache row/byte totals update in the vector transaction. Warm begin/finalize
  reads these totals; older writers remove the new accounting marker when rewriting
  state, triggering one reconciliation. Explicit repair can recount. Stale finalizers
  cannot overwrite newer totals; payload validation still occurs on vector use.
- Each card owns a bounded memo of identical normalized segments (128 KiB logical
  payload, 1,024 entries). Every complete candidate still passes through normalization,
  added-token extraction, pretokenization and postprocessing, preserving first-overflow
  fitting even when prefix lengths are nonmonotone. Overflow uses ordinary tokenization;
  BPE dropout disables memoization. No recipe/input-key change or persistent token cache.

Upstream inspected: tokenizers **0.23.2**, `tokenizer/mod.rs`
`encode_single_sequence`, `do_tokenize`, `post_process`; `added_vocabulary.rs`
`extract_and_normalize`; byte-level test `offsets_when_char_split_up` and BPE test
`test_tokenize_with_and_without_dropout`. T006/T007 reuse that public pipeline;
only bounded per-card deterministic segment memoization differs. Sandbox tests compare
exact IDs, offsets, spans and keys, including Unicode and overflow. The macOS descriptor
handoff has not been executed on this Linux sandbox; real-device throughput is unverified.
T008 E2 is implemented below; E3 delta publication, T009 and 010 generation remain
separate pending increments. See
[validation](../../docs/validation.md#bootstrap-work-reuse--2026-10-09) for sandbox
evidence; no real-device throughput or task-quality gain is claimed.

### T008 — Maintain and publish only changed semantic work

2026-10-10 shared-resource amendment: opt-in owner memory admission reserves the embedding
worker ceiling before loading and retains it through provider shutdown/reaping. The same
owner defers background embedding batches under observed memory pressure while preserving
foreground query access. Generation shares this reservation sum and can unload idle residency.
The implementation, limitations and verification belong to
[010 T002 shared admission](../010-source-bound-derived-knowledge/spec.md#t002-shared-memory-admission--2026-10-10).
Default serving/model/cache identity and existing per-worker controls are unchanged.

Selected; E2/E3 maintenance, ANN deltas, affected-location reconciliation and verified/
resident segment reuse are implemented experimentally. Generation v5 adds bounded
location overlays, shared mapping snapshots and clean-restart publication reuse. Depends on
001 T011's source acceptance seam and T006/T007's bounded preparation. Current input-key cache reuse is retained. Replace
normal edit-time full walks and generation rebuilds with work proportional to the
changed inputs and their locations. Full reconciliation is still valid for initial
bootstrap, profile changes, repair and missed filesystem notifications.

- **Durable obligations:** source replacement/deletion records a coalesced current
  version or tombstone in the same transaction as source publication. Semantic and
  010 generation completion are independent of lexical `PENDING` acknowledgment.
  A completion clears only the version/recipe it processed; late results cannot
  erase newer work. Bound queues and use a durable reconcile-required marker on
  overflow. Use redb and the existing owner, not a separate broker, journal or
  generic job service. A versioned schema change, if needed, preserves existing
  source, user memory, briefs and reusable caches through upgrade/restart.
- **Lifecycle:** watching alone still starts no model. Add explicit continuous
  maintenance enablement with a selected profile and resource limits. When enabled,
  wake the existing preparation owner on changed work and wait when caught up;
  retain one-shot prepare and explicit pause. Startup resumes only under that
  explicit mode. This amends T003's stopped-driver behavior for the new mode only.
  Continuous source traffic must allow bounded semantic progress without delaying
  source invalidation or foreground queries. Status polling must not perpetually
  impose the two-input/60-second foreground throttle; observation still uses bounded
  serving resources. This follow-up amends T003 D5, not its query-priority guarantee.
- **Publication:** separate vector membership from source/location mapping. A
  mapping-only change must not reconstruct unchanged ANN data. Select the simplest
  restart-safe delta design over the existing cache and USearch: evaluate a single
  immutable base plus a bounded delta and tombstones against in-place USearch edits
  with safe checkpoints. Adding/removing an in-memory vector alone does not avoid
  full checkpoint writes or solve atomic publication. Bound query fan-out, delta
  bytes and compaction scratch; avoid an unbounded segment collection. Preserve
  current-source eligibility, duplicate suppression, profile identity and fresh-hit
  recall when stale base entries occupy the candidate window. Publish manifest,
  vectors and mappings atomically under a generation identity; interrupted writes
  remain ineligible. Cache commits precede publication. Failed compaction retains a
  readable generation, durable pending work and a visible bounded-backoff state.
- **Status:** maintain versioned committed coverage/progress counters with cheap
  reads and explicit stale/unknown states. Keep reconciliation/audit available;
  replace neither payload validation on use nor corruption reporting with optimistic
  counters. Progress polling must not hash an entire index or census every source
  and cache row. Derive counters from the same accepted transitions as work state.
- **Verification:** unchanged replay makes zero document calls; an ordinary edit
  visits only affected derivation work and reuses unchanged input keys. Cover
  delete/rename/revert, duplicate hints, edit during inference, coalescing, overflow,
  pause, owner exit and restart at source/cache/publication/compaction boundaries.
  Below compaction thresholds a small edit performs no full ANN rebuild. Assert
  mapping-only publication, stale candidate suppression, bounded query merge,
  convergence under sustained edits, cheap status and foreground service. Fixtures
  establish work selection and recovery, not real-device throughput.

Implemented E2 in [work.rs](../../src/neural/work.rs), [driver.rs](../../src/neural/driver.rs),
[status.rs](../../src/neural/status.rs) and the existing source/MCP owner:

- Schema 9 adds two domain-specific redb tables, `semantic_work` and `knowledge_work`.
  One shared queue would let one consumer erase or crowd out the other's obligations;
  each table owns a metadata row, at most 1,024 paths and 1 MiB of queued path text
  plus one bounded rotation cursor. Updates,
  tombstones and overflow markers commit with source/chunk/revision publication,
  including scan sweeps. There is no new broker, service, model thread or dependency.
- Overflow clears the bounded lane and advances its reconciliation epoch; subsequent
  changes continue to be recorded. A baseline completion clears only its epoch and
  versions at or before its start revision. Ordinary batches select eight exact paths
  with a persisted rotating cursor, so repeatedly edited paths cannot starve the tail.
  Acknowledgment requires the same owner, lane, epoch, function/recipe and exact version;
  it follows successful cache work and publication. Source/lexical acknowledgment
  never clears derivation obligations. Upgrade initializes reconciliation and preserves
  source, briefs, memory, task, graph, semantic cache and learning data.
- `index {semantic:"maintain"}` enables the existing driver for this owner. Explicit
  `mcp --semantic-maintain --semantic-profile FILE` starts it at launch; `--watch` is
  separately needed for automatic filesystem acceptance. One-shot `prepare` remains
  an explicit full reconciliation and persists that obligation before publication.
  `pause`, shutdown, provider errors and cache limits retain unfinished work; named
  failures require explicit resume, with no hidden provider retry. Quiet mode waits
  on committed-source notifications without body/cache/index polling; pause/shutdown
  is observed within the 250 ms notification wait, apart from existing in-flight work.
- Source work gets eight owner steps before a requested semantic step may reserve
  the slot briefly (100 ms reservation ceiling); foreground requests still go first.
  Existing CPU, cache, profile batch and document-call limits apply. Continuous mode
  does not grant new filesystem roots or model download/execution permission.
- Publication records versioned coverage from its existing mapping walk; no extra
  census is added. Ordinary CLI/MCP status reads transactional cache totals, pending
  metadata and the last recorded publication. Source/profile mismatch yields stale
  or unknown counters (`null`), with the prior observation labeled separately.
  `index.available` is unknown and `validation:not_checked`; this observation does
  not certify on-disk integrity or claim live cache census totals. Use
  `semantic status --audit` or MCP `status {semantic_audit:true}` for the bounded audit.
  Status still obeys serving admission but does not renew the two-input/60-second
  foreground throttle. This amends T001 readiness, T003 D4/D5 and startup behavior
  only for the explicitly selected mode.

Implemented E3 ANN publication in [publication.rs](../../src/neural/index/publication.rs)
and [query.rs](../../src/neural/query.rs), shared by CLI preparation and the existing
MCP driver:

- Generation **v5** separates current input locations from immutable base
  membership. There is one USearch F16/cosine base and at most one F16/cosine delta.
  Missing active base keys are tombstones; a reverted base key reactivates its existing
  vector. Duplicate source locations share one input vector. Location-only publication
  decodes no cached vectors and builds/writes no ANN bytes. A changed overlay decodes
  and rebuilds only its bounded active keys; explicit repair still compacts from cache.
- Compact when either delta membership or retired base membership exceeds
  `clamp(base_count / 4, 64, 4096)`, or delta f32 payload would exceed 32 MiB. The delta
  serialized file also has a 32 MiB hard bound. Compaction retains the existing 2 GiB
  logical f32 payload, 4 GiB serialized base and 256 MiB per-metadata-file bounds.
  These are payload limits, not total RSS or a measured device throughput guarantee.
- One manifest atomically references content-named base, delta and location files.
  Private-stage files are synced, promoted and directory-synced before the manifest
  rename; the manifest directory is synced afterwards. All operations use held
  descriptors with no symlink traversal. Failed compaction leaves the prior manifest
  and referenced artifacts readable. Next staging reclaims only owned unreferenced
  content files and abandoned private stages; current artifacts remain. This replaces
  the former three-file overwrite that could destroy the last readable pair.
- Cache commits still precede publication; E2 acknowledgment follows publication.
  Crashes before the manifest switch leave the prior generation; crashes after it
  leave the complete new generation and pending work that can replay without inference.
  The MCP owner retries publication I/O/index failures after 250 ms, 1 s and 3 s,
  exposing `publication_backoff_ms`, retry count and last publication error in progress.
  Backoff releases the serving slot, observes pause/shutdown and admits no model work.
  Exhaustion pauses by name with pending work intact; finalization does not retry again.
  CLI publication retains its existing bounded-command, named-failure behavior.
- Query fan-out is at most two USearch filtered searches and 128 candidates before
  merging the final 64. Segment ordinals select immutable input keys; a persistent
  ordered map checks active membership and supplies locations in O(log n).
  Tombstones are excluded inside ANN selection, before the candidate window is limited;
  final source-hash/range eligibility remains unchanged. Segment memberships are
  disjoint and validated, so shared inputs cannot appear twice. Approximate ANN search
  remains approximate; the stale-neighbor fixture establishes suppression behavior,
  not general recall or task quality.
- In-place USearch edits were rejected for this slice because a restart-safe checkpoint
  would still serialize the entire base or require a separate mutation journal. The
  bounded overlay reuses the maintained library's serialization, redb's authoritative
  cache and the existing preparation owner. No new table, broker or thread.
  Format v1–v4 generations are refused by name and republished from cache by preparation
  or repair; schema 9, partition identities and vector cache format do not change.

Implemented warm publication reuse in [mapping.rs](../../src/neural/index/mapping.rs)
and [verification.rs](../../src/neural/index/verification.rs), shared by the CLI/owner:

- An owner-local mapping view retains current partitions, input-key-to-path reverse
  references, cached locations and coverage counters. Source replace/delete/sweep,
  partition acceptance and cache commits record affected paths/keys under the same
  short mutex as the redb commit. A walk captures that commit serial; edits during
  paged work remain dirty, and an old completion cannot clear newer work or claim
  complete coverage. Cache completion refreshes all locations sharing that input key,
  including when no source revision changed. Unchanged scans do not dirty the view.
- The disposable frontier holds at most 1,024 combined paths/keys and 1 MiB of their
  text. Overflow drops the view and requires reconciliation. The retained view has a
  conservative 256 MiB logical-payload bound (not allocator overhead or total RSS).
  Refresh is paged through the existing owner steps. Restart, cancellation of a taken
  view, identity change, purge and explicit mapping audit/repair can rebuild it from
  redb when authoritative inputs changed. No new durable journal, schema, table or
  thread is added: the schema-9 queues preserve obligations. Clean restart publication
  can reuse its revision fence, as described below.
- Publication and serving share at most two verified ANN-file receipts per Engine.
  A receipt retains the verified file descriptor and checks device/inode, length,
  mtime and ctime (including nanoseconds), against both the held inode and the current
  descriptor-relative no-follow file open. New/changed files are bounded and hashed;
  before/after metadata must agree before retaining the receipt. Unchanged files skip
  SHA work. This is trusted-local-writer integrity reuse, not authentication or a
  defense against an actor able to forge filesystem metadata. Explicit audits always
  hash independently; restart receives no inherited receipt. Cache-lock waiting obeys
  caller cancellation/deadlines.
- Serving retains immutable `Arc<usearch::Index>` segments in those two receipts.
  A new delta restores only that segment; location-only publication or base reversion
  restores no ANN data. Header/geometry validation remains mandatory for new buffers;
  reused segments still check dimensions, count, scalar and metric against the new
  manifest. Generations share immutable membership and location snapshots; no full
  active-ordinal translation array is rebuilt.
  In-flight windows retain their original immutable generation. Every warm request
  checks Engine lifetime, prepared identity and the held manifest version; manifest
  replacement or profile changes refresh/refuse even without an invalidation hint.
  Once loaded, an unchanged generation serves its verified in-memory copy; segment
  file versions are rechecked on generation refresh, not on every warm query.

Implemented compact metadata in [locations.rs](../../src/neural/index/locations.rs),
[publication.rs](../../src/neural/index/publication.rs), and [cache.rs](../../src/neural/cache.rs):

- Generation v5 moves base membership into one immutable `members-<sha>.json` artifact.
  The manifest retains counts, artifact hashes, coverage/revision and bounded ANN-delta
  keys. A location base has one replacement/tombstone overlay, at most 4,096 changed
  keys and 32 MiB serialized. Crossing either limit compacts locations independently;
  unchanged ANN base, ANN delta and membership remain untouched. ANN compaction also
  compacts locations. The existing atomic manifest switch and closed-set cleanup own
  all artifacts. No additional publication journal or database checkpoint is introduced.
- The disposable mapping view, published snapshot and serving generations share
  `imbl::OrdMap<String, Arc<Vec<UnitLocation>>>`. Clone shares tree nodes; updates copy
  affected tree paths and changed location values. Structural diff skips unchanged
  shared nodes. The owner remembers a published root only after successful publication;
  failures leave differences discoverable on retry without another acknowledgment log.
  Full reconciliation can create an independently allocated tree and thus needs a full
  comparison. Cold load and location compaction also retain corpus-sized work.
- Held-file receipts cache decoded membership, location base and the single overlay.
  New or changed bytes are bounded, hashed and checked for sorted unique keys, valid
  units, disjoint segment membership and active counts. Only new overlays are read on
  ordinary refresh; the base membership join is reused for the same immutable pair.
  Retention is one artifact per metadata role, one active snapshot, and a bounded set
  of non-base keys; old request windows retain their own snapshots. As with ANN receipts,
  audits always verify independently and receipts do not survive restart.
- Every source/partition/cache mapping commit advances `semantic_mapping_revision` in
  the same redb transaction, including lexical-only builds. This is an additive key in
  existing META, with absent value zero and checked overflow; schema remains 9. A stable
  mapping walk records that revision plus coverage counters in the manifest. Publication
  first verifies the generation and compares its durable revision and source revision;
  an unchanged reopened owner can return Current without partition/cache-layout scans.
  Partition/cache changes invalidate reuse even without a source revision change.
  Changed input after restart still requires one paged baseline; explicit repair/audit
  deliberately reconciles. Preparation's source/partition discovery remains separate.
  This is publication reuse, not zero-I/O restart or automatically resumed inference.

**Implemented cold/restart follow-up (2026-10-10):** schema 12 adds a disposable
redb publication lookup (partition rows and input-key-to-path multimap) and a bounded
changed-input frontier. The current failure is a corpus-wide partition/cache census
after a small offline edit. An in-memory view alone cannot survive owner restart;
reusing the derivation queue would lose cache/partition-only changes and would couple
independent acknowledgments. These three tables reuse redb transactions, with no new
worker, broker or log format. Maintenance is explicit schema upgrade, bounded
invalidation, revision-matched checkpoint reuse and full reconciliation on mismatch.
The lookup is an optimization, never source authority or a persisted file-verification
receipt. Initial reconciliation still builds it; ordinary refresh changes only affected
rows and reverse references. Cold generation bytes must still verify independently.

The frontier holds at most 1,024 paths/keys and 1 MiB of key text. It records source,
partition and cache-only changes under the mapping revision even without the semantic
feature. Before the first reusable checkpoint, changes advance the reconciliation floor
without recording unusable work. A stable mapping walk commits its lookup and counters
before manifest publication;
a restart between those commits refuses the mismatch and reconciles. A matching restart
loads only affected lookup rows/reverse postings, then revalidates current source/partition/
cache metadata. Partial coverage and zero-card sources retain exact counts. Explicit
purge removes the lookup. If a concurrent edit makes a retained full view unstable, the
next stable checkpoint includes all of that view's uncheckpointed rows. The initial/full checkpoint is a bounded redb transaction;
its write cost still scales with the admitted mapping and is not a constant-time claim.

Cache-only CLI preparation now verifies only the tokenizer. A private tokenizer receipt
can be promoted to a full artifact receipt only by verifying every remaining pinned file;
changed descriptors/tokenizer paths refuse promotion. Missing weights are therefore allowed
for cached work and still fail before acquisition when inference is required. Completed
partition metadata can survive refused/cancelled model verification. Worker executable and
worker-boundary checks remain. A single-file check runs on the caller without extra threads.
Explicit `repair-index` can reconstruct even missing disposable lookup tables; ordinary
open never silently invents them. Source/cache/partition authority remains unchanged.

Cold reconciliation shares one source/partition/cache read snapshot per 128-path page,
with a bounded 1,024-key probe memo. Artifact acquisition hashes independent opened files
with at most `min(available_parallelism, 4)` scoped workers and one MiB scratch each,
retaining the existing 64-file/64-MiB-tokenizer limits. All workers join before receipts
escape; descriptor/path versions are rechecked. A single large GGUF still needs its full
sequential SHA-256. ANN loading hashes each MiB while filling the exact restore buffer,
avoiding a second uninterruptible whole-buffer hash pass. No new backend or hash identity.

**Remaining costs:** initial bootstrap/checkpoint construction, cold artifact verification,
overflow or unmatched-checkpoint reconciliation, explicit repair, location rebasing and
ANN compaction still scale with
corpus size. Overlay comparison/serialization and ANN delta rebuilds are bounded, not
strictly proportional to the latest edit. Candidate eligibility now uses ordered-map
lookup rather than an ordinal boolean array. E4 model-aware inputs remain separate.
No 80,000-file throughput, real-model cost or task-quality result is claimed.

Reference inspected, not executed upstream: imbl **7.0.2**, Cargo.lock archive SHA-256
`46bad832b9b463ed9398b8506488cc2e3b897a9d44f13af115f382aac71f4fec`,
[`src/ord/map.rs`](https://docs.rs/imbl/7.0.2/src/imbl/ord/map.rs.html),
`OrdMap::{clone,diff}`, `PartialEq`, and tests `diff_clone_derived_shared_structure` /
`diff_clone_derived_narrow_keys`. The published archive records source commit
`834c67f1e08c63a4d662a4132425026a4cea7b25` with a dirty flag; the archive pin is the
inspected authority. T008 uses its ordinary persistent tree for disposable in-memory
snapshots only. Foundry owns durable JSON, integrity, bounded overlays and atomic
publication. This dependency avoids a custom persistent map or a second dirty-key log.

Reference inspected, not executed: USearch **2.26.2** (the Cargo.lock version),
[`rust/lib.rs`](https://github.com/unum-cloud/usearch/blob/v2.26.2/rust/lib.rs)
`Index::{add,remove,filtered_search,save_to_buffer,restore_from_buffer,metric_kind,scalar_kind}`, tests
`add_remove_vector`, `search_with_stateless_filter` and `search_with_stateful_filter`.
Inspected at the pinned Cargo.lock release, not executed upstream. These operations
inform T008: Foundry supplies source-bound locations, validated disjoint membership,
atomic publication and bounded compaction; USearch supplies ANN storage and search.

### T009 — Use model-owned geometry and contextual representations

Implemented experimentally, 2026-10-10; real-device acceptance remains open. The exact publisher contracts and their limits
are in the [model-window audit](../../docs/context-ecosystem-plan.md#model-windows-and-chunk-representations).
The selected EmbeddingGemma 2 advertises 8,192 tokens, while the legacy worker geometry has
a 2,048-token context/batch limit and the address recipe defaults to 128 tokens.
Raising the card limit does not add multiline function bodies to that recipe.

- Version provider capabilities independently: per-input tokens, total batch tokens,
  sequence count, dimensions, tokenizer/template identity, pooling and normalization.
  Validate actual backend support and retain hard protocol byte/memory ceilings;
  reject oversize inputs explicitly rather than silently truncating. Use actual
  token lengths for bounded packing, with length bucketing only where it helps.
  Preserve foreground latency and single-versus-batched vector parity at every
  supported geometry. Thread counts, kernels and KV precision are backend-specific
  experiments; additional CPU cores do not authorize extra resident GPU models.
- Keep address cards as a reproducible baseline. Add an explicit body-aware recipe
  for short complete units and bounded windows for oversized units, with surrounding
  context and exact cited span maps. Select parent/file/area inputs when useful;
  deduplicate representations by evidence identity and limit their candidate shares.
  Long embedding inputs and small delivered evidence are independent choices.
  Unsupported syntax, opaque names, configuration and tests need explicit coverage.
- Bind reuse to all input and model properties that change vector meaning. Separating
  path/line locations from vector content is a recipe change, not permission to
  reuse vectors for different text. Parent context increases the invalidation domain.
  New profiles/recipes coexist with the old cache namespace until an explicit
  migration or purge; never mix vector spaces in an ANN generation. Enforce truthful
  partial coverage while new representations populate.
- Late chunking is a candidate, not part of the current pooled-vector protocol. It
  requires contextual token states and correct span pooling/projection/normalization;
  averaging existing chunk vectors does not implement it. Compare its recomputation
  and memory costs against independent body units before committing to that backend.
- Verify body-only edits, small versus oversized units, context-only edits, rename
  mapping, model/recipe migration, limits, batch parity and recoverable exact spans.
  Use sandbox fixtures first. Actual long-context quality, device throughput and
  task economics need the real model/runtime. Publisher window size alone proves
  none of them. A hosted adapter requires explicit endpoint/data scope; this task
  does not select a paid endpoint, download weights or send repository source.


#### T009 implementation — 2026-10-10

[Provider geometry](../../src/neural/provider.rs) is an optional strict `geometry`
object inside descriptor v2: `v:1`, `input_tokens`, `batch_tokens`, `sequences`.
It requires `1 <= input_tokens <= batch_tokens <= 32768` and 1–32 sequences. The
32,768-token value is a hard admission ceiling, not a model capability. Omission
preserves the legacy serialized descriptor, digest, adapter revision 1 and 2,048-token
input/call limits. Explicit geometry requires real-adapter revision 2 and participates
in vector identity because a changed context configuration can affect computation.
Dimensions, tokenizer, templates, pooling and normalization retain their existing
identities. IPC remains v2 with independently versioned geometry and unchanged byte
caps; older strict workers refuse the new field instead of silently ignoring it.

The [real worker](../../src/bin/foundry_embed.rs) checks explicit per-input capacity
against the loaded GGUF training context, sets unified `n_ctx = n_batch = n_ubatch`
to the call total and checks actual context, per-sequence context, batch, microbatch
and sequence capacity before Ready. No per-input context extrapolation is selected. F32 KV,
flash-attention-off and length-grouped sequence construction remain unchanged.
Profile `card_tokens` bounds individual documents; profile `batch` bounds sequences.
[CLI preparation](../../src/neural/prepare.rs) and the [MCP driver](../../src/neural/driver.rs)
pack ordered prefixes by actual token totals, retaining unadmitted inputs. Foreground
sequence limits and one resident model slot remain. A batch need not satisfy the old
`batch * card_tokens` worst-case restriction. Query limits come from the same descriptor;
all oversize inputs refuse, with no token-ID truncation.

The explicit profile setting `representation: {"kind":"body-v1","parent_tokens":64}`
selects [body windows](../../src/neural/partition/body.rs). Omission preserves cards-v1.
The recipe keeps complete short functions/methods/sections; containers contribute
residual source ranges and child units. Oversized regions split into non-overlapping,
UTF-8-safe windows, preferring line boundaries. File preamble, unparsed/unsupported
syntax, configuration and tests use the same source-window fallback. Whitespace-only
ranges produce no input. Each child window includes a bounded nearest-parent/own-header
or file-preamble prefix; it is not a generated summary or an independent parent ANN
candidate. Thus this increment introduces no duplicate parent evidence or competing
parent candidate quota. Independent file/area representations remain selected future
work with 010 T002; they still require deduplication and candidate-share limits.

`parent_tokens` is at most one quarter of `card_tokens` (zero disables context).
The complete rendered input, including template, context, body and special tokens,
must fit `card_tokens`. Tokenizer offsets suggest cuts; exact candidate re-encoding
verifies every window without assuming prefix-monotonic token counts. A template
that leaves no space for one source character refuses. Bounds are 2 MiB source,
16,384 windows per source and 16 MiB accumulated context text in a source plan;
context construction is byte-bounded before tokenization. These are payload bounds,
not peak RSS: parser/tokenizer scratch, span arrays and allocator overhead remain.
Fresh cards share the existing 2 MiB-per-source retained handoff; overflow falls back
to one lazy source plan and selected-card rendering, preserving exact input parity.

Input keys bind the full rendered text and function digest. Paths/offsets stay in
provenance, so a same-language rename reuses vectors; a body edit or included parent
context edit changes affected keys. Context outside the bounded prefix is not encoded.
Recipe identity binds syntax schema, body version, tokenizer and both token settings.
Profile switches rebuild partitions/publication without purging old vector rows;
reversion reuses them. Existing partial coverage and freshness checks remain authoritative.
No new table, worker, dependency, endpoint or representation-specific index is added.
Long embedding inputs do not increase MCP evidence allowances. Late chunking still
requires a different backend exposing contextual token states and is not implemented.

Upstream reference for this task: llama.cpp
[`b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea`](https://github.com/ggml-org/llama.cpp/tree/b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea),
`include/llama.h` (`llama_model_n_ctx_train`, `llama_n_ctx_seq`, context parameters)
and `examples/embedding/embedding.cpp` (unified context and noncausal microbatch
capacity). Its `tests/test-backend-ops.cpp` contains `test_rope` and `test_mean` backend
operator checks; these were inspected as references, not executed or treated as
end-to-end vector parity evidence. Foundry intentionally keeps its own tokenizer,
explicit geometry, strict limits and existing pooling/normalization. The model-window
audit above retains publisher revisions. Foundry's ignored real-consumer
`dev_real_worker_batched_and_single_sequence_vectors_agree` now exercises the selected
profile's input limit and heterogeneous batches; it remains unexecuted for explicit
geometry on this Linux sandbox. See [executed fixture evidence](../../docs/validation.md#model-aware-bodyparent-inputs--2026-10-10).

The macOS consumer harness checks finite, unit-normalized vectors as well as
cosine agreement, prints actual descriptor/document geometry, and repeats a query at
the full input limit. The eight-sequence workload explicitly requires a profile that
admits eight sequences instead of silently skipping that coverage. A short card window
does not certify long-document batching. This macOS-only harness is neither target-typechecked
nor executed here: the selected worker requires a macOS App Sandbox bundle,
the selected embedding artifacts and Metal hardware. This Linux CPU sandbox has no GPU
device. Selected embedding weights were subsequently supplied for the test-only CPU
diagnostic below; that result does not accept the production macOS worker. Generation's
real Qwen CPU results are separate evidence; fake embedding transport/lifecycle tests
cannot establish embedding numerical quality.


T006/T008 ingestion overlap repair (2026-10-10): four-file transaction gathering must
not delay an available prefix behind a still-running read. The existing ordered
read/read-writer overlap test exposed that regression. Ingestion now blocks only when
it has no accepted prefix, then gathers already-ready neighbors up to four; a pending
next receiver keeps its original slot/order for the next turn. Source descriptors and
final identity checks, the eight-read/result ceiling, cancellation accounting and
periodic lexical publication remain. No queue, worker or store contract is added.
The original synchronization test remains unchanged. Groups stop at the exact lexical
publication boundary. A direct four-source transaction test preserves rollback coverage;
the scan fixture permits readiness-dependent successful grouping. Prior cold-bootstrap
timing is historical; mixed-size throughput still needs a workload comparison.


Linux ecosystem diagnostics continuation (2026-10-10):

- **T005:** `tests/retrieval_attribution.rs` validates query/source/delivery identity
  and reports observed candidate/filter/freshness/packing losses. Truncated, conflicting
  or missing evidence remains unknown. Eight frozen public Foundry development labels
  were run with baseline and behavioral lexical retrieval at 2,048 tokens: each arm
  delivered 1/8; three candidate-limit losses, two packing omissions and two unknowns.
  Tracing preserved identical delivery. This neither improves acceptance nor attributes
  the historical 249 G2 misses; preserved G2 inputs and fresh held-out repair checks remain.
- **T006/T008:** an opt-in mixed-size 8,192-file fixture runs production body/parent
  fitting, cache and ANN publication with a byte-tokenizer fixture and fake vectors.
  Cold publication completed; unchanged restart did zero partitioning/model work, one
  edit plus deletion embedded one new unit, and revert reused cached vectors. This is
  storage/lifecycle scale evidence, not selected-model tokenization or inference throughput.
- **T009:** the embedding binary's shared numerical implementation has an explicitly
  test-only Linux CPU diagnostic using selected Gemma BF16 weights and the pinned native
  runtime. Eight-sequence single/batch/reverse/mixed-length/full-window comparisons
  passed at 2,048 tokens (minimum cosine 0.99999528). At 8,192 tokens the pinned
  `llama_context::output_reserve` requests an 8,216 MiB logits/embedding buffer and
  fails under the tested 8 GiB address-space ceiling. `decode` requires all tokens for
  pooled embeddings, so clearing output flags is not a compatible repair. Runtime
  output allocation needs a separately pinned, parity-validated fix or a justified
  larger resource envelope; a larger model window alone does not establish usable
  geometry. The diagnostic uses GGUF tokenization, not profile tokenizer.json parity.
  It does not admit a Linux production embedding worker. macOS/Metal, signed bundle,
  worker IPC/isolation acceptance and real long-context retrieval quality remain open.

Native reference: llama.cpp `b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea`,
`src/llama-context.cpp::{decode,output_reserve}`, and Foundry's ignored
`cpu_pinned_gemma_batch_geometry` diagnostic. No upstream runtime patch or descriptor
identity change is hidden in these measurements. Raw logs/artifacts remain outside Git;
[validation](../../docs/validation.md) records reproduction and limitations.
