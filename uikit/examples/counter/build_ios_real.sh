#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY="$SCRIPT_DIR/target/aarch64-apple-ios/release/counter"
BUNDLE_DIR="$SCRIPT_DIR/target/Counter.app"
BUNDLE_ID="com.example.counter"

# 1. Build
echo "==> Building for aarch64-apple-ios..."
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml" --target aarch64-apple-ios

# 2. Create app bundle
echo "==> Creating app bundle..."
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"
cp "$BINARY" "$BUNDLE_DIR/Counter"

cat > "$BUNDLE_DIR/Info.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>Counter</string>
    <key>CFBundleIdentifier</key>
    <string>com.example.counter</string>
    <key>CFBundleName</key>
    <string>Counter</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSRequiresIPhoneOS</key>
    <true/>
    <key>UIRequiredDeviceCapabilities</key>
    <array><string>arm64</string></array>
    <key>UISupportedInterfaceOrientations</key>
    <array><string>UIInterfaceOrientationPortrait</string></array>
    <key>UIStatusBarHidden</key>
    <false/>
</dict>
</plist>
PLIST

