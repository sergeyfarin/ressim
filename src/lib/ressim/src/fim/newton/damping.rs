use super::*;

pub(super) fn fw_at_sw(
    sim: &ReservoirSimulator,
    cell: &crate::fim::state::FimCellState,
    sw: f64,
) -> f64 {
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
            let (krw, kro) = sim.fim_two_phase_relperm(sw);
            (krw / mu_w, kro / mu_o, 0.0)
        }
    } else {
        // The fractional-flow inflection chop must see the same relperm model as the residual,
        // otherwise it damps curvature the reservoir no longer has (WATER-020).
        let (krw, kro) = sim.fim_two_phase_relperm(sw);
        (krw / mu_w, kro / mu_o, 0.0)
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
pub(super) fn fw_inflection_point_sw(
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

pub(super) fn appleyard_damping_breakdown(
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

pub(super) fn cell_phase_saturations(cell: &crate::fim::state::FimCellState) -> (f64, f64, f64) {
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

pub(super) fn local_cell_move_deltas(
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

pub(super) fn move_is_below_effective_trace_threshold(
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

pub(super) fn cell_attached_perforation_context_trace(
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

pub(super) fn effective_move_threshold_trace(
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
pub(super) struct NonlinearHistoryStabilizationDecision {
    pub(super) damping_cap: f64,
    pub(super) repeated_site_streak: u32,
    pub(super) site: FimHotspotSite,
}

pub(super) fn cell_ijk(sim: &ReservoirSimulator, cell_idx: usize) -> (usize, usize, usize) {
    let i = cell_idx % sim.nx;
    let j = (cell_idx / sim.nx) % sim.ny;
    let k = cell_idx / (sim.nx * sim.ny);
    (i, j, k)
}

pub(super) fn exact_residual_hotspot_site(peak: &ResidualFamilyPeak) -> FimHotspotSite {
    match peak.family {
        ResidualRowFamily::Water
        | ResidualRowFamily::OilComponent
        | ResidualRowFamily::GasComponent => FimHotspotSite::Cell(peak.item_index),
        ResidualRowFamily::WellConstraint => FimHotspotSite::Well(peak.item_index),
        ResidualRowFamily::PerforationFlow => FimHotspotSite::Perforation(peak.item_index),
    }
}

pub(super) fn gas_injector_symmetry_site(
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

pub(super) fn residual_hotspot_site(
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

pub(super) fn representative_well_index(sim: &ReservoirSimulator, well_idx: usize) -> usize {
    let Some(physical_well_id) = sim.wells[well_idx].physical_well_id.as_deref() else {
        return well_idx;
    };

    sim.wells
        .iter()
        .position(|well| well.physical_well_id.as_deref() == Some(physical_well_id))
        .unwrap_or(well_idx)
}

pub(super) fn nearest_well_reference_index(
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

pub(super) fn non_gas_hotspot_sites_share_local_region(
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

pub(super) fn repeated_nonlinear_hotspot_streak(
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

pub(super) fn nonlinear_history_stabilization_decision(
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

pub(super) fn nonlinear_history_stabilization_trace(
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
pub(super) const OSCILLATION_MAX_RELAX_FLOOR: f64 = 0.5;
const OSCILLATION_MIN_OSCILLATING_PHASES: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PerFamilyNorms {
    pub(super) water: f64,
    pub(super) oil_component: f64,
    pub(super) gas_component: f64,
    pub(super) well_constraint: f64,
    pub(super) perforation_flow: f64,
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
    pub(super) fn from_diagnostics(diagnostics: &ResidualFamilyDiagnostics) -> Self {
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
pub(super) struct RelaxationState {
    pub(super) residual_norm_2ago: PerFamilyNorms,
    pub(super) residual_norm_1ago: PerFamilyNorms,
    pub(super) current_relaxation: f64,
    pub(super) history_len: u32,
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
pub(super) fn family_is_oscillating(f0: f64, f1: f64, f2: f64) -> bool {
    if !(f0.is_finite() && f1.is_finite() && f2.is_finite()) || f0 <= 0.0 {
        return false;
    }
    let d1 = (f0 - f2).abs() / f0;
    let d2 = (f0 - f1).abs() / f0;
    d1 < OSCILLATION_RELAX_REL_TOL && OSCILLATION_RELAX_REL_TOL < d2
}

pub(super) fn detect_oscillation(
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
pub(super) fn next_relaxation_factor(current_relaxation: f64, oscillating_phase_count: u32) -> f64 {
    if oscillating_phase_count >= OSCILLATION_MIN_OSCILLATING_PHASES {
        (current_relaxation - OSCILLATION_RELAX_INCREMENT).max(OSCILLATION_MAX_RELAX_FLOOR)
    } else {
        current_relaxation
    }
}

/// Sub-phase 7.2: compose Appleyard damping, history-stabilization cap (if any), and the
/// OPM-style oscillation-relaxation scalar as three independent multiplicative bounds on
/// the same Newton update — whichever is tightest wins.
pub(super) fn compose_damping(
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

pub(super) fn state_update_change_bounds(
    previous_state: &FimState,
    candidate_state: &FimState,
) -> (f64, f64) {
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

pub(super) fn candidate_respects_update_bounds(
    previous_state: &FimState,
    candidate_state: &FimState,
    options: &FimNewtonOptions,
) -> bool {
    let (max_pressure_change, max_saturation_change) =
        state_update_change_bounds(previous_state, candidate_state);
    max_pressure_change <= options.max_pressure_change_bar + 1e-9
        && max_saturation_change <= options.max_saturation_change + 1e-9
}
