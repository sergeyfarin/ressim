# Solver Test Ownership Inventory

This document records the Phase 2 ownership split for Rust solver tests in `src/lib/ressim/src`.

The forward-looking completion criteria for this ownership split now live in
`docs/SOLVER_TEST_COVERAGE_PLAN.md`.

The goal is a stable layout with three explicit buckets:

- shared crate-level tests under `src/tests/`
- FIM-owned solver-local tests under `src/fim/tests/`
- IMPES-owned solver-local tests under `src/impes/tests/`

Module-adjacent white-box tests such as `src/lib/ressim/src/fim/assembly_tests.rs` remain next to
the implementation when they verify private algebraic structure rather than solver-family behavior.

## Current Ownership Rules

### Shared: `src/lib/ressim/src/tests/`

Use the shared crate-level suite when a test should stay valid across solver refactors or when it
exercises the simulator through public or crate-visible APIs above the solver boundary.

Current shared files:

- `runtime_api.rs`: public stepping contracts, runtime reporting, and simulator-level behavior
- `geometry_api.rs`: geometry and layer API behavior
- `pvt_properties.rs`: shared PVT/property behavior
- `three_phase.rs`: three-phase API and public behavior
- `well_controls.rs`: shared well-control decisions and public control behavior
- `buckley.rs`: shared Buckley-style benchmark and reference checks
- `physics/`
  - `depletion_oil.rs`, `depletion_gas.rs`, `depletion_liberation.rs`: solver-agnostic depletion,
    inventory, and flash behavior
  - `waterflood.rs`, `gas_flood.rs`, `gas_cap.rs`: scenario-scale transport and benchmark-envelope
    behavior
  - `wells_sources.rs`: shared public well/source behavior that should hold for either solver
  - `pvt_flash.rs`: shared flash and PVT behavior
  - `geometry_anisotropy.rs`: shared geometry/anisotropy outcomes
  - `fixtures.rs`: shared test fixtures; may expose crate-visible helpers when solver-local tests
    need shared scenario builders

### FIM-owned: `src/lib/ressim/src/fim/tests/`

Use FIM-local tests when the test depends on FIM internals, FIM-only smoke coverage, or coupled
well/Newton behavior that is not a stable cross-solver contract.

Current FIM-owned files:

- `depletion.rs`: FIM-owned depletion diagnostics and implicit-solver-specific coverage
- `spe1.rs`: stable FIM SPE1 smoke coverage
- `wells.rs`: FIM-local well topology, Peaceman oracle, coupled rate/BHP behavior, and accepted
  state/reporting consistency checks

### IMPES-owned: `src/lib/ressim/src/impes/tests/`

Use IMPES-local tests when the test exercises IMPES-only pressure/transport helpers or reporting
 behavior coupled directly to the explicit transport path.

Current IMPES-owned files:

- `transport.rs`: transport/reporting checks that depend on `update_saturations_and_pressure()` or
  IMPES-local near-well mixture handling
- `timestep.rs`: adaptive-substep and pressure-state-sanity checks that exercise the IMPES retry
  loop and pressure-physicality guard directly

## Phase 2 Moves Completed

- moved FIM-only depletion coverage from `src/tests/depletion.rs` to `src/fim/tests/depletion.rs`
- moved FIM-only SPE1 smoke coverage from the shared suite to `src/fim/tests/spe1.rs`
- extracted FIM-local well tests from shared physics coverage into `src/fim/tests/wells.rs`
- extracted IMPES-local transport/reporting tests into `src/impes/tests/transport.rs`
- moved IMPES-only adaptive-substep and pressure-physicality coverage out of
  `src/tests/runtime_api.rs` into `src/impes/tests/timestep.rs`
- removed the dead orphan `src/lib/ressim/src/tests/spe1_short.rs`
- rewrote the shared flash helper path in `src/tests/physics/fixtures.rs` so the pressure-only
  liberation checks use shared gas-split logic instead of the IMPES transport updater
- replaced the old IMPES-only public gas-injection source-rate check with a shared two-solver test
  in `src/tests/physics/wells_sources.rs`

## Current Gaps And Next Sub-Phase

Phase 2 established the ownership structure and moved the clearest solver-local tests, but the
coverage program is not finished.

The first shared parity slice above the solver boundary is now in place in
`src/lib/ressim/src/tests/runtime_api.rs`:

- `public_step_bhp_limited_producer_reports_same_control_state_on_both_solvers`
- `public_step_gas_injector_reports_same_control_state_on_both_solvers`
- `mixed_control_public_step_keeps_same_limit_flags_on_both_solvers`

An additional stable-contract parity slice is now also in place in
`src/lib/ressim/src/tests/runtime_api.rs`:

- `closed_system_public_step_keeps_same_water_inventory_on_both_solvers`
- `simple_pressure_control_public_step_has_same_stable_contract_on_both_solvers`
- `shared_block_multiwell_public_step_remains_finite_on_both_solvers`

Shared parity now also extends into the physics suite for stable public outcomes that do not depend
on matching internal stepping patterns:

- `src/lib/ressim/src/tests/physics/waterflood.rs`:
  `physics_waterflood_1d_public_reporting_contract_holds_on_both_solvers`
- `src/lib/ressim/src/tests/physics/gas_flood.rs`:
  `physics_gas_flood_short_inventory_and_reporting_contract_hold_on_both_solvers`
- `src/lib/ressim/src/tests/physics/depletion_oil.rs`:
  `physics_depletion_oil_public_reporting_contract_holds_on_both_solvers`
- `src/lib/ressim/src/tests/physics/depletion_gas.rs`:
  `physics_depletion_gas_public_invariants_hold_on_both_solvers`
- `src/lib/ressim/src/tests/physics/depletion_liberation.rs`:
  `physics_depletion_liberation_public_transition_contract_holds_on_both_solvers`

These physics-level parity checks intentionally stay on public contracts only:

- bounded accounting rather than exact internal history matching
- positive injection / non-negative production sign conventions
- finite public report values such as `producing_gor`
- final-time completion of the requested step schedule
- BHP-limit fractions staying inside the public `[0, 1]` range
- closed-system monotonicity such as non-increasing pressure or in-place inventory where the
  scenario guarantees it
- phase-transition public outcomes such as crossing the bubble point, creating free gas, and
  keeping reporting/accounting fields finite without matching internal stepping traces

The IMPES pressure/timestep audit is now complete for the obvious ownership mismatches.

Audit result:

- `adaptive_timestep_produces_multiple_substeps_for_strong_flow` is IMPES-local because it exists
  to verify the explicit retry/substep loop rather than a solver-agnostic public contract
- `pressure_resolve_on_substep_produces_physical_results` is IMPES-local because it directly
  validates the IMPES pressure-state guard and retry behavior
- `default_step_path_reports_rate_controlled_well_state` remains shared because it checks a public
  reporting contract of the default solver path rather than a private IMPES retry invariant
- `benchmark_like_substepping_completes_requested_dt` remains shared because it checks public step
  completion semantics, not the internal IMPES retry implementation details

Next sub-phase priorities:

1. Record the default-fast-gate to ignored-diagnostic mapping for FIM and IMPES obligations, using
  `docs/SOLVER_TEST_COVERAGE_PLAN.md` as the canonical checklist.
2. Add more shared parity coverage only where the public contract is stable enough to avoid baking
  in known rate-magnitude gaps between the solvers.
3. Continue pruning mixed ownership from the shared suite whenever a test needs private solver
  internals to stay meaningful.
4. Keep shared physics helpers solver-agnostic where possible so future parity tests do not depend
  on one solver's internal update path.

## Validation Status For This Ownership Pass

The moved and rewritten tests were validated with focused Rust runs:

- `spe1_fim_first_steps_converge_without_stall`
- `spe1_fim_gas_injection_creates_free_gas`
- `fim::tests::wells::*`
- `impes::tests::transport::*`
- `impes::tests::timestep::*`
- `physics_depletion_liberation_undersaturated_rs_stays_constant`
- `physics_wells_sources_gas_injection_surface_totals_match_target_on_both_solvers`
- `public_step_bhp_limited_producer_reports_same_control_state_on_both_solvers`
- `public_step_gas_injector_reports_same_control_state_on_both_solvers`
- `mixed_control_public_step_keeps_same_limit_flags_on_both_solvers`
- `closed_system_public_step_keeps_same_water_inventory_on_both_solvers`
- `simple_pressure_control_public_step_has_same_stable_contract_on_both_solvers`
- `shared_block_multiwell_public_step_remains_finite_on_both_solvers`
- `physics_waterflood_1d_public_reporting_contract_holds_on_both_solvers`
- `physics_gas_flood_short_inventory_and_reporting_contract_hold_on_both_solvers`
- `physics_depletion_oil_public_reporting_contract_holds_on_both_solvers`
- `physics_depletion_gas_public_invariants_hold_on_both_solvers`
- `physics_depletion_liberation_public_transition_contract_holds_on_both_solvers`
- `physics_gas_cap_vertical_column_fim_matches_impes_hydrostatic_benchmark`
- `physics_wells_sources_gas_injection_surface_totals_match_target_on_both_solvers`
- `physics_depletion_oil_closed_system_monotone`
- `physics_depletion_gas_single_cell_closed_system_monotone`
- `physics_depletion_liberation_undersaturated_rs_stays_constant`
- `tests::runtime_api::default_step_path_reports_rate_controlled_well_state`
- `tests::runtime_api::benchmark_like_substepping_completes_requested_dt`

All of the above passed after the ownership split.