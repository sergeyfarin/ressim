use super::fixtures::{
    flash_below_bubble_point, make_below_bubble_point_flash_sim, total_gas_inventory_sc,
    total_gas_inventory_sc_all_cells,
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
    sim.set_cell_dimensions_per_layer(200.0, 200.0, vec![20.0]).unwrap();
    sim.set_permeability_per_layer(vec![500.0], vec![500.0], vec![500.0]).unwrap();
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.add_well(0, 0, 0, 80.0, 0.1, 0.0, false).unwrap();

    let gas_initial_sc = total_gas_inventory_sc_all_cells(&sim);
    assert_eq!(sim.sat_gas[0], 0.0, "should start with no free gas at bubble point");

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
fn physics_depletion_liberation_inventory_conserved_with_redissolution() {
    assert_below_bubble_point_flash_conserves_total_gas_inventory(true);
}