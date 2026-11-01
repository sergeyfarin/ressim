//! The SPE3 Case 1 scenario payload (#52), run natively.
//!
//! The case is authored in `src/lib/catalog/scenarios/comp_spe3_gas_cycling.ts` and exported to
//! `opm/compositional/spe3/case.json`.

use super::api::{CaseConfig, CompositionalCase, WellControlConfig};
use crate::fluid::flash::{PhaseState, flash};
use crate::fluid::pinned;
use std::path::PathBuf;

fn case_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../opm/compositional/spe3/case.json")
}

fn case_config() -> CaseConfig {
    let text = std::fs::read_to_string(case_path()).expect("read opm/compositional/spe3/case.json");
    serde_json::from_str(&text).expect("parse the SPE3 case payload")
}

fn hydrostatic_gradient_bar_per_m(config: &CaseConfig) -> f64 {
    let spec = pinned::spe3().expect("SPE3 specification");
    let state = flash(
        &spec,
        3550.0 * 0.0689476 * 1e5,
        spec.reservoir_temperature_k(),
        &config.initial_composition,
        None,
    )
    .expect("the initial SPE3 state must flash");
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
    density * 9.80665 / 1e5
}

#[test]
fn comp_spe3_case_payload_is_admissible() {
    let config = case_config();
    assert_eq!(config.fluid, super::api::FluidChoice::PinnedSpe3);
    assert_eq!(config.grid.cells, 9 * 9 * 4);
    assert_eq!(config.initial_composition.len(), 9);
    assert_eq!(config.schedule.len(), 1);
    assert!(CompositionalCase::new(config).is_ok());
}

#[test]
fn comp_spe3_initial_gradient_is_the_pinned_fluids_hydrostatic_gradient() {
    let config = case_config();
    let expected = hydrostatic_gradient_bar_per_m(&config);
    let configured = config
        .initial_pressure_gradient_bar_per_m
        .expect("SPE3 carries a gradient");
    println!("SPE3 EOS gradient {expected:.9} bar/m, configured {configured:.9} bar/m");
    assert!(
        (configured - expected).abs() < 5e-5,
        "configured {configured:.9} bar/m, EOS {expected:.9} bar/m"
    );
}

#[test]
#[ignore = "a 1-day SPE3 equilibrium step is minutes of debug work; run with --release"]
fn comp_spe3_closed_wells_hold_the_initial_gravity_equilibrium() {
    let mut config = case_config();
    config.schedule.clear();
    for well in &mut config.wells {
        well.control = WellControlConfig::ShutIn;
        well.injection_composition = None;
        well.inject_water = false;
    }
    let mut case = CompositionalCase::new(config).expect("admissible");
    let before = case.snapshot();
    let outcome = case.step(1.0);
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
    println!("SPE3 closed wells: max |dp| {worst:e} bar, max |dSw| {worst_sw:e}");
    // The engine holds one constant gradient; the EOS density varies over the 160 ft column.
    // A 1-day transient below 0.01 bar is that linearization, not a gravity initialization error.
    assert!(worst < 1e-2, "gravity transient {worst:e} bar");
    assert!(worst_sw < 1e-6, "water moved by {worst_sw:e}");
}
