use super::fixtures::{
    flash_below_bubble_point, make_below_bubble_point_flash_sim, total_gas_inventory_sc,
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

#[test]
fn physics_depletion_liberation_inventory_conserved_with_redissolution() {
    assert_below_bubble_point_flash_conserves_total_gas_inventory(true);
}