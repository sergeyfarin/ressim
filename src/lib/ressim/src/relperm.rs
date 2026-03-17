use serde::{Deserialize, Serialize};

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
    /// Residual (trapped) gas saturation [dimensionless]
    pub s_gr: f64,
    /// Corey exponent for gas [dimensionless]
    pub n_g: f64,
    /// Max gas relative permeability at So = Sor [dimensionless]
    pub k_rg_max: f64,
}

impl RockFluidPropsThreePhase {
    /// Water relative permeability — Corey-Brooks (same formula as 2-phase).
    pub fn k_rw(&self, s_w: f64) -> f64 {
        let s_eff = ((s_w - self.s_wc) / (1.0 - self.s_wc - self.s_or)).clamp(0.0, 1.0);
        self.k_rw_max * s_eff.powf(self.n_w)
    }

    /// Gas relative permeability — Corey-Brooks.
    /// S_g_eff = (S_g − S_gc) / (1 − S_wc − S_gc − S_gr)
    pub fn k_rg(&self, s_g: f64) -> f64 {
        let denom = 1.0 - self.s_wc - self.s_gc - self.s_gr;
        if denom <= 0.0 {
            return 0.0;
        }
        let s_eff = ((s_g - self.s_gc) / denom).clamp(0.0, 1.0);
        self.k_rg_max * s_eff.powf(self.n_g)
    }

    /// Oil relative permeability in oil-water 2-phase system (Sg = 0).
    /// k_ro_w = k_ro_max * ((1 − Sw − Sor) / (1 − Swc − Sor))^no
    pub fn k_ro_water(&self, s_w: f64) -> f64 {
        let s_eff = ((1.0 - s_w - self.s_or) / (1.0 - self.s_wc - self.s_or)).clamp(0.0, 1.0);
        self.k_ro_max * s_eff.powf(self.n_o)
    }

    /// Oil relative permeability in oil-gas 2-phase system (Sw = Swc).
    /// S_o_eff = (1 − Swc − Sg − Sgr) / (1 − Swc − Sgr)
    pub fn k_ro_gas(&self, s_g: f64) -> f64 {
        let denom = 1.0 - self.s_wc - self.s_gr;
        if denom <= 0.0 {
            return 0.0;
        }
        let s_eff = ((1.0 - self.s_wc - s_g - self.s_gr) / denom).clamp(0.0, 1.0);
        self.k_ro_max * s_eff.powf(self.n_o)
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

    /// Oil relative permeability [dimensionless] using Corey-Brooks correlation
    /// k_ro(Sw) = kro_max * ((1 - Sw - Sor) / (1 - Swc - Sor))^no
    /// Returns 0 for Sw >= 1-Sor (critical water saturation), kro_max for Sw <= Swc
    pub fn k_ro(&self, s_w: f64) -> f64 {
        let s_eff = ((1.0 - s_w - self.s_or) / (1.0 - self.s_wc - self.s_or)).clamp(0.0, 1.0);
        self.k_ro_max * s_eff.powf(self.n_o)
    }
}
