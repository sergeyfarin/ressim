use serde::{Deserialize, Serialize};

use crate::ReservoirSimulator;

const PVTO_RS_TOLERANCE: f64 = 1e-6;

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
    /// Original flat PVTO-style rows in input order.
    pub rows: Vec<PvtRow>,
    saturated_rows: Vec<PvtRow>,
    oil_branches: Vec<PvtOilBranch>,
    /// Base oil compressibility above bubble point [1/bar]
    pub c_o: f64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct PvtOilBranch {
    rs_m3m3: f64,
    rows: Vec<PvtRow>,
}

impl PvtTable {
    pub fn new(rows: Vec<PvtRow>, c_o: f64) -> Self {
        let mut oil_branches = Self::build_oil_branches(&rows);
        oil_branches.sort_by(|a, b| a.rs_m3m3.partial_cmp(&b.rs_m3m3).unwrap());

        let mut saturated_rows = oil_branches
            .iter()
            .filter_map(|branch| branch.rows.first().cloned())
            .collect::<Vec<_>>();
        saturated_rows.sort_by(|a, b| a.p_bar.partial_cmp(&b.p_bar).unwrap());

        Self {
            rows,
            saturated_rows,
            oil_branches,
            c_o,
        }
    }

    fn build_oil_branches(rows: &[PvtRow]) -> Vec<PvtOilBranch> {
        let mut branches: Vec<PvtOilBranch> = Vec::new();

        for row in rows {
            if let Some(branch) = branches.last_mut() {
                let same_rs = (row.rs_m3m3 - branch.rs_m3m3).abs() <= PVTO_RS_TOLERANCE;
                let higher_pressure = branch
                    .rows
                    .last()
                    .map(|prev| row.p_bar >= prev.p_bar - 1e-9)
                    .unwrap_or(true);

                if same_rs && higher_pressure {
                    branch.rows.push(row.clone());
                    continue;
                }
            }

            branches.push(PvtOilBranch {
                rs_m3m3: row.rs_m3m3,
                rows: vec![row.clone()],
            });
        }

        for branch in &mut branches {
            branch.rows.sort_by(|a, b| a.p_bar.partial_cmp(&b.p_bar).unwrap());
        }

        branches
    }

    fn interpolate_rows(rows: &[PvtRow], p: f64, c_o: f64) -> PvtRow {
        if rows.is_empty() {
            return PvtRow {
                p_bar: p,
                rs_m3m3: 0.0,
                bo_m3m3: 1.0,
                mu_o_cp: 1.0,
                bg_m3m3: 1.0,
                mu_g_cp: 0.02,
            };
        }

        if p <= rows[0].p_bar {
            return rows[0].clone();
        }

        let last_idx = rows.len() - 1;
        if p >= rows[last_idx].p_bar {
            let last = &rows[last_idx];
            let mut extrapolated = last.clone();
            extrapolated.p_bar = p;
            extrapolated.bo_m3m3 = last.bo_m3m3 * f64::exp(-c_o * (p - last.p_bar));
            extrapolated.bg_m3m3 = last.bg_m3m3 * (last.p_bar / p);
            return extrapolated;
        }

        for i in 0..last_idx {
            let r0 = &rows[i];
            let r1 = &rows[i + 1];
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

        rows[last_idx].clone()
    }

    fn branch_props(branch: &PvtOilBranch, p: f64, c_o: f64) -> (f64, f64) {
        let rows = &branch.rows;
        if rows.is_empty() {
            return (1.0, 1.0);
        }
        if rows.len() == 1 {
            let row = &rows[0];
            let bo = row.bo_m3m3 * f64::exp(-c_o * (p - row.p_bar));
            let mu = row.mu_o_cp * f64::exp(c_o * (p - row.p_bar));
            return (bo, mu);
        }
        if p <= rows[0].p_bar {
            return (rows[0].bo_m3m3, rows[0].mu_o_cp);
        }

        for pair in rows.windows(2) {
            let r0 = &pair[0];
            let r1 = &pair[1];
            if p >= r0.p_bar && p <= r1.p_bar {
                let dp = r1.p_bar - r0.p_bar;
                if dp.abs() < 1e-9 {
                    return (r0.bo_m3m3, r0.mu_o_cp);
                }
                let t = (p - r0.p_bar) / dp;
                return (
                    r0.bo_m3m3 + t * (r1.bo_m3m3 - r0.bo_m3m3),
                    r0.mu_o_cp + t * (r1.mu_o_cp - r0.mu_o_cp),
                );
            }
        }

        let last = &rows[rows.len() - 1];
        let prev = &rows[rows.len() - 2];
        let dp = (last.p_bar - prev.p_bar).max(1e-9);
        let t = (p - last.p_bar) / dp;
        (
            last.bo_m3m3 + t * (last.bo_m3m3 - prev.bo_m3m3),
            last.mu_o_cp + t * (last.mu_o_cp - prev.mu_o_cp),
        )
    }

    fn branch_bounds(&self, rs: f64) -> Option<(&PvtOilBranch, &PvtOilBranch)> {
        let first = self.oil_branches.first()?;
        let last = self.oil_branches.last()?;
        if rs <= first.rs_m3m3 + PVTO_RS_TOLERANCE {
            return Some((first, first));
        }
        if rs >= last.rs_m3m3 - PVTO_RS_TOLERANCE {
            return Some((last, last));
        }

        for pair in self.oil_branches.windows(2) {
            let low = &pair[0];
            let high = &pair[1];
            if rs >= low.rs_m3m3 - PVTO_RS_TOLERANCE && rs <= high.rs_m3m3 + PVTO_RS_TOLERANCE {
                return Some((low, high));
            }
        }

        Some((last, last))
    }

    /// Interpolate PVT properties for a given pressure.
    /// Extrapolates using constant compressibility for points above the table max pressure.
    pub fn interpolate(&self, p: f64) -> PvtRow {
        Self::interpolate_rows(&self.saturated_rows, p, self.c_o)
    }

    /// Find the bubble-point pressure for a given dissolved-gas ratio.
    ///
    /// Inverse interpolation on the saturated Rs-vs-pressure curve.
    /// Returns the lowest table pressure if `rs` is below the table minimum,
    /// and the highest table pressure if `rs` exceeds the table maximum.
    pub fn bubble_point_pressure(&self, rs: f64) -> f64 {
        if self.saturated_rows.is_empty() {
            return 0.0;
        }
        if rs <= self.oil_branches[0].rs_m3m3 {
            return self.oil_branches[0].rows[0].p_bar;
        }
        let last = self.oil_branches.len() - 1;
        if rs >= self.oil_branches[last].rs_m3m3 {
            return self.oil_branches[last].rows[0].p_bar;
        }
        for i in 0..last {
            let r0 = &self.oil_branches[i];
            let r1 = &self.oil_branches[i + 1];
            if rs >= r0.rs_m3m3 && rs <= r1.rs_m3m3 {
                let drs = r1.rs_m3m3 - r0.rs_m3m3;
                if drs < 1e-12 {
                    return r0.rows[0].p_bar;
                }
                let t = (rs - r0.rs_m3m3) / drs;
                return r0.rows[0].p_bar + t * (r1.rows[0].p_bar - r0.rows[0].p_bar);
            }
        }
        self.oil_branches[last].rows[0].p_bar
    }

    /// Interpolate oil properties accounting for undersaturation.
    ///
    /// If the cell's dissolved gas `rs` is below the saturated Rs at pressure `p`,
    /// the oil is undersaturated: find the bubble-point pressure for `rs`, read
    /// saturated Bo and μ_o there, then apply undersaturated corrections above it.
    pub fn interpolate_oil(&self, p: f64, rs: f64) -> (f64, f64) {
        if self.saturated_rows.is_empty() {
            return (1.0, 1.0);
        }
        let sat_row = self.interpolate(p);
        let rs_sat = sat_row.rs_m3m3;

        if rs >= rs_sat - 1e-6 {
            return (sat_row.bo_m3m3, sat_row.mu_o_cp);
        }

        if let Some((low, high)) = self.branch_bounds(rs) {
            let (bo_low, mu_low) = Self::branch_props(low, p, self.c_o);
            if (high.rs_m3m3 - low.rs_m3m3).abs() <= PVTO_RS_TOLERANCE {
                return (bo_low, mu_low);
            }

            let (bo_high, mu_high) = Self::branch_props(high, p, self.c_o);
            let t = (rs - low.rs_m3m3) / (high.rs_m3m3 - low.rs_m3m3);
            return (
                bo_low + t * (bo_high - bo_low),
                mu_low + t * (mu_high - mu_low),
            );
        }

        (sat_row.bo_m3m3, sat_row.mu_o_cp)
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

    pub(crate) fn d_bo_d_rs(&self, p: f64, rs: f64) -> f64 {
        let drs = 1.0;
        let rs_lo = (rs - drs).max(0.0);
        let (bo_lo, _) = self.interpolate_oil(p, rs_lo);
        let (bo_hi, _) = self.interpolate_oil(p, rs + drs);
        (bo_hi - bo_lo) / (2.0 * drs)
    }

    pub(crate) fn d_bo_sat_d_p(&self, p: f64) -> f64 {
        let dp = 1.0;
        let p_lo = (p - dp).max(0.0);
        let row_lo = self.interpolate(p_lo);
        let row_hi = self.interpolate(p + dp);
        (row_hi.bo_m3m3 - row_lo.bo_m3m3) / (2.0 * dp)
    }

    pub(crate) fn d_bg_d_p(&self, p: f64) -> f64 {
        let dp = 1.0;
        let p_lo = (p - dp).max(0.0);
        let row_lo = self.interpolate(p_lo);
        let row_hi = self.interpolate(p + dp);
        (row_hi.bg_m3m3 - row_lo.bg_m3m3) / (2.0 * dp)
    }

    pub(crate) fn d_mu_o_d_p(&self, p: f64, rs: f64) -> f64 {
        let dp = 1.0;
        let p_lo = (p - dp).max(0.0);
        let (_, mu_lo) = self.interpolate_oil(p_lo, rs);
        let (_, mu_hi) = self.interpolate_oil(p + dp, rs);
        (mu_hi - mu_lo) / (2.0 * dp)
    }

    pub(crate) fn d_mu_o_sat_d_p(&self, p: f64) -> f64 {
        let dp = 1.0;
        let p_lo = (p - dp).max(0.0);
        let row_lo = self.interpolate(p_lo);
        let row_hi = self.interpolate(p + dp);
        (row_hi.mu_o_cp - row_lo.mu_o_cp) / (2.0 * dp)
    }

    pub(crate) fn d_mu_o_d_rs(&self, p: f64, rs: f64) -> f64 {
        let drs = 1.0;
        let rs_lo = (rs - drs).max(0.0);
        let (_, mu_lo) = self.interpolate_oil(p, rs_lo);
        let (_, mu_hi) = self.interpolate_oil(p, rs + drs);
        (mu_hi - mu_lo) / (2.0 * drs)
    }

    pub(crate) fn d_mu_g_d_p(&self, p: f64) -> f64 {
        let dp = 1.0;
        let p_lo = (p - dp).max(0.0);
        let row_lo = self.interpolate(p_lo);
        let row_hi = self.interpolate(p + dp);
        (row_hi.mu_g_cp - row_lo.mu_g_cp) / (2.0 * dp)
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
                let margin = 5.0;

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

    pub(crate) fn get_d_bo_d_p_for_state(&self, p: f64, rs_sm3_sm3: f64, saturated: bool) -> f64 {
        if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                if saturated {
                    return table.d_bo_sat_d_p(p);
                }
                return table.d_bo_d_p(p, rs_sm3_sm3);
            }
            return table.d_bo_sat_d_p(p);
        }
        -self.pvt.c_o * self.b_o
    }

    pub(crate) fn get_d_bo_d_rs_for_state(&self, p: f64, rs_sm3_sm3: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                return table.d_bo_d_rs(p, rs_sm3_sm3);
            }
        }
        0.0
    }

    pub(crate) fn get_d_bg_d_p_for_state(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            return table.d_bg_d_p(p);
        }
        let _ = p;
        0.0
    }

    pub(crate) fn get_d_rs_sat_d_p_for_state(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                return table.d_rs_sat_d_p(p);
            }
        }
        0.0
    }

    pub(crate) fn get_d_mu_o_d_p_for_state(&self, p: f64, rs_sm3_sm3: f64, saturated: bool) -> f64 {
        if let Some(table) = &self.pvt_table {
            if saturated {
                return table.d_mu_o_sat_d_p(p);
            }
            return table.d_mu_o_d_p(p, rs_sm3_sm3);
        }
        0.0
    }

    pub(crate) fn get_d_mu_o_d_rs_for_state(&self, p: f64, rs_sm3_sm3: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            return table.d_mu_o_d_rs(p, rs_sm3_sm3);
        }
        0.0
    }

    pub(crate) fn get_d_mu_g_d_p_for_state(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            return table.d_mu_g_d_p(p);
        }
        0.0
    }

    pub(crate) fn get_d_rho_o_d_p_for_state(&self, p: f64, rs_sm3_sm3: f64, saturated: bool) -> f64 {
        if self.pvt_table.is_none() {
            return 0.0;
        }

        let bo = self.get_b_o_for_rs(p, rs_sm3_sm3).max(1e-9);
        let rho_o = self.oil_props_for_state(p, rs_sm3_sm3).rho_o_kg_m3;
        let d_bo_d_p = self.get_d_bo_d_p_for_state(p, rs_sm3_sm3, saturated);
        let d_rs_d_p = if saturated {
            self.get_d_rs_sat_d_p_for_state(p)
        } else {
            0.0
        };

        self.rho_g * d_rs_d_p / bo - rho_o * d_bo_d_p / bo
    }

    pub(crate) fn get_d_rho_o_d_rs_for_state(&self, p: f64, rs_sm3_sm3: f64) -> f64 {
        if self.pvt_table.is_none() {
            return 0.0;
        }

        let bo = self.get_b_o_for_rs(p, rs_sm3_sm3).max(1e-9);
        let rho_o = self.oil_props_for_state(p, rs_sm3_sm3).rho_o_kg_m3;
        let d_bo_d_rs = self.get_d_bo_d_rs_for_state(p, rs_sm3_sm3);
        self.rho_g / bo - rho_o * d_bo_d_rs / bo
    }

    pub(crate) fn get_d_rho_g_d_p_for_state(&self, p: f64) -> f64 {
        if self.pvt_table.is_none() {
            return 0.0;
        }

        let bg = self.get_b_g(p).max(1e-9);
        let rho_g = self.get_rho_g(p);
        let d_bg_d_p = self.get_d_bg_d_p_for_state(p);
        -rho_g * d_bg_d_p / bg
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bg_derivative_matches_flat_bg_without_pvt_table() {
        let sim = ReservoirSimulator::new(1, 1, 1, 0.2);

        assert!((sim.get_b_g(250.0) - 1.0).abs() < 1e-12);
        assert!(sim.get_d_bg_d_p_for_state(250.0).abs() < 1e-12);
    }
}
