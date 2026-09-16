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
//! feed is stable only if neither indicates a lower tangent plane. The thresholds (`1 + 1e-5` on
//! the trial sum, `1e-5` on the trivial-solution norm, `1e-10` on the residual norm) are that
//! source's, reproduced so this module and `opm/compositional/ptflash_fixtures.json` agree on
//! every verdict.
//!
//! The one place this departs from the source is the failure contract: OPM throws after 20000
//! iterations, and here a non-converged trial is a typed [`StabilityError`] carrying the iteration
//! count and the norms it stalled at. A stability test that cannot decide must say so — silently
//! reporting "stable" because the trial ran out of iterations is the specific failure the plan
//! calls out, since it turns a two-phase cell into a single-phase one with no diagnostic.

use super::eos::{EosError, PhaseBranch, evaluate};
use super::specification::FluidSpecification;

/// Iteration cap for one trial-phase substitution loop. OPM uses 20000.
const MAX_TRIAL_ITERATIONS: usize = 20_000;

/// A trial sum above `1 + TRIAL_SUM_TOLERANCE` indicates the feed is unstable.
const TRIAL_SUM_TOLERANCE: f64 = 1e-5;

/// `sum (ln K_i)^2` below this means the trial collapsed onto the feed — the trivial solution,
/// which carries no information about instability.
const TRIVIAL_SOLUTION_TOLERANCE: f64 = 1e-5;

/// `sum (R_i - 1)^2` below this means the trial phase converged.
const TRIAL_RESIDUAL_TOLERANCE: f64 = 1e-10;

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
            (5.3727 * (1.0 + c.acentric_factor) * (1.0 - c.critical_temperature_k / temperature_k))
                .exp()
                * (c.critical_pressure_pa / pressure_pa)
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
}

/// One Michelsen trial phase, by successive substitution on `K`.
///
/// Transcribed from `PTFlash.hpp::checkStability_`, including its choice to evaluate the feed on
/// the *opposite* EOS branch from the trial phase.
fn run_trial(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    k_seed: &[f64],
    trial: TrialPhase,
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

    let mut w = vec![0.0; n];

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
        let feed_props = evaluate(spec, pressure_pa, temperature_k, z, feed_branch)
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
            let ln_f_feed = feed_props.ln_phi[i] + safe_ln(z[i]);
            let ratio = match trial {
                TrialPhase::VapourLike => (ln_f_feed - ln_f_trial).exp() / sum,
                TrialPhase::LiquidLike => (ln_f_trial - ln_f_feed).exp() * sum,
            };
            k[i] *= ratio;
            residual_norm += (ratio - 1.0) * (ratio - 1.0);
            k_norm += k[i].ln() * k[i].ln();
        }

        let trivial = k_norm < TRIVIAL_SOLUTION_TOLERANCE;
        if trivial || residual_norm < TRIAL_RESIDUAL_TOLERANCE {
            return Ok(TrialOutcome {
                sum,
                trivial,
                composition: normalized,
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
    }

    Err(StabilityError::TrialNotConverged {
        trial,
        iterations: MAX_TRIAL_ITERATIONS,
        residual_norm: f64::NAN,
        k_norm: f64::NAN,
    })
}

/// `ln x`, with an exactly zero component mapped to a large negative number rather than `-inf`.
///
/// An absent component contributes no material and must not turn the whole residual into a NaN;
/// mapping it to a finite floor keeps it out of the way without giving it material, which is what
/// the active-component policy in `specification.rs` requires. A *trace* component is untouched —
/// this only fires on exact zero.
fn safe_ln(x: f64) -> f64 {
    if x == 0.0 { -700.0 } else { x.ln() }
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
    seeds.push(wilson.iter().map(|k| k.cbrt()).collect());

    let mut first_error = None;
    let mut any_trial_converged = false;

    for seed in &seeds {
        // Each trial starts from its own copy of the seed: OPM's `K0` and `K1`. Sharing one vector
        // would make the second trial depend on the first, and the two are meant to be independent
        // probes of the same tangent plane.
        let vapour = run_trial(
            spec,
            pressure_pa,
            temperature_k,
            z,
            seed,
            TrialPhase::VapourLike,
        );
        let liquid = run_trial(
            spec,
            pressure_pa,
            temperature_k,
            z,
            seed,
            TrialPhase::LiquidLike,
        );

        let (vapour, liquid) = match (vapour, liquid) {
            (Ok(v), Ok(l)) => (v, l),
            (Err(e), _) | (_, Err(e)) => {
                first_error.get_or_insert(e);
                continue;
            }
        };
        any_trial_converged = true;

        // A trial indicates instability only if its sum exceeds one *and* it did not collapse onto
        // the feed. `PTFlash.hpp::phaseStabilityTest_` writes the same condition in the negative.
        let vapour_indicates = vapour.sum >= 1.0 + TRIAL_SUM_TOLERANCE && !vapour.trivial;
        let liquid_indicates = liquid.sum >= 1.0 + TRIAL_SUM_TOLERANCE && !liquid.trivial;
        if !vapour_indicates && !liquid_indicates {
            continue;
        }

        // `PTFlash.hpp::phaseStabilityTest_` recovers K from the two trial *compositions*:
        // `K_i = y_i / x_i`, with `y` the normalized vapour-like trial and `x` the normalized
        // liquid-like one. An absent component has `y_i = x_i = 0` and no split to describe, so it
        // falls back to the Wilson value, which Rachford-Rice then skips anyway.
        let mut k = Vec::with_capacity(n);
        for i in 0..n {
            let y = vapour.composition[i];
            let x = liquid.composition[i];
            k.push(if x > 0.0 && y > 0.0 { y / x } else { wilson[i] });
        }
        return Ok(StabilityVerdict::Unstable { k });
    }

    if !any_trial_converged {
        // No seed produced a usable answer. "Stable" would be a fabricated verdict.
        return Err(first_error.unwrap_or(StabilityError::InvalidFeed {
            reason: "no stability seed produced a converged trial",
        }));
    }
    Ok(StabilityVerdict::Stable)
}
