# 005 — Real relationships in a large workspace

Status: Proposed; compiler-symbol integration is unimplemented. Dependencies: 001;
003 only for the agent-facing part of T003. Authorization: specification refinement.
Spec-pass decisions recorded 2026-10-03: pinned producer release, UTF-8-only positions,
the producer coverage rule, budgeted v2 `references` and MCP import through `index`.
External prerequisites (owner): a permitted jailed rust-analyzer run for T001, and the
scale corpus and numeric profile for T003.

Owner answers, 2026-10-04:
- **T001 producer.** Run the installed rust-analyzer: Homebrew 2026-08-31, i.e. weekly
  tag `2026-08-31`, commit `f8996691e991`. It ran once in a no-network `sandbox-exec`
  jail on the T001 fixture; the producer record is
  `tests/fixtures/semantic/producer.json`.
- **T003 corpus.** rust-lang/rust at tag 1.99.0 (commit `b940084d`; 62,035 files and
  214 MiB tracked). Owner decisions, 2026-10-04:
  - Foundry admits the whole repository.
  - The producer covers only the `library/` (std) workspace, after one authorized
    `cargo fetch` of its 30 registry crates outside the jail. The 20 questions target
    std symbols, and other paths are out of producer scope.
  - Numeric profile, fixed before the run: `max_index_seconds=900`,
    `max_peak_rss_bytes=2147483648`, `max_query_p95_ms=250`, `max_run_seconds=3600`.
    Producer time and RSS are disclosed separately.
  - The MCP catalog ceiling rises from 800 to 900 o200k tokens for the `references`
    tool (003).
- **T002 header segments.** Decided 2026-10-04 in context-v2 § Header line, segments
  12–14.

Current baseline: `src/graph.rs` replaces whole producer bundles, indexes both file
adjacency directions and filters endpoint hashes. Supplied symbol labels are not a
resolved symbol graph; whole-producer replacement and decoding are capped today.

## Outcome and requirements

Given a definition of `parse_record` among same-named definitions in different
modules, retrieve its actual Rust references and cited source. Repeat after an edit;
stale compiler evidence becomes unavailable until explicitly refreshed. Use one
bounded context response for the investigation instead of a mandatory graph-hop ritual.

- **FR-001:** First supported compiler relations are `defines` and `references` from
  rust-analyzer SCIP. A reference is never relabeled `calls`; requesting unsupported
  callers is `unsupported_relation`. Preserve producer/config/source evidence.
- **FR-002:** Replace one producer's contribution for one source atomically in both
  adjacency directions. Valid empty replaces old facts; malformed/stale input cannot
  partly replace a scope. Graph facts never outlive their declared input snapshot.
- **FR-003:** Logical identities are independent of storage slots and ANN neighbors.
  Bound query work and output, not stored truth. Limits, stale evidence and incomplete
  producer coverage prevent a false claim of exhaustive impact/caller absence.
- **FR-004:** Decode/import in bounded batches and exercise a declared large workspace.
  Reject an oversized document by name; never silently drop it or load the whole
  producer corpus in memory. Publication is source-atomic, not whole-run atomic.
- **FR-005:** Source indexing never runs a producer. Compiler execution is a separate
  explicit operation on permitted inputs; import of its completed artifact executes
  no workspace code and never downloads dependencies.

## Artifact and freshness contract

T001 pins one rust-analyzer weekly release tag. Owner decision 2026-10-04: the
installed 2026-08-31 build rather than a fresh download. T001 records that tag, the
binary's SHA-256 and its `--version` output. It reads that tag's
[SCIP implementation](https://github.com/rust-lang/rust-analyzer/blob/master/crates/rust-analyzer/src/cli/scip.rs)
and the matching [SCIP schema](https://github.com/scip-code/scip/blob/main/scip.proto)
revision rather than these floating `master`/`main` links. SCIP carries symbol
occurrences and roles; optional source text and position encodings need explicit
handling. The first live producer run (2026-10-04) is recorded with its jail and
snapshot hashes in `tests/fixtures/semantic/producer.json`. A bare artifact plus hashes
taken afterwards does not prove its basis.

Proposed CLI: `import-scip --index FILE --snapshot MANIFEST`. The manifest identifies
workspace ID, source revision, producer binary/version, literal invocation/config,
artifact SHA-256, and a sorted list of path/hash inputs from an isolated immutable
source snapshot. Produce the artifact **from that snapshot**. Its tree and required
compiler configuration must be captured before production; hashing the live tree
only before/after a run is insufficient. Permit no edits to the snapshot. The adapter
checks artifact digest and input hashes against the bound indexed snapshot. Missing
binding is `unbound_artifact`, mismatched revision/hash is `stale_artifact`.

Copy the supplied artifact and manifest into the already owned import scratch area,
hashing those copies before publication. Both parsing passes consume these same
copies; never hash a pathname and later reopen possibly changed input. Bound the
manifest to 64 MiB. Check scratch filesystem usage between copy/lookup batches and
stop at the 4 GiB budget with `scratch_full`; library writes may overshoot within a
batch, so record the observed peak rather than promise an OS quota. Oversized manifests
are `manifest_too_large`. Copy in at most 1 MiB buffers with cancellation checks.
Input producers must finish their files before handoff. The copied artifact must
match the copied manifest's expected digest before any graph selection changes;
completed source admission is never rolled back by import failure.
Global preflight rejects duplicate normalized document paths as `duplicate_document`
and duplicate manifest paths as `duplicate_input`, even when their hashes match.
Detect them with the existing disk-backed lookup, before changing the selected tuple;
never let artifact order choose the last definition of a source scope.

Conservative first rule: any indexed source add/change/delete invalidates **all**
compiler facts from an earlier source revision, even if endpoint hashes still match.
Resolution may depend on a third file. Feedback and memory changes do not invalidate
code facts. This trades refresh work for simple correct ownership; selective dependency
invalidation requires separate evidence before adoption. Current-disk freshness is
still not promised. Legacy manual bundles retain their existing endpoint-hash rule
and `manual`/supplied evidence label; never silently upgrade them to compiler evidence.

Each compiler producer has one selected snapshot identity in its existing metadata:
artifact digest, input-manifest digest, source revision and producer/config identity.
After global preflight/definition lookup succeeds, select that tuple before publishing
its first document. A fact is eligible only when both source freshness and this tuple
match. Replacing an artifact/config at the same source revision therefore cannot mix
old and new document facts. Interrupted imports expose a named partial snapshot; old
bytes remain for replacement/cleanup but are ineligible. Same-artifact retry is
idempotent; explicitly importing a different artifact selects that snapshot. No whole-
run atomic swap, extra generation framework or second graph writer is introduced.
Graph claims apply to the named compiler inputs/configuration, not every build variant
or unobserved external dependency. Unsupported/external resolution remains explicit.

For each SCIP document, validate regular relative path and source hash, then decode
positions. Only `UTF8CodeUnitOffsetFromLineStart` is accepted: rust-analyzer declares it
on every document (`scip.rs` read at `master` 3332931 on 2026-10-03; recheck at the
pinned tag), and any other or unspecified encoding is `unsupported_encoding`. UTF-16/32
conversion is cut until a producer needs it. Ranges come from `typed_range` and
`typed_enclosing_range` when present, otherwise from the deprecated `range` and
`enclosing_range` that rust-analyzer still emits; SCIP ranges are zero-based and
half-open. Convert to UTF-8 byte ranges and validate them against source bytes; byte
boundaries cannot split a codepoint.

Logical symbol ID is `(producer_namespace, SCIP symbol)` for global symbols and
`(producer_namespace, document_path, SCIP symbol)` for local symbols. Store explicit
definition occurrences and reference→symbol occurrences. Reference evidence includes
origin/target path, hash and range, source revision, producer/config revision and
artifact digest. Unknown/external target definitions are counted as unresolved and
not fabricated. Ambiguous multiple definitions are reported as candidates, not a
silently selected call target. Stable edge ID is SHA-256 of the compact JSON array
`[producer,kind,from_identity,from_hash,from_start,from_end,to_identity,to_hash,to_start,to_end]`.

The stored `symbol_id` is lowercase SHA-256 of compact UTF-8 JSON
`[producer_namespace,"global",SCIP_symbol]` or
`[producer_namespace,"local",document_path,SCIP_symbol]`. Stored identities stay full
64-hex values; on the wire, symbol and occurrence IDs are displayed and accepted as
their first 16 lowercase hex characters. A prefix matching more than one stored ID is
`ambiguous_symbol` with at most eight candidates. Manual file neighborhood identifiers
keep their existing separate meaning.

Persist each definition/reference occurrence once and address references by logical
symbol. Do not materialize reference×definition pairs: a generated symbol with many
definitions/references would otherwise exceed the per-document bound despite valid
input. A resolved edge uses a unique eligible target and the edge ID above. Multiple
targets retain ambiguous reference occurrences, without invented resolved edges.
Queries name at most eight ordered definition candidates (path, byte range), with a
truncation flag if more exist; bounded lookup uses the same examined-
record allowance, without scanning the rest merely to print an exact total. An
unfinished uniqueness check is unknown/truncated, never a uniquely resolved target.
Exact duplicate occurrences are deduplicated within the validated document; the
16,384 occurrence input cap is checked before deduplication.
Every occurrence has `occurrence_id` = SHA-256 of compact UTF-8 JSON
`[producer_namespace,kind,path,source_hash,start,end,symbol_id]`, where kind is
`definition` or `reference`. That ID is the stable deduplication identity of reference
evidence; `edge_id` exists only for a uniquely resolved target. Ambiguous/unresolved
results do not manufacture target coordinates to satisfy an edge schema.

## Publication, queries and limits

Transaction scope is `(producer_namespace, origin_document_path)`. Remove only that
scope's prior outgoing facts and their reverse entries; insert the validated new set
and coverage row in the same transaction. Keep other producers and sources. An empty
SCIP document is accepted-empty. An absent document is **unknown**, not empty, unless
the producer profile says otherwise: rust-analyzer skips documents with zero
occurrences and files outside its root, so for the pinned rust-analyzer profile a
manifest-listed `.rs` path absent from the artifact is `accepted_empty`, and non-`.rs`
manifest paths are outside producer scope rather than unknown coverage. A source
with unresolved references has partial coverage with their count. Failed import keeps
prior bytes: they remain eligible only if their original snapshot is still current,
and the latest import report must name the failure. Never label the new run complete.

Limits (selected engineering bounds that T003's declared corpus exercises): artifact
1 GiB total; document message 8 MiB; decoded document 16,384 occurrences;
symbol string 1024 bytes; producer namespace/revision each 128 bytes; 128 manifest/input
rows buffered at a time. Stream protobuf document boundaries rather than decode an
entire Index into a Vec. A first pass builds a temporary disk-backed definition lookup
using the existing transactional library; the second resolves/publishes one bounded
document at a time. This lookup is disposable import scratch, not a new truth store.
Missing/oversized documents fail by path and leave that scope unchanged. Keep counts
and 20 bounded error samples. Cancellation occurs between documents; retry idempotently
replaces completed scopes. Remove only positively owned scratch after success or an
explicit cleanup; no whole-run generation framework or auto-retry daemon.

On a fully consumed manifest/artifact, record completed/accepted-empty/unresolved/failed
source counts. Retire obsolete scopes only for source paths proven absent by the
completed input manifest; never infer absence from a cancelled/failed run. A run with
unresolved/unsupported inputs remains partial and cannot certify no references.

Proposed `references` operation arguments are `{symbol_id?, handle?, byte_offset?,
limit?, tokens?, after?}`: exactly one seed form, either a `symbol_id` (16-hex prefix)
or a source `handle` plus an absolute `byte_offset` within it; unknown/null fields and
both/neither forms are `invalid_argument`. Resolve the occurrence at that position
(refined 2026-10-04: rust-analyzer's module-definition occurrences span whole files):
- among all occurrences whose range contains the offset, the narrowest range wins;
- none→`symbol_not_found`;
- two or more different symbols sharing that narrowest range→`ambiguous_symbol`, with
  at most eight candidates and a truncation flag;
- a resolution scan that exhausts its examination window certifies neither uniqueness
  nor absence. It reports truncation, and so does an interrupted definition lookup.

Defaults: limit 64, max examined graph records 256, max visited
files 64; request limits 1..256. `tokens` budgets the response like search (1..32768,
default 1024; the effective budget, reservation and charge of
[003's economics contract](../003-agent-retrieval-context/contracts/adapter-economics.md)).
The result is v2 text under 001's
[shared context contract](../001-source-state-recovery/contracts/context-v2.md): its
header, then one line per reference in (path, byte start, byte end) order,
`<handle> L<line> in <kind> <qualified name>`, where the handle covers the reference's
enclosing delivery unit, `L<line>` is the occurrence's line and `<kind> <qualified name>`
names that unit. While more references remain, the last line is
`next: after=<path>#<start>-<end>`, a cursor passed back as `after` to continue (amended
2026-10-04 so that references sharing a start paginate without skipping; limits apply
per record and no tied group is indivisible). The seed's own source read counts toward
the 64-file budget. Coverage
(complete/partial/stale/unavailable) and exact examined/stale/unresolved/omitted counts
are reported in the header as context-v2 § Header line segments 12–14 (`examined`,
`unresolved`, `coverage`), together with the shared `stale`, `omitted` and
`candidates:full` segments (decided 2026-10-04). These counts cover the examined window,
not all unvisited graph records. Source/handle errors use 001 precedence.
Import failures name unbound_artifact, stale_artifact, invalid_range,
unsupported_encoding, artifact_too_large, document_too_large or producer_incomplete;
An import is `complete` when every manifest/artifact document was consumed and published
with none failed or interrupted; it exits 0. Failed or interrupted documents make a
partial import that exits 1 with committed/failed counts. Invalid invocation exits 2,
and cooperative cancellation exits 130. Coverage is reported separately (decided
2026-10-04): the import report carries `coverage: complete|partial` with its unresolved,
unknown and unsupported source counts. Unresolved or unsupported references never fail
the run, but they keep coverage `partial`, which certifies no absence. No failed
document is counted accepted-empty. A failed or cancelled run retires no scope. `complete`
coverage means complete for supported indexed input, not all possible runtime
references. Bounds do not remove facts.

`context(strategy=graph)` expands at most three retrieved source spans' overlapping
resolved symbols in that order, symbol-ID tie break. Reference occurrences contribute
their enclosing delivery units (001's search documents), deduplicated into at most 32
total units and 256 examined graph records, including definition lookups as well as
reference occurrences.
Before 009 these seeds are lexical; when semantics is enabled its merged ordering
applies, excluding unlocalized previews. Compiler occurrences are not embedding units,
and graph arrival cannot repartition source embeddings or require a second vector set.
Use 001's source-first packing. No eligible compiler graph yields source-only context
with `graph_unavailable`, `graph_stale` or `graph_invalid`, never a false empty call
graph. Existing file-neighborhood `graph PATH` remains explicitly separate from
symbol references.

### Import through the existing agent owner

005 adds `references` to 003's catalog, making six tools (`search`, `context`,
`retrieve`, `index`, `status`, `references`), and moves MCP import into `index`:
`index {scip: {index_file, snapshot_file}, timeout_ms?, root?}` performs the import
that a separate `import_scip` tool would have, for the selected root's store; without
`scip`, `index` refreshes sources as before. There is no `import_scip` tool. File names
are 1..128 ASCII characters from
`[A-Za-z0-9._-]`, excluding `.`/`..`, and contain no path components. They name regular
files directly under `<store>/imports`; reject symlinks and unavailable/unrecognized
paths as `artifact_unavailable`. The caller explicitly stages completed producer
outputs there; this directory is excluded from source admission and is not watched.
CLI import retains its explicit file paths. Both use the same frozen-copy importer.

The import inherits `index`'s timeout range/default and serialized admission. Return
the same aggregate import counts/completeness/reason, within 256 KiB; controlled partial
or cancelled work returns `complete:false` with committed counts, never an accepted-
empty graph. Preflight failures use 001's bounded tool error. Competing CLI import still
returns `store_busy`. This lets an agent re-index and import newly produced facts in
one session without stopping the store owner. The import never runs a compiler,
downloads dependencies, opens manifest-listed external sources or removes caller input
files.

## Tasks and acceptance

SC-00N is the acceptance of T00N: SC-001 is T001's outcome, SC-002 T002's and SC-003
T003's, each passing only with that task's verification.

### T001 — Import one actual Rust reference relationship

- **Depends:** 001 identity/revision/handle contract. **Scope:** new `src/scip.rs`, graph
  adapter/types in `src/graph.rs`, CLI dispatch, new `tests/semantic.rs` and a public
  fixture under `tests/fixtures/semantic`; pin only needed decoder dependencies.
- **Outcome (FR-001, FR-005 / SC-001):** capture actual tool version, command, config,
  immutable snapshot manifest and SCIP artifact; implement input validation/coordinate
  conversion and references to a real definition. No plugin framework or model code.
- **Verification:** fixture has `a::parse_record`, `b::parse_record`, two direct uses of
  `a` from separate files, one use of `b`, Unicode before an occurrence, and an indirect
  function-pointer call. Expected occurrence identities/ranges are independently
  enumerated in the fixture. Return both `a` uses, exclude `b`; preserve the pointer's
  reference without calling it a resolved runtime call. Wrong hash, malformed range,
  a non-UTF-8 position encoding and unbound artifact each name an error; a document
  carrying both typed and deprecated ranges decodes the typed one.
  Replace an input pathname after the frozen copy: both parsing passes still use the
  validated copy. A copied artifact/manifest hash mismatch makes no graph selection
  or source mutation. Manifest and scratch limits fail by name without uncontrolled
  re-copy/retry. Record copy/scratch costs with the existing import timing.
  Duplicate document/manifest paths fail before selection; local symbols with the
  same SCIP spelling in different files have different wire IDs, and returned IDs
  round-trip through `references` without private importer state.
- **Review/stop:** recorded 2026-10-03 from rust-analyzer source (`scip.rs` at
  `master` 3332931: `load_out_dirs_from_check: true`, `ProcMacroServerChoice::Sysroot`):
  `rust-analyzer scip` runs build scripts and proc macros, so every producer run needs
  deployment's compiler grants. Recheck this at the pinned tag and confirm permission
  for this fixture before execution. If the producer cannot supply correct
  references, leave SC-001 failed; do not reduce it to a manual/syntactic fixture.
  Tool availability/permission is a prerequisite, not an implicit install/build grant.
  Import-only operation can consume an already produced artifact without Foundry
  running a compiler. Any Foundry-launched producer must meet the explicit compiler
  grants in [deployment](../../docs/deployment.md), including immutable source and
  dependency inputs, no network and bounded private scratch. No compiler launcher,
  VM or generic producer runner is required merely to ship the import workflow.

### T002 — Publish and query source-scoped facts safely

- **Depends:** T001. **Scope:** scoped tables/reverse indexes and scratch import in
  `src/graph.rs`/`src/scip.rs`, `references` CLI, `tests/semantic.rs` plus restart cases.
- **Outcome (FR-002, FR-003, FR-004 / SC-002):** implement publication and limits above.
  If schema changes, add one explicit supported upgrade step preserving source,
  feedback, memory and manual graph data; older readers refuse the newer schema.
- **Verification:** replay identical input (same rows/counts); accepted-empty clears
  only one scope; failure preserves it; producer B unaffected by A replacement; edit
  a third file (unchanged endpoints) makes A's old compiler graph stale; import at
  new revision restores eligibility. Interrupt before/after scope commit and between
  documents; no half reverse index. A 300-reference hub with limit 256 truncates
  retrieval, and read-back still finds all 300 stored facts. Exercise every size cap
  at limit/limit+1 and scratch cleanup ownership. Completed scopes survive cancellation.
  Import a second artifact/config at the same source revision and interrupt after one
  document: queries expose only that selected artifact, with partial coverage, never
  a union with old-config facts. Replay resumes the same snapshot without duplicates;
  global preflight failure leaves the prior selection unchanged. Use 001's schema
  owner and preserve rows from other implemented features, including disabled ones.
  Use a generated 300-definition/300-reference symbol: store occurrences, not 90,000
  pairs; return bounded ambiguity/truncation without inventing a unique target.
  Exhaust the shared examination allowance during definition resolution and retain
  unknown coverage. Duplicate occurrences do not duplicate stored facts or bypass
  input-size caps. A manifest-listed `.rs` path absent from the artifact is
  accepted-empty and a non-`.rs` path leaves coverage unaffected; `references` output
  stays within its token budget and continues exactly through `next: after=…`; a
  16-hex prefix shared by two stored IDs is `ambiguous_symbol`. This focused fixture is
  not a new large-workspace measurement.
- **Review/cutover:** validate source revision and both directions in the transaction;
  whole-run atomicity must not be claimed. Keep legacy manual import independent;
  document explicit schema upgrade/backup rollback, with no downgrade writes.

### T003 — Complete the declared large-workspace task

- **Depends:** T002 and 003 for MCP extension. **Scope:** add the `references` tool,
  `index`'s `scip` import and graph context integration to the existing owner, focused
  protocol checks, one real run.
- **Required run inputs:** exact authorized corpus revision/dirty-file manifest, byte/
  file counts, hardware/OS, producer command and artifact identity; positive numeric
  `max_index_seconds`, `max_peak_rss_bytes`, `max_query_p95_ms` and `max_run_seconds`.
  Missing values are `scale_profile_missing` before starting. The implementation owner
  proposes the profile to the user when this real run is selected; never fills limits
  after seeing results. Profile may test one named corpus, not universal capacity.
- **Acceptance (FR-003, FR-004, FR-005 / SC-003):** profile covers at least 10,000 admitted
  source files and 100 MiB, excluding generated padding. A smaller corpus proves
  functionality only. Index/import completes within profile time/RSS limits; all
  omissions are named. Twenty preselected definition/reference questions include 15
  supported nonempty sets within the query bounds (all expected references, no extras)
  and five high-degree sets (nonempty correct subset, explicit truncation, no stored
  fact loss). Fix expected sets from source/producer evidence before timing; unavailable
  graph or all-empty results cannot pass. No stale/mislabeled reference is accepted.
  Query p95 over those 20 queries meets the declared limit (small-sample scope only;
  nearest-rank p95 is sorted sample 19). Complete one edit/re-index/reimport cycle
  with stale→fresh evidence observed, through the existing MCP owner without a restart
  or competing CLI writer. Producer execution remains a separate permitted action.
- **Verification:** record the pre-run profile, actual command/exit codes, peak RSS and
  elapsed time of Foundry separately from the external producer, raw 20 query timings,
  returned identities/ranges and expected-set comparison. The profile's RSS/index
  limits apply to Foundry; producer time/RSS are separately disclosed rather than
  silently excluded from an end-to-end performance claim.
  Check staged-file name/traversal/symlink/size errors, cancelled copy/import, and a
  competing CLI writer. Calls do not execute a producer or enroll artifact paths as
  source. Direct references remain available if lexical search alone is damaged.
- **Review/rollback:** record failed criteria and actual measured scope; do not repeat
  unchanged runs to seek a pass. Stop at run budget, retain committed state and named
  limitations; undo only session-owned host configuration. Token savings require
  actual consumer/provider comparison and are not implied by passing SC-003.

All three SCs remain unexecuted. Compiler/scale prerequisites are explicit. A failed
scale run does not block an already accepted smaller CLI or agent release.
