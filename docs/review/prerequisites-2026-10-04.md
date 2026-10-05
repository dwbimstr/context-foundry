# External prerequisite evidence — 2026-10-04

The owner answered the open 005/009/013 prerequisites on 2026-10-04:
- 005: the installed rust-analyzer producer and the rust-lang/rust corpus;
- 009: downloads, the runtime install and the USearch build;
- 013: task-checker labels and the LibTorch download;
- signing/notarization: decide later.

This record holds the captain-executed checks. The host is macOS 27.0.1 on Apple M3 Pro
(18 GiB); the toolchain is the repository floor, rustc 1.90.0 (1159e78c4 2025-09-14),
except where noted. Scratch crates lived under `/private/tmp` and are not part of the
repository.
These are build and smoke checks only. They are not product integration, corpus-scale,
quality or package acceptance.

## Downloads

| Artifact | Identity | Location |
| --- | --- | --- |
| MLX embedding artifact | `mlx-community/Nemotron-3-Embed-1B-BF16-4bit@d0408b94c50fc327b6ea37dce7409c51e020a4d8`; `model.safetensors` SHA-256 `73b06890e9d44ac792c98d38cd1d624ad4c45b5e0a4106bae6bef1ae356dd62f` and `tokenizer.json` SHA-256 `797410dfb649a5b9ba92bc4fef7dbf4022d00e73de6867c4ac199a8846439421`, both equal to the Hugging Face LFS oids; 641 MiB | `~/VSC_DEV/models/Nemotron-3-Embed-1B-BF16-4bit-d0408b94` |
| LibTorch 2.11.0 CPU, macOS arm64 | `download.pytorch.org/libtorch/cpu/libtorch-macos-arm64-2.11.0.zip` (redirects to download-r2); 77,607,410 B; observed SHA-256 `0edc138545c84879240cc07994f1e9a0ed35a1102ffe2b99e13f3a7b57b323e9` (no published checksum compared); `build-version` 2.11.0 | `~/VSC_DEV/vendor/libtorch-2.11.0` |
| rust-lang/rust 1.99.0 (005 T003 corpus) | annotated tag `daa8d75b`, commit `b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`; shallow clone without submodules; 62,035 tracked files | `~/VSC_DEV/corpora/rust-1.99.0` |
| 013 decision checkpoint (owner-authorized 2026-10-04) | `convaiinnovations/laya-typed-decisions@1a793eb568e6718f15941d08f85432581df534e3`, ungated, card license apache-2.0; 7 files, 846,203,578 B, each equal to its LFS SHA-256 or git blob ID; `model.safetensors` SHA-256 `4fa56de72383a9d3efa9cfa78955733c81b9fc8067a587ca4beb82c78107a24e`, `tokenizer/tokenizer.json` `6c8aaa9a…0d30`, `encoder/config.json` `5268d24a…ae6d` (ModernBERT-large, 28 layers, hidden 1024, vocab 50,368) | `~/VSC_DEV/models/laya-typed-decisions-1a793eb5` |

## 005 T001 — jailed producer run

- **Producer.** Homebrew `rust-analyzer` formula 2026-08-31.
  - Weekly tag `2026-08-31`, commit `f8996691e991a4dc3c6f135e0fc04fc5561e4e9a`.
  - `--version` prints `rust-analyzer 0.0.0 (f8996691e9 2026-08-30)`.
  - Binary SHA-256 `91c40055ac1da1e21c0983fda9f3bee75253a3e84a5ed9d3ab6b5e4df4d020a8`.
  - Toolchain: rustc 1.97.1 (Homebrew).
- **Recheck at that tag.** In `crates/rust-analyzer/src/cli/scip.rs`:
  `load_out_dirs_from_check: true` and `ProcMacroServerChoice::Sysroot` (lines 52-53),
  and `PositionEncoding::UTF8CodeUnitOffsetFromLineStart` (lines 217-218, 321).
- **Jail.** A `sandbox-exec` profile: `(allow default) (deny network*) (deny file-write*)`,
  with writes re-allowed only under the private scratch directory and on `/dev/null`,
  `/dev/tty`, `/dev/fd/*` and `/dev/ttys*`. The environment was `env -i` with
  `HOME`/`TMPDIR`/`CARGO_HOME`/`CARGO_TARGET_DIR` inside scratch and
  `CARGO_NET_OFFLINE=true`.
  - Negative checks under the same profile: a write outside scratch was denied; a
    write into the live fixture was denied; network was denied (DNS resolution failed).
  - Positive check: a scratch write was allowed.
- **Snapshot.** The fixture `tests/fixtures/semantic/workspace` (dependency-free) was
  copied, made read-only, and hashed before and after production. Both hash lists were
  identical; they are in [`producer.json`](../../tests/fixtures/semantic/producer.json).
- **Run.** Exit 0 in 5 s. The artifact
  [`index.scip`](../../tests/fixtures/semantic/index.scip) is 6,845 B, SHA-256
  `0e4d34e7fce4725a714401cdae48fac9122f44ae6b56bac988d2e751dc920080`.
- **Decode.** A plain protobuf wire read, compared with the independently enumerated
  [`expected.json`](../../tests/fixtures/semantic/expected.json):
  - every document declares encoding 1 (`UTF8CodeUnitOffsetFromLineStart`);
  - only the deprecated `range`/`enclosing_range` fields are emitted;
  - `a::parse_record` references appear at `use_one.rs` 1:14, `use_two.rs` 1:63 and
    `pointer.rs` 1:53. The Unicode before the `use_two.rs` occurrence makes the UTF-16
    column 53, which the artifact correctly does not use;
  - one `b::parse_record` reference appears in `use_b.rs`;
  - both definitions carry role 1 at 2:7–19.
  - T001's typed-range case therefore needs a constructed artifact variant.

## 009 — USearch on the Rust 1.90 floor

- `usearch = "=2.26.2"` (crates.io checksum `b43f3a7c8d3b100d53c1a34e9ce4cd76cc83627bdb4fcd035d4e82102778aeee`,
  Apache-2.0) with Apple clang 21.0.0. The release build took 35 s.
- The smoke used 2048 dimensions, `MetricKind::Cos` and `ScalarKind::F16`. It ran add 3
  → search (self nearest) → save → load (size 3) → remove → re-add a new vector under the
  same key → save → load → search, which returned the updated key. It passed.
- Not covered: stale-profile, rebuild-from-cache and corrupt-index cases (009 T001
  tests), and corpus scale.

## 013 — tch, LibTorch and tokenizers on the Rust 1.90 floor

- `tch = "=0.24.0"` (torch-sys 0.24.0) against the LibTorch tree above. The release
  build took 39 s.
  - A CPU float32 `nn::linear` head ran 20 SGD steps: loss 1.1705 → 0.2865.
  - VarStore save/reload through safetensors was exact (max absolute difference 0).
  - Max RSS 110,100,480 B; peak footprint 61,883,136 B.
- `tokenizers = 0.23.2` (`default-features = false`, `onig`) built in 38 s. It loaded a
  real `tokenizer.json` (the embedding artifact's; the ModernBERT/typed-choice
  checkpoint is separately obtained and not local) and encoded a `passage:`-prefixed
  line.
- macOS SIP strips `DYLD_LIBRARY_PATH` across `/usr/bin/time`. Measure with
  `/usr/bin/time -l /usr/bin/env DYLD_LIBRARY_PATH=… <binary>` or use an rpath.
- Not covered: ModernBERT weights, the package target, signing/notarization.
- Update, same day: the ModernBERT/typed-choice checkpoint is now local (Downloads
  table). Rust `tokenizers` 0.23.2 loaded its `tokenizer/tokenizer.json` and encoded
  nine renderer-shaped inputs with special tokens on and off: the instruction, both
  space-prefixed options, states with locator lines, Unicode/CRLF/emoji, a literal
  `[MASK]`, whitespace runs and code. All 18 ID sequences equal the publisher stack's
  (`AutoTokenizer`, transformers 5.18.0). Special IDs: CLS 50281, SEP 50282, PAD 50283,
  MASK 50284. Scratch: `/private/tmp/cf-013-tok`.
