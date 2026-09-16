#!/usr/bin/env bash
set -euo pipefail

# Compositional validation gate (COMPOSITIONAL_FLUID_EXECUTION_PLAN_2026-09-14.md, C6).
#
# Modes:
#   thermo   the standalone thermodynamics: specification, units, EOS, stability, flash,
#            derivatives, transport, surface separation, and the domain sweep. ~20 s.
#   fixture  rebuild the C0 reference fixture and verify it reproduces byte for byte.
#            Needs the OPM headers and a C++20 compiler. ~15 s.
#   native   thermo + fixture. This is THERMO-READY.
#   wasm     compile-only check for the wasm32 target. Not a substitute for the
#            execution-based native/WASM parity C13 owes.
#   all      every mode above.
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
    run_filter comp_newton_      10   # C10 Newton solve and update policy
    run_filter comp_rollback_    10   # C10 timestep lifecycle, retry and commit
    run_filter comp_well_        20   # C11 wells: sources, derivatives and controls
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
    native)  run_thermo; run_fixture ;;
    wasm)    run_wasm ;;
    all)     run_thermo; run_fixture; run_wasm ;;
    *)
        echo "usage: $0 {thermo|fixture|native|wasm|all}" >&2
        exit 2
        ;;
esac

echo
echo "compositional gate '$mode': OK"
