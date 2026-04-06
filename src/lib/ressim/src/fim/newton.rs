use nalgebra::DVector;

use crate::ReservoirSimulator;
use crate::fim::assembly::{
    CellFacePhaseDiagnostics, FacePhaseDiagnostics, FimAssemblyOptions, PhaseFluxDiagnostic,
    assemble_fim_system, cell_equation_residual_breakdown, cell_face_phase_flux_diagnostics,
};
use crate::fim::linear::{
    FimLinearBlockLayout, FimLinearSolveOptions, FimLinearSolveReport, FimLinearSolverKind,
    active_direct_solve_row_threshold, solve_linearized_system,
};
use crate::fim::state::FimState;
use crate::fim::wells::{
    build_well_topology, perforation_local_block, physical_well_control,
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
const EFFECTIVE_TRACE_PRESSURE_MOVE_THRESHOLD_BAR: f64 = 5e-3;
const EFFECTIVE_TRACE_SATURATION_MOVE_THRESHOLD: f64 = 5e-5;
const CHECKPOINT_TRACE_TARGET_CELL_INDEX: usize = 143;
const PRODUCER_HOTSPOT_MIN_BOUNDARY_PLANES: usize = 2;
const PRODUCER_HOTSPOT_STAGNATION_THRESHOLD: u32 = 1;
const NONLINEAR_HISTORY_WEAK_PROGRESS_RATIO: f64 = 0.98;
const NONLINEAR_HISTORY_MIN_STREAK: u32 = 1;
const NONLINEAR_HISTORY_FIRST_DAMPING_CAP: f64 = 0.5;
const NONLINEAR_HISTORY_REPEAT_DAMPING_CAP: f64 = 0.25;
const NONLINEAR_HISTORY_RESIDUAL_BAND_FACTOR: f64 = 10.0;

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
    pub(crate) linear_iterations: Option<usize>,
    pub(crate) used_linear_fallback: bool,
    pub(crate) cpr_average_reduction_ratio: Option<f64>,
    pub(crate) cpr_last_reduction_ratio: Option<f64>,
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
        crate::fim::linear::FimPressureCoarseSolverKind::IluDefectCorrection => "ilu",
    };

    format!(
        " lin=[req={} used={} rows={} direct_thr={}] cpr=[rows={} solver={} apps={} avg_rr={:.3e} last_rr={:.3e}]",
        requested_kind.label(),
        report.backend_used.label(),
        report.solution.len(),
        active_direct_solve_row_threshold(),
        cpr.coarse_rows,
        solver,
        cpr.coarse_applications,
        cpr.average_reduction_ratio,
        cpr.last_reduction_ratio,
    )
}

fn classify_retry_failure(
    linear_report: Option<&FimLinearSolveReport>,
    residual_diagnostics: &ResidualFamilyDiagnostics,
) -> FimRetryFailureDiagnostics {
    let used_linear_fallback = linear_report.is_some_and(|report| report.used_fallback);
    let cpr_average_reduction_ratio = linear_report
        .and_then(|report| report.cpr_diagnostics.as_ref())
        .map(|diagnostics| diagnostics.average_reduction_ratio);
    let cpr_last_reduction_ratio = linear_report
        .and_then(|report| report.cpr_diagnostics.as_ref())
        .map(|diagnostics| diagnostics.last_reduction_ratio);
    let class = if let Some(report) = linear_report {
        if !report.converged || used_linear_fallback {
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
        linear_iterations: linear_report.map(|report| report.iterations),
        used_linear_fallback,
        cpr_average_reduction_ratio,
        cpr_last_reduction_ratio,
    }
}

fn retry_failure_trace_suffix(diagnostics: &FimRetryFailureDiagnostics) -> String {
    let mut parts = vec![format!(
        " retry=[class={} dom={} row={} item={}",
        diagnostics.class.label(),
        diagnostics.dominant_family_label,
        diagnostics.dominant_row,
        diagnostics.dominant_item_index,
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

/// Appleyard chop: compute the largest damping factor such that no cell variable
/// exceeds its per-iteration limit. Returns a value in (0, 1].
///
/// Also enforces the trust-region boundary at the fw inflection point (Wang & Tchelepi,
/// 2013): if a Newton update would move Sw across the inflection point, the damping is
/// chopped to keep the iterate on the same side. This prevents Newton from jumping into
/// the wrong convergence basin of the fractional-flow curve, which is the primary source
/// of nonlinear stagnation near water-breakthrough fronts.
fn appleyard_damping(
    sim: &ReservoirSimulator,
    state: &FimState,
    update: &DVector<f64>,
    options: &FimNewtonOptions,
) -> f64 {
    let mut max_damping = 1.0_f64;
    let n_cells = state.cells.len();

    for idx in 0..n_cells {
        let offset = idx * 3;
        let cell = state.cell(idx);

        // Pressure
        let dp = update[offset].abs();
        if dp > 1e-12 {
            max_damping = max_damping.min(options.max_pressure_change_bar / dp);
        }

        // Water saturation — standard Appleyard limit.
        let dsw = update[offset + 1].abs();
        if dsw > 1e-12 {
            max_damping = max_damping.min(options.max_saturation_change / dsw);
        }

        // Trust-region boundary at the fw inflection point (water).
        // If the full update would cross the inflection point, chop so Sw lands at it.
        // This guides Newton to stay within one convergence basin of the fractional-flow
        // curve and prevents the stagnation plateau observed at breakthrough.
        let dsw_signed = update[offset + 1];
        if dsw_signed.abs() > 1e-12 {
            if let Some(sw_inflect) = fw_inflection_point_sw(sim, cell) {
                let sw_full = cell.sw + max_damping * dsw_signed;
                // Check if the damped step would cross the inflection point.
                let side_before = cell.sw - sw_inflect;
                let side_after = sw_full - sw_inflect;
                if side_before * side_after < 0.0 {
                    // Cross detected: chop so update stops exactly at inflection point.
                    let dist = (sw_inflect - cell.sw).abs();
                    let chop = (dist / dsw_signed.abs()).clamp(0.0, max_damping);
                    max_damping = max_damping.min(chop);
                }
            }
        }

        // Hydrocarbon variable (Sg or Rs)
        let dh = update[offset + 2];
        if dh.abs() > 1e-12 {
            match cell.regime {
                crate::fim::state::HydrocarbonState::Saturated => {
                    // Sg: limit absolute change.
                    max_damping = max_damping.min(options.max_saturation_change / dh.abs());

                    // So = 1 - Sw - Sg, so bound the implied oil-saturation move too.
                    let dso = (update[offset + 1] + dh).abs();
                    if dso > 1e-12 {
                        max_damping = max_damping.min(options.max_saturation_change / dso);
                    }
                }
                crate::fim::state::HydrocarbonState::Undersaturated => {
                    // Rs: limit relative change.
                    let rs_scale = cell.hydrocarbon_var.abs().max(1.0);
                    max_damping =
                        max_damping.min(options.max_rs_change_fraction * rs_scale / dh.abs());
                }
            }
        }
    }

    // Well BHP: same pressure limit
    let well_offset = state.n_cell_unknowns();
    for well_idx in 0..state.n_well_unknowns() {
        let dbhp = update[well_offset + well_idx].abs();
        if dbhp > 1e-12 {
            max_damping = max_damping.min(options.max_pressure_change_bar / dbhp);
        }
    }

    max_damping.clamp(0.0, 1.0)
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
            let detail = perforation_local_block(topology, state, perf_idx).residual_diagnostics(sim)?;
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

    if diagnostics.global.item_index != CHECKPOINT_TRACE_TARGET_CELL_INDEX {
        return None;
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
        cell_attached_perforation_context_trace(sim, candidate, topology, diagnostics.global.item_index),
    ))
}

#[derive(Clone, Debug, PartialEq)]
struct ProducerHotspotStagnationDiagnostics {
    cell_idx: usize,
    row: usize,
    damping: f64,
    pressure_delta_bar: f64,
    water_delta: f64,
    oil_delta: f64,
    gas_delta: f64,
    attached_perforation_context: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResidualHotspotSite {
    Cell(usize),
    Well(usize),
    Perforation(usize),
}

#[derive(Clone, Debug, PartialEq)]
struct NonlinearHistoryStabilizationDecision {
    damping_cap: f64,
    repeated_site_streak: u32,
    site: ResidualHotspotSite,
}

fn cell_boundary_plane_count(sim: &ReservoirSimulator, cell_idx: usize) -> usize {
    if sim.nx == 0 || sim.ny == 0 || sim.nz == 0 {
        return 0;
    }

    let i = cell_idx % sim.nx;
    let j = (cell_idx / sim.nx) % sim.ny;
    let k = cell_idx / (sim.nx * sim.ny);
    let mut count = 0;

    if i == 0 || i + 1 == sim.nx {
        count += 1;
    }
    if j == 0 || j + 1 == sim.ny {
        count += 1;
    }
    if k == 0 || k + 1 == sim.nz {
        count += 1;
    }

    count
}

fn cell_has_only_attached_producer_perforations(
    topology: &crate::fim::wells::FimWellTopology,
    cell_idx: usize,
) -> bool {
    let mut has_attached_perforation = false;

    for perforation in &topology.perforations {
        if perforation.cell_index != cell_idx {
            continue;
        }
        has_attached_perforation = true;
        if perforation.injector {
            return false;
        }
    }

    has_attached_perforation
}

fn producer_hotspot_stagnation_diagnostics(
    sim: &ReservoirSimulator,
    state: &FimState,
    candidate: &FimState,
    topology: &crate::fim::wells::FimWellTopology,
    diagnostics: &ResidualFamilyDiagnostics,
    damping: f64,
) -> Option<ProducerHotspotStagnationDiagnostics> {
    let cell_idx = match diagnostics.global.family {
        ResidualRowFamily::Water
        | ResidualRowFamily::OilComponent
        | ResidualRowFamily::GasComponent => diagnostics.global.item_index,
        _ => return None,
    };

    if cell_boundary_plane_count(sim, cell_idx) < PRODUCER_HOTSPOT_MIN_BOUNDARY_PLANES {
        return None;
    }
    if !cell_has_only_attached_producer_perforations(topology, cell_idx) {
        return None;
    }

    let (pressure_delta_bar, water_delta, oil_delta, gas_delta) =
        local_cell_move_deltas(state, candidate, cell_idx)?;
    if !move_is_below_effective_trace_threshold(
        pressure_delta_bar,
        water_delta,
        oil_delta,
        gas_delta,
    ) {
        return None;
    }

    Some(ProducerHotspotStagnationDiagnostics {
        cell_idx,
        row: diagnostics.global.row,
        damping,
        pressure_delta_bar,
        water_delta,
        oil_delta,
        gas_delta,
        attached_perforation_context: cell_attached_perforation_context_trace(
            sim,
            candidate,
            topology,
            cell_idx,
        ),
    })
}

fn producer_hotspot_stagnation_trace(
    diagnostics: &ProducerHotspotStagnationDiagnostics,
) -> String {
    format!(
        "cell{} row={} damp={:.4} local_dP={:.5} local_dSw={:.6} local_dSo={:.6} local_dSg={:.6} {}",
        diagnostics.cell_idx,
        diagnostics.row,
        diagnostics.damping,
        diagnostics.pressure_delta_bar,
        diagnostics.water_delta,
        diagnostics.oil_delta,
        diagnostics.gas_delta,
        diagnostics.attached_perforation_context,
    )
}

fn residual_hotspot_site(peak: &ResidualFamilyPeak) -> ResidualHotspotSite {
    match peak.family {
        ResidualRowFamily::Water
        | ResidualRowFamily::OilComponent
        | ResidualRowFamily::GasComponent => ResidualHotspotSite::Cell(peak.item_index),
        ResidualRowFamily::WellConstraint => ResidualHotspotSite::Well(peak.item_index),
        ResidualRowFamily::PerforationFlow => ResidualHotspotSite::Perforation(peak.item_index),
    }
}

fn repeated_nonlinear_hotspot_streak(
    previous_diagnostics: Option<&ResidualFamilyDiagnostics>,
    previous_residual_norm: f64,
    current_diagnostics: &ResidualFamilyDiagnostics,
    current_residual_norm: f64,
    current_streak: u32,
) -> u32 {
    let Some(previous_diagnostics) = previous_diagnostics else {
        return 0;
    };
    if !previous_residual_norm.is_finite() || previous_residual_norm <= f64::EPSILON {
        return 0;
    }

    let same_site = residual_hotspot_site(&previous_diagnostics.global)
        == residual_hotspot_site(&current_diagnostics.global);
    let weak_progress = current_residual_norm
        >= previous_residual_norm * NONLINEAR_HISTORY_WEAK_PROGRESS_RATIO;

    if same_site && weak_progress {
        current_streak + 1
    } else {
        0
    }
}

fn nonlinear_history_stabilization_decision(
    linear_report: &FimLinearSolveReport,
    current_diagnostics: &ResidualFamilyDiagnostics,
    current_residual_norm: f64,
    options: &FimNewtonOptions,
    repeated_site_streak: u32,
) -> Option<NonlinearHistoryStabilizationDecision> {
    if repeated_site_streak < NONLINEAR_HISTORY_MIN_STREAK
        || !linear_report.converged
        || linear_report.used_fallback
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
        site: residual_hotspot_site(&current_diagnostics.global),
    })
}

fn nonlinear_history_stabilization_trace(
    decision: &NonlinearHistoryStabilizationDecision,
) -> String {
    let site = match decision.site {
        ResidualHotspotSite::Cell(cell_idx) => format!("cell{}", cell_idx),
        ResidualHotspotSite::Well(well_idx) => format!("well{}", well_idx),
        ResidualHotspotSite::Perforation(perf_idx) => format!("perf{}", perf_idx),
    };
    format!(
        " hist=[site={} streak={} damp_cap={:.3}]",
        site, decision.repeated_site_streak, decision.damping_cap,
    )
}

fn producer_hotspot_cell_index(diagnostics: &ResidualFamilyDiagnostics) -> Option<usize> {
    match diagnostics.global.family {
        ResidualRowFamily::Water
        | ResidualRowFamily::OilComponent
        | ResidualRowFamily::GasComponent => Some(diagnostics.global.item_index),
        _ => None,
    }
}

fn producer_hotspot_stagnation_should_bail(
    previous_effective_move: Option<&ProducerHotspotStagnationDiagnostics>,
    residual_diagnostics: &ResidualFamilyDiagnostics,
    candidate_is_valid: bool,
    stagnation_count: u32,
) -> bool {
    if !candidate_is_valid || stagnation_count < PRODUCER_HOTSPOT_STAGNATION_THRESHOLD {
        return false;
    }

    let Some(previous_effective_move) = previous_effective_move else {
        return false;
    };

    producer_hotspot_cell_index(residual_diagnostics)
        .is_some_and(|cell_idx| cell_idx == previous_effective_move.cell_idx)
}

fn classify_producer_hotspot_stagnation_failure(
    linear_report: Option<&FimLinearSolveReport>,
    residual_diagnostics: &ResidualFamilyDiagnostics,
) -> FimRetryFailureDiagnostics {
    let mut diagnostics = classify_retry_failure(linear_report, residual_diagnostics);
    diagnostics.class = FimRetryFailureClass::NonlinearBad;
    diagnostics
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

fn iterate_has_material_change(previous_state: &FimState, state: &FimState) -> bool {
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
            assemble_residual_only: false,
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
        options.material_balance_tolerance * factor,
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

fn stagnation_acceptance_allows(
    materially_changed: bool,
    residual_inf_norm: f64,
    material_balance_inf_norm: f64,
    update_inf_norm: f64,
    options: &FimNewtonOptions,
) -> bool {
    materially_changed
        && update_inf_norm <= options.update_tolerance
        && residual_inf_norm <= options.residual_tolerance * NONLINEAR_HISTORY_RESIDUAL_BAND_FACTOR
        && material_balance_inf_norm <= options.material_balance_tolerance
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
    let mut previous_residual_diagnostics: Option<ResidualFamilyDiagnostics> = None;
    let mut repeated_hotspot_streak: u32 = 0;
    let mut previous_producer_hotspot_effective_move: Option<ProducerHotspotStagnationDiagnostics> =
        None;
    let mut assembly_ms = 0.0;
    let mut property_eval_ms = 0.0;
    let mut linear_solve_time_ms = 0.0;
    let mut linear_preconditioner_build_time_ms = 0.0;
    let mut state_update_ms = 0.0;
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
        assembly_ms +=
            assembly.timing.residual_ms + assembly.timing.sensitivity_eval_ms + assembly.timing.jacobian_ms;
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

        let current_norm = final_residual_inf_norm.unwrap_or(f64::INFINITY);
        let previous_iteration_residual_norm = prev_residual_norm;
        repeated_hotspot_streak = repeated_nonlinear_hotspot_streak(
            previous_residual_diagnostics.as_ref(),
            previous_iteration_residual_norm,
            &residual_diagnostics,
            current_norm,
            repeated_hotspot_streak,
        );
        let materially_changed = iterate_has_material_change(previous_state, &state);
        let converged_on_entry = if iteration == 0 && !materially_changed {
            current_norm <= options.residual_tolerance * NOOP_ENTRY_EXACT_FACTOR
        } else {
            current_norm <= options.residual_tolerance
                || (iteration == 0
                    && materially_changed
                    && current_norm <= options.residual_tolerance * ENTRY_RESIDUAL_GUARD_FACTOR)
        };
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
                return FimStepReport {
                    accepted_state: accepted_diagnostics.state,
                    converged: true,
                    newton_iterations: iteration + 1,
                    final_residual_inf_norm: accepted_diagnostics.residual_inf_norm,
                    final_material_balance_inf_norm: accepted_diagnostics.material_balance_inf_norm,
                    final_update_inf_norm,
                    last_linear_report,
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

            let failure_diagnostics = classify_retry_failure(
                last_linear_report.as_ref(),
                &accepted_diagnostics.residual_diagnostics,
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
            return FimStepReport {
                accepted_state: accepted_diagnostics.state,
                converged: false,
                newton_iterations: iteration + 1,
                final_residual_inf_norm: accepted_diagnostics.residual_inf_norm,
                final_material_balance_inf_norm: accepted_diagnostics.material_balance_inf_norm,
                final_update_inf_norm,
                last_linear_report,
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
        if iteration >= 2 && current_norm >= prev_residual_norm * 0.95 {
            stagnation_count += 1;
            if stagnation_count >= 3 {
                let materially_changed = iterate_has_material_change(previous_state, &state);
                let accepted_diagnostics = evaluate_accepted_state_convergence(
                    sim,
                    previous_state,
                    &state,
                    &topology,
                    dt_days,
                );
                if stagnation_acceptance_allows(
                    materially_changed,
                    accepted_diagnostics.residual_inf_norm,
                    accepted_diagnostics.material_balance_inf_norm,
                    final_update_inf_norm,
                    options,
                ) {
                    fim_trace!(
                        sim,
                        options.verbose,
                        "    iter {:>2}: STAGNATION-ACCEPTED res={:.3e} mb={:.3e} upd={:.3e} fam=[{}] mb=[{}]{}",
                        iteration,
                        accepted_diagnostics.residual_inf_norm,
                        accepted_diagnostics.material_balance_inf_norm,
                        final_update_inf_norm,
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
                        last_linear_report,
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

                let failure_diagnostics =
                    classify_retry_failure(last_linear_report.as_ref(), &residual_diagnostics);
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
            stagnation_count = 0;
        }
        prev_residual_norm = current_norm;

        let rhs = -&assembly.residual;
        let mut linear_report =
            solve_linearized_system(&assembly.jacobian, &rhs, &options.linear, block_layout);
        linear_solve_time_ms += linear_report.total_time_ms;
        linear_preconditioner_build_time_ms += linear_report.preconditioner_build_time_ms;

        let mut used_fallback = false;
        if !linear_report.converged || !linear_report.solution.iter().all(|value| value.is_finite())
        {
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: linear solver FAILED (converged={}, finite={}), trying fallback",
                iteration,
                linear_report.converged,
                linear_report.solution.iter().all(|v| v.is_finite())
            );
            let mut fallback_options = options.linear;
            fallback_options.kind = {
                #[cfg(target_arch = "wasm32")]
                {
                    FimLinearSolverKind::DenseLuDebug
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    FimLinearSolverKind::SparseLuDebug
                }
            };
            linear_report =
                solve_linearized_system(&assembly.jacobian, &rhs, &fallback_options, block_layout);
            used_fallback = true;
            linear_report.used_fallback = true;
            linear_solve_time_ms += linear_report.total_time_ms;
            linear_preconditioner_build_time_ms += linear_report.preconditioner_build_time_ms;
        }

        final_update_inf_norm =
            scaled_update_inf_norm(&linear_report.solution, &assembly.variable_scaling);
        last_linear_report = Some(linear_report.clone());

        let converged = final_update_inf_norm <= options.update_tolerance
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
                    linear_report_trace_suffix(&linear_report, options.linear.kind),
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

            let failure_diagnostics = classify_retry_failure(
                Some(&linear_report),
                &accepted_diagnostics.residual_diagnostics,
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
                linear_report_trace_suffix(&linear_report, options.linear.kind),
                residual_family_trace(&accepted_diagnostics.residual_diagnostics),
                global_material_balance_trace(&accepted_diagnostics.material_balance_diagnostics),
                accepted_diagnostics
                    .residual_detail
                    .as_ref()
                    .map(|detail| format!(" detail=[{}]", detail))
                    .unwrap_or_default(),
                retry_failure_trace_suffix(&failure_diagnostics)
            );
            return FimStepReport {
                accepted_state: accepted_diagnostics.state,
                converged: false,
                newton_iterations: iteration + 1,
                final_residual_inf_norm: accepted_diagnostics.residual_inf_norm,
                final_material_balance_inf_norm: accepted_diagnostics.material_balance_inf_norm,
                final_update_inf_norm,
                last_linear_report: Some(linear_report),
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

        let history_stabilization = nonlinear_history_stabilization_decision(
            &linear_report,
            &residual_diagnostics,
            current_norm,
            options,
            repeated_hotspot_streak,
        );
        let damping = history_stabilization
            .as_ref()
            .map_or_else(
                || appleyard_damping(sim, &state, &linear_report.solution, options),
                |decision| {
                    appleyard_damping(sim, &state, &linear_report.solution, options)
                        .min(decision.damping_cap)
                },
            );
        let state_update_timer = PerfTimer::start();
        let candidate =
            state.apply_newton_update_frozen(sim, &linear_report.solution, damping, &topology);
        state_update_ms += state_update_timer.elapsed_ms();
        let (candidate_pressure_change, candidate_saturation_change) =
            state_update_change_bounds(&state, &candidate);
        let effective_move_trace = effective_move_threshold_trace(
            sim,
            &state,
            &candidate,
            &topology,
            &residual_diagnostics,
            damping,
        );
        let producer_hotspot_stagnation = producer_hotspot_stagnation_diagnostics(
            sim,
            &state,
            &candidate,
            &topology,
            &residual_diagnostics,
            damping,
        );
        let candidate_is_valid = damping.is_finite()
            && damping > 0.0
            && iterate_has_material_change(&state, &candidate)
            && candidate.is_finite()
            && candidate.respects_basic_bounds(sim)
            && candidate_respects_update_bounds(&state, &candidate, options);
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
            linear_report_trace_suffix(&linear_report, options.linear.kind),
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
        if let Some(effective_move_trace) = effective_move_trace {
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: HOTSPOT effective-move floor {}",
                iteration,
                effective_move_trace,
            );
        }

        if producer_hotspot_stagnation_should_bail(
            previous_producer_hotspot_effective_move.as_ref(),
            &residual_diagnostics,
            candidate_is_valid,
            stagnation_count,
        ) {
            let producer_hotspot_stagnation = previous_producer_hotspot_effective_move
                .as_ref()
                .expect("checked producer hotspot stagnation");
            let failure_diagnostics = classify_producer_hotspot_stagnation_failure(
                Some(&linear_report),
                &residual_diagnostics,
            );
            let retry_factor = retry_factor_for_failure(Some(&failure_diagnostics));
            fim_trace!(
                sim,
                options.verbose,
                "    iter {:>2}: PRODUCER-HOTSPOT STAGNATION {}{} — bailing out",
                iteration,
                producer_hotspot_stagnation_trace(&producer_hotspot_stagnation),
                retry_failure_trace_suffix(&failure_diagnostics)
            );
            return FimStepReport {
                accepted_state: state,
                converged: false,
                newton_iterations: iteration + 1,
                final_residual_inf_norm: current_norm,
                final_material_balance_inf_norm,
                final_update_inf_norm,
                last_linear_report: Some(linear_report),
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

        previous_producer_hotspot_effective_move = producer_hotspot_stagnation;
        previous_residual_diagnostics = Some(residual_diagnostics.clone());

        if !candidate_is_valid {
            let failure_diagnostics =
                classify_retry_failure(Some(&linear_report), &residual_diagnostics);
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

        state = candidate;
    }

    let final_assembly = assemble_fim_system(
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
    if final_residual_inf_norm.unwrap_or(f64::INFINITY) <= options.residual_tolerance
        && final_material_balance_inf_norm <= options.material_balance_tolerance
    {
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

    let failure_diagnostics =
        classify_retry_failure(last_linear_report.as_ref(), &final_residual_diagnostics);
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
            final_residual_norm: 1e-12,
            used_fallback: false,
            backend_used: FimLinearSolverKind::FgmresCpr,
            cpr_diagnostics: Some(crate::fim::linear::FimCprDiagnostics {
                coarse_rows: 10,
                coarse_solver: crate::fim::linear::FimPressureCoarseSolverKind::ExactDense,
                coarse_applications: 4,
                average_reduction_ratio: 1e-12,
                last_reduction_ratio: 1e-12,
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
            final_residual_norm: 1e-12,
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
            final_residual_norm: 1e-12,
            used_fallback: false,
            backend_used: FimLinearSolverKind::FgmresCpr,
            cpr_diagnostics: Some(crate::fim::linear::FimCprDiagnostics {
                coarse_rows: 10,
                coarse_solver: crate::fim::linear::FimPressureCoarseSolverKind::ExactDense,
                coarse_applications: 4,
                average_reduction_ratio: 0.6,
                last_reduction_ratio: 0.8,
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
    fn failure_classification_marks_fallback_path_as_linear_bad() {
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
            final_residual_norm: 1e-8,
            used_fallback: true,
            backend_used: FimLinearSolverKind::SparseLuDebug,
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
        let previous_peak = ResidualFamilyPeak {
            family: ResidualRowFamily::Water,
            scaled_value: 1.0,
            row: 429,
            item_index: 143,
        };
        let current_peak = ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 0.98,
            row: 430,
            item_index: 143,
        };
        let previous = ResidualFamilyDiagnostics {
            water: previous_peak,
            oil_component: previous_peak,
            gas_component: previous_peak,
            well_constraint: None,
            perforation_flow: None,
            global: previous_peak,
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
            Some(&previous),
            1.0,
            &current,
            0.95,
            0,
        );

        assert_eq!(streak, 1);
    }

    #[test]
    fn repeated_nonlinear_hotspot_streak_resets_after_strong_progress() {
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
            Some(&diagnostics),
            1.0,
            &diagnostics,
            0.5,
            2,
        );

        assert_eq!(streak, 0);
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
            final_residual_norm: 1e-12,
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
        )
            .expect("expected first stabilization decision");
        let repeated = nonlinear_history_stabilization_decision(
            &report,
            &diagnostics,
            5e-5,
            &FimNewtonOptions::default(),
            2,
        )
            .expect("expected repeated stabilization decision");

        assert_eq!(first.site, ResidualHotspotSite::Cell(143));
        assert!((first.damping_cap - 0.5).abs() < 1e-12);
        assert!((repeated.damping_cap - 0.25).abs() < 1e-12);

        assert!(nonlinear_history_stabilization_decision(
            &report,
            &diagnostics,
            1e-3,
            &FimNewtonOptions::default(),
            2,
        )
        .is_none());
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
        let damping = appleyard_damping(&sim, &state, &update, &FimNewtonOptions::default());

        assert!((damping - (1.0 / 3.0)).abs() < 1e-12);
    }

    #[test]
    fn move_is_below_effective_trace_threshold_detects_rounds_to_zero() {
        assert!(move_is_below_effective_trace_threshold(0.0049, 0.000049, 0.000049, 0.0));
        assert!(!move_is_below_effective_trace_threshold(0.0051, 0.000049, 0.000049, 0.0));
        assert!(!move_is_below_effective_trace_threshold(0.0049, 0.000051, 0.000049, 0.0));
    }

    #[test]
    fn cell_boundary_plane_count_detects_corner_cells() {
        let sim = ReservoirSimulator::new(12, 12, 3, 0.2);

        assert_eq!(cell_boundary_plane_count(&sim, 143), 3);
        assert_eq!(cell_boundary_plane_count(&sim, sim.idx(5, 5, 1)), 0);
    }

    #[test]
    fn producer_hotspot_stagnation_requires_producer_boundary_cell() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 1, 0, 50.0, 0.1, 0.0, false).unwrap();

        let topology = build_well_topology(&sim);
        let state = FimState::from_simulator(&sim);
        let producer_cell_idx = sim.idx(1, 1, 0);
        let injector_cell_idx = sim.idx(0, 0, 0);

        let producer_peak = ResidualFamilyPeak {
            family: ResidualRowFamily::Water,
            scaled_value: 1.0,
            row: producer_cell_idx * 3,
            item_index: producer_cell_idx,
        };
        let diagnostics = ResidualFamilyDiagnostics {
            water: producer_peak,
            oil_component: ResidualFamilyPeak {
                family: ResidualRowFamily::OilComponent,
                scaled_value: 0.5,
                row: producer_cell_idx * 3 + 1,
                item_index: producer_cell_idx,
            },
            gas_component: ResidualFamilyPeak {
                family: ResidualRowFamily::GasComponent,
                scaled_value: 0.25,
                row: producer_cell_idx * 3 + 2,
                item_index: producer_cell_idx,
            },
            well_constraint: None,
            perforation_flow: None,
            global: producer_peak,
        };

        assert!(producer_hotspot_stagnation_diagnostics(
            &sim,
            &state,
            &state,
            &topology,
            &diagnostics,
            0.0,
        )
        .is_some());

        let injector_peak = ResidualFamilyPeak {
            family: ResidualRowFamily::Water,
            scaled_value: 1.0,
            row: injector_cell_idx * 3,
            item_index: injector_cell_idx,
        };
        let injector_diagnostics = ResidualFamilyDiagnostics {
            water: injector_peak,
            oil_component: ResidualFamilyPeak {
                family: ResidualRowFamily::OilComponent,
                scaled_value: 0.5,
                row: injector_cell_idx * 3 + 1,
                item_index: injector_cell_idx,
            },
            gas_component: ResidualFamilyPeak {
                family: ResidualRowFamily::GasComponent,
                scaled_value: 0.25,
                row: injector_cell_idx * 3 + 2,
                item_index: injector_cell_idx,
            },
            well_constraint: None,
            perforation_flow: None,
            global: injector_peak,
        };

        assert!(producer_hotspot_stagnation_diagnostics(
            &sim,
            &state,
            &state,
            &topology,
            &injector_diagnostics,
            0.0,
        )
        .is_none());
    }

    #[test]
    fn producer_hotspot_stagnation_bails_on_following_same_cell_stagnation() {
        let previous = ProducerHotspotStagnationDiagnostics {
            cell_idx: 143,
            row: 430,
            damping: 0.0,
            pressure_delta_bar: 0.0,
            water_delta: 0.0,
            oil_delta: 0.0,
            gas_delta: 0.0,
            attached_perforation_context: String::new(),
        };
        let peak = ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 1.0,
            row: 430,
            item_index: 143,
        };
        let diagnostics = ResidualFamilyDiagnostics {
            water: ResidualFamilyPeak {
                family: ResidualRowFamily::Water,
                scaled_value: 0.5,
                row: 429,
                item_index: 143,
            },
            oil_component: peak,
            gas_component: ResidualFamilyPeak {
                family: ResidualRowFamily::GasComponent,
                scaled_value: 0.0,
                row: 0,
                item_index: 0,
            },
            well_constraint: None,
            perforation_flow: None,
            global: peak,
        };

        assert!(producer_hotspot_stagnation_should_bail(
            Some(&previous),
            &diagnostics,
            true,
            1,
        ));
    }

    #[test]
    fn producer_hotspot_stagnation_does_not_bail_for_different_cell() {
        let previous = ProducerHotspotStagnationDiagnostics {
            cell_idx: 143,
            row: 430,
            damping: 0.0,
            pressure_delta_bar: 0.0,
            water_delta: 0.0,
            oil_delta: 0.0,
            gas_delta: 0.0,
            attached_perforation_context: String::new(),
        };
        let peak = ResidualFamilyPeak {
            family: ResidualRowFamily::OilComponent,
            scaled_value: 1.0,
            row: 1294,
            item_index: 431,
        };
        let diagnostics = ResidualFamilyDiagnostics {
            water: ResidualFamilyPeak {
                family: ResidualRowFamily::Water,
                scaled_value: 0.5,
                row: 1293,
                item_index: 431,
            },
            oil_component: peak,
            gas_component: ResidualFamilyPeak {
                family: ResidualRowFamily::GasComponent,
                scaled_value: 0.0,
                row: 0,
                item_index: 0,
            },
            well_constraint: None,
            perforation_flow: None,
            global: peak,
        };

        assert!(!producer_hotspot_stagnation_should_bail(
            Some(&previous),
            &diagnostics,
            true,
            1,
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
