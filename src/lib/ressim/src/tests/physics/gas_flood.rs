use super::fixtures::{
    cumulative_component_production_sc, make_3phase_gas_injection_sim,
    total_component_inventory_sc_all_cells, total_gas_inventory_sc_all_cells,
};
use super::super::make_spe1_like_grid_sim;
use crate::ReservoirSimulator;

#[derive(Clone, Copy)]
struct GasFloodCase {
    name: &'static str,
    initial_sw: f64,
    perm_md: f64,
    mu_g_cp: f64,
    c_g: f64,
    n_g: f64,
    gas_oil_pc_entry_bar: f64,
}

fn make_gas_flood_case(case: GasFloodCase) -> ReservoirSimulator {
    let mut sim = make_3phase_gas_injection_sim(8, true);
    sim.set_three_phase_rel_perm_props(
        0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, case.n_g, 0.8, 0.9, 0.7,
    )
    .unwrap();
    sim.set_initial_saturation(case.initial_sw);
    sim.set_gas_fluid_properties(case.mu_g_cp, case.c_g, 10.0)
        .unwrap();
    sim.set_permeability_random_seeded(case.perm_md, case.perm_md, 42)
        .unwrap();
    if case.gas_oil_pc_entry_bar > 0.0 {
        sim.set_gas_oil_capillary_params(case.gas_oil_pc_entry_bar, 1.8)
            .unwrap();
    } else {
        sim.pc_og = None;
    }
    sim
}

fn cumulative_gas_production_sc(sim: &ReservoirSimulator) -> f64 {
    let mut cumulative_gas = 0.0;
    let mut previous_time_days = 0.0;

    for point in &sim.rate_history {
        let dt_days = point.time - previous_time_days;
        previous_time_days = point.time;
        cumulative_gas += point.total_production_gas.max(0.0) * dt_days;
    }

    cumulative_gas
}

fn cumulative_gas_injection_sc(sim: &ReservoirSimulator) -> f64 {
    let mut cumulative_gas = 0.0;
    let mut previous_time_days = 0.0;

    for point in &sim.rate_history {
        let dt_days = point.time - previous_time_days;
        previous_time_days = point.time;
        cumulative_gas += point.total_injection.max(0.0) * dt_days;
    }

    cumulative_gas
}

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
fn physics_gas_flood_1d_short_material_balance_matches_inventory_change() {
    let mut sim = make_3phase_gas_injection_sim(8, true);
    let initial_inventory = total_component_inventory_sc_all_cells(&sim);

    for _ in 0..8 {
        sim.step(0.5);
        assert!(
            sim.last_solver_warning.is_empty(),
            "short 1D gas flood MB case emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    let final_inventory = total_component_inventory_sc_all_cells(&sim);
    let produced = cumulative_component_production_sc(&sim);
    let injected_gas_sc = cumulative_gas_injection_sc(&sim);

    let water_accounted = final_inventory.water_sc + produced.water_sc;
    let oil_accounted = final_inventory.oil_sc + produced.oil_sc;
    let gas_accounted = final_inventory.gas_sc + produced.gas_sc;
    let expected_gas_sc = initial_inventory.gas_sc + injected_gas_sc;

    assert!(
        (water_accounted - initial_inventory.water_sc).abs()
            <= initial_inventory.water_sc.max(1.0) * 5e-6,
        "short 1D gas flood water balance drift too large: initial={:.6}, final+prod={:.6}",
        initial_inventory.water_sc,
        water_accounted
    );
    assert!(
        (oil_accounted - initial_inventory.oil_sc).abs() <= initial_inventory.oil_sc.max(1.0) * 5e-3,
        "short 1D gas flood oil balance drift too large: initial={:.6}, final+prod={:.6}",
        initial_inventory.oil_sc,
        oil_accounted
    );
    assert!(
        (gas_accounted - expected_gas_sc).abs() <= expected_gas_sc.max(1.0) * 1e-2,
        "short 1D gas flood gas balance drift too large: initial+inj={:.6}, final+prod={:.6}",
        expected_gas_sc,
        gas_accounted
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

#[test]
fn physics_gas_flood_case_matrix_remains_bounded_across_sat_perm_pvt_and_capillary_ranges() {
    let cases = [
        GasFloodCase {
            name: "base",
            initial_sw: 0.10,
            perm_md: 2_000.0,
            mu_g_cp: 0.02,
            c_g: 1e-4,
            n_g: 1.5,
            gas_oil_pc_entry_bar: 0.0,
        },
        GasFloodCase {
            name: "slower gas with entry pressure",
            initial_sw: 0.15,
            perm_md: 500.0,
            mu_g_cp: 0.03,
            c_g: 1.5e-4,
            n_g: 2.0,
            gas_oil_pc_entry_bar: 3.0,
        },
        GasFloodCase {
            name: "more mobile gas",
            initial_sw: 0.06,
            perm_md: 5_000.0,
            mu_g_cp: 0.015,
            c_g: 7e-5,
            n_g: 1.2,
            gas_oil_pc_entry_bar: 1.0,
        },
    ];

    for case in cases {
        let mut sim = make_gas_flood_case(case);
        let initial_avg_sg = sim.sat_gas.iter().copied().sum::<f64>() / sim.sat_gas.len() as f64;

        for _ in 0..10 {
            sim.step(1.0);
            assert!(
                sim.last_solver_warning.is_empty(),
                "{} gas flood emitted solver warning at t={}: {}",
                case.name,
                sim.time_days,
                sim.last_solver_warning
            );
        }

        let latest = sim
            .rate_history
            .last()
            .expect("gas flood case matrix should record history");
        let final_avg_sg = sim.sat_gas.iter().copied().sum::<f64>() / sim.sat_gas.len() as f64;

        assert!(
            final_avg_sg > initial_avg_sg + 1e-8,
            "{} average Sg should increase: initial={:.6}, final={:.6}",
            case.name,
            initial_avg_sg,
            final_avg_sg
        );
        assert!(latest.total_production_gas.is_finite());
        assert!(latest.producing_gor.is_finite());
        assert!(
            latest.material_balance_error_gas_m3 < 8.0e3,
            "{} gas MB drift too large: {} Sm3",
            case.name,
            latest.material_balance_error_gas_m3
        );

        for (idx, (&sw, (&so, &sg))) in sim
            .sat_water
            .iter()
            .zip(sim.sat_oil.iter().zip(sim.sat_gas.iter()))
            .enumerate()
        {
            let sum = sw + so + sg;
            assert!(
                (sum - 1.0).abs() < 1e-8,
                "{} saturation closure failed at cell {}: sw={:.6}, so={:.6}, sg={:.6}",
                case.name,
                idx,
                sw,
                so,
                sg
            );
        }
    }
}

#[test]
#[ignore = "explicit refinement probe: short 1D gas flood should stay directionally stable under timestep refinement"]
fn physics_gas_flood_1d_timestep_refinement_keeps_breakthrough_ordering_stable() {
    let mut coarse = make_3phase_gas_injection_sim(8, true);
    let mut fine = make_3phase_gas_injection_sim(8, true);

    for _ in 0..16 {
        coarse.step(1.0);
        assert!(
            coarse.last_solver_warning.is_empty(),
            "coarse 1D gas flood emitted solver warning at t={}: {}",
            coarse.time_days,
            coarse.last_solver_warning
        );
    }
    for _ in 0..32 {
        fine.step(0.5);
        assert!(
            fine.last_solver_warning.is_empty(),
            "fine 1D gas flood emitted solver warning at t={}: {}",
            fine.time_days,
            fine.last_solver_warning
        );
    }

    let coarse_last = coarse.rate_history.last().expect("coarse gas flood should record history");
    let fine_last = fine.rate_history.last().expect("fine gas flood should record history");
    let coarse_avg_sg = coarse.sat_gas.iter().copied().sum::<f64>() / coarse.sat_gas.len() as f64;
    let fine_avg_sg = fine.sat_gas.iter().copied().sum::<f64>() / fine.sat_gas.len() as f64;
    let coarse_cum_gas = cumulative_gas_production_sc(&coarse);
    let fine_cum_gas = cumulative_gas_production_sc(&fine);

    let avg_sg_abs_diff = (coarse_avg_sg - fine_avg_sg).abs();
    let pressure_rel_diff = ((coarse_last.avg_reservoir_pressure - fine_last.avg_reservoir_pressure)
        / fine_last.avg_reservoir_pressure.max(1e-12))
    .abs();
    let cumulative_gas_rel_diff = ((coarse_cum_gas - fine_cum_gas) / fine_cum_gas.max(1e-12)).abs();

    assert!(
        avg_sg_abs_diff <= 0.03,
        "1D gas flood average Sg drift too large under timestep refinement: coarse={:.6}, fine={:.6}, abs_diff={:.6}",
        coarse_avg_sg,
        fine_avg_sg,
        avg_sg_abs_diff
    );
    assert!(
        pressure_rel_diff <= 0.05,
        "1D gas flood avg-pressure drift too large under timestep refinement: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_last.avg_reservoir_pressure,
        fine_last.avg_reservoir_pressure,
        pressure_rel_diff
    );
    assert!(
        cumulative_gas_rel_diff <= 0.10,
        "1D gas flood cumulative-gas drift too large under timestep refinement: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_cum_gas,
        fine_cum_gas,
        cumulative_gas_rel_diff
    );
    assert!(coarse_last.material_balance_error_gas_m3 <= 8.0e3);
    assert!(fine_last.material_balance_error_gas_m3 <= 8.0e3);
}

#[test]
#[ignore = "larger-grid benchmark probe: coarse SPE1-like gas injection should still reach producer gas breakthrough"]
fn physics_gas_flood_spe1_coarse_grid_reaches_producer_gas_breakthrough() {
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
            "SPE1-like gas-flood probe emitted solver warning at t={}: {}",
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
        "coarse SPE1-like gas-flood probe should reach producer gas breakthrough within 3600 days; final producer sg={}, previous producer sg={}, final gor={}, final time={}",
        sim.sat_gas[producer_id],
        previous_producer_sg,
        last_gor,
        sim.time_days,
    );
}