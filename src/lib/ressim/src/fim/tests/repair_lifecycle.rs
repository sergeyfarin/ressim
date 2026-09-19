//! `FIM-REPAIR-F5`: rejected-step rollback and component-inventory conservation.
//!
//! These cover the two lifecycle properties that are load-bearing for every accepted result but
//! were previously asserted nowhere: that a rejected Newton attempt leaves no trace in the
//! simulator, and that a closed system conserves each component's inventory across accepted
//! steps.

use crate::ReservoirSimulator;
use crate::fim::assembly::FimAssemblyOptions;
use crate::fim::assembly_ad::assemble_fim_system_ad;
use crate::fim::newton::convergence::scaled_residual_inf_norm;
use crate::fim::newton::{FimNewtonOptions, FimNonlinearFlavor, run_fim_timestep};
use crate::fim::state::FimState;
use crate::fim::wells::build_well_topology;
use crate::pvt::{PvtRow, PvtTable};

#[derive(Clone, PartialEq, Debug)]
struct RatePointSnapshot {
    time: f64,
    oil_rate: f64,
    liquid_rate: f64,
    liquid_rate_reservoir: f64,
    injection_rate: f64,
    injection_rate_reservoir: f64,
    water_mb_error: f64,
    oil_mb_error: f64,
    gas_rate: f64,
    gas_mb_error: f64,
    avg_pressure: f64,
    avg_sw: f64,
    avg_sg: f64,
    producing_gor: f64,
}

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
    well_flowing_bhp: Vec<Option<f64>>,
    rate_history: Vec<RatePointSnapshot>,
    cumulative_injection_m3: f64,
    cumulative_production_m3: f64,
    cumulative_mb_error_m3: f64,
    cumulative_mb_oil_error_m3: f64,
    cumulative_mb_gas_error_m3: f64,
}

impl SimSnapshot {
    fn capture(sim: &ReservoirSimulator) -> Self {
        Self {
            time_days: sim.time_days,
            pressure: sim.pressure.clone(),
            sat_water: sim.sat_water.clone(),
            sat_oil: sim.sat_oil.clone(),
            sat_gas: sim.sat_gas.clone(),
            rs: sim.rs.clone(),
            well_bhp: sim.wells.iter().map(|well| well.bhp).collect(),
            well_flowing_bhp: sim.wells.iter().map(|well| well.flowing_bhp).collect(),
            rate_history: sim
                .rate_history
                .iter()
                .map(|point| RatePointSnapshot {
                    time: point.time,
                    oil_rate: point.total_production_oil,
                    liquid_rate: point.total_production_liquid,
                    liquid_rate_reservoir: point.total_production_liquid_reservoir,
                    injection_rate: point.total_injection,
                    injection_rate_reservoir: point.total_injection_reservoir,
                    water_mb_error: point.material_balance_error_m3,
                    oil_mb_error: point.material_balance_error_oil_m3,
                    gas_rate: point.total_production_gas,
                    gas_mb_error: point.material_balance_error_gas_m3,
                    avg_pressure: point.avg_reservoir_pressure,
                    avg_sw: point.avg_water_saturation,
                    avg_sg: point.avg_gas_saturation,
                    producing_gor: point.producing_gor,
                })
                .collect(),
            cumulative_injection_m3: sim.cumulative_injection_m3,
            cumulative_production_m3: sim.cumulative_production_m3,
            cumulative_mb_error_m3: sim.cumulative_mb_error_m3,
            cumulative_mb_oil_error_m3: sim.cumulative_mb_oil_error_m3,
            cumulative_mb_gas_error_m3: sim.cumulative_mb_gas_error_m3,
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

/// `FIM-REPAIR-F5`, inner-Newton guard: a rejected Newton solve must advance nothing.
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
        // One iteration cannot satisfy the Legacy tolerances on this fixture. The outer retry
        // contract below covers both Legacy and OpmAligned through the real controller.
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

fn three_phase_lifecycle_fixture() -> ReservoirSimulator {
    let mut sim = ReservoirSimulator::new(3, 1, 1, 0.2);
    sim.set_fim_enabled(true);
    sim.set_three_phase_mode_enabled(true);
    sim.set_three_phase_rel_perm_props(
        0.12, 0.12, 0.04, 0.04, 0.18, 2.0, 2.5, 1.5, 1e-5, 1.0, 0.984,
    )
    .unwrap();
    sim.pvt_table = Some(PvtTable::new(
        vec![
            PvtRow {
                p_bar: 100.0,
                rs_m3m3: 20.0,
                bo_m3m3: 1.10,
                mu_o_cp: 1.20,
                bg_m3m3: 0.0120,
                mu_g_cp: 0.0180,
            },
            PvtRow {
                p_bar: 200.0,
                rs_m3m3: 60.0,
                bo_m3m3: 1.22,
                mu_o_cp: 1.05,
                bg_m3m3: 0.0055,
                mu_g_cp: 0.0200,
            },
            PvtRow {
                p_bar: 300.0,
                rs_m3m3: 95.0,
                bo_m3m3: 1.32,
                mu_o_cp: 0.95,
                bg_m3m3: 0.0034,
                mu_g_cp: 0.0225,
            },
        ],
        sim.pvt.c_o,
    ));
    sim.set_initial_pressure(250.0);
    sim.set_initial_saturation(0.20);
    sim.set_initial_gas_saturation(0.0);
    sim.set_initial_rs(60.0);
    sim.set_gravity_enabled(false);
    sim.add_well(2, 0, 0, 90.0, 0.1, 0.0, false).unwrap();
    sim
}

fn assert_close(label: &str, actual: f64, expected: f64, scale: f64) {
    let tolerance = 2e-9 * scale.abs().max(expected.abs()).max(1.0);
    assert!(
        (actual - expected).abs() <= tolerance,
        "{label}: actual {actual:.16e}, expected {expected:.16e}, tolerance {tolerance:.3e}"
    );
}

/// `FIM-REPAIR-F5`, plan step 3: exercise the actual outer accept/commit/report path and compare
/// its simulator fields with an independently solved accepted state. This is deliberately
/// three-phase: corrupting the `Rs` write-back must fail here even if the inner Newton report is
/// internally consistent.
#[test]
fn fim_repair_outer_commit_matches_accepted_three_phase_state() {
    const DT_DAYS: f64 = 0.05;
    for flavor in [FimNonlinearFlavor::OpmAligned, FimNonlinearFlavor::Legacy] {
        let mut expected_sim = three_phase_lifecycle_fixture();
        let previous_state = FimState::from_simulator(&expected_sim);
        let options = FimNewtonOptions {
            nonlinear_flavor: flavor,
            max_saturation_change: 0.2,
            max_pressure_change_bar: 200.0,
            ..FimNewtonOptions::default()
        };
        let expected = run_fim_timestep(
            &mut expected_sim,
            &previous_state,
            &previous_state,
            DT_DAYS,
            &options,
        );
        assert!(expected.converged, "{flavor:?}: reference Newton solve");

        let mut committed = three_phase_lifecycle_fixture();
        committed.set_fim_opm_aligned_nonlinear(flavor == FimNonlinearFlavor::OpmAligned);
        committed.step(DT_DAYS);
        assert!(committed.last_solver_warning.is_empty());
        let stats = committed
            .last_fim_step_stats_ref()
            .expect("outer step must publish stats");
        let accepted = stats.accepted_rungs.as_deref().unwrap_or(&[]);
        assert_eq!(accepted.len(), 1, "{flavor:?}: expected one real accept");
        assert_close("accepted dt", accepted[0].dt_days, DT_DAYS, DT_DAYS);

        for idx in 0..expected.accepted_state.cells.len() {
            let derived = expected.accepted_state.derive_cell(&expected_sim, idx);
            assert_close(
                "pressure",
                committed.pressure[idx],
                expected.accepted_state.cells[idx].pressure_bar,
                300.0,
            );
            assert_close(
                "Sw",
                committed.sat_water[idx],
                expected.accepted_state.cells[idx].sw,
                1.0,
            );
            assert_close("So", committed.sat_oil[idx], derived.so, 1.0);
            assert_close("Sg", committed.sat_gas[idx], derived.sg, 1.0);
            assert_close("Rs", committed.rs[idx], derived.rs, 100.0);
        }
        for (idx, expected_bhp) in expected.accepted_state.well_bhp.iter().enumerate() {
            assert_close("well BHP", committed.wells[idx].bhp, *expected_bhp, 500.0);
        }

        let committed_state = FimState::from_simulator(&committed);
        let topology = build_well_topology(&committed);
        let assembly = assemble_fim_system_ad(
            &committed,
            &previous_state,
            &committed_state,
            &FimAssemblyOptions {
                dt_days: DT_DAYS,
                include_wells: true,
                assemble_residual_only: true,
                topology: Some(&topology),
                flow_resv_context: None,
            },
        );
        let norm = scaled_residual_inf_norm(&assembly.residual, &assembly.equation_scaling);
        assert!(norm < 1e-5, "{flavor:?}: committed residual {norm:e}");
    }
}

/// `FIM-REPAIR-F5`, plan steps 4–5: inject one real outer-controller rejection, then compare the
/// retried result with a clean run over the exact accepted-dt sequence. The comparison includes
/// every physical field, full rate-history values, well publication and cumulative ledgers.
/// Gas and oil source integration are checked independently from the material-balance fields.
#[test]
fn fim_repair_outer_retry_has_no_physical_or_ledger_trace() {
    const TARGET_DT_DAYS: f64 = 0.15;
    for flavor in [FimNonlinearFlavor::OpmAligned, FimNonlinearFlavor::Legacy] {
        let mut retried = three_phase_lifecycle_fixture();
        retried.set_fim_opm_aligned_nonlinear(flavor == FimNonlinearFlavor::OpmAligned);
        let initial_inventory = component_inventories(&retried);
        retried.force_next_fim_outer_attempts_to_reject(1);
        retried.step(TARGET_DT_DAYS);
        assert!(retried.last_solver_warning.is_empty(), "{flavor:?}");

        let stats = retried
            .last_fim_step_stats_ref()
            .expect("retried outer step must publish stats");
        let accepted_dt: Vec<f64> = stats
            .accepted_rungs
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|rung| rung.dt_days)
            .collect();
        let retry_count = stats.retry_rungs.as_deref().unwrap_or(&[]).len();
        assert_eq!(
            retry_count, 1,
            "{flavor:?}: injected attempt was not rejected"
        );
        assert!(
            !accepted_dt.is_empty(),
            "{flavor:?}: no accepted retry rungs"
        );
        assert_close(
            "accepted dt total",
            accepted_dt.iter().sum(),
            TARGET_DT_DAYS,
            TARGET_DT_DAYS,
        );

        let mut clean = three_phase_lifecycle_fixture();
        clean.set_fim_opm_aligned_nonlinear(flavor == FimNonlinearFlavor::OpmAligned);
        for dt_days in &accepted_dt {
            clean.step(*dt_days);
            assert!(
                clean.last_solver_warning.is_empty(),
                "{flavor:?}: clean replay"
            );
        }
        assert_eq!(
            SimSnapshot::capture(&retried),
            SimSnapshot::capture(&clean),
            "{flavor:?}: rejected attempt leaked into committed state or reporting"
        );

        let final_inventory = component_inventories(&retried);
        let mut integrated_water_production = 0.0;
        let mut integrated_oil_production = 0.0;
        let mut integrated_gas_production = 0.0;
        let mut integrated_gas_injection = 0.0;
        for (point, dt_days) in retried.rate_history.iter().zip(&accepted_dt) {
            integrated_water_production +=
                (point.total_production_liquid - point.total_production_oil) * dt_days;
            integrated_oil_production += point.total_production_oil * dt_days;
            integrated_gas_production += point.total_production_gas * dt_days;
            integrated_gas_injection += point.total_injection * dt_days;
        }
        let water_error = (final_inventory.0 - initial_inventory.0) + integrated_water_production;
        let oil_error = (final_inventory.1 - initial_inventory.1) + integrated_oil_production;
        let gas_error = (final_inventory.2 - initial_inventory.2) - integrated_gas_injection
            + integrated_gas_production;
        let final_rates = retried
            .rate_history
            .last()
            .expect("accepted retries must publish rates");
        assert!(
            (water_error.abs() - final_rates.material_balance_error_m3).abs()
                < initial_inventory.0 * 1e-10,
            "{flavor:?}: independently reconstructed water error {water_error:e} does not match \
             the once-committed ledger {:e}",
            final_rates.material_balance_error_m3
        );
        assert!(
            (oil_error.abs() - final_rates.material_balance_error_oil_m3).abs()
                < initial_inventory.1 * 1e-10,
            "{flavor:?}: independently reconstructed oil error {oil_error:e} does not match \
             the once-committed ledger {:e}",
            final_rates.material_balance_error_oil_m3
        );
        assert!(
            (gas_error.abs() - final_rates.material_balance_error_gas_m3).abs()
                < initial_inventory.2 * 1e-10,
            "{flavor:?}: independently reconstructed gas error {gas_error:e} does not match \
             the once-committed ledger {:e}",
            final_rates.material_balance_error_gas_m3
        );
        assert!(
            water_error.abs() / initial_inventory.0 < 1e-2
                && oil_error.abs() / initial_inventory.1 < 1e-2
                && gas_error.abs() / initial_inventory.2 < 1e-2,
            "{flavor:?}: fixture source/inventory error is too large: water {water_error:e}, \
             oil {oil_error:e}, gas {gas_error:e}"
        );
        assert!(
            integrated_water_production > 0.0
                && integrated_oil_production > 0.0
                && integrated_gas_production > 0.0,
            "{flavor:?}: fixture did not exercise nonzero water, oil and gas sources"
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
/// negative result here does not prove the shipped case is clean. It establishes only that both
/// backends remain physical and mutually consistent on the reconstruction. Because they share
/// the geometry, Peaceman PI and head-offset code, their agreement cannot exclude a common error.
fn gravity_section(fim: bool, fully_perforated: bool) -> ReservoirSimulator {
    let nz = 20;
    let mut sim = ReservoirSimulator::new(30, 1, nz, 0.2);
    sim.set_fim_enabled(fim);
    sim.set_cell_dimensions(10.0, 100.0, 2.0).unwrap();
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
/// FIM agree to about 1 % on the pressure envelope and to 5e-3 on peak water saturation. This
/// reconstruction does not reproduce #10; its cause remains inconclusive without an independent
/// geometry oracle or an exact replay.
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

        // This is a cross-backend consistency assertion, not an independent geometry oracle.
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
