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

## F13. Continuous semantics stopped on a stale-source partition race (fixed 2026-10-13)

- **Status:** fixed in `1f29ec2` as designed below, after the owner asked for the fix
  (2026-10-13). On the owner's next request, `b32ec73` gave `stale_area` the same
  treatment (fix step 5). Nothing is blocked. This entry stays as the record of the race.
  The race was found while investigating
  `continuous_semantics_progresses_during_filesystem_traffic_and_converges`
  (`tests/neural_retrieval.rs`), which failed about 1 run in 6 under the full parallel
  suite on 4 vCPUs.
- **Diagnosis (before `1f29ec2`):** `Run::partition_wave` (`src/neural/driver.rs`) reads
  each source under the engine slot, releases the slot while the partition workers
  compute, then retakes it for each `commit_partition` (`src/neural/prepare.rs`), which
  calls `Engine::semantic_accept_partition` (`src/neural/cache.rs`). The owner lets pending
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
    preparation, also ended in `paused` (until `b32ec73`). It was not observed.
- **Fix:** treat a superseded source as a normal outcome. No new requeue is needed:
  `replace_source` and `delete_source` record the new version in the semantic
  obligation table in the same transaction (`work::changed`), and
  `derivation_ack` removes a path only when its stored version equals the batch's. A
  skipped path therefore stays pending, and the next `maintain` iteration, or the
  one-shot pass's revision check, partitions it again.
  1. `Engine::semantic_accept_partition` returns `PartitionCommit::Committed` or
     `PartitionCommit::Superseded` (hash differs, or source gone) instead of an error.
     `Superseded` keeps the old refusal (`partition_invalid` or `not_found`), and
     `PartitionCommit::committed` turns it back into that error for callers no
     concurrent writer can reach.
  2. `commit_partition` passes that through, and counts `partitioned_sources` and fires
     `PARTITION_AFTER_COMMIT` only on a commit. The CLI `run` keeps the old error,
     which it cannot reach: it opens the store exclusively.
  3. `Run::partition_wave` drops a superseded partition and its retained cards and
     continues; the pause and cancel checks before each commit stay as they are.
  4. The public `Engine::semantic_record_partition` still refuses a stale hash with
     `partition_invalid`, so its contract in `tests/neural.rs` holds. The structural
     `partition_invalid` checks (bounds, order, UTF-8, keys, rendering) are untouched.
  5. (`b32ec73`) The driver's `area_pass` skips an area that `area::bind` refuses
     instead of stopping with `stale_area`. Since the pass's snapshot, its brief
     changed or was forgotten, or one of its sources changed. A brief write marks
     overviews pending (`work::overview_changed`) and a source write records its path,
     both in the writing transaction, so the next `maintain` batch binds the current
     area or prunes it. A one-shot pass completes and leaves a changed brief pending as
     an overview obligation: brief writes do not move the source revision its loop
     checks. The CLI `run` keeps `stale_area`, which it cannot reach.

  Rejected: restarting the wave from a fresh batch (it discards the wave's other work
  and can starve unrelated paths under steady churn), and serializing partition and
  commit against the watcher (it reverses the 009 T003 slot decision and breaks the
  driver test that requires partition CPU work to release the serving slot). A path
  rewritten faster than its own partition compute is retried each pass; the watcher's
  debounce (75 ms quiet, 1 s maximum) bounds that.
- **Evidence (local only; GitHub Actions is unavailable):**
  - Four tests in `src/neural/driver.rs` drive the race deterministically through the
    fault points. Each runs a replace and a delete variant, except the mid-wave test,
    which replaces only:
    - `a_source_changed_during_partition_compute_is_superseded`: a fault at
      `PARTITION_BEFORE_COMPUTE` changes `note00.md` while the slot is free. The wave
      returns `Ok`, writes no partition for it, keeps no cards for it, and commits the
      other 7 sources.
    - `a_source_changed_between_partition_commits_is_superseded`: after the first
      commit, a foreground write between two store steps replaces `note01.md`; only
      that source is superseded and the wave goes on.
    - `maintenance_converges_after_a_source_changes_during_partition_compute`: `maintain`
      reaches `waiting` with no pending path and a partition for the current hash, or
      none once the source is gone.
    - `a_one_shot_pass_completes_after_a_source_changes_during_partition_compute`: a
      one-shot pass ends `Complete` with the current version partitioned.

    On the parent `6dff64a`, each fails with `partition_invalid` (replace) or
    `not_found` (delete).
  - Three tests in the same module cover step 5 with a provider that makes a foreground
    write during an area's model call, which holds no engine slot. Each runs three
    variants on the area the pass visits first: its brief revised, its brief forgotten,
    and one of its sources replaced.
    - `an_area_changed_during_its_model_call_is_superseded`: the pass skips that area
      and binds the next.
    - `maintenance_converges_after_an_area_changes_during_its_model_call`: `maintain`
      reaches `waiting` with nothing pending; the revised brief's area is bound, and
      the other two variants' areas are gone.
    - `a_one_shot_pass_completes_after_an_area_changes_during_its_model_call`: a
      one-shot pass ends `Complete`, with overviews still pending after a brief write.

    On the parent `a126eac`, every variant stops with `stale_area`.
  - `tests/neural.rs` passes unchanged, including the stale-hash refusal of
    `semantic_record_partition` and the structural `partition_invalid` checks.
  - `cargo test --locked --test neural_retrieval`, deadlines unchanged: 12 of 12
    consecutive runs passed (93 passed, 1 ignored each), on the fix and again on
    `b32ec73`. The race showed in about 1 run in 6 of the full parallel suite, and this
    soak ran the target alone, so it shows the target still passes; the deterministic
    tests above are the proof of the fix.
  - The full local gates on `1f29ec2` and `b32ec73`: see
    [validation](docs/validation.md#f13-superseded-partitions--2026-10-13).
- **Remaining:** nothing.

## F14. God-module splits (audit 2026-10-13)

- **Stopped (owner decision, 2026-10-13):** the refactor stops after PR #21 (merge
  `2bab612`). PRs #17–#21 landed 16 verbatim splits with no visibility change and an
  unchanged public API. Every split still in the plan below needed crate-internal
  visibility changes, a Mac, or F13. F13 is fixed since `1f29ec2`, so the
  `src/neural/prepare.rs` splits `order` and `cli` no longer have a blocker. Do not start
  one without a new owner decision; the plan and resume steps stay as the record.
- **Why deferred:** each split is a separate, gated, revertable commit. On 2026-10-13
  only the splits that were purely mechanical ran: verbatim moves with no visibility
  change and an unchanged public API, 16 in all (see
  [validation](docs/validation.md#god-module-splits--2026-10-13)). The rest change
  crate-internal visibility, touch macOS-only code that Linux gates do not compile, or
  waited on F13 (fixed since `1f29ec2`).
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
- **Done (2026-10-13):** each split is a verbatim move with `use super::*;` in the
  child and no visibility change; see
  [validation](docs/validation.md#god-module-splits--2026-10-13) for the evidence.
  - PR #17 (merge `1593c2c`): `14729de` moved the tool layer of `src/mcp.rs` to
    `src/mcp/tools.rs`, and `9a86665` moved context assembly from `src/store.rs` to
    `src/store/context.rs`.
  - Second batch:
    - `src/mcp.rs`: `94b9219` `owners`; `7353251` `transport`, with its three tests.
    - `src/store.rs`: `e3c7b08` `retrieve`; `7dff09b` `index_hooks`, an inline module
      turned into a file, so one indentation level is removed; `9d65e9d` `sources`;
      `17697da` `indexing`, declared after the `index_event!` macros; `e814da2`
      `lifecycle`, with repair.
    - `src/neural/query.rs`: `57ae9de` `refinement`, with its seven tests; `1cb3173`
      `runtime`, with the rest of the test module.
    - `src/graph.rs`: `7c25def` `references`, with exact doors; `385a593` `edges`.

    The parent files are now `src/mcp.rs` 3,313 lines (6,506 at `a088969`),
    `src/store.rs` 4,967 (7,654), `src/neural/query.rs` 449 (2,358) and `src/graph.rs`
    777 (2,954).
  - Follow-up: `3beac37` moved the `src/graph.rs` compiler facts to
    `src/graph/compiler.rs`, leaving `src/graph.rs` at 247 lines. The mapped boundary,
    lines 619–939 and 1094–1342 at `a088969`, moved `impl OccurrenceKind`, whose private
    `tag` and `from_tag` the parent's storage key codec also calls (E0624 three times).
    So `OccurrenceKind` and its `impl` stay in the parent next to the codec, and the
    other three ranges moved verbatim.
  - After F15 froze the test sample: `973548d` moved outlines to
    `src/syntax/outliner.rs`. `COMMENT_LINES` stays in the parent because unit
    extraction uses it. `f072a11` moved the tokenizers and path segments to
    `src/syntax/tokens.rs`; the plan called it `text`, but `src/syntax.rs` already has
    a private `fn text`. `src/syntax.rs` is now 4,584 lines (5,075 before).
- **Remaining plan**, each step one commit, in this order within a file, gated as
  below. "Bumps" are crate-internal visibility changes (`pub(super)`); none changes
  the public API.

  | File | Next splits (bumps) | Precondition or blocker |
  | --- | --- | --- |
  | `src/store.rs` | `doors` (1), `semantic_search` (1, `#[cfg(feature = "semantic")]`), `lexical` (2) | Keep the shared core (`Fields`, `Candidate`, `TwoTier`, `WindowSlice`, storage helpers) in the parent; moving it costs 4–25 bumps each. |
  | `src/mcp.rs` | `startup` (1), `semantic` (2–3) | Keep `Shared`, `RootMeta`, the slot and `run_op` in the parent. |
  | `src/syntax.rs` | `tiling`, `forest`, `markdown`, `imports`, `extract` (1 each), `parser` with `parse_hooks` (2), `calls` (5) | None since `1cced0f` (F15): the in-file test now reads a frozen copy of the file, so a split no longer changes its input. |
  | `src/learning.rs` | `state` with its tests (0), `run_policy` (0), `feedback` (0), `strict` and `io` (0), `prep` (2), `readback` (7 if merged with `prep`) | Needs a Mac. `serve`, `supervisor` and `worker` are `#[cfg(target_os = "macos")]`, so Linux gates do not compile the items they reach (`LearningPolicy`, `fail`, `hex64`); keep those in the root or re-export them. |
  | `src/neural/prepare.rs` | `order` (0), `cli` (0), `steps` with the tests (1) | None since F13 (`1f29ec2`) gave `commit_partition` its new shape. |

  `src/neural/query.rs` keeps the dense index, the fallback and query tokenization, and
  `src/graph.rs` keeps `invalid`, `OccurrenceKind`, the storage key codec and the test
  seam; neither has a further split planned.

- **Required resource:** none beyond a normal session, except a Mac for the
  `src/learning.rs` row.
- **Resume, per split:**
  1. Move the planned line ranges verbatim into `src/<module>/<child>.rs`, which starts
     with a `//!` line and `use super::*;`; add `mod <child>;` after the parent's
     imports and `pub use` / `pub(crate) use` lines only for moved items used outside
     the child (every `pub` item, and the `pub(crate)` items named elsewhere). Insert
     those lines where `rustfmt` sorts them (by path, whatever the visibility), so
     later splits never edit an earlier split's lines. A child module that uses a
     `macro_rules!` macro is declared after the macro. Prove the move is verbatim by
     diffing the removed ranges against the new file.
  2. Run `cargo fmt --check` and `cargo clippy --locked --all-targets -- -D warnings`.
     If the compiler asks for a bump the plan did not list, revert the commit and update
     this entry instead of forcing it.
  3. Check the public API: `cargo doc --locked --no-deps --lib --features test-faults`
     before and after, compare every page with the source links stripped, and for any
     changed type page compare the set of documented members (an `impl` block moved to
     a child reorders sections but must keep every signature and doc). Build each side
     into an empty `doc/` directory: rustdoc never deletes stale pages, so a reused
     directory can hide a removed page. Two differences are expected: a redirect page
     for each re-exported item's private child path, and, on a type-alias page, the
     internal `data-self-path` naming where the alias is defined.
  4. Run `cargo check --locked --no-default-features --lib` (no warnings), then
     `scripts/gates.sh . OUTDIR` and the unprivileged rerun of the gateway target, and
     record the receipt in [validation](docs/validation.md#local-gates-receipts). In a
     cloud session run the gates as `bash scripts/cloud/gates.sh OUTDIR`, which applies
     `scripts/cloud/environment.env` and unsets `RUSTUP_TOOLCHAIN`: the learning tests
     need its tokenizer path, the gates choose their own toolchains, and its
     `CARGO_INCREMENTAL=0` and debuginfo-off profiles keep the gates target within the
     disk allowance (with debuginfo each test binary is about 600 MB); see F16.
  5. For `src/learning.rs`, also run `scripts/gates.sh . OUTDIR` on macOS, where
     `serve`, `supervisor` and `worker` compile, and record that receipt next to the
     Linux one.

## F15. `syntax.rs` self-sample (decided 2026-10-13: frozen as a fixture)

- **Status:** done in `1cced0f`; the owner chose option 1.
  `tests/fixtures/syntax/syntax-rs-sample.txt` is `src/syntax.rs` at `5d9596d`, byte
  for byte (202,038 bytes, SHA-256 `6d909735…7b33035`), and the test includes it under
  the label `"syntax.rs"`. The rest of this entry is the proposal as the owner saw it.
- **The test:** `per_character_reads_give_the_whole_buffer_tree` (`src/syntax.rs`,
  `mod tests`) parses one source per grammar (25) in four forms: as written, with CRLF
  line ends, behind a byte-order mark, and empty. Each form is parsed twice: by `parse`,
  which feeds tree-sitter one UTF-8 character per read under the work budget, and by
  tree-sitter on the whole buffer. The test then asserts that the two trees match node for
  node: kind, flags, byte and point ranges, field and child count. The Rust sample is
  `include_str!("syntax.rs")`, the file itself.
- **Why it depends on the file's own size:** that sample is the test's only large input.
  At `1593c2c` it is 202,038 bytes and 5,075 lines, with 72 non-ASCII characters (52 in
  production code, 20 in the test module) and 108 raw-string openers. The other 24
  samples are at most 1,126 bytes. Size does not matter for the budget:
  `PARSE_WORK_BUDGET` is 2^25 units, and the reads alone, one unit per character, are
  about 0.6% of it. Progress callbacks spend more units; that share is not measured, and
  the test passes. What size buys is variety: many lines, multi-byte characters at many
  offsets, and real Rust constructs in one continuous parse. Each split removes the
  moved code from the sample. The F14 plan moves nine of the file's concerns out, so the
  sample shrinks at every step. How far it shrinks is not measured, because the line
  boundaries of the `syntax.rs` splits are not mapped yet.
- **Options:**
  1. **Freeze the input as a fixture.** Copy today's bytes to
     `tests/fixtures/syntax/syntax-rs-sample.txt` and change the one `include_str!` line;
     the label stays `"syntax.rs"`, which is what selects the Rust grammar. The `.txt`
     extension keeps Rust tooling and code search from reading a second copy of the
     source as code. Cost: 202 KB in the repository.
  2. **Point the test at paths it controls.** Sample every file of the split module,
     `src/syntax.rs` and `src/syntax/*.rs`, each as its own Rust input. Done at compile
     time, the source list grows with each split; done at run time, the test reads the
     directory. This keeps tracking live code and today's total bytes, but no single
     input stays long, and the input still changes with every edit to those files.
  3. **Leave `src/syntax.rs` unsplit.** No test change; the file stays a 4,856-line
     production module, second in the F14 ranking.
- **Recommendation:** option 1. The property under test belongs to the character reader
  and the grammar, not to whatever `src/syntax.rs` contains on a given day; the
  22,492-file calibration, not this test, is what covers real-world variety. A frozen
  input keeps today's exact catching power, and a failure then points at the reader or a
  grammar bump instead of an unrelated edit.
- **Does freezing change what the test can catch?** Not at the moment of freezing: the
  same bytes go through the same four forms and the same grammar. Afterwards there are two
  differences. The test no longer samples Rust syntax that is added to `src/syntax.rs`
  later. It also no longer fails because an unrelated edit to that file happens to
  contain a construct where the two parses disagree; today such a failure would be a true
  finding about the reader, found by accident. If the owner wants to keep that live
  tracking too, add the frozen fixture as a second Rust sample and keep the live file as
  the first. That costs one more entry; the grammar count stays 25.
- **Resume:** nothing remains here. The `syntax.rs` splits in the F14 table may run,
  gated as F14 describes; they no longer change this test's input.

## F16. Cloud sessions without the environment variables (found 2026-10-13)

- **Why open:** the 2026-10-13 cloud session (`CLAUDE_CODE_REMOTE=true`) started with
  none of the variables in [`scripts/cloud/environment.env`](scripts/cloud/environment.env)
  set. `env` showed no `RUSTUP_TOOLCHAIN`, `CARGO_INCREMENTAL`,
  `CARGO_PROFILE_TEST_DEBUG`, `CONTEXT_FOUNDRY_013_TOKENIZER`, `LLAMA_CPP_DIR` or
  `CF_TOOLS`. Those values come from the environment's settings: the cloud environment
  menu in the session's title bar, then **Edit**, under environment variables. A session
  reads them only when it starts, so a change there reaches only new sessions.
- **What breaks without them:**
  - **The doctor fails.** `scripts/cloud/doctor.sh` sees the default `rustc` 1.97.0
    instead of 1.90.0, since it reads `RUSTUP_TOOLCHAIN`. Every path check also reports
    a missing file: the host `rustc`, the GGUF and tokenizer paths, `LLAMA_CPP_DIR` and
    the bindgen include.
  - **Test builds fill the disk.** Without `CARGO_INCREMENTAL=0` and the debuginfo-off
    profiles, each integration-test binary is about 600 MB with debuginfo. `target/gates`
    reached 19 GB, and one gates run ran out of the disk allowance while linking the tests.
  - **The `learning_data` tests fail.** They read the tokenizer from
    `CONTEXT_FOUNDRY_013_TOKENIZER` and otherwise fall back to
    `$HOME/VSC_DEV/models/…`, which does not exist in a cloud session, so 52 of them fail
    with `NotFound`.
- **Workaround used on 2026-10-13:** every counted gates run first ran
  `set -a; . scripts/cloud/environment.env; set +a; unset RUSTUP_TOOLCHAIN`. The unset is
  for the gates only: `scripts/gates.sh` runs 1.97 and then 1.90 by itself, while other
  cargo commands in a cloud session should keep the 1.90 pin. Each receipt's `env.txt`
  lists the variables that run saw.
- **Verify that it took effect:**
  - `env | grep -E '^(CARGO_INCREMENTAL|CARGO_PROFILE_TEST_DEBUG)=0$'` prints both lines.
  - `echo "$CONTEXT_FOUNDRY_013_TOKENIZER"` names an existing directory.
  - `rustc --version` reports 1.90.0.
  - `bash scripts/cloud/doctor.sh` prints `PASS environment` and exits 0.
  - After a gates run, `du -sh target/gates` stays near 10 GB, not 19 GB.
- **Durable fix (proposed; none of it is implemented):**
  1. **Owner action, no repository change:** paste the file into the environment's
     variables (title bar → environment menu → Edit) and start a new session. The file's
     first line already asks for this.
  2. **Doctor check: done in `b565784`.** `scripts/cloud/doctor.sh` now prints
     `FAIL environment` first, naming the file, the missing variables and the fix, when
     any variable the file sets to a value is unset. A variable set to a different value
     gets an INFO line only, since an override can be deliberate. Before this change, a
     session without the variables stopped at `BINDGEN_EXTRA_CLANG_ARGS: unbound
     variable` before naming any cause; the doctor now runs to the end.
  3. **Wrapper: done in `13e051c`.** `bash scripts/cloud/gates.sh OUTDIR` applies the
     file, unsets `RUSTUP_TOOLCHAIN`, writes the file's variables as the gates saw them
     to `OUTDIR/env.txt` and runs `scripts/gates.sh` on the checkout. Cloud receipts are
     then reproducible whatever the session holds.
  4. **CLAUDE.md: done in `13e051c`.** "Cloud sessions" says what `FAIL environment`
     means, who fixes it and how to work until then, and the local-gates rule names the
     wrapper for cloud sessions.

  Item 1 is the owner's remaining action; items 2–4 only detect the problem and work
  around it.
- **Required resource:** owner access to the environment settings for item 1.
