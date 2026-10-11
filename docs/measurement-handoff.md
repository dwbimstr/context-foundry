# Measurement handoff

The experimental branch now includes an explicit
[native macOS workflow](../.github/workflows/native-macos.yml) and
[reproducible runner](../scripts/native-macos-acceptance.py). It runs the native CPU
checks and synthetic worker lifecycle checks, then requires a successful Metal
compute dispatch before downloading pinned public embedding artifacts. A signed
development worker must produce the full-window receipt and be observed and reaped.
The runner preserves failures and blocked hardware as such; adding the job does not
establish acceptance. Its selected checks exclude generation, real-model owner death
under pressure, global GPU headroom and signed installed-package acceptance.

**2026-10-11 resource-code handoff.** The macOS CPU sampler now exists in
[`model_resources.rs`](../src/model_resources.rs), using the existing optional
background CPU policy. Linux verifies tick accounting/reset and policy hysteresis;
native compilation and actual CPU-pressure behavior have not run here. The current
Linux environment has no configured native Mac runner or GPU access. On an idle Mac:

```sh
cargo test --locked --lib model_resources::tests::native_macos_cpu_observation \
  -- --ignored --exact --nocapture --test-threads=1
cargo test --locked --lib model_resources::tests::native_macos_cpu_pressure_pauses_and_resumes_both_lanes \
  -- --ignored --exact --nocapture --test-threads=1
```

The first check samples real CPU counters twice. The second uses bounded CPU load and
the actual CPU/memory sampler to check unknown-sample refusal, background pause/resume
for semantic and generation reservations, foreground admission, reservation retention
and release. It requires 1–128 logical CPUs and enough free memory for its small
reservations; it uses no model weights. This is an admission-API check. It does not
launch model workers, prove worker lifetime or measure GPU allocations, throughput,
hard CPU quotas or thermal/battery policy. Actual model-worker pressure/lifecycle,
Metal parity and signed installed-artifact checks below remain separate acceptance.
The current default macOS CI job does not run these ignored checks or enable native
model-worker features. `overview-v1` and hierarchy's `+overview3` fitting recipe also need
representation-quality and device checks; complete file-vector coverage is not full
body evidence or a task-quality result. The [latest Linux record](validation.md#named-re-exports-and-current-fitting--2026-10-11)
preserves the precise corpus/input counts and bounded real-model progress.

**2026-10-11 portable numerical handoff.** Linux has verified real CPU embedding
artifacts; macOS/Metal hardware remains unavailable in this sandbox. The new
[`embedding_parity`](../tests/embedding_parity.rs) test writes a versioned receipt
containing the descriptor, worker executable hash, exact token IDs, vectors,
platform and elapsed time. It requires the production 8,192-token geometry, checks
four heterogeneous inputs singly and together, and includes a full-window input.
It refuses an existing output path. Run the same test separately on Linux CPU
(adapter revision 3) and Apple silicon Metal (revision 4), using the same pinned
BF16 GGUF, tokenizer, pooling, dimensions, templates and geometry:

```sh
FOUNDRY_EMBED_PROFILE=/absolute/native-profile.json \
FOUNDRY_EMBED_RECEIPT=/absolute/new-native-receipt.json \
cargo test --locked --test embedding_parity \
  -- --ignored --exact real_native_embedding_receipt --nocapture

FOUNDRY_EMBED_CPU_RECEIPT=/absolute/cpu-receipt.json \
FOUNDRY_EMBED_METAL_RECEIPT=/absolute/metal-receipt.json \
cargo test --locked --test embedding_parity \
  -- --ignored --exact compare_cpu_and_metal_receipts --nocapture
```

Use a non-root Apple-silicon Mac with Rust 1.90 or newer, CMake, Xcode/macOS SDK
and Metal compiler tools, libclang and signing tools. `LLAMA_CPP_DIR` must name a
llama.cpp repository containing pinned commit
`b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea`. With the explicit native model/tokenizer
profile, existing physical model/scratch directories and a fresh bundle destination:

```sh
LLAMA_CPP_DIR=/absolute/llama.cpp cargo build --locked --features embed-worker --bin foundry-embed
scripts/embed-worker-bundle.sh --binary target/debug/foundry-embed \
  --out /absolute/new/FoundryEmbed.app --profile /absolute/native-profile.json
```

Update the profile's bundle path and executable hash **after signing**. The optional
Linux AVX-512 BF16 worker uses adapter revision 5: it can produce its own receipt,
but the CPU/Metal comparator requires baseline revision 3 and Metal revision 4.
A revision-5 receipt does not satisfy that comparison. Use the existing bundle
instructions for signing and installed-artifact acceptance. The comparator requires matching descriptors
except for the backend revision and identical token inputs. Both vectors must be
finite and normalized within 0.0001, with cosine at least 0.9999. It explicitly
requires Linux and macOS arm64 receipts; a Linux-only or zero-test run cannot pass
as Metal validation. Passing does not make the two function/cache identities
interchangeable, select a model, establish retrieval quality, or certify GPU pressure,
isolation or a distributed package. Keep the existing native lifecycle and signed
installed-artifact checks below.

**Representative workload continuation.** The Linux corpus, exact manifest, profile
and raw reports are recorded in [current validation](validation.md#broader-evidence-and-representative-workload--2026-10-11).
To inventory another restored copy, provide the 60–80k regular-file manifest as JSON
`{"files":[["relative/path", byte_length, "sha256"], ...]}` and the matching body-v1
profile. Use a new absolute output directory outside Git and outside the corpus:

```sh
FOUNDRY_SEMANTIC_SCALE_ROOT=/absolute/corpus \
FOUNDRY_SEMANTIC_SCALE_MANIFEST=/absolute/manifest.json \
FOUNDRY_SEMANTIC_SCALE_PROFILE=/absolute/body-profile.json \
FOUNDRY_SEMANTIC_SCALE_DIR=/absolute/new-inventory \
cargo test --locked --test semantic_scale \
  -- --ignored --exact representative_corpus_workload_inventory --nocapture

foundry --store /absolute/new-inventory/store semantic prepare \
  --profile /absolute/body-profile.json --budget-seconds 1800 --development-isolation
```

The inventory has a 1,800-second deadline including indexing; failure preserves a
partial workload report. To continue only its unfinished suffix, set
`FOUNDRY_SEMANTIC_SCALE_PREVIOUS=/absolute/prior-inventory/workload.json` and select
a new output directory with the same manifest/root/profile. The prior store must
have no source changes; the continuation rechecks the fitted prefix's hashes. The
report must also carry the matching versioned `recipe_id`; missing/older fitter
identities are refused before indexing, even if profile and source hashes match.
Use the `store_directory` from the continuation report for subsequent preparation;
the new report directory does not duplicate the store. Per-file counts continue exactly. Because the prior key set is not persisted,
combined distinct-input/token counts become explicit bounds rather than an exact
union; raw vector-payload bounds follow those counts. Preserve the failed phase.
The inventory uses the real tokenizer but performs no model inference
and does not persist semantic partitions. Preparation must still fit/cache/publish
its own current sources. Resume the same store/profile, retaining every phase's
output and the current coverage counts. Freeze hardware, budget, artifact hashes,
cache capacity and corpus identity before timing; run one model/build job at a time.
The measured Rust/Go **body recipe** needs at least 2.24 GiB of raw vector payload; the
default 2 GiB cache cap cannot cover it. Select a larger explicit `--cache-cap` and
check disk for cache, partitions and ANN storage before attempting a complete run.
The current overview recipe has completed fitting (177.93–193.24 MiB raw vectors),
with representation quality and full inference still unaccepted. Full completion
requires no unpartitioned sources, missing inputs or unpublished
current locations, followed by real serving and changed-input checks. A completed
file scan or fitting inventory alone cannot close semantic scale acceptance.

**Opt-in native scale calibration.** After the ordinary embedding receipt and
cleanup (and generation cleanup when `--generation` is selected),
`scripts/native-macos-acceptance.py --out /absolute/new-results --work
/absolute/new-work --scale-calibration` can measure 128 public overview inputs.
The flag is off by default. Push-triggered jobs leave calibration disabled; a
manual workflow dispatch can select the boolean `scale_calibration` input.
Neither setting requests a full-corpus run. The script downloads the same checksum-pinned Rust 1.90.0 and
Go 1.25.0 archives below and records each selected path, source SHA-256 and byte
size. Selection fixes 16 files in each source-size quartile of each project by
SHA-256 rank of `native-scale-v1` + NUL + relative path. It includes only nonblank
UTF-8 `.rs`/`.go` files from 64 bytes through 2 MiB, excluding hidden paths,
`target`, `node_modules` and normal private-key-marker exclusions. Its fixed
128-file manifest is 670,355 source bytes. This deliberately spans project and
source-size groups. This is a `.rs`/`.go` **archive sample**, distinct from the
full admitted-source population: upstream `.gitignore` and other ignore controls
are not extracted with the selected files, and membership of these 128 paths in
the full 67,663-source admitted set is unverified. Ordinary indexing checks only
the isolated sample. It is neither population-weighted nor representative of all
admitted file kinds. No expected labels guide selection.

Calibration uses `overview-v1`, `card_tokens:512`, `batch:1`, the same verified
Metal revision 4 descriptor, 8,192-token geometry and signed development bundle,
and a fresh source tree/store/cache with a separate private scratch subdirectory.
The cache cap is **512 MiB (536,870,912 bytes)** for preparation and replay, matching
the full-scale configuration; this is a storage limit, not a memory allocation.
The canonical function digest stays unchanged when only the recipe/card limit or
batch changes; it must remain distinct from CPU revisions 3 and 5. No vectors are
imported. The ignored `native_scale` integration test uses ordinary production
indexing, fitting and preparation with a recording wrapper around the actual
supervised worker. It requires exactly 128 distinct fitted inputs, 128 successful
calls, complete publication, and zero model calls on unchanged replay. It retains
source-to-input keys, complete fitted token IDs and SHA-256 of their concatenated
little-endian u32 bytes; the wrapper verifies those hashes against actual calls.
Per-call JSONL is flushed as work completes, including a failed call when one
returns an error. Preparation has a 1,200-second budget with the normal publication
reserve; prior partial records are never promoted to completion.

The receipt separates provider-call nanoseconds, acquisition time, preparation
wall time and total harness time; script stages include download/build/test wall
time separately. The harness independently fits inputs before preparation for its
identity audit; its concrete-worker test seam repeats artifact verification to
expose the owned PID, so startup and total harness cost are not an unmodified CLI
benchmark. It measures native `ri_phys_footprint` immediately before and
after every successful call. The maximum of those 256 observations is **not** a
continuous or loading peak, GPU allocation, or global GPU headroom. The production
4 GiB footprint guard remains active. Model teardown and an independent observed
process-identity cleanup check precede success. Review these actual measurements,
input lengths, publication/replay results and disk availability before selecting
any separate full-corpus wall budget. This calibration does not establish complete
67,663-source preparation, its end-to-end throughput, simultaneous model residency,
or retrieval/task quality; any scale projection remains an explicitly labeled
estimate until the full receipts exist.

**Portable complete-overview reproduction (Linux).** The following uses ordinary
CLI operations and operator-controlled phases; it adds no second preparation owner
or tracked orchestration framework. Keep binaries, profiles, archives, corpus,
manifest and reports outside Git. A fresh source tree/store avoids importing cache
entries from another adapter. This reproduces file-overview inference and unchanged
replay, not exhaustive body coverage, retrieval quality or changed-source acceptance.

Restore the exact public inputs below. Rust is commit
`1159e78c4747b02ef996e55082b704c09b970588` (1.90.0); Go is commit
`6e676ab2b809d46623acb5988248d95d1eb7939c` (go1.25.0). The Go tag archive is pinned
by its full checksum; a moved tag or changed archive must fail verification.
These commands require Python 3, curl and Linux `sha256sum`/`timeout`:

```sh
set -eu
export CF_SCALE_WORK=/absolute/new-scale-work
mkdir "$CF_SCALE_WORK"
curl --fail --location --proto '=https' --proto-redir '=https' \
  --output "$CF_SCALE_WORK/rust.tar.gz" \
  https://codeload.github.com/rust-lang/rust/tar.gz/1159e78c4747b02ef996e55082b704c09b970588
curl --fail --location --proto '=https' --proto-redir '=https' \
  --output "$CF_SCALE_WORK/go.tar.gz" \
  https://codeload.github.com/golang/go/tar.gz/refs/tags/go1.25.0
python3 - "$CF_SCALE_WORK" <<'PY'
import hashlib, json, pathlib, tarfile, sys
work = pathlib.Path(sys.argv[1]).resolve()
root = work / "corpus"
root.mkdir()
rows = []
for project, expected in [
    ("rust", "8dc476ed3ccd405304236e3ee737b365b49168f1cfebd4da537ee0b4380e96bf"),
    ("go", "7e5813e23f221dc193dec557379fe22607baa6fc2e321a1b8e39b658848a5f5f"),
]:
    archive = work / (project + ".tar.gz")
    assert hashlib.sha256(archive.read_bytes()).hexdigest() == expected
    with tarfile.open(archive, "r:gz") as source:
        for member in source:
            if not member.isfile():
                continue
            parts = pathlib.PurePosixPath(member.name).parts
            assert len(parts) > 1 and not member.name.startswith("/") and ".." not in parts
            relative = pathlib.PurePosixPath(project, *parts[1:])
            data = source.extractfile(member).read()
            destination = root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            with destination.open("xb") as output:
                output.write(data)
            rows.append([relative.as_posix(), len(data), hashlib.sha256(data).hexdigest()])
rows.sort()
assert len(rows) == 69565 and sum(row[1] for row in rows) == 324780308
assert len({row[2] for row in rows}) == 68056
encoded = (json.dumps({"files": rows}, separators=(",", ":"), ensure_ascii=True) + "\n").encode()
assert hashlib.sha256(encoded).hexdigest() == "f3748bda2b6ea83ba249e43525d7631262cc5e8eddc0526e6e75f7726b48df63"
with (work / "manifest.json").open("xb") as output:
    output.write(encoded)
PY
```

This exports every regular archive member, preserving upstream ignore files, without
`.git`, fetched submodules or Rust's four license symlinks. The portable manifest
contains globally sorted `[relative_path, bytes, sha256]` rows and has no absolute
root. Its hash differs from the historical `b4c3e6a6…` manifest, which includes
machine-specific root/provenance fields and a different row order; its per-file
obligation is identical. The ordinary admission policy yields **67,663 sources**
from 69,565 regular files. The 1,902 unadmitted files include ignored paths and are
not the same count as the historical 816 explicit exclusion events. Do not delete
files or prefilter content to force either count.

Supply a pinned CLI with `semantic` support and an explicitly selected local
profile. For the measured CPU variant, build the worker with the existing
`embed-worker-avx512-bf16` feature on supported Linux x86_64 hardware and use
**adapter revision 5**; see [worker setup](experimental-context.md#independent-representations-and-the-linux-embedding-worker).
Keep its BF16 GGUF/tokenizer, descriptor, pooling, dimensions, templates and geometry
as pinned by the existing [native profile recipe](../scripts/native-macos-acceptance.py). Set `representation: {"kind":"overview-v1"}`,
`card_tokens:512`, `batch:1`, `dimensions:768`, `cpu:{"threads":4,"seconds":86400}`,
`memory_ceiling_bytes:8589934592` and `load_timeout_seconds:120`. CPU seconds cover
all worker threads for its lifetime. Revision 3 is a separate portable CPU function;
its vectors must not be relabeled/reused as revision 5. Local paths and worker hashes
must identify the actual verified bundle; keep the profile frozen for every phase.

```sh
export CF_SCALE_CLI=/absolute/pinned-foundry
export CF_SCALE_PROFILE=/absolute/verified-overview-profile.json
export CF_SCALE_OUT="$CF_SCALE_WORK/results"
mkdir "$CF_SCALE_OUT"
cp "$CF_SCALE_CLI" "$CF_SCALE_OUT/foundry"
cp "$CF_SCALE_PROFILE" "$CF_SCALE_OUT/profile.json"
python3 - "$CF_SCALE_OUT/profile.json" <<'PY'
import json, pathlib, sys
profile = json.loads(pathlib.Path(sys.argv[1]).read_text())
descriptor = profile["descriptor"]
assert descriptor["adapter_revision"] == 5 and descriptor["dimensions"] == 768
assert profile["representation"] == {"kind": "overview-v1"}
assert profile["card_tokens"] == 512 and profile["batch"] == 1
assert profile["cpu"] == {"threads": 4, "seconds": 86400}
assert profile["memory_ceiling_bytes"] == 8589934592
artifacts = {row["name"]: row["sha256"] for row in descriptor["artifact_files"]}
assert artifacts["embeddinggemma-2-BF16.gguf"] == "68bae29d62fb8c7d23e98d21fd4662753ddd636e6b62b8a70dfe059a9844f216"
assert artifacts["tokenizer.json"] == "4d777ef5bdc1aa36227abdfb77c3e49e7b9c892d16e1b6bda41c393504828be4"
PY
sha256sum "$CF_SCALE_OUT/foundry" "$CF_SCALE_OUT/profile.json" \
  "$CF_SCALE_WORK/manifest.json" > "$CF_SCALE_OUT/pins.sha256"
"$CF_SCALE_OUT/foundry" --store "$CF_SCALE_OUT/store" \
  index "$CF_SCALE_WORK/corpus" > "$CF_SCALE_OUT/index.json" \
  2> "$CF_SCALE_OUT/index.stderr"
```

Require `index.json` to report `scan_complete:true`, `partial:false`, zero failures,
zero pending sources/deletions, and `changed + unchanged = 67663`. Declare the total
wall budget before the first command; the measured attempt uses 12,600 seconds,
including indexing and 600 seconds reserved for final audits/replay. Record actual
command start/end times, not just provider time. Use one model/build/indexing job
at a time. The 128-input SIMD observation projects 2.65–2.87 provider hours; total
completion within 3.5 hours remains a hypothesis until the full receipts exist.
Allow disk for the fresh store, cache, ANN and atomic publication scratch; the
estimate is 1.5–2.5 GiB additional with a separate 512 MiB stop reserve, not a measured
peak. Never delete earlier evidence to make a phase fit.

For each phase, choose a new ordinal, verify the pins, and choose a CLI budget of
at most 3,600 seconds that fits the remaining declared wall budget/reserve. Record
the exit code as well as JSON/stderr. `timeout` is only a final wall-time guard:

```sh
CF_SCALE_PHASE=000
CF_SCALE_SECONDS=3600
sha256sum --check "$CF_SCALE_OUT/pins.sha256"
test ! -e "$CF_SCALE_OUT/prepare-$CF_SCALE_PHASE.json"
CF_SCALE_EXIT=0
timeout --signal=INT --kill-after=60s "$((CF_SCALE_SECONDS + 60))s" \
  "$CF_SCALE_OUT/foundry" --store "$CF_SCALE_OUT/store" semantic prepare \
  --profile "$CF_SCALE_OUT/profile.json" --budget-seconds "$CF_SCALE_SECONDS" \
  --development-isolation --cache-cap 536870912 \
  > "$CF_SCALE_OUT/prepare-$CF_SCALE_PHASE.json" \
  2> "$CF_SCALE_OUT/prepare-$CF_SCALE_PHASE.stderr" || CF_SCALE_EXIT=$?
printf '%s\n' "$CF_SCALE_EXIT" > "$CF_SCALE_OUT/prepare-$CF_SCALE_PHASE.exit"
```

Advance the ordinal and repeat against the **same store and profile** until
`partial:false`. Accepted batches are durable across interruption/restart. Retain
partial/failed reports; a missing report makes exact aggregate call/token accounting
unknown. Only an explicit budget/cancellation stop is resumable without diagnosing
another failure. Check `index_reason` as well as `reason_code`: a previous budget
stop can coexist with a later publication failure. The exact deadline reason says
publication was cut off by that phase's budget; cached vectors can be published in
a later phase. Never reinterpret a provider/cache/publication failure as completion.
Stop on the declared aggregate deadline or disk reserve, retaining partial evidence.

After complete preparation, choose a fresh verification directory for these CLI
processes. A failed verification retry gets a new directory; retain the prior one.
The CLI verifies the worker and model bytes against the frozen profile before use:

```sh
sha256sum --check "$CF_SCALE_OUT/pins.sha256"
export CF_SCALE_CHECK="$CF_SCALE_OUT/verify-000"
mkdir "$CF_SCALE_CHECK"
"$CF_SCALE_OUT/foundry" --store "$CF_SCALE_OUT/store" semantic status --audit \
  > "$CF_SCALE_CHECK/audit.json" 2> "$CF_SCALE_CHECK/audit.stderr"
"$CF_SCALE_OUT/foundry" --store "$CF_SCALE_OUT/store" semantic prepare \
  --profile "$CF_SCALE_OUT/profile.json" --budget-seconds 180 \
  --development-isolation --cache-cap 536870912 \
  > "$CF_SCALE_CHECK/unchanged.json" 2> "$CF_SCALE_CHECK/unchanged.stderr"
"$CF_SCALE_OUT/foundry" --store "$CF_SCALE_OUT/store" semantic status --audit \
  > "$CF_SCALE_CHECK/final-audit.json" 2> "$CF_SCALE_CHECK/final-audit.stderr"
python3 - "$CF_SCALE_WORK" "$CF_SCALE_CHECK" <<'PY'
import hashlib, json, pathlib, sys
work = pathlib.Path(sys.argv[1]).resolve()
out = work / "results"
checks = pathlib.Path(sys.argv[2])
read = lambda path: json.loads(path.read_text())
recipe = "overview-v1+overview3+kinds1+units5:512+tokenizers 0.23.2 onig"
for name in ["audit.json", "final-audit.json"]:
    status = read(checks / name)
    assert status["sources"] == 67663 and status["partition_coverage"] == "complete"
    assert status["unpartitioned_sources"] == status["missing_units"] == 0
    assert status["eligible_units"] == status["cached_current_units"] == status["searchable_current_units"] == 67585
    assert status["index"]["available"] and status["index"]["reason"] is None
    assert all(status["cache"][key] == 0 for key in ["corrupt_entries", "orphan_entries", "legacy_entries"])
replay = read(checks / "unchanged.json")
assert not replay["partial"] and replay["recipe_id"] == recipe
assert replay["provider_state"] is None and replay["provider_code"] is None
assert all(replay[key] == 0 for key in ["document_calls", "input_tokens", "embedded_units", "partitioned_sources"])
assert replay["index_published"] and replay["index_reason"] is None
manifest = out / "store" / "semantic" / replay["function_digest"] / "generation.json"
generation = read(manifest)
assert generation["coverage"] == "complete" and generation["recipe_id"] == recipe
assert generation["function_digest"] == replay["function_digest"] == status["profile"]["function_digest"]
assert generation["dimensions"] == 768 and status["profile"]["recipe_id"] == recipe
assert generation["source_revision"] == status["source_revision"]
assert generation["count"] == status["index"]["entries"] == status["cache"]["entries"]
root = work / "corpus"
rows = []
for path in root.rglob("*"):
    if path.is_file() and not path.is_symlink():
        data = path.read_bytes()
        rows.append([path.relative_to(root).as_posix(), len(data), hashlib.sha256(data).hexdigest()])
assert sorted(rows) == read(work / "manifest.json")["files"]
(checks / "published-generation.json").write_bytes(manifest.read_bytes())
result = {"overview_complete": True, "sources": 67663, "file_locations": 67585,
          "distinct_vectors": generation["count"], "unchanged_document_calls": 0}
(checks / "completion.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result))
PY
```

The 78 admitted sources without cards still require current completed partitions.
The 67,585 current locations may share exact-input vectors; source count is not a
model-call count. Preserve all phase usage reports and sum actual `document_calls`,
`input_tokens`, `embedded_units` and `bootstrap.provider_ms`. With batch one, exact
distinct-token accounting additionally requires every reported call to commit and
total new vectors to equal the final fresh-cache count; otherwise report attempted
usage separately and leave exact distinct tokens unknown. The private fully automated
run receipt also records binary/profile/manifest hashes, phase budgets and resource
stops. These public commands reproduce its CLI boundaries without depending on a
private corpus path, task label, weight copy or orchestration script.

**2026-10-10 continuation:** historical results below do not cover the new explicit
geometry/body representation or generation Metal controls. This cloud sandbox is
Linux x86_64 with a four-CPU cgroup quota, five affinity-visible CPUs and 32 GiB memory;
there is no exposed GPU or macOS execution target. The subsequent Linux work selected and verified real embedding artifacts.
Real CPU generation comparisons are recorded in [validation](validation.md).
For an admitted macOS development bundle and explicitly selected profile, run:

```sh
CF_EMBED_DEV_PROFILE=/absolute/profile.json cargo test --locked --test embed_worker \
  -- --ignored --exact dev_real_worker_batched_and_single_sequence_vectors_agree --nocapture
```

This workload requires document batch >= 8, includes reordered/heterogeneous document
batches, finite/unit-norm checks, repeated full-window queries and limit+1 refusal.
Record the printed descriptor, dimensions, card/input/batch limits, device and bundle
identity. Run separately for every proposed geometry/dimension; a small card limit only
certifies small-card batching. Compare held-out retrieval/task outcomes before selecting
a profile. macOS generation still needs native denial/owner-death, memory and Metal
allocation/reserve checks on the real target. No result from Linux closes those rows or
the signed installed-artifact requirement. GPU access and artifacts are prerequisites,
not a reason to add an unvalidated backend or infer a hardware pass from a mock.

On 2026-10-08 the owner first moved the remaining real-model and timing measurements
to a cloud agent, then reversed that decision the same day: the baseline stores are too
large to copy. Rows 1–3, 5, 6 and 7 below ran on the development host (Apple silicon,
18 GiB), one job at a time, with results recorded in [validation](validation.md).

The selected profile is EmbeddingGemma 2. Nemotron 3 Embed 1B's G2 runs were INVALID,
and the owner closed it.

For that 2026-10-08 baseline, row 4 and two record-only timings remained. The newer
continuation above adds open numerical, scale and hardware checks; the historical
table does not establish their acceptance. The baseline timing attempts swapped.

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
