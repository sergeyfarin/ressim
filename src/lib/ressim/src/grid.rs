use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct GridCell {
    /// Porosity [dimensionless, 0-1]
    pub porosity: f64,
    /// Permeability in x-direction [mD] (milliDarcy)
    pub perm_x: f64,
    /// Permeability in y-direction [mD] (milliDarcy)
    pub perm_y: f64,
    /// Permeability in z-direction [mD] (milliDarcy)
    pub perm_z: f64,
    /// Pressure [bar]
    pub pressure: f64,
    /// Water saturation [dimensionless, 0-1]
    pub sat_water: f64,
    /// Oil saturation [dimensionless, 0-1]. Note: sat_water + sat_oil = 1.0
    pub sat_oil: f64,
}

impl GridCell {
    /// Create default grid cell with oil-field units
    pub(crate) fn default_cell() -> Self {
        GridCell {
            porosity: 0.2,   // dimensionless [0-1]
            perm_x: 100.0,   // mD (milliDarcy)
            perm_y: 100.0,   // mD
            perm_z: 10.0,    // mD (vertical permeability typically lower)
            pressure: 300.0, // bar (typical reservoir pressure ~30 MPa = 300 bar)
            sat_water: 0.3,  // dimensionless [0-1]
            sat_oil: 0.7,    // dimensionless [0-1], s_w + s_o = 1.0
        }
    }

    /// Returns pore volume in cubic meters
    /// Cell dimensions (dx, dy, dz) must be in meters (m)
    pub fn pore_volume_m3(&self, dx: f64, dy: f64, dz: f64) -> f64 {
        dx * dy * dz * self.porosity
    }
}
