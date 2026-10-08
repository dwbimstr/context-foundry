# Context packet: restarting this work elsewhere

Written 2026-10-08 so that a fresh agent (for example a cloud agent) can continue
without the original sessions, their memory store or their temporary directories. Read
it after [AGENTS.md](../AGENTS.md), the [constitution](../.specify/memory/constitution.md)
and the [portfolio](../specs/README.md). Where this file and a spec disagree, the spec
wins; this file only orients.

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
| 009 Semantic retrieval | T001–T004 done. T004 (address cards on a statically linked llama.cpp worker) is accepted on implementation evidence; whether any embedding profile is enabled is decided by the measurements below. Semantic retrieval is off by default. |

013 is frozen off; 002, 004, 006, 011, 012 and 015 are superseded; 010 and 014 are
deferred. None of these is release work.

Validation for each item, with counts, is in [validation](validation.md). Key commits:
T007 `4e23dee`, T009 `89a8c54`, 005 T004 `d00fc19`, tie-group amendment `483810a`
(G1 PASS recorded in `1e0bb6a`), T008 `3e0a117`, 009 T004 `fd01489`; the cache cap
became an operator setting on 2026-10-08 (`semantic prepare --cache-cap`,
`mcp --semantic-cache-cap`).

## What remains, in order

1. **Measurements.** Everything still needing a real model, timing or a quiet large
   host is in the [measurement handoff](measurement-handoff.md): G2 per embedding
   profile, G1 with a profile, vector parity, installed-artifact checks, preparation
   cost, T009 timing, door latency. They decide whether a semantic profile is enabled
   and advertised; they gate nothing else.
2. **Signing.** Developer ID signing and notarization of the macOS package and worker
   bundles (009 package acceptance, [deployment](deployment.md)). Needs the owner's Apple
   Developer ID; nothing has been signed yet.
3. **Release.** The [release checklist](release.md) to a GitHub release on
   `dwbimstr/context-foundry` (owner-selected destination). The tag and the publication
   each need the owner's explicit go-ahead.

## In flight when this was written

A local G2 run (agent `G2Run`, on the owner's machine) was left to finish. It prepares
both frozen profiles and runs G2 and then G1 with a profile, writing everything under
`~/VSC_DEV/datasets/context-foundry-g2-runs` (`FROZEN.txt`, `runs/`, `prep/`, `logs/`).
Its frozen inputs are K = 2 profiles (EmbeddingGemma 2 at 768 dimensions, Nemotron 3
Embed 1B converted offline to GGUF, parity min cosine 0.999691). Before using any of its
results, check its final report and `FROZEN.txt`: a result counts only if its binary,
profiles and stores match the frozen identities and the run was not INVALID. One early
EmbeddingGemma G2 result (139 questions gained, 0 lost) ran on a binary that predates
the cache-cap change and was discarded by the run itself; it is indicative only.

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

- **Platform.** All development, gates and measurements so far ran on macOS arm64.
  `foundry-embed` (feature `embed-worker`) needs macOS with Metal, and its worker
  isolation is macOS-specific. The core has not been built or tested on Linux; a Linux
  host is not a substitute for the macOS gates or the measurements.
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
- **Measurements moved off the development host** (owner, 2026-10-08), because that
  18 GiB host swapped under parallel G1 shards with resident workers.

## Known rough edges

- `tests/mcp.rs::http_dropped_stream_keeps_slot_and_overload_keeps_control_traffic_serviceable`
  assumes a request starts within 300 ms and can fail under heavy host load; it passed
  alone each time it was rerun.
- The G2 baseline without a profile delivers evidence on 0 of 388 questions: those
  questions name no identifier, which lexical retrieval cannot answer. That is the gap a
  semantic profile is meant to close, not an apparatus fault (a 6-question positive
  control passed).
- Exact-door latency for common names with tie groups (up to 4 references windows per
  root) is not yet measured.
