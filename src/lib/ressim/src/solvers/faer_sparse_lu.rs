use std::f64;

use faer::Col;
use faer::linalg::solvers::Solve;
use faer::sparse::{SparseRowMat, Triplet};
use nalgebra::DVector;

use super::{LinearSolveParams, LinearSolveResult, cs_mat_mul_vec};

#[cfg(test)]
thread_local! {
    static FORCE_FAIL_FOR_TESTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(super) fn with_forced_failure_for_tests<T>(callback: impl FnOnce() -> T) -> T {
    struct ResetGuard;

    impl Drop for ResetGuard {
        fn drop(&mut self) {
            FORCE_FAIL_FOR_TESTS.with(|flag| flag.set(false));
        }
    }

    FORCE_FAIL_FOR_TESTS.with(|flag| flag.set(true));
    let _reset_guard = ResetGuard;
    callback()
}

fn build_sparse_row_matrix(matrix: &sprs::CsMat<f64>) -> Option<SparseRowMat<usize, f64>> {
    let mut triplets = Vec::with_capacity(matrix.nnz());
    for (row, vec) in matrix.outer_iterator().enumerate() {
        for (&col, &val) in vec.indices().iter().zip(vec.data().iter()) {
            triplets.push(Triplet::new(row, col, val));
        }
    }

    SparseRowMat::<usize, f64>::try_new_from_triplets(matrix.rows(), matrix.cols(), &triplets).ok()
}

pub(super) fn solve(params: &LinearSolveParams<'_>) -> LinearSolveResult {
    #[cfg(test)]
    if FORCE_FAIL_FOR_TESTS.with(|flag| flag.get()) {
        return LinearSolveResult {
            solution: params.initial_guess.clone(),
            converged: false,
            iterations: 0,
        };
    }

    let Some(matrix) = build_sparse_row_matrix(params.matrix) else {
        return LinearSolveResult {
            solution: params.initial_guess.clone(),
            converged: false,
            iterations: 0,
        };
    };

    let Ok(factorization) = matrix.sp_lu() else {
        return LinearSolveResult {
            solution: params.initial_guess.clone(),
            converged: false,
            iterations: 0,
        };
    };

    let rhs = Col::from_fn(params.rhs.len(), |row| params.rhs[row]);
    let solved = factorization.solve(&rhs);
    let solution =
        DVector::from_iterator(params.rhs.len(), (0..params.rhs.len()).map(|i| solved[i]));

    let residual = params.rhs - &cs_mat_mul_vec(params.matrix, &solution);
    let rhs_norm = params.rhs.norm().max(f64::EPSILON);
    let residual_ratio = residual.norm() / rhs_norm;
    let converged = residual_ratio.is_finite()
        && residual_ratio <= params.tolerance
        && solution.iter().all(|value| value.is_finite());

    LinearSolveResult {
        solution,
        converged,
        iterations: usize::from(converged),
    }
}

#[cfg(test)]
mod tests {
    use nalgebra::DVector;
    use sprs::TriMatI;

    use super::*;

    #[test]
    fn faer_sparse_lu_solves_small_nonsymmetric_system() {
        let mut tri = TriMatI::<f64, usize>::new((2, 2));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 1, 1.0);
        tri.add_triplet(1, 0, 2.0);
        tri.add_triplet(1, 1, 3.0);
        let matrix = tri.to_csr();

        let rhs = DVector::from_vec(vec![1.0, 1.0]);
        let initial_guess = DVector::from_vec(vec![0.0, 0.0]);
        let preconditioner_inv_diag = DVector::from_vec(vec![0.25, 1.0 / 3.0]);

        let result = solve(&LinearSolveParams {
            matrix: &matrix,
            rhs: &rhs,
            preconditioner_inv_diag: &preconditioner_inv_diag,
            initial_guess: &initial_guess,
            tolerance: 1e-10,
            max_iterations: 100,
        });

        assert!(
            result.converged,
            "faer sparse LU should converge for a small nonsymmetric system"
        );
        assert!((result.solution[0] - 0.2).abs() < 1e-8);
        assert!((result.solution[1] - 0.2).abs() < 1e-8);
    }
}
