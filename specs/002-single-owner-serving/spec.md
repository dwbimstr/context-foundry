# 002 — Disposition

Status: Superseded, 2026-09-28.

The custom socket daemon plus MCP shim remains removed. 003 keeps default direct
stdio ownership. On 2026-10-01 the owner selected optional standard Streamable HTTP
MCP for concurrent OMP/Codex clients on one root; its authentication, admission and
native-client acceptance belong to 003, not a revived protocol or separate bundle.

Current owner: [003](../003-agent-retrieval-context/spec.md).

This replaces the earlier proposed bundle. It has no active plan or implementation
tasks. See [the current portfolio](../README.md); old drafts are not requirements.
