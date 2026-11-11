//! #116: SPE5 replays under experimental timestep settings. **Acceptance is unchanged (`1e-8`).**
//!
//! One driver serves two measurements:
//!
//! - **Retry budget.** `max_attempts` is the timestep loop's halving budget (default 10). Seeded
//!   from failure B, this measures whether more halvings carry the run through the crossing, and
//!   at what cost.
//! - **ResSim's own timestep convergence.** An internal cap on each call's `dt`, with monthly reports
//!   and WAG event times kept (the schedule already cuts a step at an event), to compare with the
//!   JutulDarcy runs at the same caps.
//!
//! Settings come from the environment so one ignored test covers every run:
//!
//! | Variable | Meaning | Default |
//! |---|---|---|
//! | `SPE5_REPLAY_OUT` | output JSON, rewritten after every month | required |
//! | `SPE5_REPLAY_SEED` | `scratch`, a checkpoint file in `opm/compositional/spe5/`, or an absolute path | `scratch` |
//! | `SPE5_MAX_DT_DAYS` | cap on each call's `dt` | none (the month's remainder) |
//! | `SPE5_MAX_ATTEMPTS` | the halving budget | 10 |
//! | `SPE5_LAST_MONTH` | last report month | 240 |
//! | `SPE5_WALL_BUDGET_S` | stop, recording why, once this much wall time is spent | none |
//! | `SPE5_DT_MEMORY_GROWTH` | bounded timestep memory: after an accepted step `dt`, the next request is at most this factor times `dt` (a driver policy; the engine's own halving is unchanged) | none |
//! | `SPE5_FLASH_REFINE` | `1`: the experimental difference-form flash refinement on (`fluid::flash`) | off |
//! | `SPE5_N1` | `1`: the experimental split-based phase decision (N1) on (`fluid::flash`) | off |
//! | `SPE5_SEED_RECOVERY` | `1`: carry each cell's previous flash `K` as a test-only split seed | off |
//! | `SPE5_STEP_DAMPING` | extra Newton step multiplier for the test-only damping probe | 1 |
//! | `SPE5_REPLAY_CHECKPOINT_OUT` | on a failed step, write the failing call's starting checkpoint here (a seed for `SPE5_REPLAY_SEED`) | none |
//!
//! Per month it records the snapshot (the format `tools/jutul/spe5_compare.py` reads), the calls,
//! accepted steps, failed attempts, Newton iterations of accepted attempts, the smallest accepted
//! `dt`, wall time, and the signed global component balance since the run's start:
//! `(N(t) - N(t0)) - (C(t) - C(t0))` over `N(t0)`, with `N` the inventory and `C` the wells'
//! cumulative moles (positive into the reservoir). A failed attempt's Newton iterations are not
//! exposed by `StepOutcome`; `newton_upper_bound` counts each failed attempt at the Newton cap.
//! A run that stops records the unfinished month's counts in `stopped`.
//!
//! Each call requests `end - now` (or the cap), recomputed from the snapshot time. That can differ
//! in the last bits from a loop that subtracts accepted steps, and at failure B's near-critical
//! state an attempt's outcome changes with that difference (#116).
use super::api::{Checkpoint, CompositionalCase};
use super::newton::with_step_damping_for_test;
use crate::fluid::flash::{
    reset_split_phase_decision_acceptances_for_test, split_phase_decision_acceptances_for_test,
    with_difference_refinement_for_test, with_seed_recovery_for_test,
    with_split_phase_decision_for_test,
};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Instant;

const MONTH_DAYS: f64 = 30.4375;

fn spe5_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../opm/compositional/spe5")
}

fn env_or<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

fn balance_since(case: &CompositionalCase, n0: &[f64], c0: &[f64]) -> Vec<f64> {
    let n = case.component_inventory_for_test();
    let c = well_cumulative(case);
    (0..n.len())
        .map(|i| ((n[i] - n0[i]) - (c[i] - c0[i])) / n0[i])
        .collect()
}

fn well_cumulative(case: &CompositionalCase) -> Vec<f64> {
    let snapshot = case.snapshot();
    let width = case.component_inventory_for_test().len();
    let mut total = vec![0.0; width];
    for well in &snapshot.cumulative_well_moles {
        for (t, v) in total.iter_mut().zip(well) {
            *t += v;
        }
    }
    total
}

#[test]
#[ignore = "#116 timestep-control replay; minutes to hours; configure through SPE5_* (module docs)"]
fn comp_spe5_replay_with_timestep_options() {
    let refine = std::env::var("SPE5_FLASH_REFINE").is_ok_and(|v| v == "1");
    let n1 = std::env::var("SPE5_N1").is_ok_and(|v| v == "1");
    let recovery = std::env::var("SPE5_SEED_RECOVERY").is_ok_and(|v| v == "1");
    let damping: f64 = env_or("SPE5_STEP_DAMPING").unwrap_or(1.0);
    reset_split_phase_decision_acceptances_for_test();
    with_difference_refinement_for_test(refine, || {
        with_split_phase_decision_for_test(n1, || {
            with_seed_recovery_for_test(recovery, || {
                with_step_damping_for_test(damping, || replay(refine, n1, recovery, damping))
            })
        })
    });
}

fn replay(refine: bool, n1: bool, recovery: bool, damping: f64) {
    let out = PathBuf::from(std::env::var("SPE5_REPLAY_OUT").expect("set SPE5_REPLAY_OUT"));
    let seed = std::env::var("SPE5_REPLAY_SEED").unwrap_or_else(|_| "scratch".to_string());
    let cap: Option<f64> = env_or("SPE5_MAX_DT_DAYS");
    let max_attempts: usize = env_or("SPE5_MAX_ATTEMPTS").unwrap_or(10);
    let last_month: usize = env_or("SPE5_LAST_MONTH").unwrap_or(240);
    let wall_budget: Option<f64> = env_or("SPE5_WALL_BUDGET_S");
    let growth: Option<f64> = env_or("SPE5_DT_MEMORY_GROWTH");
    let mut remembered: Option<f64> = None;
    let checkpoint_out: Option<PathBuf> = std::env::var("SPE5_REPLAY_CHECKPOINT_OUT")
        .ok()
        .map(PathBuf::from);

    let mut case = if seed == "scratch" {
        let text = std::fs::read_to_string(spe5_dir().join("case.json")).expect("read case.json");
        CompositionalCase::new(serde_json::from_str(&text).expect("parse")).expect("admissible")
    } else {
        let text = std::fs::read_to_string(spe5_dir().join(&seed)).expect("read the seed");
        let checkpoint: Checkpoint = serde_json::from_str(&text).expect("parse the seed");
        CompositionalCase::restore(&checkpoint).expect("restore the seed")
    };
    case.timestep_options_mut_for_test().max_attempts = max_attempts;

    let start_time = case.snapshot().time_days;
    let first_month = (start_time / MONTH_DAYS).floor() as usize + 1;
    let n0 = case.component_inventory_for_test();
    let c0 = well_cumulative(&case);
    let clock = Instant::now();
    let mut months: Vec<Value> = Vec::new();
    let mut stopped: Option<Value> = None;
    let settings = json!({
        "seed": seed, "start_day": start_time, "max_dt_days": cap,
        "max_attempts": max_attempts, "last_month": last_month, "wall_budget_s": wall_budget,
        "tolerance": 1e-8, "flash_refinement": refine, "n1": n1,
        "seed_recovery": recovery, "step_damping": damping, "dt_memory_growth": growth,
        "request": "end - now from the snapshot time, or the cap",
    });
    let write = |months: &[Value], stopped: &Option<Value>| {
        let body = json!({"schema": "ressim-spe5-replay/1", "settings": settings,
            "stopped": stopped, "months": months});
        std::fs::write(&out, serde_json::to_string(&body).expect("serialize")).expect("write");
    };

    'months: for month in first_month..=last_month {
        let month_start = Instant::now();
        let end = month as f64 * MONTH_DAYS;
        let (mut calls, mut steps, mut failed_attempts, mut newton) = (0usize, 0usize, 0usize, 0);
        let mut min_dt = f64::INFINITY;
        loop {
            let now = case.snapshot().time_days;
            let remaining = end - now;
            if remaining <= 1e-9 {
                break;
            }
            if let Some(budget) = wall_budget
                && clock.elapsed().as_secs_f64() > budget
            {
                stopped = Some(json!({"reason": "wall budget", "month": month, "day": now,
                    "month_so_far": {"calls": calls, "accepted_steps": steps,
                        "failed_attempts": failed_attempts, "accepted_newton": newton,
                        "wall_s": month_start.elapsed().as_secs_f64()}}));
                break 'months;
            }
            let mut request = cap.map_or(remaining, |c| c.min(remaining));
            if let (Some(g), Some(last)) = (growth, remembered) {
                request = request.min(g * last);
            }
            let before = checkpoint_out.as_ref().map(|_| case.checkpoint());
            let outcome = case.step(request);
            calls += 1;
            failed_attempts += outcome
                .attempts
                .saturating_sub(usize::from(outcome.succeeded()));
            if !outcome.succeeded() {
                if let (Some(path), Some(before)) = (&checkpoint_out, &before) {
                    let text = serde_json::to_string(before).expect("serialize the checkpoint");
                    std::fs::write(path, text).expect("write the checkpoint");
                }
                stopped = Some(json!({"reason": "step failed", "month": month, "day": now,
                    "attempts": outcome.attempts, "failure": format!("{:?}", outcome.failure),
                    "month_so_far": {"calls": calls, "accepted_steps": steps,
                        "failed_attempts": failed_attempts, "accepted_newton": newton,
                        "wall_s": month_start.elapsed().as_secs_f64()}}));
                break 'months;
            }
            steps += 1;
            newton += outcome.newton_iterations;
            min_dt = min_dt.min(outcome.accepted_dt_days.unwrap_or(f64::NAN));
            remembered = outcome.accepted_dt_days;
        }
        let record = json!({
            "month": month,
            "calls": calls,
            "accepted_steps": steps,
            "failed_attempts": failed_attempts,
            "accepted_newton": newton,
            "newton_upper_bound": newton + failed_attempts * 20,
            "min_accepted_dt_days": min_dt,
            "wall_s": month_start.elapsed().as_secs_f64(),
            "balance_rel": balance_since(&case, &n0, &c0),
            "snapshot": case.snapshot(),
        });
        println!(
            "month {month}: {steps} steps, {failed_attempts} failed attempts, newton {newton}, \
             min dt {min_dt:.3e} d, {:.1} s",
            record["wall_s"].as_f64().unwrap_or(f64::NAN)
        );
        months.push(record);
        write(&months, &stopped);
    }
    write(&months, &stopped);
    println!(
        "stopped: {stopped:?}; total wall {:.0} s; N1 acceptances {}",
        clock.elapsed().as_secs_f64(),
        split_phase_decision_acceptances_for_test()
    );
}
