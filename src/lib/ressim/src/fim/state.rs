use crate::fim::flash::{classify_cell_regime, resolve_cell_flash};
use crate::ReservoirSimulator;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HydrocarbonState {
    Saturated,
    Undersaturated,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimCellState {
    pub(crate) pressure_bar: f64,
    pub(crate) sw: f64,
    pub(crate) hydrocarbon_var: f64,
    pub(crate) regime: HydrocarbonState,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimCellDerived {
    pub(crate) so: f64,
    pub(crate) sg: f64,
    pub(crate) rs: f64,
    pub(crate) bo: f64,
    pub(crate) bg: f64,
    pub(crate) mu_o: f64,
    pub(crate) mu_g: f64,
    pub(crate) mu_w: f64,
    pub(crate) rho_o: f64,
    pub(crate) rho_g: f64,
    pub(crate) rho_w: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FimState {
    pub(crate) cells: Vec<FimCellState>,
}

impl FimState {
    pub(crate) fn from_simulator(sim: &ReservoirSimulator) -> Self {
        let n_cells = sim.nx * sim.ny * sim.nz;
        let mut cells = Vec::with_capacity(n_cells);

        for idx in 0..n_cells {
            let pressure_bar = sim.pressure[idx];
            let sw = sim.sat_water[idx];
            let regime = classify_cell_regime(sim, pressure_bar, sim.sat_gas[idx], sim.rs[idx]);
            let hydrocarbon_var = match regime {
                HydrocarbonState::Saturated => sim.sat_gas[idx],
                HydrocarbonState::Undersaturated => sim.rs[idx],
            };

            cells.push(FimCellState {
                pressure_bar,
                sw,
                hydrocarbon_var,
                regime,
            });
        }

        Self { cells }
    }

    pub(crate) fn cell(&self, idx: usize) -> &FimCellState {
        &self.cells[idx]
    }

    pub(crate) fn cell_mut(&mut self, idx: usize) -> &mut FimCellState {
        &mut self.cells[idx]
    }

    pub(crate) fn n_unknowns(&self) -> usize {
        self.cells.len() * 3
    }

    pub(crate) fn classify_regimes(&mut self, sim: &ReservoirSimulator) {
        for cell in &mut self.cells {
            let (gas_saturation, rs_sm3_sm3) = match cell.regime {
                HydrocarbonState::Saturated => (
                    cell.hydrocarbon_var,
                    sim.pvt_table
                        .as_ref()
                        .map(|table| table.interpolate(cell.pressure_bar).rs_m3m3)
                        .unwrap_or(0.0),
                ),
                HydrocarbonState::Undersaturated => (0.0, cell.hydrocarbon_var),
            };
            let regime = classify_cell_regime(sim, cell.pressure_bar, gas_saturation, rs_sm3_sm3);
            cell.hydrocarbon_var = match regime {
                HydrocarbonState::Saturated => gas_saturation,
                HydrocarbonState::Undersaturated => rs_sm3_sm3,
            };
            cell.regime = regime;
        }
    }

    pub(crate) fn derive_cell(&self, sim: &ReservoirSimulator, idx: usize) -> FimCellDerived {
        let cell = self.cell(idx);
        let flash = resolve_cell_flash(
            sim,
            cell.pressure_bar,
            cell.sw,
            cell.hydrocarbon_var,
            cell.regime,
        );
        let oil = sim.oil_props_for_state(cell.pressure_bar, flash.rs);
        let gas = sim.gas_props_for_state(cell.pressure_bar);

        FimCellDerived {
            so: flash.so,
            sg: flash.sg,
            rs: flash.rs,
            bo: oil.bo_m3m3,
            bg: gas.bg_m3m3,
            mu_o: oil.mu_o_cp,
            mu_g: gas.mu_g_cp,
            mu_w: sim.get_mu_w(cell.pressure_bar),
            rho_o: oil.rho_o_kg_m3,
            rho_g: gas.rho_g_kg_m3,
            rho_w: sim.get_rho_w(cell.pressure_bar),
        }
    }

    pub(crate) fn is_finite(&self) -> bool {
        self.cells.iter().all(|cell| {
            cell.pressure_bar.is_finite() && cell.sw.is_finite() && cell.hydrocarbon_var.is_finite()
        })
    }

    pub(crate) fn respects_basic_bounds(&self, sim: &ReservoirSimulator) -> bool {
        self.cells.iter().enumerate().all(|(idx, cell)| {
            let derived = self.derive_cell(sim, idx);
            cell.sw >= -1e-9
                && cell.sw <= 1.0 + 1e-9
                && derived.sg >= -1e-9
                && derived.so >= -1e-9
                && derived.so <= 1.0 + 1e-9
                && derived.sg <= 1.0 + 1e-9
                && (cell.sw + derived.so + derived.sg - 1.0).abs() < 1e-6
        })
    }

    pub(crate) fn write_back_to_simulator(&self, sim: &mut ReservoirSimulator) {
        for (idx, cell) in self.cells.iter().enumerate() {
            let derived = self.derive_cell(sim, idx);
            sim.pressure[idx] = cell.pressure_bar;
            sim.sat_water[idx] = cell.sw;
            sim.sat_gas[idx] = derived.sg;
            sim.sat_oil[idx] = derived.so;
            sim.rs[idx] = derived.rs;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::pvt::{PvtRow, PvtTable};
    use crate::ReservoirSimulator;

    use super::*;

    #[test]
    fn from_simulator_uses_rs_for_undersaturated_cells() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_mode_enabled(true);
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 100.0,
                    rs_m3m3: 10.0,
                    bo_m3m3: 1.1,
                    mu_o_cp: 1.2,
                    bg_m3m3: 0.01,
                    mu_g_cp: 0.02,
                },
                PvtRow {
                    p_bar: 200.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.0,
                    mu_o_cp: 1.1,
                    bg_m3m3: 0.005,
                    mu_g_cp: 0.02,
                },
            ],
            sim.pvt.c_o,
        ));
        sim.pressure[0] = 150.0;
        sim.sat_water[0] = 0.25;
        sim.sat_gas[0] = 0.0;
        sim.rs[0] = 12.0;

        let state = FimState::from_simulator(&sim);
        assert_eq!(state.cells[0].regime, HydrocarbonState::Undersaturated);
        assert_eq!(state.cells[0].hydrocarbon_var, 12.0);
    }

    #[test]
    fn derive_cell_recovers_saturations_and_props() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_mode_enabled(true);
        sim.pvt_table = Some(PvtTable::new(
            vec![
                PvtRow {
                    p_bar: 100.0,
                    rs_m3m3: 10.0,
                    bo_m3m3: 1.1,
                    mu_o_cp: 1.2,
                    bg_m3m3: 0.01,
                    mu_g_cp: 0.02,
                },
                PvtRow {
                    p_bar: 200.0,
                    rs_m3m3: 20.0,
                    bo_m3m3: 1.0,
                    mu_o_cp: 1.1,
                    bg_m3m3: 0.005,
                    mu_g_cp: 0.02,
                },
            ],
            sim.pvt.c_o,
        ));
        let state = FimState {
            cells: vec![FimCellState {
                pressure_bar: 150.0,
                sw: 0.2,
                hydrocarbon_var: 0.1,
                regime: HydrocarbonState::Saturated,
            }],
        };

        let derived = state.derive_cell(&sim, 0);
        assert!((derived.sg - 0.1).abs() < 1e-12);
        assert!((derived.so - 0.7).abs() < 1e-12);
        assert!(derived.bo > 0.0);
        assert!(derived.bg > 0.0);
    }
}
