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
These are local capabilities, not measured host outcomes. The subsequent efficiency
review below supersedes the earlier immediate jump from progressive bootstrap to
broad enrichment: cold throughput, changed-work maintenance and model-aware inputs
are the next implementation priorities. The first E1 increment now reuses semantic
CPU workers and bounded verified cards, defers CLI model acquisition until missing
inputs exist, and avoids chunk allocation for unchanged sources. The next increment
adds bounded read/hash overlap, ordered freshness-checked source acceptance, early
lexical publication and reusable ingestion/lexical workers. Verified-artifact handoff,
transactional cache accounting and exact per-card segment reuse now complete the selected
E1 implementation. E2 adds durable changed-source queues, opt-in continuous maintenance
and cheap progress observations. E3 now publishes bounded ANN deltas and independent
location maps through an atomic manifest, refreshes affected locations, and reuses
verified/resident segments. Generation v5 adds bounded location overlays, shared mapping
and metadata snapshots, and clean-restart publication reuse. Schema 12 also makes
matching changed-input restarts incremental; cold verification, unmatched/overflow
reconciliation and compaction remain corpus-sized. E4 now adds explicit model geometry, exact
body windows with bounded parent context, and actual-token batching. Existing address
profiles/caches remain usable. Real-model long-context parity, quality and throughput
remain unaccepted across devices. The normal Linux CPU worker now passes 8,192-token
inference under 8 GiB, and opt-in `hierarchy-v1` adds independent parent/file/area
vectors. See [current validation](validation.md#retrieval-scale-and-host-economics--2026-10-11).
F now adds explicit selective HTTP generation, exact completion reuse, source-bound
area summaries and independent knowledge acknowledgment in schema 10. Unchanged nodes
skip source-body reads and model calls; changed children invalidate their ancestors.
Opt-in continuous MCP maintenance, shared client admission and bounded derived context
are now implemented. Owned local generation adds pinned artifact reuse, idle unload and
per-worker CPU/RAM/Metal controls. Shared memory and Linux affinity/cgroup CPU admission are implemented; native macOS CPU/global GPU pressure policies, Metal hardware
acceptance and real-model task utility remain open. See [usage](experimental-context.md#owned-local-generation).

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

Updated on `experimental/context-ecosystem`, 2026-10-11. The experimental pipeline
exists end to end, but general retrieval quality, representative task completion and
total token economics are **not solved**. Implemented does not mean enabled by
default, accepted on every platform or ready for release. The
[latest Linux checks](validation.md#ecosystem-code-improvements--2026-10-11)
record real model/runtime evidence and preserve negative host outcomes.

The code continuation adds explicit-language/Unicode prose repair (Go **5/8**, versus
the preceding 1/8, with Foundry/Rust delivery retained), opt-in `overview-v1` file
preparation without exhaustive body fitting, bounded file-overview token fitting,
requirement-to-observed-source traceability, reusable `usage economics` reporting and
a native macOS CPU sampler. Broader inflection expansion was removed after Rust
regressions. The sampler's portable logic is tested; native acceptance is still owed.
These implemented increments do not close general retrieval, full real-model corpus
coverage, free-form task benefit or complete economics.

The [delivery/startup continuation](validation.md#evidence-delivery-and-unused-startup--2026-10-11)
adds coarse-vector passage delivery, lexical-first interleaving even when file coverage
is complete, plural acronym normalization and unused CLI startup avoidance. Both
file-capable recipes use `+overview3`, with short-input parsing avoided and distributed
nonblank text fallback. Real Gemma completes the 274-source pinned Foundry overview
index and unchanged preparation spends zero calls; final semantic delivery stays 2/8.
The current checkout's detector source now indexes without changing marker rejection.
All prior lexical deliveries survive. These are implemented repairs, not completion
of the broader acceptance work below.

| Capability | Implemented and checked | Still missing or unaccepted |
| --- | --- | --- |
| Exact source and compiler graph | Versioned source, lexical search, 23-language extraction, exact citations, admitted roots and compiler references/doors: [store](../src/store.rs), [SCIP](../src/scip.rs), [graph tests](../tests/semantic.rs). | Compiler reference identity does not establish runtime call behavior or test execution. Cross-root queries do not create cross-root edges. |
| Boundary relationships | `python-http-v6`: static FastAPI routes/imports/direct package definitions plus literal Flask routes and same-file Blueprints, dependency freshness and whole-proof delivery; real Flask/FastAPI checks: [producer](../src/boundary.rs), [tests](../tests/boundary.rs). | Re-exports, other import/mount forms, more frameworks and brokers. Dynamic/ambiguous/incomplete cases remain explicit. |
| Task evidence | Requirements, source links, briefs, boundary/compiler neighbors and explicit source-hashed execution imports; caller/target/test proof fits atomically; actual Flask requests now cover a dynamic callback and suppress stale observing tests: [composition](../src/task_evidence.rs), [runtime check](../tests/execution.rs). | More execution collectors/frameworks, cross-language callers and semantic requirement satisfaction. Explicit fresh requirement links now join observed target lines; overlap does not prove correctness. Producer reports do not establish complete coverage or execution attestation. |
| Bootstrap and reuse | Bounded pools, progressive publication, changed-input queues, ANN deltas and reuse. Real Gemma passes the repeated-family 80k lifecycle; representative Rust phases establish partial preparation/reuse. The new pinned Rust/Go corpus contains 69,565 regular files and indexes 67,663 admitted sources in 219.8 s. All admitted sources fit across a retained partial phase and verified suffix continuation, producing 944,387 card locations and a 2.24–2.38 GiB raw-vector obligation. Real preparation/resume publishes 2,820 vectors, reusing all 2,678 cached inputs on resume; 67,631 sources remain unpartitioned: [results](validation.md#broader-evidence-and-representative-workload--2026-10-11).  The preceding overview inventory fits all 67,663 sources with 63,621 distinct inputs (186.39 MiB raw vectors); bounded real prepare/resume publishes 592 vectors, with 67,021 sources still unpartitioned. | Complete model coverage and serving on unique-content 60–80k repositories, useful coverage, inference cost, sustained churn, reconciliation and compaction remain. |
| Continuous maintenance | Native primary-root watching, independent durable semantic/knowledge queues, explicit continuous scheduling and boundary refresh: [watcher](../src/watch.rs), [semantic driver](../src/neural/driver.rs), [knowledge maintenance](../src/knowledge/maintenance.rs), [watch tests](../tests/watch.rs). | Target-scale semantic/knowledge churn and platform robustness. Source watching alone starts no model; model lanes require explicit admission. |
| Model-aware chunking | Opt-in overview-v1 prepares at most one file input without fitting body cards; bounded file fitting shares keys with the updated hierarchy recipe. Actual-token batching, exact body windows, opt-in independent parent/file vectors and source-bound area vectors. Normal Linux worker passes 8,192 tokens under 8 GiB; tokenizer parity and a portable CPU receipt pass: [hierarchy](../src/neural/partition/hierarchy.rs), [areas](../src/neural/area.rs), [worker](../src/bin/foundry_embed.rs). | Comparative representation/long-context quality, full-window cross-runtime parity and Mac/GPU acceptance. Areas currently have a 256-record bound; true late chunking remains unimplemented. |
| LLM repository knowledge | Source-cited briefs and area summaries, HTTP/owned local generation, transactional completion reuse, changed-input/revert reuse and lineage invalidation: [knowledge](../src/knowledge.rs), [tests](../tests/knowledge.rs). | A useful model/recipe and demonstrated task benefit, generation throughput and amortized maintenance economics. Existing Qwen runs establish lifecycle behavior, not useful repository understanding. |
| Task/project memory | Explicit task state, reversible folds, original recovery, grounded review, promotion/correction/forgetting: [task context](../src/task_context.rs), [tests](../tests/task_context.rs). | Measured reduction in repeated failures/missed constraints and unnecessary interventions. Semantic conflict/loop reasoning remains a conditional extension. |
| Host integration and accounting | Declared receipt-based task economics now counts failed attempts and separates amortization assumptions. Frozen and live task-view application through the existing MCP owner and Rust gateway, revision checks, final-request receipts, restart/revocation refusal; cached/uncached accounting and per-arm cache reset; installed OMP checks: [live resolution](../src/task_host/live.rs), [gateway tests](../tests/gateway.rs). | Broader matched free-form tasks with a capable tool-using model and complete usage. Ordinary MCP delivery does not prove host application. Broader view regions remain a conditional extension. |
| Local resource controls | Per-worker CPU/RAM, shared memory admission, Linux affinity/cgroup hysteresis and idle generation eviction. Linux embedding now enforces seccomp, CPU/AS ceilings and owner death: [resources](../src/knowledge/local/resources.rs), [worker checks](../tests/embed_linux.rs). | Native macOS CPU observation/pressure acceptance (sampler code implemented), global GPU headroom, thermal/battery adaptation, optional embedding eviction and combined-model device acceptance. |
| Retrieval quality and economics | Experimental prose coverage/proximity scoring, exact body/declaration excerpts and partial-coverage interleaving. Development delivery is 2/8 versus previous 1/8 on Foundry and 3/6 versus 0/6 on inspected Rust probes; eight additional public Rust questions deliver 5/8 versus legacy 0/8. A regressing passage reranker was removed. Two-passage selection, language-qualifier disambiguation and matching semantic admission preserve prior delivery; mixed-corpus Go now improves from prior 1/8 to 5/8, retaining all preceding Foundry/Rust deliveries. Broader inflection expansion was removed after Rust regressions. Frozen-source attribution and pinned real-host repair diagnostics: [attribution](../tests/retrieval_attribution.rs), [host tasks](../tests/gateway/host_task.rs). | Effective general anchorless repair and held-out acceptance. The parser patches pass but composed execution reaches its deadline with greater known usage. Both new upstream percent-decoder pairs fail, with public checks and with/without prompt caching. Broader representative quality and amortized total-cost improvement remain. |

The next implementation priority is a repair tied to observed retrieval losses, with
model/runtime efficiency and broader evidence coverage following under their existing
owners. The [pending inventory](#pending-work-inventory--2026-10-10) retains all open
work, including hardware acceptance and conditional extensions. Existing historical
Gemma preparation times (112,549 cards in 1,256 s; 332,732 in 4,437 s) do not profile
today's pipeline or establish its speedup. Process memory measurements are not whole-host RAM.

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
track bytes, definitions/cards, input tokens and changed dependency fanout. T006 removes
the partition computation lock/serialization. T007 now interleaves bounded waves
and publishes early coverage from prepared rows; geometric full-generation builds
amortize cold publication without a new segment format. Accepted cache work survives
restart and source locations still revalidate before delivery.

Bound retained source bytes, queued token IDs, completed-but-uncommitted outputs,
model/device residency and publication scratch separately. Avoid repeated body
copies, parse/tokenize passes and full-store materialization. Use owned buffers and
existing redb/USearch facilities before introducing a new allocator or custom memory
mapping. A byte bound on source inputs is not a claim about total process memory.
Reusable CPU workers, token-shaped device batching and changed-vector publication
are now selected follow-ups below; do not add a generic scheduler or a new store.

Watching supplies hints to the existing ingestion owner. Coalesce repeated events;
reconcile admitted paths with the same no-follow, ignore and deletion rules; handle
renames, event overflow and restart through bounded reconciliation. Missed events
must not become certified freshness. Quiet operation must avoid repeated full-body
reads. Changed sources invalidate affected vectors, briefs and area dependencies
before their replacements are ready. An idle policy should consider foreground work,
CPU pressure, memory/device headroom and battery where available, with explicit
operator limits and hysteresis. Unknown host signals remain unknown.

### Efficiency audit after progressive bootstrap — 2026-10-09

Inspected baseline: `62c6c97` on `experimental/context-ecosystem`. This is a code and
publisher-contract audit, not a new runtime profile. The prior increment improves
first-searchable latency. It does not establish minimum cold-build cost, automatic
semantic maintenance or efficient publication of a small edit.
The table records the audit baseline. E1 now addresses CPU, startup and source-allocation
reuse; E2 implements changed-source selection and cheap status. Publication below remains
the E3 opportunity; the baseline rows are not a description of the updated implementation.

| Stage | What the code actually does | Remaining opportunity / owner |
| --- | --- | --- |
| Initial source ingestion | [ingest.rs](../src/ingest.rs) `index` walks, reads and calls `replace_source` serially, then drains lexical work. `ChangeScan` handles one entry per owner step. | Bounded parallel read/hash/prepare, batched conditional source commits and overlapping lexical drain. Preserve held-root/no-follow checks, partial progress and deletion rules. 001 T009/T011 follow-up. |
| Source storage | [store.rs](../src/store.rs) `replace_source` constructs copied 2,048-byte storage blocks before checking whether the stored hash matches; a changed source rewrites all its blocks. | Avoid chunk allocation for unchanged sources; reuse bounded buffers. Storage blocks, syntax units, embedding inputs and delivered evidence need separate policies. 001 T009 follow-up. |
| CPU preparation | Lexical `index_page` spawns threads per page; [prepare.rs](../src/neural/prepare.rs) `prepare_partitions_with_workers` spawns per wave. Partitioning parses/tokenizes; it retains range/key tuples, then selection reconstructs the source and renders/tokenizes wanted cards again. | Owner-lifetime bounded workers and a bounded, source/recipe-bound handoff of parsed units/token IDs. Keep current-source checks; avoid storing every AST or all token IDs for the repo. 009 T006/T007 follow-up. |
| Startup and warm replay | CLI `inner` verifies artifacts before acquisition; [supervisor.rs](../src/neural/supervisor.rs) verifies again. Acquisition precedes discovering that all vectors are cached. `semantic_begin_run` scans cache bytes; finalization counts again. | One verified-artifact handoff within a launch, lazy model acquisition where no inference is needed, trusted transactional counters with explicit reconciliation. Retain hash verification at trust boundaries. 009 T006/T007. |
| Embedding updates | [cache.rs](../src/neural/cache.rs) already commits only supplied keys. Matching inputs reuse vectors, including after restart/revert. Renames alter address text and thus keys. | Preserve exact-input reuse. A body-only edit can reuse a card because the body was never encoded; low call counts alone do not establish behavioral coverage. 009 T008/T009. |
| Work discovery | [driver.rs](../src/neural/driver.rs) starts a fresh source walk on each preparation and repeats it if the source revision changes. A stopped driver is not restarted by watcher edits. | Durable coalesced changed-source work, compare-and-clear acknowledgment and explicitly enabled continuous maintenance. Ordinary edits should cost affected work, not a corpus walk. 009 T008. |
| Vector publication | [index.rs](../src/neural/index.rs) rebuilds a complete USearch generation from current cached vectors/locations; geometric thresholds amortize cold publication. A mapping-only change can also rebuild ANN data. | Separate mapping changes from vector changes; bounded delta publication and compaction, with restart-safe generations. 009 T008. |
| Foreground/resource policy | Every MCP tool call, including status, extends the two-input batch window for 60 seconds. Any pending watcher work denies semantic store steps. Worker context is fixed at 2,048 tokens, flash attention disabled, f32 KV, Metal offload. | Protect real foreground demand while preventing observation-induced throttling and starvation during sustained source work. Admit by CPU/bytes/tokens/device headroom; validate backend-specific kernel/precision changes. 001 T011 / 009 T007/T009. |
| Status | [status.rs](../src/neural/status.rs) validates/hashes a generation, walks source partitions and performs a cache census; live bootstrap counters overlay that result. | Cheap versioned progress/coverage snapshots, with explicit audit/reconciliation and unknown/stale markers. Frequent progress polling must not scan all source/cache/index data. 009 T008. |
| Enrichment | [knowledge.rs](../src/knowledge.rs) now generates a bounded selected plan through an explicit HTTP provider, reuses exact completions and publishes through [brief.rs](../src/brief.rs). Source/child changes suppress stale area ancestors immediately. | A bounded selected-set scan avoids a new reverse-dependency index. MCP continuous maintenance, shared client admission and owned local workers with per-worker limits are implemented; native macOS CPU/global GPU pressure policies, Metal acceptance, real-model task utility and optional dense brief vectors remain 010 T002 work. |

The watcher updates source and lexical state incrementally; immediate invalidation
keeps stale semantic/brief evidence out. Invalidation alone does not schedule or
produce a replacement. The lexical `PENDING` table is cleared by its own consumer,
so a new semantic/generation consumer cannot simply treat that same acknowledgment
as its completed work. Source commits must durably record derivation obligations
with independent completion. Normal updates target the changed set; profile changes,
repair and unreported offline filesystem changes may require a bounded full
reconciliation. An unchanged restart means zero repeated inference, not necessarily
zero filesystem or metadata work.

### Model windows and chunk representations

Publisher sources checked 2026-10-09; no model was downloaded or executed. Keep exact
model/version distinctions rather than treating all “Gemini” or “1B” models alike.

| Model / contract | Documented input window | Relevance to Foundry |
| --- | --- | --- |
| [EmbeddingGemma 2, revision 914f7f89](https://huggingface.co/google/embeddinggemma-2/blob/914f7f89142e33e77833254d9c9b90c3cef7303b/README.md) | 8,192 tokens | Already the selected local model. Its model card supports 768d and 512/256/128d truncations. Foundry currently limits a whole worker call to 2,048 tokens. Model capacity is not yet proof that the pinned GGUF/backend is correct or efficient at 8K. |
| [Nemotron-3-Embed-1B-BF16, revision c0c9fea9](https://huggingface.co/nvidia/Nemotron-3-Embed-1B-BF16/blob/c0c9fea93ea424587517f2c59e20db9f1d6bf615/README.md) | 32,768 tokens | Card specifies average pooling and 2,048d with normalized prefix slicing. The larger `max_position_embeddings` configuration value is not a serving promise. Earlier invalid Foundry G2 runs do not establish long-context quality; latest owner request reopens comparison, not automatic model replacement. |
| [Mistral Embed 23.12](https://docs.mistral.ai/models/mistral-embed-23-12), [Codestral Embed 25.05](https://docs.mistral.ai/models/codestral-embed-25-05) | 8K on each model card | Official text/code embedding APIs are distinct products. “Mistral 1B” does not identify one of these artifacts. Their hosted API contracts do not demonstrate a compatible local 1B worker. |
| [Gemini Embedding 001 / Embedding 2](https://ai.google.dev/gemini-api/docs/embeddings#model-versions) | 2,048 / 8,192 tokens respectively | Different embedding spaces; migration requires re-embedding. Both offer dimensions through 3,072, above Foundry's current 2,048 cap. Embedding 2 multipart aggregation is distinct from independent batch inputs. A hosted adapter needs explicit endpoint/data scope. |

The current `cards-v1` recipe encodes an address, signature/head and leading docs,
128 tokens by default. It omits multiline implementation bodies regardless of how
far its token limit is raised; unsupported/unparsed sources may have no cards.
[partition.rs](../src/neural/partition.rs) repeatedly tokenizes growing line prefixes
while fitting a card. [profile.rs](../src/neural/profile.rs) also enforces
`batch × card_tokens <= 2048`. The nominal 32-sequence protocol cap, default batch
of eight and foreground cap of two are separate limits. Bigger chunks can therefore
shrink batches in the current implementation rather than fill the device better.

Revisit four independent sizes: storage blocks; parsed/retrieval units; model input
windows; and evidence delivered under the caller's budget. A long input can inform
many small cited results; a large model window need not become a large agent reply.
Select representations by role and workload:

- Keep compact addresses for identity and navigation. Compare semantic body units
  containing a full short function/section, with bounded windows only for oversized
  units. Include necessary local context and exact span maps. Test implementation-
  only edits, opaque names, config/tests and sources without syntax coverage.
- Add parent/file/area representations selectively for orientation; retain smaller
  child evidence for localization. Avoid embedding every possible representation
  of every source. Deduplicate their results by evidence identity and bound each
  representation's candidate share. A whole-file vector can dilute a small but
  essential behavior and makes any included edit invalidate that large input.
- Make per-input tokens, batch-total tokens, sequences, dimensions and device memory
  separate verified provider capabilities. Pack actual token lengths, bucket similar
  lengths where useful, and overlap bounded CPU preparation with one resident model.
  More CPU cores do not imply more copies of a GPU model. Preserve query priority.
- Consider late chunking as a separate candidate: encode longer surrounding text,
  then pool token spans into smaller contextual vectors. It needs token-level
  states, the model's projection/pooling recipe and a larger invalidation domain;
  the current pooled-vector-only interface cannot implement it by averaging outputs.
- Keep vector identity bound to actual model/tokenizer/templates/pooling/normalization
  and any context that changes meaning. Separating path/line locators from vector
  content may reduce rename churn, but changes the recipe and must retain enough
  context to disambiguate equal code. Never reuse an old vector for changed input
  merely to lower a recomputation counter.

The first runtime baseline remains the existing Metal worker; portable CPU/store
changes come first. The next hardware target remains unselected.
CUDA/CPU/remote execution each needs its real adapter and
capability checks. Larger contexts, flash attention, KV precision and thread policies
are candidates to validate, not claimed speedups. Preserve no-truncation behavior,
single-versus-batched parity and foreground responsiveness. Existing G1/G2 results
remain evidence about their historical recipe only.

### Revised build order

A–D remain completed experimental capabilities; the owner's efficiency correction
puts E1–E4 ahead of broad generated knowledge. E1 and E2 are implemented experimentally as recorded
below; E3 ANN deltas are implemented, with remaining publication costs named below.
E4 body/parent inputs and model geometry are implemented experimentally; real-device
acceptance remains. F implements CLI/MCP generation, continuous scheduling and shared
client admission plus owned local generation with per-worker resource controls. Aggregate
memory admission now shares embedding/generation ceilings and defers background work under
host/container memory pressure, with opt-in Linux host CPU admission. Native CPU/global
GPU pressure, Metal acceptance and real-model task utility remain. G has an implemented
experimental slice with broader evidence coverage still pending. Sandbox build/test authorization continues.

| Order | Concrete increment | Owner/status |
| --- | --- | --- |
| A | Bounded parallel semantic partition computation outside serving/writer locks | 009 T006: implemented |
| B | Source-bound structured brief import, lexical discovery and lifecycle | 010 T001: implemented; selective generation follows in F |
| C | Opt-in native source watching and bounded reconciliation | 001 T011: implemented; platform/scale limits remain |
| D | First semantic result before full partitioning, priorities and geometric publication | 009 T007: implemented; total throughput/maintenance unresolved |
| E1 | Remove repeated cold/warm CPU and startup work | Semantic worker reuse, bounded tokenized-card handoff, lazy CLI acquisition, no-op chunk allocation avoidance, bounded initial ingestion and lexical worker reuse implemented. Verified artifact handoff, transactional counters/reconciliation and exact per-card fitting reuse implemented under 009 T006/T007; macOS model handoff and real-device throughput remain unexecuted here. |
| E2 | Durable changed-work frontier, explicit continuous semantic maintenance, independent acknowledgments and cheap status | 009 T008 with 001 T011; implemented experimentally in schema 9; ordinary changed-input selection is bounded; ANN deltas follow in E3 |
| E3 | Publish vector/location deltas and compact by bounded policy; a single edit must not rebuild the full ANN | 009 T008; ANN deltas, mapping-only publication, bounded compaction and crash recovery implemented. Warm affected-location, bounded location overlays, shared metadata and clean-restart publication reuse are implemented. Schema-12 changed-input restart now resumes a durable lookup and touches affected paths/keys. Required cold byte verification, full reconciliation and compaction remain corpus-sized; total affected-path cost is not claimed. |
| E4 | Model-owned input/batch geometry, body-aware and selective parent representations, token-shaped device batches | 009 T009; opt-in body windows, bounded parent prefixes, versioned limits and actual-token batches implemented. Address profiles/caches preserved; hierarchy-v1 independent parent/file/area vectors implemented; real-model quality comparison remains |
| F | Selective memoized LLM generation, dependency tracking, affected area rollups and indexed derived knowledge | 010 T002: explicit HTTP/CLI generation, transactional completion reuse, bounded transitive lineage and area freshness implemented in schema 10. Continuous MCP scheduling, shared client admission, owned local generation and opt-in shared memory admission are implemented. Linux host CPU admission is also implemented; native macOS CPU/global GPU pressure, Metal acceptance and real-model task utility remain. A bounded selected-set scan replaces a persistent reverse index for now. |
| G | Evidence-bearing boundary joins and task-oriented implementation/caller/test composition | 005 T005 / 003 T009: implemented experimentally. Static FastAPI/literal HTTP joins, changed-file publication, source/config freshness, ambiguity and primary-task composition under one allowance. Sandbox contract repair verified; broader producer/host acceptance remains. |

Use work-count assertions in sandbox tests: unchanged replay sends zero embeddings;
one edit visits only its derivation work and affected dependencies; unchanged inputs
are parsed/tokenized once per intended stage; no full ANN build occurs for a small
delta below compaction thresholds; pause/quiet operation admits no background model
work. Include changes during compute, burst coalescing, delete/rename/revert, cache
corruption, profile migration, restart at every commit boundary, overflow fallback
and foreground demand. These checks reveal repeated work without pretending to
measure device throughput. Observe actual CPU/device utilization, RAM, total tokens,
first-result and full-build time when the target machine is available; historical
wall times cannot identify which stage caused them.

Previously selected work also remains open: 009 T005 anchorless miss repair beyond
tracing; 001 T010 behavioral retrieval acceptance; 003 T006 broader real-host
quality/economics acceptance of the implemented live working-view synchronization; 010 T002 model/resource acceptance; broader 005 T005
producer coverage and 003 T009 real-host task acceptance. Experimental task state,
recovery and reminders are implemented. Explicitly configured OMP gateway application
is verified; ordinary MCP application and measured quality/economics remain unknown.
Package signing/installed-artifact acceptance remain
release follow-up, not a sandbox prerequisite. 013 stays frozen off.

Experimental follow-ups need not wait for full throughput or held-out quality measurements.
Contract tests and a small sandbox task fixture accompany each implementation.
Larger evaluations assess usefulness rather than blocking the owner's experimental
build authorization. Continue on `experimental/context-ecosystem` with local commits;
no default promotion, release, training unfreeze or predecessor deployment is implied.

### Pending work inventory — 2026-10-10

Updated **2026-10-11** after full Rust/Go fitting, bounded real-model preparation and
reuse, prose/model-admission repairs, package routers, observed Flask callbacks,
portable CPU/Metal receipts and cached/uncached upstream host diagnostics. The heading
retains its earlier date to preserve incoming links. Experimental implementation is
committed separately from release acceptance.
This collects the current unfinished work; the linked owning specs retain its contracts.
The latest code continuation is recorded in [current validation](validation.md#evidence-delivery-and-unused-startup--2026-10-11).
File-overview bootstrap and bounded fitting, explicit requirement/execution joins,
declared task-cost reporting and macOS CPU sampling code are now implemented. Coarse evidence delivery, blank-opening repair and CLI anchor startup avoidance are now
implemented and checked, including the real model on the small pinned Foundry repository.
Native CPU observation remains unverified; broader collectors, automatic selective body
expansion, full representative inference/churn, general retrieval and demonstrated
task-cost benefits remain TODOs. The earlier detailed measurements below are retained
as historical baselines, not claims about the new overview recipe.
The [audited implementation map](#what-exists-and-what-is-missing) is the closeout
summary. Read the rows below as three different kinds of unfinished work: missing
capability, implemented behavior awaiting scale/quality/platform evidence, and
conditional extensions requiring a concrete failure before more machinery. They are
not a list of wholly unimplemented subsystems.
Hardware/host evidence is pending acceptance, not a prerequisite for sandbox implementation.
005 T005 and 003 T009 now have implemented sandbox slices; they are no longer first-build
TODOs. Cold/changed-input publication work under 009 T006–T008 now has another implemented
increment: bounded durable restart lookup, paged database reads, lazy cold hashing and
atomic commits containing at most four ready sources within the existing eight-read bound. Scope a concrete repeated operation
before adding infrastructure. Resource and broader evidence-coverage follow-ups remain
explicit and can proceed when their platform or workload is available.

| Work | Remaining TODOs and owner |
| --- | --- |
| Shared resources | **Implemented earlier:** opt-in embedding/generation ceiling reservations, host/visible-cgroup memory headroom, hysteresis, idle generation eviction, status and lifecycle tests ([010 T002](../specs/010-source-bound-derived-knowledge/spec.md#t002-shared-memory-admission--2026-10-10)). Now also implemented: opt-in Linux affinity and visible cgroup-v2 ancestor CPU admission with sampled pause/resume hysteresis, preserving idle generation residency. Quota/affinity changes reset observations. Native macOS host CPU sampling code and portable arithmetic checks are now implemented. Remaining: native CPU observation/pressure acceptance, global GPU headroom, thermal/battery adaptation, optional embedding eviction and real combined-model hardware acceptance. Available Linux CPU validation passed with two threads and a 2 GiB worker address-space ceiling; five visible CPUs have a four-CPU cgroup quota. GPU/Metal are unavailable here. |
| Boundary coverage | **Implemented under 005 T005:** static FastAPI routes, nested/direct configured same-service routers, literal requests/httpx consumers and dependency-wide freshness. `python-http-v6` includes literal Flask routes/Blueprints and direct configured package-initializer router definitions. Real Flask 3.1.2 and FastAPI 0.141.1 checks pass; edits invalidate dependencies and package cycles/re-exports remain incomplete. Remaining: re-exports, more import/mount forms, further frameworks/brokers and cross-language boundaries. Dynamic/unsupported cases stay incomplete; no inferred cross-repository admission. |
| Task evidence breadth/acceptance | **Implemented under 003 T009 / 005:** source/brief/boundary/compiler composition and compact facet accounting. `import-execution` adds explicit completed producer receipts for dynamic calls and covered source spans. Actual CPython `getattr` and Flask test-client/dynamic-callback fixtures are captured and imported (12 and 19 observations); unexecuted branches stay absent and edits to the observing test invalidate its observations after restart. Caller, target and test citations must fit together and remain current. Remaining: broader collectors, frameworks, cross-language callers, semantic requirement satisfaction and representative host outcomes. Explicit source-linked requirements now join observed target-line proofs; source overlap is traceability only. Syntax is not execution; imported observations are producer reports, not full coverage or attestation. |
| Bootstrap/publication scale | **Implemented under 009 T006–T008:** progressive publication, incremental ANN/cache reuse and eight-input contended turns. The repeated-family 80k lifecycle passes. The earlier 53,955-source Rust store publishes 582 active vectors; the tuned cold phase covers all 16 priorities but leaves 53,931 sources unpartitioned. The new pinned Rust/Go corpus has 69,565 regular files / 68,056 distinct contents; 67,663 admitted sources index in 219.8 s. Whole-input fitting completes across two retained phases: 944,387 card locations, 782,716–831,067 distinct inputs, and 2.24–2.38 GiB raw vectors before metadata. The default 2 GiB cache cannot complete this recipe. Real preparation/resume publishes 2,820 vectors and reuses all 2,678 cached inputs; 67,631 sources remain unpartitioned. Cold provider calls take 96.85% of elapsed. Fitting and bounded inference reports are kept separately in [current validation](validation.md#broader-evidence-and-representative-workload--2026-10-11). The new overview-v1 recipe fits the same corpus into 63,621 distinct inputs; real bounded preparation publishes 592 vectors. This lowers the obligation but does not establish equal quality. **Remaining:** complete current model coverage with sufficient explicit cache capacity, task-relevant serving, sustained semantic/knowledge churn, overflow reconciliation and compaction costs. Compare existing coarse file/area representations and selective body preparation on task outcomes before changing exhaustive preparation defaults. Prior real-model waits dominate source preparation; more parsing threads alone will not solve inference throughput. |
| Watching and maintenance robustness | Under 001 T011 / 009 T008, the synthetic 80,000-file Linux fixture now passes: startup reconciliation 32.9 s, then a watched edit/search in 176 ms with one source read and no full reconciliation. Still validate mixed-size repositories, semantic/knowledge maintenance at that scale and target platforms. The traffic fixture now includes stable pending work alongside churn; continuously invalidating every candidate could correctly produce no inference. The HTTP fixture now starts its tested timeout at provider admission, removing the invalid assumption that an 80 ms whole-run budget always reaches a call. Keep observing platform/workload failures; these fixture repairs are not real-device throughput acceptance. |
| Chunking/model geometry | **Implemented under 009 T009:** body-v1 plus opt-in hierarchy-v1 independent parent/file inputs and current multi-source area vectors, exact-key/revert reuse and brief-only maintenance wakeups. The normal Linux worker adopts the checked pooled-output fix with CPU revision 3 / Metal revision 4; two full 8,192-token IPC calls pass under 8 GiB, and eight tokenizer.json/GGUF parity strings pass. A new portable CPU receipt passes single/batched short inputs plus 8,192 tokens; strict comparison and native commands are ready, but no Metal receipt exists here. Remaining: representation/long-context quality comparisons, full-window cross-runtime parity, native Mac/GPU tests and scaling beyond the explicit 256-area bound. True late chunking is an unimplemented conditional backend candidate. |
| Generated knowledge | Under 010 T002, select a useful model/recipe and establish task benefit, throughput, maintenance costs and amortized economics. Real Qwen CPU validation now covers initial/changed-input generation and restart/revert cache reuse without weights or worker acquisition (two model calls, then zero). This establishes lifecycle correctness, not task benefit or model selection. The original bounded Qwen repair-selection comparison failed at the output ceiling. Revised short JSON recipes and completion-at-object-end termination now complete all calls, but repair quality remains only one of two cases in every arm; useful general task performance/model selection remains unaccepted. The earlier 0.5B free-form OMP calibration fails both initial-context arms with no edits. The new pinned Qwen3-1.7B run makes actual tool edits but fails both external checks; the model/recipe remains unaccepted for useful tasks. Failed local output-limit calls now retain actual token usage without publishing partial output. Independent area vectors now exist under hierarchy-v1 with a 256-area bound. A larger overview index needs an observed scaling requirement; general generated-knowledge quality remains open. |
| Anchorless retrieval | **Experimental under 009 T005:** disjoint code/prose windows and exact matching excerpts, now distinct-term coverage/proximity scoring, hyphenated prose and lexical/dense interleaving under partial semantic coverage. Frozen Foundry source delivers 2/8 required spans versus 1/8 from the saved previous executable. Initial Rust probes deliver 0/6 in all arms; a whole-declaration excerpt repair recovers 3/6 on the same inspected cases while controls stay 0/6. Eight additional public Rust questions deliver 5/8 versus legacy 0/8. A passage-locality reranker was rejected: no additional delivery and a 3/6 to 2/6 regression on the earlier Rust set. The next repair keeps two disjoint passages per long unit and disambiguates unmarked language qualifiers; the prior delivered spans are retained and mixed-corpus Go improves from 0/8 to 1/8. Attribution now distinguishes actual excerpts from outlines. CLI/MCP model admission now shares the same prose rules; a real partial-coverage query delivers `sort.Search` in 2,042 tokens. These are development results, not an effective general repair. **Remaining:** ranking, per-file selection and macro-contained evidence, preserved G2 inputs, fresh independent acceptance and useful task outcomes. One frozen Foundry source remains excluded by existing private-key marker policy; the current detector source now avoids containing its own literal, without changing runtime rejection. Complete coarse semantic preparation of the 274 admitted frozen sources still delivers 2/8; file coverage is not quality acceptance. Defaults stay unchanged. |
| Mixed symbol/behavior retrieval | Complete 001 T010 held-out retrieval/compatibility acceptance. The experimental implementation and sandbox fixtures exist; default promotion remains unearned. |
| Host context application | **Now implemented under 003 T006–T008 / 008 T003:** explicit frozen task-view export, gateway application to one bound folded observation, complete selected/protected view, exact original digest/tool linkage, final-body receipt and restart replay. Installed OMP 18.6.0 passes resumed-history/final-request checks with a scripted upstream. Live synchronization now resolves the existing HTTP MCP owner before each request, preserves the anchor and protected entries, records applied revision, and refuses revocation/unavailability without stale fallback. Installed OMP passes a post-export edit and preserved-history check. The earlier OMP/Qwen0.5B calibration failed without edits. A new Qwen3-1.7B comparison on an expanded synthetic fixture makes real edits, but both tasks fail (panic/noncompiling patch). Composed-context usage is complete; one cancelled baseline request is unknown. The subsequent 7B comparison fails native tool calls. Pinned Qwen3-4B-Instruct-2507 passes that protocol and makes edits, but its read/write-only parser/overflow repairs still fail the external checks. Public-test feedback enables a successful full-file control. After the task composer stops promoting file outlines to whole bodies, both final patches pass the external parser oracle, but the composed host still reaches its deadline with greater known usage and an unknown request. Remaining: a useful model/recipe, broader matched tasks/economics, broader region selection and durable incarnation/revocation semantics if needed. Ordinary MCP application remains unknown; live freshness is indexed state at resolution and may race later edits. |
| Memory interventions | Under 008 T004, read-only revision-bound `review` now recalls fresh source-linked constraints/failures for explicit changed paths and selected obligations/questions before finishing. Bounds, silence, stale suppression and original attribution are tested; no new durable state/model call is added. Remaining: broader loop equivalence/semantic conflict reasoning if justified, plus unnecessary-intervention and verified-task outcome evaluation. Explicit promotion/forgetting is already implemented. |
| Task quality and token economics | Under 003 T006/T009 and 008/010, demonstrate verified task completion with total request, auxiliary model, bootstrap and maintenance costs. Existing 0.5B/1.7B calibrations remain negative. The 7B backend fails native tool-call probes. Pinned Qwen3-4B-Instruct-2507/OMP makes real parser/overflow edits, but the read/write-only attempts fail in both arms. All twelve requests have complete usage; smaller composed input is not a completion benefit. The harness rejects completion-cost comparisons after a failed or incompletely metered attempt. A pinned upstream percent-decoder repair also fails both arms at their task limits, with one unknown request each; observed subtotals remain available. With public compiler/test feedback, full-file context passes the independent parser oracle in 7 requests, but the composed arm fails after 12 requests, with higher known token usage and one unknown request. A follow-up fixes outline promotion and preserves exact caller bodies under the existing budget. Both final patches now pass; the full-file host completes in 4 requests, while the composed host reaches its deadline after 10 metered requests and 1 unknown. Known input is 19,474 versus 40,844 tokens. Completion-cost benefit remains unestablished. Remaining: reliable task completion, realistic multi-step and memory-sensitive tasks, held-out matched outcomes and amortized economics. Two new pinned upstream percent-decoder pairs expose 18 unchanged public tests. Both uncached arms fail the external oracle; cached full-file edits fail compilation while cached composed context makes no edits. Cache reset receipts and complete cached usage are retained. Total and uncached inputs are separate; impossible cache totals are refused. No monetary or cost-per-completion gain is established. |
| Native/package acceptance | Portable native embedding receipt/cosine comparison is implemented and its Linux half passes; run the prepared Metal half. Validate macOS CPU/Metal generation, isolation/owner-death/resource behavior and the new memory probe. Complete 009 signed installed-artifact checks before semantic release advertising. See [measurement handoff](measurement-handoff.md). |
| Record-only timing and release | Still owed: no-swap library preparation timing and pre-T009 rust-lang/rust indexing timing. Complete active specs and the [release checklist](release.md); release destination/publication are separate. |
| Cloud environment | The restored checkout retains the experimental commit, Rust 1.90 and installed OMP/Bun; focused tests run offline. Prior `/tmp` diagnostic artifacts are absent, so their committed aggregates remain historical evidence, not reanalysed raw records. The setup draft includes pinned installation and host-test instructions; review/publication and installation from that published draft on a clean environment remain unverified. This does not block current sandbox work. |

Frozen/deferred scope is **not** an active implementation queue: 013 learning stays off;
014 migration awaits a concrete task; 002/004/006/011/012/015 are superseded. Cross-repository
graph edges, learned selection, KV/latent-memory work and new hardware backends require
their own concrete scope. Existing watcher, artifact reuse, incremental embedding/enrichment,
ANN deltas, body/parent chunking, local generation, the first boundary producer and
primary-task composition are not reopened as unimplemented work.

Use tasks such as tracing a request through configuration and a worker, repairing a
behavior while preserving its callers, and a multi-service contract change with its
tests. Compare a native-tools baseline, current Foundry and the candidate with the
same host/model and task constraints. The primary outcome is a correct, verified
change without violated requirements; record wrong edits and incomplete evidence.
For monetary economics, divide all attributable bootstrap, maintenance, retrieval,
model and retry costs by verified completions over a stated reuse horizon. Report
latency, RAM and energy separately; add failed attempts to cost rather than dropping
them. The opt-in free-form OMP calibration and earlier bounded CPU selection comparison
are recorded separately in [validation](validation.md); neither establishes broad task economics.

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
measurement, not unrelated fixture work. The original planning pass ran no benchmark;
later executed diagnostics and their limits are recorded in [validation](validation.md).

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
