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

/// `FIM-REPAIR-F6` (#10): a `wf_gravity`-shaped vertical cross-section, optionally with **every**
/// layer perforated.
///
/// Issue #10 reports that a fully perforated gravity well exhausts the IMPES physical-pressure
/// recovery budget near t=0, and its acceptance criteria ask for a focused regression with
/// multi-layer completions. The shipped `wf_gravity` scenario uses single-layer completions by
/// design, so no test covered the multi-completion geometry at all — on either solver.
///
/// This is a reconstruction of that geometry, not a replay of the scenario's exact deck; a
/// negative result here does not prove the shipped case is clean. What it does establish is a
/// backend-neutral comparison: well geometry, the Peaceman productivity index and
/// `refresh_well_head_offsets` are *shared* code, so a well-geometry defect would show on both
/// solvers, while an IMPES-only pressure-recovery failure would not.
fn gravity_section(fim: bool, fully_perforated: bool) -> ReservoirSimulator {
    let nz = 20;
    let mut sim = ReservoirSimulator::new(30, 1, nz, 0.2);
    sim.set_fim_enabled(fim);
    sim.set_cell_dimensions(10.0, 100.0, 2.0);
    sim.set_fluid_properties(5.0, 0.5).unwrap();
    sim.set_fluid_densities(700.0, 1000.0).unwrap();
    sim.set_rel_perm_props(0.2, 0.2, 2.0, 2.0, 0.4, 1.0)
        .unwrap();
    sim.set_initial_pressure(200.0);
    sim.set_initial_saturation(0.2);
    sim.set_gravity_enabled(true);
    sim.set_permeability_per_layer(vec![500.0; nz], vec![500.0; nz], vec![50.0; nz])
        .unwrap();
    sim.set_well_control_modes("rate".to_string(), "rate".to_string());
    sim.set_target_well_rates(160.0, 160.0).unwrap();
    sim.set_well_bhp_limits(50.0, 500.0).unwrap();
    let layers: Vec<usize> = if fully_perforated {
        (0..nz).collect()
    } else {
        vec![nz - 1]
    };
    for &k in &layers {
        sim.add_well_with_id(0, 0, k, 400.0, 0.1, 0.0, true, "inj".to_string())
            .unwrap();
        sim.add_well_with_id(29, 0, k, 100.0, 0.1, 0.0, false, "prod".to_string())
            .unwrap();
    }
    sim
}

/// `FIM-REPAIR-F6` (#10): multi-layer completions under gravity must stay physical on **both**
/// solvers, and the two must not diverge.
///
/// Measured at `5ebdc78` over 12 days: no solver warning on either backend for either completion
/// strategy, saturations inside `[s_wc, 1]`, and pressures bounded. Fully perforated, IMPES and
/// FIM agree to about 1 % on the pressure envelope and to 5e-3 on peak water saturation — so this
/// reconstruction finds no shared well-geometry error. Issue #10 remains scoped to IMPES on its
/// own deck.
#[test]
fn fim_repair_multi_completion_gravity_stays_physical_on_both_solvers() {
    for fully_perforated in [false, true] {
        let mut results = Vec::new();
        for fim in [false, true] {
            let mut sim = gravity_section(fim, fully_perforated);
            for _ in 0..12 {
                sim.step(1.0);
                assert!(
                    sim.last_solver_warning.is_empty(),
                    "fully_perforated={fully_perforated} fim={fim}: solver warning at \
                     t={:.3} d: {}",
                    sim.time_days,
                    sim.last_solver_warning
                );
            }

            let pressure_min = sim.pressure.iter().cloned().fold(f64::INFINITY, f64::min);
            let pressure_max = sim
                .pressure
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max);
            let sw_min = sim.sat_water.iter().cloned().fold(f64::INFINITY, f64::min);
            let sw_max = sim
                .sat_water
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max);

            assert!(
                pressure_min > 0.0 && pressure_max.is_finite(),
                "fully_perforated={fully_perforated} fim={fim}: nonphysical pressure state \
                 [{pressure_min:.3}, {pressure_max:.3}]"
            );
            // `s_wc` is not an exact floor. FIM keeps OPM's raw primary-variable state
            // (`apply_newton_update_frozen` -> `resolve_cell_flash`, which deliberately does not
            // clamp so component accumulation retains its derivative through a switch), so a
            // converged cell may sit a few parts in 1e6 outside the endpoint. Measured here:
            // 0.199998 against `s_wc = 0.2`. The contract is "physical to within the raw-primary
            // tolerance", not "exactly bounded" — asserting the latter would be asserting a
            // clamp the engine intentionally does not apply.
            const SATURATION_ENDPOINT_TOLERANCE: f64 = 1e-4;
            assert!(
                sw_min >= 0.2 - SATURATION_ENDPOINT_TOLERANCE
                    && sw_max <= 1.0 + SATURATION_ENDPOINT_TOLERANCE,
                "fully_perforated={fully_perforated} fim={fim}: saturation left its bounds \
                 [{sw_min:.6}, {sw_max:.6}]"
            );
            results.push((pressure_min, pressure_max, sw_max));
        }

        // Well geometry, the Peaceman PI and the wellbore-datum head offset are shared code. A
        // defect there would move both solvers together; a solver-specific failure would not.
        let (impes, fim) = (results[0], results[1]);
        assert!(
            (impes.0 - fim.0).abs() / impes.0 < 0.02 && (impes.1 - fim.1).abs() / impes.1 < 0.02,
            "fully_perforated={fully_perforated}: IMPES and FIM pressure envelopes diverged \
             ({impes:?} vs {fim:?})"
        );
        assert!(
            (impes.2 - fim.2).abs() < 0.02,
            "fully_perforated={fully_perforated}: IMPES and FIM peak water saturation diverged \
             ({:.6} vs {:.6})",
            impes.2,
            fim.2
        );
    }
}
