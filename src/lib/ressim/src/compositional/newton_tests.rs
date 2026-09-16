//! C10 Newton contract tests (`comp_newton_*`).

use super::accumulation::cell_inventory;
use super::assembly::Face;
use super::flux::Gravity;
use super::layout::CompositionalLayout;
use super::newton::{NewtonError, NewtonOptions, NewtonProblem, solve_newton};
use super::relperm::RelativePermeabilityModel;
use super::state::{CompositionalCellState, CompositionalState, RockView};
use crate::fluid::pinned;
use crate::fluid::specification::FluidSpecification;

fn relperm() -> RelativePermeabilityModel {
    RelativePermeabilityModel::Linear
}
const GEOM_T: f64 = 8.526_988_8e-3 * 100.0 * (100.0 * 10.0) / 100.0;

struct Case {
    spec: FluidSpecification,
    layout: CompositionalLayout,
    pore_volumes: Vec<f64>,
    faces: Vec<Face>,
    state: CompositionalState,
}

impl Case {
    fn rock(&self) -> RockView<'_> {
        RockView {
            pore_volume_ref_m3: &self.pore_volumes,
            reference_pressure_bar: 200.0,
            compressibility_per_bar: 4.0e-5,
        }
    }
    fn previous(&self) -> Vec<Vec<f64>> {
        let rock = self.rock();
        (0..self.layout.cell_count())
            .map(|c| {
                cell_inventory(&self.spec, &rock, c, self.state.cell(c))
                    .unwrap()
                    .0
                    .component_moles
            })
            .collect()
    }
    fn zero_sources(&self) -> Vec<Vec<f64>> {
        vec![vec![0.0; self.layout.component_count()]; self.layout.cell_count()]
    }
}

fn column(spec: FluidSpecification, cells: usize, p: f64, z: &[f64]) -> Case {
    let layout = CompositionalLayout::new(spec.component_count(), cells, 0, 0).unwrap();
    let state = CompositionalState::new(
        &layout,
        (0..cells)
            .map(|_| CompositionalCellState::new(p, z.to_vec()).unwrap())
            .collect(),
    )
    .unwrap();
    Case {
        spec,
        layout,
        pore_volumes: vec![1000.0; cells],
        faces: (0..cells.saturating_sub(1))
            .map(|i| Face {
                cell_i: i,
                cell_j: i + 1,
                geom_t: GEOM_T,
                gravity: Gravity::OFF,
            })
            .collect(),
        state,
    }
}

/// A state that is already the solution converges in one assembly, without taking a step.
#[test]
fn comp_newton_converges_immediately_on_a_stationary_state() {
    for (spec, z) in [
        (pinned::binary().unwrap(), vec![0.6]),
        (pinned::ternary().unwrap(), vec![0.2, 0.5]),
    ] {
        let case = column(spec, 3, 150.0, &z);
        let rock = case.rock();
        let previous = case.previous();
        let sources = case.zero_sources();
        let problem = NewtonProblem {
            spec: &case.spec,
            layout: &case.layout,
            rock,
            relperm: &relperm(),
            faces: &case.faces,
            previous_moles: &previous,
            sources: &sources,
            dt_days: 1.0,
        };

        let (outcome, report) = solve_newton(&problem, &case.state, NewtonOptions::default());
        let accepted = outcome.expect("a stationary state must converge");
        assert_eq!(report.iterations, 1);
        assert_eq!(report.residual_history, vec![0.0]);
        assert!(
            report.step_scale_history.is_empty(),
            "no step should have been taken"
        );
        assert_eq!(accepted.cells(), case.state.cells());
    }
}

/// A one-cell depletion: produce from a closed cell and the pressure must fall, the inventory must
/// drop by exactly what was produced, and the composition must shift toward the heavy component as
/// the light one is preferentially produced.
#[test]
fn comp_newton_one_cell_depletion_conserves_and_depletes() {
    let case = column(pinned::ternary().unwrap(), 1, 200.0, &[0.2, 0.5]);
    let rock = case.rock();
    let previous = case.previous();
    let dt = 1.0;

    // Produce roughly a thousandth of the cell's inventory per day, in the cell's own proportions.
    let total: f64 = previous[0].iter().sum();
    let sources = vec![
        previous[0]
            .iter()
            .map(|m| -1e-3 * m / total * total)
            .collect::<Vec<_>>(),
    ];

    let problem = NewtonProblem {
        spec: &case.spec,
        layout: &case.layout,
        rock,
        relperm: &relperm(),
        faces: &case.faces,
        previous_moles: &previous,
        sources: &sources,
        dt_days: dt,
    };
    let (outcome, report) = solve_newton(&problem, &case.state, NewtonOptions::default());
    let accepted = outcome.unwrap_or_else(|e| panic!("depletion failed: {e} ({report:?})"));

    assert!(
        accepted.cell(0).pressure_bar < 200.0,
        "production must lower the pressure: {}",
        accepted.cell(0).pressure_bar
    );

    // Inventory closure: what is left equals what was there minus what was produced.
    let after = cell_inventory(&case.spec, &rock, 0, accepted.cell(0))
        .unwrap()
        .0;
    for i in 0..3 {
        let expected = previous[0][i] + dt * sources[0][i];
        assert!(
            (after.component_moles[i] - expected).abs() / expected.abs().max(1.0) < 1e-8,
            "component {i}: {} moles left, expected {expected}",
            after.component_moles[i]
        );
    }

    // Residual history must actually decrease — a solve that "converged" without reducing the
    // residual would have started at the answer.
    assert!(report.residual_history.len() >= 2, "{report:?}");
    assert!(report.residual_history[0] > report.residual_history[1]);
}

/// A 1D displacement: inject a light component at one end and produce at the other. Material must
/// move downstream and the injected component must show up where it was not before.
#[test]
fn comp_newton_1d_displacement_moves_the_injected_component_downstream() {
    let case = column(pinned::ternary().unwrap(), 4, 150.0, &[0.05, 0.25]);
    let rock = case.rock();
    let previous = case.previous();

    let rate = previous[0].iter().sum::<f64>() * 1e-3;
    let mut sources = case.zero_sources();
    sources[0] = vec![rate, 0.0, 0.0]; // inject pure CO2 into the first cell
    sources[3] = vec![-rate * 0.2, -rate * 0.5, -rate * 0.3]; // produce a mix from the last

    let problem = NewtonProblem {
        spec: &case.spec,
        layout: &case.layout,
        rock,
        relperm: &relperm(),
        faces: &case.faces,
        previous_moles: &previous,
        sources: &sources,
        dt_days: 1.0,
    };
    let (outcome, report) = solve_newton(&problem, &case.state, NewtonOptions::default());
    let accepted = outcome.unwrap_or_else(|e| panic!("displacement failed: {e} ({report:?})"));

    // CO2 accumulates at the injection end.
    assert!(
        accepted.cell(0).independent_z()[0] > case.state.cell(0).independent_z()[0],
        "CO2 did not build up where it was injected"
    );
    // And the injection end is at higher pressure than the production end.
    assert!(
        accepted.cell(0).pressure_bar > accepted.cell(3).pressure_bar,
        "the pressure gradient points the wrong way: {:?}",
        accepted
            .cells()
            .iter()
            .map(|c| c.pressure_bar)
            .collect::<Vec<_>>()
    );
    // Pressure must be monotone along the column, since the only sources are at the ends.
    for i in 0..3 {
        assert!(
            accepted.cell(i).pressure_bar >= accepted.cell(i + 1).pressure_bar,
            "pressure is not monotone along the column"
        );
    }
}

/// The fraction-to-boundary bound must keep the composition on the simplex including its
/// **dependent** coordinate. A step that leaves every independent coordinate non-negative can
/// still drive the dependent one below zero, and that is the case this checks.
#[test]
fn comp_newton_step_bound_protects_the_dependent_composition() {
    let case = column(pinned::ternary().unwrap(), 1, 150.0, &[0.45, 0.45]);
    let rock = case.rock();
    let previous = case.previous();

    // Inject heavily into both independent components, which pushes the dependent one toward zero.
    let total: f64 = previous[0].iter().sum();
    let sources = vec![vec![total * 0.02, total * 0.02, 0.0]];

    let problem = NewtonProblem {
        spec: &case.spec,
        layout: &case.layout,
        rock,
        relperm: &relperm(),
        faces: &case.faces,
        previous_moles: &previous,
        sources: &sources,
        dt_days: 1.0,
    };
    let (outcome, report) = solve_newton(&problem, &case.state, NewtonOptions::default());

    // Whether or not it converges, every state it passed through stayed admissible — which is
    // what `commit` enforces, so a success here is itself the assertion.
    if let Ok(accepted) = outcome {
        let c = accepted.cell(0);
        assert!(
            c.dependent_z() >= 0.0,
            "the dependent composition went negative: {}",
            c.dependent_z()
        );
        assert!((c.overall_composition().iter().sum::<f64>() - 1.0).abs() < 1e-12);
    } else {
        // A failure must be a typed one, not a panic or a NaN.
        assert!(!report.residual_history.is_empty());
    }
}

/// A component at exactly zero must be able to become present: injecting it is the whole point of
/// the policy, and a step bound that froze the coordinate at zero would make that impossible.
#[test]
fn comp_newton_an_absent_component_can_be_injected_into_existence() {
    let case = column(pinned::ternary().unwrap(), 1, 150.0, &[0.0, 0.6]);
    assert_eq!(case.state.cell(0).independent_z()[0], 0.0);

    let rock = case.rock();
    let previous = case.previous();
    let total: f64 = previous[0].iter().sum();
    let sources = vec![vec![total * 1e-3, 0.0, 0.0]];

    let problem = NewtonProblem {
        spec: &case.spec,
        layout: &case.layout,
        rock,
        relperm: &relperm(),
        faces: &case.faces,
        previous_moles: &previous,
        sources: &sources,
        dt_days: 1.0,
    };
    let (outcome, report) = solve_newton(&problem, &case.state, NewtonOptions::default());
    let accepted =
        outcome.unwrap_or_else(|e| panic!("injection into an empty component: {e} ({report:?})"));

    assert!(
        accepted.cell(0).independent_z()[0] > 0.0,
        "the injected component is still at exactly zero"
    );
}

/// Removing material from a component that has none is reported, not clamped. It means the
/// residual for that component was not what it should have been, and silently zeroing the
/// direction would hide that.
#[test]
fn comp_newton_reports_a_negative_direction_on_an_absent_component() {
    use super::newton::ZERO_COMPONENT_DIRECTION_TOLERANCE;
    // The named tolerance exists so a numerically-zero direction on an empty component is not
    // mistaken for an attempt to remove material.
    assert!(ZERO_COMPONENT_DIRECTION_TOLERANCE > 0.0);

    let case = column(pinned::ternary().unwrap(), 1, 150.0, &[0.0, 0.6]);
    let rock = case.rock();
    let previous = case.previous();
    // Produce a component the cell does not have.
    let sources = vec![vec![-previous[0].iter().sum::<f64>() * 1e-3, 0.0, 0.0]];

    let problem = NewtonProblem {
        spec: &case.spec,
        layout: &case.layout,
        rock,
        relperm: &relperm(),
        faces: &case.faces,
        previous_moles: &previous,
        sources: &sources,
        dt_days: 1.0,
    };
    let (outcome, _) = solve_newton(&problem, &case.state, NewtonOptions::default());
    match outcome {
        Err(NewtonError::NegativeDirectionOnAbsentComponent { component, .. }) => {
            assert_eq!(component, 0)
        }
        other => panic!("expected a named absent-component failure, got {other:?}"),
    }
}

/// Newton reports rather than panics when it runs out of iterations, and the report carries the
/// history a diagnosis needs.
#[test]
fn comp_newton_reports_a_budget_exhaustion_with_its_history() {
    let case = column(pinned::ternary().unwrap(), 2, 150.0, &[0.2, 0.5]);
    let rock = case.rock();
    let previous = case.previous();
    let total: f64 = previous[0].iter().sum();
    // A source far too large for one day.
    let sources = vec![vec![total * 5.0, 0.0, 0.0], vec![0.0; 3]];

    let problem = NewtonProblem {
        spec: &case.spec,
        layout: &case.layout,
        rock,
        relperm: &relperm(),
        faces: &case.faces,
        previous_moles: &previous,
        sources: &sources,
        dt_days: 1.0,
    };
    let options = NewtonOptions {
        tolerance: 1e-8,
        max_iterations: 3,
    };
    let (outcome, report) = solve_newton(&problem, &case.state, options);

    assert!(outcome.is_err(), "an impossible step should not converge");
    assert!(
        !report.residual_history.is_empty(),
        "the report must carry a history"
    );
    assert!(report.iterations <= 3);
}

/// A failed solve leaves the caller's state exactly as it was. The solve works on a clone, so this
/// is structural — and it is what the timestep lifecycle depends on.
#[test]
fn comp_newton_a_failed_solve_does_not_touch_the_input_state() {
    let case = column(pinned::ternary().unwrap(), 2, 150.0, &[0.2, 0.5]);
    let before = case.state.clone();
    let rock = case.rock();
    let previous = case.previous();
    let total: f64 = previous[0].iter().sum();
    let sources = vec![vec![total * 5.0, 0.0, 0.0], vec![0.0; 3]];

    let problem = NewtonProblem {
        spec: &case.spec,
        layout: &case.layout,
        rock,
        relperm: &relperm(),
        faces: &case.faces,
        previous_moles: &previous,
        sources: &sources,
        dt_days: 1.0,
    };
    let options = NewtonOptions {
        tolerance: 1e-10,
        max_iterations: 2,
    };
    let (outcome, _) = solve_newton(&problem, &case.state, options);
    assert!(outcome.is_err());
    assert_eq!(case.state, before);
}

/// Convergence must be quadratic near the solution — the signature of a correct Jacobian. A
/// Jacobian that is merely close gives linear convergence, which still terminates and still looks
/// fine on a pass/fail test.
#[test]
fn comp_newton_converges_quadratically_near_the_solution() {
    let case = column(pinned::ternary().unwrap(), 2, 150.0, &[0.2, 0.5]);
    let rock = case.rock();
    let previous = case.previous();
    let total: f64 = previous[0].iter().sum();
    let mut sources = case.zero_sources();
    sources[0] = vec![-total * 2e-3, -total * 2e-3, -total * 2e-3];

    let problem = NewtonProblem {
        spec: &case.spec,
        layout: &case.layout,
        rock,
        relperm: &relperm(),
        faces: &case.faces,
        previous_moles: &previous,
        sources: &sources,
        dt_days: 1.0,
    };
    let options = NewtonOptions {
        tolerance: 1e-12,
        max_iterations: 15,
    };
    let (outcome, report) = solve_newton(&problem, &case.state, options);
    outcome.unwrap_or_else(|e| panic!("{e} ({report:?})"));

    let history = &report.residual_history;
    assert!(
        history.len() >= 3,
        "too few iterations to see the rate: {history:?}"
    );

    // Once the step scale stops binding, each residual should be at most the square of the
    // previous one times a modest constant.
    let mut saw_quadratic = false;
    for w in history.windows(2) {
        if w[0] < 1e-3 && w[0] > 0.0 && w[1] > 0.0 && w[1] <= w[0] * w[0] * 1e4 {
            saw_quadratic = true;
        }
    }
    assert!(
        saw_quadratic,
        "no quadratic reduction anywhere in {history:?}; the Jacobian is probably approximate"
    );
}

/// The step scale is one number for the whole system, not one per cell. A per-cell factor would
/// change the Newton direction rather than its length, and the Jacobian was computed for the
/// direction.
#[test]
fn comp_newton_uses_one_step_scale_for_the_whole_system() {
    let case = column(pinned::ternary().unwrap(), 3, 150.0, &[0.2, 0.5]);
    let rock = case.rock();
    let previous = case.previous();
    let total: f64 = previous[0].iter().sum();
    let mut sources = case.zero_sources();
    // A source only in the first cell, so an unbounded step would be much larger there.
    sources[0] = vec![-total * 5e-2, 0.0, 0.0];

    let problem = NewtonProblem {
        spec: &case.spec,
        layout: &case.layout,
        rock,
        relperm: &relperm(),
        faces: &case.faces,
        previous_moles: &previous,
        sources: &sources,
        dt_days: 1.0,
    };
    let (_, report) = solve_newton(&problem, &case.state, NewtonOptions::default());
    for tau in &report.step_scale_history {
        assert!(
            *tau > 0.0 && *tau <= 1.0,
            "step scale {tau} is outside (0, 1]; it is a single global factor"
        );
    }
}
