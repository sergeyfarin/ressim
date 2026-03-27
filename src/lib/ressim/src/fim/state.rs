use nalgebra::DVector;

use crate::fim::flash::{classify_cell_regime, resolve_cell_flash};
use crate::fim::wells::{
    build_well_topology, connection_rate_for_bhp, physical_well_control, solve_well_bhp_from_target,
};
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
    pub(crate) well_bhp: Vec<f64>,
    pub(crate) perforation_rates_m3_day: Vec<f64>,
}

impl FimState {
    pub(crate) fn from_simulator(sim: &ReservoirSimulator) -> Self {
        let n_cells = sim.nx * sim.ny * sim.nz;
        let topology = build_well_topology(sim);
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

        let mut state = Self {
            cells,
            well_bhp: topology
                .wells
                .iter()
                .enumerate()
                .map(|(well_idx, _)| physical_well_control(sim, &topology, well_idx).bhp_target)
                .collect(),
            perforation_rates_m3_day: vec![0.0; topology.perforations.len()],
        };

        for well_idx in 0..topology.wells.len() {
            if let Some((bhp_bar, _)) = solve_well_bhp_from_target(sim, &state, &topology, well_idx) {
                state.well_bhp[well_idx] = bhp_bar;
            }
        }

        for perf_idx in 0..topology.perforations.len() {
            let well_idx = topology.perforations[perf_idx].physical_well_index;
            let bhp_bar = state.well_bhp[well_idx];
            state.perforation_rates_m3_day[perf_idx] = if !physical_well_control(sim, &topology, well_idx).enabled {
                0.0
            } else {
                connection_rate_for_bhp(sim, &state, &topology, perf_idx, bhp_bar).unwrap_or(0.0)
            };
        }

        state
    }

    pub(crate) fn n_cell_unknowns(&self) -> usize {
        self.cells.len() * 3
    }

    pub(crate) fn n_well_unknowns(&self) -> usize {
        self.well_bhp.len()
    }

    pub(crate) fn n_perforation_unknowns(&self) -> usize {
        self.perforation_rates_m3_day.len()
    }

    pub(crate) fn cell(&self, idx: usize) -> &FimCellState {
        &self.cells[idx]
    }

    pub(crate) fn cell_mut(&mut self, idx: usize) -> &mut FimCellState {
        &mut self.cells[idx]
    }

    pub(crate) fn n_unknowns(&self) -> usize {
        self.n_cell_unknowns() + self.n_well_unknowns() + self.n_perforation_unknowns()
    }

    pub(crate) fn well_bhp_unknown_offset(&self, well_idx: usize) -> usize {
        self.n_cell_unknowns() + well_idx
    }

    pub(crate) fn well_equation_offset(&self, well_idx: usize) -> usize {
        self.n_cell_unknowns() + well_idx
    }

    pub(crate) fn perforation_rate_unknown_offset(&self, perf_idx: usize) -> usize {
        self.n_cell_unknowns() + self.n_well_unknowns() + perf_idx
    }

    pub(crate) fn perforation_equation_offset(&self, perf_idx: usize) -> usize {
        self.n_cell_unknowns() + self.n_well_unknowns() + perf_idx
    }

    pub(crate) fn classify_regimes(&mut self, sim: &ReservoirSimulator) {
        if !sim.three_phase_mode || sim.pvt_table.is_none() {
            return;
        }

        for idx in 0..self.cells.len() {
            let cell = self.cells[idx];
            let rs_sat = sim
                .pvt_table
                .as_ref()
                .map(|table| table.interpolate(cell.pressure_bar).rs_m3m3)
                .unwrap_or(0.0)
                .max(0.0);

            match cell.regime {
                HydrocarbonState::Saturated => {
                    let gas_saturation = cell.hydrocarbon_var.max(0.0);
                    if gas_saturation <= 1e-9 {
                        self.cells[idx].regime = HydrocarbonState::Undersaturated;
                        self.cells[idx].hydrocarbon_var = rs_sat;
                    } else {
                        self.cells[idx].hydrocarbon_var = gas_saturation;
                    }
                }
                HydrocarbonState::Undersaturated => {
                    let rs_sm3_sm3 = cell.hydrocarbon_var.max(0.0);
                    let regime = classify_cell_regime(sim, cell.pressure_bar, 0.0, rs_sm3_sm3);
                    if regime == HydrocarbonState::Undersaturated {
                        self.cells[idx].hydrocarbon_var = rs_sm3_sm3;
                        continue;
                    }

                    let hydrocarbon_saturation = (1.0 - cell.sw).max(0.0);
                    let pore_volume_m3 = sim.pore_volume_m3(idx).max(1e-9);
                    let bo = sim.get_b_o_for_rs(cell.pressure_bar, rs_sm3_sm3).max(1e-9);
                    let total_gas_sc = hydrocarbon_saturation * pore_volume_m3 * rs_sm3_sm3 / bo;
                    let (sg, _so, rs_resolved) = sim.split_gas_inventory_after_transport(
                        cell.pressure_bar,
                        pore_volume_m3,
                        cell.sw,
                        0.0,
                        total_gas_sc,
                    );

                    if sg <= 1e-9 {
                        self.cells[idx].regime = HydrocarbonState::Undersaturated;
                        self.cells[idx].hydrocarbon_var = rs_resolved.max(0.0);
                    } else {
                        self.cells[idx].regime = HydrocarbonState::Saturated;
                        self.cells[idx].hydrocarbon_var = sg;
                    }
                }
            }
        }
    }

    fn enforce_cell_bounds(&mut self, sim: &ReservoirSimulator, idx: usize) {
        let cell = &mut self.cells[idx];
        cell.pressure_bar = cell.pressure_bar.max(1e-6);

        if sim.three_phase_mode {
            if let Some(scal) = &sim.scal_3p {
                let oil_floor_no_gas = scal.s_or.max(0.0);
                cell.sw = cell
                    .sw
                    .clamp(scal.s_wc, (1.0 - oil_floor_no_gas).max(scal.s_wc));

                match cell.regime {
                    HydrocarbonState::Saturated => {
                        let oil_floor_with_gas = scal.s_org.max(scal.s_or).max(0.0);
                        let sw_max = (1.0 - oil_floor_with_gas).max(scal.s_wc);
                        cell.sw = cell.sw.min(sw_max);
                        let max_sg = (1.0 - cell.sw - oil_floor_with_gas).max(0.0);
                        cell.hydrocarbon_var = cell.hydrocarbon_var.clamp(0.0, max_sg);
                    }
                    HydrocarbonState::Undersaturated => {
                        cell.hydrocarbon_var = cell.hydrocarbon_var.max(0.0);
                    }
                }
                return;
            }
        }

        let oil_floor = sim.scal.s_or.max(0.0);
        cell.sw = cell
            .sw
            .clamp(sim.scal.s_wc, (1.0 - oil_floor).max(sim.scal.s_wc));
        match cell.regime {
            HydrocarbonState::Saturated => {
                let max_sg = (1.0 - cell.sw - oil_floor).max(0.0);
                cell.hydrocarbon_var = cell.hydrocarbon_var.clamp(0.0, max_sg);
            }
            HydrocarbonState::Undersaturated => {
                cell.hydrocarbon_var = cell.hydrocarbon_var.max(0.0);
            }
        }
    }

    fn enforce_control_bounds(&mut self, sim: &ReservoirSimulator) {
        let topology = build_well_topology(sim);
        let pressure_upper = self
            .cells
            .iter()
            .map(|cell| cell.pressure_bar)
            .fold(sim.well_bhp_max.max(1.0), f64::max)
            + 500.0;

        for (well_idx, bhp_bar) in self.well_bhp.iter_mut().enumerate() {
            let control = physical_well_control(sim, &topology, well_idx);
            if topology.wells[well_idx].injector {
                *bhp_bar = bhp_bar.clamp(1e-6, pressure_upper.max(control.bhp_limit.max(sim.well_bhp_max)));
            } else {
                *bhp_bar = bhp_bar.clamp(control.bhp_limit.min(sim.well_bhp_min).max(1e-6), pressure_upper);
            }
        }
    }

    pub(crate) fn apply_newton_update(
        &self,
        sim: &ReservoirSimulator,
        update: &DVector<f64>,
        damping: f64,
    ) -> Self {
        let mut next = self.clone();

        for (idx, cell) in next.cells.iter_mut().enumerate() {
            let offset = idx * 3;
            cell.pressure_bar += damping * update[offset];
            cell.sw += damping * update[offset + 1];
            cell.hydrocarbon_var += damping * update[offset + 2];
        }

        for well_idx in 0..self.n_well_unknowns() {
            let offset = self.well_bhp_unknown_offset(well_idx);
            next.well_bhp[well_idx] += damping * update[offset];
        }

        for perf_idx in 0..self.n_perforation_unknowns() {
            let offset = self.perforation_rate_unknown_offset(perf_idx);
            next.perforation_rates_m3_day[perf_idx] += damping * update[offset];
        }

        for idx in 0..next.cells.len() {
            next.enforce_cell_bounds(sim, idx);
        }
        next.enforce_control_bounds(sim);

        next.classify_regimes(sim);

        for idx in 0..next.cells.len() {
            next.enforce_cell_bounds(sim, idx);
        }
        next.enforce_control_bounds(sim);

        next
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
        }) && self.well_bhp.iter().all(|bhp_bar| bhp_bar.is_finite())
            && self
                .perforation_rates_m3_day
                .iter()
                .all(|rate| rate.is_finite())
    }

    pub(crate) fn respects_basic_bounds(&self, sim: &ReservoirSimulator) -> bool {
        let topology = build_well_topology(sim);
        let pressure_upper = self
            .cells
            .iter()
            .map(|cell| cell.pressure_bar)
            .fold(sim.well_bhp_max.max(1.0), f64::max)
            + 500.0;

        self.cells.iter().enumerate().all(|(idx, cell)| {
            let derived = self.derive_cell(sim, idx);
            let oil_floor = if sim.three_phase_mode {
                sim.scal_3p
                    .as_ref()
                    .map(|scal| {
                        if derived.sg > 1e-9 {
                            scal.s_org.max(scal.s_or)
                        } else {
                            scal.s_or
                        }
                    })
                    .unwrap_or(sim.scal.s_or)
            } else {
                sim.scal.s_or
            };
            cell.sw >= -1e-9
                && cell.sw >= sim.scal.s_wc - 1e-9
                && cell.sw <= 1.0 + 1e-9
                && derived.sg >= -1e-9
                && derived.so >= oil_floor - 1e-9
                && derived.so <= 1.0 + 1e-9
                && derived.sg <= 1.0 + 1e-9
                && (cell.sw + derived.so + derived.sg - 1.0).abs() < 1e-6
        }) && self.well_bhp.iter().enumerate().all(|(well_idx, bhp_bar)| {
            let control = physical_well_control(sim, &topology, well_idx);
            if topology.wells[well_idx].injector {
                *bhp_bar >= 1e-6 - 1e-9 && *bhp_bar <= pressure_upper + 1e-9
            } else {
                *bhp_bar >= control.bhp_limit.min(sim.well_bhp_min).max(1e-6) - 1e-9
                    && *bhp_bar <= pressure_upper + 1e-9
            }
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

        let topology = build_well_topology(sim);
        for perforation in topology.perforations {
            let bhp_bar = self.well_bhp[perforation.physical_well_index];
            sim.wells[perforation.well_entry_index].bhp = bhp_bar;
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
            well_bhp: Vec::new(),
            perforation_rates_m3_day: Vec::new(),
        };

        let derived = state.derive_cell(&sim, 0);
        assert!((derived.sg - 0.1).abs() < 1e-12);
        assert!((derived.so - 0.7).abs() < 1e-12);
        assert!(derived.bo > 0.0);
        assert!(derived.bg > 0.0);
    }

    #[test]
    fn classify_regimes_preserves_gas_inventory_when_undersaturated_state_exceeds_rs_sat() {
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
        let mut state = FimState {
            cells: vec![FimCellState {
                pressure_bar: 150.0,
                sw: 0.2,
                hydrocarbon_var: 30.0,
                regime: HydrocarbonState::Undersaturated,
            }],
            well_bhp: Vec::new(),
            perforation_rates_m3_day: Vec::new(),
        };

        let pore_volume_m3 = sim.pore_volume_m3(0);
        let bo_before = sim.get_b_o_for_rs(150.0, 30.0);
        let gas_before_sc = (1.0 - 0.2) * pore_volume_m3 * 30.0 / bo_before;

        state.classify_regimes(&sim);
        assert_eq!(state.cells[0].regime, HydrocarbonState::Saturated);

        let derived = state.derive_cell(&sim, 0);
        let gas_after_sc = pore_volume_m3 * derived.sg / derived.bg
            + pore_volume_m3 * derived.so * derived.rs / derived.bo;

        assert!((gas_after_sc - gas_before_sc).abs() < 1e-6);
        assert!(derived.sg > 0.0);
    }

    #[test]
    fn from_simulator_initializes_rate_control_group_bhps() {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_rate_controlled_wells(true);
        sim.add_well(0, 0, 0, 400.0, 0.1, 0.0, true).unwrap();
        sim.add_well(1, 0, 0, 50.0, 0.1, 0.0, false).unwrap();

        let state = FimState::from_simulator(&sim);
        assert_eq!(state.n_well_unknowns(), 2);
        assert_eq!(state.n_perforation_unknowns(), 2);
        assert_eq!(state.n_unknowns(), state.n_cell_unknowns() + 4);
    }
}
