# FIM Migration Plan

This document is the concrete implementation checklist for replacing the current IMPES timestep path with a fully implicit black-oil FIM path in the Rust core.

It is intentionally direct-cutover oriented:

- No long-lived production IMPES/FIM split.
- Temporary migration scaffolding is acceptable only while the branch is incomplete.
- The final production timestep path should be one FIM path through the existing public simulator API.

## Target Numerical Architecture

### Nonlinear solve

- Damped Newton method.
- Explicit iterate state object separate from committed simulator arrays.
- Residual and update convergence checks both required for acceptance.
- Timestep cutback on Newton failure, line-search exhaustion, non-finite state, or repeated linear-solver failure.

### Linear solve

Do not keep the current direct-solve-first architecture as the long-term FIM design.

Target stack:

1. Primary production linear solver: restarted FGMRES.
2. Primary preconditioner: CPR-PF style block preconditioner.
3. Secondary preconditioner/fallback for small systems and Jacobian debugging: sparse LU.
4. Development fallback: ILU(0) or block ILU(0) if CPR is temporarily unavailable during migration.

Rationale:

- The fully coupled black-oil Jacobian will be larger, more nonsymmetric, and more poorly scaled than the current pressure-only matrix.
- Sparse LU is valuable for correctness validation and small grids, but it is not the right production backbone for browser-oriented 3D FIM.
- BiCGSTAB with diagonal Jacobi is too weak to be the primary FIM linear solver.

### Primary variables

Use natural black-oil variables with cellwise switching:

- Saturated cell: pressure, `sw`, `sg`
- Undersaturated cell: pressure, `sw`, `rs`

Derived values:

- `so = 1 - sw - sg` in saturated cells
- `sg = 0` and `so = 1 - sw` in undersaturated cells

This matches the current simulator state model better than a four-variable complementarity formulation, while remaining a true FIM scheme.

## File-By-File Checklist

### 1. `src/lib/ressim/src/fim/mod.rs`

Purpose:

- Top-level FIM entrypoints and shared exports.

Checklist:

- [ ] Add module declarations for `state`, `flash`, `assembly`, `newton`, `linear`, `wells`, and `scaling`.
- [ ] Export the production `run_fim_timestep()` entrypoint used by `step.rs`.

Proposed items:

```rust
pub(crate) mod assembly;
pub(crate) mod flash;
pub(crate) mod linear;
pub(crate) mod newton;
pub(crate) mod scaling;
pub(crate) mod state;
pub(crate) mod wells;

pub(crate) use newton::{run_fim_timestep, FimStepReport};
pub(crate) use state::{FimState, HydrocarbonState};
```

### 2. `src/lib/ressim/src/fim/state.rs`

Purpose:

- Own the Newton iterate state and regime classification.
- Prevent residual assembly from reading committed simulator arrays directly.

Checklist:

- [ ] Introduce the iterate-state container.
- [ ] Add constructors from committed simulator state.
- [ ] Add commit-back helpers once a Newton solve is accepted.
- [ ] Add boundedness helpers and simple scaling accessors.

Proposed types and signatures:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HydrocarbonState {
    Saturated,
    Undersaturated,
}

#[derive(Clone, Debug)]
pub(crate) struct FimCellState {
    pub pressure_bar: f64,
    pub sw: f64,
    pub hydrocarbon_var: f64,
    pub regime: HydrocarbonState,
}

#[derive(Clone, Debug)]
pub(crate) struct FimState {
    pub cells: Vec<FimCellState>,
}

impl FimState {
    pub(crate) fn from_simulator(sim: &ReservoirSimulator) -> Self;
    pub(crate) fn cell(&self, idx: usize) -> &FimCellState;
    pub(crate) fn cell_mut(&mut self, idx: usize) -> &mut FimCellState;
    pub(crate) fn n_unknowns(&self) -> usize;
    pub(crate) fn write_back_to_simulator(&self, sim: &mut ReservoirSimulator);
    pub(crate) fn classify_regimes(&mut self, sim: &ReservoirSimulator, flash: &FimFlashWorkspace);
    pub(crate) fn is_finite(&self) -> bool;
    pub(crate) fn respects_basic_bounds(&self, sim: &ReservoirSimulator) -> bool;
}

pub(crate) struct FimCellDerived {
    pub so: f64,
    pub sg: f64,
    pub rs: f64,
    pub bo: f64,
    pub bg: f64,
    pub bw: f64,
    pub mu_o: f64,
    pub mu_g: f64,
    pub mu_w: f64,
    pub rho_o: f64,
    pub rho_g: f64,
    pub rho_w: f64,
}

impl FimState {
    pub(crate) fn derive_cell(&self, sim: &ReservoirSimulator, idx: usize) -> FimCellDerived;
}
```

### 3. `src/lib/ressim/src/fim/flash.rs`

Purpose:

- Make saturated and undersaturated closure part of Newton residual evaluation.
- Replace post-transport gas split logic as a production mechanism.

Checklist:

- [ ] Move current gas split and bubble-point logic into a dedicated flash evaluator.
- [ ] Support regime classification and regime-consistent closures.
- [ ] Provide derivative-friendly local closures for residual assembly.

Proposed types and signatures:

```rust
pub(crate) struct FimFlashWorkspace {
    scratch: Vec<f64>,
}

impl FimFlashWorkspace {
    pub(crate) fn new(n_cells: usize) -> Self;
}

pub(crate) struct FimFlashResult {
    pub regime: HydrocarbonState,
    pub so: f64,
    pub sg: f64,
    pub rs: f64,
    pub bubble_point_bar: f64,
}

pub(crate) fn resolve_cell_flash(
    sim: &ReservoirSimulator,
    state: &FimState,
    cell_idx: usize,
) -> FimFlashResult;

pub(crate) fn classify_cell_regime(
    sim: &ReservoirSimulator,
    pressure_bar: f64,
    sw: f64,
    hydrocarbon_var: f64,
) -> HydrocarbonState;
```

### 4. `src/lib/ressim/src/fim/scaling.rs`

Purpose:

- Keep nonlinear equations and updates comparably scaled.
- Make Newton convergence criteria robust across pressure and component equations.

Checklist:

- [ ] Add per-cell equation scaling.
- [ ] Add per-variable update scaling.

Proposed types and signatures:

```rust
pub(crate) struct EquationScaling {
    pub water: Vec<f64>,
    pub oil_component: Vec<f64>,
    pub gas_component: Vec<f64>,
}

pub(crate) struct VariableScaling {
    pub pressure: Vec<f64>,
    pub sw: Vec<f64>,
    pub hydrocarbon_var: Vec<f64>,
}

pub(crate) fn build_equation_scaling(
    sim: &ReservoirSimulator,
    state: &FimState,
    dt_days: f64,
) -> EquationScaling;

pub(crate) fn build_variable_scaling(
    sim: &ReservoirSimulator,
    state: &FimState,
) -> VariableScaling;
```

### 5. `src/lib/ressim/src/fim/wells.rs`

Purpose:

- Make well equations and well-control active-set logic iterate-aware.
- Keep well control switching inside the nonlinear solve.

Checklist:

- [ ] Extract state-aware well evaluation out of the current pressure/transport split.
- [ ] Support active-set freezing during a Newton iteration.
- [ ] Assemble well residual/Jacobian contributions.

Proposed types and signatures:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WellControlMode {
    Disabled,
    Rate,
    Bhp,
}

#[derive(Clone, Debug)]
pub(crate) struct FimResolvedWellControl {
    pub mode: WellControlMode,
    pub target: f64,
    pub bhp_limited: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct FimWellContributions {
    pub residual_triplets: Vec<(usize, f64)>,
    pub jacobian_triplets: Vec<(usize, usize, f64)>,
}

pub(crate) fn resolve_well_controls_for_iterate(
    sim: &ReservoirSimulator,
    state: &FimState,
    previous: Option<&[FimResolvedWellControl]>,
) -> Vec<FimResolvedWellControl>;

pub(crate) fn assemble_well_contributions(
    sim: &ReservoirSimulator,
    state: &FimState,
    controls: &[FimResolvedWellControl],
    dt_days: f64,
) -> FimWellContributions;
```

### 6. `src/lib/ressim/src/fim/assembly.rs`

Purpose:

- Assemble the full FIM residual and Jacobian.
- Own cell-cell flux coupling, accumulation terms, gravity, capillary pressure, and component balances.

Checklist:

- [ ] Add coupled residual assembly for water, oil component, and gas component equations.
- [ ] Add sparse Jacobian assembly with consistent block ordering.
- [ ] Add explicit hooks for well contributions and equation scaling.

Recommended unknown ordering:

- Cell-major, 3 unknowns per cell.
- Unknown layout: `[pressure, sw, hydrocarbon_var]`.

Recommended equation ordering:

- Cell-major, 3 equations per cell.
- Equation layout: `[water_balance, oil_component_balance, gas_component_balance]`.

Proposed types and signatures:

```rust
use sprs::CsMat;
use nalgebra::DVector;

pub(crate) struct FimAssembly {
    pub residual: DVector<f64>,
    pub jacobian: CsMat<f64>,
    pub equation_scaling: EquationScaling,
    pub variable_scaling: VariableScaling,
}

pub(crate) struct FimAssemblyOptions {
    pub dt_days: f64,
    pub include_wells: bool,
}

pub(crate) fn assemble_fim_system(
    sim: &ReservoirSimulator,
    state: &FimState,
    controls: &[FimResolvedWellControl],
    options: &FimAssemblyOptions,
) -> FimAssembly;

pub(crate) fn unknown_offset(cell_idx: usize, local_var: usize) -> usize;
pub(crate) fn equation_offset(cell_idx: usize, local_eq: usize) -> usize;
```

### 7. `src/lib/ressim/src/fim/linear/mod.rs`

Purpose:

- Define the FIM linear-solver abstraction.
- Separate FIM linear algebra choices from the current pressure-only solver path.

Checklist:

- [ ] Introduce a FIM-only linear solver interface.
- [ ] Make FGMRES+CPR the target production path.
- [ ] Keep sparse LU as a debug and small-system backend.

Proposed types and signatures:

```rust
use nalgebra::DVector;
use sprs::CsMat;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FimLinearSolverKind {
    FgmresCpr,
    GmresIlu0,
    SparseLuDebug,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FimLinearSolveOptions {
    pub kind: FimLinearSolverKind,
    pub restart: usize,
    pub max_iterations: usize,
    pub relative_tolerance: f64,
    pub absolute_tolerance: f64,
}

pub(crate) struct FimLinearSolveReport {
    pub solution: DVector<f64>,
    pub converged: bool,
    pub iterations: usize,
    pub final_residual_norm: f64,
    pub used_fallback: bool,
}

pub(crate) fn solve_linearized_system(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
) -> FimLinearSolveReport;
```

### 8. `src/lib/ressim/src/fim/linear/gmres.rs`

Purpose:

- Implement restarted GMRES or FGMRES for nonsymmetric coupled Jacobians.
- Allow variable preconditioners for CPR.

Checklist:

- [ ] Implement restarted FGMRES.
- [ ] Support left preconditioning and residual reporting.

Proposed signatures:

```rust
pub(crate) trait LeftPreconditioner {
    fn apply(&self, rhs: &DVector<f64>) -> DVector<f64>;
}

pub(crate) fn solve_fgmres(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    preconditioner: &dyn LeftPreconditioner,
    options: &FimLinearSolveOptions,
) -> FimLinearSolveReport;
```

### 9. `src/lib/ressim/src/fim/linear/cpr.rs`

Purpose:

- Implement a CPR-style preconditioner specialized for black-oil FIM.

Checklist:

- [ ] Extract pressure block approximation from the full Jacobian.
- [ ] Add pressure-stage solve.
- [ ] Add global smoothing stage.
- [ ] Make the interface compatible with FGMRES.

Recommended scope for first implementation:

- CPR-PF variant.
- Pressure restriction from the full 3x3 cell block system.
- Pressure-stage solve via sparse LU initially, replaceable later.
- Global smoother via ILU(0) or block ILU(0).

Proposed types and signatures:

```rust
pub(crate) struct CprPreconditioner {
    pressure_solver: PressureBlockSolver,
    smoother: GlobalSmoother,
}

pub(crate) enum PressureBlockSolver {
    SparseLu,
}

pub(crate) enum GlobalSmoother {
    Ilu0,
    BlockIlu0,
}

impl CprPreconditioner {
    pub(crate) fn new(jacobian: &CsMat<f64>, n_cells: usize) -> Self;
}

impl LeftPreconditioner for CprPreconditioner {
    fn apply(&self, rhs: &DVector<f64>) -> DVector<f64>;
}
```

### 10. `src/lib/ressim/src/fim/linear/ilu.rs`

Purpose:

- Implement ILU(0) or block ILU(0) smoothing for FIM linear solves.

Checklist:

- [ ] Add scalar ILU(0).
- [ ] Add block ILU(0) for 3x3 cell blocks if scalar ILU is too weak.

Proposed signatures:

```rust
pub(crate) struct Ilu0Factorization {
    // storage details intentionally hidden
}

impl Ilu0Factorization {
    pub(crate) fn factorize(matrix: &CsMat<f64>) -> Result<Self, String>;
    pub(crate) fn solve(&self, rhs: &DVector<f64>) -> DVector<f64>;
}
```

### 11. `src/lib/ressim/src/fim/newton.rs`

Purpose:

- Own the nonlinear solve loop, damping, cutback decisions, and accepted-step report.

Checklist:

- [ ] Add Newton iteration loop.
- [ ] Add line search.
- [ ] Reclassify regimes and controls after accepted updates.
- [ ] Report nonlinear and linear iteration counts for diagnostics.

Proposed types and signatures:

```rust
pub(crate) struct FimNewtonOptions {
    pub max_newton_iterations: usize,
    pub residual_tolerance: f64,
    pub update_tolerance: f64,
    pub min_damping: f64,
    pub linear: FimLinearSolveOptions,
}

pub(crate) struct FimStepReport {
    pub accepted_state: FimState,
    pub converged: bool,
    pub newton_iterations: usize,
    pub final_residual_inf_norm: f64,
    pub final_update_inf_norm: f64,
    pub last_linear_report: Option<FimLinearSolveReport>,
    pub cutback_factor: f64,
}

pub(crate) fn run_fim_timestep(
    sim: &ReservoirSimulator,
    initial_state: &FimState,
    dt_days: f64,
    options: &FimNewtonOptions,
) -> FimStepReport;
```

### 12. `src/lib/ressim/src/step.rs`

Purpose after migration:

- Timestep orchestration only.

Checklist:

- [ ] Remove pressure-only retry logic.
- [ ] Call `run_fim_timestep()`.
- [ ] Commit accepted state.
- [ ] Own timestep cutback and final warning text only.

Proposed production shape:

```rust
impl ReservoirSimulator {
    pub(crate) fn step_internal(&mut self, target_dt_days: f64) {
        // timestep loop only
    }

    fn attempt_fim_substep(&self, dt_days: f64) -> FimStepReport;
    fn commit_fim_step(&mut self, report: &FimStepReport, dt_days: f64);
}
```

### 13. `src/lib/ressim/src/pvt.rs`

Checklist:

- [ ] Add state-aware helpers that accept explicit pressure and `rs` instead of reading simulator arrays implicitly.
- [ ] Add derivative helpers needed by Jacobian assembly.

Proposed signatures:

```rust
impl PvtTable {
    pub(crate) fn oil_props_at(&self, pressure_bar: f64, rs_sm3_sm3: f64) -> OilProps;
    pub(crate) fn gas_props_at(&self, pressure_bar: f64) -> GasProps;
    pub(crate) fn d_bo_d_p(&self, pressure_bar: f64, rs_sm3_sm3: f64) -> f64;
    pub(crate) fn d_rs_sat_d_p(&self, pressure_bar: f64) -> f64;
}
```

### 14. `src/lib/ressim/src/mobility.rs`

Checklist:

- [ ] Add iterate-state mobility evaluation.
- [ ] Add derivative-ready helpers or at least consistent finite-difference entrypoints.

Proposed signatures:

```rust
pub(crate) struct PhaseMobilities {
    pub water: f64,
    pub oil: f64,
    pub gas: f64,
}

pub(crate) fn phase_mobilities_from_state(
    sim: &ReservoirSimulator,
    state: &FimState,
    cell_idx: usize,
) -> PhaseMobilities;
```

### 15. `src/lib/ressim/src/well_control.rs`

Checklist:

- [ ] Add iterate-aware control resolution.
- [ ] Stop assuming control decisions are evaluated only on committed pressure arrays.
- [ ] Keep public reporting semantics unchanged after convergence.

Proposed signatures:

```rust
impl ReservoirSimulator {
    pub(crate) fn resolve_well_control_for_fim_state(
        &self,
        well: &Well,
        state: &FimState,
    ) -> FimResolvedWellControl;
}
```

### 16. `src/lib/ressim/src/reporting.rs`

Checklist:

- [ ] Rebuild timestep reporting from converged FIM state and controls.
- [ ] Remove dependence on explicit transport deltas as production inputs.

Proposed signatures:

```rust
pub(crate) fn build_fim_timestep_report(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    accepted_state: &FimState,
    controls: &[FimResolvedWellControl],
    dt_days: f64,
) -> TimePointRates;
```

### 17. `src/lib/ressim/src/pressure_eqn.rs`

Checklist:

- [ ] Demote from production timestep path.
- [ ] Keep temporarily only if useful for reference tests during migration.
- [ ] Delete or archive after FIM cutover is validated.

### 18. `src/lib/ressim/src/transport.rs`

Checklist:

- [ ] Demote from production timestep path.
- [ ] Keep temporarily only as reference behavior while migration tests are being built.
- [ ] Delete or archive after FIM cutover is validated.

## Execution Order

1. Build `fim/state.rs`, `fim/flash.rs`, and state-aware PVT/mobility helpers.
2. Build `fim/assembly.rs` for closed systems with no wells.
3. Add `fim/linear/` with FGMRES, ILU(0), and CPR scaffolding.
4. Add `fim/newton.rs` and converge simple closed-system cases.
5. Add `fim/wells.rs` and iterate-aware well control integration.
6. Rewrite `step.rs` to call the FIM path and commit accepted states.
7. Rewire `reporting.rs` to converged FIM states.
8. Remove production dependence on `pressure_eqn.rs` and `transport.rs`.

## Completion Gates

- Existing Buckley-Leverett, PVT, gas-balance, well-control, reporting, and boundedness regressions pass through the public step API on the FIM-only path.
- New Jacobian consistency tests exist for 1-cell saturated and undersaturated states.
- New Newton convergence tests exist for closed 1-cell and 2-cell cases and for well-controlled cases.
- No production timestep path remains that applies explicit transport after a pressure solve.

## Design Rules During Migration

- Residual assembly may not read committed simulator arrays directly once FIM assembly exists; it must read only from `FimState` plus immutable simulator properties.
- Gas flash and phase-regime logic must occur inside Newton, not as a post-step repair.
- Well control changes must be tracked as an active-set event and force another Newton iteration before acceptance.
- Sparse LU is a debug and small-system backend for the new architecture, not the target production FIM solver.