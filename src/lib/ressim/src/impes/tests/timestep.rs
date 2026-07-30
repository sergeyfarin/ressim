use crate::ReservoirSimulator;

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
        assert!(
            sim.pressure[i].is_finite(),
            "Pressure not finite at cell {}",
            i
        );
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
        assert!(
            entry.material_balance_error_oil_m3.is_finite(),
            "oil MB error not finite"
        );
    }
}

#[test]
fn rate_controlled_flowing_bhp_is_reported_from_accepted_pressure() {
    let mut sim = ReservoirSimulator::new(3, 3, 1, 0.2);
    sim.set_fim_enabled(false);
    sim.set_initial_pressure(300.0);
    sim.set_permeability_per_layer(vec![10.0], vec![10.0], vec![10.0])
        .unwrap();
    sim.set_well_control_modes("pressure".to_string(), "rate".to_string());
    sim.set_target_well_rates(0.0, 1.0).unwrap();
    sim.set_well_bhp_limits(0.0, 500.0).unwrap();
    sim.add_well(1, 1, 0, 50.0, 0.1, 0.0, false).unwrap();

    let initial_control = sim
        .resolve_well_control_for_pressures(&sim.wells[0], &sim.pressure)
        .and_then(|control| control.flowing_bhp)
        .expect("initial rate control should resolve a flowing BHP");

    sim.step(0.02);

    let accepted_control = sim
        .resolve_well_control_for_pressures(&sim.wells[0], &sim.pressure)
        .and_then(|control| control.flowing_bhp)
        .expect("accepted rate control should resolve a flowing BHP");
    let reported = sim.wells[0]
        .flowing_bhp
        .expect("the accepted step should publish flowing BHP");

    assert!(
        accepted_control < initial_control,
        "drawdown should move BHP below its initial-state value"
    );
    assert!(
        (reported - accepted_control).abs() < 1e-10,
        "reported BHP {reported} must match accepted-state BHP {accepted_control}, not initial-state BHP {initial_control}"
    );
}
