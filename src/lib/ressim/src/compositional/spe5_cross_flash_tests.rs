//! #116 cross-flash evidence: ResSim's side of the comparison with MultiComponentFlash.
//!
//! The pipeline (`tools/jutul/run-spe5-cross-flash.sh`) dumps both simulators' monthly cell
//! states, builds `(p, z)` points from them, flashes every point with both flashes and merges the
//! results into `opm/compositional/spe5/cross_flash_205_216.json`. The two ignored tests here are
//! ResSim's steps of that pipeline; the fast test checks that the current flash still reproduces
//! the committed ResSim column, so a flash change that moves this evidence is visible.
use super::api::CompositionalCase;
use crate::fluid::flash::{PhaseState, flash, two_phase_gibbs_minus_feed_for_test};
use crate::fluid::pinned;
use crate::fluid::specification::FluidSpecification;
use crate::fluid::stability::{StabilityVerdict, test_stability};
use serde_json::{Value, json};
use std::path::PathBuf;

fn spe5_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../opm/compositional/spe5")
}

fn phase_label(state: PhaseState) -> &'static str {
    match state {
        PhaseState::SingleLiquid => "L",
        PhaseState::SingleVapour => "V",
        PhaseState::TwoPhase => "2P",
    }
}

/// ResSim's record for one point: the default flash (cold start) and a cold stability test.
fn flash_point(spec: &FluidSpecification, p_bar: f64, z: &[f64]) -> Value {
    let t = spec.reservoir_temperature_k();
    let total: f64 = z.iter().sum();
    let z: Vec<f64> = z.iter().map(|v| v / total).collect();
    let p = p_bar * 1e5;
    let stability = match test_stability(spec, p, t, &z, None) {
        Ok(StabilityVerdict::Stable) => "stable".to_string(),
        Ok(StabilityVerdict::Unstable { .. }) => "unstable".to_string(),
        Err(e) => format!("error: {e:?}"),
    };
    match flash(spec, p, t, &z, None) {
        Ok(f) => json!({
            "phase": phase_label(f.phase_state),
            "beta": f.beta,
            "equilibrium_residual": f.equilibrium_residual,
            "iterations": f.iterations,
            "sum_ln_k_sq": f.k.iter().map(|k| crate::math::ln(*k) * crate::math::ln(*k)).sum::<f64>(),
            "gibbs_split_minus_feed_rt": two_phase_gibbs_minus_feed_for_test(spec, p, t, &z, &f),
            "stability": stability,
        }),
        Err(e) => json!({"error": format!("{e:?}"), "stability": stability}),
    }
}

/// Pipeline step: dump ResSim's monthly snapshots for reports `SPE5_STATE_FIRST..=SPE5_STATE_LAST`
/// (default 205..=216) from a from-scratch replay to `SPE5_STATE_OUT`.
#[test]
#[ignore = "#116 cross-flash pipeline; ~8 min from scratch; run with --release --ignored"]
fn comp_spe5_dump_report_states() {
    let out = std::env::var("SPE5_STATE_OUT").expect("set SPE5_STATE_OUT");
    let first: usize = std::env::var("SPE5_STATE_FIRST")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(205);
    let last: usize = std::env::var("SPE5_STATE_LAST")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(216);
    let text = std::fs::read_to_string(spe5_dir().join("case.json")).expect("read case.json");
    let mut case =
        CompositionalCase::new(serde_json::from_str(&text).expect("parse")).expect("admissible");
    let dt = 30.4375;
    let mut states = Vec::new();
    for month in 1..=last {
        let mut remaining = dt;
        while remaining > 1e-9 {
            let outcome = case.step(remaining);
            assert!(
                outcome.succeeded(),
                "month {month} failed: {:?}",
                outcome.failure
            );
            remaining -= outcome.accepted_dt_days.expect("accepted");
        }
        if month >= first {
            states.push(json!({"month": month, "snapshot": case.snapshot()}));
        }
    }
    std::fs::write(
        &out,
        serde_json::to_string(&json!({"schema": "ressim-spe5-states/1", "states": states}))
            .expect("serialize"),
    )
    .expect("write");
}

/// Pipeline step: flash `SPE5_CROSS_FLASH_POINTS` (a list of `{label, p_bar, z}`) with ResSim and
/// write the records to `SPE5_CROSS_FLASH_OUT`.
#[test]
#[ignore = "#116 cross-flash pipeline; run with --release --ignored"]
fn comp_spe5_cross_flash_points() {
    let input = std::env::var("SPE5_CROSS_FLASH_POINTS").expect("set SPE5_CROSS_FLASH_POINTS");
    let out = std::env::var("SPE5_CROSS_FLASH_OUT").expect("set SPE5_CROSS_FLASH_OUT");
    let points: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(input).expect("read")).expect("parse");
    let spec = pinned::spe5().expect("SPE5 specification");
    let records: Vec<Value> = points
        .iter()
        .map(|pt| {
            let z: Vec<f64> = pt["z"]
                .as_array()
                .expect("z")
                .iter()
                .filter_map(Value::as_f64)
                .collect();
            let mut rec = flash_point(&spec, pt["p_bar"].as_f64().expect("p_bar"), &z);
            rec["label"] = pt["label"].clone();
            rec
        })
        .collect();
    std::fs::write(
        out,
        serde_json::to_string_pretty(&json!({"schema": "ressim-spe5-ressim-cross-flash/1",
            "points": records}))
        .expect("serialize"),
    )
    .expect("write");
}

/// The current flash reproduces the committed ResSim column of the cross-flash fixture: the same
/// phase state at every point and `beta` within `1e-9`. A deliberate flash change that moves it
/// must regenerate the fixture (`tools/jutul/run-spe5-cross-flash.sh`) and restate its verdicts.
#[test]
fn comp_spe5_cross_flash_fixture_reproduces_the_ressim_column() {
    let path = spe5_dir().join("cross_flash_205_216.json");
    let fixture: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read the fixture"))
            .expect("parse the fixture");
    let spec = pinned::spe5().expect("SPE5 specification");
    let points = fixture["points"].as_array().expect("points");
    assert!(!points.is_empty());
    for pt in points {
        let z: Vec<f64> = pt["z"]
            .as_array()
            .expect("z")
            .iter()
            .filter_map(Value::as_f64)
            .collect();
        let now = flash_point(&spec, pt["p_bar"].as_f64().expect("p_bar"), &z);
        let then = &pt["ressim"];
        assert_eq!(now["phase"], then["phase"], "{}", pt["label"]);
        let (a, b) = (
            now["beta"].as_f64().unwrap_or(f64::NAN),
            then["beta"].as_f64().unwrap_or(f64::NAN),
        );
        assert!((a - b).abs() <= 1e-9, "{}: beta {a} vs {b}", pt["label"]);
    }
}
