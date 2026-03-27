use crate::fim::state::{FimState, HydrocarbonState};
use crate::ReservoirSimulator;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EquationScaling {
    pub(crate) water: Vec<f64>,
    pub(crate) oil_component: Vec<f64>,
    pub(crate) gas_component: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VariableScaling {
    pub(crate) pressure: Vec<f64>,
    pub(crate) sw: Vec<f64>,
    pub(crate) hydrocarbon_var: Vec<f64>,
}

pub(crate) fn build_equation_scaling(
    sim: &ReservoirSimulator,
    state: &FimState,
    dt_days: f64,
) -> EquationScaling {
    let n_cells = state.cells.len();
    let mut water = Vec::with_capacity(n_cells);
    let mut oil_component = Vec::with_capacity(n_cells);
    let mut gas_component = Vec::with_capacity(n_cells);

    let dt_days = dt_days.max(1e-12);
    for idx in 0..n_cells {
        let pv_over_dt = (sim.pore_volume_m3(idx) / dt_days).abs().max(1.0);
        water.push(pv_over_dt);
        oil_component.push(pv_over_dt);
        gas_component.push(pv_over_dt);
    }

    EquationScaling {
        water,
        oil_component,
        gas_component,
    }
}

pub(crate) fn build_variable_scaling(
    _sim: &ReservoirSimulator,
    state: &FimState,
) -> VariableScaling {
    let n_cells = state.cells.len();
    let mut pressure = Vec::with_capacity(n_cells);
    let mut sw = Vec::with_capacity(n_cells);
    let mut hydrocarbon_var = Vec::with_capacity(n_cells);

    for cell in &state.cells {
        pressure.push(cell.pressure_bar.abs().max(1.0));
        sw.push(1.0);
        hydrocarbon_var.push(match cell.regime {
            HydrocarbonState::Saturated => 1.0,
            HydrocarbonState::Undersaturated => cell.hydrocarbon_var.abs().max(1.0),
        });
    }

    VariableScaling {
        pressure,
        sw,
        hydrocarbon_var,
    }
}

#[cfg(test)]
mod tests {
    use crate::fim::state::{FimCellState, FimState, HydrocarbonState};
    use crate::ReservoirSimulator;

    use super::*;

    #[test]
    fn variable_scaling_uses_pressure_and_regime_aware_hydrocarbon_scale() {
        let sim = ReservoirSimulator::new(1, 1, 2, 0.2);
        let state = FimState {
            cells: vec![
                FimCellState {
                    pressure_bar: 250.0,
                    sw: 0.3,
                    hydrocarbon_var: 0.1,
                    regime: HydrocarbonState::Saturated,
                },
                FimCellState {
                    pressure_bar: 0.5,
                    sw: 0.3,
                    hydrocarbon_var: 42.0,
                    regime: HydrocarbonState::Undersaturated,
                },
            ],
        };

        let scaling = build_variable_scaling(&sim, &state);
        assert_eq!(scaling.pressure, vec![250.0, 1.0]);
        assert_eq!(scaling.sw, vec![1.0, 1.0]);
        assert_eq!(scaling.hydrocarbon_var, vec![1.0, 42.0]);
    }
}
