//! #116 M1: the engine-side jump at the 50-digit split's collapse or trivial cutoff.
//!
//! `tools/jutul/spe5_m1_split_mp.py` follows the physical split from each M0 crossing point in
//! 50-digit arithmetic until it either reaches a phase-vanish endpoint (beta -> 0 or 1) or hits
//! the production trivial-solution cutoff (`sum ln^2 K < 1e-5`). This test takes the states that
//! script writes and asks the engine what would change if N1 returned each one where the standard
//! flash now returns a single phase:
//!
//! - component inventory per pore volume (the state's mixture molar volume);
//! - every incident face's component molar flux, with the neighbour held at the seed's state;
//! - the assembled scaled residual, split versus standard single phase;
//! - the vapour saturation, densities and hydrocarbon mobilities.
//!
//! It changes no production path. Run:
//!
//! ```bash
//! SPE5_M1_STATES=DIR/m1_split_mp.json SPE5_TRACE_OUT=DIR \
//!     cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
//!     comp_spe5_m1_cutoff_jump -- --ignored --nocapture
//! ```
use super::accumulation::EquationScaling;
use super::assembly::AssemblyResult;
use super::flux::face_flux;
use super::newton::assemble_for_test;
use super::spe5_m0_branch_tests::{CASES, restore};
use super::state::CompositionalState;
use crate::fluid::flash::{
    FlashState, PhaseState, flash, two_phase_state_from_ln_k_for_test, with_forced_states_for_test,
};
use crate::fluid::units::bar_to_pa;
use serde_json::{Value, json};
use std::path::PathBuf;

fn write(name: &str, value: &Value) {
    if let Ok(dir) = std::env::var("SPE5_TRACE_OUT") {
        let path = PathBuf::from(dir).join(name);
        std::fs::write(
            &path,
            serde_json::to_string_pretty(value).expect("serialize"),
        )
        .expect("write");
        println!("written {}", path.display());
    }
}

fn case_for(name: &str) -> (&'static str, f64) {
    let (_, seed, dt, _) = CASES
        .iter()
        .find(|(case, _, _, _)| *case == name)
        .unwrap_or_else(|| panic!("no seed for {name}"));
    (seed, *dt)
}

fn state_at(
    start: &CompositionalState,
    cell: usize,
    pressure_bar: f64,
    z: &[f64],
) -> CompositionalState {
    let mut trial = start.begin_trial();
    {
        let target = trial.cell_mut(cell);
        target.pressure_bar = pressure_bar;
        let independent = target.independent_z_mut();
        independent.copy_from_slice(&z[..independent.len()]);
    }
    trial.commit().expect("the M1 state is admissible")
}

fn inventory_per_component(state: &FlashState) -> Vec<f64> {
    // The same phase-sum form as the accumulation residual, not the equilibrium shortcut
    // `z_i / v_mix`: a state that is not exactly material-balanced must show it here.
    match state.phase_state {
        PhaseState::SingleLiquid => {
            let density = state.liquid.as_ref().expect("liquid").molar_density;
            state.x.iter().map(|x| density * x).collect()
        }
        PhaseState::SingleVapour => {
            let density = state.vapour.as_ref().expect("vapour").molar_density;
            state.y.iter().map(|y| density * y).collect()
        }
        PhaseState::TwoPhase => {
            let liquid_density = state.liquid.as_ref().expect("liquid").molar_density;
            let vapour_density = state.vapour.as_ref().expect("vapour").molar_density;
            let vapour_saturation = state.vapour_saturation();
            let liquid_saturation = 1.0 - vapour_saturation;
            state
                .x
                .iter()
                .zip(&state.y)
                .map(|(x, y)| {
                    liquid_saturation * liquid_density * x + vapour_saturation * vapour_density * y
                })
                .collect()
        }
    }
}

fn scaled_max(residual: &[f64], scale: &EquationScaling) -> f64 {
    scale
        .scale_residual(residual)
        .iter()
        .fold(0.0f64, |m, value| m.max(value.abs()))
}

fn cell_block(assembly: &AssemblyResult, cell: usize) -> Vec<f64> {
    let block = assembly.residual.len() / assembly.inventories.len();
    assembly.residual[cell * block..(cell + 1) * block].to_vec()
}

/// Reject a truncated continuation or a record whose named collapse is not an endpoint.
///
/// The producer writes `reached_endpoint`; a consumer that ignored it could accept an interior
/// point as the stated collapse. The beta check is independent of that flag.
fn validate_endpoint(
    name: &str,
    reached_endpoint: bool,
    collapse: Option<&Value>,
    has_cutoff: bool,
) -> Result<(), String> {
    if !reached_endpoint {
        return Err(format!(
            "{name}: continuation did not reach an endpoint (reached_endpoint is not true)"
        ));
    }
    let collapse = collapse.ok_or_else(|| format!("{name}: no collapse point"))?;
    let beta = collapse["beta"]
        .as_f64()
        .ok_or_else(|| format!("{name}: collapse has no beta"))?;
    if !(beta <= 1e-9 || beta >= 1.0 - 1e-9) {
        return Err(format!(
            "{name}: collapse beta {beta} is interior, not a phase-vanish endpoint"
        ));
    }
    if has_cutoff != (name == "memory") {
        return Err(format!("{name}: trivial-cutoff contract violated"));
    }
    Ok(())
}

fn validate_input_case(case: &Value) -> Result<(), String> {
    let name = case["case"].as_str().ok_or("missing case name")?;
    validate_endpoint(
        name,
        case["reached_endpoint"] == Value::Bool(true),
        case.get("collapse"),
        case["trivial_cutoff"].is_object(),
    )
}

fn validate_output_case(case: &Value) -> Result<(), String> {
    let name = case["case"].as_str().ok_or("missing case name")?;
    let points = case["points"]
        .as_array()
        .ok_or_else(|| format!("{name}: missing points"))?;
    validate_endpoint(
        name,
        case["reached_endpoint"] == Value::Bool(true),
        points.iter().find(|point| point["kind"] == "collapse"),
        points.iter().any(|point| point["kind"] == "trivial_cutoff"),
    )
}

fn describe(viscosity_state: &FlashState) -> Value {
    let (vl, vv) = (
        viscosity_state
            .liquid
            .as_ref()
            .map(|phase| phase.molar_volume),
        viscosity_state
            .vapour
            .as_ref()
            .map(|phase| phase.molar_volume),
    );
    json!({
        "phase": format!("{:?}", viscosity_state.phase_state),
        "beta": viscosity_state.beta,
        "vapour_saturation": viscosity_state.vapour_saturation(),
        "liquid_molar_volume": vl,
        "vapour_molar_volume": vv,
        "mixture_molar_volume": viscosity_state.mixture_molar_volume(),
        "liquid_mass_density": viscosity_state.liquid.as_ref().map(|p| p.mass_density),
        "vapour_mass_density": viscosity_state.vapour.as_ref().map(|p| p.mass_density),
        "equilibrium_residual": viscosity_state.equilibrium_residual,
    })
}

#[test]
#[ignore = "#116 M1 engine jump at the 50-digit cutoff/collapse; set SPE5_M1_STATES; seconds"]
fn comp_spe5_m1_cutoff_jump() {
    let path = std::env::var("SPE5_M1_STATES").expect("set SPE5_M1_STATES");
    let input: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("parse");
    let summary = input["cases"].as_array().expect("cases");
    assert_eq!(
        input["complete"],
        Value::Bool(true),
        "the continuation file is incomplete"
    );
    let mut out = Vec::new();
    for summary_case in summary {
        validate_input_case(summary_case).unwrap_or_else(|error| panic!("{error}"));
        let name = summary_case["case"].as_str().expect("case");
        let cell = summary_case["cell"].as_u64().expect("cell") as usize;
        let z: Vec<f64> = summary_case["z"]
            .as_array()
            .expect("z")
            .iter()
            .filter_map(Value::as_f64)
            .collect();
        let (seed, dt) = case_for(name);
        let case = restore(seed);
        let record = case.with_diagnostic_problem(dt, |problem, start, _options| {
            let mut points = Vec::new();
            for kind in ["start", "trivial_cutoff", "collapse"] {
                let state = &summary_case[kind];
                if !state.is_object() {
                    continue;
                }
                let Some(beta) = state["beta"].as_f64() else {
                    continue;
                };
                let p_pa = state["p_pa"].as_f64().expect("p_pa");
                let ln_k: Vec<f64> = state["ln_k"]
                    .as_array()
                    .expect("ln_k")
                    .iter()
                    .filter_map(Value::as_f64)
                    .collect();
                let p_bar = p_pa / 1e5;
                let trial = state_at(start, cell, p_bar, &z);
                let p_engine = bar_to_pa(p_bar);
                let z_engine = trial.cell(cell).overall_composition();
                let split = two_phase_state_from_ln_k_for_test(
                    problem.spec,
                    p_engine,
                    problem.spec.reservoir_temperature_k(),
                    &z_engine,
                    &ln_k,
                    beta,
                )
                .expect("the M1 split rebuilds a two-phase state");
                let standard = flash(
                    problem.spec,
                    p_engine,
                    problem.spec.reservoir_temperature_k(),
                    &z_engine,
                    None,
                )
                .expect("standard flash");
                let forced = vec![(
                    p_engine,
                    problem.spec.reservoir_temperature_k(),
                    z_engine.clone(),
                    split.clone(),
                )];
                let bhp = problem.accepted_well_bhp.expect("accepted BHP");
                let standard_assembly = assemble_for_test(problem, &trial, bhp).ok();
                let split_assembly = with_forced_states_for_test(&forced, || {
                    assemble_for_test(problem, &trial, bhp).ok()
                });
                let inventory_standard = inventory_per_component(&standard);
                let inventory_split = inventory_per_component(&split);
                let inventory_relative: Vec<f64> = inventory_split
                    .iter()
                    .zip(&inventory_standard)
                    .map(|(a, b)| (a - b) / b.abs().max(f64::MIN_POSITIVE))
                    .collect();
                let (d_residual, scaled_jump) = match (&standard_assembly, &split_assembly) {
                    (Some(standard), Some(split_assembly)) => {
                        let d: Vec<f64> = cell_block(split_assembly, cell)
                            .iter()
                            .zip(cell_block(standard, cell))
                            .map(|(a, b)| a - b)
                            .collect();
                        let jump = scaled_max(&d, &standard.scaling[cell]);
                        (Some(d), Some(jump))
                    }
                    _ => (None, None),
                };

                let mut faces = Vec::new();
                for face in problem
                    .faces
                    .iter()
                    .filter(|face| face.cell_i == cell || face.cell_j == cell)
                {
                    let eval = |forced: &[(f64, f64, Vec<f64>, FlashState)]| {
                        with_forced_states_for_test(forced, || {
                            face_flux(
                                problem.spec,
                                problem.relperm,
                                face.geom_t,
                                face.gravity,
                                (face.cell_i, trial.cell(face.cell_i)),
                                (face.cell_j, trial.cell(face.cell_j)),
                            )
                            .ok()
                        })
                    };
                    let (Some(standard_flux), Some(split_flux)) = (eval(&[]), eval(&forced)) else {
                        continue;
                    };
                    let sign = if face.cell_i == cell { 1.0 } else { -1.0 };
                    let relative: Vec<f64> = standard_flux
                        .component_moles_per_day
                        .iter()
                        .zip(&split_flux.component_moles_per_day)
                        .map(|(a, b)| (sign * (b - a)) / a.abs().max(f64::MIN_POSITIVE))
                        .collect();
                    faces.push(json!({
                        "neighbour": if face.cell_i == cell { face.cell_j } else { face.cell_i },
                        "standard_component_moles_per_day": standard_flux.component_moles_per_day,
                        "split_component_moles_per_day": split_flux.component_moles_per_day,
                        "relative_change_from_cell": relative,
                    }));
                }
                println!(
                    "{name} cell {cell} {kind}: p {p_bar:.9} beta {beta:.9e} -> {:?}; \
                     scaled residual jump {}; inventory rel max {:.3e}",
                    standard.phase_state,
                    scaled_jump.map_or("n/a".to_string(), |value| format!("{value:.3e}")),
                    inventory_relative
                        .iter()
                        .fold(0.0f64, |m, value| m.max(value.abs()))
                );
                points.push(json!({
                    "kind": kind,
                    "p_bar": p_bar,
                    "beta": beta,
                    "sum_ln_k_sq": state["sum_ln_k_sq"],
                    "g_split_minus_lower_feed": state["g_split_minus_lower_feed"],
                    "split": describe(&split),
                    "standard": describe(&standard),
                    "inventory_per_component_split": inventory_split,
                    "inventory_per_component_standard": inventory_standard,
                    "inventory_relative": inventory_relative,
                    "d_residual_mol": d_residual,
                    "scaled_residual_jump": scaled_jump,
                    "faces": faces,
                }));
            }
            json!({"case": name, "cell": cell, "seed": seed, "dt_days": dt,
                "reached_endpoint": summary_case["reached_endpoint"],
                "points": points})
        });
        out.push(record);
    }
    write("m1_cutoff_jump.json", &json!({"cases": out}));
    assert_eq!(out.len(), summary.len());
    assert_eq!(out.len(), 8);
    for case in &out {
        validate_output_case(case).unwrap_or_else(|error| panic!("{error}"));
        let points = case["points"].as_array().expect("points");
        for point in points {
            for value in point["inventory_relative"].as_array().expect("inventory") {
                assert!(value.as_f64().is_some_and(f64::is_finite));
            }
        }
    }
}

#[test]
fn comp_spe5_m1_endpoint_guard_rejects_a_truncated_continuation() {
    let endpoint = json!({"beta": 1.0});
    validate_endpoint("cap5", true, Some(&endpoint), false).expect("a real endpoint is accepted");

    // The review's false pass: an interior point named `collapse`, with the producer's
    // incomplete flag. The beta check alone also catches a wrong flag.
    let interior = json!({"beta": 0.7409030797391286});
    assert!(validate_endpoint("cap5", false, Some(&interior), false).is_err());
    assert!(validate_endpoint("cap5", true, Some(&interior), false).is_err());
    assert!(validate_endpoint("cap5", true, None, false).is_err());

    let incomplete = json!({
        "case": "cap5",
        "reached_endpoint": false,
        "collapse": {"beta": 0.7409030797391286},
        "trivial_cutoff": null,
    });
    assert!(validate_input_case(&incomplete).is_err());
    let lying = json!({
        "case": "cap5",
        "reached_endpoint": true,
        "collapse": {"beta": 0.7409030797391286},
        "trivial_cutoff": null,
    });
    assert!(validate_input_case(&lying).is_err());
}
