use std::f64;

use faer::Col;
use faer::linalg::solvers::Solve;
use faer::sparse::{SparseRowMat, Triplet};
use nalgebra::DVector;
use sprs::CsMat;

use super::{FimLinearSolveOptions, FimLinearSolveReport, FimLinearSolverKind};
use crate::timing::PerfTimer;

const MAX_ITERATIVE_REFINEMENT_STEPS: usize = 2;
const MIN_REFINEMENT_IMPROVEMENT_RATIO: f64 = 0.95;

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
            failure_diagnostics: None,
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
            failure_diagnostics: None,
            used_fallback,
            backend_used: FimLinearSolverKind::SparseLuDebug,
            cpr_diagnostics: None,
            total_time_ms: timer.elapsed_ms(),
            preconditioner_build_time_ms: 0.0,
        };
    };

    let tolerance =
        options.absolute_tolerance + options.relative_tolerance * rhs.norm().max(f64::EPSILON);
    let rhs_col = Col::from_fn(rhs.len(), |row| rhs[row]);
    let solved = factorization.solve(&rhs_col);
    let mut solution = DVector::from_iterator(rhs.len(), (0..rhs.len()).map(|idx| solved[idx]));
    let mut residual = rhs - &cs_mat_mul_vec(jacobian, &solution);
    let mut residual_norm = residual.norm();
    let mut solves = usize::from(residual_norm > 0.0);

    for _ in 0..MAX_ITERATIVE_REFINEMENT_STEPS {
        if !residual_norm.is_finite() || residual_norm <= tolerance {
            break;
        }

        let correction_rhs = Col::from_fn(rhs.len(), |row| residual[row]);
        let correction_col = factorization.solve(&correction_rhs);
        let correction = DVector::from_iterator(
            rhs.len(),
            (0..rhs.len()).map(|idx| correction_col[idx]),
        );
        if !correction.iter().all(|value| value.is_finite()) {
            break;
        }

        let candidate = &solution + correction;
        let candidate_residual = rhs - &cs_mat_mul_vec(jacobian, &candidate);
        let candidate_residual_norm = candidate_residual.norm();
        if !candidate_residual_norm.is_finite()
            || candidate_residual_norm >= residual_norm * MIN_REFINEMENT_IMPROVEMENT_RATIO
        {
            break;
        }

        solution = candidate;
        residual = candidate_residual;
        residual_norm = candidate_residual_norm;
        solves += 1;
    }

    FimLinearSolveReport {
        solution,
        converged: residual_norm <= tolerance,
        iterations: solves,
        final_residual_norm: residual_norm,
        failure_diagnostics: None,
        used_fallback,
        backend_used: FimLinearSolverKind::SparseLuDebug,
        cpr_diagnostics: None,
        total_time_ms: timer.elapsed_ms(),
        preconditioner_build_time_ms: 0.0,
    }
}
