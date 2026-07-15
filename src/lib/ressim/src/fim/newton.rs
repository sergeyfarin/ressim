use nalgebra::DVector;

use crate::ReservoirSimulator;
use crate::fim::assembly::{
    CellFacePhaseDiagnostics, FacePhaseDiagnostics, FaceUpwindSample, FimAssemblyOptions,
    PhaseFluxDiagnostic, cell_equation_residual_breakdown, cell_face_phase_flux_diagnostics,
    collect_face_upwind_snapshot, diff_face_upwind_snapshots, equation_offset, unknown_offset,
};
// Phase 5 cutover: production Newton now assembles the coupled residual/
// Jacobian via automatic differentiation (`assembly_ad`) instead of the
// legacy hand-derivative/finite-difference hybrid in `assembly`. Aliased to
// the old name so every production call site below is unchanged; the
// `#[cfg(test)]` module still imports the legacy `assemble_fim_system`
// directly for its own assertions.
use crate::fim::assembly_ad::assemble_fim_system_ad as assemble_fim_system;
use crate::fim::linear::{
    FimLinearBlockLayout, FimLinearFailureReason, FimLinearSolveOptions, FimLinearSolveReport,
    FimLinearSolverKind, active_direct_solve_row_threshold, solve_linearized_system,
};
use crate::fim::state::{FimState, HydrocarbonState};
use crate::fim::wells::{build_well_topology, perforation_local_block, physical_well_control};
use crate::timing::PerfTimer;

/// Diagnostic trace macro — persists lines for wasm diagnostics and optionally prints on native.
macro_rules! fim_trace {
    ($sim:expr, $verbose:expr, $($arg:tt)*) => {{
        let line = format!($($arg)*);
        $sim.append_fim_trace_line(&line);
        #[cfg(not(target_arch = "wasm32"))]
        if $verbose {
            eprintln!("{}", line);
        }
    }};
}

const ENTRY_RESIDUAL_GUARD_FACTOR: f64 = 2.0;
const NOOP_ENTRY_EXACT_FACTOR: f64 = 1e-3;
const STRONG_CPR_AVERAGE_REDUCTION_RATIO: f64 = 0.25;
const STRONG_CPR_LAST_REDUCTION_RATIO: f64 = 0.5;
const DEFAULT_MAX_NEWTON_PRESSURE_CHANGE_BAR: f64 = 200.0;
const DEFAULT_MAX_NEWTON_SATURATION_CHANGE: f64 = 0.1;
/// Inflection-chop overshoot factor: only chop the Newton update at the fw
/// inflection point if the proposed step would land at least this multiple
/// of `dist_to_inflection` past the inflection. Setting to 1.0 reproduces
/// the classic Wang-Tchelepi 2013 trust-region chop (any crossing fires).
/// Larger values skip "marginal" crossings while still guarding deep
/// basin-jumping overshoots.
///
/// Re-swept 2026-07-05 (`FIM-DAMP-004`) under the Phase 10/11 linear-solver bundle (loosened
/// tolerance, block-ILU0, well elimination), per `FIM-DAMP-003`'s own retry condition ("retune
/// only with k-sweep and fine-dt reference"). The relationship between `k` and heavy-case
/// substep count is now genuinely non-monotonic/chaotic (a Newton-trajectory bifurcation
/// artifact, not a smooth trend): `k=1.0`→248, `k=1.1`→32, `k=1.15`→214, `k=1.2`(the April
/// sweet spot, now stale)→62, `k=1.25`→32, `k=1.3`→32, `k=1.5`→204, `k=2.0`→134 substeps.
/// `k∈[1.25,1.3]` is a genuine stable plateau (identical trajectories, not a lucky isolated
/// point like `1.1` or the bad spot at `1.15`) — chose `1.25`, the middle of that demonstrated
/// range. Full control matrix bit-identical, locked smoke 3/3; no new failure mode introduced
/// (same benign local-Sw-plateau retry pattern as before, just less frequent, at a different
/// cell). See `docs/FIM_CONVERGENCE_WORKLOG.md` "Task #38" for the full sweep table.
const FW_INFLECTION_OVERSHOOT_FACTOR: f64 = 1.25;
const EFFECTIVE_TRACE_PRESSURE_MOVE_THRESHOLD_BAR: f64 = 5e-3;
const EFFECTIVE_TRACE_SATURATION_MOVE_THRESHOLD: f64 = 5e-5;
const NONLINEAR_HISTORY_WEAK_PROGRESS_RATIO: f64 = 0.98;
const NONLINEAR_HISTORY_GAS_WEAK_PROGRESS_RATIO: f64 = 0.90;
const NONLINEAR_HISTORY_MIN_STREAK: u32 = 1;
const NONLINEAR_HISTORY_FIRST_DAMPING_CAP: f64 = 0.5;
const NONLINEAR_HISTORY_REPEAT_DAMPING_CAP: f64 = 0.25;
const NONLINEAR_HISTORY_RESIDUAL_BAND_FACTOR: f64 = 10.0;
const RESTART_STAGNATION_DIRECT_BYPASS_THRESHOLD: u32 = 2;
const NEAR_CONVERGED_ITERATIVE_OUTER_FACTOR: f64 = 16.0;
const NEAR_CONVERGED_ITERATIVE_CANDIDATE_WORSENING_FACTOR: f64 = 8.0;
/// Historical Y2b2 first-rung retry where raw saturation retention made the live path advance.
/// The diagnostic capture is intentionally exact and does not generalize behavior.
#[cfg(not(target_arch = "wasm32"))]
const Y2B2_CAPTURE_DT_DAYS: f64 = 0.008_984_25;

#[cfg(not(target_arch = "wasm32"))]
fn y2b2_state_checksum(state: &FimState) -> u64 {
    // Stable, dependency-free FNV-1a over the stored state and well unknowns. This identifies
    // the nonlinear decision state in the trace; the artifact itself is checksummed by the run.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut add = |bits: u64| {
        hash ^= bits;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for cell in &state.cells {
        add(cell.pressure_bar.to_bits());
        add(cell.sw.to_bits());
        add(cell.hydrocarbon_var.to_bits());
        add(match cell.regime {
            HydrocarbonState::Saturated => 0,
            HydrocarbonState::Undersaturated => 1,
        });
    }
    for value in &state.well_bhp {
        add(value.to_bits());
    }
    for value in &state.perforation_rates_m3_day {
        add(value.to_bits());
    }
    hash
}

/// Y2b3c's exact-capture companion trace. Each fixed local-variable-2 column is tied to the
/// meaning/value consumed by this assembly, the immediately preceding switch decision, and its
/// actual structural occupancy. This is diagnostic-only and emits nothing unless the exact
/// Y2b capture is requested.
#[cfg(not(target_arch = "wasm32"))]
fn trace_y2b3_primary_variable_state(
    sim: &mut ReservoirSimulator,
    verbose: bool,
    state: &FimState,
    switched_on_previous_update: &[bool],
    jacobian: &sprs::CsMat<f64>,
) {
    let mut column_nnz = vec![0usize; jacobian.cols()];
    for (value, (_row, column)) in jacobian {
        if *value != 0.0 {
            column_nnz[column] += 1;
        }
    }

    for (cell_idx, cell) in state.cells.iter().enumerate() {
        let derived = state.derive_cell(sim, cell_idx);
        let rs_sat = sim
            .pvt_table
            .as_ref()
            .map(|table| table.interpolate(cell.pressure_bar).rs_m3m3)
            .unwrap_or(0.0);
        let previous_switched = switched_on_previous_update[cell_idx];
        let epsilon = if previous_switched { 1e-5 } else { 0.0 };
        let primary_column = 3 * cell_idx + 2;
        fim_trace!(
            sim,
            verbose,
            "Y2B3-PRIMARY cell={} meaning={} raw_z={:.16e} sw={:.16e} derived_sg={:.16e} derived_rs={:.16e} rs_sat={:.16e} previous_update_switched={} switch_epsilon={:.16e} primary_column={} column_nnz={}",
            cell_idx,
            match cell.regime {
                HydrocarbonState::Saturated => "Sg",
                HydrocarbonState::Undersaturated => "Rs",
            },
            cell.hydrocarbon_var,
            cell.sw,
            derived.sg,
            derived.rs,
            rs_sat,
            previous_switched,
            epsilon,
            primary_column,
            column_nnz[primary_column],
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FimRetryFailureClass {
    LinearBad,
    NonlinearBad,
    Mixed,
}

impl FimRetryFailureClass {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::LinearBad => "linear-bad",
            Self::NonlinearBad => "nonlinear-bad",
            Self::Mixed => "mixed",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FimRetryFailureDiagnostics {
    pub(crate) class: FimRetryFailureClass,
    pub(crate) dominant_family_label: &'static str,
    pub(crate) dominant_row: usize,
    pub(crate) dominant_item_index: usize,
    pub(crate) hotspot_site: FimHotspotSite,
    pub(crate) linear_iterations: Option<usize>,
    pub(crate) used_linear_fallback: bool,
    pub(crate) cpr_average_reduction_ratio: Option<f64>,
    pub(crate) cpr_last_reduction_ratio: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FimHotspotSite {
    Cell(usize),
    GasInjectorSymmetry {
        injector_well_index: usize,
        major_offset: usize,
        minor_offset: usize,
        vertical_offset: usize,
    },
    Well(usize),
    Perforation(usize),
}

impl FimHotspotSite {
    pub(crate) fn trace_label(self) -> String {
        match self {
            Self::Cell(cell_idx) => format!("cell{}", cell_idx),
            Self::GasInjectorSymmetry {
                injector_well_index,
                major_offset,
                minor_offset,
                vertical_offset,
            } => format!(
                "gasinj{}[{},{},{}]",
                injector_well_index, major_offset, minor_offset, vertical_offset
            ),
            Self::Well(well_idx) => format!("well{}", well_idx),
            Self::Perforation(perf_idx) => format!("perf{}", perf_idx),
        }
    }
}

fn linear_report_trace_suffix(
    report: &FimLinearSolveReport,
    requested_kind: FimLinearSolverKind,
) -> String {
    let Some(cpr) = &report.cpr_diagnostics else {
        return format!(
            " lin=[req={} used={} rows={} direct_thr={}]",
            requested_kind.label(),
            report.backend_used.label(),
            report.solution.len(),
            active_direct_solve_row_threshold(),
        );
    };

    let solver = match cpr.coarse_solver {
        crate::fim::linear::FimPressureCoarseSolverKind::ExactDense => "dense",
        crate::fim::linear::FimPressureCoarseSolverKind::BiCgStab => "bicgstab",
    };

    format!(
        " lin=[req={} used={} rows={} direct_thr={}] cpr=[rows={} solver={} smoother={} apps={} avg_rr={:.3e} last_rr={:.3e}]",
        requested_kind.label(),
        report.backend_used.label(),
        report.solution.len(),
        active_direct_solve_row_threshold(),
        cpr.coarse_rows,
        solver,
        cpr.smoother_label,
        cpr.coarse_applications,
        cpr.average_reduction_ratio,
        cpr.last_reduction_ratio,
    )
}

fn linear_failure_trace_suffix(report: &FimLinearSolveReport) -> String {
    let Some(failure) = &report.failure_diagnostics else {
        return String::new();
    };

    let mut parts = vec![format!(
        " fail=[backend={} iters={} final_res={:.3e} tol={:.3e} reason={}",
        report.backend_used.label(),
        report.iterations,
        report.final_residual_norm,
        failure.tolerance,
        failure.reason.label(),
    )];
    parts.push(format!(" outer_res={:.3e}", failure.outer_residual_norm));
    if let Some(norm) = failure.preconditioned_residual_norm {
        parts.push(format!(" prec_res={:.3e}", norm));
    }
    if let Some(norm) = failure.estimated_residual_norm {
        parts.push(format!(" est_res={:.3e}", norm));
    }
    if let Some(norm) = failure.candidate_residual_norm {
        parts.push(format!(" cand_res={:.3e}", norm));
    }
    if !failure.restart_diagnostics.is_empty() {
        let restart_trace = failure
            .restart_diagnostics
            .iter()
            .map(|restart| {
                let est = restart
                    .best_estimated_residual_norm
                    .map(|norm| format!("{norm:.3e}"))
                    .unwrap_or_else(|| "-".to_string());
                let cand = restart
                    .best_candidate_residual_norm
                    .map(|norm| format!("{norm:.3e}"))
                    .unwrap_or_else(|| "-".to_string());
                format!(
                    "r{}:{}-{} steps={} out={:.3e} prec={:.3e} est={} cand={} upd={}",
                    restart.restart_index,
                    restart.start_iteration,
                    restart.end_iteration,
                    restart.inner_steps,
                    restart.outer_residual_norm,
                    restart.preconditioned_residual_norm,
                    est,
                    cand,
                    if restart.solution_improved { "y" } else { "n" },
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        parts.push(format!(" restarts=[{}]", restart_trace));
    }
    parts.push("]".to_string());
    parts.join("")
}

fn direct_fallback_kind_for_rows(row_count: usize) -> FimLinearSolverKind {
    #[cfg(target_arch = "wasm32")]
    {
        if row_count > active_direct_solve_row_threshold() {
            FimLinearSolverKind::SparseLuDebug
        } else {
            FimLinearSolverKind::DenseLuDebug
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = row_count;
        FimLinearSolverKind::SparseLuDebug
    }
}

fn next_restart_stagnation_fallback_streak(
    current_streak: u32,
    failure_reason: Option<FimLinearFailureReason>,
) -> u32 {
    match failure_reason {
        Some(FimLinearFailureReason::RestartStagnation) => current_streak.saturating_add(1),
        _ => 0,
    }
}

fn should_enable_restart_stagnation_direct_bypass(streak: u32) -> bool {
    streak >= RESTART_STAGNATION_DIRECT_BYPASS_THRESHOLD
}

fn should_enable_zero_move_fallback_direct_bypass(
    used_fallback: bool,
    pressure_change_bar: f64,
    saturation_change: f64,
) -> bool {
    used_fallback
        && pressure_change_bar < EFFECTIVE_TRACE_PRESSURE_MOVE_THRESHOLD_BAR
        && saturation_change < EFFECTIVE_TRACE_SATURATION_MOVE_THRESHOLD
}

fn hotspot_sites_share_history_region(
    sim: &ReservoirSimulator,
    family: ResidualRowFamily,
    previous_site: FimHotspotSite,
    current_site: FimHotspotSite,
) -> bool {
    match family {
        ResidualRowFamily::GasComponent => previous_site == current_site,
        _ => non_gas_hotspot_sites_share_local_region(sim, previous_site, current_site),
    }
}

fn should_enable_repeated_zero_move_direct_bypass(
    sim: &ReservoirSimulator,
    previous_effective_move_site: Option<FimHotspotSite>,
    current_diagnostics: &ResidualFamilyDiagnostics,
    current_site: FimHotspotSite,
) -> bool {
    let Some(previous_site) = previous_effective_move_site else {
        return false;
    };

    hotspot_sites_share_history_region(
        sim,
        current_diagnostics.global.family,
        previous_site,
        current_site,
    )
}

fn should_accept_near_converged_iterative_step(report: &FimLinearSolveReport) -> bool {
    if report.backend_used != FimLinearSolverKind::FgmresCpr {
        return false;
    }

    let Some(failure) = &report.failure_diagnostics else {
        return false;
    };

    if !matches!(
        failure.reason,
        FimLinearFailureReason::RestartStagnation | FimLinearFailureReason::MaxIterations
    ) {
        return false;
    }

    if failure.outer_residual_norm > failure.tolerance * NEAR_CONVERGED_ITERATIVE_OUTER_FACTOR {
        return false;
    }

    let Some(candidate_residual_norm) = failure.candidate_residual_norm else {
        return false;
    };

    if candidate_residual_norm
        > failure.outer_residual_norm * NEAR_CONVERGED_ITERATIVE_CANDIDATE_WORSENING_FACTOR
    {
        return false;
    }

    failure
        .restart_diagnostics
        .iter()
        .any(|restart| restart.solution_improved)
}

#[cfg(test)]
fn classify_retry_failure(
    linear_report: Option<&FimLinearSolveReport>,
    residual_diagnostics: &ResidualFamilyDiagnostics,
) -> FimRetryFailureDiagnostics {
    classify_retry_failure_with_site(
        linear_report,
        residual_diagnostics,
        exact_residual_hotspot_site(&residual_diagnostics.global),
    )
}

fn classify_retry_failure_with_site(
    linear_report: Option<&FimLinearSolveReport>,
    residual_diagnostics: &ResidualFamilyDiagnostics,
    hotspot_site: FimHotspotSite,
) -> FimRetryFailureDiagnostics {
    let used_linear_fallback = linear_report.is_some_and(|report| report.used_fallback);
    let cpr_average_reduction_ratio = linear_report
        .and_then(|report| report.cpr_diagnostics.as_ref())
        .map(|diagnostics| diagnostics.average_reduction_ratio);
    let cpr_last_reduction_ratio = linear_report
        .and_then(|report| report.cpr_diagnostics.as_ref())
        .map(|diagnostics| diagnostics.last_reduction_ratio);
    let class = if let Some(report) = linear_report {
        if !report.converged {
            FimRetryFailureClass::LinearBad
        } else {
            match report.backend_used {
                FimLinearSolverKind::DenseLuDebug | FimLinearSolverKind::SparseLuDebug => {
                    FimRetryFailureClass::NonlinearBad
                }
                FimLinearSolverKind::FgmresCpr => {
                    let cpr_is_strong = report
                        .cpr_diagnostics
                        .as_ref()
                        .map(|diagnostics| {
                            diagnostics.coarse_applications > 0
                                && diagnostics.average_reduction_ratio
                                    <= STRONG_CPR_AVERAGE_REDUCTION_RATIO
                                && diagnostics.last_reduction_ratio
                                    <= STRONG_CPR_LAST_REDUCTION_RATIO
                        })
                        .unwrap_or(false);
                    if cpr_is_strong {
                        FimRetryFailureClass::NonlinearBad
                    } else {
                        FimRetryFailureClass::Mixed
                    }
                }
                FimLinearSolverKind::GmresIlu0 => FimRetryFailureClass::Mixed,
            }
        }
    } else {
        FimRetryFailureClass::Mixed
    };

    FimRetryFailureDiagnostics {
        class,
        dominant_family_label: residual_diagnostics.global.family.label(),
        dominant_row: residual_diagnostics.global.row,
        dominant_item_index: residual_diagnostics.global.item_index,
        hotspot_site,
        linear_iterations: linear_report.map(|report| report.iterations),
        used_linear_fallback,
        cpr_average_reduction_ratio,
        cpr_last_reduction_ratio,
    }
}

/// Y2a's deliberately narrow, test-only derivative audit for the gas injector.
///
/// The production assembly is AD-based while the independent hand-derivative
/// assembly is available only to test builds.  At the first three-count
/// stagnation point, compare the injector perforation row and its connected
/// cell component rows across those two paths and central differences of the
/// legacy residual.  This is trace-only: it neither changes the iterate nor
/// participates in convergence decisions.
#[cfg(test)]
fn trace_y2a_injector_jacobian_audit(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    topology: &crate::fim::wells::FimWellTopology,
    dt_days: f64,
    iteration: usize,
    stagnation_count: u32,
    ad_assembly: &crate::fim::assembly::FimAssembly,
) {
    if std::env::var_os("FIM_Y2A_AUDIT").is_none() {
        return;
    }
    let Some((perf_idx, perforation)) = topology
        .perforations
        .iter()
        .enumerate()
        .find(|(_, perforation)| perforation.injector)
    else {
        return;
    };

    let options = FimAssemblyOptions {
        dt_days,
        include_wells: true,
        assemble_residual_only: false,
        topology: Some(topology),
    };
    let legacy = crate::fim::assembly::assemble_fim_system(sim, previous_state, state, &options);
    let cell_idx = perforation.cell_index;
    let perf_row = state.perforation_equation_offset(perf_idx);
    let bhp_col = state.well_bhp_unknown_offset(perforation.physical_well_index);
    let q_col = state.perforation_rate_unknown_offset(perf_idx);
    let rows = [
        ("rate_consistency", perf_row),
        ("water", equation_offset(cell_idx, 0)),
        ("oil", equation_offset(cell_idx, 1)),
        ("gas", equation_offset(cell_idx, 2)),
    ];
    let columns = [
        ("p", unknown_offset(cell_idx, 0)),
        ("sw", unknown_offset(cell_idx, 1)),
        ("hc", unknown_offset(cell_idx, 2)),
        ("bhp", bhp_col),
        ("q", q_col),
    ];

    crate::fim::trace_sink::write_line(&format!(
        "Y2A header dt_days={dt_days:.12e} iter={iteration} stagnation_count={stagnation_count} perf={perf_idx} cell={cell_idx} well={} regime={:?}",
        perforation.physical_well_index, state.cells[cell_idx].regime,
    ));

    let mut max_ad_legacy_residual_abs = 0.0_f64;
    let mut max_ad_legacy_jacobian_abs = 0.0_f64;
    let mut max_legacy_fd_abs = 0.0_f64;
    let mut max_legacy_fd_rel = 0.0_f64;
    for (row_label, row) in rows {
        let ad_residual = ad_assembly.residual[row];
        let legacy_residual = legacy.residual[row];
        let residual_abs = (ad_residual - legacy_residual).abs();
        max_ad_legacy_residual_abs = max_ad_legacy_residual_abs.max(residual_abs);
        crate::fim::trace_sink::write_line(&format!(
            "Y2A residual row={row_label} ad={ad_residual:.12e} legacy={legacy_residual:.12e} abs={residual_abs:.3e}"
        ));

        for (column_label, column) in columns {
            let h = y2a_finite_difference_step(state, column);
            let mut plus = state.clone();
            let mut minus = state.clone();
            y2a_perturb_unknown(&mut plus, column, h);
            y2a_perturb_unknown(&mut minus, column, -h);
            let plus_residual =
                crate::fim::assembly::assemble_fim_system(sim, previous_state, &plus, &options)
                    .residual[row];
            let minus_residual =
                crate::fim::assembly::assemble_fim_system(sim, previous_state, &minus, &options)
                    .residual[row];
            let finite_difference = (plus_residual - minus_residual) / (2.0 * h);
            let forward_difference = (plus_residual - legacy.residual[row]) / h;
            let backward_difference = (legacy.residual[row] - minus_residual) / h;
            let ad = ad_assembly
                .jacobian
                .get(row, column)
                .copied()
                .unwrap_or(0.0);
            let legacy_derivative = legacy.jacobian.get(row, column).copied().unwrap_or(0.0);
            let ad_legacy_abs = (ad - legacy_derivative).abs();
            let legacy_fd_abs = (legacy_derivative - finite_difference).abs();
            let legacy_fd_rel = legacy_fd_abs
                / legacy_derivative
                    .abs()
                    .max(finite_difference.abs())
                    .max(1e-12);
            max_ad_legacy_jacobian_abs = max_ad_legacy_jacobian_abs.max(ad_legacy_abs);
            max_legacy_fd_abs = max_legacy_fd_abs.max(legacy_fd_abs);
            max_legacy_fd_rel = max_legacy_fd_rel.max(legacy_fd_rel);
            crate::fim::trace_sink::write_line(&format!(
                "Y2A derivative row={row_label} col={column_label} h={h:.3e} ad={ad:.12e} legacy={legacy_derivative:.12e} fd={finite_difference:.12e} fwd={forward_difference:.12e} back={backward_difference:.12e} ad_legacy_abs={ad_legacy_abs:.3e} legacy_fd_abs={legacy_fd_abs:.3e} legacy_fd_rel={legacy_fd_rel:.3e}"
            ));
        }
    }
    crate::fim::trace_sink::write_line(&format!(
        "Y2A summary dt_days={dt_days:.12e} iter={iteration} max_ad_legacy_residual_abs={max_ad_legacy_residual_abs:.3e} max_ad_legacy_jacobian_abs={max_ad_legacy_jacobian_abs:.3e} max_legacy_fd_abs={max_legacy_fd_abs:.3e} max_legacy_fd_rel={max_legacy_fd_rel:.3e}"
    ));
}

/// Y2b's test-only raw-candidate versus projected-candidate trace.
///
/// OPM keeps its raw primary-variable update for the following assembly unless
/// `--project-saturations` is selected.  ResSim normally hard-bounds that
/// state before its following assembly.  At the first sustained injector
/// stagnation point, record both possible residuals against the linear
/// prediction from the actually assembled AD Jacobian.  This is forensic
/// instrumentation only: it neither selects nor mutates a candidate.
#[cfg(test)]
fn trace_y2b_bound_projection_audit(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    topology: &crate::fim::wells::FimWellTopology,
    dt_days: f64,
    iteration: usize,
    stagnation_count: u32,
    assembly: &crate::fim::assembly::FimAssembly,
    update: &DVector<f64>,
    damping: f64,
    bounded_candidate: &FimState,
) {
    if std::env::var_os("FIM_Y2B_AUDIT").is_none() {
        return;
    }
    let Some((perf_idx, perforation)) = topology
        .perforations
        .iter()
        .enumerate()
        .find(|(_, perforation)| perforation.injector)
    else {
        return;
    };

    let raw_candidate = state.apply_unbounded_update_for_audit(update, damping);
    let options = FimAssemblyOptions {
        dt_days,
        include_wells: true,
        assemble_residual_only: true,
        topology: Some(topology),
    };
    let raw_assembly = crate::fim::assembly_ad::assemble_fim_system_ad(
        sim,
        previous_state,
        &raw_candidate,
        &options,
    );
    let bounded_assembly = crate::fim::assembly_ad::assemble_fim_system_ad(
        sim,
        previous_state,
        bounded_candidate,
        &options,
    );
    let cell_idx = perforation.cell_index;
    let before = &state.cells[cell_idx];
    let raw = &raw_candidate.cells[cell_idx];
    let bounded = &bounded_candidate.cells[cell_idx];
    let rows = [
        (
            "rate_consistency",
            state.perforation_equation_offset(perf_idx),
        ),
        ("water", equation_offset(cell_idx, 0)),
        ("oil", equation_offset(cell_idx, 1)),
        ("gas", equation_offset(cell_idx, 2)),
    ];

    crate::fim::trace_sink::write_line(&format!(
        "Y2B header dt_days={dt_days:.12e} iter={iteration} stagnation_count={stagnation_count} perf={perf_idx} cell={cell_idx} well={} regime={:?} damping={damping:.12e}",
        perforation.physical_well_index, before.regime,
    ));
    crate::fim::trace_sink::write_line(&format!(
        "Y2B candidate cell={cell_idx} p_before={:.12e} sw_before={:.12e} hc_before={:.12e} p_raw={:.12e} sw_raw={:.12e} hc_raw={:.12e} p_bounded={:.12e} sw_bounded={:.12e} hc_bounded={:.12e} dp_projected={:.12e} dsw_projected={:.12e} dhc_projected={:.12e}",
        before.pressure_bar,
        before.sw,
        before.hydrocarbon_var,
        raw.pressure_bar,
        raw.sw,
        raw.hydrocarbon_var,
        bounded.pressure_bar,
        bounded.sw,
        bounded.hydrocarbon_var,
        bounded.pressure_bar - raw.pressure_bar,
        bounded.sw - raw.sw,
        bounded.hydrocarbon_var - raw.hydrocarbon_var,
    ));

    for (label, row) in rows {
        let predicted = assembly.residual[row]
            + (0..state.n_unknowns())
                .map(|column| {
                    assembly.jacobian.get(row, column).copied().unwrap_or(0.0)
                        * damping
                        * update[column]
                })
                .sum::<f64>();
        let raw_residual = raw_assembly.residual[row];
        let bounded_residual = bounded_assembly.residual[row];
        crate::fim::trace_sink::write_line(&format!(
            "Y2B residual row={label} current={:.12e} predicted={predicted:.12e} raw_next={raw_residual:.12e} bounded_next={bounded_residual:.12e} raw_prediction_error={:.12e} bounded_prediction_error={:.12e} projection_effect={:.12e}",
            assembly.residual[row],
            raw_residual - predicted,
            bounded_residual - predicted,
            bounded_residual - raw_residual,
        ));
    }
}

#[cfg(test)]
fn y2a_finite_difference_step(state: &FimState, unknown_idx: usize) -> f64 {
    if unknown_idx < state.n_cell_unknowns() {
        let cell = state.cell(unknown_idx / 3);
        return match unknown_idx % 3 {
            0 => 1e-5 * cell.pressure_bar.abs().max(1.0),
            1 => 1e-7,
            2 => 1e-7 * cell.hydrocarbon_var.abs().max(1.0),
            _ => unreachable!(),
        };
    }
    if unknown_idx < state.n_cell_unknowns() + state.n_well_unknowns() {
        let well_idx = unknown_idx - state.n_cell_unknowns();
        return 1e-4 * state.well_bhp[well_idx].abs().max(1.0);
    }
    let perf_idx = unknown_idx - state.n_cell_unknowns() - state.n_well_unknowns();
    1e-4 * state.perforation_rates_m3_day[perf_idx].abs().max(1.0)
}

#[cfg(test)]
fn y2a_perturb_unknown(state: &mut FimState, unknown_idx: usize, delta: f64) {
    if unknown_idx < state.n_cell_unknowns() {
        let cell = &mut state.cells[unknown_idx / 3];
        match unknown_idx % 3 {
            0 => cell.pressure_bar += delta,
            1 => cell.sw += delta,
            2 => cell.hydrocarbon_var += delta,
            _ => unreachable!(),
        }
    } else if unknown_idx < state.n_cell_unknowns() + state.n_well_unknowns() {
        let well_idx = unknown_idx - state.n_cell_unknowns();
        state.well_bhp[well_idx] += delta;
    } else {
        let perf_idx = unknown_idx - state.n_cell_unknowns() - state.n_well_unknowns();
        state.perforation_rates_m3_day[perf_idx] += delta;
    }
}

fn retry_failure_trace_suffix(diagnostics: &FimRetryFailureDiagnostics) -> String {
    let mut parts = vec![format!(
        " retry=[class={} dom={} row={} item={} site={}",
        diagnostics.class.label(),
        diagnostics.dominant_family_label,
        diagnostics.dominant_row,
        diagnostics.dominant_item_index,
        diagnostics.hotspot_site.trace_label(),
    )];
    if let Some(linear_iterations) = diagnostics.linear_iterations {
        parts.push(format!(" linear_iters={}", linear_iterations));
    }
    if diagnostics.used_linear_fallback {
        parts.push(" fallback=true".to_string());
    }
    if let Some(ratio) = diagnostics.cpr_average_reduction_ratio {
        parts.push(format!(" cpr_avg_rr={:.3e}", ratio));
    }
    if let Some(ratio) = diagnostics.cpr_last_reduction_ratio {
        parts.push(format!(" cpr_last_rr={:.3e}", ratio));
    }
    parts.push("]".to_string());
    parts.join("")
}

/// Bundle N (`FIM-BUNDLE-N`, `docs/FIM_BUNDLE_N_DESIGN.md`): selects which nonlinear-layer
/// architecture the Newton loop runs. `Legacy` is the historical ResSim stack (global-scalar
/// Appleyard damping + inflection chop + history stabilization). `OpmAligned` replaces the
/// update-limiting layer with OPM's per-cell chopping (design doc §9.2); further bundle items
/// (acceptance criteria, controller) flip in later checkpoints. Default `Legacy` keeps all
/// existing behavior bit-identical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FimNonlinearFlavor {
    Legacy,
    OpmAligned,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimNewtonOptions {
    pub(crate) max_newton_iterations: usize,
    pub(crate) residual_tolerance: f64,
    pub(crate) material_balance_tolerance: f64,
    pub(crate) update_tolerance: f64,
    pub(crate) min_damping: f64,
    pub(crate) max_pressure_change_bar: f64,
    pub(crate) max_saturation_change: f64,
    pub(crate) max_rs_change_fraction: f64,
    pub(crate) linear: FimLinearSolveOptions,
    pub(crate) verbose: bool,
    pub(crate) nonlinear_flavor: FimNonlinearFlavor,
    /// Bundle W (`docs/FIM_BUNDLE_W_PLAN.md`): replace `relax_well_state_toward_local_consistency`
    /// with the converged per-well inner Newton solve (`fim/wells_inner.rs`). Independent of
    /// `nonlinear_flavor` — evaluable under either flavor. Default `false` = bit-identical to
    /// before this flag existed.
    pub(crate) nested_well_solve: bool,
}

impl Default for FimNewtonOptions {
    fn default() -> Self {
        Self {
            max_newton_iterations: 20,
            residual_tolerance: 1e-5,
            material_balance_tolerance: 1e-5,
            update_tolerance: 1e-3,
            min_damping: 1.0 / 64.0,
            max_pressure_change_bar: DEFAULT_MAX_NEWTON_PRESSURE_CHANGE_BAR,
            max_saturation_change: DEFAULT_MAX_NEWTON_SATURATION_CHANGE,
            max_rs_change_fraction: 1.0,
            linear: FimLinearSolveOptions::default(),
            verbose: false,
            nonlinear_flavor: FimNonlinearFlavor::Legacy,
            nested_well_solve: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FimStepReport {
    pub(crate) accepted_state: FimState,
    pub(crate) converged: bool,
    pub(crate) newton_iterations: usize,
    pub(crate) final_residual_inf_norm: f64,
    pub(crate) final_material_balance_inf_norm: f64,
    pub(crate) final_update_inf_norm: f64,
    pub(crate) last_linear_report: Option<FimLinearSolveReport>,
    pub(crate) accepted_hotspot_site: Option<FimHotspotSite>,
    pub(crate) failure_diagnostics: Option<FimRetryFailureDiagnostics>,
    pub(crate) retry_factor: f64,
    pub(crate) total_time_ms: f64,
    pub(crate) assembly_ms: f64,
    pub(crate) property_eval_ms: f64,
    pub(crate) linear_solve_time_ms: f64,
    pub(crate) linear_preconditioner_build_time_ms: f64,
    pub(crate) state_update_ms: f64,
}

fn retry_factor_for_failure(diagnostics: Option<&FimRetryFailureDiagnostics>) -> f64 {
    match diagnostics.map(|diagnostics| diagnostics.class) {
        Some(FimRetryFailureClass::LinearBad) => 0.25,
        Some(FimRetryFailureClass::NonlinearBad) | Some(FimRetryFailureClass::Mixed) => 0.5,
        None => 0.5,
    }
}

fn hydrocarbon_state_label(regime: HydrocarbonState) -> &'static str {
    match regime {
        HydrocarbonState::Saturated => "sat",
        HydrocarbonState::Undersaturated => "undersat",
    }
}

fn small_dt_hotspot_neighborhood_indices(
    sim: &ReservoirSimulator,
    center_idx: usize,
) -> Vec<usize> {
    let center_i = center_idx % sim.nx;
    let center_j = (center_idx / sim.nx) % sim.ny;
    let center_k = center_idx / (sim.nx * sim.ny);
    let mut indices = Vec::new();

    for dj in -1_i32..=1 {
        for di in -1_i32..=1 {
            let neighbor_i = center_i as i32 + di;
            let neighbor_j = center_j as i32 + dj;
            if neighbor_i < 0
                || neighbor_i >= sim.nx as i32
                || neighbor_j < 0
                || neighbor_j >= sim.ny as i32
            {
                continue;
            }
            let idx =
                center_k * sim.nx * sim.ny + neighbor_j as usize * sim.nx + neighbor_i as usize;
            indices.push(idx);
        }
    }

    if center_k > 0 {
        indices.push(center_idx - sim.nx * sim.ny);
    }
    if center_k + 1 < sim.nz {
        indices.push(center_idx + sim.nx * sim.ny);
    }

    indices.sort_unstable();
    indices.dedup();
    indices
}

fn maybe_trace_small_dt_hotspot_neighborhood(
    sim: &mut ReservoirSimulator,
    verbose: bool,
    context: &str,
    dt_days: f64,
    previous_state: &FimState,
    candidate_state: &FimState,
    hotspot_site: FimHotspotSite,
) {
    const SMALL_DT_NEIGHBORHOOD_TRACE_THRESHOLD_DAYS: f64 = 1.0e-3;

    if dt_days > SMALL_DT_NEIGHBORHOOD_TRACE_THRESHOLD_DAYS {
        return;
    }

    let FimHotspotSite::Cell(center_idx) = hotspot_site else {
        return;
    };

    let center_i = center_idx % sim.nx;
    let center_j = (center_idx / sim.nx) % sim.ny;
    let center_k = center_idx / (sim.nx * sim.ny);
    fim_trace!(
        sim,
        verbose,
        "      hotspot-nbhd {} dt={:.6} center=cell{}({},{},{})",
        context,
        dt_days,
        center_idx,
        center_i,
        center_j,
        center_k,
    );

    for idx in small_dt_hotspot_neighborhood_indices(sim, center_idx) {
        let before_cell = previous_state.cell(idx);
        let after_cell = candidate_state.cell(idx);
        let before = previous_state.derive_cell(sim, idx);
        let after = candidate_state.derive_cell(sim, idx);
        let i = idx % sim.nx;
        let j = (idx / sim.nx) % sim.ny;
        let k = idx / (sim.nx * sim.ny);
        fim_trace!(
            sim,
            verbose,
            "        cell{}({},{},{}) {}->{} p={:.2}->{:.2} dP={:+.2} sw={:.4}->{:.4} dSw={:+.4} so={:.4}->{:.4} dSo={:+.4} sg={:.4}->{:.4} dSg={:+.4} rs={:.4}->{:.4}",
            idx,
            i,
            j,
            k,
            hydrocarbon_state_label(before_cell.regime),
            hydrocarbon_state_label(after_cell.regime),
            before_cell.pressure_bar,
            after_cell.pressure_bar,
            after_cell.pressure_bar - before_cell.pressure_bar,
            before_cell.sw,
            after_cell.sw,
            after_cell.sw - before_cell.sw,
            before.so,
            after.so,
            after.so - before.so,
            before.sg,
            after.sg,
            after.sg - before.sg,
            before.rs,
            after.rs,
        );
    }
}

/// Compute the water fractional flow fw = λ_w / (λ_w + λ_o + λ_g) at a given Sw.
///
/// Holds Sg and pressure fixed (from `cell`). Used only for inflection-point detection,
/// not for residual/Jacobian assembly — so it uses cell.pressure_bar and cell.regime to
/// find the gas saturation but evaluates kr at the given `sw`.
fn fw_at_sw(sim: &ReservoirSimulator, cell: &crate::fim::state::FimCellState, sw: f64) -> f64 {
    let sg = match cell.regime {
        crate::fim::state::HydrocarbonState::Saturated => cell.hydrocarbon_var.max(0.0),
        crate::fim::state::HydrocarbonState::Undersaturated => 0.0,
    };
    let p = cell.pressure_bar;
    let mu_w = sim.get_mu_w(p);
    let mu_o = sim.get_mu_o(p);

    let (lambda_w, lambda_o, lambda_g) = if sim.three_phase_mode {
        if let Some(scal) = &sim.scal_3p {
            let lw = scal.k_rw(sw) / mu_w;
            let lo = scal.k_ro_stone2(sw, sg) / mu_o;
            let lg = scal.k_rg(sg) / sim.get_mu_g(p);
            (lw, lo, lg)
        } else {
            let lw = sim.scal.k_rw(sw) / mu_w;
            let lo = sim.scal.k_ro(sw) / mu_o;
            (lw, lo, 0.0)
        }
    } else {
        let lw = sim.scal.k_rw(sw) / mu_w;
        let lo = sim.scal.k_ro(sw) / mu_o;
        (lw, lo, 0.0)
    };

    let lambda_t = lambda_w + lambda_o + lambda_g;
    if lambda_t < 1e-15 {
        0.0
    } else {
        lambda_w / lambda_t
    }
}

/// Find the inflection point of fw(Sw) for a cell — the Sw at which dfw/dSw is maximum.
///
/// The inflection point divides the fractional-flow curve into two convergence basins.
/// Newton iterations that cross this boundary can diverge or converge slowly (Wang &
/// Tchelepi, 2013). Sampling at N_SAMPLES points and finding the maximum slope is
/// sufficient because the fw curve for standard Corey/tabular kr has a single inflection.
///
/// Returns None if the physical saturation range is degenerate or the fw curve is monotone
/// without a detectable inflection (e.g., very favorable mobility ratio).
fn fw_inflection_point_sw(
    sim: &ReservoirSimulator,
    cell: &crate::fim::state::FimCellState,
) -> Option<f64> {
    const N_SAMPLES: usize = 16;
    const MIN_RANGE: f64 = 1e-4;

    let sg = match cell.regime {
        crate::fim::state::HydrocarbonState::Saturated => cell.hydrocarbon_var.max(0.0),
        crate::fim::state::HydrocarbonState::Undersaturated => 0.0,
    };

    let (swc, sor) = if sim.three_phase_mode {
        sim.scal_3p
            .as_ref()
            .map_or((sim.scal.s_wc, sim.scal.s_or), |s| (s.s_wc, s.s_or))
    } else {
        (sim.scal.s_wc, sim.scal.s_or)
    };

    let sw_lo = swc;
    let sw_hi = (1.0 - sor - sg).min(1.0 - swc * 0.5);
    if sw_hi - sw_lo < MIN_RANGE {
        return None;
    }

    // Sample fw and find the segment with the steepest slope (= inflection point location).
    let dsw = (sw_hi - sw_lo) / N_SAMPLES as f64;
    let mut max_slope = 0.0_f64;
    let mut best_sw = None;

    for i in 0..N_SAMPLES {
        let sw_a = sw_lo + i as f64 * dsw;
        let sw_b = sw_a + dsw;
        let fw_a = fw_at_sw(sim, cell, sw_a);
        let fw_b = fw_at_sw(sim, cell, sw_b);
        let slope = (fw_b - fw_a) / dsw;
        if slope > max_slope {
            max_slope = slope;
            best_sw = Some(0.5 * (sw_a + sw_b));
        }
    }

    // Only meaningful if the fw curve actually curves — skip nearly flat regions.
    if max_slope < 1e-6 {
        return None;
    }

    best_sw
}

/// Newton-kernel-damping Stage 1 probe: read-only breakdown of which constraint
/// limited `appleyard_damping` and the raw (pre-damping) update peaks that fed
/// into each constraint. Used to understand why the initial-iter damping is so
/// small on the case 2 medium-water step (0.0055 at dt=0.25 observed).
#[derive(Clone, Debug)]
pub(crate) struct DampingBreakdown {
    pub(crate) final_damping: f64,
    pub(crate) binding_kind: &'static str,
    pub(crate) binding_cell: Option<usize>,
    pub(crate) binding_well: Option<usize>,
    pub(crate) raw_dp_peak: f64,
    pub(crate) raw_dp_peak_cell: Option<usize>,
    pub(crate) raw_dsw_peak: f64,
    pub(crate) raw_dsw_peak_cell: Option<usize>,
    pub(crate) raw_dh_peak: f64,
    pub(crate) raw_dh_peak_cell: Option<usize>,
    pub(crate) raw_dbhp_peak: f64,
    pub(crate) raw_dbhp_peak_well: Option<usize>,
    pub(crate) inflection_crossings: u32,
}

fn appleyard_damping_breakdown(
    sim: &ReservoirSimulator,
    state: &FimState,
    update: &DVector<f64>,
    options: &FimNewtonOptions,
) -> DampingBreakdown {
    let mut max_damping = 1.0_f64;
    let mut binding_kind: &'static str = "unbound";
    let mut binding_cell: Option<usize> = None;
    let mut binding_well: Option<usize> = None;
    let mut raw_dp_peak = 0.0_f64;
    let mut raw_dp_peak_cell: Option<usize> = None;
    let mut raw_dsw_peak = 0.0_f64;
    let mut raw_dsw_peak_cell: Option<usize> = None;
    let mut raw_dh_peak = 0.0_f64;
    let mut raw_dh_peak_cell: Option<usize> = None;
    let mut raw_dbhp_peak = 0.0_f64;
    let mut raw_dbhp_peak_well: Option<usize> = None;
    let mut inflection_crossings: u32 = 0;

    let n_cells = state.cells.len();
    for idx in 0..n_cells {
        let offset = idx * 3;
        let cell = state.cell(idx);

        let dp = update[offset].abs();
        if dp > raw_dp_peak {
            raw_dp_peak = dp;
            raw_dp_peak_cell = Some(idx);
        }
        if dp > 1e-12 {
            let cap = options.max_pressure_change_bar / dp;
            if cap < max_damping {
                max_damping = cap;
                binding_kind = "pressure";
                binding_cell = Some(idx);
                binding_well = None;
            }
        }

        let dsw = update[offset + 1].abs();
        if dsw > raw_dsw_peak {
            raw_dsw_peak = dsw;
            raw_dsw_peak_cell = Some(idx);
        }
        if dsw > 1e-12 {
            let cap = options.max_saturation_change / dsw;
            if cap < max_damping {
                max_damping = cap;
                binding_kind = "sw_appleyard";
                binding_cell = Some(idx);
                binding_well = None;
            }
        }

        // Trust-region boundary at the fw inflection point (water).
        // Only chop when the proposed step would overshoot the inflection by
        // a meaningful margin — proposed step magnitude must be at least
        // FW_INFLECTION_OVERSHOOT_FACTOR * dist_to_inflection. Marginal
        // crossings are let through; basin-jumping protection still holds
        // for genuinely wild updates.
        //
        // `FIM-NEWTON-007` (REFUTED, see registry): `dist` degenerates toward zero for a cell
        // sitting essentially at the inflection point, and the un-margined
        // `chop = dist / |dsw_signed|` then chops `max_damping` to ~0, stalling Newton at that
        // state (observed live at `water@387`/`cell129`). Three variants that relax this
        // degenerate case (additive margin, `dist.max(max_saturation_change)` floor, skip the
        // chop entirely below a `1e-4` degenerate-range threshold) were each tried live and each
        // regressed the heavy case substantially (`62→263`, `62→263`, `62→238` substeps, all with
        // `retry_dom` reverting to the just-fixed `perf@1299` pattern) — the heavy case's Newton
        // trajectory is apparently sensitive enough to this exact site's damping that any change
        // here perturbs the path into re-triggering a different, already-addressed failure mode,
        // rather than genuinely fixing anything. Left as-is; do not re-attempt a local chop
        // formula change at this site without new evidence about *why* it's this sensitive.
        let dsw_signed = update[offset + 1];
        if dsw_signed.abs() > 1e-12 {
            if let Some(sw_inflect) = fw_inflection_point_sw(sim, cell) {
                let sw_full = cell.sw + max_damping * dsw_signed;
                let side_before = cell.sw - sw_inflect;
                let side_after = sw_full - sw_inflect;
                if side_before * side_after < 0.0 {
                    let proposed_step_mag = max_damping * dsw_signed.abs();
                    let dist = (sw_inflect - cell.sw).abs();
                    let overshoot_threshold = FW_INFLECTION_OVERSHOOT_FACTOR * dist;
                    if proposed_step_mag >= overshoot_threshold {
                        inflection_crossings += 1;
                        let chop = (dist / dsw_signed.abs()).clamp(0.0, max_damping);
                        if chop < max_damping {
                            max_damping = chop;
                            binding_kind = "sw_inflection";
                            binding_cell = Some(idx);
                            binding_well = None;
                        }
                    }
                }
            }
        }

        let dh = update[offset + 2];
        let dh_abs = dh.abs();
        if dh_abs > raw_dh_peak {
            raw_dh_peak = dh_abs;
            raw_dh_peak_cell = Some(idx);
        }
        if dh_abs > 1e-12 {
            match cell.regime {
                crate::fim::state::HydrocarbonState::Saturated => {
                    let cap_sg = options.max_saturation_change / dh_abs;
                    if cap_sg < max_damping {
                        max_damping = cap_sg;
                        binding_kind = "sg_appleyard";
                        binding_cell = Some(idx);
                        binding_well = None;
                    }
                    let dso = (update[offset + 1] + dh).abs();
                    if dso > 1e-12 {
                        let cap_so = options.max_saturation_change / dso;
                        if cap_so < max_damping {
                            max_damping = cap_so;
                            binding_kind = "so_implied";
                            binding_cell = Some(idx);
                            binding_well = None;
                        }
                    }
                }
                crate::fim::state::HydrocarbonState::Undersaturated => {
                    let rs_scale = cell.hydrocarbon_var.abs().max(1.0);
                    let cap_rs = options.max_rs_change_fraction * rs_scale / dh_abs;
                    if cap_rs < max_damping {
                        max_damping = cap_rs;
                        binding_kind = "rs";
                        binding_cell = Some(idx);
                        binding_well = None;
                    }
                }
            }
        }
    }

    let well_offset = state.n_cell_unknowns();
    for well_idx in 0..state.n_well_unknowns() {
        let dbhp = update[well_offset + well_idx].abs();
        if dbhp > raw_dbhp_peak {
            raw_dbhp_peak = dbhp;
            raw_dbhp_peak_well = Some(well_idx);
        }
        if dbhp > 1e-12 {
            let cap = options.max_pressure_change_bar / dbhp;
            if cap < max_damping {
                max_damping = cap;
                binding_kind = "bhp";
                binding_cell = None;
                binding_well = Some(well_idx);
            }
        }
    }

    DampingBreakdown {
        final_damping: max_damping.clamp(0.0, 1.0),
        binding_kind,
        binding_cell,
        binding_well,
        raw_dp_peak,
        raw_dp_peak_cell,
        raw_dsw_peak,
        raw_dsw_peak_cell,
        raw_dh_peak,
        raw_dh_peak_cell,
        raw_dbhp_peak,
        raw_dbhp_peak_well,
        inflection_crossings,
    }
}

fn cell_phase_saturations(cell: &crate::fim::state::FimCellState) -> (f64, f64, f64) {
    match cell.regime {
        crate::fim::state::HydrocarbonState::Saturated => {
            let sw = cell.sw;
            let sg = cell.hydrocarbon_var;
            let so = 1.0 - sw - sg;
            (sw, so, sg)
        }
        crate::fim::state::HydrocarbonState::Undersaturated => {
            let sw = cell.sw;
            let so = 1.0 - sw;
            (sw, so, 0.0)
        }
    }
}

fn local_cell_move_deltas(
    previous_state: &FimState,
    candidate_state: &FimState,
    cell_idx: usize,
) -> Option<(f64, f64, f64, f64)> {
    let previous_cell = previous_state.cells.get(cell_idx)?;
    let candidate_cell = candidate_state.cells.get(cell_idx)?;
    let previous_phase_saturations = cell_phase_saturations(previous_cell);
    let candidate_phase_saturations = cell_phase_saturations(candidate_cell);

    Some((
        (candidate_cell.pressure_bar - previous_cell.pressure_bar).abs(),
        (candidate_phase_saturations.0 - previous_phase_saturations.0).abs(),
        (candidate_phase_saturations.1 - previous_phase_saturations.1).abs(),
        (candidate_phase_saturations.2 - previous_phase_saturations.2).abs(),
    ))
}

fn move_is_below_effective_trace_threshold(
    pressure_delta_bar: f64,
    water_delta: f64,
    oil_delta: f64,
    gas_delta: f64,
) -> bool {
    pressure_delta_bar < EFFECTIVE_TRACE_PRESSURE_MOVE_THRESHOLD_BAR
        && water_delta < EFFECTIVE_TRACE_SATURATION_MOVE_THRESHOLD
        && oil_delta < EFFECTIVE_TRACE_SATURATION_MOVE_THRESHOLD
        && gas_delta < EFFECTIVE_TRACE_SATURATION_MOVE_THRESHOLD
}

fn cell_attached_perforation_context_trace(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &crate::fim::wells::FimWellTopology,
    cell_idx: usize,
) -> String {
    let attached = topology
        .perforations
        .iter()
        .enumerate()
        .filter(|(_, perforation)| perforation.cell_index == cell_idx)
        .filter_map(|(perf_idx, _)| {
            let detail =
                perforation_local_block(topology, state, perf_idx).residual_diagnostics(sim)?;
            Some(format!(
                "perf{}->well{} inj={} q={:.3e} conn={:.3e} draw={:.3e} bhp={:.3}",
                detail.perf_idx,
                detail.physical_well_idx,
                detail.injector,
                detail.q_unknown_m3_day,
                detail.q_connection_m3_day,
                detail.drawdown_bar,
                detail.bhp_bar,
            ))
        })
        .collect::<Vec<_>>();

    if attached.is_empty() {
        "attached_perfs=none".to_string()
    } else {
        format!("attached_perfs=[{}]", attached.join(" | "))
    }
}

fn effective_move_threshold_trace(
    sim: &ReservoirSimulator,
    state: &FimState,
    candidate: &FimState,
    topology: &crate::fim::wells::FimWellTopology,
    diagnostics: &ResidualFamilyDiagnostics,
    damping: f64,
) -> Option<String> {
    match diagnostics.global.family {
        ResidualRowFamily::Water
        | ResidualRowFamily::OilComponent
        | ResidualRowFamily::GasComponent => {}
        _ => return None,
    }

    let (pressure_delta_bar, water_delta, oil_delta, gas_delta) =
        local_cell_move_deltas(state, candidate, diagnostics.global.item_index)?;

    if !move_is_below_effective_trace_threshold(
        pressure_delta_bar,
        water_delta,
        oil_delta,
        gas_delta,
    ) {
        return None;
    }

    Some(format!(
        "cell{} row={} damp={:.4} local_dP={:.5} local_dSw={:.6} local_dSo={:.6} local_dSg={:.6} {}",
        diagnostics.global.item_index,
        diagnostics.global.row,
        damping,
        pressure_delta_bar,
        water_delta,
        oil_delta,
        gas_delta,
        cell_attached_perforation_context_trace(
            sim,
            candidate,
            topology,
            diagnostics.global.item_index
        ),
    ))
}

#[derive(Clone, Debug, PartialEq)]
struct NonlinearHistoryStabilizationDecision {
    damping_cap: f64,
    repeated_site_streak: u32,
    site: FimHotspotSite,
}

fn cell_ijk(sim: &ReservoirSimulator, cell_idx: usize) -> (usize, usize, usize) {
    let i = cell_idx % sim.nx;
    let j = (cell_idx / sim.nx) % sim.ny;
    let k = cell_idx / (sim.nx * sim.ny);
    (i, j, k)
}

fn exact_residual_hotspot_site(peak: &ResidualFamilyPeak) -> FimHotspotSite {
    match peak.family {
        ResidualRowFamily::Water
        | ResidualRowFamily::OilComponent
        | ResidualRowFamily::GasComponent => FimHotspotSite::Cell(peak.item_index),
        ResidualRowFamily::WellConstraint => FimHotspotSite::Well(peak.item_index),
        ResidualRowFamily::PerforationFlow => FimHotspotSite::Perforation(peak.item_index),
    }
}

fn gas_injector_symmetry_site(
    sim: &ReservoirSimulator,
    topology: &crate::fim::wells::FimWellTopology,
    cell_idx: usize,
) -> Option<FimHotspotSite> {
    let (cell_i, cell_j, cell_k) = cell_ijk(sim, cell_idx);
    topology
        .perforations
        .iter()
        .filter(|perforation| perforation.injector)
        .map(|perforation| {
            let di = perforation.i.abs_diff(cell_i);
            let dj = perforation.j.abs_diff(cell_j);
            let dk = perforation.k.abs_diff(cell_k);
            let major_offset = di.max(dj);
            let minor_offset = di.min(dj);
            (
                (
                    di + dj + dk,
                    major_offset,
                    minor_offset,
                    dk,
                    perforation.physical_well_index,
                ),
                FimHotspotSite::GasInjectorSymmetry {
                    injector_well_index: perforation.physical_well_index,
                    major_offset,
                    minor_offset,
                    vertical_offset: dk,
                },
            )
        })
        .min_by_key(|(distance_key, _)| *distance_key)
        .map(|(_, site)| site)
}

fn residual_hotspot_site(
    sim: &ReservoirSimulator,
    topology: &crate::fim::wells::FimWellTopology,
    peak: &ResidualFamilyPeak,
) -> FimHotspotSite {
    match peak.family {
        ResidualRowFamily::GasComponent => {
            gas_injector_symmetry_site(sim, topology, peak.item_index)
                .unwrap_or_else(|| exact_residual_hotspot_site(peak))
        }
        _ => exact_residual_hotspot_site(peak),
    }
}

fn representative_well_index(sim: &ReservoirSimulator, well_idx: usize) -> usize {
    let Some(physical_well_id) = sim.wells[well_idx].physical_well_id.as_deref() else {
        return well_idx;
    };

    sim.wells
        .iter()
        .position(|well| well.physical_well_id.as_deref() == Some(physical_well_id))
        .unwrap_or(well_idx)
}

fn nearest_well_reference_index(
    sim: &ReservoirSimulator,
    i: usize,
    j: usize,
    k: usize,
) -> Option<usize> {
    sim.wells
        .iter()
        .enumerate()
        .map(|(well_idx, well)| {
            let di = well.i.abs_diff(i);
            let dj = well.j.abs_diff(j);
            let dk = well.k.abs_diff(k);
            let major_offset = di.max(dj);
            let minor_offset = di.min(dj);
            (
                (
                    di + dj + dk,
                    major_offset,
                    minor_offset,
                    dk,
                    representative_well_index(sim, well_idx),
                ),
                representative_well_index(sim, well_idx),
            )
        })
        .min_by_key(|(distance_key, _)| *distance_key)
        .map(|(_, representative_index)| representative_index)
}

fn non_gas_hotspot_sites_share_local_region(
    sim: &ReservoirSimulator,
    previous_site: FimHotspotSite,
    current_site: FimHotspotSite,
) -> bool {
    const NON_GAS_HISTORY_LATERAL_RADIUS: usize = 1;

    let (FimHotspotSite::Cell(previous_cell_idx), FimHotspotSite::Cell(current_cell_idx)) =
        (previous_site, current_site)
    else {
        return previous_site == current_site;
    };

    let (previous_i, previous_j, previous_k) = cell_ijk(sim, previous_cell_idx);
    let (current_i, current_j, current_k) = cell_ijk(sim, current_cell_idx);

    previous_k == current_k
        && nearest_well_reference_index(sim, previous_i, previous_j, previous_k)
            == nearest_well_reference_index(sim, current_i, current_j, current_k)
        && previous_i.abs_diff(current_i) <= NON_GAS_HISTORY_LATERAL_RADIUS
        && previous_j.abs_diff(current_j) <= NON_GAS_HISTORY_LATERAL_RADIUS
}

fn repeated_nonlinear_hotspot_streak(
    sim: &ReservoirSimulator,
    previous_site: Option<FimHotspotSite>,
    previous_residual_norm: f64,
    current_diagnostics: &ResidualFamilyDiagnostics,
    current_site: FimHotspotSite,
    current_residual_norm: f64,
    current_streak: u32,
) -> u32 {
    let Some(previous_site) = previous_site else {
        return 0;
    };
    if !previous_residual_norm.is_finite() || previous_residual_norm <= f64::EPSILON {
        return 0;
    }

    let same_site = hotspot_sites_share_history_region(
        sim,
        current_diagnostics.global.family,
        previous_site,
        current_site,
    );
    let weak_progress_ratio = match current_diagnostics.global.family {
        ResidualRowFamily::GasComponent => NONLINEAR_HISTORY_GAS_WEAK_PROGRESS_RATIO,
        _ => NONLINEAR_HISTORY_WEAK_PROGRESS_RATIO,
    };
    let weak_progress = current_residual_norm >= previous_residual_norm * weak_progress_ratio;

    if same_site && weak_progress {
        current_streak + 1
    } else {
        0
    }
}

fn nonlinear_history_stabilization_decision(
    linear_report: &FimLinearSolveReport,
    _current_diagnostics: &ResidualFamilyDiagnostics,
    current_residual_norm: f64,
    options: &FimNewtonOptions,
    repeated_site_streak: u32,
    current_site: FimHotspotSite,
) -> Option<NonlinearHistoryStabilizationDecision> {
    if repeated_site_streak < NONLINEAR_HISTORY_MIN_STREAK
        || !linear_report.converged
        || current_residual_norm
            > options.residual_tolerance * NONLINEAR_HISTORY_RESIDUAL_BAND_FACTOR
    {
        return None;
    }

    let damping_cap = if repeated_site_streak == NONLINEAR_HISTORY_MIN_STREAK {
        NONLINEAR_HISTORY_FIRST_DAMPING_CAP
    } else {
        NONLINEAR_HISTORY_REPEAT_DAMPING_CAP
    };

    Some(NonlinearHistoryStabilizationDecision {
        damping_cap,
        repeated_site_streak,
        site: current_site,
    })
}

fn nonlinear_history_stabilization_trace(
    decision: &NonlinearHistoryStabilizationDecision,
) -> String {
    format!(
        " hist=[site={} streak={} damp_cap={:.3}]",
        decision.site.trace_label(),
        decision.repeated_site_streak,
        decision.damping_cap,
    )
}

// OPM-style global oscillation detector + persistent relaxation scalar (Phase 7, sub-phase
// 7.1 — wired but inert: only traced, not yet folded into `damping`). Ported from
// `opm/simulators/flow/NonlinearSolver.cpp::detectOscillations()`/`stabilizeNonlinearUpdate()`.
// Unlike `nonlinear_history_stabilization_decision` above (cell-site-keyed, hard-capped),
// this tracks per-family *residual norm history* and evolves a single scalar smoothly.
//
// Phase 11 follow-up (`FIM-NEWTON-006`): originally scoped to `water`/`oil_component`/
// `gas_component` only, matching a guess that well/perforation rows have "different scaling/
// switch behavior" (deferred pending evidence). That evidence now exists: a live heavy-case
// retry showed `perforation_flow`'s scaled residual alternating in an exact 2-period cycle
// (`d1 ≈ 0, d2 ≈ 0.6` — a textbook match for this exact test) while water/oil_component stayed
// flat, and a Newton run with well/perforation unknowns fully Schur-eliminated from the linear
// system (`FIM-LINEAR-010`) showed the *identical* oscillation — proving it is not a linear-
// system-structure artifact this detector should have been blind to, but a genuine nonlinear
// residual oscillation OPM's own (family-agnostic) test is designed to catch. Widened to include
// `well_constraint`/`perforation_flow`.

const OSCILLATION_RELAX_REL_TOL: f64 = 0.2;
const OSCILLATION_RELAX_INCREMENT: f64 = 0.1;
const OSCILLATION_MAX_RELAX_FLOOR: f64 = 0.5;
const OSCILLATION_MIN_OSCILLATING_PHASES: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq)]
struct PerFamilyNorms {
    water: f64,
    oil_component: f64,
    gas_component: f64,
    well_constraint: f64,
    perforation_flow: f64,
}

impl Default for PerFamilyNorms {
    fn default() -> Self {
        Self {
            water: f64::INFINITY,
            oil_component: f64::INFINITY,
            gas_component: f64::INFINITY,
            well_constraint: f64::INFINITY,
            perforation_flow: f64::INFINITY,
        }
    }
}

impl PerFamilyNorms {
    fn from_diagnostics(diagnostics: &ResidualFamilyDiagnostics) -> Self {
        Self {
            water: diagnostics.water.scaled_value,
            oil_component: diagnostics.oil_component.scaled_value,
            gas_component: diagnostics.gas_component.scaled_value,
            well_constraint: diagnostics
                .well_constraint
                .map_or(f64::INFINITY, |peak| peak.scaled_value),
            perforation_flow: diagnostics
                .perforation_flow
                .map_or(f64::INFINITY, |peak| peak.scaled_value),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RelaxationState {
    residual_norm_2ago: PerFamilyNorms,
    residual_norm_1ago: PerFamilyNorms,
    current_relaxation: f64,
    history_len: u32,
}

impl Default for RelaxationState {
    fn default() -> Self {
        Self {
            residual_norm_2ago: PerFamilyNorms::default(),
            residual_norm_1ago: PerFamilyNorms::default(),
            current_relaxation: 1.0,
            history_len: 0,
        }
    }
}

/// OPM `detectOscillations()`'s single-family test: `d1 = |F0-F2|/F0` (2-step relative
/// change), `d2 = |F0-F1|/F0` (1-step). Oscillating iff the 2-step change is small while the
/// 1-step change is large — i.e. the residual is swinging back toward where it was.
fn family_is_oscillating(f0: f64, f1: f64, f2: f64) -> bool {
    if !(f0.is_finite() && f1.is_finite() && f2.is_finite()) || f0 <= 0.0 {
        return false;
    }
    let d1 = (f0 - f2).abs() / f0;
    let d2 = (f0 - f1).abs() / f0;
    d1 < OSCILLATION_RELAX_REL_TOL && OSCILLATION_RELAX_REL_TOL < d2
}

fn detect_oscillation(
    current: PerFamilyNorms,
    prev1: PerFamilyNorms,
    prev2: PerFamilyNorms,
) -> u32 {
    [
        family_is_oscillating(current.water, prev1.water, prev2.water),
        family_is_oscillating(
            current.oil_component,
            prev1.oil_component,
            prev2.oil_component,
        ),
        family_is_oscillating(
            current.gas_component,
            prev1.gas_component,
            prev2.gas_component,
        ),
        family_is_oscillating(
            current.well_constraint,
            prev1.well_constraint,
            prev2.well_constraint,
        ),
        family_is_oscillating(
            current.perforation_flow,
            prev1.perforation_flow,
            prev2.perforation_flow,
        ),
    ]
    .into_iter()
    .filter(|&osc| osc)
    .count() as u32
}

/// OPM never ramps `current_relaxation` back up mid-solve once it starts decaying — only
/// port that behavior; do not invent a recovery ramp (see `fim-solver-debug` skill's
/// known-reverted-lever discipline on widening acceptance/relaxation ad hoc).
fn next_relaxation_factor(current_relaxation: f64, oscillating_phase_count: u32) -> f64 {
    if oscillating_phase_count >= OSCILLATION_MIN_OSCILLATING_PHASES {
        (current_relaxation - OSCILLATION_RELAX_INCREMENT).max(OSCILLATION_MAX_RELAX_FLOOR)
    } else {
        current_relaxation
    }
}

/// Sub-phase 7.2: compose Appleyard damping, history-stabilization cap (if any), and the
/// OPM-style oscillation-relaxation scalar as three independent multiplicative bounds on
/// the same Newton update — whichever is tightest wins.
fn compose_damping(
    appleyard_final_damping: f64,
    history_stabilization_cap: Option<f64>,
    oscillation_relaxation: f64,
) -> f64 {
    [
        Some(appleyard_final_damping),
        history_stabilization_cap,
        Some(oscillation_relaxation),
    ]
    .into_iter()
    .flatten()
    .fold(1.0_f64, f64::min)
}

fn state_update_change_bounds(previous_state: &FimState, candidate_state: &FimState) -> (f64, f64) {
    let mut max_pressure_change = 0.0_f64;
    let mut max_saturation_change = 0.0_f64;

    for (previous_cell, candidate_cell) in previous_state
        .cells
        .iter()
        .zip(candidate_state.cells.iter())
    {
        max_pressure_change = max_pressure_change
            .max((candidate_cell.pressure_bar - previous_cell.pressure_bar).abs());

        let previous_phase_saturations = cell_phase_saturations(previous_cell);
        let candidate_phase_saturations = cell_phase_saturations(candidate_cell);
        max_saturation_change = max_saturation_change
            .max((candidate_phase_saturations.0 - previous_phase_saturations.0).abs())
            .max((candidate_phase_saturations.1 - previous_phase_saturations.1).abs())
            .max((candidate_phase_saturations.2 - previous_phase_saturations.2).abs());
    }

    for (previous_bhp, candidate_bhp) in previous_state
        .well_bhp
        .iter()
        .zip(candidate_state.well_bhp.iter())
    {
        max_pressure_change = max_pressure_change.max((candidate_bhp - previous_bhp).abs());
    }

    (max_pressure_change, max_saturation_change)
}

fn candidate_respects_update_bounds(
    previous_state: &FimState,
    candidate_state: &FimState,
    options: &FimNewtonOptions,
) -> bool {
    let (max_pressure_change, max_saturation_change) =
        state_update_change_bounds(previous_state, candidate_state);
    max_pressure_change <= options.max_pressure_change_bar + 1e-9
        && max_saturation_change <= options.max_saturation_change + 1e-9
}

fn scaled_residual_inf_norm(
    residual: &DVector<f64>,
    scaling: &crate::fim::scaling::EquationScaling,
) -> f64 {
    let mut max_norm = 0.0_f64;
    let n_cells = scaling.water.len();

    for i in 0..n_cells {
        max_norm = max_norm.max(residual[i * 3].abs() / scaling.water[i]);
        max_norm = max_norm.max(residual[i * 3 + 1].abs() / scaling.oil_component[i]);
        max_norm = max_norm.max(residual[i * 3 + 2].abs() / scaling.gas_component[i]);
    }

    let mut offset = n_cells * 3;
    for i in 0..scaling.well_constraint.len() {
        max_norm = max_norm.max(residual[offset + i].abs() / scaling.well_constraint[i]);
    }
    offset += scaling.well_constraint.len();
    for i in 0..scaling.perforation_flow.len() {
        max_norm = max_norm.max(residual[offset + i].abs() / scaling.perforation_flow[i]);
    }

    max_norm
}

fn scaled_update_inf_norm(
    update: &DVector<f64>,
    scaling: &crate::fim::scaling::VariableScaling,
) -> f64 {
    let mut max_norm = 0.0_f64;
    let n_cells = scaling.pressure.len();

    for i in 0..n_cells {
        max_norm = max_norm.max(update[i * 3].abs() / scaling.pressure[i]);
        max_norm = max_norm.max(update[i * 3 + 1].abs() / scaling.sw[i]);
        max_norm = max_norm.max(update[i * 3 + 2].abs() / scaling.hydrocarbon_var[i]);
    }

    let mut offset = n_cells * 3;
    for i in 0..scaling.well_bhp.len() {
        max_norm = max_norm.max(update[offset + i].abs() / scaling.well_bhp[i]);
    }
    offset += scaling.well_bhp.len();
    for i in 0..scaling.perforation_rate.len() {
        max_norm = max_norm.max(update[offset + i].abs() / scaling.perforation_rate[i]);
    }

    max_norm
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UpdateVariableFamily {
    Pressure,
    WaterSaturation,
    HydrocarbonVariable,
    WellBhp,
    PerforationRate,
}

impl UpdateVariableFamily {
    fn label(self) -> &'static str {
        match self {
            Self::Pressure => "pressure",
            Self::WaterSaturation => "sw",
            Self::HydrocarbonVariable => "hc",
            Self::WellBhp => "bhp",
            Self::PerforationRate => "perf-rate",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct UpdateFamilyPeak {
    family: UpdateVariableFamily,
    scaled_value: f64,
    row: usize,
    item_index: usize,
}

fn update_variable_peak(
    current: &mut Option<UpdateFamilyPeak>,
    family: UpdateVariableFamily,
    scaled_value: f64,
    row: usize,
    item_index: usize,
) {
    let scaled_value = if scaled_value.is_finite() {
        scaled_value
    } else {
        f64::INFINITY
    };
    let candidate = UpdateFamilyPeak {
        family,
        scaled_value,
        row,
        item_index,
    };
    if current.is_none_or(|existing| candidate.scaled_value > existing.scaled_value) {
        *current = Some(candidate);
    }
}

fn scaled_update_peak(
    update: &DVector<f64>,
    scaling: &crate::fim::scaling::VariableScaling,
) -> UpdateFamilyPeak {
    let n_cells = scaling.pressure.len();
    let mut peak = None;

    for i in 0..n_cells {
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::Pressure,
            update[i * 3].abs() / scaling.pressure[i],
            i * 3,
            i,
        );
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::WaterSaturation,
            update[i * 3 + 1].abs() / scaling.sw[i],
            i * 3 + 1,
            i,
        );
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::HydrocarbonVariable,
            update[i * 3 + 2].abs() / scaling.hydrocarbon_var[i],
            i * 3 + 2,
            i,
        );
    }

    let mut offset = n_cells * 3;
    for i in 0..scaling.well_bhp.len() {
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::WellBhp,
            update[offset + i].abs() / scaling.well_bhp[i],
            offset + i,
            i,
        );
    }
    offset += scaling.well_bhp.len();
    for i in 0..scaling.perforation_rate.len() {
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::PerforationRate,
            update[offset + i].abs() / scaling.perforation_rate[i],
            offset + i,
            i,
        );
    }

    peak.expect("update diagnostics require at least one unknown")
}

fn scaled_applied_update_peak(
    state: &FimState,
    candidate: &FimState,
    scaling: &crate::fim::scaling::VariableScaling,
) -> UpdateFamilyPeak {
    let mut peak = None;

    for (idx, (current, next)) in state.cells.iter().zip(candidate.cells.iter()).enumerate() {
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::Pressure,
            (next.pressure_bar - current.pressure_bar).abs() / scaling.pressure[idx],
            idx * 3,
            idx,
        );
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::WaterSaturation,
            (next.sw - current.sw).abs() / scaling.sw[idx],
            idx * 3 + 1,
            idx,
        );
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::HydrocarbonVariable,
            (next.hydrocarbon_var - current.hydrocarbon_var).abs() / scaling.hydrocarbon_var[idx],
            idx * 3 + 2,
            idx,
        );
    }

    let mut offset = state.cells.len() * 3;
    for (idx, (current, next)) in state
        .well_bhp
        .iter()
        .zip(candidate.well_bhp.iter())
        .enumerate()
    {
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::WellBhp,
            (next - current).abs() / scaling.well_bhp[idx],
            offset + idx,
            idx,
        );
    }
    offset += state.well_bhp.len();
    for (idx, (current, next)) in state
        .perforation_rates_m3_day
        .iter()
        .zip(candidate.perforation_rates_m3_day.iter())
        .enumerate()
    {
        update_variable_peak(
            &mut peak,
            UpdateVariableFamily::PerforationRate,
            (next - current).abs() / scaling.perforation_rate[idx],
            offset + idx,
            idx,
        );
    }

    peak.expect("applied update diagnostics require at least one unknown")
}

fn update_peak_trace(peak: UpdateFamilyPeak) -> String {
    format!(
        " upd_peak=[{}={:.3e} row={} item={}]",
        peak.family.label(),
        peak.scaled_value,
        peak.row,
        peak.item_index
    )
}

pub(crate) fn iterate_has_material_change(previous_state: &FimState, state: &FimState) -> bool {
    const PRESSURE_EPS: f64 = 1e-12;
    const SATURATION_EPS: f64 = 1e-12;
    const RS_EPS: f64 = 1e-12;
    const WELL_BHP_EPS: f64 = 1e-12;
    const PERF_RATE_EPS: f64 = 1e-12;

    previous_state
        .cells
        .iter()
        .zip(state.cells.iter())
        .any(|(previous, current)| {
            (current.pressure_bar - previous.pressure_bar).abs() > PRESSURE_EPS
                || (current.sw - previous.sw).abs() > SATURATION_EPS
                || (current.hydrocarbon_var - previous.hydrocarbon_var).abs() > RS_EPS
                || current.regime != previous.regime
        })
        || previous_state
            .well_bhp
            .iter()
            .zip(state.well_bhp.iter())
            .any(|(previous, current)| (current - previous).abs() > WELL_BHP_EPS)
        || previous_state
            .perforation_rates_m3_day
            .iter()
            .zip(state.perforation_rates_m3_day.iter())
            .any(|(previous, current)| (current - previous).abs() > PERF_RATE_EPS)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct GlobalMaterialBalanceDiagnostics {
    water: f64,
    oil_component: f64,
    gas_component: f64,
    global_family: ResidualRowFamily,
    global_value: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct AcceptedStateConvergenceDiagnostics {
    state: FimState,
    residual_inf_norm: f64,
    residual_diagnostics: ResidualFamilyDiagnostics,
    residual_detail: Option<String>,
    material_balance_inf_norm: f64,
    material_balance_diagnostics: GlobalMaterialBalanceDiagnostics,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResidualRowFamily {
    Water,
    OilComponent,
    GasComponent,
    WellConstraint,
    PerforationFlow,
}

impl ResidualRowFamily {
    fn label(self) -> &'static str {
        match self {
            Self::Water => "water",
            Self::OilComponent => "oil",
            Self::GasComponent => "gas",
            Self::WellConstraint => "well",
            Self::PerforationFlow => "perf",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ResidualFamilyPeak {
    family: ResidualRowFamily,
    scaled_value: f64,
    row: usize,
    item_index: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct ResidualFamilyDiagnostics {
    water: ResidualFamilyPeak,
    oil_component: ResidualFamilyPeak,
    gas_component: ResidualFamilyPeak,
    well_constraint: Option<ResidualFamilyPeak>,
    perforation_flow: Option<ResidualFamilyPeak>,
    global: ResidualFamilyPeak,
}

fn update_family_peak(
    current: &mut Option<ResidualFamilyPeak>,
    family: ResidualRowFamily,
    scaled_value: f64,
    row: usize,
    item_index: usize,
) {
    let scaled_value = if scaled_value.is_finite() {
        scaled_value
    } else {
        f64::INFINITY
    };
    let candidate = ResidualFamilyPeak {
        family,
        scaled_value,
        row,
        item_index,
    };
    if current.is_none_or(|existing| candidate.scaled_value > existing.scaled_value) {
        *current = Some(candidate);
    }
}

/// OPM shipped convergence tolerances (Flow 2025.10 defaults, verified from the installed
/// binary's `--help-all` and `opm-simulators` tag `release/2025.10/final` — see
/// `docs/FIM_BUNDLE_N_DESIGN.md` §9.1). Used by the inert Bundle N checkpoint-1 diagnostic
/// below; they do NOT participate in any accept/retry decision yet.
const OPM_TOLERANCE_CNV: f64 = 1e-2;
const OPM_TOLERANCE_CNV_RELAXED: f64 = 1.0;
const OPM_TOLERANCE_MB: f64 = 1e-7;
const OPM_TOLERANCE_MB_RELAXED: f64 = 1e-6;
const OPM_RELAXED_MAX_PV_FRACTION: f64 = 0.03;
/// OPM `newton-min-iterations` default (2): `iteration() >= minIter` gates
/// `NonlinearSystemBlackOilReservoir::initialLinearization`'s accept decision
/// (`NonlinearSystemBlackOilReservoir_impl.hpp:175`), where `iteration()` is
/// `NewtonIterationContext`'s 0-based counter, incremented once per full
/// assemble-check-then-update cycle via `advanceIteration()` — called only
/// *after* `nonlinearIteration()` completes (`:229`), i.e. after the check.
/// ResSim's own `for iteration in 0..max_newton_iterations` loop has the
/// identical shape (assemble, check `converged_on_entry`, apply update only
/// if not converged), so `iteration` here is the direct analog of OPM's
/// `iteration()`: both equal the number of Newton updates already applied
/// prior to the current check. OPM's default `minIter=2` therefore requires
/// `iteration >= 2` (two prior updates, three total residual evaluations)
/// before acceptance is even possible. `FIM-DIAG-003` D2 (2026-07-11/12,
/// `docs/FIM_CONVERGENCE_WORKLOG.md` "checkpoint D2"/"checkpoint D5")
/// source-traced this exactly and fixed a confirmed off-by-one that
/// previously read `1` here (letting acceptance fire one iteration too
/// early on fast-converging cases) — verified isolated to `OpmAligned`
/// (bounded-case control matrix bit-identical on the Legacy/flag-off path,
/// which never reads this constant).
const OPM_NEWTON_MIN_ITERATION_INDEX: usize = 2;
/// OPM `relaxed-linear-solver-reduction` default: a linear solve that didn't fully converge
/// but reduced the residual by at least this factor relative to `rhs_norm` (x0=0 so
/// r0=rhs, design doc §9.5) is accepted with a warning rather than triggering a fallback.
const OPM_RELAXED_LINEAR_SOLVER_REDUCTION: f64 = 0.01;

/// OPM's relaxed linear-solver criterion applies to the returned correction's actual residual
/// reduction, not to a backend-specific failure classification. In particular, direct LU has no
/// iterative failure payload but can still return a finite, useful non-strict correction.
fn opm_accepts_relaxed_linear_report(report: &FimLinearSolveReport) -> bool {
    report.solution.iter().all(|value| value.is_finite())
        && report.reduction().is_finite()
        && report.reduction() < OPM_RELAXED_LINEAR_SOLVER_REDUCTION
}

/// Bundle N checkpoint 1 (read-only): OPM-style CNV/MB convergence measures computed from the
/// RAW (unscaled) residual, mirroring `BlackoilModel::getReservoirConvergence` /
/// `getMaxCoeff` / `characteriseCnvPvSplit`. ResSim's residual is already dt-integrated
/// (surface m³), unlike OPM's rate residual, so OPM's `* dt` factor is intentionally absent —
/// `CNV = B_avg * max_i(|R_i|/pv_i)` here is dimensionally identical to OPM's
/// `B_avg * dt * max_i(|R_rate_i|/pv_i)`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CnvMbDiagnostics {
    /// Per component (water, oil-component, gas-component): field-average FVF times the
    /// worst per-cell pore-volume-normalized residual. Dimensionless local error.
    cnv: [f64; 3],
    /// Per component: |B_avg * signed residual sum| / total pore volume. Dimensionless
    /// global mass-balance error (cancellation across cells intended).
    mb: [f64; 3],
    /// Fraction of total pore volume held by cells whose own worst-component CNV exceeds
    /// the strict tolerance.
    violating_pv_fraction: f64,
    /// OPM's `relaxed-max-pv-fraction` rule: violating PV under 3% lets the whole CNV check
    /// run at the relaxed tolerance this iteration.
    pv_rule_relaxes: bool,
    /// All components pass strict CNV and strict MB.
    would_accept_strict: bool,
    /// Effective accept condition at the CURRENT iteration and its `relax_final_iteration`
    /// flag: strict CNV/MB, or CNV via the 3%-PV relaxed tier, or (only when
    /// `relax_final_iteration` was passed in) OPM's unconditional final-iteration relaxed
    /// MB/CNV tolerances (design doc §9.1's `relax_final_iteration_mb`/`_cnv`).
    would_accept: bool,
    /// Per component, the cell with the largest `|r_i,c|` among cells whose sign matches the
    /// summed residual `r_sum[c]` (i.e. the cell actually driving the MB imbalance, not one
    /// that partly cancels against it). `FIM-DIAG-003` D0 instrumentation.
    mb_peak_cell: [usize; 3],
    /// Per component, the cell with the largest scaled CNV coefficient (`|r_i,c| * B_avg[c] /
    /// pv_i`, the same quantity `cnv[c]` is the max of). `FIM-DIAG-003` D0 instrumentation.
    cnv_peak_cell: [usize; 3],
    /// The single failing criterion with the largest `value / effective_tolerance` ratio, or
    /// `None` when `would_accept`. `FIM-DIAG-003` D0 instrumentation — names which criterion
    /// blocks acceptance so the trace line doesn't require reading six numbers by hand.
    binding: Option<BindingCriterion>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum BindingCriterionKind {
    Cnv,
    Mb,
}

impl BindingCriterionKind {
    fn label(self) -> &'static str {
        match self {
            Self::Cnv => "cnv",
            Self::Mb => "mb",
        }
    }
}

const RESIDUAL_COMPONENT_LABELS: [&str; 3] = ["water", "oil", "gas"];

#[derive(Clone, Copy, Debug, PartialEq)]
struct BindingCriterion {
    kind: BindingCriterionKind,
    component: usize,
    peak_cell: usize,
    value: f64,
    tolerance: f64,
}

impl BindingCriterion {
    fn trace_string(&self) -> String {
        format!(
            "{}[{}]={:.3e}/{:.3e} cell={}",
            self.kind.label(),
            RESIDUAL_COMPONENT_LABELS[self.component],
            self.value,
            self.tolerance,
            self.peak_cell,
        )
    }
}

/// Pure core of the CNV/MB computation, split out for direct unit testing.
/// `fvf_per_cell[i] = [B_w, B_o, B_g]` (reservoir m³ per surface m³) for cell `i`;
/// `pore_volumes_m3` are REFERENCE pore volumes (porosity * bulk volume, no compressibility
/// factor), matching OPM's `referencePorosity * dofTotalVolume`. `relax_final_iteration`
/// mirrors OPM's `iteration == maxIter && min_strict_{mb,cnv}_iter == -1` triggers (the
/// shipped defaults — `min_strict_mb_iter`/`min_strict_cnv_iter` are not modeled since they
/// default to "off"): true unconditionally applies `tolerance-mb-relaxed`/`tolerance-cnv-relaxed`
/// to the WHOLE check, independent of the 3%-PV rule below.
fn cnv_mb_from_parts(
    residual: &DVector<f64>,
    pore_volumes_m3: &[f64],
    fvf_per_cell: &[[f64; 3]],
    relax_final_iteration: bool,
) -> CnvMbDiagnostics {
    let n_cells = pore_volumes_m3.len();

    let mut b_avg = [0.0_f64; 3];
    for fvf in fvf_per_cell {
        for c in 0..3 {
            b_avg[c] += fvf[c];
        }
    }
    for avg in &mut b_avg {
        *avg /= n_cells.max(1) as f64;
    }

    let mut max_coeff = [0.0_f64; 3];
    let mut cnv_peak_cell = [0usize; 3];
    let mut r_sum = [0.0_f64; 3];
    let mut pv_sum = 0.0_f64;
    let mut violating_pv = 0.0_f64;
    for i in 0..n_cells {
        let pv = pore_volumes_m3[i].max(1e-9);
        pv_sum += pv;
        let mut cell_max_cnv = 0.0_f64;
        for c in 0..3 {
            let r = residual[i * 3 + c];
            r_sum[c] += r;
            let coeff = r.abs() / pv;
            if coeff > max_coeff[c] {
                max_coeff[c] = coeff;
                cnv_peak_cell[c] = i;
            }
            cell_max_cnv = cell_max_cnv.max(r.abs() * b_avg[c] / pv);
        }
        if cell_max_cnv > OPM_TOLERANCE_CNV {
            violating_pv += pv;
        }
    }

    let mut cnv = [0.0_f64; 3];
    let mut mb = [0.0_f64; 3];
    for c in 0..3 {
        cnv[c] = b_avg[c] * max_coeff[c];
        mb[c] = (b_avg[c] * r_sum[c]).abs() / pv_sum.max(1e-9);
    }

    // Peak-contributing cell per component for MB: the largest |r_i,c| among cells whose sign
    // agrees with the summed imbalance r_sum[c] — the cell(s) actually driving the mass-balance
    // error rather than one that cancels against it. `FIM-DIAG-003` D0 instrumentation.
    let mut mb_peak_cell = [0usize; 3];
    let mut mb_peak_abs = [0.0_f64; 3];
    for c in 0..3 {
        let sign = r_sum[c].signum();
        for i in 0..n_cells {
            let r = residual[i * 3 + c];
            if r.signum() == sign && r.abs() > mb_peak_abs[c] {
                mb_peak_abs[c] = r.abs();
                mb_peak_cell[c] = i;
            }
        }
    }

    let violating_pv_fraction = violating_pv / pv_sum.max(1e-9);
    let pv_rule_relaxes = violating_pv < OPM_RELAXED_MAX_PV_FRACTION * pv_sum;
    let mb_ok = mb.iter().all(|&v| v <= OPM_TOLERANCE_MB);
    let cnv_strict_ok = cnv.iter().all(|&v| v <= OPM_TOLERANCE_CNV);
    let mb_tol_effective = if relax_final_iteration {
        OPM_TOLERANCE_MB_RELAXED
    } else {
        OPM_TOLERANCE_MB
    };
    let cnv_tol_effective = if relax_final_iteration {
        OPM_TOLERANCE_CNV_RELAXED
    } else {
        OPM_TOLERANCE_CNV
    };
    let cnv_component_ok: [bool; 3] = std::array::from_fn(|c| {
        cnv[c] <= cnv_tol_effective || (pv_rule_relaxes && cnv[c] <= OPM_TOLERANCE_CNV_RELAXED)
    });
    let mb_component_ok: [bool; 3] = std::array::from_fn(|c| mb[c] <= mb_tol_effective);
    let cnv_effective_ok = cnv_component_ok.iter().all(|&ok| ok);
    let mb_effective_ok = mb_component_ok.iter().all(|&ok| ok);

    // Binding criterion: among the failing components, the one with the largest
    // value/tolerance overshoot ratio. `FIM-DIAG-003` D0 — names which criterion blocks
    // acceptance without requiring a human to compare six numbers against two tolerances.
    let mut binding: Option<BindingCriterion> = None;
    let mut worst_ratio = 1.0_f64;
    for c in 0..3 {
        if !cnv_component_ok[c] {
            let ratio = cnv[c] / cnv_tol_effective.max(1e-300);
            if ratio > worst_ratio {
                worst_ratio = ratio;
                binding = Some(BindingCriterion {
                    kind: BindingCriterionKind::Cnv,
                    component: c,
                    peak_cell: cnv_peak_cell[c],
                    value: cnv[c],
                    tolerance: cnv_tol_effective,
                });
            }
        }
        if !mb_component_ok[c] {
            let ratio = mb[c] / mb_tol_effective.max(1e-300);
            if ratio > worst_ratio {
                worst_ratio = ratio;
                binding = Some(BindingCriterion {
                    kind: BindingCriterionKind::Mb,
                    component: c,
                    peak_cell: mb_peak_cell[c],
                    value: mb[c],
                    tolerance: mb_tol_effective,
                });
            }
        }
    }

    CnvMbDiagnostics {
        cnv,
        mb,
        violating_pv_fraction,
        pv_rule_relaxes,
        would_accept_strict: cnv_strict_ok && mb_ok,
        would_accept: cnv_effective_ok && mb_effective_ok,
        mb_peak_cell,
        cnv_peak_cell,
        binding,
    }
}

/// OPM shipped per-cell update-chopping limits (Flow 2025.10 defaults `--ds-max` /
/// `--dp-max-rel`, verified from the installed binary and `blackoilnewtonmethod.hpp` at the
/// pinned tag — `docs/FIM_BUNDLE_N_DESIGN.md` §9.2). Used by the `OpmAligned` nonlinear flavor.
const OPM_DS_MAX: f64 = 0.2;
const OPM_DP_MAX_REL: f64 = 0.3;
/// OPM shipped well-BHP update-chopping limit (`--dbhp-max-rel`, default `1.0`, verified from
/// the installed binary and `StandardWellPrimaryVariables.cpp::updateNewton` at the pinned
/// tag — Bundle N §5 follow-up, worklog "Bundle N §5 end-metric evaluation (2026-07-09)").
/// OPM clamps the ABSOLUTE BHP delta to at most this fraction of the CURRENT BHP magnitude
/// (`dx = sign(dx) * min(|dx|, |bhp_current| * dBHPLimit)`), then floors the result just above
/// zero (`bhp_lower_limit = 1 bar - 1 Pa`, i.e. effectively `>= 1.0` bar here). OPM does NOT
/// clamp well RATE (`WQTotal`) magnitude at all — only a post-hoc sign-consistency check
/// (injector can't produce, producer can't inject) — so ResSim's perforation-rate deltas stay
/// unchopped, matching OPM's own choice rather than inventing a new limit it doesn't have.
const OPM_DBHP_MAX_REL: f64 = 1.0;
const OPM_BHP_LOWER_LIMIT_BAR: f64 = 1.0;

/// Bundle N checkpoint 2 (N2, `OpmAligned` flavor only): OPM's per-cell update chopping,
/// ported from `updatePrimaryVariables_` (`blackoilnewtonmethod.hpp`, design doc §9.2).
/// Replaces the Legacy global damping scalar: each cell's own saturation deltas are scaled by
/// that cell's `satAlpha = dsMax / maxSatDelta` (including the IMPLIED oil delta
/// `dSo = -(dSw + dSg)`), and its pressure delta is clamped to `±dpMaxRel * p_current` —
/// no cell restricts any other cell's movement. Matching OPM's composition order, the global
/// oscillation-relaxation scalar (`dampen` mode, Phase 7) multiplies the RAW update first,
/// then the chop applies per cell.
///
/// Sign convention: ResSim applies `next = current + update` (OPM uses `current - delta`);
/// the chop is symmetric in sign so only the Rs non-negativity guard direction differs.
///
/// Bundle N §5 follow-up (worklog "Bundle N §5 end-metric evaluation (2026-07-09)"): the
/// well-BHP tail entry is now chopped too, matching OPM's `dbhp-max-rel` exactly — added after
/// the heavy case's §5 failure traced to a producer pinned at its BHP limit whose raw,
/// previously-unchopped BHP update oscillated each iteration, perturbing the coupled
/// reservoir residual via the shared linear solve and stalling its own MB convergence for
/// ~20 iterations per substep. Perforation-rate entries stay unchopped, matching OPM's own
/// choice not to limit well rate (`WQTotal`) magnitude. ResSim's Schur-recovered well state is
/// still post-processed by `relax_well_state_toward_local_consistency` after application.
fn opm_per_cell_chopped_update(
    state: &FimState,
    update: &DVector<f64>,
    relaxation: f64,
) -> DVector<f64> {
    let mut chopped = update * relaxation;
    for (well_idx, &bhp_bar) in state.well_bhp.iter().enumerate() {
        let offset = state.well_bhp_unknown_offset(well_idx);
        let dbhp = chopped[offset];
        let dbhp_cap = OPM_DBHP_MAX_REL * bhp_bar.abs();
        let dbhp_limited = if dbhp.abs() > dbhp_cap {
            dbhp.signum() * dbhp_cap
        } else {
            dbhp
        };
        chopped[offset] = if bhp_bar + dbhp_limited < OPM_BHP_LOWER_LIMIT_BAR {
            OPM_BHP_LOWER_LIMIT_BAR - bhp_bar
        } else {
            dbhp_limited
        };
    }
    for (idx, cell) in state.cells.iter().enumerate() {
        let offset = idx * 3;
        let dp = chopped[offset];
        let dsw = chopped[offset + 1];
        let dhc = chopped[offset + 2];

        // Saturation deltas, including the implied oil delta (design doc §9.2: OPM counts
        // dSo = -(dSw + dSg) toward the per-cell max even though So is not a primary var).
        let (dsg, dso) = match cell.regime {
            HydrocarbonState::Saturated => (dhc, -(dsw + dhc)),
            HydrocarbonState::Undersaturated => (0.0, -dsw),
        };
        let max_sat_delta = dsw.abs().max(dso.abs()).max(dsg.abs());
        let sat_alpha = if max_sat_delta > OPM_DS_MAX {
            OPM_DS_MAX / max_sat_delta
        } else {
            1.0
        };
        chopped[offset + 1] = dsw * sat_alpha;
        match cell.regime {
            HydrocarbonState::Saturated => {
                chopped[offset + 2] = dhc * sat_alpha;
            }
            HydrocarbonState::Undersaturated => {
                // hydrocarbon_var means Rs: not a saturation, so no satAlpha — only OPM's
                // guard that the R factor cannot go negative after the update.
                if cell.hydrocarbon_var + chopped[offset + 2] < 0.0 {
                    chopped[offset + 2] = -cell.hydrocarbon_var;
                }
            }
        }

        // Pressure: relative clamp, independent of satAlpha.
        let dp_cap = OPM_DP_MAX_REL * cell.pressure_bar.abs();
        if dp.abs() > dp_cap {
            chopped[offset] = dp.signum() * dp_cap;
        }
    }
    chopped
}

/// Sim-facing wrapper: extracts reference pore volumes and per-cell FVFs, then delegates to
/// `cnv_mb_from_parts`. Only the cell rows of `residual` are read; well/perforation rows are
/// excluded exactly as in OPM (wells have their own `tolerance-wells` criterion).
fn cnv_mb_diagnostics(
    sim: &ReservoirSimulator,
    state: &FimState,
    residual: &DVector<f64>,
    relax_final_iteration: bool,
) -> CnvMbDiagnostics {
    let n_cells = state.cells.len();
    let mut pore_volumes = Vec::with_capacity(n_cells);
    let mut fvf = Vec::with_capacity(n_cells);
    let b_w = sim.b_w.max(1e-9);
    for idx in 0..n_cells {
        pore_volumes.push(sim.pore_volume_m3(idx));
        let pressure_bar = state.cells[idx].pressure_bar;
        fvf.push([
            b_w,
            sim.get_b_o_cell(idx, pressure_bar).max(1e-9),
            sim.get_b_g(pressure_bar).max(1e-9),
        ]);
    }
    cnv_mb_from_parts(residual, &pore_volumes, &fvf, relax_final_iteration)
}

fn residual_family_diagnostics(
    residual: &DVector<f64>,
    scaling: &crate::fim::scaling::EquationScaling,
) -> ResidualFamilyDiagnostics {
    let n_cells = scaling.water.len();
    let mut water = None;
    let mut oil_component = None;
    let mut gas_component = None;
    let mut well_constraint = None;
    let mut perforation_flow = None;

    for i in 0..n_cells {
        update_family_peak(
            &mut water,
            ResidualRowFamily::Water,
            residual[i * 3].abs() / scaling.water[i],
            i * 3,
            i,
        );
        update_family_peak(
            &mut oil_component,
            ResidualRowFamily::OilComponent,
            residual[i * 3 + 1].abs() / scaling.oil_component[i],
            i * 3 + 1,
            i,
        );
        update_family_peak(
            &mut gas_component,
            ResidualRowFamily::GasComponent,
            residual[i * 3 + 2].abs() / scaling.gas_component[i],
            i * 3 + 2,
            i,
        );
    }

    let mut offset = n_cells * 3;
    for i in 0..scaling.well_constraint.len() {
        update_family_peak(
            &mut well_constraint,
            ResidualRowFamily::WellConstraint,
            residual[offset + i].abs() / scaling.well_constraint[i],
            offset + i,
            i,
        );
    }
    offset += scaling.well_constraint.len();
    for i in 0..scaling.perforation_flow.len() {
        update_family_peak(
            &mut perforation_flow,
            ResidualRowFamily::PerforationFlow,
            residual[offset + i].abs() / scaling.perforation_flow[i],
            offset + i,
            i,
        );
    }

    let water = water.expect("residual diagnostics require at least one cell");
    let oil_component = oil_component.expect("residual diagnostics require at least one cell");
    let gas_component = gas_component.expect("residual diagnostics require at least one cell");
    let mut global = water;
    for peak in [
        Some(oil_component),
        Some(gas_component),
        well_constraint,
        perforation_flow,
    ]
    .into_iter()
    .flatten()
    {
        if peak.scaled_value > global.scaled_value {
            global = peak;
        }
    }

    ResidualFamilyDiagnostics {
        water,
        oil_component,
        gas_component,
        well_constraint,
        perforation_flow,
        global,
    }
}

fn residual_family_trace(diagnostics: &ResidualFamilyDiagnostics) -> String {
    let mut parts = vec![
        format!(
            "water={:.3e}@cell{}",
            diagnostics.water.scaled_value, diagnostics.water.item_index
        ),
        format!(
            "oil={:.3e}@cell{}",
            diagnostics.oil_component.scaled_value, diagnostics.oil_component.item_index
        ),
        format!(
            "gas={:.3e}@cell{}",
            diagnostics.gas_component.scaled_value, diagnostics.gas_component.item_index
        ),
    ];
    if let Some(peak) = diagnostics.well_constraint {
        parts.push(format!(
            "well={:.3e}@well{}",
            peak.scaled_value, peak.item_index
        ));
    }
    if let Some(peak) = diagnostics.perforation_flow {
        parts.push(format!(
            "perf={:.3e}@perf{}",
            peak.scaled_value, peak.item_index
        ));
    }
    parts.push(format!(
        "top={} row={} item={}",
        diagnostics.global.family.label(),
        diagnostics.global.row,
        diagnostics.global.item_index
    ));
    parts.join(" ")
}

fn normalized_material_balance(component_sum: f64, component_scaling: &[f64]) -> f64 {
    let denominator = component_scaling
        .iter()
        .copied()
        .sum::<f64>()
        .abs()
        .max(1.0);
    component_sum.abs() / denominator
}

fn global_material_balance_diagnostics(
    residual: &DVector<f64>,
    scaling: &crate::fim::scaling::EquationScaling,
) -> GlobalMaterialBalanceDiagnostics {
    let n_cells = scaling.water.len();
    let mut water_sum = 0.0_f64;
    let mut oil_component_sum = 0.0_f64;
    let mut gas_component_sum = 0.0_f64;

    for i in 0..n_cells {
        water_sum += residual[i * 3];
        oil_component_sum += residual[i * 3 + 1];
        gas_component_sum += residual[i * 3 + 2];
    }

    let water = normalized_material_balance(water_sum, &scaling.water);
    let oil_component = normalized_material_balance(oil_component_sum, &scaling.oil_component);
    let gas_component = normalized_material_balance(gas_component_sum, &scaling.gas_component);

    let mut global_family = ResidualRowFamily::Water;
    let mut global_value = water;
    for (family, value) in [
        (ResidualRowFamily::OilComponent, oil_component),
        (ResidualRowFamily::GasComponent, gas_component),
    ] {
        if value > global_value {
            global_family = family;
            global_value = value;
        }
    }

    GlobalMaterialBalanceDiagnostics {
        water,
        oil_component,
        gas_component,
        global_family,
        global_value,
    }
}

fn global_material_balance_trace(diagnostics: &GlobalMaterialBalanceDiagnostics) -> String {
    format!(
        "water={:.3e} oil={:.3e} gas={:.3e} top={}",
        diagnostics.water,
        diagnostics.oil_component,
        diagnostics.gas_component,
        diagnostics.global_family.label(),
    )
}

fn cell_index_to_ijk(sim: &ReservoirSimulator, cell_idx: usize) -> (usize, usize, usize) {
    let cells_per_layer = sim.nx * sim.ny;
    let k = cell_idx / cells_per_layer;
    let in_layer = cell_idx % cells_per_layer;
    let j = in_layer / sim.nx;
    let i = in_layer % sim.nx;
    (i, j, k)
}

fn format_phase_flux_diagnostic(
    sim: &ReservoirSimulator,
    label: &str,
    diagnostic: &PhaseFluxDiagnostic,
) -> String {
    let (i, j, k) = cell_index_to_ijk(sim, diagnostic.upwind_cell_idx);
    format!(
        "{}(dphi={:.3e},up=({}, {}, {}),mob={:.3e},flux={:.3e})",
        label, diagnostic.dphi, i, j, k, diagnostic.mobility, diagnostic.flux,
    )
}

fn format_face_phase_diagnostics(
    sim: &ReservoirSimulator,
    label: &str,
    diagnostics: Option<&FacePhaseDiagnostics>,
) -> String {
    match diagnostics {
        Some(face) => format!(
            "{}=[{} {} {}]",
            label,
            format_phase_flux_diagnostic(sim, "w", &face.water),
            format_phase_flux_diagnostic(sim, "o", &face.oil),
            format_phase_flux_diagnostic(sim, "g", &face.gas),
        ),
        None => format!("{}=[boundary]", label),
    }
}

fn format_cell_face_phase_diagnostics(
    sim: &ReservoirSimulator,
    diagnostics: &CellFacePhaseDiagnostics,
) -> String {
    [
        format_face_phase_diagnostics(sim, "x-", diagnostics.x_minus.as_ref()),
        format_face_phase_diagnostics(sim, "x+", diagnostics.x_plus.as_ref()),
        format_face_phase_diagnostics(sim, "y-", diagnostics.y_minus.as_ref()),
        format_face_phase_diagnostics(sim, "y+", diagnostics.y_plus.as_ref()),
    ]
    .join(" ")
}

fn cell_residual_detail_trace(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    topology: &crate::fim::wells::FimWellTopology,
    dt_days: f64,
    peak: &ResidualFamilyPeak,
) -> Option<String> {
    let cell_idx = peak.item_index;
    if cell_idx >= state.cells.len() {
        return None;
    }
    let (i, j, k) = cell_index_to_ijk(sim, cell_idx);
    let cell = state.cell(cell_idx);
    let derived = state.derive_cell(sim, cell_idx);
    let equation = match peak.family {
        ResidualRowFamily::Water => "water",
        ResidualRowFamily::OilComponent => "oil",
        ResidualRowFamily::GasComponent => "gas",
        _ => return None,
    };
    let component = match peak.family {
        ResidualRowFamily::Water => 0,
        ResidualRowFamily::OilComponent => 1,
        ResidualRowFamily::GasComponent => 2,
        _ => return None,
    };
    let breakdown = cell_equation_residual_breakdown(
        sim,
        previous_state,
        state,
        topology,
        dt_days,
        cell_idx,
        component,
    )?;
    let face_diagnostics = cell_face_phase_flux_diagnostics(sim, state, dt_days, cell_idx)?;

    Some(format!(
        "eq={} cell{}=({}, {}, {}) p={:.3} sw={:.4} so={:.4} sg={:.4} rs={:.4} regime={:?} accum={:.3e} x-={:.3e} x+={:.3e} y-={:.3e} y+={:.3e} z-={:.3e} z+={:.3e} well={:.3e} total={:.3e} faces={}",
        equation,
        cell_idx,
        i,
        j,
        k,
        cell.pressure_bar,
        cell.sw,
        derived.so,
        derived.sg,
        derived.rs,
        cell.regime,
        breakdown.accumulation,
        breakdown.x_minus,
        breakdown.x_plus,
        breakdown.y_minus,
        breakdown.y_plus,
        breakdown.z_minus,
        breakdown.z_plus,
        breakdown.well_source,
        breakdown.total,
        format_cell_face_phase_diagnostics(sim, &face_diagnostics),
    ))
}

fn well_constraint_detail_trace(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &crate::fim::wells::FimWellTopology,
    peak: &ResidualFamilyPeak,
) -> Option<String> {
    let well_idx = peak.item_index;
    let well_topology = topology.wells.get(well_idx)?;
    let representative = &sim.wells[well_topology.representative_well_index];
    let control = physical_well_control(sim, topology, well_idx);
    let decision = if !control.enabled {
        "disabled".to_string()
    } else if control.rate_controlled {
        if control.uses_surface_target {
            "rate(surface)".to_string()
        } else {
            "rate(reservoir)".to_string()
        }
    } else {
        "bhp".to_string()
    };

    Some(format!(
        "well{} id={} inj={} head=({}, {}) bhp={:.3} mode={} target={} bhp_limit={:.3} nperf={}",
        well_idx,
        representative
            .physical_well_id
            .as_deref()
            .unwrap_or("<legacy>"),
        well_topology.injector,
        well_topology.head_i,
        well_topology.head_j,
        state
            .well_bhp
            .get(well_idx)
            .copied()
            .unwrap_or(representative.bhp),
        decision,
        control
            .target_rate
            .map(|value| format!("{:.3e}", value))
            .unwrap_or_else(|| "none".to_string()),
        control.bhp_limit,
        well_topology.perforation_indices.len(),
    ))
}

fn residual_family_detail_trace(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    topology: &crate::fim::wells::FimWellTopology,
    dt_days: f64,
    diagnostics: &ResidualFamilyDiagnostics,
) -> Option<String> {
    match diagnostics.global.family {
        ResidualRowFamily::Water
        | ResidualRowFamily::OilComponent
        | ResidualRowFamily::GasComponent => cell_residual_detail_trace(
            sim,
            previous_state,
            state,
            topology,
            dt_days,
            &diagnostics.global,
        ),
        ResidualRowFamily::WellConstraint => {
            well_constraint_detail_trace(sim, state, topology, &diagnostics.global)
        }
        ResidualRowFamily::PerforationFlow => {
            let detail = perforation_local_block(topology, state, diagnostics.global.item_index)
                .residual_diagnostics(sim)?;
            let mut parts = vec![
                format!(
                    "perf{} well{} inj={} q={:.3e} conn={:.3e} raw={:.3e}",
                    detail.perf_idx,
                    detail.physical_well_idx,
                    detail.injector,
                    detail.q_unknown_m3_day,
                    detail.q_connection_m3_day,
                    detail.raw_connection_m3_day,
                ),
                format!(
                    "wi={:.3e} mob={:.3e} draw={:.3e} p={:.3} bhp={:.3}",
                    detail.well_index,
                    detail.connection_mobility,
                    detail.drawdown_bar,
                    detail.cell_pressure_bar,
                    detail.bhp_bar,
                ),
            ];
            if let Some(surface_rate) = detail.surface_rate_unknown_sc_day {
                parts.push(format!("surf_q={:.3e}", surface_rate));
            }
            if let Some(target_rate) = detail.target_rate_sc_day {
                parts.push(format!("target={:.3e}", target_rate));
            }
            if let Some(actual_rate) = detail.actual_well_rate_sc_day {
                parts.push(format!("well_rate={:.3e}", actual_rate));
            }
            if let Some(bhp_slack) = detail.bhp_slack {
                parts.push(format!("bhp_slack={:.3e}", bhp_slack));
            }
            if let Some(rate_slack) = detail.rate_slack {
                parts.push(format!("rate_slack={:.3e}", rate_slack));
            }
            if let Some(frozen_bhp) = detail.frozen_consistent_bhp_bar {
                parts.push(format!("frozen_bhp={:.3}", frozen_bhp));
            }
            if let Some(frozen_q) = detail.frozen_consistent_perf_rate_m3_day {
                parts.push(format!(
                    "frozen_q={:.3e} dq={:.3e}",
                    frozen_q,
                    detail.q_unknown_m3_day - frozen_q,
                ));
            }
            if let Some(frozen_rate) = detail.frozen_consistent_well_rate_sc_day {
                parts.push(format!("frozen_well_rate={:.3e}", frozen_rate));
            }
            if let Some(frozen_limited) = detail.frozen_consistent_bhp_limited {
                parts.push(format!("frozen_bhp_limited={}", frozen_limited));
            }
            Some(parts.join(" "))
        }
    }
}

fn evaluate_accepted_state_convergence(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    candidate_state: &FimState,
    topology: &crate::fim::wells::FimWellTopology,
    dt_days: f64,
) -> AcceptedStateConvergenceDiagnostics {
    let mut state = candidate_state.clone();
    state.classify_regimes(sim);

    let assembly = assemble_fim_system(
        sim,
        previous_state,
        &state,
        &FimAssemblyOptions {
            dt_days,
            include_wells: true,
            assemble_residual_only: true,
            topology: Some(topology),
        },
    );
    let residual_inf_norm =
        scaled_residual_inf_norm(&assembly.residual, &assembly.equation_scaling);
    let residual_diagnostics =
        residual_family_diagnostics(&assembly.residual, &assembly.equation_scaling);
    let residual_detail = residual_family_detail_trace(
        sim,
        previous_state,
        &state,
        topology,
        dt_days,
        &residual_diagnostics,
    );
    let material_balance_diagnostics =
        global_material_balance_diagnostics(&assembly.residual, &assembly.equation_scaling);

    AcceptedStateConvergenceDiagnostics {
        state,
        residual_inf_norm,
        residual_diagnostics,
        residual_detail,
        material_balance_inf_norm: material_balance_diagnostics.global_value,
        material_balance_diagnostics,
    }
}

fn convergence_limits(options: &FimNewtonOptions, use_guard_band: bool) -> (f64, f64) {
    let factor = if use_guard_band {
        ENTRY_RESIDUAL_GUARD_FACTOR
    } else {
        1.0
    };
    (
        options.residual_tolerance * factor,
        options.material_balance_tolerance,
    )
}

fn accepted_state_meets_convergence(
    diagnostics: &AcceptedStateConvergenceDiagnostics,
    residual_limit: f64,
    material_balance_limit: f64,
) -> bool {
    diagnostics.residual_inf_norm <= residual_limit
        && diagnostics.material_balance_inf_norm <= material_balance_limit
}

fn zero_move_appleyard_acceptance_allows(
    materially_changed: bool,
    residual_inf_norm: f64,
    material_balance_inf_norm: f64,
    options: &FimNewtonOptions,
) -> bool {
    if materially_changed {
        return false;
    }

    residual_inf_norm
        <= options.residual_tolerance * NOOP_ENTRY_EXACT_FACTOR * ENTRY_RESIDUAL_GUARD_FACTOR
        && material_balance_inf_norm
            <= options.material_balance_tolerance
                * NOOP_ENTRY_EXACT_FACTOR
                * ENTRY_RESIDUAL_GUARD_FACTOR
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StagnationAcceptanceGateStatus {
    materially_changed: bool,
    update_ok: bool,
    residual_ok: bool,
    material_balance_ok: bool,
}

impl StagnationAcceptanceGateStatus {
    fn allows(self) -> bool {
        self.materially_changed && self.update_ok && self.residual_ok && self.material_balance_ok
    }
}

fn stagnation_acceptance_gate_status(
    materially_changed: bool,
    residual_inf_norm: f64,
    material_balance_inf_norm: f64,
    update_inf_norm: f64,
    options: &FimNewtonOptions,
) -> StagnationAcceptanceGateStatus {
    StagnationAcceptanceGateStatus {
        materially_changed,
        update_ok: update_inf_norm <= options.update_tolerance,
        residual_ok: residual_inf_norm
            <= options.residual_tolerance * NONLINEAR_HISTORY_RESIDUAL_BAND_FACTOR,
        material_balance_ok: material_balance_inf_norm <= options.material_balance_tolerance,
    }
}

fn stagnation_acceptance_gate_trace(
    status: StagnationAcceptanceGateStatus,
    residual_inf_norm: f64,
    material_balance_inf_norm: f64,
    update_inf_norm: f64,
    options: &FimNewtonOptions,
) -> String {
    format!(
        " gates=[changed={} upd={:.3e}/{:.3e} {} res={:.3e}/{:.3e} {} mb={:.3e}/{:.3e} {}]",
        status.materially_changed,
        update_inf_norm,
        options.update_tolerance,
        if status.update_ok { "ok" } else { "reject" },
        residual_inf_norm,
        options.residual_tolerance * NONLINEAR_HISTORY_RESIDUAL_BAND_FACTOR,
        if status.residual_ok { "ok" } else { "reject" },
        material_balance_inf_norm,
        options.material_balance_tolerance,
        if status.material_balance_ok {
            "ok"
        } else {
            "reject"
        },
    )
}

fn stagnation_acceptance_allows(
    materially_changed: bool,
    residual_inf_norm: f64,
    material_balance_inf_norm: f64,
    update_inf_norm: f64,
    options: &FimNewtonOptions,
) -> bool {
    stagnation_acceptance_gate_status(
        materially_changed,
        residual_inf_norm,
        material_balance_inf_norm,
        update_inf_norm,
        options,
    )
    .allows()
}

pub(crate) fn run_fim_timestep(
    sim: &mut ReservoirSimulator,
    previous_state: &FimState,
    initial_iterate: &FimState,
    dt_days: f64,
    options: &FimNewtonOptions,
) -> FimStepReport {
    let total_timer = PerfTimer::start();
    let mut state = initial_iterate.clone();
    let mut last_linear_report = None;
    let mut final_residual_inf_norm: Option<f64>;
    let mut final_material_balance_inf_norm = f64::INFINITY;
    let mut final_update_inf_norm = f64::INFINITY;
    let mut prev_residual_norm = f64::INFINITY;
    let mut stagnation_count: u32 = 0;
    let mut stagnation_attrib_zero_move: u32 = 0;
    let mut stagnation_attrib_real_bump: u32 = 0;
    let mut stagnation_attrib_slow_decay: u32 = 0;
    // Fix-A2 Stage 1 residual-trend probe: track best-so-far and the residual at which
    // the current stagnation run started. Read-only diagnostic — used to emit STAG-TREND
    // and WOULD-WIDEN lines that classify whether the widened count=3 gate would fire.
    let mut best_residual_so_far = f64::INFINITY;
    let mut stagnation_entry_residual: Option<f64> = None;
    let mut previous_hotspot_site: Option<FimHotspotSite> = None;
    let mut repeated_hotspot_streak: u32 = 0;
    let mut assembly_ms = 0.0;
    let mut property_eval_ms = 0.0;
    let mut linear_solve_time_ms = 0.0;
    let mut linear_preconditioner_build_time_ms = 0.0;
    let mut state_update_ms = 0.0;
    let mut last_effective_update_inf_norm = f64::INFINITY;
    let mut last_effective_update_peak: Option<UpdateFamilyPeak> = None;
    let requested_linear_kind = options.linear.kind;
    let mut dead_state_direct_bypass = false;
    let mut restart_stagnation_direct_bypass = false;
    let mut restart_stagnation_fallback_streak = 0_u32;
    let mut zero_move_fallback_direct_bypass = false;
    let mut iterative_failed_last_iter = false;
    let mut previous_effective_move_floor_site: Option<FimHotspotSite> = None;
    // Phase 7 sub-phase 7.1: OPM-style oscillation-detection state, traced but not yet
    // folded into `damping` (see OSC-DETECT trace line below).
    let mut relaxation_state = RelaxationState::default();
    // Fix-B Stage 1 upwind-flip probe: snapshot of per-face upstream choices from the
    // previous Newton iteration. Compared against the current-iter snapshot to detect
    // saturation-front upwinding flips; read-only diagnostic.
    let mut previous_face_upwind_snapshot: Vec<FaceUpwindSample> = Vec::new();
    let block_layout = Some(FimLinearBlockLayout {
        cell_block_count: state.cells.len(),
        cell_block_size: 3,
        well_bhp_count: state.n_well_unknowns(),
        perforation_tail_start: state.n_cell_unknowns() + state.n_well_unknowns(),
    });
    let topology = build_well_topology(sim);

    fim_trace!(
        sim,
        options.verbose,
        "  Newton: dt={:.6} days, n_cells={}, n_wells={}, n_perfs={}, n_rows={}, req_lin={}, direct_thr={}",
        dt_days,
        state.cells.len(),
        state.n_well_unknowns(),
        state.n_perforation_unknowns(),
        state.n_unknowns(),
        options.linear.kind.label(),
        active_direct_solve_row_threshold()
    );

    let opm_aligned = options.nonlinear_flavor == FimNonlinearFlavor::OpmAligned;
    // Y2b3a: the existing native/default-off raw-state flag now selects the coupled,
    // deck-scoped OPM Sg/Rs primary-variable lifecycle. It remains unavailable in wasm and
    // does not alter Legacy behavior.
    #[cfg(not(target_arch = "wasm32"))]
    let y2b3_primary_variable_lifecycle =
        opm_aligned && std::env::var_os("FIM_Y2B_RAW_SATURATION").is_some();
    #[cfg(target_arch = "wasm32")]
    let y2b3_primary_variable_lifecycle = false;
    let mut primary_variables_switched = vec![false; state.cells.len()];

    if y2b3_primary_variable_lifecycle {
        fim_trace!(
            sim,
            options.verbose,
            "  Y2B3 OPM primary-variable lifecycle active (native/default-off; raw saturation plus per-iteration Sg/Rs adaptation)"
        );
    }

    for iteration in 0..options.max_newton_iterations {
        let assembly = assemble_fim_system(
            sim,
            previous_state,
            &state,
            &FimAssemblyOptions {
                dt_days,
                include_wells: true,
                assemble_residual_only: false,
                topology: Some(&topology),
            },
        );
        assembly_ms += assembly.timing.residual_ms
            + assembly.timing.sensitivity_eval_ms
            + assembly.timing.jacobian_ms;
        property_eval_ms += assembly.timing.property_eval_ms;
        final_residual_inf_norm = Some(scaled_residual_inf_norm(
            &assembly.residual,
            &assembly.equation_scaling,
        ));
        let residual_diagnostics =
            residual_family_diagnostics(&assembly.residual, &assembly.equation_scaling);
        let residual_detail = residual_family_detail_trace(
            sim,
            previous_state,
            &state,
            &topology,
            dt_days,
            &residual_diagnostics,
        );

        // Phase 7: OPM-style oscillation detection (sub-phase 7.1). The resulting
        // `relaxation_state.current_relaxation` scalar is folded into `damping` below
        // (sub-phase 7.2, at the `appleyard_damping_breakdown`/`history_stabilization`
        // composition site).
        {
            let current_family_norms = PerFamilyNorms::from_diagnostics(&residual_diagnostics);
            let osc_phase_count = detect_oscillation(
                current_family_norms,
                relaxation_state.residual_norm_1ago,
                relaxation_state.residual_norm_2ago,
            );
            relaxation_state.current_relaxation =
                next_relaxation_factor(relaxation_state.current_relaxation, osc_phase_count);
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: OSC-DETECT osc_phases={} relax={:.2}",
                iteration,
                osc_phase_count,
                relaxation_state.current_relaxation,
            );
            relaxation_state.residual_norm_2ago = relaxation_state.residual_norm_1ago;
            relaxation_state.residual_norm_1ago = current_family_norms;
            relaxation_state.history_len += 1;
        }

        // OPM's `newton-max-iterations` triggers its final-iteration relaxed MB/CNV tiers
        // (design doc §9.1) when this is the last attempt; `iters_remaining` (computed later
        // in the loop for a different purpose) is not yet in scope here, so recompute cheaply.
        let is_final_newton_iteration = iteration + 1 == options.max_newton_iterations;

        // Bundle N checkpoints 1+3: OPM CNV/MB criteria, computed unconditionally alongside
        // the existing inf-norm acceptance (trace-only under Legacy; the sole acceptance
        // decision under `OpmAligned` — see the `converged_on_entry` branch below).
        let opm_conv =
            cnv_mb_diagnostics(sim, &state, &assembly.residual, is_final_newton_iteration);
        fim_trace!(
            sim,
            options.verbose,
            "    iter {:>2}: CNV-MB cnv=[{:.3e},{:.3e},{:.3e}] mb=[{:.3e},{:.3e},{:.3e}] viol_pv={:.4} would_accept={} binding=[{}]",
            iteration,
            opm_conv.cnv[0],
            opm_conv.cnv[1],
            opm_conv.cnv[2],
            opm_conv.mb[0],
            opm_conv.mb[1],
            opm_conv.mb[2],
            opm_conv.violating_pv_fraction,
            if opm_conv.would_accept_strict {
                "strict"
            } else if opm_conv.would_accept {
                "pv-relaxed"
            } else {
                "no"
            },
            opm_conv
                .binding
                .as_ref()
                .map(BindingCriterion::trace_string)
                .unwrap_or_else(|| "none".to_string()),
        );

        let current_norm = final_residual_inf_norm.unwrap_or(f64::INFINITY);
        let previous_iteration_residual_norm = prev_residual_norm;
        let current_hotspot_site =
            residual_hotspot_site(sim, &topology, &residual_diagnostics.global);

        // Fix-A2 Stage 1 probe: update best-residual tracker every iter (read-only).
        if current_norm.is_finite() && current_norm < best_residual_so_far {
            best_residual_so_far = current_norm;
        }

        // Fix-B Stage 1 probe: upwind-flip snapshot diff against previous iter.
        // Read-only — emits an UPWIND-SUMMARY trace line per iter (from iter 1 onward).
        // Runs unconditionally (not gated on options.verbose) because `fim_trace!`
        // writes to the sim's internal trace buffer which is what the harness
        // exposes; the verbose flag only controls the stderr eprintln path.
        {
            let current_upwind_snapshot = collect_face_upwind_snapshot(sim, &state);
            let hotspot_cells: Vec<usize> = [
                previous_effective_move_floor_site,
                Some(current_hotspot_site),
                previous_hotspot_site,
            ]
            .into_iter()
            .flatten()
            .filter_map(|site| match site {
                FimHotspotSite::Cell(idx) => Some(idx),
                _ => None,
            })
            .collect();
            let flip_report = diff_face_upwind_snapshots(
                &previous_face_upwind_snapshot,
                &current_upwind_snapshot,
                &hotspot_cells,
                3,
            );
            if !previous_face_upwind_snapshot.is_empty() {
                let total_flips = flip_report.flips.iter().sum::<u32>();
                let total_hotspot = flip_report.hotspot_flips.iter().sum::<u32>();
                let sample_trace = if flip_report.samples.is_empty() {
                    String::new()
                } else {
                    let entries: Vec<String> = flip_report
                        .samples
                        .iter()
                        .map(|f| {
                            let phase_label = match f.phase {
                                0 => 'w',
                                1 => 'o',
                                _ => 'g',
                            };
                            format!(
                                "({} {}-{} {}:{:+.3e}->{:+.3e}{})",
                                phase_label,
                                f.id_i,
                                f.id_j,
                                f.dim,
                                f.dphi_prev,
                                f.dphi_curr,
                                if f.is_hotspot { " HS" } else { "" }
                            )
                        })
                        .collect();
                    format!(" samples=[{}]", entries.join(" "))
                };
                fim_trace!(
                    sim,
                    options.verbose,
                    "    iter {:>2}: UPWIND-SUMMARY flips_w={} flips_o={} flips_g={} total={} hotspot={} (hs_cells={:?}){}",
                    iteration,
                    flip_report.flips[0],
                    flip_report.flips[1],
                    flip_report.flips[2],
                    total_flips,
                    total_hotspot,
                    hotspot_cells,
                    sample_trace,
                );
            }
            previous_face_upwind_snapshot = current_upwind_snapshot;
        }
        repeated_hotspot_streak = repeated_nonlinear_hotspot_streak(
            sim,
            previous_hotspot_site,
            previous_iteration_residual_norm,
            &residual_diagnostics,
            current_hotspot_site,
            current_norm,
            repeated_hotspot_streak,
        );
        let repeated_zero_move_direct_bypass = should_enable_repeated_zero_move_direct_bypass(
            sim,
            previous_effective_move_floor_site,
            &residual_diagnostics,
            current_hotspot_site,
        );
        let materially_changed = iterate_has_material_change(previous_state, &state);
        // Bundle N checkpoint 3 (N1): under `OpmAligned`, acceptance is decided purely by
        // OPM's CNV/MB criteria (already computed above as `opm_conv`, including its
        // final-iteration relaxed tiers) gated on OPM's `newton-min-iterations` (design doc
        // §9.1) — replacing every Legacy entry-guard mechanism (`NOOP_ENTRY_EXACT_FACTOR`,
        // `ENTRY_RESIDUAL_GUARD_FACTOR`) for this decision.
        //
        // Bundle W (`docs/FIM_BUNDLE_W_PLAN.md` §5 item 3): when `nested_well_solve` is on,
        // AND in OPM's own well-convergence analog (`getWellConvergence`, W0 appendix G) —
        // closes N1's recorded fidelity gap (reservoir-only acceptance had no well check at
        // all). No-op when the flag is off: `wells_ok` is trivially `true`.
        let wells_ok = !opm_aligned
            || !options.nested_well_solve
            || crate::fim::wells_inner::all_wells_converged(
                sim,
                &state,
                &topology,
                &crate::fim::wells_inner::FimWellInnerSolveOptions::default(),
            );
        let converged_on_entry = if opm_aligned {
            iteration >= OPM_NEWTON_MIN_ITERATION_INDEX && opm_conv.would_accept && wells_ok
        } else if iteration == 0 && !materially_changed {
            current_norm <= options.residual_tolerance * NOOP_ENTRY_EXACT_FACTOR
        } else {
            current_norm <= options.residual_tolerance
                || (iteration == 0
                    && materially_changed
                    && current_norm <= options.residual_tolerance * ENTRY_RESIDUAL_GUARD_FACTOR)
        };
        if converged_on_entry && opm_aligned {
            // OPM-shaped accept: `opm_conv` was computed directly from the residual this
            // iteration already assembled — no re-assembly needed (matching OPM's own
            // "check convergence, then solve only if not converged" structure exactly,
            // unlike the Legacy path below which re-derives via a fresh residual-only
            // assembly for extra safety).
            final_update_inf_norm = 0.0;
            let mb_value =
                global_material_balance_diagnostics(&assembly.residual, &assembly.equation_scaling)
                    .global_value;
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: OPM-CONVERGED cnv=[{:.3e},{:.3e},{:.3e}] mb=[{:.3e},{:.3e},{:.3e}]{}",
                iteration,
                opm_conv.cnv[0],
                opm_conv.cnv[1],
                opm_conv.cnv[2],
                opm_conv.mb[0],
                opm_conv.mb[1],
                opm_conv.mb[2],
                if is_final_newton_iteration {
                    " (final-iteration relaxed tiers)"
                } else if opm_conv.pv_rule_relaxes && !opm_conv.would_accept_strict {
                    " (pv-relaxed)"
                } else {
                    ""
                },
            );
            return FimStepReport {
                accepted_state: state,
                converged: true,
                newton_iterations: iteration + 1,
                final_residual_inf_norm: current_norm,
                final_material_balance_inf_norm: mb_value,
                final_update_inf_norm,
                last_linear_report,
                accepted_hotspot_site: Some(current_hotspot_site),
                failure_diagnostics: None,
                retry_factor: 1.0,
                total_time_ms: total_timer.elapsed_ms(),
                assembly_ms,
                property_eval_ms,
                linear_solve_time_ms,
                linear_preconditioner_build_time_ms,
                state_update_ms,
            };
        }
        if converged_on_entry {
            final_update_inf_norm = 0.0;
            let use_guard_band = current_norm > options.residual_tolerance;
            let (residual_limit, material_balance_limit) =
                convergence_limits(options, use_guard_band);
            let accepted_diagnostics = evaluate_accepted_state_convergence(
                sim,
                previous_state,
                &state,
                &topology,
                dt_days,
            );
            let unchanged_entry_is_effectively_exact = iteration != 0
                || materially_changed
                || (accepted_diagnostics.residual_inf_norm
                    <= options.residual_tolerance * NOOP_ENTRY_EXACT_FACTOR
                    && accepted_diagnostics.material_balance_inf_norm
                        <= options.material_balance_tolerance * NOOP_ENTRY_EXACT_FACTOR);
            if unchanged_entry_is_effectively_exact
                && accepted_state_meets_convergence(
                    &accepted_diagnostics,
                    residual_limit,
                    material_balance_limit,
                )
            {
                fim_trace!(
                    sim,
                    options.verbose,
                    "    iter {:>2}: CONVERGED on residual check res={:.3e} mb={:.3e}{} fam=[{}] mb=[{}]{}",
                    iteration,
                    accepted_diagnostics.residual_inf_norm,
                    accepted_diagnostics.material_balance_inf_norm,
                    if use_guard_band {
                        format!(" (entry guard {:.1}x)", ENTRY_RESIDUAL_GUARD_FACTOR)
                    } else {
                        String::new()
                    },
                    residual_family_trace(&accepted_diagnostics.residual_diagnostics),
                    global_material_balance_trace(
                        &accepted_diagnostics.material_balance_diagnostics
                    ),
                    accepted_diagnostics
                        .residual_detail
                        .as_ref()
                        .map(|detail| format!(" detail=[{}]", detail))
                        .unwrap_or_default()
                );
                maybe_trace_small_dt_hotspot_neighborhood(
                    sim,
                    options.verbose,
                    "accepted",
                    dt_days,
                    previous_state,
                    &accepted_diagnostics.state,
                    residual_hotspot_site(
                        sim,
                        &topology,
                        &accepted_diagnostics.residual_diagnostics.global,
                    ),
                );
                return FimStepReport {
                    accepted_state: accepted_diagnostics.state,
                    converged: true,
                    newton_iterations: iteration + 1,
                    final_residual_inf_norm: accepted_diagnostics.residual_inf_norm,
                    final_material_balance_inf_norm: accepted_diagnostics.material_balance_inf_norm,
                    final_update_inf_norm,
                    last_linear_report,
                    accepted_hotspot_site: Some(residual_hotspot_site(
                        sim,
                        &topology,
                        &accepted_diagnostics.residual_diagnostics.global,
                    )),
                    failure_diagnostics: None,
                    retry_factor: 1.0,
                    total_time_ms: total_timer.elapsed_ms(),
                    assembly_ms,
                    property_eval_ms,
                    linear_solve_time_ms,
                    linear_preconditioner_build_time_ms,
                    state_update_ms,
                };
            }

            let failure_diagnostics = classify_retry_failure_with_site(
                last_linear_report.as_ref(),
                &accepted_diagnostics.residual_diagnostics,
                residual_hotspot_site(
                    sim,
                    &topology,
                    &accepted_diagnostics.residual_diagnostics.global,
                ),
            );
            let retry_factor = retry_factor_for_failure(Some(&failure_diagnostics));
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: POST-CLASSIFICATION REJECTED res={:.3e} mb={:.3e}{} fam=[{}] mb=[{}]{}{}",
                iteration,
                accepted_diagnostics.residual_inf_norm,
                accepted_diagnostics.material_balance_inf_norm,
                if use_guard_band {
                    format!(" (entry guard {:.1}x)", ENTRY_RESIDUAL_GUARD_FACTOR)
                } else {
                    String::new()
                },
                residual_family_trace(&accepted_diagnostics.residual_diagnostics),
                global_material_balance_trace(&accepted_diagnostics.material_balance_diagnostics),
                accepted_diagnostics
                    .residual_detail
                    .as_ref()
                    .map(|detail| format!(" detail=[{}]", detail))
                    .unwrap_or_default(),
                retry_failure_trace_suffix(&failure_diagnostics)
            );
            maybe_trace_small_dt_hotspot_neighborhood(
                sim,
                options.verbose,
                "rejected",
                dt_days,
                previous_state,
                &accepted_diagnostics.state,
                failure_diagnostics.hotspot_site,
            );
            return FimStepReport {
                accepted_state: accepted_diagnostics.state,
                converged: false,
                newton_iterations: iteration + 1,
                final_residual_inf_norm: accepted_diagnostics.residual_inf_norm,
                final_material_balance_inf_norm: accepted_diagnostics.material_balance_inf_norm,
                final_update_inf_norm,
                last_linear_report,
                accepted_hotspot_site: None,
                failure_diagnostics: Some(failure_diagnostics),
                retry_factor,
                total_time_ms: total_timer.elapsed_ms(),
                assembly_ms,
                property_eval_ms,
                linear_solve_time_ms,
                linear_preconditioner_build_time_ms,
                state_update_ms,
            };
        }

        // Early termination: if residual is not decreasing, bail out to trigger timestep cut.
        // Zero-move iters (prev iter hit HOTSPOT effective-move floor) neither count against
        // nor reset the stagnation budget — they make no progress by construction, so treating
        // them as either progress or stagnation is wrong.
        let prev_iter_was_zero_move = previous_effective_move_floor_site.is_some();
        if iteration >= 2 && current_norm >= prev_residual_norm * 0.95 && !prev_iter_was_zero_move {
            stagnation_count += 1;
            let stagnation_attrib_class: &'static str = if current_norm > prev_residual_norm {
                stagnation_attrib_real_bump += 1;
                "real-bump"
            } else {
                stagnation_attrib_slow_decay += 1;
                "slow-decay"
            };
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: STAGNATION-ATTRIB class={} count={} res={:.3e} prev_res={:.3e} ratio={:.4} (zero_move_skipped={} real_bump={} slow_decay={})",
                iteration,
                stagnation_attrib_class,
                stagnation_count,
                current_norm,
                prev_residual_norm,
                current_norm / prev_residual_norm,
                stagnation_attrib_zero_move,
                stagnation_attrib_real_bump,
                stagnation_attrib_slow_decay,
            );
            // Fix-A2 Stage 1 probe: residual-trend classification at every stagnation iter.
            // Read-only — no behavioral change. Records the residual at which this stagnation
            // run began, then reports trend relative to that entry point and to the best
            // residual seen in this timestep, along with whether the proposed widened gate
            // (progress-since-entry < 0.5 AND iter-budget >= 3) would suppress the bailout.
            if stagnation_entry_residual.is_none() {
                stagnation_entry_residual = Some(prev_residual_norm);
            }
            let entry_res = stagnation_entry_residual.unwrap_or(current_norm);
            let trend_vs_entry = if entry_res.is_finite() && entry_res > 0.0 {
                current_norm / entry_res
            } else {
                f64::NAN
            };
            let progress_vs_best = if best_residual_so_far.is_finite() && best_residual_so_far > 0.0
            {
                current_norm / best_residual_so_far
            } else {
                f64::NAN
            };
            let iters_remaining = options.max_newton_iterations.saturating_sub(iteration + 1);
            let would_widen = stagnation_count >= 3
                && trend_vs_entry.is_finite()
                && trend_vs_entry < 0.5
                && iters_remaining >= 3;
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: STAG-TREND count={} res={:.3e} entry_res={:.3e} best_res={:.3e} trend_vs_entry={:.4} vs_best={:.4} iters_remain={} would_widen={}",
                iteration,
                stagnation_count,
                current_norm,
                entry_res,
                best_residual_so_far,
                trend_vs_entry,
                progress_vs_best,
                iters_remaining,
                would_widen,
            );
            #[cfg(test)]
            if stagnation_count == 3 {
                trace_y2a_injector_jacobian_audit(
                    sim,
                    previous_state,
                    &state,
                    &topology,
                    dt_days,
                    iteration,
                    stagnation_count,
                    &assembly,
                );
            }
            // Bundle N checkpoint 5 (N4, `OpmAligned` only): this residual-trend bailout has
            // no OPM analog — OPM never inspects the residual TREND mid-solve, only its
            // absolute value against CNV/MB at each iteration's entry check. Under
            // `OpmAligned`, a stagnating trajectory simply keeps iterating (as OPM's does)
            // until the entry check accepts it or the iteration budget is exhausted and the
            // relaxed tiers decide (design doc N4).
            if !opm_aligned && stagnation_count >= 3 {
                let materially_changed = iterate_has_material_change(previous_state, &state);
                let accepted_diagnostics = evaluate_accepted_state_convergence(
                    sim,
                    previous_state,
                    &state,
                    &topology,
                    dt_days,
                );
                let gate_status = stagnation_acceptance_gate_status(
                    materially_changed,
                    accepted_diagnostics.residual_inf_norm,
                    accepted_diagnostics.material_balance_inf_norm,
                    last_effective_update_inf_norm,
                    options,
                );
                let raw_update_peak_trace = last_linear_report
                    .as_ref()
                    .map(|report| {
                        update_peak_trace(scaled_update_peak(
                            &report.solution,
                            &assembly.variable_scaling,
                        ))
                    })
                    .unwrap_or_else(|| " raw_upd_peak=[unavailable]".to_string());
                let effective_update_peak_trace = last_effective_update_peak
                    .map(|peak| format!(" eff{}", update_peak_trace(peak)))
                    .unwrap_or_else(|| " eff_upd_peak=[unavailable]".to_string());
                if stagnation_acceptance_allows(
                    materially_changed,
                    accepted_diagnostics.residual_inf_norm,
                    accepted_diagnostics.material_balance_inf_norm,
                    last_effective_update_inf_norm,
                    options,
                ) {
                    fim_trace!(
                        sim,
                        options.verbose,
                        "    iter {:>2}: STAGNATION-ACCEPTED res={:.3e} mb={:.3e} raw_upd={:.3e} eff_upd={:.3e}{}{}{} fam=[{}] mb=[{}]{}",
                        iteration,
                        accepted_diagnostics.residual_inf_norm,
                        accepted_diagnostics.material_balance_inf_norm,
                        final_update_inf_norm,
                        last_effective_update_inf_norm,
                        stagnation_acceptance_gate_trace(
                            gate_status,
                            accepted_diagnostics.residual_inf_norm,
                            accepted_diagnostics.material_balance_inf_norm,
                            last_effective_update_inf_norm,
                            options,
                        ),
                        raw_update_peak_trace,
                        effective_update_peak_trace,
                        residual_family_trace(&accepted_diagnostics.residual_diagnostics),
                        global_material_balance_trace(
                            &accepted_diagnostics.material_balance_diagnostics
                        ),
                        accepted_diagnostics
                            .residual_detail
                            .as_ref()
                            .map(|detail| format!(" detail=[{}]", detail))
                            .unwrap_or_default()
                    );
                    maybe_trace_small_dt_hotspot_neighborhood(
                        sim,
                        options.verbose,
                        "accepted",
                        dt_days,
                        previous_state,
                        &accepted_diagnostics.state,
                        residual_hotspot_site(
                            sim,
                            &topology,
                            &accepted_diagnostics.residual_diagnostics.global,
                        ),
                    );
                    return FimStepReport {
                        accepted_state: accepted_diagnostics.state,
                        converged: true,
                        newton_iterations: iteration + 1,
                        final_residual_inf_norm: accepted_diagnostics.residual_inf_norm,
                        final_material_balance_inf_norm: accepted_diagnostics
                            .material_balance_inf_norm,
                        final_update_inf_norm,
                        last_linear_report,
                        accepted_hotspot_site: Some(residual_hotspot_site(
                            sim,
                            &topology,
                            &accepted_diagnostics.residual_diagnostics.global,
                        )),
                        failure_diagnostics: None,
                        retry_factor: 1.0,
                        total_time_ms: total_timer.elapsed_ms(),
                        assembly_ms,
                        property_eval_ms,
                        linear_solve_time_ms,
                        linear_preconditioner_build_time_ms,
                        state_update_ms,
                    };
                }

                fim_trace!(
                    sim,
                    options.verbose,
                    "    iter {:>2}: STAGNATION-REJECTED res={:.3e} mb={:.3e} raw_upd={:.3e} eff_upd={:.3e}{}{}{} fam=[{}] mb=[{}]{}",
                    iteration,
                    accepted_diagnostics.residual_inf_norm,
                    accepted_diagnostics.material_balance_inf_norm,
                    final_update_inf_norm,
                    last_effective_update_inf_norm,
                    stagnation_acceptance_gate_trace(
                        gate_status,
                        accepted_diagnostics.residual_inf_norm,
                        accepted_diagnostics.material_balance_inf_norm,
                        last_effective_update_inf_norm,
                        options,
                    ),
                    raw_update_peak_trace,
                    effective_update_peak_trace,
                    residual_family_trace(&accepted_diagnostics.residual_diagnostics),
                    global_material_balance_trace(
                        &accepted_diagnostics.material_balance_diagnostics
                    ),
                    accepted_diagnostics
                        .residual_detail
                        .as_ref()
                        .map(|detail| format!(" detail=[{}]", detail))
                        .unwrap_or_default()
                );

                let failure_diagnostics = classify_retry_failure_with_site(
                    last_linear_report.as_ref(),
                    &residual_diagnostics,
                    current_hotspot_site,
                );
                let retry_factor = retry_factor_for_failure(Some(&failure_diagnostics));
                fim_trace!(
                    sim,
                    options.verbose,
                    "    iter {:>2}: STAGNATION (count={}) res={:.3e} fam=[{}]{}{} — bailing out",
                    iteration,
                    stagnation_count,
                    current_norm,
                    residual_family_trace(&residual_diagnostics),
                    residual_detail
                        .as_ref()
                        .map(|detail| format!(" detail=[{}]", detail))
                        .unwrap_or_default(),
                    retry_failure_trace_suffix(&failure_diagnostics)
                );
                return FimStepReport {
                    accepted_state: state,
                    converged: false,
                    newton_iterations: iteration + 1,
                    final_residual_inf_norm: current_norm,
                    final_material_balance_inf_norm,
                    final_update_inf_norm,
                    last_linear_report,
                    accepted_hotspot_site: None,
                    failure_diagnostics: Some(failure_diagnostics),
                    retry_factor,
                    total_time_ms: total_timer.elapsed_ms(),
                    assembly_ms,
                    property_eval_ms,
                    linear_solve_time_ms,
                    linear_preconditioner_build_time_ms,
                    state_update_ms,
                };
            }
        } else {
            if prev_iter_was_zero_move {
                stagnation_attrib_zero_move += 1;
            }
            stagnation_count = 0;
            stagnation_entry_residual = None;
        }
        prev_residual_norm = current_norm;

        let rhs = -&assembly.residual;
        // Y2d6c: materialize the source-complete companion only when its dedicated corpus
        // trigger is enabled. The selected failure/near-miss hooks below write it alongside the
        // same exact full system used by the established bounded-eight/gas-five corpora.
        #[cfg(not(target_arch = "wasm32"))]
        let y2d6_corpus_flow = crate::fim::linear::capture::y2d6_corpus_dir_from_env().map(|_| {
            crate::fim::linear::flow_lifecycle::build_capture_data(
                sim,
                previous_state,
                &state,
                block_layout.expect("FIM always defines a linear block layout"),
                &assembly.jacobian,
            )
        });
        // Y2d6a: capture the first exact, uneliminated FIM linear system together with the
        // source-pinned Flow storage blocks, true-IMPES weights, and reservoir/well partition.
        // This is a native/default-off diagnostic oracle; production dispatch is unchanged.
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(dir) = crate::fim::linear::capture::y2d6_capture_dir_from_env()
            && crate::fim::linear::capture::claim_y2d6_capture()
        {
            let metadata = crate::fim::linear::capture::FimCaptureMetadata {
                newton_iteration: iteration,
                failure_reason: "y2d6a-first-linear-system".to_string(),
                dominant_family: residual_diagnostics.global.family.label().to_string(),
                dominant_item_index: residual_diagnostics.global.item_index,
            };
            match crate::fim::linear::flow_lifecycle::build_capture_data(
                sim,
                previous_state,
                &state,
                block_layout.expect("FIM always defines a linear block layout"),
                &assembly.jacobian,
            ) {
                Ok(flow_lifecycle) => {
                    let sequence = crate::fim::linear::capture::next_capture_sequence();
                    crate::fim::linear::capture::write_flow_lifecycle_capture(
                        &dir,
                        sequence,
                        &metadata,
                        block_layout.expect("FIM always defines a linear block layout"),
                        &assembly.jacobian,
                        &rhs,
                        Some(&assembly.equation_scaling),
                        &flow_lifecycle,
                    );
                    fim_trace!(
                        sim,
                        options.verbose,
                        "    iter {:>2}: Y2D6-CAPTURE rows={} nnz={} cells={} reservoir_rows={} sequence={}",
                        iteration,
                        assembly.jacobian.rows(),
                        assembly.jacobian.nnz(),
                        state.cells.len(),
                        flow_lifecycle.reservoir_unknown_count,
                        sequence,
                    );
                }
                Err(error) => eprintln!("Y2d6 capture rejected: {error}"),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        if y2b3_primary_variable_lifecycle
            && iteration == 1
            && (dt_days - Y2B2_CAPTURE_DT_DAYS).abs() <= 1e-12
            && let Some(dir) = crate::fim::linear::capture::y2b2_capture_dir_from_env()
            && crate::fim::linear::capture::claim_y2b2_capture()
        {
            let metadata = crate::fim::linear::capture::FimCaptureMetadata {
                newton_iteration: iteration,
                failure_reason: "y2b3c-exact-decision".to_string(),
                dominant_family: residual_diagnostics.global.family.label().to_string(),
                dominant_item_index: residual_diagnostics.global.item_index,
            };
            let sequence = crate::fim::linear::capture::next_capture_sequence();
            crate::fim::linear::capture::write_capture(
                &dir,
                sequence,
                &metadata,
                block_layout,
                &assembly.jacobian,
                &rhs,
                Some(&assembly.equation_scaling),
            );
            trace_y2b3_primary_variable_state(
                sim,
                options.verbose,
                &state,
                &primary_variables_switched,
                &assembly.jacobian,
            );
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: Y2B2-CAPTURE dt={:.8} rows={} nnz={} rhs_norm={:.6e} state_checksum={:016x} sequence={}",
                iteration,
                dt_days,
                assembly.jacobian.rows(),
                assembly.jacobian.nnz(),
                rhs.norm(),
                y2b2_state_checksum(&state),
                sequence,
            );
        }
        let mut linear_options = options.linear;
        // Bundle N checkpoint 5 (N4, `OpmAligned` only): none of these preemptive direct-solve
        // bypasses have an OPM analog (design doc N4 lists "the direct-solve bypass ladder" for
        // deletion) — under `OpmAligned` every linear solve goes through the requested iterative
        // backend first, with N5 (checkpoint 3) deciding after the fact whether to accept it.
        let any_preexisting_bypass = !opm_aligned
            && (dead_state_direct_bypass
                || restart_stagnation_direct_bypass
                || zero_move_fallback_direct_bypass
                || repeated_zero_move_direct_bypass);
        if any_preexisting_bypass {
            linear_options.kind = direct_fallback_kind_for_rows(assembly.jacobian.rows());
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: bypassing iterative backend after {}; using {}",
                iteration,
                if dead_state_direct_bypass {
                    "dead-state"
                } else if restart_stagnation_direct_bypass {
                    "repeated restart-stagnation"
                } else if repeated_zero_move_direct_bypass {
                    "repeated zero-move hotspot"
                } else {
                    "zero-move fallback"
                },
                linear_options.kind.label(),
            );
        } else if iterative_failed_last_iter {
            linear_options.kind = direct_fallback_kind_for_rows(assembly.jacobian.rows());
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: iterative-failure short-circuit (prev iter fell back); using {}",
                iteration,
                linear_options.kind.label(),
            );
        }
        // Step 10.1 follow-up (`FIM-LINEAR-009`): family-aware convergence is built and
        // offline-lab-validated infrastructure, but the offline evidence did NOT support it as
        // the primary fix (only 1/35 heavy-corpus systems showed a real per-family overshoot;
        // see `docs/FIM_CONVERGENCE_WORKLOG.md` "Step 10.1 follow-up"). Kept opt-in (`None`
        // here) rather than wired live, pending stronger evidence or a different application.
        let mut linear_report = solve_linearized_system(
            &assembly.jacobian,
            &rhs,
            &linear_options,
            block_layout,
            None,
        );
        linear_solve_time_ms += linear_report.total_time_ms;
        linear_preconditioner_build_time_ms += linear_report.preconditioner_build_time_ms;

        // Bundle P (`FIM-BUNDLE-P`) P0.2: unconditional per-iteration capture of every linear
        // system actually solved, gated on a distinct env var from the failure-only
        // `FIM_CAPTURE_DIR` below — the offline CPR-setup-reuse staleness study needs truly
        // consecutive Newton-iteration systems (file order == solve order within a run), which
        // the failure/near-miss-only capture cannot provide.
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(sequence_dir) = crate::fim::linear::capture::capture_sequence_dir_from_env() {
            let metadata = crate::fim::linear::capture::FimCaptureMetadata {
                newton_iteration: iteration,
                failure_reason: if linear_report.converged {
                    "converged".to_string()
                } else {
                    "not-converged".to_string()
                },
                dominant_family: residual_diagnostics.global.family.label().to_string(),
                dominant_item_index: residual_diagnostics.global.item_index,
            };
            crate::fim::linear::capture::write_capture(
                &sequence_dir,
                crate::fim::linear::capture::next_capture_sequence(),
                &metadata,
                block_layout,
                &assembly.jacobian,
                &rhs,
                Some(&assembly.equation_scaling),
            );
        }

        // Step 10.1 follow-up (`FIM-LINEAR-008` reopened): under the loosened Phase 10 CPR
        // tolerance the linear solve itself usually succeeds, but the *Newton* loop can still
        // exhaust `max_newton_iterations` on a near-miss (see the worklog's "Step 10.4
        // (reopened)" section). The existing capture below only fires when the linear solve
        // fails, so it never captures these systems. Capture the final iteration's system
        // unconditionally (regardless of `linear_report.converged`) so the offline lab has real
        // near-miss systems to test a family-aware convergence criterion against.
        #[cfg(not(target_arch = "wasm32"))]
        if iteration + 1 == options.max_newton_iterations {
            if let Some(capture_dir) = crate::fim::linear::capture::capture_dir_from_env() {
                let metadata = crate::fim::linear::capture::FimCaptureMetadata {
                    newton_iteration: iteration,
                    failure_reason: "final-iteration-near-miss".to_string(),
                    dominant_family: residual_diagnostics.global.family.label().to_string(),
                    dominant_item_index: residual_diagnostics.global.item_index,
                };
                crate::fim::linear::capture::write_capture(
                    &capture_dir,
                    crate::fim::linear::capture::next_capture_sequence(),
                    &metadata,
                    block_layout,
                    &assembly.jacobian,
                    &rhs,
                    Some(&assembly.equation_scaling),
                );
            }
            if let Some(corpus_dir) = crate::fim::linear::capture::y2d6_corpus_dir_from_env() {
                let metadata = crate::fim::linear::capture::FimCaptureMetadata {
                    newton_iteration: iteration,
                    failure_reason: "final-iteration-near-miss".to_string(),
                    dominant_family: residual_diagnostics.global.family.label().to_string(),
                    dominant_item_index: residual_diagnostics.global.item_index,
                };
                match y2d6_corpus_flow
                    .as_ref()
                    .expect("Y2d6 corpus payload was requested")
                {
                    Ok(flow_lifecycle) => {
                        crate::fim::linear::capture::write_flow_lifecycle_capture(
                            &corpus_dir,
                            crate::fim::linear::capture::next_capture_sequence(),
                            &metadata,
                            block_layout.expect("FIM always defines a linear block layout"),
                            &assembly.jacobian,
                            &rhs,
                            Some(&assembly.equation_scaling),
                            flow_lifecycle,
                        );
                    }
                    Err(error) => eprintln!("Y2d6 corpus capture rejected: {error}"),
                }
            }
        }

        let mut used_fallback = false;
        if !opm_aligned {
            if !linear_report.converged
                || !linear_report.solution.iter().all(|value| value.is_finite())
            {
                if should_accept_near_converged_iterative_step(&linear_report) {
                    fim_trace!(
                        sim,
                        options.verbose,
                        "    iter {:>2}: accepting near-converged iterative step without direct fallback{}{}",
                        iteration,
                        linear_report_trace_suffix(&linear_report, requested_linear_kind),
                        linear_failure_trace_suffix(&linear_report),
                    );
                    linear_report.converged = true;
                }
            }
            if !linear_report.converged
                || !linear_report.solution.iter().all(|value| value.is_finite())
            {
                let iterative_failure_reason = linear_report
                    .failure_diagnostics
                    .as_ref()
                    .map(|diagnostics| diagnostics.reason);
                fim_trace!(
                    sim,
                    options.verbose,
                    "    iter {:>2}: linear solver FAILED (converged={}, finite={}), trying fallback{}{}",
                    iteration,
                    linear_report.converged,
                    linear_report.solution.iter().all(|v| v.is_finite()),
                    linear_report_trace_suffix(&linear_report, requested_linear_kind),
                    linear_failure_trace_suffix(&linear_report),
                );
                // Phase 8 step 8.1: on a linear-solve failure only, capture the actual nonlinear
                // state (saturations/regime/mobility for cells; slack/freeze/mobility for
                // perforations) at the hotspot row so failure characterization doesn't require a
                // separate repro. Gated on failure_diagnostics being Some (i.e. only this already-
                // failing branch) so it cannot affect the solve-succeeds path.
                if let Some(detail) = residual_family_detail_trace(
                    sim,
                    previous_state,
                    &state,
                    &topology,
                    dt_days,
                    &residual_diagnostics,
                ) {
                    fim_trace!(
                        sim,
                        options.verbose,
                        "    iter {:>2}: FAIL-SITE-DETAIL {}",
                        iteration,
                        detail,
                    );
                }
                // Phase 9 step 9.1: offline solver-lab capture. Native-only and inert unless
                // FIM_CAPTURE_DIR is set — dumps the exact failed system so preconditioner
                // variants can be compared as full solves out-of-loop instead of via live
                // solver changes.
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(capture_dir) = crate::fim::linear::capture::capture_dir_from_env() {
                    let metadata = crate::fim::linear::capture::FimCaptureMetadata {
                        newton_iteration: iteration,
                        failure_reason: iterative_failure_reason
                            .map(|reason| reason.label().to_string())
                            .unwrap_or_else(|| "non-finite".to_string()),
                        dominant_family: residual_diagnostics.global.family.label().to_string(),
                        dominant_item_index: residual_diagnostics.global.item_index,
                    };
                    crate::fim::linear::capture::write_capture(
                        &capture_dir,
                        crate::fim::linear::capture::next_capture_sequence(),
                        &metadata,
                        block_layout,
                        &assembly.jacobian,
                        &rhs,
                        Some(&assembly.equation_scaling),
                    );
                }
                if linear_report
                    .failure_diagnostics
                    .as_ref()
                    .is_some_and(|diagnostics| {
                        diagnostics.reason == FimLinearFailureReason::DeadStateDetected
                    })
                {
                    dead_state_direct_bypass = true;
                }
                restart_stagnation_fallback_streak = next_restart_stagnation_fallback_streak(
                    restart_stagnation_fallback_streak,
                    iterative_failure_reason,
                );
                if should_enable_restart_stagnation_direct_bypass(
                    restart_stagnation_fallback_streak,
                ) {
                    restart_stagnation_direct_bypass = true;
                }
                let mut fallback_options = options.linear;
                fallback_options.kind = direct_fallback_kind_for_rows(assembly.jacobian.rows());
                linear_report = solve_linearized_system(
                    &assembly.jacobian,
                    &rhs,
                    &fallback_options,
                    block_layout,
                    None,
                );
                used_fallback = true;
                linear_report.used_fallback = true;
                linear_solve_time_ms += linear_report.total_time_ms;
                linear_preconditioner_build_time_ms += linear_report.preconditioner_build_time_ms;
            } else {
                restart_stagnation_fallback_streak = 0;
            }
        } else {
            // Bundle N checkpoint 3 (N5, `OpmAligned` only): OPM's linear-failure handling
            // (design doc §9.5) — no direct-solve fallback ladder exists in OPM's path at
            // all. A solve that fully converged is used as-is. One that missed its target
            // but still achieved the relaxed reduction (`< relaxed-linear-solver-reduction`,
            // i.e. at least ~100x) is accepted with a warning, matching OPM's
            // `checkConvergence` exactly. Anything else — including a non-finite solution —
            // is a genuine failure that aborts this Newton iteration immediately (OPM's
            // `NumericalProblem` throw): no dead-state/restart-stagnation/zero-move
            // bypass bookkeeping, no direct-LU rescue.
            let all_finite = linear_report.solution.iter().all(|value| value.is_finite());
            if !linear_report.converged || !all_finite {
                // Bundle N checkpoint 6 (N5 follow-up, bug fix): `failure.outer_residual_norm`
                // is the residual at the START of the FINAL restart cycle (computed from the
                // last *committed* solution, before that cycle's candidate is evaluated) — on
                // a solve that never fully converges it can stay pinned at `rhs_norm` (the
                // x_0=0 starting point) even though later restarts produced a materially
                // better, already-returned candidate. `final_residual_norm` on the report
                // itself is exactly that candidate's true residual (`gmres_block_jacobi.rs`
                // sets it from `candidate_residual` at the max-iterations return site) — the
                // correct quantity for OPM's reduction check, matching Dune ISTL's
                // `result.reduction` (computed from the solution actually returned, not an
                // intermediate diagnostic). Confirmed via the `23x23x1` trace: every observed
                // failure previously reported `reduction=1.000e0` regardless of how much the
                // candidate had actually improved.
                let reduction = linear_report.reduction();
                let accept_relaxed = opm_accepts_relaxed_linear_report(&linear_report);
                if accept_relaxed {
                    fim_trace!(
                        sim,
                        options.verbose,
                        "    iter {:>2}: LINEAR-ACCEPT relaxed reduction={:.3e} (< {:.0e}){}",
                        iteration,
                        reduction,
                        OPM_RELAXED_LINEAR_SOLVER_REDUCTION,
                        linear_report_trace_suffix(&linear_report, requested_linear_kind),
                    );
                    linear_report.converged = true;
                } else {
                    fim_trace!(
                        sim,
                        options.verbose,
                        "    iter {:>2}: LINEAR FAILED (opm-aligned, no fallback) converged={} finite={} reduction={}{}",
                        iteration,
                        linear_report.converged,
                        all_finite,
                        format!("{reduction:.3e}"),
                        linear_failure_trace_suffix(&linear_report),
                    );
                    // Bundle Y Y1a (`docs/FIM_OPM_PARITY_PLAN.md` §7): the offline solver lab's
                    // entire evidence base (`FIM-LINEAR-005`/`008`/`010`) was captured exclusively
                    // via the `!opm_aligned` branch above — this `OpmAligned` abort path had no
                    // capture call at all, so no linear-stack decision has ever been validated
                    // against an actual `OpmAligned` failure. Mirrors the Legacy capture site
                    // exactly (same metadata shape, same `FIM_CAPTURE_DIR` gate); native-only and
                    // inert unless the env var is set.
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Some(capture_dir) = crate::fim::linear::capture::capture_dir_from_env() {
                        let metadata = crate::fim::linear::capture::FimCaptureMetadata {
                            newton_iteration: iteration,
                            failure_reason: linear_report
                                .failure_diagnostics
                                .as_ref()
                                .map(|diagnostics| diagnostics.reason.label().to_string())
                                .unwrap_or_else(|| {
                                    if all_finite {
                                        "opm-aligned-no-diagnostics".to_string()
                                    } else {
                                        "non-finite".to_string()
                                    }
                                }),
                            dominant_family: residual_diagnostics.global.family.label().to_string(),
                            dominant_item_index: residual_diagnostics.global.item_index,
                        };
                        crate::fim::linear::capture::write_capture(
                            &capture_dir,
                            crate::fim::linear::capture::next_capture_sequence(),
                            &metadata,
                            block_layout,
                            &assembly.jacobian,
                            &rhs,
                            Some(&assembly.equation_scaling),
                        );
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Some(corpus_dir) =
                        crate::fim::linear::capture::y2d6_corpus_dir_from_env()
                    {
                        let metadata = crate::fim::linear::capture::FimCaptureMetadata {
                            newton_iteration: iteration,
                            failure_reason: linear_report
                                .failure_diagnostics
                                .as_ref()
                                .map(|diagnostics| diagnostics.reason.label().to_string())
                                .unwrap_or_else(|| {
                                    if all_finite {
                                        "opm-aligned-no-diagnostics".to_string()
                                    } else {
                                        "non-finite".to_string()
                                    }
                                }),
                            dominant_family: residual_diagnostics.global.family.label().to_string(),
                            dominant_item_index: residual_diagnostics.global.item_index,
                        };
                        match y2d6_corpus_flow
                            .as_ref()
                            .expect("Y2d6 corpus payload was requested")
                        {
                            Ok(flow_lifecycle) => {
                                crate::fim::linear::capture::write_flow_lifecycle_capture(
                                    &corpus_dir,
                                    crate::fim::linear::capture::next_capture_sequence(),
                                    &metadata,
                                    block_layout.expect("FIM always defines a linear block layout"),
                                    &assembly.jacobian,
                                    &rhs,
                                    Some(&assembly.equation_scaling),
                                    flow_lifecycle,
                                );
                            }
                            Err(error) => eprintln!("Y2d6 corpus capture rejected: {error}"),
                        }
                    }
                    let failure_diagnostics = classify_retry_failure_with_site(
                        Some(&linear_report),
                        &residual_diagnostics,
                        current_hotspot_site,
                    );
                    let retry_factor = retry_factor_for_failure(Some(&failure_diagnostics));
                    return FimStepReport {
                        accepted_state: state,
                        converged: false,
                        newton_iterations: iteration + 1,
                        final_residual_inf_norm: current_norm,
                        final_material_balance_inf_norm,
                        final_update_inf_norm,
                        last_linear_report: Some(linear_report),
                        accepted_hotspot_site: None,
                        failure_diagnostics: Some(failure_diagnostics),
                        retry_factor,
                        total_time_ms: total_timer.elapsed_ms(),
                        assembly_ms,
                        property_eval_ms,
                        linear_solve_time_ms,
                        linear_preconditioner_build_time_ms,
                        state_update_ms,
                    };
                }
            }
        }
        iterative_failed_last_iter = used_fallback && !any_preexisting_bypass;

        let update_peak = scaled_update_peak(&linear_report.solution, &assembly.variable_scaling);
        final_update_inf_norm =
            scaled_update_inf_norm(&linear_report.solution, &assembly.variable_scaling);
        debug_assert!((update_peak.scaled_value - final_update_inf_norm).abs() < 1e-12);
        last_linear_report = Some(linear_report.clone());

        // Bundle N checkpoint 3: this "raw update is already tiny" shortcut has no OPM
        // analog — OPM's only acceptance test is CNV/MB, freshly re-evaluated at the top of
        // the next iteration (`converged_on_entry` above). Disabled entirely under
        // `OpmAligned` so control falls through to the candidate-validity check and the
        // next loop iteration's entry check, instead of this Legacy-only mid-iteration exit.
        let converged = !opm_aligned
            && final_update_inf_norm <= options.update_tolerance
            && current_norm <= options.residual_tolerance * ENTRY_RESIDUAL_GUARD_FACTOR
            && iterate_has_material_change(previous_state, &state);
        if converged {
            let use_guard_band = current_norm > options.residual_tolerance;
            let (residual_limit, material_balance_limit) =
                convergence_limits(options, use_guard_band);
            let accepted_diagnostics = evaluate_accepted_state_convergence(
                sim,
                previous_state,
                &state,
                &topology,
                dt_days,
            );
            if accepted_state_meets_convergence(
                &accepted_diagnostics,
                residual_limit,
                material_balance_limit,
            ) {
                fim_trace!(
                    sim,
                    options.verbose,
                    "    iter {:>2}: CONVERGED res={:.3e} mb={:.3e} upd={:.3e} linear_iters={}{}{} fam=[{}] mb=[{}]{}",
                    iteration,
                    accepted_diagnostics.residual_inf_norm,
                    accepted_diagnostics.material_balance_inf_norm,
                    final_update_inf_norm,
                    linear_report.iterations,
                    if used_fallback { " [fallback]" } else { "" },
                    linear_report_trace_suffix(&linear_report, requested_linear_kind),
                    residual_family_trace(&accepted_diagnostics.residual_diagnostics),
                    global_material_balance_trace(
                        &accepted_diagnostics.material_balance_diagnostics
                    ),
                    accepted_diagnostics
                        .residual_detail
                        .as_ref()
                        .map(|detail| format!(" detail=[{}]", detail))
                        .unwrap_or_default()
                );
                return FimStepReport {
                    accepted_state: accepted_diagnostics.state,
                    converged: true,
                    newton_iterations: iteration + 1,
                    final_residual_inf_norm: accepted_diagnostics.residual_inf_norm,
                    final_material_balance_inf_norm: accepted_diagnostics.material_balance_inf_norm,
                    final_update_inf_norm,
                    last_linear_report: Some(linear_report),
                    accepted_hotspot_site: Some(residual_hotspot_site(
                        sim,
                        &topology,
                        &accepted_diagnostics.residual_diagnostics.global,
                    )),
                    failure_diagnostics: None,
                    retry_factor: 1.0,
                    total_time_ms: total_timer.elapsed_ms(),
                    assembly_ms,
                    property_eval_ms,
                    linear_solve_time_ms,
                    linear_preconditioner_build_time_ms,
                    state_update_ms,
                };
            }

            let failure_diagnostics = classify_retry_failure_with_site(
                Some(&linear_report),
                &accepted_diagnostics.residual_diagnostics,
                residual_hotspot_site(
                    sim,
                    &topology,
                    &accepted_diagnostics.residual_diagnostics.global,
                ),
            );
            let retry_factor = retry_factor_for_failure(Some(&failure_diagnostics));
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: POST-CLASSIFICATION REJECTED res={:.3e} mb={:.3e} upd={:.3e} linear_iters={}{}{} fam=[{}] mb=[{}]{}{}",
                iteration,
                accepted_diagnostics.residual_inf_norm,
                accepted_diagnostics.material_balance_inf_norm,
                final_update_inf_norm,
                linear_report.iterations,
                if used_fallback { " [fallback]" } else { "" },
                linear_report_trace_suffix(&linear_report, requested_linear_kind),
                residual_family_trace(&accepted_diagnostics.residual_diagnostics),
                global_material_balance_trace(&accepted_diagnostics.material_balance_diagnostics),
                accepted_diagnostics
                    .residual_detail
                    .as_ref()
                    .map(|detail| format!(" detail=[{}]", detail))
                    .unwrap_or_default(),
                retry_failure_trace_suffix(&failure_diagnostics)
            );
            maybe_trace_small_dt_hotspot_neighborhood(
                sim,
                options.verbose,
                "rejected",
                dt_days,
                previous_state,
                &accepted_diagnostics.state,
                failure_diagnostics.hotspot_site,
            );
            return FimStepReport {
                accepted_state: accepted_diagnostics.state,
                converged: false,
                newton_iterations: iteration + 1,
                final_residual_inf_norm: accepted_diagnostics.residual_inf_norm,
                final_material_balance_inf_norm: accepted_diagnostics.material_balance_inf_norm,
                final_update_inf_norm,
                last_linear_report: Some(linear_report),
                accepted_hotspot_site: None,
                failure_diagnostics: Some(failure_diagnostics),
                retry_factor,
                total_time_ms: total_timer.elapsed_ms(),
                assembly_ms,
                property_eval_ms,
                linear_solve_time_ms,
                linear_preconditioner_build_time_ms,
                state_update_ms,
            };
        }

        // Bundle N checkpoint 2 (N2): under `OpmAligned` the entire Legacy update-limiting
        // layer (history stabilization + global Appleyard scalar + inflection chop) is
        // replaced by OPM's per-cell chopping; the oscillation-relaxation scalar stays and
        // pre-multiplies the raw update, matching OPM's `dampen`-then-chop order (§9.2/§9.3
        // of `docs/FIM_BUNDLE_N_DESIGN.md`).
        let opm_chopped_update = if opm_aligned {
            let chopped = opm_per_cell_chopped_update(
                &state,
                &linear_report.solution,
                relaxation_state.current_relaxation,
            );
            let mut sat_chopped_cells = 0usize;
            let mut dp_clamped_cells = 0usize;
            let relax = relaxation_state.current_relaxation;
            for idx in 0..state.cells.len() {
                let offset = idx * 3;
                if (chopped[offset + 1] - relax * linear_report.solution[offset + 1]).abs() > 1e-15
                {
                    sat_chopped_cells += 1;
                }
                if (chopped[offset] - relax * linear_report.solution[offset]).abs() > 1e-15 {
                    dp_clamped_cells += 1;
                }
            }
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: PERCELL-CHOP relax={:.2} sat_chopped_cells={} dp_clamped_cells={}",
                iteration,
                relax,
                sat_chopped_cells,
                dp_clamped_cells,
            );
            Some(chopped)
        } else {
            None
        };
        let history_stabilization = if opm_aligned {
            None
        } else {
            nonlinear_history_stabilization_decision(
                &linear_report,
                &residual_diagnostics,
                current_norm,
                options,
                repeated_hotspot_streak,
                current_hotspot_site,
            )
        };
        let damping = if opm_aligned {
            // Per-cell chopping already limited the update; relaxation is folded into the
            // chopped vector. The scalar passed to the state application must be exactly 1.
            1.0
        } else {
            let damping_breakdown =
                appleyard_damping_breakdown(sim, &state, &linear_report.solution, options);
            // Phase 7 sub-phase 7.2: fold OPM's global oscillation-relaxation scalar into the
            // same damping bound as Appleyard and history-stabilization — matching how OPM
            // itself composes per-variable chopping and oscillation relaxation as two
            // independent multiplicative bounds on the same update, not layered stages.
            let damping = compose_damping(
                damping_breakdown.final_damping,
                history_stabilization.as_ref().map(|d| d.damping_cap),
                relaxation_state.current_relaxation,
            );
            // Fix A3 Stage 1 probe: read-only damping breakdown — which constraint bound
            // the Appleyard chop, raw per-variable update peaks, and whether history
            // stabilization further capped it. Used to investigate why initial-iter
            // damping is 0.005-0.07 on the case 2 medium-water step.
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: DAMP-BREAKDOWN final={:.4} appleyard={:.4} hist_cap={} osc_relax={:.2} bind={}@{} raw_dp={:.3e}@cell{} raw_dsw={:.3e}@cell{} raw_dh={:.3e}@cell{} raw_dbhp={:.3e}@well{} inflection={}",
                iteration,
                damping,
                damping_breakdown.final_damping,
                history_stabilization
                    .as_ref()
                    .map(|d| format!("{:.3}", d.damping_cap))
                    .unwrap_or_else(|| "none".to_string()),
                relaxation_state.current_relaxation,
                damping_breakdown.binding_kind,
                damping_breakdown
                    .binding_cell
                    .map(|c| format!("cell{}", c))
                    .or_else(|| damping_breakdown.binding_well.map(|w| format!("well{}", w)))
                    .unwrap_or_else(|| "none".to_string()),
                damping_breakdown.raw_dp_peak,
                damping_breakdown
                    .raw_dp_peak_cell
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                damping_breakdown.raw_dsw_peak,
                damping_breakdown
                    .raw_dsw_peak_cell
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                damping_breakdown.raw_dh_peak,
                damping_breakdown
                    .raw_dh_peak_cell
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                damping_breakdown.raw_dbhp_peak,
                damping_breakdown
                    .raw_dbhp_peak_well
                    .map(|w| w.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                damping_breakdown.inflection_crossings,
            );
            // Stage 1 over-damping probe: compute the per-variable damp candidate
            // each component would have allowed in isolation, and report the wasted
            // ratio (applied_damp / per_var_candidate). Ratios <1 mean the global
            // single-scalar damping is over-damping that component.
            {
                let damp_dp_only = if damping_breakdown.raw_dp_peak > 1e-12 {
                    (options.max_pressure_change_bar / damping_breakdown.raw_dp_peak).min(1.0)
                } else {
                    1.0
                };
                let damp_dsw_only = if damping_breakdown.raw_dsw_peak > 1e-12 {
                    (options.max_saturation_change / damping_breakdown.raw_dsw_peak).min(1.0)
                } else {
                    1.0
                };
                let damp_dh_only = if damping_breakdown.raw_dh_peak > 1e-12 {
                    (options.max_saturation_change / damping_breakdown.raw_dh_peak).min(1.0)
                } else {
                    1.0
                };
                let damp_dbhp_only = if damping_breakdown.raw_dbhp_peak > 1e-12 {
                    (options.max_pressure_change_bar / damping_breakdown.raw_dbhp_peak).min(1.0)
                } else {
                    1.0
                };
                let waste = |allowed: f64| {
                    if allowed > 0.0 {
                        damping / allowed
                    } else {
                        1.0
                    }
                };
                fim_trace!(
                    sim,
                    options.verbose,
                    "    iter {:>2}: COMPONENT-CLIP applied={:.4} dp_allowed={:.4}(waste={:.4}) dsw_allowed={:.4}(waste={:.4}) dh_allowed={:.4}(waste={:.4}) dbhp_allowed={:.4}(waste={:.4})",
                    iteration,
                    damping,
                    damp_dp_only,
                    waste(damp_dp_only),
                    damp_dsw_only,
                    waste(damp_dsw_only),
                    damp_dh_only,
                    waste(damp_dh_only),
                    damp_dbhp_only,
                    waste(damp_dbhp_only),
                );
            }
            damping
        };
        let state_update_timer = PerfTimer::start();
        let update_to_apply = opm_chopped_update
            .as_ref()
            .unwrap_or(&linear_report.solution);
        // Bundle W (`docs/FIM_BUNDLE_W_PLAN.md` §5 item 1): independent flag, evaluable under
        // either `nonlinear_flavor` — default false selects `Relax`, bit-identical to before.
        let well_update_mode = if options.nested_well_solve {
            crate::fim::state::WellStateUpdateMode::NestedSolve
        } else {
            crate::fim::state::WellStateUpdateMode::Relax
        };
        let (candidate, candidate_primary_variables_switched) = if y2b3_primary_variable_lifecycle {
            let (candidate, switched) = state.apply_newton_update_opm_primary_variables(
                sim,
                update_to_apply,
                damping,
                &topology,
                well_update_mode,
                &primary_variables_switched,
            );
            (candidate, Some(switched))
        } else {
            (
                state.apply_newton_update_frozen(
                    sim,
                    update_to_apply,
                    damping,
                    &topology,
                    well_update_mode,
                ),
                None,
            )
        };
        #[cfg(test)]
        if stagnation_count == 3 {
            trace_y2b_bound_projection_audit(
                sim,
                previous_state,
                &state,
                &topology,
                dt_days,
                iteration,
                stagnation_count,
                &assembly,
                update_to_apply,
                damping,
                &candidate,
            );
        }
        state_update_ms += state_update_timer.elapsed_ms();
        // Late-window trace diagnostic (`docs/FIM_BUNDLE_N_DESIGN.md` §10): per-iteration
        // well/perforation state, window-gated so this never runs unconditionally (unlike
        // `fim_trace!`'s own `format!()`, which does — see the trace-overhead note on the
        // `_opm_aligned_no_trace` repro). `relax_dbhp_approx`/`relax_dq_approx` attribute
        // `relax_well_state_toward_local_consistency`'s contribution by arithmetic
        // (`candidate − (state + damping·update)`), which ignores `enforce_control_bounds`
        // clamping that can also land in between — a first-order approximation, not exact.
        #[cfg(not(target_arch = "wasm32"))]
        if sim.fim_trace_window_active {
            let raw_dbhp: Vec<f64> = (0..state.n_well_unknowns())
                .map(|well_idx| damping * update_to_apply[state.well_bhp_unknown_offset(well_idx)])
                .collect();
            let raw_dq: Vec<f64> = (0..state.n_perforation_unknowns())
                .map(|perf_idx| {
                    damping * update_to_apply[state.perforation_rate_unknown_offset(perf_idx)]
                })
                .collect();
            let relax_dbhp_approx: Vec<f64> = (0..state.n_well_unknowns())
                .map(|well_idx| {
                    candidate.well_bhp[well_idx] - (state.well_bhp[well_idx] + raw_dbhp[well_idx])
                })
                .collect();
            let relax_dq_approx: Vec<f64> = (0..state.n_perforation_unknowns())
                .map(|perf_idx| {
                    candidate.perforation_rates_m3_day[perf_idx]
                        - (state.perforation_rates_m3_day[perf_idx] + raw_dq[perf_idx])
                })
                .collect();
            crate::fim::trace_sink::write_line(&format!(
                "WELLTRACE iter={:>2} bhp_pre={:?} bhp_post={:?} q_pre={:?} q_post={:?} raw_dbhp={:?} raw_dq={:?} relax_dbhp_approx={:?} relax_dq_approx={:?} res_wc={:.6e} res_pf={:.6e}",
                iteration,
                state.well_bhp,
                candidate.well_bhp,
                state.perforation_rates_m3_day,
                candidate.perforation_rates_m3_day,
                raw_dbhp,
                raw_dq,
                relax_dbhp_approx,
                relax_dq_approx,
                residual_diagnostics
                    .well_constraint
                    .map_or(f64::INFINITY, |peak| peak.scaled_value),
                residual_diagnostics
                    .perforation_flow
                    .map_or(f64::INFINITY, |peak| peak.scaled_value),
            ));
            // FIM-BUNDLE-X X0 (`docs/FIM_BUNDLE_X_PLAN.md`): stage-by-stage first-order-
            // consistency forensics. `enforce_cell_bounds`/`enforce_control_bounds` never touch
            // perforation rates (verified by inspection: neither function references
            // `perforation_rates_m3_day`) and `opm_per_cell_chopped_update` never chops
            // perforation-rate entries either — so `raw_dq` above (post relaxation-scalar,
            // post chop) is the coupled linear system's dq unmodified by anything except the
            // `NestedSolve` override that follows. This line answers X0's "why does the coupled
            // solve propose a large dq when rate_consistency's own residual is already ~0"
            // question directly from the assembled Jacobian: per perforation, its own
            // `rate_consistency` row's residual and `d/dq` diagonal, plus its cell's
            // water/oil/gas row residuals and their `d/dq` coupling (the well source term's
            // sensitivity to this perforation's rate) — read straight from `assembly.jacobian`
            // via the same `CsMat::get(row, col)` accessor the W1 agreement test uses.
            for (perf_idx, perforation) in topology.perforations.iter().enumerate() {
                let perf_row = state.perforation_equation_offset(perf_idx);
                let q_col = state.perforation_rate_unknown_offset(perf_idx);
                let cell_idx = perforation.cell_index;
                let p_col = unknown_offset(cell_idx, 0);
                let sw_col = unknown_offset(cell_idx, 1);
                let d_pf_dq = assembly
                    .jacobian
                    .get(perf_row, q_col)
                    .copied()
                    .unwrap_or(0.0);
                let d_pf_dp = assembly
                    .jacobian
                    .get(perf_row, p_col)
                    .copied()
                    .unwrap_or(0.0);
                let d_pf_dsw = assembly
                    .jacobian
                    .get(perf_row, sw_col)
                    .copied()
                    .unwrap_or(0.0);
                let cell_rows = [
                    ("water", equation_offset(cell_idx, 0)),
                    ("oil", equation_offset(cell_idx, 1)),
                    ("gas", equation_offset(cell_idx, 2)),
                ];
                let cell_terms: Vec<String> = cell_rows
                    .iter()
                    .map(|(label, row)| {
                        let r = assembly.residual[*row];
                        let d_dq = assembly.jacobian.get(*row, q_col).copied().unwrap_or(0.0);
                        let d_dp = assembly.jacobian.get(*row, p_col).copied().unwrap_or(0.0);
                        let d_dsw = assembly.jacobian.get(*row, sw_col).copied().unwrap_or(0.0);
                        format!(
                            "{label}=[r={r:.6e} d/dq={d_dq:.6e} d/dp={d_dp:.6e} d/dsw={d_dsw:.6e}]"
                        )
                    })
                    .collect();
                let raw_dp_cell = update_to_apply[p_col];
                let raw_dsw_cell = update_to_apply[sw_col];
                let sw_current = state.cells[cell_idx].sw;
                let sw_wc = if sim.three_phase_mode {
                    sim.scal_3p
                        .as_ref()
                        .map(|s| s.s_wc)
                        .unwrap_or(sim.scal.s_wc)
                } else {
                    sim.scal.s_wc
                };
                crate::fim::trace_sink::write_line(&format!(
                    "WELLJAC iter={:>2} perf={} cell={} res_pf={:.6e} d(res_pf)/dq={:.6e} d(res_pf)/dp={:.6e} d(res_pf)/dsw={:.6e} sw={:.6} sw_wc={:.6} raw_dp={:.6e} raw_dsw={:.6e} sw_unclamped_would_be={:.6} {}",
                    iteration,
                    perf_idx,
                    cell_idx,
                    assembly.residual[perf_row],
                    d_pf_dq,
                    d_pf_dp,
                    d_pf_dsw,
                    sw_current,
                    sw_wc,
                    raw_dp_cell,
                    raw_dsw_cell,
                    sw_current + raw_dsw_cell,
                    cell_terms.join(" "),
                ));
                if let Some(water_breakdown) = cell_equation_residual_breakdown(
                    sim,
                    previous_state,
                    &state,
                    &topology,
                    dt_days,
                    cell_idx,
                    0,
                ) {
                    crate::fim::trace_sink::write_line(&format!(
                        "WELLJAC-WATER iter={:>2} cell={} accum={:.6e} x-={:.6e} x+={:.6e} y-={:.6e} y+={:.6e} z-={:.6e} z+={:.6e} well={:.6e} total={:.6e}",
                        iteration,
                        cell_idx,
                        water_breakdown.accumulation,
                        water_breakdown.x_minus,
                        water_breakdown.x_plus,
                        water_breakdown.y_minus,
                        water_breakdown.y_plus,
                        water_breakdown.z_minus,
                        water_breakdown.z_plus,
                        water_breakdown.well_source,
                        water_breakdown.total,
                    ));
                }
            }
        }
        let effective_update_peak =
            scaled_applied_update_peak(&state, &candidate, &assembly.variable_scaling);
        last_effective_update_inf_norm = effective_update_peak.scaled_value;
        last_effective_update_peak = Some(effective_update_peak);
        let (candidate_pressure_change, candidate_saturation_change) =
            state_update_change_bounds(&state, &candidate);
        zero_move_fallback_direct_bypass = should_enable_zero_move_fallback_direct_bypass(
            used_fallback,
            candidate_pressure_change,
            candidate_saturation_change,
        );
        let effective_move_trace = effective_move_threshold_trace(
            sim,
            &state,
            &candidate,
            &topology,
            &residual_diagnostics,
            damping,
        );
        let candidate_materially_changed = iterate_has_material_change(&state, &candidate);
        // Bundle N checkpoint 5 (N4): confirmed by checkpoint-4 forensics to be the dominant
        // cause of "regression" on `--opm-aligned` runs — 11 of 12 failures on the tracked
        // small case were this exit firing on a near-zero-move update, aborting the substep
        // before OPM's relaxed exhaustion tiers ever got a chance to accept the (genuinely
        // plateaued) state. OPM has no "was the raw update materially small" validity check
        // at all — a near-zero update is still a normal Newton step; the loop simply keeps
        // iterating and lets the entry check (or post-loop exhaustion) decide. Required only
        // under Legacy.
        let candidate_is_valid = damping.is_finite()
            && damping > 0.0
            && (opm_aligned || candidate_materially_changed)
            && candidate.is_finite()
            && (y2b3_primary_variable_lifecycle || candidate.respects_basic_bounds(sim))
            // OpmAligned: the per-cell chop bounds the update by construction (§9.2 limits,
            // not the Legacy max_pressure/saturation_change options this check enforces).
            && (opm_aligned || candidate_respects_update_bounds(&state, &candidate, options));
        let iteration_suffix = format!(
            "{}{}",
            if stagnation_count > 0 {
                format!(" stag={}", stagnation_count)
            } else {
                String::new()
            },
            if !candidate_is_valid {
                " [invalid-step]"
            } else {
                ""
            }
        );

        fim_trace!(
            sim,
            options.verbose,
            "    iter {:>2}: res={:.3e} upd={:.3e} damp={:.4} step_dP={:.2} step_dS={:.4} linear_iters={}{}{}{}{} fam=[{}]{}",
            iteration,
            current_norm,
            final_update_inf_norm,
            damping,
            candidate_pressure_change,
            candidate_saturation_change,
            linear_report.iterations,
            if used_fallback { " [fallback]" } else { "" },
            linear_report_trace_suffix(&linear_report, requested_linear_kind),
            history_stabilization
                .as_ref()
                .map(nonlinear_history_stabilization_trace)
                .unwrap_or_default(),
            iteration_suffix,
            residual_family_trace(&residual_diagnostics),
            residual_detail
                .as_ref()
                .map(|detail| format!(" detail=[{}]", detail))
                .unwrap_or_default()
        );
        if let Some(ref effective_move_trace) = effective_move_trace {
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: HOTSPOT effective-move floor {}",
                iteration,
                effective_move_trace,
            );
        }
        previous_effective_move_floor_site =
            effective_move_trace.as_ref().map(|_| current_hotspot_site);

        previous_hotspot_site = Some(current_hotspot_site);

        if !candidate_is_valid {
            let accepted_diagnostics = evaluate_accepted_state_convergence(
                sim,
                previous_state,
                &state,
                &topology,
                dt_days,
            );
            // Bundle N checkpoint 3: this Legacy rescue has no OPM analog and uses
            // Legacy-scaled thresholds (`NOOP_ENTRY_EXACT_FACTOR`/`ENTRY_RESIDUAL_GUARD_FACTOR`)
            // that don't apply under `OpmAligned` — never take it there. A zero-move/invalid
            // candidate under `OpmAligned` instead falls straight through to the "DAMPING
            // FAILED"-style genuine-failure return below, matching OPM's own behavior of a
            // stalled Newton iteration eventually failing the substep for the outer retry
            // ladder to handle (dt cut), rather than being locally rescued.
            if !opm_aligned
                && zero_move_appleyard_acceptance_allows(
                    candidate_materially_changed,
                    accepted_diagnostics.residual_inf_norm,
                    accepted_diagnostics.material_balance_inf_norm,
                    options,
                )
            {
                final_update_inf_norm = 0.0;
                let use_guard_band =
                    accepted_diagnostics.residual_inf_norm > options.residual_tolerance;
                fim_trace!(
                    sim,
                    options.verbose,
                    "    iter {:>2}: ZERO-MOVE APPLEYARD ACCEPTED res={:.3e} mb={:.3e}{} fam=[{}] mb=[{}]{}",
                    iteration,
                    accepted_diagnostics.residual_inf_norm,
                    accepted_diagnostics.material_balance_inf_norm,
                    if use_guard_band {
                        format!(" (entry guard {:.1}x)", ENTRY_RESIDUAL_GUARD_FACTOR)
                    } else {
                        String::new()
                    },
                    residual_family_trace(&accepted_diagnostics.residual_diagnostics),
                    global_material_balance_trace(
                        &accepted_diagnostics.material_balance_diagnostics
                    ),
                    accepted_diagnostics
                        .residual_detail
                        .as_ref()
                        .map(|detail| format!(" detail=[{}]", detail))
                        .unwrap_or_default()
                );
                maybe_trace_small_dt_hotspot_neighborhood(
                    sim,
                    options.verbose,
                    "accepted",
                    dt_days,
                    previous_state,
                    &accepted_diagnostics.state,
                    current_hotspot_site,
                );
                return FimStepReport {
                    accepted_state: accepted_diagnostics.state,
                    converged: true,
                    newton_iterations: iteration + 1,
                    final_residual_inf_norm: accepted_diagnostics.residual_inf_norm,
                    final_material_balance_inf_norm: accepted_diagnostics.material_balance_inf_norm,
                    final_update_inf_norm,
                    last_linear_report: Some(linear_report),
                    accepted_hotspot_site: Some(current_hotspot_site),
                    failure_diagnostics: None,
                    retry_factor: 1.0,
                    total_time_ms: total_timer.elapsed_ms(),
                    assembly_ms,
                    property_eval_ms,
                    linear_solve_time_ms,
                    linear_preconditioner_build_time_ms,
                    state_update_ms,
                };
            }

            let failure_diagnostics = classify_retry_failure_with_site(
                Some(&linear_report),
                &residual_diagnostics,
                current_hotspot_site,
            );
            let retry_factor = retry_factor_for_failure(Some(&failure_diagnostics));
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: DAMPING FAILED — invalid bounded Appleyard candidate (current={:.3e}, step_dP={:.2}, step_dS={:.4}){}",
                iteration,
                current_norm,
                candidate_pressure_change,
                candidate_saturation_change,
                retry_failure_trace_suffix(&failure_diagnostics)
            );
            return FimStepReport {
                accepted_state: state,
                converged: false,
                newton_iterations: iteration + 1,
                final_residual_inf_norm: final_residual_inf_norm.unwrap_or(f64::INFINITY),
                final_material_balance_inf_norm,
                final_update_inf_norm,
                last_linear_report: Some(linear_report),
                accepted_hotspot_site: None,
                failure_diagnostics: Some(failure_diagnostics),
                retry_factor,
                total_time_ms: total_timer.elapsed_ms(),
                assembly_ms,
                property_eval_ms,
                linear_solve_time_ms,
                linear_preconditioner_build_time_ms,
                state_update_ms,
            };
        }

        if let Some(switched) = candidate_primary_variables_switched {
            primary_variables_switched = switched;
        }
        state = candidate;
    }

    let final_assembly = assemble_fim_system(
        sim,
        previous_state,
        &state,
        &FimAssemblyOptions {
            dt_days,
            include_wells: true,
            assemble_residual_only: true,
            topology: Some(&topology),
        },
    );
    assembly_ms += final_assembly.timing.residual_ms
        + final_assembly.timing.sensitivity_eval_ms
        + final_assembly.timing.jacobian_ms;
    property_eval_ms += final_assembly.timing.property_eval_ms;
    final_residual_inf_norm = Some(scaled_residual_inf_norm(
        &final_assembly.residual,
        &final_assembly.equation_scaling,
    ));
    let final_residual_diagnostics =
        residual_family_diagnostics(&final_assembly.residual, &final_assembly.equation_scaling);
    let final_residual_detail = residual_family_detail_trace(
        sim,
        previous_state,
        &state,
        &topology,
        dt_days,
        &final_residual_diagnostics,
    );
    let final_material_balance_diagnostics = global_material_balance_diagnostics(
        &final_assembly.residual,
        &final_assembly.equation_scaling,
    );
    final_material_balance_inf_norm = final_material_balance_diagnostics.global_value;
    // Bundle N checkpoint 3: this is genuinely OPM's `iteration == maxIter` case (the Newton
    // budget is exhausted), so the final-iteration relaxed MB/CNV tiers always apply here
    // under `OpmAligned` — matching design doc §9.1 exactly, not just approximating it.
    let post_loop_converged = if opm_aligned {
        cnv_mb_diagnostics(sim, &state, &final_assembly.residual, true).would_accept
    } else {
        final_residual_inf_norm.unwrap_or(f64::INFINITY) <= options.residual_tolerance
            && final_material_balance_inf_norm <= options.material_balance_tolerance
    };
    if post_loop_converged {
        fim_trace!(
            sim,
            options.verbose,
            "    post-loop: CONVERGED on final residual check res={:.3e} mb={:.3e} fam=[{}] mb=[{}]{}",
            final_residual_inf_norm.unwrap_or(f64::INFINITY),
            final_material_balance_inf_norm,
            residual_family_trace(&final_residual_diagnostics),
            global_material_balance_trace(&final_material_balance_diagnostics),
            final_residual_detail
                .as_ref()
                .map(|detail| format!(" detail=[{}]", detail))
                .unwrap_or_default()
        );
        return FimStepReport {
            accepted_state: state,
            converged: true,
            newton_iterations: options.max_newton_iterations,
            final_residual_inf_norm: final_residual_inf_norm.unwrap_or(f64::INFINITY),
            final_material_balance_inf_norm,
            final_update_inf_norm,
            last_linear_report,
            accepted_hotspot_site: Some(residual_hotspot_site(
                sim,
                &topology,
                &final_residual_diagnostics.global,
            )),
            failure_diagnostics: None,
            retry_factor: 1.0,
            total_time_ms: total_timer.elapsed_ms(),
            assembly_ms,
            property_eval_ms,
            linear_solve_time_ms,
            linear_preconditioner_build_time_ms,
            state_update_ms,
        };
    }

    let failure_diagnostics = classify_retry_failure_with_site(
        last_linear_report.as_ref(),
        &final_residual_diagnostics,
        residual_hotspot_site(sim, &topology, &final_residual_diagnostics.global),
    );
    let retry_factor = retry_factor_for_failure(Some(&failure_diagnostics));
    fim_trace!(
        sim,
        options.verbose,
        "    post-loop: NOT CONVERGED after {} iterations, res={:.3e} mb={:.3e} upd={:.3e} fam=[{}] mb=[{}]{}{}",
        options.max_newton_iterations,
        final_residual_inf_norm.unwrap_or(f64::INFINITY),
        final_material_balance_inf_norm,
        final_update_inf_norm,
        residual_family_trace(&final_residual_diagnostics),
        global_material_balance_trace(&final_material_balance_diagnostics),
        final_residual_detail
            .as_ref()
            .map(|detail| format!(" detail=[{}]", detail))
            .unwrap_or_default(),
        retry_failure_trace_suffix(&failure_diagnostics)
    );

    FimStepReport {
        accepted_state: state,
        converged: false,
        newton_iterations: options.max_newton_iterations,
        final_residual_inf_norm: final_residual_inf_norm.unwrap_or(f64::INFINITY),
        final_material_balance_inf_norm,
        final_update_inf_norm,
        last_linear_report,
        accepted_hotspot_site: None,
        failure_diagnostics: Some(failure_diagnostics),
        retry_factor,
        total_time_ms: total_timer.elapsed_ms(),
        assembly_ms,
        property_eval_ms,
        linear_solve_time_ms,
        linear_preconditioner_build_time_ms,
        state_update_ms,
    }
}

#[cfg(test)]
mod tests {
    use nalgebra::DVector;

    use crate::ReservoirSimulator;
    use crate::fim::assembly::{FimAssemblyOptions, assemble_fim_system};
    use crate::fim::scaling::EquationScaling;
    use crate::fim::state::FimState;
    use crate::pvt::{PvtRow, PvtTable};

    use super::*;

    #[test]
    fn opm_relaxed_linear_acceptance_is_backend_neutral() {
        let mut report = FimLinearSolveReport {
            solution: DVector::from_element(1, 1.0),
            converged: false,
            iterations: 1,
            rhs_norm: 10.0,
            final_residual_norm: 0.05,
            // Sparse LU intentionally has no iterative-failure payload. The relaxed decision
            // must be identical to a CPR report with the same returned correction quality.
            failure_diagnostics: None,
            used_fallback: false,
            backend_used: FimLinearSolverKind::SparseLuDebug,
            cpr_diagnostics: None,
            total_time_ms: 0.0,
            preconditioner_build_time_ms: 0.0,
        };

        assert!(opm_accepts_relaxed_linear_report(&report));

        report.final_residual_norm = 0.1;
        assert!(!opm_accepts_relaxed_linear_report(&report));

        report.final_residual_norm = 0.05;
        report.solution[0] = f64::NAN;
        assert!(!opm_accepts_relaxed_linear_report(&report));
    }

    fn two_cell_state_for_chop(regimes: [HydrocarbonState; 2]) -> FimState {
        let sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        let mut state = FimState::from_simulator(&sim);
        for (idx, regime) in regimes.iter().enumerate() {
            state.cells[idx].pressure_bar = 200.0;
            state.cells[idx].sw = 0.3;
            state.cells[idx].hydrocarbon_var = match regime {
                HydrocarbonState::Saturated => 0.1,       // Sg meaning
                HydrocarbonState::Undersaturated => 50.0, // Rs meaning
            };
            state.cells[idx].regime = *regime;
        }
        state
    }

    /// Boundary-only derivative evidence for Y2b.  This is deliberately a
    /// one-cell gas injector so each reported reservoir row is the injector's
    /// connected cell and the perforation row remains present.  The exact
    /// 10x10x3 repro supplies the live trajectory; this fixture supplies the
    /// controlled `bound-eps/bound/bound+eps` probes for every active clamp.
    #[test]
    fn y2b_boundary_injector_fixture_reports_ad_legacy_and_one_sided_fd() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.15, 0.15, 0.05, 0.05, 0.2, 2.0, 2.0, 2.0, 1e-5, 1.0, 0.95,
        )
        .unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_injected_fluid("gas").unwrap();
        sim.set_rate_controlled_wells(true);
        sim.set_target_well_rates(100.0, 0.0).unwrap();
        sim.add_well(0, 0, 0, 250.0, 0.1, 0.0, true).unwrap();
        let topology = build_well_topology(&sim);
        let perf_idx = 0;
        let cell_idx = 0;
        let options = FimAssemblyOptions {
            dt_days: 0.25,
            include_wells: true,
            assemble_residual_only: false,
            topology: Some(&topology),
        };
        let eps = 1e-7;
        let cases = [
            ("swc", 0.15, 0.10, 1usize),
            ("sg_zero", 0.30, 0.0, 2usize),
            ("sw_upper", 0.80, 0.0, 1usize),
            ("sg_upper", 0.15, 0.65, 2usize),
        ];

        for (boundary, sw, sg, boundary_column) in cases {
            for (sample, signed_eps) in [("minus", -eps), ("exact", 0.0), ("plus", eps)] {
                let mut state = FimState::from_simulator(&sim);
                state.cells[cell_idx].sw = sw;
                state.cells[cell_idx].hydrocarbon_var = sg;
                state.cells[cell_idx].regime = HydrocarbonState::Saturated;
                if boundary_column == 1 {
                    state.cells[cell_idx].sw += signed_eps;
                } else {
                    state.cells[cell_idx].hydrocarbon_var += signed_eps;
                }
                let previous_state = state.clone();
                let ad = crate::fim::assembly_ad::assemble_fim_system_ad(
                    &sim,
                    &previous_state,
                    &state,
                    &options,
                );
                let legacy = assemble_fim_system(&sim, &previous_state, &state, &options);
                let rows = [
                    (
                        "rate_consistency",
                        state.perforation_equation_offset(perf_idx),
                    ),
                    ("water", equation_offset(cell_idx, 0)),
                    ("oil", equation_offset(cell_idx, 1)),
                    ("gas", equation_offset(cell_idx, 2)),
                ];
                let columns = [
                    ("p", unknown_offset(cell_idx, 0)),
                    ("sw", unknown_offset(cell_idx, 1)),
                    ("hc", unknown_offset(cell_idx, 2)),
                    ("bhp", state.well_bhp_unknown_offset(0)),
                    ("q", state.perforation_rate_unknown_offset(perf_idx)),
                ];
                for (row_label, row) in rows {
                    assert!(ad.residual[row].is_finite() && legacy.residual[row].is_finite());
                    for (column_label, column) in columns {
                        let h = y2a_finite_difference_step(&state, column);
                        let mut plus = state.clone();
                        let mut minus = state.clone();
                        y2a_perturb_unknown(&mut plus, column, h);
                        y2a_perturb_unknown(&mut minus, column, -h);
                        let plus_residual =
                            assemble_fim_system(&sim, &previous_state, &plus, &options).residual
                                [row];
                        let minus_residual =
                            assemble_fim_system(&sim, &previous_state, &minus, &options).residual
                                [row];
                        let central = (plus_residual - minus_residual) / (2.0 * h);
                        let forward = (plus_residual - legacy.residual[row]) / h;
                        let backward = (legacy.residual[row] - minus_residual) / h;
                        let ad_derivative = ad.jacobian.get(row, column).copied().unwrap_or(0.0);
                        let legacy_derivative =
                            legacy.jacobian.get(row, column).copied().unwrap_or(0.0);
                        assert!(
                            ad_derivative.is_finite()
                                && legacy_derivative.is_finite()
                                && central.is_finite()
                                && forward.is_finite()
                                && backward.is_finite(),
                            "{boundary}/{sample} {row_label}/{column_label} must remain finite"
                        );
                        println!(
                            "Y2B fixture boundary={boundary} sample={sample} row={row_label} col={column_label} residual_ad={:.12e} residual_legacy={:.12e} ad={ad_derivative:.12e} legacy={legacy_derivative:.12e} central={central:.12e} forward={forward:.12e} backward={backward:.12e}",
                            ad.residual[row], legacy.residual[row],
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn opm_per_cell_chop_scales_only_the_violating_cell_and_counts_implied_so() {
        let state = two_cell_state_for_chop([HydrocarbonState::Saturated; 2]);
        let mut update = DVector::zeros(state.n_unknowns());
        // Cell 0: dSw=0.15, dSg=0.15 → implied dSo=-0.3 is the max → satAlpha=0.2/0.3.
        update[1] = 0.15;
        update[2] = 0.15;
        // Cell 1: small, untouched.
        update[4] = 0.01;
        update[5] = -0.02;

        let chopped = opm_per_cell_chopped_update(&state, &update, 1.0);

        let alpha = OPM_DS_MAX / 0.3;
        assert!((chopped[1] - 0.15 * alpha).abs() < 1e-12);
        assert!((chopped[2] - 0.15 * alpha).abs() < 1e-12);
        // Neither dSw=0.15 nor dSg=0.15 alone exceeds 0.2 — only the implied So does;
        // a port missing the implied-So term would leave cell 0 unchopped.
        assert!(alpha < 1.0);
        // Cell 1 must be exactly untouched — no global coupling.
        assert_eq!(chopped[4], 0.01);
        assert_eq!(chopped[5], -0.02);
    }

    #[test]
    fn opm_per_cell_chop_clamps_pressure_relative_and_independent_of_saturation() {
        let state = two_cell_state_for_chop([HydrocarbonState::Saturated; 2]);
        let mut update = DVector::zeros(state.n_unknowns());
        update[0] = -100.0; // |dp| > 0.3 * 200 = 60 → clamp to -60
        update[1] = 0.5; // dSw drives satAlpha = 0.2/0.5 = 0.4
        update[3] = 30.0; // cell 1: within cap, untouched

        let chopped = opm_per_cell_chopped_update(&state, &update, 1.0);

        assert!(
            (chopped[0] - (-60.0)).abs() < 1e-12,
            "dp clamped to signum*0.3*p"
        );
        assert!((chopped[1] - 0.5 * 0.4).abs() < 1e-12);
        assert_eq!(chopped[3], 30.0);
    }

    #[test]
    fn opm_per_cell_chop_guards_rs_nonnegative_without_sat_alpha() {
        let state = two_cell_state_for_chop([
            HydrocarbonState::Undersaturated,
            HydrocarbonState::Undersaturated,
        ]);
        let mut update = DVector::zeros(state.n_unknowns());
        // Rs delta would take hydrocarbon_var (50.0) to -30 → guard to exactly -50.
        update[2] = -80.0;
        // dSw alone at 0.25 → implied dSo=-0.25 → satAlpha=0.8; Rs delta must NOT be
        // scaled by satAlpha (it is not a saturation).
        update[1] = 0.25;
        update[5] = -10.0; // cell 1: Rs delta within bounds, untouched

        let chopped = opm_per_cell_chopped_update(&state, &update, 1.0);

        assert!(
            (chopped[2] - (-50.0)).abs() < 1e-12,
            "Rs guard: current + delta >= 0"
        );
        assert!((chopped[1] - 0.25 * 0.8).abs() < 1e-12);
        assert_eq!(chopped[5], -10.0);
    }

    #[test]
    fn opm_per_cell_chop_applies_relaxation_before_chopping() {
        let state = two_cell_state_for_chop([HydrocarbonState::Saturated; 2]);
        let mut update = DVector::zeros(state.n_unknowns());
        update[1] = 0.3; // raw dSw exceeds dsMax, but relax=0.5 brings it to 0.15 → no chop

        let chopped = opm_per_cell_chopped_update(&state, &update, 0.5);

        assert!(
            (chopped[1] - 0.15).abs() < 1e-12,
            "relaxation applies first, then chop"
        );
    }

    #[test]
    fn opm_per_cell_chop_clamps_well_bhp_relative_when_increasing() {
        // An INCREASING dBHP isolates the relative clamp from the positivity floor (a
        // decreasing dBHP at dbhp_max_rel=1.0 always drives next_bhp to exactly 0, which is
        // always below the floor too — covered separately below).
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        let state = FimState::from_simulator(&sim);
        let mut update = DVector::zeros(state.n_unknowns());
        // Raw dBHP=+600 exceeds dbhp-max-rel=1.0 * bhp(500) = 500 → clamp to +500.
        update[state.well_bhp_unknown_offset(0)] = 600.0;

        let chopped = opm_per_cell_chopped_update(&state, &update, 1.0);

        assert!(
            (chopped[state.well_bhp_unknown_offset(0)] - 500.0).abs() < 1e-12,
            "dBHP clamped to +dbhp_max_rel*bhp_current"
        );
    }

    #[test]
    fn opm_per_cell_chop_well_bhp_within_cap_is_untouched() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        let state = FimState::from_simulator(&sim);
        let mut update = DVector::zeros(state.n_unknowns());
        update[state.well_bhp_unknown_offset(0)] = 200.0; // within 1.0*500 cap

        let chopped = opm_per_cell_chopped_update(&state, &update, 1.0);

        assert_eq!(chopped[state.well_bhp_unknown_offset(0)], 200.0);
    }

    #[test]
    fn opm_per_cell_chop_well_bhp_floors_above_lower_limit() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 1.5, 0.1, 0.0, true).unwrap();
        let state = FimState::from_simulator(&sim);
        let mut update = DVector::zeros(state.n_unknowns());
        // bhp=1.5; dbhp_max_rel*1.5=1.5 caps |dbhp| at 1.5, which alone would take bhp to 0.0 —
        // below OPM_BHP_LOWER_LIMIT_BAR (1.0) — so the floor must bind instead of the raw clamp.
        update[state.well_bhp_unknown_offset(0)] = -3.0;

        let chopped = opm_per_cell_chopped_update(&state, &update, 1.0);

        let next_bhp = 1.5 + chopped[state.well_bhp_unknown_offset(0)];
        assert!(
            (next_bhp - OPM_BHP_LOWER_LIMIT_BAR).abs() < 1e-12,
            "next bhp floored at OPM_BHP_LOWER_LIMIT_BAR, got {next_bhp}"
        );
    }

    #[test]
    fn cnv_mb_from_parts_matches_hand_computed_values() {
        // Two cells, pv = [100, 300] m³; FVFs chosen distinct per component so a mix-up
        // between components or between B_avg and per-cell B shows up in the numbers.
        // B_avg = [(1.0+1.0)/2, (1.2+1.4)/2, (0.01+0.03)/2] = [1.0, 1.3, 0.02].
        let residual = DVector::from_vec(vec![
            2.0, -0.6, 0.0, // cell 0: water, oil, gas
            -1.0, 0.9, 0.0, // cell 1
        ]);
        let pv = [100.0, 300.0];
        let fvf = [[1.0, 1.2, 0.01], [1.0, 1.4, 0.03]];

        let d = cnv_mb_from_parts(&residual, &pv, &fvf, false);

        // maxCoeff per component: water max(2/100, 1/300)=0.02; oil max(0.6/100, 0.9/300)=0.006.
        // CNV = B_avg * maxCoeff.
        assert!((d.cnv[0] - 1.0 * 0.02).abs() < 1e-12);
        assert!((d.cnv[1] - 1.3 * 0.006).abs() < 1e-12);
        assert_eq!(d.cnv[2], 0.0);
        // MB = |B_avg * signed sum| / pvSum; water sum = 1.0, oil sum = 0.3, pvSum = 400.
        assert!((d.mb[0] - 1.0 * 1.0 / 400.0).abs() < 1e-12);
        assert!((d.mb[1] - 1.3 * 0.3 / 400.0).abs() < 1e-12);
        // Cell 0's own max CNV = max(2*1.0, 0.6*1.3)/100 = 0.02 > 1e-2 → violating.
        // Cell 1's = max(1*1.0, 0.9*1.3)/300 = 0.0039 < 1e-2 → not violating.
        assert!((d.violating_pv_fraction - 100.0 / 400.0).abs() < 1e-12);
        assert!(!d.pv_rule_relaxes); // 25% of PV violating >> 3%
        assert!(!d.would_accept_strict);
        assert!(!d.would_accept);
    }

    #[test]
    fn cnv_pv_rule_relaxes_when_violating_pv_below_three_percent() {
        // Cell 0 is a tiny plateau cell (1% of PV) with a CNV way above strict tolerance
        // but below the relaxed 1.0; cell 1 is huge and clean. MB stays tiny because the
        // residual is small in absolute terms. OPM accepts this state; the strict check
        // does not — exactly the water@1215 heavy-case pattern.
        let residual = DVector::from_vec(vec![
            0.5, 0.0, 0.0, // cell 0: water CNV = 0.5/10 = 0.05 (> 1e-2, < 1.0)
            0.0, 0.0, 0.0, // cell 1: clean
        ]);
        let pv = [10.0, 990.0];
        let fvf = [[1.0, 1.0, 1.0], [1.0, 1.0, 1.0]];

        let d = cnv_mb_from_parts(&residual, &pv, &fvf, false);

        assert!((d.cnv[0] - 0.05).abs() < 1e-12);
        assert!((d.violating_pv_fraction - 0.01).abs() < 1e-12);
        assert!(d.pv_rule_relaxes);
        assert!(!d.would_accept_strict, "0.05 > strict 1e-2");
        // MB = 0.5/1000 = 5e-4 > 1e-7 → even the pv-relaxed accept must fail on MB.
        assert!(!d.would_accept);

        // Shrink the residual so MB passes while local CNV still violates strict:
        // water residual 2e-4 on pv=10 → CNV 2e-5... too small. Use pv weighting instead:
        // keep CNV at 0.05 but cancel MB with an opposite residual in the big cell.
        let residual = DVector::from_vec(vec![
            0.5, 0.0, 0.0, //
            -0.5, 0.0, 0.0, // cancels the sum → MB = 0 exactly
        ]);
        let d = cnv_mb_from_parts(&residual, &pv, &fvf, false);
        assert_eq!(d.mb[0], 0.0);
        assert!(!d.would_accept_strict, "local CNV 0.05 still above strict");
        assert!(
            d.would_accept,
            "1% violating PV < 3% rule → relaxed CNV 1.0 applies, MB clean → OPM accepts"
        );
    }

    #[test]
    fn cnv_mb_relax_final_iteration_applies_relaxed_tiers_unconditionally() {
        // A state that fails BOTH strict tolerances and the PV-rule tier (>3% of PV
        // violating strict CNV) — the pv-relaxed path alone must not accept it. Both cells
        // have CNV 25/500 = 0.05 (> strict 1e-2); residuals cancel so MB = 0.
        let residual = DVector::from_vec(vec![25.0, 0.0, 0.0, -25.0, 0.0, 0.0]);
        let pv = [500.0, 500.0];
        let fvf = [[1.0, 1.0, 1.0], [1.0, 1.0, 1.0]];

        let not_final = cnv_mb_from_parts(&residual, &pv, &fvf, false);
        assert!(!not_final.pv_rule_relaxes, "100% violating PV >> 3% rule");
        assert!(
            !not_final.would_accept,
            "CNV 0.05 > strict 1e-2, PV rule doesn't apply"
        );

        let final_iter = cnv_mb_from_parts(&residual, &pv, &fvf, true);
        assert!(
            final_iter.would_accept,
            "relax_final_iteration unconditionally applies CNV-relaxed(1.0)/MB-relaxed(1e-6), \
             independent of the PV rule"
        );
    }

    #[test]
    fn zero_residual_scaffold_converges_in_one_newton_step() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        let state = FimState::from_simulator(&sim);

        let report = run_fim_timestep(&mut sim, &state, &state, 1.0, &FimNewtonOptions::default());

        assert!(report.converged);
        assert_eq!(report.newton_iterations, 1);
        assert_eq!(report.retry_factor, 1.0);
        assert!(report.final_residual_inf_norm <= 1e-12);
        assert!(report.final_material_balance_inf_norm <= 1e-12);
        assert!(report.final_update_inf_norm <= 1e-12);
    }

    #[test]
    fn local_closed_system_newton_recovers_previous_state_from_perturbed_iterate() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_mode_enabled(true);
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 100.0,
                    rs_m3m3: 10.0,
                    bo_m3m3: 1.1,
                    mu_o_cp: 1.2,
                    bg_m3m3: 0.01,
                    mu_g_cp: 0.02,
                },
                PvtRow {
                    p_bar: 200.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.0,
                    mu_o_cp: 1.1,
                    bg_m3m3: 0.005,
                    mu_g_cp: 0.02,
                },
            ],
            sim.pvt.c_o,
        ));
        let previous_state = FimState::from_simulator(&sim);
        let mut iterate = previous_state.clone();
        iterate.cells[0].pressure_bar += 5.0;
        iterate.cells[0].sw += 0.02;

        let report = run_fim_timestep(
            &mut sim,
            &previous_state,
            &iterate,
            1.0,
            &FimNewtonOptions::default(),
        );

        assert!(report.converged);
        assert!(
            (report.accepted_state.cells[0].pressure_bar - previous_state.cells[0].pressure_bar)
                .abs()
                < 0.5
        );
        assert!((report.accepted_state.cells[0].sw - previous_state.cells[0].sw).abs() < 1e-3);
    }

    #[test]
    fn rate_controlled_well_bhp_unknown_is_solved_implicitly() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        let previous_state = FimState::from_simulator(&sim);

        let report = run_fim_timestep(
            &mut sim,
            &previous_state,
            &previous_state,
            1.0,
            &FimNewtonOptions::default(),
        );

        assert!(report.converged);
        assert_eq!(report.accepted_state.well_bhp.len(), 2);
        assert_eq!(report.accepted_state.perforation_rates_m3_day.len(), 2);
    }

    #[test]
    fn entry_guard_does_not_accept_unchanged_previous_state() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        let previous_state = FimState::from_simulator(&sim);

        let assembly = assemble_fim_system(
            &sim,
            &previous_state,
            &previous_state,
            &FimAssemblyOptions {
                dt_days: 0.01,
                include_wells: true,
                assemble_residual_only: false,
                topology: None,
            },
        );
        let residual_norm =
            scaled_residual_inf_norm(&assembly.residual, &assembly.equation_scaling);
        assert!(residual_norm.is_finite() && residual_norm > 0.0);

        let options = FimNewtonOptions {
            residual_tolerance: residual_norm * 0.75,
            ..FimNewtonOptions::default()
        };

        let report = run_fim_timestep(&mut sim, &previous_state, &previous_state, 0.01, &options);

        assert!(
            !report.converged
                || iterate_has_material_change(&previous_state, &report.accepted_state),
            "unchanged previous state must not be accepted as converged inside the residual guard band"
        );
        if report.converged {
            assert!(
                report.final_update_inf_norm > 0.0,
                "guarded residual acceptance should not report a zero-update shortcut for an unchanged previous state"
            );
        }
    }

    #[test]
    fn iterate_has_material_change_detects_well_and_perforation_updates() {
        let previous_state = FimState {
            cells: vec![crate::fim::state::FimCellState {
                pressure_bar: 250.0,
                sw: 0.25,
                hydrocarbon_var: 0.0,
                regime: crate::fim::state::HydrocarbonState::Saturated,
            }],
            well_bhp: vec![300.0],
            perforation_rates_m3_day: vec![-150.0],
        };

        let mut bhp_changed = previous_state.clone();
        bhp_changed.well_bhp[0] += 1.0;
        assert!(iterate_has_material_change(&previous_state, &bhp_changed));

        let mut perf_changed = previous_state.clone();
        perf_changed.perforation_rates_m3_day[0] += 1.0;
        assert!(iterate_has_material_change(&previous_state, &perf_changed));
    }

    #[test]
    fn stagnation_acceptance_requires_material_change() {
        let options = FimNewtonOptions::default();
        assert!(!stagnation_acceptance_allows(
            false,
            options.residual_tolerance * 2.0,
            options.material_balance_tolerance * 0.5,
            options.update_tolerance * 0.5,
            &options,
        ));
    }

    #[test]
    fn stagnation_acceptance_allows_near_converged_state() {
        let options = FimNewtonOptions::default();
        assert!(stagnation_acceptance_allows(
            true,
            options.residual_tolerance * 6.0,
            options.material_balance_tolerance * 0.5,
            options.update_tolerance * 0.1,
            &options,
        ));
    }

    #[test]
    fn zero_move_appleyard_acceptance_allows_guarded_unchanged_state() {
        let options = FimNewtonOptions::default();
        assert!(zero_move_appleyard_acceptance_allows(
            false,
            options.residual_tolerance
                * NOOP_ENTRY_EXACT_FACTOR
                * ENTRY_RESIDUAL_GUARD_FACTOR
                * 0.9,
            options.material_balance_tolerance
                * NOOP_ENTRY_EXACT_FACTOR
                * ENTRY_RESIDUAL_GUARD_FACTOR
                * 0.9,
            &options,
        ));
    }

    #[test]
    fn zero_move_appleyard_acceptance_rejects_changed_or_out_of_band_state() {
        let options = FimNewtonOptions::default();
        assert!(!zero_move_appleyard_acceptance_allows(
            true,
            options.residual_tolerance * NOOP_ENTRY_EXACT_FACTOR * ENTRY_RESIDUAL_GUARD_FACTOR,
            options.material_balance_tolerance
                * NOOP_ENTRY_EXACT_FACTOR
                * ENTRY_RESIDUAL_GUARD_FACTOR,
            &options,
        ));
        assert!(!zero_move_appleyard_acceptance_allows(
            false,
            options.residual_tolerance
                * NOOP_ENTRY_EXACT_FACTOR
                * ENTRY_RESIDUAL_GUARD_FACTOR
                * 1.1,
            options.material_balance_tolerance
                * NOOP_ENTRY_EXACT_FACTOR
                * ENTRY_RESIDUAL_GUARD_FACTOR,
            &options,
        ));
        assert!(!zero_move_appleyard_acceptance_allows(
            false,
            options.residual_tolerance * NOOP_ENTRY_EXACT_FACTOR * ENTRY_RESIDUAL_GUARD_FACTOR,
            options.material_balance_tolerance
                * NOOP_ENTRY_EXACT_FACTOR
                * ENTRY_RESIDUAL_GUARD_FACTOR
                * 1.1,
            &options,
        ));
    }

    #[test]
    fn stagnation_acceptance_rejects_material_balance_failure() {
        let options = FimNewtonOptions::default();
        assert!(!stagnation_acceptance_allows(
            true,
            options.residual_tolerance * 6.0,
            options.material_balance_tolerance * 2.0,
            options.update_tolerance * 0.1,
            &options,
        ));
    }

    #[test]
    fn stagnation_acceptance_gate_status_reports_update_failure() {
        let options = FimNewtonOptions::default();
        let status = stagnation_acceptance_gate_status(
            true,
            options.residual_tolerance * 6.0,
            options.material_balance_tolerance * 0.5,
            options.update_tolerance * 1.5,
            &options,
        );

        assert!(status.materially_changed);
        assert!(!status.update_ok);
        assert!(status.residual_ok);
        assert!(status.material_balance_ok);
        assert!(!status.allows());
    }

    #[test]
    fn stagnation_acceptance_gate_trace_marks_rejected_limits() {
        let options = FimNewtonOptions::default();
        let status = stagnation_acceptance_gate_status(
            true,
            options.residual_tolerance * 12.0,
            options.material_balance_tolerance * 2.0,
            options.update_tolerance * 1.5,
            &options,
        );

        let trace = stagnation_acceptance_gate_trace(
            status,
            options.residual_tolerance * 12.0,
            options.material_balance_tolerance * 2.0,
            options.update_tolerance * 1.5,
            &options,
        );

        assert!(trace.contains("upd="));
        assert!(trace.contains("res="));
        assert!(trace.contains("mb="));
        assert!(trace.contains("reject"));
    }

    #[test]
    fn guard_band_keeps_material_balance_limit_strict() {
        let options = FimNewtonOptions::default();
        let (residual_limit, material_balance_limit) = convergence_limits(&options, true);

        assert_eq!(
            residual_limit,
            options.residual_tolerance * ENTRY_RESIDUAL_GUARD_FACTOR
        );
        assert_eq!(material_balance_limit, options.material_balance_tolerance);
    }

    #[test]
    fn accepted_state_convergence_rejects_guard_band_material_balance_violation() {
        let diagnostics = AcceptedStateConvergenceDiagnostics {
            state: FimState {
                cells: Vec::new(),
                well_bhp: Vec::new(),
                perforation_rates_m3_day: Vec::new(),
            },
            residual_inf_norm: 1.5e-5,
            residual_diagnostics: ResidualFamilyDiagnostics {
                water: ResidualFamilyPeak {
                    family: ResidualRowFamily::Water,
                    scaled_value: 1.5e-5,
                    row: 0,
                    item_index: 0,
                },
                oil_component: ResidualFamilyPeak {
                    family: ResidualRowFamily::OilComponent,
                    scaled_value: 1.0e-5,
                    row: 1,
                    item_index: 0,
                },
                gas_component: ResidualFamilyPeak {
                    family: ResidualRowFamily::GasComponent,
                    scaled_value: 0.5e-5,
                    row: 2,
                    item_index: 0,
                },
                well_constraint: None,
                perforation_flow: None,
                global: ResidualFamilyPeak {
                    family: ResidualRowFamily::Water,
                    scaled_value: 1.5e-5,
                    row: 0,
                    item_index: 0,
                },
            },
            residual_detail: None,
            material_balance_inf_norm: 1.5e-5,
            material_balance_diagnostics: GlobalMaterialBalanceDiagnostics {
                water: 1.5e-5,
                oil_component: 1.0e-5,
                gas_component: 0.5e-5,
                global_family: ResidualRowFamily::Water,
                global_value: 1.5e-5,
            },
        };
        let options = FimNewtonOptions::default();
        let (residual_limit, material_balance_limit) = convergence_limits(&options, true);

        assert!(diagnostics.residual_inf_norm <= residual_limit);
        assert!(diagnostics.material_balance_inf_norm > material_balance_limit);
        assert!(!accepted_state_meets_convergence(
            &diagnostics,
            residual_limit,
            material_balance_limit,
        ));
    }

    #[test]
    fn scaled_update_peak_reports_dominant_family() {
        let update = DVector::from_vec(vec![2.0, 0.1, 0.05, 30.0, 0.2]);
        let scaling = crate::fim::scaling::VariableScaling {
            pressure: vec![100.0],
            sw: vec![1.0],
            hydrocarbon_var: vec![1.0],
            well_bhp: vec![1000.0],
            perforation_rate: vec![1.0],
        };

        let peak = scaled_update_peak(&update, &scaling);

        assert_eq!(peak.family, UpdateVariableFamily::PerforationRate);
        assert!((peak.scaled_value - 0.2).abs() < 1e-12);
        assert_eq!(peak.row, 4);
        assert_eq!(peak.item_index, 0);
    }

    #[test]
    fn scaled_applied_update_peak_reports_effective_family() {
        let state = FimState {
            cells: vec![crate::fim::state::FimCellState {
                pressure_bar: 200.0,
                sw: 0.1,
                hydrocarbon_var: 80.0,
                regime: crate::fim::state::HydrocarbonState::Undersaturated,
            }],
            well_bhp: vec![150.0],
            perforation_rates_m3_day: vec![10.0],
        };

        let candidate = FimState {
            cells: vec![crate::fim::state::FimCellState {
                pressure_bar: 200.5,
                sw: 0.11,
                hydrocarbon_var: 80.0,
                regime: crate::fim::state::HydrocarbonState::Undersaturated,
            }],
            well_bhp: vec![150.0],
            perforation_rates_m3_day: vec![10.2],
        };

        let scaling = crate::fim::scaling::VariableScaling {
            pressure: vec![100.0],
            sw: vec![1.0],
            hydrocarbon_var: vec![100.0],
            well_bhp: vec![1000.0],
            perforation_rate: vec![100.0],
        };

        let peak = scaled_applied_update_peak(&state, &candidate, &scaling);

        assert_eq!(peak.family, UpdateVariableFamily::WaterSaturation);
        assert!((peak.scaled_value - 0.01).abs() < 1e-12);
        assert_eq!(peak.row, 1);
        assert_eq!(peak.item_index, 0);
    }

    #[test]
    fn residual_family_diagnostics_reports_global_peak_family() {
        let residual = DVector::from_vec(vec![5.0, 12.0, 8.0, 4.0, 9.0, 6.0, 3.0, 40.0, 1.0]);
        let scaling = EquationScaling {
            water: vec![10.0, 10.0],
            oil_component: vec![10.0, 10.0],
            gas_component: vec![10.0, 10.0],
            well_constraint: vec![10.0, 5.0],
            perforation_flow: vec![2.0],
        };

        let diagnostics = residual_family_diagnostics(&residual, &scaling);

        assert_eq!(diagnostics.water.item_index, 0);
        assert!((diagnostics.water.scaled_value - 0.5).abs() < 1e-12);
        assert_eq!(diagnostics.oil_component.item_index, 0);
        assert!((diagnostics.oil_component.scaled_value - 1.2).abs() < 1e-12);
        assert_eq!(diagnostics.gas_component.item_index, 0);
        assert!((diagnostics.gas_component.scaled_value - 0.8).abs() < 1e-12);
        assert_eq!(
            diagnostics.well_constraint.expect("well peak").item_index,
            1
        );
        assert!((diagnostics.well_constraint.expect("well peak").scaled_value - 8.0).abs() < 1e-12);
        assert_eq!(
            diagnostics.perforation_flow.expect("perf peak").item_index,
            0
        );
        assert!(
            (diagnostics
                .perforation_flow
                .expect("perf peak")
                .scaled_value
                - 0.5)
                .abs()
                < 1e-12
        );
        assert_eq!(diagnostics.global.family, ResidualRowFamily::WellConstraint);
        assert_eq!(diagnostics.global.row, 7);
        assert_eq!(diagnostics.global.item_index, 1);
        assert!((diagnostics.global.scaled_value - 8.0).abs() < 1e-12);
    }

    #[test]
    fn global_material_balance_diagnostics_normalizes_component_sums() {
        let residual = DVector::from_vec(vec![1.0, -4.0, 9.0, 3.0, 6.0, -3.0, 50.0, -20.0, 7.0]);
        let scaling = EquationScaling {
            water: vec![10.0, 10.0],
            oil_component: vec![10.0, 10.0],
            gas_component: vec![10.0, 10.0],
            well_constraint: vec![5.0, 5.0],
            perforation_flow: vec![2.0],
        };

        let diagnostics = global_material_balance_diagnostics(&residual, &scaling);

        assert!((diagnostics.water - 0.2).abs() < 1e-12);
        assert!((diagnostics.oil_component - 0.1).abs() < 1e-12);
        assert!((diagnostics.gas_component - 0.3).abs() < 1e-12);
        assert_eq!(diagnostics.global_family, ResidualRowFamily::GasComponent);
        assert!((diagnostics.global_value - 0.3).abs() < 1e-12);
    }

    #[test]
    fn failure_classification_marks_clean_cpr_failure_as_nonlinear_bad() {
        let diagnostics = ResidualFamilyDiagnostics {
            water: ResidualFamilyPeak {
                family: ResidualRowFamily::Water,
                scaled_value: 1.0,
                row: 0,
                item_index: 0,
            },
            oil_component: ResidualFamilyPeak {
                family: ResidualRowFamily::OilComponent,
                scaled_value: 0.5,
                row: 1,
                item_index: 0,
            },
            gas_component: ResidualFamilyPeak {
                family: ResidualRowFamily::GasComponent,
                scaled_value: 0.25,
                row: 2,
                item_index: 0,
            },
            well_constraint: None,
            perforation_flow: None,
            global: ResidualFamilyPeak {
                family: ResidualRowFamily::Water,
                scaled_value: 1.0,
                row: 0,
                item_index: 0,
            },
        };
        let report = FimLinearSolveReport {
            solution: DVector::zeros(1),
            converged: true,
            iterations: 12,
            rhs_norm: 1.0,
            final_residual_norm: 1e-12,
            failure_diagnostics: None,
            used_fallback: false,
            backend_used: FimLinearSolverKind::FgmresCpr,
            cpr_diagnostics: Some(crate::fim::linear::FimCprDiagnostics {
                coarse_rows: 10,
                coarse_solver: crate::fim::linear::FimPressureCoarseSolverKind::ExactDense,
                smoother_label: "ilu0",
                coarse_applications: 4,
                average_reduction_ratio: 1e-12,
                last_reduction_ratio: 1e-12,
                build_timing: None,
            }),
            total_time_ms: 0.0,
            preconditioner_build_time_ms: 0.0,
        };

        let classified = classify_retry_failure(Some(&report), &diagnostics);

        assert_eq!(classified.class, FimRetryFailureClass::NonlinearBad);
        assert_eq!(classified.dominant_family_label, "water");
        assert_eq!(classified.cpr_last_reduction_ratio, Some(1e-12));
    }

    #[test]
    fn failure_classification_marks_direct_backend_as_nonlinear_bad_when_clean() {
        let diagnostics = ResidualFamilyDiagnostics {
            water: ResidualFamilyPeak {
                family: ResidualRowFamily::Water,
                scaled_value: 1.0,
                row: 0,
                item_index: 0,
            },
            oil_component: ResidualFamilyPeak {
                family: ResidualRowFamily::OilComponent,
                scaled_value: 0.5,
                row: 1,
                item_index: 0,
            },
            gas_component: ResidualFamilyPeak {
                family: ResidualRowFamily::GasComponent,
                scaled_value: 0.25,
                row: 2,
                item_index: 0,
            },
            well_constraint: None,
            perforation_flow: None,
            global: ResidualFamilyPeak {
                family: ResidualRowFamily::Water,
                scaled_value: 1.0,
                row: 0,
                item_index: 0,
            },
        };
        let report = FimLinearSolveReport {
            solution: DVector::zeros(1),
            converged: true,
            iterations: 3,
            rhs_norm: 1.0,
            final_residual_norm: 1e-12,
            failure_diagnostics: None,
            used_fallback: false,
            backend_used: FimLinearSolverKind::DenseLuDebug,
            cpr_diagnostics: None,
            total_time_ms: 0.0,
            preconditioner_build_time_ms: 0.0,
        };

        let classified = classify_retry_failure(Some(&report), &diagnostics);

        assert_eq!(classified.class, FimRetryFailureClass::NonlinearBad);
        assert!(!classified.used_linear_fallback);
    }

    #[test]
    fn failure_classification_marks_weak_cpr_as_mixed() {
        let diagnostics = ResidualFamilyDiagnostics {
            water: ResidualFamilyPeak {
                family: ResidualRowFamily::Water,
                scaled_value: 1.0,
                row: 0,
                item_index: 0,
            },
            oil_component: ResidualFamilyPeak {
                family: ResidualRowFamily::OilComponent,
                scaled_value: 0.5,
                row: 1,
                item_index: 0,
            },
            gas_component: ResidualFamilyPeak {
                family: ResidualRowFamily::GasComponent,
                scaled_value: 0.25,
                row: 2,
                item_index: 0,
            },
            well_constraint: None,
            perforation_flow: None,
            global: ResidualFamilyPeak {
                family: ResidualRowFamily::Water,
                scaled_value: 1.0,
                row: 0,
                item_index: 0,
            },
        };
        let report = FimLinearSolveReport {
            solution: DVector::zeros(1),
            converged: true,
            iterations: 12,
            rhs_norm: 1.0,
            final_residual_norm: 1e-12,
            failure_diagnostics: None,
            used_fallback: false,
            backend_used: FimLinearSolverKind::FgmresCpr,
            cpr_diagnostics: Some(crate::fim::linear::FimCprDiagnostics {
                coarse_rows: 10,
                coarse_solver: crate::fim::linear::FimPressureCoarseSolverKind::ExactDense,
                smoother_label: "block-jacobi",
                coarse_applications: 4,
                average_reduction_ratio: 0.6,
                last_reduction_ratio: 0.8,
                build_timing: None,
            }),
            total_time_ms: 0.0,
            preconditioner_build_time_ms: 0.0,
        };

        let classified = classify_retry_failure(Some(&report), &diagnostics);

        assert_eq!(classified.class, FimRetryFailureClass::Mixed);
        assert_eq!(classified.cpr_average_reduction_ratio, Some(0.6));
        assert_eq!(classified.cpr_last_reduction_ratio, Some(0.8));
    }

    #[test]
    fn failure_classification_marks_converged_fallback_path_as_nonlinear_bad() {
        let diagnostics = ResidualFamilyDiagnostics {
            water: ResidualFamilyPeak {
                family: ResidualRowFamily::Water,
                scaled_value: 0.1,
                row: 0,
                item_index: 0,
            },
            oil_component: ResidualFamilyPeak {
                family: ResidualRowFamily::OilComponent,
                scaled_value: 1.0,
                row: 1,
                item_index: 0,
            },
            gas_component: ResidualFamilyPeak {
                family: ResidualRowFamily::GasComponent,
                scaled_value: 0.0,
                row: 2,
                item_index: 0,
            },
            well_constraint: None,
            perforation_flow: None,
            global: ResidualFamilyPeak {
                family: ResidualRowFamily::OilComponent,
                scaled_value: 1.0,
                row: 1,
                item_index: 0,
            },
        };
        let report = FimLinearSolveReport {
            solution: DVector::zeros(1),
            converged: true,
            iterations: 1,
            rhs_norm: 1.0,
            final_residual_norm: 1e-8,
            failure_diagnostics: None,
            used_fallback: true,
            backend_used: FimLinearSolverKind::SparseLuDebug,
            cpr_diagnostics: None,
            total_time_ms: 0.0,
            preconditioner_build_time_ms: 0.0,
        };

        let classified = classify_retry_failure(Some(&report), &diagnostics);

        assert_eq!(classified.class, FimRetryFailureClass::NonlinearBad);
        assert!(classified.used_linear_fallback);
    }

    #[test]
    fn restart_stagnation_fallback_streak_only_accumulates_matching_failures() {
        let streak = next_restart_stagnation_fallback_streak(
            0,
            Some(FimLinearFailureReason::RestartStagnation),
        );
        let streak = next_restart_stagnation_fallback_streak(
            streak,
            Some(FimLinearFailureReason::RestartStagnation),
        );
        let reset = next_restart_stagnation_fallback_streak(
            streak,
            Some(FimLinearFailureReason::DeadStateDetected),
        );

        assert_eq!(streak, 2);
        assert_eq!(reset, 0);
    }

    #[test]
    fn restart_stagnation_direct_bypass_requires_two_consecutive_fallbacks() {
        assert!(!should_enable_restart_stagnation_direct_bypass(1));
        assert!(should_enable_restart_stagnation_direct_bypass(2));
    }

    #[test]
    fn zero_move_fallback_direct_bypass_uses_existing_effective_move_floor() {
        assert!(should_enable_zero_move_fallback_direct_bypass(
            true, 0.0049, 0.000049,
        ));
        assert!(!should_enable_zero_move_fallback_direct_bypass(
            false, 0.0049, 0.000049,
        ));
        assert!(!should_enable_zero_move_fallback_direct_bypass(
            true, 0.0051, 0.000049,
        ));
        assert!(!should_enable_zero_move_fallback_direct_bypass(
            true, 0.0049, 0.000051,
        ));
    }

    #[test]
    fn repeated_zero_move_direct_bypass_groups_nearby_non_gas_cells_in_same_layer() {
        let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
            .expect("injector");
        sim.add_well(19, 19, 0, 50.0, 0.1, 0.0, false)
            .expect("producer");
        let peak = ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 0.99,
            row: 250,
            item_index: sim.idx(3, 4, 0),
        };
        let diagnostics = ResidualFamilyDiagnostics {
            water: peak,
            oil_component: peak,
            gas_component: peak,
            well_constraint: None,
            perforation_flow: None,
            global: peak,
        };

        assert!(should_enable_repeated_zero_move_direct_bypass(
            &sim,
            Some(FimHotspotSite::Cell(sim.idx(3, 3, 0))),
            &diagnostics,
            FimHotspotSite::Cell(sim.idx(3, 4, 0)),
        ));
    }

    #[test]
    fn repeated_zero_move_direct_bypass_does_not_group_vertical_non_gas_shift() {
        let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
            .expect("injector");
        sim.add_well(19, 19, 0, 50.0, 0.1, 0.0, false)
            .expect("producer");
        let peak = ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 0.99,
            row: 1390,
            item_index: sim.idx(3, 3, 1),
        };
        let diagnostics = ResidualFamilyDiagnostics {
            water: peak,
            oil_component: peak,
            gas_component: peak,
            well_constraint: None,
            perforation_flow: None,
            global: peak,
        };

        assert!(!should_enable_repeated_zero_move_direct_bypass(
            &sim,
            Some(FimHotspotSite::Cell(sim.idx(3, 3, 0))),
            &diagnostics,
            FimHotspotSite::Cell(sim.idx(3, 3, 1)),
        ));
    }

    #[test]
    fn near_converged_iterative_accept_requires_small_outer_and_bounded_candidate_worsening() {
        let report = FimLinearSolveReport {
            solution: DVector::zeros(1),
            converged: false,
            iterations: 80,
            rhs_norm: 1.0,
            final_residual_norm: 1.0e-6,
            failure_diagnostics: Some(crate::fim::linear::FimLinearFailureDiagnostics {
                reason: FimLinearFailureReason::RestartStagnation,
                tolerance: 1.0e-7,
                rhs_norm: 1.0,
                outer_residual_norm: 1.2e-6,
                preconditioned_residual_norm: Some(1.1e-6),
                estimated_residual_norm: Some(1.0e-8),
                candidate_residual_norm: Some(4.0e-6),
                restart_diagnostics: vec![crate::fim::linear::FimLinearRestartDiagnostics {
                    restart_index: 3,
                    start_iteration: 60,
                    end_iteration: 80,
                    inner_steps: 20,
                    outer_residual_norm: 1.2e-6,
                    preconditioned_residual_norm: 1.1e-6,
                    best_estimated_residual_norm: Some(1.0e-8),
                    best_candidate_residual_norm: Some(4.0e-6),
                    solution_improved: true,
                }],
            }),
            used_fallback: false,
            backend_used: FimLinearSolverKind::FgmresCpr,
            cpr_diagnostics: None,
            total_time_ms: 0.0,
            preconditioner_build_time_ms: 0.0,
        };

        assert!(should_accept_near_converged_iterative_step(&report));
    }

    #[test]
    fn near_converged_iterative_accept_rejects_large_outer_tail() {
        let report = FimLinearSolveReport {
            solution: DVector::zeros(1),
            converged: false,
            iterations: 80,
            rhs_norm: 1.0,
            final_residual_norm: 1.0e-6,
            failure_diagnostics: Some(crate::fim::linear::FimLinearFailureDiagnostics {
                reason: FimLinearFailureReason::MaxIterations,
                tolerance: 1.0e-7,
                rhs_norm: 1.0,
                outer_residual_norm: 3.0e-6,
                preconditioned_residual_norm: Some(3.0e-6),
                estimated_residual_norm: Some(1.0e-8),
                candidate_residual_norm: Some(4.0e-6),
                restart_diagnostics: vec![crate::fim::linear::FimLinearRestartDiagnostics {
                    restart_index: 3,
                    start_iteration: 60,
                    end_iteration: 80,
                    inner_steps: 20,
                    outer_residual_norm: 3.0e-6,
                    preconditioned_residual_norm: 3.0e-6,
                    best_estimated_residual_norm: Some(1.0e-8),
                    best_candidate_residual_norm: Some(4.0e-6),
                    solution_improved: true,
                }],
            }),
            used_fallback: false,
            backend_used: FimLinearSolverKind::FgmresCpr,
            cpr_diagnostics: None,
            total_time_ms: 0.0,
            preconditioner_build_time_ms: 0.0,
        };

        assert!(!should_accept_near_converged_iterative_step(&report));
    }

    #[test]
    fn failure_classification_keeps_nonconverged_fallback_path_linear_bad() {
        let diagnostics = ResidualFamilyDiagnostics {
            water: ResidualFamilyPeak {
                family: ResidualRowFamily::Water,
                scaled_value: 0.5,
                row: 0,
                item_index: 0,
            },
            oil_component: ResidualFamilyPeak {
                family: ResidualRowFamily::OilComponent,
                scaled_value: 1.0,
                row: 1,
                item_index: 0,
            },
            gas_component: ResidualFamilyPeak {
                family: ResidualRowFamily::GasComponent,
                scaled_value: 0.0,
                row: 2,
                item_index: 0,
            },
            well_constraint: None,
            perforation_flow: None,
            global: ResidualFamilyPeak {
                family: ResidualRowFamily::OilComponent,
                scaled_value: 1.0,
                row: 1,
                item_index: 0,
            },
        };
        let report = FimLinearSolveReport {
            solution: DVector::zeros(1),
            converged: false,
            iterations: 1,
            rhs_norm: 1.0,
            final_residual_norm: 1e-2,
            failure_diagnostics: None,
            used_fallback: true,
            backend_used: FimLinearSolverKind::DenseLuDebug,
            cpr_diagnostics: None,
            total_time_ms: 0.0,
            preconditioner_build_time_ms: 0.0,
        };

        let classified = classify_retry_failure(Some(&report), &diagnostics);

        assert_eq!(classified.class, FimRetryFailureClass::LinearBad);
        assert!(classified.used_linear_fallback);
    }

    #[test]
    fn repeated_nonlinear_hotspot_streak_groups_phase_rows_by_cell_site() {
        let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
            .expect("injector");
        sim.add_well(19, 19, 0, 50.0, 0.1, 0.0, false)
            .expect("producer");
        let current_peak = ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 0.98,
            row: 430,
            item_index: 143,
        };
        let current = ResidualFamilyDiagnostics {
            water: current_peak,
            oil_component: current_peak,
            gas_component: current_peak,
            well_constraint: None,
            perforation_flow: None,
            global: current_peak,
        };

        let streak = repeated_nonlinear_hotspot_streak(
            &sim,
            Some(FimHotspotSite::Cell(143)),
            1.0,
            &current,
            FimHotspotSite::Cell(143),
            0.99,
            0,
        );

        assert_eq!(streak, 1);
    }

    #[test]
    fn repeated_nonlinear_hotspot_streak_resets_after_strong_progress() {
        let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
            .expect("injector");
        sim.add_well(19, 19, 0, 50.0, 0.1, 0.0, false)
            .expect("producer");
        let peak = ResidualFamilyPeak {
            family: ResidualRowFamily::Water,
            scaled_value: 1.0,
            row: 429,
            item_index: 143,
        };
        let diagnostics = ResidualFamilyDiagnostics {
            water: peak,
            oil_component: peak,
            gas_component: peak,
            well_constraint: None,
            perforation_flow: None,
            global: peak,
        };

        let streak = repeated_nonlinear_hotspot_streak(
            &sim,
            Some(FimHotspotSite::Cell(143)),
            1.0,
            &diagnostics,
            FimHotspotSite::Cell(143),
            0.5,
            2,
        );

        assert_eq!(streak, 0);
    }

    #[test]
    fn repeated_nonlinear_hotspot_streak_relaxes_threshold_for_gas_hotspot_site() {
        let mut sim = ReservoirSimulator::new(10, 10, 3, 0.2);
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
            .expect("injector");
        sim.add_well(9, 9, 2, 50.0, 0.1, 0.0, false)
            .expect("producer");
        let current_peak = ResidualFamilyPeak {
            family: ResidualRowFamily::GasComponent,
            scaled_value: 0.91,
            row: 92,
            item_index: 30,
        };
        let current = ResidualFamilyDiagnostics {
            water: current_peak,
            oil_component: current_peak,
            gas_component: current_peak,
            well_constraint: None,
            perforation_flow: None,
            global: current_peak,
        };

        let streak = repeated_nonlinear_hotspot_streak(
            &sim,
            Some(FimHotspotSite::Cell(30)),
            1.0e-4,
            &current,
            FimHotspotSite::Cell(30),
            9.1e-5,
            0,
        );

        assert_eq!(streak, 1);
    }

    #[test]
    fn repeated_nonlinear_hotspot_streak_keeps_stricter_threshold_for_non_gas_sites() {
        let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
            .expect("injector");
        sim.add_well(19, 19, 0, 50.0, 0.1, 0.0, false)
            .expect("producer");
        let current_peak = ResidualFamilyPeak {
            family: ResidualRowFamily::Water,
            scaled_value: 0.91,
            row: 429,
            item_index: 143,
        };
        let current = ResidualFamilyDiagnostics {
            water: current_peak,
            oil_component: current_peak,
            gas_component: current_peak,
            well_constraint: None,
            perforation_flow: None,
            global: current_peak,
        };

        let streak = repeated_nonlinear_hotspot_streak(
            &sim,
            Some(FimHotspotSite::Cell(143)),
            1.0e-4,
            &current,
            FimHotspotSite::Cell(143),
            9.1e-5,
            0,
        );

        assert_eq!(streak, 0);
    }

    #[test]
    fn repeated_nonlinear_hotspot_streak_groups_nearby_non_gas_cells_in_same_layer() {
        let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
            .expect("injector");
        sim.add_well(19, 19, 0, 50.0, 0.1, 0.0, false)
            .expect("producer");
        let peak = ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 0.99,
            row: 250,
            item_index: sim.idx(3, 4, 0),
        };
        let diagnostics = ResidualFamilyDiagnostics {
            water: peak,
            oil_component: peak,
            gas_component: peak,
            well_constraint: None,
            perforation_flow: None,
            global: peak,
        };

        let streak = repeated_nonlinear_hotspot_streak(
            &sim,
            Some(FimHotspotSite::Cell(sim.idx(3, 3, 0))),
            1.0,
            &diagnostics,
            FimHotspotSite::Cell(sim.idx(3, 4, 0)),
            0.99,
            0,
        );

        assert_eq!(streak, 1);
    }

    #[test]
    fn repeated_nonlinear_hotspot_streak_does_not_group_vertical_non_gas_shift() {
        let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
            .expect("injector");
        sim.add_well(19, 19, 0, 50.0, 0.1, 0.0, false)
            .expect("producer");
        let peak = ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 0.99,
            row: 1390,
            item_index: sim.idx(3, 3, 1),
        };
        let diagnostics = ResidualFamilyDiagnostics {
            water: peak,
            oil_component: peak,
            gas_component: peak,
            well_constraint: None,
            perforation_flow: None,
            global: peak,
        };

        let streak = repeated_nonlinear_hotspot_streak(
            &sim,
            Some(FimHotspotSite::Cell(sim.idx(3, 3, 0))),
            1.0,
            &diagnostics,
            FimHotspotSite::Cell(sim.idx(3, 3, 1)),
            0.99,
            0,
        );

        assert_eq!(streak, 0);
    }

    #[test]
    fn gas_injector_symmetry_site_groups_axis_swapped_cells() {
        let mut sim = ReservoirSimulator::new(10, 10, 3, 0.2);
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true)
            .expect("injector");
        sim.add_well(9, 9, 2, 50.0, 0.1, 0.0, false)
            .expect("producer");
        let topology = build_well_topology(&sim);

        let east_site = gas_injector_symmetry_site(&sim, &topology, sim.idx(3, 0, 0));
        let north_site = gas_injector_symmetry_site(&sim, &topology, sim.idx(0, 3, 0));

        assert_eq!(east_site, north_site);
        assert_eq!(
            east_site,
            Some(FimHotspotSite::GasInjectorSymmetry {
                injector_well_index: 0,
                major_offset: 3,
                minor_offset: 0,
                vertical_offset: 0,
            })
        );
    }

    #[test]
    fn detect_oscillation_flags_single_phase_two_step_relative_change() {
        // Water swings back close to its value from 2 iterations ago (small d1) while
        // having moved a lot 1 iteration ago (large d2) -> classic oscillation signature.
        let current = PerFamilyNorms {
            water: 1.0,
            oil_component: 1.0,
            gas_component: 1.0,
            well_constraint: 1.0,
            perforation_flow: 1.0,
        };
        let prev1 = PerFamilyNorms {
            water: 2.0,
            oil_component: 1.0,
            gas_component: 1.0,
            well_constraint: 1.0,
            perforation_flow: 1.0,
        };
        let prev2 = PerFamilyNorms {
            water: 1.01,
            oil_component: 1.0,
            gas_component: 1.0,
            well_constraint: 1.0,
            perforation_flow: 1.0,
        };
        assert_eq!(detect_oscillation(current, prev1, prev2), 1);
    }

    #[test]
    fn detect_oscillation_flags_perforation_flow_two_step_relative_change() {
        // Matches the measured heavy-case pattern: perforation_flow alternates while the cell
        // families stay flat (`FIM-NEWTON-006`).
        let current = PerFamilyNorms {
            water: 1.0,
            oil_component: 1.0,
            gas_component: 1.0,
            perforation_flow: 2.137e-5,
            well_constraint: 1.0,
        };
        let prev1 = PerFamilyNorms {
            water: 1.0,
            oil_component: 1.0,
            gas_component: 1.0,
            perforation_flow: 3.419e-5,
            well_constraint: 1.0,
        };
        let prev2 = PerFamilyNorms {
            water: 1.0,
            oil_component: 1.0,
            gas_component: 1.0,
            perforation_flow: 2.137e-5,
            well_constraint: 1.0,
        };
        assert_eq!(detect_oscillation(current, prev1, prev2), 1);
    }

    #[test]
    fn detect_oscillation_ignores_missing_well_and_perforation_families() {
        // No wells/perforations in this system: both default to infinity (from_diagnostics'
        // `None` mapping) and must never register as oscillating.
        let missing = PerFamilyNorms {
            water: 1.0,
            oil_component: 1.0,
            gas_component: 1.0,
            well_constraint: f64::INFINITY,
            perforation_flow: f64::INFINITY,
        };
        assert_eq!(detect_oscillation(missing, missing, missing), 0);
    }

    #[test]
    fn detect_oscillation_requires_below_two_step_above_one_step_threshold() {
        // Monotonic decrease (no oscillation): d1 and d2 both large, but d1 is NOT < tol.
        let current = PerFamilyNorms {
            water: 1.0,
            oil_component: 1.0,
            gas_component: 1.0,
            well_constraint: 1.0,
            perforation_flow: 1.0,
        };
        let prev1 = PerFamilyNorms {
            water: 2.0,
            oil_component: 1.0,
            gas_component: 1.0,
            well_constraint: 1.0,
            perforation_flow: 1.0,
        };
        let prev2 = PerFamilyNorms {
            water: 4.0,
            oil_component: 1.0,
            gas_component: 1.0,
            well_constraint: 1.0,
            perforation_flow: 1.0,
        };
        assert_eq!(detect_oscillation(current, prev1, prev2), 0);

        // Steady state (both d1 and d2 tiny): not oscillating either.
        let steady = PerFamilyNorms {
            water: 1.0,
            oil_component: 1.0,
            gas_component: 1.0,
            well_constraint: 1.0,
            perforation_flow: 1.0,
        };
        assert_eq!(detect_oscillation(steady, steady, steady), 0);
    }

    #[test]
    fn next_relaxation_factor_floors_at_newton_max_relax() {
        let mut relax = 1.0;
        for _ in 0..20 {
            relax = next_relaxation_factor(relax, 1);
        }
        assert!((relax - OSCILLATION_MAX_RELAX_FLOOR).abs() < 1e-12);
    }

    #[test]
    fn next_relaxation_factor_holds_when_not_oscillating() {
        assert!((next_relaxation_factor(0.7, 0) - 0.7).abs() < 1e-12);
        // One decrement step from full relaxation.
        assert!((next_relaxation_factor(1.0, 1) - 0.9).abs() < 1e-12);
    }

    #[test]
    fn appleyard_and_oscillation_relaxation_compose_via_min() {
        // No history-stabilization cap active; Appleyard is tighter than relaxation.
        assert!((compose_damping(0.3, None, 0.8) - 0.3).abs() < 1e-12);
        // Oscillation relaxation is tighter than Appleyard.
        assert!((compose_damping(0.9, None, 0.5) - 0.5).abs() < 1e-12);
        // History-stabilization cap is the tightest of all three.
        assert!((compose_damping(0.9, Some(0.25), 0.5) - 0.25).abs() < 1e-12);
        // All three at 1.0 (nothing binding) -> 1.0.
        assert!((compose_damping(1.0, None, 1.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn nonlinear_history_stabilization_caps_damping_for_repeated_weak_progress() {
        let peak = ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 1.0,
            row: 430,
            item_index: 143,
        };
        let diagnostics = ResidualFamilyDiagnostics {
            water: peak,
            oil_component: peak,
            gas_component: peak,
            well_constraint: None,
            perforation_flow: None,
            global: peak,
        };
        let report = FimLinearSolveReport {
            solution: DVector::zeros(1),
            converged: true,
            iterations: 6,
            rhs_norm: 1.0,
            final_residual_norm: 1e-12,
            failure_diagnostics: None,
            used_fallback: false,
            backend_used: FimLinearSolverKind::FgmresCpr,
            cpr_diagnostics: None,
            total_time_ms: 0.0,
            preconditioner_build_time_ms: 0.0,
        };

        let first = nonlinear_history_stabilization_decision(
            &report,
            &diagnostics,
            5e-5,
            &FimNewtonOptions::default(),
            1,
            FimHotspotSite::Cell(143),
        )
        .expect("expected first stabilization decision");
        let repeated = nonlinear_history_stabilization_decision(
            &report,
            &diagnostics,
            5e-5,
            &FimNewtonOptions::default(),
            2,
            FimHotspotSite::Cell(143),
        )
        .expect("expected repeated stabilization decision");

        assert_eq!(first.site, FimHotspotSite::Cell(143));
        assert!((first.damping_cap - 0.5).abs() < 1e-12);
        assert!((repeated.damping_cap - 0.25).abs() < 1e-12);

        assert!(
            nonlinear_history_stabilization_decision(
                &report,
                &diagnostics,
                1e-3,
                &FimNewtonOptions::default(),
                2,
                FimHotspotSite::Cell(143),
            )
            .is_none()
        );
    }

    #[test]
    fn nonlinear_history_stabilization_allows_converged_fallback_path() {
        let peak = ResidualFamilyPeak {
            family: ResidualRowFamily::GasComponent,
            scaled_value: 1.0,
            row: 92,
            item_index: 30,
        };
        let diagnostics = ResidualFamilyDiagnostics {
            water: peak,
            oil_component: peak,
            gas_component: peak,
            well_constraint: None,
            perforation_flow: None,
            global: peak,
        };
        let report = FimLinearSolveReport {
            solution: DVector::zeros(1),
            converged: true,
            iterations: 1,
            rhs_norm: 1.0,
            final_residual_norm: 1e-12,
            failure_diagnostics: None,
            used_fallback: true,
            backend_used: FimLinearSolverKind::DenseLuDebug,
            cpr_diagnostics: None,
            total_time_ms: 0.0,
            preconditioner_build_time_ms: 0.0,
        };

        let decision = nonlinear_history_stabilization_decision(
            &report,
            &diagnostics,
            5e-5,
            &FimNewtonOptions::default(),
            1,
            FimHotspotSite::Cell(30),
        )
        .expect("expected stabilization after converged fallback");

        assert_eq!(decision.site, FimHotspotSite::Cell(30));
        assert!((decision.damping_cap - 0.5).abs() < 1e-12);
    }

    #[test]
    fn appleyard_damping_limits_combined_oil_saturation_change() {
        let state = FimState {
            cells: vec![crate::fim::state::FimCellState {
                pressure_bar: 200.0,
                sw: 0.2,
                hydrocarbon_var: 0.2,
                regime: crate::fim::state::HydrocarbonState::Saturated,
            }],
            well_bhp: Vec::new(),
            perforation_rates_m3_day: Vec::new(),
        };
        let mut update = DVector::zeros(state.n_unknowns());
        update[1] = 0.15;
        update[2] = 0.15;

        let sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        let damping =
            appleyard_damping_breakdown(&sim, &state, &update, &FimNewtonOptions::default())
                .final_damping;

        assert!((damping - (1.0 / 3.0)).abs() < 1e-12);
    }

    #[test]
    fn move_is_below_effective_trace_threshold_detects_rounds_to_zero() {
        assert!(move_is_below_effective_trace_threshold(
            0.0049, 0.000049, 0.000049, 0.0
        ));
        assert!(!move_is_below_effective_trace_threshold(
            0.0051, 0.000049, 0.000049, 0.0
        ));
        assert!(!move_is_below_effective_trace_threshold(
            0.0049, 0.000051, 0.000049, 0.0
        ));
    }

    #[test]
    fn local_cell_move_deltas_tracks_pressure_and_phase_changes() {
        let previous_state = FimState {
            cells: vec![crate::fim::state::FimCellState {
                pressure_bar: 200.0,
                sw: 0.2,
                hydrocarbon_var: 0.1,
                regime: crate::fim::state::HydrocarbonState::Saturated,
            }],
            well_bhp: Vec::new(),
            perforation_rates_m3_day: Vec::new(),
        };
        let candidate_state = FimState {
            cells: vec![crate::fim::state::FimCellState {
                pressure_bar: 200.004,
                sw: 0.20002,
                hydrocarbon_var: 0.10001,
                regime: crate::fim::state::HydrocarbonState::Saturated,
            }],
            well_bhp: Vec::new(),
            perforation_rates_m3_day: Vec::new(),
        };

        let (pressure_delta_bar, water_delta, oil_delta, gas_delta) =
            local_cell_move_deltas(&previous_state, &candidate_state, 0).expect("cell move");

        assert!((pressure_delta_bar - 0.004).abs() < 1e-12);
        assert!((water_delta - 0.00002).abs() < 1e-12);
        assert!((oil_delta - 0.00003).abs() < 1e-12);
        assert!((gas_delta - 0.00001).abs() < 1e-12);
    }

    #[test]
    fn candidate_update_bounds_include_oil_saturation_change() {
        let previous_state = FimState {
            cells: vec![crate::fim::state::FimCellState {
                pressure_bar: 200.0,
                sw: 0.2,
                hydrocarbon_var: 0.2,
                regime: crate::fim::state::HydrocarbonState::Saturated,
            }],
            well_bhp: Vec::new(),
            perforation_rates_m3_day: Vec::new(),
        };
        let candidate_state = FimState {
            cells: vec![crate::fim::state::FimCellState {
                pressure_bar: 200.0,
                sw: 0.35,
                hydrocarbon_var: 0.35,
                regime: crate::fim::state::HydrocarbonState::Saturated,
            }],
            well_bhp: Vec::new(),
            perforation_rates_m3_day: Vec::new(),
        };

        let (max_pressure_change, max_saturation_change) =
            state_update_change_bounds(&previous_state, &candidate_state);

        assert_eq!(max_pressure_change, 0.0);
        assert!((max_saturation_change - 0.3).abs() < 1e-12);
        assert!(!candidate_respects_update_bounds(
            &previous_state,
            &candidate_state,
            &FimNewtonOptions::default(),
        ));
    }
}
