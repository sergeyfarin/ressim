use nalgebra::DVector;
use sprs::{CsMat, TriMatI};
use std::f64;

use crate::solver::solve_pcg_with_guess;
use crate::{GridCell, ReservoirSimulator, TimePointRates, Well};

pub(crate) enum WellControlDecision {
    Disabled,
    Rate { q_m3_day: f64 },
    Bhp { bhp_bar: f64 },
}

impl ReservoirSimulator {
    pub(crate) fn calculate_well_productivity_index(
        &self,
        cell: &GridCell,
        well_radius: f64,
        skin: f64,
    ) -> Result<f64, String> {
        // Calculate equivalent radius (Peaceman's model)
        let kx = cell.perm_x;
        let ky = cell.perm_y;
        if !kx.is_finite() || !ky.is_finite() || kx <= 0.0 || ky <= 0.0 {
            return Err(format!(
                "Cell permeability must be positive and finite for well PI calculation, got kx={}, ky={}",
                kx, ky
            ));
        }

        let r_eq = 0.28
            * f64::sqrt(
                f64::sqrt(kx / ky) * self.dx.powi(2) + f64::sqrt(ky / kx) * self.dy.powi(2),
            )
            / ((kx / ky).powf(0.25) + (ky / kx).powf(0.25));
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
        let total_mobility = self.total_mobility(cell);
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
        // Constant 8.527e-5 converts from mD·m²/(m·cP) to m³/day/bar
        // Derivation: 1 mD = 9.8692e-16 m², 1 cP = 1e-3 Pa·s, 1 bar = 1e5 Pa, 1 day = 86400 s
        // Factor = 9.8692e-16 * 86400 / 1e-3 / 1e5 = 8.527e-5
        Ok((8.527e-5 * 2.0 * std::f64::consts::PI * k_avg * self.dz * total_mobility) / denom)
    }

    pub(crate) fn update_dynamic_well_productivity_indices(&mut self) {
        let mut updated_pi: Vec<Option<f64>> = Vec::with_capacity(self.wells.len());

        for well in self.wells.iter() {
            let id = self.idx(well.i, well.j, well.k);
            let cell = self.grid_cells[id];
            let maybe_pi = self
                .calculate_well_productivity_index(&cell, well.well_radius, well.skin)
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

    /// Total mobility [1/cP] = lambda_t = (k_rw/μ_w) + (k_ro/μ_o)
    /// Sum of phase mobilities used in pressure equation
    fn total_mobility(&self, cell: &GridCell) -> f64 {
        let krw = self.scal.k_rw(cell.sat_water);
        let kro = self.scal.k_ro(cell.sat_water);
        krw / self.pvt.mu_w + kro / self.pvt.mu_o
    }

    /// Phase mobilities [1/cP] for water and oil
    fn phase_mobilities(&self, cell: &GridCell) -> (f64, f64) {
        let krw = self.scal.k_rw(cell.sat_water);
        let kro = self.scal.k_ro(cell.sat_water);
        (krw / self.pvt.mu_w, kro / self.pvt.mu_o)
    }

    /// Get capillary pressure [bar] at given water saturation
    fn get_capillary_pressure(&self, s_w: f64) -> f64 {
        self.pc.capillary_pressure(s_w, &self.scal)
    }

    fn depth_at_k(&self, k: usize) -> f64 {
        self.depth_reference_m + (k as f64 + 0.5) * self.dz
    }

    fn gravity_head_bar(&self, depth_i: f64, depth_j: f64, density_kg_m3: f64) -> f64 {
        if !self.gravity_enabled {
            return 0.0;
        }

        // rho [kg/m³] * g [m/s²] * dz [m] = Pa, then convert Pa -> bar using 1e-5
        density_kg_m3 * 9.80665 * (depth_i - depth_j) * 1e-5
    }

    fn total_density_face(&self, c_i: &GridCell, c_j: &GridCell) -> f64 {
        let (lam_w_i, lam_o_i) = self.phase_mobilities(c_i);
        let (lam_w_j, lam_o_j) = self.phase_mobilities(c_j);

        let lam_w_avg = 0.5 * (lam_w_i + lam_w_j);
        let lam_o_avg = 0.5 * (lam_o_i + lam_o_j);
        let lam_t_avg = lam_w_avg + lam_o_avg;

        if lam_t_avg <= f64::EPSILON {
            return 0.5 * (self.pvt.rho_w + self.pvt.rho_o);
        }

        ((lam_w_avg * self.pvt.rho_w) + (lam_o_avg * self.pvt.rho_o)) / lam_t_avg
    }

    /// Fractional flow of water [dimensionless] = f_w = λ_w / λ_t
    /// Used in upwind scheme for saturation transport
    fn frac_flow_water(&self, cell: &GridCell) -> f64 {
        let krw = self.scal.k_rw(cell.sat_water);
        let lam_w = krw / self.pvt.mu_w;
        let lam_t = lam_w + (self.scal.k_ro(cell.sat_water) / self.pvt.mu_o);
        if lam_t <= 0.0 {
            0.0
        } else {
            (lam_w / lam_t).clamp(0.0, 1.0)
        }
    }

    /// Geometric transmissibility factor [mD·m²/m] - geometry only, no mobility
    /// This is the constant part of transmissibility that depends only on rock properties
    /// and grid geometry. Used with upstream mobility for proper flow direction.
    /// Formula: T_geom = k_h * A / L where k_h is harmonic mean of permeabilities
    fn geometric_transmissibility(&self, c1: &GridCell, c2: &GridCell, dim: char) -> f64 {
        let (perm1, perm2, dist, area) = match dim {
            'x' => (c1.perm_x, c2.perm_x, self.dx, self.dy * self.dz),
            'y' => (c1.perm_y, c2.perm_y, self.dy, self.dx * self.dz),
            'z' => (c1.perm_z, c2.perm_z, self.dz, self.dx * self.dy),
            _ => (0.0, 0.0, 1.0, 1.0),
        };
        // Harmonic mean of permeabilities [mD]
        let k_h = if perm1 + perm2 == 0.0 {
            0.0
        } else {
            2.0 * perm1 * perm2 / (perm1 + perm2)
        };
        if k_h == 0.0 {
            return 0.0;
        }

        // Geometric transmissibility factor [mD·m²/m]
        k_h * area / dist
    }

    /// Full transmissibility [m³/day/bar] with upstream-weighted total mobility
    /// Uses upstream weighting: mobility is taken from the upstream cell (where flow comes from)
    /// This is the standard reservoir simulation practice for better accuracy at sharp fronts
    /// p_i, p_j: pressures in cells i and j
    /// grav_head_bar: gravity head term (positive if cell i is deeper)
    fn transmissibility_upstream(
        &self,
        c1: &GridCell,
        c2: &GridCell,
        dim: char,
        p_i: f64,
        p_j: f64,
        grav_head_bar: f64,
    ) -> f64 {
        let t_geom = self.geometric_transmissibility(c1, c2, dim);
        if t_geom == 0.0 {
            return 0.0;
        }

        // Potential difference determines flow direction
        // Positive potential_diff means flow from c1 to c2
        let potential_diff = (p_i - p_j) - grav_head_bar;

        // Upstream total mobility: take from the cell where flow originates
        let mob_upstream = if potential_diff >= 0.0 {
            self.total_mobility(c1)
        } else {
            self.total_mobility(c2)
        };

        // Transmissibility [m³/day/bar]
        // 8.527e-5 converts mD·m²/(m·cP) to m³/day/bar
        8.527e-5 * t_geom * mob_upstream
    }

    /// Full transmissibility [m³/day/bar] with upstream-weighted total mobility
    /// Uses previous pressure solution to determine flow direction
    fn transmissibility_with_prev_pressure(
        &self,
        c1: &GridCell,
        c2: &GridCell,
        dim: char,
        grav_head_bar: f64,
    ) -> f64 {
        self.transmissibility_upstream(c1, c2, dim, c1.pressure, c2.pressure, grav_head_bar)
    }

    pub(crate) fn step_internal(&mut self, target_dt_days: f64) {
        let mut time_stepped = 0.0;
        const MAX_ATTEMPTS: u32 = 10;
        let mut attempts = 0;
        self.last_solver_warning = String::new();

        while time_stepped < target_dt_days && attempts < MAX_ATTEMPTS {
            let remaining_dt = target_dt_days - time_stepped;

            // Dynamic PI update with latest local saturation/mobility before pressure solve
            self.update_dynamic_well_productivity_indices();

            // Calculate fluxes and stability for the remaining time step
            let (p_new, delta_water_m3, stable_dt_factor, pcg_converged, pcg_iters) =
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
            let final_delta_water_m3;

            if stable_dt_factor < 1.0 {
                // Timestep is too large, reduce it based on CFL condition
                actual_dt = remaining_dt * stable_dt_factor * 0.9; // Use 90% for safety

                // Scale the water volume change by the ratio of the new dt to the old dt
                let dt_ratio = actual_dt / remaining_dt;
                final_delta_water_m3 = delta_water_m3.iter().map(|&dv| dv * dt_ratio).collect();

                attempts += 1;
            } else {
                // The full remaining timestep is stable
                actual_dt = remaining_dt;
                final_delta_water_m3 = delta_water_m3;
                attempts = 0; // Reset attempts on a successful full step
            }

            // Update saturations and pressure with the adjusted (or full) timestep
            self.update_saturations_and_pressure(&p_new, &final_delta_water_m3, actual_dt);

            time_stepped += actual_dt;
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
                return Some(WellControlDecision::Rate {
                    q_m3_day: q_target,
                });
            }

            // Rate target implies a dynamic BHP. If it violates limits, switch to BHP-control.
            let implied_bhp = pressure_bar - (q_target / well.productivity_index);
            let constrained_bhp = implied_bhp.clamp(self.well_bhp_min, self.well_bhp_max);

            if (constrained_bhp - implied_bhp).abs() > 1e-9 {
                return Some(WellControlDecision::Bhp {
                    bhp_bar: constrained_bhp,
                });
            }

            return Some(WellControlDecision::Rate {
                q_m3_day: q_target,
            });
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

    fn calculate_fluxes(&self, delta_t_days: f64) -> (DVector<f64>, Vec<f64>, f64, bool, usize) {
        let n_cells = self.nx * self.ny * self.nz;
        if n_cells == 0 {
            return (DVector::zeros(0), vec![], 1.0, true, 0);
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
                    let cell = &self.grid_cells[id];

                    // Pore volume [m³]
                    let vp_m3 = cell.pore_volume_m3(self.dx, self.dy, self.dz);

                    // Per-cell total compressibility [1/bar]
                    // c_t = (c_o * S_o + c_w * S_w) + c_r
                    // Note: pore volume Vp already includes porosity, so do not multiply by ϕ again.
                    let c_t =
                        (self.pvt.c_o * cell.sat_oil + self.pvt.c_w * cell.sat_water)
                            + self.rock_compressibility;

                    // Accumulation term: (Vp [m³] * c_t [1/bar]) / dt [day]
                    // Units: [m³ * 1/bar / day] = [m³/bar/day]
                    let accum = (vp_m3 * c_t) / dt_days;
                    let mut diag = accum;

                    // Move old pressure term to RHS: accum * p_old
                    b_rhs[id] += accum * cell.pressure;

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
                        // Gravity head for potential calculation
                        let depth_i = self.depth_at_k(k);
                        let depth_j = self.depth_at_k(*n_k);
                        let rho_t = self.total_density_face(cell, &self.grid_cells[*n_id]);
                        let grav_head_bar = self.gravity_head_bar(depth_i, depth_j, rho_t);

                        // Transmissibility with upstream weighting [m³/day/bar]
                        // Uses previous pressure to determine flow direction
                        let t = self.transmissibility_with_prev_pressure(
                            cell,
                            &self.grid_cells[*n_id],
                            *dim,
                            grav_head_bar,
                        );
                        diag += t;
                        rows.push(id);
                        cols.push(*n_id);
                        vals.push(-t);

                        // Gravity source contribution on RHS from potential formulation
                        b_rhs[id] += t * grav_head_bar;
                    }

                    // Well source terms
                    for w in &self.wells {
                        if w.i == i && w.j == j && w.k == k {
                            if let Some(control) = self.resolve_well_control(w, cell.pressure) {
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
            x0[i] = self.grid_cells[i].pressure;
        }
        let pcg_result = solve_pcg_with_guess(&a_mat, &b_rhs, &diag_inv, &x0, 1e-7, 1000);
        let p_new = pcg_result.solution;

        // Compute phase fluxes and explicit saturation update (upwind fractional flow method)
        // Track total water volume change [m³] per cell over dt_days
        let mut delta_water_m3 = vec![0.0f64; n_cells];
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

                        // Gravity head for potential calculation
                        let depth_i = self.depth_at_k(k);
                        let depth_j = self.depth_at_k(n_k);
                        let rho_t = self.total_density_face(&self.grid_cells[id], &self.grid_cells[nid]);
                        let grav_head_bar = self.gravity_head_bar(depth_i, depth_j, rho_t);

                        // Transmissibility with upstream weighting [m³/day/bar]
                        // Uses new pressure solution to determine flow direction
                        let t = self.transmissibility_upstream(
                            &self.grid_cells[id],
                            &self.grid_cells[nid],
                            dim,
                            p_i,
                            p_j,
                            grav_head_bar,
                        );

                        // Get geometric transmissibility for capillary flux calculation
                        let geom_t = 8.527e-5
                            * self.geometric_transmissibility(
                                &self.grid_cells[id],
                                &self.grid_cells[nid],
                                dim,
                            );

                        let (lam_w_i, lam_o_i) = self.phase_mobilities(&self.grid_cells[id]);
                        let (lam_w_j, lam_o_j) = self.phase_mobilities(&self.grid_cells[nid]);
                        let lam_t_i = lam_w_i + lam_o_i;
                        let lam_t_j = lam_w_j + lam_o_j;
                        let lam_t_avg = 0.5 * (lam_t_i + lam_t_j);
                        if lam_t_avg <= f64::EPSILON {
                            continue;
                        }

                        // Total volumetric flux [m³/day]: positive = from id -> nid
                        let total_flux = t * ((p_i - p_j) - grav_head_bar);

                        // Upwind fractional flow for water
                        let f_w = if total_flux >= 0.0 {
                            self.frac_flow_water(&self.grid_cells[id])
                        } else {
                            self.frac_flow_water(&self.grid_cells[nid])
                        };

                        // Capillary-driven diffusion term using harmonic transmissibility geometry
                        let pc_i = self.get_capillary_pressure(self.grid_cells[id].sat_water);
                        let pc_j = self.get_capillary_pressure(self.grid_cells[nid].sat_water);
                        let lam_w_avg = 0.5 * (lam_w_i + lam_w_j);
                        let lam_o_avg = 0.5 * (lam_o_i + lam_o_j);
                        let capillary_flux = if lam_t_avg <= f64::EPSILON {
                            0.0
                        } else {
                            -geom_t * (lam_w_avg * lam_o_avg / lam_t_avg) * (pc_i - pc_j)
                        };

                        // Water flux [m³/day]
                        let water_flux_m3_day = total_flux * f_w + capillary_flux;
                        // Volume change over dt_days [m³]
                        let dv_water = water_flux_m3_day * dt_days;

                        // Distribute: outgoing flow reduces water in source cell
                        delta_water_m3[id] -= dv_water;
                        delta_water_m3[nid] += dv_water;
                    }
                }
            }
        }

        // Add well explicit contributions using solved pressure
        for w in &self.wells {
            let id = self.idx(w.i, w.j, w.k);

            if let Some(q_m3_day) = self.well_rate_m3_day(w, p_new[id]) {
                // Determine water composition of well fluid
                let fw = if w.injector {
                    // Injectors inject 100% water
                    1.0
                } else {
                    // Producers produce at reservoir fluid composition (fractional flow)
                    self.frac_flow_water(&self.grid_cells[id])
                };

                let water_q_m3_day = q_m3_day * fw;

                // Volume change [m³]. Production (q>0) removes fluid from block.
                // For injector: q<0 (inflow), so -q_water*dt adds water to the block
                delta_water_m3[id] -= water_q_m3_day * dt_days;
            }
        }

        // Calculate max saturation change for CFL condition
        for idx in 0..n_cells {
            let vp_m3 = self.grid_cells[idx].pore_volume_m3(self.dx, self.dy, self.dz);
            if vp_m3 > 0.0 {
                let sat_change = (delta_water_m3[idx] / vp_m3).abs();
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
            let dp = (p_new[idx] - self.grid_cells[idx].pressure).abs();
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
            let q_old = self.well_rate_m3_day(w, self.grid_cells[id].pressure).unwrap_or(0.0);
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

        let stable_dt_factor = sat_factor.min(pressure_factor).min(rate_factor).clamp(0.01, 1.0);

        (
            p_new,
            delta_water_m3,
            stable_dt_factor,
            pcg_result.converged,
            pcg_result.iterations,
        )
    }

    fn update_saturations_and_pressure(
        &mut self,
        p_new: &DVector<f64>,
        delta_water_m3: &Vec<f64>,
        dt_days: f64,
    ) {
        let n_cells = self.nx * self.ny * self.nz;
        // Update saturations based on water volume changes
        for idx in 0..n_cells {
            let vp_m3 = self.grid_cells[idx].pore_volume_m3(self.dx, self.dy, self.dz);
            if vp_m3 <= 0.0 {
                continue;
            }

            // Change in water saturation [dimensionless] = ΔV_water [m³] / V_pore [m³]
            let sw_old = self.grid_cells[idx].sat_water;
            let sw_min = self.scal.s_wc;
            let sw_max = 1.0 - self.scal.s_or;
            let delta_sw = delta_water_m3[idx] / vp_m3;
            let sw_new = (sw_old + delta_sw).clamp(sw_min, sw_max);

            // Ensure material balance: s_w + s_o = 1.0 (two-phase system, no gas phase)
            let so_new = (1.0 - sw_new).clamp(0.0, 1.0);

            // Update state variables
            self.grid_cells[idx].sat_water = sw_new;
            self.grid_cells[idx].sat_oil = so_new;
            self.grid_cells[idx].pressure = p_new[idx];
        }

        // Calculate and store rates
        let mut total_prod_oil = 0.0;
        let mut total_prod_liquid = 0.0;
        let mut total_prod_liquid_reservoir = 0.0;
        let mut total_injection = 0.0;
        let mut total_injection_reservoir = 0.0;
        let mut total_prod_water_reservoir = 0.0;

        for w in &self.wells {
            let id = self.idx(w.i, w.j, w.k);
            if let Some(q_m3_day) = self.well_rate_m3_day(w, p_new[id]) {
                if w.injector {
                    // Injection is negative flow
                    total_injection -= q_m3_day / self.b_w.max(1e-9);
                    total_injection_reservoir += -q_m3_day;
                } else {
                    // Production is positive flow
                    total_prod_liquid_reservoir += q_m3_day;
                    let fw = self.frac_flow_water(&self.grid_cells[id]);
                    total_prod_water_reservoir += q_m3_day * fw;
                    let oil_rate = q_m3_day * (1.0 - fw) / self.b_o.max(1e-9);
                    let water_rate = q_m3_day * fw / self.b_w.max(1e-9);
                    total_prod_oil += oil_rate;
                    total_prod_liquid += oil_rate + water_rate;
                }
            }
        }

        // Update cumulative water volumes in reservoir conditions for material balance
        self.cumulative_injection_m3 += total_injection_reservoir * dt_days;
        self.cumulative_production_m3 += total_prod_water_reservoir * dt_days;

        // Compute material balance error [m³]
        // Step-wise water balance in reservoir conditions:
        // Net water added via wells = (water injection_res - water production_res) * dt
        let net_added_m3 = (total_injection_reservoir - total_prod_water_reservoir) * dt_days;
        // Actual water in-place change = sum of delta_water volumes (already applied)
        let actual_change_m3: f64 = delta_water_m3.iter().sum();
        // Error = water added/removed by wells minus water-volume change in cells
        let mb_error = (net_added_m3 - actual_change_m3).abs();

        self.rate_history.push(TimePointRates {
            time: self.time_days + dt_days,
            total_production_oil: total_prod_oil,
            total_production_liquid: total_prod_liquid,
            total_production_liquid_reservoir: total_prod_liquid_reservoir,
            total_injection: total_injection,
            total_injection_reservoir: total_injection_reservoir,
            material_balance_error_m3: mb_error,
        });

        // Advance simulation time
        self.time_days += dt_days;
    }
}
