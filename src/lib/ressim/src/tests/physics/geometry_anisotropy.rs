use crate::ReservoirSimulator;

fn row_average_water_saturation(sim: &ReservoirSimulator, j: usize) -> f64 {
    let mut sum = 0.0;
    let mut count = 0usize;
    for i in 0..sim.nx {
        let id = sim.idx(i, j, 0);
        sum += sim.sat_water[id];
        count += 1;
    }
    sum / count as f64
}

fn row_average_gas_saturation(sim: &ReservoirSimulator, j: usize) -> f64 {
    let mut sum = 0.0;
    let mut count = 0usize;
    for i in 0..sim.nx {
        let id = sim.idx(i, j, 0);
        sum += sim.sat_gas[id];
        count += 1;
    }
    sum / count as f64
}

fn layer_average_water_saturation(sim: &ReservoirSimulator, k: usize) -> f64 {
    let mut sum = 0.0;
    let mut count = 0usize;
    for j in 0..sim.ny {
        for i in 0..sim.nx {
            let id = sim.idx(i, j, k);
            sum += sim.sat_water[id];
            count += 1;
        }
    }
    sum / count as f64
}

fn layer_average_gas_saturation(sim: &ReservoirSimulator, k: usize) -> f64 {
    let mut sum = 0.0;
    let mut count = 0usize;
    for j in 0..sim.ny {
        for i in 0..sim.nx {
            let id = sim.idx(i, j, k);
            sum += sim.sat_gas[id];
            count += 1;
        }
    }
    sum / count as f64
}

fn build_2d_areal_waterflood_streak_sim(nx: usize, ny: usize) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(nx, ny, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions(10.0, 10.0, 1.0).unwrap();
    sim.set_rel_perm_props(0.10, 0.10, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_initial_pressure(300.0);
    sim.set_initial_saturation(0.10);
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
    sim.set_rock_properties(1e-6, 0.0, 1.0, 1.0).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.05, 75.0, 0.75);

    for j in 0..ny {
        for i in 0..nx {
            let id = sim.idx(i, j, 0);
            sim.perm_x[id] = 300.0;
            sim.perm_y[id] = 300.0;
            sim.perm_z[id] = 30.0;
        }
    }
    let streak_row = ny / 2;
    for i in 0..nx {
        let id = sim.idx(i, streak_row, 0);
        sim.perm_x[id] = 3_000.0;
        sim.perm_y[id] = 3_000.0;
        sim.perm_z[id] = 300.0;
    }

    for j in 0..ny {
        sim.add_well(0, j, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(nx - 1, j, 0, 100.0, 0.1, 0.0, false)
            .unwrap();
    }

    sim
}

fn build_2d_areal_gas_flood_streak_sim(nx: usize, ny: usize) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(nx, ny, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions(10.0, 10.0, 1.0).unwrap();
    sim.set_three_phase_rel_perm_props(
        0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7,
    )
    .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_injected_fluid("gas").unwrap();
    sim.set_gas_redissolution_enabled(false);
    sim.set_initial_pressure(300.0);
    sim.set_initial_saturation(0.10);
    sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_gravity_enabled(false);
    sim.pc.p_entry = 0.0;

    for j in 0..ny {
        for i in 0..nx {
            let id = sim.idx(i, j, 0);
            sim.perm_x[id] = 300.0;
            sim.perm_y[id] = 300.0;
            sim.perm_z[id] = 30.0;
        }
    }
    let streak_row = ny / 2;
    for i in 0..nx {
        let id = sim.idx(i, streak_row, 0);
        sim.perm_x[id] = 3_000.0;
        sim.perm_y[id] = 3_000.0;
        sim.perm_z[id] = 300.0;
    }

    for j in 0..ny {
        sim.add_well(0, j, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(nx - 1, j, 0, 100.0, 0.1, 0.0, false)
            .unwrap();
    }

    sim
}

fn build_3d_layered_waterflood_kz_sim(nx: usize, ny: usize, nz: usize, kz_md: f64) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(nx, ny, nz, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![2.0; nz]).unwrap();
    sim.set_rel_perm_props(0.10, 0.10, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_initial_pressure(300.0);
    sim.set_initial_saturation(0.10);
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
    sim.set_rock_properties(1e-6, 0.0, 1.0, 1.0).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.05, 75.0, 0.75);

    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let id = sim.idx(i, j, k);
                sim.perm_x[id] = 1_000.0;
                sim.perm_y[id] = 1_000.0;
                sim.perm_z[id] = kz_md;
            }
        }
    }

    let mid_layer = nz / 2;
    for j in 0..ny {
        sim.add_well(0, j, mid_layer, 500.0, 0.1, 0.0, true).unwrap();
        sim.add_well(nx - 1, j, mid_layer, 100.0, 0.1, 0.0, false)
            .unwrap();
    }

    sim
}

fn build_3d_vertical_gas_segregation_sim(nx: usize, ny: usize, nz: usize, kz_md: f64) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(nx, ny, nz, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(10.0, 10.0, vec![2.0; nz]).unwrap();
    sim.set_three_phase_rel_perm_props(
        0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7,
    )
    .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_gas_redissolution_enabled(false);
    sim.set_initial_pressure(250.0);
    sim.set_initial_saturation(0.20);
    let mut sg = vec![0.0; nz];
    sg[nz - 1] = 0.35;
    sim.set_initial_gas_saturation_per_layer(sg).unwrap();
    sim.set_initial_rs(0.0);
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_gravity_enabled(true);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_capillary_params(0.0, 2.0).unwrap();

    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let id = sim.idx(i, j, k);
                sim.perm_x[id] = 500.0;
                sim.perm_y[id] = 500.0;
                sim.perm_z[id] = kz_md;
            }
        }
    }

    sim
}

#[test]
fn physics_geometry_waterflood_2d_high_perm_streak_advances_front_faster() {
    let mut sim = build_2d_areal_waterflood_streak_sim(9, 3);
    let streak_row = sim.ny / 2;

    for _ in 0..4 {
        sim.step(0.1);
        assert!(
            sim.last_solver_warning.is_empty(),
            "2D heterogeneous waterflood emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    let center_row_sw = row_average_water_saturation(&sim, streak_row);
    let flank_row_sw = 0.5
        * (row_average_water_saturation(&sim, 0)
            + row_average_water_saturation(&sim, sim.ny - 1));
    let center_downstream = sim.sat_water[sim.idx(sim.nx - 3, streak_row, 0)];
    let flank_downstream = 0.5
        * (sim.sat_water[sim.idx(sim.nx - 3, 0, 0)]
            + sim.sat_water[sim.idx(sim.nx - 3, sim.ny - 1, 0)]);
    let latest = sim.rate_history.last().expect("2D waterflood should record history");

    assert!(
        center_row_sw > flank_row_sw + 0.02,
        "high-perm streak should advance the average water front farther: center={:.6}, flank={:.6}",
        center_row_sw,
        flank_row_sw
    );
    assert!(
        center_downstream > flank_downstream + 0.03,
        "high-perm streak should reach downstream cells earlier: center={:.6}, flank={:.6}",
        center_downstream,
        flank_downstream
    );
    assert!(latest.total_injection > 0.0);
    assert!(latest.total_production_liquid > 0.0);
}

#[test]
#[ignore = "slow Phase 3 areal gas-flood probe; keep opt-in while the default geometry slice stays fast"]
fn physics_geometry_gas_flood_2d_high_perm_streak_advances_gas_front_faster() {
    let mut sim = build_2d_areal_gas_flood_streak_sim(8, 3);
    let streak_row = sim.ny / 2;

    for _ in 0..4 {
        sim.step(0.5);
        assert!(
            sim.last_solver_warning.is_empty(),
            "2D heterogeneous gas flood emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    let center_row_sg = row_average_gas_saturation(&sim, streak_row);
    let flank_row_sg = 0.5
        * (row_average_gas_saturation(&sim, 0)
            + row_average_gas_saturation(&sim, sim.ny - 1));
    let center_downstream = sim.sat_gas[sim.idx(sim.nx - 3, streak_row, 0)];
    let flank_downstream = 0.5
        * (sim.sat_gas[sim.idx(sim.nx - 3, 0, 0)]
            + sim.sat_gas[sim.idx(sim.nx - 3, sim.ny - 1, 0)]);
    let latest = sim.rate_history.last().expect("2D gas flood should record history");

    assert!(
        center_row_sg > flank_row_sg + 0.01,
        "high-perm streak should advance average gas saturation farther: center={:.6}, flank={:.6}",
        center_row_sg,
        flank_row_sg
    );
    assert!(
        center_downstream > flank_downstream + 0.02,
        "high-perm streak should move gas farther downstream: center={:.6}, flank={:.6}",
        center_downstream,
        flank_downstream
    );
    assert!(latest.material_balance_error_gas_m3 <= 2.0e4);
}

#[test]
#[ignore = "slow Phase 3 layered waterflood anisotropy probe; keep opt-in while the default geometry slice stays fast"]
fn physics_geometry_waterflood_3d_high_kz_spreads_front_across_layers() {
    let mut low_kz = build_3d_layered_waterflood_kz_sim(6, 2, 3, 5.0);
    let mut high_kz = build_3d_layered_waterflood_kz_sim(6, 2, 3, 500.0);

    for _ in 0..4 {
        low_kz.step(0.1);
        high_kz.step(0.1);
        assert!(low_kz.last_solver_warning.is_empty());
        assert!(high_kz.last_solver_warning.is_empty());
    }

    let low_off_layer_sw = 0.5
        * (layer_average_water_saturation(&low_kz, 0)
            + layer_average_water_saturation(&low_kz, 2));
    let high_off_layer_sw = 0.5
        * (layer_average_water_saturation(&high_kz, 0)
            + layer_average_water_saturation(&high_kz, 2));

    assert!(
        high_off_layer_sw > low_off_layer_sw + 0.01,
        "higher kz should spread the water front into adjacent layers: low_kz={:.6}, high_kz={:.6}",
        low_off_layer_sw,
        high_off_layer_sw
    );
}

#[test]
fn physics_geometry_gas_segregation_3d_high_kz_accelerates_vertical_migration() {
    let mut low_kz = build_3d_vertical_gas_segregation_sim(3, 3, 4, 5.0);
    let mut high_kz = build_3d_vertical_gas_segregation_sim(3, 3, 4, 500.0);

    for _ in 0..6 {
        low_kz.step(0.5);
        high_kz.step(0.5);
        assert!(low_kz.last_solver_warning.is_empty());
        assert!(high_kz.last_solver_warning.is_empty());
    }

    let low_top_sg = layer_average_gas_saturation(&low_kz, 0);
    let high_top_sg = layer_average_gas_saturation(&high_kz, 0);
    let low_bottom_sg = layer_average_gas_saturation(&low_kz, low_kz.nz - 1);
    let high_bottom_sg = layer_average_gas_saturation(&high_kz, high_kz.nz - 1);

    assert!(
        high_top_sg > low_top_sg + 0.01,
        "higher kz should accelerate upward gas migration into the top layer: low_kz={:.6}, high_kz={:.6}",
        low_top_sg,
        high_top_sg
    );
    assert!(
        high_bottom_sg < low_bottom_sg - 0.01,
        "higher kz should drain gas out of the bottom layer faster: low_kz={:.6}, high_kz={:.6}",
        low_bottom_sg,
        high_bottom_sg
    );
}

#[test]
#[ignore = "slow refined Phase 3 probe; uses a >1024-row areal case to exercise the iterative backend and should be run explicitly"]
fn physics_geometry_waterflood_2d_refined_streak_uses_iterative_backend_and_keeps_row_ordering() {
    let mut sim = build_2d_areal_waterflood_streak_sim(36, 15);
    let streak_row = sim.ny / 2;

    let trace = sim.step_with_diagnostics(0.05);
    assert!(
        sim.last_solver_warning.is_empty(),
        "refined 2D heterogeneous waterflood emitted solver warning at t={}: {}",
        sim.time_days,
        sim.last_solver_warning
    );
    assert!(
        trace.contains("used=fgmres-cpr"),
        "expected refined >1024-row case to use iterative backend, trace was: {}",
        trace
    );

    for _ in 0..2 {
        sim.step(0.05);
        assert!(sim.last_solver_warning.is_empty());
    }

    let center_row_sw = row_average_water_saturation(&sim, streak_row);
    let flank_row_sw = 0.5
        * (row_average_water_saturation(&sim, 0)
            + row_average_water_saturation(&sim, sim.ny - 1));
    assert!(
        center_row_sw > flank_row_sw + 0.01,
        "refined high-perm streak should still advance the water front faster: center={:.6}, flank={:.6}",
        center_row_sw,
        flank_row_sw
    );
}