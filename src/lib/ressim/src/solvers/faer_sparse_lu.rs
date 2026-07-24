use std::f64;

use faer::Col;
use faer::linalg::solvers::Solve;
use faer::sparse::linalg::solvers::{Lu, SymbolicLu};
use faer::sparse::{SparseColMatRef, SparseRowMat, Triplet};
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

// Cached symbolic LU factorization.
//
// The IMPES pressure matrix keeps a fixed sparsity pattern for the lifetime of a
// run (Cartesian grid, 7-point stencil, fixed well completions) while its values
// change every substep. Symbolic analysis (fill-reducing ordering plus the
// elimination structure) depends only on the pattern, so recomputing it per solve
// is pure overhead — a fine-grid SPE1 run performs thousands of solves against one
// pattern. The numeric factorization still runs every solve, so solutions are
// unchanged.
//
// The cache holds a single entry and is invalidated whenever the pattern changes
// (different grid, or a structurally different matrix), which keeps it correct for
// simulators that are recreated or resized within one thread.
thread_local! {
    static SYMBOLIC_CACHE: std::cell::RefCell<Option<(SymbolicPatternKey, SymbolicLu<usize>)>> =
        const { std::cell::RefCell::new(None) };
}

/// Identifies a sparsity pattern well enough to detect any structural change.
#[derive(PartialEq, Eq)]
struct SymbolicPatternKey {
    rows: usize,
    cols: usize,
    indptr: Vec<usize>,
    indices: Vec<usize>,
}

impl SymbolicPatternKey {
    fn matches(&self, matrix: &sprs::CsMat<f64>) -> bool {
        self.rows == matrix.rows()
            && self.cols == matrix.cols()
            && *matrix.indptr().to_proper() == *self.indptr
            && matrix.indices() == self.indices
    }

    fn from_matrix(matrix: &sprs::CsMat<f64>) -> Self {
        Self {
            rows: matrix.rows(),
            cols: matrix.cols(),
            indptr: matrix.indptr().to_proper().to_vec(),
            indices: matrix.indices().to_vec(),
        }
    }
}

/// Returns a symbolic factorization for `pattern`'s structure, reusing the cached
/// one when the structure is unchanged. Only a cache miss allocates a new key.
fn symbolic_for(
    matrix: SparseColMatRef<'_, usize, f64>,
    pattern: &sprs::CsMat<f64>,
) -> Option<SymbolicLu<usize>> {
    SYMBOLIC_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((cached_key, symbolic)) = cache.as_ref() {
            if cached_key.matches(pattern) {
                return Some(symbolic.clone());
            }
        }

        let symbolic = SymbolicLu::try_new(matrix.symbolic()).ok()?;
        *cache = Some((SymbolicPatternKey::from_matrix(pattern), symbolic.clone()));
        Some(symbolic)
    })
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

    // faer's LU factorizes column-major input; convert once and reuse the
    // conversion for both the symbolic lookup and the numeric factorization.
    let Ok(matrix) = matrix.to_col_major() else {
        return LinearSolveResult {
            solution: params.initial_guess.clone(),
            converged: false,
            iterations: 0,
        };
    };
    let matrix = matrix.as_ref();

    let Some(symbolic) = symbolic_for(matrix, params.matrix) else {
        return LinearSolveResult {
            solution: params.initial_guess.clone(),
            converged: false,
            iterations: 0,
        };
    };

    let Ok(factorization) = Lu::try_new_with_symbolic(symbolic, matrix) else {
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

    /// The symbolic factorization is cached per sparsity pattern. Alternating
    /// between differently-shaped systems must invalidate and rebuild it, and
    /// returning to an earlier pattern must still produce the correct solution —
    /// a stale symbolic structure would silently corrupt results.
    #[test]
    fn faer_sparse_lu_reuses_and_invalidates_the_symbolic_cache() {
        let solve_2x2 = || {
            let mut tri = TriMatI::<f64, usize>::new((2, 2));
            tri.add_triplet(0, 0, 4.0);
            tri.add_triplet(0, 1, 1.0);
            tri.add_triplet(1, 0, 2.0);
            tri.add_triplet(1, 1, 3.0);
            let matrix = tri.to_csr();
            let rhs = DVector::from_vec(vec![1.0, 1.0]);
            let result = solve(&LinearSolveParams {
                matrix: &matrix,
                rhs: &rhs,
                preconditioner_inv_diag: &DVector::from_vec(vec![0.25, 1.0 / 3.0]),
                initial_guess: &DVector::from_vec(vec![0.0, 0.0]),
                tolerance: 1e-10,
                max_iterations: 100,
            });
            assert!(result.converged);
            assert!((result.solution[0] - 0.2).abs() < 1e-8);
            assert!((result.solution[1] - 0.2).abs() < 1e-8);
        };

        // A structurally different system: 3x3 tridiagonal, solution [1, 1, 1].
        let solve_3x3 = || {
            let mut tri = TriMatI::<f64, usize>::new((3, 3));
            tri.add_triplet(0, 0, 2.0);
            tri.add_triplet(0, 1, -1.0);
            tri.add_triplet(1, 0, -1.0);
            tri.add_triplet(1, 1, 2.0);
            tri.add_triplet(1, 2, -1.0);
            tri.add_triplet(2, 1, -1.0);
            tri.add_triplet(2, 2, 2.0);
            let matrix = tri.to_csr();
            let rhs = DVector::from_vec(vec![1.0, 0.0, 1.0]);
            let result = solve(&LinearSolveParams {
                matrix: &matrix,
                rhs: &rhs,
                preconditioner_inv_diag: &DVector::from_vec(vec![0.5, 0.5, 0.5]),
                initial_guess: &DVector::zeros(3),
                tolerance: 1e-10,
                max_iterations: 100,
            });
            assert!(result.converged);
            for i in 0..3 {
                assert!((result.solution[i] - 1.0).abs() < 1e-8);
            }
        };

        solve_2x2();
        solve_2x2(); // cache hit on the same pattern
        solve_3x3(); // different pattern — must invalidate
        solve_2x2(); // back to the first pattern — must rebuild, not reuse 3x3
    }
}
