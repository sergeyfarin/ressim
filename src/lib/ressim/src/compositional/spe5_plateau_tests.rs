//! #116 step 0: measurement of the month-218 Newton plateau. **No solver behaviour is changed.**
//!
//! Every Newton iterate of a failing SPE5 attempt is recorded through
//! [`super::newton::solve_newton_observed`]: each cell's phase state and which cells flipped since
//! the previous iterate, the cells above the `1e-8` test, and the residual in three
//! normalizations:
//!
//! - **ResSim's own:** per cell and component, against that component's previous moles.
//! - **JutulDarcy's CNV:** per cell and hydrocarbon component, the mass imbalance times
//!   `(1 - Sw)^2` over the cell's hydrocarbon mass. This is JutulDarcy 0.3.7's
//!   `compositional_criterion` rewritten for a residual already integrated over `dt`: Jutul scales by
//!   `dt (1 - Sw) / (V rho)` with `rho` the hydrocarbon density at saturations renormalized by
//!   `1 - Sw`, so `V rho = m_hc / (1 - Sw)`, hence the square. The water row is
//!   `|R_w| Sw / n_w = |R_w| M_w / (V rho_w)`, Jutul's water CNV except that Jutul compares the
//!   signed value (a negative water residual never raises it), so this is an upper bound on Jutul's.
//! - **JutulDarcy's MB:** per hydrocarbon component, the absolute signed sum of the mass residual
//!   over all cells over the domain's total mass, water included. The water row uses the same
//!   formula, which makes it a water conservation metric and **not** Jutul's water criterion: Jutul
//!   sums per-cell volume-scaled values and multiplies by `dt` twice, which is not a mass balance.
//!
//! It also records the conservation quantities an acceptance rule would have to budget: per
//! component, the signed global imbalance in moles (internal faces cancel, so this is what an
//! accepted iterate would book as numerical gain or loss) and the sum of absolute local defects, so
//! cancellation between cells cannot hide a local error.
//!
//! At each flip cell the iterate is re-flashed: a two-phase iterate reports `g_split - g_feed` in
//! `R T` against the flash's own `1e-10` margin, and a single-phase iterate reports the stability
//! verdict warm-started from the cell's last two-phase `K`.
//!
//! The traces are written as JSON to `SPE5_TRACE_OUT` when it is set. The tests assert only that
//! the measurement ran; they pin no threshold.
use super::api::{Checkpoint, CompositionalCase};
use super::newton::NewtonIterate;
use super::water::WATER_MOLAR_MASS_KG_PER_MOL;
use crate::fluid::flash::{
    GIBBS_ENERGY_TOLERANCE_FOR_TEST, PhaseState, flash, two_phase_gibbs_minus_feed_for_test,
};
use crate::fluid::stability::{StabilityVerdict, test_stability};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;

const RESSIM_TOLERANCE: f64 = 1e-8;

fn seed(name: &str) -> CompositionalCase {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../opm/compositional/spe5")
        .join(name);
    restore(&path)
}

fn restore(path: &std::path::Path) -> CompositionalCase {
    let text = std::fs::read_to_string(path).expect("read the checkpoint");
    let checkpoint: Checkpoint = serde_json::from_str(&text).expect("parse the checkpoint");
    CompositionalCase::restore(&checkpoint).expect("restore the checkpoint")
}

fn phase_label(state: PhaseState) -> &'static str {
    match state {
        PhaseState::SingleLiquid => "L",
        PhaseState::SingleVapour => "V",
        PhaseState::TwoPhase => "2P",
    }
}

/// Everything recorded about one iterate.
fn record_iterate(
    case: &CompositionalCase,
    it: &NewtonIterate<'_>,
    previous_phase: &mut Option<Vec<PhaseState>>,
    last_two_phase_k: &mut HashMap<usize, Vec<f64>>,
) -> Value {
    let spec = case.spec_for_test();
    let t = spec.reservoir_temperature_k();
    let molar_mass: Vec<f64> = spec
        .components()
        .iter()
        .map(|c| c.molar_mass_kg_per_mol)
        .collect();
    let n_hc = molar_mass.len();
    let a = it.assembled;
    let cells = a.inventories.len();
    let block = a.residual.len() / cells;
    let has_water = block > n_hc;

    let phases: Vec<PhaseState> = a.inventories.iter().map(|inv| inv.phase_state).collect();
    let flipped: Vec<usize> = match previous_phase.as_ref() {
        Some(prev) => (0..cells).filter(|&c| prev[c] != phases[c]).collect(),
        None => Vec::new(),
    };

    let mut ressim_max = 0.0f64;
    let mut above = Vec::new();
    let mut cnv = vec![0.0f64; block];
    let mut cnv_cell = vec![0usize; block];
    let mut signed = vec![0.0f64; block];
    let mut absolute = vec![0.0f64; block];
    let mut inventory = vec![0.0f64; block];
    let mut domain_mass = 0.0f64;
    for cell in 0..cells {
        let rows = &a.residual[cell * block..(cell + 1) * block];
        let scaled = a.scaling[cell].scale_residual(rows);
        let worst = scaled.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        ressim_max = ressim_max.max(worst);
        if worst > RESSIM_TOLERANCE {
            above.push(json!({"cell": cell, "scaled": worst,
                "rows": scaled.iter().map(|v| v.abs()).collect::<Vec<_>>()}));
        }
        let inv = &a.inventories[cell];
        let hc_mass: f64 = (0..n_hc)
            .map(|i| inv.component_moles[i] * molar_mass[i])
            .sum();
        let water_moles = if has_water {
            inv.component_moles[n_hc]
        } else {
            0.0
        };
        domain_mass += hc_mass + water_moles * WATER_MOLAR_MASS_KG_PER_MOL;
        let lv = 1.0 - inv.water_saturation;
        for row in 0..block {
            signed[row] += rows[row];
            absolute[row] += rows[row].abs();
            inventory[row] += inv.component_moles[row];
            let value = if row < n_hc {
                rows[row].abs() * molar_mass[row] * lv * lv / hc_mass
            } else if water_moles > 0.0 {
                rows[row].abs() * inv.water_saturation / water_moles
            } else {
                0.0
            };
            if value > cnv[row] {
                cnv[row] = value;
                cnv_cell[row] = cell;
            }
        }
    }
    let mass_of = |row: usize| {
        if row < n_hc {
            molar_mass[row]
        } else {
            WATER_MOLAR_MASS_KG_PER_MOL
        }
    };
    let mb: Vec<f64> = (0..block)
        .map(|row| (signed[row] * mass_of(row)).abs() / domain_mass)
        .collect();

    // Re-flash the cells that flipped, or that sit above the test, at this iterate.
    let mut watch: Vec<usize> = flipped.clone();
    for entry in &above {
        let cell = entry["cell"].as_u64().unwrap_or(0) as usize;
        if !watch.contains(&cell) {
            watch.push(cell);
        }
    }
    let mut cell_detail = Vec::new();
    for &cell in &watch {
        let state = &it.state.cells()[cell];
        let z = state.overall_composition();
        let p_pa = state.pressure_bar * 1e5;
        let inv = &a.inventories[cell];
        let mut detail = json!({
            "cell": cell,
            "p_bar": state.pressure_bar,
            "z": z,
            "phase": phase_label(inv.phase_state),
            "sg": inv.vapour_saturation,
            "sw": inv.water_saturation,
            "flipped": flipped.contains(&cell),
        });
        match flash(spec, p_pa, t, &z, None) {
            Ok(f) => {
                detail["beta"] = json!(f.beta);
                detail["equilibrium_residual"] = json!(f.equilibrium_residual);
                detail["flash_iterations"] = json!(f.iterations);
                let lnk2: f64 = f.k.iter().map(|k| k.ln() * k.ln()).sum();
                detail["sum_ln_k_sq"] = json!(lnk2);
                if f.phase_state == PhaseState::TwoPhase {
                    detail["gibbs_split_minus_feed_rt"] =
                        json!(two_phase_gibbs_minus_feed_for_test(spec, p_pa, t, &z, &f));
                    last_two_phase_k.insert(cell, f.k.clone());
                } else if let Some(k) = last_two_phase_k.get(&cell) {
                    let verdict = test_stability(spec, p_pa, t, &z, Some(k));
                    detail["stability_warm_from_two_phase_k"] = json!(match verdict {
                        Ok(StabilityVerdict::Stable) => "stable".to_string(),
                        Ok(StabilityVerdict::Unstable { .. }) => "unstable".to_string(),
                        Err(e) => format!("error: {e:?}"),
                    });
                }
            }
            Err(e) => detail["flash_error"] = json!(format!("{e:?}")),
        }
        cell_detail.push(detail);
    }

    let well_rows: Vec<Value> = a
        .well_eliminations
        .iter()
        .map(|w| json!({"well": w.well, "scaled": w.residual.abs() / w.scale}))
        .collect();

    *previous_phase = Some(phases);
    json!({
        "iteration": it.iteration,
        "ressim_scaled_max": ressim_max,
        "well_rows": well_rows,
        "cells_above_1e-8": above,
        "flipped_cells": flipped,
        "jutul_cnv": cnv,
        "jutul_cnv_cell": cnv_cell,
        "jutul_mb": mb,
        "signed_imbalance_mol": signed,
        "absolute_local_defect_mol": absolute,
        "signed_imbalance_rel_inventory": (0..block).map(|r| signed[r].abs() / inventory[r]).collect::<Vec<_>>(),
        "cells": cell_detail,
    })
}

fn trace_attempt(case: &CompositionalCase, dt_days: f64) -> Value {
    let mut previous_phase = None;
    let mut last_k = HashMap::new();
    let mut iterates = Vec::new();
    let report = case.diagnostic_newton_trace(dt_days, &mut |it| {
        iterates.push(record_iterate(case, it, &mut previous_phase, &mut last_k));
    });
    json!({
        "dt_days": dt_days,
        "iterations": report.iterations,
        "residual_history": report.residual_history,
        "converged": report.residual_history.last().is_some_and(|r| *r <= RESSIM_TOLERANCE),
        "iterates": iterates,
    })
}

fn summarize(trace: &Value) {
    println!(
        "dt {:e}: {} iterations, converged {}",
        trace["dt_days"].as_f64().unwrap_or(f64::NAN),
        trace["iterations"],
        trace["converged"]
    );
    for it in trace["iterates"].as_array().into_iter().flatten() {
        let max_of = |key: &str| {
            it[key]
                .as_array()
                .map(|v| v.iter().filter_map(Value::as_f64).fold(0.0f64, f64::max))
                .unwrap_or(f64::NAN)
        };
        let above: Vec<u64> = it["cells_above_1e-8"]
            .as_array()
            .map(|v| v.iter().filter_map(|e| e["cell"].as_u64()).collect())
            .unwrap_or_default();
        let well = it["well_rows"]
            .as_array()
            .map(|v| {
                v.iter()
                    .filter_map(|w| w["scaled"].as_f64())
                    .fold(0.0f64, f64::max)
            })
            .unwrap_or(0.0);
        println!(
            "  it {:>2} ressim {:.3e} well {:.3e} cnv {:.3e} mb {:.3e} flipped {} above {:?}",
            it["iteration"],
            it["ressim_scaled_max"].as_f64().unwrap_or(f64::NAN),
            well,
            max_of("jutul_cnv"),
            max_of("jutul_mb"),
            it["flipped_cells"],
            above
        );
    }
}

fn write_traces(name: &str, traces: Value) {
    if let Ok(dir) = std::env::var("SPE5_TRACE_OUT") {
        let path = PathBuf::from(dir).join(name);
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&traces).expect("serialize"),
        )
        .expect("write the trace");
        println!("trace written to {}", path.display());
    }
}

/// Step 0 on the committed month-218 seed (`stall_218.json`, 6612.820363 d), over the timestep
/// ladder the earlier diagnosis reported: `0.02 d` and the `1.95e-5 d` retry ladder down to
/// `2.44e-6 d`.
#[test]
#[ignore = "#116 step-0 measurement; run with --release --ignored --nocapture"]
fn comp_spe5_month218_plateau_trace() {
    let case = seed("stall_218.json");
    println!("GIBBS margin {GIBBS_ENERGY_TOLERANCE_FOR_TEST:e}");
    let ladder = [0.02, 1.953125e-5, 9.765625e-6, 4.8828125e-6, 2.44140625e-6];
    let traces: Vec<Value> = ladder
        .iter()
        .map(|&dt| {
            let trace = trace_attempt(&case, dt);
            summarize(&trace);
            trace
        })
        .collect();
    assert!(
        traces
            .iter()
            .all(|t| t["iterations"].as_u64().unwrap_or(0) > 0)
    );
    write_traces(
        "stall_218_plateau_trace.json",
        json!({"seed": "stall_218.json", "attempts": traces}),
    );
}

/// Step 0 on the from-scratch month-218 failure. Replays SPE5 from t = 0, keeps the checkpoint
/// before every call in month 218, and on the first failed call traces each of its attempts from
/// that checkpoint. With `SPE5_TRACE_OUT` set, the failing call's starting checkpoint is written
/// there too (`from_scratch_218_start.json`); the committed copy is
/// `opm/compositional/spe5/stall_218_from_scratch.json`, which
/// [`comp_spe5_from_scratch_218_seed_reproduces_signature_b`] replays in seconds.
#[test]
#[ignore = "#116 step-0 measurement; ~15 min from scratch; run with --release --ignored"]
fn comp_spe5_from_scratch_month218_failure_trace() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../opm/compositional/spe5/case.json");
    let text = std::fs::read_to_string(path).expect("read case.json");
    let config = serde_json::from_str(&text).expect("parse case.json");
    let mut case = CompositionalCase::new(config).expect("admissible");
    let dt = 30.4375;
    for month in 0..218 {
        let mut remaining = dt;
        while remaining > 1e-9 {
            let before = case.checkpoint();
            let outcome = case.step(remaining);
            if !outcome.succeeded() {
                assert_eq!(month, 217, "unexpected failure in month {}", month + 1);
                println!(
                    "month 218 call failed at {:.6} d after {} attempts: {:?}",
                    outcome.time_days, outcome.attempts, outcome.failure
                );
                if let Ok(dir) = std::env::var("SPE5_TRACE_OUT") {
                    std::fs::write(
                        PathBuf::from(dir).join("from_scratch_218_start.json"),
                        serde_json::to_string(&before).expect("serialize"),
                    )
                    .expect("write the checkpoint");
                }
                let start = CompositionalCase::restore(&before).expect("restore");
                // The call's attempts: the requested remainder, halved on each failure.
                let traces: Vec<Value> = (0..outcome.attempts)
                    .map(|k| {
                        let trace = trace_attempt(&start, remaining * 0.5f64.powi(k as i32));
                        summarize(&trace);
                        trace
                    })
                    .collect();
                write_traces(
                    "from_scratch_218_failure_trace.json",
                    json!({"start_day": before.time_days, "attempts": traces}),
                );
                return;
            }
            remaining -= outcome.accepted_dt_days.expect("accepted");
        }
    }
    panic!("month 218 did not fail; the plateau has moved");
}

/// Signature B from the committed failure-B seed (`stall_218_from_scratch.json`, 6612.703446 d, the
/// state before the from-scratch replay's failing month-218 call), in seconds instead of the
/// ~16-minute replay. It asserts the call still fails the way step 0 measured it, so later work on
/// this failure (flash-tolerance sensitivity, linear-solve diagnostics, a BHP warm start) starts
/// from a state that still shows it:
///
/// - the call fails after its ten halvings of the 22.67 d remainder;
/// - the last attempt (0.0443 d) flips no cell on any iterate, gets below `1e-6` but not to the
///   `1e-8` test, and its signed global imbalance stays below `1e-12` of inventory.
///
/// These bounds pin the measured signature, not an acceptance threshold. With `SPE5_TRACE_OUT` set,
/// the last attempt's trace is written there.
#[test]
#[ignore = "#116 failure-B seed; run with --release --ignored --nocapture"]
fn comp_spe5_from_scratch_218_seed_reproduces_signature_b() {
    let remaining = 22.671554013151308;
    let mut case = seed("stall_218_from_scratch.json");
    let start = case.checkpoint();
    let outcome = case.step(remaining);
    println!(
        "call from {:.6} d: succeeded {}, {} attempts, {:?}",
        start.time_days,
        outcome.succeeded(),
        outcome.attempts,
        outcome.failure
    );
    assert!(!outcome.succeeded(), "the month-218 call no longer fails");
    assert_eq!(outcome.attempts, 10);

    let start = CompositionalCase::restore(&start).expect("restore");
    let last_dt = remaining * 0.5f64.powi(9);
    let trace = trace_attempt(&start, last_dt);
    summarize(&trace);
    let iterates = trace["iterates"].as_array().cloned().unwrap_or_default();
    assert!(!iterates.is_empty());
    let flips: usize = iterates
        .iter()
        .map(|it| it["flipped_cells"].as_array().map_or(0, Vec::len))
        .sum();
    let best = iterates
        .iter()
        .filter_map(|it| it["ressim_scaled_max"].as_f64())
        .fold(f64::INFINITY, f64::min);
    let worst_signed_late = iterates
        .iter()
        .skip(3)
        .filter_map(|it| it["signed_imbalance_rel_inventory"].as_array())
        .flatten()
        .filter_map(Value::as_f64)
        .fold(0.0f64, f64::max);
    println!("last attempt: flips {flips}, best {best:e}, signed (it >= 4) {worst_signed_late:e}");
    assert_eq!(trace["converged"], json!(false));
    assert_eq!(flips, 0, "signature B has no phase flips");
    assert!(
        best > RESSIM_TOLERANCE && best < 1e-6,
        "best iterate {best:e}"
    );
    assert!(
        worst_signed_late < 1e-12,
        "signed imbalance {worst_signed_late:e}"
    );
    write_traces(
        "stall_218_from_scratch_last_attempt.json",
        json!({"seed": "stall_218_from_scratch.json", "attempts": [trace]}),
    );
}
