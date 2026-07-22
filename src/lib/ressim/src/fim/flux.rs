//! Generic (differentiable) inter-cell flux evaluation.
//!
//! Mirrors `assembly::interface_flux_terms` / `interface_flux_contribution`,
//! written once over [`Scalar`] so it produces the plain flux with `f64` and
//! the exact Jacobian with `Ad<6>` — six slots because a face flux depends on
//! both neighboring cells' primary variables: `[p_i, sw_i, hc_i, p_j, sw_j,
//! hc_j]`. One evaluation with this seeding yields all four 3x3 sub-blocks
//! (i-i, i-j, j-i, j-j) of the face's contribution to the Jacobian.
//!
//! Upwind direction and phase regime are chosen on the *value* of the driving
//! quantity (potential difference, regime classification), matching the
//! existing scalar code's control flow exactly and keeping the frozen-regime,
//! frozen-upwind behavior expected within one Newton iteration.

#![allow(dead_code)]

use crate::ReservoirSimulator;
use crate::fim::ad::{Ad, Scalar};
use crate::fim::properties::cell_props_generic;
use crate::fim::state::HydrocarbonState;

/// Per-face flux terms in standard-condition rate units (before multiplying by
/// `dt_days`), generic over `S`.
pub(crate) struct FaceFluxTermsGeneric<S> {
    pub(crate) flux_sc_day: [S; 3],
}

/// One neighboring cell's primary-variable inputs to a face flux evaluation.
#[derive(Clone, Copy)]
pub(crate) struct FaceCellInput<S> {
    pub(crate) p: S,
    pub(crate) sw: S,
    pub(crate) hydrocarbon_var: S,
    pub(crate) regime: HydrocarbonState,
    pub(crate) depth: f64,
    pub(crate) drsdt0_base_rs: Option<f64>,
}

/// Generic mirror of `assembly::interface_flux_terms`'s flux computation.
/// `geom_t` is the precomputed `DARCY_METRIC_FACTOR * geometric_transmissibility`
/// (purely geometric, independent of the current unknowns).
pub(crate) fn face_flux_terms_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    geom_t: f64,
    i: &FaceCellInput<S>,
    j: &FaceCellInput<S>,
) -> FaceFluxTermsGeneric<S> {
    let props_i = cell_props_generic(
        sim,
        i.regime,
        i.p,
        i.sw,
        i.hydrocarbon_var,
        i.drsdt0_base_rs,
    );
    let props_j = cell_props_generic(
        sim,
        j.regime,
        j.p,
        j.sw,
        j.hydrocarbon_var,
        j.drsdt0_base_rs,
    );

    let rho_w_i = sim.water_density_generic(i.p);
    let rho_w_j = sim.water_density_generic(j.p);
    let rho_o_i = sim.oil_density_generic(i.p, props_i.rs);
    let rho_o_j = sim.oil_density_generic(j.p, props_j.rs);
    let rho_g_i = sim.gas_density_generic(i.p);
    let rho_g_j = sim.gas_density_generic(j.p);

    let pcw_i = sim.pc.capillary_pressure_generic(i.sw, &sim.scal);
    let pcw_j = sim.pc.capillary_pressure_generic(j.sw, &sim.scal);
    let pcog_i = gas_oil_capillary_pressure_generic(sim, props_i.sg);
    let pcog_j = gas_oil_capillary_pressure_generic(sim, props_j.sg);

    let grav_w = gravity_head_generic(sim, i.depth, j.depth, rho_w_i, rho_w_j);
    let grav_o = gravity_head_generic(sim, i.depth, j.depth, rho_o_i, rho_o_j);
    let grav_g = gravity_head_generic(sim, i.depth, j.depth, rho_g_i, rho_g_j);

    let dphi_w = (i.p - j.p) - (pcw_i - pcw_j) - grav_w;
    let dphi_o = (i.p - j.p) - grav_o;
    let dphi_g = (i.p - j.p) + (pcog_i - pcog_j) - grav_g;

    let mob_i = sim.phase_mobilities_for_state_generic(i.sw, props_i.sg, i.p, props_i.rs);
    let mob_j = sim.phase_mobilities_for_state_generic(j.sw, props_j.sg, j.p, props_j.rs);

    // Upwind selection: branch on the value of the potential difference,
    // matching `interface_flux_terms`'s `dphi >= 0.0` convention exactly.
    let (mobility_w, p_w) = if dphi_w.value() >= 0.0 {
        (mob_i.water, i.p)
    } else {
        (mob_j.water, j.p)
    };

    let (mobility_o, bo_o, rs_o) = if dphi_o.value() >= 0.0 {
        (mob_i.oil, props_i.bo, props_i.rs)
    } else {
        (mob_j.oil, props_j.bo, props_j.rs)
    };

    let (mobility_g, bg_g) = if dphi_g.value() >= 0.0 {
        (mob_i.gas, props_i.bg)
    } else {
        (mob_j.gas, props_j.bg)
    };

    let q_w_sc_day = mobility_w * dphi_w * geom_t * sim.water_inverse_fvf_generic(p_w);
    let q_o_res_day = mobility_o * dphi_o * geom_t;
    let q_o_sc_day = q_o_res_day / bo_o.max_floor(1e-9);
    let q_g_free_sc_day = mobility_g * dphi_g * geom_t / bg_g.max_floor(1e-9);
    let q_g_dissolved_sc_day = q_o_sc_day * rs_o;
    let q_g_sc_day = q_g_free_sc_day + q_g_dissolved_sc_day;

    FaceFluxTermsGeneric {
        flux_sc_day: [q_w_sc_day, q_o_sc_day, q_g_sc_day],
    }
}

fn gravity_head_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    depth_i: f64,
    depth_j: f64,
    rho_i: S,
    rho_j: S,
) -> S {
    if !sim.gravity_enabled {
        return S::from_f64(0.0);
    }
    let density_avg = (rho_i + rho_j) * 0.5;
    density_avg * (9.80665 * (depth_i - depth_j) * 1e-5)
}

fn gas_oil_capillary_pressure_generic<S: Scalar>(sim: &ReservoirSimulator, sg: S) -> S {
    match (&sim.pc_og, &sim.scal_3p) {
        (Some(pc), Some(rock)) => pc.capillary_pressure_og_generic(sg, rock),
        _ => S::from_f64(0.0),
    }
}

/// Assemble the four 3x3 Jacobian sub-blocks of one face's contribution to the
/// residual, by seeding `Ad<6>` against `[p_i, sw_i, hc_i, p_j, sw_j, hc_j]`.
///
/// Convention (matching `assembly::interface_flux_contribution`): cell `i`'s
/// three equations receive `+flux_sc_day * dt_days`; cell `j`'s receive the
/// negation. Returns `(block_ii, block_ij, block_ji, block_jj)` where
/// `block_xy[eq][var]` is `d(residual_x[eq]) / d(unknown_y[var])`.
#[allow(clippy::type_complexity)]
pub(crate) fn face_flux_jacobian_blocks(
    sim: &ReservoirSimulator,
    geom_t: f64,
    dt_days: f64,
    i: &FaceCellInput<f64>,
    j: &FaceCellInput<f64>,
) -> ([[f64; 3]; 3], [[f64; 3]; 3], [[f64; 3]; 3], [[f64; 3]; 3]) {
    let i_ad = FaceCellInput {
        p: Ad::<6>::variable(i.p, 0),
        sw: Ad::<6>::variable(i.sw, 1),
        hydrocarbon_var: Ad::<6>::variable(i.hydrocarbon_var, 2),
        regime: i.regime,
        depth: i.depth,
        drsdt0_base_rs: i.drsdt0_base_rs,
    };
    let j_ad = FaceCellInput {
        p: Ad::<6>::variable(j.p, 3),
        sw: Ad::<6>::variable(j.sw, 4),
        hydrocarbon_var: Ad::<6>::variable(j.hydrocarbon_var, 5),
        regime: j.regime,
        depth: j.depth,
        drsdt0_base_rs: j.drsdt0_base_rs,
    };

    let terms = face_flux_terms_generic(sim, geom_t, &i_ad, &j_ad);

    let mut block_ii = [[0.0; 3]; 3];
    let mut block_ij = [[0.0; 3]; 3];
    let mut block_ji = [[0.0; 3]; 3];
    let mut block_jj = [[0.0; 3]; 3];

    for (eq, flux) in terms.flux_sc_day.iter().enumerate() {
        let d = flux.deriv();
        for v in 0..3 {
            block_ii[eq][v] = d[v] * dt_days;
            block_ij[eq][v] = d[3 + v] * dt_days;
            block_ji[eq][v] = -d[v] * dt_days;
            block_jj[eq][v] = -d[3 + v] * dt_days;
        }
    }

    (block_ii, block_ij, block_ji, block_jj)
}

/// Plain-`f64` face residual contribution `[i_water, i_oil, i_gas, j_water,
/// j_oil, j_gas]`, for numerical-Jacobian comparison.
pub(crate) fn face_flux_residual_f64(
    sim: &ReservoirSimulator,
    geom_t: f64,
    dt_days: f64,
    i: &FaceCellInput<f64>,
    j: &FaceCellInput<f64>,
) -> [f64; 6] {
    let terms = face_flux_terms_generic(sim, geom_t, i, j);
    let f = terms.flux_sc_day;
    [
        f[0] * dt_days,
        f[1] * dt_days,
        f[2] * dt_days,
        -f[0] * dt_days,
        -f[1] * dt_days,
        -f[2] * dt_days,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReservoirSimulator;
    use crate::fim::numjac::{assert_jacobian_matches, central_difference_jacobian};
    use crate::pvt::{PvtRow, PvtTable};

    fn three_phase_sim(gravity: bool, capillary: bool) -> ReservoirSimulator {
        let mut sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        sim.set_three_phase_mode_enabled(true);
        sim.set_gravity_enabled(gravity);
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
        sim.set_three_phase_rel_perm_props(
            0.1, 0.1, 0.05, 0.05, 0.15, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0,
        )
        .unwrap();
        if capillary {
            sim.set_gas_oil_capillary_params(2.0, 2.0).unwrap();
        }
        sim
    }

    fn input(p: f64, sw: f64, hc: f64, regime: HydrocarbonState, depth: f64) -> FaceCellInput<f64> {
        FaceCellInput {
            p,
            sw,
            hydrocarbon_var: hc,
            regime,
            depth,
            drsdt0_base_rs: None,
        }
    }

    /// One face-flux gate: AD-derived 4-block Jacobian must match the central-
    /// difference reference for the packed 6-unknown / 6-equation face system.
    fn flux_gate(gravity: bool, capillary: bool, i: FaceCellInput<f64>, j: FaceCellInput<f64>) {
        let sim = three_phase_sim(gravity, capillary);
        let geom_t = 3.5_f64;
        let dt_days = 0.4_f64;

        let (bii, bij, bji, bjj) = face_flux_jacobian_blocks(&sim, geom_t, dt_days, &i, &j);
        let mut analytic = vec![vec![0.0; 6]; 6];
        for eq in 0..3 {
            for v in 0..3 {
                analytic[eq][v] = bii[eq][v];
                analytic[eq][3 + v] = bij[eq][v];
                analytic[3 + eq][v] = bji[eq][v];
                analytic[3 + eq][3 + v] = bjj[eq][v];
            }
        }

        let x0 = [i.p, i.sw, i.hydrocarbon_var, j.p, j.sw, j.hydrocarbon_var];
        let residual = |x: &[f64]| {
            let i2 = FaceCellInput {
                p: x[0],
                sw: x[1],
                hydrocarbon_var: x[2],
                ..i
            };
            let j2 = FaceCellInput {
                p: x[3],
                sw: x[4],
                hydrocarbon_var: x[5],
                ..j
            };
            face_flux_residual_f64(&sim, geom_t, dt_days, &i2, &j2).to_vec()
        };
        let numerical = central_difference_jacobian(&x0, 6, residual);

        assert_jacobian_matches(&analytic, &numerical, 1e-6, 1e-9);
    }

    #[test]
    fn flux_gate_no_gravity_no_capillary_i_upwind() {
        // p_i > p_j -> flow from i to j, cell i upwind for all phases.
        flux_gate(
            false,
            false,
            input(160.0, 0.3, 0.1, HydrocarbonState::Saturated, 0.0),
            input(140.0, 0.25, 0.08, HydrocarbonState::Saturated, 0.0),
        );
    }

    #[test]
    fn flux_gate_no_gravity_no_capillary_j_upwind() {
        // p_j > p_i -> flow from j to i, cell j upwind for all phases.
        flux_gate(
            false,
            false,
            input(140.0, 0.25, 0.08, HydrocarbonState::Saturated, 0.0),
            input(160.0, 0.3, 0.1, HydrocarbonState::Saturated, 0.0),
        );
    }

    #[test]
    fn flux_gate_with_gravity_and_capillary() {
        flux_gate(
            true,
            true,
            input(160.0, 0.3, 0.1, HydrocarbonState::Saturated, 1000.0),
            input(150.0, 0.25, 0.08, HydrocarbonState::Saturated, 1010.0),
        );
    }

    #[test]
    fn flux_gate_undersaturated_cells() {
        flux_gate(
            false,
            false,
            input(160.0, 0.3, 12.0, HydrocarbonState::Undersaturated, 0.0),
            input(150.0, 0.25, 11.0, HydrocarbonState::Undersaturated, 0.0),
        );
    }

    /// The generic f64 flux path must reproduce `assembly::interface_flux_terms`
    /// on a real grid, so the AD path shares a single source of truth with the
    /// existing residual (mirrors the Phase 1 accumulation parity check).
    #[test]
    fn generic_f64_flux_matches_assembly_interface_flux_terms() {
        use crate::fim::assembly::{self, DARCY_METRIC_FACTOR};
        use crate::fim::state::{FimCellState, FimState};

        let sim = three_phase_sim(true, true);
        let state = FimState {
            cells: vec![
                FimCellState {
                    pressure_bar: 160.0,
                    sw: 0.3,
                    hydrocarbon_var: 0.1,
                    regime: HydrocarbonState::Saturated,
                },
                FimCellState {
                    pressure_bar: 150.0,
                    sw: 0.25,
                    hydrocarbon_var: 0.08,
                    regime: HydrocarbonState::Saturated,
                },
            ],
            well_bhp: Vec::new(),
            perforation_primaries: Vec::new(),
        };

        let derived_0 = state.derive_cell(&sim, 0);
        let derived_1 = state.derive_cell(&sim, 1);
        let legacy =
            assembly::interface_flux_terms(&sim, &state, 0, 1, 'x', 0, 0, &derived_0, &derived_1)
                .expect("nonzero transmissibility");

        let geom_t = DARCY_METRIC_FACTOR * sim.geometric_transmissibility(0, 1, 'x');
        let i = input(
            160.0,
            0.3,
            0.1,
            HydrocarbonState::Saturated,
            sim.depth_at_k(0),
        );
        let j = input(
            150.0,
            0.25,
            0.08,
            HydrocarbonState::Saturated,
            sim.depth_at_k(0),
        );
        let generic = face_flux_terms_generic(&sim, geom_t, &i, &j);

        for phase in 0..3 {
            assert!(
                (legacy.flux_sc_day[phase] - generic.flux_sc_day[phase]).abs() < 1e-12,
                "phase {phase}: legacy={} generic={}",
                legacy.flux_sc_day[phase],
                generic.flux_sc_day[phase]
            );
        }
    }

    /// AD-vs-numerical gate for the TWO-PHASE flux path (`three_phase_mode =
    /// false`) -- every prior flux gate used a three-phase fixture; this is
    /// the untested branch relevant to `water-pressure`-style scenarios.
    #[test]
    fn flux_gate_two_phase_with_capillary() {
        let sim = ReservoirSimulator::new(2, 1, 1, 0.2);
        assert!(!sim.three_phase_mode);
        let geom_t = 3.5_f64;
        let dt_days = 0.4_f64;

        let i = input(310.0, 0.15, 0.0, HydrocarbonState::Saturated, 0.0);
        let j = input(290.0, 0.12, 0.0, HydrocarbonState::Saturated, 0.0);

        let (bii, bij, bji, bjj) = face_flux_jacobian_blocks(&sim, geom_t, dt_days, &i, &j);
        let mut analytic = vec![vec![0.0; 6]; 6];
        for eq in 0..3 {
            for v in 0..3 {
                analytic[eq][v] = bii[eq][v];
                analytic[eq][3 + v] = bij[eq][v];
                analytic[3 + eq][v] = bji[eq][v];
                analytic[3 + eq][3 + v] = bjj[eq][v];
            }
        }

        let x0 = [i.p, i.sw, i.hydrocarbon_var, j.p, j.sw, j.hydrocarbon_var];
        let residual = |x: &[f64]| {
            let i2 = FaceCellInput {
                p: x[0],
                sw: x[1],
                hydrocarbon_var: x[2],
                ..i
            };
            let j2 = FaceCellInput {
                p: x[3],
                sw: x[4],
                hydrocarbon_var: x[5],
                ..j
            };
            face_flux_residual_f64(&sim, geom_t, dt_days, &i2, &j2).to_vec()
        };
        let numerical = central_difference_jacobian(&x0, 6, residual);

        assert_jacobian_matches(&analytic, &numerical, 1e-6, 1e-9);
    }
}
