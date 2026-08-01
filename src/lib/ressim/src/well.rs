use serde::{Deserialize, Serialize};

fn default_well_schedule_enabled() -> bool {
    true
}

/// Recognized schedule control kinds. The serialized schedule keeps its historical string field
/// so existing scenario payloads remain compatible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WellScheduleControl {
    Pressure,
    Rate,
    Resv,
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
    pub fn control_kind(&self) -> Option<WellScheduleControl> {
        match self
            .control_mode
            .as_deref()?
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "pressure" => Some(WellScheduleControl::Pressure),
            "rate" => Some(WellScheduleControl::Rate),
            "resv" => Some(WellScheduleControl::Resv),
            _ => None,
        }
    }

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
    /// Depth the well's `bhp` is referenced to [m TVDSS], shared by every
    /// completion of the same physical well.
    ///
    /// `None` defers to the shallowest completion of the well, which is the
    /// Eclipse `WELSPECS` default. Only consulted when gravity is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datum_depth_m: Option<f64>,
    /// Density of the fluid column standing in the wellbore [kg/m³], used to
    /// carry `bhp` from the datum down to each completion.
    ///
    /// `None` lets the engine derive it per step from the completion fluids —
    /// the injected phase for an injector, the mobility-weighted in-situ
    /// mixture for a producer. Only consulted when gravity is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wellbore_density_kg_m3: Option<f64>,
    /// Hydrostatic head from the datum down to *this* completion [bar], so the
    /// pressure the connection law sees is `bhp + head_offset_bar`.
    ///
    /// Derived, not input: [`ReservoirSimulator::refresh_well_head_offsets`]
    /// rewrites it whenever the well set or the fluid state moves, and it is
    /// identically zero while gravity is disabled. It is held fixed across a
    /// step so the FIM Jacobian keeps `∂q/∂bhp` unchanged.
    #[serde(default)]
    pub head_offset_bar: f64,
    /// Bottomhole pressure the well actually flowed at on the last recorded
    /// step [bar], or `None` before the first step / while disabled.
    ///
    /// `bhp` above is the well's *configured* pressure — a target for a
    /// BHP-controlled well and a limit for a rate-controlled one, and it does
    /// not move. This field is the pressure the solver arrived at, which for a
    /// rate-controlled well is the quantity a pressure-transient analysis
    /// needs. Written by the reporting pass only; no solver reads it back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flowing_bhp: Option<f64>,
}

impl Well {
    /// Pressure this completion's connection law works against [bar]: the
    /// well's datum `bhp` carried down the wellbore by [`Self::head_offset_bar`].
    ///
    /// Every consumer of a BHP as a *connection* pressure must go through here;
    /// `bhp` alone is the datum value and is only correct at the datum depth.
    pub fn connection_pressure_bar(&self, bhp_bar: f64) -> f64 {
        bhp_bar + self.head_offset_bar
    }

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

        if let Some(datum_depth) = self.datum_depth_m {
            if !datum_depth.is_finite() {
                return Err(format!(
                    "Well datum depth must be finite, got: {}",
                    datum_depth
                ));
            }
        }

        if let Some(wellbore_density) = self.wellbore_density_kg_m3 {
            if !wellbore_density.is_finite() || wellbore_density < 0.0 {
                return Err(format!(
                    "Wellbore density must be finite and non-negative, got: {}",
                    wellbore_density
                ));
            }
        }

        if let Some(well_id) = &self.physical_well_id {
            if well_id.trim().is_empty() {
                return Err("Physical well id must not be empty when provided".to_string());
            }
        }

        if let Some(control_mode) = &self.schedule.control_mode {
            if self.schedule.control_kind().is_none() {
                return Err(format!(
                    "Well control mode must be 'pressure', 'rate', or 'resv', got: {}",
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
