use serde::{Deserialize, Serialize};

use crate::relperm::RockFluidPropsThreePhase;
use crate::RockFluidProps;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct CapillaryPressure {
    /// Entry pressure (displacement pressure) [bar]
    /// Minimum pressure needed to enter largest pores
    pub p_entry: f64,
    /// Brooks-Corey exponent (lambda) [dimensionless]
    /// Controls shape of capillary pressure curve
    pub lambda: f64,
}

impl CapillaryPressure {
    /// Create capillary pressure with default parameters
    pub(crate) fn default_pc() -> Self {
        Self {
            p_entry: 5.0, // bar - typical entry pressure
            lambda: 2.0,  // dimensionless - typical exponent
        }
    }

    /// Calculate capillary pressure [bar] at given water saturation
    /// Uses Brooks-Corey correlation:
    /// P_c(S_w) = P_entry * ((S_eff)^(-1/lambda))
    /// where S_eff = (S_w - S_wc) / (1 - S_wc - S_or)
    ///
    /// Physical meaning: P_c = P_oil - P_water (oil-water capillary pressure)
    pub fn capillary_pressure(&self, s_w: f64, rock: &RockFluidProps) -> f64 {
        // Calculate effective saturation
        let s_eff = ((s_w - rock.s_wc) / (1.0 - rock.s_wc - rock.s_or)).clamp(0.0, 1.0);

        // Avoid division by zero and handle physical bounds
        if s_eff >= 1.0 {
            return 0.0; // At maximum water saturation, capillary pressure is zero
        }

        // Use a scaled heuristic to avoid the "capillary sponge" artifact where infinite curves
        // overpower gravity over hundreds of meters. We cap at 20x the entry pressure.
        let pc_max = self.p_entry * 20.0;

        if s_eff <= 0.0 {
            return pc_max; // At connate water, capillary pressure hits the scaling bound
        }

        // Brooks-Corey capillary pressure: P_c = P_entry * (S_eff)^(-1/lambda)
        let pc = self.p_entry * s_eff.powf(-1.0 / self.lambda);

        pc.clamp(0.0, pc_max)
    }

    pub fn d_capillary_pressure_d_sw(&self, s_w: f64, rock: &RockFluidProps) -> f64 {
        let denom = 1.0 - rock.s_wc - rock.s_or;
        if denom <= 0.0 {
            return 0.0;
        }

        let s_eff = (s_w - rock.s_wc) / denom;
        if !(0.0..1.0).contains(&s_eff) {
            return 0.0;
        }

        self.p_entry * (-1.0 / self.lambda) * s_eff.powf(-1.0 / self.lambda - 1.0) / denom
    }
}

/// Oil-gas capillary pressure: P_cog(S_g) = P_gas − P_oil (gas is non-wetting).
/// Used in step.rs as: P_gas = P_oil + P_cog, consistent with standard black-oil convention.
///
/// Parameterised on S_o_eff (oil wetting-phase effective saturation) using `s_org`
/// (residual oil to gas). As S_g increases, S_o decreases, S_o_eff decreases, and
/// P_cog = P_entry × S_o_eff^(−1/λ) increases — physically correct for a non-wetting gas.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct GasOilCapillaryPressure {
    /// Entry pressure [bar] — P_cog when S_o is at its maximum (S_g = 0)
    pub p_entry: f64,
    /// Brooks-Corey exponent (lambda) [dimensionless]
    pub lambda: f64,
}

impl GasOilCapillaryPressure {
    /// P_cog(S_g): capillary pressure [bar] as a function of gas saturation.
    ///
    /// Uses oil effective saturation at connate water: S_o = 1 − S_wc − S_g.
    /// S_o_eff = (S_o − S_org) / (1 − S_wc − S_org)
    ///
    /// P_cog increases with S_g (correct non-wetting behaviour):
    /// - S_g = 0          → S_o_eff = 1 → P_cog = P_entry (minimum)
    /// - S_g → 1−Swc−Sorg → S_o_eff = 0 → P_cog = 20 × P_entry (cap)
    pub fn capillary_pressure_og(&self, s_g: f64, rock: &RockFluidPropsThreePhase) -> f64 {
        let denom = 1.0 - rock.s_wc - rock.s_org;
        if denom <= 0.0 {
            return self.p_entry * 20.0;
        }

        let s_o = 1.0 - rock.s_wc - s_g;
        let s_eff = ((s_o - rock.s_org) / denom).clamp(0.0, 1.0);

        let pc_max = self.p_entry * 20.0;

        if s_eff <= 0.0 {
            return pc_max;
        }

        // Brooks-Corey: P_cog = P_entry × S_o_eff^(−1/λ)
        // At s_eff = 1: pc = P_entry; decreases toward 0 only if s_eff > 1 (physically excluded)
        let pc = self.p_entry * s_eff.powf(-1.0 / self.lambda);
        pc.clamp(0.0, pc_max)
    }

    pub fn d_capillary_pressure_og_d_sg(&self, s_g: f64, rock: &RockFluidPropsThreePhase) -> f64 {
        let denom = 1.0 - rock.s_wc - rock.s_org;
        if denom <= 0.0 {
            return 0.0;
        }

        let s_o = 1.0 - rock.s_wc - s_g;
        let s_eff = (s_o - rock.s_org) / denom;
        if !(0.0..1.0).contains(&s_eff) {
            return 0.0;
        }

        self.p_entry * (1.0 / self.lambda) * s_eff.powf(-1.0 / self.lambda - 1.0) / denom
    }
}
