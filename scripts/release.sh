#!/usr/bin/env bash
# Publishes a new version: bumps it in Cargo.toml, commits, tags and pushes.
# The tag starts .github/workflows/release.yml, which builds the app, makes the GitHub
# release and updates Homebrew and the AUR.
# Usage: scripts/release.sh 0.2.0
set -euo pipefail
cd "$(dirname "$0")/.."

v="${1:?usage: scripts/release.sh <version>, for example 0.2.0}"
[[ "$v" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "The version must look like 0.2.0." >&2; exit 1; }
[ "$(git branch --show-current)" = main ] || { echo "Release from the main branch." >&2; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "Commit or stash your changes first." >&2; exit 1; }
if git rev-parse -q --verify "refs/tags/v$v" >/dev/null; then
  echo "Tag v$v exists already." >&2
  exit 1
fi

git pull --ff-only
sed -i.bak "/^\[workspace.package\]/,/^\[/ s/^version = \".*\"/version = \"$v\"/" Cargo.toml
rm Cargo.toml.bak
# Writes the new version of our own crates into Cargo.lock, without network access.
cargo update --workspace --offline

git commit -am "chore: release v$v"
git tag "v$v"
git push origin main "v$v"
echo "Pushed v$v. Follow the release with: gh run watch"
