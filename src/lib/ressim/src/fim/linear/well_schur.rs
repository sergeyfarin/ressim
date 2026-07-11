//! Well/perforation Schur elimination (Phase 11, `FIM-LINEAR-010`).
//!
//! OPM eliminates the well block (BHP + connection rates) from the reservoir linear system
//! every Newton iteration via a Schur complement (`StandardWellEquations`), recovering the well
//! update from the local solve after the reservoir-only system converges. ResSim previously
//! solved well/perforation unknowns as ordinary global Newton unknowns mixed in with the
//! reservoir cells — direct measurement (`docs/FIM_CONVERGENCE_WORKLOG.md`, "Step 10.4
//! (reopened)") found this let a well's connection-rate residual genuinely oscillate against the
//! reservoir cells across Newton iterations, a pattern neither of the existing Newton
//! stabilization mechanisms was built to catch (one structurally excludes the well/perforation
//! family; the other's "weak progress vs. immediately-prior-iteration" test is fooled by a clean
//! two-step bounce).
//!
//! This module performs the elimination purely as sparse/dense linear algebra on the assembled
//! `(jacobian, rhs)` pair, using only the existing `FimLinearBlockLayout` row-partition contract
//! — no well-specific physics knowledge is needed. Given the block system
//!
//! ```text
//! [ J_RR  J_RW ] [dx_R]   [ r_R ]
//! [ J_WR  J_WW ] [dx_W] = [ r_W ]
//! ```
//!
//! where `R` is the reservoir-cell rows and `W` is the well-BHP + perforation-rate tail
//! (contiguous, `well_bhp_start..rows()`, confirmed block-diagonal per well by direct assembly
//! read — no well ever couples to another well's rows/columns), the reduced reservoir-only
//! system is
//!
//! ```text
//! (J_RR - J_RW * J_WW^-1 * J_WR) dx_R = r_R - J_RW * J_WW^-1 * r_W
//! ```
//!
//! solved through the ordinary dispatch (`solve_linearized_system`, recursed on a tail-free
//! layout so it falls straight through to the normal iterative/direct path), then the tail is
//! recovered exactly: `dx_W = J_WW^-1 * (r_W - J_WR * dx_R)`.

use nalgebra::{DMatrix, DVector};
use sprs::{CsMat, TriMatI};

use super::gmres_block_jacobi::{cs_mat_mul_vec, invert_tail_block, matrix_value};
use super::{
    FimLinearBlockLayout, FimLinearSolveOptions, FimLinearSolveReport, solve_linearized_system,
};
use crate::fim::scaling::EquationScaling;

/// Result of Schur-eliminating the well-BHP/perforation tail: the reduced (tail-free) system to
/// solve, plus everything needed to recover the tail unknowns afterward.
pub(super) struct WellEliminationResult {
    pub(super) reduced_jacobian: CsMat<f64>,
    pub(super) reduced_rhs: DVector<f64>,
    pub(super) reduced_layout: FimLinearBlockLayout,
    pub(super) reduced_equation_scaling: Option<EquationScaling>,
    tail_inverse: DMatrix<f64>,
    j_wr: Vec<Vec<(usize, f64)>>,
    well_bhp_start: usize,
    tail_count: usize,
}

/// Reduces `(jacobian, rhs, layout)` by Schur-eliminating the well-BHP/perforation tail.
/// Returns `None` when there is no tail to eliminate (nothing to reduce). Shared by the
/// production solve path (`solve_with_well_elimination`) and the offline CPR-reuse lab
/// (`solver_lab.rs`'s Bundle P `FIM-BUNDLE-P` P0.2 study), which needs to build/reuse a
/// preconditioner against the exact same reduced system the live path actually solves against
/// (`eliminate_wells` defaults `true` in production).
pub(super) fn eliminate_wells(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    layout: FimLinearBlockLayout,
    equation_scaling: Option<&EquationScaling>,
) -> Option<WellEliminationResult> {
    let well_bhp_start = layout.well_bhp_start();
    let tail_count = jacobian.rows().saturating_sub(well_bhp_start);

    if tail_count == 0 {
        return None;
    }

    // J_WW: dense tail-diagonal block (well BHP + perforation rows/cols), same extraction as the
    // existing CPR preconditioner's `tail_inverse` computation, just spanning the whole tail
    // (well BHP + perforations together) rather than perforations alone.
    let mut tail_block = DMatrix::zeros(tail_count, tail_count);
    for tail_row in 0..tail_count {
        for tail_col in 0..tail_count {
            tail_block[(tail_row, tail_col)] = matrix_value(
                jacobian,
                well_bhp_start + tail_row,
                well_bhp_start + tail_col,
            );
        }
    }
    let tail_inverse = invert_tail_block(&tail_block);

    // J_WR: tail rows x reservoir columns, sparse per tail row (a well only couples to its own
    // perforations' connected cells, so this is a handful of nonzeros per row regardless of grid
    // size).
    let mut j_wr: Vec<Vec<(usize, f64)>> = vec![Vec::new(); tail_count];
    for (tail_idx, entries) in j_wr.iter_mut().enumerate() {
        let row_idx = well_bhp_start + tail_idx;
        if let Some(view) = jacobian.outer_view(row_idx) {
            for (col_idx, &value) in view.indices().iter().zip(view.data().iter()) {
                if *col_idx < well_bhp_start && value.abs() > f64::EPSILON {
                    entries.push((*col_idx, value));
                }
            }
        }
    }

    // J_RW: reservoir rows x tail columns, sparse. Only rows touching a perforated cell have any
    // entries here, so this stays proportional to well/perforation count, not grid size, even
    // though the outer loop scans every reservoir row once (a single O(nnz) pass).
    let mut j_rw: Vec<(usize, Vec<(usize, f64)>)> = Vec::new();
    for row_idx in 0..well_bhp_start {
        if let Some(view) = jacobian.outer_view(row_idx) {
            let mut entries = Vec::new();
            for (col_idx, &value) in view.indices().iter().zip(view.data().iter()) {
                if *col_idx >= well_bhp_start && value.abs() > f64::EPSILON {
                    entries.push((*col_idx - well_bhp_start, value));
                }
            }
            if !entries.is_empty() {
                j_rw.push((row_idx, entries));
            }
        }
    }

    // r_R_eff = r_R - J_RW * (J_WW^-1 * r_W)
    let rhs_tail =
        DVector::from_iterator(tail_count, (0..tail_count).map(|i| rhs[well_bhp_start + i]));
    let tail_inverse_rhs = &tail_inverse * &rhs_tail;

    let mut reduced_rhs =
        DVector::from_iterator(well_bhp_start, (0..well_bhp_start).map(|i| rhs[i]));
    for (row_idx, entries) in &j_rw {
        let mut correction = 0.0;
        for &(tail_col, value) in entries {
            correction += value * tail_inverse_rhs[tail_col];
        }
        reduced_rhs[*row_idx] -= correction;
    }

    // J_RR_eff = J_RR - J_RW * J_WW^-1 * J_WR, built as "original reservoir-reservoir triplets"
    // plus "correction triplets" — `TriMatI::to_csr()` sums duplicate (row, col) entries (the
    // same behavior the assembly layer already relies on for accumulated physics contributions),
    // so the correction can simply be appended rather than manually merged.
    let mut tri = TriMatI::<f64, usize>::new((well_bhp_start, well_bhp_start));
    for row_idx in 0..well_bhp_start {
        if let Some(view) = jacobian.outer_view(row_idx) {
            for (col_idx, &value) in view.indices().iter().zip(view.data().iter()) {
                if *col_idx < well_bhp_start {
                    tri.add_triplet(row_idx, *col_idx, value);
                }
            }
        }
    }
    for (row_idx, row_entries) in &j_rw {
        // weight = (row of J_RW at row_idx) * J_WW^-1, a 1 x tail_count row vector.
        let mut weight = vec![0.0; tail_count];
        for &(t1, value) in row_entries {
            for (t2, w) in weight.iter_mut().enumerate() {
                *w += value * tail_inverse[(t1, t2)];
            }
        }
        for (t2, &w) in weight.iter().enumerate() {
            if w.abs() <= f64::EPSILON {
                continue;
            }
            for &(col, jwr_value) in &j_wr[t2] {
                tri.add_triplet(*row_idx, col, -w * jwr_value);
            }
        }
    }
    let reduced_jacobian = tri.to_csr();

    let reduced_layout = FimLinearBlockLayout {
        cell_block_count: layout.cell_block_count,
        cell_block_size: layout.cell_block_size,
        well_bhp_count: 0,
        perforation_tail_start: well_bhp_start,
    };

    // The reduced system has no well/perforation rows left, so any `EquationScaling` passed
    // through must drop its `well_constraint`/`perforation_flow` vectors — otherwise
    // `family_peaks` indexes past the end of the (now shorter) residual vector. The cell-level
    // scaling (`water`/`oil_component`/`gas_component`) is unchanged, since the reduced system's
    // cell rows are identical to the original's.
    let reduced_equation_scaling = equation_scaling.map(|scaling| EquationScaling {
        water: scaling.water.clone(),
        oil_component: scaling.oil_component.clone(),
        gas_component: scaling.gas_component.clone(),
        well_constraint: Vec::new(),
        perforation_flow: Vec::new(),
    });

    Some(WellEliminationResult {
        reduced_jacobian,
        reduced_rhs,
        reduced_layout,
        reduced_equation_scaling,
        tail_inverse,
        j_wr,
        well_bhp_start,
        tail_count,
    })
}

pub(super) fn solve_with_well_elimination(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
    options: &FimLinearSolveOptions,
    layout: FimLinearBlockLayout,
    equation_scaling: Option<&EquationScaling>,
) -> FimLinearSolveReport {
    let Some(elimination) = eliminate_wells(jacobian, rhs, layout, equation_scaling) else {
        // Nothing to eliminate. The dispatcher in `solve_linearized_system` already checks this
        // before calling here, but stay correct if called directly (e.g. from a future test).
        return solve_linearized_system(jacobian, rhs, options, Some(layout), equation_scaling);
    };

    // Solve the reduced (tail-free) system through the normal dispatcher. This recursion is safe
    // and terminates in one step: `reduced_layout` has no well/perforation tail, so the
    // `eliminate_wells` check in `solve_linearized_system` is false for this call regardless of
    // `options.eliminate_wells`, and it falls straight through to the ordinary iterative/direct
    // path.
    let reduced_report = solve_linearized_system(
        &elimination.reduced_jacobian,
        &elimination.reduced_rhs,
        options,
        Some(elimination.reduced_layout),
        elimination.reduced_equation_scaling.as_ref(),
    );

    let well_bhp_start = elimination.well_bhp_start;
    let tail_count = elimination.tail_count;

    // Recover the tail: dx_W = J_WW^-1 * (r_W - J_WR * dx_R).
    let mut residual_tail =
        DVector::from_iterator(tail_count, (0..tail_count).map(|i| rhs[well_bhp_start + i]));
    for (tail_idx, row) in residual_tail.iter_mut().enumerate() {
        let mut sum = 0.0;
        for &(col, value) in &elimination.j_wr[tail_idx] {
            sum += value * reduced_report.solution[col];
        }
        *row -= sum;
    }
    let dx_tail = &elimination.tail_inverse * &residual_tail;

    let mut solution = DVector::zeros(jacobian.rows());
    solution
        .rows_mut(0, well_bhp_start)
        .copy_from(&reduced_report.solution);
    solution
        .rows_mut(well_bhp_start, tail_count)
        .copy_from(&dx_tail);

    // End-to-end safety net: recompute the residual against the *original, full* system. Any
    // arithmetic bug in the elimination/recovery math surfaces here as a residual mismatch
    // instead of a silently wrong answer that happens to look converged on the reduced system.
    let full_residual = rhs - &cs_mat_mul_vec(jacobian, &solution);
    let full_residual_norm = full_residual.norm();
    let tolerance =
        options.absolute_tolerance + options.relative_tolerance * rhs.norm().max(f64::EPSILON);

    FimLinearSolveReport {
        solution,
        converged: reduced_report.converged && full_residual_norm <= tolerance,
        iterations: reduced_report.iterations,
        final_residual_norm: full_residual_norm,
        failure_diagnostics: reduced_report.failure_diagnostics,
        used_fallback: reduced_report.used_fallback,
        backend_used: reduced_report.backend_used,
        cpr_diagnostics: reduced_report.cpr_diagnostics,
        total_time_ms: reduced_report.total_time_ms,
        preconditioner_build_time_ms: reduced_report.preconditioner_build_time_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{FimLinearSolverKind, sparse_lu_debug};
    use super::*;

    /// Synthetic 2-cell + 1-well + 2-perforation system mirroring the real row layout: cells
    /// `[0..6)` (2 cells x 3 vars), well BHP at `6`, perforations at `7, 8`. Coupling mirrors the
    /// real assembly structure (confirmed by direct code read, `assembly_ad.rs:183-289`): each
    /// perforation couples to its own rate column, its connected cell's 3 columns, and the well's
    /// BHP column; each perforated cell's rows get a term against that perforation's rate column;
    /// the well constraint row couples to its own BHP and both perforations' rates.
    fn sample_system() -> (CsMat<f64>, DVector<f64>, FimLinearBlockLayout) {
        let n = 9;
        let mut tri = TriMatI::<f64, usize>::new((n, n));

        // Cell 0 (rows/cols 0,1,2) and cell 1 (rows/cols 3,4,5): diagonal-dominant with a little
        // cross-cell coupling so the reservoir block alone is well-conditioned.
        for cell in 0..2 {
            let base = cell * 3;
            for local in 0..3 {
                tri.add_triplet(base + local, base + local, 5.0 + local as f64);
            }
        }
        tri.add_triplet(0, 3, -0.3);
        tri.add_triplet(3, 0, -0.3);

        // Perforation 0 (row/col 7) connects to cell 0; perforation 1 (row/col 8) connects to
        // cell 1. Both belong to well 0 (BHP row/col 6).
        // Rate-consistency rows (own q, own connected cell, own well BHP):
        tri.add_triplet(7, 7, 1.0);
        tri.add_triplet(7, 0, 0.05);
        tri.add_triplet(7, 1, 0.02);
        tri.add_triplet(7, 6, -1.0);

        tri.add_triplet(8, 8, 1.0);
        tri.add_triplet(8, 3, 0.04);
        tri.add_triplet(8, 4, 0.01);
        tri.add_triplet(8, 6, -1.0);

        // Perforated cells' own mass-balance rows pick up a term against their own perforation's
        // rate column (mirrors `add_if_nonzero(tri, eq_row, q_col, row[3] * dt_days)`).
        tri.add_triplet(0, 7, 0.2);
        tri.add_triplet(1, 7, 0.1);
        tri.add_triplet(3, 8, 0.15);
        tri.add_triplet(4, 8, 0.08);

        // Well constraint row (BHP-controlled): bhp == target, no coupling to perforations.
        tri.add_triplet(6, 6, 1.0);

        let jacobian = tri.to_csr();
        let rhs = DVector::from_vec(vec![1.0, 0.5, 0.2, -0.8, 0.3, 0.1, 2.0, 0.6, -0.4]);
        let layout = FimLinearBlockLayout {
            cell_block_count: 2,
            cell_block_size: 3,
            well_bhp_count: 1,
            perforation_tail_start: 7,
        };
        (jacobian, rhs, layout)
    }

    #[test]
    fn well_elimination_matches_direct_full_system_solve() {
        let (jacobian, rhs, layout) = sample_system();
        let options = FimLinearSolveOptions {
            kind: FimLinearSolverKind::FgmresCpr,
            ..FimLinearSolveOptions::default()
        };

        let reference = sparse_lu_debug::solve(&jacobian, &rhs, &options, false);
        assert!(reference.converged, "reference direct solve must converge");

        let eliminated = solve_with_well_elimination(&jacobian, &rhs, &options, layout, None);
        assert!(
            eliminated.converged,
            "eliminated solve did not converge: {eliminated:?}"
        );

        for i in 0..jacobian.rows() {
            assert!(
                (eliminated.solution[i] - reference.solution[i]).abs() < 1e-9,
                "row {i}: eliminated={} reference={}",
                eliminated.solution[i],
                reference.solution[i]
            );
        }
    }

    #[test]
    fn well_elimination_no_tail_falls_through_unchanged() {
        let mut tri = TriMatI::<f64, usize>::new((3, 3));
        for idx in 0..3 {
            tri.add_triplet(idx, idx, 2.0 + idx as f64);
        }
        let jacobian = tri.to_csr();
        let rhs = DVector::from_element(3, 1.0);
        let layout = FimLinearBlockLayout {
            cell_block_count: 1,
            cell_block_size: 3,
            well_bhp_count: 0,
            perforation_tail_start: 3,
        };
        let options = FimLinearSolveOptions {
            kind: FimLinearSolverKind::FgmresCpr,
            ..FimLinearSolveOptions::default()
        };

        let report = solve_with_well_elimination(&jacobian, &rhs, &options, layout, None);
        assert!(report.converged);
        for i in 0..3 {
            assert!((report.solution[i] - 1.0 / (2.0 + i as f64)).abs() < 1e-9);
        }
    }
}
