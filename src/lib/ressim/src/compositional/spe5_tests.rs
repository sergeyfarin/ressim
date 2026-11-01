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
    assert!(CompositionalCase::new(config).is_ok());
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
    println!(
        "SPE5 first cycle: t={:.1} d, p[0]={:.3} bar, Sw[0]={:.4}",
        snapshot.time_days,
        snapshot.pressure[0],
        snapshot.water.as_ref().unwrap().saturation[0]
    );
}

/// Sixteen years of WAG. Release only: the monthly replay is minutes of work.
///
/// The last four years are blocked by the late-life Newton stall filed as #93. Stopping before it
/// is a regression replay of everything up to the stall, not a claim that Case 1 completes.
#[test]
#[ignore = "16-year SPE5 replay; #93 blocks the last four years"]
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

/// #93: restore the accepted month-192 state and walk into the late-life stall.
///
/// This is a reproducer, not an acceptance test. It fails if the stall disappears, which is the
/// signal that #93 has been fixed and the full 20-year replay should replace it.
#[test]
#[ignore = "reproduces #93 from the committed month-192 checkpoint; run with --release"]
fn comp_spe5_resume_192_reproduces_the_stall() {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../opm/compositional/spe5/stall_192.json"),
    )
    .expect("read the committed SPE5 checkpoint");
    let checkpoint: Checkpoint = serde_json::from_str(&text).expect("parse the checkpoint");
    let mut case = CompositionalCase::restore(&checkpoint).expect("restore the checkpoint");
    let target = 201.0 * 30.4375;
    let budget = 80;
    for call in 1..=budget {
        let dt = (target - case.time_days()).min(5.0);
        let outcome = case.step(dt);
        if !outcome.succeeded() {
            let failure = outcome.failure.expect("a failed step carries a failure");
            println!(
                "SPE5 stalled at {:.3} d after {} calls: {} — {}",
                outcome.time_days, call, failure.kind, failure.message
            );
            assert_eq!(failure.kind, "Budget");
            assert!(failure.message.contains("Nonlinear"), "{}", failure.message);
            assert!(
                (outcome.time_days - 6114.808).abs() < 0.01,
                "stall moved to {:.3} d; update #93",
                outcome.time_days
            );
            return;
        }
    }
    panic!("the #93 stall did not reproduce within {budget} calls; update the issue");
}
