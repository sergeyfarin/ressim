//! Generic (differentiable) mirror of the undersaturated-cell excess-gas
//! flash: `flash::resolve_cell_flash` falls into this path when a trial `Rs`
//! exceeds its (possibly DRSDT0-capped) saturated bound, calling
//! `ReservoirSimulator::split_gas_inventory_after_transport` — which itself
//! calls the 64-iteration bisection `solve_rs_for_dissolved_gas`.
//!
//! The bisection is intentionally NOT differentiated step-by-step: chain-
//! ruling through 64 sequential branch selections is both slower and, right
//! at convergence, numerically noisier (the last iterations become no-ops in
//! value once `low == high` in floating point, so naive AD through them adds
//! nothing but risk). Instead this uses implicit differentiation: run the
//! existing, unchanged f64 bisection to get the converged root `rs*`, then
//! recover its exact derivative via one throwaway Newton-corrector step
//! evaluated in the AD type. For the smooth residual `g(rs, theta) =
//! dissolved_gas(rs, theta) - target(theta)`, with `g(rs*, theta) ≈ 0` to
//! bisection's convergence tolerance:
//!
//!   rs_result = rs* - g(rs*, theta) / (∂g/∂rs at rs*)
//!
//! reproduces `rs*` to first order in the (negligible) residual, while its
//! AD derivative is exactly `-(∂g/∂theta) / (∂g/∂rs)` — the implicit
//! function theorem, applied without needing a closed form for `rs*(theta)`.

use crate::ReservoirSimulator;
use crate::fim::ad::{Ad, Scalar};

/// `dissolved_gas_sc(rs) = oil_saturation * pore_volume_m3 * rs / Bo(p, rs)`,
/// the same closed-form expression `solve_rs_for_dissolved_gas`'s bisection
/// evaluates at each trial `rs`. Falls back to `Bo = 1` when there is no PVT
/// table (mirrors `PvtTable::interpolate_oil`'s absence handling upstream).
fn dissolved_gas_value_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    pressure_bar: S,
    rs: S,
    oil_saturation: S,
    pore_volume_m3: S,
) -> S {
    let bo = match &sim.pvt_table {
        Some(table) => table.interpolate_oil_generic(pressure_bar, rs).0,
        None => S::from_f64(1.0),
    };
    (oil_saturation * pore_volume_m3 / bo.max_floor(1e-9)) * rs
}

/// Generic mirror of `ReservoirSimulator::solve_rs_for_dissolved_gas`.
fn solve_rs_for_dissolved_gas_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    pressure_bar: S,
    water_saturation: S,
    gas_saturation: S,
    pore_volume_m3: S,
    dissolved_gas_sc: S,
    rs_upper: f64,
) -> S {
    let rs_star = sim.solve_rs_for_dissolved_gas(
        pressure_bar.value(),
        water_saturation.value(),
        gas_saturation.value(),
        pore_volume_m3.value(),
        dissolved_gas_sc.value(),
        rs_upper,
    );

    let oil_saturation = (S::from_f64(1.0) - water_saturation - gas_saturation).max_floor(0.0);
    let target = dissolved_gas_sc.max_floor(0.0);

    // ∂g/∂rs at the converged root, isolated via a local unit-width dual so
    // it doesn't pick up cross-derivatives from `theta`.
    let d_g_d_rs = {
        let rs_local = Ad::<1>::variable(rs_star, 0);
        dissolved_gas_value_generic(
            sim,
            Ad::<1>::constant(pressure_bar.value()),
            rs_local,
            Ad::<1>::constant(oil_saturation.value()),
            Ad::<1>::constant(pore_volume_m3.value()),
        )
        .d(0)
    };

    if d_g_d_rs.abs() < 1e-14 {
        // Degenerate cases (matches the real solve's early `return 0.0`
        // guards: no local sensitivity of dissolved gas to rs at this point).
        return S::from_f64(rs_star);
    }

    let g_theta =
        dissolved_gas_value_generic(sim, pressure_bar, S::from_f64(rs_star), oil_saturation, pore_volume_m3)
            - target;

    S::from_f64(rs_star) - g_theta / d_g_d_rs
}

/// Generic mirror of `ReservoirSimulator::split_gas_inventory_after_transport`.
/// Branch selection (redissolution on/off, which saturation regime results,
/// the `denom` closed-form vs. degenerate fallback) is made on `.value()`,
/// matching the f64 control flow exactly; every branch is otherwise plain
/// `Scalar` arithmetic except the two internal `solve_rs_for_dissolved_gas`
/// calls, which go through the IFT wrapper above.
pub(crate) fn split_gas_inventory_after_transport_generic<S: Scalar>(
    sim: &ReservoirSimulator,
    pressure_bar: S,
    pore_volume_m3: S,
    water_saturation: S,
    transported_free_gas_sc: S,
    dissolved_gas_sc: S,
    drsdt0_base_rs: Option<f64>,
) -> (S, S, S) {
    let Some(table) = sim.pvt_table.as_ref() else {
        let bg = S::from_f64(1.0);
        let sg = ((transported_free_gas_sc.max_floor(0.0) * bg) / pore_volume_m3.max_floor(1e-9))
            .max_floor(0.0)
            .min_of((S::from_f64(1.0) - water_saturation).max_floor(0.0));
        let so = (S::from_f64(1.0) - water_saturation - sg).max_floor(0.0);
        return (sg, so, S::from_f64(0.0));
    };

    let total_hydrocarbon_saturation = (S::from_f64(1.0) - water_saturation).max_floor(0.0);
    let bg = table.interpolate_saturated_generic(pressure_bar).bg.max_floor(1e-9);
    let free_gas_sc_transport = transported_free_gas_sc.max_floor(0.0);
    let sg_transport = ((free_gas_sc_transport * bg) / pore_volume_m3.max_floor(1e-9))
        .max_floor(0.0)
        .min_of(total_hydrocarbon_saturation);
    let so_transport = (total_hydrocarbon_saturation - sg_transport).max_floor(0.0);
    let dissolved_gas_sc = dissolved_gas_sc.max_floor(0.0);

    let rs_max = table.interpolate_saturated_generic(pressure_bar).rs.max_floor(0.0);
    let rs_dissolution_cap = if sim.gas_redissolution_enabled {
        rs_max
    } else {
        match drsdt0_base_rs {
            Some(base_rs) => S::from_f64(base_rs.max(0.0)).min_of(rs_max),
            None => rs_max,
        }
    };
    let (bo_dissolution_cap, _mu) = table.interpolate_oil_generic(pressure_bar, rs_dissolution_cap);
    let bo_dissolution_cap = bo_dissolution_cap.max_floor(1e-9);

    if !sim.gas_redissolution_enabled {
        let max_dissolved_sc_transport =
            (so_transport * pore_volume_m3 / bo_dissolution_cap) * rs_dissolution_cap;
        if dissolved_gas_sc.value() <= max_dissolved_sc_transport.value() + 1e-9 {
            let rs = solve_rs_for_dissolved_gas_generic(
                sim,
                pressure_bar,
                water_saturation,
                sg_transport,
                pore_volume_m3,
                dissolved_gas_sc,
                rs_dissolution_cap.value(),
            );
            return (sg_transport, so_transport, rs);
        }
    }

    let total_gas_sc = free_gas_sc_transport + dissolved_gas_sc;
    let (rs_saturated, bo_saturated) = if sim.gas_redissolution_enabled {
        let (bo_sat, _) = table.interpolate_oil_generic(pressure_bar, rs_max);
        (rs_max, bo_sat.max_floor(1e-9))
    } else {
        (rs_dissolution_cap, bo_dissolution_cap)
    };
    let max_all_dissolved_sc =
        (total_hydrocarbon_saturation * pore_volume_m3 / bo_saturated) * rs_saturated;
    if sim.gas_redissolution_enabled && total_gas_sc.value() <= max_all_dissolved_sc.value() + 1e-9 {
        let rs = solve_rs_for_dissolved_gas_generic(
            sim,
            pressure_bar,
            water_saturation,
            S::from_f64(0.0),
            pore_volume_m3,
            total_gas_sc,
            rs_saturated.value(),
        );
        return (S::from_f64(0.0), total_hydrocarbon_saturation, rs);
    }

    let denom = bg.recip() - (rs_saturated / bo_saturated);
    let sg_saturated = if denom.value().abs() > 1e-12 {
        ((total_gas_sc / pore_volume_m3) - (total_hydrocarbon_saturation * rs_saturated / bo_saturated))
            / denom
    } else {
        sg_transport
    };
    let sg_lower_bound = if sim.gas_redissolution_enabled {
        S::from_f64(0.0)
    } else {
        sg_transport
    };
    let sg = sg_saturated.max_of(sg_lower_bound).min_of(total_hydrocarbon_saturation);
    let so = (total_hydrocarbon_saturation - sg).max_floor(0.0);
    (sg, so, rs_saturated)
}
