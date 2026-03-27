use super::*;

struct BuckleyCase {
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
    rel_tol_breakthrough_pv: f64,
}

struct BuckleyMetrics {
    breakthrough_pv: f64,
    reference_breakthrough_pv: f64,
}

fn buckley_case_a(name: &'static str, nx: usize, dt_days: f64, max_steps: usize) -> BuckleyCase {
    BuckleyCase {
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
        rel_tol_breakthrough_pv: 0.25,
    }
}

fn buckley_case_b(name: &'static str, nx: usize, dt_days: f64, max_steps: usize) -> BuckleyCase {
    BuckleyCase {
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
        rel_tol_breakthrough_pv: 0.30,
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

fn buckley_reference_breakthrough_pv(case: &BuckleyCase) -> f64 {
    let sw_init = case.s_wc;
    let mut sw_shock = sw_init;
    let mut best_slope = 0.0;
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

fn run_buckley_case(case: &BuckleyCase) -> BuckleyMetrics {
    let mut sim = ReservoirSimulator::new(case.nx, 1, 1, 0.2);
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

    BuckleyMetrics {
        breakthrough_pv,
        reference_breakthrough_pv: buckley_reference_breakthrough_pv(case),
    }
}

#[test]
fn benchmark_buckley_leverett_case_a_favorable_mobility() {
    let case = buckley_case_a("BL-Case-A", 24, 0.5, 4000);

    let metrics = run_buckley_case(&case);
    let rel_err = ((metrics.breakthrough_pv - metrics.reference_breakthrough_pv)
        / metrics.reference_breakthrough_pv)
        .abs();

    println!(
        "{}: breakthrough_pv_sim={:.4}, breakthrough_pv_ref={:.4}, rel_err={:.3}",
        case.name, metrics.breakthrough_pv, metrics.reference_breakthrough_pv, rel_err
    );

    assert!(
        rel_err <= case.rel_tol_breakthrough_pv,
        "{} breakthrough PV mismatch too high: sim={:.4}, ref={:.4}, rel_err={:.3}, tol={:.3}",
        case.name,
        metrics.breakthrough_pv,
        metrics.reference_breakthrough_pv,
        rel_err,
        case.rel_tol_breakthrough_pv,
    );
}

#[test]
fn benchmark_buckley_leverett_case_b_more_adverse_mobility() {
    let case = buckley_case_b("BL-Case-B", 24, 0.25, 4000);

    let metrics = run_buckley_case(&case);
    let rel_err = ((metrics.breakthrough_pv - metrics.reference_breakthrough_pv)
        / metrics.reference_breakthrough_pv)
        .abs();

    println!(
        "{}: breakthrough_pv_sim={:.4}, breakthrough_pv_ref={:.4}, rel_err={:.3}",
        case.name, metrics.breakthrough_pv, metrics.reference_breakthrough_pv, rel_err
    );

    assert!(
        rel_err <= case.rel_tol_breakthrough_pv,
        "{} breakthrough PV mismatch too high: sim={:.4}, ref={:.4}, rel_err={:.3}, tol={:.3}",
        case.name,
        metrics.breakthrough_pv,
        metrics.reference_breakthrough_pv,
        rel_err,
        case.rel_tol_breakthrough_pv,
    );
}

#[test]
#[ignore = "slow discretization-sensitivity regression; run explicitly when tuning Buckley-Leverett numerics"]
fn benchmark_buckley_leverett_refined_discretization_improves_alignment() {
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

    println!(
        "Case-A coarse/refined rel_err: {:.3} -> {:.3}",
        rel_err_coarse_a, rel_err_refined_a
    );
    println!(
        "BL-Case-A-Refined: breakthrough_pv_sim={:.4}, breakthrough_pv_ref={:.4}, rel_err={:.3}",
        metrics_refined_a.breakthrough_pv,
        metrics_refined_a.reference_breakthrough_pv,
        rel_err_refined_a
    );
    println!(
        "Case-B coarse/refined rel_err: {:.3} -> {:.3}",
        rel_err_coarse_b, rel_err_refined_b
    );
    println!(
        "BL-Case-B-Refined: breakthrough_pv_sim={:.4}, breakthrough_pv_ref={:.4}, rel_err={:.3}",
        metrics_refined_b.breakthrough_pv,
        metrics_refined_b.reference_breakthrough_pv,
        rel_err_refined_b
    );

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

#[test]
fn benchmark_buckley_leverett_smaller_dt_improves_coarse_alignment() {
    let case_a_dt_050 = buckley_case_a("BL-Case-A-Coarse-dt0.50", 24, 0.5, 4000);
    let case_a_dt_025 = buckley_case_a("BL-Case-A-Coarse-dt0.25", 24, 0.25, 8000);
    let metrics_a_dt_050 = run_buckley_case(&case_a_dt_050);
    let metrics_a_dt_025 = run_buckley_case(&case_a_dt_025);
    let rel_err_a_dt_050 = ((metrics_a_dt_050.breakthrough_pv
        - metrics_a_dt_050.reference_breakthrough_pv)
        / metrics_a_dt_050.reference_breakthrough_pv)
        .abs();
    let rel_err_a_dt_025 = ((metrics_a_dt_025.breakthrough_pv
        - metrics_a_dt_025.reference_breakthrough_pv)
        / metrics_a_dt_025.reference_breakthrough_pv)
        .abs();

    let case_b_dt_050 = buckley_case_b("BL-Case-B-Coarse-dt0.50", 24, 0.5, 4000);
    let case_b_dt_025 = buckley_case_b("BL-Case-B-Coarse-dt0.25", 24, 0.25, 4000);
    let metrics_b_dt_050 = run_buckley_case(&case_b_dt_050);
    let metrics_b_dt_025 = run_buckley_case(&case_b_dt_025);
    let rel_err_b_dt_050 = ((metrics_b_dt_050.breakthrough_pv
        - metrics_b_dt_050.reference_breakthrough_pv)
        / metrics_b_dt_050.reference_breakthrough_pv)
        .abs();
    let rel_err_b_dt_025 = ((metrics_b_dt_025.breakthrough_pv
        - metrics_b_dt_025.reference_breakthrough_pv)
        / metrics_b_dt_025.reference_breakthrough_pv)
        .abs();

    println!(
        "Case-A coarse dt sweep rel_err: dt=0.50 -> {:.3}, dt=0.25 -> {:.3}",
        rel_err_a_dt_050, rel_err_a_dt_025
    );
    println!(
        "Case-B coarse dt sweep rel_err: dt=0.50 -> {:.3}, dt=0.25 -> {:.3}",
        rel_err_b_dt_050, rel_err_b_dt_025
    );

    assert!(
        rel_err_a_dt_025 + 1e-9 < rel_err_a_dt_050,
        "Smaller dt should improve Case-A coarse alignment: dt=0.50 -> {:.3}, dt=0.25 -> {:.3}",
        rel_err_a_dt_050,
        rel_err_a_dt_025
    );
    assert!(
        rel_err_b_dt_025 + 1e-9 < rel_err_b_dt_050,
        "Smaller dt should improve Case-B coarse alignment: dt=0.50 -> {:.3}, dt=0.25 -> {:.3}",
        rel_err_b_dt_050,
        rel_err_b_dt_025
    );
}
