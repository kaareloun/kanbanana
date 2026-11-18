#!/usr/bin/env bash
# Builds kanbanana for one target and packs the release archive and its checksum.
# Used by .github/workflows/release.yml. Usage: VERSION=0.2.0 scripts/package-release.sh <target>
set -euo pipefail
target="${1:?usage: scripts/package-release.sh <target>}"
: "${VERSION:?set VERSION, for example 0.2.0}"

cargo build --release --locked -p kanbanana --target "$target"
name="kanbanana-v${VERSION}-${target}"
mkdir -p "$name"
cp "target/$target/release/kanbanana" LICENSE README.md \
  assets/kanbanana.desktop assets/brand/icon.png assets/brand/icon.svg "$name/"
cp -r assets/brand/icons "$name/icons"
tar czf "$name.tar.gz" "$name"
shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256"
