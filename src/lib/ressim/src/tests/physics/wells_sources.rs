use nalgebra::DVector;

use crate::fim::newton::{FimNewtonOptions, run_fim_timestep};
use crate::fim::state::FimState;
use crate::fim::wells::{
    build_well_topology, connection_rate_for_bhp, perforation_component_rates_sc_day,
    perforation_rate_residual, perforation_residual_diagnostics, well_constraint_residual,
};
use crate::pvt::{PvtRow, PvtTable};
use crate::well_control::{ProducerControlState, ResolvedWellControl, WellControlDecision};

use super::fixtures::make_closed_depletion_single_cell_sim;

const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;

#[test]
fn physics_wells_sources_peaceman_connection_law_matches_analytical_wi() {
    let sim = make_closed_depletion_single_cell_sim();
    let state = FimState::from_simulator(&sim);
    let topology = build_well_topology(&sim);
    let perf_idx = topology.wells[0].perforation_indices[0];
    let perf = &topology.perforations[perf_idx];
    let id = perf.cell_index;
    let well = &sim.wells[perf.well_entry_index];
    let bhp_bar = state.well_bhp[perf.physical_well_index];
    let pressure_bar = state.cell(id).pressure_bar;
    let sw = state.cell(id).sw;

    let kx = sim.perm_x[id];
    let ky = sim.perm_y[id];
    let k_avg = (kx * ky).sqrt();
    let r_eq = 0.28
        * ((kx / ky).sqrt() * sim.dx.powi(2) + (ky / kx).sqrt() * sim.dy.powi(2)).sqrt()
        / ((kx / ky).powf(0.25) + (ky / kx).powf(0.25));
    let wi_geom = DARCY_METRIC_FACTOR * 2.0 * std::f64::consts::PI * k_avg * sim.dz_at(id)
        / ((r_eq / well.well_radius).ln() + well.skin);

    let kro = sim.scal.k_ro(sw);
    let mu_o = sim.get_mu_o_for_rs(pressure_bar, sim.rs[id]).max(1e-9);
    let bo = sim.get_b_o_cell(id, pressure_bar).max(1e-9);
    let expected_oil_sc_day = wi_geom * (pressure_bar - bhp_bar) * kro / mu_o / bo;

    let component_rates = perforation_component_rates_sc_day(&sim, &state, &topology, perf_idx);

    assert!(
        component_rates[1] > 0.0,
        "Peaceman oracle should exercise a positive oil producer rate"
    );
    assert!(
        component_rates[0] >= 0.0,
        "water rate should stay non-negative for the producer oracle"
    );
    assert!(
        component_rates[2].abs() < 1e-12,
        "two-phase depletion oracle should have zero gas production, got {}",
        component_rates[2]
    );
    assert!(
        (component_rates[1] - expected_oil_sc_day).abs() / expected_oil_sc_day.max(1.0) < 1e-9,
        "oil component rate should match WI * dp * kro / (mu_o * Bo); got {}, expected {}",
        component_rates[1],
        expected_oil_sc_day
    );
}

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
    assert!(
        report.converged,
        "single-cell producer consistency case should converge"
    );

    let topology = build_well_topology(&sim);
    let perf_idx = topology.wells[0].perforation_indices[0];
    let component_rates =
        perforation_component_rates_sc_day(&sim, &report.accepted_state, &topology, perf_idx);

    sim.record_fim_step_report(&report.accepted_state, 0.1, 0.0, 0.0, 0.0);
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
    sim.set_three_phase_rel_perm_props(0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7)
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
fn physics_wells_sources_rate_controlled_injector_fim_path_converges() {
    let mut sim = crate::ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_three_phase_rel_perm_props(0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7)
        .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_injected_fluid("gas").unwrap();
    sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
    sim.set_initial_pressure(100.0);
    sim.set_initial_saturation(0.10);
    sim.set_gravity_enabled(false);
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
    sim.add_well(0, 0, 0, 250.0, 0.1, 0.0, true).unwrap();

    let previous_state = FimState::from_simulator(&sim);
    let report = run_fim_timestep(
        &mut sim,
        &previous_state,
        &previous_state,
        0.1,
        &FimNewtonOptions::default(),
    );

    assert!(
        report.converged,
        "rate-controlled injector FIM case should converge"
    );

    let topology = build_well_topology(&sim);
    assert_eq!(topology.wells.len(), 1);
    assert_eq!(topology.perforations.len(), 1);

    let well_residual = well_constraint_residual(&sim, &report.accepted_state, &topology, 0)
        .expect("well constraint residual should exist for rate-controlled injector");
    let perf_residual = perforation_rate_residual(&sim, &report.accepted_state, &topology, 0)
        .expect("perforation residual should exist for rate-controlled injector");
    let diagnostics = perforation_residual_diagnostics(&sim, &report.accepted_state, &topology, 0)
        .expect("injector residual diagnostics should exist");
    let component_rates =
        perforation_component_rates_sc_day(&sim, &report.accepted_state, &topology, 0);

    let target_surface_rate = diagnostics
        .target_rate_sc_day
        .expect("rate-controlled injector should expose a target rate");
    let actual_surface_rate = diagnostics
        .actual_well_rate_sc_day
        .expect("rate-controlled injector should expose the actual well rate");
    let relative_rate_error =
        (actual_surface_rate - target_surface_rate).abs() / target_surface_rate.max(1.0);

    assert!(
        well_residual.abs() < 1e-6,
        "well constraint residual should be near zero, got {}",
        well_residual
    );
    assert!(
        perf_residual.abs() < 1e-6,
        "perforation residual should be near zero, got {}",
        perf_residual
    );
    assert!(diagnostics.enabled);
    assert!(diagnostics.injector);
    assert!(
        diagnostics
            .bhp_slack
            .expect("injector should report BHP slack")
            > 0.0
    );
    assert!(
        diagnostics
            .rate_slack
            .expect("injector should report rate slack")
            .abs()
            < 1e-4,
        "rate slack should be near zero, got {:?} with actual={} target={} bhp={} bhp_slack={:?}",
        diagnostics.rate_slack,
        actual_surface_rate,
        target_surface_rate,
        diagnostics.bhp_bar,
        diagnostics.bhp_slack
    );
    assert!(
        component_rates[2] < 0.0,
        "gas injector should add negative source to the residual sign convention"
    );
    assert!(
        relative_rate_error < 0.05,
        "injector surface rate should stay within 5% of target, got actual={} target={}",
        actual_surface_rate,
        target_surface_rate
    );

    sim.record_fim_step_report(&report.accepted_state, 0.1, 0.0, 0.0, 0.0);
    let latest = sim
        .rate_history
        .last()
        .expect("rate-controlled injector FIM case should record history");

    assert!(latest.total_injection > 0.0);
    assert!((latest.total_production_oil).abs() < 1e-12);
    assert!((latest.total_production_gas).abs() < 1e-12);
    assert!(
        (latest.total_injection - target_surface_rate).abs() / target_surface_rate.max(1.0) < 0.05,
        "recorded injector surface total should stay within 5% of target, got {} vs {}",
        latest.total_injection,
        target_surface_rate
    );
    assert_eq!(latest.injector_bhp_limited_fraction, 0.0);
}

#[test]
fn physics_wells_sources_rate_controlled_producer_fim_hits_bhp_limit() {
    let mut sim = make_closed_depletion_single_cell_sim();
    sim.set_rate_controlled_wells(true);
    sim.set_target_well_rates(0.0, 1.0e6).unwrap();
    sim.set_well_bhp_limits(80.0, 1.0e9).unwrap();

    let previous_state = FimState::from_simulator(&sim);
    let report = run_fim_timestep(
        &mut sim,
        &previous_state,
        &previous_state,
        0.1,
        &FimNewtonOptions::default(),
    );

    assert!(
        report.converged,
        "BHP-limited producer FIM case should converge"
    );

    let topology = build_well_topology(&sim);
    assert_eq!(topology.wells.len(), 1);
    assert_eq!(topology.perforations.len(), 1);

    let well_residual = well_constraint_residual(&sim, &report.accepted_state, &topology, 0)
        .expect("well constraint residual should exist for BHP-limited producer");
    let perf_residual = perforation_rate_residual(&sim, &report.accepted_state, &topology, 0)
        .expect("perforation residual should exist for BHP-limited producer");
    let diagnostics = perforation_residual_diagnostics(&sim, &report.accepted_state, &topology, 0)
        .expect("BHP-limited producer diagnostics should exist");
    let component_rates =
        perforation_component_rates_sc_day(&sim, &report.accepted_state, &topology, 0);

    let bhp_slack = diagnostics
        .bhp_slack
        .expect("BHP-limited producer should report BHP slack");
    let rate_slack = diagnostics
        .rate_slack
        .expect("BHP-limited producer should report rate slack");
    let target_rate = diagnostics
        .target_rate_sc_day
        .expect("rate-controlled producer should expose a target rate");
    let actual_rate = diagnostics
        .actual_well_rate_sc_day
        .expect("rate-controlled producer should expose the actual well rate");

    assert!(
        well_residual.abs() < 1e-6,
        "well constraint residual should be near zero at the BHP-limited branch, got {}",
        well_residual
    );
    assert!(
        perf_residual.abs() < 2e-3,
        "perforation residual should be near zero at the BHP-limited branch, got {}",
        perf_residual
    );
    assert!(diagnostics.enabled);
    assert!(!diagnostics.injector);
    assert!(
        bhp_slack.abs() < 1e-6,
        "BHP slack should collapse to zero at the active limit, got {}",
        bhp_slack
    );
    assert!(
        rate_slack > 1e-3,
        "rate slack should stay positive when the producer is clamped by BHP, got {}",
        rate_slack
    );
    assert!(
        (diagnostics.bhp_bar - 80.0).abs() < 1e-6,
        "accepted producer BHP should sit at the lower limit, got {}",
        diagnostics.bhp_bar
    );
    assert_eq!(diagnostics.frozen_consistent_bhp_limited, Some(true));
    assert!(
        actual_rate < target_rate,
        "actual producer rate should stay below the infeasible target when clamped by BHP: actual={} target={}",
        actual_rate,
        target_rate
    );
    assert!(
        component_rates[1] > 0.0,
        "BHP-limited producer should still produce oil"
    );
    assert!(
        component_rates[2].abs() < 1e-12,
        "two-phase BHP-limited producer should have zero gas production, got {}",
        component_rates[2]
    );

    sim.record_fim_step_report(&report.accepted_state, 0.1, 0.0, 0.0, 0.0);
    let latest = sim
        .rate_history
        .last()
        .expect("BHP-limited producer FIM case should record history");

    assert!(latest.total_production_oil > 0.0);
    assert_eq!(latest.producer_bhp_limited_fraction, 1.0);
    assert_eq!(latest.injector_bhp_limited_fraction, 0.0);
}

#[test]
fn physics_wells_sources_multi_layer_well_shares_bhp_and_splits_rate_by_mobility() {
    let mut sim = crate::ReservoirSimulator::new(1, 1, 2, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(20.0, 20.0, vec![8.0, 12.0])
        .unwrap();
    sim.set_rel_perm_props(0.10, 0.10, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_rock_properties(1e-6, 0.2, 1.0, 1.0).unwrap();
    sim.set_initial_pressure(250.0);
    sim.set_initial_saturation_per_layer(vec![0.15, 0.80])
        .unwrap();
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_permeability_per_layer(vec![80.0, 80.0], vec![80.0, 80.0], vec![25.0, 150.0])
        .unwrap();
    sim.set_rate_controlled_wells(true);
    sim.set_target_well_rates(0.0, 35.0).unwrap();
    sim.set_well_bhp_limits(0.0, 1.0e9).unwrap();
    sim.add_well_with_id(
        0,
        0,
        0,
        120.0,
        0.1,
        0.0,
        false,
        "dual-layer-prod".to_string(),
    )
    .unwrap();
    sim.add_well_with_id(
        0,
        0,
        1,
        120.0,
        0.1,
        0.0,
        false,
        "dual-layer-prod".to_string(),
    )
    .unwrap();

    let state = FimState::from_simulator(&sim);
    let topology = build_well_topology(&sim);
    assert_eq!(topology.wells.len(), 1);
    assert_eq!(topology.perforations.len(), 2);
    assert_eq!(state.well_bhp.len(), 1);

    let well_residual = well_constraint_residual(&sim, &state, &topology, 0)
        .expect("shared physical well should expose a well constraint residual");
    assert!(well_residual.abs() < 1e-6);

    let bhp_bar = state.well_bhp[0];
    let perf0 = topology.wells[0].perforation_indices[0];
    let perf1 = topology.wells[0].perforation_indices[1];

    let diagnostics0 = perforation_residual_diagnostics(&sim, &state, &topology, perf0)
        .expect("first perforation diagnostics should exist");
    let diagnostics1 = perforation_residual_diagnostics(&sim, &state, &topology, perf1)
        .expect("second perforation diagnostics should exist");

    assert!((diagnostics0.bhp_bar - bhp_bar).abs() < 1e-10);
    assert!((diagnostics1.bhp_bar - bhp_bar).abs() < 1e-10);
    assert!((diagnostics0.bhp_bar - diagnostics1.bhp_bar).abs() < 1e-12);

    let q0 = connection_rate_for_bhp(&sim, &state, &topology, perf0, bhp_bar)
        .expect("first perforation connection rate should exist");
    let q1 = connection_rate_for_bhp(&sim, &state, &topology, perf1, bhp_bar)
        .expect("second perforation connection rate should exist");

    let perf_residual0 = perforation_rate_residual(&sim, &state, &topology, perf0)
        .expect("first perforation residual should exist");
    let perf_residual1 = perforation_rate_residual(&sim, &state, &topology, perf1)
        .expect("second perforation residual should exist");

    assert!(perf_residual0.abs() < 1e-6);
    assert!(perf_residual1.abs() < 1e-6);
    assert!((state.perforation_rates_m3_day[perf0] - q0).abs() < 1e-8);
    assert!((state.perforation_rates_m3_day[perf1] - q1).abs() < 1e-8);
    assert!(
        (q0 - q1).abs() > 1e-6,
        "layered completions should split rate unevenly"
    );

    let total_connection_rate = q0 + q1;
    let total_perforation_rate =
        state.perforation_rates_m3_day[perf0] + state.perforation_rates_m3_day[perf1];

    assert!((total_perforation_rate - total_connection_rate).abs() < 1e-8);
    assert!(
        (diagnostics0
            .actual_well_rate_sc_day
            .expect("first perforation should report actual well rate")
            - total_connection_rate)
            .abs()
            < 1e-8
    );
    assert!(
        (diagnostics1
            .actual_well_rate_sc_day
            .expect("second perforation should report actual well rate")
            - total_connection_rate)
            .abs()
            < 1e-8
    );
}

#[test]
fn physics_wells_sources_gas_injection_surface_totals_use_bg_conversion() {
    let mut sim = crate::ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_fim_enabled(false);
    sim.set_three_phase_rel_perm_props(0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7)
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
    sim.set_three_phase_rel_perm_props(0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7)
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
