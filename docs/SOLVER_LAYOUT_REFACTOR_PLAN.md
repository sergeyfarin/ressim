# Solver Layout Refactor Plan

This note proposes a clean Rust-core layout with three ownership zones:

- root `src/lib/ressim/src/` for solver-agnostic domain code and public facade code
- `src/lib/ressim/src/impes/` for the legacy IMPES timestep path
- `src/lib/ressim/src/fim/` for the fully implicit timestep path

The immediate goal is structural clarity, not a physics rewrite. The public `ReservoirSimulator`
API should stay stable while internal ownership becomes explicit.

## Why Change The Layout

The current tree already has a dedicated `fim/` folder, but IMPES remains spread across root files:

- `step.rs` mixes solver dispatch with substantial FIM retry logic and the full IMPES loop.
- `pressure_eqn.rs` and `transport.rs` are IMPES-specific, but they live beside shared modules.
- crate-level tests under `src/tests/` mix solver-agnostic behavior checks with FIM-owned coverage.

That makes it harder to answer basic ownership questions:

- what code is shared physics versus solver implementation
- what tests are stable cross-solver behavior versus white-box solver checks
- where a new solver-specific helper should live

## Target Tree

```text
src/lib/ressim/src/
  lib.rs
  frontend.rs
  capillary.rs
  grid.rs
  mobility.rs
  pvt.rs
  relperm.rs
  reporting.rs
  well.rs
  well_control.rs
  solvers/
    mod.rs
    bicgstab.rs
    faer_sparse_lu.rs
  impes/
    mod.rs
    timestep.rs
    pressure.rs
    transport.rs
    tests/
      mod.rs
      timestep.rs
      pressure.rs
      transport.rs
      wells.rs
  fim/
    mod.rs
    timestep.rs
    assembly.rs
    flash.rs
    newton.rs
    scaling.rs
    state.rs
    wells.rs
    linear/
      mod.rs
      gmres_block_jacobi.rs
      dense_lu_debug.rs
      sparse_lu_debug.rs
    tests/
      mod.rs
      assembly.rs
      flash.rs
      newton.rs
      state.rs
      wells.rs
      timestep.rs
  tests/
    README.md
    runtime_api.rs
    geometry_api.rs
    three_phase.rs
    buckley.rs
    depletion.rs
    pvt_properties.rs
    well_controls.rs
    physics/
      mod.rs
      fixtures.rs
      depletion_oil.rs
      depletion_gas.rs
      depletion_liberation.rs
      waterflood.rs
      gas_flood.rs
      gas_cap.rs
      geometry_anisotropy.rs
      pvt_flash.rs
      wells_sources.rs
```

## Ownership Rules

### Root: shared/common only

Keep only modules that are valid inputs to more than one solver or are part of the public facade:

- `lib.rs`: crate facade, type exports, module declarations
- `frontend.rs`: wasm/public API methods on `ReservoirSimulator`
- `grid.rs`: geometry and indexing
- `pvt.rs`, `relperm.rs`, `capillary.rs`: constitutive physics
- `mobility.rs`: shared mobility and fractional-flow helpers
- `well.rs`: well definitions and scheduling data
- `well_control.rs`: shared control policy and resolved-control structs
- `reporting.rs`: shared reporting/output structs and per-step history appenders
- `solvers/`: reusable linear-algebra backends, only if they remain solver-agnostic

Root should not own a concrete timestep algorithm once this refactor is done.

### `impes/`: explicit pressure plus transport path

Move the current IMPES-specific execution code here:

- `pressure_eqn.rs` -> `impes/pressure.rs`
- `transport.rs` -> `impes/transport.rs`
- the IMPES part of `step.rs` -> `impes/timestep.rs`

`impes/timestep.rs` should own:

- the adaptive substep loop for the IMPES path
- pressure solve retry handling for IMPES
- calls into `impes::pressure` and `impes::transport`

It should not own public wasm-facing methods. Those stay in `frontend.rs`.

### `fim/`: fully implicit path

`fim/` already has the right high-level shape. The main change is to make it complete and
self-contained by moving the remaining FIM driver logic out of `step.rs`:

- move `step_internal_fim` and its cooldown or trace helpers into `fim/timestep.rs`
- keep `assembly.rs`, `flash.rs`, `newton.rs`, `scaling.rs`, `state.rs`, `wells.rs`, and
  `linear/` in `fim/`

After that, `fim/mod.rs` becomes the single internal entrypoint for the FIM solver path.

## Dispatcher Shape

The current root `step.rs` is the main structural smell because it contains both:

- solver selection
- substantial solver implementation

Preferred end state:

```rust
impl ReservoirSimulator {
    pub(crate) fn step_internal(&mut self, target_dt_days: f64) {
        if self.fim_enabled {
            crate::fim::step_timestep(self, target_dt_days);
        } else {
            crate::impes::step_timestep(self, target_dt_days);
        }
    }
}
```

There are two acceptable ways to realize that:

1. Keep a tiny root `step.rs` that only dispatches.
2. Remove root `step.rs` entirely and let `frontend.rs` call `fim::step_timestep()` or
   `impes::step_timestep()` directly.

Option 2 is cleaner if the public `step()` API already lives in `frontend.rs` and no other code
needs a root step module.

## Test Layout

The repo already has a useful split between crate-level behavior tests and module-adjacent white-box
tests. The missing piece is solver ownership.

### Root `src/tests/`

Keep only tests that should remain valid regardless of whether the underlying solver is IMPES or
FIM:

- API contract tests
- geometry and layered-grid behavior
- PVT and table validation behavior
- solver-agnostic physics families that can run through either path
- parity tests that intentionally compare IMPES versus FIM through the public simulator API

This directory is the shared behavior suite.

### `src/impes/tests/`

Put IMPES-specific tests here:

- IMPES pressure-assembly behavior
- transport-update details that are specific to the explicit saturation path
- IMPES timestep retry and stability-factor rules
- any IMPES-only well/source handling that does not apply to FIM

If a test directly calls `calculate_fluxes()` or `update_saturations_and_pressure()`, it is a good
candidate for `impes/tests/`.

### `src/fim/tests/`

Put FIM-specific tests here:

- Newton acceptance and damping rules
- FIM timestep retry and cooldown policy
- assembly Jacobian or residual structure
- flash and regime-switch logic
- FIM-specific well coupling or linear-solver behavior

The existing `fim/assembly_tests.rs` is the clearest example of test code that belongs under a FIM
solver-owned test tree.

## Concrete File Moves

### Phase 1: directory split with no behavior change

- create `src/lib/ressim/src/impes/`
- rename `pressure_eqn.rs` to `impes/pressure.rs`
- rename `transport.rs` to `impes/transport.rs`
- move the IMPES-only parts of `step.rs` to `impes/timestep.rs`
- move the FIM-only parts of `step.rs` to `fim/timestep.rs`
- leave a minimal dispatcher in root or remove root `step.rs`

This phase should be mechanical and should avoid algorithm changes.

### Phase 2: test ownership cleanup

- create `src/lib/ressim/src/impes/tests/`
- create `src/lib/ressim/src/fim/tests/`
- move `fim/assembly_tests.rs` to `fim/tests/assembly.rs`
- move other FIM module-internal test blocks into files under `fim/tests/` where practical
- move any IMPES-only crate tests out of root `src/tests/`
- keep shared public-behavior tests in root `src/tests/`

### Phase 3: shared helper audit

After the physical split is done, audit whether any remaining helpers are in the wrong place:

- if a helper only serves IMPES, move it under `impes/`
- if a helper only serves FIM, move it under `fim/`
- if both solvers use it, keep it at root

Do not force solver-specific code into root just to avoid duplicate imports.

## Module Declaration Sketch

At the end of the refactor, `lib.rs` should look conceptually like this:

```rust
mod capillary;
mod fim;
mod frontend;
mod grid;
mod impes;
mod mobility;
mod pvt;
mod relperm;
mod reporting;
mod solvers;
mod well;
mod well_control;

#[cfg(test)]
mod tests;
```

And the solver modules should own their internal subtrees:

```rust
// impes/mod.rs
pub(crate) mod pressure;
pub(crate) mod timestep;
pub(crate) mod transport;

#[cfg(test)]
mod tests;

// fim/mod.rs
pub(crate) mod assembly;
pub(crate) mod flash;
pub(crate) mod linear;
pub(crate) mod newton;
pub(crate) mod scaling;
pub(crate) mod state;
pub(crate) mod timestep;
pub(crate) mod wells;

#[cfg(test)]
mod tests;
```

## Recommended Classification Of Existing Files

Keep at root:

- `capillary.rs`
- `frontend.rs`
- `grid.rs`
- `mobility.rs`
- `pvt.rs`
- `relperm.rs`
- `reporting.rs`
- `well.rs`
- `well_control.rs`
- `solvers/`

Move to `impes/`:

- `pressure_eqn.rs`
- `transport.rs`
- IMPES-specific parts of `step.rs`

Keep in `fim/`, but complete the subtree:

- `assembly.rs`
- `flash.rs`
- `linear/`
- `newton.rs`
- `scaling.rs`
- `state.rs`
- `wells.rs`
- FIM-specific parts of `step.rs` -> `fim/timestep.rs`

Keep in shared root tests:

- `tests/runtime_api.rs`
- `tests/geometry_api.rs`
- `tests/three_phase.rs`
- public physics families that should stay stable across internal solver refactors

Likely move to solver-owned test trees over time:

- `tests/spe1_fim.rs` -> `fim/tests/spe1.rs`
- any test that directly exercises FIM internals
- any test that directly exercises IMPES-only internals

## Recommended Execution Order

1. Create `impes/mod.rs` and `fim/timestep.rs` first.
2. Move code out of `step.rs` without changing logic.
3. Rewire imports and module declarations.
4. Move solver-owned tests under `impes/tests/` and `fim/tests/`.
5. Only after the tree is stable, do any deeper cleanup such as renaming helpers or shrinking root
   facade code.

## Acceptance Criteria

The refactor is structurally complete when all of the following are true:

- root contains only shared domain code, public facade code, and shared reusable linear-solvers
- IMPES implementation lives entirely under `impes/`
- FIM implementation lives entirely under `fim/`
- root `tests/` contains only solver-agnostic or intentional cross-solver tests
- solver-specific white-box tests live under the owning solver subtree
- changing one solver no longer requires searching mixed root files to discover ownership