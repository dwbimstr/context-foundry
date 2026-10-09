# Experimental task context

This branch adds recoverable task state beside retrieval. Start with a disposable
store. New stores use schema 7; an existing store needs an explicit
`foundry --store STORE upgrade-store --to 7`. Take a closed-store backup first if
using an existing store: schema 6 binaries cannot open schema 7, and rollback means
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
escaping/envelopes add overhead. Render and advice budgets count JSON bytes, not
provider tokens. Views exceeding the budget require an explicit fold or selection
change; protected evidence is never silently dropped.

Foundry delivers a view; it does not replace a host conversation. Results say
`host_application:"unknown"`. Real-host application, task-success improvements and
provider economics remain unmeasured. The sandbox lifecycle suite exercises actual
CLI restarts, real SDK MCP calls, concurrent revisions, source staleness, exact
recovery and abrupt exits around commits. See [validation](validation.md),
[008](../specs/008-scoped-durable-memory/spec.md) and
[003](../specs/003-agent-retrieval-context/spec.md).
