# Roadmap

The product goal is for a project to retain the context its operator supplies,
show how decisions depend on original evidence, and answer which features need
review after a change. ChatGPT desktop is the primary place to use that workflow.
The [capability map](architecture.md#capability-map) describes the implementation.
The [portfolio](../specs/README.md) owns task scope and the [handoff](resume-context-ecosystem.md)
owns acceptance and remaining work.

| Workstream | What is available or still gated |
| --- | --- |
| Source, graph and delivery (001, 003, 005, 007) | Cited search and retrieval, explicit graph and compiler imports, opt-in watching and bounded multi-root context. The Python HTTP boundary has a specific accepted static scope. |
| Project memory and change impact (003, 008) | Recoverable task records, shared originals, reviewed dependencies and changed-source review are implemented on the experimental branch. Broader real-host task outcomes and economics remain tied to the unfunded pilot. |
| Semantic retrieval (009) | EmbeddingGemma 2 is the selected optional source profile. The experimental saved-decision lookup reuses its worker. The 009 T005 anchorless source-retrieval investigation closed at the existing baseline. |
| Generation, learning and metering (010, 013, 003) | Generated knowledge is experimental with real-model utility deferred. Owned learning is frozen off. A new live proof belongs to the gateway's separate release. |
| Distribution | The first release requires all active tasks, including 003 T006, and the signed semantic package with its installed-artifact checks. Paid trials and signing require their existing authorization. [Release checklist](release.md) |

009's cache, profile and chunk lifecycle follows 001 source identity. EmbeddingGemma 2 on the pinned
llama.cpp worker is the only selected profile (G2, G1 and vector parity, 2026-10-08).
The owner closed the Nemotron comparison on 2026-11-06 ("Close it; Gemma only") and
keeps the Nemotron files only as historical evidence.
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
acceptance remain open. Learning stays disabled in normal retrieval; since 2026-10-06 a
candidate is selectable only with a net evidence gain over deterministic routing (a
visible `--lifecycle-check` selection is package verification, not enablement). Laya
is a research reference only. The [deployment contract](deployment.md) follows each
advertised feature through installation, shutdown, upgrade, rollback and uninstall;
libkrun is conditional infrastructure, needed only where an isolation profile requires it.

Federation and legacy migration need specific user failures before implementation;
launch-time multi-root context (007) is not federation. Opt-in watching (001 T011) is
accepted at the owner-selected native scope after the F18 repair and the pinned
rust-lang/rust `mcp --watch` run on the owner's Mac
([receipt](validation.md#native-mac-watcher-acceptance-001-t011--2026-11-10)). Generated knowledge (010) is implemented
experimentally; its real-model acceptance is deferred and it stays unadvertised
([owner decisions, 2026-11-06](validation.md#owner-decisions--2026-11-06)). The
[subtraction review](review/subtraction.md) explains the smaller design; do not
resurrect old plans because an ID still exists.
