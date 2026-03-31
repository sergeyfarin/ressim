use crate::ReservoirSimulator;
use super::fixtures::make_closed_gas_depletion_single_cell_sim;

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

#[test]
fn physics_pvt_flash_tabular_bg_derivative_consistent() {
    // Use the gas depletion fixture which has a 3-point tabular PVT: Bg goes from 0.012 at 100
    // bar to 0.006 at 200 bar to 0.004 at 300 bar.  The analytic derivative from the table must
    // match a central finite difference to machine precision (linear interpolation → exact FD).
    let sim = make_closed_gas_depletion_single_cell_sim();

    let dp = 1e-3;
    for p in [125.0, 150.0, 175.0, 250.0] {
        let bg_lo = sim.get_b_g(p - dp);
        let bg_hi = sim.get_b_g(p + dp);
        let fd = (bg_hi - bg_lo) / (2.0 * dp);
        let analytic = sim.get_d_bg_d_p_for_state(p);

        assert!(
            analytic < 0.0,
            "dBg/dp should be negative at p={} (Bg decreases with rising pressure), got {}",
            p,
            analytic
        );
        assert!(
            (analytic - fd).abs() < 1e-8,
            "tabular dBg/dp mismatch at p={}: analytic={:.10}, fd={:.10}",
            p,
            analytic,
            fd
        );
    }
}