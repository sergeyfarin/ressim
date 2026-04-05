#!/usr/bin/env bash
set -euo pipefail

bucket="${1:-all}"

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
manifest_path="$repo_root/src/lib/ressim/Cargo.toml"

run_test() {
    cargo test --manifest-path "$manifest_path" "$1" -- --nocapture
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
    run_test physics_geometry_gas_flood_2d_high_perm_streak_public_contract_holds_on_both_solvers
    run_test physics_geometry_waterflood_3d_high_kz_public_contract_holds_on_both_solvers
}

run_fim() {
    run_test fim::tests::spe1::
    run_test fim::tests::wells::
    run_test dep_pss_fim_closed_system_depletion_invariants_hold
    run_test dep_pss_fim_single_cell_local_newton_leaves_small_absolute_oil_residual
    run_test dep_pss_fim_single_cell_depletion_is_timestep_stable
}

run_impes() {
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