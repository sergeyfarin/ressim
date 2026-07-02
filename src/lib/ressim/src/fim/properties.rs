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

    // Two-phase / no PVT table: no gas is present (the flash pins sg = 0 and
    // the state's hydrocarbon_var stays 0), but the third unknown must keep
    // LIVE derivatives (sg = hydrocarbon_var formally, as in the saturated
    // regime) or its Jacobian row and column are identically zero and the
    // global system is structurally singular — one empty row per cell. The
    // legacy assembler regularizes the same way: its accumulation block
    // applies the saturated-regime chain rule (d_sg/d_hc = 1 -> gas-row
    // diagonal pv/bg) even though its residual pins sg = 0. Along the actual
    // trajectory hc = 0, so the residual value is unchanged and Newton yields
    // delta_hc = 0 exactly; only the Jacobian structure differs.
    if !sim.three_phase_mode || sim.pvt_table.is_none() {
        let sg = hydrocarbon_var.max_floor(0.0).min_of(total_hc);
        let so = (one - sw - sg).max_floor(0.0);
        let bo = base_oil_fvf_generic(sim, p);
        return CellProps {
            so,
            sg,
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
            // Mirrors `flash::resolve_cell_flash`'s Undersaturated arm: below
            // the (possibly DRSDT0-capped) saturated Rs bound, oil simply
            // carries the trial Rs. Above it, excess dissolved gas flashes to
            // free gas via `split_gas_inventory_after_transport` (which can
            // itself flip the physical regime to Saturated -- that flip is
            // resolved by `classify_regimes` between Newton iterations, not
            // here; this function only needs the resulting so/sg/rs).
            let rs_cap_base = table.interpolate_saturated_generic(p).rs.max_floor(0.0);
            let rs_cap = match drsdt0_base_rs {
                Some(base_rs) => rs_cap_base.min_of(S::from_f64(base_rs.max(0.0))),
                None => rs_cap_base,
            };
            let rs_trial = hydrocarbon_var.max_floor(0.0);

            let (sg, so, rs) = if rs_trial.value() <= rs_cap.value() + 1e-6 {
                (S::from_f64(0.0), total_hc, rs_trial)
            } else {
                let (bo_trial, _mu_o) = table.interpolate_oil_generic(p, rs_trial);
                let bo_trial = bo_trial.max_floor(1e-9);
                let dissolved_gas_sc = total_hc * rs_trial / bo_trial;
                // `split_gas_inventory_after_transport_generic` returns
                // `(sg, so, rs)`, matching `split_gas_inventory_after_transport`.
                crate::fim::flash_ad::split_gas_inventory_after_transport_generic(
                    sim,
                    p,
                    S::from_f64(1.0),
                    sw,
                    S::from_f64(0.0),
                    dissolved_gas_sc,
                    drsdt0_base_rs,
                )
            };

            // Bo/Bg are always read off the FINAL (post-flash) Rs, matching
            // `state::FimState::derive_cell`'s `oil_props_for_state(p, flash.rs)`.
            let (bo, _mu_o) = table.interpolate_oil_generic(p, rs);
            let bg = table.interpolate_saturated_generic(p).bg;
            CellProps { so, sg, rs, bo, bg }
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
    // own reference (delta 0 -> pv = pv_ref). `derive_cell` derives the
    // DRSDT0 cap from `sim.rs[idx]` -- a simulator-level constant, not either
    // state's own hydrocarbon_var -- so the SAME `drsdt0_base_rs` the caller
    // passed in for the current point applies unchanged to the previous point.
    let prev_props = cell_props_generic::<f64>(
        sim,
        prev_regime,
        prev_p,
        prev_sw,
        prev_hydrocarbon_var,
        drsdt0_base_rs,
    );
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

    fn accumulation_gate_with(
        sim: &ReservoirSimulator,
        regime: HydrocarbonState,
        p: f64,
        sw: f64,
        hc_var: f64,
        drsdt0: Option<f64>,
    ) {
        // Previous iterate: perturb slightly so accumulation is nonzero but the
        // current iterate stays interior to its branch.
        let prev_p = p - 5.0;
        let prev_sw = sw - 0.01;
        let prev_hc = hc_var * 0.7;

        let analytic = accumulation_jacobian_block(
            sim, 0, p, sw, hc_var, regime, drsdt0, prev_p, prev_sw, prev_hc, regime,
        );

        let residual = |x: &[f64]| {
            let acc = cell_accumulation_generic::<f64>(
                sim, 0, x[0], x[1], x[2], regime, drsdt0, prev_p, prev_sw, prev_hc, regime,
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

    fn accumulation_gate(regime: HydrocarbonState, p: f64, sw: f64, hc_var: f64) {
        let sim = three_phase_sim();
        // gas redissolution on (default) -> uncapped Rs_sat, clean smooth branch
        accumulation_gate_with(&sim, regime, p, sw, hc_var, None);
    }

    #[test]
    fn ad_accumulation_matches_numerical_saturated() {
        accumulation_gate(HydrocarbonState::Saturated, 150.0, 0.2, 0.1);
    }

    #[test]
    fn ad_accumulation_matches_numerical_undersaturated() {
        accumulation_gate(HydrocarbonState::Undersaturated, 150.0, 0.2, 12.0);
    }

    /// AD-vs-numerical gate for the undersaturated excess-gas-flash branch
    /// (trial Rs above its DRSDT0-capped bound), which routes through
    /// `flash_ad::split_gas_inventory_after_transport_generic` and its
    /// implicit-function-theorem-differentiated internal bisection. This is
    /// the branch that caught a real tuple-order bug during development
    /// (`(sg, so, rs)` mis-destructured as `(so, sg, rs)`), so this gate
    /// specifically targets the derivative, not just the value.
    #[test]
    fn ad_accumulation_matches_numerical_undersaturated_excess_flash() {
        let mut sim = three_phase_sim();
        sim.set_gas_redissolution_enabled(false);
        // p=150 -> rs_sat=15; cap=11 well below both rs_sat and the trial Rs=12,
        // safely inside the excess-flash branch (away from the rs_trial==cap kink).
        accumulation_gate_with(
            &sim,
            HydrocarbonState::Undersaturated,
            150.0,
            0.2,
            12.0,
            Some(11.0),
        );
    }

    /// Bit-parity gate for the DRSDT0 (`gas_redissolution_enabled = false`)
    /// path: `FimState::derive_cell` caps `Rs` against `sim.rs[idx]` -- a
    /// simulator-level constant shared by BOTH the current and previous
    /// accumulation evaluations, not a value derived from either state's own
    /// hydrocarbon_var. An earlier draft of `cell_accumulation_generic` used
    /// `prev_hydrocarbon_var.max(0.0)` as the previous-point cap instead,
    /// which is wrong whenever `prev_hydrocarbon_var != sim.rs[idx]`. This
    /// test exercises exactly that path against the real
    /// `cell_equation_residual_breakdown` on an actual
    /// `ReservoirSimulator`/`FimWellTopology`.
    #[test]
    fn generic_accumulation_matches_real_residual_with_drsdt0_disabled() {
        use crate::fim::assembly::cell_equation_residual_breakdown;
        use crate::fim::wells::build_well_topology;

        let mut sim = three_phase_sim();
        sim.set_gas_redissolution_enabled(false);
        // Base Rs cap distinct from either state's own hydrocarbon_var, so a
        // self-derived cap would diverge from the real `sim.rs[idx]` cap.
        sim.rs[0] = 11.0;

        let previous_state = FimState {
            cells: vec![FimCellState {
                pressure_bar: 145.0,
                sw: 0.18,
                hydrocarbon_var: 9.0,
                regime: HydrocarbonState::Undersaturated,
            }],
            well_bhp: Vec::new(),
            perforation_rates_m3_day: Vec::new(),
        };
        let state = FimState {
            cells: vec![FimCellState {
                pressure_bar: 150.0,
                sw: 0.2,
                hydrocarbon_var: 12.0,
                regime: HydrocarbonState::Undersaturated,
            }],
            well_bhp: Vec::new(),
            perforation_rates_m3_day: Vec::new(),
        };
        let topology = build_well_topology(&sim);
        let dt_days = 0.5;

        for component in 0..3 {
            let real = cell_equation_residual_breakdown(
                &sim,
                &previous_state,
                &state,
                &topology,
                dt_days,
                0,
                component,
            )
            .unwrap()
            .accumulation;

            let drsdt0 = Some(sim.rs[0]);
            let generic = cell_accumulation_generic::<f64>(
                &sim,
                0,
                state.cells[0].pressure_bar,
                state.cells[0].sw,
                state.cells[0].hydrocarbon_var,
                state.cells[0].regime,
                drsdt0,
                previous_state.cells[0].pressure_bar,
                previous_state.cells[0].sw,
                previous_state.cells[0].hydrocarbon_var,
                previous_state.cells[0].regime,
            )[component];

            assert!(
                (real - generic).abs() < 1e-10,
                "component {component}: real={real} generic={generic}"
            );
        }
    }

    /// AD-vs-numerical gate for the TWO-PHASE path (`three_phase_mode = false`,
    /// no PVT table) -- every prior Phase 1/2 gate used a three-phase fixture,
    /// so this branch (`cell_props_generic`'s first `if` arm, and everything
    /// downstream in flux/mobility that special-cases `!three_phase_mode`) was
    /// never checked against numerical differentiation.
    ///
    /// Evaluated at hc = 0.05, interior to the live branch: hc = 0 (the
    /// actual two-phase operating point) sits exactly on the `max_floor(0.0)`
    /// clamp kink of the inactive-unknown regularization, where a central
    /// difference straddles the branch switch and reports half the one-sided
    /// derivative AD (correctly, matching the legacy assembler) selects. The
    /// hc = 0 point itself is covered by
    /// `assembly_ad::two_phase_singularity_check`, which pins the row/column
    /// structure against the legacy Jacobian instead of against a numerical
    /// difference.
    #[test]
    fn ad_accumulation_matches_numerical_two_phase() {
        let sim = ReservoirSimulator::new(1, 1, 1, 0.2);
        assert!(!sim.three_phase_mode);
        accumulation_gate_with(&sim, HydrocarbonState::Saturated, 300.0, 0.2, 0.05, None);
    }
}

