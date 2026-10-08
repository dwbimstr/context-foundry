# AGENTS.md — Context Foundry

Build useful context for coding agents in Rust: source, graph, explicit memory and
bounded delivery. Learning is an owned Rust subsystem in isolated workers; Laya is a
read-only research reference, not a runtime dependency. The repo is MIT.

Read the [constitution](.specify/memory/constitution.md), [current portfolio](specs/README.md)
and selected spec. User instructions take precedence. Respect implemented/proposed
boundaries and existing authorization; do not ask again for already selected work.

- Apply the constitution's KISS principle: preserve sophisticated outcomes while
  reducing what maintainers and operators must coordinate. Do not weaken acceptance
  or quietly abandon a product goal to make the implementation look simpler.
- Keep one crate and ordinary modules. Challenge each new worker, protocol, table or
  required document against the actual user need and a simpler alternative. No custom
  database, generic orchestration platform or inherited C++ obligations.
- Use the [modified Spec Kit tools](.specify/README.md) as needed. One spec is sufficient;
  plan/tasks files and command stages are optional. No benchmark per feature by default.
- When compatible Foundry tools are mounted for an admitted indexed repository, use
  `search`/`context`/`retrieve` before eligible repository grep/ripgrep. Follow
  [003's fallback rules](specs/003-agent-retrieval-context/spec.md#native-source-discovery-and-fallback)
  for unsupported patterns, current-file checks and unavailable coverage.
- Preserve source identity, freshness, provenance, bounded output and user data. Never
  run workspace code merely to index it. Training stays outside source transactions.
  Training consent differs from retrieval consent.
- Keep private corpora, transcripts, datasets, credentials and weights out of Git.
  Neither regex filtering nor an MIT code license establishes data/model rights.
- Preserve unrelated work. One writer per tree. Do not touch predecessor/Laya services,
  stores, host configuration or repositories without task-specific authority.
- Verify at the claimed boundary: focused tests for behavior, real consumers for
  integration, provider usage for cost. No source or histogram inference presented as
  runtime causal proof. Do not label an in-session review independent.
- When upstream code informs a design or review, retain its commit/release, module/
  symbol and relevant test, the owning Foundry requirement/task and intentional
  differences. Link actual landed Rust code when available; mark proposed paths and
  unexecuted evidence. Use the [decision-ecosystem map](docs/references/laya-decision-ecosystem.md)
  for learning reviews. Keep references with their existing owner, without a new stage.
- Release only when every task of every active spec is implemented and accepted, then
  run [the checklist](docs/release.md) (owner policy 2026-10-04). Deferred and
  superseded specs are not release conditions. Publication is separate.
- Bootstrap, worker isolation and upgrades follow [deployment](docs/deployment.md).
  Distinguish MCP delivery budgets from host-request control; do not claim full token
  economics without actual adapter visibility. Never label an ordinary subprocess a jail.

See [architecture](docs/architecture.md), [subtraction review](docs/review/subtraction.md)
and [current validation](docs/validation.md). Repairs update the existing contract;
they do not automatically create another numbered spec.
