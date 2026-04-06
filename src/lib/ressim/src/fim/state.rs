use nalgebra::DVector;

use crate::ReservoirSimulator;
use crate::fim::flash::{classify_cell_regime, resolve_cell_flash};
use crate::fim::wells::{
    build_well_topology, perforation_local_block, physical_well_control, well_local_block,
};

const WELL_BHP_MANIFOLD_BLEND: f64 = 0.9;
const WELL_BHP_TRUST_RADIUS_BAR: f64 = 25.0;
const WELL_RATE_MANIFOLD_BLEND: f64 = 0.75;
const WELL_RATE_TRUST_RADIUS_FRAC: f64 = 0.1;
const WELL_RATE_TRUST_RADIUS_MIN_M3_DAY: f64 = 250.0;

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
            let drsdt0_base_rs = if !sim.gas_redissolution_enabled {
                Some(sim.rs[idx])
            } else {
                None
            };
            let regime = classify_cell_regime(
                sim,
                pressure_bar,
                sim.sat_gas[idx],
                sim.rs[idx],
                drsdt0_base_rs,
            );
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
            if let Some((bhp_bar, _)) = well_local_block(&topology, &state, well_idx).solve_bhp_from_target(sim) {
                state.well_bhp[well_idx] = bhp_bar;
            }
        }

        for perf_idx in 0..topology.perforations.len() {
            let perf = perforation_local_block(&topology, &state, perf_idx);
            let well_idx = perf.physical_well_idx();
            let bhp_bar = state.well_bhp[well_idx];
            state.perforation_rates_m3_day[perf_idx] =
                if !physical_well_control(sim, &topology, well_idx).enabled {
                    0.0
                } else {
                    perf.connection_rate_for_bhp(sim, bhp_bar).unwrap_or(0.0)
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

    #[cfg(test)]
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

        // Saturated cells keep any physically required free gas. Undersaturated
        // cells switch as soon as Rs materially exceeds Rs_sat so excess
        // dissolved gas is flashed instead of being silently clamped away.
        const SG_LOWER: f64 = 1e-4;
        const SG_SWITCH_TOL: f64 = 1e-12;
        const RS_SWITCH_TOL: f64 = 1e-6;

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
                    if gas_saturation > SG_LOWER {
                        self.cells[idx].hydrocarbon_var = gas_saturation;
                        continue;
                    }

                    let derived = self.derive_cell(sim, idx);
                    let pore_volume_m3 = sim.pore_volume_m3(idx).max(1e-9);
                    let total_gas_sc = pore_volume_m3 * derived.sg / derived.bg.max(1e-9)
                        + pore_volume_m3 * derived.so * derived.rs / derived.bo.max(1e-9);
                    let (sg, _so, rs_resolved) = sim.split_gas_inventory_after_transport(
                        cell.pressure_bar,
                        pore_volume_m3,
                        cell.sw,
                        0.0,
                        total_gas_sc,
                        if sim.gas_redissolution_enabled {
                            None
                        } else {
                            Some(sim.rs[idx])
                        },
                    );

                    if sg <= SG_SWITCH_TOL {
                        self.cells[idx].regime = HydrocarbonState::Undersaturated;
                        self.cells[idx].hydrocarbon_var = rs_resolved.max(0.0).min(rs_sat);
                    } else {
                        self.cells[idx].regime = HydrocarbonState::Saturated;
                        self.cells[idx].hydrocarbon_var = sg;
                    }
                }
                HydrocarbonState::Undersaturated => {
                    let rs_sm3_sm3 = cell.hydrocarbon_var.max(0.0);
                    if rs_sm3_sm3 <= rs_sat + RS_SWITCH_TOL {
                        self.cells[idx].hydrocarbon_var = rs_sm3_sm3.min(rs_sat);
                        continue;
                    }

                    // Rs exceeded the saturated value: resolve the flash
                    // immediately so excess dissolved gas becomes free gas.
                    let derived = self.derive_cell(sim, idx);

                    if derived.sg <= SG_SWITCH_TOL {
                        self.cells[idx].regime = HydrocarbonState::Undersaturated;
                        self.cells[idx].hydrocarbon_var = derived.rs.max(0.0).min(rs_sat);
                    } else {
                        self.cells[idx].regime = HydrocarbonState::Saturated;
                        self.cells[idx].hydrocarbon_var = derived.sg;
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

    fn enforce_control_bounds(
        &mut self,
        sim: &ReservoirSimulator,
        topology: &crate::fim::wells::FimWellTopology,
    ) {
        let pressure_upper = self
            .cells
            .iter()
            .map(|cell| cell.pressure_bar)
            .fold(sim.well_bhp_max.max(1.0), f64::max)
            + 500.0;

        for (well_idx, bhp_bar) in self.well_bhp.iter_mut().enumerate() {
            let control = physical_well_control(sim, &topology, well_idx);
            if topology.wells[well_idx].injector {
                *bhp_bar = bhp_bar.clamp(
                    1e-6,
                    pressure_upper.max(control.bhp_limit.max(sim.well_bhp_max)),
                );
            } else {
                *bhp_bar = bhp_bar.clamp(
                    control.bhp_limit.min(sim.well_bhp_min).max(1e-6),
                    pressure_upper,
                );
            }
        }
    }

    fn relax_well_state_toward_local_consistency(
        &mut self,
        sim: &ReservoirSimulator,
        topology: &crate::fim::wells::FimWellTopology,
    ) {
        for well_idx in 0..topology.wells.len() {
            let (control, consistent_bhp, perforation_indices) = {
                let block = well_local_block(topology, self, well_idx);
                let control = block.control(sim);
                let consistent_bhp = if !control.enabled {
                    Some(control.bhp_target)
                } else if control.rate_controlled {
                    block.solve_bhp_from_target(sim).map(|(bhp_bar, _)| bhp_bar)
                } else {
                    Some(control.bhp_target)
                };
                let perforation_indices = block.perforation_indices().to_vec();
                (control, consistent_bhp, perforation_indices)
            };

            let Some(consistent_bhp) = consistent_bhp else {
                continue;
            };

            let proposed_bhp = self.well_bhp[well_idx];
            let blended_bhp =
                proposed_bhp + WELL_BHP_MANIFOLD_BLEND * (consistent_bhp - proposed_bhp);
            self.well_bhp[well_idx] = (consistent_bhp
                + (blended_bhp - consistent_bhp)
                    .clamp(-WELL_BHP_TRUST_RADIUS_BAR, WELL_BHP_TRUST_RADIUS_BAR))
            .max(1e-6);

            for perf_idx in perforation_indices {
                let consistent_q = if !control.enabled {
                    0.0
                } else {
                    perforation_local_block(topology, self, perf_idx)
                        .connection_rate_for_bhp(sim, self.well_bhp[well_idx])
                        .unwrap_or(0.0)
                };
                let proposed_q = self.perforation_rates_m3_day[perf_idx];
                let blended_q = proposed_q + WELL_RATE_MANIFOLD_BLEND * (consistent_q - proposed_q);
                let trust_radius = (WELL_RATE_TRUST_RADIUS_FRAC * consistent_q.abs())
                    .max(WELL_RATE_TRUST_RADIUS_MIN_M3_DAY);
                let q =
                    consistent_q + (blended_q - consistent_q).clamp(-trust_radius, trust_radius);
                self.perforation_rates_m3_day[perf_idx] = if !control.enabled {
                    0.0
                } else if topology.wells[well_idx].injector {
                    q.min(0.0)
                } else {
                    q.max(0.0)
                };
            }
        }
    }

    /// Apply Newton update with regime reclassification (for use outside Newton loop).
    #[cfg(test)]
    pub(crate) fn apply_newton_update(
        &self,
        sim: &ReservoirSimulator,
        update: &DVector<f64>,
        damping: f64,
    ) -> Self {
        let topology = build_well_topology(sim);
        let mut next = self.apply_raw_update(sim, update, damping, &topology, false);
        next.classify_regimes(sim);
        for idx in 0..next.cells.len() {
            next.enforce_cell_bounds(sim, idx);
        }
        next
    }

    /// Apply Newton update WITHOUT regime reclassification — keeps the regime map
    /// frozen so the Jacobian stays smooth within a Newton solve.
    pub(crate) fn apply_newton_update_frozen(
        &self,
        sim: &ReservoirSimulator,
        update: &DVector<f64>,
        damping: f64,
        topology: &crate::fim::wells::FimWellTopology,
    ) -> Self {
        self.apply_raw_update(sim, update, damping, topology, true)
    }

    fn apply_raw_update(
        &self,
        sim: &ReservoirSimulator,
        update: &DVector<f64>,
        damping: f64,
        topology: &crate::fim::wells::FimWellTopology,
        relax_well_state: bool,
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
        next.enforce_control_bounds(sim, topology);
        if relax_well_state {
            next.relax_well_state_toward_local_consistency(sim, topology);
            next.enforce_control_bounds(sim, topology);
        }

        next
    }

    pub(crate) fn derive_cell(&self, sim: &ReservoirSimulator, idx: usize) -> FimCellDerived {
        let cell = self.cell(idx);
        let drsdt0_base_rs = if !sim.gas_redissolution_enabled {
            Some(sim.rs[idx])
        } else {
            None
        };
        let flash = resolve_cell_flash(
            sim,
            cell.pressure_bar,
            cell.sw,
            cell.hydrocarbon_var,
            cell.regime,
            drsdt0_base_rs,
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
        // Lightweight check — no PVT flash or topology rebuild.
        // apply_newton_update already enforced bounds and classified regimes,
        // so we just verify the state hasn't gone numerically wild.
        let oil_floor = if sim.three_phase_mode {
            sim.scal_3p
                .as_ref()
                .map(|scal| scal.s_or.max(0.0))
                .unwrap_or(sim.scal.s_or.max(0.0))
        } else {
            sim.scal.s_or.max(0.0)
        };

        self.cells.iter().all(|cell| {
            let (sg, so) = match cell.regime {
                HydrocarbonState::Saturated => {
                    let sg = cell.hydrocarbon_var;
                    (sg, 1.0 - cell.sw - sg)
                }
                HydrocarbonState::Undersaturated => (0.0, 1.0 - cell.sw),
            };
            cell.pressure_bar >= 1e-6
                && cell.sw >= sim.scal.s_wc - 1e-9
                && cell.sw <= 1.0 + 1e-9
                && sg >= -1e-9
                && so >= oil_floor - 1e-9
                && so <= 1.0 + 1e-9
        }) && self
            .well_bhp
            .iter()
            .all(|bhp_bar| *bhp_bar >= 1e-6 - 1e-9 && *bhp_bar <= 50_000.0)
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
    use nalgebra::DVector;

    use crate::ReservoirSimulator;
    use crate::pvt::{PvtRow, PvtTable};

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
    fn classify_regimes_hysteresis_keeps_saturated_near_zero_gas() {
        // A saturated cell with tiny but physically required free gas should
        // remain saturated instead of silently dropping that gas inventory.
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

        // Sg = 1e-3 is above the 1e-4 hysteresis band — should stay Saturated.
        let mut state = FimState {
            cells: vec![FimCellState {
                pressure_bar: 150.0,
                sw: 0.2,
                hydrocarbon_var: 1e-3,
                regime: HydrocarbonState::Saturated,
            }],
            well_bhp: Vec::new(),
            perforation_rates_m3_day: Vec::new(),
        };
        state.classify_regimes(&sim);
        assert_eq!(state.cells[0].regime, HydrocarbonState::Saturated);

        // Even at very small free-gas saturation, the transition should not
        // discard gas inventory just because the cell is near the zero-gas line.
        state.cells[0].hydrocarbon_var = 1e-5;
        state.cells[0].regime = HydrocarbonState::Saturated;
        state.classify_regimes(&sim);
        assert_eq!(state.cells[0].regime, HydrocarbonState::Saturated);
        assert!(state.cells[0].hydrocarbon_var > 0.0);
    }

    #[test]
    fn apply_newton_update_frozen_limits_well_overshoot_toward_local_consistency() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, 500.0, 0.1, 0.0, true).unwrap();
        sim.injector_rate_controlled = false;

        let topology = build_well_topology(&sim);
        let state = FimState::from_simulator(&sim);
        let mut update = DVector::zeros(state.n_unknowns());
        update[state.well_bhp_unknown_offset(0)] = 400.0;
        update[state.perforation_rate_unknown_offset(0)] = 100_000.0;

        let updated = state.apply_newton_update_frozen(&sim, &update, 1.0, &topology);
        let consistent_q =
            perforation_local_block(&topology, &updated, 0).connection_rate_for_bhp(&sim, updated.well_bhp[0]).unwrap();
        let trust_radius = (WELL_RATE_TRUST_RADIUS_FRAC * consistent_q.abs())
            .max(WELL_RATE_TRUST_RADIUS_MIN_M3_DAY);

        assert!((updated.well_bhp[0] - 500.0).abs() <= WELL_BHP_TRUST_RADIUS_BAR + 1e-9);
        assert!(updated.perforation_rates_m3_day[0] <= 0.0);
        assert!((updated.perforation_rates_m3_day[0] - consistent_q).abs() <= trust_radius + 1e-9);
    }

    #[test]
    fn classify_regimes_switches_immediately_when_rs_exceeds_rs_sat() {
        // Once Rs exceeds Rs_sat, even slightly, the excess gas should be
        // flashed instead of being clamped away.
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_mode_enabled(true);
        sim.set_gas_redissolution_enabled(false);
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

        // At p=150, Rs_sat=15. Rs=15 is exactly at bubble point — should stay Undersaturated.
        let mut state = FimState {
            cells: vec![FimCellState {
                pressure_bar: 150.0,
                sw: 0.2,
                hydrocarbon_var: 15.0,
                regime: HydrocarbonState::Undersaturated,
            }],
            well_bhp: Vec::new(),
            perforation_rates_m3_day: Vec::new(),
        };
        state.classify_regimes(&sim);
        assert_eq!(state.cells[0].regime, HydrocarbonState::Undersaturated);
        assert!(state.cells[0].hydrocarbon_var <= 15.0 + 1e-12);

        let pore_volume_m3 = sim.pore_volume_m3(0);
        let bo_before = sim.get_b_o_for_rs(150.0, 15.01);
        let gas_before_sc = (1.0 - 0.2) * pore_volume_m3 * 15.01 / bo_before;

        // Rs = 15.01 is only 0.067% above Rs_sat=15. The old 1% hysteresis
        // would clamp this back to Rs_sat and lose gas inventory.
        state.cells[0].hydrocarbon_var = 15.01;
        state.cells[0].regime = HydrocarbonState::Undersaturated;
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
