use crate::fim::newton::FimNewtonOptions;

use super::fixtures::{
    DEP_PSS_INITIAL_PRESSURE_BAR, collect_depletion_snapshots,
    make_closed_depletion_single_cell_sim, make_closed_depletion_single_cell_sim_with_storage,
    make_dep_pss_like_sim, run_single_cell_local_newton,
};

fn cumulative_reservoir_withdrawal_and_pressure_work_proxy(
    sim: &crate::ReservoirSimulator,
    producer_bhp_bar: f64,
) -> (f64, f64) {
    let mut cumulative_withdrawal_rm3 = 0.0;
    let mut pressure_work_proxy = 0.0;
    let mut previous_time_days = 0.0;

    for point in &sim.rate_history {
        let dt_days = point.time - previous_time_days;
        previous_time_days = point.time;
        cumulative_withdrawal_rm3 += point.total_production_liquid_reservoir.max(0.0) * dt_days;
        pressure_work_proxy += point.total_production_liquid_reservoir.max(0.0)
            * (point.avg_reservoir_pressure - producer_bhp_bar).max(0.0)
            * dt_days;
    }

    (cumulative_withdrawal_rm3, pressure_work_proxy)
}

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

#[test]
fn physics_depletion_oil_higher_oil_compressibility_cushions_pressure_drop() {
    let mut low_storage = make_closed_depletion_single_cell_sim_with_storage(0.0, 0.0, 0.0, 100.0);
    let mut high_storage =
        make_closed_depletion_single_cell_sim_with_storage(5e-5, 3e-6, 5e-6, 100.0);

    low_storage.step(0.05);
    high_storage.step(0.05);

    assert!(
        low_storage.last_solver_warning.is_empty(),
        "low-storage depletion emitted solver warning: {}",
        low_storage.last_solver_warning
    );
    assert!(
        high_storage.last_solver_warning.is_empty(),
        "high-storage depletion emitted solver warning: {}",
        high_storage.last_solver_warning
    );

    let low_pressure = low_storage.rate_history.last().unwrap().avg_reservoir_pressure;
    let high_pressure = high_storage.rate_history.last().unwrap().avg_reservoir_pressure;

    assert!(
        high_pressure > low_pressure + 1e-3,
        "higher compressive storage should cushion pressure drop: low-storage p={:.6}, high-storage p={:.6}",
        low_pressure,
        high_pressure
    );
}

#[test]
fn physics_depletion_oil_stronger_drawdown_increases_pressure_work_proxy() {
    let mut mild_drawdown =
        make_closed_depletion_single_cell_sim_with_storage(1e-5, 3e-6, 1e-6, 180.0);
    let mut strong_drawdown =
        make_closed_depletion_single_cell_sim_with_storage(1e-5, 3e-6, 1e-6, 80.0);

    mild_drawdown.step(0.05);
    strong_drawdown.step(0.05);

    assert!(
        mild_drawdown.last_solver_warning.is_empty(),
        "mild-drawdown depletion emitted solver warning: {}",
        mild_drawdown.last_solver_warning
    );
    assert!(
        strong_drawdown.last_solver_warning.is_empty(),
        "strong-drawdown depletion emitted solver warning: {}",
        strong_drawdown.last_solver_warning
    );

    let mild_latest = mild_drawdown.rate_history.last().unwrap();
    let strong_latest = strong_drawdown.rate_history.last().unwrap();
    let (mild_withdrawal, mild_work) =
        cumulative_reservoir_withdrawal_and_pressure_work_proxy(&mild_drawdown, 180.0);
    let (strong_withdrawal, strong_work) =
        cumulative_reservoir_withdrawal_and_pressure_work_proxy(&strong_drawdown, 80.0);

    assert!(
        strong_latest.total_production_oil > mild_latest.total_production_oil,
        "stronger drawdown should increase oil production rate: mild={:.6}, strong={:.6}",
        mild_latest.total_production_oil,
        strong_latest.total_production_oil
    );
    assert!(
        strong_withdrawal > mild_withdrawal,
        "stronger drawdown should increase cumulative reservoir withdrawal: mild={:.6}, strong={:.6}",
        mild_withdrawal,
        strong_withdrawal
    );
    assert!(
        strong_work > mild_work,
        "stronger drawdown should increase the pressure-work proxy: mild={:.6}, strong={:.6}",
        mild_work,
        strong_work
    );
    assert!(
        strong_latest.avg_reservoir_pressure <= mild_latest.avg_reservoir_pressure + 1e-9,
        "stronger drawdown should not leave a higher reservoir pressure: mild={:.6}, strong={:.6}",
        mild_latest.avg_reservoir_pressure,
        strong_latest.avg_reservoir_pressure
    );
}

#[test]
fn physics_depletion_oil_rock_compressibility_adds_storage_response() {
    let mut stiff_rock = make_closed_depletion_single_cell_sim_with_storage(1e-5, 3e-6, 0.0, 100.0);
    let mut compressible_rock =
        make_closed_depletion_single_cell_sim_with_storage(1e-5, 3e-6, 5e-5, 100.0);

    stiff_rock.step(0.05);
    compressible_rock.step(0.05);

    assert!(
        stiff_rock.last_solver_warning.is_empty(),
        "stiff-rock depletion emitted solver warning: {}",
        stiff_rock.last_solver_warning
    );
    assert!(
        compressible_rock.last_solver_warning.is_empty(),
        "compressible-rock depletion emitted solver warning: {}",
        compressible_rock.last_solver_warning
    );

    let stiff_pressure = stiff_rock.rate_history.last().unwrap().avg_reservoir_pressure;
    let compressible_pressure = compressible_rock.rate_history.last().unwrap().avg_reservoir_pressure;

    assert!(
        compressible_pressure > stiff_pressure + 1e-3,
        "higher rock compressibility should cushion pressure drop: stiff={:.6}, compressible={:.6}",
        stiff_pressure,
        compressible_pressure
    );
}