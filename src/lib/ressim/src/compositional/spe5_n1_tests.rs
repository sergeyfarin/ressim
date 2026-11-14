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

#[test]
#[ignore = "#116 N1 margin gate; set SPE5_M0_POINTS; seconds"]
fn comp_spe5_n1_out_of_band_companion_is_not_a_margin_seed() {
    use crate::fluid::stability::test_stability_with_margin_seed;
    let path = std::env::var("SPE5_M0_POINTS").expect("set SPE5_M0_POINTS");
    let input: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("parse");
    let point = input["points"]
        .as_array()
        .expect("points")
        .iter()
        .find(|point| point["tag"] == "cap05 iterate 20 (single side)" && point["cell"] == 16)
        .expect("the review's cap05 cell-16 point");
    let seed_file = case_for("cap05");
    let case_file = restore(seed_file);
    let spec = case_file.spec_for_test();
    let z: Vec<f64> = point["z"]
        .as_array()
        .expect("z")
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    let p_pa = point["p_pa"].as_f64().expect("p") + 100.0;
    let result =
        test_stability_with_margin_seed(spec, p_pa, spec.reservoir_temperature_k(), &z, None)
            .expect("stability");
    assert!(
        result.margin_seed.is_none(),
        "a trivial sum of one must not qualify an out-of-band non-trivial companion"
    );
}

#[test]
#[ignore = "#116 N1 + refinement matched-inventory memory-seed controls; seconds"]
fn comp_spe5_n1_refinement_memory_seed_controls() {
    use super::spe5_failure_b_tests::run;
    use crate::fluid::flash::{
        with_difference_refinement_for_test, with_split_phase_decision_for_test,
    };
    let case = restore("stall_6612_memory.json");
    let dt = 4.8828125e-6;
    let modes = case.with_diagnostic_problem(dt, |problem, start, options| {
        // One problem, one previous inventory, one start state, one set of wells/sources and one
        // dt. Only refinement and N1 change between the four solves.
        let mut out = Vec::new();
        for refine in [false, true] {
            for n1 in [false, true] {
                let history = with_difference_refinement_for_test(refine, || {
                    with_split_phase_decision_for_test(n1, || {
                        run(problem, start, options).residual_history
                    })
                });
                let best = history.iter().copied().fold(f64::INFINITY, f64::min);
                println!(
                    "refine {refine} n1 {n1}: {} iterations, best {best:.10e}, last {:.10e}",
                    history.len(),
                    history.last().copied().unwrap_or(f64::NAN)
                );
                out.push((refine, n1, history.len(), best));
            }
        }
        out
    });
    assert_eq!(modes.len(), 4);
    for (refine, n1, iterations, best) in modes {
        if n1 {
            assert_eq!(iterations, 4, "N1 must converge in four iterations");
            assert!(best <= 1e-8, "N1 must converge, got {best:e}");
        } else {
            assert_eq!(iterations, 20, "N1 off must exhaust the Newton cap");
            assert!(best > 1e-8, "N1 off must stall, got {best:e}");
            if refine {
                assert!(
                    (1.4e-6..1.5e-6).contains(&best),
                    "refined N1-off floor moved: {best:e}"
                );
            } else {
                assert!(
                    (8.8e-7..9.0e-7).contains(&best),
                    "unrefined N1-off floor moved: {best:e}"
                );
            }
        }
        if refine && n1 {
            assert!(
                best < 1e-12,
                "refinement + N1 must reach the reported 3.77e-13, got {best:e}"
            );
        }
    }
}

#[test]
#[ignore = "#116 failure-B last rung with refinement, N1 off/on; minute"]
fn comp_spe5_n1_refinement_failure_b_last_rung_controls() {
    use super::spe5_failure_b_tests::{last_attempt_dt, run, seed};
    use crate::fluid::flash::{
        with_difference_refinement_for_test, with_split_phase_decision_for_test,
    };
    let case = seed();
    let dt = last_attempt_dt();
    let (off, on) = case.with_diagnostic_problem(dt, |problem, start, options| {
        let off = with_difference_refinement_for_test(true, || {
            with_split_phase_decision_for_test(false, || run(problem, start, options))
        });
        let on = with_difference_refinement_for_test(true, || {
            with_split_phase_decision_for_test(true, || run(problem, start, options))
        });
        (off.residual_history, on.residual_history)
    });
    for (label, history) in [("N1 off", off), ("N1 on", on)] {
        let best = history.iter().copied().fold(f64::INFINITY, f64::min);
        println!(
            "failure B last rung, refined {label}: {} iterations, best {best:.6e}, last {:.6e}",
            history.len(),
            history.last().copied().unwrap_or(f64::NAN)
        );
        assert!(
            best <= 1e-8,
            "failure B's last rung must converge with refinement, {label}: {best:e}"
        );
    }
}

#[test]
#[ignore = "#116 next failing call: branch, seed and cutoff diagnostics; seconds"]
fn comp_spe5_n1_next_call_phase_diagnostics() {
    use super::spe5_failure_b_tests::{run, taylor};
    use crate::fluid::flash::{
        feed_gibbs_on_branches_for_test, flash, seeded_two_phase_for_test,
        solve_rachford_rice_extended, split_gibbs_for_test, two_phase_gibbs_minus_feed_for_test,
        with_difference_refinement_for_test, with_split_phase_decision_for_test,
    };
    use crate::fluid::stability::{TRIAL_SUM_TOLERANCE_FOR_TEST, test_stability_with_margin_seed};
    use crate::fluid::units::bar_to_pa;
    use crate::math;

    let case = restore("stall_n1_6612_836.json");
    let dt = 0.04402126141911289;
    let out = with_difference_refinement_for_test(true, || {
        with_split_phase_decision_for_test(true, || {
            case.with_diagnostic_problem(dt, |problem, start, options| {
                // The previous inventory is built under the same flags because the whole
                // diagnostic, including this closure, runs inside them.
                let attempt = run(problem, start, options);
                let iterates = &attempt.iterates;
                let step = taylor(problem, &attempt, iterates.len() - 2);
        println!(
            "step from iterate {} to {}: {}",
            iterates.len() - 1,
            iterates.len(),
            step
        );
        let spec = problem.spec;
        let temperature = spec.reservoir_temperature_k();
        let mut records = Vec::new();
        for index in [iterates.len() - 2, iterates.len() - 1] {
            let (state, _) = &iterates[index];
            for cell in [33usize, 39] {
                let target = state.cell(cell);
                let p_pa = bar_to_pa(target.pressure_bar);
                let z = target.overall_composition();
                let standard = with_split_phase_decision_for_test(false, || {
                    flash(spec, p_pa, temperature, &z, None).expect("standard flash")
                });
                let n1 = with_split_phase_decision_for_test(true, || {
                    flash(spec, p_pa, temperature, &z, None).expect("N1 flash")
                });
                let stability = test_stability_with_margin_seed(spec, p_pa, temperature, &z, None)
                    .expect("stability");
                let seed_norm = stability.margin_seed.as_ref().map(|seed| {
                    seed.iter()
                        .map(|value| {
                            let ln = math::ln(*value);
                            ln * ln
                        })
                        .sum::<f64>()
                });
                let trials = trial_sums_for_test(spec, p_pa, temperature, &z, None);
                let other = &iterates[if index + 1 == iterates.len() {
                    index - 1
                } else {
                    index + 1
                }]
                .0;
                let other_cell = other.cell(cell);
                let other_p = bar_to_pa(other_cell.pressure_bar);
                let other_z = other_cell.overall_composition();
                let other_state = with_split_phase_decision_for_test(false, || {
                    flash(spec, other_p, temperature, &other_z, None).expect("other flash")
                });
                let (candidate, candidate_beta, candidate_sum, candidate_dg) = if other_state
                    .phase_state
                    == crate::fluid::flash::PhaseState::TwoPhase
                {
                    let ln_k: Vec<f64> =
                        other_state.k.iter().map(|value| math::ln(*value)).collect();
                    let beta = solve_rachford_rice_extended(&other_state.k, &z).ok();
                    beta.map_or((false, f64::NAN, f64::NAN, None), |beta| {
                        let seeded =
                            seeded_two_phase_for_test(spec, p_pa, temperature, &z, &ln_k, beta);
                        let dg = seeded.state.as_ref().and_then(|split| {
                            two_phase_gibbs_minus_feed_for_test(spec, p_pa, temperature, &z, split)
                        });
                        (
                            seeded.state.is_some() && seeded.residual_norm < 1e-12,
                            seeded.beta,
                            seeded.sum_ln_k_sq,
                            dg,
                        )
                    })
                } else {
                    (false, f64::NAN, f64::NAN, None)
                };
                let g_feed = feed_gibbs_on_branches_for_test(spec, p_pa, temperature, &z);
                let g_split = if n1.phase_state == crate::fluid::flash::PhaseState::TwoPhase {
                    split_gibbs_for_test(&n1)
                } else {
                    None
                };
                println!(
                    "iter {} cell {}: p {:.9} bar, standard {:?} beta {:.6}, N1 {:?} beta {:.6}, \
                     margin seed norm {:?}, candidate {:?} beta {:.6} sum {:.4e} dg {:?}",
                    index + 1,
                    cell,
                    target.pressure_bar,
                    standard.phase_state,
                    standard.beta,
                    n1.phase_state,
                    n1.beta,
                    seed_norm,
                    candidate,
                    candidate_beta,
                    candidate_sum,
                    candidate_dg
                );
                let trial_records: Vec<Value> = trials
                    .into_iter()
                    .map(|(seed, result)| match result {
                        Ok((vapour, vapour_trivial, liquid, liquid_trivial)) => {
                            serde_json::json!({"seed": seed, "vapour_minus_one": vapour - 1.0,
                                "vapour_trivial": vapour_trivial,
                                "liquid_minus_one": liquid - 1.0,
                                "liquid_trivial": liquid_trivial})
                        }
                        Err(error) => serde_json::json!({"seed": seed, "error": error}),
                    })
                    .collect();
                records.push(serde_json::json!({
                    "iteration": index + 1, "cell": cell,
                    "p_bar": target.pressure_bar, "z": z,
                    "standard_phase": format!("{:?}", standard.phase_state),
                    "standard_beta": standard.beta,
                    "n1_phase": format!("{:?}", n1.phase_state),
                    "n1_beta": n1.beta,
                    "margin_seed_norm": seed_norm,
                    "verdict": format!("{:?}", stability.verdict),
                    "candidate": candidate,
                    "candidate_beta": candidate_beta,
                    "candidate_sum_ln_k_sq": candidate_sum,
                    "candidate_gibbs_minus_feed": candidate_dg,
                    "n1_gibbs": g_split,
                    "feed_gibbs": g_feed,
                    "trials": trial_records,
                }));
            }
        }
        serde_json::json!({"dt_days": dt, "trial_sum_tolerance": TRIAL_SUM_TOLERANCE_FOR_TEST,
            "attempt_iterations": attempt.residual_history, "points": records})
            })
        })
    });
    if let Ok(dir) = std::env::var("SPE5_TRACE_OUT") {
        let path = PathBuf::from(dir).join("n1_next_call_diagnostics.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&out).expect("serialize"),
        )
        .expect("write");
        println!("written {}", path.display());
    }
}

#[test]
#[ignore = "#116 next failing call: damped-step probe; seconds"]
fn comp_spe5_n1_next_call_step_damping_probe() {
    use super::newton::with_step_damping_for_test;
    use super::spe5_failure_b_tests::run;
    use crate::fluid::flash::{
        with_difference_refinement_for_test, with_split_phase_decision_for_test,
    };
    let case = restore("stall_n1_6612_836.json");
    let dt = 0.04402126141911289;
    let histories = with_difference_refinement_for_test(true, || {
        with_split_phase_decision_for_test(true, || {
            case.with_diagnostic_problem(dt, |problem, start, options| {
                let mut out = Vec::new();
                for factor in [1.0, 0.5, 0.25, 0.1, 0.05, 0.01] {
                    let history = with_step_damping_for_test(factor, || {
                        run(problem, start, options).residual_history
                    });
                    out.push((format!("{factor}"), history));
                }
                let mut longer = options;
                longer.max_iterations = 40;
                let history = with_step_damping_for_test(0.5, || {
                    run(problem, start, longer).residual_history
                });
                out.push(("0.5 with max_iterations 40".to_string(), history));
                out
            })
        })
    });
    for (label, history) in histories {
        let best = history.iter().copied().fold(f64::INFINITY, f64::min);
        let last = history.last().copied().unwrap_or(f64::NAN);
        println!(
            "step damping {label}: {} iterations, best {best:.6e}, last {last:.6e}",
            history.len()
        );
    }
}

#[test]
#[ignore = "#116 next failing call: actual-path switch probe; seconds"]
fn comp_spe5_n1_next_call_switch_probe() {
    use super::assembly::AssemblyResult;
    use super::newton::assemble_for_test;
    use super::spe5_failure_b_tests::run;
    use super::state::CompositionalState;
    use crate::fluid::flash::{
        PhaseState, feed_gibbs_on_branches_for_test, flash, seeded_two_phase_for_test,
        solve_rachford_rice_extended, two_phase_gibbs_minus_feed_for_test,
        with_difference_refinement_for_test, with_forced_states_for_test,
        with_split_phase_decision_for_test,
    };
    use crate::fluid::stability::test_stability_with_margin_seed;
    use crate::fluid::units::bar_to_pa;

    fn along(a: &CompositionalState, b: &CompositionalState, s: f64) -> CompositionalState {
        if s >= 1.0 {
            return b.clone();
        }
        let mut trial = a.begin_trial();
        for c in 0..a.cells().len() {
            let (x, y) = (a.cell(c), b.cell(c));
            let target = trial.cell_mut(c);
            target.pressure_bar = x.pressure_bar + s * (y.pressure_bar - x.pressure_bar);
            target.water_saturation =
                x.water_saturation + s * (y.water_saturation - x.water_saturation);
            let z: Vec<f64> = x
                .independent_z()
                .iter()
                .zip(y.independent_z())
                .map(|(u, v)| u + s * (v - u))
                .collect();
            target.independent_z_mut().copy_from_slice(&z);
        }
        trial.commit().expect("interpolated state")
    }

    fn label(
        state: &CompositionalState,
        cell: usize,
        spec: &crate::fluid::specification::FluidSpecification,
    ) -> PhaseState {
        let c = state.cell(cell);
        flash(
            spec,
            bar_to_pa(c.pressure_bar),
            spec.reservoir_temperature_k(),
            &c.overall_composition(),
            None,
        )
        .expect("flash")
        .phase_state
    }

    fn scaled_cell_jump(a: &AssemblyResult, b: &AssemblyResult, cell: usize) -> f64 {
        let block = a.residual.len() / a.inventories.len();
        let d: Vec<f64> = (0..block)
            .map(|row| a.residual[cell * block + row] - b.residual[cell * block + row])
            .collect();
        a.scaling[cell]
            .scale_residual(&d)
            .iter()
            .fold(0.0f64, |m, value| m.max(value.abs()))
    }

    let case = restore("stall_n1_6612_836.json");
    let dt = 0.04402126141911289;
    let out = with_difference_refinement_for_test(true, || {
        with_split_phase_decision_for_test(true, || {
            case.with_diagnostic_problem(dt, |problem, start, options| {
                // The previous inventory is constructed under the same flags because the whole
                // closure runs inside them.
                let attempt = run(problem, start, options);
                let iterates = &attempt.iterates;
                let (a, bhp_a) = (&iterates[iterates.len() - 2].0, &iterates[iterates.len() - 2].1);
                let (b, bhp_b) = (&iterates[iterates.len() - 1].0, &iterates[iterates.len() - 1].1);
                let mut records = Vec::new();
                for cell in [33usize, 39] {
                    let la = label(a, cell, problem.spec);
                    let lb = label(b, cell, problem.spec);
                    assert_ne!(la, lb, "cell {cell} does not switch in the last step");
                    let (mut lo, mut hi) = (0.0f64, 1.0f64);
                    for _ in 0..200 {
                        let mid = 0.5 * (lo + hi);
                        if mid <= lo || mid >= hi {
                            break;
                        }
                        if label(&along(a, b, mid), cell, problem.spec) == la {
                            lo = mid;
                        } else {
                            hi = mid;
                        }
                    }
                    let x0 = along(a, b, lo);
                    let x1 = along(a, b, hi);
                    let bhp = if lo >= 1.0 { bhp_b } else { bhp_a };
                    let mut points = Vec::new();
                    let mut split_x0 = None;
                    for (side, state) in [("lower", &x0), ("upper", &x1)] {
                        let c = state.cell(cell);
                        let p_pa = bar_to_pa(c.pressure_bar);
                        let z = c.overall_composition();
                        let standard = with_split_phase_decision_for_test(false, || {
                            flash(problem.spec, p_pa, problem.spec.reservoir_temperature_k(), &z, None)
                                .expect("standard flash")
                        });
                        let n1 = flash(problem.spec, p_pa, problem.spec.reservoir_temperature_k(), &z, None)
                            .expect("N1 flash");
                        if side == "lower" && n1.phase_state == PhaseState::TwoPhase {
                            split_x0 = Some(n1.clone());
                        }
                        let stability = test_stability_with_margin_seed(
                            problem.spec,
                            p_pa,
                            problem.spec.reservoir_temperature_k(),
                            &z,
                            None,
                        )
                        .expect("stability");
                        let trials = crate::fluid::stability::trial_sums_for_test(
                            problem.spec,
                            p_pa,
                            problem.spec.reservoir_temperature_k(),
                            &z,
                            None,
                        );
                        let trial_records: Vec<Value> = trials
                            .into_iter()
                            .map(|(seed, result)| match result {
                                Ok((vapour, vapour_trivial, liquid, liquid_trivial)) => {
                                    serde_json::json!({"seed": seed,
                                        "vapour_minus_one": vapour - 1.0,
                                        "vapour_trivial": vapour_trivial,
                                        "liquid_minus_one": liquid - 1.0,
                                        "liquid_trivial": liquid_trivial})
                                }
                                Err(error) => serde_json::json!({"seed": seed, "error": error}),
                            })
                            .collect();
                        points.push(serde_json::json!({"side": side, "s": if side == "lower" { lo } else { hi },
                            "p_bar": c.pressure_bar, "z": z,
                            "standard_phase": format!("{:?}", standard.phase_state),
                            "n1_phase": format!("{:?}", n1.phase_state),
                            "n1_beta": n1.beta,
                            "margin_seed": stability.margin_seed.is_some(),
                            "trials": trial_records}));
                    }
                    let x1_cell = x1.cell(cell);
                    let z1 = x1_cell.overall_composition();
                    let p1 = bar_to_pa(x1_cell.pressure_bar);
                    let (candidate, candidate_beta, candidate_sum, candidate_dg) =
                        if let Some(split) = split_x0.as_ref() {
                            let ln_k: Vec<f64> = split.k.iter().map(|value| crate::math::ln(*value)).collect();
                            let beta = solve_rachford_rice_extended(&split.k, &z1).ok();
                            beta.map_or((false, f64::NAN, f64::NAN, None), |beta| {
                                let seeded = seeded_two_phase_for_test(
                                    problem.spec,
                                    p1,
                                    problem.spec.reservoir_temperature_k(),
                                    &z1,
                                    &ln_k,
                                    beta,
                                );
                                let dg = seeded.state.as_ref().and_then(|state| {
                                    two_phase_gibbs_minus_feed_for_test(
                                        problem.spec,
                                        p1,
                                        problem.spec.reservoir_temperature_k(),
                                        &z1,
                                        state,
                                    )
                                });
                                (
                                    seeded.state.is_some() && seeded.residual_norm < 1e-12,
                                    seeded.beta,
                                    seeded.sum_ln_k_sq,
                                    dg,
                                )
                            })
                        } else {
                            (false, f64::NAN, f64::NAN, None)
                        };
                    let standard = assemble_for_test(problem, &x1, bhp).expect("standard assembly");
                    let forced = split_x0.clone().map(|split| {
                        vec![(
                            p1,
                            problem.spec.reservoir_temperature_k(),
                            z1.clone(),
                            split,
                        )]
                    });
                    let jump = forced.as_ref().map(|forced| {
                        let alt = with_forced_states_for_test(forced, || {
                            assemble_for_test(problem, &x1, bhp).expect("forced assembly")
                        });
                        scaled_cell_jump(&alt, &standard, cell)
                    });
                    let g_feed = feed_gibbs_on_branches_for_test(
                        problem.spec,
                        p1,
                        problem.spec.reservoir_temperature_k(),
                        &z1,
                    );
                    records.push(serde_json::json!({"cell": cell, "bracket": [lo, hi],
                        "points": points, "carried_seed": candidate,
                        "carried_beta": candidate_beta, "carried_sum_ln_k_sq": candidate_sum,
                        "carried_gibbs_minus_feed": candidate_dg, "feed_gibbs": g_feed,
                        "scaled_residual_jump": jump}));
                }
                serde_json::json!({"dt_days": dt, "residual_history": attempt.residual_history,
                    "switches": records})
            })
        })
    });
    if let Ok(dir) = std::env::var("SPE5_TRACE_OUT") {
        let path = PathBuf::from(dir).join("n1_next_call_switch.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&out).expect("serialize"),
        )
        .expect("write");
        println!("written {}", path.display());
    }
    let switches = out["switches"].as_array().expect("switches");
    assert_eq!(switches.len(), 2);
    for switch in switches {
        let points = switch["points"].as_array().expect("points");
        let lower = &points[0];
        let upper = &points[1];
        assert_eq!(lower["margin_seed"], Value::Bool(true));
        assert_eq!(upper["margin_seed"], Value::Bool(false));
        assert_eq!(upper["n1_phase"], "SingleLiquid");
        assert_eq!(switch["carried_seed"], Value::Bool(true));
        let sum = switch["carried_sum_ln_k_sq"].as_f64().expect("sum");
        assert!((1.6e-5..1.7e-5).contains(&sum), "carried split sum {sum:e}");
        let jump = switch["scaled_residual_jump"].as_f64().expect("jump");
        assert!(jump > 3e-3, "branch jump {jump:e}");
        let bracket = switch["bracket"].as_array().expect("bracket");
        assert!(bracket[1].as_f64().unwrap() > bracket[0].as_f64().unwrap());
    }
    println!("{}", serde_json::to_string_pretty(&out).expect("serialize"));
}

#[test]
#[ignore = "#116 carried-K seed recovery on the captured failing call; seconds"]
fn comp_spe5_seed_recovery_resolves_the_switch() {
    use super::spe5_failure_b_tests::run;
    use crate::fluid::flash::{
        with_difference_refinement_for_test, with_seed_recovery_for_test,
        with_split_phase_decision_for_test,
    };
    let case = restore("stall_n1_6612_836.json");
    let dt = 0.04402126141911289;
    let (control, recovery) = case.with_diagnostic_problem(dt, |problem, start, options| {
        let control = with_difference_refinement_for_test(true, || {
            with_split_phase_decision_for_test(true, || {
                with_seed_recovery_for_test(false, || run(problem, start, options))
            })
        });
        let recovery = with_difference_refinement_for_test(true, || {
            with_split_phase_decision_for_test(true, || {
                with_seed_recovery_for_test(true, || run(problem, start, options))
            })
        });
        (control.residual_history, recovery.residual_history)
    });
    for (label, history) in [("N1 + refinement", &control), ("with carried K", &recovery)] {
        let best = history.iter().copied().fold(f64::INFINITY, f64::min);
        println!(
            "{label}: {} iterations, best {best:.6e}, last {:.6e}",
            history.len(),
            history.last().copied().unwrap_or(f64::NAN)
        );
        assert_eq!(history.len(), 20, "{label} must exhaust the Newton cap");
        assert!(best > 1e-8, "{label} must still stall, got {best:e}");
    }
    // The full step jumps from ~227.55 to ~228.16 bar; the switch at ~228.126 is never evaluated,
    // so carrying K from the previous iterate alone cannot recover it. Both runs stall.
    assert!(recovery.last().copied().unwrap_or(f64::NAN) > 1e-8);
}

#[test]
#[ignore = "#116 carried-K recovery at the switch inputs; seconds"]
fn comp_spe5_seed_recovery_carries_the_switch() {
    use crate::fluid::flash::{
        flash_for_cell, remember_k_for_test, with_difference_refinement_for_test,
        with_seed_recovery_for_test, with_split_phase_decision_for_test,
    };
    use crate::fluid::units::bar_to_pa;
    let case = restore("stall_n1_6612_836.json");
    let spec = case.spec_for_test();
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../opm/compositional/spe5/n1_switch_6612_cell33.json"),
        )
        .expect("read switch fixture"),
    )
    .expect("parse switch fixture");
    let point = &fixture["points"][0];
    let z: Vec<f64> = point["z"]
        .as_array()
        .expect("z")
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    let upper_pa = point["p_pa"].as_f64().expect("p");
    let lower_pa = f64::from_bits(upper_pa.to_bits() - 1);
    let temperature = point["temperature_k"].as_f64().expect("T");
    let run = |recovery: bool| {
        with_difference_refinement_for_test(true, || {
            with_split_phase_decision_for_test(true, || {
                with_seed_recovery_for_test(recovery, || {
                    let lower = flash_for_cell(0, spec, lower_pa, temperature, &z, None)
                        .expect("lower flash");
                    remember_k_for_test(0, &lower);
                    let upper = flash_for_cell(0, spec, upper_pa, temperature, &z, None)
                        .expect("upper flash");
                    (lower, upper)
                })
            })
        })
    };
    let (control_lower, control_upper) = run(false);
    let (recovery_lower, recovery_upper) = run(true);
    println!(
        "control: lower {:?} beta {:.6}, upper {:?} beta {:.6}",
        control_lower.phase_state,
        control_lower.beta,
        control_upper.phase_state,
        control_upper.beta
    );
    println!(
        "recovery: lower {:?} beta {:.6}, upper {:?} beta {:.6}",
        recovery_lower.phase_state,
        recovery_lower.beta,
        recovery_upper.phase_state,
        recovery_upper.beta
    );
    assert_eq!(
        recovery_lower.phase_state,
        crate::fluid::flash::PhaseState::TwoPhase
    );
    assert_eq!(
        recovery_upper.phase_state,
        crate::fluid::flash::PhaseState::TwoPhase
    );
    assert_eq!(
        control_upper.phase_state,
        crate::fluid::flash::PhaseState::SingleLiquid
    );
    let sum: f64 = recovery_upper
        .k
        .iter()
        .map(|value| {
            let ln = crate::math::ln(*value);
            ln * ln
        })
        .sum();
    assert!(
        sum > 1e-5,
        "carried split must pass the trivial cutoff, got {sum:e}"
    );
    let _ = bar_to_pa(0.0);
}

#[test]
#[ignore = "#116 carried-K repeated-flash and reset isolation; seconds"]
fn comp_spe5_seed_recovery_repeated_flash_and_reset() {
    use crate::fluid::flash::{
        PhaseState, flash_for_cell, remember_k_for_test, reset_carried_k_for_test,
        with_difference_refinement_for_test, with_seed_recovery_for_test,
        with_split_phase_decision_for_test,
    };
    let case = restore("stall_n1_6612_836.json");
    let spec = case.spec_for_test();
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../opm/compositional/spe5/n1_switch_6612_cell33.json"),
        )
        .expect("read switch fixture"),
    )
    .expect("parse switch fixture");
    let point = &fixture["points"][0];
    let z: Vec<f64> = point["z"]
        .as_array()
        .expect("z")
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    let upper_pa = point["p_pa"].as_f64().expect("p");
    let lower_pa = f64::from_bits(upper_pa.to_bits() - 1);
    let temperature = point["temperature_k"].as_f64().expect("T");
    with_difference_refinement_for_test(true, || {
        with_split_phase_decision_for_test(true, || {
            with_seed_recovery_for_test(true, || {
                reset_carried_k_for_test();
                let lower =
                    flash_for_cell(0, spec, lower_pa, temperature, &z, None).expect("lower flash");
                assert_eq!(lower.phase_state, PhaseState::TwoPhase);
                remember_k_for_test(0, &lower);
                let first = flash_for_cell(0, spec, upper_pa, temperature, &z, None)
                    .expect("first upper flash");
                let repeated = flash_for_cell(0, spec, upper_pa, temperature, &z, None)
                    .expect("repeated upper flash");
                assert_eq!(first.phase_state, PhaseState::TwoPhase);
                assert_eq!(repeated.phase_state, PhaseState::TwoPhase);
                assert_eq!(first.beta, repeated.beta);
                assert_eq!(first.k, repeated.k);
                // A reset starts cold: without the lower-side carry the upper input is liquid.
                reset_carried_k_for_test();
                let cold = flash_for_cell(0, spec, upper_pa, temperature, &z, None)
                    .expect("cold upper flash");
                assert_eq!(cold.phase_state, PhaseState::SingleLiquid);
            })
        })
    });
}

#[test]
#[ignore = "#116 whole-assembly residual consistency across seed commits; seconds"]
fn comp_spe5_seed_recovery_assembly_is_stable_across_commits() {
    use super::newton::assemble_for_test;
    use crate::fluid::flash::{
        flash_for_cell, remember_k_for_test, with_difference_refinement_for_test,
        with_seed_recovery_for_test, with_split_phase_decision_for_test,
    };
    let case = restore("stall_n1_6612_836.json");
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../opm/compositional/spe5/n1_switch_6612_cell33.json"),
        )
        .expect("read switch fixture"),
    )
    .expect("parse switch fixture");
    let point = &fixture["points"][0];
    let z: Vec<f64> = point["z"]
        .as_array()
        .expect("z")
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    let upper_pa = point["p_pa"].as_f64().expect("p");
    let lower_pa = f64::from_bits(upper_pa.to_bits() - 1);
    let temperature = point["temperature_k"].as_f64().expect("T");
    let dt = 0.04402126141911289;
    let norms = with_difference_refinement_for_test(true, || {
        with_split_phase_decision_for_test(true, || {
            with_seed_recovery_for_test(true, || {
                case.with_diagnostic_problem(dt, |problem, start, _options| {
                    let spec = problem.spec;
                    let lower = flash_for_cell(33, spec, lower_pa, temperature, &z, None)
                        .expect("lower flash");
                    assert_eq!(lower.phase_state, crate::fluid::flash::PhaseState::TwoPhase);
                    remember_k_for_test(33, &lower);
                    let mut trial = start.begin_trial();
                    {
                        let target = trial.cell_mut(33);
                        target.pressure_bar = upper_pa / 1e5;
                        let independent = target.independent_z_mut();
                        independent.copy_from_slice(&z[..independent.len()]);
                    }
                    let trial = trial.commit().expect("upper state");
                    let bhp = problem.accepted_well_bhp.expect("accepted BHP");
                    let mut norms = Vec::new();
                    for _ in 0..3 {
                        // The attempt-owned hint is fixed; assembling must not publish a new one.
                        let assembly = assemble_for_test(problem, &trial, bhp).expect("assembly");
                        norms.push(assembly.scaled_residual_norm(problem.layout));
                    }
                    let cold = with_seed_recovery_for_test(false, || {
                        assemble_for_test(problem, &trial, bhp).expect("cold assembly")
                    });
                    norms.push(cold.scaled_residual_norm(problem.layout));
                    norms
                })
            })
        })
    });
    println!("assembly norms: {norms:?}");
    for pair in norms[..3].windows(2) {
        assert!(
            (pair[0] - pair[1]).abs() <= 1e-8,
            "repeated assembly changed the residual: {:?}",
            pair
        );
    }
    assert!(
        (norms[2] - norms[3]).abs() > 1e-8,
        "the cold re-evaluation should be a negative control, got {:?}",
        &norms[2..]
    );
}

#[test]
#[ignore = "#116 carried-K attempt and restart lifecycle; seconds"]
fn comp_spe5_seed_recovery_attempt_and_restart_lifecycle() {
    use super::newton::solve_newton_observed;
    use crate::fluid::flash::{
        reset_carried_k_for_test, with_difference_refinement_for_test, with_seed_recovery_for_test,
        with_split_phase_decision_for_test,
    };
    let case = restore("stall_n1_6612_836.json");
    let dt = 0.04402126141911289;
    // Two consecutive solves from the same accepted state must reproduce the same history even
    // though the first leaves a committed hint behind.
    let (first, second) = case.with_diagnostic_problem(dt, |problem, start, options| {
        let first = with_difference_refinement_for_test(true, || {
            with_split_phase_decision_for_test(true, || {
                with_seed_recovery_for_test(true, || {
                    solve_newton_observed(problem, start, options, None)
                        .1
                        .residual_history
                })
            })
        });
        let second = with_difference_refinement_for_test(true, || {
            with_split_phase_decision_for_test(true, || {
                with_seed_recovery_for_test(true, || {
                    solve_newton_observed(problem, start, options, None)
                        .1
                        .residual_history
                })
            })
        });
        (first, second)
    });
    assert_eq!(
        first, second,
        "a retry must not inherit the previous attempt's hint"
    );

    // A checkpoint restart reproduces the accepted inventory with an empty cache.
    with_difference_refinement_for_test(true, || {
        with_split_phase_decision_for_test(true, || {
            with_seed_recovery_for_test(true, || {
                reset_carried_k_for_test();
                let cold = case.component_inventory_for_test();
                let restored = super::api::CompositionalCase::restore(&case.checkpoint())
                    .expect("restore checkpoint");
                reset_carried_k_for_test();
                let restarted = restored.component_inventory_for_test();
                assert_eq!(cold, restarted, "checkpoint restart changed the inventory");
            })
        })
    });
}

#[test]
fn comp_spe5_seed_recovery_cold_acceptance_requires_cold_convergence() {
    use super::newton::cold_acceptance_ok;
    let tolerance = 1e-8;
    // The review's case: warm 8e-9, cold 1.5e-8. The old max-norm difference (7e-9) passed,
    // but cold convergence must be independent.
    assert!(!cold_acceptance_ok(8e-9, 1.5e-8, 7e-9, tolerance));
    // A cold-converged state with an agreeing scaled residual vector is accepted.
    assert!(cold_acceptance_ok(8e-9, 9e-9, 7e-9, tolerance));
    // A vector difference above tolerance is refused even when both norms agree.
    assert!(!cold_acceptance_ok(8e-9, 8.5e-9, 1.1e-8, tolerance));
    // Warm non-convergence is never accepted.
    assert!(!cold_acceptance_ok(1.1e-8, 8e-9, 0.0, tolerance));
}

#[test]
#[ignore = "#116 well-control change through the cold comparison; seconds"]
fn comp_spe5_seed_recovery_well_control_change_is_rejected() {
    use super::api::{CompletionConfig, CompositionalCase, WellConfig, WellControlConfig};
    use super::newton::{assemble_for_test, scaled_residual_vector_difference};
    use crate::fluid::flash::{
        flash_for_cell, remember_k_for_test, with_difference_refinement_for_test,
        with_seed_recovery_for_test, with_split_phase_decision_for_test,
    };
    let base = restore("stall_n1_6612_836.json");
    let mut checkpoint = base.checkpoint();
    checkpoint.config.wells.push(WellConfig {
        id: "PROBE33".to_string(),
        completions: vec![CompletionConfig {
            cell: 33,
            well_index: 1.0,
            head_offset_bar: 0.0,
        }],
        control: WellControlConfig::MolarRate {
            target_moles_per_day: -1_170_167.30,
            bhp_limit_bar: 200.0,
        },
        injection_composition: None,
        inject_water: false,
    });
    checkpoint.well_bhp_bar.push(210.0);
    // The synthetic probe is a new well; preserve the existing wells' history and append zeros.
    if let Some(totals) = checkpoint.cumulative_well_moles.as_mut() {
        totals.push(vec![0.0; base.snapshot().component_ids.len() + 1]);
    }
    if let Some(rates) = checkpoint.well_surface_rates.as_mut() {
        rates.push(super::wells::WellSurfaceRates::zero(true));
    }
    for totals in [
        &mut checkpoint.cumulative_well_surface_volumes,
        &mut checkpoint.cumulative_well_surface_production_volumes,
    ]
    .into_iter()
    .flatten()
    {
        totals.push(super::wells::WellSurfaceVolumes::zero(true));
    }
    let case = CompositionalCase::restore(&checkpoint).expect("restore probe case");
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../opm/compositional/spe5/n1_switch_6612_cell33.json"),
        )
        .expect("read switch fixture"),
    )
    .expect("parse switch fixture");
    let point = &fixture["points"][0];
    let z: Vec<f64> = point["z"]
        .as_array()
        .expect("z")
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    let upper_pa = point["p_pa"].as_f64().expect("p");
    let lower_pa = f64::from_bits(upper_pa.to_bits() - 1);
    let temperature = point["temperature_k"].as_f64().expect("T");
    let dt = 0.04402126141911289;
    let result = with_difference_refinement_for_test(true, || {
        with_split_phase_decision_for_test(true, || {
            with_seed_recovery_for_test(true, || {
                case.with_diagnostic_problem(dt, |problem, start, _options| {
                    let lower = flash_for_cell(33, problem.spec, lower_pa, temperature, &z, None)
                        .expect("lower flash");
                    remember_k_for_test(33, &lower);
                    let mut trial = start.begin_trial();
                    {
                        let target = trial.cell_mut(33);
                        target.pressure_bar = upper_pa / 1e5;
                        let independent = target.independent_z_mut();
                        independent.copy_from_slice(&z[..independent.len()]);
                    }
                    let trial = trial.commit().expect("upper state");
                    let bhp = problem.accepted_well_bhp.expect("accepted BHP").to_vec();
                    let warm = assemble_for_test(problem, &trial, &bhp).expect("warm assembly");
                    let cold = with_seed_recovery_for_test(false, || {
                        assemble_for_test(problem, &trial, &bhp).expect("cold assembly")
                    });
                    println!(
                        "warm limits {:?}, cold limits {:?}; warm eliminations {}, cold {}",
                        warm.well_on_bhp_limit,
                        cold.well_on_bhp_limit,
                        warm.well_eliminations.len(),
                        cold.well_eliminations.len()
                    );
                    scaled_residual_vector_difference(&warm, &cold, problem.layout)
                })
            })
        })
    });
    assert!(
        result.is_none(),
        "a changed active well-rate set must not be paired by position"
    );
}

#[test]
#[ignore = "#116 line search with and without carried K; about a minute"]
fn comp_spe5_line_search_recovery_comparison() {
    use super::newton::{NewtonError, solve_newton, with_line_search_for_test};
    use crate::fluid::flash::{
        with_difference_refinement_for_test, with_seed_recovery_for_test,
        with_split_phase_decision_for_test,
    };
    let case = restore("stall_n1_6612_836.json");
    let dt = 0.04402126141911289;
    let rows = with_difference_refinement_for_test(true, || {
        with_split_phase_decision_for_test(true, || {
            case.with_diagnostic_problem(dt, |problem, start, options| {
                assert_eq!(
                    options.tolerance, 1e-8,
                    "the acceptance tolerance must not move"
                );
                let mut rows = Vec::new();
                for line_search in [false, true] {
                    for recovery in [false, true] {
                        let (outcome, report) = with_line_search_for_test(line_search, || {
                            with_seed_recovery_for_test(recovery, || {
                                solve_newton(problem, start, options)
                            })
                        });
                        let accepted = outcome.is_ok();
                        let failure_residual =
                            outcome.as_ref().err().and_then(|error| match error {
                                NewtonError::NotConverged {
                                    scaled_residual, ..
                                } => Some(*scaled_residual),
                                _ => None,
                            });
                        let best = report
                            .residual_history
                            .iter()
                            .copied()
                            .fold(f64::INFINITY, f64::min);
                        let last = report.residual_history.last().copied().unwrap_or(f64::NAN);
                        println!(
                            "line search {:<5} recovery {:<5}: accepted {accepted:<5} \
                             iterations {:>2}, best {best:.6e}, last {last:.6e}, \
                             failure residual {:?}, step scales {:?}",
                            if line_search { "on" } else { "off" },
                            if recovery { "on" } else { "off" },
                            report.iterations,
                            failure_residual,
                            report.step_scale_history,
                        );
                        for (iteration, trials) in report.line_search_history.iter().enumerate() {
                            if trials.is_empty() {
                                continue;
                            }
                            let evaluated: Vec<String> = trials
                                .iter()
                                .map(|trial| {
                                    let changes = if trial.phase_changes.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" changes {:?}", trial.phase_changes)
                                    };
                                    match trial.scaled_residual {
                                        Some(norm) => format!(
                                            "tau {:.3e} -> {:.6e}{}{}",
                                            trial.tau,
                                            norm,
                                            if trial.accepted { "*" } else { "" },
                                            changes
                                        ),
                                        None => format!(
                                            "tau {:.3e} -> assembly failed{}",
                                            trial.tau, changes
                                        ),
                                    }
                                })
                                .collect();
                            println!(
                                "  iteration {}: tau/norm {}",
                                iteration + 1,
                                evaluated.join(" ")
                            );
                        }
                        rows.push(serde_json::json!({
                            "line_search": line_search,
                            "recovery": recovery,
                            "accepted": accepted,
                            "failure_residual": failure_residual,
                            "iterations": report.iterations,
                            "best": best,
                            "last": last,
                            "step_scales": report.step_scale_history,
                            "line_search_history": report
                                .line_search_history
                                .iter()
                                .map(|trials| {
                                    trials
                                        .iter()
                                        .map(|trial| {
                                            serde_json::json!({
                                                "tau": trial.tau,
                                                "scaled_residual": trial.scaled_residual,
                                                "phase_changes": trial.phase_changes,
                                                "accepted": trial.accepted,
                                            })
                                        })
                                        .collect::<Vec<_>>()
                                })
                                .collect::<Vec<_>>(),
                        }));
                    }
                }
                rows
            })
        })
    });
    assert_eq!(rows.len(), 4);
    let row = |line_search: bool, recovery: bool| {
        rows.iter()
            .find(|row| {
                row["line_search"].as_bool() == Some(line_search)
                    && row["recovery"].as_bool() == Some(recovery)
            })
            .expect("mode row")
    };
    let off_off = row(false, false);
    let off_on = row(false, true);
    let on_off = row(true, false);
    let on_on = row(true, true);

    // The two controls without a line search are the committed #134 result: neither reaches the
    // 1e-8 test and carrying K changes nothing.
    for control in [off_off, off_on] {
        assert!(!control["accepted"].as_bool().expect("accepted flag"));
        assert_eq!(control["iterations"].as_u64(), Some(20));
        assert!(control["best"].as_f64().expect("best") > 1e-3);
    }
    assert_eq!(off_off["best"], off_on["best"]);

    // With a monotone line search both modes make many accepted Newton steps, then hit a state
    // where no backtracked trial lowers the residual. Neither reaches 1e-8.
    for line_searched in [on_off, on_on] {
        assert!(!line_searched["accepted"].as_bool().expect("accepted flag"));
        let best = line_searched["best"].as_f64().expect("best");
        assert!((1e-7..5e-6).contains(&best), "line-search floor {best:e}");
        assert!(line_searched["iterations"].as_u64().expect("iterations") < 20);
    }
    // Carrying K lowers the barrier and keeps the line search decreasing for more steps, but it
    // does not turn the barrier into an accepted timestep.
    assert!(
        on_on["best"].as_f64().expect("best") < on_off["best"].as_f64().expect("best"),
        "carried K must lower the line-search floor"
    );
    assert!(
        on_on["iterations"].as_u64().expect("iterations")
            > on_off["iterations"].as_u64().expect("iterations")
    );
    for line_searched in [on_off, on_on] {
        let last = line_searched["last"].as_f64().expect("last residual");
        assert_eq!(
            line_searched["failure_residual"].as_f64(),
            Some(last),
            "a failed line search must report the current iterate's residual"
        );
        let history = line_searched["line_search_history"]
            .as_array()
            .expect("line-search history");
        assert!(
            history
                .iter()
                .any(|trials| trials.as_array().is_some_and(|t| t.len() > 1)),
            "the line search must have backtracked at least once"
        );
        let mut crossed = false;
        for trials in history {
            for trial in trials.as_array().expect("trials") {
                let changes = trial["phase_changes"].as_array().expect("phase changes");
                let has = |cell: u64| changes.iter().any(|value| value.as_u64() == Some(cell));
                if has(33) && has(39) {
                    crossed = true;
                }
            }
        }
        assert!(
            crossed,
            "the early full steps must cross cells 33/39: {history:?}"
        );
        let last_trials = history
            .last()
            .expect("a final line search")
            .as_array()
            .expect("trials");
        assert!(
            last_trials
                .iter()
                .all(|trial| trial["phase_changes"].as_array().is_some_and(Vec::is_empty)),
            "the final failed iteration must not change a phase label: {last_trials:?}"
        );
        for (index, trials) in history.iter().enumerate() {
            let trials = trials.as_array().expect("trials");
            assert!(!trials.is_empty());
            let last = trials.last().expect("trial");
            if index + 1 == history.len() {
                assert_eq!(
                    last["accepted"],
                    Value::Bool(false),
                    "the last line search failed"
                );
            } else {
                assert_eq!(last["accepted"], Value::Bool(true));
            }
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&rows).expect("serialize")
    );
}
