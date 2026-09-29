# Design lessons

The current [feasibility disposition](feasibility.md) records real bounded probes,
decisions and remaining owner-specific checks. The earlier
[team handoff simulation](handoff-simulation.md) records the paper-review baseline.
For decision/learning reviews, the [source-to-contract map](../references/laya-decision-ecosystem.md)
links pinned upstream code and tests to Foundry tasks, modules and explicit differences.

The design was informed by a private predecessor architecture review and direct
inspection of Laya. Private specifications and detailed operational evidence are
not part of this source distribution. These are design choices, not claims that
the new implementation has already proved superior outcomes.

| Lesson | Consequence here | Tradeoff |
| --- | --- | --- |
| Database, indexing and runtime ownership can dominate delivery work | Use transactional storage and rebuildable search; keep authoritative data ownership explicit | A separate search index needs pending work and stale-candidate checks |
| A memory-safe language does not remove consistency obligations | Change responsibility boundaries as well as language | Fewer custom optimizations in the first cut |
| Many small graph tool results can cost more than direct source reading | Assemble a bounded cited neighborhood alongside source spans | Packing and seed selection still need quality validation |
| Approximate search limits must not delete semantic truth | Retain accepted graph facts; bound traversal and report truncation | Persistent facts need ordinary maintenance as sources/providers change |
| A component benchmark is not an end-to-end product demonstration | Exercise the actual CLI and later a real agent before its release claim | Some component achievements do not count as finished user workflows |
| Learning needs a concrete decision and external correctness evidence | Start with search/graph routing, explicit feedback and task-grouped exports | This does not train a general coding agent |
| Every optional feature multiplies operational states | Start with one crate and CLI; add serving, semantic production and learning as independently usable cuts | The first release has narrower capabilities |

Two storage shapes were considered: a single SQLite database with FTS and adjacency,
and a Rust transactional store plus a derived Rust search index. The latter was
implemented for the Rust direction and retained by 001 D001 after comparison,
with bounded recovery and the tradeoff stated in
the [architecture](../architecture.md). No microservice or database-framework
abstraction was added to make either alternative appear interchangeable.

The review used Jinx-1120's
[architecture-review method](https://github.com/Jinx-1120/skills/blob/6700e01afc9db95fefa4a6779a596f70fe9b877d/skills/architecture-review/SKILL.md):
intent, hypotheses, counterevidence, ownership and proof limits. The requested
[codex-architect method](https://github.com/sd0xdev/sd0x-harness/blob/7805e98510943f45e57f9856966172401cff0c40/skills/codex-architect/SKILL.md)
was also read. Its CLI dispatch failed before research because the local CLI did
not support the configured model. The owner chose to keep the audit in the main
session. This is therefore a single-session review, not independent approval.

All source-level conclusions remain separate from runtime measurements. The
first-slice tests do not establish large-codebase performance, graph precision,
live Laya behavior, trained-model improvement, or provider-cost savings.

The [subtraction review](subtraction.md) supersedes the fifteen-bundle portfolio and
its readiness framing. The all-spec crosswalk remains historical, not a feature mandate.
The 2026-09-29 amendment supersedes external Laya integration with owned Rust learning,
verified worker isolation, explicit bootstrap and both delivery/gateway economics.
Those are proposed contracts; the legacy prototype remains unchanged.
