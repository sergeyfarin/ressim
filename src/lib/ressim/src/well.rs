use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct WellRates {
    pub oil_rate: f64,
    pub water_rate: f64,
    pub total_liquid_rate: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TimePointRates {
    pub time: f64,
    pub total_production_oil: f64,
    pub total_production_liquid: f64,
    pub total_production_liquid_reservoir: f64,
    pub total_injection: f64,
    pub total_injection_reservoir: f64,
    /// Material balance error [m³]: cumulative (injection - production) vs actual in-place change
    pub material_balance_error_m3: f64,
    /// Average reservoir pressure [bar]
    pub avg_reservoir_pressure: f64,
    /// Average water saturation
    pub avg_water_saturation: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Well {
    /// Cell index i (x-direction)
    pub i: usize,
    /// Cell index j (y-direction)
    pub j: usize,
    /// Cell index k (z-direction)
    pub k: usize,
    /// Bottom hole pressure [bar]
    pub bhp: f64,
    /// Productivity index [m³/(day·bar)]
    /// Rate = PI * (p_cell - bhp) for producer; negative for injector.
    pub productivity_index: f64,
    /// True if injector, false if producer
    pub injector: bool,
    /// Wellbore radius [m]
    pub well_radius: f64,
    /// Skin factor [dimensionless]
    pub skin: f64,
}

impl Well {
    /// Validate well parameters to prevent NaN/Inf and unphysical values
    /// Returns Ok(()) if parameters are valid, Err(message) otherwise
    pub fn validate(&self, nx: usize, ny: usize, nz: usize) -> Result<(), String> {
        // Check grid indices are within bounds
        if self.i >= nx {
            return Err(format!("Well index i={} out of bounds (nx={})", self.i, nx));
        }
        if self.j >= ny {
            return Err(format!("Well index j={} out of bounds (ny={})", self.j, ny));
        }
        if self.k >= nz {
            return Err(format!("Well index k={} out of bounds (nz={})", self.k, nz));
        }

        // Check BHP is finite (not NaN or Inf)
        if !self.bhp.is_finite() {
            return Err(format!("BHP must be finite, got: {}", self.bhp));
        }

        // Check well radius is positive and finite
        if self.well_radius <= 0.0 || !self.well_radius.is_finite() {
            return Err(format!(
                "Well radius must be positive and finite, got: {}",
                self.well_radius
            ));
        }

        // Check skin factor is finite
        if !self.skin.is_finite() {
            return Err(format!("Skin factor must be finite, got: {}", self.skin));
        }

        // Check productivity index is non-negative (PI = 0 means no well, PI < 0 is unphysical)
        if self.productivity_index < 0.0 {
            return Err(format!(
                "Productivity index must be non-negative, got: {}",
                self.productivity_index
            ));
        }

        // Check productivity index is finite
        if !self.productivity_index.is_finite() {
            return Err(format!(
                "Productivity index must be finite, got: {}",
                self.productivity_index
            ));
        }

        // Check BHP is physically reasonable (typically between -50 bar vacuum to 1000 bar)
        // Allow wider range for generality: [-100, 2000] bar
        if self.bhp < -100.0 || self.bhp > 2000.0 {
            return Err(format!(
                "BHP out of reasonable range [-100, 2000] bar, got: {}",
                self.bhp
            ));
        }

        Ok(())
    }
}
