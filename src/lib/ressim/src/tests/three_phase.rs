use super::*;

#[test]
fn three_phase_relperm_k_ro_stone2_endpoints() {
    use crate::relperm::RockFluidPropsThreePhase;

    let rock = RockFluidPropsThreePhase {
        s_wc: 0.10,
        s_or: 0.10,
        n_w: 2.0,
        n_o: 2.0,
        k_rw_max: 0.8,
        k_ro_max: 0.9,
        s_gc: 0.05,
        s_gr: 0.05,
        s_org: 0.10,
        n_g: 1.5,
        k_rg_max: 0.7,
        tables: None,
    };

    let kro_at_swc = rock.k_ro_stone2(rock.s_wc, 0.0);
    assert!((kro_at_swc - rock.k_ro_max).abs() < 1e-10);

    let sg_at_sorg = 1.0 - rock.s_wc - rock.s_org;
    let kro_max_gas = rock.k_ro_stone2(rock.s_wc, sg_at_sorg);
    assert!(kro_max_gas < 1e-9);

    let kro_at_max_water = rock.k_ro_stone2(1.0 - rock.s_or, 0.0);
    assert!(kro_at_max_water < 1e-9);

    for i in 0..=20 {
        let sw = rock.s_wc + i as f64 * (1.0 - rock.s_wc - rock.s_or) / 20.0;
        for j in 0..=20 {
            let sg = j as f64 * (1.0 - rock.s_wc - rock.s_gr) / 20.0;
            if sw + sg <= 1.0 {
                let kro = rock.k_ro_stone2(sw, sg);
                assert!(kro >= -1e-10);
                assert!(kro <= rock.k_ro_max + 1e-10);
            }
        }
    }
}

#[test]
fn three_phase_relperm_k_rg_endpoints_and_monotonicity() {
    use crate::relperm::RockFluidPropsThreePhase;

    let rock = RockFluidPropsThreePhase {
        s_wc: 0.10,
        s_or: 0.10,
        n_w: 2.0,
        n_o: 2.0,
        k_rw_max: 0.8,
        k_ro_max: 0.9,
        s_gc: 0.05,
        s_gr: 0.05,
        s_org: 0.10,
        n_g: 2.0,
        k_rg_max: 0.7,
        tables: None,
    };

    assert_eq!(rock.k_rg(0.0), 0.0);
    assert_eq!(rock.k_rg(rock.s_gc * 0.5), 0.0);
    assert!(rock.k_rg(rock.s_gc) < 1e-10);

    let sg_at_kmax = 1.0 - rock.s_wc - rock.s_gr;
    let krg_at_max = rock.k_rg(sg_at_kmax);
    assert!((krg_at_max - rock.k_rg_max).abs() < 1e-10);

    let mut prev_krg = 0.0;
    let n = 50;
    for i in 0..=n {
        let sg = rock.s_gc + i as f64 * (sg_at_kmax - rock.s_gc) / n as f64;
        let krg = rock.k_rg(sg);
        assert!(krg >= prev_krg - 1e-12);
        prev_krg = krg;
    }
}

#[test]
fn three_phase_relperm_stone2_reduces_to_two_phase_at_zero_gas() {
    use crate::relperm::RockFluidPropsThreePhase;

    let rock = RockFluidPropsThreePhase {
        s_wc: 0.10,
        s_or: 0.10,
        n_w: 2.0,
        n_o: 2.0,
        k_rw_max: 0.8,
        k_ro_max: 0.9,
        s_gc: 0.05,
        s_gr: 0.05,
        s_org: 0.10,
        n_g: 1.5,
        k_rg_max: 0.7,
        tables: None,
    };

    let sw_vals = [0.10, 0.20, 0.30, 0.50, 0.70, 0.85, 0.90];
    for &sw in &sw_vals {
        let kro_stone2 = rock.k_ro_stone2(sw, 0.0);
        let kro_ow = rock.k_ro_water(sw);
        assert!((kro_stone2 - kro_ow).abs() < 1e-10);
    }
}

#[test]
fn three_phase_relperm_tables_interpolate_exact_spe1_points() {
    use crate::relperm::{RockFluidPropsThreePhase, SgofRow, SwofRow, ThreePhaseScalTables};

    let rock = RockFluidPropsThreePhase {
        s_wc: 0.12,
        s_or: 0.12,
        n_w: 2.0,
        n_o: 2.5,
        k_rw_max: 1e-5,
        k_ro_max: 1.0,
        s_gc: 0.04,
        s_gr: 0.04,
        s_org: 0.18,
        n_g: 1.5,
        k_rg_max: 0.984,
        tables: Some(ThreePhaseScalTables {
            swof: vec![
                SwofRow {
                    sw: 0.12,
                    krw: 0.0,
                    krow: 1.0,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 0.24,
                    krw: 1.86e-7,
                    krow: 0.997,
                    pcow: Some(0.0),
                },
                SwofRow {
                    sw: 1.0,
                    krw: 1e-5,
                    krow: 0.0,
                    pcow: Some(0.0),
                },
            ],
            sgof: vec![
                SgofRow {
                    sg: 0.0,
                    krg: 0.0,
                    krog: 1.0,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.5,
                    krg: 0.72,
                    krog: 0.001,
                    pcog: Some(0.0),
                },
                SgofRow {
                    sg: 0.88,
                    krg: 0.984,
                    krog: 0.0,
                    pcog: Some(0.0),
                },
            ],
        }),
    };

    assert!((rock.k_rw(0.12) - 0.0).abs() < 1e-12);
    assert!((rock.k_ro_water(0.24) - 0.997).abs() < 1e-12);
    assert!((rock.k_rg(0.5) - 0.72).abs() < 1e-12);
    assert!((rock.k_ro_gas(0.5) - 0.001).abs() < 1e-12);
}

#[test]
fn three_phase_scal_tables_validate_valid_spe1_fragment() {
    use crate::relperm::{SgofRow, SwofRow, ThreePhaseScalTables};

    let tables = ThreePhaseScalTables {
        swof: vec![
            SwofRow {
                sw: 0.12,
                krw: 0.0,
                krow: 1.0,
                pcow: Some(0.0),
            },
            SwofRow {
                sw: 1.0,
                krw: 1e-5,
                krow: 0.0,
                pcow: Some(0.0),
            },
        ],
        sgof: vec![
            SgofRow {
                sg: 0.0,
                krg: 0.0,
                krog: 1.0,
                pcog: Some(0.0),
            },
            SgofRow {
                sg: 0.88,
                krg: 0.984,
                krog: 0.0,
                pcog: Some(0.0),
            },
        ],
    };

    assert!(tables.validate().is_ok());
}

#[test]
fn api_contract_rejects_invalid_three_phase_relperm_parameters() {
    let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);

    err_contains(
        sim.set_three_phase_rel_perm_props(
            0.1, 0.1, 0.05, 0.05, 0.10, 0.0, 2.0, 1.5, 1.0, 1.0, 0.7,
        ),
        "must be positive",
    );
    err_contains(
        sim.set_three_phase_rel_perm_props(
            0.1, 0.1, 0.05, 0.05, 0.10, 2.0, 2.0, 0.0, 1.0, 1.0, 0.7,
        ),
        "must be positive",
    );
}

#[test]
fn api_contract_rejects_invalid_gas_fluid_properties() {
    let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);

    err_contains(
        sim.set_gas_fluid_properties(0.0, 1e-4, 10.0),
        "must be positive",
    );
    err_contains(
        sim.set_gas_fluid_properties(-0.01, 1e-4, 10.0),
        "must be positive",
    );
    err_contains(
        sim.set_gas_fluid_properties(0.02, -1e-4, 10.0),
        "non-negative",
    );
    err_contains(
        sim.set_gas_fluid_properties(0.02, 1e-4, 0.0),
        "must be positive",
    );
}

#[test]
fn api_contract_rejects_invalid_injected_fluid_string() {
    let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);

    err_contains(sim.set_injected_fluid("steam"), "Unknown injected fluid");
    err_contains(sim.set_injected_fluid(""), "Unknown injected fluid");
    assert!(sim.set_injected_fluid("water").is_ok());
    assert!(sim.set_injected_fluid("gas").is_ok());
}

#[test]
fn api_contract_rejects_invalid_gas_oil_capillary_params() {
    let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);

    err_contains(sim.set_gas_oil_capillary_params(-1.0, 2.0), "non-negative");
    err_contains(sim.set_gas_oil_capillary_params(5.0, 0.0), "positive");
    assert!(sim.set_gas_oil_capillary_params(0.0, 2.0).is_ok());
    assert!(sim.set_gas_oil_capillary_params(5.0, 1.5).is_ok());
}

#[test]
fn set_initial_gas_saturation_per_layer_applies_by_k() {
    let mut sim = ReservoirSimulator::new(2, 2, 3, 0.2);
    sim.set_initial_saturation(0.2);
    sim.set_initial_gas_saturation_per_layer(vec![0.7, 0.0, 0.0])
        .unwrap();

    for j in 0..2 {
        for i in 0..2 {
            let id = sim.idx(i, j, 0);
            assert!((sim.sat_gas[id] - 0.7).abs() < 1e-10);
            assert!((sim.sat_oil[id] - 0.1).abs() < 1e-10);
        }
    }

    for k in 1..3 {
        for j in 0..2 {
            for i in 0..2 {
                let id = sim.idx(i, j, k);
                assert!((sim.sat_gas[id] - 0.0).abs() < 1e-10);
                assert!((sim.sat_oil[id] - 0.8).abs() < 1e-10);
            }
        }
    }
}

#[test]
fn set_initial_gas_saturation_per_layer_clamps_to_available() {
    let mut sim = ReservoirSimulator::new(1, 1, 2, 0.2);
    sim.set_initial_saturation(0.5);
    sim.set_initial_gas_saturation_per_layer(vec![0.8, 0.0])
        .unwrap();

    let id0 = sim.idx(0, 0, 0);
    assert!((sim.sat_gas[id0] - 0.5).abs() < 1e-10);
    assert!((sim.sat_oil[id0] - 0.0).abs() < 1e-10);
}

#[test]
fn set_initial_gas_saturation_per_layer_validation() {
    let mut sim = ReservoirSimulator::new(2, 2, 3, 0.2);

    err_contains(
        sim.set_initial_gas_saturation_per_layer(vec![0.5, 0.0]),
        "length equal to nz",
    );
    err_contains(
        sim.set_initial_gas_saturation_per_layer(vec![0.5, -0.1, 0.0]),
        "within [0, 1]",
    );
}
