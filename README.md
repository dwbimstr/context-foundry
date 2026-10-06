# Context Foundry

A local context engine for coding agents, written in Rust. It combines source
search, imported code relationships, and cited context under an explicit token
budget. The revised plan includes Foundry-owned Rust learning in isolated workers,
explicit repository bootstrap, adapter budgets and optional request forwarding/metering. Ordinary retrieval
requires no model or GPU. Laya is a research reference, not the target runtime.

**Status: 001, 003, 005, 007 and 008 complete; 009 T001–T003 and 013 T001–T003 accepted
(2026-10-05), with model workers under development isolation only. Open: the deferred
measurement phase, 013 T004 and signing. Pushed to `main`; nothing released.** Reliable cited CLI context,
explicit bootstrap and agent access over MCP (stdio, plus an opt-in shared owner for
concurrent hosts) work on macOS arm64; see [validation](docs/validation.md).
Agent task improvement and dollar savings have not been established. There is no
published release yet.

## Try it

Rust 1.90 or newer is required. From this checkout:

```sh
cargo build --locked --release
cargo run --locked -- --store /tmp/foundry-demo index ./examples/workspace
cargo run --locked -- --store /tmp/foundry-demo search parse_record
cargo run --locked -- --store /tmp/foundry-demo context parse_record --tokens 1024
cargo run --locked -- --store /tmp/foundry-demo status
```

`index` is the only command that creates a store; reads on a missing store fail with
`store_not_found`. A store binds to one canonical workspace path. After editing,
adding or deleting files, run `index` again. There is no watcher. To install the
executable locally: `cargo install --path . --locked`.

Agent setup for a repository (prints configuration; edits nothing unless asked):

```sh
foundry bootstrap --root ~/src/repo            # inspect: writes nothing
foundry bootstrap --root ~/src/repo --apply    # create the store, index baseline source
foundry connect --host omp --root ~/src/repo --print-config     # or --host codex
foundry --store ~/src/repo/.context-foundry mcp --root ~/src/repo   # stdio MCP server
```

For concurrent hosts on one repository, run one explicit shared owner and print
matching host configuration with `--http-port PORT --token-env NAME`:
`foundry --store DIR mcp --root ROOT --transport streamable-http --bind 127.0.0.1:PORT --auth-token-env NAME`.

## What works

- Explicit store creation, schema-6 upgrade (`upgrade-store --to 6`, from v1–v5), bounded
  refresh and explicit `repair-index` with one retained quarantine. Reads never create,
  upgrade or repair a store; a broken lexical index leaves status and retrieve usable.
  An upgraded store, or one indexed before search schema 3 (leading-run units),
  needs `repair-index` before search and context work.
- Owned learning data (013 T001, `semantic` feature): `feedback v4` records exact, permitted
  examples through the trusted operator CLI only (MCP can never grant training consent);
  `learning prepare --out DIR --policy FILE [--parent MANIFEST]` freezes an immutable
  schema-4 grouped, tokenized dataset through the exact pinned renderer, and
  `learning check --manifest FILE --policy FILE` verifies it read-back, re-rendering every
  row through the policy's pinned tokenizer (a missing tokenizer is a named refusal).
  `learning compose-state --query Q` prints the core's composed `state` (the query,
  `graph: <complete|partial>` from the current compiler graph, and up to three lexical
  locator lines) so rows are built from the one composer; it refuses `graph_unavailable`
  or `graph_stale` when no current graph exists. No model is loaded and nothing is
  trained; legacy `feedback` rows stay exportable and ineligible.
- Paged, held-root reconciliation: symlink/FIFO/root replacement is refused, failures
  defer absence deletion, and source revision/scan IDs are checked counters.
- Syntax-unit search (Rust, Python, TS/TSX/JS, Go, C/C++, Java, Markdown) with exact
  definitions first, a `path` filter and a per-file cap; a unit includes its leading
  doc comments and Rust attributes. Every emitted span is verified against the
  committed source in one final read transaction.
- v2 source handles, `retrieve` with `lines`, continuation and an outline view, and
  context packing (verbatim, signature or outline forms) whose exact
  emitted bytes are counted with `o200k_base` (complete CLI stdout including its
  trailing LF, or the final MCP text block). The serialized MCP result is separately
  byte-capped. Not a host-envelope or dollar-savings measurement.
- Seven MCP tools (`search`, `context`, `retrieve`, `index`, `status`, `memory`, `references`) through the rmcp
  SDK: one active engine operation and zero queued, bounded frames/bodies/handlers,
  named `busy`/deadline/cancellation errors, delivery budgets and usage receipts.
- Explicit project memory (008): `foundry memory put|update|get|forget|search|export`
  and the MCP `memory` tool. Records are attributed and revisioned, and conflicting
  writes are refused. Source links report fresh, stale or missing. `context
  include_memory` adds compact `mem:` lines; forget is logical; memory never feeds
  training export. `mcp --no-memory` hides the tool.
- Hash-checked graph bundle imports with bounded file-neighborhood traversal; graph
  failures degrade context to source evidence with a named reason.
- Compiler references (005): `foundry import-scip --index FILE --snapshot MANIFEST`
  imports a rust-analyzer SCIP artifact bound by its manifest to the indexed source
  revision and hashes. It decodes in bounded batches, publishes per source and records
  coverage. `foundry references` answers from a symbol ID or a handle plus byte offset,
  paginating with `next: after=`. Facts become stale when any indexed source changes,
  until a fresh artifact is imported. Import never runs the producer. Over MCP, the
  `references` tool answers the same queries, `index {scip: {index_file,
  snapshot_file}}` imports files staged in `<store>/imports`, and `context` with the
  graph strategy adds the units enclosing a resolved symbol's references.
- Multi-root context (007): `--reference ROOT=STORE` (up to 8) admits outside
  repositories at launch; `search`/`context` merge them with per-root coverage and
  handles, and an unavailable root is named rather than silently skipped.
- Semantic preparation (009 T001): `foundry semantic prepare --profile FILE
  --budget-seconds N`, `semantic status` and `semantic purge`. Sources are partitioned
  into embedding units of at most 1024 tokens and tokenized in Rust. Vectors are cached
  by document function and exact input, and a USearch F16 index is built from the cache.
  Unchanged inputs are never re-embedded across restarts, repairs or profile rollback.
  Model execution needs an accepted isolation profile. Until signing and package
  acceptance close it runs only with `--development-isolation`, in an ad-hoc App
  Sandbox worker; otherwise it is `isolation_unavailable`. Semantic search delivery is
  009 T002.
- Progressive semantic preparation (009 T003): an MCP owner started with
  `--semantic-profile` prepares its primary root in the background after
  `index {semantic: "prepare"}` and stops admitting batches after
  `index {semantic: "pause"}`. Source, search, context and `status` keep working
  meanwhile; context uses the coverage that has arrived and says `partial` or
  `ready`. Queries and document batches share the one resident worker and one
  model slot, and nothing is queued behind it. `status` adds a `semantic` object.
  Startup never resumes preparation.
- `foundry usage import --host omp|codex --session FILE`: offline counter summary of a
  host session (provider usage, tool calls and Foundry payloads); opens no store.
- Owned model gateway (003 T004): `foundry gateway --config FILE` meters OMP 18.6.0
  traffic to Z.ai `glm-5.3-flash`. It is a single-flight loopback forwarder that
  validates requests against the pinned OMP request profile, forwards bytes unchanged,
  withholds upstream error events and records per-attempt usage receipts.
  `foundry gateway-omp` runs OMP in a fresh dedicated profile, and only the gateway
  ever holds the Z.ai key. Meter mode only; no spend cap.

```sh
foundry --store /tmp/foundry-demo import-graph examples/graph.json
foundry --store /tmp/foundry-demo graph src/main.rs --depth 1
foundry --store /tmp/foundry-demo context 'who calls parse_record' --tokens 1024
```

## Current limits

One store has one owner: a CLI command, a stdio MCP session or the explicit shared
HTTP owner. Others get `store_busy`. It is not a daemon fleet, compiler indexer,
vector engine or memory service. Imported graph-bundle labels are supplied by the
importer; compiler references come only from an imported SCIP artifact, never from a
compiler run by Foundry.
Routing is deterministic (`auto`/`search`/`graph`); the legacy Laya client is no
longer reachable from the CLI.

Results describe an **indexed snapshot**, not verified current filesystem bytes.
Index again after edits. Hidden files, ignored paths, `target`, `node_modules`,
symlinks, non-UTF-8 files and files over 2 MiB are excluded. Read errors are reported
and defer missing-file deletion. Bytes are read relative to the held root, but names
are enumerated by path: a process that swaps and restores a directory during one
scan can make that scan retire records of files that still exist (the next scan
restores them). The narrow sensitive-file deny rules are not a secret/PII scrubber.

Search examines at most 256 candidates; graph traversal examines at most 256
edges and visits at most 64 files. Whole-producer graph replacement is capped at
100,000 edges. `references` examines at most 256 records and visits at most 64
files per call; a SCIP import accepts artifacts up to 1 GiB, documents up to 8 MiB
and 16,384 occurrences, and uses at most 4 GiB of scratch. Reconciliation pages
source keys (128) and bounds diagnostic samples; these are explicit limits, not
evidence of million-file scale.

## Design and development

- [Product direction and workflow specs](specs/README.md)
- [Modified Spec Kit workflow and commands](.specify/README.md)
- [Subtraction review and removed complexity](docs/review/subtraction.md)
- [Extraction strategy](docs/extraction-plan.md) and [all 71 predecessor dispositions](docs/extraction-map.md)
- [Architecture and alternatives](docs/architecture.md)
- [Owned learning and continuous fine-tuning](docs/learning.md)
- [Bootstrap, isolation and deployment](docs/deployment.md)
- [Adapter token economics](specs/003-agent-retrieval-context/contracts/adapter-economics.md)
- [Graph bundle format](docs/graph.md)
- [Release scope and next steps](docs/roadmap.md)
- [Design lessons](docs/review/README.md)
- [Security and data handling](SECURITY.md)
- [Contributing](CONTRIBUTING.md)
- [Validation and proof limits](docs/validation.md)
- [Dependencies and licenses](docs/dependencies.md)

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

The software is [MIT licensed](LICENSE). Dependencies retain their own licenses.
No predecessor runtime, private specifications, session transcripts, datasets or
model weights are included. Third-party library/runtime and each model/dataset's terms
are independent of this repository's license.
