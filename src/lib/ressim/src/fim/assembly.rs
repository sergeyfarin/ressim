use nalgebra::DVector;
use sprs::{CsMat, TriMatI};

use crate::fim::scaling::{
    build_equation_scaling, build_variable_scaling, EquationScaling, VariableScaling,
};
use crate::fim::state::FimState;
use crate::ReservoirSimulator;

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
    let n_unknowns = state.n_unknowns();
    let equation_scaling = build_equation_scaling(sim, state, options.dt_days);
    let variable_scaling = build_variable_scaling(sim, state);
    let mut tri = TriMatI::<f64, usize>::new((n_unknowns, n_unknowns));
    let mut residual = DVector::zeros(n_unknowns);

    for cell_idx in 0..state.cells.len() {
        let cell_residual = cell_accumulation_residual(sim, previous_state, state, cell_idx);
        for local_eq in 0..3 {
            residual[equation_offset(cell_idx, local_eq)] = cell_residual[local_eq];
        }

        let base_cell = state.cell(cell_idx);
        for local_var in 0..3 {
            let perturbation = finite_difference_step(base_cell, local_var);
            let mut perturbed_state = state.clone();
            let perturbed_cell = perturbed_state.cell_mut(cell_idx);
            match local_var {
                0 => perturbed_cell.pressure_bar += perturbation,
                1 => perturbed_cell.sw += perturbation,
                2 => perturbed_cell.hydrocarbon_var += perturbation,
                _ => unreachable!(),
            }
            let perturbed_residual =
                cell_accumulation_residual(sim, previous_state, &perturbed_state, cell_idx);

            for local_eq in 0..3 {
                let jac = (perturbed_residual[local_eq] - cell_residual[local_eq]) / perturbation;
                tri.add_triplet(
                    equation_offset(cell_idx, local_eq),
                    unknown_offset(cell_idx, local_var),
                    jac,
                );
            }
        }
    }

    FimAssembly {
        residual,
        jacobian: tri.to_csr(),
        equation_scaling,
        variable_scaling,
    }
}

fn finite_difference_step(cell: &crate::fim::state::FimCellState, local_var: usize) -> f64 {
    match local_var {
        0 => 1e-5 * cell.pressure_bar.abs().max(1.0),
        1 => 1e-7,
        2 => 1e-7 * cell.hydrocarbon_var.abs().max(1.0),
        _ => unreachable!(),
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

#[cfg(test)]
mod tests {
    use crate::fim::state::{FimCellState, FimState, HydrocarbonState};
    use crate::ReservoirSimulator;

    use super::*;

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
        assert_eq!(assembly.jacobian.nnz(), 18);
        assert_eq!(assembly.equation_scaling.water.len(), 2);
        assert_eq!(assembly.variable_scaling.pressure.len(), 2);
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
}
