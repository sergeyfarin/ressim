//! #116: what sets failure B's residual floor. **Measurement only; no solver behaviour changes.**
//!
//! Failure B is the from-scratch month-218 call: its last attempt (0.0443 d) flips no phase label,
//! drops to 4e-8 by iteration 4 and then wanders between 3.6e-8 and 1.4e-7 without reaching the
//! `1e-8` test. Calling that floor "noise" is a hypothesis. Starting from the committed seed
//! (`stall_218_from_scratch.json`), this probe measures three candidate causes on that attempt:
//!
//! 1. **The linear solve.** Every iteration's linear report (route, Krylov iterations, true
//!    residual `||b - A x|| / ||b||` on the full row-scaled system), and the same attempt on the
//!    dense and sparse LU routes. If the floor is the iterative solve's, a direct route removes it.
//! 2. **The flash tolerance.** The same attempt with the flash's equilibrium tolerance replaced
//!    (test-only override) from 1e-9 to 1e-13. If the floor is flash noise, it moves with it.
//! 3. **Jacobian consistency.** A Taylor test at chosen iterates along the step Newton actually
//!    took, with well BHPs held at the iterate's: `e(h) = |R(x + h d) - R(x) - h J d|` in ResSim's
//!    row scaling. A consistent Jacobian gives `e ~ h^2` down to the residual's own evaluation
//!    noise, where `e` flattens; an inconsistent one gives `e ~ h`.
//!
//! The probe pins no threshold. With `SPE5_TRACE_OUT` set, everything is written as JSON.
use super::api::{Checkpoint, CompositionalCase};
use super::assembly::{AssemblyResult, assemble_with_well_bhp};
use super::layout::CellPrimary;
use super::linear::LinearRoute;
use super::newton::{NewtonOptions, NewtonProblem, solve_newton_observed};
use super::state::CompositionalState;
use crate::fluid::flash::with_equilibrium_tolerance_for_test;
use serde_json::{Value, json};
use std::path::PathBuf;

/// The failing call's remainder and its tenth (last) halving.
const REMAINING_DAYS: f64 = 22.671554013151308;

fn last_attempt_dt() -> f64 {
    REMAINING_DAYS * 0.5f64.powi(9)
}

fn seed() -> CompositionalCase {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../opm/compositional/spe5/stall_218_from_scratch.json");
    let text = std::fs::read_to_string(path).expect("read the seed");
    let checkpoint: Checkpoint = serde_json::from_str(&text).expect("parse the seed");
    CompositionalCase::restore(&checkpoint).expect("restore the seed")
}

/// One attempt: its residual history, every linear report, and each iterate's state and BHPs.
struct Attempt {
    residual_history: Vec<f64>,
    linear: Vec<Value>,
    iterates: Vec<(CompositionalState, Vec<f64>)>,
}

fn run(problem: &NewtonProblem<'_>, start: &CompositionalState, options: NewtonOptions) -> Attempt {
    let mut iterates = Vec::new();
    let (_, report) = solve_newton_observed(
        problem,
        start,
        options,
        Some(&mut |it| iterates.push((it.state.clone(), it.assembled.well_bhp.clone()))),
    );
    let linear = report
        .linear_history
        .iter()
        .map(|l| {
            json!({
                "route": format!("{:?}", l.route),
                "iterations": l.iterations,
                "rhs_norm": l.rhs_norm,
                "residual_norm": l.residual_norm,
                "reduction": l.reduction(),
                "finite": l.finite,
                "min_pivot": l.min_pivot,
            })
        })
        .collect();
    Attempt {
        residual_history: report.residual_history,
        linear,
        iterates,
    }
}

fn floor_summary(history: &[f64]) -> Value {
    let best = history.iter().copied().fold(f64::INFINITY, f64::min);
    let late: Vec<f64> = history.iter().skip(3).copied().collect();
    let late_max = late.iter().copied().fold(0.0f64, f64::max);
    let late_min = late.iter().copied().fold(f64::INFINITY, f64::min);
    json!({
        "iterations": history.len(),
        "converged": history.last().is_some_and(|r| *r <= 1e-8),
        "best": best,
        "last": history.last(),
        "from_iteration_4_min": late_min,
        "from_iteration_4_max": late_max,
    })
}

fn assemble(
    problem: &NewtonProblem<'_>,
    state: &CompositionalState,
    bhp: &[f64],
) -> AssemblyResult {
    assemble_with_well_bhp(
        problem.spec,
        problem.layout,
        &problem.rock,
        problem.relperm,
        state,
        problem.faces,
        problem.wells,
        problem.previous_moles,
        problem.sources,
        problem.dt_days,
        bhp,
    )
    .expect("assemble")
}

/// Largest row-scaled entry of a cell-block vector, with the scaling of `at`.
fn scaled_max(at: &AssemblyResult, n: usize, v: &[f64]) -> f64 {
    at.scaling
        .iter()
        .enumerate()
        .flat_map(|(cell, s)| s.scale_residual(&v[cell * n..(cell + 1) * n]))
        .fold(0.0f64, |m, x| m.max(x.abs()))
}

/// The Taylor test at iterate `k` along `x_{k+1} - x_k`, BHPs fixed at iterate `k`'s.
fn taylor(problem: &NewtonProblem<'_>, attempt: &Attempt, k: usize) -> Value {
    let layout = problem.layout;
    let n = layout.cell_block_size();
    let (x, bhp) = &attempt.iterates[k];
    let next = &attempt.iterates[k + 1].0;
    let mut d = vec![0.0; layout.cell_count() * n];
    for (cell, (a, b)) in x.cells().iter().zip(next.cells()).enumerate() {
        let col = |p| layout.cell_unknown(cell, p).expect("column");
        d[col(CellPrimary::Pressure)] = b.pressure_bar - a.pressure_bar;
        for (i, (za, zb)) in a.independent_z().iter().zip(b.independent_z()).enumerate() {
            d[col(CellPrimary::Composition(i))] = zb - za;
        }
        if layout.has_water() {
            d[col(CellPrimary::WaterSaturation)] = b.water_saturation - a.water_saturation;
        }
    }
    let base = assemble(problem, x, bhp);
    let again = assemble(problem, x, bhp);
    let repeat: Vec<f64> = base
        .residual
        .iter()
        .zip(&again.residual)
        .map(|(a, b)| a - b)
        .collect();
    let mut jd = vec![0.0; d.len()];
    base.jacobian.multiply(&d, &mut jd);
    let mut rows = Vec::new();
    let mut localized = Vec::new();
    for e in 0..=10 {
        let h = 10f64.powi(-e);
        let cells: Vec<_> = x
            .cells()
            .iter()
            .enumerate()
            .map(|(cell, c)| {
                let mut c = c.clone();
                let col = |p| layout.cell_unknown(cell, p).expect("column");
                c.pressure_bar += h * d[col(CellPrimary::Pressure)];
                for (i, z) in c.independent_z_mut().iter_mut().enumerate() {
                    *z += h * d[col(CellPrimary::Composition(i))];
                }
                if layout.has_water() {
                    c.water_saturation += h * d[col(CellPrimary::WaterSaturation)];
                }
                c
            })
            .collect();
        let Ok(state) = CompositionalState::new(layout, cells) else {
            rows.push(json!({"h": h, "inadmissible": true}));
            continue;
        };
        let perturbed = assemble(problem, &state, bhp);
        let r = perturbed.residual.clone();
        let change: Vec<f64> = r.iter().zip(&base.residual).map(|(a, b)| a - b).collect();
        if e == 6 {
            // Localize the change: the accumulation part is the cell's own inventory change
            // (the residual's accumulation term is `n(x) - n_prev`); the rest is faces and wells.
            let mut entries: Vec<Value> = Vec::new();
            for cell in 0..layout.cell_count() {
                let s = &base.scaling[cell];
                let scaled = s.scale_residual(&change[cell * n..(cell + 1) * n]);
                let inv: Vec<f64> = perturbed.inventories[cell]
                    .component_moles
                    .iter()
                    .zip(&base.inventories[cell].component_moles)
                    .map(|(a, b)| a - b)
                    .collect();
                let acc = s.scale_residual(&inv);
                for (row, &total) in scaled.iter().enumerate() {
                    entries.push(json!({
                        "cell": cell,
                        "row": row,
                        "scaled_change": total,
                        "scaled_accumulation": acc.get(row).copied().unwrap_or(0.0),
                        "scaled_faces_and_wells": total - acc.get(row).copied().unwrap_or(0.0),
                        "phase": format!("{:?}", base.inventories[cell].phase_state),
                        "phase_perturbed": format!("{:?}", perturbed.inventories[cell].phase_state),
                        "sg": base.inventories[cell].vapour_saturation,
                    }));
                }
            }
            entries.sort_by(|a, b| {
                let f = |v: &Value| v["scaled_change"].as_f64().unwrap_or(0.0).abs();
                f(b).total_cmp(&f(a))
            });
            entries.truncate(8);
            for en in &entries {
                println!(
                    "      h 1e-6 cell {:>3} row {} change {:+.3e} accumulation {:+.3e} faces/wells {:+.3e} {} -> {} sg {:.4}",
                    en["cell"],
                    en["row"],
                    en["scaled_change"].as_f64().unwrap_or(f64::NAN),
                    en["scaled_accumulation"].as_f64().unwrap_or(f64::NAN),
                    en["scaled_faces_and_wells"].as_f64().unwrap_or(f64::NAN),
                    en["phase"].as_str().unwrap_or(""),
                    en["phase_perturbed"].as_str().unwrap_or(""),
                    en["sg"].as_f64().unwrap_or(f64::NAN),
                );
            }
            // The phase split behind the flux: re-flash the two near-critical cells at both states.
            let t = problem.spec.reservoir_temperature_k();
            let mut flashes = Vec::new();
            for cell in [33usize, 39] {
                let a = &x.cells()[cell];
                let b = &state.cells()[cell];
                let fa = crate::fluid::flash::flash(
                    problem.spec,
                    a.pressure_bar * 1e5,
                    t,
                    &a.overall_composition(),
                    None,
                );
                let fb = crate::fluid::flash::flash(
                    problem.spec,
                    b.pressure_bar * 1e5,
                    t,
                    &b.overall_composition(),
                    None,
                );
                if let (Ok(fa), Ok(fb)) = (fa, fb) {
                    let dmax = |u: &[f64], v: &[f64]| {
                        u.iter()
                            .zip(v)
                            .fold(0.0f64, |m, (p, q)| m.max((p - q).abs()))
                    };
                    let rec = json!({
                        "cell": cell,
                        "dp_bar": b.pressure_bar - a.pressure_bar,
                        "dz_max": dmax(&a.overall_composition(), &b.overall_composition()),
                        "beta": [fa.beta, fb.beta],
                        "dbeta": fb.beta - fa.beta,
                        "dx_max": dmax(&fa.x, &fb.x),
                        "dy_max": dmax(&fa.y, &fb.y),
                        "iterations": [fa.iterations, fb.iterations],
                        "equilibrium_residual": [fa.equilibrium_residual, fb.equilibrium_residual],
                    });
                    println!(
                        "      flash cell {cell}: dp {:.2e} bar, dz {:.1e}, dbeta {:+.3e}, dx {:.2e}, dy {:.2e}, its {:?}, eq.res {:?}",
                        rec["dp_bar"].as_f64().unwrap_or(f64::NAN),
                        rec["dz_max"].as_f64().unwrap_or(f64::NAN),
                        rec["dbeta"].as_f64().unwrap_or(f64::NAN),
                        rec["dx_max"].as_f64().unwrap_or(f64::NAN),
                        rec["dy_max"].as_f64().unwrap_or(f64::NAN),
                        rec["iterations"],
                        rec["equilibrium_residual"],
                    );
                    flashes.push(rec);
                }
            }
            localized.push(json!({"h": h, "largest_changes": entries, "flashes": flashes}));
        }
        let error: Vec<f64> = change.iter().zip(&jd).map(|(c, j)| c - h * j).collect();
        let e = scaled_max(&base, n, &error);
        rows.push(json!({
            "h": h,
            "scaled_change": scaled_max(&base, n, &change),
            "scaled_linear_prediction": h * scaled_max(&base, n, &jd),
            "scaled_taylor_error": e,
            "error_over_h": e / h,
            "error_over_h2": e / (h * h),
        }));
    }
    json!({
        "iterate": k + 1,
        "scaled_residual": scaled_max(&base, n, &base.residual),
        "scaled_step_max_dp_bar": d.iter().step_by(n).fold(0.0f64, |m, v| m.max(v.abs())),
        "repeat_evaluation_scaled_difference": scaled_max(&base, n, &repeat),
        "rows": rows,
        "localized": localized,
    })
}

fn print_history(label: &str, attempt: &Attempt) {
    let s = floor_summary(&attempt.residual_history);
    println!(
        "{label:<28} best {:.3e}  it>=4 [{:.3e}, {:.3e}]  converged {}  ({} it)",
        s["best"].as_f64().unwrap_or(f64::NAN),
        s["from_iteration_4_min"].as_f64().unwrap_or(f64::NAN),
        s["from_iteration_4_max"].as_f64().unwrap_or(f64::NAN),
        s["converged"],
        s["iterations"],
    );
}

#[test]
#[ignore = "#116 failure-B cause probe; ~minutes; run with --release --ignored --nocapture"]
fn comp_spe5_failure_b_floor_probe() {
    let case = seed();
    let dt = last_attempt_dt();
    let out = case.with_diagnostic_problem(dt, |problem, start, options| {
        // 1. Linear route.
        let mut routes = serde_json::Map::new();
        let mut baseline = None;
        for route in [
            LinearRoute::Auto,
            LinearRoute::FgmresCpr,
            LinearRoute::SparseLu,
            LinearRoute::DenseLu,
        ] {
            let mut o = options;
            o.linear.route = route;
            let attempt = run(problem, start, o);
            print_history(&format!("route {route:?}"), &attempt);
            routes.insert(
                format!("{route:?}"),
                json!({
                    "residual_history": attempt.residual_history,
                    "summary": floor_summary(&attempt.residual_history),
                    "linear": attempt.linear,
                }),
            );
            if route == LinearRoute::Auto {
                baseline = Some(attempt);
            }
        }
        let baseline = baseline.expect("baseline attempt");

        // 2. Flash tolerance, default route.
        let mut flash = serde_json::Map::new();
        for tolerance in [1e-9, 1e-10, 1e-11, 1e-12, 1e-13] {
            let attempt =
                with_equilibrium_tolerance_for_test(tolerance, || run(problem, start, options));
            print_history(&format!("flash tolerance {tolerance:e}"), &attempt);
            flash.insert(
                format!("{tolerance:e}"),
                json!({
                    "residual_history": attempt.residual_history,
                    "summary": floor_summary(&attempt.residual_history),
                }),
            );
        }

        // 3. Taylor test: iterate 2 (descending) as the control, then floor iterates.
        let mut taylor_rows = Vec::new();
        for k in [1usize, 3, 5, 7] {
            if k + 1 >= baseline.iterates.len() {
                continue;
            }
            let t = taylor(problem, &baseline, k);
            println!(
                "taylor at iterate {}: scaled residual {:.3e}, repeat diff {:.1e}",
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
            "seed": "stall_218_from_scratch.json",
            "dt_days": dt,
            "routes": routes,
            "flash_tolerance": flash,
            "taylor": taylor_rows,
        })
    });
    assert!(out["routes"].as_object().is_some_and(|r| r.len() == 4));
    if let Ok(dir) = std::env::var("SPE5_TRACE_OUT") {
        let path = PathBuf::from(dir).join("failure_b_floor_probe.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&out).expect("serialize"),
        )
        .expect("write the probe");
        println!("probe written to {}", path.display());
    }
}

/// The flash alone at cell 33 of failure B's best iterate: what each equilibrium tolerance
/// actually achieves, and how far the converged beta scatters when the pressure moves by a few
/// ulps (composition fixed). Measures the split's conditioning, not a tolerance to adopt.
#[test]
#[ignore = "#116 failure-B flash conditioning; seconds; run with --release --ignored --nocapture"]
fn comp_spe5_failure_b_flash_conditioning() {
    let case = seed();
    let dt = last_attempt_dt();
    let out = case.with_diagnostic_problem(dt, |problem, start, options| {
        let attempt = run(problem, start, options);
        let best = attempt
            .residual_history
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
            .expect("iterates");
        let t = problem.spec.reservoir_temperature_k();
        let mut cells = Vec::new();
        for cell in [33usize, 39, 40] {
            let c = &attempt.iterates[best].0.cells()[cell];
            let z = c.overall_composition();
            let p = c.pressure_bar * 1e5;
            let mut tolerances = Vec::new();
            for tol in [1e-9, 1e-10, 1e-11, 1e-12, 1e-13, 1e-14] {
                let r = with_equilibrium_tolerance_for_test(tol, || {
                    crate::fluid::flash::flash(problem.spec, p, t, &z, None)
                });
                let rec = match r {
                    Ok(f) => json!({"tolerance": tol, "beta": f.beta,
                        "equilibrium_residual": f.equilibrium_residual, "iterations": f.iterations,
                        "phase": format!("{:?}", f.phase_state)}),
                    Err(e) => json!({"tolerance": tol, "error": format!("{e:?}")}),
                };
                println!("cell {cell} tol {tol:e}: {rec}");
                tolerances.push(rec);
            }
            let mut betas = Vec::new();
            let mut points = Vec::new();
            let mut q = p;
            for _ in 0..64 {
                if let Ok(f) = crate::fluid::flash::flash(problem.spec, q, t, &z, None) {
                    betas.push(f.beta);
                    // The whole split, for comparison with an independent flash at the same
                    // inputs: compositions, phase volumes and the vapour volume fraction.
                    let (vl, vv) = (
                        f.liquid.as_ref().map(|l| l.molar_volume),
                        f.vapour.as_ref().map(|v| v.molar_volume),
                    );
                    let vapour_volume_fraction = match (vl, vv) {
                        (Some(vl), Some(vv)) => {
                            Some(f.beta * vv / (f.beta * vv + (1.0 - f.beta) * vl))
                        }
                        _ => None,
                    };
                    points.push(json!({
                        "p_pa": q,
                        "beta": f.beta,
                        "x": f.x,
                        "y": f.y,
                        "liquid_molar_volume": vl,
                        "vapour_molar_volume": vv,
                        "liquid_mass_density": f.liquid.as_ref().map(|l| l.mass_density),
                        "vapour_mass_density": f.vapour.as_ref().map(|v| v.mass_density),
                        "vapour_volume_fraction": vapour_volume_fraction,
                        "equilibrium_residual": f.equilibrium_residual,
                        "iterations": f.iterations,
                    }));
                }
                q = f64::from_bits(q.to_bits() + 1);
            }
            let lo = betas.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = betas.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let dp_bar = (q - p) / 1e5;
            println!(
                "cell {cell}: over 64 ulps ({dp_bar:.2e} bar) beta spans [{lo:.9}, {hi:.9}], spread {:.2e}",
                hi - lo
            );
            cells.push(json!({"cell": cell, "pressure_bar": c.pressure_bar,
                "temperature_k": t, "z": z,
                "tolerances": tolerances, "ulp_sweep": {"ulps": 64, "dp_bar": dp_bar,
                "beta_min": lo, "beta_max": hi, "betas": betas, "points": points}}));
        }
        // The fluid exactly as ResSim holds it, so an independent flash solves the same problem.
        let spec = problem.spec;
        let n = spec.component_count();
        let fluid = json!({
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
        });
        json!({"best_iterate": best + 1, "fluid": fluid, "cells": cells})
    });
    if let Ok(dir) = std::env::var("SPE5_TRACE_OUT") {
        let path = PathBuf::from(dir).join("failure_b_flash_conditioning.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&out).expect("serialize"),
        )
        .expect("write");
    }
    assert!(out["cells"].as_array().is_some_and(|c| c.len() == 3));
}

/// If failure B's floor is the near-critical split's indeterminacy reaching the residual through
/// the face fluxes, its scaled size is roughly `flux noise x dt / inventory` and should fall in
/// proportion to `dt`. This replays the last attempt's `dt` and five further halvings (the step's
/// own ladder stops at ten attempts) and reports each one's floor.
#[test]
#[ignore = "#116 failure-B dt scaling; ~1 min; run with --release --ignored --nocapture"]
fn comp_spe5_failure_b_floor_scales_with_dt() {
    let case = seed();
    let mut rows = Vec::new();
    for k in 0..6 {
        let dt = last_attempt_dt() * 0.5f64.powi(k);
        let attempt = case.with_diagnostic_problem(dt, run);
        print_history(&format!("dt {dt:.3e} d"), &attempt);
        rows.push(
            json!({"dt_days": dt, "summary": floor_summary(&attempt.residual_history),
            "residual_history": attempt.residual_history}),
        );
    }
    if let Ok(dir) = std::env::var("SPE5_TRACE_OUT") {
        let path = PathBuf::from(dir).join("failure_b_dt_scaling.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&rows).expect("serialize"),
        )
        .expect("write");
    }
    assert_eq!(rows.len(), 6);
}
