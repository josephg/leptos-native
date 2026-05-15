#!/bin/bash
# Build + bundle + install + launch a single iOS example crate on the
# iOS Simulator. Replaces the per-example run_ios.sh copies; called by
# them via a 2-line shim.
#
# Usage:
#   uikit/tools/run_ios.sh <example_dir> [-t SECONDS]
#
#   example_dir   Path to the iOS example crate (the one with
#                 Cargo.toml). Can be absolute or relative to cwd.
#
#   -t SECONDS    Auto-terminate the app after the given delay.
#                 Useful for CI / agent verification. Without -t,
#                 the script streams console output and blocks.
#
# Derives:
#   - Cargo package name from the dir name + "_ios" suffix
#     (matches the existing convention, e.g. counter/ → counter_ios)
#   - App display name as PascalCase of the dir name
#   - Bundle ID as com.example.<dir_name>

set -euo pipefail

usage() {
    echo "Usage: $0 <example_dir> [-t SECONDS]" >&2
    echo "" >&2
    echo "Build + run an iOS example on the iOS Simulator." >&2
    echo "" >&2
    echo "Args:" >&2
    echo "  example_dir   Path to the iOS example crate." >&2
    echo "  -t SECONDS    Auto-terminate after N seconds (for CI)." >&2
    exit 1
}

EXAMPLE_DIR=""
TIMEOUT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        -t|--timeout) TIMEOUT="$2"; shift 2 ;;
        -h|--help) usage ;;
        -*) echo "Unknown flag: $1" >&2; usage ;;
        *)
            if [ -z "$EXAMPLE_DIR" ]; then
                EXAMPLE_DIR="$1"
                shift
            else
                echo "Extra positional arg: $1" >&2
                usage
            fi
            ;;
    esac
done

if [ -z "$EXAMPLE_DIR" ]; then
    echo "Missing example_dir argument." >&2
    usage
fi

if [ ! -f "$EXAMPLE_DIR/Cargo.toml" ]; then
    echo "Error: $EXAMPLE_DIR/Cargo.toml not found." >&2
    exit 1
fi

# Resolve to absolute path
EXAMPLE_DIR="$(cd "$EXAMPLE_DIR" && pwd)"
DIR_NAME="$(basename "$EXAMPLE_DIR")"
PKG_NAME="${DIR_NAME}_ios"

# PascalCase the directory name for the app display name.
# (counter → Counter; keyboard_avoidance → KeyboardAvoidance)
APP_NAME="$(echo "$DIR_NAME" | awk -F_ '{
    for (i=1; i<=NF; i++) {
        $i = toupper(substr($i,1,1)) substr($i,2)
    }
    OFS=""
    print
}')"

BUNDLE_ID="com.example.${DIR_NAME}"

# Locate the workspace root so we can share target/ across examples.
# This script lives at <workspace>/uikit/tools/run_ios.sh, so the
# workspace is two levels up.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

BINARY="$WORKSPACE_ROOT/target/aarch64-apple-ios-sim/debug/$PKG_NAME"
BUNDLE_DIR="$EXAMPLE_DIR/bundle/${APP_NAME}.app"

export CARGO_TARGET_DIR="$WORKSPACE_ROOT/target"

# 1. Build
echo "==> Building $PKG_NAME for aarch64-apple-ios-sim..."
cargo build --manifest-path "$EXAMPLE_DIR/Cargo.toml" --target aarch64-apple-ios-sim

# 2. Create app bundle
echo "==> Creating app bundle: $BUNDLE_DIR"
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"
cp "$BINARY" "$BUNDLE_DIR/${APP_NAME}"

cat > "$BUNDLE_DIR/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
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
    <!-- Empty UILaunchScreen = "modern device sizes, no custom
         launch UI". Without this, iOS runs in 320×480 compatibility
         scaling and the UI is letterboxed. -->
    <key>UILaunchScreen</key>
    <dict/>
    <!-- Scene support. The framework provides UISceneConfiguration
         programmatically via AppDelegate, so no per-scene entries
         are needed here. -->
    <key>UIApplicationSceneManifest</key>
    <dict>
        <key>UIApplicationSupportsMultipleScenes</key>
        <false/>
    </dict>
</dict>
</plist>
PLIST

# 3. Find or create a booted simulator
echo "==> Finding booted iPhone simulator..."
DEVICE_ID=$(xcrun simctl list devices booted | grep -E 'iPhone.*Booted' | head -1 | sed -n 's/.*(\([A-F0-9-]*\)).*/\1/p' || true)

if [ -z "$DEVICE_ID" ]; then
    EXISTING=$(xcrun simctl list devices | grep -E 'iPhone.*Shutdown' | head -1 | sed -n 's/.*(\([A-F0-9-]*\)).*/\1/p' || true)
    if [ -z "$EXISTING" ]; then
        echo "==> Creating new iPhone simulator..."
        EXISTING=$(xcrun simctl create "tmp-${DIR_NAME}-$$" "iPhone 16" 2>/dev/null \
            || xcrun simctl create "tmp-${DIR_NAME}-$$" com.apple.CoreSimulator.SimDeviceType.iPhone-16)
    fi
    echo "==> Booting simulator $EXISTING..."
    xcrun simctl boot "$EXISTING" || true
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

# Terminate any prior instance.
xcrun simctl terminate "$DEVICE_ID" "$BUNDLE_ID" 2>/dev/null || true

# 4. Install
echo "==> Installing $APP_NAME.app..."
xcrun simctl install "$DEVICE_ID" "$BUNDLE_DIR" 2>&1 || {
    echo "Install failed; retrying after uninstall..." >&2
    xcrun simctl uninstall "$DEVICE_ID" "$BUNDLE_ID" 2>/dev/null || true
    xcrun simctl install "$DEVICE_ID" "$BUNDLE_DIR"
}

# 5. Open the simulator UI
echo "==> Opening Simulator..."
open -a Simulator

# 6. Launch
echo "==> Launching $APP_NAME..."
if [ -n "$TIMEOUT" ]; then
    xcrun simctl launch "$DEVICE_ID" "$BUNDLE_ID"
    echo "==> Auto-terminating in ${TIMEOUT}s..."
    sleep "$TIMEOUT"
    xcrun simctl terminate "$DEVICE_ID" "$BUNDLE_ID" 2>/dev/null || true
    echo "==> Terminated."
    exit 0
else
    xcrun simctl launch --console "$DEVICE_ID" "$BUNDLE_ID" \
        || xcrun simctl launch "$DEVICE_ID" "$BUNDLE_ID"
fi

echo "==> App launched. Stream logs with:"
echo "    xcrun simctl spawn $DEVICE_ID log stream --predicate 'process contains \"${APP_NAME}\"'"
