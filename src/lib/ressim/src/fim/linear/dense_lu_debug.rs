use std::f64;

use nalgebra::{DMatrix, DVector};
use sprs::CsMat;

use super::{FimLinearSolveOptions, FimLinearSolveReport, FimLinearSolverKind};
use crate::timing::PerfTimer;

fn cs_mat_mul_vec(a: &CsMat<f64>, x: &DVector<f64>) -> DVector<f64> {
    let mut y = DVector::<f64>::zeros(a.rows());
    for (row_idx, row) in a.outer_iterator().enumerate() {
        let mut sum = 0.0;
        for (col_idx, value) in row.iter() {
            sum += *value * x[col_idx];
        }
        y[row_idx] = sum;
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
    let mut dense = DMatrix::<f64>::zeros(jacobian.rows(), jacobian.cols());
    for (row_idx, row) in jacobian.outer_iterator().enumerate() {
        for (col_idx, value) in row.iter() {
            dense[(row_idx, col_idx)] = *value;
        }
    }

    let solution = dense.lu().solve(rhs).unwrap_or_else(|| DVector::zeros(rhs.len()));
    let residual = rhs - &cs_mat_mul_vec(jacobian, &solution);
    let residual_norm = residual.norm();
    let tolerance =
        options.absolute_tolerance + options.relative_tolerance * rhs.norm().max(f64::EPSILON);

    FimLinearSolveReport {
        solution,
        converged: residual_norm <= tolerance,
        iterations: 1,
        final_residual_norm: residual_norm,
        failure_diagnostics: None,
        used_fallback,
        backend_used: FimLinearSolverKind::DenseLuDebug,
        cpr_diagnostics: None,
        total_time_ms: timer.elapsed_ms(),
        preconditioner_build_time_ms: 0.0,
    }
}
