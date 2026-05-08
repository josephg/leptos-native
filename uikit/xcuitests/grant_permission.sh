#!/usr/bin/env bash
# Open System Settings to the Accessibility privacy pane so the
# user can grant permission to the test runner.
#
# After running this:
#   1. Find your terminal / IDE in the list (the app launching
#      `swift test`).
#   2. Toggle it on.
#   3. Re-run `./run_tests.sh`.
#
# Alternatively you can grant to the `xctest` binary directly at
# `/Applications/Xcode.app/Contents/Developer/usr/bin/xctest`.
open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
