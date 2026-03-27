use super::*;

#[test]
fn black_oil_compressibility_falls_back_when_bo_slope_goes_negative() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.pvt.c_o = 1e-5;
    sim.pvt_table = Some(pvt::PvtTable::new(
        vec![
            pvt::PvtRow {
                p_bar: 100.0,
                rs_m3m3: 5.0,
                bo_m3m3: 1.05,
                mu_o_cp: 1.5,
                bg_m3m3: 0.01,
                mu_g_cp: 0.02,
            },
            pvt::PvtRow {
                p_bar: 150.0,
                rs_m3m3: 15.0,
                bo_m3m3: 1.12,
                mu_o_cp: 1.2,
                bg_m3m3: 0.006,
                mu_g_cp: 0.025,
            },
            pvt::PvtRow {
                p_bar: 200.0,
                rs_m3m3: 15.0,
                bo_m3m3: 1.11944,
                mu_o_cp: 1.3,
                bg_m3m3: 0.0045,
                mu_g_cp: 0.03,
            },
        ],
        sim.pvt.c_o,
    ));

    let c_o_below_bubble_point = sim.get_c_o(149.0);
    let c_o_above_bubble_point = sim.get_c_o(175.0);

    assert!(c_o_below_bubble_point.is_finite());
    assert!(c_o_above_bubble_point.is_finite());
    assert_eq!(c_o_below_bubble_point, sim.pvt.c_o);
    assert!(c_o_above_bubble_point >= sim.pvt.c_o);
}

#[test]
fn effective_oil_compressibility_includes_dissolved_gas_below_bubble_point() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.pvt.c_o = 1e-5;
    sim.rho_g = 0.9;
    sim.pvt_table = Some(pvt::PvtTable::new(
        vec![
            pvt::PvtRow {
                p_bar: 100.0,
                rs_m3m3: 5.0,
                bo_m3m3: 1.05,
                mu_o_cp: 1.5,
                bg_m3m3: 0.01,
                mu_g_cp: 0.02,
            },
            pvt::PvtRow {
                p_bar: 150.0,
                rs_m3m3: 15.0,
                bo_m3m3: 1.12,
                mu_o_cp: 1.2,
                bg_m3m3: 0.006,
                mu_g_cp: 0.025,
            },
            pvt::PvtRow {
                p_bar: 200.0,
                rs_m3m3: 15.0,
                bo_m3m3: 1.119,
                mu_o_cp: 1.3,
                bg_m3m3: 0.0045,
                mu_g_cp: 0.03,
            },
        ],
        sim.pvt.c_o,
    ));

    let rs_sat_125 = sim.pvt_table.as_ref().unwrap().interpolate(125.0).rs_m3m3;
    let c_eff_below = sim.get_c_o_effective(125.0, rs_sat_125);
    let c_o_below = sim.get_c_o(125.0);
    assert!(c_eff_below.is_finite());
    assert!(c_eff_below > 0.0);
    assert!(
        c_eff_below > c_o_below,
        "c_o_effective ({c_eff_below}) must exceed c_o ({c_o_below}) below bubble point"
    );

    let rs_sat_175 = sim.pvt_table.as_ref().unwrap().interpolate(175.0).rs_m3m3;
    let c_eff_above = sim.get_c_o_effective(175.0, rs_sat_175);
    let c_o_above = sim.get_c_o(175.0);
    assert!(c_eff_above.is_finite());
    assert!(c_eff_above > 0.0);
    assert!(
        (c_eff_above - c_o_above).abs() / c_o_above < 0.5,
        "c_o_effective ({c_eff_above}) should be close to c_o ({c_o_above}) above bubble point"
    );

    let rho = sim.get_rho_o(125.0);
    let row = sim.pvt_table.as_ref().unwrap().interpolate(125.0);
    let expected = (sim.pvt.rho_o + row.rs_m3m3 * sim.rho_g) / row.bo_m3m3;
    assert!(
        (rho - expected).abs() < 1e-6,
        "ρ_o ({rho}) should include dissolved gas ({expected})"
    );
    let rho_simple = sim.pvt.rho_o / row.bo_m3m3;
    assert!(
        rho > rho_simple,
        "ρ_o with Rs ({rho}) must exceed dead-oil density ({rho_simple})"
    );
}

#[test]
fn bubble_point_blending_smooths_compressibility_near_bubble_point() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.pvt.c_o = 2e-4;
    sim.max_pressure_change_per_step = 50.0;
    sim.pvt_table = Some(pvt::PvtTable::new(
        vec![
            pvt::PvtRow {
                p_bar: 100.0,
                rs_m3m3: 50.0,
                bo_m3m3: 1.30,
                mu_o_cp: 0.7,
                bg_m3m3: 0.010,
                mu_g_cp: 0.020,
            },
            pvt::PvtRow {
                p_bar: 200.0,
                rs_m3m3: 150.0,
                bo_m3m3: 1.50,
                mu_o_cp: 0.5,
                bg_m3m3: 0.005,
                mu_g_cp: 0.025,
            },
            pvt::PvtRow {
                p_bar: 300.0,
                rs_m3m3: 250.0,
                bo_m3m3: 1.70,
                mu_o_cp: 0.4,
                bg_m3m3: 0.004,
                mu_g_cp: 0.030,
            },
        ],
        sim.pvt.c_o,
    ));

    let rs_cell = 150.0;
    let c_unsat = sim.pvt.c_o;

    let c_far = sim.get_c_o_effective(250.0, rs_cell);
    assert!(
        (c_far - c_unsat).abs() < 1e-9,
        "Far from bubble point: should equal c_o={c_unsat}, got {c_far}"
    );

    let c_near = sim.get_c_o_effective(225.0, rs_cell);
    assert!(
        c_near > c_unsat,
        "Near bubble point: should exceed c_o={c_unsat}, got {c_near}"
    );

    let c_close = sim.get_c_o_effective(202.0, rs_cell);
    assert!(
        c_close > c_near,
        "Closer to BP: c_o_eff({c_close}) should exceed value at 225 bar ({c_near})"
    );

    let c_at_bp = sim.get_c_o_effective(200.0, rs_cell);
    assert!(
        c_at_bp > c_unsat,
        "At bubble point: should use saturated c_o_eff={c_at_bp} > c_o={c_unsat}"
    );

    assert!(
        c_close > c_near && c_near > c_far,
        "Compressibility should increase monotonically toward bubble point"
    );
}
