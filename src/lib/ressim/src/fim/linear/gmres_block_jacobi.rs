use std::f64;

use nalgebra::{DMatrix, DVector};
use sprs::CsMat;

use super::{
    FimLinearBlockLayout, FimLinearSolveOptions, FimLinearSolveReport, FimLinearSolverKind,
};

#[derive(Clone, Debug, PartialEq)]
struct BlockJacobiPreconditioner {
    cell_block_size: usize,
    cell_block_inverses: Vec<DMatrix<f64>>,
    scalar_tail_start: usize,
    scalar_inv_diag: Vec<f64>,
}

impl BlockJacobiPreconditioner {
    fn identity(n: usize) -> Self {
        Self {
            cell_block_size: 0,
            cell_block_inverses: Vec::new(),
            scalar_tail_start: n,
            scalar_inv_diag: Vec::new(),
        }
    }

    fn apply(&self, vector: &DVector<f64>) -> DVector<f64> {
        let mut result = DVector::zeros(vector.len());

        for (block_idx, inverse) in self.cell_block_inverses.iter().enumerate() {
            let start = block_idx * self.cell_block_size;
            let end = start + self.cell_block_size;
            let block = DVector::from_iterator(
                self.cell_block_size,
                (start..end).map(|idx| vector[idx]),
            );
            let solved = inverse * block;
            for local in 0..self.cell_block_size {
                result[start + local] = solved[local];
            }
        }

        for (tail_idx, inv_diag) in self.scalar_inv_diag.iter().enumerate() {
            let idx = self.scalar_tail_start + tail_idx;
            if idx < vector.len() {
                result[idx] = inv_diag * vector[idx];
            }
        }

        result
    }
}

fn matrix_value(matrix: &CsMat<f64>, row: usize, col: usize) -> f64 {
    matrix
        .outer_view(row)
        .and_then(|view| view.iter().find(|(index, _)| *index == col).map(|(_, value)| *value))
        .unwrap_or(0.0)
}

fn build_block_jacobi_preconditioner(
    matrix: &CsMat<f64>,
    layout: Option<FimLinearBlockLayout>,
) -> BlockJacobiPreconditioner {
    let Some(layout) = layout else {
        let scalar_inv_diag = (0..matrix.rows())
            .map(|idx| {
                let diag = matrix_value(matrix, idx, idx);
                if diag.abs() > f64::EPSILON {
                    1.0 / diag
                } else {
                    1.0
                }
            })
            .collect();
        return BlockJacobiPreconditioner {
            cell_block_size: 0,
            cell_block_inverses: Vec::new(),
            scalar_tail_start: 0,
            scalar_inv_diag,
        };
    };

    let mut cell_block_inverses = Vec::with_capacity(layout.cell_block_count);
    for block_idx in 0..layout.cell_block_count {
        let start = block_idx * layout.cell_block_size;
        let mut block = DMatrix::zeros(layout.cell_block_size, layout.cell_block_size);
        for row in 0..layout.cell_block_size {
            for col in 0..layout.cell_block_size {
                block[(row, col)] = matrix_value(matrix, start + row, start + col);
            }
        }

        let inverse = block.clone().try_inverse().unwrap_or_else(|| {
            let mut fallback = DMatrix::zeros(layout.cell_block_size, layout.cell_block_size);
            for diag_idx in 0..layout.cell_block_size {
                let diag = block[(diag_idx, diag_idx)];
                fallback[(diag_idx, diag_idx)] = if diag.abs() > f64::EPSILON {
                    1.0 / diag
                } else {
                    1.0
                };
            }
            fallback
        });
        cell_block_inverses.push(inverse);
    }

    let scalar_inv_diag = (layout.scalar_tail_start..matrix.rows())
        .map(|idx| {
            let diag = matrix_value(matrix, idx, idx);
            if diag.abs() > f64::EPSILON {
                1.0 / diag
            } else {
                1.0
            }
        })
        .collect();

    BlockJacobiPreconditioner {
        cell_block_size: layout.cell_block_size,
        cell_block_inverses,
        scalar_tail_start: layout.scalar_tail_start,
        scalar_inv_diag,
    }
}

fn cs_mat_mul_vec(matrix: &CsMat<f64>, x: &DVector<f64>) -> DVector<f64> {
    let mut y = DVector::<f64>::zeros(matrix.rows());
    for (row, vec) in matrix.outer_iterator().enumerate() {
        let mut sum = 0.0;
        for (&col, &val) in vec.indices().iter().zip(vec.data().iter()) {
            sum += val * x[col];
        }
        y[row] = sum;
    }
    y
}

fn combine_basis(basis: &[DVector<f64>], coefficients: &DVector<f64>) -> DVector<f64> {
    let mut update = DVector::zeros(basis.first().map_or(0, DVector::len));
    for (column, coefficient) in basis.iter().zip(coefficients.iter()) {
        update += column * *coefficient;
    }
    update
}

pub(super) fn solve(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
    layout: Option<FimLinearBlockLayout>,
    used_fallback: bool,
) -> FimLinearSolveReport {
    if jacobian.rows() == 0 {
        return FimLinearSolveReport {
            solution: DVector::zeros(0),
            converged: true,
            iterations: 0,
            final_residual_norm: 0.0,
            used_fallback,
            backend_used: FimLinearSolverKind::GmresIlu0,
        };
    }

    let restart = options.restart.max(2);
    let max_iterations = options.max_iterations.max(restart);
    let rhs_norm = rhs.norm();
    let tolerance = options.absolute_tolerance
        + options.relative_tolerance * rhs_norm.max(f64::EPSILON);
    let preconditioner = build_block_jacobi_preconditioner(jacobian, layout);
    let mut solution = DVector::zeros(rhs.len());
    let mut iterations = 0usize;

    while iterations < max_iterations {
        let residual = rhs - &cs_mat_mul_vec(jacobian, &solution);
        let residual_norm = residual.norm();
        if residual_norm <= tolerance {
            return FimLinearSolveReport {
                solution,
                converged: true,
                iterations,
                final_residual_norm: residual_norm,
                used_fallback,
                backend_used: FimLinearSolverKind::GmresIlu0,
            };
        }

        let preconditioned_residual = preconditioner.apply(&residual);
        let beta = preconditioned_residual.norm();
        if beta <= tolerance {
            return FimLinearSolveReport {
                solution,
                converged: true,
                iterations,
                final_residual_norm: residual_norm,
                used_fallback,
                backend_used: FimLinearSolverKind::GmresIlu0,
            };
        }

        let mut basis = Vec::with_capacity(restart + 1);
        basis.push(preconditioned_residual / beta);
        let mut hessenberg = DMatrix::<f64>::zeros(restart + 1, restart);
        let mut best_solution = solution.clone();
        let mut best_residual = residual_norm;
        let mut inner_steps = 0usize;

        for inner in 0..restart {
            let w = preconditioner.apply(&cs_mat_mul_vec(jacobian, &basis[inner]));
            let mut orthogonal = w;
            for prev in 0..=inner {
                let coeff = basis[prev].dot(&orthogonal);
                hessenberg[(prev, inner)] = coeff;
                orthogonal -= &basis[prev] * coeff;
            }

            let next_norm = orthogonal.norm();
            hessenberg[(inner + 1, inner)] = next_norm;
            if next_norm > f64::EPSILON {
                basis.push(orthogonal / next_norm);
            }

            let rows = inner + 2;
            let cols = inner + 1;
            let h_sub = hessenberg.view((0, 0), (rows, cols)).into_owned();
            let mut g = DVector::<f64>::zeros(rows);
            g[0] = beta;
            let y = h_sub
                .clone()
                .svd(true, true)
                .solve(&g, 1e-12)
                .unwrap_or_else(|_| DVector::zeros(cols));
            let candidate = &solution + combine_basis(&basis[..cols], &y);
            let candidate_residual = (rhs - &cs_mat_mul_vec(jacobian, &candidate)).norm();
            iterations += 1;
            inner_steps = cols;

            if candidate_residual < best_residual {
                best_residual = candidate_residual;
                best_solution = candidate.clone();
            }

            if candidate_residual <= tolerance || iterations >= max_iterations {
                return FimLinearSolveReport {
                    solution: candidate,
                    converged: candidate_residual <= tolerance,
                    iterations,
                    final_residual_norm: candidate_residual,
                    used_fallback,
                    backend_used: FimLinearSolverKind::GmresIlu0,
                };
            }

            if next_norm <= f64::EPSILON {
                break;
            }
        }

        if inner_steps == 0 {
            break;
        }
        solution = best_solution;
    }

    let final_residual = (rhs - &cs_mat_mul_vec(jacobian, &solution)).norm();
    FimLinearSolveReport {
        solution,
        converged: final_residual <= tolerance,
        iterations,
        final_residual_norm: final_residual,
        used_fallback,
        backend_used: FimLinearSolverKind::GmresIlu0,
    }
}