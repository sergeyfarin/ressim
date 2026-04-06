use std::f64;

use faer::Col;
use faer::linalg::solvers::Solve;
use faer::sparse::{SparseRowMat, Triplet};
use nalgebra::DVector;
use sprs::CsMat;

use super::{FimLinearSolveOptions, FimLinearSolveReport, FimLinearSolverKind};
use crate::timing::PerfTimer;

fn build_sparse_row_matrix(matrix: &CsMat<f64>) -> Option<SparseRowMat<usize, f64>> {
    let mut triplets = Vec::with_capacity(matrix.nnz());
    for (row, vec) in matrix.outer_iterator().enumerate() {
        for (&col, &val) in vec.indices().iter().zip(vec.data().iter()) {
            triplets.push(Triplet::new(row, col, val));
        }
    }

    SparseRowMat::<usize, f64>::try_new_from_triplets(matrix.rows(), matrix.cols(), &triplets).ok()
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

pub(super) fn solve(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
    used_fallback: bool,
) -> FimLinearSolveReport {
    let timer = PerfTimer::start();
    let Some(matrix) = build_sparse_row_matrix(jacobian) else {
        return FimLinearSolveReport {
            solution: DVector::zeros(rhs.len()),
            converged: false,
            iterations: 0,
            final_residual_norm: rhs.norm(),
            used_fallback,
            backend_used: FimLinearSolverKind::SparseLuDebug,
            cpr_diagnostics: None,
            total_time_ms: timer.elapsed_ms(),
            preconditioner_build_time_ms: 0.0,
        };
    };

    let Ok(factorization) = matrix.sp_lu() else {
        return FimLinearSolveReport {
            solution: DVector::zeros(rhs.len()),
            converged: false,
            iterations: 0,
            final_residual_norm: rhs.norm(),
            used_fallback,
            backend_used: FimLinearSolverKind::SparseLuDebug,
            cpr_diagnostics: None,
            total_time_ms: timer.elapsed_ms(),
            preconditioner_build_time_ms: 0.0,
        };
    };

    let rhs_col = Col::from_fn(rhs.len(), |row| rhs[row]);
    let solved = factorization.solve(&rhs_col);
    let solution = DVector::from_iterator(rhs.len(), (0..rhs.len()).map(|idx| solved[idx]));
    let residual = rhs - &cs_mat_mul_vec(jacobian, &solution);
    let residual_norm = residual.norm();
    let tolerance =
        options.absolute_tolerance + options.relative_tolerance * rhs.norm().max(f64::EPSILON);

    FimLinearSolveReport {
        solution,
        converged: residual_norm <= tolerance,
        iterations: usize::from(residual_norm > 0.0),
        final_residual_norm: residual_norm,
        used_fallback,
        backend_used: FimLinearSolverKind::SparseLuDebug,
        cpr_diagnostics: None,
        total_time_ms: timer.elapsed_ms(),
        preconditioner_build_time_ms: 0.0,
    }
}
