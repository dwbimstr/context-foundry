#!/bin/sh
# Development App Sandbox bundle for the owned generator. All model inputs use
# inherited read-only descriptors; no model/home/repository read grants are needed.
# This is not Developer ID signing, notarization or installed-package acceptance.
set -eu
if [ "$#" -ne 2 ]; then
    echo "usage: $0 BINARY OUTPUT.app" >&2
    exit 64
fi
case $(uname -s) in Darwin) ;; *) echo "requires macOS codesign" >&2; exit 69 ;; esac
binary=$1
bundle=$2
[ -x "$binary" ] || { echo "worker binary is not executable" >&2; exit 66; }
[ ! -e "$bundle" ] || { echo "output already exists" >&2; exit 73; }
case "$bundle" in *.app) ;; *) echo "output must end in .app" >&2; exit 64 ;; esac
entitlements="${bundle%.app}.entitlements.plist"
[ ! -e "$entitlements" ] || { echo "entitlements output already exists" >&2; exit 73; }
mkdir -p "$bundle/Contents/MacOS"
cp "$binary" "$bundle/Contents/MacOS/foundry-generate"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>org.context-foundry.generate.dev</string>
<key>CFBundleExecutable</key><string>foundry-generate</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
</dict></plist>
PLIST
cat > "$entitlements" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>com.apple.security.app-sandbox</key><true/>
</dict></plist>
PLIST
codesign --force --sign - --entitlements "$entitlements" "$bundle"
codesign --verify --strict "$bundle"
shasum -a 256 "$bundle/Contents/MacOS/foundry-generate"
