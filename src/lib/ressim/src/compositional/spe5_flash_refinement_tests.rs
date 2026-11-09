//! #116: the difference-form flash refinement (experimental, off by default). Measurement only.
//!
//! `fluid::flash` can polish a converged two-phase split with a residual whose vapour-minus-liquid
//! differences are formed directly (see the note above `difference_residual` there). These tests
//! turn it on for the calling thread with `with_difference_refinement_for_test` and compare it
//! with the unrefined flash, before any long replay:
//!
//! - **`comp_spe5_flash_refinement_states`**: the captured near-critical states (cells 33 and 39
//!   of failure B's best iterate; cell 40 as the control) under pressure-ulp, small-pressure and
//!   composition perturbations. For each point and each mode: the split, phase volumes and
//!   densities, vapour volume fraction, both phases' mobilities, and both residual measures. The
//!   output feeds `tools/jutul/spe5_flash_mp.py` (50-digit reference) and
//!   `tools/jutul/spe5_flash_refinement_compare.py`.
//! - **`comp_spe5_failure_b_with_refinement`**: failure B from its committed seed with the
//!   refinement on: the last attempt's floor, the Taylor test at the same iterates as the floor
//!   probe, the dt ladder up to the whole remainder, and the failing call itself.
//!
//! With `SPE5_TRACE_OUT=DIR` everything is written as JSON. No threshold is pinned.
use super::flux::hydrocarbon_kr_with_water;
use super::newton::NewtonProblem;
use super::spe5_failure_b_tests::{
    REMAINING_DAYS, floor_summary, last_attempt_dt, print_history, run, seed, taylor,
};
use crate::fluid::flash::{FlashState, flash, with_difference_refinement_for_test};
use crate::fluid::transport::flash_viscosities;
use serde_json::{Value, json};
use std::path::PathBuf;

pub(super) fn write(name: &str, value: &Value) {
    if let Ok(dir) = std::env::var("SPE5_TRACE_OUT") {
        let path = PathBuf::from(dir).join(name);
        std::fs::write(
            &path,
            serde_json::to_string_pretty(value).expect("serialize"),
        )
        .expect("write");
        println!("written {}", path.display());
    }
}

/// Everything about one flash result that reaches the flux: split, volumes, densities, vapour
/// volume fraction and both hydrocarbon phases' mobilities (kr / viscosity, 1/cP) at the cell's
/// water saturation.
pub(super) fn describe(problem: &NewtonProblem<'_>, sw: f64, f: &FlashState) -> Value {
    let t = problem.spec.reservoir_temperature_k();
    let (vl, vv) = (
        f.liquid.as_ref().map(|l| l.molar_volume),
        f.vapour.as_ref().map(|v| v.molar_volume),
    );
    let s_liquid = f.liquid_saturation();
    let kr = |liquid: bool| match problem.relperm.water() {
        Some(w) => hydrocarbon_kr_with_water(w, sw, s_liquid, liquid),
        None => problem.relperm.kr(s_liquid, liquid),
    };
    let (mu_l, mu_v) = flash_viscosities(problem.spec, t, f).unwrap_or((None, None));
    let mobility = |kr: f64, mu: Option<f64>| mu.map(|m| kr / (m * 1e3));
    json!({
        "phase": format!("{:?}", f.phase_state),
        "beta": f.beta,
        "x": f.x,
        "y": f.y,
        "liquid_molar_volume": vl,
        "vapour_molar_volume": vv,
        "liquid_mass_density": f.liquid.as_ref().map(|l| l.mass_density),
        "vapour_mass_density": f.vapour.as_ref().map(|v| v.mass_density),
        "vapour_volume_fraction": f.vapour_saturation(),
        "liquid_mobility": mobility(kr(true), mu_l),
        "vapour_mobility": mobility(kr(false), mu_v),
        "equilibrium_residual": f.equilibrium_residual,
        "iterations": f.iterations,
    })
}

/// The difference-form residual norm at a returned split (`ln K` from the phases' `ln phi`, as
/// the derivative pass takes it).
fn difference_norm(problem: &NewtonProblem<'_>, p: f64, z: &[f64], f: &FlashState) -> Option<f64> {
    let (l, v) = (f.liquid.as_ref()?, f.vapour.as_ref()?);
    let ln_k: Vec<f64> = l.ln_phi.iter().zip(&v.ln_phi).map(|(a, b)| a - b).collect();
    let t = problem.spec.reservoir_temperature_k();
    crate::fluid::flash::difference_residual(problem.spec, p, t, z, &ln_k, f.beta)
        .map(|r| r.iter().fold(0.0f64, |m, x| m.max(x.abs())))
}

#[test]
#[ignore = "#116 flash refinement on the captured states; seconds; --release --ignored --nocapture"]
fn comp_spe5_flash_refinement_states() {
    let case = seed();
    let out = case.with_diagnostic_problem(last_attempt_dt(), |problem, start, options| {
        let attempt = run(problem, start, options);
        let best = attempt
            .residual_history
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
            .expect("iterates");
        let t = problem.spec.reservoir_temperature_k();
        let mut sweeps = Vec::new();
        for cell in [33usize, 39, 40] {
            let c = &attempt.iterates[best].0.cells()[cell];
            let z0 = c.overall_composition();
            let p0 = c.pressure_bar * 1e5;
            let n = z0.len();
            // (kind, p, z) inputs: 64 pressure ulps, 32 steps of 0.01 Pa, and 32 composition
            // steps moving 4e-16 per step from the heaviest component to methane.
            let mut inputs: Vec<(&str, f64, Vec<f64>)> = Vec::new();
            let mut q = p0;
            for _ in 0..64 {
                inputs.push(("pressure_ulps", q, z0.clone()));
                q = f64::from_bits(q.to_bits() + 1);
            }
            for k in 0..32 {
                inputs.push(("pressure_0.01pa", p0 + 0.01 * f64::from(k), z0.clone()));
            }
            for k in 0..32 {
                let mut z = z0.clone();
                z[0] += 4e-16 * f64::from(k);
                z[n - 1] -= 4e-16 * f64::from(k);
                inputs.push(("composition", p0, z));
            }
            let mut by_kind: std::collections::BTreeMap<&str, Vec<Value>> = Default::default();
            for (kind, p, z) in inputs {
                let modes: Vec<(bool, Option<FlashState>)> = [false, true]
                    .into_iter()
                    .map(|on| {
                        let f = with_difference_refinement_for_test(on, || {
                            flash(problem.spec, p, t, &z, None)
                        });
                        (on, f.ok())
                    })
                    .collect();
                let Some(off) = modes[0].1.as_ref() else {
                    continue;
                };
                let mut point = json!({
                    "p_pa": p, "z": z,
                    // The starting guess for the 50-digit solver.
                    "beta": off.beta, "x": off.x, "y": off.y,
                });
                for (on, f) in &modes {
                    let key = if *on { "refined" } else { "unrefined" };
                    point[key] = match f {
                        Some(f) => {
                            let mut d = describe(problem, c.water_saturation, f);
                            d["difference_residual"] = json!(difference_norm(problem, p, &z, f));
                            d
                        }
                        None => json!({"error": true}),
                    };
                }
                by_kind.entry(kind).or_default().push(point);
            }
            for (kind, points) in by_kind {
                let spread = |key: &str| {
                    let b: Vec<f64> = points
                        .iter()
                        .filter_map(|p| p[key]["beta"].as_f64())
                        .collect();
                    b.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                        - b.iter().copied().fold(f64::INFINITY, f64::min)
                };
                println!(
                    "cell {cell} {kind:<15} beta spread unrefined {:.2e}  refined {:.2e}",
                    spread("unrefined"),
                    spread("refined")
                );
                sweeps.push(json!({
                    "cell": cell, "kind": kind, "pressure_bar": c.pressure_bar,
                    "water_saturation": c.water_saturation, "temperature_k": t, "z": z0,
                    "ulp_sweep": {"points": points},
                }));
            }
        }
        json!({"best_iterate": best + 1, "fluid": fluid_json(problem), "cells": sweeps})
    });
    write("flash_refinement_states.json", &out);
    assert!(out["cells"].as_array().is_some_and(|c| c.len() == 9));
}

/// The fluid exactly as ResSim holds it, for an independent solver.
pub(super) fn fluid_json(problem: &NewtonProblem<'_>) -> Value {
    let spec = problem.spec;
    let n = spec.component_count();
    json!({
        "eos": format!("{:?}", spec.eos()),
        "components": spec.components().iter().map(|c| json!({
            "id": c.id, "molar_mass_kg_per_mol": c.molar_mass_kg_per_mol,
            "critical_temperature_k": c.critical_temperature_k,
            "critical_pressure_pa": c.critical_pressure_pa,
            "acentric_factor": c.acentric_factor})).collect::<Vec<_>>(),
        "interaction": (0..n).map(|i| (0..n).map(|j| spec.interaction(i, j))
            .collect::<Vec<_>>()).collect::<Vec<_>>(),
        "volume_shifts": spec.volume_shifts(),
        "omega_a": 0.457_235_529,
        "omega_b": 0.077_796_074,
        "corrected_above_acentric": 0.49,
        "gas_constant_j_per_mol_k": 8.314_462_618_153_24,
    })
}

#[test]
#[ignore = "#116 failure B with the flash refinement; minutes; --release --ignored --nocapture"]
fn comp_spe5_failure_b_with_refinement() {
    let case = seed();
    let dt = last_attempt_dt();

    // 1. The last attempt, both modes, and the Taylor test with the refinement on.
    let local = case.with_diagnostic_problem(dt, |problem, start, options| {
        let off = run(problem, start, options);
        print_history("last attempt, unrefined", &off);
        with_difference_refinement_for_test(true, || {
            let on = run(problem, start, options);
            print_history("last attempt, refined", &on);
            let mut taylor_rows = Vec::new();
            for k in [1usize, 3, 5, 7] {
                if k + 1 >= on.iterates.len() {
                    continue;
                }
                let t = taylor(problem, &on, k);
                println!(
                    "taylor (refined) at iterate {}: scaled residual {:.3e}, repeat diff {:.1e}",
                    t["iterate"],
                    t["scaled_residual"].as_f64().unwrap_or(f64::NAN),
                    t["repeat_evaluation_scaled_difference"]
                        .as_f64()
                        .unwrap_or(f64::NAN)
                );
                for row in t["rows"].as_array().into_iter().flatten() {
                    println!(
                        "    h {:.0e}  change {:.3e}  error {:.3e}  e/h {:.3e}  e/h^2 {:.3e}",
                        row["h"].as_f64().unwrap_or(f64::NAN),
                        row["scaled_change"].as_f64().unwrap_or(f64::NAN),
                        row["scaled_taylor_error"].as_f64().unwrap_or(f64::NAN),
                        row["error_over_h"].as_f64().unwrap_or(f64::NAN),
                        row["error_over_h2"].as_f64().unwrap_or(f64::NAN),
                    );
                }
                taylor_rows.push(t);
            }
            json!({
                "unrefined": {"residual_history": off.residual_history,
                    "summary": floor_summary(&off.residual_history)},
                "refined": {"residual_history": on.residual_history,
                    "summary": floor_summary(&on.residual_history)},
                "taylor_refined": taylor_rows,
            })
        })
    });

    // 2. One attempt at each dt from the last halving up to the whole remainder.
    let mut ladder = Vec::new();
    for k in 0..10 {
        let dt_k = dt * 2f64.powi(k);
        let mut row = json!({"dt_days": dt_k});
        for on in [false, true] {
            let a =
                with_difference_refinement_for_test(on, || case.with_diagnostic_problem(dt_k, run));
            print_history(
                &format!(
                    "dt {dt_k:.3e} d {}",
                    if on { "refined" } else { "unrefined" }
                ),
                &a,
            );
            row[if on { "refined" } else { "unrefined" }] = floor_summary(&a.residual_history);
        }
        ladder.push(row);
    }

    // 3. The failing call itself: the whole remainder, default timestep control.
    let mut call = seed();
    let outcome = with_difference_refinement_for_test(true, || call.step(REMAINING_DAYS));
    println!(
        "failing call, refined: succeeded {}, attempts {}, accepted dt {:?}, newton {}, {:?}",
        outcome.succeeded(),
        outcome.attempts,
        outcome.accepted_dt_days,
        outcome.newton_iterations,
        outcome.failure
    );
    let out = json!({
        "seed": "stall_218_from_scratch.json",
        "last_attempt": local,
        "ladder": ladder,
        "failing_call_refined": {
            "succeeded": outcome.succeeded(), "attempts": outcome.attempts,
            "accepted_dt_days": outcome.accepted_dt_days,
            "newton_iterations": outcome.newton_iterations,
            "failure": format!("{:?}", outcome.failure),
        },
    });
    write("failure_b_with_refinement.json", &out);
    assert_eq!(out["ladder"].as_array().map(Vec::len), Some(10));
}
