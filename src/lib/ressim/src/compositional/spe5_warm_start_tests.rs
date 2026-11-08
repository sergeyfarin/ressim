//! #116: BHP warm start as a separate experiment, with acceptance unchanged at `1e-8`.
//!
//! [`WellBhpStart::LastAccepted`] starts a rate-controlled well's BHP at its last accepted value
//! instead of the control's guess. Signature A's diagnosis suggested it: on the `stall_218.json`
//! seed at 2.44e-6 d every cell was at 3.3e-9 by the second iterate, but the water injector's row
//! started from a guess (initial residual 3.38) and its update pushed cell 33 across the phase
//! switch. This runs, with the option off and on:
//!
//! - **A:** the seed's timestep ladder (0.02 d, then 1.95e-5 down to 2.44e-6 d), one Newton attempt
//!   each;
//! - **B:** the from-scratch failing call from `stall_218_from_scratch.json` (22.67 d remainder,
//!   ten halvings), through the real timestep loop.
//!
//! Nothing here changes acceptance; the option is off by default and these tests pin no threshold.
use super::api::{Checkpoint, CompositionalCase};
use super::newton::{WellBhpStart, solve_newton_observed};
use serde_json::{Value, json};
use std::path::PathBuf;

fn seed(name: &str) -> CompositionalCase {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../opm/compositional/spe5")
        .join(name);
    let text = std::fs::read_to_string(path).expect("read the seed");
    let checkpoint: Checkpoint = serde_json::from_str(&text).expect("parse the seed");
    CompositionalCase::restore(&checkpoint).expect("restore the seed")
}

fn attempt(case: &CompositionalCase, dt: f64, start: WellBhpStart) -> Value {
    case.with_diagnostic_problem(dt, |problem, state, mut options| {
        options.well_bhp_start = start;
        let (outcome, report) = solve_newton_observed(problem, state, options, None);
        json!({
            "dt_days": dt,
            "converged": outcome.is_ok(),
            "iterations": report.iterations,
            "first_residual": report.residual_history.first(),
            "last_residual": report.residual_history.last(),
            "best_residual": report.residual_history.iter().copied().fold(f64::INFINITY, f64::min),
        })
    })
}

#[test]
#[ignore = "#116 BHP warm-start experiment; ~2 min; run with --release --ignored --nocapture"]
fn comp_spe5_bhp_warm_start_experiment() {
    let mut out = serde_json::Map::new();
    for (label, start) in [
        ("control_guess", WellBhpStart::ControlGuess),
        ("last_accepted", WellBhpStart::LastAccepted),
    ] {
        // A: one attempt per rung of the seed's ladder.
        let a = seed("stall_218.json");
        let ladder = [0.02, 1.953125e-5, 9.765625e-6, 4.8828125e-6, 2.44140625e-6];
        let a_rows: Vec<Value> = ladder.iter().map(|&dt| attempt(&a, dt, start)).collect();
        for r in &a_rows {
            println!(
                "{label:<14} A dt {:.3e}: converged {} in {} it, first {:.2e}, best {:.2e}",
                r["dt_days"].as_f64().unwrap_or(f64::NAN),
                r["converged"],
                r["iterations"],
                r["first_residual"].as_f64().unwrap_or(f64::NAN),
                r["best_residual"].as_f64().unwrap_or(f64::NAN),
            );
        }

        // B: the real failing call, through the timestep loop.
        let mut b = seed("stall_218_from_scratch.json");
        b.timestep_options_mut_for_test().newton.well_bhp_start = start;
        let outcome = b.step(22.671554013151308);
        println!(
            "{label:<14} B call: succeeded {}, {} attempts, accepted dt {:?}, {:?}",
            outcome.succeeded(),
            outcome.attempts,
            outcome.accepted_dt_days,
            outcome.failure
        );
        let b_last = attempt(
            &seed("stall_218_from_scratch.json"),
            22.671554013151308 * 0.5f64.powi(9),
            start,
        );
        println!(
            "{label:<14} B last attempt: converged {} in {} it, best {:.2e}",
            b_last["converged"],
            b_last["iterations"],
            b_last["best_residual"].as_f64().unwrap_or(f64::NAN),
        );
        out.insert(
            label.to_string(),
            json!({
                "a_ladder": a_rows,
                "b_call": {
                    "succeeded": outcome.succeeded(),
                    "attempts": outcome.attempts,
                    "accepted_dt_days": outcome.accepted_dt_days,
                    "failure": format!("{:?}", outcome.failure),
                },
                "b_last_attempt": b_last,
            }),
        );
    }
    if let Ok(dir) = std::env::var("SPE5_TRACE_OUT") {
        let path = PathBuf::from(dir).join("bhp_warm_start_experiment.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&out).expect("serialize"),
        )
        .expect("write");
    }
    assert_eq!(out.len(), 2);
}
