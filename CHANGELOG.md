# Changelog

All notable changes to this project are recorded in this file. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). No version has been released
yet; acceptance evidence for every item below is in [validation](docs/validation.md).
The local tag `v0.1.0` made on 2026-10-04 was withdrawn and deleted the same day; it
was never published and is not a release.

## [Unreleased]

### Changes since November 20, 2026

- ChatGPT desktop project configuration and the opt-in `task_context` tool support
  saving an attributed decision and reopening it in a fresh chat. [Desktop workflow](README.md#chatgpt-desktop-on-your-mac).
- Capture a supplied meeting or indexed spec once. Features can cite shared
  originals, declare reliance on a decision, record corrections and review
  changed passages. A change proposal can recover earlier evidence and ask for
  a scope decision. [Shared-source review](specs/003-agent-retrieval-context/contracts/shared-source-review.md) · [Change proposals](specs/003-agent-retrieval-context/contracts/indirect-change-impact.md).
- Saved decisions can be found through a dependent requirement, a feature title
  or, on eligible misses, the optional local Gemma decision profile. Readers
  see original quotes and review status. [Decision lookup](docs/deployment.md#desktop-decision-lookup).

- The no-provider OMP pilot startup probe fails closed on network denials apart
  from the documented system logging socket. Startup owner inventory and admission
  remain open. [Startup evidence](docs/resume-context-ecosystem.md#current-status).

The entries under Added, Changed, Fixed and Known limitations below describe
the baseline merged to `main` on November 20. [Merge record](docs/validation.md#merge-of-the-experimental-line-to-main--2026-11-20) · [Current handoff](docs/resume-context-ecosystem.md).

### Added

- Explicit indexing into a local store (redb and Tantivy) with interrupted-index
  recovery, bounded refresh, `repair-index` and `upgrade-store --to 12`.
- Syntax units in 23 languages through tree-sitter, Markdown heading sections, and
  parallel indexing on up to 8 threads with a deterministic work budget per parse.
- The city map: addresses for every definition, anchors resolved to a definition or a
  directory of namesakes, and doors for usage questions (exact from imported compiler
  references, otherwise approximate).
- `search`, `context` and `retrieve` with exact `o200k_base` token budgets, verified
  source handles, outline views and identical CLI and MCP output.
- An MCP server with seven tools (`search`, `context`, `retrieve`, `index`, `status`,
  `memory`, `references`) over stdio or one shared loopback HTTP owner, plus
  `bootstrap` and `connect` for OMP and Codex.
- Compiler references from imported rust-analyzer SCIP artifacts (`import-scip`,
  `references`).
- Explicit project memory (`foundry memory`, the MCP `memory` tool).
- Multi-root context across one primary repository and up to eight admitted reference
  repositories.
- Offline host usage import (`foundry usage import`).
- Optional semantic retrieval (complete `hierarchy-v1` embeddings, the default since
  F4, with address cards selectable, on a statically linked llama.cpp worker), off by
  default and run only under development isolation until its measurements and signing
  pass.
- An installable macOS arm64 package with install, upgrade, rollback and uninstall
  (`scripts/package.sh`, `scripts/install.sh`); not signed yet.
- A meter-only model gateway for one pinned host profile (OMP 18.6.0 with Z.ai
  `glm-5.3-flash`). It is released separately from the CLI and MCP, not advertised
  with them, and needs a new live proof first (owner decision, 2026-11-06, "Release it
  separately"; [record](docs/validation.md#owner-decisions--2026-11-06)).
- Owned learning commands (spec 013), frozen off: kept as research tooling, not
  packaged or advertised.
- File watching: `mcp --watch` keeps the index of the admitted primary repository
  current from filesystem notifications. It is off by default and adds no MCP tools.
  A change whose notification is lost without being reported is picked up only by an
  explicit `index` or a restart of the MCP owner, and behavior on network filesystems
  is unverified. A native run over the pinned rust-lang/rust corpus on the owner's Mac
  accepted it on 2026-11-10
  ([record](docs/validation.md#native-mac-watcher-acceptance-001-t011--2026-11-10)).
  That run used the batched watcher commits listed under Fixed.
- Experimental, opt-in commands, described in
  [experimental context](docs/experimental-context.md): `task-context` (recoverable
  task state, also an MCP tool under `mcp --experimental-task-context`), `brief`
  (derived briefs bound to cited sources), `knowledge` (generation of briefs and area
  knowledge under an explicit profile) and `boundary` (HTTP relationships between
  indexed Python services that use FastAPI or Flask; indexing runs no workspace code).
  The boundary relationships are accepted at their written fixture scope
  ([record](docs/validation.md#boundary-scope-acceptance-005-t005--2026-11-10)).
  Task evidence composition (`context --compose-evidence`), which can include these
  relationships, is implemented but not accepted; it stays open until an authorized
  run passes (003 T009).
  `brief` and `knowledge` are not advertised for release (see Known limitations).
- Structured assertions in `task-context`: a task entry can carry
  exact key and value assertions, and `review` reports a proposed fact that conflicts
  with a stored one or violates a stored requirement or constraint. No model is used.
  The local checks pass; a benefit on real tasks is not established
  ([record](docs/validation.md#structured-task-assertions-008-t004--2026-11-13)).

### Changed

- Lexical behavioral selection is on by default in `context` and the MCP owner.
  Behavioral selection chooses evidence for a question about how
  code behaves. A question that starts with a recognized phrase, such as "how does" or
  "why does", and names exactly one type gets up to four of that type's methods that
  match the question, in place of the type's body; when no method matches, the earlier
  selection applies. A plain-language
  question of three or more words without an identifier is treated as prose: code
  passages and text passages are ranked separately and alternate, and a word of the
  question puts a definition first only when exactly one indexed definition has that
  name. `--experimental-behavior-context=false` selects the earlier behavior, and a
  request that gets a semantic candidate window, even a partial or empty one, does not
  use the selection. It is accepted as a lexical retrieval change; no benefit on real
  coding tasks is claimed
  ([record](docs/validation.md#t010-singleton-qualification--2026-11-11);
  [contract](specs/001-source-state-recovery/contracts/context-v2.md#experimental-behavioral-support--001-t010-2026-10-09)).
- The parser dependencies move together: tree-sitter-scala 0.26.4,
  tree-sitter-powershell 0.26.6 and pulldown-cmark 0.13.6. The derived search index
  schema becomes 9, so an index built earlier needs an explicit `repair-index`; the
  store schema stays 12, and sources, memories and tasks are kept. Semantic retrieval
  then needs `semantic prepare` again, which reuses cached vectors for unchanged inputs
  ([record](docs/validation.md#parser-bundle-and-complete-unit-comparison--2026-11-11)).
- Tantivy moves from 0.26.2 to 0.27.0. This change alone needs no
  repair: in the recorded check, stores written with 0.26.2 opened without one. The
  parser change above is what requires `repair-index`
  ([record](docs/validation.md#tantivy-027-store-compatibility-and-g1-neutrality--2026-11-11)).
- rustls moves from 0.23.45 to 0.23.46, on `main` since 2026-11-07
  ([record](docs/validation.md#owner-decisions--2026-11-06)).

### Fixed

- Opening a store no longer reports `repair_required` while a child process between
  fork and exec still holds the search index's writer lock. The open waits at most one
  second for that lock and then returns `store_busy`
  ([record](docs/validation.md#derived-writer-contention-after-fork--2026-11-11)).
- Semantic and learning workers start with umask 0077, so the files they create with
  default permissions are accessible only to their owner, whatever the caller's umask
  ([record](docs/validation.md#worker-scratch-creation-defaults--2026-11-13)).
- A watcher walk commits the files it reads in batches of at most four, where it used
  to force one commit to disk per file, and a background index page holds 32 keys
  instead of eight. This shortened the first reconciliation in the 6,000-file watcher
  test, which had run past its deadline
  ([record](docs/validation.md#watcher-test-repair-f18--2026-11-10)).

### Known limitations

Draft, written 2026-11-19. The [release checklist](docs/release.md) requires release
notes that name these limitations; this list is finalized, and every item rechecked,
on the signed release candidate. The fixed bounds of the core are listed in the
[README](README.md#limits).

- **Device policies that are not built.** Device acceptance is what
  `scripts/native-macos-acceptance.py` checks (owner decision, 2026-11-06, "Script
  checks only"; [record](docs/validation.md#owner-decisions--2026-11-06)). Four
  policies do not exist: global GPU pressure, combined residency of the embedding and
  generation models, thermal and battery adaptation, and embedding eviction. The
  script has passed once, with ad-hoc signing
  ([record](docs/validation.md#native-macos-acceptance--2026-11-10)), and must run
  again on the signed release candidate.
- **Query-time parsing is neither cached nor bounded per request.** `context` parses
  the candidate bodies it has verified to build outlines and forms, limited only by
  the per-file parse limits. In a synthetic worst case of 32 Rust files of about 1 MiB
  each, `context` took about 4.2 s of CLI wall time. The MCP owner's read deadline is
  5,000 ms, which the roots of a multi-root request share, and a request that runs
  past it fails with the retryable `deadline_exceeded`. The owner accepted this as a
  named limitation that reopens only after a measured failure on a real corpus
  ([spec 001, T006](specs/001-source-state-recovery/spec.md#t006--fit-more-evidence-with-outlines-and-forms);
  [measurement](docs/validation.md#001-t005t006-local-acceptance--2026-10-04)).
- **Semantic scale is accepted on one host and one corpus.** The accepted boundary is
  the complete `hierarchy-v1` preparation of rust-lang/rust on the owner's Mac with its
  replay and changed-input check
  ([record](docs/validation.md#semantic-scale-replay-and-changed-input-acceptance--2026-11-10)).
  Scale on Linux x86 and on the 67,663-source Rust and Go corpus is not accepted. That
  record accepts neither retrieval quality nor a benefit on coding tasks.
- **Anchorless retrieval quality is not improved beyond the current baseline.** The
  work to improve retrieval for questions with no anchor (spec 009 T005) closed
  without promotion: four candidates were measured, none was promoted, and the current
  retrieval stays (owner decision, 2026-11-19, "Close, no promotion";
  [record](docs/validation.md#owner-decisions--2026-11-19);
  [loss analysis](docs/validation.md#t005-loss-analysis--2026-11-19)). The
  [spec](specs/009-optional-semantic-retrieval/spec.md#t005--diagnose-and-improve-anchorless-evidence-retrieval)
  gives the condition for reopening it.
- **Other limitations the owner named on 2026-11-19.** Each has no planned work and a
  condition for reopening it in its spec
  ([record](docs/validation.md#owner-decisions--2026-11-19)):
  - device tuning and real throughput for semantic preparation (009 T007) are
    unverified beyond what the native script checks;
  - a semantic embedding worker paused with SIGSTOP is not ended if its owning
    `foundry` process dies;
  - late chunking and selective body preparation are not built (009 T009);
  - task evidence composition does not cover dynamic dispatch, indirect wrappers,
    unsupported languages, broader framework behaviour, requirement satisfaction or
    semantic constraint conflicts (003 T009);
  - a task view cannot be applied through ordinary MCP, to more than one host region
    or automatically (008).
- **Boundary relationships cover the accepted fixture scope only.** The producer reads
  Python FastAPI and Flask services. Other frameworks and brokers, broader import
  forms, identity across languages and dynamic collectors are outside that scope
  ([record](docs/validation.md#boundary-scope-acceptance-005-t005--2026-11-10)).
- **Not advertised for the first release.** Source-bound derived knowledge (`brief`
  and `knowledge`, spec 010) stays experimental, and its acceptance with a real model
  is deferred. The meter-only model gateway is released separately, after a new live
  proof. Owned learning (spec 013) is frozen off and is not packaged
  ([release checklist](docs/release.md)).

[Unreleased]: https://github.com/dwbimstr/context-foundry/commits/main
