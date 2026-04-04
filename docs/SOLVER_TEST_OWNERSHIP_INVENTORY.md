# Solver Test Ownership Inventory

This document records the Phase 2 ownership split for Rust solver tests in `src/lib/ressim/src`.

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

## Phase 2 Moves Completed

- moved FIM-only depletion coverage from `src/tests/depletion.rs` to `src/fim/tests/depletion.rs`
- moved FIM-only SPE1 smoke coverage from the shared suite to `src/fim/tests/spe1.rs`
- extracted FIM-local well tests from shared physics coverage into `src/fim/tests/wells.rs`
- extracted IMPES-local transport/reporting tests into `src/impes/tests/transport.rs`
- removed the dead orphan `src/lib/ressim/src/tests/spe1_short.rs`
- rewrote the shared flash helper path in `src/tests/physics/fixtures.rs` so the pressure-only
  liberation checks use shared gas-split logic instead of the IMPES transport updater
- replaced the old IMPES-only public gas-injection source-rate check with a shared two-solver test
  in `src/tests/physics/wells_sources.rs`

## Current Gaps And Next Sub-Phase

Phase 2 established the ownership structure and moved the clearest solver-local tests, but the
coverage program is not finished.

Next sub-phase priorities:

1. Add more shared parity coverage that runs the same public scenario through both solver paths for
   above-solver behavior, especially well-control and rate-reporting cases.
2. Audit `src/lib/ressim/src/impes/pressure.rs` and `src/lib/ressim/src/impes/timestep.rs` for any
   remaining IMPES-internal behavior that deserves solver-local tests rather than living only under
   shared runtime coverage.
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
- `physics_depletion_liberation_undersaturated_rs_stays_constant`
- `physics_wells_sources_gas_injection_surface_totals_match_target_on_both_solvers`

All of the above passed after the ownership split.