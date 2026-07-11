use nalgebra::DVector;
use sprs::CsMat;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod capture;
mod dense_lu_debug;
mod gmres_block_jacobi;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod solver_lab;
mod sparse_lu_debug;
mod well_schur;

const DIRECT_SOLVE_ROW_THRESHOLD: usize = 512;
const WASM_DIRECT_SOLVE_ROW_THRESHOLD: usize = DIRECT_SOLVE_ROW_THRESHOLD;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FimLinearSolverKind {
    FgmresCpr,
    GmresIlu0,
    DenseLuDebug,
    SparseLuDebug,
}

impl FimLinearSolverKind {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::FgmresCpr => "fgmres-cpr",
            Self::GmresIlu0 => "gmres-ilu0",
            Self::DenseLuDebug => "dense-lu",
            Self::SparseLuDebug => "sparse-lu",
        }
    }
}

pub(crate) const fn active_direct_solve_row_threshold() -> usize {
    #[cfg(target_arch = "wasm32")]
    {
        WASM_DIRECT_SOLVE_ROW_THRESHOLD
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        DIRECT_SOLVE_ROW_THRESHOLD
    }
}

const fn direct_solve_row_threshold_for_target(is_wasm: bool) -> usize {
    if is_wasm {
        WASM_DIRECT_SOLVE_ROW_THRESHOLD
    } else {
        DIRECT_SOLVE_ROW_THRESHOLD
    }
}

fn should_force_direct_solve(
    requested_kind: FimLinearSolverKind,
    row_count: usize,
    is_wasm: bool,
) -> bool {
    if is_wasm {
        requested_kind != FimLinearSolverKind::SparseLuDebug
            && row_count <= direct_solve_row_threshold_for_target(true)
    } else {
        requested_kind == FimLinearSolverKind::FgmresCpr
            && row_count <= direct_solve_row_threshold_for_target(false)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FimPressureCoarseSolverKind {
    ExactDense,
    BiCgStab,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FimLinearFailureReason {
    MaxIterations,
    ArnoldiBreakdown,
    RestartStagnation,
    DeadStateDetected,
}

impl FimLinearFailureReason {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::MaxIterations => "max-iters",
            Self::ArnoldiBreakdown => "arnoldi-breakdown",
            Self::RestartStagnation => "restart-stagnation",
            Self::DeadStateDetected => "dead-state",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FimLinearRestartDiagnostics {
    pub(crate) restart_index: usize,
    pub(crate) start_iteration: usize,
    pub(crate) end_iteration: usize,
    pub(crate) inner_steps: usize,
    pub(crate) outer_residual_norm: f64,
    pub(crate) preconditioned_residual_norm: f64,
    pub(crate) best_estimated_residual_norm: Option<f64>,
    pub(crate) best_candidate_residual_norm: Option<f64>,
    pub(crate) solution_improved: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FimLinearFailureDiagnostics {
    pub(crate) reason: FimLinearFailureReason,
    pub(crate) tolerance: f64,
    pub(crate) rhs_norm: f64,
    pub(crate) outer_residual_norm: f64,
    pub(crate) preconditioned_residual_norm: Option<f64>,
    pub(crate) estimated_residual_norm: Option<f64>,
    pub(crate) candidate_residual_norm: Option<f64>,
    pub(crate) restart_diagnostics: Vec<FimLinearRestartDiagnostics>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimLinearSolveOptions {
    pub(crate) kind: FimLinearSolverKind,
    pub(crate) restart: usize,
    pub(crate) max_iterations: usize,
    pub(crate) relative_tolerance: f64,
    pub(crate) absolute_tolerance: f64,
    /// Phase 11 (`FIM-LINEAR-010`): Schur-eliminate well-BHP and perforation-rate unknowns from
    /// the linear system before the iterative CPR/GMRES solve, matching OPM's `StandardWell`
    /// architecture (well block eliminated every Newton iteration, recovered after the reservoir
    /// solve) rather than iterating them as ordinary global unknowns. Off by default pending
    /// offline-lab validation (`solve_with_well_elimination`, `fim/linear/well_schur.rs`).
    pub(crate) eliminate_wells: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FimLinearBlockLayout {
    pub(crate) cell_block_count: usize,
    pub(crate) cell_block_size: usize,
    pub(crate) well_bhp_count: usize,
    pub(crate) perforation_tail_start: usize,
}

impl FimLinearBlockLayout {
    pub(crate) const fn cell_unknown_count(self) -> usize {
        self.cell_block_count * self.cell_block_size
    }

    pub(crate) const fn well_bhp_start(self) -> usize {
        self.cell_unknown_count()
    }

    pub(crate) const fn well_bhp_end(self) -> usize {
        self.well_bhp_start() + self.well_bhp_count
    }

    pub(crate) const fn coarse_pressure_unknown_count(self) -> usize {
        self.cell_block_count + self.well_bhp_count
    }

    pub(crate) const fn coarse_pressure_end(self) -> usize {
        self.perforation_tail_start
    }

    pub(crate) const fn noncell_start(self) -> usize {
        self.well_bhp_start()
    }
}

impl Default for FimLinearSolveOptions {
    fn default() -> Self {
        // Phase 10 (`FIM-LINEAR-008`): OPM's actual shipped `cprw` recipe pairs a loose
        // linear tolerance (`0.005` relative reduction) with a small iteration budget
        // (`maxiter: 20`). ResSim's linear solve always starts from x_0=0, so r_0=rhs
        // exactly and OPM's relative-reduction target translates exactly to
        // `relative_tolerance = 5e-3` here. Re-applied after a first live attempt
        // regressed the heavy case (Newton-side mechanisms weren't yet reconciled to the
        // new linear-solve noise level, Step 10.1) — see `docs/FIM_CONVERGENCE_WORKLOG.md`
        // "Phase 10" for the re-applied investigation.
        Self {
            kind: FimLinearSolverKind::FgmresCpr,
            restart: 30,
            max_iterations: 20,
            relative_tolerance: 5e-3,
            absolute_tolerance: 1e-12,
            // Phase 11 (`FIM-LINEAR-010`): offline lab on 35 real captured heavy-case systems
            // showed a decisive win (34/35 -> 35/35 converged, mean linear iterations 3.9 -> 1.1)
            // — promoted to default pending the live control-matrix gate.
            eliminate_wells: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FimLinearSolveReport {
    pub(crate) solution: DVector<f64>,
    pub(crate) converged: bool,
    pub(crate) iterations: usize,
    pub(crate) final_residual_norm: f64,
    pub(crate) failure_diagnostics: Option<FimLinearFailureDiagnostics>,
    pub(crate) used_fallback: bool,
    pub(crate) backend_used: FimLinearSolverKind,
    pub(crate) cpr_diagnostics: Option<FimCprDiagnostics>,
    pub(crate) total_time_ms: f64,
    pub(crate) preconditioner_build_time_ms: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FimCprDiagnostics {
    pub(crate) coarse_rows: usize,
    pub(crate) coarse_solver: FimPressureCoarseSolverKind,
    pub(crate) smoother_label: &'static str,
    pub(crate) coarse_applications: usize,
    pub(crate) average_reduction_ratio: f64,
    pub(crate) last_reduction_ratio: f64,
    /// Bundle P (`FIM-BUNDLE-P`) P0.1: per-phase preconditioner build-cost breakdown, filled in
    /// by `gmres_block_jacobi::solve_with_cpr_fine_smoother` after the preconditioner is built.
    pub(crate) build_timing: Option<gmres_block_jacobi::CprBuildTiming>,
}

pub(crate) fn solve_linearized_system(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
    layout: Option<FimLinearBlockLayout>,
    equation_scaling: Option<&crate::fim::scaling::EquationScaling>,
) -> FimLinearSolveReport {
    #[cfg(not(target_arch = "wasm32"))]
    if should_force_direct_solve(options.kind, jacobian.rows(), false) {
        return sparse_lu_debug::solve(jacobian, rhs, options, false);
    }

    #[cfg(target_arch = "wasm32")]
    if should_force_direct_solve(options.kind, jacobian.rows(), true) {
        return dense_lu_debug::solve(jacobian, rhs, options, false);
    }

    // Phase 11 (`FIM-LINEAR-010`): eliminate well/perforation unknowns before the iterative
    // solve, matching OPM's `StandardWell` architecture. Only applies to the iterative backends
    // (direct solves are already exact, no oscillation-avoidance value); only fires when the
    // layout actually has a well/perforation tail to eliminate, so the recursive call this makes
    // back into `solve_linearized_system` for the reduced (tail-free) system naturally falls
    // through to the normal dispatch below without re-entering this branch.
    if options.eliminate_wells
        && matches!(
            options.kind,
            FimLinearSolverKind::FgmresCpr | FimLinearSolverKind::GmresIlu0
        )
        && layout
            .is_some_and(|l| l.well_bhp_count > 0 || l.perforation_tail_start < jacobian.rows())
    {
        return well_schur::solve_with_well_elimination(
            jacobian,
            rhs,
            options,
            layout.expect("checked above"),
            equation_scaling,
        );
    }

    match options.kind {
        FimLinearSolverKind::DenseLuDebug => dense_lu_debug::solve(jacobian, rhs, options, false),
        FimLinearSolverKind::SparseLuDebug => sparse_lu_debug::solve(jacobian, rhs, options, false),
        FimLinearSolverKind::GmresIlu0 => {
            gmres_block_jacobi::solve(jacobian, rhs, options, layout, false, equation_scaling)
        }
        // CPR is still incomplete, but the default FIM path now uses a pressure-first
        // two-stage iterative backend instead of falling straight back to sparse LU.
        FimLinearSolverKind::FgmresCpr => {
            gmres_block_jacobi::solve(jacobian, rhs, options, layout, false, equation_scaling)
        }
    }
}

#[cfg(test)]
mod tests {
    use nalgebra::DVector;
    use sprs::TriMatI;

    use super::*;

    #[test]
    fn default_fim_linear_solver_targets_fgmres_cpr() {
        assert_eq!(
            FimLinearSolveOptions::default().kind,
            FimLinearSolverKind::FgmresCpr
        );
    }

    #[test]
    fn gmsres_ilu0_backend_solves_simple_system_iteratively() {
        let mut tri = TriMatI::<f64, usize>::new((2, 2));
        tri.add_triplet(0, 0, 2.0);
        tri.add_triplet(1, 1, 3.0);
        let jacobian = tri.to_csr();
        let rhs = DVector::from_vec(vec![4.0, 9.0]);

        let report = solve_linearized_system(
            &jacobian,
            &rhs,
            &FimLinearSolveOptions {
                kind: FimLinearSolverKind::GmresIlu0,
                ..FimLinearSolveOptions::default()
            },
            None,
            None,
        );

        assert!(report.converged);
        assert!(!report.used_fallback);
        assert_eq!(report.backend_used, FimLinearSolverKind::GmresIlu0);
        assert!((report.solution[0] - 2.0).abs() < 1e-12);
        assert!((report.solution[1] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn default_fim_solver_uses_iterative_fallback_before_sparse_lu() {
        let mut tri = TriMatI::<f64, usize>::new((2, 2));
        tri.add_triplet(0, 0, 2.0);
        tri.add_triplet(1, 1, 3.0);
        let jacobian = tri.to_csr();
        let rhs = DVector::from_vec(vec![4.0, 9.0]);

        let report = solve_linearized_system(
            &jacobian,
            &rhs,
            &FimLinearSolveOptions::default(),
            None,
            None,
        );

        assert!(report.converged);
        assert!(!report.used_fallback);
        assert_eq!(report.backend_used, FimLinearSolverKind::SparseLuDebug);
    }

    #[test]
    fn large_default_fim_system_still_uses_iterative_backend() {
        let n = DIRECT_SOLVE_ROW_THRESHOLD + 1;
        let mut tri = TriMatI::<f64, usize>::new((n, n));
        for idx in 0..n {
            tri.add_triplet(idx, idx, 2.0);
        }
        let jacobian = tri.to_csr();
        let rhs = DVector::from_element(n, 1.0);

        let report = solve_linearized_system(
            &jacobian,
            &rhs,
            &FimLinearSolveOptions::default(),
            None,
            None,
        );

        assert!(report.converged);
        assert!(!report.used_fallback);
        assert_eq!(report.backend_used, FimLinearSolverKind::FgmresCpr);
    }

    #[test]
    fn wasm_target_hands_off_direct_backend_above_512_rows() {
        assert_eq!(direct_solve_row_threshold_for_target(true), 512);
        assert!(should_force_direct_solve(
            FimLinearSolverKind::FgmresCpr,
            512,
            true,
        ));
        assert!(!should_force_direct_solve(
            FimLinearSolverKind::FgmresCpr,
            513,
            true,
        ));
    }

    #[test]
    fn wasm_target_still_respects_explicit_sparse_lu_choice() {
        assert!(!should_force_direct_solve(
            FimLinearSolverKind::SparseLuDebug,
            32,
            true,
        ));
    }

    #[test]
    fn linear_block_layout_exposes_explicit_cprw_ranges() {
        let layout = FimLinearBlockLayout {
            cell_block_count: 2,
            cell_block_size: 3,
            well_bhp_count: 2,
            perforation_tail_start: 8,
        };

        assert_eq!(layout.cell_unknown_count(), 6);
        assert_eq!(layout.well_bhp_start(), 6);
        assert_eq!(layout.well_bhp_end(), 8);
        assert_eq!(layout.noncell_start(), 6);
        assert_eq!(layout.coarse_pressure_unknown_count(), 4);
        assert_eq!(layout.coarse_pressure_end(), 8);
        assert_eq!(layout.perforation_tail_start, 8);
    }
}
