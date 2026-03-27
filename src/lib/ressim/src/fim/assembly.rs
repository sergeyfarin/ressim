use nalgebra::DVector;
use sprs::{CsMat, TriMatI};

use crate::fim::scaling::{
    build_equation_scaling, build_variable_scaling, EquationScaling, VariableScaling,
};
use crate::fim::state::FimState;
use crate::fim::wells::{
    build_well_topology, fischer_burmeister_gradient, perforation_component_rate_derivatives_sc_day,
    perforation_component_rates_sc_day, perforation_connection_bhp_derivative,
    perforation_rate_residual, perforation_target_rate_derivative, physical_well_control,
    well_constraint_residual, well_control_slacks, FimWellTopology,
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
pub(crate) struct FimAssemblyOptions {
    pub(crate) dt_days: f64,
    pub(crate) include_wells: bool,
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
    let topology = build_well_topology(sim);
    let n_unknowns = state.n_unknowns();
    let equation_scaling = build_equation_scaling(sim, state, options.dt_days);
    let variable_scaling = build_variable_scaling(sim, state);
    let mut tri = TriMatI::<f64, usize>::new((n_unknowns, n_unknowns));
    let residual = assemble_residual(sim, previous_state, state, &topology, options);

    add_exact_accumulation_jacobian(sim, previous_state, state, &mut tri);
    add_local_flux_jacobian_fd(sim, state, options.dt_days, &mut tri);

    if options.include_wells {
        add_exact_well_source_jacobian(sim, state, &topology, options.dt_days, &mut tri);
        add_exact_well_constraint_jacobian(sim, state, &topology, &mut tri);
        add_exact_perforation_jacobian(sim, state, &topology, &mut tri);
        add_local_well_source_cell_jacobian_fd(sim, state, &topology, options.dt_days, &mut tri);
        add_local_well_constraint_cell_jacobian_fd(sim, state, &topology, &mut tri);
        add_local_perforation_cell_jacobian_fd(sim, state, &topology, &mut tri);
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
        let (dphi_da, dphi_db) = fischer_burmeister_gradient(bhp_slack, rate_slack);
        let dslack_dbhp = if topology.wells[well_idx].injector { -1.0 } else { 1.0 };
        let bhp_value = dphi_da * dslack_dbhp;
        if bhp_value.abs() > 1e-14 {
            tri.add_triplet(row, column_bhp, bhp_value);
        }

        for &perf_idx in &topology.wells[well_idx].perforation_indices {
            let column = state.perforation_rate_unknown_offset(perf_idx);
            let dactual_dq = perforation_target_rate_derivative(sim, state, topology, perf_idx);
            let value = -dphi_db * dactual_dq;
            if value.abs() > 1e-14 {
                tri.add_triplet(row, column, value);
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

fn well_constraint_influence_cells(
    sim: &ReservoirSimulator,
    topology: &FimWellTopology,
    well_idx: usize,
) -> Vec<usize> {
    let mut cells = Vec::new();
    for &perf_idx in &topology.wells[well_idx].perforation_indices {
        cells.extend(perforation_control_influence_cells(sim, topology, perf_idx));
    }
    cells.sort_unstable();
    cells.dedup();
    cells
}

fn add_local_well_source_cell_jacobian_fd(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    dt_days: f64,
    tri: &mut TriMatI<f64, usize>,
) {
    for perf_idx in 0..topology.perforations.len() {
        let base = perforation_component_rates_sc_day(sim, state, topology, perf_idx);
        let row_cell = topology.perforations[perf_idx].cell_index;

        for cell_idx in perforation_control_influence_cells(sim, topology, perf_idx) {
            for local_var in 0..3 {
                let (perturbation, perturbed_state) = perturb_cell_unknown(sim, state, cell_idx, local_var);
                let perturbed =
                    perforation_component_rates_sc_day(sim, &perturbed_state, topology, perf_idx);
                let column = unknown_offset(cell_idx, local_var);
                for component in 0..3 {
                    let derivative = (perturbed[component] - base[component]) * dt_days / perturbation;
                    if derivative.abs() > 1e-14 {
                        tri.add_triplet(equation_offset(row_cell, component), column, derivative);
                    }
                }
            }
        }
    }
}

fn add_local_well_constraint_cell_jacobian_fd(
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

        let Some(base) = well_constraint_residual(sim, state, topology, well_idx) else {
            continue;
        };
        let row = state.well_equation_offset(well_idx);
        for cell_idx in well_constraint_influence_cells(sim, topology, well_idx) {
            for local_var in 0..3 {
                let (perturbation, perturbed_state) = perturb_cell_unknown(sim, state, cell_idx, local_var);
                let Some(perturbed) = well_constraint_residual(sim, &perturbed_state, topology, well_idx) else {
                    continue;
                };
                let derivative = (perturbed - base) / perturbation;
                if derivative.abs() > 1e-14 {
                    tri.add_triplet(row, unknown_offset(cell_idx, local_var), derivative);
                }
            }
        }
    }
}

fn add_local_perforation_cell_jacobian_fd(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    tri: &mut TriMatI<f64, usize>,
) {
    for perf_idx in 0..topology.perforations.len() {
        let Some(base) = perforation_rate_residual(sim, state, topology, perf_idx) else {
            continue;
        };

        let row = state.perforation_equation_offset(perf_idx);
        let cell_idx = topology.perforations[perf_idx].cell_index;
        for local_var in 0..3 {
            let (perturbation, perturbed_state) = perturb_cell_unknown(sim, state, cell_idx, local_var);
            let Some(perturbed) = perforation_rate_residual(sim, &perturbed_state, topology, perf_idx) else {
                continue;
            };
            let derivative = (perturbed - base) / perturbation;
            if derivative.abs() > 1e-14 {
                tri.add_triplet(row, unknown_offset(cell_idx, local_var), derivative);
            }
        }
    }
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
) -> [f64; 3] {
    let pore_volume_m3 = pore_volume_at_state(sim, previous_state, state, cell_idx).max(1e-9);
    let cell = state.cell(cell_idx);
    let derived = state.derive_cell(sim, cell_idx);

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
) -> [f64; 3] {
    let current = cell_component_inventory_sc(sim, previous_state, state, cell_idx);
    let previous = cell_component_inventory_sc(sim, previous_state, previous_state, cell_idx);
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
) -> [[f64; 3]; 3] {
    let pore_volume_m3 = pore_volume_at_state(sim, previous_state, state, cell_idx).max(1e-9);
    let d_pore_volume_d_p = pore_volume_m3 * sim.rock_compressibility;
    let cell = state.cell(cell_idx);
    let flash = crate::fim::flash::resolve_cell_flash(
        sim,
        cell.pressure_bar,
        cell.sw,
        cell.hydrocarbon_var,
        cell.regime,
    );
    let bw = sim.b_w.max(1e-9);
    let bo = sim.get_b_o_for_rs(cell.pressure_bar, flash.rs).max(1e-9);
    let bg = sim.get_b_g(cell.pressure_bar).max(1e-9);

    let saturated = flash.regime == crate::fim::state::HydrocarbonState::Saturated;
    let d_bo_d_p = sim.get_d_bo_d_p_for_state(cell.pressure_bar, flash.rs, saturated);
    let d_bo_d_rs = if saturated {
        0.0
    } else {
        sim.get_d_bo_d_rs_for_state(cell.pressure_bar, flash.rs)
    };
    let d_bg_d_p = sim.get_d_bg_d_p_for_state(cell.pressure_bar);
    let d_rs_sat_d_p = if saturated {
        sim.get_d_rs_sat_d_p_for_state(cell.pressure_bar)
    } else {
        0.0
    };

    let (d_so_d_sw, d_so_d_h, d_sg_d_h, d_rs_d_h) = match flash.regime {
        crate::fim::state::HydrocarbonState::Saturated => (-1.0, -1.0, 1.0, 0.0),
        crate::fim::state::HydrocarbonState::Undersaturated => (-1.0, 0.0, 0.0, 1.0),
    };

    let oil_inventory = pore_volume_m3 * flash.so / bo;
    let d_water_d_p = d_pore_volume_d_p * cell.sw / bw;
    let d_water_d_sw = pore_volume_m3 / bw;

    let d_oil_d_p = d_pore_volume_d_p * flash.so / bo
        - pore_volume_m3 * flash.so * d_bo_d_p / (bo * bo);
    let d_oil_d_sw = pore_volume_m3 * d_so_d_sw / bo;
    let d_oil_d_h = pore_volume_m3 * d_so_d_h / bo
        - pore_volume_m3 * flash.so * d_bo_d_rs * d_rs_d_h / (bo * bo);

    let d_free_gas_d_p = d_pore_volume_d_p * flash.sg / bg
        - pore_volume_m3 * flash.sg * d_bg_d_p / (bg * bg);
    let d_free_gas_d_h = pore_volume_m3 * d_sg_d_h / bg;

    let d_gas_d_p = d_free_gas_d_p + d_oil_d_p * flash.rs + oil_inventory * d_rs_sat_d_p;
    let d_gas_d_sw = d_oil_d_sw * flash.rs;
    let d_gas_d_h = d_free_gas_d_h + d_oil_d_h * flash.rs + oil_inventory * d_rs_d_h;

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
) -> DVector<f64> {
    assemble_residual_with_flags(sim, previous_state, state, topology, options, true, true)
}

fn assemble_residual_with_flags(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    topology: &FimWellTopology,
    options: &FimAssemblyOptions,
    include_accumulation: bool,
    include_flux: bool,
) -> DVector<f64> {
    let n_unknowns = state.n_unknowns();
    let mut residual = DVector::zeros(n_unknowns);

    if include_accumulation {
        for cell_idx in 0..state.cells.len() {
            let accumulation = cell_accumulation_residual(sim, previous_state, state, cell_idx);
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
                        add_interface_flux(
                            sim,
                            state,
                            options.dt_days,
                            id,
                            sim.idx(i + 1, j, k),
                            'x',
                            k,
                            k,
                            &mut residual,
                        );
                    }
                    if j + 1 < sim.ny {
                        add_interface_flux(
                            sim,
                            state,
                            options.dt_days,
                            id,
                            sim.idx(i, j + 1, k),
                            'y',
                            k,
                            k,
                            &mut residual,
                        );
                    }
                    if k + 1 < sim.nz {
                        add_interface_flux(
                            sim,
                            state,
                            options.dt_days,
                            id,
                            sim.idx(i, j, k + 1),
                            'z',
                            k,
                            k + 1,
                            &mut residual,
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
    tri: &mut TriMatI<f64, usize>,
) {
    for cell_idx in 0..state.cells.len() {
        let block = cell_accumulation_jacobian_block(sim, previous_state, state, cell_idx);
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

fn interface_flux_contribution(
    sim: &ReservoirSimulator,
    state: &FimState,
    dt_days: f64,
    id_i: usize,
    id_j: usize,
    dim: char,
    k_i: usize,
    k_j: usize,
) -> Option<[[f64; 3]; 2]> {
    let cell_i = state.cell(id_i);
    let cell_j = state.cell(id_j);
    let derived_i = state.derive_cell(sim, id_i);
    let derived_j = state.derive_cell(sim, id_j);

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
    let Some(base_flux) = interface_flux_contribution(sim, state, dt_days, id_i, id_j, dim, k_i, k_j) else {
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

            let Some(perturbed_flux) = interface_flux_contribution(
                sim,
                &perturbed_state,
                dt_days,
                id_i,
                id_j,
                dim,
                k_i,
                k_j,
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
    residual: &mut DVector<f64>,
) {
    let Some(flux) = interface_flux_contribution(sim, state, dt_days, id_i, id_j, dim, k_i, k_j) else {
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
    use crate::ReservoirSimulator;

    use super::*;

    fn jacobian_value(matrix: &CsMat<f64>, row: usize, col: usize) -> f64 {
        matrix
            .outer_view(row)
            .and_then(|view| view.iter().find(|(index, _)| *index == col).map(|(_, value)| *value))
            .unwrap_or(0.0)
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
            },
        );
        let assembly = assemble_fim_system(
            &sim,
            &state,
            &state,
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: true,
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
            },
        );
        let high_sw_assembly = assemble_fim_system(
            &sim,
            &high_sw_state,
            &high_sw_state,
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: true,
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
        let block = cell_accumulation_jacobian_block(&sim, &previous_state, &state, 0);
        let pv = sim.pore_volume_m3(0).max(1e-9);

        assert!((block[0][0] - pv * sim.rock_compressibility * state.cells[0].sw / sim.b_w).abs() < 1e-12);
        assert!((block[0][1] - pv / sim.b_w).abs() < 1e-12);
        assert!(block[0][2].abs() < 1e-12);
    }
}
