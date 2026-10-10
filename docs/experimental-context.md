# Experimental task context

## Pipelined source ingestion

Ordinary CLI/MCP `index` now overlaps source reads and hashing with ordered source
commits and publishes a lexical page after every 128 accepted candidates. A single
Engine-owned pool serves reads and lexical builds and survives later refresh calls;
its size is fixed at creation to the smaller of available CPU parallelism and eight.
The source scan and final pending-work drain still report exact committed counts.

At most eight read/results occupy slots, each containing at most 2 MiB plus one
sentinel byte; completed results keep their slots until acceptance. Before committing,
Foundry rechecks the held root/file and the current name. An edit, rename or unsafe
replacement during preparation preserves the previous accepted record and defers
unseen deletions. Cancellation/restart reuses committed sources and lexical pages;
buffered, unaccepted reads are discarded. Normal explicit re-indexing still reads
the tree; native watching remains the path for targeted updates.

The read payload limit and existing 64 MiB lexical hand-out limit are separate and
exclude parser, index, thread-stack and allocator overhead. Explicit MCP indexing
still occupies its engine slot; this change adds early durable publication, while
foreground admission continues to follow the existing watcher/owner rules. Source
transactions remain individual. Continuous embedding is separately enabled below;
continuous brief regeneration is separately enabled with an explicit knowledge profile
and maintenance option below. Sandbox tests do not establish 80,000-file throughput.

## Continuous semantic maintenance

On an owner already serving an admitted semantic profile, call:

```json
{"semantic":"maintain"}
```

This `index` action keeps preparation enabled for the owner's lifetime. For explicit
startup/resume, use `mcp --semantic-profile PROFILE --semantic-maintain`, with the
profile's required isolation flags and optional `--semantic-cache-cap BYTES`. Add
`--watch` for filesystem updates; without watching, an explicit `index` accepts edits.
Watching alone starts no model. `index {"semantic":"pause"}` stops further admission;
`prepare` remains a one-shot full reconciliation.

Schema 9 stores independent semantic and future generated-knowledge obligations with
each source commit/delete. Each lane retains at most 1,024 paths / 1 MiB path text;
overflow leaves a durable full-reconciliation marker. Cold/profile-change/overflow
runs walk the corpus. Normal changes select eight exact paths at a time, with fair
rotation and conditional acknowledgments after successful publication. Edits during
inference stay pending. Unchanged exact inputs, including reverts, reuse cached vectors.
Renames change rendered addresses and can require new embeddings. The generated-knowledge
lane records obligations but runs no LLM generator yet.

When caught up, semantic status says `waiting`: the same preparation thread waits for
committed-source notifications without repeated index/source/cache scans. Errors or
resource limits pause by name and retain pending work; resume explicitly after fixing
the cause. Status polling does not extend the foreground batch throttle. A restarted
owner prepares automatically only when `--semantic-maintain` is explicitly supplied.

Generation v5 keeps one immutable base ANN and one bounded delta. Small edits rebuild
only the delta; location-only changes build and write no ANN data. Retired base keys
are filtered before query candidate selection. Compaction runs at bounded churn/byte
thresholds, and an atomic manifest switch preserves a readable generation on failure.
Publication I/O/index failures in MCP retry after 250 ms, 1 s and 3 s; status exposes
`bootstrap.publication_backoff_ms`, `publication_retries` and `publication_error`.
Exhaustion pauses with pending work intact. No embedding is repeated by these retries.
`generation_builds`, `delta_publications` and `mapping_publications` distinguish work.

Within an open owner, publication now refreshes only paths affected by source,
partition or cache commits. Bounded dirty-state overflow, changed-input restart, identity
changes and repair reconcile from redb. Publication and serving share verified-file receipts
and at most two immutable resident ANN segments: unchanged segments are neither
rehashed nor restored when locations or the other segment change. Explicit audits
still hash; changed file versions are reverified on refresh. Warm queries detect a
changed manifest or prepared profile without needing an invalidation hint.

Membership and location bases are immutable; one location overlay holds at most 4,096
changed keys / 32 MiB. A larger change compacts locations without rebuilding ANN.
Decoded metadata and persistent mapping trees are shared across publications and
requests. Unchanged restart publication verifies files and checks a transactional
revision instead of scanning partitions/cache layouts. The first changed-input restart
still reconciles; cold verification and compaction remain corpus-sized. These fixtures
do not establish total edit cost at 80,000-file scale.

## Progressive semantic bootstrapping

With an admitted semantic profile, start preparation on the existing MCP owner:

```json
{"semantic":"prepare","priority_paths":["src/retrieval","src/main.rs"]}
```

These are arguments to `index`. Priority paths name already indexed files or
subtrees, relative to the workspace; they do not enroll new paths. Up to 16 scopes
rotate in request order, alternating with the ordinary source walk. Repeating
`prepare` with a list replaces preferences at the next wave; omitting it retains
preferences for that owner, and `[]` clears them. A new owner starts with none.
Pause through `index {"semantic":"pause"}`; explicit preparation resumes from the
accepted partitions and vector cache. Nothing auto-starts on restart.

The equivalent bounded CLI run is:

```sh
foundry --store STORE semantic prepare --profile PROFILE --budget-seconds 300 --priority src/retrieval --priority src/main.rs
```

Preparation now partitions, selects and embeds in waves, with at most eight new
source bodies per wave. The first committed batch can become searchable while later
sources remain unpartitioned. Publication uses the existing USearch generation:
first batch, approximately doubled new-vector coverage, and completion/pause. Its
mapping visits prepared rows, so cold publication avoids a whole-source census.
No new segment format, job store or worker process is introduced. MCP overlaps one
bounded prefetch with one provider call; overlapping priority inputs are deduplicated
before another call. Foreground admission retains one model slot; the opt-in geometry
and body recipe below extend input sizing and actual-token batch packing.

The semantic CPU pool starts only when a new partition needs computation and is
reused across waves (and MCP prepare runs), with at most eight workers. Fresh
verified cards pass directly to selection under a 2 MiB retained-payload limit per
source, at most 16 MiB per wave; larger inputs use on-demand rendering. A source or
recipe change cannot reuse the old handoff. These payload limits exclude parser,
tokenizer, thread-stack and allocator overhead.

CLI preparation now acquires a model only when a missing input needs embedding and
the budget admits startup. Cache-only replay and mapping-only publication need no
worker; profiles/artifacts are still verified. If startup fails after a CPU wave,
accepted partitions and prior cached coverage remain reusable. No new provider
observation is reported for a run that never attempted acquisition. MCP's existing
resident model for query serving is unchanged.

`status.semantic` is a cheap observation of the last recorded publication. Its
`coverage_observation` is `current`, `stale` or `unknown` against source/profile identity;
stale/unknown current counters are null. `index.validation` is `not_checked` and
availability is unknown until actual use or audit. Explicit MCP
`status {"semantic_audit":true}` and CLI `semantic status --audit` retain the full census
and generation validation. Inspect `unpartitioned_sources`, `missing_units` and
`searchable_current_units` with that observation boundary in mind. Context stays
`partial` until coverage is complete; source edits immediately invalidate old vector
locations, and lexical retrieval remains available. Its additional `bootstrap`
object reports waves, priority waves, partition/render/provider/publication wall
milliseconds, cache reuse, embedded units, input tokens, generation builds and time
to first searchable coverage. MCP exposes live snapshots; the last finalized run is
also stored for CLI status/restart. These timings can overlap and exclude unsampled
in-flight time; they are not additive CPU utilization or device profiling. Examined
unit/reuse counters can revisit priority sources; coverage counts remain unique.

Publication refuses oversized logical mapping metadata (256 MiB), assembled f32
vectors (2 GiB), or serialized ANN data (4 GiB), preserving the accepted cache and
previous generation. These are payload bounds, not total-process RAM guarantees.
A generation still rebuilds in memory. Real-device throughput, larger model batches,
80,000-file performance and task-quality improvements remain unmeasured in this
Linux sandbox; integration tests use synthetic embedding providers.

## Model-aware body/parent chunking

Semantic profile v2 keeps address cards by default. To encode behavior inside source
bodies, add these fields to a copy of an existing verified profile:

```json
{
  "representation": {"kind": "body-v1", "parent_tokens": 64},
  "card_tokens": 1024,
  "batch": 8
}
```

This is a profile fragment, not a complete model profile. Short units stay whole;
large units split into exact source windows with bounded parent/header context.
Unsupported syntax and configuration use source windows too. Parent text is fitted separately
to at most one quarter of the configured card allowance; templates and special tokens
also count in the complete input. Parent context is an input prefix, not a generated
brief or a separate retrieval result. Returned citations identify child source spans,
under the existing response allowance.

With a legacy descriptor, preparation packs actual input lengths within 2,048 total
tokens per call and the configured sequence count. It does not reserve 1,024 tokens
for each short input. CLI preparation and continuous MCP maintenance share this rule.
Body-only edits change affected inputs; included parent-header changes also invalidate
their children. Same-language renames update provenance without re-embedding identical
text. Repeated runs, reverts and switching back to cards-v1 reuse retained cache rows.
Do not purge to switch recipes. Coverage stays partial while missing inputs populate.

A model/backend that supports a larger window can explicitly pin new descriptor
fields, for example:

```json
{
  "adapter_revision": 2,
  "geometry": {"v": 1, "input_tokens": 8192, "batch_tokens": 8192, "sequences": 8}
}
```

This fragment belongs inside `descriptor`. It requires a worker built with geometry
support; updating the worker executable also requires updating its verified hash.
Keep the existing artifact/tokenizer/template/pooling/dimension claims accurate.
`card_tokens` can then be raised within the input limit. Explicit geometry changes the
function digest, so these vectors have a separate namespace. Without geometry, adapter
revision 1 and the exact old digest remain. Worker startup checks the GGUF training
window and actual context/batch/sequence capacities. Unsupported claims and oversize
inputs refuse; nothing silently truncates. The hard ceiling is 32,768 input/call tokens
and 32 sequences, not a promise that any selected model supports those values. Larger
contexts also consume more device memory under the existing supervised memory limit.

No default model profile is enlarged by this change. Sandbox evidence uses real
partitioning/tokenization/storage/retrieval with deterministic provider vectors;
long-context Metal parity, throughput and task quality are still unexecuted. Source
plans are bounded by 2 MiB per source, 16,384 windows and 16 MiB accumulated context
text, plus parser/tokenizer/array overhead. Fresh-card retention remains 2 MiB per source
with bounded fallback. Separate parent/file/area vectors and late chunking are not
implemented. See [009 T009](../specs/009-optional-semantic-retrieval/spec.md#t009--use-model-owned-geometry-and-contextual-representations)
and [validation](validation.md#model-aware-bodyparent-inputs--2026-10-10).

## Incremental workspace watching

After initializing or bootstrapping a disposable store, enable source maintenance on
its long-lived MCP owner:

```sh
foundry --store STORE mcp --root ROOT --watch
```

The same flag works with the existing shared HTTP owner. It watches only the admitted
primary root; reference stores remain read-only. The flag defaults off and
adds no MCP tools. Call `status` to inspect its `watch` object: phase, pending paths
and index work, overflow count, source reads/changes/deletions, reconciliation counts,
last successful drain time and errors. Source freshness still means the committed
snapshot, with asynchronous disk-to-store delay visible through this status.

Native notifications coalesce for 75 ms (at most one second). Known file edits read
only that file. New paths and directory changes use scoped walks with the same ignore
rules as indexing; new-file admission can enumerate sibling metadata but does not
read sibling bodies. Ignore-policy changes, notification overflow and every owner
restart trigger a streaming root reconciliation. Quiet ticks do not reread source
bodies. Changes immediately invalidate old handles and dependent graph/vector/brief
evidence through existing source commits; lexical updates publish in small batches.
Generation and embedding inference do not run on the watcher path.

The pending hint queue holds at most 2,048 paths or 1 MiB of path text, then collapses
to one dirty-root marker. Each owner step handles one walk entry/file, at most 128
retirements, or eight lexical keys; foreground calls have priority between steps.
These are work and queue bounds, not whole-process RAM or hard latency guarantees:
filesystem calls, parsers, database commits and the notification library's OS watch
registrations have their own costs. Source work gets eight steps before a waiting
semantic step may reserve the owner briefly; foreground operations retain priority. Shutdown cancels maintenance and releases the existing store owner.

Failed reads preserve accepted bytes; an incomplete reconciliation cannot retire
unseen sources. Failures back off from two to 30 seconds and remain visible. A native
registration failure refuses startup; a later backend error stays visible in
`backend_error` even after a successful source reconciliation. Restart the owner to
re-register a failed backend. No durable event log or periodic whole-tree polling is
added: unreported notification loss is recovered by explicit indexing or restart.
Ignore files outside the watched root (for example global Git excludes) likewise
require an explicit index or restart. Native event integration is tested on Linux;
macOS and network-filesystem event behavior remain unverified.

## Recoverable task context

This branch adds recoverable task state beside retrieval. Start with a disposable
store. New stores use schema 11; an existing store needs an explicit
`foundry --store STORE upgrade-store --to 11`. Take a closed-store backup first if
using an existing store: schema 10 and earlier binaries cannot open schema 11, and rollback means
restoring that backup. This work does not upgrade stores automatically.

The core works through `foundry --store STORE task-context`, which accepts one JSON
request on stdin and prints one compact JSON result. Enable the identical MCP tool
with `foundry --store STORE mcp --root ROOT --experimental-task-context`. `--no-memory` refuses promotion into project memory. The ordinary
MCP catalog is unchanged without the flag. Task IDs are caller-chosen, scoped to the
primary store's workspace ID (shown by `status`). They are not per-user access controls.

Every request has this shape:

```json
{"workspace_id":"<workspace ID from status>","task_id":"retry-investigation","command":{"op":"create","title":"Repair retry backoff"}}
```

Save the returned `revision`. Every mutation after creation names the current
`expected_revision`; a conflict means inspect the live record before retrying. A
single `apply` may combine up to 32 edits. For example, place this inside `command`,
using the revision returned by `create`:

```json
{
  "op": "apply",
  "expected_revision": 1,
  "edits": [
    {"op":"add","entry":{"id":"limit","kind":"constraint","text":"Keep the retry limit at three.","author":"user","provenance":"explicit task instruction","source_links":[]}},
    {"op":"add","entry":{"id":"trace","kind":"observation","text":"Tool reported retry-after: 73 seconds.","author":"agent","provenance":"tool run test-service/revision-42","source_links":[]}},
    {"op":"checkpoint","name":"before-fold"},
    {"op":"fold","entry_id":"trace","summary":"Recover the original retry-after before changing backoff."}
  ]
}
```

Entries distinguish requirements, constraints, observations, hypotheses, decisions,
rejected attempts, questions, remaining work and artifacts. Author and provenance
are caller attribution, not verification. Original text is immutable. A new entry of
the same kind may name `supersedes`; older evidence stays available through `peek`
but is historical and excluded from the current view. Unlinked conflicting claims
coexist until the caller explicitly resolves them.

| Command inside `command` | Effect |
| --- | --- |
| `{"op":"inspect"}` | List entry identities, hashes, byte lengths, provenance, source freshness, view state, checkpoints, dismissals and promotions. |
| `{"op":"render","max_bytes":4096}` | Return selected active entries and explicitly labeled caller summaries. Refuse if the complete compact JSON plus newline exceeds the byte budget. |
| `{"op":"peek","entry_id":"trace","offset":0,"bytes":4096}` | Return exact original UTF-8 text, digest and next byte offset. Continue until `next_offset:null`. Peeks do not edit the view. |
| `{"op":"apply","expected_revision":2,"edits":[{"op":"expand","entry_id":"trace"}]}` | Expand an original. Use `fold`, `select` or `pin` to change its representation/selection. |
| `{"op":"apply","expected_revision":3,"edits":[{"op":"restore","name":"before-fold"}]}` | Restore the checkpoint's views. Later evidence and current supersession remain; external tool effects are not undone. |
| `{"op":"export"}` | Export one exact task record, including originals, views, checkpoints and promotion identities. CLI only. |
| `{"op":"forget","expected_revision":4}` | Delete the task and unchanged promoted copies atomically. Reports contain identities, never forgotten text. |

Requirements and constraints stay selected, pinned and expanded. You can replace one
through explicit same-kind supersession, but cannot silently hide it with a fold or
view restore. Source handles validate at admission; reads report fresh/stale/missing
against the indexed snapshot. A retained observation remains historical evidence when
its linked source changes. Unindexed disk edits are not silently certified fresh.

For selective reminders, add a `rejected_attempt` entry with an `action_key` encoding
the tool, arguments and relevant environment version. Ask
`{"op":"advise","action_key":"retry-loop/args-v1/env-42","max_bytes":4096}` before
repeating it. Foundry returns at most one matching, active, undismissed failure that
fits, with its original hash and attribution. Stale source-linked failures are
excluded. Otherwise it returns `silence:true`. This is exact matching, not semantic
failure or constraint detection. A CAS `dismiss` edit acknowledges a displayed entry;
dismissal survives restart and view restore. There is no autonomous polling or extra
model call, and no claim that a host read the reminder.

To find a retained observation when its ID has been forgotten, search the originals:

```json
{"op":"search","expected_revision":5,"query":"retry-after","limit":8,"max_bytes":4096}
```

Recovery search includes folded and deselected entries. It uses literal matching
with ASCII case folding, ordered by entry ID. Each hit supplies the original hash,
matching byte range, an exact UTF-8 excerpt and its offsets, attribution and current
source-link statuses. Use the returned entry ID and offset with `peek`. When
`next_after_id` is non-null, pass it as `after_id` with the same task revision to
continue. A changed revision refuses the page so you can inspect and restart the
search. Superseded entries are excluded unless `include_history:true` is explicit.
`matching_entries` counts all eligible matches before applying the page cursor.

Task-aware source retrieval now combines the selected view with source evidence:

```sh
foundry --store STORE context 'How is retry delay calculated?' \
  --task retry-investigation --task-revision 5 --tokens 2048
```

On an MCP owner with `--experimental-task-context`, call the ordinary `context` tool:

```json
{"query":"How is retry delay calculated?","task_id":"retry-investigation","task_revision":5,"tokens":2048}
```

Use the current revision returned by your own mutations; the example's `5` is only
illustrative. The response has the normal context header, one `task-view:` JSON line
with a digest and the complete selected view, then cited source/graph/project-memory
items. **One token budget covers all of this text.** Requirements, constraints and
other selected entries cannot silently disappear to make room for source. If the
view will not fit, explicitly fold or deselect an unprotected entry before retrying.
The request itself never edits the view. Source omissions remain visible in the
normal context header; a fitting task view does not guarantee a code answer fits.

The task revision, selected representations and source-link freshness are checked in
the source query's final snapshot. The digest binds the delivered view, including
those freshness statuses. It does not acknowledge host consumption. Multi-root
context includes only the primary task, once; the primary must be selected and
serving. Reference sources still use their own snapshots. Optional `--include-memory`
and experimental behavioral packing work with the combined response.

Promote a selected decision explicitly:

```json
{"op":"promote","expected_revision":5,"entry_id":"chosen-backoff","memory_id":"retry-policy","author":"operator","provenance":"Explicitly retained project decision"}
```

Each decision and memory ID can appear only once in a task’s promotion lineage, even
after the project record is deleted. Only active `decision` entries qualify, within the existing 16 KiB memory text limit.
Promotion rechecks source links and commits the project record, replay key, task
lineage and revisions together. It retains task/entry/digest provenance and becomes
available to ordinary `memory get/search` and opt-in memory context. No task text
enters project memory, source search or training automatically.

An unchanged promoted copy is removed with task forget, even if its search document
has not yet been deleted. If someone independently updates or recreates the project
record, task forget refuses atomically: resolve/forget that record through `memory`
first. Likewise, explicitly resolve/forget a live promoted copy before superseding
its task decision. This makes correction visible. Deletion is logical: prior exports,
backups, transcripts and provider caches are outside this store's control.

Limits are 128 live tasks, 128 entries per task, 256 KiB text per ordinary entry,
16 KiB per protected entry, 2 MiB serialized task and 16 named checkpoints. Retention
lasts until explicit forget; there is no automatic expiry. CLI requests are at most
1 MiB. MCP retains its 64 KiB input-frame and 256 KiB serialized-result limits, so use
CLI for large observations and exports. Peek pages hold 4–65,536 payload bytes; JSON
escaping/envelopes add overhead. Standalone render, search and advice budgets count JSON bytes. Task-aware `context`
uses the existing o200k token budget for the complete delivered text; provider token
usage remains unknown. Views exceeding the budget require an explicit fold or selection
change; protected evidence is never silently dropped.

Foundry delivers a view; it does not replace a host conversation. Results say
`host_application:"unknown"`. Real-host application, task-success improvements and
provider economics remain unmeasured. The sandbox lifecycle suite exercises actual
CLI restarts, real SDK MCP calls, concurrent revisions, source staleness, exact
recovery and abrupt exits around commits. See [validation](validation.md),
[008](../specs/008-scoped-durable-memory/spec.md) and
[003](../specs/003-agent-retrieval-context/spec.md).


## Source-bound derived briefs

010 T001 adds explicit imported repository knowledge through the CLI and library.
This is separate from task state and project memory. No model is called yet; the
same acceptance seam is intended for the future generator. First index the sources
and obtain exact v2 source handles. `foundry --store STORE brief` accepts one JSON
request on stdin:

```json
{
  "workspace_id": "<full workspace ID from status>",
  "command": {
    "op": "put",
    "brief": {
      "cited_spans": ["<primary implementation handle>", "<test handle>"],
      "recipe_sha256": "<64 lowercase hex characters>",
      "model": "<generator model or synthetic fixture identity>",
      "artifact_sha256": "<64 lowercase hex characters>",
      "prompt_sha256": "<64 lowercase hex characters>",
      "generated_at_unix_ms": 1791504000000,
      "purpose": [{"text":"Enforce queue capacity.","citations":[0],"identifiers":[]}],
      "behavior": [{"text":"Remove oldest queued entries until length fits capacity.","citations":[0],"identifiers":[]}],
      "provides": [],
      "consumes": [],
      "constraints": [],
      "tests": [{"text":"Exercise capacity eviction.","citations":[1],"identifiers":[]}],
      "unknowns": ["Thread safety is unknown."]
    }
  }
}
```

Replace the placeholders and descriptions with the actual source-bound generator
result. Claims use zero-based citation indices; declared identifiers must appear in
their cited bytes. Byte presence is not semantic verification. Provenance identifies
the asserted generator inputs; import does not prove the model/artifact was executed.

Keep the returned `id` and `revision`. One current record is retained per primary
source and recipe. Identical input/output replay is a no-op (a changed generation
timestamp alone is ignored). Different output for identical inputs conflicts. A
new input replaces the current record only with `expected_revision` alongside
`brief`; source changes during generation still cause refusal.

Use these scoped commands inside the same `workspace_id` envelope:

| Command | Result |
| --- | --- |
| `{"op":"get","id":"<id>"}` | Current structured record, or a content-free stale status. |
| `{"op":"search","query":"capacity eviction","limit":8,"max_bytes":262144}` | Derived vocabulary matches, grouped by source and validated against current rows/dependencies. `max_bytes` is 1024–262144, including the CLI newline. |
| `{"op":"export","id":"<id>"}` | Full retained record, including stale text explicitly labeled `eligible:false`. |
| `{"op":"forget","id":"<id>","expected_revision":1}` | Remove retained text and queue search cleanup; explicit project memory is unaffected. |

A source/dependency edit or deletion in Foundry's authoritative snapshot immediately
excludes the old brief from search/context, even before search cleanup. Unindexed disk
edits are not observed by this slice. CLI mutations attempt immediate derived cleanup;
failed indexing leaves durable pending work for the ordinary drain/repair paths.
`status` reports total pending work; brief search also reports `pending_briefs`.
Exports/backups and data already delivered to callers are outside live-store forget.

To use conceptual discovery with bounded evidence:

```sh
foundry --store STORE context 'capacity eviction' --include-briefs --tokens 4096
```

This augments anchorless queries, delivers the cited primary source, and includes
`brief:` JSON labeled `kind:derived` only if that source and the optional orientation
fit the same budget. Test/dependency handles can be recovered with `retrieve`.
Exact-name context keeps its original path. A matching summary is not proof that an
answer is true or that the evidence is complete. A selected `--task` and
`--include-memory` may share this same allowance. Explicit generation and area rollups
are described below, including opt-in continuous MCP maintenance and brief delivery.

Requests are capped at 256 KiB, claims/identifiers/unknowns at 16 KiB of text, and
citations at 32 spans. Stored records are capped at 512 KiB including full dependency
paths. Search refuses when its first matching record cannot fit the requested byte
allowance; `get`/`export` by ID can recover a larger bounded record. No output is
silently cut inside a claim.


## Selective knowledge generation

`knowledge prepare` generates only an explicitly selected set of source briefs and
area summaries through a configured OpenAI-compatible Chat Completions endpoint.
Source indexing alone never launches generation. This experimental CLI owns the store
exclusively: stop the MCP owner before using this standalone command. To generate inside
a running MCP owner, use the continuous workflow below. Schema 10
is required; upgrade an existing store explicitly as described above.

Create a profile with the actual workspace ID from `status`, the provider's exact
response model name, and a lowercase 64-hex deployment/artifact digest. The digest is
your deployment pin; the HTTP response cannot attest which weights the server ran.
This illustrative profile requires replacing its placeholders and selecting paths
present in your indexed repository:

```json
{
  "v": 1,
  "workspace_id": "<workspace-sha256>",
  "recipe": "behavior-v1",
  "model": "<configured-response-model>",
  "artifact_sha256": "<deployment-sha256>",
  "endpoint": "http://127.0.0.1:8000/v1/chat/completions",
  "allow_remote": false,
  "source_scope": ["src/queue.rs", "tests/queue.rs", "Cargo.toml"],
  "instructions": "Describe capacity handling and its tests; preserve uncertainty.",
  "max_request_tokens": 8192,
  "max_request_bytes": 131072,
  "output_tokens": 1024,
  "call_seconds": 60,
  "cache_bytes": 16777216,
  "nodes": [
    {
      "id": "queue",
      "sources": [{"path": "src/queue.rs"}, {"path": "tests/queue.rs"}]
    },
    {
      "id": "queue-area",
      "sources": [{"path": "Cargo.toml"}],
      "children": ["queue"]
    }
  ]
}
```

```sh
foundry --store STORE knowledge prepare --profile knowledge.json --budget-seconds 300 --max-calls 8 --request-tokens 32768 --output-tokens 8192
foundry --store STORE knowledge status
foundry --store STORE context 'capacity eviction' --include-briefs --tokens 4096
```

Paths are relative to the admitted workspace and must match the explicit scope, as an
exact path or directory prefix. Each source may include `start` and `end` byte offsets;
otherwise the complete indexed body is selected. Endpoints must end in `/chat/completions`.
Literal loopback HTTP(S) is permitted. Remote HTTPS additionally requires
`allow_remote:true`; an optional `token_env` names an environment variable containing
bearer auth. No redirects, ambient proxies, automatic retries, model download or model
process launch are performed. No actual provider is configured by this example.

A plan has at most 128 nodes, eight direct children per node and four area levels.
The first span anchors the result; parent inputs include child briefs and their original
source spans. All transitive evidence must fit 32 cited spans and 128 KiB of serialized
request bytes. Oversized source requires explicit smaller spans; no source text is
silently truncated. Accepted claims retain exact citations, full source hashes and
model/recipe identity, and remain labeled derived rather than verified source facts.

Request admission uses **o200k tokens over the serialized JSON request**, separately
from the provider's tokenizer and chat framing. `--request-tokens` caps that per-run
sum; `--output-tokens` caps the sum of requested completion ceilings. Reports separately
show observed provider prompt/completion usage and calls with unknown usage. An absent
usage report never becomes zero-cost evidence. These counters do not establish billing,
full-task savings or output quality. A timeout closes the client request; an external
service may continue computing. Shared device/CPU admission and an owned isolated local
generator are not implemented in this CLI slice.

Unchanged current nodes reuse source/child metadata without reading or tokenizing their
bodies. Changed inputs regenerate only missing results in the selected plan and affected
ancestors; reverting inputs can reuse exact completed results from the persistent cache.
Parent freshness binds every transitive source hash and each child's accepted text, even
when source changes leave the child's prose unchanged. Selection scans at most 128 nodes,
not the full repository; no persistent reverse-dependency index is added.

The bounded cache retains completed outputs atomically with accepted briefs. Its cap is
1–128 MiB of serialized values, with at most 4,096 entries (database/key overhead is extra).
Lowering the cap evicts cached values even on an otherwise unchanged run; current briefs
remain reusable after eviction. Interrupted or budget-limited preparation prints a
partial report and exits with `knowledge_incomplete`. Re-run the same command to resume;
completed current/cached nodes consume no provider-call budget. Provider/invalid-output
failures persist a 30-second backoff, while current and cached results remain available.
The independent knowledge-work obligation is acknowledged only after all selected nodes
are current and lexical publication succeeds. It is not corpus-wide generation coverage.

`knowledge purge-cache` removes historical completions and backoff, preserving current
briefs. `brief` forget also removes cached copies and generated ancestors that depend on
the forgotten brief, in one transaction; explicit project memory is unaffected. Exports,
backups and previously delivered text remain outside live-store deletion guarantees.

The local HTTP fixture verifies transport, provenance, reuse and recovery, using synthetic
responses. Real-model quality, long-repository throughput and hardware headroom
enforcement remain 010 T002 work. The MCP continuation below now
implements continuous scheduling and shared client admission. See
[the implemented contract](../specs/010-source-bound-derived-knowledge/spec.md#t002-implementation-decisions-and-boundaries).

## Continuous knowledge maintenance

Use the same bounded knowledge profile with the MCP owner:

```sh
foundry --store STORE mcp --root REPO --knowledge-profile knowledge.json --knowledge-maintain --watch
```

The profile admits one provider and source scope for the primary workspace. It is loaded
once; restart to change it. Omitting `--knowledge-maintain` leaves generation paused.
Watching alone does not generate. An admitted owner accepts these separate index actions:

```json
{"knowledge":"maintain"}
{"knowledge":"pause"}
{"knowledge":"prepare"}
```

`maintain` continues accepted changes and waits when caught up. `prepare` runs one bounded
pass. A pause lets the current call finish and publish if its sources remain current,
then admits no next call. Repeated resume does not reset the owner's generation budget.
A provider or invalid-input failure pauses with a named reason; explicitly resume after
resolving it. Missing sources and edits during generation retain pending work and recover
from source notifications. Without `--watch`, explicit source indexing supplies changes.

Use generated vocabulary through the same context allowance:

```json
{"query":"capacity eviction","include_briefs":true,"tokens":4096}
```

Briefs stay labeled derived and are delivered only with their exact source evidence.
This works on stdio and shared HTTP owners; multi-root requests must include the primary,
and only the primary contributes generated briefs. Without a knowledge profile the flag
is unadvertised and true is refused. No source is enrolled from a query.

`status.knowledge` reports cache/work metadata, maintenance state and `last_run` coverage.
`completed_pass_usage` contains owner-local totals for finished passes, separately showing
observed provider tokens, unknown-usage calls and o200k request admission tokens. A currently
`generating` call has no claimed terminal usage yet. Current/cache hits make no model calls;
unchanged idle operation waits on notifications without reading source rows.

Each 60-second owner window allows eight calls, 32,768 request admission tokens and 16,384
requested completion tokens. Each pass, including admission waits, has a 300-second cooperative deadline;
expiry pauses with work retained until explicit resume. Continuous work exceeding a
window waits in `budget_wait` and resumes afterward. Budgets survive
pause/resume but reset with a new owner; they are not durable account-wide billing limits.
Modes also require explicit re-enabling on a new owner; completed outputs remain reusable.
Closing one shared HTTP session does not stop the owner's maintenance.

Request construction/tokenization and HTTP run with the store slot free. Foreground work
wins store admission, and generation waits for 500 ms of foreground quiet (status excluded)
and outstanding watcher source work. Generation and embeddings share background admission;
dense queries retain the existing bounded wait/fallback rules. Already admitted HTTP may
still be running when a query arrives. Payloads and local preparation are bounded, but
Foundry cannot enforce CPU/GPU headroom or terminate computation inside an external service.
Shutdown closes HTTP and discards uncommitted output before releasing the store.

Knowledge tables were introduced in schema 10; the current branch uses schema 11. Selected brief publication does not drain
unrelated source work. Owned local generation is available below; real-model task quality
and CPU/global GPU pressure policies remain open; shared memory admission is described below.

## Owned local generation

The experimental `foundry-generate` worker runs a pinned single-file decoder GGUF through
llama.cpp. The CLI and continuous MCP owner use the same completion cache, source checks
and budgets as HTTP generation. No service, listener, model download or second store is
required. Local selection is explicit; an unsupported device or isolation profile refuses.

Build with Rust 1.90, C/C++, CMake, libclang and a llama.cpp Git object store containing
`b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea`:

```sh
LLAMA_CPP_DIR=/absolute/path/to/llama.cpp cargo build --locked --release --features generate-worker --bin foundry-generate
```

The build exports the pinned objects, ignoring checkout changes, and statically links
llama.cpp only into the optional worker. Linux builds CPU kernels; macOS builds Metal
and Accelerate. CUDA is not supported by this profile. `foundry-generate --notices DIR`
exports the backend license and reports its commit. Model rights remain separate.

Start with the [knowledge profile](#selective-knowledge-generation), remove `endpoint`,
keep `allow_remote:false` and `token_env:null`, and add this `local` object. Replace
paths and SHA-256 placeholders with the actual worker and model digests; top-level
`artifact_sha256` now pins the GGUF bytes. Keep the existing explicit workspace, nodes,
source scope, instructions and request/output limits.

```json
{
  "development": true,
  "executable": "/absolute/path/to/foundry-generate",
  "executable_sha256": "<64 lowercase SHA-256 hex digits>",
  "gguf": "/absolute/path/to/model.gguf",
  "llama_cpp": "b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea",
  "chat_template": "chatml",
  "device": {"kind":"cpu"},
  "context_tokens": 4096,
  "batch_tokens": 128,
  "threads": 2,
  "memory_bytes": 2147483648,
  "memory_enforcement": "hard_address_space",
  "cpu_seconds": 180,
  "load_seconds": 90,
  "idle_seconds": 15
}
```

The example template fits Qwen2.5; select an explicit llama.cpp built-in template
appropriate to the model. Context must fit the model's trained capacity. The complete
formatted prompt is tokenized by the GGUF tokenizer; prompt plus requested completion
must fit without prefix truncation. Greedy decoding uses the exact brief JSON grammar,
then the owner independently validates every citation, identifier and source version.
Grammar compliance does not prove prose correctness. Successful replies report actual
GGUF prompt and sampled completion tokens, separately from o200k request admission.
An interrupted call retains unknown terminal usage; it is not reported as zero cost.

On Linux, `threads` sets backend thread counts and restricts all worker threads to at
most that many available CPUs. `memory_bytes` is a **hard virtual address-space ceiling**,
including mapped weights; it must leave room for KV, scratch, libraries and threads.
`cpu_seconds` is a hard worker-lifetime, all-thread CPU allowance, not a per-call duration.
The separate `call_seconds` and `load_seconds` bound wall time. The descriptor-only
seccomp profile denies path opens, networking, process creation and execution after
initialization; the loader reads only the verified model descriptor. Missing enforcement
refuses generation. See [deployment controls](deployment.md#experimental-owned-generation-controls).

For macOS development, first bundle and ad-hoc sign the worker:

```sh
scripts/generate-worker-bundle.sh target/release/foundry-generate /absolute/path/FoundryGenerate.app
```

Use the printed hash and `Contents/MacOS/foundry-generate` path, set
`memory_enforcement:"supervised_footprint"`, and optionally select:

```json
{"kind":"metal","layers":16,"allocation_bytes":2147483648,"reserve_bytes":536870912}
```

Metal monitors this process's allocation and the remaining allowance under the device's
recommended working-set size; this is **not global free VRAM or a hard GPU quota**.
The worker stops if the allocation/reserve bounds fail, with 20 ms sampling during
load and inference. macOS RAM is also supervised rather than a hard allocation quota.
This branch's Linux checks do not establish Metal execution or signed-package acceptance.

Artifacts are verified lazily only when a selected node needs generation. Streaming
SHA-256 creates private unlinked read-only snapshots once per active run; idle unload
retains these snapshots, so later source edits can reload without copying or hashing
again. Snapshot disk space and kernel page cache are additional to the worker cap.
Current/cache hits need neither model files nor a worker. A failed artifact acquisition
is memoized within the run rather than repeated for every selected node. Explicit resume
starts a fresh acquisition after the operator repairs it. macOS rechecks its signed
executable path on each launch; Linux executes its retained executable descriptor.

Continuous maintenance unloads the worker after `idle_seconds`, during budget waits,
and when its run ends. Normal pause finishes the admitted call, then stops and reaps
the worker. Shutdown/cancellation kills and reaps it before releasing the store; owner
death also ends computation. A failed call keeps source work pending and completed
briefs reusable. These native limits are per worker. The optional
[shared memory policy](#shared-model-memory-admission) additionally accounts for
retained embedding residency. Global GPU, thermal and battery policies remain follow-up work.

`status.knowledge.maintenance.local_worker` exposes PID, launches, calls, verified bytes,
sampled peak RAM, reported peak GPU allocation, isolation and last stop reason without
holding the engine slot. GPU observations update on completed replies and are not live
machine telemetry. CLI reports include `local_worker`; counters are local to the owner/run.

## Shared model memory admission

Opt in on the MCP owner with `--model-resources /absolute/path/resources.json`:

```json
{
  "resident_bytes": 6442450944,
  "reserve_bytes": 1073741824,
  "resume_margin_bytes": 268435456
}
```

This example reserves up to 6 GiB for the **sum of configured embedding and generation
worker memory ceilings**, leaves 1 GiB of observed headroom, and requires an additional
256 MiB before resuming after low-memory pressure. Choose these values for the actual
machine and profiles. All three fields are mandatory positive byte counts; unknown
fields, overflow and files larger than 64 KiB are refused. The policy is independent
of model identity and never invalidates accepted completions or embeddings.

Embedding reserves its ceiling before worker startup and holds it until the provider
thread has stopped/reaped the worker. Generation reserves before loading/calling and
releases only after unload/reaping. Idle and pressure unloads keep the existing verified
artifact snapshots. An embedding worker stays resident for foreground queries: this
policy does not evict/reload it. If both ceilings cannot fit, generation pauses with
`model_memory_budget`; restart with appropriate ceilings/budget or a different profile
combination. A refused embedding startup serves its named lexical fallback; explicit
continuous semantic startup still requires an available embedding runtime.

Background admission samples at most once a second (also refreshing after reservation
changes). Linux uses `MemAvailable` and the minimum remaining `memory.max`/`memory.high`
capacity across the visible cgroup v2 ancestors. It does not change cgroups. Hidden
ancestors are not observable. Inherited mounts with namespace-relative `/..` roots use
a bounded owner-PID lookup (128 directories, depth 64) to locate the actual group;
each proc/cgroup text read is bounded to 64 KiB. Cgroup v1, exhausted lookup bounds,
malformed/unavailable observations or unsupported
platforms defer with `model_memory_observation_unavailable`. macOS uses immediately free
VM pages conservatively, without assuming inactive/compressed pages can be reclaimed;
this path has not been exercised on hardware here.

A new worker additionally needs headroom for its entire ceiling. Low-memory pressure
defers both maintenance lanes, and idle generation is reaped on its next maintenance
poll. Recovery above reserve plus margin reopens admission. In-flight calls retain their
existing per-worker controls and deadlines; foreground queries continue using the already
reserved embedding worker. Generation reports `resource_wait` with `model_memory_pressure`,
`model_memory_headroom` or an unavailable-observation reason; waiting sends no provider
call and charges no usage. Pause/shutdown remain responsive. Waits respect the existing
300-second knowledge pass deadline, after which explicit resume may be needed.

`status.model_resources` reports the policy, reservations by lane, last observed available
bytes, observation age, pressure latch and last admission reason. Reading status does not
probe the host. Current/cache hits need no worker admission and remain usable under pressure.
This is owner-local admission, **not an aggregate OS memory quota**: owner/index memory,
artifact disk/page cache, other processes and other Foundry owners are outside its reservation
sum. External HTTP servers have no local worker reservation. Global GPU usage, CPU utilization,
thermal/battery policy and simultaneous real-model hardware acceptance remain open. Without
this option, previous admission and cache behavior is preserved.


## Boundary relationships and task evidence

The first boundary producer supports static Python FastAPI routes and direct literal
`requests`/`httpx` calls. Put a configuration such as `boundary.json` in the admitted
workspace and index it with the sources:

```json
{
  "v": 1,
  "services": [
    {"id": "catalog", "providers": ["services/catalog.py"], "origins": ["http://catalog"]},
    {"id": "billing", "providers": ["services/billing.py"], "origins": ["http://billing"]}
  ],
  "consumers": ["clients/catalog.py", "tests/test_catalog.py"]
}
```

```sh
foundry --store STORE index ROOT
foundry --store STORE boundary index --config boundary.json
foundry --store STORE boundary relations --config boundary.json --path services/catalog.py --tokens 4096
# After indexing/watching an edit, replace just the selected file's facts:
foundry --store STORE boundary index --config boundary.json --path clients/catalog.py
```

Paths and config refer to indexed source snapshots, not unindexed filesystem bytes.
An unchanged publication skips body reads/parsing/writes; changed config hashes
invalidate old scopes. Refresh all selected files after editing the configuration.
Changed/missing provider scopes yield `incomplete`, not a false unique/absent match.
`exact_key` means syntactic method/service/path agreement; it does not prove a runtime
call or compatible payload. `ambiguous`, `unresolved`, `unsupported` and source spans
remain visible. Dynamic URLs, parameterized routes, indirect clients and other
frameworks are outside this first recipe. No workspace Python executes during indexing.

Reuse a task created with the [task-context workflow](#recoverable-task-context),
retaining its requirements, questions and constraints:

```sh
foundry --store STORE context 'catalog response contract' \
  --task contract --task-revision REV --compose-evidence \
  --boundary-config boundary.json --include-briefs --tokens 8192
```

The selected task view and all source/relationship/brief/report text share the same
allowance. `task-evidence` reports bodies, addresses only, omitted-by-budget and missing
facets; `complete:false` is intentional. Tests are discovered candidates, not executed
verification. Constraints are caller assertions. Derived briefs retain their distinct
provenance and require source evidence; nothing claims host conversation application.
The default context path is unchanged. Composition is currently primary-store only.

For an existing primary-only MCP owner launched with `--experimental-task-context`,
use `index {"boundary":{"config":"boundary.json","paths":["clients/catalog.py"]}}`
after accepting source edits, then `context` with `task_id`, `task_revision`,
`compose_evidence:true` and `boundary_config:"boundary.json"`. Omit `paths` to refresh
the full configured set. Boundary publication and source indexing are separate bounded
operations; no background extraction loop is added. `include_briefs:true` keeps the
existing requirement for a configured knowledge profile on MCP. Without the task flag,
the catalog advertises neither composition nor boundary publication.

The first recipe admits at most 16 services/64 Python paths and 128 facts per file;
traversal takes at most eight seeds, examines 512 key postings and emits at most 64
relations. Composition uses at most four additional lexical task queries, eight links,
eight relationships, four briefs and 32 source units. These are explicit discovery
limits, not whole-repository coverage guarantees. See the [owning boundary spec](../specs/005-graph-evidence-lifecycle/spec.md#t005--connect-repository-behavior-across-process-boundaries)
and [composition spec](../specs/003-agent-retrieval-context/spec.md#t009--compose-evidence-for-a-repository-task).
