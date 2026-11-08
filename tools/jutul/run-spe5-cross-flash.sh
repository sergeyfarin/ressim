#!/usr/bin/env bash
# Regenerate or verify opm/compositional/spe5/cross_flash_205_216.json (#116): dump both
# simulators' monthly cell states for reports 205..216, flash every (p, z) point with both flashes,
# and merge. The merge refuses to write a fixture unless all 192 points have valid results from
# both flashes (tools/jutul/spe5_cross_flash_fixture.py).
#
#   bash tools/jutul/run-spe5-cross-flash.sh [--check] [WORKDIR]
#
# --check validates the committed fixture, regenerates it into WORKDIR and compares the two with
# provenance excluded. On a run that starts and ends on the same clean commit it then records that
# commit, the Jutul model configuration and the source hashes in the fixture's
# `provenance.verification`; on a dirty tree it verifies but records nothing.
#
# Runtime: about 8 minutes for the ResSim replay and about 30 minutes for the JutulDarcy state dump
# (they run in parallel). Needs julia +1.12 with tools/jutul instantiated, and cargo.
set -euo pipefail
cd "$(dirname "$0")/../.."
check=false
work=""
for arg in "$@"; do
    case "$arg" in
        --check) check=true ;;
        -*) echo "usage: $0 [--check] [WORKDIR]" >&2; exit 2 ;;
        *) work="$arg" ;;
    esac
done
work="${work:-$(mktemp -d)}"
mkdir -p "$work"
fixture=opm/compositional/spe5/cross_flash_205_216.json
manifest_dir=src/lib/ressim
start_commit=$(git rev-parse HEAD)
start_clean=$([ -z "$(git status --porcelain)" ] && echo true || echo false)

if "$check"; then
    python3 -I tools/jutul/spe5_cross_flash_fixture.py check "$fixture"
fi

echo "workdir: $work"
julia +1.12 --project=tools/jutul tools/jutul/spe5_state_dump.jl "$work/jutul_states.json" 205 216 \
    > "$work/jutul_states.log" 2>&1 &
jutul_pid=$!
(cd "$manifest_dir" && SPE5_STATE_OUT="$work/ressim_states.json" \
    cargo test --release --lib comp_spe5_dump_report_states -- --ignored) \
    > "$work/ressim_states.log" 2>&1
wait "$jutul_pid"

python3 -I tools/jutul/spe5_cross_flash_fixture.py points \
    "$work/ressim_states.json" "$work/jutul_states.json" "$work/points.json"
(cd "$manifest_dir" && SPE5_CROSS_FLASH_POINTS="$work/points.json" \
    SPE5_CROSS_FLASH_OUT="$work/ressim_flash.json" \
    cargo test --release --lib comp_spe5_cross_flash_points -- --ignored)
julia +1.12 --project=tools/jutul tools/jutul/spe5_cross_flash.jl \
    "$work/points.json" "$work/jutul_flash.json" > "$work/jutul_flash.log" 2>&1

provenance=$(python3 -I - "$work/jutul_states.json" "$start_commit" "$start_clean" <<'PY'
import json, sys
states, commit, clean = sys.argv[1:4]
print(json.dumps({"generator": "bash tools/jutul/run-spe5-cross-flash.sh", "commit": commit,
                  "worktree_dirty": clean != "true", "reports": [205, 216],
                  "cells": [32, 33, 39, 40],
                  "jutul_configuration": json.load(open(states)).get("configuration")}))
PY
)
target="$fixture"
"$check" && target="$work/cross_flash_205_216.json"
python3 -I tools/jutul/spe5_cross_flash_fixture.py merge "$work/points.json" \
    "$work/ressim_flash.json" "$work/jutul_flash.json" "$target" --provenance "$provenance"

if ! "$check"; then
    echo "wrote $fixture"
    exit 0
fi
python3 -I tools/jutul/spe5_cross_flash_fixture.py compare "$fixture" "$target"
python3 -I - "$fixture" "$target" "$start_commit" "$start_clean" <<'PY'
import datetime, hashlib, json, pathlib, subprocess, sys
fixture_path, fresh_path, start_commit, start_clean = sys.argv[1:5]
commit = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
clean = not subprocess.check_output(["git", "status", "--porcelain"], text=True).strip()
if not (start_clean == "true" and clean and commit == start_commit):
    print("verified on a dirty or moving tree; fixture provenance not updated")
    sys.exit(0)
fixture = json.load(open(fixture_path))
fresh = json.load(open(fresh_path))
sources = ["tools/jutul/run-spe5-cross-flash.sh", "tools/jutul/spe5_cross_flash_fixture.py",
           "tools/jutul/spe5_cross_flash.jl", "tools/jutul/spe5_state_dump.jl",
           "tools/jutul/spe5_reference.jl", "tools/jutul/Manifest.toml",
           "opm/compositional/spe5/case.json",
           "src/lib/ressim/src/compositional/spe5_cross_flash_tests.rs"]
fixture["provenance"]["verification"] = {
    "command": "bash tools/jutul/run-spe5-cross-flash.sh --check",
    "commit": commit,
    "worktree_clean": True,
    "date": datetime.date.today().isoformat(),
    "result": "fixture regenerated and matched, provenance excluded",
    "jutul_configuration": fresh["provenance"]["jutul_configuration"],
    "sha256": {p: hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest() for p in sources},
}
json.dump(fixture, open(fixture_path, "w"), indent=1)
print(f"verification recorded in {fixture_path} for {commit}")
PY
