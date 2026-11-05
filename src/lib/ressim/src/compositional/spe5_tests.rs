//! The SPE5 Case 1 scenario payload (#52), run natively.
//!
//! The case is authored in `src/lib/catalog/scenarios/comp_spe5_wag.ts` and exported to
//! `opm/compositional/spe5/case.json`, so this test runs the exact payload the browser sends
//! rather than a second transcription of it.

use super::api::{CaseConfig, Checkpoint, CompositionalCase, WellControlConfig};
use crate::fluid::flash::{PhaseState, flash};
use crate::fluid::pinned;
use std::path::PathBuf;

/// The mass density of the initial fluid at the datum, from the pinned EOS.
fn hydrostatic_gradient_bar_per_m(config: &CaseConfig) -> f64 {
    let spec = pinned::spe5().expect("SPE5 specification");
    let state = flash(
        &spec,
        4000.0 * 0.0689476 * 1e5,
        spec.reservoir_temperature_k(),
        &config.initial_composition,
        None,
    )
    .expect("the initial SPE5 state must flash");
    let density = match state.phase_state {
        PhaseState::SingleLiquid => state.liquid.as_ref().expect("liquid").mass_density,
        PhaseState::SingleVapour => state.vapour.as_ref().expect("vapour").mass_density,
        PhaseState::TwoPhase => {
            let volume = state.mixture_molar_volume();
            let liquid = state.liquid.as_ref().expect("liquid");
            let vapour = state.vapour.as_ref().expect("vapour");
            let liquid_fraction = (1.0 - state.beta) / liquid.molar_density / volume;
            let vapour_fraction = (state.beta / vapour.molar_density) / volume;
            liquid_fraction * liquid.mass_density + vapour_fraction * vapour.mass_density
        }
    };
    // rho g is Pa/m; one bar is 1e5 Pa.
    density * 9.80665 / 1e5
}

/// Gravity is initialized at the pinned fluid's own hydrostatic gradient, not at a surface
/// density or a typed pressure gradient.
#[test]
fn comp_spe5_initial_gradient_is_the_pinned_fluids_hydrostatic_gradient() {
    let config = case_config();
    let expected = hydrostatic_gradient_bar_per_m(&config);
    let configured = config
        .initial_pressure_gradient_bar_per_m
        .expect("SPE5 carries a gradient");
    println!("SPE5 EOS gradient {expected:.9} bar/m, configured {configured:.9} bar/m");
    assert!(
        (configured - expected).abs() < 5e-5,
        "configured {configured:.9} bar/m, EOS {expected:.9} bar/m"
    );
}

/// With every well shut in and no schedule, the initial state is a gravity equilibrium: a step
/// must not create a vertical transient of its own.
#[test]
fn comp_spe5_closed_wells_hold_the_initial_gravity_equilibrium() {
    let mut config = case_config();
    config.schedule.clear();
    for well in &mut config.wells {
        well.control = WellControlConfig::ShutIn;
        well.injection_composition = None;
        well.inject_water = false;
    }
    let mut case = CompositionalCase::new(config).expect("admissible");
    let before = case.snapshot();
    let outcome = case.step(10.0);
    assert!(outcome.succeeded(), "{:?}", outcome.failure);
    let after = case.snapshot();
    let worst = before
        .pressure
        .iter()
        .zip(&after.pressure)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f64, f64::max);
    let worst_sw = before
        .water
        .as_ref()
        .zip(after.water.as_ref())
        .map(|(a, b)| {
            a.saturation
                .iter()
                .zip(&b.saturation)
                .map(|(x, y)| (x - y).abs())
                .fold(0.0_f64, f64::max)
        })
        .unwrap_or(0.0);
    println!("SPE5 closed wells: max |dp| {worst:e} bar, max |dSw| {worst_sw:e}");
    assert!(worst < 1e-3, "gravity transient {worst:e} bar");
    assert!(worst_sw < 1e-8, "water moved by {worst_sw:e}");
}

fn case_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../opm/compositional/spe5/case.json")
}

fn case_config() -> CaseConfig {
    let text = std::fs::read_to_string(case_path()).expect("read opm/compositional/spe5/case.json");
    serde_json::from_str(&text).expect("parse the SPE5 case payload")
}

/// The authored payload is admissible: six SPE5 components, the structured 7×7×3 grid, connate
/// water and the WAG schedule. Cheap enough for the compositional gate.
#[test]
fn comp_spe5_case_payload_is_admissible() {
    let config = case_config();
    assert_eq!(config.fluid, super::api::FluidChoice::PinnedSpe5);
    assert_eq!(config.grid.cells, 7 * 7 * 3);
    assert_eq!(config.initial_composition.len(), 6);
    assert_eq!(config.schedule.len(), 36);
    assert_eq!(config.water.as_ref().unwrap().initial_saturation, 0.2);
    let well_count = config.wells.len();
    let case = CompositionalCase::new(config).expect("the SPE5 payload is admissible");
    // #95: the pinned SPE5 fluid carries surface conditions, so the case starts with a surface
    // ledger at exactly zero for every well, water included.
    let snapshot = case.snapshot();
    let rates = snapshot
        .well_surface_rates
        .expect("SPE5 must report surface rates");
    let volumes = snapshot
        .cumulative_well_surface_volumes
        .expect("SPE5 must report surface volumes");
    assert_eq!(rates.len(), well_count);
    assert_eq!(volumes.len(), well_count);
    for rate in &rates {
        assert_eq!(rate.liquid_m3_per_day, 0.0);
        assert_eq!(rate.vapour_m3_per_day, 0.0);
        assert_eq!(rate.water_m3_per_day, Some(0.0));
    }
}

/// A smoke run of the first WAG cycle. Release only: the monthly replay is minutes of debug work.
#[test]
#[ignore = "first WAG cycle; run with --release and --ignored"]
fn comp_spe5_case_accepts_its_first_water_and_gas_slugs() {
    let config = case_config();
    assert_eq!(config.fluid, super::api::FluidChoice::PinnedSpe5);
    assert_eq!(config.water.as_ref().unwrap().initial_saturation, 0.2);
    let mut case = CompositionalCase::new(config).expect("the SPE5 payload is admissible");

    // Two years of production, then one water year and one gas year, at monthly reports.
    let dt = 30.4375;
    for month in 0..48 {
        let mut remaining = dt;
        while remaining > 1e-9 {
            let outcome = case.step(remaining);
            assert!(
                outcome.succeeded(),
                "month {} failed at {:.3} d: {:?}",
                month + 1,
                outcome.time_days,
                outcome.failure
            );
            remaining -= outcome.accepted_dt_days.expect("accepted");
        }
    }
    let snapshot = case.snapshot();
    assert!((snapshot.time_days - 48.0 * dt).abs() < 1e-6);
    assert_eq!(snapshot.component_ids.len(), 6);
    assert!(snapshot.water.is_some(), "the case carries connate water");
    // Water has been injected for a year by month 36, so the near-injector cell must have moved.
    assert!(
        snapshot.water.as_ref().unwrap().saturation[0] > 0.2,
        "water injection did not raise the injector cell's saturation"
    );
    // #95: the producer's separator ledger is live at the end of the first cycle.
    let rates = snapshot
        .well_surface_rates
        .as_ref()
        .expect("SPE5 reports surface rates");
    let volumes = snapshot
        .cumulative_well_surface_volumes
        .as_ref()
        .expect("SPE5 reports surface volumes");
    let producer = rates
        .iter()
        .zip(volumes)
        .find(|(rate, _)| rate.liquid_m3_per_day < 0.0)
        .expect("the producer must have a negative separator-liquid rate");
    assert!(
        producer.1.liquid_m3 < 0.0,
        "the producer's cumulative separator liquid must be negative, got {}",
        producer.1.liquid_m3
    );
    assert!(producer.0.liquid_m3_per_day.is_finite());
    assert!(producer.0.vapour_m3_per_day.is_finite());
    println!(
        "SPE5 first cycle: t={:.1} d, p[0]={:.3} bar, Sw[0]={:.4}, q_liq={:.1} m³/day",
        snapshot.time_days,
        snapshot.pressure[0],
        snapshot.water.as_ref().unwrap().saturation[0],
        -producer.0.liquid_m3_per_day
    );
}

/// Sixteen years of WAG. Release only: the monthly replay is minutes of work.
///
/// This is the checkpoint generator and the from-scratch replay up to the month-192 seed. The
/// month-192 → 201 segment that [#93](https://github.com/sergeyfarin/ressim/issues/93) used to
/// stall is covered by `comp_spe5_resume_192_clears_the_201_report`; the later horizon is blocked
/// by the separate flash-acceleration gap (#99).
#[test]
#[ignore = "16-year SPE5 replay; run with --release"]
fn comp_spe5_case_reaches_the_late_life_checkpoint() {
    let mut case = CompositionalCase::new(case_config()).expect("admissible");
    let dt = 30.4375;
    let started = std::time::Instant::now();
    for month in 0..192 {
        let mut remaining = dt;
        while remaining > 1e-9 {
            let outcome = case.step(remaining);
            assert!(
                outcome.succeeded(),
                "month {} failed at {:.3} d: {:?}",
                month + 1,
                outcome.time_days,
                outcome.failure
            );
            remaining -= outcome.accepted_dt_days.expect("accepted");
        }
        if (month + 1) % 24 == 0 {
            let state = case.snapshot();
            let mean = state.pressure.iter().sum::<f64>() / state.pressure.len() as f64;
            let min = state.pressure.iter().cloned().fold(f64::INFINITY, f64::min);
            println!(
                "SPE5 month {}: {:?}, mean p {:.3} bar, min p {:.3} bar",
                month + 1,
                started.elapsed(),
                mean,
                min
            );
        }
    }
    let snapshot = case.snapshot();
    assert!((snapshot.time_days - 192.0 * dt).abs() < 1e-6);
    assert!(snapshot.pressure.iter().all(|p| p.is_finite() && *p > 0.0));
    if let Ok(path) = std::env::var("SPE5_CHECKPOINT_OUT") {
        let path = if std::path::Path::new(&path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../..")
                .join(path)
        };
        std::fs::write(
            path,
            serde_json::to_string(&case.checkpoint()).expect("serialize the checkpoint"),
        )
        .expect("write the checkpoint");
    }
    println!(
        "SPE5 month 192: t={:.1} d, wall={:?}",
        snapshot.time_days,
        started.elapsed()
    );
}

/// #112 regression: the from-scratch replay must clear the month-215 flash trial.
///
/// The near-trivial VapourLike trial at cell 32 used to run to the 100,000-iteration cap at
/// 6528.84375 d and fail the step. The practical residual floor resolves it as the stable trial it
/// is. The current replay reaches month 218 before the next, separate nonlinear plateau, so this
/// test stops at month 217 and asserts the 215 region is behind it.
#[test]
#[ignore = "SPE5 from-scratch replay through month 217; run with --release"]
fn comp_spe5_from_scratch_clears_the_month_215_flash_stall() {
    let mut case = CompositionalCase::new(case_config()).expect("admissible");
    let dt = 30.4375;
    let started = std::time::Instant::now();
    for month in 0..217 {
        let mut remaining = dt;
        while remaining > 1e-9 {
            let outcome = case.step(remaining);
            assert!(
                outcome.succeeded(),
                "month {} failed at {:.3} d: {:?}",
                month + 1,
                outcome.time_days,
                outcome.failure
            );
            remaining -= outcome.accepted_dt_days.expect("accepted");
        }
    }
    let snapshot = case.snapshot();
    assert!((snapshot.time_days - 217.0 * dt).abs() < 1e-6);
    println!(
        "SPE5 from scratch reached month 217 at t={:.3} d in {:?}",
        snapshot.time_days,
        started.elapsed()
    );
}

/// Fast #93 regression: the exact failing call must converge cleanly.
///
/// `stall_call_058.json` is the accepted state at 6114.808 d, immediately before the call that
/// used to exhaust all 20 Newton iterations at a scaled residual of 1.04e-3. With the
/// near-critical stability fix the same call takes a handful.
#[test]
#[ignore = "fast #93 regression from the committed failing state; run with --release"]
fn comp_spe5_resume_192_first_stalled_call_converges() {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../opm/compositional/spe5/stall_call_058.json"),
    )
    .expect("read the committed SPE5 failing state");
    let checkpoint: Checkpoint = serde_json::from_str(&text).expect("parse the checkpoint");
    assert!(
        (checkpoint.time_days - 6114.808).abs() < 0.01,
        "the failing-state fixture moved to {:.3} d",
        checkpoint.time_days
    );
    let mut case = CompositionalCase::restore(&checkpoint).expect("restore the failing state");
    let outcome = case.step(3.129044);
    assert!(
        outcome.succeeded(),
        "the #93 call failed again at {:.3} d: {:?}",
        outcome.time_days,
        outcome.failure
    );
    assert_eq!(outcome.attempts, 1, "the call still needs a timestep cut");
    assert!(
        outcome.newton_iterations <= 10,
        "the call converged only after {} Newton iterations",
        outcome.newton_iterations
    );
    println!(
        "SPE5 #93 call: t={:.3} d, dt={:.3} d, attempts={}, Newton iterations={}",
        outcome.time_days,
        outcome.accepted_dt_days.expect("accepted"),
        outcome.attempts,
        outcome.newton_iterations
    );
}

/// #93 regression: the month-192 → 201 segment that used to stall.
///
/// The seed is the **pre-fix** checkpoint that reproduced the 6114.808 d stall. It is deliberately
/// kept rather than regenerated, so the regression starts exactly where the solver used to stop.
/// Before the fix this failed after 58 calls with `Budget` over `Nonlinear: Newton did not converge`
/// at a scaled residual of 1.04e-3; the flash jumped from a real two-phase split to a single liquid
/// at a near-critical bubble point and Newton could not cross the jump.
///
/// The segment ends at the month-201 report the #93 reproducer named. A later near-critical region
/// around 6375 d (month 209) needs flash acceleration and is tracked separately as #99; it is not
/// part of this regression.
#[test]
#[ignore = "SPE5 month-192 to month-201 segment; run with --release"]
fn comp_spe5_resume_192_clears_the_201_report() {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../opm/compositional/spe5/stall_192.json"),
    )
    .expect("read the committed SPE5 checkpoint");
    let checkpoint: Checkpoint = serde_json::from_str(&text).expect("parse the checkpoint");
    let mut case = CompositionalCase::restore(&checkpoint).expect("restore the checkpoint");
    let target = 201.0 * 30.4375;
    let budget = 80;
    let started = std::time::Instant::now();
    let mut calls = 0;
    while case.time_days() < target - 1e-9 {
        calls += 1;
        assert!(
            calls <= budget,
            "SPE5 did not reach month 201 within {budget} calls (last t={:.3} d)",
            case.time_days()
        );
        let dt = (target - case.time_days()).min(5.0);
        let outcome = case.step(dt);
        assert!(
            outcome.succeeded(),
            "SPE5 failed at {:.3} d after {calls} calls: {:?}",
            outcome.time_days,
            outcome.failure
        );
    }
    let snapshot = case.snapshot();
    assert!(
        (snapshot.time_days - target).abs() < 1e-6,
        "resumed replay ended at {:.6} d, expected {target:.6}",
        snapshot.time_days
    );
    assert!(snapshot.pressure.iter().all(|p| p.is_finite() && *p > 0.0));
    println!(
        "SPE5 cleared month 201: t={:.3} d, {calls} calls, wall={:?}",
        snapshot.time_days,
        started.elapsed()
    );
}

/// #99 regression: the month-192 → 6375 d near-critical region that used to stall.
///
/// The seed is the committed month-192 checkpoint, so this starts exactly where #93 left off.
/// Before the flash acceleration, the 6375.188 d flash needed 20,696 substitutions and then the
/// timestep collapsed 32× to 0.156 d; the stability test's `LiquidLike` trial at 6366.59 d also
/// exhausted its 20,000-iteration cap. Both caps are now backstops: every call in the segment
/// succeeds, no accepted step is shorter than 2.5 d (except a final report remainder), and the
/// Newton solve takes at most 20 iterations.
#[test]
#[ignore = "SPE5 month-192 to 6375 d near-critical region; run with --release"]
fn comp_spe5_resume_192_clears_the_near_critical_region() {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../opm/compositional/spe5/stall_192.json"),
    )
    .expect("read the committed SPE5 checkpoint");
    let checkpoint: super::api::Checkpoint =
        serde_json::from_str(&text).expect("parse the checkpoint");
    let mut case = CompositionalCase::restore(&checkpoint).expect("restore the checkpoint");

    let target = 6375.2;
    let budget = 140;
    let started = std::time::Instant::now();
    let mut calls = 0usize;
    let mut shortest = f64::INFINITY;
    let mut worst_newton = 0usize;
    while case.time_days() < target - 1e-9 {
        calls += 1;
        assert!(
            calls <= budget,
            "SPE5 did not reach {target} d within {budget} calls (last t={:.3} d)",
            case.time_days()
        );
        let dt = (target - case.time_days()).min(5.0);
        let outcome = case.step(dt);
        assert!(
            outcome.succeeded(),
            "SPE5 failed at {:.3} d after {calls} calls: {:?}",
            outcome.time_days,
            outcome.failure
        );
        // Only a retry is evidence of a collapse. A single-attempt step shorter than the request
        // is a schedule event or the final report remainder, both legitimate.
        if outcome.attempts > 1 {
            if let Some(accepted) = outcome.accepted_dt_days {
                if dt >= 2.5 {
                    shortest = shortest.min(accepted);
                }
            }
        }
        worst_newton = worst_newton.max(outcome.newton_iterations);
    }

    let snapshot = case.snapshot();
    assert!((snapshot.time_days - target).abs() < 1e-6);
    assert!(snapshot.pressure.iter().all(|p| p.is_finite() && *p > 0.0));
    assert!(
        shortest >= 2.5 - 1e-9,
        "the near-critical region still collapses the timestep: shortest retried step {shortest:.6} d"
    );
    assert!(
        worst_newton <= 20,
        "the near-critical region needed {worst_newton} Newton iterations"
    );
    println!(
        "SPE5 cleared 6375 d: t={:.3} d, {calls} calls, shortest {shortest:.3} d, worst {worst_newton} Newton, wall={:?}",
        snapshot.time_days,
        started.elapsed()
    );
}

/// #102 regression: the rest of the 20-year replay from the month-192 checkpoint.
///
/// The next trial-substitution stall after #99 was at 6646.0625 d, cell 33, where a `LiquidLike`
/// trial sat just above the trivial-solution threshold and needed more than 100,000 substitutions.
/// The first half of this segment is covered by `comp_spe5_resume_192_clears_the_near_critical_region`;
/// here the whole month-192 → month-240 replay must finish without a failure.
#[test]
#[ignore = "SPE5 month-192 to month-240 replay; run with --release"]
fn comp_spe5_resume_192_completes_the_20_year_replay() {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../opm/compositional/spe5/stall_192.json"),
    )
    .expect("read the committed SPE5 checkpoint");
    let checkpoint: super::api::Checkpoint =
        serde_json::from_str(&text).expect("parse the checkpoint");
    let mut case = CompositionalCase::restore(&checkpoint).expect("restore the checkpoint");

    let target = 240.0 * 30.4375;
    let budget = 500;
    let started = std::time::Instant::now();
    let mut calls = 0usize;
    let mut worst_newton = 0usize;
    while case.time_days() < target - 1e-9 {
        calls += 1;
        assert!(
            calls <= budget,
            "SPE5 did not reach {target} d within {budget} calls (last t={:.3} d)",
            case.time_days()
        );
        let dt = (target - case.time_days()).min(5.0);
        let outcome = case.step(dt);
        assert!(
            outcome.succeeded(),
            "SPE5 failed at {:.3} d after {calls} calls: {:?}",
            outcome.time_days,
            outcome.failure
        );
        worst_newton = worst_newton.max(outcome.newton_iterations);
    }

    let snapshot = case.snapshot();
    assert!((snapshot.time_days - target).abs() < 1e-6);
    assert!(snapshot.pressure.iter().all(|p| p.is_finite() && *p > 0.0));
    println!(
        "SPE5 completed month 240: t={:.3} d, {calls} calls, worst {worst_newton} Newton, wall={:?}",
        snapshot.time_days,
        started.elapsed()
    );
}

/// #116 regression: the month-218 seed's accepted state must reproduce its own accumulation.
///
/// `stall_218.json` is the frozen month-218 state at 6612.820363 d, where the from-scratch
/// replay's Newton iteration starts alternating a cell between single-liquid and two-phase.
/// The checkpoint is a seed for that solver investigation, not a current-engine state.
#[test]
fn comp_spe5_month218_seed_state_is_consistent() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../opm/compositional/spe5/stall_218.json");
    let text = std::fs::read_to_string(path).expect("read stall_218.json");
    let checkpoint: Checkpoint = serde_json::from_str(&text).expect("parse checkpoint");
    let case = CompositionalCase::restore(&checkpoint).expect("restore");
    // A tiny step isolates the accumulation term: a consistent accepted state must reproduce it
    // without the flux contribution. Newton starts from freshly guessed well BHPs, not the
    // checkpointed ones, so its first residual is larger and is reported separately.
    let (accepted_norm, _, newton_norm, _) = case.diagnostic_residuals(1e-9);
    println!("accepted-state residual {accepted_norm:e}; Newton starting residual {newton_norm:e}");
    assert!(
        accepted_norm < 1e-8,
        "accepted-state scaled residual is {accepted_norm:e}"
    );
}

/// #116 regression target: cell 33's near-critical crossing is a model jump, not a tolerance
/// artefact. The two-phase branch ends where the phases merge, at `beta` around 0.48.
#[test]
fn comp_spe5_month218_cell33_crosses_at_the_near_critical_pressure() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../opm/compositional/spe5/stall_218.json");
    let text = std::fs::read_to_string(path).expect("read stall_218.json");
    let checkpoint: Checkpoint = serde_json::from_str(&text).expect("parse checkpoint");
    let case = CompositionalCase::restore(&checkpoint).expect("restore");
    let snapshot = case.snapshot();
    let z: Vec<f64> = snapshot.composition.iter().map(|c| c[33]).collect();
    let spec = pinned::spe5().expect("SPE5 specification");
    let t = spec.reservoir_temperature_k();
    let below = flash(&spec, 228.1242e5, t, &z, None).expect("below the crossing");
    let above = flash(&spec, 228.1244e5, t, &z, None).expect("above the crossing");
    assert_eq!(below.phase_state, PhaseState::TwoPhase);
    assert!(
        below.beta > 0.4,
        "beta below the crossing is {}",
        below.beta
    );
    assert_eq!(above.phase_state, PhaseState::SingleLiquid);
}

/// #119 regression seed: cell 32's near-critical flash exhausts the iteration cap.
///
/// The accepted state is at 6540.217531 d; the memory-capped next step is 2.0409934222698212e-6 d.
/// The flash is still converging (equilibrium residual 1.10e-8 against 1e-11) when the 17,500-
/// iteration cap rejects it. This test pins that reproduction; when the acceleration is fixed it
/// should be changed to expect success.
#[test]
#[ignore = "SPE5 month-215 flash-cap seed; run with --release"]
fn comp_spe5_month215_flash_seed_reproduces_the_cap() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../opm/compositional/spe5/stall_215_memory.json");
    let text = std::fs::read_to_string(path).expect("read stall_215_memory.json");
    let checkpoint: Checkpoint = serde_json::from_str(&text).expect("parse checkpoint");
    let mut case = CompositionalCase::restore(&checkpoint).expect("restore");
    let outcome = case.step(2.0409934222698212e-6);
    println!("month-215 flash seed outcome: {outcome:?}");
    assert!(!outcome.succeeded(), "the seed stopped reproducing the cap");
    let failure = outcome.failure.expect("a failure");
    assert_eq!(failure.kind, "Flash");
    assert!(
        failure.message.contains("cell 32"),
        "failure moved away from cell 32: {}",
        failure.message
    );
}
