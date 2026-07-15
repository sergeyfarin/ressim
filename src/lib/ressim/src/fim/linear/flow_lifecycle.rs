//! Native-only source-pinned helpers for the Y2d6 Flow 2026.04 linear-lifecycle oracle.
//!
//! This module does not participate in production dispatch. Y2d6a uses it only when the
//! dedicated capture environment variable is set; later Y2d6 gates reuse the same formulas in
//! test-only component identities.

use nalgebra::{Matrix3, Vector3};
use sprs::{CsMat, TriMatI};

use super::FimLinearBlockLayout;
use super::capture::FimFlowLifecycleCapture;
use crate::ReservoirSimulator;
use crate::fim::assembly_ad::accumulation_jacobian_blocks_for_state;
use crate::fim::state::FimState;

pub(crate) const FLOW_SOURCE_TAG: &str = "release/2026.04/final";
pub(crate) const FLOW_SOURCE_COMMIT: &str = "b82f21dba405286c4c4446614dd3bf9cdebf7a2c";
pub(crate) const DUNE_ISTL_VERSION: &str = "2.11.0";

/// OPM multiplies derivatives with respect to pressure in pascals by `50e5`. ResSim's pressure
/// primary is bar, so the equivalent nondimensionalizing scale is 50 bar.
pub(crate) const TRUE_IMPES_PRESSURE_SCALE_BAR: f64 = 50.0;

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

fn matrix_value(matrix: &CsMat<f64>, row: usize, col: usize) -> f64 {
    matrix.get(row, col).copied().unwrap_or(0.0)
}

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
