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
// - Transmissibility / PI use a metric Darcy factor that converts mD·m²/(m·cP) to m³/day/bar
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
mod pvt;
mod solver;
mod step;
mod well;

pub use capillary::{CapillaryPressure, GasOilCapillaryPressure};
pub use relperm::{RockFluidProps, RockFluidPropsThreePhase, SgofRow, SwofRow, ThreePhaseScalTables};
pub use well::{TimePointRates, Well, WellRates};

/// Which fluid the injector injects in three-phase mode.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum InjectedFluid {
    Water,
    Gas,
}

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
    /// Cell thickness per layer [m] (length = nz)
    dz: Vec<f64>,
    porosity: Vec<f64>,
    perm_x: Vec<f64>,
    perm_y: Vec<f64>,
    perm_z: Vec<f64>,
    pressure: Vec<f64>,
    sat_water: Vec<f64>,
    sat_oil: Vec<f64>,
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
    target_injector_surface_rate_m3_day: Option<f64>,
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
    /// Cumulative water material balance error [m³]
    pub cumulative_mb_error_m3: f64,
    /// Cumulative gas material balance error [Sm³] for total gas inventory
    /// (free gas + dissolved gas) in three-phase mode.
    pub cumulative_mb_gas_error_m3: f64,
    target_producer_rate_m3_day: f64,
    target_producer_surface_rate_m3_day: Option<f64>,
    rock_compressibility: f64,
    depth_reference_m: f64,
    b_o: f64,
    b_w: f64,
    rate_history: Vec<TimePointRates>,

    // ── Three-phase additions (inactive when three_phase_mode = false) ────────
    /// Gas saturation per cell (all zeros in 2-phase mode)
    pub(crate) sat_gas: Vec<f64>,
    /// Three-phase SCAL (Stone II). None while running in 2-phase mode.
    pub(crate) scal_3p: Option<RockFluidPropsThreePhase>,
    /// Gas-oil capillary pressure curve. None while disabled or 2-phase mode.
    pub(crate) pc_og: Option<GasOilCapillaryPressure>,
    /// Enable three-phase equations in step.rs
    pub(crate) three_phase_mode: bool,
    /// Which fluid is injected (only relevant when three_phase_mode = true)
    pub(crate) injected_fluid: InjectedFluid,
    /// Gas viscosity [cP]
    pub(crate) mu_g: f64,
    /// Gas compressibility [1/bar]
    pub(crate) c_g: f64,
    /// Gas density [kg/m³]
    pub(crate) rho_g: f64,
    pub(crate) pvt_table: Option<pvt::PvtTable>,
    /// Dissolved gas ratio at standard conditions [m³ gas / m³ oil] per cell
    pub(crate) rs: Vec<f64>,
    /// Whether liberated free gas may dissolve back into oil.
    pub(crate) gas_redissolution_enabled: bool,
}

#[wasm_bindgen]
impl ReservoirSimulator {

    /// Oil viscosity [cP].  Uses undersaturated correction when cell Rs is tracked.
    pub(crate) fn get_mu_o(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table { table.interpolate(p).mu_o_cp } else { self.pvt.mu_o }
    }
    /// Oil viscosity accounting for undersaturation at the given cell.
    pub(crate) fn get_mu_o_cell(&self, id: usize, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                let (_, mu) = table.interpolate_oil(p, self.rs[id]);
                return mu;
            }
            table.interpolate(p).mu_o_cp
        } else {
            self.pvt.mu_o
        }
    }
    pub(crate) fn get_mu_w(&self, _p: f64) -> f64 {
        self.pvt.mu_w // Water viscosity is constant for now
    }
    pub(crate) fn get_mu_g(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table { table.interpolate(p).mu_g_cp } else { self.mu_g }
    }
    pub(crate) fn get_c_o(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let dp = 1.0;
            let p_minus = if p > dp { p - dp } else { 0.0 };
            let b1 = table.interpolate(p_minus).bo_m3m3;
            let b2 = table.interpolate(p + dp).bo_m3m3;
            let bo = table.interpolate(p).bo_m3m3;
            if bo > 1e-12 {
                let derived_c_o = (-1.0 / bo) * (b2 - b1) / (2.0 * dp);
                if derived_c_o.is_finite() && derived_c_o > 0.0 {
                    derived_c_o.max(self.pvt.c_o)
                } else {
                    self.pvt.c_o
                }
            } else {
                self.pvt.c_o
            }
        } else {
            self.pvt.c_o
        }
    }
    pub(crate) fn get_c_g(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let dp = 1.0;
            let p_minus = if p > dp { p - dp } else { 0.0 };
            let b1 = table.interpolate(p_minus).bg_m3m3;
            let b2 = table.interpolate(p + dp).bg_m3m3;
            let bg = table.interpolate(p).bg_m3m3;
            if bg > 1e-12 { (-1.0 / bg) * (b2 - b1) / (2.0 * dp) } else { self.c_g }
        } else {
            self.c_g
        }
    }
    /// Effective oil compressibility for IMPES accumulation [1/bar].
    ///
    /// For black-oil with dissolved gas (Aziz & Settari, Eq. 7.60):
    ///   c_o_eff = -(1/Bo) dBo/dp  +  (Bg/Bo) dRs/dp
    ///
    /// The dissolved-gas term is only active when the oil is *saturated*
    /// (cell Rs ≈ Rs_sat at the current pressure).  When the oil is
    /// undersaturated (Rs < Rs_sat), dRs/dp = 0 and we use the constant
    /// undersaturated compressibility c_o.
    pub(crate) fn get_c_o_effective(&self, p: f64, rs_cell: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            // Check if the cell is undersaturated
            let rs_sat = table.interpolate(p).rs_m3m3;
            if rs_cell < rs_sat - 1e-6 {
                // Undersaturated: no gas liberation, standard c_o only
                return self.pvt.c_o;
            }

            let dp = 1.0; // 1 bar finite-difference step
            let p_lo = (p - dp).max(0.0);
            let row_lo = table.interpolate(p_lo);
            let row_hi = table.interpolate(p + dp);
            let row_mid = table.interpolate(p);

            let bo = row_mid.bo_m3m3;
            let bg = row_mid.bg_m3m3;
            if bo > 1e-12 {
                // Standard oil compressibility: -(1/Bo) dBo/dp
                let dbo_dp = (row_hi.bo_m3m3 - row_lo.bo_m3m3) / (2.0 * dp);
                let c_o = -dbo_dp / bo;

                // Dissolved gas contribution: (Bg/Bo) dRs/dp
                let drs_dp = (row_hi.rs_m3m3 - row_lo.rs_m3m3) / (2.0 * dp);
                let c_dg = if bg > 0.0 { (bg / bo) * drs_dp } else { 0.0 };

                let c_eff = c_o + c_dg;
                if c_eff.is_finite() && c_eff > 0.0 {
                    return c_eff;
                }
            }
            // Fallback: undersaturated constant
            self.pvt.c_o
        } else {
            self.pvt.c_o
        }
    }

    /// Oil formation volume factor accounting for undersaturation at the given cell.
    pub(crate) fn get_b_o_cell(&self, id: usize, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                let (bo, _) = table.interpolate_oil(p, self.rs[id]);
                return bo;
            }
            table.interpolate(p).bo_m3m3
        } else {
            self.b_o
        }
    }

    /// Oil density at reservoir conditions [kg/m³].
    ///
    /// Uses the cell's actual Rs (not the saturated-curve Rs) so that
    /// undersaturated oil gets the correct density:
    ///   ρ_o(p, Rs) = (ρ_o_sc + Rs·ρ_g_sc) / Bo(p, Rs)
    pub(crate) fn get_rho_o_cell(&self, id: usize, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let rs = self.rs[id];
            let (bo, _) = table.interpolate_oil(p, rs);
            (self.pvt.rho_o + rs * self.rho_g) / bo
        } else {
            self.pvt.rho_o
        }
    }

    /// Oil density at reservoir conditions [kg/m³] (pressure-only variant).
    ///
    /// Accounts for dissolved gas mass:  ρ_o(p) = (ρ_o_sc + Rs(p)·ρ_g_sc) / Bo(p)
    pub(crate) fn get_rho_o(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            let row = table.interpolate(p);
            (self.pvt.rho_o + row.rs_m3m3 * self.rho_g) / row.bo_m3m3
        } else {
            self.pvt.rho_o
        }
    }
    pub(crate) fn get_rho_g(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            self.rho_g / table.interpolate(p).bg_m3m3
        } else {
            self.rho_g
        }
    }
    pub(crate) fn get_rho_w(&self, _p: f64) -> f64 {
        self.pvt.rho_w
    }

    pub(crate) fn get_b_g(&self, p: f64) -> f64 {
        if let Some(table) = &self.pvt_table {
            table.interpolate(p).bg_m3m3
        } else {
            1.0
        }
    }

    /// Get the z-direction cell thickness for a given cell index.
    pub(crate) fn dz_at(&self, id: usize) -> f64 {
        let k = id / (self.nx * self.ny);
        self.dz[k]
    }

    /// Pore-volume-weighted average reservoir pressure [bar].
    pub(crate) fn average_reservoir_pressure_pv_weighted(&self) -> f64 {
        let mut weighted_pressure_sum = 0.0;
        let mut pore_volume_sum = 0.0;

        for id in 0..self.nx * self.ny * self.nz {
            let pore_volume = self.pore_volume_m3(id);
            if pore_volume <= 0.0 || !pore_volume.is_finite() {
                continue;
            }
            weighted_pressure_sum += self.pressure[id] * pore_volume;
            pore_volume_sum += pore_volume;
        }

        if pore_volume_sum > 0.0 {
            weighted_pressure_sum / pore_volume_sum
        } else {
            0.0
        }
    }

    /// Create a new reservoir simulator with oil-field units
    /// Grid dimensions: nx, ny, nz (number of cells in each direction)
    /// All parameters use: Pressure [bar], Distance [m], Time [day], Permeability [mD], Viscosity [cP]
    #[wasm_bindgen(constructor)]
    pub fn new(nx: usize, ny: usize, nz: usize, porosity_val: f64) -> Self {
        let n = nx * ny * nz;
        let porosity = vec![porosity_val; n];
        let perm_x = vec![100.0; n];
        let perm_y = vec![100.0; n];
        let perm_z = vec![10.0; n];
        let pressure = vec![300.0; n];
        let sat_water = vec![0.3; n];
        let sat_oil = vec![0.7; n];
        let sat_gas = vec![0.0; n];
        let rs = vec![0.0; n];
        ReservoirSimulator {
            nx,
            ny,
            nz,
            dx: 10.0,           // meters (x-direction cell size)
            dy: 10.0,           // meters (y-direction cell size)
            dz: vec![1.0; nz],  // meters (z-direction cell size per layer)
            porosity,
            perm_x,
            perm_y,
            perm_z,
            pressure,
            sat_water,
            sat_oil,
            sat_gas,
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
            target_injector_surface_rate_m3_day: None,
            target_producer_rate_m3_day: 0.0,
            target_producer_surface_rate_m3_day: None,
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
            cumulative_mb_error_m3: 0.0,
            cumulative_mb_gas_error_m3: 0.0,
            // Three-phase (disabled by default)
            scal_3p: None,
            pc_og: None,
            three_phase_mode: false,
            injected_fluid: InjectedFluid::Gas,
            mu_g: 0.02,
            c_g: 1e-4,
            rho_g: 10.0,
            pvt_table: None,
            rs,
            gas_redissolution_enabled: true,
        }
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

        let pi = self.calculate_well_productivity_index(cell_id, well_radius, skin)?;

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

    #[wasm_bindgen(js_name = setTargetWellSurfaceRates)]
    pub fn set_target_well_surface_rates(
        &mut self,
        injector_rate_m3_day: f64,
        producer_rate_m3_day: f64,
    ) -> Result<(), String> {
        if !injector_rate_m3_day.is_finite() || !producer_rate_m3_day.is_finite() {
            return Err("Target well surface rates must be finite numbers".to_string());
        }
        if injector_rate_m3_day < 0.0 || producer_rate_m3_day < 0.0 {
            return Err(format!(
                "Target well surface rates must be non-negative, got injector={}, producer={}",
                injector_rate_m3_day, producer_rate_m3_day
            ));
        }

        self.target_injector_surface_rate_m3_day = if injector_rate_m3_day > 0.0 {
            Some(injector_rate_m3_day)
        } else {
            None
        };
        self.target_producer_surface_rate_m3_day = if producer_rate_m3_day > 0.0 {
            Some(producer_rate_m3_day)
        } else {
            None
        };
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

    #[wasm_bindgen(js_name = getPressures)]
    pub fn get_pressures(&self) -> Vec<f64> {
        self.pressure.clone()
    }

    #[wasm_bindgen(js_name = getSatWater)]
    pub fn get_sat_water(&self) -> Vec<f64> {
        self.sat_water.clone()
    }

    #[wasm_bindgen(js_name = getSatOil)]
    pub fn get_sat_oil(&self) -> Vec<f64> {
        self.sat_oil.clone()
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
        for i in 0..self.nx * self.ny * self.nz {
            self.pressure[i] = pressure;
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
        self.dz = vec![dz; self.nz];
        Ok(())
    }

    /// Set cell dimensions with per-layer thickness in the z-direction.
    /// `dz_per_layer` must have length equal to nz.
    #[wasm_bindgen(js_name = setCellDimensionsPerLayer)]
    pub fn set_cell_dimensions_per_layer(&mut self, dx: f64, dy: f64, dz_per_layer: Vec<f64>) -> Result<(), String> {
        if !dx.is_finite() || !dy.is_finite() {
            return Err("Cell dimensions must be finite numbers".to_string());
        }
        if dx <= 0.0 || dy <= 0.0 {
            return Err(format!(
                "Cell dimensions must be positive, got dx={}, dy={}",
                dx, dy
            ));
        }
        if dz_per_layer.len() != self.nz {
            return Err(format!(
                "dz_per_layer must have length equal to nz ({}), got {}",
                self.nz, dz_per_layer.len()
            ));
        }
        for (k, &dz_k) in dz_per_layer.iter().enumerate() {
            if !dz_k.is_finite() || dz_k <= 0.0 {
                return Err(format!(
                    "dz for layer {} must be positive and finite, got {}",
                    k, dz_k
                ));
            }
        }

        self.dx = dx;
        self.dy = dy;
        self.dz = dz_per_layer;
        Ok(())
    }

    /// Set initial water saturation for all grid cells
    #[wasm_bindgen(js_name = setInitialSaturation)]
    pub fn set_initial_saturation(&mut self, sat_water: f64) {
        for i in 0..self.nx * self.ny * self.nz {
            self.sat_water[i] = sat_water.clamp(0.0, 1.0);
            self.sat_oil[i] = 1.0 - self.sat_water[i];
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
                    self.sat_water[id] = sw[k];
                    self.sat_oil[id] = 1.0 - sw[k];
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
        k_rw_max: f64,
        k_ro_max: f64,
    ) -> Result<(), String> {
        if !s_wc.is_finite()
            || !s_or.is_finite()
            || !n_w.is_finite()
            || !n_o.is_finite()
            || !k_rw_max.is_finite()
            || !k_ro_max.is_finite()
        {
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

        if k_rw_max < 0.0 || k_rw_max > 1.0 {
            return Err(format!("k_rw_max must be in [0, 1], got {}", k_rw_max));
        }

        if k_ro_max <= 0.0 || k_ro_max > 1.0 {
            return Err(format!("k_ro_max must be in (0, 1], got {}", k_ro_max));
        }

        self.scal = RockFluidProps {
            s_wc,
            s_or,
            n_w,
            n_o,
            k_rw_max,
            k_ro_max,
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
        for i in 0..self.nx * self.ny * self.nz {
            self.perm_x[i] = rng.random_range(min_perm..=max_perm);
            self.perm_y[i] = rng.random_range(min_perm..=max_perm);
            self.perm_z[i] = rng.random_range(min_perm..=max_perm) / 10.0; // Anisotropy
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
        for i in 0..self.nx * self.ny * self.nz {
            self.perm_x[i] = rng.random_range(min_perm..=max_perm);
            self.perm_y[i] = rng.random_range(min_perm..=max_perm);
            self.perm_z[i] = rng.random_range(min_perm..=max_perm) / 10.0; // Anisotropy
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
        let wells: Vec<Well> = serde_wasm_bindgen::from_value(well_state)?;
        let rate_history_vec: Vec<TimePointRates> = serde_wasm_bindgen::from_value(rate_history)?;

        #[derive(Deserialize)]
        struct GridStatePayload {
            pressure: Vec<f64>,
            sat_water: Vec<f64>,
            sat_oil: Vec<f64>,
        }
        let grid_data: GridStatePayload = serde_wasm_bindgen::from_value(grid_state)?;

        let expected_cells = self.nx * self.ny * self.nz;
        if grid_data.pressure.len() != expected_cells
            || grid_data.sat_water.len() != expected_cells
            || grid_data.sat_oil.len() != expected_cells
        {
            return Err(JsValue::from_str(&format!(
                "Mismatch grid size. Expected {}, got pressure len: {}, sat_water len: {}, sat_oil len: {}",
                expected_cells, grid_data.pressure.len(), grid_data.sat_water.len(), grid_data.sat_oil.len()
            )));
        }

        self.time_days = time_days;
        self.pressure = grid_data.pressure;
        self.sat_water = grid_data.sat_water;
        self.sat_oil = grid_data.sat_oil;

        self.wells = wells;
        self.rate_history = rate_history_vec;

        if let Some(last) = self.rate_history.last() {
            self.cumulative_injection_m3 = last.total_injection_reservoir;
            self.cumulative_production_m3 = last.total_production_liquid_reservoir;
        }

        Ok(())
    }

    /// Set initial gas saturation for all grid cells (three-phase mode)

    #[wasm_bindgen(js_name = setPvtTable)]
    pub fn set_pvt_table(&mut self, table_js: JsValue) -> Result<(), JsValue> {
        let rows: Vec<pvt::PvtRow> = serde_wasm_bindgen::from_value(table_js)?;
        let table = pvt::PvtTable::new(rows, self.pvt.c_o);
        // Initialize rs array for all cells based on initial pressure
        let n = self.nx * self.ny * self.nz;
        for i in 0..n {
            self.rs[i] = table.interpolate(self.pressure[i]).rs_m3m3;
        }
        self.pvt_table = Some(table);
        Ok(())
    }

    /// Override the dissolved-gas ratio for all cells with a uniform value.
    /// Must be called **after** `setPvtTable` so the Rs array already exists.
    /// This is used when the reservoir starts undersaturated (Rs < Rs_sat at initial P),
    /// e.g. SPE1 Case 1 whose RSVD table specifies a constant Rs below saturation.
    #[wasm_bindgen(js_name = setInitialRs)]
    pub fn set_initial_rs(&mut self, rs: f64) {
        let n = self.nx * self.ny * self.nz;
        for i in 0..n {
            self.rs[i] = rs;
        }
    }

    #[wasm_bindgen(js_name = setInitialGasSaturation)]
    pub fn set_initial_gas_saturation(&mut self, sat_gas: f64) {
        let n = self.nx * self.ny * self.nz;
        let sg = sat_gas.clamp(0.0, 1.0);
        for i in 0..n {
            let sw = self.sat_water[i];
            let sg_clamped = sg.min(1.0 - sw);
            self.sat_gas[i] = sg_clamped;
            self.sat_oil[i] = (1.0 - sw - sg_clamped).max(0.0);
        }
    }

    /// Set initial gas saturation per z-layer (three-phase mode)
    #[wasm_bindgen(js_name = setInitialGasSaturationPerLayer)]
    pub fn set_initial_gas_saturation_per_layer(&mut self, sg: Vec<f64>) -> Result<(), String> {
        if sg.len() != self.nz {
            return Err(format!(
                "Initial gas saturation vector must have length equal to nz ({}), got {}",
                self.nz, sg.len()
            ));
        }

        for (k, sat) in sg.iter().enumerate() {
            if !sat.is_finite() {
                return Err(format!(
                    "Initial gas saturation for layer {} must be finite, got {}",
                    k, sat
                ));
            }
            if *sat < 0.0 || *sat > 1.0 {
                return Err(format!(
                    "Initial gas saturation for layer {} must be within [0, 1], got {}",
                    k, sat
                ));
            }
        }

        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i, j, k);
                    let sw = self.sat_water[id];
                    let sg_clamped = sg[k].min(1.0 - sw);
                    self.sat_gas[id] = sg_clamped;
                    self.sat_oil[id] = (1.0 - sw - sg_clamped).max(0.0);
                }
            }
        }

        Ok(())
    }

    /// Get gas saturation array (zeros when running in 2-phase mode)
    #[wasm_bindgen(js_name = getSatGas)]
    pub fn get_sat_gas(&self) -> Vec<f64> {
        self.sat_gas.clone()
    }

    /// Get dissolved gas ratio array [m3/m3]
    #[wasm_bindgen(js_name = getRs)]
    pub fn get_rs(&self) -> Vec<f64> {
        self.rs.clone()
    }

    /// Enable or disable the three-phase simulation mode
    #[wasm_bindgen(js_name = setThreePhaseModeEnabled)]
    pub fn set_three_phase_mode_enabled(&mut self, enabled: bool) {
        self.three_phase_mode = enabled;
    }

    /// Set three-phase relative permeability parameters (Stone II model).
    /// `s_org` is the residual oil saturation in a gas flood (typically ≥ `s_or`).
    /// It is distinct from `s_gr` (trapped residual gas) and is used as the terminal
    /// oil saturation in `k_ro_gas` and gas-oil capillary pressure.
    #[wasm_bindgen(js_name = setThreePhaseRelPermProps)]
    pub fn set_three_phase_rel_perm_props(
        &mut self,
        s_wc: f64,
        s_or: f64,
        s_gc: f64,
        s_gr: f64,
        s_org: f64,
        n_w: f64,
        n_o: f64,
        n_g: f64,
        k_rw_max: f64,
        k_ro_max: f64,
        k_rg_max: f64,
    ) -> Result<(), String> {
        if s_wc + s_or + s_gc + s_gr >= 1.0 {
            return Err(format!(
                "Invalid saturation endpoints: S_wc + S_or + S_gc + S_gr must be < 1.0, got {}",
                s_wc + s_or + s_gc + s_gr
            ));
        }
        if s_wc + s_org >= 1.0 {
            return Err(format!(
                "Invalid saturation endpoints: S_wc + S_org must be < 1.0, got {}",
                s_wc + s_org
            ));
        }
        if n_w <= 0.0 || n_o <= 0.0 || n_g <= 0.0 {
            return Err(format!(
                "Corey exponents must be positive, got n_w={}, n_o={}, n_g={}",
                n_w, n_o, n_g
            ));
        }
        self.scal_3p = Some(RockFluidPropsThreePhase {
            s_wc, s_or, n_w, n_o, k_rw_max, k_ro_max, s_gc, s_gr, s_org, n_g, k_rg_max, tables: None,
        });
        Ok(())
    }

    #[wasm_bindgen(js_name = setThreePhaseScalTables)]
    pub fn set_three_phase_scal_tables(&mut self, table_js: JsValue) -> Result<(), JsValue> {
        let tables: ThreePhaseScalTables = serde_wasm_bindgen::from_value(table_js)?;
        tables
            .validate()
            .map_err(|message| JsValue::from_str(&message))?;

        let scal = self
            .scal_3p
            .as_mut()
            .ok_or_else(|| JsValue::from_str("Three-phase relperm props must be configured before SWOF/SGOF tables"))?;
        scal.tables = Some(tables);
        Ok(())
    }

    /// Set gas-oil capillary pressure parameters (Brooks-Corey form)
    #[wasm_bindgen(js_name = setGasOilCapillaryParams)]
    pub fn set_gas_oil_capillary_params(&mut self, p_entry: f64, lambda: f64) -> Result<(), String> {
        if p_entry < 0.0 {
            return Err(format!("Gas-oil capillary entry pressure must be non-negative, got {}", p_entry));
        }
        if lambda <= 0.0 {
            return Err(format!("Gas-oil capillary lambda must be positive, got {}", lambda));
        }
        self.pc_og = Some(GasOilCapillaryPressure { p_entry, lambda });
        Ok(())
    }

    /// Set gas fluid properties for three-phase mode
    #[wasm_bindgen(js_name = setGasFluidProperties)]
    pub fn set_gas_fluid_properties(&mut self, mu_g: f64, c_g: f64, rho_g: f64) -> Result<(), String> {
        if mu_g <= 0.0 {
            return Err(format!("Gas viscosity must be positive, got {}", mu_g));
        }
        if c_g < 0.0 {
            return Err(format!("Gas compressibility must be non-negative, got {}", c_g));
        }
        if rho_g <= 0.0 {
            return Err(format!("Gas density must be positive, got {}", rho_g));
        }
        self.mu_g = mu_g;
        self.c_g = c_g;
        self.rho_g = rho_g;
        Ok(())
    }

    #[wasm_bindgen(js_name = setGasRedissolutionEnabled)]
    pub fn set_gas_redissolution_enabled(&mut self, enabled: bool) {
        self.gas_redissolution_enabled = enabled;
    }

    /// Set injected fluid type for three-phase mode: "water" or "gas"
    #[wasm_bindgen(js_name = setInjectedFluid)]
    pub fn set_injected_fluid(&mut self, fluid: &str) -> Result<(), String> {
        self.injected_fluid = match fluid.to_ascii_lowercase().as_str() {
            "water" => InjectedFluid::Water,
            "gas" => InjectedFluid::Gas,
            other => return Err(format!("Unknown injected fluid '{}'; expected 'water' or 'gas'", other)),
        };
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
                    self.perm_x[id] = perms_x[k];
                    self.perm_y[id] = perms_y[k];
                    self.perm_z[id] = perms_z[k];
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::{ResolvedWellControl, WellControlDecision};

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

    #[test]
    fn black_oil_compressibility_falls_back_when_bo_slope_goes_negative() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.pvt.c_o = 1e-5;
        sim.pvt_table = Some(pvt::PvtTable::new(
            vec![
                pvt::PvtRow {
                    p_bar: 100.0,
                    rs_m3m3: 5.0,
                    bo_m3m3: 1.05,
                    mu_o_cp: 1.5,
                    bg_m3m3: 0.01,
                    mu_g_cp: 0.02,
                },
                pvt::PvtRow {
                    p_bar: 150.0,
                    rs_m3m3: 15.0,
                    bo_m3m3: 1.12,
                    mu_o_cp: 1.2,
                    bg_m3m3: 0.006,
                    mu_g_cp: 0.025,
                },
                pvt::PvtRow {
                    p_bar: 200.0,
                    rs_m3m3: 15.0,
                    bo_m3m3: 1.11944,
                    mu_o_cp: 1.3,
                    bg_m3m3: 0.0045,
                    mu_g_cp: 0.03,
                },
            ],
            sim.pvt.c_o,
        ));

        let c_o_below_bubble_point = sim.get_c_o(149.0);
        let c_o_above_bubble_point = sim.get_c_o(175.0);

        assert!(c_o_below_bubble_point.is_finite());
        assert!(c_o_above_bubble_point.is_finite());
        assert_eq!(c_o_below_bubble_point, sim.pvt.c_o);
        assert!(c_o_above_bubble_point >= sim.pvt.c_o);
    }

    #[test]
    fn effective_oil_compressibility_includes_dissolved_gas_below_bubble_point() {
        // Below bubble point, Bo increases with pressure and Rs increases with pressure.
        // The dissolved gas term (Bg/Bo)·dRs/dp should dominate, giving c_o_eff >> c_o_base.
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.pvt.c_o = 1e-5;
        sim.rho_g = 0.9; // surface gas density for density test
        sim.pvt_table = Some(pvt::PvtTable::new(
            vec![
                pvt::PvtRow { p_bar: 100.0, rs_m3m3:  5.0, bo_m3m3: 1.05,  mu_o_cp: 1.5, bg_m3m3: 0.01,  mu_g_cp: 0.02 },
                pvt::PvtRow { p_bar: 150.0, rs_m3m3: 15.0, bo_m3m3: 1.12,  mu_o_cp: 1.2, bg_m3m3: 0.006, mu_g_cp: 0.025 },
                pvt::PvtRow { p_bar: 200.0, rs_m3m3: 15.0, bo_m3m3: 1.119, mu_o_cp: 1.3, bg_m3m3: 0.0045, mu_g_cp: 0.03 },
            ],
            sim.pvt.c_o,
        ));

        // Below bubble point (125 bar): Bo is rising, Rs is rising → dissolved gas term active
        // Pass saturated Rs so the cell is recognized as saturated (gas liberation active)
        let rs_sat_125 = sim.pvt_table.as_ref().unwrap().interpolate(125.0).rs_m3m3;
        let c_eff_below = sim.get_c_o_effective(125.0, rs_sat_125);
        let c_o_below = sim.get_c_o(125.0);
        assert!(c_eff_below.is_finite());
        assert!(c_eff_below > 0.0);
        // Effective compressibility should be much larger than fallback c_o due to dissolved gas
        assert!(c_eff_below > c_o_below, "c_o_effective ({c_eff_below}) must exceed c_o ({c_o_below}) below bubble point");

        // Above bubble point (175 bar): Rs is constant → dissolved gas term ≈ 0
        let rs_sat_175 = sim.pvt_table.as_ref().unwrap().interpolate(175.0).rs_m3m3;
        let c_eff_above = sim.get_c_o_effective(175.0, rs_sat_175);
        let c_o_above = sim.get_c_o(175.0);
        assert!(c_eff_above.is_finite());
        assert!(c_eff_above > 0.0);
        // Above bubble point, effective ≈ standard (both come from Bo decline with p)
        assert!((c_eff_above - c_o_above).abs() / c_o_above < 0.5,
            "c_o_effective ({c_eff_above}) should be close to c_o ({c_o_above}) above bubble point");

        // Density should include dissolved gas mass: ρ = (ρ_o_sc + Rs·ρ_g_sc) / Bo
        let rho = sim.get_rho_o(125.0);
        let row = sim.pvt_table.as_ref().unwrap().interpolate(125.0);
        let expected = (sim.pvt.rho_o + row.rs_m3m3 * sim.rho_g) / row.bo_m3m3;
        assert!((rho - expected).abs() < 1e-6, "ρ_o ({rho}) should include dissolved gas ({expected})");
        // With dissolved gas, density must exceed simple ρ_o_sc / Bo
        let rho_simple = sim.pvt.rho_o / row.bo_m3m3;
        assert!(rho > rho_simple, "ρ_o with Rs ({rho}) must exceed dead-oil density ({rho_simple})");
    }

    #[test]
    fn producer_surface_rate_target_converts_using_oil_fraction_and_bo() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_initial_pressure(200.0);
        sim.set_initial_saturation(0.2);
        sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
        sim.set_target_well_surface_rates(0.0, 100.0).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let well = sim.wells.first().unwrap();
        let q_target = sim.target_rate_m3_day(well, 200.0).unwrap();
        let krw = sim.scal.k_rw(sim.sat_water[0]);
        let kro = sim.scal.k_ro(sim.sat_water[0]);
        let lambda_w = krw / sim.get_mu_w(200.0);
        let lambda_o = kro / sim.get_mu_o(200.0);
        let oil_fraction = lambda_o / (lambda_w + lambda_o);
        let expected = 100.0 * sim.get_b_o_cell(0, 200.0) / oil_fraction;

        assert!((q_target - expected).abs() < 1e-9);
    }

    #[test]
    fn rate_history_records_bhp_limited_fraction_for_rate_controlled_wells() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_initial_pressure(200.0);
        sim.set_initial_saturation(0.2);
        sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
        sim.set_target_well_rates(0.0, 5000.0).unwrap();
        sim.set_well_bhp_limits(150.0, 300.0).unwrap();
        sim.add_well(0, 0, 0, 150.0, 0.1, 0.0, false).unwrap();

        sim.step(1.0);

        let point = sim.rate_history.last().unwrap();
        assert_eq!(point.producer_bhp_limited_fraction, 1.0);
        assert_eq!(point.injector_bhp_limited_fraction, 0.0);
    }

    fn buckley_case_a(
        name: &'static str,
        nx: usize,
        dt_days: f64,
        max_steps: usize,
    ) -> BuckleyCase {
        BuckleyCase {
            name,
            nx,
            permeability_md: 2000.0,
            dt_days,
            max_steps,
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
        }
    }

    fn buckley_case_b(
        name: &'static str,
        nx: usize,
        dt_days: f64,
        max_steps: usize,
    ) -> BuckleyCase {
        BuckleyCase {
            name,
            nx,
            permeability_md: 2000.0,
            dt_days,
            max_steps,
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
        }
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
        (0..sim.nx * sim.ny * sim.nz)
            .map(|i| sim.sat_water[i] * sim.pore_volume_m3(i))
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
        let mut sim = ReservoirSimulator::new(case.nx, 1, 1, 0.2);
        sim.set_rel_perm_props(case.s_wc, case.s_or, case.n_w, case.n_o, 1.0, 1.0)
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

        let total_pv = (0..sim.nx * sim.ny * sim.nz)
            .map(|i| sim.pore_volume_m3(i))
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
        let mut sim = ReservoirSimulator::new(5, 1, 1, 0.2);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(4, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        for _ in 0..20 {
            sim.step(0.5);
        }

        let sw_min = sim.scal.s_wc;
        let sw_max = 1.0 - sim.scal.s_or;

        for i in 0..sim.nx * sim.ny * sim.nz {
            assert!(sim.sat_water[i] >= sw_min - 1e-9);
            assert!(sim.sat_water[i] <= sw_max + 1e-9);
            assert!(sim.sat_oil[i] >= -1e-9);
            assert!(sim.sat_oil[i] <= 1.0 + 1e-9);
            assert!((sim.sat_water[i] + sim.sat_oil[i] - 1.0).abs() < 1e-8);
        }
    }

    #[test]
    fn water_mass_balance_sanity_without_wells() {
        let mut sim = ReservoirSimulator::new(4, 4, 1, 0.2);
        let water_before = total_water_volume(&sim);

        sim.step(1.0);

        let water_after = total_water_volume(&sim);
        assert!((water_after - water_before).abs() < 1e-6);
    }

    #[test]
    fn adaptive_timestep_produces_multiple_substeps_for_strong_flow() {
        let mut sim = ReservoirSimulator::new(3, 1, 1, 0.2);
        sim.set_permeability_random(100_000.0, 100_000.0).unwrap();
        sim.set_stability_params(0.01, 75.0, 0.75);
        sim.add_well(0, 0, 0, 700.0, 0.1, 0.0, true).unwrap();
        sim.add_well(2, 0, 0, 50.0, 0.1, 0.0, false).unwrap();

        sim.step(30.0);

        assert!(sim.rate_history.len() > 1);
        assert!(sim.time_days > 0.0);
        assert!((sim.time_days - 30.0).abs() < 1e-9);
    }

    #[test]
    fn multiple_wells_in_same_block_keep_rates_finite() {
        let mut sim = ReservoirSimulator::new(4, 1, 1, 0.2);
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

        for i in 0..sim.nx * sim.ny * sim.nz {
            assert!(sim.pressure[i].is_finite());
            assert!(sim.sat_water[i].is_finite());
            assert!(sim.sat_oil[i].is_finite());
        }
    }

    #[test]
    fn out_of_bounds_well_is_rejected_without_state_change() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
        let wells_before = sim.wells.len();

        let result = sim.add_well(2, 0, 0, 250.0, 0.1, 0.0, false);
        err_contains(result, "out of bounds");

        assert_eq!(sim.wells.len(), wells_before);
    }

    #[test]
    fn stability_extremes_produce_finite_state() {
        let mut sim_loose = ReservoirSimulator::new(3, 1, 1, 0.2);
        sim_loose.set_stability_params(1.0, 75.0, 0.75);
        sim_loose
            .set_permeability_random(20_000.0, 20_000.0)
            .unwrap();
        sim_loose.add_well(0, 0, 0, 650.0, 0.1, 0.0, true).unwrap();
        sim_loose.add_well(2, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        sim_loose.step(5.0);

        let mut sim_tight = ReservoirSimulator::new(3, 1, 1, 0.2);
        sim_tight.set_stability_params(0.01, 75.0, 0.75);
        sim_tight
            .set_permeability_random(20_000.0, 20_000.0)
            .unwrap();
        sim_tight.add_well(0, 0, 0, 650.0, 0.1, 0.0, true).unwrap();
        sim_tight.add_well(2, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
        sim_tight.step(5.0);

        for sim in [&sim_loose, &sim_tight] {
            for i in 0..sim.nx * sim.ny * sim.nz {
                assert!(sim.pressure[i].is_finite());
                assert!(sim.sat_water[i].is_finite());
                assert!(sim.sat_oil[i].is_finite());
            }
            assert!(sim.time_days > 0.0);
            assert!(sim.time_days <= 5.0);
            assert!(!sim.rate_history.is_empty());
        }

        assert!(sim_tight.rate_history.len() >= sim_loose.rate_history.len());
    }

    #[test]
    fn api_contract_rejects_invalid_relperm_parameters() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
        err_contains(
            sim.set_rel_perm_props(0.6, 0.5, 2.0, 2.0, 1.0, 1.0),
            "must be < 1.0",
        );
        err_contains(
            sim.set_rel_perm_props(0.1, 0.1, 0.0, 2.0, 1.0, 1.0),
            "must be positive",
        );
        err_contains(
            sim.set_rel_perm_props(f64::NAN, 0.1, 2.0, 2.0, 1.0, 1.0),
            "finite numbers",
        );
    }

    #[test]
    fn api_contract_allows_zero_water_relperm_endpoint() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
        sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 0.0, 1.0)
            .expect("k_rw_max = 0 should be accepted for immobile-water cases");
    }

    #[test]
    fn api_contract_rejects_invalid_density_inputs() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
        err_contains(sim.set_fluid_densities(-800.0, 1000.0), "must be positive");
        err_contains(sim.set_fluid_densities(800.0, f64::NAN), "finite numbers");
    }

    #[test]
    fn api_contract_rejects_invalid_capillary_inputs() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
        err_contains(sim.set_capillary_params(-1.0, 2.0), "non-negative");
        err_contains(sim.set_capillary_params(5.0, 0.0), "positive");
        err_contains(sim.set_capillary_params(f64::NAN, 2.0), "finite numbers");
    }

    #[test]
    fn gravity_toggle_builds_hydrostatic_vertical_gradient() {
        let mut sim_no_g = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim_no_g
            .set_permeability_random_seeded(50_000.0, 50_000.0, 42)
            .unwrap();
        sim_no_g.set_initial_pressure(300.0);
        sim_no_g.set_initial_saturation(0.9);
        sim_no_g.pc.p_entry = 0.0;
        sim_no_g.set_gravity_enabled(false);
        sim_no_g.step(2.0);

        let p_top_no_g = sim_no_g.pressure[sim_no_g.idx(0, 0, 0)];
        let p_bot_no_g = sim_no_g.pressure[sim_no_g.idx(0, 0, 1)];

        let mut sim_g = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim_g
            .set_permeability_random_seeded(50_000.0, 50_000.0, 42)
            .unwrap();
        sim_g.set_initial_pressure(300.0);
        sim_g.set_initial_saturation(0.9);
        sim_g.pc.p_entry = 0.0;
        sim_g.set_gravity_enabled(true);
        sim_g.step(2.0);

        let p_top_g = sim_g.pressure[sim_g.idx(0, 0, 0)];
        let p_bot_g = sim_g.pressure[sim_g.idx(0, 0, 1)];

        assert!((p_bot_no_g - p_top_no_g).abs() < 1e-5);
        assert!(p_bot_g > p_top_g);
    }

    #[test]
    fn hydrostatic_initial_gradient_stays_quieter_with_gravity_enabled() {
        let initial_sw = 0.9;

        let mut sim_g = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim_g
            .set_permeability_random_seeded(80_000.0, 80_000.0, 7)
            .unwrap();
        sim_g.set_initial_saturation(initial_sw);
        sim_g.pc.p_entry = 0.0;
        sim_g.set_fluid_densities(800.0, 1000.0).unwrap();
        sim_g.set_gravity_enabled(true);

        let hydro_dp_bar = sim_g.pvt.rho_w * 9.80665 * sim_g.dz[0] * 1e-5;
        let top_id_g = sim_g.idx(0, 0, 0);
        let bot_id_g = sim_g.idx(0, 0, 1);
        sim_g.pressure[top_id_g] = 300.0;
        sim_g.pressure[bot_id_g] = 300.0 + hydro_dp_bar;
        sim_g.step(5.0);
        let sw_change_top_g = (sim_g.sat_water[top_id_g] - initial_sw).abs();

        let mut sim_no_g = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim_no_g
            .set_permeability_random_seeded(80_000.0, 80_000.0, 7)
            .unwrap();
        sim_no_g.set_initial_saturation(initial_sw);
        sim_no_g.pc.p_entry = 0.0;
        sim_no_g.set_fluid_densities(800.0, 1000.0).unwrap();
        sim_no_g.set_gravity_enabled(false);

        let top_id_no_g = sim_no_g.idx(0, 0, 0);
        let bot_id_no_g = sim_no_g.idx(0, 0, 1);
        sim_no_g.pressure[top_id_no_g] = 300.0;
        sim_no_g.pressure[bot_id_no_g] = 300.0 + hydro_dp_bar;
        sim_no_g.step(5.0);
        let sw_change_top_no_g = (sim_no_g.sat_water[top_id_no_g] - initial_sw).abs();

        assert!(sw_change_top_g <= sw_change_top_no_g);
    }

    #[test]
    fn api_contract_rejects_invalid_permeability_inputs() {
        let mut sim = ReservoirSimulator::new(2, 2, 2, 0.2);
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
    fn pressure_resolve_on_substep_produces_physical_results() {
        // Setup: high permeability + large dt forces stable_dt_factor < 1.0
        // triggering the re-solve path in step_internal
        let mut sim = ReservoirSimulator::new(5, 1, 1, 0.2);
        sim.set_permeability_random_seeded(100_000.0, 100_000.0, 42)
            .unwrap();
        sim.set_stability_params(0.02, 50.0, 0.5);
        sim.pc.p_entry = 0.0;
        sim.add_well(0, 0, 0, 600.0, 0.1, 0.0, true).unwrap();
        sim.add_well(4, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        // Large dt to force sub-stepping
        sim.step(20.0);

        // Must have sub-stepped (multiple rate history entries)
        assert!(
            sim.rate_history.len() > 1,
            "Expected sub-stepping, got {} entries",
            sim.rate_history.len()
        );

        // All state must be finite and physical
        for i in 0..sim.nx * sim.ny * sim.nz {
            assert!(
                sim.pressure[i].is_finite(),
                "Pressure not finite at cell {}",
                i
            );
            assert!(sim.sat_water[i].is_finite(), "Sw not finite at cell {}", i);
            assert!(
                sim.sat_water[i] >= sim.scal.s_wc - 1e-9,
                "Sw below s_wc at cell {}",
                i
            );
            assert!(
                sim.sat_water[i] <= 1.0 - sim.scal.s_or + 1e-9,
                "Sw above 1-s_or at cell {}",
                i
            );
            assert!((sim.sat_water[i] + sim.sat_oil[i] - 1.0).abs() < 1e-8);
        }

        // Pressure should remain within physical range (bounded by well BHPs)
        for i in 0..sim.nx * sim.ny * sim.nz {
            assert!(
                sim.pressure[i] > 50.0 && sim.pressure[i] < 700.0,
                "Pressure {} at cell {} outside physical range",
                sim.pressure[i],
                i
            );
        }

        // Material balance: each rate entry should have finite MB error
        for entry in &sim.rate_history {
            assert!(
                entry.material_balance_error_m3.is_finite(),
                "MB error not finite"
            );
        }
    }

    #[test]
    fn benchmark_like_substepping_completes_requested_dt() {
        let mut sim = ReservoirSimulator::new(24, 1, 1, 0.2);
        sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
            .unwrap();
        sim.set_initial_saturation(0.1);
        sim.set_permeability_random_seeded(2000.0, 2000.0, 42)
            .unwrap();
        sim.set_stability_params(0.05, 75.0, 0.75);
        sim.pc.p_entry = 0.0;
        sim.pvt.mu_w = 0.5;
        sim.pvt.mu_o = 1.0;
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(23, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        sim.step(0.5);

        assert!(
            (sim.time_days - 0.5).abs() < 1e-9,
            "Expected the simulator to complete the requested 0.5 day step, advanced {} days",
            sim.time_days
        );
        assert!(
            !sim.rate_history.is_empty() && (sim.rate_history.last().unwrap().time - 0.5).abs() < 1e-9,
            "Expected the last recorded rate-history time to match the completed step"
        );
    }

    #[test]
    fn benchmark_buckley_leverett_case_a_favorable_mobility() {
        let case = buckley_case_a("BL-Case-A", 24, 0.5, 4000);

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
        let case = buckley_case_b("BL-Case-B", 24, 0.25, 4000);

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
        let coarse_a = buckley_case_a("BL-Case-A-Coarse", 24, 0.5, 4000);
        let refined_a = buckley_case_a("BL-Case-A-Refined", 96, 0.125, 20000);

        let coarse_b = buckley_case_b("BL-Case-B-Coarse", 24, 0.25, 4000);
        let refined_b = buckley_case_b("BL-Case-B-Refined", 96, 0.125, 20000);

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
    fn benchmark_buckley_leverett_smaller_dt_improves_coarse_alignment() {
        let case_a_dt_050 = buckley_case_a("BL-Case-A-Coarse-dt0.50", 24, 0.5, 4000);
        let case_a_dt_025 = buckley_case_a("BL-Case-A-Coarse-dt0.25", 24, 0.25, 8000);
        let metrics_a_dt_050 = run_buckley_case(&case_a_dt_050);
        let metrics_a_dt_025 = run_buckley_case(&case_a_dt_025);
        let rel_err_a_dt_050 = ((metrics_a_dt_050.breakthrough_pv
            - metrics_a_dt_050.reference_breakthrough_pv)
            / metrics_a_dt_050.reference_breakthrough_pv)
            .abs();
        let rel_err_a_dt_025 = ((metrics_a_dt_025.breakthrough_pv
            - metrics_a_dt_025.reference_breakthrough_pv)
            / metrics_a_dt_025.reference_breakthrough_pv)
            .abs();

        let case_b_dt_050 = buckley_case_b("BL-Case-B-Coarse-dt0.50", 24, 0.5, 4000);
        let case_b_dt_025 = buckley_case_b("BL-Case-B-Coarse-dt0.25", 24, 0.25, 4000);
        let metrics_b_dt_050 = run_buckley_case(&case_b_dt_050);
        let metrics_b_dt_025 = run_buckley_case(&case_b_dt_025);
        let rel_err_b_dt_050 = ((metrics_b_dt_050.breakthrough_pv
            - metrics_b_dt_050.reference_breakthrough_pv)
            / metrics_b_dt_050.reference_breakthrough_pv)
            .abs();
        let rel_err_b_dt_025 = ((metrics_b_dt_025.breakthrough_pv
            - metrics_b_dt_025.reference_breakthrough_pv)
            / metrics_b_dt_025.reference_breakthrough_pv)
            .abs();

        println!(
            "Case-A coarse dt sweep rel_err: dt=0.50 -> {:.3}, dt=0.25 -> {:.3}",
            rel_err_a_dt_050, rel_err_a_dt_025
        );
        println!(
            "Case-B coarse dt sweep rel_err: dt=0.50 -> {:.3}, dt=0.25 -> {:.3}",
            rel_err_b_dt_050, rel_err_b_dt_025
        );

        assert!(
            rel_err_a_dt_025 + 1e-9 < rel_err_a_dt_050,
            "Smaller dt should improve Case-A coarse alignment: dt=0.50 -> {:.3}, dt=0.25 -> {:.3}",
            rel_err_a_dt_050,
            rel_err_a_dt_025
        );
        assert!(
            rel_err_b_dt_025 + 1e-9 < rel_err_b_dt_050,
            "Smaller dt should improve Case-B coarse alignment: dt=0.50 -> {:.3}, dt=0.25 -> {:.3}",
            rel_err_b_dt_050,
            rel_err_b_dt_025
        );
    }

    #[test]
    fn set_initial_saturation_per_layer_applies_uniformly_by_k() {
        let mut sim = ReservoirSimulator::new(2, 2, 3, 0.2);
        sim.set_initial_saturation_per_layer(vec![0.1, 0.4, 0.8])
            .unwrap();

        for k in 0..sim.nz {
            for j in 0..sim.ny {
                for i in 0..sim.nx {
                    let id = sim.idx(i, j, k);
                    let sw = sim.sat_water[id];
                    assert!((sw - [0.1, 0.4, 0.8][k]).abs() < 1e-12);
                    assert!((sim.sat_oil[id] - (1.0 - sw)).abs() < 1e-12);
                }
            }
        }
    }

    #[test]
    fn dynamic_pi_increases_with_higher_water_saturation() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
            .unwrap();
        sim.set_fluid_properties(3.0, 0.5).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let id = sim.idx(0, 0, 0);
        sim.sat_water[id] = sim.scal.s_wc;
        sim.sat_oil[id] = 1.0 - sim.scal.s_wc;
        sim.update_dynamic_well_productivity_indices();
        let pi_low_sw = sim.wells[0].productivity_index;

        let sw_high = 0.95 - sim.scal.s_or;
        sim.sat_water[id] = sw_high;
        sim.sat_oil[id] = 1.0 - sw_high;
        sim.update_dynamic_well_productivity_indices();
        let pi_high_sw = sim.wells[0].productivity_index;

        assert!(pi_high_sw > pi_low_sw);
    }

    #[test]
    fn well_productivity_index_matches_metric_unit_conversion() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
            .unwrap();
        sim.set_fluid_properties(2.0, 0.5).unwrap();
        sim.set_initial_saturation(0.1);

        let id = sim.idx(0, 0, 0);
        let well_radius = 0.1;
        let skin = 0.0;

        let pi = sim
            .calculate_well_productivity_index(id, well_radius, skin)
            .expect("PI should calculate for a valid isotropic cell");

        let kx = sim.perm_x[id];
        let ky = sim.perm_y[id];
        let k_avg = (kx * ky).sqrt();
        let ratio = kx / ky;
        let r_eq = 0.28
            * ((ratio.sqrt() * sim.dx.powi(2) + (1.0 / ratio).sqrt() * sim.dy.powi(2)).sqrt())
            / (ratio.powf(0.25) + (1.0 / ratio).powf(0.25));
        let denom = (r_eq / well_radius).ln() + skin;
        let total_mobility = 1.0 / sim.pvt.mu_o;

        // 1 mD = 9.8692e-16 m², 1/cP = 1000/(Pa·s), 1 bar = 1e5 Pa, 1 day = 86400 s
        let expected_darcy_metric_factor = 9.8692e-16 * 1e3 * 1e5 * 86400.0;
        let expected_pi = expected_darcy_metric_factor
            * 2.0
            * std::f64::consts::PI
            * k_avg
            * sim.dz[0]
            * total_mobility
            / denom;

        assert!(
            (pi - expected_pi).abs() / expected_pi < 1e-9,
            "PI mismatch: got {}, expected {}",
            pi,
            expected_pi
        );
    }

    #[test]
    fn rate_control_switches_to_bhp_when_limits_are_hit() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
        sim.set_target_well_rates(0.0, 500.0).unwrap();
        sim.set_well_bhp_limits(80.0, 120.0).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let well = &sim.wells[0];
        let pressure = 100.0;

        let control = sim
            .resolve_well_control(well, pressure)
            .expect("control decision should be available");

        assert!(control.bhp_limited);

        match control.decision {
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

    #[test]
    fn multi_completion_producer_rate_control_uses_shared_bhp() {
        let mut sim = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
        sim.set_target_well_rates(0.0, 100.0).unwrap();
        sim.set_well_bhp_limits(0.0, 300.0).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();
        sim.add_well(0, 0, 1, 100.0, 0.1, 0.0, false).unwrap();

        let id0 = sim.idx(0, 0, 0);
        let id1 = sim.idx(0, 0, 1);
        sim.pressure[id0] = 200.0;
        sim.pressure[id1] = 180.0;

        let well0 = &sim.wells[0];
        let well1 = &sim.wells[1];
        let q0 = sim.well_rate_m3_day(well0, sim.pressure[id0]).unwrap();
        let q1 = sim.well_rate_m3_day(well1, sim.pressure[id1]).unwrap();

        let bhp0 = sim.pressure[id0] - q0 / well0.productivity_index;
        let bhp1 = sim.pressure[id1] - q1 / well1.productivity_index;

        assert!(
            ((q0 + q1) - 100.0).abs() < 1e-6,
            "Expected producer completions to satisfy the group target, got q0={}, q1={}",
            q0,
            q1
        );
        assert!(
            (bhp0 - bhp1).abs() < 1e-9,
            "Expected all producer completions to share one flowing BHP, got {} and {}",
            bhp0,
            bhp1
        );
    }

    // ── Three-phase tests ────────────────────────────────────────────────────

    #[test]
    fn three_phase_relperm_k_ro_stone2_endpoints() {
        use crate::relperm::RockFluidPropsThreePhase;

        let rock = RockFluidPropsThreePhase {
            s_wc: 0.10, s_or: 0.10, n_w: 2.0, n_o: 2.0, k_rw_max: 0.8, k_ro_max: 0.9,
            s_gc: 0.05, s_gr: 0.05, s_org: 0.10, n_g: 1.5, k_rg_max: 0.7,
            tables: None,
        };

        // At connate water with no free gas → k_ro should equal k_ro_max
        let kro_at_swc = rock.k_ro_stone2(rock.s_wc, 0.0);
        assert!(
            (kro_at_swc - rock.k_ro_max).abs() < 1e-10,
            "k_ro_stone2(Swc, 0) should equal k_ro_max ({}) but got {}",
            rock.k_ro_max, kro_at_swc
        );

        // When gas saturation reaches 1 − Swc − Sorg, oil is at residual → k_ro = 0
        let sg_at_sorg = 1.0 - rock.s_wc - rock.s_org;
        let kro_max_gas = rock.k_ro_stone2(rock.s_wc, sg_at_sorg);
        assert!(
            kro_max_gas < 1e-9,
            "k_ro_stone2(Swc, sg_at_sorg) should be ~0 but got {}",
            kro_max_gas
        );

        // At fully flooded water (1-Sor), no gas → k_ro = 0
        let kro_at_max_water = rock.k_ro_stone2(1.0 - rock.s_or, 0.0);
        assert!(
            kro_at_max_water < 1e-9,
            "k_ro_stone2(1-Sor, 0) should be ~0 but got {}",
            kro_at_max_water
        );

        // k_ro must stay in [0, k_ro_max] across the entire saturation triangle
        for i in 0..=20 {
            let sw = rock.s_wc + i as f64 * (1.0 - rock.s_wc - rock.s_or) / 20.0;
            for j in 0..=20 {
                let sg = j as f64 * (1.0 - rock.s_wc - rock.s_gr) / 20.0;
                if sw + sg <= 1.0 {
                    let kro = rock.k_ro_stone2(sw, sg);
                    assert!(
                        kro >= -1e-10,
                        "k_ro_stone2 negative at sw={:.3}, sg={:.3}: {}",
                        sw, sg, kro
                    );
                    assert!(
                        kro <= rock.k_ro_max + 1e-10,
                        "k_ro_stone2 exceeds k_ro_max at sw={:.3}, sg={:.3}: {}",
                        sw, sg, kro
                    );
                }
            }
        }
    }

    #[test]
    fn three_phase_relperm_k_rg_endpoints_and_monotonicity() {
        use crate::relperm::RockFluidPropsThreePhase;

        let rock = RockFluidPropsThreePhase {
            s_wc: 0.10, s_or: 0.10, n_w: 2.0, n_o: 2.0, k_rw_max: 0.8, k_ro_max: 0.9,
            s_gc: 0.05, s_gr: 0.05, s_org: 0.10, n_g: 2.0, k_rg_max: 0.7,
            tables: None,
        };

        // Below and at critical gas saturation → k_rg = 0
        assert_eq!(rock.k_rg(0.0), 0.0);
        assert_eq!(rock.k_rg(rock.s_gc * 0.5), 0.0);
        assert!(rock.k_rg(rock.s_gc) < 1e-10, "k_rg(Sgc) = {}", rock.k_rg(rock.s_gc));

        // At maximum mobile gas saturation (Sg = 1 - Swc - Sgr) → k_rg = k_rg_max
        let sg_at_kmax = 1.0 - rock.s_wc - rock.s_gr;
        let krg_at_max = rock.k_rg(sg_at_kmax);
        assert!(
            (krg_at_max - rock.k_rg_max).abs() < 1e-10,
            "k_rg at max gas sat should be k_rg_max ({}) but got {}",
            rock.k_rg_max, krg_at_max
        );

        // k_rg is monotonically non-decreasing from Sgc to sg_at_kmax
        let mut prev_krg = 0.0;
        let n = 50;
        for i in 0..=n {
            let sg = rock.s_gc + i as f64 * (sg_at_kmax - rock.s_gc) / n as f64;
            let krg = rock.k_rg(sg);
            assert!(
                krg >= prev_krg - 1e-12,
                "k_rg not monotone at sg={:.4}: {} < prev {}",
                sg, krg, prev_krg
            );
            prev_krg = krg;
        }
    }

    #[test]
    fn three_phase_relperm_stone2_reduces_to_two_phase_at_zero_gas() {
        use crate::relperm::RockFluidPropsThreePhase;

        let rock = RockFluidPropsThreePhase {
            s_wc: 0.10, s_or: 0.10, n_w: 2.0, n_o: 2.0, k_rw_max: 0.8, k_ro_max: 0.9,
            s_gc: 0.05, s_gr: 0.05, s_org: 0.10, n_g: 1.5, k_rg_max: 0.7,
            tables: None,
        };

        // When Sg = 0, Stone II must collapse exactly to the oil-water k_ro curve:
        //   Stone II at Sg=0: kro_g→k_ro_max, krg→0
        //   => k_ro = k_ro_max * [(kro_w/k_ro_max + krw)(1 + 0) − krw] = kro_w
        let sw_vals = [0.10, 0.20, 0.30, 0.50, 0.70, 0.85, 0.90];
        for &sw in &sw_vals {
            let kro_stone2 = rock.k_ro_stone2(sw, 0.0);
            let kro_ow = rock.k_ro_water(sw);
            assert!(
                (kro_stone2 - kro_ow).abs() < 1e-10,
                "Stone II at Sg=0 does not match k_ro_water at sw={}: stone2={}, k_ro_w={}",
                sw, kro_stone2, kro_ow
            );
        }
    }

    #[test]
    fn three_phase_relperm_tables_interpolate_exact_spe1_points() {
        use crate::relperm::{RockFluidPropsThreePhase, SgofRow, SwofRow, ThreePhaseScalTables};

        let rock = RockFluidPropsThreePhase {
            s_wc: 0.12, s_or: 0.12, n_w: 2.0, n_o: 2.5, k_rw_max: 1e-5, k_ro_max: 1.0,
            s_gc: 0.04, s_gr: 0.04, s_org: 0.18, n_g: 1.5, k_rg_max: 0.984,
            tables: Some(ThreePhaseScalTables {
                swof: vec![
                    SwofRow { sw: 0.12, krw: 0.0, krow: 1.0, pcow: Some(0.0) },
                    SwofRow { sw: 0.24, krw: 1.86e-7, krow: 0.997, pcow: Some(0.0) },
                    SwofRow { sw: 1.0, krw: 1e-5, krow: 0.0, pcow: Some(0.0) },
                ],
                sgof: vec![
                    SgofRow { sg: 0.0, krg: 0.0, krog: 1.0, pcog: Some(0.0) },
                    SgofRow { sg: 0.5, krg: 0.72, krog: 0.001, pcog: Some(0.0) },
                    SgofRow { sg: 0.88, krg: 0.984, krog: 0.0, pcog: Some(0.0) },
                ],
            }),
        };

        assert!((rock.k_rw(0.12) - 0.0).abs() < 1e-12);
        assert!((rock.k_ro_water(0.24) - 0.997).abs() < 1e-12);
        assert!((rock.k_rg(0.5) - 0.72).abs() < 1e-12);
        assert!((rock.k_ro_gas(0.5) - 0.001).abs() < 1e-12);
    }

    #[test]
    fn three_phase_scal_tables_validate_valid_spe1_fragment() {
        use crate::relperm::{SgofRow, SwofRow, ThreePhaseScalTables};

        let tables = ThreePhaseScalTables {
            swof: vec![
                SwofRow { sw: 0.12, krw: 0.0, krow: 1.0, pcow: Some(0.0) },
                SwofRow { sw: 1.0, krw: 1e-5, krow: 0.0, pcow: Some(0.0) },
            ],
            sgof: vec![
                SgofRow { sg: 0.0, krg: 0.0, krog: 1.0, pcog: Some(0.0) },
                SgofRow { sg: 0.88, krg: 0.984, krog: 0.0, pcog: Some(0.0) },
            ],
        };

        assert!(tables.validate().is_ok());
    }

    /// Build a minimal 3-phase simulator with gas injection for physics tests.
    fn make_3phase_gas_injection_sim(nx: usize) -> ReservoirSimulator {
        let mut sim = ReservoirSimulator::new(nx, 1, 1, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.10, 0.10, 0.05, 0.05, 0.10,
            2.0, 2.0, 1.5,
            0.8, 0.9, 0.7,
        ).unwrap();
        sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_injected_fluid("gas").unwrap();
        sim.set_initial_saturation(0.10);
        sim.pc.p_entry = 0.0;
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(nx - 1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();
        sim
    }

    #[test]
    fn three_phase_saturation_sum_equals_one() {
        let mut sim = make_3phase_gas_injection_sim(8);

        for _ in 0..30 {
            sim.step(1.0);
        }

        let n = sim.nx * sim.ny * sim.nz;
        for i in 0..n {
            let sw = sim.sat_water[i];
            let so = sim.sat_oil[i];
            let sg = sim.sat_gas[i];
            let sum = sw + so + sg;
            assert!(
                (sum - 1.0).abs() < 1e-8,
                "sw+so+sg != 1 at cell {}: sw={:.6}, so={:.6}, sg={:.6}, sum={:.9}",
                i, sw, so, sg, sum
            );
            assert!(sw >= -1e-9, "Sw negative at cell {}: {}", i, sw);
            assert!(so >= -1e-9, "So negative at cell {}: {}", i, so);
            assert!(sg >= -1e-9, "Sg negative at cell {}: {}", i, sg);
        }
    }

    #[test]
    fn three_phase_gas_injection_increases_avg_gas_saturation() {
        let mut sim = make_3phase_gas_injection_sim(5);

        let n = sim.nx * sim.ny * sim.nz;
        let avg_sg_initial: f64 = sim.sat_gas.iter().sum::<f64>() / n as f64;
        assert_eq!(avg_sg_initial, 0.0, "Initial gas saturation should be zero");

        for _ in 0..50 {
            sim.step(2.0);
        }

        let avg_sg_final: f64 = sim.sat_gas.iter().sum::<f64>() / n as f64;
        assert!(
            avg_sg_final > 0.01,
            "Gas saturation should increase during gas injection, avg_sg={:.6}",
            avg_sg_final
        );
    }

    #[test]
    fn three_phase_rate_history_records_gas_production() {
        let mut sim = make_3phase_gas_injection_sim(5);

        for _ in 0..20 {
            sim.step(2.0);
        }

        let last = sim.rate_history.last().expect("rate history should have entries");
        assert!(
            last.total_production_gas.is_finite(),
            "total_production_gas should be finite, got {}",
            last.total_production_gas
        );

        let total_gas_produced: f64 = sim.rate_history.iter()
            .map(|r| r.total_production_gas.max(0.0))
            .sum();
        assert!(
            total_gas_produced > 0.0,
            "Expected positive cumulative gas production after gas injection"
        );
    }

    #[test]
    fn three_phase_gas_injection_keeps_gas_balance_bounded() {
        let mut sim = make_3phase_gas_injection_sim(8);

        for _ in 0..40 {
            sim.step(2.0);
        }

        let latest = sim.rate_history.last().expect("rate history should have entries");
        assert!(latest.material_balance_error_gas_m3.is_finite());
        assert!(
            latest.material_balance_error_gas_m3 < 5.0e3,
            "gas material balance drift too large: {} Sm3",
            latest.material_balance_error_gas_m3
        );
    }

    #[test]
    fn three_phase_gas_injection_keeps_pressures_bounded_under_large_steps() {
        let mut sim = ReservoirSimulator::new(6, 1, 3, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.10, 0.10, 0.05, 0.05, 0.10,
            2.0, 2.0, 1.5,
            0.8, 0.9, 0.7,
        ).unwrap();
        sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_injected_fluid("gas").unwrap();
        sim.set_initial_pressure(330.0);
        sim.set_initial_saturation(0.12);
        sim.set_stability_params(0.05, 75.0, 0.75);
        sim.pc.p_entry = 0.0;
        sim.add_well(0, 0, 0, 450.0, 0.1, 0.0, true).unwrap();
        sim.add_well(5, 0, 2, 150.0, 0.1, 0.0, false).unwrap();

        for _ in 0..12 {
            sim.step(5.0);
        }

        for (idx, pressure) in sim.pressure.iter().enumerate() {
            assert!(pressure.is_finite(), "pressure must remain finite at cell {}", idx);
            assert!(
                *pressure > 1.0 && *pressure < 5_000.0,
                "pressure {} at cell {} escaped the physical envelope",
                pressure,
                idx
            );
        }

        for (idx, sg) in sim.sat_gas.iter().enumerate() {
            assert!(sg.is_finite(), "gas saturation must remain finite at cell {}", idx);
            assert!(
                *sg >= -1e-9 && *sg <= 1.0 + 1e-9,
                "gas saturation {} at cell {} escaped bounds",
                sg,
                idx
            );
        }

        for point in &sim.rate_history {
            assert!(point.avg_reservoir_pressure.is_finite());
            assert!(point.avg_reservoir_pressure > 1.0);
            assert!(point.avg_reservoir_pressure < 5_000.0);
        }
    }

    #[test]
    fn gas_injection_surface_totals_use_bg_conversion() {
        use crate::pvt::{PvtRow, PvtTable};

        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.10, 0.10, 0.05, 0.05, 0.10,
            2.0, 2.0, 1.5,
            0.8, 0.9, 0.7,
        ).unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_injected_fluid("gas").unwrap();
        sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
        sim.set_initial_pressure(100.0);
        sim.set_initial_saturation(0.10);
        sim.pvt_table = Some(PvtTable::new(
            vec![PvtRow {
                p_bar: 100.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.2,
                mu_o_cp: 1.0,
                bg_m3m3: 0.25,
                mu_g_cp: 0.02,
            }],
            sim.pvt.c_o,
        ));
        sim.set_well_control_modes("rate".to_string(), "bhp".to_string());
        sim.set_target_well_surface_rates(120.0, 0.0).unwrap();
        sim.set_well_bhp_limits(0.0, 1.0e9).unwrap();
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, true).unwrap();

        sim.step(1.0);

        let latest = sim.rate_history.last().expect("rate history should have an entry");
        assert!(
            (latest.total_injection - 120.0).abs() < 1e-6,
            "Expected gas injector surface total to match target surface rate, got {}",
            latest.total_injection
        );
    }

    #[test]
    fn below_bubble_point_flash_conserves_total_gas_inventory() {
        use crate::pvt::{PvtRow, PvtTable};
        use nalgebra::DVector;

        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.10, 0.10, 0.05, 0.05, 0.10,
            2.0, 2.0, 1.5,
            0.8, 0.9, 0.7,
        ).unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_gas_redissolution_enabled(false);
        sim.set_initial_pressure(175.0);
        sim.set_initial_saturation(0.10);
        sim.set_initial_gas_saturation(0.0);
        sim.pvt.c_o = 1e-5;
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow { p_bar: 100.0, rs_m3m3: 5.0, bo_m3m3: 1.05,  mu_o_cp: 1.5, bg_m3m3: 0.01,  mu_g_cp: 0.02 },
                PvtRow { p_bar: 150.0, rs_m3m3: 15.0, bo_m3m3: 1.12,  mu_o_cp: 1.2, bg_m3m3: 0.006, mu_g_cp: 0.025 },
                PvtRow { p_bar: 200.0, rs_m3m3: 15.0, bo_m3m3: 1.119, mu_o_cp: 1.3, bg_m3m3: 0.0045, mu_g_cp: 0.03 },
            ],
            sim.pvt.c_o,
        ));
        sim.set_initial_rs(15.0);

        let vp_m3 = sim.pore_volume_m3(0);
        let p_old = sim.pressure[0];
        let bg_old = sim.get_b_g(p_old).max(1e-9);
        let bo_old = sim.get_b_o_cell(0, p_old).max(1e-9);
        let gas_before_sc = sim.sat_gas[0] * vp_m3 / bg_old
            + (sim.sat_oil[0] * vp_m3 / bo_old) * sim.rs[0];

        sim.update_saturations_and_pressure(
            &DVector::from_vec(vec![125.0]),
            &vec![0.0],
            &vec![0.0],
            &vec![0.0],
            &[],
            1.0,
        );

        let p_new = sim.pressure[0];
        let bg_new = sim.get_b_g(p_new).max(1e-9);
        let bo_new = sim.get_b_o_cell(0, p_new).max(1e-9);
        let gas_after_sc = sim.sat_gas[0] * vp_m3 / bg_new
            + (sim.sat_oil[0] * vp_m3 / bo_new) * sim.rs[0];

        assert!(sim.sat_gas[0] > 0.0, "pressure drop below bubble point should liberate free gas");
        assert!(
            (gas_after_sc - gas_before_sc).abs() < 1e-8,
            "local flash should conserve total gas inventory, before={}, after={}",
            gas_before_sc,
            gas_after_sc,
        );
    }

    #[test]
    fn reporting_reuses_transport_control_rates() {
        use crate::pvt::{PvtRow, PvtTable};
        use nalgebra::DVector;

        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.10, 0.10, 0.05, 0.05, 0.10,
            2.0, 2.0, 1.5,
            0.8, 0.9, 0.7,
        ).unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_injected_fluid("gas").unwrap();
        sim.set_initial_pressure(100.0);
        sim.set_initial_saturation(0.10);
        sim.pvt_table = Some(PvtTable::new(
            vec![PvtRow {
                p_bar: 100.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.2,
                mu_o_cp: 1.0,
                bg_m3m3: 0.25,
                mu_g_cp: 0.02,
            }],
            sim.pvt.c_o,
        ));
        sim.set_well_control_modes("rate".to_string(), "bhp".to_string());
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, true).unwrap();

        let controls = vec![Some(ResolvedWellControl {
            decision: WellControlDecision::Rate { q_m3_day: -30.0 },
            bhp_limited: false,
        })];

        sim.update_saturations_and_pressure(
            &DVector::from_vec(vec![300.0]),
            &vec![0.0],
            &vec![0.0],
            &vec![0.0],
            &controls,
            1.0,
        );

        let latest = sim.rate_history.last().expect("rate history should have an entry");
        assert!(
            (latest.total_injection - 360.0).abs() < 1e-6,
            "reporting should reuse the transport rate-control decision, got {}",
            latest.total_injection,
        );
        assert_eq!(latest.injector_bhp_limited_fraction, 0.0);
    }

    #[test]
    fn producing_gor_is_zero_when_oil_rate_is_negligible() {
        use crate::pvt::{PvtRow, PvtTable};
        use nalgebra::DVector;

        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.10, 0.10, 0.05, 0.05, 0.10,
            2.0, 2.0, 1.5,
            0.8, 0.9, 0.7,
        ).unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_initial_pressure(100.0);
        sim.set_initial_saturation(0.10);
        sim.pvt_table = Some(PvtTable::new(
            vec![PvtRow {
                p_bar: 100.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.2,
                mu_o_cp: 1.0,
                bg_m3m3: 0.25,
                mu_g_cp: 0.02,
            }],
            sim.pvt.c_o,
        ));
        sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        let id = sim.idx(0, 0, 0);
        sim.pressure[id] = 300.0;
        sim.sat_water[id] = 0.12;
        sim.sat_gas[id] = 0.879_999;
        sim.sat_oil[id] = 0.000_001;

        let controls = vec![Some(ResolvedWellControl {
            decision: WellControlDecision::Bhp { bhp_bar: 100.0 },
            bhp_limited: false,
        })];

        sim.update_saturations_and_pressure(
            &DVector::from_vec(vec![300.0]),
            &vec![0.0],
            &vec![0.0],
            &vec![0.0],
            &controls,
            1.0,
        );

        let latest = sim.rate_history.last().expect("rate history should have an entry");
        assert_eq!(latest.producing_gor, 0.0);
    }

    #[test]
    fn three_phase_mode_disabled_sat_gas_stays_zero() {
        // In the default 2-phase mode, sat_gas must remain all zeros and sw+so=1.
        let mut sim = ReservoirSimulator::new(5, 1, 1, 0.2);
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(4, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        for _ in 0..20 {
            sim.step(1.0);
        }

        for (i, &sg) in sim.sat_gas.iter().enumerate() {
            assert_eq!(sg, 0.0, "sat_gas[{}] should be zero in 2-phase mode, got {}", i, sg);
        }
        for i in 0..sim.nx * sim.ny * sim.nz {
            assert!(
                (sim.sat_water[i] + sim.sat_oil[i] - 1.0).abs() < 1e-8,
                "2-phase sw+so != 1 at cell {}",
                i
            );
        }
    }

    #[test]
    fn api_contract_rejects_invalid_3phase_relperm_params() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);

        // Endpoint sum >= 1.0 must be rejected
        err_contains(
            sim.set_three_phase_rel_perm_props(0.4, 0.3, 0.2, 0.2, 0.1, 2.0, 2.0, 1.5, 1.0, 1.0, 0.7),
            "must be < 1.0",
        );

        // Zero Corey exponent for water
        err_contains(
            sim.set_three_phase_rel_perm_props(0.1, 0.1, 0.05, 0.05, 0.10, 0.0, 2.0, 1.5, 1.0, 1.0, 0.7),
            "must be positive",
        );

        // Zero Corey exponent for gas
        err_contains(
            sim.set_three_phase_rel_perm_props(0.1, 0.1, 0.05, 0.05, 0.10, 2.0, 2.0, 0.0, 1.0, 1.0, 0.7),
            "must be positive",
        );
    }

    #[test]
    fn api_contract_rejects_invalid_gas_fluid_properties() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);

        err_contains(sim.set_gas_fluid_properties(0.0, 1e-4, 10.0), "must be positive");
        err_contains(sim.set_gas_fluid_properties(-0.01, 1e-4, 10.0), "must be positive");
        err_contains(sim.set_gas_fluid_properties(0.02, -1e-4, 10.0), "non-negative");
        err_contains(sim.set_gas_fluid_properties(0.02, 1e-4, 0.0), "must be positive");
    }

    #[test]
    fn api_contract_rejects_invalid_injected_fluid_string() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);

        err_contains(sim.set_injected_fluid("steam"), "Unknown injected fluid");
        err_contains(sim.set_injected_fluid(""), "Unknown injected fluid");

        // Valid strings must succeed
        assert!(sim.set_injected_fluid("water").is_ok());
        assert!(sim.set_injected_fluid("gas").is_ok());
    }

    #[test]
    fn api_contract_rejects_invalid_gas_oil_capillary_params() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);

        err_contains(sim.set_gas_oil_capillary_params(-1.0, 2.0), "non-negative");
        err_contains(sim.set_gas_oil_capillary_params(5.0, 0.0), "positive");

        // Valid parameters must succeed
        assert!(sim.set_gas_oil_capillary_params(0.0, 2.0).is_ok());
        assert!(sim.set_gas_oil_capillary_params(5.0, 1.5).is_ok());
    }

    #[test]
    fn per_layer_dz_affects_pore_volume_and_depth() {
        let mut sim = ReservoirSimulator::new(2, 2, 3, 0.25);
        sim.set_cell_dimensions_per_layer(100.0, 100.0, vec![6.0, 9.0, 15.0])
            .unwrap();

        // Pore volume = dx * dy * dz_k * porosity
        let id_k0 = sim.idx(0, 0, 0);
        let id_k1 = sim.idx(0, 0, 1);
        let id_k2 = sim.idx(0, 0, 2);

        let pv0 = sim.pore_volume_m3(id_k0);
        let pv1 = sim.pore_volume_m3(id_k1);
        let pv2 = sim.pore_volume_m3(id_k2);

        assert!((pv0 - 100.0 * 100.0 * 6.0 * 0.25).abs() < 1e-10);
        assert!((pv1 - 100.0 * 100.0 * 9.0 * 0.25).abs() < 1e-10);
        assert!((pv2 - 100.0 * 100.0 * 15.0 * 0.25).abs() < 1e-10);

        // Depth at k: sum of layers above + half of current layer
        let d0 = sim.depth_at_k(0);
        let d1 = sim.depth_at_k(1);
        let d2 = sim.depth_at_k(2);

        assert!((d0 - 3.0).abs() < 1e-10, "k=0: depth should be 6/2 = 3, got {}", d0);
        assert!((d1 - 10.5).abs() < 1e-10, "k=1: depth should be 6 + 9/2 = 10.5, got {}", d1);
        assert!((d2 - 22.5).abs() < 1e-10, "k=2: depth should be 6 + 9 + 15/2 = 22.5, got {}", d2);
    }

    #[test]
    fn per_layer_dz_validation_rejects_invalid_inputs() {
        let mut sim = ReservoirSimulator::new(2, 2, 3, 0.2);

        // Wrong length
        err_contains(
            sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![1.0, 2.0]),
            "length equal to nz",
        );

        // Non-positive dz
        err_contains(
            sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![1.0, 0.0, 3.0]),
            "positive and finite",
        );

        // Non-positive dx
        err_contains(
            sim.set_cell_dimensions_per_layer(-1.0, 10.0, vec![1.0, 2.0, 3.0]),
            "positive",
        );
    }

    #[test]
    fn set_initial_gas_saturation_per_layer_applies_by_k() {
        let mut sim = ReservoirSimulator::new(2, 2, 3, 0.2);
        sim.set_initial_saturation(0.2); // Sw = 0.2 everywhere
        sim.set_initial_gas_saturation_per_layer(vec![0.7, 0.0, 0.0])
            .unwrap();

        // Layer 0: Sg = 0.7, So = 1 - 0.2 - 0.7 = 0.1
        for j in 0..2 {
            for i in 0..2 {
                let id = sim.idx(i, j, 0);
                assert!((sim.sat_gas[id] - 0.7).abs() < 1e-10);
                assert!((sim.sat_oil[id] - 0.1).abs() < 1e-10);
            }
        }

        // Layers 1-2: Sg = 0, So = 0.8
        for k in 1..3 {
            for j in 0..2 {
                for i in 0..2 {
                    let id = sim.idx(i, j, k);
                    assert!((sim.sat_gas[id] - 0.0).abs() < 1e-10);
                    assert!((sim.sat_oil[id] - 0.8).abs() < 1e-10);
                }
            }
        }
    }

    #[test]
    fn set_initial_gas_saturation_per_layer_clamps_to_available() {
        let mut sim = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim.set_initial_saturation(0.5); // Sw = 0.5

        // Request Sg = 0.8 but only 0.5 is available (1 - Sw)
        sim.set_initial_gas_saturation_per_layer(vec![0.8, 0.0])
            .unwrap();

        let id0 = sim.idx(0, 0, 0);
        assert!((sim.sat_gas[id0] - 0.5).abs() < 1e-10, "Sg should clamp to 0.5");
        assert!((sim.sat_oil[id0] - 0.0).abs() < 1e-10, "So should be 0");
    }

    #[test]
    fn set_initial_gas_saturation_per_layer_validation() {
        let mut sim = ReservoirSimulator::new(2, 2, 3, 0.2);

        // Wrong length
        err_contains(
            sim.set_initial_gas_saturation_per_layer(vec![0.5, 0.0]),
            "length equal to nz",
        );

        // Out of range
        err_contains(
            sim.set_initial_gas_saturation_per_layer(vec![0.5, -0.1, 0.0]),
            "within [0, 1]",
        );
    }

    #[test]
    fn non_uniform_dz_transmissibility_z_direction() {
        let mut sim = ReservoirSimulator::new(1, 1, 2, 0.2);
        sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![6.0, 15.0])
            .unwrap();
        sim.set_permeability_random_seeded(100.0, 100.0, 42).unwrap();

        let id0 = sim.idx(0, 0, 0);
        let id1 = sim.idx(0, 0, 1);

        let t_z = sim.geometric_transmissibility(id0, id1, 'z');

        // For z-direction: area = dx * dy = 100, dist = (dz0 + dz1) / 2 = 10.5
        // T = k_h * area / dist
        let kz0 = sim.perm_z[id0];
        let kz1 = sim.perm_z[id1];
        let k_h = 2.0 * kz0 * kz1 / (kz0 + kz1);
        let expected = k_h * 100.0 / 10.5;

        assert!(
            (t_z - expected).abs() / expected < 1e-9,
            "Z-transmissibility with non-uniform dz: expected {}, got {}",
            expected, t_z
        );
    }

    #[test]
    fn average_reservoir_pressure_is_pv_weighted() {
        let mut sim = ReservoirSimulator::new(1, 1, 2, 0.25);
        sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![1.0, 9.0])
            .unwrap();

        let id0 = sim.idx(0, 0, 0);
        let id1 = sim.idx(0, 0, 1);
        sim.pressure[id0] = 100.0;
        sim.pressure[id1] = 200.0;

        let pv0 = sim.pore_volume_m3(id0);
        let pv1 = sim.pore_volume_m3(id1);
        let expected = (100.0 * pv0 + 200.0 * pv1) / (pv0 + pv1);

        assert!(
            (sim.average_reservoir_pressure_pv_weighted() - expected).abs() < 1e-12,
            "Expected PV-weighted pressure {}, got {}",
            expected,
            sim.average_reservoir_pressure_pv_weighted()
        );
        assert!(
            (sim.average_reservoir_pressure_pv_weighted() - 150.0).abs() > 1e-6,
            "PV-weighted average should differ from arithmetic mean when pore volumes differ"
        );
    }
}
