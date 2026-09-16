//! Newton solve and the trial-update policy (compositional plan, C10).
//!
//! # What this is and is not
//!
//! A **direct** solve on the dense assembled system, which is what the plan asks C10 to start
//! with: a correction-quality oracle on small nonsingular systems, before any iterative or
//! preconditioned path exists to be compared against. The component-aware block-ILU/CPR adapter is
//! explicitly deferred — it is a separate piece of work, and its whole value is being measured
//! against a correction that is known to be right.
//!
//! # The update policy
//!
//! Newton gives a direction; the state has a domain. Pressure must stay positive and the overall
//! composition must stay on the simplex, **including its dependent coordinate**:
//!
//! ```text
//! z_(N-1) = 1 - sum(z_0..z_(N-2))     so    dz_(N-1) = -sum(dz_0..dz_(N-2))
//! ```
//!
//! so a step that leaves every independent coordinate non-negative can still drive the dependent
//! one below zero. The fraction-to-boundary bound below includes it, which is the plan's explicit
//! requirement and is easy to miss because the dependent coordinate has no column to inspect.
//!
//! **One scale factor for the whole step.** A per-cell factor would shorten some cells' steps and
//! not others, which changes the Newton *direction* rather than its length — and a changed
//! direction is no longer the direction the Jacobian was computed for. Taking the global minimum
//! keeps the direction exactly and only shortens the step.
//!
//! Nothing is clamped or renormalized after the step. That is the same requirement stated
//! differently: clamping one coordinate of a computed direction produces a step nobody solved for.
//!
//! # A component at exactly zero
//!
//! An absent component must be able to become present — a well can inject it. So a positive
//! direction on a zero coordinate is unconstrained, and only a **negative** one is a problem,
//! because there is nothing there to remove. That case is reported as a named admissibility
//! failure rather than quietly clamped, because a materially negative direction on an empty
//! component means the residual for that component was not what it should have been.

use crate::fluid::specification::FluidSpecification;

use super::assembly::{AssemblyError, AssemblyResult, Face, assemble};
use super::flux::HydrocarbonRelPerm;
use super::layout::{CellPrimary, CompositionalLayout};
use super::state::{CompositionalState, RockView, StateError, TrialState};

/// How close to the domain boundary a step is allowed to land.
///
/// A step that lands exactly on `z_i = 0` puts the next flash on the boundary of its own domain,
/// where the stability test has the least to work with. Stopping just short costs one more Newton
/// iteration and avoids evaluating thermodynamics at a degenerate composition.
pub const FRACTION_TO_BOUNDARY: f64 = 0.99;

/// Below this, a direction on an exactly-zero component counts as numerically zero rather than as
/// an attempt to remove material that is not there.
pub const ZERO_COMPONENT_DIRECTION_TOLERANCE: f64 = 1e-12;

/// Why a Newton solve did not produce an accepted state.
#[derive(Clone, Debug, PartialEq)]
pub enum NewtonError {
    /// Assembly failed — typically a flash that could not converge at some cell's primaries.
    Assembly(AssemblyError),
    /// The Jacobian is singular to working precision.
    SingularJacobian { min_pivot: f64 },
    /// The iteration budget ran out with the residual still above tolerance.
    NotConverged {
        iterations: usize,
        scaled_residual: f64,
    },
    /// A trial state was inadmissible in a way the step bound cannot fix.
    Inadmissible { cell: usize, reason: &'static str },
    /// The direction wants to remove material from a component that has none.
    NegativeDirectionOnAbsentComponent {
        cell: usize,
        component: usize,
        direction: f64,
    },
    /// The trial could not be committed.
    State(StateError),
}

impl core::fmt::Display for NewtonError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Assembly(e) => write!(f, "assembly failed: {e}"),
            Self::SingularJacobian { min_pivot } => {
                write!(f, "the Jacobian is singular: smallest pivot {min_pivot:e}")
            }
            Self::NotConverged {
                iterations,
                scaled_residual,
            } => write!(
                f,
                "Newton did not converge in {iterations} iterations \
                 (scaled residual {scaled_residual:e})"
            ),
            Self::Inadmissible { cell, reason } => {
                write!(f, "cell {cell}: trial state is inadmissible: {reason}")
            }
            Self::NegativeDirectionOnAbsentComponent {
                cell,
                component,
                direction,
            } => write!(
                f,
                "cell {cell}: the Newton direction removes {direction:e} from component \
                 {component}, which holds nothing"
            ),
            Self::State(e) => write!(f, "{e}"),
        }
    }
}

/// What a Newton solve did, whether or not it succeeded.
#[derive(Clone, Debug, PartialEq)]
pub struct NewtonReport {
    pub iterations: usize,
    /// Scaled residual norm after each iteration, so a stall is visible as a shape rather than a
    /// single number.
    pub residual_history: Vec<f64>,
    /// Step scale factor applied at each iteration. Persistent values below one mean the update is
    /// being held back by the domain, not by the physics.
    pub step_scale_history: Vec<f64>,
    /// Smallest linear pivot seen, across iterations.
    pub min_pivot: f64,
}

/// Convergence and iteration settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewtonOptions {
    /// Scaled residual below which the step is accepted.
    pub tolerance: f64,
    pub max_iterations: usize,
}

impl Default for NewtonOptions {
    fn default() -> Self {
        Self {
            // Derived from C8's scaling rather than borrowed: a row is divided by its component's
            // own previous inventory, so this is "every component's mole balance closes to one
            // part in 1e8 of what that component holds". Deliberately not a black-oil number.
            tolerance: 1e-8,
            max_iterations: 20,
        }
    }
}

/// Everything a Newton solve needs that does not change during it.
pub struct NewtonProblem<'a> {
    pub spec: &'a FluidSpecification,
    pub layout: &'a CompositionalLayout,
    pub rock: RockView<'a>,
    pub relperm: HydrocarbonRelPerm,
    pub faces: &'a [Face],
    /// Previous accepted inventory, one row per cell. Constant for the whole timestep.
    pub previous_moles: &'a [Vec<f64>],
    /// Sources in mol/day, positive for injection.
    pub sources: &'a [Vec<f64>],
    pub dt_days: f64,
}

/// Solve `A x = b` in place by Gaussian elimination with partial pivoting, returning the smallest
/// pivot magnitude seen.
///
/// Dense, because the systems C10 validates on are small and being obviously correct matters more
/// here than being fast. The sparse path is the deferred adapter's, and this is the oracle it will
/// be measured against.
fn solve_dense(matrix: &mut [Vec<f64>], rhs: &mut [f64]) -> f64 {
    let n = matrix.len();
    let mut min_pivot = f64::INFINITY;

    for k in 0..n {
        let mut pivot_row = k;
        for r in (k + 1)..n {
            if matrix[r][k].abs() > matrix[pivot_row][k].abs() {
                pivot_row = r;
            }
        }
        matrix.swap(k, pivot_row);
        rhs.swap(k, pivot_row);

        let pivot = matrix[k][k];
        min_pivot = min_pivot.min(pivot.abs());
        if pivot == 0.0 || !pivot.is_finite() {
            return min_pivot;
        }
        for r in (k + 1)..n {
            let factor = matrix[r][k] / pivot;
            if factor == 0.0 {
                continue;
            }
            for c in k..n {
                matrix[r][c] -= factor * matrix[k][c];
            }
            rhs[r] -= factor * rhs[k];
        }
    }

    for k in (0..n).rev() {
        let mut acc = rhs[k];
        for j in (k + 1)..n {
            acc -= matrix[k][j] * rhs[j];
        }
        rhs[k] = acc / matrix[k][k];
    }
    min_pivot
}

/// The largest `tau` in `(0, 1]` for which the whole step stays strictly inside the domain.
///
/// Returns `Ok(tau)`, or the offending cell and component when a zero-inventory component is being
/// driven negative.
fn fraction_to_boundary(
    layout: &CompositionalLayout,
    state: &CompositionalState,
    direction: &[f64],
) -> Result<f64, NewtonError> {
    let n = layout.component_count();
    let mut tau = 1.0f64;

    for cell in 0..layout.cell_count() {
        let base = cell * n;
        let c = state.cell(cell);

        // Pressure must stay strictly positive.
        let dp = direction[base];
        if dp < 0.0 {
            let limit = -c.pressure_bar / dp;
            tau = tau.min(FRACTION_TO_BOUNDARY * limit);
        }

        // Independent coordinates, and the dependent one, which has no column of its own.
        let mut d_dependent = 0.0;
        for k in 0..(n - 1) {
            let z = c.independent_z()[k];
            let dz = direction[base + 1 + k];
            d_dependent -= dz;

            if dz >= 0.0 {
                continue;
            }
            if z == 0.0 {
                if dz.abs() > ZERO_COMPONENT_DIRECTION_TOLERANCE {
                    return Err(NewtonError::NegativeDirectionOnAbsentComponent {
                        cell,
                        component: k,
                        direction: dz,
                    });
                }
                continue;
            }
            tau = tau.min(FRACTION_TO_BOUNDARY * (-z / dz));
        }

        let z_last = c.dependent_z();
        if d_dependent < 0.0 {
            if z_last == 0.0 {
                if d_dependent.abs() > ZERO_COMPONENT_DIRECTION_TOLERANCE {
                    return Err(NewtonError::NegativeDirectionOnAbsentComponent {
                        cell,
                        component: n - 1,
                        direction: d_dependent,
                    });
                }
            } else {
                tau = tau.min(FRACTION_TO_BOUNDARY * (-z_last / d_dependent));
            }
        }
    }

    Ok(tau.clamp(0.0, 1.0))
}

/// Apply a scaled direction to produce a trial.
fn apply_step(
    layout: &CompositionalLayout,
    state: &CompositionalState,
    direction: &[f64],
    tau: f64,
) -> TrialState {
    let n = layout.component_count();
    let mut trial = state.begin_trial();
    for cell in 0..layout.cell_count() {
        let base = cell * n;
        let target = trial.cell_mut(cell);
        target.pressure_bar += tau * direction[base];
        for k in 0..(n - 1) {
            target.independent_z_mut()[k] += tau * direction[base + 1 + k];
        }
    }
    trial
}

/// Solve one timestep's nonlinear system from `state`.
///
/// Returns the accepted state and a report, or a typed failure and the report of what happened
/// before it. **Nothing outside the returned state is mutated**: the caller's state, the previous
/// inventory and the sources are all untouched, so a failure leaves the timestep exactly where it
/// started.
pub fn solve_newton(
    problem: &NewtonProblem<'_>,
    state: &CompositionalState,
    options: NewtonOptions,
) -> (Result<CompositionalState, NewtonError>, NewtonReport) {
    let mut report = NewtonReport {
        iterations: 0,
        residual_history: Vec::new(),
        step_scale_history: Vec::new(),
        min_pivot: f64::INFINITY,
    };
    let mut current = state.clone();

    for iteration in 1..=options.max_iterations {
        report.iterations = iteration;

        let assembled = match assemble_at(problem, &current) {
            Ok(a) => a,
            Err(e) => return (Err(NewtonError::Assembly(e)), report),
        };
        let norm = assembled.scaled_residual_norm(problem.layout);
        report.residual_history.push(norm);

        if norm <= options.tolerance {
            return (Ok(current), report);
        }

        // Scale the rows before solving. The unscaled system spans many orders of magnitude
        // between a large component and a trace one, and pivoting on it would select pivots by
        // inventory rather than by conditioning.
        let n = problem.layout.component_count();
        let size = problem.layout.cell_count() * n;
        let mut matrix = assembled.jacobian.clone();
        let mut rhs = vec![0.0; size];
        for cell in 0..problem.layout.cell_count() {
            for i in 0..n {
                let row = cell * n + i;
                let scale = assembled.scaling[cell].row_scale[i];
                rhs[row] = -assembled.residual[row] / scale;
                for entry in matrix[row].iter_mut() {
                    *entry /= scale;
                }
            }
        }

        let min_pivot = solve_dense(&mut matrix, &mut rhs);
        report.min_pivot = report.min_pivot.min(min_pivot);
        if !rhs.iter().all(|v| v.is_finite()) {
            return (Err(NewtonError::SingularJacobian { min_pivot }), report);
        }

        let tau = match fraction_to_boundary(problem.layout, &current, &rhs) {
            Ok(t) => t,
            Err(e) => return (Err(e), report),
        };
        report.step_scale_history.push(tau);
        if tau <= 0.0 {
            return (
                Err(NewtonError::Inadmissible {
                    cell: 0,
                    reason: "the step bound collapsed to zero; no admissible step exists",
                }),
                report,
            );
        }

        let trial = apply_step(problem.layout, &current, &rhs, tau);
        current = match trial.commit() {
            Ok(s) => s,
            Err(e) => return (Err(NewtonError::State(e)), report),
        };
    }

    let last = report.residual_history.last().copied().unwrap_or(f64::NAN);
    (
        Err(NewtonError::NotConverged {
            iterations: options.max_iterations,
            scaled_residual: last,
        }),
        report,
    )
}

fn assemble_at(
    problem: &NewtonProblem<'_>,
    state: &CompositionalState,
) -> Result<AssemblyResult, AssemblyError> {
    assemble(
        problem.spec,
        problem.layout,
        &problem.rock,
        problem.relperm,
        state,
        problem.faces,
        problem.previous_moles,
        problem.sources,
        problem.dt_days,
    )
}

/// Which primary a column addresses, for a diagnostic that names what moved.
pub fn describe_column(
    layout: &CompositionalLayout,
    column: usize,
) -> Option<(usize, CellPrimary)> {
    layout.split_cell_unknown(column)
}
