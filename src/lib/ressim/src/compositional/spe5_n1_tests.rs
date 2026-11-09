//! #116 N1: the experimental split-based phase decision. Diagnostic tests only.
//!
//! N1 is test-only (`with_split_phase_decision_for_test`); production flash behavior is unchanged.
//! These tests record where the decision fires and how the band affects it:
//!
//! - `comp_spe5_n1_trial_sums`: stability trial sums at the M1 continuation states;
//! - `comp_spe5_n1_margin_decisions`: phase decisions changed on the M0 points;
//! - `comp_spe5_n1_band_sweep`: the same count over 1e-12..1e-2;
//! - `comp_spe5_n1_m1_states`: the M1 start/cutoff/collapse states.
//!
//! The from-scratch replay is `comp_spe5_replay_with_timestep_options` with `SPE5_N1=1`.
use super::spe5_m0_branch_tests::{CASES, restore};
use crate::fluid::stability::trial_sums_for_test;
use serde_json::Value;
use std::path::PathBuf;

fn case_for(name: &str) -> &'static str {
    CASES
        .iter()
        .find(|(case, _, _, _)| *case == name)
        .map(|(_, seed, _, _)| *seed)
        .unwrap_or_else(|| panic!("no seed for {name}"))
}

#[test]
#[ignore = "#116 N1 design probe; set SPE5_M1_STATES; seconds"]
fn comp_spe5_n1_trial_sums() {
    let path = std::env::var("SPE5_M1_STATES").expect("set SPE5_M1_STATES");
    let input: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("parse");
    let mut out = Vec::new();
    for case in input["cases"].as_array().expect("cases") {
        let name = case["case"].as_str().expect("case");
        let cell = case["cell"].as_u64().expect("cell") as usize;
        let seed_file = case_for(name);
        let case_file = restore(seed_file);
        let spec = case_file.spec_for_test();
        let z: Vec<f64> = case["z"]
            .as_array()
            .expect("z")
            .iter()
            .filter_map(Value::as_f64)
            .collect();
        for kind in ["start", "trivial_cutoff", "collapse"] {
            let state = &case[kind];
            if !state.is_object() {
                continue;
            }
            let p_pa = state["p_pa"].as_f64().expect("p_pa");
            let temperature = spec.reservoir_temperature_k();
            let trials = trial_sums_for_test(spec, p_pa, temperature, &z, None);
            println!("{name} cell {cell} {kind}:");
            let mut record = Vec::new();
            for (seed, result) in trials {
                match result {
                    Ok((vapour, vapour_trivial, liquid, liquid_trivial)) => {
                        println!(
                            "  {seed}: vapour {:+.3e}{} liquid {:+.3e}{}",
                            vapour - 1.0,
                            if vapour_trivial { " trivial" } else { "" },
                            liquid - 1.0,
                            if liquid_trivial { " trivial" } else { "" }
                        );
                        record.push(serde_json::json!({"seed": seed,
                            "vapour_minus_one": vapour - 1.0, "vapour_trivial": vapour_trivial,
                            "liquid_minus_one": liquid - 1.0, "liquid_trivial": liquid_trivial}));
                    }
                    Err(error) => {
                        println!("  {seed}: error {error}");
                        record.push(serde_json::json!({"seed": seed, "error": error}));
                    }
                }
            }
            out.push(serde_json::json!({"case": name, "cell": cell, "kind": kind,
                "p_pa": p_pa, "trials": record}));
        }
    }
    if let Ok(dir) = std::env::var("SPE5_TRACE_OUT") {
        let path = PathBuf::from(dir).join("n1_trial_sums.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&out).expect("serialize"),
        )
        .expect("write");
        println!("written {}", path.display());
    }
}

#[test]
#[ignore = "#116 N1 margin decision probe; set SPE5_M0_POINTS; seconds"]
fn comp_spe5_n1_margin_decisions() {
    use crate::fluid::flash::{flash, with_split_phase_decision_for_test};
    let path = std::env::var("SPE5_M0_POINTS").expect("set SPE5_M0_POINTS");
    let input: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("parse");
    let mut changed = 0usize;
    for point in input["points"].as_array().expect("points") {
        let name = point["tag"].as_str().expect("tag");
        let cell = point["cell"].as_u64().expect("cell") as usize;
        let p_pa = point["p_pa"].as_f64().expect("p");
        let temperature = point["temperature_k"].as_f64().expect("t");
        let z: Vec<f64> = point["z"]
            .as_array()
            .expect("z")
            .iter()
            .filter_map(Value::as_f64)
            .collect();
        let label = point["standard_label"].as_str().unwrap_or("?");
        let seed_file = case_for(name.split(' ').next().expect("case"));
        let case_file = restore(seed_file);
        let spec = case_file.spec_for_test();
        let off = flash(spec, p_pa, temperature, &z, None).expect("off flash");
        let on = with_split_phase_decision_for_test(true, || {
            flash(spec, p_pa, temperature, &z, None).expect("on flash")
        });
        if off.phase_state != on.phase_state {
            changed += 1;
            println!(
                "{name} cell {cell} recorded {label}: {:?} beta {:.6} -> {:?} beta {:.6}",
                off.phase_state, off.beta, on.phase_state, on.beta
            );
        }
    }
    println!(
        "phase decisions changed at {changed} of {} points",
        input["points"].as_array().expect("points").len()
    );
}

#[test]
#[ignore = "#116 N1 band sweep; set SPE5_M0_POINTS; seconds"]
fn comp_spe5_n1_band_sweep() {
    use crate::fluid::flash::{flash, with_split_phase_decision_for_test};
    use crate::fluid::stability::with_split_decision_band_for_test;
    let path = std::env::var("SPE5_M0_POINTS").expect("set SPE5_M0_POINTS");
    let input: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("parse");
    let points = input["points"].as_array().expect("points");
    for band in [1e-12, 1e-10, 1e-9, 1e-8, 1e-6, 1e-4, 1e-2] {
        let mut changed = 0usize;
        for point in points {
            let name = point["tag"].as_str().expect("tag");
            let p_pa = point["p_pa"].as_f64().expect("p");
            let temperature = point["temperature_k"].as_f64().expect("t");
            let z: Vec<f64> = point["z"]
                .as_array()
                .expect("z")
                .iter()
                .filter_map(Value::as_f64)
                .collect();
            let seed_file = case_for(name.split(' ').next().expect("case"));
            let case_file = restore(seed_file);
            let spec = case_file.spec_for_test();
            let off = flash(spec, p_pa, temperature, &z, None).expect("off flash");
            let on = with_split_decision_band_for_test(band, || {
                with_split_phase_decision_for_test(true, || {
                    flash(spec, p_pa, temperature, &z, None).expect("on flash")
                })
            });
            if off.phase_state != on.phase_state {
                changed += 1;
            }
        }
        println!(
            "band {band:e}: {changed} of {} phase decisions changed",
            points.len()
        );
    }
}

#[test]
#[ignore = "#116 N1 on the M1 continuation states; set SPE5_M1_STATES; seconds"]
fn comp_spe5_n1_m1_states() {
    use crate::fluid::flash::{flash, with_split_phase_decision_for_test};
    let path = std::env::var("SPE5_M1_STATES").expect("set SPE5_M1_STATES");
    let input: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("parse");
    let mut changed = 0usize;
    let mut total = 0usize;
    for case in input["cases"].as_array().expect("cases") {
        let name = case["case"].as_str().expect("case");
        let cell = case["cell"].as_u64().expect("cell") as usize;
        let seed_file = case_for(name);
        let case_file = restore(seed_file);
        let spec = case_file.spec_for_test();
        let z: Vec<f64> = case["z"]
            .as_array()
            .expect("z")
            .iter()
            .filter_map(Value::as_f64)
            .collect();
        for kind in ["start", "trivial_cutoff", "collapse"] {
            let state = &case[kind];
            if !state.is_object() {
                continue;
            }
            total += 1;
            let p_pa = state["p_pa"].as_f64().expect("p_pa");
            let off = flash(spec, p_pa, spec.reservoir_temperature_k(), &z, None).expect("off");
            let on = with_split_phase_decision_for_test(true, || {
                flash(spec, p_pa, spec.reservoir_temperature_k(), &z, None).expect("on")
            });
            if off.phase_state != on.phase_state {
                changed += 1;
                println!(
                    "{name} cell {cell} {kind}: {:?} -> {:?}",
                    off.phase_state, on.phase_state
                );
            }
        }
    }
    println!("N1 changed {changed} of {total} M1 states");
}
