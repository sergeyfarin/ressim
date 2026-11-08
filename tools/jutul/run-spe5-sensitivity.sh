#!/usr/bin/env bash
# #116: bounded JutulDarcy well-volume / timestep sensitivity of the SPE5 reference. Runs the
# reference generator (tools/jutul/spe5_reference.jl) once per configuration below, writes each
# reference and its resolved configuration into OUTDIR, and prints the comparison table
# (tools/jutul/spe5_compare.py --references) with the default configuration as the base.
#
#   bash tools/jutul/run-spe5-sensitivity.sh OUTDIR [JOBS]
#
# JOBS (default 4) runs execute in parallel; each is single-threaded Julia. The slowest runs (the
# 1e-4 m³ nodes) take about 80 minutes alone. Needs julia +1.12 with tools/jutul instantiated.
set -euo pipefail
cd "$(dirname "$0")/../.."
out="${1:?usage: $0 OUTDIR [JOBS]}"
jobs="${2:-4}"
mkdir -p "$out"

# label  SPE5_WELL_VOLUME_M3  SPE5_MAX_DT_DAYS
configs="default default default
vol-1e-4 1e-4 default
vol-1e-3 1e-3 default
vol-1e-2 1e-2 default
default-dt5 default 5
default-dt0.5 default 0.5
vol-1e-4-dt0.5 1e-4 0.5"

{
    echo "commit $(git rev-parse HEAD)"
    echo "worktree_dirty $([ -z "$(git status --porcelain)" ] && echo false || echo true)"
} > "$out/provenance.txt"

run_one() {
    local label=$1 volume=$2 maxdt=$3 dir=$4
    local start=$SECONDS
    if SPE5_WELL_VOLUME_M3="$volume" SPE5_MAX_DT_DAYS="$maxdt" \
        julia +1.12 --project=tools/jutul tools/jutul/spe5_reference.jl "$dir/$label.json" \
        > "$dir/$label.log" 2>&1; then
        echo "$label: done in $((SECONDS - start)) s"
    else
        echo "$label: FAILED after $((SECONDS - start)) s (see $dir/$label.log)"
    fi
}
export -f run_one
echo "$configs" | xargs -P "$jobs" -L 1 bash -c 'run_one "$0" "$1" "$2" '"'$out'"

args=()
while read -r label _ _; do
    [ -s "$out/$label.json" ] && args+=("$label=$out/$label.json")
done <<< "$configs"
python3 -I tools/jutul/spe5_compare.py --references "${args[@]}" | tee "$out/comparison.md"
