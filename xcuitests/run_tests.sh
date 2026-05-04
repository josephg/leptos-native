#!/usr/bin/env bash
# Build all leptos-mac example apps as `.app` bundles, then run the
# XCUIAutomation-equivalent tests against them.
#
# Each test target reads its target app's path from a dedicated
# env var:
#   LEPTOS_MAC_LOGIN_FORM_PATH
#   LEPTOS_MAC_SETTINGS_PATH
#   LEPTOS_MAC_COUNTERS_PATH
#
# Tests run on the actual desktop session — windows briefly appear
# and get clicked / typed into. Don't run interactively.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

bundle() {
    local example="$1"
    local bundle_id="$2"
    local var_name="$3"
    local path
    path="$(./bundle_app.sh "$example" "$bundle_id")"
    echo "  $var_name=$path"
    printf -v "$var_name" '%s' "$path"
    export "$var_name"
}

echo "Bundling example apps..."
bundle login_form_macos com.leptos.test.LoginForm \
    LEPTOS_MAC_LOGIN_FORM_PATH
bundle settings_macos com.leptos.test.Settings \
    LEPTOS_MAC_SETTINGS_PATH
bundle counters_macos com.leptos.test.Counters \
    LEPTOS_MAC_COUNTERS_PATH

# Backward-compat: older tests / docs reference LEPTOS_MAC_APP_PATH
# pointing at the login form. Keep it set to that.
export LEPTOS_MAC_APP_PATH="$LEPTOS_MAC_LOGIN_FORM_PATH"

swift test --package-path "$SCRIPT_DIR" "$@"
