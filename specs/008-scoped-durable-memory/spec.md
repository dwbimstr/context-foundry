# 008 — Explicit project memory

Status: T001 and T002 implemented locally 2026-10-04 and unreleased (see
[validation](../../docs/validation.md)). Depends: 001 authoritative ownership, repair and
context contract; MCP extension only if 003 is available.
Authorization: specification/task refinement. Ordinary indexed Markdown notes remain
supported; this feature adds explicit record lifecycle rather than replacing them.
Spec-pass decisions recorded 2026-10-03: one MCP `memory` tool, compact memory lines in
context and search, content-free forget/export reports and refusal of contradictory
input before commit. No external prerequisite.

Implementation decisions (2026-10-04, captain, after cross-lab review):

- **Store schema 3.**
  - `upgrade-store --to 3` upgrades a v1 or v2 store in one write transaction, and
    `--to 2` is refused.
  - It adds the `memory` table and the never-reset `memory_revision` counter.
  - Every pending-index key becomes typed, `source:<path>` or `memory:<id>`. A raw path
    such as `memory:x` therefore cannot collide with a record; migration removes every
    raw key before inserting typed ones.
- **Search documents.** Memory search documents reuse the existing Tantivy fields with
  `kind:"memory"`, so there is no search-schema change. Source tiers exclude that kind,
  and source deletes and repair never touch memory documents.
- **Inputs.**
  - `workspace_id` is required on every operation.
  - Field bounds are enforced at the engine mutation boundary, not only by the request
    parser.
  - Put/update decide idempotence, CAS and `not_found` against the live row first.
    Only an actual create or replace validates source links, inside the same write
    transaction.
- **Index drain.** After a committed mutation, its own `memory:<id>` key is drained
  best-effort. A failure leaves the key pending and never alters the result.
- **Pending counts.** Pending memory work counts in status `pending_count` and in the
  `lagging` state. Source headers and `drained_sources` count only `source:` work;
  repair reports `drained_memory` separately.
- **Memory search output.** v2 text with header segment 1 `foundry memory` and
  search's other segments, including `stale:` and `candidates:full` (256-document
  window), followed by compact `mem:` lines.
- **Context.** `include_memory` validates memory rows inside the context's one final
  read, so memory stale drops count in `stale:`.
  - In a multi-root owner, memory lines come only from the primary root, and only when
    it is selected and serving. Memory trouble never fails a multi-root response.
- **MCP `memory` tool.**
  - Description: `Explicit project memory records.`
  - Schema: `required ["op","workspace_id"]`, `additionalProperties:false`, with no
    `limit`/`tokens`. Search uses 10 hits and search's effective budget.
  - The six-tool `tools/list` measures 793 o200k tokens against the 800 ceiling.
  - `mcp --no-memory` removes the tool and refuses `include_memory:true` with
    `unsupported_mode`. The static `context` schema still lists that optional
    property.
- **Reports and export.**
  - Forget reports are content-free: `{id, outcome, removed_revision?, memory_revision,
    live_records}`.
  - Export pages stop at 128 rows or 4 MiB, whichever comes first, with a cursor. Rows
    carrying eight long source links reach the byte cap before the row cap.
  - An undecodable row stays `corrupt_memory` for get, forget, export and search
    validation.

## Outcome and requirements

Remember a project decision, retrieve it with attribution, correct it without losing
another edit, export it and forget it. Source re-indexing and model failure cannot
rewrite or delete it. No automatic transcripts, reflection, consolidation, decay or
model-written records; no model reads or writes this table.

- **FR-001 / SC-001:** Create/update/read semantics and revision checks below survive
  restart and response retry. Conflicts and contradictory input do not mutate records.
- **FR-002 / SC-002:** Workspace scope and evidence kind are explicit. Source links
  expose fresh/stale/missing against the indexed snapshot and never imply revalidation
  of the memory's statement. Budgeted delivery cannot present memory as compiler fact.
- **FR-003 / SC-003:** Exact export and committed forget affect future read/search/export
  even during index lag; source repair/re-index preserves all live memory records.
- **FR-004 / SC-004:** All records are retrieval-only. No memory is a 013 training row;
  future training use would require a separate explicit contract and permission.

## Inputs and operations

One record: `{id,workspace_id,revision,text,author,provenance,source_links}`. Caller
provides `id` matching `[A-Za-z0-9_-]{1,128}`; scope must equal the bound workspace ID;
revision is a checked u64 allocated from one store-owned memory revision counter,
initially 0; each new create or successful update increments it and uses the new value. A
record's revisions need not be consecutive. Text is nonblank UTF-8 up to 16 KiB; author
1..256 bytes; provenance is a nonblank caller attribution string up to 1024 bytes,
not a server certification. Source links is an array of 0..8 full source handles
from 001; an empty array is required when none exist. Entire request <=64 KiB. Reject
unknown fields/null/type mismatches, oversize and wrong scope before mutation.

The MCP surface is one tool, `memory {op, …}`, with `op` one of `put`, `update`, `get`,
`forget` or `search`; the remaining fields are those of the matching CLI command below.
It replaces the separately named `remember`, `memory_update`, `memory_get`, `forget`
and `memory_search` tools; there are no aliases. Export stays CLI-only.

| Proposed operation | Preconditions | Commit/result |
| --- | --- | --- |
| `memory put` / MCP `memory {op:"put"}` | full creation fields, no caller revision | absent ID→new revision; identical fields→existing row unchanged; different existing fields→`conflict` |
| `memory update` / `memory {op:"update"}` | full replacement fields plus `expected_revision` | exact current revision→one atomic replacement with new revision; otherwise `conflict`; missing ID→`not_found` |
| `memory get` / `memory {op:"get"}` | ID and workspace ID | exact live record including its full text, source-link statuses and `kind:memory`; absent→`not_found` |
| `memory forget` / `memory {op:"forget"}` | ID, workspace ID and `expected_revision` | exact revision→delete live row and queue index deletion; missing→`already_absent`; mismatch→`conflict` |
| `memory search` / `memory {op:"search"}` | query and workspace ID | compact memory lines (below) for matching live rows |
| `memory export` (CLI only) | workspace ID; optional `after_id`, limit default 128, 1..128 | sorted JSONL page of live records on stdout plus next-cursor metadata on stderr |

Contradictory input is refused before commit: a request whose fields contradict its
`op` (for example `put` with `expected_revision`, or `update`/`forget` without it) is
`invalid_argument`, and a `put` whose fields differ from the live record with the same
ID is `conflict`. Like every other refusal, nothing is written and no revision is
consumed.

CLI mutating inputs are one bounded JSON object on stdin. Memory reads/exports are
capped at 4 MiB per page and 128 rows, stopping earlier with a cursor if needed. Wrong
workspace always fails, including forget of an absent ID. Source handles validate
on creation/update; any stale/missing link refuses the whole mutation. On later read,
a changed/deleted source updates the reported link status without editing the record.
`get` returns the stored text exactly, not a generated summary.

Reports are content-free. A forget result carries the ID, the outcome (`deleted`,
`already_absent` or `conflict`), the removed revision when deleted and counts, never
record text, author or provenance. Export's stderr metadata carries row and byte counts
and the next cursor; the exported records themselves are the requested JSONL page on
stdout, which is the purpose of export and not a report.

Update retry after a lost response: read the current row. If expected revision was
already consumed, return conflict even when replacement text happens to match; the
caller can confirm current fields/revision. A successful identical create is idempotent.
An ID can be explicitly reused after forget: a stale concurrent create can therefore
recreate it. No tombstone history is promised; callers must stop outstanding writes
before treating forgetting as final. Document this limitation, not an exactly-once claim.
Recreation always receives a fresh revision, so an old update/forget or stale search
document cannot match the new record by ID/revision. Commit the counter and changed
row together; conflicts, identical create and failed transactions consume no revision.
Counter exhaustion is `revision_exhausted` before mutation. Preserve the counter
through repair, restart and deletion of the last memory; never reset it on an empty
table. This single metadata value avoids per-ID tombstones or incarnation histories.

## Ownership, search and deletion

Use one memory table in the authoritative store. Atomically commit the row/revision
and a typed pending-index key `memory:<id>`; source work uses a distinct namespace.
Derived search documents carry `kind`, ID and memory revision/hash. Validate against
the live memory row before delivery so a forgotten or old revision never leaks through
stale search. Reuse 001's index replay/repair; do not create another database or queue.
Code `source_revision` is unaffected by memory edits.
Memory get/export uses 001's authoritative-only open and is available when lexical
search is broken. A corrupt memory record is named; it cannot silently become absent
or prevent unrelated valid source retrieval. Memory schema changes follow the one
store-owned upgrade sequence; disabling memory never removes its records.

Source search remains source-only by default. Add `include_memory` default false to
context and the bounded `memory search` (`memory {op:"search"}`) for explicit memory
queries. Both render a memory as one compact line,
`mem:<id>@r<revision> <author>: <first line>`, where the first line is the record
text's first line cut at a UTF-8 boundary to at most 120 bytes; the `mem:` prefix is
the kind, and the line carries ID, revision and attribution. Full text comes only from
`get`. Memory search uses shared query/hit limits; context with memory enabled obeys
the same complete-output budget and keeps a fitting source span first. Plain memory
get/export are bounded by bytes, not advertised as provider-token-budgeted. The MCP
`memory` tool uses the existing serialized worker.

Forget is logical removal from this store, not forensic disk erasure, deletion from
old exports/backups, or untraining model weights. No model reads this table in 013.
An export is a read snapshot **per page**; concurrent changes between pages can produce
a mixed revision export. For a consistent full export, run the CLI with sole ownership
and no intervening mutations. Store/schema upgrades preserve records or refuse before
writing with export instructions. No predecessor knowledge migration is included.

## Tasks

### T001 — Store, correct and retrieve an attributed memory

- **Depends:** 001 accepted, and 003 only for the advertised MCP tool. **Scope:** new
  `src/memory.rs`, existing store schema/index/packing, CLI commands and the MCP
  `memory` tool above, new `tests/memory.rs`. Do not alter Laya training or add
  automatic ingestion.
- **Outcome/acceptance (FR-001, FR-002 / SC-001, SC-002):** implement create/update/get,
  explicit memory search and opt-in context inclusion, using exact request limits,
  conflict rules, compact memory lines and source-link statuses above.
- **Verification:** restart read-back; same-ID same-content create preserves revision;
  conflicting create changes nothing; two updates expecting the same revision yield
  one newer revision and one conflict; empty/oversize/null/unknown fields and
  contradictory `op` fields fail before write without consuming a revision. Wrong
  workspace cannot read/write. Edit/delete linked source and assert stale/missing status
  while memory bytes persist. Source-only search/context exclude memory; enabled context
  includes compact `mem:` lines, a multi-line text and a first line longer than 120 bytes
  cut at a UTF-8 boundary, without violating token budget; `get` returns the full text.
  The MCP catalog names `memory` and none of the five replaced tool names.
  Disable/re-enable memory across reopen and preserve exact rows. Break lexical
  search and still read/export them; malformed memory data names corrupt_memory
  while a healthy source record remains retrievable.
- **Review/cutover:** one explicit schema upgrade transaction preserves old source,
  graph and feedback rows and refuses old readers after version change. Check typed
  index identities cannot collide with paths. Roll back only with the prior backup;
  do not ask old binaries to open the new schema. No ephemeral parallel memory store.

### T002 — Export and forget without stale retrieval or training leakage

- **Depends:** T001. **Scope:** memory export/forget and shared index-repair interactions,
  CLI/MCP parity tests, future-learning exclusion fixture. No full-history/tombstone API.
- **Outcome/acceptance (FR-003, FR-004 / SC-003, SC-004):** page export losslessly;
  forget removes live read/search/export eligibility at authoritative commit, before
  derived deletion drains. Return correct conflicts/already-absent outcomes.
- **Verification:** interrupted pre/post forget commit; stale Tantivy document after
  forget; source re-index and full derived repair; two export pages and restart.
  Before forget commit, the exact prior row remains; after commit, get is not_found
  and search/export omit it. Other records retain their bytes/revisions. Forget results
  and export stderr metadata contain no record text, author or provenance. Memory text
  is never in `export-training`/013 preparation even if its provenance mentions a
  successful task.
  Test explicit recreation after forget, including same text with different attribution:
  the new revision is greater; old update/forget requests conflict and stale search
  documents fail validation. Delete the last row, restart and recreate without revision
  reuse; exhaustion/failed commit leaves the counter and records unchanged. Document
  that a late create can still recreate an absent ID under the supported semantics.
- **Review/cutover:** verify logical-deletion disclosure, no forgotten live rows and
  preservation of unrelated knowledge. Exercise put→update→export→forget→reopen
  through CLI and, if advertised, the same MCP owner. Remove owned temporary exports
  only; user backups remain. These functional tests close the spec without a benchmark.

SC-001–SC-004 were verified on 2026-10-04 by the T001/T002 functional tests in
`tests/memory.rs` and the cutover tests ([validation](../../docs/validation.md)). The
limits are input contracts, not memory throughput or outcome-improvement measurements.
