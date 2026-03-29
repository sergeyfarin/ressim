use nalgebra::DVector;
use sprs::{CsMat, TriMatI};
use std::f64;

use crate::solvers::{LinearSolveParams, solve_with_default};
use crate::well_control::{ResolvedWellControl, WellControlDecision};
use crate::{InjectedFluid, ReservoirSimulator};

/// Conversion factor from mD·m²/(m·cP) to m³/day/bar.
const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;

impl ReservoirSimulator {
    pub(crate) fn calculate_fluxes(
        &self,
        delta_t_days: f64,
    ) -> (
        DVector<f64>,
        Vec<f64>,
        Vec<f64>,
        Vec<f64>,
        Vec<Option<ResolvedWellControl>>,
        f64,
        bool,
        usize,
    ) {
        let n_cells = self.nx * self.ny * self.nz;
        if n_cells == 0 {
            return (
                DVector::zeros(0),
                vec![],
                vec![],
                vec![],
                vec![],
                1.0,
                true,
                0,
            );
        }
        let dt_days = delta_t_days.max(1e-12);

        let mut rows: Vec<usize> = Vec::with_capacity(n_cells * 7);
        let mut cols: Vec<usize> = Vec::with_capacity(n_cells * 7);
        let mut vals: Vec<f64> = Vec::with_capacity(n_cells * 7);

        let mut b_rhs = DVector::<f64>::zeros(n_cells);
        let mut diag_inv = DVector::<f64>::zeros(n_cells);

        let well_controls: Vec<Option<ResolvedWellControl>> = self
            .wells
            .iter()
            .map(|w| self.resolve_well_control_for_pressures(w, &self.pressure))
            .collect();

        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i, j, k);
                    let vp_m3 = self.pore_volume_m3(id);

                    let sg_id = self.sat_gas[id];
                    let so_id = if self.three_phase_mode {
                        (1.0 - self.sat_water[id] - sg_id).max(0.0)
                    } else {
                        self.sat_oil[id]
                    };
                    let c_o_term = if self.three_phase_mode {
                        self.get_c_o_effective(self.pressure[id], self.rs[id])
                    } else {
                        self.get_c_o(self.pressure[id])
                    };
                    let c_t = (c_o_term * so_id
                        + self.pvt.c_w * self.sat_water[id]
                        + if self.three_phase_mode {
                            self.get_c_g(self.pressure[id]) * sg_id
                        } else {
                            0.0
                        })
                        + self.rock_compressibility;

                    let accum = (vp_m3 * c_t) / dt_days;
                    let mut diag = accum;
                    b_rhs[id] += accum * self.pressure[id];

                    let mut neighbors: Vec<(usize, char, usize)> = Vec::new();
                    if i > 0 {
                        neighbors.push((self.idx(i - 1, j, k), 'x', k));
                    }
                    if i < self.nx - 1 {
                        neighbors.push((self.idx(i + 1, j, k), 'x', k));
                    }
                    if j > 0 {
                        neighbors.push((self.idx(i, j - 1, k), 'y', k));
                    }
                    if j < self.ny - 1 {
                        neighbors.push((self.idx(i, j + 1, k), 'y', k));
                    }
                    if k > 0 {
                        neighbors.push((self.idx(i, j, k - 1), 'z', k - 1));
                    }
                    if k < self.nz - 1 {
                        neighbors.push((self.idx(i, j, k + 1), 'z', k + 1));
                    }

                    for (n_id, dim, n_k) in &neighbors {
                        let depth_i = self.depth_at_k(k);
                        let depth_j = self.depth_at_k(*n_k);

                        let p_i = self.pressure[id];
                        let p_j = self.pressure[*n_id];
                        let pc_i = self.get_capillary_pressure(self.sat_water[id]);
                        let pc_j = self.get_capillary_pressure(self.sat_water[*n_id]);

                        let rho_w_i = self.get_rho_w(p_i);
                        let rho_w_j = self.get_rho_w(p_j);
                        let grav_w = self.gravity_head_bar(
                            depth_i,
                            depth_j,
                            self.interface_density_barrier(rho_w_i, rho_w_j),
                        );
                        let rho_o_i = if self.three_phase_mode {
                            self.get_rho_o_cell(id, p_i)
                        } else {
                            self.get_rho_o(p_i)
                        };
                        let rho_o_j = if self.three_phase_mode {
                            self.get_rho_o_cell(*n_id, p_j)
                        } else {
                            self.get_rho_o(p_j)
                        };
                        let grav_o = self.gravity_head_bar(
                            depth_i,
                            depth_j,
                            self.interface_density_barrier(rho_o_i, rho_o_j),
                        );

                        let dphi_o = (p_i - p_j) - grav_o;
                        let dphi_w = (p_i - p_j) - (pc_i - pc_j) - grav_w;

                        let geom_t =
                            DARCY_METRIC_FACTOR * self.geometric_transmissibility(id, *n_id, *dim);

                        let t_total;
                        let explicit_rhs;
                        if self.three_phase_mode {
                            let (lam_w_i, lam_o_i, lam_g_i) = self.phase_mobilities_3p(id);
                            let (lam_w_j, lam_o_j, lam_g_j) = self.phase_mobilities_3p(*n_id);

                            let lam_o_up = if dphi_o >= 0.0 { lam_o_i } else { lam_o_j };
                            let lam_w_up = if dphi_w >= 0.0 { lam_w_i } else { lam_w_j };

                            let pc_og_i = self.get_gas_oil_capillary_pressure(self.sat_gas[id]);
                            let pc_og_j = self.get_gas_oil_capillary_pressure(self.sat_gas[*n_id]);
                            let rho_g_i = self.get_rho_g(p_i);
                            let rho_g_j = self.get_rho_g(p_j);
                            let grav_g = self.gravity_head_bar(
                                depth_i,
                                depth_j,
                                self.interface_density_barrier(rho_g_i, rho_g_j),
                            );
                            let dphi_g = (p_i - p_j) + (pc_og_i - pc_og_j) - grav_g;
                            let lam_g_up = if dphi_g >= 0.0 { lam_g_i } else { lam_g_j };

                            let t_o = geom_t * lam_o_up;
                            let t_w = geom_t * lam_w_up;
                            let t_g = geom_t * lam_g_up;
                            t_total = t_o + t_w + t_g;
                            explicit_rhs = t_o * grav_o + t_w * (pc_i - pc_j + grav_w)
                                - t_g * (pc_og_i - pc_og_j - grav_g);
                        } else {
                            let (lam_w_i, lam_o_i) = self.phase_mobilities(id);
                            let (lam_w_j, lam_o_j) = self.phase_mobilities(*n_id);

                            let lam_o_up = if dphi_o >= 0.0 { lam_o_i } else { lam_o_j };
                            let lam_w_up = if dphi_w >= 0.0 { lam_w_i } else { lam_w_j };

                            let t_o = geom_t * lam_o_up;
                            let t_w = geom_t * lam_w_up;
                            t_total = t_o + t_w;
                            explicit_rhs = t_o * grav_o + t_w * (pc_i - pc_j + grav_w);
                        }

                        diag += t_total;
                        rows.push(id);
                        cols.push(*n_id);
                        vals.push(-t_total);
                        b_rhs[id] += explicit_rhs;
                    }

                    for (w_idx, w) in self.wells.iter().enumerate() {
                        if w.i == i && w.j == j && w.k == k {
                            if let Some(ref control) = well_controls[w_idx] {
                                match &control.decision {
                                    WellControlDecision::Disabled => {}
                                    WellControlDecision::Rate { q_m3_day } => {
                                        b_rhs[id] -= q_m3_day;
                                    }
                                    WellControlDecision::Bhp { bhp_bar } => {
                                        if w.productivity_index.is_finite() && bhp_bar.is_finite() {
                                            diag += w.productivity_index;
                                            b_rhs[id] += w.productivity_index * *bhp_bar;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    rows.push(id);
                    cols.push(id);
                    vals.push(diag);
                    diag_inv[id] = if diag.abs() > f64::EPSILON {
                        1.0 / diag
                    } else {
                        1.0
                    };
                }
            }
        }

        let mut tri = TriMatI::<f64, usize>::new((n_cells, n_cells));
        for idx in 0..vals.len() {
            tri.add_triplet(rows[idx], cols[idx], vals[idx]);
        }
        let a_mat: CsMat<f64> = tri.to_csr();

        let mut x0 = DVector::<f64>::zeros(n_cells);
        for i in 0..n_cells {
            x0[i] = self.pressure[i];
        }
        let solver_result = solve_with_default(LinearSolveParams {
            matrix: &a_mat,
            rhs: &b_rhs,
            preconditioner_inv_diag: &diag_inv,
            initial_guess: &x0,
            tolerance: 1e-7,
            max_iterations: 1000,
        });
        let p_new = solver_result.solution;

        let mut delta_water_m3 = vec![0.0f64; n_cells];
        let mut delta_free_gas_sc = vec![0.0f64; n_cells];
        let mut delta_dg_sc = vec![0.0f64; n_cells];
        let mut max_sat_change = 0.0;

        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let id = self.idx(i, j, k);
                    let mut check = Vec::new();
                    if i < self.nx - 1 {
                        check.push((self.idx(i + 1, j, k), 'x', k));
                    }
                    if j < self.ny - 1 {
                        check.push((self.idx(i, j + 1, k), 'y', k));
                    }
                    if k < self.nz - 1 {
                        check.push((self.idx(i, j, k + 1), 'z', k + 1));
                    }

                    for (nid, dim, n_k) in check {
                        let depth_i = self.depth_at_k(k);
                        let depth_j = self.depth_at_k(n_k);

                        let pc_i = self.get_capillary_pressure(self.sat_water[id]);
                        let pc_j = self.get_capillary_pressure(self.sat_water[nid]);

                        let rho_w_old_i = self.get_rho_w(self.pressure[id]);
                        let rho_w_old_j = self.get_rho_w(self.pressure[nid]);
                        let rho_w_new_i = self.get_rho_w(p_new[id]);
                        let rho_w_new_j = self.get_rho_w(p_new[nid]);
                        let grav_w_old = self.gravity_head_bar(
                            depth_i,
                            depth_j,
                            self.interface_density_barrier(rho_w_old_i, rho_w_old_j),
                        );
                        let grav_w_new = self.gravity_head_bar(
                            depth_i,
                            depth_j,
                            self.interface_density_barrier(rho_w_new_i, rho_w_new_j),
                        );

                        let dphi_w_old =
                            (self.pressure[id] - self.pressure[nid]) - (pc_i - pc_j) - grav_w_old;
                        let dphi_w = (p_new[id] - p_new[nid]) - (pc_i - pc_j) - grav_w_new;

                        let (lam_w_i, lam_w_j) = if self.three_phase_mode {
                            let (w_i, _, _) = self.phase_mobilities_3p(id);
                            let (w_j, _, _) = self.phase_mobilities_3p(nid);
                            (w_i, w_j)
                        } else {
                            let (w_i, _) = self.phase_mobilities(id);
                            let (w_j, _) = self.phase_mobilities(nid);
                            (w_i, w_j)
                        };

                        let lam_w_up = if dphi_w_old >= 0.0 { lam_w_i } else { lam_w_j };
                        let geom_t =
                            DARCY_METRIC_FACTOR * self.geometric_transmissibility(id, nid, dim);
                        let t_w = geom_t * lam_w_up;
                        let water_flux_m3_day = t_w * dphi_w;
                        let dv_water = water_flux_m3_day * dt_days;

                        delta_water_m3[id] -= dv_water;
                        delta_water_m3[nid] += dv_water;
                    }
                }
            }
        }

        if self.three_phase_mode {
            for k in 0..self.nz {
                for j in 0..self.ny {
                    for i in 0..self.nx {
                        let id = self.idx(i, j, k);

                        let mut check = Vec::new();
                        if i < self.nx - 1 {
                            check.push((self.idx(i + 1, j, k), 'x', k));
                        }
                        if j < self.ny - 1 {
                            check.push((self.idx(i, j + 1, k), 'y', k));
                        }
                        if k < self.nz - 1 {
                            check.push((self.idx(i, j, k + 1), 'z', k + 1));
                        }

                        for (nid, dim, n_k) in check {
                            let depth_i = self.depth_at_k(k);
                            let depth_j = self.depth_at_k(n_k);

                            let pc_og_i = self.get_gas_oil_capillary_pressure(self.sat_gas[id]);
                            let pc_og_j = self.get_gas_oil_capillary_pressure(self.sat_gas[nid]);
                            let rho_g_old_i = self.get_rho_g(self.pressure[id]);
                            let rho_g_old_j = self.get_rho_g(self.pressure[nid]);
                            let rho_g_new_i = self.get_rho_g(p_new[id]);
                            let rho_g_new_j = self.get_rho_g(p_new[nid]);
                            let grav_g_old = self.gravity_head_bar(
                                depth_i,
                                depth_j,
                                self.interface_density_barrier(rho_g_old_i, rho_g_old_j),
                            );
                            let grav_g_new = self.gravity_head_bar(
                                depth_i,
                                depth_j,
                                self.interface_density_barrier(rho_g_new_i, rho_g_new_j),
                            );

                            let dphi_g_old = (self.pressure[id] - self.pressure[nid])
                                + (pc_og_i - pc_og_j)
                                - grav_g_old;
                            let dphi_g =
                                (p_new[id] - p_new[nid]) + (pc_og_i - pc_og_j) - grav_g_new;

                            let lam_g_up = if dphi_g_old >= 0.0 {
                                self.gas_mobility(id)
                            } else {
                                self.gas_mobility(nid)
                            };
                            let geom_t =
                                DARCY_METRIC_FACTOR * self.geometric_transmissibility(id, nid, dim);
                            let t_g = geom_t * lam_g_up;
                            let gas_flux_m3_day = t_g * dphi_g;
                            let up_id = if dphi_g_old >= 0.0 { id } else { nid };
                            let gas_flux_sc_day =
                                gas_flux_m3_day / self.get_b_g(p_new[up_id]).max(1e-9);
                            let dv_gas_sc = gas_flux_sc_day * dt_days;

                            delta_free_gas_sc[id] -= dv_gas_sc;
                            delta_free_gas_sc[nid] += dv_gas_sc;

                            if self.pvt_table.is_some() {
                                let rho_o_old_i = self.get_rho_o_cell(id, self.pressure[id]);
                                let rho_o_old_j = self.get_rho_o_cell(nid, self.pressure[nid]);
                                let rho_o_new_i = self.get_rho_o_cell(id, p_new[id]);
                                let rho_o_new_j = self.get_rho_o_cell(nid, p_new[nid]);
                                let grav_o_old = self.gravity_head_bar(
                                    depth_i,
                                    depth_j,
                                    self.interface_density_barrier(rho_o_old_i, rho_o_old_j),
                                );
                                let grav_o_new = self.gravity_head_bar(
                                    depth_i,
                                    depth_j,
                                    self.interface_density_barrier(rho_o_new_i, rho_o_new_j),
                                );
                                let dphi_o_old =
                                    (self.pressure[id] - self.pressure[nid]) - grav_o_old;
                                let dphi_o = (p_new[id] - p_new[nid]) - grav_o_new;

                                let (_, lam_o_i, _) = self.phase_mobilities_3p(id);
                                let (_, lam_o_j, _) = self.phase_mobilities_3p(nid);
                                let lam_o_up = if dphi_o_old >= 0.0 { lam_o_i } else { lam_o_j };
                                let t_o = geom_t * lam_o_up;

                                let oil_flux_res_day = t_o * dphi_o;
                                let up_id = if dphi_o_old >= 0.0 { id } else { nid };
                                let oil_flux_sc_day = oil_flux_res_day
                                    / self.get_b_o_cell(up_id, p_new[up_id]).max(1e-9);
                                let rs_upwind = if dphi_o_old >= 0.0 {
                                    self.rs[id]
                                } else {
                                    self.rs[nid]
                                };
                                let dg_flux_sc_day = oil_flux_sc_day * rs_upwind;
                                let dv_dg_sc = dg_flux_sc_day * dt_days;

                                delta_dg_sc[id] -= dv_dg_sc;
                                delta_dg_sc[nid] += dv_dg_sc;
                            }
                        }
                    }
                }
            }
        }

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
                        delta_water_m3[id] -= q_m3_day * fw * dt_days;
                        let producer_state = if w.injector {
                            None
                        } else {
                            Some(self.producer_control_state_from_resolved_control(
                                w,
                                control,
                                &self.pressure,
                            ))
                        };
                        delta_free_gas_sc[id] -= q_m3_day * fg * dt_days
                            / producer_state
                                .map(|state| state.gas_fvf)
                                .unwrap_or_else(|| self.get_b_g(p_new[id]).max(1e-9));

                        if !w.injector && self.pvt_table.is_some() {
                            let producer_state = producer_state
                                .expect("producer state should exist for producer controls");
                            let q_o_res = q_m3_day * fo;
                            let q_o_sc = q_o_res / producer_state.oil_fvf;
                            let q_dg_sc = q_o_sc * producer_state.rs_sm3_sm3;
                            delta_dg_sc[id] -= q_dg_sc * dt_days;
                        }
                    } else {
                        let fw = if w.injector {
                            1.0
                        } else {
                            self.frac_flow_water(id)
                        };
                        delta_water_m3[id] -= q_m3_day * fw * dt_days;
                    }
                }
            }
        }

        for idx in 0..n_cells {
            let vp_m3 = self.pore_volume_m3(idx);
            if vp_m3 > 0.0 {
                let sat_change_w = (delta_water_m3[idx] / vp_m3).abs();
                let sat_change_g = if self.three_phase_mode {
                    (delta_free_gas_sc[idx].abs() * self.get_b_g(self.pressure[idx]).max(1e-9)
                        / vp_m3)
                        .abs()
                } else {
                    0.0
                };
                let sat_change = sat_change_w.max(sat_change_g);
                if sat_change > max_sat_change {
                    max_sat_change = sat_change;
                }
            }
        }

        let sat_factor = if max_sat_change > self.max_sat_change_per_step {
            self.max_sat_change_per_step / max_sat_change
        } else {
            1.0
        };

        let mut max_pressure_change = 0.0;
        for idx in 0..n_cells {
            let dp = (p_new[idx] - self.pressure[idx]).abs();
            if dp > max_pressure_change {
                max_pressure_change = dp;
            }
        }
        let pressure_factor = if max_pressure_change > self.max_pressure_change_per_step {
            self.max_pressure_change_per_step / max_pressure_change
        } else {
            1.0
        };

        let mut max_well_rate_rel_change = 0.0;
        let mut crossed_control_mode = false;
        for w in &self.wells {
            let old_control = self.resolve_well_control_for_pressures(w, &self.pressure);
            let new_control = self.resolve_well_control_for_pressures(w, p_new.as_slice());
            let q_old = old_control
                .and_then(|control| {
                    self.well_transport_rate_from_control(
                        w,
                        control,
                        self.pressure[self.idx(w.i, w.j, w.k)],
                    )
                })
                .unwrap_or(0.0);
            let q_new = new_control
                .and_then(|control| {
                    self.well_transport_rate_from_control(
                        w,
                        control,
                        p_new[self.idx(w.i, w.j, w.k)],
                    )
                })
                .unwrap_or(0.0);

            let rel = (q_new - q_old).abs() / (q_old.abs() + 1.0);
            if rel > max_well_rate_rel_change {
                max_well_rate_rel_change = rel;
            }
            if !Self::well_control_mode_matches(old_control, new_control) {
                crossed_control_mode = true;
            }
        }
        let rate_factor = if max_well_rate_rel_change > self.max_well_rate_change_fraction {
            self.max_well_rate_change_fraction / max_well_rate_rel_change
        } else {
            1.0
        };
        let control_transition_factor = if crossed_control_mode { 0.5 } else { 1.0 };
        let stable_dt_factor = sat_factor
            .min(pressure_factor)
            .min(rate_factor)
            .min(control_transition_factor)
            .clamp(0.01, 1.0);

        (
            p_new,
            delta_water_m3,
            delta_free_gas_sc,
            delta_dg_sc,
            well_controls,
            stable_dt_factor,
            solver_result.converged,
            solver_result.iterations,
        )
    }

}
