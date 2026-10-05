use nalgebra::DVector;

use crate::ReservoirSimulator;
use crate::impes::pressure::TransportDeltas;
use crate::well_control::ResolvedWellControl;

impl ReservoirSimulator {
    /// Apply one accepted substep: transported masses become the new cell state at `p_new`.
    ///
    /// Both modes add `deltas` to the beginning-of-substep component masses and flash them
    /// at the new pressure, conserving surface water and oil (and gas in three-phase mode).
    pub(crate) fn update_saturations_and_pressure(
        &mut self,
        p_new: &DVector<f64>,
        deltas: &TransportDeltas,
        well_controls: &[Option<ResolvedWellControl>],
        dt_days: f64,
    ) {
        let n_cells = self.nx * self.ny * self.nz;
        // Must be captured before the saturation update below overwrites the state the
        // transport was built from.
        let phase_splits = self.producer_transport_phase_splits(well_controls);
        let mut actual_change_water = 0.0;
        let mut actual_oil_removed_sc = 0.0;
        let mut actual_change_gas_sc = 0.0;
        for idx in 0..n_cells {
            let vp_m3 = self.pore_volume_m3(idx);
            if vp_m3 <= 0.0 {
                continue;
            }

            let old = self.cell_masses(idx);
            let new = deltas.applied_to(idx, &old);
            let flash = self.flash_cell(idx, p_new[idx], &new);
            self.sat_water[idx] = flash.sw;
            self.sat_oil[idx] = flash.so;
            self.sat_gas[idx] = flash.sg;
            self.rs[idx] = flash.rs;
            self.pressure[idx] = p_new[idx];
            let placed = self.cell_masses(idx);
            actual_change_water += placed.water_sc - old.water_sc;
            actual_oil_removed_sc += old.oil_sc - placed.oil_sc;
            actual_change_gas_sc += placed.total_gas_sc() - old.total_gas_sc();
        }

        if !self.three_phase_mode {
            self.sat_gas.fill(0.0);
            self.rs.fill(0.0);
        }

        self.record_step_report(
            well_controls,
            &phase_splits,
            dt_days,
            actual_change_water,
            actual_oil_removed_sc,
            actual_change_gas_sc,
        );
        self.time_days += dt_days;
    }
}
