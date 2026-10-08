#!/usr/bin/env bash
# The merge gates of spec 001 § Tasks and exact acceptance, for one tree:
#   scripts/gates.sh TREE OUTDIR
# fmt, clippy and the full test suite on the default toolchain, then check and
# clippy under the actual Rust 1.90 toolchain (its bin directory first on PATH,
# never `cargo +1.90.0`). Prints one `GATE <name> exit=<n>` line per gate; logs
# go to OUTDIR. TGT and TGT_MSRV override the two target directories;
# MSRV_BIN overrides the 1.90 bin directory.
set -u
TREE=$1
OUT=$2
mkdir -p "$OUT"
cd "$TREE" || exit 99
git status --porcelain > "$OUT/status.txt"
git diff HEAD > "$OUT/tracked.patch"
git ls-files --others --exclude-standard > "$OUT/untracked.txt"
run() { local name=$1; shift; "$@" > "$OUT/$name.log" 2>&1; echo "GATE $name exit=$?"; }
export CARGO_TARGET_DIR=${TGT:-target/gates}
{ rustc --version; cargo --version; cargo clippy --version; } > "$OUT/default-versions.log" 2>&1
run fmt cargo fmt --check
run clippy cargo clippy --locked --all-targets -- -D warnings
run test cargo test --locked --no-fail-fast
HOST=$(rustc -vV | sed -n 's/^host: //p')
MSRV_BIN=${MSRV_BIN:-${RUSTUP_HOME:-$HOME/.rustup}/toolchains/1.90.0-$HOST/bin}
export PATH="$MSRV_BIN:$PATH"
export CARGO_TARGET_DIR=${TGT_MSRV:-target/gates-msrv}
{ command -v rustc cargo clippy-driver; rustc --version; cargo --version; clippy-driver --version; } > "$OUT/msrv-versions.log" 2>&1
run msrv-check cargo check --all-targets --locked
run msrv-clippy cargo clippy --locked --all-targets -- -D warnings
