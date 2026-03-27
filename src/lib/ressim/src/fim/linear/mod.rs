use nalgebra::DVector;
use sprs::CsMat;

mod sparse_lu_debug;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FimLinearSolverKind {
    FgmresCpr,
    GmresIlu0,
    SparseLuDebug,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimLinearSolveOptions {
    pub(crate) kind: FimLinearSolverKind,
    pub(crate) restart: usize,
    pub(crate) max_iterations: usize,
    pub(crate) relative_tolerance: f64,
    pub(crate) absolute_tolerance: f64,
}

impl Default for FimLinearSolveOptions {
    fn default() -> Self {
        Self {
            kind: FimLinearSolverKind::FgmresCpr,
            restart: 30,
            max_iterations: 150,
            relative_tolerance: 1e-7,
            absolute_tolerance: 1e-10,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FimLinearSolveReport {
    pub(crate) solution: DVector<f64>,
    pub(crate) converged: bool,
    pub(crate) iterations: usize,
    pub(crate) final_residual_norm: f64,
    pub(crate) used_fallback: bool,
    pub(crate) backend_used: FimLinearSolverKind,
}

pub(crate) fn solve_linearized_system(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
) -> FimLinearSolveReport {
    match options.kind {
        FimLinearSolverKind::SparseLuDebug => sparse_lu_debug::solve(jacobian, rhs, options, false),
        // Temporary scaffold for the target FIM architecture: route through the
        // sparse-LU debug backend until GMRES/CPR and ILU are implemented.
        FimLinearSolverKind::FgmresCpr | FimLinearSolverKind::GmresIlu0 => {
            sparse_lu_debug::solve(jacobian, rhs, options, true)
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
    fn target_fim_solver_kinds_temporarily_report_sparse_lu_fallback() {
        let mut tri = TriMatI::<f64, usize>::new((2, 2));
        tri.add_triplet(0, 0, 2.0);
        tri.add_triplet(1, 1, 3.0);
        let jacobian = tri.to_csr();
        let rhs = DVector::from_vec(vec![4.0, 9.0]);

        let report = solve_linearized_system(&jacobian, &rhs, &FimLinearSolveOptions::default());

        assert!(report.converged);
        assert!(report.used_fallback);
        assert_eq!(report.backend_used, FimLinearSolverKind::SparseLuDebug);
        assert!((report.solution[0] - 2.0).abs() < 1e-12);
        assert!((report.solution[1] - 3.0).abs() < 1e-12);
    }
}
