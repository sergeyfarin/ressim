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

// Sparse LU has excellent robustness and low setup cost on the small systems
// used by the analytical/contract cases.  Its numeric factorization becomes
// the dominant cost on larger Cartesian grids, where ILU(0)-preconditioned
// BiCGSTAB scales much better and can reuse the previous pressure as its warm
// start.  Keep the crossover deliberately above the 300-cell SPE1 base case,
// where direct LU is still faster.
const ITERATIVE_SOLVE_MIN_ROWS: usize = 512;

#[derive(Clone, Copy)]
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
    if params.matrix.rows() >= ITERATIVE_SOLVE_MIN_ROWS {
        let iterative = solve_linear_system(LinearSolverKind::BiCgStab, params);
        if iterative.converged {
            return iterative;
        }

        // Preserve the direct solver as a robustness backstop.  A failed
        // iterative solve must not turn into an IMPES timestep cut when LU can
        // still solve the same pressure system.
        return solve_linear_system(LinearSolverKind::FaerSparseLu, params);
    }

    let direct = solve_linear_system(LinearSolverKind::DEFAULT, params);
    if direct.converged {
        return direct;
    }

    let iterative = solve_linear_system(LinearSolverKind::BiCgStab, params);
    if iterative.converged
        || relative_residual_ratio(params.matrix, params.rhs, &iterative.solution)
            < relative_residual_ratio(params.matrix, params.rhs, &direct.solution)
    {
        iterative
    } else {
        direct
    }
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

fn relative_residual_ratio(a: &CsMat<f64>, rhs: &DVector<f64>, solution: &DVector<f64>) -> f64 {
    let residual = rhs - &cs_mat_mul_vec(a, solution);
    residual.norm() / rhs.norm().max(f64::EPSILON)
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
    fn large_system_uses_iterative_solver_before_direct_lu() {
        let n = ITERATIVE_SOLVE_MIN_ROWS;
        let mut tri = TriMatI::<f64, usize>::new((n, n));
        for row in 0..n {
            tri.add_triplet(row, row, 2.0);
        }
        let matrix = tri.to_csr();
        let rhs = DVector::from_element(n, 2.0);
        let preconditioner_inv_diag = DVector::from_element(n, 0.5);
        // Exact warm start: the iterative route recognizes this without a
        // Krylov iteration, whereas direct LU would report one solve.
        let initial_guess = DVector::from_element(n, 1.0);

        let result = faer_sparse_lu::with_forced_failure_for_tests(|| {
            solve_with_default(LinearSolveParams {
                matrix: &matrix,
                rhs: &rhs,
                preconditioner_inv_diag: &preconditioner_inv_diag,
                initial_guess: &initial_guess,
                tolerance: 1e-10,
                max_iterations: 100,
            })
        });

        assert!(result.converged);
        assert_eq!(result.iterations, 0);
        assert!(
            result
                .solution
                .iter()
                .all(|value| (*value - 1.0).abs() < 1e-10)
        );
    }

    #[test]
    fn ilu0_bicgstab_matches_direct_on_large_cartesian_pressure_system() {
        let nx = 24;
        let ny = 24;
        let n = nx * ny;
        let mut tri = TriMatI::<f64, usize>::new((n, n));
        for j in 0..ny {
            for i in 0..nx {
                let row = j * nx + i;
                let mut diagonal = 1.0;
                for (neighbor, transmissibility) in [
                    (i.checked_sub(1).map(|x| j * nx + x), 0.8),
                    ((i + 1 < nx).then_some(j * nx + i + 1), 1.1),
                    (j.checked_sub(1).map(|y| y * nx + i), 0.9),
                    ((j + 1 < ny).then_some((j + 1) * nx + i), 1.2),
                ] {
                    if let Some(column) = neighbor {
                        tri.add_triplet(row, column, -transmissibility);
                        diagonal += transmissibility;
                    }
                }
                tri.add_triplet(row, row, diagonal);
            }
        }
        let matrix = tri.to_csr();
        let expected = DVector::from_iterator(n, (0..n).map(|i| 250.0 + (i % nx) as f64));
        let rhs = cs_mat_mul_vec(&matrix, &expected);
        let inverse_diagonal = DVector::from_iterator(
            n,
            (0..n).map(|row| {
                1.0 / matrix
                    .get(row, row)
                    .copied()
                    .expect("pressure matrix diagonal")
            }),
        );
        let initial_guess = DVector::from_element(n, 250.0);
        let params = LinearSolveParams {
            matrix: &matrix,
            rhs: &rhs,
            preconditioner_inv_diag: &inverse_diagonal,
            initial_guess: &initial_guess,
            tolerance: 1e-10,
            max_iterations: 1000,
        };

        let iterative = solve_linear_system(LinearSolverKind::BiCgStab, params);
        let direct = solve_linear_system(LinearSolverKind::FaerSparseLu, params);

        assert!(iterative.converged);
        assert!(direct.converged);
        assert!(iterative.iterations < 30);
        assert!(relative_residual_ratio(&matrix, &rhs, &iterative.solution) <= 1e-10);
        assert!(
            iterative
                .solution
                .iter()
                .zip(direct.solution.iter())
                .all(|(lhs, rhs)| (lhs - rhs).abs() < 1e-6)
        );
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

    #[test]
    fn default_solver_falls_back_to_bicgstab_when_faer_fails() {
        let (matrix, rhs, m_inv_diag, x0) = small_nonsymmetric_system();

        let result = faer_sparse_lu::with_forced_failure_for_tests(|| {
            solve_with_default(LinearSolveParams {
                matrix: &matrix,
                rhs: &rhs,
                preconditioner_inv_diag: &m_inv_diag,
                initial_guess: &x0,
                tolerance: 1e-10,
                max_iterations: 100,
            })
        });

        assert!(
            result.converged,
            "default solver should fall back to BiCGSTAB when faer LU fails"
        );
        assert!((result.solution[0] - 0.2).abs() < 1e-8);
        assert!((result.solution[1] - 0.2).abs() < 1e-8);
    }
}
