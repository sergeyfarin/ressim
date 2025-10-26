
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
}

impl FluidProperties {
    fn default_pvt() -> Self {
        Self {
            mu_o: 1.0,      // cP (typical oil)
            mu_w: 0.5,      // cP (water at reservoir conditions)
            c_o: 1e-5,      // 1/bar (oil compressibility)
            c_w: 3e-6,      // 1/bar (water compressibility)
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
        Self { s_wc: 0.2, s_or: 0.2, n_w: 2.0, n_o: 2.0 }
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
            dx: 100.0,   // meters (x-direction cell size)
            dy: 100.0,   // meters (y-direction cell size)
            dz: 20.0,    // meters (z-direction cell size)
            grid_cells,
            wells: Vec::new(),
            time_days: 0.0,  // simulation time in days
            pvt: FluidProperties::default_pvt(),
            scal: RockFluidProps::default_scal(),
            pc: CapillaryPressure::default_pc(),  // Brooks-Corey capillary pressure
        }
    }

    fn idx(&self, i: usize, j: usize, k: usize) -> usize {
        (k * self.nx * self.ny) + (j * self.nx) + i
    }


    /// Add a well to the simulator
    /// Parameters in oil-field units:
    /// - i, j, k: grid cell indices (must be within grid bounds)
    /// - bhp: bottom-hole pressure [bar] (must be finite, typical: -100 to 2000 bar)
    /// - pi: productivity index [m³/day/bar] (must be non-negative and finite)
    /// - injector: true for injector (injects fluid), false for producer (extracts fluid)
    /// 
    /// Returns Ok(()) on success, or Err(message) if parameters are invalid.
    /// Invalid parameters include:
    /// - Out-of-bounds grid indices
    /// - NaN or Inf values in bhp or pi
    /// - Negative productivity index
    /// - BHP outside reasonable range
    pub fn add_well(&mut self, i: usize, j: usize, k: usize, bhp: f64, pi: f64, injector: bool) -> Result<(), String> {
        let well = Well { i, j, k, bhp, productivity_index: pi, injector };
        
        // Validate well parameters
        well.validate(self.nx, self.ny, self.nz)?;
        
        self.wells.push(well);
        Ok(())
    }

    /// Total mobility [1/cP] = lambda_t = (k_rw/μ_w) + (k_ro/μ_o)
    /// Sum of phase mobilities used in pressure equation
    fn total_mobility(&self, cell: &GridCell) -> f64 {
        let krw = self.scal.k_rw(cell.sat_water);
        let kro = self.scal.k_ro(cell.sat_water);
        krw / self.pvt.mu_w + kro / self.pvt.mu_o
    }

    /// Get capillary pressure [bar] at given water saturation
    fn get_capillary_pressure(&self, s_w: f64) -> f64 {
        self.pc.capillary_pressure(s_w, &self.scal)
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
    pub fn step(&mut self, delta_t_days: f64) {
        let n_cells = self.nx * self.ny * self.nz;
        if n_cells == 0 { return; }
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
                    let mut neighbors: Vec<(usize, char)> = Vec::new();
                    if i > 0 { neighbors.push((self.idx(i-1,j,k), 'x')); }
                    if i < self.nx-1 { neighbors.push((self.idx(i+1,j,k), 'x')); }
                    if j > 0 { neighbors.push((self.idx(i,j-1,k), 'y')); }
                    if j < self.ny-1 { neighbors.push((self.idx(i,j+1,k), 'y')); }
                    if k > 0 { neighbors.push((self.idx(i,j,k-1), 'z')); }
                    if k < self.nz-1 { neighbors.push((self.idx(i,j,k+1), 'z')); }

                    for (n_id, dim) in neighbors.iter() {
                        // Transmissibility [m³/day/bar]
                        let t = self.transmissibility(cell, &self.grid_cells[*n_id], *dim);
                        diag += t;
                        rows.push(id); cols.push(*n_id); vals.push(-t);
                    }

                    // well implicit coupling: add PI to diagonal and PI*BHP to RHS
                    // Well rate [m³/day] = PI [m³/day/bar] * (p_cell - BHP) [bar]
                    for w in &self.wells {
                        if w.i == i && w.j == j && w.k == k {
                            // Defensive checks: well should be validated on add_well, but check at runtime too
                            if w.productivity_index.is_finite() && w.bhp.is_finite() {
                                // For producer: positive PI, well produces when p_cell > BHP
                                // For injector: set injector=true to control injection
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

        // Interface fluxes: compute once per neighbor pair and distribute upwind
        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i,j,k);
                    let p_i = p_new[id];
                    
                    // Check neighbors in positive direction to avoid duplicate pairs
                    let mut check = Vec::new();
                    if i < self.nx - 1 { check.push((self.idx(i+1,j,k), 'x')); }
                    if j < self.ny - 1 { check.push((self.idx(i,j+1,k), 'y')); }
                    if k < self.nz - 1 { check.push((self.idx(i,j,k+1), 'z')); }

                    for (nid, dim) in check {
                        let p_j = p_new[nid];
                        // Transmissibility [m³/day/bar]
                        let t = self.transmissibility(&self.grid_cells[id], &self.grid_cells[nid], dim);
                        
                        // Calculate capillary pressure [bar] at each cell
                        // P_c = P_oil - P_water (capillary pressure difference)
                        let pc_i = self.get_capillary_pressure(self.grid_cells[id].sat_water);
                        let pc_j = self.get_capillary_pressure(self.grid_cells[nid].sat_water);
                        
                        // Pressure gradient including capillary pressure effects
                        // Effective pressure difference = pressure difference + capillary pressure gradient
                        let dp_total = (p_i - p_j) + (pc_i - pc_j);
                        
                        // Volumetric flux [m³/day]: positive = from id -> nid
                        let flux_m3_per_day = t * dp_total;
                        
                        // Upwind fractional flow for water
                        let f_w = if flux_m3_per_day >= 0.0 {
                            self.frac_flow_water(&self.grid_cells[id])
                        } else {
                            self.frac_flow_water(&self.grid_cells[nid])
                        };
                        // Water flux [m³/day]
                        let water_flux_m3_day = flux_m3_per_day * f_w;
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
                    // Water fractional flow at block condition
                    let fw = self.frac_flow_water(&self.grid_cells[id]);
                    let water_q_m3_day = q_m3_day * fw;
                    
                    // Volume change [m³]. Production (q>0) removes fluid from block.
                    delta_water_m3[id] -= water_q_m3_day * dt_days;
                }
            }
            // Skip malformed well parameters (shouldn't happen with validation)
        }

        // Update saturations based on water volume changes
        for idx in 0..n_cells {
            let vp_m3 = self.grid_cells[idx].pore_volume_m3(self.dx, self.dy, self.dz);
            if vp_m3 <= 0.0 { continue; }

            // Change in water saturation [dimensionless] = ΔV_water [m³] / V_pore [m³]
            let delta_sw = delta_water_m3[idx] / vp_m3;
            let sw_new = (self.grid_cells[idx].sat_water + delta_sw).clamp(0.0, 1.0);
            
            // Ensure material balance: s_w + s_o = 1.0 (two-phase system, no gas phase)
            let so_new = (1.0 - sw_new).clamp(0.0, 1.0);

            // Update state variables
            self.grid_cells[idx].sat_water = sw_new;
            self.grid_cells[idx].sat_oil = so_new;
            self.grid_cells[idx].pressure = p_new[idx];
        }

        // Advance simulation time [s]
        self.time_days += dt_days;
    }

    pub fn get_time(&self) -> f64 { self.time_days }

    #[wasm_bindgen(js_name = getGridState)]
    pub fn get_grid_state(&self) -> JsValue { serde_wasm_bindgen::to_value(&self.grid_cells).unwrap() }

    #[wasm_bindgen(js_name = getWellState)]
    pub fn get_well_state(&self) -> JsValue { serde_wasm_bindgen::to_value(&self.wells).unwrap() }

    #[wasm_bindgen(js_name = getDimensions)]
    pub fn get_dimensions(&self) -> JsValue { serde_wasm_bindgen::to_value(&[self.nx, self.ny, self.nz]).unwrap() }
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