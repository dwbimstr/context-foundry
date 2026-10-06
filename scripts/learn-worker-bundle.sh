#!/bin/sh
# Build the 013 development learning-worker bundle: an ad-hoc-signed App
# Sandbox .app around foundry-learn whose grants come from the LEARNING
# PROFILE, never from loose options: read-only for exactly the profile's
# checkpoint_dir and libtorch_dir, read-write for exactly the profile's
# worker.scratch_root. No network, user-selected-file, inherit or
# library-validation grants. Mirrors scripts/embed-worker-bundle.sh.
#
# Scratch root contract: App Sandbox grants are static per signed bundle, so
# the supervisor uses EXACTLY `worker.scratch_root` from the profile,
# creating one 0700 run directory per run and removing it after the worker
# is reaped. This script grants that path; a missing root is created 0700
# here (its parent must exist), an existing one is left alone and the
# supervisor refuses it at launch unless it is a real directory owned by the
# current user and not group- or world-writable.
#
# LibTorch: the binary must load LibTorch from libtorch_dir without
# DYLD_LIBRARY_PATH (SIP strips it). build.rs records `$LIBTORCH/lib` as the
# binary's rpath; this script requires that rpath to equal the profile's
# libtorch_dir and adds it with install_name_tool (before signing) when it is
# missing.
#
# Prints the executable's SHA-256 AFTER signing; the profile's
# worker.executable_sha256 must be set to that value. The entitlements are
# written next to the bundle as `<APP minus .app>.entitlements.plist`.
#
# usage: learn-worker-bundle.sh --binary PATH --out APP --profile PROFILE.json \
#         [--identifier ID]
set -eu

usage() {
    echo "usage: $0 --binary PATH --out APP --profile PROFILE.json [--identifier ID]" >&2
    exit 64
}

canon_dir() {
    (cd "$1" && pwd -P)
}

profile_value() {
    plutil -extract "$1" raw -o - -- "$PROFILE" 2>/dev/null || {
        echo "profile $PROFILE has no usable $1" >&2
        exit 66
    }
}

BINARY=""
OUT=""
PROFILE=""
IDENTIFIER="org.context-foundry.learn.dev"

while [ $# -gt 0 ]; do
    case "$1" in
        --binary) BINARY=${2:?}; shift 2 ;;
        --out) OUT=${2:?}; shift 2 ;;
        --profile) PROFILE=${2:?}; shift 2 ;;
        --identifier) IDENTIFIER=${2:?}; shift 2 ;;
        *) usage ;;
    esac
done

for required in "$BINARY" "$OUT" "$PROFILE"; do
    [ -n "$required" ] || usage
done
[ -x "$BINARY" ] || { echo "binary is not executable: $BINARY" >&2; exit 66; }
[ -f "$PROFILE" ] || { echo "profile is not a file: $PROFILE" >&2; exit 66; }

CHECKPOINT_DIR=$(profile_value checkpoint_dir)
LIBTORCH_DIR=$(profile_value libtorch_dir)
SCRATCH_ROOT=$(profile_value worker.scratch_root)
for absolute in "$CHECKPOINT_DIR" "$LIBTORCH_DIR" "$SCRATCH_ROOT"; do
    case "$absolute" in
        /*) ;;
        *) echo "profile paths must be absolute: '$absolute'" >&2; exit 66 ;;
    esac
done

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

CHECKPOINT_DIR=$(canon_dir "$CHECKPOINT_DIR")
LIBTORCH_DIR=$(canon_dir "$LIBTORCH_DIR")
SCRATCH_ROOT=$(canon_maybe "$SCRATCH_ROOT")
[ -f "$CHECKPOINT_DIR/model.safetensors" ] || {
    echo "checkpoint_dir has no model.safetensors: $CHECKPOINT_DIR" >&2
    exit 66
}
[ -f "$LIBTORCH_DIR/libtorch.dylib" ] || {
    echo "libtorch_dir has no libtorch.dylib: $LIBTORCH_DIR" >&2
    exit 66
}

# The read-write scratch grant must not overlap either read-only grant
# (equal, ancestor or descendant), before anything is created or signed.
overlap() {
    [ "$1" = "$2" ] && return 0
    case "$1" in "$2"/*) return 0 ;; esac
    case "$2" in "$1"/*) return 0 ;; esac
    return 1
}
for read_only in "$CHECKPOINT_DIR" "$LIBTORCH_DIR"; do
    if overlap "$SCRATCH_ROOT" "$read_only"; then
        echo "worker.scratch_root ($SCRATCH_ROOT) overlaps $read_only; \
the write grant must be disjoint from every read-only grant" >&2
        exit 66
    fi
done

if [ ! -e "$SCRATCH_ROOT" ]; then
    (umask 077 && mkdir "$SCRATCH_ROOT")
fi

ENTITLEMENTS="${OUT%.app}.entitlements.plist"
EXE="$OUT/Contents/MacOS/foundry-learn"

rm -rf "$OUT"
mkdir -p "$OUT/Contents/MacOS"
cp "$BINARY" "$EXE"
chmod +x "$EXE"

# The rpath must name exactly the granted LibTorch directory.
if ! otool -l "$EXE" | grep -A2 LC_RPATH | grep -q "path $LIBTORCH_DIR "; then
    install_name_tool -add_rpath "$LIBTORCH_DIR" "$EXE"
fi

cat > "$OUT/Contents/Info.plist" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>$IDENTIFIER</string>
  <key>CFBundleName</key><string>FoundryLearn</string>
  <key>CFBundleExecutable</key><string>foundry-learn</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>CFBundleShortVersionString</key><string>1</string>
</dict>
</plist>
PL

cat > "$ENTITLEMENTS" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.security.app-sandbox</key>
  <true/>
  <key>com.apple.security.temporary-exception.files.absolute-path.read-only</key>
  <array>
    <string>$CHECKPOINT_DIR/</string>
    <string>$LIBTORCH_DIR/</string>
  </array>
  <key>com.apple.security.temporary-exception.files.absolute-path.read-write</key>
  <array>
    <string>$SCRATCH_ROOT/</string>
  </array>
</dict>
</plist>
PL

codesign --force -s - --entitlements "$ENTITLEMENTS" "$OUT"
codesign --verify --strict "$OUT"

SHA256=$(shasum -a 256 "$EXE" | cut -d' ' -f1)
echo "bundle: $OUT"
echo "identifier: $IDENTIFIER"
echo "entitlements: $ENTITLEMENTS"
echo "read-only grants: $CHECKPOINT_DIR/ $LIBTORCH_DIR/"
echo "read-write grant: $SCRATCH_ROOT/"
echo "executable-sha256: $SHA256"
