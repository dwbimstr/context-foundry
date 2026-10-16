#!/bin/bash
# The merge gates (scripts/gates.sh) for a cloud session, run with the environment the
# receipts assume, with an explicit override for a different local layout:
#   bash scripts/cloud/gates.sh OUTDIR
# CF_GATE_ENV may name a trusted, non-secret shell environment file. It is sourced
# after environment.env. Incremental compilation and dev/test debuginfo must stay
# disabled. RUSTUP_TOOLCHAIN is then unset: the runner uses PATH for its default
# compiler and prepends the actual Rust 1.90 bin directory for the MSRV checks.
# Effective non-secret settings and the override's identity go to OUTDIR/env.txt.
set -euo pipefail
[ $# -eq 1 ] || { echo "usage: bash scripts/cloud/gates.sh OUTDIR" >&2; exit 2; }
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
env_file=$root/scripts/cloud/environment.env
cf_gate_override=${CF_GATE_ENV:-}
cf_gate_override_sha=none
if [ -n "$cf_gate_override" ]; then
  [ -f "$cf_gate_override" ] && [ -r "$cf_gate_override" ] && [ -s "$cf_gate_override" ] ||
    { echo "CF_GATE_ENV must name a readable, nonempty file" >&2; exit 2; }
  cf_gate_override=$(cd -- "$(dirname -- "$cf_gate_override")" && printf '%s/%s' "$PWD" "$(basename -- "$cf_gate_override")")
  cf_gate_override_sha=$(sha256sum < "$cf_gate_override")
  cf_gate_override_sha=${cf_gate_override_sha%% *}
fi
mkdir -p "$1"
out=$(cd "$1" && pwd)
set -a
. "$env_file"
[ -z "$cf_gate_override" ] || . "$cf_gate_override"
set +a
for name in CARGO_INCREMENTAL CARGO_PROFILE_DEV_DEBUG CARGO_PROFILE_TEST_DEBUG; do
  [ "${!name:-}" = 0 ] || { echo "$name must remain 0 for cloud gates" >&2; exit 2; }
done
unset RUSTUP_TOOLCHAIN
{
  printf 'CF_GATE_ENV=%s\nCF_GATE_ENV_SHA256=%s\n' "$cf_gate_override" "$cf_gate_override_sha"
  # Only known configuration names are eligible for receipts. Private archive
  # configuration may contain signed URLs; never log it or arbitrary exports.
  names=(CARGO_HOME RUSTUP_HOME LIBCLANG_PATH CARGO_BUILD_JOBS MSRV_BIN TGT TGT_MSRV
    CF_GENERATION_REQUIRED CF_SETUP_RECEIPT)
  while IFS='=' read -r name value; do
    [[ $name =~ ^[A-Z_][A-Z0-9_]*$ ]] || continue
    [[ $name == CF_RESUME_* ]] || names+=("$name")
  done < "$env_file"
  for name in "${names[@]}"; do
    if [ -n "${!name+set}" ]; then echo "$name=${!name}"; else echo "$name unset"; fi
  done
  printf 'rustc=%s\ncargo=%s\n' "$(command -v rustc || true)" "$(command -v cargo || true)"
} > "$out/env.txt"
exec bash "$root/scripts/gates.sh" "$root" "$out"
