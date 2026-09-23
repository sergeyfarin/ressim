use nalgebra::DVector;

use crate::ReservoirSimulator;
use crate::impes::pressure::TransportDeltas;
use crate::well_control::ResolvedWellControl;

impl ReservoirSimulator {
    /// Apply one accepted substep: transported masses become the new cell state at `p_new`.
    ///
    /// Three-phase mode adds `deltas` to each cell's beginning-of-substep masses and flashes
    /// them at the new pressure ([`flash_cell`](ReservoirSimulator::flash_cell)), so water, oil
    /// and gas are all conserved exactly. Two-phase mode still moves water by volume and keeps
    /// oil as the residual `1 − Sw`.
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

            if self.three_phase_mode {
                let old = self.cell_masses(idx);
                let new = deltas.applied_to(idx, &old);
                let flash = self.flash_cell(idx, p_new[idx], &new);

                self.sat_water[idx] = flash.sw;
                self.sat_oil[idx] = flash.so;
                self.sat_gas[idx] = flash.sg;
                self.rs[idx] = flash.rs;
                self.pressure[idx] = p_new[idx];
                // Account with the masses the stored state now holds, which is what the next
                // substep reads back. They differ from `new` only where the flash floored a
                // negative mass at zero, the one place a component can be created.
                let placed = self.cell_masses(idx);

                actual_change_water += placed.water_sc - old.water_sc;
                actual_oil_removed_sc += old.oil_sc - placed.oil_sc;
                actual_change_gas_sc += placed.total_gas_sc() - old.total_gas_sc();
            } else {
                let sw_old = self.sat_water[idx];
                let delta_sw = deltas.water_m3[idx] / vp_m3;
                let sw_min = self.scal.s_wc;
                let sw_max = 1.0 - self.scal.s_or;
                let p_old = self.pressure[idx];
                let so_old = self.sat_oil[idx];
                let bo_old = self.get_b_o_cell(idx, p_old).max(1e-9);
                let old_oil_sc = so_old * vp_m3 / bo_old;
                let sw_new = (sw_old + delta_sw).clamp(sw_min, sw_max);
                let so_new = 1.0 - sw_new;
                let bo_new = self.get_b_o_cell(idx, p_new[idx]).max(1e-9);
                let new_oil_sc = so_new * vp_m3 / bo_new;

                actual_change_water += (sw_new - sw_old) * vp_m3;
                actual_oil_removed_sc += old_oil_sc - new_oil_sc;
                self.sat_water[idx] = sw_new;
                self.sat_oil[idx] = so_new;
                self.sat_gas[idx] = 0.0;
                self.rs[idx] = 0.0;
            }

            self.pressure[idx] = p_new[idx];
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
