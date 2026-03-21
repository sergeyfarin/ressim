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
}
