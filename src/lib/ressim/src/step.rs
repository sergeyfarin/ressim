use nalgebra::DVector;
use sprs::{CsMat, TriMatI};
use std::f64;

use crate::solver::solve_pcg_with_guess;
use crate::{InjectedFluid, ReservoirSimulator, TimePointRates, Well};

/// Conversion factor from mD·m²/(m·cP) to m³/day/bar.
/// Derivation: 1 mD = 9.8692e-16 m², 1 cP = 1e-3 Pa·s, 1 bar = 1e5 Pa, 1 day = 86400 s
/// Factor = 9.8692e-16 × 1e3 × 1e5 × 86400 = 8.5269888e-3
const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;

pub(crate) enum WellControlDecision {
    Disabled,
    Rate { q_m3_day: f64 },
    Bhp { bhp_bar: f64 },
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
            + s.k_ro_stone2(sw, sg) / self.get_mu_o(self.pressure[id])
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
            s.k_ro_stone2(sw, sg) / self.get_mu_o(self.pressure[id]),
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
        let mut substeps = 0;
        self.last_solver_warning = String::new();

        while time_stepped < target_dt_days && substeps < MAX_SUBSTEPS {
            let remaining_dt = target_dt_days - time_stepped;

            // Dynamic PI update with latest local saturation/mobility before pressure solve
            self.update_dynamic_well_productivity_indices();

            // Calculate fluxes and stability for the remaining time step
            let (p_new, delta_water_m3, delta_gas_m3, delta_dg_sc, stable_dt_factor, pcg_converged, pcg_iters) =
                self.calculate_fluxes(remaining_dt);

            // Track solver convergence warning
            if !pcg_converged {
                self.last_solver_warning = format!(
                    "PCG solver did not converge after {} iterations (t={:.2} days)",
                    pcg_iters,
                    self.time_days + time_stepped
                );
            }

            let actual_dt;
            let final_p;
            let final_delta_water_m3;
            let final_delta_gas_m3;
            let final_delta_dg_sc;

            if stable_dt_factor < 1.0 {
                // Timestep is too large, reduce it based on CFL condition
                actual_dt = remaining_dt * stable_dt_factor * 0.9; // Use 90% for safety

                // Re-solve pressure and fluxes with reduced dt for accuracy.
                let (p_resolv, dw_resolv, dg_resolv, ddg_resolv, _factor2, pcg_conv2, pcg_iters2) =
                    self.calculate_fluxes(actual_dt);
                final_p = p_resolv;
                final_delta_water_m3 = dw_resolv;
                final_delta_gas_m3 = dg_resolv;
                final_delta_dg_sc = ddg_resolv;

                if !pcg_conv2 {
                    self.last_solver_warning = format!(
                        "PCG solver did not converge after {} iterations (re-solve, t={:.2} days)",
                        pcg_iters2,
                        self.time_days + time_stepped
                    );
                }
            } else {
                // The full remaining timestep is stable
                actual_dt = remaining_dt;
                final_p = p_new;
                final_delta_water_m3 = delta_water_m3;
                final_delta_gas_m3 = delta_gas_m3;
                final_delta_dg_sc = delta_dg_sc;
            }

            if !actual_dt.is_finite() || actual_dt <= 1e-12 {
                self.last_solver_warning = format!(
                    "Adaptive timestep collapsed to non-physical dt={} at t={:.6} days",
                    actual_dt,
                    self.time_days + time_stepped
                );
                break;
            }

            // Update saturations and pressure with the adjusted (or full) timestep
            self.update_saturations_and_pressure(
                &final_p,
                &final_delta_water_m3,
                &final_delta_gas_m3,
                &final_delta_dg_sc,
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

    fn injector_well_count(&self) -> usize {
        self.wells.iter().filter(|w| w.injector).count()
    }

    fn producer_well_count(&self) -> usize {
        self.wells.iter().filter(|w| !w.injector).count()
    }

    fn target_rate_m3_day(&self, well: &Well) -> Option<f64> {
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
                return Some(-(self.target_injector_rate_m3_day / n_inj as f64));
            }

            let n_prod = self.producer_well_count();
            if n_prod == 0 {
                return Some(0.0);
            }
            return Some(self.target_producer_rate_m3_day / n_prod as f64);
        }

        None
    }

    pub(crate) fn resolve_well_control(
        &self,
        well: &Well,
        pressure_bar: f64,
    ) -> Option<WellControlDecision> {
        if well.injector && !self.injector_enabled {
            return Some(WellControlDecision::Disabled);
        }

        let use_rate_control = if well.injector {
            self.injector_rate_controlled
        } else {
            self.producer_rate_controlled
        };

        if use_rate_control {
            let q_target = self.target_rate_m3_day(well)?;

            if !well.productivity_index.is_finite()
                || !well.bhp.is_finite()
                || !pressure_bar.is_finite()
            {
                return None;
            }

            if well.productivity_index <= f64::EPSILON {
                return Some(WellControlDecision::Rate { q_m3_day: q_target });
            }

            // Rate target implies a dynamic BHP. If it violates limits, switch to BHP-control.
            let implied_bhp = pressure_bar - (q_target / well.productivity_index);
            let constrained_bhp = implied_bhp.clamp(self.well_bhp_min, self.well_bhp_max);

            if (constrained_bhp - implied_bhp).abs() > 1e-9 {
                return Some(WellControlDecision::Bhp {
                    bhp_bar: constrained_bhp,
                });
            }

            return Some(WellControlDecision::Rate { q_m3_day: q_target });
        }

        if !well.productivity_index.is_finite()
            || !well.bhp.is_finite()
            || !pressure_bar.is_finite()
        {
            return None;
        }

        Some(WellControlDecision::Bhp { bhp_bar: well.bhp })
    }

    pub(crate) fn well_rate_m3_day(&self, well: &Well, pressure_bar: f64) -> Option<f64> {
        match self.resolve_well_control(well, pressure_bar)? {
            WellControlDecision::Disabled => Some(0.0),
            WellControlDecision::Rate { q_m3_day } => Some(q_m3_day),
            WellControlDecision::Bhp { bhp_bar } => {
                let q_m3_day = well.productivity_index * (pressure_bar - bhp_bar);
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
    ) -> (DVector<f64>, Vec<f64>, Vec<f64>, Vec<f64>, f64, bool, usize) {
        let n_cells = self.nx * self.ny * self.nz;
        if n_cells == 0 {
            return (DVector::zeros(0), vec![], vec![], vec![], 1.0, true, 0);
        }
        let dt_days = delta_t_days.max(1e-12);

        // Build triplet lists for A matrix and RHS b of pressure equation
        let mut rows: Vec<usize> = Vec::with_capacity(n_cells * 7);
        let mut cols: Vec<usize> = Vec::with_capacity(n_cells * 7);
        let mut vals: Vec<f64> = Vec::with_capacity(n_cells * 7);

        let mut b_rhs = DVector::<f64>::zeros(n_cells);
        let mut diag_inv = DVector::<f64>::zeros(n_cells);

        // Assemble pressure equation: accumulation + transmissibility + well terms
        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i, j, k);

                    // Pore volume [m³]
                    let vp_m3 = self.pore_volume_m3(i);

                    // Per-cell total compressibility [1/bar]
                    // c_t = (c_o * S_o + c_w * S_w [+ c_g * S_g]) + c_r
                    let sg_id = self.sat_gas[id];
                    let so_id = if self.three_phase_mode {
                        (1.0 - self.sat_water[id] - sg_id).max(0.0)
                    } else {
                        self.sat_oil[id]
                    };
                    let c_t = (self.get_c_o(self.pressure[id]) * so_id
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

                        let grav_w = self.gravity_head_bar(depth_i, depth_j, self.get_rho_w(self.pressure[id]));
                        let grav_o = self.gravity_head_bar(depth_i, depth_j, self.get_rho_o(self.pressure[id]));

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
                            let grav_g = self.gravity_head_bar(depth_i, depth_j, self.rho_g);
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

                    // Well source terms
                    for w in &self.wells {
                        if w.i == i && w.j == j && w.k == k {
                            if let Some(control) = self.resolve_well_control(w, self.pressure[id]) {
                                match control {
                                    WellControlDecision::Disabled => {}
                                    WellControlDecision::Rate { q_m3_day } => {
                                        b_rhs[id] -= q_m3_day;
                                    }
                                    WellControlDecision::Bhp { bhp_bar } => {
                                        // BHP-controlled: add PI to diagonal and PI*BHP to RHS
                                        // q [m³/day] = PI [m³/day/bar] * (p_cell - BHP)
                                        if w.productivity_index.is_finite() && bhp_bar.is_finite() {
                                            diag += w.productivity_index;
                                            b_rhs[id] += w.productivity_index * bhp_bar;
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

        // Solve pressure equation A*p_new = b with PCG, initial guess = current pressures
        let mut x0 = DVector::<f64>::zeros(n_cells);
        for i in 0..n_cells {
            x0[i] = self.pressure[i];
        }
        let pcg_result = solve_pcg_with_guess(&a_mat, &b_rhs, &diag_inv, &x0, 1e-7, 1000);
        let p_new = pcg_result.solution;

        // Compute phase fluxes and explicit saturation update (upwind fractional flow method)
        // Track total water and gas volume change [m³] per cell over dt_days
        let mut delta_water_m3 = vec![0.0f64; n_cells];
        let mut delta_gas_m3 = vec![0.0f64; n_cells];
        let mut delta_dg_sc = vec![0.0f64; n_cells];
        let mut max_sat_change = 0.0;

        // Interface fluxes: compute once per neighbor pair and distribute upwind
        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i, j, k);
                    let p_i = p_new[id];

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
                        let p_j = p_new[nid];

                        let depth_i = self.depth_at_k(k);
                        let depth_j = self.depth_at_k(n_k);

                        let pc_i = self.get_capillary_pressure(self.sat_water[id]);
                        let pc_j = self.get_capillary_pressure(self.sat_water[nid]);

                        let grav_w = self.gravity_head_bar(depth_i, depth_j, self.get_rho_w(self.pressure[id]));

                        let dphi_w = (p_i - p_j) - (pc_i - pc_j) - grav_w;

                        let (lam_w_i, _) = self.phase_mobilities(id);
                        let (lam_w_j, _) = self.phase_mobilities(nid);

                        let lam_w_up = if dphi_w >= 0.0 { lam_w_i } else { lam_w_j };

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
        if self.three_phase_mode {
            for k in 0..self.nz {
                for j in 0..self.ny {
                    for i in 0..self.nx {
                        let id = self.idx(i, j, k);
                        let p_i = p_new[id];

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
                            let p_j = p_new[nid];
                            let depth_i = self.depth_at_k(k);
                            let depth_j = self.depth_at_k(n_k);

                            let pc_og_i = self.get_gas_oil_capillary_pressure(self.sat_gas[id]);
                            let pc_og_j = self.get_gas_oil_capillary_pressure(self.sat_gas[nid]);
                            let grav_g = self.gravity_head_bar(depth_i, depth_j, self.rho_g);
                            let dphi_g = (p_i - p_j) + (pc_og_i - pc_og_j) - grav_g;

                            let lam_g_up = if dphi_g >= 0.0 {
                                self.gas_mobility(id)
                            } else {
                                self.gas_mobility(nid)
                            };

                            let geom_t = DARCY_METRIC_FACTOR
                                * self.geometric_transmissibility(id, nid, dim);
                            let t_g = geom_t * lam_g_up;
                            let gas_flux_m3_day = t_g * dphi_g;
                            let dv_gas = gas_flux_m3_day * dt_days;

                            delta_gas_m3[id] -= dv_gas;
                            delta_gas_m3[nid] += dv_gas;

                            // Oil flux to advect Dissolved Gas (Rs)
                            if self.pvt_table.is_some() {
                                let grav_o = self.gravity_head_bar(depth_i, depth_j, self.get_rho_o(self.pressure[id]));
                                let dphi_o = (p_i - p_j) - grav_o;
                                let (_, lam_o_i, _) = self.phase_mobilities_3p(id);
                                let (_, lam_o_j, _) = self.phase_mobilities_3p(nid);
                                let lam_o_up = if dphi_o >= 0.0 { lam_o_i } else { lam_o_j };
                                let t_o = geom_t * lam_o_up;
                                
                                let oil_flux_res_day = t_o * dphi_o;
                                let p_upwind = if dphi_o >= 0.0 { p_i } else { p_j };
                                let oil_flux_sc_day = oil_flux_res_day / self.get_b_o(p_upwind);
                                
                                let rs_upwind = if dphi_o >= 0.0 { self.rs[id] } else { self.rs[nid] };
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

        // Add well explicit contributions using solved pressure
        for w in &self.wells {
            let id = self.idx(w.i, w.j, w.k);

            if let Some(q_m3_day) = self.well_rate_m3_day(w, p_new[id]) {
                if self.three_phase_mode {
                    // Three-phase: injection fluid depends on injectedFluid setting;
                    // producers produce at local fractional flow composition.
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
                    delta_gas_m3[id] -= q_m3_day * fg * dt_days;
                    
                    if !w.injector && self.pvt_table.is_some() {
                        let q_o_res = q_m3_day * fo;
                        let q_o_sc = q_o_res / self.get_b_o(p_new[id]);
                        let q_dg_sc = q_o_sc * self.rs[id];
                        delta_dg_sc[id] -= q_dg_sc * dt_days;
                    }
                } else {
                    // Two-phase: injectors always inject 100% water
                    let fw = if w.injector {
                        1.0
                    } else {
                        self.frac_flow_water(id)
                    };
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
                    (delta_gas_m3[idx] / vp_m3).abs()
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
            let id = self.idx(w.i, w.j, w.k);
            let q_old = self.well_rate_m3_day(w, self.pressure[id]).unwrap_or(0.0);
            let q_new = self.well_rate_m3_day(w, p_new[id]).unwrap_or(0.0);

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
            delta_gas_m3,
            delta_dg_sc,
            stable_dt_factor,
            pcg_result.converged,
            pcg_result.iterations,
        )
    }

    fn update_saturations_and_pressure(
        &mut self,
        p_new: &DVector<f64>,
        delta_water_m3: &Vec<f64>,
        delta_gas_m3: &Vec<f64>,
        delta_dg_sc: &Vec<f64>,
        dt_days: f64,
    ) {
        let n_cells = self.nx * self.ny * self.nz;
        // Apply saturation updates with physical clipping
        let mut actual_change_m3 = 0.0;
        let mut actual_change_gas_m3 = 0.0;
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
                let delta_sg = delta_gas_m3[idx] / vp_m3;

                let (s_wc, s_or, s_gc, s_gr) =
                    if let Some(s) = &self.scal_3p {
                        (s.s_wc, s.s_or, s.s_gc, s.s_gr)
                    } else {
                        (self.scal.s_wc, self.scal.s_or, 0.0, 0.0)
                    };

                let sw_new = (sw_old + delta_sw).clamp(s_wc, 1.0 - s_or - s_gc);
                let mut sg_new = (sg_old + delta_sg).clamp(0.0, 1.0 - s_wc - s_gr);
                let mut so_new = (1.0 - sw_new - sg_new).max(0.0);

                // --- Phase Split: Bubble Point Tracking ---
                if self.pvt_table.is_some() {
                    let mut rs_cell = self.rs[idx];
                    let bo_new = self.get_b_o(p_new[idx]);
                    let bg_new = self.get_b_g(p_new[idx]);
                    let rs_max = self.pvt_table.as_ref().unwrap().interpolate(p_new[idx]).rs_m3m3;

                    // Advect Rs using standard oil volume
                    let so_old = self.sat_oil[idx];
                    let bo_old = self.get_b_o(self.pressure[idx]);
                    let v_o_sc_old = (so_old * vp_m3) / bo_old;
                    let v_o_sc_new = (so_new * vp_m3) / bo_new;
                    let v_dg_sc_old = v_o_sc_old * rs_cell;
                    let v_dg_sc_new = v_dg_sc_old + delta_dg_sc[idx];
                    if v_o_sc_new > 0.0 {
                        rs_cell = v_dg_sc_new / v_o_sc_new;
                    }

                    // Phase Flash
                    if rs_cell > rs_max {
                        // Liberate internal excess dissolved gas
                        let liberated_rs = rs_cell - rs_max;
                        let v_g_liberated_sc = liberated_rs * v_o_sc_new;
                        let v_g_liberated_res = v_g_liberated_sc * bg_new;
                        
                        let delta_sg_liberated = v_g_liberated_res / vp_m3;
                        let effective_delta = delta_sg_liberated.min(so_new); // Prevent negative oil

                        sg_new += effective_delta;
                        so_new -= effective_delta;
                        rs_cell = rs_max;
                    } else if rs_cell < rs_max && sg_new > 0.0 {
                        // Dissolve free gas back into undersaturated oil
                        let capacity_rs = rs_max - rs_cell;
                        let v_g_free_sc = (sg_new * vp_m3) / bg_new;
                        let max_dissolvable_gas_sc = capacity_rs * v_o_sc_new;
                        
                        let dissolved_gas_sc = v_g_free_sc.min(max_dissolvable_gas_sc);
                        let dissolved_gas_res = dissolved_gas_sc * bg_new;
                        let delta_sg_dissolved = dissolved_gas_res / vp_m3;

                        sg_new -= delta_sg_dissolved;
                        so_new += delta_sg_dissolved;
                        if v_o_sc_new > 0.0 {
                            rs_cell += dissolved_gas_sc / v_o_sc_new;
                        }
                    }
                    self.rs[idx] = rs_cell;
                }

                // Re-normalise if floating point causes sum ≠ 1
                let sum = sw_new + so_new + sg_new;
                self.sat_water[idx] = if sum > 0.0 { sw_new / sum } else { sw_new };
                self.sat_oil[idx] = if sum > 0.0 { so_new / sum } else { so_new };
                self.sat_gas[idx] = if sum > 0.0 { sg_new / sum } else { sg_new };

                actual_change_m3 += (self.sat_water[idx] - sw_old) * vp_m3;
                actual_change_gas_m3 += (self.sat_gas[idx] - sg_old) * vp_m3;
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
        let mut total_gas_injection_reservoir = 0.0;
        let mut total_prod_water_reservoir = 0.0;
        let mut total_prod_gas_reservoir = 0.0;
        let mut total_prod_gas = 0.0;
        let mut total_prod_dissolved_gas = 0.0;

        for w in &self.wells {
            let id = self.idx(w.i, w.j, w.k);
            if let Some(q_m3_day) = self.well_rate_m3_day(w, p_new[id]) {
                let p_cell = p_new[id];
                if w.injector {
                    total_injection -= q_m3_day / self.b_w.max(1e-9);
                    total_injection_reservoir += -q_m3_day;
                    if self.three_phase_mode {
                        match self.injected_fluid {
                            InjectedFluid::Water => total_water_injection_reservoir += -q_m3_day,
                            InjectedFluid::Gas => total_gas_injection_reservoir += -q_m3_day,
                        }
                    } else {
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
                    total_prod_gas_reservoir += q_m3_day * fg;
                    // Surface rates: divide reservoir volumes by pressure-dependent FVFs
                    let bo = self.get_b_o(p_cell).max(1e-9);
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

        // Gas material balance (three-phase only): (gas injected − gas produced) should equal ΔSg × Vp
        if self.three_phase_mode {
            let net_gas_added_m3 = (total_gas_injection_reservoir - total_prod_gas_reservoir) * dt_days;
            self.cumulative_mb_gas_error_m3 += net_gas_added_m3 - actual_change_gas_m3;
        }

        // Report the absolute cumulative error
        let mb_error = self.cumulative_mb_error_m3.abs();

        let mut sum_pressure = 0.0;
        let mut sum_sat_water = 0.0;
        let mut sum_sat_gas = 0.0;
        for i in 0..self.nx * self.ny * self.nz {
            sum_pressure += self.pressure[i];
            sum_sat_water += self.sat_water[i];
            sum_sat_gas += self.sat_gas[i];
        }
        let avg_reservoir_pressure = if n_cells > 0 {
            sum_pressure / (n_cells as f64)
        } else {
            0.0
        };
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
        let producing_gor = if total_prod_oil > 1e-12 {
            total_gas_sc / total_prod_oil
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
        });

        // Advance simulation time
        self.time_days += dt_days;
    }
}
