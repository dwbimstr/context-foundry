# Validation

## Cold verification and changed-input publication — 2026-10-10

Experimental schema 12 adds a redb publication lookup and bounded transactional
invalidation frontier. Tests exercise ordinary and lexical-only writers, explicit derived
repair, lazy model acquisition, source batching, background CPU admission and maintained
boundary relationships. Real-model lifecycle measurements remain explicitly ignored.
This uses synthetic vectors and real storage/parser/ANN/owner boundaries; it establishes work selection and recovery, not device throughput or task quality.

A 260-file addition fixture makes an offline location edit, adds one prepared source and
one zero-card source, and deletes another source. Restart publication reads exactly three
extant dirty partition rows in one shared metadata snapshot, with **zero full mapping pages**;
its locations match an independent authoritative audit. Cache-only completion after a
partial publication refreshes both paths sharing that key without a census. Exact counters,
ordinary replay, overflow, profile changes, cancellation and independent audits remain covered.
Three real subprocess aborts around lookup commit and manifest publication recover without
losing edits or re-embedding. An interrupted schema-11 upgrade remains old; explicit upgrade
preserves sources, cache and the existing generation. File substitution and corruption still
fail verification; receipts do not survive restart.

Cache-only CLI replay succeeds with the unused GGUF absent. A missing input still
refuses before acquisition, and tokenizer substitution cannot promote a partial receipt.
Three older assertions expected all model verification before any partitioning; they
were revised for lazy acquisition while preserving zero-call, cache and deadline checks.
Independent profile files hash using at most four joined scoped workers; a single GGUF
still needs a full sequential SHA-256. ANN loading hashes while filling the exact restore
buffer, with cancellation checks per MiB. Initial/overflow/repair lookup construction,
cold metadata decoding, required byte verification and compaction remain corpus-sized.
No semantic throughput or task-quality improvement is inferred from these fixtures.

Cold ingestion now commits up to four prepared sources atomically while retaining the
eight-read/16 MiB admission limit. Each changed source keeps its own source revision and
derivation obligation; reporting follows the group commit. Failure, cancellation and real
process deaths prove whole-group rollback/replay. Unchanged replay makes no source commit.
Before a reusable mapping checkpoint exists, source writes no longer populate an unusable
publication frontier. A source edit during mapping construction cannot drop a previously
processed row from the next durable checkpoint; an explicit race test covers that retry.

A bounded scale fixture uses **80,000 tiny synthetic Rust files**, 200 directories of
400 files: `d{n//400:03}/f{n:05}.rs`, containing `pub fn scale_{n:05}() -> usize { n }`.
On this Linux x86_64 sandbox (five available CPUs, Rust 1.90, unoptimized dev build,
debug information disabled), indexing exceeded the 360-second stop before the final
ingestion changes. A fresh store after those changes completed in **164.893 seconds**,
with 80,000 changed sources, no failures, a complete scan and zero pending source work.
The same corpus was reused; neither run is a cold OS-cache or optimized-release claim.
The earlier observation is censored, so no precise speedup ratio is inferred.

A real stdio MCP watcher then reconciled its 80,000-file startup baseline in **32.907
seconds**. Replacing `d199/f79999.rs` with `updated_scale_sentinel` became searchable at
revision 80,001 in **0.176 seconds**, with **one source read and zero full reconciliations**
after that baseline. Timing includes client polling. The complete fixture took 200.498
seconds; maximum child RSS was 170,076 KiB (about 166 MiB). The tested default binary SHA-256
was `1996c0609a00c72ea95d083829ea987d6aa845cb0c24e0dc9ec485f35a30ec94`.
These are single-run lexical/watcher observations; mixed-size repositories, semantic
80,000-file preparation, real models, task quality and other platforms remain unverified.

The existing watcher can maintain one explicitly enabled boundary configuration, one file
per turn, including replay, source/config changes and pause. A real MCP consumer checks the
full enable/edit/pause cycle. Optional Linux host aggregate CPU admission samples with
pause/resume hysteresis and defers on unknown observations without evicting an idle generator.
This is not a cgroup/affinity CPU quota, GPU, thermal, battery or macOS CPU implementation.

Across the affected suites, **648 tests pass**, with two explicitly ignored real-model
measurements, plus all **21 scenarios** in the separate recovery runner. This covers
library, core, neural storage/preparation/retrieval, boundary, brief, CLI, knowledge,
MCP, memory, task-context and task-evidence tests. A real build without default features,
followed by direct integration-harness execution, passes 22 boundary/task/publication
checks, including MCP and a lexical-only CLI edit followed by semantic publication.
Harness libraries retain their semantic test feature; the disabled-feature claim is for
the spawned binary. All-target Clippy, Clippy for the binary without default features,
formatting, Markdown links and diff checks pass.

The continuous-semantic fixture now includes stable pending work alongside two constantly
changing files; otherwise every candidate could correctly fail freshness checks. The HTTP
timeout fixture now starts the tested deadline at provider admission, avoiding an assumption
that preprocessing always finishes within 80 ms. Both pass with their original safety
requirements intact.

## Boundary relationships and task evidence — 2026-10-10

Experimental [005 T005](../specs/005-graph-evidence-lifecycle/spec.md#t005--connect-repository-behavior-across-process-boundaries)
and [003 T009](../specs/003-agent-retrieval-context/spec.md#t009--compose-evidence-for-a-repository-task)
are implemented on `experimental/context-ecosystem`. The environment is Linux x86_64,
Rust 1.90.0, offline cached dependencies; no GPU, downloaded model or external provider
is involved. Source fixtures are synthetic; redb/Tantivy, parsers, CLI and SDK MCP
consumers are real. The database is schema 11, upgraded explicitly with preserved data.

The boundary fixture exercises exact source spans, replay with zero parses/writes,
one-file replacement, changed and deleted dependencies, configuration changes, two
providers, dynamic calls, logging strings, shadowed/mutated bindings, parser scanner
limits and extraction caps. Named faults cover changed inputs after extraction,
cancellation, transaction failure, and real process death before/after commit followed
by restart. A schema-10 store refuses ordinary open, survives a failed upgrade and
upgrades explicitly to schema 11 with source identity preserved.

The task fixture covers implementation/consumer/configuration/test/constraint evidence,
range deduplication, actual delivered-facet accounting, token and serialized-result
byte caps, task/source races during discovery, ambiguous providers, missing tests,
ordinary-context compatibility, and derived-vocabulary discovery with source-first
boundary expansion. MCP checks the opt-in catalog, owner publication and actual result
budget. Brief records in these tests are declared synthetic, not generated by a model.

A two-service sandbox task delivers a catalog provider returning `items`, its HTTP
consumer requiring `products`, the caller's constraint to retain `/v1/items` and billing,
and the relevant test. A one-line provider repair changes the payload key. A deterministic
Python in-process transport executes the selected provider/consumer/test: it fails before
and passes after, while billing bytes stay identical. This establishes a reproducible
contract-repair fixture, not deployed HTTP behavior or autonomous model patch quality.
The response itself correctly continues to say its discovered tests are unexecuted.

Verification across the affected suites: **465 tests pass**, including all 20 new
boundary/composition tests, plus all **20 scenarios** in the separate recovery runner.
The broad run covers library, boundary, brief, CLI, core, MCP, memory, task-context and
task-evidence suites; the schema-9 knowledge-upgrade test is also checked separately.
The new brief-only fixture initially violated existing required-behavior and protected
entry rules; corrected fixture inputs pass without weakening either production rule.

- `cargo test --offline --test boundary --test task_evidence` passes on the final code.
- The affected library/CLI/MCP/storage/task/brief suites and recovery runner pass;
  `cargo test --offline --test knowledge schema_nine_requires_explicit_atomic_upgrade_without_losing_sources`
  passes after the schema-target update.
- `cargo clippy --offline --all-targets -- -D warnings`, formatting, Markdown links
  and `git diff --check` pass.
- A real `cargo build --offline --no-default-features --features test-faults --bin foundry`
  is followed by direct execution of the already compiled boundary/task-evidence test
  harnesses: all 20 pass, including CLI and MCP subprocesses against that binary.
  This avoids Cargo's self dev-dependency re-enabling the default semantic feature
  during `cargo test --no-default-features`. Library portions of those harnesses still
  use their ordinary test build; the disabled-feature claim is for the spawned binary.

Real-host outcomes, provider usage/economics, full-repository scale, other framework
patterns, parameterized routes, and multi-root task composition remain unverified or
outside this first slice. No default promotion, release or 013 learning unfreeze follows.

## Pipelined source ingestion — 2026-10-09

The next E1 increment in [001 T009/T011](../specs/001-source-state-recovery/spec.md)
overlaps bounded source reads/hash with ordered, freshness-checked source commits.
One Engine-owned Rayon pool serves reads and lexical builds across pages/calls;
ingestion drains one lexical page after 128 accepted candidates, then finishes the
existing final drain. No schema, source transaction, pending-acknowledgment or model
recipe changes. [Usage and limits](experimental-context.md#pipelined-source-ingestion)
name the separate payload budgets and unchanged explicit MCP engine-slot policy.

Linux/Rust 1.90.0 sandbox tests exercise these boundaries:

- Deliberate out-of-order reads overlap the single writer, retain walker-order
  commits and occupy at most eight active/completed result slots. Lexical builds
  run on the same three observed worker threads. A single worker is reused for
  261 lexical builds across three pages and another refresh call. Serial and
  eight-worker ingestion produce identical source tables and logical search
  documents for 260 Rust files, an empty source and an excluded binary file.
- Buffered in-place edits, atomic replacements, symlinks, removal and a changed
  exclusion decision preserve the accepted source and defer unseen deletions.
  Sensitive filenames are checked by descriptor without reading their contents;
  replacement before deletion preserves the accepted record until a clean retry.
  A read-worker panic is named, preserves prior data and permits the pool to recover.
- Cancellation after the first lexical commit in a 257-file scan leaves exactly
  128 committed sources searchable and retains an unseen old source. Reopen/retry
  reports 128 unchanged, 129 changed and one verified deletion, with no pending work.
- A real SIGABRT at the same early-publication boundary leaves 128 sources and
  their conditional pending acknowledgments. Restart reuses those versions,
  accepts the remaining 129 and verifies every final hash. The existing child-process
  source, parallel-index, repair and upgrade recovery scenarios also pass.
- Parse failures from early pages and the final drain are counted together once;
  a later unchanged scan does not inherit the previous scan's failures.

`cargo test --locked --no-fail-fast` passes on the final implementation:
**899 passed, zero failed, two existing measurements ignored**, plus all scenarios
in the separate recovery runner. This includes the CLI/MCP consumers, semantic
integration and the 6,000-file watcher fixture with foreground service and no idle
body rescans. The strengthened read/commit overlap barrier also passes separately.
`cargo clippy --locked --all-targets -- -D warnings`,
`cargo check --locked --all-targets`, library/CLI Clippy without default features,
`cargo fmt --all --check`, Markdown links/anchors and `git diff --check` pass.

These are synthetic source fixtures with real storage, parsers and lexical indexing.
They establish overlap, reuse and recovery, not a numerical throughput improvement,
80,000-file scalability or coding-task quality. Source transactions remain individual;
explicit re-indexing still reads the tree. Remaining E1 artifact handoff/cache accounting
and per-card fitting, E2/E3 incremental semantic maintenance/publication, E4 representations
and automatic generation remain pending. Self-review only; no release or deployment.

## Bootstrap work reuse — 2026-10-09

The first efficiency increment in [009 T006/T007](../specs/009-optional-semantic-retrieval/spec.md#t006t007-follow-up--remove-repeated-preparation-and-startup-work)
reuses bounded semantic CPU workers and exact tokenized cards across pipeline
stages. CLI model acquisition is lazy; source replacement no longer allocates
replacement storage chunks for unchanged content. Source/cache schemas, input
recipes, model geometry and vector meaning are unchanged.

Focused Linux/Rust 1.90.0 preparation evidence: all eight unit tests pass. A
40-definition source produces identical keys/token IDs across five batches with
one rendering/tokenization pass; retained-payload overflow takes the original
on-demand path and preserves every input. Source replacement invalidates the
handoff, differing stored tuples are refused, serial/parallel mappings match,
workers overlap, and the same CPU thread handles multiple waves and recovers
after an injected tokenizer panic. Empty/cancelled work starts no pool. Existing
partition cancellation, source-step limits and no-publication-on-error tests pass.

The full `cargo test --locked --no-fail-fast` run on the final code passes:
**891 passed, zero failed, two existing measurements ignored**, plus all scenarios
in the separate recovery runner. The 61 semantic-preparation and 56 semantic/MCP
integration tests include these new boundary checks:

- Cache-only restart and deletion-only mapping publication never call the acquisition
  seam; accepted vectors remain searchable and the previous provider observation is
  preserved. Empty/uncarded work also starts no model.
- A cold 25-source run acquires exactly once across multiple waves. A run inside
  its publication reserve never attempts startup. An acquisition that outlasts its
  deadline retains verified partitions, commits no vectors and reports the named
  budget stop. Existing provider refusal/malformed-output tests still pass.
- Existing pause/shutdown, stale-source rejection, prefetch, partial publication,
  cache corruption and child-process recovery cases pass. The watcher suite's
  6,000-file fixture retains foreground service, targeted edit updates and zero
  idle body rescans.

`cargo clippy --locked --all-targets -- -D warnings`,
`cargo check --locked --all-targets`, Clippy for the library/CLI without default
features, `cargo fmt --all --check`, Markdown links/anchors and `git diff --check`
pass. Rust 1.90.0 throughout; self-review only.

These fixtures use synthetic embeddings with real Rust tokenization and storage;
they demonstrate removed work and preserved inputs, not device throughput or
coding-task quality. Subsequent E1 ingestion/lexical work is recorded above;
artifact handoff/cache accounting and per-card fitting, T008 incremental
maintenance/publication, T009 representations and 010 generation remain pending.
No release, deployment or model-provider execution is part of this slice.

## Efficiency and incremental-maintenance audit — 2026-10-09

Inspected `62c6c97` on `experimental/context-ecosystem`: ingestion/storage, lexical
and semantic CPU work, profiles, worker acquisition/geometry, cache commits,
preparation scheduling, USearch publication, status, watching and brief lifecycle.
The [audit and model-window sources](context-ecosystem-plan.md#efficiency-audit-after-progressive-bootstrap--2026-10-09)
separate implemented reuse from pending work. Publisher metadata was read for the
pinned EmbeddingGemma 2 and Nemotron revisions and official Mistral/Codestral/Gemini
contracts. No model weights, repository uploads or provider inference were involved.

Findings: exact-input embedding reuse already works; discovery still walks source
metadata, stopped preparation does not wake on watcher edits, publication rebuilds
the complete ANN generation, status performs broad validation/censuses, and briefs
are imported rather than automatically regenerated. Cold ingestion and preparation
retain serial/repeated work. Current 128-token cards omit multiline implementation
bodies; the worker's 2,048-token total geometry is narrower than the selected model's
advertised 8,192-token input window. These are code/contract findings, not a profile
attributing elapsed time or a demonstration of long-context quality.

The revised plan selects existing 001/009 CPU/startup follow-ups, new 009 T008/T009
tasks within the existing spec, and dependency-driven 010 generation afterward.
Runtime behavior is unchanged by this documentation revision. Documentation
consistency, Markdown links/anchors and whitespace are checked; no additional Rust
tests or performance runs are warranted for this change. Prior executed evidence
below remains scoped to the implementation that produced it.

## Progressive semantic bootstrap — 2026-10-09

[009 T007](../specs/009-optional-semantic-retrieval/spec.md#t007--make-semantic-bootstrap-useful-before-a-complete-corpus-pass)
now composes bounded partition/select/embed waves in CLI and the existing MCP owner.
Explicit priority scopes alternate with the general cursor; one in-flight batch and
one bounded prefetch preserve model ownership. Publication visits prepared rows and
rebuilds the existing USearch generation at geometric growth boundaries and final
coverage changes. Cache-only pauses also publish changed locations. No schema,
model geometry, segment format or additional process changes. [Usage and limits](experimental-context.md#progressive-semantic-bootstrapping)
describe priorities, stage accounting, coverage and payload bounds.

Executed Linux/Rust 1.90.0 sandbox evidence:

- A real MCP SDK/client/server fixture with 86 indexed sources serves the explicitly
  requested semantic document while at least 77 sources remain unpartitioned. The
  general walk also progresses. A zero-card tail cannot make prefetch traverse the
  whole corpus before the first publication. A new owner resumes without repeating
  the accepted input; a completed preparation adds zero document calls on replay.
- An overlapping priority/background fixture counts one document input per eligible
  unit, including overlap with the first in-flight call. No concurrent provider calls
  are added. Invalid priorities and priorities on non-prepare operations are refused.
- CLI cancellation after two nonempty publications leaves 16 searchable inputs from
  priority and ordinary waves, with 48 of 64 sources unpartitioned. Resume embeds
  exactly those 48 and uses at most three further generation builds.
- An oversized generation request returns `resource_limit` before replacing the
  previously validated generation. A pause after source deletion republishes the
  remaining 11 locations without additional document work.
- Focused semantic checks: 48 unit tests and 114 integration tests pass, with one
  existing real-model measurement ignored. Existing malformed-provider, edit,
  cancellation, bounded prefetch, foreground admission and recovery assertions pass.

`cargo clippy --locked --all-targets -- -D warnings`,
`cargo check --locked --all-targets`, the library/CLI Clippy build without default
features, `cargo fmt --all --check`, Markdown links/anchors and `git diff --check`
pass. CLI help exposes the repeatable `--priority PATH` option.

The full `cargo test --locked --no-fail-fast` run completed with 884 passed, one
catalog-budget failure and two existing model/hardware measurements ignored; the
separate recovery runner also passed. Adding priority metadata took `tools/list` to
1,021 o200k tokens. The only subsequent code change shortened three tool descriptions,
preserving their guidance and schema constraints. The exact catalog contract now
passes at **997 tokens** under its unchanged 1,000-token ceiling. The final complete
MCP suite passes **90/90**, resolving the only failure: **885 unique tests passed**
across the full run and affected-suite rerun, with two measurements ignored. The
full run's other checks were not repeated for this wording-only correction. All
targets and the build without default features pass Clippy/check again afterward.
These tests use synthetic embedding functions with real Rust tokenization, redb,
USearch and MCP delivery; they establish scheduling/freshness/reuse behavior, not
real-model speed or task-quality gains. Stage times overlap and are wall accounting,
not device profiling. Full USearch generation builds and status coverage censuses
still scale with prepared/source metadata respectively. Device tuning, CPU pool
reuse, host-pressure admission and 80,000-file performance remain unverified.
Self-review only; no release, model download, external provider or deployment ran.
At this checkpoint the next selected increment was generation/area knowledge
(010 T002). The later efficiency audit above puts CPU/startup reuse, changed-work
maintenance, vector deltas and model-aware inputs before that work. Boundary joins
and task evidence composition remain selected.

## Incremental primary-root watching — 2026-10-09

[001 T011](../specs/001-source-state-recovery/spec.md#t011--maintain-the-admitted-workspace-from-filesystem-change-hints)
adds explicit `mcp --watch` on the existing stdio/shared-HTTP owner. Native `notify`
events feed a bounded, coalescing queue in [watch.rs](../src/watch.rs); the held-root
reader and streaming reconciler in [ingest.rs](../src/ingest.rs) publish through the
existing source transactions, seen marks and pending lexical index. No schema change
or additional service is required. [Usage and recovery](experimental-context.md#incremental-workspace-watching)
describe source delay, status, startup reconciliation and platform limits.

Executed Linux/Rust 1.90.0 sandbox evidence:

- Ten watcher unit tests cover 10,000 duplicate hints, path/byte overflow, read and
  store-event noise, ignore precedence, subtree deletion boundaries, incomplete and
  superseded scans, edits during reads, cancellation, symlink/root replacement,
  deliberately lost hints followed by rescan/restart, visible error backoff, and
  handle/brief invalidation before lexical drain. A busy known-file queue publishes
  searchable changes before its hints drain. A deterministic MCP owner test
  holds a watcher step while a foreground request waits and verifies source work
  yields to foreground admission.
- Four real SDK integration tests in [tests/watch.rs](../tests/watch.rs) pass. Native
  events drive atomic saves, new files, directory moves/deletes and ignore edits.
  Prior handles become stale. A killed stdio owner recovers edits, deletion and new
  files made offline. Default-off behavior, HTTP primary-only maintenance, reference
  isolation, bind-failure cleanup and clean store release pass.
- A synthetic **6,000-file** workspace starts from an empty store. Foreground status
  succeeds during partial reconciliation; after convergence the watcher reports
  exactly 6,000 source reads. A known-file edit needs at most three additional body
  reads (allowing duplicate native hints), causes no new root reconciliation, and is
  searchable. Both observed quiet intervals add zero source reads. This fixture
  verifies work selection and coexistence, not 80,000-file throughput or a speedup.
- The shared drain now checks pending work before treating a zero source/memory
  count as completion: briefs can occupy an entire page. A regression with 129
  pending briefs followed by source work drains completely and preserves public
  source/memory counters. All 18 brief tests pass.
- Final `cargo test --locked --no-fail-fast`: **880 passed, zero failed, two existing
  model/hardware measurements ignored**, plus all scenarios in the separate recovery
  runner. Source stayed fixed during this full run; subsequent warning cleanup only
  renamed a fault-hook parameter and gated the semantic-only status helper.
- `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo check --locked --all-targets`, the build without default features,
  `cargo fmt --all --check`, Markdown link/anchor checks and `git diff --check` pass.

Self-review only. No embedding/generation provider, real-host task-quality trial,
macOS/native-network-filesystem validation or release promotion ran. Native watch
registration memory is outside the queue bound. Notifications silently lost without
a rescan signal and ignore-policy files outside the root need explicit indexing or
owner restart; there is no periodic all-source polling. Progressive semantic bootstrap
(009 T007) follows in the newer record above.

## Source-bound brief import and conceptual discovery — 2026-10-09

[010 T001](../specs/010-source-bound-derived-knowledge/spec.md) is implemented on the
experimental branch in [brief.rs](../src/brief.rs), the shared store/index and the
source-first response packer. Schema 8 retains one structured derived brief per
source/recipe, with generator attribution, exact cited spans and full dependency
hashes. Import, get/search, explicit per-ID historical export and CAS forget use the
same authoritative owner. `context --include-briefs` maps generated vocabulary to
source for anchorless queries and can deliver derived orientation within the same
allowance as supporting code, a selected task view and explicit memory.

Executed Linux/Rust 1.90.0 sandbox evidence:

- **17 tests in [tests/brief.rs](../tests/brief.rs)** pass against real redb, Tantivy,
  response packing and independent CLI processes. The synthetic `capacity eviction`
  fixture has no baseline source match; a brief finds the opaque routine and exact
  source/test bytes are recovered. Exact-name context remains byte-identical in the
  fixture. A deliberately false summary is still only `kind:derived`; its primary
  source must fit before any summary is delivered.
- Source/dependency edits and deletes invalidate delivery before index cleanup;
  tests force edits during generation and final query validation and forget after
  candidate selection. Replay, replacement CAS, monotonic revisions, malformed
  citations/identifiers, UTF-8/JSON/text limits, escaped dependency paths, corruption,
  namespace separation and user-memory preservation pass. Restart, full repair and
  pending replacement during derived drain converge on current rows.
- Real child processes abort before/after put and forget commits. Reopen/replay
  proves row, revision and pending work are atomic at those boundaries. This is
  application recovery evidence, not a hardware power-loss claim.
- The previous schema-7 binary creates source plus task state in an isolated
  temporary store; the current binary upgrades it to schema 8 and preserves both.
  The previous binary then returns `unsupported_schema`. A separate test interrupts
  schema-7 upgrade before commit and verifies retry; old source/memory/task data is
  retained. Existing repository indexes were not upgraded.
- `cargo test --locked --no-fail-fast` exercised the full suite: 860 passed,
  four older migration assertions still expected schema 7, and two existing model/
  hardware measurements were ignored. Only those test expectations were then
  corrected; rerunning both affected suites (`--test neural --test semantic`)
  passed all 159 tests. **864 tests now pass, zero unresolved failures, two ignored**,
  plus every scenario in the separate recovery runner. Source was not edited during
  the full run; the rerun changed only the four schema assertions.
- `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo check --locked --no-default-features --lib --bin foundry`,
  `cargo fmt --all --check`, Markdown link/anchor checks and `git diff --check` pass.

These inputs are synthetic generator outputs. No generation provider, area rollup,
MCP brief tool or automatic background generation ran. A citation establishes which
bytes were used, not that model prose is correct. Freshness is relative to committed
source state; the watcher is the next build-order increment. No real-model quality,
80,000-file throughput or full-task token/cost improvement is claimed. Self-review
only; no release/default promotion or change to 013's freeze.

## Large-repository architecture and parallel partitioning — 2026-10-09

The owner redirected the experimental work toward verified task quality, practical
bootstrap at 60,000–80,000 files, LLM-derived repository knowledge, parallel execution
and continuous incremental maintenance. The
[revised design and build order](context-ecosystem-plan.md#architecture-correction-useful-context-across-6000080000-files)
records the comparison with Prakarana, read at commit
`3143213d63c22a097c77fb229bfc910e08c6e01e`. Its sources, contract tests and historical
reports were inspected; no predecessor code, service or test suite was executed.
Prakarana's draft cross-service graph spec is not treated as an implemented loader.

Findings from current Foundry source: source-index construction already parallelizes;
semantic partition computation held the redb write transaction and, in MCP, the
serving engine slot. Source watching is absent and 010 was deferred. The latest
recorded full Gemma preparation took 4,437 s for 332,732 cards; that historical elapsed
time does not identify the share caused by partitioning. Recent task-state and exact
delivery-budget features do not establish repository-understanding or task-cost gains.

[009 T006](../specs/009-optional-semantic-retrieval/spec.md#t006--bound-and-parallelize-semantic-partition-computation)
is implemented in [cache.rs](../src/neural/cache.rs),
[prepare.rs](../src/neural/prepare.rs) and [driver.rs](../src/neural/driver.rs):

- Snapshot and verify source bodies under short read transactions. Compute card
  tuples outside all store transactions and the MCP engine slot, with up to
  min(available parallelism, eight) CPU workers and at most eight source bodies per
  wave (16 MiB input bound, not a whole-process memory guarantee).
- Join workers and commit mappings in source order with a current source-hash check.
  Changed/deleted sources refuse publication; cancellation, pause and shutdown keep
  unpublished work out of storage. A worker panic is a named persistent error.
- CLI and MCP share the computation and acceptance path. Existing card recipes,
  vector identities, schema 7, model admission, caching and source outputs stay intact.
  This changes CPU preparation concurrency, not model/device batch parallelism.

Linux/Rust 1.90.0 sandbox evidence:

- Five partition tests and seven driver tests passed, including serial/parallel tuple
  equality, bounded admission, actual overlapping CPU calls, cancellation/panic,
  foreground retrieval and writes while computation has no engine slot, and pause/
  shutdown discarding unpublished work. Four of those are new tests.
- The neural lifecycle suite passed 57 tests, including the new source-change/delete
  during-computation test and restart preservation. Neural retrieval passed 54 tests
  with one real-model measurement ignored. The separate recovery runner passed all
  scenarios. Inputs/providers here are synthetic; no generation/embedding model ran.
- Final `cargo test --locked --no-fail-fast`: **847 passed, zero failed, two ignored**,
  plus every scenario in the separate recovery runner. Ignored hardware/model
  measurements are not counted as passed. `cargo clippy --locked --all-targets --
  -D warnings`, `cargo check --locked --no-default-features --lib --bin foundry`,
  `cargo fmt --check`, the repository Markdown link/anchor checker and
  `git diff --check` pass. No source was edited during the final regression run.

010 T001/T002, 001 T011, 009 T007, 005 T005 and 003 T009 now have selected contracts;
their LLM briefs, watcher, progressive publication, boundary joins and task composition
are **not implemented by this increment**. No real bootstrap speedup, 80,000-file
throughput, live coding-task improvement or provider-dollar saving is claimed.
013 stays frozen; the owner's instruction permits sandbox implementation without
waiting for external measurement gates. Evidence is self-review, not independent review.

## Task-aware retrieval and recovery search — 2026-10-09

The next sandbox increment on `experimental/context-ecosystem` implements
[008 T005](../specs/008-scoped-durable-memory/spec.md#t005--recover-evidence-by-content-and-compose-task-aware-retrieval)
and [003 T008](../specs/003-agent-retrieval-context/spec.md#t008--deliver-task-aware-source-context-under-one-allowance):

- Source `context` can include the complete selected task view. Task revision,
  original/summary representations and linked-source freshness validate inside the
  primary source query's final read transaction. A changed or forgotten task refuses
  the whole response. Multi-root retrieval includes primary task state once and
  retains the existing separate source snapshot for each root.
- The task line and ordinary evidence share one exact o200k token allowance and the
  existing byte cap. Selected entries cannot silently disappear or become summaries.
  Oversized views require explicit edits; conflicts and packing refusals refund MCP
  reservations, and successful replies charge the entire delivered text.
- Read-only task search finds retained originals, including folded/deselected
  observations, with byte-bounded pages, exact UTF-8 excerpts, hashes, attribution,
  revision checks and source freshness. Superseded history requires explicit opt-in.
- The existing schema-7 table, source transaction and packing ladder own the work.
  No new schema, index, background worker or automatic capture is introduced. The
  default MCP catalog remains unchanged; task-aware fields require the experimental
  task-context flag. See the [usage guide](experimental-context.md).

Verification on Linux, Rust/Cargo 1.90.0, using synthetic fixtures:

- `cargo test --locked --no-fail-fast`: **842 passed, zero failed, two ignored**,
  plus every scenario in the separate recovery runner. The ignored hardware/model
  measurements are not counted as passed. This complete run also verifies the
  schema assertions repaired in the preceding increment.
- [task_retrieval.rs](../tests/task_retrieval.rs) adds **11 integration tests**:
  real CLI/SDK MCP delivery, opt-in schema, exact shared budgets and MCP refunds,
  single/multi-root scope, source staleness and task-change/forget fault hooks,
  restart recovery, Unicode offsets, history and byte-limited search pagination.
  The byte-cap edge fixture was then strengthened to keep tokens below their cap
  while the mandatory context header makes the bytes overflow; all 11 tests pass
  again. The existing 19 task-context tests pass in the complete run.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and
  `cargo check --locked --no-default-features --lib --bin foundry` pass. Local
  Markdown targets and `git diff --check` are checked before commit.
- Evidence is self-review and local execution. Tool delivery is verified; actual
  host application, held-out retrieval improvement, coding-task outcomes and provider
  economics remain unmeasured. These remain future acceptance evidence and do not
  block the owner's authorized experimental sandbox work.

## Experimental context ecosystem — 2026-10-09

The owner explicitly selected an experimental Git branch, local commits and sandbox
building/testing without waiting for external measurements. Work is on
`experimental/context-ecosystem`; the retrieval increment is committed as `556b434`.
This instruction supersedes the later-milestone implementation stop in the earlier
retrieval record below. No release, default promotion or publication is claimed.

[008 T003/T004](../specs/008-scoped-durable-memory/spec.md) and
[003 T007](../specs/003-agent-retrieval-context/spec.md) are implemented experimentally
in [task_context.rs](../src/task_context.rs), the existing store and CLI/MCP owners:

- Typed task records retain immutable observations and attributed constraints,
  requirements, hypotheses, decisions, failed attempts, questions, work and artifacts.
  Updates compare revisions and commit complete batches atomically.
- Selection, pinning, folding, expansion, checkpoints and restoration edit a working
  view. Protected constraints remain visible; old views cannot revive superseded
  constraints. UTF-8 peeks recover exact original bytes and hashes after restart.
- Exact failed-action matching produces grounded advice or explicit silence within a
  byte budget. Explicit dismissal survives restart. There is no automatic polling,
  semantic constraint detector, auxiliary model call or claimed host consumption.
- Explicit decision promotion atomically writes ordinary project memory, its replay
  key and task lineage. Forget removes unchanged retained copies and invalidates
  stale search hits; independently edited copies cause an atomic conflict. Deleted
  promotion IDs cannot be reassigned within the same task's lineage.
- Schema 7 adds one table and one revision counter to the existing redb owner. Earlier
  schemas require explicit upgrade; current-schema corruption is never auto-repaired.
  The preserved schema-6 baseline binary was run against a disposable schema-7 store
  and refused with `unsupported_schema` (exit 2). The repository's existing store was
  not upgraded. Old-binary rollback requires a pre-upgrade closed-store backup.
- CLI `task-context` and MCP `--experimental-task-context` share engine operations.
  The default catalog is unchanged. MCP retains its existing frame/result bounds;
  large observations and exact exports use CLI. See the
  [usage guide](experimental-context.md) for request shapes and limits.

Verification on Linux, Rust/Cargo 1.90.0, with synthetic fixtures:

- The broad `cargo test --locked --no-fail-fast` run executed 830 ordinary tests:
  829 passed, one obsolete schema-6 assertion failed, two measurements were ignored.
  The separate recovery runner exposed a second obsolete schema-6 assertion. Both
  assertions were updated to the current schema without weakening their preservation
  checks. All other suites passed in that run.
- On the final implementation after the lineage-reuse repair,
  `cargo test --locked --test task_context --test memory --test recovery` passed:
  **19 task-context tests, 29 memory tests, and every recovery-runner scenario**.
  This includes real CLI process restarts, a real SDK MCP client, simultaneous MCP
  edits, stale-source refusal, protected entries, UTF-8 paging, exact export,
  capacity/refusal behavior, and real abrupt exits before/after apply, promotion and
  cascading-forget commits. One interaction-only observation is absent from the
  repository and still recovered exactly after folding and restart.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and
  `cargo check --locked --no-default-features --lib --bin foundry` pass. Local Markdown
  targets and `git diff --check` are checked before commit.
- The checks are self-review and executable sandbox evidence. No model solved a live
  coding task using this working view. Applied host replacement, provider economics,
  held-out retrieval benefit and broad intervention quality remain unmeasured. They
  remain future acceptance evidence, not blockers to experimental implementation.

## Retrieval execution — 2026-10-09 (partial milestone)

The owner authorized the ecosystem sequence (“go for 1 then 2 and onwards”).
001 T010, 009 T005 and 003 T006 are active; none is fully accepted by this record.
Implementation and self-review ran in the Linux cloud workspace on Rust/Cargo 1.90.0.

- [Request-local diagnostics](../src/retrieval_trace.rs) are available through
  `context --retrieval-trace-dir DIR` and the same MCP startup flag. The directory
  must be new. Records have explicit truncation and run limits, full source identity,
  selection losses, packing inputs, and the actual Foundry result. Host consumption
  remains unknown. Source/memory payloads and raw traces stay outside Git.
- [Behavioral packing](../specs/001-source-state-recovery/contracts/context-v2.md#experimental-behavioral-support--001-t010-2026-10-09)
  is implemented behind `--experimental-behavior-context`, **default off**. It uses
  existing candidates and changes neither embeddings nor candidate recall. Locator,
  unknown-intent, ambiguous-name and graph behavior retain the default path.
- On the preserved local Foundry index at source revision 222, the development query
  “How does `QueryRuntime` load the dense index on the first query?” exposed a packing
  loss: `QueryRuntime::dense_index` was candidate rank 2 (zero based), survived the
  four-per-file and 32-unit limits, and reached packing, but the baseline delivered
  only the struct. The candidate delivers the method body and the owner's address.
  At the same 1,024-token request cap, delivered tokens are **437 → 357**, and shown
  items **1 → 2**. Both use the same indexed source identity and no semantic profile.
  This is one local development example, not a retrieval-quality rate, provider-cost
  result or live coding-task result. A separate public Rust fixture exercises
  the same candidate-kept/packing-omitted mechanism.
- Fixtures cover stale and capped candidates, dense/lexical placement tracing,
  incomplete traces and private file permissions, unchanged traced response bytes,
  actual stdio success/refusal records, Rust/Python/TypeScript methods, budget sweeps,
  locator/unknown/graph controls and cross-root ambiguity. The actual stdio test
  confirms that the opt-in flag reaches the packing policy without a tool-schema
  change.
- Rust validation: `cargo fmt --check`, `cargo check --locked --all-targets`,
  `cargo clippy --locked --all-targets -- -D warnings`, and
  `cargo check --locked --no-default-features --lib --bin foundry` pass on 1.90.0.
  The final `cargo test --locked --no-fail-fast` run after review changes passed
  **812 tests, zero failed, two ignored**, plus the recovery runner. The ignored
  hardware/lifecycle measurements are not counted as passed. This is local contract
  verification, not the unexecuted retrieval-quality or live-host gates below.
- [003 T006](../specs/003-agent-retrieval-context/spec.md#t006--validate-retrieval-repairs-in-real-coding-tasks)
  records the host capability inspection and matched-run procedure. Codex CLI
  `0.159.0-alpha.3` parses the command-local MCP configuration; no model task or
  provider-usage measurement ran. Pinned OMP source exposes context/payload hooks,
  but that corpus pin is package 18.5.0, distinct from the previously measured 18.6.0
  host. Upstream source/tests were inspected, not executed or treated as host proof.

**Remaining gates:** the private G1/G2 questions/responses, prepared Gemma apparatus
and Metal-capable measurement host are absent here. The 249 historical G2 misses
remain unattributed; no anchorless repair has been selected and no fresh holdout
comparison has run. OMP/Bun are not installed here, and matched live host tasks,
complete provider economics and applied-view acknowledgment are unexecuted. The
candidate stays opt-in. Milestones 3–6 retain their dependency gates and are not
implemented by this change. Existing signed-package requirements and the 013 freeze
remain in force.

## v0.1.0 local release — 2026-10-04 (withdrawn)

**Withdrawn the same day.** The owner decided that a version is released only when
every active spec is complete (constitution 0.5.0), so the local tag `v0.1.0` and its
artifacts were deleted; nothing had been published or pushed. This record remains as
history of the checklist run against the scope implemented at the time.

Owner decision 2026-10-04, since withdrawn: a local tag `v0.1.0` and a macOS arm64
artifact; nothing published or pushed. Checklist (`docs/release.md`):

1. **Scope and evidence:** 001 T001–T006 (with the T005 leading-run amendment), 003
   T001–T003 and T005, 007 T001; acceptance evidence and limitations are the sections
   below. Advertised integrations were exercised with their real consumers: OMP 18.6.0
   and Codex 0.159.2 ran against this release binary in the 2026-10-04 runbook.
2. **Checks:** the integrated gates (fmt, clippy `-D warnings`, 258 passing Rust tests
   plus 16 custom-harness recovery scenarios with one payload report ignored, Rust 1.90
   check and clippy) ran on build inputs identical to the tag's `src/`, `tests/`,
   `Cargo.toml` and `Cargo.lock`. Install smoke:
   `cargo install --path . --locked` into a scratch root, then 18 README steps on fresh
   stores (missing store, index, search, context, status, graph import and traversal,
   retrieve, re-index, bootstrap inspect/apply, connect for OMP and Codex, usage help)
   all exited as documented; the installed binary equals the release binary.
3. **Compatibility and data:** stores are schema 2 (`upgrade-store --to 2` from v1);
   search schema `"3"` makes older indexes report `repair_required` until
   `repair-index`, preserving source, revisions and feedback. No automatic migration or
   downgrade; rollback is restoring a store copy and the previous binary. Removing the
   binary never deletes stores.
4. **Contents and licenses:** 154 tracked files, no credential or private data patterns
   (the four private-key-marker matches are deny-rule string literals at
   `src/ingest.rs:489–490` and `tests/fixtures/.economics/src/ingest.rs:489–490`). The
   artifact carries `LICENSE` (MIT)
   and `THIRD-PARTY/` with the license/notice files of the 287 runtime dependency
   packages; 20 crate packages ship no license file, so a supplied notice records each
   declared license with canonical texts. The binary links only system libraries and
   contains no fault-hook strings.
5. **Artifact:** `context-foundry-v0.1.0-macos-arm64.tar.gz` (binary, README, LICENSE,
   release notes, THIRD-PARTY) and `context-foundry-v0.1.0-source.tar.gz` (`git archive`
   of the tag), with `SHA256SUMS`; binary SHA-256 `e32204fb…6cee7`. Local preparation
   only.

## Measurements on the development host — 2026-10-08

The owner reversed the cloud handoff below because the baseline stores are too large to copy. The remaining rows ran on this host (Mac15,6, 12 cores, 18 GiB), one job at a time, using the frozen apparatus (`FROZEN.txt` of 11:31:14Z, with addenda at 14:45Z and 18:48Z).

Every result below comes from a run with 0 retried, refused, missing or duplicate tasks. INVALID runs are named and not used.

A timing run counts only if it had no swapouts and the 1-minute load was below 6 before it started. The owner's resident services keep the idle load at 4–7, so the usual idle rule (load below 2) could not be met. The owner stopped three resident model servers for the timing rows.

**Result:** EmbeddingGemma 2 is the selected profile. Semantic retrieval stays off by default and unadvertised until the installed-artifact checks pass on the Developer ID-signed package.

- **G2 (row 1).** Foundry `8035faa` (`0ca21c8e…`); K = 2; pass requires one-sided p < 0.025.
  - **No-profile baseline:** delivered evidence on 0 of 388 questions. The result is identical on `90a82cd`. Two checks confirm the scoring:
    - A positive control of 6 questions that name the definition passed 6 of 6.
    - The 21 baseline responses that name the answer's file deliver other units from that file.
  - **EmbeddingGemma 2** (`gemma.json` `bb1c9d08…`): **ENABLE.**
    - 139 of 388 gained (rust 45/197, bun 94/191), 0 lost; p = 1.43e-42.
    - Mean delivered tokens 2,019.7 → 2,018.1.
  - **Nemotron 3 Embed 1B** (`nemotron.json` `619205ad…`): **INVALID twice.**
    - In both runs the first rust question was refused once and retried.
    - The refusal code was not captured. [inference] The likely cause is the first dense query loading the 2.74 GB rust index past the read deadline.
    - For information only: 147 gained, 0 lost; mean tokens 2,019.7 → 2,015.2.
    - The owner closed Nemotron: it is not enabled and will not be measured further.
- **G1 with EmbeddingGemma 2 (row 2): PASS.**
  - Sets: checker (6,881 tasks), qualified (2,551) and bun (999), via `g1quick.sh … 1/1 1`.
  - The run used one shard. Six shards, each with its own worker, swapped this host and produced 44 retries (INVALID).
  - No target or guard that passes without the profile fails with it.
  - First-call passes, task by task against the no-profile run:
    - **checker:** 3,741 → 3,716. Every change is on an unmarked wording: 26 definition and 7 usage tasks lost; 3 definition and 5 usage tasks gained.
    - **qualified:** unchanged.
    - **bun:** 971 → 969.
  - The largest guard drop is checker definition/unmarked first-call, 55.0% → 51.7%. It stays above its frozen baseline of 37.4%.
- **Nemotron conversion.**
  - Converted offline from the local BF16 safetensors with llama.cpp `b9acf138`'s converter (`mistral3.attention.causal = False`); GGUF `e0cd7ff3…`.
  - Worker against the torch BF16 bidirectional reference, 12 inputs: min cosine 0.999691. A causal reference scores only 0.62–0.90.
- **Batched against single-sequence vectors (row 3), EmbeddingGemma 2: PASS.**
  - The ignored `embed_worker` test on the frozen bundle: all 18 rows have cosine ≥ 0.9999. The lowest is 0.9999878; a 128-token heterogeneous batch scores 0.9999962.
  - Max absolute difference: 5.79e-4.
  - A 2,048-token query embeds; a 2,049-token query is refused.
- **T009 index time (row 6).** Release builds, fresh stores, `/usr/bin/time -l`.

  | Corpus | Build | Valid runs | Wall time | Peak footprint |
  | --- | --- | --- | --- | --- |
  | oh-my-pi (8,436 sources) | before T009 (`fb4511c`) | 3 | 56.1–57.2 s | 328–350 MB |
  | oh-my-pi | T009 (`89a8c54`) | 1 | 54.9 s | 368 MB |
  | oh-my-pi | main (`d61f1da`) | 2 | 53.2–54.4 s | 364–375 MB |
  | rust-lang/rust (60,739 sources) | before T009 | 0 | none valid | — |
  | rust-lang/rust | T009 | 1 | 363.5 s | 703 MB |
  | rust-lang/rust | main | 2 | 361.2–365.5 s | 731–733 MB |

  - On oh-my-pi, comparing the fastest valid runs, T009 takes 0.978× and main 0.949× the before-T009 time.
  - Every rust before-T009 run swapped (4 attempts, wall 361.6–390.1 s). In total, 15 runs swapped and are not used.
  - In the valid runs, user plus system time is about 18–51% of wall time. [inference] Indexing on this host is bound by I/O under memory pressure, so T009's eight build threads give no visible wall-time gain here.
- **Door latency (row 7).** Main `d61f1da`, `context` at 2,048 tokens, 3 passes, 0 swapouts. Every response text equals its G1 response.

  | Corpus | Group | Queries | p50 | p95 |
  | --- | --- | --- | --- | --- |
  | rust-lang/rust with SCIP | tie group (`doors:each`) | 30 | 50.2 ms | 125.7 ms |
  | rust-lang/rust with SCIP | resolved anchor (`doors:exact`) | 30 | 35.4 ms | 113.7 ms |
  | rust-lang/rust with SCIP | approximate (`doors:approx`) | 11 | 28.4 ms | 85.0 ms |
  | oh-my-pi | tie group, approximate doors | 14 | 61.3 ms | 170.5 ms |
  | oh-my-pi | approximate (`doors:approx`) | 30 | 53.3 ms | 80.6 ms |

**Preparation (record; rows 1 and 5).** `semantic prepare --budget-seconds 3600 --development-isolation`, repeated until `partial` is false. Peak is the foundry process's footprint.

| Store | Profile | Rounds | Cards | Embedded | Input tokens | Elapsed | Peak |
| --- | --- | --- | --- | --- | --- | --- | --- |
| oh-my-pi | EmbeddingGemma 2 | 1 | 112,549 | 111,239 | 6,798,414 | 1,256 s | 1.97 GB |
| rust-lang/rust | EmbeddingGemma 2 | 2 | 332,732 | 331,556 | 19,303,357 | 4,437 s | 4.72 GB |
| oh-my-pi | Nemotron | 2 | 112,549 | 111,240 | 5,848,851 | 4,129 s | 3.60 GB |
| rust-lang/rust | Nemotron, `--cache-cap 4294967296` | 4 | 332,732 | 331,556 | 16,213,239 | 12,884 s | 7.63 GB |

- **Library-only rust-lang/rust store, EmbeddingGemma 2 (row 5; the 2,255 files of `library/`).**
  - 62,023 cards; 61,674 embedded; 4,681,997 input tokens; 7,710 document calls.
  - Cache 194 MB; foundry peak 1.11–1.12 GB across attempts; worker peak 1.27 GB.
  - Its time is not accepted: all three attempts swapped (65,628–953,714 swapouts). They took 694–747 s, against 2,139 s for 15,026 body units with EmbeddingGemma at 768 dimensions on 2026-10-07.
- **Nemotron's rust index.** The 2,048-dimension index needed 2.74 GB of cache, hence the cap setting below. Publishing it at about 292k rows overran one 3,600 s round.

## Measurement handoff and cache cap — 2026-10-08

- **Handoff.** The owner moved the remaining real-model and timing measurements (G2, G1 with a semantic profile, vector parity, the installed artifact, preparation cost, T009 timing, tie-group door latency, rollback re-embedding) to the [measurement handoff](measurement-handoff.md) for a cloud agent: this 18 GiB host swapped under six G1 shards with a resident worker each, and the resulting deadline refusals made that run INVALID. The frozen inputs that lived only under `/private/tmp` (the G1 baseline stores, SCIP artifact, baseline binary and token counter) were cloned into the G1 dataset directory outside Git. Semantic retrieval stays off by default and unadvertised until the handoff's rows 1–4 pass.
- **Cache cap setting.** G2 preparation of the full rust-lang/rust store found 332,732 cards, 2.74 GB at 2,048 dimensions, above 009's fixed 2 GiB cap, so preparation could never complete. The owner made the cap an operator setting: `semantic prepare --cache-cap BYTES` and `mcp --semantic-cache-cap BYTES` (default 2 GiB, not part of the profile or its function digest); the run records it and `semantic status` reports it, and a run under a larger cap resumes from the kept rows. Cross-lab review by OpenAI GPT-6.1 Sol: SHIP with one minor (the older-binary test fixture now omits the new field). Author: Anthropic Opus 5.5.

## 009 T004 address cards on a llama.cpp worker — 2026-10-08

Merged at `fd01489`; unreleased. Its real-model and timing checks (G2, batched against single-sequence vectors, the installed artifact's isolation, resource and numerical checks, preparation time and footprint) are in the [measurement handoff](measurement-handoff.md); nothing here ran a model.
- **Shape.** One card per definition and per Markdown section (address, signature or head lines, leading documentation; 128 tokens by default under the profile's template), recipe `cards-v1+units5`, so a unit change re-partitions; acceptance takes only the cards the verified body renders. Dense retrieval serves only anchor-less queries, its units first in similarity order and tagged `[semantic]`, then the lexical candidates in today's order; an anchored query never calls the model and its output is byte-identical to no profile. RRF and the semantic preview ladder are removed. Descriptor and profile v2 own the dimension, templates, pooling and card limit; cache rows are self-describing and each run reconciles the byte total from the actual rows; generation v3 records its geometry; one batch is prefetched and a failed prefetch still publishes the committed batch. `foundry-embed` links llama.cpp `b9acf138` statically (Metal), built by `build.rs` from the pinned commit's tree with fixed flags (`GGML_NATIVE=OFF`, portable across Apple silicon), refuses split GGUFs, and writes llama.cpp's license for THIRD-PARTY (`--notices`). pyo3, the MLX worker and its Python closure are gone; a descriptor v1 profile is refused with `profile_unsupported`, and an upgrade over one keeps the core and disables semantic retrieval by name.
- **Review.** Cross-lab by OpenAI GPT-6.1 Sol over four rounds: REVISE (6 major: card provenance not bound to its input, cache-cap accounting after a binary rollback, a prefetch failure hiding the committed batch, whole-file re-tokenization per batch, unhashed GGUF shards, linked archives not tied to the pinned source; 1 minor), REVISE (card partitions not invalidated by 001 T008's unit change), SHIP, SHIP on the gate fixes. Author: Anthropic Opus 5.5 (four continuation sessions).
- **Gates.** fmt, clippy (stable and Rust 1.90), the full test suite and Rust 1.90 check passed at `fd01489`; the gate found two tests whose expectations predated T004 (an anchored query's header has no `semantic:` word; the prefetch takes the engine slot for one selection step during an inference) and two lints.
- **G1 unchanged.** Without a profile, G1 on `fd01489` (binary `b9b01e93…`) matches `483810a` on every task of all three sets (0 of 10,431 differ in pass, body or tokens).
- **Rollback check (no model).** No release exists, so the 005 T004 release build (its semantic code equals `7da1125`) read a store written by T004 with the fake worker: rows counted corrupt, the v3 generation unavailable by name, source status and search intact, `semantic purge` removing every row. Re-embedding under its own v1 profile needs the MLX model and is in the measurement handoff, needed only if a pre-T004 build was ever distributed.

## G1 city-map verdict — 2026-10-08: PASS

On main `483810a` (001 T007, T008, T009; 005 T004 with its tie-group amendment), release binary SHA-256 `cd99e35a…`, deterministic full sets through the frozen apparatus (`g1quick.sh` at SAMPLE=1/1, its acceptance mode), stores copied from the frozen baselines and brought to schema `"5"` by `repair-index` (rust-lang/rust 52 s, 580 MB peak). Every set has 0 duplicate, missing, retried or apparatus-error tasks.

| Set | Targets (baseline → city map) | Tokens |
| --- | --- | --- |
| checker | D1 89.1 → 97.8%, D2 82.1 → 98.0%, D3 77.1 → 92.8%, U1 8.2 → 86.6% | median to pass 806 → 234 (grep 682); mean 2,056 → 1,440 |
| qualified | D4 45.7 → 94.0%, U2 8.0 → 81.2% | median 2,017 → 234 (grep 674); mean 2,313 → 1,153 |
| bun | B1 93.1 → 99.1%, B2 16.2 → 96.8% | median 1,994 → 184 (grep 647); mean 1,718 → 455 |

Every guard passes; the closest are definitions with more than 16 names (pass@1 20.9 → 25.0%, body 20.5 → 21.7%) and usage with more than 16 names (6.3 → 6.8%, allowed drop 0.6). Usage with 2–4 names went 54.3 → 86.4% (50.5% before the amendment) and with 5–16 names 38.3 → 48.9%. Results, responses and the per-task comparisons stay outside Git.

## 005 T004 amendment: doors for a tie group — 2026-10-08

Merged at `483810a`; unreleased. When a first anchor is ambiguous and its tie group (the window entries the resolver cannot tell apart) has at most 4 definitions, each gets its own exact door group of at most 4 lines right after its entry (`doors:each`), packed after the address pass and before signature upgrades; an entry without exact doors gets none, and with none for every entry the name's approximate doors follow the list once, joined from every tie root. A stale entry promotes nothing, in one root or across roots. At most 4 references windows per root's final read. A Head form for ambiguous lists was rejected: both refuters (Anthropic Opus 5.5, OpenAI GPT-6 Astra) showed it would be a directory line credited only by the scorer's fence rule.
- **Review.** Cross-lab by OpenAI GPT-6.1 Sol over two rounds: REVISE (final backtracking could split a door group; the multi-root approximate fallback took only the first root's group), SHIP. Author: Anthropic Opus 5.5.
- **Gates.** fmt, the full test suite and Rust 1.90 check passed at `70b97e6`; two clippy findings (a redundant closure, a test type) were fixed in `483810a` and clippy then passed on stable and Rust 1.90.
- **Latency.** Exact-door latency for common names with 2–4 namesakes (up to 4 windows per root) was recorded on 2026-10-08, under *Measurements on the development host* above.

## 001 T008 languages — 2026-10-08

Merged at `3e0a117`; unreleased. G1's final verdict is recorded on the city-map binary with the 005 T004 tie-group amendment.
- **Shape.** Runtime `tree-sitter` 0.26 with 15 more grammars (C#, F#, VB.NET, PHP, Perl, shell, PowerShell, Ruby, Kotlin, Swift, Scala, Lua, Dart, Elixir, Haskell; release binary 44.6 → 92.4 MB); search schema `"5"`. Every parse reads one character per input call under a 2^25-unit work budget, so a stop is deterministic; a stopped or panicking parse indexes as plain blocks (`parse_stopped`, `parse_panicked`). A scanner audit of all 22 external scanners (call graphs, loops, serializers, about 400,000 fuzzed parses) found five native hazards, guarded before parsing (F# comment nesting and Perl bracket nesting above 8,192, Perl heredoc words over 512 bytes, Python sources with over 256 indent widths, a Kotlin `@` with no line break after it). Three grammars resolve from owner-approved forks pinned by git rev: Ruby `1a594bf` (heredoc serialization bound), Perl `0686313` (scanner frees its heredoc queues; it leaked about 1 KB per heredoc per parse: +7.3 MB over 2,100 parses before, +16 KiB after) and Rust `d43cece` (rustc 1.99 nightly syntax: `const impl`, `[const]` bounds, const/auto/impl-restricted traits, macros 2.0, `safe` extern items, trait aliases; error files under `library/` 188 → 1 and `compiler/` 63 → 1; 34,707 of 34,723 error-free corpus files keep a byte-identical tree, the other 16 were silent mis-parses). Packaging reads license files from the source Cargo resolved (`cargo metadata`), so git pins package. A unit keeps at most 16 qualifiers (nested one-letter modules: 864 → 197 MB peak indexing a 200 KB source).
- **Review.** Cross-lab by OpenAI GPT-6.1 Sol over four rounds: REVISE (6 major), REVISE (packaging from a git pin, the Perl scanner leak, a quadratic leading-comment walk, PowerShell operand alignment), REVISE (`safe`/`auto` type names in the Rust fork, interpolated PowerShell strings), SHIP with one minor (a bare PowerShell token containing `$` can still give a key; fixed with the next merge). Captain findings fixed in review: a Perl `package` statement dropped its last `sub` (the grammar's last block runs over trailing whitespace), qualifier memory. Author: Anthropic Opus 5.5.
- **Gates.** fmt, clippy (stable and Rust 1.90), the full test suite and Rust 1.90 check passed at `3e0a117`.
- **G1** (deterministic, full sets, against `0882766`): checker D1 97.8%, D2 98.0%, D3 92.8%, U1 84.2% and every guard but usage on 2–4 names (50.5% against 54.3%, 50.1% before T008); qualified PASS (D4 94.0%, U2 60.0%); bun unchanged. Tasks whose result changed: checker 209 of 6,881 (definitions 135 gained, 22 lost, 9 otherwise changed; usage 32 gained, 5 lost, 6 otherwise), qualified 119 of 2,551 (111 gained, 3 lost, 5 otherwise), bun 0 of 999. The losses are definitions with more than 4 namesakes, which newly parsed files add.

## 005 T004 doors — 2026-10-08

Merged at `d00fc19`; unreleased. G1 judges it with 001 T007; the verdict is recorded once on the final city-map binary (001 T008's Rust grammar fork changes the checker set's definitions).
- **Shape.** A context request that asks who uses a resolved anchor gets doors: exact doors from the definition's stored name range through the same references window as `references {handle}` (shared file window and record allowance, inside the response's final read), else approximate doors from `ident` and index-time import keys; one door line per file, 16 lines, then `⋯`. Header states `doors:exact|approx|ambiguous|none`; `candidates:full` and `stale:` cover door windows; `graph:` is removed with the span-seeded graph expansion. `references` takes a handle alone. A target that goes stale before the final read gives `doors:none` and never promotes a runner-up, in one root or across roots (pre-validation heads carried through the merge); later roots follow the first serving root's decided strategy.
- **Review.** Cross-lab by OpenAI GPT-6.1 Sol over two rounds: REVISE (5 major: per-scope hash check of the file cache, the primary's doors request reaching other roots, stale-target promotion, a lost exhausted-scan flag, the door window not shared with `references`; 1 minor docs), SHIP. Each major has a regression test that failed before the fix. Author: Anthropic Opus 5.5.
- **Gates.** fmt, clippy (stable and Rust 1.90), the full test suite and Rust 1.90 check passed at `d00fc19`.
- **Integrated G1 before the review fixes** (T007 + T004, deterministic, full sets): bun PASS (B1 99.1%, B2 96.8%); qualified PASS (D4 89.7%, U2 58.1%); checker FAIL on D2 93.6%, D3 89.1% and the usage guards for 2–4 names (50.1% against 54.3%) and 5–16 names (35.1% against 38.3%). Most checker definition misses (27 of 35 D2, 39 of 77 D3, 17 of 24 D1) had the definition inside a tree-sitter-rust parse error on nightly syntax; the owner chose to fork the grammar (001 T008).

## 001 T007 city-map core — 2026-10-08

Merged at `4e23dee`; unreleased. Its G1 verdict is recorded with 005 T004 (G1 judges them together).
- **Shape.** Search schema `"4"`: one definition document per unit (the one holding its name node) with role, exact-case name hash, address hashes, name range and per-file import keys. Queries take up to four anchors; each anchor gets a resolver window (qualifier hits, exact case, role, then today's tier-1 order). A resolved anchor shows its definition through verbatim, signature and `[address]` plus up to 8 namesakes; an ambiguous one lists up to 16 in three passes (address lines for all, then signature, then verbatim upgrades). Compact context and its pointers are removed. In a multi-root owner the anchors are chosen once over every root. New definition units after G1 samples exposed unanswerable names: Rust trait/extern declarations and enum variants, module-level bindings in TS/JS, Python and Go, enum members in TS, Java, C and C++, named Go type specs; Rust `impl` blocks are containers. Address qualifiers come from the syntax tree, never from qname text.
- **Review.** Cross-lab by OpenAI GPT-6.1 Sol over three rounds: REVISE (4 major: cross-root anchor selection, `->` in generic stripping, quoted TS enum members, grouped Go specs; 3 minor), REVISE (comments and C++ shifts in generics, escaped quoted names: fixed by reading qualifiers from the tree), SHIP. A greedy packing pass that could drop an ambiguous candidate while showing another's body was found through a nondeterministic test (7 of 12 runs failed) and replaced by the three-pass rule. Author: Anthropic Opus 5.5.
- **Gates.** fmt, clippy (stable and Rust 1.90), the full test suite and Rust 1.90 check passed. Ten tests whose queries now anchor were reworded to keep their intent (plain-word queries for graph, dense and two-tier paths; expected bytes in anchor order for 013's composition).
- **Named limitations.** Definitions generated by macros (`define!`, `cfg_select!` bodies) have no unit. Constructs tree-sitter-rust 0.24.2 does not parse (the `[const]` bound, `const impl`, macros 2.0) had none either; 001 T008's grammar fork parses them.

## 001 T009 parallel indexing — 2026-10-08

Accepted locally at `89a8c54`; unreleased.
- **Shape.** `src/store.rs` refresh hands each page's sources out in key order to min(available parallelism, 8) scoped threads; at most 64 MiB of source bytes are out and not yet added. The single writer adds documents in key order as builds finish, then commits and reloads before the version-matched pending clear. A parse panic (`catch_unwind` per source) gives a named scan failure and the plain blocks of an unmapped source whose first document carries kind `unparsed`; `status` reports `parse_failures` by counting those documents and samples at most 20. A worker panic outside the parse guard, a writer-side panic and a corrupt recorded length end the page with an error, never a hang. Cancellation is checked at each hand-out and after the commit. No new dependency, table, META key or schema change.
- **Tests.** 1 versus 8 threads give equal documents and tables with the earliest key finishing last; panic fallback, status counting and persistence until reparse or `repair-index`; the hand-out bound counting built-but-unadded sources (at a bound reduced through a test hook); add, commit, reload and cancellation faults with replay; two crash stages, one with a build still in flight. The T009 tests take about 11 s in total.
- **G1 identity.** Copies of the G1 baseline stores (rust-lang/rust, oh-my-pi) rebuilt by the T009 release binary's `repair-index` gave the same result on all 10,431 G1 tasks (pass, calls, tokens and body), with no repetition, exclusion, duplicate or missing task.
- **Review.** Cross-lab by OpenAI GPT-6.1 Sol over four rounds: REVISE (M1–M5, m1–m2; M5 rejected by the captain and the contract amended), REVISE (m1), SHIP, then SHIP on the `unparsed` census changes made after cross-lab refutation by GPT-6 Astra. Author: Anthropic Opus 5.5.
- **Gates.** fmt, clippy (stable and Rust 1.90) and Rust 1.90 check passed. The test suite passed except one load-sensitive test, `tests/mcp.rs::http_dropped_stream_keeps_slot_and_overload_keeps_control_traffic_serviceable`: it drops an `index` request after 300 ms and expects the engine to be busy, and at host load 20–150 the request had not started yet. It passed 3 of 3 runs alone. It does not touch T009's code and is a pre-existing timing assumption.
- **Not measured:** index time and peak memory before and after, in the [measurement handoff](measurement-handoff.md) as a record.

## City-map evidence — 2026-10-07

Deterministic runs and arithmetic on recorded responses; no model calls. They motivate the owner-approved city map (001 T007–T009, 005 T004, 009 T004) and are not acceptance of it.
- **One call by name ambiguity** (library-only rust-lang/rust store, lexical, the frozen checker tasks, required evidence from `search` or `graph`): names with one library definition (SCIP name counts, a proxy for Foundry's) 74% dev / 74% held-out; 2–4 definitions 68% / 65%; 5–16 59% / 49%; 17–64 32% / 21%; more than 64 10% / 6%. On the full-repository store the comparable `search`-only definition figures were 77% / 71% and 7% / 2% (2026-10-06, below).
- **A second, scripted call recovers usage questions when the definition is known.** 400 failing tasks sampled per kind (seed 11), library-only lexical store:
  - definition failures: the definition was within the first 10 `search` locators for 8% (dev) / 2% (held-out) and within 64 for 27% / 32%; most of the rest have more than 64 same-name definitions;
  - usage failures: `search` (limit 64) listed the definition for 68% / 64%; given that definition (the script picked the correct one among the hits, which was hit #1 in 92 of 400), `references` covered the required sites for 265 of 265, median about 185 delivered tokens; with a single library definition, 68 of 78 failures recovered.
- **Path-seeded graph expansion expands the wrong symbols.** ``context "callers of `sleep_ms`" --strategy graph`` on the full store returned `fn sleep_ms` and then `Duration` constants (`Duration::SECOND`, `MILLISECOND`, …): `context_graph_units` seeds from every occurrence inside the top units, not from the queried symbol.
- **A Bun monorepo** (oh-my-pi `5b8d5b8a15`, 8,436 files indexed in 65 s, 348 MB peak, single-threaded): `interface ToolSession` (`packages/coding-agent/src/tools/index.ts` lines 207–555, 19,500 bytes) is the first `search` hit for `ToolSession`, and the first context item at 8,192 tokens, but absent at 2,048: neither its verbatim nor its signature form fits and units have no smaller form, so the response delivered test helpers `function toolSession` (seven test files; tier 1's `def_name` is case-folded) instead. `bun.lock` and 217 `.snap` files are indexed as ordinary text.
- **Languages.** Foundry and Prakarana both parse the same eight code languages (Rust, Python, TypeScript/TSX, JavaScript, Go, C, C++, Java); every other UTF-8 file is plain-text only.
- **Grammar spike for 001 T008** (outside the repository): all 23 grammars build and parse on `tree-sitter` 0.26.13 with Rust 1.90; the original eight gave identical units on 7,357 of 7,358 real files and their syntax tests pass unchanged; the grammar archives add about 48 MB to a release binary (F# 14.1 MB); deeply nested Haskell and F# inputs parse in tens of seconds (a 68 KB Haskell file in 42 s), so T008 bounds every parse by a deterministic work budget; unparsed constructs are named in the contract's § Languages.
- **G1 apparatus and baselines** (001 § G1; frozen 2026-10-08 00:22Z before any city-map code, refrozen 00:51Z after two cross-lab refutations: apparatus, name-count buckets and four targets). The first baselines (G1 v2/v3, 2026-10-07) were discarded: two runs on the same tasks differed because ripgrep's parallel walk reorders the grep agent's output before its 50 KiB cut (7 of 21 grep responses equal), and on a host at load 120–150 the Foundry agent's calls hit the 5 s read deadline (49 of 1,000 bun tasks). G1 v4 sorts grep output, records repeated refusals, and its verdict fails closed (INVALID) on any repetition, exclusion, duplicate or missing task. Two independent full Foundry baseline runs agree on all 10,431 scored tasks (pass, calls, tokens); the second, on a quiet host, needed 0 repetitions. The first bucket regex miscounted `const fn` and `static mut` definitions (`rg -o` takes the leftmost match) and was corrected before any code. On an uncontended host the Foundry agent ran all 10,432 tasks in 228 s, and paired scoring of all three sets took 18 s. Baseline (current release binary): checker 6,881 tasks, Foundry pass@1 35.6%, pass≤4 45.1%, 2,056 tokens per task; grep pass≤4 52.6%, 7,491 tokens. Qualified 2,551 tasks: Foundry 35.0% / 40.8% / 2,313; grep 47.4% / 7,228. Bun 999 scored tasks: Foundry 61.3% / 61.6% / 1,718; grep 96.9% / 836. Per-group baselines and the targets are in 001 § G1.
- **G2 question set** (009 T004; outside the repository; frozen 2026-10-08 03:03Z before any T004 code). 388 identifier-free questions (197 over rust-lang/rust `library/`, answers SCIP definitions; 191 over oh-my-pi, answers top-level declarations or direct methods of top-level classes), written from code by Anthropic Opus agents without running any retrieval. A mechanical verifier checks offsets and scope and rejects answers inside macro invocations, 5-word overlaps with the answer's file, the answer's string literals, inflected name pieces and meaningful 3-letter pieces (46-case self-test). Cross-lab refutation by OpenAI GPT-6 Astra took three rounds: REFUTED (ambiguous answers, a macro-generated answer, copied comment text, misdescribed behavior), REPAIR (10 wordings, one scope clause), ACCEPT. The scoring harness (one `context` call per question, the frozen G1 evidence parser, fail-closed integrity, exact one-sided McNemar divided by the number of candidate profiles) was frozen with it; repeated runs were identical and a positive control passed 6 of 6. No G2 measurement has been run.

## Embedding model comparison: Nemotron 3 Embed 1B vs EmbeddingGemma 2 — 2026-10-07

Owner-authorized exception to the measurement freeze, before signing; nothing in the repository changed except this record and the 009 status note. Decision rule (owner): switch only if EmbeddingGemma 2 delivers at least as much required evidence. **Result: rule not met; Nemotron stays.**
- **Runtime.** No released MLX package supports EmbeddingGemma 2 (mlx-vlm 0.7.6 has no `embedding_gemma2`; support is an open pull request), so it ran through llama.cpp at build `b9acf138` (includes EmbeddingGemma 2 support, PR #30054), statically linked with Metal, using the ggml-org GGUF `bfcd2987` (BF16, converted from `google/embeddinggemma-2@914f7f89`, Apache-2.0). Parity against the reference sentence-transformers 6.1.0 float32 implementation: cosine ≥ 0.99999 on 20 queries and 40 code windows (Q8_0: ≥ 0.99982).
- **Evaluation-only harness** (outside the repository, not product code). Foundry's embedding path is fixed at 2048 dimensions and the Nemotron `passage: `/`query: ` recipe, so a llama.cpp worker speaks Foundry's unchanged worker protocol inside the same App Sandbox bundle machinery. It decodes Foundry's token IDs, re-renders them with Gemma's prompts (`task: code retrieval | query: `, `title: none | text: `), keeps the first 768 or 256 dimensions, renormalizes and zero-pads to 2048. Zero padding leaves cosine similarity unchanged. Both arms embed the identical 15,026 units because Foundry partitions with the Nemotron tokenizer in both; decoding round-trips exactly on 200 of 200 sampled code windows.
- **Protocol.** Library-only store (the 2,255 files of rust-lang/rust 1.99.0 `library/`, where every checker answer lives) with the 005 SCIP graph; the frozen 4,597 development and 2,284 held-out tasks (checker v3 `e190b7dd…f6c8`, pass logic unchanged); `context` at 2048 tokens, `search` and `graph`, through `foundry mcp --semantic-profile` (every response `semantic:ready`).

| Required evidence delivered | dev search | dev graph | held-out search | held-out graph |
| --- | ---: | ---: | ---: | ---: |
| Lexical only | 49.3% | 22.2% | 44.1% | 19.5% |
| Nemotron (MLX 4-bit, 2048-d) | 48.3% | 22.8% | 40.9% | 20.0% |
| EmbeddingGemma 2, 768-d | 48.0% | 22.4% | 41.5% | 19.6% |
| EmbeddingGemma 2, 256-d | 47.8% | 22.3% | 41.2% | 19.5% |

- **Paired comparisons** (tasks gained / lost, exact McNemar):
  - Gemma 768 vs Nemotron: search dev 70 / 83 (p = 0.33) and held-out 45 / 30 (p = 0.11), so within noise; graph dev 17 / 35 (p = 0.018) and held-out 6 / 16 (p = 0.052), so Gemma is worse. Gemma 256 is worse than Nemotron on graph (dev p = 0.001, held-out p = 0.043).
  - Both models vs lexical only: search loses significantly (Nemotron dev 89 / 134, held-out 28 / 101; Gemma 768 dev 58 / 116, held-out 17 / 75; all p ≤ 0.003). Nemotron gains on graph for dev (51 / 21, p = 0.001).
  - The loss is in usage questions (dev search: lexical 33.3%, Nemotron 30.7%, Gemma 29.9%; held-out 35.3%, 27.5%, 28.6%): fused dense candidates displace reference-site units. Definition questions are unchanged (58.2%, 58.1%, 58.1%).
  - These tasks name an identifier; the set has no natural-language query without one, which is the case dense retrieval exists for. Delivered tokens were equal across arms (mean 1,807–1,812 for search).
- **Cost** (same machine; MLX 4-bit vs llama.cpp BF16, so runtime and model both differ):
  - Full preparation of the 15,026 units: Nemotron 6,889 s in three bounded rounds; Gemma 768 2,139 s in one, alone (3.2× faster). Gemma 256 took 2,098 s while two checker runs shared the machine.
  - Worker peak footprint: Gemma about 1.62 GiB (eight 120-line windows, 4,096-token context); Nemotron about 1.6 GiB at a 2,048-token input (009 D001).
  - Weights: 629 MB (Nemotron 4-bit artifact) vs 532 MB (Gemma BF16 GGUF).
  - Index and cache size are computed, not measured, because the harness pads every vector to 2048: an F16 index of 15,026 vectors would be 61.5 MB at 2048-d, 23.1 MB at 768-d and 7.7 MB at 256-d.
- **What a switch would have needed.** Profile-owned dimensions and prompts (today `DIMENSIONS`, the prefixes, the cache row layout and the frame size are constants), a llama.cpp worker in place of pyo3 and MLX, and a new signed bundle. The harness worker shows the llama.cpp path works under the existing protocol and sandbox, and it is the template if a future model without MLX support wins.

## Learned-router economics and tier-1 ranking — 2026-10-06

Owner-approved after the 013 corpus analysis: (1) account for the learned router by economics, (2) fix the deterministic retrieval causes first, (3) decide a second learned decision from the rerun. Offline arithmetic and deterministic `context` runs only: no model call, no timing claim.
- **Router counterfactual** (recorded round-1/round-2 evaluation cases joined with each row's task-checker evidence; no new runs). Round 1, 71 held-out rows: deterministic routing and the served router both delivered the required evidence on 64 rows (143,142 vs 143,136 delivered tokens); the label would deliver 71. The router changed one route, on a row where both strategies passed. All 7 deterministic misses chose `search` where `graph` was needed (mostly "what would break if `X` changed"); the model chose `search` on all 7. Round 2 (101 rows): the same single change, 92 both ways. Label accuracy rose 59 → 60/71 without any evidence gain.
- **Enablement gate** (013 contract amendment, `24abb11`). Evaluation reports `economics` (evidence and delivered tokens for baseline, routed and label; changed, gained and lost routes). `learning select` and owner startup require `gained > lost` (`economics_unknown`, `candidate_no_benefit`); token-only savings never enable. `learning select --lifecycle-check` is the visible, status-marked override for T004/D001 package acceptance only. Round 1 would be refused.
- **Tier-1 cause and fix** (context-v2 amendment, `c3437e6`). Tier 1 matched every identifier run of a query, English words included (`find`, `is`, `of`), kept a key_hash-arbitrary 64 and sorted them by path: `find \`sleep_ms\`` returned `fn find` definitions and never `fn sleep_ms`; `callers of \`sleep_ms\`` seeded graph expansion from `fn of`. Tier 1 now uses backtick-marked runs when present, ranks runs by exact filtered definition count and fills its 64 slots per run. Also accepted: the MCP `lines` array form and an empty-selection refusal naming the handle's lines. tools/list measures 995 of the 1000-token ceiling.
- **Rerun protocol.** `econ_check.py` (SHA-256 `fafba7ee…ac2ac2`, frozen 19:35Z before any fix) regenerates the 4,597 labeling tasks exactly and adds 2,284 held-out tasks in eight wordings never used for development (four marked, four unmarked). `context` at 2048 tokens, `search` and `graph`, on a schema-6 copy of the rust-lang/rust 1.99.0 store. The baseline binary (`77bf120`) reproduced every labeling-run pass flag and search token count.

| Required evidence delivered | search | graph | either | `auto` |
| --- | ---: | ---: | ---: | ---: |
| Dev 4,597, before | 22.2% | 13.0% | 23.7% | 22.2% |
| Dev 4,597, after | 42.1% | 23.1% | 44.1% | 40.6% |
| Held-out 2,284, before | 30.1% | 16.2% | 32.2% | 30.1% |
| Held-out 2,284, after | 34.3% | 17.6% | 36.5% | 34.5% |

- **Per task** (search / graph): dev gained 930 / 566 and lost 16 / 99; held-out gained 155 / 101 and lost 59 / 70. Paired dev wordings now match: "find `X`" 12.5% → 53.2%, "where is `X` defined" 42.7% → 53.0%.
- **Route keywords withdrawn.** Adding `uses`, `used`, `break`, `breaks` and `referenced` lowered `auto` evidence after the fix (dev 40.6% → 38.0%, held-out 34.5% → 34.2%): graph context places path-seeded neighbors, not the queried symbol's references, and delivered less than search for every usage wording except held-out unmarked "callers of X". The shipped list is unchanged.
- **Residual causes.**
  - Ambiguous names: definitions whose name has more than 64 library definitions pass 7% (dev) and 2% (held-out); unique names pass 77% and 71%. The query does not say which definition is meant.
  - Unmarked queries rank English words by the same specificity rule: held-out "where is X defined" fell 43.6% → 38.6%, while marked wordings rose.
  - Usage questions: at most 37.8% even for the best wording; `context` is not a reference listing.
  - Tokens: every response still fills the budget (mean 2,014–2,019 of 2,048).
- **Compact sufficiency** (`search` at 512 tokens, same tasks, fix binary). 56% of dev and 58% of held-out definition passes at 2048 also pass at 512 (46% / 38% for usage), at about 480 instead of 2,015 delivered tokens; three tasks passed only at 512. A compact decision saving about 1,530 delivered tokens must be weighed against a miss costing about one agent turn (12.4–13k tokens per OMP request, 003 T005 root cause).
- **013 note.** Composed route states (query, coverage, top-3 locator lines) change under the new tier-1 order; states composed before `c3437e6` do not match serving composition, so a next round recomposes and relabels.
- **Gates** (`24abb11` with the keyword withdrawal): fmt; clippy `-D warnings` on stable and 1.90; full suite 871 passed, 0 failed, 8 ignored.
- **Review.** Anthropic Opus 5.5 wrote both slices; OpenAI GPT-6.1 Sol reviewed them: tier-1/lines SHIP; economics gate REVISE (1 major: the lifecycle mark was lost on failed startup and inspection), then SHIP.
- **Compact context** (owner chose the fixed rule over a learned decision; `844796c`). When every backticked identifier with a definition has exactly one, `context` delivers that definition plus at most 8 one-line pointers and marks the header `compact`; other responses are byte-identical. Same frozen tasks (checker v2 `038182f6…53adb4` adds body/pointer and header fields; pass logic unchanged), compared per task with the pre-compact run:
  - Applied to 14.3% of dev tasks and 7.0% of held-out tasks (half the held-out wordings are unmarked and never compact).
  - On those rows: mean delivered tokens 2,012 → 824 (search); required evidence dev 458 → 458 (search) and 401 → 399 (graph), held-out 119 → 114 and 100 → 97. The held-out losses are usage tasks whose third reference site lay beyond the 8 pointers. Definitions arrive as bodies in 317 of 324 dev passes.
  - All tasks: delivered tokens −8.5% (dev) and −4.1% (held-out); evidence 42.1% → 42.1% (dev search), 34.3% → 34.1% (held-out search).
  - Arithmetic, not a host measurement: on the held-out compact rows, 5 extra misses at about one turn each (~62k tokens) against 160 × ~1,190 fewer delivered tokens (~190k) on the first delivery alone.
  - Remaining room (non-compact tasks; `search` at 512 vs 2048 on the pre-compact binary, whose non-compact output is byte-identical; arithmetic on recorded runs, one turn taken as 12.7k tokens, replay factor = later turns that re-send a response):
    - Dev 3,938 / held-out 2,124 tasks: short enough 16.5% / 15.5%; needs the full budget 21.0% / 15.8%; fails at both sizes 62.4% / 68.7%. Shortening saves about 1,535 delivered tokens per response.
    - Net delivered tokens per task (dev; held-out): shorten when the name has more than 64 definitions +178 / +910 / +1,641 at replay ×1 / ×3 / ×5 (+282 / +978 / +1,675); more than 16 definitions +55 / +1,267 / +2,479 (+325 / +1,477 / +2,629); always 512 −1,128 / +1,943 / +5,014 (−475 / +2,594 / +5,662), break-even at replay 1.73 (1.31); a perfect decision +1,213 / +3,640 / +6,067 (+1,290 / +3,871 / +6,452). Definition counts here are the library SCIP counts, a proxy for Foundry's tier-1 counts.
    - The needs-full tasks are mostly definitions (dev 652 of 826; held-out 264 of 336) of names with 2–16 definitions (69% / 78%), delivered at positions 1–9 (82% / 87%) among same-named definitions, which tier 1 orders by path. The query does not say which one is meant, and a learned decision sees only the query, coverage and top-3 locator lines, so most of the gap to a perfect decision is not in its input.
    - Not measured: `graph` at 512 and the replay factor of real host sessions (host compaction drops old tool results).
  - Gates (`844796c`): fmt; clippy `-D warnings` on stable and 1.90; full suite 877 passed, 0 failed, 8 ignored. Review: Anthropic Opus 5.5 wrote it; OpenAI GPT-6.1 Sol reviewed it: SHIP.

## 009 runtime fixes after measurement — 2026-10-06

Implementation only; measured in the end-of-implementation batch (owner, 2026-10-06).
- **Why.** The 2026-10-06 measurement found two problems. During continuous preparation every query fell back with `provider_busy`. On the 60,739-file corpus, every request in the first ~12 minutes failed with engine `busy`, because back-to-back driver store steps held the engine slot.
- **Shape** (spec 009 T003 decisions D5–D8):
  - Batches of at most 2 inputs while foreground activity happened in the last 60 s, otherwise 8.
  - One query may wait for this owner's document batch, up to its own ceiling.
  - Requests wait for a driver store step, bounded by their deadline. A single FREE/DRIVER/REQUEST holder mark decides who waits.
  - Time-bounded partition steps.
  - Generations are built and staged without the engine slot.
  - Status names a dead worker (`fallback:provider_exited`) instead of `ready`.
  - Dead owners' scratch is reclaimed at launch under the launch control. It is first claimed into a non-run `.reclaim-*` quarantine and deleted only if it is the validated inode.
  - The readiness wait never enters a receive once the deadline has passed.
  - The `embed_worker` and `learning_worker` load-timing flakes were fixed at the root cause.
- **Review.** Anthropic Opus 5.5 wrote it; OpenAI GPT-6.1 Sol reviewed it: REVISE (6 major, 2 minor), then REVISE (3), then SHIP. Each fix has a barrier or fault-point test.
- **Gates** (`b1466ca`, all slices integrated): fmt; clippy `-D warnings` on stable and 1.90; full suite 858 passed, 0 failed, 8 ignored, no flake.

## Installable package — 2026-10-06

Unsigned (ad-hoc-signed development bundles). Signing is the last step, by owner decision (2026-10-06).
- **Shape.**
  - `scripts/package.sh` builds `context-foundry-<version>-macos-arm64.tar.gz`: the core, optional `foundry-embed`/`foundry-learn` with their bundle scripts, README, LICENSE, THIRD-PARTY (331 packages) and `PACKAGE.json` (every file's SHA-256 plus the dependency identities). No weights, datasets, profiles or credentials.
  - `scripts/install.sh` supports `install`, `upgrade`, `rollback`, `disable-semantic`, `disable-learning` and `uninstall`.
  - Versions install side by side, selected by an atomically switched `current` link.
  - Worker bundles are built at install time from the operator's profiles, which are written back as installed copies.
  - One lock and one recorded intent (`pending.json`) make every interrupted command recoverable: it completes or rolls back from recorded hashes only.
  - Refusals happen before any change: running owners, symlinked layout, a foreign launcher, non-ASCII or control-character paths.
  - Uninstall removes only owned files that still match their hashes, plus the `connect` host-config block (byte-exact restore through `connect --remove-config`).
  - `bootstrap` reports a `versions` object.
- **Review.** Anthropic Opus 5.5 implemented it; OpenAI GPT-6.1 Sol reviewed it over four rounds (REVISE ×3, then SHIP). The findings fixed were:
  - deletion through replaced symlinks;
  - pruning of unowned empty directories;
  - overwriting an operator launcher;
  - unrecoverable interruptions;
  - ownership recovered by rehashing;
  - glob-based temp cleanup;
  - a symlinked lock;
  - busy-check gaps;
  - marker substrings in host configs;
  - recovery bypassing the busy gate;
  - post-commit rollback;
  - unproven host cleanup;
  - foreign `current` links.
  
  The remaining limits are documented in deployment.md: a check-to-delete TOCTOU window, an owner starting after the final busy check, and recovery's own temporaries.
- **Tests.** `tests/install.rs` (5): package contents and manifest; install, run, upgrade, rollback and uninstall with user data kept; busy refusal; worker bundles with fake workers; byte-exact host-config removal; interrupted-command recovery at every recorded point; refusal cases. Since 2026-10-08 the host-config removal test is in `tests/mcp.rs`, and `tests/install.rs` builds only on macOS arm64, the package target.
- **Real smoke** (release build with both real workers):
  - Install builds both ad-hoc bundles. Semantic context is `ready` through the installed Nemotron bundle; context is `route:policy` through the installed learning bundle.
  - `connect --apply-config` works.
  - Upgrade to a relabeled package rebuilds both bundles and serves `ready`.
  - Rollback and `disable-learning` work.
  - Uninstall leaves the store byte-identical, the host config byte-exact, the supplied profiles intact and nothing under the prefix.

## Measurement phase — 2026-10-06

The owner deferred every measurement until the implementable spec tasks were done (2026-10-05). All runs used release builds and the development bundles; nothing ran under production isolation. Evidence: [009 T003 lifecycle](review/009-t003-lifecycle-2026-10-06.json), [013 rounds](review/013-measurement-2026-10-06.json).

- **009 T003 real lifecycle** (Nemotron 4-bit, production `foundry mcp`, this repository as the corpus: 193 sources, 2,099 units). It passed its predeclared bounds in 872 s:
  - cold preparation took about 845 s (about 2.5 units/s);
  - the query during preparation got baseline results with `fallback:provider_busy`, because continuous preparation keeps the one model slot nearly always busy;
  - steady queries took 1.25 s for the first (index load), then 184–215 ms;
  - after an edit, baseline `partial` came back in 195 ms; catch-up embedded exactly 1 unit; restart embedded 0.
- **013 rows.** 1,088 labeled tasks were composed into states by the core in 52 s.
  - 32 rows were recorded `allow_training:false`. 27 composed states recur across groups (the same symbol name in different modules yields the same query and locators), and one state carried conflicting labels. The contract refuses both as `duplicate_conflict`.
  - Rounds follow the owner's split: 770 rows in round 1, 318 in round 2.
- **Defect found and fixed.** The first sandboxed training run failed: App Sandbox moves the worker's working directory into its container, so the relative `head.safetensors` was written there.
  - The supervisor now passes `--run-dir`; the worker re-enters it and confirms it by device and inode.
  - A regression test starts the worker elsewhere.
  - Fake-worker tests could not see this; it is the reason the sandboxed run is in this phase.
- **013 round 1** (555 train, 124 calibration, 71 evaluation rows): 569 steps in 195 s, worker peak RSS 2.3 GB, temperature 3.0 (the grid's upper edge). Coverage 0.56 and accepted accuracy 0.90 (36/40), so it is eligible. Fallback-inclusive accuracy was 60/71 against 59/71 for deterministic routing, a one-row difference. Eligibility permits a trial; it is not an improvement claim.
- **013 round 2** (306 new plus 217 replay train rows; the evaluation reuses round 1's held-out groups): 434 steps in 200 s, 2.6 GB. Not eligible: accepted accuracy 0.8625 is below 0.9. It equals the incumbent (84/101) against 83/101 for baseline. This is a valid rejected candidate; the incumbent stays.
- **013 lifecycle (T004 verification items):**
  - represented input → `no_new_data`;
  - withdrawing an inherited contributor → `base_permission_changed` at prepare and `contribution_changed` before any update at train;
  - owner SIGKILL during training → worker gone in 41 ms, no candidate, incumbent unchanged;
  - select writes a config for round 1 and refuses round 2 (`candidate_ineligible`) and any overwrite;
  - rollback is restarting without the config.
  - Limitation: a killed owner's scratch run directory (100 MB staged base head) is not reclaimed by later runs.
- **013 serving.**
  - Resident MCP owner with the selected candidate: start 13.2 s, first routed call 1.3 s, steady 265–435 ms, against 66–160 ms without a policy.
  - A CLI call with `--policy-config` pays the model load each time (about 8.6–9 s).
  - The learned router stays off by default. The owner declined the paid enablement comparison.
- **013 numerics.** The exhaustive comparison holds for every element of the full gradients (34- and 1024-token cases) and of the post-sequence parameters, within atol 1e-5 / rtol 1e-4. The signed-bundle negative probes pass.
- **Not measured:** packaged install/upgrade/disable/uninstall and the distributed profile (013 T004, 009/013 package acceptance) need signing. The aggregate 009+013 residency was not run as a joint test; observed peaks were about 2.6 GB for the training worker, and the 009 worker's ceiling is 3 GiB.

## 013 T003 serve, select, roll back and retire the legacy HTTP path — 2026-10-05

Development isolation only. Deferred to the measurement phase (owner, 2026-10-05), not run: the enablement comparison (the same checked tasks with `foundry usage import`), the aggregate 009+013 residency test, and latency figures. The learned policy stays off by default. (Superseded 2026-10-06: enablement now requires the economics gate above.)
- **Shape.**
  - `src/policy.rs`: config v2 validated at owner startup. An invalid config is `policy_config_invalid`, with baseline retrieval intact.
  - `learning select`: validates the candidate and writes a new config by no-replace publication. Rollback is starting with the prior config or none.
  - Serving: `foundry-learn` loads the candidate and answers contract IPC-v2 requests over the shared frame. The core validates the reply and thresholds the unrounded maximum probability.
  - Slot: one prediction, no waiting. The wait is min(2 s, half the remaining read deadline); below 50 ms the model is skipped. A request is on time only if its reply is published to the core before the ceiling. Three consecutive timeouts, or any malformed, wrong-identity or dead reply, terminate the worker and make the policy unavailable.
  - Routing: only for `auto` with a current graph, after the 009 merge and before graph expansion. The `route:` header segment (context-v2 amendment) appears only when a policy is configured; otherwise output is byte-identical.
  - `src/laya.rs` and its HTTP client are deleted. Legacy feedback/export is unchanged. `--laya-port` is refused with migration guidance.
- **Review.** Anthropic Opus 5.5 implemented it; OpenAI GPT-6.1 Sol reviewed it. Round 1, REVISE (3 major): a late reply granting a route and resetting the count; a policy timeout consuming the whole read deadline; evaluation rows' rights not checked at selection. Round 2, REVISE: an on-time reply could still be counted as a timeout. That was resolved by one publication-based linearization point per request, a captain decision recorded in the contract. Round 3, SHIP. Real prediction parity against T002's evaluation probabilities: 21 rows, bit-exact (release build).
- **Gates** (`5585a6e`): fmt; clippy `-D warnings` on stable and 1.90 (default, `--no-default-features`, `learning-worker`); full suite 820 passed, 0 failed, 8 ignored.

## 013 T002 tch training worker, calibration, evaluation and candidates — 2026-10-05

Development isolation only. Deferred to the measurement phase (owner, 2026-10-05), not run: training and evaluation on the real labeled rust-lang/rust rows, the exhaustive elementwise comparison against `reference-t002-full.safetensors`, and the signed-bundle negative probes. They are ignored tests and documented commands in [learning](learning.md).
- **Shape.**
  - Optional `learning-worker` feature (tch 0.24.0 / LibTorch 2.11.0), outside the default build. `build.rs` sets the rpath and works around LibTorch's libomp install name.
  - The `foundry-learn` worker is a numerical engine only. The core owns read-back, the pre-fit permission gate under exclusive ownership, seeded order, `max_steps`, the clock, calibration, evaluation, candidate validation and publication.
  - Worker IPC rides on the shared frame format with a request ID and the loaded identity. A duplicate, stale or late reply is terminal.
  - Trainable: the head, the scorer and type-embedding row 0 only.
  - AdamW with clip 1.0. Temperature grid 0.5–3.0. 15-bin ECE. Query-only deterministic routing as the comparator.
  - Candidates: float32 head plus manifest, published through anchored no-replace publication. Inherited per-option temperatures and a fitted scalar below 0.5 are refused.
  - Enforcement matrix: hard process count, output and CPU; supervised memory, failing closed.
- **Numerical parity** with a reference generated from unchanged Laya @4066d5d5 (transformers 5.18.0, torch 2.14.1). It covers 5 cases (34 to 1024 tokens) and a 3-step AdamW sequence with two clipped steps:
  - logits and losses exact;
  - gradients ≤1.9e-13 absolute;
  - post-sequence parameters ≤2.5e-10;
  - post-sequence logits ≤2.4e-7.
  
  The tolerance is atol 1e-5 / rtol 1e-4.
- **Design and review.** The captain's brief was refuted cross-lab by GPT-6 Astra (refuter-b) before implementation. Findings folded in: the pre-fit consent gate; type row 0 only; query-only comparator; IPC identity; full validation before adopt; base versus candidate temperatures; fail-closed measurement; real ENOSPC paths; no encoder cache; a clipped reference step. Anthropic Opus 5.5 implemented it. OpenAI GPT-6.1 Sol reviewed: REVISE (4 major: duplicate final reply, breach after the last reply, reply crossing a deadline, manifest outside the output total), then SHIP.
- **Gates** (`390c282`, rebased on 009 T003): fmt; clippy `-D warnings` on stable and 1.90, default and with `learning-worker`; full suite 796 passed. The only failure was the known `embed_worker` launch-timing flake, which passed alone.

## 009 T003 progressive preparation in the MCP owner — 2026-10-05

Development isolation only. The real lifecycle exercise is deferred to the measurement phase (owner, 2026-10-05) as an ignored test (`real_lifecycle_exercise_on_a_permitted_declared_corpus`).
- **Shape:**
  - The resident `QueryRuntime` is the one model worker for query embeddings and document batches of up to 8, behind one slot. Admission claims the slot, then checks for an abandoned late call; a query being dispatched wins over a document admission. A refused document admission pauses preparation with `provider_busy`. Nothing is queued.
  - The shared preparation steps in prepare.rs serve both the CLI and the MCP driver.
  - The driver takes the engine slot only to select and to commit. Inference holds no slot and no transaction.
  - `index {semantic: prepare|pause}` controls preparation; `status` reports a `semantic` object. The catalog is 986 o200k tokens, within the 1000 limit.
  - Shutdown joins the provider, so the worker is reaped before the store is released.
- **Authors and review.** Anthropic Opus 5.5 wrote it; cross-lab review by OpenAI GPT-6.1 Sol. The review went REVISE (5 major: hidden late-query occupancy, commit after EOF, ownership released before the worker was gone, an admission race, pause racing admission), then REVISE (the active-to-abandoned transition), then SHIP. Each fix has a deterministic barrier test.
- **Gates** (`75cd840`): fmt; clippy `-D warnings` on stable and 1.90 (default, `--no-default-features` and `embed-worker`); full suite 742 passed, 0 failed, 7 ignored.

## 009 T002 semantic context within the existing budget — 2026-10-05

Development isolation only. [Evidence](review/009-t002-2026-10-05.json).
- **Shape.**
  - D001 merge: exact definitions first, then RRF k=60 over the lexical top 256 and dense top 64. Each arm votes once per delivery unit.
  - Stale lexical documents are dropped in the final read before fusion.
  - Dense-only hits never seed graph or compiler expansion.
  - The `semantic:<word>` header segment and the whole/lexical-span/preview forms ladder; neural batches deduplicate by full identity.
  - Generation format v2: the label map carries unit locations plus `source_revision` and `coverage`, so no request walks partitions. A v1 generation is unavailable by name and republished from the cache with zero document calls.
  - The index bytes restored are the bytes that were hash-verified.
  - `ready` only when the generation is complete at the response's own final-read revision.
  - The query embedding waits min(1500 ms, half the remaining deadline). A failed footprint measurement of a live worker stops it like a breach.
  - MCP starts one resident worker (`mcp --semantic-profile FILE --development-isolation`); the CLI takes the same flags.
- **Authors and review.** Z.ai GLM-5.3 wrote the first version; after the owner retired GLM, Anthropic Opus 5.5 completed it and wrote both fix rounds. Cross-lab review by OpenAI GPT-6.1 Sol: REVISE (8 major), then REVISE (one residual coverage case), then SHIP. Each fix has a dedicated test.
- **Real model** (Nemotron-3-Embed-1B 4-bit, production development bundle):
  - All three frozen vocabulary-gap spans are delivered at 2048 tokens (2,023 / 2,023 / 1,947 tokens); the lexical baseline delivers none of them.
  - No-profile output is byte-identical.
  - Preparation: 8 units, 1 document call, 2,969 input tokens. A one-file edit embeds 1 unit (140 tokens) and reuses 7.
  - After a delete without re-preparation: `stale:1`, `semantic:partial`, and the deleted source is never served.
  - A profile mismatch falls back by name.
  - Resident MCP: worker load 10.6 s, first query 314 ms, warm queries 24–32 ms.
  - The index is 34,032 bytes.
- **Gates** (`451033b`): fmt; clippy `-D warnings` stable and 1.90, default, `--no-default-features` and `embed-worker`; full suite 720 passed. The only failure is `embed_worker::a_ready_crossing_the_deadline_inside_one_receive_slice_is_never_accepted`, a 400 ms launch-timing test that already flakes on main (1 in 6 solo binary runs; the worker's PID file is not yet written at the deadline). It passes alone.
- **Known gaps, closed 2026-10-06** (`121eba2`). The owner had accepted them on 2026-10-05; each now has a behavior test that GPT-6.1 Sol reviewed (REVISE → SHIP). No product defect was found.
  - Semantic evidence crowded by graph/compiler evidence; dense-only hits seed no expansion.
  - Multi-root:
    - the wording says primary root only;
    - a secondary-only request spends no embedding and returns the plain bytes;
    - no secondary item leaks.
  - The no-profile byte-identity matrix with positive hits: CLI and MCP × search and context × single and multi root.
  - A worker dying mid-query: named `provider_exited`, then a stable named fallback.
  - A localized lexical-span preview continues to the span's end.
  - CRLF, multibyte and fence-like bytes stay exact in every semantic form.
  - The MCP byte cap. It is measured at the packer with the owner's exact measure; end to end it takes 1.3 s in release, against 10 s in debug.
  - Exact session-allowance charging, including a refused request charged 0.

## 013 T001 exact permitted ModernBERT examples — 2026-10-05

Locally implemented and verified; no model is loaded and nothing is trained.
- **Shape.**
  - Store schema 6 adds `learning_feedback`, `learning_history` and `learning_datasets`. The explicit `upgrade-store --to 6` runs from v1–v5 in one transaction; every earlier table is preserved, the 009 vector cache included.
  - `feedback v4` is operator-only. MCP offers no feedback or consent path.
  - `learning prepare|check|compose-state`. `check` takes `--policy`, because the policy pins the tokenizer the exact re-render needs (contract amended).
  - The renderer (`src/decision_model.rs`) matches unchanged Laya `build_sequence` @4066d5d5 on 9 pinned cases, including the 1024/1025 boundary.
- **Authors and review.** Z.ai GLM-5.3 wrote the first version. After the owner retired GLM for erroring, Anthropic Opus 5.5 wrote both fix rounds. Cross-lab review by OpenAI GPT-6.1 Sol: REVISE (10 major, 2 minor), REVISE (one new major), then SHIP. Findings fixed:
  - inherited contributions forgotten across rounds, now cumulative coverage plus a recorded lineage table;
  - a rights-assertion change not detected;
  - a split conflict hidden by the no-op path;
  - a hand-written JSON scanner that could loop, replaced by strict serde parsing;
  - reads before bounds checks;
  - pathname-only output containment, now judged through descriptors;
  - replace-on-publish;
  - forged token arrays accepted on read-back;
  - group ownership not checked;
  - empty bases accepted;
  - publication of a substituted staging source, now staged inside a held private directory with source and destination identity checks.
  
  A lost publication response is verified with a full read-back and adopted.
- **Gates** (`9e1c6bc`): fmt; clippy `-D warnings`, stable and 1.90, default and `--no-default-features`; the full suite, 679 passed / 0 failed / 6 ignored, with the recovery harness passing. `learning_data` has 67 tests.
- **Not yet done:** feedback v4 rows from the accepted labeling run (4,597 tasks, 1,088 labeled) are built with `compose-state` against the refreshed corpus store when T002 needs them.

## 009 T001 semantic preparation and supervised worker — 2026-10-05

Locally implemented and verified, **pushed 2026-10-05, unreleased**. Development isolation
only; normal admission stays `isolation_unavailable` until signing/notarization and
package acceptance. [Evidence](review/009-t001-2026-10-05.json).
- **Authors and shape.** Z.ai GLM-5.3 wrote two slices; the captain wrote the shared
  boundary (`provider`, `protocol`, `profile`). Slice A covers partition, the schema-5
  tables, cache, USearch generations, bounded prepare, status and purge. Slice B
  covers the worker runtime, the `foundry-embed` worker, the supervisor and the bundle
  script.
- **Review:** cross-lab by OpenAI GPT-6.1 Sol, four rounds per slice.
  - Slice A: BLOCK ×3, then SHIP. Findings fixed:
    - symlink and ancestor substitution in purge and publication, now anchored to
      a store descriptor held for the Engine's life;
    - the run deadline not reaching the provider or artifact verification;
    - duplicate-input generations;
    - partial runs never publishing, now under a publication reserve;
    - mapping ranges and exact-input key identity;
    - whitespace direction, and a depth cutoff now replaced by a work stack;
    - durable progress counters;
    - retained-profile and nonfinite cache classification;
    - status deadlines.
  - Slice B: BLOCK ×2, REVISE, then SHIP. Findings fixed:
    - an unexplained owner-death miss, now behind a pressure gate;
    - a lost exit event during watcher arming;
    - the worker's descriptor and inventory not being validated;
    - acquisition not bounded by the caller's deadline;
    - late replies and retries after the deadline;
    - site hooks running before isolation, now an isolated `PyConfig`;
    - a vacuous symlink probe;
    - scratch-root binding and overlap with read-only grants, aliases included;
    - timeout overflow.

  The captain recorded three decisions in spec 009: publication shares the budget,
  status trusts committed payloads, and destructive filesystem work is
  descriptor-anchored.
- **Gates** on `te-009` over main `59dcfa0`:
  - fmt, clippy `-D warnings` and the Rust 1.90.0 check and clippy pass, including the
    `embed-worker` and `--no-default-features` builds;
  - full suite **598 passed, 0 failed, 6 ignored**: five `--ignored` real-model
    development gates plus the existing economics test;
  - `tests/neural.rs` 51, `tests/embed_worker.rs` 46 (+5).
- **Real model.** A release `foundry` drove the production worker bundle (`3cb10e27…`,
  no test hooks) with the pinned Nemotron 4-bit profile (function digest
  `5e0e9cb8…`):
  - first preparation: 5 sources → 17 units, 3 document calls, 17 cached f32 vectors,
    an F16 generation of 17 entries, 15.8 s, 657 MiB peak (CLI process);
  - unchanged restart: 0 calls, 17 reused;
  - one-file edit: the old mapping was rejected immediately; re-preparation made 1 call
    and reused 16;
  - `repair-index`: rebuilt the semantic index from the cache with 0 calls;
  - purge: removed partitions, cache (including the orphan) and the generation, while
    lexical search kept working;
  - without `--development-isolation`: `isolation_unavailable`.
- **Development worker runs.**
  - Parity: the direct-ID path equals publisher `encode` (cosine 1.0000000, max
    difference 0) for batch 1 and 8, reordered and heterogeneous batches, and exact
    1024- and 2048-token inputs; 2049 is refused before any model call.
  - Owner death: across six pressure scenarios (cold and warm, CPU and GPU), the worker
    was gone within 2 s in every one, worst 102.7 ms. One earlier unacknowledged run
    (pid 95942) exceeded 2 s; it was not reproduced, and its cause is unknown.
  - Sandbox: negative probes, each with an unsandboxed positive control.
- **Limits.**
  - A SIGSTOPped worker cannot exit itself; package acceptance needs an OS guardian.
  - Memory is supervised, not hard.
  - The ad-hoc signed bundle is not a distribution package.
  - Semantic search delivery and MCP preparation are T002 and T003.

## 005 T003 measured run — rust-lang/rust 1.99.0, 2026-10-05

One run, executed once, against the declared profile. Every acceptance criterion
passed, with no failed check (50 checks).
[Evidence](review/005-t003-run-2026-10-05.json);
[preselected questions and source check](review/005-t003-questions-2026-10-04.json).
- **Inputs.**
  - Corpus: rust-lang/rust 1.99.0 (`b940084d`), clean before the run. Foundry admitted
    60,739 sources: 75 named exclusions, about 179 MB by the admission rule.
  - Host: Apple M3 Pro, macOS 27.
  - Binary: release `eac5d97b…8775` from main `f7a7233`, with no test-hook strings.
  - Producer artifact: `db306662…0dc6`.
  - Questions: preselected and source-checked before timing (`4f564f2`).
  - Profile, fixed before the run: `max_index_seconds=900`, `max_peak_rss_bytes=2 GiB`,
    `max_query_p95_ms=250`, `max_run_seconds=3600`.
- **Foundry (measured).**
  - Index: 429.6 s, 556 MiB peak.
  - Import: 30.0 s, 469 MiB; `complete: true`, 0 failed, `coverage: partial` (external
    references unresolved).
  - Queries: the 20 timed MCP `references` calls (tokens 4096) had p95 105.3 ms
    (nearest rank, sorted sample 19) and a maximum of 132.8 ms.
  - Session: 861.3 s, with the timer paused during the producer rerun.
- **Producer (disclosed separately).** Jailed rust-analyzer 2026-08-31 over the
  filtered `library/` snapshot:
  - first run 1,128 s at 1.07 GiB (2026-10-04);
  - edit-cycle rerun 73.3 s at 1.10 GiB (a warm scratch target directory);
  - end to end 934.6 s.
- **Answers.**
  - The 15 supported questions returned exactly their expected references on page 1,
    with `coverage:complete` and no continuation.
  - The five high-degree questions returned a correct nonempty first page with
    `next: after=`. Their full pagination equals the expected sets: 2,633 records.
  - All 2,780 records match on exact byte ranges, read back through single-record
    pages.
  - Controls held: a deep position seed, a corrupted handle, a foreign workspace, an
    unknown symbol and a same-name different symbol.
- **Edit cycle through one MCP owner.** An appended comment in
  `library/std/src/os/unix/net/datagram.rs` was picked up by MCP `index`
  (revision 60739 → 60740), and q02 then answered `coverage:stale` with no items. The
  producer was rerun on an edited snapshot copy and `index {scip}` re-imported. The
  same q02 then answered `coverage:complete` with its expected set. There was no restart
  and no competing writer; a competing CLI import was refused `store_busy`.
- **Protocol checks.**
  - Traversal, path and `.` names are `invalid_argument`.
  - Missing and symlinked staged files are `artifact_unavailable`.
  - A 65 MiB manifest is `manifest_too_large` (untimed supplement).
  - A 2 s deadline mid-import returned `complete: false` with committed counts.
  - Staged caller files were untouched.
- **Limits of this result.**
  - The producer input is a filtered, incomplete `library/` snapshot (named omissions in
    spec 005).
  - The independent source check found one reference rust-analyzer did not emit (q19:
    `library/compiler-builtins/compiler-builtins/build.rs:594`); the expected sets follow
    the artifact.
  - SCIP identities stay as produced: the `core 0.0.0` workspace instance is not merged
    into the sysroot instance (counts are in the questions record).
  - This tests one named corpus, not universal capacity. It is not a token-savings or
    cost claim.

## 005 T003 agent surface — MCP `references`, `index {scip}`, graph context, 2026-10-05

Locally implemented and verified, **pushed 2026-10-05, unreleased**. The measured
large-workspace run is recorded separately.
- **Authors:** Z.ai GLM-5.3 (implementer). The captain made the contract rulings during
  review (spec 005, "Amended 2026-10-05").
- **Review:** cross-lab by OpenAI GPT-6.1 Sol in four rounds:
  - round 1, REVISE with 10 Majors:
    - context units lacked snapshot provenance;
    - staged files were checked and then reopened by path;
    - ambiguous symbols expanded;
    - compiler units bypassed the 32-unit bound, including the multi-root merge;
    - corruption in the final read failed source context;
    - seed ranges went unvalidated;
    - deduplication turned a used graph into `graph_unavailable`;
    - field validation ran after routing and admission;
    - multi-root `references` headers were missing;
    - the budget floor hint could be too low;
  - round 2, REVISE: a same-snapshot completion could make a witnessed definition
    ambiguous, and the final read did not reverify definition source chunks;
  - round 3, REVISE: the final pass did not charge witness membership reads to its
    allowance;
  - round 4, SHIP.

  Each round's fixes were mutation-checked: disabling a fix makes its regression test
  fail. Midway, the implementer accidentally overwrote `src/mcp.rs` and rebuilt it. The
  captain confirmed only 2 removed lines against the base, and the reviewer read the
  whole rebuilt diff.
- **Behaviour.**
  - `references` is the seventh MCP tool (catalog 977 o200k tokens; ceiling 1000). It
    validates fields before routing and admission, routes handles by `ws16` like
    `retrieve`, uses the root-aware header, and answers while the lexical index is
    `repair_required`.
  - `index {scip}` imports files staged in `<store>/imports`. It opens and freezes
    them by descriptor (no-follow), inside the engine slot, with the timeout and
    cancellation of `index`.
  - `context` with the graph strategy expands only uniquely resolved symbols. One
    32-unit bound covers lexical and compiler units in single- and multi-root
    responses. The final read re-proves each expansion's snapshot, witness rows,
    uniqueness and source bodies within a separate ≤256-record allowance.
- **Gates** (worktree on `3a81f8c`): fmt, clippy `-D warnings`, Rust 1.90.0 `check`
  and clippy pass; full suite **474 passed, 0 failed, 1 ignored**.
  `http_dropped_stream_keeps_slot_and_overload_keeps_control_traffic_serviceable` (an
  existing test) failed once under concurrent build load: its 300 ms client timeout
  dropped the request before it reached the server. It passed 3/3 in isolation.
- **Integration notes.** `src/graph.rs` changes are additions only: lines 1–1928 are
  byte-identical to `3a81f8c`. Every removed line elsewhere was audited: the
  index schema and allow-list, catalog assertions, graph-state branches, the multi-root
  merge, the references header prologue and the `scip.rs` freeze signature.

## 005 compiler references — T001 and T002, 2026-10-04

Locally implemented and verified, **pushed 2026-10-05, unreleased**.
- **Authors:** Z.ai GLM-5.3 (implementer); captain integration (the `fault.rs` point
  union and the `Cargo.lock` three-way merge with the gateway commit).
- **Review:** cross-lab by OpenAI GPT-6.1 Sol:
  - round 1, REVISE with 8 Majors: same-start pagination bypassed `limit`; the narrowest
    position rule was wrong; no import `coverage`; failed documents counted toward
    resolution; failed runs retired scopes; key parsing could underflow; FIFO inputs
    blocked; the occurrence cap ran after allocation;
  - round 2, REVISE: a reachable panic on a missing definition source; non-atomic
    retirement under cancellation; a budget cliff and masked staleness; key decode
    before the examined cap; outside-scope inputs degrading coverage; ignored SCIP
    metadata still materialized; an ordinal label treated as a path. The captain added
    M10: on the real artifact, position seeds deep in a file exhausted the scan because
    rust-analyzer's whole-file module occurrence defeated the early stop;
  - round 3, REVISE:
    - a stored seed-key range not checked against the verified source body;
    - an oversized document identified by its first rather than last path, which let a
      duplicate slip past preflight.

    In flight, the captain added two Majors, both closed by round 4: definition lookup
    could consume the record reserved for reference progress, and the new decoder
    accepted field number 0 and unterminated groups;
  - round 4, REVISE: the rewritten oversized-message scan accepted a document truncated
    at physical end of file;
  - round 5, SHIP: the truncation guard closed it, and the reviewer checked that no
    other early-EOF path accepts a truncated message.

  Round 2 found no remaining issue with the FIFO refusal or with failed documents
  counting toward resolution. Each later round's fixes were mutation-checked: disabling
  a fix makes its regression test fail.
- **Gates on the integrated main tree.**
  - Source manifest `8ba6fa5a…4a00`, the SHA-256 of sorted file hashes over `src`,
    `tests` (minus the frozen economics corpus), `Cargo.toml` and `Cargo.lock`.
  - `cargo fmt --check` and `cargo clippy --all-targets -D warnings` pass.
  - The full suite: **435 passed, 0 failed, 1 ignored**; all recovery scenarios passed.
  - Rust 1.90.0 `check` and `clippy -D warnings` pass.
- **`tests/semantic.rs` (83 tests).** These run on the T001 fixture artifact
  (`tests/fixtures/semantic/`) and on crafted SCIP:
  - `a::parse_record` versus `b::parse_record`, Unicode before an occurrence, an
    indirect use, and stale-to-fresh after a third-file edit and reimport;
  - frozen copies, FIFO refusal, hash mismatch before selection, duplicate
    documents and inputs;
  - every import limit at its real value, and the counting-allocator proofs that 4
    million ignored symbols or diagnostics are skipped without being materialized;
  - strict wire decoding (field 0, groups, wire types 6/7, overruns) at every
    nesting level;
  - atomic finalization under cancellation;
  - the narrowest-range and tie rules, deep seeds, pagination and budget reserves
    (a property loop over seed costs 250–255);
  - corrupted rows, short IDs and missing definition sources named without panics;
  - oversized documents identified by their last path, and truncated messages
    refused before selection.
- **Real artifact.** The 005 T003 producer run (spec 005 § Owner answers, T003 producer run;
  [questions record](review/005-t003-questions-2026-10-04.json)) produced a 56.9 MB
  rust-lang/rust `library/` artifact (958 documents, 436,829 occurrences). Imported into
  the whole-repository store (60,739 sources):
  - `complete: true`, 0 failed, `coverage: partial` (unresolved external references);
  - 22.5 s at 480 MiB peak RSS (round 2); with the final release binary
    `fd3c166b…6e3a` (no test-hook strings), 35.3 s at 465 MiB;
  - all 20 preselected questions match their expected sets on exact byte ranges
    (2,780 records), read back through single-record pages;
  - the controls hold: a deep position seed, a corrupted handle, a foreign workspace,
    an unknown ID, and a same-name symbol that differs.

  This is functional evidence, not the T003 measured run.
- **Compatibility.**
  - Store schema 4: `upgrade-store --to 4` from v1, v2 or v3 in one transaction; v3
    memory and typed pending keys are preserved.
  - Older readers refuse. Rollback means restoring a pre-upgrade store copy and the
    previous binary.
  - New dependencies `scip` =0.10.0 and `protobuf` =3.7.2 (Rust 1.90 verified).
- **Integration notes.** Every removed line was audited: 87 lines, all schema 3→4
  edits, visibility widenings and test assertions; nothing was removed from
  `graph.rs`, `scip.rs` or `response.rs`.

## 003 T004 owned model gateway — 2026-10-04

Locally implemented and verified, **pushed 2026-10-05, unreleased**.
- **Authors:** Z.ai GLM-5.3 (implementer), plus captain fixes: the flush-before-abort
  in the response body, test-only fixes, and a Rust 1.90 clippy rewrite.
- **Review:** cross-lab by OpenAI GPT-6.1 Sol in three rounds:
  - BLOCK: a credential-bearing SSE error event could pass, plus 9 Majors;
  - BLOCK: C1, M1 and M4 reopened by new counterexamples;
  - SHIP: the reviewer replayed the counterexamples against a fresh build.
- **Evidence:** [t004-gateway-2026-10-04.json](review/t004-gateway-2026-10-04.json).

- **Request pin.** OMP 18.6.0's request profile was pinned before code by running
  actual OMP (the `dist/cli.js` bundle, not the 18.1.11 TypeScript sources beside it)
  against a credential-free capture endpoint; see adapter-economics § Pinned OMP 18.6.0
  request profile. Sanitized fixtures are in `tests/fixtures/gateway/`.
- **Gates on the integrated main tree** (with 008):
  - source manifest `87aea7ff…07d3`;
  - `cargo fmt --check` and `cargo clippy --all-targets -D warnings` pass;
  - the full suite: **352 passed, 0 failed, 1 ignored**; all recovery scenarios passed;
  - Rust 1.90.0 `check` and `clippy -D warnings` pass;
  - `tests/gateway.rs` is 42/42 on three consecutive runs.
- **Release binary.** `53d43b0c…2662` contains none of the test-hook strings
  (`FOUNDRY_GATEWAY_TEST`, `ctxfoundry-fault`, `FOUNDRY_TEST_FAULT`).
- **Real OMP 18.6.0 against a loopback fake upstream** (test build, throwaway `HOME`,
  synthetic key canary):
  - A tool call and its continuation reconciled exactly with `usage import`: 2 attempts,
    2100/1000/38.
  - OMP stopped reading 4–158 ms after the trailing usage chunk: `complete`,
    `client_closed`, counts kept.
  - A finish without usage was aborted by OMP at 2,506–2,684 ms (the pinned 2,500 ms
    grace) and recorded `unknown`.
  - Empty completions produced 12 recorded attempts while OMP persisted none.
  - Upstream HTTP 500 produced 35 attempts in 90 s, with the canary never forwarded.
  - With the slot held by another client, OMP took 10 `gateway_busy` refusals,
    counted in `refused`, with no in-transport retries, then succeeded.
  - Launcher refusals were `profile_exists`, `invalid_argument` (a `default` profile;
    `--hook`), `profile_invalid`, `token_missing` and `gateway_unavailable`. OMP never
    started, nothing reached the upstream, and the global `models.yml` was never
    created.
- **Live runs against Z.ai.** Owner-authorized for 3 runs within 15 minutes; 2 were used
  and finished in 77 s.
  - Run 1, a tool turn, matched `usage import` exactly: 2 attempts and 2 assistant
    messages, input 13,263, cached 6,592, output 57.
  - Run 2, SIGTERM 4 s in, produced one `unknown`/`client_closed` attempt with no
    counts. OMP recorded zeros, its default for absent usage. That attempt may still
    have been billed.
  - While live, `run_dir` was 0700, the token 0600, `gateway.json` 0600 and
    `models.yml` 0600. Afterwards the token and `models.yml` were removed, while
    receipts and sessions remained.
  - A key scan (`grep -F -f` over 35 text files) found 0 matches.
- **Limits of this evidence.** Meter mode only. Receipts are in-memory with an optional
  bounded log, so whole-run coverage stays unverified across crashes. A terminal-
  generated SIGINT is documented, not tested. No savings claim follows from
  forwarding.

## 008 explicit memory — T001 and T002, 2026-10-04

Locally implemented and verified, **pushed 2026-10-05, unreleased**.
- **Authors:** Z.ai GLM-5.3 (implementer), plus captain integration fixes.
- **Review:** cross-lab by OpenAI GPT-6.1 Sol in three rounds:
  - REVISE: 8 Majors, including pending-key migration loss and enabled context reading
    two snapshots;
  - REVISE: 5 Majors (full link validation, memory-ID check in the combined read, a
    swallowed multi-root error, remaining source/memory count consumers, public search
    bounds), and an export byte-cut test that had to actually cut;
  - SHIP.

- **Gates on the integrated main tree.**
  - Source manifest `f6151644…5cdf`, the SHA-256 of sorted file hashes over `src`,
    `tests` (minus the frozen economics corpus), `Cargo.toml` and `Cargo.lock`.
  - `cargo fmt --check` and `cargo clippy --all-targets -D warnings` pass.
  - The full suite: **287 passed, 0 failed, 1 ignored** (the existing payload report),
    and all recovery scenarios passed.
  - Rust 1.90.0 `check` and `clippy -D warnings` pass.
- **`tests/memory.rs` (29 tests).** It covers every T001/T002 Verification bullet and
  the review regressions:
  - put/update/conflict/idempotent revisions; refusals consume no revision;
  - wrong workspace on every op;
  - stale/missing links, with retrieve-grade validation inside the write transaction;
  - source-only search/context exclusion; compact `mem:` lines cut at a UTF-8 boundary
    within the budget;
  - `--no-memory`;
  - a broken lexical index with get/export still working;
  - `corrupt_memory` isolation, including an ID mismatch in the combined read;
  - the v2→v3 upgrade, including a `a`/`source:a`/`source:source:a` prefix chain and
    populated graph/scan rows, interrupted before and after commit;
  - a `memory:x` path versus record `x`;
  - re-index and repair;
  - export paging, including a 4 MiB byte cut with cursor resume;
  - forget/recreate without revision reuse; `revision_exhausted`;
  - content-free reports; training exclusion;
  - multi-root selection and corruption propagation;
  - namespace-specific pending counts; CLI/MCP parity.
- **Catalog.** Measured on the integrated debug binary with `tiktoken-rs` 0.12.1
  `o200k_base`: `tools/list` is **793** tokens for six tools and **669** with
  `--no-memory`. The ceiling is 800.
- **Compatibility.**
  - Store schema 3: `upgrade-store --to 3` from v1 or v2 in one transaction; `--to 2`
    and older readers refuse.
  - Rollback means restoring a pre-upgrade store copy and the previous binary.
  - The search schema is unchanged, so no forced repair.
- **Integration notes.** The implementer repeatedly replaced existing lines instead of
  inserting. The captain restored `pub mod laya`, `use crate::graph`, the
  `RETRIEVE_BEFORE_FINAL_READ` fault point, `scan_status` initialization,
  `open_table(PENDING)` and a displaced doc comment. The captain also routed
  `#[tool_handler]` through the instance router, so `--no-memory` really removes the
  tool. Every removed line in the final diff was audited by the captain and the
  reviewer.

## Token-economics remainder — 003 T005 and 007 T001, 2026-10-04

Locally implemented and verified; committed on main as `bd1d890` (001 T005 amendment),
`5e99ffd` (003 T005) and `cc402e0` (007 T001), **pushed 2026-10-05, unreleased**. Authors: Z.ai
`glm-5.3` (usage import and economics tests, multi-root, OMP hook) and Anthropic Claude
Opus 5.5 (captain: 001 T005 leading-run amendment and integration). Reviewer: OpenAI
`gpt-6.1-sol:xhigh`, one reviewer per slice on a different lab from both authors; not an
independent quorum.

| Slice | First review | Fixes | Final |
| --- | --- | --- | --- |
| 001 T005 leading-run amendment (`src/syntax.rs`, `src/store.rs`) | REVISE: ASCII-only whitespace detached docs (NBSP, form feed); `/**/` broke a JSDoc/Javadoc run; inner-doc test masked | One Unicode whitespace predicate; `/**/` kept; boundary test, which failed on the old predicates | SHIP (delta) |
| 003 T005 usage import and economics tests | REVISE: unchecked per-record OMP input sum; full conflicting id fed a quadratic bounded-error renderer; whole-unit check compared a self-selected slice; fixture gaps; corpus identity unguarded | Checked arithmetic; bounded id in the message; hand-frozen unit spans; extended fixtures; nine frozen corpus digests | SHIP (delta) |
| 007 T001 multi-root | REVISE: unknown alias ignored on a single-root owner; request order replaced admission order; empty-serving refusal listed no coverage; truncation could drop coverage pairs; outline slots backfilled | Alias validation before dispatch; admission-order selection; listed-root refusal; bounded compact pairs; key-counted outline slots | SHIP (delta) |
| OMP first-call hook (team-kit) | REVISE in three rounds: per-command flag arity, relative `input.cwd`, stdio without args, `/` root, `rg` encoding clusters, explicit stdio with a stray `url`, empty `command` | Each fixed with a behavioral case; 21 cases | SHIP (delta) |

Integration found one regression before review: with leading runs inside units, a
tier-1 locator showed `#[derive(…)]` instead of `pub struct BudgetConfig`. The locator
now uses the unit's head (its own node or wrapper start), separate from the outline's
signature start. Units with leading runs need search schema `"3"`; older stores report
`repair_required` until `repair-index`.

**Gates** on the integrated tree (32-file manifest SHA-256 `b885606c…161c`), rustc
1.97.1 and Rust 1.90.0:

| Check | Exit |
| --- | ---: |
| `cargo fmt --check` | 0 |
| Locked all-targets clippy, warnings denied | 0 |
| Locked full test run, no fail-fast: 258 passed, 1 ignored (the payload report) | 0 |
| Rust 1.90.0 check and clippy (warnings denied) | 0 |
| `cargo test --locked --test economics -- --ignored --nocapture payload_report` | 0 |
| `bun ~/.omp/team-kit/bin/check-foundry.ts`: 21 cases | 0 |

**Economics on the frozen corpus** (exact `o200k_base` text-block tokens; one
observation each): 12/12 expected units (six identifiers as search hit #1 retrieved
whole within 2048 tokens; six questions surface their unit within `context` 2048).
`tools/list` is 661 tokens (598 before 007 added `root`/`roots`; ceiling 800).

| Query | v2 search (10) | v1 search | Ratio | v2 search + retrieve | v1 pair | Ratio | v2 context 2048 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `reconstruct_verified` | 253 | 2651 | 9.5% | 854 | 3399 | 25.1% | 1555 |
| `pack_ordered` | 167 | 2056 | 8.1% | 627 | 2806 | 22.3% | 1352 |
| `BudgetConfig` | 440 | 4640 | 9.5% | 570 | 5434 | 10.5% | 2032 |
| `native_discovery_block` | 119 | 1337 | 8.9% | 392 | 2123 | 18.5% | 1356 |
| `fit_prefix` | 192 | 2079 | 9.2% | 865 | 2880 | 30.0% | 1309 |
| `take_context_id` | 133 | 1382 | 9.6% | 261 | 2162 | 12.1% | 1020 |
| `stale handle rejected on retrieve` | 601 | 6483 | — | 1294 | — | — | 2030 |
| `repair index quarantine marker` | 592 | 6069 | — | 1253 | — | — | 2011 |
| `inbound frame byte limit` | 639 | 6478 | — | 739 | — | — | 1979 |
| `session allowance refund` | 606 | 5934 | — | 731 | — | — | 1980 |
| `receipt deduplication conflict` | 602 | 6803 | — | 848 | — | — | 2012 |
| `sweep unseen sources after complete scan` | 607 | 6328 | — | 685 | — | — | 2019 |

The search (≤20%) and search-plus-unit (≤35%) targets hold for all six identifier
queries. The ≤40-token header bound is asserted on the v2 success fixtures of
`tests/response.rs` (`assert_v2_success`) and `tests/mcp.rs` (`assert_v2_delivery`);
this report does not measure it separately. v2 `context` ranges 1020–2032
tokens against v1's 1439–1853 at the same 2048 request; there is no context-size
target.

**Latency of the two T006 risks** is recorded under the 001 T005/T006 acceptance below.

**Real hosts (003 T005 runbook)**, counters only in
[`real-host-te-2026-10-04.json`](review/real-host-te-2026-10-04.json): release binary
SHA-256 `e32204fb…6cee7` built from the gated tree; OMP 18.6.0 with `zai/glm-5.3`;
Codex CLI 0.159.2 with `gpt-6.1-sol`. Eight runs in 358.7 s of the 2400 s cap, no
timeout or harness failure, plus one owner-authorized rerun of run 8 (run 9, 39.4 s;
398.1 s in total). Label: **bundled Foundry adoption (v2 + instructions +
optional hook), n=1 per task and arm**, not an isolated v2 effect or a general savings
result. Every OMP import is `complete:false` because the session usage records omit a
reasoning-token category; totals are the reported input-plus-output sums, and the
reasoning breakdown is unknown.

| Run | Oracle | Wall s | Provider tokens (input / cached / output) | Tool calls | Foundry / host payload estimate |
| --- | --- | ---: | --- | --- | --- |
| 1 hook probe, F | pass: step 1 refused with the hook reason; repeat, regex and `/tmp` greps ran; Foundry search via `xd://` then its grep ran | 69.2 | 80,013 (76,740 / 61,440 / 3,273) | grep 5, foundry.search 1, read 1 | 368 / 738 |
| 2 A1-H | pass | 31.8 | 114,208 (113,129 / 83,200 / 1,079) | grep 3, bash 1, read 1, context 1 | 0 / 16,075 |
| 3 A1-F | pass | 46.3 | 90,464 (89,081 / 84,736 / 1,383) | foundry.search 2, foundry.retrieve 3 | 2,668 / 0 |
| 4 A2-H | pass | 27.4 | 79,360 (78,282 / 71,936 / 1,078) | grep 2, read 2 | 0 / 5,752 |
| 5 A2-F | pass | 70.9 | 97,623 (94,323 / 87,424 / 3,300) | foundry.search 2, foundry.retrieve 4 | 4,686 / 0 |
| 6 A3-H | pass: check exit 1 → 0 | 25.6 | 66,163 (65,655 / 53,312 / 508) | bash 2, read 2, edit 1 | 0 / 883 |
| 7 A3-F | pass: check 1 → 0; pre-edit handle now `stale_handle` | 68.5 | 163,876 (162,335 / 158,016 / 1,541) | foundry.search 2, foundry.retrieve 4, foundry.index 1, read 2, bash 1, edit 1 | 1,082 / 736 |
| 8 Codex T003 discovery | **miss**: first discovery was shell `rg`; the answer was correct (`src/records.rs:5`, callers `src/main.rs:13`, `:33`) | 19.0 | not imported (`--ephemeral`); stdout turn usage 50,729 (50,493 / 44,928 / 236) | shell 2 (`rg`, `nl`) | — |
| 9 Codex rerun with instructions | pass: first discovery Foundry `search`; the answer cites `src/records.rs:5` and both callers | 39.4 | not imported (`--ephemeral`); stdout turn usage 105,511 (104,919 / 87,552 / 592) | context-foundry search 2, retrieve 5 | — |

Findings, reported without prompt or threshold tuning:

- A1-F used 21% fewer provider tokens than A1-H; A2-F used 23% more and A3-F 2.5×
  more (A3-F's prompt adds the index refresh and citation, a recorded asymmetry
  against F). Total imported tool-result payload estimates fell from 16,075 (H) to
  2,668 (F) for A1 and from 5,752 (H) to 4,686 (F) for A2. These opened a follow-up
  decision; the owner chose a root-cause analysis of the retained sessions (below).
- **Cost root cause** (retained transcripts, arithmetic on the recorded turns; no new
  runs or tuning). OMP re-sends the fixed prefix and task prompt plus accumulated
  history on each turn. The fixed prefix plus initial user content is about
  12.4–13.0k tokens per request; complete request input also includes replayed
  history. F's fixed prefix is 11,672 tokens (11,671 in A3-F), versus 11,130 in H: a
  bundled-setup difference of 542 tokens per request (541 in A3-F).
  A1: one unscoped whole-repository grep in A1-H returned 51 KB, re-sent on three
  later turns; H's accumulated history outweighs F's extra turn and catalog. (Turn-1
  provider-cache warmth differed between arms but cannot move these totals, which
  count cached tokens one for one.) A2: the extra F turn came from a failed
  `retrieve`; removing that round trip leaves F 2.5% above H. A3: 11 turns against 5;
  removing the runbook's refresh and citation turns would save about 47.7k input
  tokens and, on top of that, the turn spent on a failed retrieve about 14.6k (each
  estimate is arithmetic on the recorded run and they do not add exactly); a failed
  retrieve inside another turn re-sent about 1.9k; F also fell back to two plain-read
  turns. Three of eleven in-session retrieves failed: twice the agent sent `lines` as
  a JSON array (`[1630, 1800]`), which OMP rejected against the schema's `a-b` string
  pattern before calling Foundry, echoing the arguments in about 870 bytes; once it
  copied a handle's byte range (`#650-1459`) as line numbers (`invalid_range`).
  Accepting the array form and separating byte ranges from line numbers in handles
  are Foundry-side options for the owner. Additional turns and replayed history are
  observed costs; avoiding refusals and redundant calls is one option, but these n=1
  runs do not establish that fewer turns are necessary for further savings.
- Run 8: no MCP call and no MCP error item. A later local probe of the same release
  owner on the run's store (no model) listed all five tools. Unlike the passing
  2026-10-01 Codex run, which shared the OMP F arm's copy and its Foundry `AGENTS.md`,
  this copy carried no project instructions (the runbook then named none). The owner
  amended the runbook to write the printed Codex instructions into the copy and
  authorized one rerun (run 9): Codex then called Foundry `search` first and answered
  correctly, using about twice run 8's provider tokens (two searches and five
  retrieves against two shell calls on this two-file fixture).
- The A/B F arms (runs 3, 5 and 7) first called Foundry search through the `xd://`
  device and ran no grep or `rg`, so the hook blocked nothing in runs 2–7; run 1 shows
  one refusal followed by an allowed identical repeat.
- A user-level custom Prakarana `context` tool stays loaded under `--no-extensions` in
  both arms; A1-H's one `context` call returned a no-daemon error.

Deviations: OMP 18.6.0 instead of the runbook's 18.4.10 (run 1 confirmed the hook
matchers); the hook loaded from the team-kit source path, not an installed copy,
because `./install.sh --sync` would also install 42 unrelated uncommitted team-kit
edits; the binary and the A3 checks used private `CARGO_TARGET_DIR`s.

Committed after the runs (owner decision): the team-kit hook as team-kit `e65f8cf`, only
its own files (the 42 unrelated kit edits untouched), installed by copying the one
extension file into `~/.omp/agent/extensions` (inactive until `TEAM_KIT_FOUNDRY_ROUTE=1`;
not in the kit's install manifest). Nothing is pushed or released. Residual: the
long-id usage regression test measures elapsed time but has no watchdog, so a
reintroduced quadratic renderer would hang it rather than fail it (reviewer minor,
accepted).

This section and the real-host record were reviewed by the same OpenAI
`gpt-6.1-sol:xhigh` lane: REVISE (a private hook digest and home path in the record,
Codex stdout usage reported as absent, an unsupported reasoning-exclusion claim, three
wording scopes), then SHIP on the corrections.

## 001 T005/T006 local acceptance — 2026-10-04

001 T005 (syntax units, search index v2, two-tier ranking, `path` filter) and T006
(outlines, forms ladder, candidate seam, retrieve `view`; `following_chunks` removed)
are locally implemented and **unreleased**. Each was accepted at the r3 **SHIP** of the
same existing reviewer, `T004CutoverReview` (OpenAI `gpt-6.1-sol:xhigh`), cross-lab
to the Anthropic author; one reviewer, not an independent quorum. Both were committed
with T004 in `5edf32c`, whose 19 code files matched the accepted T006 hashes. Receipts,
review history and the gate script are retained outside Git in the owner's handoff
folder (`handoffs/context-foundry-001/artifacts/`); this record restates them and did
not rerun the gates.

| r3 gate (each task) | Exit |
| --- | ---: |
| `cargo fmt --check` | 0 |
| Locked all-targets clippy, warnings denied (rustc 1.97.1) | 0 |
| Locked full test run, no fail-fast | 0 |
| Rust 1.90.0 `cargo check --all-targets` (toolchain first on `PATH`) | 0 |
| Rust 1.90.0 clippy, warnings denied | 0 |
| Freeze recheck: 19/19 files unchanged during the run | 0 |

T006 test counts: lib 11, cli 16, core 36, integrity 15, laya 3, mcp 73, recovery all
passed (custom harness), repair 9, response 15, scan_faults 15, syntax 28; 0 warnings.
Review rounds: T005 r1 REVISE (C/C++ parenthesized declarator names; native recursion
and a quadratic parent walk), r2 REVISE (quadratic qualified-name content), r3 SHIP;
T006 r1 REVISE (mandatory member signatures, outline refusal hints, graph window,
Python span ends, skip-versus-stop), r2 REVISE (C++ template wrapper on members), r3
SHIP. Each fix had RED captured on the frozen pre-fix tree before GREEN. Owner decisions
taken in review — the 256-byte qualified-name tail, metadata-only upgrade with explicit
`repair-index`, and the two outline refusal policies — are written into the
[v2 contract](../specs/001-source-state-recovery/contracts/context-v2.md). No RED was
captured for T005's path-filter tests or limit-cut change.

**Catalog re-measured, 2026-10-04.** At `c430997` (clean source; debug `foundry`
SHA-256 `2f455d16…`) a throwaway stdio probe, mirroring `tests/mcp.rs` `RawStdio`,
counted the raw `tools/list` result: 5 tools, 2401 bytes, **598** o200k tokens (568 at
T004; v1 711; ceiling 800). T005/T006 grew the `search` (`path`) and `retrieve`
(`view`) schemas. The probe was outside the repository and was removed.

**Latency of the two T006 risks, measured 2026-10-04.** Release `foundry` built at
`c430997` (SHA-256 `7e85efff…`); CLI wall time including process start and store
open, median of 3 runs, local macOS arm64; disposable fixtures, removed afterwards.

| Corpus | context `tokens` 2048 | context `tokens` 32768 | search |
| --- | ---: | ---: | ---: |
| Real: `git archive` of prakarana `ae159e85` (5,481 sources; index 36.5 s), four queries (one with no hits) | 0.08–0.27 s | 0.08–0.35 s | 0.08–0.12 s |
| Synthetic worst case: 32 Rust files of about 1 MiB, each holding one matching unit | 4.19 s | 4.24 s | 0.22 s |

Increasing the context budget from 2048 to 32768 changed the reported CLI wall time by
at most 0.1 s. The synthetic 32-file corpus took about 4.2 s for context versus 0.22 s
for search. Context additionally constructs syntax-based forms and ladder-packs them;
this probe did not time parsing, form rendering and packing separately, so their
individual costs are not established. The approximately 4.2 s CLI wall time is about
84% of the MCP owner's 5,000 ms read-deadline duration, but is not an MCP latency
measurement. 007 plans to share that deadline sequentially across roots, so these
results identify a deadline risk, not a measured multi-root failure. Whether to bound
or cache query-time parsing before release remains an owner decision; the measured
real corpus's CLI times were substantially below five seconds.

## 001 T004 local closure — final4

001 T004 is locally implemented and verified, **unreleased**. The owner reports that
all six final4 gates exited 0 on the unchanged source manifest retained at
`/private/tmp/cf-t004-gates-final/manifest.txt`; logs are in that directory. The same
existing `T004CutoverReview` (OpenAI `gpt-6.1-sol:xhigh`) returned **SHIP** after its
reported finding/delta closures. The author lane is Anthropic. This is the existing
in-session cross-lab review, not a fresh independent review. This docs-only closure
does not rerun or independently re-certify the six checks.

| Reported final4 gate | Exit |
| --- | ---: |
| `cargo fmt --check` | 0 |
| Locked all-targets clippy, warnings denied | 0 |
| Locked full test run, no fail-fast | 0 |
| Rust 1.90.0 check | 0 |
| Rust 1.90.0 clippy | 0 |
| CLI smoke | 0 |

The retained MSRV version log names rustc/cargo 1.90.0 and clippy 0.1.90. The retained
smoke shows index/search and `retrieve --lines 1-3` succeeding with v2 text and exact
`parse_record` bytes. Its negative `--lines 4-6` returns `invalid_range`, exit 2,
as invalid input under the existing CLI contract; that is not a changed exit rule
or a failed smoke gate.

| Measurement | T004 v2 | v1 comparison | Boundary and scope |
| --- | ---: | ---: | --- |
| Handle on `examples/workspace` | 33 o200k tokens; 68 bytes | 94 tokens; 207 bytes | Isolated v2 handle versus v1 JSON handle; fixture-specific, superseding the 29/85 estimate |
| Five-tool `tools/list` | 568 o200k tokens (598 after T005/T006, above) | Historical baseline: 711 | Serialized catalog; the 800-token ceiling is met |

These are handle/catalog measurements, not the complete twelve-query payload
comparison, provider usage, a universal per-handle cost or host-session savings.
T005, T006 and their measurements are recorded above; the 003 T005 remainder and
007 T001 are recorded in the 2026-10-04 token-economics section at the top.

**Reviewed ambiguity limitation:** a valid path embedding a complete handle suffix
and valid item tail can give an item line two complete readings. The test parser
`testkit::parse_v2` refuses both without attributing either handle; production
rendering and direct retrieve are unaffected. No universal item-line round trip is
claimed. The required 4096-byte special-path CLI/MCP/core cases passed in final4.
Zero-allowance `budget_exhausted` uses an outcome-free sufficient budget, not a
proved minimal budget, as the existing v2 refusal-floor contract allows.

## Token-economics v1 payload baseline — 2026-10-03

This is historical v1-era evidence. Its frozen queries and byte ranges describe
`6bb81e6`, including the subsequently deleted `take_context_id`; they are not
claims about current symbol availability.

Phase 0 capture for the approved token-economics tranche (001 T004–T006, 003 T005,
007 T001), taken on baseline `6bb81e6` before any code change, on the local macOS
arm64 workstation. One run of a temporary `#[ignore]` test appended to `tests/mcp.rs`,
reusing that suite's real rmcp client helpers:
`cargo test --locked --test mcp -- --ignored --nocapture v1_payload_report` → 1 passed,
65 filtered out, 2.93 s. The test was then removed byte-exactly (`tests/mcp.rs` is back
to its 4686 baseline lines at capture time, with no diff against `6bb81e6` then).
One observation per cell; no repetition or variance claim.

- **Corpus:** the frozen nine-file economics corpus, exact bytes at `6bb81e6` of
  `src/{store,response,mcp,ingest,bootstrap,receipts,config,error}.rs` and
  `docs/architecture.md`, indexed as one workspace from a temporary copy.
- **Boundary:** exact `o200k_base` `encode_ordinary` count of the MCP text-block
  content Foundry emits, which the
  [v2 contract](../specs/001-source-state-recovery/contracts/context-v2.md) counts.
  It is the delivered payload, not host or provider cost: hosts forward it unchanged
  only below their own size limits.
  v1 itself budgets the whole serialized `CallToolResult`, including a second JSON
  escaping, so a 2048-token v1 request delivers fewer text-block tokens than 2048.
- **Requests:** `search` with `limit` 10; `retrieve` of search hit #1's handle with
  `tokens` 2048; `context` with `tokens` 2048. No request returned an error.
- **Catalog:** serialized `tools/list` catalog, 5 tools: 3044 bytes, 711 tokens.

| Query | Kind | Search hits | Search tokens | Retrieve hit #1 tokens | Context tokens | Hit #1 v1 block (bytes) |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| `reconstruct_verified` | identifier | 4 | 2651 | 748 | 1457 | `src/store.rs` 52648–54672 |
| `pack_ordered` | identifier | 3 | 2056 | 750 | 1498 | `src/response.rs` 8077–10069 |
| `BudgetConfig` | identifier | 7 | 4640 | 794 | 1567 | `src/config.rs` 0–2028 |
| `native_discovery_block` | identifier | 2 | 1337 | 786 | 1439 | `src/bootstrap.rs` 20047–22048 |
| `fit_prefix` | identifier | 3 | 2079 | 801 | 1808 | `src/response.rs` 14136–16142 |
| `take_context_id` | identifier | 2 | 1382 | 780 | 1483 | `src/mcp.rs` 38555–40586 |
| `stale handle rejected on retrieve` | question | 10 | 6483 | 748 | 1478 | `src/store.rs` 58701–60738 |
| `repair index quarantine marker` | question | 10 | 6069 | 689 | 1853 | `src/store.rs` 64784–66820 |
| `inbound frame byte limit` | question | 10 | 6478 | 796 | 1836 | `src/mcp.rs` 4061–6076 |
| `session allowance refund` | question | 10 | 5934 | 732 | 1784 | `src/mcp.rs` 12188–14200 |
| `receipt deduplication conflict` | question | 10 | 6803 | 767 | 1460 | `src/receipts.rs` 14222–16240 |
| `sweep unseen sources after complete scan` | question | 10 | 6328 | 800 | 1445 | `src/store.rs` 34366–36411 |

Byte ranges are half-open. Every hit #1 is one v1 storage block of at most 2048 bytes,
whatever the size of the matched code; this probe did not check whether that block
holds the expected definition. These rows are the comparison basis for the targets in
the [adapter economics contract](../specs/003-agent-retrieval-context/contracts/adapter-economics.md):
v2 search at most 20% of the identifier rows' search tokens, v2 search plus unit
retrieve at most 35% of search plus hit-#1 retrieve, a context header of at most 40
tokens against v1's 199-token minimum envelope (2026-10-01 allowance run below),
12/12 expected units and `tools/list` at most 800 tokens. Full-tranche payload/host
comparisons await the remaining tasks; T004's handle/catalog measurements are
recorded above. The historical v1 table remains unchanged.

## 001 + 003 first implementation — 2026-10-01

This section records the historical v1 wire and counting boundary; its envelope
fields and measurements are not descriptions of current T004 output.

Owner-approved scope: 001 T001–T003 and 003 T001–T003, including the optional shared
MCP owner for concurrent OMP/Codex sessions. Verified locally on macOS 26.5 arm64
(rustc 1.97.1) and in a Rust 1.90.0 Linux aarch64 container. Not published, signed or
packaged; no hosted CI run. No predecessor store, service or global host configuration
was touched; real-host runs used temporary public fixtures and fixture-scoped config.

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Pass |
| `cargo clippy --locked --all-targets -- -D warnings` | Pass |
| `cargo build --locked --all-targets` | Pass, 0 warnings |
| `cargo test --locked --no-fail-fast` | Pass: 157 tests (unit 10, cli 13, core 20, integrity 15, laya_protocol 3, mcp 65, repair 8, response 8, scan_faults 15) plus 12 child-process recovery scenarios |
| `cargo build --locked --release` | Pass; SHA-256 `c36d0fb3975168b115309946954f8f673dc8252e2682420fae4865373c62c996`; 0 fault-hook strings |
| Rust 1.90.0, Linux aarch64 (`rust:1.90-slim-bookworm`) | `cargo check --all-targets --locked` pass; core 20, integrity 15, recovery, repair 8 and scan_faults 15 pass, including the non-UTF-8 filename branch APFS cannot create |
| README commands on the release binary | Pass, including `store_not_found` on a read before `index` |

Fault and recovery tests use named fault points behind the non-default `test-faults`
feature (enabled for tests by a self dev-dependency) and a separate `foundry-faults`
test binary. The shipped `foundry` binary has no arming path in any build; release
builds contain no fault-point strings. `--all-targets` debug builds contain four inert
fault-point names because Cargo unifies the test feature there.

**Real hosts (003 T003),** shared HTTP owner, fixture `tests/fixtures/agent-task`, host
instructions printed by `connect`. The host runs below used release `8968e70b…`; the
only later v1-era change added the counted `budget_limited_by` JSON-envelope field
and limiter-named refusals, rechecked by the mcp suite and the allowance run on `c36d0fb3…`:

- OMP 18.4.9 (model `zai/glm-5.3`), five runs: every first eligible discovery call was
  Foundry `search`, with no grep or ripgrep before it; the agent edited `parse_record`,
  the checker passed 4/4, it re-indexed through MCP and the pre-edit handle returned
  `stale_handle` (revision 6). An exact byte-pattern request used host `grep`
  (permitted). With the owner stopped, OMP's own diagnostic named `failed to connect`
  and the agent fell back to host tools. In both fallback cases the agent's reply did
  not restate the reason. Instruction-based preference only; no enforced routing hook.
- Codex CLI 0.159.2 (`gpt-6.1-sol`): native Streamable HTTP attachment (negotiated
  2025-06-18), tool listing, `search` and `retrieve` succeed. Alone, the discovery task
  listed the directory, then used Foundry `search`/`retrieve` and cited
  `src/records.rs:5`. Codex requires MCP tool approval by default; without the printed
  approval configuration non-interactive runs hang or refuse every call.
- Concurrent, same shared owner and fixture store: OMP performed the edit task while
  Codex answered a read-only discovery task. Their Foundry calls interleaved (OMP
  `search` 24.4 s, Codex `search` 31.1 s, OMP `retrieve` 35.5 s, Codex `retrieve` 44.9 s,
  OMP `index` 88.4 s); both exited 0, the owner exited 0 and the checker passed. Codex
  used only Foundry tools and cited `src/records.rs:5` and both callers. Three earlier
  concurrent attempts stalled Codex because the captain's harness never drained
  Codex's verbose stderr pipe; with stderr sent to a file the stall did not recur.
  Product code was unchanged by that harness fix. Evidence:
  [real-host record](review/real-host-t003-2026-10-01.json).
- Delivery allowance on a real host (release `c36d0fb3…`, OMP, owner launched from the
  printed `--budget` with `session_context_tokens: 500`): the first context request
  for 2000 tokens delivered with the v1 JSON fields `requested_budget: 500` and
  `budget_limited_by: session_allowance`; the next two were refused with
  `budget_exhausted` ("cannot fit within 73 tokens (limited by session_allowance);
  minimum 199 tokens"), and the unchanged 73 shows refusals are not charged.
  Host-request mode stays `budget_scope_unsupported`; neither host offers the hooks.

**Reviews.** Recorded session events, not self-reports: source review round 1 and
adapter review round 1 ran on OpenAI `gpt-6.1-sol`, cross-lab to the Z.ai GLM-5.3 and
Anthropic Sonnet 5.5 authors; source round 2 fell back to Sonnet 5.5, same lab as that
revision, so a final OpenAI pass (`codex exec`, `gpt-6.1-sol`) covered the integrated
tree. Its three new defects were fixed; it could not bind HTTP sockets in its sandbox,
so HTTP behavior rests on the native test run above.

**Accepted limitations.** Names are enumerated by path while bytes are read through
the held root: a same-user process that swaps and restores a directory during one scan
can retire records of still-present files until the next scan (001 records the
re-entry). The 16-handler cap is proven at its admission owners, not with 16 truly
stalled handlers (one engine slot makes that unreachable). Cancellation-registration
ordering is fixed by construction without an interleaving test. Several MCP tests
observe timing and fail closed on an unusually loaded machine. One continuation test
was made full-suite-safe after a single unreproduced load failure.

## First prototype slice — 2026-09-28

Local validation on 2026-09-28, macOS, Rust 1.97.1. No predecessor build, benchmark
or live-store experiment was part of this validation.

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Pass |
| `cargo clippy --locked --all-targets -- -D warnings` | Pass |
| `cargo test --locked` | Pass: 10 integration tests |
| `cargo build --locked --release` | Pass |
| Release CLI help → index → graph import → context on the bundled fixture | Pass: 2 sources, 1 manual edge, 439 context-text tokens under a 1024-token budget |
| Public documentation relative links | Pass |

Tests cover source replacement/deletion and stale-search rejection, reopen with
pending index work, rebuilding a missing search index across another restart,
graph producer isolation/hash validity/traversal bounds, UTF-8 token-budget
accounting, workspace binding, feedback consent/splitting, the CLI lifecycle, and
Laya response parsing/fallback through a real local HTTP transport fixture.

The HTTP fixture is not a model. No Laya inference, fine-tuning, candidate promotion,
provider billing or task-quality experiment ran. No million-file scale, power-loss
fault injection or cross-platform result is claimed. The configured Linux/macOS
CI and Rust 1.90 minimum-toolchain jobs have not run on a hosted runner yet.

Release executable SHA-256 for this local build:
`5589bc3018c4149e9bd62804a0eab13e27a5d91ba966fefea54ed69efc16da78`.
This is a local build fingerprint, not a signed distribution artifact.

## Subsequent feasibility, 2026-09-29

[Bounded feasibility results](review/feasibility.md) add actual sandboxed model,
gradient, vector-index and Rust 1.90 MCP probes. They live in a separate scratch-probe
crate and do not change the root product or retroactively complete its proposed specs.
No production test suite rerun was needed for these documentation/probe-only edits.

## Artifact, accounting and card reuse — 2026-10-09

Experimental E1 continuation: verified artifact receipts, transactional cache totals and
exact per-card normalized-segment reuse. Linux sandbox: 165 library tests and 65 neural
integration tests passed; Clippy all targets passed. Receipt tests count artifact reads,
refuse replacement/in-place modification and descriptor mismatch; cache tests cover warm
runs, stale finalizers, duplicate keys, mixed retained rows and restart. Card tests compare
exact IDs/offsets/keys/spans with the ordinary tokenizer and bounded overflow fallback.
These fixtures establish work reuse and equivalence, not model speed, cost or task quality.
macOS supervised `/dev/fd` loading remains unexecuted here.

## Continuous semantic maintenance — 2026-10-09

Experimental E2 implements schema-9 source-transaction obligations, independent bounded
semantic/knowledge lanes, exact conditional acknowledgments, fair changed-path selection,
explicit continuous preparation and cheap publication observations. Fixtures run the
real redb, tokenizer, USearch and MCP/watch paths with a counted fake embedding provider.
They cover edit/revert/rename/delete, inference races, pause/restart, explicit startup,
queue overflow and late acknowledgments, hot-path rotation, schema upgrade, status
polling without census/model work or batch throttling, and progress during filesystem
traffic followed by convergence. Recovery snapshots now include both obligation tables.

Focused checks passed: 170 library, 65 neural, 61 neural retrieval/MCP, 18 brief and
19 task-context tests (one real-model lifecycle test remains ignored). The full
`cargo test --locked --all-targets --no-fail-fast` run passed **915 tests**, with two
intentional ignores, plus the custom crash-recovery runner. The 6,000-file watcher
fixture passed. Subsequent review checks reran all 170 library and 61 neural
retrieval tests, including explicit mode switching, and the five work-queue tests
with owner/lane fences. `cargo clippy --locked --all-targets -- -D warnings`,
`cargo fmt --all --check`, `cargo check --locked --no-default-features --lib` and the
Markdown link check passed. The real MCP catalog measured **996 o200k tokens**, inside
its unchanged 1,000-token limit. A CLI smoke check verified cheap status, explicit
`--audit` and refusal of `--semantic-maintain` without a profile. These tests establish work
selection and recovery; publication still walks prepared mappings and can rebuild the
full ANN. E3 vector/location deltas and E4 model-aware inputs remain pending. No
80,000-file throughput, real-provider token economics or task-quality result is claimed.

## ANN delta publication — 2026-10-09

Experimental 009 T008/E3 introduces generation v4: one immutable USearch base, one
bounded delta, independent active locations and a single atomic manifest switch.
The actual tokenizer/redb/USearch/MCP paths use counted fixture providers here.

Focused tests verify zero vector decodes/builds for location-only changes, unchanged
base/delta file identities, one-vector delta work, duplicate input suppression,
reactivation of reverted base keys, compaction thresholds, empty generations,
malformed membership refusal and bounded orphan reclamation. An 80-tombstone fixture
places retired vectors ahead of fresh base hits in the 64-candidate window; filtered
ANN selection still returns a full active window merged with the new delta hit.
Real child-process aborts before/after the manifest switch cover both delta publication
and compaction: reopen keeps a readable generation and pending obligations, then
publishes from committed cache without a provider. These establish application ordering,
not hardware power-loss immunity.

The driver fixture exercises three visible publication retries, serviceable foreground
reads during backoff, exhaustion without a second finalization retry cycle, durable
pending work and explicit recovery without more embedding. The real MCP maintenance
fixture verifies that an edit increments delta publications without replacing the base
ANN, then covers revert, rename, deletion, pause and restart.

Focused checks passed: nine index tests, the four child-process crash boundaries, all
62 neural library tests and the continuous-maintenance MCP case. Clippy all targets
and the build without default features passed. Across the full all-targets run,
**919 regular tests passed**, with two intentional ignores, including the 6,000-file
watcher fixture. A concurrent test rebuild replaced the self-executing recovery binary
mid-run, causing five harness restart failures (`ENOENT`); after builds finished, the
complete recovery runner passed **all 20 cases** separately. This was test orchestration,
not a product recovery failure. Final Clippy, formatting, build without default features
and Markdown link checks passed (47 files, 694 links, 153 heading anchors).

At this checkpoint, full mapping scans, cache-layout probes, referenced-file hashing,
complete location-map writes and resident-generation reload remained T008 costs; the
warm-reuse continuation below addresses three of these. E4 model-aware inputs remain
pending. This is no real-device speed, provider-token-cost or task-quality claim.


## Warm publication reuse — 2026-10-10

Experimental 009 T008 now keeps a disposable mapping view per Engine, refreshes paths
and input-key dependents recorded with committed source/partition/cache changes, and
shares bounded file-verification receipts and immutable resident USearch segments
between publication and serving. No durable format or schema changed.

The mapping fixtures use 260 admitted sources and count actual partition reads,
full-mapping pages and cache probes. Quiet replay does no such work; completing a
shared cached input refreshes its two paths without a source revision change. Source
edits/deletion/sweep remove stale locations, rejected commits leave the view clean,
and changes between pages remain pending after an incomplete result. Cancellation,
overflow, profile changes, purge and reopen rebuild safely; a subsequent warm pass
stays incremental. Explicit audit still walks authoritative rows.

The retrieval fixture uses the actual renderer, redb store, publisher, USearch loader
and query runtime with deterministic concept vectors. A warmed location-only change
reads one affected partition and performs zero ANN hashes/restores. A new delta
hashes/restores only that segment; explicit generation refresh and base reversion reuse
resident segments. Crossing the compaction threshold restores the new base once.
Held old query windows preserve their generation revision. Reopen invalidates owner
reuse, and profile/manifest changes are detected without an invalidation hint.
Same-size in-place mutation, replacement and symlink substitution refuse on refresh;
changed-during-read and cancellation cases cannot mint a receipt. Receipt-cache waiting
obeys control, retention stays at two segments, and explicit audit rehashes independently.

The full regression run passed **928 regular tests**, with two intentional ignores,
and **all 20 child-process recovery cases**, including delta/compaction publication.
The 6,000-file watcher fixture passed. Final Clippy (all targets, warnings denied),
formatting, the build without default features and Markdown link checks passed
(47 files, 696 links, 153 heading anchors).

These fixtures establish work selection, identity checks and recovery, not model quality,
provider-token savings or 80,000-file throughput. At this checkpoint, full location
serialization, membership/label metadata work and a restart baseline remained; the
following continuation addresses those repeated costs.

## Compact publication metadata — 2026-10-10

Experimental 009 T008 generation v5 now publishes immutable base membership and a
location base with one bounded replacement/tombstone overlay. Persistent ordered maps
share unchanged nodes across the mapping view, publication and serving. Metadata receipts
reuse decoded artifacts; a redb mapping revision lets an unchanged restart reuse verified
publication coverage without a database mapping scan. Schema remains 9.

Focused fixtures exercise real redb, publication, metadata validation and USearch:

- A location edit writes one overlay; serving reads/hashes/decodes only that new metadata
  file. An empty source then changes coverage without location comparisons or metadata
  writes/reads. A failed manifest switch retries from the warm view without a full scan.
- One ANN-delta addition plus 4,095 shifted inputs fits the 4,096-key location overlay.
  The next shift rebases locations while ANN base/delta and membership inodes remain
  identical. Failed rebase leaves the prior generation readable; retry converges, cleanup
  stays bounded, and edits/reverts operate against the new location base. The fixture
  manifest remains below 2 KiB with over 4,000 base keys.
- Cold clean publication returns Current with zero mapping pages, partition reads or
  cache-layout probes; it still reads and verifies the immutable artifacts. Same-source-
  revision partition changes and offline source edits invalidate this reuse and rebuild
  the baseline. Failed source commits leave the durable mapping revision unchanged.
- Duplicate membership/overlay keys, malformed units, redundant replacements, excess
  overlay entries and unbound active keys refuse even with matching content hashes.
  Same-size in-place edits and replacements invalidate metadata receipts; cancellation,
  changes during verification and byte limits cannot mint/reuse an invalid receipt.
- Older derived formats republish from cache without inference; generation v4 has an
  explicit migration fixture in addition to the existing v1 case. Process recovery adds
  before/after location-overlay manifest switches to the ANN delta/compaction scenarios.

Regression verification passed **934 regular tests** (two intentional measurement
ignores) and **all 20 child-process recovery cases**, including six semantic crash
boundaries across delta, ANN compaction and location publication. The 6,000-file watcher
fixture passed in the regression run (89.43 s for its suite). One prior-format file-count
assertion was updated from six to seven; the complete affected suite and remaining
regression suites then passed. Earlier successful suites were retained. Final all-target
Clippy with warnings denied, formatting, the build without default features and Markdown
link checks passed (47 files, 700 links, 154 heading anchors).

These fixtures establish bounded work, integrity and recovery. They do not measure
large-repository throughput, actual provider-token savings or task quality. Initial
bootstrap, cold artifact verification, changed-input restart baselines and compaction
remain corpus-sized. The first changed mapping after a clean restart still needs its
baseline; the revision shortcut applies to publication, not preparation's source walk.


## Model-aware body/parent inputs — 2026-10-10

Experimental 009 T009 adds strict versioned descriptor geometry and an explicit
body-v1 representation. Address profiles serialize identically without the new fields.
Both CLI and MCP preparation pack actual token lengths within independent per-input,
call-total and sequence bounds. The macOS adapter checks configured geometry against
the loaded GGUF and actual backend capacities; no default model window is raised.

Executed fixtures use the real tokenizer, parser, redb store, cache, ANN publication,
query runtime and MCP stdio consumer with deterministic provider vectors:

- Complete short units, long windows, Unicode/CRLF and unsupported syntax retain exact,
  non-overlapping source spans. Candidate re-encoding verifies total input limits;
  an impossible template refuses instead of dropping body characters.
- Body-only behavior reaches final packed context with the child's exact span. A body
  edit embeds one changed input; included parent-header edits invalidate their child
  context. Unchanged preparation, same-language rename, body/parent revert and return
  to the address recipe issue zero document calls. Old cache rows remain available.
- Explicit 8,192-token geometry passes documents longer than 2,048 tokens intact, while
  short inputs share batches beyond the old worst-case-size cap. Observed call tokens
  equal the preparation report. Continuous MCP maintenance catches a body edit and
  preserves actual call bounds alongside foreground context retrieval.
- Retained-card and forced-fallback paths produce identical keys and token IDs across
  multiple selection batches for both recipes. Fresh handoff avoids a second encoding;
  body fallback builds one lazy span plan, then re-encodes only selected windows.
- IPC round-trips preserve each sequence at 2,048, 8,192 and 32,768 total-token
  boundaries; excess input length, total tokens and sequence count refuse. Unknown
  geometry versions and invalid limits refuse; explicit geometry changes vector identity.

The full regression run passed **943 regular tests** (two intentional measurement
ignores) and **all 20 child-process recovery cases**. The 6,000-file watcher suite
passed in 87.46 s. After the final query-boundary assertion and test cleanup, all
77 neural unit tests and 70 retrieval integration tests passed again (one intentional
measurement ignore). Final all-target Clippy with warnings denied, formatting, the
build without default features and Markdown link checks passed (47 files, 710 links,
159 heading anchors). The explicit-geometry query fixture passes an input longer
than the legacy limit intact and refuses an oversize query before a provider call.

These establish representation, work selection, token admission and provenance, not
embedding quality or device throughput. Linux cannot build/run the selected macOS
Metal adapter. Pinned llama.cpp API/example/operator-test references were inspected;
real-model single/batched numerical parity for new geometries remains unexecuted. The
existing ignored real-worker parity fixture now reads the selected descriptor's input
limit. Parent/header text is a bounded child prefix; independent parent/file/area vectors,
contextual token-state late chunking and 010 T002 generated knowledge remain future work.


## Selective knowledge generation

Experimental 010 T002 now has explicit `knowledge prepare`, `status` and `purge-cache`
CLI operations with a strict configured HTTP provider/source scope. Schema 10 adds a
bounded completion cache and optional transitive area lineage to the existing brief
owner. Current inputs bypass body reconstruction, tokenization and provider calls;
selected source/area results remain derived, with exact original evidence recoverable.

`tests/knowledge.rs` uses a real local TCP/HTTP fixture and the actual CLI, tokenizer,
redb transactions, lexical index and context packer. All completions and reported
provider usage in these tests are **synthetic**. The executed boundaries cover:

- Partial two-call preparation followed by restart generates only the two missing
  ancestors. A complete unchanged replay sends zero requests, including when a fault
  would refuse source-body preparation. Independent embedding obligations stay pending.
- A source edit regenerates the changed child and two ancestors, reusing the other
  child. An immediate stale read excludes affected ancestors. Reverting source inputs
  reuses three cached results without a call, even when child prose remained identical
  across the source edit. Unselected source changes require no generation.
- Model/deployment changes rekey inputs; return to the prior configuration reuses cached
  completions. Recipe changes and primary-source reassignment cannot reuse the wrong
  node/area identity. If an evicted child input is regenerated with different prose,
  parents regenerate even after its original source input identity returns.
- Exact final source bytes accompany derived orientation. Oversized source/request
  inputs and run budgets refuse before a provider call; smaller explicit spans fit.
  Strict claim/citation/identifier checks refuse invalid model output before publication.
- Malformed replies, a wrong response model, incomplete/oversized output, HTTP 429 and
  redirects refuse without leaking response text. Persistent backoff suppresses retries
  after restart. Deadline cancellation closes the client request; absent usage stays
  unknown. Observed prompt/completion totals remain separate from o200k admission counts.
- Seventy-two retained outputs exercise bounded cache storage and transactional counts.
  Lowering the cap from 2 MiB to 1 MiB evicts history while all current briefs still replay
  with zero calls. Cache purge preserves current briefs. Forget removes a selected child,
  generated ancestors and cached copies in the same transaction, preserving user memory.
- A source edit during generation refuses the stale result. Faults and actual child-process
  aborts immediately before/after brief commit prove the cache and live brief publish
  atomically; restart performs four or three calls respectively. Schema-9 upgrade is
  explicit, preserves sources and remains wholly old after a pre-commit failure.

The full regression run plus the final affected-suite rerun passed **957 regular tests**
(two intentional measurement ignores) and **all 20 existing child-process recovery cases**.
The first full run found one old schema-9 upgrade target assertion; after updating it,
all 189 library tests, 18 brief tests and 14 knowledge tests passed. Other successful
regression suites were retained. The 6,000-file watcher suite passed in 84.37 seconds.
The new knowledge suite additionally runs two real process-abort boundaries internally.
Final all-target Clippy with warnings denied, formatting, the build without default
features and Markdown link checks passed (47 files, 719 links, 165 heading anchors).

No actual generation model was selected or called, and no task-quality, billing or
throughput improvement is inferred. HTTP cannot attest deployed weights or stop external
computation after cancellation. This CLI requires exclusive store ownership; continuous
MCP knowledge scheduling, shared foreground/device admission and an owned isolated local
generator remain open. Preparation scans a bounded selected plan, not a corpus-wide
reverse-dependency index. Knowledge acknowledgments describe that selected profile only.

## Continuous MCP knowledge maintenance

010 T002 now composes the existing generation pipeline inside the MCP owner. An
explicit profile enables generation controls and derived context; `maintain` or the
launch flag enables ongoing work. Source snapshot/acceptance steps use the shared
foreground-prioritized engine slot. JSON construction, tokenization and HTTP hold no
engine slot or source transaction. Schema 10 remains unchanged.

Executed checks use synthetic HTTP completions and embedding vectors, with real Rust
providers/transports, source storage, generation cache, native watching, lexical/dense
indexes, context packing and MCP SDK consumers:

- Real stdio watching/profile admission alone sends zero requests. Explicit maintenance
  generates four source/area nodes, an edit generates the changed child and two ancestors,
  and revert/restart reuse sends zero further requests. Context delivers labeled derived
  orientation together with the original implementation under the same token allowance.
- A gated HTTP call leaves foreground context and watcher source acceptance available.
  Editing a cited source during that call rejects its completion, then regenerates only
  the missing child and ancestors. Accepted transitive dependencies use the new full hash.
- Pause during a call accepts that one current result and admits no next call. Resume
  preserves it. An eight-call window stops a nine-node plan; rapid pause/resume cannot
  reset the allowance, and a fresh owner generates only the ninth missing result.
  Invalid output on the eighth call pauses explicitly even when a later node exhausts
  the window, preventing an unintended provider retry when a new window opens.
- A quiet driver waiting for a missing source performs no store steps across repeated
  notification timeouts. A prepare action wakes the waiting driver without a source edit.
  Publication preserves unrelated pending source indexing rather than draining the corpus.
- Shared admission excludes generation and embedding requests. A selected embedding batch
  waits for generation to release admission and then completes all 12 fixture inputs.
  Dispatched/waiting dense queries get priority; a late embedding call still blocks new
  generation. These exercise the actual runtime slot, not a mock scheduling counter.
- Shutdown releases the owner store in under two seconds while a synthetic external service
  still takes four seconds to answer; the uncommitted result is absent and work stays pending.
  This proves client cancellation and store lifecycle, not termination of external compute.
- A shared HTTP owner returns primary-root derived context alongside reference-root headers;
  excluding the primary refuses derived delivery. Both transports preserve exact source
  recovery. The HTTP teardown fixture waits for final SDK ownership to release the store
  after its completion notification, rather than assuming those two events are atomic.

The full regression plus final affected-suite reruns passed **967 regular tests**
(two intentional measurement ignores) and **all 20 child-process recovery cases**.
The full run passed 966 tests; the final library/knowledge rerun passed all 192 library
and 21 knowledge tests, including the added mixed provider-failure/window-exhaustion case.
The 6,000-file watcher suite passed in 127.20 seconds; this is a fixture result, not a
throughput claim. A build without default features exposed a watcher admission method
that was unnecessarily gated on embeddings; after removing that gate, the build and all
21 knowledge integration tests also passed under a `--no-default-features` invocation.
Correction recorded 2026-10-10: the self dev-dependency re-enables semantic in Cargo test,
so that invocation alone does not establish disabled-feature runtime coverage. Final all-target
Clippy with warnings denied, formatting and Markdown link checks passed (47 files,
721 links, 166 heading anchors). No schema migration or dependency changes were added.

The fixtures establish lifecycle, bounded admission, reuse, provenance and foreground
availability. They do not establish real-model quality, server CPU/GPU headroom, weights,
provider billing or large-repository throughput. Usage is reported at completed-pass
boundaries and is owner-local; a process restart is not a durable account-wide quota.
Actual isolated local generation and hardware resource enforcement remain open.

## Owned local generation and resource controls

010 T002 adds an explicit owned decoder to the existing CLI/MCP pipeline, with lazy
pinned artifacts, per-worker CPU/RAM/device limits and model unload/reuse. Schema 10
and existing HTTP cache identities are unchanged. This is experimental Linux CPU work
plus an unexecuted macOS App Sandbox/Metal path; it is not installed-package acceptance.

The native lifecycle fixtures use synthetic completions behind the **actual Rust worker
entry, supervisor, seccomp and OS resource limits**. They exercise:

- Denied access to an existing private file, socket creation, subprocess execution and
  `fcntl` ownership that could request asynchronous signals to another process,
  while model-descriptor reads and in-process threads remain available. The child sees
  only the explicit environment. CPU affinity, hard address-space/lifetime CPU limits
  and `no_new_privs` are inspected on the live process through MCP status.
- Actual address-space allocation refusal and CPU-time termination preserve pending
  source work and publish no output. Unsupported GPU/memory combinations refuse before
  reading an absent model, rather than falling back to unrestricted execution.
- MCP startup/profile admission alone launches nothing. A four-node pass uses one
  worker, idle unload removes its PID, and a changed child plus two ancestors reload
  from retained verified snapshots even after the original artifact paths are removed.
  A fresh owner with current briefs performs zero calls/launches without those paths.
- A blocked native call leaves foreground exact-source context available. Shutdown
  terminates/reaps the child before store release and leaves no uncommitted brief.
  Abrupt owner death stops computation; reopening recovers the retained obligation.
  The container may retain an orphan zombie under PID 1, so the death check distinguishes
  a stopped zombie from a live process; normal shutdown separately requires reaping.
- Oversized frames, wrong reply identity and token-usage overflow kill the worker and
  refuse publication. Bad artifact pins fail before a call and acquisition failure is
  memoized across selected nodes; a fresh explicit session can retry repaired artifacts.

The cloud instance is Linux x86_64, kernel 6.18.44, five available CPUs and no GPU.
Landlock reports unavailable and cgroup v2 is not delegated. The implemented CPU profile
therefore needs neither: a seccomp allowlist denies all new path opens, and the pinned
loader consumes a read-only `FILE*` model descriptor. This profile was exercised here;
aarch64, macOS filesystem/process denials, Metal load/inference, GPU pressure and signed
distribution remain unexecuted. Sampled footprint/Metal allocation is not a hard quota.

The opt-in real CPU smoke uses upstream
`Qwen/Qwen2.5-Coder-0.5B-Instruct-GGUF/qwen2.5-coder-0.5b-instruct-q4_k_m.gguf`, SHA-256
`1d9614638d18024d0fbb36575a15f1302a3adf044df10345688ec4f6e1c4ff32`, kept outside Git.
The worker is built from llama.cpp `b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea`, using the
explicit `chatml` template, 4,096 context tokens, batch 128, two CPU threads, 2 GiB hard
address space and 180 lifetime CPU seconds. The ignored integration test requires explicit
worker/GGUF paths and never downloads weights. Its input is a tiny synthetic Rust function;
it does not establish coding-task quality, corpus throughput or economic improvement.

Early smoke attempts revealed two separate model-output problems: generic JSON decoding
allowed incorrect brief field types, and the small model emitted unsupported identifiers
and misleading prose. The worker now constrains the brief structure with a grammar; source
acceptance continues to reject unsupported identifiers. The bounded smoke profile asks for
two short claims and empty optional arrays. Such integration evidence does not select this
model for production, make citations proof of truth or solve model quality. Terminal usage
from successful calls is actual GGUF token usage; interrupted calls remain unknown.

The final real CPU run passed: one accepted/cached brief, **377 prompt / 124 completion tokens**,
one launch, zero unknown-usage calls, and 725,540,864 bytes sampled peak worker RSS under
the 2 GiB virtual-address ceiling. The test's 40.90 seconds include hash/copy/setup work;
there is no throughput comparison. Replaying with an absent model path used zero calls
and zero launches. All nine new native fixture tests also passed. The first abrupt-death
fixture assumed the optional procfs child-list file; matching ordinary process PPid
instead made it portable to this kernel without weakening the stopped-process assertion.

The full regression executed **975 passes and one timeout**, with three intentional ignores
(two existing measurements and the separately executed real-model smoke), plus **all 20
child-process recovery cases**. The existing
`continuous_semantics_progresses_during_filesystem_traffic_and_converges` case exceeded its
12-second progress deadline. It passed unchanged in isolation in 2.99 seconds, then the
entire affected neural-retrieval suite passed **70/70** in 45.19 seconds with its measurement
case still ignored. Thus all **976 regular tests** have passing evidence across the full
run and affected-suite rerun; the one transient timeout remains recorded rather than
reported as an uninterrupted green run. No test deadlines or production scheduling were
changed to obtain the rerun. The 6,000-file watcher suite passed in 109.53 seconds.

The no-default-features build and a 30-test knowledge invocation also passed. The self
dev-dependency re-enables semantic in Cargo test, so the latter does not establish
disabled-feature integration; the shared-admission checks below exercise those binaries
directly. Native worker
build and Clippy, all-target Clippy with warnings denied, formatting, bundle-script syntax
and Markdown link checks passed (47 files, 729 links, 172 heading anchors). No dependencies,
schema changes or weights were added. The cloud setup draft records the additional pinned
CMake/libclang prerequisites and optional native-worker activation; publication/fresh-task
restoration is not claimed by these current-instance checks.

## Shared model memory admission — 2026-10-10

010 T002 / 009 maintenance now share opt-in worker-ceiling reservations and observed
memory headroom under the MCP owner. The policy has no schema, dependency or model/cache
identity change. Tests exercise the public `--model-resources` option, native synthetic
generation worker, source store and MCP status/context/pause/shutdown paths. The embedding
provider and deterministic pressure sampler are fixtures; combined real-model performance
and macOS memory/Metal behavior are not claimed.

New checks cover combined ceilings, refusal before embedding startup, retention through
provider shutdown, headroom for loading, pressure hysteresis, unavailable observations,
and status reads that do not sample. Background embedding waits under pressure while a
foreground query succeeds, then commits all 12 fixture rows after recovery. A synthetic
sleeping child verifies that idle eviction reaps before releasing capacity and preserves
artifact descriptors; existing native denial/resource tests retain their separate scope.

CLI/MCP fixtures verify generation reservation while resident, zero reservation after idle
unload, current-brief replay under pressure with missing artifacts and zero calls/launches,
zero usage on pressure/budget refusal, foreground source access, responsive pause and durable
pending work. Restarting after budget refusal completes the four-node plan.

The initial real-container checks exposed an inherited cgroup mount whose root is `/..`,
while `/proc/self/cgroup` names `/`. Treating that as an ordinary path correctly refused
observation but prevented admission. A bounded lookup of the owner's live PID now locates
the actual `init` group and includes its visible ancestor limits. A regression fixture
checks this namespace case alongside nested `memory.high`/`memory.max` limits and malformed
observations. No cgroup files or host settings were changed.

Admission is conservative rather than an OS quota: worker ceilings are reserved, while
host/container availability is sampled. In-flight work keeps existing native limits;
embedding residency stays available for queries. Owner/index memory, artifact storage,
other owners/processes and hidden cgroup ancestors are outside the reservation sum.
CPU utilization, global GPU pressure, thermal/battery policy and live macOS validation
remain pending in the [inventory](context-ecosystem-plan.md#pending-work-inventory--2026-10-10).

Affected suites have passing evidence for **392 tests**: 199 library, 33 knowledge,
90 MCP and 70 neural-retrieval tests, including ten new tests. The real-model generation
smoke and semantic measurement remain intentionally ignored in this run. After the cgroup
fix, one rerun of the existing 80 ms HTTP deadline fixture expired before call admission
(zero unknown-usage calls versus the fixture's expected one); the full knowledge suite
then passed unchanged in 34.88 seconds. That transient fixture failure is retained here;
no production/test deadlines were relaxed. The existing filesystem-traffic semantic case
passed in the 70-test affected-suite run. This is focused verification, not a claim that
the entire repository suite was rerun for this increment.

The core and synthetic generator additionally built with `--no-default-features
--features test-faults`. Invoking the existing knowledge test harness directly against
those rebuilt executables passed all three new CLI/MCP memory-policy cases in 6.80 seconds,
without Cargo rebuilding/re-enabling semantic. All-target Clippy with warnings denied,
formatting and relative Markdown link checks passed (47 files, 741 links, 181 anchors).
No real-model, macOS/GPU or large-repository performance experiment ran for this increment.
