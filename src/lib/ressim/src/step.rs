use nalgebra::DVector;
use sprs::{CsMat, TriMatI};
use std::f64;

use crate::solver::solve_bicgstab_with_guess;
use crate::{InjectedFluid, ReservoirSimulator, TimePointRates, Well};

/// Conversion factor from mD·m²/(m·cP) to m³/day/bar.
/// Derivation: 1 mD = 9.8692e-16 m², 1 cP = 1e-3 Pa·s, 1 bar = 1e5 Pa, 1 day = 86400 s
/// Factor = 9.8692e-16 × 1e3 × 1e5 × 86400 = 8.5269888e-3
const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;
const MIN_GOR_OIL_RATE_SC_DAY: f64 = 10.0;

#[derive(Clone, Copy)]
pub(crate) enum WellControlDecision {
    Disabled,
    Rate { q_m3_day: f64 },
    Bhp { bhp_bar: f64 },
}

#[derive(Clone, Copy)]
pub(crate) struct ResolvedWellControl {
    pub(crate) decision: WellControlDecision,
    pub(crate) bhp_limited: bool,
}

impl ReservoirSimulator {
    pub(crate) fn calculate_well_productivity_index(
        &self,
        id: usize,
        well_radius: f64,
        skin: f64,
    ) -> Result<f64, String> {
        // Calculate equivalent radius (Peaceman's model)
        let kx = self.perm_x[id];
        let ky = self.perm_y[id];
        if !kx.is_finite() || !ky.is_finite() || kx <= 0.0 || ky <= 0.0 {
            return Err(format!(
                "Cell permeability must be positive and finite for well PI calculation, got kx={}, ky={}",
                kx, ky
            ));
        }

        let r_eq =
            0.28 * f64::sqrt(
                f64::sqrt(kx / ky) * self.dx.powi(2) + f64::sqrt(ky / kx) * self.dy.powi(2),
            ) / ((kx / ky).powf(0.25) + (ky / kx).powf(0.25));
        if !r_eq.is_finite() || r_eq <= 0.0 {
            return Err(format!(
                "Equivalent radius must be positive and finite, got: {}",
                r_eq
            ));
        }

        if r_eq <= well_radius {
            return Err(format!(
                "Equivalent radius must be greater than well radius for valid PI. r_eq={}, rw={}",
                r_eq, well_radius
            ));
        }

        // Calculate productivity index (PI)
        let k_avg = f64::sqrt(kx * ky); // Geometric mean of horizontal permeabilities
        let total_mobility = self.total_mobility(id);
        if !k_avg.is_finite() || k_avg <= 0.0 {
            return Err(format!(
                "Average permeability must be positive and finite, got: {}",
                k_avg
            ));
        }
        if !total_mobility.is_finite() || total_mobility < 0.0 {
            return Err(format!(
                "Total mobility must be finite and non-negative, got: {}",
                total_mobility
            ));
        }

        let denom = f64::ln(r_eq / well_radius) + skin;
        if !denom.is_finite() || denom.abs() <= f64::EPSILON {
            return Err(format!(
                "Invalid PI denominator ln(r_eq/r_w)+skin = {}. Check well radius and skin.",
                denom
            ));
        }

        // Peaceman's well index formula for metric/bar units (m³, bar, day)
        // PI = C * 2π * k_avg * h * total_mobility / (ln(r_eq/r_w) + skin)
        // Constant 8.5269888e-3 converts from mD·m²/(m·cP) to m³/day/bar
        // Derivation: 1 mD = 9.8692e-16 m², 1 cP = 1e-3 Pa·s, 1 bar = 1e5 Pa, 1 day = 86400 s
        // Factor = 9.8692e-16 * 1e3 * 1e5 * 86400 = 8.5269888e-3
        Ok(
            (DARCY_METRIC_FACTOR * 2.0 * std::f64::consts::PI * k_avg * self.dz_at(id) * total_mobility)
                / denom,
        )
    }

    pub(crate) fn update_dynamic_well_productivity_indices(&mut self) {
        let mut updated_pi: Vec<Option<f64>> = Vec::with_capacity(self.wells.len());

        for well in self.wells.iter() {
            let id = self.idx(well.i, well.j, well.k);

            let maybe_pi = self
                .calculate_well_productivity_index(id, well.well_radius, well.skin)
                .ok()
                .filter(|pi| pi.is_finite() && *pi >= 0.0);
            updated_pi.push(maybe_pi);
        }

        for (well, maybe_pi) in self.wells.iter_mut().zip(updated_pi.into_iter()) {
            if let Some(pi) = maybe_pi {
                well.productivity_index = pi;
            }
        }
    }

    /// Total mobility [1/cP] = lambda_t = (k_rw/μ_w) + (k_ro/μ_o) [+ k_rg/μ_g in 3-phase]
    fn total_mobility(&self, id: usize) -> f64 {
        if self.three_phase_mode {
            return self.total_mobility_3p(id);
        }
        let krw = self.scal.k_rw(self.sat_water[id]);
        let kro = self.scal.k_ro(self.sat_water[id]);
        krw / self.get_mu_w(self.pressure[id]) + kro / self.get_mu_o(self.pressure[id])
    }

    /// Phase mobilities [1/cP] for water and oil (2-phase)
    fn phase_mobilities(&self, id: usize) -> (f64, f64) {
        let krw = self.scal.k_rw(self.sat_water[id]);
        let kro = self.scal.k_ro(self.sat_water[id]);
        (krw / self.get_mu_w(self.pressure[id]), kro / self.get_mu_o(self.pressure[id]))
    }

    // ── Three-phase helper methods ────────────────────────────────────────────

    /// Total mobility using Stone II k_ro and Corey k_rg
    fn total_mobility_3p(&self, id: usize) -> f64 {
        let s = match &self.scal_3p {
            Some(s) => s,
            None => return self.total_mobility(id),
        };
        let sw = self.sat_water[id];
        let sg = self.sat_gas[id];
        s.k_rw(sw) / self.get_mu_w(self.pressure[id])
            + s.k_ro_stone2(sw, sg) / self.get_mu_o_cell(id, self.pressure[id])
            + s.k_rg(sg) / self.get_mu_g(self.pressure[id])
    }

    /// Phase mobilities (λ_w, λ_o, λ_g) using Stone II k_ro
    fn phase_mobilities_3p(&self, id: usize) -> (f64, f64, f64) {
        let s = match &self.scal_3p {
            Some(s) => s,
            None => {
                let (w, o) = self.phase_mobilities(id);
                return (w, o, 0.0);
            }
        };
        let sw = self.sat_water[id];
        let sg = self.sat_gas[id];
        (
            s.k_rw(sw) / self.get_mu_w(self.pressure[id]),
            s.k_ro_stone2(sw, sg) / self.get_mu_o_cell(id, self.pressure[id]),
            s.k_rg(sg) / self.get_mu_g(self.pressure[id]),
        )
    }

    /// Gas mobility [1/cP]
    fn gas_mobility(&self, id: usize) -> f64 {
        self.scal_3p
            .as_ref()
            .map_or(0.0, |s| s.k_rg(self.sat_gas[id]) / self.get_mu_g(self.pressure[id]))
    }

    /// Oil-gas capillary pressure [bar] at given gas saturation
    fn get_gas_oil_capillary_pressure(&self, s_g: f64) -> f64 {
        match (&self.pc_og, &self.scal_3p) {
            (Some(pc), Some(rock)) => pc.capillary_pressure_og(s_g, rock),
            _ => 0.0,
        }
    }

    /// Fractional flow of gas = λ_g / λ_t (three-phase)
    fn frac_flow_gas(&self, id: usize) -> f64 {
        let lam_g = self.gas_mobility(id);
        let lam_t = self.total_mobility_3p(id);
        if lam_t <= 0.0 {
            0.0
        } else {
            (lam_g / lam_t).clamp(0.0, 1.0)
        }
    }

    /// Fractional flow of water in three-phase system = λ_w / λ_t
    fn frac_flow_water_3p(&self, id: usize) -> f64 {
        let lam_t = self.total_mobility_3p(id);
        if lam_t <= 0.0 {
            return 0.0;
        }
        let (lam_w, _, _) = self.phase_mobilities_3p(id);
        (lam_w / lam_t).clamp(0.0, 1.0)
    }

    /// Get capillary pressure [bar] at given water saturation
    fn get_capillary_pressure(&self, s_w: f64) -> f64 {
        self.pc.capillary_pressure(s_w, &self.scal)
    }

    fn gravity_head_bar(&self, depth_i: f64, depth_j: f64, density_kg_m3: f64) -> f64 {
        if !self.gravity_enabled {
            return 0.0;
        }

        // rho [kg/m³] * g [m/s²] * dz [m] = Pa, then convert Pa -> bar using 1e-5
        density_kg_m3 * 9.80665 * (depth_i - depth_j) * 1e-5
    }

    fn interface_density_barrier(&self, rho_i: f64, rho_j: f64) -> f64 {
        0.5 * (rho_i + rho_j)
    }

    /// Fractional flow of water [dimensionless] = f_w = λ_w / λ_t
    /// Used in upwind scheme for saturation transport
    fn frac_flow_water(&self, id: usize) -> f64 {
        let krw = self.scal.k_rw(self.sat_water[id]);
        let lam_w = krw / self.get_mu_w(self.pressure[id]);
        let lam_t = lam_w + (self.scal.k_ro(self.sat_water[id]) / self.get_mu_o(self.pressure[id]));
        if lam_t <= 0.0 {
            0.0
        } else {
            (lam_w / lam_t).clamp(0.0, 1.0)
        }
    }
    pub(crate) fn step_internal(&mut self, target_dt_days: f64) {
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
                // Dynamic PI update with latest local saturation/mobility before pressure solve
                self.update_dynamic_well_productivity_indices();

                let (
                    p_new,
                    delta_water_m3,
                    delta_free_gas_sc,
                    delta_dg_sc,
                    well_controls,
                    stable_dt_factor,
                    pcg_converged,
                    pcg_iters,
                ) = self.calculate_fluxes(trial_dt);

                let pressure_physical = self.pressure_state_is_physical(p_new.as_slice());
                let solver_retry_factor = if pcg_converged { 1.0 } else { 0.5 };
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
                    self.last_solver_warning = if !pcg_converged {
                        format!(
                            "BiCGSTAB solver did not converge after {} iterations and timestep collapsed at t={:.6} days",
                            pcg_iters,
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
                    self.last_solver_warning = if !pcg_converged {
                        format!(
                            "BiCGSTAB solver did not converge after {} iterations even after {} retries at t={:.6} days",
                            pcg_iters,
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

            // Update saturations and pressure with the adjusted (or full) timestep
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

    #[cfg_attr(not(test), allow(dead_code))]
    fn injector_well_count(&self) -> usize {
        self.wells.iter().filter(|w| w.injector).count()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn producer_well_count(&self) -> usize {
        self.wells.iter().filter(|w| !w.injector).count()
    }

    fn phase_mobilities_at_pressure(&self, id: usize, pressure_bar: f64) -> (f64, f64) {
        let krw = self.scal.k_rw(self.sat_water[id]);
        let kro = self.scal.k_ro(self.sat_water[id]);
        (
            krw / self.get_mu_w(pressure_bar),
            kro / self.get_mu_o_cell(id, pressure_bar),
        )
    }

    fn phase_mobilities_3p_at_pressure(&self, id: usize, pressure_bar: f64) -> (f64, f64, f64) {
        let s = match &self.scal_3p {
            Some(s) => s,
            None => {
                let (w, o) = self.phase_mobilities_at_pressure(id, pressure_bar);
                return (w, o, 0.0);
            }
        };
        let sw = self.sat_water[id];
        let sg = self.sat_gas[id];
        (
            s.k_rw(sw) / self.get_mu_w(pressure_bar),
            s.k_ro_stone2(sw, sg) / self.get_mu_o_cell(id, pressure_bar),
            s.k_rg(sg) / self.get_mu_g(pressure_bar),
        )
    }

    fn producer_oil_fraction_at_pressure(&self, id: usize, pressure_bar: f64) -> f64 {
        if self.three_phase_mode {
            let (lam_w, lam_o, lam_g) = self.phase_mobilities_3p_at_pressure(id, pressure_bar);
            let lam_t = (lam_w + lam_o + lam_g).max(f64::EPSILON);
            (lam_o / lam_t).clamp(0.0, 1.0)
        } else {
            let (lam_w, lam_o) = self.phase_mobilities_at_pressure(id, pressure_bar);
            let lam_t = (lam_w + lam_o).max(f64::EPSILON);
            (lam_o / lam_t).clamp(0.0, 1.0)
        }
    }

    fn completion_rate_for_bhp(&self, well: &Well, pressure_bar: f64, bhp_bar: f64) -> Option<f64> {
        if !well.productivity_index.is_finite() || !pressure_bar.is_finite() || !bhp_bar.is_finite() {
            return None;
        }
        let raw_rate = well.productivity_index * (pressure_bar - bhp_bar);
        if !raw_rate.is_finite() {
            return None;
        }
        if well.injector {
            Some(raw_rate.min(0.0))
        } else {
            Some(raw_rate.max(0.0))
        }
    }

    fn completion_surface_rate_sc_day(&self, well: &Well, pressure_bar: f64, bhp_bar: f64) -> Option<f64> {
        let q_m3_day = self.completion_rate_for_bhp(well, pressure_bar, bhp_bar)?;
        let id = self.idx(well.i, well.j, well.k);
        if well.injector {
            let injected_sc_rate = match self.injected_fluid {
                InjectedFluid::Water => (-q_m3_day) / self.b_w.max(1e-9),
                InjectedFluid::Gas => (-q_m3_day) / self.get_b_g(pressure_bar).max(1e-9),
            };
            Some(injected_sc_rate.max(0.0))
        } else {
            let oil_fraction = self.producer_oil_fraction_at_pressure(id, pressure_bar);
            let oil_rate_sc = q_m3_day * oil_fraction / self.get_b_o_cell(id, pressure_bar).max(1e-9);
            Some(oil_rate_sc.max(0.0))
        }
    }

    fn well_transport_rate_from_control(
        &self,
        well: &Well,
        control: ResolvedWellControl,
        pressure_bar: f64,
    ) -> Option<f64> {
        match control.decision {
            WellControlDecision::Disabled => Some(0.0),
            WellControlDecision::Rate { q_m3_day } => Some(q_m3_day),
            WellControlDecision::Bhp { bhp_bar } => {
                self.completion_rate_for_bhp(well, pressure_bar, bhp_bar)
            }
        }
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

    fn split_gas_inventory_after_transport(
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
        let max_all_dissolved_sc = (total_hydrocarbon_saturation * pore_volume_m3 / bo_sat) * rs_max;
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
            ((total_gas_sc / pore_volume_m3) - (total_hydrocarbon_saturation * rs_max / bo_sat)) / denom
        } else {
            sg_transport
        };
        let sg_lower_bound = if self.gas_redissolution_enabled { 0.0 } else { sg_transport };
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

    #[cfg_attr(not(test), allow(dead_code))]
    fn group_pressures_with_override(&self, well: &Well, pressure_bar: f64) -> Vec<f64> {
        let mut pressures = self.pressure.clone();
        let id = self.idx(well.i, well.j, well.k);
        if id < pressures.len() {
            pressures[id] = pressure_bar;
        }
        pressures
    }

    fn solve_group_bhp_for_pressures(&self, injector: bool, pressures: &[f64]) -> Option<(f64, bool)> {
        let use_rate_control = if injector {
            self.injector_rate_controlled
        } else {
            self.producer_rate_controlled
        };
        if !use_rate_control {
            return None;
        }

        let wells: Vec<&Well> = self.wells.iter().filter(|well| well.injector == injector).collect();
        if wells.is_empty() {
            return None;
        }

        let total_surface_target = if injector {
            self.target_injector_surface_rate_m3_day
        } else {
            self.target_producer_surface_rate_m3_day
        };
        let total_reservoir_target = if injector {
            self.target_injector_rate_m3_day
        } else {
            self.target_producer_rate_m3_day
        };

        let total_rate_for_bhp = |bhp_bar: f64| -> f64 {
            wells.iter()
                .filter_map(|well| {
                    let id = self.idx(well.i, well.j, well.k);
                    let pressure_bar = pressures[id];
                    if injector {
                        match total_surface_target {
                            Some(_) => self.completion_surface_rate_sc_day(well, pressure_bar, bhp_bar),
                            None => self.completion_rate_for_bhp(well, pressure_bar, bhp_bar).map(|q| (-q).max(0.0)),
                        }
                    } else {
                        match total_surface_target {
                            Some(_) => self.completion_surface_rate_sc_day(well, pressure_bar, bhp_bar),
                            None => self.completion_rate_for_bhp(well, pressure_bar, bhp_bar),
                        }
                    }
                })
                .sum()
        };

        let target_rate = if let Some(surface_target) = total_surface_target {
            surface_target.max(0.0)
        } else {
            total_reservoir_target.max(0.0)
        };

        let group_min_pressure = wells
            .iter()
            .map(|well| pressures[self.idx(well.i, well.j, well.k)])
            .fold(f64::INFINITY, f64::min);
        let group_max_pressure = wells
            .iter()
            .map(|well| pressures[self.idx(well.i, well.j, well.k)])
            .fold(f64::NEG_INFINITY, f64::max);

        if !group_min_pressure.is_finite() || !group_max_pressure.is_finite() {
            return None;
        }

        if injector {
            let bhp_limit = self.well_bhp_max;
            let max_achievable_rate = total_rate_for_bhp(bhp_limit);
            if target_rate >= max_achievable_rate - 1e-9 {
                return Some((bhp_limit, true));
            }

            let mut low = group_min_pressure.min(bhp_limit);
            let mut high = bhp_limit;
            for _ in 0..64 {
                let mid = 0.5 * (low + high);
                let rate_mid = total_rate_for_bhp(mid);
                if rate_mid < target_rate {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            Some((0.5 * (low + high), false))
        } else {
            let bhp_limit = self.well_bhp_min;
            let max_achievable_rate = total_rate_for_bhp(bhp_limit);
            if target_rate >= max_achievable_rate - 1e-9 {
                return Some((bhp_limit, true));
            }

            let mut low = bhp_limit;
            let mut high = group_max_pressure.max(bhp_limit);
            for _ in 0..64 {
                let mid = 0.5 * (low + high);
                let rate_mid = total_rate_for_bhp(mid);
                if rate_mid > target_rate {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            Some((0.5 * (low + high), false))
        }
    }

    pub(crate) fn resolve_well_control_for_pressures(
        &self,
        well: &Well,
        pressures: &[f64],
    ) -> Option<ResolvedWellControl> {
        if well.injector && !self.injector_enabled {
            return Some(ResolvedWellControl {
                decision: WellControlDecision::Disabled,
                bhp_limited: false,
            });
        }

        let use_rate_control = if well.injector {
            self.injector_rate_controlled
        } else {
            self.producer_rate_controlled
        };

        let id = self.idx(well.i, well.j, well.k);
        let pressure_bar = pressures[id];

        if use_rate_control {
            let (group_bhp, bhp_limited) = self.solve_group_bhp_for_pressures(well.injector, pressures)?;
            let q_target = self.completion_rate_for_bhp(well, pressure_bar, group_bhp)?;
            return Some(ResolvedWellControl {
                decision: if bhp_limited {
                    WellControlDecision::Bhp { bhp_bar: group_bhp }
                } else {
                    WellControlDecision::Rate { q_m3_day: q_target }
                },
                bhp_limited,
            });
        }

        if !well.productivity_index.is_finite()
            || !well.bhp.is_finite()
            || !pressure_bar.is_finite()
        {
            return None;
        }

        Some(ResolvedWellControl {
            decision: WellControlDecision::Bhp { bhp_bar: well.bhp },
            bhp_limited: false,
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn target_rate_m3_day(&self, well: &Well, pressure_bar: f64) -> Option<f64> {
        if well.injector && !self.injector_enabled {
            return Some(0.0);
        }

        let use_rate_control = if well.injector {
            self.injector_rate_controlled
        } else {
            self.producer_rate_controlled
        };

        if use_rate_control {
            if well.injector {
                let n_inj = self.injector_well_count();
                if n_inj == 0 {
                    return Some(0.0);
                }
                if let Some(surface_rate_sc_day) = self.target_injector_surface_rate_m3_day {
                    let surface_rate_per_well = surface_rate_sc_day / n_inj as f64;
                    let reservoir_rate_per_well = match self.injected_fluid {
                        InjectedFluid::Water => surface_rate_per_well * self.b_w.max(1e-9),
                        InjectedFluid::Gas => surface_rate_per_well * self.get_b_g(pressure_bar).max(1e-9),
                    };
                    return Some(-reservoir_rate_per_well);
                }
                return Some(-(self.target_injector_rate_m3_day / n_inj as f64));
            }

            let n_prod = self.producer_well_count();
            if n_prod == 0 {
                return Some(0.0);
            }
            if let Some(surface_rate_sc_day) = self.target_producer_surface_rate_m3_day {
                let surface_rate_per_well = surface_rate_sc_day / n_prod as f64;
                let id = self.idx(well.i, well.j, well.k);
                let (fw, fg) = if self.three_phase_mode {
                    (self.frac_flow_water_3p(id), self.frac_flow_gas(id))
                } else {
                    (self.frac_flow_water(id), 0.0)
                };
                let oil_fraction = (1.0 - fw - fg).max(1e-6);
                let reservoir_rate_per_well = surface_rate_per_well
                    * self.get_b_o_cell(id, pressure_bar).max(1e-9)
                    / oil_fraction;
                return Some(reservoir_rate_per_well);
            }
            return Some(self.target_producer_rate_m3_day / n_prod as f64);
        }

        None
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn resolve_well_control(
        &self,
        well: &Well,
        pressure_bar: f64,
    ) -> Option<ResolvedWellControl> {
        let pressures = self.group_pressures_with_override(well, pressure_bar);
        self.resolve_well_control_for_pressures(well, &pressures)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn well_rate_m3_day(&self, well: &Well, pressure_bar: f64) -> Option<f64> {
        let pressures = self.group_pressures_with_override(well, pressure_bar);
        self.well_rate_m3_day_for_pressures(well, &pressures)
    }

    pub(crate) fn well_rate_m3_day_for_pressures(&self, well: &Well, pressures: &[f64]) -> Option<f64> {
        let id = self.idx(well.i, well.j, well.k);
        let pressure_bar = pressures[id];
        match self.resolve_well_control_for_pressures(well, pressures)?.decision {
            WellControlDecision::Disabled => Some(0.0),
            WellControlDecision::Rate { q_m3_day } => Some(q_m3_day),
            WellControlDecision::Bhp { bhp_bar } => {
                let q_m3_day = self.completion_rate_for_bhp(well, pressure_bar, bhp_bar)?;
                if q_m3_day.is_finite() {
                    Some(q_m3_day)
                } else {
                    None
                }
            }
        }
    }

    fn calculate_fluxes(
        &self,
        delta_t_days: f64,
    ) -> (DVector<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<Option<ResolvedWellControl>>, f64, bool, usize) {
        let n_cells = self.nx * self.ny * self.nz;
        if n_cells == 0 {
            return (DVector::zeros(0), vec![], vec![], vec![], vec![], 1.0, true, 0);
        }
        let dt_days = delta_t_days.max(1e-12);

        // Build triplet lists for A matrix and RHS b of pressure equation
        let mut rows: Vec<usize> = Vec::with_capacity(n_cells * 7);
        let mut cols: Vec<usize> = Vec::with_capacity(n_cells * 7);
        let mut vals: Vec<f64> = Vec::with_capacity(n_cells * 7);

        let mut b_rhs = DVector::<f64>::zeros(n_cells);
        let mut diag_inv = DVector::<f64>::zeros(n_cells);

        // Pre-compute well control decisions at OLD pressure — used in both
        // pressure equation assembly and saturation transport for consistency.
        // Without this, the pressure equation (old p) and saturation update (new p)
        // can make different rate/BHP decisions, causing oscillations.
        let well_controls: Vec<Option<ResolvedWellControl>> = self
            .wells
            .iter()
            .map(|w| self.resolve_well_control_for_pressures(w, &self.pressure))
            .collect();

        // Assemble pressure equation: accumulation + transmissibility + well terms
        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i, j, k);

                    // Pore volume [m³]
                    let vp_m3 = self.pore_volume_m3(id);

                    // Per-cell total compressibility [1/bar]
                    // Black-oil IMPES (Aziz & Settari Eq. 7.60):
                    //   c_t = c_rock + So·c_o_eff + Sw·c_w + Sg·c_g
                    // where c_o_eff = -(1/Bo)dBo/dp + (Bg/Bo)dRs/dp
                    // includes the dissolved-gas liberation compressibility.
                    let sg_id = self.sat_gas[id];
                    let so_id = if self.three_phase_mode {
                        (1.0 - self.sat_water[id] - sg_id).max(0.0)
                    } else {
                        self.sat_oil[id]
                    };
                    let c_o_term = if self.three_phase_mode {
                        self.get_c_o_effective(self.pressure[id], self.rs[id])
                    } else {
                        self.get_c_o(self.pressure[id])
                    };
                    let c_t = (c_o_term * so_id
                        + self.pvt.c_w * self.sat_water[id]
                        + if self.three_phase_mode { self.get_c_g(self.pressure[id]) * sg_id } else { 0.0 })
                        + self.rock_compressibility;

                    // Accumulation term: (Vp [m³] * c_t [1/bar]) / dt [day]
                    // Units: [m³ * 1/bar / day] = [m³/bar/day]
                    let accum = (vp_m3 * c_t) / dt_days;
                    let mut diag = accum;

                    // Move old pressure term to RHS: accum * p_old
                    b_rhs[id] += accum * self.pressure[id];

                    // neighbors: compute flux transmissibilities
                    let mut neighbors: Vec<(usize, char, usize)> = Vec::new();
                    if i > 0 {
                        neighbors.push((self.idx(i - 1, j, k), 'x', k));
                    }
                    if i < self.nx - 1 {
                        neighbors.push((self.idx(i + 1, j, k), 'x', k));
                    }
                    if j > 0 {
                        neighbors.push((self.idx(i, j - 1, k), 'y', k));
                    }
                    if j < self.ny - 1 {
                        neighbors.push((self.idx(i, j + 1, k), 'y', k));
                    }
                    if k > 0 {
                        neighbors.push((self.idx(i, j, k - 1), 'z', k - 1));
                    }
                    if k < self.nz - 1 {
                        neighbors.push((self.idx(i, j, k + 1), 'z', k + 1));
                    }

                    for (n_id, dim, n_k) in neighbors.iter() {
                        // Phase potential formulation
                        let depth_i = self.depth_at_k(k);
                        let depth_j = self.depth_at_k(*n_k);

                        let p_i = self.pressure[id];
                        let p_j = self.pressure[*n_id];
                        let pc_i = self.get_capillary_pressure(self.sat_water[id]);
                        let pc_j = self.get_capillary_pressure(self.sat_water[*n_id]);

                        let rho_w_i = self.get_rho_w(p_i);
                        let rho_w_j = self.get_rho_w(p_j);
                        let grav_w = self.gravity_head_bar(
                            depth_i,
                            depth_j,
                            self.interface_density_barrier(rho_w_i, rho_w_j),
                        );
                        let rho_o_i = if self.three_phase_mode {
                            self.get_rho_o_cell(id, p_i)
                        } else {
                            self.get_rho_o(p_i)
                        };
                        let rho_o_j = if self.three_phase_mode {
                            self.get_rho_o_cell(*n_id, p_j)
                        } else {
                            self.get_rho_o(p_j)
                        };
                        let grav_o = self.gravity_head_bar(
                            depth_i,
                            depth_j,
                            self.interface_density_barrier(rho_o_i, rho_o_j),
                        );

                        // Phase potential differences (positive if flowing from i to j)
                        let dphi_o = (p_i - p_j) - grav_o;
                        let dphi_w = (p_i - p_j) - (pc_i - pc_j) - grav_w;

                        let geom_t =
                            DARCY_METRIC_FACTOR * self.geometric_transmissibility(id, *n_id, *dim);

                        let t_total;
                        let explicit_rhs;
                        if self.three_phase_mode {
                            let (lam_w_i, lam_o_i, lam_g_i) = self.phase_mobilities_3p(id);
                            let (lam_w_j, lam_o_j, lam_g_j) = self.phase_mobilities_3p(*n_id);

                            let lam_o_up = if dphi_o >= 0.0 { lam_o_i } else { lam_o_j };
                            let lam_w_up = if dphi_w >= 0.0 { lam_w_i } else { lam_w_j };

                            // Gas potential: P_gas = P_oil + P_cog => dphi_g = dphi_oil + d(P_cog)
                            let pc_og_i = self.get_gas_oil_capillary_pressure(self.sat_gas[id]);
                            let pc_og_j = self.get_gas_oil_capillary_pressure(self.sat_gas[*n_id]);
                            let rho_g_i = self.get_rho_g(p_i);
                            let rho_g_j = self.get_rho_g(p_j);
                            let grav_g = self.gravity_head_bar(
                                depth_i,
                                depth_j,
                                self.interface_density_barrier(rho_g_i, rho_g_j),
                            );
                            let dphi_g = (p_i - p_j) + (pc_og_i - pc_og_j) - grav_g;
                            let lam_g_up = if dphi_g >= 0.0 { lam_g_i } else { lam_g_j };

                            let t_o = geom_t * lam_o_up;
                            let t_w = geom_t * lam_w_up;
                            let t_g = geom_t * lam_g_up;
                            t_total = t_o + t_w + t_g;
                            explicit_rhs = t_o * grav_o
                                + t_w * (pc_i - pc_j + grav_w)
                                - t_g * (pc_og_i - pc_og_j - grav_g);
                        } else {
                            let (lam_w_i, lam_o_i) = self.phase_mobilities(id);
                            let (lam_w_j, lam_o_j) = self.phase_mobilities(*n_id);

                            let lam_o_up = if dphi_o >= 0.0 { lam_o_i } else { lam_o_j };
                            let lam_w_up = if dphi_w >= 0.0 { lam_w_i } else { lam_w_j };

                            let t_o = geom_t * lam_o_up;
                            let t_w = geom_t * lam_w_up;
                            t_total = t_o + t_w;
                            explicit_rhs = t_o * grav_o + t_w * (pc_i - pc_j + grav_w);
                        }

                        diag += t_total;
                        rows.push(id);
                        cols.push(*n_id);
                        vals.push(-t_total);

                        // Explicit terms: gravity and capillary forces
                        b_rhs[id] += explicit_rhs;
                    }

                    // Well source terms (use pre-computed well_controls for consistency)
                    for (w_idx, w) in self.wells.iter().enumerate() {
                        if w.i == i && w.j == j && w.k == k {
                            if let Some(ref control) = well_controls[w_idx] {
                                match &control.decision {
                                    WellControlDecision::Disabled => {}
                                    WellControlDecision::Rate { q_m3_day } => {
                                        b_rhs[id] -= q_m3_day;
                                    }
                                    WellControlDecision::Bhp { bhp_bar } => {
                                        // BHP-controlled: add PI to diagonal and PI*BHP to RHS
                                        // q [m³/day] = PI [m³/day/bar] * (p_cell - BHP)
                                        if w.productivity_index.is_finite() && bhp_bar.is_finite() {
                                            diag += w.productivity_index;
                                            b_rhs[id] += w.productivity_index * *bhp_bar;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // push diagonal to matrix
                    rows.push(id);
                    cols.push(id);
                    vals.push(diag);
                    // Safe inverse for Jacobi preconditioner
                    diag_inv[id] = if diag.abs() > f64::EPSILON {
                        1.0 / diag
                    } else {
                        1.0
                    };
                }
            }
        }

        // Build sparse matrix and solve pressure equation
        let mut tri = TriMatI::<f64, usize>::new((n_cells, n_cells));
        for idx in 0..vals.len() {
            tri.add_triplet(rows[idx], cols[idx], vals[idx]);
        }
        let a_mat: CsMat<f64> = tri.to_csr();

        // Solve pressure equation A*p_new = b with BiCGSTAB, initial guess = current pressures
        let mut x0 = DVector::<f64>::zeros(n_cells);
        for i in 0..n_cells {
            x0[i] = self.pressure[i];
        }
        let pcg_result = solve_bicgstab_with_guess(&a_mat, &b_rhs, &diag_inv, &x0, 1e-7, 1000);
        let p_new = pcg_result.solution;

        let mut delta_water_m3 = vec![0.0f64; n_cells];
        let mut delta_free_gas_sc = vec![0.0f64; n_cells];
        let mut delta_dg_sc = vec![0.0f64; n_cells];

        // Compute phase fluxes and explicit saturation update (upwind fractional flow method)
        // Track water in reservoir m³ and gas in standard m³ over dt_days.
        let mut max_sat_change = 0.0;

        // Interface fluxes: compute once per neighbor pair and distribute upwind.
        // IMPES consistency: upwind direction from OLD pressure potentials (same as
        // pressure equation), flux magnitude from NEW pressure gradient.
        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i, j, k);

                    // Check neighbors in positive direction to avoid duplicate pairs
                    let mut check = Vec::new();
                    if i < self.nx - 1 {
                        check.push((self.idx(i + 1, j, k), 'x', k));
                    }
                    if j < self.ny - 1 {
                        check.push((self.idx(i, j + 1, k), 'y', k));
                    }
                    if k < self.nz - 1 {
                        check.push((self.idx(i, j, k + 1), 'z', k + 1));
                    }

                    for (nid, dim, n_k) in check {
                        let depth_i = self.depth_at_k(k);
                        let depth_j = self.depth_at_k(n_k);

                        let pc_i = self.get_capillary_pressure(self.sat_water[id]);
                        let pc_j = self.get_capillary_pressure(self.sat_water[nid]);

                        let rho_w_old_i = self.get_rho_w(self.pressure[id]);
                        let rho_w_old_j = self.get_rho_w(self.pressure[nid]);
                        let rho_w_new_i = self.get_rho_w(p_new[id]);
                        let rho_w_new_j = self.get_rho_w(p_new[nid]);
                        let grav_w_old = self.gravity_head_bar(
                            depth_i,
                            depth_j,
                            self.interface_density_barrier(rho_w_old_i, rho_w_old_j),
                        );
                        let grav_w_new = self.gravity_head_bar(
                            depth_i,
                            depth_j,
                            self.interface_density_barrier(rho_w_new_i, rho_w_new_j),
                        );

                        // Old-pressure potential for upwind direction (matches pressure equation)
                        let dphi_w_old = (self.pressure[id] - self.pressure[nid]) - (pc_i - pc_j) - grav_w_old;
                        // New-pressure potential for flux magnitude
                        let dphi_w = (p_new[id] - p_new[nid]) - (pc_i - pc_j) - grav_w_new;

                        let (lam_w_i, lam_w_j) = if self.three_phase_mode {
                            let (w_i, _, _) = self.phase_mobilities_3p(id);
                            let (w_j, _, _) = self.phase_mobilities_3p(nid);
                            (w_i, w_j)
                        } else {
                            let (w_i, _) = self.phase_mobilities(id);
                            let (w_j, _) = self.phase_mobilities(nid);
                            (w_i, w_j)
                        };

                        let lam_w_up = if self.three_phase_mode {
                            if dphi_w_old >= 0.0 { lam_w_i } else { lam_w_j }
                        } else {
                            if dphi_w_old >= 0.0 { lam_w_i } else { lam_w_j }
                        };

                        let geom_t =
                            DARCY_METRIC_FACTOR * self.geometric_transmissibility(id, nid, dim);
                        let t_w = geom_t * lam_w_up;

                        // Water flux [m³/day]
                        let water_flux_m3_day = t_w * dphi_w;
                        // Volume change over dt_days [m³]
                        let dv_water = water_flux_m3_day * dt_days;

                        // Distribute: outgoing flow reduces water in source cell
                        delta_water_m3[id] -= dv_water;
                        delta_water_m3[nid] += dv_water;
                    }
                }
            }
        }

        // Gas flux loop (three-phase only)
        // IMPES consistency: upwind from OLD pressure potentials, flux from NEW.
        if self.three_phase_mode {
            for k in 0..self.nz {
                for j in 0..self.ny {
                    for i in 0..self.nx {
                        let id = self.idx(i, j, k);

                        let mut check = Vec::new();
                        if i < self.nx - 1 {
                            check.push((self.idx(i + 1, j, k), 'x', k));
                        }
                        if j < self.ny - 1 {
                            check.push((self.idx(i, j + 1, k), 'y', k));
                        }
                        if k < self.nz - 1 {
                            check.push((self.idx(i, j, k + 1), 'z', k + 1));
                        }

                        for (nid, dim, n_k) in check {
                            let depth_i = self.depth_at_k(k);
                            let depth_j = self.depth_at_k(n_k);

                            let pc_og_i = self.get_gas_oil_capillary_pressure(self.sat_gas[id]);
                            let pc_og_j = self.get_gas_oil_capillary_pressure(self.sat_gas[nid]);
                            let rho_g_old_i = self.get_rho_g(self.pressure[id]);
                            let rho_g_old_j = self.get_rho_g(self.pressure[nid]);
                            let rho_g_new_i = self.get_rho_g(p_new[id]);
                            let rho_g_new_j = self.get_rho_g(p_new[nid]);
                            let grav_g_old = self.gravity_head_bar(
                                depth_i,
                                depth_j,
                                self.interface_density_barrier(rho_g_old_i, rho_g_old_j),
                            );
                            let grav_g_new = self.gravity_head_bar(
                                depth_i,
                                depth_j,
                                self.interface_density_barrier(rho_g_new_i, rho_g_new_j),
                            );

                            // Old-pressure potential for upwind direction
                            let dphi_g_old = (self.pressure[id] - self.pressure[nid]) + (pc_og_i - pc_og_j) - grav_g_old;
                            // New-pressure potential for flux magnitude
                            let dphi_g = (p_new[id] - p_new[nid]) + (pc_og_i - pc_og_j) - grav_g_new;

                            let lam_g_up = if dphi_g_old >= 0.0 {
                                self.gas_mobility(id)
                            } else {
                                self.gas_mobility(nid)
                            };

                            let geom_t = DARCY_METRIC_FACTOR
                                * self.geometric_transmissibility(id, nid, dim);
                            let t_g = geom_t * lam_g_up;
                            let gas_flux_m3_day = t_g * dphi_g;
                            let up_id = if dphi_g_old >= 0.0 { id } else { nid };
                            let gas_flux_sc_day = gas_flux_m3_day / self.get_b_g(p_new[up_id]).max(1e-9);
                            let dv_gas_sc = gas_flux_sc_day * dt_days;

                            delta_free_gas_sc[id] -= dv_gas_sc;
                            delta_free_gas_sc[nid] += dv_gas_sc;

                            // Oil flux to advect Dissolved Gas (Rs)
                            if self.pvt_table.is_some() {
                                let rho_o_old_i = self.get_rho_o_cell(id, self.pressure[id]);
                                let rho_o_old_j = self.get_rho_o_cell(nid, self.pressure[nid]);
                                let rho_o_new_i = self.get_rho_o_cell(id, p_new[id]);
                                let rho_o_new_j = self.get_rho_o_cell(nid, p_new[nid]);
                                let grav_o_old = self.gravity_head_bar(
                                    depth_i,
                                    depth_j,
                                    self.interface_density_barrier(rho_o_old_i, rho_o_old_j),
                                );
                                let grav_o_new = self.gravity_head_bar(
                                    depth_i,
                                    depth_j,
                                    self.interface_density_barrier(rho_o_new_i, rho_o_new_j),
                                );
                                // Old-pressure potential for oil upwinding
                                let dphi_o_old = (self.pressure[id] - self.pressure[nid]) - grav_o_old;
                                // New-pressure potential for oil flux
                                let dphi_o = (p_new[id] - p_new[nid]) - grav_o_new;

                                let (_, lam_o_i, _) = self.phase_mobilities_3p(id);
                                let (_, lam_o_j, _) = self.phase_mobilities_3p(nid);
                                let lam_o_up = if dphi_o_old >= 0.0 { lam_o_i } else { lam_o_j };
                                let t_o = geom_t * lam_o_up;

                                let oil_flux_res_day = t_o * dphi_o;
                                let up_id = if dphi_o_old >= 0.0 { id } else { nid };
                                let oil_flux_sc_day =
                                    oil_flux_res_day / self.get_b_o_cell(up_id, p_new[up_id]).max(1e-9);

                                let rs_upwind = if dphi_o_old >= 0.0 { self.rs[id] } else { self.rs[nid] };
                                let dg_flux_sc_day = oil_flux_sc_day * rs_upwind;
                                let dv_dg_sc = dg_flux_sc_day * dt_days;

                                delta_dg_sc[id] -= dv_dg_sc;
                                delta_dg_sc[nid] += dv_dg_sc;
                            }
                        }
                    }
                }
            }
        }

        for (w_idx, w) in self.wells.iter().enumerate() {
            let id = self.idx(w.i, w.j, w.k);

            let q_m3_day = well_controls[w_idx]
                .and_then(|control| self.well_transport_rate_from_control(w, control, p_new[id]));
            if let Some(q_m3_day) = q_m3_day {
                if self.three_phase_mode {
                    let (fw, fg, fo) = if w.injector {
                        match self.injected_fluid {
                            InjectedFluid::Water => (1.0, 0.0, 0.0),
                            InjectedFluid::Gas => (0.0, 1.0, 0.0),
                        }
                    } else {
                        let lam_t = self.total_mobility_3p(id).max(f64::EPSILON);
                        let (lam_w, lam_o, lam_g) = self.phase_mobilities_3p(id);
                        (lam_w / lam_t, lam_g / lam_t, lam_o / lam_t)
                    };
                    delta_water_m3[id] -= q_m3_day * fw * dt_days;
                    delta_free_gas_sc[id] -=
                        q_m3_day * fg * dt_days / self.get_b_g(p_new[id]).max(1e-9);

                    if !w.injector && self.pvt_table.is_some() {
                        let q_o_res = q_m3_day * fo;
                        let q_o_sc = q_o_res / self.get_b_o_cell(id, p_new[id]);
                        let q_dg_sc = q_o_sc * self.rs[id];
                        delta_dg_sc[id] -= q_dg_sc * dt_days;
                    }
                } else {
                    let fw = if w.injector { 1.0 } else { self.frac_flow_water(id) };
                    delta_water_m3[id] -= q_m3_day * fw * dt_days;
                }
            }
        }

        // Calculate max saturation change for CFL condition (water + gas)
        for idx in 0..n_cells {
            let vp_m3 = self.pore_volume_m3(idx);
            if vp_m3 > 0.0 {
                let sat_change_w = (delta_water_m3[idx] / vp_m3).abs();
                let sat_change_g = if self.three_phase_mode {
                    (delta_free_gas_sc[idx].abs() * self.get_b_g(self.pressure[idx]).max(1e-9) / vp_m3).abs()
                } else {
                    0.0
                };
                let sat_change = sat_change_w.max(sat_change_g);
                if sat_change > max_sat_change {
                    max_sat_change = sat_change;
                }
            }
        }

        let sat_factor = if max_sat_change > self.max_sat_change_per_step {
            self.max_sat_change_per_step / max_sat_change
        } else {
            1.0
        };

        let mut max_pressure_change = 0.0;
        for idx in 0..n_cells {
            let dp = (p_new[idx] - self.pressure[idx]).abs();
            if dp > max_pressure_change {
                max_pressure_change = dp;
            }
        }
        let pressure_factor = if max_pressure_change > self.max_pressure_change_per_step {
            self.max_pressure_change_per_step / max_pressure_change
        } else {
            1.0
        };

        let mut max_well_rate_rel_change = 0.0;
        for w in &self.wells {
            let q_old = self.well_rate_m3_day_for_pressures(w, &self.pressure).unwrap_or(0.0);
            let q_new = self.well_rate_m3_day_for_pressures(w, p_new.as_slice()).unwrap_or(0.0);

            let rel = (q_new - q_old).abs() / (q_old.abs() + 1.0);
            if rel > max_well_rate_rel_change {
                max_well_rate_rel_change = rel;
            }
        }
        let rate_factor = if max_well_rate_rel_change > self.max_well_rate_change_fraction {
            self.max_well_rate_change_fraction / max_well_rate_rel_change
        } else {
            1.0
        };

        let stable_dt_factor = sat_factor
            .min(pressure_factor)
            .min(rate_factor)
            .clamp(0.01, 1.0);

        (
            p_new,
            delta_water_m3,
            delta_free_gas_sc,
            delta_dg_sc,
            well_controls,
            stable_dt_factor,
            pcg_result.converged,
            pcg_result.iterations,
        )
    }

    #[cfg(test)]
    pub(crate) fn debug_calculate_fluxes(
        &self,
        delta_t_days: f64,
    ) -> (DVector<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<Option<ResolvedWellControl>>, f64, bool, usize) {
        self.calculate_fluxes(delta_t_days)
    }

    pub(crate) fn update_saturations_and_pressure(
        &mut self,
        p_new: &DVector<f64>,
        delta_water_m3: &Vec<f64>,
        delta_free_gas_sc: &Vec<f64>,
        delta_dg_sc: &Vec<f64>,
        well_controls: &[Option<ResolvedWellControl>],
        dt_days: f64,
    ) {
        let n_cells = self.nx * self.ny * self.nz;
        // Apply saturation updates with physical clipping
        let mut actual_change_m3 = 0.0;
        let mut actual_change_gas_sc = 0.0;
        for idx in 0..n_cells {
            let vp_m3 = self.pore_volume_m3(idx);
            if vp_m3 <= 0.0 {
                continue;
            }

            let sw_old = self.sat_water[idx];
            let delta_sw = delta_water_m3[idx] / vp_m3;

            if self.three_phase_mode {
                // Three-phase: update water and gas, derive oil by material balance
                let sg_old = self.sat_gas[idx];
                let so_old = self.sat_oil[idx];
                let p_old = self.pressure[idx];
                let bg_old = self.get_b_g(p_old).max(1e-9);
                let bo_old = self.get_b_o_cell(idx, p_old).max(1e-9);
                let rs_old = self.rs[idx];
                let old_free_gas_sc = sg_old * vp_m3 / bg_old;
                let old_dissolved_gas_sc = if self.pvt_table.is_some() {
                    (so_old * vp_m3 / bo_old) * rs_old
                } else {
                    0.0
                };

                let (s_wc, s_or, s_gc, s_gr) =
                    if let Some(s) = &self.scal_3p {
                        (s.s_wc, s.s_or, s.s_gc, s.s_gr)
                    } else {
                        (self.scal.s_wc, self.scal.s_or, 0.0, 0.0)
                    };

                let sw_new = (sw_old + delta_sw).clamp(s_wc, 1.0 - s_or - s_gc);
                let bg_new = self.get_b_g(p_new[idx]).max(1e-9);
                let transported_free_gas_sc = (old_free_gas_sc + delta_free_gas_sc[idx]).max(0.0);
                let mut sg_new = ((transported_free_gas_sc * bg_new) / vp_m3)
                    .clamp(0.0, 1.0 - s_wc - s_gr);

                // --- Phase split: conservative local gas flash ---
                if self.pvt_table.is_some() {
                    let dissolved_gas_sc_transport = (old_dissolved_gas_sc + delta_dg_sc[idx]).max(0.0);
                    let (sg_resolved, _so_resolved, rs_cell) = self.split_gas_inventory_after_transport(
                        p_new[idx],
                        vp_m3,
                        sw_new,
                        transported_free_gas_sc,
                        dissolved_gas_sc_transport,
                    );
                    sg_new = sg_resolved;
                    self.rs[idx] = rs_cell;
                }

                self.sat_water[idx] = sw_new;
                self.sat_gas[idx] = sg_new.clamp(0.0, (1.0 - sw_new).max(0.0));
                self.sat_oil[idx] = (1.0 - self.sat_water[idx] - self.sat_gas[idx]).max(0.0);

                actual_change_m3 += (self.sat_water[idx] - sw_old) * vp_m3;
                let bo_new = self.get_b_o_cell(idx, p_new[idx]).max(1e-9);
                let new_free_gas_sc = self.sat_gas[idx] * vp_m3 / bg_new;
                let new_dissolved_gas_sc = if self.pvt_table.is_some() {
                    (self.sat_oil[idx] * vp_m3 / bo_new) * self.rs[idx]
                } else {
                    0.0
                };
                actual_change_gas_sc +=
                    (new_free_gas_sc + new_dissolved_gas_sc) - (old_free_gas_sc + old_dissolved_gas_sc);
            } else {
                // Two-phase: material balance s_w + s_o = 1
                let sw_min = self.scal.s_wc;
                let sw_max = 1.0 - self.scal.s_or;
                let sw_new = (sw_old + delta_sw).clamp(sw_min, sw_max);

                actual_change_m3 += (sw_new - sw_old) * vp_m3;
                self.sat_water[idx] = sw_new;
                self.sat_oil[idx] = 1.0 - sw_new;
            }

            self.pressure[idx] = p_new[idx];
        }

        // Calculate and store rates
        let mut total_prod_oil = 0.0;
        let mut total_prod_liquid = 0.0;
        let mut total_prod_liquid_reservoir = 0.0;
        let mut total_injection = 0.0;
        let mut total_injection_reservoir = 0.0;
        let mut total_water_injection_reservoir = 0.0;
        let mut total_prod_water_reservoir = 0.0;
        let mut total_prod_gas = 0.0;
        let mut total_prod_dissolved_gas = 0.0;
        let mut total_gas_injection_sc = 0.0;
        let mut producer_rate_controlled_wells = 0usize;
        let mut injector_rate_controlled_wells = 0usize;
        let mut producer_bhp_limited_wells = 0usize;
        let mut injector_bhp_limited_wells = 0usize;

        for (w_idx, w) in self.wells.iter().enumerate() {
            let id = self.idx(w.i, w.j, w.k);
            if let Some(control) = well_controls.get(w_idx).and_then(|control| *control) {
                let group_rate_controlled = if w.injector {
                    self.injector_rate_controlled
                } else {
                    self.producer_rate_controlled
                };
                if group_rate_controlled {
                    if w.injector {
                        injector_rate_controlled_wells += 1;
                        if control.bhp_limited {
                            injector_bhp_limited_wells += 1;
                        }
                    } else {
                        producer_rate_controlled_wells += 1;
                        if control.bhp_limited {
                            producer_bhp_limited_wells += 1;
                        }
                    }
                }

                let q_m3_day = match self.well_transport_rate_from_control(w, control, p_new[id]) {
                    Some(q_m3_day) if q_m3_day.is_finite() => q_m3_day,
                    _ => continue,
                };
                let p_cell = p_new[id];
                if w.injector {
                    total_injection_reservoir += -q_m3_day;
                    if self.three_phase_mode {
                        match self.injected_fluid {
                            InjectedFluid::Water => {
                                if matches!(control.decision, WellControlDecision::Rate { .. }) {
                                    if let Some(surface_target_sc_day) = self.target_injector_surface_rate_m3_day {
                                        total_injection += surface_target_sc_day / self.injector_well_count().max(1) as f64;
                                    } else {
                                        total_injection += -q_m3_day / self.b_w.max(1e-9);
                                    }
                                } else {
                                    total_injection += -q_m3_day / self.b_w.max(1e-9);
                                }
                                total_water_injection_reservoir += -q_m3_day;
                            }
                            InjectedFluid::Gas => {
                                // Always compute SC rate from the actual reservoir rate
                                // and Bg at p_new, matching the saturation transport.
                                let bg = self.get_b_g(p_cell).max(1e-9);
                                total_injection += -q_m3_day / bg;
                                total_gas_injection_sc += -q_m3_day / bg;
                            }
                        }
                    } else {
                        if matches!(control.decision, WellControlDecision::Rate { .. }) {
                            if let Some(surface_target_sc_day) = self.target_injector_surface_rate_m3_day {
                                total_injection += surface_target_sc_day / self.injector_well_count().max(1) as f64;
                            } else {
                                total_injection += -q_m3_day / self.b_w.max(1e-9);
                            }
                        } else {
                            total_injection += -q_m3_day / self.b_w.max(1e-9);
                        }
                        total_water_injection_reservoir += -q_m3_day;
                    }
                } else {
                    total_prod_liquid_reservoir += q_m3_day;
                    let (fw, fg) = if self.three_phase_mode {
                        (self.frac_flow_water_3p(id), self.frac_flow_gas(id))
                    } else {
                        (self.frac_flow_water(id), 0.0)
                    };
                    total_prod_water_reservoir += q_m3_day * fw;
                    // Surface rates: divide reservoir volumes by pressure-dependent FVFs
                    let bo = self.get_b_o_cell(id, p_cell).max(1e-9);
                    let bw = self.b_w.max(1e-9); // Bw essentially constant in black-oil
                    let oil_rate_sc = q_m3_day * (1.0 - fw - fg) / bo;
                    let water_rate_sc = q_m3_day * fw / bw;
                    total_prod_oil += oil_rate_sc;
                    total_prod_liquid += oil_rate_sc + water_rate_sc;
                    // Free gas at surface: reservoir gas rate / Bg
                    let bg = self.get_b_g(p_cell).max(1e-9);
                    total_prod_gas += q_m3_day * fg / bg;
                    // Dissolved gas liberated at surface: oil_sc × Rs
                    if self.pvt_table.is_some() && self.three_phase_mode {
                        total_prod_dissolved_gas += oil_rate_sc * self.rs[id];
                    }
                }
            }
        }

        // Update cumulative water volumes in reservoir conditions for material balance
        self.cumulative_injection_m3 += total_water_injection_reservoir * dt_days;
        self.cumulative_production_m3 += total_prod_water_reservoir * dt_days;

        // Water material balance: (water injected − water produced) should equal ΔSw × Vp
        let net_water_added_m3 = (total_water_injection_reservoir - total_prod_water_reservoir) * dt_days;
        self.cumulative_mb_error_m3 += net_water_added_m3 - actual_change_m3;

        // Gas material balance (three-phase only):
        // (surface gas injected − surface gas produced) should equal the change in total
        // in-place gas inventory expressed at standard conditions (free gas + dissolved gas).
        if self.three_phase_mode {
            let total_gas_prod_sc = total_prod_gas + total_prod_dissolved_gas;
            let net_gas_added_sc = (total_gas_injection_sc - total_gas_prod_sc) * dt_days;
            self.cumulative_mb_gas_error_m3 += net_gas_added_sc - actual_change_gas_sc;
        }

        // Report the absolute cumulative error
        let mb_error = self.cumulative_mb_error_m3.abs();

        let mut sum_sat_water = 0.0;
        let mut sum_sat_gas = 0.0;
        for i in 0..self.nx * self.ny * self.nz {
            sum_sat_water += self.sat_water[i];
            sum_sat_gas += self.sat_gas[i];
        }
        let avg_reservoir_pressure = self.average_reservoir_pressure_pv_weighted();
        let avg_water_saturation = if n_cells > 0 {
            sum_sat_water / (n_cells as f64)
        } else {
            0.0
        };
        let avg_gas_saturation = if n_cells > 0 {
            sum_sat_gas / (n_cells as f64)
        } else {
            0.0
        };

        // Producing GOR [Sm³/Sm³] = (free gas SC + dissolved gas SC) / oil SC
        let total_gas_sc = total_prod_gas + total_prod_dissolved_gas;
        let producing_gor = if total_prod_oil > MIN_GOR_OIL_RATE_SC_DAY {
            total_gas_sc / total_prod_oil
        } else {
            0.0
        };
        let producer_bhp_limited_fraction = if producer_rate_controlled_wells > 0 {
            producer_bhp_limited_wells as f64 / producer_rate_controlled_wells as f64
        } else {
            0.0
        };
        let injector_bhp_limited_fraction = if injector_rate_controlled_wells > 0 {
            injector_bhp_limited_wells as f64 / injector_rate_controlled_wells as f64
        } else {
            0.0
        };

        self.rate_history.push(TimePointRates {
            time: self.time_days + dt_days,
            total_production_oil: total_prod_oil,
            total_production_liquid: total_prod_liquid,
            total_production_liquid_reservoir: total_prod_liquid_reservoir,
            total_injection: total_injection,
            total_injection_reservoir: total_injection_reservoir,
            material_balance_error_m3: mb_error,
            material_balance_error_gas_m3: self.cumulative_mb_gas_error_m3.abs(),
            avg_reservoir_pressure,
            avg_water_saturation,
            total_production_gas: total_gas_sc,
            avg_gas_saturation,
            producing_gor,
            producer_bhp_limited_fraction,
            injector_bhp_limited_fraction,
        });

        // Advance simulation time
        self.time_days += dt_days;
    }
}
