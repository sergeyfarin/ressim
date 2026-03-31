use nalgebra::DVector;

use crate::fim::newton::{FimNewtonOptions, run_fim_timestep};
use crate::fim::state::FimState;
use crate::fim::wells::{build_well_topology, perforation_component_rates_sc_day};
use crate::pvt::{PvtRow, PvtTable};
use crate::well_control::{ProducerControlState, ResolvedWellControl, WellControlDecision};

use super::fixtures::make_closed_depletion_single_cell_sim;

#[test]
fn physics_wells_sources_single_cell_producer_reporting_matches_local_source_state() {
    let mut sim = make_closed_depletion_single_cell_sim();
    let previous_state = FimState::from_simulator(&sim);
    let report = run_fim_timestep(
        &mut sim,
        &previous_state,
        &previous_state,
        0.1,
        &FimNewtonOptions::default(),
    );
    assert!(report.converged, "single-cell producer consistency case should converge");

    let topology = build_well_topology(&sim);
    let perf_idx = topology.wells[0].perforation_indices[0];
    let component_rates =
        perforation_component_rates_sc_day(&sim, &report.accepted_state, &topology, perf_idx);

    sim.record_fim_step_report(&report.accepted_state, 0.1, 0.0, 0.0);
    let latest = sim
        .rate_history
        .last()
        .expect("single-cell producer consistency case should record history");

    assert_eq!(topology.wells.len(), 1);
    assert_eq!(topology.perforations.len(), 1);
    assert!((latest.total_injection).abs() < 1e-12);
    assert!((latest.total_injection_reservoir).abs() < 1e-12);
    assert!((latest.total_production_gas - component_rates[2]).abs() < 1e-10);
    assert!((latest.total_production_oil - component_rates[1]).abs() < 1e-10);
    assert!(
        (latest.total_production_liquid - (component_rates[0] + component_rates[1])).abs() < 1e-10
    );
    assert!(
        (latest.total_production_liquid_reservoir
            - report.accepted_state.perforation_rates_m3_day[perf_idx])
            .abs()
            < 1e-10
    );
}

#[test]
fn physics_wells_sources_transport_reporting_reuses_rate_control_decision() {
    let mut sim = crate::ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_three_phase_rel_perm_props(
        0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7,
    )
    .unwrap();
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
        producer_state: None,
    })];

    sim.update_saturations_and_pressure(
        &DVector::from_vec(vec![300.0]),
        &vec![0.0],
        &vec![0.0],
        &vec![0.0],
        &controls,
        1.0,
    );

    let latest = sim
        .rate_history
        .last()
        .expect("rate history should have an entry");
    assert!(
        (latest.total_injection - 360.0).abs() < 1e-6,
        "reporting should reuse the transport rate-control decision, got {}",
        latest.total_injection,
    );
    assert_eq!(latest.injector_bhp_limited_fraction, 0.0);
}

#[test]
fn physics_wells_sources_gas_injection_surface_totals_use_bg_conversion() {
    let mut sim = crate::ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_fim_enabled(false);
    sim.set_three_phase_rel_perm_props(
        0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7,
    )
    .unwrap();
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

    let latest = sim
        .rate_history
        .last()
        .expect("rate history should have an entry");
    assert!(
        latest.total_injection > 100.0 && latest.total_injection < 200.0,
        "expected gas injector surface total near target surface rate, got {}",
        latest.total_injection
    );
}

#[test]
fn physics_wells_sources_producing_gor_is_zero_when_oil_rate_is_negligible() {
    let mut sim = crate::ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_three_phase_rel_perm_props(
        0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7,
    )
    .unwrap();
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
        producer_state: Some(ProducerControlState {
            water_fraction: 0.12,
            oil_fraction: 0.000_001,
            gas_fraction: 0.879_999,
            oil_fvf: sim.get_b_o_cell(id, sim.pressure[id]).max(1e-9),
            gas_fvf: sim.get_b_g(sim.pressure[id]).max(1e-9),
            rs_sm3_sm3: sim.rs[id],
        }),
    })];

    sim.update_saturations_and_pressure(
        &DVector::from_vec(vec![300.0]),
        &vec![0.0],
        &vec![0.0],
        &vec![0.0],
        &controls,
        1.0,
    );

    let latest = sim
        .rate_history
        .last()
        .expect("rate history should have an entry");
    assert_eq!(latest.producing_gor, 0.0);
}