#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY="$SCRIPT_DIR/target/aarch64-apple-ios-sim/debug/greeter"
BUNDLE_DIR="$SCRIPT_DIR/target/Greeter.app"
BUNDLE_ID="com.example.greeter"

# 1. Build
echo "==> Building for aarch64-apple-ios-sim..."
cargo build --manifest-path "$SCRIPT_DIR/Cargo.toml" --target aarch64-apple-ios-sim

# 2. Create app bundle
echo "==> Creating app bundle..."
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"
cp "$BINARY" "$BUNDLE_DIR/Greeter"

cat > "$BUNDLE_DIR/Info.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>Greeter</string>
    <key>CFBundleIdentifier</key>
    <string>com.example.greeter</string>
    <key>CFBundleName</key>
    <string>Greeter</string>
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
    <key>MinimumOSVersion</key>
    <string>15.0</string>
    <!-- Without UILaunchScreen, iOS runs the app in 320x480
         compatibility scaling and the UI gets letterboxed into a
         centered card. Empty dict = "I support modern device sizes,
         no custom launch UI." -->
    <key>UILaunchScreen</key>
    <dict/>
    <!-- Declare scene support so iOS routes window creation through
         UIWindowSceneDelegate. We don't list any UISceneConfigurations;
         AppDelegate.application:configurationForConnectingSceneSession:
         provides one programmatically (so the runtime-mangled SceneDelegate
         class name doesn't have to be baked into Info.plist). -->
    <key>UIApplicationSceneManifest</key>
    <dict>
        <key>UIApplicationSupportsMultipleScenes</key>
        <false/>
    </dict>
</dict>
</plist>
PLIST

# 3. Find or create a booted simulator
echo "==> Finding booted simulator..."
DEVICE_ID=$(xcrun simctl list devices booted | grep -E 'iPhone.*Booted' | head -1 | sed -n 's/.*(\([A-F0-9-]*\)).*/\1/p' || true)

if [ -z "$DEVICE_ID" ]; then
    # Try to boot an existing iPhone simulator
    EXISTING=$(xcrun simctl list devices | grep -E 'iPhone.*Shutdown' | head -1 | sed -n 's/.*(\([A-F0-9-]*\)).*/\1/p' || true)
    if [ -z "$EXISTING" ]; then
        echo "==> Creating new iPhone simulator..."
        EXISTING=$(xcrun simctl create "tmp-greeter-$$" "iPhone 16" 2>/dev/null || xcrun simctl create "tmp-greeter-$$" com.apple.CoreSimulator.SimDeviceType.iPhone-16)
    fi
    echo "==> Booting simulator $EXISTING..."
    xcrun simctl boot "$EXISTING" || true
    # Wait for boot to complete
    echo "==> Waiting for boot..."
    for i in $(seq 1 30); do
        if xcrun simctl list devices booted | grep -q "$EXISTING"; then
            break
        fi
        sleep 2
    done
    DEVICE_ID="$EXISTING"
fi

echo "==> Using device: $DEVICE_ID"

# Terminate any prior instance so we don't end up debugging a stale build.
xcrun simctl terminate "$DEVICE_ID" "$BUNDLE_ID" 2>/dev/null || true

# 4. Install
echo "==> Installing app..."
xcrun simctl install "$DEVICE_ID" "$BUNDLE_DIR" 2>&1 || {
    echo "Install failed. Try: xcrun simctl uninstall $DEVICE_ID $BUNDLE_ID && xcrun simctl install $DEVICE_ID $BUNDLE_DIR"
    # Try uninstalling first then re-installing
    xcrun simctl uninstall "$DEVICE_ID" "$BUNDLE_ID" 2>/dev/null || true
    xcrun simctl install "$DEVICE_ID" "$BUNDLE_DIR"
}

# 5. Open the simulator app so you can see it
echo "==> Opening Simulator..."
open -a Simulator

# 6. Launch
echo "==> Launching app..."
xcrun simctl launch --console "$DEVICE_ID" "$BUNDLE_ID" || xcrun simctl launch "$DEVICE_ID" "$BUNDLE_ID"

echo "==> App launched. If it crashed, check logs with:"
echo "    xcrun simctl spawn $DEVICE_ID log stream --predicate 'process contains \"Greeter\"'"
