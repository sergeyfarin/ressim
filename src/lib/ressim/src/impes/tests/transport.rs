use nalgebra::DVector;

use crate::ReservoirSimulator;
use crate::well_control::{ProducerControlState, ResolvedWellControl, WellControlDecision};

#[test]
fn transport_reporting_reuses_rate_control_decision() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_three_phase_rel_perm_props(0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7)
        .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_injected_fluid("gas").unwrap();
    sim.set_initial_pressure(100.0);
    sim.set_initial_saturation(0.10);
    sim.pvt_table = Some(crate::pvt::PvtTable::new(
        vec![crate::pvt::PvtRow {
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
    assert!((latest.total_injection - 360.0).abs() < 1e-6);
    assert_eq!(latest.injector_bhp_limited_fraction, 0.0);
}

#[test]
fn producing_gor_is_zero_when_oil_rate_is_negligible() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_three_phase_rel_perm_props(0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7)
        .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_initial_pressure(100.0);
    sim.set_initial_saturation(0.10);
    sim.pvt_table = Some(crate::pvt::PvtTable::new(
        vec![crate::pvt::PvtRow {
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

#[test]
fn producer_reporting_uses_same_sampled_near_well_mixture() {
    let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
    sim.set_initial_pressure(200.0);
    sim.set_initial_saturation(0.12);
    sim.set_three_phase_rel_perm_props(
        0.12, 0.12, 0.04, 0.04, 0.18, 2.0, 2.5, 1.5, 1e-5, 1.0, 0.984,
    )
    .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
    sim.add_well(1, 1, 0, 100.0, 0.1, 0.0, false).unwrap();

    let producer_id = sim.idx(1, 1, 0);
    let left_id = sim.idx(0, 1, 0);
    let down_id = sim.idx(1, 0, 0);
    let diag_id = sim.idx(0, 0, 0);

    sim.sat_water = vec![0.12; 4];
    sim.sat_gas[producer_id] = 0.25;
    sim.sat_oil[producer_id] = 1.0 - sim.sat_water[producer_id] - sim.sat_gas[producer_id];
    for id in [left_id, down_id, diag_id] {
        sim.sat_gas[id] = 0.0;
        sim.sat_oil[id] = 1.0 - sim.sat_water[id];
    }

    let (sample_fw, sample_fo, sample_fg) =
        sim.producer_control_phase_fractions_for_pressures(&sim.wells[0], &sim.pressure);
    let cached_state = ProducerControlState {
        water_fraction: sample_fw,
        oil_fraction: sample_fo,
        gas_fraction: sample_fg,
        oil_fvf: sim
            .get_b_o_cell(producer_id, sim.pressure[producer_id])
            .max(1e-9),
        gas_fvf: sim.get_b_g(sim.pressure[producer_id]).max(1e-9),
        rs_sm3_sm3: sim.rs[producer_id],
    };

    let controls = vec![Some(ResolvedWellControl {
        decision: WellControlDecision::Rate { q_m3_day: 1000.0 },
        bhp_limited: false,
        producer_state: Some(cached_state),
    })];

    sim.update_saturations_and_pressure(
        &DVector::from_vec(vec![150.0, 150.0, 150.0, 150.0]),
        &vec![0.0, 0.0, 0.0, 0.0],
        &vec![0.0, 0.0, 0.0, 0.0],
        &vec![0.0, 0.0, 0.0, 0.0],
        &controls,
        1.0,
    );

    let latest = sim
        .rate_history
        .last()
        .expect("rate history should have an entry");
    let expected_oil_sc = 1000.0 * cached_state.oil_fraction / cached_state.oil_fvf;
    let expected_total_gas_sc = 1000.0 * cached_state.gas_fraction / cached_state.gas_fvf
        + expected_oil_sc * cached_state.rs_sm3_sm3;

    assert!((latest.total_production_oil - expected_oil_sc).abs() < 1e-9);
    assert!((latest.total_production_gas - expected_total_gas_sc).abs() < 1e-9);
    assert!((latest.total_production_liquid_reservoir - 1000.0).abs() < 1e-9);
    assert!(sample_fo > 0.0 && sample_fo < 1.0);
    assert!(sample_fg > 0.0);
    assert!(sample_fw >= 0.0);
}
