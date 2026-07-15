use nalgebra::DVector;

use crate::ReservoirSimulator;
use crate::fim::flash::{classify_cell_regime, resolve_cell_flash};
use crate::fim::flow_resv::FlowResvReportStepContext;
use crate::fim::wells::{
    build_well_topology, connection_rate_for_bhp, perforation_local_block, physical_well_control,
    well_local_block,
};

/// Which well-state post-processing `apply_raw_update` applies after the raw Newton update.
/// `docs/FIM_BUNDLE_W_PLAN.md` §5 item 1: Bundle W's `NestedSolve` replaces `Relax` as a
/// drop-in at the single call site (`apply_newton_update_frozen`), flag-gated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum WellStateUpdateMode {
    /// No well-state post-processing (test-only path, `apply_newton_update`).
    None,
    /// Legacy: `relax_well_state_toward_local_consistency` (blend + trust radius).
    Relax,
    /// Bundle W: converged per-well inner Newton solve (`fim/wells_inner.rs`).
    NestedSolve,
    /// G4b2's one-perf Flow RESV route: update the selected surface-rate primary directly and
    /// never feed it to the historical q-coordinate relaxer/inner solve.
    FlowResv(FlowResvReportStepContext),
}

const WELL_BHP_MANIFOLD_BLEND: f64 = 0.9;
const WELL_BHP_TRUST_RADIUS_BAR: f64 = 25.0;
const WELL_RATE_MANIFOLD_BLEND: f64 = 0.75;
const WELL_RATE_TRUST_RADIUS_FRAC: f64 = 0.1;
const WELL_RATE_TRUST_RADIUS_MIN_M3_DAY: f64 = 250.0;
const OPM_PRIMARY_VARIABLE_OSCILLATION_THRESHOLD: f64 = 1e-5;

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

/// Physical meaning of an unchanged-layout perforation-tail slot.  The numerical vector remains
/// contiguous for the linear block layout, but consumers must resolve its meaning through
/// `FimState` rather than assuming every tail value is a reservoir connection rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FimPerforationPrimaryKind {
    ReservoirConnectionQ,
    FlowResvGasSurfaceU,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FimPerforationPrimary {
    pub(crate) kind: FimPerforationPrimaryKind,
    pub(crate) value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FimState {
    pub(crate) cells: Vec<FimCellState>,
    pub(crate) well_bhp: Vec<f64>,
    pub(crate) perforation_rates_m3_day: Vec<f64>,
    pub(crate) perforation_primary_kinds: Vec<FimPerforationPrimaryKind>,
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
            perforation_primary_kinds: vec![
                FimPerforationPrimaryKind::ReservoirConnectionQ;
                topology.perforations.len()
            ],
        };

        for well_idx in 0..topology.wells.len() {
            if let Some((bhp_bar, _)) =
                well_local_block(&topology, &state, well_idx).solve_bhp_from_target(sim)
            {
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

    /// Convert the already-created historical tail into G4's scoped positive surface-rate
    /// primary before the first Newton assembly. The stored tail slot keeps its existing matrix
    /// position; every RESV route consumer is selected by the immutable context and must treat
    /// this value as `u`, never as q.
    pub(crate) fn initialize_flow_resv_gas_primary(
        &mut self,
        sim: &ReservoirSimulator,
        topology: &crate::fim::wells::FimWellTopology,
        context: FlowResvReportStepContext,
    ) -> Result<(), String> {
        let bg_ref = context.reference.bg_rm3_per_sm3;
        let target = context.reservoir_target_rm3_day;
        if !(bg_ref.is_finite() && bg_ref > 0.0 && target.is_finite() && target > 0.0) {
            return Err("requires finite positive RESV target and reference B_g".to_string());
        }
        let perf_idx = context.perforation_idx;
        let well_idx = context.physical_well_idx;
        if topology
            .wells
            .get(well_idx)
            .map(|well| well.perforation_indices.as_slice())
            != Some(&[perf_idx])
        {
            return Err("RESV context/topology identity changed before Newton".to_string());
        }

        // The local connection law is monotone for the validated active injector. Solve the
        // report-start reservoir rate directly instead of reusing the historical RESV->BHP
        // fall-through target.
        let cell_pressure = self.cells[topology.perforations[perf_idx].cell_index].pressure_bar;
        let desired_q = -target;
        let mut low = cell_pressure;
        let mut high = cell_pressure + 100.0;
        for _ in 0..32 {
            let q = connection_rate_for_bhp(sim, self, topology, perf_idx, high)
                .ok_or_else(|| "RESV initial connection is not finite".to_string())?;
            if q <= desired_q {
                break;
            }
            high = cell_pressure + 2.0 * (high - cell_pressure);
        }
        let q_high = connection_rate_for_bhp(sim, self, topology, perf_idx, high)
            .ok_or_else(|| "RESV initial connection is not finite".to_string())?;
        if q_high > desired_q {
            return Err("could not bracket RESV initial BHP".to_string());
        }
        for _ in 0..80 {
            let mid = 0.5 * (low + high);
            let q = connection_rate_for_bhp(sim, self, topology, perf_idx, mid)
                .ok_or_else(|| "RESV initial connection is not finite".to_string())?;
            if q > desired_q {
                low = mid;
            } else {
                high = mid;
            }
        }
        self.well_bhp[well_idx] = 0.5 * (low + high);
        self.perforation_rates_m3_day[perf_idx] = target / bg_ref;
        self.perforation_primary_kinds[perf_idx] = FimPerforationPrimaryKind::FlowResvGasSurfaceU;
        Ok(())
    }

    pub(crate) fn perforation_primary(&self, perf_idx: usize) -> FimPerforationPrimary {
        FimPerforationPrimary {
            kind: self.perforation_primary_kinds[perf_idx],
            value: self.perforation_rates_m3_day[perf_idx],
        }
    }

    pub(crate) fn perforation_primary_value(&self, perf_idx: usize) -> f64 {
        self.perforation_primary(perf_idx).value
    }

    pub(crate) fn reservoir_connection_q(&self, perf_idx: usize) -> Option<f64> {
        (self.perforation_primary(perf_idx).kind == FimPerforationPrimaryKind::ReservoirConnectionQ)
            .then(|| self.perforation_primary_value(perf_idx))
    }

    pub(crate) fn flow_resv_surface_u(&self, perf_idx: usize) -> Option<f64> {
        (self.perforation_primary(perf_idx).kind == FimPerforationPrimaryKind::FlowResvGasSurfaceU)
            .then(|| self.perforation_primary_value(perf_idx))
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
        let mut next = self.apply_raw_update(
            sim,
            update,
            damping,
            &topology,
            WellStateUpdateMode::None,
            true,
        );
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
        well_update_mode: WellStateUpdateMode,
    ) -> Self {
        self.apply_raw_update(sim, update, damping, topology, well_update_mode, true)
    }

    /// Historical Y2b2b raw-state view retained for boundary/unit comparisons.
    ///
    /// The live native/default-off flag now uses
    /// `apply_newton_update_opm_primary_variables`; this narrower helper deliberately omits
    /// primary-variable adaptation and must not be used as a behavior probe.
    pub(crate) fn apply_newton_update_frozen_raw_saturation(
        &self,
        sim: &ReservoirSimulator,
        update: &DVector<f64>,
        damping: f64,
        topology: &crate::fim::wells::FimWellTopology,
        well_update_mode: WellStateUpdateMode,
    ) -> Self {
        self.apply_raw_update(sim, update, damping, topology, well_update_mode, false)
    }

    /// Y2b3a's deck-scoped OPM primary-variable lifecycle.
    ///
    /// The fixed third cell unknown keeps its matrix position but changes meaning atomically
    /// between free-gas saturation (`Sg`) and dissolved-gas ratio (`Rs`). Raw saturation state
    /// is retained, matching the existing native/default-off Y2b probe. Adaptation happens
    /// before well post-processing so reservoir and well consumers observe the same meaning.
    pub(crate) fn apply_newton_update_opm_primary_variables(
        &self,
        sim: &ReservoirSimulator,
        update: &DVector<f64>,
        damping: f64,
        topology: &crate::fim::wells::FimWellTopology,
        well_update_mode: WellStateUpdateMode,
        was_switched: &[bool],
    ) -> (Self, Vec<bool>) {
        assert_eq!(
            was_switched.len(),
            self.cells.len(),
            "OPM primary-variable switch history must have one entry per cell"
        );

        let mut next = self.apply_unknown_update(update, damping);
        for cell in &mut next.cells {
            cell.pressure_bar = cell.pressure_bar.max(1e-6);
        }
        let switched = next.adapt_opm_primary_variables(sim, was_switched);
        next.finish_well_update(sim, topology, well_update_mode);
        (next, switched)
    }

    /// Test-only view of the Newton candidate before ResSim's state projection.
    ///
    /// This intentionally applies the same damped unknown vector as
    /// `apply_raw_update`, but skips cell/control bounds and the optional well
    /// post-processing.  It exists solely for the Y2b OPM-parity audit, where
    /// the distinction between the raw primary-variable state and ResSim's
    /// accepted (bounded) state is the subject under test.  Production Newton
    /// updates must continue through `apply_newton_update_frozen`.
    #[cfg(test)]
    pub(crate) fn apply_unbounded_update_for_audit(
        &self,
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

        next
    }

    fn apply_raw_update(
        &self,
        sim: &ReservoirSimulator,
        update: &DVector<f64>,
        damping: f64,
        topology: &crate::fim::wells::FimWellTopology,
        well_update_mode: WellStateUpdateMode,
        enforce_saturation_bounds: bool,
    ) -> Self {
        let mut next = self.apply_unknown_update(update, damping);

        for idx in 0..next.cells.len() {
            if enforce_saturation_bounds {
                next.enforce_cell_bounds(sim, idx);
            } else {
                // Keep the historical Y2b2 pressure floor and all well/control bounds while
                // deliberately retaining the raw saturation-space Newton proposal.
                next.cells[idx].pressure_bar = next.cells[idx].pressure_bar.max(1e-6);
            }
        }
        next.finish_well_update(sim, topology, well_update_mode);
        next
    }

    fn apply_unknown_update(&self, update: &DVector<f64>, damping: f64) -> Self {
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

        next
    }

    fn finish_well_update(
        &mut self,
        sim: &ReservoirSimulator,
        topology: &crate::fim::wells::FimWellTopology,
        well_update_mode: WellStateUpdateMode,
    ) {
        self.enforce_control_bounds(sim, topology);
        match well_update_mode {
            WellStateUpdateMode::None => {}
            WellStateUpdateMode::Relax => {
                self.relax_well_state_toward_local_consistency(sim, topology);
                self.enforce_control_bounds(sim, topology);
            }
            WellStateUpdateMode::NestedSolve => {
                crate::fim::wells_inner::solve_wells_locally(
                    sim,
                    self,
                    topology,
                    &crate::fim::wells_inner::FimWellInnerSolveOptions::default(),
                );
                self.enforce_control_bounds(sim, topology);
            }
            WellStateUpdateMode::FlowResv(context) => {
                // The selected slot is surface u, not the historical signed connection q.
                // Preserve normal BHP finite bounds but do not apply q relaxation or FB/BHP
                // control handling to this route.
                self.perforation_rates_m3_day[context.perforation_idx] =
                    self.perforation_rates_m3_day[context.perforation_idx].max(0.0);
            }
        }
    }

    fn adapt_opm_primary_variables(
        &mut self,
        sim: &ReservoirSimulator,
        was_switched: &[bool],
    ) -> Vec<bool> {
        let mut switched = vec![false; self.cells.len()];
        let Some(table) = sim.pvt_table.as_ref().filter(|_| sim.three_phase_mode) else {
            return switched;
        };

        for (idx, cell) in self.cells.iter_mut().enumerate() {
            let eps = if was_switched[idx] {
                OPM_PRIMARY_VARIABLE_OSCILLATION_THRESHOLD
            } else {
                0.0
            };
            let rs_sat = table.interpolate(cell.pressure_bar).rs_m3m3.max(0.0);
            let rs_max = if sim.gas_redissolution_enabled {
                f64::INFINITY
            } else {
                sim.rs[idx].max(0.0)
            };

            match cell.regime {
                HydrocarbonState::Saturated => {
                    let oil_plus_gas_saturation = 1.0 - cell.sw;
                    if cell.hydrocarbon_var < -eps && oil_plus_gas_saturation > 0.0 {
                        cell.regime = HydrocarbonState::Undersaturated;
                        cell.hydrocarbon_var = rs_max.min(rs_sat);
                        switched[idx] = true;
                    }
                }
                HydrocarbonState::Undersaturated => {
                    let appearance_limit = rs_max.min(rs_sat * (1.0 + eps));
                    if cell.hydrocarbon_var > appearance_limit {
                        cell.regime = HydrocarbonState::Saturated;
                        cell.hydrocarbon_var = 0.0;
                        switched[idx] = true;
                    }
                }
            }
        }

        switched
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
    use crate::fim::properties::cell_accumulation_generic;
    use crate::pvt::{PvtRow, PvtTable};

    use super::*;

    fn y2b3_switch_sim() -> ReservoirSimulator {
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
        sim
    }

    fn y2b3_switch_state(
        sim: &ReservoirSimulator,
        regime: HydrocarbonState,
        hydrocarbon_var: f64,
    ) -> FimState {
        let mut state = FimState::from_simulator(sim);
        state.cells[0] = FimCellState {
            pressure_bar: 150.0,
            sw: 0.2,
            hydrocarbon_var,
            regime,
        };
        state
    }

    fn y2b3_apply(
        sim: &ReservoirSimulator,
        state: &FimState,
        dsw: f64,
        dhc: f64,
        was_switched: bool,
    ) -> (FimState, Vec<bool>) {
        let topology = build_well_topology(sim);
        let mut update = DVector::zeros(state.n_unknowns());
        update[1] = dsw;
        update[2] = dhc;
        state.apply_newton_update_opm_primary_variables(
            sim,
            &update,
            1.0,
            &topology,
            WellStateUpdateMode::None,
            &[was_switched],
        )
    }

    #[test]
    fn y2b3_opm_lifecycle_keeps_positive_sg_and_raw_sw_without_mutating_previous_state() {
        let sim = y2b3_switch_sim();
        let state = y2b3_switch_state(&sim, HydrocarbonState::Saturated, 0.1);
        let previous = state.clone();

        let (candidate, switched) = y2b3_apply(&sim, &state, -0.06, 0.0, false);

        assert_eq!(candidate.cells[0].regime, HydrocarbonState::Saturated);
        assert!((candidate.cells[0].hydrocarbon_var - 0.1).abs() < 1e-15);
        assert!((candidate.cells[0].sw - 0.14).abs() < 1e-15);
        assert_eq!(switched, vec![false]);
        assert_eq!(
            state, previous,
            "candidate update must not mutate the prior time level"
        );
        let derived = candidate.derive_cell(&sim, 0);
        assert!(derived.so.is_finite() && derived.sg.is_finite() && derived.rs.is_finite());
        let accumulation = cell_accumulation_generic::<f64>(
            &sim,
            0,
            candidate.cells[0].pressure_bar,
            candidate.cells[0].sw,
            candidate.cells[0].hydrocarbon_var,
            candidate.cells[0].regime,
            None,
            previous.cells[0].pressure_bar,
            previous.cells[0].sw,
            previous.cells[0].hydrocarbon_var,
            previous.cells[0].regime,
        );
        assert!(
            accumulation.iter().any(|value| value.abs() > 0.0),
            "raw endpoint-crossing Sw must remain visible to component accumulation"
        );
    }

    #[test]
    fn y2b3_opm_lifecycle_switches_negative_sg_to_saturated_rs() {
        let sim = y2b3_switch_sim();
        let state = y2b3_switch_state(&sim, HydrocarbonState::Saturated, 0.01);

        let (candidate, switched) = y2b3_apply(&sim, &state, 0.0, -0.02, false);

        assert_eq!(candidate.cells[0].regime, HydrocarbonState::Undersaturated);
        assert!((candidate.cells[0].hydrocarbon_var - 15.0).abs() < 1e-12);
        assert_eq!(switched, vec![true]);
    }

    #[test]
    fn y2b3_opm_lifecycle_honors_rs_max_when_gas_redissolution_is_disabled() {
        let mut sim = y2b3_switch_sim();
        sim.set_gas_redissolution_enabled(false);
        sim.rs[0] = 12.0;
        let state = y2b3_switch_state(&sim, HydrocarbonState::Saturated, 0.01);

        let (candidate, switched) = y2b3_apply(&sim, &state, 0.0, -0.02, false);

        assert_eq!(candidate.cells[0].regime, HydrocarbonState::Undersaturated);
        assert!((candidate.cells[0].hydrocarbon_var - 12.0).abs() < 1e-12);
        assert_eq!(switched, vec![true]);
    }

    #[test]
    fn y2b3_opm_lifecycle_keeps_subsaturated_rs_and_switches_excess_rs_to_zero_sg() {
        let sim = y2b3_switch_sim();
        let below = y2b3_switch_state(&sim, HydrocarbonState::Undersaturated, 14.0);
        let (below_candidate, below_switched) = y2b3_apply(&sim, &below, 0.0, 0.0, false);
        assert_eq!(
            below_candidate.cells[0].regime,
            HydrocarbonState::Undersaturated
        );
        assert!((below_candidate.cells[0].hydrocarbon_var - 14.0).abs() < 1e-12);
        assert_eq!(below_switched, vec![false]);

        let (above_candidate, above_switched) = y2b3_apply(&sim, &below, 0.0, 2.0, false);
        assert_eq!(above_candidate.cells[0].regime, HydrocarbonState::Saturated);
        assert_eq!(above_candidate.cells[0].hydrocarbon_var, 0.0);
        assert_eq!(above_switched, vec![true]);
    }

    #[test]
    fn y2b3_opm_lifecycle_applies_previous_switch_hysteresis_in_both_directions() {
        let sim = y2b3_switch_sim();
        let sg = y2b3_switch_state(&sim, HydrocarbonState::Saturated, 0.0);
        let (without_sg_hysteresis, switched) = y2b3_apply(&sim, &sg, 0.0, -5e-6, false);
        assert_eq!(
            without_sg_hysteresis.cells[0].regime,
            HydrocarbonState::Undersaturated
        );
        assert_eq!(switched, vec![true]);
        let (with_sg_hysteresis, switched) = y2b3_apply(&sim, &sg, 0.0, -5e-6, true);
        assert_eq!(
            with_sg_hysteresis.cells[0].regime,
            HydrocarbonState::Saturated
        );
        assert_eq!(switched, vec![false]);

        let rs = y2b3_switch_state(&sim, HydrocarbonState::Undersaturated, 15.0);
        let (without_rs_hysteresis, switched) = y2b3_apply(&sim, &rs, 0.0, 1e-4, false);
        assert_eq!(
            without_rs_hysteresis.cells[0].regime,
            HydrocarbonState::Saturated
        );
        assert_eq!(switched, vec![true]);
        let (with_rs_hysteresis, switched) = y2b3_apply(&sim, &rs, 0.0, 1e-4, true);
        assert_eq!(
            with_rs_hysteresis.cells[0].regime,
            HydrocarbonState::Undersaturated
        );
        assert_eq!(switched, vec![false]);
    }

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
            perforation_primary_kinds: Vec::new(),
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
            perforation_primary_kinds: Vec::new(),
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
            perforation_primary_kinds: Vec::new(),
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
    fn y2b_audit_view_retains_corrections_discarded_by_cell_bounds() {
        let mut sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        sim.set_three_phase_rel_perm_props(
            0.15, 0.15, 0.05, 0.05, 0.2, 2.0, 2.0, 2.0, 1e-5, 1.0, 0.95,
        )
        .unwrap();
        sim.set_three_phase_mode_enabled(true);
        let topology = build_well_topology(&sim);
        let eps = 1e-6;

        let cases = [
            ("swc", 0.15, 0.10, -eps, 0.0, 0.15, 0.10),
            ("sg_zero", 0.30, 0.0, 0.0, -eps, 0.30, 0.0),
            ("sw_upper", 0.80, 0.0, eps, 0.0, 0.80, 0.0),
            ("sg_upper", 0.15, 0.65, 0.0, eps, 0.15, 0.65),
        ];

        for (label, sw, sg, dsw, dsg, bounded_sw, bounded_sg) in cases {
            let mut state = FimState::from_simulator(&sim);
            state.cells[0].sw = sw;
            state.cells[0].hydrocarbon_var = sg;
            state.cells[0].regime = HydrocarbonState::Saturated;
            let mut update = DVector::zeros(state.n_unknowns());
            update[1] = dsw;
            update[2] = dsg;

            let raw = state.apply_unbounded_update_for_audit(&update, 1.0);
            let bounded = state.apply_newton_update_frozen(
                &sim,
                &update,
                1.0,
                &topology,
                WellStateUpdateMode::None,
            );
            let raw_saturation = state.apply_newton_update_frozen_raw_saturation(
                &sim,
                &update,
                1.0,
                &topology,
                WellStateUpdateMode::None,
            );
            assert!(
                (raw.cells[0].sw - (sw + dsw)).abs() < 1e-15
                    && (raw.cells[0].hydrocarbon_var - (sg + dsg)).abs() < 1e-15,
                "{label}: audit state must retain the raw correction"
            );
            assert!(
                (bounded.cells[0].sw - bounded_sw).abs() < 1e-15
                    && (bounded.cells[0].hydrocarbon_var - bounded_sg).abs() < 1e-15,
                "{label}: production state must retain its existing bound policy"
            );
            assert!(
                (raw_saturation.cells[0].sw - (sw + dsw)).abs() < 1e-15
                    && (raw_saturation.cells[0].hydrocarbon_var - (sg + dsg)).abs() < 1e-15
                    && raw_saturation.cells[0].pressure_bar >= 1e-6,
                "{label}: Y2b2b probe retains raw saturation variables but keeps pressure bounded"
            );
        }
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

        let updated = state.apply_newton_update_frozen(
            &sim,
            &update,
            1.0,
            &topology,
            WellStateUpdateMode::Relax,
        );
        let consistent_q = perforation_local_block(&topology, &updated, 0)
            .connection_rate_for_bhp(&sim, updated.well_bhp[0])
            .unwrap();
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
            perforation_primary_kinds: Vec::new(),
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
