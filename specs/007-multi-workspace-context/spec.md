# 007 — Disposition

Status: Deferred, 2026-09-28.

First prove one workspace. Re-enter only with a specific cross-repository task that has a documented missing join or identity ambiguity with separate workspace queries, including scope/identity expectations. No registry or federation coordinator is planned now.

Session mentions of outside paths are already covered by the
[shared scope contract](../001-source-state-recovery/contracts/context-v1.md): they
never enroll, watch or index those paths. Explicit separate stores preserve access
to multiple repos without making one reconciliation own them all. Host-read external
snippets are not freshness-checked workspace evidence. This limitation is visible;
no automatic cross-repo graph join is implied. These rules do not require activating
this deferred feature or building an external-reference registry.

The project owner can reactivate this goal by supplying two named repositories,
one question requiring evidence from both, the expected source references, and a
recorded failure/extra work from querying each separately. The new spec must state
identity collision and partial-workspace failure behavior before implementation.
No task, registration service or cross-workspace storage is authorized by this stub.

These are prerequisites to selecting future scope, not executable tasks or release gates.

This replaces the earlier proposed bundle. It has no active plan or implementation
tasks. See [the current portfolio](../README.md); old drafts are not requirements.
