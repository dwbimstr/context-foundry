# 010 — Source-bound derived repository knowledge

Status: Experimental T001 implemented locally; T002 now implements explicit selective
HTTP generation, completion reuse, area lineage and opt-in continuous MCP maintenance.
Shared client admission, derived MCP context and an experimental owned local generator
with per-worker CPU/RAM/Metal controls and shared memory admission are implemented.
Global GPU pressure policies, native CPU sampling, hardware and real-model task acceptance
remain selected work. The owner's
large-repository direction supersedes the earlier blanket deferral.
Import and lifecycle fixtures use synthetic outputs through real storage, retrieval,
HTTP/CLI and native-worker boundaries. Real CPU initial/changed-input generation and restart/revert reuse are recorded in validation;
it is not a selected production model or task-quality result. 013 remains frozen off.

## Outcome and ownership

An agent should find behavior and understand a subsystem without repeatedly reading
large files merely because the user's vocabulary differs from the code's identifiers.
LLM-generated source briefs and area summaries complement exact source and graph
facts. They are distinct from caller-authored task state or durable project memory.
The [ecosystem plan](../../docs/context-ecosystem-plan.md#architecture-correction-useful-context-across-6000080000-files)
records the architecture, Prakarana pin and build order.

Existing source transactions remain authoritative. A generated assertion is always
labeled derived and attributed to its generator; citations prove which source was
used, not that the assertion is true. Unlinked claims and unsupported fields stay
unknown. Generation never runs inside source commits or foreground queries.

### T001 — Store, find and invalidate a structured source brief

First implement an explicit CLI/library import seam for a bounded generator result,
using synthetic outputs to exercise the lifecycle before choosing a real generator.
This is the same acceptance path the later worker uses; importing a fixture is not
LLM runtime evidence. Example target: find a capacity-eviction routine with opaque
identifiers from a behavioral question, then recover its implementation and tests.
Record the baseline result and exact expected evidence in the implementation fixture;
do not claim current G2 attribution from that constructed example.

- One current brief per source and generation recipe. Schema fields: purpose,
  behavior, provides, consumes, constraints, tests and unknowns. Assertions carry
  exact source handles and an explicit derived classification. Use bounded JSON,
  at most 16 KiB of text and 32 cited spans per brief; reject malformed/extra fields.
- Record workspace, source/dependency hashes, model/artifact identity, prompt/recipe
  digest and generation time. The same accepted input identity is a replay no-op;
  replacing it atomically retires the old searchable representation.
- Reconstruct/hash-check the cited source snapshot on input. Before acceptance,
  compare every dependency with current source state. Unsupported identifiers are
  refused or reported as unknown; byte presence is not semantic verification.
- Use a small source-bound record table inside the existing redb owner and the
  existing derived lexical index. A separate table is justified because automatic
  source invalidation must not delete explicit user-authored project memory. No
  standalone knowledge database, custom journal or generic orchestration framework.
  Implementation must include the ordinary explicit schema upgrade and recovery tests.
- Source edits/deletion make a brief ineligible immediately, before index cleanup or
  regeneration. Its old text cannot be returned as current evidence from lexical or
  dense lookup. Explicit forget removes retained brief text and schedules derived
  cleanup; exports/backups remain outside live-store deletion guarantees.
- Search through generated vocabulary maps to the cited source identity. Group
  multiple forms of one source so summaries do not crowd out independent evidence.
  Keep exact-name retrieval unchanged. Task context may deliver a brief as labeled
  orientation plus supporting source; it cannot substitute for required edit evidence
  solely because it is shorter. Count all delivered text in the same allowance.
- Verify conceptual discovery, stale dependencies, edit-during-generation, deletion,
  malformed claims, replay, restart, output bounds and exact-source recovery. Include
  a misleading-summary fixture so a lexical hit cannot certify answer completeness.

### T001 implementation decisions and acceptance

[brief.rs](../../src/brief.rs) owns the strict CLI/library import seam;
[store.rs](../../src/store.rs) shares Tantivy record documents, bounded candidate
selection, pending drain and repair. Schema **8** adds `source_briefs` and the
never-reset `brief_revision`; upgrading schemas 1–7 is explicit and transactional.
The search schema remains 5. Old binaries refuse schema 8 before reading its typed
`brief:` pending keys. Explicit user memory retains its independent lifetime.

- `cited_spans` contains 1–32 nonempty v2 spans; span zero identifies the owning
  source and is the primary context evidence. Claims refer to zero-based indices
  and declare the identifiers to byte-check in those spans. Purpose and behavior
  are required; the other claim lists may be empty. Unknowns are delivered as
  unknowns and excluded from the lexical document. All extra JSON fields are refused.
- ID is SHA-256 of the JSON tuple `(workspace, primary path, recipe digest)`.
  The input identity binds the workspace, ordered spans, model, artifact, recipe,
  prompt digest and every dependency's full source digest. Generation time is
  attribution. An identical output replay may change that timestamp and remains
  a no-op; differing output for identical inputs conflicts. Changed inputs require
  the current `expected_revision`. Admission rechecks sources even on replay, so
  a late stale generation result is refused rather than reinstated.
- Admission reconstructs and hashes every cited source, reusing bodies by path
  inside one writer transaction (at most 32 sources, 64 MiB of source bytes).
  No generation occurs there. Identifier presence validates only declared bytes;
  citations and generator metadata do not verify prose or prove a provider ran.
- Requests are at most 256 KiB; claim/identifier/unknown text totals at most 16 KiB.
  Persisted records, including repeated full dependency paths, are at most 512 KiB.
  `get` suppresses stale text; explicit per-ID `export` may return it with
  `eligible:false`. Forget removes the live row and queues index deletion atomically.
- Brief search examines at most 256 derived candidates, validates live revisions
  and every dependency in one final snapshot, groups by primary source, and returns
  at most eight records under an explicit JSON-byte allowance. Pending brief work,
  stale candidates, a filled candidate window and output omissions are visible.
  A first record that cannot fit causes a refusal, not a successful empty answer.
- CLI `context --include-briefs` and library `ContextOptions.briefs` augment only
  anchorless queries. Supporting primary source spans precede ordinary candidates.
  [response.rs](../../src/response.rs) permits a `brief:` line only when its primary
  span is delivered, using the same complete-response tokenizer and byte allowance
  as source, explicit project memory and a selected task view. Records are explicitly
  `kind:derived`; test/dependency citations remain exactly recoverable. This does
  not assert that primary source plus orientation is a complete editing packet.
  The MCP catalog has no brief tool/flag in this slice; generation follows in T002
  below. Model-derived dense brief indexing remains unimplemented.

[tests/brief.rs](../../tests/brief.rs) uses the real store, lexical index, packer and
CLI. Its constructed baseline for `capacity eviction` is empty; the imported brief
finds the opaque routine and recovers exact implementation/test bytes. Other cases
cover an intentionally false summary, exact-name output stability, source/dependency
edit/delete, edits during generation and final selection, CAS/replay, unknown fields,
byte/UTF-8/citation bounds, row corruption, hash verification, namespace collisions,
user-memory preservation, restart/repair, pending replacement during drain, explicit
schema-7 upgrade and real child-process exits before/after put and forget commits.
See [validation](../../docs/validation.md) for executed checks and limits.

### T002 — Generate useful briefs and area knowledge within idle resources

Status: explicit CLI and continuous MCP generation implemented experimentally. Shared
client admission and owned local per-worker limits are implemented; aggregate hardware headroom and real-model task acceptance
remain open. The efficiency amendment places this after
changed-work ownership (009 T008) and input/resource identity (009 T009); see the
[build order](../../docs/context-ecosystem-plan.md#revised-build-order).

After T001, select one accountable generation provider and a pinned recipe. First-
party orchestration stays Rust; local generation uses an isolated, bounded model
worker. A remote provider requires an explicitly configured endpoint and data scope.
Neither this spec nor retrieval admission sends source to an unspecified service.
Use existing maintained transports and the shared source acceptance seam.

- Source replacement only records durable pending work; it never waits for a model.
  Resume from missing/stale input identities and preserve completed results across
  crash/restart. Deduplicate in-flight source/recipe work; late results must recheck
  dependencies, never overwrite a newer source version.
- Prioritize requested areas, entry points and changed dependencies within explicit
  work/token/time limits. Generate useful behavioral briefs selectively; generating
  one description for every file is not a bootstrap prerequisite. Do not continually
  regenerate identical inputs or recursively summarize summaries without dependency
  identities and a bounded level count.
- Derive area overviews from current child briefs plus exact manifests/entry points
  and tests. Retire affected ancestors on dependency changes. Composing model-written
  content preserves the derived label. Keep original source recoverable at every level.
- Share resource admission with 009: foreground priority, CPU and device headroom,
  bounded queued bytes/tokens, and hysteresis for idle work. Account for model
  generation separately from embedding; increasing one lane's work must not starve
  source freshness or foreground evidence delivery.
- Test with a real configured provider at its actual boundary before claiming model
  integration. Compare verified coding tasks and complete generation/maintenance/task
  cost; the owner's sandbox build does not wait on a held-out promotion gate.

Incremental design amendment, 2026-10-09: record exact generation-input identity
(source/dependency hashes, selected spans, prompt, model and recipe) and reusable
completed results. A reverse dependency index should identify only affected briefs
and area ancestors when source changes; compare its durable maintenance cost with
scanning the bounded selected brief set before adding it. It is not a second source
ledger. Coalesce superseded obligations and acknowledge generation independently
from lexical or embedding work. Accept generated output only if dependencies remain
current; every derived parent retains the full transitive freshness dependency even
when an intermediate brief's prose happens to stay identical. Unchanged inputs must
not trigger another generation call. Crash/restart, edits during generation, failed
ancestors and provider backoff must preserve reusable results and bounded work.

Lexical brief discovery is implemented in T001; dense brief vectors are optional
follow-up within the same representation/identity contract, not a prerequisite for
useful generation. Generation tokens, embedding tokens and query delivery remain
separately accounted. The task does not require describing every file on bootstrap.


### T002 implementation decisions and boundaries

[knowledge.rs](../../src/knowledge.rs) adds `knowledge prepare --profile FILE` under
explicit exclusive CLI ownership. A strict profile selects at most 128 source/area
nodes, a source-path allowlist, a model/deployment digest, instructions and one
OpenAI-compatible Chat Completions endpoint. Sources are exact byte spans, with the
first span anchoring each brief in original source. Node order prioritizes ready work;
a stable topological walk handles at most eight children per node and four area levels.
No all-file description pass or model download is added. The HTTP profile launches no
executable; the explicit local profile below owns its worker. MCP
startup generation requires the explicit maintenance option described below. Schema **10**
requires an explicit upgrade.

The first provider is a configured HTTP service through the existing reqwest transport.
Literal loopback HTTP(S) is allowed; remote HTTPS also requires `allow_remote:true` and
the explicit source scope. No ambient proxies, redirects, automatic retries or URL
credentials. An optional named environment variable supplies bearer auth. Requests use
non-streaming JSON completions, one active client request, a 1–300 second call timeout,
and the whole-run cooperative deadline/cancel flag. A timeout closes the client request;
Foundry cannot prove that an external service stopped model computation. No external
process/device-isolation or weight-attestation guarantee is inferred from HTTP. The
response must name the configured model, finish with `stop` and contain no tool call;
the deployment digest is the operator's pin, not an attestation from the server.

Inputs are bounded at 128 KiB of serialized request bytes and 32,768 **o200k request
admission tokens**, using the existing pinned response tokenizer over the complete JSON
request. These are explicitly separate from the provider's tokenizer/chat framing.
Each call requests at most 8,192 completion tokens; per-run call, request-token and
requested-output-token budgets admit work before sending. The existing 32-span/16-KiB
claim-text limits apply to accepted outputs, and the HTTP response cap is 256 KiB.
No implicit source-prefix truncation: an oversized node requires smaller explicit spans.
Reported provider prompt/completion tokens are counted separately; absent usage stays
unknown. Reported completion overrun refuses publication. No billing or total host-token
savings claim follows from local admission counts. Failure details never echo provider
response text or credentials. Failed providers/invalid outputs persist a 30-second
backoff; cached/current results remain usable during it.

Generation first checks each selected current brief using source and child metadata,
without reading/tokenizing unchanged source bodies or contacting a provider. Missing
inputs bind the workspace, exact source spans and full source hashes, instructions,
wire request, model/deployment/endpoint identity and full child meaning. Changing call
budgets or cache size does not re-generate current inputs. Changing model identity,
source selection, instructions or completion capacity does. Prepared source is untrusted
data, and model assertions stay derived even when all citation/identifier checks pass.

Area input includes exact manifests/entry points/tests and current child briefs. The
optional brief `derivation` records the complete transitive child closure (at most 128
identities, depth at most four), with every transitive source retained in the ordinary
cited-span/dependency set. Child fingerprints bind both generator inputs and accepted
prose, excluding timestamps/revisions. This catches different prose after an evicted
input is regenerated, and different source dependencies even when prose stays identical.
Admission rechecks the closure and full source identities in the existing brief writer
transaction. Search/get/context check the closure in their final source snapshot; a
source or child change makes ancestors ineligible immediately, before regeneration or
lexical cleanup. Original source remains exactly recoverable at each level.

One new redb `knowledge_cache` table retains completed inputs/outputs and provider usage;
cache insertion and accepted brief publication commit in **the same transaction**.
Its cap is configured from 1–128 MiB of serialized cache payload and at most 4,096 rows.
Key/database/allocator overhead is additional. Accounting updates transactionally;
deterministic eviction avoids another LRU index. Lowering the cap trims history before
new work. Current accepted briefs remain reusable after eviction or `knowledge purge-cache`.
An interrupted call before commit may be billed again; no exactly-once provider guarantee
is claimed. A committed result survives process death without another generation call.
Forget removes the selected brief, generated ancestors and their memoized copies in one
transaction; independent explicit user memory is untouched. Purge-cache removes only
memoization/backoff, not current briefs; source deletion makes retained outputs stale.

The schema-9 knowledge work lane remains independent of embeddings. One explicit run
reconciles only the bounded selected plan and acknowledges its captured frontier after
all selected nodes are current and lexical publication succeeds; edits after its fence
remain pending. Unselected paths have no generation obligation under that profile.
A different plan changes the frontier identity. This bounded metadata scan is the chosen
alternative to a new durable reverse-dependency index; it is not a corpus scan or a claim
of strictly affected-node-only metadata work. Failed nodes/ancestors preserve successful
children and pending work. JSON reports distinguish current, cached, generated and
pending nodes, incomplete publication, observed usage and unknown calls.

The standalone CLI retains the store's exclusive-owner rule. The MCP continuation
below reuses the same acceptance, cache and generation pipeline inside the existing
owner; there is no second database writer. The local continuation below adds an owned
model process. Shared memory headroom is now implemented as described below; CPU/global GPU
policies and real-model task/throughput evidence remain
T002 follow-ups. Dense brief vectors remain optional. [tests/knowledge.rs](../../tests/knowledge.rs)
exercises the real HTTP/CLI/store/retrieval boundaries with a local synthetic server;
see [validation](../../docs/validation.md#selective-knowledge-generation).

### Continuous MCP maintenance and shared admission

`mcp --knowledge-profile FILE` validates and binds a profile to the admitted primary
workspace once at startup. It enables `index {knowledge:"prepare"|"maintain"|"pause"}`
and `context {include_briefs:true}`; neither the profile nor watching alone starts a
model call. `--knowledge-maintain` explicitly starts continuous work. Modes are owner
local and do not persist across restart. Reference roots are never generated implicitly.
Derived context uses the primary's exact citations and the same delivery allowance;
multi-root requests must include the primary. Owners without a knowledge profile keep
their existing catalog and response shapes. The configured index tool advertises its
external effects and cannot mix knowledge control with source/semantic indexing arguments.

[maintenance.rs](../../src/knowledge/maintenance.rs) owns one bounded driver thread,
composing the CLI pipeline through foreground-prioritized store steps. Snapshotting
reconstructs each distinct source once per node (at most 32 sources / 64 MiB), then
releases the engine and read transaction before JSON construction, tokenization and HTTP.
At most one prepared request (128 KiB), bounded child briefs and one HTTP response
(256 KiB) are retained. Acceptance rechecks current sources, transitive child identities
and expected revision. Publication drains only selected brief keys; it cannot force
unrelated source indexing or acknowledge embedding work. No corpus-wide source walk or
additional job/dependency table is introduced; the schema remains **10**.

Generation waits for a 500 ms foreground quiet interval and outstanding watcher source
work before admitting HTTP. Status observations do not extend this interval. A shared
owner permit serializes background generation and embedding calls; the existing semantic
runtime slot also gives dispatched/waiting queries priority. Embedding preparation retains
its selected batch while generation occupies admission, instead of pausing on contention.
An already admitted generation request can delay a dense query up to the query's existing
ceiling; lexical fallback and exact evidence remain available. These are client admission
and bounded CPU/payload guarantees, **not** measurements or enforcement of external server
CPU/GPU headroom, residency or cancellation. An HTTP disconnect may leave external compute
running. The local profile below supplies owned worker isolation and resource telemetry.

Each owner window admits at most eight generation calls, 32,768 serialized-request
o200k tokens and 16,384 requested completion tokens over 60 seconds. Limits persist
through pause/resume within that owner; restarting begins a new owner window, so this is
not a durable account-wide quota or billing ceiling. Each pass, including admission waits,
is additionally bounded by 300 seconds; a deadline pauses with work retained until explicit
resume. Successful/current/cached nodes survive partial passes. Exhaustion enters
`budget_wait`; continuous mode resumes when the window expires, while `prepare` stops
after one bounded pass. No provider call is made for current or cached nodes in a pass.

The driver checks the durable knowledge obligation and waits on the existing source
notification when caught up or waiting for a missing source. Quiet waits read no store
rows. Notifications are hints; redb work/version fences remain authoritative, including
edits during generation and restart. Each dirty pass scans the bounded selected plan,
not strictly only affected metadata. Provider/input/publication failures pause with work
retained and require explicit resume; source staleness/missing-input waits can recover
on a subsequent notification. A pause admits no new call and accepts the one already
running if still current. Shutdown cancels HTTP, rejects uncommitted output and waits for
the driver before releasing the store. Rapid pause/resume must not lose a resume request.

`status.knowledge` combines cheap cache/work metadata with live state, named reason,
last-pass coverage and `completed_pass_usage`. Usage updates at pass boundaries; an
active `generating` phase has no claimed terminal provider usage. Observed provider usage,
unknown calls and local admission units remain separate. These counters are owner-local;
completed outputs and provider backoff remain durable. The configured endpoint fixture
exercises real stdio and shared HTTP consumers, not a real generation model.

### Owned local generation and resource controls

[local](../../src/knowledge/local/mod.rs) and the optional
[foundry-generate](../../src/bin/foundry_generate.rs) implement one explicit local provider
within the existing pipeline. This worker is necessary to own compute termination and
native resource limits; an HTTP disconnect cannot provide them. It adds a bounded private
pipe protocol, native isolation and a supervisor, with no listener, job table, schema
migration or new dependency. The core never links the decoder backend. HTTP profiles and
their existing cache identities remain compatible; remote and local selection are exclusive.

The local profile pins executable bytes, a single-file GGUF, the existing llama.cpp commit,
explicit built-in chat template, CPU/Metal choice, layer count, context/batch geometry and
threads. These computation inputs participate in generation identity. Paths, resource
ceilings and load/idle timeouts do not invalidate accepted completions. Resource bounds
and `development:true` are mandatory. Unsupported profiles refuse before artifact work.

Current/cache checks precede local acquisition. Streaming hashes create private, unlinked,
read-only executable/model snapshots in bounded buffers; no weight-sized Rust copy is
retained. Snapshots survive idle unload within a continuous run. Preflight failure is
memoized for that run to prevent repeated scans across selected nodes. Linux executes the
snapshot descriptor; macOS must retain and reverify the signed bundle path. Disk/page-cache
usage is additional to worker RAM. Snapshot paths never grant ambient source/store rights.

The worker loads through `llama_model_load_from_file_ptr`, formats the complete prompt
with the pinned template, tokenizes with the GGUF vocabulary and refuses prompt+output
overflow. Greedy grammar-constrained decoding produces the brief structure, while the
source owner still performs semantic-shape/citation/identifier/freshness acceptance.
No generic JSON-to-claim coercion or partial-output publication is allowed. Observed
prompt/completion counts use actual model tokens; o200k request admission remains separate.

Linux CPU uses inherited affinity, lowered scheduling priority, hard address-space and
all-thread lifetime CPU limits, and `no_new_privs` plus an architecture-checked syscall
allowlist before model loading. It permits descriptor I/O and in-process threads, while
denying path opens, network, exec, child processes and changes to resource/affinity limits.
No cgroup or Landlock availability is assumed. macOS requires a non-root App Sandbox
development bundle, hard CPU/process limits and supervised physical footprint. Metal
additionally monitors process-local allocations/reserve against its recommended working
set at 20 ms intervals, including model load. This is not a hard GPU or global-memory quota.
See the precise [enforcement table](../../docs/deployment.md#experimental-owned-generation-controls).

Frames are length-prefixed strict JSON, at most 512 KiB, with full config readiness and
monotonic request/reply identity. Only one request is active. Load/call wall deadlines,
cancellation, malformed replies, usage overruns or resource failures terminate/reap the
process before background admission is released. Normal pause accepts the current call
then unloads; shutdown cancels it. Owner liveness and native owner-death containment stop
orphan computation. Idle and budget waits release model/device residency while retaining
verified snapshots. Local PID/call/verification/peak/stop observations are available outside
the engine mutex. RAM is sampled; GPU peaks are reported at completed calls, not global
live telemetry. A worker CPU-lifetime kill reports exit without inventing terminal usage.

**Failed-computation accounting continuation (2026-10-10).** A bounded real CPU
repair-selection comparison exposed that reaching the output allowance lost actual
token counters. The native decoder now retains prompt/completion counts after fully
processing the prompt and exhausting its sampled output allowance, even though no
admissible result exists. Optional `usage` on the existing private `Failed` reply is
checked against the admitted context/output bounds and retained as
`local_worker.last_call_usage`. The supervisor clears that observation before each
call. Run/pass totals include these failures before recording provider backoff; they
never publish/cache partial prose. Old workers may omit usage, which remains unknown;
older strict readers may refuse the extension. Executable pinning still identifies
the exact worker. Cancellation, CPU kills, failed prefill and other incomplete work
retain unknown usage rather than estimated zeroes. Fixtures reject invalid failure
counters and verify failed work creates no brief. This fixes accounting, not model
quality; the fixed repair-selection workloads and negative results live in validation.

This controls one generation worker. Opt-in shared memory admission now accounts for
combined configured embedding/generation ceilings, as described below. Global GPU use,
native CPU utilization, thermal pressure, battery draw and macOS hardware/package acceptance remain open, alongside
verified task utility and large-repository economics. Linux synthetic native-denial and
lifecycle tests plus the real-model smoke are separate evidence in
[validation](../../docs/validation.md#owned-local-generation-and-resource-controls).

#### T002 shared memory admission — 2026-10-10

Implemented experimentally in [model_resources.rs](../../src/model_resources.rs), shared
with 009 preparation through the existing MCP owner. `--model-resources FILE` declares
resident worker-ceiling bytes, observed headroom reserve and recovery margin. It creates
no durable rows, service, model/cache identity or schema change. Reservations cover
embedding and local generation; HTTP remains client admission only. Learning stays frozen.

Reserve embedding before startup using the same parsed profile passed to acquisition.
Its provider thread owns the reservation until worker drop/reaping, including late calls.
Generation owns its reservation across calls until idle/error/pause/shutdown unload, and
pressure can evict its idle process without discarding verified artifact descriptors.
Neither early caller return nor a source transaction releases a live process's reservation.
A static combined-ceiling failure pauses with `model_memory_budget` before calls/usage;
temporary pressure/headroom/unknown-observation waits preserve pending work and resume
within the existing pass deadline. They never enter provider-failure backoff or invent usage.

Linux samples host `MemAvailable` and visible cgroup v2 ancestor high/max headroom; macOS
conservatively samples free VM pages (hardware acceptance outstanding). Unknown observations
defer. A low-memory latch clears only above reserve plus margin. New residency also needs
headroom for its ceiling; samples are memoized for one second and invalidated on reservation
changes. Status reads only observations. Source/current/cache reuse bypasses model admission;
foreground embedding stays available, while new background embedding/generation calls wait.

This is admission using configured worker bounds, not a global/hard quota. It excludes
owner/index memory, artifact storage/page cache, hidden cgroup ancestors and other processes
from the reservation sum. Native per-worker enforcement is unchanged. In-flight work is not
preempted by this policy; embedding residency is retained for foreground use. Linux host
CPU admission is implemented below; native CPU/global GPU pressure, thermal/battery
adaptation and embedding eviction are still follow-ups.
Tests cover startup refusal, combined reservations, pressure hysteresis, provider lifetime,
idle eviction/reaping, cache reuse, pending work, pause/shutdown and foreground service.
See [configuration and limits](../../docs/experimental-context.md#shared-model-memory-admission).

**Optional CPU admission follow-up (2026-10-10):** `background_cpu` adds pause/resume
percent thresholds to the same resource policy. No new worker, table or scheduler.
Linux reads the first bounded `/proc/stat` aggregate, compares monotonic samples at
one-second intervals and excludes already-counted guest time. The first/failed/reset
observation defers by name. A pressure latch clears only at/below the resume threshold.
Both background embedding and generation use the gate, including initial local generation
admission. Foreground embedding and cached/current results remain available. CPU pressure
does not evict idle generation; memory pressure still can. Status reads cached observations.
This is **host aggregate** contention admission, not a cgroup CPU quota, affinity-aware
utilization bound or a hard CPU limit. macOS CPU sampling is unavailable in this slice;
explicitly enabling this optional field there defers by name. Existing native worker
thread/CPU-time controls remain. Global GPU, thermal/battery, embedding eviction and
real-device throughput/combined-model acceptance remain open.

Pinned backend reference: llama.cpp `b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea`,
`include/llama.h` / `llama_model_load_from_file_ptr`, `llama_chat_apply_template`,
`llama_decode`, `llama_sampler_init_grammar`; `src/llama-model-loader.cpp` and
`src/llama-mmap.cpp` / borrowed `FILE*` lifetime;
`ggml/src/ggml-metal/ggml-metal-device.m` / `ggml_metal_device_get_memory` and
`ggml/src/ggml-metal/ggml-metal.cpp` / backend device memory observation. Foundry intentionally
uses its own bounded Rust supervisor, descriptor-only CPU profile and fixed brief grammar,
not the upstream server or tool loop. Relevant Foundry executions are
`local_worker_enforces_os_denials_and_reuses_source_bound_outputs` and
`local_real_cpu_generation_accepts_a_cited_brief_and_reuses_it`; upstream Metal behavior
is inspected reference, not executed hardware acceptance in this Linux session.

## Pinned reference and differences

Prakarana at `3143213d63c22a097c77fb229bfc910e08c6e01e`:
`src/internal/enrich_worker.cpp::process_one`,
`src/internal/enrich_structured.cpp::process_structured`,
`src/internal/spine_tree.cpp::{render_card,write_spine_tree}`;
`tests/contract/test_structured_docfact_{parse,lane,retire,delivery,feeder}.cpp` and
`test_door_wiring.cpp`. Source/tests inspected, not executed here. Its doc-fact lane
and structured tier motivate this scope; its sidecar queue, C ABI and custom HTTP
client are not dependencies. See the pinned links and small-fixture result limits
in the ecosystem plan. Existing L3/L4 research continues to inform task state and
recoverability; it does not replace this repository-knowledge work.


T002 real-model continuation (2026-10-10): the explicitly supplied Qwen2.5-Coder-0.5B
Q4_K_M fixture now runs initial and changed-input generation, then reopens/reverts and
reuses the original cached brief with the GGUF absent. Two calls have complete worker
usage; revert has zero calls/launches/verified bytes. Linux CPU execution and reuse are
verified. Model/recipe task utility, long-context embedding comparisons, combined model
pressure and macOS/Metal acceptance remain open. See
[validation](../../docs/validation.md#evidence-coverage-host-application-and-real-cpu-maintenance--2026-10-10).
