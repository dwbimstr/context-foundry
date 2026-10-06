# Spec and task audit

## 001 T005/T006 documentation closure — 2026-10-04

S1 contract and status pass after T005/T006 acceptance and the owner's commits
`5edf32c` (code) and `c430997` (specs/docs). Amended the
[v2 contract](../specs/001-source-state-recovery/contracts/context-v2.md) with the
owner decisions and implemented details reported by the authoring session and
checked against source at `c430997`: the 256-byte qualified-name tail (`src/syntax.rs`
`QNAME_BYTES`, `qualified`); declaration-only member signatures as mandatory outline
lines; signature and closing-line rules; whole-line block comments; span eligibility at
a range end (`content_end`); the graph window in `candidates:full`; the outline refusal
policies (`response::pack_retrieve_outline`, `outline_refusal_floor`); the
non-verbatim framing-LF limitation; "mapped language" read literally; unit handles
preserving syntax-node or Markdown-section ranges without appending line terminators;
and the seam types (`RankedItem.handle: Option`, `end_line`,
`CandidateCounters.graph`, tiers 3/4, `RenderedForm`). Go type declarations stay
unnamed under the literal Name rule; that is recorded as an accepted limitation with
its re-entry condition rather than amended. Status lines across specs and docs now say
T004–T006 are accepted and committed, unreleased. The 001 and 007 gate text now names
the actual 1.90 toolchain on `PATH`, because `rustup run 1.90.0` resolved to another
compiler on this workstation. The catalog was re-measured (598 tokens; see validation),
and the T006 latency risk was probed (end-to-end CLI wall time only; see validation).

Checks: a throwaway link checker over README, `docs/` and `specs/` (40 files, 463
relative links, 88 heading links, zero missing) and `git diff --check`, both clean.
Review: `S1DocsReview`, OpenAI `gpt-6.1-sol:xhigh`, cross-lab to the Anthropic author,
read the diff against source at `c430997`: REVISE (M1 Markdown section handles can
include a trailing LF; M2 the latency text attributed wall time to parsing without
separate timing; m1 003's opening status), all three corrected with its wording, then
delta SHIP. One reviewer, not an independent quorum. Documentation only: no source,
test or build change is part of this closure.

## 001 T004 documentation closure — final4

001 T004 is locally implemented and verified, unreleased. The owner reports all six
final4 gates exited 0 on an unchanged source manifest and the same existing
`T004CutoverReview` (OpenAI `gpt-6.1-sol:xhigh`) returned SHIP. This is the existing
in-session cross-lab review of the Anthropic author, not a fresh independent review.
See [validation](validation.md) for scope, commands, smoke and measurement boundaries.
This documentation closure did not rerun any source/build/test or runtime check.

The measured `examples/workspace` handle is 33 o200k tokens (68 bytes) versus
94 (207 bytes) for v1, superseding the 29/85 estimate; the serialized five-tool
catalog is 568 tokens versus the historical v1 711. Historical v1 results below and
in validation remain evidence for their original versions, not current wire output.
The ambiguity refusal is reviewed: the test parser refuses two complete item-line
readings without attributing either; no universal round trip is claimed. `invalid_range`
remains invalid input with CLI exit 2 under the existing contract; no rule changed.
T005 and T006 were later accepted and committed; see the closure above.

## Spec pass 2026-10-03 — preceding pass

2026-10-03 to 2026-10-04. The owner asked for one pass over every Context Foundry spec
so that no unknown stays implicit, with token economics as the top priority. This pass
is documentation only: it transcribes the approved decisions into their owning specs,
corrects stale text and names what remains open. No product source, test, build or
host configuration changed for it, and no runtime check ran. The token-economics
tranche (001 T004–T006, 003 T005, 007 T001) was approved and not implemented at that
spec pass; subsequent T004 final4 closure is recorded above.

Sources: two in-session spec audits, consolidated into one 69-row map. The original
audit reports are no longer reachable, so the rows below reproduce that map's
dispositions against the current repository text. A separate in-session citation
audit found drifted claims, corrected in this pass: the counting-boundary and
host-forwarding wording in
[context-v2](../specs/001-source-state-recovery/contracts/context-v2.md#counting-boundary)
and the [economics contract](../specs/003-agent-retrieval-context/contracts/adapter-economics.md#evidence-behind-the-rules),
and that contract's ADR-0052 summary. The handle-cap arithmetic is stated as a chosen
bound (AT3). Claims that audit could not verify keep their existing labels and are not
presented as verified. Author: Anthropic Opus 5.5; no independent or cross-lab
approval of this pass is implied.

Dispositions: **resolved decision** (an approved decision is now written in its owner),
**text fix** (stale wording corrected), **external prerequisite** (supplied or
authorized by the owner; listed in the portfolio's
[external prerequisites](../specs/README.md#external-prerequisites)), **open owned
decision** (explicitly unsettled, with the task that closes it) and **unchanged** (with
the reason). Two decisions remain open and owned: 005 T002 fixes the `references`
coverage and count header segments, and 009 T002 fixes the semantic-item line form,
each before implementation and reviewed against 001's v2 contract.

| ID | Finding | Disposition | Owner |
| --- | --- | --- | --- |
| A1 | `context-v1.md` still read "Proposed" | Resolved decision and text fix: v1 is a three-line superseded disposition; v2 carries every valid rule plus the compact wire, ranking, outlines and multi-root identity; links point to v2 | [context-v2](../specs/001-source-state-recovery/contracts/context-v2.md) |
| A2 | Economics contract status still "Proposed, 2026-09-29" | Text fix: delivery budgets and receipts implemented 2026-10-01 (003 T002); host-request mode and gateway proposed | [adapter economics](../specs/003-agent-retrieval-context/contracts/adapter-economics.md) |
| A3 | 001 heading said "Proposed persistence and recovery contract" for implemented behavior | Text fix: "Persistence and recovery contract" | [001](../specs/001-source-state-recovery/spec.md#persistence-and-recovery-contract) |
| A4 | 001 said "Proposed fixed limits" for implemented limits | Text fix: "Fixed limits" | [001](../specs/001-source-state-recovery/spec.md#reconciliation-rules-and-limits) |
| A5 | 007 was a deferral stub with re-entry criteria | Resolved decision: 007 reactivated with launch-time admission, aliases, one owner, merged search/context and T001; scope paragraphs in context-v2, deployment and architecture updated | [007](../specs/007-multi-workspace-context/spec.md) |
| A6 | 003 left combined root selection to "007's separate scope" | Text fix: 003 points to 007 for admission, aliases and merged responses | [003](../specs/003-agent-retrieval-context/spec.md#native-source-discovery-and-fallback) |
| A7 | v1 forbade fusing rankings across roots | Resolved decision: superseded by 007's merge (tier-1 first, then reciprocal-rank fusion) | [007](../specs/007-multi-workspace-context/spec.md#combined-search-and-context) |
| A8 | `verify_current` looked like stale text | Unchanged: it is a tested refused strategy (`unsupported_mode`), kept in v2 | [context-v2](../specs/001-source-state-recovery/contracts/context-v2.md#inputs-defaults-and-outputs) |
| A9 | 003 T004 credentials undefined | External prerequisite: an API key, a spend cap and the Codex custom-endpoint configuration; usage import measures provider usage without the gateway | [003 T004](../specs/003-agent-retrieval-context/spec.md#t004--forward-and-meter-an-actual-supported-model-workflow) |
| A10 | 009 pinned artifact/runtime unresolved | Resolved decision (D001 chosen values) plus external prerequisites: weights authorization, a USearch build on Rust 1.90, isolation and package acceptance | [009 D001](../specs/009-optional-semantic-retrieval/spec.md#d001-chosen-values-2026-10-03) |
| A11 | 013 package acceptance unresolved | External prerequisite: approved labeled rows, the LibTorch package, signing/notarization, an aggregate residency run; the spec decisions are recorded | [013 D001](../specs/013-owned-learning/spec.md#d001--concrete-path-and-remaining-feasibility) |
| A12 | Deployment isolation profiles never run | External prerequisite: isolation and package acceptance for 009 and 013; no code in this pass | [deployment](deployment.md#isolation-is-an-enforced-profile) |
| A13 | 001 accepts a scan race | Unchanged: the accepted enumeration swap-and-restore limitation keeps its re-entry condition | [001](../specs/001-source-state-recovery/spec.md#reconciliation-rules-and-limits) |
| A14 | No published release | External prerequisite: a release destination | [release](release.md) |
| AT1 | Search returned a whole 2 KiB block per hit | Resolved decision: locator lines with delivery-unit handles and excerpts of at most 160 bytes, over syntax-unit search documents | [v2 search locator lines](../specs/001-source-state-recovery/contracts/context-v2.md#search-locator-lines) |
| AT2 | About 17 metadata fields per envelope | Resolved decision: one header line, default segments omitted, v1 JSON success paths deleted | [v2 header line](../specs/001-source-state-recovery/contracts/context-v2.md#header-line) |
| AT3 | 64-hex JSON handles were verbose | Resolved decision: `path#start-end@sha32.ws16`. The 4200-byte cap allows 4188 bytes (4096-byte path plus 92-byte suffix), with 12 bytes headroom. The initial 29/85 o200k estimate is superseded by T004's fixture measurement: 33 tokens/68 bytes versus v1's 94/207; not a universal per-handle cost | [v2 source handles](../specs/001-source-state-recovery/contracts/context-v2.md#source-handles) |
| AT4 | Source was JSON-escaped twice | Resolved decision: plain v2 text; count the final text block or complete CLI stdout | [v2 counting boundary](../specs/001-source-state-recovery/contracts/context-v2.md#counting-boundary) |
| AT5 | Chunking rationale absent | Resolved decision: syntax-unit regions and blocks; 009 embedding units reuse delivery units up to 1024 model tokens | [v2 search documents](../specs/001-source-state-recovery/contracts/context-v2.md#search-documents) |
| AT6 | Compression policy unstated | Resolved decision: deterministic elision only (signature and outline forms); no compression model | [v2 ladder packing](../specs/001-source-state-recovery/contracts/context-v2.md#ladder-packing) |
| AT7 | Cross-call deduplication unstated | Resolved decision: none across calls; deduplication within one response | [v2 deduplication](../specs/001-source-state-recovery/contracts/context-v2.md#deduplication-no-cross-call-suppression) |
| AT8 | Tool catalog cost unbudgeted | Resolved decision: exact trigger-first descriptions; serialized `tools/list` at most 800 tokens (a test) | [003 catalog](../specs/003-agent-retrieval-context/spec.md#catalog-and-instruction-text) |
| AT9 | Multi-root budget unclear | Resolved decision: one effective budget, one reservation, one charge | [007](../specs/007-multi-workspace-context/spec.md#combined-search-and-context) |
| AT10 | A 2048-token default left little room after the envelope | Resolved decision: a v2 header target of at most 40 tokens and the effective-budget rule; a target, not a measurement | [v2 budgets](../specs/001-source-state-recovery/contracts/context-v2.md#budgets) |
| AT11 | Continuations and omissions were verbose | Resolved decision: a `next: <handle>` line, omitted/capped/stale header counts, prefix fit kept for the retrieve text view | [v2 retrieve views](../specs/001-source-state-recovery/contracts/context-v2.md#retrieve-views) |
| B1 | rust-analyzer run needs a jail | External prerequisite: a permitted jailed rust-analyzer run | [005 T001](../specs/005-graph-evidence-lifecycle/spec.md#t001--import-one-actual-rust-reference-relationship) |
| B2 | rust-analyzer release unpinned | Resolved decision: pin the weekly tag current at T001 start; record tag, binary SHA-256 and `--version` | [005](../specs/005-graph-evidence-lifecycle/spec.md#artifact-and-freshness-contract) |
| B3 | SCIP position encodings open | Resolved decision: UTF-8 code-unit offsets only; others are `unsupported_encoding`; UTF-16/32 conversion cut | [005](../specs/005-graph-evidence-lifecycle/spec.md#artifact-and-freshness-contract) |
| B4 | `typed_range` versus `range` precedence open | Resolved decision: typed ranges when present, else the deprecated fields | [005](../specs/005-graph-evidence-lifecycle/spec.md#artifact-and-freshness-contract) |
| B5 | Absent-document coverage open | Resolved decision: for the pinned profile a manifest-listed `.rs` absent from the artifact is `accepted_empty`; non-`.rs` paths are outside producer scope | [005](../specs/005-graph-evidence-lifecycle/spec.md#publication-queries-and-limits) |
| B6 | Build scripts and proc macros unaddressed | Resolved decision: recorded that `rust-analyzer scip` runs them, so each run needs deployment's compiler grants (part of B1's prerequisite) | [005 T001](../specs/005-graph-evidence-lifecycle/spec.md#t001--import-one-actual-rust-reference-relationship) |
| B7 | 1 GiB/8 MiB/4 GiB limits unmeasured | External prerequisite: the scale corpus and numeric profile | [005 T003](../specs/005-graph-evidence-lifecycle/spec.md#t003--complete-the-declared-large-workspace-task) |
| B8 | Scale-profile thresholds open | External prerequisite: same as B7 | [005 T003](../specs/005-graph-evidence-lifecycle/spec.md#t003--complete-the-declared-large-workspace-task) |
| B9 | No provider token-savings comparison | Resolved decision for measurement: usage import plus the authorized real-host runs under the bundled-adoption label; gateway measurement stays 003 T004's prerequisite | [003 T005](../specs/003-agent-retrieval-context/spec.md#t005--displace-grep-and-exploratory-reads-at-the-fewest-delivered-tokens) |
| B10 | Graph failure reasons mismatched | Resolved decision: header `graph:<ok\|graph_unavailable\|graph_stale\|graph_invalid>`; 005 adds `graph_invalid` | [v2 header line](../specs/001-source-state-recovery/contracts/context-v2.md#header-line) |
| B11 | "Source chunks" wording | Text fix: enclosing delivery units | [005](../specs/005-graph-evidence-lifecycle/spec.md#publication-queries-and-limits) |
| B12 | SC-00N undefined | Text fix: "SC-00N is the acceptance of T00N" in 005, 009 and 013 | [005](../specs/005-graph-evidence-lifecycle/spec.md#tasks-and-acceptance) |
| B13 | Graph doc named no producer | Text fix: names 005's rust-analyzer import and `references` | [graph](graph.md) |
| B14 | 009 status date stale | Text fix: dated D001 and this amendment | [009](../specs/009-optional-semantic-retrieval/spec.md) |
| B15 | 009 recipe and MSRV open | Resolved decision (D001 chosen values); the remaining hardware and Rust 1.90 checks are external prerequisites | [009 D001](../specs/009-optional-semantic-retrieval/spec.md#d001-chosen-values-2026-10-03) |
| B16 | Serving limit unselected | Resolved decision: 2048 tokens, checked at 2048 and 2049; the audit's loader-warning note (`apply_yarn_scaling`) belongs to that check, which runs under the weights-authorization prerequisite and has not run | [009 D001](../specs/009-optional-semantic-retrieval/spec.md#d001-chosen-values-2026-10-03) |
| B17 | Cache, batch and deadline caps open | Resolved decision: batches of 8 and 1, f32 cache with an F16 index, 2 GiB default cap, query ceiling min(1500 ms, remaining deadline) | [009 D001](../specs/009-optional-semantic-retrieval/spec.md#d001-chosen-values-2026-10-03) |
| B18 | Embedding-unit token limit open | Resolved decision: at most 1024 model tokens, greedy source order, derived deliberately from 001's unit forest so each byte is encoded once | [009 units](../specs/009-optional-semantic-retrieval/spec.md#documents-embedding-units-and-returned-evidence) |
| B19 | Parsers unselected | Resolved decision: `pulldown-cmark` 0.13 sections; other files by paragraph and line | [009 units](../specs/009-optional-semantic-retrieval/spec.md#documents-embedding-units-and-returned-evidence) |
| B20 | PyO3/MLX cancellation unproven | External prerequisite: isolation and supervisor acceptance in 009 D001 | [009 D001](../specs/009-optional-semantic-retrieval/spec.md#selection-decision--d001) |
| B21 | USearch C++ core on Rust 1.90 unproven | External prerequisite: a USearch build on the Rust 1.90 floor | [009 D001](../specs/009-optional-semantic-retrieval/spec.md#selection-decision--d001) |
| B22 | Lexical/dense merge unpinned | Resolved decision: exact definitions first, then reciprocal-rank fusion with k = 60 over the lexical top 256 and the dense top 64 | [009 ordering](../specs/009-optional-semantic-retrieval/spec.md#ordering-without-another-required-model) |
| B23 | Running the weights not authorized | External prerequisite: authorization to run the pinned local weights | [009](../specs/009-optional-semantic-retrieval/spec.md) |
| B24 | 013 packaging open | External prerequisite: signing/notarization and the LibTorch package | [013 D001](../specs/013-owned-learning/spec.md#d001--concrete-path-and-remaining-feasibility) |
| B25 | tch 0.24/LibTorch 2.11 on Rust 1.90 unproven | External prerequisite: the LibTorch package; 013 D001's MSRV check stays open | [013 D001](../specs/013-owned-learning/spec.md#d001--concrete-path-and-remaining-feasibility) |
| B26 | Float16 checkpoint handling unstated | Resolved decision: upcast to float32 at load; dtype in the model-function identity | [learning contract](../specs/013-owned-learning/contracts/learning-loop.md#one-model-and-one-decision) |
| B27 | Policy `state` content unstated | Resolved decision: query, graph coverage and top-3 lexical locator lines within the 1024-token total | [learning contract](../specs/013-owned-learning/contracts/learning-loop.md#exact-input-and-identity) |
| B28 | 16 KiB versus 1024-token limits conflicted | Resolved decision: the byte cap is a pre-tokenization guard; the token cap binds | [learning contract](../specs/013-owned-learning/contracts/learning-loop.md#exact-input-and-identity) |
| B29 | Prediction timeout policy unstated | Resolved decision: per-request fallback; three consecutive prediction timeouts disable the policy until restart; capacity stays held while work survives | [learning contract](../specs/013-owned-learning/contracts/learning-loop.md#serving-selection-and-rollback) |
| B30 | Group floors need data | External prerequisite: approved labeled rows | [learning contract](../specs/013-owned-learning/contracts/learning-loop.md#dataset-and-repeat-rounds) |
| B31 | Further decision families unspecified | Unchanged: variable-choice, boolean and ordinal families stay extension targets that need a named consumer; the related calibration guard (Laya #394) is resolved in 013 T002 | [013](../specs/013-owned-learning/spec.md#decision-ecosystem-and-review-traceability) |
| B32 | 8 GiB combined residency unproven | External prerequisite: an aggregate residency run | [deployment](deployment.md#isolation-is-an-enforced-profile) |
| B33 | Ledger still planned external Laya changes | Text fix: superseded by 013 contract v4; no external Laya change is selected | [this ledger](#remaining-prerequisites-explicitly-owned) |
| B34 | Architecture called the MCP envelope future | Text fix: the implemented 2026-10-01 boundary plus the approved v2 wire | [architecture](architecture.md#preparation-is-part-of-the-neural-feature) |
| B35 | Architecture said store upgrade was not implemented | Text fix: schema 2 with `upgrade-store --to 2` is implemented | [architecture](architecture.md#commit-and-failure-ordering) |
| BT1 | `references` payload unbudgeted | Resolved decision: v2 lines, a `tokens` budget, a `next: after=` cursor, 16-hex IDs (`ambiguous_symbol` with at most 8), import through `index {scip}`. The coverage and count header segments were decided 2026-10-04 (context-v2 segments 12–14) and implemented in 005 T002 (CLI, 2026-10-04) | [005](../specs/005-graph-evidence-lifecycle/spec.md#publication-queries-and-limits) |
| BT2 | Graph evidence rendering open | Resolved decision: `edge [<alias> ]<text>` lines after the first unit; the alias only in multi-root responses | [v2 evidence items](../specs/001-source-state-recovery/contracts/context-v2.md#evidence-items) |
| BT3 | Identity overhead per item | Resolved decision: 16-hex workspace prefixes in handles; per-root identity | [v2 multi-root identity](../specs/001-source-state-recovery/contracts/context-v2.md#multi-root-identity) |
| BT4 | Embedding unit versus delivered excerpt | Resolved decision on content: whole unit, lexical span or labeled preview, under the same budget. Open owned decision: the semantic item's v2 line form, fixed by 009 T002 before implementation | [009 units](../specs/009-optional-semantic-retrieval/spec.md#documents-embedding-units-and-returned-evidence) |
| BT5 | Code partitioning rationale | Resolved decision: tree-sitter units (001 T005); 009 reuses the delivery units | [v2 dependencies](../specs/001-source-state-recovery/contracts/context-v2.md#dependencies-and-languages) |
| BT6 | Query-time model cost unbounded | Resolved decision: min(1500 ms, remaining deadline); provider usage through usage import | [009 D001](../specs/009-optional-semantic-retrieval/spec.md#d001-chosen-values-2026-10-03) |
| BT7 | Preparation cost unaccounted | Resolved decision on method: usage import plus 009 T002's record list; no measurement yet | [009 T002](../specs/009-optional-semantic-retrieval/spec.md#t002--deliver-useful-semantic-context-within-the-existing-budget) |
| BT8 | Policy cost unaccounted | Resolved decision, revised 2026-10-06: `learning select` and owner startup require a net task-checker evidence gain over deterministic routing (`economics_unknown`, `candidate_no_benefit`); token savings at equal evidence never enable; a paid usage-import comparison, when authorized, runs only on changed routes. A visible `--lifecycle-check` override (`"lifecycle_check": true` in config and status) exists solely for lifecycle and package verification (013 T004, D001) and is not enablement | [013 T003](../specs/013-owned-learning/spec.md#t003--serve-select-roll-back-and-retire-the-old-http-path) |
| BT9 | Import and scratch cost unestimated | Measured 2026-10-04 (005 T001/T002, see validation): the real 56.9 MB `library/` artifact imported in 22–37 s at 470–480 MiB peak RSS, 57 MB copied, 87 MB scratch peak; the T003 run records the profiled figures | [005 T003](../specs/005-graph-evidence-lifecycle/spec.md#t003--complete-the-declared-large-workspace-task) |

Portfolio inventory, all 15 specs:

| Spec | State after this pass |
| --- | --- |
| [001](../specs/001-source-state-recovery/spec.md) | T001–T003 verified 2026-10-01; T004 locally verified on final4 at the time of this pass; T005/T006 later accepted (see the 2026-10-04 closure above) |
| [002](../specs/002-single-owner-serving/spec.md) | Superseded (owner: 003) |
| [003](../specs/003-agent-retrieval-context/spec.md) | T001–T003 implemented 2026-10-01; T004 proposed with external prerequisites; T005 implemented and accepted locally 2026-10-04, committed (`5e99ffd`) |
| [004](../specs/004-workspace-freshness-scale/spec.md) | Superseded (001 and 005) |
| [005](../specs/005-graph-evidence-lifecycle/spec.md) | Proposed; decisions recorded; one open owned decision (T002 header segments); external prerequisites |
| [006](../specs/006-semantic-producer-adapters/spec.md) | Superseded (005) |
| [007](../specs/007-multi-workspace-context/spec.md) | Reactivated 2026-10-03; T001 implemented and accepted locally 2026-10-04 (delta SHIP), committed (`cc402e0`) |
| [008](../specs/008-scoped-durable-memory/spec.md) | Proposed; decisions recorded; no external prerequisite |
| [009](../specs/009-optional-semantic-retrieval/spec.md) | Proposed; D001 chosen values recorded; one open owned decision (T002 line form); external prerequisites |
| [010](../specs/010-source-bound-derived-knowledge/spec.md) | Deferred, re-entry criteria unchanged |
| [011](../specs/011-outcome-token-economics/spec.md) | Superseded (001 and 003) |
| [012](../specs/012-learning-data-contract/spec.md) | Superseded (013) |
| [013](../specs/013-owned-learning/spec.md) | Proposed; decisions recorded; external prerequisites |
| [014](../specs/014-migration-coexistence/spec.md) | Deferred, re-entry criteria unchanged |
| [015](../specs/015-open-source-release-lifecycle/spec.md) | Superseded (release checklist); release destination is an external prerequisite |

Also carried by this pass: the [token-economics flow](architecture.md#token-economics-flow)
with measured v1 payload ranges beside labeled v2 targets; the portfolio and roadmap
order; and the [Laya upstream drift note](references/laya-decision-ecosystem.md#upstream-drift-recorded-2026-10-03).
The historical [subtraction review](review/subtraction.md) row that names a separate
`import_scip` tool predates this pass and is left as history; 005 now imports through
`index`.

After these edits settled, the coordinating session ran a temporary read-only checker
over `README.md`, `docs/**/*.md` and `specs/**/*.md`: 40 Markdown files, 453 relative
links and 85 heading links, with zero missing targets or anchors; all 69 expected
ledger IDs, with none missing, duplicated or unexpected. `git diff --check -- docs
specs README.md` exited 0 with no output. These are documentation consistency checks
only: no Rust build or test, live store, host or model check ran. The coordinating
review was an in-session OpenAI Sol review, not the required fresh-context GPT-6 Astra
acceptance of this spec pass; that acceptance and the commit remain deferred. The
documents stay uncommitted in the shared working tree, whose source and tests belong
to the concurrent S2 bootstrap work.

## 001 + 003 implementation and contract repairs — preceding pass

2026-10-01. Implemented the owner-approved tranche and recorded evidence in
[validation](validation.md). Contract repairs made during implementation, each from
executed evidence: shared owner pins session-bearing MCP versions, refuses stateless
2026-07-28 requests and treats stream loss as non-cancelling (rmcp 3.5.0 source);
the counts-only partial index error and `resultType` clearing (context-v1); inverted
intervals fail at the field stage; Codex needs printed MCP approval configuration and
tools carry `readOnlyHint` annotations (real host); the enumeration swap-and-restore
residual is an accepted, named limitation with a re-entry condition (001).

Authors: Z.ai GLM-5.3 with Anthropic Sonnet 5.5 fallback (two isolated worktrees).
Reviews: OpenAI `gpt-6.1-sol` source and adapter rounds, a same-lab Sonnet source
round 2, and a final OpenAI `codex exec` pass; providers taken from recorded session
events. Decisions after the two-round limit were the captain's and are recorded in
the owning specs. No predecessor store/service, global host configuration, model
download/training or paid gateway request was used.

## Ecosystem decision reconciliation — preceding pass

2026-10-01. The owner's reported coverage, argument failures and stale busy health
are accepted observations, not remeasured. The
[deployment disposition](deployment.md#ecosystem-readiness-disposition--2026-10-01)
maps them to existing 001/003/009/release owners without adopting predecessor stores,
reintroducing a custom WAL or activating federation/migration.

The unchanged release prototype, SHA-256
`5589bc3018c4149e9bd62804a0eab13e27a5d91ba966fefea54ed69efc16da78`,
was exercised in a removed temporary fixture: `--help` exposes no MCP/bootstrap/retrieve;
`--store <missing-path> status` returned schema-1 success and created the store;
explicit fixture indexing/search from a different cwd returned the fixture source.
This proves the existing open-path defect and explicit-path prototype behavior,
not the proposed recovery, MCP, health or ecosystem acceptance.

Two bounded read-only in-session decision reviews used Anthropic Opus 5.5 without
observed fallback; they are not independent or cross-lab approval. They separate
selected model/backend contracts from missing execution/rights/package inputs and
identify the persistent single-host conflict with the actual OMP+Codex workflow.
Rotating operation-scoped owners are rejected. The owner subsequently selected
optional shared SDK MCP and the full 001/003 T001–T003 implementation tranche.
The owning contracts now record that approval, without implying acceptance.

Corrected stale artifact-execution, ModernBERT CPU and vector-head wording, classified
009/013 D001 accurately, and made startup-unavailability fallback name the available
host diagnostic rather than require a nonexistent tool result. Two isolated GLM
worktrees now implement the approved source/adapter slices; root integration waits
for their settlement and review. No predecessor/global configuration, model
download/training or paid gateway request is authorized by that scope.

Documentation verification: seven affected files, 56 relative links and five heading
links checked; zero missing targets/anchors or trailing-whitespace errors. No Rust
build, new model run or live Foundry acceptance is claimed for these document changes.

## Native discovery requirement — preceding documentation pass

2026-10-01. The owner requires Foundry to precede grep/ripgrep in ordinary agent
source discovery. Added 003 FR-008 and its canonical selection/fallback rules,
bootstrap project guidance and actual tool-order acceptance inside existing T001/T003.
Updated architecture, deployment, roadmap, portfolio and contributor guidance.
MCP availability, instruction preference and enforced host routing are distinguished;
the amendment does not implement MCP or imply a supported multi-repository join.

Checked documentation paths/anchors, requirement/task mapping and whitespace.
No product/probe code, dependencies, models, stores, host configuration or upstream
repositories changed. No runtime host task, performance or token-savings acceptance
was executed. Existing graph/neural/learning/gateway readiness boundaries remain.

## Decision-ecosystem traceability — preceding documentation pass

2026-09-29. Added [the source-to-contract map](references/laya-decision-ecosystem.md):
Laya source commit, checkpoint revision, numerical-library versions/commits, functions,
regression tests, notebook cell/hash references, retained/adapted/deferred behavior,
existing proof and proposed Rust owners. All mapped Laya source was inspected at its
clean pinned checkout; notebook JSON was read, not executed. Checked the published
`common.py` bytes against that checkout and resolved the reference Transformers tag.

Made v4 answer-probability/abstention/tie semantics explicit, removed ambiguous private
reply confidence, specified calibration-override rejection and per-case evaluation
diagnostics, and attached source rows to T001–T004. Retained broader typed decisions
as explicit extension targets without claiming that the first choice family implements
them. Corrected stale roadmap/architecture statements about the completed scratch probes.
These are documentation/contracts/reference changes; no product/probe code, model runs,
upstream tests or services changed. Structural/link checks are recorded with the commit;
they do not establish learning or deployment acceptance.

## Executed feasibility — preceding runtime pass

2026-09-29. Owner authorized bounded sandboxed probes and direct main publication.
[The feasibility report](review/feasibility.md) records real Rust/MLX execution,
ModernBERT/head/selected-QKV reference parity, a head update/read-back, native process
limit checks, Rust 1.90 MCP exchange and a vector-index smoke. Product source and root
dependencies are unchanged. Probe sources/locks and public synthetic results are in
`tools/feasibility`; weights, private data and third-party source remain outside Git.

Replaced 013's obsolete vector-head contract/tasks with ModernBERT v4 and propagated
the current disposition through architecture, deployment and portfolio. Complete
model recipe, distributed package, actual host/provider and scale/quality evidence
remain explicit, scoped prerequisites. No full-product readiness or savings claim.

Checks: probe Rust formatting and documentation links/task structure; unchanged
protected product/source/build/license files. The legacy checker has a hardcoded old
013→009 dependency and therefore cannot validate the new semantic dependency graph;
that boundary was reviewed directly. Structural counts are not readiness evidence.
The historical entries below retain their original time and scope.

## Team handoff simulation — preceding paper review

2026-09-29. After pushing `f51e6c6`, traced the fifteen current spec dispositions,
six active workflows and adjacent source owners as a paper implementation handoff.
The [report](review/handoff-simulation.md) distinguishes contract conflicts, missing
decisions and unexecuted integration proofs. It is an in-session review, not code
execution, independent approval or an exhaustive predecessor/source audit.

Corrected remaining Nemotron-vector assumptions in 001/009, architecture, deployment
and the dependency inventory. ModernBERT policy availability follows its own inputs;
009 failure alone does not disable it. Moved combined-policy deadline acceptance to
013's later integration so semantic-only release does not wait for learning. Added
combined model-residency planning to the existing deployment owner. 013's replacement
contract/tasks, real Rust/runtime bridges and packaged profiles remain incomplete.

Inspected the selected publisher's MLX loader at
`d0408b94c50fc327b6ea37dce7409c51e020a4d8` without executing it or obtaining weights.
Its Python load/encode API is not a verified Rust service. No application code, tests,
dependencies, models, corpora, services or host configuration were modified. Only
documentation and Git publication were performed; runtime claims remain untested.
Structural/documentation checks passed: 15 spec files, 276 relative links, nine
heading links and unchanged hashes for 13 source/build/license files. The checker
counts retained superseded task entries and does not prove semantic task readiness;
the handoff report supplies that assessment. `git diff --check` also passed.

## ModernBERT review — current scope correction

2026-09-29. Read Laya's actual joint-input/typed-head implementation, checkpoint
loader and source cells of the fine-tuning notebook at local revision
`4066d5d5fbf08b66c6757ddeedbd797bd7655bc0`. The notebook assigns nonzero learning
rates to encoder and head and backpropagates through both; the earlier small fixed
query-vector head is not an equivalent capability. Details and primary links are
in [learning](learning.md#modernbert-and-the-cost-of-matching-laya--2026-09-29).

Inspected Candle ModernBERT→RoPE→custom-op code at
`5ba5d5b468b5b1df40e82dd3d556987bedeea041`: the Q/K rotary path drops the backward
graph. Inspected Burn's official model catalog and tch's LibTorch/autograd/import
surface. These establish integration options and a concrete training-path gap,
not local gradient correctness, speed, backend/MSRV compatibility or model quality.

The owner clarified ModernBERT **with a decision head** as the target. 013 D001 now
pins trainable parameters and the model/head contract before library selection;
v3 and its four implementation task entries are superseded, retained for review only.
D001 must replace affected clauses before implementation; this is not a build-ready
ModernBERT plan or permission to substitute the earlier query-vector classifier.
Preserved Rust ownership, Nemotron retrieval, consent, deterministic hard budgets,
independent source/gateway releases and the existing task slots without new stages.

This was source/documentation review only: no Laya/notebook execution, model download,
dependency installation, training, service operation or provider request. Production
code and Cargo dependencies are unchanged. Eight preceding planning files were
archived locally with hashes; no archive is public. The following older entries
retain their original scope and do not establish ModernBERT support.

Documentation checks pass for 15 specs and 265 relative links plus seven anchor links;
`git diff --check` passes. The 19 counted task entries include four superseded 013
entries, so this count does not establish implementation readiness. All 13 checked
source/build/license files remain unchanged. No Rust build or model test was run.

## Current amendment — owned Rust learning, bootstrap and economics

2026-09-29. The owner's latest answers select all first-party implementation in Rust
and both context budgeting and optional model-request forwarding/metering. Revised
013 into owned learning (renamed directory), replaced its v2 external-Laya handoff
with v3 feature/head/worker contracts, and added 013 D001 for a real Rust-library and
packaged-isolation proof. Laya stays read-only reference material; its current HTTP
client and legacy format fixture remain unchanged until implementation cutover.

003 now covers explicit bootstrap, connection config, budget boundaries, usage
receipts and a separately accepted Rust gateway task. 001 retains store/packing
ownership; 005 distinguishes artifact import from isolated producer execution;
009 owns the selected MLX runtime's still-unproven Rust bridge and isolation. Updated
architecture, constitution, roadmap, deployment, release and security boundaries.
Reviewed the current active ownership/dependencies and all nine inactive dispositions
for contradictions. This does not rerun or claim exhaustive coverage of 71 predecessor
specs/codepaths. Findings, hypotheses and tradeoffs are in the existing subtraction review.

Read current Rust inference/feedback code and Laya reference material. Checked official
Burn, Linux, Apple and libkrun documentation; consulted OpenAI Docs for Codex provider
configuration, Responses streaming and input counting. Documentation establishes
possible interfaces, not installed compatibility, model quality, OS enforcement or
provider economics. No credentials were read, model/provider calls made, packages
installed, hosts configured, repositories published or Prakarana/Laya state changed.

Documentation consistency checks pass: 15 specs, six active/nine inactive, 19
implementation tasks, three recorded decisions, 33 FR and 21 SC IDs; the explicit
dependency map is acyclic. Relative links/anchors and whitespace pass. All 13 protected
source/test/Cargo/CI/license files match the prior baseline; all 53 files in the new
ignored pre-amendment archive match their captured hashes. The old public Laya-format
fixture is unchanged. Checks use temporary read-only scripts, not product tooling.
No Rust build/test, sandbox test, fine-tuning or gateway integration was executed.

Remaining concrete proofs: 009 model/runtime/bridge/profile; 013 library/MSRV and
train/save/load/predict under an actual packaged jail; 003 gateway host/schema/counting/
stream/tool compatibility; and each selected package's install/upgrade/rollback.
No proposed feature is marked implemented or independently approved. Older entries
below retain their original scope and counts; they do not override this amendment.

Date: 2026-09-28. Scope: all 15 current spec files, every active task, both shared
contracts and the adjacent implementation. This replaces the earlier structural-only
readiness record. Single-session review as requested, not independent counsel.

Earlier follow-up: 009 became an active proposed preparation/retrieval workflow.
The portfolio has six active specs, nine dispositions, eighteen implementation tasks
and two explicit decision tasks. Earlier counts below are preserved as historical
review scope. No new implementation is implied by reactivation.

## Earlier pass: reuse, composition and hidden work

The second requested adjacent-pattern review found seven additional failures in
source/proposed contracts: reused memory revisions, a filesystem replacement race,
learning novelty tied to group novelty, graph cardinality multiplication, status
requiring unperformed tokenization, abandoned inference escaping client bounds, and
serialized handles exceeding their input cap. Updated the existing 001/005/008/009/013
tasks and shared contracts; checked 003 and all nine inactive dispositions against
those changes. Findings and retained tradeoffs are in the existing
[subtraction review](review/subtraction.md).

The JSON bound counterexample was executed in isolation: a valid 4096-byte escaped
path makes an 8353-byte handle, exceeding the former 8192 limit. No Foundry code,
filesystem race, provider admission or learning behavior was executed. Source files,
tests, build files, services, models, private stores and external repositories remain
unchanged. Documentation checks and code/contract reasoning are the evidence for
this pass; implementation acceptance is still outstanding.

Current consistency checks pass: all 15 specs, six active/nine inactive, eighteen
implementation tasks and two recorded decisions, 30 FR/20 SC IDs, 231 relative links
and two anchors. The reviewed dependency map is acyclic; whitespace checks pass.
All thirteen protected source/test/build/license files and archived drafts match
the captured baseline. The checks do not execute the proposed behavior or establish
that all future interactions have been discovered.

## Earlier adjacent-pattern review

Expanded the review from model/chunk examples to initialization, read side effects,
failure isolation, repeated repair/cleanup, artifact identity, real caller reachability,
schema ownership, response snapshots and input truncation. The concrete failures,
source/design evidence, accepted tradeoffs and existing task owners are recorded in
[the subtraction review](review/subtraction.md). Six active specs and their shared
contracts were amended; all nine inactive dispositions remain inactive. No task or
spec count increased. Review/analyze/template prompts now carry the adjacent-boundary
question without imposing a new portfolio review or measurement stage.

Read-only source evidence includes `Engine::open` and CLI dispatch, graph/source/cache
ownership boundaries, and Laya `build_sequence` at the recorded local revision. Its
512-token sequence can truncate state after question/options; identical train/serve
formatting alone would preserve that loss. The planned adapter now rejects oversized
labeled inputs before fitting and returns a named serving fallback before inference.
This is source evidence, not a tokenizer/model execution result.

The amended specs also make staged graph import reachable through the existing MCP
owner, keep source reads/exports usable after a derived-index failure, prevent mixing
compiler artifacts at one source revision, bound repeated repair artifacts and require
exclusive vector purge. These are proposed acceptance contracts, not implemented fixes.
No Rust build/test, compiler/model execution, training, package install, live service
operation, Prakarana/Laya mutation or publication occurred in this pass.

That pass's consistency checks passed: 15 specs, six active/nine inactive, 18 implementation
tasks, two recorded decision tasks, 30 FR/20 SC IDs, 229 relative Markdown links and
two anchors; no cycles in the reviewed task dependency map. All 13 protected
source/test/build/license files and archived drafts match the prior captured hashes.
Whitespace checks pass. These checks verify document structure and preservation;
the semantic findings above come from manual review, not the counting script.

## Coverage and substantive findings

Read every spec: five active workflows and ten Superseded/Deferred dispositions.
Reviewed every task in 001/003/005/008/013, the shared source/response contract and
Laya handoff. Traced current `store.rs`, `ingest.rs`, `graph.rs`, `laya.rs`, CLI/module
entry points and existing core/CLI/protocol tests. In Laya, verified the current commit,
package entry points, sequence/option helpers, Agent preparation/loading, evaluation
parser and training/calibration sections of notebook cell 8. No training code ran.

| Ambiguity/failure found | Concrete correction |
| --- | --- |
| Recovery task could not be reached when index open itself failed | Authoritative-only status/export and explicit restartable derived-index repair, with interruption points and preservation assertions |
| Bounded scan had no bounds or partial-deletion semantics | Fixed source/page/sample limits, last-seen scan ownership, interrupted-sweep behavior and exact counters |
| Shared handles lived under a later adapter spec | Moved contract ownership to 001; 003 consumes it without a reverse dependency |
| Budgeting left escaping, continuation progress and source starvation unclear | Exact emitted-byte boundaries, bounded prefix trials, named errors and first-source packing witness |
| MCP work omitted admission/cancellation and a concrete user task | One active/zero waiting operation, explicit cooperative deadlines and partial writes; a specific checked parser edit in a real host |
| Compiler freshness checked only edge endpoints | Bind compiler output to an immutable input snapshot and whole source revision; test a third-file edit |
| Graph replacement had no defined scope or accepted-empty distinction | Producer/origin-document transaction, reverse-index atomicity, unknown versus empty coverage and explicit import/query bounds |
| Memory tasks lacked API/conflict/deletion behavior | Exact request fields/limits, revisions, retry/conflict semantics, logical forgetting and source/memory separation |
| Feedback CLI could not write beside the agent owner | Planned MCP observation route, explicit offline training permission/preparation, no competing writer |
| Training rules used undefined sufficiency and selection language | Typed dataset/lineage limits, execution floors, fixed recipe, exact metric/eligibility rules and incomplete/rejected outcomes |
| Laya task combined nonexistent training and serving prerequisites | Four separately verifiable tasks; real checkpoint loading and second round cannot be replaced by JSONL/protocol fixtures |
| Dataset first-manifest field could create a self-hash cycle | Compute dataset identity from input file/policy hashes before group-map/manifest hashes |
| Deferred stubs could be mistaken for executable plans | Concrete re-entry evidence for 007/009/010/014; no tasks/contracts in any inactive bundle |

The behavior pass tightened contracts; the ownership pass checked adjacent code;
the consistency pass checked consumers, dependencies, failure/restart and acceptance.
More words are not evidence of better architecture: no new product worker, scheduler,
protocol or storage layer was implemented. Substantial safeguards stay with the five
workflows rather than becoming new feature specs or measurement stages.

## Remaining prerequisites, explicitly owned

001 D001 is now resolved by the ecosystem pass below: retain redb/Tantivy within the
one-pending-table/one-rebuild-marker bound; recovery acceptance is still unexecuted. 003 T001
pins an MSRV-compatible SDK, and T003 needs an installed authorized host. 005 T001 needs
an actual permitted producer; T003 needs a declared corpus/hardware profile with numeric
limits before running. 013 needs permitted local model/tokenizer/data, hardware and
separately selected external Laya changes (superseded by 013 contract v4, 2026-09-29:
Laya is a read-only reference and no external Laya change is selected). These are
execution/selection prerequisites, not vague success criteria or evidence that every
task can run immediately.

## Follow-up: external references and neural ownership

The session raised an additional boundary: mentioning or reading another repo must
not silently enroll it in source reconciliation, watching or training. Tightened
the existing 001/003 scope and 013 learning contracts and their acceptance cases;
At that pass 007/009 remained deferred dispositions, with no new task, service or
registry. The later neural-preparation correction below supersedes 009's deferral.
Tests now specified cover wrong-root refusal without source changes, launch-directory
independence, foreign handles, separate-store reconciliation, opaque external paths
and no implicit cross-store training data. These tests have not been implemented.

Source verification: Foundry `ingest.rs::sync` binds the canonical root and disables
symlink traversal; `store.rs::bind_workspace` rejects a different root; `laya.rs::predict`
sends the query string and does not resolve paths or record feedback. In Prakarana,
`workspace.cpp::scan_run` refuses a root change before mutation; its comment records
the prior umbrella-store deletion failure. `pas_serve.cpp::pas_scan_cycle` passes the
configured root to that scan. `session_ingest.cpp::index_episode` stores scrubbed
session bytes under a session URI, and `enrich_worker.cpp::process_embed` reads stored
record bytes and caches the result by content hash/model tuple. Thus session evidence
about a file is not evidence that the file itself is indexed or fresh. These are
source conclusions, not a newly executed live-session or cross-repo experiment.

The history tool returned `query-gen transport` after about 90 seconds; no answer
from that call was used. The checked-in M26 result was read directly: on its deliberately
constructed vocabulary-gap corpus, sole-neural delivered 35/120 hits and fused dense
delivered 22/120 at k=5. This supports preserving semantic retrieval and examining
composition losses; it does not establish current live quality or select Nemotron.
The configured neural output was 768, distinct from the 256 lexical dimension.
Official model cards linked in architecture confirm model-specific output sizes.
No model download, training, new benchmark or running-service change occurred.

## Checks and proof limits

### Neural preparation correction

The previous deferral preserved model identity but omitted a preparation lifecycle.
Read predecessor specs 032/042/071, the M42 result, neural worker/cache interfaces and
the reuse contract test; checked Foundry's chunking, context strategy and 003's owner
contract. Preparation on the single MCP operation slot would block queries, while
rebuilding document vectors on ordinary index repair would repeat expensive inference.
009 now owns a cache in the selected transactional library, a derived search index,
one bounded inference worker, source-owner publication, partial coverage and explicit
pause/resume. Source ownership still never expands from an outside path mention.

Three proposed tasks cover durable preparation, actual semantic context and progressive
agent use. 009 D001 requires exact permitted artifacts/runtime/library, schemas and
numeric bounds before code; it is not complete. Query/ranking-only changes explicitly
reuse document vectors. Laya is outside the preparation dependency chain, and its
outcome evidence must name retrieval readiness rather than treat cold-index failure
as a learned-strategy label. Existing prototype code and public fixtures are unchanged.

This is a scoped design correction, not another all-spec or live-runtime audit. The
M42 figures are historical; no new throughput/latency estimate, model download,
training or background worker ran. Dimension facts were checked against the primary
Google/NVIDIA model cards linked in architecture. Documentation checks include the
new task dependencies and links; runtime acceptance remains unexecuted.

Follow-up consistency results: 15 spec files checked, six active/nine inactive;
18 implementation tasks and two decision tasks; 30 FR and 20 SC IDs; 218 relative
links/two anchors resolve. The task dependency graph has no cycles. All 13 protected
source/test/build/license files match the earlier baseline. Whitespace checks pass.
These counts establish document consistency only, not neural implementation readiness.

### Selected embedding checkpoint

The owner supplied the exact `nvidia/Nemotron-3-Embed-1B-BF16` model page and selected
it over the earlier Llama Nemotron suggestion. Updated 009, architecture and portfolio:
initial 2048-dimensional output, explicit prompting/normalization and separately
licensed weights. D001 no longer owns a cross-model choice; it still must pin artifact
revision, runtime/index integration, schemas and bounds. Verified the official model
card, including the distinction between model input ceiling and serving limits.
No model download/inference or hardware compatibility claim was made. Existing
documentation checks were rerun; source/test/build/license files remain unchanged.

The owner then supplied `mlx-community/Nemotron-3-Embed-1B-BF16-4bit` for local MLX
execution. Read its card and bundled loader without executing it. The loader has a
4096-token truncation default and can download a missing model path implicitly;
009 now requires preflight length checks and a pinned existing local directory.
Quantization/loader identities remain distinct from upstream BF16. The publisher's
reported throughput is not local validation. Updated the selected local profile in
architecture/portfolio/roadmap and reran documentation checks; no new runtime ran.

### Document chunking correction

Read the predecessor's current `src/internal/raw_projector.cpp::project_spans`,
`manthan.cpp::stage_raw_source_delta`, `enrich_worker.cpp::process_embed` and
`embed_neural_gemma.cpp` input guards. Source projection admits hard cuts from 1024
bytes, probes a soft cut at 3200 and forces a boundary at 5000, with no overlap.
The embedding adapter independently limits its first single input to at most 2400
bytes and can halve it again on HTTP 500. This demonstrates two different text
boundaries and possible loss of a record's tail from the model input; it does not
establish how often that occurs in the current live corpus or its measured effect.
Foundry's `store.rs::chunks` also uses fixed 2048-byte blocks in its prototype.

Revised 009 to separate authoritative source, embedding unit and delivered evidence:
whole documents within a selected tested token limit, grouped sections/paragraphs
for larger sources, no overlap or duplicate document/section vector hierarchy.
The loader's 4096 default is no longer a permanent architectural size. Larger limits
require one bounded check on the selected runtime; no weights were downloaded/run.
Source coordinates and input-key cache reuse survive storage-boundary/offset changes;
whole-unit edits and repartitioning can still incur new inference. Context distinguishes
the matched unit from returned bytes and labels an unlocalized prefix as a preview.
T001/T002 now cover complete source coverage, tail evidence, edit reuse and actual
input/output costs. A filename hit alone cannot pass span-delivery acceptance.

Aligned architecture, the shared 001 response contract, 005 graph seed handling and
013 outcome provenance. No new numbered spec, task, service or release gate was added.
This is source/design evidence; document consistency checks do not prove the MLX
runtime's usable long-input limit, semantic quality or preparation-time improvement.

The check results below describe documentation consistency, not runtime acceptance.
The historical crosswalk still inventories all 71 predecessor specs; this turn did
not re-audit every predecessor implementation. Original review coverage included
full reads and scoped sampling, not exhaustive requirement verification.

No Context Foundry source/test/Cargo/CI file changed in this pass. No Rust build/test,
Prakarana/Laya mutation, live-store probe, host configuration, model training or
publication was performed. Earlier first-slice runtime evidence remains in
[validation](validation.md). Original and immediately preceding plan drafts are
preserved in ignored local archives. OMP file layout is present; host command
registration/execution is untested. Public Laya examples remain format-only fixtures.

## Earlier task-pass consistency checks

Latest ecosystem follow-up: reread all 15 current specs and both shared contracts;
reviewed adjacent Rust source and sampled local Laya interfaces. The detailed findings
and all-ID dispositions are in [the subtraction review](review/subtraction.md).
Updated existing contracts for deterministic ranking versus optional Laya, one
embedding profile, shared provider deadlines, graph-availability bypass, truthful
graph criteria, and mixed strategy labels within a task group. 001 D001 is resolved
as an architectural decision, not a new test result. No spec/task count increased.

The two public format rows now describe definition/reference exploration rather than
resolved callers. Their JSON shape is checked; no new Laya parser/model acceptance
is claimed. All production source/test/build/license files remain unchanged. No
model execution, service operation, training, package install or publication occurred.
That earlier pass's checks passed: 15 spec files, six active/nine inactive, 18 implementation tasks,
two recorded decisions (one resolved), 30 FR/20 SC IDs, 224 relative links and two
anchors; no dependency cycles. All 13 protected implementation/build/license files
match the prior baseline. Whitespace and both revised format rows' JSON/choice shape
pass. These checks do not execute the proposed Rust behavior or real Laya integration.
The following older counts describe their original pass, not the current portfolio.

- 15/15 current spec files inspected; five active specs contain 15 implementation
  tasks plus one explicit decision task. The ten inactive dispositions contain no
  executable plans/tasks/contracts.
- 24 functional requirement IDs and 17 acceptance IDs map to active task behavior;
  each task states dependencies, scope, outcome, verification and review/cutover.
- Task dependencies are acyclic; shared context ownership is 001→003, with no
  lower-level dependency on building the adapter. Learning data can precede graph
  acceptance, while claims about useful learned graph selection cannot.
- All Git-visible relative Markdown paths and heading anchors resolve. The first
  check found a stale historical anchor and two inconsistent task field labels;
  these were fixed and the check rerun successfully.
- The 71-entry historical crosswalk and ten command/playbook pairs still match their
  inventories. No inactive bundle gained implementation work during this refinement.
- SHA-256 comparison to the start-of-turn snapshot confirms all 13 protected source,
  test, build/CI and license files are unchanged. Archived prior drafts also match
  their captured hashes. Public format fixtures remain unchanged.
- `git diff --check` and whitespace checks over tracked/untracked public Markdown
  pass. These are documentation checks, not the new specs' runtime acceptance.

Checks used a temporary read-only local Python script, not new product tooling or a
new mandatory gate. The spec review did not declare future hardware, model quality,
SDK integration, graph scale or learned savings already validated.
