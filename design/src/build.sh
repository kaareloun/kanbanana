#!/usr/bin/env bash
# Builds linear-board.figcraft from build.py and exports one PNG per screen.
set -euo pipefail
cd "$(dirname "$0")/.."
F="${FIGCRAFT:-figcraft}"
tmp=$(mktemp -d)
python3 src/build.py > "$tmp/script.json"
$F cli run "$tmp/script.json" --out linear-board.figcraft | grep -v '"ok":true' | cut -c1-800 || true
rm -rf png && $F cli export --out png linear-board.figcraft
rm -rf "$tmp"
