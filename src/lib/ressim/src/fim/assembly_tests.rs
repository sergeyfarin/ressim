use sprs::{CsMat, TriMatI};

use crate::ReservoirSimulator;
use crate::fim::state::{FimCellState, FimState, HydrocarbonState};
use crate::pvt::{PvtRow, PvtTable};

use super::*;

fn jacobian_value(matrix: &CsMat<f64>, row: usize, col: usize) -> f64 {
    matrix
        .outer_view(row)
        .and_then(|view| {
            view.iter()
                .find(|(index, _)| *index == col)
                .map(|(_, value)| *value)
        })
        .unwrap_or(0.0)
}

fn build_rate_controlled_waterflood_fd_fixture() -> (ReservoirSimulator, FimState, FimState) {
    let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
    sim.set_injected_fluid("water").unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.pvt_table = Some(PvtTable::new(
        vec![
            PvtRow {
                p_bar: 100.0,
                rs_m3m3: 10.0,
                bo_m3m3: 1.2,
                mu_o_cp: 1.4,
                bg_m3m3: 0.02,
                mu_g_cp: 0.03,
            },
            PvtRow {
                p_bar: 200.0,
                rs_m3m3: 20.0,
                bo_m3m3: 1.1,
                mu_o_cp: 1.2,
                bg_m3m3: 0.01,
                mu_g_cp: 0.025,
            },
            PvtRow {
                p_bar: 400.0,
                rs_m3m3: 40.0,
                bo_m3m3: 1.0,
                mu_o_cp: 1.0,
                bg_m3m3: 0.005,
                mu_g_cp: 0.02,
            },
        ],
        sim.pvt.c_o,
    ));
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
    sim.add_well(1, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
    sim.set_rate_controlled_wells(true);
    sim.set_target_well_rates(10.0, 10.0).unwrap();
    sim.well_bhp_min = 20.0;
    sim.well_bhp_max = 600.0;

    let previous_state = FimState::from_simulator(&sim);
    let mut state = previous_state.clone();
    for cell in state.cells.iter_mut() {
        cell.hydrocarbon_var = 0.05;
    }
    state.cells[0].pressure_bar = 310.0;
    state.cells[1].pressure_bar = 280.0;
    state.cells[0].sw = 0.35;
    state.perforation_rates_m3_day[0] = -5.0;
    state.perforation_rates_m3_day[1] = 8.0;

    (sim, previous_state, state)
}

fn build_closed_depletion_fd_fixture() -> (ReservoirSimulator, FimState, FimState) {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_cell_dimensions(200.0, 200.0, 20.0).unwrap();
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
    sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

    let previous_state = FimState::from_simulator(&sim);
    let mut state = previous_state.clone();
    state.cells[0].pressure_bar = 185.0;
    state.cells[0].sw = 0.16;
    state.cells[0].regime = HydrocarbonState::Saturated;
    state.cells[0].hydrocarbon_var = 0.72;
    state.well_bhp[0] = 120.0;
    state.perforation_rates_m3_day[0] = 10.0;

    (sim, previous_state, state)
}

fn build_mixed_regime_fd_fixture() -> (ReservoirSimulator, FimState, FimState) {
    let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
    sim.set_injected_fluid("water").unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.set_three_phase_rel_perm_props(0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7)
        .unwrap();
    sim.pvt_table = Some(PvtTable::new(
        vec![
            PvtRow {
                p_bar: 100.0,
                rs_m3m3: 10.0,
                bo_m3m3: 1.2,
                mu_o_cp: 1.4,
                bg_m3m3: 0.02,
                mu_g_cp: 0.03,
            },
            PvtRow {
                p_bar: 200.0,
                rs_m3m3: 20.0,
                bo_m3m3: 1.1,
                mu_o_cp: 1.2,
                bg_m3m3: 0.01,
                mu_g_cp: 0.025,
            },
            PvtRow {
                p_bar: 400.0,
                rs_m3m3: 40.0,
                bo_m3m3: 1.0,
                mu_o_cp: 1.0,
                bg_m3m3: 0.005,
                mu_g_cp: 0.02,
            },
        ],
        sim.pvt.c_o,
    ));
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
    sim.add_well(1, 0, 0, 75.0, 0.1, 0.0, false).unwrap();
    sim.set_rate_controlled_wells(true);
    sim.set_target_well_rates(12.0, 10.0).unwrap();
    sim.well_bhp_min = 40.0;
    sim.well_bhp_max = 600.0;

    let previous_state = FimState::from_simulator(&sim);
    let mut state = previous_state.clone();
    state.cells[0].pressure_bar = 255.0;
    state.cells[0].sw = 0.26;
    state.cells[0].regime = HydrocarbonState::Undersaturated;
    state.cells[0].hydrocarbon_var = 15.0;
    state.cells[1].pressure_bar = 185.0;
    state.cells[1].sw = 0.34;
    state.cells[1].regime = HydrocarbonState::Saturated;
    state.cells[1].hydrocarbon_var = 0.08;
    state.well_bhp[0] = 360.0;
    state.well_bhp[1] = 120.0;
    state.perforation_rates_m3_day[0] = -7.0;
    state.perforation_rates_m3_day[1] = 6.5;

    (sim, previous_state, state)
}

fn assert_full_system_fd_matches_for_fixture(
    columns: &[usize],
    build_fixture: fn() -> (ReservoirSimulator, FimState, FimState),
) {
    let (sim, previous_state, state) = build_fixture();
    let options = FimAssemblyOptions {
        dt_days: 1.0,
        include_wells: true,
        assemble_residual_only: false,
        topology: None,
    };
    let assembly = assemble_fim_system(&sim, &previous_state, &state, &options);
    let n = assembly.residual.len();

    let mut max_error = 0.0_f64;
    let mut worst_entry = (0usize, 0usize, 0.0_f64, 0.0_f64);

    for &col in columns {
        let mut state_plus = state.clone();
        let mut state_minus = state.clone();
        let n_cells = state.cells.len();
        let n_wells = state.n_well_unknowns();

        let h = if col < n_cells * 3 {
            let cell_idx = col / 3;
            let local_var = col % 3;
            let step = finite_difference_step(&state, col);
            match local_var {
                0 => {
                    state_plus.cells[cell_idx].pressure_bar += step;
                    state_minus.cells[cell_idx].pressure_bar -= step;
                }
                1 => {
                    state_plus.cells[cell_idx].sw += step;
                    state_minus.cells[cell_idx].sw -= step;
                }
                2 => {
                    state_plus.cells[cell_idx].hydrocarbon_var += step;
                    state_minus.cells[cell_idx].hydrocarbon_var -= step;
                }
                _ => unreachable!(),
            }
            step
        } else if col < n_cells * 3 + n_wells {
            let well_idx = col - n_cells * 3;
            let step = 1e-4 * state.well_bhp[well_idx].abs().max(1.0);
            state_plus.well_bhp[well_idx] += step;
            state_minus.well_bhp[well_idx] -= step;
            step
        } else {
            let perf_idx = col - n_cells * 3 - n_wells;
            let step = 1e-4 * state.perforation_rates_m3_day[perf_idx].abs().max(0.1);
            state_plus.perforation_rates_m3_day[perf_idx] += step;
            state_minus.perforation_rates_m3_day[perf_idx] -= step;
            step
        };

        let r_plus = assemble_fim_system(&sim, &previous_state, &state_plus, &options).residual;
        let r_minus = assemble_fim_system(&sim, &previous_state, &state_minus, &options).residual;

        for row in 0..n {
            let fd_value = (r_plus[row] - r_minus[row]) / (2.0 * h);
            let exact_value = jacobian_value(&assembly.jacobian, row, col);
            let scale = exact_value.abs().max(fd_value.abs()).max(1e-6);
            let error = (exact_value - fd_value).abs() / scale;
            if error > max_error {
                max_error = error;
                worst_entry = (row, col, exact_value, fd_value);
            }
            assert!(
                error < 5e-2,
                "Jacobian mismatch at (row={}, col={}): exact={:.6e}, fd={:.6e}, rel_error={:.3e}",
                row,
                col,
                exact_value,
                fd_value,
                error
            );
        }
    }

    eprintln!(
        "sampled full_system_jacobian FD check: max_rel_error={:.3e} at ({}, {}): exact={:.6e} fd={:.6e}",
        max_error, worst_entry.0, worst_entry.1, worst_entry.2, worst_entry.3
    );
}

#[test]
fn full_system_jacobian_matches_fd_for_single_cell_depletion() {
    assert_full_system_fd_matches_for_fixture(&[0, 1, 2, 3, 4], build_closed_depletion_fd_fixture);
}

#[test]
fn full_system_jacobian_matches_fd_for_mixed_saturated_and_undersaturated_cells() {
    assert_full_system_fd_matches_for_fixture(
        &(0..10).collect::<Vec<_>>(),
        build_mixed_regime_fd_fixture,
    );
}

#[test]
fn residual_only_assembly_matches_full_residual_for_rate_controlled_waterflood() {
    let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
    sim.set_injected_fluid("water").unwrap();
    sim.add_well(0, 0, 0, 300.0, 0.1, 0.0, true).unwrap();
    sim.add_well(1, 0, 0, 50.0, 0.1, 0.0, false).unwrap();

    let previous_state = FimState::from_simulator(&sim);
    let mut state = previous_state.clone();
    state.cells[0].pressure_bar = 280.0;
    state.cells[1].pressure_bar = 180.0;
    state.cells[0].sw = 0.35;
    state.cells[1].sw = 0.45;
    state.perforation_rates_m3_day[0] = -15.0;
    state.perforation_rates_m3_day[1] = 12.0;

    let full_options = FimAssemblyOptions {
        dt_days: 1.0,
        include_wells: true,
        assemble_residual_only: false,
        topology: None,
    };
    let residual_only_options = FimAssemblyOptions {
        assemble_residual_only: true,
        ..full_options
    };

    let full = assemble_fim_system(&sim, &previous_state, &state, &full_options);
    let residual_only = assemble_fim_system(&sim, &previous_state, &state, &residual_only_options);

    assert!((&full.residual - &residual_only.residual).amax() <= 1e-12);
    assert!(full.equation_scaling.water == residual_only.equation_scaling.water);
    assert!(full.equation_scaling.oil_component == residual_only.equation_scaling.oil_component);
    assert!(full.equation_scaling.gas_component == residual_only.equation_scaling.gas_component);
    assert!(
        full.equation_scaling.well_constraint == residual_only.equation_scaling.well_constraint
    );
    assert!(
        full.equation_scaling.perforation_flow == residual_only.equation_scaling.perforation_flow
    );
    assert!(full.variable_scaling.pressure == residual_only.variable_scaling.pressure);
    assert!(full.variable_scaling.sw == residual_only.variable_scaling.sw);
    assert!(
        full.variable_scaling.hydrocarbon_var == residual_only.variable_scaling.hydrocarbon_var
    );
    assert!(full.variable_scaling.well_bhp == residual_only.variable_scaling.well_bhp);
    assert!(
        full.variable_scaling.perforation_rate == residual_only.variable_scaling.perforation_rate
    );
    assert_eq!(residual_only.jacobian.nnz(), 0);
}

#[test]
fn offsets_follow_cell_major_three_unknown_layout() {
    assert_eq!(unknown_offset(0, 0), 0);
    assert_eq!(unknown_offset(0, 2), 2);
    assert_eq!(unknown_offset(1, 0), 3);
    assert_eq!(equation_offset(2, 1), 7);
}

#[test]
fn assembly_scaffold_matches_state_size() {
    let sim = ReservoirSimulator::new(1, 1, 2, 0.2);
    let state = FimState {
        cells: vec![
            FimCellState {
                pressure_bar: 300.0,
                sw: 0.3,
                hydrocarbon_var: 0.0,
                regime: HydrocarbonState::Saturated,
            },
            FimCellState {
                pressure_bar: 301.0,
                sw: 0.31,
                hydrocarbon_var: 0.0,
                regime: HydrocarbonState::Saturated,
            },
        ],
        well_bhp: Vec::new(),
        perforation_rates_m3_day: Vec::new(),
    };

    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: false,
            assemble_residual_only: false,
            topology: None,
        },
    );

    assert_eq!(assembly.residual.len(), 6);
    assert_eq!(assembly.jacobian.rows(), 6);
    assert_eq!(assembly.jacobian.cols(), 6);
    assert!(assembly.jacobian.nnz() >= 18);
    assert_eq!(assembly.equation_scaling.water.len(), 2);
    assert_eq!(assembly.variable_scaling.pressure.len(), 2);
    assert!(assembly.equation_scaling.well_constraint.is_empty());
    assert!(assembly.equation_scaling.perforation_flow.is_empty());
}

#[test]
fn accumulation_residual_is_nonzero_for_perturbed_cell_state() {
    let sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    let previous_state = FimState::from_simulator(&sim);
    let mut state = previous_state.clone();
    state.cells[0].pressure_bar += 10.0;
    state.cells[0].sw += 0.05;

    let assembly = assemble_fim_system(
        &sim,
        &previous_state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: false,
            assemble_residual_only: false,
            topology: None,
        },
    );

    assert!(assembly.residual.norm() > 1e-9);
}

#[test]
fn uniform_state_has_zero_flux_residual() {
    let sim = ReservoirSimulator::new(2, 1, 1, 0.2);
    let state = FimState::from_simulator(&sim);

    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: false,
            assemble_residual_only: false,
            topology: None,
        },
    );

    assert!(assembly.residual.norm() <= 1e-12);
}

#[test]
fn residual_only_two_cell_flux_is_component_conservative_for_oil_water() {
    let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
    sim.pvt.c_o = 0.0;
    sim.pvt.c_w = 0.0;
    sim.rock_compressibility = 0.0;
    let mut state = FimState::from_simulator(&sim);
    state.cells[0].pressure_bar = 250.0;
    state.cells[1].pressure_bar = 150.0;
    state.cells[0].sw = 0.30;
    state.cells[1].sw = 0.55;

    let topology = build_well_topology(&sim);
    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: false,
            assemble_residual_only: true,
            topology: Some(&topology),
        },
    );

    assert_eq!(assembly.jacobian.nnz(), 0);

    let cell0_water = cell_equation_residual_breakdown(&sim, &state, &state, &topology, 1.0, 0, 0)
        .expect("cell 0 water breakdown should exist");
    let cell1_water = cell_equation_residual_breakdown(&sim, &state, &state, &topology, 1.0, 1, 0)
        .expect("cell 1 water breakdown should exist");
    let cell0_oil = cell_equation_residual_breakdown(&sim, &state, &state, &topology, 1.0, 0, 1)
        .expect("cell 0 oil breakdown should exist");
    let cell1_oil = cell_equation_residual_breakdown(&sim, &state, &state, &topology, 1.0, 1, 1)
        .expect("cell 1 oil breakdown should exist");

    assert!(cell0_water.accumulation.abs() < 1e-12);
    assert!(cell1_water.accumulation.abs() < 1e-12);
    assert!(cell0_oil.accumulation.abs() < 1e-12);
    assert!(cell1_oil.accumulation.abs() < 1e-12);
    assert!(cell0_water.well_source.abs() < 1e-12);
    assert!(cell1_water.well_source.abs() < 1e-12);
    assert!(cell0_oil.well_source.abs() < 1e-12);
    assert!(cell1_oil.well_source.abs() < 1e-12);
    assert!(cell0_water.x_plus.abs() > 1e-9);
    assert!(cell0_oil.x_plus.abs() > 1e-9);
    assert!((cell0_water.x_plus + cell1_water.x_minus).abs() < 1e-12);
    assert!((cell0_oil.x_plus + cell1_oil.x_minus).abs() < 1e-12);
    assert!((assembly.residual[equation_offset(0, 0)] - cell0_water.total).abs() < 1e-12);
    assert!((assembly.residual[equation_offset(1, 0)] - cell1_water.total).abs() < 1e-12);
    assert!((assembly.residual[equation_offset(0, 1)] - cell0_oil.total).abs() < 1e-12);
    assert!((assembly.residual[equation_offset(1, 1)] - cell1_oil.total).abs() < 1e-12);

    let water_sum =
        assembly.residual[equation_offset(0, 0)] + assembly.residual[equation_offset(1, 0)];
    let oil_sum =
        assembly.residual[equation_offset(0, 1)] + assembly.residual[equation_offset(1, 1)];

    assert!(water_sum.abs() < 1e-12);
    assert!(oil_sum.abs() < 1e-12);
}

#[test]
fn residual_only_two_cell_flux_is_component_conservative_for_three_phase_gas() {
    let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
    sim.set_three_phase_mode_enabled(true);
    sim.set_gas_redissolution_enabled(false);
    sim.set_three_phase_rel_perm_props(0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7)
        .unwrap();
    sim.pvt_table = Some(PvtTable::new(
        vec![
            PvtRow {
                p_bar: 100.0,
                rs_m3m3: 10.0,
                bo_m3m3: 1.2,
                mu_o_cp: 1.4,
                bg_m3m3: 0.02,
                mu_g_cp: 0.03,
            },
            PvtRow {
                p_bar: 200.0,
                rs_m3m3: 20.0,
                bo_m3m3: 1.1,
                mu_o_cp: 1.2,
                bg_m3m3: 0.01,
                mu_g_cp: 0.025,
            },
            PvtRow {
                p_bar: 400.0,
                rs_m3m3: 40.0,
                bo_m3m3: 1.0,
                mu_o_cp: 1.0,
                bg_m3m3: 0.005,
                mu_g_cp: 0.02,
            },
        ],
        sim.pvt.c_o,
    ));

    let mut state = FimState::from_simulator(&sim);
    state.cells[0].pressure_bar = 260.0;
    state.cells[1].pressure_bar = 180.0;
    state.cells[0].sw = 0.18;
    state.cells[1].sw = 0.32;
    state.cells[0].regime = HydrocarbonState::Saturated;
    state.cells[0].hydrocarbon_var = 0.14;
    state.cells[1].regime = HydrocarbonState::Saturated;
    state.cells[1].hydrocarbon_var = 0.05;

    let topology = build_well_topology(&sim);
    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: false,
            assemble_residual_only: true,
            topology: Some(&topology),
        },
    );

    assert_eq!(assembly.jacobian.nnz(), 0);

    let cell0_gas = cell_equation_residual_breakdown(&sim, &state, &state, &topology, 1.0, 0, 2)
        .expect("cell 0 gas breakdown should exist");
    let cell1_gas = cell_equation_residual_breakdown(&sim, &state, &state, &topology, 1.0, 1, 2)
        .expect("cell 1 gas breakdown should exist");
    assert!(cell0_gas.accumulation.abs() < 1e-12);
    assert!(cell1_gas.accumulation.abs() < 1e-12);
    assert!(cell0_gas.well_source.abs() < 1e-12);
    assert!(cell1_gas.well_source.abs() < 1e-12);
    assert!(cell0_gas.x_plus.abs() > 1e-9);
    assert!((cell0_gas.x_plus + cell1_gas.x_minus).abs() < 1e-12);
    assert!((assembly.residual[equation_offset(0, 2)] - cell0_gas.total).abs() < 1e-12);
    assert!((assembly.residual[equation_offset(1, 2)] - cell1_gas.total).abs() < 1e-12);

    let gas_sum =
        assembly.residual[equation_offset(0, 2)] + assembly.residual[equation_offset(1, 2)];
    assert!(gas_sum.abs() < 1e-12);
}

#[test]
fn gas_component_flux_includes_dissolved_gas_term_with_upwind_rs_sign() {
    let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
    sim.set_three_phase_mode_enabled(true);
    sim.set_gas_redissolution_enabled(true);
    sim.set_gravity_enabled(false);
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_gas_oil_capillary_params(0.0, 2.0).unwrap();
    sim.set_three_phase_rel_perm_props(0.10, 0.10, 0.05, 0.05, 0.10, 2.0, 2.0, 1.5, 0.8, 0.9, 0.7)
        .unwrap();
    sim.pvt_table = Some(PvtTable::new(
        vec![
            PvtRow {
                p_bar: 100.0,
                rs_m3m3: 5.0,
                bo_m3m3: 1.15,
                mu_o_cp: 1.3,
                bg_m3m3: 0.02,
                mu_g_cp: 0.03,
            },
            PvtRow {
                p_bar: 200.0,
                rs_m3m3: 15.0,
                bo_m3m3: 1.05,
                mu_o_cp: 1.1,
                bg_m3m3: 0.01,
                mu_g_cp: 0.025,
            },
            PvtRow {
                p_bar: 300.0,
                rs_m3m3: 25.0,
                bo_m3m3: 0.98,
                mu_o_cp: 0.95,
                bg_m3m3: 0.006,
                mu_g_cp: 0.02,
            },
        ],
        sim.pvt.c_o,
    ));

    let mut state = FimState::from_simulator(&sim);
    state.cells[0].pressure_bar = 260.0;
    state.cells[1].pressure_bar = 210.0;
    state.cells[0].sw = 0.25;
    state.cells[1].sw = 0.25;
    state.cells[0].regime = HydrocarbonState::Undersaturated;
    state.cells[0].hydrocarbon_var = 20.0;
    state.cells[1].regime = HydrocarbonState::Undersaturated;
    state.cells[1].hydrocarbon_var = 5.0;

    let derived0 = state.derive_cell(&sim, 0);
    let derived1 = state.derive_cell(&sim, 1);
    assert!(derived0.sg.abs() < 1e-12);
    assert!(derived1.sg.abs() < 1e-12);

    let diagnostics = cell_face_phase_flux_diagnostics(&sim, &state, 1.0, 0)
        .expect("cell 0 face diagnostics should exist");
    let face = diagnostics.x_plus.expect("cell 0 should have an x+ face");

    let topology = build_well_topology(&sim);
    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: false,
            assemble_residual_only: true,
            topology: Some(&topology),
        },
    );
    let cell0_gas = cell_equation_residual_breakdown(&sim, &state, &state, &topology, 1.0, 0, 2)
        .expect("cell 0 gas breakdown should exist");
    let cell1_gas = cell_equation_residual_breakdown(&sim, &state, &state, &topology, 1.0, 1, 2)
        .expect("cell 1 gas breakdown should exist");

    let expected_dissolved_gas_flux = face.oil.flux * derived0.rs;
    let gas_flux_rel_err = (face.gas.flux - expected_dissolved_gas_flux).abs()
        / expected_dissolved_gas_flux.abs().max(1e-12);

    assert_eq!(face.oil.upwind_cell_idx, 0);
    assert_eq!(face.gas.upwind_cell_idx, 0);
    assert!(face.oil.flux > 0.0);
    assert_eq!(face.gas.mobility, 0.0);
    assert!(face.gas.flux > 0.0);
    assert!(gas_flux_rel_err < 1e-12);
    assert!((cell0_gas.x_plus - face.gas.flux).abs() < 1e-12);
    assert!((cell1_gas.x_minus + face.gas.flux).abs() < 1e-12);
    assert!((assembly.residual[equation_offset(0, 2)] - face.gas.flux).abs() < 1e-12);
    assert!((assembly.residual[equation_offset(1, 2)] + face.gas.flux).abs() < 1e-12);
}

#[test]
fn direct_transmissibility_formula_matches_homogeneous_two_cell_oil_flux() {
    let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
    sim.set_cell_dimensions(20.0, 10.0, 5.0).unwrap();
    sim.set_rel_perm_props(0.10, 0.10, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_initial_pressure(200.0);
    sim.set_initial_saturation(0.20);
    sim.set_gravity_enabled(false);
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.set_permeability_per_layer(vec![150.0], vec![150.0], vec![150.0])
        .unwrap();

    let mut state = FimState::from_simulator(&sim);
    state.cells[0].pressure_bar = 300.0;
    state.cells[1].pressure_bar = 200.0;
    state.cells[0].sw = 0.20;
    state.cells[1].sw = 0.20;

    let diagnostics = cell_face_phase_flux_diagnostics(&sim, &state, 1.0, 0)
        .expect("cell 0 face diagnostics should exist");
    let oil = diagnostics
        .x_plus
        .expect("cell 0 should have an x+ face")
        .oil;

    let area = sim.dy * sim.dz_at(0);
    let expected_t = sim.perm_x[0] * area / sim.dx;
    let expected_flux = DARCY_METRIC_FACTOR
        * expected_t
        * (state.cells[0].pressure_bar - state.cells[1].pressure_bar)
        * oil.mobility
        / sim
            .get_b_o_cell(
                oil.upwind_cell_idx,
                state.cells[oil.upwind_cell_idx].pressure_bar,
            )
            .max(1e-9);

    assert_eq!(oil.upwind_cell_idx, 0);
    assert!((oil.dphi - 100.0).abs() < 1e-12);
    assert!((oil.flux - expected_flux).abs() / expected_flux.abs().max(1e-12) < 1e-12);
}

#[test]
fn direct_transmissibility_formula_matches_heterogeneous_two_cell_oil_flux() {
    let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
    sim.set_cell_dimensions(20.0, 10.0, 5.0).unwrap();
    sim.set_rel_perm_props(0.10, 0.10, 2.0, 2.0, 1.0, 1.0)
        .unwrap();
    sim.set_initial_pressure(200.0);
    sim.set_initial_saturation(0.25);
    sim.set_gravity_enabled(false);
    sim.set_capillary_params(0.0, 2.0).unwrap();
    sim.perm_x[0] = 100.0;
    sim.perm_x[1] = 400.0;

    let mut state = FimState::from_simulator(&sim);
    state.cells[0].pressure_bar = 260.0;
    state.cells[1].pressure_bar = 200.0;
    state.cells[0].sw = 0.25;
    state.cells[1].sw = 0.25;

    let diagnostics = cell_face_phase_flux_diagnostics(&sim, &state, 1.0, 0)
        .expect("cell 0 face diagnostics should exist");
    let oil = diagnostics
        .x_plus
        .expect("cell 0 should have an x+ face")
        .oil;

    let area = sim.dy * sim.dz_at(0);
    let harmonic_k = 2.0 * sim.perm_x[0] * sim.perm_x[1] / (sim.perm_x[0] + sim.perm_x[1]);
    let expected_t = harmonic_k * area / sim.dx;
    let expected_flux = DARCY_METRIC_FACTOR
        * expected_t
        * (state.cells[0].pressure_bar - state.cells[1].pressure_bar)
        * oil.mobility
        / sim
            .get_b_o_cell(
                oil.upwind_cell_idx,
                state.cells[oil.upwind_cell_idx].pressure_bar,
            )
            .max(1e-9);

    assert_eq!(oil.upwind_cell_idx, 0);
    assert!((oil.dphi - 60.0).abs() < 1e-12);
    assert!((oil.flux - expected_flux).abs() / expected_flux.abs().max(1e-12) < 1e-12);
}

#[test]
fn exact_vertical_flux_jacobian_matches_local_fd_oracle() {
    let mut sim = ReservoirSimulator::new(1, 1, 2, 0.2);
    sim.set_three_phase_mode_enabled(true);
    sim.set_gravity_enabled(true);
    sim.set_capillary_params(4.0, 2.0).unwrap();
    sim.set_gas_oil_capillary_params(3.0, 1.8).unwrap();
    sim.pvt_table = Some(PvtTable::new(
        vec![
            PvtRow {
                p_bar: 100.0,
                rs_m3m3: 10.0,
                bo_m3m3: 1.2,
                mu_o_cp: 1.4,
                bg_m3m3: 0.02,
                mu_g_cp: 0.03,
            },
            PvtRow {
                p_bar: 200.0,
                rs_m3m3: 25.0,
                bo_m3m3: 1.05,
                mu_o_cp: 1.1,
                bg_m3m3: 0.01,
                mu_g_cp: 0.025,
            },
            PvtRow {
                p_bar: 300.0,
                rs_m3m3: 40.0,
                bo_m3m3: 0.95,
                mu_o_cp: 0.95,
                bg_m3m3: 0.006,
                mu_g_cp: 0.02,
            },
        ],
        sim.pvt.c_o,
    ));

    let mut state = FimState::from_simulator(&sim);
    state.cells[0].pressure_bar = 260.0;
    state.cells[0].sw = 0.28;
    state.cells[0].hydrocarbon_var = 0.08;
    state.cells[0].regime = HydrocarbonState::Saturated;
    state.cells[1].pressure_bar = 185.0;
    state.cells[1].sw = 0.44;
    state.cells[1].hydrocarbon_var = 0.14;
    state.cells[1].regime = HydrocarbonState::Saturated;

    let n_cells = state.cells.len();
    let derived: Vec<FimCellDerived> = (0..n_cells)
        .map(|idx| state.derive_cell(&sim, idx))
        .collect();
    let sensitivities: Vec<LocalFluxCellSensitivity> = (0..n_cells)
        .map(|idx| local_flux_cell_sensitivity(&sim, &state, idx, &derived[idx]))
        .collect();
    let mut exact = TriMatI::<f64, usize>::new((state.n_unknowns(), state.n_unknowns()));
    add_exact_flux_jacobian(&sim, &state, 1.0, &derived, &sensitivities, &mut exact);
    let exact = exact.to_csr();

    let mut fd = TriMatI::<f64, usize>::new((state.n_unknowns(), state.n_unknowns()));
    add_local_flux_jacobian_fd(&sim, &state, 1.0, &mut fd);
    let fd = fd.to_csr();

    for row in 0..state.n_cell_unknowns() {
        for col in 0..state.n_cell_unknowns() {
            let exact_value = jacobian_value(&exact, row, col);
            let fd_value = jacobian_value(&fd, row, col);
            let tolerance = 2e-3 * exact_value.abs().max(fd_value.abs()).max(1.0);
            assert!(
                (exact_value - fd_value).abs() <= tolerance,
                "flux jacobian mismatch at ({}, {}): exact={}, fd={}, tol={}",
                row,
                col,
                exact_value,
                fd_value,
                tolerance
            );
        }
    }
}

#[test]
fn water_injector_adds_negative_water_source_term() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_injected_fluid("water").unwrap();
    sim.add_well(0, 0, 0, 1000.0, 0.1, 0.0, true).unwrap();
    sim.injector_rate_controlled = false;
    sim.injector_enabled = true;

    let state = FimState::from_simulator(&sim);
    let baseline = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: false,
            assemble_residual_only: false,
            topology: None,
        },
    );
    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: true,
            assemble_residual_only: false,
            topology: None,
        },
    );

    assert!(baseline.residual.norm() <= 1e-12);
    assert!(assembly.residual[equation_offset(0, 0)] < baseline.residual[equation_offset(0, 0)]);
    assert!(
        (assembly.residual[equation_offset(0, 1)] - baseline.residual[equation_offset(0, 1)]).abs()
            < 1e-12
    );
    assert!(
        (assembly.residual[equation_offset(0, 2)] - baseline.residual[equation_offset(0, 2)]).abs()
            < 1e-12
    );
}

#[test]
fn producer_source_uses_iterate_state_phase_split() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.add_well(0, 0, 0, 0.0, 0.1, 0.0, false).unwrap();
    let low_sw_state = FimState::from_simulator(&sim);
    let mut high_sw_state = low_sw_state.clone();
    high_sw_state.cells[0].sw = 0.8;

    let low_sw_assembly = assemble_fim_system(
        &sim,
        &low_sw_state,
        &low_sw_state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: true,
            assemble_residual_only: false,
            topology: None,
        },
    );
    let high_sw_assembly = assemble_fim_system(
        &sim,
        &high_sw_state,
        &high_sw_state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: true,
            assemble_residual_only: false,
            topology: None,
        },
    );

    assert!(
        high_sw_assembly.residual[equation_offset(0, 0)]
            > low_sw_assembly.residual[equation_offset(0, 0)]
    );
    assert!(
        high_sw_assembly.residual[equation_offset(0, 1)]
            < low_sw_assembly.residual[equation_offset(0, 1)]
    );
}

#[test]
fn rate_control_adds_extra_unknowns_and_equations() {
    let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
    sim.set_rate_controlled_wells(true);
    sim.set_injected_fluid("water").unwrap();
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
    sim.add_well(1, 0, 0, 50.0, 0.1, 0.0, false).unwrap();
    let state = FimState::from_simulator(&sim);

    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: true,
            assemble_residual_only: false,
            topology: None,
        },
    );

    assert_eq!(state.n_unknowns(), 10);
    assert_eq!(assembly.residual.len(), 10);
    assert_eq!(assembly.equation_scaling.well_constraint.len(), 2);
    assert_eq!(assembly.equation_scaling.perforation_flow.len(), 2);
    assert_eq!(assembly.variable_scaling.well_bhp.len(), 2);
    assert_eq!(assembly.variable_scaling.perforation_rate.len(), 2);
}

#[test]
fn well_source_and_perforation_rows_have_exact_q_coupling_entries() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_injected_fluid("water").unwrap();
    sim.add_well(0, 0, 0, 1000.0, 0.1, 0.0, true).unwrap();

    let state = FimState::from_simulator(&sim);
    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 2.0,
            include_wells: true,
            assemble_residual_only: false,
            topology: None,
        },
    );

    let q_column = state.perforation_rate_unknown_offset(0);
    let perf_row = state.perforation_equation_offset(0);
    let water_row = equation_offset(0, 0);

    assert!((jacobian_value(&assembly.jacobian, perf_row, q_column) - 1.0).abs() < 1e-12);
    assert!(
        (jacobian_value(&assembly.jacobian, water_row, q_column) - 2.0 / sim.b_w.max(1e-9)).abs()
            < 1e-12
    );
}

#[test]
fn water_injector_perforation_row_has_exact_pressure_derivative() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_injected_fluid("water").unwrap();
    sim.add_well(0, 0, 0, 250.0, 0.1, 0.0, true).unwrap();
    sim.injector_rate_controlled = false;

    let state = FimState::from_simulator(&sim);
    let topology = build_well_topology(&sim);
    let expected = -crate::fim::wells::perforation_connection_pressure_derivative(
        &sim,
        &state,
        &topology,
        0,
        state.well_bhp[0],
    )
    .unwrap();

    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: true,
            assemble_residual_only: false,
            topology: None,
        },
    );

    let row = state.perforation_equation_offset(0);
    let col = unknown_offset(0, 0);
    assert!((jacobian_value(&assembly.jacobian, row, col) - expected).abs() < 1e-10);
}

#[test]
fn producer_perforation_row_has_exact_local_connection_derivatives() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_three_phase_rel_perm_props(
        0.12, 0.12, 0.04, 0.04, 0.18, 2.0, 2.5, 1.5, 1e-5, 1.0, 0.984,
    )
    .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

    let mut state = FimState::from_simulator(&sim);
    state.cells[0].pressure_bar = 220.0;
    state.cells[0].sw = 0.25;
    state.cells[0].hydrocarbon_var = 0.15;

    let topology = build_well_topology(&sim);
    let expected = crate::fim::wells::perforation_connection_cell_derivatives(
        &sim,
        &state,
        &topology,
        0,
        state.well_bhp[0],
    )
    .unwrap();

    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: true,
            assemble_residual_only: false,
            topology: None,
        },
    );

    let row = state.perforation_equation_offset(0);
    for (local_var, derivative) in expected.into_iter().enumerate() {
        let value = jacobian_value(&assembly.jacobian, row, unknown_offset(0, local_var));
        assert!((value + derivative).abs() < 1e-9);
    }
}

#[test]
fn producer_source_row_matches_exact_neighborhood_derivative() {
    let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
    sim.set_three_phase_rel_perm_props(
        0.12, 0.12, 0.04, 0.04, 0.18, 2.0, 2.5, 1.5, 1e-5, 1.0, 0.984,
    )
    .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.add_well(1, 1, 0, 100.0, 0.1, 0.0, false).unwrap();

    let mut state = FimState::from_simulator(&sim);
    state.cells[sim.idx(1, 1, 0)].pressure_bar = 220.0;
    state.cells[sim.idx(1, 1, 0)].sw = 0.25;
    state.cells[sim.idx(1, 1, 0)].hydrocarbon_var = 0.15;
    state.perforation_rates_m3_day[0] = 40.0;

    let topology = build_well_topology(&sim);
    let expected = crate::fim::wells::perforation_component_rate_cell_derivatives_sc_day_by_var(
        &sim,
        &state,
        &topology,
        0,
        sim.idx(1, 1, 0),
    );

    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: true,
            assemble_residual_only: false,
            topology: None,
        },
    );
    let baseline = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: false,
            assemble_residual_only: false,
            topology: None,
        },
    );

    let row_cell = sim.idx(1, 1, 0);
    for local_var in 0..3 {
        let col = unknown_offset(row_cell, local_var);
        for component in 0..3 {
            let row = equation_offset(row_cell, component);
            let value = jacobian_value(&assembly.jacobian, row, col)
                - jacobian_value(&baseline.jacobian, row, col);
            assert!((value - expected[local_var][component]).abs() < 1e-9);
        }
    }
}

#[test]
fn producer_control_row_matches_exact_surface_rate_derivative() {
    let mut sim = ReservoirSimulator::new(2, 2, 1, 0.2);
    sim.set_three_phase_rel_perm_props(
        0.12, 0.12, 0.04, 0.04, 0.18, 2.0, 2.5, 1.5, 1e-5, 1.0, 0.984,
    )
    .unwrap();
    sim.set_three_phase_mode_enabled(true);
    sim.add_well(1, 1, 0, 100.0, 0.1, 0.0, false).unwrap();
    sim.producer_rate_controlled = true;
    sim.target_producer_surface_rate_m3_day = Some(25.0);
    sim.target_producer_rate_m3_day = 25.0;
    sim.well_bhp_min = 50.0;

    let mut state = FimState::from_simulator(&sim);
    state.cells[sim.idx(1, 1, 0)].pressure_bar = 220.0;
    state.cells[sim.idx(1, 1, 0)].sw = 0.25;
    state.cells[sim.idx(1, 1, 0)].hydrocarbon_var = 0.15;
    state.perforation_rates_m3_day[0] = 40.0;

    let topology = build_well_topology(&sim);
    let control = crate::fim::wells::physical_well_control(&sim, &topology, 0);
    let (bhp_slack, rate_slack) =
        crate::fim::wells::well_control_slacks(&sim, &state, &topology, 0).unwrap();
    let bhp_scale = control.bhp_limit.abs().max(1.0);
    let rate_scale = control.target_rate.unwrap_or(1.0).abs().max(1.0);
    let (_, dphi_db) = crate::fim::wells::fischer_burmeister_gradient(
        bhp_slack / bhp_scale,
        rate_slack / rate_scale,
    );
    let d_surface = crate::fim::wells::perforation_surface_rate_cell_derivatives_sc_day(
        &sim,
        &state,
        &topology,
        0,
        sim.idx(1, 1, 0),
    );

    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: true,
            assemble_residual_only: false,
            topology: None,
        },
    );

    let row = state.well_equation_offset(0);
    for (local_var, derivative) in d_surface.into_iter().enumerate() {
        let value = jacobian_value(
            &assembly.jacobian,
            row,
            unknown_offset(sim.idx(1, 1, 0), local_var),
        );
        assert!((value + dphi_db * derivative / rate_scale).abs() < 1e-9);
    }
}

#[test]
fn gas_injector_source_row_has_exact_pressure_conversion_derivative() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.set_injected_fluid("gas").unwrap();
    sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();

    let mut state = FimState::from_simulator(&sim);
    state.perforation_rates_m3_day[0] = -120.0;
    let topology = build_well_topology(&sim);
    let expected = crate::fim::wells::perforation_source_pressure_derivatives_sc_day(
        &sim, &state, &topology, 0,
    )[2];

    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: true,
            assemble_residual_only: false,
            topology: None,
        },
    );

    let row = equation_offset(0, 2);
    let col = unknown_offset(0, 0);
    assert!((jacobian_value(&assembly.jacobian, row, col) - expected).abs() < 1e-10);
}

#[test]
fn bhp_controlled_well_constraint_row_has_unit_bhp_derivative() {
    let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    sim.add_well(0, 0, 0, 100.0, 0.1, 0.0, false).unwrap();

    let state = FimState::from_simulator(&sim);
    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: true,
            assemble_residual_only: false,
            topology: None,
        },
    );

    let row = state.well_equation_offset(0);
    let column = state.well_bhp_unknown_offset(0);
    assert!((jacobian_value(&assembly.jacobian, row, column) - 1.0).abs() < 1e-12);
}

#[test]
fn flux_jacobian_couples_only_face_local_cells() {
    let sim = ReservoirSimulator::new(3, 1, 1, 0.2);
    let mut state = FimState::from_simulator(&sim);
    state.cells[0].pressure_bar = 250.0;
    state.cells[1].pressure_bar = 200.0;
    state.cells[2].pressure_bar = 150.0;

    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: false,
            assemble_residual_only: false,
            topology: None,
        },
    );

    let row_cell0_water = equation_offset(0, 0);
    let col_cell1_pressure = unknown_offset(1, 0);
    let col_cell2_pressure = unknown_offset(2, 0);

    assert!(jacobian_value(&assembly.jacobian, row_cell0_water, col_cell1_pressure).abs() > 1e-12);
    assert!(jacobian_value(&assembly.jacobian, row_cell0_water, col_cell2_pressure).abs() < 1e-12);
}

#[test]
fn producer_well_source_jacobian_stays_within_completion_neighborhood() {
    let mut sim = ReservoirSimulator::new(5, 1, 1, 0.2);
    sim.add_well(1, 0, 0, 0.0, 0.1, 0.0, false).unwrap();

    let mut state = FimState::from_simulator(&sim);
    state.cells[0].sw = 0.15;
    state.cells[1].sw = 0.35;
    state.cells[2].sw = 0.55;
    state.cells[3].sw = 0.75;
    state.cells[4].sw = 0.25;

    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: true,
            assemble_residual_only: false,
            topology: None,
        },
    );

    let row = equation_offset(sim.idx(1, 0, 0), 0);
    let col_neighbor_sw = unknown_offset(sim.idx(0, 0, 0), 1);
    let col_far_sw = unknown_offset(sim.idx(4, 0, 0), 1);

    assert!(jacobian_value(&assembly.jacobian, row, col_neighbor_sw).abs() > 1e-12);
    assert!(jacobian_value(&assembly.jacobian, row, col_far_sw).abs() < 1e-12);
}

#[test]
fn perforation_equation_jacobian_only_couples_to_completion_cell() {
    let mut sim = ReservoirSimulator::new(3, 1, 1, 0.2);
    sim.add_well(1, 0, 0, 0.0, 0.1, 0.0, false).unwrap();

    let mut state = FimState::from_simulator(&sim);
    state.cells[1].pressure_bar = 180.0;
    state.cells[0].pressure_bar = 220.0;
    state.cells[2].pressure_bar = 140.0;

    let assembly = assemble_fim_system(
        &sim,
        &state,
        &state,
        &FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: true,
            assemble_residual_only: false,
            topology: None,
        },
    );

    let row = state.perforation_equation_offset(0);
    let col_local_pressure = unknown_offset(sim.idx(1, 0, 0), 0);
    let col_neighbor_pressure = unknown_offset(sim.idx(0, 0, 0), 0);

    assert!(jacobian_value(&assembly.jacobian, row, col_local_pressure).abs() > 1e-12);
    assert!(jacobian_value(&assembly.jacobian, row, col_neighbor_pressure).abs() < 1e-12);
}

#[test]
fn accumulation_block_has_exact_water_derivatives() {
    let sim = ReservoirSimulator::new(1, 1, 1, 0.2);
    let previous_state = FimState::from_simulator(&sim);
    let state = previous_state.clone();
    let d = state.derive_cell(&sim, 0);
    let block = cell_accumulation_jacobian_block(&sim, &previous_state, &state, 0, &d);
    let pv = sim.pore_volume_m3(0).max(1e-9);

    assert!(
        (block[0][0] - pv * sim.rock_compressibility * state.cells[0].sw / sim.b_w).abs() < 1e-12
    );
    assert!((block[0][1] - pv / sim.b_w).abs() < 1e-12);
    assert!(block[0][2].abs() < 1e-12);
}

#[test]
fn water_accumulation_residual_scales_with_bw_denominator() {
    let mut bw1 = ReservoirSimulator::new(1, 1, 1, 0.2);
    bw1.set_rock_properties(
        bw1.rock_compressibility,
        bw1.depth_reference_m,
        bw1.b_o,
        1.0,
    )
    .unwrap();

    let mut bw2 = ReservoirSimulator::new(1, 1, 1, 0.2);
    bw2.set_rock_properties(
        bw2.rock_compressibility,
        bw2.depth_reference_m,
        bw2.b_o,
        2.0,
    )
    .unwrap();

    let previous_state_bw1 = FimState::from_simulator(&bw1);
    let previous_state_bw2 = FimState::from_simulator(&bw2);
    let mut state_bw1 = previous_state_bw1.clone();
    let mut state_bw2 = previous_state_bw2.clone();

    state_bw1.cells[0].pressure_bar += 12.0;
    state_bw1.cells[0].sw += 0.08;
    state_bw2.cells[0].pressure_bar += 12.0;
    state_bw2.cells[0].sw += 0.08;

    let derived_bw1 = state_bw1.derive_cell(&bw1, 0);
    let derived_bw2 = state_bw2.derive_cell(&bw2, 0);
    let prev_derived_bw1 = previous_state_bw1.derive_cell(&bw1, 0);
    let prev_derived_bw2 = previous_state_bw2.derive_cell(&bw2, 0);

    let water_residual_bw1 = cell_accumulation_residual(
        &bw1,
        &previous_state_bw1,
        &state_bw1,
        0,
        &derived_bw1,
        &prev_derived_bw1,
    )[0];
    let water_residual_bw2 = cell_accumulation_residual(
        &bw2,
        &previous_state_bw2,
        &state_bw2,
        0,
        &derived_bw2,
        &prev_derived_bw2,
    )[0];

    assert!(water_residual_bw1.abs() > 1e-12);
    assert!((water_residual_bw1 / water_residual_bw2 - 2.0).abs() < 1e-12);
}

#[test]
#[ignore = "Expensive test that can be enabled for debugging specific Jacobian issues"]
fn full_system_jacobian_matches_fd_for_rate_controlled_waterflood() {
    assert_full_system_fd_matches_for_fixture(
        &(0..10).collect::<Vec<_>>(),
        build_rate_controlled_waterflood_fd_fixture,
    );
}

#[test]
#[ignore = "diagnostic: sampled full-system central-FD Jacobian"]
fn representative_full_system_jacobian_columns_match_fd_for_rate_controlled_waterflood() {
    assert_full_system_fd_matches_for_fixture(
        &[0, 1, 2, 6, 8, 9],
        build_rate_controlled_waterflood_fd_fixture,
    );
}
