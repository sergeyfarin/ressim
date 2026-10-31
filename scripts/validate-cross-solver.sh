#!/usr/bin/env bash
set -euo pipefail

# Cross-solver gate: every ResSim solver against OPM Flow, and against each other, on the
# small-direct decks (opm/reference-decks/small-direct/README.md).
#
# Runs FIM (sparse LU, dense LU) and IMPES natively in release on the eight decks, runs Flow on
# the same decks (skipped per case when the deck is unchanged since the last run), and checks
# the result against the committed scorecard. A regression is an accuracy metric against Flow,
# or a work metric (substeps, Newton), past its band in compare_small_direct.py's BANDS; a new
# solver warning; or a scorecard run with no output. Improvements are reported so a better
# baseline is recorded on purpose.
#
# Usage:
#   bash scripts/validate-cross-solver.sh              # check against the scorecard
#   bash scripts/validate-cross-solver.sh --update     # rewrite the scorecard (commit first)
#   bash scripts/validate-cross-solver.sh --case bo-1d-10 [--case ...]
#   bash scripts/validate-cross-solver.sh --markdown   # tables for a README, then check
#   bash scripts/validate-cross-solver.sh --refine 0.025 [--case ow-2d-12x12]
#       time-refined referee: regenerate the decks with a finer report step, run all three
#       simulators on them and compare, ungated. Agreement with Flow at the *same* dt is not
#       accuracy: FIM and Flow share an implicit scheme and its time-step error, so a gap between
#       them and IMPES says nothing about which is right until all three are refined. Slow.
#   bash scripts/validate-cross-solver.sh --grid-refine
#       grid-refined referee: run the uniform bo-1d depletion column at 10, 20 and 40 cells
#       through FIM sparse and IMPES, then report successive differences, contraction ratios,
#       observed order and the Richardson extrapolation. Needs no Flow or OPM bindings.
#
# Needs `flow` and its `opm.io` Python package for the normal and time-refined modes; skips with
# a note when Flow is absent, like validate-compositional.sh's reference mode. About 20 s with Flow
# cached, ~40 s cold. `--grid-refine` is the exception: it needs only ResSim and numpy.
#
# Deliberately NOT in validate:* or PR CI: CI has no Flow. Run it after any change to the
# engine's physics, PVT, wells, either timestep controller or the small-system linear route,
# and before quoting a small-direct number anywhere.

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
manifest_path="$repo_root/src/lib/ressim/Cargo.toml"
scorecard="$repo_root/opm/reference-decks/small-direct/scorecard.json"
work="${CROSS_SOLVER_OUT:-${TMPDIR:-/tmp}/ressim-cross-solver}"

mode="--check"
compare_args=(--inventory-check)
cases=()
refine=""
grid_refine=0
while [ $# -gt 0 ]; do
    case "$1" in
        --update) mode="--write-scorecard"; shift ;;
        --markdown) compare_args+=(--markdown); shift ;;
        --case) cases+=("${2:?missing case}"); compare_args+=(--case "$2"); shift 2 ;;
        --refine) refine="${2:?missing report dt in days}"; shift 2 ;;
        --grid-refine) grid_refine=1; shift ;;
        --help) sed -n '3,40p' "$0"; exit 0 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
done

run_ressim() {
    local solver="$1" backend="${2:-}"
    local case_filter=()
    # The driver takes one case or all; loop when several are named.
    if [ ${#cases[@]} -eq 0 ]; then case_filter=(""); else case_filter=("${cases[@]}"); fi
    for c in "${case_filter[@]}"; do
        env OPM_SMALL_OUT="$ressim_dir" OPM_SMALL_SOLVER="$solver" OPM_SMALL_BACKEND="$backend" \
            ${c:+OPM_SMALL_CASE="$c"} \
            cargo test --release --quiet --manifest-path "$manifest_path" --lib \
            opm_small_direct_run_ressim -- --ignored --nocapture --exact \
            tests::opm_small_direct::opm_small_direct_run_ressim 2>&1 \
            | grep -E 'substeps=|panicked|error' || true
    done
}

if [ "$grid_refine" -eq 1 ]; then
    # Richardson self-convergence needs only ResSim. It deliberately runs before the Flow
    # availability check so the referee still works on a machine without OPM installed.
    if [ -n "$refine" ] || [ "$mode" != "--check" ] || [ ${#cases[@]} -gt 0 ] \
        || [ ${#compare_args[@]} -gt 1 ]; then
        echo "--grid-refine is standalone: do not combine it with --case, --update, --markdown or --refine" >&2
        exit 2
    fi
    if ! python3 -c 'import numpy' 2>/dev/null; then
        echo "skip: the grid-refinement referee needs numpy for its field averages." >&2
        exit 0
    fi
    work="$work/grid-refine"
    ressim_dir="$work/ressim"
    rm -rf "$ressim_dir"
    mkdir -p "$ressim_dir"
    cases=(bo-1d-10 bo-1d-20 bo-1d-40)
    echo "== ResSim grid ladder (native release)"
    run_ressim fim sparse
    run_ressim impes
    produced="$(find "$ressim_dir" -name '*.json' | wc -l)"
    if [ "$produced" -eq 0 ]; then
        echo "FAIL: the ResSim grid ladder wrote no output." >&2
        exit 1
    fi
    python3 "$repo_root/tools/opm_flow/grid_refinement_referee.py" \
        --ressim-dir "$ressim_dir" --levels 10 20 40
    exit $?
fi

if ! command -v flow >/dev/null 2>&1 || ! python3 -c 'import opm.io' 2>/dev/null; then
    echo "skip: OPM Flow or its opm.io Python package is not installed; nothing was compared."
    exit 0
fi

deck_args=()
if [ -n "$refine" ]; then
    # Its own tree: refined Flow output must never be mistaken for the scorecard's.
    work="$work/refine-$refine"
    deck_args=(--deck-dir "$work/decks")
    mode=""
    export OPM_SMALL_REPORT_DT="$refine"
    mkdir -p "$work/decks"
    OPM_SMALL_DECK_DIR="$work/decks" cargo test --release --quiet --manifest-path "$manifest_path" \
        --lib opm_small_direct_write_decks -- --ignored >/dev/null
fi
ressim_dir="$work/ressim"
rm -rf "$ressim_dir"
mkdir -p "$ressim_dir" "$work/flow"

echo "== ResSim (native release)"
run_ressim fim sparse
run_ressim fim dense
run_ressim impes

produced="$(find "$ressim_dir" -name '*.json' | wc -l)"
if [ "$produced" -eq 0 ]; then
    echo "FAIL: the ResSim driver wrote no output; is opm_small_direct_run_ressim still named so?" >&2
    exit 1
fi

echo "== Flow and comparison"
gate_args=()
if [ -n "$mode" ]; then gate_args=(--scorecard "$scorecard" "$mode"); fi
python3 "$repo_root/tools/opm_flow/compare_small_direct.py" \
    --ressim-dir "$ressim_dir" --flow-dir "$work/flow" --json "$work/report.json" \
    "${deck_args[@]}" "${gate_args[@]}" "${compare_args[@]}"
echo "full report: $work/report.json"
