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
    state: &FimState,
    options: &FimAssemblyOptions,
) -> FimAssembly {
    let n_unknowns = state.n_unknowns();
    let equation_scaling = build_equation_scaling(sim, state, options.dt_days);
    let variable_scaling = build_variable_scaling(sim, state);

    // Temporary scaffold: allocate a well-posed diagonal system so later slices can
    // replace this with true coupled residual and Jacobian assembly without changing
    // the surrounding API.
    let mut tri = TriMatI::<f64, usize>::new((n_unknowns, n_unknowns));
    for idx in 0..n_unknowns {
        tri.add_triplet(idx, idx, 1.0);
    }

    FimAssembly {
        residual: DVector::zeros(n_unknowns),
        jacobian: tri.to_csr(),
        equation_scaling,
        variable_scaling,
    }
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
            &FimAssemblyOptions {
                dt_days: 1.0,
                include_wells: false,
            },
        );

        assert_eq!(assembly.residual.len(), 6);
        assert_eq!(assembly.jacobian.rows(), 6);
        assert_eq!(assembly.jacobian.cols(), 6);
        assert_eq!(assembly.jacobian.nnz(), 6);
        assert_eq!(assembly.equation_scaling.water.len(), 2);
        assert_eq!(assembly.variable_scaling.pressure.len(), 2);
    }
}
