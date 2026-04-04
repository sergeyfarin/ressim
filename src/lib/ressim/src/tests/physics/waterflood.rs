use super::fixtures::make_short_waterflood_1d_sim;

#[derive(Clone, Copy)]
struct BuckleyBenchmarkCase {
    name: &'static str,
    nx: usize,
    permeability_md: f64,
    dt_days: f64,
    max_steps: usize,
    injector_bhp: f64,
    producer_bhp: f64,
    s_wc: f64,
    s_or: f64,
    n_w: f64,
    n_o: f64,
    mu_w: f64,
    mu_o: f64,
    breakthrough_watercut: f64,
}

struct BuckleyBenchmarkMetrics {
    breakthrough_pv: f64,
    reference_breakthrough_pv: f64,
}

struct BuckleyProfileMetrics {
    cumulative_injection_pv: f64,
    avg_sw: f64,
    producer_sw: f64,
    pressure_drop_bar: f64,
    water_sat_l1: f64,
    final_time_days: f64,
    history_len: usize,
    last_solver_warning: String,
}

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
    sim.set_fluid_properties(case.mu_o_cp, case.mu_w_cp)
        .unwrap();
    sim.set_capillary_params(case.pc_entry_bar, 2.0).unwrap();
    sim.set_permeability_random_seeded(case.perm_md, case.perm_md, 42)
        .unwrap();
    sim
}

fn cumulative_oil_production_sc(sim: &crate::ReservoirSimulator) -> f64 {
    let mut cumulative_oil = 0.0;
    let mut previous_time_days = 0.0;

    for point in &sim.rate_history {
        let dt_days = point.time - previous_time_days;
        previous_time_days = point.time;
        cumulative_oil += point.total_production_oil.max(0.0) * dt_days;
    }

    cumulative_oil
}

fn buckley_case_a(
    name: &'static str,
    nx: usize,
    dt_days: f64,
    max_steps: usize,
) -> BuckleyBenchmarkCase {
    BuckleyBenchmarkCase {
        name,
        nx,
        permeability_md: 2000.0,
        dt_days,
        max_steps,
        injector_bhp: 500.0,
        producer_bhp: 100.0,
        s_wc: 0.1,
        s_or: 0.1,
        n_w: 2.0,
        n_o: 2.0,
        mu_w: 0.5,
        mu_o: 1.0,
        breakthrough_watercut: 0.01,
    }
}

fn buckley_case_b(
    name: &'static str,
    nx: usize,
    dt_days: f64,
    max_steps: usize,
) -> BuckleyBenchmarkCase {
    BuckleyBenchmarkCase {
        name,
        nx,
        permeability_md: 2000.0,
        dt_days,
        max_steps,
        injector_bhp: 500.0,
        producer_bhp: 100.0,
        s_wc: 0.15,
        s_or: 0.15,
        n_w: 2.2,
        n_o: 2.0,
        mu_w: 0.6,
        mu_o: 1.4,
        breakthrough_watercut: 0.01,
    }
}

fn corey_fractional_flow(
    s_w: f64,
    s_wc: f64,
    s_or: f64,
    n_w: f64,
    n_o: f64,
    mu_w: f64,
    mu_o: f64,
) -> f64 {
    let denom_sat = 1.0 - s_wc - s_or;
    if denom_sat <= 0.0 {
        return 0.0;
    }

    let s_eff_w = ((s_w - s_wc) / denom_sat).clamp(0.0, 1.0);
    let s_eff_o = ((1.0 - s_w - s_or) / denom_sat).clamp(0.0, 1.0);
    let krw = s_eff_w.powf(n_w);
    let kro = s_eff_o.powf(n_o);
    let lam_w = krw / mu_w;
    let lam_o = kro / mu_o;
    let lam_t = lam_w + lam_o;

    if lam_t <= f64::EPSILON {
        0.0
    } else {
        (lam_w / lam_t).clamp(0.0, 1.0)
    }
}

fn buckley_reference_breakthrough_pv(case: &BuckleyBenchmarkCase) -> f64 {
    let sw_init = case.s_wc;
    let mut best_slope = 0.0;
    let mut sw_shock = sw_init;
    let ds = 5e-4;
    let mut s = sw_init + ds;
    let s_max = 1.0 - case.s_or;

    while s <= s_max {
        let fw = corey_fractional_flow(
            s, case.s_wc, case.s_or, case.n_w, case.n_o, case.mu_w, case.mu_o,
        );
        let slope = fw / (s - sw_init);
        if slope > best_slope && slope.is_finite() {
            best_slope = slope;
            sw_shock = s;
        }
        s += ds;
    }

    let fw_eps = 1e-4;
    let fw_plus = corey_fractional_flow(
        (sw_shock + fw_eps).clamp(sw_init, s_max),
        case.s_wc,
        case.s_or,
        case.n_w,
        case.n_o,
        case.mu_w,
        case.mu_o,
    );
    let fw_minus = corey_fractional_flow(
        (sw_shock - fw_eps).clamp(sw_init, s_max),
        case.s_wc,
        case.s_or,
        case.n_w,
        case.n_o,
        case.mu_w,
        case.mu_o,
    );
    let dfw_dsw = (fw_plus - fw_minus) / (2.0 * fw_eps);

    if dfw_dsw <= f64::EPSILON {
        f64::INFINITY
    } else {
        1.0 / dfw_dsw
    }
}

fn build_buckley_simulator(
    case: &BuckleyBenchmarkCase,
    fim_enabled: bool,
) -> crate::ReservoirSimulator {
    let mut sim = crate::ReservoirSimulator::new(case.nx, 1, 1, 0.2);
    sim.set_fim_enabled(fim_enabled);
    sim.set_rel_perm_props(case.s_wc, case.s_or, case.n_w, case.n_o, 1.0, 1.0)
        .unwrap();
    sim.set_initial_saturation(case.s_wc);
    sim.set_permeability_random_seeded(case.permeability_md, case.permeability_md, 42)
        .unwrap();
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.pc.p_entry = 0.0;
    sim.pvt.mu_w = case.mu_w;
    sim.pvt.mu_o = case.mu_o;
    sim.add_well(0, 0, 0, case.injector_bhp, 0.1, 0.0, true)
        .unwrap();
    sim.add_well(case.nx - 1, 0, 0, case.producer_bhp, 0.1, 0.0, false)
        .unwrap();
    sim
}

fn run_buckley_case(case: &BuckleyBenchmarkCase) -> BuckleyBenchmarkMetrics {
    let mut sim = build_buckley_simulator(case, false);
    let total_pv = (0..sim.nx * sim.ny * sim.nz)
        .map(|i| sim.pore_volume_m3(i))
        .sum::<f64>();
    let mut cumulative_injection = 0.0;
    let mut previous_time = 0.0;
    let mut breakthrough_pv = None;

    for _ in 0..case.max_steps {
        sim.step(case.dt_days);
        let point = sim
            .rate_history
            .last()
            .expect("rate history should have entries");
        let dt = point.time - previous_time;
        previous_time = point.time;
        cumulative_injection += point.total_injection.max(0.0) * dt;

        if point.total_production_liquid > 1e-9 {
            let water_rate = (point.total_production_liquid - point.total_production_oil).max(0.0);
            let watercut = (water_rate / point.total_production_liquid).clamp(0.0, 1.0);
            if watercut >= case.breakthrough_watercut {
                breakthrough_pv = Some(cumulative_injection / total_pv);
                break;
            }
        }
    }

    let breakthrough_pv = breakthrough_pv.unwrap_or_else(|| {
        panic!(
            "{} did not reach breakthrough (watercut >= {}) in {} steps",
            case.name, case.breakthrough_watercut, case.max_steps
        )
    });

    BuckleyBenchmarkMetrics {
        breakthrough_pv,
        reference_breakthrough_pv: buckley_reference_breakthrough_pv(case),
    }
}

fn run_buckley_profile_case(
    case: &BuckleyBenchmarkCase,
    fim_enabled: bool,
    step_count: usize,
) -> BuckleyProfileMetrics {
    let mut sim = build_buckley_simulator(case, fim_enabled);
    let total_pv = (0..sim.nx * sim.ny * sim.nz)
        .map(|i| sim.pore_volume_m3(i))
        .sum::<f64>();
    let mut cumulative_injection = 0.0;
    let mut previous_time = 0.0;

    for _ in 0..step_count {
        sim.step(case.dt_days);
        let point = sim
            .rate_history
            .last()
            .expect("rate history should have entries");
        let dt = point.time - previous_time;
        previous_time = point.time;
        cumulative_injection += point.total_injection.max(0.0) * dt;
    }

    let cell_count = sim.nx * sim.ny * sim.nz;
    let avg_sw = sim.sat_water.iter().copied().sum::<f64>() / cell_count as f64;
    let producer_id = sim.idx(sim.nx - 1, 0, 0);
    let pressure_drop_bar = (sim.pressure[sim.idx(0, 0, 0)] - sim.pressure[producer_id]).abs();
    let water_sat_l1 = sim
        .sat_water
        .iter()
        .map(|&sw| (sw - case.s_wc).abs())
        .sum::<f64>()
        / cell_count as f64;

    BuckleyProfileMetrics {
        cumulative_injection_pv: cumulative_injection / total_pv,
        avg_sw,
        producer_sw: sim.sat_water[producer_id],
        pressure_drop_bar,
        water_sat_l1,
        final_time_days: sim.time_days,
        history_len: sim.rate_history.len(),
        last_solver_warning: sim.last_solver_warning.clone(),
    }
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
fn physics_waterflood_1d_public_reporting_contract_holds_on_both_solvers() {
    fn run_case(fim_enabled: bool) -> (f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, usize) {
        let mut sim = make_short_waterflood_1d_sim();
        sim.set_fim_enabled(fim_enabled);
        let initial_avg_sw = sim.sat_water.iter().copied().sum::<f64>() / sim.sat_water.len() as f64;
        let injector_cell = sim.idx(0, 0, 0);
        let producer_cell = sim.idx(sim.nx - 1, 0, 0);

        for _ in 0..12 {
            sim.step(0.25);
            assert!(
                sim.last_solver_warning.is_empty(),
                "two-solver waterflood public-contract case emitted solver warning for fim_enabled={}: {}",
                fim_enabled,
                sim.last_solver_warning
            );
        }

        let latest = sim
            .rate_history
            .last()
            .expect("two-solver waterflood public-contract case should record history");
        let final_avg_sw = sim.sat_water.iter().copied().sum::<f64>() / sim.sat_water.len() as f64;

        (
            initial_avg_sw,
            final_avg_sw,
            sim.sat_water[injector_cell],
            sim.sat_water[producer_cell],
            latest.total_injection,
            latest.total_production_liquid,
            latest.total_production_oil,
            latest.material_balance_error_m3,
            latest.producer_bhp_limited_fraction,
            latest.injector_bhp_limited_fraction,
            latest.time,
            sim.rate_history.len(),
        )
    }

    for (fim_enabled, metrics) in [(false, run_case(false)), (true, run_case(true))] {
        assert!(metrics.1 > metrics.0 + 1e-4);
        assert!(metrics.2 > metrics.3);
        assert!(metrics.4 > 0.0);
        assert!(metrics.5 > 0.0);
        assert!(metrics.6 >= 0.0);
        assert!(metrics.6 <= metrics.5 + 1e-9);
        assert!(metrics.7.is_finite());
        assert!((0.0..=1.0).contains(&metrics.8));
        assert!((0.0..=1.0).contains(&metrics.9));
        assert!((metrics.10 - 3.0).abs() <= 1e-9);
        assert!(metrics.11 > 0, "expected rate history for fim_enabled={}", fim_enabled);
    }
}

#[test]
fn physics_waterflood_case_matrix_respects_mass_and_front_direction_across_scal_and_capillary_ranges()
 {
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
        let initial_avg_sw =
            sim.sat_water.iter().copied().sum::<f64>() / sim.sat_water.len() as f64;
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

#[test]
#[ignore = "explicit refinement probe: short 1D waterflood should stay directionally stable under timestep refinement"]
fn physics_waterflood_1d_timestep_refinement_keeps_front_and_balance_stable() {
    let mut coarse = make_short_waterflood_1d_sim();
    let mut fine = make_short_waterflood_1d_sim();

    for _ in 0..12 {
        coarse.step(0.25);
        assert!(
            coarse.last_solver_warning.is_empty(),
            "coarse 1D waterflood emitted solver warning at t={}: {}",
            coarse.time_days,
            coarse.last_solver_warning
        );
    }
    for _ in 0..24 {
        fine.step(0.125);
        assert!(
            fine.last_solver_warning.is_empty(),
            "fine 1D waterflood emitted solver warning at t={}: {}",
            fine.time_days,
            fine.last_solver_warning
        );
    }

    let coarse_avg_sw =
        coarse.sat_water.iter().copied().sum::<f64>() / coarse.sat_water.len() as f64;
    let fine_avg_sw = fine.sat_water.iter().copied().sum::<f64>() / fine.sat_water.len() as f64;
    let coarse_last = coarse
        .rate_history
        .last()
        .expect("coarse waterflood should record history");
    let fine_last = fine
        .rate_history
        .last()
        .expect("fine waterflood should record history");
    let coarse_cum_oil = cumulative_oil_production_sc(&coarse);
    let fine_cum_oil = cumulative_oil_production_sc(&fine);

    let avg_sw_abs_diff = (coarse_avg_sw - fine_avg_sw).abs();
    let pressure_rel_diff = ((coarse_last.avg_reservoir_pressure
        - fine_last.avg_reservoir_pressure)
        / fine_last.avg_reservoir_pressure.max(1e-12))
    .abs();
    let cumulative_oil_rel_diff = ((coarse_cum_oil - fine_cum_oil) / fine_cum_oil.max(1e-12)).abs();

    assert!(
        avg_sw_abs_diff <= 0.03,
        "1D waterflood average Sw drift too large under timestep refinement: coarse={:.6}, fine={:.6}, abs_diff={:.6}",
        coarse_avg_sw,
        fine_avg_sw,
        avg_sw_abs_diff
    );
    assert!(
        pressure_rel_diff <= 0.03,
        "1D waterflood avg-pressure drift too large under timestep refinement: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_last.avg_reservoir_pressure,
        fine_last.avg_reservoir_pressure,
        pressure_rel_diff
    );
    assert!(
        cumulative_oil_rel_diff <= 0.08,
        "1D waterflood cumulative-oil drift too large under timestep refinement: coarse={:.6}, fine={:.6}, rel_diff={:.4}",
        coarse_cum_oil,
        fine_cum_oil,
        cumulative_oil_rel_diff
    );
    assert!(
        coarse_last.material_balance_error_m3 <= 2e-2,
        "coarse 1D waterflood MB drift too large under timestep refinement: {}",
        coarse_last.material_balance_error_m3
    );
    assert!(
        fine_last.material_balance_error_m3 <= 5e-2,
        "fine 1D waterflood MB drift too large under timestep refinement: {}",
        fine_last.material_balance_error_m3
    );
}

#[test]
#[ignore = "diagnostic benchmark-parity probe: Buckley case A early FIM profile should stay within the current IMPES envelope while the known parity gap remains open"]
fn physics_waterflood_buckley_case_a_fim_matches_impes_early_profile() {
    let case = buckley_case_a("BL-Case-A-FIM-Early", 24, 0.125, 128);
    let step_count = 8;

    let impes = run_buckley_profile_case(&case, false, step_count);
    let fim = run_buckley_profile_case(&case, true, step_count);

    let injection_rel_diff = ((fim.cumulative_injection_pv - impes.cumulative_injection_pv)
        / impes.cumulative_injection_pv.max(1e-12))
    .abs();
    let avg_sw_abs_diff = (fim.avg_sw - impes.avg_sw).abs();
    let producer_sw_abs_diff = (fim.producer_sw - impes.producer_sw).abs();
    let pressure_drop_rel_diff = ((fim.pressure_drop_bar - impes.pressure_drop_bar)
        / impes.pressure_drop_bar.max(1e-12))
    .abs();
    let transport_activity_rel_diff =
        ((fim.water_sat_l1 - impes.water_sat_l1) / impes.water_sat_l1.max(1e-12)).abs();

    assert!(
        injection_rel_diff <= 0.20,
        "{} injection PV drift too large: IMPES={:.4}, FIM={:.4}, rel_diff={:.3}",
        case.name,
        impes.cumulative_injection_pv,
        fim.cumulative_injection_pv,
        injection_rel_diff,
    );
    assert!(
        avg_sw_abs_diff <= 0.08,
        "{} avg Sw drift too large: IMPES={:.4}, FIM={:.4}, abs_diff={:.4}",
        case.name,
        impes.avg_sw,
        fim.avg_sw,
        avg_sw_abs_diff,
    );
    assert!(
        producer_sw_abs_diff <= 0.20,
        "{} producer-cell Sw drift too large: IMPES={:.4}, FIM={:.4}, abs_diff={:.4}",
        case.name,
        impes.producer_sw,
        fim.producer_sw,
        producer_sw_abs_diff,
    );
    assert!(
        pressure_drop_rel_diff <= 0.35,
        "{} pressure-drop drift too large: IMPES={:.3}, FIM={:.3}, rel_diff={:.3}",
        case.name,
        impes.pressure_drop_bar,
        fim.pressure_drop_bar,
        pressure_drop_rel_diff,
    );
    assert!(
        transport_activity_rel_diff <= 0.45,
        "{} transport activity drift too large: IMPES satL1={:.4}, FIM satL1={:.4}, rel_diff={:.3}",
        case.name,
        impes.water_sat_l1,
        fim.water_sat_l1,
        transport_activity_rel_diff,
    );
    assert!(fim.last_solver_warning.is_empty());
    assert!(impes.last_solver_warning.is_empty());
    assert!(fim.final_time_days > 0.0);
    assert!(impes.final_time_days > 0.0);
    assert!(fim.history_len > 0);
    assert!(impes.history_len > 0);
}

#[test]
#[ignore = "larger-grid benchmark probe: refined Buckley discretization should not worsen analytical alignment"]
fn physics_waterflood_buckley_refined_discretization_improves_alignment() {
    let coarse_a = buckley_case_a("BL-Case-A-Coarse", 24, 0.5, 4000);
    let refined_a = buckley_case_a("BL-Case-A-Refined", 96, 0.125, 20000);
    let coarse_b = buckley_case_b("BL-Case-B-Coarse", 24, 0.25, 4000);
    let refined_b = buckley_case_b("BL-Case-B-Refined", 96, 0.125, 20000);

    let metrics_coarse_a = run_buckley_case(&coarse_a);
    let metrics_refined_a = run_buckley_case(&refined_a);
    let rel_err_coarse_a = ((metrics_coarse_a.breakthrough_pv
        - metrics_coarse_a.reference_breakthrough_pv)
        / metrics_coarse_a.reference_breakthrough_pv)
        .abs();
    let rel_err_refined_a = ((metrics_refined_a.breakthrough_pv
        - metrics_refined_a.reference_breakthrough_pv)
        / metrics_refined_a.reference_breakthrough_pv)
        .abs();

    let metrics_coarse_b = run_buckley_case(&coarse_b);
    let metrics_refined_b = run_buckley_case(&refined_b);
    let rel_err_coarse_b = ((metrics_coarse_b.breakthrough_pv
        - metrics_coarse_b.reference_breakthrough_pv)
        / metrics_coarse_b.reference_breakthrough_pv)
        .abs();
    let rel_err_refined_b = ((metrics_refined_b.breakthrough_pv
        - metrics_refined_b.reference_breakthrough_pv)
        / metrics_refined_b.reference_breakthrough_pv)
        .abs();

    assert!(
        rel_err_refined_a <= rel_err_coarse_a,
        "Refined discretization should not worsen Case-A alignment: coarse={:.3}, refined={:.3}",
        rel_err_coarse_a,
        rel_err_refined_a
    );
    assert!(
        rel_err_refined_b <= rel_err_coarse_b,
        "Refined discretization should not worsen Case-B alignment: coarse={:.3}, refined={:.3}",
        rel_err_coarse_b,
        rel_err_refined_b
    );
}
