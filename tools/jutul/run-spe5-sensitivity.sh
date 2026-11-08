#!/usr/bin/env bash
# #116: bounded JutulDarcy well-volume / timestep sensitivity of the SPE5 reference. Runs the
# reference generator (tools/jutul/spe5_reference.jl) once per configuration below, writes each
# reference and its resolved configuration into OUTDIR, validates every run, and only then prints
# the comparison table (tools/jutul/spe5_compare.py --references) with the default configuration
# as the base.
#
#   bash tools/jutul/run-spe5-sensitivity.sh OUTDIR [JOBS]
#   bash tools/jutul/run-spe5-sensitivity.sh --validate-only OUTDIR
#
# The sweep fails (exit 1, no table) unless all seven runs exit cleanly and each output is a
# complete 240-report, 7305 d run, written after this sweep started (OUTDIR/provenance.txt), whose
# recorded configuration matches the requested volume and timestep cap. OUTDIR must not already
# hold any of the sweep's outputs, so a stale file cannot enter the table. --validate-only checks
# and tabulates an existing OUTDIR without running anything.
#
# JOBS (default 4) runs execute in parallel; each is single-threaded Julia. The slowest runs (the
# 1e-4 m³ nodes) take well over an hour. Needs julia +1.12 with tools/jutul instantiated.
set -euo pipefail
cd "$(dirname "$0")/../.."
validate_only=false
if [ "${1:-}" = "--validate-only" ]; then
    validate_only=true
    shift
fi
out="${1:?usage: $0 [--validate-only] OUTDIR [JOBS]}"
jobs="${2:-4}"

# label  SPE5_WELL_VOLUME_M3  SPE5_MAX_DT_DAYS
configs="default default default
vol-1e-4 1e-4 default
vol-1e-3 1e-3 default
vol-1e-2 1e-2 default
default-dt5 default 5
default-dt0.5 default 0.5
vol-1e-4-dt0.5 1e-4 0.5"

if ! "$validate_only"; then
    mkdir -p "$out"
    while read -r label _ _; do
        for f in "$out/$label.json" "$out/$label.json.config.json" "$out/$label.log" \
            "$out/$label.status"; do
            if [ -e "$f" ]; then
                echo "refusing to run: $f already exists; use an empty OUTDIR" >&2
                exit 2
            fi
        done
    done <<< "$configs"
    {
        echo "commit $(git rev-parse HEAD)"
        echo "worktree_dirty $([ -z "$(git status --porcelain)" ] && echo false || echo true)"
    } > "$out/provenance.txt"

    run_one() {
        local label=$1 volume=$2 maxdt=$3 dir=$4
        local start=$SECONDS status=0
        SPE5_WELL_VOLUME_M3="$volume" SPE5_MAX_DT_DAYS="$maxdt" \
            julia +1.12 --project=tools/jutul tools/jutul/spe5_reference.jl "$dir/$label.json" \
            > "$dir/$label.log" 2>&1 || status=$?
        echo "$status" > "$dir/$label.status"
        echo "$label: exit $status after $((SECONDS - start)) s"
    }
    export -f run_one
    # xargs appends each line's three fields after the fixed argument, so $0 is OUTDIR.
    echo "$configs" | xargs -P "$jobs" -L 1 bash -c 'run_one "$1" "$2" "$3" "$0"' "$out" || true
fi

failed=0
args=()
while read -r label volume maxdt; do
    if [ -e "$out/$label.status" ] && [ "$(cat "$out/$label.status")" != 0 ]; then
        echo "$label: generator exited $(cat "$out/$label.status")" >&2
        failed=1
        continue
    fi
    if ! python3 -I tools/jutul/spe5_compare.py --validate-run "$out/$label.json" "$volume" \
        "$maxdt" "$out/provenance.txt"; then
        failed=1
        continue
    fi
    args+=("$label=$out/$label.json")
done <<< "$configs"
if [ "$failed" != 0 ]; then
    echo "sensitivity sweep FAILED: not every run is complete and valid; no table written" >&2
    exit 1
fi
python3 -I tools/jutul/spe5_compare.py --references "${args[@]}" | tee "$out/comparison.md"
