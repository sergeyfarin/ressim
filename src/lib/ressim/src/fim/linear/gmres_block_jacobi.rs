use std::f64;

use nalgebra::{DMatrix, DVector};
use sprs::{CsMat, TriMatI};

use super::{
    FimCprDiagnostics, FimLinearBlockLayout, FimLinearFailureDiagnostics, FimLinearFailureReason,
    FimLinearRestartDiagnostics, FimLinearSolveOptions, FimLinearSolveReport, FimLinearSolverKind,
    FimPressureCoarseSolverKind,
};
use crate::fim::scaling::EquationScaling;
use crate::timing::PerfTimer;

// Coarse-factorization cost lever (2026-07-10, follow-up to `FIM-BUNDLE-P`'s P0): offline
// comparison on 599 captured systems (heavy 432 coarse rows, bounded 529) found the explicit
// dense inverse taken below this threshold costs ~45-90ms to build, while the BiCGStab+ILU0
// path already used above it costs ~0.5ms — ~100-170x cheaper — with zero convergence failures
// and residual reduction ~4e-7 (far tighter than needed) on every single captured system.
// Lowered from 512 to 300 to move the heavy case (432 rows) and the `22x22x1` control-matrix
// case (484 rows) onto the already-proven BiCGStab path; 300 keeps the smallest control-matrix
// case (`gas-rate 10x10x3`, exactly 300 rows) on the dense path, untested at that size and
// trivially cheap regardless. See `docs/FIM_BUNDLE_P_PLAN.md` for the full comparison numbers.
const PRESSURE_DIRECT_SOLVE_ROW_THRESHOLD: usize = 300;
const PRESSURE_DEFECT_CORRECTION_MAX_ITERS: usize = 50;
const PRESSURE_DEFECT_CORRECTION_REL_TOL: f64 = 1e-6;
const FULL_ILU_ROW_LIMIT: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CprFineSmootherKind {
    BlockJacobi,
    FullIlu0,
    BlockIlu0,
}

impl CprFineSmootherKind {
    fn label(self) -> &'static str {
        match self {
            Self::BlockJacobi => "block-jacobi",
            Self::FullIlu0 => "ilu0",
            Self::BlockIlu0 => "block-ilu0",
        }
    }
}

/// Pressure-restriction/prolongation variants for the CPR coarse stage. `Row0Schur` is the
/// only variant used by the production path (`solve()`, i.e. `FgmresCpr`/`GmresIlu0` as
/// dispatched from `solve_linearized_system`) — the others exist for the offline solver
/// lab (`fim/linear/solver_lab.rs`) to compare as full solves on captured real systems
/// before any live-solver change is considered (Phase 9, `FIM-LINEAR-005`/`FIM-LINEAR-007`).
/// Salvaged from the reverted in-situ probe (commit `db3bdaf`, unreachable from production
/// because it was gated on `options.verbose`) plus a new `QuasiImpes` variant matching
/// OPM's `getQuasiImpesWeights.hpp` construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) enum CprPressureRestrictionKind {
    Row0Schur,
    SummedRows,
    DiagBalancedRows,
    DominantDiagonalRow,
    LocalSchurBalanced,
    QuasiImpes,
}

impl CprPressureRestrictionKind {
    #[cfg(test)]
    pub(super) const ALL: [Self; 6] = [
        Self::Row0Schur,
        Self::SummedRows,
        Self::DiagBalancedRows,
        Self::DominantDiagonalRow,
        Self::LocalSchurBalanced,
        Self::QuasiImpes,
    ];

    #[cfg(test)]
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Row0Schur => "row0-schur",
            Self::SummedRows => "sum-rows",
            Self::DiagBalancedRows => "diag-balanced-sum",
            Self::DominantDiagonalRow => "dominant-diag-row",
            Self::LocalSchurBalanced => "local-schur-balanced",
            Self::QuasiImpes => "quasi-impes",
        }
    }
}

fn normalize_weights(weights: &mut [f64]) {
    let max_abs = weights
        .iter()
        .fold(0.0_f64, |max_abs, value| max_abs.max(value.abs()));
    if max_abs > f64::EPSILON {
        for value in weights {
            *value /= max_abs;
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FimIlu0Factors {
    l_rows: Vec<Vec<(usize, f64)>>,
    u_diag: Vec<f64>,
    u_rows: Vec<Vec<(usize, f64)>>,
}

impl FimIlu0Factors {
    fn apply(&self, rhs: &DVector<f64>) -> DVector<f64> {
        let mut y = DVector::zeros(rhs.len());
        for row_idx in 0..rhs.len() {
            let mut sum = rhs[row_idx];
            for &(col_idx, value) in &self.l_rows[row_idx] {
                sum -= value * y[col_idx];
            }
            y[row_idx] = sum;
        }

        let mut x = DVector::zeros(rhs.len());
        for row_idx in (0..rhs.len()).rev() {
            let mut sum = y[row_idx];
            for &(col_idx, value) in &self.u_rows[row_idx] {
                sum -= value * x[col_idx];
            }
            let diag = self.u_diag[row_idx];
            x[row_idx] = if diag.abs() > f64::EPSILON {
                sum / diag
            } else {
                sum
            };
        }

        x
    }
}

/// Block-ILU(0) on the natural per-cell `cell_block_size × cell_block_size` reservoir blocks
/// (matching OPM's `Dune::BCRSMatrix<MatrixBlock<Scalar,3,3>>` block-structured ILU0), with the
/// well-BHP/perforation tail kept scalar and factorized independently (Option A from the Phase
/// 10 plan: wells are not naturally block-sized or periodically located, and OPM itself handles
/// wells via a separate operator alongside its block-ILU0 reservoir smoother, not folded into
/// it — so a scalar, uncoupled tail factorization is architecturally consistent with OPM, not a
/// compromise). No cross-block fill between the cell region and the tail; the CPR pressure-
/// correction stage already carries that coupling via `tail_inverse`/`pressure_tail_coupling`.
#[derive(Clone, Debug, PartialEq)]
struct FimBlockIlu0Factors {
    block_size: usize,
    block_count: usize,
    l_block_rows: Vec<Vec<(usize, DMatrix<f64>)>>,
    u_diag_inv: Vec<DMatrix<f64>>,
    u_block_rows: Vec<Vec<(usize, DMatrix<f64>)>>,
    tail_start: usize,
    tail: Option<FimIlu0Factors>,
}

impl FimBlockIlu0Factors {
    fn apply(&self, rhs: &DVector<f64>) -> DVector<f64> {
        let n = rhs.len();
        let mut result = DVector::zeros(n);

        // Block forward/back substitution over the reservoir-cell region, mirroring
        // `FimIlu0Factors::apply`'s scalar sweep exactly, one block at a time.
        let mut y: Vec<DVector<f64>> = Vec::with_capacity(self.block_count);
        for i in 0..self.block_count {
            let start = i * self.block_size;
            let mut rhs_block = DVector::from_iterator(
                self.block_size,
                (0..self.block_size).map(|l| rhs[start + l]),
            );
            for &(k, ref l_ik) in &self.l_block_rows[i] {
                rhs_block -= l_ik * &y[k];
            }
            y.push(rhs_block);
        }

        let mut x: Vec<DVector<f64>> = vec![DVector::zeros(self.block_size); self.block_count];
        for i in (0..self.block_count).rev() {
            let mut rhs_block = y[i].clone();
            for &(j, ref u_ij) in &self.u_block_rows[i] {
                rhs_block -= u_ij * &x[j];
            }
            x[i] = &self.u_diag_inv[i] * rhs_block;
        }

        for i in 0..self.block_count {
            let start = i * self.block_size;
            for local in 0..self.block_size {
                result[start + local] = x[i][local];
            }
        }

        if let Some(tail) = &self.tail {
            let tail_n = n - self.tail_start;
            let tail_rhs =
                DVector::from_iterator(tail_n, (0..tail_n).map(|l| rhs[self.tail_start + l]));
            let tail_x = tail.apply(&tail_rhs);
            for local in 0..tail_n {
                result[self.tail_start + local] = tail_x[local];
            }
        }

        result
    }
}

fn factorize_block_ilu0(
    matrix: &CsMat<f64>,
    layout: FimLinearBlockLayout,
) -> Option<FimBlockIlu0Factors> {
    let block_size = layout.cell_block_size;
    let block_count = layout.cell_block_count;
    if block_size == 0 || block_count == 0 {
        return None;
    }
    let cell_unknown_count = layout.cell_unknown_count();

    let extract_block = |row_block: usize, col_block: usize| -> DMatrix<f64> {
        let mut block = DMatrix::zeros(block_size, block_size);
        for row in 0..block_size {
            for col in 0..block_size {
                block[(row, col)] = matrix_value(
                    matrix,
                    row_block * block_size + row,
                    col_block * block_size + col,
                );
            }
        }
        block
    };

    // Discover the occupied block-column pattern per block-row from the original scalar
    // sparsity (ILU(0): no fill beyond this pattern).
    let mut rows: Vec<std::collections::BTreeMap<usize, DMatrix<f64>>> =
        Vec::with_capacity(block_count);
    for row_block in 0..block_count {
        let mut cols = std::collections::BTreeSet::new();
        cols.insert(row_block);
        for local in 0..block_size {
            let row_idx = row_block * block_size + local;
            let Some(row) = matrix.outer_view(row_idx) else {
                continue;
            };
            for (col_idx, value) in row.iter() {
                if col_idx >= cell_unknown_count || value.abs() <= f64::EPSILON {
                    continue;
                }
                cols.insert(col_idx / block_size);
            }
        }
        let entries = cols
            .into_iter()
            .map(|col_block| (col_block, extract_block(row_block, col_block)))
            .collect();
        rows.push(entries);
    }

    let mut l_block_rows: Vec<Vec<(usize, DMatrix<f64>)>> = vec![Vec::new(); block_count];
    let mut u_diag_inv: Vec<DMatrix<f64>> = Vec::with_capacity(block_count);
    let mut u_block_rows: Vec<Vec<(usize, DMatrix<f64>)>> = vec![Vec::new(); block_count];

    for i in 0..block_count {
        let ks: Vec<usize> = rows[i].keys().copied().filter(|&k| k < i).collect();
        for k in ks {
            let a_ik = rows[i]
                .get(&k)
                .cloned()
                .unwrap_or_else(|| DMatrix::zeros(block_size, block_size));
            let l_ik = &a_ik * &u_diag_inv[k];
            let u_k_row = u_block_rows[k].clone();
            for (j, u_kj) in &u_k_row {
                if let Some(a_ij) = rows[i].get_mut(j) {
                    *a_ij -= &l_ik * u_kj;
                }
            }
            l_block_rows[i].push((k, l_ik));
            rows[i].remove(&k);
        }

        let diag = rows[i]
            .get(&i)
            .cloned()
            .unwrap_or_else(|| DMatrix::zeros(block_size, block_size));
        u_diag_inv.push(invert_tail_block(&diag));

        for (&j, block) in rows[i].iter() {
            if j > i {
                u_block_rows[i].push((j, block.clone()));
            }
        }
    }

    let tail_start = cell_unknown_count;
    let tail_n = matrix.rows().saturating_sub(tail_start);
    let tail = if tail_n > 0 {
        let mut tri = TriMatI::<f64, usize>::new((tail_n, tail_n));
        for row in 0..tail_n {
            let Some(view) = matrix.outer_view(tail_start + row) else {
                continue;
            };
            for (col_idx, value) in view.iter() {
                if col_idx >= tail_start {
                    tri.add_triplet(row, col_idx - tail_start, *value);
                }
            }
        }
        factorize_full_ilu0(&tri.to_csr())
    } else {
        None
    };

    Some(FimBlockIlu0Factors {
        block_size,
        block_count,
        l_block_rows,
        u_diag_inv,
        u_block_rows,
        tail_start,
        tail,
    })
}

#[derive(Clone, Debug, PartialEq)]
struct BlockJacobiPreconditioner {
    fine_smoother_kind: CprFineSmootherKind,
    post_pressure_block_jacobi_experiment: bool,
    cell_block_size: usize,
    cell_block_inverses: Vec<DMatrix<f64>>,
    well_bhp_start: usize,
    well_bhp_count: usize,
    noncell_start: usize,
    perforation_tail_start: usize,
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
    full_ilu: Option<FimIlu0Factors>,
    block_ilu: Option<FimBlockIlu0Factors>,
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
                smoother_label: preconditioner.smoother_label(),
                coarse_applications: 0,
                average_reduction_ratio: 1.0,
                last_reduction_ratio: 1.0,
                build_timing: None,
            });
        }

        Some(FimCprDiagnostics {
            coarse_rows: preconditioner.pressure_rows.len(),
            coarse_solver,
            smoother_label: preconditioner.smoother_label(),
            coarse_applications: self.applications,
            average_reduction_ratio: self.accumulated_reduction_ratio / self.applications as f64,
            last_reduction_ratio: self.last_reduction_ratio,
            build_timing: None,
        })
    }
}

impl BlockJacobiPreconditioner {
    fn smoother_label(&self) -> &'static str {
        if self.post_pressure_block_jacobi_experiment {
            "ilu0/post-bj"
        } else {
            self.fine_smoother_kind.label()
        }
    }

    fn apply_block_jacobi_stage_one(&self, vector: &DVector<f64>) -> DVector<f64> {
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
            let idx = self.noncell_start + tail_idx;
            if idx < vector.len() {
                result[idx] = inv_diag * vector[idx];
            }
        }

        result
    }

    fn apply_stage_one(&self, vector: &DVector<f64>) -> DVector<f64> {
        if self.fine_smoother_kind == CprFineSmootherKind::FullIlu0 {
            if let Some(ilu) = &self.full_ilu {
                return ilu.apply(vector);
            }
        }
        if self.fine_smoother_kind == CprFineSmootherKind::BlockIlu0 {
            if let Some(ilu) = &self.block_ilu {
                return ilu.apply(vector);
            }
        }

        self.apply_block_jacobi_stage_one(vector)
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

            // Over-threshold CPR shelves currently show good coarse reduction but
            // unstable full-system post-smoothing. Keep the experiment bounded to
            // the post-coarse pass on the non-dense coarse path.
            let corrected_residual = vector - &cs_mat_mul_vec(matrix, &result);
            let stage_one_correction = if self.post_pressure_block_jacobi_experiment {
                self.apply_block_jacobi_stage_one(&corrected_residual)
            } else {
                self.apply_stage_one(&corrected_residual)
            };
            result += stage_one_correction;
        } else {
            result = self.apply_stage_one(vector);
        }

        (result, pressure_reduction_ratio)
    }

    fn extract_pressure_rhs(&self, residual: &DVector<f64>) -> DVector<f64> {
        let mut rhs = DVector::zeros(self.pressure_u_diag.len());
        let tail_rhs =
            if self.tail_inverse.nrows() > 0 && self.perforation_tail_start < residual.len() {
                let tail_residual = DVector::from_iterator(
                    self.tail_inverse.nrows(),
                    (self.perforation_tail_start..residual.len()).map(|idx| residual[idx]),
                );
                Some(&self.tail_inverse * tail_residual)
            } else {
                None
            };

        let cell_coarse_count = self.pressure_restriction.len();
        debug_assert_eq!(
            self.pressure_rows.len(),
            cell_coarse_count + self.well_bhp_count
        );

        for cell_idx in 0..cell_coarse_count {
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

        for well_idx in 0..self.well_bhp_count {
            let coarse_idx = cell_coarse_count + well_idx;
            let mut value = residual[self.well_bhp_start + well_idx];
            if let Some(tail_rhs) = &tail_rhs {
                for (tail_idx, coupling) in
                    self.pressure_tail_coupling[coarse_idx].iter().enumerate()
                {
                    value -= coupling * tail_rhs[tail_idx];
                }
            }
            rhs[coarse_idx] = value;
        }

        rhs
    }

    fn add_pressure_correction(
        &self,
        result: &mut DVector<f64>,
        pressure_correction: &DVector<f64>,
    ) {
        let cell_coarse_count = self.pressure_restriction.len();
        for cell_idx in 0..cell_coarse_count {
            let correction = pressure_correction[cell_idx];
            let start = cell_idx * self.cell_block_size;
            for local in 0..self.cell_block_size {
                result[start + local] += self.pressure_prolongation[cell_idx][local] * correction;
            }
        }

        for well_idx in 0..self.well_bhp_count {
            let coarse_idx = cell_coarse_count + well_idx;
            result[self.well_bhp_start + well_idx] += pressure_correction[coarse_idx];
        }

        for (tail_idx, prolongation_row) in self.pressure_tail_prolongation.iter().enumerate() {
            let idx = self.perforation_tail_start + tail_idx;
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
        let tolerance = rhs.norm().max(f64::EPSILON) * PRESSURE_DEFECT_CORRECTION_REL_TOL;

        let solution =
            self.solve_pressure_with_bicgstab(rhs, PRESSURE_DEFECT_CORRECTION_MAX_ITERS, tolerance);

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
            Some(FimPressureCoarseSolverKind::BiCgStab)
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

    fn solve_pressure_with_bicgstab(
        &self,
        rhs: &DVector<f64>,
        max_iters: usize,
        tol: f64,
    ) -> DVector<f64> {
        let mut x = DVector::zeros(rhs.len());
        let mut r = rhs - &self.pressure_mat_vec(&x);
        let r0_norm = r.norm();
        if r0_norm <= tol {
            return x;
        }

        let r_hat = r.clone();
        let mut rho_prev = 1.0;
        let mut alpha = 1.0;
        let mut omega = 1.0;
        let mut v = DVector::zeros(rhs.len());
        let mut p = DVector::zeros(rhs.len());

        for iter_idx in 0..max_iters {
            let rho = r_hat.dot(&r);
            if !rho.is_finite() || rho.abs() < f64::EPSILON {
                break;
            }

            if iter_idx == 0 {
                p = r.clone();
            } else {
                let beta = (rho / rho_prev) * (alpha / omega);
                p = &r + beta * (&p - omega * &v);
            }

            let p_hat = self.apply_pressure_ilu(&p);
            v = self.pressure_mat_vec(&p_hat);
            let v_dot = r_hat.dot(&v);
            if !v_dot.is_finite() || v_dot.abs() < f64::EPSILON {
                break;
            }

            alpha = rho / v_dot;
            let s = &r - alpha * &v;
            if s.norm() <= tol {
                x += alpha * p_hat;
                break;
            }

            let s_hat = self.apply_pressure_ilu(&s);
            let t = self.pressure_mat_vec(&s_hat);
            let t_dot_t = t.dot(&t);
            if !t_dot_t.is_finite() || t_dot_t <= f64::EPSILON {
                break;
            }

            omega = t.dot(&s) / t_dot_t;
            if !omega.is_finite() || omega.abs() < f64::EPSILON {
                break;
            }

            x += alpha * p_hat + omega * &s_hat;
            r = s - omega * t;
            if r.norm() <= tol {
                break;
            }

            rho_prev = rho;
        }

        x
    }
}

fn row_entry(entries: &[(usize, f64)], target_col: usize) -> f64 {
    match entries.binary_search_by_key(&target_col, |&(col_idx, _)| col_idx) {
        Ok(index) => entries[index].1,
        Err(_) => 0.0,
    }
}

fn sparse_row_entry(indices: &[usize], data: &[f64], target_col: usize) -> f64 {
    match indices.binary_search(&target_col) {
        Ok(index) => data[index],
        Err(_) => 0.0,
    }
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

fn factorize_full_ilu0(matrix: &CsMat<f64>) -> Option<FimIlu0Factors> {
    let n = matrix.rows();
    if n == 0 || n > FULL_ILU_ROW_LIMIT {
        return None;
    }

    let mut l_rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    let mut u_diag = vec![0.0_f64; n];
    let mut u_rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];

    for row_idx in 0..n {
        let row = matrix.outer_view(row_idx)?;
        let mut lower_cols = Vec::new();
        let mut upper_cols = Vec::new();
        let mut diag_entry = 0.0;

        for (col_idx, value) in row.iter() {
            if col_idx < row_idx {
                lower_cols.push((col_idx, *value));
            } else if col_idx == row_idx {
                diag_entry = *value;
            } else {
                upper_cols.push((col_idx, *value));
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
            if diag.abs() <= f64::EPSILON {
                return None;
            }
            l_rows[row_idx].push((col_idx, sum / diag));
        }

        let mut u_diag_value = diag_entry;
        for &(k, l_ik) in &l_rows[row_idx] {
            let upper_row = matrix.outer_view(k)?;
            u_diag_value -= l_ik * sparse_row_entry(upper_row.indices(), upper_row.data(), row_idx);
        }
        if u_diag_value.abs() <= f64::EPSILON {
            return None;
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

    Some(FimIlu0Factors {
        l_rows,
        u_diag,
        u_rows,
    })
}

pub(super) fn matrix_value(matrix: &CsMat<f64>, row: usize, col: usize) -> f64 {
    matrix
        .outer_view(row)
        .and_then(|view| {
            view.iter()
                .find(|(index, _)| *index == col)
                .map(|(_, value)| *value)
        })
        .unwrap_or(0.0)
}

pub(super) fn invert_tail_block(matrix: &DMatrix<f64>) -> DMatrix<f64> {
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

fn build_pressure_transfer_weights(
    block: &DMatrix<f64>,
    restriction_kind: CprPressureRestrictionKind,
) -> (Vec<f64>, Vec<f64>) {
    let size = block.nrows();
    let mut restriction = vec![0.0; size];
    let mut prolongation = vec![0.0; size];
    if size == 0 {
        return (restriction, prolongation);
    }

    prolongation[0] = 1.0;
    if size == 1 {
        restriction[0] = 1.0;
        return (restriction, prolongation);
    }

    match restriction_kind {
        CprPressureRestrictionKind::SummedRows => {
            restriction.fill(1.0);
            return (restriction, prolongation);
        }
        CprPressureRestrictionKind::DiagBalancedRows => {
            for row in 0..size {
                let diag = block[(row, row)].abs();
                restriction[row] = if diag > f64::EPSILON { 1.0 / diag } else { 1.0 };
            }
            normalize_weights(&mut restriction);
            return (restriction, prolongation);
        }
        CprPressureRestrictionKind::DominantDiagonalRow => {
            let dominant_row = (0..size)
                .max_by(|a, b| {
                    block[(*a, *a)]
                        .abs()
                        .partial_cmp(&block[(*b, *b)].abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(0);
            restriction[dominant_row] = 1.0;
            return (restriction, prolongation);
        }
        CprPressureRestrictionKind::QuasiImpes => {
            // OPM's `getQuasiImpesWeights.hpp`: solve `A^T w = e_pressure` using only the
            // diagonal accumulation block, i.e. w = A^{-1}.row(pressure_index). Pressure
            // is unknown index 0 in this block layout.
            let inverse = invert_tail_block(block);
            for col in 0..size {
                restriction[col] = inverse[(0, col)];
            }
            normalize_weights(&mut restriction);
            return (restriction, prolongation);
        }
        CprPressureRestrictionKind::Row0Schur | CprPressureRestrictionKind::LocalSchurBalanced => {
            restriction[0] = 1.0;
        }
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

    if restriction_kind == CprPressureRestrictionKind::LocalSchurBalanced {
        normalize_weights(&mut restriction);
    }

    (restriction, prolongation)
}

/// Bundle P (`FIM-BUNDLE-P`) P0.1: per-phase build-cost breakdown for
/// `build_block_jacobi_preconditioner`, in build-cost order. Always populated (timers are
/// cheap relative to the O(n^3)/O(nnz) work they wrap) so the offline lab can measure which
/// phases actually dominate before any reuse/factorization change is attempted.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CprBuildTiming {
    pub(crate) weights_ms: f64,
    pub(crate) coarse_assembly_ms: f64,
    /// Split from the original single `coarse_factorization_ms` (coarse-factorization cost
    /// lever follow-up, 2026-07-10): `factorize_pressure_ilu0` runs unconditionally alongside
    /// `invert_pressure_block` even when the dense inverse succeeds, so the two must be
    /// measured separately to confirm which one actually dominates before proposing a fix.
    pub(crate) dense_inverse_ms: f64,
    pub(crate) coarse_ilu0_ms: f64,
    pub(crate) fine_smoother_ms: f64,
    pub(crate) block_inverses_ms: f64,
}

fn build_block_jacobi_preconditioner(
    matrix: &CsMat<f64>,
    layout: Option<FimLinearBlockLayout>,
    fine_smoother_kind: CprFineSmootherKind,
    restriction_kind: CprPressureRestrictionKind,
) -> (BlockJacobiPreconditioner, CprBuildTiming) {
    let Some(layout) = layout else {
        let scalar_timer = PerfTimer::start();
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
        let timing = CprBuildTiming {
            block_inverses_ms: scalar_timer.elapsed_ms(),
            ..CprBuildTiming::default()
        };
        return (
            BlockJacobiPreconditioner {
                fine_smoother_kind: CprFineSmootherKind::BlockJacobi,
                post_pressure_block_jacobi_experiment: false,
                cell_block_size: 0,
                cell_block_inverses: Vec::new(),
                well_bhp_start: 0,
                well_bhp_count: 0,
                noncell_start: 0,
                perforation_tail_start: 0,
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
                full_ilu: None,
                block_ilu: None,
            },
            timing,
        );
    };

    let well_bhp_start = layout.well_bhp_start();
    let well_bhp_end = layout.well_bhp_end();
    let noncell_start = layout.noncell_start();
    let perforation_tail_start = layout.coarse_pressure_end();
    debug_assert_eq!(well_bhp_start, noncell_start);
    debug_assert_eq!(well_bhp_end, perforation_tail_start);
    debug_assert!(perforation_tail_start <= matrix.rows());

    let mut cell_block_inverses = Vec::with_capacity(layout.cell_block_count);
    let mut pressure_restriction = Vec::with_capacity(layout.cell_block_count);
    let mut pressure_prolongation = Vec::with_capacity(layout.cell_block_count);
    let mut block_inverses_ms = 0.0;
    let mut weights_ms = 0.0;
    for block_idx in 0..layout.cell_block_count {
        let start = block_idx * layout.cell_block_size;
        let mut block = DMatrix::zeros(layout.cell_block_size, layout.cell_block_size);
        for row in 0..layout.cell_block_size {
            for col in 0..layout.cell_block_size {
                block[(row, col)] = matrix_value(matrix, start + row, start + col);
            }
        }

        let block_inverse_timer = PerfTimer::start();
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
        block_inverses_ms += block_inverse_timer.elapsed_ms();
        cell_block_inverses.push(inverse);

        let weights_timer = PerfTimer::start();
        let (restriction, prolongation) = build_pressure_transfer_weights(&block, restriction_kind);
        weights_ms += weights_timer.elapsed_ms();
        pressure_restriction.push(restriction);
        pressure_prolongation.push(prolongation);
    }

    let coarse_assembly_timer = PerfTimer::start();
    let schur_tail_count = matrix.rows().saturating_sub(perforation_tail_start);
    let tail_inverse = if schur_tail_count > 0 {
        let mut tail_block = DMatrix::zeros(schur_tail_count, schur_tail_count);
        for tail_row in 0..schur_tail_count {
            for tail_col in 0..schur_tail_count {
                tail_block[(tail_row, tail_col)] = matrix_value(
                    matrix,
                    perforation_tail_start + tail_row,
                    perforation_tail_start + tail_col,
                );
            }
        }
        invert_tail_block(&tail_block)
    } else {
        DMatrix::zeros(0, 0)
    };

    let coarse_row_count = layout.coarse_pressure_unknown_count();
    let mut tail_to_pressure = vec![vec![0.0; coarse_row_count]; schur_tail_count];
    for tail_idx in 0..schur_tail_count {
        let row_idx = perforation_tail_start + tail_idx;
        if let Some(view) = matrix.outer_view(row_idx) {
            for (col_idx, value) in view.iter() {
                if col_idx >= perforation_tail_start {
                    continue;
                }
                if col_idx < well_bhp_start {
                    let neighbor_block = col_idx / layout.cell_block_size;
                    let neighbor_local = col_idx % layout.cell_block_size;
                    let prolongation = pressure_prolongation[neighbor_block][neighbor_local];
                    if prolongation.abs() <= f64::EPSILON {
                        continue;
                    }
                    tail_to_pressure[tail_idx][neighbor_block] += value * prolongation;
                } else if col_idx < well_bhp_end {
                    tail_to_pressure[tail_idx]
                        [layout.cell_block_count + (col_idx - well_bhp_start)] += value;
                }
            }
        }
    }

    let mut pressure_rows = Vec::with_capacity(coarse_row_count);
    let mut pressure_tail_coupling = Vec::with_capacity(coarse_row_count);
    for block_idx in 0..layout.cell_block_count {
        let start = block_idx * layout.cell_block_size;
        let restriction = &pressure_restriction[block_idx];
        let mut coefficients = std::collections::BTreeMap::<usize, f64>::new();
        let mut tail_coupling = vec![0.0; schur_tail_count];

        for local_row in 0..layout.cell_block_size {
            let row_idx = start + local_row;
            let row_weight = restriction[local_row];
            if row_weight.abs() <= f64::EPSILON {
                continue;
            }

            if let Some(view) = matrix.outer_view(row_idx) {
                for (col_idx, value) in view.iter() {
                    if col_idx >= perforation_tail_start {
                        tail_coupling[col_idx - perforation_tail_start] += row_weight * value;
                        continue;
                    }
                    if col_idx < well_bhp_start {
                        let neighbor_block = col_idx / layout.cell_block_size;
                        let neighbor_local = col_idx % layout.cell_block_size;
                        let prolongation = pressure_prolongation[neighbor_block][neighbor_local];
                        if prolongation.abs() <= f64::EPSILON {
                            continue;
                        }
                        *coefficients.entry(neighbor_block).or_insert(0.0) +=
                            row_weight * value * prolongation;
                    } else if col_idx < well_bhp_end {
                        *coefficients
                            .entry(layout.cell_block_count + (col_idx - well_bhp_start))
                            .or_insert(0.0) += row_weight * value;
                    }
                }
            }
        }

        if schur_tail_count > 0 {
            let schur_weights =
                DVector::from_vec(tail_coupling.clone()).transpose() * &tail_inverse;
            for tail_idx in 0..schur_tail_count {
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

    for well_idx in 0..layout.well_bhp_count {
        let row_idx = well_bhp_start + well_idx;
        let mut coefficients = std::collections::BTreeMap::<usize, f64>::new();
        let mut tail_coupling = vec![0.0; schur_tail_count];

        if let Some(view) = matrix.outer_view(row_idx) {
            for (col_idx, value) in view.iter() {
                if col_idx >= perforation_tail_start {
                    tail_coupling[col_idx - perforation_tail_start] += value;
                    continue;
                }
                if col_idx < well_bhp_start {
                    let neighbor_block = col_idx / layout.cell_block_size;
                    let neighbor_local = col_idx % layout.cell_block_size;
                    let prolongation = pressure_prolongation[neighbor_block][neighbor_local];
                    if prolongation.abs() <= f64::EPSILON {
                        continue;
                    }
                    *coefficients.entry(neighbor_block).or_insert(0.0) += value * prolongation;
                } else if col_idx < well_bhp_end {
                    *coefficients
                        .entry(layout.cell_block_count + (col_idx - well_bhp_start))
                        .or_insert(0.0) += value;
                }
            }
        }

        if schur_tail_count > 0 {
            let schur_weights =
                DVector::from_vec(tail_coupling.clone()).transpose() * &tail_inverse;
            for tail_idx in 0..schur_tail_count {
                let weight = schur_weights[(0, tail_idx)];
                if weight.abs() <= f64::EPSILON {
                    continue;
                }
                for (neighbor_coarse_idx, coefficient) in
                    tail_to_pressure[tail_idx].iter().enumerate()
                {
                    if coefficient.abs() <= f64::EPSILON {
                        continue;
                    }
                    *coefficients.entry(neighbor_coarse_idx).or_insert(0.0) -= weight * coefficient;
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

    let mut pressure_tail_prolongation = vec![vec![0.0; coarse_row_count]; schur_tail_count];
    if schur_tail_count > 0 {
        for tail_row in 0..schur_tail_count {
            for coarse_col in 0..coarse_row_count {
                let mut value = 0.0;
                for inner_tail in 0..schur_tail_count {
                    value += tail_inverse[(tail_row, inner_tail)]
                        * tail_to_pressure[inner_tail][coarse_col];
                }
                pressure_tail_prolongation[tail_row][coarse_col] = -value;
            }
        }
    }
    let coarse_assembly_ms = coarse_assembly_timer.elapsed_ms();

    let dense_inverse_timer = PerfTimer::start();
    let pressure_dense_inverse = invert_pressure_block(&pressure_rows);
    let dense_inverse_ms = dense_inverse_timer.elapsed_ms();
    let coarse_ilu0_timer = PerfTimer::start();
    let (pressure_l_rows, pressure_u_diag, pressure_u_rows) =
        factorize_pressure_ilu0(&pressure_rows);
    let coarse_ilu0_ms = coarse_ilu0_timer.elapsed_ms();
    let post_pressure_block_jacobi_experiment =
        fine_smoother_kind == CprFineSmootherKind::FullIlu0 && pressure_dense_inverse.is_none();
    let fine_smoother_timer = PerfTimer::start();
    let full_ilu = if fine_smoother_kind == CprFineSmootherKind::FullIlu0 {
        factorize_full_ilu0(matrix)
    } else {
        None
    };
    let block_ilu = if fine_smoother_kind == CprFineSmootherKind::BlockIlu0 {
        factorize_block_ilu0(matrix, layout)
    } else {
        None
    };
    let fine_smoother_ms = fine_smoother_timer.elapsed_ms();

    let scalar_inv_diag = (noncell_start..matrix.rows())
        .map(|idx| {
            let diag = matrix_value(matrix, idx, idx);
            if diag.abs() > f64::EPSILON {
                1.0 / diag
            } else {
                1.0
            }
        })
        .collect();

    let timing = CprBuildTiming {
        weights_ms,
        coarse_assembly_ms,
        dense_inverse_ms,
        coarse_ilu0_ms,
        fine_smoother_ms,
        block_inverses_ms,
    };

    (
        BlockJacobiPreconditioner {
            fine_smoother_kind,
            post_pressure_block_jacobi_experiment,
            cell_block_size: layout.cell_block_size,
            cell_block_inverses,
            well_bhp_start,
            well_bhp_count: layout.well_bhp_count,
            noncell_start,
            perforation_tail_start,
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
            full_ilu,
            block_ilu,
        },
        timing,
    )
}

pub(super) fn cs_mat_mul_vec(matrix: &CsMat<f64>, x: &DVector<f64>) -> DVector<f64> {
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

fn build_iterative_failure_diagnostics(
    reason: FimLinearFailureReason,
    tolerance: f64,
    rhs_norm: f64,
    outer_residual_norm: f64,
    preconditioned_residual_norm: Option<f64>,
    estimated_residual_norm: Option<f64>,
    candidate_residual_norm: Option<f64>,
    restart_diagnostics: Vec<FimLinearRestartDiagnostics>,
) -> FimLinearFailureDiagnostics {
    FimLinearFailureDiagnostics {
        reason,
        tolerance,
        rhs_norm,
        outer_residual_norm,
        preconditioned_residual_norm,
        estimated_residual_norm,
        candidate_residual_norm,
        restart_diagnostics,
    }
}

fn build_restart_diagnostics(
    restart_index: usize,
    start_iteration: usize,
    end_iteration: usize,
    inner_steps: usize,
    outer_residual_norm: f64,
    preconditioned_residual_norm: f64,
    best_estimated_residual_norm: Option<f64>,
    best_candidate_residual_norm: Option<f64>,
    solution_improved: bool,
) -> FimLinearRestartDiagnostics {
    FimLinearRestartDiagnostics {
        restart_index,
        start_iteration,
        end_iteration,
        inner_steps,
        outer_residual_norm,
        preconditioned_residual_norm,
        best_estimated_residual_norm,
        best_candidate_residual_norm,
        solution_improved,
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

const TINY_RESIDUAL_TAIL_MIN_RESTART_INDEX: usize = 3;
const TINY_RESIDUAL_TAIL_ESTIMATE_FACTOR: f64 = 0.1;
const TINY_RESIDUAL_TAIL_WORSENING_FACTOR: f64 = 2.0;
const TINY_RESIDUAL_TAIL_ACCEPT_FACTOR: f64 = 8.0;
const TINY_RESIDUAL_TAIL_TERMINATE_FACTOR: f64 = 128.0;
const DEAD_STATE_DETECTOR_RESTART_INDEX: usize = 1;
const DEAD_STATE_MIN_OUTER_FACTOR: f64 = 1024.0;
const DEAD_STATE_MIN_ESTIMATE_FACTOR: f64 = 16.0;
const DEAD_STATE_MIN_PRECONDITIONED_RATIO: f64 = 4.0;
const DEAD_STATE_MIN_CANDIDATE_WORSENING: f64 = 1.05;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TinyResidualTailAction {
    Continue,
    AcceptCurrentIterate,
    TerminateStagnation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeadStateAction {
    Continue,
    Terminate,
}

fn classify_tiny_residual_tail_action(
    restart_index: usize,
    tolerance: f64,
    outer_residual_norm: f64,
    preconditioned_residual_norm: f64,
    estimated_residual_norm: f64,
    candidate_residual_norm: f64,
) -> TinyResidualTailAction {
    if restart_index < TINY_RESIDUAL_TAIL_MIN_RESTART_INDEX
        || estimated_residual_norm > tolerance * TINY_RESIDUAL_TAIL_ESTIMATE_FACTOR
        || candidate_residual_norm < outer_residual_norm * TINY_RESIDUAL_TAIL_WORSENING_FACTOR
    {
        return TinyResidualTailAction::Continue;
    }

    if outer_residual_norm <= tolerance * TINY_RESIDUAL_TAIL_ACCEPT_FACTOR
        && preconditioned_residual_norm <= tolerance * TINY_RESIDUAL_TAIL_ACCEPT_FACTOR
    {
        return TinyResidualTailAction::AcceptCurrentIterate;
    }

    if outer_residual_norm <= tolerance * TINY_RESIDUAL_TAIL_TERMINATE_FACTOR
        && preconditioned_residual_norm <= tolerance * TINY_RESIDUAL_TAIL_TERMINATE_FACTOR
    {
        return TinyResidualTailAction::TerminateStagnation;
    }

    TinyResidualTailAction::Continue
}

fn classify_dead_state_action(
    restart_index: usize,
    restart_size: usize,
    inner_steps: usize,
    solution_improved: bool,
    tolerance: f64,
    outer_residual_norm: f64,
    preconditioned_residual_norm: f64,
    estimated_residual_norm: f64,
    candidate_residual_norm: f64,
) -> DeadStateAction {
    if restart_index != DEAD_STATE_DETECTOR_RESTART_INDEX
        || inner_steps < restart_size
        || solution_improved
        || outer_residual_norm < tolerance * DEAD_STATE_MIN_OUTER_FACTOR
        || estimated_residual_norm < tolerance * DEAD_STATE_MIN_ESTIMATE_FACTOR
        || preconditioned_residual_norm < outer_residual_norm * DEAD_STATE_MIN_PRECONDITIONED_RATIO
        || candidate_residual_norm < outer_residual_norm * DEAD_STATE_MIN_CANDIDATE_WORSENING
    {
        return DeadStateAction::Continue;
    }

    DeadStateAction::Terminate
}

fn solve_with_cpr_fine_smoother(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
    layout: Option<FimLinearBlockLayout>,
    used_fallback: bool,
    cpr_fine_smoother_kind: CprFineSmootherKind,
    restriction_kind: CprPressureRestrictionKind,
    equation_scaling: Option<&EquationScaling>,
) -> FimLinearSolveReport {
    let total_timer = PerfTimer::start();
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
            failure_diagnostics: None,
            used_fallback,
            backend_used,
            cpr_diagnostics: None,
            total_time_ms: total_timer.elapsed_ms(),
            preconditioner_build_time_ms: 0.0,
        };
    }

    let preconditioner_timer = PerfTimer::start();
    let (preconditioner, build_timing) = build_block_jacobi_preconditioner(
        jacobian,
        layout,
        cpr_fine_smoother_kind,
        restriction_kind,
    );
    let preconditioner_build_time_ms = preconditioner_timer.elapsed_ms();

    let mut report = run_cpr_iterative_solve(
        jacobian,
        rhs,
        options,
        used_fallback,
        backend_used,
        &preconditioner,
        equation_scaling,
        &total_timer,
    );
    report.preconditioner_build_time_ms = preconditioner_build_time_ms;
    if let Some(diagnostics) = report.cpr_diagnostics.as_mut() {
        diagnostics.build_timing = Some(build_timing);
    }
    report
}

/// Bundle P (`FIM-BUNDLE-P`) P0: the actual CPR/FGMRES iterative solve, taking an
/// already-built preconditioner by reference instead of building its own. Split out of
/// `solve_with_cpr_fine_smoother` so the offline reuse lab (`solve_reusing_stale_preconditioner`
/// below) can drive the identical solve loop against a preconditioner built from a *different*
/// system, without duplicating ~400 lines of FGMRES bookkeeping.
fn run_cpr_iterative_solve(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
    used_fallback: bool,
    backend_used: FimLinearSolverKind,
    preconditioner: &BlockJacobiPreconditioner,
    equation_scaling: Option<&EquationScaling>,
    total_timer: &PerfTimer,
) -> FimLinearSolveReport {
    let preconditioner_build_time_ms = 0.0;
    let restart = options.restart.max(2);
    let max_iterations = options.max_iterations.max(restart);
    let rhs_norm = rhs.norm();
    let tolerance =
        options.absolute_tolerance + options.relative_tolerance * rhs_norm.max(f64::EPSILON);
    // `FIM-LINEAR-008` follow-up (Step 10.1 reconciliation): the global relative-residual
    // criterion above can leave a numerically small subsystem (the well/perforation rows)
    // systematically under-resolved even while the whole-system norm is satisfied. When an
    // `EquationScaling` is supplied (always true from the production Newton call site; opt-in
    // elsewhere so every pre-existing synthetic-matrix test keeps its old behavior), require
    // every equation family's own scaled residual to also clear its own relative-reduction
    // target, using `x_0 = 0` so the family's initial scaled peak is just `scaling.family_peaks(rhs)`.
    let initial_family_peaks = equation_scaling.map(|scaling| scaling.family_peaks(rhs));
    let family_ok = |residual_vec: &DVector<f64>| -> bool {
        match (equation_scaling, &initial_family_peaks) {
            (Some(scaling), Some(initial)) => scaling
                .family_peaks(residual_vec)
                .within_relative_reduction(
                    initial,
                    options.absolute_tolerance,
                    options.relative_tolerance,
                ),
            _ => true,
        }
    };
    let use_pressure_correction = options.kind == FimLinearSolverKind::FgmresCpr;
    let mut solution = DVector::zeros(rhs.len());
    let mut iterations = 0usize;
    let mut pressure_correction_stats = PressureCorrectionAccumulator::default();
    let mut failure_reason = None;
    let mut last_outer_residual_norm = rhs_norm;
    let mut last_preconditioned_residual_norm = None;
    let mut last_estimated_residual_norm = None;
    let mut last_candidate_residual_norm = None;
    let mut restart_diagnostics = Vec::new();
    let mut terminate_tiny_residual_tail = false;
    let mut terminate_dead_state = false;

    while iterations < max_iterations {
        let residual = rhs - &cs_mat_mul_vec(jacobian, &solution);
        let residual_norm = residual.norm();
        last_outer_residual_norm = residual_norm;
        if residual_norm <= tolerance && family_ok(&residual) {
            return FimLinearSolveReport {
                solution,
                converged: true,
                iterations,
                final_residual_norm: residual_norm,
                failure_diagnostics: None,
                used_fallback,
                backend_used,
                cpr_diagnostics: if use_pressure_correction {
                    pressure_correction_stats.build_report(preconditioner)
                } else {
                    None
                },
                total_time_ms: total_timer.elapsed_ms(),
                preconditioner_build_time_ms,
            };
        }

        let (preconditioned_residual, pressure_reduction_ratio) =
            preconditioner.apply(jacobian, &residual, use_pressure_correction);
        if let Some(reduction_ratio) = pressure_reduction_ratio {
            pressure_correction_stats.record(reduction_ratio);
        }
        let beta = preconditioned_residual.norm();
        last_preconditioned_residual_norm = Some(beta);
        // Bundle Y Y1e (`docs/FIM_OPM_PARITY_PLAN.md` §8.6-§9, `FIM-LINEAR-013`): `iterations > 0`
        // guards the exact defect Y1d measured on 337 real captured systems. At `iterations ==
        // 0`, `solution` is still the untouched `x_0 = 0` initial guess (no Krylov correction has
        // been applied yet) — accepting here purely because the PRECONDITIONED residual `beta`
        // happens to be small returns the zero vector as "the solution" whenever the
        // preconditioner alone crushes `beta`, regardless of whether `x_0 = 0` is actually close
        // to correct (checked separately, correctly, by the raw-residual test above). Measured:
        // `beta` can land 12-31x under tolerance while the raw residual sits a constant 200x over
        // it (`= 1/relative_tolerance`) for well-Schur-reduced systems specifically, but nothing
        // in the check is well-Schur-specific — this guard applies to every `FgmresCpr`/
        // `GmresIlu0` call. From `iterations >= 1` onward `solution` reflects at least one real
        // Krylov correction built from a preconditioned basis, so trusting `beta` there is the
        // ordinary (unchanged) preconditioned-residual convergence test.
        if iterations > 0 && beta <= tolerance && family_ok(&residual) {
            if residual_norm > 10.0 * tolerance && std::env::var_os("FIM_WELL_SCHUR_DEBUG").is_some()
            {
                let line = format!(
                    "CPR-ACCEPT-DEBUG iterations={} beta={:.6e} residual_norm={:.6e} tolerance={:.6e} rhs_norm={:.6e}",
                    iterations, beta, residual_norm, tolerance, rhs_norm,
                );
                #[cfg(not(target_arch = "wasm32"))]
                crate::fim::trace_sink::write_line(&line);
                eprintln!("{line}");
            }
            return FimLinearSolveReport {
                solution,
                converged: true,
                iterations,
                final_residual_norm: residual_norm,
                failure_diagnostics: None,
                used_fallback,
                backend_used,
                cpr_diagnostics: if use_pressure_correction {
                    pressure_correction_stats.build_report(preconditioner)
                } else {
                    None
                },
                total_time_ms: total_timer.elapsed_ms(),
                preconditioner_build_time_ms,
            };
        }

        let mut basis = Vec::with_capacity(restart + 1);
        basis.push(preconditioned_residual / beta);
        let mut hessenberg = DMatrix::<f64>::zeros(restart + 1, restart);
        let mut best_solution = solution.clone();
        let mut givens_cosines = vec![0.0; restart];
        let mut givens_sines = vec![0.0; restart];
        let mut rotated_rhs = DVector::<f64>::zeros(restart + 1);
        rotated_rhs[0] = beta;
        let mut inner_steps = 0usize;
        let restart_index = restart_diagnostics.len() + 1;
        let restart_start_iteration = iterations;
        let mut restart_best_estimated_residual_norm = None;
        let mut restart_best_candidate_residual_norm = None;
        let mut restart_solution_improved = false;
        let mut restart_recorded = false;

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
            last_estimated_residual_norm = Some(estimated_residual);
            restart_best_estimated_residual_norm = Some(
                restart_best_estimated_residual_norm
                    .map_or(estimated_residual, |best: f64| best.min(estimated_residual)),
            );

            let estimated_trigger = estimated_residual <= tolerance;
            if estimated_trigger || iterations >= max_iterations || next_norm <= f64::EPSILON {
                // Construct the actual solution only when we need it.
                let cols = inner + 1;
                let y = back_substitute_upper(&hessenberg, &rotated_rhs, cols);
                let candidate = &solution + combine_basis(&basis[..cols], &y);
                let candidate_residual_vec = rhs - &cs_mat_mul_vec(jacobian, &candidate);
                let candidate_residual = candidate_residual_vec.norm();
                last_candidate_residual_norm = Some(candidate_residual);
                restart_best_candidate_residual_norm = Some(
                    restart_best_candidate_residual_norm
                        .map_or(candidate_residual, |best: f64| best.min(candidate_residual)),
                );

                if candidate_residual < residual_norm {
                    best_solution = candidate.clone();
                    restart_solution_improved = true;
                }

                let tiny_tail_action = classify_tiny_residual_tail_action(
                    restart_index,
                    tolerance,
                    residual_norm,
                    beta,
                    estimated_residual,
                    candidate_residual,
                );

                if tiny_tail_action == TinyResidualTailAction::AcceptCurrentIterate {
                    restart_diagnostics.push(build_restart_diagnostics(
                        restart_index,
                        restart_start_iteration,
                        iterations,
                        inner_steps,
                        residual_norm,
                        beta,
                        restart_best_estimated_residual_norm,
                        restart_best_candidate_residual_norm,
                        restart_solution_improved,
                    ));
                    return FimLinearSolveReport {
                        solution,
                        converged: true,
                        iterations,
                        final_residual_norm: residual_norm,
                        failure_diagnostics: None,
                        used_fallback,
                        backend_used,
                        cpr_diagnostics: if use_pressure_correction {
                            pressure_correction_stats.build_report(preconditioner)
                        } else {
                            None
                        },
                        total_time_ms: total_timer.elapsed_ms(),
                        preconditioner_build_time_ms,
                    };
                }

                if tiny_tail_action == TinyResidualTailAction::TerminateStagnation {
                    restart_diagnostics.push(build_restart_diagnostics(
                        restart_index,
                        restart_start_iteration,
                        iterations,
                        inner_steps,
                        residual_norm,
                        beta,
                        restart_best_estimated_residual_norm,
                        restart_best_candidate_residual_norm,
                        restart_solution_improved,
                    ));
                    restart_recorded = true;
                    failure_reason = Some(FimLinearFailureReason::RestartStagnation);
                    terminate_tiny_residual_tail = true;
                    break;
                }

                let dead_state_action = classify_dead_state_action(
                    restart_index,
                    restart,
                    inner_steps,
                    restart_solution_improved,
                    tolerance,
                    residual_norm,
                    beta,
                    estimated_residual,
                    candidate_residual,
                );

                if dead_state_action == DeadStateAction::Terminate {
                    restart_diagnostics.push(build_restart_diagnostics(
                        restart_index,
                        restart_start_iteration,
                        iterations,
                        inner_steps,
                        residual_norm,
                        beta,
                        restart_best_estimated_residual_norm,
                        restart_best_candidate_residual_norm,
                        restart_solution_improved,
                    ));
                    restart_recorded = true;
                    failure_reason = Some(FimLinearFailureReason::DeadStateDetected);
                    break;
                }

                if estimated_trigger
                    && candidate_residual > tolerance
                    && iterations < max_iterations
                    && next_norm > f64::EPSILON
                {
                    // On the over-threshold CPR path the preconditioned GMRES estimate
                    // can become over-optimistic in the asymptotic tail. Do not restart
                    // yet if the true residual still disagrees and the Krylov space can
                    // still grow inside this restart.
                    continue;
                }

                restart_diagnostics.push(build_restart_diagnostics(
                    restart_index,
                    restart_start_iteration,
                    iterations,
                    inner_steps,
                    residual_norm,
                    beta,
                    restart_best_estimated_residual_norm,
                    restart_best_candidate_residual_norm,
                    restart_solution_improved,
                ));
                restart_recorded = true;

                let family_converged =
                    candidate_residual <= tolerance && family_ok(&candidate_residual_vec);
                if family_converged || iterations >= max_iterations {
                    let converged = family_converged;
                    return FimLinearSolveReport {
                        solution: candidate,
                        converged,
                        iterations,
                        final_residual_norm: candidate_residual,
                        failure_diagnostics: if converged {
                            None
                        } else {
                            Some(build_iterative_failure_diagnostics(
                                FimLinearFailureReason::MaxIterations,
                                tolerance,
                                rhs_norm,
                                last_outer_residual_norm,
                                last_preconditioned_residual_norm,
                                last_estimated_residual_norm,
                                last_candidate_residual_norm,
                                restart_diagnostics,
                            ))
                        },
                        used_fallback,
                        backend_used,
                        cpr_diagnostics: if use_pressure_correction {
                            pressure_correction_stats.build_report(preconditioner)
                        } else {
                            None
                        },
                        total_time_ms: total_timer.elapsed_ms(),
                        preconditioner_build_time_ms,
                    };
                }

                if next_norm <= f64::EPSILON {
                    failure_reason = Some(FimLinearFailureReason::ArnoldiBreakdown);
                }
                break;
            }
        }

        if inner_steps > 0 && !restart_recorded {
            let cols = inner_steps;
            let y = back_substitute_upper(&hessenberg, &rotated_rhs, cols);
            let candidate = &solution + combine_basis(&basis[..cols], &y);
            let candidate_residual = (rhs - &cs_mat_mul_vec(jacobian, &candidate)).norm();
            last_candidate_residual_norm = Some(candidate_residual);
            restart_best_candidate_residual_norm = Some(
                restart_best_candidate_residual_norm
                    .map_or(candidate_residual, |best: f64| best.min(candidate_residual)),
            );
            if candidate_residual < residual_norm {
                best_solution = candidate;
                restart_solution_improved = true;
            }

            let dead_state_action = classify_dead_state_action(
                restart_index,
                restart,
                inner_steps,
                restart_solution_improved,
                tolerance,
                residual_norm,
                beta,
                restart_best_estimated_residual_norm.unwrap_or(f64::INFINITY),
                candidate_residual,
            );

            restart_diagnostics.push(build_restart_diagnostics(
                restart_index,
                restart_start_iteration,
                iterations,
                inner_steps,
                residual_norm,
                beta,
                restart_best_estimated_residual_norm,
                restart_best_candidate_residual_norm,
                restart_solution_improved,
            ));
            if dead_state_action == DeadStateAction::Terminate {
                failure_reason = Some(FimLinearFailureReason::DeadStateDetected);
                terminate_dead_state = true;
            }
        }

        if inner_steps == 0 {
            failure_reason = Some(FimLinearFailureReason::RestartStagnation);
            break;
        }
        if terminate_dead_state {
            break;
        }
        solution = best_solution;
        if terminate_tiny_residual_tail {
            break;
        }
    }

    let final_residual_vec = rhs - &cs_mat_mul_vec(jacobian, &solution);
    let final_residual = final_residual_vec.norm();
    let final_converged = final_residual <= tolerance && family_ok(&final_residual_vec);
    FimLinearSolveReport {
        solution,
        converged: final_converged,
        iterations,
        final_residual_norm: final_residual,
        failure_diagnostics: if final_converged {
            None
        } else {
            Some(build_iterative_failure_diagnostics(
                failure_reason.unwrap_or(FimLinearFailureReason::MaxIterations),
                tolerance,
                rhs_norm,
                last_outer_residual_norm,
                last_preconditioned_residual_norm,
                last_estimated_residual_norm,
                last_candidate_residual_norm,
                restart_diagnostics,
            ))
        },
        used_fallback,
        backend_used,
        cpr_diagnostics: if use_pressure_correction {
            pressure_correction_stats.build_report(&preconditioner)
        } else {
            None
        },
        total_time_ms: total_timer.elapsed_ms(),
        preconditioner_build_time_ms,
    }
}

pub(super) fn solve(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
    layout: Option<FimLinearBlockLayout>,
    used_fallback: bool,
    equation_scaling: Option<&EquationScaling>,
) -> FimLinearSolveReport {
    // Phase 10 (`FIM-LINEAR-008`): block-ILU0 on the natural per-cell blocks, matching OPM's
    // `Dune::BCRSMatrix<MatrixBlock<Scalar,3,3>>` block smoother. Re-applied (see `mod.rs`)
    // after a first live attempt regressed the heavy case — investigating the Newton-side
    // reconciliation (Step 10.1) the plan called for but was skipped before the first
    // live test, rather than concluding the recipe doesn't work.
    let cpr_fine_smoother_kind = if options.kind == FimLinearSolverKind::FgmresCpr {
        CprFineSmootherKind::BlockIlu0
    } else {
        CprFineSmootherKind::BlockJacobi
    };

    // Promoted 2026-07-04 (Phase 9 Step 9.3, FIM-LINEAR-005): the offline solver lab showed
    // `Row0Schur` (the historical restriction) never converges on either of two captured
    // real-failure corpora, while `QuasiImpes` (OPM's own `getQuasiImpesWeights.hpp`
    // construction) converges on ~92-93% of both. See `solve_with_restriction_kind` below
    // for the offline solver lab's full variant comparisons.
    solve_with_cpr_fine_smoother(
        jacobian,
        rhs,
        options,
        layout,
        used_fallback,
        cpr_fine_smoother_kind,
        CprPressureRestrictionKind::QuasiImpes,
        equation_scaling,
    )
}

/// Lab-only entry point (Phase 9 offline solver lab): full solve with an explicit CPR
/// pressure-restriction variant, otherwise identical to `solve()`. Never called from the
/// production path (`solve_linearized_system`) — only from `fim/linear/solver_lab.rs`.
#[cfg(test)]
pub(super) fn solve_with_restriction_kind(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
    layout: Option<FimLinearBlockLayout>,
    restriction_kind: CprPressureRestrictionKind,
) -> FimLinearSolveReport {
    let cpr_fine_smoother_kind = if options.kind == FimLinearSolverKind::FgmresCpr {
        CprFineSmootherKind::FullIlu0
    } else {
        CprFineSmootherKind::BlockJacobi
    };

    solve_with_cpr_fine_smoother(
        jacobian,
        rhs,
        options,
        layout,
        false,
        cpr_fine_smoother_kind,
        restriction_kind,
        None,
    )
}

/// Lab-only entry point (Phase 10 bundle test): full solve with an explicit fine-smoother
/// kind *and* restriction kind, so the offline lab can vary both axes independently. Never
/// called from the production path.
#[cfg(test)]
pub(super) fn solve_with_smoother_and_restriction(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
    layout: Option<FimLinearBlockLayout>,
    fine_smoother_kind: CprFineSmootherKind,
    restriction_kind: CprPressureRestrictionKind,
    equation_scaling: Option<&EquationScaling>,
) -> FimLinearSolveReport {
    solve_with_cpr_fine_smoother(
        jacobian,
        rhs,
        options,
        layout,
        false,
        fine_smoother_kind,
        restriction_kind,
        equation_scaling,
    )
}

/// Lab-only opaque handle (Bundle P `FIM-BUNDLE-P`, P0.2 staleness study): a CPR preconditioner
/// built once from one captured system, reused to solve many *different* systems without
/// rebuilding — the offline stand-in for what `cpr_reuse_interval` will do live. Kept opaque
/// (internals stay private to this module) previewing the same shape the real P1 cache will
/// need; never called from the production path.
#[cfg(test)]
pub(super) struct CprLabPreconditionerHandle(BlockJacobiPreconditioner);

/// Builds a lab preconditioner handle from one captured `(jacobian, layout)`. Build once per
/// "stale from here" origin system, then reuse the returned handle across many `k` via
/// `solve_with_prebuilt_preconditioner` instead of rebuilding per `k` (rebuilding per pair would
/// redo the O(n^3) coarse factorization up to 30x more than the staleness study needs).
#[cfg(test)]
pub(super) fn build_preconditioner_for_lab(
    jacobian: &CsMat<f64>,
    layout: Option<FimLinearBlockLayout>,
    fine_smoother_kind: CprFineSmootherKind,
    restriction_kind: CprPressureRestrictionKind,
) -> CprLabPreconditionerHandle {
    let (preconditioner, _build_timing) =
        build_block_jacobi_preconditioner(jacobian, layout, fine_smoother_kind, restriction_kind);
    CprLabPreconditionerHandle(preconditioner)
}

/// Lab-only (coarse-factorization cost lever, 2026-07-10 follow-up to `FIM-BUNDLE-P`'s P0):
/// on the SAME coarse pressure operator `build_block_jacobi_preconditioner` already assembled
/// for a captured system, times an explicit dense inverse (`try_inverse()`, today's production
/// path when coarse rows are within threshold) against an LU factorization (`nalgebra`'s
/// `.lu()`), verifies LU-based solve reproduces the inverse-based solution (correctness, not
/// assumed), and times the existing BiCGStab+ILU0 coarse solve (today's over-threshold path)
/// on the identical system for comparison. Reuses `build_block_jacobi_preconditioner` itself
/// (not a parallel reimplementation) so the coarse operator is bit-identical to production.
#[cfg(test)]
pub(super) struct CoarseFactorizationLabResult {
    pub(super) coarse_rows: usize,
    pub(super) dense_inverse_ms: f64,
    pub(super) lu_factorization_ms: f64,
    pub(super) lu_vs_inverse_solution_diff_norm: Option<f64>,
    pub(super) bicgstab_ms: f64,
    pub(super) bicgstab_reduction_ratio: f64,
}

#[cfg(test)]
pub(super) fn coarse_factorization_lab_compare(
    jacobian: &CsMat<f64>,
    layout: Option<FimLinearBlockLayout>,
) -> Option<CoarseFactorizationLabResult> {
    let (preconditioner, _build_timing) = build_block_jacobi_preconditioner(
        jacobian,
        layout,
        CprFineSmootherKind::BlockIlu0,
        CprPressureRestrictionKind::QuasiImpes,
    );
    let n = preconditioner.pressure_rows.len();
    if n == 0 {
        return None;
    }

    let mut dense = DMatrix::zeros(n, n);
    for (row_idx, row) in preconditioner.pressure_rows.iter().enumerate() {
        for &(col_idx, value) in row {
            dense[(row_idx, col_idx)] = value;
        }
    }
    let rhs = DVector::from_element(n, 1.0);

    let inv_timer = PerfTimer::start();
    let inverse = dense.clone().try_inverse();
    let dense_inverse_ms = inv_timer.elapsed_ms();

    let lu_timer = PerfTimer::start();
    let lu = dense.clone().lu();
    let lu_factorization_ms = lu_timer.elapsed_ms();

    let lu_vs_inverse_solution_diff_norm = inverse.as_ref().map(|inv| {
        let via_inverse = inv * &rhs;
        let via_lu = lu.solve(&rhs).unwrap_or_else(|| DVector::zeros(n));
        (via_inverse - via_lu).norm()
    });

    let bicg_tolerance = rhs.norm().max(f64::EPSILON) * PRESSURE_DEFECT_CORRECTION_REL_TOL;
    let bicg_timer = PerfTimer::start();
    let bicg_solution = preconditioner.solve_pressure_with_bicgstab(
        &rhs,
        PRESSURE_DEFECT_CORRECTION_MAX_ITERS,
        bicg_tolerance,
    );
    let bicgstab_ms = bicg_timer.elapsed_ms();
    let bicg_residual = &rhs - preconditioner.pressure_mat_vec(&bicg_solution);
    let bicgstab_reduction_ratio = bicg_residual.norm() / rhs.norm().max(f64::EPSILON);

    Some(CoarseFactorizationLabResult {
        coarse_rows: n,
        dense_inverse_ms,
        lu_factorization_ms,
        lu_vs_inverse_solution_diff_norm,
        bicgstab_ms,
        bicgstab_reduction_ratio,
    })
}

/// Solves `(jacobian, rhs)` reusing a preconditioner built (possibly for a different, earlier
/// system) by `build_preconditioner_for_lab`, instead of building a fresh one.
#[cfg(test)]
pub(super) fn solve_with_prebuilt_preconditioner(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
    handle: &CprLabPreconditionerHandle,
    equation_scaling: Option<&EquationScaling>,
) -> FimLinearSolveReport {
    let backend_used = if options.kind == FimLinearSolverKind::FgmresCpr {
        FimLinearSolverKind::FgmresCpr
    } else {
        FimLinearSolverKind::GmresIlu0
    };
    let total_timer = PerfTimer::start();
    run_cpr_iterative_solve(
        jacobian,
        rhs,
        options,
        false,
        backend_used,
        &handle.0,
        equation_scaling,
        &total_timer,
    )
}

#[cfg(test)]
mod tests {
    use nalgebra::DVector;

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

        let (preconditioner, _timing) = build_block_jacobi_preconditioner(
            &matrix,
            Some(FimLinearBlockLayout {
                cell_block_count: 1,
                cell_block_size: 3,
                well_bhp_count: 0,
                perforation_tail_start: 3,
            }),
            CprFineSmootherKind::BlockJacobi,
            CprPressureRestrictionKind::Row0Schur,
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

        let (preconditioner, _timing) = build_block_jacobi_preconditioner(
            &matrix,
            Some(FimLinearBlockLayout {
                cell_block_count: 1,
                cell_block_size: 3,
                well_bhp_count: 0,
                perforation_tail_start: 3,
            }),
            CprFineSmootherKind::BlockJacobi,
            CprPressureRestrictionKind::Row0Schur,
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

        let (preconditioner, _timing) = build_block_jacobi_preconditioner(
            &matrix,
            Some(FimLinearBlockLayout {
                cell_block_count: 1,
                cell_block_size: 3,
                well_bhp_count: 0,
                perforation_tail_start: 3,
            }),
            CprFineSmootherKind::BlockJacobi,
            CprPressureRestrictionKind::Row0Schur,
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

        let (preconditioner, _timing) = build_block_jacobi_preconditioner(
            &matrix,
            Some(FimLinearBlockLayout {
                cell_block_count: 2,
                cell_block_size: 3,
                well_bhp_count: 0,
                perforation_tail_start: 6,
            }),
            CprFineSmootherKind::BlockJacobi,
            CprPressureRestrictionKind::Row0Schur,
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
                well_bhp_count: 0,
                perforation_tail_start: 6,
            }),
            true,
            None,
        );

        let diagnostics = report.cpr_diagnostics.expect("expected CPR diagnostics");
        assert_eq!(diagnostics.coarse_rows, 2);
        assert_eq!(
            diagnostics.coarse_solver,
            FimPressureCoarseSolverKind::ExactDense
        );
        assert_eq!(diagnostics.smoother_label, "block-ilu0");
        assert!(diagnostics.coarse_applications > 0);
        assert!(diagnostics.average_reduction_ratio <= 1.0);
    }

    #[test]
    fn pressure_transfer_weights_follow_local_schur_elimination() {
        let block = DMatrix::from_row_slice(3, 3, &[4.0, 1.0, 0.0, 2.0, 3.0, 0.0, 0.0, 0.0, 1.0]);

        let (restriction, prolongation) =
            build_pressure_transfer_weights(&block, CprPressureRestrictionKind::Row0Schur);

        assert!((restriction[0] - 1.0).abs() < 1e-12);
        assert!((restriction[1] + 1.0 / 3.0).abs() < 1e-12);
        assert!(restriction[2].abs() < 1e-12);
        assert!((prolongation[0] - 1.0).abs() < 1e-12);
        assert!((prolongation[1] + 2.0 / 3.0).abs() < 1e-12);
        assert!(prolongation[2].abs() < 1e-12);
    }

    #[test]
    fn cpr_coarse_operator_promotes_explicit_well_bhp_rows() {
        let mut tri = TriMatI::<f64, usize>::new((5, 5));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 1, 1.0);
        tri.add_triplet(0, 3, -2.0);
        tri.add_triplet(1, 1, 3.0);
        tri.add_triplet(2, 2, 1.0);
        tri.add_triplet(3, 0, -1.5);
        tri.add_triplet(3, 3, 5.0);
        tri.add_triplet(3, 4, 1.0);
        tri.add_triplet(4, 0, -1.0);
        tri.add_triplet(4, 3, -0.5);
        tri.add_triplet(4, 4, 7.0);
        let matrix = tri.to_csr();

        let (preconditioner, _timing) = build_block_jacobi_preconditioner(
            &matrix,
            Some(FimLinearBlockLayout {
                cell_block_count: 1,
                cell_block_size: 3,
                well_bhp_count: 1,
                perforation_tail_start: 4,
            }),
            CprFineSmootherKind::BlockJacobi,
            CprPressureRestrictionKind::Row0Schur,
        );

        assert_eq!(preconditioner.pressure_rows.len(), 2);
        assert!(row_entry(&preconditioner.pressure_rows[0], 1).abs() > 1e-12);
        assert!(row_entry(&preconditioner.pressure_rows[1], 0).abs() > 1e-12);
    }

    #[test]
    fn pressure_projection_updates_explicit_well_bhp_unknowns() {
        let mut tri = TriMatI::<f64, usize>::new((5, 5));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 1, 1.0);
        tri.add_triplet(0, 3, -2.0);
        tri.add_triplet(1, 1, 3.0);
        tri.add_triplet(2, 2, 1.0);
        tri.add_triplet(3, 0, -1.5);
        tri.add_triplet(3, 3, 5.0);
        tri.add_triplet(3, 4, 1.0);
        tri.add_triplet(4, 0, -1.0);
        tri.add_triplet(4, 3, -0.5);
        tri.add_triplet(4, 4, 7.0);
        let matrix = tri.to_csr();

        let (preconditioner, _timing) = build_block_jacobi_preconditioner(
            &matrix,
            Some(FimLinearBlockLayout {
                cell_block_count: 1,
                cell_block_size: 3,
                well_bhp_count: 1,
                perforation_tail_start: 4,
            }),
            CprFineSmootherKind::BlockJacobi,
            CprPressureRestrictionKind::Row0Schur,
        );

        let rhs = DVector::from_vec(vec![1.0, 0.0, 0.0, 0.0, 0.0]);
        let (applied, _) = preconditioner.apply(&matrix, &rhs, true);

        assert!(applied[0].abs() > 1e-12);
        assert!(applied[3].abs() > 1e-12);
        assert!(applied[4].abs() > 1e-12);
    }

    fn reconstruct_ilu0_matrix(factors: &FimIlu0Factors) -> DMatrix<f64> {
        let n = factors.u_diag.len();
        let mut lower = DMatrix::<f64>::identity(n, n);
        let mut upper = DMatrix::<f64>::zeros(n, n);

        for row_idx in 0..n {
            for &(col_idx, value) in &factors.l_rows[row_idx] {
                lower[(row_idx, col_idx)] = value;
            }
            upper[(row_idx, row_idx)] = factors.u_diag[row_idx];
            for &(col_idx, value) in &factors.u_rows[row_idx] {
                upper[(row_idx, col_idx)] = value;
            }
        }

        lower * upper
    }

    #[test]
    fn full_ilu0_lower_times_upper_recovers_original_matrix() {
        let mut tri = TriMatI::<f64, usize>::new((4, 4));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 1, -1.0);
        tri.add_triplet(1, 0, -1.5);
        tri.add_triplet(1, 1, 5.0);
        tri.add_triplet(1, 2, -0.5);
        tri.add_triplet(2, 1, -0.25);
        tri.add_triplet(2, 2, 6.0);
        tri.add_triplet(2, 3, -0.75);
        tri.add_triplet(3, 2, -0.8);
        tri.add_triplet(3, 3, 3.5);
        let matrix = tri.to_csr();

        let factors = factorize_full_ilu0(&matrix).expect("expected ILU factors");
        let reconstructed = reconstruct_ilu0_matrix(&factors);

        for row_idx in 0..matrix.rows() {
            let row = matrix.outer_view(row_idx).expect("expected row");
            for (col_idx, value) in row.iter() {
                assert!((reconstructed[(row_idx, col_idx)] - value).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn full_ilu0_apply_solves_diagonal_system_exactly() {
        let mut tri = TriMatI::<f64, usize>::new((4, 4));
        tri.add_triplet(0, 0, 2.0);
        tri.add_triplet(1, 1, 3.0);
        tri.add_triplet(2, 2, 4.0);
        tri.add_triplet(3, 3, 5.0);
        let matrix = tri.to_csr();
        let factors = factorize_full_ilu0(&matrix).expect("expected ILU factors");
        let x = DVector::from_vec(vec![1.0, -2.0, 3.0, -4.0]);
        let rhs = cs_mat_mul_vec(&matrix, &x);

        let solved = factors.apply(&rhs);

        assert!((&solved - x).norm() < 1e-12);
    }

    #[test]
    fn block_ilu0_solves_exactly_when_cell_blocks_are_uncoupled() {
        // Two 2x2 diagonal-dominant cell blocks with no cross-cell coupling: block-ILU(0)
        // has nothing to approximate here, so `apply` must be exact, just like the scalar
        // ILU(0) diagonal-system test above.
        let mut tri = TriMatI::<f64, usize>::new((4, 4));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 1, 1.0);
        tri.add_triplet(1, 0, 2.0);
        tri.add_triplet(1, 1, 3.0);
        tri.add_triplet(2, 2, 5.0);
        tri.add_triplet(2, 3, -1.0);
        tri.add_triplet(3, 2, 1.0);
        tri.add_triplet(3, 3, 6.0);
        let matrix = tri.to_csr();
        let layout = FimLinearBlockLayout {
            cell_block_count: 2,
            cell_block_size: 2,
            well_bhp_count: 0,
            perforation_tail_start: 4,
        };

        let factors = factorize_block_ilu0(&matrix, layout).expect("expected block ILU factors");
        let x = DVector::from_vec(vec![1.0, -2.0, 3.0, -4.0]);
        let rhs = cs_mat_mul_vec(&matrix, &x);

        let solved = factors.apply(&rhs);

        assert!((&solved - x).norm() < 1e-10);
    }

    #[test]
    fn block_ilu0_reduces_residual_with_cell_coupling_and_scalar_tail() {
        // Two coupled 2x2 cell blocks plus a scalar well-BHP tail row coupled to cell 0.
        // ILU(0) with fill restricted to the original pattern is not expected to be exact
        // here (that would be the wrong gate) — the gate is that applying it as a
        // preconditioner meaningfully reduces the residual, i.e. it is a valid, stable
        // smoother, not a numerically broken one.
        let mut tri = TriMatI::<f64, usize>::new((5, 5));
        tri.add_triplet(0, 0, 6.0);
        tri.add_triplet(0, 1, 1.0);
        tri.add_triplet(0, 2, 0.5);
        tri.add_triplet(0, 4, 0.3);
        tri.add_triplet(1, 0, 1.0);
        tri.add_triplet(1, 1, 5.0);
        tri.add_triplet(2, 0, 0.5);
        tri.add_triplet(2, 2, 7.0);
        tri.add_triplet(2, 3, 1.0);
        tri.add_triplet(3, 2, 1.0);
        tri.add_triplet(3, 3, 6.0);
        tri.add_triplet(4, 0, 0.3);
        tri.add_triplet(4, 4, 4.0);
        let matrix = tri.to_csr();
        let layout = FimLinearBlockLayout {
            cell_block_count: 2,
            cell_block_size: 2,
            well_bhp_count: 1,
            perforation_tail_start: 5,
        };

        let factors = factorize_block_ilu0(&matrix, layout).expect("expected block ILU factors");
        let x = DVector::from_vec(vec![1.0, -2.0, 3.0, -1.5, 0.7]);
        let rhs = cs_mat_mul_vec(&matrix, &x);
        let initial_residual = rhs.norm();

        let preconditioned = factors.apply(&rhs);
        let residual_after = (&rhs - &cs_mat_mul_vec(&matrix, &preconditioned)).norm();

        assert!(preconditioned.iter().all(|v| v.is_finite()));
        assert!(
            residual_after < initial_residual,
            "block-ILU0 should reduce the residual as a preconditioner: before={initial_residual:.3e} after={residual_after:.3e}"
        );
    }

    #[test]
    fn cpr_with_full_ilu_smoother_reduces_fgmres_iteration_count() {
        let mut tri = TriMatI::<f64, usize>::new((9, 9));
        tri.add_triplet(0, 0, 8.0);
        tri.add_triplet(0, 1, 1.0);
        tri.add_triplet(0, 3, -1.0);
        tri.add_triplet(1, 1, 4.0);
        tri.add_triplet(1, 4, -1.0);
        tri.add_triplet(2, 2, 4.0);
        tri.add_triplet(2, 5, -1.0);
        tri.add_triplet(3, 0, -1.0);
        tri.add_triplet(3, 3, 8.0);
        tri.add_triplet(3, 4, 1.0);
        tri.add_triplet(3, 6, -1.0);
        tri.add_triplet(4, 1, -1.0);
        tri.add_triplet(4, 4, 4.0);
        tri.add_triplet(4, 7, -1.0);
        tri.add_triplet(5, 2, -1.0);
        tri.add_triplet(5, 5, 4.0);
        tri.add_triplet(5, 8, -1.0);
        tri.add_triplet(6, 3, -1.0);
        tri.add_triplet(6, 6, 8.0);
        tri.add_triplet(6, 7, 1.0);
        tri.add_triplet(7, 4, -1.0);
        tri.add_triplet(7, 7, 4.0);
        tri.add_triplet(8, 5, -1.0);
        tri.add_triplet(8, 8, 4.0);
        let matrix = tri.to_csr();
        let rhs = DVector::from_element(9, 1.0);
        let options = FimLinearSolveOptions {
            kind: FimLinearSolverKind::FgmresCpr,
            restart: 8,
            max_iterations: 30,
            relative_tolerance: 1e-10,
            absolute_tolerance: 1e-12,
            eliminate_wells: false,
        };
        let layout = Some(FimLinearBlockLayout {
            cell_block_count: 3,
            cell_block_size: 3,
            well_bhp_count: 0,
            perforation_tail_start: 9,
        });

        let block_jacobi_report = solve_with_cpr_fine_smoother(
            &matrix,
            &rhs,
            &options,
            layout,
            false,
            CprFineSmootherKind::BlockJacobi,
            CprPressureRestrictionKind::Row0Schur,
            None,
        );
        let ilu_report = solve_with_cpr_fine_smoother(
            &matrix,
            &rhs,
            &options,
            layout,
            false,
            CprFineSmootherKind::FullIlu0,
            CprPressureRestrictionKind::Row0Schur,
            None,
        );

        assert!(block_jacobi_report.converged);
        assert!(ilu_report.converged);
        assert!(
            ilu_report.iterations < block_jacobi_report.iterations,
            "expected full ILU smoother to reduce iterations, got ilu={} vs block-jacobi={}",
            ilu_report.iterations,
            block_jacobi_report.iterations
        );
        assert_eq!(
            block_jacobi_report
                .cpr_diagnostics
                .as_ref()
                .map(|diag| diag.smoother_label),
            Some("block-jacobi")
        );
        assert_eq!(
            ilu_report
                .cpr_diagnostics
                .as_ref()
                .map(|diag| diag.smoother_label),
            Some("ilu0")
        );
    }

    #[test]
    fn gmres_ilu0_backend_keeps_non_cpr_semantics() {
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
            &FimLinearSolveOptions {
                kind: FimLinearSolverKind::GmresIlu0,
                ..FimLinearSolveOptions::default()
            },
            Some(FimLinearBlockLayout {
                cell_block_count: 2,
                cell_block_size: 3,
                well_bhp_count: 0,
                perforation_tail_start: 6,
            }),
            false,
            None,
        );

        assert!(report.converged);
        assert_eq!(report.backend_used, FimLinearSolverKind::GmresIlu0);
        assert!(report.cpr_diagnostics.is_none());
    }

    #[test]
    fn beta_only_accept_never_fires_on_the_untouched_initial_guess() {
        // Bundle Y Y1e (`docs/FIM_OPM_PARITY_PLAN.md` §9, `FIM-LINEAR-013`): reproduces the
        // defect class Y1d measured on 337 real captured well-Schur-reduced systems, using a
        // minimal synthetic system instead of a real capture. Two independent 3x3 diagonal
        // blocks, no cross-block coupling — block 0's diagonal is inflated (`1e6`) relative to
        // block 1's (`1.0`), and all of the RHS mass sits in block 0's rows. Block-Jacobi's
        // per-block *exact* inverse crushes the preconditioned residual `beta` to ~`1.7e-6`
        // (`1e-6 * ||[1,1,1]||`) in a single application, while the raw (unpreconditioned)
        // residual at `x_0 = 0` stays at `||rhs|| ≈ 1.732` — about 200x over the default
        // `5e-3`-relative tolerance, the same order as the live gas-rate corpus. Before the
        // `iterations > 0` guard, this returned `converged: true` at `iterations == 0` with
        // `solution` still the untouched zero vector — i.e. "converged" on a state that was
        // never actually updated. The guard forces at least one real Krylov step, which (since
        // the preconditioner is exact for this diagonal system) finds the true, small-but-
        // nonzero solution and converges genuinely.
        let mut tri = TriMatI::<f64, usize>::new((6, 6));
        for idx in 0..3 {
            tri.add_triplet(idx, idx, 1.0e6);
        }
        for idx in 3..6 {
            tri.add_triplet(idx, idx, 1.0);
        }
        let matrix = tri.to_csr();
        let rhs = DVector::from_vec(vec![1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);

        let report = solve(
            &matrix,
            &rhs,
            &FimLinearSolveOptions {
                kind: FimLinearSolverKind::GmresIlu0,
                ..FimLinearSolveOptions::default()
            },
            Some(FimLinearBlockLayout {
                cell_block_count: 2,
                cell_block_size: 3,
                well_bhp_count: 0,
                perforation_tail_start: 6,
            }),
            false,
            None,
        );

        assert!(
            report.iterations >= 1,
            "beta-only accept fired at iterations == 0 again (solution never updated from x_0 \
             = 0) — the `iterations > 0` guard at gmres_block_jacobi.rs's preconditioned-residual \
             accept branch has regressed"
        );
        let true_residual = &rhs - &cs_mat_mul_vec(&matrix, &report.solution);
        assert!(
            !report.converged || true_residual.norm() < 1e-8,
            "reported converged=true but the raw residual against the original system is {:.3e} \
             — the report's solution does not actually satisfy the original problem",
            true_residual.norm()
        );
    }

    #[test]
    fn pressure_bicgstab_reduces_residual_on_small_nonsymmetric_system() {
        let mut tri = TriMatI::<f64, usize>::new((6, 6));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 3, 1.0);
        tri.add_triplet(1, 1, 2.0);
        tri.add_triplet(2, 2, 1.5);
        tri.add_triplet(3, 0, -1.5);
        tri.add_triplet(3, 3, 5.0);
        tri.add_triplet(4, 4, 3.0);
        tri.add_triplet(5, 5, 7.0);
        let matrix = tri.to_csr();

        let (preconditioner, _timing) = build_block_jacobi_preconditioner(
            &matrix,
            Some(FimLinearBlockLayout {
                cell_block_count: 2,
                cell_block_size: 3,
                well_bhp_count: 0,
                perforation_tail_start: 6,
            }),
            CprFineSmootherKind::BlockJacobi,
            CprPressureRestrictionKind::Row0Schur,
        );
        let rhs = DVector::from_vec(vec![1.0, -2.0]);

        let solution = preconditioner.solve_pressure_with_bicgstab(&rhs, 20, 1e-10);
        let residual = rhs - preconditioner.pressure_mat_vec(&solution);

        assert!(residual.norm() < 1e-8);
    }

    #[test]
    fn cpr_report_uses_bicgstab_when_coarse_system_exceeds_dense_threshold() {
        let n = PRESSURE_DIRECT_SOLVE_ROW_THRESHOLD + 1;
        let mut tri = TriMatI::<f64, usize>::new((n, n));
        for idx in 0..n {
            tri.add_triplet(idx, idx, 2.0);
        }
        let matrix = tri.to_csr();
        let rhs = DVector::from_element(n, 1.0);

        let report = solve(
            &matrix,
            &rhs,
            &FimLinearSolveOptions::default(),
            Some(FimLinearBlockLayout {
                cell_block_count: n,
                cell_block_size: 1,
                well_bhp_count: 0,
                perforation_tail_start: n,
            }),
            false,
            None,
        );

        let diagnostics = report.cpr_diagnostics.expect("expected CPR diagnostics");
        assert_eq!(diagnostics.coarse_rows, n);
        assert_eq!(
            diagnostics.coarse_solver,
            FimPressureCoarseSolverKind::BiCgStab
        );
        assert_eq!(diagnostics.smoother_label, "block-ilu0");
    }

    #[test]
    fn gmres_commits_restart_boundary_progress() {
        let mut tri = TriMatI::<f64, usize>::new((6, 6));
        for idx in 0..6 {
            tri.add_triplet(idx, idx, 4.0);
        }
        for idx in 0..5 {
            tri.add_triplet(idx, idx + 1, -1.0);
            tri.add_triplet(idx + 1, idx, -1.0);
        }
        let matrix = tri.to_csr();
        let rhs = DVector::from_element(6, 1.0);

        let report = solve(
            &matrix,
            &rhs,
            &FimLinearSolveOptions {
                kind: FimLinearSolverKind::GmresIlu0,
                restart: 2,
                max_iterations: 12,
                relative_tolerance: 1e-8,
                absolute_tolerance: 1e-10,
                eliminate_wells: false,
            },
            Some(FimLinearBlockLayout {
                cell_block_count: 6,
                cell_block_size: 1,
                well_bhp_count: 0,
                perforation_tail_start: 6,
            }),
            false,
            None,
        );

        assert!(
            report.converged,
            "expected restarted GMRES to converge, got {report:?}"
        );
        assert!(report.iterations > 2);
    }

    #[test]
    fn tiny_residual_tail_accepts_current_iterate_when_already_near_tolerance() {
        let action = classify_tiny_residual_tail_action(3, 1e-8, 4e-8, 2e-8, 5e-10, 2e-7);

        assert_eq!(action, TinyResidualTailAction::AcceptCurrentIterate);
    }

    #[test]
    fn tiny_residual_tail_terminates_when_tail_is_small_but_not_close_enough_to_accept() {
        let action = classify_tiny_residual_tail_action(3, 1e-8, 7e-7, 5e-7, 4e-10, 4e-6);

        assert_eq!(action, TinyResidualTailAction::TerminateStagnation);
    }

    #[test]
    fn tiny_residual_tail_ignores_large_residual_hard_states() {
        let action = classify_tiny_residual_tail_action(3, 1e-8, 4e0, 8e1, 5e-4, 1e1);

        assert_eq!(action, TinyResidualTailAction::Continue);
    }

    #[test]
    fn dead_state_detector_trips_on_non_improving_first_restart_hard_state() {
        let action = classify_dead_state_action(1, 30, 30, false, 1e-8, 8.0, 150.0, 5e-4, 10.0);

        assert_eq!(action, DeadStateAction::Terminate);
    }

    #[test]
    fn dead_state_detector_ignores_tiny_tail_regime() {
        let action = classify_dead_state_action(1, 30, 30, false, 1e-8, 4e-7, 4e-7, 1e-10, 2e-6);

        assert_eq!(action, DeadStateAction::Continue);
    }

    #[test]
    fn dead_state_detector_ignores_improving_first_restart() {
        let action = classify_dead_state_action(1, 30, 30, true, 1e-8, 8.0, 150.0, 5e-4, 10.0);

        assert_eq!(action, DeadStateAction::Continue);
    }

    #[test]
    fn cpr_report_counts_cells_and_bhp_rows_without_perf_tail() {
        let mut tri = TriMatI::<f64, usize>::new((5, 5));
        tri.add_triplet(0, 0, 4.0);
        tri.add_triplet(0, 1, 1.0);
        tri.add_triplet(0, 3, -2.0);
        tri.add_triplet(1, 1, 3.0);
        tri.add_triplet(2, 2, 1.0);
        tri.add_triplet(3, 0, -1.5);
        tri.add_triplet(3, 3, 5.0);
        tri.add_triplet(3, 4, 1.0);
        tri.add_triplet(4, 0, -1.0);
        tri.add_triplet(4, 3, -0.5);
        tri.add_triplet(4, 4, 7.0);
        let matrix = tri.to_csr();
        let rhs = DVector::from_element(5, 1.0);

        let report = solve(
            &matrix,
            &rhs,
            &FimLinearSolveOptions::default(),
            Some(FimLinearBlockLayout {
                cell_block_count: 1,
                cell_block_size: 3,
                well_bhp_count: 1,
                perforation_tail_start: 4,
            }),
            true,
            None,
        );

        let diagnostics = report.cpr_diagnostics.expect("expected CPR diagnostics");
        assert_eq!(diagnostics.coarse_rows, 2);
    }
}
