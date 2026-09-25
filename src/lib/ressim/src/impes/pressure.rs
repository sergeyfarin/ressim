use nalgebra::DVector;
use sprs::{CsMat, TriMatI};
use std::f64;

use crate::impes::closure::CellMasses;
use crate::impes::wells::{MAX_ACTIVE_SET_PASSES, WellSystem};
use crate::solvers::{LinearSolveParams, solve_with_default};
use crate::well_control::{ResolvedWellControl, WellControlDecision, WellControlGroupKey};
use crate::{InjectedFluid, ReservoirSimulator};

/// Conversion factor from mD·m²/(m·cP) to m³/day/bar.
const DARCY_METRIC_FACTOR: f64 = 8.526_988_8e-3;

/// Newton iterations allowed for the three-phase volume balance before the substep is retried
/// at a smaller dt.
const MAX_VOLUME_BALANCE_ITERATIONS: usize = 20;
/// Converged when every cell's volume residual is worth less than this much pressure [bar],
/// i.e. `|V − Vp| ≤ tol · Vp·c_t`. A volume tolerance alone is not enough: a leftover of
/// 1e-6·Vp in a cell whose only storage is water (c_t ≈ 3e-7 /bar) is a 3 bar pressure error
/// that the next substep then has to correct.
const VOLUME_BALANCE_PRESSURE_TOLERANCE_BAR: f64 = 1e-4;
/// Floor on `c_t` [1/bar] when converting a volume residual to pressure, so a cell with almost
/// no storage is not held to roundoff.
const VOLUME_BALANCE_MIN_COMPRESSIBILITY: f64 = 1e-7;
/// Converged when every implicit rate well meets its target to this rate [m³/day].
const VOLUME_BALANCE_RATE_TOLERANCE_M3_DAY: f64 = 1e-6;
/// Largest change in a cell's dissolved-gas ratio one substep may make, as a fraction of the
/// oil's saturated Rs at the cell's pressure (#44).
///
/// Transport is explicit, so a substep produces oil at the Rs it started with. Below the bubble
/// point Rs falls with pressure, and a substep that crosses it overstates the solution gas it
/// produces. None of the other limits sees this: liberated gas is immobile until it reaches the
/// critical gas saturation, so the saturation limit never trips, and a bubble-point crossing
/// can be a few bar. On a depletion column through the bubble point, taking the whole 5-day
/// report as one substep put cumulative gas 1.5% above the time-converged answer; this limit
/// holds it to 0.2%.
const MAX_RS_RELATIVE_CHANGE_PER_STEP: f64 = 0.05;
/// Floor on the Rs [Sm³/Sm³] the change above is measured against, so nearly dead oil is not
/// held to a vanishing absolute change.
const RS_CHANGE_REFERENCE_FLOOR_M3M3: f64 = 1.0;

/// Per-cell component changes over one substep, fluxes and wells included.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TransportDeltas {
    /// Water [reservoir m³]. The two-phase path transports water by volume, and both paths use
    /// it for the saturation-change timestep limit.
    pub(crate) water_m3: Vec<f64>,
    /// Water [Sm³]; the three-phase path conserves this.
    pub(crate) water_sc: Vec<f64>,
    pub(crate) oil_sc: Vec<f64>,
    pub(crate) free_gas_sc: Vec<f64>,
    pub(crate) dissolved_gas_sc: Vec<f64>,
}

impl TransportDeltas {
    pub(crate) fn zeros(n_cells: usize) -> Self {
        Self {
            water_m3: vec![0.0; n_cells],
            water_sc: vec![0.0; n_cells],
            oil_sc: vec![0.0; n_cells],
            free_gas_sc: vec![0.0; n_cells],
            dissolved_gas_sc: vec![0.0; n_cells],
        }
    }

    /// `masses` after this substep's changes to cell `id`.
    pub(crate) fn applied_to(&self, id: usize, masses: &CellMasses) -> CellMasses {
        CellMasses {
            water_sc: masses.water_sc + self.water_sc[id],
            oil_sc: masses.oil_sc + self.oil_sc[id],
            free_gas_sc: masses.free_gas_sc + self.free_gas_sc[id],
            dissolved_gas_sc: masses.dissolved_gas_sc + self.dissolved_gas_sc[id],
        }
    }
}

/// Outcome of one IMPES pressure solve and its transport.
pub(crate) struct PressureStep {
    pub(crate) p_new: DVector<f64>,
    pub(crate) deltas: TransportDeltas,
    pub(crate) well_controls: Vec<Option<ResolvedWellControl>>,
    pub(crate) stable_dt_factor: f64,
    /// The part of `stable_dt_factor` that scales with dt: the saturation, well-throughput,
    /// dissolved-gas, pressure and rate limits, without the fixed halving on a control-mode switch.
    pub(crate) change_factor: f64,
    /// Every linear solve converged and, in three-phase mode, so did the volume balance.
    pub(crate) converged: bool,
    pub(crate) linear_iterations: usize,
}

/// The assembled pressure matrix, as triplets, with what the volume-balance Newton step needs
/// to swap its storage diagonal.
struct PressureOperator<'a> {
    rows: &'a [usize],
    cols: &'a [usize],
    vals: &'a [f64],
    diag_positions: &'a [usize],
    accumulation: &'a [f64],
}

impl ReservoirSimulator {
    pub(crate) fn calculate_fluxes(&self, delta_t_days: f64) -> PressureStep {
        let n_cells = self.nx * self.ny * self.nz;
        if n_cells == 0 {
            return PressureStep {
                p_new: DVector::zeros(0),
                deltas: TransportDeltas::zeros(0),
                well_controls: vec![],
                stable_dt_factor: 1.0,
                change_factor: 1.0,
                converged: true,
                linear_iterations: 0,
            };
        }
        let dt_days = delta_t_days.max(1e-12);

        let mut rows: Vec<usize> = Vec::with_capacity(n_cells * 7);
        let mut cols: Vec<usize> = Vec::with_capacity(n_cells * 7);
        let mut vals: Vec<f64> = Vec::with_capacity(n_cells * 7);

        let mut b_rhs = DVector::<f64>::zeros(n_cells);
        let mut accumulation = vec![0.0f64; n_cells];
        let mut diag_positions = vec![0usize; n_cells];

        let well_controls: Vec<Option<ResolvedWellControl>> = self
            .wells
            .iter()
            .map(|w| self.resolve_well_control_for_pressures(w, &self.pressure))
            .collect();
        let mut well_system = self.build_well_system(&well_controls);

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
                    accumulation[id] = accum;
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

                    // Only single-completion rate wells enter as a fixed source here. BHP
                    // completions and multi-completion rate wells are `WellSystem` terms.
                    for (w_idx, w) in self.wells.iter().enumerate() {
                        if w.i == i && w.j == j && w.k == k && well_system.has_fixed_rate(w_idx) {
                            if let Some(ResolvedWellControl {
                                decision: WellControlDecision::Rate { q_m3_day },
                                ..
                            }) = well_controls[w_idx]
                            {
                                b_rhs[id] -= q_m3_day;
                            }
                        }
                    }

                    diag_positions[id] = vals.len();
                    rows.push(id);
                    cols.push(id);
                    vals.push(diag);
                }
            }
        }

        // Solve with the active well terms, then let the solution revise which completions
        // flow and which implicit wells hit their BHP limit, until both settle
        // (`impes/wells.rs`, #10).
        let mut x0 = well_system.initial_guess(&self.pressure);
        let mut linear_converged = true;
        let mut linear_iterations = 0;
        let mut solution;
        let mut operator;
        let mut active_set_passes = 0;
        loop {
            operator = well_system.assemble(&rows, &cols, &vals, &b_rhs, &diag_positions);
            let (ext_rows, ext_cols, ext_vals, ext_rhs, ext_diag_inv) = &operator;
            let n = well_system.unknowns();
            let mut tri = TriMatI::<f64, usize>::new((n, n));
            for idx in 0..ext_vals.len() {
                tri.add_triplet(ext_rows[idx], ext_cols[idx], ext_vals[idx]);
            }
            let a_mat: CsMat<f64> = tri.to_csr();
            let solver_result = solve_with_default(LinearSolveParams {
                matrix: &a_mat,
                rhs: ext_rhs,
                preconditioner_inv_diag: ext_diag_inv,
                initial_guess: &x0,
                tolerance: 1e-7,
                max_iterations: 1000,
            });
            linear_converged &= solver_result.converged;
            linear_iterations += solver_result.iterations;
            solution = solver_result.solution;
            active_set_passes += 1;
            if !well_system.update(&solution) || active_set_passes >= MAX_ACTIVE_SET_PASSES {
                break;
            }
            x0 = solution.clone();
        }
        let well_controls = well_system.step_controls(&well_controls);
        let cell_solution = DVector::from_iterator(n_cells, solution.iter().take(n_cells).copied());

        let (p_new, deltas, well_controls, volume_balance_converged) = if self.three_phase_mode {
            let (ext_rows, ext_cols, ext_vals, _, _) = &operator;
            self.iterate_volume_balance(
                solution,
                &mut well_system,
                &well_controls,
                dt_days,
                PressureOperator {
                    rows: ext_rows,
                    cols: ext_cols,
                    vals: ext_vals,
                    diag_positions: &diag_positions,
                    accumulation: &accumulation,
                },
                &mut linear_converged,
                &mut linear_iterations,
            )
        } else {
            let deltas = self.transport_deltas(&cell_solution, &well_controls, dt_days);
            (cell_solution, deltas, well_controls, true)
        };
        let mut max_sat_change = 0.0;

        for idx in 0..n_cells {
            let vp_m3 = self.pore_volume_m3(idx);
            if vp_m3 > 0.0 {
                let sat_change_w = (deltas.water_m3[idx] / vp_m3).abs();
                let sat_change_g = if self.three_phase_mode {
                    (deltas.free_gas_sc[idx].abs() * self.get_b_g(self.pressure[idx]).max(1e-9)
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

        // Explicit fractional flow at a producer cell (see `frac_flow_water_derivative`). The
        // per-cell saturation cap above is not this criterion: a substep can move the well cell
        // by well under `max_sat_change_per_step` and still overshoot the fractional-flow
        // response, which shows up as the producer's water cut alternating between substeps and
        // hence as chatter on the reported oil rate. Capping the substep throughput at the
        // monotone limit (ratio 1) damps it instead of leaving it to the report step to average.
        let mut max_well_throughput_ratio = 0.0_f64;
        for (w_idx, w) in self.wells.iter().enumerate() {
            if w.injector {
                continue;
            }
            let Some(control) = well_controls[w_idx] else {
                continue;
            };
            let id = self.idx(w.i, w.j, w.k);
            let vp_m3 = self.pore_volume_m3(id);
            if vp_m3 <= 0.0 {
                continue;
            }
            let q_m3_day = match self.well_transport_rate_from_control(w, control, p_new[id]) {
                Some(q_m3_day) if q_m3_day.is_finite() => q_m3_day.max(0.0),
                _ => continue,
            };
            let ratio = q_m3_day * dt_days * self.frac_flow_water_derivative(id) / vp_m3;
            if ratio > max_well_throughput_ratio {
                max_well_throughput_ratio = ratio;
            }
        }
        let well_throughput_factor = if max_well_throughput_ratio > 1.0 {
            1.0 / max_well_throughput_ratio
        } else {
            1.0
        };

        // The change in dissolved gas the flash at the end of the substep will make.
        let mut max_rs_rel_change = 0.0_f64;
        if self.three_phase_mode
            && let Some(table) = &self.pvt_table
        {
            for idx in 0..n_cells {
                if self.pore_volume_m3(idx) <= 0.0 {
                    continue;
                }
                let old = self.cell_masses(idx);
                let flash = self.flash_cell(idx, p_new[idx], &deltas.applied_to(idx, &old));
                let rs_reference = table
                    .interpolate(self.pressure[idx])
                    .rs_m3m3
                    .max(self.rs[idx])
                    .max(RS_CHANGE_REFERENCE_FLOOR_M3M3);
                max_rs_rel_change =
                    max_rs_rel_change.max((flash.rs - self.rs[idx]).abs() / rs_reference);
            }
        }
        let rs_factor = if max_rs_rel_change > MAX_RS_RELATIVE_CHANGE_PER_STEP {
            MAX_RS_RELATIVE_CHANGE_PER_STEP / max_rs_rel_change
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

        // Rate change is measured per physical well: the summed rate of its completions at the
        // beginning and end of the substep. A rate-controlled well spreads a fixed total over
        // its completions, and that split moves with every pressure change. Taken completion
        // by completion, against a 1 m³/day floor, a completion going from 0 to 7.5 m³/day read
        // as a 750% change and cut dt tenfold at any dt while the well's total never moved, so
        // a fully perforated well exhausted the retry budget (#10). For a single-completion
        // well the per-well sum is the completion's own rate, so nothing else changes.
        let mut well_rate_totals: Vec<(WellControlGroupKey, f64, f64)> = Vec::new();
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

            let key = self.well_control_group_key(w);
            match well_rate_totals.iter_mut().find(|(k, _, _)| *k == key) {
                Some((_, old_total, new_total)) => {
                    *old_total += q_old;
                    *new_total += q_new;
                }
                None => well_rate_totals.push((key, q_old, q_new)),
            }
            if !Self::well_control_mode_matches(old_control, new_control) {
                crossed_control_mode = true;
            }
        }
        let max_well_rate_rel_change = well_rate_totals
            .iter()
            .map(|(_, q_old, q_new)| (q_new - q_old).abs() / (q_old.abs() + 1.0))
            .fold(0.0_f64, f64::max);
        let rate_factor = if max_well_rate_rel_change > self.max_well_rate_change_fraction {
            self.max_well_rate_change_fraction / max_well_rate_rel_change
        } else {
            1.0
        };
        let control_transition_factor = if crossed_control_mode { 0.5 } else { 1.0 };
        // The factors that scale with dt, as opposed to the fixed halving on a control switch.
        let change_factor = sat_factor
            .min(well_throughput_factor)
            .min(rs_factor)
            .min(pressure_factor)
            .min(rate_factor);
        let stable_dt_factor = change_factor
            .min(control_transition_factor)
            .clamp(0.01, 1.0);
        PressureStep {
            p_new,
            deltas,
            well_controls,
            stable_dt_factor,
            change_factor,
            converged: linear_converged && volume_balance_converged,
            linear_iterations,
        }
    }

    /// Component changes over a substep of `dt_days` for the pressure field `p_new`: inter-cell
    /// fluxes (explicit mobilities, upwinded on the beginning-of-substep potential) plus well
    /// sources and sinks.
    pub(crate) fn transport_deltas(
        &self,
        p_new: &DVector<f64>,
        well_controls: &[Option<ResolvedWellControl>],
        dt_days: f64,
    ) -> TransportDeltas {
        let n_cells = self.nx * self.ny * self.nz;
        let mut deltas = TransportDeltas::zeros(n_cells);

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

                        deltas.water_m3[id] -= dv_water;
                        deltas.water_m3[nid] += dv_water;
                        let up_w = if dphi_w_old >= 0.0 { id } else { nid };
                        let dv_water_sc = dv_water * self.water_inverse_fvf(p_new[up_w]);
                        deltas.water_sc[id] -= dv_water_sc;
                        deltas.water_sc[nid] += dv_water_sc;
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

                            deltas.free_gas_sc[id] -= dv_gas_sc;
                            deltas.free_gas_sc[nid] += dv_gas_sc;

                            {
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
                                let dv_oil_sc = oil_flux_sc_day * dt_days;
                                deltas.oil_sc[id] -= dv_oil_sc;
                                deltas.oil_sc[nid] += dv_oil_sc;

                                if self.pvt_table.is_some() {
                                    let dv_dg_sc = dv_oil_sc * self.rs[up_id];
                                    deltas.dissolved_gas_sc[id] -= dv_dg_sc;
                                    deltas.dissolved_gas_sc[nid] += dv_dg_sc;
                                }
                            }
                        }
                    }
                }
            }
        }

        for (w_idx, w) in self.wells.iter().enumerate() {
            let id = self.idx(w.i, w.j, w.k);
            let Some(control) = well_controls[w_idx] else {
                continue;
            };
            let Some(q_m3_day) = self.well_transport_rate_from_control(w, control, p_new[id])
            else {
                continue;
            };
            let q_m3 = q_m3_day * dt_days;
            if !self.three_phase_mode {
                let fw = if w.injector {
                    1.0
                } else {
                    self.frac_flow_water(id)
                };
                deltas.water_m3[id] -= q_m3 * fw;
                continue;
            }
            // Withdrawals use exactly the conversions `record_step_report` applies, so the
            // reported production is the mass this substep removes.
            let inv_bw = self.water_inverse_fvf(p_new[id]);
            if w.injector {
                let (fw, fg) = match self.injected_fluid {
                    InjectedFluid::Water => (1.0, 0.0),
                    InjectedFluid::Gas => (0.0, 1.0),
                };
                deltas.water_m3[id] -= q_m3 * fw;
                deltas.water_sc[id] -= q_m3 * fw * inv_bw;
                deltas.free_gas_sc[id] -= q_m3 * fg / self.get_b_g(p_new[id]).max(1e-9);
            } else {
                let producer =
                    self.producer_control_state_from_resolved_control(w, control, &self.pressure);
                let oil_sc = q_m3 * producer.oil_fraction / producer.oil_fvf.max(1e-9);
                deltas.water_m3[id] -= q_m3 * producer.water_fraction;
                deltas.water_sc[id] -= q_m3 * producer.water_fraction * inv_bw;
                deltas.oil_sc[id] -= oil_sc;
                deltas.free_gas_sc[id] -= q_m3 * producer.gas_fraction / producer.gas_fvf.max(1e-9);
                if self.pvt_table.is_some() {
                    deltas.dissolved_gas_sc[id] -= oil_sc * producer.rs_sm3_sm3;
                }
            }
        }

        deltas
    }

    /// Drive the three-phase volume balance `V(p, N(p)) = Vp(p)` to convergence, starting from
    /// the linear pressure solve `p_initial`.
    ///
    /// `N(p)` is each cell's masses after this substep's transport at pressure `p`, and `V` is
    /// the volume [`flash_cell`](ReservoirSimulator::flash_cell) gives them. The Newton matrix
    /// is the assembled pressure operator with its storage diagonal `Vp·c_t/dt` replaced by the
    /// closure's own slope, and the right-hand side is the volume residual over dt. The linear
    /// solve's `c_t` only supplies the first guess, so an approximate storage term, such as the
    /// one for a substep crossing the bubble point, costs iterations rather than mass (#37).
    fn iterate_volume_balance(
        &self,
        x_initial: DVector<f64>,
        well_system: &mut WellSystem,
        resolved_controls: &[Option<ResolvedWellControl>],
        dt_days: f64,
        operator: PressureOperator<'_>,
        linear_converged: &mut bool,
        linear_iterations: &mut usize,
    ) -> (
        DVector<f64>,
        TransportDeltas,
        Vec<Option<ResolvedWellControl>>,
        bool,
    ) {
        let n_cells = self.nx * self.ny * self.nz;
        let n = well_system.unknowns();
        let old_masses: Vec<CellMasses> = (0..n_cells).map(|id| self.cell_masses(id)).collect();
        let mut x = x_initial;
        let mut vals = operator.vals.to_vec();
        let mut residual = DVector::<f64>::zeros(n);
        let mut diag_inv = DVector::<f64>::zeros(n);
        for idx in 0..vals.len() {
            let row = operator.rows[idx];
            if row >= n_cells && row == operator.cols[idx] && vals[idx].abs() > f64::EPSILON {
                diag_inv[row] = 1.0 / vals[idx];
            }
        }

        let finish = |x: &DVector<f64>, well_system: &WellSystem| {
            let p = DVector::from_iterator(n_cells, x.iter().take(n_cells).copied());
            let controls = well_system.step_controls(resolved_controls);
            let deltas = self.transport_deltas(&p, &controls, dt_days);
            (p, deltas, controls)
        };

        for _ in 0..MAX_VOLUME_BALANCE_ITERATIONS {
            well_system.set_bhps(&x);
            let (p, deltas, _) = finish(&x, well_system);
            let mut worst = 0.0_f64;
            for id in 0..n_cells {
                let masses = deltas.applied_to(id, &old_masses[id]);
                let flash = self.flash_cell(id, p[id], &masses);
                let r = flash.volume_residual_m3();
                residual[id] = r / dt_days;
                let storage = self.closure_storage_m3_per_bar(id, p[id], &masses);
                let pressure_equivalent = r.abs()
                    / storage.max(VOLUME_BALANCE_MIN_COMPRESSIBILITY * flash.pore_volume_m3);
                worst = worst.max(pressure_equivalent);
                let pos = operator.diag_positions[id];
                let diag = operator.vals[pos] - operator.accumulation[id] + storage / dt_days;
                vals[pos] = diag;
                diag_inv[id] = if diag.abs() > f64::EPSILON {
                    1.0 / diag
                } else {
                    1.0
                };
            }
            // Implicit wells: `Σ q_k − Q`, the right-hand side of the well rows' Newton step.
            let mut wells_converged = true;
            for (g, r) in well_system.rate_residuals(self, &p).into_iter().enumerate() {
                residual[n_cells + g] = r;
                wells_converged &= r.abs() <= VOLUME_BALANCE_RATE_TOLERANCE_M3_DAY;
            }
            if !worst.is_finite() {
                let (p, deltas, controls) = finish(&x, well_system);
                return (p, deltas, controls, false);
            }
            if worst <= VOLUME_BALANCE_PRESSURE_TOLERANCE_BAR && wells_converged {
                let (p, deltas, controls) = finish(&x, well_system);
                return (p, deltas, controls, true);
            }

            let mut tri = TriMatI::<f64, usize>::new((n, n));
            for idx in 0..vals.len() {
                tri.add_triplet(operator.rows[idx], operator.cols[idx], vals[idx]);
            }
            let matrix: CsMat<f64> = tri.to_csr();
            let correction = solve_with_default(LinearSolveParams {
                matrix: &matrix,
                rhs: &residual,
                preconditioner_inv_diag: &diag_inv,
                initial_guess: &DVector::zeros(n),
                tolerance: 1e-9,
                max_iterations: 1000,
            });
            *linear_iterations += correction.iterations;
            if !correction.converged {
                *linear_converged = false;
                let (p, deltas, controls) = finish(&x, well_system);
                return (p, deltas, controls, false);
            }
            x += correction.solution;
        }

        well_system.set_bhps(&x);
        let (p, deltas, controls) = finish(&x, well_system);
        (p, deltas, controls, false)
    }
}
