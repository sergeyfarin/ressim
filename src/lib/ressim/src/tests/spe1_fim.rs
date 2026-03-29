use super::{make_spe1_like_base_sim, make_spe1_like_grid_sim};
use crate::ReservoirSimulator;

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
fn spe1_fim_coarse_grid_reaches_producer_gas_breakthrough() {
    let mut sim = make_spe1_like_grid_sim(5, 5, 4, 4, vec![500.0, 50.0, 200.0], 0.05, 20.0, 0.2);
    sim.set_fim_enabled(true);

    let producer_id = sim.idx(4, 4, 2);
    let mut breakthrough_time_days = None;
    let mut previous_producer_sg = sim.sat_gas[producer_id];
    let mut last_gor = 0.0;

    for _ in 0..120 {
        sim.step(30.0);
        assert!(
            sim.last_solver_warning.is_empty(),
            "FIM solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );

        let rate_point = sim.rate_history.last().expect("rate history should exist");
        last_gor = rate_point.producing_gor;
        if sim.sat_gas[producer_id] > 1e-4 || last_gor > 50.0 {
            breakthrough_time_days = Some(sim.time_days);
            break;
        }

        previous_producer_sg = sim.sat_gas[producer_id];
    }

    assert!(
        breakthrough_time_days.is_some(),
        "coarse SPE1 grid should reach producer gas breakthrough within 3600 days; final producer sg={}, previous producer sg={}, final gor={}, final time={}",
        sim.sat_gas[producer_id],
        previous_producer_sg,
        last_gor,
        sim.time_days,
    );
}
