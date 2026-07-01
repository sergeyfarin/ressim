//! Generic (differentiable) well/perforation residual evaluation.
//!
//! Mirrors the hand-derivative helpers in `wells.rs`
//! (`connection_rate_for_bhp`, `perforation_component_rate_derivatives_sc_day`,
//! `perforation_target_rate_derivative`) written once over [`Scalar`], so a
//! single evaluation with `Ad<5>` — seeded against `[p, sw, hydrocarbon_var,
//! well_bhp, perforation_rate]` — yields the exact Jacobian for:
//!
//! 1. the perforation rate-consistency row (`q - connection_rate(p, bhp)`),
//! 2. the well control-constraint row (BHP target or rate target, surface or
//!    reservoir-volume), and
//! 3. the well-source contribution to the connected cell's three mass-balance
//!    equations (`coefficient(cell_state) * q` per phase).
//!
//! The producer phase-fraction split is genuinely linear in `q` for a frozen
//! cell state (that's why `wells.rs` can reuse the same coefficients as both
//! a rate and a derivative), so AD's ordinary product rule reproduces the
//! well's bilinear structure without any special-casing.

#![allow(dead_code)]

use crate::InjectedFluid;
use crate::ReservoirSimulator;
use crate::fim::ad::{Ad, Scalar};
use crate::fim::properties::cell_props_generic;
use crate::fim::state::HydrocarbonState;

/// One connected cell's primary-variable inputs to a well/perforation residual.
#[derive(Clone, Copy)]
pub(crate) struct WellCellInput<S> {
    pub(crate) p: S,
    pub(crate) sw: S,
    pub(crate) hydrocarbon_var: S,
    pub(crate) regime: HydrocarbonState,
    pub(crate) drsdt0_base_rs: Option<f64>,
}

/// Aggregated producer phase-mobility fractions over the perforation's control
/// neighborhood (generic mirror of `wells::producer_control_state`'s fraction
/// computation).
#[derive(Clone, Copy)]
pub(crate) struct ProducerFractionsGeneric<S> {
    pub(crate) water_fraction: S,
    pub(crate) oil_fraction: S,
    pub(crate) gas_fraction: S,
}

/// Generic mirror of `wells::producer_control_state`'s neighborhood mobility
/// sum, generalized to an arbitrary neighborhood width (the real control
/// neighborhood is a 3x3 cell window; the Phase 3 gate case uses a single
/// connected cell, which is the width-1 instance of this same function).
pub(crate) fn producer_fractions_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    neighborhood: &[WellCellInput<S>],
) -> ProducerFractionsGeneric<S> {
    let mut lambda_w = S::from_f64(0.0);
    let mut lambda_o = S::from_f64(0.0);
    let mut lambda_g = S::from_f64(0.0);

    for cell in neighborhood {
        let props = cell_props_generic(
            sim,
            cell.regime,
            cell.p,
            cell.sw,
            cell.hydrocarbon_var,
            cell.drsdt0_base_rs,
        );
        let mob = sim.phase_mobilities_for_state_generic(cell.sw, props.sg, cell.p, props.rs);
        lambda_w = lambda_w + mob.water.max_floor(0.0);
        lambda_o = lambda_o + mob.oil.max_floor(0.0);
        lambda_g = lambda_g + mob.gas.max_floor(0.0);
    }

    let lambda_total = (lambda_w + lambda_o + lambda_g).max_floor(f64::EPSILON);
    ProducerFractionsGeneric {
        water_fraction: (lambda_w / lambda_total).max_floor(0.0).min_ceil(1.0),
        oil_fraction: (lambda_o / lambda_total).max_floor(0.0).min_ceil(1.0),
        gas_fraction: (lambda_g / lambda_total).max_floor(0.0).min_ceil(1.0),
    }
}

/// Exact Jacobian of the aggregated producer fractions w.r.t. ONE neighbor
/// cell's own `[p, sw, hydrocarbon_var]`, holding the rest of the
/// neighborhood at its plain forward-evaluated value.
///
/// This reproduces the same result a single wide differentiation over the
/// whole neighborhood would give, without needing a dual number wider than
/// `Ad<3>`: `producer_fractions_generic`'s lambda sums are linear across
/// neighborhood cells (`lambda_x_sum = sum_i g_i(vars_i)`), so the partial
/// derivative w.r.t. one cell's vars depends only on that cell's own term —
/// injecting the other cells as `Ad::constant` (zero derivative, correct
/// forward value) is mathematically exact, not an approximation. Assembling
/// one such 3x3 block per neighbor and scattering each into its own column
/// range of the global Jacobian reconstructs the full-neighborhood Jacobian
/// exactly (verified in `neighbor_blocks_reconstruct_full_neighborhood_jacobian`
/// below against a numerical Jacobian over the whole stacked neighborhood).
pub(crate) fn producer_fractions_neighbor_block(
    sim: &ReservoirSimulator,
    neighborhood: &[WellCellInput<f64>],
    neighbor_idx: usize,
) -> (ProducerFractionsGeneric<f64>, [[f64; 3]; 3]) {
    let seeded: Vec<WellCellInput<Ad<3>>> = neighborhood
        .iter()
        .enumerate()
        .map(|(idx, c)| {
            if idx == neighbor_idx {
                WellCellInput {
                    p: Ad::<3>::variable(c.p, 0),
                    sw: Ad::<3>::variable(c.sw, 1),
                    hydrocarbon_var: Ad::<3>::variable(c.hydrocarbon_var, 2),
                    regime: c.regime,
                    drsdt0_base_rs: c.drsdt0_base_rs,
                }
            } else {
                WellCellInput {
                    p: Ad::<3>::constant(c.p),
                    sw: Ad::<3>::constant(c.sw),
                    hydrocarbon_var: Ad::<3>::constant(c.hydrocarbon_var),
                    regime: c.regime,
                    drsdt0_base_rs: c.drsdt0_base_rs,
                }
            }
        })
        .collect();

    let fractions_ad = producer_fractions_generic(sim, &seeded);
    let fractions_f64 = ProducerFractionsGeneric {
        water_fraction: fractions_ad.water_fraction.value(),
        oil_fraction: fractions_ad.oil_fraction.value(),
        gas_fraction: fractions_ad.gas_fraction.value(),
    };
    let block = [
        *fractions_ad.water_fraction.deriv(),
        *fractions_ad.oil_fraction.deriv(),
        *fractions_ad.gas_fraction.deriv(),
    ];
    (fractions_f64, block)
}

/// Generic mirror of `wells::connection_rate_for_bhp`.
pub(crate) fn connection_rate_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    wi_geom: f64,
    injector: bool,
    cell: &WellCellInput<S>,
    bhp: S,
) -> S {
    let props = cell_props_generic(
        sim,
        cell.regime,
        cell.p,
        cell.sw,
        cell.hydrocarbon_var,
        cell.drsdt0_base_rs,
    );
    let mob = sim.phase_mobilities_for_state_generic(cell.sw, props.sg, cell.p, props.rs);
    let connection_mobility = (mob.water + mob.oil + mob.gas).max_floor(0.0);
    let raw_rate = (connection_mobility * (cell.p - bhp)) * wi_geom;

    if injector {
        raw_rate.min_ceil(0.0)
    } else {
        raw_rate.max_floor(0.0)
    }
}

/// Generic mirror of `wells::perforation_component_rate_derivatives_sc_day`'s
/// per-phase coefficient — the linear-in-`q` factor `component_rate[phase] =
/// coefficient[phase] * q` for a frozen cell state.
pub(crate) fn component_rate_coefficients_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    injector: bool,
    injected_fluid: InjectedFluid,
    cell: &WellCellInput<S>,
    fractions: Option<&ProducerFractionsGeneric<S>>,
) -> [S; 3] {
    let props = cell_props_generic(
        sim,
        cell.regime,
        cell.p,
        cell.sw,
        cell.hydrocarbon_var,
        cell.drsdt0_base_rs,
    );

    if injector {
        return match injected_fluid {
            InjectedFluid::Water => [
                S::from_f64(1.0 / sim.b_w.max(1e-9)),
                S::from_f64(0.0),
                S::from_f64(0.0),
            ],
            InjectedFluid::Gas => [
                S::from_f64(0.0),
                S::from_f64(0.0),
                props.bg.max_floor(1e-9).recip(),
            ],
        };
    }

    let fractions = fractions.expect("producer coefficients require aggregated fractions");
    let bo_safe = props.bo.max_floor(1e-9);
    let bg_safe = props.bg.max_floor(1e-9);
    let oil_coef = fractions.oil_fraction / bo_safe;
    [
        fractions.water_fraction / sim.b_w.max(1e-9),
        oil_coef,
        fractions.gas_fraction / bg_safe + oil_coef * props.rs,
    ]
}

/// Generic mirror of `wells::perforation_surface_rate_sc_day`'s CLAMPED rate
/// used by the well-level rate constraint. Distinct from the unclamped
/// `component_rate_coefficients_generic` used for the mass-balance source
/// term: here `q`'s "wrong-sign" contribution is truncated to zero before
/// converting to surface volume, matching the production formula exactly —
/// `(-q).max(0.0) / b_w_or_bg` for an injector, `q.max(0.0) * oil_fraction /
/// oil_fvf` for a producer.
pub(crate) fn perforation_surface_rate_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    injector: bool,
    injected_fluid: InjectedFluid,
    cell: &WellCellInput<S>,
    fractions: Option<&ProducerFractionsGeneric<S>>,
    q: S,
) -> S {
    let props = cell_props_generic(
        sim,
        cell.regime,
        cell.p,
        cell.sw,
        cell.hydrocarbon_var,
        cell.drsdt0_base_rs,
    );

    if injector {
        let clamped = (-q).max_floor(0.0);
        return match injected_fluid {
            InjectedFluid::Water => clamped / sim.b_w.max(1e-9),
            InjectedFluid::Gas => clamped / props.bg.max_floor(1e-9),
        };
    }

    let fractions = fractions.expect("producer surface rate requires aggregated fractions");
    q.max_floor(0.0) * fractions.oil_fraction / props.bo.max_floor(1e-9)
}

/// Generic mirror of `FimWellLocalBlock::total_rate_from_unknowns`'s
/// reservoir-volume-target branch: `(-q).max(0.0)` for an injector,
/// `q.max(0.0)` for a producer.
pub(crate) fn perforation_reservoir_rate_generic<S: Scalar>(injector: bool, q: S) -> S {
    if injector {
        (-q).max_floor(0.0)
    } else {
        q.max_floor(0.0)
    }
}

/// One perforation's inputs to a well-level rate aggregation.
pub(crate) struct WellPerforationInputGeneric<S> {
    pub(crate) cell: WellCellInput<S>,
    /// Producer perforations only (mirrors `producer_control_state`, computed
    /// per-perforation from that perforation's own control neighborhood).
    pub(crate) fractions: Option<ProducerFractionsGeneric<S>>,
    pub(crate) q: S,
}

/// Generic mirror of `FimWellLocalBlock::total_rate_from_unknowns`: sums each
/// perforation's clamped surface- or reservoir-volume rate across the whole
/// physical well.
pub(crate) fn well_actual_rate_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    injector: bool,
    injected_fluid: InjectedFluid,
    uses_surface_target: bool,
    perforations: &[WellPerforationInputGeneric<S>],
) -> S {
    let mut total = S::from_f64(0.0);
    for perf in perforations {
        total = total
            + if uses_surface_target {
                perforation_surface_rate_generic(
                    sim,
                    injector,
                    injected_fluid,
                    &perf.cell,
                    perf.fractions.as_ref(),
                    perf.q,
                )
            } else {
                perforation_reservoir_rate_generic(injector, perf.q)
            };
    }
    total
}

/// Generic mirror of `wells::fischer_burmeister`.
pub(crate) fn fischer_burmeister_generic<S: Scalar>(a: S, b: S) -> S {
    let eps2_2 = 2.0 * crate::fim::wells::FB_EPSILON * crate::fim::wells::FB_EPSILON;
    let norm = ((a * a + b * b) + eps2_2).sqrt();
    norm - a - b
}

/// Well control target values (generic mirror of `wells::PhysicalWellControl`,
/// minus the `enabled`/`rate_controlled` gating already resolved by the
/// caller into which branch of `well_constraint_residual_fb_generic` runs).
pub(crate) struct WellControlValuesGeneric {
    pub(crate) enabled: bool,
    pub(crate) rate_controlled: bool,
    pub(crate) uses_surface_target: bool,
    pub(crate) target_rate: Option<f64>,
    pub(crate) bhp_limit: f64,
    pub(crate) bhp_target: f64,
}

/// Generic mirror of `FimWellLocalBlock::constraint_residual`: BHP-controlled
/// (or disabled) wells pin `bhp == bhp_target`; rate-controlled wells use the
/// Fischer-Burmeister complementarity reformulation of "BHP is at its limit OR
/// the aggregated well rate hits target" — a smooth stand-in for the
/// active-set switch between rate and BHP control, matching `wells.rs`'s
/// scaling exactly (`bhp_limit.abs().max(1.0)`, `target_rate.abs().max(1.0)`).
pub(crate) fn well_constraint_residual_fb_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    injector: bool,
    injected_fluid: InjectedFluid,
    control: &WellControlValuesGeneric,
    bhp: S,
    perforations: &[WellPerforationInputGeneric<S>],
) -> Option<S> {
    if !control.enabled || !control.rate_controlled {
        return Some(bhp - control.bhp_target);
    }

    let target_rate = control.target_rate?;
    let actual_rate = well_actual_rate_generic(
        sim,
        injector,
        injected_fluid,
        control.uses_surface_target,
        perforations,
    );
    let bhp_slack = if injector {
        S::from_f64(control.bhp_limit) - bhp
    } else {
        bhp - control.bhp_limit
    };
    let rate_slack = S::from_f64(target_rate) - actual_rate;

    let bhp_scale = control.bhp_limit.abs().max(1.0);
    let rate_scale = target_rate.abs().max(1.0);
    Some(fischer_burmeister_generic(
        bhp_slack / bhp_scale,
        rate_slack / rate_scale,
    ))
}

// ---------------------------------------------------------------------------
// Grid-wide Jacobian-block extraction for the full assembler.
//
// A well/perforation row can, for producers, depend on cells OUTSIDE the
// connected cell (the control neighborhood feeding `producer_fractions_generic`).
// Rather than a dual number wide enough for the whole neighborhood, every
// function below follows the same two-part pattern proven in
// `neighbor_blocks_reconstruct_full_neighborhood_jacobian`:
//
//   "own" block:      the connected cell's combined direct (via its own
//                      Bo/Bg/Rs) + indirect (via its own contribution to the
//                      neighborhood sum) derivative, obtained by seeding the
//                      connected cell as Ad<4> = [p, sw, hydrocarbon_var, q]
//                      and re-using it as the active neighborhood member.
//
//   "neighbor" block:  an OTHER neighbor's derivative, which (because Bo/Bg/Rs
//                      at the connected cell don't depend on other cells at
//                      all) only flows through the fraction sum -- obtained
//                      via `producer_fractions_neighbor_block` and the same
//                      quotient-rule combination as `component_rate_coefficients_generic`.
// ---------------------------------------------------------------------------

/// Seeds the connected cell as `Ad<4>` (slots 0-2 = its own primaries, slot 3
/// reserved for `q`) and, if a producer control neighborhood is given,
/// recomputes fractions with that SAME `Ad<4>` value standing in for the
/// connected cell's position in the neighborhood (other neighbors held
/// constant) -- giving fractions whose slot 0-2 derivatives already combine
/// both the direct and indirect paths through the connected cell.
fn fractions_with_connected_cell_active(
    sim: &ReservoirSimulator,
    cell_ad: WellCellInput<Ad<4>>,
    producer_neighborhood: Option<(&[WellCellInput<f64>], usize)>,
) -> Option<ProducerFractionsGeneric<Ad<4>>> {
    producer_neighborhood.map(|(neighborhood, connected_index)| {
        let seeded: Vec<WellCellInput<Ad<4>>> = neighborhood
            .iter()
            .enumerate()
            .map(|(idx, c)| {
                if idx == connected_index {
                    cell_ad
                } else {
                    WellCellInput {
                        p: Ad::<4>::constant(c.p),
                        sw: Ad::<4>::constant(c.sw),
                        hydrocarbon_var: Ad::<4>::constant(c.hydrocarbon_var),
                        regime: c.regime,
                        drsdt0_base_rs: c.drsdt0_base_rs,
                    }
                }
            })
            .collect();
        producer_fractions_generic(sim, &seeded)
    })
}

fn cell_as_ad4(cell: &WellCellInput<f64>) -> WellCellInput<Ad<4>> {
    WellCellInput {
        p: Ad::<4>::variable(cell.p, 0),
        sw: Ad::<4>::variable(cell.sw, 1),
        hydrocarbon_var: Ad::<4>::variable(cell.hydrocarbon_var, 2),
        regime: cell.regime,
        drsdt0_base_rs: cell.drsdt0_base_rs,
    }
}

/// Jacobian of the perforation rate-consistency row (`q - connection_rate`)
/// w.r.t. its connected cell's own `[p, sw, hydrocarbon_var]` and the well's
/// `bhp`. The row's derivative w.r.t. its own `q` is exactly `1.0` and is not
/// part of this block (added directly by the caller).
pub(crate) fn rate_consistency_cell_bhp_jacobian(
    sim: &ReservoirSimulator,
    wi_geom: f64,
    injector: bool,
    cell: &WellCellInput<f64>,
    bhp: f64,
) -> ([f64; 3], f64) {
    let cell_ad = cell_as_ad4(cell);
    let bhp_ad = Ad::<4>::variable(bhp, 3);
    let connection = connection_rate_generic(sim, wi_geom, injector, &cell_ad, bhp_ad);
    let d = connection.deriv();
    ([-d[0], -d[1], -d[2]], -d[3])
}

/// Jacobian of one perforation's `[water, oil, gas]` mass-balance source
/// terms w.r.t. its connected cell's own `[p, sw, hydrocarbon_var]` and its
/// own perforation rate `q` (the combined "own" block; see module note above).
pub(crate) fn mass_balance_own_jacobian(
    sim: &ReservoirSimulator,
    injector: bool,
    injected_fluid: InjectedFluid,
    cell: &WellCellInput<f64>,
    producer_neighborhood: Option<(&[WellCellInput<f64>], usize)>,
    q: f64,
) -> [[f64; 4]; 3] {
    let cell_ad = cell_as_ad4(cell);
    let q_ad = Ad::<4>::variable(q, 3);
    let fractions_ad = fractions_with_connected_cell_active(sim, cell_ad, producer_neighborhood);

    let coefficients =
        component_rate_coefficients_generic(sim, injector, injected_fluid, &cell_ad, fractions_ad.as_ref());

    let mut block = [[0.0; 4]; 3];
    for (phase, coef) in coefficients.iter().enumerate() {
        block[phase] = *(*coef * q_ad).deriv();
    }
    block
}

/// Cross-derivative of one perforation's `[water, oil, gas]` mass-balance
/// source terms w.r.t. an OTHER (non-connected) cell in its producer control
/// neighborhood. Injector perforations have no neighborhood and never call
/// this (their coefficients don't depend on any fraction).
pub(crate) fn mass_balance_neighbor_jacobian(
    sim: &ReservoirSimulator,
    cell: &WellCellInput<f64>,
    neighborhood: &[WellCellInput<f64>],
    neighbor_idx: usize,
    q: f64,
) -> [[f64; 3]; 3] {
    let props = cell_props_generic::<f64>(
        sim,
        cell.regime,
        cell.p,
        cell.sw,
        cell.hydrocarbon_var,
        cell.drsdt0_base_rs,
    );
    let (_fractions, frac_block) = producer_fractions_neighbor_block(sim, neighborhood, neighbor_idx);
    let bo = props.bo.max(1e-9);
    let bg = props.bg.max(1e-9);
    let bw = sim.b_w.max(1e-9);

    let mut block = [[0.0; 3]; 3];
    for v in 0..3 {
        let d_water_frac = frac_block[0][v];
        let d_oil_frac = frac_block[1][v];
        let d_gas_frac = frac_block[2][v];
        block[0][v] = (d_water_frac / bw) * q;
        block[1][v] = (d_oil_frac / bo) * q;
        block[2][v] = (d_gas_frac / bg + (d_oil_frac / bo) * props.rs) * q;
    }
    block
}

/// BHP column of the well constraint row plus the raw Fischer-Burmeister
/// partials/scales needed to build each perforation's own/cross-term columns
/// with a consistent `dphi_db` (generic mirror of
/// `add_exact_well_constraint_jacobian`'s scalar bookkeeping; the FB
/// gradient itself is the existing, already-validated
/// `wells::fischer_burmeister_gradient`, not re-derived here).
pub(crate) fn well_constraint_bhp_column_and_fb_gradient(
    sim: &ReservoirSimulator,
    injector: bool,
    injected_fluid: InjectedFluid,
    control: &WellControlValuesGeneric,
    bhp: f64,
    perforations: &[WellPerforationInputGeneric<f64>],
) -> Option<(f64, f64, f64)> {
    if !control.enabled || !control.rate_controlled {
        return None;
    }
    let target_rate = control.target_rate?;
    let actual_rate = well_actual_rate_generic(
        sim,
        injector,
        injected_fluid,
        control.uses_surface_target,
        perforations,
    );
    let bhp_slack = if injector {
        control.bhp_limit - bhp
    } else {
        bhp - control.bhp_limit
    };
    let rate_slack = target_rate - actual_rate;
    let bhp_scale = control.bhp_limit.abs().max(1.0);
    let rate_scale = target_rate.abs().max(1.0);
    let (dphi_da, dphi_db) =
        crate::fim::wells::fischer_burmeister_gradient(bhp_slack / bhp_scale, rate_slack / rate_scale);
    let dslack_dbhp = if injector { -1.0 } else { 1.0 };
    let bhp_column = dphi_da * dslack_dbhp / bhp_scale;
    Some((bhp_column, dphi_db, rate_scale))
}

/// One perforation's own contribution to the well constraint row's Jacobian
/// (`-dphi_db/rate_scale * d(actual_rate)/d(theta)` restricted to that
/// perforation's own `[p, sw, hydrocarbon_var, q]`), via
/// `perforation_surface_rate_generic` / `perforation_reservoir_rate_generic`.
pub(crate) fn well_constraint_own_perforation_rate_jacobian(
    sim: &ReservoirSimulator,
    injector: bool,
    injected_fluid: InjectedFluid,
    uses_surface_target: bool,
    cell: &WellCellInput<f64>,
    producer_neighborhood: Option<(&[WellCellInput<f64>], usize)>,
    q: f64,
) -> [f64; 4] {
    let cell_ad = cell_as_ad4(cell);
    let q_ad = Ad::<4>::variable(q, 3);

    let rate = if uses_surface_target {
        let fractions_ad = fractions_with_connected_cell_active(sim, cell_ad, producer_neighborhood);
        perforation_surface_rate_generic(sim, injector, injected_fluid, &cell_ad, fractions_ad.as_ref(), q_ad)
    } else {
        perforation_reservoir_rate_generic(injector, q_ad)
    };

    *rate.deriv()
}

/// One perforation's cross-term contribution to the well constraint row's
/// Jacobian w.r.t. an OTHER (non-connected) cell in its control neighborhood
/// (producers with a surface-rate target only).
pub(crate) fn well_constraint_neighbor_rate_jacobian(
    sim: &ReservoirSimulator,
    cell: &WellCellInput<f64>,
    neighborhood: &[WellCellInput<f64>],
    neighbor_idx: usize,
    q: f64,
) -> [f64; 3] {
    let props = cell_props_generic::<f64>(
        sim,
        cell.regime,
        cell.p,
        cell.sw,
        cell.hydrocarbon_var,
        cell.drsdt0_base_rs,
    );
    let (_fractions, frac_block) = producer_fractions_neighbor_block(sim, neighborhood, neighbor_idx);
    let bo = props.bo.max(1e-9);
    let q_clamped = q.max(0.0);

    let mut d = [0.0; 3];
    for v in 0..3 {
        d[v] = (q_clamped / bo) * frac_block[1][v];
    }
    d
}

/// Packed 5-row residual `[rate_consistency, well_constraint, water_source,
/// oil_source, gas_source]` for one single-perforation well, generic over `S`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn perforation_residual_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    wi_geom: f64,
    injector: bool,
    injected_fluid: InjectedFluid,
    cell: &WellCellInput<S>,
    fractions: Option<&ProducerFractionsGeneric<S>>,
    bhp: S,
    q: S,
    control: &WellControlValuesGeneric,
) -> [S; 5] {
    let rate_consistency = q - connection_rate_generic(sim, wi_geom, injector, cell, bhp);
    let solo = [WellPerforationInputGeneric {
        cell: *cell,
        fractions: fractions.copied(),
        q,
    }];
    let constraint =
        well_constraint_residual_fb_generic(sim, injector, injected_fluid, control, bhp, &solo)
            .expect("target_rate present for rate-controlled test wells");
    let coefficients = component_rate_coefficients_generic(sim, injector, injected_fluid, cell, fractions);

    [
        rate_consistency,
        constraint,
        coefficients[0] * q,
        coefficients[1] * q,
        coefficients[2] * q,
    ]
}

/// Assemble the perforation's 5x5 Jacobian by seeding `Ad<5>` against
/// `[p, sw, hydrocarbon_var, well_bhp, perforation_rate]`.
///
/// `producer_neighborhood` is the connected cell's control neighborhood
/// (`wells::producer_control_state` widens this to a 3x3 window; the Phase 3
/// gate case is the width-1 instance where the neighborhood is just the
/// connected cell itself) plus the index of the connected cell within it, so
/// that entry can be re-seeded as the same `Ad<5>` variable used for `cell`
/// (giving it the correct cross-derivatives) while the rest of the
/// neighborhood is held constant.
pub(crate) fn perforation_jacobian(
    sim: &ReservoirSimulator,
    wi_geom: f64,
    injector: bool,
    injected_fluid: InjectedFluid,
    cell: &WellCellInput<f64>,
    producer_neighborhood: Option<(&[WellCellInput<f64>], usize)>,
    bhp: f64,
    q: f64,
    control: &WellControlValuesGeneric,
) -> [[f64; 5]; 5] {
    let cell_ad = WellCellInput {
        p: Ad::<5>::variable(cell.p, 0),
        sw: Ad::<5>::variable(cell.sw, 1),
        hydrocarbon_var: Ad::<5>::variable(cell.hydrocarbon_var, 2),
        regime: cell.regime,
        drsdt0_base_rs: cell.drsdt0_base_rs,
    };
    let bhp_ad = Ad::<5>::variable(bhp, 3);
    let q_ad = Ad::<5>::variable(q, 4);

    let fractions_ad = producer_neighborhood.map(|(neighborhood, connected_index)| {
        let seeded: Vec<WellCellInput<Ad<5>>> = neighborhood
            .iter()
            .enumerate()
            .map(|(idx, c)| {
                if idx == connected_index {
                    cell_ad
                } else {
                    WellCellInput {
                        p: Ad::<5>::constant(c.p),
                        sw: Ad::<5>::constant(c.sw),
                        hydrocarbon_var: Ad::<5>::constant(c.hydrocarbon_var),
                        regime: c.regime,
                        drsdt0_base_rs: c.drsdt0_base_rs,
                    }
                }
            })
            .collect();
        producer_fractions_generic(sim, &seeded)
    });

    let residual = perforation_residual_generic(
        sim,
        wi_geom,
        injector,
        injected_fluid,
        &cell_ad,
        fractions_ad.as_ref(),
        bhp_ad,
        q_ad,
        control,
    );

    let mut jac = [[0.0; 5]; 5];
    for (eq, r) in residual.iter().enumerate() {
        jac[eq] = *r.deriv();
    }
    jac
}

/// Plain-`f64` packed residual, for numerical-Jacobian comparison.
pub(crate) fn perforation_residual_f64(
    sim: &ReservoirSimulator,
    wi_geom: f64,
    injector: bool,
    injected_fluid: InjectedFluid,
    cell: &WellCellInput<f64>,
    producer_neighborhood: Option<(&[WellCellInput<f64>], usize)>,
    bhp: f64,
    q: f64,
    control: &WellControlValuesGeneric,
) -> [f64; 5] {
    let fractions = producer_neighborhood.map(|(n, _)| producer_fractions_generic(sim, n));
    perforation_residual_generic(
        sim,
        wi_geom,
        injector,
        injected_fluid,
        cell,
        fractions.as_ref(),
        bhp,
        q,
        control,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fim::numjac::{assert_jacobian_matches, central_difference_jacobian};
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
        sim.set_three_phase_rel_perm_props(0.1, 0.1, 0.05, 0.05, 0.15, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0)
            .unwrap();
        sim
    }

    fn cell(p: f64, sw: f64, hc: f64) -> WellCellInput<f64> {
        WellCellInput {
            p,
            sw,
            hydrocarbon_var: hc,
            regime: HydrocarbonState::Saturated,
            drsdt0_base_rs: None,
        }
    }

    /// One perforation gate: AD-derived 5x5 Jacobian must match the central-
    /// difference reference for the packed 5-unknown / 5-equation perforation
    /// system `[p, sw, hydrocarbon_var, well_bhp, perforation_rate]`.
    fn bhp_control(target_bhp: f64) -> WellControlValuesGeneric {
        WellControlValuesGeneric {
            enabled: true,
            rate_controlled: false,
            uses_surface_target: false,
            target_rate: None,
            bhp_limit: target_bhp,
            bhp_target: target_bhp,
        }
    }

    fn rate_control(
        target: f64,
        uses_surface_target: bool,
        bhp_limit: f64,
    ) -> WellControlValuesGeneric {
        WellControlValuesGeneric {
            enabled: true,
            rate_controlled: true,
            uses_surface_target,
            target_rate: Some(target),
            bhp_limit,
            bhp_target: bhp_limit,
        }
    }

    fn perforation_gate(
        injector: bool,
        injected_fluid: InjectedFluid,
        producer: bool,
        cell_in: WellCellInput<f64>,
        bhp: f64,
        q: f64,
        control: WellControlValuesGeneric,
    ) {
        let sim = three_phase_sim();
        let wi_geom = 12.0_f64;
        let neighborhood = [cell_in];
        let producer_neighborhood = producer.then_some((neighborhood.as_slice(), 0usize));

        let analytic = perforation_jacobian(
            &sim,
            wi_geom,
            injector,
            injected_fluid,
            &cell_in,
            producer_neighborhood,
            bhp,
            q,
            &control,
        );

        let x0 = [cell_in.p, cell_in.sw, cell_in.hydrocarbon_var, bhp, q];
        let residual = |x: &[f64]| {
            let c2 = WellCellInput {
                p: x[0],
                sw: x[1],
                hydrocarbon_var: x[2],
                ..cell_in
            };
            let neighborhood = [c2];
            let producer_neighborhood = producer.then_some((neighborhood.as_slice(), 0usize));
            perforation_residual_f64(
                &sim,
                wi_geom,
                injector,
                injected_fluid,
                &c2,
                producer_neighborhood,
                x[3],
                x[4],
                &control,
            )
            .to_vec()
        };
        let numerical = central_difference_jacobian(&x0, 5, residual);

        assert_jacobian_matches(
            &analytic.iter().map(|r| r.to_vec()).collect::<Vec<_>>(),
            &numerical,
            1e-6,
            1e-9,
        );
    }

    #[test]
    fn injector_water_bhp_controlled() {
        perforation_gate(
            true,
            InjectedFluid::Water,
            false,
            cell(150.0, 0.3, 0.15),
            180.0, // bhp above cell pressure -> injecting
            -80.0,
            bhp_control(180.0),
        );
    }

    #[test]
    fn injector_gas_rate_controlled_surface_target() {
        perforation_gate(
            true,
            InjectedFluid::Gas,
            false,
            cell(150.0, 0.3, 0.15),
            180.0,
            -80.0,
            rate_control(500.0, true, 250.0),
        );
    }

    #[test]
    fn producer_bhp_controlled() {
        perforation_gate(
            false,
            InjectedFluid::Water,
            true,
            cell(150.0, 0.3, 0.15),
            80.0, // bhp below cell pressure -> producing
            60.0,
            bhp_control(80.0),
        );
    }

    #[test]
    fn producer_rate_controlled_surface_target() {
        perforation_gate(
            false,
            InjectedFluid::Water,
            true,
            cell(150.0, 0.3, 0.15),
            80.0,
            60.0,
            rate_control(40.0, true, 40.0),
        );
    }

    #[test]
    fn producer_rate_controlled_reservoir_target() {
        perforation_gate(
            false,
            InjectedFluid::Water,
            true,
            cell(150.0, 0.3, 0.15),
            80.0,
            60.0,
            rate_control(60.0, false, 40.0),
        );
    }

    #[test]
    fn injector_reservoir_target() {
        perforation_gate(
            true,
            InjectedFluid::Water,
            false,
            cell(150.0, 0.3, 0.15),
            180.0,
            -80.0,
            rate_control(80.0, false, 250.0),
        );
    }

    /// Proves the "preserve neighborhood exactly" design for Phase 4: summing
    /// one `producer_fractions_neighbor_block` per neighbor cell must
    /// reconstruct the exact Jacobian of the fractions w.r.t. the WHOLE
    /// stacked neighborhood, matching a numerical Jacobian taken over all
    /// neighbors' variables simultaneously. This is what lets the full
    /// assembler support wells::producer_control_state's real 3x3 (or
    /// edge-clamped smaller) neighborhood without a dual number wider than
    /// `Ad<3>`.
    #[test]
    fn neighbor_blocks_reconstruct_full_neighborhood_jacobian() {
        let sim = three_phase_sim();
        // Four distinct cells standing in for a (possibly edge-clamped)
        // control neighborhood.
        let neighborhood = [
            cell(160.0, 0.30, 0.15),
            cell(155.0, 0.28, 0.12),
            cell(148.0, 0.32, 0.20),
            cell(152.0, 0.25, 0.18),
        ];

        let mut analytic = vec![vec![0.0; 3 * neighborhood.len()]; 3];
        for (n_idx, _) in neighborhood.iter().enumerate() {
            let (_, block) = producer_fractions_neighbor_block(&sim, &neighborhood, n_idx);
            for eq in 0..3 {
                for v in 0..3 {
                    analytic[eq][3 * n_idx + v] = block[eq][v];
                }
            }
        }

        let x0: Vec<f64> = neighborhood
            .iter()
            .flat_map(|c| [c.p, c.sw, c.hydrocarbon_var])
            .collect();
        let residual = |x: &[f64]| {
            let cells: Vec<WellCellInput<f64>> = neighborhood
                .iter()
                .enumerate()
                .map(|(idx, c)| WellCellInput {
                    p: x[3 * idx],
                    sw: x[3 * idx + 1],
                    hydrocarbon_var: x[3 * idx + 2],
                    ..*c
                })
                .collect();
            let fractions = producer_fractions_generic::<f64>(&sim, &cells);
            vec![
                fractions.water_fraction,
                fractions.oil_fraction,
                fractions.gas_fraction,
            ]
        };
        let numerical = central_difference_jacobian(&x0, 3, residual);

        assert_jacobian_matches(&analytic, &numerical, 1e-6, 1e-9);
    }

    /// Bit-parity gate: the generic f64 well/perforation path must reproduce
    /// the REAL production functions (`wells::perforation_rate_residual`,
    /// `wells::well_constraint_residual` — Fischer-Burmeister based —, and
    /// `wells::perforation_component_rates_sc_day`) on an actual
    /// `ReservoirSimulator` + `FimWellTopology` + `FimState`, not just a
    /// hand-built scalar test rig. This is what caught the first draft of
    /// `well_constraint_residual_generic` using the wrong (non-FB, non
    /// multi-perforation) formula.
    fn well_parity_case(injector: bool, rate_controlled: bool, uses_surface_target: bool) {
        use crate::fim::state::FimState;
        use crate::fim::wells::{
            self, build_well_topology, geometric_well_index, perforation_component_rates_sc_day,
            physical_well_control, producer_control_state,
        };

        let mut sim = three_phase_sim();
        sim.set_injected_fluid("water").unwrap();
        sim.add_well(0, 0, 0, if injector { 250.0 } else { 40.0 }, 0.1, 0.0, injector)
            .unwrap();
        if rate_controlled {
            sim.set_rate_controlled_wells(true);
            if uses_surface_target {
                sim.set_target_well_surface_rates(300.0, 25.0).unwrap();
            } else {
                sim.set_target_well_rates(80.0, 60.0).unwrap();
            }
        } else {
            sim.set_rate_controlled_wells(false);
        }

        let topology = build_well_topology(&sim);
        let mut state = FimState::from_simulator(&sim);
        // Perturb off the auto-initialized consistent rate so the rate-
        // consistency and constraint rows are genuinely exercised, not
        // trivially zero.
        state.perforation_rates_m3_day[0] += if injector { -15.0 } else { 12.0 };

        let perforation = &topology.perforations[0];
        let well_idx = perforation.physical_well_index;
        let cell_idx = perforation.cell_index;
        let cell_state = state.cell(cell_idx);
        let wi_geom = geometric_well_index(&sim, perforation).expect("finite transmissibility");
        let injected_fluid = wells::effective_injected_fluid(&sim);
        let control = physical_well_control(&sim, &topology, well_idx);
        let bhp = state.well_bhp[well_idx];
        let q = state.perforation_rates_m3_day[0];

        let cell = WellCellInput {
            p: cell_state.pressure_bar,
            sw: cell_state.sw,
            hydrocarbon_var: cell_state.hydrocarbon_var,
            regime: cell_state.regime,
            drsdt0_base_rs: None,
        };
        let fractions = (!injector).then(|| {
            let f = producer_control_state(&sim, &state, perforation);
            ProducerFractionsGeneric {
                water_fraction: f.water_fraction,
                oil_fraction: f.oil_fraction,
                gas_fraction: f.gas_fraction,
            }
        });
        let control_generic = WellControlValuesGeneric {
            enabled: control.enabled,
            rate_controlled: control.rate_controlled,
            uses_surface_target: control.uses_surface_target,
            target_rate: control.target_rate,
            bhp_limit: control.bhp_limit,
            bhp_target: control.bhp_target,
        };

        // Rate-consistency row. `connection_rate_generic` is infallible
        // (unlike the `Option`-returning production helper, which only
        // returns `None` on non-finite geometry); compare against the
        // always-`Some` case directly.
        let real_rate_residual = wells::perforation_rate_residual(&sim, &state, &topology, 0);
        let generic_rate_residual = q - connection_rate_generic(&sim, wi_geom, injector, &cell, bhp);
        assert!(
            (real_rate_residual.unwrap() - generic_rate_residual).abs() < 1e-10,
            "rate residual: real={:?} generic={}",
            real_rate_residual,
            generic_rate_residual
        );

        // Well constraint row.
        let real_constraint = wells::well_constraint_residual(&sim, &state, &topology, well_idx);
        let solo = [WellPerforationInputGeneric { cell, fractions, q }];
        let generic_constraint = well_constraint_residual_fb_generic(
            &sim,
            injector,
            injected_fluid,
            &control_generic,
            bhp,
            &solo,
        );
        assert!(
            (real_constraint.unwrap() - generic_constraint.unwrap()).abs() < 1e-10,
            "constraint residual: real={:?} generic={:?}",
            real_constraint,
            generic_constraint
        );

        // Mass-balance well-source contribution (per phase).
        let real_components = perforation_component_rates_sc_day(&sim, &state, &topology, 0);
        let generic_coefficients =
            component_rate_coefficients_generic(&sim, injector, injected_fluid, &cell, fractions.as_ref());
        for phase in 0..3 {
            let generic_component = generic_coefficients[phase] * q;
            assert!(
                (real_components[phase] - generic_component).abs() < 1e-10,
                "phase {phase}: real={} generic={}",
                real_components[phase],
                generic_component
            );
        }
    }

    #[test]
    fn parity_injector_bhp_controlled() {
        well_parity_case(true, false, false);
    }

    #[test]
    fn parity_injector_rate_controlled_reservoir_target() {
        well_parity_case(true, true, false);
    }

    #[test]
    fn parity_injector_rate_controlled_surface_target() {
        well_parity_case(true, true, true);
    }

    #[test]
    fn parity_producer_bhp_controlled() {
        well_parity_case(false, false, false);
    }

    #[test]
    fn parity_producer_rate_controlled_reservoir_target() {
        well_parity_case(false, true, false);
    }

    #[test]
    fn parity_producer_rate_controlled_surface_target() {
        well_parity_case(false, true, true);
    }
}
