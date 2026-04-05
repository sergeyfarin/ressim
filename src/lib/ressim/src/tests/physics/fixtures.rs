use crate::ReservoirSimulator;
use crate::pvt::{PvtRow, PvtTable};

pub(super) const DEP_PSS_LENGTH_M: f64 = 420.0;
pub(super) const DEP_PSS_WIDTH_M: f64 = 420.0;
pub(super) const DEP_PSS_HEIGHT_M: f64 = 10.0;
pub(super) const DEP_PSS_POROSITY: f64 = 0.2;
pub(super) const DEP_PSS_PERM_MD: f64 = 50.0;
pub(super) const DEP_PSS_WELL_RADIUS_M: f64 = 0.1;
pub(super) const DEP_PSS_INITIAL_PRESSURE_BAR: f64 = 300.0;
pub(super) const DEP_PSS_PRODUCER_BHP_BAR: f64 = 100.0;
pub(super) const DEP_PSS_INITIAL_SW: f64 = 0.1;
pub(super) const DEP_PSS_MU_O_CP: f64 = 1.0;
pub(super) const DEP_PSS_C_O_BAR_INV: f64 = 1e-5;
pub(super) const DEP_PSS_C_W_BAR_INV: f64 = 3e-6;
pub(super) const DEP_PSS_C_ROCK_BAR_INV: f64 = 1e-6;
pub(super) const DEP_PSS_WELL_SKIN: f64 = 0.0;

#[derive(Clone, Debug)]
pub(super) struct DepletionSnapshot {
    pub(super) time_days: f64,
    pub(super) oil_rate_sc_day: f64,
    pub(super) cumulative_oil_sc: f64,
    pub(super) avg_pressure_bar: f64,
    pub(super) total_injection_sc_day: f64,
}

#[derive(Clone, Debug)]
pub(super) struct GravityBenchmarkMetrics {
    pub(super) pressure_gradient_bar: f64,
    pub(super) top_sw_change: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ComponentInventorySc {
    pub(crate) water_sc: f64,
    pub(crate) oil_sc: f64,
    pub(crate) gas_sc: f64,
}

pub(super) fn make_dep_pss_like_sim(dt_days: f64, steps: usize) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(21, 21, 1, DEP_PSS_POROSITY);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(20.0, 20.0, vec![DEP_PSS_HEIGHT_M])
        .unwrap();
    sim.set_fluid_properties(DEP_PSS_MU_O_CP, 0.5).unwrap();
    sim.set_fluid_compressibilities(DEP_PSS_C_O_BAR_INV, DEP_PSS_C_W_BAR_INV)
        .unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_rock_properties(DEP_PSS_C_ROCK_BAR_INV, DEP_PSS_POROSITY, 1.0, 1.0)
        .unwrap();
    sim.set_initial_pressure(DEP_PSS_INITIAL_PRESSURE_BAR);
    sim.set_initial_saturation(DEP_PSS_INITIAL_SW);
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_permeability_per_layer(vec![DEP_PSS_PERM_MD], vec![DEP_PSS_PERM_MD], vec![5.0])
        .unwrap();
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.injector_enabled = false;
    sim.add_well(
        10,
        10,
        0,
        DEP_PSS_PRODUCER_BHP_BAR,
        DEP_PSS_WELL_RADIUS_M,
        DEP_PSS_WELL_SKIN,
        false,
    )
    .unwrap();

    for _ in 0..steps {
        sim.step(dt_days);
        assert!(
            sim.last_solver_warning.is_empty(),
            "depletion case emitted solver warning at t={}: {}",
            sim.time_days,
            sim.last_solver_warning
        );
    }

    sim
}

pub(crate) fn make_closed_depletion_single_cell_sim() -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(1, 1, 1, DEP_PSS_POROSITY);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(DEP_PSS_LENGTH_M, DEP_PSS_WIDTH_M, vec![DEP_PSS_HEIGHT_M])
        .unwrap();
    sim.set_fluid_properties(DEP_PSS_MU_O_CP, 0.5).unwrap();
    sim.set_fluid_compressibilities(DEP_PSS_C_O_BAR_INV, DEP_PSS_C_W_BAR_INV)
        .unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_rock_properties(DEP_PSS_C_ROCK_BAR_INV, DEP_PSS_POROSITY, 1.0, 1.0)
        .unwrap();
    sim.set_initial_pressure(DEP_PSS_INITIAL_PRESSURE_BAR);
    sim.set_initial_saturation(DEP_PSS_INITIAL_SW);
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_permeability_per_layer(vec![DEP_PSS_PERM_MD], vec![DEP_PSS_PERM_MD], vec![5.0])
        .unwrap();
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.injector_enabled = false;
    sim.add_well(
        0,
        0,
        0,
        DEP_PSS_PRODUCER_BHP_BAR,
        DEP_PSS_WELL_RADIUS_M,
        DEP_PSS_WELL_SKIN,
        false,
    )
    .unwrap();
    sim
}

pub(super) fn make_closed_depletion_single_cell_sim_with_storage(
    c_o_bar_inv: f64,
    c_w_bar_inv: f64,
    c_r_bar_inv: f64,
    producer_bhp_bar: f64,
) -> ReservoirSimulator {
    let mut sim = make_closed_depletion_single_cell_sim();
    sim.set_fluid_compressibilities(c_o_bar_inv, c_w_bar_inv)
        .unwrap();
    sim.set_rock_properties(c_r_bar_inv, DEP_PSS_POROSITY, 1.0, 1.0)
        .unwrap();
    sim.wells[0].bhp = producer_bhp_bar;
    sim
}

pub(super) fn collect_depletion_snapshots(sim: &ReservoirSimulator) -> Vec<DepletionSnapshot> {
    let mut cumulative_oil_sc = 0.0;
    let mut previous_time_days = 0.0;

    sim.rate_history
        .iter()
        .map(|point| {
            let dt_days = point.time - previous_time_days;
            previous_time_days = point.time;
            cumulative_oil_sc += point.total_production_oil * dt_days;
            DepletionSnapshot {
                time_days: point.time,
                oil_rate_sc_day: point.total_production_oil,
                cumulative_oil_sc,
                avg_pressure_bar: point.avg_reservoir_pressure,
                total_injection_sc_day: point.total_injection,
            }
        })
        .collect()
}

pub(super) fn make_below_bubble_point_flash_sim(
    gas_redissolution_enabled: bool,
) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_three_phase_rel_perm_props(0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7)
        .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_gas_redissolution_enabled(gas_redissolution_enabled);
    sim.set_initial_pressure(175.0);
    sim.set_initial_saturation(0.10);
    sim.set_initial_gas_saturation(0.0);
    sim.pvt.c_o = 1e-5;
    sim.pvt_table = Some(PvtTable::new(
        vec![
            PvtRow {
                p_bar: 100.0,
                rs_m3m3: 5.0,
                bo_m3m3: 1.05,
                mu_o_cp: 1.5,
                bg_m3m3: 0.01,
                mu_g_cp: 0.02,
            },
            PvtRow {
                p_bar: 150.0,
                rs_m3m3: 15.0,
                bo_m3m3: 1.12,
                mu_o_cp: 1.2,
                bg_m3m3: 0.006,
                mu_g_cp: 0.025,
            },
            PvtRow {
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
    sim.set_initial_rs(15.0);
    sim
}

pub(super) fn total_gas_inventory_sc(sim: &ReservoirSimulator) -> f64 {
    let vp_m3 = sim.pore_volume_m3(0);
    let p = sim.pressure[0];
    let bg = sim.get_b_g(p).max(1e-9);
    let bo = sim.get_b_o_cell(0, p).max(1e-9);
    sim.sat_gas[0] * vp_m3 / bg + (sim.sat_oil[0] * vp_m3 / bo) * sim.rs[0]
}

pub(super) fn apply_pressure_only_flash_update(sim: &mut ReservoirSimulator, pressure_bar: f64) {
    let pore_volume_m3 = sim.pore_volume_m3(0).max(1e-9);
    let water_saturation = sim.sat_water[0];
    let old_pressure_bar = sim.pressure[0];
    let old_bg = sim.get_b_g(old_pressure_bar).max(1e-9);
    let old_bo = sim.get_b_o_cell(0, old_pressure_bar).max(1e-9);
    let transported_free_gas_sc = sim.sat_gas[0] * pore_volume_m3 / old_bg;
    let dissolved_gas_sc = if sim.pvt_table.is_some() {
        sim.sat_oil[0] * pore_volume_m3 * sim.rs[0] / old_bo
    } else {
        0.0
    };
    let (sg, so, rs) = sim.split_gas_inventory_after_transport(
        pressure_bar,
        pore_volume_m3,
        water_saturation,
        transported_free_gas_sc,
        dissolved_gas_sc,
        if sim.gas_redissolution_enabled {
            None
        } else {
            Some(sim.rs[0])
        },
    );

    sim.pressure[0] = pressure_bar;
    sim.sat_gas[0] = sg;
    sim.sat_oil[0] = so;
    sim.rs[0] = rs;
}

pub(super) fn flash_below_bubble_point(sim: &mut ReservoirSimulator, pressure_bar: f64) {
    apply_pressure_only_flash_update(sim, pressure_bar);
}

pub(super) fn make_closed_gas_depletion_single_cell_sim() -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(200.0, 200.0, vec![20.0])
        .unwrap();
    sim.set_three_phase_rel_perm_props(0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7)
        .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_gas_redissolution_enabled(false);
    sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_initial_pressure(220.0);
    sim.set_initial_saturation(0.10);
    sim.set_initial_gas_saturation(0.89);
    sim.set_initial_rs(0.0);
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_permeability_per_layer(vec![500.0], vec![500.0], vec![500.0])
        .unwrap();
    sim.pvt_table = Some(PvtTable::new(
        vec![
            PvtRow {
                p_bar: 100.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.05,
                mu_o_cp: 1.1,
                bg_m3m3: 0.012,
                mu_g_cp: 0.02,
            },
            PvtRow {
                p_bar: 200.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.02,
                mu_o_cp: 1.1,
                bg_m3m3: 0.006,
                mu_g_cp: 0.022,
            },
            PvtRow {
                p_bar: 300.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.00,
                mu_o_cp: 1.1,
                bg_m3m3: 0.004,
                mu_g_cp: 0.024,
            },
        ],
        sim.pvt.c_o,
    ));
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.injector_enabled = false;
    sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();
    sim
}

pub(super) fn make_short_waterflood_1d_sim() -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(12, 1, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions(10.0, 10.0, 1.0).unwrap();
    sim.set_rel_perm_props(0.1, 0.1, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_initial_pressure(300.0);
    sim.set_initial_saturation(0.1);
    sim.set_fluid_properties(1.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(1e-5, 3e-6).unwrap();
    sim.set_rock_properties(1e-6, 0.2, 1.0, 1.0).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_permeability_random_seeded(2000.0, 2000.0, 42)
        .unwrap();
    sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
    sim.add_well(11, 0, 0, 100.0, 0.1, 0.0, false).unwrap();
    sim
}

pub(super) fn make_3phase_gas_injection_sim(nx: usize, fim_enabled: bool) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(nx, 1, 1, 0.2);
    sim.set_fim_enabled(fim_enabled);
    sim.set_three_phase_rel_perm_props(0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7)
        .unwrap();
    sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_injected_fluid("gas").unwrap();
    sim.set_initial_pressure(300.0);
    sim.set_initial_saturation(0.10);
    sim.set_gravity_enabled(false);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.pc.p_entry = 0.0;
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
    sim.add_well(nx - 1, 0, 0, 100.0, 0.1, 0.0, false).unwrap();
    sim
}

pub(super) fn total_gas_inventory_sc_all_cells(sim: &ReservoirSimulator) -> f64 {
    (0..sim.nx * sim.ny * sim.nz)
        .map(|idx| {
            let pore_volume_m3 = sim.pore_volume_m3(idx).max(1e-9);
            let free_gas_sc =
                sim.sat_gas[idx] * pore_volume_m3 / sim.get_b_g(sim.pressure[idx]).max(1e-9);
            let dissolved_gas_sc = if sim.pvt_table.is_some() {
                sim.sat_oil[idx] * pore_volume_m3 * sim.rs[idx]
                    / sim.get_b_o_cell(idx, sim.pressure[idx]).max(1e-9)
            } else {
                0.0
            };
            free_gas_sc + dissolved_gas_sc
        })
        .sum()
}

pub(crate) fn total_component_inventory_sc_all_cells(
    sim: &ReservoirSimulator,
) -> ComponentInventorySc {
    let mut water_sc = 0.0;
    let mut oil_sc = 0.0;
    let mut gas_sc = 0.0;

    for idx in 0..sim.nx * sim.ny * sim.nz {
        let pore_volume_m3 = sim.pore_volume_m3(idx).max(1e-9);
        let pressure_bar = sim.pressure[idx];
        let bw = sim.b_w.max(1e-9);
        let bo = sim.get_b_o_cell(idx, pressure_bar).max(1e-9);
        let bg = sim.get_b_g(pressure_bar).max(1e-9);

        let water_cell_sc = sim.sat_water[idx] * pore_volume_m3 / bw;
        let oil_cell_sc = sim.sat_oil[idx] * pore_volume_m3 / bo;
        let dissolved_gas_sc = if sim.three_phase_mode || sim.pvt_table.is_some() {
            oil_cell_sc * sim.rs[idx]
        } else {
            0.0
        };
        let free_gas_sc = sim.sat_gas[idx] * pore_volume_m3 / bg;

        water_sc += water_cell_sc;
        oil_sc += oil_cell_sc;
        gas_sc += free_gas_sc + dissolved_gas_sc;
    }

    ComponentInventorySc {
        water_sc,
        oil_sc,
        gas_sc,
    }
}

pub(super) fn cumulative_component_production_sc(sim: &ReservoirSimulator) -> ComponentInventorySc {
    let mut water_sc = 0.0;
    let mut oil_sc = 0.0;
    let mut gas_sc = 0.0;
    let mut previous_time_days = 0.0;

    for point in &sim.rate_history {
        let dt_days = point.time - previous_time_days;
        previous_time_days = point.time;
        oil_sc += point.total_production_oil.max(0.0) * dt_days;
        water_sc += (point.total_production_liquid - point.total_production_oil).max(0.0) * dt_days;
        gas_sc += point.total_production_gas.max(0.0) * dt_days;
    }

    ComponentInventorySc {
        water_sc,
        oil_sc,
        gas_sc,
    }
}

pub(super) fn make_free_gas_cap_runtime_sim(gas_oil_pc_entry_bar: f64) -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(1, 1, 3, 0.2);
    sim.set_fim_enabled(true);
    sim.set_cell_dimensions_per_layer(20.0, 20.0, vec![8.0, 8.0, 8.0])
        .unwrap();
    sim.set_three_phase_rel_perm_props(0.12, 0.10, 0.04, 0.02, 0.14, 2.0, 2.2, 1.5, 0.8, 0.9, 0.7)
        .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_gas_redissolution_enabled(false);
    sim.set_fluid_properties(1.1, 0.5).unwrap();
    sim.set_gas_fluid_properties(0.02, 1e-4, 10.0).unwrap();
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_initial_pressure(240.0);
    sim.set_initial_saturation(0.20);
    sim.set_initial_gas_saturation_per_layer(vec![0.35, 0.0, 0.0])
        .unwrap();
    sim.set_initial_rs(0.0);
    sim.set_gravity_enabled(true);
    sim.set_stability_params(0.05, 75.0, 0.75);
    sim.set_permeability_per_layer(
        vec![400.0, 400.0, 400.0],
        vec![400.0, 400.0, 400.0],
        vec![250.0, 250.0, 250.0],
    )
    .unwrap();
    sim.set_capillary_params(0.0, 2.0).unwrap();
    if gas_oil_pc_entry_bar > 0.0 {
        sim.set_gas_oil_capillary_params(gas_oil_pc_entry_bar, 1.8)
            .unwrap();
    }
    sim.pvt_table = Some(PvtTable::new(
        vec![
            PvtRow {
                p_bar: 100.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.05,
                mu_o_cp: 1.10,
                bg_m3m3: 0.012,
                mu_g_cp: 0.020,
            },
            PvtRow {
                p_bar: 200.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.02,
                mu_o_cp: 1.08,
                bg_m3m3: 0.006,
                mu_g_cp: 0.022,
            },
            PvtRow {
                p_bar: 300.0,
                rs_m3m3: 0.0,
                bo_m3m3: 1.00,
                mu_o_cp: 1.05,
                bg_m3m3: 0.004,
                mu_g_cp: 0.024,
            },
        ],
        sim.pvt.c_o,
    ));
    sim.set_well_control_modes("pressure".to_string(), "pressure".to_string());
    sim.injector_enabled = false;
    sim.add_well(0, 0, 2, 120.0, 0.1, 0.0, false).unwrap();
    sim
}

pub(super) fn run_hydrostatic_gravity_benchmark(fim_enabled: bool) -> GravityBenchmarkMetrics {
    let initial_sw = 0.9;
    let mut sim = ReservoirSimulator::new(1, 1, 2, 0.2);
    sim.set_fim_enabled(fim_enabled);
    sim.set_permeability_random_seeded(80_000.0, 80_000.0, 7)
        .unwrap();
    sim.set_initial_saturation(initial_sw);
    sim.pc.p_entry = 0.0;
    sim.set_fluid_densities(800.0, 1000.0).unwrap();
    sim.set_gravity_enabled(true);

    let hydro_dp_bar = sim.pvt.rho_w * 9.80665 * sim.dz[0] * 1e-5;
    let top_id = sim.idx(0, 0, 0);
    let bot_id = sim.idx(0, 0, 1);
    sim.pressure[top_id] = 300.0;
    sim.pressure[bot_id] = 300.0 + hydro_dp_bar;

    sim.step(5.0);

    GravityBenchmarkMetrics {
        pressure_gradient_bar: sim.pressure[bot_id] - sim.pressure[top_id],
        top_sw_change: (sim.sat_water[top_id] - initial_sw).abs(),
    }
}
