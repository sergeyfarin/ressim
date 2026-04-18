use crate::ReservoirSimulator;
use crate::fim::assembly::{FimAssemblyOptions, assemble_fim_system, equation_offset};
use crate::fim::linear::{FimLinearSolveReport, active_direct_solve_row_threshold};
use crate::fim::newton::FimRetryFailureClass;
use crate::fim::newton::{
    FimHotspotSite, FimNewtonOptions, FimRetryFailureDiagnostics, iterate_has_material_change,
    run_fim_timestep,
};
use crate::fim::state::{FimCellState, FimState, HydrocarbonState};
use crate::reporting::{FimAcceptedRungStats, FimRetryRungStats, FimStepStats};

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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct FimGrowthCooldown {
    cap_dt_days: Option<f64>,
    clean_successes_remaining: u32,
    hotspot: Option<FimRetryHotspot>,
    hotspot_repeat_failures: u32,
    hotspot_clean_successes_without_retry: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FimRetryMemoryRegion {
    Exact(FimHotspotSite),
    NonGasReservoirRegion {
        reference_well_index: Option<usize>,
        anchor_i: usize,
        anchor_j: usize,
        anchor_k: usize,
    },
    GasReservoirRegion {
        injector_well_index: usize,
        vertical_offset: usize,
    },
    GasFamily,
}

fn cell_ijk(sim: &ReservoirSimulator, cell_idx: usize) -> (usize, usize, usize) {
    let i = cell_idx % sim.nx;
    let j = (cell_idx / sim.nx) % sim.ny;
    let k = cell_idx / (sim.nx * sim.ny);
    (i, j, k)
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FimRetryHotspot {
    dominant_family_label: &'static str,
    dominant_row: usize,
    dominant_item_index: usize,
    site: FimHotspotSite,
    memory_region: FimRetryMemoryRegion,
}

impl FimRetryHotspot {
    fn memory_region_from_failure(
        sim: &ReservoirSimulator,
        failure_diagnostics: &FimRetryFailureDiagnostics,
    ) -> FimRetryMemoryRegion {
        if failure_diagnostics.dominant_family_label != "gas" {
            return match failure_diagnostics.hotspot_site {
                FimHotspotSite::Cell(cell_idx) => {
                    let (anchor_i, anchor_j, anchor_k) = cell_ijk(sim, cell_idx);
                    FimRetryMemoryRegion::NonGasReservoirRegion {
                        reference_well_index: nearest_well_reference_index(
                            sim, anchor_i, anchor_j, anchor_k,
                        ),
                        anchor_i,
                        anchor_j,
                        anchor_k,
                    }
                }
                _ => FimRetryMemoryRegion::Exact(failure_diagnostics.hotspot_site),
            };
        }

        match failure_diagnostics.hotspot_site {
            FimHotspotSite::GasInjectorSymmetry {
                injector_well_index,
                vertical_offset,
                ..
            } => FimRetryMemoryRegion::GasReservoirRegion {
                injector_well_index,
                vertical_offset,
            },
            _ => FimRetryMemoryRegion::GasFamily,
        }
    }

    fn from_failure(
        sim: &ReservoirSimulator,
        failure_diagnostics: &FimRetryFailureDiagnostics,
    ) -> Option<Self> {
        match failure_diagnostics.class {
            FimRetryFailureClass::LinearBad => None,
            FimRetryFailureClass::NonlinearBad | FimRetryFailureClass::Mixed => Some(Self {
                dominant_family_label: failure_diagnostics.dominant_family_label,
                dominant_row: failure_diagnostics.dominant_row,
                dominant_item_index: failure_diagnostics.dominant_item_index,
                site: failure_diagnostics.hotspot_site,
                memory_region: Self::memory_region_from_failure(sim, failure_diagnostics),
            }),
        }
    }

    fn same_site(self, other: Self) -> bool {
        const NON_GAS_MEMORY_LATERAL_RADIUS: usize = 1;
        const NON_GAS_MEMORY_VERTICAL_RADIUS: usize = 0;

        match (self.memory_region, other.memory_region) {
            (
                FimRetryMemoryRegion::NonGasReservoirRegion {
                    reference_well_index: left_well,
                    anchor_i: left_i,
                    anchor_j: left_j,
                    anchor_k: left_k,
                },
                FimRetryMemoryRegion::NonGasReservoirRegion {
                    reference_well_index: right_well,
                    anchor_i: right_i,
                    anchor_j: right_j,
                    anchor_k: right_k,
                },
            ) => {
                left_well == right_well
                    && left_i.abs_diff(right_i) <= NON_GAS_MEMORY_LATERAL_RADIUS
                    && left_j.abs_diff(right_j) <= NON_GAS_MEMORY_LATERAL_RADIUS
                    && left_k.abs_diff(right_k) <= NON_GAS_MEMORY_VERTICAL_RADIUS
            }
            _ => self.memory_region == other.memory_region,
        }
    }
}

impl FimGrowthCooldown {
    const HOTSPOT_MEMORY_CLEAR_CLEAN_SUCCESSES: u32 = 2;

    fn extra_clean_successes_for_repeated_hotspot(self) -> u32 {
        match self.hotspot_repeat_failures {
            0..=1 => 0,
            2..=3 => 1,
            4..=7 => 2,
            _ => 3,
        }
    }

    fn clamp_trial_dt(self, trial_dt_days: f64, remaining_dt_days: f64) -> f64 {
        self.cap_dt_days
            .map(|cap_dt_days| trial_dt_days.min(cap_dt_days))
            .unwrap_or(trial_dt_days)
            .min(remaining_dt_days)
    }

    fn clear_hotspot_memory(&mut self) {
        self.hotspot = None;
        self.hotspot_repeat_failures = 0;
        self.hotspot_clean_successes_without_retry = 0;
    }

    fn note_retry_failure(
        &mut self,
        sim: &ReservoirSimulator,
        failure_diagnostics: &FimRetryFailureDiagnostics,
    ) {
        let Some(hotspot) = FimRetryHotspot::from_failure(sim, failure_diagnostics) else {
            self.clear_hotspot_memory();
            return;
        };

        if self
            .hotspot
            .is_some_and(|previous| previous.same_site(hotspot))
        {
            self.hotspot_repeat_failures = self.hotspot_repeat_failures.saturating_add(1);
        } else {
            self.hotspot_repeat_failures = 1;
        }
        self.hotspot = Some(hotspot);
        self.hotspot_clean_successes_without_retry = 0;
    }

    fn cap_growth_decision(
        self,
        decision: AcceptedStepGrowthDecision,
    ) -> AcceptedStepGrowthDecision {
        if decision.factor <= 1.0 {
            return decision;
        }

        let capped_factor = match self.hotspot_repeat_failures {
            0 | 1 => return decision,
            _ => 1.0,
        };

        if capped_factor >= decision.factor {
            return decision;
        }

        AcceptedStepGrowthDecision {
            factor: capped_factor,
            limiter: "hotspot-repeat",
        }
    }

    fn stabilize_growth_decision(
        self,
        decision: AcceptedStepGrowthDecision,
    ) -> AcceptedStepGrowthDecision {
        if self.cap_dt_days.is_some() && decision.factor < 1.0 {
            return AcceptedStepGrowthDecision {
                factor: 1.0,
                limiter: "cooldown-hold",
            };
        }

        decision
    }

    fn note_retry_accepted(&mut self, accepted_dt_days: f64, clean_successes_required: u32) {
        self.cap_dt_days = Some(accepted_dt_days);
        let extra_clean_successes = self.extra_clean_successes_for_repeated_hotspot();
        self.clean_successes_remaining = clean_successes_required + extra_clean_successes;
        self.hotspot_clean_successes_without_retry = 0;
    }

    fn note_clean_accepted(&mut self, materially_changed: bool) {
        if self.clean_successes_remaining > 0 {
            self.clean_successes_remaining -= 1;
            if self.clean_successes_remaining == 0 {
                self.cap_dt_days = None;
            }
            return;
        }

        if self.hotspot.is_some() {
            if !materially_changed {
                return;
            }
            self.hotspot_clean_successes_without_retry += 1;
            if self.hotspot_clean_successes_without_retry
                >= Self::HOTSPOT_MEMORY_CLEAR_CLEAN_SUCCESSES
            {
                self.clear_hotspot_memory();
            }
        }
    }

    fn trace_suffix(self) -> String {
        self.cap_dt_days
            .map(|cap_dt_days| {
                let hotspot_trace = self
                    .hotspot
                    .map(|hotspot| {
                        format!(
                            " hotspot={} row={} item={} site={} repeats={} clear_left={}",
                            hotspot.dominant_family_label,
                            hotspot.dominant_row,
                            hotspot.dominant_item_index,
                            hotspot.site.trace_label(),
                            self.hotspot_repeat_failures,
                            Self::HOTSPOT_MEMORY_CLEAR_CLEAN_SUCCESSES
                                .saturating_sub(self.hotspot_clean_successes_without_retry),
                        )
                    })
                    .unwrap_or_default();
                format!(
                    " cooldown_cap={:.6} clean_left={}{}",
                    cap_dt_days, self.clean_successes_remaining, hotspot_trace
                )
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
fn accepted_step_growth_factor(
    newton_iterations: usize,
    max_saturation_change: f64,
    max_pressure_change_bar: f64,
) -> f64 {
    accepted_step_growth_decision(
        newton_iterations,
        max_saturation_change,
        max_pressure_change_bar,
    )
    .factor
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AcceptedStepGrowthDecision {
    factor: f64,
    limiter: &'static str,
}

fn accepted_step_growth_decision(
    newton_iterations: usize,
    max_saturation_change: f64,
    max_pressure_change_bar: f64,
) -> AcceptedStepGrowthDecision {
    const MAX_GROWTH: f64 = 3.0;
    const MIN_ITERATION_GROWTH: f64 = 1.0;
    const MIN_PHYSICAL_GROWTH: f64 = 0.25;
    const TARGET_NEWTON_ITERS: f64 = 8.0;
    const SMOOTH_NEWTON_GROWTH_WINDOW: usize = 11;
    const SMOOTH_NEWTON_GROWTH_START_FACTOR: f64 = 1.1;
    const SMOOTH_NEWTON_GROWTH_END_FACTOR: f64 = 1.05;
    const TARGET_MAX_SAT_CHANGE: f64 = 0.2;
    const TARGET_MAX_PRESSURE_CHANGE_BAR: f64 = 200.0;

    let raw_iteration_growth =
        (TARGET_NEWTON_ITERS / newton_iterations as f64).clamp(MIN_ITERATION_GROWTH, MAX_GROWTH);
    let smooth_growth_start_iteration = TARGET_NEWTON_ITERS as usize + 1;
    let smooth_growth_end_iteration =
        smooth_growth_start_iteration + SMOOTH_NEWTON_GROWTH_WINDOW - 1;
    let iteration_growth = if (smooth_growth_start_iteration..=smooth_growth_end_iteration)
        .contains(&newton_iterations)
    {
        let growth_progress = if SMOOTH_NEWTON_GROWTH_WINDOW <= 1 {
            0.0
        } else {
            (newton_iterations - smooth_growth_start_iteration) as f64
                / (SMOOTH_NEWTON_GROWTH_WINDOW - 1) as f64
        };
        let smooth_growth_floor = SMOOTH_NEWTON_GROWTH_START_FACTOR
            + (SMOOTH_NEWTON_GROWTH_END_FACTOR - SMOOTH_NEWTON_GROWTH_START_FACTOR)
                * growth_progress;
        raw_iteration_growth.max(smooth_growth_floor)
    } else {
        raw_iteration_growth
    };
    let sat_growth = if max_saturation_change > TARGET_MAX_SAT_CHANGE {
        TARGET_MAX_SAT_CHANGE / max_saturation_change
    } else {
        MAX_GROWTH
    };
    let pressure_growth = if max_pressure_change_bar > TARGET_MAX_PRESSURE_CHANGE_BAR {
        TARGET_MAX_PRESSURE_CHANGE_BAR / max_pressure_change_bar
    } else {
        MAX_GROWTH
    };

    let mut decision = AcceptedStepGrowthDecision {
        factor: MAX_GROWTH,
        limiter: "max-growth",
    };

    for (factor, limiter) in [
        (iteration_growth, "newton-iters"),
        (sat_growth, "sat-change"),
        (pressure_growth, "pressure-change"),
    ] {
        if factor < decision.factor {
            decision = AcceptedStepGrowthDecision { factor, limiter };
        }
    }

    decision.factor = decision.factor.clamp(MIN_PHYSICAL_GROWTH, MAX_GROWTH);
    if (decision.factor - MAX_GROWTH).abs() < 1e-12 {
        decision.limiter = "max-growth";
    }
    decision
}

#[cfg(test)]
fn replayable_unchanged_cooldown_accepts(
    cooldown: FimGrowthCooldown,
    accepted_dt_days: f64,
    remaining_dt_days: f64,
) -> u32 {
    if cooldown.cap_dt_days != Some(accepted_dt_days) || cooldown.clean_successes_remaining == 0 {
        return 0;
    }

    let mut replayable = 0_u32;
    let mut replay_time_days = 0.0;
    while replayable < cooldown.clean_successes_remaining
        && replay_time_days + accepted_dt_days <= remaining_dt_days + 1e-12
    {
        replayable += 1;
        replay_time_days += accepted_dt_days;
    }

    replayable
}

fn replayable_unchanged_accepts_after_retry(
    mut cooldown: FimGrowthCooldown,
    accepted_dt_days: f64,
    remaining_dt_days: f64,
) -> (u32, u32) {
    let mut cooldown_replayable = 0_u32;
    let mut plateau_replayable = 0_u32;
    let mut replay_time_days = 0.0;

    while replay_time_days + accepted_dt_days <= remaining_dt_days + 1e-12 {
        if cooldown.cap_dt_days == Some(accepted_dt_days) && cooldown.clean_successes_remaining > 0 {
            cooldown.note_clean_accepted(false);
            cooldown_replayable += 1;
            replay_time_days += accepted_dt_days;
            continue;
        }

        if cooldown.cap_dt_days.is_none()
            && cooldown.hotspot.is_some()
            && cooldown.hotspot_repeat_failures >= 2
        {
            plateau_replayable += 1;
            replay_time_days += accepted_dt_days;
            continue;
        }

        break;
    }

    (cooldown_replayable, plateau_replayable)
}

fn replayable_unchanged_hotspot_plateau_accepts(
    cooldown: FimGrowthCooldown,
    accepted_dt_days: f64,
    remaining_dt_days: f64,
) -> u32 {
    if cooldown.cap_dt_days.is_some()
        || cooldown.hotspot.is_none()
        || cooldown.hotspot_repeat_failures < 2
    {
        return 0;
    }

    let mut replayable = 0_u32;
    let mut replay_time_days = 0.0;
    while replay_time_days + accepted_dt_days <= remaining_dt_days + 1e-12 {
        replayable += 1;
        replay_time_days += accepted_dt_days;
    }

    replayable
}

fn accelerated_retry_factor_for_repeated_hotspot_failure(
    base_retry_factor: f64,
    repeated_same_hotspot_failures: u32,
    hotspot_repeat_failures: u32,
    trial_dt_days: f64,
    report: &crate::fim::newton::FimStepReport,
) -> f64 {
    let Some(failure_diagnostics) = &report.failure_diagnostics else {
        return base_retry_factor;
    };

    if failure_diagnostics.class != FimRetryFailureClass::NonlinearBad
        || repeated_same_hotspot_failures < 2
        || hotspot_repeat_failures < 2
        || trial_dt_days <= 1.0e-4
        || report.newton_iterations != 1
        || failure_diagnostics.linear_iterations != Some(1)
    {
        return base_retry_factor;
    }

    base_retry_factor.min(0.2)
}

fn proposed_trial_dt_days(
    sim_time_days: f64,
    substeps: u32,
    remaining_dt_days: f64,
    last_successful_dt_days: f64,
    last_growth_factor: f64,
) -> f64 {
    const INITIAL_OUTER_STEP_SMALL_TARGET_CAP_THRESHOLD_DAYS: f64 = 1.0;
    const INITIAL_OUTER_STEP_SMALL_TARGET_TRIAL_CAP_DAYS: f64 = 0.25;
    const INITIAL_OUTER_STEP_LARGE_TARGET_TRIAL_CAP_DAYS: f64 = 1.0;

    if substeps == 0 {
        if sim_time_days <= 1e-12 {
            let initial_trial_cap_days = if remaining_dt_days
                <= INITIAL_OUTER_STEP_SMALL_TARGET_CAP_THRESHOLD_DAYS + 1e-12
            {
                INITIAL_OUTER_STEP_SMALL_TARGET_TRIAL_CAP_DAYS
            } else {
                INITIAL_OUTER_STEP_LARGE_TARGET_TRIAL_CAP_DAYS
            };
            remaining_dt_days.min(initial_trial_cap_days)
        } else {
            remaining_dt_days
        }
    } else {
        remaining_dt_days.min(last_successful_dt_days * last_growth_factor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GasOuterStepTrialCarryover {
    cap_dt_days: f64,
    clean_steps_remaining: u32,
}

fn seed_gas_outer_step_trial_carryover(
    min_accepted_dt_days: Option<f64>,
    max_accepted_dt_days: Option<f64>,
    last_accepted_dt_days: Option<f64>,
    last_growth_limiter: Option<&str>,
    last_retry_dominant_family: Option<&str>,
) -> Option<GasOuterStepTrialCarryover> {
    const GAS_OUTER_STEP_CARRYOVER_CLEAN_PERSISTENCE_STEPS: u32 = 3;

    let accepted_dt_days = last_accepted_dt_days?;

    if last_growth_limiter != Some("hotspot-repeat")
        || last_retry_dominant_family != Some("gas")
    {
        return None;
    }

    let min_dt_days = min_accepted_dt_days?;
    let max_dt_days = max_accepted_dt_days?;
    if (min_dt_days - accepted_dt_days).abs() > 1e-12
        || (max_dt_days - accepted_dt_days).abs() > 1e-12
    {
        return None;
    }

    Some(GasOuterStepTrialCarryover {
        cap_dt_days: accepted_dt_days,
        clean_steps_remaining: GAS_OUTER_STEP_CARRYOVER_CLEAN_PERSISTENCE_STEPS,
    })
}

fn next_gas_outer_step_trial_carryover(
    existing_carryover: Option<GasOuterStepTrialCarryover>,
    linear_bad_retries: usize,
    nonlinear_bad_retries: usize,
    mixed_retries: usize,
    min_accepted_dt_days: Option<f64>,
    max_accepted_dt_days: Option<f64>,
    last_accepted_dt_days: Option<f64>,
    last_growth_limiter: Option<&str>,
    last_retry_dominant_family: Option<&str>,
) -> Option<GasOuterStepTrialCarryover> {
    if let Some(seed) = seed_gas_outer_step_trial_carryover(
        min_accepted_dt_days,
        max_accepted_dt_days,
        last_accepted_dt_days,
        last_growth_limiter,
        last_retry_dominant_family,
    ) {
        return Some(seed);
    }

    let existing_carryover = existing_carryover?;
    if linear_bad_retries + nonlinear_bad_retries + mixed_retries > 0
        || existing_carryover.clean_steps_remaining == 0
    {
        return None;
    }

    Some(GasOuterStepTrialCarryover {
        cap_dt_days: existing_carryover.cap_dt_days,
        clean_steps_remaining: existing_carryover.clean_steps_remaining - 1,
    })
}

fn fim_linear_report_step_suffix(linear_report: Option<&FimLinearSolveReport>) -> String {
    linear_report
        .map(|report| {
            format!(
                " lin=[used={} rows={} direct_thr={}]",
                report.backend_used.label(),
                report.solution.len(),
                active_direct_solve_row_threshold(),
            )
        })
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn store_fim_outer_step_stats(
    sim: &mut ReservoirSimulator,
    target_dt_days: f64,
    advanced_dt_days: f64,
    accepted_substeps: u32,
    real_accepted_substeps: u32,
    replayed_cooldown_accepts: u32,
    replayed_hotspot_plateau_accepts: u32,
    accepted_rungs: Vec<FimAcceptedRungStats>,
    retry_rungs: Vec<FimRetryRungStats>,
    linear_bad_retries: usize,
    nonlinear_bad_retries: usize,
    mixed_retries: usize,
    min_accepted_dt_days: Option<f64>,
    max_accepted_dt_days: Option<f64>,
    last_accepted_dt_days: Option<f64>,
    last_growth_factor: f64,
    growth_limiter: Option<&str>,
    hotspot_repeat_suppressed_newton_iters_growth_count: u32,
    last_retry_class: Option<&str>,
    last_retry_dominant_family: Option<&str>,
    last_retry_dominant_row: Option<usize>,
    solver_ms: f64,
    accepted_solver_ms: f64,
    retry_solver_ms: f64,
    assembly_ms: f64,
    property_eval_ms: f64,
    linear_solve_ms: f64,
    linear_preconditioner_ms: f64,
    state_update_ms: f64,
) {
    sim.store_fim_step_stats(FimStepStats {
        time_days: sim.time_days,
        target_dt_days,
        advanced_dt_days,
        accepted_substeps,
        real_accepted_substeps: Some(real_accepted_substeps),
        replayed_cooldown_accepts: Some(replayed_cooldown_accepts),
        replayed_hotspot_plateau_accepts: Some(replayed_hotspot_plateau_accepts),
        accepted_rungs: Some(accepted_rungs),
        retry_rungs: Some(retry_rungs),
        linear_bad_retries,
        nonlinear_bad_retries,
        mixed_retries,
        min_accepted_dt_days,
        max_accepted_dt_days,
        last_accepted_dt_days,
        last_growth_factor: last_accepted_dt_days.map(|_| last_growth_factor),
        growth_limiter: growth_limiter.map(str::to_string),
        hotspot_repeat_suppressed_newton_iters_growth_count: Some(
            hotspot_repeat_suppressed_newton_iters_growth_count,
        ),
        last_retry_class: last_retry_class.map(str::to_string),
        last_retry_dominant_family: last_retry_dominant_family.map(str::to_string),
        last_retry_dominant_row,
        solver_ms: Some(solver_ms),
        accepted_solver_ms: Some(accepted_solver_ms),
        retry_solver_ms: Some(retry_solver_ms),
        assembly_ms: Some(assembly_ms),
        property_eval_ms: Some(property_eval_ms),
        linear_solve_ms: Some(linear_solve_ms),
        linear_preconditioner_ms: Some(linear_preconditioner_ms),
        state_update_ms: Some(state_update_ms),
    });
}

/// Linearly extrapolate a scalar between two accepted-state snapshots by the
/// ratio `dt_next / dt_prev`. This is the OPM Flow `extrapolateInitialGuess`
/// rule reduced to a single component.
fn linear_extrapolate_scalar(prev: f64, curr: f64, dt_ratio: f64) -> f64 {
    curr + (curr - prev) * dt_ratio
}

/// Build a fully extrapolated `FimState` from the two most recent
/// clean-accepted states, applied globally (per-cell + per-well + per-perf).
/// The regime is inherited from `curr` — regime flips are precisely what
/// this probe is meant to surface as residual amplification, so we do NOT
/// re-classify here.
fn globally_extrapolated_state(prev: &FimState, curr: &FimState, dt_ratio: f64) -> FimState {
    let cells: Vec<FimCellState> = prev
        .cells
        .iter()
        .zip(curr.cells.iter())
        .map(|(p, c)| FimCellState {
            pressure_bar: linear_extrapolate_scalar(p.pressure_bar, c.pressure_bar, dt_ratio),
            sw: linear_extrapolate_scalar(p.sw, c.sw, dt_ratio).clamp(0.0, 1.0),
            hydrocarbon_var: linear_extrapolate_scalar(
                p.hydrocarbon_var,
                c.hydrocarbon_var,
                dt_ratio,
            ),
            regime: c.regime,
        })
        .collect();
    let well_bhp: Vec<f64> = prev
        .well_bhp
        .iter()
        .zip(curr.well_bhp.iter())
        .map(|(p, c)| linear_extrapolate_scalar(*p, *c, dt_ratio))
        .collect();
    let perforation_rates_m3_day: Vec<f64> = prev
        .perforation_rates_m3_day
        .iter()
        .zip(curr.perforation_rates_m3_day.iter())
        .map(|(p, c)| linear_extrapolate_scalar(*p, *c, dt_ratio))
        .collect();
    FimState {
        cells,
        well_bhp,
        perforation_rates_m3_day,
    }
}

/// Per-family scaled residual inf-norm and dominant-cell index for the
/// three cell-component families (water/oil/gas). Well/perforation rows
/// are excluded — regime-flip / basin-escape risk lives in cell space,
/// and including well slack rows would dilute the signal.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct BasinEscapeFamily {
    pub(crate) inf_norm: f64,
    pub(crate) top_cell_idx: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct BasinEscapeProbeResult {
    pub(crate) water: BasinEscapeFamily,
    pub(crate) oil: BasinEscapeFamily,
    pub(crate) gas: BasinEscapeFamily,
}

impl BasinEscapeProbeResult {
    fn total_inf_norm(&self) -> f64 {
        self.water
            .inf_norm
            .max(self.oil.inf_norm)
            .max(self.gas.inf_norm)
    }

    fn top_family_and_cell(&self) -> (&'static str, Option<usize>) {
        let (label, family) = [
            ("water", self.water),
            ("oil", self.oil),
            ("gas", self.gas),
        ]
        .into_iter()
        .max_by(|a, b| a.1.inf_norm.partial_cmp(&b.1.inf_norm).unwrap())
        .unwrap();
        (label, family.top_cell_idx)
    }
}

/// Evaluate the residual at `state` using the same assembly the solver uses,
/// scale per-equation the same way the solver's convergence check does, and
/// reduce to per-family inf-norms. Pure diagnostic call — no Newton, no
/// linear solve, no state mutation.
pub(crate) fn evaluate_basin_escape_residual(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    dt_days: f64,
) -> BasinEscapeProbeResult {
    let assembly = assemble_fim_system(
        sim,
        previous_state,
        state,
        &FimAssemblyOptions {
            dt_days,
            include_wells: true,
            assemble_residual_only: true,
            topology: None,
        },
    );
    let n_cells = state.cells.len();
    let mut water = BasinEscapeFamily::default();
    let mut oil = BasinEscapeFamily::default();
    let mut gas = BasinEscapeFamily::default();
    for cell_idx in 0..n_cells {
        let wr = (assembly.residual[equation_offset(cell_idx, 0)]
            / assembly.equation_scaling.water[cell_idx])
            .abs();
        let or = (assembly.residual[equation_offset(cell_idx, 1)]
            / assembly.equation_scaling.oil_component[cell_idx])
            .abs();
        let gr = (assembly.residual[equation_offset(cell_idx, 2)]
            / assembly.equation_scaling.gas_component[cell_idx])
            .abs();
        if wr > water.inf_norm {
            water.inf_norm = wr;
            water.top_cell_idx = Some(cell_idx);
        }
        if or > oil.inf_norm {
            oil.inf_norm = or;
            oil.top_cell_idx = Some(cell_idx);
        }
        if gr > gas.inf_norm {
            gas.inf_norm = gr;
            gas.top_cell_idx = Some(cell_idx);
        }
    }
    BasinEscapeProbeResult { water, oil, gas }
}

pub(crate) fn step_internal(sim: &mut ReservoirSimulator, target_dt_days: f64) {
    sim.step_internal_fim(target_dt_days);
}

impl ReservoirSimulator {
    pub(crate) fn append_fim_trace_line(&mut self, line: &str) {
        if !self.capture_fim_trace {
            return;
        }
        if !self.last_fim_trace.is_empty() {
            self.last_fim_trace.push('\n');
        }
        self.last_fim_trace.push_str(line);
    }

    fn step_internal_fim(&mut self, target_dt_days: f64) {
        self.step_internal_fim_impl(target_dt_days, false);
    }

    fn step_internal_fim_impl(&mut self, target_dt_days: f64, verbose: bool) {
        let mut time_stepped = 0.0;
        const MAX_SUBSTEPS: u32 = 100_000;
        const MAX_NEWTON_RETRIES_PER_SUBSTEP: u32 = 16;
        const MAX_GROWTH: f64 = 3.0;
        const TARGET_MAX_SAT_CHANGE: f64 = 0.2;
        const TARGET_MAX_PRESSURE_CHANGE_BAR: f64 = 200.0;
        const RETRY_GROWTH_COOLDOWN_CLEAN_SUCCESSES: u32 = 2;
        let mut substeps = 0;
        let mut linear_bad_retries = 0usize;
        let mut nonlinear_bad_retries = 0usize;
        let mut mixed_retries = 0usize;
        let mut real_accepted_substeps = 0_u32;
        let mut replayed_cooldown_accepts = 0_u32;
        let mut replayed_hotspot_plateau_accepts = 0_u32;
        let mut accepted_rungs: Vec<FimAcceptedRungStats> = Vec::new();
        let mut retry_rungs: Vec<FimRetryRungStats> = Vec::new();
        let mut min_accepted_dt_days: Option<f64> = None;
        let mut max_accepted_dt_days: Option<f64> = None;
        let mut last_accepted_dt_days: Option<f64> = None;
        let mut last_growth_limiter: Option<&'static str> = None;
        let mut hotspot_repeat_suppressed_newton_iters_growth_count = 0u32;
        let mut last_retry_class: Option<&'static str> = None;
        let mut last_retry_dominant_family: Option<&'static str> = None;
        let mut last_retry_dominant_row: Option<usize> = None;
        self.last_solver_warning = String::new();
        let mut last_successful_dt = target_dt_days;
        let mut last_growth_factor = MAX_GROWTH;
        let mut growth_cooldown = FimGrowthCooldown::default();
        let carried_gas_outer_step_trial_carryover = self.gas_outer_step_trial_carryover.take();
        let mut solver_ms = 0.0;
        let mut accepted_solver_ms = 0.0;
        let mut retry_solver_ms = 0.0;
        let mut assembly_ms = 0.0;
        let mut property_eval_ms = 0.0;
        let mut linear_solve_ms = 0.0;
        let mut linear_preconditioner_ms = 0.0;
        let mut state_update_ms = 0.0;
        // Basin-escape probe history: last two clean-accepted, materially-changed
        // snapshots with the dt that produced each one. Diagnostic only.
        let mut basin_escape_prev_prev: Option<(FimState, f64)> = None;
        let mut basin_escape_prev: Option<(FimState, f64)> = None;

        fim_trace!(
            self,
            verbose,
            "FIM step: target_dt={:.6} days, t={:.6} days",
            target_dt_days,
            self.time_days
        );

        let mut newton_options = FimNewtonOptions::default();
        newton_options.verbose = verbose;
        newton_options.max_saturation_change = TARGET_MAX_SAT_CHANGE;
        newton_options.max_pressure_change_bar = TARGET_MAX_PRESSURE_CHANGE_BAR;

        while time_stepped < target_dt_days && substeps < MAX_SUBSTEPS {
            let remaining_dt = target_dt_days - time_stepped;
            let proposed_trial = proposed_trial_dt_days(
                self.time_days,
                substeps,
                remaining_dt,
                last_successful_dt,
                last_growth_factor,
            );
            let cooldown_clamped_trial = growth_cooldown.clamp_trial_dt(proposed_trial, remaining_dt);
            let gas_carryover_clamped_trial = if substeps == 0 {
                carried_gas_outer_step_trial_carryover
                    .map(|carryover| cooldown_clamped_trial.min(carryover.cap_dt_days))
                    .unwrap_or(cooldown_clamped_trial)
            } else {
                cooldown_clamped_trial
            };
            let initial_trial = gas_carryover_clamped_trial;
            let gas_carryover_trace = if substeps == 0
                && carried_gas_outer_step_trial_carryover.is_some_and(|carryover| {
                    carryover.cap_dt_days + 1e-12 < cooldown_clamped_trial
                })
            {
                let carryover = carried_gas_outer_step_trial_carryover.expect("checked above");
                format!(
                    " [gas-carryover-clamped from {:.6} persist_left={}]",
                    cooldown_clamped_trial,
                    carryover.clean_steps_remaining,
                )
            } else {
                String::new()
            };
            let cooldown_trace = if cooldown_clamped_trial + 1e-12 < proposed_trial {
                format!(
                    " [cooldown-clamped from {:.6}{}]{}",
                    proposed_trial,
                    growth_cooldown.trace_suffix(),
                    gas_carryover_trace,
                )
            } else {
                format!("{}{}", growth_cooldown.trace_suffix(), gas_carryover_trace)
            };
            let mut trial_dt = initial_trial;
            let mut retry_count = 0;
            let mut previous_retry_hotspot: Option<FimRetryHotspot> = None;
            let mut repeated_same_hotspot_failures = 0_u32;
            let previous_state = FimState::from_simulator(self);

            loop {
                fim_trace!(
                    self,
                    verbose,
                    "  substep {}: trial_dt={:.6} (retry={}){}",
                    substeps,
                    trial_dt,
                    retry_count,
                    if retry_count == 0 {
                        cooldown_trace.as_str()
                    } else {
                        ""
                    }
                );

                let report = run_fim_timestep(
                    self,
                    &previous_state,
                    &previous_state,
                    trial_dt,
                    &newton_options,
                );
                solver_ms += report.total_time_ms;
                if report.converged {
                    accepted_solver_ms += report.total_time_ms;
                } else {
                    retry_solver_ms += report.total_time_ms;
                }
                assembly_ms += report.assembly_ms;
                property_eval_ms += report.property_eval_ms;
                linear_solve_ms += report.linear_solve_time_ms;
                linear_preconditioner_ms += report.linear_preconditioner_build_time_ms;
                state_update_ms += report.state_update_ms;

                if report.converged {
                    let materially_changed = iterate_has_material_change(
                        &previous_state,
                        &report.accepted_state,
                    );
                    let mut max_dsat: f64 = 0.0;
                    let mut max_dp: f64 = 0.0;
                    for (idx, new_cell) in report.accepted_state.cells.iter().enumerate() {
                        let old_cell = &previous_state.cells[idx];
                        max_dsat = max_dsat.max((new_cell.sw - old_cell.sw).abs());
                        max_dp = max_dp.max((new_cell.pressure_bar - old_cell.pressure_bar).abs());
                        let old_sg = match old_cell.regime {
                            HydrocarbonState::Saturated => old_cell.hydrocarbon_var.max(0.0),
                            HydrocarbonState::Undersaturated => 0.0,
                        };
                        let new_sg = match new_cell.regime {
                            HydrocarbonState::Saturated => new_cell.hydrocarbon_var.max(0.0),
                            HydrocarbonState::Undersaturated => 0.0,
                        };
                        max_dsat = max_dsat.max((new_sg - old_sg).abs());
                        let _ = idx;
                    }
                    let unchanged_retry_accept = retry_count > 0
                        && report.newton_iterations == 1
                        && report.final_update_inf_norm == 0.0
                        && !materially_changed;

                    let uncapped_growth_decision =
                        accepted_step_growth_decision(report.newton_iterations, max_dsat, max_dp);
                    let growth_decision = growth_cooldown.stabilize_growth_decision(
                        growth_cooldown.cap_growth_decision(uncapped_growth_decision),
                    );
                    if growth_decision.limiter == "hotspot-repeat"
                        && uncapped_growth_decision.limiter == "newton-iters"
                    {
                        hotspot_repeat_suppressed_newton_iters_growth_count += 1;
                    }
                    let adjusted_growth_decision =
                        if retry_count > 0 && growth_decision.factor < 1.0 {
                            AcceptedStepGrowthDecision {
                                factor: 1.0,
                                limiter: "retry-hold",
                            }
                        } else {
                            growth_decision
                        };
                    let unchanged_hotspot_plateau_accept = retry_count == 0
                        && report.newton_iterations == 1
                        && report.final_update_inf_norm == 0.0
                        && !materially_changed
                        && adjusted_growth_decision.factor == 1.0
                        && adjusted_growth_decision.limiter == "hotspot-repeat";
                    last_growth_factor = adjusted_growth_decision.factor;
                    last_growth_limiter = Some(adjusted_growth_decision.limiter);
                    min_accepted_dt_days = Some(
                        min_accepted_dt_days
                            .map(|value| value.min(trial_dt))
                            .unwrap_or(trial_dt),
                    );
                    max_accepted_dt_days = Some(
                        max_accepted_dt_days
                            .map(|value| value.max(trial_dt))
                            .unwrap_or(trial_dt),
                    );
                    last_accepted_dt_days = Some(trial_dt);

                    if retry_count > 0 {
                        growth_cooldown
                            .note_retry_accepted(trial_dt, RETRY_GROWTH_COOLDOWN_CLEAN_SUCCESSES);
                    } else {
                        growth_cooldown.note_clean_accepted(materially_changed);
                    }

                    fim_trace!(
                        self,
                        verbose,
                        "  substep {}: ACCEPTED dt={:.6} iters={} res={:.3e} mb={:.3e} upd={:.3e} max_dSat={:.4} max_dP={:.2} growth={:.3}{}{}",
                        substeps,
                        trial_dt,
                        report.newton_iterations,
                        report.final_residual_inf_norm,
                        report.final_material_balance_inf_norm,
                        report.final_update_inf_norm,
                        max_dsat,
                        max_dp,
                        last_growth_factor,
                        growth_cooldown.trace_suffix(),
                        fim_linear_report_step_suffix(report.last_linear_report.as_ref())
                    );
                    let water_before = self.total_water_inventory_m3();
                    let oil_before = self.total_oil_inventory_sc();
                    let gas_before = self.total_gas_inventory_sc();
                    report.accepted_state.write_back_to_simulator(self);
                    self.update_dynamic_well_productivity_indices();
                    let water_after = self.total_water_inventory_m3();
                    let oil_after = self.total_oil_inventory_sc();
                    let gas_after = self.total_gas_inventory_sc();
                    self.record_fim_step_report(
                        &report.accepted_state,
                        trial_dt,
                        water_after - water_before,
                        oil_before - oil_after,
                        gas_after - gas_before,
                    );
                    accepted_rungs.push(FimAcceptedRungStats {
                        substep: substeps,
                        dt_days: trial_dt,
                        newton_iterations: report.newton_iterations,
                        linear_backend: report
                            .last_linear_report
                            .as_ref()
                            .map(|linear_report| linear_report.backend_used.label().to_string()),
                        linear_iterations: report
                            .last_linear_report
                            .as_ref()
                            .map(|linear_report| linear_report.iterations),
                        linear_solve_ms: report.linear_solve_time_ms,
                        linear_preconditioner_ms: report.linear_preconditioner_build_time_ms,
                    });

                    // Basin-escape diagnostic probe: only on real, clean,
                    // materially-changed accepts (no retries, no replay) and
                    // only once we have two prior such accepts to extrapolate
                    // from. Pure diagnostic — does not touch controller state.
                    if retry_count == 0 && materially_changed {
                        if let (Some((prev_prev_state, _prev_prev_dt)), Some((prev_state, prev_dt))) =
                            (basin_escape_prev_prev.as_ref(), basin_escape_prev.as_ref())
                        {
                            let dt_ratio = if *prev_dt > 0.0 {
                                trial_dt / *prev_dt
                            } else {
                                1.0
                            };
                            let extrapolated = globally_extrapolated_state(
                                prev_prev_state,
                                prev_state,
                                dt_ratio,
                            );
                            let baseline = evaluate_basin_escape_residual(
                                self,
                                &previous_state,
                                prev_state,
                                trial_dt,
                            );
                            let probe = evaluate_basin_escape_residual(
                                self,
                                &previous_state,
                                &extrapolated,
                                trial_dt,
                            );
                            let baseline_norm = baseline.total_inf_norm();
                            let probe_norm = probe.total_inf_norm();
                            let amp = if baseline_norm > 0.0 {
                                probe_norm / baseline_norm
                            } else {
                                f64::INFINITY
                            };
                            let (top_family, top_cell) = probe.top_family_and_cell();
                            let top_cell_trace = match top_cell {
                                Some(idx) => {
                                    let (i, j, k) = cell_ijk(self, idx);
                                    format!("cell{}(ijk={},{},{})", idx, i, j, k)
                                }
                                None => "none".to_string(),
                            };
                            fim_trace!(
                                self,
                                verbose,
                                "  substep {}: BASIN-ESCAPE PROBE dt_ratio={:.3} res_prev={:.3e} res_extrap={:.3e} amp={:.2e} top=[{}] site={} fam=[water={:.3e},oil={:.3e},gas={:.3e}]",
                                substeps,
                                dt_ratio,
                                baseline_norm,
                                probe_norm,
                                amp,
                                top_family,
                                top_cell_trace,
                                probe.water.inf_norm,
                                probe.oil.inf_norm,
                                probe.gas.inf_norm
                            );
                        }
                        basin_escape_prev_prev = basin_escape_prev.take();
                        basin_escape_prev = Some((report.accepted_state.clone(), trial_dt));
                    }

                    self.time_days += trial_dt;
                    time_stepped += trial_dt;
                    last_successful_dt = trial_dt;
                    real_accepted_substeps += 1;
                    substeps += 1;

                    let (replayed_cooldown, replayed_plateau, replayable_accepts) =
                        if unchanged_retry_accept {
                            let (cooldown_replayable, plateau_replayable) =
                                replayable_unchanged_accepts_after_retry(
                                    growth_cooldown,
                                    trial_dt,
                                    target_dt_days - time_stepped,
                                );
                            (
                                cooldown_replayable,
                                plateau_replayable,
                                cooldown_replayable + plateau_replayable,
                            )
                        } else if unchanged_hotspot_plateau_accept {
                            let plateau_replayable = replayable_unchanged_hotspot_plateau_accepts(
                                growth_cooldown,
                                trial_dt,
                                target_dt_days - time_stepped,
                            );
                            (0, plateau_replayable, plateau_replayable)
                        } else {
                            (0, 0, 0)
                        };

                    if unchanged_retry_accept {
                        replayed_cooldown_accepts += replayed_cooldown;
                        replayed_hotspot_plateau_accepts += replayed_plateau;
                    } else if unchanged_hotspot_plateau_accept {
                        replayed_hotspot_plateau_accepts += replayed_plateau;
                    }

                    if replayable_accepts > 0 {
                        for _ in 0..replayable_accepts {
                            growth_cooldown.note_clean_accepted(false);
                        }

                        let replayed_dt_days = trial_dt * replayable_accepts as f64;
                        let replay_trace_suffix = if unchanged_retry_accept {
                            format!(
                                " [replayed unchanged accepts cooldown={} hotspot-plateau={}]",
                                replayed_cooldown, replayed_plateau
                            )
                        } else if unchanged_hotspot_plateau_accept {
                            format!(
                                " [replayed unchanged hotspot plateau accepts count={}]",
                                replayed_plateau
                            )
                        } else {
                            String::new()
                        };
                        fim_trace!(
                            self,
                            verbose,
                            "  substep {}: ACCEPTED dt={:.6} iters={} res={:.3e} mb={:.3e} upd={:.3e} max_dSat={:.4} max_dP={:.2} growth={:.3}{}{}{}",
                            substeps,
                            replayed_dt_days,
                            report.newton_iterations,
                            report.final_residual_inf_norm,
                            report.final_material_balance_inf_norm,
                            report.final_update_inf_norm,
                            max_dsat,
                            max_dp,
                            last_growth_factor,
                            growth_cooldown.trace_suffix(),
                            fim_linear_report_step_suffix(report.last_linear_report.as_ref()),
                            replay_trace_suffix
                        );
                        self.record_fim_step_report(
                            &report.accepted_state,
                            replayed_dt_days,
                            0.0,
                            0.0,
                            0.0,
                        );
                        self.time_days += replayed_dt_days;
                        time_stepped += replayed_dt_days;
                        last_successful_dt = trial_dt;
                        last_accepted_dt_days = Some(trial_dt);
                        substeps += 1;
                    }

                    break;
                }

                if let Some(failure_diagnostics) = &report.failure_diagnostics {
                    let current_retry_hotspot = FimRetryHotspot::from_failure(self, failure_diagnostics);
                    if let Some(current_retry_hotspot) = current_retry_hotspot {
                        if previous_retry_hotspot
                            .is_some_and(|previous| previous.same_site(current_retry_hotspot))
                        {
                            repeated_same_hotspot_failures =
                                repeated_same_hotspot_failures.saturating_add(1);
                        } else {
                            repeated_same_hotspot_failures = 1;
                        }
                        previous_retry_hotspot = Some(current_retry_hotspot);
                    } else {
                        repeated_same_hotspot_failures = 0;
                        previous_retry_hotspot = None;
                    }
                } else {
                    repeated_same_hotspot_failures = 0;
                    previous_retry_hotspot = None;
                }

                let effective_retry_factor = accelerated_retry_factor_for_repeated_hotspot_failure(
                    report.retry_factor,
                    repeated_same_hotspot_failures,
                    growth_cooldown.hotspot_repeat_failures,
                    trial_dt,
                    &report,
                );
                let next_dt = trial_dt * effective_retry_factor.clamp(0.1, 0.5);
                retry_count += 1;

                if let Some(failure_diagnostics) = &report.failure_diagnostics {
                    growth_cooldown.note_retry_failure(self, failure_diagnostics);
                    last_retry_class = Some(failure_diagnostics.class.label());
                    last_retry_dominant_family = Some(failure_diagnostics.dominant_family_label);
                    last_retry_dominant_row = Some(failure_diagnostics.dominant_row);
                    retry_rungs.push(FimRetryRungStats {
                        substep: substeps,
                        dt_days: trial_dt,
                        newton_iterations: report.newton_iterations,
                        linear_backend: report
                            .last_linear_report
                            .as_ref()
                            .map(|linear_report| linear_report.backend_used.label().to_string()),
                        linear_iterations: report
                            .last_linear_report
                            .as_ref()
                            .map(|linear_report| linear_report.iterations),
                        linear_solve_ms: report.linear_solve_time_ms,
                        linear_preconditioner_ms: report.linear_preconditioner_build_time_ms,
                        retry_class: Some(failure_diagnostics.class.label().to_string()),
                        dominant_family: Some(failure_diagnostics.dominant_family_label.to_string()),
                        dominant_row: Some(failure_diagnostics.dominant_row),
                    });
                    match failure_diagnostics.class {
                        FimRetryFailureClass::LinearBad => linear_bad_retries += 1,
                        FimRetryFailureClass::NonlinearBad => nonlinear_bad_retries += 1,
                        FimRetryFailureClass::Mixed => mixed_retries += 1,
                    }
                }

                fim_trace!(
                    self,
                    verbose,
                    "  substep {}: FAILED (iters={} res={:.3e} mb={:.3e} upd={:.3e} retry_factor={:.2}){}{} → next_dt={:.6}",
                    substeps,
                    report.newton_iterations,
                    report.final_residual_inf_norm,
                    report.final_material_balance_inf_norm,
                    report.final_update_inf_norm,
                    effective_retry_factor,
                    report
                        .failure_diagnostics
                        .as_ref()
                        .map(|diagnostics| {
                            format!(
                                " [retry_class={} dom={}]",
                                diagnostics.class.label(),
                                diagnostics.dominant_family_label,
                            )
                        })
                        .unwrap_or_default(),
                    fim_linear_report_step_suffix(report.last_linear_report.as_ref()),
                    next_dt
                );

                if !next_dt.is_finite() || next_dt <= 1e-12 {
                    fim_trace!(
                        self,
                        verbose,
                        "  ABORT: timestep collapsed to {:.3e}",
                        next_dt
                    );
                    self.last_solver_warning = format!(
                        "FIM Newton step collapsed timestep at t={:.6} days after {} iterations",
                        self.time_days + time_stepped,
                        report.newton_iterations
                    );
                    store_fim_outer_step_stats(
                        self,
                        target_dt_days,
                        time_stepped,
                        substeps,
                        real_accepted_substeps,
                        replayed_cooldown_accepts,
                        replayed_hotspot_plateau_accepts,
                        accepted_rungs,
                        retry_rungs,
                        linear_bad_retries,
                        nonlinear_bad_retries,
                        mixed_retries,
                        min_accepted_dt_days,
                        max_accepted_dt_days,
                        last_accepted_dt_days,
                        last_growth_factor,
                        last_growth_limiter,
                        hotspot_repeat_suppressed_newton_iters_growth_count,
                        last_retry_class,
                        last_retry_dominant_family,
                        last_retry_dominant_row,
                        solver_ms,
                        accepted_solver_ms,
                        retry_solver_ms,
                        assembly_ms,
                        property_eval_ms,
                        linear_solve_ms,
                        linear_preconditioner_ms,
                        state_update_ms,
                    );
                    self.gas_outer_step_trial_carryover = None;
                    return;
                }

                if retry_count >= MAX_NEWTON_RETRIES_PER_SUBSTEP {
                    fim_trace!(
                        self,
                        verbose,
                        "  ABORT: exceeded retry budget ({} retries)",
                        retry_count
                    );
                    self.last_solver_warning = format!(
                        "FIM Newton step exceeded retry budget at t={:.6} days after {} retries",
                        self.time_days + time_stepped,
                        retry_count
                    );
                    store_fim_outer_step_stats(
                        self,
                        target_dt_days,
                        time_stepped,
                        substeps,
                        real_accepted_substeps,
                        replayed_cooldown_accepts,
                        replayed_hotspot_plateau_accepts,
                        accepted_rungs,
                        retry_rungs,
                        linear_bad_retries,
                        nonlinear_bad_retries,
                        mixed_retries,
                        min_accepted_dt_days,
                        max_accepted_dt_days,
                        last_accepted_dt_days,
                        last_growth_factor,
                        last_growth_limiter,
                        hotspot_repeat_suppressed_newton_iters_growth_count,
                        last_retry_class,
                        last_retry_dominant_family,
                        last_retry_dominant_row,
                        solver_ms,
                        accepted_solver_ms,
                        retry_solver_ms,
                        assembly_ms,
                        property_eval_ms,
                        linear_solve_ms,
                        linear_preconditioner_ms,
                        state_update_ms,
                    );
                    self.gas_outer_step_trial_carryover = None;
                    return;
                }

                trial_dt = next_dt;
            }
        }

        if substeps == MAX_SUBSTEPS && time_stepped < target_dt_days {
            fim_trace!(self, verbose, "  ABORT: hit MAX_SUBSTEPS={}", MAX_SUBSTEPS);
            self.last_solver_warning = format!(
                "FIM adaptive timestep hit MAX_SUBSTEPS before completing requested dt (advanced {:.6} of {:.6} days)",
                time_stepped, target_dt_days
            );
        }

        fim_trace!(
            self,
            verbose,
            "FIM step done: {} substeps, advanced {:.6} of {:.6} days",
            substeps,
            time_stepped,
            target_dt_days
        );
        if linear_bad_retries + nonlinear_bad_retries + mixed_retries > 0 {
            fim_trace!(
                self,
                verbose,
                "FIM retry summary: linear-bad={} nonlinear-bad={} mixed={}",
                linear_bad_retries,
                nonlinear_bad_retries,
                mixed_retries
            );
        }

        store_fim_outer_step_stats(
            self,
            target_dt_days,
            time_stepped,
            substeps,
            real_accepted_substeps,
            replayed_cooldown_accepts,
            replayed_hotspot_plateau_accepts,
            accepted_rungs,
            retry_rungs,
            linear_bad_retries,
            nonlinear_bad_retries,
            mixed_retries,
            min_accepted_dt_days,
            max_accepted_dt_days,
            last_accepted_dt_days,
            last_growth_factor,
            last_growth_limiter,
            hotspot_repeat_suppressed_newton_iters_growth_count,
            last_retry_class,
            last_retry_dominant_family,
            last_retry_dominant_row,
            solver_ms,
            accepted_solver_ms,
            retry_solver_ms,
            assembly_ms,
            property_eval_ms,
            linear_solve_ms,
            linear_preconditioner_ms,
            state_update_ms,
        );
        self.gas_outer_step_trial_carryover = next_gas_outer_step_trial_carryover(
            carried_gas_outer_step_trial_carryover,
            linear_bad_retries,
            nonlinear_bad_retries,
            mixed_retries,
            min_accepted_dt_days,
            max_accepted_dt_days,
            last_accepted_dt_days,
            last_growth_limiter,
            last_retry_dominant_family,
        );
    }

    fn total_water_inventory_m3(&self) -> f64 {
        (0..self.nx * self.ny * self.nz)
            .map(|idx| self.sat_water[idx] * self.pore_volume_m3(idx))
            .sum()
    }

    fn total_oil_inventory_sc(&self) -> f64 {
        (0..self.nx * self.ny * self.nz)
            .map(|idx| {
                let pore_volume_m3 = self.pore_volume_m3(idx).max(1e-9);
                let bo = self.get_b_o_cell(idx, self.pressure[idx]).max(1e-9);
                self.sat_oil[idx] * pore_volume_m3 / bo
            })
            .sum()
    }

    fn total_gas_inventory_sc(&self) -> f64 {
        if !self.three_phase_mode {
            return 0.0;
        }

        (0..self.nx * self.ny * self.nz)
            .map(|idx| {
                let pore_volume_m3 = self.pore_volume_m3(idx).max(1e-9);
                let free_gas_sc =
                    self.sat_gas[idx] * pore_volume_m3 / self.get_b_g(self.pressure[idx]).max(1e-9);
                let dissolved_gas_sc = if self.pvt_table.is_some() {
                    self.sat_oil[idx] * pore_volume_m3 * self.rs[idx]
                        / self.get_b_o_cell(idx, self.pressure[idx]).max(1e-9)
                } else {
                    0.0
                };
                free_gas_sc + dissolved_gas_sc
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AcceptedStepGrowthDecision, FimGrowthCooldown, GasOuterStepTrialCarryover,
        accelerated_retry_factor_for_repeated_hotspot_failure, accepted_step_growth_factor,
        next_gas_outer_step_trial_carryover, seed_gas_outer_step_trial_carryover,
        proposed_trial_dt_days, replayable_unchanged_accepts_after_retry,
        replayable_unchanged_cooldown_accepts,
        replayable_unchanged_hotspot_plateau_accepts,
    };
    use crate::ReservoirSimulator;
    use crate::fim::newton::{FimHotspotSite, FimRetryFailureClass, FimRetryFailureDiagnostics};
    use crate::fim::newton::FimStepReport;
    use crate::fim::state::FimState;

    fn failure_diagnostics(
        class: FimRetryFailureClass,
        dominant_row: usize,
        dominant_item_index: usize,
    ) -> FimRetryFailureDiagnostics {
        FimRetryFailureDiagnostics {
            class,
            dominant_family_label: "water",
            dominant_row,
            dominant_item_index,
            hotspot_site: FimHotspotSite::Cell(dominant_item_index),
            linear_iterations: Some(12),
            used_linear_fallback: false,
            cpr_average_reduction_ratio: Some(1e-13),
            cpr_last_reduction_ratio: Some(1e-14),
        }
    }

    fn hotspot_memory_sim() -> ReservoirSimulator {
        let mut sim = ReservoirSimulator::new(20, 20, 3, 0.2);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(19, 19, 0, 100.0, 0.1, 0.0, false).unwrap();
        sim
    }

    #[test]
    fn retry_acceptance_freezes_growth_until_clean_success_budget_is_spent() {
        let mut cooldown = FimGrowthCooldown::default();
        cooldown.note_retry_accepted(0.25, 2);

        assert_eq!(cooldown.clamp_trial_dt(0.3125, 1.0), 0.25);

        cooldown.note_clean_accepted(true);
        assert_eq!(cooldown.clamp_trial_dt(0.3125, 1.0), 0.25);

        cooldown.note_clean_accepted(true);
        assert_eq!(cooldown.clamp_trial_dt(0.3125, 1.0), 0.3125);
    }

    #[test]
    fn cooldown_clamp_never_exceeds_remaining_dt() {
        let mut cooldown = FimGrowthCooldown::default();
        cooldown.note_retry_accepted(0.25, 2);

        assert_eq!(cooldown.clamp_trial_dt(0.5, 0.1), 0.1);
    }

    #[test]
    fn repeated_same_hotspot_extends_growth_cooldown_budget() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let failure = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 429, 143);

        cooldown.note_retry_failure(&sim, &failure);
        cooldown.note_retry_accepted(0.25, 2);
        assert_eq!(cooldown.clean_successes_remaining, 2);

        cooldown.note_retry_failure(&sim, &failure);
        cooldown.note_retry_accepted(0.25, 2);
        assert_eq!(cooldown.clean_successes_remaining, 2);
        assert!(cooldown.trace_suffix().contains("repeats=2"));
    }

    #[test]
    fn alternating_cell_row_families_count_as_same_hotspot_site() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let water_failure = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 429, 143);
        let oil_failure = FimRetryFailureDiagnostics {
            dominant_family_label: "oil",
            dominant_row: 430,
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 430, 143)
        };

        cooldown.note_retry_failure(&sim, &water_failure);
        cooldown.note_retry_failure(&sim, &oil_failure);

        assert_eq!(cooldown.hotspot_repeat_failures, 2);
    }

    #[test]
    fn repeated_hotspot_caps_max_growth_decision() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let failure = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 429, 143);

        cooldown.note_retry_failure(&sim, &failure);
        cooldown.note_retry_failure(
            &sim,
            &FimRetryFailureDiagnostics {
                dominant_family_label: "oil",
                dominant_row: 430,
                ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 430, 143)
            },
        );

        let capped = cooldown.cap_growth_decision(AcceptedStepGrowthDecision {
            factor: 1.25,
            limiter: "max-growth",
        });

        assert_eq!(capped.factor, 1.0);
        assert_eq!(capped.limiter, "hotspot-repeat");
    }

    #[test]
    fn repeated_hotspot_also_caps_newton_limited_regrowth() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let failure = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 11, 3);

        cooldown.note_retry_failure(&sim, &failure);
        cooldown.note_retry_failure(&sim, &failure);

        let capped = cooldown.cap_growth_decision(AcceptedStepGrowthDecision {
            factor: 1.25,
            limiter: "newton-iters",
        });

        assert_eq!(capped.factor, 1.0);
        assert_eq!(capped.limiter, "hotspot-repeat");
    }

    #[test]
    fn alternating_gas_cells_with_shared_injector_symmetry_site_count_as_same_hotspot() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let east_failure = FimRetryFailureDiagnostics {
            dominant_family_label: "gas",
            dominant_row: 11,
            dominant_item_index: 3,
            hotspot_site: FimHotspotSite::GasInjectorSymmetry {
                injector_well_index: 0,
                major_offset: 3,
                minor_offset: 0,
                vertical_offset: 0,
            },
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 11, 3)
        };
        let north_failure = FimRetryFailureDiagnostics {
            dominant_family_label: "gas",
            dominant_row: 83,
            dominant_item_index: 30,
            hotspot_site: FimHotspotSite::GasInjectorSymmetry {
                injector_well_index: 0,
                major_offset: 3,
                minor_offset: 0,
                vertical_offset: 0,
            },
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 83, 30)
        };

        cooldown.note_retry_failure(&sim, &east_failure);
        cooldown.note_retry_failure(&sim, &north_failure);

        assert_eq!(cooldown.hotspot_repeat_failures, 2);
        assert_eq!(
            cooldown.hotspot.expect("hotspot").site,
            FimHotspotSite::GasInjectorSymmetry {
                injector_well_index: 0,
                major_offset: 3,
                minor_offset: 0,
                vertical_offset: 0,
            }
        );
    }

    #[test]
    fn gas_hotspots_in_same_injector_region_count_as_same_memory_region() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let east_failure = FimRetryFailureDiagnostics {
            dominant_family_label: "gas",
            dominant_row: 11,
            dominant_item_index: 3,
            hotspot_site: FimHotspotSite::GasInjectorSymmetry {
                injector_well_index: 0,
                major_offset: 3,
                minor_offset: 0,
                vertical_offset: 0,
            },
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 11, 3)
        };
        let farther_failure = FimRetryFailureDiagnostics {
            dominant_family_label: "gas",
            dominant_row: 122,
            dominant_item_index: 41,
            hotspot_site: FimHotspotSite::GasInjectorSymmetry {
                injector_well_index: 0,
                major_offset: 5,
                minor_offset: 2,
                vertical_offset: 0,
            },
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 122, 41)
        };

        cooldown.note_retry_failure(&sim, &east_failure);
        cooldown.note_retry_failure(&sim, &farther_failure);

        assert_eq!(cooldown.hotspot_repeat_failures, 2);
    }

    #[test]
    fn gas_hotspots_on_different_vertical_offsets_do_not_share_memory_region() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let lower_failure = FimRetryFailureDiagnostics {
            dominant_family_label: "gas",
            dominant_row: 11,
            dominant_item_index: 3,
            hotspot_site: FimHotspotSite::GasInjectorSymmetry {
                injector_well_index: 0,
                major_offset: 3,
                minor_offset: 0,
                vertical_offset: 0,
            },
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 11, 3)
        };
        let upper_failure = FimRetryFailureDiagnostics {
            dominant_family_label: "gas",
            dominant_row: 211,
            dominant_item_index: 103,
            hotspot_site: FimHotspotSite::GasInjectorSymmetry {
                injector_well_index: 0,
                major_offset: 3,
                minor_offset: 0,
                vertical_offset: 1,
            },
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 211, 103)
        };

        cooldown.note_retry_failure(&sim, &lower_failure);
        cooldown.note_retry_failure(&sim, &upper_failure);

        assert_eq!(cooldown.hotspot_repeat_failures, 1);
    }

    #[test]
    fn gas_family_fallback_memory_groups_non_symmetry_gas_failures() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let gas_cell_a = FimRetryFailureDiagnostics {
            dominant_family_label: "gas",
            dominant_row: 50,
            dominant_item_index: 10,
            hotspot_site: FimHotspotSite::Cell(10),
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 50, 10)
        };
        let gas_cell_b = FimRetryFailureDiagnostics {
            dominant_family_label: "gas",
            dominant_row: 77,
            dominant_item_index: 22,
            hotspot_site: FimHotspotSite::Cell(22),
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 77, 22)
        };

        cooldown.note_retry_failure(&sim, &gas_cell_a);
        cooldown.note_retry_failure(&sim, &gas_cell_b);

        assert_eq!(cooldown.hotspot_repeat_failures, 2);
    }

    #[test]
    fn nearby_non_gas_cells_share_local_memory_region() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let first = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 189, sim.idx(3, 3, 0));
        let nearby = FimRetryFailureDiagnostics {
            dominant_family_label: "oil",
            dominant_row: 250,
            dominant_item_index: sim.idx(3, 4, 0),
            hotspot_site: FimHotspotSite::Cell(sim.idx(3, 4, 0)),
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 250, sim.idx(3, 4, 0))
        };

        cooldown.note_retry_failure(&sim, &first);
        cooldown.note_retry_failure(&sim, &nearby);

        assert_eq!(cooldown.hotspot_repeat_failures, 2);
    }

    #[test]
    fn vertical_non_gas_shift_does_not_share_local_memory_region() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let lower = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 189, sim.idx(3, 3, 0));
        let upper = FimRetryFailureDiagnostics {
            dominant_family_label: "oil",
            dominant_row: 1390,
            dominant_item_index: sim.idx(3, 3, 1),
            hotspot_site: FimHotspotSite::Cell(sim.idx(3, 3, 1)),
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 1390, sim.idx(3, 3, 1))
        };

        cooldown.note_retry_failure(&sim, &lower);
        cooldown.note_retry_failure(&sim, &upper);

        assert_eq!(cooldown.hotspot_repeat_failures, 1);
    }

    #[test]
    fn far_non_gas_cells_do_not_share_local_memory_region() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let first = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 189, sim.idx(3, 3, 0));
        let far = FimRetryFailureDiagnostics {
            dominant_family_label: "oil",
            dominant_row: 840,
            dominant_item_index: sim.idx(8, 8, 0),
            hotspot_site: FimHotspotSite::Cell(sim.idx(8, 8, 0)),
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 840, sim.idx(8, 8, 0))
        };

        cooldown.note_retry_failure(&sim, &first);
        cooldown.note_retry_failure(&sim, &far);

        assert_eq!(cooldown.hotspot_repeat_failures, 1);
    }

    #[test]
    fn nearby_non_gas_cells_near_different_wells_do_not_share_memory_region() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let injector_side =
            failure_diagnostics(FimRetryFailureClass::NonlinearBad, 189, sim.idx(3, 3, 0));
        let producer_side = FimRetryFailureDiagnostics {
            dominant_family_label: "oil",
            dominant_row: 3190,
            dominant_item_index: sim.idx(16, 16, 0),
            hotspot_site: FimHotspotSite::Cell(sim.idx(16, 16, 0)),
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 3190, sim.idx(16, 16, 0))
        };

        cooldown.note_retry_failure(&sim, &injector_side);
        cooldown.note_retry_failure(&sim, &producer_side);

        assert_eq!(cooldown.hotspot_repeat_failures, 1);
    }

    #[test]
    fn cooldown_holds_growth_flat_instead_of_shrinking() {
        let mut cooldown = FimGrowthCooldown::default();
        cooldown.note_retry_accepted(0.25, 2);

        let held = cooldown.stabilize_growth_decision(AcceptedStepGrowthDecision {
            factor: 0.75,
            limiter: "newton-iters",
        });

        assert_eq!(held.factor, 1.0);
        assert_eq!(held.limiter, "cooldown-hold");
    }

    #[test]
    fn changing_hotspot_resets_extra_growth_cooldown_budget() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        cooldown.note_retry_failure(
            &sim,
            &failure_diagnostics(FimRetryFailureClass::NonlinearBad, 429, 143),
        );
        cooldown.note_retry_failure(
            &sim,
            &failure_diagnostics(FimRetryFailureClass::NonlinearBad, 429, 143),
        );
        cooldown.note_retry_accepted(0.25, 2);
        assert_eq!(cooldown.clean_successes_remaining, 2);

        cooldown.note_retry_failure(
            &sim,
            &failure_diagnostics(FimRetryFailureClass::NonlinearBad, 297, 99),
        );
        cooldown.note_retry_accepted(0.2, 2);
        assert_eq!(cooldown.clean_successes_remaining, 2);
        assert!(cooldown.trace_suffix().contains("row=297"));
    }

    #[test]
    fn repeated_hotspot_increases_clean_success_budget() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let failure = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 429, 143);

        cooldown.note_retry_failure(&sim, &failure);
        cooldown.note_retry_failure(&sim, &failure);
        cooldown.note_retry_failure(&sim, &failure);
        cooldown.note_retry_failure(&sim, &failure);
        cooldown.note_retry_accepted(0.25, 2);

        assert_eq!(cooldown.clean_successes_remaining, 4);
    }

    #[test]
    fn linear_failure_does_not_seed_hotspot_memory() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        cooldown.note_retry_failure(
            &sim,
            &failure_diagnostics(FimRetryFailureClass::LinearBad, 10, 3),
        );
        cooldown.note_retry_accepted(0.25, 2);

        assert_eq!(cooldown.clean_successes_remaining, 2);
        assert!(!cooldown.trace_suffix().contains("hotspot="));
    }

    #[test]
    fn hotspot_memory_persists_across_release_and_decays_after_clean_steps() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let failure = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 429, 143);

        cooldown.note_retry_failure(&sim, &failure);
        cooldown.note_retry_accepted(0.25, 2);
        cooldown.note_clean_accepted(true);
        cooldown.note_clean_accepted(true);

        assert_eq!(cooldown.hotspot_repeat_failures, 1);

        cooldown.note_retry_failure(&sim, &failure);
        cooldown.note_retry_accepted(0.25, 2);
        assert_eq!(cooldown.clean_successes_remaining, 3);

        cooldown.note_clean_accepted(true);
        cooldown.note_clean_accepted(true);
        cooldown.note_clean_accepted(true);
        cooldown.note_clean_accepted(true);
        cooldown.note_clean_accepted(true);

        assert!(cooldown.hotspot.is_none());
    }

    #[test]
    fn unchanged_clean_accepts_do_not_decay_hotspot_memory() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let failure = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 429, 143);

        cooldown.note_retry_failure(&sim, &failure);
        cooldown.note_retry_failure(&sim, &failure);

        cooldown.note_clean_accepted(false);
        cooldown.note_clean_accepted(false);
        cooldown.note_clean_accepted(false);

        assert!(cooldown.hotspot.is_some());
        assert_eq!(cooldown.hotspot_repeat_failures, 2);
    }

    #[test]
    fn residual_margin_no_longer_throttles_growth_factor() {
        let growth = accepted_step_growth_factor(2, 0.003, 0.2);
        assert!((growth - 3.0).abs() < 1e-12);
    }

    #[test]
    fn accepted_step_iterations_do_not_shrink_converged_dt() {
        let growth = accepted_step_growth_factor(20, 0.03, 12.0);
        assert!((growth - 1.0).abs() < 1e-12);
    }

    #[test]
    fn accepted_step_iterations_just_above_target_can_regrow_slowly() {
        let growth = accepted_step_growth_factor(12, 0.03, 12.0);
        assert!((growth - 1.085).abs() < 1e-12);
    }

    #[test]
    fn accepted_step_iterations_fourteen_can_regrow_slowly() {
        let growth = accepted_step_growth_factor(14, 0.03, 12.0);
        assert!((growth - 1.075).abs() < 1e-12);
    }

    #[test]
    fn accepted_step_iterations_sixteen_can_regrow_very_slowly() {
        let growth = accepted_step_growth_factor(16, 0.03, 12.0);
        assert!((growth - 1.065).abs() < 1e-12);
    }

    #[test]
    fn accepted_step_iterations_eighteen_can_regrow_very_slowly() {
        let growth = accepted_step_growth_factor(18, 0.03, 12.0);
        assert!((growth - 1.055).abs() < 1e-12);
    }

    #[test]
    fn accepted_step_iterations_nineteen_can_regrow_very_slowly() {
        let growth = accepted_step_growth_factor(19, 0.03, 12.0);
        assert!((growth - 1.05).abs() < 1e-12);
    }

    #[test]
    fn physical_change_limits_can_still_shrink_growth() {
        let growth = accepted_step_growth_factor(6, 0.35, 12.0);
        assert!((growth - (0.2 / 0.35)).abs() < 1e-12);
    }

    #[test]
    fn cold_start_first_outer_step_caps_initial_trial_dt() {
        let trial = proposed_trial_dt_days(0.0, 0, 31.0, 31.0, 3.0);
        assert!((trial - 1.0).abs() < 1e-12);
    }

    #[test]
    fn cold_start_small_target_first_outer_step_caps_initial_trial_dt_more_tightly() {
        let trial = proposed_trial_dt_days(0.0, 0, 1.0, 1.0, 3.0);
        assert!((trial - 0.25).abs() < 1e-12);
    }

    #[test]
    fn non_startup_first_outer_step_keeps_full_trial_dt() {
        let trial = proposed_trial_dt_days(10.0, 0, 31.0, 1.0, 3.0);
        assert!((trial - 31.0).abs() < 1e-12);
    }

    #[test]
    fn later_substeps_still_use_growth_based_trial_dt() {
        let trial = proposed_trial_dt_days(0.0, 3, 20.0, 0.75, 1.6);
        assert!((trial - 1.2).abs() < 1e-12);
    }

    #[test]
    fn flat_hotspot_repeat_gas_shelf_seeds_next_outer_step_trial_cap() {
        assert_eq!(
            seed_gas_outer_step_trial_carryover(
                Some(0.0625),
                Some(0.0625),
                Some(0.0625),
                Some("hotspot-repeat"),
                Some("gas"),
            ),
            Some(GasOuterStepTrialCarryover {
                cap_dt_days: 0.0625,
                clean_steps_remaining: 3,
            })
        );
    }

    #[test]
    fn varying_gas_accept_dt_does_not_seed_outer_step_trial_cap() {
        assert_eq!(
            seed_gas_outer_step_trial_carryover(
                Some(0.004042),
                Some(0.04232),
                Some(0.004042),
                Some("hotspot-repeat"),
                Some("gas"),
            ),
            None
        );
    }

    #[test]
    fn non_gas_hotspot_repeat_does_not_seed_outer_step_trial_cap() {
        assert_eq!(
            seed_gas_outer_step_trial_carryover(
                Some(0.0625),
                Some(0.0625),
                Some(0.0625),
                Some("hotspot-repeat"),
                Some("water"),
            ),
            None
        );
    }

    #[test]
    fn clean_step_preserves_gas_outer_step_trial_cap_for_three_steps() {
        assert_eq!(
            next_gas_outer_step_trial_carryover(
                Some(GasOuterStepTrialCarryover {
                    cap_dt_days: 0.0625,
                    clean_steps_remaining: 3,
                }),
                0,
                0,
                0,
                Some(0.05005),
                Some(0.07089),
                Some(0.05005),
                Some("newton-iters"),
                None,
            ),
            Some(GasOuterStepTrialCarryover {
                cap_dt_days: 0.0625,
                clean_steps_remaining: 2,
            })
        );
    }

    #[test]
    fn second_clean_step_preserves_gas_outer_step_trial_cap_for_one_more_step() {
        assert_eq!(
            next_gas_outer_step_trial_carryover(
                Some(GasOuterStepTrialCarryover {
                    cap_dt_days: 0.0625,
                    clean_steps_remaining: 2,
                }),
                0,
                0,
                0,
                Some(0.04809),
                Some(0.07223),
                Some(0.04809),
                Some("newton-iters"),
                None,
            ),
            Some(GasOuterStepTrialCarryover {
                cap_dt_days: 0.0625,
                clean_steps_remaining: 1,
            })
        );
    }

    #[test]
    fn third_clean_step_exhausts_gas_outer_step_trial_cap() {
        assert_eq!(
            next_gas_outer_step_trial_carryover(
                Some(GasOuterStepTrialCarryover {
                    cap_dt_days: 0.0625,
                    clean_steps_remaining: 1,
                }),
                0,
                0,
                0,
                Some(0.04807),
                Some(0.07256),
                Some(0.04807),
                Some("newton-iters"),
                None,
            ),
            Some(GasOuterStepTrialCarryover {
                cap_dt_days: 0.0625,
                clean_steps_remaining: 0,
            })
        );
    }

    #[test]
    fn retrying_step_does_not_preserve_gas_outer_step_trial_cap_without_reseeding() {
        assert_eq!(
            next_gas_outer_step_trial_carryover(
                Some(GasOuterStepTrialCarryover {
                    cap_dt_days: 0.0625,
                    clean_steps_remaining: 1,
                }),
                0,
                2,
                0,
                Some(0.0625),
                Some(0.0625),
                Some(0.0625),
                Some("newton-iters"),
                Some("gas"),
            ),
            None
        );
    }

    #[test]
    fn unchanged_cooldown_accepts_replay_remaining_clean_steps() {
        let mut cooldown = FimGrowthCooldown::default();
        cooldown.note_retry_accepted(0.25, 2);

        assert_eq!(
            replayable_unchanged_cooldown_accepts(cooldown, 0.25, 1.0),
            2
        );
    }

    #[test]
    fn unchanged_cooldown_replay_stops_at_remaining_time() {
        let mut cooldown = FimGrowthCooldown::default();
        cooldown.note_retry_accepted(0.25, 2);

        assert_eq!(
            replayable_unchanged_cooldown_accepts(cooldown, 0.25, 0.24),
            0
        );
        assert_eq!(
            replayable_unchanged_cooldown_accepts(cooldown, 0.25, 0.26),
            1
        );
    }

    #[test]
    fn unchanged_retry_accepts_can_flow_directly_into_hotspot_plateau_replay() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let failure = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 429, 143);

        cooldown.note_retry_failure(&sim, &failure);
        cooldown.note_retry_failure(&sim, &failure);
        cooldown.note_retry_accepted(0.25, 2);

        assert_eq!(
            replayable_unchanged_accepts_after_retry(cooldown, 0.25, 1.0),
            (3, 1)
        );
        assert_eq!(
            replayable_unchanged_accepts_after_retry(cooldown, 0.25, 0.74),
            (2, 0)
        );
    }

    #[test]
    fn unchanged_hotspot_plateau_replays_full_steps_until_remaining_time() {
        let sim = hotspot_memory_sim();
        let mut cooldown = FimGrowthCooldown::default();
        let failure = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 429, 143);

        cooldown.note_retry_failure(&sim, &failure);
        cooldown.note_retry_failure(&sim, &failure);

        assert_eq!(
            replayable_unchanged_hotspot_plateau_accepts(cooldown, 0.25, 1.0),
            4
        );
        assert_eq!(
            replayable_unchanged_hotspot_plateau_accepts(cooldown, 0.25, 0.24),
            0
        );
    }

    #[test]
    fn unchanged_hotspot_plateau_does_not_replay_while_cooldown_hold_is_active() {
        let mut cooldown = FimGrowthCooldown::default();
        cooldown.note_retry_accepted(0.25, 2);

        assert_eq!(
            replayable_unchanged_hotspot_plateau_accepts(cooldown, 0.25, 1.0),
            0
        );
    }

    #[test]
    fn repeated_one_iteration_nonlinear_hotspot_failure_accelerates_retry_cut() {
        let report = FimStepReport {
            accepted_state: FimState::from_simulator(&ReservoirSimulator::new(1, 1, 1, 0.2)),
            converged: false,
            newton_iterations: 1,
            final_residual_inf_norm: 1.0,
            final_material_balance_inf_norm: 1.0,
            final_update_inf_norm: 0.0,
            last_linear_report: None,
            accepted_hotspot_site: None,
            failure_diagnostics: Some(FimRetryFailureDiagnostics {
                class: FimRetryFailureClass::NonlinearBad,
                dominant_family_label: "water",
                dominant_row: 1020,
                dominant_item_index: 340,
                hotspot_site: FimHotspotSite::Cell(340),
                linear_iterations: Some(1),
                used_linear_fallback: false,
                cpr_average_reduction_ratio: None,
                cpr_last_reduction_ratio: None,
            }),
            retry_factor: 0.5,
            total_time_ms: 0.0,
            assembly_ms: 0.0,
            property_eval_ms: 0.0,
            linear_solve_time_ms: 0.0,
            linear_preconditioner_build_time_ms: 0.0,
            state_update_ms: 0.0,
        };

        assert_eq!(
            accelerated_retry_factor_for_repeated_hotspot_failure(0.5, 1, 2, 1.0e-3, &report),
            0.5
        );
        assert_eq!(
            accelerated_retry_factor_for_repeated_hotspot_failure(0.5, 2, 2, 1.0e-3, &report),
            0.2
        );
        assert_eq!(
            accelerated_retry_factor_for_repeated_hotspot_failure(0.5, 2, 2, 8.0e-5, &report),
            0.5
        );
        assert_eq!(
            accelerated_retry_factor_for_repeated_hotspot_failure(0.5, 2, 1, 1.0e-3, &report),
            0.5
        );
    }

    #[test]
    fn globally_extrapolated_state_linearly_extrapolates_each_scalar_by_dt_ratio() {
        use super::{globally_extrapolated_state, linear_extrapolate_scalar};
        use crate::fim::state::{FimCellState, HydrocarbonState};

        let prev = FimState {
            cells: vec![
                FimCellState {
                    pressure_bar: 100.0,
                    sw: 0.20,
                    hydrocarbon_var: 0.10,
                    regime: HydrocarbonState::Saturated,
                },
                FimCellState {
                    pressure_bar: 150.0,
                    sw: 0.30,
                    hydrocarbon_var: 40.0,
                    regime: HydrocarbonState::Undersaturated,
                },
            ],
            well_bhp: vec![200.0],
            perforation_rates_m3_day: vec![10.0, -5.0],
        };
        let curr = FimState {
            cells: vec![
                FimCellState {
                    pressure_bar: 105.0,
                    sw: 0.22,
                    hydrocarbon_var: 0.11,
                    regime: HydrocarbonState::Saturated,
                },
                FimCellState {
                    pressure_bar: 148.0,
                    sw: 0.31,
                    hydrocarbon_var: 39.0,
                    regime: HydrocarbonState::Undersaturated,
                },
            ],
            well_bhp: vec![198.0],
            perforation_rates_m3_day: vec![12.0, -6.0],
        };
        let dt_ratio = 0.5;
        let extrapolated = globally_extrapolated_state(&prev, &curr, dt_ratio);

        let expected_p0 = linear_extrapolate_scalar(100.0, 105.0, dt_ratio);
        let expected_sw1 = linear_extrapolate_scalar(0.30, 0.31, dt_ratio);
        let expected_bhp = linear_extrapolate_scalar(200.0, 198.0, dt_ratio);
        let expected_rate1 = linear_extrapolate_scalar(-5.0, -6.0, dt_ratio);

        assert!((extrapolated.cells[0].pressure_bar - expected_p0).abs() < 1e-12);
        assert!((extrapolated.cells[1].sw - expected_sw1).abs() < 1e-12);
        assert!((extrapolated.well_bhp[0] - expected_bhp).abs() < 1e-12);
        assert!((extrapolated.perforation_rates_m3_day[1] - expected_rate1).abs() < 1e-12);
        // Regime is inherited from curr, not re-classified.
        assert_eq!(extrapolated.cells[0].regime, HydrocarbonState::Saturated);
        assert_eq!(extrapolated.cells[1].regime, HydrocarbonState::Undersaturated);
    }

    #[test]
    fn globally_extrapolated_state_clamps_sw_into_unit_interval() {
        use super::globally_extrapolated_state;
        use crate::fim::state::{FimCellState, HydrocarbonState};

        let prev = FimState {
            cells: vec![FimCellState {
                pressure_bar: 100.0,
                sw: 0.90,
                hydrocarbon_var: 0.0,
                regime: HydrocarbonState::Undersaturated,
            }],
            well_bhp: vec![],
            perforation_rates_m3_day: vec![],
        };
        let curr = FimState {
            cells: vec![FimCellState {
                pressure_bar: 100.0,
                sw: 0.98,
                hydrocarbon_var: 0.0,
                regime: HydrocarbonState::Undersaturated,
            }],
            well_bhp: vec![],
            perforation_rates_m3_day: vec![],
        };
        // dt_ratio=2 would extrapolate sw to 0.98 + (0.98-0.90)*2 = 1.14,
        // which must clamp to 1.0.
        let extrapolated = globally_extrapolated_state(&prev, &curr, 2.0);
        assert!(extrapolated.cells[0].sw <= 1.0);
        assert!(extrapolated.cells[0].sw >= 0.0);
    }

    #[test]
    fn evaluate_basin_escape_residual_returns_zero_at_previous_state_with_zero_dt() {
        // At zero dt, the accumulation term trivially satisfies the residual
        // equation when state == previous_state (no flux accumulation and no
        // well source contribution for a closed, equilibrium system). This
        // verifies the residual-only assembly path through the probe's public
        // entry point.
        use super::evaluate_basin_escape_residual;

        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_fim_enabled(true);
        let state = FimState::from_simulator(&sim);
        let result = evaluate_basin_escape_residual(&sim, &state, &state, 1.0);
        // A closed 2-cell system at its initial state with matching previous
        // state should have near-zero residual across all three families.
        assert!(result.water.inf_norm < 1e-8);
        assert!(result.oil.inf_norm < 1e-8);
        assert!(result.gas.inf_norm < 1e-8);
    }

    #[test]
    fn fim_enabled_step_advances_time_and_records_history_for_closed_system() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_fim_enabled(true);

        let pressure_before = sim.pressure[0];
        let sw_before = sim.sat_water[0];

        sim.step_internal(1.0);

        assert!((sim.time_days - 1.0).abs() < 1e-12);
        assert_eq!(sim.rate_history.len(), 1);
        assert!((sim.pressure[0] - pressure_before).abs() < 1e-12);
        assert!((sim.sat_water[0] - sw_before).abs() < 1e-12);
        assert!(sim.last_solver_warning.is_empty());
    }
}
