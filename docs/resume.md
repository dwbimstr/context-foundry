# Resume on another machine

Exact steps to continue this work on a different Mac: what to copy from the original
machine, what to clone from GitHub, and how to prove the new machine reproduces the
recorded results before measuring anything. Background is in the
[context packet](context-packet.md); the measurements themselves are in the
[measurement handoff](measurement-handoff.md).

## Requirements

- **macOS on Apple silicon.** The embedding worker needs Metal, and the stores, binaries
  and worker bundles below are macOS arm64 builds. Linux is untested for the core and
  unsupported for the worker.
- **At least 32 GiB of memory** for measurements (one job at a time), about 20 GB of free
  disk, and administrator rights (step 1).
- **Tools:** Xcode command-line tools (`xcode-select --install`), `rustup`, `cmake`,
  `python3`, `node`, `git`.

## Why the paths must be identical

A store is bound to its corpus by absolute path: the workspace ID is the SHA-256 of the
canonical root path, and a store refuses any other root (`wrong_workspace`). The frozen
G1 baseline stores are bound to these two paths:

| Corpus | Required path | Workspace ID |
| --- | --- | --- |
| rust-lang/rust 1.99.0 | `/Users/satishlomte/VSC_DEV/corpora/rust-1.99.0` | `e148bc0a63340c19…` |
| oh-my-pi | `/Users/satishlomte/VSC_DEV/oh-my-pi` | `af49817228f5f515…` |

The G1 and G2 scripts also read `~/VSC_DEV/...`, and the frozen G2 profiles and token
counter use fixed `/private/tmp` paths. So the new machine recreates
`/Users/satishlomte/VSC_DEV` exactly and links `~/VSC_DEV` to it. Indexing the corpora at
any other path gives different stores, and a different compiler graph would need a new
SCIP run; results would no longer be comparable with the frozen baselines.

## On the original machine: what to copy

Copy only the private datasets; everything else is cloned. After the G2 agent has
finished (its final report and `FROZEN.txt` are in `datasets/context-foundry-g2-runs`):

```sh
cd /Users/satishlomte/VSC_DEV
COPYFILE_DISABLE=1 tar --no-mac-metadata -cf cf-private.tar \
  datasets/context-foundry-citymap \
  datasets/context-foundry-g2 \
  datasets/context-foundry-g2-harness \
  datasets/context-foundry-g2-runs
shasum -a 256 cf-private.tar > cf-private.tar.sha256
```

This is about 4–5 GB (more if the G2 agent preserved prepared stores). Transfer both files
privately; they hold private datasets and model files and never go into Git. Optional:
add `models/Nemotron-3-Embed-1B-BF16` (2.1 GB) only to redo the Nemotron conversion, and
`models/Nemotron-3-Embed-1B-BF16-4bit-d0408b94` only for the rollback re-embedding row of
the handoff.

## On the new machine

### 1. Recreate the path layout

```sh
sudo mkdir -p /Users/satishlomte/VSC_DEV/corpora
sudo chown -R "$(id -un)":staff /Users/satishlomte
# Only when your home directory is not /Users/satishlomte:
[ "$HOME" = /Users/satishlomte ] || ln -s /Users/satishlomte/VSC_DEV "$HOME/VSC_DEV"
```

### 2. Clone the code and the corpora

```sh
cd /Users/satishlomte/VSC_DEV
git clone https://github.com/dwbimstr/context-foundry.git
git -c core.autocrlf=false clone --depth 1 --branch 1.99.0 \
  https://github.com/rust-lang/rust.git corpora/rust-1.99.0
git -c core.autocrlf=false clone https://github.com/can1357/oh-my-pi.git
git -C oh-my-pi checkout --detach 5b8d5b8a15ab1711597584aadbdc112c55befed1
git clone https://github.com/ggml-org/llama.cpp.git
git -C llama.cpp checkout --detach b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea
```

Do not initialize the rust-lang/rust submodules and do not run `bun install` in
oh-my-pi: the stores were built from exactly the committed trees. Check the commits:

```sh
git -C corpora/rust-1.99.0 rev-parse HEAD   # b940084d7eb6a299eb4bfeb8e34901bc051e7ac4
git -C oh-my-pi rev-parse HEAD              # 5b8d5b8a15ab1711597584aadbdc112c55befed1
```

### 3. Restore the private datasets

```sh
cd /Users/satishlomte/VSC_DEV
shasum -a 256 -c cf-private.tar.sha256
tar -xf cf-private.tar
xattr -dr com.apple.quarantine datasets 2>/dev/null || true
(cd datasets/context-foundry-citymap/inputs && shasum -a 256 -c SHA256SUMS)
```

Then compare the files named in `datasets/context-foundry-citymap/FROZEN-G1v4.txt`,
`datasets/context-foundry-g2-harness/FROZEN.txt` and
`datasets/context-foundry-g2-runs/FROZEN.txt` with `shasum -a 256`; every hash must
match.

### 4. Install the toolchains and the token counter

```sh
rustup toolchain install stable --component clippy,rustfmt
rustup toolchain install 1.90.0 --component clippy,rustfmt
cp -R /Users/satishlomte/VSC_DEV/datasets/context-foundry-citymap/inputs/o200k /private/tmp/cf-o200k
[ -x /private/tmp/cf-o200k/target/release/o200k_count ] \
  || cargo build --release --manifest-path /private/tmp/cf-o200k/Cargo.toml
```

`/private/tmp` is emptied when macOS restarts; repeat the last two commands after a
reboot (and step 8's restore).

### 5. Prove the corpora match the stores

Index a scratch copy of each baseline store with the baseline binary. Nothing may change:

```sh
I=/Users/satishlomte/VSC_DEV/datasets/context-foundry-citymap/inputs
mkdir -p /private/tmp/cf-check
cp -cR $I/store-rust-g1base /private/tmp/cf-check/rust
cp -cR $I/store-bun-g1base /private/tmp/cf-check/bun
$I/foundry-baseline --store /private/tmp/cf-check/rust index /Users/satishlomte/VSC_DEV/corpora/rust-1.99.0
$I/foundry-baseline --store /private/tmp/cf-check/bun index /Users/satishlomte/VSC_DEV/oh-my-pi
rm -rf /private/tmp/cf-check
```

Expected: rust-lang/rust `changed: 0, unchanged: 60739, deleted: 0, excluded: 75`;
oh-my-pi `changed: 0, unchanged: 8436, deleted: 0, excluded: 60`; `failures: 0` for
both (recorded 2026-10-08). Any change means a wrong commit, a line-ending conversion or
an extra file; fix it before continuing. `wrong_workspace` means step 1's path is wrong.

### 6. Build and gate the code

```sh
cd /Users/satishlomte/VSC_DEV/context-foundry
cargo build --release --locked          # first build fetches the grammar forks
scripts/gates.sh . /private/tmp/cf-gates   # every GATE line must say exit=0
node scripts/check-links.mjs               # "errors": []
```

The full suite takes 16–20 minutes; one test
(`tests/mcp.rs::http_dropped_stream_keeps_slot_and_overload_keeps_control_traffic_serviceable`)
is load-sensitive and passes when rerun alone.

### 7. Reproduce G1 without a profile (setup acceptance)

```sh
I=/Users/satishlomte/VSC_DEV/datasets/context-foundry-citymap/inputs
W=/private/tmp/cf-g1; mkdir -p $W
cp target/release/foundry $W/foundry
cp -cR $I/store-rust-g1base $W/store-rust; cp -cR $I/store-bun-g1base $W/store-bun
$W/foundry repair-index --store $W/store-rust
$W/foundry repair-index --store $W/store-bun
Q=/Users/satishlomte/VSC_DEV/datasets/context-foundry-citymap/g1quick.sh
$Q checker   $W/foundry $W/store-rust $W/g1-checker   1/1
$Q qualified $W/foundry $W/store-rust $W/g1-qualified 1/1
$Q bun       $W/foundry $W/store-bun  $W/g1-bun       1/1
```

Run on an idle host, one set at a time. Expected: all three PASS with checker D1 97.8%,
D2 98.0%, D3 92.8%, U1 86.6%; qualified D4 94.0%, U2 81.2%; bun B1 99.1%, B2 96.8%, and
0 duplicate, missing, retried or apparatus-error tasks ([validation](validation.md),
G1 city-map verdict). Different numbers mean the setup differs from the original; do not
measure until they match. `INVALID` (retries) means the host was too loaded; rerun.

### 8. Restore or rebuild the G2 runtime

The frozen G2 profiles name their files under `/private/tmp/cf-city/g2/`
(`models/`, `bundles/`, `bin/foundry`, `scratch-*`), and the wrappers in
`datasets/context-foundry-g2-runs/bin` call `/private/tmp/cf-city/g2/bin/foundry`.

First restore the two models (preserved in `artifacts/models` on 2026-10-08):

```sh
A=/Users/satishlomte/VSC_DEV/datasets/context-foundry-g2-runs/artifacts
mkdir -p /private/tmp/cf-city/g2/models /private/tmp/cf-city/g2/bundles /private/tmp/cf-city/g2/bin
cp -R $A/models/. /private/tmp/cf-city/g2/models/
shasum -a 256 /private/tmp/cf-city/g2/models/*/*   # must equal the FROZEN.txt lines
```

- **If `artifacts/` also holds the binaries and bundles** listed in `FROZEN.txt` (and any
  addendum), copy each back to the path `FROZEN.txt` names (`bin/foundry`,
  `bundles/*.app` and their `.entitlements.plist`), check every SHA-256 against
  `FROZEN.txt`, then run `xattr -dr com.apple.quarantine /private/tmp/cf-city/g2`. The
  frozen profiles then load unchanged.
- **Otherwise rebuild the worker, the binary and the bundles:**

  ```sh
  cd /Users/satishlomte/VSC_DEV/context-foundry
  LLAMA_CPP_DIR=/Users/satishlomte/VSC_DEV/llama.cpp \
    cargo build --release --locked --features embed-worker --bin foundry-embed
  cp target/release/foundry /private/tmp/cf-city/g2/bin/foundry
  P=/Users/satishlomte/VSC_DEV/datasets/context-foundry-g2-runs/profiles
  scripts/embed-worker-bundle.sh --binary target/release/foundry-embed \
    --out /private/tmp/cf-city/g2/bundles/FoundryEmbedG2Gemma.app --profile $P/gemma.json
  scripts/embed-worker-bundle.sh --binary target/release/foundry-embed \
    --out /private/tmp/cf-city/g2/bundles/FoundryEmbedG2Nemotron.app --profile $P/nemotron.json
  ```

  The bundle script prints the signed executable's SHA-256, which will differ from the
  frozen profiles' `worker.executable_sha256`. Write copies of the two profiles that
  change only that field, keep every `descriptor` field identical (so the embedding
  function and its cache keys are unchanged), point the wrappers in
  `datasets/context-foundry-g2-runs/bin` at the copies, and append an addendum to
  `FROZEN.txt` naming the new binary, bundle and profile hashes and the reason, before
  any G2 run.

Then follow the [measurement handoff](measurement-handoff.md) from row 1.

## Continuing code work

Code work needs only steps 2, 4 (toolchains) and 6. The working rules (focused tests
while developing, `scripts/gates.sh` once per merge, cross-lab review before merge, link
check before each Markdown commit) are in the [context packet](context-packet.md).
