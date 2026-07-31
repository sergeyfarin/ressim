use crate::ReservoirSimulator;

/// 1D waterflood matching the `wf_bl1d` catalog scenario, run past water breakthrough at the
/// producer — the regime where the explicit fractional flow at the well cell is exercised.
fn build_waterflood() -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(96, 1, 1, 0.2);
    sim.set_fim_enabled(false);
    sim.set_cell_dimensions(10.0, 10.0, 1.0).unwrap();
    sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_initial_pressure(300.0);
    sim.set_initial_saturation(0.1);
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
    sim.set_permeability_random_seeded(2000.0, 2000.0, 42)
        .unwrap();
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.pc.p_entry = 0.0;
    sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
    sim.add_well(95, 0, 0, 100.0, 0.1, 0.0, false).unwrap();
    sim
}

fn oil_in_place_sc(sim: &ReservoirSimulator) -> f64 {
    (0..sim.nx * sim.ny * sim.nz)
        .map(|id| {
            let bo = sim.get_b_o_cell(id, sim.pressure[id]).max(1e-9);
            sim.sat_oil[id] * sim.pore_volume_m3(id) / bo
        })
        .sum()
}

/// The reported producer oil rate must describe the oil the substep actually removed.
///
/// `calculate_fluxes` transports the produced phase split evaluated at the beginning of the
/// substep; reporting must use that same split rather than recomputing it from the updated
/// saturations. When it did recompute, the reported rate was half an oscillation cycle out of
/// phase with the oil actually produced — up to 66% off over a report step here, with the sign
/// flipping between steps.
#[test]
fn impes_reported_oil_rate_tracks_oil_actually_produced() {
    let mut sim = build_waterflood();
    let mut worst_error = 0.0_f64;

    for _ in 0..25 {
        let oil_before = oil_in_place_sc(&sim);
        let first_new_point = sim.rate_history.len();
        let time_before = sim.time_days;

        sim.step(2.0);

        let mut reported_oil_sc = 0.0;
        let mut previous_time = time_before;
        for point in &sim.rate_history[first_new_point..] {
            reported_oil_sc += point.total_production_oil * (point.time - previous_time);
            previous_time = point.time;
        }

        let produced_oil_sc = oil_before - oil_in_place_sc(&sim);
        assert!(
            produced_oil_sc > 0.0,
            "expected oil production over the step"
        );
        worst_error = worst_error.max((reported_oil_sc - produced_oil_sc).abs() / produced_oil_sc);
    }

    assert!(
        worst_error < 0.02,
        "reported oil rate drifted from inventory depletion by {:.2}% over a report step",
        worst_error * 100.0
    );
}

/// A coarse report step must not turn the post-breakthrough decline into chatter.
///
/// The producer cell is drained explicitly, so without a throughput limit its water cut
/// alternates between substeps and the reported oil rate zig-zags — ~33% here at a two-day
/// report step, while the same run at a quarter-day step is smooth. Measured as the mean
/// absolute second difference of the reported rate relative to its mean: a smooth monotone
/// decline scores near zero regardless of how steep it is.
#[test]
fn impes_producer_oil_rate_declines_smoothly_at_coarse_report_steps() {
    let mut sim = build_waterflood();
    let mut reported = Vec::new();
    for _ in 0..25 {
        sim.step(2.0);
        reported.push(sim.rate_history.last().unwrap().total_production_oil);
    }

    // Post-breakthrough only: breakthrough itself is a legitimate step change in the rate.
    let decline = &reported[10..];
    let mean = decline.iter().sum::<f64>() / decline.len() as f64;
    let zig_zag = decline
        .windows(3)
        .map(|w| (w[2] - 2.0 * w[1] + w[0]).abs())
        .sum::<f64>()
        / (decline.len() - 2) as f64
        / mean;

    assert!(
        zig_zag < 0.05,
        "reported oil rate zig-zagged by {:.2}% of its mean between report steps",
        zig_zag * 100.0
    );
}

/// Timestep-refinement probe for both formulations. `#[ignore]`d — it is a measurement, not a
/// gate (~6 min in release, dominated by FIM). Recorded results are in `TODO.md`; rerun with
/// `cargo test --release --manifest-path src/lib/ressim/Cargo.toml solver_timestep_refinement_probe -- --ignored --nocapture`.
#[test]
#[ignore]
fn solver_timestep_refinement_probe() {
    // Analytical Buckley-Leverett breakthrough for these rock/fluid params (BL Case A):
    const REFERENCE_BREAKTHROUGH_PV: f64 = 0.5860;
    println!(
        "\n solver  report_dt  substeps  breakthrough_pv  err_vs_BL  cum_oil_50d  oil_rate_50d"
    );
    for &fim in &[false, true] {
        for &report_dt in &[5.0_f64, 2.0, 1.0, 0.5, 0.25] {
            let mut sim = build_waterflood();
            sim.set_fim_enabled(fim);
            let total_pv: f64 = (0..sim.nx * sim.ny * sim.nz)
                .map(|i| sim.pore_volume_m3(i))
                .sum();
            let steps = (50.0 / report_dt).round() as usize;
            let mut cum_oil = 0.0;
            let mut cum_inj = 0.0;
            let mut previous_time = 0.0;
            let mut breakthrough_pv = None;
            let mut prev_len = 0usize;
            for _ in 0..steps {
                sim.step(report_dt);
                for point in &sim.rate_history[prev_len..] {
                    let dt = point.time - previous_time;
                    previous_time = point.time;
                    cum_oil += point.total_production_oil * dt;
                    cum_inj += point.total_injection.max(0.0) * dt;
                    if breakthrough_pv.is_none() && point.total_production_liquid > 1e-9 {
                        let water =
                            (point.total_production_liquid - point.total_production_oil).max(0.0);
                        if water / point.total_production_liquid >= 0.01 {
                            breakthrough_pv = Some(cum_inj / total_pv);
                        }
                    }
                }
                prev_len = sim.rate_history.len();
            }
            let bt = breakthrough_pv.unwrap_or(f64::NAN);
            println!(
                "  {:5}  {:8.2}  {:8}  {:14.4}  {:+8.2}%  {:10.1}  {:11.4}",
                if fim { "FIM" } else { "IMPES" },
                report_dt,
                sim.rate_history.len(),
                bt,
                (bt - REFERENCE_BREAKTHROUGH_PV) / REFERENCE_BREAKTHROUGH_PV * 100.0,
                cum_oil,
                sim.rate_history.last().unwrap().total_production_oil
            );
        }
    }
}

/// Front-sharpness probe: the saturation profile at a fixed time for both formulations across
/// report-step sizes, with the internal substep sizes and Courant numbers each solver actually
/// used. `#[ignore]`d measurement, not a gate. Recorded results are in `TODO.md`; rerun with
/// `cargo test --release --manifest-path src/lib/ressim/Cargo.toml solver_front_sharpness_probe -- --ignored --nocapture`.
#[test]
#[ignore]
fn solver_front_sharpness_probe() {
    const SAMPLE_TIME_DAYS: f64 = 10.0;
    println!(
        "\n solver  report_dt  substeps  max_sub_dt  mean_sub_dt  max_cfl  lead_cell  shock_width_cells"
    );
    let mut profiles: Vec<(String, Vec<f64>)> = Vec::new();

    for &(fim, report_dt) in &[
        (false, 0.25_f64),
        (false, 5.0),
        (true, 5.0),
        (true, 0.25),
        (true, 0.05),
        (true, 0.02),
    ] {
        let mut sim = build_waterflood();
        sim.set_fim_enabled(fim);
        let steps = (SAMPLE_TIME_DAYS / report_dt).round() as usize;
        for _ in 0..steps {
            sim.step(report_dt);
        }

        // Substep sizes from the rate-history timestamps.
        let mut previous = 0.0;
        let mut sub_dts = Vec::new();
        for point in &sim.rate_history {
            sub_dts.push(point.time - previous);
            previous = point.time;
        }
        let max_sub = sub_dts.iter().cloned().fold(0.0_f64, f64::max);
        let mean_sub = sub_dts.iter().sum::<f64>() / sub_dts.len() as f64;

        // Courant number of the largest substep at the injector-side pore volume.
        // v = q_inj / (A phi); C = v dt / dx.
        let q_res = sim
            .rate_history
            .last()
            .map(|p| p.total_injection_reservoir)
            .unwrap_or(0.0);
        let area = 10.0 * 1.0;
        let velocity = q_res / (area * 0.2);
        let max_cfl = velocity * max_sub / 10.0;

        // Front geometry: leading edge = cells above S_wc + 0.02.
        let n = sim.nx;
        let lead = (0..n).filter(|&i| sim.sat_water[i] > 0.12).count();
        // Interpolated shock width: distance between the S_w = 0.45 and S_w = 0.15
        // crossings, in cells. The analytical shock is a discontinuity (width 0).
        let crossing = |target: f64| -> f64 {
            for i in 0..n - 1 {
                let (a, b) = (sim.sat_water[i], sim.sat_water[i + 1]);
                if a >= target && b < target {
                    return i as f64 + (a - target) / (a - b).max(1e-12);
                }
            }
            f64::NAN
        };
        let shock_width = crossing(0.15) - crossing(0.45);
        let label = format!("{}@{}", if fim { "FIM" } else { "IMPES" }, report_dt);
        println!(
            "  {:5}  {:8.2}  {:8}  {:10.4}  {:11.4}  {:7.3}  {:9}  {:17.2}",
            if fim { "FIM" } else { "IMPES" },
            report_dt,
            sim.rate_history.len(),
            max_sub,
            mean_sub,
            max_cfl,
            lead,
            shock_width
        );
        profiles.push((label, (0..n).map(|i| sim.sat_water[i]).collect()));
    }

    println!("\n S_w profile at t={SAMPLE_TIME_DAYS} d (cells 50..80)");
    print!("  cell ");
    for (label, _) in &profiles {
        print!("{label:>12}");
    }
    println!();
    for cell in 50..80 {
        print!("  {cell:4} ");
        for (_, profile) in &profiles {
            print!("{:12.4}", profile[cell]);
        }
        println!();
    }
}
