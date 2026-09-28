# Graph import

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

The planned compiler adapter will add producer capabilities, source census,
coordinate validation and stable symbol identity. It must not automatically run
an indexed repository's build scripts or pretend syntactic candidates are resolved
cross-file calls. Provider execution is explicit and outside retrieval.
