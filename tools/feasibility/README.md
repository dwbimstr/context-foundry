# Disposable Rust feasibility probes

These are reproducible integration experiments, not a second application or production
model implementation. See [results and limitations](../../docs/review/feasibility.md).
The root product crate does not depend on this crate. All inputs are synthetic.
Never point these executables at a live store or grant them a home/credential directory.

## Pins and preparation

The 2026-10-10 [dependency review](../../docs/review/tch-026.md) coordinates the
proposed Python torch 2.13.0 update (PR #2) with this scratch crate's `tch` 0.26.0
pin and the product migration (PR #9). These are the current reproduction inputs;
the measurements below and files in `results/` used the earlier 2.11.0/0.24.0
pairing and have **not** been regenerated or accepted for 2.13.0. Updating only
Python breaks the documented `LIBTORCH_USE_PYTORCH=1` Rust probe build.

- Cargo.lock pins Rust dependencies. Local model/index builds used Rust 1.97.1.
- MCP was built with Rust 1.90 in `rust:1.90-slim-bookworm`, image digest
  `sha256:64232e656c058f4468e8d024e990acff04f0fd5a5c0a88a574dc37773d7325c9`.
- Historical isolated Python 3.12.11 environment: torch 2.11.0, transformers 5.17.0,
  mlx 0.32.3, mlx-metal 0.32.3, mlx-lm 0.31.3, safetensors 0.8.0.
  These third-party dependencies served the publisher loader and numerical reference;
  no first-party Python model code was written. The complete installed package capture
  originally populated [runtime-requirements.txt](runtime-requirements.txt), now
  updated to the coordinated reproduction inputs. It is not a distributable runtime
  lock with wheel hashes and notices.
- Download exact public revisions/files in [artifacts.json](results/artifacts.json),
  verify their SHA256, and review their separate licenses before use. Fetch config
  and unchanged `nemotron3_embed_mlx.py` from the same MLX revision. Fetch
  `encoder/config.json` (save as `encoder_config.json`) and `rl_agent_config.json`
  from the same typed-decision revision. Download unchanged `laya/common.py` from
  revision `4066d5d5fbf08b66c6757ddeedbd797bd7655bc0` into `input/reference/common.py`.
  No upstream source or weights are included here.

Use a newly created private scratch directory (`PROBE_ROOT`) with `input/`, `output/`
and `venv/`. Model folders must be named `input/Nemotron-3-Embed-1B-BF16-4bit` and
`input/laya-typed-decisions`; tokenizer files live under the latter's `tokenizer/`.
Preparation may download pinned dependencies; **runtime runs offline in the jail**.
Copy this small crate to scratch for building. Do not put weights/venv/targets in Git.
Set a supervisor deadline of ten minutes per model call; terminate only owned processes
on timeout. Actual calls here completed in seconds; no timeout outcome was claimed.

## Build and actual macOS invocation shape

Build selected binaries only:

```sh
CARGO_BUILD_JOBS=2 PYO3_PYTHON="$PROBE_ROOT/venv/bin/python" \
  cargo build --locked --features pyo3 --bin mlx-probe
CARGO_BUILD_JOBS=2 LIBTORCH_USE_PYTORCH=1 \
  PYO3_PYTHON="$PROBE_ROOT/venv/bin/python" \
  PATH="$PROBE_ROOT/venv/bin:/opt/homebrew/bin:/usr/bin:/bin" \
  cargo rustc --locked --features tch,pyo3 --bin decision-probe -- \
  -C "link-arg=-Wl,-rpath,$PROBE_ROOT/venv/lib/python3.12/site-packages/torch/lib"
cargo build --locked --features usearch --bin index-probe
rustc --edition 2024 nproc_launcher.rs -o "$PROBE_ROOT/nproc-launcher"
```

The source folder is the working directory for these commands. An initial toy
optimizer compile is not evidence of model parity and was omitted from this bundle.
`decision.rs` tests real weights. The failed initial RoPE result is retained deliberately.

Create one `.app` per executable with `Contents/MacOS/probe` and an Info.plist containing
CFBundleIdentifier (unique `org.context-foundry.feasibility.*`), CFBundleExecutable
`probe`, CFBundlePackageType `APPL`, CFBundleVersion `1`. Sign with
`codesign -s - --entitlements ENTITLEMENTS APP`. Entitlements used:

- `com.apple.security.app-sandbox=true`;
- model probes only: `com.apple.security.temporary-exception.files.absolute-path.read-only`
  for exact scratch `input/`, `venv/` and `/opt/homebrew/`;
- decision/index only: corresponding `absolute-path.read-write` for owned `output/`;
- no network, user-selected-file, child-inheritance or disable-library-validation grants.

The Homebrew read grant is a development limitation. A release packages private pinned
libraries, signs them appropriately and proves its actual narrower grants. Do not
copy these development grants into release entitlements.

From scratch, launch the signed bundle after setting the irreversible per-process
creation limit, preserving only the explicitly shown environment:

```sh
env -i PATH=/usr/bin:/bin \
  PYTHONPATH="$PROBE_ROOT/venv/lib/python3.12/site-packages" \
  PYTHONDONTWRITEBYTECODE=1 HF_HUB_OFFLINE=1 TRANSFORMERS_OFFLINE=1 \
  TOKENIZERS_PARALLELISM=false OMP_NUM_THREADS=2 \
  "$PROBE_ROOT/nproc-launcher" "$PROBE_ROOT/Mlx.app/Contents/MacOS/probe" \
  "$PROBE_ROOT/input/Nemotron-3-Embed-1B-BF16-4bit"
```

For Decision.app use arguments `reference "$PROBE_ROOT"`, then `rust "$PROBE_ROOT"`.
Reference writes only synthetic token IDs, reference hidden/logit/gradient tensors in
output. Then `reference-encoder` followed by `rust-encoder` checks selected QKV gradients.
Reference and Rust execute sequentially. `rust` is the full Rust model/head calculation;
PyO3 exists in this test binary solely for its separate reference mode. The recorded
reference/QKV runs preceded the process-limit addition; final head run and MLX were
repeated under that limit. Result scope is stated in each JSON.

For Index.app pass `"$PROBE_ROOT/output/vector.index"`. Sandbox probe variants are
standalone rustc sources, signed/bundled with only app-sandbox entitlement. Create
`outside-sentinel` under scratch, then feed `synthetic input\n` on stdin. `limit_probe`
asserts spawn/fork denied and threads work. `nproc_launcher` is macOS-only (Darwin
RLIMIT_NPROC=7), sets soft/hard zero before exec, and is not a portable sandbox library.

## MCP build and jailed runtime

Copy only this crate into a dedicated Docker-visible scratch directory. Existing Colima
on this host did not expose `/private/tmp` bind content; use a dedicated narrow visible
scratch path, not a whole repository/home mount. Build dependencies with bounded CPU,
RAM, pids, read-only root and writable scratch Cargo/target. Build command inside the
pinned image was `cargo build --features rmcp,tokio --bin protocol-probe`; repeat with
`--locked --offline` once dependencies are fetched. Optional ML/vector features are off.

Run the built Linux executable in a separate container:

```sh
docker run --rm --network none --read-only --cap-drop ALL \
  --security-opt no-new-privileges --pids-limit 64 --memory 256m --cpus 1 \
  --user 65534:65534 --tmpfs /tmp:rw,nosuid,nodev,size=16m \
  -v "$PROTOCOL_EXE:/probe:ro" rust:1.90-slim-bookworm /probe
```

The probe intentionally spawns its own Rust SDK server over stdio. It is a protocol
fixture, not the zero-child model profile. The predecode test uses the same capped
codec as its server. No production engine admission, adversarial notification flood,
provider, host adapter or store is simulated by this happy path.

## Retained evidence

`results/*.json` contains synthetic results and public artifact identities only.
`results/executables.json` binds the local tested binaries; signing/runtime paths make
these host-specific fingerprints, not a reproducible release-build assertion.
The repository excludes weights, tensor outputs, signed bundles, venvs and targets.
Raw build/OS logs stayed in private scratch; they are not required to reproduce checks.
Use commands and fixed identities above; do not execute downloaded reference code
outside the sandbox or treat this source as production-ready just because a probe passed.
