#!/bin/bash
# The merge gates (scripts/gates.sh) for a cloud session, run with the environment the
# receipts assume whatever the session's own variables hold (FOLLOWUP F16):
#   bash scripts/cloud/gates.sh OUTDIR
# It applies scripts/cloud/environment.env (the debuginfo-off build profiles that keep
# target/gates within the disk allowance, and the tokenizer path the learning tests
# read), then unsets RUSTUP_TOOLCHAIN, because scripts/gates.sh runs the default
# toolchain and then the actual Rust 1.90 itself. The file's variables, as the gates saw
# them, go to OUTDIR/env.txt.
set -euo pipefail
[ $# -eq 1 ] || { echo "usage: bash scripts/cloud/gates.sh OUTDIR" >&2; exit 2; }
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
env_file=$root/scripts/cloud/environment.env
mkdir -p "$1"
out=$(cd "$1" && pwd)
set -a
. "$env_file"
set +a
unset RUSTUP_TOOLCHAIN
for name in $(grep -oE '^[A-Z_][A-Z0-9_]*=' "$env_file" | tr -d =); do
  if [ -n "${!name+set}" ]; then echo "$name=${!name}"; else echo "$name unset"; fi
done > "$out/env.txt"
exec bash "$root/scripts/gates.sh" "$root" "$out"
