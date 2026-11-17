#!/usr/bin/env bash
# Builds linear-board-v3.figcraft from build_v3.py and exports one PNG per screen to png-v3/.
set -euo pipefail
cd "$(dirname "$0")/.."
F="${FIGCRAFT:-figcraft}"
tmp=$(mktemp -d)
python3 src/build_v3.py > "$tmp/script.json"
$F cli run "$tmp/script.json" --out linear-board-v3.figcraft | grep -v '"ok":true' | cut -c1-600 || true
rm -rf png-v3 && $F cli export --out png-v3 linear-board-v3.figcraft
rm -rf "$tmp"
