use crate::fim::newton::{FimNewtonOptions, run_fim_timestep};
use crate::fim::state::FimState;
use crate::fim::wells::{
    build_well_topology, connection_rate_for_bhp, perforation_component_rates_sc_day,
    perforation_local_block, well_local_block,
};
use crate::pvt::{PvtRow, PvtTable};
use crate::tests::physics::fixtures::make_closed_depletion_single_cell_sim;

const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;

#[test]
fn peaceman_connection_law_matches_analytical_wi() {
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
    let r_eq = 0.28_f64
        * ((kx / ky).sqrt() * sim.dx.powi(2) + (ky / kx).sqrt() * sim.dy.powi(2)).sqrt()
        / ((kx / ky).powf(0.25_f64) + (ky / kx).powf(0.25_f64));
    let wi_geom = DARCY_METRIC_FACTOR * 2.0 * std::f64::consts::PI * k_avg * sim.dz_at(id)
        / ((r_eq / well.well_radius).ln() + well.skin);

    let kro = sim.scal.k_ro(sw);
    let mu_o = sim.get_mu_o_for_rs(pressure_bar, sim.rs[id]).max(1e-9);
    let bo = sim.get_b_o_cell(id, pressure_bar).max(1e-9);
    let expected_oil_sc_day = wi_geom * (pressure_bar - bhp_bar) * kro / mu_o / bo;

    let component_rates = perforation_component_rates_sc_day(&sim, &state, &topology, perf_idx);

    assert!(component_rates[1] > 0.0);
    assert!(component_rates[0] >= 0.0);
    assert!(component_rates[2].abs() < 1e-12);
    assert!(
        (component_rates[1] - expected_oil_sc_day).abs() / expected_oil_sc_day.max(1.0) < 1e-9,
        "oil component rate should match WI * dp * kro / (mu_o * Bo); got {}, expected {}",
        component_rates[1],
        expected_oil_sc_day
    );
}

#[test]
fn single_cell_producer_reporting_matches_local_source_state() {
    let mut sim = make_closed_depletion_single_cell_sim();
    let previous_state = FimState::from_simulator(&sim);
    let report = run_fim_timestep(
        &mut sim,
        &previous_state,
        &previous_state,
        0.1,
        &FimNewtonOptions::default(),
    );
    assert!(report.converged);

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
fn rate_controlled_injector_fim_path_converges() {
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

    assert!(report.converged);

    let topology = build_well_topology(&sim);
    let well_block = well_local_block(&topology, &report.accepted_state, 0);
    let perf = perforation_local_block(&topology, &report.accepted_state, 0);
    let well_residual = well_block.constraint_residual(&sim)
        .expect("well constraint residual should exist for rate-controlled injector");
    let perf_residual = perf
        .rate_residual(&sim)
        .expect("perforation residual should exist for rate-controlled injector");
    let diagnostics = perf
        .residual_diagnostics(&sim)
        .expect("injector residual diagnostics should exist");
    let component_rates =
        perforation_component_rates_sc_day(&sim, &report.accepted_state, &topology, 0);

    let target_surface_rate = diagnostics.target_rate_sc_day.expect("target rate missing");
    let actual_surface_rate = diagnostics
        .actual_well_rate_sc_day
        .expect("actual well rate missing");
    let relative_rate_error =
        (actual_surface_rate - target_surface_rate).abs() / target_surface_rate.max(1.0);

    assert!(well_residual.abs() < 1e-6);
    assert!(perf_residual.abs() < 1e-6);
    assert!(diagnostics.enabled);
    assert!(diagnostics.injector);
    assert!(diagnostics.bhp_slack.expect("injector bhp slack missing") > 0.0);
    assert!(
        diagnostics
            .rate_slack
            .expect("injector rate slack missing")
            .abs()
            < 1e-4
    );
    assert!(component_rates[2] < 0.0);
    assert!(relative_rate_error < 0.05);

    sim.record_fim_step_report(&report.accepted_state, 0.1, 0.0, 0.0, 0.0);
    let latest = sim
        .rate_history
        .last()
        .expect("rate-controlled injector FIM case should record history");

    assert!(latest.total_injection > 0.0);
    assert!((latest.total_production_oil).abs() < 1e-12);
    assert!((latest.total_production_gas).abs() < 1e-12);
    assert!(
        (latest.total_injection - target_surface_rate).abs() / target_surface_rate.max(1.0) < 0.05
    );
    assert_eq!(latest.injector_bhp_limited_fraction, 0.0);
}

#[test]
fn rate_controlled_producer_fim_hits_bhp_limit() {
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

    assert!(report.converged);

    let topology = build_well_topology(&sim);
    let well_block = well_local_block(&topology, &report.accepted_state, 0);
    let perf = perforation_local_block(&topology, &report.accepted_state, 0);
    let well_residual = well_block.constraint_residual(&sim)
        .expect("well constraint residual should exist for BHP-limited producer");
    let perf_residual = perf
        .rate_residual(&sim)
        .expect("perforation residual should exist for BHP-limited producer");
    let diagnostics = perf
        .residual_diagnostics(&sim)
        .expect("BHP-limited producer diagnostics should exist");
    let component_rates =
        perforation_component_rates_sc_day(&sim, &report.accepted_state, &topology, 0);

    let bhp_slack = diagnostics.bhp_slack.expect("BHP slack missing");
    let rate_slack = diagnostics.rate_slack.expect("rate slack missing");
    let target_rate = diagnostics.target_rate_sc_day.expect("target rate missing");
    let actual_rate = diagnostics
        .actual_well_rate_sc_day
        .expect("actual rate missing");

    assert!(well_residual.abs() < 1e-6);
    assert!(perf_residual.abs() < 2e-3);
    assert!(diagnostics.enabled);
    assert!(!diagnostics.injector);
    assert!(bhp_slack.abs() < 1e-6);
    assert!(rate_slack > 1e-3);
    assert!((diagnostics.bhp_bar - 80.0).abs() < 1e-6);
    assert_eq!(diagnostics.frozen_consistent_bhp_limited, Some(true));
    assert!(actual_rate < target_rate);
    assert!(component_rates[1] > 0.0);
    assert!(component_rates[2].abs() < 1e-12);

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
fn multi_layer_well_shares_bhp_and_splits_rate_by_mobility() {
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
    let well_block = well_local_block(&topology, &state, 0);
    let well_residual = well_block.constraint_residual(&sim)
        .expect("shared physical well should expose a well constraint residual");
    assert!(well_residual.abs() < 1e-6);

    let bhp_bar = state.well_bhp[0];
    let perf0 = topology.wells[0].perforation_indices[0];
    let perf1 = topology.wells[0].perforation_indices[1];

    let diagnostics0 = perforation_local_block(&topology, &state, perf0)
        .residual_diagnostics(&sim)
        .expect("first perforation diagnostics should exist");
    let diagnostics1 = perforation_local_block(&topology, &state, perf1)
        .residual_diagnostics(&sim)
        .expect("second perforation diagnostics should exist");

    assert!((diagnostics0.bhp_bar - bhp_bar).abs() < 1e-10);
    assert!((diagnostics1.bhp_bar - bhp_bar).abs() < 1e-10);

    let q0 = connection_rate_for_bhp(&sim, &state, &topology, perf0, bhp_bar)
        .expect("first perforation connection rate should exist");
    let q1 = connection_rate_for_bhp(&sim, &state, &topology, perf1, bhp_bar)
        .expect("second perforation connection rate should exist");

    let perf_residual0 = perforation_local_block(&topology, &state, perf0)
        .rate_residual(&sim)
        .expect("first perforation residual should exist");
    let perf_residual1 = perforation_local_block(&topology, &state, perf1)
        .rate_residual(&sim)
        .expect("second perforation residual should exist");

    assert!(perf_residual0.abs() < 1e-6);
    assert!(perf_residual1.abs() < 1e-6);
    assert!((state.perforation_rates_m3_day[perf0] - q0).abs() < 1e-8);
    assert!((state.perforation_rates_m3_day[perf1] - q1).abs() < 1e-8);
    assert!((q0 - q1).abs() > 1e-6);

    let total_connection_rate = q0 + q1;
    let total_perforation_rate =
        state.perforation_rates_m3_day[perf0] + state.perforation_rates_m3_day[perf1];

    assert!((total_perforation_rate - total_connection_rate).abs() < 1e-8);
    assert!(
        (diagnostics0
            .actual_well_rate_sc_day
            .expect("first actual well rate")
            - total_connection_rate)
            .abs()
            < 1e-8
    );
    assert!(
        (diagnostics1
            .actual_well_rate_sc_day
            .expect("second actual well rate")
            - total_connection_rate)
            .abs()
            < 1e-8
    );
}
