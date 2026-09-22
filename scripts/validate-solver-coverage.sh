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
#
# "Executed" means *passed*, not merely *matched*. An `#[ignore]`d test is
# reported on the `test result:` line under `ignored`, so counting passed and
# ignored together would accept a filter that ran no code at all — exactly the
# silent-omission failure this check exists to prevent. Release-only replays in
# this repository are `#[ignore]`d, so that is a reachable state, not a
# hypothetical one.
run_test() {
    local filter="$1"
    local status=0

    cargo test --manifest-path "$manifest_path" "$filter" -- --nocapture 2>&1 | tee "$log_file" || status=$?
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

    if [ "$passed" -eq 0 ]; then
        if [ "$ignored" -gt 0 ]; then
            echo "FAIL: filter '$filter' matched only $ignored ignored test(s) — no code ran." >&2
            echo "      An \`#[ignore]\`d test cannot serve as a gate; select a running test" >&2
            echo "      or move this replay to the explicit release gate." >&2
        else
            echo "FAIL: filter '$filter' matched no tests — the gate did not run." >&2
            echo "      A test was probably renamed, removed or cfg-ed out; fix the filter." >&2
        fi
        exit 1
    fi

    if [ "$ignored" -gt 0 ]; then
        echo "gate ok: '$filter' ran $passed test(s) ($ignored ignored)"
    else
        echo "gate ok: '$filter' ran $passed test(s)"
    fi
}

run_shared() {
    run_test public_step_bhp_limited_producer_reports_same_control_state_on_both_solvers
    run_test public_step_gas_injector_reports_same_control_state_on_both_solvers
    run_test mixed_control_public_step_keeps_same_limit_flags_on_both_solvers
    run_test closed_system_public_step_keeps_same_water_inventory_on_both_solvers
    run_test material_balance_drift_warns_on_both_solvers
    run_test simple_pressure_control_public_step_has_same_stable_contract_on_both_solvers
    run_test multiple_wells_in_same_block_are_rejected_without_state_change
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
    # `fim::wells::tests::` is a *different* module from `fim::tests::wells::` — the unit tests
    # living beside `fim/wells.rs` rather than the integration tests under `fim/tests/`. It was
    # in no bucket, which is how two failures there (#27, #28) survived every green gate run
    # until the compositional-readiness audit found them by hand (#13).
    run_test fim::wells::tests::
    # The AD assembly and well-AD parity gates were likewise unselected by any bucket.
    run_test assembly_ad
    run_test wells_ad
    # `FIM-REPAIR-F4`: the whole linear stack (dispatch, well-Schur elimination/recovery, CPR,
    # direct backends and the report contract) was in no bucket either. The expensive offline
    # solver labs in this module are `#[ignore]`d and stay out; the gate counts passed tests.
    run_test fim::linear::
    # `FIM-REPAIR-F7`: `fim::flow_resv::` was likewise ungated, which is how a stale 1x1x1
    # fixture sat failing in it. Same omission class as #13.
    run_test fim::flow_resv::
    # `FIM-REPAIR-F5`: the lifecycle/rollback/conservation contracts, runnable as one named
    # group. Covers acceptance-residual identity, rejected-attempt rollback and closed-system
    # component conservation.
    run_test fim_repair_
    run_test dep_pss_fim_closed_system_depletion_invariants_hold
    run_test dep_pss_fim_single_cell_local_newton_leaves_small_absolute_oil_residual
    run_test dep_pss_fim_single_cell_depletion_is_timestep_stable
    # FIM-DIRECT-001 / FIM-BUBBLE-001 regressions. Their unit contracts live in `fim::properties`
    # and `fim::flash`, which, like the modules above, were in no bucket; the system tests share a
    # `does_not_fragment` suffix so a new one joins this gate by name.
    run_test fim::properties::
    run_test fim::flash::
    run_test does_not_fragment
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
