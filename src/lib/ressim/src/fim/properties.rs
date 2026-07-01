//! Generic (differentiable) cell property and accumulation evaluation.
//!
//! Every quantity is written once over the [`Scalar`] trait, so the same code
//! produces the plain `f64` residual and — when instantiated with `Ad<3>` seeded
//! against a cell's own primary variables `[p, sw, hydrocarbon_var]` — the exact
//! diagonal accumulation Jacobian block. This is the AD counterpart of the flash
//! + PVT lookups in `state::derive_cell` and the accumulation term in `assembly`.
//!
//! The hydrocarbon phase regime is frozen within a Newton iteration (matching
//! `apply_newton_update_frozen`), so the regime-dependent branch is selected once
//! from the iterate and differentiated through smoothly.

#![allow(dead_code)]

use crate::ReservoirSimulator;
use crate::fim::ad::{Ad, Scalar};
use crate::fim::state::HydrocarbonState;

/// Derived cell fluid properties as differentiable scalars.
pub(crate) struct CellProps<S> {
    pub(crate) so: S,
    pub(crate) sg: S,
    pub(crate) rs: S,
    pub(crate) bo: S,
    pub(crate) bg: S,
}

/// Generic mirror of `state::derive_cell` restricted to the fields the mass
/// balance needs (saturations, dissolved gas, and oil/gas FVFs).
///
/// `p`, `sw`, `hydrocarbon_var` are the cell primary variables in the chosen
/// scalar type. The regime is frozen; the undersaturated overflow flash split
/// (`rs` above the saturated cap producing free gas) is intentionally not
/// modelled here — `classify_regimes` moves such cells to the saturated regime
/// between Newton iterations, and the Jacobian gate evaluates inside the clean
/// branch. Full-assembly overflow handling is wired in the full-AD phase.
pub(crate) fn cell_props_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    regime: HydrocarbonState,
    p: S,
    sw: S,
    hydrocarbon_var: S,
    drsdt0_base_rs: Option<f64>,
) -> CellProps<S> {
    let one = S::from_f64(1.0);
    let total_hc = (one - sw).max_floor(0.0);

    // Two-phase / no PVT table: oil fills the hydrocarbon pore space, no gas.
    if !sim.three_phase_mode || sim.pvt_table.is_none() {
        let bo = base_oil_fvf_generic(sim, p);
        return CellProps {
            so: total_hc,
            sg: S::from_f64(0.0),
            rs: S::from_f64(0.0),
            bo,
            bg: S::from_f64(1.0),
        };
    }

    let table = sim.pvt_table.as_ref().expect("checked above");

    match regime {
        HydrocarbonState::Saturated => {
            let sg = hydrocarbon_var.max_floor(0.0).min_of(total_hc);
            let mut rs = table.interpolate_saturated_generic(p).rs;
            if let Some(base_rs) = drsdt0_base_rs {
                rs = rs.min_ceil(base_rs);
            }
            let so = (one - sw - sg).max_floor(0.0);
            let (bo, _mu_o) = table.interpolate_oil_generic(p, rs);
            let bg = table.interpolate_saturated_generic(p).bg;
            CellProps { so, sg, rs, bo, bg }
        }
        HydrocarbonState::Undersaturated => {
            let so = total_hc;
            let rs = hydrocarbon_var.max_floor(0.0);
            let (bo, _mu_o) = table.interpolate_oil_generic(p, rs);
            let bg = table.interpolate_saturated_generic(p).bg;
            CellProps {
                so,
                sg: S::from_f64(0.0),
                rs,
                bo,
                bg,
            }
        }
    }
}

fn base_oil_fvf_generic<S: Scalar>(sim: &ReservoirSimulator, p: S) -> S {
    // (b_o * exp(-c_o * p)).max(1e-9)
    (S::from_f64(sim.b_o) * (p * (-sim.pvt.c_o)).exp()).max_floor(1e-9)
}

/// Pore volume at the current pressure with rock compressibility, generic over `S`.
/// `p_prev` is the committed previous-iterate pressure (constant w.r.t. current
/// unknowns).
pub(crate) fn pore_volume_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    cell_idx: usize,
    p: S,
    p_prev: f64,
) -> S {
    let pore_volume_ref_m3 = sim.pore_volume_m3(cell_idx);
    // pv_ref * exp(rock_comp * (p - p_prev))
    S::from_f64(pore_volume_ref_m3) * ((p - p_prev) * sim.rock_compressibility).exp()
}

/// Standard-condition component inventory `[water, oil, gas]` for one cell,
/// generic over `S`. Mirrors `assembly::cell_component_inventory_sc`.
pub(crate) fn component_inventory_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    pore_volume: S,
    sw: S,
    props: &CellProps<S>,
) -> [S; 3] {
    let water_sc = pore_volume * sw / sim.b_w.max(1e-9);
    let oil_sc = pore_volume * props.so / props.bo.max_floor(1e-9);
    let gas_sc = pore_volume * props.sg / props.bg.max_floor(1e-9) + oil_sc * props.rs;
    [water_sc, oil_sc, gas_sc]
}

/// Accumulation residual `[water, oil, gas]` for one cell in scalar type `S`,
/// i.e. `current_inventory(S) - previous_inventory(const)`. The previous-iterate
/// inventory is independent of the current unknowns, so it enters as a constant.
pub(crate) fn cell_accumulation_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    cell_idx: usize,
    // current iterate (differentiated)
    p: S,
    sw: S,
    hydrocarbon_var: S,
    regime: HydrocarbonState,
    drsdt0_base_rs: Option<f64>,
    // previous committed iterate (constant)
    prev_p: f64,
    prev_sw: f64,
    prev_hydrocarbon_var: f64,
    prev_regime: HydrocarbonState,
) -> [S; 3] {
    let props = cell_props_generic(sim, regime, p, sw, hydrocarbon_var, drsdt0_base_rs);
    let pv = pore_volume_generic(sim, cell_idx, p, prev_p);
    let current = component_inventory_generic(sim, pv, sw, &props);

    // Previous inventory: same code, f64 instantiation, previous pressure is its
    // own reference (delta 0 -> pv = pv_ref).
    let prev_props =
        cell_props_generic::<f64>(sim, prev_regime, prev_p, prev_sw, prev_hydrocarbon_var, {
            if !sim.gas_redissolution_enabled {
                Some(prev_hydrocarbon_var.max(0.0))
            } else {
                None
            }
        });
    let prev_pv = pore_volume_generic::<f64>(sim, cell_idx, prev_p, prev_p);
    let previous = component_inventory_generic::<f64>(sim, prev_pv, prev_sw, &prev_props);

    [
        current[0] - previous[0],
        current[1] - previous[1],
        current[2] - previous[2],
    ]
}

/// Assemble the 3x3 diagonal accumulation Jacobian block for one cell by seeding
/// the cell's own primaries as `Ad<3>` and reading off the partials.
///
/// Returns `block[eq][var]` where `eq`/`var` index `[water/oil/gas]` equations
/// and `[p, sw, hydrocarbon_var]` unknowns.
pub(crate) fn accumulation_jacobian_block(
    sim: &ReservoirSimulator,
    cell_idx: usize,
    p: f64,
    sw: f64,
    hydrocarbon_var: f64,
    regime: HydrocarbonState,
    drsdt0_base_rs: Option<f64>,
    prev_p: f64,
    prev_sw: f64,
    prev_hydrocarbon_var: f64,
    prev_regime: HydrocarbonState,
) -> [[f64; 3]; 3] {
    let p_ad = Ad::<3>::variable(p, 0);
    let sw_ad = Ad::<3>::variable(sw, 1);
    let hc_ad = Ad::<3>::variable(hydrocarbon_var, 2);

    let acc = cell_accumulation_generic(
        sim,
        cell_idx,
        p_ad,
        sw_ad,
        hc_ad,
        regime,
        drsdt0_base_rs,
        prev_p,
        prev_sw,
        prev_hydrocarbon_var,
        prev_regime,
    );

    let mut block = [[0.0; 3]; 3];
    for (eq, entry) in acc.iter().enumerate() {
        block[eq] = *entry.deriv();
    }
    block
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fim::numjac::{assert_jacobian_matches, central_difference_jacobian};
    use crate::fim::state::{FimCellState, FimState};
    use crate::pvt::{PvtRow, PvtTable};

    fn three_phase_sim() -> ReservoirSimulator {
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
        sim
    }

    /// The generic f64 property path must reproduce `state::derive_cell` so the
    /// AD path shares a single source of truth with the existing residual.
    #[test]
    fn generic_f64_props_match_derive_cell() {
        let sim = three_phase_sim();

        for (regime, hc_var) in [
            (HydrocarbonState::Saturated, 0.1),
            (HydrocarbonState::Undersaturated, 12.0),
        ] {
            let state = FimState {
                cells: vec![FimCellState {
                    pressure_bar: 150.0,
                    sw: 0.2,
                    hydrocarbon_var: hc_var,
                    regime,
                }],
                well_bhp: Vec::new(),
                perforation_rates_m3_day: Vec::new(),
            };
            let derived = state.derive_cell(&sim, 0);
            let drsdt0 = if !sim.gas_redissolution_enabled {
                Some(sim.rs[0])
            } else {
                None
            };
            let props = cell_props_generic::<f64>(&sim, regime, 150.0, 0.2, hc_var, drsdt0);

            assert!((props.so - derived.so).abs() < 1e-12, "so {regime:?}");
            assert!((props.sg - derived.sg).abs() < 1e-12, "sg {regime:?}");
            assert!((props.rs - derived.rs).abs() < 1e-12, "rs {regime:?}");
            assert!((props.bo - derived.bo).abs() < 1e-12, "bo {regime:?}");
            assert!((props.bg - derived.bg).abs() < 1e-12, "bg {regime:?}");
        }
    }

    fn accumulation_gate(regime: HydrocarbonState, p: f64, sw: f64, hc_var: f64) {
        let sim = three_phase_sim();
        let drsdt0 = None; // gas redissolution on -> uncapped Rs_sat, clean smooth branch

        // Previous iterate: perturb slightly so accumulation is nonzero but the
        // current iterate stays interior to its branch.
        let prev_p = p - 5.0;
        let prev_sw = sw - 0.01;
        let prev_hc = hc_var * 0.95;

        let analytic = accumulation_jacobian_block(
            &sim, 0, p, sw, hc_var, regime, drsdt0, prev_p, prev_sw, prev_hc, regime,
        );

        let residual = |x: &[f64]| {
            let acc = cell_accumulation_generic::<f64>(
                &sim, 0, x[0], x[1], x[2], regime, drsdt0, prev_p, prev_sw, prev_hc, regime,
            );
            acc.to_vec()
        };
        let numerical = central_difference_jacobian(&[p, sw, hc_var], 3, residual);

        assert_jacobian_matches(
            &analytic.iter().map(|r| r.to_vec()).collect::<Vec<_>>(),
            &numerical,
            1e-6,
            1e-9,
        );
    }

    #[test]
    fn ad_accumulation_matches_numerical_saturated() {
        accumulation_gate(HydrocarbonState::Saturated, 150.0, 0.2, 0.1);
    }

    #[test]
    fn ad_accumulation_matches_numerical_undersaturated() {
        accumulation_gate(HydrocarbonState::Undersaturated, 150.0, 0.2, 12.0);
    }
}
