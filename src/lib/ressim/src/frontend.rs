use rand::rngs::StdRng;
use rand::RngExt;
use rand::SeedableRng;
use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::pvt;
use crate::well::WellSchedule;
use crate::{
    CapillaryPressure, FluidProperties, GasOilCapillaryPressure, InjectedFluid, ReservoirSimulator,
    RockFluidProps, RockFluidPropsThreePhase, ThreePhaseScalTables, TimePointRates, Well,
};

#[derive(Deserialize)]
struct GridStatePayload {
    pressure: Vec<f64>,
    sat_water: Vec<f64>,
    sat_oil: Vec<f64>,
}

#[wasm_bindgen]
impl ReservoirSimulator {
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
            dx: 10.0,
            dy: 10.0,
            dz: vec![1.0; nz],
            porosity,
            perm_x,
            perm_y,
            perm_z,
            pressure,
            sat_water,
            sat_oil,
            sat_gas,
            wells: Vec::new(),
            time_days: 0.0,
            pvt: FluidProperties::default_pvt(),
            scal: RockFluidProps::default_scal(),
            pc: CapillaryPressure::default_pc(),
            gravity_enabled: false,
            max_sat_change_per_step: 0.1,
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

    fn add_well_internal(
        &mut self,
        i: usize,
        j: usize,
        k: usize,
        bhp: f64,
        well_radius: f64,
        skin: f64,
        injector: bool,
        physical_well_id: Option<String>,
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
            physical_well_id,
            schedule: WellSchedule::default(),
            i,
            j,
            k,
            bhp,
            productivity_index: pi,
            injector,
            well_radius,
            skin,
        };
        well.validate(self.nx, self.ny, self.nz)?;
        self.wells.push(well);
        Ok(())
    }

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
        self.add_well_internal(i, j, k, bhp, well_radius, skin, injector, None)
    }

    #[wasm_bindgen(js_name = addWellWithId)]
    pub fn add_well_with_id(
        &mut self,
        i: usize,
        j: usize,
        k: usize,
        bhp: f64,
        well_radius: f64,
        skin: f64,
        injector: bool,
        physical_well_id: String,
    ) -> Result<(), String> {
        self.add_well_internal(
            i,
            j,
            k,
            bhp,
            well_radius,
            skin,
            injector,
            Some(physical_well_id),
        )
    }

    #[wasm_bindgen(js_name = setWellSchedule)]
    pub fn set_well_schedule(
        &mut self,
        physical_well_id: String,
        control_mode: String,
        target_rate_m3_day: f64,
        target_surface_rate_m3_day: f64,
        bhp_limit: f64,
        enabled: bool,
    ) -> Result<(), String> {
        let well_id = physical_well_id.trim();
        if well_id.is_empty() {
            return Err("Physical well id must not be empty".to_string());
        }

        let normalized_control_mode = match control_mode.trim().to_ascii_lowercase().as_str() {
            "rate" => "rate",
            "pressure" | "" => "pressure",
            other => {
                return Err(format!(
                    "Well control mode must be 'pressure' or 'rate', got: {}",
                    other
                ))
            }
        };

        let target_rate_m3_day = if target_rate_m3_day.is_finite() && target_rate_m3_day >= 0.0 {
            Some(target_rate_m3_day)
        } else {
            None
        };
        let target_surface_rate_m3_day = if target_surface_rate_m3_day.is_finite()
            && target_surface_rate_m3_day >= 0.0
        {
            Some(target_surface_rate_m3_day)
        } else {
            None
        };
        let bhp_limit = if bhp_limit.is_finite() {
            Some(bhp_limit)
        } else {
            None
        };

        let mut updated_any = false;
        for well in self.wells.iter_mut() {
            if well.physical_well_id.as_deref() == Some(well_id) {
                well.schedule = WellSchedule {
                    control_mode: Some(normalized_control_mode.to_string()),
                    target_rate_m3_day,
                    target_surface_rate_m3_day,
                    bhp_limit,
                    enabled,
                };
                updated_any = true;
            }
        }

        if !updated_any {
            return Err(format!("No well found for physical well id '{}'", well_id));
        }

        Ok(())
    }

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

    #[wasm_bindgen(js_name = getLastSolverWarning)]
    pub fn get_last_solver_warning(&self) -> String {
        self.last_solver_warning.clone()
    }

    #[wasm_bindgen(js_name = getDimensions)]
    pub fn get_dimensions(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&[self.nx, self.ny, self.nz]).unwrap()
    }

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

    #[wasm_bindgen(js_name = setCellDimensionsPerLayer)]
    pub fn set_cell_dimensions_per_layer(
        &mut self,
        dx: f64,
        dy: f64,
        dz_per_layer: Vec<f64>,
    ) -> Result<(), String> {
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
                self.nz,
                dz_per_layer.len()
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

    #[wasm_bindgen(js_name = setInitialSaturation)]
    pub fn set_initial_saturation(&mut self, sat_water: f64) {
        for i in 0..self.nx * self.ny * self.nz {
            self.sat_water[i] = sat_water.clamp(0.0, 1.0);
            self.sat_oil[i] = 1.0 - self.sat_water[i];
        }
    }

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
            self.perm_z[i] = rng.random_range(min_perm..=max_perm) / 10.0;
        }
        Ok(())
    }

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
            self.perm_z[i] = rng.random_range(min_perm..=max_perm) / 10.0;
        }
        Ok(())
    }

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
        let grid_data: GridStatePayload = serde_wasm_bindgen::from_value(grid_state)?;

        let expected_cells = self.nx * self.ny * self.nz;
        if grid_data.pressure.len() != expected_cells
            || grid_data.sat_water.len() != expected_cells
            || grid_data.sat_oil.len() != expected_cells
        {
            return Err(JsValue::from_str(&format!(
                "Mismatch grid size. Expected {}, got pressure len: {}, sat_water len: {}, sat_oil len: {}",
                expected_cells,
                grid_data.pressure.len(),
                grid_data.sat_water.len(),
                grid_data.sat_oil.len()
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

    #[wasm_bindgen(js_name = setPvtTable)]
    pub fn set_pvt_table(&mut self, table_js: JsValue) -> Result<(), JsValue> {
        let rows: Vec<pvt::PvtRow> = serde_wasm_bindgen::from_value(table_js)?;
        let table = pvt::PvtTable::new(rows, self.pvt.c_o);
        let n = self.nx * self.ny * self.nz;
        for i in 0..n {
            self.rs[i] = table.interpolate(self.pressure[i]).rs_m3m3;
        }
        self.pvt_table = Some(table);
        Ok(())
    }

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

    #[wasm_bindgen(js_name = setInitialGasSaturationPerLayer)]
    pub fn set_initial_gas_saturation_per_layer(&mut self, sg: Vec<f64>) -> Result<(), String> {
        if sg.len() != self.nz {
            return Err(format!(
                "Initial gas saturation vector must have length equal to nz ({}), got {}",
                self.nz,
                sg.len()
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

    #[wasm_bindgen(js_name = getSatGas)]
    pub fn get_sat_gas(&self) -> Vec<f64> {
        self.sat_gas.clone()
    }

    #[wasm_bindgen(js_name = getRs)]
    pub fn get_rs(&self) -> Vec<f64> {
        self.rs.clone()
    }

    #[wasm_bindgen(js_name = setThreePhaseModeEnabled)]
    pub fn set_three_phase_mode_enabled(&mut self, enabled: bool) {
        self.three_phase_mode = enabled;
    }

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
            s_wc,
            s_or,
            n_w,
            n_o,
            k_rw_max,
            k_ro_max,
            s_gc,
            s_gr,
            s_org,
            n_g,
            k_rg_max,
            tables: None,
        });
        Ok(())
    }

    #[wasm_bindgen(js_name = setThreePhaseScalTables)]
    pub fn set_three_phase_scal_tables(&mut self, table_js: JsValue) -> Result<(), JsValue> {
        let tables: ThreePhaseScalTables = serde_wasm_bindgen::from_value(table_js)?;
        tables
            .validate()
            .map_err(|message| JsValue::from_str(&message))?;

        let scal = self.scal_3p.as_mut().ok_or_else(|| {
            JsValue::from_str(
                "Three-phase relperm props must be configured before SWOF/SGOF tables",
            )
        })?;
        scal.tables = Some(tables);
        Ok(())
    }

    #[wasm_bindgen(js_name = setGasOilCapillaryParams)]
    pub fn set_gas_oil_capillary_params(
        &mut self,
        p_entry: f64,
        lambda: f64,
    ) -> Result<(), String> {
        if p_entry < 0.0 {
            return Err(format!(
                "Gas-oil capillary entry pressure must be non-negative, got {}",
                p_entry
            ));
        }
        if lambda <= 0.0 {
            return Err(format!(
                "Gas-oil capillary lambda must be positive, got {}",
                lambda
            ));
        }
        self.pc_og = Some(GasOilCapillaryPressure { p_entry, lambda });
        Ok(())
    }

    #[wasm_bindgen(js_name = setGasFluidProperties)]
    pub fn set_gas_fluid_properties(
        &mut self,
        mu_g: f64,
        c_g: f64,
        rho_g: f64,
    ) -> Result<(), String> {
        if mu_g <= 0.0 {
            return Err(format!("Gas viscosity must be positive, got {}", mu_g));
        }
        if c_g < 0.0 {
            return Err(format!(
                "Gas compressibility must be non-negative, got {}",
                c_g
            ));
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

    #[wasm_bindgen(js_name = setInjectedFluid)]
    pub fn set_injected_fluid(&mut self, fluid: &str) -> Result<(), String> {
        self.injected_fluid = match fluid.to_ascii_lowercase().as_str() {
            "water" => InjectedFluid::Water,
            "gas" => InjectedFluid::Gas,
            other => {
                return Err(format!(
                    "Unknown injected fluid '{}'; expected 'water' or 'gas'",
                    other
                ))
            }
        };
        Ok(())
    }

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
