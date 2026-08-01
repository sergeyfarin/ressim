#!/usr/bin/env bash
set -euo pipefail

bucket="${1:-all}"

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
manifest_path="$repo_root/src/lib/ressim/Cargo.toml"

log_file="$(mktemp)"
trap 'rm -f "$log_file"' EXIT

# Compile the test target once, up front. `set -e` would already abort on the
# first failing `cargo test`, but a dedicated build step reports a broken crate
# as a build failure instead of burying E0xxx output in a test bucket.
if ! cargo test --manifest-path "$manifest_path" --no-run; then
    echo "FAIL: test target does not compile — no gate was run." >&2
    exit 1
fi

# `cargo test <filter>` exits 0 when the filter matches nothing ("0 passed;
# ... N filtered out"). A renamed, deleted or cfg-ed-out test would silently
# turn its gate line into a no-op that still reports success, so every filter
# must be shown to have actually executed at least one test.
run_test() {
    local filter="$1"
    local status=0

    cargo test --manifest-path "$manifest_path" "$filter" -- --nocapture 2>&1 | tee "$log_file" || status=$?
    if [ "$status" -ne 0 ]; then
        echo "FAIL: cargo test '$filter' exited $status." >&2
        exit "$status"
    fi

    local ran
    ran="$(awk '/^test result:/ {
        gsub(/;/, "")
        for (i = 2; i <= NF; i++) if ($i == "passed" || $i == "ignored") total += $(i - 1)
    } END { print total + 0 }' "$log_file")"
    if [ "$ran" -eq 0 ]; then
        echo "FAIL: filter '$filter' matched no tests — the gate did not run." >&2
        echo "      A test was probably renamed, removed or cfg-ed out; fix the filter." >&2
        exit 1
    fi
    echo "gate ok: '$filter' ran $ran test(s)"
}

run_shared() {
    run_test public_step_bhp_limited_producer_reports_same_control_state_on_both_solvers
    run_test public_step_gas_injector_reports_same_control_state_on_both_solvers
    run_test mixed_control_public_step_keeps_same_limit_flags_on_both_solvers
    run_test closed_system_public_step_keeps_same_water_inventory_on_both_solvers
    run_test simple_pressure_control_public_step_has_same_stable_contract_on_both_solvers
    run_test shared_block_multiwell_public_step_remains_finite_on_both_solvers
    run_test physics_depletion_oil_public_reporting_contract_holds_on_both_solvers
    run_test physics_depletion_gas_public_invariants_hold_on_both_solvers
    run_test physics_depletion_liberation_public_transition_contract_holds_on_both_solvers
    run_test physics_waterflood_1d_public_reporting_contract_holds_on_both_solvers
    run_test physics_gas_flood_short_inventory_and_reporting_contract_hold_on_both_solvers
    run_test physics_gas_cap_vertical_column_fim_matches_impes_hydrostatic_benchmark
    run_test physics_wells_sources_gas_injection_surface_totals_match_target_on_both_solvers
    run_test physics_wellbore_datum
    run_test physics_geometry_gas_flood_2d_high_perm_streak_public_contract_holds_on_both_solvers
    run_test physics_geometry_waterflood_3d_high_kz_public_contract_holds_on_both_solvers
}

run_fim() {
    run_test spe1_first_year_matches_published_reference
    # Three-phase acceptance criteria — docs/THREE_PHASE_VALIDATION.md.
    run_test three_phase_gas_drive_matches_opm_flow_reference
    run_test three_phase_gas_drive_liberates_solution_gas_as_pressure_falls
    run_test three_phase_gas_flood_breakthrough_time_is_within_acceptance_band
    run_test three_phase_gas_flood_saturation_front_is_monotone_and_advances
    run_test three_phase_gas_flood_phase_closure_holds_for_all_three_phases
    run_test fim::tests::spe1::
    run_test fim::tests::wells::
    run_test dep_pss_fim_closed_system_depletion_invariants_hold
    run_test dep_pss_fim_single_cell_local_newton_leaves_small_absolute_oil_residual
    run_test dep_pss_fim_single_cell_depletion_is_timestep_stable
}

run_impes() {
    run_test physics_depletion_grid_convergence_impes
    run_test impes::tests::reporting::
    run_test impes::tests::transport::
    run_test impes::tests::timestep::
}

case "$bucket" in
    shared)
        run_shared
        ;;
    fim)
        run_fim
        ;;
    impes)
        run_impes
        ;;
    all)
        run_shared
        run_fim
        run_impes
        ;;
    *)
        echo "Usage: $0 {shared|fim|impes|all}" >&2
        exit 2
        ;;
esac