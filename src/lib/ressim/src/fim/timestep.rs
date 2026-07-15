use crate::ReservoirSimulator;
use crate::fim::assembly::{FimAssemblyOptions, equation_offset};
// See newton.rs: production assembly now goes through the AD assembler.
use crate::fim::assembly_ad::assemble_fim_system_ad as assemble_fim_system;
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

/// Bundle N checkpoint 4 (N3, `OpmAligned` only, `docs/FIM_BUNDLE_N_DESIGN.md` §9.3/9.4): OPM
/// shipped `time-step-control=pid+newtoniteration` defaults, verified from the installed
/// `flow` binary and `opm-simulators` `release/2025.10/final` source at Bundle N step 0.
const OPM_TIME_STEP_CONTROL_TOLERANCE: f64 = 0.1;
const OPM_TARGET_NEWTON_ITERATIONS: f64 = 8.0;
const OPM_ITER_DECAY_DAMPING: f64 = 1.0;
const OPM_ITER_GROWTH_DAMPING: f64 = 3.2;
const OPM_SOLVER_RESTART_FACTOR: f64 = 0.33;
const OPM_SOLVER_MAX_GROWTH: f64 = 3.0;
const OPM_SOLVER_GROWTH_FACTOR_AFTER_RESTART: f64 = 2.0;

/// OPM's `BlackoilModel::relativeChange()`: one global scalar comparing consecutive ACCEPTED
/// substep states (not Newton iterations) — sum-of-squares of pressure/saturation deltas,
/// normalized by the sum-of-squares of the new state's own values. Implied `So` participates
/// like the other phases, matching OPM's own (`NB fix me!`-flagged) mixing of pressure and
/// saturation units verbatim — ported as-is, not "improved" (design doc §9.3).
fn opm_relative_change(previous: &FimState, current: &FimState) -> f64 {
    let mut delta = 0.0_f64;
    let mut denom = 0.0_f64;
    for (old_cell, new_cell) in previous.cells.iter().zip(current.cells.iter()) {
        let dp = new_cell.pressure_bar - old_cell.pressure_bar;
        delta += dp * dp;
        denom += new_cell.pressure_bar * new_cell.pressure_bar;

        let sg_of = |cell: &FimCellState| match cell.regime {
            HydrocarbonState::Saturated => cell.hydrocarbon_var.max(0.0),
            HydrocarbonState::Undersaturated => 0.0,
        };
        let new_sg = sg_of(new_cell);
        let old_sg = sg_of(old_cell);
        let new_so = 1.0 - new_cell.sw - new_sg;
        let old_so = 1.0 - old_cell.sw - old_sg;

        for (new_s, old_s) in [
            (new_cell.sw, old_cell.sw),
            (new_sg, old_sg),
            (new_so, old_so),
        ] {
            let ds = new_s - old_s;
            delta += ds * ds;
            denom += new_s * new_s;
        }
    }
    if denom > 0.0 { delta / denom } else { 0.0 }
}

/// OPM's `PIDTimeStepControl::computeTimeStepSize` half of the composite controller.
/// `errors` holds the last three `opm_relative_change` values (oldest first), matching OPM's
/// rolling `errors_` window (constructed filled with `tol`, design doc §9.3).
fn opm_pid_dt(dt_days: f64, errors: [f64; 3]) -> f64 {
    let tol = OPM_TIME_STEP_CONTROL_TOLERANCE;
    let e2 = errors[2];
    if e2 > tol {
        dt_days * tol / e2
    } else if errors[0] == 0.0 || errors[1] == 0.0 || e2 == 0.0 {
        f64::MAX
    } else {
        dt_days
            * (errors[1] / e2).powf(0.075)
            * (tol / e2).powf(0.175)
            * (errors[1] * errors[1] / (errors[0] * e2)).powf(0.01)
    }
}

/// OPM's `PIDAndIterationCountTimeStepControl` iteration-target half: grow/decay dt toward
/// hitting `target-newton-iterations` (8) on the next substep.
fn opm_iteration_count_dt(dt_days: f64, newton_iterations: usize) -> f64 {
    let its = newton_iterations as f64;
    let target = OPM_TARGET_NEWTON_ITERATIONS;
    if its > target {
        dt_days / (1.0 + (its - target) / target * OPM_ITER_DECAY_DAMPING)
    } else {
        dt_days * (1.0 + (target - its) / target * OPM_ITER_GROWTH_DAMPING)
    }
}

/// OPM's full accepted-substep dt proposal: `dt_next = min(dt_pid, dt_iter)`
/// (`computeTimeStepSize`), then `maybeRestrictTimeStepGrowth_`'s two growth ceilings — always
/// `solver-max-growth` (3.0), further tightened to `solver-growth-factor` (2.0) if this substep
/// needed any retries. Returned as a growth FACTOR (matching `AcceptedStepGrowthDecision`) so
/// it composes with the existing `proposed_trial_dt_days(last_successful_dt * last_growth_factor)`
/// call site unchanged.
fn opm_accepted_step_growth_decision(
    dt_days: f64,
    newton_iterations: usize,
    errors: [f64; 3],
    retries_this_substep: u32,
) -> AcceptedStepGrowthDecision {
    let dt_pid = opm_pid_dt(dt_days, errors);
    let dt_iter = opm_iteration_count_dt(dt_days, newton_iterations);

    let mut decision = AcceptedStepGrowthDecision {
        factor: OPM_SOLVER_MAX_GROWTH,
        limiter: "opm-max-growth",
    };
    for (dt_candidate, limiter) in [(dt_pid, "opm-pid"), (dt_iter, "opm-iter")] {
        let factor = dt_candidate / dt_days;
        if factor < decision.factor {
            decision = AcceptedStepGrowthDecision { factor, limiter };
        }
    }
    if retries_this_substep > 0 && OPM_SOLVER_GROWTH_FACTOR_AFTER_RESTART < decision.factor {
        decision = AcceptedStepGrowthDecision {
            factor: OPM_SOLVER_GROWTH_FACTOR_AFTER_RESTART,
            limiter: "opm-restart-growth",
        };
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
        if cooldown.cap_dt_days == Some(accepted_dt_days) && cooldown.clean_successes_remaining > 0
        {
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

    if last_growth_limiter != Some("hotspot-repeat") || last_retry_dominant_family != Some("gas") {
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
        let (label, family) = [("water", self.water), ("oil", self.oil), ("gas", self.gas)]
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
    /// `FIM-DIAG-003` D0/D1 dev setter (`docs/FIM_DIAG_003_PLAN.md`): see
    /// `fim_force_direct_linear` doc comment. No wasm surface — set by the native repro test
    /// driver from `FIM_FORCE_DIRECT_LINEAR`, same pattern as `set_fim_nested_well_solve`.
    pub(crate) fn set_fim_force_direct_linear(&mut self, enabled: bool) {
        self.fim_force_direct_linear = enabled;
    }

    pub(crate) fn append_fim_trace_line(&mut self, line: &str) {
        // Late-window trace diagnostic: runs on the production `step()` path too (independent
        // of `capture_fim_trace`) so a native no-trace run still gets full per-iteration
        // visibility once the dt-collapse window activates. No-op when `FIM_TRACE_FILE` is
        // unset — `fim_trace_window_active` is then always false.
        #[cfg(not(target_arch = "wasm32"))]
        if self.fim_trace_window_active {
            crate::fim::trace_sink::write_line(line);
        }
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
        // Late-window trace diagnostic: `FIM_MAX_SUBSTEPS` lets a windowed rerun abort shortly
        // after the trace window is captured instead of running to completion. Unset (the
        // common/production case) falls back to the same `MAX_SUBSTEPS` constant as before.
        #[cfg(not(target_arch = "wasm32"))]
        let max_substeps =
            crate::fim::trace_sink::max_substeps_override_from_env().unwrap_or(MAX_SUBSTEPS);
        #[cfg(target_arch = "wasm32")]
        let max_substeps = MAX_SUBSTEPS;
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
        let opm_aligned = self.fim_opm_aligned_nonlinear;
        if opm_aligned {
            newton_options.nonlinear_flavor = crate::fim::newton::FimNonlinearFlavor::OpmAligned;
        }
        newton_options.nested_well_solve = self.fim_nested_well_solve;
        newton_options.linear.use_true_fgmres = self.fim_true_fgmres;
        // FIM-DIAG-003 D0/D1: forced-direct-linear switch. Off unless the native repro driver
        // set it from FIM_FORCE_DIRECT_LINEAR; no-op elsewhere including all wasm paths.
        if self.fim_force_direct_linear {
            newton_options.linear.kind = crate::fim::linear::FimLinearSolverKind::SparseLuDebug;
        }
        // Bundle N checkpoint 4 (N3, `OpmAligned` only): rolling `relativeChange()` history for
        // OPM's PID controller half (design doc §9.3), reset each outer step — matching OPM's
        // `PIDTimeStepControl` constructor (`errors_(3, tol_)`). Legacy's cooldown/carryover/
        // hotspot-streak state below keeps updating unconditionally (harmless bookkeeping) but
        // its output is only ever read on the Legacy path.
        let mut opm_pid_errors: [f64; 3] = [OPM_TIME_STEP_CONTROL_TOLERANCE; 3];

        while time_stepped < target_dt_days && substeps < max_substeps {
            let remaining_dt = target_dt_days - time_stepped;
            let proposed_trial = proposed_trial_dt_days(
                self.time_days,
                substeps,
                remaining_dt,
                last_successful_dt,
                last_growth_factor,
            );
            // Bundle N checkpoint 4: the cooldown/gas-carryover trial clamps are Legacy-only
            // hotspot mitigations with no OPM analog; `opm_aligned`'s own growth formula
            // (computed on accept, below) already incorporates OPM's own caution (the
            // iteration-target and post-retry growth ceilings), so skip them here.
            let cooldown_clamped_trial = if opm_aligned {
                proposed_trial.min(remaining_dt)
            } else {
                growth_cooldown.clamp_trial_dt(proposed_trial, remaining_dt)
            };
            let gas_carryover_clamped_trial = if opm_aligned {
                cooldown_clamped_trial
            } else if substeps == 0 {
                carried_gas_outer_step_trial_carryover
                    .map(|carryover| cooldown_clamped_trial.min(carryover.cap_dt_days))
                    .unwrap_or(cooldown_clamped_trial)
            } else {
                cooldown_clamped_trial
            };
            let initial_trial = gas_carryover_clamped_trial;
            let gas_carryover_trace = if substeps == 0
                && carried_gas_outer_step_trial_carryover
                    .is_some_and(|carryover| carryover.cap_dt_days + 1e-12 < cooldown_clamped_trial)
            {
                let carryover = carried_gas_outer_step_trial_carryover.expect("checked above");
                format!(
                    " [gas-carryover-clamped from {:.6} persist_left={}]",
                    cooldown_clamped_trial, carryover.clean_steps_remaining,
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
                // Late-window trace diagnostic: sticky activation, checked every pass
                // (initial trial + every retry) since `trial_dt` shrinks across retries.
                // `should_activate_window` short-circuits on `sink_enabled()` first, so this
                // is a single cheap no-op check when `FIM_TRACE_FILE` is unset.
                #[cfg(not(target_arch = "wasm32"))]
                if !self.fim_trace_window_active
                    && crate::fim::trace_sink::should_activate_window(trial_dt, substeps)
                {
                    self.fim_trace_window_active = true;
                    crate::fim::trace_sink::write_line(&format!(
                        "WINDOW-START substep={} t={:.6} trial_dt={:.9}",
                        substeps, self.time_days, trial_dt
                    ));
                }

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
                    let materially_changed =
                        iterate_has_material_change(&previous_state, &report.accepted_state);
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

                    // Bundle N checkpoint 4 (N3, `OpmAligned` only): OPM's own
                    // `pid+newtoniteration` growth decision replaces the Legacy iteration/
                    // sat/pressure-based decision AND its cooldown-capping/stabilization and
                    // "retry-hold" override — those are Legacy-specific mitigations for a
                    // problem (the hotspot-repeat pathology) that OPM's own growth ceilings
                    // (`opm-max-growth`/`opm-restart-growth`) already guard against directly.
                    let adjusted_growth_decision = if opm_aligned {
                        let rel_change =
                            opm_relative_change(&previous_state, &report.accepted_state);
                        opm_pid_errors = [opm_pid_errors[1], opm_pid_errors[2], rel_change];
                        opm_accepted_step_growth_decision(
                            trial_dt,
                            report.newton_iterations,
                            opm_pid_errors,
                            retry_count,
                        )
                    } else {
                        let uncapped_growth_decision = accepted_step_growth_decision(
                            report.newton_iterations,
                            max_dsat,
                            max_dp,
                        );
                        let growth_decision = growth_cooldown.stabilize_growth_decision(
                            growth_cooldown.cap_growth_decision(uncapped_growth_decision),
                        );
                        if growth_decision.limiter == "hotspot-repeat"
                            && uncapped_growth_decision.limiter == "newton-iters"
                        {
                            hotspot_repeat_suppressed_newton_iters_growth_count += 1;
                        }
                        if retry_count > 0 && growth_decision.factor < 1.0 {
                            AcceptedStepGrowthDecision {
                                factor: 1.0,
                                limiter: "retry-hold",
                            }
                        } else {
                            growth_decision
                        }
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

                    // Late-window trace diagnostic: one ledger line per accepted substep,
                    // always on when the sink is enabled (not just inside the trace window) —
                    // substep-scale BHP/perforation-rate oscillation may already be visible
                    // here without needing the finer per-iteration WELLTRACE lines.
                    #[cfg(not(target_arch = "wasm32"))]
                    if crate::fim::trace_sink::sink_enabled() {
                        crate::fim::trace_sink::write_line(&format!(
                            "LEDGER accept substep={} t={:.6} dt={:.9} iters={} retry_count={} growth={:.3} limiter={} res={:.6e} mb={:.6e} bhp={:?} q={:?}",
                            substeps,
                            self.time_days + trial_dt,
                            trial_dt,
                            report.newton_iterations,
                            retry_count,
                            adjusted_growth_decision.factor,
                            adjusted_growth_decision.limiter,
                            report.final_residual_inf_norm,
                            report.final_material_balance_inf_norm,
                            report.accepted_state.well_bhp,
                            report.accepted_state.perforation_rates_m3_day,
                        ));
                    }

                    // Basin-escape diagnostic probe: only on real, clean,
                    // materially-changed accepts (no retries, no replay) and
                    // only once we have two prior such accepts to extrapolate
                    // from. Pure diagnostic — does not touch controller state.
                    if retry_count == 0 && materially_changed {
                        if let (
                            Some((prev_prev_state, _prev_prev_dt)),
                            Some((prev_state, prev_dt)),
                        ) = (basin_escape_prev_prev.as_ref(), basin_escape_prev.as_ref())
                        {
                            let dt_ratio = if *prev_dt > 0.0 {
                                trial_dt / *prev_dt
                            } else {
                                1.0
                            };
                            let extrapolated =
                                globally_extrapolated_state(prev_prev_state, prev_state, dt_ratio);
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
                    let current_retry_hotspot =
                        FimRetryHotspot::from_failure(self, failure_diagnostics);
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

                // Bundle N checkpoint 4 (N3, `OpmAligned` only): OPM's flat
                // `solver-restart-factor` (0.33) replaces both ResSim's failure-classified
                // `retry_factor` and the repeated-hotspot acceleration on top of it — OPM's
                // retry backoff carries no failure-family or site memory at all.
                let effective_retry_factor = if opm_aligned {
                    OPM_SOLVER_RESTART_FACTOR
                } else {
                    accelerated_retry_factor_for_repeated_hotspot_failure(
                        report.retry_factor,
                        repeated_same_hotspot_failures,
                        growth_cooldown.hotspot_repeat_failures,
                        trial_dt,
                        &report,
                    )
                };
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
                        dominant_family: Some(
                            failure_diagnostics.dominant_family_label.to_string(),
                        ),
                        dominant_row: Some(failure_diagnostics.dominant_row),
                    });

                    // Late-window trace diagnostic: mirror the accept-side ledger line for
                    // retries, using the last attempted (unconverged) candidate state — still
                    // useful for spotting BHP/perforation-rate oscillation on failed attempts.
                    #[cfg(not(target_arch = "wasm32"))]
                    if crate::fim::trace_sink::sink_enabled() {
                        crate::fim::trace_sink::write_line(&format!(
                            "LEDGER retry substep={} t={:.6} dt={:.9} iters={} retry_count={} retry_class={} dominant={}@{} res={:.6e} mb={:.6e} bhp={:?} q={:?}",
                            substeps,
                            self.time_days,
                            trial_dt,
                            report.newton_iterations,
                            retry_count,
                            failure_diagnostics.class.label(),
                            failure_diagnostics.dominant_family_label,
                            failure_diagnostics.dominant_row,
                            report.final_residual_inf_norm,
                            report.final_material_balance_inf_norm,
                            report.accepted_state.well_bhp,
                            report.accepted_state.perforation_rates_m3_day,
                        ));
                    }
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

        if substeps == max_substeps && time_stepped < target_dt_days {
            fim_trace!(self, verbose, "  ABORT: hit MAX_SUBSTEPS={}", max_substeps);
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
        OPM_SOLVER_GROWTH_FACTOR_AFTER_RESTART, OPM_SOLVER_MAX_GROWTH,
        OPM_TIME_STEP_CONTROL_TOLERANCE, accelerated_retry_factor_for_repeated_hotspot_failure,
        accepted_step_growth_factor, next_gas_outer_step_trial_carryover,
        opm_accepted_step_growth_decision, opm_iteration_count_dt, opm_pid_dt, opm_relative_change,
        proposed_trial_dt_days, replayable_unchanged_accepts_after_retry,
        replayable_unchanged_cooldown_accepts, replayable_unchanged_hotspot_plateau_accepts,
        seed_gas_outer_step_trial_carryover,
    };
    use crate::ReservoirSimulator;
    use crate::fim::newton::FimStepReport;
    use crate::fim::newton::{FimHotspotSite, FimRetryFailureClass, FimRetryFailureDiagnostics};
    use crate::fim::state::{FimCellState, FimState, HydrocarbonState};

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

    fn cell(
        pressure_bar: f64,
        sw: f64,
        hydrocarbon_var: f64,
        regime: HydrocarbonState,
    ) -> FimCellState {
        FimCellState {
            pressure_bar,
            sw,
            hydrocarbon_var,
            regime,
        }
    }

    #[test]
    fn opm_relative_change_is_zero_for_identical_states() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        let state = FimState::from_simulator(&sim);
        assert_eq!(opm_relative_change(&state, &state), 0.0);
    }

    #[test]
    fn opm_relative_change_matches_hand_computed_sum_of_squares() {
        let previous = FimState {
            cells: vec![cell(200.0, 0.3, 0.1, HydrocarbonState::Saturated)],
            well_bhp: vec![],
            perforation_rates_m3_day: vec![],
        };
        let mut current = previous.clone();
        current.cells[0].pressure_bar = 220.0; // dp = 20
        current.cells[0].sw = 0.35; // dSw = 0.05
        current.cells[0].hydrocarbon_var = 0.12; // dSg = 0.02, implied dSo = -0.07

        // new: p=220, Sw=0.35, Sg=0.12, So=0.53; old: p=200, Sw=0.3, Sg=0.1, So=0.6
        let delta = 20.0_f64.powi(2) + 0.05_f64.powi(2) + 0.02_f64.powi(2) + (-0.07_f64).powi(2);
        let denom = 220.0_f64.powi(2) + 0.35_f64.powi(2) + 0.12_f64.powi(2) + 0.53_f64.powi(2);
        let expected = delta / denom;

        assert!((opm_relative_change(&previous, &current) - expected).abs() < 1e-9);
    }

    #[test]
    fn opm_pid_dt_shrinks_when_error_exceeds_tolerance() {
        // e2=0.2 > tol=0.1 -> dt * tol/e2 = dt * 0.5
        let dt = opm_pid_dt(1.0, [0.1, 0.1, 0.2]);
        assert!((dt - 0.5).abs() < 1e-12);
    }

    #[test]
    fn opm_pid_dt_returns_max_when_history_has_a_zero_error() {
        let dt = opm_pid_dt(1.0, [0.0, 0.05, 0.05]);
        assert_eq!(dt, f64::MAX);
    }

    #[test]
    fn opm_pid_dt_uses_full_formula_below_tolerance_with_nonzero_history() {
        // e2=0.05 <= tol=0.1, all errors nonzero -> full PID formula, not the shrink or max branch.
        let dt = opm_pid_dt(1.0, [0.05, 0.05, 0.05]);
        let expected = 1.0
            * (0.05_f64 / 0.05).powf(0.075)
            * (0.1_f64 / 0.05).powf(0.175)
            * (0.05_f64 * 0.05 / (0.05 * 0.05)).powf(0.01);
        assert!((dt - expected).abs() < 1e-9);
        assert!(dt > 1.0, "error well below tolerance should propose growth");
    }

    #[test]
    fn opm_iteration_count_dt_grows_below_target_and_shrinks_above() {
        // its=2 < target=8: dt * (1 + 6/8*3.2) = dt * 3.4
        assert!((opm_iteration_count_dt(1.0, 2) - 3.4).abs() < 1e-12);
        // its=8 == target: no adjustment
        assert!((opm_iteration_count_dt(1.0, 8) - 1.0).abs() < 1e-12);
        // its=16 > target=8: dt / (1 + 8/8*1.0) = dt / 2.0
        assert!((opm_iteration_count_dt(1.0, 16) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn opm_accepted_step_growth_decision_picks_the_tightest_candidate() {
        // A very clean history (errors far below tol) and a low iteration count both
        // propose growth well past the ceiling; the max-growth ceiling (3.0) must win.
        let tiny_errors = [1e-6, 1e-6, 1e-6];
        let decision = opm_accepted_step_growth_decision(1.0, 0, tiny_errors, 0);
        assert!((decision.factor - OPM_SOLVER_MAX_GROWTH).abs() < 1e-9);
        assert_eq!(decision.limiter, "opm-max-growth");

        // A high iteration count should bind via the iteration-count formula instead.
        let decision = opm_accepted_step_growth_decision(1.0, 16, tiny_errors, 0);
        assert!((decision.factor - 0.5).abs() < 1e-9);
        assert_eq!(decision.limiter, "opm-iter");
    }

    #[test]
    fn opm_accepted_step_growth_decision_tightens_after_a_retry() {
        // Very clean history + low iteration count would otherwise propose max growth
        // (3.0), but any retry this substep clamps further to solver-growth-factor (2.0).
        let tiny_errors = [1e-6, 1e-6, 1e-6];
        let decision = opm_accepted_step_growth_decision(1.0, 0, tiny_errors, 1);
        assert!((decision.factor - OPM_SOLVER_GROWTH_FACTOR_AFTER_RESTART).abs() < 1e-9);
        assert_eq!(decision.limiter, "opm-restart-growth");
    }

    #[test]
    fn opm_time_step_control_tolerance_matches_opm_default() {
        assert!((OPM_TIME_STEP_CONTROL_TOLERANCE - 0.1).abs() < 1e-12);
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
        assert_eq!(
            extrapolated.cells[1].regime,
            HydrocarbonState::Undersaturated
        );
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

#[cfg(test)]
mod phase5_repro {
    use crate::ReservoirSimulator;
    use std::time::Instant;

    /// Native reproduction of the wasm diagnostic's `water-pressure --grid
    /// 12x12x3 --steps 1 --dt 1` case (see `scripts/fim-wasm-diagnostic.mjs`
    /// `configureCommonTwoPhase` + `waterWellConfig` (pressure control) +
    /// `setWells`).
    ///
    /// History: this documented the Phase 5 AD-cutover dt-floor collapse; that
    /// two-phase Jacobian singularity was fixed in `ffd965a` and the case now
    /// completes (31 substeps as of the Phase 7 baseline). Kept `#[ignore]`d
    /// because it is still slow in debug builds, and doubles as the capture
    /// driver for the offline solver lab: run manually with
    /// `FIM_CAPTURE_DIR=<dir> cargo test --release ... repro_water_pressure_12x12x3 -- --ignored`
    /// to dump every failed iterative linear system for offline analysis
    /// (`fim/linear/capture.rs` / `fim/linear/solver_lab.rs`).
    #[test]
    #[ignore]
    fn repro_water_pressure_12x12x3() {
        run_water_pressure_capture_driver(12, 12, 3, 1.0);
    }

    /// Bundle N `docs/FIM_BUNDLE_N_DESIGN.md` §5 end-metric evaluation: the same heavy-case
    /// repro as `repro_water_pressure_12x12x3`, but with `OpmAligned` turned on. Native +
    /// `--release` so the run isn't gated by the wasm diagnostic runner's I/O-buffering/
    /// timeout issues that blocked live `--opm-aligned` measurement on this case at
    /// checkpoints 3-6. Run with:
    /// `cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib repro_water_pressure_12x12x3_opm_aligned -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn repro_water_pressure_12x12x3_opm_aligned() {
        run_water_pressure_capture_driver_with_flavor(12, 12, 3, 1.0, true);
    }

    /// Bundle N §5 follow-up: isolate genuine solver cost from trace overhead. The
    /// `_opm_aligned` repro above used `step_with_diagnostics` (`capture_fim_trace=true`),
    /// which stores every `fim_trace!` line into a growing `String` across all 18,002
    /// substeps it took (176 real minutes). `fim_trace!`'s `format!()` call — and any
    /// function calls embedded in its arguments (e.g. `residual_family_detail_trace`) — run
    /// unconditionally regardless of `capture_fim_trace`, so this does NOT isolate all
    /// per-iteration overhead, only the string-storage/growth cost. Uses the production
    /// `step()` path (`capture_fim_trace=false`) and reads the compact `FimStepStats` summary
    /// instead of the full text trace. Run with:
    /// `cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture`
    ///
    /// Late-window trace diagnostic (`docs/FIM_BUNDLE_N_DESIGN.md` §10, `fim::trace_sink`):
    /// this same driver is also the vehicle for both diagnostic passes, still on the
    /// production `step()` path — no code path change, only env vars:
    ///
    /// ```text
    /// # Re-baseline + per-substep ledger (background; uncapped)
    /// FIM_TRACE_FILE=/path/to/ledger.log \
    /// cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    ///   repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture
    ///
    /// # Windowed deep-trace rerun once the ledger shows where dt collapses (cheap, capped)
    /// FIM_TRACE_FILE=/path/to/window.log FIM_TRACE_DT_BELOW=1e-3 \
    /// FIM_MAX_SUBSTEPS=<onset_substep + ~500> \
    /// cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    ///   repro_water_pressure_12x12x3_opm_aligned_no_trace -- --ignored --nocapture
    /// ```
    ///
    /// `FIM_TRACE_FILE` alone gets one `LEDGER` line per accepted substep/retry (BHP,
    /// perforation rates, iters, growth) for the whole run. Adding `FIM_TRACE_DT_BELOW`
    /// (days) additionally activates full per-iteration tracing — every existing `fim_trace!`
    /// line plus `WELLTRACE` (per-iteration well/perforation state) — once a trial dt drops
    /// below the threshold; `FIM_TRACE_SUBSTEP_START=<n>` is the substep-indexed alternative
    /// once the onset substep is already known. `FIM_MAX_SUBSTEPS` overrides the hardcoded
    /// 100,000 cap so a windowed rerun can abort shortly after the window instead of running
    /// to completion. All four are no-ops when unset (see `fim::trace_sink`).
    ///
    /// Bundle W (`docs/FIM_BUNDLE_W_PLAN.md` W4): `FIM_NESTED_WELL_SOLVE=1` additionally
    /// enables the converged per-well inner Newton solve
    /// (`setFimNestedWellSolve`/`nested_well_solve`), independent of the trace env vars above —
    /// this is the §5 re-run vehicle for the mechanism this bundle targets, layered on the same
    /// `FIM-DIAG-002` re-baseline driver. No-op (Legacy relax path) when unset.
    #[test]
    #[ignore]
    fn repro_water_pressure_12x12x3_opm_aligned_no_trace() {
        let (nx, ny, nz, dt_days) = (12usize, 12usize, 3usize, 1.0_f64);
        let mut sim = ReservoirSimulator::new(nx, ny, nz, 0.2);
        sim.set_fim_enabled(true);
        sim.set_cell_dimensions(10.0, 10.0, 1.0).unwrap();
        sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
            .unwrap();
        sim.set_initial_pressure(300.0);
        sim.set_initial_saturation(0.1);
        sim.set_fluid_properties(1.0, 0.5).unwrap();
        sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
        sim.set_rock_properties(1e-6, 0.0, 1.0, 1.0).unwrap();
        sim.set_fluid_densities(800.0, 1000.0).unwrap();
        sim.set_capillary_params(0.0, 2.0).unwrap();
        sim.set_gravity_enabled(false);
        sim.set_permeability_per_layer(vec![2000.0; nz], vec![2000.0; nz], vec![200.0; nz])
            .unwrap();
        sim.set_stability_params(0.05, 75.0, 0.75);
        sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
        sim.set_target_well_rates(0.0, 0.0).unwrap();
        sim.set_well_bhp_limits(100.0, 500.0).unwrap();
        sim.set_rate_controlled_wells(false);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(nx - 1, ny - 1, 0, 100.0, 0.1, 0.0, false)
            .unwrap();
        sim.set_fim_opm_aligned_nonlinear(true);
        sim.set_fim_true_fgmres(std::env::var_os("FIM_TRUE_FGMRES").is_some());
        // Bundle W (`docs/FIM_BUNDLE_W_PLAN.md` W4): env-gated so this same driver, already the
        // FIM-DIAG-002 re-baseline vehicle, is also the §5 re-run vehicle — no code path change.
        let nested_well_solve = std::env::var_os("FIM_NESTED_WELL_SOLVE").is_some();
        sim.set_fim_nested_well_solve(nested_well_solve);
        // FIM-DIAG-003 D0/D1 (`docs/FIM_DIAG_003_PLAN.md`): same env-gated pattern, forces
        // every Newton linear solve through the exact direct backend for H1/H2 discrimination.
        let force_direct_linear = std::env::var_os("FIM_FORCE_DIRECT_LINEAR").is_some();
        sim.set_fim_force_direct_linear(force_direct_linear);

        let start = Instant::now();
        sim.step(dt_days);
        let elapsed = start.elapsed();
        println!(
            "native step (no trace) elapsed: {:.3}s (nested_well_solve={nested_well_solve} force_direct_linear={force_direct_linear})",
            elapsed.as_secs_f64()
        );
        if let Some(stats) = sim.last_fim_step_stats_ref() {
            println!(
                "accepted_substeps={} advanced_dt={:.6}/{:.6} linear_bad={} nonlinear_bad={} mixed={} solver_ms={:?} min_dt={:?} max_dt={:?} last_dt={:?}",
                stats.accepted_substeps,
                stats.advanced_dt_days,
                stats.target_dt_days,
                stats.linear_bad_retries,
                stats.nonlinear_bad_retries,
                stats.mixed_retries,
                stats.solver_ms,
                stats.min_accepted_dt_days,
                stats.max_accepted_dt_days,
                stats.last_accepted_dt_days,
            );
        } else {
            println!("no FimStepStats recorded");
        }
    }

    #[test]
    #[ignore]
    fn repro_water_pressure_23x23x1_opm_aligned() {
        run_water_pressure_capture_driver_with_flavor(23, 23, 1, 0.25, true);
    }

    /// Sibling capture driver mirroring the wasm diagnostic's bounded
    /// `water-pressure --grid 23x23x1 --steps 1 --dt 0.25` control case —
    /// the second case the offline solver lab compares against (the
    /// CPR-improvement doc's over-threshold probe).
    #[test]
    #[ignore]
    fn repro_water_pressure_23x23x1() {
        run_water_pressure_capture_driver(23, 23, 1, 0.25);
    }

    /// Shared body for the native water-pressure repro/capture drivers. Exact
    /// mirror of `configureCommonTwoPhase` + `waterWellConfig` (pressure
    /// control) in `scripts/fim-wasm-diagnostic.mjs`.
    fn run_water_pressure_capture_driver(nx: usize, ny: usize, nz: usize, dt_days: f64) {
        run_water_pressure_capture_driver_with_flavor(nx, ny, nz, dt_days, false);
    }

    /// Y2c native bounded-control entry point. Unlike the wasm matrix, this can exercise the
    /// native/default-off primary-variable lifecycle flag on the exact same water fixtures.
    /// `FIM_Y2C_WATER_GRID` accepts `20x20x3`, `22x22x1`, or `23x23x1`; `FIM_Y2C_FLAVOR`
    /// accepts `legacy` or `opm`.
    #[test]
    #[ignore]
    fn repro_water_pressure_y2c_control() {
        let grid = std::env::var("FIM_Y2C_WATER_GRID").unwrap_or_else(|_| "20x20x3".to_string());
        let (nx, ny, nz) = match grid.as_str() {
            "20x20x3" => (20, 20, 3),
            "22x22x1" => (22, 22, 1),
            "23x23x1" => (23, 23, 1),
            _ => panic!("FIM_Y2C_WATER_GRID must be 20x20x3|22x22x1|23x23x1"),
        };
        let flavor = std::env::var("FIM_Y2C_FLAVOR").unwrap_or_else(|_| "opm".to_string());
        assert!(
            matches!(flavor.as_str(), "legacy" | "opm"),
            "FIM_Y2C_FLAVOR must be legacy|opm"
        );
        println!("Y2C water control grid={grid} flavor={flavor}");
        run_water_pressure_capture_driver_with_flavor(nx, ny, nz, 0.25, flavor == "opm");
    }

    fn run_water_pressure_capture_driver_with_flavor(
        nx: usize,
        ny: usize,
        nz: usize,
        dt_days: f64,
        opm_aligned: bool,
    ) {
        let mut sim = ReservoirSimulator::new(nx, ny, nz, 0.2);
        sim.set_fim_enabled(true);
        sim.set_cell_dimensions(10.0, 10.0, 1.0).unwrap();
        sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
            .unwrap();
        sim.set_initial_pressure(300.0);
        sim.set_initial_saturation(0.1);
        sim.set_fluid_properties(1.0, 0.5).unwrap();
        sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
        sim.set_rock_properties(1e-6, 0.0, 1.0, 1.0).unwrap();
        sim.set_fluid_densities(800.0, 1000.0).unwrap();
        sim.set_capillary_params(0.0, 2.0).unwrap();
        sim.set_gravity_enabled(false);
        sim.set_permeability_per_layer(vec![2000.0; nz], vec![2000.0; nz], vec![200.0; nz])
            .unwrap();
        sim.set_stability_params(0.05, 75.0, 0.75);

        // waterWellConfig(pressure control)
        sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
        sim.set_target_well_rates(0.0, 0.0).unwrap();
        sim.set_well_bhp_limits(100.0, 500.0).unwrap();
        sim.set_rate_controlled_wells(false);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(nx - 1, ny - 1, 0, 100.0, 0.1, 0.0, false)
            .unwrap();
        sim.set_fim_opm_aligned_nonlinear(opm_aligned);
        sim.set_fim_true_fgmres(std::env::var_os("FIM_TRUE_FGMRES").is_some());

        let start = Instant::now();
        let trace = sim.step_with_diagnostics(dt_days);
        let elapsed = start.elapsed();
        println!(
            "native step elapsed: {:.3}s (opm_aligned={})",
            elapsed.as_secs_f64(),
            opm_aligned
        );
        println!("trace tail (last 4000 chars):");
        let tail_start = trace.len().saturating_sub(4000);
        println!("{}", &trace[tail_start..]);
        if let Some(stats) = sim.last_fim_step_stats_ref() {
            let newton_iterations: Vec<usize> = stats
                .accepted_rungs
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|rung| rung.newton_iterations)
                .collect();
            println!(
                "Y2C water result grid={}x{}x{} flavor={} accepted_substeps={} advanced_dt={:.6}/{:.6} newton={:?} linear_bad={} nonlinear_bad={} mixed={} min_dt={:?} max_dt={:?}",
                nx,
                ny,
                nz,
                if opm_aligned { "opm" } else { "legacy" },
                stats.accepted_substeps,
                stats.advanced_dt_days,
                stats.target_dt_days,
                newton_iterations,
                stats.linear_bad_retries,
                stats.nonlinear_bad_retries,
                stats.mixed_retries,
                stats.min_accepted_dt_days,
                stats.max_accepted_dt_days,
            );
        }
    }

    /// Native mirror of the wasm diagnostic's `gas-rate --grid 20x20x3 --steps 1 --dt 0.25
    /// --opm-aligned` case (`scripts/fim-wasm-diagnostic.mjs` `configureGasBase` +
    /// `gasWellConfig` rate control + `setWells`, both-wells layout). Built for `FIM_BUNDLE_Y`
    /// Y0 (`docs/FIM_OPM_PARITY_PLAN.md`): the wasm runner cannot host `fim::trace_sink`'s
    /// env-gated file trace (every call site is `#[cfg(not(target_arch = "wasm32"))]`), so the
    /// 459-substep gas-rate catastrophe needed this native equivalent before it could be
    /// windowed-traced the same way the heavy water case was in `FIM-DIAG-002`/`003`. Run with:
    /// `FIM_TRACE_FILE=<path> FIM_TRACE_DT_BELOW=<days> FIM_MAX_SUBSTEPS=<n> \
    ///  cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
    ///  repro_gas_rate_20x20x3_opm_aligned_no_trace -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn repro_gas_rate_20x20x3_opm_aligned_no_trace() {
        use crate::pvt::{PvtRow, PvtTable};

        let (nx, ny, nz, dt_days) = (20usize, 20usize, 3usize, 0.25_f64);
        let mut sim = ReservoirSimulator::new(nx, ny, nz, 0.2);

        // configureGasBase
        sim.set_fim_enabled(true);
        sim.set_cell_dimensions(10.0, 10.0, 5.0).unwrap();
        sim.set_initial_pressure(200.0);
        sim.set_initial_saturation(0.15);
        sim.set_initial_gas_saturation(0.0);
        sim.set_fluid_properties(0.8, 0.4).unwrap();
        sim.set_fluid_compressibilities(1e-4, 5e-5).unwrap();
        sim.set_rock_properties(4e-5, 2500.0, 1.2, 1.0).unwrap();
        sim.set_fluid_densities(850.0, 1020.0).unwrap();
        sim.set_gas_fluid_properties(0.02, 1e-4, 0.8).unwrap();
        sim.set_capillary_params(0.0, 2.0).unwrap();
        // gravity: null resolves to `nz > 1` in the wasm runner (`configureOptions`); nz=3 here.
        sim.set_gravity_enabled(true);
        sim.set_permeability_per_layer(vec![500.0; nz], vec![500.0; nz], vec![50.0; nz])
            .unwrap();
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 50.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.1,
                    mu_o_cp: 1.0,
                    bg_m3m3: 0.02,
                    mu_g_cp: 0.015,
                },
                PvtRow {
                    p_bar: 150.0,
                    rs_m3m3: 80.0,
                    bo_m3m3: 1.25,
                    mu_o_cp: 0.7,
                    bg_m3m3: 0.008,
                    mu_g_cp: 0.018,
                },
                PvtRow {
                    p_bar: 250.0,
                    rs_m3m3: 140.0,
                    bo_m3m3: 1.4,
                    mu_o_cp: 0.5,
                    bg_m3m3: 0.005,
                    mu_g_cp: 0.022,
                },
                PvtRow {
                    p_bar: 350.0,
                    rs_m3m3: 200.0,
                    bo_m3m3: 1.55,
                    mu_o_cp: 0.4,
                    bg_m3m3: 0.004,
                    mu_g_cp: 0.025,
                },
            ],
            sim.pvt.c_o,
        ));
        sim.set_initial_rs(80.0);
        sim.set_three_phase_rel_perm_props(
            0.15, 0.15, 0.05, 0.05, 0.2, 2.0, 2.0, 2.0, 1e-5, 1.0, 0.95,
        )
        .unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_injected_fluid("gas").unwrap();
        sim.set_gas_redissolution_enabled(false);
        sim.set_stability_params(0.05, 75.0, 0.75);

        // gasWellConfig(rate) + setWells (both wells)
        sim.set_well_control_modes("rate".to_string(), "rate".to_string());
        sim.set_target_well_rates(500.0, 200.0).unwrap();
        sim.set_well_bhp_limits(50.0, 400.0).unwrap();
        sim.set_rate_controlled_wells(true);
        sim.add_well(0, 0, 0, 350.0, 0.1, 0.0, true).unwrap();
        sim.add_well(nx - 1, ny - 1, 0, 100.0, 0.1, 0.0, false)
            .unwrap();
        sim.set_fim_opm_aligned_nonlinear(true);
        sim.set_fim_true_fgmres(std::env::var_os("FIM_TRUE_FGMRES").is_some());
        let nested_well_solve = std::env::var_os("FIM_NESTED_WELL_SOLVE").is_some();
        sim.set_fim_nested_well_solve(nested_well_solve);
        let force_direct_linear = std::env::var_os("FIM_FORCE_DIRECT_LINEAR").is_some();
        sim.set_fim_force_direct_linear(force_direct_linear);

        let start = Instant::now();
        sim.step(dt_days);
        let elapsed = start.elapsed();
        println!(
            "native gas-rate step (no trace) elapsed: {:.3}s (nested_well_solve={nested_well_solve} force_direct_linear={force_direct_linear})",
            elapsed.as_secs_f64()
        );
        if let Some(stats) = sim.last_fim_step_stats_ref() {
            println!(
                "accepted_substeps={} advanced_dt={:.6}/{:.6} linear_bad={} nonlinear_bad={} mixed={} solver_ms={:?} min_dt={:?} max_dt={:?} last_dt={:?}",
                stats.accepted_substeps,
                stats.advanced_dt_days,
                stats.target_dt_days,
                stats.linear_bad_retries,
                stats.nonlinear_bad_retries,
                stats.mixed_retries,
                stats.solver_ms,
                stats.min_accepted_dt_days,
                stats.max_accepted_dt_days,
                stats.last_accepted_dt_days,
            );
        } else {
            println!("no FimStepStats recorded");
        }
    }

    /// Exact native mirror of the matching ResSim side of the tracked Flow oracle
    /// `gas-rate-10x10x3` (`opm/reference-decks/gas-rate-10x10x3/manifest.json`).
    ///
    /// `FIM_BUNDLE_Y` Y1j uses this as a *diagnostic matrix*, not a solver-behavior test:
    /// it isolates whether the first injector-adjacent ineffective Newton update remains when
    /// the live iterative backend is replaced by the exact direct backend, and when individual
    /// wells or rate control are removed. The default is the exact wasm diagnostic mapping:
    /// both wells, rate control, OPM-aligned nonlinear flavor, one 0.25-day report step.
    ///
    /// Environment selectors:
    /// - `FIM_Y1J_WELLS=both|injector|producer|none` (default `both`)
    /// - `FIM_Y1J_CONTROL=rate|pressure` (default `rate`)
    /// - `FIM_Y1J_DT_DAYS=<days>` overrides the default `0.25` report-step target. Y2b3c uses
    ///   exactly `0.00898425` to regenerate the historical iteration-1 decision system even when
    ///   the completed lifecycle no longer cuts down to that rung.
    /// - `FIM_Y1J_STEPS=<count>` runs sequential report steps on the same simulator (default 1),
    ///   preserving cross-step controller state for Y2c's six-step and fine-dt promotion gates.
    /// - `FIM_Y1J_GRID=10|20` selects the square lateral grid size (default 10).
    /// - `FIM_Y1J_FLAVOR=legacy|opm` selects the nonlinear flavor (default `opm`).
    /// - `FIM_FORCE_DIRECT_LINEAR=1` selects the direct backend; unset uses the live stack.
    /// - `FIM_MAX_SUBSTEPS=1` caps after the first accepted rung; use this for the bounded
    ///   first-rung comparison, not as a completed 0.25-day-step result.
    /// - `FIM_TRACE_FILE=<path> FIM_TRACE_DT_BELOW=1` records every iteration's `WELLTRACE`
    ///   and `WELLJAC` lines for this first report step.
    /// - `FIM_Y2A_AUDIT=1` additionally records the test-only injector `Y2A` audit at each
    ///   three-count stagnation point: AD and legacy Jacobian entries for the perforation and
    ///   connected-cell rows, plus central and one-sided differences of the legacy residual.
    ///   This does not alter a Newton iterate or any convergence decision.
    ///
    /// Example:
    /// `FIM_Y1J_WELLS=injector FIM_FORCE_DIRECT_LINEAR=1 FIM_TRACE_FILE=/tmp/y1j.log
    ///  FIM_TRACE_DT_BELOW=1 cargo test --release --manifest-path src/lib/ressim/Cargo.toml
    ///  --lib repro_gas_rate_10x10x3_y1j -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn repro_gas_rate_10x10x3_y1j() {
        use crate::pvt::{PvtRow, PvtTable};

        let wells = std::env::var("FIM_Y1J_WELLS").unwrap_or_else(|_| "both".to_string());
        let control = std::env::var("FIM_Y1J_CONTROL").unwrap_or_else(|_| "rate".to_string());
        assert!(
            matches!(wells.as_str(), "both" | "injector" | "producer" | "none"),
            "FIM_Y1J_WELLS must be both|injector|producer|none, got {wells:?}"
        );
        assert!(
            matches!(control.as_str(), "rate" | "pressure"),
            "FIM_Y1J_CONTROL must be rate|pressure, got {control:?}"
        );

        let lateral_grid = std::env::var("FIM_Y1J_GRID")
            .map(|value| {
                value
                    .parse::<usize>()
                    .expect("FIM_Y1J_GRID must be 10 or 20")
            })
            .unwrap_or(10);
        assert!(matches!(lateral_grid, 10 | 20));
        let (nx, ny, nz) = (lateral_grid, lateral_grid, 3usize);
        let flavor = std::env::var("FIM_Y1J_FLAVOR").unwrap_or_else(|_| "opm".to_string());
        assert!(matches!(flavor.as_str(), "legacy" | "opm"));
        let dt_days = std::env::var("FIM_Y1J_DT_DAYS")
            .map(|value| {
                value
                    .parse::<f64>()
                    .expect("FIM_Y1J_DT_DAYS must be a finite positive number")
            })
            .unwrap_or(0.25);
        assert!(
            dt_days.is_finite() && dt_days > 0.0,
            "FIM_Y1J_DT_DAYS must be a finite positive number, got {dt_days}"
        );
        let step_count = std::env::var("FIM_Y1J_STEPS")
            .map(|value| {
                value
                    .parse::<usize>()
                    .expect("FIM_Y1J_STEPS must be a positive integer")
            })
            .unwrap_or(1);
        assert!(step_count > 0, "FIM_Y1J_STEPS must be positive");
        let mut sim = ReservoirSimulator::new(nx, ny, nz, 0.2);

        // Exact `configureGasBase` mapping from `fim-wasm-diagnostic.mjs`. `capillary=true`
        // and `gravity=null` resolve to these values for the 10x10x3 gas-rate preset.
        sim.set_fim_enabled(true);
        sim.set_cell_dimensions(10.0, 10.0, 5.0).unwrap();
        sim.set_initial_pressure(200.0);
        sim.set_initial_saturation(0.15);
        sim.set_initial_gas_saturation(0.0);
        sim.set_fluid_properties(0.8, 0.4).unwrap();
        sim.set_fluid_compressibilities(1e-4, 5e-5).unwrap();
        sim.set_rock_properties(4e-5, 2500.0, 1.2, 1.0).unwrap();
        sim.set_fluid_densities(850.0, 1020.0).unwrap();
        sim.set_gas_fluid_properties(0.02, 1e-4, 0.8).unwrap();
        sim.set_capillary_params(0.0, 2.0).unwrap();
        sim.set_gravity_enabled(true);
        sim.set_permeability_per_layer(vec![500.0; nz], vec![500.0; nz], vec![50.0; nz])
            .unwrap();
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 50.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.1,
                    mu_o_cp: 1.0,
                    bg_m3m3: 0.02,
                    mu_g_cp: 0.015,
                },
                PvtRow {
                    p_bar: 150.0,
                    rs_m3m3: 80.0,
                    bo_m3m3: 1.25,
                    mu_o_cp: 0.7,
                    bg_m3m3: 0.008,
                    mu_g_cp: 0.018,
                },
                PvtRow {
                    p_bar: 250.0,
                    rs_m3m3: 140.0,
                    bo_m3m3: 1.4,
                    mu_o_cp: 0.5,
                    bg_m3m3: 0.005,
                    mu_g_cp: 0.022,
                },
                PvtRow {
                    p_bar: 350.0,
                    rs_m3m3: 200.0,
                    bo_m3m3: 1.55,
                    mu_o_cp: 0.4,
                    bg_m3m3: 0.004,
                    mu_g_cp: 0.025,
                },
            ],
            sim.pvt.c_o,
        ));
        sim.set_initial_rs(80.0);
        sim.set_three_phase_rel_perm_props(
            0.15, 0.15, 0.05, 0.05, 0.2, 2.0, 2.0, 2.0, 1e-5, 1.0, 0.95,
        )
        .unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_injected_fluid("gas").unwrap();
        sim.set_gas_redissolution_enabled(false);
        sim.set_stability_params(0.05, 75.0, 0.75);

        let rate_controlled = control == "rate";
        let (injector_bhp, producer_bhp) = (350.0, 100.0);
        sim.set_well_control_modes(control.clone(), control.clone());
        sim.set_target_well_rates(
            if rate_controlled { 500.0 } else { 0.0 },
            if rate_controlled { 200.0 } else { 0.0 },
        )
        .unwrap();
        sim.set_well_bhp_limits(50.0, 400.0).unwrap();
        sim.set_rate_controlled_wells(rate_controlled);
        if matches!(wells.as_str(), "both" | "injector") {
            sim.add_well(0, 0, 0, injector_bhp, 0.1, 0.0, true).unwrap();
        }
        if matches!(wells.as_str(), "both" | "producer") {
            sim.add_well(nx - 1, ny - 1, 0, producer_bhp, 0.1, 0.0, false)
                .unwrap();
        }
        sim.set_fim_opm_aligned_nonlinear(flavor == "opm");
        sim.set_fim_true_fgmres(std::env::var_os("FIM_TRUE_FGMRES").is_some());
        sim.set_fim_force_direct_linear(std::env::var_os("FIM_FORCE_DIRECT_LINEAR").is_some());

        let start = Instant::now();
        let force_direct_linear = std::env::var_os("FIM_FORCE_DIRECT_LINEAR").is_some();
        println!(
            "Y1J config grid={nx}x{ny}x{nz} dt={dt_days} steps={step_count} flavor={flavor} wells={wells} control={control} force_direct_linear={force_direct_linear}"
        );

        for step_idx in 0..step_count {
            sim.step(dt_days);
            let stats = sim
                .last_fim_step_stats_ref()
                .expect("Y1J native repro did not record FimStepStats");
            let newton_iterations: Vec<usize> = stats
                .accepted_rungs
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|rung| rung.newton_iterations)
                .collect();
            let rate = sim
                .rate_history
                .last()
                .expect("accepted FIM report step must record rates");
            let mut min_sw = f64::INFINITY;
            let mut max_sw = f64::NEG_INFINITY;
            let mut min_so = f64::INFINITY;
            let mut max_so = f64::NEG_INFINITY;
            let mut min_sg = f64::INFINITY;
            let mut max_sg = f64::NEG_INFINITY;
            let mut max_saturation_closure = 0.0_f64;
            for cell_idx in 0..sim.nx * sim.ny * sim.nz {
                let sw = sim.sat_water[cell_idx];
                let so = sim.sat_oil[cell_idx];
                let sg = sim.sat_gas[cell_idx];
                assert!(
                    sim.pressure[cell_idx].is_finite()
                        && sw.is_finite()
                        && so.is_finite()
                        && sg.is_finite()
                        && sim.rs[cell_idx].is_finite(),
                    "non-finite accepted state at step {} cell {}",
                    step_idx + 1,
                    cell_idx
                );
                min_sw = min_sw.min(sw);
                max_sw = max_sw.max(sw);
                min_so = min_so.min(so);
                max_so = max_so.max(so);
                min_sg = min_sg.min(sg);
                max_sg = max_sg.max(sg);
                max_saturation_closure = max_saturation_closure.max((sw + so + sg - 1.0).abs());
            }
            assert!(
                max_saturation_closure < 1e-10,
                "accepted saturation closure drift at step {}: {}",
                step_idx + 1,
                max_saturation_closure
            );
            assert!(
                rate.total_production_oil.is_finite()
                    && rate.total_production_gas.is_finite()
                    && rate.total_injection.is_finite()
                    && rate.material_balance_error_m3.is_finite()
                    && rate.material_balance_error_oil_m3.is_finite()
                    && rate.material_balance_error_gas_m3.is_finite(),
                "non-finite reporting at step {}",
                step_idx + 1
            );
            println!(
                "Y1J step={} accepted_substeps={} advanced_dt={:.6}/{:.6} newton={:?} linear_bad={} nonlinear_bad={} mixed={} min_dt={:?} max_dt={:?} last_dt={:?} state={{sw=[{:.9e},{:.9e}],so=[{:.9e},{:.9e}],sg=[{:.9e},{:.9e}],closure={:.3e},water_inv={:.9e},oil_inv={:.9e},gas_inv={:.9e}}} report={{oil_rate={:.9e},gas_rate={:.9e},injection={:.9e},mb_water={:.9e},mb_oil={:.9e},mb_gas={:.9e}}}",
                step_idx + 1,
                stats.accepted_substeps,
                stats.advanced_dt_days,
                stats.target_dt_days,
                newton_iterations,
                stats.linear_bad_retries,
                stats.nonlinear_bad_retries,
                stats.mixed_retries,
                stats.min_accepted_dt_days,
                stats.max_accepted_dt_days,
                stats.last_accepted_dt_days,
                min_sw,
                max_sw,
                min_so,
                max_so,
                min_sg,
                max_sg,
                max_saturation_closure,
                sim.total_water_inventory_m3(),
                sim.total_oil_inventory_sc(),
                sim.total_gas_inventory_sc(),
                rate.total_production_oil,
                rate.total_production_gas,
                rate.total_injection,
                rate.material_balance_error_m3,
                rate.material_balance_error_oil_m3,
                rate.material_balance_error_gas_m3,
            );
        }
        println!("Y1J elapsed_s={:.3}", start.elapsed().as_secs_f64());
    }
}
