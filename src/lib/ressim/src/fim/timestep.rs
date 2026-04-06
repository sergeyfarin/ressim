use crate::ReservoirSimulator;
use crate::fim::linear::{FimLinearSolveReport, active_direct_solve_row_threshold};
use crate::fim::newton::FimRetryFailureClass;
use crate::fim::newton::{FimNewtonOptions, FimRetryFailureDiagnostics, run_fim_timestep};
use crate::fim::state::{FimState, HydrocarbonState};
use crate::reporting::FimStepStats;

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
struct FimGrowthCooldown {
    cap_dt_days: Option<f64>,
    clean_successes_remaining: u32,
    hotspot: Option<FimRetryHotspot>,
    hotspot_repeat_failures: u32,
    hotspot_clean_successes_without_retry: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FimRetryHotspot {
    dominant_family_label: &'static str,
    dominant_row: usize,
    dominant_item_index: usize,
    scope: FimRetryHotspotScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FimRetryHotspotScope {
    Cell(usize),
    Well(usize),
    Perforation(usize),
    Exact { row: usize, item_index: usize },
}

impl FimRetryHotspot {
    fn from_failure(failure_diagnostics: &FimRetryFailureDiagnostics) -> Option<Self> {
        match failure_diagnostics.class {
            FimRetryFailureClass::LinearBad => None,
            FimRetryFailureClass::NonlinearBad | FimRetryFailureClass::Mixed => Some(Self {
                dominant_family_label: failure_diagnostics.dominant_family_label,
                dominant_row: failure_diagnostics.dominant_row,
                dominant_item_index: failure_diagnostics.dominant_item_index,
                scope: FimRetryHotspotScope::from_failure(failure_diagnostics),
            }),
        }
    }

    fn same_site(self, other: Self) -> bool {
        self.scope == other.scope
    }
}

impl FimRetryHotspotScope {
    fn from_failure(failure_diagnostics: &FimRetryFailureDiagnostics) -> Self {
        match failure_diagnostics.dominant_family_label {
            "water" | "oil" | "gas" => Self::Cell(failure_diagnostics.dominant_item_index),
            "well" => Self::Well(failure_diagnostics.dominant_item_index),
            "perf" => Self::Perforation(failure_diagnostics.dominant_item_index),
            _ => Self::Exact {
                row: failure_diagnostics.dominant_row,
                item_index: failure_diagnostics.dominant_item_index,
            },
        }
    }
}

impl FimGrowthCooldown {
    const HOTSPOT_MEMORY_CLEAR_CLEAN_SUCCESSES: u32 = 2;

    fn extra_clean_successes_for_repeated_hotspot(self) -> u32 {
        match self.hotspot_repeat_failures {
            0 | 1 => 0,
            2 | 3 => 1,
            _ => 2,
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

    fn note_retry_failure(&mut self, failure_diagnostics: &FimRetryFailureDiagnostics) {
        let Some(hotspot) = FimRetryHotspot::from_failure(failure_diagnostics) else {
            self.clear_hotspot_memory();
            return;
        };

        if self.hotspot.is_some_and(|previous| previous.same_site(hotspot)) {
            self.hotspot_repeat_failures = self.hotspot_repeat_failures.saturating_add(1);
        } else {
            self.hotspot_repeat_failures = 1;
        }
        self.hotspot = Some(hotspot);
        self.hotspot_clean_successes_without_retry = 0;
    }

    fn cap_growth_decision(self, decision: AcceptedStepGrowthDecision) -> AcceptedStepGrowthDecision {
        if decision.limiter != "max-growth" {
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

    fn stabilize_growth_decision(self, decision: AcceptedStepGrowthDecision) -> AcceptedStepGrowthDecision {
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

    fn note_clean_accepted(&mut self) {
        if self.clean_successes_remaining > 0 {
            self.clean_successes_remaining -= 1;
            if self.clean_successes_remaining == 0 {
                self.cap_dt_days = None;
            }
            return;
        }

        if self.hotspot.is_some() {
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
                            " hotspot={} row={} item={} repeats={} clear_left={}",
                            hotspot.dominant_family_label,
                            hotspot.dominant_row,
                            hotspot.dominant_item_index,
                            self.hotspot_repeat_failures,
                            Self::HOTSPOT_MEMORY_CLEAR_CLEAN_SUCCESSES
                                .saturating_sub(self.hotspot_clean_successes_without_retry),
                        )
                    })
                    .unwrap_or_default();
                format!(
                    " cooldown_cap={:.6} clean_left={}{}",
                    cap_dt_days,
                    self.clean_successes_remaining,
                    hotspot_trace
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
    const MIN_GROWTH: f64 = 0.75;
    const TARGET_NEWTON_ITERS: f64 = 8.0;
    const TARGET_MAX_SAT_CHANGE: f64 = 0.2;
    const TARGET_MAX_PRESSURE_CHANGE_BAR: f64 = 200.0;

    let iteration_growth =
        (TARGET_NEWTON_ITERS / newton_iterations as f64).clamp(MIN_GROWTH, MAX_GROWTH);
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

    decision.factor = decision.factor.clamp(MIN_GROWTH, MAX_GROWTH);
    if (decision.factor - MAX_GROWTH).abs() < 1e-12 {
        decision.limiter = "max-growth";
    }
    decision
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
    linear_bad_retries: usize,
    nonlinear_bad_retries: usize,
    mixed_retries: usize,
    min_accepted_dt_days: Option<f64>,
    max_accepted_dt_days: Option<f64>,
    last_accepted_dt_days: Option<f64>,
    last_growth_factor: f64,
    growth_limiter: Option<&str>,
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
        linear_bad_retries,
        nonlinear_bad_retries,
        mixed_retries,
        min_accepted_dt_days,
        max_accepted_dt_days,
        last_accepted_dt_days,
        last_growth_factor: last_accepted_dt_days.map(|_| last_growth_factor),
        growth_limiter: growth_limiter.map(str::to_string),
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
        const RETRY_GROWTH_COOLDOWN_CLEAN_SUCCESSES: u32 = 4;
        let mut substeps = 0;
        let mut linear_bad_retries = 0usize;
        let mut nonlinear_bad_retries = 0usize;
        let mut mixed_retries = 0usize;
        let mut min_accepted_dt_days: Option<f64> = None;
        let mut max_accepted_dt_days: Option<f64> = None;
        let mut last_accepted_dt_days: Option<f64> = None;
        let mut last_growth_limiter: Option<&'static str> = None;
        let mut last_retry_class: Option<&'static str> = None;
        let mut last_retry_dominant_family: Option<&'static str> = None;
        let mut last_retry_dominant_row: Option<usize> = None;
        self.last_solver_warning = String::new();
        let mut last_successful_dt = target_dt_days;
        let mut last_growth_factor = MAX_GROWTH;
        let mut growth_cooldown = FimGrowthCooldown::default();
        let mut solver_ms = 0.0;
        let mut accepted_solver_ms = 0.0;
        let mut retry_solver_ms = 0.0;
        let mut assembly_ms = 0.0;
        let mut property_eval_ms = 0.0;
        let mut linear_solve_ms = 0.0;
        let mut linear_preconditioner_ms = 0.0;
        let mut state_update_ms = 0.0;

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
            let proposed_trial = if substeps == 0 {
                remaining_dt
            } else {
                remaining_dt.min(last_successful_dt * last_growth_factor)
            };
            let initial_trial = growth_cooldown.clamp_trial_dt(proposed_trial, remaining_dt);
            let cooldown_trace = if initial_trial + 1e-12 < proposed_trial {
                format!(
                    " [cooldown-clamped from {:.6}{}]",
                    proposed_trial,
                    growth_cooldown.trace_suffix()
                )
            } else {
                growth_cooldown.trace_suffix()
            };
            let mut trial_dt = initial_trial;
            let mut retry_count = 0;
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

                    let growth_decision = growth_cooldown.stabilize_growth_decision(
                        growth_cooldown.cap_growth_decision(
                        accepted_step_growth_decision(
                        report.newton_iterations,
                        max_dsat,
                        max_dp,
                        ),
                    ));
                    let adjusted_growth_decision = if retry_count > 0 && growth_decision.factor < 1.0 {
                        AcceptedStepGrowthDecision {
                            factor: 1.0,
                            limiter: "retry-hold",
                        }
                    } else {
                        growth_decision
                    };
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
                        growth_cooldown.note_clean_accepted();
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
                    self.time_days += trial_dt;
                    time_stepped += trial_dt;
                    last_successful_dt = trial_dt;
                    substeps += 1;
                    break;
                }

                let next_dt = trial_dt * report.retry_factor.clamp(0.1, 0.5);
                retry_count += 1;

                if let Some(failure_diagnostics) = &report.failure_diagnostics {
                    growth_cooldown.note_retry_failure(failure_diagnostics);
                    last_retry_class = Some(failure_diagnostics.class.label());
                    last_retry_dominant_family = Some(failure_diagnostics.dominant_family_label);
                    last_retry_dominant_row = Some(failure_diagnostics.dominant_row);
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
                    report.retry_factor,
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
                        linear_bad_retries,
                        nonlinear_bad_retries,
                        mixed_retries,
                        min_accepted_dt_days,
                        max_accepted_dt_days,
                        last_accepted_dt_days,
                        last_growth_factor,
                        last_growth_limiter,
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
                        linear_bad_retries,
                        nonlinear_bad_retries,
                        mixed_retries,
                        min_accepted_dt_days,
                        max_accepted_dt_days,
                        last_accepted_dt_days,
                        last_growth_factor,
                        last_growth_limiter,
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
            linear_bad_retries,
            nonlinear_bad_retries,
            mixed_retries,
            min_accepted_dt_days,
            max_accepted_dt_days,
            last_accepted_dt_days,
            last_growth_factor,
            last_growth_limiter,
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
        AcceptedStepGrowthDecision, FimGrowthCooldown, accepted_step_growth_factor,
    };
    use crate::ReservoirSimulator;
    use crate::fim::newton::{FimRetryFailureClass, FimRetryFailureDiagnostics};

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
            linear_iterations: Some(12),
            used_linear_fallback: false,
            cpr_average_reduction_ratio: Some(1e-13),
            cpr_last_reduction_ratio: Some(1e-14),
        }
    }

    #[test]
    fn retry_acceptance_freezes_growth_until_clean_success_budget_is_spent() {
        let mut cooldown = FimGrowthCooldown::default();
        cooldown.note_retry_accepted(0.25, 4);

        assert_eq!(cooldown.clamp_trial_dt(0.3125, 1.0), 0.25);

        cooldown.note_clean_accepted();
        assert_eq!(cooldown.clamp_trial_dt(0.3125, 1.0), 0.25);

        cooldown.note_clean_accepted();
        assert_eq!(cooldown.clamp_trial_dt(0.3125, 1.0), 0.25);

        cooldown.note_clean_accepted();
        assert_eq!(cooldown.clamp_trial_dt(0.3125, 1.0), 0.25);

        cooldown.note_clean_accepted();
        assert_eq!(cooldown.clamp_trial_dt(0.3125, 1.0), 0.3125);
    }

    #[test]
    fn cooldown_clamp_never_exceeds_remaining_dt() {
        let mut cooldown = FimGrowthCooldown::default();
        cooldown.note_retry_accepted(0.25, 4);

        assert_eq!(cooldown.clamp_trial_dt(0.5, 0.1), 0.1);
    }

    #[test]
    fn repeated_same_hotspot_extends_growth_cooldown_budget() {
        let mut cooldown = FimGrowthCooldown::default();
        let failure = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 429, 143);

        cooldown.note_retry_failure(&failure);
        cooldown.note_retry_accepted(0.25, 4);
        assert_eq!(cooldown.clean_successes_remaining, 4);

        cooldown.note_retry_failure(&failure);
        cooldown.note_retry_accepted(0.25, 4);
        assert_eq!(cooldown.clean_successes_remaining, 5);
        assert!(cooldown.trace_suffix().contains("repeats=2"));
    }

    #[test]
    fn alternating_cell_row_families_count_as_same_hotspot_site() {
        let mut cooldown = FimGrowthCooldown::default();
        let water_failure = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 429, 143);
        let oil_failure = FimRetryFailureDiagnostics {
            dominant_family_label: "oil",
            dominant_row: 430,
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 430, 143)
        };

        cooldown.note_retry_failure(&water_failure);
        cooldown.note_retry_failure(&oil_failure);

        assert_eq!(cooldown.hotspot_repeat_failures, 2);
    }

    #[test]
    fn repeated_hotspot_caps_max_growth_decision() {
        let mut cooldown = FimGrowthCooldown::default();
        let failure = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 429, 143);

        cooldown.note_retry_failure(&failure);
        cooldown.note_retry_failure(&FimRetryFailureDiagnostics {
            dominant_family_label: "oil",
            dominant_row: 430,
            ..failure_diagnostics(FimRetryFailureClass::NonlinearBad, 430, 143)
        });

        let capped = cooldown.cap_growth_decision(AcceptedStepGrowthDecision {
            factor: 1.25,
            limiter: "max-growth",
        });

        assert_eq!(capped.factor, 1.0);
        assert_eq!(capped.limiter, "hotspot-repeat");
    }

    #[test]
    fn cooldown_holds_growth_flat_instead_of_shrinking() {
        let mut cooldown = FimGrowthCooldown::default();
        cooldown.note_retry_accepted(0.25, 4);

        let held = cooldown.stabilize_growth_decision(AcceptedStepGrowthDecision {
            factor: 0.75,
            limiter: "newton-iters",
        });

        assert_eq!(held.factor, 1.0);
        assert_eq!(held.limiter, "cooldown-hold");
    }

    #[test]
    fn changing_hotspot_resets_extra_growth_cooldown_budget() {
        let mut cooldown = FimGrowthCooldown::default();
        cooldown.note_retry_failure(&failure_diagnostics(
            FimRetryFailureClass::NonlinearBad,
            429,
            143,
        ));
        cooldown.note_retry_failure(&failure_diagnostics(
            FimRetryFailureClass::NonlinearBad,
            429,
            143,
        ));
        cooldown.note_retry_accepted(0.25, 4);
        assert_eq!(cooldown.clean_successes_remaining, 5);

        cooldown.note_retry_failure(&failure_diagnostics(
            FimRetryFailureClass::NonlinearBad,
            297,
            99,
        ));
        cooldown.note_retry_accepted(0.2, 4);
        assert_eq!(cooldown.clean_successes_remaining, 4);
        assert!(cooldown.trace_suffix().contains("row=297"));
    }

    #[test]
    fn linear_failure_does_not_seed_hotspot_memory() {
        let mut cooldown = FimGrowthCooldown::default();
        cooldown.note_retry_failure(&failure_diagnostics(FimRetryFailureClass::LinearBad, 10, 3));
        cooldown.note_retry_accepted(0.25, 4);

        assert_eq!(cooldown.clean_successes_remaining, 4);
        assert!(!cooldown.trace_suffix().contains("hotspot="));
    }

    #[test]
    fn hotspot_memory_persists_across_release_and_decays_after_clean_steps() {
        let mut cooldown = FimGrowthCooldown::default();
        let failure = failure_diagnostics(FimRetryFailureClass::NonlinearBad, 429, 143);

        cooldown.note_retry_failure(&failure);
        cooldown.note_retry_accepted(0.25, 4);
        cooldown.note_clean_accepted();
        cooldown.note_clean_accepted();
        cooldown.note_clean_accepted();
        cooldown.note_clean_accepted();

        assert_eq!(cooldown.hotspot_repeat_failures, 1);

        cooldown.note_retry_failure(&failure);
        cooldown.note_retry_accepted(0.25, 4);
        assert_eq!(cooldown.clean_successes_remaining, 5);

        cooldown.note_clean_accepted();
        cooldown.note_clean_accepted();
        cooldown.note_clean_accepted();
        cooldown.note_clean_accepted();
        cooldown.note_clean_accepted();
        cooldown.note_clean_accepted();
        cooldown.note_clean_accepted();

        assert!(cooldown.hotspot.is_none());
    }

    #[test]
    fn residual_margin_no_longer_throttles_growth_factor() {
        let growth = accepted_step_growth_factor(2, 0.003, 0.2);
        assert!((growth - 3.0).abs() < 1e-12);
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
