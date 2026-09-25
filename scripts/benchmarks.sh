#!/usr/bin/env bash
set -euo pipefail

# Measure every benchmark behind docs/BENCHMARKS.md and keep the page generated from the records
# (#54). Nothing on that page is typed by hand: the tables between <!-- GENERATED:x --> markers come
# from docs/benchmarks/benchmarks.json, which records per section the commit it was measured on.
#
#   bash scripts/benchmarks.sh run    [--tier fast|full] [--only SECTION]...   measure into $BENCH_OUT
#   bash scripts/benchmarks.sh check  [--tier fast|full] [--only SECTION]...   measure, compare with the records
#   bash scripts/benchmarks.sh update [--tier fast|full] [--only SECTION]...   measure, re-record, re-render
#   bash scripts/benchmarks.sh render [--check]                               re-render (or verify) only
#
# Tiers. `fast` needs only the Rust toolchain, Node and Python (what CI has): Buckley-Leverett,
# SPE1 against the published series, three-phase, the depletion column, native/wasm parity, the
# long-horizon FIM cases and the compositional comparisons against committed flowexp_comp fixtures.
# `full` (the default) adds everything that runs OPM Flow: the cross-solver scorecard, SPE1 against
# Flow on the same grid, the Flow column of the depletion table, and the check that the
# compositional fixtures still reproduce from flowexp_comp. A missing simulator is reported, never
# passed over in silence; a section run without one keeps its previous records on `update`.
#
# `update` refuses a dirty tree: a baseline has to name the commit it was measured on (AGENTS.md).
# `check` fails when a banded error grows by more than a tenth of its band or leaves it, or when a
# recorded measurement is no longer produced; other changes are listed to be recorded with `update`.

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
manifest="$repo_root/src/lib/ressim/Cargo.toml"
tool="$repo_root/tools/benchmarks/benchmarks.py"
run_dir="${BENCH_OUT:-${TMPDIR:-/tmp}/ressim-benchmarks}"

command="${1:-}"
[ -n "$command" ] && shift
tier="full"
only=()
render_check=""
while [ $# -gt 0 ]; do
    case "$1" in
        --tier) tier="${2:?missing tier}"; shift 2 ;;
        --only) only+=("${2:?missing section}"); shift 2 ;;
        --check) render_check="--check"; shift ;;
        --help|-h) sed -n '3,24p' "$0"; exit 0 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
done
case "$tier" in fast|full) ;; *) echo "--tier is fast or full" >&2; exit 2 ;; esac

case "$command" in
    render) exec python3 "$tool" render $render_check ;;
    run|check|update) ;;
    *) sed -n '3,24p' "$0"; exit 2 ;;
esac

if [ "$command" = update ] && [ -n "$(git -C "$repo_root" status --porcelain --untracked-files=no)" ]; then
    echo "update: tracked files are modified; commit first so the records name the commit they measure" >&2
    exit 1
fi

have_flow=""
if [ "$tier" = full ] && command -v flow >/dev/null 2>&1 && python3 -c 'import opm.io' 2>/dev/null; then
    have_flow="$(flow --version 2>/dev/null | head -1)"
fi
flowexp_comp="${FLOWEXP_COMP:-$(cd "$repo_root/.." && pwd)/ressim-opm-build/opm-simulators/build/bin/flowexp_comp}"

rm -rf "$run_dir"
mkdir -p "$run_dir/fim_wasm"
export RESSIM_BENCH_OUT="$run_dir"
status_lines=()

wanted() {
    [ ${#only[@]} -eq 0 ] && return 0
    local s
    for s in "${only[@]}"; do [ "$s" = "$1" ] && return 0; done
    return 1
}

# status SECTION STATUS COMMAND [NOTE] [MISSING_ORACLE] [key=value ...]
status() {
    local section="$1" state="$2" cmd="$3" note="${4:-}" missing="${5:-}"
    shift 5 2>/dev/null || shift $#
    status_lines+=("$(python3 - "$section" "$state" "$cmd" "$note" "$missing" "$@" <<'EOF'
import json, sys
section, state, cmd, note, missing, *extra = sys.argv[1:]
entry = {"status": state, "command": cmd}
if note:
    entry["note"] = note
if missing:
    entry["missing"] = missing.split(",")
entry.update(kv.split("=", 1) for kv in extra)
print(json.dumps([section, entry]))
EOF
)")
}

# Runs release lib tests; prints their result lines and returns their exit status, so a failing
# benchmark marks its section `failed` instead of quietly writing fewer records.
cargo_bench() {
    local out rc=0
    out="$(cargo test --release --quiet --manifest-path "$manifest" --lib "$@" 2>&1)" || rc=$?
    printf '%s\n' "$out" | grep -E '^test result|panicked|FAILED' || true
    return "$rc"
}

# ran_or_failed SECTION CMD RC [NOTE] [MISSING] [key=value ...]
ran_or_failed() {
    local section="$1" cmd="$2" rc="$3"
    shift 3
    if [ "$rc" -eq 0 ]; then
        status "$section" ran "$cmd" "$@"
    else
        status "$section" failed "$cmd" "a benchmark test failed (exit $rc)" "${2:-}"
    fi
}

echo "== building the release test binary"
cargo test --release --quiet --manifest-path "$manifest" --lib --no-run 2>&1 | grep -E '^error' && exit 1

if wanted buckley; then
    echo "== buckley"
    cmd="cargo test --release --lib benchmark_buckley -- --include-ignored"
    rc=0; cargo_bench benchmark_buckley -- --include-ignored || rc=$?
    ran_or_failed buckley "$cmd" "$rc" "" ""
fi

if wanted spe1; then
    echo "== spe1"
    cmd="cargo test --release --lib -- --include-ignored --exact tests::spe1_acceptance::spe1_full_horizon_matches_published_reference tests::spe1_acceptance::spe1_areal_refinement_reference_error_replay"
    rc=0
    cargo_bench -- --include-ignored --exact \
        tests::spe1_acceptance::spe1_full_horizon_matches_published_reference \
        tests::spe1_acceptance::spe1_areal_refinement_reference_error_replay || rc=$?
    if [ -n "$have_flow" ]; then
        python3 "$repo_root/tools/opm_flow/spe1_refinement_oracle.py" --out "$run_dir/spe1-flow" \
            --records "$run_dir/records.jsonl" >/dev/null || rc=$?
        ran_or_failed spe1 "$cmd; python3 tools/opm_flow/spe1_refinement_oracle.py" "$rc" "" "" "flow=$have_flow"
    else
        ran_or_failed spe1 "$cmd" "$rc" "" flow
    fi
fi

if wanted three_phase; then
    echo "== three_phase"
    cmd="cargo test --release --lib -- --include-ignored --exact tests::three_phase_acceptance::three_phase_acceptance_error_replay tests::three_phase_acceptance::three_phase_gas_injection_matches_opm_flow_twin"
    rc=0
    cargo_bench -- --include-ignored --exact \
        tests::three_phase_acceptance::three_phase_acceptance_error_replay \
        tests::three_phase_acceptance::three_phase_gas_injection_matches_opm_flow_twin || rc=$?
    ran_or_failed three_phase "$cmd" "$rc" "" ""
fi

# The cross-solver run comes before the depletion column, whose Flow values are the small-direct
# bo-1d decks' final reports.
cross_out="$run_dir/cross"
if wanted cross_solver || wanted depletion; then
    if [ -n "$have_flow" ]; then
        echo "== cross_solver"
        set +e
        CROSS_SOLVER_OUT="$cross_out" bash "$repo_root/scripts/validate-cross-solver.sh" > "$run_dir/cross_solver.log" 2>&1
        rc=$?
        set -e
        tail -1 "$run_dir/cross_solver.log"
        cp "$cross_out/report.json" "$run_dir/cross_solver.json"
        note=""
        [ "$rc" -ne 0 ] && note="scorecard check failed (exit $rc); see validate-cross-solver.sh"
        wanted cross_solver && status cross_solver ran "bash scripts/validate-cross-solver.sh" "$note" "" "flow=$have_flow"
    else
        wanted cross_solver && status cross_solver skipped "bash scripts/validate-cross-solver.sh" "OPM Flow not available (tier $tier)" ""
    fi
fi

if wanted depletion; then
    echo "== depletion"
    cmd="cargo test --release --lib physics_depletion_grid_convergence_"
    rc=0; cargo_bench physics_depletion_grid_convergence_ || rc=$?
    if [ -n "$have_flow" ]; then
        python3 - "$repo_root" "$cross_out/flow" "$run_dir/records.jsonl" <<'EOF'
import json, sys
from pathlib import Path
repo, flow_dir, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
sys.path.insert(0, str(repo / "tools" / "opm_flow"))
from compare_small_direct import flow_fields  # noqa: E402
with out.open("a") as f:
    for nx in (10, 40):
        final = flow_fields(flow_dir / f"bo-1d-{nx}")[-1]
        # Uniform cells and porosity: the plain mean is the pore-volume-weighted average.
        for name, value, unit in (("avg_pressure", float(final["p"].mean()), "bar"),
                                  ("avg_sat_gas", float(final["sg"].mean()), "frac")):
            f.write(json.dumps({"kind": "metric", "section": "depletion", "case": f"Flow nx={nx}",
                                "metric": name, "value": value, "band": None, "unit": unit,
                                "reference": "OPM Flow, small-direct bo-1d deck", "same_model": True,
                                "at": "t=100 d"}) + "\n")
EOF
        ran_or_failed depletion "$cmd; small-direct bo-1d-10/40 Flow output" "$rc" "" "" "flow=$have_flow"
    else
        ran_or_failed depletion "$cmd" "$rc" "" flow
    fi
fi

if wanted parity; then
    echo "== parity"
    if bash "$repo_root/scripts/validate-native-binding.sh" > "$run_dir/parity.log" 2>&1; then
        status parity ran "bash scripts/validate-native-binding.sh" "" ""
    else
        tail -5 "$run_dir/parity.log"
        status parity failed "bash scripts/validate-native-binding.sh" "gate failed; see its output" ""
    fi
fi

if wanted fim_wasm; then
    echo "== fim_wasm"
    bash "$repo_root/scripts/build-wasm.sh" > "$run_dir/build-wasm.log" 2>&1
    python3 "$tool" plan-fim-wasm | while read -r file preset grid dt steps; do
        node "$repo_root/scripts/fim-wasm-diagnostic.mjs" --preset "$preset" --grid "$grid" --dt "$dt" \
            --steps "$steps" --diagnostic quiet --json > "$run_dir/fim_wasm/$file.json"
    done
    status fim_wasm ran "node scripts/fim-wasm-diagnostic.mjs --preset <P> --grid <G> --dt <D> --steps <N> --diagnostic quiet --json" "" "" "node=$(node --version)"
fi

if wanted compositional; then
    echo "== compositional"
    cmd="cargo test --release --lib -- comp_matched_1d_trajectory_agrees_with_opm comp_depletion_bhp_trajectory_matches_opm_at_matched_timestep comp_depletion_bhp_cumulative_production_matches_opm"
    rc=0
    cargo_bench -- comp_matched_1d_trajectory_agrees_with_opm \
        comp_depletion_bhp_trajectory_matches_opm_at_matched_timestep \
        comp_depletion_bhp_cumulative_production_matches_opm || rc=$?
    if [ "$rc" -ne 0 ]; then
        ran_or_failed compositional "$cmd" "$rc" "" ""
    elif [ "$tier" = full ] && [ -x "$flowexp_comp" ]; then
        if bash "$repo_root/scripts/validate-compositional.sh" reference > "$run_dir/compositional.log" 2>&1; then
            status compositional ran "$cmd; bash scripts/validate-compositional.sh reference" \
                "fixtures reproduce from flowexp_comp" ""
        else
            status compositional failed "bash scripts/validate-compositional.sh reference" \
                "fixtures no longer reproduce from flowexp_comp; see its output" ""
        fi
    else
        status compositional ran "$cmd" "fixtures not re-derived: flowexp_comp not built (tier $tier)" ""
    fi
fi

printf '%s\n' "${status_lines[@]}" | python3 -c '
import json, sys
print(json.dumps(dict(json.loads(line) for line in sys.stdin if line.strip()), indent=1))
' > "$run_dir/status.json"
echo "== records in $run_dir"

case "$command" in
    run) python3 "$tool" check --run "$run_dir" || true ;;
    check) python3 "$tool" check --run "$run_dir" && python3 "$tool" render --check ;;
    update)
        python3 "$tool" collect --run "$run_dir"
        python3 "$tool" render
        echo "review, then commit docs/benchmarks/benchmarks.json and docs/BENCHMARKS.md"
        ;;
esac
