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
mod mobility;
mod pressure_eqn;
mod pvt;
mod relperm;
mod reporting;
mod solvers;
mod step;
mod transport;
mod well;
mod well_control;

pub use capillary::{CapillaryPressure, GasOilCapillaryPressure};
pub use relperm::{
    RockFluidProps, RockFluidPropsThreePhase, SgofRow, SwofRow, ThreePhaseScalTables,
};
pub use reporting::{TimePointRates, WellRates};
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
    cumulative_injection_m3: f64,
    cumulative_production_m3: f64,
    pub cumulative_mb_error_m3: f64,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::well_control::{ProducerControlState, ResolvedWellControl, WellControlDecision};
    mod buckley;
    mod depletion;
    mod physics;
    mod pvt_properties;
    mod spe1_fim;
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
    fn water_mass_balance_sanity_without_wells_on_fim_branch() {
        let mut sim = ReservoirSimulator::new(4, 4, 1, 0.2);
        sim.set_fim_enabled(true);
        let water_before = total_water_volume(&sim);

        sim.step(1.0);

        let water_after = total_water_volume(&sim);
        assert!((water_after - water_before).abs() < 1e-6);
        assert!((sim.time_days - 1.0).abs() < 1e-12);
        assert_eq!(sim.rate_history.len(), 1);
    }

    #[test]
    fn fim_branch_advances_simple_well_case_with_finite_state() {
        let mut sim = ReservoirSimulator::new(3, 1, 1, 0.2);
        sim.set_fim_enabled(true);
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(2, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        sim.step(0.25);

        assert!((sim.time_days - 0.25).abs() < 1e-12);
        assert_eq!(sim.rate_history.len(), 1);
        assert!(sim.last_solver_warning.is_empty());

        for i in 0..sim.nx * sim.ny * sim.nz {
            assert!(sim.pressure[i].is_finite());
            assert!(sim.sat_water[i].is_finite());
            assert!(sim.sat_oil[i].is_finite());
            assert!(sim.sat_gas[i].is_finite());
        }
    }

    #[test]
    fn adaptive_timestep_produces_multiple_substeps_for_strong_flow() {
        let mut sim = ReservoirSimulator::new(3, 1, 1, 0.2);
        sim.set_fim_enabled(false);
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
    fn default_step_path_reports_rate_controlled_well_state() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
        sim.set_target_well_surface_rates(0.0, 50.0).unwrap();
        sim.set_well_bhp_limits(50.0, 500.0).unwrap();
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

        sim.step(0.25);

        assert!(!sim.rate_history.is_empty());
        let point = sim.rate_history.last().unwrap();
        assert!((point.time - 0.25).abs() < 1e-9);
        assert!(point.total_production_oil.is_finite());
        assert!(point.total_injection.is_finite());
        assert!(point.producer_bhp_limited_fraction.is_finite());
        assert!(point.injector_bhp_limited_fraction.is_finite());
    }

    #[test]
    #[ignore = "known FIM rate-control parity mismatch: public-step mixed-control well rates are now nonzero but still differ materially from IMPES; run explicitly while tuning coupled well behavior"]
    fn rate_control_reporting_benchmark_fim_matches_impes() {
        let impes = run_rate_control_reporting_benchmark(false);
        let fim = run_rate_control_reporting_benchmark(true);

        let oil_rel_diff = ((fim.total_production_oil - impes.total_production_oil)
            / impes.total_production_oil.max(1e-12))
        .abs();
        let injection_abs_diff = (fim.total_injection - impes.total_injection).abs();
        let avg_pressure_rel_diff = ((fim.avg_reservoir_pressure - impes.avg_reservoir_pressure)
            / impes.avg_reservoir_pressure.max(1e-12))
        .abs();

        assert!(fim.total_production_oil.is_finite());
        assert!(fim.total_injection.is_finite());
        assert!(
            oil_rel_diff <= 0.20,
            "rate-control benchmark oil-rate drift too large: IMPES={:.6}, FIM={:.6}, rel_diff={:.4}",
            impes.total_production_oil,
            fim.total_production_oil,
            oil_rel_diff,
        );
        assert!(
            injection_abs_diff <= 1e-9,
            "rate-control benchmark injector-rate drift too large: IMPES={:.6}, FIM={:.6}, abs_diff={:.6}",
            impes.total_injection,
            fim.total_injection,
            injection_abs_diff,
        );
        assert!(
            avg_pressure_rel_diff <= 0.10,
            "rate-control benchmark average-pressure drift too large: IMPES={:.6}, FIM={:.6}, rel_diff={:.4}",
            impes.avg_reservoir_pressure,
            fim.avg_reservoir_pressure,
            avg_pressure_rel_diff,
        );
        assert!(
            (fim.producer_bhp_limited_fraction - impes.producer_bhp_limited_fraction).abs() <= 1e-9,
            "rate-control benchmark producer clamp fraction drift: IMPES={:.3}, FIM={:.3}",
            impes.producer_bhp_limited_fraction,
            fim.producer_bhp_limited_fraction,
        );
        assert!(
            (fim.injector_bhp_limited_fraction - impes.injector_bhp_limited_fraction).abs() <= 1e-9,
            "rate-control benchmark injector clamp fraction drift: IMPES={:.3}, FIM={:.3}",
            impes.injector_bhp_limited_fraction,
            fim.injector_bhp_limited_fraction,
        );
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
        sim.set_fim_enabled(false);
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
            !sim.rate_history.is_empty()
                && (sim.rate_history.last().unwrap().time - 0.5).abs() < 1e-9,
            "Expected the last recorded rate-history time to match the completed step"
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

    // ── Three-phase tests ────────────────────────────────────────────────────

    #[test]
    fn three_phase_relperm_k_ro_stone2_endpoints() {
        use crate::relperm::RockFluidPropsThreePhase;

        let rock = RockFluidPropsThreePhase {
            s_wc: 0.10,
            s_or: 0.10,
            n_w: 2.0,
            n_o: 2.0,
            k_rw_max: 0.8,
            k_ro_max: 0.9,
            s_gc: 0.05,
            s_gr: 0.05,
            s_org: 0.10,
            n_g: 1.5,
            k_rg_max: 0.7,
            tables: None,
        };

        // At connate water with no free gas → k_ro should equal k_ro_max
        let kro_at_swc = rock.k_ro_stone2(rock.s_wc, 0.0);
        assert!(
            (kro_at_swc - rock.k_ro_max).abs() < 1e-10,
            "k_ro_stone2(Swc, 0) should equal k_ro_max ({}) but got {}",
            rock.k_ro_max,
            kro_at_swc
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
                        sw,
                        sg,
                        kro
                    );
                    assert!(
                        kro <= rock.k_ro_max + 1e-10,
                        "k_ro_stone2 exceeds k_ro_max at sw={:.3}, sg={:.3}: {}",
                        sw,
                        sg,
                        kro
                    );
                }
            }
        }
    }

    #[test]
    fn three_phase_relperm_k_rg_endpoints_and_monotonicity() {
        use crate::relperm::RockFluidPropsThreePhase;

        let rock = RockFluidPropsThreePhase {
            s_wc: 0.10,
            s_or: 0.10,
            n_w: 2.0,
            n_o: 2.0,
            k_rw_max: 0.8,
            k_ro_max: 0.9,
            s_gc: 0.05,
            s_gr: 0.05,
            s_org: 0.10,
            n_g: 2.0,
            k_rg_max: 0.7,
            tables: None,
        };

        // Below and at critical gas saturation → k_rg = 0
        assert_eq!(rock.k_rg(0.0), 0.0);
        assert_eq!(rock.k_rg(rock.s_gc * 0.5), 0.0);
        assert!(
            rock.k_rg(rock.s_gc) < 1e-10,
            "k_rg(Sgc) = {}",
            rock.k_rg(rock.s_gc)
        );

        // At maximum mobile gas saturation (Sg = 1 - Swc - Sgr) → k_rg = k_rg_max
        let sg_at_kmax = 1.0 - rock.s_wc - rock.s_gr;
        let krg_at_max = rock.k_rg(sg_at_kmax);
        assert!(
            (krg_at_max - rock.k_rg_max).abs() < 1e-10,
            "k_rg at max gas sat should be k_rg_max ({}) but got {}",
            rock.k_rg_max,
            krg_at_max
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
                sg,
                krg,
                prev_krg
            );
            prev_krg = krg;
        }
    }

    #[test]
    fn three_phase_relperm_stone2_reduces_to_two_phase_at_zero_gas() {
        use crate::relperm::RockFluidPropsThreePhase;

        let rock = RockFluidPropsThreePhase {
            s_wc: 0.10,
            s_or: 0.10,
            n_w: 2.0,
            n_o: 2.0,
            k_rw_max: 0.8,
            k_ro_max: 0.9,
            s_gc: 0.05,
            s_gr: 0.05,
            s_org: 0.10,
            n_g: 1.5,
            k_rg_max: 0.7,
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
                sw,
                kro_stone2,
                kro_ow
            );
        }
    }

    #[test]
    fn three_phase_relperm_tables_interpolate_exact_spe1_points() {
        use crate::relperm::{RockFluidPropsThreePhase, SgofRow, SwofRow, ThreePhaseScalTables};

        let rock = RockFluidPropsThreePhase {
            s_wc: 0.12,
            s_or: 0.12,
            n_w: 2.0,
            n_o: 2.5,
            k_rw_max: 1e-5,
            k_ro_max: 1.0,
            s_gc: 0.04,
            s_gr: 0.04,
            s_org: 0.18,
            n_g: 1.5,
            k_rg_max: 0.984,
            tables: Some(ThreePhaseScalTables {
                swof: vec![
                    SwofRow {
                        sw: 0.12,
                        krw: 0.0,
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
                        sg: 0.5,
                        krg: 0.72,
                        krog: 0.001,
                        pcog: Some(0.0),
                    },
                    SgofRow {
                        sg: 0.88,
                        krg: 0.984,
                        krog: 0.0,
                        pcog: Some(0.0),
                    },
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
                SwofRow {
                    sw: 0.12,
                    krw: 0.0,
                    krow: 1.0,
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
                    sg: 0.88,
                    krg: 0.984,
                    krog: 0.0,
                    pcog: Some(0.0),
                },
            ],
        };

        assert!(tables.validate().is_ok());
    }

    fn make_spe1_like_grid_sim(
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

    fn make_spe1_like_base_sim() -> ReservoirSimulator {
        make_spe1_like_sim(vec![500.0, 50.0, 200.0], 0.05, 50.0, 0.5)
    }


    #[test]
    fn api_contract_rejects_invalid_three_phase_relperm_parameters() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);

        // Zero Corey exponent for water
        err_contains(
            sim.set_three_phase_rel_perm_props(
                0.1, 0.1, 0.05, 0.05, 0.10, 0.0, 2.0, 1.5, 1.0, 1.0, 0.7,
            ),
            "must be positive",
        );

        // Zero Corey exponent for gas
        err_contains(
            sim.set_three_phase_rel_perm_props(
                0.1, 0.1, 0.05, 0.05, 0.10, 2.0, 2.0, 0.0, 1.0, 1.0, 0.7,
            ),
            "must be positive",
        );
    }

    #[test]
    fn api_contract_rejects_invalid_gas_fluid_properties() {
        let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);

        err_contains(
            sim.set_gas_fluid_properties(0.0, 1e-4, 10.0),
            "must be positive",
        );
        err_contains(
            sim.set_gas_fluid_properties(-0.01, 1e-4, 10.0),
            "must be positive",
        );
        err_contains(
            sim.set_gas_fluid_properties(0.02, -1e-4, 10.0),
            "non-negative",
        );
        err_contains(
            sim.set_gas_fluid_properties(0.02, 1e-4, 0.0),
            "must be positive",
        );
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

        assert!(
            (d0 - 3.0).abs() < 1e-10,
            "k=0: depth should be 6/2 = 3, got {}",
            d0
        );
        assert!(
            (d1 - 10.5).abs() < 1e-10,
            "k=1: depth should be 6 + 9/2 = 10.5, got {}",
            d1
        );
        assert!(
            (d2 - 22.5).abs() < 1e-10,
            "k=2: depth should be 6 + 9 + 15/2 = 22.5, got {}",
            d2
        );
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
        assert!(
            (sim.sat_gas[id0] - 0.5).abs() < 1e-10,
            "Sg should clamp to 0.5"
        );
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
        sim.set_permeability_random_seeded(100.0, 100.0, 42)
            .unwrap();

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
            expected,
            t_z
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
