# Experimental task context

This branch adds recoverable task state beside retrieval. Start with a disposable
store. New stores use schema 8; an existing store needs an explicit
`foundry --store STORE upgrade-store --to 8`. Take a closed-store backup first if
using an existing store: schema 7 and earlier binaries cannot open schema 8, and rollback means
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
`--include-memory` may share this same allowance. MCP brief import/discovery flags,
automatic generation and area rollups remain future work.

Requests are capped at 256 KiB, claims/identifiers/unknowns at 16 KiB of text, and
citations at 32 spans. Stored records are capped at 512 KiB including full dependency
paths. Search refuses when its first matching record cannot fit the requested byte
allowance; `get`/`export` by ID can recover a larger bounded record. No output is
silently cut inside a claim.
