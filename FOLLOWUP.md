# Follow-ups that cannot run in a cloud session

This list holds the remaining work on the
[experimental context ecosystem](docs/resume-context-ecosystem.md) that the 2026-10-12
Claude Code cloud session could not do. That session had:

- 4 vCPUs, 15.7 GiB of RAM, no swap and about 30 GB of disk;
- no GPU, no private resume archive and no GitHub Actions;
- no access to GitHub archives of repositories outside the session (the proxy returns
  403 for `rust-lang/rust` and `golang/go`).

Its VM also restarted mid-session onto a CPU without `avx512_bf16`, so the revision-5
Gemma worker refused to load. Each entry below names the blocker, the resource that
removes it and the exact way to resume. Everything that could run there has run; its
results are in [validation](docs/validation.md#cloud-continuation--2026-10-12).

Do not work around a blocker by changing a model, threshold, question set or protocol.

## F1. Re-run CI for commits made while Actions was unavailable

- **Why blocked:** GitHub Actions is out of budget (owner decision 2026-10-12). Workflows
  are never dispatched, re-run or waited on until the owner says the budget is back.
- **Required resource:** GitHub Actions budget.
- **Resume:**
  1. The last commits with CI are `e0a1458` for Rust CI
     ([38207948806](https://github.com/dwbimstr/context-foundry/actions/runs/38207948806))
     and `7e7176b` for native macOS
     ([38209502529](https://github.com/dwbimstr/context-foundry/actions/runs/38209502529)).
     List the later commits with `git log --oneline 7e7176b..HEAD`. They carry `[skip ci]`.
  2. Run `rust.yml` on the current head of each branch merged since then. Use a push
     without `[skip ci]` or `gh api -X POST .../actions/workflows/rust.yml/dispatches`
     once the workflow accepts dispatch.
  3. Dispatch `native-macos.yml` with `scale_calibration=true` on the same head.
  4. Compare each result with the local gates receipt in
     [validation](docs/validation.md#local-gates-receipts). Cite the CI run beside the
     receipt; do not delete the receipt.
  5. Apply the workflow action updates that were closed while Actions was unavailable,
     and verify them with the re-run:
     - `actions/checkout` v4 → v7 (`3d3c42e5…`, v7.0.1) in all three workflows
       (`rust.yml`, `native-macos.yml`, `learning-worker.yml`). PR #4 bumped only
       `rust.yml`.
     - `actions/upload-artifact` 4.6.2 → 7.0.2 (`cf430e03…`) in `native-macos.yml`,
       from PR #14.

     Dependabot replied on both PRs that it will not propose those exact versions again.
     Apply them by hand, or take the next newer upstream release when Dependabot offers
     it.

## F2. Stronger-model host pair (resume step 5)

- **Why blocked:**
  - The prepared runner streams the pinned 18,556,689,568-byte Qwen3-Coder-30B-A3B
    Q4_K_M GGUF into a sealed memfd and serves it with a 32K context on one CPU server.
    That exceeds a 15.7 GiB host with no swap.
  - The runner and its `next-run.md` are private, under
    `V11/stronger-model-research/` in the resume archive.
- **Required resource:** the checkpoint sandbox (32 GiB RAM) or another host with at least
  32 GiB, plus the restored archive.
- **Resume:**
  1. On that host, read `V11/stronger-model-research/next-run.md` and run it unchanged.
  2. Keep its budgets: 4,200 s outer, 900 s per task.
  3. Memory admission, OOM, swap and pressure checks, the exact model and template, and
     genuine native tool calls must all pass.
  4. Keep every earlier negative or unknown cost.
  5. Compare costs only after both unchanged external oracles, real edits and complete
     accounting pass.
  6. Do not change the model without the owner.

## F3. Task economics (the economics gate)

- **Why blocked:** a completion benefit needs a correct task completion first. That
  depends on F2.
- **Required resource:** F2 completed.
- **Resume:** after F2, account for every cost: bootstrap, acquisition, retrieval,
  maintenance, and all failed and unknown attempts. The selected history has 71 requests,
  of which 66 are complete and five unknown. A smaller failed prompt is not a benefit.

## F4. Sealed independent 48-question evaluation (resume step 6)

- **Why blocked:** the sealed set is private and lives only in the resume archive. Root
  has never read its questions or labels.
- **Required resource:** the restored archive, and the owner's explicit freeze of the
  final candidates.
- **Resume:**
  1. Verify these identities. Verifying a hash does not authorize reading the labels.
     - manifest `01a9ad47473c0b99f82bee5f2fabf67cf96c86134e9627c63bd50cb67d80d429`
     - seal `5d599fb9d13a9950233491dd8c4881ef44cbcdbb06287ba64cd0bcc0c836da22`
     - protocol `7bb60b43908269410cfdb29bc35086f9da684d9c0d480086907ee3c97c9d02a0`
  2. Freeze at most three candidates in writing before releasing the set. The current
     ones are the default lexical delivery and the experimental hierarchy placement
     recorded in [validation](docs/validation.md#cloud-continuation--2026-10-12).
  3. Keep the protocol: 2,048-token budget, a paired test with a simultaneous interval,
     and the token, resource and latency guards. Acceptance needs at least 13 gains and
     zero losses out of 48.

## F5. Private G1/G2 question sets

- **Why blocked:** the G2 set (388 questions) and the G1 responses are outside the
  repository and absent from this session.
- **Required resource:** the owner's private G1/G2 inputs.
- **Resume:** follow the frozen G2 protocol in [validation](docs/validation.md). It
  allows one `context` call per question, uses the frozen G1 evidence parser and fails
  closed on integrity errors. No G2 measurement has run.

## F6. Frozen Rust and Go regression question sets

- **Why blocked:** the label files for the Inspected Rust (6), Additional Rust (8) and
  Mixed-corpus Go (8) sets were frozen in private `retrieval-final-*` directories. Only
  the eight Foundry questions are in Git, as `CASES` in `tests/retrieval_attribution.rs`.
  Writing new labels would create new, author-visible sets, not the frozen ones.
- **Required resource:** the restored archive.
- **Resume:** run `tests/retrieval_attribution.rs` with `FOUNDRY_ATTRIBUTION_ROOT`,
  `FOUNDRY_ATTRIBUTION_LABELS` and `FOUNDRY_ATTRIBUTION_STORE` pointing at each frozen
  corpus, its labels file and its store. Compare the current experimental placement with
  the `fe37eff3` baseline under the 2,048-token budget. Every earlier delivered span must
  survive.

## F7. Native macOS acceptance beyond the CI runner

- **Why blocked:** this needs a physical Apple-silicon Mac, for performance, global GPU
  pressure and combined embedding and generation residency, plus a Developer ID signing
  and notarization identity for the signed installed package. The CI runner is Apple
  Paravirtual hardware, and Actions is unavailable (F1).
- **Required resource:** a physical Apple-silicon Mac, and the owner's signing identity.
- **Resume:**
  1. On the Mac, run
     `python3 scripts/native-macos-acceptance.py --out /abs/new-out --work /abs/new-work.noindex --generation --scale-calibration`.
  2. Spotlight skips folders whose names end in `.noindex`. Spotlight indexing of build
     output caused the earlier CPU-pressure failures. Do not turn Spotlight off on a
     personal Mac.
  3. For the signed package, follow [release](docs/release.md) and
     [deployment](docs/deployment.md).

## F8. Private capsule patches and drivers

- **Why blocked:** they exist only in the resume archive, under `V11/resume-capsule/`.
- **Required resource:** the restored archive.
- **Resume:**
  - `foundry-document-body-rrf/`: compare its unfinished patch with the independent
    implementation and result recorded in
    [validation](docs/validation.md#cloud-continuation--2026-10-12). Apply nothing that
    duplicates or contradicts the recorded decision without a new measurement.
  - `foundry-native-cpu-diagnostics/`: superseded. Spotlight indexing caused the
    CPU-pressure failure, and the workflow now stops Spotlight. Archive the patch unless
    a new failure appears.
  - `linux-batch-resume-handoff.md` and the batch-aware full driver: compare them with
    the public harness this session used for resume steps 3 and 4, and keep the stricter
    of the two checks.

## F9. Measure the document/body RRF candidate (resume step 2)

- **Why blocked:** the real-model comparison needs the revision-5 AVX-512 BF16 Gemma
  worker. The session's host lost `avx512_bf16` when its VM restarted, and the
  embedding function must not be swapped. Everything before the dense run is done:
  - the candidate is implemented, tested and committed as `9ccefcc`, then reverted by
    `ef524b9`;
  - the frozen corpus is in Git (`ebf28f8`);
  - the lexical baseline reproduces: default 1/8, experimental 2/8, 274 admitted
    sources.
- **Required resource:** an x86_64 host whose `scripts/cloud/doctor.sh` reports
  `cpu avx512_bf16: yes`, with about 12 GB of free RAM and Hugging Face access. A cloud
  session works if its VM lands on such a CPU; the sandbox does.
- **Resume:**
  1. Extract the corpus: `git archive ebf28f8 | tar -x -C $CORPUS`. The SHA-256 of the
     uncompressed tar stream is
     `31ff389a41d92f399f1099547198088310b4163671866222493ba3438433f7c2`.
  2. At `ef524b9` (the interleave arm), build:
     ```sh
     cargo build --release --locked --features embed-worker-avx512-bf16 --bin foundry-embed
     cargo build --release --locked --bin foundry
     ```
     Copy both binaries aside as the baseline.
  3. Put the pinned `embeddinggemma-2-BF16.gguf` (`68bae29d…`) and `tokenizer.json`
     (`4d777ef5…`) in one model directory. Write the profile:
     - `v: 2`, `representation: {"kind":"hierarchy-v1","parent_tokens":64}`,
       `card_tokens: 512`, `batch: 8`;
     - `memory_ceiling_bytes: 8589934592`, `load_timeout_seconds: 120`,
       `cpu: {"threads":4,"seconds":86400}`;
     - `query_template: "task: search result | query: {text}"`;
     - a descriptor with `adapter_revision: 5`, `dimensions: 768`, `pooling: "mean"`,
       `output: "f32"`, `document_template: "title: none | text: {text}"`,
       `tokenizer: "tokenizers 0.23.2 onig"`, `add_special_tokens: true`,
       `llama_cpp: "b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea"`, and
       `geometry: {"v":1,"input_tokens":8192,"batch_tokens":8192,"sequences":8}`.

     Then run `scripts/embed-worker-bundle.sh --binary <foundry-embed> --out <bundle> --profile <profile>`
     and set `worker.executable_sha256` to the bundled binary's SHA-256.
  4. Create the store and score the lexical baseline. This also deletes the evaluator
     source, so it must happen before preparation:
     ```sh
     FOUNDRY_ATTRIBUTION_DIR=<new> FOUNDRY_ATTRIBUTION_ROOT=$CORPUS FOUNDRY_ATTRIBUTION_STORE=<store> \
       cargo test --release --locked --test retrieval_attribution repository_development_attribution \
       -- --ignored --exact --nocapture
     ```
     Expect 274 admitted sources, default 1/8 and experimental 2/8.
  5. Prepare the store with the baseline binary:
     `foundry --store <store> semantic prepare --profile <profile> --budget-seconds 7200 --development-isolation`.
     Repeat until `semantic status` reports complete coverage. Run nothing else heavy
     meanwhile.
  6. Check out a scratch branch at `ef524b9`, run `git cherry-pick 9ccefcc` and rebuild
     `foundry`. Rerun step 4's command against the same store, adding
     `FOUNDRY_ATTRIBUTION_SEMANTIC_PROFILE=<profile>` and
     `FOUNDRY_ATTRIBUTION_BASELINE=<baseline foundry>`. Both arms then run on one store
     with a 2,048-token budget.
  7. Keep the candidate, by reverting `ef524b9`, only if it delivers more expected spans
     than the interleave arm and loses none that the interleave arm delivers. Otherwise
     leave it reverted. Record either result in validation. F6's private Rust and Go
     sets must also pass before any retrieval acceptance claim.

## F10. Linux batch-one/eight calibration (resume step 3)

- **Why blocked:** this step needs three things:
  - the revision-5 worker, which needs `avx512_bf16`;
  - the pinned Rust 1.90.0 and Go 1.25.0 archives, which the session's proxy refused
    (403 from `codeload.github.com`);
  - optionally, the private `linux-batch-resume-handoff.md` exporter.

  `tests/native_scale.rs` is macOS-only, so Linux needs that exporter or a port of the
  test.
- **Required resource:** the sandbox, which has the CPU, the restored corpus and the
  handoff.
- **Resume:** follow resume step 3 unchanged. The 128 files come from
  `scale_sample()` in `scripts/native-macos-acceptance.py`; it verifies the sample
  SHA-256 `76b510031972d619b01c0f767b5c0d66c6da4899ff8628c62ff9943a7b1b7a26`. The Mac
  sample manifest is `a59c9a1e08ca36dd2baa1449420b4b803671db6689d18de8835510d6fbf221ad`
  (native run 38209502529). The fixed selection requires:
  - identical keys, tokens and locations, and zero-call replay;
  - cosine at least 0.9999, with normalized, finite vectors;
  - at least 1.10x provider throughput, and at most 1.05x preparation wall time;
  - four threads, an 8 GiB worker ceiling, 600 s per arm and 1,500 s in total.

## F11. Full Linux overview of 67,663 sources (resume step 4)

- **Why blocked:** the same two blockers as F10 (`avx512_bf16` and the archives). The
  run also needs about three provider-hours plus overhead in one uninterrupted job.
  A cloud session's VM can pause or move, which makes such a run invalid.
- **Required resource:** the sandbox, and F10's selected batch size.
- **Resume:** follow resume step 4 unchanged:
  - a fresh store, 512-token overviews and an explicit 512 MiB cache;
  - the same revision-5 function and F10's batch;
  - a declared wall budget, with partial phases preserved;
  - at least 2.75 GiB free before starting, and stop before the disk reserve.

  `full_corpus()` in `scripts/native-macos-acceptance.py` verifies the 69,565-file
  population. Completing the overview does not establish body coverage.

## F12. `sha2` 0.11 migration (done 2026-10-13; kept as the site record)

- **Status:** done in `16093de` and `41e72b4`. Nothing is blocked. This entry records
  the sites in case the encoding is revisited.
- **What changed:** `sha2` 0.11's digest type has no `LowerHex`. All 18 SHA-256 hex sites,
  which produce source hashes, cache keys and artifact pins, now use
  `context_foundry::lower_hex`; the 3 test oracles encode inline:
  - library: `src/lib.rs:78` (`digest`), `src/scip.rs:718`, `src/decision_model/net.rs:709`,
    `src/learning.rs:1466`, `src/learning.rs:3005`, `src/learning/candidate.rs:292`,
    `:322`, `:410`, `:472`, `src/learning/ipc.rs:208`, `src/neural/index.rs:393`, `:453`,
    `src/neural/profile.rs:721`, `src/knowledge/local/supervisor.rs:70`;
  - binary: `src/bin/foundry_embed.rs:634` (feature `embed-worker`);
  - test oracles: `tests/economics.rs:392`, `tests/embed_worker.rs:53`, `tests/usage.rs:66`.

  Line numbers are as of `16093de`. `tests/gateway.rs:1088` also uses `{:x}`, but for
  HTTP chunk framing, not a digest.
- **Proof of byte identity:** on 0.10, `lower_hex(d) == format!("{:x}", d)` held for every
  input length 0..=300. Pinned FIPS 180-2 vectors pass on both 0.10 and 0.11
  (`hex_tests` in `src/lib.rs`). See
  [validation](docs/validation.md#sha2-011-migration--2026-10-13).
- **Remaining:** `sha2` 0.10.9 is still compiled under the optional `learning-worker`
  feature, via `tch` → `zip` → `pbkdf2`. It disappears when that chain moves.

## F13. Continuous semantics stops on a stale-source partition race

- **Why parked:** it is a product change in the concurrent semantic driver, and the fix
  below is a design awaiting owner approval (2026-10-13). It was found while
  investigating `continuous_semantics_progresses_during_filesystem_traffic_and_converges`
  (`tests/neural_retrieval.rs`), which fails about 1 run in 6 under the full parallel
  suite on 4 vCPUs. Nothing else blocks it in a cloud session.
- **Diagnosis:** `Run::partition_wave` (`src/neural/driver.rs`) reads each source under
  the engine slot, releases the slot while the partition workers compute, then retakes
  it for each `commit_partition` (`src/neural/prepare.rs`), which calls
  `Engine::semantic_accept_partition` (`src/neural/cache.rs`). The owner lets pending
  watcher steps run before the driver (`try_primary` in `src/mcp.rs`), so under churn a
  source edited during the compute is usually stale at accept. Accept refuses it with
  `partition_invalid: <path>: the mapping names a stale source version`; the error
  leaves `maintain`, `finish` records `paused` with that `last_error`, continuous mode is
  switched off and the driver thread exits with the paths still pending. It never
  converges, so a longer deadline would only hide the defect; the test's bound is not
  the cause. Details:
  - Only the accept-time check fires in the MCP owner: `semantic_partition_source` reads
    in the same slot step as the source page, and every writer takes that slot.
  - A source deleted in the same window fails the same way with `not_found`, which the
    state row records as `internal`.
  - A one-shot MCP `index {semantic: "prepare"}` runs the same wave and can stop the same
    way. The CLI `semantic prepare` cannot: it holds the store exclusively.
  - `area_pass` has the same shape: `stale_area`, when a brief changes during
    preparation, also ends in `paused`. It was not observed.
- **Proposed fix (design only):** treat a superseded source as a normal outcome. No new
  requeue is needed: `replace_source` and `delete_source` record the new version in the
  semantic obligation table in the same transaction (`work::changed`), and
  `derivation_ack` removes a path only when its stored version equals the batch's. A
  skipped path therefore stays pending, and the next `maintain` iteration, or the
  one-shot pass's revision check, partitions it again.
  1. `Engine::semantic_accept_partition` returns committed or superseded (hash differs,
     or source gone) instead of an error.
  2. `commit_partition` passes that through and counts `partitioned_sources` only on a
     commit. The CLI `run` keeps today's error, which it cannot reach.
  3. `Run::partition_wave` drops a superseded partition and its retained cards and
     continues; the pause and cancel checks before each commit stay as they are.
  4. The public `Engine::semantic_record_partition` still refuses a stale hash with
     `partition_invalid`, so its contract in `tests/neural.rs` holds. The structural
     `partition_invalid` checks (bounds, order, UTF-8, keys, rendering) are untouched.

  Rejected: restarting the wave from a fresh batch (it discards the wave's other work
  and can starve unrelated paths under steady churn), and serializing partition and
  commit against the watcher (it reverses the 009 T003 slot decision and breaks the
  driver test that requires partition CPU work to release the serving slot). A path
  rewritten faster than its own partition compute is retried each pass; the watcher's
  debounce (75 ms quiet, 1 s maximum) bounds that. Whether `stale_area` gets the same
  treatment is a separate owner call.
- **Required resource:** none beyond a normal session; owner approval of the design.
- **Resume:**
  1. Red first, deterministic: in the `src/neural/driver.rs` tests, arm
     `PARTITION_BEFORE_COMPUTE` with an `Action::Call` that replaces the source being
     partitioned (`engine.replace_source`), as the existing slot-release test does, and
     call `partition_wave`. Assert `Ok`, no partition row for that path, its obligation
     still pending with the new version, and the wave's other sources committed. Today
     it returns `partition_invalid`. Add the deletion variant (`delete_source`; today
     `not_found`) and a mid-wave variant (`PARTITION_AFTER_COMMIT` on the first source
     replaces the second).
  2. Drive `maintain` through the same fault and assert state `waiting`, no pending
     paths and a current partition for the new hash.
  3. Implement fix steps 1-3; keep the `tests/neural.rs` stale-hash refusal and the
     structural `partition_invalid` tests unchanged.
  4. Soak `cargo test --locked --test neural_retrieval` about 12 times with deadlines
     unchanged, then run the full local gates. Update the driver module doc ("a
     superseded partition is dropped and stays pending") and
     [validation](docs/validation.md).

## F14. God-module splits (audit 2026-10-13)

- **Why deferred:** each split is a separate, gated, revertable commit. On 2026-10-13
  only the two splits that were purely mechanical (verbatim moves, no visibility
  change, public API unchanged) ran; see [validation](docs/validation.md#god-module-splits--2026-10-13).
  The rest change crate-internal visibility, split test modules, touch macOS-only or
  feature-gated code that Linux gates do not compile, or collide with pending work.
- **Audit:** a god module here is large and multi-responsibility. Production lines
  exclude in-file test modules; fan-in counts other `src` files naming the module
  (plus test files); churn counts commits touching the file in the shallow clone's
  152 commits since 2026-10-06. Figures are at `a088969`, before the splits below.

  | File | Prod. lines | Fns | Concerns | Fan-in (src + tests) | Churn | Hot path | Verdict |
  | --- | --- | --- | --- | --- | --- | --- | --- |
  | `src/store.rs` | 7,586 | 196 | 12+ (lifecycle, upgrade, scans, writes, indexing pool, lexical and semantic search, context, doors, retrieve, repair, query parsing, result types) | 30 + 22 | 44 | yes | god module |
  | `src/mcp.rs` | 4,622 | 127 | ~14 (owner state, slot, ops, results, args, tools, delivery, Owner impls for three subsystems, transports, HTTP gate, startup, CLI semantic path) | 4 + 7 | 25 | yes | god module |
  | `src/syntax.rs` | 4,857 | 113 | ~15 (languages, bounded parse, scanner limits, unit rules, outline, calls, imports, Markdown, forest, tiling, tokenizers, paths) | 11 + 7 | 17 | yes (indexing) | god module |
  | `src/learning.rs` | 3,014 | 87 | ~13 (feedback, strict JSON, route state, store surface, run policy, dataset prepare and check, output publication, renderer) | 8 + 9 | 1 | no | god module |
  | `src/graph.rs` | 2,955 | 65 | 3 (edge graph, compiler facts, references and doors) | 8 + 8 | 6 | yes (references) | borderline |
  | `src/neural/query.rs` | 1,654 | 61 | 3 (refinement, provider-thread runtime, dense index) | 5 + 3 | 18 | yes | borderline |
  | `src/neural/prepare.rs` | 2,319 | 67 | 3, already layered (CLI loop and lifecycle, scheduling, step machinery) | 3 + 2 | 19 | yes | borderline |

  **Not god modules** (large but one responsibility; do not target them):
  `src/gateway.rs` (one subcommand, one entry point), `src/scip.rs` (one import
  pipeline), `src/response.rs` (one wire renderer), `src/cli.rs` (the command
  dispatcher), `src/testkit.rs` (test-faults support), the `tests/*.rs` integration
  suites (4,400–8,760 lines each), and the llama.cpp bindings, which `build.rs`
  generates into `OUT_DIR` and are not in the tree.
- **Done (2026-10-13):** `14729de` moved the tool layer of `src/mcp.rs` to
  `src/mcp/tools.rs`, and `9a86665` moved context assembly from `src/store.rs` to
  `src/store/context.rs`. Both are verbatim moves with `use super::*;` in the child and
  no visibility change; see validation for the evidence.
- **Remaining plan**, each step one commit, in this order within a file, gated as
  below. "Bumps" are crate-internal visibility changes (`pub(super)`); none changes
  the public API.

  | File | Next splits (bumps) | Precondition or blocker |
  | --- | --- | --- |
  | `src/store.rs` | `retrieve` (0), `index_hooks` inline module to a file (0), `sources` (0), `indexing` (0; declare after the `index_fault!` macros), `lifecycle` (0; +2 to separate repair), `doors` (1), `semantic_search` (1, `#[cfg(feature = "semantic")]`), `lexical` (2) | Keep the shared core (`Fields`, `Candidate`, `TwoTier`, `WindowSlice`, storage helpers) in the parent; moving it costs 4–25 bumps each. |
  | `src/mcp.rs` | `owners` (0), `transport` with its tests (0; not named `http`), `startup` (1), `semantic` (2–3) | Keep `Shared`, `RootMeta`, the slot and `run_op` in the parent. |
  | `src/syntax.rs` | `outliner` (0), `text` (0), `tiling`, `forest`, `markdown`, `imports`, `extract` (1 each), `parser` with `parse_hooks` (2), `calls` (5) | Its in-file test parses `include_str!("syntax.rs")`; every split shrinks that sample. Freeze the sample as a fixture first; that is a test change and an owner call. |
  | `src/learning.rs` | `state` with its tests (0), `run_policy` (0), `feedback` (0), `strict` and `io` (0), `prep` (2), `readback` (7 if merged with `prep`) | `serve`, `supervisor` and `worker` compile only on macOS, so Linux gates cannot check items they reach (`LearningPolicy`, `fail`, `hex64`); keep those in the root or re-export them, and check on a Mac. |
  | `src/neural/prepare.rs` | `order` (0), `cli` (0), `steps` with the tests (1) | Wait for F13: its fix changes `commit_partition`. |
  | `src/neural/query.rs` | `runtime` with its tests (0), `refinement` with its tests (0) | The test module must be split in two; add `use super::{area, cache};` in the parent for the moved relative paths. |
  | `src/graph.rs` | `references` (0), `edges` (0), `compiler` (0) | Re-export the moved `pub` items exactly; check with the rustdoc comparison below. |

- **Required resource:** none beyond a normal session, except a Mac for the
  `learning.rs` splits that touch items its macOS-only children use.
- **Resume, per split:**
  1. Move the planned line ranges verbatim into `src/<module>/<child>.rs`, which starts
     with a `//!` line and `use super::*;`; add `mod <child>;` after the parent's
     imports and `pub use` / `pub(crate) use` lines only for moved items used outside
     the child. Prove the move is verbatim by diffing the removed ranges against the
     new file.
  2. Run `cargo fmt --check` and `cargo clippy --locked --all-targets -- -D warnings`.
     If the compiler asks for a bump the plan did not list, revert the commit and update
     this entry instead of forcing it.
  3. Check the public API: `cargo doc --locked --no-deps --lib --features test-faults`
     before and after, compare every page with the source links stripped, and for any
     changed type page compare the set of documented members (an `impl` block moved to
     a child reorders sections but must keep every signature and doc). A new redirect
     page for the private child path is expected.
  4. Run `cargo check --locked --no-default-features --lib` (no warnings), then
     `scripts/gates.sh . OUTDIR` and the unprivileged rerun of the gateway target, and
     record the receipt in [validation](docs/validation.md#local-gates-receipts). Source
     `scripts/cloud/environment.env` and unset `RUSTUP_TOOLCHAIN` first: the learning
     tests need its tokenizer path, the gates choose their own toolchains, and its
     `CARGO_INCREMENTAL=0` and debuginfo-off profiles keep the gates target within the
     disk allowance (with debuginfo each test binary is about 600 MB).
