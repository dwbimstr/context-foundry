# 001 — Reliable local context

Status: T001–T006 implemented and verified locally; not released. T001–T003 were
verified on 2026-10-01 (owner-approved scope: 001 + 003 first implementation). T004–T006
(token-economics tranche, approved 2026-10-03) were each accepted locally at the
cross-lab reviewer's SHIP on 2026-10-04 and committed in `5edf32c`. D001 remains
resolved below. Evidence, review provenance and accepted limitations:
[validation](../../docs/validation.md). The 2026-10-06 amendment (tier-1 marked runs
and specificity order, an MCP `lines` array, an empty-`lines` refusal naming the
handle's lines; [context v2](contracts/context-v2.md)) was accepted locally at the
cross-lab SHIP and committed (`c3437e6`); its proposed route keywords were measured and
withdrawn. A second owner-approved 2026-10-06 amendment, compact context (a context
whose marked identifiers each have exactly one definition returns them plus at most 8
one-line pointers; [context v2](contracts/context-v2.md#compact-context)), was accepted
locally at the cross-lab SHIP and committed (`844796c`); unreleased. The owner approved
the city-map design on 2026-10-07 (T007–T009 below; [context v2](contracts/context-v2.md#city-map)):
T009 implemented and accepted locally 2026-10-08 (`89a8c54`); T007 implemented and
merged 2026-10-08 (`4e23dee`, cross-lab SHIP, gates green); G1 passed 2026-10-08 on the
city-map binary (`483810a`, with 005 T004 and its tie-group amendment); T008 implemented and merged
2026-10-08 (`3e0a117`, cross-lab SHIP, gates green) with owner-approved forks of the Ruby,
Perl and Rust grammars.

## Outcome and baseline

Index a workspace, retrieve verbatim cited context, edit/delete source, re-index and
restart without stale evidence or lost feedback. A CLI release of this behavior is
independent of MCP, graph producers and models. The 2026-10-03 amendment delivers the
same cited evidence in fewer tokens — compact v2 text, syntax-unit search with exact
definitions first and deterministic outlines (T004–T006) — so agents can use Foundry
instead of grep/ripgrep and exploratory file reads.

Current owners: `src/store.rs::Engine::{open,replace_source,delete_source,refresh_index}`,
`src/ingest.rs::sync`, `src/main.rs` and `tests/{core,cli}.rs`. Source/chunks/pending
work already commit in redb; Tantivy refresh clears matching pending hashes. Existing
corrupt Tantivy blocks `Engine::open`; `paths()`, the scan's path set and diagnostic
vectors collect the whole workspace. New contracts below correct these limits.

## Required behavior

- **FR-001:** Source metadata, chunks, source revision and derived-index work commit
  in one authoritative transaction. Acknowledgement follows commit. Replay is
  idempotent; interrupted index repair preserves sources, graph and feedback.
- **FR-002:** Reconciliation has bounded application buffers and explicit completion.
  A failed/interrupted scan never retires unseen paths. A completed scan retires
  absent/excluded source records only, never memory or feedback.
- **FR-003:** Search/context/retrieve obey the shared [context contract](contracts/context-v2.md):
  exact bytes, coordinates, source/workspace identity, stale-handle rejection and
  visible snapshot/index/scan limitations. Queries never read live disk implicitly
  or expand workspace ownership from paths mentioned in text.
- **FR-004:** Count complete CLI search/context/retrieve stdout with `o200k_base`;
  reject over-budget success. Provider usage and dollar savings are separate claims.
- **FR-005:** One owner per store. Busy, unknown schema, corrupt authoritative data
  and derived-index repair needs have distinct errors. No lock stealing or silent
  recreation of authoritative state.

## Storage decision — D001

**Resolved, 2026-09-28: retain redb/Tantivy for 001 and the first supported release.**
This is an architectural disposition from source review, not a recovery/performance
test result. `replace_source` already commits source/chunks/pending work atomically;
`refresh_index` already commits search before clearing matching versions; `search`
rejects stale source hashes. The missing bounded repair/status behavior below fits
one pending-version table and one rebuild marker. Do not reopen the backend choice
merely because another optional retrieval feature is planned.

| Concern | Retain redb/Tantivy | Replace with SQLite/FTS |
| --- | --- | --- |
| Source/feedback/graph ownership | Existing redb transactions and tables | Rewrite owners and map all v1 records |
| Exact path/literal retrieval | Existing path boost, literal terms and source checks | Reimplement path ordering and tokenization; no parity evidence yet |
| Repair obligation | One pending table, version validation and explicit rebuild marker | Shared transactions can remove cross-engine replay; FTS population/update/rebuild still needs correct ownership |
| Dependency/work scope | Existing locked Rust libraries; focused repair changes | Replace search/graph adapters and add a SQLite binding/C library |
| Preserve user state | Explicit v1-to-v2 transaction | Need a separately verified importer or refuse existing stores; cannot discard feedback |

SQLite remains a credible smaller consistency design, not an inferior product.
Its [FTS documentation](https://www.sqlite.org/fts5.html#external_content_tables)
also names external-content synchronization and rebuild obligations. No SQLite
prototype, migration or benchmark is needed to complete this bounded disposition.

Also preserve 009's planned expensive-vector cache through source reconciliation and
derived-index repair using the selected library. This constrains ownership/lifetime;
it does not require implementing neural tables or waiting for model selection in 001.

**Reconsider only on contrary evidence:**
additional mutation-specific durability classes, a second authoritative journal or
cross-store atomicity claims require stopping and revising the design. SQLite can
remove index coordination, but dependency/query/migration differences must be explicit.
Do not implement both or hide the choice in a storage abstraction. T001 now has its
storage direction (2026-09-28); its implementation and restart/fault acceptance passed
on 2026-10-01 (SC-001 below).

## Persistence and recovery contract

Store creation is explicit: CLI `index`, or 003 startup with an explicit root, validates
arguments/root before initializing a new store. Store operations otherwise open existing state;
a missing database is `store_not_found` without creating directories/tables. Explicit
initialization accepts a missing or empty directory; a nonempty directory without a
recognized store is `unrecognized_store`. Ordinary open never upgrades schema or
creates missing tables in an existing schema. Reads do not persist access heat, inferred
tags, query history or observations; explicit feedback has its own write operation.
Library locking/recovery I/O is not a promise of physically read-only disk access.
003's bootstrap inspection, printed connection config, capabilities, gateway and usage-summary
commands are store-independent: dispatch before opening a database. Bootstrap apply
calls this same explicit initializer; it does not add an alternate store-creation path.

For the retained pair, normal opens never drain or enqueue a full-store rebuild.
A fresh empty store creates its empty derived index. A missing/corrupt index in an
existing store is `repair_required`; explicit repair owns full re-enqueue. This changes
the prototype's automatic missing-index enqueue and its test expectation deliberately.
Normal retrieval exposes pending counts; stale index candidates are rejected. `refresh` processes at most 128 source
keys per transaction/batch, repeats until empty or cancelled, and clears only the
version actually indexed. Before commit failure leaves prior source bytes intact;
after source commit and before search commit the new version may be absent from
search, but the old version must be ineligible.

`repair-index` opens the authoritative store without constructing Tantivy first.
Under the same exclusive owner it sets a durable `search_rebuild_required` marker,
queues source versions in pages of 128, moves the old derived directory to a uniquely
named quarantine sibling, creates the replacement and drains pending work. Clear the
marker only after successful index commit/reload and an empty pending table. Any
interruption leaves the marker set; rerunning repair starts from authoritative rows,
not presumed progress. Keep the original quarantine across retries; never quarantine
each partial replacement again. The existing marker records the quarantine basename
and replacement ownership ID; the replacement directory records that same ID. Reset
only a positively matched replacement. If identity/path checks fail, return
`repair_path_conflict` with a bounded diagnostic instead of moving/deleting unknown
files. Three interrupted retries retain at most one original quarantine for that
repair. A completed repair reports its retained diagnostic path; later operator-kept
copies are not claimed to have automatic retention or a global disk quota.
Search/context while marked returns `repair_required`.
Do not delete the quarantine automatically. Refuse symlinked/unrecognized repair
paths rather than deleting outside the owned derived directory. `status`, direct source
`retrieve`, memory get/export, direct graph references and feedback export do not require
Tantivy. Keep them available through authoritative-only open when their own stored
data is valid; search-backed operations still name `repair_required`. This applies to
MCP startup too. Unreadable/corrupt authoritative storage is an error, not a fallback
to cached source or a reason to invent an empty store.
Status may attempt a read-only derived-index open; failed open or rebuild marker reports
repair_required with a named reason. Successful open plus pending=0 reports ready,
which is an operational observation, not a full corruption/integrity certification.

Adding revision/repair metadata is store schema **2**. `upgrade-store --to 2` is an
explicit v1→v2 transaction under exclusive ownership: preserve all existing records,
initialize source revision/scan metadata, and publish schema last in that transaction.
There is no general migration framework. Interrupted upgrade is wholly v1 or v2;
old binaries refuse v2. Unknown versions and authoritative corruption do not migrate.
Rollback before upgrade uses v1; after upgrade use v2 or a pre-upgrade backup, never
write v2 with the old binary. D001 must amend this if storage selection changes.
Since 008 (2026-10-04), the current store schema is **3**. `upgrade-store --to 3`
performs these v1→v2 steps and the memory/typed-pending-key steps in one transaction
from v1 or v2, and `--to 2` is refused ([008](../008-scoped-durable-memory/spec.md)).
Search index schema v2 (T005) is derived state, not a store-schema change: an upgrade
stays metadata-only, so an upgraded store, or any store indexed before T005, reports
`repair_required` with reason `search_schema` until an explicit `repair-index`.

One store-schema version and its explicit upgrades are owned by the store module.
005/008/009/013 changes use the next supported step when implemented; they do not
assign competing version numbers or silently create tables on first use. Feature
enable/disable changes behavior, not the on-disk schema or whether other owners' data
survives. Existing records from disabled features are preserved during upgrade/repair.
Wire versions, compiler artifacts and model recipes keep their own meaning; changing
a model recipe alone is not a store upgrade. No general migration/plugin framework.

The workspace ID and monotonic source revision are defined in the shared contract.
Every committed source add/content change/delete increments revision in that same
transaction; unchanged re-indexing, scan bookkeeping and feedback do not. Checked
counter exhaustion refuses the mutation. This revision is the conservative compiler-
graph freshness basis in 005, not a new persistence plane.

## Reconciliation rules and limits

Fixed limits: file 2 MiB; relative path 4096 UTF-8 bytes; source-key page 128;
20 failure samples and 20 exclusion samples, each at most 512 UTF-8 bytes; exact
64-bit counts beyond samples. Bound application buffers, not the database's disk size
or library cache. Do not collect all source keys, paths or reports in a Vec/set.

Use one checked u64 scan ID (initial 0, increment before each run), completion status
and per-source last-seen ID. Counter exhaustion is `scan_id_exhausted` before any
scan mutation. Enumerate serially. Mark a path seen when encountered even if unreadable. Successful enumeration
permits a paged sweep of unseen source rows; otherwise preserve them and report
`deletions_deferred:true`. Interrupted sweep may retire some verified-unseen rows;
restart with a new full scan before retiring more. Never resume a stale sweep against
a newly changed tree. All scans are observed snapshots, not atomic filesystem snapshots.

Ignore/hidden/vendor/store/symlink exclusions retain the existing policy. Explicitly
encountered oversized/binary/non-UTF-8/sensitive content retires its previous source
record. Directory/read/path-encoding errors prevent the unseen sweep globally.
Excluded subtrees are retired only after a complete scan. Bound file reads to limit+1.
A file disappearing during read is a named failure, not proof of global absence.

Validate every admitted relative path against the shared handle rules before source
commit; unsupported path syntax is a named scan failure and prevents the unseen sweep.
Open candidates relative to a held root-directory handle, rejecting symlink components
and non-regular files at open/read time using ordinary supported OS/library primitives.
Walker metadata and `follow_links(false)` alone do not establish this: a file or
ancestor can be replaced after enumeration. Such a replacement is `source_changed`
or `unsafe_source_path`, preserving the previous accepted record and deferring absence
deletion. Do not reopen the enumerated absolute pathname or fall back to following
links. Unsupported platform primitives are a named platform limitation, not a weaker
scope guarantee. This belongs in ingestion, not a new filesystem service. The source
hash still identifies the bytes actually read; concurrent in-place edits do not turn
an observed scan into an atomic filesystem snapshot.

Accepted limitation (captain decision after the final cross-lab gate, 2026-10-01):
names are enumerated by the pathname `ignore` walker so ignore policy stays exact;
bytes are always read relative to the held root. The root's identity is checked
before and after enumeration, so a persistent substitution defers the sweep. A
same-user process that substitutes the root or a subdirectory and restores it within
one enumeration can still make that scan miss names and retire their source records.
No outside bytes, memory or feedback are affected; the next complete scan restores
the records. Re-entry: descriptor-anchored enumeration with equivalent ignore policy,
when a supported workflow renames workspace directories during indexing or a racing
same-user process enters the threat model. Do not describe the scan as rename-proof.

`index` reports changed/unchanged/deleted/excluded/failure counts, bounded samples,
scan completion and remaining index work. Success requires complete enumeration,
sweep and drained indexing. Partial work remains committed and is reported as partial;
retry re-scans safely. Check cancellation between files and index batches. This is
cooperative cancellation, not a hard deadline on an OS/library call.

## Tasks and exact acceptance

### T001 — Recover derived search without losing knowledge

- **Depends:** D001 disposition. **Scope:** `src/store.rs`, command dispatch in
  `src/main.rs`, `tests/core.rs` and new `tests/recovery.rs`; no daemon/model work.
- **Outcome/acceptance (FR-001, FR-005 / SC-001):** implement schema-2 upgrade, metadata-
  only status/export, bounded refresh and explicit repair as above. Unknown schema
  changes no records; competing open returns `store_busy` without retry/lock stealing.
- **Verification:** child-process exits at before/after source commit, after search
  commit before pending clear, during repair enqueue, after quarantine, and before
  marker clear. Reopen/retry; exact source hashes, graph rows and feedback records
  match the committed expectation; no stale hit; repair ends pending=0/marker=false.
  Missing/corrupt derived index, disk/write failure and repeated repair preserve data.
  Upgrade interruption leaves an entirely readable old or new schema, not a mixture.
  Missing-store status/search creates no filesystem state; wrong-root/invalid index
  arguments do not initialize a store. Repeated reads leave authoritative rows and
  pending work unchanged. Broken search still permits status, direct source retrieval
  and independent exports. Inject three repair interruptions and check one quarantine,
  preserved original bytes and refusal to reset an unrecognized replacement directory.
- **Review/cutover:** verify commit order and owned-path checks before T002. Retain
  quarantine and explicit upgrade notes; remove any old open path that masks repair
  failure. These tests prove application ordering, not hardware power-loss immunity.

### T002 — Reconcile without an all-workspace buffer

- **Depends:** T001. **Scope:** `src/ingest.rs`, paged source/scan methods in
  `src/store.rs`, CLI reports and `tests/cli.rs`; no watcher or crawler service.
- **Outcome/acceptance (FR-002 / SC-002):** implement the scan rules and exact limits
  above; status counts no longer call `paths().len()`; remove unbounded scan/report
  collections from production paths.
- **Verification:** edit/add/delete, ignored directory, symlink, binary, unreadable
  file and path error; inject failures deterministically rather than relying on chmod
  under an elevated user. Interrupt enumeration and sweep separately. Assert absence
  deletion only after complete enumeration, preserved feedback and exact aggregate
  counts with 25 failures/25 exclusions but only 20 samples each. A generated 10,000-
  path fixture checks batch/sample high-water marks, not a total-RSS or scale claim.
  Bind store A to root A, attempt root B and assert refusal before any source/revision/
  pending change. Index B in its own store; deleting/reconciling A leaves B intact.
  Replace a file and then an ancestor directory with an outside-pointing symlink
  between enumeration and open: no outside bytes are read/indexed; name the failure,
  preserve prior accepted bytes and defer the unseen sweep. Invalid handle-path
  syntax never commits an unreachable source. Use deterministic synchronization,
  not timing-based sleeps or assumptions from the walker's initial file type.
- **Review/cutover:** prove sweep start condition and restart behavior; discard old
  path-set logic when the paged flow passes. Failed scans remain repairable by re-index.

### T003 — Retrieve exact source within the declared budget

- **Depends:** T001/T002. **Scope:** source reads/context packing in `src/store.rs`,
  CLI `retrieve` plus existing search/context/status, and `tests/{core,cli}.rs`.
- **Outcome/acceptance (FR-003, FR-004 / SC-003):** implement shared handles, bounds,
  error precedence and source-first packing; expose exact pending/scan limitations.
- **Verification:** the public parser fixture finds `parse_record`; context contains
  its source span when it fits. Edit its body, index, and prove old handles fail/new
  handles return new bytes; delete and prove `not_found`. Test CRLF, no final newline,
  Unicode, 4096-byte paths, empty files, invalid ranges and budgets 1/32/64/256/1024/32768.
  Count actual stdout, reconstruct every returned span and check no source bytes were
  rewritten. Graph annotations cannot crowd out a fitting first source span.
  Assert `references to parse_record` selects graph when available, while `find
  preferences` and `find calls_tracker` select search; no supported current graph
  skips optional learned routing and returns the named source-only outcome.
  Launch against A from directory B and query text naming B's absolute path: the
  bound workspace and source/pending/revision state stay unchanged, B's unique source
  marker is not retrieved, and a valid B handle returns `wrong_workspace`. This
  asserts source scope, not a claim that query text cannot mention B.
- **Review/cutover:** preserve existing text CLI mode, version added JSON fields and
  document new errors. No provider-cost claim. Run the fresh-store CLI flow and relevant
  Rust checks, then [release checks](../../docs/release.md) for the advertised CLI scope.

SC-001/002/003: passed locally on 2026-10-01 (`cargo test --locked --no-fail-fast`:
cli 13, core 20, integrity 15, repair 8, response 8, scan_faults 15 plus 12 child-process
recovery scenarios; the same safety suites pass on Rust 1.90.0 Linux aarch64). Faults use
named points behind the non-default `test-faults` feature. Owners: `src/store.rs`,
`src/ingest.rs`, `src/response.rs`, `src/control.rs`, `src/error.rs`, `src/cli.rs`.
Two cross-lab review rounds plus a final OpenAI pass closed every finding except the
accepted enumeration residual above. Hardware power-loss behavior is not claimed.

### T004 — Deliver compact v2 text with exact accounting

**Status:** locally implemented and verified on the final4 manifest; committed in
`5edf32c`, unreleased. All six gates exited 0 and the same existing OpenAI T004
reviewer returned SHIP.
See [validation](../../docs/validation.md). The reviewed ambiguous-item-line refusal
is an accepted test-parser limitation, not a universal item-line round-trip claim.

- **Depends:** T001–T003 and 003 T001–T002. **Scope:** `src/store.rs` (handle
  parse/format/validation and `retrieve` with `lines`; not search internals),
  `src/response.rs`, `src/mcp.rs`, `src/cli.rs`, `src/bootstrap.rs`, `src/testkit.rs`
  (adds `parse_v2(&str) -> V2Response { header: Vec<String>, items: Vec<V2Item { handle,
  lines, label, form, body }> }`) and the affected cases in
  `tests/{mcp,response,cli,core,integrity,recovery,repair,scan_faults}.rs`. Schemas gain
  `tokens` on search and `lines` on retrieve; `path`, `view`, `roots` and `root` arrive
  with T005, T006 and 007. The adapter's atomic allowance (owned by 003's
  [economics contract](../003-agent-retrieval-context/contracts/adapter-economics.md))
  and 003 T005's catalog and instruction text land here because they share
  `src/mcp.rs` and `src/bootstrap.rs`. Until T005, search documents remain today's
  storage blocks, each its own delivery unit rendered as kind `block`, with the lexical
  best-line rule.
- **Outcome/acceptance (FR-003, FR-004 / SC-004):** search, context and retrieve emit
  the [v2 wire](contracts/context-v2.md) at both boundaries with v2 handles, exact
  counting, budgeted search, `lines` and one atomic reservation per response. Delete
  the v1 JSON renderers (`format_version`, `search_json`, `application_item`,
  `pack_*_application`), `take_context_id`/`return_context_id`, `remaining_allowance`,
  `effective_with_session` and `reserve`.
- **Verification:** at budgets 1/32/64/256/1024/32768, the exact o200k count of the MCP
  text block and of CLI stdout (including its newline); serialized result at most
  256 KiB; every fenced body byte-equal to its handle's range for no-final-LF, CRLF,
  empty-content, embedded-fence and JSON-escape-heavy sources; forward-progress `next`.
  Handle round trip including a 4096-byte path containing `#`, `@`, `.` and JSON-special
  characters; a v1 object and uppercase hex → `invalid_argument`; synthetic full IDs
  sharing no 16-hex prefix → `wrong_workspace`; edited source → `stale_handle`;
  deleted → `not_found`. `lines`: mid-line unit boundaries, continuation handles, CRLF,
  an unterminated last line, a trailing LF, and an empty file → `invalid_range`.
  Headers carry none of the removed fields and stay within 40 tokens on the fixture;
  no delivery ID anywhere; `host_request` still refused on both transports. Search
  charges the session allowance. Synchronized same-session concurrency
  (barrier-released HTTP calls): one refused and one successful delivery leave the
  exact expected remaining balance. Instruction-like source stays inside a fence.
  `tools/list` at most 800 tokens. CLI smoke: `foundry --store /tmp/cf-smoke index
  examples/workspace`, `foundry --store /tmp/cf-smoke search parse_record`, then
  `foundry --store /tmp/cf-smoke retrieve --handle '<handle from that output>'
  --lines 1-3` prints a v2 header and exactly those lines
  (`examples/workspace/src/parser.rs` has three lines and `parse_record` spans 1–3, so
  `--lines 4-6` there is necessarily `invalid_range`). Clipping: in
  `tests/fixtures/agent-task/src/records.rs`, where `parse_record` occupies lines 5–7
  after a three-line `//!` comment and a blank line, a handle covering exactly lines
  5–7 with `--lines 4-6` returns lines 5–6, never widened.
- **Review/cutover:** one renderer per boundary and nothing appended after counting;
  clean cutover with no v1 compatibility mode (the v1 wire was never released). Move
  the `src/mcp.rs` comments that cite context-v1 to v2. Record the measured handle
  cost for 003 T005's payload report.

### T005 — Rank syntax units with exact definitions first

**Status:** accepted locally at the reviewer's r3 SHIP on 2026-10-04; committed in
`5edf32c`, unreleased. Owner decisions taken during review: an upgrade stays
metadata-only (explicit `repair-index`), and qualified names keep their last 256
bytes (§ Unit kinds of the [v2 contract](contracts/context-v2.md#unit-kinds)). See
[validation](../../docs/validation.md).

**Amendment (owner decision 2026-10-04), implemented locally, unreleased:** units take
their leading run of documentation comments and Rust attributes (§ Unit forest of the
v2 contract), a tier-1 best line stays on the definition's own (head) line, a leading
documentation span of at least 2 lines is elidable, and `search_schema` becomes `"3"`
so indexes built before the amendment report `repair_required` until `repair-index`.
Cause: 003 T005's frozen-corpus check found 10/12 expected units; both misses were
Rust definitions whose doc comment and attribute sat outside the unit, so the
question matched the preceding block. Verification adds per-language leading-run
range tests (attached, blank-line-separated, same-line-trailing, inner-doc, Unicode
whitespace and empty-`/**/` cases), the best-line rule, the outline doc span and the
schema gate; the economics check found 12/12. Accepted at the OpenAI Sol reviewer's
delta SHIP (see [validation](../../docs/validation.md)).

- **Depends:** T004. **Scope:** `Cargo.toml` and `Cargo.lock` (the contract's
  dependency list), new `src/syntax.rs` with `pub mod syntax;` in `src/lib.rs`,
  `src/store.rs` (schema v2, tokenizers, refresh document building, two-tier search,
  META `search_schema`, `Engine::search_candidates`), `src/response.rs` (locator
  rendering with delivery-unit handles and labels), `src/mcp.rs`/`src/cli.rs` (`path` /
  `--path`), new `tests/syntax.rs` and search cases in `tests/{core,mcp,repair}.rs`.
  Gate: `cargo check --all-targets --locked` under the actual Rust 1.90 toolchain
  (§ SC paragraph below) passes with every selected grammar.
- **Outcome/acceptance (FR-003 / SC-005):** the v2 contract's syntax units and search
  documents: unit forest, delivery-unit documents, schema v2 with its tokenizers,
  two-tier ranking with deterministic cutoffs, hit materialization and the
  `search_schema` repair gate.
- **Verification:** `tests/syntax.rs` per mapped language: a nested container, a
  function of at least 5 lines and a wrapper (decorator/export/template) yield the
  expected units (kind, qualified name, byte ranges); partition invariant and
  delivery-unit containment; a malformed parse still tiles; oversize parts are at most
  4096 bytes; an unmapped extension yields blocks; Markdown sections stop at
  equal-or-higher-rank headings and ignore fenced headings. `tests/core.rs`:
  `parseRecord` and `parse_record` are both found by `parse record`; the definition is
  hit #1 over more than 3 call sites and repeated identifiers; `path` filter; per-file
  cap 4 with `capped`; more than 256 equal-score documents select identical handles
  across two index builds with shuffled insertion order. `tests/repair.rs`: a store
  indexed before T005 (no `search_schema = "2"`) reports `repair_required` while
  retrieve/status work; `repair-index` makes it ready with zero source changes; a crash
  injected before/after replacement creation, index commit and schema publication
  converges on rerun with one original quarantine and no stale documents.
- **Review/cutover:** record the new dependencies and their licenses in
  [dependencies](../../docs/dependencies.md). A grammar that cannot meet Rust 1.90
  stops the task for an owner decision. Existing stores need an explicit
  `repair-index`; ordinary opens never rebuild.

### T006 — Fit more evidence with outlines and forms

**Status:** accepted locally at the reviewer's r3 SHIP on 2026-10-04; committed in
`5edf32c`, unreleased. `following_chunks` is removed. Owner decisions on outline
refusals are in § Retrieve views of the [v2 contract](contracts/context-v2.md#retrieve-views).

- **Depends:** T005. **Scope:** `src/syntax.rs` (outline), `src/store.rs`
  (`context_candidates`, `CandidateBatch`, removal of `following_chunks`),
  `src/response.rs` (`pack` and the forms ladder), `src/mcp.rs`/`src/cli.rs` (`view` /
  `--view`) and tests in `tests/{syntax,core,mcp,response}.rs`.
- **Outcome/acceptance (FR-003, FR-004 / SC-006):** the v2 contract's outlines and
  forms, candidate seam, context candidates, ladder packing and retrieve views.
- **Verification:** outlines of a 300-line Rust file and of a many-member class keep
  every signature, including multi-line and same-line-brace signatures; each `⋯ a-b`
  matches `retrieve --lines a-b` bytes exactly; `[signature]` is used for a unit larger
  than the budget; a fitting first unit precedes graph items; at most 3 outlines;
  `view:"outline"` never returns `next`, falls back to `outline-min`, then
  `budget_too_small`; an unmapped language is `unsupported_mode`.
- **Design reference (recorded per AGENTS.md):** oh-my-pi at commit
  [`5b8d5b8a15ab1711597584aadbdc112c55befed1`](https://github.com/can1357/oh-my-pi/blob/5b8d5b8a15ab1711597584aadbdc112c55befed1/crates/pi-ast/src/summary.rs),
  `crates/pi-ast/src/summary.rs`: `summarize_code` and `select_folded_spans`
  (breadth-first unfold between `unfold_until_lines` and `unfold_limit_lines`), with
  tests `bfs_unfold_stops_when_visible_already_exceeds_target`,
  `bfs_unfold_reverts_when_next_step_overflows_limit` and
  `bfs_unfold_skips_unfoldable_leaf_and_continues_to_siblings`; MIT licensed.
  `src/syntax.rs::outline` is an independent Rust implementation and copies no code.
  Intentional differences: elidable spans come from Foundry's own unit forest, kinds
  and thresholds; visible lines count each marker as a line (the reference counts
  unfolded source lines only); segments are byte ranges plus line-numbered markers, so
  every `⋯ a-b` is retrievable exactly; signatures and closing lines are mandatory;
  forms are whole-or-nothing with fixed parameters (60/120 and 0).
- **Review/cutover:** remove `following_chunks` and any second packer; retrieve's text
  view keeps the bounded prefix fit.

### T007 — Addresses, anchors and the anchored context (city map)

**Status:** approved 2026-10-07 (owner), revised after cross-lab refutation the same
day; implemented and merged 2026-10-08 (`4e23dee`; [validation](../../docs/validation.md)).
G1 judged it together with 005 T004: PASS on 2026-10-08 (`483810a`).

- **Depends:** T005/T006 and the 2026-10-06 amendments. **Scope:** `src/syntax.rs`
  (name-node range on each unit), `src/store.rs` (roles, one definition document per
  unit, the new fields, search schema `"4"`, anchors, resolver windows), `src/response.rs`
  (`RenderedForm::Address`, anchored selection, header segments), `src/roots.rs` and
  007's merge rule, `src/testkit.rs::parse_v2`, tests in `tests/{core,syntax,mcp,economics}.rs`.
  Doors are 005 T004; import keys are written here for it.
- **Outcome/acceptance:** the v2 contract's § Roles, § Definitions and addresses,
  § Anchors and qualifiers, § Resolver order, § Ladder for anchored definitions and
  § Anchored context; § Compact context, its trigger and its tests are replaced, not
  kept beside it.
- **Verification:**
  - roles on a path table with first-match precedence and every listed pattern;
  - one definition document per unit: a 20 KiB interface and a container with
    residuals count once, and a 70-part region still counts once; `struct Foo` with
    two `impl Foo` blocks is one definition of `Foo` (resolved), and the impls'
    methods keep `Foo` as an address qualifier;
  - anchors: marked chains (`Vec::push`, `` `Foo.bar` ``, `` `Get-ChildItem` ``) split
    name and qualifiers; paths (`a/b.rs`, `learning.rs`, `package.sh`) never anchor;
    unmarked `sleep_ms`, `READY_RECEIVE_ENTERED`, `toolSession`, `HttpServer` and `a::b`
    anchor; `find`, `spawn`, `MCP`, `WAL`, `v2`, `T002`, `e.g.` and a URL do not;
    `Engine` anchors only when not first and an exact-case definition exists; marked
    anchors come first, then unmarked identifier-shaped runs (``how does `Engine` handle
    refresh_index errors`` anchors both), then capitalized ones, at most four;
  - the 27 recorded agent queries of the refutation, kept outside Git with their
    expected anchors against this repository at `2513748` in
    `~/VSC_DEV/datasets/context-foundry-citymap/anchor-queries.json`, give exactly those
    anchors: none from an extension, acronym, id, abbreviation or URL;
  - resolver: a qualifier beats exact case, exact case beats role, role beats path; a
    window over more than 64 definitions finds the intended one; resolved selection
    (1 ladder entry, 8 directory lines) and ambiguous selection (16 entries: address
    lines for all first, then signature and then verbatim upgrades in list order while
    the budget fits; no entry is omitted while any shows a body, across a budget sweep)
    definitions in one file; an anchored context carries no pointer lines;
  - the ladder: the oh-my-pi `ToolSession` shape (a 349-line interface) degrades to
    `[address]` and is never omitted while the remaining budget fits that line;
  - a query without anchors differs from before only through one-document-per-
    definition; header segment order and the 40-token bound at the largest values;
  - schema `"3"` stores report `repair_required`, and `repair-index` rebuilds them;
  - 007: counts summed over merged roots; merged windows ordered by tuple, then root
    order, then each root's order, with `key_hash` never crossing roots.
- **G1 (frozen before any T007 code):** § G1 below.

### T008 — Languages beyond the first eight

**Status:** approved 2026-10-07 (owner), revised after refutation; implemented and
merged 2026-10-08 (`3e0a117`; [validation](../../docs/validation.md)).

- **Depends:** T007 (schema and import keys). **Scope:** `Cargo.toml` (the grammar
  crates of the contract's § Languages and `tree-sitter` 0.25 → 0.26), `src/syntax.rs`
  (`Lang`, `from_path` with the two basenames, `unit_kind`, `unit_name` with suffix
  stripping, `qname_separator`, `body_range`, `is_wrapper`, outline members, import
  node kinds for every language including Rust, C and C++), fixtures in
  `tests/syntax.rs`, THIRD-PARTY notices.
- **Outcome/acceptance:** § Languages: 15 more languages get units, addresses, outlines
  and import keys; every other text source stays plain text. Search schema bumps again
  if T008 lands after T007.
- **Verification:** the original eight's units are unchanged on their fixtures under
  `tree-sitter` 0.26; per new language, a fixture with nested containers, an
  overloaded name, a method in a nested type, a decorated/attributed/annotated
  definition, a name with a `?`/`!`/`'` suffix where the language allows one, and one
  import of each kind: exact units, qnames, name ranges and import keys; malformed and
  deeply nested inputs parse without panic, and a parse that exceeds the work budget
  (a 4,000-deep Haskell `let`, which the 2026-10-07 spike measured at 42 s) stops at
  the same point on 1 and 8 threads and becomes a named failure with `unparsed` blocks;
  each grammar loads at runtime on Rust
  1.90; the release binary size before and after is recorded (the spike measured about
  +48 MB for all 15; the owner accepted all 15 on 2026-10-08); a grammar that fails is
  dropped with its reason recorded.

### T009 — Parallel indexing

**Status:** approved 2026-10-07 (owner), revised after refutation; implemented and
accepted locally 2026-10-08 (`89a8c54`, [validation](../../docs/validation.md)). Its
timing is a record, not an acceptance condition, and was taken on 2026-10-08. On the
18 GiB development host, indexing oh-my-pi took 53–57 s with every binary (fastest
valid runs: main 0.949× of before). [inference] Indexing there is I/O-bound, so the eight threads
give no visible gain. A valid rust-lang/rust run with the binary before T009 is still
owed: every attempt swapped
([measurement handoff](../../docs/measurement-handoff.md)).

- **Depends:** T005. **Scope:** `src/store.rs` refresh (bounded parse fan-out ahead of
  the one Tantivy writer), `tests/recovery.rs` and `tests/core.rs`.
- **Outcome/acceptance:** § Parallel indexing.
- **Verification:** 1 and 8 threads give equal documents (field values in key order)
  and equal store tables, also when completions arrive out of order; a panic in the
  earliest key's worker gives that source a named failure and plain-block documents,
  and every other key of the page is indexed; add, commit and reload failures and
  cancellation before and after commit lose no pending key and acknowledge none
  early; restart after a crash replays; the hand-out bound counts sources built but
  not yet added (tested at a bound reduced through a test-only hook, so the test stays
  fast); G1 results (§ G1) on a store rebuilt by the T009 binary are identical to the
  baseline's. Index time and peak memory for oh-my-pi (8,436 files, 65 s
  single-threaded on 2026-10-07) and rust-lang/rust before and after are recorded by
  the [measurement handoff](../../docs/measurement-handoff.md).

### G1 — city-map measurement

**Frozen 2026-10-08 00:22Z, before any T007 code; refrozen 00:51Z after cross-lab
refutation (apparatus, buckets and four targets), still before any T007 code**
(captain; the 2026-10-07 draft's baselines were unreproducible). G1 measures whether
an agent that names an identifier gets the evidence in fewer calls and tokens. It
gates T007 and 005 T004 together (one city-map binary). T009 leaves every G1 result
unchanged; T008 shows no target or guard regression and lists every task whose result
changes (its new shell, PowerShell, Perl and Swift units add names and change lexical
statistics).

- **Apparatus** (outside Git, `~/VSC_DEV/datasets/context-foundry-citymap`, SHA-256 in
  its `FROZEN-G1v4.txt`): `g1v4.py` (`5f7dac42…`) runs a scripted Foundry agent (at
  most 4 MCP calls: `context`, `search` on a miss, `retrieve`, `references`) and a
  scripted grep agent (ripgrep and read windows, output cut at 50 KiB); `g1score.py`
  (`5dc1c5e8…`) scores delivered evidence; `g1compare.py` (`458a8c0d…`) holds the
  targets below and prints the verdict; `g1estimates.py` (`6553cc12…`) derives the two
  corpus estimates cited below; `g1quick.sh` runs a sample for iteration. Evidence is
  delivered source: the definition's name line visible in a body, or reference-site
  lines in at least min(3, files) files; handles, addresses, directory and locator
  lines are navigation only. **Body** counts a definition task whose name line was
  delivered inside a verbatim body (not only a signature, outline or elided form).
- **Sets:** `checker` (6,881 tasks on rust-lang/rust 1.99.0: 4,597 development and
  2,284 held-out wordings), `qualified` (2,551: each checker development symbol asked as
  `q::name`, `q` its module's last segment) and `bun` (999 on oh-my-pi `5b8d5b8a15`:
  exported TS/TSX definitions and their importers). Each task is also bucketed by how
  many definitions share its exact name in the corpus (`g1buckets.py`, `53cdaffe…`: the
  grep agent's definition keywords plus `const fn`, `static mut` and `const enum`
  forms; 0, 1, 2–4, 5–16, >16). Bucket 0 is enum variants and macro-generated items,
  which have no definition unit of their own.
- **Reproducibility:** two runs of the same binary give identical results and
  responses (timestamps aside); two full Foundry baseline runs agree on all 10,431
  scored tasks. The grep agent sorts output by path and line before the cut. A call
  refused as retryable (`deadline_exceeded`, `busy`) is repeated and the repetitions
  are recorded; a task still refused is marked. The verdict fails closed: a run with a
  duplicate or missing task, a marked task or any repetition is INVALID, never PASS,
  and is rerun on a quiet host; a refusal the candidate reproduces there fails G1. The
  2026-10-07 v2/v3 baselines failed these checks (ripgrep's parallel walk changed
  grep's cut on every run; 49 of 1,000 bun tasks hit the 5 s read deadline on an
  overloaded host) and were discarded.
- **Baselines** (the `844796c`-equivalent release binary, full sets, 0 exclusions, 0
  repetitions):

  | Group | Tasks | Foundry pass@1 | Foundry pass≤4 | Foundry body | grep pass≤4 | Foundry / grep median tokens to pass |
  | --- | ---: | ---: | ---: | ---: | ---: | ---: |
  | checker marked definitions, unique name | 495 | 89.1% | 89.1% | 89.1% | 97.6% | 806 / 682 |
  | checker marked definitions, 2–4 names | 546 | 82.1% | 82.1% | 82.1% | 92.5% | 2,016 / 941 |
  | checker marked definitions, 5–16 names | 707 | 77.1% | 77.1% | 76.5% | 49.1% | 2,022 / 1,327 |
  | checker definitions, >16 names | 2,159 | 20.9% | 21.0% | 20.5% | 5.0% | 2,026 / 2,635 |
  | checker marked usage, unique name | 417 | 8.2% | 93.8% | — | 100% | 1,088 / 177 |
  | checker usage, 2–4 names | 457 | 22.5% | 54.3% | — | 99.6% | 2,066 / 670 |
  | checker usage, 5–16 names | 444 | 21.8% | 38.3% | — | 96.6% | 2,043 / 2,827 |
  | qualified definitions (excluding bucket 0) | 1,836 | 45.7% | 45.8% | 45.2% | 39.3% | 2,025 / 868 |
  | qualified usage (excluding bucket 0) | 623 | 8.0% | 31.8% | — | 74.6% | 2,149 / 2,246 |
  | bun definitions, unique name | 462 | 93.1% | 93.5% | 90.3% | 100% | 1,994 / 647 |
  | bun usage, unique name | 314 | 16.2% | 16.2% | — | 99.4% | 2,021 / 231 |

  Mean tokens per task: checker 2,056, qualified 2,313 and bun 1,718 (Foundry); 7,491,
  7,228 and 836 (grep). Grep's first call is a location list, so its pass@1 for
  definitions is 0 by construction.
- **Targets** (full sets, the city-map binary against the Foundry baseline):
  - checker marked definitions: pass@1 ≥ 95% (unique name, 2–4 names), ≥ 90% (5–16
    names);
  - checker marked usage with a unique name: pass@1 ≥ 80% (48 of its 417 tasks use the
    held-out wording "which code relies on", which requests no doors);
  - qualified, excluding bucket 0: definitions pass@1 ≥ 80% (the qualifier narrows the
    name to one definition for 41.8% and to 2–16 for 46.6%); usage pass@1 ≥ 40% (doors
    need a resolved anchor; the qualifier resolves 51.8%);
  - bun unique names: definitions pass@1 ≥ 97%; usage pass@1 ≥ 85%;
  - median tokens to pass of unique-name definitions at most grep's (checker marked,
    qualified and bun); mean tokens per task at most the Foundry baseline's, per set;
  - guard: in every group of at least 300 tasks (intent × bucket, intent × marked,
    targeted groups included), pass≤4, and for definition groups also pass@1 and body,
    fall by at most min(2.0 points, 10% of the baseline value) against the Foundry
    baseline. This is a fixed rule on the frozen census, not a statistical claim of
    non-regression. Equally scored definitions use today's tier-1 mechanism (§ Resolver
    order).
- **Protocol:** a copy of the baseline store (rust-lang/rust with the same imported
  SCIP artifact; oh-my-pi) is brought to the candidate's schema with its
  `repair-index`; `g1v4.py` runs every task; `g1compare.py` gives the verdict. The
  result and its counts are recorded in [validation](../../docs/validation.md);
  results, responses and transcripts stay outside Git. During development,
  `g1quick.sh` runs a 1/8 sample of a set in under a minute; samples are indicative
  and never recorded as acceptance.
- **Result (2026-10-08): PASS** on `483810a` (T007, T008, T009 and 005 T004 with its
  tie-group amendment), every target and guard on all three full sets:
  checker D1 97.8%, D2 98.0%, D3 92.8%, U1 86.6%; qualified D4 94.0%, U2 81.2%; bun B1
  99.1%, B2 96.8%; tokens below grep's median and the baseline's mean everywhere
  ([validation](../../docs/validation.md)). The first integrated run (T007 + T004)
  failed D2, D3 and two usage guards; parse errors on rustc nightly syntax (fixed by
  T008's Rust grammar fork) and ambiguous anchors without doors (the tie-group
  amendment) were the causes.

SC-004/005/006 are the acceptance of T004/T005/T006 respectively: each passed locally
at its reviewer's SHIP (2026-10-04), recorded in [validation](../../docs/validation.md).
Each task runs `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked --no-fail-fast`, then `cargo check --all-targets --locked` and
`cargo clippy --locked --all-targets -- -D warnings` under the actual Rust 1.90
toolchain: put `<rustup home>/toolchains/1.90.0-<host>/bin` first on `PATH` and record
`rustc`, `cargo` and `clippy-driver --version`. Do not rely on `rustup run 1.90.0` or
`cargo +1.90.0` alone: where another package manager's `cargo` precedes the rustup
proxies, they can resolve to a different compiler.
