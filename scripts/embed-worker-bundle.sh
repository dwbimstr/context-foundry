#!/bin/sh
# Build the 009 development embedding-worker bundle: an ad-hoc-signed App
# Sandbox .app around foundry-embed (statically linked llama.cpp; no Python)
# whose grants come from the SEMANTIC PROFILE, never from loose options:
# read-only for exactly the profile's model_dir, and read-write for exactly
# the profile's worker.scratch_root. No network, user-selected-file, inherit
# or library-validation grants.
#
# Scratch root contract: App Sandbox grants are static per signed bundle, so
# the supervisor uses EXACTLY `worker.scratch_root` from the profile (an
# absolute path), creating one 0700 subdirectory per run and removing it
# after the worker is reaped. This script reads that path from the profile
# and grants it; there is no other way to name a scratch root, so the signed
# grant and the supervisor cannot disagree. A missing root is created 0700
# here (its parent must exist); an existing one is not touched, and the
# supervisor refuses it at launch unless it is a real directory owned by the
# current user and not group- or world-writable.
#
# Extra read grants (--extra-read DIR, repeatable) exist for a directory the
# model directory's files resolve into outside model_dir (for example the
# blob store a symlinked snapshot points at). Nothing is granted unless
# named.
#
# Prints the executable's SHA-256 AFTER signing (codesign embeds the
# signature in the executable, so the signed file is hashed); the profile's
# worker.executable_sha256 must be set to that value. The entitlements are
# written next to the bundle as `<APP minus .app>.entitlements.plist` so they
# stay auditable without sitting inside the sealed bundle.
#
# usage: embed-worker-bundle.sh --binary PATH --out APP --profile PROFILE.json \
#         [--extra-read DIR]... [--identifier ID]
set -eu

usage() {
    echo "usage: $0 --binary PATH --out APP --profile PROFILE.json [--extra-read DIR]... [--identifier ID]" >&2
    exit 64
}

# Sandbox grants name physical paths. Canonicalize with the shell alone; no
# interpreter is assumed on the build host.
canon_dir() {
    (cd "$1" && pwd -P)
}

canon_file() {
    dir=$(canon_dir "$(dirname "$1")") || return 1
    printf '%s/%s\n' "$dir" "$(basename "$1")"
}

# One value from the profile. `plutil` ships with macOS and reads JSON.
profile_value() {
    plutil -extract "$1" raw -o - -- "$PROFILE" 2>/dev/null || {
        echo "profile $PROFILE has no usable $1" >&2
        exit 66
    }
}

BINARY=""
OUT=""
PROFILE=""
EXTRA_READ=""
IDENTIFIER="org.context-foundry.embed.dev"

while [ $# -gt 0 ]; do
    case "$1" in
        --binary) BINARY=${2:?}; shift 2 ;;
        --out) OUT=${2:?}; shift 2 ;;
        --profile) PROFILE=${2:?}; shift 2 ;;
        --extra-read)
            extra=$(canon_dir "${2:?}")
            EXTRA_READ="$EXTRA_READ$extra
"
            shift 2
            ;;
        --identifier) IDENTIFIER=${2:?}; shift 2 ;;
        *) usage ;;
    esac
done

for required in "$BINARY" "$OUT" "$PROFILE"; do
    [ -n "$required" ] || usage
done
[ -x "$BINARY" ] || { echo "binary is not executable: $BINARY" >&2; exit 66; }
[ -f "$PROFILE" ] || { echo "profile is not a file: $PROFILE" >&2; exit 66; }

# Read every value in its own assignment: a failed `$(...)` ends the script
# (`set -e`), while a nested substitution would swallow the failure and hand
# `cd` an empty path.
MODEL_DIR=$(profile_value model_dir)
SCRATCH_ROOT=$(profile_value worker.scratch_root)
for absolute in "$MODEL_DIR" "$SCRATCH_ROOT"; do
    case "$absolute" in
        /*) ;;
        *) echo "profile paths must be absolute: '$absolute'" >&2; exit 66 ;;
    esac
done
# Resolve a path that may not exist yet: canonicalize its nearest existing
# ancestor and append the missing trailing components, so an alias chain on
# the ancestors cannot hide an overlap.
canon_maybe() {
    p=$1
    if [ -e "$p" ]; then
        canon_dir "$p"
        return
    fi
    suffix=""
    while [ ! -e "$p" ]; do
        suffix="/$(basename "$p")$suffix"
        parent=$(dirname "$p")
        if [ "$parent" = "$p" ]; then
            printf '%s%s\n' "$p" "$suffix"
            return
        fi
        p=$parent
    done
    printf '%s%s\n' "$(canon_dir "$p")" "$suffix"
}

MODEL_DIR=$(canon_dir "$MODEL_DIR")
SCRATCH_ROOT=$(canon_maybe "$SCRATCH_ROOT")

# M10, before anything is created or signed: the read-write scratch grant
# must not overlap ANY read-only grant -- the profile's model directory and
# EVERY canonical --extra-read directory (equal, ancestor or descendant).
overlap() {
    a=$1
    b=$2
    [ "$a" = "$b" ] && return 0
    case "$a" in
        "$b"/*) return 0 ;;
    esac
    case "$b" in
        "$a"/*) return 0 ;;
    esac
    return 1
}
READ_ONLY="$MODEL_DIR
$EXTRA_READ"
while IFS= read -r read_only; do
    [ -n "$read_only" ] || continue
    if overlap "$SCRATCH_ROOT" "$read_only"; then
        echo "worker.scratch_root ($SCRATCH_ROOT) overlaps $read_only; \
the write grant must be disjoint from every read-only grant" >&2
        exit 66
    fi
done <<READ_ONLY_PATHS
$READ_ONLY
READ_ONLY_PATHS

# Only now, with the grants proven disjoint, is the scratch root created.
if [ ! -e "$SCRATCH_ROOT" ]; then
    (umask 077 && mkdir "$SCRATCH_ROOT")
fi

ENTITLEMENTS="${OUT%.app}.entitlements.plist"

rm -rf "$OUT"
mkdir -p "$OUT/Contents/MacOS"
cp "$BINARY" "$OUT/Contents/MacOS/foundry-embed"
chmod +x "$OUT/Contents/MacOS/foundry-embed"

cat > "$OUT/Contents/Info.plist" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>$IDENTIFIER</string>
  <key>CFBundleName</key><string>FoundryEmbed</string>
  <key>CFBundleExecutable</key><string>foundry-embed</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>CFBundleShortVersionString</key><string>1</string>
</dict>
</plist>
PL

# Directory grants end in `/` so the sandbox matches the subtree.
read_only_grants() {
    printf '    <string>%s/</string>\n' "$MODEL_DIR"
    printf '%s' "$EXTRA_READ" | while IFS= read -r extra_dir; do
        if [ -n "$extra_dir" ]; then
            printf '    <string>%s/</string>\n' "$extra_dir"
        fi
    done
}

{
    cat <<'PL'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.security.app-sandbox</key>
  <true/>
  <key>com.apple.security.temporary-exception.files.absolute-path.read-only</key>
  <array>
PL
    read_only_grants
    cat <<PL
  </array>
  <key>com.apple.security.temporary-exception.files.absolute-path.read-write</key>
  <array>
    <string>$SCRATCH_ROOT/</string>
  </array>
</dict>
</plist>
PL
} > "$ENTITLEMENTS"

codesign --force -s - --entitlements "$ENTITLEMENTS" "$OUT"
codesign --verify --strict "$OUT"

SHA256=$(shasum -a 256 "$OUT/Contents/MacOS/foundry-embed" | cut -d' ' -f1)
echo "bundle: $OUT"
echo "identifier: $IDENTIFIER"
echo "entitlements: $ENTITLEMENTS"
echo "read-only grants: $MODEL_DIR/"
if [ -n "$EXTRA_READ" ]; then
    echo "extra read grants: $(printf '%s' "$EXTRA_READ" | tr '\n' ' ')"
fi
echo "read-write grant: $SCRATCH_ROOT/"
echo "executable-sha256: $SHA256"
