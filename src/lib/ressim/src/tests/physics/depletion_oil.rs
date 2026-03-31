use crate::fim::newton::FimNewtonOptions;

use super::fixtures::{
    DEP_PSS_INITIAL_PRESSURE_BAR, collect_depletion_snapshots, make_closed_depletion_single_cell_sim,
    make_dep_pss_like_sim, run_single_cell_local_newton,
};

#[test]
fn physics_depletion_oil_single_cell_abs_oil_balance() {
    let diagnostics = run_single_cell_local_newton(0.1, FimNewtonOptions::default());

    assert!(diagnostics.residual_inf_norm <= 1e-5);
    assert!(diagnostics.material_balance_inf_norm <= 1e-5);
    assert!(
        diagnostics.oil_residual_abs_sc <= 0.1,
        "single-cell local depletion accepted too much absolute oil imbalance: oil_abs={:.6} scale={:.6}",
        diagnostics.oil_residual_abs_sc,
        diagnostics.oil_residual_scale_sc,
    );
}

#[test]
fn physics_depletion_oil_single_cell_timestep_stable() {
    let mut coarse = make_closed_depletion_single_cell_sim();
    coarse.step(0.1);
    assert!(
        coarse.last_solver_warning.is_empty(),
        "single-cell coarse depletion emitted solver warning: {}",
        coarse.last_solver_warning
    );
    let coarse_point = coarse
        .rate_history
        .last()
        .expect("single-cell coarse depletion should record history");

    let mut fine = make_closed_depletion_single_cell_sim();
    fine.step(0.05);
    fine.step(0.05);
    assert!(
        fine.last_solver_warning.is_empty(),
        "single-cell fine depletion emitted solver warning: {}",
        fine.last_solver_warning
    );
    let fine_point = fine
        .rate_history
        .last()
        .expect("single-cell fine depletion should record history");

    let oil_rate_rel_diff = ((coarse_point.total_production_oil - fine_point.total_production_oil)
        / fine_point.total_production_oil.max(1e-12))
    .abs();
    let avg_pressure_rel_diff = ((coarse_point.avg_reservoir_pressure
        - fine_point.avg_reservoir_pressure)
        / fine_point.avg_reservoir_pressure.max(1e-12))
    .abs();

    assert!(
        oil_rate_rel_diff <= 0.01,
        "single-cell depletion timestep oil-rate drift too large: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_point.total_production_oil,
        fine_point.total_production_oil,
        oil_rate_rel_diff,
    );
    assert!(
        avg_pressure_rel_diff <= 0.005,
        "single-cell depletion timestep pressure drift too large: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_point.avg_reservoir_pressure,
        fine_point.avg_reservoir_pressure,
        avg_pressure_rel_diff,
    );
}

#[test]
fn physics_depletion_oil_closed_system_monotone() {
    let sim = make_dep_pss_like_sim(0.1, 8);
    let snapshots = collect_depletion_snapshots(&sim);

    assert!(!snapshots.is_empty());

    let mut previous_pressure = DEP_PSS_INITIAL_PRESSURE_BAR;
    let mut previous_rate = f64::INFINITY;
    let mut previous_cumulative_oil = 0.0;
    for snapshot in &snapshots {
        assert!(snapshot.total_injection_sc_day.abs() <= 1e-12);
        assert!(snapshot.avg_pressure_bar <= previous_pressure + 1e-9);
        assert!(snapshot.oil_rate_sc_day <= previous_rate + 1e-9);
        assert!(snapshot.cumulative_oil_sc >= previous_cumulative_oil - 1e-9);

        previous_pressure = snapshot.avg_pressure_bar;
        previous_rate = snapshot.oil_rate_sc_day;
        previous_cumulative_oil = snapshot.cumulative_oil_sc;
    }
}