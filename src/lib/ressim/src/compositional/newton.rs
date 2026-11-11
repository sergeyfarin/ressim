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

use super::assembly::{AssemblyError, AssemblyResult, Face, assemble_with_well_bhp};
use super::layout::{CellPrimary, CompositionalLayout};
use super::linear::{self, LinearFailure, LinearOptions, LinearSolveReport};
use super::relperm::RelativePermeabilityModel;
use super::state::{CompositionalState, RockView, StateError, TrialState};
use super::wells::{
    BHP_SEARCH_MAX_BAR, BHP_SEARCH_MIN_BAR, CompositionalWell, WellControl, initial_bhp_guesses,
};

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

/// One trial evaluated by the test-only backtracking line search (#116).
#[derive(Clone, Debug, PartialEq)]
pub struct LineSearchTrial {
    /// Step fraction of the Newton direction this trial evaluated.
    pub tau: f64,
    /// Scaled residual at the trial, or `None` when the trial could not be assembled.
    pub scaled_residual: Option<f64>,
    /// Cells whose flashed phase state differs from the current iterate's assembly.
    pub phase_changes: Vec<usize>,
    /// Whether this trial met the decrease test and was taken.
    pub accepted: bool,
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
    /// One entry per iteration that evaluated a step: the test-only line search's trials, in
    /// order. Empty in production builds and when the line search is off.
    pub line_search_history: Vec<Vec<LineSearchTrial>>,
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
    /// Each well's BHP at the accepted state, in the order the wells were given.
    pub accepted_well_bhp: Vec<f64>,
}

/// Convergence and iteration settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewtonOptions {
    /// Scaled residual below which the step is accepted.
    pub tolerance: f64,
    pub max_iterations: usize,
    /// Which linear solve each iteration uses, and its settings.
    pub linear: LinearOptions,
    /// Where a rate-controlled well's BHP unknown starts. Off by default (#116).
    pub well_bhp_start: WellBhpStart,
}

/// The first BHP a Newton solve gives a rate-controlled well.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WellBhpStart {
    /// The control's own guess ([`super::wells::initial_bhp_guesses`]), independent of history,
    /// so a retry starts where a fresh step does. The default.
    #[default]
    ControlGuess,
    /// The BHP the well was last accepted at ([`NewtonProblem::accepted_well_bhp`]), when one is
    /// known and finite; the control's guess otherwise. A BHP-controlled or shut-in well always
    /// starts at its control's value. Experimental (#116): it lets a step whose cells are already
    /// converged start without the well correction that can push a near-critical cell across its
    /// phase switch.
    LastAccepted,
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
            well_bhp_start: WellBhpStart::ControlGuess,
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
    /// Each well's last accepted BHP, in the well order, when the caller has one. Read only under
    /// [`WellBhpStart::LastAccepted`].
    pub accepted_well_bhp: Option<&'a [f64]>,
}

/// The BHPs a Newton solve starts from, per [`NewtonOptions::well_bhp_start`].
fn starting_well_bhp(
    problem: &NewtonProblem<'_>,
    state: &CompositionalState,
    start: WellBhpStart,
) -> Vec<f64> {
    let mut guesses = initial_bhp_guesses(problem.wells, state.cells());
    if start == WellBhpStart::LastAccepted
        && let Some(accepted) = problem.accepted_well_bhp
        && accepted.len() == guesses.len()
    {
        for ((guess, well), last) in guesses.iter_mut().zip(problem.wells).zip(accepted) {
            let rate_controlled = matches!(
                well.control,
                super::wells::WellControl::MolarRate { .. }
                    | super::wells::WellControl::SurfaceRate { .. }
            );
            if rate_controlled && last.is_finite() && *last > 0.0 {
                *guess = *last;
            }
        }
    }
    guesses
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

/// The BHP part of the fraction-to-boundary bound.
///
/// A rate-controlled well's BHP is a Newton unknown, so the step must keep it positive and inside
/// the same interval the control uses. Clipping at the limit is not a substitute for the control
/// switch — the next assembly makes that decision from the cell state — but it stops a linear
/// correction overshooting into a pressure the well could never run at.
fn well_fraction_to_boundary(
    wells: &[CompositionalWell],
    eliminations: &[super::assembly::WellElimination],
    current_bhp: &[f64],
    direction: &[f64],
) -> f64 {
    let mut tau = 1.0f64;
    for (index, elimination) in eliminations.iter().enumerate() {
        let dbhp = direction[index];
        if dbhp == 0.0 || !dbhp.is_finite() {
            continue;
        }
        let bhp = current_bhp[elimination.well];
        let well = &wells[elimination.well];
        let (limit, injecting) = match &well.control {
            WellControl::MolarRate {
                target_mol_per_day,
                bhp_limit_bar,
            } => (*bhp_limit_bar, *target_mol_per_day > 0.0),
            WellControl::SurfaceRate {
                target_m3_per_day,
                bhp_limit_bar,
                ..
            } => (*bhp_limit_bar, *target_m3_per_day > 0.0),
            WellControl::Bhp { .. } => continue,
            WellControl::ShutIn => continue,
        };

        // Positivity, and the absolute search interval.
        if dbhp < 0.0 {
            tau = tau.min(FRACTION_TO_BOUNDARY * (bhp - BHP_SEARCH_MIN_BAR) / -dbhp);
        } else {
            tau = tau.min(FRACTION_TO_BOUNDARY * (BHP_SEARCH_MAX_BAR - bhp) / dbhp);
        }
        // The control's limit is the hard end on the direction the well is allowed to move.
        if injecting && dbhp > 0.0 {
            tau = tau.min(FRACTION_TO_BOUNDARY * (limit - bhp) / dbhp);
        } else if !injecting && dbhp < 0.0 {
            tau = tau.min(FRACTION_TO_BOUNDARY * (bhp - limit) / -dbhp);
        }
    }
    tau.clamp(0.0, 1.0)
}

/// The BHPs after a fraction `tau` of the Newton direction.
fn bhp_after_step(
    current: &[f64],
    eliminations: &[super::assembly::WellElimination],
    updates: &[f64],
    tau: f64,
) -> Vec<f64> {
    let mut bhp = current.to_vec();
    for (index, elimination) in eliminations.iter().enumerate() {
        bhp[elimination.well] += tau * updates[index];
    }
    bhp
}

/// Test-only (#116): backtrack along the fraction-to-boundary step until the full scaled residual
/// decreases. Every trial is assembled with the attempt's fixed carried-`K` hint, so an evaluation
/// cannot change the operator the next trial sees.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn line_search(
    problem: &NewtonProblem<'_>,
    current: &CompositionalState,
    current_norm: f64,
    direction: &[f64],
    bhp_updates: &[f64],
    assembled: &AssemblyResult,
    initial_tau: f64,
    iteration: usize,
    trials: &mut Vec<LineSearchTrial>,
) -> Result<(CompositionalState, Vec<f64>, f64, f64, AssemblyResult), NewtonError> {
    let mut tau = initial_tau;
    for _ in 0..=MAX_LINE_SEARCH_HALVINGS {
        let candidate = apply_step(problem.layout, current, direction, tau)
            .commit()
            .ok()
            .and_then(|state| {
                let bhp = bhp_after_step(
                    &assembled.well_bhp,
                    &assembled.well_eliminations,
                    bhp_updates,
                    tau,
                );
                assemble_at(problem, &state, &bhp).ok().map(|trial| {
                    let norm = trial.scaled_residual_norm(problem.layout);
                    (state, bhp, norm, trial)
                })
            });
        match candidate {
            Some((state, bhp, norm, trial)) => {
                let accepted = norm <= current_norm * (1.0 - LINE_SEARCH_DECREASE * tau);
                let phase_changes = trial
                    .inventories
                    .iter()
                    .enumerate()
                    .filter(|(cell, inventory)| {
                        inventory.phase_state != assembled.inventories[*cell].phase_state
                    })
                    .map(|(cell, _)| cell)
                    .collect();
                trials.push(LineSearchTrial {
                    tau,
                    scaled_residual: Some(norm),
                    phase_changes,
                    accepted,
                });
                if accepted {
                    return Ok((state, bhp, norm, tau, trial));
                }
            }
            None => trials.push(LineSearchTrial {
                tau,
                scaled_residual: None,
                phase_changes: Vec::new(),
                accepted: false,
            }),
        }
        tau *= 0.5;
    }
    Err(NewtonError::NotConverged {
        iterations: iteration,
        // The failed search did not move: the reported residual is the iterate's, not the best
        // rejected trial's (which is necessarily above it and can be infinite if nothing
        // assembled).
        scaled_residual: current_norm,
    })
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

/// Test-only (#116): extra multiplier applied to Newton's fraction-to-boundary step. One in every
/// non-test build; the failing-call diagnostic sets it to measure a damped step.
#[cfg(not(test))]
#[inline]
fn step_damping_for_test() -> f64 {
    1.0
}

#[cfg(test)]
thread_local! {
    static STEP_DAMPING: core::cell::Cell<f64> = const { core::cell::Cell::new(1.0) };
}

#[cfg(test)]
fn step_damping_for_test() -> f64 {
    STEP_DAMPING.with(|cell| cell.get())
}

/// Test-only (#116): run `f` with Newton's step multiplied by `factor`, then restore it.
#[cfg(test)]
pub(crate) fn with_step_damping_for_test<T>(factor: f64, f: impl FnOnce() -> T) -> T {
    let previous = STEP_DAMPING.with(|cell| cell.replace(factor));
    let out = f();
    STEP_DAMPING.with(|cell| cell.set(previous));
    out
}

/// Maximum number of halvings in the test-only Newton line search (#116).
#[cfg(test)]
const MAX_LINE_SEARCH_HALVINGS: usize = 20;

/// Required relative decrease at a line-search trial (#116). The flash's own line search accepts a
/// plain decrease; this Armijo-style margin keeps a "decrease" that is only roundoff from being
/// taken as progress.
#[cfg(test)]
const LINE_SEARCH_DECREASE: f64 = 1e-4;

/// Test-only (#116): whether Newton backtracks along its fraction-to-boundary step until the
/// assembled scaled residual decreases. Off in every non-test build.
#[cfg(not(test))]
#[inline]
fn line_search_enabled_for_test() -> bool {
    false
}

#[cfg(test)]
thread_local! {
    static LINE_SEARCH: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

#[cfg(test)]
fn line_search_enabled_for_test() -> bool {
    LINE_SEARCH.with(|cell| cell.get())
}

/// Test-only (#116): run `f` with the backtracking line search switched `on` or off on this thread,
/// and restore the previous setting afterwards.
#[cfg(test)]
pub(crate) fn with_line_search_for_test<T>(on: bool, f: impl FnOnce() -> T) -> T {
    let previous = LINE_SEARCH.with(|cell| cell.replace(on));
    let out = f();
    LINE_SEARCH.with(|cell| cell.set(previous));
    out
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
    solve_newton_observed(problem, state, options, None)
}

/// One Newton iterate as [`solve_newton_observed`] hands it to an observer: the iterate and its
/// assembly, before the convergence test and the update.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct NewtonIterate<'a> {
    /// One-based, as in [`NewtonReport::iterations`].
    pub iteration: usize,
    pub state: &'a CompositionalState,
    pub assembled: &'a AssemblyResult,
}

/// Test-only (#116): the cold-recheck acceptance rule for carried-K recovery.
///
/// Warm convergence alone is not enough: the cold evaluation must be converged independently,
/// and the scaled residual-vector difference must fit the same tolerance.
#[cfg(test)]
pub(crate) fn cold_acceptance_ok(
    warm_norm: f64,
    cold_norm: f64,
    vector_difference: f64,
    tolerance: f64,
) -> bool {
    warm_norm <= tolerance
        && cold_norm <= tolerance
        && (warm_norm - cold_norm).abs() <= tolerance
        && vector_difference <= tolerance
}

/// Test-only (#116): maximum scaled per-cell/well residual difference between two assemblies.
#[cfg(test)]
pub(crate) fn scaled_residual_vector_difference(
    warm: &AssemblyResult,
    cold: &AssemblyResult,
    layout: &CompositionalLayout,
) -> Option<f64> {
    let n = layout.cell_block_size();
    let mut worst = 0.0f64;
    for cell in 0..layout.cell_count() {
        let difference: Vec<f64> = (0..n)
            .map(|row| warm.residual[cell * n + row] - cold.residual[cell * n + row])
            .collect();
        worst = worst.max(warm.scaling[cell].scaled_residual_norm(&difference));
    }
    if warm.well_eliminations.len() != cold.well_eliminations.len() {
        return None;
    }
    for warm_well in &warm.well_eliminations {
        let Some(cold_well) = cold
            .well_eliminations
            .iter()
            .find(|cold_well| cold_well.well == warm_well.well)
        else {
            return None;
        };
        if cold_well.id != warm_well.id || warm_well.scale <= 0.0 {
            return None;
        }
        worst = worst.max(((warm_well.residual - cold_well.residual) / warm_well.scale).abs());
    }
    Some(worst)
}

/// [`solve_newton`], calling `observer` with every assembled iterate.
///
/// Diagnostic only (#116): the observer reads, it cannot change the iteration, so a run with an
/// observer is bit-identical to one without.
pub(crate) fn solve_newton_observed(
    problem: &NewtonProblem<'_>,
    state: &CompositionalState,
    options: NewtonOptions,
    mut observer: Option<&mut dyn FnMut(&NewtonIterate<'_>)>,
) -> (Result<CompositionalState, NewtonError>, NewtonReport) {
    let mut report = NewtonReport {
        iterations: 0,
        residual_history: Vec::new(),
        step_scale_history: Vec::new(),
        line_search_history: Vec::new(),
        min_pivot: f64::INFINITY,
        linear_history: Vec::new(),
        accepted_well_rates: Vec::new(),
        accepted_well_bhp: Vec::new(),
    };
    #[cfg(test)]
    {
        crate::fluid::flash::reset_carried_k_for_test();
        if crate::fluid::flash::seed_recovery_enabled_for_test() {
            // Attempt-owned hint: initialize once from the accepted state under this attempt, then
            // keep it fixed for every Newton iteration. Rejected attempts and restarts recompute
            // the same hint from the same state.
            crate::fluid::flash::begin_flash_round_for_test();
            for cell_index in 0..state.cells().len() {
                let cell = state.cell(cell_index);
                let _ = crate::fluid::flash::flash_for_cell(
                    cell_index,
                    problem.spec,
                    crate::fluid::units::bar_to_pa(cell.pressure_bar),
                    problem.spec.reservoir_temperature_k(),
                    &cell.overall_composition(),
                    None,
                );
            }
            crate::fluid::flash::commit_flash_round_for_test();
        }
    }
    let mut current = state.clone();
    let mut current_bhp = starting_well_bhp(problem, &current, options.well_bhp_start);
    // Test-only (#116): the winning line-search trial was assembled before the step, so reuse
    // that assembly at the next iteration instead of assembling the same state twice.
    #[cfg(test)]
    let mut pending_assembly: Option<(AssemblyResult, f64)> = None;

    for iteration in 1..=options.max_iterations {
        report.iterations = iteration;

        #[cfg(test)]
        let pending = pending_assembly.take();
        #[cfg(not(test))]
        let pending: Option<(AssemblyResult, f64)> = None;
        let (assembled, norm) = match pending {
            Some((assembled, norm)) => (assembled, norm),
            None => {
                let assembled = match assemble_at(problem, &current, &current_bhp) {
                    Ok(a) => a,
                    Err(e) => return (Err(NewtonError::Assembly(e)), report),
                };
                let norm = assembled.scaled_residual_norm(problem.layout);
                (assembled, norm)
            }
        };
        report.residual_history.push(norm);
        if let Some(observe) = observer.as_mut() {
            observe(&NewtonIterate {
                iteration,
                state: &current,
                assembled: &assembled,
            });
        }

        if norm <= options.tolerance {
            // A carried-K recovery is admissible only if a cold re-evaluation at the same
            // primaries agrees. Otherwise the accepted state (and the inventory/reporting flash
            // after it) would depend on the seed history and could not be reproduced from a
            // restart. Test-only: seed recovery is off in production.
            #[cfg(test)]
            if crate::fluid::flash::seed_recovery_enabled_for_test() {
                let cold = crate::fluid::flash::with_seed_recovery_for_test(false, || {
                    assemble_at(problem, &current, &current_bhp)
                });
                match cold {
                    Ok(cold) => {
                        let cold_norm = cold.scaled_residual_norm(problem.layout);
                        let Some(vector_difference) =
                            scaled_residual_vector_difference(&assembled, &cold, problem.layout)
                        else {
                            // The active well-rate equations changed between warm and cold.
                            // Matching rows by position would silently drop or mispair them.
                            return (
                                Err(NewtonError::NotConverged {
                                    iterations: iteration,
                                    scaled_residual: f64::INFINITY,
                                }),
                                report,
                            );
                        };
                        if !cold_acceptance_ok(
                            norm,
                            cold_norm,
                            vector_difference,
                            options.tolerance,
                        ) {
                            return (
                                Err(NewtonError::NotConverged {
                                    iterations: iteration,
                                    scaled_residual: cold_norm.max(vector_difference),
                                }),
                                report,
                            );
                        }
                    }
                    Err(error) => return (Err(NewtonError::Assembly(error)), report),
                }
            }
            report.accepted_well_rates = assembled.well_rates;
            report.accepted_well_bhp = assembled.well_bhp;
            return (Ok(current), report);
        }

        // Eliminate the active rate-control BHPs before the linear solve. The reduced system is
        // still the cell block, so the linear route and CPR see the matrix shape they always did;
        // the BHP correction is recovered from the same constraint row afterwards.
        let (mut matrix, mut residual) = match assembled.reduce_well_unknowns() {
            Ok(system) => system,
            Err(e) => return (Err(NewtonError::Assembly(e)), report),
        };

        // Scale the rows before solving. The unscaled system spans many orders of magnitude
        // between a large component and a trace one, and pivoting on it would select pivots by
        // inventory rather than by conditioning.
        let n = problem.layout.cell_block_size();
        let row_scale: Vec<f64> = assembled
            .scaling
            .iter()
            .flat_map(|s| s.row_scale.iter().copied())
            .collect();
        matrix.scale_rows(&row_scale);
        let rhs: Vec<f64> = residual
            .drain(..)
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
        let bhp_updates = match assembled.recover_well_updates(&rhs) {
            Ok(u) => u,
            Err(e) => return (Err(NewtonError::Assembly(e)), report),
        };

        let tau = match fraction_to_boundary(problem.layout, &current, &rhs) {
            Ok(t) => t,
            Err(e) => return (Err(e), report),
        };
        let tau_bound = tau
            .min(well_fraction_to_boundary(
                problem.wells,
                &assembled.well_eliminations,
                &assembled.well_bhp,
                &bhp_updates,
            ))
            .min(1.0)
            * step_damping_for_test();
        // Record the pre-line-search bound first: a failed step must still report the scale it
        // attempted, as it did before the line-search experiment.
        report.step_scale_history.push(tau_bound);
        if tau_bound <= 0.0 {
            return (
                Err(NewtonError::Inadmissible {
                    cell: 0,
                    reason: "the step bound collapsed to zero; no admissible step exists",
                }),
                report,
            );
        }

        #[cfg_attr(not(test), allow(unused_mut))]
        let mut line_search_trials = Vec::new();
        #[cfg(test)]
        let stepped = if line_search_enabled_for_test() {
            match line_search(
                problem,
                &current,
                norm,
                &rhs,
                &bhp_updates,
                &assembled,
                tau_bound,
                iteration,
                &mut line_search_trials,
            ) {
                Ok((state, bhp, trial_norm, tau, trial_assembly)) => {
                    pending_assembly = Some((trial_assembly, trial_norm));
                    Some((state, bhp, tau))
                }
                Err(error) => {
                    report.line_search_history.push(line_search_trials);
                    return (Err(error), report);
                }
            }
        } else {
            None
        };
        #[cfg(not(test))]
        let stepped: Option<(CompositionalState, Vec<f64>, f64)> = None;

        let (next_state, next_bhp, applied_tau) = match stepped {
            Some(step) => step,
            None => {
                let next_bhp = bhp_after_step(
                    &assembled.well_bhp,
                    &assembled.well_eliminations,
                    &bhp_updates,
                    tau_bound,
                );
                let trial = apply_step(problem.layout, &current, &rhs, tau_bound);
                match trial.commit() {
                    Ok(state) => (state, next_bhp, tau_bound),
                    Err(e) => return (Err(NewtonError::State(e)), report),
                }
            }
        };
        if let Some(last) = report.step_scale_history.last_mut() {
            *last = applied_tau;
        }
        report.line_search_history.push(line_search_trials);
        current = next_state;
        current_bhp = next_bhp;
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

#[cfg(test)]
pub(crate) fn assemble_for_test(
    problem: &NewtonProblem<'_>,
    state: &CompositionalState,
    well_bhp: &[f64],
) -> Result<AssemblyResult, AssemblyError> {
    assemble_at(problem, state, well_bhp)
}

fn assemble_at(
    problem: &NewtonProblem<'_>,
    state: &CompositionalState,
    well_bhp: &[f64],
) -> Result<AssemblyResult, AssemblyError> {
    assemble_with_well_bhp(
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
        well_bhp,
    )
}

/// Which primary a column addresses, for a diagnostic that names what moved.
pub fn describe_column(
    layout: &CompositionalLayout,
    column: usize,
) -> Option<(usize, CellPrimary)> {
    layout.split_cell_unknown(column)
}
