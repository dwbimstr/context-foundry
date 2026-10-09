# Measurement handoff

On 2026-10-08 the owner first moved the remaining real-model and timing measurements
to a cloud agent, then reversed that decision the same day: the baseline stores are too
large to copy. Rows 1–3, 5, 6 and 7 below ran on the development host (Apple silicon,
18 GiB), one job at a time, with results recorded in [validation](validation.md).

The selected profile is EmbeddingGemma 2. Nemotron 3 Embed 1B's G2 runs were INVALID,
and the owner closed it.

What remains is row 4, which needs the Developer ID-signed package, and two
record-only timings that this host could not take without swapping. Every task below
is implemented, reviewed and gated. This file lists only what is still owed, what each
measurement decides, and how to run it.

## While this handoff is open

- Semantic retrieval stays optional and off by default. Until row 4 passes, a release
  package is built without `--with-semantic` and advertises no semantic retrieval, just
  as learning is not advertised while 013 is frozen off ([release](release.md)). The
  record-only rows never gate packaging.
- Nothing here changes code. Each result is recorded as counts in
  [validation](validation.md) and in the owning spec's status line. Results,
  responses, transcripts, models and private datasets stay outside Git.
- Frozen inputs are never edited. A run that breaks a fail-closed rule (any retry,
  refusal, missing or duplicate task) is INVALID and is rerun; it is never accepted. A
  timing run with swapouts is INVALID too.

## Host

- macOS on Apple silicon with Metal. The worker, its static llama.cpp build and every
  baseline binary are macOS arm64 builds; the installed-artifact rows also need the
  Developer ID-signed package.
- At least 32 GiB of memory. One owner holds about 1 GiB of index for rust-lang/rust;
  each embedding worker takes 1.6–3 GiB (its ceiling is 3 GiB). Run one measurement
  job at a time, and run G1 with a semantic profile at `SHARDS=1` unless memory covers
  six owners and six workers.
- Rust stable and 1.90 toolchains, `cmake`, `git` and `python3`; about 20 GB free.

## Inputs

All of these live outside Git on the owner's machine and are transferred privately
under the owner's authority; paths are given relative to `~/VSC_DEV`. The corpora are
public sources; the datasets, results and converted model files are not redistributed.
The stores are bound to the absolute corpus paths under `/Users/satishlomte/VSC_DEV`;
[resume on another machine](resume.md) gives the exact transfer, clone and
verification steps.

| Input | Location | Identity |
| --- | --- | --- |
| Context Foundry | this repository | main, at or after the cache-cap commit |
| rust-lang/rust source | `corpora/rust-1.99.0`, or a clone of `github.com/rust-lang/rust` at tag `1.99.0` without submodules | commit `b940084d` |
| oh-my-pi source | `oh-my-pi`, or a clone of `github.com/can1357/oh-my-pi` (no `node_modules`) | commit `5b8d5b8a15` |
| G1 apparatus and baseline runs | `datasets/context-foundry-citymap` | `FROZEN-G1v4.txt` (`g1v4.py` `5f7dac42…`, `g1score.py` `5dc1c5e8…`, `g1compare.py` `458a8c0d…`, `g1quick.sh` `dc2e96e9…`, `runs-SHA256SUMS` `9cee37e2…`) |
| G1 baseline stores, SCIP artifact, baseline binary, o200k counter | `datasets/context-foundry-citymap/inputs` | `store-rust-g1base`, `store-bun-g1base`, `index.scip`, `foundry-baseline` (`SHA256SUMS`), `o200k` |
| G2 questions | `datasets/context-foundry-g2/questions.jsonl` | `e434d569…` (388: 197 rust, 191 bun) |
| G2 harness | `datasets/context-foundry-g2-harness/g2.py` | `6cf3e1bc…` |
| G2 candidate profiles, wrappers, Nemotron conversion | `datasets/context-foundry-g2-runs` | `FROZEN.txt` of 2026-10-08T11:31:14Z: K = 2, `gemma.json` `bb1c9d08…`, `nemotron.json` `619205ad…` |
| EmbeddingGemma 2 | `datasets/context-foundry-g2-runs/artifacts/models/gemma` (from `models/embeddinggemma-2-GGUF-bfcd2987` and the tokenizer of `models/embeddinggemma-2-914f7f89`) | GGUF `68bae29d…`, tokenizer `4d777ef5…` (Apache-2.0) |
| Nemotron 3 Embed 1B | `datasets/context-foundry-g2-runs/artifacts/models/nemotron`, converted offline from `models/Nemotron-3-Embed-1B-BF16` by `nemotron-conversion/convert_noncausal.py` | GGUF `e0cd7ff3…`, tokenizer `797410df…`; parity against the torch reference min cosine 0.999691 (OpenMDW-1.1) |
| llama.cpp | public `ggml-org/llama.cpp` | commit `b9acf138`; `build.rs` builds it |

`g2.py` reads the token counter at `/private/tmp/cf-o200k/target/release/o200k_count`;
place the preserved `o200k` directory there (or build it) before a G2 run. Stores are
copied with `cp -cR` and brought to the current schema with `foundry repair-index
--store <copy>`.

## Measurements

| # | Measurement | Owning spec | Decides | Status (2026-10-08) |
| --- | --- | --- | --- | --- |
| 1 | G2 per candidate profile | 009 T004 *G2 and enablement* | whether a profile may be enabled for anchor-less queries | EmbeddingGemma 2 ENABLE; Nemotron INVALID twice, closed by the owner |
| 2 | G1 with each profile that passed G2 | 009 T004 *G2 and enablement* | same; all targets and guards must still pass | EmbeddingGemma 2 PASS |
| 3 | Batched against single-sequence vectors | 009 T004 Verification | numerical acceptance of the worker | EmbeddingGemma 2 PASS (development bundle) |
| 4 | Installed-artifact checks | 009 T004 Verification | advertising semantic retrieval in a signed release | **deferred**: needs the Developer ID-signed package; the owner defers signing until the product is ready for release |
| 5 | Preparation cost before and after cards | 009 T004 Verification | record only | counts recorded; time not accepted (every attempt swapped) |
| 6 | Index time and peak memory before and after T009 | 001 T009 Verification | record only | oh-my-pi recorded; rust-lang/rust before T009 not valid (every attempt swapped) |
| 7 | Door latency for tie groups | 005 T004 amendment | record only (not an acceptance criterion) | recorded |
| 8 | Rollback re-embedding under a pre-T004 binary | 009 T004 *Data cutover* | needed only if a pre-T004 build was ever distributed | not needed: no pre-T004 build was distributed |

1. **G2.** First a baseline without a profile. Then, for each frozen profile, run on a
   store prepared with that profile:
   - Prepare: `foundry semantic prepare --profile P --store S --budget-seconds 3600
     --development-isolation`, repeated until `partial` is false.
   - Run `g2.py run` with the frozen wrapper, then `g2.py compare --profiles 2`.

   Nemotron's 2,048-dimensional rows for the full rust-lang/rust store (332,732 cards)
   need 2.74 GB, above the 2 GiB default. Prepare with `--cache-cap 4294967296` and
   serve with `mcp --semantic-cache-cap 4294967296`.

   Pass: delivered evidence on significantly more questions (exact one-sided McNemar
   p < 0.025) at no more mean delivered tokens.
2. **G1 with a profile.** For each profile that passed G2: `g1quick.sh SET WRAPPER STORE
   OUT 1/1 1` for `checker`, `qualified` and `bun`, the wrapper adding the profile to
   every `mcp`. Pass: every target and guard that passes without the profile passes
   with it (the no-profile verdict is PASS, [validation](validation.md)).
3. **Parity.** `CF_EMBED_DEV_PROFILE=<profile> cargo test --locked --test embed_worker
   -- --ignored dev_real_worker_batched_and_single_sequence_vectors_agree` for each
   profile; the test starts the worker bundle the profile names. Pass: cosine ≥ 0.9999
   for every input. The worker is built with `GGML_NATIVE=OFF`, so this also covers its
   portable CPU kernels.
4. **Installed artifact** (after Developer ID signing). Package with `--with-semantic
   --semantic-profile` and the frozen EmbeddingGemma 2 profile, then install it. On the
   installed bundle, check that:
   - `otool -L` lists only system libraries and frameworks;
   - the isolation profile and the 3 GiB ceiling hold ([deployment](deployment.md));
   - row 3 passes when rerun against it.
5. **Preparation cost.** On the library-only rust-lang/rust store (the 2,255 files of
   `library/`), record time, peak footprint and tokens embedded under cards. The counts
   and footprints are recorded. Still owed: an elapsed time from a run without
   swapouts. Each attempt here needed about 2.4 GB for the foundry and its worker. The
   earlier body-unit figures are in [validation](validation.md) (15,026 units, 6,889 s
   with Nemotron on MLX).
6. **T009 timing.** Index oh-my-pi (8,436 files) and rust-lang/rust into fresh stores
   with the binary before T009 (the parent of `89a8c54`) and with main, alone on an
   idle host, under `/usr/bin/time -l`, and record wall time and peak memory. Still
   owed: a rust-lang/rust run with the before-T009 binary, without swapouts. The build
   and run scripts are in `datasets/context-foundry-timing` (`build.sh`, `t009.sh`,
   `README.md`).
7. **Door latency.** On the rust-lang/rust store with its SCIP graph (exact doors) and
   on oh-my-pi (approximate doors), p50 and p95 of `context` for `who calls` queries
   whose first anchor has a tie group of 2–4 definitions, against the same queries'
   resolved-anchor latency.
8. **Rollback re-embedding.** The no-model half passed on 2026-10-08. The re-embedding
   half needs the pre-T004 binary with its descriptor v1 profile, the MLX 4-bit
   artifact (`models/Nemotron-3-Embed-1B-BF16-4bit-d0408b94`) and its Python runtime;
   no pre-T004 build was released, so run it only if one was distributed.

## Closing the handoff

Record each row's counts in [validation](validation.md) and update the owning spec's
status line. When row 4 passes, amend the release checklist to build the package with
`--with-semantic` and the EmbeddingGemma 2 profile. Remove this file when row 4 is
recorded and the owner has either rerun or retired the two missing timings.
