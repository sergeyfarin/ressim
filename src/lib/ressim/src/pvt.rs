use serde::{Deserialize, Serialize};

use crate::ReservoirSimulator;
use crate::fim::ad::{Ad, Scalar};

/// Generic (differentiable) counterpart of the fields interpolated from the
/// saturated PVT curve. Parameterized by a `Scalar` so the same interpolation
/// serves both the plain `f64` residual and the `Ad<N>` Jacobian path.
pub(crate) struct SatProps<S> {
    pub(crate) rs: S,
    pub(crate) bo: S,
    pub(crate) bg: S,
    pub(crate) mu_o: S,
    pub(crate) mu_g: S,
}

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
    /// OPM/ECL PVDG interpolation is linear in `1/Bg` and `1/(Bg*mu_g)`, not in the
    /// reported `Bg` and `mu_g` columns themselves.
    fn interpolate_gas_segment(r0: &PvtRow, r1: &PvtRow, t: f64) -> (f64, f64) {
        let inv_bg = (1.0 / r0.bg_m3m3) + t * (1.0 / r1.bg_m3m3 - 1.0 / r0.bg_m3m3);
        let inv_bg_mu = (1.0 / (r0.bg_m3m3 * r0.mu_g_cp))
            + t * (1.0 / (r1.bg_m3m3 * r1.mu_g_cp) - 1.0 / (r0.bg_m3m3 * r0.mu_g_cp));
        (1.0 / inv_bg, inv_bg / inv_bg_mu)
    }

    /// OPM/ECL PVTO interpolation is performed on `1/Bo` and `1/(Bo*mu_o)`.
    fn interpolate_oil_segment(r0: &PvtRow, r1: &PvtRow, t: f64) -> (f64, f64) {
        let inv_bo = (1.0 / r0.bo_m3m3) + t * (1.0 / r1.bo_m3m3 - 1.0 / r0.bo_m3m3);
        let inv_bo_mu = (1.0 / (r0.bo_m3m3 * r0.mu_o_cp))
            + t * (1.0 / (r1.bo_m3m3 * r1.mu_o_cp) - 1.0 / (r0.bo_m3m3 * r0.mu_o_cp));
        (1.0 / inv_bo, inv_bo / inv_bo_mu)
    }

    fn interpolate_oil_segment_generic<S: Scalar>(r0: &PvtRow, r1: &PvtRow, t: S) -> (S, S) {
        let inv_bo = t * (1.0 / r1.bo_m3m3 - 1.0 / r0.bo_m3m3) + 1.0 / r0.bo_m3m3;
        let inv_bo_mu = t * (1.0 / (r1.bo_m3m3 * r1.mu_o_cp) - 1.0 / (r0.bo_m3m3 * r0.mu_o_cp))
            + 1.0 / (r0.bo_m3m3 * r0.mu_o_cp);
        (inv_bo.recip(), inv_bo / inv_bo_mu)
    }

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
            branch
                .rows
                .sort_by(|a, b| a.p_bar.partial_cmp(&b.p_bar).unwrap());
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
                let (bo_m3m3, mu_o_cp) = Self::interpolate_oil_segment(r0, r1, t);
                let (bg_m3m3, mu_g_cp) = Self::interpolate_gas_segment(r0, r1, t);
                return PvtRow {
                    p_bar: p,
                    rs_m3m3: r0.rs_m3m3 + t * (r1.rs_m3m3 - r0.rs_m3m3),
                    bo_m3m3,
                    mu_o_cp,
                    bg_m3m3,
                    mu_g_cp,
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
                return Self::interpolate_oil_segment(r0, r1, t);
            }
        }

        // p is beyond the last table point: exponential extrapolation, consistent
        // with the single-row case and with interpolate_rows above bubble point.
        // Linear extrapolation can produce non-physical (negative) Bo values.
        let last = &rows[rows.len() - 1];
        let excess = p - last.p_bar;
        (
            (last.bo_m3m3 * f64::exp(-c_o * excess)).max(1e-9),
            last.mu_o_cp * f64::exp(c_o * excess),
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

    /// Generic mirror of [`Self::interpolate`] over the saturated curve.
    ///
    /// Segment/extrapolation branch is chosen on `p.value()`, so evaluating with
    /// `S = f64` reproduces `interpolate` exactly, while `S = Ad<N>` yields the
    /// exact analytic derivatives of each interpolated field w.r.t. pressure.
    pub(crate) fn interpolate_saturated_generic<S: Scalar>(&self, p: S) -> SatProps<S> {
        let rows = &self.saturated_rows;
        if rows.is_empty() {
            return SatProps {
                rs: S::from_f64(0.0),
                bo: S::from_f64(1.0),
                bg: S::from_f64(1.0),
                mu_o: S::from_f64(1.0),
                mu_g: S::from_f64(0.02),
            };
        }

        let pv = p.value();

        // Below the first table pressure: clamp to the first row (constant).
        if pv <= rows[0].p_bar {
            let r = &rows[0];
            return SatProps {
                rs: S::from_f64(r.rs_m3m3),
                bo: S::from_f64(r.bo_m3m3),
                bg: S::from_f64(r.bg_m3m3),
                mu_o: S::from_f64(r.mu_o_cp),
                mu_g: S::from_f64(r.mu_g_cp),
            };
        }

        let last_idx = rows.len() - 1;
        // Above the last table pressure: exponential Bo, Boyle-law Bg, constant Rs/mu.
        if pv >= rows[last_idx].p_bar {
            let r = &rows[last_idx];
            // Bo = last.Bo * exp(-c_o * (p - last.p))
            let bo = S::from_f64(r.bo_m3m3) * ((p * (-self.c_o)) + (self.c_o * r.p_bar)).exp();
            // Bg = last.Bg * (last.p / p)
            let bg = S::from_f64(r.bg_m3m3 * r.p_bar) / p;
            return SatProps {
                rs: S::from_f64(r.rs_m3m3),
                bo,
                bg,
                mu_o: S::from_f64(r.mu_o_cp),
                mu_g: S::from_f64(r.mu_g_cp),
            };
        }

        for i in 0..last_idx {
            let r0 = &rows[i];
            let r1 = &rows[i + 1];
            if pv >= r0.p_bar && pv <= r1.p_bar {
                let dp = r1.p_bar - r0.p_bar;
                if dp < 1e-6 {
                    return SatProps {
                        rs: S::from_f64(r0.rs_m3m3),
                        bo: S::from_f64(r0.bo_m3m3),
                        bg: S::from_f64(r0.bg_m3m3),
                        mu_o: S::from_f64(r0.mu_o_cp),
                        mu_g: S::from_f64(r0.mu_g_cp),
                    };
                }
                let t = (p - r0.p_bar) / dp;
                let lerp = |a: f64, b: f64| t * (b - a) + a;
                let inv_bg = lerp(1.0 / r0.bg_m3m3, 1.0 / r1.bg_m3m3);
                let inv_bg_mu = lerp(
                    1.0 / (r0.bg_m3m3 * r0.mu_g_cp),
                    1.0 / (r1.bg_m3m3 * r1.mu_g_cp),
                );
                return SatProps {
                    rs: lerp(r0.rs_m3m3, r1.rs_m3m3),
                    bo: Self::interpolate_oil_segment_generic(r0, r1, t).0,
                    bg: S::from_f64(1.0) / inv_bg,
                    mu_o: Self::interpolate_oil_segment_generic(r0, r1, t).1,
                    mu_g: inv_bg / inv_bg_mu,
                };
            }
        }

        let r = &rows[last_idx];
        SatProps {
            rs: S::from_f64(r.rs_m3m3),
            bo: S::from_f64(r.bo_m3m3),
            bg: S::from_f64(r.bg_m3m3),
            mu_o: S::from_f64(r.mu_o_cp),
            mu_g: S::from_f64(r.mu_g_cp),
        }
    }

    /// Generic mirror of [`Self::branch_props`] (Bo, mu_o on a fixed-Rs branch).
    fn branch_props_generic<S: Scalar>(branch: &PvtOilBranch, p: S, c_o: f64) -> (S, S) {
        let rows = &branch.rows;
        if rows.is_empty() {
            return (S::from_f64(1.0), S::from_f64(1.0));
        }
        if rows.len() == 1 {
            let row = &rows[0];
            // Bo = Bo0 * exp(-c_o * (p - p0)) ; mu = mu0 * exp(c_o * (p - p0))
            let excess = p - row.p_bar;
            let bo = S::from_f64(row.bo_m3m3) * (excess * (-c_o)).exp();
            let mu = S::from_f64(row.mu_o_cp) * (excess * c_o).exp();
            return (bo, mu);
        }

        let pv = p.value();
        if pv <= rows[0].p_bar {
            return (S::from_f64(rows[0].bo_m3m3), S::from_f64(rows[0].mu_o_cp));
        }

        for pair in rows.windows(2) {
            let r0 = &pair[0];
            let r1 = &pair[1];
            if pv >= r0.p_bar && pv <= r1.p_bar {
                let dp = r1.p_bar - r0.p_bar;
                if dp.abs() < 1e-9 {
                    return (S::from_f64(r0.bo_m3m3), S::from_f64(r0.mu_o_cp));
                }
                let t = (p - r0.p_bar) / dp;
                return Self::interpolate_oil_segment_generic(r0, r1, t);
            }
        }

        let last = &rows[rows.len() - 1];
        let excess = p - last.p_bar;
        let bo = (S::from_f64(last.bo_m3m3) * (excess * (-c_o)).exp()).max_floor(1e-9);
        let mu = S::from_f64(last.mu_o_cp) * (excess * c_o).exp();
        (bo, mu)
    }

    /// Generic mirror of [`Self::interpolate_oil`] (undersaturation-aware Bo, mu_o).
    pub(crate) fn interpolate_oil_generic<S: Scalar>(&self, p: S, rs: S) -> (S, S) {
        if self.saturated_rows.is_empty() {
            return (S::from_f64(1.0), S::from_f64(1.0));
        }
        let sat = self.interpolate_saturated_generic(p);
        let rs_sat = sat.rs.value();

        if rs.value() >= rs_sat - 1e-6 {
            return (sat.bo, sat.mu_o);
        }

        if let Some((low, high)) = self.branch_bounds(rs.value()) {
            let (bo_low, mu_low) = Self::branch_props_generic(low, p, self.c_o);
            if (high.rs_m3m3 - low.rs_m3m3).abs() <= PVTO_RS_TOLERANCE {
                return (bo_low, mu_low);
            }

            // OPM's live-oil table uses UniformXTabulated2DFunction::LeftExtreme. Across Rs,
            // interpolation follows the saturated-pressure guide instead of evaluating both
            // branches at the same pressure. This preserves the saturated boundary and, at an
            // Rs knot, selects OPM's left-segment derivative convention.
            let t = (rs - low.rs_m3m3) / (high.rs_m3m3 - low.rs_m3m3);
            let pressure_shift = high.rows[0].p_bar - low.rows[0].p_bar;
            let p_low = p - t * pressure_shift;
            let p_high = p + (S::from_f64(1.0) - t) * pressure_shift;
            let (bo_low, mu_low) = Self::branch_props_generic(low, p_low, self.c_o);
            let (bo_high, mu_high) = Self::branch_props_generic(high, p_high, self.c_o);
            let inv_bo = t * (bo_high.recip() - bo_low.recip()) + bo_low.recip();
            let inv_bo_mu = t * ((bo_high * mu_high).recip() - (bo_low * mu_low).recip())
                + (bo_low * mu_low).recip();
            let bo = inv_bo.recip();
            let mu = inv_bo / inv_bo_mu;
            return (bo, mu);
        }

        (sat.bo, sat.mu_o)
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

            let t = (rs - low.rs_m3m3) / (high.rs_m3m3 - low.rs_m3m3);
            let pressure_shift = high.rows[0].p_bar - low.rows[0].p_bar;
            let p_low = p - t * pressure_shift;
            let p_high = p + (1.0 - t) * pressure_shift;
            let (bo_low, mu_low) = Self::branch_props(low, p_low, self.c_o);
            let (bo_high, mu_high) = Self::branch_props(high, p_high, self.c_o);
            let inv_bo = 1.0 / bo_low + t * (1.0 / bo_high - 1.0 / bo_low);
            let inv_bo_mu =
                1.0 / (bo_low * mu_low) + t * (1.0 / (bo_high * mu_high) - 1.0 / (bo_low * mu_low));
            return (1.0 / inv_bo, inv_bo / inv_bo_mu);
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
        self.interpolate_oil_generic(Ad::<1>::variable(p, 0), Ad::<1>::constant(rs))
            .0
            .d(0)
    }

    #[cfg(test)]
    pub(crate) fn d_bo_d_rs(&self, p: f64, rs: f64) -> f64 {
        self.interpolate_oil_generic(Ad::<1>::constant(p), Ad::<1>::variable(rs, 0))
            .0
            .d(0)
    }

    #[cfg(test)]
    pub(crate) fn d_bo_sat_d_p(&self, p: f64) -> f64 {
        self.interpolate_saturated_generic(Ad::<1>::variable(p, 0))
            .bo
            .d(0)
    }

    #[cfg(test)]
    pub(crate) fn d_bg_d_p(&self, p: f64) -> f64 {
        let rows = &self.saturated_rows;
        if rows.is_empty() || p <= rows[0].p_bar {
            return 0.0;
        }
        let last = &rows[rows.len() - 1];
        if p >= last.p_bar {
            return -last.bg_m3m3 * last.p_bar / (p * p);
        }
        for pair in rows.windows(2) {
            let low = &pair[0];
            let high = &pair[1];
            if p >= low.p_bar && p <= high.p_bar {
                let pressure_span = high.p_bar - low.p_bar;
                return if pressure_span.abs() <= f64::EPSILON {
                    0.0
                } else {
                    let inv_bg = 1.0 / low.bg_m3m3
                        + (p - low.p_bar) / pressure_span
                            * (1.0 / high.bg_m3m3 - 1.0 / low.bg_m3m3);
                    let d_inv_bg_d_p = (1.0 / high.bg_m3m3 - 1.0 / low.bg_m3m3) / pressure_span;
                    -d_inv_bg_d_p / (inv_bg * inv_bg)
                };
            }
        }
        0.0
    }

    #[cfg(test)]
    pub(crate) fn d_mu_o_d_p(&self, p: f64, rs: f64) -> f64 {
        self.interpolate_oil_generic(Ad::<1>::variable(p, 0), Ad::<1>::constant(rs))
            .1
            .d(0)
    }

    #[cfg(test)]
    pub(crate) fn d_mu_o_sat_d_p(&self, p: f64) -> f64 {
        self.interpolate_saturated_generic(Ad::<1>::variable(p, 0))
            .mu_o
            .d(0)
    }

    #[cfg(test)]
    pub(crate) fn d_mu_o_d_rs(&self, p: f64, rs: f64) -> f64 {
        self.interpolate_oil_generic(Ad::<1>::constant(p), Ad::<1>::variable(rs, 0))
            .1
            .d(0)
    }

    #[cfg(test)]
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
    /// PVTW inverse formation-volume factor at pressure `p`.
    ///
    /// Matches OPM's constant-compressibility water polynomial
    /// `(1 + X*(1 + X/2))/Bw_ref`, `X=c_w*(p-p_ref)`.
    pub(crate) fn water_inverse_fvf_generic<S: Scalar>(&self, p: S) -> S {
        let x = (p - self.water_pvt_reference_pressure_bar) * self.pvt.c_w;
        (S::from_f64(1.0) + x * (S::from_f64(1.0) + x * 0.5)) / self.b_w.max(1e-9)
    }

    pub(crate) fn water_inverse_fvf(&self, p: f64) -> f64 {
        self.water_inverse_fvf_generic(p)
    }

    pub(crate) fn water_fvf(&self, p: f64) -> f64 {
        self.water_inverse_fvf(p).max(1e-9).recip()
    }

    pub(crate) fn water_density_generic<S: Scalar>(&self, p: S) -> S {
        self.water_inverse_fvf_generic(p) * self.pvt.rho_w
    }

    fn base_oil_fvf(&self, p: f64) -> f64 {
        (self.b_o * f64::exp(-self.pvt.c_o * p)).max(1e-9)
    }

    fn base_oil_density(&self, p: f64) -> f64 {
        self.pvt.rho_o / self.base_oil_fvf(p)
    }

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

    /// Generic (differentiable) mirror of [`Self::get_mu_o_for_rs`].
    pub(crate) fn get_mu_o_for_rs_generic<S: Scalar>(&self, p: S, rs_sm3_sm3: S) -> S {
        if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                let (_, mu) = table.interpolate_oil_generic(p, rs_sm3_sm3);
                return mu;
            }
            table.interpolate_saturated_generic(p).mu_o
        } else {
            S::from_f64(self.pvt.mu_o)
        }
    }

    /// Generic (differentiable) mirror of [`Self::get_mu_g`].
    pub(crate) fn get_mu_g_generic<S: Scalar>(&self, p: S) -> S {
        if let Some(table) = &self.pvt_table {
            table.interpolate_saturated_generic(p).mu_g
        } else {
            S::from_f64(self.mu_g)
        }
    }

    pub(crate) fn get_c_o(&self, _p: f64) -> f64 {
        // Called in two-phase mode only (three-phase uses get_c_o_effective instead).
        // Reading dBo/dp from the saturated curve conflates oil compressibility with
        // changing Rs along the bubble-point locus, overestimating undersaturated c_o.
        // The scalar undersaturated c_o is the correct value here.
        self.pvt.c_o
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
            self.base_oil_fvf(p)
        }
    }

    pub(crate) fn get_rho_o_cell(&self, id: usize, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let rs = self.rs[id];
            let (bo, _) = table.interpolate_oil(p, rs);
            (self.pvt.rho_o + rs * self.rho_g) / bo
        } else {
            self.base_oil_density(p)
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
            self.base_oil_fvf(p)
        }
    }

    #[allow(dead_code)]
    pub(crate) fn get_rho_o_for_rs(&self, p: f64, rs_sm3_sm3: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let props = table.oil_props_at(p, rs_sm3_sm3, self.pvt.rho_o, self.rho_g);
            props.rho_o_kg_m3
        } else {
            self.base_oil_density(p)
        }
    }

    pub(crate) fn get_rho_o(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let row = table.interpolate(p);
            (self.pvt.rho_o + row.rs_m3m3 * self.rho_g) / row.bo_m3m3
        } else {
            self.base_oil_density(p)
        }
    }

    pub(crate) fn get_rho_g(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            self.rho_g / table.interpolate(p).bg_m3m3
        } else {
            self.rho_g
        }
    }

    pub(crate) fn get_rho_w(&self, p: f64) -> f64 {
        self.water_density_generic(p)
    }

    pub(crate) fn get_b_g(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            table.interpolate(p).bg_m3m3
        } else {
            1.0
        }
    }

    #[cfg(test)]
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
        -self.pvt.c_o * self.base_oil_fvf(p)
    }

    #[cfg(test)]
    pub(crate) fn get_d_bo_d_rs_for_state(&self, p: f64, rs_sm3_sm3: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                return table.d_bo_d_rs(p, rs_sm3_sm3);
            }
        }
        0.0
    }

    #[cfg(test)]
    pub(crate) fn get_d_bg_d_p_for_state(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            return table.d_bg_d_p(p);
        }
        let _ = p;
        0.0
    }

    #[cfg(test)]
    pub(crate) fn get_d_rs_sat_d_p_for_state(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                return table.d_rs_sat_d_p(p);
            }
        }
        0.0
    }

    #[cfg(test)]
    pub(crate) fn get_d_mu_o_d_p_for_state(&self, p: f64, rs_sm3_sm3: f64, saturated: bool) -> f64 {
        if let Some(table) = &self.pvt_table {
            if saturated {
                return table.d_mu_o_sat_d_p(p);
            }
            return table.d_mu_o_d_p(p, rs_sm3_sm3);
        }
        0.0
    }

    #[cfg(test)]
    pub(crate) fn get_d_mu_o_d_rs_for_state(&self, p: f64, rs_sm3_sm3: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            return table.d_mu_o_d_rs(p, rs_sm3_sm3);
        }
        0.0
    }

    #[cfg(test)]
    pub(crate) fn get_d_mu_g_d_p_for_state(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            return table.d_mu_g_d_p(p);
        }
        0.0
    }

    #[cfg(test)]
    pub(crate) fn get_d_rho_o_d_p_for_state(
        &self,
        p: f64,
        rs_sm3_sm3: f64,
        saturated: bool,
    ) -> f64 {
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

    #[cfg(test)]
    pub(crate) fn get_d_rho_o_d_rs_for_state(&self, p: f64, rs_sm3_sm3: f64) -> f64 {
        if self.pvt_table.is_none() {
            return 0.0;
        }

        let bo = self.get_b_o_for_rs(p, rs_sm3_sm3).max(1e-9);
        let rho_o = self.oil_props_for_state(p, rs_sm3_sm3).rho_o_kg_m3;
        let d_bo_d_rs = self.get_d_bo_d_rs_for_state(p, rs_sm3_sm3);
        self.rho_g / bo - rho_o * d_bo_d_rs / bo
    }

    #[cfg(test)]
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
                bo_m3m3: self.base_oil_fvf(p),
                mu_o_cp: self.pvt.mu_o,
                rho_o_kg_m3: self.base_oil_density(p),
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

    /// Generic (differentiable) mirror of [`Self::oil_props_for_state`], oil
    /// mass density only (the field the flux gravity term needs).
    pub(crate) fn oil_density_generic<S: Scalar>(&self, p: S, rs: S) -> S {
        if let Some(table) = &self.pvt_table {
            let (bo, _mu) = table.interpolate_oil_generic(p, rs);
            (S::from_f64(self.pvt.rho_o) + rs * self.rho_g) / bo.max_floor(1e-9)
        } else {
            let bo = (S::from_f64(self.b_o) * (p * (-self.pvt.c_o)).exp()).max_floor(1e-9);
            S::from_f64(self.pvt.rho_o) / bo
        }
    }

    /// Generic (differentiable) mirror of [`Self::gas_props_for_state`], gas
    /// mass density only.
    pub(crate) fn gas_density_generic<S: Scalar>(&self, p: S) -> S {
        if let Some(table) = &self.pvt_table {
            let bg = table.interpolate_saturated_generic(p).bg;
            S::from_f64(self.rho_g) / bg.max_floor(1e-9)
        } else {
            S::from_f64(self.rho_g)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_table_oil_props_respect_base_compressibility() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.b_o = 1.0;
        sim.pvt.c_o = 1e-5;

        let oil_lo = sim.oil_props_for_state(100.0, 0.0);
        let oil_hi = sim.oil_props_for_state(300.0, 0.0);

        assert!(oil_hi.bo_m3m3 < oil_lo.bo_m3m3);
        assert!(oil_hi.rho_o_kg_m3 > oil_lo.rho_o_kg_m3);

        let expected_bo_hi = f64::exp(-sim.pvt.c_o * 300.0);
        assert!((oil_hi.bo_m3m3 - expected_bo_hi).abs() < 1e-12);

        let derivative = sim.get_d_bo_d_p_for_state(300.0, 0.0, false);
        assert!((derivative + sim.pvt.c_o * oil_hi.bo_m3m3).abs() < 1e-12);
    }

    #[test]
    fn pvtw_inverse_fvf_uses_reference_pressure_and_quadratic_compressibility() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_initial_pressure(200.0);
        sim.set_fluid_compressibilities(1e-5, 5e-5).unwrap();
        sim.set_rock_properties(0.0, 0.0, 1.0, 1.2).unwrap();

        assert!((sim.water_inverse_fvf(200.0) - 1.0 / 1.2).abs() < 1e-14);
        let x = 5e-5 * 50.0;
        let expected = (1.0 + x * (1.0 + x / 2.0)) / 1.2;
        assert!((sim.water_inverse_fvf(250.0) - expected).abs() < 1e-14);
        assert!((sim.get_rho_w(250.0) - sim.pvt.rho_w * expected).abs() < 1e-12);
    }

    #[test]
    fn pvto_interpolates_inverse_bo_and_inverse_bo_mu_in_pressure_and_rs() {
        let table = PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 50.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.10,
                    mu_o_cp: 1.00,
                    bg_m3m3: 0.02,
                    mu_g_cp: 0.015,
                },
                PvtRow {
                    p_bar: 350.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.0675,
                    mu_o_cp: 1.030,
                    bg_m3m3: 0.02,
                    mu_g_cp: 0.015,
                },
                PvtRow {
                    p_bar: 150.0,
                    rs_m3m3: 80.0,
                    bo_m3m3: 1.25,
                    mu_o_cp: 0.70,
                    bg_m3m3: 0.008,
                    mu_g_cp: 0.018,
                },
                PvtRow {
                    p_bar: 350.0,
                    rs_m3m3: 80.0,
                    bo_m3m3: 1.2252,
                    mu_o_cp: 0.714,
                    bg_m3m3: 0.008,
                    mu_g_cp: 0.018,
                },
            ],
            1e-4,
        );

        let (low_bo, low_mu) = PvtTable::interpolate_oil_segment(
            &table.oil_branches[0].rows[0],
            &table.oil_branches[0].rows[1],
            1.0 / 3.0,
        );
        let (high_bo, high_mu) = PvtTable::interpolate_oil_segment(
            &table.oil_branches[1].rows[0],
            &table.oil_branches[1].rows[1],
            0.5,
        );
        let rs_t = 0.5;
        let inv_bo = 1.0 / low_bo + rs_t * (1.0 / high_bo - 1.0 / low_bo);
        let inv_bo_mu =
            1.0 / (low_bo * low_mu) + rs_t * (1.0 / (high_bo * high_mu) - 1.0 / (low_bo * low_mu));
        let expected = (1.0 / inv_bo, inv_bo / inv_bo_mu);
        let scalar = table.interpolate_oil(200.0, 50.0);
        let generic = table.interpolate_oil_generic(200.0_f64, 50.0_f64);

        assert!((scalar.0 - expected.0).abs() < 1e-14);
        assert!((scalar.1 - expected.1).abs() < 1e-14);
        assert!((generic.0 - scalar.0).abs() < 1e-14);
        assert!((generic.1 - scalar.1).abs() < 1e-14);
    }

    #[test]
    fn pvto_rs_knot_uses_left_extreme_guided_derivative() {
        let table = PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 50.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.10,
                    mu_o_cp: 1.00,
                    bg_m3m3: 0.02,
                    mu_g_cp: 0.015,
                },
                PvtRow {
                    p_bar: 350.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.0675,
                    mu_o_cp: 1.030,
                    bg_m3m3: 0.02,
                    mu_g_cp: 0.015,
                },
                PvtRow {
                    p_bar: 150.0,
                    rs_m3m3: 80.0,
                    bo_m3m3: 1.25,
                    mu_o_cp: 0.70,
                    bg_m3m3: 0.008,
                    mu_g_cp: 0.018,
                },
                PvtRow {
                    p_bar: 350.0,
                    rs_m3m3: 80.0,
                    bo_m3m3: 1.2252,
                    mu_o_cp: 0.714,
                    bg_m3m3: 0.008,
                    mu_g_cp: 0.018,
                },
                PvtRow {
                    p_bar: 250.0,
                    rs_m3m3: 140.0,
                    bo_m3m3: 1.40,
                    mu_o_cp: 0.50,
                    bg_m3m3: 0.005,
                    mu_g_cp: 0.022,
                },
                PvtRow {
                    p_bar: 350.0,
                    rs_m3m3: 140.0,
                    bo_m3m3: 1.3861,
                    mu_o_cp: 0.505,
                    bg_m3m3: 0.005,
                    mu_g_cp: 0.022,
                },
            ],
            1e-4,
        );

        let ad =
            table.interpolate_oil_generic(Ad::<1>::constant(200.0), Ad::<1>::variable(80.0, 0));
        let h = 1e-4;
        let at_knot = table.interpolate_oil(200.0, 80.0);
        let from_left = table.interpolate_oil(200.0, 80.0 - h);
        let fd_bo = (at_knot.0 - from_left.0) / h;
        let fd_mu = (at_knot.1 - from_left.1) / h;

        assert!((ad.0.d(0) - fd_bo).abs() < 1e-8);
        assert!((ad.1.d(0) - fd_mu).abs() < 1e-8);
    }

    #[test]
    fn bg_derivative_matches_flat_bg_without_pvt_table() {
        let sim = ReservoirSimulator::new(1, 1, 1, 0.2);

        assert!((sim.get_b_g(250.0) - 1.0).abs() < 1e-12);
        assert!(sim.get_d_bg_d_p_for_state(250.0).abs() < 1e-12);
    }

    #[test]
    fn table_bg_derivative_uses_the_same_active_segment_as_generic_interpolation() {
        let table = PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 100.0,
                    rs_m3m3: 10.0,
                    bo_m3m3: 1.2,
                    mu_o_cp: 1.0,
                    bg_m3m3: 0.01,
                    mu_g_cp: 0.02,
                },
                PvtRow {
                    p_bar: 200.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.1,
                    mu_o_cp: 1.0,
                    bg_m3m3: 0.0065,
                    mu_g_cp: 0.02,
                },
                PvtRow {
                    p_bar: 300.0,
                    rs_m3m3: 30.0,
                    bo_m3m3: 1.0,
                    mu_o_cp: 1.0,
                    bg_m3m3: 0.005,
                    mu_g_cp: 0.02,
                },
            ],
            1.0e-5,
        );

        let expected = |p: f64, low_p: f64, low_bg: f64, high_p: f64, high_bg: f64| {
            let slope = (1.0 / high_bg - 1.0 / low_bg) / (high_p - low_p);
            let inv_bg = 1.0 / low_bg + (p - low_p) * slope;
            -slope / (inv_bg * inv_bg)
        };
        assert!(
            (table.d_bg_d_p(200.0) - expected(200.0, 100.0, 0.01, 200.0, 0.0065)).abs() < 1e-15
        );
        assert!(
            (table.d_bg_d_p(250.0) - expected(250.0, 200.0, 0.0065, 300.0, 0.005)).abs() < 1e-15
        );
        assert!((table.d_bg_d_p(300.0) + 0.005 / 300.0).abs() < 1e-15);
    }

    #[test]
    fn pvdg_segment_interpolates_inverse_bg_and_inverse_bg_mu() {
        let table = PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 150.0,
                    rs_m3m3: 10.0,
                    bo_m3m3: 1.0,
                    mu_o_cp: 1.0,
                    bg_m3m3: 0.008,
                    mu_g_cp: 0.018,
                },
                PvtRow {
                    p_bar: 250.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.0,
                    mu_o_cp: 1.0,
                    bg_m3m3: 0.005,
                    mu_g_cp: 0.022,
                },
            ],
            1.0e-5,
        );

        let row = table.interpolate(200.0);
        let expected_inv_bg = 0.5 * (1.0 / 0.008 + 1.0 / 0.005);
        let expected_inv_bg_mu = 0.5 * (1.0 / (0.008 * 0.018) + 1.0 / (0.005 * 0.022));
        assert!((row.bg_m3m3 - 1.0 / expected_inv_bg).abs() < 1e-15);
        assert!((row.mu_g_cp - expected_inv_bg / expected_inv_bg_mu).abs() < 1e-15);

        let generic = table.interpolate_saturated_generic(200.0_f64);
        assert!((generic.bg - row.bg_m3m3).abs() < 1e-15);
        assert!((generic.mu_g - row.mu_g_cp).abs() < 1e-15);
    }
}
