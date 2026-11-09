//! #116 M0: the assembled residual of **identical primaries** under two phase branches.
//!
//! The crossing traces show cells whose phase label alternates between consecutive Newton
//! iterates. Those iterates differ in every primary, so they cannot say whether the residual has a
//! finite jump at the label change or only a steep continuous transition. This measurement holds
//! everything fixed except the branch of the flipping cells:
//!
//! 1. **Branch comparison.** At each of the cycle's two iterates, with the same primaries in every
//!    cell, the same well BHPs and the same previous inventory, the residual is assembled with the
//!    standard flash and again with the flipping cells forced onto the competing branch (the
//!    single phase on the root Li's rule picks, or a two-phase split solved from the other
//!    iterate's split with no stability test). The difference is decomposed into inventory, face
//!    fluxes and wells, and the decomposition is checked against the assembled difference.
//! 2. **Switch sweep.** Along the straight path between the two iterates (all primaries and BHPs),
//!    the point where the standard label changes is bracketed by bisection, and the standard
//!    residual is compared across it at distances shrinking by decades. A finite jump stays
//!    finite as the distance goes to zero; a steep continuous transition falls with it.
//! 3. **Gibbs data** at every evaluated flipping-cell state, in Float64, and the inputs a
//!    50-digit oracle needs (`tools/jutul/spe5_m0_gibbs_mp.py`).
//!
//! Inputs are the committed crossing seeds (`opm/compositional/spe5/stall_*.json`, provenance in
//! that folder's README) at the failing `dt` of each trace. The default flash is used (no
//! refinement). A seeded split that does not converge is reported as inconclusive, not as proof
//! that no split exists.
//!
//! Run: `SPE5_TRACE_OUT=DIR cargo test --release --lib comp_spe5_m0_branch_comparison --
//! --ignored --nocapture` (writes `m0_branch_comparison.json` and `m0_gibbs_points.json`).
use super::api::{Checkpoint, CompositionalCase};
use super::assembly::AssemblyResult;
use super::flux::face_flux;
use super::newton::{NewtonProblem, assemble_for_test};
use super::spe5_failure_b_tests::run;
use super::spe5_flash_refinement_tests::{describe, fluid_json, write};
use super::state::CompositionalState;
use crate::fluid::flash::{
    FlashState, PhaseState, feed_gibbs_on_branches_for_test, flash, li_labels_liquid_for_test,
    seeded_two_phase_for_test, single_phase_on_branch_for_test, split_gibbs_for_test,
    with_forced_states_for_test,
};
use crate::fluid::units::bar_to_pa;
use crate::math;
use serde_json::{Value, json};
use std::path::PathBuf;

/// `(name, seed file, failing dt [d], flipping cells)`: the last failing rung of each trace.
const CASES: [(&str, &str, f64, &[usize]); 5] = [
    ("cap5", "stall_6556_cap5.json", 9.765625e-3, &[25, 31]),
    (
        "cap5_att12",
        "stall_6556_cap5_att12.json",
        1.220703125e-3,
        &[25, 31],
    ),
    ("cap05", "stall_6432_cap05.json", 9.765625e-4, &[16]),
    (
        "cap05_att12",
        "stall_6432_cap05_att12.json",
        1.220703125e-4,
        &[16],
    ),
    ("memory", "stall_6612_memory.json", 4.8828125e-6, &[33, 39]),
];

fn restore(name: &str) -> CompositionalCase {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../opm/compositional/spe5")
        .join(name);
    let text = std::fs::read_to_string(path).expect("read the seed");
    let checkpoint: Checkpoint = serde_json::from_str(&text).expect("parse the seed");
    CompositionalCase::restore(&checkpoint).expect("restore the seed")
}

fn cell_flash(problem: &NewtonProblem<'_>, state: &CompositionalState, c: usize) -> FlashState {
    let cell = state.cell(c);
    flash(
        problem.spec,
        bar_to_pa(cell.pressure_bar),
        problem.spec.reservoir_temperature_k(),
        &cell.overall_composition(),
        None,
    )
    .expect("standard flash")
}

fn labels(problem: &NewtonProblem<'_>, state: &CompositionalState, cells: &[usize]) -> Vec<String> {
    cells
        .iter()
        .map(|&c| format!("{:?}", cell_flash(problem, state, c).phase_state))
        .collect()
}

fn ln_k_of(f: &FlashState) -> Vec<f64> {
    f.k.iter().map(|&k| math::ln(k)).collect()
}

/// The competing branch for cell `c` at `state`: the Li-rule single phase when the standard flash
/// is two-phase, else a split solved from `seed` (a two-phase state of the same cell elsewhere).
fn competing(
    problem: &NewtonProblem<'_>,
    state: &CompositionalState,
    c: usize,
    seed: Option<&FlashState>,
) -> (Option<FlashState>, Value) {
    let spec = problem.spec;
    let t = spec.reservoir_temperature_k();
    let cell = state.cell(c);
    let (p, z) = (bar_to_pa(cell.pressure_bar), cell.overall_composition());
    let standard = cell_flash(problem, state, c);
    if standard.phase_state == PhaseState::TwoPhase {
        let liquid = li_labels_liquid_for_test(spec, t, &z);
        let single = single_phase_on_branch_for_test(spec, p, t, &z, liquid).ok();
        (
            single,
            json!({"kind": "single phase on Li's root", "li_liquid": liquid}),
        )
    } else {
        let Some(seed) = seed else {
            return (None, json!({"kind": "no two-phase seed"}));
        };
        let s = seeded_two_phase_for_test(spec, p, t, &z, &ln_k_of(seed), seed.beta);
        let info = json!({"kind": "seeded two-phase split", "iterations": s.iterations,
            "residual_norm": s.residual_norm, "beta": s.beta, "sum_ln_k_sq": s.sum_ln_k_sq,
            "converged_two_phase": s.state.is_some() && s.residual_norm < 1e-12,
            "verdict": if s.state.is_some() && s.residual_norm < 1e-12 { "found" }
                       else { "inconclusive (seed did not converge to a physical split)" }});
        let found = s.state.filter(|_| s.residual_norm < 1e-12);
        (found, info)
    }
}

/// Float64 Gibbs data and the 50-digit oracle's inputs for one cell state.
fn gibbs(
    problem: &NewtonProblem<'_>,
    state: &CompositionalState,
    c: usize,
    split: Option<&FlashState>,
    tag: &str,
) -> Value {
    let spec = problem.spec;
    let t = spec.reservoir_temperature_k();
    let cell = state.cell(c);
    let (p, z) = (bar_to_pa(cell.pressure_bar), cell.overall_composition());
    let [g_l, g_v] = feed_gibbs_on_branches_for_test(spec, p, t, &z);
    let li_liquid = li_labels_liquid_for_test(spec, t, &z);
    let g_li = if li_liquid { g_l } else { g_v };
    let g_split = split.and_then(split_gibbs_for_test);
    json!({
        "tag": tag, "cell": c, "p_pa": p, "temperature_k": t, "z": z,
        "standard_label": format!("{:?}", cell_flash(problem, state, c).phase_state),
        "li_liquid": li_liquid,
        "g_feed_liquid_root": g_l, "g_feed_vapour_root": g_v,
        "g_split": g_split,
        "g_split_minus_li_feed": g_split.zip(g_li).map(|(a, b)| a - b),
        "g_split_minus_lower_feed": g_split.map(|a| a - g_l.unwrap_or(f64::INFINITY)
            .min(g_v.unwrap_or(f64::INFINITY))),
        "split_seed": split.map(|s| json!({"ln_k": ln_k_of(s), "beta": s.beta,
            "x": s.x, "y": s.y})),
    })
}

fn assemble(
    problem: &NewtonProblem<'_>,
    state: &CompositionalState,
    bhp: &[f64],
    forced: &[(f64, f64, Vec<f64>, FlashState)],
) -> AssemblyResult {
    with_forced_states_for_test(forced, || {
        assemble_for_test(problem, state, bhp).expect("assemble")
    })
}

fn forced_entry(
    problem: &NewtonProblem<'_>,
    state: &CompositionalState,
    c: usize,
    f: &FlashState,
) -> (f64, f64, Vec<f64>, FlashState) {
    let cell = state.cell(c);
    (
        bar_to_pa(cell.pressure_bar),
        problem.spec.reservoir_temperature_k(),
        cell.overall_composition(),
        f.clone(),
    )
}

/// Largest scaled `|a - b|` per cell, and the overall maximum.
fn scaled_difference(a: &AssemblyResult, b: &AssemblyResult) -> (f64, Vec<(usize, f64)>) {
    let cells = a.inventories.len();
    let block = a.residual.len() / cells;
    let mut worst = 0.0f64;
    let mut per_cell = Vec::new();
    for c in 0..cells {
        let d: Vec<f64> = (0..block)
            .map(|r| a.residual[c * block + r] - b.residual[c * block + r])
            .collect();
        let m = a.scaling[c]
            .scale_residual(&d)
            .iter()
            .fold(0.0f64, |m, v| m.max(v.abs()));
        worst = worst.max(m);
        if m > 0.0 {
            per_cell.push((c, m));
        }
    }
    per_cell.sort_by(|x, y| y.1.total_cmp(&x.1));
    (worst, per_cell)
}

/// The branch comparison at one state: assembled difference, its decomposition, and every face of
/// the flipping cells under both branches.
fn compare_branches(
    problem: &NewtonProblem<'_>,
    state: &CompositionalState,
    bhp: &[f64],
    cells: &[usize],
    seeds: &[Option<FlashState>],
    tag: &str,
    gibbs_points: &mut Vec<Value>,
) -> Value {
    let mut forced = Vec::new();
    let mut per_cell = Vec::new();
    for (k, &c) in cells.iter().enumerate() {
        let standard = cell_flash(problem, state, c);
        let (alt, info) = competing(problem, state, c, seeds[k].as_ref());
        let sw = state.cell(c).water_saturation;
        let split = if standard.phase_state == PhaseState::TwoPhase {
            Some(&standard)
        } else {
            alt.as_ref()
        };
        gibbs_points.push(gibbs(problem, state, c, split, tag));
        per_cell.push(json!({"cell": c, "competing": info,
            "standard": describe(problem, sw, &standard),
            "competing_state": alt.as_ref().map(|a| describe(problem, sw, a))}));
        if let Some(alt) = alt {
            forced.push(forced_entry(problem, state, c, &alt));
        }
    }
    if forced.len() != cells.len() {
        return json!({"tag": tag, "cells": per_cell,
            "comparison": "inconclusive: no competing state for every flipping cell"});
    }
    let standard = assemble(problem, state, bhp, &[]);
    let alternative = assemble(problem, state, bhp, &forced);
    let (worst, by_cell) = scaled_difference(&alternative, &standard);
    let dt = problem.dt_days;
    let block = standard.residual.len() / standard.inventories.len();

    // Decomposition for each flipping cell: R = (n - n_prev) + dt * sum(outflow) - dt * (wells +
    // sources), so with everything else fixed dR = dn + dt * sum(d outflow) - dt * d wells.
    let mut faces_out = Vec::new();
    let mut decomposition = Vec::new();
    for &c in cells {
        let mut d_flux = vec![0.0; block];
        for face in problem
            .faces
            .iter()
            .filter(|f| f.cell_i == c || f.cell_j == c)
        {
            let eval = |forced: &[(f64, f64, Vec<f64>, FlashState)]| {
                with_forced_states_for_test(forced, || {
                    face_flux(
                        problem.spec,
                        problem.relperm,
                        face.geom_t,
                        face.gravity,
                        (face.cell_i, state.cell(face.cell_i)),
                        (face.cell_j, state.cell(face.cell_j)),
                    )
                    .expect("face flux")
                })
            };
            let (a, b) = (eval(&[]), eval(&forced));
            let sign = if face.cell_i == c { 1.0 } else { -1.0 };
            for (r, d) in d_flux.iter_mut().enumerate() {
                *d += sign * (b.component_moles_per_day[r] - a.component_moles_per_day[r]);
            }
            let relative: Vec<f64> = (0..block)
                .map(|r| {
                    let (x, y) = (a.component_moles_per_day[r], b.component_moles_per_day[r]);
                    (y - x).abs() / x.abs().max(y.abs()).max(f64::MIN_POSITIVE)
                })
                .collect();
            faces_out.push(json!({
                "cell": c, "neighbour": if face.cell_i == c { face.cell_j } else { face.cell_i },
                "standard": {"component_moles_per_day": a.component_moles_per_day,
                    "phase_rates_m3_per_day": a.phase_rates_m3_per_day,
                    "phase_potential_bar": a.phase_potential_bar,
                    "upstream": format!("{:?}", a.upstream)},
                "competing": {"component_moles_per_day": b.component_moles_per_day,
                    "phase_rates_m3_per_day": b.phase_rates_m3_per_day,
                    "phase_potential_bar": b.phase_potential_bar,
                    "upstream": format!("{:?}", b.upstream)},
                "relative_change": relative,
            }));
        }
        let d_inv: Vec<f64> = (0..block)
            .map(|r| {
                alternative.inventories[c].component_moles[r]
                    - standard.inventories[c].component_moles[r]
            })
            .collect();
        let d_res: Vec<f64> = (0..block)
            .map(|r| alternative.residual[c * block + r] - standard.residual[c * block + r])
            .collect();
        // Wells are reported per well, not per connection; a flipping cell with no connection
        // contributes nothing, which the identity check below confirms.
        let identity: Vec<f64> = (0..block)
            .map(|r| d_res[r] - (d_inv[r] + dt * d_flux[r]))
            .collect();
        let inventory: Vec<f64> = standard.inventories[c].component_moles.clone();
        decomposition.push(json!({
            "cell": c,
            "d_residual_mol": d_res,
            "d_inventory_mol": d_inv,
            "d_inventory_rel_component": d_inv.iter().zip(&inventory)
                .map(|(d, n)| d / n.abs().max(f64::MIN_POSITIVE)).collect::<Vec<_>>(),
            "d_inventory_rel_cell_total": d_inv.iter()
                .map(|d| d / inventory.iter().map(|v| v.abs()).sum::<f64>()).collect::<Vec<_>>(),
            "dt_d_outflow_mol": d_flux.iter().map(|v| dt * v).collect::<Vec<_>>(),
            "identity_defect_mol": identity,
            "scaled_d_residual": standard.scaling[c].scale_residual(&d_res)
                .iter().map(|v| v.abs()).collect::<Vec<_>>(),
        }));
    }
    let d_wells: Vec<Vec<f64>> = standard
        .well_rates
        .iter()
        .zip(&alternative.well_rates)
        .map(|(a, b)| a.iter().zip(b).map(|(x, y)| y - x).collect())
        .collect();
    json!({
        "tag": tag,
        "cells": per_cell,
        "max_scaled_d_residual": worst,
        "cells_changed": by_cell.iter().take(12).map(|(c, m)| json!([c, m])).collect::<Vec<_>>(),
        "decomposition": decomposition,
        "d_well_rates_mol_per_day": d_wells,
        "faces": faces_out,
        "standard_scaled_norm": standard.scaled_residual_norm(problem.layout),
        "competing_scaled_norm": alternative.scaled_residual_norm(problem.layout),
    })
}

/// `a + s (b - a)` in every primary of every cell, exactly `b` at `s = 1`.
fn along(a: &CompositionalState, b: &CompositionalState, s: f64) -> CompositionalState {
    if s >= 1.0 {
        return b.clone();
    }
    let mut trial = a.begin_trial();
    for c in 0..a.cells().len() {
        let (x, y) = (a.cell(c), b.cell(c));
        let cell = trial.cell_mut(c);
        cell.pressure_bar = x.pressure_bar + s * (y.pressure_bar - x.pressure_bar);
        cell.water_saturation = x.water_saturation + s * (y.water_saturation - x.water_saturation);
        let z: Vec<f64> = x
            .independent_z()
            .iter()
            .zip(y.independent_z())
            .map(|(u, v)| u + s * (v - u))
            .collect();
        cell.independent_z_mut().copy_from_slice(&z);
    }
    trial.commit().expect("an interpolated state is admissible")
}

fn lerp(a: &[f64], b: &[f64], s: f64) -> Vec<f64> {
    if s >= 1.0 {
        return b.to_vec();
    }
    a.iter().zip(b).map(|(u, v)| u + s * (v - u)).collect()
}

#[test]
#[ignore = "#116 M0 branch measurement; minutes; --release --ignored --nocapture"]
fn comp_spe5_m0_branch_comparison() {
    let mut out = Vec::new();
    let mut gibbs_points = Vec::new();
    let mut fluid = Value::Null;
    for (name, seed_file, dt, cells) in CASES {
        let case = restore(seed_file);
        let record = case.with_diagnostic_problem(dt, |problem, start, options| {
            fluid = fluid_json(problem);
            let attempt = run(problem, start, options);
            let its = &attempt.iterates;
            // The cycle's last two iterates, ordered (two-phase side, single side).
            let (k1, k2) = (its.len() - 2, its.len() - 1);
            let l1 = labels(problem, &its[k1].0, cells);
            let l2 = labels(problem, &its[k2].0, cells);
            println!(
                "{name}: dt {dt:e}, iterates {}/{}: {l1:?} / {l2:?}",
                k1 + 1,
                k2 + 1
            );
            let two_phase_first = l1.iter().all(|l| l == "TwoPhase");
            let (ia, ib) = if two_phase_first { (k1, k2) } else { (k2, k1) };
            let (a, bhp_a) = (&its[ia].0, &its[ia].1);
            let (b, bhp_b) = (&its[ib].0, &its[ib].1);
            let splits_a: Vec<Option<FlashState>> = cells
                .iter()
                .map(|&c| Some(cell_flash(problem, a, c)))
                .collect();

            // 1. Branch comparison at both iterates.
            let at_a = compare_branches(
                problem,
                a,
                bhp_a,
                cells,
                &splits_a,
                &format!("{name} iterate {} (two-phase side)", ia + 1),
                &mut gibbs_points,
            );
            let at_b = compare_branches(
                problem,
                b,
                bhp_b,
                cells,
                &splits_a,
                &format!("{name} iterate {} (single side)", ib + 1),
                &mut gibbs_points,
            );

            // 2. The switch along the path a -> b.
            let la = labels(problem, a, cells);
            let profile: Vec<Value> = (0..=20)
                .map(|i| {
                    let s = i as f64 / 20.0;
                    json!([s, labels(problem, &along(a, b, s), cells)])
                })
                .collect();
            let (mut lo, mut hi) = (0.0f64, 1.0f64);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if mid <= lo || mid >= hi {
                    break;
                }
                if labels(problem, &along(a, b, mid), cells) == la {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let path_length_bar = cells
                .iter()
                .map(|&c| (b.cell(c).pressure_bar - a.cell(c).pressure_bar).abs())
                .fold(0.0f64, f64::max);
            let mut sweep = Vec::new();
            for e in 1..=15 {
                let eps = 10f64.powi(-e);
                let (s0, s1) = ((lo - eps).max(0.0), (hi + eps).min(1.0));
                if s0 == 0.0 && s1 == 1.0 && e > 1 {
                    continue;
                }
                let (x0, x1) = (along(a, b, s0), along(a, b, s1));
                let (b0, b1) = (lerp(bhp_a, bhp_b, s0), lerp(bhp_a, bhp_b, s1));
                let (r0, r1) = (
                    assemble(problem, &x0, &b0, &[]),
                    assemble(problem, &x1, &b1, &[]),
                );
                let (jump, by_cell) = scaled_difference(&r1, &r0);
                let flipping_max = by_cell
                    .iter()
                    .filter(|(c, _)| cells.contains(c))
                    .map(|(_, m)| *m)
                    .fold(0.0f64, f64::max);
                let neighbours: Vec<usize> = problem
                    .faces
                    .iter()
                    .filter(|f| cells.contains(&f.cell_i) || cells.contains(&f.cell_j))
                    .flat_map(|f| [f.cell_i, f.cell_j])
                    .collect();
                let elsewhere = by_cell
                    .iter()
                    .filter(|(c, _)| !neighbours.contains(c))
                    .map(|(_, m)| *m)
                    .fold(0.0f64, f64::max);
                let seeds0: Vec<Option<FlashState>> = cells
                    .iter()
                    .map(|&c| Some(cell_flash(problem, &x0, c)))
                    .collect();
                let tag0 = format!("{name} switch -{eps:e}");
                let tag1 = format!("{name} switch +{eps:e}");
                let (branch0, branch1) = if e >= 6 && e % 3 == 0 {
                    (
                        Some(compare_branches(
                            problem,
                            &x0,
                            &b0,
                            cells,
                            &seeds0,
                            &tag0,
                            &mut gibbs_points,
                        )),
                        Some(compare_branches(
                            problem,
                            &x1,
                            &b1,
                            cells,
                            &seeds0,
                            &tag1,
                            &mut gibbs_points,
                        )),
                    )
                } else {
                    (None, None)
                };
                println!(
                    "  eps {eps:e}: labels {:?} -> {:?}, scaled jump {jump:.3e} \
                          (flipping cells {flipping_max:.3e}, cells not touching them \
                          {elsewhere:.3e})",
                    labels(problem, &x0, cells),
                    labels(problem, &x1, cells)
                );
                sweep.push(json!({
                    "eps": eps, "s": [s0, s1],
                    "labels": [labels(problem, &x0, cells), labels(problem, &x1, cells)],
                    "max_scaled_jump": jump, "flipping_cells_max": flipping_max,
                    "cells_not_touching_max": elsewhere,
                    "cells_changed": by_cell.iter().take(8).map(|(c, m)| json!([c, m]))
                        .collect::<Vec<_>>(),
                    "branch_comparison_minus": branch0, "branch_comparison_plus": branch1,
                }));
            }
            println!(
                "  branch comparison: at the two-phase iterate {:.3e}, at the single iterate \
                      {:.3e}",
                at_a["max_scaled_d_residual"].as_f64().unwrap_or(f64::NAN),
                at_b["max_scaled_d_residual"].as_f64().unwrap_or(f64::NAN)
            );
            json!({
                "case": name, "seed": seed_file, "dt_days": dt, "flipping_cells": cells,
                "residual_history": attempt.residual_history,
                "iterates_compared": [ia + 1, ib + 1],
                "branch_comparison": [at_a, at_b],
                "path": {"max_cell_pressure_change_bar": path_length_bar,
                    "label_profile": profile, "bracket": [lo, hi],
                    "bracket_labels": [labels(problem, &along(a, b, lo), cells),
                        labels(problem, &along(a, b, hi), cells)]},
                "switch_sweep": sweep,
            })
        });
        out.push(record);
    }
    write("m0_branch_comparison.json", &json!({"cases": out}));
    write(
        "m0_gibbs_points.json",
        &json!({"fluid": fluid, "points": gibbs_points}),
    );
}

/// The stability test's trial sums at every M0 point (`SPE5_M0_POINTS`, the
/// `m0_gibbs_points.json` the test above writes): for each seed (the point's two-phase split's K
/// when it has one, Wilson, Wilson's cube root), `sum W - 1` of both trials against the verdict's
/// `TRIAL_SUM_TOLERANCE`. Seconds.
#[test]
#[ignore = "#116 M0 trial sums; seconds; set SPE5_M0_POINTS; --release --ignored --nocapture"]
fn comp_spe5_m0_trial_sums() {
    use crate::fluid::stability::{TRIAL_SUM_TOLERANCE_FOR_TEST, trial_sums_for_test};
    let path = std::env::var("SPE5_M0_POINTS").expect("set SPE5_M0_POINTS");
    let points: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("parse");
    let case = restore(CASES[0].1);
    let spec = case.spec_for_test();
    println!("verdict threshold: sum - 1 >= {TRIAL_SUM_TOLERANCE_FOR_TEST:e}");
    let mut out = Vec::new();
    for pt in points["points"].as_array().expect("points") {
        let p = pt["p_pa"].as_f64().expect("p");
        let t = pt["temperature_k"].as_f64().expect("t");
        let z: Vec<f64> = pt["z"]
            .as_array()
            .expect("z")
            .iter()
            .filter_map(Value::as_f64)
            .collect();
        let seed: Option<Vec<f64>> = pt["split_seed"]["ln_k"]
            .as_array()
            .map(|v| v.iter().filter_map(Value::as_f64).map(math::exp).collect());
        let sums = trial_sums_for_test(spec, p, t, &z, seed.as_deref());
        let text: Vec<String> = sums
            .iter()
            .map(|(name, r)| match r {
                Ok((v, vt, l, lt)) => format!(
                    "{name}: V {:+.3e}{} L {:+.3e}{}",
                    v - 1.0,
                    if *vt { " (trivial)" } else { "" },
                    l - 1.0,
                    if *lt { " (trivial)" } else { "" }
                ),
                Err(e) => format!("{name}: error {e}"),
            })
            .collect();
        println!(
            "{} cell {} {}: {}",
            pt["tag"].as_str().unwrap_or("?"),
            pt["cell"],
            pt["standard_label"].as_str().unwrap_or("?"),
            text.join("; ")
        );
        out.push(json!({"tag": pt["tag"], "cell": pt["cell"],
            "standard_label": pt["standard_label"],
            "trials": sums.iter().map(|(name, r)| match r {
                Ok((v, vt, l, lt)) => json!({"seed": name, "vapour_sum_minus_one": v - 1.0,
                    "vapour_trivial": vt, "liquid_sum_minus_one": l - 1.0, "liquid_trivial": lt}),
                Err(e) => json!({"seed": name, "error": e}),
            }).collect::<Vec<_>>()}));
    }
    write(
        "m0_trial_sums.json",
        &json!({"threshold": TRIAL_SUM_TOLERANCE_FOR_TEST, "points": out}),
    );
}
