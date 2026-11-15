#!/usr/bin/env bash
# Print the canonical GitHub repository of this checkout as OWNER/NAME.
#
#   bash scripts/canonical-repo.sh            # upstream when that remote exists, else origin
#   bash scripts/canonical-repo.sh --remote   # the remote name that rule picks
#   bash scripts/canonical-repo.sh REMOTE     # OWNER/NAME of one named remote (e.g. origin)
#
# Every `gh` call in the agent skills names its repository with this, as a one-line prefix:
#
#   repo=$(bash scripts/canonical-repo.sh) && gh issue list -R "$repo" ...
#
# because `gh`'s default repository in a fork clone is the fork, and an empty `-R ""` silently
# falls back to that default. Exits non-zero, printing nothing on stdout, when the remote is
# missing or is not a GitHub repository, so a `&&` chain stops before any `gh` call.
set -euo pipefail

usage() { echo "usage: $0 [--remote | REMOTE]" >&2; exit 2; }
[ $# -le 1 ] || usage

if [ $# -eq 1 ] && [ "$1" != "--remote" ]; then
    case $1 in -*) usage ;; esac
    remote=$1
elif git remote get-url upstream >/dev/null 2>&1; then
    remote=upstream
else
    remote=origin
fi

if [ "${1-}" = "--remote" ]; then
    git remote get-url "$remote" >/dev/null
    printf '%s\n' "$remote"
    exit 0
fi

url=$(git remote get-url "$remote")
repo=$(printf '%s\n' "$url" \
    | sed -E 's#^(https://github\.com/|git@github\.com:|ssh://git@github\.com/)##; s#\.git$##; s#/$##')
if [ "$repo" = "$url" ] || ! [[ $repo =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]]; then
    echo "canonical-repo: remote '$remote' is not a GitHub repository: $url" >&2
    exit 1
fi
printf '%s\n' "$repo"
