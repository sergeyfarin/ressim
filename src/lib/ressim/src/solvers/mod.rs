use nalgebra::DVector;
use sprs::CsMat;

mod bicgstab;
mod faer_sparse_lu;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LinearSolverKind {
    FaerSparseLu,
    BiCgStab,
}

impl LinearSolverKind {
    pub(crate) const DEFAULT: Self = Self::FaerSparseLu;
}

pub(crate) struct LinearSolveParams<'a> {
    pub(crate) matrix: &'a CsMat<f64>,
    pub(crate) rhs: &'a DVector<f64>,
    pub(crate) preconditioner_inv_diag: &'a DVector<f64>,
    pub(crate) initial_guess: &'a DVector<f64>,
    pub(crate) tolerance: f64,
    pub(crate) max_iterations: usize,
}

/// Result from the linear solver including convergence info.
pub(crate) struct LinearSolveResult {
    pub(crate) solution: DVector<f64>,
    pub(crate) converged: bool,
    pub(crate) iterations: usize,
}

pub(crate) fn solve_with_default(params: LinearSolveParams<'_>) -> LinearSolveResult {
    solve_linear_system(LinearSolverKind::DEFAULT, params)
}

pub(crate) fn solve_linear_system(
    kind: LinearSolverKind,
    params: LinearSolveParams<'_>,
) -> LinearSolveResult {
    match kind {
        LinearSolverKind::FaerSparseLu => faer_sparse_lu::solve(&params),
        LinearSolverKind::BiCgStab => bicgstab::solve(&params),
    }
}

fn cs_mat_mul_vec(a: &CsMat<f64>, x: &DVector<f64>) -> DVector<f64> {
    let n = a.rows();
    let mut y = DVector::<f64>::zeros(n);
    for (row, vec) in a.outer_iterator().enumerate() {
        let mut sum = 0.0;
        for (&col, &val) in vec.indices().iter().zip(vec.data().iter()) {
            sum += val * x[col];
        }
        y[row] = sum;
    }
    y
}

fn apply_jacobi_preconditioner(m_inv_diag: &DVector<f64>, rhs: &DVector<f64>) -> DVector<f64> {
    let mut out = DVector::<f64>::zeros(rhs.len());
    for i in 0..rhs.len() {
        out[i] = rhs[i] * m_inv_diag[i];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sprs::TriMatI;

    fn small_nonsymmetric_system() -> (CsMat<f64>, DVector<f64>, DVector<f64>, DVector<f64>) {
        let mut tri = TriMatI::<f64, usize>::new((2, 2));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 1, 1.0);
        tri.add_triplet(1, 0, 2.0);
        tri.add_triplet(1, 1, 3.0);

        (
            tri.to_csr(),
            DVector::from_vec(vec![1.0, 1.0]),
            DVector::from_vec(vec![0.25, 1.0 / 3.0]),
            DVector::from_vec(vec![0.0, 0.0]),
        )
    }

    #[test]
    fn faer_sparse_lu_is_the_default_solver() {
        assert_eq!(LinearSolverKind::DEFAULT, LinearSolverKind::FaerSparseLu);
    }

    #[test]
    fn default_solver_solves_small_nonsymmetric_system() {
        let (matrix, rhs, m_inv_diag, x0) = small_nonsymmetric_system();
        let result = solve_with_default(LinearSolveParams {
            matrix: &matrix,
            rhs: &rhs,
            preconditioner_inv_diag: &m_inv_diag,
            initial_guess: &x0,
            tolerance: 1e-10,
            max_iterations: 100,
        });

        assert!(
            result.converged,
            "default linear solver should converge for a small nonsymmetric system"
        );
        assert!((result.solution[0] - 0.2).abs() < 1e-8);
        assert!((result.solution[1] - 0.2).abs() < 1e-8);
    }
}
