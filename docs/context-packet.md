# Context packet: restarting this work elsewhere

Written 2026-10-08 so that a fresh agent (for example a cloud agent) can continue
without the original sessions, their memory store or their temporary directories. Read
it after [AGENTS.md](../AGENTS.md), the [constitution](../.specify/memory/constitution.md)
and the [portfolio](../specs/README.md). Where this file and a spec disagree, the spec
wins; this file only orients. To set up a new machine step by step (what to copy,
what to clone, how to prove the setup reproduces the recorded results), follow
[resume on another machine](resume.md).

## Where things stand

Every task of every active spec is implemented, cross-lab reviewed, gated and merged
on `main` (`dwbimstr/context-foundry`). Nothing is released.

| Spec | State |
| --- | --- |
| 001 Reliable local context | T001–T009 done. The city map (T007 addresses and anchors, T008 15 more languages, T009 parallel indexing) passed G1 on all three full sets on 2026-10-08. |
| 003 Agent retrieval context | T001–T005 done (2026-10-01 to 10-04). |
| 005 Graph evidence | T001–T004 done; T004 doors plus the tie-group amendment (`doors:each`). |
| 007 Multi-workspace context | T001 done. |
| 008 Project memory | T001–T002 done. |
| 009 Semantic retrieval | T001–T004 done. T004 (address cards on a statically linked llama.cpp worker) is accepted on implementation evidence. On 2026-10-08, G2, G1 with the profile, and vector parity selected EmbeddingGemma 2. Nemotron was INVALID in G2 and closed by the owner. Semantic retrieval stays off by default until the installed-artifact checks pass on the signed package. |

013 is frozen off; 002, 004, 006, 011, 012 and 015 are superseded; 010 and 014 are
deferred. None of these is release work.

Validation for each item, with counts, is in [validation](validation.md). Key commits:
T007 `4e23dee`, T009 `89a8c54`, 005 T004 `d00fc19`, tie-group amendment `483810a`
(G1 PASS recorded in `1e0bb6a`), T008 `3e0a117`, 009 T004 `fd01489`, the cache cap as an
operator setting `8035faa` (`semantic prepare --cache-cap`, `mcp --semantic-cache-cap`),
and the measurement handoff `18e27f9`.

## What remains, in order

1. **Signing.** Developer ID signing and notarization of the macOS package and worker
   bundles (009 package acceptance, [deployment](deployment.md)). This needs the owner's
   Apple Developer ID; nothing has been signed yet. After signing, run the
   installed-artifact checks (row 4 of the [measurement handoff](measurement-handoff.md))
   with the EmbeddingGemma 2 profile. They decide whether the release packages and
   advertises semantic retrieval; they gate nothing else.
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
  needs macOS with Metal, its worker isolation is macOS-specific, and the package
  target is macOS arm64 (`tests/install.rs` builds only there). A Linux host is not a
  substitute for the macOS gates or the measurements.
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
