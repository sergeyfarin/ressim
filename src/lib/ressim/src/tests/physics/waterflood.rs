use super::fixtures::make_short_waterflood_1d_sim;

#[test]
fn physics_waterflood_1d_mass_conservative() {
    let mut sim = make_short_waterflood_1d_sim();
    let initial_avg_sw = sim.sat_water.iter().copied().sum::<f64>() / sim.sat_water.len() as f64;

    for _ in 0..12 {
        sim.step(0.25);
        assert!(
            sim.last_solver_warning.is_empty(),
            "1D waterflood emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    let latest = sim
        .rate_history
        .last()
        .expect("1D waterflood should record history");
    let avg_sw = sim.sat_water.iter().copied().sum::<f64>() / sim.sat_water.len() as f64;
    let injector_cell = sim.idx(0, 0, 0);
    let producer_cell = sim.idx(sim.nx - 1, 0, 0);

    assert!(avg_sw > initial_avg_sw + 1e-4);
    assert!(sim.sat_water[injector_cell] > sim.sat_water[producer_cell]);
    assert!(latest.total_injection > 0.0);
    assert!(latest.total_production_liquid > 0.0);
    assert!(
        latest.material_balance_error_m3 <= 1e-2,
        "1D waterflood water MB drift too large: {}",
        latest.material_balance_error_m3
    );
}