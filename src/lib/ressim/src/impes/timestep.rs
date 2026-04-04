use crate::ReservoirSimulator;

pub(crate) fn step_internal(sim: &mut ReservoirSimulator, target_dt_days: f64) {
    let mut time_stepped = 0.0;
    const MAX_SUBSTEPS: u32 = 100_000;
    const MAX_PRESSURE_RETRIES_PER_SUBSTEP: u32 = 32;
    let mut substeps = 0;
    sim.last_solver_warning = String::new();

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
            sim.update_dynamic_well_productivity_indices();

            let (
                p_new,
                delta_water_m3,
                delta_free_gas_sc,
                delta_dg_sc,
                well_controls,
                stable_dt_factor,
                solver_converged,
                solver_iterations,
            ) = sim.calculate_fluxes(trial_dt);

            let pressure_physical = sim.pressure_state_is_physical(p_new.as_slice());
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
                sim.last_solver_warning = if !solver_converged {
                    format!(
                        "Linear solver did not converge after {} iterations and timestep collapsed at t={:.6} days",
                        solver_iterations,
                        sim.time_days + time_stepped
                    )
                } else {
                    format!(
                        "Adaptive timestep collapsed to non-physical dt={} at t={:.6} days",
                        next_dt,
                        sim.time_days + time_stepped
                    )
                };
                return;
            }

            if retry_count >= MAX_PRESSURE_RETRIES_PER_SUBSTEP {
                sim.last_solver_warning = if !solver_converged {
                    format!(
                        "Linear solver did not converge after {} iterations even after {} retries at t={:.6} days",
                        solver_iterations,
                        retry_count,
                        sim.time_days + time_stepped
                    )
                } else {
                    format!(
                        "Adaptive timestep exceeded retry budget while recovering a physical pressure state at t={:.6} days",
                        sim.time_days + time_stepped
                    )
                };
                return;
            }

            trial_dt = next_dt;
        }

        sim.update_saturations_and_pressure(
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
        sim.last_solver_warning = format!(
            "Adaptive timestep hit MAX_SUBSTEPS before completing requested dt (advanced {:.6} of {:.6} days)",
            time_stepped, target_dt_days
        );
    }
}

impl ReservoirSimulator {
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
