use nalgebra::DVector;
use sprs::CsMat;

mod dense_lu_debug;
mod gmres_block_jacobi;
mod sparse_lu_debug;

const DIRECT_SOLVE_ROW_THRESHOLD: usize = 512;
const WASM_DIRECT_SOLVE_ROW_THRESHOLD: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FimLinearSolverKind {
    FgmresCpr,
    GmresIlu0,
    DenseLuDebug,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FimLinearBlockLayout {
    pub(crate) cell_block_count: usize,
    pub(crate) cell_block_size: usize,
    pub(crate) scalar_tail_start: usize,
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
    layout: Option<FimLinearBlockLayout>,
) -> FimLinearSolveReport {
    #[cfg(not(target_arch = "wasm32"))]
    if options.kind == FimLinearSolverKind::FgmresCpr
        && jacobian.rows() <= DIRECT_SOLVE_ROW_THRESHOLD
    {
        return sparse_lu_debug::solve(jacobian, rhs, options, options.kind != FimLinearSolverKind::SparseLuDebug);
    }

    #[cfg(target_arch = "wasm32")]
    if options.kind != FimLinearSolverKind::SparseLuDebug
        && jacobian.rows() <= WASM_DIRECT_SOLVE_ROW_THRESHOLD
    {
        return dense_lu_debug::solve(jacobian, rhs, options, true);
    }

    match options.kind {
        FimLinearSolverKind::DenseLuDebug => dense_lu_debug::solve(jacobian, rhs, options, false),
        FimLinearSolverKind::SparseLuDebug => sparse_lu_debug::solve(jacobian, rhs, options, false),
        FimLinearSolverKind::GmresIlu0 => {
            gmres_block_jacobi::solve(jacobian, rhs, options, layout, false)
        }
        // CPR is still incomplete, but the default FIM path now uses a pressure-first
        // two-stage iterative backend instead of falling straight back to sparse LU.
        FimLinearSolverKind::FgmresCpr => {
            gmres_block_jacobi::solve(jacobian, rhs, options, layout, true)
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

        let report = solve_linearized_system(&jacobian, &rhs, &FimLinearSolveOptions::default(), None);

        assert!(report.converged);
        assert!(report.used_fallback);
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

        let report = solve_linearized_system(&jacobian, &rhs, &FimLinearSolveOptions::default(), None);

        assert!(report.converged);
        assert!(report.used_fallback);
        assert_eq!(report.backend_used, FimLinearSolverKind::FgmresCpr);
    }
}
