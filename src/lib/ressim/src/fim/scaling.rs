use nalgebra::DVector;

use crate::ReservoirSimulator;
use crate::fim::state::{FimState, HydrocarbonState};
use crate::fim::wells::FimWellTopology;
use crate::fim::wells::{physical_well_control, well_local_block};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EquationScaling {
    pub(crate) water: Vec<f64>,
    pub(crate) oil_component: Vec<f64>,
    pub(crate) gas_component: Vec<f64>,
    pub(crate) well_constraint: Vec<f64>,
    pub(crate) perforation_flow: Vec<f64>,
}

/// Per-equation-family peak of a scaled residual vector (row-space, not variable-space).
/// Mirrors `fim/newton.rs`'s `residual_family_diagnostics` peak computation, but as a
/// reusable value the linear solver can also compute — see
/// `EquationScaling::family_peaks`/`family_relative_reduction_ok`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct EquationFamilyPeaks {
    pub(crate) water: f64,
    pub(crate) oil_component: f64,
    pub(crate) gas_component: f64,
    pub(crate) well_constraint: f64,
    pub(crate) perforation_flow: f64,
}

impl EquationScaling {
    /// Per-family peak of `|residual[row]| / scale[row]`, using this scaling's own row
    /// partition (cell rows in `water, oil_component, gas_component` interleaved triples,
    /// then well-constraint rows, then perforation-flow rows). `residual` must be laid out
    /// in exactly that row order (true for both the Newton residual and a FIM linear
    /// system's residual, which share the same unknown/equation ordering).
    pub(crate) fn family_peaks(&self, residual: &DVector<f64>) -> EquationFamilyPeaks {
        let mut peaks = EquationFamilyPeaks::default();
        let n_cells = self.water.len();

        for i in 0..n_cells {
            peaks.water = peaks.water.max(residual[i * 3].abs() / self.water[i]);
            peaks.oil_component = peaks
                .oil_component
                .max(residual[i * 3 + 1].abs() / self.oil_component[i]);
            peaks.gas_component = peaks
                .gas_component
                .max(residual[i * 3 + 2].abs() / self.gas_component[i]);
        }

        let mut offset = n_cells * 3;
        for (i, scale) in self.well_constraint.iter().enumerate() {
            peaks.well_constraint = peaks
                .well_constraint
                .max(residual[offset + i].abs() / scale);
        }
        offset += self.well_constraint.len();
        for (i, scale) in self.perforation_flow.iter().enumerate() {
            peaks.perforation_flow = peaks
                .perforation_flow
                .max(residual[offset + i].abs() / scale);
        }

        peaks
    }
}

impl EquationFamilyPeaks {
    /// True iff every family's scaled peak has been reduced to within
    /// `absolute_tolerance + relative_tolerance * initial_peak` of its own value at the
    /// start of the solve (`initial`, i.e. the scaled peaks of the rhs since `x_0 = 0`).
    /// This is the per-family generalization of the linear solver's global relative-
    /// reduction stopping criterion (`FIM-LINEAR-008`'s tolerance): a family with small
    /// raw magnitude (e.g. a well perforation row) gets its own reduction target instead of
    /// being invisible to a single whole-system norm dominated by larger-magnitude families.
    pub(crate) fn within_relative_reduction(
        &self,
        initial: &EquationFamilyPeaks,
        absolute_tolerance: f64,
        relative_tolerance: f64,
    ) -> bool {
        let ok = |current: f64, initial_peak: f64| {
            current <= absolute_tolerance + relative_tolerance * initial_peak.max(f64::EPSILON)
        };
        ok(self.water, initial.water)
            && ok(self.oil_component, initial.oil_component)
            && ok(self.gas_component, initial.gas_component)
            && ok(self.well_constraint, initial.well_constraint)
            && ok(self.perforation_flow, initial.perforation_flow)
    }
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
        let block = well_local_block(topology, state, well_idx);
        let bhp_bar = block.bhp_bar();
        let control = physical_well_control(sim, topology, well_idx);
        if control.rate_controlled {
            if let Some((_bhp_slack, _rate_slack)) = block.control_slacks(sim) {
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

    fn sample_scaling() -> EquationScaling {
        EquationScaling {
            water: vec![10.0, 10.0],
            oil_component: vec![10.0, 10.0],
            gas_component: vec![10.0, 10.0],
            well_constraint: vec![1.0],
            perforation_flow: vec![1000.0],
        }
    }

    #[test]
    fn family_peaks_scales_each_row_by_its_own_family_scale() {
        let scaling = sample_scaling();
        // cell0: water=5, oil=12, gas=8; cell1: water=4, oil=9, gas=6; well=3; perf=4000
        let residual = DVector::from_vec(vec![5.0, 12.0, 8.0, 4.0, 9.0, 6.0, 3.0, 4000.0]);

        let peaks = scaling.family_peaks(&residual);

        assert!((peaks.water - 0.5).abs() < 1e-12);
        assert!((peaks.oil_component - 1.2).abs() < 1e-12);
        assert!((peaks.gas_component - 0.8).abs() < 1e-12);
        assert!((peaks.well_constraint - 3.0).abs() < 1e-12);
        assert!((peaks.perforation_flow - 4.0).abs() < 1e-12);
    }

    #[test]
    fn within_relative_reduction_requires_every_family_to_meet_its_own_target() {
        let initial = EquationFamilyPeaks {
            water: 100.0,
            oil_component: 100.0,
            gas_component: 100.0,
            well_constraint: 100.0,
            perforation_flow: 100.0,
        };
        // All families reduced by 1% except perforation_flow, which barely moved.
        let mostly_reduced = EquationFamilyPeaks {
            water: 1.0,
            oil_component: 1.0,
            gas_component: 1.0,
            well_constraint: 1.0,
            perforation_flow: 99.0,
        };

        assert!(!mostly_reduced.within_relative_reduction(&initial, 1e-12, 5e-2));

        let fully_reduced = EquationFamilyPeaks {
            perforation_flow: 1.0,
            ..mostly_reduced
        };
        assert!(fully_reduced.within_relative_reduction(&initial, 1e-12, 5e-2));
    }

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
