//! The SPE5 Case 1 scenario payload (#52), run natively.
//!
//! The case is authored in `src/lib/catalog/scenarios/comp_spe5_wag.ts` and exported to
//! `opm/compositional/spe5/case.json`, so this test runs the exact payload the browser sends
//! rather than a second transcription of it.

use super::api::{CaseConfig, CompositionalCase};
use std::path::PathBuf;

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
    println!(
        "SPE5 month 192: t={:.1} d, wall={:?}",
        snapshot.time_days,
        started.elapsed()
    );
}
