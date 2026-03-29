use crate::ReservoirSimulator;
use crate::fim::state::{FimState, HydrocarbonState};
use crate::fim::wells::FimWellTopology;
use crate::fim::wells::{physical_well_control, well_control_slacks};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EquationScaling {
    pub(crate) water: Vec<f64>,
    pub(crate) oil_component: Vec<f64>,
    pub(crate) gas_component: Vec<f64>,
    pub(crate) well_constraint: Vec<f64>,
    pub(crate) perforation_flow: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VariableScaling {
    pub(crate) pressure: Vec<f64>,
    pub(crate) sw: Vec<f64>,
    pub(crate) hydrocarbon_var: Vec<f64>,
    pub(crate) well_bhp: Vec<f64>,
    pub(crate) perforation_rate: Vec<f64>,
}

pub(crate) fn build_equation_scaling(
    sim: &ReservoirSimulator,
    state: &FimState,
    topology: &FimWellTopology,
    dt_days: f64,
) -> EquationScaling {
    let n_cells = state.cells.len();
    let mut water = Vec::with_capacity(n_cells);
    let mut oil_component = Vec::with_capacity(n_cells);
    let mut gas_component = Vec::with_capacity(n_cells);
    let mut well_constraint = Vec::with_capacity(state.n_well_unknowns());
    let mut perforation_flow = Vec::with_capacity(state.n_perforation_unknowns());

    let dt_days = dt_days.max(1e-12);
    for idx in 0..n_cells {
        let pv_over_dt = (sim.pore_volume_m3(idx) / dt_days).abs().max(1.0);
        let cell = state.cells[idx];
        let bo = sim.get_b_o_cell(idx, cell.pressure_bar).max(1e-9);
        let bg = sim.get_b_g(cell.pressure_bar).max(1e-9);
        let bw = sim.b_w.max(1e-9);

        water.push(pv_over_dt / bw);
        oil_component.push(pv_over_dt / bo);
        gas_component.push(pv_over_dt / bg);
    }

    for well_idx in 0..state.n_well_unknowns() {
        let bhp_bar = state.well_bhp[well_idx];
        let control = physical_well_control(sim, topology, well_idx);
        if control.rate_controlled {
            if let Some((_bhp_slack, _rate_slack)) =
                well_control_slacks(sim, state, topology, well_idx)
            {
                // Scaled FB residual is O(1), so equation scale = 1.0
                well_constraint.push(1.0);
                continue;
            }
        }
        well_constraint.push(bhp_bar.abs().max(1.0));
    }
    for rate in &state.perforation_rates_m3_day {
        perforation_flow.push(rate.abs().max(1.0));
    }

    EquationScaling {
        water,
        oil_component,
        gas_component,
        well_constraint,
        perforation_flow,
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
    let mut well_bhp = Vec::with_capacity(state.n_well_unknowns());
    let mut perforation_rate = Vec::with_capacity(state.n_perforation_unknowns());

    for cell in &state.cells {
        pressure.push(cell.pressure_bar.abs().max(1.0));
        sw.push(1.0);
        hydrocarbon_var.push(match cell.regime {
            HydrocarbonState::Saturated => 1.0,
            HydrocarbonState::Undersaturated => cell.hydrocarbon_var.abs().max(1.0),
        });
    }

    for bhp_bar in &state.well_bhp {
        well_bhp.push(bhp_bar.abs().max(1.0));
    }
    for rate in &state.perforation_rates_m3_day {
        perforation_rate.push(rate.abs().max(1.0));
    }

    VariableScaling {
        pressure,
        sw,
        hydrocarbon_var,
        well_bhp,
        perforation_rate,
    }
}

#[cfg(test)]
mod tests {
    use crate::ReservoirSimulator;
    use crate::fim::state::{FimCellState, FimState, HydrocarbonState};

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
            well_bhp: vec![350.0],
            perforation_rates_m3_day: vec![-25.0],
        };

        let scaling = build_variable_scaling(&sim, &state);
        assert_eq!(scaling.pressure, vec![250.0, 1.0]);
        assert_eq!(scaling.sw, vec![1.0, 1.0]);
        assert_eq!(scaling.hydrocarbon_var, vec![1.0, 42.0]);
        assert_eq!(scaling.well_bhp, vec![350.0]);
        assert_eq!(scaling.perforation_rate, vec![25.0]);
    }
}
