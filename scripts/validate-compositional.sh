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
#            1D displacement, its three refinements, and the single-cell depletion.
#            Skips with a note when flowexp_comp has not been built; the committed fixture is
#            still checked by the comp_reference_* tests either way.
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
trap 'rm -f "$log_file"' EXIT

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
    run_filter comp_domain_      4   # C6 domain sweep
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
    run_filter comp_well_        29   # C11 wells: sources, derivatives, controls, multi-completion
    run_filter comp_reference_   10   # C12 against OPM's flowexp_comp on 1D_COMP
    run_filter comp_depletion_    4   # C12's second fixture: single-cell phase-changing depletion
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
    echo "gate ok: the committed fixture reproduces from the installed OPM headers"
}

run_reference() {
    echo "== reference: re-run the 1D compositional case and verify its fixture =="
    local binary="${FLOWEXP_COMP:-$(cd "${repo_root}/.." && pwd)/ressim-opm-build/opm-simulators/build/bin/flowexp_comp}"
    if [ ! -x "${binary}" ]; then
        echo "SKIP: flowexp_comp not built; the committed fixture is still checked by the" >&2
        echo "      comp_reference_* tests, but it was not regenerated." >&2
        echo "      Build it with: bash tools/opm_compositional/build-flowexp-comp.sh" >&2
        return 0
    fi
    bash "$repo_root/tools/opm_compositional/run-1d-comp.sh" --check
    bash "$repo_root/tools/opm_compositional/run-refinement.sh" --check
    bash "$repo_root/tools/opm_compositional/run-depletion.sh" --check
    echo "gate ok: every compositional reference fixture reproduces from flowexp_comp"
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
    refinement) run_refinement ;;
    all)     run_thermo; run_fixture; run_reference; run_wasm ;;
    *)
        echo "usage: $0 {thermo|fixture|reference|refinement|native|wasm|all}" >&2
        exit 2
        ;;
esac

echo
echo "compositional gate '$mode': OK"
