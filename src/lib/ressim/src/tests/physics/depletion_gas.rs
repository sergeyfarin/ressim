use super::fixtures::{make_closed_gas_depletion_single_cell_sim, total_gas_inventory_sc_all_cells};
use crate::pvt::{PvtRow, PvtTable};

#[derive(Clone, Copy)]
struct GasDepletionCase {
    name: &'static str,
    initial_sw: f64,
    initial_sg: f64,
    perm_md: f64,
    mu_g_cp: f64,
    c_g: f64,
    bg_scale: f64,
    n_g: f64,
}

fn make_gas_depletion_case(case: GasDepletionCase) -> crate::ReservoirSimulator {
    let mut sim = make_closed_gas_depletion_single_cell_sim();
    sim.set_three_phase_rel_perm_props(
        0.10, 0.10, 0.03, 0.03, 0.10, 2.0, 2.0, case.n_g, 0.8, 0.9, 0.7,
    )
    .unwrap();
    sim.set_gas_fluid_properties(case.mu_g_cp, case.c_g, 10.0)
        .unwrap();
    sim.set_initial_saturation(case.initial_sw);
    sim.set_initial_gas_saturation(case.initial_sg);
    sim.set_permeability_per_layer(vec![case.perm_md], vec![case.perm_md], vec![case.perm_md])
        .unwrap();
    sim.pvt_table = Some(PvtTable::new(
        vec![
            PvtRow {
                p_bar: 100.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.05,
                mu_o_cp: 1.1,
                bg_m3m3: 0.012 * case.bg_scale,
                mu_g_cp: case.mu_g_cp,
            },
            PvtRow {
                p_bar: 200.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.02,
                mu_o_cp: 1.1,
                bg_m3m3: 0.006 * case.bg_scale,
                mu_g_cp: case.mu_g_cp * 1.05,
            },
            PvtRow {
                p_bar: 300.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.00,
                mu_o_cp: 1.1,
                bg_m3m3: 0.004 * case.bg_scale,
                mu_g_cp: case.mu_g_cp * 1.10,
            },
        ],
        sim.pvt.c_o,
    ));
    sim
}

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

#[test]
fn physics_depletion_gas_single_cell_closed_system_monotone() {
    let mut sim = make_closed_gas_depletion_single_cell_sim();
    let mut prev_gas_inventory_sc = total_gas_inventory_sc_all_cells(&sim);
    let mut prev_pressure = sim.pressure[0];

    for _ in 0..8 {
        sim.step(0.005);
        assert!(
            sim.last_solver_warning.is_empty(),
            "single-cell gas depletion emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );

        let latest = sim
            .rate_history
            .last()
            .expect("gas depletion should record history");
        let gas_inventory_sc = total_gas_inventory_sc_all_cells(&sim);

        assert!(
            latest.total_production_gas > 0.0,
            "gas rate must remain positive during closed depletion at t={}",
            sim.time_days
        );
        assert!(
            latest.avg_reservoir_pressure <= prev_pressure + 1e-9,
            "pressure must not increase during closed gas depletion: prev={:.4}, now={:.4}",
            prev_pressure,
            latest.avg_reservoir_pressure
        );
        assert!(
            gas_inventory_sc <= prev_gas_inventory_sc + 1e-4,
            "gas SC inventory must not increase during closed depletion: prev={:.4}, now={:.4}",
            prev_gas_inventory_sc,
            gas_inventory_sc
        );

        prev_gas_inventory_sc = gas_inventory_sc;
        prev_pressure = latest.avg_reservoir_pressure;
    }
}

#[test]
fn physics_depletion_gas_case_matrix_stays_physical_across_sat_perm_pvt_and_scal_ranges() {
    let cases = [
        GasDepletionCase {
            name: "gas-rich base",
            initial_sw: 0.08,
            initial_sg: 0.89,
            perm_md: 500.0,
            mu_g_cp: 0.02,
            c_g: 1e-4,
            bg_scale: 1.0,
            n_g: 1.5,
        },
        GasDepletionCase {
            name: "lower perm steeper gas relperm",
            initial_sw: 0.15,
            initial_sg: 0.80,
            perm_md: 75.0,
            mu_g_cp: 0.03,
            c_g: 2e-4,
            bg_scale: 1.2,
            n_g: 2.0,
        },
        GasDepletionCase {
            name: "high perm lighter gas",
            initial_sw: 0.05,
            initial_sg: 0.92,
            perm_md: 2_000.0,
            mu_g_cp: 0.015,
            c_g: 5e-5,
            bg_scale: 0.85,
            n_g: 1.2,
        },
    ];

    for case in cases {
        let mut sim = make_gas_depletion_case(case);
        let initial_pressure = sim.pressure[0];
        let initial_inventory_sc = total_gas_inventory_sc_all_cells(&sim);

        sim.step(0.005);
        sim.step(0.005);

        assert!(
            sim.last_solver_warning.is_empty(),
            "{} gas depletion emitted solver warning at t={}: {}",
            case.name,
            sim.time_days,
            sim.last_solver_warning
        );

        let latest = sim
            .rate_history
            .last()
            .expect("gas depletion case matrix should record history");
        let final_inventory_sc = total_gas_inventory_sc_all_cells(&sim);

        assert!(
            latest.total_production_gas > 0.0,
            "{} should keep positive gas production, got {}",
            case.name,
            latest.total_production_gas
        );
        assert!(
            latest.avg_reservoir_pressure <= initial_pressure + 1e-9,
            "{} should not increase pressure: initial={:.6}, final={:.6}",
            case.name,
            initial_pressure,
            latest.avg_reservoir_pressure
        );
        assert!(
            final_inventory_sc <= initial_inventory_sc + 1e-4,
            "{} should not create gas inventory: initial={:.6}, final={:.6}",
            case.name,
            initial_inventory_sc,
            final_inventory_sc
        );
        assert!(
            sim.sat_gas[0].is_finite() && sim.sat_gas[0] >= -1e-9 && sim.sat_gas[0] <= 1.0 + 1e-9,
            "{} gas saturation escaped bounds: {}",
            case.name,
            sim.sat_gas[0]
        );
    }
}