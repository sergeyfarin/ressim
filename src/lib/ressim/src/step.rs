use std::f64;

use crate::fim::newton::{run_fim_timestep, FimNewtonOptions};
use crate::fim::newton::FimRetryFailureClass;
use crate::fim::state::FimState;
use crate::ReservoirSimulator;

/// Diagnostic print macro — compiles to nothing on WASM, prints to stderr on native.
macro_rules! fim_trace {
    ($verbose:expr, $($arg:tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        if $verbose {
            eprintln!($($arg)*);
        }
    };
}

impl ReservoirSimulator {
    pub(crate) fn step_internal(&mut self, target_dt_days: f64) {
        if self.fim_enabled {
            self.step_internal_fim(target_dt_days);
            return;
        }

        let mut time_stepped = 0.0;
        const MAX_SUBSTEPS: u32 = 100_000;
        const MAX_PRESSURE_RETRIES_PER_SUBSTEP: u32 = 32;
        let mut substeps = 0;
        self.last_solver_warning = String::new();

        while time_stepped < target_dt_days && substeps < MAX_SUBSTEPS {
            let remaining_dt = target_dt_days - time_stepped;
            let mut trial_dt = remaining_dt;
            let mut retry_count = 0;
            let actual_dt;
            let final_p;
            let final_delta_water_m3;
            let final_delta_free_gas_sc;
            let final_delta_dg_sc;
            let final_well_controls;

            loop {
                self.update_dynamic_well_productivity_indices();

                let (
                    p_new,
                    delta_water_m3,
                    delta_free_gas_sc,
                    delta_dg_sc,
                    well_controls,
                    stable_dt_factor,
                    solver_converged,
                    solver_iterations,
                ) = self.calculate_fluxes(trial_dt);

                let pressure_physical = self.pressure_state_is_physical(p_new.as_slice());
                let solver_retry_factor = if solver_converged { 1.0 } else { 0.5 };
                let physics_retry_factor = if pressure_physical { 1.0 } else { 0.5 };
                let retry_factor = stable_dt_factor
                    .min(solver_retry_factor)
                    .min(physics_retry_factor);

                if retry_factor >= 1.0 {
                    actual_dt = trial_dt;
                    final_p = p_new;
                    final_delta_water_m3 = delta_water_m3;
                    final_delta_free_gas_sc = delta_free_gas_sc;
                    final_delta_dg_sc = delta_dg_sc;
                    final_well_controls = well_controls;
                    break;
                }

                let next_dt = trial_dt * retry_factor * 0.9;
                retry_count += 1;

                if !next_dt.is_finite() || next_dt <= 1e-12 {
                    self.last_solver_warning = if !solver_converged {
                        format!(
                            "Linear solver did not converge after {} iterations and timestep collapsed at t={:.6} days",
                            solver_iterations,
                            self.time_days + time_stepped
                        )
                    } else {
                        format!(
                            "Adaptive timestep collapsed to non-physical dt={} at t={:.6} days",
                            next_dt,
                            self.time_days + time_stepped
                        )
                    };
                    return;
                }

                if retry_count >= MAX_PRESSURE_RETRIES_PER_SUBSTEP {
                    self.last_solver_warning = if !solver_converged {
                        format!(
                            "Linear solver did not converge after {} iterations even after {} retries at t={:.6} days",
                            solver_iterations,
                            retry_count,
                            self.time_days + time_stepped
                        )
                    } else {
                        format!(
                            "Adaptive timestep exceeded retry budget while recovering a physical pressure state at t={:.6} days",
                            self.time_days + time_stepped
                        )
                    };
                    return;
                }

                trial_dt = next_dt;
            }

            self.update_saturations_and_pressure(
                &final_p,
                &final_delta_water_m3,
                &final_delta_free_gas_sc,
                &final_delta_dg_sc,
                &final_well_controls,
                actual_dt,
            );

            time_stepped += actual_dt;
            substeps += 1;
        }

        if substeps == MAX_SUBSTEPS && time_stepped < target_dt_days {
            self.last_solver_warning = format!(
                "Adaptive timestep hit MAX_SUBSTEPS before completing requested dt (advanced {:.6} of {:.6} days)",
                time_stepped,
                target_dt_days
            );
        }
    }

    fn step_internal_fim(&mut self, target_dt_days: f64) {
        self.step_internal_fim_impl(target_dt_days, false);
    }

    /// Native-only entry point: runs the same FIM timestep loop with diagnostic output.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn step_fim_verbose(&mut self, target_dt_days: f64) {
        self.step_internal_fim_impl(target_dt_days, true);
    }

    fn step_internal_fim_impl(&mut self, target_dt_days: f64, verbose: bool) {
        let mut time_stepped = 0.0;
        const MAX_SUBSTEPS: u32 = 100_000;
        const MAX_NEWTON_RETRIES_PER_SUBSTEP: u32 = 16;
        const MAX_GROWTH: f64 = 2.0;
        const TARGET_NEWTON_ITERS: f64 = 6.0;
        const TARGET_MAX_SAT_CHANGE: f64 = 0.2;
        const TARGET_MAX_PRESSURE_CHANGE_BAR: f64 = 200.0;
        let mut substeps = 0;
        let mut linear_bad_retries = 0usize;
        let mut nonlinear_bad_retries = 0usize;
        let mut mixed_retries = 0usize;
        self.last_solver_warning = String::new();
        let mut last_successful_dt = target_dt_days;
        let mut last_growth_factor = MAX_GROWTH;

        fim_trace!(verbose, "FIM step: target_dt={:.6} days, t={:.6} days", target_dt_days, self.time_days);

        let mut newton_options = FimNewtonOptions::default();
        newton_options.verbose = verbose;

        while time_stepped < target_dt_days && substeps < MAX_SUBSTEPS {
            let remaining_dt = target_dt_days - time_stepped;
            let initial_trial = if substeps == 0 {
                remaining_dt
            } else {
                remaining_dt.min(last_successful_dt * last_growth_factor)
            };
            let mut trial_dt = initial_trial;
            let mut retry_count = 0;
            // Build state once; it doesn't change across retries (only dt changes).
            let previous_state = FimState::from_simulator(self);

            loop {
                fim_trace!(verbose, "  substep {}: trial_dt={:.6} (retry={})", substeps, trial_dt, retry_count);

                let report = run_fim_timestep(
                    self,
                    &previous_state,
                    &previous_state,
                    trial_dt,
                    &newton_options,
                );

                if report.converged {
                    // Compute max saturation and pressure change for adaptive growth.
                    let mut max_dsat: f64 = 0.0;
                    let mut max_dp: f64 = 0.0;
                    for (idx, new_cell) in report.accepted_state.cells.iter().enumerate() {
                        let old_cell = &previous_state.cells[idx];
                        max_dsat = max_dsat.max((new_cell.sw - old_cell.sw).abs());
                        max_dp = max_dp.max((new_cell.pressure_bar - old_cell.pressure_bar).abs());
                    }

                    // Newton-iteration-aware growth: easy steps grow fast, hard steps barely grow.
                    let iteration_growth = (TARGET_NEWTON_ITERS / report.newton_iterations as f64)
                        .clamp(1.0, MAX_GROWTH);
                    // Saturation-change-based growth: scale down if sat change exceeded target.
                    let sat_growth = if max_dsat > TARGET_MAX_SAT_CHANGE {
                        TARGET_MAX_SAT_CHANGE / max_dsat
                    } else {
                        MAX_GROWTH
                    };
                    // Pressure-change-based growth: scale down if pressure change exceeded target.
                    let pressure_growth = if max_dp > TARGET_MAX_PRESSURE_CHANGE_BAR {
                        TARGET_MAX_PRESSURE_CHANGE_BAR / max_dp
                    } else {
                        MAX_GROWTH
                    };
                    last_growth_factor = iteration_growth.min(sat_growth).min(pressure_growth).clamp(1.0, MAX_GROWTH);

                    fim_trace!(verbose, "  substep {}: ACCEPTED dt={:.6} iters={} res={:.3e} upd={:.3e} max_dSw={:.4} max_dP={:.2} growth={:.3}",
                        substeps, trial_dt, report.newton_iterations,
                        report.final_residual_inf_norm, report.final_update_inf_norm,
                        max_dsat, max_dp, last_growth_factor);
                    let water_before = self.total_water_inventory_m3();
                    let gas_before = self.total_gas_inventory_sc();
                    report.accepted_state.write_back_to_simulator(self);
                    self.update_dynamic_well_productivity_indices();
                    let water_after = self.total_water_inventory_m3();
                    let gas_after = self.total_gas_inventory_sc();
                    self.record_fim_step_report(
                        &report.accepted_state,
                        trial_dt,
                        water_after - water_before,
                        gas_after - gas_before,
                    );
                    self.time_days += trial_dt;
                    time_stepped += trial_dt;
                    last_successful_dt = trial_dt;
                    substeps += 1;
                    break;
                }

                // Use Newton-reported cutback factor instead of fixed 0.5.
                let next_dt = trial_dt * report.cutback_factor.clamp(0.1, 0.5);
                retry_count += 1;

                if let Some(failure_diagnostics) = &report.failure_diagnostics {
                    match failure_diagnostics.class {
                        FimRetryFailureClass::LinearBad => linear_bad_retries += 1,
                        FimRetryFailureClass::NonlinearBad => nonlinear_bad_retries += 1,
                        FimRetryFailureClass::Mixed => mixed_retries += 1,
                    }
                }

                fim_trace!(verbose, "  substep {}: FAILED (iters={} res={:.3e} upd={:.3e} cutback={:.2}){} → next_dt={:.6}",
                    substeps, report.newton_iterations, report.final_residual_inf_norm,
                    report.final_update_inf_norm, report.cutback_factor,
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
                    next_dt);

                if !next_dt.is_finite() || next_dt <= 1e-12 {
                    fim_trace!(verbose, "  ABORT: timestep collapsed to {:.3e}", next_dt);
                    self.last_solver_warning = format!(
                        "FIM Newton step collapsed timestep at t={:.6} days after {} iterations",
                        self.time_days + time_stepped,
                        report.newton_iterations
                    );
                    return;
                }

                if retry_count >= MAX_NEWTON_RETRIES_PER_SUBSTEP {
                    fim_trace!(verbose, "  ABORT: exceeded retry budget ({} retries)", retry_count);
                    self.last_solver_warning = format!(
                        "FIM Newton step exceeded retry budget at t={:.6} days after {} retries",
                        self.time_days + time_stepped,
                        retry_count
                    );
                    return;
                }

                trial_dt = next_dt;
            }
        }

        if substeps == MAX_SUBSTEPS && time_stepped < target_dt_days {
            fim_trace!(verbose, "  ABORT: hit MAX_SUBSTEPS={}", MAX_SUBSTEPS);
            self.last_solver_warning = format!(
                "FIM adaptive timestep hit MAX_SUBSTEPS before completing requested dt (advanced {:.6} of {:.6} days)",
                time_stepped,
                target_dt_days
            );
        }

        fim_trace!(verbose, "FIM step done: {} substeps, advanced {:.6} of {:.6} days",
            substeps, time_stepped, target_dt_days);
        if linear_bad_retries + nonlinear_bad_retries + mixed_retries > 0 {
            fim_trace!(verbose, "FIM retry summary: linear-bad={} nonlinear-bad={} mixed={}",
                linear_bad_retries, nonlinear_bad_retries, mixed_retries);
        }
    }

    fn total_water_inventory_m3(&self) -> f64 {
        (0..self.nx * self.ny * self.nz)
            .map(|idx| self.sat_water[idx] * self.pore_volume_m3(idx))
            .sum()
    }

    fn total_gas_inventory_sc(&self) -> f64 {
        if !self.three_phase_mode {
            return 0.0;
        }

        (0..self.nx * self.ny * self.nz)
            .map(|idx| {
                let pore_volume_m3 = self.pore_volume_m3(idx).max(1e-9);
                let free_gas_sc = self.sat_gas[idx] * pore_volume_m3 / self.get_b_g(self.pressure[idx]).max(1e-9);
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

    fn solve_rs_for_dissolved_gas(
        &self,
        pressure_bar: f64,
        water_saturation: f64,
        gas_saturation: f64,
        pore_volume_m3: f64,
        dissolved_gas_sc: f64,
        rs_upper: f64,
    ) -> f64 {
        let table = match &self.pvt_table {
            Some(table) => table,
            None => return 0.0,
        };

        let target_dissolved_gas_sc = dissolved_gas_sc.max(0.0);
        if target_dissolved_gas_sc <= 0.0 || rs_upper <= 0.0 {
            return 0.0;
        }

        let oil_saturation = (1.0 - water_saturation - gas_saturation).max(0.0);
        if oil_saturation <= 1e-12 || pore_volume_m3 <= 0.0 {
            return 0.0;
        }

        let mut low = 0.0;
        let mut high = rs_upper.max(0.0);
        for _ in 0..64 {
            let mid = 0.5 * (low + high);
            let (bo_mid, _) = table.interpolate_oil(pressure_bar, mid);
            let dissolved_mid = (oil_saturation * pore_volume_m3 / bo_mid.max(1e-9)) * mid;
            if dissolved_mid < target_dissolved_gas_sc {
                low = mid;
            } else {
                high = mid;
            }
        }

        0.5 * (low + high)
    }

    pub(crate) fn split_gas_inventory_after_transport(
        &self,
        pressure_bar: f64,
        pore_volume_m3: f64,
        water_saturation: f64,
        transported_free_gas_sc: f64,
        dissolved_gas_sc: f64,
    ) -> (f64, f64, f64) {
        let table = match &self.pvt_table {
            Some(table) => table,
            None => {
                let bg = self.get_b_g(pressure_bar).max(1e-9);
                let sg = ((transported_free_gas_sc.max(0.0) * bg) / pore_volume_m3.max(1e-9))
                    .clamp(0.0, (1.0 - water_saturation).max(0.0));
                let so = (1.0 - water_saturation - sg).max(0.0);
                return (sg, so, 0.0);
            }
        };

        let total_hydrocarbon_saturation = (1.0 - water_saturation).max(0.0);
        let bg = self.get_b_g(pressure_bar).max(1e-9);
        let free_gas_sc_transport = transported_free_gas_sc.max(0.0);
        let sg_transport = ((free_gas_sc_transport * bg) / pore_volume_m3.max(1e-9))
            .clamp(0.0, total_hydrocarbon_saturation);
        let so_transport = (total_hydrocarbon_saturation - sg_transport).max(0.0);
        let dissolved_gas_sc = dissolved_gas_sc.max(0.0);

        let rs_max = table.interpolate(pressure_bar).rs_m3m3.max(0.0);
        let (bo_sat, _) = table.interpolate_oil(pressure_bar, rs_max);
        let bo_sat = bo_sat.max(1e-9);

        if !self.gas_redissolution_enabled {
            let max_dissolved_sc_transport = (so_transport * pore_volume_m3 / bo_sat) * rs_max;
            if dissolved_gas_sc <= max_dissolved_sc_transport + 1e-9 {
                let rs = self.solve_rs_for_dissolved_gas(
                    pressure_bar,
                    water_saturation,
                    sg_transport,
                    pore_volume_m3,
                    dissolved_gas_sc,
                    rs_max,
                );
                return (sg_transport, so_transport, rs);
            }
        }

        let total_gas_sc = free_gas_sc_transport + dissolved_gas_sc;
        let max_all_dissolved_sc =
            (total_hydrocarbon_saturation * pore_volume_m3 / bo_sat) * rs_max;
        if self.gas_redissolution_enabled && total_gas_sc <= max_all_dissolved_sc + 1e-9 {
            let rs = self.solve_rs_for_dissolved_gas(
                pressure_bar,
                water_saturation,
                0.0,
                pore_volume_m3,
                total_gas_sc,
                rs_max,
            );
            return (0.0, total_hydrocarbon_saturation, rs);
        }

        let denom = (1.0 / bg) - (rs_max / bo_sat);
        let sg_saturated = if denom.abs() > 1e-12 {
            ((total_gas_sc / pore_volume_m3) - (total_hydrocarbon_saturation * rs_max / bo_sat))
                / denom
        } else {
            sg_transport
        };
        let sg_lower_bound = if self.gas_redissolution_enabled {
            0.0
        } else {
            sg_transport
        };
        let sg = sg_saturated.clamp(sg_lower_bound, total_hydrocarbon_saturation);
        let so = (total_hydrocarbon_saturation - sg).max(0.0);
        (sg, so, rs_max)
    }

    fn pressure_state_bounds(&self) -> (f64, f64) {
        let current_min = self
            .pressure
            .iter()
            .copied()
            .filter(|p| p.is_finite())
            .fold(f64::INFINITY, f64::min);
        let current_max = self
            .pressure
            .iter()
            .copied()
            .filter(|p| p.is_finite())
            .fold(f64::NEG_INFINITY, f64::max);
        let bhp_min = self
            .wells
            .iter()
            .map(|w| w.bhp)
            .filter(|p| p.is_finite())
            .fold(f64::INFINITY, f64::min);
        let bhp_max = self
            .wells
            .iter()
            .map(|w| w.bhp)
            .filter(|p| p.is_finite())
            .fold(f64::NEG_INFINITY, f64::max);

        let control_min = [self.well_bhp_min]
            .into_iter()
            .filter(|p| p.is_finite())
            .fold(f64::INFINITY, f64::min);
        let control_max = [self.well_bhp_max]
            .into_iter()
            .filter(|p| p.is_finite())
            .fold(f64::NEG_INFINITY, f64::max);

        let reference_min = current_min.min(bhp_min).min(control_min);
        let reference_max = current_max.max(bhp_max).max(control_max);
        let swing_allowance = 10.0 * self.max_pressure_change_per_step + 500.0;
        let lower = if reference_min.is_finite() {
            (reference_min - swing_allowance).max(1.0)
        } else {
            1.0
        };
        let upper = if reference_max.is_finite() {
            reference_max + swing_allowance
        } else {
            10_000.0
        }
        .min(50_000.0);
        (lower, upper.max(lower + 1.0))
    }

    fn pressure_state_is_physical(&self, pressures: &[f64]) -> bool {
        let (lower, upper) = self.pressure_state_bounds();
        pressures
            .iter()
            .all(|p| p.is_finite() && *p >= lower && *p <= upper)
    }
}

#[cfg(test)]
mod tests {
    use crate::ReservoirSimulator;

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
