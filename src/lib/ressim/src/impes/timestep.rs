use crate::ReservoirSimulator;

pub(crate) fn step_internal(sim: &mut ReservoirSimulator, target_dt_days: f64) {
    let mut time_stepped = 0.0;
    const MAX_SUBSTEPS: u32 = 100_000;
    const MAX_PRESSURE_RETRIES_PER_SUBSTEP: u32 = 32;
    let mut substeps = 0;
    sim.last_solver_warning = String::new();

    while time_stepped < target_dt_days && substeps < MAX_SUBSTEPS {
        let remaining_dt = target_dt_days - time_stepped;
        // A priori CFL estimate using current pressures and mobilities. Only
        // applied when it trims less than 50% of remaining_dt — if the flow is
        // so fast that CFL gives a tiny fraction of remaining_dt, the retry
        // loop is already the right mechanism and the pre-estimate would just
        // fragment the step budget needlessly.
        let cfl_dt = sim.cfl_dt_estimate();
        let mut trial_dt = if cfl_dt >= remaining_dt * 0.5 {
            remaining_dt.min(cfl_dt)
        } else {
            remaining_dt
        };
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
    /// A priori CFL timestep estimate [days] using current pressures and mobilities.
    ///
    /// For each cell, sums outgoing flux T·λ_t·max(Δp,0) across all faces and
    /// divides by pore volume. The CFL-limited dt is `max_sat_change_per_step`
    /// divided by the maximum such ratio. Gravity and capillary corrections are
    /// omitted here (they complicate the pre-solve estimate and the retry loop
    /// handles any residual exceedances). Returns `f64::INFINITY` when all fluxes
    /// are zero (quiescent state).
    fn cfl_dt_estimate(&self) -> f64 {
        const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;
        let mut max_flux_over_pv = 0.0_f64;

        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i, j, k);
                    let vp = self.pore_volume_m3(id);
                    if vp <= 0.0 {
                        continue;
                    }
                    let lam_t = self.total_mobility(id);
                    let mut outflow = 0.0_f64;

                    let neighbors: &[(isize, isize, isize, char)] = &[
                        (1, 0, 0, 'x'),
                        (-1, 0, 0, 'x'),
                        (0, 1, 0, 'y'),
                        (0, -1, 0, 'y'),
                        (0, 0, 1, 'z'),
                        (0, 0, -1, 'z'),
                    ];
                    for &(di, dj, dk, dim) in neighbors {
                        let ni = i as isize + di;
                        let nj = j as isize + dj;
                        let nk = k as isize + dk;
                        if ni < 0
                            || nj < 0
                            || nk < 0
                            || ni >= self.nx as isize
                            || nj >= self.ny as isize
                            || nk >= self.nz as isize
                        {
                            continue;
                        }
                        let nid = self.idx(ni as usize, nj as usize, nk as usize);
                        let dp = self.pressure[id] - self.pressure[nid];
                        if dp > 0.0 {
                            let geom_t =
                                DARCY_METRIC_FACTOR * self.geometric_transmissibility(id, nid, dim);
                            outflow += geom_t * lam_t * dp;
                        }
                    }

                    let ratio = outflow / vp;
                    if ratio > max_flux_over_pv {
                        max_flux_over_pv = ratio;
                    }
                }
            }
        }

        if max_flux_over_pv > 0.0 {
            // Apply a 2× relaxation: the estimate ignores capillary and gravity
            // corrections and uses single-point mobilities, so it is inherently
            // conservative. The retry loop handles any residual exceedances.
            2.0 * self.max_sat_change_per_step / max_flux_over_pv
        } else {
            f64::INFINITY
        }
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
