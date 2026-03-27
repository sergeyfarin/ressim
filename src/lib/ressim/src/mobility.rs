use crate::ReservoirSimulator;

impl ReservoirSimulator {
    // ── Two-phase mobility ────────────────────────────────────────────────────

    /// Total mobility [1/cP] = lambda_t = (k_rw/μ_w) + (k_ro/μ_o) [+ k_rg/μ_g in 3-phase]
    pub(crate) fn total_mobility(&self, id: usize) -> f64 {
        if self.three_phase_mode {
            return self.total_mobility_3p(id);
        }
        let krw = self.scal.k_rw(self.sat_water[id]);
        let kro = self.scal.k_ro(self.sat_water[id]);
        krw / self.get_mu_w(self.pressure[id]) + kro / self.get_mu_o(self.pressure[id])
    }

    /// Phase mobilities [1/cP] for water and oil (2-phase)
    pub(crate) fn phase_mobilities(&self, id: usize) -> (f64, f64) {
        let krw = self.scal.k_rw(self.sat_water[id]);
        let kro = self.scal.k_ro(self.sat_water[id]);
        (krw / self.get_mu_w(self.pressure[id]), kro / self.get_mu_o(self.pressure[id]))
    }

    // ── Three-phase mobility ──────────────────────────────────────────────────

    /// Total mobility using Stone II k_ro and Corey k_rg
    pub(crate) fn total_mobility_3p(&self, id: usize) -> f64 {
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
    pub(crate) fn phase_mobilities_3p(&self, id: usize) -> (f64, f64, f64) {
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
    pub(crate) fn gas_mobility(&self, id: usize) -> f64 {
        self.scal_3p
            .as_ref()
            .map_or(0.0, |s| s.k_rg(self.sat_gas[id]) / self.get_mu_g(self.pressure[id]))
    }

    // ── Mobility at arbitrary pressure (for well calculations) ────────────────

    pub(crate) fn phase_mobilities_at_pressure(&self, id: usize, pressure_bar: f64) -> (f64, f64) {
        let krw = self.scal.k_rw(self.sat_water[id]);
        let kro = self.scal.k_ro(self.sat_water[id]);
        (
            krw / self.get_mu_w(pressure_bar),
            kro / self.get_mu_o_cell(id, pressure_bar),
        )
    }

    pub(crate) fn phase_mobilities_3p_at_pressure(&self, id: usize, pressure_bar: f64) -> (f64, f64, f64) {
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

    #[allow(dead_code)]
    pub(crate) fn producer_oil_fraction_at_pressure(&self, id: usize, pressure_bar: f64) -> f64 {
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

    // ── Fractional flow ───────────────────────────────────────────────────────

    /// Fractional flow of gas = λ_g / λ_t (three-phase)
    #[allow(dead_code)]
    pub(crate) fn frac_flow_gas(&self, id: usize) -> f64 {
        let lam_g = self.gas_mobility(id);
        let lam_t = self.total_mobility_3p(id);
        if lam_t <= 0.0 {
            0.0
        } else {
            (lam_g / lam_t).clamp(0.0, 1.0)
        }
    }

    /// Fractional flow of water in three-phase system = λ_w / λ_t
    #[allow(dead_code)]
    pub(crate) fn frac_flow_water_3p(&self, id: usize) -> f64 {
        let lam_t = self.total_mobility_3p(id);
        if lam_t <= 0.0 {
            return 0.0;
        }
        let (lam_w, _, _) = self.phase_mobilities_3p(id);
        (lam_w / lam_t).clamp(0.0, 1.0)
    }

    /// Fractional flow of water [dimensionless] = f_w = λ_w / λ_t (2-phase)
    pub(crate) fn frac_flow_water(&self, id: usize) -> f64 {
        let krw = self.scal.k_rw(self.sat_water[id]);
        let lam_w = krw / self.get_mu_w(self.pressure[id]);
        let lam_t = lam_w + (self.scal.k_ro(self.sat_water[id]) / self.get_mu_o(self.pressure[id]));
        if lam_t <= 0.0 {
            0.0
        } else {
            (lam_w / lam_t).clamp(0.0, 1.0)
        }
    }

    // ── Capillary and gravity ─────────────────────────────────────────────────

    /// Oil-gas capillary pressure [bar] at given gas saturation
    pub(crate) fn get_gas_oil_capillary_pressure(&self, s_g: f64) -> f64 {
        match (&self.pc_og, &self.scal_3p) {
            (Some(pc), Some(rock)) => pc.capillary_pressure_og(s_g, rock),
            _ => 0.0,
        }
    }

    /// Water-oil capillary pressure [bar] at given water saturation
    pub(crate) fn get_capillary_pressure(&self, s_w: f64) -> f64 {
        self.pc.capillary_pressure(s_w, &self.scal)
    }

    pub(crate) fn gravity_head_bar(&self, depth_i: f64, depth_j: f64, density_kg_m3: f64) -> f64 {
        if !self.gravity_enabled {
            return 0.0;
        }
        // rho [kg/m³] * g [m/s²] * dz [m] = Pa, then convert Pa -> bar using 1e-5
        density_kg_m3 * 9.80665 * (depth_i - depth_j) * 1e-5
    }

    pub(crate) fn interface_density_barrier(&self, rho_i: f64, rho_j: f64) -> f64 {
        0.5 * (rho_i + rho_j)
    }
}
