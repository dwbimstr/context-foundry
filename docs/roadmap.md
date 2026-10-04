# Roadmap

The [portfolio](../specs/README.md) owns order and status. Release useful workflows:

1. Reliable cited CLI context, including source update and interrupted-index recovery (001).
2. Explicit repo bootstrap, direct stdio MCP, native source discovery before eligible
   grep/ripgrep, and honest adapter budget/usage boundaries (003). The actual agent
   workflow verifies default tool order and named fallback, not just MCP availability.
   Its optional T004 adds a Rust forwarding/metering gateway for a verified host/API;
   gateway work does not gate the independently useful MCP release.
3. Fewest delivered tokens for the same cited evidence, so agents use Foundry instead
   of grep/ripgrep and exploratory reads: compact v2 text, syntax-unit search with
   exact definitions first and deterministic outlines (001 T004–T006); trigger-first
   tool text, the opt-in OMP first-call hook, usage import and measured economics
   (003 T005); one budgeted response across explicitly admitted repositories (007 T001).
4. One real code-relationship workflow on a declared large workspace (005).
5. Progressive neural preparation, semantic retrieval and retained work across restart/edit (009).
6. Explicit project memory as an independent feature (008).
7. Repeated owned Rust policy fine-tuning with isolated execution, selection and rollback (013).

All seven are planned work; a release waits until every active spec is complete (owner
policy 2026-10-04). As of
2026-10-01, 001 T001–T003 and 003 T001–T003 (including the optional shared owner) are
implemented and verified locally ([validation](validation.md)). The owner selected
item 3 on 2026-10-03 as the current core work, ahead of the optional features; it is
implemented locally: 001 T004–T006 are accepted and committed (`5edf32c`); 003 T005 and
007 T001 were accepted and committed locally on 2026-10-04; none is released, and the
[release checklist](release.md) runs only after every active spec is complete. 003 T004
(gateway), 005, 008, 009 and 013 remain proposed; the 2026-10-03 spec pass classified
their remaining unknowns as settled decisions, named open owned decisions (005 T002's
`references` header segments, 009 T002's semantic-item line form) or external
prerequisites, all listed in the portfolio. 001 D001 stays
resolved: redb/Tantivy. Optional model choices do not reopen that decision by default.

Plan 009's cache/profile/chunk lifecycle alongside 001. Nemotron 3 Embed 1B with the
owner-supplied MLX 4-bit artifact is selected and its D001 recipe values are chosen;
run the serving-limit check and validate the isolated runtime bridge before
full-corpus preparation.
Neural retrieval can precede graph; it
does not depend on policy training or generated summaries. Semantic-enabled use
requires cold/partial/warm/edit/restart proof.
Use one embedding profile and deterministic candidate ordering first. A learned
reranker is not selected; 009 names the ordering failure needed to revisit it.
The owned policy target is ModernBERT with a decision head, separate from 009 retrieval.
013 v4 now pins initial frozen-encoder/head adaptation and the Rust tch/LibTorch path;
broader encoder adaptation and typed decisions retain their explicit acceptance.
The [source-to-contract map](references/laya-decision-ecosystem.md) preserves the
decision ecosystem beyond the first search/graph consumer. [Feasibility](review/feasibility.md)
proves selected model/gradient boundaries; complete recipe and installed-profile
acceptance remain open. Learning stays disabled in normal retrieval until a
usage-import comparison shows equal correctness and lower total provider tokens. Laya
is a research reference only. The [deployment contract](deployment.md) follows each
advertised feature through installation, shutdown, upgrade, rollback and uninstall;
libkrun is conditional infrastructure, needed only where an isolation profile requires it.

Federation, generated knowledge, automatic watchers and legacy migration
need specific user failures before implementation; launch-time multi-root context
(007) is not federation. The [subtraction review](review/subtraction.md)
explains the smaller design; do not resurrect old plans because an ID still exists.
