#!/usr/bin/env bash
set -euo pipefail

# Compositional validation gate (COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md, C6).
#
# Modes:
#   thermo   the standalone thermodynamics: specification, units, EOS, stability, flash,
#            derivatives, transport, surface separation, and the domain sweep. ~20 s.
#   fixture  rebuild the C0 thermodynamic fixture and verify it reproduces byte for byte.
#            Needs the OPM headers and a C++20 compiler. ~15 s.
#   reference re-run every compositional case through flowexp_comp and verify its fixture: the
#            1D displacement, its three refinements, the skin variant, both single-cell
#            depletions and #47's 2-D quarter five spot. Also remeasures the convergence census -
#            how far each reference is from timestep-converged - because C12 once assumed that and
#            was wrong.
#            Then writes every committed trajectory fixture's ResSim counterpart as
#            `<case>.comp.json` and checks it against the shared cross-solver scorecard, so a
#            compositional regression fails the same ratchet as a black-oil one (Part C, #22).
#            Skips the fixture regeneration with a note when flowexp_comp has not been built; the
#            record comparison still runs against the committed fixtures.
#   records  the record comparison alone: run the ignored writer against the committed fixtures and
#            check the shared scorecard. No flowexp_comp, OPM headers or OPM Python bindings. This
#            is the mode normal CI runs; `reference` adds the local fixture regeneration.
#   update-scorecard
#            As `reference`, but deliberately re-baselines the compositional cases in
#            `opm/reference-decks/small-direct/scorecard.json`. Run it on a committed tree.
#   refinement C12's timestep and grid refinement study, in RELEASE. Runs the deck's case at four
#            sub-steps and on four grids against references re-solved on each grid. ~3 min, which
#            is why the plan puts it behind an explicit runner rather than in the default gate.
#   native   thermo + fixture + reference.
#   wasm     compile-only check for the wasm32 target. Not a substitute for the
#            execution-based native/WASM parity C13 owes.
#   all      thermo + fixture + reference + wasm. NOT refinement: it is the slow one, and a gate
#            nobody runs is not a gate. Run it before touching the well model, the flux or the
#            timestep lifecycle, and before any claim about C12's acceptance bands.
#
# Deliberately NOT included: any black-oil gate. Use scripts/validate-solver-coverage.sh for
# those. Running both is what a shared-code change needs; this script does not decide that for
# you, because silently widening a gate is how a gate stops meaning anything.

mode="${1:-native}"

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
manifest_path="$repo_root/src/lib/ressim/Cargo.toml"

log_file="$(mktemp)"
cleanup_dirs=()
trap 'rm -rf "${cleanup_dirs[@]}" 2>/dev/null || true; rm -f "$log_file"' EXIT

# Every filter below must be shown to have actually executed tests. `cargo test <filter>` exits 0
# when the filter matches nothing, so a renamed or cfg-ed-out test would otherwise turn its gate
# line into a no-op that still reports success. "Executed" means *passed*: an `#[ignore]`d test is
# counted separately and cannot serve as a gate. This mirrors validate-solver-coverage.sh, which
# has caught exactly that failure four times in this repository.
run_filter() {
    local filter="$1"
    local minimum="$2"
    local status=0

    cargo test --manifest-path "$manifest_path" "$filter" 2>&1 | tee "$log_file" || status=$?
    if [ "$status" -ne 0 ]; then
        echo "FAIL: cargo test '$filter' exited $status." >&2
        exit "$status"
    fi

    local counts passed ignored
    counts="$(awk '/^test result:/ {
        gsub(/;/, "")
        for (i = 2; i <= NF; i++) {
            if ($i == "passed") p += $(i - 1)
            if ($i == "ignored") g += $(i - 1)
        }
    } END { print (p + 0) " " (g + 0) }' "$log_file")"
    passed="${counts%% *}"
    ignored="${counts##* }"

    if [ "$passed" -lt "$minimum" ]; then
        echo "FAIL: filter '$filter' ran $passed test(s), expected at least $minimum." >&2
        if [ "$ignored" -gt 0 ]; then
            echo "      $ignored were ignored; an ignored test cannot serve as a gate." >&2
        fi
        echo "      Either a test was renamed or removed, or the expected count needs" >&2
        echo "      updating deliberately in this script." >&2
        exit 1
    fi
    echo "gate ok: '$filter' ran $passed test(s)"
}

# The expected minimums are the counts at the commit that introduced each task. They are floors,
# not exact matches, so adding a test never breaks the gate while deleting one does.
run_thermo() {
    echo "== thermo: standalone thermodynamics =="
    if ! cargo test --manifest-path "$manifest_path" --no-run; then
        echo "FAIL: test target does not compile — no gate was run." >&2
        exit 1
    fi
    run_filter comp_spec_       19   # C1 specification
    run_filter comp_units_       9   # C1 units and the gas-constant contract
    run_filter comp_eos_        20   # C2 Peng-Robinson
    run_filter comp_stability_   7   # C3a Michelsen stability
    run_filter comp_rr_          6   # C3b Rachford-Rice, including the extended window
    run_filter comp_flash_      13   # C3b flash
    run_filter comp_derivatives_ 12  # C4
    run_filter comp_transport_  11   # C5 LBC and saturations
    run_filter comp_surface_    10   # C5 surface separation
    run_filter comp_domain_      5   # C6 domain sweep, and #75's SPE5 dew-point sweep
    run_filter comp_api_        11   # C13 engine boundary: payload, snapshot, checkpoint
    run_filter comp_layout_     12   # C7 component layout
    run_filter comp_state_      18   # C7 accepted/trial state, cache, checkpoints
    run_filter comp_accumulation_ 12  # C8 cell inventory, residual and Jacobian
    run_filter comp_scaling_      7   # C8 row and primary-variable scaling
    run_filter comp_flux_        15   # C9 component face flux
    run_filter comp_assembly_     9   # C9 global residual and Jacobian assembly
    run_filter comp_gravity_      9   # C9 gravity subtask
    run_filter comp_relperm_      8   # relative permeability models
    run_filter comp_newton_      10   # C10 Newton solve and update policy
    run_filter comp_rollback_    10   # C10 timestep lifecycle, retry and commit
    run_filter comp_linear_      11   # C10 block-sparse Jacobian, sparse LU and CPR (#49)
    run_filter comp_well_        29   # C11 wells: sources, derivatives, controls, multi-completion
    run_filter comp_reference_   11   # C12 on 1D_COMP and #47's 2-D quarter five spot, against OPM
    run_filter comp_skin_         3   # C12's well-conditioned 1D variant: the cumulative target
    run_filter comp_depletion_    9   # C12's second fixture: single-cell depletion, rate and BHP
    run_filter comp_matched_      1   # C12 at the reference's own timestep - see the forensics doc
    run_filter comp_oracle_       3   # the oracle convergence census, and the band rule it enforces
    run_filter comp_injection_    1   # surface-rate control on a MIXTURE, against OPM's own flash
    run_filter comp_spe3_        10   # #51/#52: fluid/separator/CVD/1D gates and the authored payload
    run_filter comp_spe5_         3   # #52: payload, EOS gradient and closed-well equilibrium
    run_filter comp_water_       18   # #46: immiscible water, Stone II, FD Jacobian, conservation, JutulDarcy
    run_filter comp_schedule_    10   # #50: WAG, both short boundaries, restore policy, mass ledger and restart
    run_filter schedule::tests::  5   # #50/#17: the engine-neutral event envelope, ordering and clock rules
}

run_fixture() {
    echo "== fixture: regenerate the C0 oracle and verify it reproduces =="
    if ! command -v g++ > /dev/null; then
        echo "FAIL: g++ not found; the OPM PTFlash harness cannot be built." >&2
        exit 1
    fi
    if [ ! -f /usr/include/opm/material/constraintsolvers/PTFlash.hpp ]; then
        echo "FAIL: OPM headers not installed (libopm-common-dev)." >&2
        exit 1
    fi
    bash "$repo_root/tools/opm_compositional/generate.sh" --check
    bash "$repo_root/tools/opm_compositional/run-spe3.sh" --check --opm-only
    echo "gate ok: the committed fixture reproduces from the installed OPM headers"
}

run_records() {
    # The same `<case>.<run>.json` record and scorecard the black-oil gate uses (#22, Part C).
    # The committed fixture is the oracle here, so this half needs neither flowexp_comp nor the
    # OPM Python bindings - only the release build and the shared scorecard.
    local mode="${1:---check}"
    local provenance="bash scripts/validate-compositional.sh records"
    if [ "$mode" = "--write-scorecard" ]; then
        provenance="bash scripts/validate-compositional.sh update-scorecard"
    fi

    local records_dir
    if [ -n "${COMP_REFERENCE_OUT:-}" ]; then
        records_dir="$COMP_REFERENCE_OUT"
        mkdir -p "$records_dir"
    else
        records_dir="$(mktemp -d)"
        cleanup_dirs+=("$records_dir")
    fi

    echo "== records: ResSim trajectories against the committed fixtures and the shared scorecard =="
    local python="python3"
    if ! "$python" -c 'import numpy' 2>/dev/null; then
        # CI's system Python has no numpy. Keep the fallback local to this gate rather than
        # changing the runner image; the existing workflow already proves venv/pip work there.
        local venv="${RESSIM_COMPARE_VENV:-${TMPDIR:-/tmp}/ressim-compare-venv}"
        if [ ! -x "$venv/bin/python" ] || ! "$venv/bin/python" -c 'import numpy' 2>/dev/null; then
            echo "== records: creating $venv with numpy"
            if command -v uv > /dev/null; then
                uv venv "$venv" > /dev/null
                uv pip install --python "$venv/bin/python" numpy > /dev/null
            else
                python3 -m venv "$venv"
                "$venv/bin/pip" install --quiet numpy
            fi
        fi
        python="$venv/bin/python"
    fi
    local status=0
    COMP_REFERENCE_OUT="$records_dir" cargo test --release --manifest-path "$manifest_path" --lib \
        comp_reference_run_records -- --ignored --nocapture 2>&1 | tee "$log_file" || status=$?
    if [ "$status" -ne 0 ]; then
        echo "FAIL: comp_reference_run_records exited $status." >&2
        exit "$status"
    fi
    local passed
    passed="$(awk '/^test result:/ { gsub(/;/, ""); for (i = 2; i <= NF; i++) if ($i == "passed") p += $(i - 1) } END { print (p + 0) }' "$log_file")"
    if [ "$passed" -lt 1 ]; then
        echo "FAIL: comp_reference_run_records did not run; the record gate would be a no-op." >&2
        exit 1
    fi
    echo "gate ok: 'comp_reference_run_records' ran $passed test(s)"

    "$python" "$repo_root/tools/opm_flow/compare_small_direct.py" \
        --ressim-dir "$records_dir" \
        --comp-reference-dir "$repo_root/opm/compositional" \
        --scorecard "$repo_root/opm/reference-decks/small-direct/scorecard.json" \
        "$mode" --provenance-command "$provenance"
    echo "gate ok: every compositional trajectory matches its fixture within the scorecard bands"
}

run_reference() {
    echo "== reference: re-run the compositional cases and verify their fixtures =="
    local binary="${FLOWEXP_COMP:-$(cd "${repo_root}/.." && pwd)/ressim-opm-build/opm-simulators/build/bin/flowexp_comp}"
    if [ ! -x "${binary}" ]; then
        echo "SKIP: flowexp_comp not built; the committed fixtures are still checked by the" >&2
        echo "      comp_reference_* tests and by the run-record comparison below, but they" >&2
        echo "      were not regenerated." >&2
        echo "      Build it with: bash tools/opm_compositional/build-flowexp-comp.sh" >&2
    else
        bash "$repo_root/tools/opm_compositional/run-1d-comp.sh" --check
        bash "$repo_root/tools/opm_compositional/run-refinement.sh" --check
        bash "$repo_root/tools/opm_compositional/run-depletion.sh" --check
        bash "$repo_root/tools/opm_compositional/run-injection.sh" --check
        bash "$repo_root/tools/opm_compositional/run-quarter-five-spot.sh" --check
        bash "$repo_root/tools/opm_compositional/check-reference-convergence.sh" --check
        echo "gate ok: every compositional reference fixture reproduces from flowexp_comp, and each"
        echo "         one's distance from timestep-convergence is what the census records"
    fi

    local mode="--check"
    if [ "${1:-}" = "update-scorecard" ]; then
        mode="--write-scorecard"
    fi
    run_records "$mode"
}

# The refinement study. Release, because a debug build turns three minutes into forty.
#
# These tests are `#[ignore]`d, so `run_filter`'s "an ignored test cannot serve as a gate" rule
# would reject them; they are run with `--ignored` and counted the same way.
run_refinement() {
    echo "== refinement: C12's timestep and grid refinement study (release, ~3 min) =="
    local status=0
    cargo test --release --manifest-path "$manifest_path" --lib -- comp_refinement_ \
        --ignored --nocapture --test-threads=1 2>&1 | tee "$log_file" || status=$?
    if [ "$status" -ne 0 ]; then
        echo "FAIL: the refinement study exited $status." >&2
        exit "$status"
    fi
    local passed
    passed="$(awk '/^test result:/ { gsub(/;/, ""); for (i = 2; i <= NF; i++) if ($i == "passed") p += $(i - 1) } END { print (p + 0) }' "$log_file")"
    if [ "$passed" -lt 3 ]; then
        echo "FAIL: expected at least 3 comp_refinement_* tests, $passed ran." >&2
        exit 1
    fi
    echo "gate ok: 'comp_refinement_' ran $passed test(s)"
    echo "note: the measured ladders are printed above. docs/COMPOSITIONAL_VALIDATION.md's C12"
    echo "      record is what they are compared against; update it deliberately, not silently."
}

run_wasm() {
    echo "== wasm: compile-only check =="
    if ! rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; then
        echo "FAIL: the wasm32-unknown-unknown target is not installed." >&2
        echo "      rustup target add wasm32-unknown-unknown" >&2
        exit 1
    fi
    cargo check --manifest-path "$manifest_path" --target wasm32-unknown-unknown
    echo "gate ok: the crate compiles for wasm32-unknown-unknown"
    echo "note: this is compilation only. Execution-based native/WASM parity is C13's."
}

case "$mode" in
    thermo)  run_thermo ;;
    fixture) run_fixture ;;
    native)  run_thermo; run_fixture; run_reference ;;
    wasm)    run_wasm ;;
    reference) run_reference ;;
    records) run_records ;;
    update-scorecard) run_reference update-scorecard ;;
    refinement) run_refinement ;;
    all)     run_thermo; run_fixture; run_reference; run_wasm ;;
    *)
        echo "usage: $0 {thermo|fixture|reference|records|update-scorecard|refinement|native|wasm|all}" >&2
        exit 2
        ;;
esac

echo
echo "compositional gate '$mode': OK"
