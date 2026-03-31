use super::fixtures::{make_3phase_gas_injection_sim, total_gas_inventory_sc_all_cells};
use crate::ReservoirSimulator;

#[test]
fn physics_gas_flood_1d_creates_free_gas_and_keeps_balance_bounded() {
    let mut sim = make_3phase_gas_injection_sim(8, true);
    let initial_gas_sc = total_gas_inventory_sc_all_cells(&sim);
    let initial_avg_sg = sim.sat_gas.iter().copied().sum::<f64>() / sim.sat_gas.len() as f64;
    let producer_id = sim.idx(sim.nx - 1, 0, 0);

    for _ in 0..16 {
        sim.step(1.0);
        assert!(
            sim.last_solver_warning.is_empty(),
            "1D gas flood emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    let final_gas_sc = total_gas_inventory_sc_all_cells(&sim);
    let latest = sim
        .rate_history
        .last()
        .expect("1D gas flood should record history");
    let max_sg = sim.sat_gas.iter().copied().fold(0.0_f64, f64::max);
    let final_avg_sg = sim.sat_gas.iter().copied().sum::<f64>() / sim.sat_gas.len() as f64;
    let cumulative_gas_produced_sc: f64 = sim
        .rate_history
        .iter()
        .map(|point| point.total_production_gas.max(0.0))
        .sum();

    assert!(max_sg > 1e-6);
    assert!(final_gas_sc > initial_gas_sc + 1.0);
    assert!(final_avg_sg > initial_avg_sg + 1e-8);
    assert!(sim.sat_gas[0] > sim.sat_gas[producer_id]);
    assert!(latest.total_production_gas.is_finite());
    assert!(latest.producing_gor.is_finite());
    assert!(cumulative_gas_produced_sc > 0.0);
    assert!(
        latest.material_balance_error_gas_m3 < 5.0e3,
        "1D gas flood gas MB drift too large: {} Sm3",
        latest.material_balance_error_gas_m3
    );
}

#[test]
fn physics_gas_flood_saturation_sum_stays_physical() {
    let mut sim = make_3phase_gas_injection_sim(8, true);

    for _ in 0..30 {
        sim.step(1.0);
        assert!(
            sim.last_solver_warning.is_empty(),
            "gas flood saturation-closure case emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    for (idx, (&sw, (&so, &sg))) in sim
        .sat_water
        .iter()
        .zip(sim.sat_oil.iter().zip(sim.sat_gas.iter()))
        .enumerate()
    {
        let sum = sw + so + sg;
        assert!(
            (sum - 1.0).abs() < 1e-8,
            "sw+so+sg != 1 at cell {}: sw={:.6}, so={:.6}, sg={:.6}, sum={:.9}",
            idx,
            sw,
            so,
            sg,
            sum
        );
        assert!(sw >= -1e-9, "Sw negative at cell {}: {}", idx, sw);
        assert!(so >= -1e-9, "So negative at cell {}: {}", idx, so);
        assert!(sg >= -1e-9, "Sg negative at cell {}: {}", idx, sg);
    }
}

#[test]
fn physics_gas_flood_large_steps_keep_state_bounded() {
    let mut sim = ReservoirSimulator::new(6, 1, 3, 0.2);
    sim.set_fim_enabled(true);
    sim.set_three_phase_rel_perm_props(
        0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7,
    )
    .unwrap();
    sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_injected_fluid("gas").unwrap();
    sim.set_initial_pressure(330.0);
    sim.set_initial_saturation(0.12);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.pc.p_entry = 0.0;
    sim.add_well(0, 0, 0, 450.0, 0.1, 0.0, true).unwrap();
    sim.add_well(5, 0, 2, 150.0, 0.1, 0.0, false).unwrap();

    for _ in 0..12 {
        sim.step(5.0);
        assert!(
            sim.last_solver_warning.is_empty(),
            "large-step gas flood emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    for (idx, pressure) in sim.pressure.iter().enumerate() {
        assert!(pressure.is_finite(), "pressure must remain finite at cell {}", idx);
        assert!(
            *pressure > 1.0 && *pressure < 5_000.0,
            "pressure {} at cell {} escaped the physical envelope",
            pressure,
            idx
        );
    }

    for (idx, sg) in sim.sat_gas.iter().enumerate() {
        assert!(sg.is_finite(), "gas saturation must remain finite at cell {}", idx);
        assert!(
            *sg >= -1e-9 && *sg <= 1.0 + 1e-9,
            "gas saturation {} at cell {} escaped bounds",
            sg,
            idx
        );
    }

    for point in &sim.rate_history {
        assert!(point.avg_reservoir_pressure.is_finite());
        assert!(point.avg_reservoir_pressure > 1.0);
        assert!(point.avg_reservoir_pressure < 5_000.0);
    }
}