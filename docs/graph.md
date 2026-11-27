# Graph import

Use the graph operation that owns the relationship you need.

| Question | Evidence and scope |
| --- | --- |
| Which files are connected by an imported edge? | `import-graph` and `graph` traverse the producer's declared file relationships. The format is described below. |
| Where is this symbol referenced? | `import-scip` and `references` use a compiler artifact bound to the indexed snapshot. Without compiler facts, source context can show approximate usage. [Compiler contract](../specs/005-graph-evidence-lifecycle/spec.md) |
| What executed, or which service handles this HTTP call? | `import-execution` reads a completed producer artifact. `boundary` publishes or queries scoped static Python Flask/FastAPI and literal HTTP facts. [Execution evidence](experimental-context.md#explicit-execution-evidence) · [Boundary scope](experimental-context.md#boundary-relationships-and-task-evidence) |
| Which feature relies on a saved decision and why? | The dependent feature records a requirement or constraint in task memory. `task_context` with `decision_uses` returns its quoted reason. `source_review` checks which shared originals have changed. A wording match asks the operator for clarification. [Memory workflow](experimental-context.md#recoverable-task-context) |

`foundry import-graph FILE` atomically replaces the named producer's complete
bundle. Other producers retain their facts. An empty `edges` array removes that
producer's contribution. A rejected bundle preserves the prior contribution.

See [the runnable fixture](../examples/graph.json). Each bundle contains:

- `provider`: stable producer namespace, nonempty and at most 128 bytes.
- `revision`: producer/recipe revision, nonempty and at most 128 bytes.
- `edges`: at most 100,000 relationships; the CLI input cap is 64 MiB.

Each edge has `from` and `to` endpoints, `kind` and `evidence`. An endpoint includes
the normalized relative source `path`, one-based `line`, a producer-supplied
`symbol` label (at most 1024 bytes), and full source SHA-256 `hash`. Both sources
must already exist in the indexed snapshot with matching hashes and line bounds.
Unknown fields are rejected.

Kinds: `calls`, `references`, `imports`, `contains`, `depends_on`.
Evidence classes: `resolved`, `syntactic`, `inferred`, `manual`.
These are declarations, not importer-verified semantic resolution. The bundled
example is deliberately `manual`.

`graph PATH` follows outgoing edges; `--reverse` follows incoming edges. The
current node identity for traversal is a file path. Symbol labels do not make it
a symbol-level call graph. Depth is 0..4, examined-edge budget is 1..256, and
visited files are capped at 64. Stale edges count against examination work and are
omitted. The result reports staleness and truncation; an empty result does not
certify the absence of callers or a producer's complete coverage.

The compiler adapter is owned by [005](../specs/005-graph-evidence-lifecycle/spec.md)
and is implemented: its rust-analyzer SCIP import (`import-scip`) and `references`
operation were accepted on 2026-10-04
([record](validation.md#005-compiler-references--t001-and-t002-2026-10-04)). The
adapter adds producer capabilities, source census, coordinate validation and stable
symbol identity. It must not automatically run an indexed repository's build scripts or
pretend syntactic candidates are resolved cross-file calls. Provider execution is
explicit and outside retrieval.
