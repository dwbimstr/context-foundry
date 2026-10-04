# Context Foundry

A local context engine for coding agents, written in Rust. It combines source
search, imported code relationships, and cited context under an explicit token
budget. The revised plan includes Foundry-owned Rust learning in isolated workers,
explicit repository bootstrap, adapter budgets and optional request forwarding/metering. Ordinary retrieval
requires no model or GPU. Laya is a research reference, not the target runtime.

**Status: 001 (T001–T006) and 003 T001–T003 verified locally; 003 T005 and 007 T001
accepted and committed locally on 2026-10-04 (unpushed); nothing released.** Reliable
cited CLI context, explicit bootstrap and agent access over MCP (stdio, plus an opt-in
shared owner for concurrent hosts) work on macOS arm64; see [validation](docs/validation.md).
Large-codebase performance, agent task improvement, and dollar savings have not been
established. There is no published release yet.

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

- Explicit store creation, schema-2 upgrade (`upgrade-store --to 2`), bounded
  refresh and explicit `repair-index` with one retained quarantine. Reads never create,
  upgrade or repair a store; a broken lexical index leaves status and retrieve usable.
  An upgraded store, or one indexed before search schema 3 (leading-run units),
  needs `repair-index` before search and context work.
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
- Five MCP tools (`search`, `context`, `retrieve`, `index`, `status`) through the rmcp
  SDK: one active engine operation and zero queued, bounded frames/bodies/handlers,
  named `busy`/deadline/cancellation errors, delivery budgets and usage receipts.
- Hash-checked graph bundle imports with bounded file-neighborhood traversal; graph
  failures degrade context to source evidence with a named reason.
- Multi-root context (007): `--reference ROOT=STORE` (up to 8) admits outside
  repositories at launch; `search`/`context` merge them with per-root coverage and
  handles, and an unavailable root is named rather than silently skipped.
- `foundry usage import --host omp|codex --session FILE`: offline counter summary of a
  host session (provider usage, tool calls and Foundry payloads); opens no store.

```sh
foundry --store /tmp/foundry-demo import-graph examples/graph.json
foundry --store /tmp/foundry-demo graph src/main.rs --depth 1
foundry --store /tmp/foundry-demo context 'who calls parse_record' --tokens 1024
```

## Current limits

One store has one owner: a CLI command, a stdio MCP session or the explicit shared
HTTP owner. Others get `store_busy`. It is not a daemon fleet, compiler indexer,
vector engine or memory service. Graph symbol labels and evidence classes are supplied
by the importer; the implementation traverses files, not a resolved symbol graph.
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
100,000 edges. Reconciliation pages source keys (128) and bounds diagnostic samples;
these are explicit limits, not evidence of million-file scale.

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
