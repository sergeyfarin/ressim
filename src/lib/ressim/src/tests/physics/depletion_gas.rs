use super::fixtures::make_closed_gas_depletion_single_cell_sim;

#[test]
fn physics_depletion_gas_single_cell_timestep_stable() {
    let mut coarse = make_closed_gas_depletion_single_cell_sim();
    coarse.step(0.01);
    assert!(
        coarse.last_solver_warning.is_empty(),
        "single-cell coarse gas depletion emitted solver warning: {}",
        coarse.last_solver_warning
    );
    let coarse_point = coarse
        .rate_history
        .last()
        .expect("single-cell coarse gas depletion should record history");

    let mut fine = make_closed_gas_depletion_single_cell_sim();
    fine.step(0.005);
    fine.step(0.005);
    assert!(
        fine.last_solver_warning.is_empty(),
        "single-cell fine gas depletion emitted solver warning: {}",
        fine.last_solver_warning
    );
    let fine_point = fine
        .rate_history
        .last()
        .expect("single-cell fine gas depletion should record history");

    let gas_rate_rel_diff = ((coarse_point.total_production_gas - fine_point.total_production_gas)
        / fine_point.total_production_gas.max(1e-12))
    .abs();
    let avg_pressure_rel_diff = ((coarse_point.avg_reservoir_pressure
        - fine_point.avg_reservoir_pressure)
        / fine_point.avg_reservoir_pressure.max(1e-12))
    .abs();

    assert!(
        gas_rate_rel_diff <= 0.05,
        "single-cell gas depletion timestep gas-rate drift too large: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_point.total_production_gas,
        fine_point.total_production_gas,
        gas_rate_rel_diff,
    );
    assert!(
        avg_pressure_rel_diff <= 0.01,
        "single-cell gas depletion timestep pressure drift too large: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_point.avg_reservoir_pressure,
        fine_point.avg_reservoir_pressure,
        avg_pressure_rel_diff,
    );
}

#[test]
fn physics_depletion_gas_single_cell_runs_with_positive_gas_rate() {
    let mut sim = make_closed_gas_depletion_single_cell_sim();

    sim.step(0.01);
    assert!(
        sim.last_solver_warning.is_empty(),
        "single-cell gas depletion emitted solver warning: {}",
        sim.last_solver_warning
    );

    let latest = sim
        .rate_history
        .last()
        .expect("single-cell gas depletion should record history");

    assert!(latest.total_production_gas > 0.0);
    assert!(latest.avg_reservoir_pressure.is_finite());
    assert!(sim.sat_gas[0].is_finite());
    assert!(sim.sat_gas[0] >= 0.0);
}