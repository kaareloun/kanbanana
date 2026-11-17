#!/usr/bin/env bash
# Takes a screenshot of a mock screen on Hyprland without taking keyboard focus.
# Usage: scripts/shot.sh <screen> <out.png>. Build first: scripts/cargo.sh build -p kanbanana
set -euo pipefail
bin="$(cd "$(dirname "$0")/.." && pwd)/target/debug/kanbanana"
screen=$1
out=$(realpath -m "$2")
mkdir -p "$(dirname "$out")"
rm -f "$out"
log="/tmp/kanbanana-shot-$screen.log"
# Hyprland does not draw windows on hidden workspaces, so the window must be visible.
hyprctl dispatch "hl.dsp.exec_cmd(\"sh -c '$bin --mock --screen $screen --screenshot $out > $log 2>&1'\", { float = true, size = \"1440 900\", no_initial_focus = true, no_focus = true })" >/dev/null
for _ in $(seq 1 60); do
  [ -s "$out" ] && break
  sleep 0.5
done
sleep 1
pkill -f "kanbanana --mock --screen $screen --screenshot" || true
[ -s "$out" ] && echo "ok $out" || { echo "no screenshot, see $log"; exit 1; }
