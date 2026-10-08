# Context Foundry

[![CI](https://github.com/dwbimstr/context-foundry/actions/workflows/rust.yml/badge.svg)](https://github.com/dwbimstr/context-foundry/actions/workflows/rust.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV 1.90](https://img.shields.io/badge/MSRV-1.90-orange.svg)](Cargo.toml)

A local, deterministic context engine for coding agents, written in Rust. Name an
identifier and Context Foundry answers with its definition, a short directory of its
namesakes, or the places that use it, delivered as cited source under an exact token
budget. It serves agents over MCP and the same answers on the command line. Core
retrieval needs no model, GPU, network or provider key.

**Pre-release.** Everything below runs from a checkout of `main`; there is no published
release yet. See [Status](#status).

## The problem

A coding agent spends calls and tokens finding code before it can change it. Grep
returns a list of locations, and the agent then reads files to find the one definition
it wanted. In a large repository a name is often defined many times, and a question
like "who calls `parse_record`" takes several more rounds of searching and reading.
Each round adds tokens to the agent's context.

Context Foundry indexes a repository once and answers by name:

- **Definition.** A name that resolves to one definition returns its source in the
  first form that fits the budget: verbatim, signature, or a one-line address.
- **Namesakes.** An ambiguous name returns a short directory of the definitions that
  share it, with their addresses, so the next call can be exact.
- **Doors.** "Who calls/uses/references X" returns one line per file that uses it,
  exact when compiler references were imported, otherwise marked `[approx]`.

Every item carries a handle (`path#start-end@sha32.ws16`) that `retrieve` resolves to
the exact bytes, and every span is checked against the indexed source before it is
sent.

## Who it is for

- Developers who run coding agents (OMP and Codex are the hosts exercised so far) on
  local repositories, from small projects up to rust-lang/rust.
- Teams that want context they can audit: cited spans, deterministic results, a token
  count of exactly what was delivered.
- Anyone who needs retrieval to stay on the machine: no telemetry, no hosted service.

It is **not**:

- a hosted or multi-user service (one store has one owner; there is no user
  authentication or tenant isolation);
- a file watcher (index again after edits);
- a compiler or code-intelligence server (it imports compiler references, it never
  runs a compiler);
- a vector database, general memory service or secret scrubber;
- available as a signed binary or on Windows today.

## How it works

1. **Index.** `foundry index ROOT` reads the workspace, respecting ignore files and
   never executing repository code, into a local store (redb for durable state,
   Tantivy for search). tree-sitter splits source into syntax units (definitions with
   their leading doc comments) in 23 languages: Rust, Python, TypeScript/TSX,
   JavaScript, Go, C, C++, Java, C#, F#, VB.NET, PHP, Perl, shell, PowerShell, Ruby,
   Kotlin, Swift, Scala, Lua, Dart, Elixir and Haskell. Markdown is split into heading
   sections; other UTF-8 text is indexed as plain blocks. Parsing runs on up to 8
   threads, and every parse has a deterministic work budget.
2. **Addresses and anchors.** Every definition gets an address from its path and the
   names that enclose it: a `find` method in `impl UnionFind<Key>` in `src/dsu.rs` is
   addressed `src dsu unionfind`, so `` `UnionFind::find` `` or `` `dsu::find` `` can
   pick it out among other `find`s. Identifiers in a query, such as `` `sleep_ms` ``,
   `toolSession` or `a::b`, become up to four anchors; the resolver picks a definition
   or lists up to 16 namesakes. This is the "city map" of
   [context-v2 § City map](specs/001-source-state-recovery/contracts/context-v2.md#city-map).
3. **Doors.** A usage question gets door lines for its first anchor: exact ones from an
   imported [SCIP](https://github.com/scip-code/scip) artifact (rust-analyzer today),
   otherwise approximate ones from identifiers and per-file import keys.
4. **Budgeted packing.** Each candidate takes the first form that fits the remaining
   budget. The budget is the exact `o200k_base` token count of the bytes emitted (CLI
   stdout or the MCP text block), and CLI and MCP emit the same text.
5. **Memory and multiple roots.** Explicit project memory records can be added to
   context. One MCP owner can admit up to 8 other repositories and answer across all
   of them in one budgeted response.

## Status

"Stable core" means implemented, cross-lab reviewed, gated and merged on `main`.
Nothing is released, so no compatibility promise is made yet; store and index
upgrades are explicit (`upgrade-store`, `repair-index`). Per-task evidence is in
[validation](docs/validation.md).

| Area | Maturity | Notes |
| --- | --- | --- |
| Indexing, recovery, repair | Stable core | Explicit store creation, bounded refresh, interrupted-index recovery, `repair-index`, `upgrade-store --to 6` |
| `search`, `context`, `retrieve` | Stable core | City map, doors, exact budgets; CLI and MCP |
| MCP server | Stable core | Seven tools over stdio, or one shared loopback HTTP owner |
| `bootstrap`, `connect` | Stable core | Configuration printed for OMP and Codex |
| Project memory | Stable core | `foundry memory …` and the MCP `memory` tool |
| Compiler references (SCIP) | Stable core | `import-scip` of rust-analyzer artifacts, `references`, exact doors |
| Multi-root context | Stable core | `mcp --reference ROOT=STORE`, up to 8 |
| Semantic retrieval | Optional, off by default | Implemented; enabled only after the [measurement handoff](docs/measurement-handoff.md) and signing pass |
| Owned learning | Frozen off | Implemented commands remain as research tooling; not packaged or advertised |
| Model gateway | Meter only | One pinned host profile (OMP 18.6.0 with Z.ai `glm-5.3-flash`); no spend cap |

**Not released yet:**

- no tagged release, prebuilt binary or crates.io package (`publish = false`);
- the macOS package is not signed; Developer ID signing and notarization come last;
- semantic retrieval is not advertised until its measurements pass;
- index time and memory after parallel indexing are not yet measured.

The withdrawn local `v0.1.0` tag of 2026-10-04 is not a release.

## Measured results

G1 asks whether an agent that names an identifier gets the evidence in fewer calls
and tokens. A scripted agent (at most 4 MCP calls, no model) answers frozen tasks on
rust-lang/rust 1.99.0 (`checker`; `qualified`, the same names written `module::name`)
and on a Bun monorepo (`bun`); a scripted grep agent is the comparison. Pass@1 means
the first call delivered the evidence: the definition's name line inside a delivered
body, or usage lines in at least min(3, files) files. Result on 2026-10-08, against the
Foundry build before the city map:

| Set | Pass@1 (baseline → city map) | Tokens |
| --- | --- | --- |
| checker | D1 89.1 → 97.8%, D2 82.1 → 98.0%, D3 77.1 → 92.8%, U1 8.2 → 86.6% | median to pass 806 → 234 (grep 682); mean 2,056 → 1,440 |
| qualified | D4 45.7 → 94.0%, U2 8.0 → 81.2% | median 2,017 → 234 (grep 674); mean 2,313 → 1,153 |
| bun | B1 93.1 → 99.1%, B2 16.2 → 96.8% | median 1,994 → 184 (grep 647); mean 1,718 → 455 |

D1–D3 ask for a definition named in backticks whose name has 1, 2–4 or 5–16
definitions, and U1 for the uses of a unique name; D4 and U2 are the qualified
definitions and uses; B1 and B2 are unique-name definitions and uses. "Median to
pass" is the median tokens to pass of unique-name definition tasks (the grep agent's
in parentheses); "mean" is the mean tokens per task. Every regression guard also
passed. Source:
[validation § G1 city-map verdict](docs/validation.md#g1-city-map-verdict--2026-10-08-pass);
targets and baselines: [001 § G1](specs/001-source-state-recovery/spec.md#g1--city-map-measurement).

What this does not show: the tasks, apparatus and results are private and outside
Git; the agent is scripted, so this measures delivered evidence and tokens, not task
success or cost with a real model.

## Requirements and platform support

- **Rust 1.90 or newer** (edition 2024) and a C/C++ compiler: the grammars and the
  USearch index are compiled from C and C++ sources (Xcode command-line tools on macOS).
- **Network on the first build:** Cargo fetches crates and three grammar forks pinned
  by git revision in `Cargo.toml` (`[patch.crates-io]`, Ruby, Perl and Rust, under
  `github.com/dwbimstr`). See [dependencies](docs/dependencies.md).
- **Platforms:** macOS arm64 is where everything is developed, gated and measured. CI
  builds and tests the core on Linux and macOS. The installable package, the embedding
  worker (Metal) and worker isolation are macOS arm64 only. Windows has not been built
  or tested.

## Install from source

```sh
git clone https://github.com/dwbimstr/context-foundry
cd context-foundry
cargo install --locked --path .        # installs `foundry` into ~/.cargo/bin
# or build without installing:
cargo build --release --locked         # target/release/foundry
```

`--no-default-features` builds the lexical-only core without the semantic feature.

### Install a package (macOS arm64, unsigned)

`scripts/package.sh` builds a versioned package and `scripts/install.sh` installs it
under a prefix, with upgrade, rollback and uninstall. The package is not signed yet.
`package.sh` builds offline, so fetch dependencies first.

```sh
cargo fetch --locked
scripts/package.sh --out dist
scripts/install.sh install --package dist/context-foundry-0.1.0-macos-arm64.tar.gz --prefix ~/.local
scripts/install.sh upgrade --package NEWER.tar.gz --prefix ~/.local --store ~/src/repo/.context-foundry
scripts/install.sh rollback --prefix ~/.local
scripts/install.sh uninstall --prefix ~/.local
```

Every file is checked against the package manifest; versions sit side by side and
`PREFIX/bin/foundry` follows the current one. Uninstall never removes stores, memory
or caches. Details: [deployment § Lifecycle and installation](docs/deployment.md#lifecycle-and-installation).

## Quickstart

From the checkout, with the small example workspace:

```sh
cargo build --release --locked
export PATH="$PWD/target/release:$PATH"

foundry --store /tmp/foundry-demo index examples/workspace   # creates the store
foundry --store /tmp/foundry-demo search parse_record
foundry --store /tmp/foundry-demo context parse_record --tokens 1024
foundry --store /tmp/foundry-demo context 'who calls parse_record' --tokens 1024
foundry --store /tmp/foundry-demo retrieve --handle 'HANDLE'   # a handle from the output
foundry --store /tmp/foundry-demo status
```

A store is created only by an explicit `index` or `bootstrap --apply`; reads on a
missing store fail with `store_not_found`. A store is bound to one workspace path. Run `index` again after
editing files. The usage question above gets approximate doors because no compiler
references were imported. Graph bundles add producer-supplied file relationships:

```sh
foundry --store /tmp/foundry-demo import-graph examples/graph.json
foundry --store /tmp/foundry-demo graph src/main.rs --depth 1
```

## Use it from an agent

```sh
foundry bootstrap --root ~/src/repo             # inspect: prints a report, writes nothing
foundry bootstrap --root ~/src/repo --apply     # create ~/src/repo/.context-foundry and index
foundry connect --host omp --root ~/src/repo --print-config     # or --host codex
```

`connect` prints the host configuration, the launch command and the tool-use
instructions; it edits a host file only with `--apply-config FILE`, and
`--remove-config FILE` restores that file's previous bytes. The printed configuration
launches the stdio server:

```sh
foundry --store ~/src/repo/.context-foundry mcp --root ~/src/repo
```

The server exposes seven tools: `search`, `context`, `retrieve`, `references` and
`status` read; `index` and `memory` write, only to Foundry's own stores (`index` to the
primary store or an admitted reference store, `memory` to the primary store), never to
workspace files. Other MCP
clients can launch the same command; only OMP and Codex have been exercised. Keep
`.context-foundry/` out of version control.

**Several sessions on one repository.** By default one host session owns the store and
others get `store_busy`. For concurrent hosts, run one shared owner on IPv4 loopback
with a bearer token read from an environment variable, and print matching host
configuration. The variable (here `FOUNDRY_TOKEN`) must hold the same secret in the
owner's and the host's environment; configuration carries only its name.

```sh
foundry --store ~/src/repo/.context-foundry mcp --root ~/src/repo \
  --transport streamable-http --bind 127.0.0.1:9633 --auth-token-env FOUNDRY_TOKEN
foundry connect --host codex --root ~/src/repo --print-config --http-port 9633 --token-env FOUNDRY_TOKEN
```

`mcp --reference ROOT=STORE` (repeatable, up to 8) admits other indexed repositories;
`mcp --budget FILE` sets delivery budgets
([adapter economics](specs/003-agent-retrieval-context/contracts/adapter-economics.md)).

## What works

Short pointers; each links to its contract.

- **Search and context.** `search` prints one locator line per hit (`--path` restricts
  it to a subtree); `context` packs cited items under `--tokens`, with
  `--strategy auto|search|graph`. Wire format: [context-v2](specs/001-source-state-recovery/contracts/context-v2.md).
- **Retrieve.** `retrieve --handle H` returns exact bytes, optionally narrowed with
  `--lines`, or `--view outline`; long spans continue with `next:`.
- **Compiler references** ([005](specs/005-graph-evidence-lifecycle/spec.md)).
  `import-scip --index FILE --snapshot MANIFEST` imports a rust-analyzer SCIP artifact
  bound to the indexed source; `references --handle H` or `--symbol-id S` pages through
  them. Facts go stale when indexed source changes, until a fresh import.
- **Project memory** ([008](specs/008-scoped-durable-memory/spec.md)).
  `foundry memory put|update|get|forget|search|export`; `context --include-memory` adds
  validated `mem:` lines. Nothing is harvested from transcripts.
- **Multi-root** ([007](specs/007-multi-workspace-context/spec.md)). Admitted roots are
  merged with per-root coverage; an unavailable root is named, not skipped.
- **Recovery** ([001](specs/001-source-state-recovery/spec.md)). Interrupted indexing
  resumes with `refresh`; a broken search index is rebuilt by `repair-index`; reads
  never create, upgrade or repair a store.
- **Usage import.** `foundry usage import --host omp|codex --session FILE` summarizes
  a host session's provider usage and Foundry payloads offline.
- **Semantic retrieval** ([009](specs/009-optional-semantic-retrieval/spec.md),
  optional). `semantic prepare|status|purge` embed one address card per definition and
  per Markdown section with a pinned GGUF model on a statically linked llama.cpp
  worker; dense results serve only queries without an anchor. Model execution
  currently requires `--development-isolation`.
- **Owned learning** ([013](specs/013-owned-learning/spec.md), frozen off) and the
  **model gateway** (`foundry gateway`, `gateway-omp`;
  [003 T004](specs/003-agent-retrieval-context/spec.md#t004--forward-and-meter-an-actual-supported-model-workflow)):
  see [learning](docs/learning.md) and [deployment](docs/deployment.md).

## Limits

- One store has one owner: a CLI command, a stdio MCP session or the shared HTTP
  owner. Others get `store_busy`.
- Results describe the **indexed snapshot**, not the current bytes on disk. There is
  no watcher.
- Excluded from indexing: hidden files, ignored paths, `target`, `node_modules`,
  symlinks, non-UTF-8 files and files over 2 MiB. The sensitive-file deny rules are
  narrow; they are not a secret or PII scrubber.
- Lexical search retains at most 256 candidates per root, and exact definitions at most
  64 per anchor (up to four anchors); ranking may examine every matching index
  document. `references` examines at most 256 records in 64 files per call; graph
  traversal at most 256 edges and 64 files. A SCIP import accepts artifacts up to
  1 GiB. These are fixed bounds, not evidence of million-file scale.
- Doors answer "who uses X", not "what does X use". Definitions generated by macros
  have no unit. Other named limitations are listed per language in
  [context-v2 § Languages](specs/001-source-state-recovery/contracts/context-v2.md#languages).
- Compiler references come only from an imported artifact; Foundry never runs the
  producer.

## Documentation

| Read | For |
| --- | --- |
| [Spec portfolio](specs/README.md) | What each spec delivers and its status |
| [Validation](docs/validation.md) | Acceptance evidence, measurements and their limits |
| [Architecture](docs/architecture.md) | Design, ownership and alternatives |
| [Deployment](docs/deployment.md) | Bootstrap, worker isolation, package lifecycle |
| [Context-v2 contract](specs/001-source-state-recovery/contracts/context-v2.md) | Wire format, city map, languages, packing |
| [Graph bundle format](docs/graph.md) | `import-graph` input |
| [Owned learning](docs/learning.md) | The frozen learning subsystem |
| [Measurement handoff](docs/measurement-handoff.md) | Measurements still owed before semantic retrieval is enabled |
| [Release checklist](docs/release.md) | What a release requires |
| [Dependencies and licenses](docs/dependencies.md) | Third-party crates, forks and licenses |
| [Context packet](docs/context-packet.md) | Orientation for a new maintainer or agent |
| [Changelog](CHANGELOG.md) | Unreleased changes |

## Contributing, security and license

Contributions are welcome; read [CONTRIBUTING](CONTRIBUTING.md) first and follow the
[Code of Conduct](CODE_OF_CONDUCT.md). Report vulnerabilities privately as described
in [SECURITY](SECURITY.md).

Context Foundry is [MIT licensed](LICENSE). Dependencies keep their own licenses, and
model and dataset terms are independent of this license. The repository contains no
private datasets, transcripts or model weights.
