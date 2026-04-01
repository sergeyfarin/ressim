use super::fixtures::make_short_waterflood_1d_sim;

#[derive(Clone, Copy)]
struct WaterfloodCase {
    name: &'static str,
    initial_sw: f64,
    perm_md: f64,
    mu_o_cp: f64,
    mu_w_cp: f64,
    s_wc: f64,
    s_or: f64,
    n_w: f64,
    n_o: f64,
    pc_entry_bar: f64,
}

fn make_waterflood_case(case: WaterfloodCase) -> crate::ReservoirSimulator {
    let mut sim = make_short_waterflood_1d_sim();
    sim.set_rel_perm_props(case.s_wc, case.s_or, case.n_w, case.n_o, 1.0, 1.0)
        .unwrap();
    sim.set_initial_saturation(case.initial_sw);
    sim.set_fluid_properties(case.mu_o_cp, case.mu_w_cp).unwrap();
    sim.set_capillary_params(case.pc_entry_bar, 2.0).unwrap();
    sim.set_permeability_random_seeded(case.perm_md, case.perm_md, 42)
        .unwrap();
    sim
}

#[test]
fn physics_waterflood_1d_mass_conservative() {
    let mut sim = make_short_waterflood_1d_sim();
    let initial_avg_sw = sim.sat_water.iter().copied().sum::<f64>() / sim.sat_water.len() as f64;

    for _ in 0..12 {
        sim.step(0.25);
        assert!(
            sim.last_solver_warning.is_empty(),
            "1D waterflood emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    let latest = sim
        .rate_history
        .last()
        .expect("1D waterflood should record history");
    let avg_sw = sim.sat_water.iter().copied().sum::<f64>() / sim.sat_water.len() as f64;
    let injector_cell = sim.idx(0, 0, 0);
    let producer_cell = sim.idx(sim.nx - 1, 0, 0);

    assert!(avg_sw > initial_avg_sw + 1e-4);
    assert!(sim.sat_water[injector_cell] > sim.sat_water[producer_cell]);
    assert!(latest.total_injection > 0.0);
    assert!(latest.total_production_liquid > 0.0);
    assert!(
        latest.material_balance_error_m3 <= 1e-2,
        "1D waterflood water MB drift too large: {}",
        latest.material_balance_error_m3
    );
}

#[test]
fn physics_waterflood_1d_injector_saturation_increases() {
    let mut sim = make_short_waterflood_1d_sim();
    let injector = sim.idx(0, 0, 0);
    let mut prev_sw = sim.sat_water[injector];

    for _ in 0..12 {
        sim.step(0.25);
        assert!(
            sim.last_solver_warning.is_empty(),
            "1D waterflood emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
        let sw = sim.sat_water[injector];
        assert!(
            sw >= prev_sw - 1e-9,
            "injector cell Sw must not decrease during waterflood: \
             prev={:.6}, now={:.6} at t={}",
            prev_sw,
            sw,
            sim.time_days
        );
        prev_sw = sw;
    }

    assert!(
        prev_sw > 0.3,
        "injector cell Sw should reach significant water saturation after injection, got {:.4}",
        prev_sw
    );
}

#[test]
fn physics_waterflood_case_matrix_respects_mass_and_front_direction_across_scal_and_capillary_ranges() {
    let cases = [
        WaterfloodCase {
            name: "base",
            initial_sw: 0.10,
            perm_md: 2_000.0,
            mu_o_cp: 1.0,
            mu_w_cp: 0.5,
            s_wc: 0.10,
            s_or: 0.10,
            n_w: 2.0,
            n_o: 2.0,
            pc_entry_bar: 0.0,
        },
        WaterfloodCase {
            name: "higher connate water with capillary",
            initial_sw: 0.18,
            perm_md: 750.0,
            mu_o_cp: 1.5,
            mu_w_cp: 0.45,
            s_wc: 0.15,
            s_or: 0.12,
            n_w: 2.5,
            n_o: 1.8,
            pc_entry_bar: 2.0,
        },
        WaterfloodCase {
            name: "more favorable water mobility",
            initial_sw: 0.10,
            perm_md: 2_500.0,
            mu_o_cp: 1.5,
            mu_w_cp: 0.5,
            s_wc: 0.08,
            s_or: 0.15,
            n_w: 1.8,
            n_o: 2.2,
            pc_entry_bar: 0.5,
        },
    ];

    for case in cases {
        let mut sim = make_waterflood_case(case);
        let initial_avg_sw = sim.sat_water.iter().copied().sum::<f64>() / sim.sat_water.len() as f64;
        let injector_cell = sim.idx(0, 0, 0);
        let producer_cell = sim.idx(sim.nx - 1, 0, 0);

        for _ in 0..6 {
            sim.step(0.125);
            assert!(
                sim.last_solver_warning.is_empty(),
                "{} waterflood emitted solver warning at t={}: {}",
                case.name,
                sim.time_days,
                sim.last_solver_warning
            );
        }

        let latest = sim
            .rate_history
            .last()
            .expect("waterflood case matrix should record history");
        let avg_sw = sim.sat_water.iter().copied().sum::<f64>() / sim.sat_water.len() as f64;

        assert!(
            avg_sw > initial_avg_sw + 1e-4,
            "{} average Sw should increase: initial={:.6}, final={:.6}",
            case.name,
            initial_avg_sw,
            avg_sw
        );
        assert!(
            sim.sat_water[injector_cell] > sim.sat_water[producer_cell],
            "{} injector cell should stay wetter than producer cell: inj={:.6}, prod={:.6}",
            case.name,
            sim.sat_water[injector_cell],
            sim.sat_water[producer_cell]
        );
        assert!(latest.total_injection > 0.0);
        assert!(latest.total_production_liquid > 0.0);
        assert!(
            latest.material_balance_error_m3 <= 3e-2,
            "{} water MB drift too large: {}",
            case.name,
            latest.material_balance_error_m3
        );
    }
}