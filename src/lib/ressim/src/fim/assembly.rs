use nalgebra::DVector;
use sprs::{CsMat, TriMatI};

use crate::ReservoirSimulator;
use crate::fim::scaling::{
    EquationScaling, VariableScaling, build_equation_scaling, build_variable_scaling,
};
use crate::fim::state::{FimCellDerived, FimState, HydrocarbonState};
use crate::fim::wells::{
    FimWellTopology, build_well_topology, fischer_burmeister_gradient,
    perforation_component_rate_cell_derivatives_sc_day_by_var,
    perforation_component_rate_derivatives_sc_day, perforation_component_rates_sc_day,
    perforation_connection_bhp_derivative, perforation_connection_cell_derivatives,
    perforation_rate_residual, perforation_surface_rate_cell_derivatives_sc_day,
    perforation_target_rate_derivative, physical_well_control, well_constraint_residual,
    well_control_slacks,
};

const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;

pub(crate) struct FimAssembly {
    pub(crate) residual: DVector<f64>,
    pub(crate) jacobian: CsMat<f64>,
    pub(crate) equation_scaling: EquationScaling,
    pub(crate) variable_scaling: VariableScaling,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CellResidualBreakdown {
    pub(crate) accumulation: f64,
    pub(crate) x_minus: f64,
    pub(crate) x_plus: f64,
    pub(crate) y_minus: f64,
    pub(crate) y_plus: f64,
    pub(crate) z_minus: f64,
    pub(crate) z_plus: f64,
    pub(crate) well_source: f64,
    pub(crate) total: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PhaseFluxDiagnostic {
    pub(crate) dphi: f64,
    pub(crate) upwind_cell_idx: usize,
    pub(crate) mobility: f64,
    pub(crate) flux: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FacePhaseDiagnostics {
    pub(crate) water: PhaseFluxDiagnostic,
    pub(crate) oil: PhaseFluxDiagnostic,
    pub(crate) gas: PhaseFluxDiagnostic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CellFacePhaseDiagnostics {
    pub(crate) x_minus: Option<FacePhaseDiagnostics>,
    pub(crate) x_plus: Option<FacePhaseDiagnostics>,
    pub(crate) y_minus: Option<FacePhaseDiagnostics>,
    pub(crate) y_plus: Option<FacePhaseDiagnostics>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct InterfaceFluxTerms {
    dphi: [f64; 3],
    upwind_cell_idx: [usize; 3],
    mobility: [f64; 3],
    flux_sc_day: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimAssemblyOptions<'a> {
    pub(crate) dt_days: f64,
    pub(crate) include_wells: bool,
    pub(crate) assemble_residual_only: bool,
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
    let derived: Vec<FimCellDerived> = (0..n_cells)
        .map(|idx| state.derive_cell(sim, idx))
        .collect();
    let prev_derived: Vec<FimCellDerived> = (0..n_cells)
        .map(|idx| previous_state.derive_cell(sim, idx))
        .collect();
    let residual = assemble_residual(
        sim,
        previous_state,
        state,
        &topology,
        options,
        &derived,
        &prev_derived,
    );

    if options.assemble_residual_only {
        return FimAssembly {
            residual,
            jacobian: TriMatI::<f64, usize>::new((n_unknowns, n_unknowns)).to_csr(),
            equation_scaling,
            variable_scaling,
        };
    }

    // Pre-compute local flux sensitivities for the Jacobian (each cell appears in ~5 interfaces).
    let sensitivities: Vec<LocalFluxCellSensitivity> = (0..n_cells)
        .map(|idx| local_flux_cell_sensitivity(sim, state, idx, &derived[idx]))
        .collect();

    let mut tri = TriMatI::<f64, usize>::new((n_unknowns, n_unknowns));

    add_exact_accumulation_jacobian(sim, previous_state, state, &derived, &mut tri);
    add_exact_flux_jacobian(
        sim,
        state,
        options.dt_days,
        &derived,
        &sensitivities,
        &mut tri,
    );

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
        let derivatives =
            perforation_component_rate_derivatives_sc_day(sim, state, topology, perf_idx);
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
            let derivatives = perforation_component_rate_cell_derivatives_sc_day_by_var(
                sim, state, topology, perf_idx, cell_idx,
            );
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

        let Some((bhp_slack, rate_slack)) = well_control_slacks(sim, state, topology, well_idx)
        else {
            continue;
        };
        let bhp_scale = control.bhp_limit.abs().max(1.0);
        let rate_scale = control.target_rate.unwrap_or(1.0).abs().max(1.0);
        let (dphi_da, dphi_db) =
            fischer_burmeister_gradient(bhp_slack / bhp_scale, rate_slack / rate_scale);
        let dslack_dbhp = if topology.wells[well_idx].injector {
            -1.0
        } else {
            1.0
        };
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

        let Some((bhp_slack, rate_slack)) = well_control_slacks(sim, state, topology, well_idx)
        else {
            continue;
        };
        let bhp_scale = control.bhp_limit.abs().max(1.0);
        let rate_scale = control.target_rate.unwrap_or(1.0).abs().max(1.0);
        let (_, dphi_db) =
            fischer_burmeister_gradient(bhp_slack / bhp_scale, rate_slack / rate_scale);
        let row = state.well_equation_offset(well_idx);
        for &perf_idx in &topology.wells[well_idx].perforation_indices {
            for cell_idx in perforation_control_influence_cells(sim, topology, perf_idx) {
                let derivatives = perforation_surface_rate_cell_derivatives_sc_day(
                    sim, state, topology, perf_idx, cell_idx,
                );
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
    let previous =
        cell_component_inventory_sc(sim, previous_state, previous_state, cell_idx, prev_derived);
    [
        current[0] - previous[0],
        current[1] - previous[1],
        current[2] - previous[2],
    ]
}

pub(crate) fn cell_equation_residual_breakdown(
    sim: &ReservoirSimulator,
    previous_state: &FimState,
    state: &FimState,
    topology: &FimWellTopology,
    dt_days: f64,
    cell_idx: usize,
    component: usize,
) -> Option<CellResidualBreakdown> {
    if cell_idx >= state.cells.len() || component >= 3 {
        return None;
    }

    let derived_cell = state.derive_cell(sim, cell_idx);
    let prev_derived_cell = previous_state.derive_cell(sim, cell_idx);
    let accumulation = cell_accumulation_residual(
        sim,
        previous_state,
        state,
        cell_idx,
        &derived_cell,
        &prev_derived_cell,
    )[component];

    let cells_per_layer = sim.nx * sim.ny;
    let k = cell_idx / cells_per_layer;
    let in_layer = cell_idx % cells_per_layer;
    let j = in_layer / sim.nx;
    let i = in_layer % sim.nx;

    let mut breakdown = CellResidualBreakdown {
        accumulation,
        x_minus: 0.0,
        x_plus: 0.0,
        y_minus: 0.0,
        y_plus: 0.0,
        z_minus: 0.0,
        z_plus: 0.0,
        well_source: 0.0,
        total: 0.0,
    };

    if i > 0 {
        let neighbor = sim.idx(i - 1, j, k);
        let derived_neighbor = state.derive_cell(sim, neighbor);
        if let Some(flux) = interface_flux_contribution(
            sim,
            state,
            dt_days,
            neighbor,
            cell_idx,
            'x',
            k,
            k,
            &derived_neighbor,
            &derived_cell,
        ) {
            breakdown.x_minus = flux[1][component];
        }
    }
    if i + 1 < sim.nx {
        let neighbor = sim.idx(i + 1, j, k);
        let derived_neighbor = state.derive_cell(sim, neighbor);
        if let Some(flux) = interface_flux_contribution(
            sim,
            state,
            dt_days,
            cell_idx,
            neighbor,
            'x',
            k,
            k,
            &derived_cell,
            &derived_neighbor,
        ) {
            breakdown.x_plus = flux[0][component];
        }
    }
    if j > 0 {
        let neighbor = sim.idx(i, j - 1, k);
        let derived_neighbor = state.derive_cell(sim, neighbor);
        if let Some(flux) = interface_flux_contribution(
            sim,
            state,
            dt_days,
            neighbor,
            cell_idx,
            'y',
            k,
            k,
            &derived_neighbor,
            &derived_cell,
        ) {
            breakdown.y_minus = flux[1][component];
        }
    }
    if j + 1 < sim.ny {
        let neighbor = sim.idx(i, j + 1, k);
        let derived_neighbor = state.derive_cell(sim, neighbor);
        if let Some(flux) = interface_flux_contribution(
            sim,
            state,
            dt_days,
            cell_idx,
            neighbor,
            'y',
            k,
            k,
            &derived_cell,
            &derived_neighbor,
        ) {
            breakdown.y_plus = flux[0][component];
        }
    }
    if k > 0 {
        let neighbor = sim.idx(i, j, k - 1);
        let derived_neighbor = state.derive_cell(sim, neighbor);
        if let Some(flux) = interface_flux_contribution(
            sim,
            state,
            dt_days,
            neighbor,
            cell_idx,
            'z',
            k - 1,
            k,
            &derived_neighbor,
            &derived_cell,
        ) {
            breakdown.z_minus = flux[1][component];
        }
    }
    if k + 1 < sim.nz {
        let neighbor = sim.idx(i, j, k + 1);
        let derived_neighbor = state.derive_cell(sim, neighbor);
        if let Some(flux) = interface_flux_contribution(
            sim,
            state,
            dt_days,
            cell_idx,
            neighbor,
            'z',
            k,
            k + 1,
            &derived_cell,
            &derived_neighbor,
        ) {
            breakdown.z_plus = flux[0][component];
        }
    }

    for (perf_idx, perforation) in topology.perforations.iter().enumerate() {
        if perforation.cell_index == cell_idx {
            breakdown.well_source +=
                perforation_component_rates_sc_day(sim, state, topology, perf_idx)[component]
                    * dt_days;
        }
    }

    breakdown.total = breakdown.accumulation
        + breakdown.x_minus
        + breakdown.x_plus
        + breakdown.y_minus
        + breakdown.y_plus
        + breakdown.z_minus
        + breakdown.z_plus
        + breakdown.well_source;

    Some(breakdown)
}

pub(crate) fn cell_face_phase_flux_diagnostics(
    sim: &ReservoirSimulator,
    state: &FimState,
    dt_days: f64,
    cell_idx: usize,
) -> Option<CellFacePhaseDiagnostics> {
    if cell_idx >= state.cells.len() {
        return None;
    }

    let derived_cell = state.derive_cell(sim, cell_idx);
    let cells_per_layer = sim.nx * sim.ny;
    let k = cell_idx / cells_per_layer;
    let in_layer = cell_idx % cells_per_layer;
    let j = in_layer / sim.nx;
    let i = in_layer % sim.nx;

    let x_minus = if i > 0 {
        let neighbor = sim.idx(i - 1, j, k);
        let derived_neighbor = state.derive_cell(sim, neighbor);
        oriented_face_phase_diagnostics(
            sim,
            state,
            dt_days,
            neighbor,
            cell_idx,
            'x',
            k,
            k,
            &derived_neighbor,
            &derived_cell,
            1,
        )
    } else {
        None
    };

    let x_plus = if i + 1 < sim.nx {
        let neighbor = sim.idx(i + 1, j, k);
        let derived_neighbor = state.derive_cell(sim, neighbor);
        oriented_face_phase_diagnostics(
            sim,
            state,
            dt_days,
            cell_idx,
            neighbor,
            'x',
            k,
            k,
            &derived_cell,
            &derived_neighbor,
            0,
        )
    } else {
        None
    };

    let y_minus = if j > 0 {
        let neighbor = sim.idx(i, j - 1, k);
        let derived_neighbor = state.derive_cell(sim, neighbor);
        oriented_face_phase_diagnostics(
            sim,
            state,
            dt_days,
            neighbor,
            cell_idx,
            'y',
            k,
            k,
            &derived_neighbor,
            &derived_cell,
            1,
        )
    } else {
        None
    };

    let y_plus = if j + 1 < sim.ny {
        let neighbor = sim.idx(i, j + 1, k);
        let derived_neighbor = state.derive_cell(sim, neighbor);
        oriented_face_phase_diagnostics(
            sim,
            state,
            dt_days,
            cell_idx,
            neighbor,
            'y',
            k,
            k,
            &derived_cell,
            &derived_neighbor,
            0,
        )
    } else {
        None
    };

    Some(CellFacePhaseDiagnostics {
        x_minus,
        x_plus,
        y_minus,
        y_plus,
    })
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

    let d_oil_d_p =
        d_pore_volume_d_p * derived.so / bo - pore_volume_m3 * derived.so * d_bo_d_p / (bo * bo);
    let d_oil_d_sw = pore_volume_m3 * d_so_d_sw / bo;
    let d_oil_d_h = pore_volume_m3 * d_so_d_h / bo
        - pore_volume_m3 * derived.so * d_bo_d_rs * d_rs_d_h / (bo * bo);

    let d_free_gas_d_p =
        d_pore_volume_d_p * derived.sg / bg - pore_volume_m3 * derived.sg * d_bg_d_p / (bg * bg);
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
    assemble_residual_with_flags(
        sim,
        previous_state,
        state,
        topology,
        options,
        true,
        true,
        derived,
        prev_derived,
    )
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
            let accumulation = cell_accumulation_residual(
                sim,
                previous_state,
                state,
                cell_idx,
                &derived[cell_idx],
                &prev_derived[cell_idx],
            );
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
                            sim,
                            state,
                            options.dt_days,
                            id,
                            id_j,
                            'x',
                            k,
                            k,
                            &derived[id],
                            &derived[id_j],
                            &mut residual,
                        );
                    }
                    if j + 1 < sim.ny {
                        let id_j = sim.idx(i, j + 1, k);
                        add_interface_flux(
                            sim,
                            state,
                            options.dt_days,
                            id,
                            id_j,
                            'y',
                            k,
                            k,
                            &derived[id],
                            &derived[id_j],
                            &mut residual,
                        );
                    }
                    if k + 1 < sim.nz {
                        let id_j = sim.idx(i, j, k + 1);
                        add_interface_flux(
                            sim,
                            state,
                            options.dt_days,
                            id,
                            id_j,
                            'z',
                            k,
                            k + 1,
                            &derived[id],
                            &derived[id_j],
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
    derived: &[FimCellDerived],
    tri: &mut TriMatI<f64, usize>,
) {
    for cell_idx in 0..state.cells.len() {
        let block = cell_accumulation_jacobian_block(
            sim,
            previous_state,
            state,
            cell_idx,
            &derived[cell_idx],
        );
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
        [
            sim.get_d_bo_d_p_for_state(cell.pressure_bar, derived.rs, true),
            0.0,
            0.0,
        ]
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
    let terms = interface_flux_terms(sim, state, id_i, id_j, dim, k_i, k_j, derived_i, derived_j)?;
    let flux_sc = [
        terms.flux_sc_day[0] * dt_days,
        terms.flux_sc_day[1] * dt_days,
        terms.flux_sc_day[2] * dt_days,
    ];
    Some([
        [flux_sc[0], flux_sc[1], flux_sc[2]],
        [-flux_sc[0], -flux_sc[1], -flux_sc[2]],
    ])
}

fn interface_flux_terms(
    sim: &ReservoirSimulator,
    state: &FimState,
    id_i: usize,
    id_j: usize,
    dim: char,
    k_i: usize,
    k_j: usize,
    derived_i: &FimCellDerived,
    derived_j: &FimCellDerived,
) -> Option<InterfaceFluxTerms> {
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
        (id_i, derived_i, mobilities_i)
    } else {
        (id_j, derived_j, mobilities_j)
    };
    let oil_upstream = if dphi_o >= 0.0 {
        (id_i, derived_i, mobilities_i)
    } else {
        (id_j, derived_j, mobilities_j)
    };
    let gas_upstream = if dphi_g >= 0.0 {
        (id_i, derived_i, mobilities_i)
    } else {
        (id_j, derived_j, mobilities_j)
    };

    let q_w_sc_day = geom_t * water_upstream.2.water * dphi_w / sim.b_w.max(1e-9);
    let q_o_res_day = geom_t * oil_upstream.2.oil * dphi_o;
    let q_o_sc_day = q_o_res_day / oil_upstream.1.bo.max(1e-9);
    let q_g_free_sc_day = geom_t * gas_upstream.2.gas * dphi_g / gas_upstream.1.bg.max(1e-9);
    let q_g_dissolved_sc_day = q_o_sc_day * oil_upstream.1.rs;
    let q_g_sc_day = q_g_free_sc_day + q_g_dissolved_sc_day;

    Some(InterfaceFluxTerms {
        dphi: [dphi_w, dphi_o, dphi_g],
        upwind_cell_idx: [water_upstream.0, oil_upstream.0, gas_upstream.0],
        mobility: [
            water_upstream.2.water,
            oil_upstream.2.oil,
            gas_upstream.2.gas,
        ],
        flux_sc_day: [q_w_sc_day, q_o_sc_day, q_g_sc_day],
    })
}

fn oriented_face_phase_diagnostics(
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
    target_side: usize,
) -> Option<FacePhaseDiagnostics> {
    let terms = interface_flux_terms(sim, state, id_i, id_j, dim, k_i, k_j, derived_i, derived_j)?;
    let sign = if target_side == 0 { 1.0 } else { -1.0 };

    Some(FacePhaseDiagnostics {
        water: PhaseFluxDiagnostic {
            dphi: terms.dphi[0],
            upwind_cell_idx: terms.upwind_cell_idx[0],
            mobility: terms.mobility[0],
            flux: sign * terms.flux_sc_day[0] * dt_days,
        },
        oil: PhaseFluxDiagnostic {
            dphi: terms.dphi[1],
            upwind_cell_idx: terms.upwind_cell_idx[1],
            mobility: terms.mobility[1],
            flux: sign * terms.flux_sc_day[1] * dt_days,
        },
        gas: PhaseFluxDiagnostic {
            dphi: terms.dphi[2],
            upwind_cell_idx: terms.upwind_cell_idx[2],
            mobility: terms.mobility[2],
            flux: sign * terms.flux_sc_day[2] * dt_days,
        },
    })
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
                    add_exact_interface_flux_jacobian(
                        sim,
                        state,
                        dt_days,
                        id,
                        id_j,
                        'x',
                        k,
                        k,
                        &derived[id],
                        &derived[id_j],
                        &sensitivities[id],
                        &sensitivities[id_j],
                        tri,
                    );
                }
                if j + 1 < sim.ny {
                    let id_j = sim.idx(i, j + 1, k);
                    add_exact_interface_flux_jacobian(
                        sim,
                        state,
                        dt_days,
                        id,
                        id_j,
                        'y',
                        k,
                        k,
                        &derived[id],
                        &derived[id_j],
                        &sensitivities[id],
                        &sensitivities[id_j],
                        tri,
                    );
                }
                if k + 1 < sim.nz {
                    let id_j = sim.idx(i, j, k + 1);
                    add_exact_interface_flux_jacobian(
                        sim,
                        state,
                        dt_days,
                        id,
                        id_j,
                        'z',
                        k,
                        k + 1,
                        &derived[id],
                        &derived[id_j],
                        &sensitivities[id],
                        &sensitivities[id_j],
                        tri,
                    );
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
    let grav_w = grav_half * (derived_i.rho_w + derived_j.rho_w);
    let grav_o = grav_half * (derived_i.rho_o + derived_j.rho_o);
    let grav_g = grav_half * (derived_i.rho_g + derived_j.rho_g);

    let dphi_w = (p_i - p_j) - (pcw_i - pcw_j) - grav_w;
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
            let mut dq_w_sc_day =
                geom_t * lambda_w * dphi_w_derivatives[local_var] / sim.b_w.max(1e-9);
            if side_idx == water_upwind {
                dq_w_sc_day +=
                    geom_t * locals[side_idx].mobility_derivatives[0][local_var] * dphi_w
                        / sim.b_w.max(1e-9);
            }

            let mut dq_o_sc_day = geom_t * lambda_o * dphi_o_derivatives[local_var] / bo_up;
            if side_idx == oil_upwind {
                dq_o_sc_day +=
                    geom_t * locals[side_idx].mobility_derivatives[1][local_var] * dphi_o / bo_up;
                dq_o_sc_day -=
                    geom_t * lambda_o * dphi_o * locals[side_idx].bo_derivatives[local_var]
                        / (bo_up * bo_up);
            }

            let mut dq_g_free_sc_day = geom_t * lambda_g * dphi_g_derivatives[local_var] / bg_up;
            if side_idx == gas_upwind {
                dq_g_free_sc_day +=
                    geom_t * locals[side_idx].mobility_derivatives[2][local_var] * dphi_g / bg_up;
                dq_g_free_sc_day -=
                    geom_t * lambda_g * dphi_g * locals[side_idx].bg_derivatives[local_var]
                        / (bg_up * bg_up);
            }

            let mut dq_g_sc_day = dq_g_free_sc_day + rs_up * dq_o_sc_day;
            if side_idx == oil_upwind {
                dq_g_sc_day += q_o_sc_day * locals[side_idx].rs_derivatives[local_var];
            }

            let derivatives = [
                dq_w_sc_day * dt_days,
                dq_o_sc_day * dt_days,
                dq_g_sc_day * dt_days,
            ];
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
                    add_interface_flux_jacobian_fd(
                        sim,
                        state,
                        dt_days,
                        id,
                        sim.idx(i + 1, j, k),
                        'x',
                        k,
                        k,
                        tri,
                    );
                }
                if j + 1 < sim.ny {
                    add_interface_flux_jacobian_fd(
                        sim,
                        state,
                        dt_days,
                        id,
                        sim.idx(i, j + 1, k),
                        'y',
                        k,
                        k,
                        tri,
                    );
                }
                if k + 1 < sim.nz {
                    add_interface_flux_jacobian_fd(
                        sim,
                        state,
                        dt_days,
                        id,
                        sim.idx(i, j, k + 1),
                        'z',
                        k,
                        k + 1,
                        tri,
                    );
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
    let Some(base_flux) = interface_flux_contribution(
        sim,
        state,
        dt_days,
        id_i,
        id_j,
        dim,
        k_i,
        k_j,
        &base_derived_i,
        &base_derived_j,
    ) else {
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
                    let derivative = (perturbed_flux[eq_side][component]
                        - base_flux[eq_side][component])
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
    let Some(flux) = interface_flux_contribution(
        sim, state, dt_days, id_i, id_j, dim, k_i, k_j, derived_i, derived_j,
    ) else {
        return;
    };

    for component in 0..3 {
        residual[equation_offset(id_i, component)] += flux[0][component];
        residual[equation_offset(id_j, component)] += flux[1][component];
    }
}

#[cfg(test)]
#[path = "assembly_tests.rs"]
mod assembly_tests;
