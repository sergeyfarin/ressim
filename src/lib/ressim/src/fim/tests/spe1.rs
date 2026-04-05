use crate::ReservoirSimulator;
use crate::tests::make_spe1_like_base_sim;

#[test]
fn spe1_fim_first_steps_converge_without_stall() {
    let mut sim = make_spe1_like_base_sim();
    sim.set_fim_enabled(true);
    for _ in 0..5 {
        sim.step(1.0);
        assert!(
            sim.last_solver_warning.is_empty(),
            "FIM solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }
    assert!(
        sim.time_days >= 5.0 - 1e-9,
        "Simulation should advance to 5 days, got {}",
        sim.time_days
    );
}

#[test]
fn spe1_fim_gas_injection_creates_free_gas() {
    let mut sim = make_spe1_like_base_sim();
    sim.set_fim_enabled(true);

    let total_gas_inventory_sc = |sim: &ReservoirSimulator| -> f64 {
        (0..sim.nx * sim.ny * sim.nz)
            .map(|idx| {
                let pore_volume_m3 = sim.pore_volume_m3(idx).max(1e-9);
                let free_gas_sc =
                    sim.sat_gas[idx] * pore_volume_m3 / sim.get_b_g(sim.pressure[idx]).max(1e-9);
                let dissolved_gas_sc = if sim.pvt_table.is_some() {
                    sim.sat_oil[idx] * pore_volume_m3 * sim.rs[idx]
                        / sim.get_b_o_cell(idx, sim.pressure[idx]).max(1e-9)
                } else {
                    0.0
                };
                free_gas_sc + dissolved_gas_sc
            })
            .sum()
    };

    let initial_avg_sg = sim.sat_gas.iter().copied().sum::<f64>() / sim.sat_gas.len() as f64;
    let initial_total_gas_sc = total_gas_inventory_sc(&sim);

    for _ in 0..10 {
        sim.step(1.0);
        assert!(
            sim.last_solver_warning.is_empty(),
            "FIM solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    let final_avg_sg = sim.sat_gas.iter().copied().sum::<f64>() / sim.sat_gas.len() as f64;
    let final_total_gas_sc = total_gas_inventory_sc(&sim);
    let max_sg = sim.sat_gas.iter().copied().fold(0.0_f64, f64::max);

    assert!(
        max_sg > 1e-6,
        "some cell should contain free gas after FIM gas injection, max_sg={} at t={} days",
        max_sg,
        sim.time_days
    );
    assert!(
        final_avg_sg > initial_avg_sg + 1e-8,
        "average gas saturation should increase under FIM gas injection, before={}, after={}",
        initial_avg_sg,
        final_avg_sg
    );
    assert!(
        final_total_gas_sc > initial_total_gas_sc + 1.0,
        "total gas inventory should increase under FIM gas injection, before={}, after={}",
        initial_total_gas_sc,
        final_total_gas_sc
    );
}

#[test]
fn spe1_fim_producer_gas_breakthrough_smoke() {
    let mut sim = crate::tests::make_spe1_like_grid_sim(4, 4, 3, 3, vec![500.0, 50.0, 200.0], 0.05, 20.0, 0.2);
    sim.set_fim_enabled(true);

    let producer_id = sim.idx(3, 3, 2);
    let mut breakthrough_time_days = None;

    for _ in 0..80 {
        sim.step(20.0);
        assert!(
            sim.last_solver_warning.is_empty(),
            "FIM coarse-grid producer breakthrough smoke emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );

        let rate_point = sim.rate_history.last().expect("rate history should exist");
        if sim.sat_gas[producer_id] > 1e-4 || rate_point.producing_gor > 50.0 {
            breakthrough_time_days = Some(sim.time_days);
            break;
        }
    }

    let breakthrough_time_days = breakthrough_time_days.expect(
        "coarse-grid FIM gas-injection smoke should reach producer gas breakthrough within 1600 days"
    );

    assert!(breakthrough_time_days > 0.0);
    assert!(sim.rate_history.len() > 0);
}
