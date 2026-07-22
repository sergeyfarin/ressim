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
use crate::fim::flow_resv::{FlowResvReportStepContext, flow_resv_injector_residual};
use crate::fim::linear::{
    FimLinearBlockLayout, FimLinearFailureReason, FimLinearSolveOptions, FimLinearSolveReport,
    FimLinearSolverKind, active_direct_solve_row_threshold, solve_linearized_system,
};
use crate::fim::state::{FimState, HydrocarbonState};
use crate::fim::wells::{
    build_well_topology, connection_rate_for_bhp, perforation_component_rates_sc_day,
    perforation_local_block, physical_well_control,
};
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
    for primary in state.perforation_primaries() {
        add(primary.value.to_bits());
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
        flow_resv_context: None,
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
        flow_resv_context: None,
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
    1e-4 * state.perforation_primary_value(perf_idx).abs().max(1.0)
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
        *state.perforation_primary_value_mut(perf_idx) += delta;
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
    /// G4b0 carries an immutable report-step conversion context through a Newton attempt. It is
    /// intentionally not consumed by assembly until G4b1/G4b2 implements all coupled rows.
    pub(crate) flow_resv_context: Option<FlowResvReportStepContext>,
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
            flow_resv_context: None,
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
    /// Number of reservoir linear-system solve calls made by this Newton attempt.
    pub(crate) linear_solve_count: usize,
    /// Sum of backend Krylov iterations across all reservoir linear solve calls.
    pub(crate) linear_iteration_count: usize,
    /// Number of candidate Newton corrections committed to the iterate.
    pub(crate) applied_update_count: usize,
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

mod convergence;
mod damping;
mod diagnostics;

pub(crate) use convergence::iterate_has_material_change;
use convergence::*;
use damping::*;
use diagnostics::*;

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
    let mut linear_solve_count = 0_usize;
    let mut linear_iteration_count = 0_usize;
    let mut applied_update_count = 0_usize;
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
                flow_resv_context: options.flow_resv_context,
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
        let wells_ok = if !opm_aligned || !options.nested_well_solve {
            true
        } else if let Some(context) = options.flow_resv_context {
            crate::fim::wells_inner::all_wells_converged_with_flow_resv(
                sim,
                &state,
                &topology,
                context,
                &crate::fim::wells_inner::FimWellInnerSolveOptions::default(),
            )
        } else {
            crate::fim::wells_inner::all_wells_converged(
                sim,
                &state,
                &topology,
                &crate::fim::wells_inner::FimWellInnerSolveOptions::default(),
            )
        };
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
                linear_solve_count,
                linear_iteration_count,
                applied_update_count,
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
                options.flow_resv_context,
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
                    linear_solve_count,
                    linear_iteration_count,
                    applied_update_count,
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
                linear_solve_count,
                linear_iteration_count,
                applied_update_count,
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
                    options.flow_resv_context,
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
                        linear_solve_count,
                        linear_iteration_count,
                        applied_update_count,
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
                    linear_solve_count,
                    linear_iteration_count,
                    applied_update_count,
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
        #[cfg(not(target_arch = "wasm32"))]
        let mut linear_report = if linear_options.use_flow_lifecycle {
            crate::fim::linear::flow_lifecycle::solve_live_flow_lifecycle(
                sim,
                previous_state,
                &state,
                block_layout.expect("FIM always defines a linear block layout"),
                &assembly.jacobian,
                &rhs,
            )
            .unwrap_or_else(|error| panic!("Y2d6d Flow lifecycle setup failed: {error}"))
        } else {
            solve_linearized_system(
                &assembly.jacobian,
                &rhs,
                &linear_options,
                block_layout,
                None,
            )
        };
        #[cfg(target_arch = "wasm32")]
        let mut linear_report = solve_linearized_system(
            &assembly.jacobian,
            &rhs,
            &linear_options,
            block_layout,
            None,
        );
        linear_solve_time_ms += linear_report.total_time_ms;
        linear_preconditioner_build_time_ms += linear_report.preconditioner_build_time_ms;
        linear_solve_count += 1;
        linear_iteration_count += linear_report.iterations;

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
                linear_solve_count += 1;
                linear_iteration_count += linear_report.iterations;
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
                        linear_solve_count,
                        linear_iteration_count,
                        applied_update_count,
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
        // A Legacy direct fallback is still a linear solve, not an unconditional rescue. Do not
        // pass a non-finite fallback correction into update diagnostics/state mutation; besides
        // producing invalid states, `f64::max` in the historical norm can hide NaNs. Preserve the
        // historical use of finite direct corrections whose debug-backend convergence flag is
        // false: locked rate-controlled well tests rely on those usable corrections.
        if !linear_report.solution.iter().all(|value| value.is_finite()) {
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
                linear_solve_count,
                linear_iteration_count,
                applied_update_count,
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
                options.flow_resv_context,
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
                    linear_solve_count,
                    linear_iteration_count,
                    applied_update_count,
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
                linear_solve_count,
                linear_iteration_count,
                applied_update_count,
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
        let well_update_mode = if let Some(context) = options.flow_resv_context {
            crate::fim::state::WellStateUpdateMode::FlowResv {
                context,
                nested_well_solve: options.nested_well_solve,
            }
        } else if options.nested_well_solve {
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
                    candidate.perforation_primary_value(perf_idx)
                        - (state.perforation_primary_value(perf_idx) + raw_dq[perf_idx])
                })
                .collect();
            crate::fim::trace_sink::write_line(&format!(
                "WELLTRACE iter={:>2} bhp_pre={:?} bhp_post={:?} q_pre={:?} q_post={:?} raw_dbhp={:?} raw_dq={:?} relax_dbhp_approx={:?} relax_dq_approx={:?} res_wc={:.6e} res_pf={:.6e}",
                iteration,
                state.well_bhp,
                candidate.well_bhp,
                state.perforation_primaries(),
                candidate.perforation_primaries(),
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
            // `perforation_primaries`) and `opm_per_cell_chopped_update` never chops
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
                let hc_col = unknown_offset(cell_idx, 2);
                let raw_dhc_cell = update_to_apply[hc_col];
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
                    "WELLJAC iter={:>2} perf={} cell={} res_pf={:.6e} d(res_pf)/dq={:.6e} d(res_pf)/dp={:.6e} d(res_pf)/dsw={:.6e} sw={:.6} sw_wc={:.6} hc_meaning={:?} hc_pre={:.9e} raw_dp={:.6e} raw_dsw={:.6e} raw_dhc={:.6e} sw_unclamped_would_be={:.6} hc_post_meaning={:?} hc_post={:.9e} {}",
                    iteration,
                    perf_idx,
                    cell_idx,
                    assembly.residual[perf_row],
                    d_pf_dq,
                    d_pf_dp,
                    d_pf_dsw,
                    sw_current,
                    sw_wc,
                    state.cells[cell_idx].regime,
                    state.cells[cell_idx].hydrocarbon_var,
                    raw_dp_cell,
                    raw_dsw_cell,
                    raw_dhc_cell,
                    sw_current + raw_dsw_cell,
                    candidate.cells[cell_idx].regime,
                    candidate.cells[cell_idx].hydrocarbon_var,
                    cell_terms.join(" "),
                ));
                // Y2d8/G4 source-formulation audit: emit the exact component source passed to
                // the connected reservoir cell, its dt-weighted residual contribution, and the
                // assembled q-column. This is observation-only and deliberately shares the
                // production `perforation_component_rates_sc_day` helper, so the trace cannot
                // drift into a re-derived source formula. For the tracked RESV gas injector,
                // the expected sole nonzero component is q/bg and its residual source is
                // q*dt/bg, the same surface-component conversion used by Flow StandardWell.
                let derived = state.derive_cell(sim, cell_idx);
                if options
                    .flow_resv_context
                    .map(|context| context.perforation_idx)
                    == Some(perf_idx)
                {
                    let context = options.flow_resv_context.expect("selected above");
                    let q_connection = connection_rate_for_bhp(
                        sim,
                        &state,
                        &topology,
                        perf_idx,
                        state.well_bhp[context.physical_well_idx],
                    )
                    .expect("validated RESV trace connection");
                    let u = state
                        .flow_resv_surface_u(perf_idx)
                        .expect("RESV trace requires a typed surface-u primary");
                    let terms = flow_resv_injector_residual(
                        q_connection,
                        derived.bg,
                        u,
                        context.reference.bg_rm3_per_sm3,
                        context.reservoir_target_rm3_day,
                    );
                    crate::fim::trace_sink::write_line(&format!(
                        "WELLSOURCE iter={:>2} perf={} cell={} primary_kind=flow_resv_gas_u u_sm3_day={:.9e} q_connection_rm3_day={:.9e} c_s_sm3_day={:.9e} bg_current={:.9e} bg_reference={:.9e} q_resv_target={:.9e} r_perf={:.9e} r_ctrl={:.9e} gas_source_sm3_day={:.9e} dgas_dp={:.9e} dgas_dsw={:.9e} dgas_dhc={:.9e} dgas_dbhp={:.9e} dgas_du={:.9e}",
                        iteration,
                        perf_idx,
                        cell_idx,
                        u,
                        q_connection,
                        terms.connection_rate_sc_day,
                        derived.bg,
                        context.reference.bg_rm3_per_sm3,
                        context.reservoir_target_rm3_day,
                        terms.perforation,
                        terms.control,
                        terms.gas_source_sc_day,
                        assembly
                            .jacobian
                            .get(equation_offset(cell_idx, 2), p_col)
                            .copied()
                            .unwrap_or(0.0)
                            / dt_days,
                        assembly
                            .jacobian
                            .get(equation_offset(cell_idx, 2), sw_col)
                            .copied()
                            .unwrap_or(0.0)
                            / dt_days,
                        assembly
                            .jacobian
                            .get(equation_offset(cell_idx, 2), unknown_offset(cell_idx, 2))
                            .copied()
                            .unwrap_or(0.0)
                            / dt_days,
                        assembly
                            .jacobian
                            .get(
                                equation_offset(cell_idx, 2),
                                state.well_bhp_unknown_offset(context.physical_well_idx)
                            )
                            .copied()
                            .unwrap_or(0.0)
                            / dt_days,
                        assembly
                            .jacobian
                            .get(equation_offset(cell_idx, 2), q_col)
                            .copied()
                            .unwrap_or(0.0)
                            / dt_days,
                    ));
                } else {
                    let component_rates =
                        perforation_component_rates_sc_day(sim, &state, &topology, perf_idx);
                    let q = state
                        .reservoir_connection_q(perf_idx)
                        .expect("historical source trace requires a reservoir-q primary");
                    let source_residual: [f64; 3] = component_rates.map(|rate| rate * dt_days);
                    let source_dq: [f64; 3] = [
                        assembly
                            .jacobian
                            .get(equation_offset(cell_idx, 0), q_col)
                            .copied()
                            .unwrap_or(0.0),
                        assembly
                            .jacobian
                            .get(equation_offset(cell_idx, 1), q_col)
                            .copied()
                            .unwrap_or(0.0),
                        assembly
                            .jacobian
                            .get(equation_offset(cell_idx, 2), q_col)
                            .copied()
                            .unwrap_or(0.0),
                    ];
                    crate::fim::trace_sink::write_line(&format!(
                        "WELLSOURCE iter={:>2} perf={} cell={} inj={} q={:.9e} p={:.9e} sw={:.9e} sg={:.9e} rs={:.9e} bo={:.9e} bg={:.9e} comp_rate_sc_day={:?} source_residual={:?} dres_dq={:?}",
                        iteration,
                        perf_idx,
                        cell_idx,
                        perforation.injector,
                        q,
                        state.cells[cell_idx].pressure_bar,
                        state.cells[cell_idx].sw,
                        derived.sg,
                        derived.rs,
                        derived.bo,
                        derived.bg,
                        component_rates,
                        source_residual,
                        source_dq,
                    ));
                }
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
                for (component, label) in ["water", "oil", "gas"].iter().enumerate() {
                    if let Some(partition) =
                        crate::fim::assembly_ad::cell_equation_residual_breakdown_ad(
                            sim,
                            previous_state,
                            &state,
                            &topology,
                            dt_days,
                            options.flow_resv_context,
                            cell_idx,
                            component,
                        )
                    {
                        let assembled = assembly.residual[equation_offset(cell_idx, component)];
                        crate::fim::trace_sink::write_line(&format!(
                            "RESERVOIR-PARTITION iter={:>2} cell={} component={} accumulation={:.9e} x-={:.9e} x+={:.9e} y-={:.9e} y+={:.9e} z-={:.9e} z+={:.9e} well_source={:.9e} total={:.9e} assembled={:.9e} reconstruction_delta={:.9e}",
                            iteration,
                            cell_idx,
                            label,
                            partition.accumulation,
                            partition.x_minus,
                            partition.x_plus,
                            partition.y_minus,
                            partition.y_plus,
                            partition.z_minus,
                            partition.z_plus,
                            partition.well_source,
                            partition.total,
                            assembled,
                            partition.total - assembled,
                        ));
                    }
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
                options.flow_resv_context,
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
                    linear_solve_count,
                    linear_iteration_count,
                    applied_update_count,
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
                linear_solve_count,
                linear_iteration_count,
                applied_update_count,
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
        applied_update_count += 1;
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
            flow_resv_context: options.flow_resv_context,
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
            linear_solve_count,
            linear_iteration_count,
            applied_update_count,
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
        linear_solve_count,
        linear_iteration_count,
        applied_update_count,
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
#[path = "newton/tests.rs"]
mod tests;
