//! Native-only source-pinned helpers for the Y2d6 Flow 2026.04 linear-lifecycle oracle.
//!
//! This module does not participate in production dispatch. Y2d6a uses it only when the
//! dedicated capture environment variable is set; later Y2d6 gates reuse the same formulas in
//! test-only component identities.

#[cfg(test)]
use nalgebra::{DMatrix, DVector};
use nalgebra::{Matrix3, Vector3};
use sprs::{CsMat, TriMatI};

use super::FimLinearBlockLayout;
use super::capture::FimFlowLifecycleCapture;
#[cfg(test)]
use super::gmres_block_jacobi::{FimBlockIlu0Factors, factorize_block_ilu0};
use crate::ReservoirSimulator;
use crate::fim::assembly_ad::accumulation_jacobian_blocks_for_state;
use crate::fim::state::FimState;

pub(crate) const FLOW_SOURCE_TAG: &str = "release/2026.04/final";
pub(crate) const FLOW_SOURCE_COMMIT: &str = "b82f21dba405286c4c4446614dd3bf9cdebf7a2c";
pub(crate) const DUNE_ISTL_VERSION: &str = "2.11.0";

/// OPM multiplies derivatives with respect to pressure in pascals by `50e5`. ResSim's pressure
/// primary is bar, so the equivalent nondimensionalizing scale is 50 bar.
pub(crate) const TRUE_IMPES_PRESSURE_SCALE_BAR: f64 = 50.0;
#[cfg(test)]
pub(crate) const FLOW_AMG_COARSEN_TARGET: usize = 1200;

pub(crate) fn true_impes_weight(storage_block: &[[f64; 3]; 3]) -> Result<[f64; 3], String> {
    let mut transposed = Matrix3::zeros();
    for equation in 0..3 {
        for variable in 0..3 {
            let mut value = storage_block[equation][variable];
            if variable == 0 {
                value *= TRUE_IMPES_PRESSURE_SCALE_BAR;
            }
            transposed[(variable, equation)] = value;
        }
    }

    let rhs = Vector3::new(1.0, 0.0, 0.0);
    let solved = transposed
        .lu()
        .solve(&rhs)
        .ok_or_else(|| "singular true-IMPES storage block".to_string())?;
    if !solved.iter().all(|value| value.is_finite()) {
        return Err("non-finite true-IMPES storage solve".to_string());
    }
    let scale = solved
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    if !scale.is_finite() || scale <= f64::EPSILON {
        return Err("degenerate true-IMPES normalization".to_string());
    }
    Ok([solved[0] / scale, solved[1] / scale, solved[2] / scale])
}

fn extract_block(
    matrix: &CsMat<f64>,
    row_start: usize,
    row_count: usize,
    col_start: usize,
    col_count: usize,
) -> CsMat<f64> {
    let mut tri = TriMatI::<f64, usize>::new((row_count, col_count));
    for local_row in 0..row_count {
        let global_row = row_start + local_row;
        if let Some(view) = matrix.outer_view(global_row) {
            for (&global_col, &value) in view.indices().iter().zip(view.data().iter()) {
                if global_col >= col_start && global_col < col_start + col_count {
                    tri.add_triplet(local_row, global_col - col_start, value);
                }
            }
        }
    }
    tri.to_csr()
}

pub(crate) fn build_capture_data(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    layout: FimLinearBlockLayout,
    jacobian: &CsMat<f64>,
) -> Result<FimFlowLifecycleCapture, String> {
    if layout.cell_block_size != 3 || layout.cell_block_count != state.cells.len() {
        return Err("Y2d6 requires one three-primary block per reservoir cell".to_string());
    }
    if jacobian.rows() != jacobian.cols() || jacobian.rows() != state.n_unknowns() {
        return Err("Y2d6 full Jacobian dimensions do not match the FIM state".to_string());
    }

    let storage_blocks = accumulation_jacobian_blocks_for_state(sim, previous_state, state);
    let true_impes_weights = storage_blocks
        .iter()
        .enumerate()
        .map(|(cell_idx, block)| {
            true_impes_weight(block).map_err(|error| format!("cell {cell_idx}: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let reservoir_unknown_count = layout.cell_unknown_count();
    let well_unknown_count = jacobian.rows() - reservoir_unknown_count;
    Ok(FimFlowLifecycleCapture {
        source_tag: FLOW_SOURCE_TAG.to_string(),
        source_commit: FLOW_SOURCE_COMMIT.to_string(),
        dune_istl_version: DUNE_ISTL_VERSION.to_string(),
        pressure_scale_bar: TRUE_IMPES_PRESSURE_SCALE_BAR,
        storage_blocks,
        true_impes_weights,
        reservoir_unknown_count,
        j_rr: extract_block(
            jacobian,
            0,
            reservoir_unknown_count,
            0,
            reservoir_unknown_count,
        ),
        j_rw: extract_block(
            jacobian,
            0,
            reservoir_unknown_count,
            reservoir_unknown_count,
            well_unknown_count,
        ),
        j_wr: extract_block(
            jacobian,
            reservoir_unknown_count,
            well_unknown_count,
            0,
            reservoir_unknown_count,
        ),
        j_ww: extract_block(
            jacobian,
            reservoir_unknown_count,
            well_unknown_count,
            reservoir_unknown_count,
            well_unknown_count,
        ),
    })
}

#[cfg(test)]
fn matrix_value(matrix: &CsMat<f64>, row: usize, col: usize) -> f64 {
    matrix.get(row, col).copied().unwrap_or(0.0)
}

#[cfg(test)]
pub(crate) fn validate_capture_data(
    data: &FimFlowLifecycleCapture,
    layout: FimLinearBlockLayout,
    full_jacobian: &CsMat<f64>,
) -> Result<(), String> {
    if data.source_tag != FLOW_SOURCE_TAG
        || data.source_commit != FLOW_SOURCE_COMMIT
        || data.dune_istl_version != DUNE_ISTL_VERSION
        || (data.pressure_scale_bar - TRUE_IMPES_PRESSURE_SCALE_BAR).abs() > f64::EPSILON
    {
        return Err("Y2d6 source/config fingerprint mismatch".to_string());
    }
    if layout.cell_block_size != 3
        || data.storage_blocks.len() != layout.cell_block_count
        || data.true_impes_weights.len() != layout.cell_block_count
        || data.reservoir_unknown_count != layout.cell_unknown_count()
    {
        return Err("Y2d6 storage/layout cardinality mismatch".to_string());
    }

    for (cell_idx, (block, captured)) in data
        .storage_blocks
        .iter()
        .zip(&data.true_impes_weights)
        .enumerate()
    {
        let recomputed = true_impes_weight(block)?;
        let disagreement = recomputed
            .iter()
            .zip(captured)
            .map(|(left, right)| (left - right).abs())
            .fold(0.0_f64, f64::max);
        if disagreement > 1e-12 {
            return Err(format!(
                "Y2d6 true-IMPES weight mismatch at cell {cell_idx}: {disagreement:e}"
            ));
        }
    }

    let reservoir = data.reservoir_unknown_count;
    let well = full_jacobian.rows().saturating_sub(reservoir);
    let expected_dims = [
        (data.j_rr.rows(), data.j_rr.cols(), reservoir, reservoir),
        (data.j_rw.rows(), data.j_rw.cols(), reservoir, well),
        (data.j_wr.rows(), data.j_wr.cols(), well, reservoir),
        (data.j_ww.rows(), data.j_ww.cols(), well, well),
    ];
    if expected_dims
        .iter()
        .any(|&(rows, cols, expected_rows, expected_cols)| {
            rows != expected_rows || cols != expected_cols
        })
    {
        return Err("Y2d6 reservoir/well partition dimensions mismatch".to_string());
    }

    for row in 0..full_jacobian.rows() {
        for col in 0..full_jacobian.cols() {
            let captured = match (row < reservoir, col < reservoir) {
                (true, true) => matrix_value(&data.j_rr, row, col),
                (true, false) => matrix_value(&data.j_rw, row, col - reservoir),
                (false, true) => matrix_value(&data.j_wr, row - reservoir, col),
                (false, false) => matrix_value(&data.j_ww, row - reservoir, col - reservoir),
            };
            let original = matrix_value(full_jacobian, row, col);
            if captured.to_bits() != original.to_bits() {
                return Err(format!("Y2d6 partition mismatch at ({row},{col})"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
fn matrix_vector(matrix: &CsMat<f64>, vector: &DVector<f64>) -> DVector<f64> {
    let mut result = DVector::zeros(matrix.rows());
    for (row, entries) in matrix.outer_iterator().enumerate() {
        for (&col, &value) in entries.indices().iter().zip(entries.data().iter()) {
            result[row] += value * vector[col];
        }
    }
    result
}

#[cfg(test)]
fn dense_matrix(matrix: &CsMat<f64>) -> DMatrix<f64> {
    let mut dense = DMatrix::zeros(matrix.rows(), matrix.cols());
    for (row, entries) in matrix.outer_iterator().enumerate() {
        for (&col, &value) in entries.indices().iter().zip(entries.data().iter()) {
            dense[(row, col)] = value;
        }
    }
    dense
}

#[cfg(test)]
fn max_abs(vector: &DVector<f64>) -> f64 {
    vector.iter().map(|value| value.abs()).fold(0.0, f64::max)
}

#[cfg(test)]
fn relative_disagreement(left: &DVector<f64>, right: &DVector<f64>) -> f64 {
    (left - right).norm() / left.norm().max(right.norm()).max(1.0)
}

/// Test-only implementation of the source-pinned Flow component lifecycle on a capture-v3
/// system. It intentionally stops at the D6b identity surface: no Krylov solve or live routing.
#[cfg(test)]
pub(crate) struct FlowComponentOracle<'a> {
    data: &'a FimFlowLifecycleCapture,
    layout: FimLinearBlockLayout,
    j_ww_inverse: DMatrix<f64>,
    explicit_schur: DMatrix<f64>,
    coarse_reservoir: DMatrix<f64>,
    coarse_well: DMatrix<f64>,
    coarse_full: DMatrix<f64>,
    coarse_inverse: DMatrix<f64>,
    fine_factors: FimBlockIlu0Factors,
}

#[cfg(test)]
impl<'a> FlowComponentOracle<'a> {
    pub(crate) fn new(
        data: &'a FimFlowLifecycleCapture,
        layout: FimLinearBlockLayout,
    ) -> Result<Self, String> {
        let reservoir = data.reservoir_unknown_count;
        let cells = layout.cell_block_count;
        if layout.cell_block_size != 3 || reservoir != layout.cell_unknown_count() {
            return Err("Y2d6b requires the captured three-primary reservoir layout".to_string());
        }
        if cells > FLOW_AMG_COARSEN_TARGET {
            return Err(format!(
                "Y2d6b one-level oracle is valid only at or below Flow coarsenTarget={FLOW_AMG_COARSEN_TARGET}, got {cells}"
            ));
        }

        let j_ww_inverse = dense_matrix(&data.j_ww)
            .try_inverse()
            .ok_or_else(|| "Y2d6b well block is singular".to_string())?;
        let j_rr_dense = dense_matrix(&data.j_rr);
        let j_rw_dense = dense_matrix(&data.j_rw);
        let j_wr_dense = dense_matrix(&data.j_wr);
        let well_correction = &j_rw_dense * &j_ww_inverse * &j_wr_dense;
        let explicit_schur = &j_rr_dense - &well_correction;

        let restrict_prolong = |matrix: &DMatrix<f64>| {
            let mut coarse = DMatrix::zeros(cells, cells);
            for row_cell in 0..cells {
                for col_cell in 0..cells {
                    coarse[(row_cell, col_cell)] = (0..3)
                        .map(|equation| {
                            data.true_impes_weights[row_cell][equation]
                                * matrix[(3 * row_cell + equation, 3 * col_cell)]
                        })
                        .sum();
                }
            }
            coarse
        };
        let coarse_reservoir = restrict_prolong(&j_rr_dense);
        let coarse_well = -restrict_prolong(&well_correction);
        let coarse_full = restrict_prolong(&explicit_schur);
        let coarse_inverse = coarse_full
            .clone()
            .try_inverse()
            .ok_or_else(|| "Y2d6b one-level Flow coarse matrix is singular".to_string())?;
        let reservoir_layout = FimLinearBlockLayout {
            cell_block_count: cells,
            cell_block_size: 3,
            well_bhp_count: 0,
            perforation_tail_start: reservoir,
        };
        let fine_factors = factorize_block_ilu0(&data.j_rr, reservoir_layout)
            .ok_or_else(|| "Y2d6b paroverilu0 factorization failed".to_string())?;

        Ok(Self {
            data,
            layout,
            j_ww_inverse,
            explicit_schur,
            coarse_reservoir,
            coarse_well,
            coarse_full,
            coarse_inverse,
            fine_factors,
        })
    }

    pub(crate) fn reduced_rhs(&self, full_rhs: &DVector<f64>) -> DVector<f64> {
        let reservoir = self.data.reservoir_unknown_count;
        let reservoir_rhs = full_rhs.rows(0, reservoir).into_owned();
        let well_rhs = full_rhs
            .rows(reservoir, full_rhs.len() - reservoir)
            .into_owned();
        reservoir_rhs - matrix_vector(&self.data.j_rw, &(&self.j_ww_inverse * well_rhs))
    }

    pub(crate) fn outer_apply(&self, vector: &DVector<f64>) -> DVector<f64> {
        let reservoir_part = matrix_vector(&self.data.j_rr, vector);
        let well_response = &self.j_ww_inverse * matrix_vector(&self.data.j_wr, vector);
        reservoir_part - matrix_vector(&self.data.j_rw, &well_response)
    }

    fn restrict(&self, vector: &DVector<f64>) -> DVector<f64> {
        DVector::from_iterator(
            self.layout.cell_block_count,
            (0..self.layout.cell_block_count).map(|cell| {
                (0..3)
                    .map(|equation| {
                        self.data.true_impes_weights[cell][equation] * vector[3 * cell + equation]
                    })
                    .sum()
            }),
        )
    }

    fn prolong(&self, pressure: &DVector<f64>) -> DVector<f64> {
        let mut vector = DVector::zeros(self.data.reservoir_unknown_count);
        for cell in 0..self.layout.cell_block_count {
            vector[3 * cell] = pressure[cell];
        }
        vector
    }

    fn coarse_apply(&self, rhs: &DVector<f64>) -> DVector<f64> {
        &self.coarse_inverse * rhs
    }

    fn cpr_apply(&self, rhs: &DVector<f64>) -> DVector<f64> {
        // Flow CPRW configuration: zero pre-sweeps, one coarse correction, one post
        // paroverilu0 correction evaluated against the outer (well-eliminated) residual.
        let pressure = self.coarse_apply(&self.restrict(rhs));
        let mut update = self.prolong(&pressure);
        let post_rhs = rhs - self.outer_apply(&update);
        update += self.fine_factors.apply(&post_rhs);
        update
    }

    pub(crate) fn validate_identities(
        &self,
        full_rhs: &DVector<f64>,
    ) -> Result<FlowComponentIdentityMetrics, String> {
        // Identity 1 is also enforced during capture parsing; repeat it here so D6b cannot be
        // called on a programmatically-constructed companion that bypassed the parser.
        for (cell, (storage, captured)) in self
            .data
            .storage_blocks
            .iter()
            .zip(&self.data.true_impes_weights)
            .enumerate()
        {
            let recomputed = true_impes_weight(storage)?;
            let disagreement = recomputed
                .iter()
                .zip(captured)
                .map(|(left, right)| (left - right).abs())
                .fold(0.0, f64::max);
            if disagreement > 1e-12 {
                return Err(format!("identity 1 true-IMPES mismatch at cell {cell}"));
            }
        }

        let n = self.data.reservoir_unknown_count;
        let x = DVector::from_iterator(n, (0..n).map(|index| ((index + 1) as f64 * 0.017).sin()));
        let y = DVector::from_iterator(n, (0..n).map(|index| ((index + 3) as f64 * 0.013).cos()));

        // Identity 2: matrix-free standard-well outer operator equals the explicit Schur form.
        let outer = self.outer_apply(&x);
        let explicit = &self.explicit_schur * &x;
        let outer_disagreement = relative_disagreement(&outer, &explicit);
        if outer_disagreement > 5e-12 {
            return Err(format!(
                "identity 2 outer/Schur mismatch: {outer_disagreement:e}"
            ));
        }

        // Identity 3: the CPRW coarse operator is reservoir contribution plus exactly one well
        // correction. A nonzero well term makes accidental omission/double-addition observable.
        let coarse_sum = &self.coarse_reservoir + &self.coarse_well;
        let coarse_disagreement =
            (&coarse_sum - &self.coarse_full).norm() / self.coarse_full.norm().max(1.0);
        let coarse_well_norm = self.coarse_well.norm();
        if coarse_disagreement > 5e-12 || coarse_well_norm <= f64::EPSILON {
            return Err(format!(
                "identity 3 coarse well composition mismatch={coarse_disagreement:e} well_norm={coarse_well_norm:e}"
            ));
        }
        let doubled = &coarse_sum + &self.coarse_well;
        if (&doubled - &self.coarse_full).norm() <= f64::EPSILON {
            return Err("identity 3 cannot distinguish a doubled well contribution".to_string());
        }

        // Identity 4: one setup of the block ILU on J_rr is a fixed linear map.
        let fine_x_a = self.fine_factors.apply(&x);
        let fine_x_b = self.fine_factors.apply(&x);
        if fine_x_a.as_slice() != fine_x_b.as_slice() {
            return Err("identity 4 paroverilu0 repeatability failed".to_string());
        }
        let fine_linearity = relative_disagreement(
            &self.fine_factors.apply(&(&x + &y)),
            &(&fine_x_a + self.fine_factors.apply(&y)),
        );
        if fine_linearity > 5e-12 {
            return Err(format!(
                "identity 4 paroverilu0 linearity failed: {fine_linearity:e}"
            ));
        }

        // Identity 5: these artifacts are below coarsenTarget, so DUNE AMG has one level and
        // its sequential direct coarse solver is applied once. The stored factor map must repeat
        // bit-for-bit and be linear; no tolerance-terminated inner iteration exists here.
        let coarse_x = self.restrict(&x);
        let coarse_y = self.restrict(&y);
        let coarse_apply_a = self.coarse_apply(&coarse_x);
        let coarse_apply_b = self.coarse_apply(&coarse_x);
        if coarse_apply_a.as_slice() != coarse_apply_b.as_slice() {
            return Err("identity 5 one-level AMG repeatability failed".to_string());
        }
        let coarse_linearity = relative_disagreement(
            &self.coarse_apply(&(&coarse_x + &coarse_y)),
            &(&coarse_apply_a + self.coarse_apply(&coarse_y)),
        );
        if coarse_linearity > 5e-12 {
            return Err(format!(
                "identity 5 one-level AMG linearity failed: {coarse_linearity:e}"
            ));
        }

        // Identity 6: complete zero-pre/coarse/one-post CPR order is itself fixed and linear.
        let cpr_x_a = self.cpr_apply(&x);
        let cpr_x_b = self.cpr_apply(&x);
        if cpr_x_a.as_slice() != cpr_x_b.as_slice() {
            return Err("identity 6 CPR repeatability failed".to_string());
        }
        let coarse_first = self.prolong(&self.coarse_apply(&self.restrict(&x)));
        let independently_ordered = &coarse_first
            + self
                .fine_factors
                .apply(&(&x - self.outer_apply(&coarse_first)));
        let order_disagreement = relative_disagreement(&cpr_x_a, &independently_ordered);
        if order_disagreement > 5e-12 {
            return Err(format!(
                "identity 6 coarse-then-post order mismatch: {order_disagreement:e}"
            ));
        }
        let fine_first = self.fine_factors.apply(&x);
        let reversed_order = &fine_first
            + self
                .prolong(&self.coarse_apply(&self.restrict(&(&x - self.outer_apply(&fine_first)))));
        if relative_disagreement(&cpr_x_a, &reversed_order) <= f64::EPSILON {
            return Err("identity 6 cannot distinguish reversed CPR order".to_string());
        }
        let cpr_linearity = relative_disagreement(
            &self.cpr_apply(&(&x + &y)),
            &(&cpr_x_a + self.cpr_apply(&y)),
        );
        if cpr_linearity > 2e-11 {
            return Err(format!(
                "identity 6 CPR linearity failed: {cpr_linearity:e}"
            ));
        }

        // Identity 7: compare the outer residual norm with an independent explicit-Schur path.
        let reduced_rhs = self.reduced_rhs(full_rhs);
        let correction = self.cpr_apply(&reduced_rhs);
        let matrix_free_residual = &reduced_rhs - self.outer_apply(&correction);
        let explicit_residual = &reduced_rhs - &self.explicit_schur * &correction;
        let residual_norm_disagreement = (matrix_free_residual.norm() - explicit_residual.norm())
            .abs()
            / explicit_residual.norm().max(1.0);
        if residual_norm_disagreement > 5e-12 {
            return Err(format!(
                "identity 7 independent outer residual mismatch: {residual_norm_disagreement:e}"
            ));
        }

        Ok(FlowComponentIdentityMetrics {
            reservoir_rows: n,
            pressure_rows: self.layout.cell_block_count,
            outer_disagreement,
            coarse_disagreement,
            coarse_well_norm,
            fine_linearity,
            coarse_linearity,
            cpr_linearity,
            residual_norm: matrix_free_residual.norm(),
            residual_norm_disagreement,
            correction_max_abs: max_abs(&correction),
        })
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct FlowComponentIdentityMetrics {
    pub(crate) reservoir_rows: usize,
    pub(crate) pressure_rows: usize,
    pub(crate) outer_disagreement: f64,
    pub(crate) coarse_disagreement: f64,
    pub(crate) coarse_well_norm: f64,
    pub(crate) fine_linearity: f64,
    pub(crate) coarse_linearity: f64,
    pub(crate) cpr_linearity: f64,
    pub(crate) residual_norm: f64,
    pub(crate) residual_norm_disagreement: f64,
    pub(crate) correction_max_abs: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sparse(rows: usize, cols: usize, entries: &[(usize, usize, f64)]) -> CsMat<f64> {
        let mut tri = TriMatI::<f64, usize>::new((rows, cols));
        for &(row, col, value) in entries {
            tri.add_triplet(row, col, value);
        }
        tri.to_csr()
    }

    #[test]
    fn component_oracle_proves_all_seven_identities_on_coupled_well_system() {
        let storage = [[1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 3.0]];
        let data = FimFlowLifecycleCapture {
            source_tag: FLOW_SOURCE_TAG.to_string(),
            source_commit: FLOW_SOURCE_COMMIT.to_string(),
            dune_istl_version: DUNE_ISTL_VERSION.to_string(),
            pressure_scale_bar: TRUE_IMPES_PRESSURE_SCALE_BAR,
            storage_blocks: vec![storage, storage],
            true_impes_weights: vec![
                true_impes_weight(&storage).unwrap(),
                true_impes_weight(&storage).unwrap(),
            ],
            reservoir_unknown_count: 6,
            j_rr: sparse(
                6,
                6,
                &[
                    (0, 0, 8.0),
                    (0, 1, 0.2),
                    (0, 3, -1.0),
                    (1, 0, 0.1),
                    (1, 1, 5.0),
                    (2, 2, 4.0),
                    (3, 0, -0.5),
                    (3, 3, 7.0),
                    (3, 4, 0.1),
                    (4, 3, 0.2),
                    (4, 4, 6.0),
                    (5, 5, 3.0),
                ],
            ),
            j_rw: sparse(6, 2, &[(0, 0, 0.5), (3, 1, -0.4), (4, 1, 0.2)]),
            j_wr: sparse(2, 6, &[(0, 0, 0.3), (0, 1, 0.1), (1, 3, -0.2)]),
            j_ww: sparse(2, 2, &[(0, 0, 2.0), (0, 1, 0.1), (1, 0, -0.2), (1, 1, 3.0)]),
        };
        let layout = FimLinearBlockLayout {
            cell_block_count: 2,
            cell_block_size: 3,
            well_bhp_count: 2,
            perforation_tail_start: 8,
        };
        let rhs = DVector::from_vec(vec![1.0, -2.0, 0.5, 3.0, -1.0, 0.25, 2.0, -0.5]);

        let metrics = FlowComponentOracle::new(&data, layout)
            .unwrap()
            .validate_identities(&rhs)
            .unwrap();

        assert!(metrics.coarse_well_norm > 0.0);
        assert!(metrics.outer_disagreement < 5e-12);
        assert!(metrics.residual_norm_disagreement < 5e-12);
    }
}
