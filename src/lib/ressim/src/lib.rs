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

use serde::{Deserialize, Serialize};
use std::f64;
use wasm_bindgen::prelude::*;

mod capillary;
mod fim;
mod frontend;
mod grid;
mod impes;
mod mobility;
mod pvt;
mod relperm;
mod reporting;
mod solvers;
mod step;
mod well;
mod well_control;

pub use capillary::{CapillaryPressure, GasOilCapillaryPressure};
pub use relperm::{
    RockFluidProps, RockFluidPropsThreePhase, SgofRow, SwofRow, ThreePhaseScalTables,
};
pub use reporting::{FimStepStats, SweepConfig, TimePointRates, WellRates};
pub use well::Well;

/// Which fluid the injector injects in three-phase mode.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum InjectedFluid {
    Water,
    Gas,
}

#[wasm_bindgen(start)]
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct FluidProperties {
    pub mu_o: f64,
    pub mu_w: f64,
    pub c_o: f64,
    pub c_w: f64,
    pub rho_o: f64,
    pub rho_w: f64,
}

impl FluidProperties {
    fn default_pvt() -> Self {
        Self {
            mu_o: 1.0,
            mu_w: 0.5,
            c_o: 1e-5,
            c_w: 3e-6,
            rho_o: 800.0,
            rho_w: 1000.0,
        }
    }
}

#[wasm_bindgen]
pub struct ReservoirSimulator {
    nx: usize,
    ny: usize,
    nz: usize,
    dx: f64,
    dy: f64,
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
    well_bhp_min: f64,
    well_bhp_max: f64,
    last_solver_warning: String,
    last_fim_trace: String,
    capture_fim_trace: bool,
    last_fim_step_stats: Option<reporting::FimStepStats>,
    fim_step_stats_history: Vec<reporting::FimStepStats>,
    cumulative_injection_m3: f64,
    cumulative_production_m3: f64,
    pub cumulative_mb_error_m3: f64,
    cumulative_mb_oil_error_m3: f64,
    pub cumulative_mb_gas_error_m3: f64,
    target_producer_rate_m3_day: f64,
    target_producer_surface_rate_m3_day: Option<f64>,
    rock_compressibility: f64,
    depth_reference_m: f64,
    b_o: f64,
    b_w: f64,
    rate_history: Vec<TimePointRates>,
    pub(crate) sat_gas: Vec<f64>,
    pub(crate) scal_3p: Option<RockFluidPropsThreePhase>,
    pub(crate) pc_og: Option<GasOilCapillaryPressure>,
    pub(crate) three_phase_mode: bool,
    pub(crate) injected_fluid: InjectedFluid,
    pub(crate) mu_g: f64,
    pub(crate) c_g: f64,
    pub(crate) rho_g: f64,
    pub(crate) pvt_table: Option<pvt::PvtTable>,
    pub(crate) rs: Vec<f64>,
    pub(crate) gas_redissolution_enabled: bool,
    pub(crate) fim_enabled: bool,
    pub(crate) sweep_config: Option<SweepConfig>,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::well_control::WellControlDecision;
    mod buckley;
    mod geometry_api;
    pub(crate) mod physics;
    mod pvt_properties;
    mod runtime_api;
    mod three_phase;
    mod well_controls;

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

    struct RateControlBenchmarkMetrics {
        total_production_oil: f64,
        total_injection: f64,
        producer_bhp_limited_fraction: f64,
        injector_bhp_limited_fraction: f64,
        avg_reservoir_pressure: f64,
    }

    fn run_rate_control_reporting_benchmark(fim_enabled: bool) -> RateControlBenchmarkMetrics {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_fim_enabled(fim_enabled);
        sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
        sim.set_target_well_surface_rates(0.0, 50.0).unwrap();
        sim.set_well_bhp_limits(50.0, 500.0).unwrap();
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        sim.step(0.25);

        let point = sim.rate_history.last().unwrap();
        RateControlBenchmarkMetrics {
            total_production_oil: point.total_production_oil,
            total_injection: point.total_injection,
            producer_bhp_limited_fraction: point.producer_bhp_limited_fraction,
            injector_bhp_limited_fraction: point.injector_bhp_limited_fraction,
            avg_reservoir_pressure: point.avg_reservoir_pressure,
        }
    }

    pub(crate) fn make_spe1_like_grid_sim(
        nx: usize,
        ny: usize,
        producer_i: usize,
        producer_j: usize,
        layer_perms_z: Vec<f64>,
        max_sat_change_per_step: f64,
        max_pressure_change_per_step: f64,
        max_well_rate_change_fraction: f64,
    ) -> ReservoirSimulator {
        use crate::pvt::{PvtRow, PvtTable};
        use crate::relperm::{SgofRow, SwofRow, ThreePhaseScalTables};

        let mut sim = ReservoirSimulator::new(nx, ny, 3, 0.3);
        let cell_dx = 3048.0 / nx as f64;
        let cell_dy = 3048.0 / ny as f64;
        sim.set_cell_dimensions_per_layer(cell_dx, cell_dy, vec![6.096, 9.144, 15.24])
            .unwrap();
        sim.set_fluid_properties(0.51, 0.318).unwrap();
        sim.set_fluid_compressibilities(2.06e-4, 4.67e-5).unwrap();
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 1.01,
                    rs_m3m3: 0.18,
                    bo_m3m3: 1.062,
                    mu_o_cp: 1.040,
                    bg_m3m3: 0.9361,
                    mu_g_cp: 0.0080,
                },
                PvtRow {
                    p_bar: 18.25,
                    rs_m3m3: 16.12,
                    bo_m3m3: 1.150,
                    mu_o_cp: 0.975,
                    bg_m3m3: 0.0679,
                    mu_g_cp: 0.0096,
                },
                PvtRow {
                    p_bar: 35.49,
                    rs_m3m3: 32.06,
                    bo_m3m3: 1.207,
                    mu_o_cp: 0.910,
                    bg_m3m3: 0.0352,
                    mu_g_cp: 0.0112,
                },
                PvtRow {
                    p_bar: 69.96,
                    rs_m3m3: 66.08,
                    bo_m3m3: 1.295,
                    mu_o_cp: 0.830,
                    bg_m3m3: 0.0179,
                    mu_g_cp: 0.0140,
                },
                PvtRow {
                    p_bar: 138.91,
                    rs_m3m3: 113.29,
                    bo_m3m3: 1.435,
                    mu_o_cp: 0.695,
                    bg_m3m3: 0.00906,
                    mu_g_cp: 0.0189,
                },
                PvtRow {
                    p_bar: 173.38,
                    rs_m3m3: 138.03,
                    bo_m3m3: 1.500,
                    mu_o_cp: 0.641,
                    bg_m3m3: 0.00727,
                    mu_g_cp: 0.0208,
                },
                PvtRow {
                    p_bar: 207.85,
                    rs_m3m3: 165.64,
                    bo_m3m3: 1.565,
                    mu_o_cp: 0.594,
                    bg_m3m3: 0.00607,
                    mu_g_cp: 0.0228,
                },
                PvtRow {
                    p_bar: 276.79,
                    rs_m3m3: 226.20,
                    bo_m3m3: 1.695,
                    mu_o_cp: 0.510,
                    bg_m3m3: 0.00455,
                    mu_g_cp: 0.0268,
                },
                PvtRow {
                    p_bar: 345.73,
                    rs_m3m3: 288.17,
                    bo_m3m3: 1.827,
                    mu_o_cp: 0.449,
                    bg_m3m3: 0.00364,
                    mu_g_cp: 0.0309,
                },
            ],
            sim.pvt.c_o,
        ));
        sim.set_initial_rs(226.197);
        sim.set_rock_properties(4.35e-5, 2560.0, 1.695, 1.038)
            .unwrap();
        sim.set_fluid_densities(860.0, 1033.0).unwrap();
        sim.set_initial_pressure(331.0);
        sim.set_initial_saturation(0.12);
        sim.set_capillary_params(0.0, 2.0).unwrap();
        sim.set_gravity_enabled(true);
        sim.set_three_phase_rel_perm_props(
            0.12, 0.12, 0.04, 0.04, 0.18, 2.0, 2.5, 1.5, 1e-5, 1.0, 0.984,
        )
        .unwrap();
        sim.scal_3p.as_mut().unwrap().tables = Some(ThreePhaseScalTables {
            swof: vec![
                SwofRow {
                    sw: 0.12,
                    krw: 0.0,
                    krow: 1.0,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.18,
                    krw: 4.64876033057851e-8,
                    krow: 1.0,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.24,
                    krw: 1.86e-7,
                    krow: 0.997,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.3,
                    krw: 4.18388429752066e-7,
                    krow: 0.98,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.36,
                    krw: 7.43801652892562e-7,
                    krow: 0.7,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.42,
                    krw: 1.16219008264463e-6,
                    krow: 0.35,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.48,
                    krw: 1.67355371900826e-6,
                    krow: 0.2,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.54,
                    krw: 2.27789256198347e-6,
                    krow: 0.09,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.6,
                    krw: 2.97520661157025e-6,
                    krow: 0.021,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.66,
                    krw: 3.7654958677686e-6,
                    krow: 0.01,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.72,
                    krw: 4.64876033057851e-6,
                    krow: 0.001,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.78,
                    krw: 5.625e-6,
                    krow: 0.0001,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.84,
                    krw: 6.69421487603306e-6,
                    krow: 0.0,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.91,
                    krw: 8.05914256198347e-6,
                    krow: 0.0,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 1.0,
                    krw: 1e-5,
                    krow: 0.0,
                    pcow: Some(0.0),
                },
            ],
            sgof: vec![
                SgofRow {
                    sg: 0.0,
                    krg: 0.0,
                    krog: 1.0,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.001,
                    krg: 0.0,
                    krog: 1.0,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.02,
                    krg: 0.0,
                    krog: 0.997,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.05,
                    krg: 0.005,
                    krog: 0.98,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.12,
                    krg: 0.025,
                    krog: 0.7,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.2,
                    krg: 0.075,
                    krog: 0.35,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.25,
                    krg: 0.125,
                    krog: 0.2,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.3,
                    krg: 0.19,
                    krog: 0.09,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.4,
                    krg: 0.41,
                    krog: 0.021,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.45,
                    krg: 0.6,
                    krog: 0.01,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.5,
                    krg: 0.72,
                    krog: 0.001,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.6,
                    krg: 0.87,
                    krog: 0.0001,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.7,
                    krg: 0.94,
                    krog: 0.0,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.85,
                    krg: 0.98,
                    krog: 0.0,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.88,
                    krg: 0.984,
                    krog: 0.0,
                    pcog: Some(0.0),
                },
            ],
        });
        sim.set_gas_fluid_properties(0.027, 1e-4, 0.854).unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.set_injected_fluid("gas").unwrap();
        sim.set_gas_redissolution_enabled(false);
        sim.set_stability_params(
            max_sat_change_per_step,
            max_pressure_change_per_step,
            max_well_rate_change_fraction,
        );
        sim.set_well_control_modes("rate".to_string(), "rate".to_string());
        sim.set_target_well_rates(12_000.0, 5_400.0).unwrap();
        sim.set_target_well_surface_rates(2_831_680.0, 3_179.74)
            .unwrap();
        sim.set_well_bhp_limits(69.0, 621.0).unwrap();
        sim.set_permeability_per_layer(
            vec![500.0, 50.0, 200.0],
            vec![500.0, 50.0, 200.0],
            layer_perms_z,
        )
        .unwrap();
        sim.add_well(producer_i, producer_j, 2, 69.0, 0.0762, 0.0, false)
            .unwrap();
        sim.add_well(0, 0, 0, 621.0, 0.0762, 0.0, true).unwrap();
        sim
    }

    fn make_spe1_like_sim(
        layer_perms_z: Vec<f64>,
        max_sat_change_per_step: f64,
        max_pressure_change_per_step: f64,
        max_well_rate_change_fraction: f64,
    ) -> ReservoirSimulator {
        make_spe1_like_grid_sim(
            10,
            10,
            9,
            9,
            layer_perms_z,
            max_sat_change_per_step,
            max_pressure_change_per_step,
            max_well_rate_change_fraction,
        )
    }

    pub(crate) fn make_spe1_like_base_sim() -> ReservoirSimulator {
        make_spe1_like_sim(vec![500.0, 50.0, 200.0], 0.05, 50.0, 0.5)
    }
}
