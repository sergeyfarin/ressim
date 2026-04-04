use nalgebra::DVector;

use crate::well_control::ResolvedWellControl;
use crate::{InjectedFluid, ReservoirSimulator};

impl ReservoirSimulator {
    fn accumulate_well_source_deltas(
        &self,
        p_new: &DVector<f64>,
        well_controls: &[Option<ResolvedWellControl>],
    ) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let n_cells = self.nx * self.ny * self.nz;
        let mut delta_water_m3 = vec![0.0; n_cells];
        let mut delta_free_gas_sc = vec![0.0; n_cells];
        let mut delta_dg_sc = vec![0.0; n_cells];

        for (w_idx, w) in self.wells.iter().enumerate() {
            let id = self.idx(w.i, w.j, w.k);
            if let Some(control) = well_controls[w_idx] {
                if let Some(q_m3_day) = self.well_transport_rate_from_control(w, control, p_new[id])
                {
                    if self.three_phase_mode {
                        let (fw, fg, fo) = if w.injector {
                            match self.injected_fluid {
                                InjectedFluid::Water => (1.0, 0.0, 0.0),
                                InjectedFluid::Gas => (0.0, 1.0, 0.0),
                            }
                        } else {
                            let producer_state = self.producer_control_state_from_resolved_control(
                                w,
                                control,
                                &self.pressure,
                            );
                            (
                                producer_state.water_fraction,
                                producer_state.gas_fraction,
                                producer_state.oil_fraction,
                            )
                        };
                        delta_water_m3[id] -= q_m3_day * fw;
                        let producer_state = if w.injector {
                            None
                        } else {
                            Some(self.producer_control_state_from_resolved_control(
                                w,
                                control,
                                &self.pressure,
                            ))
                        };
                        delta_free_gas_sc[id] -= q_m3_day * fg
                            / producer_state
                                .map(|state| state.gas_fvf)
                                .unwrap_or_else(|| self.get_b_g(p_new[id]).max(1e-9));

                        if !w.injector && self.pvt_table.is_some() {
                            let producer_state = producer_state
                                .expect("producer state should exist for producer controls");
                            let q_o_res = q_m3_day * fo;
                            let q_o_sc = q_o_res / producer_state.oil_fvf;
                            delta_dg_sc[id] -= q_o_sc * producer_state.rs_sm3_sm3;
                        }
                    } else {
                        let fw = if w.injector {
                            1.0
                        } else {
                            self.frac_flow_water(id)
                        };
                        delta_water_m3[id] -= q_m3_day * fw;
                    }
                }
            }
        }

        (delta_water_m3, delta_free_gas_sc, delta_dg_sc)
    }

    fn has_active_injector_completion(
        &self,
        cell_idx: usize,
        well_controls: &[Option<ResolvedWellControl>],
    ) -> bool {
        self.wells.iter().enumerate().any(|(w_idx, w)| {
            w.injector
                && well_controls
                    .get(w_idx)
                    .and_then(|control| *control)
                    .is_some()
                && self.idx(w.i, w.j, w.k) == cell_idx
        })
    }

    fn apply_three_phase_deltas_to_cell(
        &self,
        idx: usize,
        state_pressure_bar: f64,
        target_pressure_bar: f64,
        sw_old: f64,
        so_old: f64,
        sg_old: f64,
        rs_old: f64,
        delta_water_m3: f64,
        delta_free_gas_sc: f64,
        delta_dg_sc: f64,
    ) -> (f64, f64, f64, f64) {
        let vp_m3 = self.pore_volume_m3(idx);
        let delta_sw = delta_water_m3 / vp_m3;
        let bg_old = self.get_b_g(state_pressure_bar).max(1e-9);
        let bo_old = if let Some(table) = &self.pvt_table {
            if self.three_phase_mode {
                let (bo, _) = table.interpolate_oil(state_pressure_bar, rs_old);
                bo.max(1e-9)
            } else {
                table.interpolate(state_pressure_bar).bo_m3m3.max(1e-9)
            }
        } else {
            self.b_o.max(1e-9)
        };
        let old_free_gas_sc = sg_old * vp_m3 / bg_old;
        let old_dissolved_gas_sc = if self.pvt_table.is_some() {
            (so_old * vp_m3 / bo_old) * rs_old
        } else {
            0.0
        };

        let (s_wc, s_or, s_gc, s_gr) = if let Some(s) = &self.scal_3p {
            (s.s_wc, s.s_or, s.s_gc, s.s_gr)
        } else {
            (self.scal.s_wc, self.scal.s_or, 0.0, 0.0)
        };

        let sw_new = (sw_old + delta_sw).clamp(s_wc, 1.0 - s_or - s_gc);
        let bg_new = self.get_b_g(target_pressure_bar).max(1e-9);
        let transported_free_gas_sc = (old_free_gas_sc + delta_free_gas_sc).max(0.0);
        let mut sg_new = ((transported_free_gas_sc * bg_new) / vp_m3).clamp(0.0, 1.0 - s_wc - s_gr);
        let mut rs_new = rs_old;

        if self.pvt_table.is_some() {
            let dissolved_gas_sc_transport = (old_dissolved_gas_sc + delta_dg_sc).max(0.0);
            let (sg_resolved, _so_resolved, rs_cell) = self.split_gas_inventory_after_transport(
                target_pressure_bar,
                vp_m3,
                sw_new,
                transported_free_gas_sc,
                dissolved_gas_sc_transport,
                if self.gas_redissolution_enabled {
                    None
                } else {
                    Some(rs_old)
                },
            );
            sg_new = sg_resolved;
            rs_new = rs_cell;
        }

        let sg_new = sg_new.clamp(0.0, (1.0 - sw_new).max(0.0));
        let so_new = (1.0 - sw_new - sg_new).max(0.0);
        (sw_new, sg_new, so_new, rs_new)
    }

    pub(crate) fn update_saturations_and_pressure(
        &mut self,
        p_new: &DVector<f64>,
        delta_water_m3: &Vec<f64>,
        delta_free_gas_sc: &Vec<f64>,
        delta_dg_sc: &Vec<f64>,
        well_controls: &[Option<ResolvedWellControl>],
        dt_days: f64,
    ) {
        let n_cells = self.nx * self.ny * self.nz;
        let (well_source_water_m3_day, well_source_free_gas_sc_day, well_source_dg_sc_day) =
            self.accumulate_well_source_deltas(p_new, well_controls);
        let mut actual_change_m3 = 0.0;
        let mut actual_oil_removed_sc = 0.0;
        let mut actual_change_gas_sc = 0.0;
        for idx in 0..n_cells {
            let vp_m3 = self.pore_volume_m3(idx);
            if vp_m3 <= 0.0 {
                continue;
            }

            let sw_old = self.sat_water[idx];
            let delta_sw = delta_water_m3[idx] / vp_m3;

            if self.three_phase_mode {
                let sg_old = self.sat_gas[idx];
                let so_old = self.sat_oil[idx];
                let p_old = self.pressure[idx];
                let bg_old = self.get_b_g(p_old).max(1e-9);
                let bo_old = self.get_b_o_cell(idx, p_old).max(1e-9);
                let rs_old = self.rs[idx];
                let old_oil_sc = so_old * vp_m3 / bo_old;
                let old_free_gas_sc = sg_old * vp_m3 / bg_old;
                let old_dissolved_gas_sc = if self.pvt_table.is_some() {
                    old_oil_sc * rs_old
                } else {
                    0.0
                };

                let (sw_new, sg_new, so_new, rs_new) = if self
                    .has_active_injector_completion(idx, well_controls)
                {
                    let source_water_m3 = well_source_water_m3_day[idx] * dt_days;
                    let source_free_gas_sc = well_source_free_gas_sc_day[idx] * dt_days;
                    let source_dg_sc = well_source_dg_sc_day[idx] * dt_days;
                    let (sw_mid, sg_mid, so_mid, rs_mid) = self.apply_three_phase_deltas_to_cell(
                        idx,
                        p_old,
                        p_new[idx],
                        sw_old,
                        so_old,
                        sg_old,
                        rs_old,
                        source_water_m3,
                        source_free_gas_sc,
                        source_dg_sc,
                    );
                    self.apply_three_phase_deltas_to_cell(
                        idx,
                        p_new[idx],
                        p_new[idx],
                        sw_mid,
                        so_mid,
                        sg_mid,
                        rs_mid,
                        delta_water_m3[idx] - source_water_m3,
                        delta_free_gas_sc[idx] - source_free_gas_sc,
                        delta_dg_sc[idx] - source_dg_sc,
                    )
                } else {
                    self.apply_three_phase_deltas_to_cell(
                        idx,
                        p_old,
                        p_new[idx],
                        sw_old,
                        so_old,
                        sg_old,
                        rs_old,
                        delta_water_m3[idx],
                        delta_free_gas_sc[idx],
                        delta_dg_sc[idx],
                    )
                };

                self.sat_water[idx] = sw_new;
                self.sat_gas[idx] = sg_new;
                self.sat_oil[idx] = so_new;
                self.rs[idx] = rs_new;

                actual_change_m3 += (sw_new - sw_old) * vp_m3;
                let bg_new = self.get_b_g(p_new[idx]).max(1e-9);
                let bo_new = self.get_b_o_cell(idx, p_new[idx]).max(1e-9);
                let new_oil_sc = so_new * vp_m3 / bo_new;
                actual_oil_removed_sc += old_oil_sc - new_oil_sc;
                let new_free_gas_sc = sg_new * vp_m3 / bg_new;
                let new_dissolved_gas_sc = if self.pvt_table.is_some() {
                    new_oil_sc * rs_new
                } else {
                    0.0
                };
                actual_change_gas_sc += (new_free_gas_sc + new_dissolved_gas_sc)
                    - (old_free_gas_sc + old_dissolved_gas_sc);
            } else {
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

                actual_change_m3 += (sw_new - sw_old) * vp_m3;
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
            dt_days,
            actual_change_m3,
            actual_oil_removed_sc,
            actual_change_gas_sc,
        );
        self.time_days += dt_days;
    }
}
