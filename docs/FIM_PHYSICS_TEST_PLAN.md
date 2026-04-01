# FIM Physics Test Plan

This document defines the physics-first test expansion plan for the Rust FIM path.

Goal: catch physically wrong FIM behavior before spending time on nonlinear convergence tuning.

Primary rule: a case that is not locally conservative, not materially timestep-stable, or not internally consistent at wells/phase transitions is not a convergence problem yet.

## Objectives

1. Build a cheap default regression gate that catches critical physical mistakes early.
2. Keep test files separated by physics family so solver files do not grow into test containers.
3. Make it easy to run one physics family at a time with `cargo test <filter>`.
4. Keep the slowest refinement and analytical checks opt-in so development loops stay fast.
5. Reuse small fixture builders so the same physical identities are tested in 1D, 2D, and 3D without duplicating setup logic.

## Test Hierarchy

### Tier 0: Local Physics Identities

Purpose: catch critical mistakes in storage, flash, PVT, well/source conversion, and phase accounting.

Properties:

- no history scans
- 1 cell or 2 connected cells
- no slow analytical comparisons
- default suite
- should run in milliseconds

Examples:

- no-PVT oil compressibility affects `B_o`, density, and accumulation consistently
- 2-phase FIM path keeps `S_g = 0` and `R_s = 0` exactly
- local single-cell depletion accepted state leaves small absolute oil residual
- local well source term, perforation residual, and reported rate agree on the same state
- undersaturated-to-saturated switch preserves total gas inventory
- interface fluxes are conservative when accumulation is disabled in the fixture

### Tier 1: Small Closed-System Physics

Purpose: catch wrong physics that only appears once accumulation, wells, and state evolution interact across several steps.

Properties:

- 1D or tiny 2D grids
- short runtimes
- default suite if kept under practical runtime limits

Examples:

- closed oil depletion monotonicity and cumulative production consistency
- closed gas depletion pressure and inventory monotonicity
- local timestep refinement stability on a single-cell and short 1D case
- gravity-free waterflood mass conservation in a short 1D line drive
- gas flood with `DRSDT = 0` preserves injected free-gas accounting

### Tier 2: Small Open-System Scenario Checks

Purpose: verify coupled physics families on realistic but still cheap fixtures.

Properties:

- small 1D, 2D, or thin 3D cases
- default if cheap enough, otherwise opt-in

Examples:

- 1D Buckley-style waterflood front sanity
- 1D gas flood with producer breakthrough ordering
- 2D areal depletion / waterflood / gas flood material balance checks
- 3D thin-column gravity segregation and gas-cap sanity checks
- layered anisotropic permeability cases that verify `k_z / k_h` actually changes outcomes in the correct direction

### Tier 3: Refinement And Analytical Probes

Purpose: identify subtler discretization/modeling gaps after the critical physics gate is green.

Properties:

- slower
- `#[ignore]` by default
- explicitly named as refinement or analytical probes

Examples:

- timestep refinement on `dep_pss`, `dep_decline`, waterflood, gas flood, gas-cap cases
- grid refinement in 1D and 2D
- late-time Dietz/Fetkovich/decline analytical checks
- benchmark-like SPE1 and future black-oil comparison probes

## Coverage Matrix

Each family below should eventually have at least one Tier 0 or Tier 1 test, then one Tier 2 or Tier 3 follow-up.

### 1. Oil Depletion

Purpose: verify oil storage, pressure decline, and well production under compressibility.

Required checks:

- single-cell depletion local absolute oil residual stays small
- single-cell timestep stability
- closed-system monotonic pressure decline
- cumulative oil is nondecreasing and bounded by storage loss
- short 2D `dep_pss` refinement stability
- late-time analytical comparison as opt-in

### 2. Gas Depletion

Purpose: verify gas storage, gas compressibility, and gas-rate accounting.

Required checks:

- single-cell gas depletion storage responds to `B_g(p)` correctly
- gas material balance in closed depletion stays small
- local gas pressure decline is timestep-stable
- produced gas rate and inventory loss are consistent
- layered 3D gas depletion shows gravity-consistent gas migration only when gravity is enabled

### 3. Oil Depletion With Gas Liberation / Separation

Purpose: verify below-bubble-point flash behavior, liberated gas accounting, and `DRSDT = 0` vs redissolution semantics.

Required checks:

- single-cell pressure drop below bubble point preserves total gas inventory
- phase switch does not lose or create gas
- liberated gas changes oil storage and local mobility consistently
- timestep refinement on a small depletion-with-liberation case
- separate tests for `gas_redissolution_enabled = false` and `true`

### 4. Waterflood

Purpose: verify water transport, oil displacement, and well/source partitioning.

Required checks:

- 1D line-drive mass conservation
- water saturation remains bounded and monotone in the expected direction
- injected water and produced water/oil accounting are consistent
- local timestep refinement on a short 1D waterflood
- 2D areal waterflood with heterogeneity preserves total mass and expected front ordering
- 3D layered waterflood reacts correctly to `k_z / k_h`

### 5. Gas Flood

Purpose: verify injected gas transport, gas/oil partitioning, and breakthrough physics.

Required checks:

- 1D gas injection creates free gas without spurious oil/gas loss
- local injector-cell accounting matches exported intercell flux plus storage change
- producer GOR rises only after physically meaningful gas arrival
- timestep refinement on a small gas flood
- 3D layered gas flood reacts correctly to gravity and `k_z / k_h`

### 6. Gas Cap / Gravity Segregation

Purpose: verify vertical segregation, gas-cap support, and capillary/gravity sign correctness.

Required checks:

- 1D vertical column keeps hydrostatic ordering stable when stationary
- gas-cap case shows upward gas preference when gravity is enabled
- disabling gravity removes the segregation trend
- capillary pressure sign tests remain separate from runtime scenario tests
- 3D thin layered gas-cap case checks directional migration under anisotropy

### 7. PVT And Flash Behavior

Purpose: verify property reconstruction and derivatives independent of scenario type.

Required checks:

- no-PVT oil compressibility consistency
- tabular saturated and undersaturated `B_o`, `mu_o`, `R_s`, `B_g`, `mu_g`
- derivative consistency against finite difference for key PVT functions
- flash invariants: `S_w + S_o + S_g = 1`, nonnegative phases, no hidden phase loss
- exact 2-phase mode zero-gas invariant

### 8. Well And Source Consistency

Purpose: verify that well control, perforation residuals, reporting, and source terms all use the same physical state.

Required checks:

- same-state perforation source equals reported component rates
- well residual is near zero when local state is declared converged
- BHP-controlled producer/injector local rate matches Peaceman connection law
- surface-rate-controlled producer/injector conversions are consistent with local PVT state
- group-well shared-BHP consistency across completions

### 9. Permeability, Geometry, And Dimensionality

Purpose: verify that real geometry and anisotropy actually affect the physics in the correct direction.

Required checks:

- 1D homogeneous cases for the cheapest baseline
- 2D areal cases for sweep geometry and multi-direction fluxes
- 3D thin-layer cases for vertical communication and gravity
- heterogeneous permeability cases with known ordering expectations
- anisotropy checks for `k_z / k_h`
- per-layer `dz` and pore-volume sensitivity checks

## File And Module Layout

Keep tests out of solver implementation files and split them by physics family.

Target structure:

```text
src/lib/ressim/src/tests/
  mod.rs
  physics/
    mod.rs
    fixtures.rs
    depletion_oil.rs
    depletion_gas.rs
    depletion_liberation.rs
    waterflood.rs
    gas_flood.rs
    gas_cap.rs
    pvt_flash.rs
    wells_sources.rs
    geometry_anisotropy.rs
```

Rules:

- `fixtures.rs` holds reusable builders only.
- each family file owns its fast regressions and ignored diagnostics for that family.
- implementation files like `pvt.rs`, `transport.rs`, `wells.rs`, `assembly.rs` should keep only tightly local unit tests that are truly private-API-specific.
- scenario-sized physics checks should move into `src/lib/ressim/src/tests/physics/*` instead of staying in calculation files.

## Naming And Runtime Policy

Naming should make filtering cheap and predictable.

Examples:

- `physics_depletion_oil_single_cell_abs_oil_balance`
- `physics_depletion_gas_closed_inventory_monotone`
- `physics_waterflood_1d_mass_conservative`
- `physics_gas_cap_3d_gravity_changes_vertical_profile`
- `physics_pvt_flash_two_phase_zero_gas_exact`

Runtime policy:

- fast default regressions: no `#[ignore]`
- slower refinement or analytical checks: `#[ignore]` with a reason string
- diagnostics that print traces: `#[ignore]` and clearly named `diagnostic`

This keeps the following usable:

- `cargo test physics_depletion_oil`
- `cargo test physics_waterflood`
- `cargo test physics_gas_cap -- --ignored`

## Parallel And Separate Execution

Rust already runs independent tests in parallel by default, so keep tests stateless.

Requirements:

- no shared mutable global fixture state
- no writes to shared temp files
- no dependence on execution order
- keep long analytical probes ignored by default
- avoid giant monolithic tests that check many families at once

Recommended execution sets:

- fast physics gate: all Tier 0 plus the cheapest Tier 1 tests
- family slice: one family file at a time with name filters
- nightly/explicit slice: ignored refinement and analytical probes

## Rollout Order

### Phase 1: Build The Cheap Physics Gate

Add or keep default tests for:

1. no-PVT oil compressibility
2. exact 2-phase zero-gas invariant
3. local single-cell oil depletion absolute oil balance
4. local single-cell oil depletion timestep stability
5. closed-system depletion monotonicity
6. local well/source/reporting consistency for a producer
7. gas liberation inventory conservation

### Phase 2: Cover Major Flow Families

Add family files and fast tests for:

1. depletion gas
2. depletion with gas liberation
3. waterflood 1D
4. gas flood 1D
5. gas-cap vertical column

Phase 2 completeness audit after the first migration pass:

1. edge-case sweeps across initial saturation, permeability, PVT response, and SCAL shape should live inside the existing Phase 2 family modules before Phase 3 adds bigger geometries
2. the current gas-cap family is still primarily a gravity/hydrostatic sanity family; a true free-gas gas-cap runtime case with oil-gas capillary entry pressure is still missing
3. capillary coverage is currently stronger at local/API level than at runtime scenario level; add at least one runtime case where nonzero entry pressure measurably changes migration, front behavior, or gas-cap support relative to the zero-capillary baseline
4. well/source scenario checks now cover same-state reporting and gas-rate conversion, but shared-BHP multi-completion and Peaceman-law consistency still largely live in `well_controls.rs`
5. anisotropy and heterogeneous geometry outcomes remain Phase 3 work, not Phase 2 regressions

### Phase 3: Add Geometry And Heterogeneity

Add:

1. 2D areal waterflood and gas flood
2. 3D layered gas-cap and flood cases
3. anisotropy and `dz` sensitivity checks

Already done as prep while reducing legacy sprawl:

1. thin-column gas-cap gravity sanity checks already live in dedicated physics modules instead of `lib.rs`
2. 1D gas-flood saturation-closure and large-step bounded-state checks already moved into the gas-flood family module
3. Phase 3 should build on these by adding true 2D/3D sweep and anisotropy cases rather than redoing the same 1D/column coverage
4. the highest-value remaining gaps after the Phase 2 audit are true runtime capillary-direction checks and geometry/anisotropy outcome checks, not more duplicate nominal 1D cases

Phase 3 start criteria and coverage shape:

1. keep at least one fast 2D areal heterogeneity case for waterflood and one for gas flood, with explicit directional expectations such as high-perm streaks advancing fronts faster than flanks
2. keep at least one fast 3D layered anisotropy case where changing `k_z / k_h` measurably changes vertical communication or gravity segregation speed in the expected direction
3. add at least one ignored refined geometry case with enough cells to exercise the iterative linear backend instead of only the small direct-solve path
4. prefer paired fast and slow variants of the same physical idea so default regressions stay cheap while refined probes still cover larger systems
5. base the gravity-segregation expectations on classical unstable-to-stable segregation behavior, as used in industry examples such as MRST gravity-segregation demonstrations, and base heterogeneity expectations on standard high-perm-streak sweep ordering used in black-oil benchmark practice

### Phase 4: Add Slower Refinement And Analytical Probes

Add ignored probes for:

1. timestep refinement in each family
2. selected grid refinement checks
3. late-time analytical comparisons
4. future benchmark parity probes

Phase 4 kickoff status:

1. the depletion-oil family now owns the ignored `dep_pss` timestep-refinement probe and a late-time Dietz comparison diagnostic, instead of leaving those probes only in the legacy depletion test module
2. the waterflood, gas-flood, gas-depletion, liberation, and gas-cap families now each have an ignored coarse-vs-fine refinement probe attached directly to the owning family fixture
3. the waterflood family now owns the Buckley early-profile parity probe and the refined-discretization benchmark probe, and the gas-flood family now owns the larger-grid SPE1-like gas-injection breakthrough probe that used to live outside the family modules
4. both the late-time Dietz probe and the Buckley early-profile parity probe should currently be treated as diagnostic envelopes, not as solved acceptance gates: the repo still shows known model-alignment mismatch there, so these probes are meant to catch catastrophic regressions while that gap stays open
5. the next Phase 4 slice should focus on any remaining slower benchmark-parity and larger-grid probes that still live outside the family modules or remain only as legacy diagnostics

Already done as prep while reducing legacy sprawl:

1. the family modules now hold the fast baseline checks that refinement probes should extend, especially for depletion oil, depletion gas, gas flood, and gas cap
2. moving scenario-scale gas-flood and reporting checks out of `lib.rs` means future ignored probes can be attached next to the owning family instead of mixed into crate-root tests

### Phase 5: Perform comprehensive review of all the tests already available in the code and move them to the new structure or delete if they not needed.

Current progress:

1. PVT/flash, depletion oil, depletion gas, liberation, gas flood, gas cap, and well/source scenario checks have already started moving out of `lib.rs`
2. duplicate gravity, two-phase zero-gas, gas-flood scenario, and gas-specific reporting checks were already consolidated into family modules where they fit better
3. the last mixed crate-root runtime/API, three-phase, and geometry/reporting groups now live in dedicated domain files: `src/lib/ressim/src/tests/runtime_api.rs`, `src/lib/ressim/src/tests/three_phase.rs`, and `src/lib/ressim/src/tests/geometry_api.rs`
4. `src/lib/ressim/src/lib.rs` now keeps only shared test helpers plus explicit benchmark fixture builders used by submodules such as the SPE1-like gas-injection probes
5. the remaining non-family files outside `src/lib/ressim/src/tests/physics/` are intentional benchmark or API-contract homes, not leftover scenario-scale physics tests waiting for a family owner


## Immediate Next Tests To Add

These should be added before more convergence tuning:

1. producer well/source/reporting consistency in a 1-cell oil depletion fixture
2. single-cell gas depletion storage and material-balance test
3. single-cell depletion-with-liberation total-gas conservation test with and without redissolution
4. 1D short waterflood material-balance test
5. 1D short gas-flood material-balance test
6. 1D vertical gas-cap gravity-on vs gravity-off comparison
7. one runtime capillary-entry case where nonzero `pc` or `pc_og` measurably changes migration, front advance, or gas-cap support relative to the zero-capillary baseline
8. one explicit anisotropy outcome case (`k_z / k_h`) before declaring geometry coverage complete
9. one refined geometry probe with row count safely above the direct-solve threshold so the iterative backend path is covered explicitly

## Exit Criterion For Physics-First Gate

Before prioritizing more convergence tuning, the following should be true:

1. fast local storage, flash, and well/source consistency tests are green
2. fast 1D family tests for depletion oil, depletion gas, liberation, waterflood, gas flood, and gas cap are green
3. at least one small 2D and one small 3D physics sanity case are green
4. ignored refinement probes no longer show first-order contradictions in the main family cases
5. any remaining mismatch is clearly benchmark/model-alignment work, not basic conservation or phase-accounting failure
6. gravity and capillary are both exercised by at least one runtime scenario test in addition to local/unit checks
7. geometry coverage includes both a cheap default case and at least one larger ignored case that crosses the iterative linear-backend path