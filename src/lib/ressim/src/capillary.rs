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
}

/// Oil-gas capillary pressure: P_cog(S_g) = P_oil − P_gas.
/// Higher S_g → lower P_cog (oil drains, gas fills larger pores).
/// Same Brooks-Corey form as `CapillaryPressure` but parameterised on S_g.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct GasOilCapillaryPressure {
    /// Entry pressure [bar]
    pub p_entry: f64,
    /// Brooks-Corey exponent (lambda) [dimensionless]
    pub lambda: f64,
}

impl GasOilCapillaryPressure {
    /// P_cog(S_g): capillary pressure [bar] as a function of gas saturation.
    /// S_g_eff = (S_g − S_gc) / (1 − S_wc − S_gc − S_gr)
    pub fn capillary_pressure_og(&self, s_g: f64, rock: &RockFluidPropsThreePhase) -> f64 {
        let denom = 1.0 - rock.s_wc - rock.s_gc - rock.s_gr;
        if denom <= 0.0 {
            return 0.0;
        }
        let s_eff = ((s_g - rock.s_gc) / denom).clamp(0.0, 1.0);

        if s_eff >= 1.0 {
            return 0.0; // At maximum gas saturation, capillary pressure is zero
        }

        let pc_max = self.p_entry * 20.0;

        if s_eff <= 0.0 {
            return pc_max;
        }

        let pc = self.p_entry * s_eff.powf(-1.0 / self.lambda);
        pc.clamp(0.0, pc_max)
    }
}
