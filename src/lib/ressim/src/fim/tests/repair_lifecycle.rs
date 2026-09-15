//! `FIM-REPAIR-F5`: rejected-step rollback and component-inventory conservation.
//!
//! These cover the two lifecycle properties that are load-bearing for every accepted result but
//! were previously asserted nowhere: that a rejected Newton attempt leaves no trace in the
//! simulator, and that a closed system conserves each component's inventory across accepted
//! steps.

use crate::ReservoirSimulator;
use crate::fim::newton::{FimNewtonOptions, run_fim_timestep};
use crate::fim::state::FimState;

/// Full physical snapshot of everything a rejected attempt must not touch.
#[derive(Clone, PartialEq, Debug)]
struct SimSnapshot {
    time_days: f64,
    pressure: Vec<f64>,
    sat_water: Vec<f64>,
    sat_oil: Vec<f64>,
    sat_gas: Vec<f64>,
    rs: Vec<f64>,
    well_bhp: Vec<f64>,
    rate_history_len: usize,
    material_balance_error_m3: f64,
    material_balance_error_oil_m3: f64,
}

impl SimSnapshot {
    fn capture(sim: &ReservoirSimulator) -> Self {
        let latest = sim.rate_history.last();
        Self {
            time_days: sim.time_days,
            pressure: sim.pressure.clone(),
            sat_water: sim.sat_water.clone(),
            sat_oil: sim.sat_oil.clone(),
            sat_gas: sim.sat_gas.clone(),
            rs: sim.rs.clone(),
            well_bhp: sim.wells.iter().map(|well| well.bhp).collect(),
            rate_history_len: sim.rate_history.len(),
            material_balance_error_m3: latest
                .map(|point| point.material_balance_error_m3)
                .unwrap_or(0.0),
            material_balance_error_oil_m3: latest
                .map(|point| point.material_balance_error_oil_m3)
                .unwrap_or(0.0),
        }
    }
}

fn rollback_fixture() -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(4, 1, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_fluid_properties(2.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(1.5e-4, 4.5e-5).unwrap();
    sim.set_initial_pressure(250.0);
    sim.set_initial_saturation(0.25);
    sim.set_gravity_enabled(false);
    sim.add_well(3, 0, 0, 120.0, 0.1, 0.0, false).unwrap();
    sim
}

/// `FIM-REPAIR-F5`, plan step 4: **a rejected Newton attempt must advance nothing.**
///
/// Rollback here is structural rather than an undo: `run_fim_timestep` computes on a `FimState`
/// cloned from the simulator, and only the *accepted* state is written back
/// (`write_back_to_simulator`, called once in the accept branch of `step_internal_fim_impl`).
/// `run_fim_timestep` does take `&mut ReservoirSimulator`, but only so `fim_trace!` can append to
/// the simulator's internal trace buffer — no physical field is assigned.
///
/// That is precisely the kind of invariant that decays: any future code that writes a diagnostic
/// or a controller hint back onto `sim` from inside the Newton loop would silently start leaking
/// rejected-attempt state into accepted results. The failure is injected deterministically by
/// capping the iteration budget rather than by relying on a physical case staying hard — the
/// lesson of the `#[ignore]`d `legacy_resv_failed_direct_fallback_is_rejected_before_state_update`
/// fixture, which stopped exercising its branch when the physics changed under it.
#[test]
fn fim_repair_rejected_attempt_leaves_simulator_state_untouched() {
    let mut sim = rollback_fixture();
    let before = SimSnapshot::capture(&sim);

    let previous_state = FimState::from_simulator(&sim);
    let initial_iterate = previous_state.clone();
    let options = FimNewtonOptions {
        // One iteration cannot satisfy the tolerance on this step, and `OpmAligned` additionally
        // requires `iteration >= OPM_NEWTON_MIN_ITERATION_INDEX` before it may accept at all.
        max_newton_iterations: 1,
        ..FimNewtonOptions::default()
    };

    let report = run_fim_timestep(&mut sim, &previous_state, &initial_iterate, 5.0, &options);
    assert!(
        !report.converged,
        "the fixture must actually produce a rejected attempt"
    );

    let after = SimSnapshot::capture(&sim);
    assert_eq!(
        before, after,
        "a rejected Newton attempt mutated simulator state"
    );
}

/// `FIM-REPAIR-F5`, plan step 4 (second half): a retried step must land on the same physical
/// state as a clean run that started at the accepted smaller `dt`, with no double-counting of
/// time or history from the discarded attempt.
#[test]
fn fim_repair_retried_step_matches_a_clean_run_at_the_same_dt() {
    let mut retried = rollback_fixture();
    let previous_state = FimState::from_simulator(&retried);
    let initial_iterate = previous_state.clone();

    // A capped attempt that is rejected, followed by a normal attempt at the same `dt`, must
    // leave exactly what the normal attempt alone would.
    let capped = FimNewtonOptions {
        max_newton_iterations: 1,
        ..FimNewtonOptions::default()
    };
    let rejected = run_fim_timestep(
        &mut retried,
        &previous_state,
        &initial_iterate,
        5.0,
        &capped,
    );
    assert!(!rejected.converged);

    let options = FimNewtonOptions::default();
    let after_retry = run_fim_timestep(
        &mut retried,
        &previous_state,
        &initial_iterate,
        1.0,
        &options,
    );
    assert!(after_retry.converged);

    let mut clean = rollback_fixture();
    let clean_previous = FimState::from_simulator(&clean);
    let clean_initial = clean_previous.clone();
    let clean_report = run_fim_timestep(&mut clean, &clean_previous, &clean_initial, 1.0, &options);
    assert!(clean_report.converged);

    assert_eq!(
        after_retry.newton_iterations, clean_report.newton_iterations,
        "the discarded attempt changed how the retry converged"
    );
    for idx in 0..after_retry.accepted_state.cells.len() {
        let retried_cell = after_retry.accepted_state.cells[idx];
        let clean_cell = clean_report.accepted_state.cells[idx];
        assert_eq!(
            retried_cell, clean_cell,
            "cell {idx} differs between a retried step and a clean run at the same dt"
        );
    }
}

fn closed_system_fixture() -> ReservoirSimulator {
    // Five cells, no wells, with an initial pressure gradient so fluid genuinely redistributes.
    // A closed system has no sources at all, so total inventory of every component is an exact
    // invariant — which makes this check fully independent of the reported cumulative fields.
    let mut sim = ReservoirSimulator::new(5, 1, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_fluid_properties(2.0, 0.5).unwrap();
    sim.set_fluid_compressibilities(1.5e-4, 4.5e-5).unwrap();
    sim.set_initial_saturation(0.3);
    sim.set_gravity_enabled(false);
    sim.set_initial_pressure(250.0);
    for idx in 0..5 {
        sim.pressure[idx] = 200.0 + 25.0 * idx as f64;
    }
    sim
}

/// Independently computed component inventories at standard conditions, read straight off the
/// simulator arrays. Deliberately **not** derived from `rate_history`'s cumulative fields: those
/// are themselves defined as inventory differences in `step_internal_fim_impl`
/// (`water_after - water_before`, `oil_before - oil_after`, `gas_after - gas_before`), so testing
/// one against the other would be circular by construction.
fn component_inventories(sim: &ReservoirSimulator) -> (f64, f64, f64) {
    let cells = sim.nx * sim.ny * sim.nz;
    let mut water = 0.0;
    let mut oil = 0.0;
    let mut gas = 0.0;
    for idx in 0..cells {
        let pore_volume_m3 = sim.pore_volume_m3(idx).max(1e-9);
        let pressure = sim.pressure[idx];
        water += sim.sat_water[idx] * pore_volume_m3 * sim.water_inverse_fvf(pressure);
        let bo = sim.get_b_o_cell(idx, pressure).max(1e-9);
        oil += sim.sat_oil[idx] * pore_volume_m3 / bo;
        if sim.three_phase_mode {
            gas += sim.sat_gas[idx] * pore_volume_m3 / sim.get_b_g(pressure).max(1e-9)
                + sim.sat_oil[idx] * pore_volume_m3 * sim.rs[idx] / bo;
        }
    }
    (water, oil, gas)
}

/// `FIM-REPAIR-F5`, plan step 5: **a closed system conserves each component's inventory.**
///
/// With no wells there are no sources, so the integrated accepted source over any number of
/// accepted steps is exactly zero and the inventory must be unchanged. The initial pressure
/// gradient makes this non-trivial — fluid really does redistribute and pressures equilibrate —
/// while keeping the invariant exact.
#[test]
fn fim_repair_closed_system_conserves_component_inventory() {
    let mut sim = closed_system_fixture();
    let (water_initial, oil_initial, _) = component_inventories(&sim);
    let initial_spread = sim
        .pressure
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max)
        - sim.pressure.iter().cloned().fold(f64::INFINITY, f64::min);

    for _ in 0..8 {
        sim.step(1.0);
        assert!(
            sim.last_solver_warning.is_empty(),
            "closed-system step emitted a solver warning: {}",
            sim.last_solver_warning
        );
    }

    let (water_final, oil_final, _) = component_inventories(&sim);
    let final_spread = sim
        .pressure
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max)
        - sim.pressure.iter().cloned().fold(f64::INFINITY, f64::min);

    assert!(
        final_spread < initial_spread * 0.75,
        "fixture must actually redistribute fluid: pressure spread {initial_spread:.3} -> \
         {final_spread:.3}"
    );

    // Signed errors, normalized by the initial inventory of the same component.
    let water_error = (water_final - water_initial) / water_initial;
    let oil_error = (oil_final - oil_initial) / oil_initial;
    assert!(
        water_error.abs() < 1e-6,
        "closed-system water inventory drifted by {water_error:e} relative \
         ({water_initial:.6} -> {water_final:.6})"
    );
    assert!(
        oil_error.abs() < 1e-6,
        "closed-system oil inventory drifted by {oil_error:e} relative \
         ({oil_initial:.6} -> {oil_final:.6})"
    );

    // `record_fim_step_report` derives reported production/injection from the **well rates**,
    // while `material_balance_error_*` accumulates `rate-integrated production - inventory
    // change`. With no wells the rate side is identically zero, so that published error is a
    // direct, non-circular measure of numerical conservation drift.
    let point = sim
        .rate_history
        .last()
        .expect("an accepted step must publish a rate-history point");
    assert_eq!(point.total_production_oil, 0.0);
    assert_eq!(point.total_injection, 0.0);
    assert!(
        point.material_balance_error_oil_m3 < oil_initial * 1e-6,
        "closed-system oil material-balance error {:e} Sm3 against {oil_initial:.3} Sm3 in place",
        point.material_balance_error_oil_m3
    );
    assert!(
        point.material_balance_error_m3 < water_initial * 1e-6,
        "closed-system water material-balance error {:e} m3 against {water_initial:.3} in place",
        point.material_balance_error_m3
    );
}
