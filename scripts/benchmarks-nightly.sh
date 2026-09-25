#!/usr/bin/env bash
set -uo pipefail

# Nightly benchmark signals (#54 phase 3). Runs the full benchmark tier on origin/master in its own
# worktree, then rewrites one pinned GitHub issue, "Benchmark signals (nightly)", with the result:
# drift against the committed records, same-model gaps, near-band criteria and stale sections.
# It comments on the issue only when the state changes (check result or attention count), so a
# notification means something moved.
#
#   bash scripts/benchmarks-nightly.sh              # what cron runs
#   NIGHTLY_DRY_RUN=1 bash scripts/benchmarks-nightly.sh   # print the issue body, touch nothing
#
# It needs what the full tier needs (Rust, wasm-pack, Node, OPM Flow, Julia 1.12, flowexp_comp)
# plus an authenticated `gh`. It never commits: re-recording stays a deliberate `update` in a commit.
# Installed as a user crontab entry; see docs/BENCHMARKS.md ("Nightly signals").

repo="${RESSIM_REPO:-$(cd "$(dirname "$0")/.." && pwd)}"
home_dir="${RESSIM_NIGHTLY_HOME:-$HOME/.cache/ressim-nightly}"
worktree="$home_dir/worktree"
logs="$home_dir/logs"
title="Benchmark signals (nightly)"
mkdir -p "$logs"

# cron's PATH is minimal: the toolchains live in the user's home.
export PATH="$HOME/.cargo/bin:$HOME/.juliaup/bin:$HOME/.local/share/fnm/aliases/default/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
export CARGO_TARGET_DIR="$home_dir/target"
export FLOWEXP_COMP="${FLOWEXP_COMP:-$(cd "$repo/.." && pwd)/ressim-opm-build/opm-simulators/build/bin/flowexp_comp}"

exec 9> "$home_dir/lock"
flock -n 9 || { echo "another nightly run holds $home_dir/lock"; exit 0; }

stamp="$(date -u +%Y-%m-%dT%H%MZ)"
log="$logs/$stamp.log"
ls -1t "$logs"/*.log 2>/dev/null | tail -n +15 | xargs -r rm -f

git -C "$repo" fetch --quiet origin master || { echo "fetch failed" >&2; exit 1; }
if [ ! -d "$worktree/.git" ] && [ ! -f "$worktree/.git" ]; then
    git -C "$repo" worktree add --detach "$worktree" origin/master >/dev/null
fi
git -C "$worktree" checkout --quiet --detach origin/master
git -C "$worktree" reset --quiet --hard origin/master
commit="$(git -C "$worktree" rev-parse --short HEAD)"

cd "$worktree"
started=$(date +%s)
BENCH_OUT="$home_dir/run" bash "$worktree/scripts/benchmarks.sh" check > "$log" 2>&1
check_rc=$?
minutes=$(( ($(date +%s) - started) / 60 ))
signals="$(python3 "$worktree/tools/benchmarks/benchmarks.py" signals 2>&1)"
attention="$(printf '%s\n' "$signals" | sed -n 's/^benchmark signals: \([0-9]*\) needing attention$/\1/p')"
check_lines="$(grep -E '^  (DRIFT|note):|^benchmark (check|page)|FAILED|panicked|^jutul .*FAILED' "$log" | grep -v 'not checked, needs' | head -60)"
if [ -z "$check_lines" ]; then
    # benchmarks.sh stopped before its check: show where.
    check_lines="benchmarks.sh exited $check_rc before comparing; last lines of the log:
$(tail -15 "$log")"
fi
check_state=$([ "$check_rc" -eq 0 ] && echo OK || echo FAILED)
state="check=$check_state attention=${attention:-?}"

body="$(cat <<EOF
<!-- nightly-state: $state -->
Rewritten every night by \`scripts/benchmarks-nightly.sh\` (#54): the full benchmark tier on \`origin/master\`, compared with the committed records in \`docs/benchmarks/benchmarks.json\`. It comments below only when the state changes.

**Last run:** $stamp on \`$commit\`, $minutes min. **Check:** $check_state. **Needing attention:** ${attention:-unknown}.

### Check against the committed records
\`\`\`
${check_lines:-no output}
\`\`\`

### Signals
Unexplained gaps, near-band criteria and stale sections need attention (\`GAP\`, \`NEAR\`, \`STALE\`); lower-case lines are tracked or explained in \`docs/benchmarks/explained.json\`.
\`\`\`
$signals
\`\`\`

To act: a DRIFT or a new GAP needs an issue or a fix; a change that is right gets recorded with \`bash scripts/benchmarks.sh update\` on a committed tree. Log on the host: \`$log\`.
EOF
)"

if [ -n "${NIGHTLY_DRY_RUN:-}" ]; then
    printf '%s\n' "$body"
    exit 0
fi

number="$(gh issue list --repo "$(git -C "$repo" remote get-url origin)" --state open --search "in:title \"$title\"" \
    --json number,title --jq ".[] | select(.title == \"$title\") | .number" | head -1)"
remote="$(git -C "$repo" remote get-url origin)"
if [ -z "$number" ]; then
    url="$(gh issue create --repo "$remote" --title "$title" --label "area:validation" --body "$body")"
    number="${url##*/}"
    gh issue pin "$number" --repo "$remote" >/dev/null 2>&1 || true
    echo "created and pinned #$number"
else
    previous="$(gh issue view "$number" --repo "$remote" --json body --jq .body | sed -n 's/^<!-- nightly-state: \(.*\) -->$/\1/p')"
    gh issue edit "$number" --repo "$remote" --body "$body" >/dev/null
    if [ "$previous" != "$state" ]; then
        gh issue comment "$number" --repo "$remote" \
            --body "State changed on \`$commit\`: \`${previous:-none}\` → \`$state\`. See the updated body." >/dev/null
        echo "updated #$number; state changed: ${previous:-none} -> $state"
    else
        echo "updated #$number; state unchanged: $state"
    fi
fi
