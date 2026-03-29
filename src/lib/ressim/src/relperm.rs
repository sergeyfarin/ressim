use serde::{Deserialize, Serialize};

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
}
