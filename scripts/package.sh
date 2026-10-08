#!/bin/sh
# Build the installable macOS arm64 package (deployment § Lifecycle and
# installation, docs/release.md step 5):
#
#   context-foundry-<version>-macos-arm64.tar.gz
#     PACKAGE.json                 manifest: version, git commit, target, the
#                                  SHA-256 of every other file, the versions
#                                  the binary reports, dependency identities
#     bin/foundry                  release core
#     README.md, LICENSE
#     THIRD-PARTY/                 license/notice files of every normal
#                                  dependency of the packaged binaries, an
#                                  INDEX.json, and supplied notices (with
#                                  canonical texts in _canonical/) for
#                                  packages that ship none
#     scripts/install.sh           the installer (also kept in the install)
#   --with-semantic adds libexec/foundry-embed, scripts/embed-worker-bundle.sh
#   --with-learning adds libexec/foundry-learn, scripts/learn-worker-bundle.sh
#
# Model weights, datasets, profiles and credentials are never packaged: the
# contents are exactly the list above. The semantic profile named here is
# read only to check it pins the llama.cpp commit the worker is built from.
# That commit and the llama.cpp license text (which covers its vendored
# ggml) come from the worker itself (`foundry-embed --notices DIR`; build.rs
# embeds both), so packaging a built worker needs no llama.cpp checkout.
#
# Binaries are built here (`cargo build --locked --offline --release`; the
# semantic worker needs LLAMA_CPP_DIR, a llama.cpp checkout at the pinned
# commit with its static build in LLAMA_BUILD_DIR, default
# $LLAMA_CPP_DIR/build-static; the learning worker LIBTORCH), or taken
# already built from --bin-dir DIR (DIR/foundry, DIR/foundry-embed,
# DIR/foundry-learn). Tests only: with --bin-dir, CF_TEST_VERSION_LABEL
# relabels the package as `<crate version>-test.<suffix>`; it is refused
# for a built package and in any other form. A path argument that is
# empty, ends in `/` or holds a byte outside printable ASCII (0x20-0x7E) is
# refused (exit 64).
#
# usage: package.sh --out DIR [--with-semantic --semantic-profile FILE]
#                   [--with-learning] [--bin-dir DIR]
set -eu
LC_ALL=C
export LC_ALL

usage() {
    echo "usage: $0 --out DIR [--with-semantic --semantic-profile FILE] [--with-learning] [--bin-dir DIR]" >&2
    exit 64
}

die() {
    code=$1
    shift
    echo "package.sh: $*" >&2
    exit "$code"
}

# A JSON string literal; manifest values never carry control characters.
json_str() {
    case "$1" in
        *[[:cntrl:]]*) die 65 "refusing a control character in a manifest value" ;;
        *[\"\\]*) printf '"%s"' "$(printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g')" ;;
        *) printf '"%s"' "$1" ;;
    esac
}

# Refuse a value that would need JSON escaping (no fork: called per file).
plain() {
    case "$1" in
        *[[:cntrl:]\"\\]*) die 65 "refusing '$1' in THIRD-PARTY/INDEX.json" ;;
    esac
}

# A path argument that is empty, ends in `/` or holds a byte outside
# printable ASCII is refused (exit 64), never reinterpreted.
check_value() {
    case "$2" in
        "" | */) die 64 "refusing $1 '$2': empty or ending in /" ;;
    esac
    rest=$(printf '%s' "$2" | tr -d ' -~'; printf x)
    [ "$rest" = x ] || die 64 "refusing $1: it holds a byte outside printable ASCII (0x20-0x7E)"
}

# One value from a JSON file. `plutil` ships with macOS and reads JSON.
json_value() {
    plutil -extract "$2" raw -o - -- "$1" 2>/dev/null || die 66 "$1 has no usable $2"
}

# The version Cargo.lock pins for one package.
lock_version() {
    awk -v name="$1" '
        $0 == "name = \"" name "\"" { found = 1; next }
        found && /^version = / { gsub(/"/, "", $3); print $3; exit }
    ' "$REPO/Cargo.lock"
}

REPO=$(cd "$(dirname "$0")/.." && pwd -P)
CARGO=${CARGO:-cargo}
TARGET=aarch64-apple-darwin
OUT=""
BIN_DIR=""
SEMANTIC_PROFILE=""
WITH_SEMANTIC=0
WITH_LEARNING=0

while [ $# -gt 0 ]; do
    case "$1" in
        --out) check_value "$1" "${2-}"; OUT=$2; shift 2 ;;
        --bin-dir) check_value "$1" "${2-}"; BIN_DIR=$2; shift 2 ;;
        --semantic-profile) check_value "$1" "${2-}"; SEMANTIC_PROFILE=$2; shift 2 ;;
        --with-semantic) WITH_SEMANTIC=1; shift ;;
        --with-learning) WITH_LEARNING=1; shift ;;
        *) usage ;;
    esac
done
[ -n "$OUT" ] || usage
if [ "$WITH_SEMANTIC" = 1 ]; then
    [ -n "$SEMANTIC_PROFILE" ] || die 64 "--with-semantic needs --semantic-profile FILE (its llama.cpp pin is checked)"
    [ -f "$SEMANTIC_PROFILE" ] || die 66 "semantic profile is not a file: $SEMANTIC_PROFILE"
elif [ -n "$SEMANTIC_PROFILE" ]; then
    die 64 "--semantic-profile is only meaningful with --with-semantic"
fi
[ "$(uname -s) $(uname -m)" = "Darwin arm64" ] || die 69 "the package target is macOS arm64; this host is $(uname -s) $(uname -m)"

VERSION=$(awk '
    /^\[package\]/ { p = 1; next }
    /^\[/ { p = 0 }
    p && /^version = / { gsub(/"/, "", $3); print $3; exit }
' "$REPO/Cargo.toml")
[ -n "$VERSION" ] || die 65 "no package version in Cargo.toml"
LABEL=$VERSION
if [ -n "${CF_TEST_VERSION_LABEL:-}" ]; then
    [ -n "$BIN_DIR" ] || die 64 "CF_TEST_VERSION_LABEL is honoured only for tests, with --bin-dir"
    case "$CF_TEST_VERSION_LABEL" in
        "$VERSION"-test.*) ;;
        *) die 64 "CF_TEST_VERSION_LABEL must be $VERSION-test.<suffix>" ;;
    esac
    LABEL=$CF_TEST_VERSION_LABEL
fi
case "$LABEL" in
    *[!A-Za-z0-9.+-]*) die 65 "version label '$LABEL' has characters outside [A-Za-z0-9.+-]" ;;
esac

COMMIT=$(git -C "$REPO" rev-parse HEAD 2>/dev/null) || die 69 "$REPO is not a git checkout; a package records its commit"
DIRTY=false
[ -z "$(git -C "$REPO" status --porcelain)" ] || DIRTY=true

NAME=context-foundry-$LABEL-macos-arm64
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd -P)
[ ! -e "$OUT/$NAME.tar.gz" ] || die 73 "$OUT/$NAME.tar.gz already exists"
STAGE=$(mktemp -d "$OUT/.package.XXXXXX")
trap 'rm -rf "$STAGE"' EXIT
ROOT=$STAGE/$NAME
mkdir -p "$ROOT/bin" "$ROOT/scripts"

# --- binaries -------------------------------------------------------------
PREBUILT=false
TREE_FEATURES=""
if [ -n "$BIN_DIR" ]; then
    PREBUILT=true
    BIN_DIR=$(cd "$BIN_DIR" && pwd -P)
    FOUNDRY_BIN=$BIN_DIR/foundry
    EMBED_BIN=$BIN_DIR/foundry-embed
    LEARN_BIN=$BIN_DIR/foundry-learn
else
    RELEASE=${CARGO_TARGET_DIR:-$REPO/target}/release
    FOUNDRY_BIN=$RELEASE/foundry
    EMBED_BIN=$RELEASE/foundry-embed
    LEARN_BIN=$RELEASE/foundry-learn
    "$CARGO" build --manifest-path "$REPO/Cargo.toml" --locked --offline --release --bin foundry
fi
[ -x "$FOUNDRY_BIN" ] || die 66 "no foundry executable at $FOUNDRY_BIN"
cp "$FOUNDRY_BIN" "$ROOT/bin/foundry"
if [ "$WITH_SEMANTIC" = 1 ]; then
    TREE_FEATURES="$TREE_FEATURES,embed-worker"
    if [ "$PREBUILT" = false ]; then
        [ -n "${LLAMA_CPP_DIR:-}" ] || die 64 "building foundry-embed needs LLAMA_CPP_DIR (the pinned llama.cpp checkout)"
        "$CARGO" build --manifest-path "$REPO/Cargo.toml" --locked --offline --release \
            --features embed-worker --bin foundry-embed
    fi
    [ -x "$EMBED_BIN" ] || die 66 "no foundry-embed executable at $EMBED_BIN"
    mkdir -p "$ROOT/libexec"
    cp "$EMBED_BIN" "$ROOT/libexec/foundry-embed"
    cp "$REPO/scripts/embed-worker-bundle.sh" "$ROOT/scripts/"
    # The worker prints the llama.cpp commit it is built from and writes the
    # license text it links; the profile must pin that same commit.
    LLAMA_CPP=$(json_value "$SEMANTIC_PROFILE" descriptor.llama_cpp)
    plain "$LLAMA_CPP"
    built=$("$ROOT/libexec/foundry-embed" --notices "$STAGE/llama.cpp") ||
        die 65 "$EMBED_BIN --notices failed: not a foundry-embed this package can ship"
    [ "$built" = "$LLAMA_CPP" ] || die 65 "the profile pins llama.cpp $LLAMA_CPP; the worker is built from $built"
fi
if [ "$WITH_LEARNING" = 1 ]; then
    TREE_FEATURES="$TREE_FEATURES,learning-worker"
    if [ "$PREBUILT" = false ]; then
        [ -n "${LIBTORCH:-}" ] || die 64 "building foundry-learn needs LIBTORCH (the LibTorch 2.11.0 tree)"
        "$CARGO" build --manifest-path "$REPO/Cargo.toml" --locked --offline --release \
            --features learning-worker --bin foundry-learn
    fi
    [ -x "$LEARN_BIN" ] || die 66 "no foundry-learn executable at $LEARN_BIN"
    mkdir -p "$ROOT/libexec"
    cp "$LEARN_BIN" "$ROOT/libexec/foundry-learn"
    cp "$REPO/scripts/learn-worker-bundle.sh" "$ROOT/scripts/"
fi
cp "$REPO/scripts/install.sh" "$ROOT/scripts/"
cp "$REPO/README.md" "$REPO/LICENSE" "$ROOT/"
chmod 0755 "$ROOT"/bin/* "$ROOT"/scripts/*
[ ! -d "$ROOT/libexec" ] || chmod 0755 "$ROOT"/libexec/*

# --- THIRD-PARTY ----------------------------------------------------------
# The v0.1.0 recipe: every normal (non-dev, non-build) dependency of the
# packaged binaries, for this target and the packaged features, gets
# THIRD-PARTY/<name>-<version>/ with the license/notice files its published
# crate ships. A crate that ships none gets NOTICE-SUPPLIED.txt naming its
# declared license, authors and repository, with the canonical text in
# THIRD-PARTY/_canonical/ (a multi-license expression elects MIT).
THIRD=$ROOT/THIRD-PARTY
mkdir -p "$THIRD/_canonical"
REGISTRY=${CARGO_HOME:-$HOME/.cargo}/registry/src
TREE_FEATURES=${TREE_FEATURES#,}
set -- --manifest-path "$REPO/Cargo.toml" --locked --offline -e normal --prefix none \
    --target "$TARGET" --format '{p}|{l}|{r}'
[ -z "$TREE_FEATURES" ] || set -- "$@" --features "$TREE_FEATURES"
"$CARGO" tree "$@" > "$STAGE/tree.txt"
sed 's/ (\*)$//' "$STAGE/tree.txt" | LC_ALL=C sort -u > "$STAGE/packages.txt"

sed 's/^Copyright (c) .*/Copyright (c) <year> <copyright holders>/' "$REPO/LICENSE" \
    > "$THIRD/_canonical/MIT.txt"
APACHE=""
INDEX_ROWS=""
PACKAGES=0
while IFS='|' read -r pkg license repository; do
    name=${pkg%% *}
    rest=${pkg#* v}
    version=${rest%% *}
    [ "$name" != context-foundry ] || continue
    src=""
    for candidate in "$REGISTRY"/*/"$name-$version"; do
        [ -d "$candidate" ] && src=$candidate && break
    done
    [ -n "$src" ] || die 66 "no registry source for $name $version under $REGISTRY"
    dest=$THIRD/$name-$version
    mkdir -p "$dest"
    plain "$name"
    plain "$version"
    plain "$license"
    files=""
    set --
    for file in "$src"/*; do
        [ -f "$file" ] || continue
        base=${file##*/}
        case "$base" in
            [Ll][Ii][Cc][Ee][Nn][Ss][Ee]* | [Ll][Ii][Cc][Ee][Nn][Cc][Ee]* | [Cc][Oo][Pp][Yy][Ii][Nn][Gg]* | \
                [Nn][Oo][Tt][Ii][Cc][Ee]* | [Uu][Nn][Ll][Ii][Cc][Ee][Nn][Ss][Ee]* | [Cc][Oo][Pp][Yy][Rr][Ii][Gg][Hh][Tt]*)
                plain "$base"
                set -- "$@" "$file"
                files="$files${files:+, }\"$base\""
                if [ -z "$APACHE" ] && [ "$base" = LICENSE-APACHE ]; then
                    APACHE=$file
                fi
                ;;
        esac
    done
    [ $# -eq 0 ] || cp "$@" "$dest/"
    if [ -z "$files" ]; then
        case "$license" in
            *MIT*) text=_canonical/MIT.txt elect=MIT ;;
            *Apache-2.0*) text=_canonical/Apache-2.0.txt elect=Apache-2.0 ;;
            *) die 65 "$name $version ships no license file and declares '$license'; supply its notice" ;;
        esac
        authors=$(awk '
            /^\[/ { pkg = ($0 == "[package]"); next }
            pkg && /^authors[ ]*=/ { grab = 1 }
            grab {
                line = $0
                while (match(line, /"[^"]*"/)) {
                    out = out (out == "" ? "" : ", ") substr(line, RSTART + 1, RLENGTH - 2)
                    line = substr(line, RSTART + RLENGTH)
                }
                if ($0 ~ /\]/) grab = 0
            }
            END { print out }
        ' "$src/Cargo.toml")
        cat > "$dest/NOTICE-SUPPLIED.txt" <<NOTICE
$name $version
Declared license: $license
Authors: ${authors:-(none declared)}
Repository: ${repository:-(none declared)}

The published crate package contains no license file. The declared license applies; its canonical text is in THIRD-PARTY/$text (for a multi-license expression this distribution elects $elect), with copyright held by the authors above.
NOTICE
        files='"NOTICE-SUPPLIED.txt"'
    fi
    INDEX_ROWS="$INDEX_ROWS${INDEX_ROWS:+,
} {\"name\": \"$name\", \"version\": \"$version\", \"license\": \"$license\", \"files\": [$files]}"
    PACKAGES=$((PACKAGES + 1))
done < "$STAGE/packages.txt"
[ -n "$APACHE" ] || die 66 "no dependency ships LICENSE-APACHE to supply the canonical Apache-2.0 text"
cp "$APACHE" "$THIRD/_canonical/Apache-2.0.txt"
if [ "$WITH_SEMANTIC" = 1 ]; then
    # llama.cpp and its vendored ggml are statically linked into the worker.
    # At the pinned commit one MIT LICENSE ("The ggml authors") covers both:
    # ggml has no license file of its own there, so the text ships once.
    mkdir "$THIRD/llama.cpp-$LLAMA_CPP"
    cp "$STAGE/llama.cpp/LICENSE" "$THIRD/llama.cpp-$LLAMA_CPP/LICENSE"
    INDEX_ROWS="$INDEX_ROWS${INDEX_ROWS:+,
} {\"name\": \"llama.cpp\", \"version\": \"$LLAMA_CPP\", \"license\": \"MIT\", \"files\": [\"LICENSE\"]}"
    PACKAGES=$((PACKAGES + 1))
fi
printf '[\n%s\n]\n' "$INDEX_ROWS" > "$THIRD/INDEX.json"

# --- manifest -------------------------------------------------------------
# The versions the packaged core reports (inspection writes nothing).
"$ROOT/bin/foundry" bootstrap --root "$STAGE" > "$STAGE/bootstrap.json"
V_core=$(json_value "$STAGE/bootstrap.json" versions.core)
V_store_schema=$(json_value "$STAGE/bootstrap.json" versions.store_schema)
V_embed_protocol=$(json_value "$STAGE/bootstrap.json" versions.embed_protocol)
V_learn_protocol=$(json_value "$STAGE/bootstrap.json" versions.learn_protocol)
V_predict_protocol=$(json_value "$STAGE/bootstrap.json" versions.predict_protocol)

COMPONENTS='"core"'
DEPENDENCIES="  \"cargo_lock_sha256\": \"$(shasum -a 256 "$REPO/Cargo.lock" | cut -d' ' -f1)\",
  \"third_party_packages\": $PACKAGES"
if [ "$WITH_SEMANTIC" = 1 ]; then
    COMPONENTS="$COMPONENTS, \"semantic\""
    DEPENDENCIES="$DEPENDENCIES,
  \"semantic\": {
   \"llama_cpp\": $(json_str "$LLAMA_CPP"),
   \"runtime_profile\": $(json_str "$(json_value "$SEMANTIC_PROFILE" name)"),
   \"note\": \"Not packaged: the GGUF model and its tokenizer. The operator supplies them through the semantic profile given at install, which builds the signed bundle; the worker statically links llama.cpp (Metal) and only system libraries and frameworks, and its hello is checked against the installed profile.\"
  }"
fi
if [ "$WITH_LEARNING" = 1 ]; then
    COMPONENTS="$COMPONENTS, \"learning\""
    torch_sys=$(lock_version torch-sys)
    # torch-sys 0.24.0 checks for LibTorch 2.11.0 at build time.
    [ "$torch_sys" = 0.24.0 ] || die 65 "torch-sys $torch_sys: update the LibTorch requirement recorded here"
    # Whole pathnames: otool prints `path <dir> (offset N)` under LC_RPATH.
    rpath=$(otool -l "$ROOT/libexec/foundry-learn" | awk '
        /cmd LC_RPATH/ { r = 1; next }
        r && $1 == "path" { line = $0; sub(/^[ \t]*path /, "", line); sub(/ \(offset [0-9]+\)$/, "", line); print line; r = 0 }
    ' | while IFS= read -r dir; do
        if [ -e "$dir/libtorch_cpu.dylib" ] || [ -e "$dir/libtorch.dylib" ]; then
            echo "$dir"
            break
        fi
    done)
    build_version=""
    if [ -n "$rpath" ] && [ -f "$rpath/../build-version" ]; then
        build_version=$(cat "$rpath/../build-version")
    fi
    DEPENDENCIES="$DEPENDENCIES,
  \"learning\": {
   \"tch\": $(json_str "$(lock_version tch)"),
   \"torch_sys\": $(json_str "$torch_sys"),
   \"libtorch_required\": \"2.11.0\",
   \"libtorch_build_dir\": $(json_str "$rpath"),
   \"libtorch_build_version\": $(json_str "$build_version"),
   \"note\": \"Not packaged: LibTorch and the checkpoint. The operator supplies LibTorch 2.11.0; learn-worker-bundle.sh makes the learning profile's libtorch_dir the worker's rpath.\"
  }"
fi

# Every packaged path stays in a plain character set, so the manifest and
# the installer's records need no escaping, and no entry is a link.
(cd "$ROOT" && find . ! -type f ! -type d) > "$STAGE/special.txt"
[ ! -s "$STAGE/special.txt" ] || die 65 "the package may hold only regular files: $(head -1 "$STAGE/special.txt")"
(cd "$ROOT" && find . -type f | sed 's|^\./||' | LC_ALL=C sort) > "$STAGE/files.txt"
if grep -v '^[A-Za-z0-9._+/-]*$' "$STAGE/files.txt" > "$STAGE/odd.txt"; then
    die 65 "packaged path outside [A-Za-z0-9._+/-]: $(head -1 "$STAGE/odd.txt")"
fi
FILES=$(cd "$ROOT" && tr '\n' '\0' < "$STAGE/files.txt" | xargs -0 shasum -a 256 |
    awk '{ printf "%s  \"%s\": \"%s\"", (NR > 1 ? ",\n" : ""), $2, $1 }')

cat > "$ROOT/PACKAGE.json" <<MANIFEST
{
 "v": 1,
 "name": "context-foundry",
 "version": $(json_str "$LABEL"),
 "target": "$TARGET",
 "git_commit": $(json_str "$COMMIT"),
 "git_dirty": $DIRTY,
 "prebuilt": $PREBUILT,
 "components": [$COMPONENTS],
 "versions": {
  "core": $(json_str "$V_core"),
  "store_schema": $V_store_schema,
  "embed_protocol": $V_embed_protocol,
  "learn_protocol": $V_learn_protocol,
  "predict_protocol": $V_predict_protocol
 },
 "dependencies": {
$DEPENDENCIES
 },
 "files": {
$FILES
 }
}
MANIFEST

COPYFILE_DISABLE=1 tar -C "$STAGE" --no-mac-metadata -czf "$STAGE/$NAME.tar.gz" "$NAME"
mv "$STAGE/$NAME.tar.gz" "$OUT/$NAME.tar.gz"
echo "package: $OUT/$NAME.tar.gz"
echo "version: $LABEL (core $V_core, store schema $V_store_schema)"
echo "commit: $COMMIT$([ "$DIRTY" = false ] || echo ' (uncommitted changes)')"
echo "components: $(printf '%s' "$COMPONENTS" | tr -d '"')"
echo "third-party packages: $PACKAGES"
echo "sha256: $(shasum -a 256 "$OUT/$NAME.tar.gz" | cut -d' ' -f1)"
