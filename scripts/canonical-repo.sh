#!/usr/bin/env bash
# Print a GitHub repository of this checkout as OWNER/NAME.
#
#   bash scripts/canonical-repo.sh                # canonical: upstream when that remote exists, else origin
#   bash scripts/canonical-repo.sh --remote       # the remote name that rule picks
#   bash scripts/canonical-repo.sh REMOTE         # OWNER/NAME that REMOTE fetches from
#   bash scripts/canonical-repo.sh --push REMOTE  # OWNER/NAME that `git push REMOTE` writes to
#                                                 # (honours remote.REMOTE.pushurl and pushInsteadOf)
#
# Every `gh` call in the agent skills names its repository with this, as a one-line prefix:
#
#   repo=$(bash scripts/canonical-repo.sh) && gh issue list -R "$repo" ...
#
# because `gh`'s default repository in a fork clone is the fork, and an empty `-R ""` silently
# falls back to that default. Exits non-zero, printing nothing on stdout, when the remote is
# missing or is not a GitHub repository, so a `&&` chain stops before any `gh` call. Error
# messages name the remote, never its URL: a remote URL can carry a credential.
#
# Accepted URL forms (an optional `user[:token]@` in an HTTPS URL is ignored):
#   https://github.com/OWNER/NAME[.git][/]
#   git@github.com:OWNER/NAME[.git]
#   ssh://git@github.com[:PORT]/OWNER/NAME[.git]
#   ssh://git@ssh.github.com[:PORT]/OWNER/NAME[.git]   (GitHub's SSH over the HTTPS port)
set -euo pipefail

usage() { echo "usage: $0 [--remote | --push REMOTE | REMOTE]" >&2; exit 2; }

# parse URL: print OWNER/NAME, or return 1 without printing anything.
parse() {
    local url=$1 path
    if [[ $url =~ ^https://([^@/]+@)?github\.com/(.+)$ ]]; then
        path=${BASH_REMATCH[2]}
    elif [[ $url =~ ^git@github\.com:(.+)$ ]]; then
        path=${BASH_REMATCH[1]}
    elif [[ $url =~ ^ssh://git@(ssh\.)?github\.com(:[0-9]+)?/(.+)$ ]]; then
        path=${BASH_REMATCH[3]}
    else
        return 1
    fi
    path=${path%/}
    path=${path%.git}
    [[ $path =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || return 1
    printf '%s\n' "$path"
}

mode=fetch
case $# in
    0)  remote= ;;
    1)  case $1 in
            --remote) mode=name; remote= ;;
            -*) usage ;;
            *) remote=$1 ;;
        esac ;;
    2)  [ "$1" = "--push" ] || usage
        case $2 in -*) usage ;; esac
        mode=push; remote=$2 ;;
    *)  usage ;;
esac

if [ -z "$remote" ]; then
    if git remote get-url upstream >/dev/null 2>&1; then remote=upstream; else remote=origin; fi
fi

if [ "$mode" = name ]; then
    git remote get-url "$remote" >/dev/null
    printf '%s\n' "$remote"
    exit 0
fi

if [ "$mode" = push ]; then
    url=$(git remote get-url --push "$remote")
    what="push URL"
else
    url=$(git remote get-url "$remote")
    what="URL"
fi

if ! parse "$url"; then
    echo "canonical-repo: the $what of remote '$remote' is not a GitHub repository URL" \
         "(https://github.com/OWNER/NAME, git@github.com:OWNER/NAME or" \
         "ssh://git@[ssh.]github.com[:PORT]/OWNER/NAME)" >&2
    exit 1
fi
