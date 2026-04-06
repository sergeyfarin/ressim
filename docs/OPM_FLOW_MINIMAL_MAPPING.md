# OPM Flow Minimal Mapping For ResSim

This note captures the minimal solver lessons from OPM Flow that are relevant to ResSim's Rust FIM path.
It is intentionally scoped to a simplified Cartesian-grid black-oil FIM solver, not the full OPM product surface.

## Scope

- Goal: identify which OPM Flow concepts matter for ResSim's current FIM convergence and linear-solver work.
- Goal: map those concepts onto concrete ResSim files and modules.
- Goal: rank the implementation opportunities by expected value for the current producer-corner / CPRW investigation.
- Non-goal: reproduce OPM's property system, Eclipse deck machinery, MPI runtime, network solver, NLDD, or multisegment-well stack.

## Minimal OPM Runtime Path

For the simplified black-oil FIM path, OPM Flow is best read as this runtime chain:

1. `flow/flow_blackoil.cpp`
2. `SimulatorFullyImplicitBlackoil`
3. `BlackoilModel::prepareStep()`
4. `BlackoilModel::nonlinearIteration()`
5. `BlackoilModel::initialLinearization()`
6. `BlackoilModel::assembleReservoir()`
7. `wellModel().linearize(...)`
8. `BlackoilModel::solveJacobianSystem(x)`
9. `wellModel().postSolve(x)`
10. `BlackoilModel::updateSolution(x)`

The important point is that the core algorithm is recognizable: assemble the coupled nonlinear system, solve the linearized system, recover well state, and apply a bounded Newton update.
The product-level complexity comes from everything wrapped around that core.

## Minimal OPM-To-ResSim Mapping

| OPM concept | OPM role | ResSim counterpart | Notes |
|-------------|----------|--------------------|-------|
| `SimulatorFullyImplicitBlackoil` + `FlowMain` | outer run loop, parameter registration, solver creation | `src/lib/ressim/src/fim/timestep.rs` and `src/lib/ressim/src/fim/newton.rs` | ResSim already has the same high-level accepted-substep and inner-Newton split, but in a much smaller runtime shell. |
| `BlackoilModel` (Flow nonlinear-system wrapper) | timestep prep, residual/Jacobian assembly, convergence checks, linear solve handoff, Newton bookkeeping | `src/lib/ressim/src/fim/assembly.rs`, `src/lib/ressim/src/fim/newton.rs`, `src/lib/ressim/src/fim/state.rs` | ResSim compresses responsibilities that OPM keeps in separate model and solver layers. |
| generic black-oil model + `FIBlackOilModel` | primary variables, intensive quantities, black-oil state interpretation, cached cell quantities | `src/lib/ressim/src/fim/state.rs` and `src/lib/ressim/src/fim/flash.rs` | Same conceptual layer exists already; OPM is broader mainly because it supports many optional modules. |
| `BlackOilNewtonMethod` + `NonlinearSolver` | bounded updates, phase-state adaptation, oscillation detection, relaxation control | `src/lib/ressim/src/fim/newton.rs` and `src/lib/ressim/src/fim/state.rs` | ResSim already has Appleyard damping, bounded candidates, frozen regimes, and a producer-hotspot bailout, but not OPM's broader residual-history-based stabilization. |
| `BlackoilWellModel` + `StandardWellEquations` | explicit reservoir-well block structure, well assembly, post-solve recovery, coarse-pressure well equations | `src/lib/ressim/src/fim/wells.rs` and well Jacobian assembly in `src/lib/ressim/src/fim/assembly.rs` | This is the clearest same-idea / thinner-implementation area in ResSim. |
| CPR / CPRW pressure transfer policy | pressure coarse solve with optional explicit well-pressure equations | `src/lib/ressim/src/fim/linear/mod.rs` and `src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs` | ResSim has a pressure-first coarse correction already, but wells are still treated as part of a generic scalar tail rather than explicit coarse pressure equations. |
| well post-solve recovery and local consistency logic | recover / limit well state after each linearized solve | `src/lib/ressim/src/fim/state.rs` | ResSim relaxes well state toward local consistency after the raw Newton update rather than solving a more explicit inner well block. |
| timestep growth / retry policy | accepted-step growth control and retry classification | `src/lib/ressim/src/fim/timestep.rs` | ResSim is already strong here; this is not the first place to borrow further from OPM. |

## What Is Actually More Complex In OPM

The parts of OPM that matter for ResSim are not uniformly more complex.

### More complex in a way that probably matters

1. Nonlinear globalization.
2. Primary-variable update discipline.
3. Explicit reservoir-well block handling.
4. CPRW coarse-pressure treatment with well equations included.

### More complex mostly because OPM is a full product

1. Type-tag / property-system layering.
2. Eclipse deck and restart handling.
3. MPI, domain decomposition, and large parallel-runtime scaffolding.
4. Network, group, and multisegment-well infrastructure.
5. Optional physics extensions such as solvent, polymer, energy, brine, biofilm, and ML-assisted initialization.

For ResSim's current FIM path, the second list is mostly noise.

## Ranked Implementation Opportunities

### 1. Explicit CPRW-style coarse pressure coupling

Expected value: highest.

Why it ranks first:

- ResSim already has a pressure-first coarse correction path.
- The remaining hard shelf is producer-local and well-driven often enough that a better reservoir-well pressure coarse solve is the most plausible structural gap.
- OPM's strongest practical difference from ResSim is not exotic Newton math; it is the combination of stronger globalization and explicit well-aware CPR.

ResSim files:

- `src/lib/ressim/src/fim/linear/mod.rs`
- `src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs`
- likely helper touch points in `src/lib/ressim/src/fim/assembly.rs` and `src/lib/ressim/src/fim/state.rs`

### 2. General nonlinear stabilization beyond the producer-hotspot rule

Expected value: high.

Why it ranks second:

- ResSim already has a tactical producer-hotspot matcher.
- OPM does not rely on single-site hard-coded bailout logic; it has broader oscillation and stagnation detection with adaptive relaxation.
- This is likely needed even if CPRW is improved.

ResSim files:

- `src/lib/ressim/src/fim/newton.rs`
- `src/lib/ressim/src/fim/state.rs`

Current experiment status as of 2026-04-06:

- First prototype landed in `src/lib/ressim/src/fim/newton.rs` as a residual-history adaptive damping cap for repeated near-flat nonlinear hotspot sites.
- The implemented rule groups repeated hotspot sites by coarse nonlinear location (`cell`, `well`, or `perforation`) instead of exact residual row identity and applies an extra damping cap only when:
  - the same hotspot site repeats,
  - residual progress is nearly flat, and
  - the solve is already in a near-converged residual band.
- Focused Rust coverage is green for the new helper path and the locked SPE1/FIM smoke gate remains green.
- Measured benchmark outcome is mixed:
  - canonical saved day-2 checkpoint improved from the rebuilt current-head baseline `246` to `196` accepted substeps
  - short day-1 `wf_p_12x12x3` summary regressed from the previously tracked `136`-substep regime to `165` accepted substeps
- Current verdict: promising as a nonlinear-stabilization direction, but not yet clean enough to call done because the checkpoint gain comes with a visible adjacent regression.

### 3. Refactor wells toward an explicit local block view

Expected value: medium-high.

Why it ranks third:

- A clearer `A / B / C / D` mental model will make both CPRW and Newton behavior easier to improve safely.
- It is probably a prerequisite for a clean CPRW implementation if the current scalar-tail abstraction becomes too opaque.

ResSim files:

- `src/lib/ressim/src/fim/wells.rs`
- `src/lib/ressim/src/fim/assembly.rs`

Current implementation status as of 2026-04-06:

- First structural slice is now implemented.
- `src/lib/ressim/src/fim/wells.rs` exposes explicit `FimWellLocalBlock` and `FimPerforationLocalBlock` views so the solver can talk about one physical well block as:
  - one BHP unknown / equation pair, plus
  - the owned perforation-rate unknown / equation pairs attached to that physical well.
- `src/lib/ressim/src/fim/assembly.rs` now assembles the well-coupled Jacobian terms through that local-block view instead of reconstructing the same row/column ownership ad hoc in each loop.
- Second structural slice is also now implemented:
  - the local-block views now expose the main well/perforation derivative and residual bookkeeping API directly
  - assembly now consumes methods on the local-block views for:
    - perforation component-rate derivatives
    - perforation target-rate derivatives
    - perforation connection BHP derivatives
    - perforation connection cell derivatives
    - well control state / slack lookups
- Third structural slice is also now implemented:
  - well-rate aggregation and target-BHP solving now live behind `FimWellLocalBlock`
  - `src/lib/ressim/src/fim/state.rs` now uses the block view for:
    - initial rate-controlled BHP solves
    - initial consistent perforation-rate population
    - well-state relaxation toward local consistency after Newton updates
  - well diagnostics now pull actual-well-rate and consistent-BHP-rate data through the well-block aggregation methods instead of rebuilding raw well/perforation loops inline
- Fourth structural slice is now implemented too:
  - the remaining well residual helpers now live on the block views directly:
    - `FimWellLocalBlock::control_slacks(...)`
    - `FimWellLocalBlock::constraint_residual(...)`
    - `FimPerforationLocalBlock::rate_residual(...)`
    - `FimPerforationLocalBlock::residual_diagnostics(...)`
  - production FIM call sites in `src/lib/ressim/src/fim/assembly.rs`, `src/lib/ressim/src/fim/scaling.rs`, and `src/lib/ressim/src/fim/newton.rs` now consume those block methods instead of raw helper functions
  - the old free-function entry points are retained only as test-compatibility shims, so the physical well/perforation block view is now the main home for both assembly-side and diagnostic-side well logic
- Focused Rust coverage is green for:
  - `local_block_exposes_bhp_and_perforation_offsets`
  - `local_block_perforation_control_cells_match_existing_control_stencil`
  - `local_block_derivative_helpers_match_free_functions`
  - `local_well_block_control_helpers_match_free_functions`
  - `local_well_block_rate_helpers_match_free_functions`
  - `local_block_residual_helpers_match_free_functions`
  - `rate_controlled_well_bhp_unknown_is_solved_implicitly`
  - `apply_newton_update_frozen_limits_well_overshoot_toward_local_consistency`
  - `spe1_fim_first_steps_converge_without_stall`
  - `spe1_fim_gas_injection_creates_free_gas`
- Rebuilt wasm replay stayed aligned with the current-head nonlinear prototype baseline:
  - `step=1 | time=1.2500d | history+=196 | substeps=196 | retries=0/31/0 | dt=[4.039e-4,2.253e-2] | growth=max-growth | retry_dom=nonlinear-bad:oil@430 | warning=none`
- Current verdict: useful structural cleanup. This does not change the physics or the benchmark outcome by itself, but it gives the well block a concrete home that should make later CPRW and well-Newton work less opaque.

### 4. Stronger primary-variable switching and update hysteresis

Expected value: medium.

Why it ranks fourth:

- It will improve robustness, but it is less directly tied to the present producer-corner shelf than CPRW and broader damping.
- ResSim's current frozen-regime update path is already a reasonable foundation.

ResSim files:

- `src/lib/ressim/src/fim/state.rs`
- `src/lib/ressim/src/fim/newton.rs`

### 5. Preserve hotspot-specific bailout logic as tactical support only

Expected value: situational.

Why it ranks fifth:

- It is useful for the current benchmark pathology.
- It should not become the main globalization mechanism.

ResSim files:

- `src/lib/ressim/src/fim/newton.rs`

## Concrete Plan For Item 1: Explicit CPRW-Style Coarse Pressure Coupling

This section is the actionable implementation plan for the highest-priority item.

## Objective

Upgrade ResSim's current pressure-first coarse correction so that well BHP unknowns participate explicitly in the coarse pressure system, closer to OPM's CPRW path, instead of being handled only as generic scalar-tail Schur content.

Success looks like this:

1. The coarse system contains both cell-pressure coarse rows and selected well-pressure rows.
2. Reservoir-to-well and well-to-reservoir pressure coupling are preserved explicitly in the coarse operator.
3. The updated CPR path improves the hard day-2 checkpoint behavior without regressing the locked Rust smoke cases.

## Current ResSim Baseline

Current relevant structure:

- `FimLinearBlockLayout` only distinguishes cell blocks and a scalar tail via `scalar_tail_start`.
- `build_block_jacobi_preconditioner()` builds a pressure coarse matrix from cell blocks and treats the tail with a generic Schur-style elimination.
- `pressure_tail_coupling` and `pressure_tail_prolongation` already prove that the implementation has enough information to couple non-cell unknowns to the coarse solve.
- What is missing is an explicit distinction between pressure-like well unknowns and transport / rate tail unknowns.

That means the implementation path should be evolutionary, not a rewrite.

## Proposed Minimal Design

Add a minimal notion of coarse pressure unknown kinds.

### Coarse unknowns to include

1. One coarse row per cell block, representing reservoir pressure.
2. One coarse row per physical-well BHP unknown.

### Coarse unknowns to exclude in the first slice

1. Perforation-rate unknowns.
2. Any non-pressure scalar unknown that is not clearly pressure-like.

This keeps the first CPRW slice aligned with OPM's useful idea without dragging transport-like tails directly into the coarse system.

## Required Structural Changes

### 1. Extend linear layout metadata

File:

- `src/lib/ressim/src/fim/linear/mod.rs`

Change:

- Extend `FimLinearBlockLayout` so the linear solver can distinguish:
  - cell block count / size
  - explicit well-pressure range
  - remaining scalar tail range

Minimal target shape:

- keep `cell_block_count`
- keep `cell_block_size`
- replace or supplement `scalar_tail_start` with enough metadata to separate:
  - cell unknowns
  - well BHP unknowns
  - remaining scalar tail unknowns

The key design requirement is that the solver no longer has to guess which tail entries are pressure-like.

### 2. Build a mixed cell-plus-well coarse operator

File:

- `src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs`

Change:

- Replace the current cell-only coarse operator build with a mixed operator:
  - top-left block: cell-pressure coarse rows
  - top-right block: cell-to-well coarse coupling
  - bottom-left block: well-to-cell coarse coupling
  - bottom-right block: well-pressure diagonal / coupling block

Implementation rule:

- Do not pull perforation-rate unknowns directly into the first coarse basis.
- Continue to Schur-eliminate or smooth the non-pressure tail after the pressure correction.

### 3. Split the current generic tail logic

File:

- `src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs`

Change:

- Replace one generic `tail_inverse` / `pressure_tail_coupling` worldview with:
  - explicit well-pressure coarse participation
  - residual generic-tail elimination only for non-pressure tail unknowns

This is the main conceptual change.
The old code assumes all tail unknowns are second-class from the coarse solve's perspective.
The new code should treat BHP as first-class pressure unknowns.

### 4. Preserve the current post-pressure smoother

Files:

- `src/lib/ressim/src/fim/linear/gmres_block_jacobi.rs`

Change:

- Keep the existing stage-one block smoother after the pressure correction.

Reason:

- Even with CPRW-style coarse pressure rows, transport and rate unknowns still need to respond to the pressure update.
- The first implementation should change only the coarse solve, not the full two-stage iteration structure.

## Implementation Phases

Current status as of 2026-04-06:

- Phase 1 is implemented in the codebase.
- `FimLinearBlockLayout` now exposes an explicit well-BHP range via `well_bhp_count` while preserving the old solver behavior through a legacy-tail helper.
- Phase 2 is now implemented in the linear backend: the CPR coarse system includes `cell-pressure + well-BHP` rows, while perforation-rate unknowns remain in the Schur-eliminated tail and both well plus perforation unknowns still participate in the stage-one scalar smoother.
- Focused validation after the Phase 1 change was green for:
  - `cargo test linear_block_layout_exposes_well_range_without_moving_legacy_tail_start -- --nocapture`
  - `cargo test producer_hotspot_stagnation -- --nocapture`
  - `cargo test spe1_fim_first_steps_converge_without_stall -- --nocapture`
- Focused validation after the Phase 2 change is also green for:
  - `cargo test cpr_coarse_operator_promotes_explicit_well_bhp_rows -- --nocapture`
  - `cargo test pressure_projection_updates_explicit_well_bhp_unknowns -- --nocapture`
  - `cargo test cpr_report_counts_cells_and_bhp_rows_without_perf_tail -- --nocapture`
  - `cargo test linear_block_layout_exposes_well_range_without_moving_legacy_tail_start -- --nocapture`
  - `cargo test producer_hotspot_stagnation -- --nocapture`
  - `cargo test spe1_fim_first_steps_converge_without_stall -- --nocapture`
- Canonical wasm replay after the Phase 2 change is neutral, not improved:
  - `step=1 | time=1.2500d | history+=246 | substeps=246 | retries=0/29/0 | dt=[4.039e-4,5.758e-3] | growth=max-growth | retry_dom=nonlinear-bad:oil@430 | warning=none`

### Phase 1: Layout split only

Goal:

- teach the linear backend where well-BHP unknowns live.

Tasks:

1. Extend `FimLinearBlockLayout`.
2. Update `run_fim_timestep()` to populate the new metadata.
3. Keep existing behavior unchanged by adapting the current code to the new layout without changing the coarse operator yet.

Exit criterion:

- all existing tests still pass with behavior unchanged.

Status:

- Completed on 2026-04-06.

### Phase 2: Explicit well-BHP coarse rows

Goal:

- build a coarse matrix over cell-pressure and well-BHP unknowns.

Tasks:

1. Build the mixed coarse row set.
2. Map residual restriction into that mixed coarse space.
3. Map prolongation back from the mixed coarse correction.
4. Leave perforation-rate unknowns in the post-pressure smoothing path.

Exit criterion:

- new focused unit tests verify that a pressure residual at a perforated cell produces a nonzero coarse well-pressure response and vice versa.

Status:

- Completed on 2026-04-06.
- Structural result: well BHP unknowns now participate explicitly in the CPR coarse system while perforation-rate unknowns remain in the eliminated tail.
- Measured result: the canonical wasm checkpoint remained at the current-head baseline (`246` accepted substeps, `0/29/0` retries), so this phase improved solver structure and test coverage but did not move the tracked day-2 shelf by itself.

### Phase 3: Checkpoint validation

Goal:

- prove improvement or revert quickly.

Tasks:

1. Rebuild wasm.
2. Replay `/tmp/fim-scan-wf12-stats/step-0001.json`.
3. Compare substeps, retry class split, growth limiter, and late-window trace shape to the documented `246` current-head baseline.

Exit criterion:

- measurable improvement on the canonical checkpoint without breaking the locked Rust regressions.

Status:

- Executed on 2026-04-06 after the Phase 2 linear change.
- Result: no measurable checkpoint improvement relative to the rebuilt current-head baseline; keep the structural CPRW change, but treat the day-2 shelf as still dominated by a different mechanism.

## Required Tests

### New unit coverage in the linear module

Add focused tests around `gmres_block_jacobi.rs` for:

1. coarse layout separates well-BHP unknowns from the generic scalar tail.
2. coarse pressure restriction includes explicit cell-to-well coupling.
3. coarse pressure prolongation writes a correction into both cell blocks and well-BHP entries.
4. non-pressure tail entries remain excluded from the coarse basis in the first CPRW slice.

### Existing regression gate to keep

1. `cargo test producer_hotspot_stagnation -- --nocapture`
2. `cargo test spe1_fim_first_steps_converge_without_stall -- --nocapture`
3. `cargo test spe1_fim_gas_injection_creates_free_gas -- --nocapture`

### Canonical acceptance target

1. `bash ./scripts/build-wasm.sh`
2. replay the saved day-2 checkpoint via `scripts/fim-wasm-diagnostic.mjs`

The wasm replay remains the real acceptance target for this slice.

## Risks And Guardrails

### Main risk

The coarse-space promotion can easily regress behavior if well-BHP rows are added with the wrong scaling or with too much transport contamination.

### Guardrails

1. Keep the first slice limited to explicit BHP pressure rows only.
2. Do not promote perforation-rate unknowns in the first slice.
3. Do not remove the post-pressure smoother.
4. Validate in wasm immediately after the code change instead of trusting native-only tests.

## Stop / Continue Rule

Stop and revert or redesign the slice if either happens:

1. the rebuilt checkpoint regresses materially relative to the documented `246` baseline.
2. locked Rust smoke tests fail or move into a clearly worse regime.

Continue into the next CPRW iteration only if:

1. the explicit BHP coarse rows are stable in tests, and
2. the wasm checkpoint improves enough to justify further refinement.

Current verdict:

- The explicit BHP coarse-row split is stable enough to keep.
- The canonical checkpoint did not improve, so the next solver iteration should not assume that missing explicit BHP coarse rows were the dominant remaining shelf cause.

## Bottom Line

The minimal OPM lesson worth copying first is not OPM's abstraction stack.
It is this:

- treat well BHP unknowns as pressure-like coarse unknowns,
- keep non-pressure tail variables out of the first coarse basis,
- preserve a second-stage smoother for the full coupled system.

That is the smallest believable CPRW-style upgrade path for ResSim's current FIM linear backend.