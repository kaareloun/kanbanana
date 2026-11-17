#!/usr/bin/env bash
# Builds linear-board-v2.figcraft from build_v2.py and exports one PNG per screen to png-v2/.
set -euo pipefail
cd "$(dirname "$0")/.."
F="${FIGCRAFT:-figcraft}"
tmp=$(mktemp -d)
python3 src/build_v2.py > "$tmp/script.json"
$F cli run "$tmp/script.json" --out linear-board-v2.figcraft | grep -v '"ok":true' | cut -c1-600 || true
rm -rf png-v2 && $F cli export --out png-v2 linear-board-v2.figcraft
rm -rf "$tmp"
