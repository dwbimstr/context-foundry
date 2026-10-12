# Context packet: restarting this work elsewhere

**Latest checkpoint, 2026-10-11:** start with the [resumable implementation handoff](resume-context-ecosystem.md). It supersedes the older status paragraphs below; the six acceptance gates are still open in part.

Written 2026-10-08 so that a fresh agent (for example a cloud agent) can continue
without the original sessions, their memory store or their temporary directories. Read
it after [AGENTS.md](../AGENTS.md), the [constitution](../.specify/memory/constitution.md)
and the [portfolio](../specs/README.md). Where this file and a spec disagree, the spec
wins; this file only orients. To set up a new machine step by step (what to copy,
what to clone, how to prove the setup reproduces the recorded results), follow
[resume on another machine](resume.md).

## Where things stand

**Continuation, 2026-10-11:** work remains on `experimental/context-ecosystem`,
including main's coordinated tch 0.26/LibTorch 2.13 migration. The product outcome is
still open. The [capability map](context-ecosystem-plan.md#what-exists-and-what-is-missing)
and [pending inventory](context-ecosystem-plan.md#pending-work-inventory--2026-10-10)
separate missing capabilities from implemented behavior awaiting acceptance.

The branch implements incremental watching, progressive semantic preparation, ANN
delta publication/reuse, source-bound generated knowledge, task state and composed
evidence, frozen/live OMP views, request receipts and Linux resource controls.
Model lanes and experimental retrieval remain explicit opt-ins. Independent
parent/file/area vectors exist under `hierarchy-v1`; the area bound is 256 records.
The normal Linux embedding worker includes the 8,192-token pooled-output repair.

Current evidence and limits:

The latest [file coverage/task continuation](validation.md#initial-file-coverage-and-task-evidence--2026-10-11)
adds unresolved Rust/Python/Go/JavaScript/TypeScript call candidates and attributed
Rust tests within existing composition budgets. Hierarchy preparation now publishes
an initial representation of every selected file before refining bodies, preserving
cache identity, changed-path scope and resident foreground limits. A cold real-model
run publishes initial views for all 274 admitted Foundry files; 783 vectors cover
784 of 14,453 representation locations at its stop. This is progressive coverage,
not completed body inference; dense retrieval remains 2/8. Lexical attribution now
checks selected document ranges before blaming excerpt construction. The 7B upstream host pair executes no edits and fails
both external checks; lower failed-attempt usage establishes no savings. Artifacts
are under `validation-runs/realistic-delivery-v10/` in the private tools directory.
CLI semantic slots now wait for worker cleanup before returning on success/error;
both paths pass with the real Linux worker, and successful delivery stays identical.

The latest [re-export/fitting continuation](validation.md#named-re-exports-and-current-fitting--2026-10-11)
supports named router re-exports in configured Python modules with every hop cited.
Boundary scopes now use `python-http-v7` and require republication. Acronym prose
matches singular/plural forms as one concept. Current `overview3` fitting covers
all 67,663 admitted Rust/Go sources across verified phases: 60,735–65,959 distinct
inputs and 14.65–15.86 million tokens. This has not completed corpus-wide inference.
Focused checks pass (556 tests), including bounded task proof delivery. Separate
real FastAPI re-export checks also pass. Further import forms/frameworks, general quality and economics remain.

The preceding [delivery/startup continuation](validation.md#evidence-delivery-and-unused-startup--2026-10-11)
adds exact local evidence from coarse vectors, preserves lexical priority under complete
file-overview coverage, fixes plural acronym queries and skips unused CLI model startup.
File/hierarchy recipes now use `+overview3`: rerun explicit preparation (or the already
opted-in maintainer); unchanged exact inputs reuse cached vectors. Short file inputs
skip parsing, and files without declarations get nonblank distributed extracts. Foundry's
own detector source no longer excludes itself; actual key markers remain rejected.

The real model completes all 274 admitted files of the pinned Foundry snapshot in
160.6 s; unchanged preparation makes zero document calls. Final semantic delivery is
still **2/8**, retaining lexical successes without establishing a general gain. Preserved
lexical results remain Foundry **2/8**, Rust **3/6** and **5/8**, Go **5/8**. One real
anchored CLI drops from 13.7 s to 1.5 s with identical output after skipping its unused
model; total task benefits remain unproven. Artifacts are under
`/workspace/.context-foundry-tools/validation-runs/evidence-delivery-v7/`.
The v6/v5 inventories below are historical; the current fitting result above also
does not establish full-model scale acceptance.

- **Retrieval:** experimental delivery remains 2/8 on Foundry, 3/6 on inspected Rust
  and 5/8 on additional public Rust questions. Two-passage selection and conservative
  language-qualifier disambiguation preserve every prior success; mixed-corpus Go
  delivers 5/8 with the final conservative repair. CLI/MCP semantic admission now shares the prose rules;
  a real partial-coverage query returns the target in 2,042 tokens. Traces distinguish
  excerpts from outlines. General ranking,
  per-file selection, macro evidence and independent acceptance remain open.
- **Scale:** the pinned Rust/Go corpus contains 69,565 regular files / 68,056 distinct
  contents. All 67,663 admitted sources index in 219.8 s and fit through a retained
  30-minute partial phase plus an 82-second verified continuation. Its 944,387 card
  locations require 782,716–831,067 distinct vectors, or 2.24–2.38 GiB of raw float
  payload before metadata/ANN storage. The default 2 GiB cache cannot complete this
  recipe. Real preparation/resume publishes 2,820 vectors, reusing all 2,678 cached
  inputs on resume; 67,631 sources remain unpartitioned. Cold provider calls take
  96.85% of elapsed. Full model coverage, useful serving and sustained churn remain open;
  fitting is not inference. The earlier repeated-family 80k lifecycle passes but
  represents only 528 distinct model inputs.
- **Evidence:** direct configured FastAPI package routers and literal Flask routes/
  Blueprints are supported. Actual FastAPI package and Flask dynamic-callback checks
  pass; observing-test edits invalidate imported observations after restart. Broader
  frameworks, collectors, cross-language callers and semantic requirement satisfaction
  remain open; explicit source-link/observed-line overlap is implemented. Indexing never executes a framework or test.
- **Task economics:** Qwen3-4B-Instruct-2507/OMP can make real edits. Both corrected
  parser patches pass their external oracle, but composed execution times out with
  greater known usage. Two further upstream percent-decoder pairs expose 18 unchanged
  public tests and compare prompt caching off/on. Every arm fails the external oracle;
  the cached composed arm makes no edits. No general task or total-cost benefit is
  established. Unknown usage and failed attempts remain in the record.
- **Native validation:** Linux passes tokenizer parity, 8,192-token inference under
  8 GiB and the new portable single/batched/full-window receipt. Strict CPU/Metal
  comparison and [native commands](measurement-handoff.md) are ready. No Metal receipt
  exists here; GPU resource/isolation and signed installed-package checks need a
  suitable runner. Model/representation quality still needs independent evaluation.

These are **not all macOS blockers**. Late chunking remains a conditional backend
candidate; native macOS CPU acceptance, global GPU headroom and useful generated-knowledge/
memory outcomes also remain open. 013 stays frozen off. Nothing is released or
promoted to a new default.

[Current validation](validation.md#initial-file-coverage-and-task-evidence--2026-10-11)
records raw artifact locations, frozen identities, failures and checks. Current files
are outside Git under `/workspace/.context-foundry-tools/validation-runs/`, with the
prior corpus, profiles, reports and transcripts in `quality-scale-economics-v5`.
Historical `/tmp` artifacts did not survive the environment restart. Restored tool
caches do not prove a clean installation from the onboarding draft; see [resume](resume.md).

The table below is the **2026-10-08 main-branch baseline**; later experimental
amendments supersede its completion and next-work descriptions. Nothing is released.

| Spec | State |
| --- | --- |
| 001 Reliable local context | T001–T009 done. The city map (T007 addresses and anchors, T008 15 more languages, T009 parallel indexing) passed G1 on all three full sets on 2026-10-08. |
| 003 Agent retrieval context | T001–T005 done (2026-10-01 to 10-04). |
| 005 Graph evidence | T001–T004 done; T004 doors plus the tie-group amendment (`doors:each`). |
| 007 Multi-workspace context | T001 done. |
| 008 Project memory | T001–T002 done. |
| 009 Semantic retrieval | T001–T004 done. T004 (address cards on a statically linked llama.cpp worker) is accepted on implementation evidence. On 2026-10-08, G2, G1 with the profile, and vector parity selected EmbeddingGemma 2. Nemotron was INVALID in G2 and closed by the owner. Semantic retrieval stays off by default until the installed-artifact checks pass on the signed package. |

013 is frozen off; 002, 004, 006, 011, 012 and 015 are superseded; 014 remains
deferred. 010's experimental reactivation is described above.

Validation for each item, with counts, is in [validation](validation.md). Key commits:
T007 `4e23dee`, T009 `89a8c54`, 005 T004 `d00fc19`, tie-group amendment `483810a`
(G1 PASS recorded in `1e0bb6a`), T008 `3e0a117`, 009 T004 `fd01489`, the cache cap as an
operator setting `8035faa` (`semantic prepare --cache-cap`, `mcp --semantic-cache-cap`),
and the measurement handoff `18e27f9`.

## Baseline release follow-up (2026-10-08)

1. **Signing, deferred.** This covers Developer ID signing and notarization of the macOS
   package and worker bundles (009 package acceptance, [deployment](deployment.md)). On
   2026-10-08 the owner deferred signing until the product is ready for release. No
   Apple Developer ID is needed before then, and nothing has been signed. After signing,
   run the installed-artifact checks (row 4 of the
   [measurement handoff](measurement-handoff.md)) with the EmbeddingGemma 2 profile.
   They decide whether the release packages and advertises semantic retrieval; they gate
   nothing else.
2. **Release.** The [release checklist](release.md) to a GitHub release on
   `dwbimstr/context-foundry` (owner-selected destination). The tag and the publication
   each need the owner's explicit go-ahead.
3. **Record-only timings** that the 18 GiB development host could not take without
   swapping: the preparation time on the library-only store, and a rust-lang/rust index
   run with the binary before T009. The owner either reruns them on a host with more
   free memory or retires them.

## Measurement results

All results are recorded in [validation](validation.md) under *Measurements on the
development host*. The raw results, logs and frozen identities are outside Git:
- `~/VSC_DEV/datasets/context-foundry-g2-runs`: `FROZEN.txt`, `runs/`, `g1/`, `prep/`,
  `rows/`.
- `~/VSC_DEV/datasets/context-foundry-timing`: scripts and `results/`.

## How work is done here

- **One crate, KISS.** Repairs amend the existing spec or contract; no new numbered
  spec for a fix. Private corpora, results, transcripts and weights stay outside Git.
- **Gates.** `scripts/gates.sh TREE OUTDIR` runs fmt, clippy, the full suite, and check
  and clippy under the Rust 1.90 toolchain. Run them once per merge, not mid-flight;
  during development run focused tests only. The full suite takes 16–20 minutes, mostly
  `tests/policy.rs`.
- **Docs.** `node scripts/check-links.mjs` from the repository root must report no
  errors before every commit that touches Markdown.
- **Review.** Every implementer change gets a cross-lab code review (a reviewer from a
  different model lab than the author) and a SHIP before merge. Design changes that
  matter get a refuter pair from two labs first. Agreement is not proof: the merging
  agent checks consequential evidence itself.
- **Isolation.** One writer per tree. Parallel slices use separate `git worktree`s and
  separate `CARGO_TARGET_DIR`s, and implementers do not commit; the integrating agent
  commits and pushes. Never run two heavy jobs (full suite, G1, preparation) at once on
  one host.
- **Measurement discipline.** G1 (`~/VSC_DEV/datasets/context-foundry-citymap`, frozen
  in `FROZEN-G1v4.txt`) and G2 apparatus are frozen before the code they judge and are
  never edited. Their verdicts fail closed: any retry, refusal, missing or duplicate
  task makes a run INVALID, and it is rerun on a quiet host, never accepted.
  `g1quick.sh SET BIN STORE OUT 1/8` gives an indicative sample in about a minute;
  only full sets (`1/1`) count as acceptance.

## Environment

- **Platform.** All development, gates and measurements ran on macOS arm64. CI also
  builds and tests the core on Linux (x86_64). `foundry-embed` (feature `embed-worker`)
  supports explicitly admitted Linux CPU and macOS Metal workers. Linux isolation
  and normal-worker numerical checks pass; the revised Metal worker still needs a
  native runner. The signed package target remains macOS arm64 (`tests/install.rs`
  builds only there); Linux is not a substitute for those installed-artifact checks.
- **Toolchains.** Rust stable plus the 1.90 toolchain (the MSRV gate). The worker build
  also needs `cmake` and a llama.cpp git checkout at commit `b9acf138` (`LLAMA_CPP_DIR`).
- **Network for dependencies.** Three tree-sitter grammars resolve from owner-approved
  forks pinned by git rev in `Cargo.toml` `[patch.crates-io]` (Ruby `1a594bf`, Perl
  `0686313`, Rust `d43cece`, all under `github.com/dwbimstr`); the first build fetches
  them. See [dependencies](dependencies.md).
- **Private inputs.** The corpora, frozen datasets, baseline stores and models are
  listed with their identities in the [measurement handoff](measurement-handoff.md).
  They are not in Git; the owner transfers them.
- **Memory.** The original sessions used a local Prakarana memory store; it is not part
  of the repository. The specs, [validation](validation.md) and Git history carry
  everything decided.

## Decisions a newcomer might otherwise reopen

- **City map.** A named identifier resolves to its definition, a directory of
  namesakes, or its doors (who uses it). Anchors and the resolver are in context-v2
  § City map; G1 judged T007, T008 and 005 T004 together.
- **Ambiguous names.** Per-namesake doors only for a resolver tie group of at most 4
  and only from exact compiler references. A "Head" form for ambiguous lists was
  rejected by two refuters: it would be a directory line that only the scorer's fence
  rule credits.
- **Grammar forks.** tree-sitter-rust 0.24.2 could not parse rustc's own nightly syntax,
  which is what failed G1's checker set before the fork. The Perl fork fixes a native
  memory leak; the Ruby fork fixes a heredoc serialization crash. All three were
  chosen by the owner over named limitations.
- **Embedding worker.** llama.cpp linked statically and built by `build.rs` from the
  pinned tree with `GGML_NATIVE=OFF` (portable across Apple silicon); no Python, no MLX.
  Dense retrieval serves only queries without an anchor; anchored queries never call
  the model.
- **Cache cap.** 2 GiB by default; an operator setting, not part of the profile or its
  function identity.
- **Measurements stayed on the development host** (owner, 2026-10-08). A cloud handoff
  was reversed because the baseline stores are too large to copy. G1 with a profile
  runs one shard at a time, because six shards with resident workers swapped the
  18 GiB host.
- **EmbeddingGemma 2 over Nemotron** (owner, 2026-10-08). Nemotron gained more in G2,
  but both of its runs were INVALID, so its result cannot be accepted. EmbeddingGemma 2
  passed every check and embeds about 3× faster.

## Known rough edges

- `tests/mcp.rs::http_dropped_stream_keeps_slot_and_overload_keeps_control_traffic_serviceable`
  assumes a request starts within 300 ms and can fail under heavy host load; it passed
  alone each time it was rerun.
- The G2 baseline without a profile delivers evidence on 0 of 388 questions: those
  questions name no identifier, which lexical retrieval cannot answer. That is the gap a
  semantic profile is meant to close, not an apparatus fault (a 6-question positive
  control passed).
- The first dense query on a large index can be refused once and then succeed on retry.
  Nemotron's G2 runs were INVALID for this reason on the 2.74 GB rust-lang/rust index.
  The refusal code was not captured; [inference] the likely cause is a read-deadline
  refusal while the index loads.
