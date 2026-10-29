//! Newton solve and the trial-update policy (compositional plan, C10).
//!
//! # The linear solve
//!
//! Each iteration solves the row-scaled block-sparse system on the route [`LinearOptions`] names
//! (see [`super::linear`]): by default sparse LU on small systems and FGMRES with the
//! compositional CPR preconditioner on large ones, or either explicitly, or the dense LU oracle
//! the plan asked C10 to start with. Every iteration's
//! [`LinearSolveReport`] is kept in [`NewtonReport::linear_history`], measured identically on
//! every route, so a direct-vs-iterative comparison reads the same observables from both.
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
//! # Water saturation (V1b, #46)
//!
//! With water, `Sw` is bounded the same way: it may not go negative, and it may not reach one,
//! because a cell with no hydrocarbon pore volume is outside the model (see [`super::water`]).
//! Both bounds join the same single fraction-to-boundary factor.
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
use super::layout::{CellPrimary, CompositionalLayout};
use super::linear::{self, LinearFailure, LinearOptions, LinearSolveReport};
use super::relperm::RelativePermeabilityModel;
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
    /// The linear solve produced no usable correction.
    Linear(LinearFailure),
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
            Self::Linear(e) => write!(f, "{e}"),
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
    /// Smallest linear pivot seen, across iterations. Only the dense route exposes its pivots;
    /// on the others this stays infinite.
    pub min_pivot: f64,
    /// One report per linear solve, in iteration order.
    pub linear_history: Vec<LinearSolveReport>,
    /// Each well's component rate [mol/day] at the **accepted** state, in the order the wells were
    /// given.
    ///
    /// The rate a well actually ran at is a result of the solve, not an input to it. Accumulating
    /// production from a rate evaluated before the step — or re-evaluated after it — is off by
    /// however much the rate moved during the step, which for a BHP well against a pressurizing
    /// cell is not small.
    pub accepted_well_rates: Vec<Vec<f64>>,
}

/// Convergence and iteration settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewtonOptions {
    /// Scaled residual below which the step is accepted.
    pub tolerance: f64,
    pub max_iterations: usize,
    /// Which linear solve each iteration uses, and its settings.
    pub linear: LinearOptions,
}

impl Default for NewtonOptions {
    fn default() -> Self {
        Self {
            // Derived from C8's scaling rather than borrowed: a row is divided by its component's
            // own previous inventory, so this is "every component's mole balance closes to one
            // part in 1e8 of what that component holds". Deliberately not a black-oil number.
            tolerance: 1e-8,
            max_iterations: 20,
            linear: LinearOptions::default(),
        }
    }
}

/// Everything a Newton solve needs that does not change during it.
pub struct NewtonProblem<'a> {
    pub spec: &'a FluidSpecification,
    pub layout: &'a CompositionalLayout,
    pub rock: RockView<'a>,
    pub relperm: &'a RelativePermeabilityModel,
    pub faces: &'a [Face],
    /// Wells, evaluated at every Newton iterate. See `assembly`'s module docs for why they are not
    /// a source term.
    pub wells: &'a [super::wells::CompositionalWell],
    /// Previous accepted inventory, one row per cell. Constant for the whole timestep.
    pub previous_moles: &'a [Vec<f64>],
    /// Sources in mol/day, positive for injection.
    pub sources: &'a [Vec<f64>],
    pub dt_days: f64,
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
    let column = |cell: usize, primary: CellPrimary| -> Result<usize, NewtonError> {
        layout
            .cell_unknown(cell, primary)
            .map_err(|_| NewtonError::Inadmissible {
                cell,
                reason: "the layout has no column for a primary the state carries",
            })
    };

    for cell in 0..layout.cell_count() {
        let c = state.cell(cell);

        // Pressure must stay strictly positive.
        let dp = direction[column(cell, CellPrimary::Pressure)?];
        if dp < 0.0 {
            let limit = -c.pressure_bar / dp;
            tau = tau.min(FRACTION_TO_BOUNDARY * limit);
        }

        // Independent coordinates, and the dependent one, which has no column of its own.
        let mut d_dependent = 0.0;
        for k in 0..(n - 1) {
            let z = c.independent_z()[k];
            let dz = direction[column(cell, CellPrimary::Composition(k))?];
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

        // Water saturation stays in [0, 1): water may not go negative, and the hydrocarbons may
        // not vanish.
        if layout.has_water() {
            let sw = c.water_saturation;
            let dsw = direction[column(cell, CellPrimary::WaterSaturation)?];
            if dsw < 0.0 {
                if sw == 0.0 {
                    if dsw.abs() > ZERO_COMPONENT_DIRECTION_TOLERANCE {
                        return Err(NewtonError::NegativeDirectionOnAbsentComponent {
                            cell,
                            component: n,
                            direction: dsw,
                        });
                    }
                } else {
                    tau = tau.min(FRACTION_TO_BOUNDARY * (-sw / dsw));
                }
            } else if dsw > 0.0 {
                tau = tau.min(FRACTION_TO_BOUNDARY * ((1.0 - sw) / dsw));
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
    let mut trial = state.begin_trial();
    // Every column below exists by construction of the layout; `fraction_to_boundary` has already
    // resolved each of them, so a miss here is a programming error and the entry is left alone.
    let at = |cell: usize, primary: CellPrimary| {
        layout
            .cell_unknown(cell, primary)
            .map_or(0.0, |column| tau * direction[column])
    };
    for cell in 0..layout.cell_count() {
        let dp = at(cell, CellPrimary::Pressure);
        let dz: Vec<f64> = (0..layout.independent_composition_count())
            .map(|k| at(cell, CellPrimary::Composition(k)))
            .collect();
        let dsw = if layout.has_water() {
            at(cell, CellPrimary::WaterSaturation)
        } else {
            0.0
        };
        let target = trial.cell_mut(cell);
        target.pressure_bar += dp;
        for (z, step) in target.independent_z_mut().iter_mut().zip(dz) {
            *z += step;
        }
        target.water_saturation += dsw;
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
        linear_history: Vec::new(),
        accepted_well_rates: Vec::new(),
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
            report.accepted_well_rates = assembled.well_rates;
            return (Ok(current), report);
        }

        // Scale the rows before solving. The unscaled system spans many orders of magnitude
        // between a large component and a trace one, and pivoting on it would select pivots by
        // inventory rather than by conditioning.
        let n = problem.layout.cell_block_size();
        let row_scale: Vec<f64> = assembled
            .scaling
            .iter()
            .flat_map(|s| s.row_scale.iter().copied())
            .collect();
        let mut matrix = assembled.jacobian;
        matrix.scale_rows(&row_scale);
        let rhs: Vec<f64> = assembled
            .residual
            .iter()
            .zip(&row_scale)
            .map(|(r, s)| -r / s)
            .collect();
        let accumulation_blocks: Vec<Vec<f64>> = assembled
            .accumulation_blocks
            .iter()
            .enumerate()
            .map(|(cell, block)| {
                let mut scaled = block.clone();
                for i in 0..n {
                    let s = row_scale[cell * n + i];
                    for entry in &mut scaled[i * n..(i + 1) * n] {
                        *entry /= s;
                    }
                }
                scaled
            })
            .collect();

        let (solved, linear_report) =
            linear::solve(&options.linear, &matrix, &accumulation_blocks, &rhs);
        if let Some(p) = linear_report.min_pivot {
            report.min_pivot = report.min_pivot.min(p);
        }
        report.linear_history.push(linear_report);
        let rhs = match solved {
            Ok(x) => x,
            Err(e) => return (Err(NewtonError::Linear(e)), report),
        };

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
        problem.wells,
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
