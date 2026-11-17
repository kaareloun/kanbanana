#!/usr/bin/env bash
# Runs cargo through one shared lock, so only one build runs at a time.
# The machine has little free RAM. Use this for every cargo command.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
exec flock /tmp/kanbanana-cargo.lock cargo "$@"
