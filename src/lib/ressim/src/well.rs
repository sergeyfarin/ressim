use serde::{Deserialize, Serialize};

fn default_well_schedule_enabled() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WellSchedule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_rate_m3_day: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_surface_rate_m3_day: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bhp_limit: Option<f64>,
    #[serde(default = "default_well_schedule_enabled")]
    pub enabled: bool,
}

impl Default for WellSchedule {
    fn default() -> Self {
        Self {
            control_mode: None,
            target_rate_m3_day: None,
            target_surface_rate_m3_day: None,
            bhp_limit: None,
            enabled: true,
        }
    }
}

impl WellSchedule {
    pub fn has_explicit_control(&self) -> bool {
        self.control_mode.is_some()
            || self.target_rate_m3_day.is_some()
            || self.target_surface_rate_m3_day.is_some()
            || self.bhp_limit.is_some()
            || !self.enabled
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Well {
    /// Stable physical-well identifier shared by all completions of the same well.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical_well_id: Option<String>,
    #[serde(default)]
    pub schedule: WellSchedule,
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

        if let Some(well_id) = &self.physical_well_id {
            if well_id.trim().is_empty() {
                return Err("Physical well id must not be empty when provided".to_string());
            }
        }

        if let Some(control_mode) = &self.schedule.control_mode {
            let normalized = control_mode.trim().to_ascii_lowercase();
            if normalized != "pressure" && normalized != "rate" {
                return Err(format!(
                    "Well control mode must be 'pressure' or 'rate', got: {}",
                    control_mode
                ));
            }
        }
        if let Some(target_rate) = self.schedule.target_rate_m3_day {
            if !target_rate.is_finite() || target_rate < 0.0 {
                return Err(format!(
                    "Well target reservoir rate must be finite and non-negative, got: {}",
                    target_rate
                ));
            }
        }
        if let Some(target_surface_rate) = self.schedule.target_surface_rate_m3_day {
            if !target_surface_rate.is_finite() || target_surface_rate < 0.0 {
                return Err(format!(
                    "Well target surface rate must be finite and non-negative, got: {}",
                    target_surface_rate
                ));
            }
        }
        if let Some(bhp_limit) = self.schedule.bhp_limit {
            if !bhp_limit.is_finite() {
                return Err(format!("Well BHP limit must be finite, got: {}", bhp_limit));
            }
        }

        Ok(())
    }
}
