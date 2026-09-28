# Working on Context Foundry

Build a local context engine for coding agents. Keep the implementation in Rust.
Laya is an optional external local decision/training runtime, not a dependency of
basic indexing or retrieval. Do not copy either predecessor repository's runtime.

- Start with the user story and the current code. One owner for each durable fact.
- Use maintained libraries for storage and search. Do not build a WAL, allocator,
  database engine, query language, or orchestration framework here.
- Keep the common flow easy to follow. Introduce a boundary for a real external
  protocol or responsibility, not a hypothetical replacement.
- Cite source paths, spans, hashes, and semantic producer identity. Inferred edges
  must never masquerade as compiler-resolved relationships.
- Bound work and output. Report omissions, stale evidence, and partial results.
- Indexing and retrieval must remain useful without a model or GPU.
- Training data and weights stay local and out of Git. Explicitly authorized data
  collection does not authorize publication. Never run a repository's own scripts
  merely because it is being indexed.
- Run focused tests for changes; run the complete Rust checks for a release.
  Reuse results when their inputs have not changed. A performance experiment must
  name the decision it will change and have a time limit.
- A release can have documented limits. Research, a new model, or an optional
  optimization cannot silently become a prerequisite for an existing feature.
- Distinguish compiled, tested, exercised with a real provider, and proven at scale.
  Token counts alone do not establish savings or task success.
- Preserve unrelated work. One writer per working tree.

See `docs/architecture.md` for the design and `docs/review/` for evidence and coverage.
