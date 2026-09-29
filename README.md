# Context Foundry

A local context engine for coding agents, written in Rust. It combines source
search, imported code relationships, and cited context under an explicit token
budget. The revised plan includes Foundry-owned Rust learning in isolated workers,
explicit repository bootstrap, adapter budgets and optional request forwarding/metering. Ordinary retrieval
requires no model or GPU. Laya is a research reference, not the target runtime.

**Status: working first slice, not a production replacement.** Large-codebase
performance, agent task improvement, and dollar savings have not been established.
The architecture targets those goals without making research a prerequisite for
using the implemented CLI. There is no published release or remote repository yet.

## Try it

Rust 1.90 or newer is required. From this checkout:

```sh
cargo build --locked --release
cargo run --locked -- --store /tmp/foundry-demo index ./examples/workspace
cargo run --locked -- --store /tmp/foundry-demo search parse_record
cargo run --locked -- --store /tmp/foundry-demo context parse_record --tokens 1024
cargo run --locked -- --store /tmp/foundry-demo status
```

Use a fresh store path for the demo. A store binds to one canonical workspace path.
After editing, adding or deleting files, run `index` again. There is no watcher yet.
To install the executable locally: `cargo install --path . --locked`.

## What works

- Incremental source replacement and deletion, with atomic content/hash/index-work
  commits in redb. Interrupted search indexing resumes with `refresh`.
- Tantivy lexical search and exact-path boosting. Stale index candidates are checked
  against the committed source version before delivery.
- Hash-checked graph bundle imports, isolated by producer, with forward and reverse
  file-neighborhood traversal. Traversal limits do not delete accepted relationships.
- Verbatim source spans, citations, hashes and omission notices packed using the
  declared `o200k_base` tokenizer. The budget covers **context stdout**, including
  its metadata. It does not cover a host's added envelope, stderr or another model's
  tokenizer, and is not a dollar-savings measurement.
- Legacy prototype Laya HTTP strategy selection, bounded to two seconds, with a named
  deterministic fallback. This remains implemented but is superseded by the owned
  learning plan; current feedback exports do not constitute a working trainer.

```sh
foundry --store /tmp/foundry-demo import-graph examples/graph.json
foundry --store /tmp/foundry-demo graph src/main.rs --depth 1
foundry --store /tmp/foundry-demo context 'who calls parse_record' --tokens 1024
```

## Current limits

This is a single-process CLI: each command owns its store exclusively. It is not
yet a multi-client daemon, MCP server, compiler indexer, vector engine, or memory
service. Graph symbol labels and evidence classes are supplied by the importer;
the implementation traverses files, not a resolved symbol graph.

Results describe an **indexed snapshot**, not verified current filesystem bytes.
Index again after edits. Hidden files, ignored paths, `target`, `node_modules`,
symlinks, non-UTF-8 files and files over 2 MiB are excluded. Read errors are reported
and defer missing-file deletion. This is not a sandbox for a hostile workspace
that changes paths concurrently, and the narrow sensitive-file deny rules are not
a general secret/PII scrubber.

Search examines at most 256 candidates; graph traversal examines at most 256
edges and visits at most 64 files. Whole-producer graph replacement is capped at
100,000 edges. Ingestion maintains an in-memory path set. These are explicit
first-slice limits, not evidence of million-file scale.

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
