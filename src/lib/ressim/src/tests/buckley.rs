use super::bench_record::{self, Metric};
use super::*;
use std::time::Instant;

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
    /// Band on the signed breakthrough error, which must also be negative (early). 15 % for both
    /// cases since #53: once the harness integrated every IMPES substep, nx = 24 reads -9.4 % (A)
    /// and -8.2 % (B), and 25 % / 30 % (from `e10560a`, sized to the harness artifact) would have
    /// passed a threefold regression. 15 % uses 63 % / 55 % of the band.
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
        rel_tol_breakthrough_pv: 0.15,
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
        rel_tol_breakthrough_pv: 0.15,
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

fn build_buckley_simulator(case: &BuckleyCase, fim_enabled: bool) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(case.nx, 1, 1, 0.2);
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

fn run_buckley_case(case: &BuckleyCase) -> BuckleyMetrics {
    let mut sim = build_buckley_simulator(case, false);

    let total_pv = (0..sim.nx * sim.ny * sim.nz)
        .map(|i| sim.pore_volume_m3(i))
        .sum::<f64>();

    let mut cumulative_injection = 0.0;
    let mut previous_time = 0.0;
    let mut breakthrough_pv = None;

    // IMPES splits each `step` into adaptive substeps and records one rate point per substep, so
    // every new point is integrated and tested. Reading only the last one would bill the whole
    // outer step at its final substep's rate and see breakthrough only at outer-step boundaries,
    // which made the result depend on the report interval (Case B: 40 % at dt = 0.5 vs 10 % at
    // 0.25) instead of on the discretization.
    'outer: for _ in 0..case.max_steps {
        let first_new_point = sim.rate_history.len();
        sim.step(case.dt_days);
        assert!(
            sim.rate_history.len() > first_new_point,
            "{}: step recorded no rate history",
            case.name
        );

        for point in &sim.rate_history[first_new_point..] {
            let dt = point.time - previous_time;
            previous_time = point.time;

            cumulative_injection += point.total_injection.max(0.0) * dt;

            if point.total_production_liquid > 1e-9 {
                let water_rate =
                    (point.total_production_liquid - point.total_production_oil).max(0.0);
                let watercut = (water_rate / point.total_production_liquid).clamp(0.0, 1.0);
                if watercut >= case.breakthrough_watercut {
                    breakthrough_pv = Some(cumulative_injection / total_pv);
                    break 'outer;
                }
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

fn build_exact_wasm_probe_simulator(nx: usize) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(nx, 1, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions(10.0, 10.0, 1.0).unwrap();
    sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_initial_pressure(300.0);
    sim.set_initial_saturation(0.1);
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
    sim.set_rock_properties(1e-6, 0.0, 1.0, 1.0).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(false);
    sim.set_permeability_per_layer(vec![2000.0], vec![2000.0], vec![200.0])
        .unwrap();
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.set_target_well_rates(0.0, 0.0).unwrap();
    sim.set_well_bhp_limits(100.0, 500.0).unwrap();
    sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
    sim.add_well(nx - 1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();
    sim
}

#[test]
#[ignore = "manual native single-step probe for comparing native vs wasm FIM behavior"]
fn native_single_step_fim_probe_case_a_24_cells() {
    let nx = 24;
    let mut sim = build_exact_wasm_probe_simulator(nx);

    let started = Instant::now();
    sim.step(0.25);
    let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let last = sim
        .rate_history
        .last()
        .expect("rate history should have an entry after one step");

    println!(
        "{{\"nx\":{},\"ms\":{:.3},\"time\":{:.6},\"warning\":\"{}\",\"history\":{}}}",
        nx,
        elapsed_ms,
        last.time,
        sim.last_solver_warning.replace('"', "\\\""),
        sim.rate_history.len(),
    );
}

/// First-order upstream smearing brings breakthrough early, so the gate fails a late front
/// however close it is, as well as an early one outside the band.
fn assert_breakthrough_within_band(case: &BuckleyCase) {
    let metrics = run_buckley_case(case);
    let rel_err = breakthrough_rel_err(&metrics);

    println!(
        "{}: breakthrough_pv_sim={:.4}, breakthrough_pv_ref={:.4}, rel_err={:.3}",
        case.name, metrics.breakthrough_pv, metrics.reference_breakthrough_pv, rel_err
    );

    record_breakthrough(
        case.name,
        case.nx,
        &metrics,
        Some(case.rel_tol_breakthrough_pv),
    );

    assert!(
        rel_err < 0.0 && -rel_err <= case.rel_tol_breakthrough_pv,
        "{} breakthrough should be early and within the band: sim={:.4}, ref={:.4}, rel_err={:.3}, tol={:.3}",
        case.name,
        metrics.breakthrough_pv,
        metrics.reference_breakthrough_pv,
        rel_err,
        case.rel_tol_breakthrough_pv,
    );
}

#[test]
fn benchmark_buckley_leverett_case_a_favorable_mobility() {
    assert_breakthrough_within_band(&buckley_case_a("BL-Case-A", 24, 0.5, 4000));
}

#[test]
fn benchmark_buckley_leverett_case_b_more_adverse_mobility() {
    assert_breakthrough_within_band(&buckley_case_b("BL-Case-B", 24, 0.5, 4000));
}

fn breakthrough_rel_err(metrics: &BuckleyMetrics) -> f64 {
    (metrics.breakthrough_pv - metrics.reference_breakthrough_pv)
        / metrics.reference_breakthrough_pv
}

/// Records a breakthrough against the Welge reference: the signed relative error (negative is
/// early) and the two pore volumes it is made of.
fn record_breakthrough(case_key: &str, nx: usize, metrics: &BuckleyMetrics, band: Option<f64>) {
    let case = format!("{case_key} nx={nx}");
    let welge = |metric, value, band, unit| {
        bench_record::metric(Metric {
            section: "buckley",
            case: &case,
            metric,
            value,
            band,
            unit,
            reference: "Buckley-Leverett + Welge",
            same_model: false,
            at: "",
        })
    };
    welge(
        "breakthrough_rel_err",
        breakthrough_rel_err(metrics),
        band,
        "frac",
    );
    welge("pv_bt_sim", metrics.breakthrough_pv, None, "PV");
    welge("pv_bt_ref", metrics.reference_breakthrough_pv, None, "PV");
}

/// IMPES chooses its own substeps, so the outer `step` size is only a report interval and must
/// not move breakthrough. It used to: the harness sampled one substep per outer step, and Case B
/// read 40 % at dt = 0.5 against 10 % at dt = 0.25.
#[test]
fn benchmark_buckley_leverett_breakthrough_is_independent_of_report_interval() {
    for (coarse, fine) in [
        (
            buckley_case_a("BL-Case-A-dt0.50", 24, 0.5, 4000),
            buckley_case_a("BL-Case-A-dt0.25", 24, 0.25, 8000),
        ),
        (
            buckley_case_b("BL-Case-B-dt0.50", 24, 0.5, 4000),
            buckley_case_b("BL-Case-B-dt0.25", 24, 0.25, 8000),
        ),
    ] {
        let pv_coarse = run_buckley_case(&coarse).breakthrough_pv;
        let pv_fine = run_buckley_case(&fine).breakthrough_pv;
        let spread = ((pv_coarse - pv_fine) / pv_fine).abs();

        println!(
            "{} vs {}: breakthrough_pv {:.4} vs {:.4}, spread={:.4}",
            coarse.name, fine.name, pv_coarse, pv_fine, spread
        );

        bench_record::metric(Metric {
            section: "buckley",
            case: coarse.name,
            metric: "report_interval_spread",
            value: spread,
            band: Some(0.01),
            unit: "frac",
            reference: fine.name,
            same_model: true,
            at: "",
        });

        assert!(
            spread <= 0.01,
            "report interval moved breakthrough: {}={:.4}, {}={:.4}",
            coarse.name,
            pv_coarse,
            fine.name,
            pv_fine,
        );
    }
}

/// The remaining mismatch is first-order upstream smearing: the front arrives early, and
/// halving the cell size brings it closer to the Welge shock.
#[test]
fn benchmark_buckley_leverett_grid_refinement_improves_alignment() {
    for (coarse, fine) in [
        (
            buckley_case_a("BL-Case-A-nx24", 24, 0.5, 4000),
            buckley_case_a("BL-Case-A-nx48", 48, 0.5, 4000),
        ),
        (
            buckley_case_b("BL-Case-B-nx24", 24, 0.5, 4000),
            buckley_case_b("BL-Case-B-nx48", 48, 0.5, 4000),
        ),
    ] {
        let (metrics_coarse, metrics_fine) = (run_buckley_case(&coarse), run_buckley_case(&fine));
        // nx = 24 is recorded, with its band, by the Case A and B gates.
        let case_key = &fine.name[..fine.name.len() - "-nx48".len()];
        record_breakthrough(case_key, fine.nx, &metrics_fine, None);
        let err_coarse = breakthrough_rel_err(&metrics_coarse);
        let err_fine = breakthrough_rel_err(&metrics_fine);

        println!(
            "{} -> {}: rel_err {:.3} -> {:.3}",
            coarse.name, fine.name, err_coarse, err_fine
        );

        assert!(
            err_coarse < 0.0 && err_fine < 0.0,
            "numerical diffusion should bring breakthrough early: {}={:.3}, {}={:.3}",
            coarse.name,
            err_coarse,
            fine.name,
            err_fine,
        );
        assert!(
            err_fine.abs() + 1e-9 < err_coarse.abs(),
            "grid refinement should improve alignment: {}={:.3}, {}={:.3}",
            coarse.name,
            err_coarse,
            fine.name,
            err_fine,
        );
    }
}

/// Continues the refinement sequence past the default gate's nx = 48, so `docs/BENCHMARKS.md`
/// §1 reports nx = 96 and 192 from a committed test rather than a one-off run. Asserts only
/// that the front stays early and keeps closing on the Welge shock.
///
/// `cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
///  benchmark_buckley_leverett_grid_sweep_replay -- --ignored --nocapture`
#[test]
#[ignore = "characterization replay: Buckley-Leverett at nx = 96 and 192, use --release"]
fn benchmark_buckley_leverett_grid_sweep_replay() {
    for (case_key, build) in [
        (
            "BL-Case-A",
            buckley_case_a as fn(&'static str, usize, f64, usize) -> BuckleyCase,
        ),
        ("BL-Case-B", buckley_case_b),
    ] {
        let mut previous = f64::NEG_INFINITY;
        for nx in [96, 192] {
            let metrics = run_buckley_case(&build(case_key, nx, 0.5, 4000));
            record_breakthrough(case_key, nx, &metrics, None);
            let err = breakthrough_rel_err(&metrics);
            println!("{case_key} nx={nx}: rel_err {err:.4}");
            assert!(
                err < 0.0 && err > previous,
                "{case_key} nx={nx}: breakthrough should stay early and move closer, rel_err={err:.4}"
            );
            previous = err;
        }
    }
}

/// The parity fixture's FIM case (`crates/ressim-py/parity/cases.json`, `buckley-fim`) takes one
/// 1-day step in 4 substeps. Natively it used to take 18: the first sparse-LU correction left the
/// inactive two-phase gas unknown at -1.1e-14 in the producer cell, its Jacobian column vanished,
/// and every later solve in the step hit an exactly singular matrix (FIM-DIRECT-001). The unit
/// contract is `two_phase_inactive_unknown_keeps_its_slope_at_negative_roundoff`; this pins the
/// system-level consequence so a regression shows up as a count rather than a slow suite.
#[test]
fn fim_two_phase_parity_step_does_not_fragment() {
    let nx = 50;
    let mut sim = ReservoirSimulator::new(nx, 1, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_initial_saturation(0.1);
    sim.set_permeability_random_seeded(2000.0, 2000.0, 42)
        .unwrap();
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.add_well_with_id(0, 0, 0, 500.0, 0.1, 0.0, true, "inj".to_string())
        .unwrap();
    sim.add_well_with_id(nx - 1, 0, 0, 100.0, 0.1, 0.0, false, "prod".to_string())
        .unwrap();

    sim.step(1.0);

    assert!(
        sim.last_solver_warning.is_empty(),
        "{}",
        sim.last_solver_warning
    );
    assert!(
        (sim.time_days - 1.0).abs() < 1e-12,
        "horizon not completed: t={}",
        sim.time_days
    );
    assert!(
        sim.rate_history.len() <= 4,
        "one 1-day step took {} substeps; 4 is the non-singular trajectory",
        sim.rate_history.len()
    );
}
