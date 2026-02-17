use serde::{Deserialize, Serialize};

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

        // Avoid division by zero
        if s_eff >= 1.0 {
            return 0.0; // At maximum water saturation, capillary pressure is zero
        }
        if s_eff <= 0.0 {
            return 1000.0; // At connate water, very high capillary pressure (clamped)
        }

        // Brooks-Corey capillary pressure: P_c = P_entry * (S_eff)^(-1/lambda)
        let pc = self.p_entry * s_eff.powf(-1.0 / self.lambda);

        // Clamp to reasonable range [0, 500 bar]
        pc.clamp(0.0, 500.0)
    }
}
