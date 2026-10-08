# Changelog

All notable changes to this project are recorded in this file. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). No version has been released
yet; acceptance evidence for every item below is in [validation](docs/validation.md).
The local tag `v0.1.0` made on 2026-10-04 was withdrawn and deleted the same day; it
was never published and is not a release.

## [Unreleased]

### Added

- Explicit indexing into a local store (redb and Tantivy) with interrupted-index
  recovery, bounded refresh, `repair-index` and `upgrade-store --to 6`.
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
- Optional semantic retrieval (address cards on a statically linked llama.cpp worker),
  off by default and run only under development isolation until its measurements and
  signing pass.
- An installable macOS arm64 package with install, upgrade, rollback and uninstall
  (`scripts/package.sh`, `scripts/install.sh`); not signed yet.
- A meter-only model gateway for one pinned host profile (OMP 18.6.0 with Z.ai
  `glm-5.3-flash`).
- Owned learning commands (spec 013), frozen off: kept as research tooling, not
  packaged or advertised.

[Unreleased]: https://github.com/dwbimstr/context-foundry/commits/main
