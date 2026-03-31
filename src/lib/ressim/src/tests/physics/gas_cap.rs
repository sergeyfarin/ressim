use super::fixtures::run_hydrostatic_gravity_benchmark;
use crate::ReservoirSimulator;

#[test]
fn physics_gas_cap_vertical_column_builds_hydrostatic_gradient() {
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
fn physics_gas_cap_vertical_column_fim_matches_impes_hydrostatic_benchmark() {
    let impes = run_hydrostatic_gravity_benchmark(false);
    let fim = run_hydrostatic_gravity_benchmark(true);

    let gradient_rel_diff = ((fim.pressure_gradient_bar - impes.pressure_gradient_bar)
        / impes.pressure_gradient_bar.max(1e-12))
    .abs();
    let top_sw_abs_diff = (fim.top_sw_change - impes.top_sw_change).abs();

    assert!(fim.pressure_gradient_bar > 0.0);
    assert!(
        gradient_rel_diff <= 0.05,
        "gravity benchmark pressure-gradient drift too large: IMPES={:.6}, FIM={:.6}, rel_diff={:.4}",
        impes.pressure_gradient_bar,
        fim.pressure_gradient_bar,
        gradient_rel_diff,
    );
    assert!(
        top_sw_abs_diff <= 1e-4,
        "gravity benchmark top-cell Sw drift too large: IMPES={:.6}, FIM={:.6}, abs_diff={:.6}",
        impes.top_sw_change,
        fim.top_sw_change,
        top_sw_abs_diff,
    );
}

#[test]
fn physics_gas_cap_vertical_column_gravity_stays_quieter_than_no_gravity() {
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

    assert!(
        sw_change_top_g <= sw_change_top_no_g + 1e-9,
        "gravity-enabled top-cell saturation drift ({}) exceeded no-gravity drift ({})",
        sw_change_top_g,
        sw_change_top_no_g
    );
}