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

    let c_near = sim.get_c_o_effective(203.0, rs_cell);
    assert!(
        c_near > c_unsat,
        "Near bubble point: should exceed c_o={c_unsat}, got {c_near}"
    );

    let c_close = sim.get_c_o_effective(201.0, rs_cell);
    assert!(
        c_close > c_near,
        "Closer to BP: c_o_eff({c_close}) should exceed value at 203 bar ({c_near})"
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

#[test]
fn hybrid_undersaturated_spe1_branch_matches_legacy_initial_bo_and_mu() {
    let co = 2.0563779328505968e-4;
    let branch_rs = 226.20;
    let p_initial = 331.0;
    let p_bubble = 276.79;
    let bo_bubble = 1.695;
    let mu_bubble = 0.510;

    let table = pvt::PvtTable::new(
        vec![
            pvt::PvtRow {
                p_bar: 1.01,
                rs_m3m3: 0.18,
                bo_m3m3: 1.062,
                mu_o_cp: 1.040,
                bg_m3m3: 0.9361,
                mu_g_cp: 0.0080,
            },
            pvt::PvtRow {
                p_bar: 18.25,
                rs_m3m3: 16.12,
                bo_m3m3: 1.150,
                mu_o_cp: 0.975,
                bg_m3m3: 0.0679,
                mu_g_cp: 0.0096,
            },
            pvt::PvtRow {
                p_bar: 35.49,
                rs_m3m3: 32.06,
                bo_m3m3: 1.207,
                mu_o_cp: 0.910,
                bg_m3m3: 0.0352,
                mu_g_cp: 0.0112,
            },
            pvt::PvtRow {
                p_bar: 69.96,
                rs_m3m3: 66.08,
                bo_m3m3: 1.295,
                mu_o_cp: 0.830,
                bg_m3m3: 0.0179,
                mu_g_cp: 0.0140,
            },
            pvt::PvtRow {
                p_bar: 138.91,
                rs_m3m3: 113.29,
                bo_m3m3: 1.435,
                mu_o_cp: 0.695,
                bg_m3m3: 0.00906,
                mu_g_cp: 0.0189,
            },
            pvt::PvtRow {
                p_bar: 173.38,
                rs_m3m3: 138.03,
                bo_m3m3: 1.500,
                mu_o_cp: 0.641,
                bg_m3m3: 0.00727,
                mu_g_cp: 0.0208,
            },
            pvt::PvtRow {
                p_bar: 207.85,
                rs_m3m3: 165.64,
                bo_m3m3: 1.565,
                mu_o_cp: 0.594,
                bg_m3m3: 0.00607,
                mu_g_cp: 0.0228,
            },
            pvt::PvtRow {
                p_bar: 276.79,
                rs_m3m3: branch_rs,
                bo_m3m3: bo_bubble,
                mu_o_cp: mu_bubble,
                bg_m3m3: 0.00455,
                mu_g_cp: 0.0268,
            },
            pvt::PvtRow {
                p_bar: 345.73,
                rs_m3m3: 288.17,
                bo_m3m3: 1.827,
                mu_o_cp: 0.449,
                bg_m3m3: 0.00364,
                mu_g_cp: 0.0309,
            },
        ],
        co,
    );

    let expected_bo = bo_bubble * f64::exp(-co * (p_initial - p_bubble));
    let expected_mu = mu_bubble * f64::exp(co * (p_initial - p_bubble));
    let (bo, mu) = table.interpolate_oil(p_initial, branch_rs);

    assert!(
        (bo - expected_bo).abs() < 1e-12,
        "hybrid Bo mismatch: {bo} vs {expected_bo}"
    );
    assert!(
        (mu - expected_mu).abs() < 1e-12,
        "hybrid mu mismatch: {mu} vs {expected_mu}"
    );
}

#[test]
fn tabular_undersaturated_pvto_matches_spe1_initial_bo_and_mu_for_rs_127_branch() {
    let co = 2.0563779328505968e-4;
    let branch_rs = 226.20;
    let p_initial = 331.0;
    let p_bubble = 276.79;
    let p_undersat = 621.54;
    let bo_bubble = 1.695;
    let bo_undersat = 1.579;
    let mu_bubble = 0.510;
    let mu_undersat = 0.740;

    let table = pvt::PvtTable::new(
        vec![
            pvt::PvtRow {
                p_bar: 1.01,
                rs_m3m3: 0.18,
                bo_m3m3: 1.062,
                mu_o_cp: 1.040,
                bg_m3m3: 0.9361,
                mu_g_cp: 0.0080,
            },
            pvt::PvtRow {
                p_bar: 18.25,
                rs_m3m3: 16.12,
                bo_m3m3: 1.150,
                mu_o_cp: 0.975,
                bg_m3m3: 0.0679,
                mu_g_cp: 0.0096,
            },
            pvt::PvtRow {
                p_bar: 35.49,
                rs_m3m3: 32.06,
                bo_m3m3: 1.207,
                mu_o_cp: 0.910,
                bg_m3m3: 0.0352,
                mu_g_cp: 0.0112,
            },
            pvt::PvtRow {
                p_bar: 69.96,
                rs_m3m3: 66.08,
                bo_m3m3: 1.295,
                mu_o_cp: 0.830,
                bg_m3m3: 0.0179,
                mu_g_cp: 0.0140,
            },
            pvt::PvtRow {
                p_bar: 138.91,
                rs_m3m3: 113.29,
                bo_m3m3: 1.435,
                mu_o_cp: 0.695,
                bg_m3m3: 0.00906,
                mu_g_cp: 0.0189,
            },
            pvt::PvtRow {
                p_bar: 173.38,
                rs_m3m3: 138.03,
                bo_m3m3: 1.500,
                mu_o_cp: 0.641,
                bg_m3m3: 0.00727,
                mu_g_cp: 0.0208,
            },
            pvt::PvtRow {
                p_bar: 207.85,
                rs_m3m3: 165.64,
                bo_m3m3: 1.565,
                mu_o_cp: 0.594,
                bg_m3m3: 0.00607,
                mu_g_cp: 0.0228,
            },
            pvt::PvtRow {
                p_bar: 276.79,
                rs_m3m3: branch_rs,
                bo_m3m3: bo_bubble,
                mu_o_cp: mu_bubble,
                bg_m3m3: 0.00455,
                mu_g_cp: 0.0268,
            },
            pvt::PvtRow {
                p_bar: p_undersat,
                rs_m3m3: branch_rs,
                bo_m3m3: bo_undersat,
                mu_o_cp: mu_undersat,
                bg_m3m3: 0.00455,
                mu_g_cp: 0.0268,
            },
            pvt::PvtRow {
                p_bar: 345.73,
                rs_m3m3: 288.17,
                bo_m3m3: 1.827,
                mu_o_cp: 0.449,
                bg_m3m3: 0.00364,
                mu_g_cp: 0.0309,
            },
            pvt::PvtRow {
                p_bar: p_undersat,
                rs_m3m3: 288.17,
                bo_m3m3: 1.737,
                mu_o_cp: 0.631,
                bg_m3m3: 0.00364,
                mu_g_cp: 0.0309,
            },
        ],
        co,
    );

    let t = (p_initial - p_bubble) / (p_undersat - p_bubble);
    let expected_bo = bo_bubble + t * (bo_undersat - bo_bubble);
    let expected_mu = mu_bubble + t * (mu_undersat - mu_bubble);
    let (bo, mu) = table.interpolate_oil(p_initial, branch_rs);

    assert!(
        (bo - expected_bo).abs() < 1e-12,
        "tabular Bo mismatch: {bo} vs {expected_bo}"
    );
    assert!(
        (mu - expected_mu).abs() < 1e-12,
        "tabular mu mismatch: {mu} vs {expected_mu}"
    );
}

#[test]
fn transported_free_gas_does_not_redissolve_into_oil_when_disabled() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_three_phase_mode_enabled(true);
    sim.set_gas_redissolution_enabled(false);
    sim.set_cell_dimensions(1.0, 1.0, 1.0).unwrap();
    sim.set_initial_pressure(175.0);
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
                bo_m3m3: 1.119,
                mu_o_cp: 1.3,
                bg_m3m3: 0.0045,
                mu_g_cp: 0.03,
            },
        ],
        sim.pvt.c_o,
    ));

    let pressure_bar = 175.0;
    let water_saturation = 0.1;
    let pore_volume_m3 = sim.pore_volume_m3(0);
    let transported_free_gas_sc = 10.0;
    let dissolved_gas_sc = 1.0;

    let (sg, so, rs) = sim.split_gas_inventory_after_transport(
        pressure_bar,
        pore_volume_m3,
        water_saturation,
        transported_free_gas_sc,
        dissolved_gas_sc,
    );

    let bg = sim.get_b_g(pressure_bar).max(1e-9);
    let bo = sim.get_b_o_for_rs(pressure_bar, rs).max(1e-9);
    let dissolved_after_sc = (so * pore_volume_m3 / bo) * rs;
    let free_after_sc = sg * pore_volume_m3 / bg;

    assert!(
        (dissolved_after_sc - dissolved_gas_sc).abs() < 1e-8,
        "free gas should not redissolve when disabled: dissolved_after_sc={} dissolved_input={}",
        dissolved_after_sc,
        dissolved_gas_sc,
    );
    assert!(
        (free_after_sc - transported_free_gas_sc).abs() < 1e-8,
        "transported free gas should remain free when redissolution is disabled: free_after_sc={} transported_free_gas_sc={}",
        free_after_sc,
        transported_free_gas_sc,
    );
}
