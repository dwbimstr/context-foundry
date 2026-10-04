# 001 — Reliable local context

Status: T001–T006 implemented and verified locally; not released. T001–T003 were
verified on 2026-10-01 (owner-approved scope: 001 + 003 first implementation). T004–T006
(token-economics tranche, approved 2026-10-03) were each accepted locally at the
cross-lab reviewer's SHIP on 2026-10-04 and committed in `5edf32c`. D001 remains
resolved below. Evidence, review provenance and accepted limitations:
[validation](../../docs/validation.md).

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

SC-004/005/006 are the acceptance of T004/T005/T006 respectively: each passed locally
at its reviewer's SHIP (2026-10-04), recorded in [validation](../../docs/validation.md).
Each task runs `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked --no-fail-fast`, then `cargo check --all-targets --locked` and
`cargo clippy --locked --all-targets -- -D warnings` under the actual Rust 1.90
toolchain: put `<rustup home>/toolchains/1.90.0-<host>/bin` first on `PATH` and record
`rustc`, `cargo` and `clippy-driver --version`. Do not rely on `rustup run 1.90.0` or
`cargo +1.90.0` alone: where another package manager's `cargo` precedes the rustup
proxies, they can resolve to a different compiler.
