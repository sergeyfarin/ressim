use serde::{Deserialize, Serialize};

use crate::ReservoirSimulator;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OilProps {
    pub(crate) bo_m3m3: f64,
    pub(crate) mu_o_cp: f64,
    pub(crate) rho_o_kg_m3: f64,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GasProps {
    pub(crate) bg_m3m3: f64,
    pub(crate) mu_g_cp: f64,
    pub(crate) rho_g_kg_m3: f64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PvtRow {
    pub p_bar: f64,
    pub rs_m3m3: f64,
    pub bo_m3m3: f64,
    pub mu_o_cp: f64,
    pub bg_m3m3: f64,
    pub mu_g_cp: f64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PvtTable {
    pub rows: Vec<PvtRow>,
    /// Base oil compressibility above bubble point [1/bar]
    pub c_o: f64,
}

impl PvtTable {
    pub fn new(mut rows: Vec<PvtRow>, c_o: f64) -> Self {
        rows.sort_by(|a, b| a.p_bar.partial_cmp(&b.p_bar).unwrap());
        Self { rows, c_o }
    }

    /// Interpolate PVT properties for a given pressure.
    /// Extrapolates using constant compressibility for points above the table max pressure.
    pub fn interpolate(&self, p: f64) -> PvtRow {
        if self.rows.is_empty() {
            // Fallback flat properties if initialized empty
            return PvtRow {
                p_bar: p,
                rs_m3m3: 0.0,
                bo_m3m3: 1.0,
                mu_o_cp: 1.0,
                bg_m3m3: 1.0,
                mu_g_cp: 0.02,
            };
        }

        if p <= self.rows[0].p_bar {
            return self.rows[0].clone();
        }

        let last_idx = self.rows.len() - 1;
        if p >= self.rows[last_idx].p_bar {
            let last = &self.rows[last_idx];
            let mut extrapolated = last.clone();
            extrapolated.p_bar = p;
            // Above table pressure usually means undersaturated oil compression.
            extrapolated.bo_m3m3 = last.bo_m3m3 * f64::exp(-self.c_o * (p - last.p_bar));
            // Simple gas extrapolation: bg inversely proportional to P, ignoring z-factor curve changes
            extrapolated.bg_m3m3 = last.bg_m3m3 * (last.p_bar / p);
            return extrapolated;
        }

        // Linear interpolation
        for i in 0..last_idx {
            let r0 = &self.rows[i];
            let r1 = &self.rows[i + 1];
            if p >= r0.p_bar && p <= r1.p_bar {
                let dp = r1.p_bar - r0.p_bar;
                if dp < 1e-6 {
                    return r0.clone();
                }
                let t = (p - r0.p_bar) / dp;
                return PvtRow {
                    p_bar: p,
                    rs_m3m3: r0.rs_m3m3 + t * (r1.rs_m3m3 - r0.rs_m3m3),
                    bo_m3m3: r0.bo_m3m3 + t * (r1.bo_m3m3 - r0.bo_m3m3),
                    mu_o_cp: r0.mu_o_cp + t * (r1.mu_o_cp - r0.mu_o_cp),
                    bg_m3m3: r0.bg_m3m3 + t * (r1.bg_m3m3 - r0.bg_m3m3),
                    mu_g_cp: r0.mu_g_cp + t * (r1.mu_g_cp - r0.mu_g_cp),
                };
            }
        }
        self.rows[last_idx].clone() // Fallback
    }

    /// Find the bubble-point pressure for a given dissolved-gas ratio.
    ///
    /// Inverse interpolation on the saturated Rs-vs-pressure curve.
    /// Returns the lowest table pressure if `rs` is below the table minimum,
    /// and the highest table pressure if `rs` exceeds the table maximum.
    pub fn bubble_point_pressure(&self, rs: f64) -> f64 {
        if self.rows.is_empty() {
            return 0.0;
        }
        if rs <= self.rows[0].rs_m3m3 {
            return self.rows[0].p_bar;
        }
        let last = self.rows.len() - 1;
        if rs >= self.rows[last].rs_m3m3 {
            return self.rows[last].p_bar;
        }
        for i in 0..last {
            let r0 = &self.rows[i];
            let r1 = &self.rows[i + 1];
            if rs >= r0.rs_m3m3 && rs <= r1.rs_m3m3 {
                let drs = r1.rs_m3m3 - r0.rs_m3m3;
                if drs < 1e-12 {
                    return r0.p_bar;
                }
                let t = (rs - r0.rs_m3m3) / drs;
                return r0.p_bar + t * (r1.p_bar - r0.p_bar);
            }
        }
        self.rows[last].p_bar
    }

    /// Interpolate oil properties accounting for undersaturation.
    ///
    /// If the cell's dissolved gas `rs` is below the saturated Rs at pressure `p`,
    /// the oil is undersaturated: find the bubble-point pressure for `rs`, read
    /// saturated Bo and μ_o there, then apply undersaturated corrections above it.
    pub fn interpolate_oil(&self, p: f64, rs: f64) -> (f64, f64) {
        if self.rows.is_empty() {
            return (1.0, 1.0);
        }
        let rs_sat = self.interpolate(p).rs_m3m3;

        if rs < rs_sat - 1e-6 {
            // Undersaturated: get bubble-point properties and extrapolate
            let p_b = self.bubble_point_pressure(rs);
            let sat = self.interpolate(p_b);
            let bo = sat.bo_m3m3 * f64::exp(-self.c_o * (p - p_b));
            // Undersaturated viscosity increases linearly with pressure
            // (simple model consistent with SPE1 PVTO data)
            let mu = sat.mu_o_cp * f64::exp(self.c_o * (p - p_b));
            (bo, mu)
        } else {
            // Saturated or above table max: use standard interpolation
            let row = self.interpolate(p);
            (row.bo_m3m3, row.mu_o_cp)
        }
    }

    pub(crate) fn oil_props_at(&self, p: f64, rs: f64, rho_o_sc: f64, rho_g_sc: f64) -> OilProps {
        let (bo_m3m3, mu_o_cp) = self.interpolate_oil(p, rs);
        OilProps {
            bo_m3m3,
            mu_o_cp,
            rho_o_kg_m3: (rho_o_sc + rs * rho_g_sc) / bo_m3m3.max(1e-9),
        }
    }

    pub(crate) fn gas_props_at(&self, p: f64, rho_g_sc: f64) -> GasProps {
        let row = self.interpolate(p);
        GasProps {
            bg_m3m3: row.bg_m3m3,
            mu_g_cp: row.mu_g_cp,
            rho_g_kg_m3: rho_g_sc / row.bg_m3m3.max(1e-9),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn d_bo_d_p(&self, p: f64, rs: f64) -> f64 {
        let dp = 1.0;
        let p_lo = (p - dp).max(0.0);
        let (bo_lo, _) = self.interpolate_oil(p_lo, rs);
        let (bo_hi, _) = self.interpolate_oil(p + dp, rs);
        (bo_hi - bo_lo) / (2.0 * dp)
    }

    #[allow(dead_code)]
    pub(crate) fn d_rs_sat_d_p(&self, p: f64) -> f64 {
        let dp = 1.0;
        let p_lo = (p - dp).max(0.0);
        let row_lo = self.interpolate(p_lo);
        let row_hi = self.interpolate(p + dp);
        (row_hi.rs_m3m3 - row_lo.rs_m3m3) / (2.0 * dp)
    }
}

impl ReservoirSimulator {
    pub(crate) fn get_mu_o(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            table.interpolate(p).mu_o_cp
        } else {
            self.pvt.mu_o
        }
    }

    pub(crate) fn get_mu_o_cell(&self, id: usize, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                let (_, mu) = table.interpolate_oil(p, self.rs[id]);
                return mu;
            }
            table.interpolate(p).mu_o_cp
        } else {
            self.pvt.mu_o
        }
    }

    #[allow(dead_code)]
    pub(crate) fn get_mu_o_for_rs(&self, p: f64, rs_sm3_sm3: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                let (_, mu) = table.interpolate_oil(p, rs_sm3_sm3);
                return mu;
            }
            table.interpolate(p).mu_o_cp
        } else {
            self.pvt.mu_o
        }
    }

    pub(crate) fn get_mu_w(&self, _p: f64) -> f64 {
        self.pvt.mu_w
    }

    pub(crate) fn get_mu_g(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            table.interpolate(p).mu_g_cp
        } else {
            self.mu_g
        }
    }

    pub(crate) fn get_c_o(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let dp = 1.0;
            let p_minus = if p > dp { p - dp } else { 0.0 };
            let b1 = table.interpolate(p_minus).bo_m3m3;
            let b2 = table.interpolate(p + dp).bo_m3m3;
            let bo = table.interpolate(p).bo_m3m3;
            if bo > 1e-12 {
                let derived_c_o = (-1.0 / bo) * (b2 - b1) / (2.0 * dp);
                if derived_c_o.is_finite() && derived_c_o > 0.0 {
                    derived_c_o.max(self.pvt.c_o)
                } else {
                    self.pvt.c_o
                }
            } else {
                self.pvt.c_o
            }
        } else {
            self.pvt.c_o
        }
    }

    pub(crate) fn get_c_g(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let dp = 1.0;
            let p_minus = if p > dp { p - dp } else { 0.0 };
            let b1 = table.interpolate(p_minus).bg_m3m3;
            let b2 = table.interpolate(p + dp).bg_m3m3;
            let bg = table.interpolate(p).bg_m3m3;
            if bg > 1e-12 {
                (-1.0 / bg) * (b2 - b1) / (2.0 * dp)
            } else {
                self.c_g
            }
        } else {
            self.c_g
        }
    }

    pub(crate) fn get_c_o_effective(&self, p: f64, rs_cell: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let rs_sat = table.interpolate(p).rs_m3m3;
            let c_sat = self.saturated_c_o_eff(table, p);

            if rs_cell < rs_sat - 1e-6 {
                let c_unsat = self.pvt.c_o;
                let p_b = table.bubble_point_pressure(rs_cell);
                let distance = p - p_b;
                let margin = self.max_pressure_change_per_step;

                if distance > 0.0 && distance < margin && c_sat > c_unsat {
                    let t = 1.0 - distance / margin;
                    let blend = t * t;
                    return c_unsat + blend * (c_sat - c_unsat);
                }
                return c_unsat;
            }

            c_sat
        } else {
            self.pvt.c_o
        }
    }

    fn saturated_c_o_eff(&self, table: &PvtTable, p: f64) -> f64 {
        let dp = 1.0;
        let p_lo = (p - dp).max(0.0);
        let row_lo = table.interpolate(p_lo);
        let row_hi = table.interpolate(p + dp);
        let row_mid = table.interpolate(p);

        let bo = row_mid.bo_m3m3;
        let bg = row_mid.bg_m3m3;
        if bo > 1e-12 {
            let dbo_dp = (row_hi.bo_m3m3 - row_lo.bo_m3m3) / (2.0 * dp);
            let c_o = -dbo_dp / bo;

            let drs_dp = (row_hi.rs_m3m3 - row_lo.rs_m3m3) / (2.0 * dp);
            let c_dg = if bg > 0.0 { (bg / bo) * drs_dp } else { 0.0 };

            let c_eff = c_o + c_dg;
            if c_eff.is_finite() && c_eff > 0.0 {
                return c_eff;
            }
        }
        self.pvt.c_o
    }

    pub(crate) fn get_b_o_cell(&self, id: usize, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                let (bo, _) = table.interpolate_oil(p, self.rs[id]);
                return bo;
            }
            table.interpolate(p).bo_m3m3
        } else {
            self.b_o
        }
    }

    pub(crate) fn get_rho_o_cell(&self, id: usize, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let rs = self.rs[id];
            let (bo, _) = table.interpolate_oil(p, rs);
            (self.pvt.rho_o + rs * self.rho_g) / bo
        } else {
            self.pvt.rho_o
        }
    }

    #[allow(dead_code)]
    pub(crate) fn get_b_o_for_rs(&self, p: f64, rs_sm3_sm3: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                let (bo, _) = table.interpolate_oil(p, rs_sm3_sm3);
                return bo;
            }
            table.interpolate(p).bo_m3m3
        } else {
            self.b_o
        }
    }

    #[allow(dead_code)]
    pub(crate) fn get_rho_o_for_rs(&self, p: f64, rs_sm3_sm3: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let props = table.oil_props_at(p, rs_sm3_sm3, self.pvt.rho_o, self.rho_g);
            props.rho_o_kg_m3
        } else {
            self.pvt.rho_o
        }
    }

    pub(crate) fn get_rho_o(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let row = table.interpolate(p);
            (self.pvt.rho_o + row.rs_m3m3 * self.rho_g) / row.bo_m3m3
        } else {
            self.pvt.rho_o
        }
    }

    pub(crate) fn get_rho_g(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            self.rho_g / table.interpolate(p).bg_m3m3
        } else {
            self.rho_g
        }
    }

    pub(crate) fn get_rho_w(&self, _p: f64) -> f64 {
        self.pvt.rho_w
    }

    pub(crate) fn get_b_g(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            table.interpolate(p).bg_m3m3
        } else {
            1.0
        }
    }

    #[allow(dead_code)]
    pub(crate) fn oil_props_for_state(&self, p: f64, rs_sm3_sm3: f64) -> OilProps {
        if let Some(table) = &self.pvt_table {
            table.oil_props_at(p, rs_sm3_sm3, self.pvt.rho_o, self.rho_g)
        } else {
            OilProps {
                bo_m3m3: self.b_o,
                mu_o_cp: self.pvt.mu_o,
                rho_o_kg_m3: self.pvt.rho_o,
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn gas_props_for_state(&self, p: f64) -> GasProps {
        if let Some(table) = &self.pvt_table {
            table.gas_props_at(p, self.rho_g)
        } else {
            GasProps {
                bg_m3m3: 1.0,
                mu_g_cp: self.mu_g,
                rho_g_kg_m3: self.rho_g,
            }
        }
    }
}
