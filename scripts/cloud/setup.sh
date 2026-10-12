#!/bin/bash
# Context Foundry: setup script for a Claude Code cloud environment.
#
# Paste this whole file into the environment's "Setup script" field. It runs as
# root on Ubuntu 24.04 x86_64 before Claude Code starts, and its filesystem is
# cached when it finishes in about five minutes. It must exit 0, so every step
# logs a WARN and continues instead of failing. Re-running it is cheap: finished
# steps are skipped. Network access: Custom, with the default package managers
# and scripts/cloud/allowed-domains.txt. Environment variables:
# scripts/cloud/environment.env. Verify a session with scripts/cloud/doctor.sh.
#
# Installs Rust 1.90.0 (the MSRV and the measurement toolchain), libclang/CMake
# for the llama.cpp workers, the pinned llama.cpp commit as an object store, the
# public pinned model files, the 013 tokenizer/config used by the default tests,
# OMP 18.6.0 with Bun 1.3.14 for host-task runs, and the locked Cargo
# dependencies (including the three grammar forks pinned by git revision).
# Private evidence is not fetched here; see scripts/cloud/fetch-resume.sh.
set -u
T=/opt/cf
LOG=/var/log/context-foundry-setup.log
LLAMA=b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea
BRANCH=experimental/context-ecosystem
START=$(date +%s)
mkdir -p "$T/bin" "$T/models" /workspace/.context-foundry-tools
: > "$T/setup-warnings.txt"
export PATH="/root/.cargo/bin:/opt/node22/bin:$PATH"
ok() { echo "setup: OK   $*"; }
warn() { echo "setup: WARN $*"; echo "$*" >> "$T/setup-warnings.txt"; }

# fetch URL SHA256 DEST: download once, keep only a verified file.
fetch() {
  local url=$1 sum=$2 dest=$3
  if [ -f "$dest" ] && echo "$sum  $dest" | sha256sum -c --status; then
    ok "$dest"
    return
  fi
  mkdir -p "$(dirname "$dest")"
  if curl -fsSL --retry 2 --connect-timeout 20 --max-time 240 --proto =https \
      -o "$dest.part" "$url" && echo "$sum  $dest.part" | sha256sum -c --status; then
    mv "$dest.part" "$dest" && ok "$dest"
  else
    rm -f "$dest.part"
    warn "download failed or hash mismatch: $url"
  fi
}

system_packages() {
  local need=()
  command -v cmake >/dev/null || need+=(cmake)
  command -v clang >/dev/null || need+=(clang)
  ls /usr/lib/llvm-*/lib/libclang.so* >/dev/null 2>&1 || need+=(libclang-dev)
  [ -x /usr/bin/time ] || need+=(time)
  command -v zstd >/dev/null || need+=(zstd)
  command -v pigz >/dev/null || need+=(pigz)
  command -v unzip >/dev/null || need+=(unzip)
  if [ ${#need[@]} -eq 0 ]; then
    ok "system packages present"
    return
  fi
  export DEBIAN_FRONTEND=noninteractive
  if timeout 200 apt-get update -qq &&
      timeout 200 apt-get install -y -qq --no-install-recommends "${need[@]}" >/dev/null; then
    ok "apt ${need[*]}"
  else
    warn "apt ${need[*]}"
  fi
}

rust_toolchain() {
  if ! command -v rustup >/dev/null; then
    curl -fsSL --retry 2 --max-time 120 -o /tmp/rustup-init \
      "https://static.rust-lang.org/rustup/dist/$(uname -m)-unknown-linux-gnu/rustup-init" &&
      chmod +x /tmp/rustup-init &&
      /tmp/rustup-init -y -q --profile minimal --default-toolchain none ||
      { warn "rustup install"; return; }
  fi
  if timeout 240 rustup toolchain install 1.90.0 --profile minimal \
      --component rustfmt --component clippy >/dev/null 2>&1; then
    ln -sf "$(rustup which --toolchain 1.90.0 rustc)" "$T/bin/rustc-1.90"
    ok "Rust $("$T/bin/rustc-1.90" --version)"
  else
    warn "Rust 1.90.0 toolchain"
    return
  fi
}

# Fetch the working branch's locked crates and git dependencies (the grammar forks)
# into CARGO_HOME, so sessions build without fetching. This runs in parallel from
# the start with the image's default toolchain; fetching does not depend on the
# compiler version. A timeout keeps what it already downloaded.
cargo_prefetch() {
  local src
  src=$(mktemp -d)
  if curl -fsSL --retry 2 --max-time 120 \
      "https://codeload.github.com/dwbimstr/context-foundry/tar.gz/refs/heads/$BRANCH" |
      tar -xz -C "$src" --strip-components=1 &&
      (cd "$src" && env -u RUSTUP_TOOLCHAIN timeout 270 cargo fetch --locked -q); then
    ok "cargo fetch --locked ($BRANCH)"
  else
    warn "cargo prefetch incomplete; the first session build fetches the rest"
  fi
  rm -rf "$src"
}

llama_cpp() {
  # build.rs reads the pinned commit's tree from this object store; no checkout.
  local dir=$T/llama.cpp
  if git -C "$dir" cat-file -e "$LLAMA^{tree}" 2>/dev/null; then
    ok "llama.cpp $LLAMA"
    return
  fi
  rm -rf "$dir"
  if git init -q "$dir" &&
      timeout 240 git -C "$dir" fetch -q --depth 1 --no-tags \
        https://github.com/ggml-org/llama.cpp "$LLAMA" &&
      git -C "$dir" cat-file -e "$LLAMA^{tree}"; then
    ok "llama.cpp $LLAMA"
  else
    warn "llama.cpp $LLAMA"
  fi
}

models() {
  local hf=https://huggingface.co m=$T/models
  fetch "$hf/ggml-org/embeddinggemma-2-GGUF/resolve/bfcd298762cc34d0357ece5ebdd31791a3a374d8/embeddinggemma-2-BF16.gguf" \
    68bae29d62fb8c7d23e98d21fd4662753ddd636e6b62b8a70dfe059a9844f216 \
    "$m/embeddinggemma-2-GGUF-bfcd2987/embeddinggemma-2-BF16.gguf" &
  fetch "$hf/google/embeddinggemma-2/resolve/914f7f89142e33e77833254d9c9b90c3cef7303b/tokenizer.json" \
    4d777ef5bdc1aa36227abdfb77c3e49e7b9c892d16e1b6bda41c393504828be4 \
    "$m/embeddinggemma-2-914f7f89/tokenizer.json" &
  fetch "$hf/Qwen/Qwen2.5-Coder-0.5B-Instruct-GGUF/resolve/ebb2015119c907b064c512bf053e945850b5875f/qwen2.5-coder-0.5b-instruct-q4_k_m.gguf" \
    1d9614638d18024d0fbb36575a15f1302a3adf044df10345688ec4f6e1c4ff32 \
    "$m/Qwen2.5-Coder-0.5B-Instruct-GGUF-ebb20151/qwen2.5-coder-0.5b-instruct-q4_k_m.gguf" &
  # 013 tokenizer and configuration only (never the weights), as in CI.
  local rev=1a793eb568e6718f15941d08f85432581df534e3 dir=$T/laya-typed-decisions
  while read -r sum file; do
    fetch "$hf/convaiinnovations/laya-typed-decisions/resolve/$rev/$file" "$sum" "$dir/$file" &
  done <<'EOF'
6c8aaa9a542084f2457eab775d4eeb51f92a70c0fd9de28d5edb0ddec3c08d30 tokenizer/tokenizer.json
08d4cf3ac4dca381759441b85b91a6d40e688471dcd33d15d6649eb0a9a854d1 tokenizer/tokenizer_config.json
ebf0cd524d92342a6be5e48e9fca3d7c2babfb5a56ccd79d2171ef5d8c7f7be8 rl_agent_config.json
5268d24ad3b77c8151de5dcb0762ba4391619aad9ab0bda33e36fb083cfeae6d encoder/config.json
EOF
  wait
}

omp_host() {
  local dir=$T/omp
  if [ -x "$dir/node_modules/@oven/bun-linux-x64/bin/bun" ] &&
      [ -f "$dir/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js" ]; then
    ok "OMP 18.6.0 / Bun 1.3.14"
    return
  fi
  mkdir -p "$dir"
  if timeout 240 npm install --prefix "$dir" --no-audit --no-fund --loglevel=error \
      @oh-my-pi/pi-coding-agent@18.6.0 @oven/bun-linux-x64@1.3.14 >/dev/null &&
      [ -f "$dir/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js" ]; then
    ok "OMP 18.6.0 / Bun 1.3.14"
  else
    warn "OMP 18.6.0 / Bun 1.3.14"
  fi
}

# Steps run in parallel inside main; its plain `wait` sees only those children.
# The log is a pipeline, never `exec > >(tee)`: Bash 5.2's `wait` would also wait
# for that logger, which never exits.
main() {
  echo "context-foundry setup start $(date -u +%FT%TZ)"
  system_packages &
  rust_toolchain &
  cargo_prefetch &
  llama_cpp &
  models &
  omp_host &
  wait
  {
    echo "finished $(date -u +%FT%TZ) after $(($(date +%s) - START)) s"
    echo "warnings: $(wc -l < "$T/setup-warnings.txt")"
    sed 's/^/  /' "$T/setup-warnings.txt"
  } | tee "$T/setup-receipt.txt"
}
main 2>&1 | tee -a "$LOG"
exit 0
