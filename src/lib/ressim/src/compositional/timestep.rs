//! Timestep lifecycle: attempt, reject, retry, commit (compositional plan, C10).
//!
//! # What "nothing is committed before acceptance" means here
//!
//! Time, inventory, the accepted state and the flash cache all advance in exactly one place —
//! [`CompositionalRun::step`]'s success path. A rejected attempt returns a report and changes
//! nothing, and that is structural rather than careful: the attempt works on a clone, and the only
//! route from a candidate to the accepted state is [`super::state::TrialState::commit`], which
//! [`super::newton::solve_newton`] performs on its own copy.
//!
//! The flash cache is cleared on **commit**, not on rejection. It is keyed on the state version
//! (see [`super::state::FlashCacheKey`]), so entries from a rejected attempt cannot be read by
//! anything — they are keyed to a version that never became accepted. Clearing them is a memory
//! question, not a correctness one, which is exactly the property the plan asks for.
//!
//! # The retry ladder
//!
//! A failed attempt halves `dt` and tries again, up to a fixed budget. The budget is fixed rather
//! than adaptive because an unbounded ladder converts a modelling error into a very slow run
//! instead of a diagnosis, and the diagnosis is what the caller needs.
//!
//! Every failure is classified — flash, linear, nonlinear, admissibility, or budget — because
//! "the step failed" is not actionable and the five have entirely different responses.

use crate::fluid::specification::FluidSpecification;

use super::accumulation::{AccumulationError, cell_inventory};
use super::assembly::AssemblyError;
use super::layout::CompositionalLayout;
use super::newton::{NewtonError, NewtonOptions, NewtonProblem, NewtonReport, solve_newton};
use super::relperm::RelativePermeabilityModel;
use super::state::{CompositionalState, FlashCache, RockView};

/// What kind of failure a rejected attempt hit.
///
/// The distinction is the point. A flash failure means the state left the thermodynamic domain and
/// a smaller step will probably help; a singular Jacobian usually will not be fixed by a smaller
/// step at all; a nonlinear stall often will. Collapsing them into one "failed" loses exactly the
/// information a caller needs to decide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    /// A cell's flash could not resolve, or could not be differentiated.
    Flash,
    /// The linear system was singular.
    Linear,
    /// Newton ran out of iterations with the residual still above tolerance.
    Nonlinear,
    /// A trial state was outside the physical domain in a way the step bound could not fix.
    Admissibility,
    /// The retry budget was exhausted.
    Budget,
}

/// One attempt at a timestep.
#[derive(Clone, Debug, PartialEq)]
pub struct AttemptReport {
    pub dt_days: f64,
    pub newton: NewtonReport,
    /// `None` when the attempt succeeded.
    pub failure: Option<(FailureKind, String)>,
}

/// The outcome of a whole step, including every attempt made.
#[derive(Clone, Debug, PartialEq)]
pub struct StepReport {
    /// The timestep actually accepted, if any.
    pub accepted_dt_days: Option<f64>,
    pub attempts: Vec<AttemptReport>,
    /// Set when no attempt succeeded.
    pub failure: Option<(FailureKind, String)>,
}

impl StepReport {
    pub fn succeeded(&self) -> bool {
        self.accepted_dt_days.is_some()
    }

    pub fn retries(&self) -> usize {
        self.attempts.len().saturating_sub(1)
    }
}

/// Timestep control settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimestepOptions {
    /// Factor applied to `dt` after a failed attempt.
    pub cut_factor: f64,
    /// Maximum number of attempts, including the first.
    pub max_attempts: usize,
    /// Smallest `dt` worth attempting. Below this the problem is not a timestep problem.
    pub min_dt_days: f64,
    pub newton: NewtonOptions,
}

impl Default for TimestepOptions {
    fn default() -> Self {
        Self {
            cut_factor: 0.5,
            max_attempts: 6,
            min_dt_days: 1e-6,
            newton: NewtonOptions::default(),
        }
    }
}

fn classify(error: &NewtonError) -> (FailureKind, String) {
    let message = error.to_string();
    let kind = match error {
        NewtonError::Assembly(AssemblyError::Accumulation {
            source: AccumulationError::Flash(_) | AccumulationError::Derivative(_),
            ..
        }) => FailureKind::Flash,
        NewtonError::Assembly(AssemblyError::Flux { .. }) => FailureKind::Flash,
        NewtonError::Assembly(_) => FailureKind::Admissibility,
        NewtonError::SingularJacobian { .. } => FailureKind::Linear,
        NewtonError::NotConverged { .. } => FailureKind::Nonlinear,
        NewtonError::Inadmissible { .. }
        | NewtonError::NegativeDirectionOnAbsentComponent { .. }
        | NewtonError::State(_) => FailureKind::Admissibility,
    };
    (kind, message)
}

/// A compositional run: the accepted state, the simulated time, and the cache.
///
/// Owns the only mutable simulation state there is, and advances it in exactly one place.
pub struct CompositionalRun {
    state: CompositionalState,
    time_days: f64,
    cache: FlashCache,
    /// Cumulative moles produced (negative) or injected (positive) per component through imposed
    /// **sources**, over accepted steps only.
    cumulative_source_moles: Vec<f64>,
    /// The same, per well, from the rates the accepted solve actually ran at.
    cumulative_well_moles: Vec<Vec<f64>>,
}

impl CompositionalRun {
    pub fn new(state: CompositionalState) -> Self {
        let n = state.component_count();
        let cells = state.cells().len();
        Self {
            state,
            time_days: 0.0,
            cache: FlashCache::with_cells(cells),
            cumulative_source_moles: vec![0.0; n],
            cumulative_well_moles: Vec::new(),
        }
    }

    pub fn state(&self) -> &CompositionalState {
        &self.state
    }

    pub fn time_days(&self) -> f64 {
        self.time_days
    }

    pub fn cumulative_source_moles(&self) -> &[f64] {
        &self.cumulative_source_moles
    }

    /// Cumulative component moles per well [mol], from the rates the accepted solves ran at.
    ///
    /// Positive means into the reservoir. Empty until a step with wells has been accepted.
    pub fn cumulative_well_moles(&self) -> &[Vec<f64>] {
        &self.cumulative_well_moles
    }

    pub fn cache_mut(&mut self) -> &mut FlashCache {
        &mut self.cache
    }

    /// Inventory of the current accepted state, one row per cell.
    pub fn accepted_inventory(
        &self,
        spec: &FluidSpecification,
        rock: &RockView<'_>,
    ) -> Result<Vec<Vec<f64>>, AccumulationError> {
        (0..self.state.cells().len())
            .map(|c| {
                cell_inventory(spec, rock, c, self.state.cell(c))
                    .map(|(inv, _)| inv.component_moles)
            })
            .collect()
    }

    /// Attempt one timestep, cutting `dt` and retrying on failure.
    ///
    /// On success the accepted state, the clock, the cumulative source totals and the cache all
    /// advance together. On failure **none of them move**, and the report says why.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        spec: &FluidSpecification,
        layout: &CompositionalLayout,
        rock: &RockView<'_>,
        relperm: &RelativePermeabilityModel,
        faces: &[super::assembly::Face],
        wells: &[super::wells::CompositionalWell],
        sources: &[Vec<f64>],
        dt_days: f64,
        options: TimestepOptions,
    ) -> StepReport {
        let mut report = StepReport {
            accepted_dt_days: None,
            attempts: Vec::new(),
            failure: None,
        };

        // The previous inventory is evaluated once, from the accepted state, and reused across
        // every attempt. Recomputing it per attempt would make the balance reference the attempt
        // rather than the step.
        let previous_moles = match self.accepted_inventory(spec, rock) {
            Ok(p) => p,
            Err(e) => {
                report.failure = Some((FailureKind::Flash, format!("previous inventory: {e}")));
                return report;
            }
        };

        let mut dt = dt_days;
        for _ in 0..options.max_attempts {
            if dt < options.min_dt_days {
                break;
            }
            let problem = NewtonProblem {
                spec,
                layout,
                rock: *rock,
                relperm,
                faces,
                wells,
                previous_moles: &previous_moles,
                sources,
                dt_days: dt,
            };
            let (outcome, newton) = solve_newton(&problem, &self.state, options.newton);

            match outcome {
                Ok(accepted) => {
                    let accepted_well_rates = newton.accepted_well_rates.clone();
                    report.attempts.push(AttemptReport {
                        dt_days: dt,
                        newton,
                        failure: None,
                    });
                    // The single commit point. Everything advances here, together.
                    self.state = accepted;
                    self.time_days += dt;
                    for (i, total) in self.cumulative_source_moles.iter_mut().enumerate() {
                        *total += dt * sources.iter().map(|s| s[i]).sum::<f64>();
                    }
                    // Well production from the rate the accepted solve ran at, not from a rate
                    // evaluated before or after the step.
                    if self.cumulative_well_moles.len() < accepted_well_rates.len() {
                        self.cumulative_well_moles.resize(
                            accepted_well_rates.len(),
                            vec![0.0; self.cumulative_source_moles.len()],
                        );
                    }
                    for (index, rate) in accepted_well_rates.iter().enumerate() {
                        for (i, value) in rate.iter().enumerate() {
                            self.cumulative_well_moles[index][i] += dt * value;
                        }
                    }
                    // Keyed on the state version, so the old entries are already unreachable;
                    // dropping them is housekeeping, not correctness.
                    self.cache.clear();
                    report.accepted_dt_days = Some(dt);
                    return report;
                }
                Err(e) => {
                    let failure = classify(&e);
                    report.attempts.push(AttemptReport {
                        dt_days: dt,
                        newton,
                        failure: Some(failure.clone()),
                    });
                    report.failure = Some(failure);
                    dt *= options.cut_factor;
                }
            }
        }

        if report.failure.is_none() || report.attempts.len() >= options.max_attempts {
            let detail = report
                .failure
                .as_ref()
                .map(|(kind, message)| format!("last failure was {kind:?}: {message}"))
                .unwrap_or_else(|| "no attempt was made above the minimum timestep".to_string());
            report.failure = Some((FailureKind::Budget, detail));
        }
        report
    }
}
