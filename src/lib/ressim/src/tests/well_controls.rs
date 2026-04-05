use super::*;

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
fn producer_surface_rate_target_uses_well_cell_only_sampling() {
    // Fractional-flow sampling uses only the well cell (not a neighborhood average).
    // Neighboring cells that are oil-rich should not dilute the gas signal at the well.
    let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
    sim.set_initial_pressure(200.0);
    sim.set_initial_saturation(0.12);
    sim.set_three_phase_rel_perm_props(
        0.12, 0.12, 0.04, 0.04, 0.18, 2.0, 2.5, 1.5, 1e-5, 1.0, 0.984,
    )
    .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
    sim.set_target_well_surface_rates(0.0, 100.0).unwrap();
    sim.add_well(1, 1, 0, 100.0, 0.1, 0.0, false).unwrap();

    let producer_id = sim.idx(1, 1, 0);
    let left_id = sim.idx(0, 1, 0);
    let down_id = sim.idx(1, 0, 0);
    let diag_id = sim.idx(0, 0, 0);

    sim.sat_water = vec![0.12; 4];
    // Well cell has gas breakthrough; neighbors remain oil-rich.
    sim.sat_gas[producer_id] = 0.25;
    sim.sat_oil[producer_id] = 1.0 - sim.sat_water[producer_id] - sim.sat_gas[producer_id];
    for id in [left_id, down_id, diag_id] {
        sim.sat_gas[id] = 0.0;
        sim.sat_oil[id] = 1.0 - sim.sat_water[id];
    }

    let well = sim.wells.first().unwrap();
    let q_target = sim.target_rate_m3_day(well, 200.0).unwrap();

    // Expected: well-cell-only mobilities.
    let local_scal = sim.scal_3p.as_ref().unwrap();
    let local_lam_w = local_scal.k_rw(sim.sat_water[producer_id]) / sim.get_mu_w(200.0);
    let local_lam_o = local_scal.k_ro_stone2(sim.sat_water[producer_id], sim.sat_gas[producer_id])
        / sim.get_mu_o_cell(producer_id, 200.0);
    let local_lam_g = local_scal.k_rg(sim.sat_gas[producer_id]) / sim.get_mu_g(200.0);
    let local_oil_fraction = local_lam_o / (local_lam_w + local_lam_o + local_lam_g);
    let expected = 100.0 * sim.get_b_o_cell(producer_id, 200.0) / local_oil_fraction;

    assert!(
        (q_target - expected).abs() < 1e-9,
        "q_target={q_target} expected={expected}"
    );
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

#[test]
fn explicit_per_well_schedule_overrides_family_control_modes() {
    let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.add_well_with_id(0, 0, 0, 90.0, 0.1, 0.0, false, "prod-a".to_string())
        .unwrap();
    sim.add_well_with_id(1, 0, 0, 110.0, 0.1, 0.0, false, "prod-b".to_string())
        .unwrap();
    sim.set_well_schedule(
        "prod-a".to_string(),
        "rate".to_string(),
        10.0,
        f64::NAN,
        60.0,
        true,
    )
    .unwrap();

    let control_a = sim
        .resolve_well_control_for_pressures(&sim.wells[0], &sim.pressure)
        .unwrap();
    let control_b = sim
        .resolve_well_control_for_pressures(&sim.wells[1], &sim.pressure)
        .unwrap();

    assert!(!control_a.bhp_limited);
    assert!(!control_b.bhp_limited);
    match control_a.decision {
        WellControlDecision::Rate { q_m3_day } => assert!(q_m3_day > 0.0),
        _ => panic!("producer A should remain rate-controlled"),
    }
    match control_b.decision {
        WellControlDecision::Bhp { bhp_bar } => assert!((bhp_bar - 110.0).abs() < 1e-9),
        _ => panic!("producer B should remain pressure-controlled"),
    }
}

#[test]
fn reporting_counts_only_rate_controlled_physical_wells() {
    let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
    sim.set_initial_pressure(200.0);
    sim.set_initial_saturation(0.2);
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.add_well_with_id(0, 0, 0, 150.0, 0.1, 0.0, false, "prod-a".to_string())
        .unwrap();
    sim.add_well_with_id(1, 0, 0, 110.0, 0.1, 0.0, false, "prod-b".to_string())
        .unwrap();
    sim.set_well_schedule(
        "prod-a".to_string(),
        "rate".to_string(),
        5000.0,
        f64::NAN,
        150.0,
        true,
    )
    .unwrap();

    sim.step(1.0);

    let point = sim.rate_history.last().unwrap();
    assert_eq!(point.producer_bhp_limited_fraction, 1.0);
    assert_eq!(point.injector_bhp_limited_fraction, 0.0);
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
