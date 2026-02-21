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

use rand::rngs::StdRng;
use rand::RngExt;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::f64;
use wasm_bindgen::prelude::*;

mod capillary;
mod grid;
mod relperm;
mod solver;
mod step;
mod well;

pub use capillary::CapillaryPressure;
pub use grid::GridCell;
pub use relperm::RockFluidProps;
pub use well::{TimePointRates, Well, WellRates};

// Utility to log panics to the browser console
#[wasm_bindgen(start)]
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
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
            mu_o: 1.0,     // cP (typical oil)
            mu_w: 0.5,     // cP (water at reservoir conditions)
            c_o: 1e-5,     // 1/bar (oil compressibility)
            c_w: 3e-6,     // 1/bar (water compressibility)
            rho_o: 800.0,  // kg/m³
            rho_w: 1000.0, // kg/m³
        }
    }
}

// --- Simulator ---
#[wasm_bindgen]
pub struct ReservoirSimulator {
    nx: usize,
    ny: usize,
    nz: usize,
    dx: f64,
    dy: f64,
    dz: f64,
    grid_cells: Vec<GridCell>,
    wells: Vec<Well>,
    time_days: f64,
    pvt: FluidProperties,
    scal: RockFluidProps,
    pc: CapillaryPressure,
    gravity_enabled: bool,
    max_sat_change_per_step: f64,
    max_pressure_change_per_step: f64,
    max_well_rate_change_fraction: f64,
    rate_controlled_wells: bool,
    injector_rate_controlled: bool,
    producer_rate_controlled: bool,
    injector_enabled: bool,
    target_injector_rate_m3_day: f64,
    /// Minimum BHP constraint [bar] when running in rate mode
    well_bhp_min: f64,
    /// Maximum BHP constraint [bar] when running in rate mode
    well_bhp_max: f64,
    /// Last solver warning message (empty if converged)
    last_solver_warning: String,
    /// Cumulative water injected in reservoir conditions [m³] for material balance
    cumulative_injection_m3: f64,
    /// Cumulative water produced in reservoir conditions [m³] for material balance
    cumulative_production_m3: f64,
    target_producer_rate_m3_day: f64,
    rock_compressibility: f64,
    depth_reference_m: f64,
    b_o: f64,
    b_w: f64,
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
            nx,
            ny,
            nz,
            dx: 10.0, // meters (x-direction cell size)
            dy: 10.0, // meters (y-direction cell size)
            dz: 1.0,  // meters (z-direction cell size)
            grid_cells,
            wells: Vec::new(),
            time_days: 0.0, // simulation time in days
            pvt: FluidProperties::default_pvt(),
            scal: RockFluidProps::default_scal(),
            pc: CapillaryPressure::default_pc(), // Brooks-Corey capillary pressure
            gravity_enabled: false,
            max_sat_change_per_step: 0.1, // Default max saturation change
            max_pressure_change_per_step: 75.0,
            max_well_rate_change_fraction: 0.75,
            rate_controlled_wells: false,
            injector_rate_controlled: false,
            producer_rate_controlled: false,
            injector_enabled: true,
            target_injector_rate_m3_day: 0.0,
            target_producer_rate_m3_day: 0.0,
            well_bhp_min: -100.0,
            well_bhp_max: 2000.0,
            rock_compressibility: 0.0,
            depth_reference_m: 0.0,
            b_o: 1.0,
            b_w: 1.0,
            rate_history: Vec::new(),
            last_solver_warning: String::new(),
            cumulative_injection_m3: 0.0,
            cumulative_production_m3: 0.0,
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
    pub fn add_well(
        &mut self,
        i: usize,
        j: usize,
        k: usize,
        bhp: f64,
        well_radius: f64,
        skin: f64,
        injector: bool,
    ) -> Result<(), String> {
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
            return Err(format!(
                "Well radius must be positive and finite, got: {}",
                well_radius
            ));
        }

        if !skin.is_finite() {
            return Err(format!("Skin factor must be finite, got: {}", skin));
        }

        let cell_id = self.idx(i, j, k);
        let cell = self.grid_cells[cell_id];

        let pi = self.calculate_well_productivity_index(&cell, well_radius, skin)?;

        let well = Well {
            i,
            j,
            k,
            bhp,
            productivity_index: pi,
            injector,
            well_radius,
            skin,
        };

        // Validate well parameters
        well.validate(self.nx, self.ny, self.nz)?;

        self.wells.push(well);
        Ok(())
    }

    /// Set stability parameters for the simulation
    #[wasm_bindgen(js_name = setStabilityParams)]
    pub fn set_stability_params(
        &mut self,
        max_sat_change_per_step: f64,
        max_pressure_change_per_step: f64,
        max_well_rate_change_fraction: f64,
    ) {
        self.max_sat_change_per_step = max_sat_change_per_step.clamp(0.01, 1.0);
        self.max_pressure_change_per_step = max_pressure_change_per_step.clamp(1.0, 2_000.0);
        self.max_well_rate_change_fraction = max_well_rate_change_fraction.clamp(0.01, 5.0);
    }

    /// Advance simulator by target timestep [days]
    pub fn step(&mut self, target_dt_days: f64) {
        self.step_internal(target_dt_days);
    }

    #[wasm_bindgen(js_name = setGravityEnabled)]
    pub fn set_gravity_enabled(&mut self, enabled: bool) {
        self.gravity_enabled = enabled;
    }

    #[wasm_bindgen(js_name = setRateControlledWells)]
    pub fn set_rate_controlled_wells(&mut self, enabled: bool) {
        self.rate_controlled_wells = enabled;
        self.injector_rate_controlled = enabled;
        self.producer_rate_controlled = enabled;
    }

    #[wasm_bindgen(js_name = setWellControlModes)]
    pub fn set_well_control_modes(&mut self, injector_mode: String, producer_mode: String) {
        let inj_mode = injector_mode.to_ascii_lowercase();
        let prod_mode = producer_mode.to_ascii_lowercase();

        self.injector_rate_controlled = inj_mode == "rate";
        self.producer_rate_controlled = prod_mode == "rate";
        self.rate_controlled_wells = self.injector_rate_controlled && self.producer_rate_controlled;
    }

    #[wasm_bindgen(js_name = setInjectorEnabled)]
    pub fn set_injector_enabled(&mut self, enabled: bool) {
        self.injector_enabled = enabled;
    }

    #[wasm_bindgen(js_name = setTargetWellRates)]
    pub fn set_target_well_rates(
        &mut self,
        injector_rate_m3_day: f64,
        producer_rate_m3_day: f64,
    ) -> Result<(), String> {
        if !injector_rate_m3_day.is_finite() || !producer_rate_m3_day.is_finite() {
            return Err("Target well rates must be finite numbers".to_string());
        }
        if injector_rate_m3_day < 0.0 || producer_rate_m3_day < 0.0 {
            return Err(format!(
                "Target well rates must be non-negative, got injector={}, producer={}",
                injector_rate_m3_day, producer_rate_m3_day
            ));
        }

        self.target_injector_rate_m3_day = injector_rate_m3_day;
        self.target_producer_rate_m3_day = producer_rate_m3_day;
        Ok(())
    }

    #[wasm_bindgen(js_name = setWellBhpLimits)]
    pub fn set_well_bhp_limits(&mut self, bhp_min: f64, bhp_max: f64) -> Result<(), String> {
        if !bhp_min.is_finite() || !bhp_max.is_finite() {
            return Err("Well BHP limits must be finite numbers".to_string());
        }
        if bhp_min > bhp_max {
            return Err(format!(
                "Invalid BHP limits: bhp_min ({}) must be <= bhp_max ({})",
                bhp_min, bhp_max
            ));
        }

        self.well_bhp_min = bhp_min;
        self.well_bhp_max = bhp_max;
        Ok(())
    }

    pub fn get_time(&self) -> f64 {
        self.time_days
    }

    #[wasm_bindgen(js_name = getGridState)]
    pub fn get_grid_state(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.grid_cells).unwrap()
    }

    #[wasm_bindgen(js_name = getWellState)]
    pub fn get_well_state(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.wells).unwrap()
    }

    #[wasm_bindgen(js_name = getRateHistory)]
    pub fn get_rate_history(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.rate_history).unwrap()
    }

    /// Get last solver warning message (empty string if no warning)
    #[wasm_bindgen(js_name = getLastSolverWarning)]
    pub fn get_last_solver_warning(&self) -> String {
        self.last_solver_warning.clone()
    }

    #[wasm_bindgen(js_name = getDimensions)]
    pub fn get_dimensions(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&[self.nx, self.ny, self.nz]).unwrap()
    }

    /// Set initial pressure for all grid cells
    #[wasm_bindgen(js_name = setInitialPressure)]
    pub fn set_initial_pressure(&mut self, pressure: f64) {
        for cell in self.grid_cells.iter_mut() {
            cell.pressure = pressure;
        }
    }

    #[wasm_bindgen(js_name = setCellDimensions)]
    pub fn set_cell_dimensions(&mut self, dx: f64, dy: f64, dz: f64) -> Result<(), String> {
        if !dx.is_finite() || !dy.is_finite() || !dz.is_finite() {
            return Err("Cell dimensions must be finite numbers".to_string());
        }
        if dx <= 0.0 || dy <= 0.0 || dz <= 0.0 {
            return Err(format!(
                "Cell dimensions must be positive, got dx={}, dy={}, dz={}",
                dx, dy, dz
            ));
        }

        self.dx = dx;
        self.dy = dy;
        self.dz = dz;
        Ok(())
    }

    /// Set initial water saturation for all grid cells
    #[wasm_bindgen(js_name = setInitialSaturation)]
    pub fn set_initial_saturation(&mut self, sat_water: f64) {
        for cell in self.grid_cells.iter_mut() {
            cell.sat_water = sat_water.clamp(0.0, 1.0);
            cell.sat_oil = 1.0 - cell.sat_water;
        }
    }

    /// Set initial water saturation per z-layer
    #[wasm_bindgen(js_name = setInitialSaturationPerLayer)]
    pub fn set_initial_saturation_per_layer(&mut self, sw: Vec<f64>) -> Result<(), String> {
        if sw.len() != self.nz {
            return Err(format!(
                "Initial saturation vector must have length equal to nz ({})",
                self.nz
            ));
        }

        for (k, sat) in sw.iter().enumerate() {
            if !sat.is_finite() {
                return Err(format!(
                    "Initial saturation for layer {} must be finite, got {}",
                    k, sat
                ));
            }
            if *sat < 0.0 || *sat > 1.0 {
                return Err(format!(
                    "Initial saturation for layer {} must be within [0, 1], got {}",
                    k, sat
                ));
            }
        }

        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i, j, k);
                    self.grid_cells[id].sat_water = sw[k];
                    self.grid_cells[id].sat_oil = 1.0 - sw[k];
                }
            }
        }

        Ok(())
    }

    /// Set relative permeability properties
    #[wasm_bindgen(js_name = setRelPermProps)]
    pub fn set_rel_perm_props(
        &mut self,
        s_wc: f64,
        s_or: f64,
        n_w: f64,
        n_o: f64,
    ) -> Result<(), String> {
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

        self.scal = RockFluidProps {
            s_wc,
            s_or,
            n_w,
            n_o,
        };
        Ok(())
    }

    #[wasm_bindgen(js_name = setFluidDensities)]
    pub fn set_fluid_densities(&mut self, rho_o: f64, rho_w: f64) -> Result<(), String> {
        if !rho_o.is_finite() || !rho_w.is_finite() {
            return Err("Fluid densities must be finite numbers".to_string());
        }
        if rho_o <= 0.0 || rho_w <= 0.0 {
            return Err(format!(
                "Fluid densities must be positive, got rho_o={}, rho_w={}",
                rho_o, rho_w
            ));
        }
        self.pvt.rho_o = rho_o;
        self.pvt.rho_w = rho_w;
        Ok(())
    }

    #[wasm_bindgen(js_name = setFluidProperties)]
    pub fn set_fluid_properties(&mut self, mu_o: f64, mu_w: f64) -> Result<(), String> {
        if !mu_o.is_finite() || !mu_w.is_finite() {
            return Err("Fluid viscosities must be finite numbers".to_string());
        }
        if mu_o <= 0.0 || mu_w <= 0.0 {
            return Err(format!(
                "Fluid viscosities must be positive, got mu_o={}, mu_w={}",
                mu_o, mu_w
            ));
        }

        self.pvt.mu_o = mu_o;
        self.pvt.mu_w = mu_w;
        Ok(())
    }

    #[wasm_bindgen(js_name = setFluidCompressibilities)]
    pub fn set_fluid_compressibilities(&mut self, c_o: f64, c_w: f64) -> Result<(), String> {
        if !c_o.is_finite() || !c_w.is_finite() {
            return Err("Fluid compressibilities must be finite numbers".to_string());
        }
        if c_o < 0.0 || c_w < 0.0 {
            return Err(format!(
                "Fluid compressibilities must be non-negative, got c_o={}, c_w={}",
                c_o, c_w
            ));
        }

        self.pvt.c_o = c_o;
        self.pvt.c_w = c_w;
        Ok(())
    }

    #[wasm_bindgen(js_name = setRockProperties)]
    pub fn set_rock_properties(
        &mut self,
        c_r: f64,
        depth_reference_m: f64,
        b_o: f64,
        b_w: f64,
    ) -> Result<(), String> {
        if !c_r.is_finite()
            || !depth_reference_m.is_finite()
            || !b_o.is_finite()
            || !b_w.is_finite()
        {
            return Err("Rock properties must be finite numbers".to_string());
        }
        if c_r < 0.0 {
            return Err(format!(
                "Rock compressibility must be non-negative, got {}",
                c_r
            ));
        }
        if b_o <= 0.0 || b_w <= 0.0 {
            return Err(format!(
                "Volume expansion factors must be positive, got b_o={}, b_w={}",
                b_o, b_w
            ));
        }

        self.rock_compressibility = c_r;
        self.depth_reference_m = depth_reference_m;
        self.b_o = b_o;
        self.b_w = b_w;
        Ok(())
    }

    #[wasm_bindgen(js_name = setCapillaryParams)]
    pub fn set_capillary_params(&mut self, p_entry: f64, lambda: f64) -> Result<(), String> {
        if !p_entry.is_finite() || !lambda.is_finite() {
            return Err("Capillary parameters must be finite numbers".to_string());
        }
        if p_entry < 0.0 {
            return Err(format!(
                "Capillary entry pressure must be non-negative, got {}",
                p_entry
            ));
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
    pub fn set_permeability_random_seeded(
        &mut self,
        min_perm: f64,
        max_perm: f64,
        seed: u64,
    ) -> Result<(), String> {
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

    /// Load the entire state to continue simulation without re-computing from step 0
    #[wasm_bindgen(js_name = loadState)]
    pub fn load_state(
        &mut self,
        time_days: f64,
        grid_state: JsValue,
        well_state: JsValue,
        rate_history: JsValue,
    ) -> Result<(), JsValue> {
        let grid_cells: Vec<GridCell> = serde_wasm_bindgen::from_value(grid_state)?;
        let wells: Vec<Well> = serde_wasm_bindgen::from_value(well_state)?;
        let rate_history_vec: Vec<TimePointRates> = serde_wasm_bindgen::from_value(rate_history)?;
        
        let expected_cells = self.nx * self.ny * self.nz;
        if grid_cells.len() != expected_cells {
            return Err(JsValue::from_str(&format!(
                "Mismatch grid size. Expected {}, got {}",
                expected_cells,
                grid_cells.len()
            )));
        }

        self.time_days = time_days;
        self.grid_cells = grid_cells;
        self.wells = wells;
        self.rate_history = rate_history_vec;
        
        if let Some(last) = self.rate_history.last() {
            self.cumulative_injection_m3 = last.total_injection_reservoir;
            self.cumulative_production_m3 = last.total_production_liquid_reservoir;
        }

        Ok(())
    }

    /// Set permeability per layer
    #[wasm_bindgen(js_name = setPermeabilityPerLayer)]
    pub fn set_permeability_per_layer(
        &mut self,
        perms_x: Vec<f64>,
        perms_y: Vec<f64>,
        perms_z: Vec<f64>,
    ) -> Result<(), String> {
        if perms_x.len() != self.nz || perms_y.len() != self.nz || perms_z.len() != self.nz {
            return Err(format!(
                "Permeability vectors must have length equal to nz ({})",
                self.nz
            ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::WellControlDecision;

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
            let fw = corey_fractional_flow(
                s, case.s_wc, case.s_or, case.n_w, case.n_o, case.mu_w, case.mu_o,
            );
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
        sim.set_rel_perm_props(case.s_wc, case.s_or, case.n_w, case.n_o)
            .unwrap();
        sim.set_initial_saturation(case.s_wc);
        sim.set_permeability_random_seeded(case.permeability_md, case.permeability_md, 42)
            .unwrap();
        sim.set_stability_params(0.05, 75.0, 0.75);
        sim.pc.p_entry = 0.0;
        sim.pvt.mu_w = case.mu_w;
        sim.pvt.mu_o = case.mu_o;

        sim.add_well(0, 0, 0, case.injector_bhp, 0.1, 0.0, true)
            .unwrap();
        sim.add_well(case.nx - 1, 0, 0, case.producer_bhp, 0.1, 0.0, false)
            .unwrap();

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
            let point = sim
                .rate_history
                .last()
                .expect("rate history should have entries");
            let dt = point.time - previous_time;
            previous_time = point.time;

            cumulative_injection += point.total_injection.max(0.0) * dt;

            if point.total_production_liquid > 1e-9 {
                let water_rate =
                    (point.total_production_liquid - point.total_production_oil).max(0.0);
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
        sim.set_stability_params(0.01, 75.0, 0.75);
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
        sim_loose.set_stability_params(1.0, 75.0, 0.75);
        sim_loose
            .set_permeability_random(20_000.0, 20_000.0)
            .unwrap();
        sim_loose.add_well(0, 0, 0, 650.0, 0.1, 0.0, true).unwrap();
        sim_loose.add_well(2, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        sim_loose.step(5.0);

        let mut sim_tight = ReservoirSimulator::new(3, 1, 1);
        sim_tight.set_stability_params(0.01, 75.0, 0.75);
        sim_tight
            .set_permeability_random(20_000.0, 20_000.0)
            .unwrap();
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
        err_contains(
            sim.set_rel_perm_props(0.1, 0.1, 0.0, 2.0),
            "must be positive",
        );
        err_contains(
            sim.set_rel_perm_props(f64::NAN, 0.1, 2.0, 2.0),
            "finite numbers",
        );
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
        sim_no_g
            .set_permeability_random_seeded(50_000.0, 50_000.0, 42)
            .unwrap();
        sim_no_g.set_initial_pressure(300.0);
        sim_no_g.set_initial_saturation(0.9);
        sim_no_g.pc.p_entry = 0.0;
        sim_no_g.set_gravity_enabled(false);
        sim_no_g.step(2.0);

        let p_top_no_g = sim_no_g.grid_cells[sim_no_g.idx(0, 0, 0)].pressure;
        let p_bot_no_g = sim_no_g.grid_cells[sim_no_g.idx(0, 0, 1)].pressure;

        let mut sim_g = ReservoirSimulator::new(1, 1, 2);
        sim_g
            .set_permeability_random_seeded(50_000.0, 50_000.0, 42)
            .unwrap();
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
        sim_g
            .set_permeability_random_seeded(80_000.0, 80_000.0, 7)
            .unwrap();
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
        sim_no_g
            .set_permeability_random_seeded(80_000.0, 80_000.0, 7)
            .unwrap();
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
        err_contains(
            sim.set_permeability_random(200.0, 50.0),
            "cannot exceed max",
        );
        err_contains(
            sim.set_permeability_random_seeded(-1.0, 100.0, 123),
            "must be positive",
        );
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
            case.name, metrics.breakthrough_pv, metrics.reference_breakthrough_pv, rel_err
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
            case.name, metrics.breakthrough_pv, metrics.reference_breakthrough_pv, rel_err
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
        let rel_err_coarse_a = ((metrics_coarse_a.breakthrough_pv
            - metrics_coarse_a.reference_breakthrough_pv)
            / metrics_coarse_a.reference_breakthrough_pv)
            .abs();
        let rel_err_refined_a = ((metrics_refined_a.breakthrough_pv
            - metrics_refined_a.reference_breakthrough_pv)
            / metrics_refined_a.reference_breakthrough_pv)
            .abs();

        let metrics_coarse_b = run_buckley_case(&coarse_b);
        let metrics_refined_b = run_buckley_case(&refined_b);
        let rel_err_coarse_b = ((metrics_coarse_b.breakthrough_pv
            - metrics_coarse_b.reference_breakthrough_pv)
            / metrics_coarse_b.reference_breakthrough_pv)
            .abs();
        let rel_err_refined_b = ((metrics_refined_b.breakthrough_pv
            - metrics_refined_b.reference_breakthrough_pv)
            / metrics_refined_b.reference_breakthrough_pv)
            .abs();

        println!(
            "Case-A coarse/refined rel_err: {:.3} -> {:.3}",
            rel_err_coarse_a, rel_err_refined_a
        );
        println!(
            "BL-Case-A-Refined: breakthrough_pv_sim={:.4}, breakthrough_pv_ref={:.4}, rel_err={:.3}",
            metrics_refined_a.breakthrough_pv,
            metrics_refined_a.reference_breakthrough_pv,
            rel_err_refined_a
        );
        println!(
            "Case-B coarse/refined rel_err: {:.3} -> {:.3}",
            rel_err_coarse_b, rel_err_refined_b
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

    #[test]
    fn set_initial_saturation_per_layer_applies_uniformly_by_k() {
        let mut sim = ReservoirSimulator::new(2, 2, 3);
        sim.set_initial_saturation_per_layer(vec![0.1, 0.4, 0.8])
            .unwrap();

        for k in 0..sim.nz {
            for j in 0..sim.ny {
                for i in 0..sim.nx {
                    let id = sim.idx(i, j, k);
                    let sw = sim.grid_cells[id].sat_water;
                    assert!((sw - [0.1, 0.4, 0.8][k]).abs() < 1e-12);
                    assert!((sim.grid_cells[id].sat_oil - (1.0 - sw)).abs() < 1e-12);
                }
            }
        }
    }

    #[test]
    fn dynamic_pi_increases_with_higher_water_saturation() {
        let mut sim = ReservoirSimulator::new(1, 1, 1);
        sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0).unwrap();
        sim.set_fluid_properties(3.0, 0.5).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let id = sim.idx(0, 0, 0);
        sim.grid_cells[id].sat_water = sim.scal.s_wc;
        sim.grid_cells[id].sat_oil = 1.0 - sim.scal.s_wc;
        sim.update_dynamic_well_productivity_indices();
        let pi_low_sw = sim.wells[0].productivity_index;

        let sw_high = 0.95 - sim.scal.s_or;
        sim.grid_cells[id].sat_water = sw_high;
        sim.grid_cells[id].sat_oil = 1.0 - sw_high;
        sim.update_dynamic_well_productivity_indices();
        let pi_high_sw = sim.wells[0].productivity_index;

        assert!(pi_high_sw > pi_low_sw);
    }

    #[test]
    fn rate_control_switches_to_bhp_when_limits_are_hit() {
        let mut sim = ReservoirSimulator::new(1, 1, 1);
        sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
        sim.set_target_well_rates(0.0, 500.0).unwrap();
        sim.set_well_bhp_limits(80.0, 120.0).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let well = &sim.wells[0];
        let pressure = 100.0;

        let control = sim
            .resolve_well_control(well, pressure)
            .expect("control decision should be available");

        match control {
            WellControlDecision::Bhp { bhp_bar } => {
                assert!((bhp_bar - 80.0).abs() < 1e-9);
            }
            _ => panic!("Expected BHP fallback control when target rate violates BHP limits"),
        }

        let q = sim.well_rate_m3_day(well, pressure).unwrap();
        let expected_q = well.productivity_index * (pressure - 80.0);
        assert!((q - expected_q).abs() < 1e-9);
        assert!(q < 500.0);
    }
}
