#!/usr/bin/env bash
# Build a cargo example and wrap it as a `.app` bundle so
# XCUIAutomation can launch it.
#
# Usage:
#   ./bundle_app.sh <example_name> <bundle_id>
#
# Example:
#   ./bundle_app.sh login_form_macos com.leptos.test.LoginForm
#
# Output:
#   target/xcuitests/<example_name>.app/
#       Contents/
#           Info.plist
#           MacOS/<example_name>          (the cargo binary)
#
# Stdout: absolute path to the .app bundle (handy for piping into a
# test runner).
set -euo pipefail

if [[ $# -lt 2 ]]; then
    echo "usage: $0 <example_name> <bundle_id>" >&2
    exit 2
fi

EXAMPLE="$1"
BUNDLE_ID="$2"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
MANIFEST="$REPO_ROOT/examples/$EXAMPLE/Cargo.toml"
TARGET_DIR="$REPO_ROOT/target/xcuitests"
APP_DIR="$TARGET_DIR/$EXAMPLE.app"

if [[ ! -f "$MANIFEST" ]]; then
    echo "error: cargo manifest not found at $MANIFEST" >&2
    exit 1
fi

# 1. Build (release for closer-to-prod behavior; could parameterise).
cargo build --release --manifest-path "$MANIFEST" >&2

# 2. Locate the produced binary. The example crates aren't part of
# the workspace, so each has its own per-crate target dir.
BIN="$REPO_ROOT/examples/$EXAMPLE/target/release/$EXAMPLE"
if [[ ! -x "$BIN" ]]; then
    echo "error: built binary not found / not executable at $BIN" >&2
    exit 1
fi

# 3. Lay out the .app skeleton.
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
cp "$BIN" "$APP_DIR/Contents/MacOS/$EXAMPLE"

# 4. Minimal Info.plist. CFBundleExecutable MUST match the binary
# name in MacOS/.
cat > "$APP_DIR/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>$EXAMPLE</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleName</key>
    <string>$EXAMPLE</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
</dict>
</plist>
EOF

# 5. Quick sanity-check.
plutil -lint "$APP_DIR/Contents/Info.plist" >&2

echo "$APP_DIR"
