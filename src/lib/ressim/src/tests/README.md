# Rust Test Layout

This directory holds the crate-level Rust test suite for behavior that is best expressed through
public or crate-visible APIs rather than through a single module's private helpers.

## What lives here

- `physics/`
  - Family-owned physics regressions.
  - Use these for scenario-scale correctness: material balance, phase behavior, gravity,
    front movement, geometry effects, and benchmark-envelope checks.
- `../fim/tests/`
  - FIM-owned solver-local tests.
  - Use these when a test depends on FIM internals, coupled well/Newton behavior, or stable
    FIM-only smoke coverage.
- `../impes/tests/`
  - IMPES-owned solver-local tests.
  - Use these when a test depends on IMPES-only pressure/transport helpers or explicit-path
    reporting behavior.
- `runtime_api.rs`
  - Runtime stepping, API contracts, and cross-module simulator behavior.
- `geometry_api.rs`
  - Geometry and per-layer API behavior that is broader than one internal formula.
- `three_phase.rs`
  - Three-phase public behavior and table/API validation.
- `buckley.rs`, `pvt_properties.rs`, `well_controls.rs`
  - Domain or benchmark tests that exercise the crate through higher-level entry points.

## What does not live here

- `fim/assembly_tests.rs`
  - White-box assembler invariants that need direct access to private assembly internals.
  - Examples: exact Jacobian entries, local flux residual structure, neighborhood coupling,
    residual-only assembly behavior, and direct finite-difference checks of the assembled system.
- Other module-adjacent `*_tests.rs` files under `fim/` or similar internal modules
  - Keep tests next to the implementation when they verify private helper behavior or exact
    algebraic structure rather than user-visible simulator behavior.

## Placement rule

- Put a test in `src/tests/` when it validates simulator behavior, physics outcomes, or API-level
  contracts and should remain stable across internal refactors.
- Put a test in `src/fim/tests/` or `src/impes/tests/` when it is solver-owned and would become
  artificial or fragile if forced through a solver-agnostic public harness.
- Put a test in a module-adjacent `*_tests.rs` file when it validates private implementation
  details that are intentionally not exposed outside that module.

This keeps one coherent test architecture without forcing white-box solver tests to leak internal
APIs just to satisfy directory uniformity.