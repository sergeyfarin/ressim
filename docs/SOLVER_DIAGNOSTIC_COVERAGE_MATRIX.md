# Solver Diagnostic Coverage Matrix

This document maps ignored or explicitly diagnostic tests to the fast default gates that protect the
same behavior surface. If no such fast sibling exists, the diagnostic must be marked exploratory
only.

Use this document together with `docs/SOLVER_TEST_COVERAGE_PLAN.md` and
`docs/SOLVER_TEST_OWNERSHIP_INVENTORY.md`.

## Shared Runtime

| Ignored / diagnostic test | Purpose | Default fast sibling(s) | Status |
|---|---|---|---|
| `rate_control_reporting_benchmark_fim_matches_impes` | Tight FIM-vs-IMPES parity on mixed-control public reporting | `public_step_bhp_limited_producer_reports_same_control_state_on_both_solvers`, `public_step_gas_injector_reports_same_control_state_on_both_solvers`, `mixed_control_public_step_keeps_same_limit_flags_on_both_solvers`, `simple_pressure_control_public_step_has_same_stable_contract_on_both_solvers` | Covered by fast siblings; ignored test remains a parity-tuning probe |

## Shared Physics: Depletion

| Ignored / diagnostic test | Purpose | Default fast sibling(s) | Status |
|---|---|---|---|
| `physics_depletion_oil_dep_pss_timestep_refinement_is_locally_stable` | Coarse-vs-fine oil depletion refinement stability | `dep_pss_fim_single_cell_depletion_is_timestep_stable`, `physics_depletion_oil_closed_system_monotone`, `physics_depletion_oil_public_reporting_contract_holds_on_both_solvers` | Covered |
| `physics_depletion_oil_dep_pss_late_time_matches_dietz_reference_smoke` | Late-time analytical alignment against Dietz | `physics_depletion_oil_closed_system_monotone`, `physics_depletion_oil_public_reporting_contract_holds_on_both_solvers` | Diagnostic-only until the current analytical/model-alignment gap is resolved |
| `physics_depletion_gas_single_cell_timestep_refinement_keeps_inventory_stable` | Coarse-vs-fine gas depletion refinement stability | `physics_depletion_gas_single_cell_timestep_stable`, `physics_depletion_gas_single_cell_closed_system_monotone`, `physics_depletion_gas_public_invariants_hold_on_both_solvers` | Covered |
| `physics_depletion_liberation_timestep_refinement_keeps_transition_accounting_stable` | Coarse-vs-fine liberation transition stability | `physics_depletion_liberation_public_transition_contract_holds_on_both_solvers`, `physics_depletion_liberation_fim_stepping_liberates_gas`, `physics_depletion_liberation_component_balances_close_across_phase_transition`, `physics_depletion_liberation_undersaturated_rs_stays_constant` | Covered |

## Shared Physics: Waterflood / Gas Flood / Gas Cap

| Ignored / diagnostic test | Purpose | Default fast sibling(s) | Status |
|---|---|---|---|
| `physics_waterflood_1d_timestep_refinement_keeps_front_and_balance_stable` | Coarse-vs-fine 1D waterflood directional stability | `physics_waterflood_1d_mass_conservative`, `physics_waterflood_1d_injector_saturation_increases`, `physics_waterflood_1d_public_reporting_contract_holds_on_both_solvers` | Covered |
| `physics_waterflood_buckley_case_a_fim_matches_impes_early_profile` | Tight early-profile FIM-vs-IMPES Buckley parity | `physics_waterflood_1d_public_reporting_contract_holds_on_both_solvers`, `benchmark_buckley_leverett_case_a_favorable_mobility`, `benchmark_buckley_leverett_case_b_more_adverse_mobility` | Covered for public contract and analytical trend; ignored test remains parity-gap probe |
| `physics_waterflood_buckley_refined_discretization_improves_alignment` | Refined-grid analytical alignment | `benchmark_buckley_leverett_case_a_favorable_mobility`, `benchmark_buckley_leverett_case_b_more_adverse_mobility`, `benchmark_buckley_leverett_smaller_dt_improves_coarse_alignment` | Covered |
| `physics_gas_flood_1d_timestep_refinement_keeps_breakthrough_ordering_stable` | Coarse-vs-fine 1D gas-flood refinement stability | `physics_gas_flood_1d_short_material_balance_matches_inventory_change`, `physics_gas_flood_saturation_sum_stays_physical`, `physics_gas_flood_short_inventory_and_reporting_contract_hold_on_both_solvers` | Covered |
| `physics_gas_flood_spe1_coarse_grid_reaches_producer_gas_breakthrough` | Larger-grid breakthrough smoke | `spe1_fim_gas_injection_creates_free_gas`, `spe1_fim_producer_gas_breakthrough_smoke`, `physics_gas_flood_short_inventory_and_reporting_contract_hold_on_both_solvers` | Covered; ignored test remains larger-grid benchmark probe |
| `physics_gas_cap_gravity_column_timestep_refinement_keeps_profile_stable` | Coarse-vs-fine gravity-column stability | `physics_gas_cap_vertical_column_fim_matches_impes_hydrostatic_benchmark` and default gas-cap family balance/gradient tests | Covered |

## Shared Physics: Geometry / Anisotropy

| Ignored / diagnostic test | Purpose | Default fast sibling(s) | Status |
|---|---|---|---|
| `physics_geometry_gas_flood_2d_high_perm_streak_advances_gas_front_faster` | Slow areal gas-flood directional heterogeneity probe | `physics_geometry_gas_flood_2d_high_perm_streak_public_contract_holds_on_both_solvers`, `physics_gas_flood_short_inventory_and_reporting_contract_hold_on_both_solvers` | Covered |
| `physics_geometry_waterflood_3d_high_kz_spreads_front_across_layers` | Slow layered waterflood anisotropy probe | `physics_geometry_waterflood_3d_high_kz_public_contract_holds_on_both_solvers`, `physics_geometry_gas_segregation_3d_high_kz_accelerates_vertical_migration`, `physics_waterflood_1d_public_reporting_contract_holds_on_both_solvers` | Covered |
| `physics_geometry_waterflood_2d_refined_streak_uses_iterative_backend_and_keeps_row_ordering` | Refined >1024-row iterative-backend plus row-ordering probe | `physics_geometry_waterflood_2d_high_perm_streak_advances_front_faster`, `large_default_fim_system_still_uses_iterative_backend`, `wasm_target_hands_off_direct_backend_above_512_rows` | Covered; ignored test remains a combined runtime-plus-physics probe |

## Shared Benchmarks / Manual Probes

| Ignored / diagnostic test | Purpose | Default fast sibling(s) | Status |
|---|---|---|---|
| `native_single_step_fim_probe_case_a_24_cells` | Manual native-vs-wasm timing and single-step probe | None required | Exploratory only |

## Shared Analytical / Parity Diagnostics Kept Explicitly Opt-In

| Ignored / diagnostic test | Purpose | Default fast sibling(s) | Status |
|---|---|---|---|
| `physics_waterflood_buckley_case_a_fim_matches_impes_early_profile` | Early-profile parity envelope between IMPES and FIM | `physics_waterflood_1d_public_reporting_contract_holds_on_both_solvers`, `benchmark_buckley_leverett_case_a_favorable_mobility`, `benchmark_buckley_leverett_case_b_more_adverse_mobility` | Diagnostic-only until the known FIM-vs-IMPES early-profile parity gap is resolved |
| `rate_control_reporting_benchmark_fim_matches_impes` | Mixed-control runtime parity envelope between IMPES and FIM | `public_step_bhp_limited_producer_reports_same_control_state_on_both_solvers`, `public_step_gas_injector_reports_same_control_state_on_both_solvers`, `mixed_control_public_step_keeps_same_limit_flags_on_both_solvers`, `simple_pressure_control_public_step_has_same_stable_contract_on_both_solvers` | Diagnostic-only until the known mixed-control rate parity gap is resolved |

## FIM-Owned Diagnostics

| Ignored / diagnostic test | Purpose | Default fast sibling(s) | Status |
|---|---|---|---|
| `dep_pss_fim_timestep_refinement_is_locally_stable` | FIM-owned depletion refinement stability | `dep_pss_fim_closed_system_depletion_invariants_hold`, `dep_pss_fim_single_cell_depletion_is_timestep_stable`, `dep_pss_fim_single_step_reports_direct_oil_mb` | Covered |
| `dep_pss_fim_late_time_matches_dietz_reference_smoke` | FIM-owned late-time analytical alignment | `dep_pss_fim_closed_system_depletion_invariants_hold` | Diagnostic-only until the current analytical/model-alignment gap is resolved |
| `dep_pss_fim_refinement_diagnostics_trace_rate_loss` | Explain where refinement drift comes from | `dep_pss_fim_timestep_refinement_is_locally_stable` and the default depletion fast gates above | Covered as debug trace only |
| `dep_pss_fim_single_cell_refinement_diagnostics` | Separate local-cell from spatial-flux contribution | `dep_pss_fim_single_cell_depletion_is_timestep_stable`, `dep_pss_fim_single_cell_local_newton_leaves_small_absolute_oil_residual` | Covered as debug trace only |
| `dep_pss_fim_single_cell_tight_newton_diagnostics` | Compare loose vs tight Newton acceptance | `dep_pss_fim_single_cell_local_newton_leaves_small_absolute_oil_residual` | Covered as debug trace only |

## Shared/FIM Boundary Audit Result

The final ownership audit found one real boundary mismatch and it has now been corrected:

- `physics_depletion_oil_single_cell_abs_oil_balance`
- `physics_depletion_oil_single_cell_timestep_stable`
- `physics_depletion_oil_fim_single_step_reports_direct_oil_mb`

These were FIM-specific depletion-oracle checks living in the shared depletion-oil family. Their
ownership is now FIM-local under `src/lib/ressim/src/fim/tests/depletion.rs` via:

- `dep_pss_fim_single_cell_local_newton_leaves_small_absolute_oil_residual`
- `dep_pss_fim_single_cell_depletion_is_timestep_stable`
- `dep_pss_fim_single_step_reports_direct_oil_mb`

No remaining shared test was found to depend directly on IMPES-private retry/substep helpers or on
FIM-private well/Newton topology beyond the explicitly crate-visible helper plumbing kept in
`tests/physics/fixtures.rs` for solver-local reuse.

## Remaining Intentional Diagnostic-Only Probes

These ignored tests are intentionally *not* being promoted into default fast gates right now:

- late-time Dietz probes in shared and FIM-owned depletion coverage because they still track a
	known analytical/model-alignment gap rather than a stable regression contract
- early-profile FIM-vs-IMPES Buckley parity and mixed-control runtime parity because both still
	expose known solver-parity gaps that are useful for targeted tuning but too unstable for the fast
	gate
- manual native-vs-wasm timing probes because they are exploratory tooling, not correctness gates
- combined large-case geometry/backend probes because the physics and backend-policy parts are now
	separately protected by fast siblings

## IMPES-Owned Diagnostics

There are currently no ignored IMPES-owned tests under `src/lib/ressim/src/impes/tests/`.

Current default obligations are:

- `transport.rs`: explicit transport/reporting sanity
- `timestep.rs`: retry/substep and pressure-state guard behavior

## Grouped Validation Commands

Use `scripts/validate-solver-coverage.sh` or the VS Code tasks in `.vscode/tasks.json` when
changing ownership or coverage instead of only running one test at a time.

### Shared bucket

```bash
./scripts/validate-solver-coverage.sh shared
```

### FIM-owned bucket

```bash
./scripts/validate-solver-coverage.sh fim
```

### IMPES-owned bucket

```bash
./scripts/validate-solver-coverage.sh impes
```

### All buckets

```bash
./scripts/validate-solver-coverage.sh all
```