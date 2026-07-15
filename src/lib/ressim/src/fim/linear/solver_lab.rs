//! Offline solver lab (Phase 9 step 9.2): runs full solves with each linear backend on
//! captured real failing systems (`capture.rs`), on identical inputs, out of the Newton
//! loop. This is the component-isolation harness for the linear solver / CPR — a new
//! preconditioner hypothesis is evaluated here in seconds against the whole captured
//! corpus before any live-solver change is considered.
//!
//! Usage (native only, manual):
//! 1. Capture a corpus:
//!    `FIM_CAPTURE_DIR=<dir> cargo test --release --lib -- --ignored repro_water_pressure_12x12x3 --nocapture`
//! 2. Run the lab on it:
//!    `FIM_CAPTURE_DIR=<dir> cargo test --release --lib -- --ignored solver_lab_compare_backends --nocapture`
//!
//! Validity gates (stop conditions from the Phase 9 plan) are asserted, not just printed:
//! - sparse-LU must converge on every captured system (else the problem is upstream in
//!   assembly at that state, not in the preconditioner);
//! - the current FgmresCpr path must reproduce its live failure on the majority of the
//!   corpus (else the capture is missing state and comparisons are not trustworthy).

use nalgebra::{DMatrix, DVector};
use sprs::CsMat;

use super::capture::{
    capture_dir_from_env, load_captures, y2b2_capture_dir_from_env, FimCapturedSystem,
};
use super::gmres_block_jacobi::{
    self, coarse_factorization_lab_compare, solve_with_restriction_kind,
    solve_with_smoother_and_restriction, CprFineSmootherKind, CprPressureRestrictionKind,
};
use super::sparse_lu_debug;
use super::well_schur;
use super::{
    solve_linearized_system, FimLinearBlockLayout, FimLinearSolveOptions, FimLinearSolveReport,
    FimLinearSolverKind,
};
use crate::fim::scaling::EquationScaling;

fn residual_vector(
    jacobian: &CsMat<f64>,
    solution: &DVector<f64>,
    rhs: &DVector<f64>,
) -> DVector<f64> {
    let mut residual = rhs.clone();
    for (row, vec) in jacobian.outer_iterator().enumerate() {
        let mut sum = 0.0;
        for (&col, &value) in vec.indices().iter().zip(vec.data().iter()) {
            sum += value * solution[col];
        }
        residual[row] -= sum;
    }
    residual
}

fn residual_norm(jacobian: &CsMat<f64>, solution: &DVector<f64>, rhs: &DVector<f64>) -> f64 {
    residual_vector(jacobian, solution, rhs).norm()
}

fn y2b2_partition_norms(layout: FimLinearBlockLayout, residual: &DVector<f64>) -> (f64, f64) {
    let reservoir_rows = layout.cell_unknown_count();
    let reservoir = residual.rows(0, reservoir_rows).norm();
    let well = residual
        .rows(reservoir_rows, residual.len() - reservoir_rows)
        .norm();
    (reservoir, well)
}

fn dense_matrix(jacobian: &CsMat<f64>) -> DMatrix<f64> {
    let mut dense = DMatrix::zeros(jacobian.rows(), jacobian.cols());
    for (row, vector) in jacobian.outer_iterator().enumerate() {
        for (&column, &value) in vector.indices().iter().zip(vector.data().iter()) {
            dense[(row, column)] = value;
        }
    }
    dense
}

/// A rank-revealing direct replay for a singular captured Newton system. This exists only in the
/// test-only lab: its Moore-Penrose correction is an oracle for `J dx = rhs`, not a production
/// FIM solver or an OPM implementation choice.
struct DenseSvdReplay {
    solution: DVector<f64>,
    rank: usize,
    singular_value_cutoff: f64,
}

fn dense_svd_replay(
    jacobian: &CsMat<f64>,
    rhs: &DVector<f64>,
) -> Result<DenseSvdReplay, &'static str> {
    let svd = dense_matrix(jacobian).svd(true, true);
    let max_singular_value = svd.singular_values.iter().copied().fold(0.0_f64, f64::max);
    let singular_value_cutoff = (max_singular_value * 1e-10).max(f64::EPSILON);
    let rank = svd.rank(singular_value_cutoff);
    let solution = svd.solve(rhs, singular_value_cutoff)?;
    Ok(DenseSvdReplay {
        solution,
        rank,
        singular_value_cutoff,
    })
}

fn y2b2_empty_column_families(layout: FimLinearBlockLayout, empty_columns: &[usize]) -> [usize; 5] {
    let mut families = [0usize; 5];
    for &column in empty_columns {
        let family = if column < layout.cell_unknown_count() {
            column % layout.cell_block_size
        } else if column < layout.well_bhp_end() {
            3
        } else {
            4
        };
        families[family] += 1;
    }
    families
}

/// Largest per-family overshoot beyond that family's own relative-reduction target, i.e. how
/// many multiples of `absolute_tolerance + relative_tolerance * initial_peak` the worst family
/// is currently at. `> 1.0` means at least one family (often the well/perforation rows) is left
/// under-resolved even though the whole-system norm may already look converged. Returns `None`
/// if the system has no captured `EquationScaling`.
fn worst_family_overshoot(
    scaling: &EquationScaling,
    rhs: &DVector<f64>,
    residual: &DVector<f64>,
    absolute_tolerance: f64,
    relative_tolerance: f64,
) -> (&'static str, f64) {
    let initial = scaling.family_peaks(rhs);
    let current = scaling.family_peaks(residual);
    let ratio = |current: f64, initial: f64| {
        current
            / (absolute_tolerance + relative_tolerance * initial.max(f64::EPSILON))
                .max(f64::EPSILON)
    };
    let candidates = [
        ("water", ratio(current.water, initial.water)),
        (
            "oil_component",
            ratio(current.oil_component, initial.oil_component),
        ),
        (
            "gas_component",
            ratio(current.gas_component, initial.gas_component),
        ),
        (
            "well_constraint",
            ratio(current.well_constraint, initial.well_constraint),
        ),
        (
            "perforation_flow",
            ratio(current.perforation_flow, initial.perforation_flow),
        ),
    ];
    candidates
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .expect("candidates is non-empty")
}

struct BackendOutcome {
    label: &'static str,
    backend_used: &'static str,
    converged: bool,
    iterations: usize,
    true_residual_norm: f64,
    failure_reason: Option<&'static str>,
}

fn run_backend(
    label: &'static str,
    kind: FimLinearSolverKind,
    system: &FimCapturedSystem,
) -> BackendOutcome {
    let options = FimLinearSolveOptions {
        kind,
        ..FimLinearSolveOptions::default()
    };
    let report: FimLinearSolveReport = solve_linearized_system(
        &system.jacobian,
        &system.rhs,
        &options,
        system.layout,
        system.equation_scaling.as_ref(),
    );
    BackendOutcome {
        label,
        backend_used: report.backend_used.label(),
        converged: report.converged,
        iterations: report.iterations,
        true_residual_norm: residual_norm(&system.jacobian, &report.solution, &system.rhs),
        failure_reason: report
            .failure_diagnostics
            .as_ref()
            .map(|diagnostics| diagnostics.reason.label()),
    }
}

/// Manual lab entry point; requires FIM_CAPTURE_DIR pointing at a captured corpus.
#[test]
#[ignore]
fn solver_lab_compare_backends() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a directory produced by a capture driver run");
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    println!(
        "solver lab: {} captured systems from {}",
        systems.len(),
        dir.display()
    );

    let mut lu_failures = 0usize;
    let mut cpr_reproduced_failure = 0usize;
    let mut cpr_converged = 0usize;
    let mut gmres_ilu0_converged = 0usize;

    for (index, system) in systems.iter().enumerate() {
        let rhs_norm = system.rhs.norm();
        println!(
            "--- capture {index:05}: rows={} nnz={} rhs_norm={:.3e} newton_iter={} live_reason={} dominant={}@{}",
            system.jacobian.rows(),
            system.jacobian.nnz(),
            rhs_norm,
            system.metadata.newton_iteration,
            system.metadata.failure_reason,
            system.metadata.dominant_family,
            system.metadata.dominant_item_index,
        );

        let outcomes = [
            run_backend("sparse-lu", FimLinearSolverKind::SparseLuDebug, system),
            run_backend("gmres-ilu0", FimLinearSolverKind::GmresIlu0, system),
            run_backend("fgmres-cpr", FimLinearSolverKind::FgmresCpr, system),
        ];

        for outcome in &outcomes {
            println!(
                "    {:<11} used={:<10} converged={:<5} iters={:<4} true_res={:.3e} rel={:.3e}{}",
                outcome.label,
                outcome.backend_used,
                outcome.converged,
                outcome.iterations,
                outcome.true_residual_norm,
                outcome.true_residual_norm / rhs_norm.max(f64::EPSILON),
                outcome
                    .failure_reason
                    .map(|reason| format!(" reason={reason}"))
                    .unwrap_or_default(),
            );
        }

        // Stop-condition 1 accounting: the direct reference must solve every system.
        let lu = &outcomes[0];
        let lu_ok = lu.converged && lu.true_residual_norm / rhs_norm.max(f64::EPSILON) < 1e-6;
        if !lu_ok {
            lu_failures += 1;
            println!("    !! sparse-LU reference did NOT cleanly solve this system");
        }

        if outcomes[1].converged {
            gmres_ilu0_converged += 1;
        }
        if outcomes[2].converged {
            cpr_converged += 1;
        } else {
            cpr_reproduced_failure += 1;
        }
    }

    println!("=== aggregate over {} systems ===", systems.len());
    println!("sparse-lu reference failures: {lu_failures}");
    println!("gmres-ilu0 converged: {gmres_ilu0_converged}");
    println!("fgmres-cpr converged: {cpr_converged}");
    println!("fgmres-cpr reproduced live failure offline: {cpr_reproduced_failure}");

    // Stop condition 1: direct solve struggling means assembly-level trouble at those
    // states — the lab's comparisons would be meaningless.
    assert_eq!(
        lu_failures, 0,
        "sparse-LU reference failed on {lu_failures} captured systems — problem is upstream \
         of the preconditioner (plan stop condition 1)"
    );

    // Stop condition 2: if the current CPR path converges offline on most systems that
    // failed live, the capture is missing state and cannot be trusted for comparisons.
    assert!(
        cpr_reproduced_failure * 2 >= systems.len(),
        "current FgmresCpr converged offline on {}/{} systems that failed live — capture \
         fidelity is suspect (plan stop condition 2)",
        cpr_converged,
        systems.len()
    );
}

/// Replay the one Y2b2b decision-point artifact through CPR and two independent direct paths.
///
/// This is deliberately diagnostic-only: it first distinguishes sparse conversion from faer
/// factorization failure, then checks whether dense LU can solve the same full system. It does
/// not modify production solver selection or manufacture a convergence-policy verdict.
#[test]
#[ignore]
fn replay_y2b2_exact_capture() {
    let dir = y2b2_capture_dir_from_env()
        .expect("set FIM_Y2B2_CAPTURE_DIR to the directory produced by the Y2b2b driver");
    let systems = load_captures(&dir).expect("load Y2b2 capture");
    assert_eq!(
        systems.len(),
        1,
        "Y2b2 replay requires exactly one isolated capture in {}",
        dir.display()
    );
    let system = &systems[0];
    assert_eq!(system.metadata.newton_iteration, 1);
    assert_eq!(system.metadata.failure_reason, "y2b2-exact-decision");
    let layout = system
        .layout
        .expect("Y2b2 exact system has FIM block layout");

    let cpr_options = FimLinearSolveOptions::default();
    let direct_options = FimLinearSolveOptions {
        kind: FimLinearSolverKind::SparseLuDebug,
        ..FimLinearSolveOptions::default()
    };
    let dense_options = FimLinearSolveOptions {
        kind: FimLinearSolverKind::DenseLuDebug,
        ..FimLinearSolveOptions::default()
    };
    let sparse_lu_diagnostics = sparse_lu_debug::diagnose(&system.jacobian);
    let svd = dense_svd_replay(&system.jacobian, &system.rhs)
        .expect("dense SVD vectors are requested for the Y2b2 direct oracle");
    let cpr = solve_linearized_system(
        &system.jacobian,
        &system.rhs,
        &cpr_options,
        Some(layout),
        system.equation_scaling.as_ref(),
    );
    let direct = solve_linearized_system(
        &system.jacobian,
        &system.rhs,
        &direct_options,
        Some(layout),
        system.equation_scaling.as_ref(),
    );
    let dense = solve_linearized_system(
        &system.jacobian,
        &system.rhs,
        &dense_options,
        Some(layout),
        system.equation_scaling.as_ref(),
    );
    assert!(cpr.solution.iter().all(|value| value.is_finite()));
    assert!(direct.solution.iter().all(|value| value.is_finite()));
    assert!(dense.solution.iter().all(|value| value.is_finite()));
    assert!(cpr.reduction().is_finite());
    assert!(direct.reduction().is_finite());
    assert!(dense.reduction().is_finite());

    let cpr_residual = residual_vector(&system.jacobian, &cpr.solution, &system.rhs);
    let direct_residual = residual_vector(&system.jacobian, &direct.solution, &system.rhs);
    let dense_residual = residual_vector(&system.jacobian, &dense.solution, &system.rhs);
    let svd_residual = residual_vector(&system.jacobian, &svd.solution, &system.rhs);
    let (cpr_reservoir, cpr_well) = y2b2_partition_norms(layout, &cpr_residual);
    let (direct_reservoir, direct_well) = y2b2_partition_norms(layout, &direct_residual);
    let (dense_reservoir, dense_well) = y2b2_partition_norms(layout, &dense_residual);
    let (svd_reservoir, svd_well) = y2b2_partition_norms(layout, &svd_residual);
    let correction_difference = cpr
        .solution
        .iter()
        .zip(direct.solution.iter())
        .map(|(left, right)| (left - right).abs())
        .fold(0.0_f64, f64::max);
    let bhp_start = layout.well_bhp_start();
    let bhp_end = layout.well_bhp_end();
    let rate_start = layout.perforation_tail_start;
    let cpr_bhp: Vec<f64> = cpr
        .solution
        .rows(bhp_start, bhp_end - bhp_start)
        .iter()
        .copied()
        .collect();
    let cpr_rates: Vec<f64> = cpr
        .solution
        .rows(rate_start, cpr.solution.len() - rate_start)
        .iter()
        .copied()
        .collect();
    let direct_bhp: Vec<f64> = direct
        .solution
        .rows(bhp_start, bhp_end - bhp_start)
        .iter()
        .copied()
        .collect();
    let direct_rates: Vec<f64> = direct
        .solution
        .rows(rate_start, direct.solution.len() - rate_start)
        .iter()
        .copied()
        .collect();
    let dense_bhp: Vec<f64> = dense
        .solution
        .rows(bhp_start, bhp_end - bhp_start)
        .iter()
        .copied()
        .collect();
    let dense_rates: Vec<f64> = dense
        .solution
        .rows(rate_start, dense.solution.len() - rate_start)
        .iter()
        .copied()
        .collect();
    let dense_cpr_difference = cpr
        .solution
        .iter()
        .zip(dense.solution.iter())
        .map(|(left, right)| (left - right).abs())
        .fold(0.0_f64, f64::max);
    let svd_cpr_difference = cpr
        .solution
        .iter()
        .zip(svd.solution.iter())
        .map(|(left, right)| (left - right).abs())
        .fold(0.0_f64, f64::max);
    let empty_families = y2b2_empty_column_families(
        layout,
        &sparse_lu_diagnostics.structure.empty_column_indices,
    );

    println!(
        "Y2B2-STRUCTURE rows={} nnz={} empty_rows={} empty_columns={} empty_column_families={{water={}, oil_component={}, gas_component={}, well_bhp={}, perforation_rate={}}} empty_column_sample={:?} duplicate_entries={} non_finite_entries={} all_zero_rows={} potential_zero_pivot_rows={} sparse_lu_preparation={}",
        system.jacobian.rows(),
        system.jacobian.nnz(),
        sparse_lu_diagnostics.structure.empty_rows,
        sparse_lu_diagnostics.structure.empty_columns,
        empty_families[0],
        empty_families[1],
        empty_families[2],
        empty_families[3],
        empty_families[4],
        &sparse_lu_diagnostics.structure.empty_column_indices[..sparse_lu_diagnostics
            .structure
            .empty_column_indices
            .len()
            .min(12)],
        sparse_lu_diagnostics.structure.duplicate_entries,
        sparse_lu_diagnostics.structure.non_finite_entries,
        sparse_lu_diagnostics.structure.all_zero_rows,
        sparse_lu_diagnostics.structure.potential_zero_pivot_rows,
        sparse_lu_diagnostics.preparation.label(),
    );
    println!(
        "Y2B2-REPLAY rows={} nnz={} rhs_norm={:.6e} cpr={{backend={}, converged={}, iterations={}, reduction={:.6e}, residual={:.6e}, reservoir={:.6e}, well={:.6e}, bhp={:?}, rate={:?}}} sparse_direct={{backend={}, converged={}, iterations={}, reduction={:.6e}, residual={:.6e}, reservoir={:.6e}, well={:.6e}, bhp={:?}, rate={:?}}} dense_lu={{backend={}, converged={}, iterations={}, reduction={:.6e}, residual={:.6e}, reservoir={:.6e}, well={:.6e}, bhp={:?}, rate={:?}, max_dx_delta_from_cpr={:.6e}}} dense_svd={{rank={}, cutoff={:.6e}, reduction={:.6e}, residual={:.6e}, reservoir={:.6e}, well={:.6e}, max_dx_delta_from_cpr={:.6e}}} sparse_max_dx_delta={:.6e}",
        system.jacobian.rows(),
        system.jacobian.nnz(),
        system.rhs.norm(),
        cpr.backend_used.label(),
        cpr.converged,
        cpr.iterations,
        cpr.reduction(),
        cpr_residual.norm(),
        cpr_reservoir,
        cpr_well,
        cpr_bhp,
        cpr_rates,
        direct.backend_used.label(),
        direct.converged,
        direct.iterations,
        direct.reduction(),
        direct_residual.norm(),
        direct_reservoir,
        direct_well,
        direct_bhp,
        direct_rates,
        dense.backend_used.label(),
        dense.converged,
        dense.iterations,
        dense.reduction(),
        dense_residual.norm(),
        dense_reservoir,
        dense_well,
        dense_bhp,
        dense_rates,
        dense_cpr_difference,
        svd.rank,
        svd.singular_value_cutoff,
        svd_residual.norm() / system.rhs.norm().max(f64::EPSILON),
        svd_residual.norm(),
        svd_reservoir,
        svd_well,
        svd_cpr_difference,
        correction_difference,
    );

    assert_eq!(sparse_lu_diagnostics.structure.empty_rows, 0);
    assert_eq!(sparse_lu_diagnostics.structure.duplicate_entries, 0);
    assert_eq!(sparse_lu_diagnostics.structure.non_finite_entries, 0);
    assert_eq!(sparse_lu_diagnostics.structure.all_zero_rows, 0);
    assert!(
        !dense.converged,
        "the structural singularity must reject dense LU"
    );
    assert!(
        svd.solution.iter().all(|value| value.is_finite())
            && svd_residual.norm() / system.rhs.norm().max(f64::EPSILON) < 1e-10,
        "rank-revealing dense SVD must solve the compatible captured system"
    );
}

#[derive(Clone, Copy, Debug)]
struct Y2b3CorrectionPeaks {
    pressure: f64,
    water_saturation: f64,
    hydrocarbon_primary: f64,
    well_bhp: f64,
    perforation_rate: f64,
}

fn y2b3_correction_peaks(
    layout: FimLinearBlockLayout,
    correction: &DVector<f64>,
) -> Y2b3CorrectionPeaks {
    let mut peaks = Y2b3CorrectionPeaks {
        pressure: 0.0,
        water_saturation: 0.0,
        hydrocarbon_primary: 0.0,
        well_bhp: 0.0,
        perforation_rate: 0.0,
    };
    for cell_idx in 0..layout.cell_block_count {
        peaks.pressure = peaks.pressure.max(correction[3 * cell_idx].abs());
        peaks.water_saturation = peaks
            .water_saturation
            .max(correction[3 * cell_idx + 1].abs());
        peaks.hydrocarbon_primary = peaks
            .hydrocarbon_primary
            .max(correction[3 * cell_idx + 2].abs());
    }
    for value in correction.rows(layout.well_bhp_start(), layout.well_bhp_count) {
        peaks.well_bhp = peaks.well_bhp.max(value.abs());
    }
    for value in correction.rows(
        layout.perforation_tail_start,
        correction.len() - layout.perforation_tail_start,
    ) {
        peaks.perforation_rate = peaks.perforation_rate.max(value.abs());
    }
    peaks
}

fn y2b3_correction_delta_peaks(
    layout: FimLinearBlockLayout,
    left: &DVector<f64>,
    right: &DVector<f64>,
) -> Y2b3CorrectionPeaks {
    y2b3_correction_peaks(layout, &(left - right))
}

fn y2b3_max_peak(peaks: Y2b3CorrectionPeaks) -> f64 {
    peaks
        .pressure
        .max(peaks.water_saturation)
        .max(peaks.hydrocarbon_primary)
        .max(peaks.well_bhp)
        .max(peaks.perforation_rate)
}

/// Y2b3c's full-rank replacement for the historical singular Y2b2 replay.
///
/// The exact same `dt=0.00898425`, iteration-1 decision system must now have a live fixed-layout
/// primary column for every cell. CPR and two independent ordinary direct solvers are compared
/// with backend-neutral full-system residuals and correction partitions before any nonlinear
/// behavior verdict is allowed.
#[test]
#[ignore]
fn replay_y2b3c_exact_capture() {
    let dir = y2b2_capture_dir_from_env()
        .expect("set FIM_Y2B2_CAPTURE_DIR to the directory produced by the Y2b3c driver");
    let systems = load_captures(&dir).expect("load Y2b3c capture");
    assert_eq!(systems.len(), 1, "Y2b3c requires one isolated capture");
    let system = &systems[0];
    assert_eq!(system.metadata.newton_iteration, 1);
    assert_eq!(system.metadata.failure_reason, "y2b3c-exact-decision");
    let layout = system.layout.expect("Y2b3c capture has block layout");
    let structure = sparse_lu_debug::diagnose(&system.jacobian);

    let cpr = solve_linearized_system(
        &system.jacobian,
        &system.rhs,
        &FimLinearSolveOptions::default(),
        Some(layout),
        system.equation_scaling.as_ref(),
    );
    let sparse = solve_linearized_system(
        &system.jacobian,
        &system.rhs,
        &FimLinearSolveOptions {
            kind: FimLinearSolverKind::SparseLuDebug,
            ..FimLinearSolveOptions::default()
        },
        Some(layout),
        system.equation_scaling.as_ref(),
    );
    let dense = solve_linearized_system(
        &system.jacobian,
        &system.rhs,
        &FimLinearSolveOptions {
            kind: FimLinearSolverKind::DenseLuDebug,
            ..FimLinearSolveOptions::default()
        },
        Some(layout),
        system.equation_scaling.as_ref(),
    );

    let cpr_residual = residual_vector(&system.jacobian, &cpr.solution, &system.rhs);
    let sparse_residual = residual_vector(&system.jacobian, &sparse.solution, &system.rhs);
    let dense_residual = residual_vector(&system.jacobian, &dense.solution, &system.rhs);
    let cpr_partitions = y2b2_partition_norms(layout, &cpr_residual);
    let sparse_partitions = y2b2_partition_norms(layout, &sparse_residual);
    let dense_partitions = y2b2_partition_norms(layout, &dense_residual);
    let cpr_peaks = y2b3_correction_peaks(layout, &cpr.solution);
    let sparse_peaks = y2b3_correction_peaks(layout, &sparse.solution);
    let dense_peaks = y2b3_correction_peaks(layout, &dense.solution);
    let cpr_sparse_delta = y2b3_correction_delta_peaks(layout, &cpr.solution, &sparse.solution);
    let sparse_dense_delta = y2b3_correction_delta_peaks(layout, &sparse.solution, &dense.solution);

    println!(
        "Y2B3C-STRUCTURE rows={} nnz={} empty_rows={} empty_columns={} duplicates={} non_finite={} all_zero_rows={} zero_diagonal_candidates={} sparse_preparation={}",
        system.jacobian.rows(),
        system.jacobian.nnz(),
        structure.structure.empty_rows,
        structure.structure.empty_columns,
        structure.structure.duplicate_entries,
        structure.structure.non_finite_entries,
        structure.structure.all_zero_rows,
        structure.structure.potential_zero_pivot_rows,
        structure.preparation.label(),
    );
    println!(
        "Y2B3C-REPLAY rhs_norm={:.9e} cpr={{converged={},iters={},reduction={:.9e},residual={:.9e},reservoir={:.9e},well={:.9e},dx={:?}}} sparse={{converged={},iters={},reduction={:.9e},residual={:.9e},reservoir={:.9e},well={:.9e},dx={:?}}} dense={{converged={},iters={},reduction={:.9e},residual={:.9e},reservoir={:.9e},well={:.9e},dx={:?}}} deltas={{cpr_sparse={:?},sparse_dense={:?}}}",
        system.rhs.norm(),
        cpr.converged,
        cpr.iterations,
        cpr.reduction(),
        cpr_residual.norm(),
        cpr_partitions.0,
        cpr_partitions.1,
        cpr_peaks,
        sparse.converged,
        sparse.iterations,
        sparse.reduction(),
        sparse_residual.norm(),
        sparse_partitions.0,
        sparse_partitions.1,
        sparse_peaks,
        dense.converged,
        dense.iterations,
        dense.reduction(),
        dense_residual.norm(),
        dense_partitions.0,
        dense_partitions.1,
        dense_peaks,
        cpr_sparse_delta,
        sparse_dense_delta,
    );

    assert_eq!(structure.structure.empty_rows, 0);
    assert_eq!(structure.structure.empty_columns, 0);
    assert_eq!(structure.structure.duplicate_entries, 0);
    assert_eq!(structure.structure.non_finite_entries, 0);
    assert_eq!(structure.structure.all_zero_rows, 0);
    assert_eq!(structure.structure.potential_zero_pivot_rows, 0);
    assert_eq!(
        structure.preparation,
        sparse_lu_debug::SparseLuPreparation::Factorized
    );
    for report in [&cpr, &sparse, &dense] {
        assert!(report.converged);
        assert!(report.solution.iter().all(|value| value.is_finite()));
        assert!(report.reduction().is_finite());
        assert!((report.rhs_norm - system.rhs.norm()).abs() < 1e-12);
    }
    assert!(cpr.reduction() < 5e-3);
    assert!(sparse.reduction() < 1e-10);
    assert!(dense.reduction() < 1e-10);
    assert!(
        y2b3_max_peak(cpr_sparse_delta) < 1e-5,
        "CPR/direct corrections disagree by family: {cpr_sparse_delta:?}"
    );
    assert!(
        y2b3_max_peak(sparse_dense_delta) < 1e-10,
        "independent direct corrections disagree: {sparse_dense_delta:?}"
    );
}

/// Y2d0's single-system oracle for the first `22x22x1` lifecycle-candidate linear retry.
///
/// This deliberately replays one isolated live-failure artifact rather than treating a corpus
/// aggregate or a backend-specific `converged` flag as evidence. Both returned corrections are
/// checked against the same full Jacobian/RHS, split into reservoir and recovered-well rows, and
/// compared by unknown family. It is diagnostic infrastructure only; it does not change solver
/// selection or nonlinear acceptance.
#[test]
#[ignore]
fn replay_y2d0_first_bounded_control_failure() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a directory containing the isolated Y2d0 capture");
    let systems = load_captures(&dir).expect("load Y2d0 capture");
    assert_eq!(systems.len(), 1, "Y2d0 requires one isolated capture");
    let system = &systems[0];
    assert_eq!(system.metadata.newton_iteration, 0);
    assert_eq!(system.metadata.failure_reason, "max-iters");
    let layout = system.layout.expect("Y2d0 capture has block layout");
    let structure = sparse_lu_debug::diagnose(&system.jacobian);

    let cpr = solve_linearized_system(
        &system.jacobian,
        &system.rhs,
        &FimLinearSolveOptions::default(),
        Some(layout),
        system.equation_scaling.as_ref(),
    );
    let direct = solve_linearized_system(
        &system.jacobian,
        &system.rhs,
        &FimLinearSolveOptions {
            kind: FimLinearSolverKind::SparseLuDebug,
            ..FimLinearSolveOptions::default()
        },
        Some(layout),
        system.equation_scaling.as_ref(),
    );

    let cpr_residual = residual_vector(&system.jacobian, &cpr.solution, &system.rhs);
    let direct_residual = residual_vector(&system.jacobian, &direct.solution, &system.rhs);
    let cpr_partitions = y2b2_partition_norms(layout, &cpr_residual);
    let direct_partitions = y2b2_partition_norms(layout, &direct_residual);
    let cpr_peaks = y2b3_correction_peaks(layout, &cpr.solution);
    let direct_peaks = y2b3_correction_peaks(layout, &direct.solution);
    let correction_delta = y2b3_correction_delta_peaks(layout, &cpr.solution, &direct.solution);

    println!(
        "Y2D0-STRUCTURE rows={} nnz={} empty_rows={} empty_columns={} duplicates={} non_finite={} all_zero_rows={} zero_diagonal_candidates={} sparse_preparation={}",
        system.jacobian.rows(),
        system.jacobian.nnz(),
        structure.structure.empty_rows,
        structure.structure.empty_columns,
        structure.structure.duplicate_entries,
        structure.structure.non_finite_entries,
        structure.structure.all_zero_rows,
        structure.structure.potential_zero_pivot_rows,
        structure.preparation.label(),
    );
    println!(
        "Y2D0-REPLAY rhs_norm={:.9e} cpr={{converged={},finite={},iters={},report_residual={:.9e},true_residual={:.9e},reduction={:.9e},reservoir={:.9e},well={:.9e},dx={:?}}} direct={{converged={},finite={},iters={},report_residual={:.9e},true_residual={:.9e},reduction={:.9e},reservoir={:.9e},well={:.9e},dx={:?}}} correction_delta={:?}",
        system.rhs.norm(),
        cpr.converged,
        cpr.solution.iter().all(|value| value.is_finite()),
        cpr.iterations,
        cpr.final_residual_norm,
        cpr_residual.norm(),
        cpr.reduction(),
        cpr_partitions.0,
        cpr_partitions.1,
        cpr_peaks,
        direct.converged,
        direct.solution.iter().all(|value| value.is_finite()),
        direct.iterations,
        direct.final_residual_norm,
        direct_residual.norm(),
        direct.reduction(),
        direct_partitions.0,
        direct_partitions.1,
        direct_peaks,
        correction_delta,
    );

    assert_eq!(structure.structure.empty_rows, 0);
    assert_eq!(structure.structure.empty_columns, 0);
    assert_eq!(structure.structure.duplicate_entries, 0);
    assert_eq!(structure.structure.non_finite_entries, 0);
    assert_eq!(structure.structure.all_zero_rows, 0);
    assert_eq!(structure.structure.potential_zero_pivot_rows, 0);
    assert_eq!(
        structure.preparation,
        sparse_lu_debug::SparseLuPreparation::Factorized
    );
    for report in [&cpr, &direct] {
        assert!(report.solution.iter().all(|value| value.is_finite()));
        assert!(report.reduction().is_finite());
        assert!((report.rhs_norm - system.rhs.norm()).abs() < 1e-12);
    }
    assert!((cpr.final_residual_norm - cpr_residual.norm()).abs() < 1e-10);
    assert!((direct.final_residual_norm - direct_residual.norm()).abs() < 1e-10);
    assert!(direct.converged);
    assert!(direct.reduction() < 1e-10);
}

struct VariantOutcome {
    kind: CprPressureRestrictionKind,
    converged: bool,
    true_residual_norm: f64,
}

fn run_restriction_variant(
    kind: CprPressureRestrictionKind,
    system: &FimCapturedSystem,
) -> VariantOutcome {
    let options = FimLinearSolveOptions {
        kind: FimLinearSolverKind::FgmresCpr,
        ..FimLinearSolveOptions::default()
    };
    let report =
        solve_with_restriction_kind(&system.jacobian, &system.rhs, &options, system.layout, kind);
    VariantOutcome {
        kind,
        converged: report.converged,
        true_residual_norm: residual_norm(&system.jacobian, &report.solution, &system.rhs),
    }
}

/// Compares every salvaged/new CPR pressure-restriction variant (`CprPressureRestrictionKind::ALL`
/// — the four restrictions salvaged from the reverted in-situ probe plus the new OPM-style
/// `QuasiImpes` weighting) as full solves against a captured corpus. This is the actual
/// systematic component test the Phase 9 lab exists for: a new restriction hypothesis is one
/// enum arm plus this rerun, not a live-solver change and wasm replay.
///
/// Manual lab entry point; requires FIM_CAPTURE_DIR pointing at a captured corpus.
#[test]
#[ignore]
fn solver_lab_compare_restriction_variants() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a directory produced by a capture driver run");
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    println!(
        "restriction-variant lab: {} captured systems from {}",
        systems.len(),
        dir.display()
    );

    let variants = CprPressureRestrictionKind::ALL;
    let mut converged_counts = vec![0usize; variants.len()];
    let mut win_counts = vec![0usize; variants.len()];
    let mut relative_residuals: Vec<Vec<f64>> = vec![Vec::new(); variants.len()];

    for (index, system) in systems.iter().enumerate() {
        let rhs_norm = system.rhs.norm().max(f64::EPSILON);
        let outcomes: Vec<VariantOutcome> = variants
            .iter()
            .map(|&kind| run_restriction_variant(kind, system))
            .collect();

        println!(
            "--- capture {index:05}: rows={} newton_iter={} live_reason={} dominant={}@{}",
            system.jacobian.rows(),
            system.metadata.newton_iteration,
            system.metadata.failure_reason,
            system.metadata.dominant_family,
            system.metadata.dominant_item_index,
        );
        let mut best_variant_idx = 0usize;
        let mut best_rel = f64::INFINITY;
        for (variant_idx, outcome) in outcomes.iter().enumerate() {
            let rel = outcome.true_residual_norm / rhs_norm;
            println!(
                "    {:<20} converged={:<5} true_res={:.3e} rel={:.3e}",
                outcome.kind.label(),
                outcome.converged,
                outcome.true_residual_norm,
                rel,
            );
            if outcome.converged {
                converged_counts[variant_idx] += 1;
            }
            relative_residuals[variant_idx].push(rel);
            if rel < best_rel {
                best_rel = rel;
                best_variant_idx = variant_idx;
            }
        }
        win_counts[best_variant_idx] += 1;
    }

    println!("=== aggregate over {} systems ===", systems.len());
    for (variant_idx, &kind) in variants.iter().enumerate() {
        let mut residuals = relative_residuals[variant_idx].clone();
        residuals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = residuals[residuals.len() / 2];
        println!(
            "{:<20} converged={:>3}/{} wins={:>3}/{} median_rel_res={:.3e}",
            kind.label(),
            converged_counts[variant_idx],
            systems.len(),
            win_counts[variant_idx],
            systems.len(),
            median,
        );
    }
}

/// Y2d1 production-faithful restriction discrimination.
///
/// Unlike `solver_lab_compare_restriction_variants`, every candidate here passes through the
/// live well-Schur reduction/recovery, block-ILU0 smoother, captured equation scaling, tolerance,
/// and iteration budget. Full-system residuals and direct-correction deltas are recomputed from
/// the original artifact. Run separately on the bounded and gas corpora, identifying the corpus
/// with `FIM_Y2D1_CORPUS` for an unambiguous evidence record.
#[test]
#[ignore]
fn solver_lab_compare_production_restriction_variants() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a bounded or gas Y2d1 capture corpus");
    let corpus = std::env::var("FIM_Y2D1_CORPUS").unwrap_or_else(|_| "unspecified".to_string());
    let systems = load_captures(&dir).expect("load Y2d1 capture corpus");
    assert!(!systems.is_empty(), "Y2d1 corpus must not be empty");

    let variants = CprPressureRestrictionKind::ALL;
    let mut strict_counts = vec![0usize; variants.len()];
    let mut relaxed_counts = vec![0usize; variants.len()];
    let mut relative_residuals = vec![Vec::<f64>::new(); variants.len()];
    let mut correction_deltas = vec![Vec::<f64>::new(); variants.len()];
    let mut reservoir_residuals = vec![Vec::<f64>::new(); variants.len()];
    let mut well_residuals = vec![Vec::<f64>::new(); variants.len()];
    let mut direct_failures = 0usize;

    for (capture_idx, system) in systems.iter().enumerate() {
        let layout = system.layout.expect("Y2d1 capture requires block layout");
        let options = FimLinearSolveOptions::default();
        let production = solve_linearized_system(
            &system.jacobian,
            &system.rhs,
            &options,
            Some(layout),
            system.equation_scaling.as_ref(),
        );
        let direct = solve_linearized_system(
            &system.jacobian,
            &system.rhs,
            &FimLinearSolveOptions {
                kind: FimLinearSolverKind::SparseLuDebug,
                ..options
            },
            Some(layout),
            system.equation_scaling.as_ref(),
        );
        let direct_residual = residual_vector(&system.jacobian, &direct.solution, &system.rhs);
        let rhs_norm = system.rhs.norm().max(1e-30);
        if !direct.converged
            || !direct.solution.iter().all(|value| value.is_finite())
            || direct_residual.norm() / rhs_norm >= 1e-10
        {
            direct_failures += 1;
        }

        for (variant_idx, &variant) in variants.iter().enumerate() {
            let report = well_schur::solve_with_well_elimination_and_restriction(
                &system.jacobian,
                &system.rhs,
                &options,
                layout,
                system.equation_scaling.as_ref(),
                variant,
            );
            let residual = residual_vector(&system.jacobian, &report.solution, &system.rhs);
            let relative_residual = residual.norm() / rhs_norm;
            let partitions = y2b2_partition_norms(layout, &residual);
            let correction_delta = y2b3_max_peak(y2b3_correction_delta_peaks(
                layout,
                &report.solution,
                &direct.solution,
            ));
            let finite = report.solution.iter().all(|value| value.is_finite());

            assert!((report.rhs_norm - system.rhs.norm()).abs() < 1e-10);
            assert!(
                (report.final_residual_norm - residual.norm()).abs()
                    <= 1e-10 * residual.norm().max(1.0),
                "capture {capture_idx} variant {} report/full residual mismatch",
                variant.label()
            );
            if variant == CprPressureRestrictionKind::QuasiImpes {
                assert_eq!(report.converged, production.converged);
                assert_eq!(report.iterations, production.iterations);
                assert!(
                    y2b3_max_peak(y2b3_correction_delta_peaks(
                        layout,
                        &report.solution,
                        &production.solution,
                    )) < 1e-12,
                    "capture {capture_idx}: injected quasi-IMPES differs from production"
                );
            }

            if report.converged && finite {
                strict_counts[variant_idx] += 1;
            }
            if finite && relative_residual < 1e-2 {
                relaxed_counts[variant_idx] += 1;
            }
            println!(
                "Y2D1 capture={capture_idx:05} variant={:<20} strict={} relaxed={} finite={} rel={:.9e} reservoir_rel={:.9e} well_rel={:.9e} direct_dx_delta={:.9e}",
                variant.label(),
                report.converged && finite,
                finite && relative_residual < 1e-2,
                finite,
                relative_residual,
                partitions.0 / rhs_norm,
                partitions.1 / rhs_norm,
                correction_delta,
            );
            relative_residuals[variant_idx].push(relative_residual);
            correction_deltas[variant_idx].push(correction_delta);
            reservoir_residuals[variant_idx].push(partitions.0 / rhs_norm);
            well_residuals[variant_idx].push(partitions.1 / rhs_norm);
        }
    }

    println!(
        "Y2D1 production restriction corpus={corpus} systems={} dir={}",
        systems.len(),
        dir.display()
    );
    for (variant_idx, &variant) in variants.iter().enumerate() {
        for values in [
            &mut relative_residuals[variant_idx],
            &mut correction_deltas[variant_idx],
            &mut reservoir_residuals[variant_idx],
            &mut well_residuals[variant_idx],
        ] {
            values.sort_by(|left, right| {
                left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        let median = |values: &[f64]| values[values.len() / 2];
        println!(
            "Y2D1 variant={:<20} strict={}/{} relaxed={}/{} median_rel={:.9e} median_reservoir_rel={:.9e} median_well_rel={:.9e} median_direct_dx_delta={:.9e}",
            variant.label(),
            strict_counts[variant_idx],
            systems.len(),
            relaxed_counts[variant_idx],
            systems.len(),
            median(&relative_residuals[variant_idx]),
            median(&reservoir_residuals[variant_idx]),
            median(&well_residuals[variant_idx]),
            median(&correction_deltas[variant_idx]),
        );
    }

    assert_eq!(
        direct_failures, 0,
        "Y2d1 direct oracle failed on {direct_failures} captures"
    );
}

#[derive(Clone, Copy)]
struct Y2d2Row {
    smoother: CprFineSmootherKind,
    max_iterations: usize,
    effective_budget: usize,
}

/// Y2d2 production-faithful smoother and Krylov-budget isolation with quasi-IMPES fixed.
///
/// `FIM_Y2D2_MODE=smoother` compares every existing fine smoother at production's effective
/// 30-iteration budget. `FIM_Y2D2_MODE=budget` requires `FIM_Y2D2_SMOOTHER` and compares effective
/// budgets 30/60/150 for that one selected smoother. Run both modes separately on the bounded and
/// gas corpora; no configuration is wired into production by this test.
#[test]
#[ignore]
fn solver_lab_compare_production_smoother_and_budget() {
    let dir = capture_dir_from_env().expect("set FIM_CAPTURE_DIR to a Y2d2 capture corpus");
    let corpus = std::env::var("FIM_Y2D1_CORPUS").unwrap_or_else(|_| "unspecified".to_string());
    let mode = std::env::var("FIM_Y2D2_MODE").unwrap_or_else(|_| "smoother".to_string());
    let systems = load_captures(&dir).expect("load Y2d2 capture corpus");
    assert!(!systems.is_empty(), "Y2d2 corpus must not be empty");

    let rows: Vec<Y2d2Row> = match mode.as_str() {
        "smoother" => CprFineSmootherKind::ALL
            .iter()
            .map(|&smoother| Y2d2Row {
                smoother,
                // Production config is maxiter=20 with restart=30; the kernel deliberately uses
                // max(maxiter, restart), so the effective baseline budget is exactly 30.
                max_iterations: 20,
                effective_budget: 30,
            })
            .collect(),
        "budget" => {
            let smoother = match std::env::var("FIM_Y2D2_SMOOTHER")
                .expect("budget mode requires FIM_Y2D2_SMOOTHER")
                .as_str()
            {
                "block-ilu0" => CprFineSmootherKind::BlockIlu0,
                "ilu0" => CprFineSmootherKind::FullIlu0,
                "block-jacobi" => CprFineSmootherKind::BlockJacobi,
                value => panic!("unsupported FIM_Y2D2_SMOOTHER={value}"),
            };
            [(20, 30), (60, 60), (150, 150)]
                .iter()
                .map(|&(max_iterations, effective_budget)| Y2d2Row {
                    smoother,
                    max_iterations,
                    effective_budget,
                })
                .collect()
        }
        value => panic!("FIM_Y2D2_MODE must be smoother|budget, got {value}"),
    };

    let mut strict_counts = vec![0usize; rows.len()];
    let mut relaxed_counts = vec![0usize; rows.len()];
    let mut relative_residuals = vec![Vec::<f64>::new(); rows.len()];
    let mut correction_deltas = vec![Vec::<f64>::new(); rows.len()];
    let mut reservoir_residuals = vec![Vec::<f64>::new(); rows.len()];
    let mut well_residuals = vec![Vec::<f64>::new(); rows.len()];
    let mut iteration_counts = vec![Vec::<usize>::new(); rows.len()];
    let mut direct_failures = 0usize;

    for (capture_idx, system) in systems.iter().enumerate() {
        let layout = system.layout.expect("Y2d2 capture requires block layout");
        let production_options = FimLinearSolveOptions::default();
        let production = solve_linearized_system(
            &system.jacobian,
            &system.rhs,
            &production_options,
            Some(layout),
            system.equation_scaling.as_ref(),
        );
        let direct = solve_linearized_system(
            &system.jacobian,
            &system.rhs,
            &FimLinearSolveOptions {
                kind: FimLinearSolverKind::SparseLuDebug,
                ..production_options
            },
            Some(layout),
            system.equation_scaling.as_ref(),
        );
        let direct_residual = residual_vector(&system.jacobian, &direct.solution, &system.rhs);
        let rhs_norm = system.rhs.norm().max(1e-30);
        if !direct.converged
            || !direct.solution.iter().all(|value| value.is_finite())
            || direct_residual.norm() / rhs_norm >= 1e-10
        {
            direct_failures += 1;
        }

        for (row_idx, row) in rows.iter().enumerate() {
            let options = FimLinearSolveOptions {
                max_iterations: row.max_iterations,
                ..production_options
            };
            let report = well_schur::solve_with_well_elimination_and_configuration(
                &system.jacobian,
                &system.rhs,
                &options,
                layout,
                system.equation_scaling.as_ref(),
                row.smoother,
                CprPressureRestrictionKind::QuasiImpes,
            );
            let residual = residual_vector(&system.jacobian, &report.solution, &system.rhs);
            let relative_residual = residual.norm() / rhs_norm;
            let partitions = y2b2_partition_norms(layout, &residual);
            let correction_delta = y2b3_max_peak(y2b3_correction_delta_peaks(
                layout,
                &report.solution,
                &direct.solution,
            ));
            let finite = report.solution.iter().all(|value| value.is_finite());

            assert!((report.rhs_norm - system.rhs.norm()).abs() < 1e-10);
            assert!(
                (report.final_residual_norm - residual.norm()).abs()
                    <= 1e-10 * residual.norm().max(1.0),
                "capture {capture_idx} smoother={} budget={} report/full residual mismatch",
                row.smoother.label(),
                row.effective_budget,
            );
            if row.smoother == CprFineSmootherKind::BlockIlu0
                && row.max_iterations == production_options.max_iterations
            {
                assert_eq!(report.converged, production.converged);
                assert_eq!(report.iterations, production.iterations);
                assert!(
                    y2b3_max_peak(y2b3_correction_delta_peaks(
                        layout,
                        &report.solution,
                        &production.solution,
                    )) < 1e-12,
                    "capture {capture_idx}: injected production smoother differs from production"
                );
            }

            if report.converged && finite {
                strict_counts[row_idx] += 1;
            }
            if finite && relative_residual < 1e-2 {
                relaxed_counts[row_idx] += 1;
            }
            println!(
                "Y2D2 capture={capture_idx:05} smoother={:<12} budget={} iters={} strict={} relaxed={} finite={} rel={:.9e} reservoir_rel={:.9e} well_rel={:.9e} direct_dx_delta={:.9e}",
                row.smoother.label(),
                row.effective_budget,
                report.iterations,
                report.converged && finite,
                finite && relative_residual < 1e-2,
                finite,
                relative_residual,
                partitions.0 / rhs_norm,
                partitions.1 / rhs_norm,
                correction_delta,
            );
            relative_residuals[row_idx].push(relative_residual);
            correction_deltas[row_idx].push(correction_delta);
            reservoir_residuals[row_idx].push(partitions.0 / rhs_norm);
            well_residuals[row_idx].push(partitions.1 / rhs_norm);
            iteration_counts[row_idx].push(report.iterations);
        }
    }

    println!(
        "Y2D2 production component corpus={corpus} mode={mode} systems={} dir={}",
        systems.len(),
        dir.display()
    );
    for (row_idx, row) in rows.iter().enumerate() {
        for values in [
            &mut relative_residuals[row_idx],
            &mut correction_deltas[row_idx],
            &mut reservoir_residuals[row_idx],
            &mut well_residuals[row_idx],
        ] {
            values.sort_by(|left, right| {
                left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        iteration_counts[row_idx].sort_unstable();
        let median = |values: &[f64]| values[values.len() / 2];
        println!(
            "Y2D2 smoother={:<12} budget={} strict={}/{} relaxed={}/{} median_iters={} max_iters={} median_rel={:.9e} median_reservoir_rel={:.9e} median_well_rel={:.9e} median_direct_dx_delta={:.9e}",
            row.smoother.label(),
            row.effective_budget,
            strict_counts[row_idx],
            systems.len(),
            relaxed_counts[row_idx],
            systems.len(),
            iteration_counts[row_idx][iteration_counts[row_idx].len() / 2],
            iteration_counts[row_idx].iter().copied().max().unwrap_or(0),
            median(&relative_residuals[row_idx]),
            median(&reservoir_residuals[row_idx]),
            median(&well_residuals[row_idx]),
            median(&correction_deltas[row_idx]),
        );
    }

    assert_eq!(
        direct_failures, 0,
        "Y2d2 direct oracle failed on {direct_failures} captures"
    );
}

/// Y2d3 production-faithful restart-boundary history. Replays only systems that fail the
/// effective production budget of 30, then records every correction through first convergence
/// with budget 60. The iteration-30 candidate must reproduce production exactly.
#[test]
#[ignore]
fn solver_lab_audit_restart_boundary_history() {
    let dir = capture_dir_from_env().expect("set FIM_CAPTURE_DIR to a Y2d3 capture corpus");
    let corpus = std::env::var("FIM_Y2D1_CORPUS").unwrap_or_else(|_| "unspecified".to_string());
    let systems = load_captures(&dir).expect("load Y2d3 capture corpus");
    assert!(!systems.is_empty(), "Y2d3 corpus must not be empty");

    let production_options = FimLinearSolveOptions::default();
    let extended_options = FimLinearSolveOptions {
        max_iterations: 60,
        ..production_options
    };
    let mut hard_systems = 0usize;
    let mut first_cycle_reductions = Vec::new();
    let mut second_cycle_factors = Vec::new();
    let mut final_direct_deltas = Vec::new();
    let mut estimate_disagreement_factors = Vec::new();
    let mut pressure_reduction_ratios_at_30 = Vec::new();

    for (capture_idx, system) in systems.iter().enumerate() {
        let layout = system.layout.expect("Y2d3 capture requires block layout");
        let production = solve_linearized_system(
            &system.jacobian,
            &system.rhs,
            &production_options,
            Some(layout),
            system.equation_scaling.as_ref(),
        );
        if production.converged {
            continue;
        }
        hard_systems += 1;

        let direct = solve_linearized_system(
            &system.jacobian,
            &system.rhs,
            &FimLinearSolveOptions {
                kind: FimLinearSolverKind::SparseLuDebug,
                ..production_options
            },
            Some(layout),
            system.equation_scaling.as_ref(),
        );
        let direct_residual = residual_vector(&system.jacobian, &direct.solution, &system.rhs);
        let rhs_norm = system.rhs.norm().max(1e-30);
        assert!(direct.converged && direct.solution.iter().all(|value| value.is_finite()));
        assert!(direct_residual.norm() / rhs_norm < 1e-10);

        let (extended, history) = well_schur::solve_with_well_elimination_configuration_and_history(
            &system.jacobian,
            &system.rhs,
            &extended_options,
            layout,
            system.equation_scaling.as_ref(),
            CprFineSmootherKind::BlockIlu0,
            CprPressureRestrictionKind::QuasiImpes,
        );
        assert!(
            extended.converged,
            "capture {capture_idx}: budget 60 must converge"
        );
        assert!(matches!(extended.iterations, 31 | 32));
        assert_eq!(history.len(), extended.iterations);

        let at_30 = history
            .iter()
            .find(|snapshot| snapshot.iteration == 30)
            .expect("Y2d3 history must contain iteration 30");
        assert_eq!(at_30.restart_index, 1);
        assert_eq!(at_30.inner_step, 30);
        assert!(
            (at_30.true_residual_norm - production.final_residual_norm).abs()
                <= 1e-10 * production.final_residual_norm.max(1.0)
        );
        assert!(
            y2b3_max_peak(y2b3_correction_delta_peaks(
                layout,
                &at_30.solution,
                &production.solution,
            )) < 1e-12,
            "capture {capture_idx}: iteration-30 candidate differs from production"
        );

        let final_snapshot = history.last().expect("Y2d3 history must not be empty");
        assert_eq!(final_snapshot.iteration, extended.iterations);
        assert_eq!(final_snapshot.restart_index, 2);
        assert!(matches!(final_snapshot.inner_step, 1 | 2));
        assert!(
            (final_snapshot.true_residual_norm - extended.final_residual_norm).abs()
                <= 1e-10 * extended.final_residual_norm.max(1.0)
        );
        assert!(
            y2b3_max_peak(y2b3_correction_delta_peaks(
                layout,
                &final_snapshot.solution,
                &extended.solution,
            )) < 1e-12
        );

        let first_cycle_reduction = at_30.true_residual_norm / rhs_norm;
        let second_cycle_factor =
            final_snapshot.true_residual_norm / at_30.true_residual_norm.max(1e-30);
        let final_direct_delta = y2b3_max_peak(y2b3_correction_delta_peaks(
            layout,
            &extended.solution,
            &direct.solution,
        ));
        first_cycle_reductions.push(first_cycle_reduction);
        second_cycle_factors.push(second_cycle_factor);
        final_direct_deltas.push(final_direct_delta);
        estimate_disagreement_factors.push(
            at_30.actual_preconditioned_residual_norm
                / at_30
                    .estimated_preconditioned_residual_norm
                    .max(f64::MIN_POSITIVE),
        );
        if let Some(pressure_reduction_ratio) = at_30.pressure_reduction_ratio {
            pressure_reduction_ratios_at_30.push(pressure_reduction_ratio);
        }

        for snapshot in history.iter().filter(|snapshot| {
            matches!(snapshot.iteration, 1 | 5 | 10 | 15 | 20 | 25) || snapshot.iteration >= 27
        }) {
            let residual = residual_vector(&system.jacobian, &snapshot.solution, &system.rhs);
            let partitions = y2b2_partition_norms(layout, &residual);
            let direct_delta = y2b3_max_peak(y2b3_correction_delta_peaks(
                layout,
                &snapshot.solution,
                &direct.solution,
            ));
            assert!(
                (snapshot.true_residual_norm - residual.norm()).abs()
                    <= 1e-10 * residual.norm().max(1.0)
            );
            println!(
                "Y2D3 capture={capture_idx:05} iter={} restart={} inner={} true_rel={:.9e} reservoir_rel={:.9e} well_rel={:.9e} estimated_prec={:.9e} actual_prec={:.9e} pressure_rr={} direct_dx_delta={:.9e}",
                snapshot.iteration,
                snapshot.restart_index,
                snapshot.inner_step,
                snapshot.true_residual_norm / rhs_norm,
                partitions.0 / rhs_norm,
                partitions.1 / rhs_norm,
                snapshot.estimated_preconditioned_residual_norm,
                snapshot.actual_preconditioned_residual_norm,
                snapshot
                    .pressure_reduction_ratio
                    .map(|value| format!("{value:.9e}"))
                    .unwrap_or_else(|| "n/a".to_string()),
                direct_delta,
            );
        }
        println!(
            "Y2D3-SUMMARY capture={capture_idx:05} production_iters={} extended_iters={} first_cycle_rel={first_cycle_reduction:.9e} second_cycle_factor={second_cycle_factor:.9e} final_rel={:.9e} final_direct_dx_delta={final_direct_delta:.9e}",
            production.iterations,
            extended.iterations,
            extended.final_residual_norm / rhs_norm,
        );
    }

    assert!(
        hard_systems > 0,
        "Y2d3 corpus contains no production-budget failures"
    );
    for values in [
        &mut first_cycle_reductions,
        &mut second_cycle_factors,
        &mut final_direct_deltas,
        &mut estimate_disagreement_factors,
        &mut pressure_reduction_ratios_at_30,
    ] {
        values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    }
    let median = |values: &[f64]| values[values.len() / 2];
    println!(
        "Y2D3 corpus={corpus} hard={hard_systems}/{} median_first_cycle_rel={:.9e} median_second_cycle_factor={:.9e} median_estimate_disagreement={:.9e} median_pressure_rr_at_30={:.9e} median_final_direct_dx_delta={:.9e} dir={}",
        systems.len(),
        median(&first_cycle_reductions),
        median(&second_cycle_factors),
        median(&estimate_disagreement_factors),
        median(&pressure_reduction_ratios_at_30),
        median(&final_direct_deltas),
        dir.display(),
    );
}

/// Y2d4 offline oracle: compare the production fixed-left recurrence with a mathematically true
/// right-preconditioned flexible-GMRES recurrence while holding every CPR component and the
/// effective 30-iteration budget fixed.
#[test]
#[ignore]
fn solver_lab_compare_true_flexible_gmres() {
    let dir = capture_dir_from_env().expect("set FIM_CAPTURE_DIR to a Y2d4 capture corpus");
    let corpus = std::env::var("FIM_Y2D1_CORPUS").unwrap_or_else(|_| "unspecified".to_string());
    let systems = load_captures(&dir).expect("load Y2d4 capture corpus");
    assert!(!systems.is_empty(), "Y2d4 corpus must not be empty");

    // Production's current loop promotes `max_iterations=20` to the restart length (`30`). Keep
    // that effective budget explicit in the independent oracle rather than inheriting the old
    // loop's accidental `max(max_iterations, restart)` behavior.
    let options = FimLinearSolveOptions {
        max_iterations: 30,
        ..FimLinearSolveOptions::default()
    };
    assert_eq!(options.restart, 30);
    assert_eq!(options.max_iterations, 30);
    let mut production_converged = 0usize;
    let mut flexible_converged = 0usize;
    let mut flexible_iterations = Vec::new();
    let mut flexible_reductions = Vec::new();
    let mut flexible_direct_deltas = Vec::new();
    let mut maximum_estimate_disagreement = 0.0_f64;

    for (capture_idx, system) in systems.iter().enumerate() {
        let layout = system.layout.expect("Y2d4 capture requires block layout");
        let rhs_norm = system.rhs.norm().max(1e-30);
        let production = solve_linearized_system(
            &system.jacobian,
            &system.rhs,
            &options,
            Some(layout),
            system.equation_scaling.as_ref(),
        );
        production_converged += usize::from(production.converged);

        let direct = solve_linearized_system(
            &system.jacobian,
            &system.rhs,
            &FimLinearSolveOptions {
                kind: FimLinearSolverKind::SparseLuDebug,
                ..options
            },
            Some(layout),
            system.equation_scaling.as_ref(),
        );
        let direct_residual = residual_vector(&system.jacobian, &direct.solution, &system.rhs);
        assert!(direct.converged && direct.solution.iter().all(|value| value.is_finite()));
        assert!(direct_residual.norm() / rhs_norm < 1e-10);

        let (flexible, history) = well_schur::solve_with_well_elimination_true_flexible(
            &system.jacobian,
            &system.rhs,
            &options,
            layout,
            system.equation_scaling.as_ref(),
            CprFineSmootherKind::BlockIlu0,
            CprPressureRestrictionKind::QuasiImpes,
        );
        if production.converged {
            assert!(
                flexible.converged,
                "capture {capture_idx}: flexible oracle lost a production pass"
            );
        }
        let flexible_residual = residual_vector(&system.jacobian, &flexible.solution, &system.rhs);
        let partitions = y2b2_partition_norms(layout, &flexible_residual);
        let flexible_reduction = flexible_residual.norm() / rhs_norm;
        let direct_delta = y2b3_max_peak(y2b3_correction_delta_peaks(
            layout,
            &flexible.solution,
            &direct.solution,
        ));
        assert!(flexible.solution.iter().all(|value| value.is_finite()));
        assert!(
            (flexible.final_residual_norm - flexible_residual.norm()).abs()
                <= 1e-10 * flexible_residual.norm().max(1.0)
        );
        assert_eq!(history.len(), flexible.iterations);
        for (history_index, snapshot) in history.iter().enumerate() {
            assert_eq!(snapshot.iteration, history_index + 1);
            assert_eq!(snapshot.restart_index, 1);
            assert_eq!(snapshot.inner_step, history_index + 1);
            assert!(snapshot
                .pressure_reduction_ratio
                .is_none_or(|ratio| ratio.is_finite()));
            let independently_recomputed =
                residual_norm(&system.jacobian, &snapshot.solution, &system.rhs);
            assert!(
                (snapshot.true_residual_norm - independently_recomputed).abs()
                    <= 1e-10 * independently_recomputed.max(1.0)
            );
            let disagreement = (snapshot.estimated_residual_norm - snapshot.true_residual_norm)
                .abs()
                / snapshot.true_residual_norm.max(1e-30);
            maximum_estimate_disagreement = maximum_estimate_disagreement.max(disagreement);
        }

        flexible_converged += usize::from(flexible.converged);
        flexible_iterations.push(flexible.iterations as f64);
        flexible_reductions.push(flexible_reduction);
        flexible_direct_deltas.push(direct_delta);
        println!(
            "Y2D4 capture={capture_idx:05} production={{converged={},iters={},rel={:.9e}}} flexible={{converged={},iters={},rel={flexible_reduction:.9e},reservoir_rel={:.9e},well_rel={:.9e},direct_dx_delta={direct_delta:.9e}}} history={} estimate_disagreement_max={:.9e}",
            production.converged,
            production.iterations,
            production.final_residual_norm / rhs_norm,
            flexible.converged,
            flexible.iterations,
            partitions.0 / rhs_norm,
            partitions.1 / rhs_norm,
            history.len(),
            history
                .iter()
                .map(|snapshot| {
                    (snapshot.estimated_residual_norm - snapshot.true_residual_norm).abs()
                        / snapshot.true_residual_norm.max(1e-30)
                })
                .fold(0.0_f64, f64::max),
        );
    }

    for values in [
        &mut flexible_iterations,
        &mut flexible_reductions,
        &mut flexible_direct_deltas,
    ] {
        values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    }
    let median = |values: &[f64]| values[values.len() / 2];
    println!(
        "Y2D4 corpus={corpus} production={production_converged}/{} flexible={flexible_converged}/{} median_iters={:.0} max_iters={:.0} median_rel={:.9e} median_direct_dx_delta={:.9e} estimate_disagreement_max={maximum_estimate_disagreement:.9e} dir={}",
        systems.len(),
        systems.len(),
        median(&flexible_iterations),
        flexible_iterations.last().copied().unwrap_or(0.0),
        median(&flexible_reductions),
        median(&flexible_direct_deltas),
        dir.display(),
    );
    assert_eq!(
        flexible_converged,
        systems.len(),
        "Y2d4 confirmation requires every captured system to converge"
    );
    assert!(flexible_iterations.last().copied().unwrap_or(0.0) <= 30.0);
    assert!(
        maximum_estimate_disagreement < 1e-6,
        "true FGMRES estimate must track independently recomputed residual"
    );
}

#[derive(Clone)]
struct BundleRow {
    label: String,
    relative_tolerance: f64,
    max_iterations: usize,
    smoother: CprFineSmootherKind,
    // Step 10.1 follow-up (`FIM-LINEAR-008` reopened): when true and the captured system has
    // an `EquationScaling`, require every equation family (not just the whole-system norm) to
    // clear its own relative-reduction target — see `EquationScaling::family_peaks`/
    // `within_relative_reduction` and `gmres_block_jacobi::solve_with_cpr_fine_smoother`'s
    // `family_ok` closure.
    family_aware: bool,
}

fn run_bundle_row(row: &BundleRow, system: &FimCapturedSystem) -> (bool, usize, f64) {
    let options = FimLinearSolveOptions {
        kind: FimLinearSolverKind::FgmresCpr,
        relative_tolerance: row.relative_tolerance,
        // Step 10.0: at OPM-equivalent looseness the old 1e-10 absolute floor is vestigial
        // across the observed rhs_norm range (5.2e-2 to 2.5e3) — drop it to a tiny
        // degenerate-case guard rather than let it silently dominate at small rhs_norm.
        absolute_tolerance: 1e-12,
        max_iterations: row.max_iterations,
        ..FimLinearSolveOptions::default()
    };
    let equation_scaling = row
        .family_aware
        .then(|| system.equation_scaling.as_ref())
        .flatten();
    let report = solve_with_smoother_and_restriction(
        &system.jacobian,
        &system.rhs,
        &options,
        system.layout,
        row.smoother,
        CprPressureRestrictionKind::QuasiImpes,
        equation_scaling,
    );
    let true_res = residual_norm(&system.jacobian, &report.solution, &system.rhs);
    (report.converged, report.iterations, true_res)
}

/// Phase 10 Step 10.3: tests the OPM-recipe bundle (loosened tolerance + reduced iteration
/// budget + block-ILU0) together against a captured corpus, using `QuasiImpes` throughout
/// (already the live restriction as of Step 9.3) — directly testing whether the Step 9.3
/// wall-clock regression was an artifact of the old, much tighter tolerance philosophy, or
/// an intrinsic property of the restriction choice. Iteration count (not offline wall-clock,
/// which doesn't map 1:1 to live wasm timing) is the comparable metric across rows.
///
/// Manual lab entry point; requires FIM_CAPTURE_DIR pointing at a captured corpus. Run once
/// per corpus (heavy, then bounded) — this test does not itself iterate multiple corpora.
#[test]
#[ignore]
fn solver_lab_compare_bundle_tolerance_iterations() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a directory produced by a capture driver run");
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    println!(
        "bundle lab: {} captured systems from {}",
        systems.len(),
        dir.display()
    );

    let rows: Vec<BundleRow> = vec![
        BundleRow {
            label: "baseline: tol=1e-7 iter=150 ilu0".to_string(),
            relative_tolerance: 1e-7,
            max_iterations: 150,
            smoother: CprFineSmootherKind::FullIlu0,
            family_aware: false,
        },
        BundleRow {
            label: "tol=5e-3 iter=150 ilu0".to_string(),
            relative_tolerance: 5e-3,
            max_iterations: 150,
            smoother: CprFineSmootherKind::FullIlu0,
            family_aware: false,
        },
        BundleRow {
            label: "tol=5e-3 iter=50 ilu0".to_string(),
            relative_tolerance: 5e-3,
            max_iterations: 50,
            smoother: CprFineSmootherKind::FullIlu0,
            family_aware: false,
        },
        BundleRow {
            label: "tol=5e-3 iter=30 ilu0".to_string(),
            relative_tolerance: 5e-3,
            max_iterations: 30,
            smoother: CprFineSmootherKind::FullIlu0,
            family_aware: false,
        },
        BundleRow {
            label: "tol=5e-3 iter=20 ilu0".to_string(),
            relative_tolerance: 5e-3,
            max_iterations: 20,
            smoother: CprFineSmootherKind::FullIlu0,
            family_aware: false,
        },
        BundleRow {
            label: "tol=5e-3 iter=50 block-ilu0".to_string(),
            relative_tolerance: 5e-3,
            max_iterations: 50,
            smoother: CprFineSmootherKind::BlockIlu0,
            family_aware: false,
        },
        BundleRow {
            label: "tol=5e-3 iter=30 block-ilu0".to_string(),
            relative_tolerance: 5e-3,
            max_iterations: 30,
            smoother: CprFineSmootherKind::BlockIlu0,
            family_aware: false,
        },
        BundleRow {
            label: "tol=5e-3 iter=20 block-ilu0".to_string(),
            relative_tolerance: 5e-3,
            max_iterations: 20,
            smoother: CprFineSmootherKind::BlockIlu0,
            family_aware: false,
        },
    ];

    println!("=== per-row aggregate over {} systems ===", systems.len());
    for row in &rows {
        let mut converged_count = 0usize;
        let mut iterations: Vec<usize> = Vec::with_capacity(systems.len());
        let mut relative_residuals: Vec<f64> = Vec::with_capacity(systems.len());

        for system in &systems {
            let rhs_norm = system.rhs.norm().max(f64::EPSILON);
            let (converged, iters, true_res) = run_bundle_row(row, system);
            if converged {
                converged_count += 1;
            }
            iterations.push(iters);
            relative_residuals.push(true_res / rhs_norm);
        }

        iterations.sort_unstable();
        relative_residuals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median_iters = iterations[iterations.len() / 2];
        let mean_iters: f64 = iterations.iter().sum::<usize>() as f64 / iterations.len() as f64;
        let median_rel_res = relative_residuals[relative_residuals.len() / 2];

        println!(
            "{:<36} converged={:>3}/{} median_iters={:>4} mean_iters={:>7.1} median_rel_res={:.3e}",
            row.label,
            converged_count,
            systems.len(),
            median_iters,
            mean_iters,
            median_rel_res,
        );
    }
}

/// Step 10.1 follow-up (`FIM-LINEAR-008` reopened): the live heavy-case regression traced back
/// to `perf@1299` being repeatedly left a small relative-residual multiple over tolerance by the
/// whole-system norm, even though the norm itself was satisfied. This test measures, on the
/// currently-live bundle (`tol=5e-3 iter=20 block-ilu0`), (a) how large that per-family
/// overshoot actually is on this corpus's captured systems (using `EquationScaling` recorded at
/// capture time — requires corpora captured after the capture-format v2 change), and (b) whether
/// requiring every family to clear its own relative-reduction target (`family_aware: true`)
/// still converges at a comparable rate, or costs materially more iterations. A real offline win
/// here is the required gate before wiring `family_ok` on by default in the live path.
///
/// Manual lab entry point; requires FIM_CAPTURE_DIR pointing at a corpus captured with
/// `equation_scaling` present (recapture after the `fim-capture-v2` format change).
#[test]
#[ignore]
fn solver_lab_compare_family_aware_convergence() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a directory produced by a capture driver run");
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    let with_scaling = systems
        .iter()
        .filter(|system| system.equation_scaling.is_some())
        .count();
    println!(
        "family-aware lab: {} captured systems from {} ({} carry equation_scaling)",
        systems.len(),
        dir.display(),
        with_scaling
    );
    assert!(
        with_scaling > 0,
        "no captured system carries equation_scaling — recapture with the fim-capture-v2 format \
         before running this test"
    );

    let base = BundleRow {
        label: "live bundle: tol=5e-3 iter=20 block-ilu0".to_string(),
        relative_tolerance: 5e-3,
        max_iterations: 20,
        smoother: CprFineSmootherKind::BlockIlu0,
        family_aware: false,
    };
    let family_aware = BundleRow {
        label: "live bundle + family-aware convergence".to_string(),
        family_aware: true,
        ..base.clone()
    };

    let mut base_converged = 0usize;
    let mut family_converged = 0usize;
    let mut base_iterations = Vec::with_capacity(systems.len());
    let mut family_iterations = Vec::with_capacity(systems.len());
    let mut overshoots: Vec<(String, &'static str, f64)> = Vec::new();

    for (index, system) in systems.iter().enumerate() {
        let (base_ok, base_iters, base_true_res) = run_bundle_row(&base, system);
        if base_ok {
            base_converged += 1;
        }
        base_iterations.push(base_iters);

        if let Some(scaling) = &system.equation_scaling {
            let options = FimLinearSolveOptions {
                kind: FimLinearSolverKind::FgmresCpr,
                relative_tolerance: base.relative_tolerance,
                absolute_tolerance: 1e-12,
                max_iterations: base.max_iterations,
                ..FimLinearSolveOptions::default()
            };
            let base_report = solve_with_smoother_and_restriction(
                &system.jacobian,
                &system.rhs,
                &options,
                system.layout,
                base.smoother,
                CprPressureRestrictionKind::QuasiImpes,
                None,
            );
            let base_residual =
                residual_vector(&system.jacobian, &base_report.solution, &system.rhs);
            let (worst_label, worst_ratio) = worst_family_overshoot(
                scaling,
                &system.rhs,
                &base_residual,
                options.absolute_tolerance,
                options.relative_tolerance,
            );
            let _ = base_true_res;
            overshoots.push((format!("capture {index:05}"), worst_label, worst_ratio));
        }

        let (family_ok, family_iters, _) = run_bundle_row(&family_aware, system);
        if family_ok {
            family_converged += 1;
        }
        family_iterations.push(family_iters);
    }

    overshoots.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    println!("=== worst per-family overshoot under the current (non-family-aware) bundle ===");
    println!(
        "(ratio > 1.0 means that family is left under-resolved relative to its own target \
         even though the whole-system norm may already be satisfied)"
    );
    for (label, family, ratio) in overshoots.iter().take(15) {
        println!("{label:<16} worst_family={family:<18} overshoot={ratio:.2}x");
    }
    let overshooting = overshoots
        .iter()
        .filter(|(_, _, ratio)| *ratio > 1.0)
        .count();
    println!(
        "{overshooting}/{} systems have at least one family left over its own target",
        overshoots.len()
    );

    base_iterations.sort_unstable();
    family_iterations.sort_unstable();
    let median = |values: &[usize]| values[values.len() / 2];
    let mean = |values: &[usize]| values.iter().sum::<usize>() as f64 / values.len() as f64;

    println!(
        "=== convergence comparison over {} systems ===",
        systems.len()
    );
    println!(
        "{:<36} converged={:>3}/{} median_iters={:>4} mean_iters={:>7.1}",
        base.label,
        base_converged,
        systems.len(),
        median(&base_iterations),
        mean(&base_iterations),
    );
    println!(
        "{:<36} converged={:>3}/{} median_iters={:>4} mean_iters={:>7.1}",
        family_aware.label,
        family_converged,
        systems.len(),
        median(&family_iterations),
        mean(&family_iterations),
    );
}

/// Phase 11 (`FIM-LINEAR-010`): offline correctness + iteration-cost check for well/perforation
/// Schur elimination (`fim/linear/well_schur.rs`) against real captured systems, before any live
/// wiring. Compares the live bundle's default dispatch (`eliminate_wells: false`, current
/// production behavior) against the same options with `eliminate_wells: true` on every captured
/// system: both should converge to closely matching solutions (both solve the same underlying
/// system, just via a different reduction), and the run reports iteration-count deltas.
///
/// Manual lab entry point; requires FIM_CAPTURE_DIR pointing at a corpus captured with
/// `equation_scaling` present (`fim-capture-v2` format).
#[test]
#[ignore]
fn solver_lab_compare_well_elimination() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a directory produced by a capture driver run");
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    println!(
        "well-elimination lab: {} captured systems from {}",
        systems.len(),
        dir.display()
    );

    let mut baseline_converged = 0usize;
    let mut eliminated_converged = 0usize;
    let mut baseline_iterations = Vec::with_capacity(systems.len());
    let mut eliminated_iterations = Vec::with_capacity(systems.len());
    let mut max_solution_delta = 0.0_f64;
    let mut worst_delta_capture = 0usize;

    for (index, system) in systems.iter().enumerate() {
        let Some(layout) = system.layout else {
            continue;
        };
        if layout.well_bhp_count == 0 && layout.perforation_tail_start >= system.jacobian.rows() {
            continue;
        }

        let baseline_options = FimLinearSolveOptions {
            eliminate_wells: false,
            ..FimLinearSolveOptions::default()
        };
        let eliminated_options = FimLinearSolveOptions {
            eliminate_wells: true,
            ..FimLinearSolveOptions::default()
        };

        let baseline_report = solve_linearized_system(
            &system.jacobian,
            &system.rhs,
            &baseline_options,
            system.layout,
            system.equation_scaling.as_ref(),
        );
        let eliminated_report = solve_linearized_system(
            &system.jacobian,
            &system.rhs,
            &eliminated_options,
            system.layout,
            system.equation_scaling.as_ref(),
        );

        if baseline_report.converged {
            baseline_converged += 1;
        }
        if eliminated_report.converged {
            eliminated_converged += 1;
        }
        baseline_iterations.push(baseline_report.iterations);
        eliminated_iterations.push(eliminated_report.iterations);

        let mut delta = 0.0_f64;
        for row in 0..system.jacobian.rows() {
            delta =
                delta.max((baseline_report.solution[row] - eliminated_report.solution[row]).abs());
        }
        if delta > max_solution_delta {
            max_solution_delta = delta;
            worst_delta_capture = index;
        }
    }

    baseline_iterations.sort_unstable();
    eliminated_iterations.sort_unstable();
    let median = |values: &[usize]| {
        if values.is_empty() {
            0
        } else {
            values[values.len() / 2]
        }
    };
    let mean = |values: &[usize]| {
        if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<usize>() as f64 / values.len() as f64
        }
    };

    println!(
        "=== well-elimination comparison over {} systems ===",
        baseline_iterations.len()
    );
    println!(
        "baseline (no elimination):   converged={:>3}/{} median_iters={:>4} mean_iters={:>7.1}",
        baseline_converged,
        baseline_iterations.len(),
        median(&baseline_iterations),
        mean(&baseline_iterations),
    );
    println!(
        "eliminated (well Schur):     converged={:>3}/{} median_iters={:>4} mean_iters={:>7.1}",
        eliminated_converged,
        eliminated_iterations.len(),
        median(&eliminated_iterations),
        mean(&eliminated_iterations),
    );
    println!(
        "max solution delta across all systems: {max_solution_delta:.3e} (capture {worst_delta_capture:05})"
    );
}

/// Bundle P (`FIM-BUNDLE-P`) P0.1: per-phase CPR preconditioner build-cost breakdown over a
/// captured corpus, using the exact production entry point (`solve_linearized_system`, default
/// options — well elimination included, matching `eliminate_wells: true`) so the timings
/// reflect what the live path actually builds against. Decides whether P2 (LU factorization
/// instead of an explicit dense inverse for the coarse pressure operator) matters independently
/// of P1 (setup reuse) — see `docs/FIM_BUNDLE_P_PLAN.md`.
///
/// Manual lab entry point; requires `FIM_CAPTURE_DIR` pointing at a captured corpus.
#[test]
#[ignore]
fn solver_lab_cpr_build_cost_breakdown() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a directory produced by a capture driver run");
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    println!(
        "build-cost breakdown lab: {} captured systems from {}",
        systems.len(),
        dir.display()
    );

    let mut weights_ms = Vec::with_capacity(systems.len());
    let mut coarse_assembly_ms = Vec::with_capacity(systems.len());
    let mut dense_inverse_ms = Vec::with_capacity(systems.len());
    let mut coarse_ilu0_ms = Vec::with_capacity(systems.len());
    let mut fine_smoother_ms = Vec::with_capacity(systems.len());
    let mut block_inverses_ms = Vec::with_capacity(systems.len());
    let mut build_fraction_of_total = Vec::with_capacity(systems.len());
    let mut with_timing = 0usize;

    for system in &systems {
        let options = FimLinearSolveOptions::default();
        let report = solve_linearized_system(
            &system.jacobian,
            &system.rhs,
            &options,
            system.layout,
            system.equation_scaling.as_ref(),
        );
        let Some(timing) = report
            .cpr_diagnostics
            .as_ref()
            .and_then(|diagnostics| diagnostics.build_timing)
        else {
            continue;
        };
        with_timing += 1;
        weights_ms.push(timing.weights_ms);
        coarse_assembly_ms.push(timing.coarse_assembly_ms);
        dense_inverse_ms.push(timing.dense_inverse_ms);
        coarse_ilu0_ms.push(timing.coarse_ilu0_ms);
        fine_smoother_ms.push(timing.fine_smoother_ms);
        block_inverses_ms.push(timing.block_inverses_ms);
        if report.total_time_ms > 0.0 {
            build_fraction_of_total
                .push(report.preconditioner_build_time_ms / report.total_time_ms);
        }
    }

    assert!(
        with_timing > 0,
        "no captured system produced CPR build timing — check that FimLinearSolveOptions::default() \
         actually dispatches to the CPR/FGMRES backend"
    );

    fn median(values: &mut [f64]) -> f64 {
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        values[values.len() / 2]
    }

    println!("=== per-phase build-cost medians over {with_timing} systems (ms) ===");
    println!(
        "weights_ms              median={:.4}",
        median(&mut weights_ms)
    );
    println!(
        "coarse_assembly_ms      median={:.4}",
        median(&mut coarse_assembly_ms)
    );
    println!(
        "dense_inverse_ms        median={:.4}",
        median(&mut dense_inverse_ms)
    );
    println!(
        "coarse_ilu0_ms          median={:.4}",
        median(&mut coarse_ilu0_ms)
    );
    println!(
        "fine_smoother_ms        median={:.4}",
        median(&mut fine_smoother_ms)
    );
    println!(
        "block_inverses_ms       median={:.4}",
        median(&mut block_inverses_ms)
    );
    if !build_fraction_of_total.is_empty() {
        println!(
            "preconditioner_build_time_ms / total_time_ms median={:.3}",
            median(&mut build_fraction_of_total)
        );
    }
}

/// Coarse-factorization cost lever (2026-07-10 follow-up to `FIM-BUNDLE-P`'s P0, which found
/// `dense_inverse_ms` alone — not the ILU0 setup running alongside it — is 97.5% of build cost
/// on the heavy corpus, ~400x the coarse ILU0 factorization on the identical system). Offline
/// 3-way comparison on both captured corpora: today's explicit dense inverse (production
/// baseline when coarse rows are within threshold) vs. an LU factorization of the same coarse
/// operator vs. today's BiCGStab+ILU0 coarse solve (production baseline when coarse rows are
/// over threshold — already used by the bounded corpus, forced here on the heavy corpus too).
/// Correctness (LU must reproduce the inverse's solution) and BiCGStab's residual reduction are
/// both measured, not assumed — a build-time win that changes the answer is not a win.
#[test]
#[ignore]
fn solver_lab_coarse_factorization_comparison() {
    let dir = capture_dir_from_env()
        .expect("set FIM_CAPTURE_DIR to a directory produced by a capture driver run");
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    let mut coarse_rows_seen = 0usize;
    let mut dense_inverse_ms = Vec::new();
    let mut lu_factorization_ms = Vec::new();
    let mut bicgstab_ms = Vec::new();
    let mut lu_diff_norms = Vec::new();
    let mut bicgstab_reductions = Vec::new();
    let mut bicgstab_failures = 0usize;
    let mut dense_inverse_unavailable = 0usize;

    for system in &systems {
        let Some((jacobian, _rhs, layout)) = reduced_system_for_lab(system) else {
            continue;
        };
        let Some(result) = coarse_factorization_lab_compare(&jacobian, Some(layout)) else {
            continue;
        };
        coarse_rows_seen = result.coarse_rows;
        dense_inverse_ms.push(result.dense_inverse_ms);
        lu_factorization_ms.push(result.lu_factorization_ms);
        bicgstab_ms.push(result.bicgstab_ms);
        match result.lu_vs_inverse_solution_diff_norm {
            Some(diff) => lu_diff_norms.push(diff),
            None => dense_inverse_unavailable += 1,
        }
        bicgstab_reductions.push(result.bicgstab_reduction_ratio);
        if result.bicgstab_reduction_ratio > 1e-4 {
            bicgstab_failures += 1;
        }
    }

    assert!(
        !dense_inverse_ms.is_empty(),
        "no captured system produced a coarse-factorization comparison"
    );

    fn median(values: &mut [f64]) -> f64 {
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        values[values.len() / 2]
    }
    fn max_of(values: &[f64]) -> f64 {
        values.iter().cloned().fold(0.0, f64::max)
    }

    println!(
        "=== coarse-factorization comparison: {} systems, coarse_rows={} ===",
        dense_inverse_ms.len(),
        coarse_rows_seen
    );
    println!(
        "dense_inverse_ms      median={:.4} (unavailable on {} systems, over threshold)",
        median(&mut dense_inverse_ms),
        dense_inverse_unavailable
    );
    println!(
        "lu_factorization_ms   median={:.4}",
        median(&mut lu_factorization_ms)
    );
    println!(
        "bicgstab_ms           median={:.4}",
        median(&mut bicgstab_ms)
    );
    println!(
        "lu_vs_inverse solution diff norm: max={:.3e} (n={})",
        max_of(&lu_diff_norms),
        lu_diff_norms.len()
    );
    println!(
        "bicgstab reduction ratio: median={:.3e} max={:.3e}, failures (ratio>1e-4)={}/{}",
        median(&mut bicgstab_reductions),
        max_of(&bicgstab_reductions),
        bicgstab_failures,
        bicgstab_reductions.len()
    );
}

/// Reduces a captured `(jacobian, rhs, layout)` triple by Schur-eliminating the well/perforation
/// tail (mirrors `eliminate_wells: true`, the production default) so the offline reuse study
/// below builds/reuses a CPR preconditioner against the exact same reduced system the live path
/// actually solves against. Falls through unchanged when there is no tail to eliminate. Returns
/// `None` only when the captured system has no layout at all.
fn reduced_system_for_lab(
    system: &FimCapturedSystem,
) -> Option<(CsMat<f64>, DVector<f64>, FimLinearBlockLayout)> {
    let layout = system.layout?;
    match well_schur::eliminate_wells(
        &system.jacobian,
        &system.rhs,
        layout,
        system.equation_scaling.as_ref(),
    ) {
        Some(elimination) => Some((
            elimination.reduced_jacobian,
            elimination.reduced_rhs,
            elimination.reduced_layout,
        )),
        None => Some((system.jacobian.clone(), system.rhs.clone(), layout)),
    }
}

/// Bundle P (`FIM-BUNDLE-P`) P0.2: offline CPR-setup-reuse staleness study. For every pair of
/// consecutive-in-file-order captured systems `(i, i+k)` (`k` up to OPM's own reuse interval,
/// 30) whose *reduced* (well-Schur-eliminated) systems share the same row count and block
/// layout, builds the preconditioner once on system `i` and reuses it — stale — to solve system
/// `i+k`, comparing iteration count and convergence against a fresh-preconditioner baseline
/// solved directly on `i+k`. Pairs whose reduced systems don't share a key are skipped (the same
/// rebuild-on-key-mismatch rule the live cache will use — no reuse would happen there anyway).
///
/// Offline gate (`docs/FIM_BUNDLE_P_PLAN.md` P0.2): median inflation <= +2 iterations and no new
/// convergence failures (fresh converged, reused did not) at k <= 30. Asserted, not just printed
/// — a failure here means live wiring is not attempted and `FIM-BUNDLE-P` closes REFUTED.
///
/// Manual lab entry point; requires `FIM_CAPTURE_SEQUENCE_DIR` (not `FIM_CAPTURE_DIR` — that
/// corpus is failures-only and not a consecutive sequence) pointing at a corpus captured by
/// setting that env var on a capture-driver run.
#[test]
#[ignore]
fn solver_lab_cpr_reuse_inflation_study() {
    let dir = super::capture::capture_sequence_dir_from_env().expect(
        "set FIM_CAPTURE_SEQUENCE_DIR to a directory produced by a capture driver run with \
         FIM_CAPTURE_SEQUENCE_DIR set (not FIM_CAPTURE_DIR — that corpus is failures-only and \
         not a consecutive sequence)",
    );
    let systems = load_captures(&dir).expect("load captured systems");
    assert!(
        !systems.is_empty(),
        "no fim_capture_*.txt files in {}",
        dir.display()
    );

    println!(
        "reuse-inflation lab: {} captured systems from {}",
        systems.len(),
        dir.display()
    );

    let reduced: Vec<Option<(CsMat<f64>, DVector<f64>, FimLinearBlockLayout)>> =
        systems.iter().map(reduced_system_for_lab).collect();

    const MAX_K: usize = 30;
    let options = FimLinearSolveOptions::default();
    let fine_smoother_kind = CprFineSmootherKind::BlockIlu0;
    let restriction_kind = CprPressureRestrictionKind::QuasiImpes;

    // Each target system's fresh-preconditioner baseline doesn't depend on which origin `i`
    // reused a stale preconditioner into it — compute it once per system instead of once per
    // (i, k) pair (a given target is the `i+k` for up to `MAX_K` different origins).
    let fresh_reports: Vec<Option<(usize, bool)>> = reduced
        .iter()
        .map(|entry| {
            let (jacobian, rhs, layout) = entry.as_ref()?;
            let report = solve_linearized_system(jacobian, rhs, &options, Some(*layout), None);
            Some((report.iterations, report.converged))
        })
        .collect();

    let mut inflations: Vec<i64> = Vec::new();
    let mut new_failures = 0usize;
    let mut pairs_evaluated = 0usize;
    let mut pairs_skipped_key_mismatch = 0usize;
    // Bundle P P0.2 follow-up: bucket by staleness distance `k` so a gate failure can
    // distinguish "breaks at any k" (REFUTED) from "only breaks past some smaller k" (adjust
    // the target reuse interval instead of abandoning the bundle).
    let mut by_k_total = vec![0usize; MAX_K + 1];
    let mut by_k_failures = vec![0usize; MAX_K + 1];

    for i in 0..reduced.len() {
        let Some((build_jacobian, _build_rhs, build_layout)) = &reduced[i] else {
            continue;
        };
        let max_k = MAX_K.min(reduced.len().saturating_sub(i + 1));
        if max_k == 0 {
            continue;
        }
        // Build once per origin system `i`, reuse across every `k` — rebuilding per (i, k) pair
        // would redo the O(n^3) coarse factorization up to `MAX_K`x more than necessary.
        let handle = gmres_block_jacobi::build_preconditioner_for_lab(
            build_jacobian,
            Some(*build_layout),
            fine_smoother_kind,
            restriction_kind,
        );
        for k in 1..=max_k {
            let Some((solve_jacobian, solve_rhs, solve_layout)) = &reduced[i + k] else {
                continue;
            };
            if solve_jacobian.rows() != build_jacobian.rows() || solve_layout != build_layout {
                pairs_skipped_key_mismatch += 1;
                continue;
            }

            let (fresh_iterations, fresh_converged) = fresh_reports[i + k]
                .expect("reduced[i + k] is Some, so fresh_reports[i + k] is too");
            let reused = gmres_block_jacobi::solve_with_prebuilt_preconditioner(
                solve_jacobian,
                solve_rhs,
                &options,
                &handle,
                None,
            );

            pairs_evaluated += 1;
            inflations.push(reused.iterations as i64 - fresh_iterations as i64);
            by_k_total[k] += 1;
            if fresh_converged && !reused.converged {
                new_failures += 1;
                by_k_failures[k] += 1;
            }
        }
    }

    println!("=== new-convergence-failures by staleness distance k ===");
    for k in 1..=MAX_K {
        if by_k_total[k] > 0 {
            println!(
                "k={k:>2} failures={:>4}/{:<4} ({:.1}%)",
                by_k_failures[k],
                by_k_total[k],
                100.0 * by_k_failures[k] as f64 / by_k_total[k] as f64
            );
        }
    }

    assert!(
        pairs_evaluated > 0,
        "no matching-key consecutive pairs found in {} systems ({} skipped for key mismatch) — \
         corpus too small or too fragmented to run the staleness study",
        reduced.len(),
        pairs_skipped_key_mismatch
    );

    inflations.sort_unstable();
    let median_inflation = inflations[inflations.len() / 2];
    let mean_inflation: f64 = inflations.iter().sum::<i64>() as f64 / inflations.len() as f64;

    println!(
        "=== reuse-inflation over {pairs_evaluated} matching-key pairs ({pairs_skipped_key_mismatch} skipped for key mismatch) ==="
    );
    println!(
        "median_inflation_iters={median_inflation} mean_inflation_iters={mean_inflation:.2} new_convergence_failures={new_failures}"
    );

    assert!(
        median_inflation <= 2,
        "median reuse inflation {median_inflation} exceeds the offline gate of +2 iterations \
         (FIM_BUNDLE_P_PLAN.md P0.2) — do not attempt live wiring, close FIM-BUNDLE-P REFUTED"
    );
    assert_eq!(
        new_failures, 0,
        "{new_failures} pairs converged fresh but failed to converge when reusing a stale \
         preconditioner — offline gate (P0.2) failed, do not attempt live wiring"
    );
}
