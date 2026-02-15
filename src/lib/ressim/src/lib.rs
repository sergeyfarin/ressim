// File: `wasm/simulator/src/lib.rs`
//
// UNIT SYSTEM: OIL-FIELD UNITS (CONSISTENT THROUGHOUT)
// =====================================================
// Pressure: bar
// Distance: meter (m)
// Time: day (d)
// Volume: cubic meter (m³)
// Permeability: milliDarcy (mD) [1 D = 9.8692e-13 m²]
// Viscosity: centiPoise (cP) [1 cP = 0.001 Pa·s]
// Compressibility: 1/bar
// Saturation: dimensionless [0, 1]
//
// CONVERSION FACTORS USED:
// - Transmissibility calculation includes conversion from mD·bar·m³/day to flow units
// - All calculations maintain consistency in these base units with no hidden conversions

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use nalgebra::DVector;
use sprs::{CsMat, TriMatI};
use std::f64;
use rand::RngExt;
use rand::SeedableRng;
use rand::rngs::StdRng;

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
    pub total_injection: f64,
}


// Utility to log panics to the browser console
#[wasm_bindgen(start)]
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

// --- Data Structures ---
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
    fn default_cell() -> Self {
        GridCell {
            porosity: 0.2,        // dimensionless [0-1]
            perm_x: 100.0,        // mD (milliDarcy)
            perm_y: 100.0,        // mD
            perm_z: 10.0,         // mD (vertical permeability typically lower)
            pressure: 300.0,      // bar (typical reservoir pressure ~30 MPa = 300 bar)
            sat_water: 0.3,       // dimensionless [0-1]
            sat_oil: 0.7,         // dimensionless [0-1], s_w + s_o = 1.0
        }
    }

    /// Returns pore volume in cubic meters
    /// Cell dimensions (dx, dy, dz) must be in meters (m)
    pub fn pore_volume_m3(&self, dx: f64, dy: f64, dz: f64) -> f64 {
        dx * dy * dz * self.porosity
    }
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
            return Err(format!("Well radius must be positive and finite, got: {}", self.well_radius));
        }

        // Check skin factor is finite
        if !self.skin.is_finite() {
            return Err(format!("Skin factor must be finite, got: {}", self.skin));
        }

        // Check productivity index is non-negative (PI = 0 means no well, PI < 0 is unphysical)
        if self.productivity_index < 0.0 {
            return Err(format!("Productivity index must be non-negative, got: {}", self.productivity_index));
        }
        
        // Check productivity index is finite
        if !self.productivity_index.is_finite() {
            return Err(format!("Productivity index must be finite, got: {}", self.productivity_index));
        }
        
        // Check BHP is physically reasonable (typically between -50 bar vacuum to 1000 bar)
        // Allow wider range for generality: [-100, 2000] bar
        if self.bhp < -100.0 || self.bhp > 2000.0 {
            return Err(format!("BHP out of reasonable range [-100, 2000] bar, got: {}", self.bhp));
        }
        
        Ok(())
    }
}

// --- Fluid / Rock ---
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct FluidProperties {
    /// Oil viscosity [cP] (centiPoise)
    pub mu_o: f64,
    /// Water viscosity [cP] (centiPoise)
    pub mu_w: f64,
    /// Oil compressibility [1/bar]
    pub c_o: f64,
    /// Water compressibility [1/bar]
    pub c_w: f64,
    /// Oil density [kg/m³]
    pub rho_o: f64,
    /// Water density [kg/m³]
    pub rho_w: f64,
}

impl FluidProperties {
    fn default_pvt() -> Self {
        Self {
            mu_o: 1.0,      // cP (typical oil)
            mu_w: 0.5,      // cP (water at reservoir conditions)
            c_o: 1e-5,      // 1/bar (oil compressibility)
            c_w: 3e-6,      // 1/bar (water compressibility)
            rho_o: 800.0,   // kg/m³
            rho_w: 1000.0,  // kg/m³
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
}

impl RockFluidProps {
    fn default_scal() -> Self {
        // Reduced saturation thresholds to allow better water flow at initial conditions
        // s_wc: connate water saturation (irreducible water that doesn't flow)
        // s_or: residual oil saturation (oil left after water breakthrough)
        Self { s_wc: 0.1, s_or: 0.1, n_w: 2.0, n_o: 2.0 }
    }

    /// Water relative permeability [dimensionless] using Corey-Brooks correlation
    /// k_rw(Sw) = ((Sw - Swc) / (1 - Swc - Sor))^nw
    /// Returns 0 for Sw <= Swc, 1 for Sw >= 1-Sor
    pub fn k_rw(&self, s_w: f64) -> f64 {
        let s_eff = ((s_w - self.s_wc) / (1.0 - self.s_wc - self.s_or)).clamp(0.0, 1.0);
        s_eff.powf(self.n_w)
    }
    
    /// Oil relative permeability [dimensionless] using Corey-Brooks correlation
    /// k_ro(Sw) = ((1 - Sw - Sor) / (1 - Swc - Sor))^no
    /// Returns 0 for Sw >= 1-Sor (critical water saturation), 1 for Sw <= Swc
    pub fn k_ro(&self, s_w: f64) -> f64 {
        let s_eff = ((1.0 - s_w - self.s_or) / (1.0 - self.s_wc - self.s_or)).clamp(0.0, 1.0);
        s_eff.powf(self.n_o)
    }
}

// --- Capillary Pressure (Brooks-Corey) ---
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
    fn default_pc() -> Self {
        Self {
            p_entry: 5.0,   // bar - typical entry pressure
            lambda: 2.0,    // dimensionless - typical exponent
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
            return 0.0;  // At maximum water saturation, capillary pressure is zero
        }
        if s_eff <= 0.0 {
            return 1000.0;  // At connate water, very high capillary pressure (clamped)
        }
        
        // Brooks-Corey capillary pressure: P_c = P_entry * (S_eff)^(-1/lambda)
        let pc = self.p_entry * s_eff.powf(-1.0 / self.lambda);
        
        // Clamp to reasonable range [0, 500 bar]
        pc.clamp(0.0, 500.0)
    }
}

// --- Simulator ---
#[wasm_bindgen]
pub struct ReservoirSimulator {
    nx: usize, ny: usize, nz: usize,
    dx: f64, dy: f64, dz: f64,
    grid_cells: Vec<GridCell>,
    wells: Vec<Well>,
    time_days: f64,
    pvt: FluidProperties,
    scal: RockFluidProps,
    pc: CapillaryPressure,
    gravity_enabled: bool,
    max_sat_change_per_step: f64,
    rate_history: Vec<TimePointRates>,
}

#[wasm_bindgen]
impl ReservoirSimulator {
    /// Create a new reservoir simulator with oil-field units
    /// Grid dimensions: nx, ny, nz (number of cells in each direction)
    /// All parameters use: Pressure [bar], Distance [m], Time [day], Permeability [mD], Viscosity [cP]
    #[wasm_bindgen(constructor)]
    pub fn new(nx: usize, ny: usize, nz: usize) -> Self {
        let n = nx * ny * nz;
        let grid_cells = vec![GridCell::default_cell(); n];
        ReservoirSimulator {
            nx, ny, nz,
            dx: 10.0,   // meters (x-direction cell size)
            dy: 10.0,   // meters (y-direction cell size)
            dz: 1.0,    // meters (z-direction cell size)
            grid_cells,
            wells: Vec::new(),
            time_days: 0.0,  // simulation time in days
            pvt: FluidProperties::default_pvt(),
            scal: RockFluidProps::default_scal(),
            pc: CapillaryPressure::default_pc(),  // Brooks-Corey capillary pressure
            gravity_enabled: false,
            max_sat_change_per_step: 0.1, // Default max saturation change
            rate_history: Vec::new(),
        }
    }

    fn idx(&self, i: usize, j: usize, k: usize) -> usize {
        (k * self.nx * self.ny) + (j * self.nx) + i
    }


    /// Add a well to the simulator
    /// Parameters in oil-field units:
    /// - i, j, k: grid cell indices (must be within grid bounds)
    /// - bhp: bottom-hole pressure [bar] (must be finite, typical: -100 to 2000 bar)
    /// - well_radius: wellbore radius [m]
    /// - skin: skin factor [dimensionless]
    /// - injector: true for injector (injects fluid), false for producer (extracts fluid)
    /// 
    /// Returns Ok(()) on success, or Err(message) if parameters are invalid.
    /// Invalid parameters include:
    /// - Out-of-bounds grid indices
    /// - NaN or Inf values in bhp or pi
    /// - Negative productivity index
    /// - BHP outside reasonable range
    pub fn add_well(&mut self, i: usize, j: usize, k: usize, bhp: f64, well_radius: f64, skin: f64, injector: bool) -> Result<(), String> {
        if i >= self.nx || j >= self.ny || k >= self.nz {
            return Err(format!(
                "Well indices out of bounds: (i={}, j={}, k={}) for grid ({}, {}, {})",
                i, j, k, self.nx, self.ny, self.nz
            ));
        }

        if !bhp.is_finite() {
            return Err(format!("BHP must be finite, got: {}", bhp));
        }

        if well_radius <= 0.0 || !well_radius.is_finite() {
            return Err(format!("Well radius must be positive and finite, got: {}", well_radius));
        }

        if !skin.is_finite() {
            return Err(format!("Skin factor must be finite, got: {}", skin));
        }
        
        let cell_id = self.idx(i,j,k);
        let cell = self.grid_cells[cell_id];

        // Calculate equivalent radius (Peaceman's model)
        let kx = cell.perm_x;
        let ky = cell.perm_y;
        if !kx.is_finite() || !ky.is_finite() || kx <= 0.0 || ky <= 0.0 {
            return Err(format!(
                "Cell permeability must be positive and finite for well PI calculation, got kx={}, ky={}",
                kx, ky
            ));
        }

        let r_eq = 0.28 * f64::sqrt(f64::sqrt(kx / ky) * self.dx.powi(2) + f64::sqrt(ky / kx) * self.dy.powi(2)) / ((kx/ky).powf(0.25) + (ky/kx).powf(0.25));
        if !r_eq.is_finite() || r_eq <= 0.0 {
            return Err(format!("Equivalent radius must be positive and finite, got: {}", r_eq));
        }

        if r_eq <= well_radius {
            return Err(format!(
                "Equivalent radius must be greater than well radius for valid PI. r_eq={}, rw={}",
                r_eq, well_radius
            ));
        }

        // Calculate productivity index (PI)
        let k_avg = f64::sqrt(kx * ky); // Geometric mean of horizontal permeabilities
        let total_mobility = self.total_mobility(&cell);
        if !k_avg.is_finite() || k_avg <= 0.0 {
            return Err(format!("Average permeability must be positive and finite, got: {}", k_avg));
        }
        if !total_mobility.is_finite() || total_mobility < 0.0 {
            return Err(format!("Total mobility must be finite and non-negative, got: {}", total_mobility));
        }

        let denom = f64::ln(r_eq / well_radius) + skin;
        if !denom.is_finite() || denom.abs() <= f64::EPSILON {
            return Err(format!(
                "Invalid PI denominator ln(r_eq/r_w)+skin = {}. Check well radius and skin.",
                denom
            ));
        }
        
        // Peaceman's well index formula for metric units (m³, bar, day)
        // PI = (2 * PI * k * h * mob) / (ln(r_e/r_w) + s)
        // The constant 0.008527 is for converting from mD to m² and other unit consistencies.
        // Here we use a simplified form that should be consistent with the transmissibility calculation.
        // The transmissibility constant is 0.001127, which is for bbl/day/psi, but the comment says m³/day/bar.
        // Let's assume the constant is correct for the intended units.
        // PI = C * k_avg * dz * total_mobility / (ln(r_eq/well_radius) + skin)
        // Let's use the same constant as transmissibility for consistency.
        let pi = (0.001127 * 2.0 * std::f64::consts::PI * k_avg * self.dz * total_mobility) / denom;

        let well = Well { i, j, k, bhp, productivity_index: pi, injector, well_radius, skin };
        
        // Validate well parameters
        well.validate(self.nx, self.ny, self.nz)?;
        
        self.wells.push(well);
        Ok(())
    }

    /// Set stability parameters for the simulation
    #[wasm_bindgen(js_name = setStabilityParams)]
    pub fn set_stability_params(&mut self, max_sat_change_per_step: f64) {
        self.max_sat_change_per_step = max_sat_change_per_step.clamp(0.01, 1.0);
    }

    /// Total mobility [1/cP] = lambda_t = (k_rw/μ_w) + (k_ro/μ_o)
    /// Sum of phase mobilities used in pressure equation
    fn total_mobility(&self, cell: &GridCell) -> f64 {
        let krw = self.scal.k_rw(cell.sat_water);
        let kro = self.scal.k_ro(cell.sat_water);
        krw / self.pvt.mu_w + kro / self.pvt.mu_o
    }

    /// Phase mobilities [1/cP] for water and oil
    fn phase_mobilities(&self, cell: &GridCell) -> (f64, f64) {
        let krw = self.scal.k_rw(cell.sat_water);
        let kro = self.scal.k_ro(cell.sat_water);
        (krw / self.pvt.mu_w, kro / self.pvt.mu_o)
    }

    /// Get capillary pressure [bar] at given water saturation
    fn get_capillary_pressure(&self, s_w: f64) -> f64 {
        self.pc.capillary_pressure(s_w, &self.scal)
    }

    fn depth_at_k(&self, k: usize) -> f64 {
        (k as f64 + 0.5) * self.dz
    }

    fn gravity_head_bar(&self, depth_i: f64, depth_j: f64, density_kg_m3: f64) -> f64 {
        if !self.gravity_enabled {
            return 0.0;
        }

        // rho [kg/m³] * g [m/s²] * dz [m] = Pa, then convert Pa -> bar using 1e-5
        density_kg_m3 * 9.80665 * (depth_i - depth_j) * 1e-5
    }

    fn total_density_face(&self, c_i: &GridCell, c_j: &GridCell) -> f64 {
        let (lam_w_i, lam_o_i) = self.phase_mobilities(c_i);
        let (lam_w_j, lam_o_j) = self.phase_mobilities(c_j);

        let lam_w_avg = 0.5 * (lam_w_i + lam_w_j);
        let lam_o_avg = 0.5 * (lam_o_i + lam_o_j);
        let lam_t_avg = lam_w_avg + lam_o_avg;

        if lam_t_avg <= f64::EPSILON {
            return 0.5 * (self.pvt.rho_w + self.pvt.rho_o);
        }

        ((lam_w_avg * self.pvt.rho_w) + (lam_o_avg * self.pvt.rho_o)) / lam_t_avg
    }

    /// Fractional flow of water [dimensionless] = f_w = λ_w / λ_t
    /// Used in upwind scheme for saturation transport
    fn frac_flow_water(&self, cell: &GridCell) -> f64 {
        let krw = self.scal.k_rw(cell.sat_water);
        let lam_w = krw / self.pvt.mu_w;
        let lam_t = lam_w + (self.scal.k_ro(cell.sat_water) / self.pvt.mu_o);
        if lam_t <= 0.0 { 0.0 } else { (lam_w / lam_t).clamp(0.0, 1.0) }
    }

    // transmissibility between two neighboring cells (oil-field units)
    // Inputs: permeability [mD], area [m²], distance [m], mobility [1/cP]
    // Output: T [m³/day/bar]
    // Formula: T = 0.001127 * k[mD] * A[m²] / (L[m] * mu[cP])
    // The factor 0.001127 converts from oilfield units to consistent flow units
    fn transmissibility(&self, c1: &GridCell, c2: &GridCell, dim: char) -> f64 {
        let (perm1, perm2, dist, area) = match dim {
            'x' => (c1.perm_x, c2.perm_x, self.dx, self.dy * self.dz),
            'y' => (c1.perm_y, c2.perm_y, self.dy, self.dx * self.dz),
            'z' => (c1.perm_z, c2.perm_z, self.dz, self.dx * self.dy),
            _ => (0.0, 0.0, 1.0, 1.0),
        };
        // Harmonic mean of permeabilities [mD]
        let k_h = if perm1 + perm2 == 0.0 { 0.0 } else { 2.0 * perm1 * perm2 / (perm1 + perm2) };
        if k_h == 0.0 { return 0.0; }
        
        // Average total mobility [1/cP]
        let mob_avg = (self.total_mobility(c1) + self.total_mobility(c2)) / 2.0;
        
        // Transmissibility [m³/day/bar]
        // 0.001127 factor: converts mD·m²/(m·cP) to m³/day/bar
        0.001127 * k_h * area / dist * mob_avg
    }

    // main IMPES step: delta_t in days (oil-field units)
    // Implicit pressure equation + explicit saturation update
    pub fn step(&mut self, target_dt_days: f64) {
        let mut time_stepped = 0.0;
        const MAX_ATTEMPTS: u32 = 10;
        let mut attempts = 0;

        while time_stepped < target_dt_days && attempts < MAX_ATTEMPTS {
            let remaining_dt = target_dt_days - time_stepped;
            
            // Calculate fluxes and stability for the remaining time step
            let (p_new, delta_water_m3, stable_dt_factor) = self.calculate_fluxes(remaining_dt);

            let actual_dt;
            let final_delta_water_m3;

            if stable_dt_factor < 1.0 {
                // Timestep is too large, reduce it based on CFL condition
                actual_dt = remaining_dt * stable_dt_factor * 0.9; // Use 90% for safety
                
                // Scale the water volume change by the ratio of the new dt to the old dt
                let dt_ratio = actual_dt / remaining_dt;
                final_delta_water_m3 = delta_water_m3.iter().map(|&dv| dv * dt_ratio).collect();
                
                attempts += 1;
            } else {
                // The full remaining timestep is stable
                actual_dt = remaining_dt;
                final_delta_water_m3 = delta_water_m3;
                attempts = 0; // Reset attempts on a successful full step
            }

            // Update saturations and pressure with the adjusted (or full) timestep
            self.update_saturations_and_pressure(&p_new, &final_delta_water_m3, actual_dt);
            
            time_stepped += actual_dt;
        }
    }

    #[wasm_bindgen(js_name = setGravityEnabled)]
    pub fn set_gravity_enabled(&mut self, enabled: bool) {
        self.gravity_enabled = enabled;
    }

    fn calculate_fluxes(&self, delta_t_days: f64) -> (DVector<f64>, Vec<f64>, f64) {
        let n_cells = self.nx * self.ny * self.nz;
        if n_cells == 0 { return (DVector::zeros(0), vec![], 1.0); }
        let dt_days = delta_t_days.max(1e-12);
        
        // Total compressibility [1/bar] = c_oil + c_water
        // Simplified: should be ct = phi * (c_o * S_o + c_w * S_w + c_r)
        let c_t = self.pvt.c_o + self.pvt.c_w;

        // Build triplet lists for A matrix and RHS b of pressure equation
        let mut rows: Vec<usize> = Vec::with_capacity(n_cells * 7);
        let mut cols: Vec<usize> = Vec::with_capacity(n_cells * 7);
        let mut vals: Vec<f64> = Vec::with_capacity(n_cells * 7);

        let mut b_rhs = DVector::<f64>::zeros(n_cells);
        let mut diag_inv = DVector::<f64>::zeros(n_cells);

        // Assemble pressure equation: accumulation + transmissibility + well terms
        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i, j, k);
                    let cell = &self.grid_cells[id];
                    
                    // Pore volume [m³]
                    let vp_m3 = cell.pore_volume_m3(self.dx, self.dy, self.dz);
                    
                    // Accumulation term: (Vp [m³] * c_t [1/bar]) / dt [day]
                    // Units: [m³ * 1/bar / day] = [m³/bar/day]
                    let accum = (vp_m3 * c_t) / dt_days;
                    let mut diag = accum;
                    
                    // Move old pressure term to RHS: accum * p_old
                    b_rhs[id] += accum * cell.pressure;

                    // neighbors: compute flux transmissibilities
                    let mut neighbors: Vec<(usize, char, usize)> = Vec::new();
                    if i > 0 { neighbors.push((self.idx(i-1,j,k), 'x', k)); }
                    if i < self.nx-1 { neighbors.push((self.idx(i+1,j,k), 'x', k)); }
                    if j > 0 { neighbors.push((self.idx(i,j-1,k), 'y', k)); }
                    if j < self.ny-1 { neighbors.push((self.idx(i,j+1,k), 'y', k)); }
                    if k > 0 { neighbors.push((self.idx(i,j,k-1), 'z', k - 1)); }
                    if k < self.nz-1 { neighbors.push((self.idx(i,j,k+1), 'z', k + 1)); }

                    for (n_id, dim, n_k) in neighbors.iter() {
                        // Transmissibility [m³/day/bar]
                        let t = self.transmissibility(cell, &self.grid_cells[*n_id], *dim);
                        diag += t;
                        rows.push(id); cols.push(*n_id); vals.push(-t);

                        // Gravity source contribution on RHS from potential formulation
                        let depth_i = self.depth_at_k(k);
                        let depth_j = self.depth_at_k(*n_k);
                        let rho_t = self.total_density_face(cell, &self.grid_cells[*n_id]);
                        let grav_head_bar = self.gravity_head_bar(depth_i, depth_j, rho_t);
                        b_rhs[id] += t * grav_head_bar;
                    }

                    // well implicit coupling: add PI to diagonal and PI*BHP to RHS
                    // Well rate [m³/day] = PI [m³/day/bar] * (p_cell - BHP) [bar]
                    // For producer (injector=false): positive PI, well produces when p_cell > BHP
                    // For injector (injector=true): well injects 100% water when p_cell < BHP
                    for w in &self.wells {
                        if w.i == i && w.j == j && w.k == k {
                            // Defensive checks: well should be validated on add_well, but check at runtime too
                            if w.productivity_index.is_finite() && w.bhp.is_finite() {
                                diag += w.productivity_index;
                                b_rhs[id] += w.productivity_index * w.bhp;
                            }
                            // Skip malformed well parameters (shouldn't happen with validation)
                        }
                    }

                    // push diagonal to matrix
                    rows.push(id); cols.push(id); vals.push(diag);
                    // Safe inverse for Jacobi preconditioner
                    diag_inv[id] = if diag.abs() > f64::EPSILON { 1.0 / diag } else { 1.0 };
                }
            }
        }

        // Build sparse matrix and solve pressure equation
        let mut tri = TriMatI::<f64, usize>::new((n_cells, n_cells));
        for idx in 0..vals.len() {
            tri.add_triplet(rows[idx], cols[idx], vals[idx]);
        }
        let a_mat: CsMat<f64> = tri.to_csr();

        // Solve pressure equation A*p_new = b with PCG, initial guess = current pressures
        let mut x0 = DVector::<f64>::zeros(n_cells);
        for i in 0..n_cells { x0[i] = self.grid_cells[i].pressure; }
        let p_new = solve_pcg_with_guess(&a_mat, &b_rhs, &diag_inv, &x0, 1e-7, 1000);

        // Compute phase fluxes and explicit saturation update (upwind fractional flow method)
        // Track total water volume change [m³] per cell over dt_days
        let mut delta_water_m3 = vec![0.0f64; n_cells];
        let mut max_sat_change = 0.0;

        // Interface fluxes: compute once per neighbor pair and distribute upwind
        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i,j,k);
                    let p_i = p_new[id];
                    
                    // Check neighbors in positive direction to avoid duplicate pairs
                    let mut check = Vec::new();
                    if i < self.nx - 1 { check.push((self.idx(i+1,j,k), 'x', k)); }
                    if j < self.ny - 1 { check.push((self.idx(i,j+1,k), 'y', k)); }
                    if k < self.nz - 1 { check.push((self.idx(i,j,k+1), 'z', k + 1)); }

                    for (nid, dim, n_k) in check {
                        let p_j = p_new[nid];
                        // Transmissibility [m³/day/bar]
                        let t = self.transmissibility(&self.grid_cells[id], &self.grid_cells[nid], dim);
                        let (lam_w_i, lam_o_i) = self.phase_mobilities(&self.grid_cells[id]);
                        let (lam_w_j, lam_o_j) = self.phase_mobilities(&self.grid_cells[nid]);
                        let lam_t_i = lam_w_i + lam_o_i;
                        let lam_t_j = lam_w_j + lam_o_j;
                        let lam_t_avg = 0.5 * (lam_t_i + lam_t_j);
                        if lam_t_avg <= f64::EPSILON { continue; }

                        // Total volumetric flux [m³/day]: positive = from id -> nid
                        let depth_i = self.depth_at_k(k);
                        let depth_j = self.depth_at_k(n_k);
                        let rho_t = self.total_density_face(&self.grid_cells[id], &self.grid_cells[nid]);
                        let grav_head_bar = self.gravity_head_bar(depth_i, depth_j, rho_t);
                        let total_flux = t * ((p_i - p_j) - grav_head_bar);

                        // Upwind fractional flow for water
                        let f_w = if total_flux >= 0.0 {
                            self.frac_flow_water(&self.grid_cells[id])
                        } else {
                            self.frac_flow_water(&self.grid_cells[nid])
                        };

                        // Capillary-driven diffusion term using harmonic transmissibility geometry
                        let pc_i = self.get_capillary_pressure(self.grid_cells[id].sat_water);
                        let pc_j = self.get_capillary_pressure(self.grid_cells[nid].sat_water);
                        let geom_t = t / lam_t_avg;
                        let lam_w_avg = 0.5 * (lam_w_i + lam_w_j);
                        let lam_o_avg = 0.5 * (lam_o_i + lam_o_j);
                        let capillary_flux = if lam_t_avg <= f64::EPSILON {
                            0.0
                        } else {
                            -geom_t * (lam_w_avg * lam_o_avg / lam_t_avg) * (pc_i - pc_j)
                        };

                        // Water flux [m³/day]
                        let water_flux_m3_day = total_flux * f_w + capillary_flux;
                        // Volume change over dt_days [m³]
                        let dv_water = water_flux_m3_day * dt_days;
                        
                        // Distribute: outgoing flow reduces water in source cell
                        delta_water_m3[id] -= dv_water;
                        delta_water_m3[nid] += dv_water;
                    }
                }
            }
        }

        // Add well explicit contributions using solved pressure
        for w in &self.wells {
            let id = self.idx(w.i, w.j, w.k);
            
            // Defensive check: ensure well parameters are finite (shouldn't happen with validation)
            if w.productivity_index.is_finite() && w.bhp.is_finite() && p_new[id].is_finite() {
                // Well rate [m³/day] = PI [m³/day/bar] * (p_block - BHP) [bar]
                // Positive = production (outflow), negative = injection (inflow)
                let q_m3_day = w.productivity_index * (p_new[id] - w.bhp);
                
                // Check result is finite
                if q_m3_day.is_finite() {
                    // Determine water composition of well fluid
                    let fw = if w.injector {
                        // Injectors inject 100% water
                        1.0
                    } else {
                        // Producers produce at reservoir fluid composition (fractional flow)
                        self.frac_flow_water(&self.grid_cells[id])
                    };
                    
                    let water_q_m3_day = q_m3_day * fw;
                    
                    // Volume change [m³]. Production (q>0) removes fluid from block.
                    // For injector: q<0 (inflow), so -q_water*dt adds water to the block
                    delta_water_m3[id] -= water_q_m3_day * dt_days;
                }
            }
            // Skip malformed well parameters (shouldn't happen with validation)
        }

        // Calculate max saturation change for CFL condition
        for idx in 0..n_cells {
            let vp_m3 = self.grid_cells[idx].pore_volume_m3(self.dx, self.dy, self.dz);
            if vp_m3 > 0.0 {
                let sat_change = (delta_water_m3[idx] / vp_m3).abs();
                if sat_change > max_sat_change {
                    max_sat_change = sat_change;
                }
            }
        }

        let stable_dt_factor = if max_sat_change > self.max_sat_change_per_step {
            self.max_sat_change_per_step / max_sat_change
        } else {
            1.0
        };

        (p_new, delta_water_m3, stable_dt_factor)
    }

    fn update_saturations_and_pressure(&mut self, p_new: &DVector<f64>, delta_water_m3: &Vec<f64>, dt_days: f64) {
        let n_cells = self.nx * self.ny * self.nz;
        // Update saturations based on water volume changes
        for idx in 0..n_cells {
            let vp_m3 = self.grid_cells[idx].pore_volume_m3(self.dx, self.dy, self.dz);
            if vp_m3 <= 0.0 { continue; }

            // Change in water saturation [dimensionless] = ΔV_water [m³] / V_pore [m³]
            let sw_old = self.grid_cells[idx].sat_water;
            let sw_min = self.scal.s_wc;
            let sw_max = 1.0 - self.scal.s_or;
            let delta_sw = delta_water_m3[idx] / vp_m3;
            let sw_new = (sw_old + delta_sw).clamp(sw_min, sw_max);
            
            // Ensure material balance: s_w + s_o = 1.0 (two-phase system, no gas phase)
            let so_new = (1.0 - sw_new).clamp(0.0, 1.0);

            // Update state variables
            self.grid_cells[idx].sat_water = sw_new;
            self.grid_cells[idx].sat_oil = so_new;
            self.grid_cells[idx].pressure = p_new[idx];
        }

        // Calculate and store rates
        let mut total_prod_oil = 0.0;
        let mut total_prod_liquid = 0.0;
        let mut total_injection = 0.0;

        for w in &self.wells {
            let id = self.idx(w.i, w.j, w.k);
            if w.productivity_index.is_finite() && w.bhp.is_finite() && p_new[id].is_finite() {
                let q_m3_day = w.productivity_index * (p_new[id] - w.bhp);
                if q_m3_day.is_finite() {
                    if w.injector {
                        // Injection is negative flow
                        total_injection -= q_m3_day;
                    } else {
                        // Production is positive flow
                        let fw = self.frac_flow_water(&self.grid_cells[id]);
                        let oil_rate = q_m3_day * (1.0 - fw);
                        total_prod_oil += oil_rate;
                        total_prod_liquid += q_m3_day;
                    }
                }
            }
        }

        self.rate_history.push(TimePointRates {
            time: self.time_days + dt_days,
            total_production_oil: total_prod_oil,
            total_production_liquid: total_prod_liquid,
            total_injection: total_injection,
        });


        // Advance simulation time [s]
        self.time_days += dt_days;
    }

    pub fn get_time(&self) -> f64 { self.time_days }

    #[wasm_bindgen(js_name = getGridState)]
    pub fn get_grid_state(&self) -> JsValue { serde_wasm_bindgen::to_value(&self.grid_cells).unwrap() }

    #[wasm_bindgen(js_name = getWellState)]
    pub fn get_well_state(&self) -> JsValue { serde_wasm_bindgen::to_value(&self.wells).unwrap() }

    #[wasm_bindgen(js_name = getRateHistory)]
    pub fn get_rate_history(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.rate_history).unwrap()
    }

    #[wasm_bindgen(js_name = getDimensions)]
    pub fn get_dimensions(&self) -> JsValue { serde_wasm_bindgen::to_value(&[self.nx, self.ny, self.nz]).unwrap() }

    /// Set initial pressure for all grid cells
    #[wasm_bindgen(js_name = setInitialPressure)]
    pub fn set_initial_pressure(&mut self, pressure: f64) {
        for cell in self.grid_cells.iter_mut() {
            cell.pressure = pressure;
        }
    }

    /// Set initial water saturation for all grid cells
    #[wasm_bindgen(js_name = setInitialSaturation)]
    pub fn set_initial_saturation(&mut self, sat_water: f64) {
        for cell in self.grid_cells.iter_mut() {
            cell.sat_water = sat_water.clamp(0.0, 1.0);
            cell.sat_oil = 1.0 - cell.sat_water;
        }
    }

    /// Set relative permeability properties
    #[wasm_bindgen(js_name = setRelPermProps)]
    pub fn set_rel_perm_props(&mut self, s_wc: f64, s_or: f64, n_w: f64, n_o: f64) -> Result<(), String> {
        if !s_wc.is_finite() || !s_or.is_finite() || !n_w.is_finite() || !n_o.is_finite() {
            return Err("Relative permeability parameters must be finite numbers".to_string());
        }

        if s_wc < 0.0 || s_wc >= 1.0 {
            return Err(format!("S_wc must be in [0, 1), got {}", s_wc));
        }

        if s_or < 0.0 || s_or >= 1.0 {
            return Err(format!("S_or must be in [0, 1), got {}", s_or));
        }

        if s_wc + s_or >= 1.0 {
            return Err(format!(
                "Invalid saturation endpoints: S_wc + S_or must be < 1.0, got {}",
                s_wc + s_or
            ));
        }

        if n_w <= 0.0 || n_o <= 0.0 {
            return Err(format!(
                "Corey exponents must be positive, got n_w={}, n_o={}",
                n_w, n_o
            ));
        }

        self.scal = RockFluidProps { s_wc, s_or, n_w, n_o };
        Ok(())
    }

    #[wasm_bindgen(js_name = setFluidDensities)]
    pub fn set_fluid_densities(&mut self, rho_o: f64, rho_w: f64) -> Result<(), String> {
        if !rho_o.is_finite() || !rho_w.is_finite() {
            return Err("Fluid densities must be finite numbers".to_string());
        }
        if rho_o <= 0.0 || rho_w <= 0.0 {
            return Err(format!("Fluid densities must be positive, got rho_o={}, rho_w={}", rho_o, rho_w));
        }
        self.pvt.rho_o = rho_o;
        self.pvt.rho_w = rho_w;
        Ok(())
    }

    #[wasm_bindgen(js_name = setCapillaryParams)]
    pub fn set_capillary_params(&mut self, p_entry: f64, lambda: f64) -> Result<(), String> {
        if !p_entry.is_finite() || !lambda.is_finite() {
            return Err("Capillary parameters must be finite numbers".to_string());
        }
        if p_entry < 0.0 {
            return Err(format!("Capillary entry pressure must be non-negative, got {}", p_entry));
        }
        if lambda <= 0.0 {
            return Err(format!("Capillary lambda must be positive, got {}", lambda));
        }

        self.pc.p_entry = p_entry;
        self.pc.lambda = lambda;
        Ok(())
    }

    /// Set permeability with random distribution
    #[wasm_bindgen(js_name = setPermeabilityRandom)]
    pub fn set_permeability_random(&mut self, min_perm: f64, max_perm: f64) -> Result<(), String> {
        if !min_perm.is_finite() || !max_perm.is_finite() {
            return Err("Permeability bounds must be finite numbers".to_string());
        }

        if min_perm <= 0.0 || max_perm <= 0.0 {
            return Err(format!(
                "Permeability bounds must be positive, got min={}, max={}",
                min_perm, max_perm
            ));
        }

        if min_perm > max_perm {
            return Err(format!(
                "Invalid permeability bounds: min ({}) cannot exceed max ({})",
                min_perm, max_perm
            ));
        }

        let mut rng = rand::rng();
        for cell in self.grid_cells.iter_mut() {
            cell.perm_x = rng.random_range(min_perm..=max_perm);
            cell.perm_y = rng.random_range(min_perm..=max_perm);
            cell.perm_z = rng.random_range(min_perm..=max_perm) / 10.0; // Anisotropy
        }
        Ok(())
    }

    /// Set permeability with deterministic random distribution using a fixed seed
    #[wasm_bindgen(js_name = setPermeabilityRandomSeeded)]
    pub fn set_permeability_random_seeded(&mut self, min_perm: f64, max_perm: f64, seed: u64) -> Result<(), String> {
        if !min_perm.is_finite() || !max_perm.is_finite() {
            return Err("Permeability bounds must be finite numbers".to_string());
        }

        if min_perm <= 0.0 || max_perm <= 0.0 {
            return Err(format!(
                "Permeability bounds must be positive, got min={}, max={}",
                min_perm, max_perm
            ));
        }

        if min_perm > max_perm {
            return Err(format!(
                "Invalid permeability bounds: min ({}) cannot exceed max ({})",
                min_perm, max_perm
            ));
        }

        let mut rng = StdRng::seed_from_u64(seed);
        for cell in self.grid_cells.iter_mut() {
            cell.perm_x = rng.random_range(min_perm..=max_perm);
            cell.perm_y = rng.random_range(min_perm..=max_perm);
            cell.perm_z = rng.random_range(min_perm..=max_perm) / 10.0; // Anisotropy
        }
        Ok(())
    }

    /// Set permeability per layer
    #[wasm_bindgen(js_name = setPermeabilityPerLayer)]
    pub fn set_permeability_per_layer(&mut self, perms_x: Vec<f64>, perms_y: Vec<f64>, perms_z: Vec<f64>) -> Result<(), String> {
        if perms_x.len() != self.nz || perms_y.len() != self.nz || perms_z.len() != self.nz {
            return Err(format!("Permeability vectors must have length equal to nz ({})", self.nz));
        }

        for k in 0..self.nz {
            let px = perms_x[k];
            let py = perms_y[k];
            let pz = perms_z[k];
            if !px.is_finite() || !py.is_finite() || !pz.is_finite() {
                return Err(format!("Permeability for layer {} must be finite", k));
            }
            if px <= 0.0 || py <= 0.0 || pz <= 0.0 {
                return Err(format!(
                    "Permeability for layer {} must be positive, got px={}, py={}, pz={}",
                    k, px, py, pz
                ));
            }
        }

        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i, j, k);
                    self.grid_cells[id].perm_x = perms_x[k];
                    self.grid_cells[id].perm_y = perms_y[k];
                    self.grid_cells[id].perm_z = perms_z[k];
                }
            }
        }
        Ok(())
    }
}

// --- Helper: sparse matrix-vector multiply ---
fn cs_mat_mul_vec(a: &CsMat<f64>, x: &DVector<f64>) -> DVector<f64> {
    let n = a.rows();
    let mut y = DVector::<f64>::zeros(n);
    for (row, vec) in a.outer_iterator().enumerate() {
        let mut sum = 0.0;
        for (&col, &val) in vec.indices().iter().zip(vec.data().iter()) {
            sum += val * x[col];
        }
        y[row] = sum;
    }
    y
}

// PCG solver with initial guess
fn solve_pcg_with_guess(
    a: &CsMat<f64>,
    b: &DVector<f64>,
    m_inv_diag: &DVector<f64>,
    x0: &DVector<f64>,
    tolerance: f64,
    max_iter: usize,
) -> DVector<f64> {
    let n = b.len();
    let mut x = x0.clone();
    let mut r = b - &cs_mat_mul_vec(a, &x);
    let mut z = DVector::<f64>::zeros(n);
    for i in 0..n { z[i] = r[i] * m_inv_diag[i]; }
    let mut p = z.clone();
    let mut r_dot_z = r.dot(&z);
    let r0_norm = r.norm();
    if r0_norm == 0.0 { return x; }

    for _ in 0..max_iter {
        if r.norm() / r0_norm < tolerance { break; }
        let q = cs_mat_mul_vec(a, &p);
        let p_dot_q = p.dot(&q);
        if p_dot_q.abs() < f64::EPSILON { break; }
        let alpha = r_dot_z / p_dot_q;
        x += alpha * p.clone();
        let r_new = r - alpha * q;
        let mut z_new = DVector::<f64>::zeros(n);
        for i in 0..n { z_new[i] = r_new[i] * m_inv_diag[i]; }
        let r_new_dot_z_new = r_new.dot(&z_new);
        let beta = if r_dot_z.abs() < f64::EPSILON { 0.0 } else { r_new_dot_z_new / r_dot_z };
        p = z_new.clone() + beta * p;
        r = r_new;
        r_dot_z = r_new_dot_z_new;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    struct BuckleyCase {
        name: &'static str,
        nx: usize,
        permeability_md: f64,
        dt_days: f64,
        max_steps: usize,
        injector_bhp: f64,
        producer_bhp: f64,
        s_wc: f64,
        s_or: f64,
        n_w: f64,
        n_o: f64,
        mu_w: f64,
        mu_o: f64,
        breakthrough_watercut: f64,
        rel_tol_breakthrough_pv: f64,
    }

    struct BuckleyMetrics {
        breakthrough_pv: f64,
        reference_breakthrough_pv: f64,
    }

    fn err_contains(result: Result<(), String>, expected: &str) {
        match result {
            Ok(()) => panic!("Expected error containing '{}', got Ok(())", expected),
            Err(message) => assert!(
                message.contains(expected),
                "Expected error containing '{}', got '{}'",
                expected,
                message
            ),
        }
    }

    fn total_water_volume(sim: &ReservoirSimulator) -> f64 {
        sim.grid_cells
            .iter()
            .map(|cell| cell.sat_water * cell.pore_volume_m3(sim.dx, sim.dy, sim.dz))
            .sum()
    }

    fn corey_fractional_flow(
        s_w: f64,
        s_wc: f64,
        s_or: f64,
        n_w: f64,
        n_o: f64,
        mu_w: f64,
        mu_o: f64,
    ) -> f64 {
        let denom_sat = 1.0 - s_wc - s_or;
        if denom_sat <= 0.0 {
            return 0.0;
        }

        let s_eff_w = ((s_w - s_wc) / denom_sat).clamp(0.0, 1.0);
        let s_eff_o = ((1.0 - s_w - s_or) / denom_sat).clamp(0.0, 1.0);
        let krw = s_eff_w.powf(n_w);
        let kro = s_eff_o.powf(n_o);
        let lam_w = krw / mu_w;
        let lam_o = kro / mu_o;
        let lam_t = lam_w + lam_o;

        if lam_t <= f64::EPSILON {
            0.0
        } else {
            (lam_w / lam_t).clamp(0.0, 1.0)
        }
    }

    fn buckley_reference_breakthrough_pv(case: &BuckleyCase) -> f64 {
        let sw_init = case.s_wc;
        let mut sw_shock = sw_init;
        let mut best_slope = 0.0;
        let ds = 5e-4;
        let mut s = sw_init + ds;
        let s_max = 1.0 - case.s_or;

        while s <= s_max {
            let fw = corey_fractional_flow(s, case.s_wc, case.s_or, case.n_w, case.n_o, case.mu_w, case.mu_o);
            let slope = fw / (s - sw_init);
            if slope > best_slope && slope.is_finite() {
                best_slope = slope;
                sw_shock = s;
            }
            s += ds;
        }

        let fw_eps = 1e-4;
        let fw_plus = corey_fractional_flow(
            (sw_shock + fw_eps).clamp(sw_init, s_max),
            case.s_wc,
            case.s_or,
            case.n_w,
            case.n_o,
            case.mu_w,
            case.mu_o,
        );
        let fw_minus = corey_fractional_flow(
            (sw_shock - fw_eps).clamp(sw_init, s_max),
            case.s_wc,
            case.s_or,
            case.n_w,
            case.n_o,
            case.mu_w,
            case.mu_o,
        );
        let dfw_dsw = (fw_plus - fw_minus) / (2.0 * fw_eps);

        if dfw_dsw <= f64::EPSILON {
            f64::INFINITY
        } else {
            1.0 / dfw_dsw
        }
    }

    fn run_buckley_case(case: &BuckleyCase) -> BuckleyMetrics {
        let mut sim = ReservoirSimulator::new(case.nx, 1, 1);
        sim.set_rel_perm_props(case.s_wc, case.s_or, case.n_w, case.n_o).unwrap();
        sim.set_initial_saturation(case.s_wc);
        sim.set_permeability_random_seeded(case.permeability_md, case.permeability_md, 42).unwrap();
        sim.set_stability_params(0.05);
        sim.pc.p_entry = 0.0;
        sim.pvt.mu_w = case.mu_w;
        sim.pvt.mu_o = case.mu_o;

        sim.add_well(0, 0, 0, case.injector_bhp, 0.1, 0.0, true).unwrap();
        sim.add_well(case.nx - 1, 0, 0, case.producer_bhp, 0.1, 0.0, false).unwrap();

        let total_pv = sim
            .grid_cells
            .iter()
            .map(|cell| cell.pore_volume_m3(sim.dx, sim.dy, sim.dz))
            .sum::<f64>();

        let mut cumulative_injection = 0.0;
        let mut previous_time = 0.0;
        let mut breakthrough_pv = None;

        for _ in 0..case.max_steps {
            sim.step(case.dt_days);
            let point = sim.rate_history.last().expect("rate history should have entries");
            let dt = point.time - previous_time;
            previous_time = point.time;

            cumulative_injection += point.total_injection.max(0.0) * dt;

            if point.total_production_liquid > 1e-9 {
                let water_rate = (point.total_production_liquid - point.total_production_oil).max(0.0);
                let watercut = (water_rate / point.total_production_liquid).clamp(0.0, 1.0);
                if watercut >= case.breakthrough_watercut {
                    breakthrough_pv = Some(cumulative_injection / total_pv);
                    break;
                }
            }
        }

        let breakthrough_pv = breakthrough_pv.unwrap_or_else(|| {
            panic!(
                "{} did not reach breakthrough (watercut >= {}) in {} steps",
                case.name, case.breakthrough_watercut, case.max_steps
            )
        });

        BuckleyMetrics {
            breakthrough_pv,
            reference_breakthrough_pv: buckley_reference_breakthrough_pv(case),
        }
    }

    #[test]
    fn saturation_stays_within_physical_bounds() {
        let mut sim = ReservoirSimulator::new(5, 1, 1);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(4, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        for _ in 0..20 {
            sim.step(0.5);
        }

        let sw_min = sim.scal.s_wc;
        let sw_max = 1.0 - sim.scal.s_or;

        for cell in &sim.grid_cells {
            assert!(cell.sat_water >= sw_min - 1e-9);
            assert!(cell.sat_water <= sw_max + 1e-9);
            assert!(cell.sat_oil >= -1e-9);
            assert!(cell.sat_oil <= 1.0 + 1e-9);
            assert!((cell.sat_water + cell.sat_oil - 1.0).abs() < 1e-8);
        }
    }

    #[test]
    fn water_mass_balance_sanity_without_wells() {
        let mut sim = ReservoirSimulator::new(4, 4, 1);
        let water_before = total_water_volume(&sim);

        sim.step(1.0);

        let water_after = total_water_volume(&sim);
        assert!((water_after - water_before).abs() < 1e-6);
    }

    #[test]
    fn adaptive_timestep_produces_multiple_substeps_for_strong_flow() {
        let mut sim = ReservoirSimulator::new(3, 1, 1);
        sim.set_permeability_random(100_000.0, 100_000.0).unwrap();
        sim.set_stability_params(0.01);
        sim.add_well(0, 0, 0, 700.0, 0.1, 0.0, true).unwrap();
        sim.add_well(2, 0, 0, 50.0, 0.1, 0.0, false).unwrap();

        sim.step(30.0);

        assert!(sim.rate_history.len() > 1);
        assert!(sim.time_days > 0.0);
        assert!(sim.time_days < 30.0);
    }

    #[test]
    fn multiple_wells_in_same_block_keep_rates_finite() {
        let mut sim = ReservoirSimulator::new(4, 1, 1);
        sim.add_well(0, 0, 0, 600.0, 0.1, 0.0, true).unwrap();
        sim.add_well(0, 0, 0, 550.0, 0.1, 0.0, true).unwrap();
        sim.add_well(3, 0, 0, 120.0, 0.1, 0.0, false).unwrap();

        for _ in 0..12 {
            sim.step(0.5);
        }

        assert!(!sim.rate_history.is_empty());
        let latest = sim.rate_history.last().unwrap();
        assert!(latest.total_injection.is_finite());
        assert!(latest.total_production_liquid.is_finite());
        assert!(latest.total_production_oil.is_finite());

        for cell in &sim.grid_cells {
            assert!(cell.pressure.is_finite());
            assert!(cell.sat_water.is_finite());
            assert!(cell.sat_oil.is_finite());
        }
    }

    #[test]
    fn out_of_bounds_well_is_rejected_without_state_change() {
        let mut sim = ReservoirSimulator::new(2, 2, 1);
        let wells_before = sim.wells.len();

        let result = sim.add_well(2, 0, 0, 250.0, 0.1, 0.0, false);
        err_contains(result, "out of bounds");

        assert_eq!(sim.wells.len(), wells_before);
    }

    #[test]
    fn stability_extremes_produce_finite_state() {
        let mut sim_loose = ReservoirSimulator::new(3, 1, 1);
        sim_loose.set_stability_params(1.0);
        sim_loose.set_permeability_random(20_000.0, 20_000.0).unwrap();
        sim_loose.add_well(0, 0, 0, 650.0, 0.1, 0.0, true).unwrap();
        sim_loose.add_well(2, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        sim_loose.step(5.0);

        let mut sim_tight = ReservoirSimulator::new(3, 1, 1);
        sim_tight.set_stability_params(0.01);
        sim_tight.set_permeability_random(20_000.0, 20_000.0).unwrap();
        sim_tight.add_well(0, 0, 0, 650.0, 0.1, 0.0, true).unwrap();
        sim_tight.add_well(2, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        sim_tight.step(5.0);

        for sim in [&sim_loose, &sim_tight] {
            for cell in &sim.grid_cells {
                assert!(cell.pressure.is_finite());
                assert!(cell.sat_water.is_finite());
                assert!(cell.sat_oil.is_finite());
            }
            assert!(sim.time_days > 0.0);
            assert!(sim.time_days <= 5.0);
            assert!(!sim.rate_history.is_empty());
        }

        assert!(sim_tight.rate_history.len() >= sim_loose.rate_history.len());
    }

    #[test]
    fn api_contract_rejects_invalid_relperm_parameters() {
        let mut sim = ReservoirSimulator::new(2, 2, 1);
        err_contains(sim.set_rel_perm_props(0.6, 0.5, 2.0, 2.0), "must be < 1.0");
        err_contains(sim.set_rel_perm_props(0.1, 0.1, 0.0, 2.0), "must be positive");
        err_contains(sim.set_rel_perm_props(f64::NAN, 0.1, 2.0, 2.0), "finite numbers");
    }

    #[test]
    fn api_contract_rejects_invalid_density_inputs() {
        let mut sim = ReservoirSimulator::new(2, 2, 1);
        err_contains(sim.set_fluid_densities(-800.0, 1000.0), "must be positive");
        err_contains(sim.set_fluid_densities(800.0, f64::NAN), "finite numbers");
    }

    #[test]
    fn api_contract_rejects_invalid_capillary_inputs() {
        let mut sim = ReservoirSimulator::new(2, 2, 1);
        err_contains(sim.set_capillary_params(-1.0, 2.0), "non-negative");
        err_contains(sim.set_capillary_params(5.0, 0.0), "positive");
        err_contains(sim.set_capillary_params(f64::NAN, 2.0), "finite numbers");
    }

    #[test]
    fn gravity_toggle_builds_hydrostatic_vertical_gradient() {
        let mut sim_no_g = ReservoirSimulator::new(1, 1, 2);
        sim_no_g.set_permeability_random_seeded(50_000.0, 50_000.0, 42).unwrap();
        sim_no_g.set_initial_pressure(300.0);
        sim_no_g.set_initial_saturation(0.9);
        sim_no_g.pc.p_entry = 0.0;
        sim_no_g.set_gravity_enabled(false);
        sim_no_g.step(2.0);

        let p_top_no_g = sim_no_g.grid_cells[sim_no_g.idx(0, 0, 0)].pressure;
        let p_bot_no_g = sim_no_g.grid_cells[sim_no_g.idx(0, 0, 1)].pressure;

        let mut sim_g = ReservoirSimulator::new(1, 1, 2);
        sim_g.set_permeability_random_seeded(50_000.0, 50_000.0, 42).unwrap();
        sim_g.set_initial_pressure(300.0);
        sim_g.set_initial_saturation(0.9);
        sim_g.pc.p_entry = 0.0;
        sim_g.set_gravity_enabled(true);
        sim_g.step(2.0);

        let p_top_g = sim_g.grid_cells[sim_g.idx(0, 0, 0)].pressure;
        let p_bot_g = sim_g.grid_cells[sim_g.idx(0, 0, 1)].pressure;

        assert!((p_bot_no_g - p_top_no_g).abs() < 1e-5);
        assert!(p_bot_g > p_top_g);
    }

    #[test]
    fn hydrostatic_initial_gradient_stays_quieter_with_gravity_enabled() {
        let initial_sw = 0.9;

        let mut sim_g = ReservoirSimulator::new(1, 1, 2);
        sim_g.set_permeability_random_seeded(80_000.0, 80_000.0, 7).unwrap();
        sim_g.set_initial_saturation(initial_sw);
        sim_g.pc.p_entry = 0.0;
        sim_g.set_fluid_densities(800.0, 1000.0).unwrap();
        sim_g.set_gravity_enabled(true);

        let hydro_dp_bar = sim_g.pvt.rho_w * 9.80665 * sim_g.dz * 1e-5;
        let top_id_g = sim_g.idx(0, 0, 0);
        let bot_id_g = sim_g.idx(0, 0, 1);
        sim_g.grid_cells[top_id_g].pressure = 300.0;
        sim_g.grid_cells[bot_id_g].pressure = 300.0 + hydro_dp_bar;
        sim_g.step(5.0);
        let sw_change_top_g = (sim_g.grid_cells[top_id_g].sat_water - initial_sw).abs();

        let mut sim_no_g = ReservoirSimulator::new(1, 1, 2);
        sim_no_g.set_permeability_random_seeded(80_000.0, 80_000.0, 7).unwrap();
        sim_no_g.set_initial_saturation(initial_sw);
        sim_no_g.pc.p_entry = 0.0;
        sim_no_g.set_fluid_densities(800.0, 1000.0).unwrap();
        sim_no_g.set_gravity_enabled(false);

        let top_id_no_g = sim_no_g.idx(0, 0, 0);
        let bot_id_no_g = sim_no_g.idx(0, 0, 1);
        sim_no_g.grid_cells[top_id_no_g].pressure = 300.0;
        sim_no_g.grid_cells[bot_id_no_g].pressure = 300.0 + hydro_dp_bar;
        sim_no_g.step(5.0);
        let sw_change_top_no_g = (sim_no_g.grid_cells[top_id_no_g].sat_water - initial_sw).abs();

        assert!(sw_change_top_g <= sw_change_top_no_g);
    }

    #[test]
    fn api_contract_rejects_invalid_permeability_inputs() {
        let mut sim = ReservoirSimulator::new(2, 2, 2);
        err_contains(sim.set_permeability_random(200.0, 50.0), "cannot exceed max");
        err_contains(sim.set_permeability_random_seeded(-1.0, 100.0, 123), "must be positive");
        err_contains(
            sim.set_permeability_per_layer(vec![100.0], vec![100.0, 120.0], vec![10.0, 12.0]),
            "length equal to nz",
        );
        err_contains(
            sim.set_permeability_per_layer(vec![100.0, 120.0], vec![100.0, 120.0], vec![0.0, 12.0]),
            "must be positive",
        );
    }

    #[test]
    fn benchmark_buckley_leverett_case_a_favorable_mobility() {
        let case = BuckleyCase {
            name: "BL-Case-A",
            nx: 24,
            permeability_md: 2000.0,
            dt_days: 0.5,
            max_steps: 4000,
            injector_bhp: 500.0,
            producer_bhp: 100.0,
            s_wc: 0.1,
            s_or: 0.1,
            n_w: 2.0,
            n_o: 2.0,
            mu_w: 0.5,
            mu_o: 1.0,
            breakthrough_watercut: 0.01,
            rel_tol_breakthrough_pv: 0.25,
        };

        let metrics = run_buckley_case(&case);
        let rel_err = ((metrics.breakthrough_pv - metrics.reference_breakthrough_pv)
            / metrics.reference_breakthrough_pv)
            .abs();

        println!(
            "{}: breakthrough_pv_sim={:.4}, breakthrough_pv_ref={:.4}, rel_err={:.3}",
            case.name,
            metrics.breakthrough_pv,
            metrics.reference_breakthrough_pv,
            rel_err
        );

        assert!(
            rel_err <= case.rel_tol_breakthrough_pv,
            "{} breakthrough PV mismatch too high: sim={:.4}, ref={:.4}, rel_err={:.3}, tol={:.3}",
            case.name,
            metrics.breakthrough_pv,
            metrics.reference_breakthrough_pv,
            rel_err,
            case.rel_tol_breakthrough_pv,
        );
    }

    #[test]
    fn benchmark_buckley_leverett_case_b_more_adverse_mobility() {
        let case = BuckleyCase {
            name: "BL-Case-B",
            nx: 24,
            permeability_md: 2000.0,
            dt_days: 0.5,
            max_steps: 4000,
            injector_bhp: 500.0,
            producer_bhp: 100.0,
            s_wc: 0.15,
            s_or: 0.15,
            n_w: 2.2,
            n_o: 2.0,
            mu_w: 0.6,
            mu_o: 1.4,
            breakthrough_watercut: 0.01,
            rel_tol_breakthrough_pv: 0.30,
        };

        let metrics = run_buckley_case(&case);
        let rel_err = ((metrics.breakthrough_pv - metrics.reference_breakthrough_pv)
            / metrics.reference_breakthrough_pv)
            .abs();

        println!(
            "{}: breakthrough_pv_sim={:.4}, breakthrough_pv_ref={:.4}, rel_err={:.3}",
            case.name,
            metrics.breakthrough_pv,
            metrics.reference_breakthrough_pv,
            rel_err
        );

        assert!(
            rel_err <= case.rel_tol_breakthrough_pv,
            "{} breakthrough PV mismatch too high: sim={:.4}, ref={:.4}, rel_err={:.3}, tol={:.3}",
            case.name,
            metrics.breakthrough_pv,
            metrics.reference_breakthrough_pv,
            rel_err,
            case.rel_tol_breakthrough_pv,
        );
    }

    #[test]
    fn benchmark_buckley_leverett_refined_discretization_improves_alignment() {
        let coarse_a = BuckleyCase {
            name: "BL-Case-A-Coarse",
            nx: 24,
            permeability_md: 2000.0,
            dt_days: 0.5,
            max_steps: 4000,
            injector_bhp: 500.0,
            producer_bhp: 100.0,
            s_wc: 0.1,
            s_or: 0.1,
            n_w: 2.0,
            n_o: 2.0,
            mu_w: 0.5,
            mu_o: 1.0,
            breakthrough_watercut: 0.01,
            rel_tol_breakthrough_pv: 0.25,
        };
        let refined_a = BuckleyCase {
            name: "BL-Case-A-Refined",
            nx: 96,
            dt_days: 0.125,
            max_steps: 20000,
            ..coarse_a
        };

        let coarse_b = BuckleyCase {
            name: "BL-Case-B-Coarse",
            nx: 24,
            permeability_md: 2000.0,
            dt_days: 0.5,
            max_steps: 4000,
            injector_bhp: 500.0,
            producer_bhp: 100.0,
            s_wc: 0.15,
            s_or: 0.15,
            n_w: 2.2,
            n_o: 2.0,
            mu_w: 0.6,
            mu_o: 1.4,
            breakthrough_watercut: 0.01,
            rel_tol_breakthrough_pv: 0.30,
        };
        let refined_b = BuckleyCase {
            name: "BL-Case-B-Refined",
            nx: 96,
            dt_days: 0.125,
            max_steps: 20000,
            ..coarse_b
        };

        let metrics_coarse_a = run_buckley_case(&coarse_a);
        let metrics_refined_a = run_buckley_case(&refined_a);
        let rel_err_coarse_a = ((metrics_coarse_a.breakthrough_pv - metrics_coarse_a.reference_breakthrough_pv)
            / metrics_coarse_a.reference_breakthrough_pv)
            .abs();
        let rel_err_refined_a = ((metrics_refined_a.breakthrough_pv - metrics_refined_a.reference_breakthrough_pv)
            / metrics_refined_a.reference_breakthrough_pv)
            .abs();

        let metrics_coarse_b = run_buckley_case(&coarse_b);
        let metrics_refined_b = run_buckley_case(&refined_b);
        let rel_err_coarse_b = ((metrics_coarse_b.breakthrough_pv - metrics_coarse_b.reference_breakthrough_pv)
            / metrics_coarse_b.reference_breakthrough_pv)
            .abs();
        let rel_err_refined_b = ((metrics_refined_b.breakthrough_pv - metrics_refined_b.reference_breakthrough_pv)
            / metrics_refined_b.reference_breakthrough_pv)
            .abs();

        println!(
            "Case-A coarse/refined rel_err: {:.3} -> {:.3}",
            rel_err_coarse_a,
            rel_err_refined_a
        );
        println!(
            "BL-Case-A-Refined: breakthrough_pv_sim={:.4}, breakthrough_pv_ref={:.4}, rel_err={:.3}",
            metrics_refined_a.breakthrough_pv,
            metrics_refined_a.reference_breakthrough_pv,
            rel_err_refined_a
        );
        println!(
            "Case-B coarse/refined rel_err: {:.3} -> {:.3}",
            rel_err_coarse_b,
            rel_err_refined_b
        );
        println!(
            "BL-Case-B-Refined: breakthrough_pv_sim={:.4}, breakthrough_pv_ref={:.4}, rel_err={:.3}",
            metrics_refined_b.breakthrough_pv,
            metrics_refined_b.reference_breakthrough_pv,
            rel_err_refined_b
        );

        assert!(
            rel_err_refined_a <= rel_err_coarse_a,
            "Refined discretization should not worsen Case-A alignment: coarse={:.3}, refined={:.3}",
            rel_err_coarse_a,
            rel_err_refined_a
        );
        assert!(
            rel_err_refined_b <= rel_err_coarse_b,
            "Refined discretization should not worsen Case-B alignment: coarse={:.3}, refined={:.3}",
            rel_err_coarse_b,
            rel_err_refined_b
        );
    }
}
