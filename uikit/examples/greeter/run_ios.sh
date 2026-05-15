#!/bin/bash
# Thin shim around the shared script. See uikit/tools/run_ios.sh.
exec "$(dirname "$0")/../../tools/run_ios.sh" "$(dirname "$0")" "$@"
