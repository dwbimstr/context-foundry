# 001 — Reliable local context

Status: T001–T003 implemented and verified locally, 2026-10-01 (owner-approved scope:
001 + 003 first implementation). Not released. D001 remains resolved below. Evidence,
review provenance and accepted limitations: [validation](../../docs/validation.md).

## Outcome and baseline

Index a workspace, retrieve verbatim cited context, edit/delete source, re-index and
restart without stale evidence or lost feedback. A CLI release of this behavior is
independent of MCP, graph producers and models.

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
- **FR-003:** Search/context/retrieve obey the shared [context contract](contracts/context-v1.md):
  exact bytes, coordinates, source/workspace identity, stale-handle rejection and
  visible snapshot/index/scan limitations. Queries never read live disk implicitly
  or expand workspace ownership from paths mentioned in text.
- **FR-004:** Count actual CLI context/retrieve stdout with `o200k_base`; reject
  over-budget success. Provider usage and dollar savings are separate claims.
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
storage direction; its implementation and restart/fault acceptance remain unexecuted.

## Proposed persistence and recovery contract

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

Proposed fixed limits: file 2 MiB; relative path 4096 UTF-8 bytes; source-key page 128;
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
