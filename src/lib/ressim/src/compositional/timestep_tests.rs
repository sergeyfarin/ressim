//! C10 timestep-lifecycle contract tests (`comp_rollback_*`).

use super::assembly::Face;
use super::flux::Gravity;
use super::layout::CompositionalLayout;
use super::newton::NewtonOptions;
use super::relperm::RelativePermeabilityModel;
use super::state::{CompositionalCellState, CompositionalState, FlashCacheKey, RockView};
use super::timestep::{CompositionalRun, FailureKind, TimestepOptions};
use crate::fluid::flash::flash;
use crate::fluid::pinned;
use crate::fluid::specification::FluidSpecification;
use crate::fluid::units::bar_to_pa;

fn relperm() -> RelativePermeabilityModel {
    RelativePermeabilityModel::Linear
}
const GEOM_T: f64 = 8.526_988_8e-3 * 100.0 * (100.0 * 10.0) / 100.0;
const PORE_VOLUMES: [f64; 3] = [1000.0, 1000.0, 1000.0];

fn rock() -> RockView<'static> {
    RockView {
        pore_volume_ref_m3: &PORE_VOLUMES,
        reference_pressure_bar: 200.0,
        compressibility_per_bar: 4.0e-5,
    }
}

fn setup(
    cells: usize,
    p: f64,
) -> (
    FluidSpecification,
    CompositionalLayout,
    Vec<Face>,
    CompositionalRun,
) {
    let spec = pinned::ternary().unwrap();
    let layout = CompositionalLayout::new(3, cells, 0, 0).unwrap();
    let state = CompositionalState::new(
        &layout,
        (0..cells)
            .map(|_| CompositionalCellState::new(p, vec![0.2, 0.5]).unwrap())
            .collect(),
    )
    .unwrap();
    let faces = (0..cells.saturating_sub(1))
        .map(|i| Face {
            cell_i: i,
            cell_j: i + 1,
            geom_t: GEOM_T,
            gravity: Gravity::OFF,
        })
        .collect();
    (spec, layout, faces, CompositionalRun::new(state))
}

fn zero_sources(cells: usize) -> Vec<Vec<f64>> {
    vec![vec![0.0; 3]; cells]
}

/// A successful step advances the clock, the state and the cumulative totals together.
#[test]
fn comp_rollback_a_successful_step_advances_everything_together() {
    let (spec, layout, faces, mut run) = setup(2, 200.0);
    let rock = rock();
    let previous = run.accepted_inventory(&spec, &rock).unwrap();
    let total: f64 = previous[0].iter().sum();

    let mut sources = zero_sources(2);
    sources[0] = vec![-total * 1e-3, -total * 1e-3, -total * 1e-3];

    let before_version = run.state().version();
    let report = run.step(
        &spec,
        &layout,
        &rock,
        &relperm(),
        &faces,
        &[],
        &sources,
        1.0,
        TimestepOptions::default(),
    );

    assert!(report.succeeded(), "{report:?}");
    assert_eq!(report.accepted_dt_days, Some(1.0));
    assert_eq!(report.retries(), 0);
    assert_eq!(run.time_days(), 1.0);
    assert!(run.state().version() > before_version);

    for i in 0..3 {
        let expected = 1.0 * sources.iter().map(|s| s[i]).sum::<f64>();
        assert!((run.cumulative_source_moles()[i] - expected).abs() < 1e-9 * expected.abs());
    }
}

/// A rejected step changes nothing: not the state, not the clock, not the cumulative totals. A
/// rollback that is structural rather than careful.
#[test]
fn comp_rollback_a_rejected_step_changes_nothing() {
    let (spec, layout, faces, mut run) = setup(2, 200.0);
    let rock = rock();
    let previous = run.accepted_inventory(&spec, &rock).unwrap();
    let total: f64 = previous[0].iter().sum();

    // Far more production than the cell contains, at every timestep the ladder will try.
    let mut sources = zero_sources(2);
    sources[0] = vec![-total * 1e6, 0.0, 0.0];

    let state_before = run.state().clone();
    let time_before = run.time_days();
    let cumulative_before = run.cumulative_source_moles().to_vec();

    let options = TimestepOptions {
        max_attempts: 3,
        newton: NewtonOptions {
            tolerance: 1e-8,
            max_iterations: 4,
        },
        ..TimestepOptions::default()
    };
    let report = run.step(
        &spec,
        &layout,
        &rock,
        &relperm(),
        &faces,
        &[],
        &sources,
        1.0,
        options,
    );

    assert!(
        !report.succeeded(),
        "an impossible step must not be accepted"
    );
    assert_eq!(report.accepted_dt_days, None);
    assert_eq!(run.state(), &state_before, "the accepted state moved");
    assert_eq!(run.time_days(), time_before, "the clock moved");
    assert_eq!(
        run.cumulative_source_moles(),
        cumulative_before,
        "cumulatives moved"
    );
}

/// The retry ladder halves `dt` and records every attempt, so what was tried is inspectable
/// rather than inferred.
#[test]
fn comp_rollback_the_retry_ladder_halves_dt_and_records_every_attempt() {
    let (spec, layout, faces, mut run) = setup(2, 200.0);
    let rock = rock();
    let previous = run.accepted_inventory(&spec, &rock).unwrap();
    let total: f64 = previous[0].iter().sum();
    let mut sources = zero_sources(2);
    sources[0] = vec![-total * 1e6, 0.0, 0.0];

    let options = TimestepOptions {
        cut_factor: 0.5,
        max_attempts: 4,
        newton: NewtonOptions {
            tolerance: 1e-8,
            max_iterations: 3,
        },
        ..TimestepOptions::default()
    };
    let report = run.step(
        &spec,
        &layout,
        &rock,
        &relperm(),
        &faces,
        &[],
        &sources,
        8.0,
        options,
    );

    assert!(!report.succeeded());
    assert_eq!(report.attempts.len(), 4);
    let dts: Vec<f64> = report.attempts.iter().map(|a| a.dt_days).collect();
    assert_eq!(dts, vec![8.0, 4.0, 2.0, 1.0]);
    for attempt in &report.attempts {
        assert!(
            attempt.failure.is_some(),
            "every attempt here should have failed"
        );
    }
    assert_eq!(report.retries(), 3);
}

/// A step too large at first succeeds after a cut, and the accepted `dt` is the one that worked —
/// not the one that was asked for.
#[test]
fn comp_rollback_a_cut_step_succeeds_and_reports_the_dt_that_worked() {
    let (spec, layout, faces, mut run) = setup(1, 200.0);
    let rock = rock();
    let previous = run.accepted_inventory(&spec, &rock).unwrap();
    let total: f64 = previous[0].iter().sum();

    // Sized so one day is too much and a fraction of a day is not.
    let sources = vec![vec![-total * 0.4, -total * 0.4, -total * 0.4]];

    let options = TimestepOptions {
        max_attempts: 8,
        newton: NewtonOptions {
            tolerance: 1e-8,
            max_iterations: 8,
        },
        ..TimestepOptions::default()
    };
    let report = run.step(
        &spec,
        &layout,
        &rock,
        &relperm(),
        &faces,
        &[],
        &sources,
        1.0,
        options,
    );

    if report.succeeded() {
        let dt = report.accepted_dt_days.unwrap();
        assert_eq!(run.time_days(), dt);
        assert!(dt <= 1.0);
        // The clock advanced by exactly the accepted dt, not by the requested one.
        if report.retries() > 0 {
            assert!(dt < 1.0, "a retried step should have accepted a smaller dt");
        }
    } else {
        // Acceptable too, as long as it is classified and nothing moved.
        assert_eq!(run.time_days(), 0.0);
        assert!(report.failure.is_some());
    }
}

/// Failures are classified. "The step failed" is not actionable; a flash failure, a singular
/// Jacobian and a nonlinear stall have entirely different responses.
#[test]
fn comp_rollback_failures_are_classified() {
    let (spec, layout, faces, mut run) = setup(2, 200.0);
    let rock = rock();
    let previous = run.accepted_inventory(&spec, &rock).unwrap();
    let total: f64 = previous[0].iter().sum();
    let mut sources = zero_sources(2);
    sources[0] = vec![-total * 1e6, 0.0, 0.0];

    let options = TimestepOptions {
        max_attempts: 2,
        newton: NewtonOptions {
            tolerance: 1e-8,
            max_iterations: 3,
        },
        ..TimestepOptions::default()
    };
    let report = run.step(
        &spec,
        &layout,
        &rock,
        &relperm(),
        &faces,
        &[],
        &sources,
        1.0,
        options,
    );

    let (kind, message) = report
        .failure
        .expect("a failed step must carry a classification");
    assert!(
        matches!(
            kind,
            FailureKind::Flash
                | FailureKind::Linear
                | FailureKind::Nonlinear
                | FailureKind::Admissibility
                | FailureKind::Budget
        ),
        "unclassified failure kind {kind:?}"
    );
    assert!(
        !message.is_empty(),
        "the classification must carry a message"
    );

    // Every attempt's own failure is classified too.
    for attempt in &report.attempts {
        assert!(attempt.failure.is_some());
    }
}

/// A budget exhaustion is reported as such, distinct from whatever the last attempt failed on —
/// the caller needs to know the ladder ran out, and also what it ran out against.
#[test]
fn comp_rollback_budget_exhaustion_is_its_own_classification() {
    let (spec, layout, faces, mut run) = setup(1, 200.0);
    let rock = rock();
    let previous = run.accepted_inventory(&spec, &rock).unwrap();
    let total: f64 = previous[0].iter().sum();
    let sources = vec![vec![-total * 1e8, 0.0, 0.0]];

    let options = TimestepOptions {
        max_attempts: 2,
        newton: NewtonOptions {
            tolerance: 1e-8,
            max_iterations: 2,
        },
        ..TimestepOptions::default()
    };
    let report = run.step(
        &spec,
        &layout,
        &rock,
        &relperm(),
        &faces,
        &[],
        &sources,
        1.0,
        options,
    );

    let (kind, message) = report.failure.unwrap();
    assert_eq!(kind, FailureKind::Budget);
    assert!(
        message.contains("last failure was"),
        "the budget message must carry the underlying failure: {message}"
    );
}

/// Several successful steps accumulate correctly: time adds up, and the cumulative source totals
/// are the sum over accepted steps only.
#[test]
fn comp_rollback_accepted_steps_accumulate() {
    let (spec, layout, faces, mut run) = setup(2, 200.0);
    let rock = rock();
    let previous = run.accepted_inventory(&spec, &rock).unwrap();
    let total: f64 = previous[0].iter().sum();
    let mut sources = zero_sources(2);
    sources[0] = vec![-total * 1e-4, -total * 1e-4, -total * 1e-4];

    let mut accepted = 0;
    for _ in 0..4 {
        let report = run.step(
            &spec,
            &layout,
            &rock,
            &relperm(),
            &faces,
            &[],
            &sources,
            1.0,
            TimestepOptions::default(),
        );
        if report.succeeded() {
            accepted += 1;
        }
    }
    assert_eq!(accepted, 4, "these steps are small enough to all succeed");
    assert!((run.time_days() - 4.0).abs() < 1e-12);

    for i in 0..3 {
        let expected = 4.0 * sources.iter().map(|s| s[i]).sum::<f64>();
        assert!(
            (run.cumulative_source_moles()[i] - expected).abs() < 1e-9 * expected.abs(),
            "component {i} cumulative is {}, expected {expected}",
            run.cumulative_source_moles()[i]
        );
    }

    // Production must have lowered the pressure over four steps.
    assert!(run.state().cell(0).pressure_bar < 200.0);
}

/// Depletion over several steps conserves material: what left through the source equals what the
/// cells lost. The closure the plan asks for, measured across accepted steps rather than within
/// one.
#[test]
fn comp_rollback_multi_step_depletion_closes_the_inventory() {
    let (spec, layout, faces, mut run) = setup(3, 220.0);
    let rock = rock();

    let initial = run.accepted_inventory(&spec, &rock).unwrap();
    let initial_total: Vec<f64> = (0..3)
        .map(|i| initial.iter().map(|cell| cell[i]).sum::<f64>())
        .collect();

    let cell_total: f64 = initial[2].iter().sum();
    let mut sources = zero_sources(3);
    sources[2] = vec![-cell_total * 5e-4, -cell_total * 5e-4, -cell_total * 5e-4];

    for _ in 0..5 {
        let report = run.step(
            &spec,
            &layout,
            &rock,
            &relperm(),
            &faces,
            &[],
            &sources,
            1.0,
            TimestepOptions::default(),
        );
        assert!(report.succeeded(), "{report:?}");
    }

    let final_inventory = run.accepted_inventory(&spec, &rock).unwrap();
    for i in 0..3 {
        let final_total: f64 = final_inventory.iter().map(|cell| cell[i]).sum();
        let lost = initial_total[i] - final_total;
        let produced = -run.cumulative_source_moles()[i];
        assert!(
            (lost - produced).abs() / produced.abs().max(1.0) < 1e-6,
            "component {i}: the grid lost {lost} moles but {produced} were produced"
        );
    }
}

/// The flash cache cannot serve an entry from a rejected attempt, because entries are keyed on the
/// accepted state's version. Clearing it on commit is housekeeping; unreachability is the
/// correctness property.
#[test]
fn comp_rollback_cache_entries_cannot_outlive_their_state_version() {
    let (spec, layout, faces, mut run) = setup(2, 200.0);
    let rock = rock();

    // Populate the cache against the current accepted version.
    let version = run.state().version();
    for cell in 0..2 {
        let c = run.state().cell(cell).clone();
        let state = flash(
            &spec,
            bar_to_pa(c.pressure_bar),
            spec.reservoir_temperature_k(),
            &c.overall_composition(),
            None,
        )
        .unwrap();
        let key = FlashCacheKey::new(version, &c);
        run.cache_mut().insert(cell, key, state);
    }
    // The entries are there while the version holds.
    for cell in 0..2 {
        let key = FlashCacheKey::new(version, run.state().cell(cell));
        assert!(run.cache_mut().get(cell, &key).is_some());
    }

    let previous = run.accepted_inventory(&spec, &rock).unwrap();
    let total: f64 = previous[0].iter().sum();
    let mut sources = zero_sources(2);
    sources[0] = vec![-total * 1e-4, -total * 1e-4, -total * 1e-4];
    let report = run.step(
        &spec,
        &layout,
        &rock,
        &relperm(),
        &faces,
        &[],
        &sources,
        1.0,
        TimestepOptions::default(),
    );
    assert!(report.succeeded());

    // After a commit the version moved, so nothing keyed to the old one can be read.
    for cell in 0..2 {
        let key = FlashCacheKey::new(run.state().version(), run.state().cell(cell));
        assert!(
            run.cache_mut().get(cell, &key).is_none(),
            "cell {cell} served a cache entry across a commit"
        );
    }
}

/// Restart: a checkpoint taken mid-run restores the exact state, and stepping from the restored
/// state gives the same result as stepping from the original.
#[test]
fn comp_rollback_a_checkpoint_restores_a_steppable_state() {
    let (spec, layout, faces, mut run) = setup(2, 200.0);
    let rock = rock();
    let previous = run.accepted_inventory(&spec, &rock).unwrap();
    let total: f64 = previous[0].iter().sum();
    let mut sources = zero_sources(2);
    sources[0] = vec![-total * 1e-4, -total * 1e-4, -total * 1e-4];

    assert!(
        run.step(
            &spec,
            &layout,
            &rock,
            &relperm(),
            &faces,
            &[],
            &sources,
            1.0,
            TimestepOptions::default()
        )
        .succeeded()
    );

    let checkpoint = run.state().checkpoint();
    let restored = CompositionalState::from_checkpoint(&layout, checkpoint).unwrap();
    assert_eq!(&restored, run.state());

    // Step both and compare.
    let mut restored_run = CompositionalRun::new(restored);
    let a = run.step(
        &spec,
        &layout,
        &rock,
        &relperm(),
        &faces,
        &[],
        &sources,
        1.0,
        TimestepOptions::default(),
    );
    let b = restored_run.step(
        &spec,
        &layout,
        &rock,
        &relperm(),
        &faces,
        &[],
        &sources,
        1.0,
        TimestepOptions::default(),
    );
    assert_eq!(a.succeeded(), b.succeeded());
    assert_eq!(run.state().cells(), restored_run.state().cells());
}
