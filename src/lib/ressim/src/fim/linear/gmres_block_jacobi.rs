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
    pressure_restriction: Vec<Vec<f64>>,
    pressure_prolongation: Vec<Vec<f64>>,
    pressure_rows: Vec<Vec<(usize, f64)>>,
    pressure_l_rows: Vec<Vec<(usize, f64)>>,
    pressure_u_diag: Vec<f64>,
    pressure_u_rows: Vec<Vec<(usize, f64)>>,
}

impl BlockJacobiPreconditioner {
    fn identity(n: usize) -> Self {
        Self {
            cell_block_size: 0,
            cell_block_inverses: Vec::new(),
            scalar_tail_start: n,
            scalar_inv_diag: Vec::new(),
            pressure_restriction: Vec::new(),
            pressure_prolongation: Vec::new(),
            pressure_rows: Vec::new(),
            pressure_l_rows: Vec::new(),
            pressure_u_diag: Vec::new(),
            pressure_u_rows: Vec::new(),
        }
    }

    fn apply_stage_one(&self, vector: &DVector<f64>) -> DVector<f64> {
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

    fn apply(
        &self,
        matrix: &CsMat<f64>,
        vector: &DVector<f64>,
        use_pressure_correction: bool,
    ) -> DVector<f64> {
        let mut result = DVector::zeros(vector.len());

        if use_pressure_correction && !self.pressure_u_diag.is_empty() && self.cell_block_size > 0 {
            let pressure_correction =
                self.solve_pressure_correction(&self.extract_pressure_rhs(vector), 2);
            self.add_pressure_correction(&mut result, &pressure_correction);

            // Follow the pressure solve with the global block smoother so the
            // transport and well unknowns respond to the pressure update.
            let corrected_residual = vector - &cs_mat_mul_vec(matrix, &result);
            result += self.apply_stage_one(&corrected_residual);
        } else {
            result = self.apply_stage_one(vector);
        }

        result
    }

    fn extract_pressure_rhs(&self, residual: &DVector<f64>) -> DVector<f64> {
        let mut rhs = DVector::zeros(self.pressure_u_diag.len());
        for cell_idx in 0..self.pressure_u_diag.len() {
            let start = cell_idx * self.cell_block_size;
            let mut value = 0.0;
            for local in 0..self.cell_block_size {
                value += self.pressure_restriction[cell_idx][local] * residual[start + local];
            }
            rhs[cell_idx] = value;
        }
        rhs
    }

    fn add_pressure_correction(&self, result: &mut DVector<f64>, pressure_correction: &DVector<f64>) {
        for (cell_idx, correction) in pressure_correction.iter().enumerate() {
            let start = cell_idx * self.cell_block_size;
            for local in 0..self.cell_block_size {
                result[start + local] += self.pressure_prolongation[cell_idx][local] * correction;
            }
        }
    }

    fn solve_pressure_correction(&self, rhs: &DVector<f64>, iterations: usize) -> DVector<f64> {
        let mut solution = DVector::zeros(rhs.len());

        for _ in 0..iterations {
            let residual = rhs - &self.pressure_mat_vec(&solution);
            let delta = self.apply_pressure_ilu(&residual);
            solution += delta;
        }

        solution
    }

    fn pressure_mat_vec(&self, x: &DVector<f64>) -> DVector<f64> {
        let mut y = DVector::zeros(x.len());
        for (row_idx, row) in self.pressure_rows.iter().enumerate() {
            let mut sum = 0.0;
            for &(col_idx, value) in row {
                sum += value * x[col_idx];
            }
            y[row_idx] = sum;
        }
        y
    }

    fn apply_pressure_ilu(&self, rhs: &DVector<f64>) -> DVector<f64> {
        let mut y = DVector::zeros(rhs.len());
        for row_idx in 0..rhs.len() {
            let mut sum = rhs[row_idx];
            for &(col_idx, value) in &self.pressure_l_rows[row_idx] {
                sum -= value * y[col_idx];
            }
            y[row_idx] = sum;
        }

        let mut x = DVector::zeros(rhs.len());
        for row_idx in (0..rhs.len()).rev() {
            let mut sum = y[row_idx];
            for &(col_idx, value) in &self.pressure_u_rows[row_idx] {
                sum -= value * x[col_idx];
            }
            let diag = self.pressure_u_diag[row_idx];
            x[row_idx] = if diag.abs() > f64::EPSILON {
                sum / diag
            } else {
                sum
            };
        }
        x
    }
}

fn row_entry(entries: &[(usize, f64)], target_col: usize) -> f64 {
    entries
        .iter()
        .find(|(col_idx, _)| *col_idx == target_col)
        .map(|(_, value)| *value)
        .unwrap_or(0.0)
}

fn factorize_pressure_ilu0(
    pressure_rows: &[Vec<(usize, f64)>],
) -> (Vec<Vec<(usize, f64)>>, Vec<f64>, Vec<Vec<(usize, f64)>>) {
    let n = pressure_rows.len();
    let mut l_rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    let mut u_diag = vec![1.0; n];
    let mut u_rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];

    for row_idx in 0..n {
        let mut lower_cols = Vec::new();
        let mut upper_cols = Vec::new();
        let mut diag_entry = 0.0;
        for &(col_idx, value) in &pressure_rows[row_idx] {
            if col_idx < row_idx {
                lower_cols.push((col_idx, value));
            } else if col_idx == row_idx {
                diag_entry = value;
            } else {
                upper_cols.push((col_idx, value));
            }
        }

        for &(col_idx, value) in &lower_cols {
            let mut sum = value;
            for &(k, l_ik) in &l_rows[row_idx] {
                if k >= col_idx {
                    break;
                }
                sum -= l_ik * row_entry(&u_rows[k], col_idx);
            }
            let diag: f64 = u_diag[col_idx];
            let l_value = if diag.abs() > f64::EPSILON { sum / diag } else { 0.0 };
            l_rows[row_idx].push((col_idx, l_value));
        }

        let mut u_diag_value = diag_entry;
        for &(k, l_ik) in &l_rows[row_idx] {
            u_diag_value -= l_ik * row_entry(&u_rows[k], row_idx);
        }
        if u_diag_value.abs() <= f64::EPSILON {
            u_diag_value = if diag_entry.abs() > f64::EPSILON { diag_entry } else { 1.0 };
        }
        u_diag[row_idx] = u_diag_value;

        for &(col_idx, value) in &upper_cols {
            let mut sum = value;
            for &(k, l_ik) in &l_rows[row_idx] {
                sum -= l_ik * row_entry(&u_rows[k], col_idx);
            }
            u_rows[row_idx].push((col_idx, sum));
        }
    }

    (l_rows, u_diag, u_rows)
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
            pressure_restriction: Vec::new(),
            pressure_prolongation: Vec::new(),
            pressure_rows: Vec::new(),
            pressure_l_rows: Vec::new(),
            pressure_u_diag: Vec::new(),
            pressure_u_rows: Vec::new(),
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

    let pressure_restriction = cell_block_inverses
        .iter()
        .map(|inverse| (0..layout.cell_block_size).map(|local| inverse[(0, local)]).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let pressure_prolongation = cell_block_inverses
        .iter()
        .map(|inverse| (0..layout.cell_block_size).map(|local| inverse[(local, 0)]).collect::<Vec<_>>())
        .collect::<Vec<_>>();

    let mut pressure_rows = Vec::with_capacity(layout.cell_block_count);
    for block_idx in 0..layout.cell_block_count {
        let start = block_idx * layout.cell_block_size;
        let restriction = &pressure_restriction[block_idx];
        let mut coefficients = std::collections::BTreeMap::<usize, f64>::new();

        for local_row in 0..layout.cell_block_size {
            let row_idx = start + local_row;
            let row_weight = restriction[local_row];
            if row_weight.abs() <= f64::EPSILON {
                continue;
            }

            if let Some(view) = matrix.outer_view(row_idx) {
                for (col_idx, value) in view.iter() {
                    if col_idx >= layout.scalar_tail_start {
                        continue;
                    }
                    let neighbor_block = col_idx / layout.cell_block_size;
                    let neighbor_local = col_idx % layout.cell_block_size;
                    let prolongation = pressure_prolongation[neighbor_block][neighbor_local];
                    if prolongation.abs() <= f64::EPSILON {
                        continue;
                    }
                    *coefficients.entry(neighbor_block).or_insert(0.0) += row_weight * value * prolongation;
                }
            }
        }

        pressure_rows.push(
            coefficients
                .into_iter()
                .filter(|(_, value)| value.abs() > 1e-14)
                .collect::<Vec<_>>(),
        );
    }
    let (pressure_l_rows, pressure_u_diag, pressure_u_rows) = factorize_pressure_ilu0(&pressure_rows);

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
        pressure_restriction,
        pressure_prolongation,
        pressure_rows,
        pressure_l_rows,
        pressure_u_diag,
        pressure_u_rows,
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

fn apply_givens_rotation(a: f64, b: f64, cosine: f64, sine: f64) -> (f64, f64) {
    (cosine * a + sine * b, -sine * a + cosine * b)
}

fn compute_givens_rotation(a: f64, b: f64) -> (f64, f64) {
    let radius = a.hypot(b);
    if radius <= f64::EPSILON {
        (1.0, 0.0)
    } else {
        (a / radius, b / radius)
    }
}

fn back_substitute_upper(hessenberg: &DMatrix<f64>, rhs: &DVector<f64>, size: usize) -> DVector<f64> {
    let mut solution = DVector::zeros(size);
    for row in (0..size).rev() {
        let mut sum = rhs[row];
        for col in row + 1..size {
            sum -= hessenberg[(row, col)] * solution[col];
        }
        let diag = hessenberg[(row, row)];
        solution[row] = if diag.abs() > f64::EPSILON {
            sum / diag
        } else {
            0.0
        };
    }
    solution
}

pub(super) fn solve(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
    layout: Option<FimLinearBlockLayout>,
    used_fallback: bool,
) -> FimLinearSolveReport {
    let backend_used = if options.kind == FimLinearSolverKind::FgmresCpr {
        FimLinearSolverKind::FgmresCpr
    } else {
        FimLinearSolverKind::GmresIlu0
    };

    if jacobian.rows() == 0 {
        return FimLinearSolveReport {
            solution: DVector::zeros(0),
            converged: true,
            iterations: 0,
            final_residual_norm: 0.0,
            used_fallback,
            backend_used,
        };
    }

    let restart = options.restart.max(2);
    let max_iterations = options.max_iterations.max(restart);
    let rhs_norm = rhs.norm();
    let tolerance = options.absolute_tolerance
        + options.relative_tolerance * rhs_norm.max(f64::EPSILON);
    let preconditioner = build_block_jacobi_preconditioner(jacobian, layout);
    let use_pressure_correction = options.kind == FimLinearSolverKind::FgmresCpr;
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
                backend_used,
            };
        }

        let preconditioned_residual =
            preconditioner.apply(jacobian, &residual, use_pressure_correction);
        let beta = preconditioned_residual.norm();
        if beta <= tolerance {
            return FimLinearSolveReport {
                solution,
                converged: true,
                iterations,
                final_residual_norm: residual_norm,
                used_fallback,
                backend_used,
            };
        }

        let mut basis = Vec::with_capacity(restart + 1);
        basis.push(preconditioned_residual / beta);
        let mut hessenberg = DMatrix::<f64>::zeros(restart + 1, restart);
        let mut best_solution = solution.clone();
        let mut best_residual = residual_norm;
        let mut givens_cosines = vec![0.0; restart];
        let mut givens_sines = vec![0.0; restart];
        let mut rotated_rhs = DVector::<f64>::zeros(restart + 1);
        rotated_rhs[0] = beta;
        let mut inner_steps = 0usize;

        for inner in 0..restart {
            if inner >= basis.len() {
                break;
            }
            let w = preconditioner.apply(
                jacobian,
                &cs_mat_mul_vec(jacobian, &basis[inner]),
                use_pressure_correction,
            );
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

            for prev in 0..inner {
                let (rotated_upper, rotated_lower) = apply_givens_rotation(
                    hessenberg[(prev, inner)],
                    hessenberg[(prev + 1, inner)],
                    givens_cosines[prev],
                    givens_sines[prev],
                );
                hessenberg[(prev, inner)] = rotated_upper;
                hessenberg[(prev + 1, inner)] = rotated_lower;
            }

            let (cosine, sine) = compute_givens_rotation(
                hessenberg[(inner, inner)],
                hessenberg[(inner + 1, inner)],
            );
            givens_cosines[inner] = cosine;
            givens_sines[inner] = sine;

            let (rotated_diag, rotated_subdiag) = apply_givens_rotation(
                hessenberg[(inner, inner)],
                hessenberg[(inner + 1, inner)],
                cosine,
                sine,
            );
            hessenberg[(inner, inner)] = rotated_diag;
            hessenberg[(inner + 1, inner)] = rotated_subdiag;

            let (rhs_upper, rhs_lower) = apply_givens_rotation(
                rotated_rhs[inner],
                rotated_rhs[inner + 1],
                cosine,
                sine,
            );
            rotated_rhs[inner] = rhs_upper;
            rotated_rhs[inner + 1] = rhs_lower;

            let cols = inner + 1;
            let y = back_substitute_upper(&hessenberg, &rotated_rhs, cols);
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
                    backend_used,
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
        backend_used,
    }
}

#[cfg(test)]
mod tests {
    use nalgebra::DVector;
    use sprs::TriMatI;

    use super::*;

    #[test]
    fn pressure_projection_updates_entire_local_block() {
        let mut tri = TriMatI::<f64, usize>::new((3, 3));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 1, 1.0);
        tri.add_triplet(1, 0, 2.0);
        tri.add_triplet(1, 1, 3.0);
        tri.add_triplet(2, 2, 1.0);
        let matrix = tri.to_csr();

        let preconditioner = build_block_jacobi_preconditioner(
            &matrix,
            Some(FimLinearBlockLayout {
                cell_block_count: 1,
                cell_block_size: 3,
                scalar_tail_start: 3,
            }),
        );

        let rhs = DVector::from_vec(vec![1.0, 0.0, 0.0]);
        let applied = preconditioner.apply(&matrix, &rhs, true);

        assert!(applied[0].abs() > 1e-12);
        assert!(applied[1].abs() > 1e-12);
        assert!(applied[2].abs() < 1e-12);
    }
}