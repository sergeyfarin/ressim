use std::f64;

use nalgebra::{DMatrix, DVector};
use sprs::CsMat;

use super::{FimLinearSolveOptions, FimLinearSolveReport, FimLinearSolverKind};

pub(super) fn solve(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
    used_fallback: bool,
) -> FimLinearSolveReport {
    let mut dense = DMatrix::<f64>::zeros(jacobian.rows(), jacobian.cols());
    for (row_idx, row) in jacobian.outer_iterator().enumerate() {
        for (col_idx, value) in row.iter() {
            dense[(row_idx, col_idx)] = *value;
        }
    }

    let solution = dense
        .clone()
        .lu()
        .solve(rhs)
        .unwrap_or_else(|| DVector::zeros(rhs.len()));
    let residual = rhs - &dense * &solution;
    let residual_norm = residual.norm();
    let tolerance =
        options.absolute_tolerance + options.relative_tolerance * rhs.norm().max(f64::EPSILON);

    FimLinearSolveReport {
        solution,
        converged: residual_norm <= tolerance,
        iterations: 1,
        final_residual_norm: residual_norm,
        used_fallback,
        backend_used: FimLinearSolverKind::DenseLuDebug,
        cpr_diagnostics: None,
    }
}
