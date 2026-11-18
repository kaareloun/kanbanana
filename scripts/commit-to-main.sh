#!/usr/bin/env bash
# Commits the given files to main from a release job and pushes. The Linux and macOS jobs
# both push to main, so it rebases and retries when the other job pushed first.
# Usage: scripts/commit-to-main.sh "<message>" <file>...
set -euo pipefail
message="${1:?usage: scripts/commit-to-main.sh <message> <file>...}"
shift
git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add "$@"
if git diff --cached --quiet; then
  echo "Nothing changed."
  exit 0
fi
git commit -m "$message"
for attempt in 1 2 3 4 5; do
  if git push origin HEAD:main; then
    exit 0
  fi
  echo "Push failed (attempt $attempt). Rebasing on the latest main."
  sleep $((attempt * 3))
  git pull --rebase origin main
done
echo "Could not push to main." >&2
exit 1
