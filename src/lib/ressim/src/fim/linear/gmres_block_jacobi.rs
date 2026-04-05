use std::f64;

use nalgebra::{DMatrix, DVector};
use sprs::CsMat;

use super::{
    FimCprDiagnostics, FimLinearBlockLayout, FimLinearSolveOptions, FimLinearSolveReport,
    FimLinearSolverKind, FimPressureCoarseSolverKind,
};

const PRESSURE_DIRECT_SOLVE_ROW_THRESHOLD: usize = 512;
const PRESSURE_DEFECT_CORRECTION_MAX_ITERS: usize = 8;
const PRESSURE_DEFECT_CORRECTION_REL_TOL: f64 = 1e-2;

#[derive(Clone, Debug, PartialEq)]
struct BlockJacobiPreconditioner {
    cell_block_size: usize,
    cell_block_inverses: Vec<DMatrix<f64>>,
    scalar_tail_start: usize,
    scalar_inv_diag: Vec<f64>,
    tail_inverse: DMatrix<f64>,
    pressure_tail_coupling: Vec<Vec<f64>>,
    pressure_tail_prolongation: Vec<Vec<f64>>,
    pressure_restriction: Vec<Vec<f64>>,
    pressure_prolongation: Vec<Vec<f64>>,
    pressure_rows: Vec<Vec<(usize, f64)>>,
    pressure_dense_inverse: Option<DMatrix<f64>>,
    pressure_l_rows: Vec<Vec<(usize, f64)>>,
    pressure_u_diag: Vec<f64>,
    pressure_u_rows: Vec<Vec<(usize, f64)>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct PressureCorrectionAccumulator {
    applications: usize,
    accumulated_reduction_ratio: f64,
    last_reduction_ratio: f64,
}

impl PressureCorrectionAccumulator {
    fn record(&mut self, reduction_ratio: f64) {
        self.applications += 1;
        self.accumulated_reduction_ratio += reduction_ratio;
        self.last_reduction_ratio = reduction_ratio;
    }

    fn build_report(
        &self,
        preconditioner: &BlockJacobiPreconditioner,
    ) -> Option<FimCprDiagnostics> {
        let coarse_solver = preconditioner.pressure_coarse_solver_kind()?;
        if self.applications == 0 {
            return Some(FimCprDiagnostics {
                coarse_rows: preconditioner.pressure_rows.len(),
                coarse_solver,
                coarse_applications: 0,
                average_reduction_ratio: 1.0,
                last_reduction_ratio: 1.0,
            });
        }

        Some(FimCprDiagnostics {
            coarse_rows: preconditioner.pressure_rows.len(),
            coarse_solver,
            coarse_applications: self.applications,
            average_reduction_ratio: self.accumulated_reduction_ratio / self.applications as f64,
            last_reduction_ratio: self.last_reduction_ratio,
        })
    }
}

impl BlockJacobiPreconditioner {
    fn apply_stage_one(&self, vector: &DVector<f64>) -> DVector<f64> {
        let mut result = DVector::zeros(vector.len());

        for (block_idx, inverse) in self.cell_block_inverses.iter().enumerate() {
            let start = block_idx * self.cell_block_size;
            let end = start + self.cell_block_size;
            let block =
                DVector::from_iterator(self.cell_block_size, (start..end).map(|idx| vector[idx]));
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
    ) -> (DVector<f64>, Option<f64>) {
        let mut result = DVector::zeros(vector.len());
        let mut pressure_reduction_ratio = None;

        if use_pressure_correction && !self.pressure_u_diag.is_empty() && self.cell_block_size > 0 {
            let (pressure_correction, reduction_ratio) =
                self.solve_pressure_correction(&self.extract_pressure_rhs(vector));
            self.add_pressure_correction(&mut result, &pressure_correction);
            pressure_reduction_ratio = Some(reduction_ratio);

            // Follow the pressure solve with the global block smoother so the
            // transport and well unknowns respond to the pressure update.
            let corrected_residual = vector - &cs_mat_mul_vec(matrix, &result);
            result += self.apply_stage_one(&corrected_residual);
        } else {
            result = self.apply_stage_one(vector);
        }

        (result, pressure_reduction_ratio)
    }

    fn extract_pressure_rhs(&self, residual: &DVector<f64>) -> DVector<f64> {
        let mut rhs = DVector::zeros(self.pressure_u_diag.len());
        let tail_rhs = if self.tail_inverse.nrows() > 0 && self.scalar_tail_start < residual.len() {
            let tail_residual = DVector::from_iterator(
                self.tail_inverse.nrows(),
                (self.scalar_tail_start..residual.len()).map(|idx| residual[idx]),
            );
            Some(&self.tail_inverse * tail_residual)
        } else {
            None
        };

        for cell_idx in 0..self.pressure_u_diag.len() {
            let start = cell_idx * self.cell_block_size;
            let mut value = 0.0;
            for local in 0..self.cell_block_size {
                value += self.pressure_restriction[cell_idx][local] * residual[start + local];
            }
            if let Some(tail_rhs) = &tail_rhs {
                for (tail_idx, coupling) in self.pressure_tail_coupling[cell_idx].iter().enumerate()
                {
                    value -= coupling * tail_rhs[tail_idx];
                }
            }
            rhs[cell_idx] = value;
        }
        rhs
    }

    fn add_pressure_correction(
        &self,
        result: &mut DVector<f64>,
        pressure_correction: &DVector<f64>,
    ) {
        for (cell_idx, correction) in pressure_correction.iter().enumerate() {
            let start = cell_idx * self.cell_block_size;
            for local in 0..self.cell_block_size {
                result[start + local] += self.pressure_prolongation[cell_idx][local] * correction;
            }
        }

        for (tail_idx, prolongation_row) in self.pressure_tail_prolongation.iter().enumerate() {
            let idx = self.scalar_tail_start + tail_idx;
            if idx >= result.len() {
                continue;
            }
            let mut correction = 0.0;
            for (cell_idx, weight) in prolongation_row.iter().enumerate() {
                correction += weight * pressure_correction[cell_idx];
            }
            result[idx] += correction;
        }
    }

    fn solve_pressure_correction(&self, rhs: &DVector<f64>) -> (DVector<f64>, f64) {
        let rhs_norm = rhs.norm();
        if let Some(inverse) = &self.pressure_dense_inverse {
            let solution = inverse * rhs;
            let residual = rhs - &self.pressure_mat_vec(&solution);
            let reduction_ratio = if rhs_norm > f64::EPSILON {
                residual.norm() / rhs_norm
            } else {
                0.0
            };
            return (solution, reduction_ratio);
        }

        let mut solution = DVector::zeros(rhs.len());
        let tolerance = rhs.norm().max(f64::EPSILON) * PRESSURE_DEFECT_CORRECTION_REL_TOL;

        for _ in 0..PRESSURE_DEFECT_CORRECTION_MAX_ITERS {
            let residual = rhs - &self.pressure_mat_vec(&solution);
            if residual.norm() <= tolerance {
                break;
            }
            let delta = self.apply_pressure_ilu(&residual);
            solution += delta;
        }

        let residual = rhs - &self.pressure_mat_vec(&solution);
        let reduction_ratio = if rhs_norm > f64::EPSILON {
            residual.norm() / rhs_norm
        } else {
            0.0
        };

        (solution, reduction_ratio)
    }

    fn pressure_coarse_solver_kind(&self) -> Option<FimPressureCoarseSolverKind> {
        if self.pressure_rows.is_empty() {
            None
        } else if self.pressure_dense_inverse.is_some() {
            Some(FimPressureCoarseSolverKind::ExactDense)
        } else {
            Some(FimPressureCoarseSolverKind::IluDefectCorrection)
        }
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
            let l_value = if diag.abs() > f64::EPSILON {
                sum / diag
            } else {
                0.0
            };
            l_rows[row_idx].push((col_idx, l_value));
        }

        let mut u_diag_value = diag_entry;
        for &(k, l_ik) in &l_rows[row_idx] {
            u_diag_value -= l_ik * row_entry(&u_rows[k], row_idx);
        }
        if u_diag_value.abs() <= f64::EPSILON {
            u_diag_value = if diag_entry.abs() > f64::EPSILON {
                diag_entry
            } else {
                1.0
            };
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
        .and_then(|view| {
            view.iter()
                .find(|(index, _)| *index == col)
                .map(|(_, value)| *value)
        })
        .unwrap_or(0.0)
}

fn invert_tail_block(matrix: &DMatrix<f64>) -> DMatrix<f64> {
    matrix.clone().try_inverse().unwrap_or_else(|| {
        let mut fallback = DMatrix::zeros(matrix.nrows(), matrix.ncols());
        for diag_idx in 0..matrix.nrows() {
            let diag = matrix[(diag_idx, diag_idx)];
            fallback[(diag_idx, diag_idx)] = if diag.abs() > f64::EPSILON {
                1.0 / diag
            } else {
                1.0
            };
        }
        fallback
    })
}

fn invert_pressure_block(pressure_rows: &[Vec<(usize, f64)>]) -> Option<DMatrix<f64>> {
    let n = pressure_rows.len();
    if n == 0 || n > PRESSURE_DIRECT_SOLVE_ROW_THRESHOLD {
        return None;
    }

    let mut matrix = DMatrix::zeros(n, n);
    for (row_idx, row) in pressure_rows.iter().enumerate() {
        for &(col_idx, value) in row {
            matrix[(row_idx, col_idx)] = value;
        }
    }

    matrix.try_inverse()
}

fn build_pressure_transfer_weights(block: &DMatrix<f64>) -> (Vec<f64>, Vec<f64>) {
    let size = block.nrows();
    let mut restriction = vec![0.0; size];
    let mut prolongation = vec![0.0; size];
    if size == 0 {
        return (restriction, prolongation);
    }

    restriction[0] = 1.0;
    prolongation[0] = 1.0;
    if size == 1 {
        return (restriction, prolongation);
    }

    let transport_size = size - 1;
    let mut transport_block = DMatrix::zeros(transport_size, transport_size);
    for row in 0..transport_size {
        for col in 0..transport_size {
            transport_block[(row, col)] = block[(row + 1, col + 1)];
        }
    }
    let transport_inverse = invert_tail_block(&transport_block);

    for local in 0..transport_size {
        let mut restriction_weight = 0.0;
        let mut prolongation_weight = 0.0;
        for inner in 0..transport_size {
            restriction_weight += block[(0, inner + 1)] * transport_inverse[(inner, local)];
            prolongation_weight += transport_inverse[(local, inner)] * block[(inner + 1, 0)];
        }
        restriction[local + 1] = -restriction_weight;
        prolongation[local + 1] = -prolongation_weight;
    }

    (restriction, prolongation)
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
            tail_inverse: DMatrix::zeros(0, 0),
            pressure_tail_coupling: Vec::new(),
            pressure_tail_prolongation: Vec::new(),
            pressure_restriction: Vec::new(),
            pressure_prolongation: Vec::new(),
            pressure_rows: Vec::new(),
            pressure_dense_inverse: None,
            pressure_l_rows: Vec::new(),
            pressure_u_diag: Vec::new(),
            pressure_u_rows: Vec::new(),
        };
    };

    let mut cell_block_inverses = Vec::with_capacity(layout.cell_block_count);
    let mut pressure_restriction = Vec::with_capacity(layout.cell_block_count);
    let mut pressure_prolongation = Vec::with_capacity(layout.cell_block_count);
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

        let (restriction, prolongation) = build_pressure_transfer_weights(&block);
        pressure_restriction.push(restriction);
        pressure_prolongation.push(prolongation);
    }

    let scalar_tail_count = matrix.rows().saturating_sub(layout.scalar_tail_start);
    let tail_inverse = if scalar_tail_count > 0 {
        let mut tail_block = DMatrix::zeros(scalar_tail_count, scalar_tail_count);
        for tail_row in 0..scalar_tail_count {
            for tail_col in 0..scalar_tail_count {
                tail_block[(tail_row, tail_col)] = matrix_value(
                    matrix,
                    layout.scalar_tail_start + tail_row,
                    layout.scalar_tail_start + tail_col,
                );
            }
        }
        invert_tail_block(&tail_block)
    } else {
        DMatrix::zeros(0, 0)
    };

    let mut tail_to_pressure = vec![vec![0.0; layout.cell_block_count]; scalar_tail_count];
    for tail_idx in 0..scalar_tail_count {
        let row_idx = layout.scalar_tail_start + tail_idx;
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
                tail_to_pressure[tail_idx][neighbor_block] += value * prolongation;
            }
        }
    }

    let mut pressure_rows = Vec::with_capacity(layout.cell_block_count);
    let mut pressure_tail_coupling = Vec::with_capacity(layout.cell_block_count);
    for block_idx in 0..layout.cell_block_count {
        let start = block_idx * layout.cell_block_size;
        let restriction = &pressure_restriction[block_idx];
        let mut coefficients = std::collections::BTreeMap::<usize, f64>::new();
        let mut tail_coupling = vec![0.0; scalar_tail_count];

        for local_row in 0..layout.cell_block_size {
            let row_idx = start + local_row;
            let row_weight = restriction[local_row];
            if row_weight.abs() <= f64::EPSILON {
                continue;
            }

            if let Some(view) = matrix.outer_view(row_idx) {
                for (col_idx, value) in view.iter() {
                    if col_idx >= layout.scalar_tail_start {
                        tail_coupling[col_idx - layout.scalar_tail_start] += row_weight * value;
                        continue;
                    }
                    let neighbor_block = col_idx / layout.cell_block_size;
                    let neighbor_local = col_idx % layout.cell_block_size;
                    let prolongation = pressure_prolongation[neighbor_block][neighbor_local];
                    if prolongation.abs() <= f64::EPSILON {
                        continue;
                    }
                    *coefficients.entry(neighbor_block).or_insert(0.0) +=
                        row_weight * value * prolongation;
                }
            }
        }

        if scalar_tail_count > 0 {
            let schur_weights =
                DVector::from_vec(tail_coupling.clone()).transpose() * &tail_inverse;
            for tail_idx in 0..scalar_tail_count {
                let weight = schur_weights[(0, tail_idx)];
                if weight.abs() <= f64::EPSILON {
                    continue;
                }
                for (neighbor_block, coefficient) in tail_to_pressure[tail_idx].iter().enumerate() {
                    if coefficient.abs() <= f64::EPSILON {
                        continue;
                    }
                    *coefficients.entry(neighbor_block).or_insert(0.0) -= weight * coefficient;
                }
            }
        }

        pressure_rows.push(
            coefficients
                .into_iter()
                .filter(|(_, value)| value.abs() > 1e-14)
                .collect::<Vec<_>>(),
        );
        pressure_tail_coupling.push(tail_coupling);
    }

    let mut pressure_tail_prolongation =
        vec![vec![0.0; layout.cell_block_count]; scalar_tail_count];
    if scalar_tail_count > 0 {
        for tail_row in 0..scalar_tail_count {
            for coarse_col in 0..layout.cell_block_count {
                let mut value = 0.0;
                for inner_tail in 0..scalar_tail_count {
                    value += tail_inverse[(tail_row, inner_tail)]
                        * tail_to_pressure[inner_tail][coarse_col];
                }
                pressure_tail_prolongation[tail_row][coarse_col] = -value;
            }
        }
    }

    let pressure_dense_inverse = invert_pressure_block(&pressure_rows);
    let (pressure_l_rows, pressure_u_diag, pressure_u_rows) =
        factorize_pressure_ilu0(&pressure_rows);

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
        tail_inverse,
        pressure_tail_coupling,
        pressure_tail_prolongation,
        pressure_restriction,
        pressure_prolongation,
        pressure_rows,
        pressure_dense_inverse,
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

fn back_substitute_upper(
    hessenberg: &DMatrix<f64>,
    rhs: &DVector<f64>,
    size: usize,
) -> DVector<f64> {
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
            cpr_diagnostics: None,
        };
    }

    let restart = options.restart.max(2);
    let max_iterations = options.max_iterations.max(restart);
    let rhs_norm = rhs.norm();
    let tolerance =
        options.absolute_tolerance + options.relative_tolerance * rhs_norm.max(f64::EPSILON);
    let preconditioner = build_block_jacobi_preconditioner(jacobian, layout);
    let use_pressure_correction = options.kind == FimLinearSolverKind::FgmresCpr;
    let mut solution = DVector::zeros(rhs.len());
    let mut iterations = 0usize;
    let mut pressure_correction_stats = PressureCorrectionAccumulator::default();

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
                cpr_diagnostics: pressure_correction_stats.build_report(&preconditioner),
            };
        }

        let (preconditioned_residual, pressure_reduction_ratio) =
            preconditioner.apply(jacobian, &residual, use_pressure_correction);
        if let Some(reduction_ratio) = pressure_reduction_ratio {
            pressure_correction_stats.record(reduction_ratio);
        }
        let beta = preconditioned_residual.norm();
        if beta <= tolerance {
            return FimLinearSolveReport {
                solution,
                converged: true,
                iterations,
                final_residual_norm: residual_norm,
                used_fallback,
                backend_used,
                cpr_diagnostics: pressure_correction_stats.build_report(&preconditioner),
            };
        }

        let mut basis = Vec::with_capacity(restart + 1);
        basis.push(preconditioned_residual / beta);
        let mut hessenberg = DMatrix::<f64>::zeros(restart + 1, restart);
        let mut best_solution = solution.clone();
        let best_residual = residual_norm;
        let mut givens_cosines = vec![0.0; restart];
        let mut givens_sines = vec![0.0; restart];
        let mut rotated_rhs = DVector::<f64>::zeros(restart + 1);
        rotated_rhs[0] = beta;
        let mut inner_steps = 0usize;

        for inner in 0..restart {
            if inner >= basis.len() {
                break;
            }
            let (w, pressure_reduction_ratio) = preconditioner.apply(
                jacobian,
                &cs_mat_mul_vec(jacobian, &basis[inner]),
                use_pressure_correction,
            );
            if let Some(reduction_ratio) = pressure_reduction_ratio {
                pressure_correction_stats.record(reduction_ratio);
            }
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

            let (cosine, sine) =
                compute_givens_rotation(hessenberg[(inner, inner)], hessenberg[(inner + 1, inner)]);
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

            let (rhs_upper, rhs_lower) =
                apply_givens_rotation(rotated_rhs[inner], rotated_rhs[inner + 1], cosine, sine);
            rotated_rhs[inner] = rhs_upper;
            rotated_rhs[inner + 1] = rhs_lower;

            iterations += 1;
            inner_steps = inner + 1;

            // Use the Givens rotation residual estimate instead of computing
            // a full matrix-vector product at every inner iteration.
            let estimated_residual = rotated_rhs[inner + 1].abs();

            if estimated_residual <= tolerance
                || iterations >= max_iterations
                || next_norm <= f64::EPSILON
            {
                // Construct the actual solution only when we need it.
                let cols = inner + 1;
                let y = back_substitute_upper(&hessenberg, &rotated_rhs, cols);
                let candidate = &solution + combine_basis(&basis[..cols], &y);
                let candidate_residual = (rhs - &cs_mat_mul_vec(jacobian, &candidate)).norm();

                if candidate_residual <= tolerance || iterations >= max_iterations {
                    return FimLinearSolveReport {
                        solution: candidate,
                        converged: candidate_residual <= tolerance,
                        iterations,
                        final_residual_norm: candidate_residual,
                        used_fallback,
                        backend_used,
                        cpr_diagnostics: pressure_correction_stats.build_report(&preconditioner),
                    };
                }

                if candidate_residual < best_residual {
                    best_solution = candidate;
                }
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
        cpr_diagnostics: pressure_correction_stats.build_report(&preconditioner),
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
        let (applied, _) = preconditioner.apply(&matrix, &rhs, true);

        assert!(applied[0].abs() > 1e-12);
        assert!(applied[1].abs() > 1e-12);
        assert!(applied[2].abs() < 1e-12);
    }

    #[test]
    fn pressure_rhs_accounts_for_tail_schur_coupling() {
        let mut tri = TriMatI::<f64, usize>::new((4, 4));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 3, 2.0);
        tri.add_triplet(1, 1, 3.0);
        tri.add_triplet(2, 2, 1.0);
        tri.add_triplet(3, 0, 3.0);
        tri.add_triplet(3, 3, 5.0);
        let matrix = tri.to_csr();

        let preconditioner = build_block_jacobi_preconditioner(
            &matrix,
            Some(FimLinearBlockLayout {
                cell_block_count: 1,
                cell_block_size: 3,
                scalar_tail_start: 3,
            }),
        );

        let rhs = DVector::from_vec(vec![0.0, 0.0, 0.0, 1.0]);
        let coarse_rhs = preconditioner.extract_pressure_rhs(&rhs);

        assert!(coarse_rhs[0].abs() > 1e-12);
    }

    #[test]
    fn pressure_projection_updates_tail_unknowns_from_coarse_correction() {
        let mut tri = TriMatI::<f64, usize>::new((4, 4));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 3, 2.0);
        tri.add_triplet(1, 1, 3.0);
        tri.add_triplet(2, 2, 1.0);
        tri.add_triplet(3, 0, 3.0);
        tri.add_triplet(3, 3, 5.0);
        let matrix = tri.to_csr();

        let preconditioner = build_block_jacobi_preconditioner(
            &matrix,
            Some(FimLinearBlockLayout {
                cell_block_count: 1,
                cell_block_size: 3,
                scalar_tail_start: 3,
            }),
        );

        let rhs = DVector::from_vec(vec![1.0, 0.0, 0.0, 0.0]);
        let (applied, _) = preconditioner.apply(&matrix, &rhs, true);

        assert!(applied[0].abs() > 1e-12);
        assert!(applied[3].abs() > 1e-12);
    }

    #[test]
    fn pressure_correction_uses_exact_dense_inverse_when_small() {
        let mut tri = TriMatI::<f64, usize>::new((6, 6));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 3, -1.0);
        tri.add_triplet(1, 1, 2.0);
        tri.add_triplet(2, 2, 1.0);
        tri.add_triplet(3, 0, -1.5);
        tri.add_triplet(3, 3, 5.0);
        tri.add_triplet(4, 4, 3.0);
        tri.add_triplet(5, 5, 7.0);
        let matrix = tri.to_csr();

        let preconditioner = build_block_jacobi_preconditioner(
            &matrix,
            Some(FimLinearBlockLayout {
                cell_block_count: 2,
                cell_block_size: 3,
                scalar_tail_start: 6,
            }),
        );

        assert!(preconditioner.pressure_dense_inverse.is_some());

        let rhs = DVector::from_vec(vec![1.0, -2.0]);
        let (correction, _) = preconditioner.solve_pressure_correction(&rhs);
        let residual = rhs - preconditioner.pressure_mat_vec(&correction);

        assert!(residual.norm() < 1e-10);
    }

    #[test]
    fn cpr_report_exposes_coarse_diagnostics() {
        let mut tri = TriMatI::<f64, usize>::new((6, 6));
        for idx in 0..6 {
            tri.add_triplet(idx, idx, 2.0 + idx as f64);
        }
        tri.add_triplet(0, 3, -0.5);
        tri.add_triplet(3, 0, -0.25);
        let matrix = tri.to_csr();
        let rhs = DVector::from_element(6, 1.0);

        let report = solve(
            &matrix,
            &rhs,
            &FimLinearSolveOptions::default(),
            Some(FimLinearBlockLayout {
                cell_block_count: 2,
                cell_block_size: 3,
                scalar_tail_start: 6,
            }),
            true,
        );

        let diagnostics = report.cpr_diagnostics.expect("expected CPR diagnostics");
        assert_eq!(diagnostics.coarse_rows, 2);
        assert_eq!(
            diagnostics.coarse_solver,
            FimPressureCoarseSolverKind::ExactDense
        );
        assert!(diagnostics.coarse_applications > 0);
        assert!(diagnostics.average_reduction_ratio <= 1.0);
    }

    #[test]
    fn pressure_transfer_weights_follow_local_schur_elimination() {
        let block = DMatrix::from_row_slice(3, 3, &[4.0, 1.0, 0.0, 2.0, 3.0, 0.0, 0.0, 0.0, 1.0]);

        let (restriction, prolongation) = build_pressure_transfer_weights(&block);

        assert!((restriction[0] - 1.0).abs() < 1e-12);
        assert!((restriction[1] + 1.0 / 3.0).abs() < 1e-12);
        assert!(restriction[2].abs() < 1e-12);
        assert!((prolongation[0] - 1.0).abs() < 1e-12);
        assert!((prolongation[1] + 2.0 / 3.0).abs() < 1e-12);
        assert!(prolongation[2].abs() < 1e-12);
    }
}
