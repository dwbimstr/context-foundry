#!/bin/bash
# Check a Context Foundry cloud session before builds or model work: host limits,
# CPU features, toolchains, the pinned llama.cpp object store, model files and the
# private evidence. Prints PASS/FAIL/INFO lines; exits 1 if anything required fails.
set -u
fail=0
pass() { echo "PASS $1: $2"; }
bad() { echo "FAIL $1: $2"; fail=1; }
info() { echo "INFO $1: $2"; }
need_file() { if [ -s "${2:-}" ]; then pass "$1" "$2"; else bad "$1" "missing ${2:-<unset>}"; fi; }

mem=$(awk '/MemTotal/ {printf "%.1f GiB", $2 / 1048576}' /proc/meminfo)
swap=$(awk '/SwapTotal/ {printf "%.1f GiB", $2 / 1048576}' /proc/meminfo)
info host "$(uname -m), $(nproc) CPUs, $mem RAM, $swap swap, $(df -h --output=avail / | tail -n 1 | tr -d ' ') free on /"
info cpu "$(awk -F': ' '/model name/ {print $2; exit}' /proc/cpuinfo)"
flags=$(grep -m1 '^flags' /proc/cpuinfo)
for flag in avx2 avx512f avx512_bf16; do
  if [[ " $flags " == *" $flag "* ]]; then info "cpu $flag" yes; else info "cpu $flag" no; fi
done
[[ " $flags " == *" avx512_bf16 "* ]] ||
  info simd "no avx512_bf16: the SIMD revision-5 Gemma worker refuses here; do not substitute another embedding function"

# Every variable environment.env sets to a value must reach the session (FOLLOWUP F16):
# without them the toolchain pin, the debuginfo-off build profiles and the pinned input
# paths are all missing, and the checks below fail without naming the cause.
env_file=$(dirname "${BASH_SOURCE[0]}")/environment.env
unset_vars=() other_vars=()
while IFS='=' read -r name value; do
  [[ $name =~ ^[A-Z_][A-Z0-9_]*$ && -n $value ]] || continue
  value=${value#\"} value=${value%\"}
  if [ -z "${!name+set}" ]; then unset_vars+=("$name")
  elif [ "${!name}" != "$value" ]; then other_vars+=("$name"); fi
done < "$env_file"
if [ ${#unset_vars[@]} -eq 0 ]; then
  pass environment "every variable $env_file sets is set"
else
  bad environment "missing ${#unset_vars[@]} of the variables $env_file sets (${unset_vars[*]}): paste the file into the environment's variables (environment menu, Edit) and start a new session; for this shell only, run set -a; . $env_file; set +a"
fi
[ ${#other_vars[@]} -eq 0 ] || info environment "set to other values than $env_file: ${other_vars[*]}"

if version=$(rustc --version 2>/dev/null) && [[ $version == "rustc 1.90.0 "* ]]; then
  pass rust "$version (RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN:-unset})"
else
  bad rust "expected rustc 1.90.0, got ${version:-none}"
fi
cargo clippy --version >/dev/null 2>&1 && cargo fmt --version >/dev/null 2>&1 &&
  pass "clippy/rustfmt" "installed" || bad "clippy/rustfmt" "missing for $(rustc --version 2>/dev/null)"
need_file "host rustc" "${FOUNDRY_HOST_RUSTC:-}"
command -v cmake >/dev/null && pass cmake "$(cmake --version | head -n 1)" || bad cmake missing
# clang-sys honors an explicit override exclusively, including a library file.
# libclang-cpp is a different API and cannot satisfy bindgen's requirement.
clang_candidates=(/usr/lib/llvm-*/lib/libclang.so* /usr/lib/llvm-*/lib/libclang-[0-9]*.so*)
if [ "${LIBCLANG_PATH+set}" ]; then
  clang_candidates=()
  if [ -n "$LIBCLANG_PATH" ]; then
    clang_candidates=("$LIBCLANG_PATH"/libclang.so* "$LIBCLANG_PATH"/libclang-[0-9]*.so*)
    [ ! -f "$LIBCLANG_PATH" ] || clang_candidates=("$LIBCLANG_PATH")
  fi
fi
clang_found=0
for candidate in "${clang_candidates[@]}"; do
  case "${candidate##*/}" in
    libclang.so*|libclang-[0-9]*.so*)
      if [ -f "$candidate" ] && [ -s "$candidate" ]; then clang_found=1; break; fi ;;
  esac
done
[ "$clang_found" -eq 1 ] && pass libclang present || bad libclang missing
include=${BINDGEN_EXTRA_CLANG_ARGS:-}
include=${include#-isystem }
[ -d "$include" ] && pass "bindgen include" "$include" ||
  bad "bindgen include" "${include:-<unset>} is not a directory; set BINDGEN_EXTRA_CLANG_ARGS to -isystem $(cc -print-file-name=include)"
commit=b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea
git -C "${LLAMA_CPP_DIR:-/nonexistent}" cat-file -e "$commit^{tree}" 2>/dev/null &&
  pass llama.cpp "$commit in $LLAMA_CPP_DIR" || bad llama.cpp "commit $commit not in ${LLAMA_CPP_DIR:-<unset>}"

need_file "gemma gguf" "${CF_GEMMA_GGUF:-}"
need_file "gemma tokenizer" "${CF_GEMMA_TOKENIZER:-}"
case "${CF_GENERATION_REQUIRED:-1}" in
  1) need_file "generation gguf" "${CF_GENERATION_GGUF:-}" ;;
  0) info "generation gguf" "not configured for this workflow; generation remains unvalidated" ;;
  *) bad "generation policy" "CF_GENERATION_REQUIRED must be 0 or 1" ;;
esac
for file in tokenizer/tokenizer.json tokenizer/tokenizer_config.json rl_agent_config.json encoder/config.json; do
  need_file "013 $file" "${CONTEXT_FOUNDRY_013_CHECKPOINT:-/nonexistent}/$file"
done
if bun=$("${FOUNDRY_OMP_BUN:-/nonexistent}" --version 2>/dev/null) && [ -f "${FOUNDRY_OMP_CLI:-}" ]; then
  pass omp "Bun $bun, $FOUNDRY_OMP_CLI"
else
  info omp "OMP/Bun not installed (needed only for host-task runs)"
fi
curl -fsS -o /dev/null --max-time 15 https://huggingface.co/api/models/google/embeddinggemma-2 &&
  info network "huggingface.co reachable" || info network "huggingface.co unreachable: check the allowed domains"

archive=/workspace/${CF_RESUME_ASSET:-context-foundry-resume-20261011.tar.gz}
[ -f "$archive" ] && info evidence "archive $archive present" ||
  info evidence "archive not fetched: run scripts/cloud/fetch-resume.sh"
[ -d "${V11:-/nonexistent}" ] && info evidence "V11 restored at $V11" || info evidence "V11 not restored"
receipt=${CF_SETUP_RECEIPT:-/opt/cf/setup-receipt.txt}
[ -f "$receipt" ] && [ -s "$receipt" ] && info setup "$(head -n 2 "$receipt" | tr '\n' ' ')" ||
  bad setup "no $receipt: no setup receipt for this environment"
exit "$fail"
