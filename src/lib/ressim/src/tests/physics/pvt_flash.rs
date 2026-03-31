use crate::ReservoirSimulator;

#[test]
fn physics_pvt_flash_no_table_oil_compressibility_consistent() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.b_o = 1.0;
    sim.pvt.c_o = 1e-5;

    let oil_lo = sim.oil_props_for_state(100.0, 0.0);
    let oil_hi = sim.oil_props_for_state(300.0, 0.0);

    assert!(oil_hi.bo_m3m3 < oil_lo.bo_m3m3);
    assert!(oil_hi.rho_o_kg_m3 > oil_lo.rho_o_kg_m3);

    let expected_bo_hi = f64::exp(-sim.pvt.c_o * 300.0);
    assert!((oil_hi.bo_m3m3 - expected_bo_hi).abs() < 1e-12);

    let derivative = sim.get_d_bo_d_p_for_state(300.0, 0.0, false);
    assert!((derivative + sim.pvt.c_o * oil_hi.bo_m3m3).abs() < 1e-12);
}

#[test]
fn physics_pvt_flash_two_phase_zero_gas_exact() {
    let mut sim = ReservoirSimulator::new(5, 1, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
    sim.add_well(4, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

    for _ in 0..20 {
        sim.step(1.0);
        assert!(
            sim.last_solver_warning.is_empty(),
            "two-phase FIM case emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    for (idx, &sg) in sim.sat_gas.iter().enumerate() {
        assert_eq!(
            sg, 0.0,
            "sat_gas[{}] should stay exactly zero in 2-phase FIM mode, got {}",
            idx, sg
        );
        assert_eq!(
            sim.rs[idx], 0.0,
            "rs[{}] should stay exactly zero in 2-phase FIM mode, got {}",
            idx, sim.rs[idx]
        );
        assert!(
            (sim.sat_water[idx] + sim.sat_oil[idx] - 1.0).abs() < 1e-8,
            "sw + so should remain 1 in 2-phase FIM mode at cell {}",
            idx
        );
    }
}