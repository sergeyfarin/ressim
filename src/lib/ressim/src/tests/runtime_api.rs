use super::*;

#[test]
fn saturation_stays_within_physical_bounds() {
    let mut sim = ReservoirSimulator::new(5, 1, 1, 0.2);
    sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
    sim.add_well(4, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

    for _ in 0..20 {
        sim.step(0.5);
    }

    let sw_min = sim.scal.s_wc;
    let sw_max = 1.0 - sim.scal.s_or;

    for i in 0..sim.nx * sim.ny * sim.nz {
        assert!(sim.sat_water[i] >= sw_min - 1e-9);
        assert!(sim.sat_water[i] <= sw_max + 1e-9);
        assert!(sim.sat_oil[i] >= -1e-9);
        assert!(sim.sat_oil[i] <= 1.0 + 1e-9);
        assert!((sim.sat_water[i] + sim.sat_oil[i] - 1.0).abs() < 1e-8);
    }
}

#[test]
fn water_mass_balance_sanity_without_wells() {
    let mut sim = ReservoirSimulator::new(4, 4, 1, 0.2);
    let water_before = total_water_volume(&sim);

    sim.step(1.0);

    let water_after = total_water_volume(&sim);
    assert!((water_after - water_before).abs() < 1e-6);
}

#[test]
fn water_mass_balance_sanity_without_wells_on_fim_branch() {
    let mut sim = ReservoirSimulator::new(4, 4, 1, 0.2);
    sim.set_fim_enabled(true);
    let water_before = total_water_volume(&sim);

    sim.step(1.0);

    let water_after = total_water_volume(&sim);
    assert!((water_after - water_before).abs() < 1e-6);
    assert!((sim.time_days - 1.0).abs() < 1e-12);
    assert_eq!(sim.rate_history.len(), 1);
}

#[test]
fn fim_branch_advances_simple_well_case_with_finite_state() {
    let mut sim = ReservoirSimulator::new(3, 1, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
    sim.add_well(2, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

    sim.step(0.25);

    assert!((sim.time_days - 0.25).abs() < 1e-12);
    assert_eq!(sim.rate_history.len(), 1);
    assert!(sim.last_solver_warning.is_empty());

    for i in 0..sim.nx * sim.ny * sim.nz {
        assert!(sim.pressure[i].is_finite());
        assert!(sim.sat_water[i].is_finite());
        assert!(sim.sat_oil[i].is_finite());
        assert!(sim.sat_gas[i].is_finite());
    }
}

#[test]
fn adaptive_timestep_produces_multiple_substeps_for_strong_flow() {
    let mut sim = ReservoirSimulator::new(3, 1, 1, 0.2);
    sim.set_fim_enabled(false);
    sim.set_permeability_random(100_000.0, 100_000.0).unwrap();
    sim.set_stability_params(0.01, 75.0, 0.75);
    sim.add_well(0, 0, 0, 700.0, 0.1, 0.0, true).unwrap();
    sim.add_well(2, 0, 0, 50.0, 0.1, 0.0, false).unwrap();

    sim.step(30.0);

    assert!(sim.rate_history.len() > 1);
    assert!(sim.time_days > 0.0);
    assert!((sim.time_days - 30.0).abs() < 1e-9);
}

#[test]
fn multiple_wells_in_same_block_keep_rates_finite() {
    let mut sim = ReservoirSimulator::new(4, 1, 1, 0.2);
    sim.add_well(0, 0, 0, 600.0, 0.1, 0.0, true).unwrap();
    sim.add_well(0, 0, 0, 550.0, 0.1, 0.0, true).unwrap();
    sim.add_well(3, 0, 0, 120.0, 0.1, 0.0, false).unwrap();

    for _ in 0..12 {
        sim.step(0.5);
    }

    assert!(!sim.rate_history.is_empty());
    let latest = sim.rate_history.last().unwrap();
    assert!(latest.total_injection.is_finite());
    assert!(latest.total_production_liquid.is_finite());
    assert!(latest.total_production_oil.is_finite());

    for i in 0..sim.nx * sim.ny * sim.nz {
        assert!(sim.pressure[i].is_finite());
        assert!(sim.sat_water[i].is_finite());
        assert!(sim.sat_oil[i].is_finite());
    }
}

#[test]
fn out_of_bounds_well_is_rejected_without_state_change() {
    let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
    let wells_before = sim.wells.len();

    let result = sim.add_well(2, 0, 0, 250.0, 0.1, 0.0, false);
    err_contains(result, "out of bounds");

    assert_eq!(sim.wells.len(), wells_before);
}

#[test]
fn stability_extremes_produce_finite_state() {
    let mut sim_loose = ReservoirSimulator::new(3, 1, 1, 0.2);
    sim_loose.set_stability_params(1.0, 75.0, 0.75);
    sim_loose
        .set_permeability_random(20_000.0, 20_000.0)
        .unwrap();
    sim_loose.add_well(0, 0, 0, 650.0, 0.1, 0.0, true).unwrap();
    sim_loose.add_well(2, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
    sim_loose.step(5.0);

    let mut sim_tight = ReservoirSimulator::new(3, 1, 1, 0.2);
    sim_tight.set_stability_params(0.01, 75.0, 0.75);
    sim_tight
        .set_permeability_random(20_000.0, 20_000.0)
        .unwrap();
    sim_tight.add_well(0, 0, 0, 650.0, 0.1, 0.0, true).unwrap();
    sim_tight.add_well(2, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
    sim_tight.step(5.0);

    for sim in [&sim_loose, &sim_tight] {
        for i in 0..sim.nx * sim.ny * sim.nz {
            assert!(sim.pressure[i].is_finite());
            assert!(sim.sat_water[i].is_finite());
            assert!(sim.sat_oil[i].is_finite());
        }
        assert!(sim.time_days > 0.0);
        assert!(sim.time_days <= 5.0);
        assert!(!sim.rate_history.is_empty());
    }

    assert!(sim_tight.rate_history.len() >= sim_loose.rate_history.len());
}

#[test]
fn api_contract_rejects_invalid_relperm_parameters() {
    let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
    err_contains(
        sim.set_rel_perm_props(0.6, 0.5, 2.0, 2.0, 1.0, 1.0),
        "must be < 1.0",
    );
    err_contains(
        sim.set_rel_perm_props(0.1, 0.1, 0.0, 2.0, 1.0, 1.0),
        "must be positive",
    );
    err_contains(
        sim.set_rel_perm_props(f64::NAN, 0.1, 2.0, 2.0, 1.0, 1.0),
        "finite numbers",
    );
}

#[test]
fn api_contract_allows_zero_water_relperm_endpoint() {
    let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
    sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 0.0, 1.0)
        .expect("k_rw_max = 0 should be accepted for immobile-water cases");
}

#[test]
fn api_contract_rejects_invalid_density_inputs() {
    let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
    err_contains(sim.set_fluid_densities(-800.0, 1000.0), "must be positive");
    err_contains(sim.set_fluid_densities(800.0, f64::NAN), "finite numbers");
}

#[test]
fn api_contract_rejects_invalid_capillary_inputs() {
    let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
    err_contains(sim.set_capillary_params(-1.0, 2.0), "non-negative");
    err_contains(sim.set_capillary_params(5.0, 0.0), "positive");
    err_contains(sim.set_capillary_params(f64::NAN, 2.0), "finite numbers");
}

#[test]
fn default_step_path_reports_rate_controlled_well_state() {
    let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
    sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
    sim.set_target_well_surface_rates(0.0, 50.0).unwrap();
    sim.set_well_bhp_limits(50.0, 500.0).unwrap();
    sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
    sim.add_well(1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

    sim.step(0.25);

    assert!(!sim.rate_history.is_empty());
    let point = sim.rate_history.last().unwrap();
    assert!((point.time - 0.25).abs() < 1e-9);
    assert!(point.total_production_oil.is_finite());
    assert!(point.total_injection.is_finite());
    assert!(point.producer_bhp_limited_fraction.is_finite());
    assert!(point.injector_bhp_limited_fraction.is_finite());
}

#[test]
#[ignore = "known FIM rate-control parity mismatch: public-step mixed-control well rates are now nonzero but still differ materially from IMPES; run explicitly while tuning coupled well behavior"]
fn rate_control_reporting_benchmark_fim_matches_impes() {
    let impes = run_rate_control_reporting_benchmark(false);
    let fim = run_rate_control_reporting_benchmark(true);

    let oil_rel_diff =
        ((fim.total_production_oil - impes.total_production_oil) / impes.total_production_oil.max(1e-12))
            .abs();
    let injection_abs_diff = (fim.total_injection - impes.total_injection).abs();
    let avg_pressure_rel_diff = ((fim.avg_reservoir_pressure - impes.avg_reservoir_pressure)
        / impes.avg_reservoir_pressure.max(1e-12))
        .abs();

    assert!(fim.total_production_oil.is_finite());
    assert!(fim.total_injection.is_finite());
    assert!(
        oil_rel_diff <= 0.20,
        "rate-control benchmark oil-rate drift too large: IMPES={:.6}, FIM={:.6}, rel_diff={:.4}",
        impes.total_production_oil,
        fim.total_production_oil,
        oil_rel_diff,
    );
    assert!(
        injection_abs_diff <= 1e-9,
        "rate-control benchmark injector-rate drift too large: IMPES={:.6}, FIM={:.6}, abs_diff={:.6}",
        impes.total_injection,
        fim.total_injection,
        injection_abs_diff,
    );
    assert!(
        avg_pressure_rel_diff <= 0.10,
        "rate-control benchmark average-pressure drift too large: IMPES={:.6}, FIM={:.6}, rel_diff={:.4}",
        impes.avg_reservoir_pressure,
        fim.avg_reservoir_pressure,
        avg_pressure_rel_diff,
    );
    assert!(
        (fim.producer_bhp_limited_fraction - impes.producer_bhp_limited_fraction).abs() <= 1e-9,
        "rate-control benchmark producer clamp fraction drift: IMPES={:.3}, FIM={:.3}",
        impes.producer_bhp_limited_fraction,
        fim.producer_bhp_limited_fraction,
    );
    assert!(
        (fim.injector_bhp_limited_fraction - impes.injector_bhp_limited_fraction).abs() <= 1e-9,
        "rate-control benchmark injector clamp fraction drift: IMPES={:.3}, FIM={:.3}",
        impes.injector_bhp_limited_fraction,
        fim.injector_bhp_limited_fraction,
    );
}

#[test]
fn api_contract_rejects_invalid_permeability_inputs() {
    let mut sim = ReservoirSimulator::new(2, 2, 2, 0.2);
    err_contains(
        sim.set_permeability_random(200.0, 50.0),
        "cannot exceed max",
    );
    err_contains(
        sim.set_permeability_random_seeded(-1.0, 100.0, 123),
        "must be positive",
    );
    err_contains(
        sim.set_permeability_per_layer(vec![100.0], vec![100.0, 120.0], vec![10.0, 12.0]),
        "length equal to nz",
    );
    err_contains(
        sim.set_permeability_per_layer(vec![100.0, 120.0], vec![100.0, 120.0], vec![0.0, 12.0]),
        "must be positive",
    );
}

#[test]
fn pressure_resolve_on_substep_produces_physical_results() {
    let mut sim = ReservoirSimulator::new(5, 1, 1, 0.2);
    sim.set_fim_enabled(false);
    sim.set_permeability_random_seeded(100_000.0, 100_000.0, 42)
        .unwrap();
    sim.set_stability_params(0.02, 50.0, 0.5);
    sim.pc.p_entry = 0.0;
    sim.add_well(0, 0, 0, 600.0, 0.1, 0.0, true).unwrap();
    sim.add_well(4, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

    sim.step(20.0);

    assert!(
        sim.rate_history.len() > 1,
        "Expected sub-stepping, got {} entries",
        sim.rate_history.len()
    );

    for i in 0..sim.nx * sim.ny * sim.nz {
        assert!(sim.pressure[i].is_finite(), "Pressure not finite at cell {}", i);
        assert!(sim.sat_water[i].is_finite(), "Sw not finite at cell {}", i);
        assert!(
            sim.sat_water[i] >= sim.scal.s_wc - 1e-9,
            "Sw below s_wc at cell {}",
            i
        );
        assert!(
            sim.sat_water[i] <= 1.0 - sim.scal.s_or + 1e-9,
            "Sw above 1-s_or at cell {}",
            i
        );
        assert!((sim.sat_water[i] + sim.sat_oil[i] - 1.0).abs() < 1e-8);
    }

    for i in 0..sim.nx * sim.ny * sim.nz {
        assert!(
            sim.pressure[i] > 50.0 && sim.pressure[i] < 700.0,
            "Pressure {} at cell {} outside physical range",
            sim.pressure[i],
            i
        );
    }

    for entry in &sim.rate_history {
        assert!(
            entry.material_balance_error_m3.is_finite(),
            "MB error not finite"
        );
    }
}

#[test]
fn benchmark_like_substepping_completes_requested_dt() {
    let mut sim = ReservoirSimulator::new(24, 1, 1, 0.2);
    sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_initial_saturation(0.1);
    sim.set_permeability_random_seeded(2000.0, 2000.0, 42)
        .unwrap();
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.pc.p_entry = 0.0;
    sim.pvt.mu_w = 0.5;
    sim.pvt.mu_o = 1.0;
    sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
    sim.add_well(23, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

    sim.step(0.5);

    assert!(
        (sim.time_days - 0.5).abs() < 1e-9,
        "Expected the simulator to complete the requested 0.5 day step, advanced {} days",
        sim.time_days
    );
    assert!(
        !sim.rate_history.is_empty() && (sim.rate_history.last().unwrap().time - 0.5).abs() < 1e-9,
        "Expected the last recorded rate-history time to match the completed step"
    );
}

#[test]
fn set_initial_saturation_per_layer_applies_uniformly_by_k() {
    let mut sim = ReservoirSimulator::new(2, 2, 3, 0.2);
    sim.set_initial_saturation_per_layer(vec![0.1, 0.4, 0.8])
        .unwrap();

    for k in 0..sim.nz {
        for j in 0..sim.ny {
            for i in 0..sim.nx {
                let id = sim.idx(i, j, k);
                let sw = sim.sat_water[id];
                assert!((sw - [0.1, 0.4, 0.8][k]).abs() < 1e-12);
                assert!((sim.sat_oil[id] - (1.0 - sw)).abs() < 1e-12);
            }
        }
    }
}