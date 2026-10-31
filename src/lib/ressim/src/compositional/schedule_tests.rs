//! Declarative compositional well schedules (`comp_schedule_*`, #50).
//!
//! The gate #50 names is a 1-D WAG cycle: water and hydrocarbon injection alternate on absolute
//! times, every switch lands exactly on its time, and the component ledger closes across it. The
//! tests below use the same well source and timestep lifecycle the browser runs — only the payload
//! differs — so the schedule position is exercised through the public engine boundary rather than
//! through a test-only hook.

use super::api::{
    CASE_SCHEMA, CaseConfig, CompletionConfig, CompositionalCase, FluidChoice, GridConfig,
    RelPermConfig, SCHEDULE_CASE_SCHEMA, WaterConfig, WellChangeConfig, WellConfig,
    WellControlConfig, WellEventConfig, WellInjectionConfig,
};
use super::assembly::Face;
use super::flux::Gravity;
use super::relperm::RelativePermeabilityModel;
use super::state::{CompositionalCellState, CompositionalState, RockView};
use super::timestep::{CompositionalRun, TimestepOptions};
use super::water::{StoneTwoRelPerm, WaterModel, WaterProperties};
use crate::fluid::pinned;
use crate::relperm::{SgofRow, SwofRow};

/// A 1-D five-cell water/hydrocarbon column, with one injector and one producer.
fn wag_case() -> CaseConfig {
    CaseConfig {
        schema: SCHEDULE_CASE_SCHEMA.to_string(),
        fluid: FluidChoice::PinnedBinary,
        grid: GridConfig {
            cells: 5,
            dx_m: 20.0,
            dy_m: 10.0,
            dz_m: 10.0,
            porosity: 0.2,
            permeability_md: 100.0,
            rock_reference_pressure_bar: 200.0,
            rock_compressibility_per_bar: 0.0,
            box_grid: None,
        },
        relperm: RelPermConfig::StoneIi {
            swof: swof(&[(0.15, 0.0, 0.9), (0.6, 0.2, 0.1), (1.0, 0.6, 0.0)]),
            sgof: sgof(&[(0.0, 0.0, 0.9), (0.05, 0.0, 0.8), (0.85, 0.8, 0.0)]),
        },
        initial_pressure_bar: 200.0,
        initial_composition: vec![0.6, 0.4],
        wells: vec![
            WellConfig {
                id: "INJ".to_string(),
                completions: vec![CompletionConfig {
                    cell: 0,
                    well_index: 2.0,
                    head_offset_bar: 0.0,
                }],
                control: WellControlConfig::Bhp { target_bar: 260.0 },
                injection_composition: None,
                inject_water: true,
            },
            WellConfig {
                id: "PROD".to_string(),
                completions: vec![CompletionConfig {
                    cell: 4,
                    well_index: 2.0,
                    head_offset_bar: 0.0,
                }],
                control: WellControlConfig::Bhp { target_bar: 170.0 },
                injection_composition: None,
                inject_water: false,
            },
        ],
        schedule: Vec::new(),
        gravity_enabled: false,
        initial_pressure_gradient_bar_per_m: None,
        water: Some(WaterConfig {
            reference_pressure_bar: 200.0,
            formation_volume_factor: 1.02,
            compressibility_per_bar: 4.5e-5,
            viscosity_cp: 0.5,
            viscosibility_per_bar: 0.0,
            surface_density_kg_per_m3: 1020.0,
            initial_saturation: 0.15,
        }),
    }
}

fn swof(rows: &[(f64, f64, f64)]) -> Vec<SwofRow> {
    rows.iter()
        .map(|&(sw, krw, krow)| SwofRow {
            sw,
            krw,
            krow,
            pcow: None,
        })
        .collect()
}

fn sgof(rows: &[(f64, f64, f64)]) -> Vec<SgofRow> {
    rows.iter()
        .map(|&(sg, krg, krog)| SgofRow {
            sg,
            krg,
            krog,
            pcog: None,
        })
        .collect()
}

fn water_event(time_days: f64, _target_m3_per_day: f64) -> WellEventConfig {
    WellEventConfig {
        time_days,
        well: "INJ".to_string(),
        change: WellChangeConfig {
            control: Some(WellControlConfig::Bhp { target_bar: 260.0 }),
            injection: Some(WellInjectionConfig::Water),
        },
    }
}

fn gas_event(time_days: f64, _target_m3_per_day: f64) -> WellEventConfig {
    WellEventConfig {
        time_days,
        well: "INJ".to_string(),
        change: WellChangeConfig {
            control: Some(WellControlConfig::Bhp { target_bar: 250.0 }),
            injection: Some(WellInjectionConfig::Hydrocarbon {
                composition: vec![1.0, 0.0],
            }),
        },
    }
}

/// Water -> gas -> water -> gas -> shut-in -> open, on integer days.
fn wag_schedule() -> Vec<WellEventConfig> {
    vec![
        gas_event(1.0, 10.0),
        water_event(2.0, 5.0),
        gas_event(3.0, 10.0),
        WellEventConfig {
            time_days: 4.0,
            well: "INJ".to_string(),
            change: WellChangeConfig {
                control: Some(WellControlConfig::ShutIn),
                injection: None,
            },
        },
        water_event(5.0, 5.0),
    ]
}

/// The grid's total inventory per component and water last.
fn grid_ledger(snapshot: &super::api::Snapshot) -> Vec<f64> {
    let mut ledger = snapshot.inventory.clone();
    if let Some(water) = &snapshot.water {
        ledger.push(water.inventory_moles);
    }
    ledger
}

/// Every well's cumulative contribution, summed. Short rows are padded with zero.
fn well_ledger(snapshot: &super::api::Snapshot, width: usize) -> Vec<f64> {
    let mut total = vec![0.0; width];
    for well in &snapshot.cumulative_well_moles {
        for (i, value) in well.iter().enumerate() {
            total[i] += value;
        }
    }
    total
}

/// The difference between two per-well cumulative rows, by well and component.
fn well_delta(before: &[Vec<f64>], after: &[Vec<f64>], well: usize, component: usize) -> f64 {
    let at = |rows: &[Vec<f64>]| {
        rows.get(well)
            .and_then(|row| row.get(component))
            .copied()
            .unwrap_or(0.0)
    };
    at(after) - at(before)
}

/// A scheduled boundary can be much shorter than the normal minimum timestep. It must be
/// attempted, not turned into a `Budget` failure before Newton runs.
#[test]
fn comp_schedule_short_boundary_interval_is_attempted() {
    let mut config = wag_case();
    config.schedule = vec![gas_event(1.000_000_5, 10.0)];
    let mut case = CompositionalCase::new(config).unwrap();

    assert!(case.step(1.0).succeeded());
    assert_eq!(case.time_days(), 1.0);
    assert_eq!(case.checkpoint().schedule_position, Some(0));

    let outcome = case.step(1.0);
    assert!(
        outcome.succeeded(),
        "the 5e-7 d interval before the event failed: {:?}",
        outcome.failure
    );
    let accepted = outcome.accepted_dt_days.expect("accepted");
    assert!(
        (accepted - 5e-7).abs() < 1e-12,
        "accepted {accepted} d rather than the 5e-7 d boundary interval"
    );
    assert!(
        (case.time_days() - 1.000_000_5).abs() < 1e-12,
        "the clock is {}",
        case.time_days()
    );
    assert_eq!(case.checkpoint().schedule_position, Some(1));

    // And the event really is now in force: the next interval injects the gas stream.
    let before = case.snapshot().cumulative_well_moles.clone();
    let next = case.step(1.0);
    assert!(next.succeeded(), "{:?}", next.failure);
    let after = case.snapshot();
    assert!(
        well_delta(&before, &after.cumulative_well_moles, 0, 0) > 0.0,
        "the gas event did not govern the interval after the short boundary"
    );
}

/// The symmetric case: an event *before* the report time leaves a sub-minimum remainder after it.
/// That call is no longer capped by the event, so the boundary it starts from has to grant the
/// same allowance as an interval ending at an event.
#[test]
fn comp_schedule_short_remainder_after_event_is_attempted() {
    let mut config = wag_case();
    config.schedule = vec![gas_event(0.999_999_5, 10.0)];
    let mut case = CompositionalCase::new(config).unwrap();

    let first = case.step(1.0);
    assert!(first.succeeded(), "{:?}", first.failure);
    assert!(
        (first.accepted_dt_days.expect("accepted") - 0.999_999_5).abs() < 1e-12,
        "the first step did not stop at the event: {:?}",
        first.accepted_dt_days
    );
    assert_eq!(case.checkpoint().schedule_position, Some(1));

    // This is exactly the daily report remainder: target day 1 minus the event time.
    let before = case.snapshot().cumulative_well_moles.clone();
    let remainder = 1.0 - case.time_days();
    let outcome = case.step(remainder);
    assert!(
        outcome.succeeded(),
        "the {remainder:e} d remainder after the event failed: {:?}",
        outcome.failure
    );
    assert!(
        (outcome.accepted_dt_days.expect("accepted") - remainder).abs() < 1e-12,
        "accepted {:?} d rather than the report remainder",
        outcome.accepted_dt_days
    );
    assert!(
        (case.time_days() - 1.0).abs() < 1e-12,
        "the clock is {} after the remainder",
        case.time_days()
    );
    assert_eq!(case.checkpoint().schedule_position, Some(1));

    // The event was in force over the remainder, not deferred to the next report step.
    let after = case.snapshot();
    assert!(
        well_delta(&before, &after.cumulative_well_moles, 0, 0) > 0.0,
        "the gas event did not govern the remainder after it"
    );
}

/// A caller-supplied tiny step away from a schedule boundary is still refused before Newton.
#[test]
fn comp_schedule_a_caller_tiny_step_is_still_refused() {
    let config = wag_case();
    let mut case = CompositionalCase::new(config).unwrap();
    let outcome = case.step(5e-7);
    assert!(
        !outcome.succeeded(),
        "an unscheduled 5e-7 d caller step must not bypass the minimum timestep"
    );
    assert_eq!(outcome.attempts, 0);
    assert_eq!(outcome.failure.expect("failure").kind, "Budget");
}

/// **#50's gate.** Each WAG switch is exact in time, changes the prescribed stream, and the
/// inventory change equals the well ledger across the switch.
#[test]
fn comp_schedule_wag_cycle_switches_exactly_in_time_and_conserves_mass() {
    let mut config = wag_case();
    config.schedule = wag_schedule();
    let mut case = CompositionalCase::new(config).expect("the WAG case must build");

    let initial = grid_ledger(&case.snapshot());
    let width = initial.len();
    let mut injector_water = Vec::new();
    let mut injector_hydrocarbon = Vec::new();

    for (index, day) in [1.0, 2.0, 3.0, 4.0, 5.0].into_iter().enumerate() {
        let before = case.snapshot();
        let before_wells = before.cumulative_well_moles.clone();
        let outcome = case.step(1.0);
        assert!(
            outcome.succeeded(),
            "step {} to day {} failed: {:?}",
            index,
            day,
            outcome.failure
        );
        assert_eq!(
            outcome.accepted_dt_days,
            Some(1.0),
            "the step to a scheduled day must stop exactly on it"
        );
        assert_eq!(
            outcome.attempts, 1,
            "a WAG switch cut the timestep {day}; the switch is a new source, not a shorter step"
        );
        assert!(
            (case.time_days() - day).abs() < 1e-12,
            "the clock is {} at event {}",
            case.time_days(),
            day
        );
        assert_eq!(
            case.checkpoint().schedule_position,
            Some(index + 1),
            "the schedule cursor did not advance to event {}",
            index + 1
        );
        if day == 4.0 || day == 5.0 {
            // A checkpoint taken exactly at a shutdown or reopen boundary must restore, including
            // the zero flowing BHP an event boundary carries.
            let restored = CompositionalCase::restore(&case.checkpoint())
                .expect("an event-boundary checkpoint");
            assert_eq!(restored.checkpoint().schedule_position, Some(index + 1));
            assert_eq!(
                restored.snapshot().well_bhp_bar,
                case.snapshot().well_bhp_bar
            );
        }

        let after = case.snapshot();
        let inventory = grid_ledger(&after);
        let wells = well_ledger(&after, width);
        for component in 0..width {
            assert!(
                (inventory[component] - initial[component] - wells[component]).abs()
                    <= 1e-8 * initial[component].max(1.0),
                "component {component} after the switch to day {day}: inventory moved {}, \
                 wells moved {}",
                inventory[component] - initial[component],
                wells[component]
            );
        }

        let water = well_delta(&before_wells, &after.cumulative_well_moles, 0, 2);
        let gas = well_delta(&before_wells, &after.cumulative_well_moles, 0, 0);
        injector_water.push(water);
        injector_hydrocarbon.push(gas);
    }

    // Days 0-1 and 2-3 are water, 1-2 and 3-4 are gas, 4-5 are shut in.
    assert!(injector_water[0] > 0.0 && injector_hydrocarbon[0].abs() < 1e-9);
    assert!(injector_hydrocarbon[1] > 0.0 && injector_water[1].abs() < 1e-9);
    assert!(injector_water[2] > 0.0 && injector_hydrocarbon[2].abs() < 1e-9);
    assert!(injector_hydrocarbon[3] > 0.0 && injector_water[3].abs() < 1e-9);
    assert!(
        injector_water[4].abs() < 1e-9 && injector_hydrocarbon[4].abs() < 1e-9,
        "a shut-in injector moved water {} and hydrocarbon {}",
        injector_water[4],
        injector_hydrocarbon[4]
    );
    assert_eq!(
        case.snapshot().well_bhp_bar[0],
        0.0,
        "a shut-in well reports no flowing BHP"
    );

    // Open it again: the event at day 5 restores water injection, and the next interval moves it.
    let before = case.snapshot();
    let outcome = case.step(1.0);
    assert!(outcome.succeeded(), "{:?}", outcome.failure);
    let after = case.snapshot();
    assert!(
        well_delta(
            &before.cumulative_well_moles,
            &after.cumulative_well_moles,
            0,
            2
        ) > 0.0,
        "the reopened injector did not inject water"
    );
    assert!(
        after.well_bhp_bar[0] > 0.0,
        "a reopened well reports its solved BHP, not zero"
    );
}

/// **#50's restart criterion.** The checkpoint carries the schedule position, restore replays the
/// prefix to that clock, and both runs continue on the same trajectory.
#[test]
fn comp_schedule_checkpoint_carries_position_and_continues_identically() {
    let mut config = wag_case();
    config.schedule = wag_schedule();
    let mut original = CompositionalCase::new(config).unwrap();
    assert!(original.step(1.0).succeeded());
    assert!(original.step(0.5).succeeded());

    let checkpoint = original.checkpoint();
    assert_eq!(checkpoint.schedule_position, Some(1));
    assert_eq!(checkpoint.config.schema, SCHEDULE_CASE_SCHEMA);
    let json = serde_json::to_string(&checkpoint).unwrap();
    assert!(json.contains("\"schedule_position\":1"), "{json}");
    assert!(json.contains(SCHEDULE_CASE_SCHEMA), "{json}");
    assert!(json.contains("\"schedule\":["), "{json}");
    assert!(json.contains("\"time_days\":1.0"), "{json}");
    assert!(
        json.contains("\"injection\":{\"injection\":\"hydrocarbon\""),
        "the injected stream must cross as a tagged plain object: {json}"
    );

    let parsed: super::api::Checkpoint = serde_json::from_str(&json).unwrap();
    let mut restored = CompositionalCase::restore(&parsed).expect("the checkpoint must restore");
    assert_eq!(restored.time_days(), original.time_days());
    assert_eq!(restored.snapshot().pressure, original.snapshot().pressure);
    assert_eq!(
        restored.snapshot().composition,
        original.snapshot().composition
    );

    // The next call crosses a scheduled boundary, so its cap and its switch both have to survive.
    for _ in 0..3 {
        let a = original.step(10.0);
        let b = restored.step(10.0);
        assert!(
            a.succeeded() && b.succeeded(),
            "{:?} / {:?}",
            a.failure,
            b.failure
        );
        assert_eq!(a.accepted_dt_days, b.accepted_dt_days);
        assert!((a.time_days - b.time_days).abs() < 1e-12);
        assert_eq!(
            original.checkpoint().schedule_position,
            restored.checkpoint().schedule_position
        );
        assert_eq!(original.snapshot().pressure, restored.snapshot().pressure);
        assert_eq!(
            original.snapshot().composition,
            restored.snapshot().composition
        );
    }

    // A checkpoint written before the field existed replays by time instead of refusing.
    let mut old = parsed.clone();
    old.schedule_position = None;
    let mut replayed = CompositionalCase::restore(&old).expect("an old checkpoint must restore");
    assert_eq!(replayed.checkpoint().schedule_position, Some(1));
    assert!(replayed.step(0.5).succeeded());
    assert!((replayed.time_days() - 2.0).abs() < 1e-12);
    assert_eq!(replayed.checkpoint().schedule_position, Some(2));

    // A build without schedules accepts the checkpoint envelope and rejects its `/1` config only
    // after `Self::new` validates it. Simulate that config to show the schedule cannot survive as
    // an ignored field.
    let mut legacy_config = parsed.clone();
    legacy_config.config.schema = CASE_SCHEMA.to_string();
    assert!(
        CompositionalCase::restore(&legacy_config).is_err(),
        "a scheduled checkpoint under the legacy case schema was accepted"
    );
}

/// An event at `t = 0` is the initial condition, and a malformed schedule is refused before a run
/// is allocated rather than producing a different answer.
#[test]
fn comp_schedule_t0_applies_at_construction_and_bad_schedules_are_refused() {
    let mut config = wag_case();
    config.schedule = vec![gas_event(0.0, 10.0)];
    let mut case = CompositionalCase::new(config.clone()).unwrap();
    assert!(case.step(0.01).succeeded());
    let snapshot = case.snapshot();
    assert!(
        snapshot.cumulative_well_moles[0][0] > 0.0,
        "the t=0 gas event did not govern the first step"
    );
    assert!(
        snapshot.cumulative_well_moles[0][2].abs() < 1e-12,
        "the t=0 gas event left the water injection in force"
    );

    let error = |config: CaseConfig| match config.validate() {
        Ok(_) => panic!("the schedule must be refused"),
        Err(error) => error.to_string(),
    };

    let mut unsorted = config.clone();
    unsorted.schedule = vec![gas_event(2.0, 1.0), water_event(1.0, 1.0)];
    assert!(error(unsorted).contains("nondecreasing"));

    let mut duplicate = config.clone();
    duplicate.schedule = vec![gas_event(1.0, 1.0), gas_event(1.0, 2.0)];
    assert!(error(duplicate).contains("ambiguous"));

    // The duplicate is not adjacent: a second INJ after a PROD event at the same instant is just
    // as ambiguous, and the last one silently winning is the failure this guards.
    let mut interleaved = config.clone();
    let mut producer_event = gas_event(1.0, 1.0);
    producer_event.well = "PROD".to_string();
    producer_event.change = WellChangeConfig {
        control: Some(WellControlConfig::Bhp { target_bar: 170.0 }),
        injection: None,
    };
    interleaved.schedule = vec![gas_event(1.0, 1.0), producer_event, water_event(1.0, 5.0)];
    assert!(error(interleaved).contains("ambiguous"));

    // A scheduled payload must use /2: /1 is what a build without schedules accepts, and it
    // would ignore the events.
    let mut legacy_schema = config.clone();
    legacy_schema.schema = CASE_SCHEMA.to_string();
    assert!(
        error(legacy_schema).contains(SCHEDULE_CASE_SCHEMA),
        "a /1 payload with events was accepted"
    );

    let mut unknown_well = config.clone();
    unknown_well.schedule[0].well = "NO_SUCH_WELL".to_string();
    assert!(error(unknown_well).contains("no configured well"));

    let mut negative = config.clone();
    negative.schedule[0].time_days = -1.0;
    assert!(error(negative).contains("before the start"));

    let mut empty = config.clone();
    empty.schedule[0].change = WellChangeConfig {
        control: None,
        injection: None,
    };
    assert!(error(empty).contains("at least one"));

    let mut wrong_components = config.clone();
    wrong_components.schedule[0].change.injection = Some(WellInjectionConfig::Hydrocarbon {
        composition: vec![1.0, 0.0, 0.0],
    });
    assert!(error(wrong_components).contains("3 entries"));

    let mut duplicate_id = config.clone();
    duplicate_id.wells[1].id = "INJ".to_string();
    assert!(error(duplicate_id).contains("already used"));

    let mut empty_id = config.clone();
    empty_id.wells[0].id = " ".to_string();
    assert!(error(empty_id).contains("non-empty identifier"));

    let mut dry = config;
    dry.water = None;
    dry.relperm = RelPermConfig::Linear;
    dry.wells[0].inject_water = false;
    dry.wells[0].control = WellControlConfig::Bhp { target_bar: 220.0 };
    dry.schedule = vec![water_event(1.0, 5.0)];
    assert!(error(dry).contains("no water block"));
}

/// `ShutIn` is a control on the well itself, not a removed well: the source is exactly zero and the
/// per-well cumulative row keeps its place.
#[test]
fn comp_schedule_shut_in_has_no_source_and_keeps_the_well_row() {
    let mut config = wag_case();
    config.schedule = vec![WellEventConfig {
        time_days: 0.0,
        well: "INJ".to_string(),
        change: WellChangeConfig {
            control: Some(WellControlConfig::ShutIn),
            injection: None,
        },
    }];
    let mut case = CompositionalCase::new(config).unwrap();
    assert!(case.step(0.25).succeeded());
    let snapshot = case.snapshot();
    assert_eq!(snapshot.cumulative_well_moles.len(), 2);
    assert_eq!(snapshot.cumulative_well_moles[0], vec![0.0, 0.0, 0.0]);
    assert_eq!(snapshot.well_bhp_bar[0], 0.0);
    assert!(snapshot.cumulative_well_moles[1][0] < 0.0);
}

/// The schedule semantics are a property of the engine, not only of the API test fixtures: a run
/// driven directly through [`CompositionalRun`] sees the same zero source when its well is shut in.
#[test]
fn comp_schedule_shut_in_zeroes_the_source_the_run_uses() {
    let spec = pinned::binary().unwrap();
    let layout = super::layout::CompositionalLayout::new(2, 3, 1, 1)
        .unwrap()
        .with_water();
    let pressure = [200.0, 195.0, 190.0];
    let state = CompositionalState::new(
        &layout,
        pressure
            .iter()
            .map(|&p| CompositionalCellState::with_water(p, vec![0.6], 0.15).unwrap())
            .collect(),
    )
    .unwrap();
    let relperm = RelativePermeabilityModel::StoneTwoWater(Box::new(WaterModel {
        properties: WaterProperties::new(200.0, 1.02, 4.5e-5, 0.5, 0.0, 1020.0).unwrap(),
        relperm: StoneTwoRelPerm::new(
            swof(&[(0.0, 0.0, 1.0), (1.0, 1.0, 0.0)]),
            sgof(&[(0.0, 0.0, 1.0), (1.0, 1.0, 0.0)]),
        )
        .unwrap(),
    }));
    let mut shut_in =
        super::wells::CompositionalWell::single("INJ", 0, 2.0, super::wells::WellControl::ShutIn);
    shut_in.injection = Some(super::wells::InjectedFluid::Water);
    let producer = super::wells::CompositionalWell::single(
        "PROD",
        2,
        2.0,
        super::wells::WellControl::Bhp { target_bar: 150.0 },
    );
    let wells = [shut_in, producer];
    let face = Face {
        cell_i: 0,
        cell_j: 1,
        geom_t: 8.526_988_8e-3 * 100.0 * 100.0 / 20.0,
        gravity: Gravity::OFF,
    };
    let face_2 = Face {
        cell_i: 1,
        cell_j: 2,
        ..face
    };
    let mut run = CompositionalRun::new(state);
    let sources = vec![vec![0.0; 3]; 3];
    let report = run.step(
        &spec,
        &layout,
        &RockView {
            pore_volume_ref_m3: &[200.0; 3],
            reference_pressure_bar: 200.0,
            compressibility_per_bar: 0.0,
        },
        &relperm,
        &[face, face_2],
        &wells,
        &sources,
        0.1,
        TimestepOptions::default(),
    );
    assert!(report.succeeded(), "{:?}", report.failure);
    assert_eq!(run.cumulative_well_moles()[0], vec![0.0, 0.0, 0.0]);
    assert_eq!(run.well_bhp_bar()[0], 0.0);
}
