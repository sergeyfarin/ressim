use super::fixtures::{
    apply_pressure_only_flash_update, cumulative_component_production_sc, flash_below_bubble_point,
    make_below_bubble_point_flash_sim, total_component_inventory_sc_all_cells,
    total_gas_inventory_sc, total_gas_inventory_sc_all_cells,
};

fn assert_below_bubble_point_flash_conserves_total_gas_inventory(gas_redissolution_enabled: bool) {
    let mut sim = make_below_bubble_point_flash_sim(gas_redissolution_enabled);
    let gas_before_sc = total_gas_inventory_sc(&sim);

    flash_below_bubble_point(&mut sim, 125.0);

    let gas_after_sc = total_gas_inventory_sc(&sim);
    assert!(
        sim.sat_gas[0] > 0.0,
        "pressure drop below bubble point should liberate free gas when gas_redissolution_enabled={}",
        gas_redissolution_enabled
    );
    assert!(
        (gas_after_sc - gas_before_sc).abs() < 1e-8,
        "local flash should conserve total gas inventory when gas_redissolution_enabled={}, before={}, after={}",
        gas_redissolution_enabled,
        gas_before_sc,
        gas_after_sc,
    );
}

#[test]
fn physics_depletion_liberation_inventory_conserved_without_redissolution() {
    assert_below_bubble_point_flash_conserves_total_gas_inventory(false);
}

/// FIM path through the undersaturated-to-saturated phase transition: run actual Newton
/// timestep steps on a single-cell sim that starts at the bubble point and drops below it.
/// The IMPES-only tests above verify the flash function in isolation; this test verifies
/// the FIM Jacobian and acceptance logic correctly handles the phase switch.
#[test]
fn physics_depletion_liberation_fim_stepping_liberates_gas() {
    let mut sim = make_below_bubble_point_flash_sim(false);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(200.0, 200.0, vec![20.0])
        .unwrap();
    sim.set_permeability_per_layer(vec![500.0], vec![500.0], vec![500.0])
        .unwrap();
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.add_well(0, 0, 0, 80.0, 0.1, 0.0, false).unwrap();

    let gas_initial_sc = total_gas_inventory_sc_all_cells(&sim);
    assert_eq!(
        sim.sat_gas[0], 0.0,
        "should start with no free gas at bubble point"
    );

    for _ in 0..5 {
        sim.step(1.0);
        assert!(
            sim.last_solver_warning.is_empty(),
            "liberation FIM case emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    assert!(
        sim.pressure[0] < 150.0,
        "pressure should drop below bubble point (150 bar), got {:.2}",
        sim.pressure[0]
    );
    assert!(
        sim.sat_gas[0] > 0.0,
        "FIM stepping through bubble point should liberate free gas, got Sg={:.6}",
        sim.sat_gas[0]
    );

    // Gas inventory decreases because some gas is produced but the flash should not
    // create or destroy total hydrocarbon: inventory loss must be accounted for by production.
    let gas_final_sc = total_gas_inventory_sc_all_cells(&sim);
    let cumulative_gas_produced_sc: f64 = sim
        .rate_history
        .iter()
        .map(|p| p.total_production_gas.max(0.0))
        .sum();
    let accounted_sc = gas_final_sc + cumulative_gas_produced_sc;
    assert!(
        (accounted_sc - gas_initial_sc).abs() < gas_initial_sc * 0.01,
        "FIM liberation gas MB error too large: initial={:.4}, final+produced={:.4}",
        gas_initial_sc,
        accounted_sc
    );
}

#[test]
fn physics_depletion_liberation_public_transition_contract_holds_on_both_solvers() {
    fn run_case(fim_enabled: bool) -> (f64, f64, f64, f64, f64, f64, f64, usize) {
        let mut sim = make_below_bubble_point_flash_sim(false);
        sim.set_fim_enabled(fim_enabled);
        sim.set_cell_dimensions_per_layer(200.0, 200.0, vec![20.0])
            .unwrap();
        sim.set_permeability_per_layer(vec![500.0], vec![500.0], vec![500.0])
            .unwrap();
        sim.set_gravity_enabled(false);
        sim.set_stability_params(0.05, 75.0, 0.75);
        sim.add_well(0, 0, 0, 80.0, 0.1, 0.0, false).unwrap();

        let initial = total_component_inventory_sc_all_cells(&sim);

        for _ in 0..5 {
            sim.step(1.0);
            assert!(
                sim.last_solver_warning.is_empty(),
                "two-solver liberation public-contract case emitted solver warning for fim_enabled={}: {}",
                fim_enabled,
                sim.last_solver_warning
            );
        }

        let latest = sim
            .rate_history
            .last()
            .expect("two-solver liberation public-contract case should record history");
        let final_inventory = total_component_inventory_sc_all_cells(&sim);
        let produced = cumulative_component_production_sc(&sim);
        let gas_accounted = final_inventory.gas_sc + produced.gas_sc;
        let gas_rel_diff = ((gas_accounted - initial.gas_sc) / initial.gas_sc.max(1.0)).abs();

        (
            sim.pressure[0],
            sim.sat_gas[0],
            latest.total_production_oil,
            latest.total_production_gas,
            latest.producing_gor,
            gas_rel_diff,
            latest.time,
            sim.rate_history.len(),
        )
    }

    for (fim_enabled, metrics) in [(false, run_case(false)), (true, run_case(true))] {
        assert!(
            metrics.0 < 150.0,
            "expected pressure below bubble point for fim_enabled={}, got {:.6}",
            fim_enabled,
            metrics.0
        );
        assert!(
            metrics.1 > 0.0,
            "expected liberated free gas for fim_enabled={}, got {:.6}",
            fim_enabled,
            metrics.1
        );
        assert!(metrics.2 >= 0.0);
        assert!(metrics.3 >= 0.0);
        assert!(metrics.4.is_finite());
        assert!(
            metrics.5 <= 1.5e-1,
            "liberation gas accounting envelope too large for fim_enabled={}: {}",
            fim_enabled,
            metrics.5
        );
        assert!((metrics.6 - 5.0).abs() <= 1e-9);
        assert!(
            metrics.7 > 0,
            "expected rate history for fim_enabled={}",
            fim_enabled
        );
    }
}

#[test]
fn physics_depletion_liberation_inventory_conserved_with_redissolution() {
    assert_below_bubble_point_flash_conserves_total_gas_inventory(true);
}

#[test]
fn physics_depletion_liberation_component_balances_close_across_phase_transition() {
    let mut sim = make_below_bubble_point_flash_sim(false);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(200.0, 200.0, vec![20.0])
        .unwrap();
    sim.set_permeability_per_layer(vec![500.0], vec![500.0], vec![500.0])
        .unwrap();
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.add_well(0, 0, 0, 80.0, 0.1, 0.0, false).unwrap();

    let initial = total_component_inventory_sc_all_cells(&sim);

    for _ in 0..5 {
        sim.step(1.0);
        assert!(
            sim.last_solver_warning.is_empty(),
            "liberation component-balance case emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    let final_inventory = total_component_inventory_sc_all_cells(&sim);
    let produced = cumulative_component_production_sc(&sim);

    let water_accounted = final_inventory.water_sc + produced.water_sc;
    let oil_accounted = final_inventory.oil_sc + produced.oil_sc;
    let gas_accounted = final_inventory.gas_sc + produced.gas_sc;

    assert!(
        (water_accounted - initial.water_sc).abs() <= initial.water_sc.max(1.0) * 1e-6,
        "water balance drift too large across liberation transition: initial={:.6}, final+prod={:.6}",
        initial.water_sc,
        water_accounted
    );
    assert!(
        (oil_accounted - initial.oil_sc).abs() <= initial.oil_sc.max(1.0) * 5e-3,
        "oil balance drift too large across liberation transition: initial={:.6}, final+prod={:.6}",
        initial.oil_sc,
        oil_accounted
    );
    assert!(
        (gas_accounted - initial.gas_sc).abs() <= initial.gas_sc.max(1.0) * 1e-3,
        "gas balance drift too large across liberation transition: initial={:.6}, final+prod={:.6}",
        initial.gas_sc,
        gas_accounted
    );
}

#[test]
fn physics_depletion_liberation_undersaturated_rs_stays_constant() {
    let mut sim = make_below_bubble_point_flash_sim(false);
    let initial_rs = sim.rs[0];
    assert_eq!(
        sim.sat_gas[0], 0.0,
        "undersaturated constancy case should start with no free gas"
    );

    for pressure_bar in [174.0, 173.0, 172.0, 171.0, 170.0] {
        apply_pressure_only_flash_update(&mut sim, pressure_bar);
        assert!(
            sim.pressure[0] > 150.0,
            "undersaturated Rs constancy case should stay above bubble point, got p={:.6}",
            sim.pressure[0]
        );
        assert!(
            (sim.rs[0] - initial_rs).abs() <= 1e-12,
            "Rs should remain constant while the cell stays undersaturated: initial={:.12}, current={:.12}",
            initial_rs,
            sim.rs[0]
        );
        assert!(
            sim.sat_gas[0].abs() <= 1e-5,
            "free gas should remain numerically zero while the cell stays undersaturated, got Sg={:.12}",
            sim.sat_gas[0]
        );
    }
}

#[test]
#[ignore = "explicit refinement probe: liberation-through-bubble-point should stay stable under coarse-vs-fine timesteps"]
fn physics_depletion_liberation_timestep_refinement_keeps_transition_accounting_stable() {
    let mut coarse = make_below_bubble_point_flash_sim(false);
    coarse.set_fim_enabled(true);
    coarse
        .set_cell_dimensions_per_layer(200.0, 200.0, vec![20.0])
        .unwrap();
    coarse
        .set_permeability_per_layer(vec![500.0], vec![500.0], vec![500.0])
        .unwrap();
    coarse.set_gravity_enabled(false);
    coarse.set_stability_params(0.05, 75.0, 0.75);
    coarse.add_well(0, 0, 0, 80.0, 0.1, 0.0, false).unwrap();

    let mut fine = make_below_bubble_point_flash_sim(false);
    fine.set_fim_enabled(true);
    fine.set_cell_dimensions_per_layer(200.0, 200.0, vec![20.0])
        .unwrap();
    fine.set_permeability_per_layer(vec![500.0], vec![500.0], vec![500.0])
        .unwrap();
    fine.set_gravity_enabled(false);
    fine.set_stability_params(0.05, 75.0, 0.75);
    fine.add_well(0, 0, 0, 80.0, 0.1, 0.0, false).unwrap();

    for _ in 0..5 {
        coarse.step(1.0);
        assert!(
            coarse.last_solver_warning.is_empty(),
            "coarse liberation refinement case emitted solver warning at t={}: {}",
            coarse.time_days,
            coarse.last_solver_warning
        );
    }
    for _ in 0..10 {
        fine.step(0.5);
        assert!(
            fine.last_solver_warning.is_empty(),
            "fine liberation refinement case emitted solver warning at t={}: {}",
            fine.time_days,
            fine.last_solver_warning
        );
    }

    let coarse_final = total_component_inventory_sc_all_cells(&coarse);
    let fine_final = total_component_inventory_sc_all_cells(&fine);
    let coarse_produced = cumulative_component_production_sc(&coarse);
    let fine_produced = cumulative_component_production_sc(&fine);

    let coarse_gas_accounted = coarse_final.gas_sc + coarse_produced.gas_sc;
    let fine_gas_accounted = fine_final.gas_sc + fine_produced.gas_sc;
    let gas_accounted_rel_diff =
        ((coarse_gas_accounted - fine_gas_accounted) / fine_gas_accounted.max(1e-12)).abs();
    let sg_abs_diff = (coarse.sat_gas[0] - fine.sat_gas[0]).abs();
    let pressure_rel_diff =
        ((coarse.pressure[0] - fine.pressure[0]) / fine.pressure[0].max(1e-12)).abs();

    assert!(
        gas_accounted_rel_diff <= 0.03,
        "liberation transition gas accounting drift too large under timestep refinement: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_gas_accounted,
        fine_gas_accounted,
        gas_accounted_rel_diff
    );
    assert!(
        sg_abs_diff <= 0.03,
        "liberation transition free-gas saturation drift too large under timestep refinement: coarse={:.6}, fine={:.6}, abs_diff={:.6}",
        coarse.sat_gas[0],
        fine.sat_gas[0],
        sg_abs_diff
    );
    assert!(
        pressure_rel_diff <= 0.03,
        "liberation transition pressure drift too large under timestep refinement: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse.pressure[0],
        fine.pressure[0],
        pressure_rel_diff
    );
}
