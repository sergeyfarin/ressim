use nalgebra::DVector;
use sprs::CsMat;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod capture;
mod dense_lu_debug;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod flow_lifecycle;
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
    /// Y2d5: use the mathematically valid right-preconditioned flexible-GMRES recurrence for
    /// `FgmresCpr`. Default false retains the historical fixed-left recurrence for controlled
    /// A/B validation; this option does not alter any CPR component or nonlinear policy.
    pub(crate) use_true_fgmres: bool,
    /// Y2d6d: use the complete source-pinned Flow lifecycle (true-IMPES weights, StandardWell
    /// matrix-free Schur action, one-level CPRW, block ILU0 post-smoother, BiCGSTAB). Native-only
    /// diagnostic option; false preserves production dispatch exactly.
    pub(crate) use_flow_lifecycle: bool,
    /// Phase 11 (`FIM-LINEAR-010`): Schur-eliminate well-BHP and perforation-rate unknowns from
    /// the linear system before the iterative CPR/GMRES solve, matching OPM's `StandardWell`
    /// architecture (well block eliminated every Newton iteration, recovered after the reservoir
    /// solve) rather than iterating them as ordinary global unknowns. Off by default pending
    /// offline-lab validation (`solve_with_well_elimination`, `fim/linear/well_schur.rs`).
    pub(crate) eliminate_wells: bool,
    /// WATER-009: accept CPR only after the raw residual of the assembled system clears the
    /// numeric tolerance. Default false preserves the historical preconditioned-residual route;
    /// this is an offline/default-off OPM-alignment probe, not a live Newton-policy change.
    pub(crate) require_raw_full_residual_acceptance: bool,
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

    /// Matrix column of `local_var` within `cell_idx`'s block.
    ///
    /// `FIM-REPAIR-F7` (#22): the index accessors the compositional plan's `EquationLayout`
    /// consumes. They exist so a caller can ask the layout where a unknown/equation lives, and
    /// what a given row or column *means*, instead of open-coding `cell_idx * 3 + local_var` and
    /// recovering meaning from `% 3`. Same arithmetic, named once.
    pub(crate) const fn cell_unknown(self, cell_idx: usize, local_var: usize) -> usize {
        cell_idx * self.cell_block_size + local_var
    }

    /// Matrix row of `local_eq` within `cell_idx`'s block.
    pub(crate) const fn cell_equation(self, cell_idx: usize, local_eq: usize) -> usize {
        cell_idx * self.cell_block_size + local_eq
    }

    /// Splits a matrix column into `(cell_idx, local_var)`, or `None` when it addresses the
    /// well-BHP / perforation tail rather than a cell block.
    pub(crate) const fn split_cell_unknown(self, unknown_idx: usize) -> Option<(usize, usize)> {
        if unknown_idx >= self.cell_unknown_count() {
            return None;
        }
        Some((
            unknown_idx / self.cell_block_size,
            unknown_idx % self.cell_block_size,
        ))
    }

    /// Splits a matrix row into `(cell_idx, local_eq)`, or `None` for a tail row.
    pub(crate) const fn split_cell_equation(self, equation_idx: usize) -> Option<(usize, usize)> {
        self.split_cell_unknown(equation_idx)
    }

    /// True when `column` is a cell's pressure column — the column the CPR coarse system is
    /// restricted onto. Answers "what does this column mean?" without an `index % 3` at the call
    /// site.
    pub(crate) fn is_cell_pressure_column(self, column: usize) -> bool {
        matches!(
            self.split_cell_unknown(column),
            Some((_, local_var)) if local_var == crate::fim::layout::CellPrimary::Pressure.local_index()
        )
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
        // `relative_tolerance = 5e-3` here. WATER-008 subsequently established that the
        // matching number is not yet a matching contract: Flow's Dune outer solver tests raw
        // residual reduction, while this historical CPR path can accept a re-applied
        // preconditioned residual. Keep the numeric default pending a separately gated
        // stopping-norm alignment. Re-applied after a first live attempt regressed the heavy
        // case (Newton-side mechanisms weren't yet reconciled to the new linear-solve noise
        // level, Step 10.1) — see `docs/FIM_CONVERGENCE_WORKLOG.md` "Phase 10".
        Self {
            kind: FimLinearSolverKind::FgmresCpr,
            restart: 30,
            max_iterations: 20,
            relative_tolerance: 5e-3,
            absolute_tolerance: 1e-12,
            use_true_fgmres: false,
            use_flow_lifecycle: false,
            // Phase 11 (`FIM-LINEAR-010`): offline lab on 35 real captured heavy-case systems
            // showed a decisive win (34/35 -> 35/35 converged, mean linear iterations 3.9 -> 1.1)
            // — promoted to default pending the live control-matrix gate.
            eliminate_wells: true,
            require_raw_full_residual_acceptance: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FimLinearSolveReport {
    pub(crate) solution: DVector<f64>,
    pub(crate) converged: bool,
    pub(crate) iterations: usize,
    /// Norm of the RHS for the same system represented by `final_residual_norm`.
    ///
    /// Direct and iterative backends must populate this even when they do not have a
    /// backend-specific failure payload. `well_schur` replaces the reduced-system value with the
    /// original full-system RHS norm after recovering the eliminated tail.
    pub(crate) rhs_norm: f64,
    pub(crate) final_residual_norm: f64,
    pub(crate) failure_diagnostics: Option<FimLinearFailureDiagnostics>,
    pub(crate) used_fallback: bool,
    pub(crate) backend_used: FimLinearSolverKind,
    pub(crate) cpr_diagnostics: Option<FimCprDiagnostics>,
    pub(crate) total_time_ms: f64,
    pub(crate) preconditioner_build_time_ms: f64,
}

impl FimLinearSolveReport {
    /// Backend-neutral residual reduction for the returned correction.
    pub(crate) fn reduction(&self) -> f64 {
        self.final_residual_norm / self.rhs_norm.max(1e-30)
    }
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
    solve_linearized_system_with_routing(jacobian, rhs, options, layout, equation_scaling, true)
}

fn solve_linearized_system_with_routing(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
    layout: Option<FimLinearBlockLayout>,
    equation_scaling: Option<&crate::fim::scaling::EquationScaling>,
    allow_forced_direct: bool,
) -> FimLinearSolveReport {
    // Small systems solve directly. If the direct factorization hits a singular Jacobian (a
    // cell driven onto a zero relperm-derivative endpoint by the raw-saturation path can make a
    // block rank-deficient), fall back to the requested iterative CPR backend, which does not
    // require an exact factorization and lets the Newton iteration proceed instead of collapsing
    // the timestep. The recursive fallback call passes `allow_forced_direct = false` so it lands
    // on the iterative dispatch below instead of re-selecting the forced direct path. Preserve
    // CPR when it was requested: downgrading a singular direct solve to plain GMRES/ILU0 discards
    // the pressure coarse correction and causes avoidable retry storms in capillary waterfloods.
    // On wasm,
    // `should_force_direct_solve` is true for any non-`SparseLuDebug` kind on a small system, so
    // without that guard the iterative re-entry would recurse into dense LU until stack overflow.
    //
    // This fallback is load-bearing, not generic defensive code: it is what keeps the OpmAligned
    // default (WATER-026, which rests on WATER-025 raw saturations) converging on small
    // well-dominated cases. Do not remove it without first landing the root-cause relperm-endpoint
    // regularization tracked in TODO.md ("ROOT-CAUSE FIX (deferred): relperm-endpoint singularity
    // under raw saturations").
    #[cfg(not(target_arch = "wasm32"))]
    if allow_forced_direct && should_force_direct_solve(options.kind, jacobian.rows(), false) {
        let direct = sparse_lu_debug::solve(jacobian, rhs, options, false);
        if direct.converged {
            return direct;
        }
        let mut iterative_options = *options;
        iterative_options.kind = match options.kind {
            FimLinearSolverKind::FgmresCpr => FimLinearSolverKind::FgmresCpr,
            _ => FimLinearSolverKind::GmresIlu0,
        };
        let mut iterative = solve_linearized_system_with_routing(
            jacobian,
            rhs,
            &iterative_options,
            layout,
            equation_scaling,
            false,
        );
        iterative.used_fallback = true;
        iterative.total_time_ms += direct.total_time_ms;
        iterative.preconditioner_build_time_ms += direct.preconditioner_build_time_ms;
        return iterative;
    }

    #[cfg(target_arch = "wasm32")]
    if allow_forced_direct && should_force_direct_solve(options.kind, jacobian.rows(), true) {
        let direct = dense_lu_debug::solve(jacobian, rhs, options, false);
        if direct.converged {
            return direct;
        }
        let mut iterative_options = *options;
        iterative_options.kind = match options.kind {
            FimLinearSolverKind::FgmresCpr => FimLinearSolverKind::FgmresCpr,
            _ => FimLinearSolverKind::GmresIlu0,
        };
        let mut iterative = solve_linearized_system_with_routing(
            jacobian,
            rhs,
            &iterative_options,
            layout,
            equation_scaling,
            false,
        );
        iterative.used_fallback = true;
        iterative.total_time_ms += direct.total_time_ms;
        iterative.preconditioner_build_time_ms += direct.preconditioner_build_time_ms;
        return iterative;
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
        let options = FimLinearSolveOptions::default();
        assert_eq!(options.kind, FimLinearSolverKind::FgmresCpr);
        assert!(!options.use_true_fgmres);
        assert!(!options.use_flow_lifecycle);
        assert!(!options.require_raw_full_residual_acceptance);
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
    fn failed_forced_direct_solve_preserves_requested_cpr_fallback() {
        let mut tri = TriMatI::<f64, usize>::new((2, 2));
        tri.add_triplet(0, 0, 2.0);
        let jacobian = tri.to_csr();
        let rhs = DVector::from_vec(vec![4.0, 0.0]);

        let direct =
            sparse_lu_debug::solve(&jacobian, &rhs, &FimLinearSolveOptions::default(), false);
        assert!(!direct.converged, "singular control must reject direct LU");

        let report = solve_linearized_system(
            &jacobian,
            &rhs,
            &FimLinearSolveOptions::default(),
            None,
            None,
        );

        assert!(report.converged);
        assert!(report.used_fallback);
        assert_eq!(report.backend_used, FimLinearSolverKind::FgmresCpr);
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
    fn iterative_fallback_bypasses_wasm_forced_direct_routing() {
        assert!(should_force_direct_solve(
            FimLinearSolverKind::GmresIlu0,
            2,
            true,
        ));

        let mut tri = TriMatI::<f64, usize>::new((2, 2));
        tri.add_triplet(0, 0, 2.0);
        let jacobian = tri.to_csr();
        let rhs = DVector::from_vec(vec![4.0, 0.0]);
        let options = FimLinearSolveOptions {
            kind: FimLinearSolverKind::GmresIlu0,
            ..FimLinearSolveOptions::default()
        };

        let report =
            solve_linearized_system_with_routing(&jacobian, &rhs, &options, None, None, false);

        assert_eq!(report.backend_used, FimLinearSolverKind::GmresIlu0);
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

/// `FIM-REPAIR-F4`: the linear diagnostic and recovery contract.
///
/// Every backend returns a `FimLinearSolveReport` whose `rhs_norm` and `final_residual_norm`
/// must describe **the original full system and the correction actually returned** — not a
/// reduced system, not a preconditioned norm, and not a placeholder. Newton reads
/// `report.reduction()` directly to decide whether to accept a non-strict solve
/// (`opm_accepts_relaxed_linear_report`), so a norm that refers to a different system than the
/// returned `solution` would silently corrupt a convergence decision rather than fail loudly.
///
/// Before this module, exactly one cell of that contract was tested
/// (`well_schur_report_uses_full_system_norms`, the converged well-Schur case). These tests walk
/// the whole coverage table — iterative/direct, with/without a well tail, finite/non-finite
/// correction, no-tail passthrough, and singular/rejected solves — checking each report against
/// an independently computed `r = rhs - J*dx` on the original full system.
///
/// Coverage table (each row is asserted by the test named in the last column; "contract" means
/// `rhs_norm`, `final_residual_norm`, `reduction()` and the reservoir/well row partitions all
/// agree with the independent recomputation):
///
/// ```text
/// backend            tail  condition            observed outcome                  test
/// ─────────────────  ────  ───────────────────  ────────────────────────────────  ────
/// sparse LU          no    regular              converged, contract holds         direct_backends_*
/// dense LU           no    regular              converged, contract holds         direct_backends_*
/// sparse/dense LU    no    singular, consistent rejected, dx = 0, reduction = 1   rejected_direct_solve_*
/// sparse/dense LU    no    inconsistent         rejected, dx = 0, reduction = 1   rejected_direct_solve_*
/// sparse/dense LU    no    NaN in J             non-finite reported as non-finite non_finite_correction_*
/// GMRES/ILU0         no    regular              converged, contract holds         iterative_backend_*
/// well-Schur + CPR   yes   regular              converged, full-system norms      well_schur_reports_*
/// well-Schur + CPR   yes   singular tail        NOT converged, norms still exact  well_schur_reports_*
/// well-Schur + CPR   no    no-tail passthrough  converged, contract holds         well_schur_no_tail_*
/// forced direct      no    singular -> fallback backend_used = actual backend     singular_forced_direct_*
/// ```
///
/// Two results are worth stating explicitly because they are easy to misread:
///
/// - A **rejected** direct solve returns a zero correction and reports `final_residual_norm ==
///   rhs_norm`. That is not a placeholder: it is the true residual of the zero vector it
///   returned, so `reduction() == 1` correctly reads as "no progress". It does mean the report
///   alone cannot distinguish "could not factorize" from "iterated without progress" — only
///   `failure_diagnostics` (populated by the iterative backends, deliberately `None` for direct
///   LU, see `opm_accepts_relaxed_linear_report`) carries that.
/// - `invert_tail_block` silently degrades to a diagonal approximation when the well tail has no
///   true inverse, which makes the Schur complement wrong with no signal inside the elimination.
///   The *only* thing that catches it is `recover_full_system_report` recomputing the residual on
///   the original full system. That safety net is load-bearing, and the singular-tail row above
///   is what pins it.
#[cfg(test)]
mod report_contract_tests {
    use nalgebra::DVector;
    use sprs::{CsMat, TriMatI};

    use super::gmres_block_jacobi::cs_mat_mul_vec;
    use super::well_schur::{sample_system, solve_with_well_elimination};
    use super::*;

    /// What an independent recomputation says about a returned report.
    #[derive(Debug)]
    struct IndependentCheck {
        rhs_norm: f64,
        residual_norm: f64,
        reservoir_partition: f64,
        well_partition: f64,
        solution_finite: bool,
    }

    /// Recompute every reported observable from `(jacobian, rhs, report.solution)` alone, without
    /// consulting the report. `noncell_start` splits reservoir rows from the well/perforation
    /// tail so the row partition is checked too, not just the aggregate norm.
    fn independent_check(
        jacobian: &CsMat<f64>,
        rhs: &DVector<f64>,
        report: &FimLinearSolveReport,
        noncell_start: usize,
    ) -> IndependentCheck {
        let residual = rhs - &cs_mat_mul_vec(jacobian, &report.solution);
        let reservoir_partition = residual.rows(0, noncell_start).norm();
        let well_partition = residual
            .rows(noncell_start, residual.len() - noncell_start)
            .norm();
        IndependentCheck {
            rhs_norm: rhs.norm(),
            residual_norm: residual.norm(),
            reservoir_partition,
            well_partition,
            solution_finite: report.solution.iter().all(|value| value.is_finite()),
        }
    }

    /// The contract itself, asserted identically for every row of the coverage table.
    ///
    /// Both norms must describe the same system as `solution`, `reduction()` must be derivable
    /// from them, and the reservoir/well row partitions must recombine into the reported total —
    /// which is what proves the report is not quoting a reduced-system norm after a Schur solve.
    fn assert_report_contract(
        label: &str,
        jacobian: &CsMat<f64>,
        rhs: &DVector<f64>,
        report: &FimLinearSolveReport,
        noncell_start: usize,
    ) {
        let check = independent_check(jacobian, rhs, report, noncell_start);

        assert!(
            (report.rhs_norm - check.rhs_norm).abs() <= 1e-12 * check.rhs_norm.max(1.0),
            "{label}: reported rhs_norm {:e} != independent {:e}",
            report.rhs_norm,
            check.rhs_norm
        );

        if check.solution_finite {
            assert!(
                (report.final_residual_norm - check.residual_norm).abs()
                    <= 1e-9 * check.residual_norm.max(1.0),
                "{label}: reported final_residual_norm {:e} != independent {:e}",
                report.final_residual_norm,
                check.residual_norm
            );

            let combined =
                (check.reservoir_partition.powi(2) + check.well_partition.powi(2)).sqrt();
            assert!(
                (combined - check.residual_norm).abs() <= 1e-9 * check.residual_norm.max(1.0),
                "{label}: reservoir/well partitions {:e}/{:e} do not recombine into {:e}",
                check.reservoir_partition,
                check.well_partition,
                check.residual_norm
            );

            let expected_reduction = report.final_residual_norm / report.rhs_norm.max(1e-30);
            assert!(
                (report.reduction() - expected_reduction).abs() <= 1e-12,
                "{label}: reduction() is not final_residual_norm / rhs_norm"
            );
        } else {
            // A non-finite correction must not be laundered into a finite-looking norm, and it
            // must never be accepted. `opm_accepts_relaxed_linear_report` is the live consumer.
            assert!(
                !report.final_residual_norm.is_finite(),
                "{label}: non-finite correction reported a finite residual norm {:e}",
                report.final_residual_norm
            );
            assert!(
                !report.converged,
                "{label}: non-finite correction must never report converged"
            );
        }

        // A report that claims convergence must actually have a small full-system residual.
        // This is the `FIM-LINEAR-013` spurious-convergence class: a reduced solve can satisfy
        // its own tolerance at `x_0 = 0` while the recovered full correction is effectively
        // zero.
        if report.converged {
            let tolerance = FimLinearSolveOptions::default().absolute_tolerance
                + FimLinearSolveOptions::default().relative_tolerance * check.rhs_norm.max(1e-30);
            assert!(
                check.residual_norm <= tolerance.max(1e-9),
                "{label}: converged=true but independent full-system residual is {:e} \
                 (rhs_norm {:e})",
                check.residual_norm,
                check.rhs_norm
            );
        }
    }

    fn diagonal_system(n: usize) -> (CsMat<f64>, DVector<f64>) {
        let mut tri = TriMatI::<f64, usize>::new((n, n));
        for idx in 0..n {
            tri.add_triplet(idx, idx, 2.0 + idx as f64);
        }
        (tri.to_csr(), DVector::from_element(n, 1.0))
    }

    /// A 2x2 system whose second row is empty. With `rhs[1] == 0` it is *consistent* (a finite
    /// exact solution exists) but has no LU factorization; with `rhs[1] != 0` it is
    /// inconsistent. Both must be reported honestly rather than as a converged solve.
    fn rank_deficient_system(inconsistent: bool) -> (CsMat<f64>, DVector<f64>) {
        let mut tri = TriMatI::<f64, usize>::new((2, 2));
        tri.add_triplet(0, 0, 2.0);
        let rhs = DVector::from_vec(vec![4.0, if inconsistent { 7.0 } else { 0.0 }]);
        (tri.to_csr(), rhs)
    }

    // ── Row 1: direct backends, no well tail ──────────────────────────────────────────────

    #[test]
    fn direct_backends_report_full_system_norms_for_the_returned_correction() {
        let options = FimLinearSolveOptions::default();
        let (jacobian, rhs) = diagonal_system(4);

        let sparse = sparse_lu_debug::solve(&jacobian, &rhs, &options, false);
        assert!(sparse.converged);
        assert_eq!(sparse.backend_used, FimLinearSolverKind::SparseLuDebug);
        assert_report_contract("sparse-lu regular", &jacobian, &rhs, &sparse, 4);

        let dense = dense_lu_debug::solve(&jacobian, &rhs, &options, false);
        assert!(dense.converged);
        assert_eq!(dense.backend_used, FimLinearSolverKind::DenseLuDebug);
        assert_report_contract("dense-lu regular", &jacobian, &rhs, &dense, 4);
    }

    // ── Row 2: direct backends, singular and inconsistent ─────────────────────────────────

    #[test]
    fn rejected_direct_solve_reports_a_zero_correction_at_full_residual() {
        let options = FimLinearSolveOptions::default();

        for inconsistent in [false, true] {
            let (jacobian, rhs) = rank_deficient_system(inconsistent);
            let label = if inconsistent {
                "inconsistent"
            } else {
                "singular-consistent"
            };

            for (name, report) in [
                (
                    "sparse-lu",
                    sparse_lu_debug::solve(&jacobian, &rhs, &options, false),
                ),
                (
                    "dense-lu",
                    dense_lu_debug::solve(&jacobian, &rhs, &options, false),
                ),
            ] {
                assert!(
                    !report.converged,
                    "{name} {label}: a rejected factorization must not report convergence"
                );
                // No correction was produced. The reported residual is the true residual of the
                // zero vector it returned — `reduction == 1`, i.e. "no progress" — rather than a
                // fabricated or reduced-system value.
                assert!(report.solution.iter().all(|value| *value == 0.0));
                assert!(
                    (report.reduction() - 1.0).abs() < 1e-12,
                    "{name} {label}: expected reduction 1.0, got {:e}",
                    report.reduction()
                );
                assert_report_contract(
                    &format!("{name} {label}"),
                    &jacobian,
                    &rhs,
                    &report,
                    jacobian.rows(),
                );
            }
        }
    }

    // ── Row 3: non-finite correction ──────────────────────────────────────────────────────

    #[test]
    fn non_finite_correction_is_reported_as_non_finite_and_never_accepted() {
        let options = FimLinearSolveOptions::default();
        let mut tri = TriMatI::<f64, usize>::new((2, 2));
        tri.add_triplet(0, 0, f64::NAN);
        tri.add_triplet(1, 1, 3.0);
        let jacobian = tri.to_csr();
        let rhs = DVector::from_vec(vec![1.0, 3.0]);

        for (name, report) in [
            (
                "sparse-lu",
                sparse_lu_debug::solve(&jacobian, &rhs, &options, false),
            ),
            (
                "dense-lu",
                dense_lu_debug::solve(&jacobian, &rhs, &options, false),
            ),
        ] {
            assert!(
                !report.solution.iter().all(|value| value.is_finite()),
                "{name}: fixture must actually produce a non-finite correction"
            );
            assert!(report.rhs_norm.is_finite(), "{name}: rhs_norm stays finite");
            assert_report_contract(name, &jacobian, &rhs, &report, jacobian.rows());
        }
    }

    // ── Row 4: iterative backend, no well tail ────────────────────────────────────────────

    #[test]
    fn iterative_backend_reports_full_system_norms_for_the_returned_correction() {
        let (jacobian, rhs) = diagonal_system(4);
        let options = FimLinearSolveOptions {
            kind: FimLinearSolverKind::GmresIlu0,
            ..FimLinearSolveOptions::default()
        };

        let report = gmres_block_jacobi::solve(&jacobian, &rhs, &options, None, false, None);
        assert!(report.converged);
        assert_eq!(report.backend_used, FimLinearSolverKind::GmresIlu0);
        assert_report_contract("gmres-ilu0 regular", &jacobian, &rhs, &report, 4);
    }

    // ── Row 5: well-Schur wrapper, converged and singular tail ────────────────────────────

    #[test]
    fn well_schur_reports_full_system_norms_including_a_singular_tail() {
        let options = FimLinearSolveOptions::default();
        let (jacobian, rhs, layout) = sample_system();

        let converged = solve_with_well_elimination(&jacobian, &rhs, &options, layout, None);
        assert!(converged.converged);
        assert_report_contract(
            "well-schur converged",
            &jacobian,
            &rhs,
            &converged,
            layout.noncell_start(),
        );

        // `invert_tail_block` silently degrades to a diagonal approximation when the well tail
        // has no true inverse. The Schur complement is then *wrong*, and nothing inside the
        // elimination says so — the only thing that catches it is the wrapper recomputing the
        // residual on the original full system. Pin that: the report must stay exact and must
        // refuse to claim convergence.
        let n = jacobian.rows();
        let tail_start = layout.noncell_start();
        let mut tri = TriMatI::<f64, usize>::new((n, n));
        for (row_idx, row) in jacobian.outer_iterator().enumerate() {
            for (col_idx, value) in row.iter() {
                if row_idx >= tail_start && col_idx >= tail_start {
                    continue;
                }
                tri.add_triplet(row_idx, col_idx, *value);
            }
        }
        // Rank-deficient tail: two identical rows, and a zero diagonal on the well-BHP row.
        tri.add_triplet(tail_start + 1, tail_start + 1, 1.0);
        tri.add_triplet(tail_start + 1, tail_start + 2, 1.0);
        tri.add_triplet(tail_start + 2, tail_start + 1, 1.0);
        tri.add_triplet(tail_start + 2, tail_start + 2, 1.0);
        let singular_tail = tri.to_csr();

        let degraded = solve_with_well_elimination(&singular_tail, &rhs, &options, layout, None);
        assert!(
            !degraded.converged,
            "a degraded tail inverse must not produce a converged report"
        );
        assert_report_contract(
            "well-schur singular tail",
            &singular_tail,
            &rhs,
            &degraded,
            tail_start,
        );
    }

    // ── Row 6: no-tail passthrough ────────────────────────────────────────────────────────

    #[test]
    fn well_schur_no_tail_passthrough_still_reports_full_system_norms() {
        let (jacobian, rhs) = diagonal_system(3);
        let layout = FimLinearBlockLayout {
            cell_block_count: 1,
            cell_block_size: 3,
            well_bhp_count: 0,
            perforation_tail_start: 3,
        };
        let options = FimLinearSolveOptions::default();

        let report = solve_with_well_elimination(&jacobian, &rhs, &options, layout, None);
        assert!(report.converged);
        assert_report_contract("well-schur no tail", &jacobian, &rhs, &report, 3);
    }

    // ── Row 7: forced-direct fallback reports its actual backend ──────────────────────────

    #[test]
    fn singular_forced_direct_fallback_reports_the_backend_that_produced_the_correction() {
        let options = FimLinearSolveOptions::default();
        let (jacobian, rhs) = rank_deficient_system(false);

        let direct = sparse_lu_debug::solve(&jacobian, &rhs, &options, false);
        assert!(
            !direct.converged,
            "control: direct LU must reject this system"
        );

        let report = solve_linearized_system(&jacobian, &rhs, &options, None, None);
        assert!(report.used_fallback, "the fallback path must be marked");
        assert_eq!(
            report.backend_used,
            FimLinearSolverKind::FgmresCpr,
            "the report must name the backend that actually produced the correction, \
             not the one that was tried first"
        );
        assert_report_contract("forced-direct fallback", &jacobian, &rhs, &report, 2);
    }
}
