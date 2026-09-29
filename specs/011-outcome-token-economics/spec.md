# 011 — Disposition

Status: Superseded, 2026-09-28.

Exact output packing belongs to 001. 003 owns deterministic delivery budgets,
bootstrap/adapter integration and optional usage receipts under its
[adapter economics contract](../003-agent-retrieval-context/contracts/adapter-economics.md).
Complete-request budgeting requires actual host pre-send hooks; MCP alone cannot
provide it. The owner's latest answer adds a narrow owned request gateway in 003 T004.
It forwards only explicitly routed supported requests and opens no source store.
Unknown provider usage remains unknown. This is no separate governor or durable
request ledger; 013's policy cannot increase a hard budget or grant spending rights.

Current owner: [001](../001-source-state-recovery/spec.md) [003](../003-agent-retrieval-context/spec.md) [013](../013-owned-learning/spec.md).

This replaces the earlier proposed bundle. It has no active plan or implementation
tasks. See [the current portfolio](../README.md); old drafts are not requirements.
