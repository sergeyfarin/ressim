use std::f64;

#[cfg(test)]
use std::collections::HashSet;

use faer::linalg::solvers::Solve;
use faer::sparse::{SparseRowMat, Triplet};
use faer::Col;
use nalgebra::DVector;
use sprs::CsMat;

use super::{FimLinearSolveOptions, FimLinearSolveReport, FimLinearSolverKind};
use crate::timing::PerfTimer;

const MAX_ITERATIVE_REFINEMENT_STEPS: usize = 2;
const MIN_REFINEMENT_IMPROVEMENT_RATIO: f64 = 0.95;

/// The two stages which can reject the debug Sparse-LU input before a correction exists.
///
/// This is intentionally diagnostic-only. The production FIM route does not select this
/// backend, and this status must not be used as a nonlinear-convergence verdict.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SparseLuPreparation {
    MatrixBuildFailed,
    FactorizationFailed,
    Factorized,
}

#[cfg(test)]
impl SparseLuPreparation {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::MatrixBuildFailed => "matrix-build-failed",
            Self::FactorizationFailed => "factorization-failed",
            Self::Factorized => "factorized",
        }
    }
}

/// Structural facts about a `CsMat` before it is handed to faer's sparse LU.
///
/// `potential_zero_pivot_rows` only identifies a missing or exactly-zero *diagonal* entry. It
/// is not a claim about the factorization's actual pivot sequence, which faer owns.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SparseLuStructureDiagnostics {
    pub(super) empty_rows: usize,
    pub(super) empty_columns: usize,
    pub(super) duplicate_entries: usize,
    pub(super) non_finite_entries: usize,
    pub(super) all_zero_rows: usize,
    pub(super) potential_zero_pivot_rows: usize,
    pub(super) empty_column_indices: Vec<usize>,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SparseLuDiagnostics {
    pub(super) structure: SparseLuStructureDiagnostics,
    pub(super) preparation: SparseLuPreparation,
}

fn build_sparse_row_matrix(matrix: &CsMat<f64>) -> Option<SparseRowMat<usize, f64>> {
    let mut triplets = Vec::with_capacity(matrix.nnz());
    for (row, vec) in matrix.outer_iterator().enumerate() {
        for (&col, &val) in vec.indices().iter().zip(vec.data().iter()) {
            triplets.push(Triplet::new(row, col, val));
        }
    }

    SparseRowMat::<usize, f64>::try_new_from_triplets(matrix.rows(), matrix.cols(), &triplets).ok()
}

/// Inspect the exact sparse representation and separately attempt construction and
/// factorization. This performs no Newton update and is used only by the native offline lab.
#[cfg(test)]
pub(super) fn diagnose(jacobian: &CsMat<f64>) -> SparseLuDiagnostics {
    let mut seen_entries = HashSet::with_capacity(jacobian.nnz());
    let mut column_entries = vec![0usize; jacobian.cols()];
    let mut empty_rows = 0usize;
    let mut all_zero_rows = 0usize;
    let mut duplicate_entries = 0usize;
    let mut non_finite_entries = 0usize;
    let mut potential_zero_pivot_rows = 0usize;

    for (row, vector) in jacobian.outer_iterator().enumerate() {
        if vector.nnz() == 0 {
            empty_rows += 1;
        }

        let mut row_has_nonzero = false;
        let mut diagonal = None;
        for (&column, &value) in vector.indices().iter().zip(vector.data().iter()) {
            if !seen_entries.insert((row, column)) {
                duplicate_entries += 1;
            }
            column_entries[column] += 1;
            row_has_nonzero |= value != 0.0;
            non_finite_entries += usize::from(!value.is_finite());
            if row == column {
                diagonal = Some(value);
            }
        }
        all_zero_rows += usize::from(!row_has_nonzero);
        potential_zero_pivot_rows += usize::from(diagonal.is_none_or(|value| value == 0.0));
    }

    let empty_column_indices: Vec<usize> = column_entries
        .iter()
        .enumerate()
        .filter_map(|(column, &count)| (count == 0).then_some(column))
        .collect();
    let structure = SparseLuStructureDiagnostics {
        empty_rows,
        empty_columns: empty_column_indices.len(),
        duplicate_entries,
        non_finite_entries,
        all_zero_rows,
        potential_zero_pivot_rows,
        empty_column_indices,
    };
    let preparation = match build_sparse_row_matrix(jacobian) {
        None => SparseLuPreparation::MatrixBuildFailed,
        Some(matrix) if matrix.sp_lu().is_err() => SparseLuPreparation::FactorizationFailed,
        Some(_) => SparseLuPreparation::Factorized,
    };

    SparseLuDiagnostics {
        structure,
        preparation,
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
            rhs_norm: rhs.norm(),
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
            rhs_norm: rhs.norm(),
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
        let correction =
            DVector::from_iterator(rhs.len(), (0..rhs.len()).map(|idx| correction_col[idx]));
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
        rhs_norm: rhs.norm(),
        final_residual_norm: residual_norm,
        failure_diagnostics: None,
        used_fallback,
        backend_used: FimLinearSolverKind::SparseLuDebug,
        cpr_diagnostics: None,
        total_time_ms: timer.elapsed_ms(),
        preconditioner_build_time_ms: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use sprs::TriMatI;

    use super::*;

    #[test]
    fn sparse_lu_non_strict_report_has_reduction() {
        let mut tri = TriMatI::<f64, usize>::new((2, 2));
        tri.add_triplet(0, 0, 2.0);
        tri.add_triplet(1, 1, 3.0);
        let jacobian = tri.to_csr();
        let rhs = DVector::from_vec(vec![4.0, 9.0]);
        let options = FimLinearSolveOptions {
            // Force a finite direct correction to be classified non-strict without depending on
            // a platform-specific round-off residual.
            absolute_tolerance: -1.0,
            relative_tolerance: 0.0,
            ..FimLinearSolveOptions::default()
        };

        let report = solve(&jacobian, &rhs, &options, false);

        assert!(!report.converged);
        assert!(report.solution.iter().all(|value| value.is_finite()));
        assert!((report.rhs_norm - rhs.norm()).abs() < 1e-12);
        assert!(report.reduction().is_finite());
        assert!(report.failure_diagnostics.is_none());
    }
}
