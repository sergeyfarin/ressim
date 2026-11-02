//! Phase stability testing (compositional plan, C3, first half).
//!
//! Answers one question: at this `(p, T, z)`, is the single-phase feed stable, or does some trial
//! composition have a lower Gibbs energy? It does **not** compute an equilibrium — that is
//! [`super::flash`]'s job, and it consumes this module's `K` estimate as its starting point.
//!
//! # Why this is not "does Rachford–Rice have a root"
//!
//! Because that question depends on the `K` vector you guessed. A Wilson estimate that happens to
//! give no root in `(0, 1)` proves nothing about the mixture; it proves something about the guess.
//! Michelsen's tangent-plane test asks the physical question instead, and the plan forbids the
//! shortcut explicitly.
//!
//! # Source
//!
//! `/usr/include/opm/material/constraintsolvers/PTFlash.hpp`, `phaseStabilityTest_` and
//! `checkStability_`, from `libopm-common-dev 2026.04-1~noble`. Two trial phases are tested — one
//! vapour-like, seeded `W_i = K_i z_i`, and one liquid-like, seeded `W_i = z_i / K_i` — and the
//! feed is stable only if neither indicates a lower tangent plane. The trivial-solution threshold
//! (`1e-5`) is that source's, reproduced so this module and
//! `opm/compositional/ptflash_fixtures.json` agree on every verdict.
//!
//! Two thresholds deliberately depart from the source. The trial-sum margin is `1e-12`, not OPM's
//! `1e-5`; OPM's margin is wider than a near-critical feed's tangent-plane signal, and the
//! resulting premature phase switch is a residual discontinuity the reservoir Newton solve cannot
//! cross. The trial update residual is `1e-15`, not OPM's `1e-10`, because the trial sum itself
//! must be measured before the verdict is used (see [`TRIAL_SUM_STEP_TOLERANCE`]). See
//! [`TRIAL_SUM_TOLERANCE`] and the SPE5 late-life stall (#93).
//!
//! The failure contract is the other departure from the source: OPM throws after 20000
//! iterations, and here a non-converged trial is a typed [`StabilityError`] carrying the iteration
//! count and the norms it stalled at. A stability test that cannot decide must say so — silently
//! reporting "stable" because the trial ran out of iterations is the specific failure the plan
//! calls out, since it turns a two-phase cell into a single-phase one with no diagnostic. The cap
//! is a backstop, not an accuracy limit: the SPE5 late-life trial that needed 28,946 plain
//! substitutions converged once the cap no longer rejected it (#99).
//!
//! Substitution is still the robust route, but it is linearly convergent and its ratio approaches
//! one in the same near-critical region that stalls the flash. When a trial step retains at least
//! [`TRIAL_NEWTON_TRIGGER_RATIO`] of the residual after [`TRIAL_NEWTON_TRIGGER_AFTER`] iterations,
//! the loop tries a damped Newton step on the tangent-plane stationarity equations for the active
//! components: `ln K_i + ln phi_i^trial(W) - ln phi_i^feed(z) = 0` for the vapour-like trial, with
//! the trial and feed terms reversed for the liquid-like one. A singular Jacobian, an EOS-domain
//! failure or a failed line search returns to substitution, so the acceleration can only shorten a
//! slow path to the same fixed point. The trial-sum and trivial-solution verdict criteria are
//! unchanged (#102).

use super::derivatives::{MIN_PIVOT, scalar_root, solve_in_place};
use super::eos::{
    EosError, PhaseBranch, evaluate, ln_fugacity_coefficients_generic, mixture_params_generic,
};
use super::specification::FluidSpecification;
use crate::ad::{Ad, Scalar};
use crate::math;

/// Iteration cap for one trial-phase substitution loop.
///
/// OPM uses 20000. C3 kept that as a correctness backstop on the same reasoning as the flash cap,
/// but the SPE5 late-life replay found a near-critical `LiquidLike` trial that needed **28,946**
/// plain substitutions to reach its 1e-15 residual (`docs/SPE5_VALIDATION.md`, #99). Failing there
/// is the same kind of cap-as-correctness-limit defect the flash acceleration removed, so the cap
/// is raised rather than left to reject a convergent trial. Newton acceleration now shortens that
/// path to tens of iterations; this remains a backstop, not an accuracy limit.
const MAX_TRIAL_ITERATIONS: usize = 100_000;

/// Trial iterations before a slow state may try Newton acceleration.
///
/// A trial that converges in a handful of substitutions never reaches this, so its path and
/// recorded iteration count are unchanged. The staged start gives Newton a state that has already
/// moved away from the crude Wilson estimate.
const TRIAL_NEWTON_TRIGGER_AFTER: usize = 10;

/// A trial step that keeps at least this fraction of the previous residual is "slow".
///
/// Successive substitution's ratio approaches one near the critical point: at the SPE5 late-life
/// state the ratio is close enough to one that the 1e-15 target is tens of thousands of
/// substitutions away. The threshold is deliberately tighter than the flash's `0.7`: a stability
/// test runs several trials per state and the C6 sweep runs thousands of states, so a merely
/// ordinary path must stay on substitution rather than pay for an AD assembly.
const TRIAL_NEWTON_TRIGGER_RATIO: f64 = 0.95;

/// Trial iterations to let pass after a Newton attempt before retrying.
///
/// A Newton step can fail while the iterate is still outside its basin. Retrying every iteration
/// would pay an AD assembly for no new information, so substitution gets a cooldown to move the
/// state first.
const TRIAL_NEWTON_RETRY_INTERVAL: usize = 50;

/// Iteration cap for one Newton acceleration attempt.
///
/// Quadratic convergence makes this a failure bound rather than a work budget; a near-critical
/// trial that has not converged in this many steps is better served by the substitution fallback.
const MAX_TRIAL_NEWTON_ITERATIONS: usize = 20;

/// Maximum number of step halvings in one Newton line search.
const MAX_TRIAL_NEWTON_HALVINGS: usize = 30;

/// A trial sum above `1 + TRIAL_SUM_TOLERANCE` indicates the feed is unstable.
///
/// OPM's source uses `1e-5`, but that margin is larger than the tangent-plane signal of a
/// near-critical feed: at the SPE5 bubble point the trial sum is `1 + 9.2e-6`, so the source
/// threshold calls a genuinely unstable state stable and the flash jumps from a two-phase split
/// with `beta = 0.065` to a single liquid. The trial sum itself is now required to stop moving to
/// within [`TRIAL_SUM_STEP_TOLERANCE`] (`1e-14`), so this `1e-12` margin keeps the phase switch
/// at `beta ~ 1e-8` rather than at a numerically ambiguous point. (See #93.)
const TRIAL_SUM_TOLERANCE: f64 = 1e-12;

/// `sum (ln K_i)^2` below this means the trial collapsed onto the feed — the trivial solution,
/// which carries no information about instability.
pub(super) const TRIVIAL_SOLUTION_TOLERANCE: f64 = 1e-5;

/// `sum (R_i - 1)^2` below this means the trial phase converged.
const TRIAL_RESIDUAL_TOLERANCE: f64 = 1e-15;

/// The largest relative change in the trial sum between the last two substitution steps.
///
/// The squared update residual does not bound the trial sum directly: the sum is stationary at
/// the fixed point, so its error is second order in the K error, and judging it from the K
/// residual alone needs an unmeasured curvature factor. Requiring the sum itself to stop moving
/// gives the phase decision a measured noise floor. The check is relative to `1 + |sum|` because
/// far from the boundary the trial sum can be large and its last-bit noise scales with it; near
/// the boundary the sum is order one and this reduces to the absolute `1e-14`.
/// [`TRIAL_SUM_TOLERANCE`] is two orders above this, so a verdict cannot flip on the trial
/// iteration's own roundoff.
const TRIAL_SUM_STEP_TOLERANCE: f64 = 1e-14;

/// What the stability test concluded.
#[derive(Clone, Debug, PartialEq)]
pub enum StabilityVerdict {
    /// The feed is a stable single phase. No trial composition lowers the tangent plane.
    Stable,
    /// The feed is unstable: it will split. `k` is the equilibrium-ratio estimate recovered from
    /// whichever trial phase indicated instability, and is a **starting point for the flash**, not
    /// an equilibrium.
    Unstable { k: Vec<f64> },
}

/// Why a stability test could not decide.
#[derive(Clone, Debug, PartialEq)]
pub enum StabilityError {
    /// A trial-phase substitution loop exhausted its iteration budget without converging and
    /// without collapsing to the trivial solution. The verdict is **unknown**, not "stable".
    TrialNotConverged {
        /// Which trial phase stalled.
        trial: TrialPhase,
        iterations: usize,
        residual_norm: f64,
        k_norm: f64,
    },
    /// A trial composition left the EOS's domain.
    Eos { trial: TrialPhase, source: EosError },
    /// The feed composition is invalid or degenerate.
    InvalidFeed { reason: &'static str },
}

impl core::fmt::Display for StabilityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TrialNotConverged {
                trial,
                iterations,
                residual_norm,
                k_norm,
            } => write!(
                f,
                "{trial:?} trial did not converge in {iterations} iterations \
                 (residual norm {residual_norm:e}, K norm {k_norm:e}); the verdict is unknown"
            ),
            Self::Eos { trial, source } => {
                write!(f, "{trial:?} trial left the EOS domain: {source}")
            }
            Self::InvalidFeed { reason } => write!(f, "invalid feed: {reason}"),
        }
    }
}

/// Which of the two trial phases is being tested.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrialPhase {
    /// Seeded `W_i = K_i z_i` and evaluated on the vapour branch.
    VapourLike,
    /// Seeded `W_i = z_i / K_i` and evaluated on the liquid branch.
    LiquidLike,
}

/// Wilson's correlation for the equilibrium ratio.
///
/// `K_i = exp(5.3727 (1 + w_i)(1 - Tc_i/T)) * (pc_i / p)`, from `PTFlash.hpp::wilsonK_`.
///
/// This is an *initial guess*, and the plan says so in as many words. It is the seed for the
/// stability test, never an answer.
pub fn wilson_k(spec: &FluidSpecification, pressure_pa: f64, temperature_k: f64) -> Vec<f64> {
    (0..spec.component_count())
        .map(|i| {
            let c = spec.component(i);
            math::exp(
                5.3727
                    * (1.0 + c.acentric_factor)
                    * (1.0 - c.critical_temperature_k / temperature_k),
            ) * (c.critical_pressure_pa / pressure_pa)
        })
        .collect()
}

/// The outcome of one trial-phase substitution loop.
struct TrialOutcome {
    /// `sum W_i` at convergence. Above one, the trial phase is more stable than the feed.
    sum: f64,
    /// True when the trial collapsed onto the feed, which indicates nothing.
    trivial: bool,
    /// The **normalized trial composition**, `W_i / sum W`.
    ///
    /// This, not the trial's `K`, is what the equilibrium-ratio estimate is built from: the two
    /// trials converge to two different stationary points of the same tangent plane, and dividing
    /// one trial's composition by the other's gives the split. Multiplying their `K` vectors
    /// instead gives a meaningless product — in the C1/C10 binary at 20 bar it produced
    /// `K = [3.1e13, 1.4e-17]`, which sends Rachford–Rice to `-inf` at `beta = 1`.
    composition: Vec<f64>,
    /// Outer substitution iterations used, including the one that accepted a Newton candidate.
    iterations: usize,
}

/// The tangent-plane stationarity residual, over any differentiable scalar.
///
/// For the vapour-like trial `r_i = ln K_i + ln phi_i^trial(W) - ln phi_i^feed(z)`; for the
/// liquid-like trial the trial and feed swap roles in the fugacity ratio, so the corresponding
/// fixed point is `r_i = ln K_i - ln phi_i^trial(W) + ln phi_i^feed(z)`. This is exactly the
/// equation the substitution loop's ratio form satisfies, written without the trial sum so its
/// derivative includes the composition normalization exactly once. An inactive component has no
/// material and no equation; its row pins `ln K` to the value the substitution loop would have
/// frozen it at.
fn trial_residual<S: Scalar>(
    spec: &FluidSpecification,
    pressure_pa: S,
    temperature_k: f64,
    z: &[S],
    ln_k: &[S],
    k_reference: &[f64],
    active: &[bool],
    trial: TrialPhase,
    feed_ln_phi: &[f64],
) -> Result<Vec<S>, EosError> {
    let n = spec.component_count();
    let mut w = Vec::with_capacity(n);
    let mut sum = S::from_f64(0.0);
    for i in 0..n {
        if !active[i] {
            w.push(S::from_f64(0.0));
            continue;
        }
        let k = ln_k[i].exp();
        let value = match trial {
            TrialPhase::VapourLike => k * z[i],
            TrialPhase::LiquidLike => z[i] / k,
        };
        let value_f64 = value.value();
        if !value_f64.is_finite() {
            return Err(EosError::NotFinite {
                what: "trial mole number",
                value: value_f64,
            });
        }
        sum = sum + value;
        w.push(value);
    }
    if !(sum.value() > 0.0) || !sum.value().is_finite() {
        return Err(EosError::NotFinite {
            what: "trial mole number sum",
            value: sum.value(),
        });
    }
    let normalized: Vec<S> = w.iter().map(|value| *value / sum).collect();
    let trial_branch = match trial {
        TrialPhase::VapourLike => PhaseBranch::Vapour,
        TrialPhase::LiquidLike => PhaseBranch::Liquid,
    };
    let params = mixture_params_generic(spec, pressure_pa, temperature_k, &normalized)?;
    let root = scalar_root(&params, trial_branch)?;
    let ln_phi = ln_fugacity_coefficients_generic(spec, &params, root, &normalized)?;

    let mut residual = Vec::with_capacity(n);
    for i in 0..n {
        if active[i] {
            let feed = S::from_f64(feed_ln_phi[i]);
            residual.push(match trial {
                TrialPhase::VapourLike => ln_k[i] + ln_phi[i] - feed,
                TrialPhase::LiquidLike => ln_k[i] - ln_phi[i] + feed,
            });
        } else {
            residual.push(ln_k[i] - S::from_f64(k_reference[i]));
        }
    }
    Ok(residual)
}

/// The trial stationarity system and its Jacobian at one `ln K`.
struct TrialNewtonSystem {
    residual: Vec<f64>,
    jacobian: Vec<Vec<f64>>,
}

/// Evaluate [`trial_residual`] and its Jacobian in one AD pass.
fn trial_newton_system<const N: usize>(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    ln_k: &[f64],
    active: &[bool],
    trial: TrialPhase,
    feed_ln_phi: &[f64],
) -> Option<TrialNewtonSystem> {
    let ln_k_ad: Vec<Ad<N>> = (0..N).map(|i| Ad::variable(ln_k[i], i)).collect();
    let z_ad: Vec<Ad<N>> = z.iter().map(|&value| Ad::constant(value)).collect();
    let residual = trial_residual::<Ad<N>>(
        spec,
        Ad::constant(pressure_pa),
        temperature_k,
        &z_ad,
        &ln_k_ad,
        ln_k,
        active,
        trial,
        feed_ln_phi,
    )
    .ok()?;

    Some(TrialNewtonSystem {
        residual: residual.iter().map(|value| value.value()).collect(),
        jacobian: (0..N)
            .map(|row| (0..N).map(|col| residual[row].d(col)).collect())
            .collect(),
    })
}

fn residual_square_sum(residual: &[f64]) -> f64 {
    residual.iter().map(|value| value * value).sum()
}

/// Try a damped Newton solve of the tangent-plane stationarity system on the active components.
///
/// Returns the converged `K` — never an error. A singular Jacobian, an EOS-domain failure, a line
/// search that cannot decrease the residual, or a solve that exhausts its iteration cap returns
/// `None`, and the caller continues substitution from its own iterate. Acceleration therefore
/// replaces a slow path only when it reaches the same admission tolerance; a partial Newton step
/// cannot move the trial into a different basin and then leave substitution there.
fn trial_newton_acceleration_generic<const N: usize>(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    ln_k: &[f64],
    active: &[bool],
    trial: TrialPhase,
    feed_ln_phi: &[f64],
) -> Option<Vec<f64>> {
    let mut ln_k = ln_k.to_vec();
    let mut system = trial_newton_system::<N>(
        spec,
        pressure_pa,
        temperature_k,
        z,
        &ln_k,
        active,
        trial,
        feed_ln_phi,
    )?;
    let mut norm = residual_square_sum(&system.residual);
    if norm < TRIAL_RESIDUAL_TOLERANCE {
        return Some(ln_k.iter().map(|value| math::exp(*value)).collect());
    }

    for _ in 0..MAX_TRIAL_NEWTON_ITERATIONS {
        let mut jacobian = system.jacobian.clone();
        let mut rhs: Vec<Vec<f64>> = system.residual.iter().map(|&value| vec![-value]).collect();
        let min_pivot = solve_in_place(&mut jacobian, &mut rhs);
        if !(min_pivot >= MIN_PIVOT) {
            break;
        }
        let delta: Vec<f64> = rhs.iter().map(|row| row[0]).collect();
        if delta.iter().any(|value| !value.is_finite()) {
            break;
        }

        let mut accepted = None;
        let mut alpha = 1.0;
        for _ in 0..MAX_TRIAL_NEWTON_HALVINGS {
            let candidate_ln_k: Vec<f64> = ln_k
                .iter()
                .zip(&delta)
                .map(|(value, step)| value + alpha * step)
                .collect();
            if candidate_ln_k.iter().all(|value| value.is_finite()) {
                if let Some(candidate) = trial_newton_system::<N>(
                    spec,
                    pressure_pa,
                    temperature_k,
                    z,
                    &candidate_ln_k,
                    active,
                    trial,
                    feed_ln_phi,
                ) {
                    let candidate_norm = residual_square_sum(&candidate.residual);
                    if candidate_norm <= (1.0 - 1e-4 * alpha) * norm
                        || candidate_norm < TRIAL_RESIDUAL_TOLERANCE
                    {
                        accepted = Some((candidate_ln_k, candidate, candidate_norm));
                        break;
                    }
                }
            }
            alpha *= 0.5;
        }

        let Some((next_ln_k, next_system, next_norm)) = accepted else {
            break;
        };
        ln_k = next_ln_k;
        system = next_system;
        norm = next_norm;
        if norm < TRIAL_RESIDUAL_TOLERANCE {
            return Some(ln_k.iter().map(|value| math::exp(*value)).collect());
        }
    }

    None
}

/// Dispatch the trial Newton attempt to the compiled AD instantiation for this component count.
fn trial_newton_acceleration(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    ln_k: &[f64],
    active: &[bool],
    trial: TrialPhase,
    feed_ln_phi: &[f64],
) -> Option<Vec<f64>> {
    match spec.component_count() {
        2 => trial_newton_acceleration_generic::<2>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            active,
            trial,
            feed_ln_phi,
        ),
        3 => trial_newton_acceleration_generic::<3>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            active,
            trial,
            feed_ln_phi,
        ),
        4 => trial_newton_acceleration_generic::<4>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            active,
            trial,
            feed_ln_phi,
        ),
        5 => trial_newton_acceleration_generic::<5>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            active,
            trial,
            feed_ln_phi,
        ),
        6 => trial_newton_acceleration_generic::<6>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            active,
            trial,
            feed_ln_phi,
        ),
        7 => trial_newton_acceleration_generic::<7>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            active,
            trial,
            feed_ln_phi,
        ),
        8 => trial_newton_acceleration_generic::<8>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            active,
            trial,
            feed_ln_phi,
        ),
        9 => trial_newton_acceleration_generic::<9>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            active,
            trial,
            feed_ln_phi,
        ),
        10 => trial_newton_acceleration_generic::<10>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            active,
            trial,
            feed_ln_phi,
        ),
        _ => None,
    }
}

/// One Michelsen trial phase, by successive substitution on `K`.
///
/// Transcribed from `PTFlash.hpp::checkStability_`, including its choice to evaluate the feed on
/// the *opposite* EOS branch from the trial phase. The substitution loop is the fallback; a slow
/// step may hand the iterate to [`trial_newton_acceleration_generic`] and continue from its answer.
fn run_trial(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    k_seed: &[f64],
    trial: TrialPhase,
    accelerate: bool,
) -> Result<TrialOutcome, StabilityError> {
    let n = spec.component_count();
    let mut k = k_seed.to_vec();

    // The active set: components that actually carry material. An absent component contributes
    // `W_i = 0` to both trial forms whatever its `K` is, so the trial learns nothing about it and
    // its `K` drifts freely — and on the liquid-like branch a drift to zero turns `z_i / K_i` into
    // `0 / 0`. Freezing its `K` and keeping it out of the norms is the `specification.rs`
    // active-component policy applied here: no material, no equation, and no influence on
    // convergence. A *trace* component is active, however small.
    let active: Vec<bool> = z.iter().map(|&v| v > 0.0).collect();
    if !active.iter().any(|&a| a) {
        return Err(StabilityError::InvalidFeed {
            reason: "feed has no active component",
        });
    }

    let (trial_branch, feed_branch) = match trial {
        TrialPhase::VapourLike => (PhaseBranch::Vapour, PhaseBranch::Liquid),
        TrialPhase::LiquidLike => (PhaseBranch::Liquid, PhaseBranch::Vapour),
    };

    // The feed is evaluated on the opposite branch and does not move during the trial, so its
    // fugacity coefficients are computed once rather than once per substitution step.
    let feed_props = evaluate(spec, pressure_pa, temperature_k, z, feed_branch)
        .map_err(|source| StabilityError::Eos { trial, source })?;
    let feed_ln_phi = feed_props.ln_phi;

    let mut w = vec![0.0; n];
    let mut previous_residual = f64::INFINITY;
    let mut next_newton_attempt = TRIAL_NEWTON_TRIGGER_AFTER;

    for iteration in 0..MAX_TRIAL_ITERATIONS {
        let mut sum = 0.0;
        for i in 0..n {
            w[i] = if !active[i] {
                0.0
            } else {
                match trial {
                    TrialPhase::VapourLike => k[i] * z[i],
                    TrialPhase::LiquidLike => z[i] / k[i],
                }
            };
            if !w[i].is_finite() {
                return Err(StabilityError::InvalidFeed {
                    reason: "trial mole number is not finite; check for a zero equilibrium ratio",
                });
            }
            sum += w[i];
        }
        if !(sum > 0.0) {
            return Err(StabilityError::InvalidFeed {
                reason: "trial phase has no material",
            });
        }
        let normalized: Vec<f64> = w.iter().map(|v| v / sum).collect();

        let trial_props = evaluate(spec, pressure_pa, temperature_k, &normalized, trial_branch)
            .map_err(|source| StabilityError::Eos { trial, source })?;

        // R_i drives K toward the tangent-plane stationary point. The pressure factor in the
        // fugacity cancels in the ratio, so it is left out; `ln phi + ln x` is the whole content.
        let mut residual_norm = 0.0;
        let mut k_norm = 0.0;
        for i in 0..n {
            if !active[i] {
                continue;
            }
            let ln_f_trial = trial_props.ln_phi[i] + safe_ln(normalized[i]);
            let ln_f_feed = feed_ln_phi[i] + safe_ln(z[i]);
            let ratio = match trial {
                TrialPhase::VapourLike => math::exp(ln_f_feed - ln_f_trial) / sum,
                TrialPhase::LiquidLike => math::exp(ln_f_trial - ln_f_feed) * sum,
            };
            k[i] *= ratio;
            residual_norm += (ratio - 1.0) * (ratio - 1.0);
            k_norm += math::ln(k[i]) * math::ln(k[i]);
        }

        // Recompute the trial sum at the updated K. The sum is what the phase decision reads,
        // and it must stop moving before the decision is allowed to use it.
        let mut next_sum = 0.0;
        for i in 0..n {
            if !active[i] {
                continue;
            }
            next_sum += match trial {
                TrialPhase::VapourLike => k[i] * z[i],
                TrialPhase::LiquidLike => z[i] / k[i],
            };
        }

        let trivial = k_norm < TRIVIAL_SOLUTION_TOLERANCE;
        let sum_stopped = (next_sum - sum).abs() < TRIAL_SUM_STEP_TOLERANCE * (1.0 + sum.abs());
        if trivial || (residual_norm < TRIAL_RESIDUAL_TOLERANCE && sum_stopped) {
            return Ok(TrialOutcome {
                sum: next_sum,
                trivial,
                composition: normalized,
                iterations: iteration + 1,
            });
        }

        if iteration + 1 == MAX_TRIAL_ITERATIONS {
            return Err(StabilityError::TrialNotConverged {
                trial,
                iterations: MAX_TRIAL_ITERATIONS,
                residual_norm,
                k_norm,
            });
        }

        // A substitution step that retains most of its residual is the near-critical stall. Hand
        // the updated K to the stationarity Newton solve; if it cannot improve the iterate, the
        // loop keeps substituting and retries only after a cooldown.
        let slow = accelerate
            && iteration + 1 >= next_newton_attempt
            && residual_norm >= TRIAL_NEWTON_TRIGGER_RATIO * previous_residual;
        previous_residual = residual_norm;
        if slow {
            let ln_k: Vec<f64> = k.iter().map(|&value| math::ln(value)).collect();
            next_newton_attempt = iteration + 1 + TRIAL_NEWTON_RETRY_INTERVAL;
            if let Some(candidate) = trial_newton_acceleration(
                spec,
                pressure_pa,
                temperature_k,
                z,
                &ln_k,
                &active,
                trial,
                &feed_ln_phi,
            ) {
                k = candidate;
                continue;
            }
        }
    }

    Err(StabilityError::TrialNotConverged {
        trial,
        iterations: MAX_TRIAL_ITERATIONS,
        residual_norm: f64::NAN,
        k_norm: f64::NAN,
    })
}

/// The outer substitution count used by one trial, for the regression that pins the acceleration.
#[cfg(test)]
pub(super) fn trial_iteration_count(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    k_seed: &[f64],
    trial: TrialPhase,
) -> Result<usize, StabilityError> {
    run_trial(spec, pressure_pa, temperature_k, z, k_seed, trial, true)
        .map(|outcome| outcome.iterations)
}

/// `ln x`, with an exactly zero component mapped to a large negative number rather than `-inf`.
///
/// An absent component contributes no material and must not turn the whole residual into a NaN;
/// mapping it to a finite floor keeps it out of the way without giving it material, which is what
/// the active-component policy in `specification.rs` requires. A *trace* component is untouched —
/// this only fires on exact zero.
fn safe_ln(x: f64) -> f64 {
    if x == 0.0 { -700.0 } else { math::ln(x) }
}

/// Run both Michelsen trials for one seed, with or without Newton acceleration.
///
/// Each trial starts from its own copy of the seed: OPM's `K0` and `K1`. Sharing one vector would
/// make the second trial depend on the first, and the two are meant to be independent probes of
/// the same tangent plane.
fn run_trial_pair(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    seed: &[f64],
    accelerate: bool,
) -> Result<(TrialOutcome, TrialOutcome), StabilityError> {
    let vapour = run_trial(
        spec,
        pressure_pa,
        temperature_k,
        z,
        seed,
        TrialPhase::VapourLike,
        accelerate,
    );
    let liquid = run_trial(
        spec,
        pressure_pa,
        temperature_k,
        z,
        seed,
        TrialPhase::LiquidLike,
        accelerate,
    );
    match (vapour, liquid) {
        (Ok(v), Ok(l)) => Ok((v, l)),
        (Err(e), _) | (_, Err(e)) => Err(e),
    }
}

/// A trial pair indicates instability when either sum exceeds one and did not collapse onto the
/// feed. `PTFlash.hpp::phaseStabilityTest_` writes the same condition in the negative.
fn pair_indicates_instability(vapour: &TrialOutcome, liquid: &TrialOutcome) -> bool {
    let vapour_indicates = vapour.sum >= 1.0 + TRIAL_SUM_TOLERANCE && !vapour.trivial;
    let liquid_indicates = liquid.sum >= 1.0 + TRIAL_SUM_TOLERANCE && !liquid.trivial;
    vapour_indicates || liquid_indicates
}

/// `K_i = y_i / x_i` from the two normalized trial compositions.
///
/// An absent component has `y_i = x_i = 0` and no split to describe, so it falls back to the
/// Wilson value, which Rachford-Rice then skips anyway.
fn pair_k_estimate(vapour: &TrialOutcome, liquid: &TrialOutcome, wilson: &[f64]) -> Vec<f64> {
    (0..wilson.len())
        .map(|i| {
            let y = vapour.composition[i];
            let x = liquid.composition[i];
            if x > 0.0 && y > 0.0 { y / x } else { wilson[i] }
        })
        .collect()
}

/// True when `K` can seed the flash: finite, positive and not the trivial solution.
///
/// Only components present in the feed count. An absent component is filled with its Wilson
/// value by [`pair_k_estimate`], which is deliberately non-trivial; including it in the norm
/// would let it mask a coincident pair of active-component ratios.
fn k_is_usable(k: &[f64], z: &[f64]) -> bool {
    let mut active = false;
    let mut norm = 0.0;
    for (value, zi) in k.iter().zip(z) {
        if *zi <= 0.0 {
            continue;
        }
        active = true;
        if !value.is_finite() || *value <= 0.0 {
            return false;
        }
        let ln = math::ln(*value);
        norm += ln * ln;
    }
    active && norm >= TRIVIAL_SOLUTION_TOLERANCE
}

/// Test the feed for stability at `(p, T, z)`.
///
/// `k_seed` is the starting equilibrium-ratio estimate. Pass `None` for a cold start, which uses
/// [`wilson_k`]; pass a previous cell's converged `K` to warm-start. Both must reach the same
/// verdict — a test asserts it — because a stability verdict that depended on the guess would make
/// the phase state a function of iteration order rather than of the fluid.
pub fn test_stability(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    k_seed: Option<&[f64]>,
) -> Result<StabilityVerdict, StabilityError> {
    let n = spec.component_count();
    if z.len() != n {
        return Err(StabilityError::InvalidFeed {
            reason: "composition length does not match",
        });
    }

    // Stability is declared only after *every* seed has failed to find a lower tangent plane, and
    // any one seed finding one is enough to declare instability. That asymmetry is deliberate:
    // missing an instability silently turns a two-phase cell into a single-phase one, whereas a
    // spurious instability is caught immediately by the flash, which cannot find a split.
    //
    // A single seed is not sufficient, and this is not a theoretical concern — a caller
    // warm-starting from `K ~ 1` (a cell that was single-phase last step, say) lands on the
    // trivial solution on the first iteration, both trials return "no information", and the feed
    // is declared stable. The Wilson seed below finds the split in exactly those cases, which is
    // why it is always tried regardless of what the caller passed.
    let wilson = wilson_k(spec, pressure_pa, temperature_k);
    let mut seeds: Vec<Vec<f64>> = Vec::with_capacity(3);
    if let Some(k) = k_seed {
        if k.len() == n && k.iter().all(|v| v.is_finite() && *v > 0.0) {
            seeds.push(k.to_vec());
        }
    }
    seeds.push(wilson.clone());
    // Michelsen's third seed: the cube root of Wilson, which probes closer to the feed and picks
    // up splits the full-strength estimate can overshoot.
    seeds.push(wilson.iter().map(|&k| math::cbrt(k)).collect());

    let mut first_error = None;
    let mut any_trial_converged = false;
    let mut saw_unusable_instability = false;

    for seed in &seeds {
        let (vapour, liquid) = match run_trial_pair(spec, pressure_pa, temperature_k, z, seed, true)
        {
            Ok(pair) => pair,
            Err(e) => {
                first_error.get_or_insert(e);
                continue;
            }
        };
        any_trial_converged = true;

        if !pair_indicates_instability(&vapour, &liquid) {
            continue;
        }

        let mut k = pair_k_estimate(&vapour, &liquid, &wilson);
        if k_is_usable(&k, z) {
            return Ok(StabilityVerdict::Unstable { k });
        }

        // Newton can converge both trials onto the same stationary composition while substitution
        // reaches two distinct ones. The ratio of coincident compositions is `K ~ 1`, which is
        // not a usable instability seed. Re-run this seed on plain substitution before giving up
        // on it; the accelerated pair is accepted only when it returns a non-trivial `K`.
        //
        // Record the instability before the retry: a retry that errors or fails to reproduce it
        // must not erase the lower tangent plane already found.
        saw_unusable_instability = true;
        let (vapour, liquid) =
            match run_trial_pair(spec, pressure_pa, temperature_k, z, seed, false) {
                Ok(pair) => pair,
                Err(e) => {
                    first_error.get_or_insert(e);
                    continue;
                }
            };
        if !pair_indicates_instability(&vapour, &liquid) {
            continue;
        }
        k = pair_k_estimate(&vapour, &liquid, &wilson);
        if k_is_usable(&k, z) {
            return Ok(StabilityVerdict::Unstable { k });
        }
    }

    if !any_trial_converged {
        // No seed produced a usable answer. "Stable" would be a fabricated verdict.
        return Err(first_error.unwrap_or(StabilityError::InvalidFeed {
            reason: "no stability seed produced a converged trial",
        }));
    }
    if saw_unusable_instability {
        // The trials did find a lower tangent plane, but every trial pair collapsed to a trivial
        // composition ratio. Return the Wilson estimate rather than a `K ~ 1` vector: the verdict
        // is what matters, and Wilson is at least a non-trivial seed for the flash.
        return Ok(StabilityVerdict::Unstable { k: wilson });
    }
    Ok(StabilityVerdict::Stable)
}
