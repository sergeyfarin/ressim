use serde::{Deserialize, Serialize};

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
}
