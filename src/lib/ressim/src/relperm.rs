use serde::{Deserialize, Serialize};

use crate::fim::ad::Scalar;

/// Generic mirror of [`interpolate_piecewise`] over a differentiable scalar.
/// The segment is chosen from `x.value()` (matching the f64 branch exactly),
/// then the interpolated value is computed with `S` arithmetic so `Ad<N>`
/// carries the correct chain-rule derivative through the active segment.
fn interpolate_piecewise_generic<S: Scalar, T>(
    rows: &[T],
    x: S,
    x_of: fn(&T) -> f64,
    y_of: fn(&T) -> f64,
) -> S {
    if rows.is_empty() {
        return S::from_f64(0.0);
    }
    let xv = x.value();
    if xv <= x_of(&rows[0]) {
        return S::from_f64(y_of(&rows[0]));
    }
    for pair in rows.windows(2) {
        let x0 = x_of(&pair[0]);
        let x1 = x_of(&pair[1]);
        if xv <= x1 {
            if (x1 - x0).abs() <= f64::EPSILON {
                return S::from_f64(y_of(&pair[1]));
            }
            let fraction = ((x - x0) / (x1 - x0)).max_floor(0.0).min_ceil(1.0);
            let y0 = y_of(&pair[0]);
            let y1 = y_of(&pair[1]);
            return fraction * (y1 - y0) + y0;
        }
    }
    if let Some(last) = rows.last() {
        S::from_f64(y_of(last))
    } else {
        S::from_f64(0.0)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SwofRow {
    pub sw: f64,
    pub krw: f64,
    pub krow: f64,
    pub pcow: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SgofRow {
    pub sg: f64,
    pub krg: f64,
    pub krog: f64,
    pub pcog: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ThreePhaseScalTables {
    pub swof: Vec<SwofRow>,
    pub sgof: Vec<SgofRow>,
}

impl ThreePhaseScalTables {
    pub fn validate(&self) -> Result<(), String> {
        validate_swof_rows(&self.swof)?;
        validate_sgof_rows(&self.sgof)?;
        Ok(())
    }
}

fn validate_swof_rows(rows: &[SwofRow]) -> Result<(), String> {
    if rows.len() < 2 {
        return Err("SWOF table must contain at least two rows".to_string());
    }
    let mut previous_sw = -f64::INFINITY;
    for (index, row) in rows.iter().enumerate() {
        if !row.sw.is_finite() || !row.krw.is_finite() || !row.krow.is_finite() {
            return Err(format!("SWOF row {} must contain finite values", index));
        }
        if !(0.0..=1.0).contains(&row.sw)
            || !(0.0..=1.0).contains(&row.krw)
            || !(0.0..=1.0).contains(&row.krow)
        {
            return Err(format!("SWOF row {} must stay within [0, 1]", index));
        }
        if row.sw <= previous_sw {
            return Err(format!(
                "SWOF saturation must be strictly increasing at row {}",
                index
            ));
        }
        previous_sw = row.sw;
    }
    Ok(())
}

fn validate_sgof_rows(rows: &[SgofRow]) -> Result<(), String> {
    if rows.len() < 2 {
        return Err("SGOF table must contain at least two rows".to_string());
    }
    let mut previous_sg = -f64::INFINITY;
    for (index, row) in rows.iter().enumerate() {
        if !row.sg.is_finite() || !row.krg.is_finite() || !row.krog.is_finite() {
            return Err(format!("SGOF row {} must contain finite values", index));
        }
        if !(0.0..=1.0).contains(&row.sg)
            || !(0.0..=1.0).contains(&row.krg)
            || !(0.0..=1.0).contains(&row.krog)
        {
            return Err(format!("SGOF row {} must stay within [0, 1]", index));
        }
        if row.sg <= previous_sg {
            return Err(format!(
                "SGOF saturation must be strictly increasing at row {}",
                index
            ));
        }
        previous_sg = row.sg;
    }
    Ok(())
}

fn interpolate_piecewise<T>(rows: &[T], x: f64, x_of: fn(&T) -> f64, y_of: fn(&T) -> f64) -> f64 {
    if rows.is_empty() {
        return 0.0;
    }
    if x <= x_of(&rows[0]) {
        return y_of(&rows[0]);
    }
    for pair in rows.windows(2) {
        let x0 = x_of(&pair[0]);
        let x1 = x_of(&pair[1]);
        if x <= x1 {
            if (x1 - x0).abs() <= f64::EPSILON {
                return y_of(&pair[1]);
            }
            let fraction = ((x - x0) / (x1 - x0)).clamp(0.0, 1.0);
            return y_of(&pair[0]) + fraction * (y_of(&pair[1]) - y_of(&pair[0]));
        }
    }
    if let Some(last) = rows.last() {
        y_of(last)
    } else {
        0.0
    }
}

fn interpolate_piecewise_slope<T>(
    rows: &[T],
    x: f64,
    x_of: fn(&T) -> f64,
    y_of: fn(&T) -> f64,
) -> f64 {
    if rows.len() < 2 {
        return 0.0;
    }
    if x <= x_of(&rows[0]) {
        let x0 = x_of(&rows[0]);
        let x1 = x_of(&rows[1]);
        let y0 = y_of(&rows[0]);
        let y1 = y_of(&rows[1]);
        let dx = x1 - x0;
        return if dx.abs() > f64::EPSILON {
            (y1 - y0) / dx
        } else {
            0.0
        };
    }
    for pair in rows.windows(2) {
        let x0 = x_of(&pair[0]);
        let x1 = x_of(&pair[1]);
        if x <= x1 {
            let dx = x1 - x0;
            return if dx.abs() > f64::EPSILON {
                (y_of(&pair[1]) - y_of(&pair[0])) / dx
            } else {
                0.0
            };
        }
    }
    let last = rows.len() - 1;
    let x0 = x_of(&rows[last - 1]);
    let x1 = x_of(&rows[last]);
    let y0 = y_of(&rows[last - 1]);
    let y1 = y_of(&rows[last]);
    let dx = x1 - x0;
    if dx.abs() > f64::EPSILON {
        (y1 - y0) / dx
    } else {
        0.0
    }
}

/// Three-phase rock/fluid properties for Stone II relative permeability model.
/// Oil-water Corey parameters are the same form as `RockFluidProps` (2-phase).
/// Gas Corey parameters are independent.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RockFluidPropsThreePhase {
    // ── Oil-water system ──────────────────────────────────────────────────────
    /// Connate water saturation [dimensionless]
    pub s_wc: f64,
    /// Residual oil saturation (oil-water system) [dimensionless]
    pub s_or: f64,
    /// Corey exponent for water [dimensionless]
    pub n_w: f64,
    /// Corey exponent for oil (oil-water system) [dimensionless]
    pub n_o: f64,
    /// Max water relative permeability at Sw = 1 − Sor [dimensionless]
    pub k_rw_max: f64,
    /// Max oil relative permeability at Sw = Swc [dimensionless]
    pub k_ro_max: f64,

    // ── Gas system ────────────────────────────────────────────────────────────
    /// Critical gas saturation (min Sg for gas to flow) [dimensionless]
    pub s_gc: f64,
    /// Residual (trapped) gas saturation after imbibition [dimensionless]
    pub s_gr: f64,
    /// Residual oil saturation in a gas flood (typically > s_or) [dimensionless]
    /// Used as the terminal oil saturation in k_ro_gas and gas-oil capillary pressure.
    pub s_org: f64,
    /// Corey exponent for gas [dimensionless]
    pub n_g: f64,
    /// Max gas relative permeability at So = Sorg [dimensionless]
    pub k_rg_max: f64,
    /// Optional exact tabular SWOF/SGOF data.
    pub tables: Option<ThreePhaseScalTables>,
}

impl RockFluidPropsThreePhase {
    /// Water relative permeability — Corey-Brooks (same formula as 2-phase).
    pub fn k_rw(&self, s_w: f64) -> f64 {
        if let Some(tables) = &self.tables {
            return interpolate_piecewise(&tables.swof, s_w, |row| row.sw, |row| row.krw);
        }
        let s_eff = ((s_w - self.s_wc) / (1.0 - self.s_wc - self.s_or)).clamp(0.0, 1.0);
        self.k_rw_max * s_eff.powf(self.n_w)
    }

    pub fn d_k_rw_d_sw(&self, s_w: f64) -> f64 {
        if let Some(tables) = &self.tables {
            return interpolate_piecewise_slope(&tables.swof, s_w, |row| row.sw, |row| row.krw);
        }
        let denom = 1.0 - self.s_wc - self.s_or;
        if denom <= 0.0 {
            return 0.0;
        }
        let s_eff = (s_w - self.s_wc) / denom;
        if !(0.0..1.0).contains(&s_eff) {
            return 0.0;
        }
        self.k_rw_max * self.n_w * s_eff.powf(self.n_w - 1.0) / denom
    }

    /// Gas relative permeability — Corey-Brooks.
    /// S_g_eff = (S_g − S_gc) / (1 − S_wc − S_gc − S_gr)
    pub fn k_rg(&self, s_g: f64) -> f64 {
        if let Some(tables) = &self.tables {
            return interpolate_piecewise(&tables.sgof, s_g, |row| row.sg, |row| row.krg);
        }
        let denom = 1.0 - self.s_wc - self.s_gc - self.s_gr;
        if denom <= 0.0 {
            return 0.0;
        }
        let s_eff = ((s_g - self.s_gc) / denom).clamp(0.0, 1.0);
        self.k_rg_max * s_eff.powf(self.n_g)
    }

    pub fn d_k_rg_d_sg(&self, s_g: f64) -> f64 {
        if let Some(tables) = &self.tables {
            return interpolate_piecewise_slope(&tables.sgof, s_g, |row| row.sg, |row| row.krg);
        }
        let denom = 1.0 - self.s_wc - self.s_gc - self.s_gr;
        if denom <= 0.0 {
            return 0.0;
        }
        let s_eff = (s_g - self.s_gc) / denom;
        if !(0.0..1.0).contains(&s_eff) {
            return 0.0;
        }
        self.k_rg_max * self.n_g * s_eff.powf(self.n_g - 1.0) / denom
    }

    /// Oil relative permeability in oil-water 2-phase system (Sg = 0).
    /// k_ro_w = k_ro_max * ((1 − Sw − Sor) / (1 − Swc − Sor))^no
    pub fn k_ro_water(&self, s_w: f64) -> f64 {
        if let Some(tables) = &self.tables {
            return interpolate_piecewise(&tables.swof, s_w, |row| row.sw, |row| row.krow);
        }
        let s_eff = ((1.0 - s_w - self.s_or) / (1.0 - self.s_wc - self.s_or)).clamp(0.0, 1.0);
        self.k_ro_max * s_eff.powf(self.n_o)
    }

    pub fn d_k_ro_water_d_sw(&self, s_w: f64) -> f64 {
        if let Some(tables) = &self.tables {
            return interpolate_piecewise_slope(&tables.swof, s_w, |row| row.sw, |row| row.krow);
        }
        let denom = 1.0 - self.s_wc - self.s_or;
        if denom <= 0.0 {
            return 0.0;
        }
        let s_eff = (1.0 - s_w - self.s_or) / denom;
        if !(0.0..1.0).contains(&s_eff) {
            return 0.0;
        }
        -self.k_ro_max * self.n_o * s_eff.powf(self.n_o - 1.0) / denom
    }

    /// Oil relative permeability in oil-gas 2-phase system (Sw = Swc).
    /// S_o_eff = (1 − Swc − Sg − Sorg) / (1 − Swc − Sorg)
    /// Uses s_org (residual oil to gas) as the terminal saturation, not s_gr (trapped gas).
    pub fn k_ro_gas(&self, s_g: f64) -> f64 {
        if let Some(tables) = &self.tables {
            return interpolate_piecewise(&tables.sgof, s_g, |row| row.sg, |row| row.krog);
        }
        let denom = 1.0 - self.s_wc - self.s_org;
        if denom <= 0.0 {
            return 0.0;
        }
        let s_eff = ((1.0 - self.s_wc - s_g - self.s_org) / denom).clamp(0.0, 1.0);
        self.k_ro_max * s_eff.powf(self.n_o)
    }

    pub fn d_k_ro_gas_d_sg(&self, s_g: f64) -> f64 {
        if let Some(tables) = &self.tables {
            return interpolate_piecewise_slope(&tables.sgof, s_g, |row| row.sg, |row| row.krog);
        }
        let denom = 1.0 - self.s_wc - self.s_org;
        if denom <= 0.0 {
            return 0.0;
        }
        let s_eff = (1.0 - self.s_wc - s_g - self.s_org) / denom;
        if !(0.0..1.0).contains(&s_eff) {
            return 0.0;
        }
        -self.k_ro_max * self.n_o * s_eff.powf(self.n_o - 1.0) / denom
    }

    /// Three-phase oil relative permeability — Stone II model.
    /// k_ro = k_ro_max * [ (k_ro_w/k_ro_max + k_rw) * (k_ro_g/k_ro_max + k_rg) − k_rw − k_rg ]
    /// Clamped to [0, k_ro_max].
    pub fn k_ro_stone2(&self, s_w: f64, s_g: f64) -> f64 {
        let kro_max = self.k_ro_max;
        let kro_w = self.k_ro_water(s_w);
        let kro_g = self.k_ro_gas(s_g);
        let krw = self.k_rw(s_w);
        let krg = self.k_rg(s_g);
        let val = kro_max * ((kro_w / kro_max + krw) * (kro_g / kro_max + krg) - krw - krg);
        val.clamp(0.0, kro_max)
    }

    pub fn d_k_ro_stone2_d_sw(&self, s_w: f64, s_g: f64) -> f64 {
        let kro_max = self.k_ro_max;
        let kro_w = self.k_ro_water(s_w);
        let kro_g = self.k_ro_gas(s_g);
        let krw = self.k_rw(s_w);
        let krg = self.k_rg(s_g);
        let val = kro_max * ((kro_w / kro_max + krw) * (kro_g / kro_max + krg) - krw - krg);
        if val <= 0.0 || val >= kro_max {
            return 0.0;
        }

        let d_kro_w = self.d_k_ro_water_d_sw(s_w);
        let d_krw = self.d_k_rw_d_sw(s_w);
        let b = kro_g / kro_max + krg;
        kro_max * ((d_kro_w / kro_max + d_krw) * b - d_krw)
    }

    pub fn d_k_ro_stone2_d_sg(&self, s_w: f64, s_g: f64) -> f64 {
        let kro_max = self.k_ro_max;
        let kro_w = self.k_ro_water(s_w);
        let kro_g = self.k_ro_gas(s_g);
        let krw = self.k_rw(s_w);
        let krg = self.k_rg(s_g);
        let val = kro_max * ((kro_w / kro_max + krw) * (kro_g / kro_max + krg) - krw - krg);
        if val <= 0.0 || val >= kro_max {
            return 0.0;
        }

        let a = kro_w / kro_max + krw;
        let d_kro_g = self.d_k_ro_gas_d_sg(s_g);
        let d_krg = self.d_k_rg_d_sg(s_g);
        kro_max * (a * (d_kro_g / kro_max + d_krg) - d_krg)
    }

    // ── Generic (differentiable) mirrors for AD Jacobian assembly ──────────────
    // Each mirrors the f64 method above exactly (same clamp/branch structure),
    // instantiated with `S = f64` for bitwise parity or `S = Ad<N>` for the
    // exact analytic derivative through the active Corey/tabular segment.

    pub(crate) fn k_rw_generic<S: Scalar>(&self, s_w: S) -> S {
        if let Some(tables) = &self.tables {
            return interpolate_piecewise_generic(&tables.swof, s_w, |row| row.sw, |row| row.krw);
        }
        let denom = 1.0 - self.s_wc - self.s_or;
        let s_eff = ((s_w - self.s_wc) / denom).max_floor(0.0).min_ceil(1.0);
        s_eff.powf(self.n_w) * self.k_rw_max
    }

    pub(crate) fn k_rg_generic<S: Scalar>(&self, s_g: S) -> S {
        if let Some(tables) = &self.tables {
            return interpolate_piecewise_generic(&tables.sgof, s_g, |row| row.sg, |row| row.krg);
        }
        let denom = 1.0 - self.s_wc - self.s_gc - self.s_gr;
        let s_eff = ((s_g - self.s_gc) / denom).max_floor(0.0).min_ceil(1.0);
        s_eff.powf(self.n_g) * self.k_rg_max
    }

    pub(crate) fn k_ro_water_generic<S: Scalar>(&self, s_w: S) -> S {
        if let Some(tables) = &self.tables {
            return interpolate_piecewise_generic(&tables.swof, s_w, |row| row.sw, |row| row.krow);
        }
        let denom = 1.0 - self.s_wc - self.s_or;
        let s_eff = ((S::from_f64(1.0 - self.s_or) - s_w) / denom)
            .max_floor(0.0)
            .min_ceil(1.0);
        s_eff.powf(self.n_o) * self.k_ro_max
    }

    pub(crate) fn k_ro_gas_generic<S: Scalar>(&self, s_g: S) -> S {
        if let Some(tables) = &self.tables {
            return interpolate_piecewise_generic(&tables.sgof, s_g, |row| row.sg, |row| row.krog);
        }
        let denom = 1.0 - self.s_wc - self.s_org;
        let s_eff = ((S::from_f64(1.0 - self.s_wc - self.s_org) - s_g) / denom)
            .max_floor(0.0)
            .min_ceil(1.0);
        s_eff.powf(self.n_o) * self.k_ro_max
    }

    pub(crate) fn k_ro_stone2_generic<S: Scalar>(&self, s_w: S, s_g: S) -> S {
        let kro_max = self.k_ro_max;
        let kro_w = self.k_ro_water_generic(s_w);
        let kro_g = self.k_ro_gas_generic(s_g);
        let krw = self.k_rw_generic(s_w);
        let krg = self.k_rg_generic(s_g);
        let val = ((kro_w / kro_max + krw) * (kro_g / kro_max + krg) - krw - krg) * kro_max;
        // Mirror `d_k_ro_stone2_d_sw` / `d_k_ro_stone2_d_sg`'s explicit
        // boundary guard (`if val <= 0.0 || val >= kro_max { return 0.0; }`):
        // legacy treats the derivative as exactly zero AT OR BEYOND either
        // clamp bound, not just strictly beyond it. The generic
        // `.max_floor(0.0).min_ceil(kro_max)` combinators keep a live
        // derivative exactly AT a bound (value equality resolves to "still on
        // the branch"), which disagrees with legacy precisely at physically
        // common corners -- e.g. Sw = Swc and Sg = 0 simultaneously (a
        // standard initial condition), where both relperms saturate to their
        // max and `val` lands exactly on `kro_max`. Match legacy's
        // zero-derivative-at-the-bound convention explicitly; the VALUE is
        // unaffected (still `val.value().clamp(0.0, kro_max)` bit-for-bit).
        if val.value() <= 0.0 || val.value() >= kro_max {
            S::from_f64(val.value().clamp(0.0, kro_max))
        } else {
            val
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RockFluidProps {
    /// Connate water saturation (irreducible water) [dimensionless]
    pub s_wc: f64,
    /// Residual oil saturation [dimensionless]
    pub s_or: f64,
    /// Corey exponent for water relative permeability [dimensionless]
    pub n_w: f64,
    /// Corey exponent for oil relative permeability [dimensionless]
    pub n_o: f64,
    /// Maximum water relative permeability at Sw = 1 - Sor [dimensionless]
    pub k_rw_max: f64,
    /// Maximum oil relative permeability at Sw = Swc [dimensionless]
    pub k_ro_max: f64,
}

impl RockFluidProps {
    pub(crate) fn default_scal() -> Self {
        // Reduced saturation thresholds to allow better water flow at initial conditions
        // s_wc: connate water saturation (irreducible water that doesn't flow)
        // s_or: residual oil saturation (oil left after water breakthrough)
        Self {
            s_wc: 0.1,
            s_or: 0.1,
            n_w: 2.0,
            n_o: 2.0,
            k_rw_max: 1.0,
            k_ro_max: 1.0,
        }
    }

    /// Water relative permeability [dimensionless] using Corey-Brooks correlation
    /// k_rw(Sw) = krw_max * ((Sw - Swc) / (1 - Swc - Sor))^nw
    /// Returns 0 for Sw <= Swc, krw_max for Sw >= 1-Sor
    pub fn k_rw(&self, s_w: f64) -> f64 {
        let s_eff = ((s_w - self.s_wc) / (1.0 - self.s_wc - self.s_or)).clamp(0.0, 1.0);
        self.k_rw_max * s_eff.powf(self.n_w)
    }

    pub fn d_k_rw_d_sw(&self, s_w: f64) -> f64 {
        let denom = 1.0 - self.s_wc - self.s_or;
        if denom <= 0.0 {
            return 0.0;
        }
        let s_eff = (s_w - self.s_wc) / denom;
        if !(0.0..1.0).contains(&s_eff) {
            return 0.0;
        }
        self.k_rw_max * self.n_w * s_eff.powf(self.n_w - 1.0) / denom
    }

    /// Oil relative permeability [dimensionless] using Corey-Brooks correlation
    /// k_ro(Sw) = kro_max * ((1 - Sw - Sor) / (1 - Swc - Sor))^no
    /// Returns 0 for Sw >= 1-Sor (critical water saturation), kro_max for Sw <= Swc
    pub fn k_ro(&self, s_w: f64) -> f64 {
        let s_eff = ((1.0 - s_w - self.s_or) / (1.0 - self.s_wc - self.s_or)).clamp(0.0, 1.0);
        self.k_ro_max * s_eff.powf(self.n_o)
    }

    pub fn d_k_ro_d_sw(&self, s_w: f64) -> f64 {
        let denom = 1.0 - self.s_wc - self.s_or;
        if denom <= 0.0 {
            return 0.0;
        }
        let s_eff = (1.0 - s_w - self.s_or) / denom;
        if !(0.0..1.0).contains(&s_eff) {
            return 0.0;
        }
        -self.k_ro_max * self.n_o * s_eff.powf(self.n_o - 1.0) / denom
    }

    // ── Generic (differentiable) mirrors, see the three-phase block above ──────

    pub(crate) fn k_rw_generic<S: Scalar>(&self, s_w: S) -> S {
        let denom = 1.0 - self.s_wc - self.s_or;
        let s_eff = ((s_w - self.s_wc) / denom).max_floor(0.0).min_ceil(1.0);
        s_eff.powf(self.n_w) * self.k_rw_max
    }

    pub(crate) fn k_ro_generic<S: Scalar>(&self, s_w: S) -> S {
        let denom = 1.0 - self.s_wc - self.s_or;
        let s_eff = ((S::from_f64(1.0 - self.s_or) - s_w) / denom)
            .max_floor(0.0)
            .min_ceil(1.0);
        s_eff.powf(self.n_o) * self.k_ro_max
    }

    /// OPM's tabulated saturation functions use constant extension at both endpoints. These
    /// mirrors retain the Corey values but freeze AD at the exact endpoints, also matching the
    /// strict-open-interval convention used by `d_k_*_d_sw` above.
    pub(crate) fn k_rw_endpoint_clipped_generic<S: Scalar>(&self, s_w: S) -> S {
        if s_w.value() <= self.s_wc {
            return S::from_f64(self.k_rw(self.s_wc));
        }
        if s_w.value() >= 1.0 - self.s_or {
            return S::from_f64(self.k_rw(1.0 - self.s_or));
        }
        self.k_rw_generic(s_w)
    }

    pub(crate) fn k_ro_endpoint_clipped_generic<S: Scalar>(&self, s_w: S) -> S {
        if s_w.value() <= self.s_wc {
            return S::from_f64(self.k_ro(self.s_wc));
        }
        if s_w.value() >= 1.0 - self.s_or {
            return S::from_f64(self.k_ro(1.0 - self.s_or));
        }
        self.k_ro_generic(s_w)
    }

    /// Exact rounded SWOF values from `opm/reference-decks/water-heavy-step1/CASE.DATA`.
    /// OPM's piecewise-linear evaluator selects the segment ending at a knot, hence `Sw=.3`
    /// uses the `.2-.3` slope rather than the analytic Corey tangent. This narrowly names the
    /// water-heavy diagnostic table; it is not a replacement for the application's Corey model.
    const WATER_HEAVY_SWOF: [(f64, f64, f64); 9] = [
        (0.10, 0.0000, 1.0000),
        (0.20, 0.0156, 0.7656),
        (0.30, 0.0625, 0.5625),
        (0.40, 0.1406, 0.3906),
        (0.50, 0.2500, 0.2500),
        (0.60, 0.3906, 0.1406),
        (0.70, 0.5625, 0.0625),
        (0.80, 0.7656, 0.0156),
        (0.90, 1.0000, 0.0000),
    ];

    /// WATER-020: OPM-style tabulated saturation functions, sampled from ResSim's *own* Corey
    /// curves rather than from a deck.
    ///
    /// OPM never evaluates an analytic relative-permeability law. It builds a piecewise-linear
    /// table from SWOF/SGOF and interpolates it, so its Newton system sees piecewise-constant
    /// relperm derivatives with no curvature. ResSim evaluates smooth Corey curves, whose
    /// curvature is what the Wang-Tchelepi inflection chop (`FIM-DAMP-002/003/004`) exists to
    /// damp.
    ///
    /// This samples `k_rw`/`k_ro` at `points` equally spaced knots across `[s_wc, 1 - s_or]` and
    /// interpolates linearly between them, exactly as `water_heavy_swof_replay` does for the
    /// tracked deck table but without being tied to that deck. Sweeping `points` separates the
    /// two candidate mechanisms: if a convergence win survives at a knot count fine enough that
    /// the table reproduces Corey to within a small tolerance, it comes from the piecewise-linear
    /// *representation* and is a faithful OPM replication; if it only appears for coarse tables,
    /// it is a physics change and must not be promoted.
    ///
    /// `points` is the number of knots and must be at least 2.
    pub(crate) fn corey_table_generic<S: Scalar>(&self, s_w: S, points: usize) -> (S, S) {
        let points = points.max(2);
        let lo = self.s_wc;
        let hi = 1.0 - self.s_or;
        let span = (hi - lo).max(1e-12);
        let step = span / ((points - 1) as f64);

        let knot = |index: usize| {
            let x = (lo + (index as f64) * step).min(hi);
            (x, self.k_rw(x), self.k_ro(x))
        };

        let value = s_w.value();
        if value <= lo {
            let (_, krw, kro) = knot(0);
            return (S::from_f64(krw), S::from_f64(kro));
        }
        if value >= hi {
            let (_, krw, kro) = knot(points - 1);
            return (S::from_f64(krw), S::from_f64(kro));
        }

        let segment = (((value - lo) / step).floor() as usize).min(points - 2);
        let (x0, krw0, kro0) = knot(segment);
        let (x1, krw1, kro1) = knot(segment + 1);
        let t = ((s_w - x0) / (x1 - x0)).max_floor(0.0).min_ceil(1.0);
        (t * (krw1 - krw0) + krw0, t * (kro1 - kro0) + kro0)
    }

    /// Segment slopes of [`Self::corey_table_generic`].
    ///
    /// The tabulated law is piecewise linear, so its derivative is the slope of the segment
    /// containing `s_w`, and zero outside `[s_wc, 1 - s_or]` where the table is clamped. Any
    /// consumer that needs `dk/dSw` must use this rather than the analytic Corey derivative,
    /// otherwise the value and its derivative come from two different models.
    pub(crate) fn corey_table_derivatives(&self, s_w: f64, points: usize) -> (f64, f64) {
        let points = points.max(2);
        let lo = self.s_wc;
        let hi = 1.0 - self.s_or;
        let span = (hi - lo).max(1e-12);
        let step = span / ((points - 1) as f64);

        if s_w <= lo || s_w >= hi {
            return (0.0, 0.0);
        }
        let segment = (((s_w - lo) / step).floor() as usize).min(points - 2);
        let x0 = (lo + (segment as f64) * step).min(hi);
        let x1 = (lo + ((segment + 1) as f64) * step).min(hi);
        let width = (x1 - x0).max(1e-12);
        (
            (self.k_rw(x1) - self.k_rw(x0)) / width,
            (self.k_ro(x1) - self.k_ro(x0)) / width,
        )
    }

    /// Scalar mirror of [`Self::corey_table_generic`].
    pub(crate) fn corey_table(&self, s_w: f64, points: usize) -> (f64, f64) {
        self.corey_table_generic(s_w, points)
    }

    pub(crate) fn water_heavy_swof_replay(&self, s_w: f64) -> (f64, f64) {
        let rows = &Self::WATER_HEAVY_SWOF;
        let value = |index: usize| (rows[index].1, rows[index].2);
        if s_w <= rows[0].0 {
            return value(0);
        }
        for index in 1..rows.len() {
            if s_w <= rows[index].0 {
                let (x0, krw0, kro0) = rows[index - 1];
                let (x1, krw1, kro1) = rows[index];
                let t = ((s_w - x0) / (x1 - x0)).clamp(0.0, 1.0);
                return (krw0 + t * (krw1 - krw0), kro0 + t * (kro1 - kro0));
            }
        }
        value(rows.len() - 1)
    }

    pub(crate) fn water_heavy_swof_replay_generic<S: Scalar>(&self, s_w: S) -> (S, S) {
        let rows = &Self::WATER_HEAVY_SWOF;
        if s_w.value() <= rows[0].0 {
            return (S::from_f64(rows[0].1), S::from_f64(rows[0].2));
        }
        for index in 1..rows.len() {
            if s_w.value() <= rows[index].0 {
                let (x0, krw0, kro0) = rows[index - 1];
                let (x1, krw1, kro1) = rows[index];
                let t = ((s_w - x0) / (x1 - x0)).max_floor(0.0).min_ceil(1.0);
                return (t * (krw1 - krw0) + krw0, t * (kro1 - kro0) + kro0);
            }
        }
        let last = rows.len() - 1;
        (S::from_f64(rows[last].1), S::from_f64(rows[last].2))
    }
}

#[cfg(test)]
mod endpoint_derivative_tests {
    use super::*;
    use crate::fim::ad::Ad;

    #[test]
    fn opm_endpoint_replay_freezes_corey_ad_without_changing_values() {
        let scal = RockFluidProps::default_scal();
        let sw = Ad::<1>::variable(scal.s_wc, 0);

        let legacy_water = scal.k_rw_generic(sw);
        let legacy_oil = scal.k_ro_generic(sw);
        let clipped_water = scal.k_rw_endpoint_clipped_generic(sw);
        let clipped_oil = scal.k_ro_endpoint_clipped_generic(sw);

        assert_eq!(clipped_water.value(), legacy_water.value());
        assert_eq!(clipped_oil.value(), legacy_oil.value());
        assert_eq!(clipped_water.d(0), scal.d_k_rw_d_sw(scal.s_wc));
        assert_eq!(clipped_oil.d(0), scal.d_k_ro_d_sw(scal.s_wc));
        assert_eq!(clipped_water.d(0), 0.0);
        assert_eq!(clipped_oil.d(0), 0.0);
        assert_ne!(legacy_oil.d(0), 0.0);
    }

    #[test]
    fn opm_endpoint_replay_keeps_interior_corey_ad_live() {
        let scal = RockFluidProps::default_scal();
        let sw_value = 0.3;
        let sw = Ad::<1>::variable(sw_value, 0);

        let water = scal.k_rw_endpoint_clipped_generic(sw);
        let oil = scal.k_ro_endpoint_clipped_generic(sw);

        assert!((water.value() - scal.k_rw(sw_value)).abs() < 1e-15);
        assert!((oil.value() - scal.k_ro(sw_value)).abs() < 1e-15);
        assert!((water.d(0) - scal.d_k_rw_d_sw(sw_value)).abs() < 1e-15);
        assert!((oil.d(0) - scal.d_k_ro_d_sw(sw_value)).abs() < 1e-15);
    }

    #[test]
    fn water_heavy_swof_replay_uses_rounded_left_segment_slope_at_point_three() {
        let scal = RockFluidProps::default_scal();
        let sw = Ad::<1>::variable(0.3, 0);
        let (krw, kro) = scal.water_heavy_swof_replay_generic(sw);
        assert!((krw.value() - 0.0625).abs() < 1e-15);
        assert!((kro.value() - 0.5625).abs() < 1e-15);
        assert!((krw.d(0) - 0.469).abs() < 1e-12);
        assert!((kro.d(0) + 2.031).abs() < 1e-12);
        let endpoint = Ad::<1>::variable(0.1, 0);
        let (water_endpoint, oil_endpoint) = scal.water_heavy_swof_replay_generic(endpoint);
        assert_eq!(water_endpoint.d(0), 0.0);
        assert_eq!(oil_endpoint.d(0), 0.0);
    }
}
