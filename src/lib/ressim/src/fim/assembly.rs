use nalgebra::DVector;
use sprs::{CsMat, TriMatI};

use crate::fim::scaling::{
    build_equation_scaling, build_variable_scaling, EquationScaling, VariableScaling,
};
use crate::fim::state::{FimCellDerived, FimState, HydrocarbonState};
use crate::fim::wells::{
    build_well_topology, fischer_burmeister_gradient, perforation_component_rate_derivatives_sc_day,
    perforation_component_rate_cell_derivatives_sc_day_by_var, perforation_component_rates_sc_day,
    perforation_connection_bhp_derivative, perforation_connection_cell_derivatives,
    perforation_rate_residual, perforation_surface_rate_cell_derivatives_sc_day,
    perforation_target_rate_derivative, physical_well_control, well_constraint_residual,
    well_control_slacks, FimWellTopology,
};
use crate::ReservoirSimulator;

const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;

pub(crate) struct FimAssembly {
    pub(crate) residual: DVector<f64>,
    pub(crate) jacobian: CsMat<f64>,
    pub(crate) equation_scaling: EquationScaling,
    pub(crate) variable_scaling: VariableScaling,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimAssemblyOptions<'a> {
    pub(crate) dt_days: f64,
    pub(crate) include_wells: bool,
    pub(crate) topology: Option<&'a FimWellTopology>,
}

pub(crate) fn unknown_offset(cell_idx: usize, local_var: usize) -> usize {
    cell_idx * 3 + local_var
}

pub(crate) fn equation_offset(cell_idx: usize, local_eq: usize) -> usize {
    cell_idx * 3 + local_eq
}

pub(crate) fn assemble_fim_system(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    options: &FimAssemblyOptions,
) -> FimAssembly {
    let owned_topology;
    let topology = if let Some(cached) = options.topology {
        cached
    } else {
        owned_topology = build_well_topology(sim);
        &owned_topology
    };
    let n_cells = state.cells.len();
    let n_unknowns = state.n_unknowns();
    let equation_scaling = build_equation_scaling(sim, state, &topology, options.dt_days);
    let variable_scaling = build_variable_scaling(sim, state);

    // Pre-compute derived properties for all cells (avoids redundant PVT lookups).
    let derived: Vec<FimCellDerived> = (0..n_cells).map(|idx| state.derive_cell(sim, idx)).collect();
    let prev_derived: Vec<FimCellDerived> = (0..n_cells).map(|idx| previous_state.derive_cell(sim, idx)).collect();
    // Pre-compute local flux sensitivities for the Jacobian (each cell appears in ~5 interfaces).
    let sensitivities: Vec<LocalFluxCellSensitivity> = (0..n_cells)
        .map(|idx| local_flux_cell_sensitivity(sim, state, idx, &derived[idx]))
        .collect();

    let mut tri = TriMatI::<f64, usize>::new((n_unknowns, n_unknowns));
    let residual = assemble_residual(sim, previous_state, state, &topology, options, &derived, &prev_derived);

    add_exact_accumulation_jacobian(sim, previous_state, state, &derived, &mut tri);
    add_exact_flux_jacobian(sim, state, options.dt_days, &derived, &sensitivities, &mut tri);

    if options.include_wells {
        add_exact_well_source_jacobian(sim, state, &topology, options.dt_days, &mut tri);
        add_exact_well_source_cell_jacobian(sim, state, &topology, options.dt_days, &mut tri);
        add_exact_well_constraint_jacobian(sim, state, &topology, &mut tri);
        add_exact_well_constraint_cell_jacobian(sim, state, &topology, &mut tri);
        add_exact_perforation_jacobian(sim, state, &topology, &mut tri);
        add_exact_perforation_cell_pressure_jacobian(sim, state, &topology, &mut tri);
    }

    FimAssembly {
        residual,
        jacobian: tri.to_csr(),
        equation_scaling,
        variable_scaling,
    }
}

fn add_exact_well_source_jacobian(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    dt_days: f64,
    tri: &mut TriMatI<f64, usize>,
) {
    for perf_idx in 0..topology.perforations.len() {
        let column = state.perforation_rate_unknown_offset(perf_idx);
        let cell_idx = topology.perforations[perf_idx].cell_index;
        let derivatives = perforation_component_rate_derivatives_sc_day(sim, state, topology, perf_idx);
        for (local_eq, derivative) in derivatives.into_iter().enumerate() {
            let value = derivative * dt_days;
            if value.abs() > 1e-14 {
                tri.add_triplet(equation_offset(cell_idx, local_eq), column, value);
            }
        }
    }
}

fn add_exact_well_source_cell_jacobian(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    dt_days: f64,
    tri: &mut TriMatI<f64, usize>,
) {
    for perf_idx in 0..topology.perforations.len() {
        let row_cell = topology.perforations[perf_idx].cell_index;
        for cell_idx in perforation_control_influence_cells(sim, topology, perf_idx) {
            let derivatives =
                perforation_component_rate_cell_derivatives_sc_day_by_var(sim, state, topology, perf_idx, cell_idx);
            for (local_var, component_derivatives) in derivatives.into_iter().enumerate() {
                let column = unknown_offset(cell_idx, local_var);
                for (local_eq, derivative) in component_derivatives.into_iter().enumerate() {
                    let value = derivative * dt_days;
                    if value.abs() > 1e-14 {
                        tri.add_triplet(equation_offset(row_cell, local_eq), column, value);
                    }
                }
            }
        }
    }
}

fn add_exact_well_constraint_jacobian(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    tri: &mut TriMatI<f64, usize>,
) {
    for well_idx in 0..topology.wells.len() {
        let row = state.well_equation_offset(well_idx);
        let column_bhp = state.well_bhp_unknown_offset(well_idx);
        let control = physical_well_control(sim, topology, well_idx);

        if !control.enabled || !control.rate_controlled {
            tri.add_triplet(row, column_bhp, 1.0);
            continue;
        }

        let Some((bhp_slack, rate_slack)) = well_control_slacks(sim, state, topology, well_idx) else {
            continue;
        };
        let bhp_scale = control.bhp_limit.abs().max(1.0);
        let rate_scale = control.target_rate.unwrap_or(1.0).abs().max(1.0);
        let (dphi_da, dphi_db) = fischer_burmeister_gradient(bhp_slack / bhp_scale, rate_slack / rate_scale);
        let dslack_dbhp = if topology.wells[well_idx].injector { -1.0 } else { 1.0 };
        let bhp_value = dphi_da * dslack_dbhp / bhp_scale;
        if bhp_value.abs() > 1e-14 {
            tri.add_triplet(row, column_bhp, bhp_value);
        }

        for &perf_idx in &topology.wells[well_idx].perforation_indices {
            let column = state.perforation_rate_unknown_offset(perf_idx);
            let dactual_dq = perforation_target_rate_derivative(sim, state, topology, perf_idx);
            let value = -dphi_db * dactual_dq / rate_scale;
            if value.abs() > 1e-14 {
                tri.add_triplet(row, column, value);
            }
        }
    }
}

fn add_exact_well_constraint_cell_jacobian(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    tri: &mut TriMatI<f64, usize>,
) {
    for well_idx in 0..topology.wells.len() {
        let control = physical_well_control(sim, topology, well_idx);
        if !control.enabled || !control.rate_controlled || !control.uses_surface_target {
            continue;
        }

        let Some((bhp_slack, rate_slack)) = well_control_slacks(sim, state, topology, well_idx) else {
            continue;
        };
        let bhp_scale = control.bhp_limit.abs().max(1.0);
        let rate_scale = control.target_rate.unwrap_or(1.0).abs().max(1.0);
        let (_, dphi_db) = fischer_burmeister_gradient(bhp_slack / bhp_scale, rate_slack / rate_scale);
        let row = state.well_equation_offset(well_idx);
        for &perf_idx in &topology.wells[well_idx].perforation_indices {
            for cell_idx in perforation_control_influence_cells(sim, topology, perf_idx) {
                let derivatives =
                    perforation_surface_rate_cell_derivatives_sc_day(sim, state, topology, perf_idx, cell_idx);
                for (local_var, derivative) in derivatives.into_iter().enumerate() {
                    let value = -dphi_db * derivative / rate_scale;
                    if value.abs() > 1e-14 {
                        tri.add_triplet(row, unknown_offset(cell_idx, local_var), value);
                    }
                }
            }
        }
    }
}

fn add_exact_perforation_jacobian(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    tri: &mut TriMatI<f64, usize>,
) {
    for perf_idx in 0..topology.perforations.len() {
        let row = state.perforation_equation_offset(perf_idx);
        let q_column = state.perforation_rate_unknown_offset(perf_idx);
        tri.add_triplet(row, q_column, 1.0);

        let well_idx = topology.perforations[perf_idx].physical_well_index;
        let bhp_column = state.well_bhp_unknown_offset(well_idx);
        let bhp_bar = state.well_bhp[well_idx];
        if let Some(connection_dbhp) =
            perforation_connection_bhp_derivative(sim, state, topology, perf_idx, bhp_bar)
        {
            let value = -connection_dbhp;
            if value.abs() > 1e-14 {
                tri.add_triplet(row, bhp_column, value);
            }
        }
    }
}

fn add_exact_perforation_cell_pressure_jacobian(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    tri: &mut TriMatI<f64, usize>,
) {
    for perf_idx in 0..topology.perforations.len() {
        let well_idx = topology.perforations[perf_idx].physical_well_index;
        let bhp_bar = state.well_bhp[well_idx];
        let Some(connection_derivatives) =
            perforation_connection_cell_derivatives(sim, state, topology, perf_idx, bhp_bar)
        else {
            continue;
        };

        let row = state.perforation_equation_offset(perf_idx);
        let cell_idx = topology.perforations[perf_idx].cell_index;
        for (local_var, derivative) in connection_derivatives.into_iter().enumerate() {
            let value = -derivative;
            if value.abs() > 1e-14 {
                tri.add_triplet(row, unknown_offset(cell_idx, local_var), value);
            }
        }
    }
}

#[cfg(test)]
fn finite_difference_step(state: &FimState, unknown_idx: usize) -> f64 {
    debug_assert!(unknown_idx < state.n_cell_unknowns());
    if unknown_idx < state.n_cell_unknowns() {
        let cell_idx = unknown_idx / 3;
        let local_var = unknown_idx % 3;
        let cell = state.cell(cell_idx);
        return match local_var {
            0 => 1e-5 * cell.pressure_bar.abs().max(1.0),
            1 => 1e-7,
            2 => 1e-7 * cell.hydrocarbon_var.abs().max(1.0),
            _ => unreachable!(),
        };
    }

    unreachable!()
}

#[cfg(test)]
fn perturb_cell_unknown(
    sim: &ReservoirSimulator,
    state: &FimState,
    cell_idx: usize,
    local_var: usize,
) -> (f64, FimState) {
    let unknown_idx = unknown_offset(cell_idx, local_var);
    let perturbation = finite_difference_step(state, unknown_idx);
    let mut update = DVector::zeros(state.n_unknowns());
    update[unknown_idx] = perturbation;
    let perturbed_state = state.apply_newton_update(sim, &update, 1.0);
    (perturbation, perturbed_state)
}

fn perforation_control_influence_cells(
    sim: &ReservoirSimulator,
    topology: &FimWellTopology,
    perf_idx: usize,
) -> Vec<usize> {
    let perforation = &topology.perforations[perf_idx];
    if perforation.injector {
        return vec![perforation.cell_index];
    }

    let i_min = perforation.i.saturating_sub(1);
    let i_max = (perforation.i + 1).min(sim.nx.saturating_sub(1));
    let j_min = perforation.j.saturating_sub(1);
    let j_max = (perforation.j + 1).min(sim.ny.saturating_sub(1));
    let mut cells = Vec::with_capacity((i_max - i_min + 1) * (j_max - j_min + 1));
    for j in j_min..=j_max {
        for i in i_min..=i_max {
            cells.push(sim.idx(i, j, perforation.k));
        }
    }
    cells
}

fn pore_volume_at_state(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    cell_idx: usize,
) -> f64 {
    let pore_volume_ref_m3 = sim.pore_volume_m3(cell_idx);
    let pressure_delta_bar =
        state.cell(cell_idx).pressure_bar - previous_state.cell(cell_idx).pressure_bar;
    pore_volume_ref_m3 * f64::exp(sim.rock_compressibility * pressure_delta_bar)
}

fn cell_component_inventory_sc(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    cell_idx: usize,
    derived: &FimCellDerived,
) -> [f64; 3] {
    let pore_volume_m3 = pore_volume_at_state(sim, previous_state, state, cell_idx).max(1e-9);
    let cell = state.cell(cell_idx);

    let water_sc = pore_volume_m3 * cell.sw / sim.b_w.max(1e-9);
    let oil_sc = pore_volume_m3 * derived.so / derived.bo.max(1e-9);
    let gas_sc = pore_volume_m3 * derived.sg / derived.bg.max(1e-9) + oil_sc * derived.rs;

    [water_sc, oil_sc, gas_sc]
}

fn cell_accumulation_residual(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    cell_idx: usize,
    derived: &FimCellDerived,
    prev_derived: &FimCellDerived,
) -> [f64; 3] {
    let current = cell_component_inventory_sc(sim, previous_state, state, cell_idx, derived);
    let previous = cell_component_inventory_sc(sim, previous_state, previous_state, cell_idx, prev_derived);
    [
        current[0] - previous[0],
        current[1] - previous[1],
        current[2] - previous[2],
    ]
}

fn cell_accumulation_jacobian_block(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    cell_idx: usize,
    derived: &FimCellDerived,
) -> [[f64; 3]; 3] {
    let pore_volume_m3 = pore_volume_at_state(sim, previous_state, state, cell_idx).max(1e-9);
    let d_pore_volume_d_p = pore_volume_m3 * sim.rock_compressibility;
    let cell = state.cell(cell_idx);
    let bw = sim.b_w.max(1e-9);
    let bo = derived.bo.max(1e-9);
    let bg = derived.bg.max(1e-9);

    let saturated = cell.regime == HydrocarbonState::Saturated;
    let d_bo_d_p = sim.get_d_bo_d_p_for_state(cell.pressure_bar, derived.rs, saturated);
    let d_bo_d_rs = if saturated {
        0.0
    } else {
        sim.get_d_bo_d_rs_for_state(cell.pressure_bar, derived.rs)
    };
    let d_bg_d_p = sim.get_d_bg_d_p_for_state(cell.pressure_bar);
    let d_rs_sat_d_p = if saturated {
        sim.get_d_rs_sat_d_p_for_state(cell.pressure_bar)
    } else {
        0.0
    };

    let (d_so_d_sw, d_so_d_h, d_sg_d_h, d_rs_d_h) = match cell.regime {
        HydrocarbonState::Saturated => (-1.0, -1.0, 1.0, 0.0),
        HydrocarbonState::Undersaturated => (-1.0, 0.0, 0.0, 1.0),
    };

    let oil_inventory = pore_volume_m3 * derived.so / bo;
    let d_water_d_p = d_pore_volume_d_p * cell.sw / bw;
    let d_water_d_sw = pore_volume_m3 / bw;

    let d_oil_d_p = d_pore_volume_d_p * derived.so / bo
        - pore_volume_m3 * derived.so * d_bo_d_p / (bo * bo);
    let d_oil_d_sw = pore_volume_m3 * d_so_d_sw / bo;
    let d_oil_d_h = pore_volume_m3 * d_so_d_h / bo
        - pore_volume_m3 * derived.so * d_bo_d_rs * d_rs_d_h / (bo * bo);

    let d_free_gas_d_p = d_pore_volume_d_p * derived.sg / bg
        - pore_volume_m3 * derived.sg * d_bg_d_p / (bg * bg);
    let d_free_gas_d_h = pore_volume_m3 * d_sg_d_h / bg;

    let d_gas_d_p = d_free_gas_d_p + d_oil_d_p * derived.rs + oil_inventory * d_rs_sat_d_p;
    let d_gas_d_sw = d_oil_d_sw * derived.rs;
    let d_gas_d_h = d_free_gas_d_h + d_oil_d_h * derived.rs + oil_inventory * d_rs_d_h;

    [
        [d_water_d_p, d_water_d_sw, 0.0],
        [d_oil_d_p, d_oil_d_sw, d_oil_d_h],
        [d_gas_d_p, d_gas_d_sw, d_gas_d_h],
    ]
}

fn assemble_residual(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    topology: &FimWellTopology,
    options: &FimAssemblyOptions,
    derived: &[FimCellDerived],
    prev_derived: &[FimCellDerived],
) -> DVector<f64> {
    assemble_residual_with_flags(sim, previous_state, state, topology, options, true, true, derived, prev_derived)
}

fn assemble_residual_with_flags(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    topology: &FimWellTopology,
    options: &FimAssemblyOptions,
    include_accumulation: bool,
    include_flux: bool,
    derived: &[FimCellDerived],
    prev_derived: &[FimCellDerived],
) -> DVector<f64> {
    let n_unknowns = state.n_unknowns();
    let mut residual = DVector::zeros(n_unknowns);

    if include_accumulation {
        for cell_idx in 0..state.cells.len() {
            let accumulation = cell_accumulation_residual(sim, previous_state, state, cell_idx, &derived[cell_idx], &prev_derived[cell_idx]);
            for local_eq in 0..3 {
                residual[equation_offset(cell_idx, local_eq)] += accumulation[local_eq];
            }
        }
    }

    if include_flux {
        for k in 0..sim.nz {
            for j in 0..sim.ny {
                for i in 0..sim.nx {
                    let id = sim.idx(i, j, k);

                    if i + 1 < sim.nx {
                        let id_j = sim.idx(i + 1, j, k);
                        add_interface_flux(
                            sim, state, options.dt_days, id, id_j, 'x', k, k,
                            &derived[id], &derived[id_j], &mut residual,
                        );
                    }
                    if j + 1 < sim.ny {
                        let id_j = sim.idx(i, j + 1, k);
                        add_interface_flux(
                            sim, state, options.dt_days, id, id_j, 'y', k, k,
                            &derived[id], &derived[id_j], &mut residual,
                        );
                    }
                    if k + 1 < sim.nz {
                        let id_j = sim.idx(i, j, k + 1);
                        add_interface_flux(
                            sim, state, options.dt_days, id, id_j, 'z', k, k + 1,
                            &derived[id], &derived[id_j], &mut residual,
                        );
                    }
                }
            }
        }
    }

    if options.include_wells {
        add_well_source_terms(sim, state, topology, options.dt_days, &mut residual);
        add_well_constraint_equations(sim, state, topology, &mut residual);
        add_perforation_equations(sim, state, topology, &mut residual);
    }

    residual
}

fn add_exact_accumulation_jacobian(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    derived: &[FimCellDerived],
    tri: &mut TriMatI<f64, usize>,
) {
    for cell_idx in 0..state.cells.len() {
        let block = cell_accumulation_jacobian_block(sim, previous_state, state, cell_idx, &derived[cell_idx]);
        for (local_eq, row_values) in block.into_iter().enumerate() {
            for (local_var, value) in row_values.into_iter().enumerate() {
                if value.abs() > 1e-14 {
                    tri.add_triplet(
                        equation_offset(cell_idx, local_eq),
                        unknown_offset(cell_idx, local_var),
                        value,
                    );
                }
            }
        }
    }
}

fn local_cell_step(cell: &crate::fim::state::FimCellState, local_var: usize) -> f64 {
    match local_var {
        0 => 1e-5 * cell.pressure_bar.abs().max(1.0),
        1 => 1e-7,
        2 => 1e-7 * cell.hydrocarbon_var.abs().max(1.0),
        _ => unreachable!(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LocalFluxCellSensitivity {
    mobilities: [f64; 3],
    mobility_derivatives: [[f64; 3]; 3],
    bo: f64,
    bg: f64,
    rs: f64,
    bo_derivatives: [f64; 3],
    bg_derivatives: [f64; 3],
    rs_derivatives: [f64; 3],
    rho_o_derivatives: [f64; 3],
    rho_g_derivatives: [f64; 3],
    pcw_derivatives: [f64; 3],
    pcog_derivatives: [f64; 3],
}

fn local_flux_cell_sensitivity(
    sim: &ReservoirSimulator,
    state: &FimState,
    cell_idx: usize,
    derived: &FimCellDerived,
) -> LocalFluxCellSensitivity {
    let cell = state.cell(cell_idx);
    let saturated = cell.regime == HydrocarbonState::Saturated;

    let krw = if sim.three_phase_mode {
        sim.scal_3p
            .as_ref()
            .map(|rock| rock.k_rw(cell.sw))
            .unwrap_or_else(|| sim.scal.k_rw(cell.sw))
    } else {
        sim.scal.k_rw(cell.sw)
    };
    let dkrw_dsw = if sim.three_phase_mode {
        sim.scal_3p
            .as_ref()
            .map(|rock| rock.d_k_rw_d_sw(cell.sw))
            .unwrap_or_else(|| sim.scal.d_k_rw_d_sw(cell.sw))
    } else {
        sim.scal.d_k_rw_d_sw(cell.sw)
    };

    let (kro, dkro_dsw, dkro_dsg, krg, dkrg_dsg) = if sim.three_phase_mode {
        sim.scal_3p
            .as_ref()
            .map(|rock| {
                (
                    rock.k_ro_stone2(cell.sw, derived.sg),
                    rock.d_k_ro_stone2_d_sw(cell.sw, derived.sg),
                    rock.d_k_ro_stone2_d_sg(cell.sw, derived.sg),
                    rock.k_rg(derived.sg),
                    rock.d_k_rg_d_sg(derived.sg),
                )
            })
            .unwrap_or_else(|| {
                (
                    sim.scal.k_ro(cell.sw),
                    sim.scal.d_k_ro_d_sw(cell.sw),
                    0.0,
                    0.0,
                    0.0,
                )
            })
    } else {
        (
            sim.scal.k_ro(cell.sw),
            sim.scal.d_k_ro_d_sw(cell.sw),
            0.0,
            0.0,
            0.0,
        )
    };

    let mu_w = sim.get_mu_w(cell.pressure_bar).max(1e-9);
    let mu_o = sim.get_mu_o_for_rs(cell.pressure_bar, derived.rs).max(1e-9);
    let mu_g = sim.get_mu_g(cell.pressure_bar).max(1e-9);
    let dmu_o_dp = sim.get_d_mu_o_d_p_for_state(cell.pressure_bar, derived.rs, saturated);
    let dmu_o_drs = if saturated {
        0.0
    } else {
        sim.get_d_mu_o_d_rs_for_state(cell.pressure_bar, derived.rs)
    };
    let dmu_g_dp = sim.get_d_mu_g_d_p_for_state(cell.pressure_bar);
    let dsg_dh = if saturated { 1.0 } else { 0.0 };

    let bo_derivatives = if saturated {
        [sim.get_d_bo_d_p_for_state(cell.pressure_bar, derived.rs, true), 0.0, 0.0]
    } else {
        [
            sim.get_d_bo_d_p_for_state(cell.pressure_bar, derived.rs, false),
            0.0,
            sim.get_d_bo_d_rs_for_state(cell.pressure_bar, derived.rs),
        ]
    };
    let bg_derivatives = [sim.get_d_bg_d_p_for_state(cell.pressure_bar), 0.0, 0.0];
    let rs_derivatives = if saturated {
        [sim.get_d_rs_sat_d_p_for_state(cell.pressure_bar), 0.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };

    let lambda_w = krw / mu_w;
    let lambda_o = kro / mu_o;
    let lambda_g = krg / mu_g;

    let dlam_w = [0.0, dkrw_dsw / mu_w, 0.0];
    let dlam_o = [
        -kro * dmu_o_dp / (mu_o * mu_o),
        dkro_dsw / mu_o,
        dkro_dsg * dsg_dh / mu_o - kro * dmu_o_drs / (mu_o * mu_o),
    ];
    let dlam_g = [
        -krg * dmu_g_dp / (mu_g * mu_g),
        0.0,
        dkrg_dsg * dsg_dh / mu_g,
    ];

    let rho_o_derivatives = [
        sim.get_d_rho_o_d_p_for_state(cell.pressure_bar, derived.rs, saturated),
        0.0,
        if saturated {
            0.0
        } else {
            sim.get_d_rho_o_d_rs_for_state(cell.pressure_bar, derived.rs)
        },
    ];
    let rho_g_derivatives = [sim.get_d_rho_g_d_p_for_state(cell.pressure_bar), 0.0, 0.0];
    let pcw_derivatives = [0.0, sim.get_d_capillary_pressure_d_sw(cell.sw), 0.0];
    let pcog_derivatives = [
        0.0,
        0.0,
        if saturated {
            sim.get_d_gas_oil_capillary_pressure_d_sg(derived.sg)
        } else {
            0.0
        },
    ];

    LocalFluxCellSensitivity {
        mobilities: [lambda_w, lambda_o, lambda_g],
        mobility_derivatives: [dlam_w, dlam_o, dlam_g],
        bo: derived.bo.max(1e-9),
        bg: derived.bg.max(1e-9),
        rs: derived.rs.max(0.0),
        bo_derivatives,
        bg_derivatives,
        rs_derivatives,
        rho_o_derivatives,
        rho_g_derivatives,
        pcw_derivatives,
        pcog_derivatives,
    }
}

fn gravity_half_coefficient(sim: &ReservoirSimulator, depth_i: f64, depth_j: f64) -> f64 {
    if sim.gravity_enabled {
        0.5 * 9.80665 * (depth_i - depth_j) * 1e-5
    } else {
        0.0
    }
}

fn phase_potential_derivatives(
    pressure_sign: f64,
    capillary_sign: f64,
    capillary_derivatives: [f64; 3],
    gravity_half_coeff: f64,
    density_derivatives: [f64; 3],
) -> [f64; 3] {
    let mut derivatives = [0.0; 3];
    derivatives[0] = pressure_sign;
    for local_var in 0..3 {
        derivatives[local_var] += capillary_sign * capillary_derivatives[local_var]
            - gravity_half_coeff * density_derivatives[local_var];
    }
    derivatives
}

fn interface_flux_contribution(
    sim: &ReservoirSimulator,
    state: &FimState,
    dt_days: f64,
    id_i: usize,
    id_j: usize,
    dim: char,
    k_i: usize,
    k_j: usize,
    derived_i: &FimCellDerived,
    derived_j: &FimCellDerived,
) -> Option<[[f64; 3]; 2]> {
    let cell_i = state.cell(id_i);
    let cell_j = state.cell(id_j);

    let p_i = cell_i.pressure_bar;
    let p_j = cell_j.pressure_bar;
    let depth_i = sim.depth_at_k(k_i);
    let depth_j = sim.depth_at_k(k_j);
    let geom_t = DARCY_METRIC_FACTOR * sim.geometric_transmissibility(id_i, id_j, dim);

    if geom_t <= 0.0 {
        return None;
    }

    let pcw_i = sim.get_capillary_pressure(cell_i.sw);
    let pcw_j = sim.get_capillary_pressure(cell_j.sw);
    let pcog_i = sim.get_gas_oil_capillary_pressure(derived_i.sg);
    let pcog_j = sim.get_gas_oil_capillary_pressure(derived_j.sg);

    let grav_w = sim.gravity_head_bar(
        depth_i,
        depth_j,
        sim.interface_density_barrier(derived_i.rho_w, derived_j.rho_w),
    );
    let grav_o = sim.gravity_head_bar(
        depth_i,
        depth_j,
        sim.interface_density_barrier(derived_i.rho_o, derived_j.rho_o),
    );
    let grav_g = sim.gravity_head_bar(
        depth_i,
        depth_j,
        sim.interface_density_barrier(derived_i.rho_g, derived_j.rho_g),
    );

    let dphi_w = (p_i - p_j) - (pcw_i - pcw_j) - grav_w;
    let dphi_o = (p_i - p_j) - grav_o;
    let dphi_g = (p_i - p_j) + (pcog_i - pcog_j) - grav_g;

    let mobilities_i = sim.phase_mobilities_for_state(cell_i.sw, derived_i.sg, p_i, derived_i.rs);
    let mobilities_j = sim.phase_mobilities_for_state(cell_j.sw, derived_j.sg, p_j, derived_j.rs);

    let water_upstream = if dphi_w >= 0.0 {
        (derived_i, mobilities_i)
    } else {
        (derived_j, mobilities_j)
    };
    let oil_upstream = if dphi_o >= 0.0 {
        (derived_i, mobilities_i)
    } else {
        (derived_j, mobilities_j)
    };
    let gas_upstream = if dphi_g >= 0.0 {
        (derived_i, mobilities_i)
    } else {
        (derived_j, mobilities_j)
    };

    let q_w_sc_day = geom_t * water_upstream.1.water * dphi_w / sim.b_w.max(1e-9);
    let q_o_res_day = geom_t * oil_upstream.1.oil * dphi_o;
    let q_o_sc_day = q_o_res_day / oil_upstream.0.bo.max(1e-9);
    let q_g_free_sc_day = geom_t * gas_upstream.1.gas * dphi_g / gas_upstream.0.bg.max(1e-9);
    let q_g_dissolved_sc_day = q_o_sc_day * oil_upstream.0.rs;
    let q_g_sc_day = q_g_free_sc_day + q_g_dissolved_sc_day;

    let flux_sc = [
        q_w_sc_day * dt_days,
        q_o_sc_day * dt_days,
        q_g_sc_day * dt_days,
    ];
    Some([
        [flux_sc[0], flux_sc[1], flux_sc[2]],
        [-flux_sc[0], -flux_sc[1], -flux_sc[2]],
    ])
}

fn add_exact_flux_jacobian(
    sim: &ReservoirSimulator,
    state: &FimState,
    dt_days: f64,
    derived: &[FimCellDerived],
    sensitivities: &[LocalFluxCellSensitivity],
    tri: &mut TriMatI<f64, usize>,
) {
    for k in 0..sim.nz {
        for j in 0..sim.ny {
            for i in 0..sim.nx {
                let id = sim.idx(i, j, k);

                if i + 1 < sim.nx {
                    let id_j = sim.idx(i + 1, j, k);
                    add_exact_interface_flux_jacobian(sim, state, dt_days, id, id_j, 'x', k, k, &derived[id], &derived[id_j], &sensitivities[id], &sensitivities[id_j], tri);
                }
                if j + 1 < sim.ny {
                    let id_j = sim.idx(i, j + 1, k);
                    add_exact_interface_flux_jacobian(sim, state, dt_days, id, id_j, 'y', k, k, &derived[id], &derived[id_j], &sensitivities[id], &sensitivities[id_j], tri);
                }
                if k + 1 < sim.nz {
                    let id_j = sim.idx(i, j, k + 1);
                    add_exact_interface_flux_jacobian(sim, state, dt_days, id, id_j, 'z', k, k + 1, &derived[id], &derived[id_j], &sensitivities[id], &sensitivities[id_j], tri);
                }
            }
        }
    }
}

fn add_exact_interface_flux_jacobian(
    sim: &ReservoirSimulator,
    state: &FimState,
    dt_days: f64,
    id_i: usize,
    id_j: usize,
    dim: char,
    k_i: usize,
    k_j: usize,
    derived_i: &FimCellDerived,
    derived_j: &FimCellDerived,
    local_i: &LocalFluxCellSensitivity,
    local_j: &LocalFluxCellSensitivity,
    tri: &mut TriMatI<f64, usize>,
) {
    let cell_i = state.cell(id_i);
    let cell_j = state.cell(id_j);
    let locals = [local_i, local_j];

    let p_i = cell_i.pressure_bar;
    let p_j = cell_j.pressure_bar;
    let depth_i = sim.depth_at_k(k_i);
    let depth_j = sim.depth_at_k(k_j);
    let geom_t = DARCY_METRIC_FACTOR * sim.geometric_transmissibility(id_i, id_j, dim);
    if geom_t <= 0.0 {
        return;
    }

    let pcw_i = sim.get_capillary_pressure(cell_i.sw);
    let pcw_j = sim.get_capillary_pressure(cell_j.sw);
    let pcog_i = sim.get_gas_oil_capillary_pressure(derived_i.sg);
    let pcog_j = sim.get_gas_oil_capillary_pressure(derived_j.sg);
    let grav_half = gravity_half_coefficient(sim, depth_i, depth_j);
    let grav_o = grav_half * (derived_i.rho_o + derived_j.rho_o);
    let grav_g = grav_half * (derived_i.rho_g + derived_j.rho_g);

    let dphi_w = (p_i - p_j) - (pcw_i - pcw_j);
    let dphi_o = (p_i - p_j) - grav_o;
    let dphi_g = (p_i - p_j) + (pcog_i - pcog_j) - grav_g;

    let water_upwind = if dphi_w >= 0.0 { 0 } else { 1 };
    let oil_upwind = if dphi_o >= 0.0 { 0 } else { 1 };
    let gas_upwind = if dphi_g >= 0.0 { 0 } else { 1 };

    let lambda_w = locals[water_upwind].mobilities[0];
    let lambda_o = locals[oil_upwind].mobilities[1];
    let lambda_g = locals[gas_upwind].mobilities[2];
    let bo_up = locals[oil_upwind].bo;
    let bg_up = locals[gas_upwind].bg;
    let rs_up = locals[oil_upwind].rs;
    let q_o_sc_day = geom_t * lambda_o * dphi_o / bo_up;

    for (side_idx, cell_idx) in [id_i, id_j].into_iter().enumerate() {
        let pressure_sign = if side_idx == 0 { 1.0 } else { -1.0 };
        let dphi_w_derivatives = phase_potential_derivatives(
            pressure_sign,
            if side_idx == 0 { -1.0 } else { 1.0 },
            locals[side_idx].pcw_derivatives,
            0.0,
            [0.0; 3],
        );
        let dphi_o_derivatives = phase_potential_derivatives(
            pressure_sign,
            0.0,
            [0.0; 3],
            grav_half,
            locals[side_idx].rho_o_derivatives,
        );
        let dphi_g_derivatives = phase_potential_derivatives(
            pressure_sign,
            if side_idx == 0 { 1.0 } else { -1.0 },
            locals[side_idx].pcog_derivatives,
            grav_half,
            locals[side_idx].rho_g_derivatives,
        );

        for local_var in 0..3 {
            let mut dq_w_sc_day = geom_t * lambda_w * dphi_w_derivatives[local_var] / sim.b_w.max(1e-9);
            if side_idx == water_upwind {
                dq_w_sc_day += geom_t
                    * locals[side_idx].mobility_derivatives[0][local_var]
                    * dphi_w
                    / sim.b_w.max(1e-9);
            }

            let mut dq_o_sc_day = geom_t * lambda_o * dphi_o_derivatives[local_var] / bo_up;
            if side_idx == oil_upwind {
                dq_o_sc_day += geom_t
                    * locals[side_idx].mobility_derivatives[1][local_var]
                    * dphi_o
                    / bo_up;
                dq_o_sc_day -= geom_t
                    * lambda_o
                    * dphi_o
                    * locals[side_idx].bo_derivatives[local_var]
                    / (bo_up * bo_up);
            }

            let mut dq_g_free_sc_day = geom_t * lambda_g * dphi_g_derivatives[local_var] / bg_up;
            if side_idx == gas_upwind {
                dq_g_free_sc_day += geom_t
                    * locals[side_idx].mobility_derivatives[2][local_var]
                    * dphi_g
                    / bg_up;
                dq_g_free_sc_day -= geom_t
                    * lambda_g
                    * dphi_g
                    * locals[side_idx].bg_derivatives[local_var]
                    / (bg_up * bg_up);
            }

            let mut dq_g_sc_day = dq_g_free_sc_day + rs_up * dq_o_sc_day;
            if side_idx == oil_upwind {
                dq_g_sc_day += q_o_sc_day * locals[side_idx].rs_derivatives[local_var];
            }

            let derivatives = [dq_w_sc_day * dt_days, dq_o_sc_day * dt_days, dq_g_sc_day * dt_days];
            let column = unknown_offset(cell_idx, local_var);
            for eq_side in 0..2 {
                let row_cell = if eq_side == 0 { id_i } else { id_j };
                let sign = if eq_side == 0 { 1.0 } else { -1.0 };
                for component in 0..3 {
                    let value = sign * derivatives[component];
                    if value.abs() > 1e-14 {
                        tri.add_triplet(equation_offset(row_cell, component), column, value);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
fn add_local_flux_jacobian_fd(
    sim: &ReservoirSimulator,
    state: &FimState,
    dt_days: f64,
    tri: &mut TriMatI<f64, usize>,
) {
    for k in 0..sim.nz {
        for j in 0..sim.ny {
            for i in 0..sim.nx {
                let id = sim.idx(i, j, k);

                if i + 1 < sim.nx {
                    add_interface_flux_jacobian_fd(sim, state, dt_days, id, sim.idx(i + 1, j, k), 'x', k, k, tri);
                }
                if j + 1 < sim.ny {
                    add_interface_flux_jacobian_fd(sim, state, dt_days, id, sim.idx(i, j + 1, k), 'y', k, k, tri);
                }
                if k + 1 < sim.nz {
                    add_interface_flux_jacobian_fd(sim, state, dt_days, id, sim.idx(i, j, k + 1), 'z', k, k + 1, tri);
                }
            }
        }
    }
}

#[cfg(test)]
fn add_interface_flux_jacobian_fd(
    sim: &ReservoirSimulator,
    state: &FimState,
    dt_days: f64,
    id_i: usize,
    id_j: usize,
    dim: char,
    k_i: usize,
    k_j: usize,
    tri: &mut TriMatI<f64, usize>,
) {
    let base_derived_i = state.derive_cell(sim, id_i);
    let base_derived_j = state.derive_cell(sim, id_j);
    let Some(base_flux) = interface_flux_contribution(sim, state, dt_days, id_i, id_j, dim, k_i, k_j, &base_derived_i, &base_derived_j) else {
        return;
    };

    for cell_idx in [id_i, id_j] {
        for local_var in 0..3 {
            let perturbation = local_cell_step(state.cell(cell_idx), local_var);
            let mut perturbed_state = state.clone();
            let perturbed_cell = perturbed_state.cell_mut(cell_idx);
            match local_var {
                0 => perturbed_cell.pressure_bar += perturbation,
                1 => perturbed_cell.sw += perturbation,
                2 => perturbed_cell.hydrocarbon_var += perturbation,
                _ => unreachable!(),
            }

            let perturbed_derived_i = perturbed_state.derive_cell(sim, id_i);
            let perturbed_derived_j = perturbed_state.derive_cell(sim, id_j);
            let Some(perturbed_flux) = interface_flux_contribution(
                sim,
                &perturbed_state,
                dt_days,
                id_i,
                id_j,
                dim,
                k_i,
                k_j,
                &perturbed_derived_i,
                &perturbed_derived_j,
            ) else {
                continue;
            };

            let column = unknown_offset(cell_idx, local_var);
            for eq_side in 0..2 {
                let row_cell = if eq_side == 0 { id_i } else { id_j };
                for component in 0..3 {
                    let derivative =
                        (perturbed_flux[eq_side][component] - base_flux[eq_side][component])
                            / perturbation;
                    if derivative.abs() > 1e-14 {
                        tri.add_triplet(equation_offset(row_cell, component), column, derivative);
                    }
                }
            }
        }
    }
}

fn add_well_source_terms(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    dt_days: f64,
    residual: &mut DVector<f64>,
) {
    for (perf_idx, perforation) in topology.perforations.iter().enumerate() {
        let id = perforation.cell_index;
        let components_sc_day = perforation_component_rates_sc_day(sim, state, topology, perf_idx);
        for (local_eq, component_rate) in components_sc_day.into_iter().enumerate() {
            residual[equation_offset(id, local_eq)] += component_rate * dt_days;
        }
    }
}

fn add_well_constraint_equations(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    residual: &mut DVector<f64>,
) {
    for well_idx in 0..topology.wells.len() {
        if let Some(well_residual) = well_constraint_residual(sim, state, topology, well_idx) {
            residual[state.well_equation_offset(well_idx)] += well_residual;
        }
    }
}

fn add_perforation_equations(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    residual: &mut DVector<f64>,
) {
    for perf_idx in 0..topology.perforations.len() {
        if let Some(rate_residual) = perforation_rate_residual(sim, state, topology, perf_idx) {
            residual[state.perforation_equation_offset(perf_idx)] += rate_residual;
        }
    }
}

fn add_interface_flux(
    sim: &ReservoirSimulator,
    state: &FimState,
    dt_days: f64,
    id_i: usize,
    id_j: usize,
    dim: char,
    k_i: usize,
    k_j: usize,
    derived_i: &FimCellDerived,
    derived_j: &FimCellDerived,
    residual: &mut DVector<f64>,
) {
    let Some(flux) = interface_flux_contribution(sim, state, dt_days, id_i, id_j, dim, k_i, k_j, derived_i, derived_j) else {
        return;
    };

    for component in 0..3 {
        residual[equation_offset(id_i, component)] += flux[0][component];
        residual[equation_offset(id_j, component)] += flux[1][component];
    }
}

#[cfg(test)]
mod tests {
    use crate::fim::state::{FimCellState, FimState, HydrocarbonState};
    use crate::pvt::{PvtRow, PvtTable};
    use crate::ReservoirSimulator;

    use super::*;

    fn jacobian_value(matrix: &CsMat<f64>, row: usize, col: usize) -> f64 {
        matrix
            .outer_view(row)
            .and_then(|view| view.iter().find(|(index, _)| *index == col).map(|(_, value)| *value))
            .unwrap_or(0.0)
    }

    fn build_rate_controlled_waterflood_fd_fixture(
    ) -> (ReservoirSimulator, FimState, FimState, FimAssemblyOptions<'static>) {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_injected_fluid("water").unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow { p_bar: 100.0, rs_m3m3: 10.0, bo_m3m3: 1.2, mu_o_cp: 1.4, bg_m3m3: 0.02, mu_g_cp: 0.03 },
                PvtRow { p_bar: 200.0, rs_m3m3: 20.0, bo_m3m3: 1.1, mu_o_cp: 1.2, bg_m3m3: 0.01, mu_g_cp: 0.025 },
                PvtRow { p_bar: 400.0, rs_m3m3: 40.0, bo_m3m3: 1.0, mu_o_cp: 1.0, bg_m3m3: 0.005, mu_g_cp: 0.02 },
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

        let topology = build_well_topology(&sim);
        let owned_topology = Box::new(topology);
        let topology_ref: &'static FimWellTopology = Box::leak(owned_topology);
        let options = FimAssemblyOptions {
            dt_days: 1.0,
            include_wells: true,
            topology: Some(topology_ref),
        };

        (sim, previous_state, state, options)
    }

    fn assert_full_system_fd_matches_for_columns(columns: &[usize]) {
        let (sim, previous_state, state, options) = build_rate_controlled_waterflood_fd_fixture();
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
                let step = match local_var {
                    0 => 1e-4 * state.cells[cell_idx].pressure_bar.abs().max(1.0),
                    1 => 1e-6,
                    2 => 1e-6 * state.cells[cell_idx].hydrocarbon_var.abs().max(1.0),
                    _ => unreachable!(),
                };
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
                    row, col, exact_value, fd_value, error
                );
            }
        }

        eprintln!(
            "sampled full_system_jacobian FD check: max_rel_error={:.3e} at ({}, {}): exact={:.6e} fd={:.6e}",
            max_error, worst_entry.0, worst_entry.1, worst_entry.2, worst_entry.3
        );
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
                topology: None,
            },
        );

        assert!(assembly.residual.norm() <= 1e-12);
    }

    #[test]
    fn intercell_flux_is_component_conservative() {
        let sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        let previous_state = FimState::from_simulator(&sim);
        let mut state = previous_state.clone();
        state.cells[0].pressure_bar = 250.0;
        state.cells[1].pressure_bar = 150.0;

        let assembly = assemble_fim_system(
            &sim,
            &previous_state,
            &state,
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: false,
                topology: None,
            },
        );

        let water_sum =
            assembly.residual[equation_offset(0, 0)] + assembly.residual[equation_offset(1, 0)];
        let oil_sum =
            assembly.residual[equation_offset(0, 1)] + assembly.residual[equation_offset(1, 1)];
        let gas_sum =
            assembly.residual[equation_offset(0, 2)] + assembly.residual[equation_offset(1, 2)];

        assert!(water_sum.abs() < 1e-9);
        assert!(oil_sum.abs() < 1e-9);
        assert!(gas_sum.abs() < 1e-9);
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
        let derived: Vec<FimCellDerived> = (0..n_cells).map(|idx| state.derive_cell(&sim, idx)).collect();
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
                topology: None,
            },
        );

        assert!(baseline.residual.norm() <= 1e-12);
        assert!(
            assembly.residual[equation_offset(0, 0)] < baseline.residual[equation_offset(0, 0)]
        );
        assert!(
            (assembly.residual[equation_offset(0, 1)] - baseline.residual[equation_offset(0, 1)])
                .abs()
                < 1e-12
        );
        assert!(
            (assembly.residual[equation_offset(0, 2)] - baseline.residual[equation_offset(0, 2)])
                .abs()
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
                topology: None,
            },
        );

        let q_column = state.perforation_rate_unknown_offset(0);
        let perf_row = state.perforation_equation_offset(0);
        let water_row = equation_offset(0, 0);

        assert!((jacobian_value(&assembly.jacobian, perf_row, q_column) - 1.0).abs() < 1e-12);
        assert!(
            (jacobian_value(&assembly.jacobian, water_row, q_column) - 2.0 / sim.b_w.max(1e-9))
                .abs()
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
                topology: None,
            },
        );

        let row_cell = sim.idx(1, 1, 0);
        for local_var in 0..3 {
            let col = unknown_offset(row_cell, local_var);
            for component in 0..3 {
                let row = equation_offset(row_cell, component);
                let value =
                    jacobian_value(&assembly.jacobian, row, col) - jacobian_value(&baseline.jacobian, row, col);
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
        let (bhp_slack, rate_slack) = crate::fim::wells::well_control_slacks(&sim, &state, &topology, 0).unwrap();
        let bhp_scale = control.bhp_limit.abs().max(1.0);
        let rate_scale = control.target_rate.unwrap_or(1.0).abs().max(1.0);
        let (_, dphi_db) = crate::fim::wells::fischer_burmeister_gradient(bhp_slack / bhp_scale, rate_slack / rate_scale);
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
                topology: None,
            },
        );

        let row = state.well_equation_offset(0);
        for (local_var, derivative) in d_surface.into_iter().enumerate() {
            let value = jacobian_value(&assembly.jacobian, row, unknown_offset(sim.idx(1, 1, 0), local_var));
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
            &sim,
            &state,
            &topology,
            0,
        )[2];

        let assembly = assemble_fim_system(
            &sim,
            &state,
            &state,
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: true,
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

        assert!((block[0][0] - pv * sim.rock_compressibility * state.cells[0].sw / sim.b_w).abs() < 1e-12);
        assert!((block[0][1] - pv / sim.b_w).abs() < 1e-12);
        assert!(block[0][2].abs() < 1e-12);
    }

    /// Full-system finite-difference Jacobian verification.
    /// Perturbs every unknown (cell states, well BHP, perforation rates) and
    /// compares the numerical derivative of the full residual against the
    /// analytical Jacobian. This catches any sign error or missing coupling.
    #[test]
    #[ignore = "Expensive test that can be enabled for debugging specific Jacobian issues"]
    fn full_system_jacobian_matches_fd_for_rate_controlled_waterflood() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_injected_fluid("water").unwrap();
        sim.set_three_phase_mode_enabled(true);
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow { p_bar: 100.0, rs_m3m3: 10.0, bo_m3m3: 1.2, mu_o_cp: 1.4, bg_m3m3: 0.02, mu_g_cp: 0.03 },
                PvtRow { p_bar: 200.0, rs_m3m3: 20.0, bo_m3m3: 1.1, mu_o_cp: 1.2, bg_m3m3: 0.01, mu_g_cp: 0.025 },
                PvtRow { p_bar: 400.0, rs_m3m3: 40.0, bo_m3m3: 1.0, mu_o_cp: 1.0, bg_m3m3: 0.005, mu_g_cp: 0.02 },
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
        // Perturb from equilibrium so residual is nonzero.
        // Keep saturations **away from boundaries** so FD central differences
        // don't hit the clamp at Sg=0 or So=S_or.
        for cell in state.cells.iter_mut() {
            cell.hydrocarbon_var = 0.05; // small non-zero Sg
        }
        state.cells[0].pressure_bar = 310.0;
        state.cells[1].pressure_bar = 280.0;
        state.cells[0].sw = 0.35;
        state.perforation_rates_m3_day[0] = -5.0;
        state.perforation_rates_m3_day[1] = 8.0;

        let dt_days = 1.0;
        let topology = build_well_topology(&sim);
        let options = FimAssemblyOptions {
            dt_days,
            include_wells: true,
            topology: Some(&topology),
        };

        let assembly = assemble_fim_system(&sim, &previous_state, &state, &options);
        let _n = assembly.residual.len();
        assert_full_system_fd_matches_for_columns(&(0..10).collect::<Vec<_>>());
    }

    #[test]
    #[ignore = "diagnostic: sampled full-system central-FD Jacobian"]
    fn representative_full_system_jacobian_columns_match_fd_for_rate_controlled_waterflood() {
        assert_full_system_fd_matches_for_columns(&[0, 1, 2, 6, 8, 9]);
    }

}
