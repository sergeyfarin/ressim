//! Isothermal pressure–temperature flash (compositional plan, C3, second half).
//!
//! Given `(p, T, z)`, decide the phase state and — when two phases coexist — solve for the vapour
//! fraction and both phase compositions at fugacity equilibrium.
//!
//! # The `beta` / `L` convention
//!
//! This module uses **`beta`, the vapour mole fraction**, as the plan specifies. OPM uses `L`, the
//! *liquid* mole fraction, and its `rachfordRice_g_` is written in `L`. They are related by
//! `beta = 1 - L`, and the fixture writes both out so nothing here has to infer which is which.
//! The Rachford–Rice function solved below is
//!
//! ```text
//! g(beta) = sum_i z_i (K_i - 1) / (1 + beta (K_i - 1)) = 0
//! ```
//!
//! which is monotonically decreasing in `beta` — that monotonicity is what makes a bracketed
//! solve unconditionally safe, and it is asserted in the tests rather than assumed.
//!
//! # Method
//!
//! Stability first ([`super::stability`]), then, if unstable, successive substitution on `K` with
//! a bracketed Rachford–Rice solve inside each iteration. Convergence is on the fugacity residual
//! itself — `max_i |ln(f_i^L / f_i^V)|` — which is the quantity equilibrium is *defined* by, not a
//! proxy for it.
//!
//! Substitution is the robust route, but it is linearly convergent and its ratio approaches one
//! in the near-critical region: the SPE5 Case 1 late-life states need tens of thousands of
//! iterations to reach the admission tolerance ([#99]). When a substitution step keeps at least
//! [`NEWTON_TRIGGER_RATIO`] of the residual, the flash tries a damped Newton solve of the full
//! equilibrium system `[ln K, beta]` from the current iterate. If that fails — singular Jacobian,
//! a trial outside the EOS domain, or no line-search decrease — substitution continues, so a
//! failed acceleration cannot turn a convergent flash into a failure. The bounded non-monotone
//! step below can leave the starting region; the physical and trivial-split guards still apply,
//! and a two-phase Gibbs-energy comparison is [#122].
//!
//! The line search is monotone except for one bounded case. At a critical point the two-phase
//! solution sits in a narrow curved valley: the full Newton step is the right direction, but the
//! residual rises before it falls, so a monotone search takes only a tiny fraction of the step and
//! crawls. [`NEWTON_NONMONOTONE_CEILING`] admits one such full step per Newton attempt when the
//! candidate is physical and its residual is still below `1e-5`; the following Newton iterations
//! converge from there. This relaxes the path, not the final [`EQUILIBRIUM_TOLERANCE`] ([#119]).
//!
//! The Newton system is the one [`super::derivatives`] already uses for implicit differentiation:
//! fugacity equality for every active component plus the Rachford–Rice equation. Keeping one
//! definition of the equations is what makes the accelerated answer and the derivatives describe
//! the same state.
//!
//! Almost every fixture state still reaches tolerance through substitution — only one slow SPE5
//! state crosses the trigger — so the comparison against the OPM fixture remains an independent
//! check of the converged answer rather than a replay of OPM's own `ssi+newton` route.
//!
//! [#99]: https://github.com/sergeyfarin/ressim/issues/99
//! [#119]: https://github.com/sergeyfarin/ressim/issues/119
//! [#122]: https://github.com/sergeyfarin/ressim/issues/122
//!
//! Near a dew point the vapour fraction approaches one, and the Rachford–Rice root is carried in
//! the liquid fraction `L` instead (see `PhaseSplit`). Before that, states a step from the dew
//! point looked like a substitution stall and were not one: `K` had converged to roundoff while
//! the residual floored at 1e-10 to 1e-7, set by one ulp of `beta` in the heavy components'
//! liquid denominators (#75).
//!
//! # Single-phase results
//!
//! A stable feed is a **success**, not a failure. It returns the present phase's own composition
//! (which is `z`) and its density, with `beta` exactly 0 or 1 — an exact physical phase amount,
//! not an approximation. There is no incipient-phase estimate in the result, because an incipient
//! phase carries no material and reporting one invites it to be treated as though it did.

use super::derivatives::{MIN_PIVOT, equilibrium_residual, solve_in_place};
use super::eos::{EosError, PhaseBranch, PhaseProperties, evaluate};
use super::specification::FluidSpecification;
use super::stability::{self, StabilityError, StabilityVerdict};
use crate::ad::Ad;
use crate::math;

/// Iteration cap for the flash (successive substitution plus Newton acceleration).
///
/// Generous because C3 is a correctness task: a state that needs many iterations must converge and
/// be measured, not be declared a failure to keep a budget. The near-critical acceleration of #99
/// exists so this stays a backstop rather than the binding limit; it is not tightened.
pub const MAX_SUBSTITUTION_ITERATIONS: usize = 5_000;

/// Convergence threshold on `max_i |ln(f_i^L / f_i^V)|`.
///
/// The plan's admission target for two-phase equilibrium is 1e-8 absolute on the log-fugacity
/// ratio, and the C0 oracle itself reaches 5.1e-10. This is set an order tighter than the target
/// so the converged state, not the stopping rule, is what the comparison measures.
pub const EQUILIBRIUM_TOLERANCE: f64 = 1e-11;

/// Bracketing tolerance for the Rachford–Rice solve.
const RACHFORD_RICE_TOLERANCE: f64 = 1e-14;

/// Iteration cap for one Rachford–Rice solve.
const MAX_RACHFORD_RICE_ITERATIONS: usize = 200;

/// Substitution iterations before a slow state may try Newton acceleration.
///
/// A flash that converges in a handful of substitutions never reaches this, so its path and its
/// recorded iteration count are unchanged. The staged start gives Newton a state that has already
/// shaken off the stability test's crude `K` estimate.
const NEWTON_TRIGGER_AFTER: usize = 10;

/// A substitution step that keeps at least this fraction of the previous residual is "slow".
///
/// Successive substitution is linearly convergent; a ratio near one is the near-critical stall
/// this exists for. A ratio of 0.7 means the tolerance is still roughly seventy iterations away,
/// which is the point at which the full-system Newton step starts to pay for its AD assembly.
/// Almost every fixture state converges faster and keeps its substitution path; the one slow SPE5
/// state that crosses the trigger is still checked against the same OPM reference.
const NEWTON_TRIGGER_RATIO: f64 = 0.7;

/// Substitution iterations to let pass after a failed Newton attempt before retrying.
///
/// A Newton step can fail because the iterate is still outside the basin where its Jacobian is
/// useful. Retrying every iteration would pay an AD assembly each time for no new information;
/// waiting lets substitution move the state and usually makes the next attempt succeed.
const NEWTON_RETRY_INTERVAL: usize = 20;

/// Iteration cap for one Newton acceleration attempt.
///
/// The full equilibrium system is quadratically convergent when it converges, so this is a
/// failure bound rather than a work budget. Exhausting it returns the flash to substitution.
const MAX_NEWTON_ITERATIONS: usize = 50;

/// Maximum number of step halvings in one Newton line search.
const MAX_NEWTON_HALVINGS: usize = 30;

/// Largest full-system residual a **non-monotone** full Newton step may leave behind.
///
/// The monotone line search is the default: a step must reduce the residual. That fails in the
/// narrow curved valley next to a critical point, where the Newton direction is right but the
/// intermediate residual rises before it falls. One full step per attempt below this ceiling is
/// still an equilibrium state good to a log-fugacity error of `1e-5` (0.001%), and the following
/// Newton iterations converge quadratically from it; a failed attempt returns `None` and
/// substitution resumes from the original iterate. This relaxes the *path*, never the final
/// [`EQUILIBRIUM_TOLERANCE`].
const NEWTON_NONMONOTONE_CEILING: f64 = 1e-5;

/// Roundoff margin on the dimensionless Gibbs-energy comparison `g / (R T)`.
///
/// A two-phase state at the phase boundary has the same Gibbs energy as the feed, and the candidate
/// is converged only to [`EQUILIBRIUM_TOLERANCE`], so the two evaluations differ by a few parts in
/// `1e-11` even when the answer is physical. The margin keeps that noise from rejecting a valid
/// Newton replacement while remaining far below any real metastable split's energy gap.
const GIBBS_ENERGY_TOLERANCE: f64 = 1e-10;

/// Which phases are present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseState {
    /// One liquid phase. `beta == 0`.
    SingleLiquid,
    /// One vapour phase. `beta == 1`.
    SingleVapour,
    /// Liquid and vapour in equilibrium.
    TwoPhase,
}

/// A converged flash result.
#[derive(Clone, Debug, PartialEq)]
pub struct FlashState {
    pub phase_state: PhaseState,
    /// Vapour mole fraction. Exactly 0 or 1 in a single-phase state.
    pub beta: f64,
    /// Liquid mole fractions. Equal to `z` in a single-phase state.
    pub x: Vec<f64>,
    /// Vapour mole fractions. Equal to `z` in a single-phase state.
    pub y: Vec<f64>,
    /// Equilibrium ratios `y_i / x_i`. In a single-phase state these are the last stability
    /// estimate and are **auxiliary** — a warm-start hint, not a property of the state.
    pub k: Vec<f64>,
    /// Liquid-branch properties. Present only when a liquid phase exists.
    pub liquid: Option<PhaseProperties>,
    /// Vapour-branch properties. Present only when a vapour phase exists.
    pub vapour: Option<PhaseProperties>,
    /// `max_i |ln(f_i^L / f_i^V)|` at convergence. Zero for a single-phase state, where there is
    /// no second phase to be in equilibrium with.
    pub equilibrium_residual: f64,
    /// Flash iterations used: successive substitution plus every Newton iteration attempted,
    /// including attempts that fell back to substitution. Zero for a single-phase state.
    pub iterations: usize,
}

impl FlashState {
    /// Liquid mole fraction, OPM's `L`. Provided so a comparison against the fixture never has to
    /// perform the `1 - beta` conversion at the call site, where it is easy to forget.
    pub fn l_liquid(&self) -> f64 {
        1.0 - self.beta
    }

    /// Molar volume of the mixture, `(1-beta)/cL + beta/cV` [m³/mol].
    ///
    /// Only the present phases contribute. In a single-phase state the absent phase's molar
    /// density is not merely multiplied by zero — it is never read, because it does not exist and
    /// reading it would be dividing by an undefined quantity.
    pub fn mixture_molar_volume(&self) -> f64 {
        match self.phase_state {
            PhaseState::SingleLiquid => 1.0 / self.liquid.as_ref().expect("liquid").molar_density,
            PhaseState::SingleVapour => 1.0 / self.vapour.as_ref().expect("vapour").molar_density,
            PhaseState::TwoPhase => {
                let cl = self.liquid.as_ref().expect("liquid").molar_density;
                let cv = self.vapour.as_ref().expect("vapour").molar_density;
                (1.0 - self.beta) / cl + self.beta / cv
            }
        }
    }

    /// Vapour **saturation** — the volume fraction, which is not `beta`.
    ///
    /// `S_V = (beta / cV) / v_mixture`. The plan calls this out specifically because setting
    /// `S_vapour = beta` is a plausible-looking error that a mole balance will not catch.
    pub fn vapour_saturation(&self) -> f64 {
        match self.phase_state {
            PhaseState::SingleLiquid => 0.0,
            PhaseState::SingleVapour => 1.0,
            PhaseState::TwoPhase => {
                let cv = self.vapour.as_ref().expect("vapour").molar_density;
                (self.beta / cv) / self.mixture_molar_volume()
            }
        }
    }

    /// Liquid saturation, `1 - S_V`.
    pub fn liquid_saturation(&self) -> f64 {
        1.0 - self.vapour_saturation()
    }
}

/// Why a flash could not produce a physical state.
#[derive(Clone, Debug, PartialEq)]
pub enum FlashError {
    /// The stability test could not decide. The phase state is unknown.
    Stability(StabilityError),
    /// A phase composition left the EOS's domain.
    Eos(EosError),
    /// The flash exhausted its iteration budget.
    NotConverged {
        iterations: usize,
        equilibrium_residual: f64,
        beta: f64,
    },
    /// Rachford–Rice had no root in `[0, 1]` despite an unstable verdict, or could not bracket one.
    ///
    /// This is a genuine inconsistency between the two halves of C3, not a routine outcome, and it
    /// is reported rather than papered over with a clamp to 0 or 1.
    RachfordRiceNoRoot {
        k: Vec<f64>,
        g_at_zero: f64,
        g_at_one: f64,
    },
    /// The feed is invalid.
    InvalidFeed { reason: &'static str },
}

impl core::fmt::Display for FlashError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Stability(e) => write!(f, "stability test failed: {e}"),
            Self::Eos(e) => write!(f, "EOS evaluation failed: {e}"),
            Self::NotConverged {
                iterations,
                equilibrium_residual,
                beta,
            } => write!(
                f,
                "flash did not converge in {iterations} iterations \
                 (equilibrium residual {equilibrium_residual:e}, beta {beta})"
            ),
            Self::RachfordRiceNoRoot {
                g_at_zero,
                g_at_one,
                ..
            } => write!(
                f,
                "Rachford-Rice has no root in [0, 1]: g(0) = {g_at_zero:e}, g(1) = {g_at_one:e}, \
                 yet the stability test called this feed unstable"
            ),
            Self::InvalidFeed { reason } => write!(f, "invalid feed: {reason}"),
        }
    }
}

/// `g(beta) = sum_i z_i (K_i - 1) / (1 + beta (K_i - 1))`.
fn rachford_rice_g(k: &[f64], z: &[f64], beta: f64) -> f64 {
    let mut g = 0.0;
    for i in 0..k.len() {
        if z[i] == 0.0 {
            continue;
        }
        let km1 = k[i] - 1.0;
        g += z[i] * km1 / (1.0 + beta * km1);
    }
    g
}

/// `dg/dbeta`, which is `-sum_i z_i (K_i - 1)^2 / (1 + beta (K_i - 1))^2` and therefore never
/// positive — the monotonicity the bracketed solve relies on.
fn rachford_rice_dg(k: &[f64], z: &[f64], beta: f64) -> f64 {
    let mut dg = 0.0;
    for i in 0..k.len() {
        if z[i] == 0.0 {
            continue;
        }
        let km1 = k[i] - 1.0;
        let d = 1.0 + beta * km1;
        dg -= z[i] * km1 * km1 / (d * d);
    }
    dg
}

/// Solve Rachford–Rice for the vapour fraction on the **extended** window.
///
/// `g` has a pole at `1/(1 - K_i)` for each component. The poles from `K_i > 1` are negative and
/// those from `K_i < 1` exceed one, so between the largest negative pole and the smallest positive
/// one, `g` is continuous, monotonically decreasing, and runs from `+inf` to `-inf`. A unique root
/// therefore always exists there whenever some `K_i > 1 > K_j` — including when it lies **outside**
/// `[0, 1]`.
///
/// Allowing that window is Whitson & Michelsen's negative flash (1989), and it is not a
/// convenience. The stability test's `K` estimate is a starting point, not an equilibrium, and a
/// crude estimate can put the root outside `[0, 1]` on an iteration whose converged answer is
/// firmly inside it. Refusing to solve there would abandon a real two-phase state because its
/// first guess was poor. Conversely, a root that is still outside `[0, 1]` *at convergence* is a
/// positive determination that the feed is single phase — which is what the caller does with it.
///
/// Returns [`FlashError::RachfordRiceNoRoot`] when there is no window at all: every `K_i` on one
/// side of one, which means no split of any sign exists.
pub fn solve_rachford_rice_extended(k: &[f64], z: &[f64]) -> Result<f64, FlashError> {
    Ok(solve_phase_split(k, z)?.beta())
}

/// A Rachford–Rice root, carried in whichever of `beta` and `L = 1 - beta` is nearer zero.
///
/// Near a dew point `beta` approaches one, and the liquid denominators `1 + beta (K_i - 1)` of the
/// heavy components approach `L`. Written in `beta`, those are differences of nearly equal numbers,
/// and one ulp of `beta` (1.1e-16) is a 1e-9 relative error in a heavy liquid mole fraction when
/// `L` is 1e-7: a floor on the fugacity residual two orders above
/// [`EQUILIBRIUM_TOLERANCE`]. Written in `L`, the same denominators are `K_i + L (1 - K_i)`,
/// accurate to the last bit (#75).
#[derive(Clone, Copy, Debug)]
enum PhaseSplit {
    /// The root in `beta`, when `beta <= 0.5`.
    Vapour(f64),
    /// The root in `L`, when `beta > 0.5`.
    Liquid(f64),
}

impl PhaseSplit {
    /// Carry `beta` in whichever phase amount is nearer zero.
    ///
    /// Newton treats `beta` as one unknown, so it can land on either side of one half. Converting
    /// the converged value to the same `L`-carried form the substitution loop uses keeps the final
    /// compositions accurate near a dew point.
    fn from_beta(beta: f64) -> Self {
        if beta <= 0.5 {
            Self::Vapour(beta)
        } else {
            Self::Liquid(1.0 - beta)
        }
    }

    fn beta(self) -> f64 {
        match self {
            Self::Vapour(beta) => beta,
            Self::Liquid(l) => 1.0 - l,
        }
    }

    /// Both phase amounts in `[0, 1]`, decided on the variable that carries the root exactly.
    fn is_physical(self) -> bool {
        let (Self::Vapour(v) | Self::Liquid(v)) = self;
        (0.0..=1.0).contains(&v)
    }

    /// `1 + beta (K_i - 1)`, the liquid denominator `x_i = z_i / d_i`.
    fn liquid_denominator(self, k: f64) -> f64 {
        match self {
            Self::Vapour(beta) => 1.0 + beta * (k - 1.0),
            Self::Liquid(l) => k + l * (1.0 - k),
        }
    }
}

/// Solve Rachford–Rice on the extended window, in whichever phase amount is smaller.
///
/// The equation in `L` is the equation in `beta` with every `K_i` inverted: substituting
/// `beta = 1 - L` and multiplying through by `K_i` gives `-sum_i z_i (1/K_i - 1) / (1 + L (1/K_i -
/// 1))`. So one bracketed solver serves both, and the second solve runs only when the first puts
/// the root past one half.
fn solve_phase_split(k: &[f64], z: &[f64]) -> Result<PhaseSplit, FlashError> {
    let beta = bracketed_rachford_rice_root(k, z)?;
    if beta <= 0.5 {
        return Ok(PhaseSplit::Vapour(beta));
    }
    let k_inverse: Vec<f64> = k.iter().map(|v| 1.0 / v).collect();
    Ok(PhaseSplit::Liquid(bracketed_rachford_rice_root(
        &k_inverse, z,
    )?))
}

/// The root of `g` in `beta` on the pole-free window: safeguarded Newton inside a bisection
/// bracket.
fn bracketed_rachford_rice_root(k: &[f64], z: &[f64]) -> Result<f64, FlashError> {
    let active: Vec<usize> = (0..k.len()).filter(|&i| z[i] > 0.0).collect();
    let k_max = active
        .iter()
        .map(|&i| k[i])
        .fold(f64::NEG_INFINITY, f64::max);
    let k_min = active.iter().map(|&i| k[i]).fold(f64::INFINITY, f64::min);

    if !(k_max > 1.0 && k_min < 1.0) {
        return Err(FlashError::RachfordRiceNoRoot {
            k: k.to_vec(),
            g_at_zero: rachford_rice_g(k, z, 0.0),
            g_at_one: rachford_rice_g(k, z, 1.0),
        });
    }

    // The pole-free window, inset so `g` stays finite at the bracket ends.
    let pole_low = 1.0 / (1.0 - k_max);
    let pole_high = 1.0 / (1.0 - k_min);
    let inset = (pole_high - pole_low) * 1e-10;
    let (mut lo, mut hi) = (pole_low + inset, pole_high - inset);

    let g_lo = rachford_rice_g(k, z, lo);
    let g_hi = rachford_rice_g(k, z, hi);
    if !(g_lo > 0.0 && g_hi < 0.0) {
        return Err(FlashError::RachfordRiceNoRoot {
            k: k.to_vec(),
            g_at_zero: g_lo,
            g_at_one: g_hi,
        });
    }

    let mut beta = 0.5 * (lo + hi);

    for _ in 0..MAX_RACHFORD_RICE_ITERATIONS {
        let g = rachford_rice_g(k, z, beta);
        if g > 0.0 {
            lo = beta;
        } else {
            hi = beta;
        }
        if hi - lo < RACHFORD_RICE_TOLERANCE * (1.0 + hi.abs()) || g == 0.0 {
            return Ok(beta);
        }

        let dg = rachford_rice_dg(k, z, beta);
        let next = if dg < 0.0 { beta - g / dg } else { f64::NAN };
        if next.is_finite() && next > lo && next < hi {
            // Newton approaching from one side never closes the bracket, so a converged Newton
            // step ends the solve as well. Its error is the square of a step this small.
            if (next - beta).abs() <= RACHFORD_RICE_TOLERANCE * (1.0 + beta.abs()) {
                return Ok(next);
            }
            beta = next;
        } else {
            beta = 0.5 * (lo + hi);
        }
    }
    Ok(0.5 * (lo + hi))
}

/// Solve Rachford–Rice for a **physical** vapour fraction in `[0, 1]`.
///
/// A wrapper on [`solve_rachford_rice_extended`] that rejects a root outside the physical window
/// rather than clamping it. Clamping would turn a contradiction between the stability test and the
/// flash into a silently single-phase answer, which is precisely the failure the plan forbids.
pub fn solve_rachford_rice(k: &[f64], z: &[f64]) -> Result<f64, FlashError> {
    let beta = solve_rachford_rice_extended(k, z)?;
    if !(0.0..=1.0).contains(&beta) {
        return Err(FlashError::RachfordRiceNoRoot {
            k: k.to_vec(),
            g_at_zero: rachford_rice_g(k, z, 0.0),
            g_at_one: rachford_rice_g(k, z, 1.0),
        });
    }
    Ok(beta)
}

/// Phase compositions from `K`, `z` and the phase split.
///
/// `x_i = z_i / (1 + beta (K_i - 1))`, `y_i = K_i x_i`. An absent component stays exactly absent
/// in both phases — dividing zero by anything finite gives zero, and nothing here adds material.
fn phase_compositions(k: &[f64], z: &[f64], split: PhaseSplit) -> (Vec<f64>, Vec<f64>) {
    let n = k.len();
    let mut x = vec![0.0; n];
    let mut y = vec![0.0; n];
    for i in 0..n {
        let d = split.liquid_denominator(k[i]);
        x[i] = z[i] / d;
        y[i] = k[i] * x[i];
    }
    (x, y)
}

/// The largest absolute entry of the full equilibrium residual.
fn equilibrium_norm(residual: &[f64]) -> f64 {
    residual
        .iter()
        .fold(0.0_f64, |acc, value| acc.max(value.abs()))
}

/// The equilibrium system `F(y)` and its Jacobian `F_y`, at `y = [ln K, beta]`.
struct NewtonSystem {
    residual: Vec<f64>,
    jacobian: Vec<Vec<f64>>,
}

/// Evaluate [`super::derivatives::equilibrium_residual`] and its Jacobian in one AD pass.
///
/// `N` is the component count and `M = N + 1` the number of equilibrium unknowns. The two are
/// separate const parameters only because a generic const expression cannot appear in a type
/// here; the dispatch below instantiates them together.
fn newton_system<const N: usize, const M: usize>(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    ln_k: &[f64],
    beta: f64,
) -> Option<NewtonSystem> {
    debug_assert_eq!(M, N + 1);
    let ln_k_ad: Vec<Ad<M>> = (0..N).map(|i| Ad::variable(ln_k[i], i)).collect();
    let beta_ad = Ad::<M>::variable(beta, N);
    let z_ad: Vec<Ad<M>> = z.iter().map(|&value| Ad::constant(value)).collect();

    let residual = equilibrium_residual::<Ad<M>>(
        spec,
        Ad::constant(pressure_pa),
        temperature_k,
        &ln_k_ad,
        &z_ad,
        beta_ad,
    )
    .ok()?;

    Some(NewtonSystem {
        residual: residual.iter().map(|value| value.value()).collect(),
        jacobian: (0..=N)
            .map(|row| (0..=N).map(|col| residual[row].d(col)).collect())
            .collect(),
    })
}

/// Dimensionless molar Gibbs energy `g / (R T)` of one phase, relative to the ideal-gas mixture at
/// the same `(p, T)`:
///
/// ```text
/// g / (R T) = sum_i x_i [ln x_i + ln phi_i]
/// ```
///
/// The common pure-component and pressure terms cancel when two phases at the same `(p, T)` are
/// compared, so they are omitted. Absent components contribute nothing: `x ln x -> 0` as `x -> 0`.
fn phase_gibbs(x: &[f64], ln_phi: &[f64]) -> f64 {
    let mut g = 0.0;
    for i in 0..x.len() {
        if x[i] <= 0.0 {
            continue;
        }
        g += x[i] * (math::ln(x[i]) + ln_phi[i]);
    }
    g
}

/// Lower dimensionless Gibbs energy of the feed as a single phase at `(p, T)`.
///
/// A cubic EOS can have three admissible roots. The stable single-phase state is the one with the
/// lower Gibbs energy, so both branches are evaluated and the lower finite value is used. A branch
/// that does not exist for this feed is not an error here: the other branch still describes the
/// single-phase state. `None` means neither branch could be evaluated, and the caller then lets the
/// Newton answer through rather than rejecting it on a comparison it could not make.
///
/// The feed is normalized before both the EOS evaluation and [`phase_gibbs`], because the two-phase
/// compositions are normalized the same way. The EOS accepts a composition whose sum is within
/// `1e-9` of one, and feeding the raw entries into `sum_i z_i (ln z_i + ln phi_i)` would move the
/// comparison by more than its own tolerance.
fn single_phase_gibbs(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
) -> Option<f64> {
    let total: f64 = z.iter().sum();
    if !(total > 0.0) || !total.is_finite() {
        return None;
    }
    let feed: Vec<f64> = z.iter().map(|value| value / total).collect();
    let mut best: Option<f64> = None;
    for branch in [PhaseBranch::Liquid, PhaseBranch::Vapour] {
        if let Ok(props) = evaluate(spec, pressure_pa, temperature_k, &feed, branch) {
            let g = phase_gibbs(&feed, &props.ln_phi);
            if g.is_finite() && best.is_none_or(|value| g < value) {
                best = Some(g);
            }
        }
    }
    best
}

/// True when a converged Newton answer has a **higher** two-phase Gibbs energy than the feed.
///
/// The bounded non-monotone step can cross a narrow critical valley, and the physical/trivial-split
/// guards do not prove the answer is thermodynamically downhill. A two-phase state is admissible
/// only when it lowers the Gibbs energy relative to the single-phase feed at the same `(p, T)`.
/// The comparison is made only for the Newton replacement: the substitution fixed point is the
/// flash's own answer and is admitted by its residual, not by this guard (#122).
fn newton_answer_raises_gibbs(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    state: &FlashState,
) -> bool {
    let (Some(liquid), Some(vapour)) = (state.liquid.as_ref(), state.vapour.as_ref()) else {
        return false;
    };
    let two_phase = (1.0 - state.beta) * phase_gibbs(&state.x, &liquid.ln_phi)
        + state.beta * phase_gibbs(&state.y, &vapour.ln_phi);
    if !two_phase.is_finite() {
        return false;
    }
    let feed = single_phase_gibbs(spec, pressure_pa, temperature_k, z);
    feed.is_some_and(|feed| two_phase > feed + GIBBS_ENERGY_TOLERANCE)
}

/// Build the two-phase result for a converged `(K, beta)`, recomputing its physical residual.
fn two_phase_from_iterate(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    k: &[f64],
    beta: f64,
    iterations: usize,
) -> Result<FlashState, FlashError> {
    let split = PhaseSplit::from_beta(beta);
    if !split.is_physical() {
        return single_phase(spec, pressure_pa, temperature_k, z);
    }
    let (x, y) = phase_compositions(k, z, split);
    let xs: f64 = x.iter().sum();
    let ys: f64 = y.iter().sum();
    if !(xs > 0.0) || !(ys > 0.0) {
        return Err(FlashError::InvalidFeed {
            reason: "a phase composition vanished",
        });
    }
    let xn: Vec<f64> = x.iter().map(|v| v / xs).collect();
    let yn: Vec<f64> = y.iter().map(|v| v / ys).collect();

    let liquid = evaluate(spec, pressure_pa, temperature_k, &xn, PhaseBranch::Liquid)
        .map_err(FlashError::Eos)?;
    let vapour = evaluate(spec, pressure_pa, temperature_k, &yn, PhaseBranch::Vapour)
        .map_err(FlashError::Eos)?;

    let mut residual: f64 = 0.0;
    for i in 0..spec.component_count() {
        if z[i] == 0.0 {
            continue;
        }
        let ln_ratio = math::ln(xn[i]) + liquid.ln_phi[i] - math::ln(yn[i]) - vapour.ln_phi[i];
        residual = residual.max(ln_ratio.abs());
    }

    Ok(FlashState {
        phase_state: PhaseState::TwoPhase,
        beta: split.beta(),
        x: xn,
        y: yn,
        k: k.to_vec(),
        liquid: Some(liquid),
        vapour: Some(vapour),
        equilibrium_residual: residual,
        iterations,
    })
}

/// Try a damped Newton solve of the full equilibrium system, from a stalled SSI iterate.
///
/// Returns `None` — never an error — when the system is singular, a trial leaves the EOS domain,
/// the line search cannot find a decrease, the converged point misses the tolerance, or it lands
/// on a trivial/unphysical split. The caller then continues successive substitution, so
/// acceleration can only replace a slow route to the same answer and can never turn a flash that
/// would have converged into a failure.
///
/// `newton_steps` accumulates every Newton iteration attempted, successful or not, so the reported
/// work count does not hide discarded attempts.
fn newton_acceleration_generic<const N: usize, const M: usize>(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    ln_k: &[f64],
    beta: f64,
    substitution_iterations: usize,
    newton_steps: &mut usize,
) -> Option<FlashState> {
    let mut ln_k = ln_k.to_vec();
    let mut beta = beta;
    let mut system = newton_system::<N, M>(spec, pressure_pa, temperature_k, z, &ln_k, beta)?;
    let mut norm = equilibrium_norm(&system.residual);
    // At most one non-monotone step per attempt. A second one would let the attempt bounce around
    // below the ceiling without ever decreasing; if the first step does not put Newton on a
    // decreasing path, the attempt fails and substitution resumes.
    let mut used_nonmonotone = false;

    for _ in 1..=MAX_NEWTON_ITERATIONS {
        *newton_steps += 1;
        let mut jacobian = system.jacobian.clone();
        let mut rhs: Vec<Vec<f64>> = system.residual.iter().map(|&value| vec![-value]).collect();
        let min_pivot = solve_in_place(&mut jacobian, &mut rhs);
        if !(min_pivot >= MIN_PIVOT) {
            return None;
        }
        let delta: Vec<f64> = rhs.iter().map(|row| row[0]).collect();
        if delta.iter().any(|value| !value.is_finite()) {
            return None;
        }

        // Damped step: accept a decrease in the full-system residual, or the bounded non-monotone
        // full step described above. A full Newton step is tried first because that is what makes
        // the final convergence quadratic.
        let mut accepted = None;
        let mut alpha = 1.0;
        for _ in 0..MAX_NEWTON_HALVINGS {
            let candidate_ln_k: Vec<f64> = ln_k
                .iter()
                .zip(&delta)
                .map(|(value, step)| value + alpha * step)
                .collect();
            let candidate_beta = beta + alpha * delta[N];
            if candidate_beta.is_finite() && candidate_ln_k.iter().all(|value| value.is_finite()) {
                if let Some(candidate) = newton_system::<N, M>(
                    spec,
                    pressure_pa,
                    temperature_k,
                    z,
                    &candidate_ln_k,
                    candidate_beta,
                ) {
                    let candidate_norm = equilibrium_norm(&candidate.residual);
                    let decreased = candidate_norm <= (1.0 - 1e-4 * alpha) * norm
                        || candidate_norm < EQUILIBRIUM_TOLERANCE;
                    // Near a critical point the two-phase solution sits in a narrow, curved
                    // valley: the full Newton step moves beta towards the answer but the
                    // intermediate residual is larger. A monotone line search rejects that step
                    // and crawls along the valley instead. Allow one such full step per attempt
                    // when the candidate is physical and still close to equilibrium; the next
                    // Newton iterations converge from there, and a failed attempt falls back to
                    // substitution.
                    let near_critical_full_step = !used_nonmonotone
                        && !decreased
                        && alpha == 1.0
                        && candidate_beta > 0.0
                        && candidate_beta < 1.0
                        && candidate_norm < NEWTON_NONMONOTONE_CEILING;
                    if decreased || near_critical_full_step {
                        used_nonmonotone |= near_critical_full_step;
                        accepted =
                            Some((candidate_ln_k, candidate_beta, candidate, candidate_norm));
                        break;
                    }
                }
            }
            alpha *= 0.5;
        }

        let (next_ln_k, next_beta, next_system, next_norm) = accepted?;
        ln_k = next_ln_k;
        beta = next_beta;
        system = next_system;
        norm = next_norm;

        if norm < EQUILIBRIUM_TOLERANCE {
            let split = PhaseSplit::from_beta(beta);
            let k: Vec<f64> = ln_k.iter().map(|value| math::exp(*value)).collect();
            let k_norm: f64 = k
                .iter()
                .map(|value| {
                    let ln = math::ln(*value);
                    ln * ln
                })
                .sum();
            // A smaller residual is not proof of the same physical solution. Near the phase
            // boundary the trivial root `K ~ 1` exists for any `beta`, and it can carry a wildly
            // non-physical phase fraction. Either condition means this Newton answer does not
            // establish a two-phase state; fall back to substitution rather than let
            // `two_phase_from_iterate` turn it into a zero-residual single-phase result.
            if !split.is_physical()
                || k_norm < stability::TRIVIAL_SOLUTION_TOLERANCE
                || !k.iter().all(|value| value.is_finite() && *value > 0.0)
            {
                return None;
            }
            let state = two_phase_from_iterate(
                spec,
                pressure_pa,
                temperature_k,
                z,
                &k,
                beta,
                substitution_iterations + *newton_steps,
            )
            .ok()?;
            // `two_phase_from_iterate` reports a collapsed split as single phase; that is a
            // determination for the negative flash, but it is not a Newton replacement for the
            // two-phase iterate the stability test asked us to solve.
            if state.phase_state != PhaseState::TwoPhase {
                return None;
            }
            // The full-system norm and the reported physical residual differ by the raw phase-sum
            // imbalance; both have to meet the admission target before this replaces SSI.
            if state.equilibrium_residual < EQUILIBRIUM_TOLERANCE {
                // The non-monotone path can leave the starting region, so a converged answer is
                // not by itself proof that the split is downhill. Compare against the single-phase
                // feed and return to substitution when it is not (#122).
                if newton_answer_raises_gibbs(spec, pressure_pa, temperature_k, z, &state) {
                    return None;
                }
                return Some(state);
            }
            return None;
        }
    }
    None
}

/// Test-only entry point for the #122 Gibbs guard: build the two-phase state for a `(K, beta)`
/// iterate and report whether the guard rejects it.
#[cfg(test)]
pub(super) fn newton_answer_from_k_raises_gibbs_for_test(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    k: &[f64],
    beta: f64,
) -> Option<bool> {
    let state = two_phase_from_iterate(spec, pressure_pa, temperature_k, z, k, beta, 0).ok()?;
    Some(newton_answer_raises_gibbs(
        spec,
        pressure_pa,
        temperature_k,
        z,
        &state,
    ))
}

/// Dispatch the Newton attempt to the compiled AD instantiation for this component count.
fn newton_acceleration(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    ln_k: &[f64],
    beta: f64,
    substitution_iterations: usize,
    newton_steps: &mut usize,
) -> Option<FlashState> {
    match spec.component_count() {
        2 => newton_acceleration_generic::<2, 3>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            beta,
            substitution_iterations,
            newton_steps,
        ),
        3 => newton_acceleration_generic::<3, 4>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            beta,
            substitution_iterations,
            newton_steps,
        ),
        4 => newton_acceleration_generic::<4, 5>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            beta,
            substitution_iterations,
            newton_steps,
        ),
        5 => newton_acceleration_generic::<5, 6>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            beta,
            substitution_iterations,
            newton_steps,
        ),
        6 => newton_acceleration_generic::<6, 7>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            beta,
            substitution_iterations,
            newton_steps,
        ),
        7 => newton_acceleration_generic::<7, 8>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            beta,
            substitution_iterations,
            newton_steps,
        ),
        8 => newton_acceleration_generic::<8, 9>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            beta,
            substitution_iterations,
            newton_steps,
        ),
        9 => newton_acceleration_generic::<9, 10>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            beta,
            substitution_iterations,
            newton_steps,
        ),
        10 => newton_acceleration_generic::<10, 11>(
            spec,
            pressure_pa,
            temperature_k,
            z,
            ln_k,
            beta,
            substitution_iterations,
            newton_steps,
        ),
        _ => None,
    }
}

/// Flash at `(p, T, z)`.
///
/// `k_seed` warm-starts both the stability test and the substitution loop. It is a hint: the
/// converged state must not depend on it, and a test asserts that across the whole fixture.
pub fn flash(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    k_seed: Option<&[f64]>,
) -> Result<FlashState, FlashError> {
    let n = spec.component_count();
    if z.len() != n {
        return Err(FlashError::InvalidFeed {
            reason: "composition length does not match",
        });
    }
    if !z.iter().any(|&v| v > 0.0) {
        return Err(FlashError::InvalidFeed {
            reason: "feed has no material",
        });
    }

    let verdict = stability::test_stability(spec, pressure_pa, temperature_k, z, k_seed)
        .map_err(FlashError::Stability)?;

    let mut k = match verdict {
        StabilityVerdict::Unstable { k } => k,
        StabilityVerdict::Stable => {
            return single_phase(spec, pressure_pa, temperature_k, z);
        }
    };

    // The iteration runs on the extended window: the stability test's K is an estimate, and a
    // poor estimate can put the root outside [0, 1] on the way to a converged answer well inside
    // it. Whether the state is really two-phase is decided at convergence, not at the first step.
    let mut residual = f64::INFINITY;
    let mut previous_residual = f64::INFINITY;
    let mut next_newton_attempt = NEWTON_TRIGGER_AFTER + 1;
    let mut newton_steps = 0usize;

    for iteration in 1..=MAX_SUBSTITUTION_ITERATIONS {
        let split = solve_phase_split(&k, z)?;
        let (x, y) = phase_compositions(&k, z, split);

        let xs: f64 = x.iter().sum();
        let ys: f64 = y.iter().sum();
        if !(xs > 0.0) || !(ys > 0.0) {
            return Err(FlashError::InvalidFeed {
                reason: "a phase composition vanished",
            });
        }
        let xn: Vec<f64> = x.iter().map(|v| v / xs).collect();
        let yn: Vec<f64> = y.iter().map(|v| v / ys).collect();

        let liquid = evaluate(spec, pressure_pa, temperature_k, &xn, PhaseBranch::Liquid)
            .map_err(FlashError::Eos)?;
        let vapour = evaluate(spec, pressure_pa, temperature_k, &yn, PhaseBranch::Vapour)
            .map_err(FlashError::Eos)?;

        // ln(f_i^L / f_i^V) = ln x_i + ln phi_i^L - ln y_i - ln phi_i^V. Absent components carry
        // no equilibrium condition and are skipped, per the active-component policy.
        residual = 0.0;
        for i in 0..n {
            if z[i] == 0.0 {
                continue;
            }
            let ln_ratio = math::ln(xn[i]) + liquid.ln_phi[i] - math::ln(yn[i]) - vapour.ln_phi[i];
            residual = residual.max(ln_ratio.abs());
        }

        if residual < EQUILIBRIUM_TOLERANCE {
            // Take the final substitution update before reporting the state, as the loop always
            // has: the converged answer is the fixed point, and one more assignment of
            // `K_i <- phi_i^L / phi_i^V` is the update whose residual just passed the test.
            for i in 0..n {
                k[i] = math::exp(liquid.ln_phi[i] - vapour.ln_phi[i]);
                if !k[i].is_finite() || k[i] <= 0.0 {
                    return Err(FlashError::InvalidFeed {
                        reason: "an equilibrium ratio left the physical range",
                    });
                }
            }
            let split = solve_phase_split(&k, z)?;
            // Converged with the vapour fraction outside the physical window: the two phases have
            // collapsed onto one another, and the stability test's instability verdict is not
            // borne out. That is a determination, not a failure — Whitson & Michelsen's negative
            // flash is exactly the test for it — so the single-phase result is returned rather
            // than an error.
            if !split.is_physical() {
                return single_phase(spec, pressure_pa, temperature_k, z);
            }
            let (x, y) = phase_compositions(&k, z, split);
            let xs: f64 = x.iter().sum();
            let ys: f64 = y.iter().sum();
            let xn: Vec<f64> = x.iter().map(|v| v / xs).collect();
            let yn: Vec<f64> = y.iter().map(|v| v / ys).collect();
            let liquid = evaluate(spec, pressure_pa, temperature_k, &xn, PhaseBranch::Liquid)
                .map_err(FlashError::Eos)?;
            let vapour = evaluate(spec, pressure_pa, temperature_k, &yn, PhaseBranch::Vapour)
                .map_err(FlashError::Eos)?;
            return Ok(FlashState {
                phase_state: PhaseState::TwoPhase,
                beta: split.beta(),
                x: xn,
                y: yn,
                k,
                liquid: Some(liquid),
                vapour: Some(vapour),
                equilibrium_residual: residual,
                iterations: iteration + newton_steps,
            });
        }

        // Successive substitution decelerates to a fixed-point iteration whose ratio approaches
        // one in the near-critical region. When a step keeps at least `NEWTON_TRIGGER_RATIO` of
        // the residual, try the full equilibrium system's damped Newton step from this iterate.
        // A failed attempt leaves substitution in charge and is retried only after a cooldown, so
        // a failure cannot make a convergent flash fail. A successful non-monotone step is not
        // claimed to be the same fixed point; see the module note and #122.
        let slow = iteration >= next_newton_attempt
            && residual >= NEWTON_TRIGGER_RATIO * previous_residual;
        previous_residual = residual;
        if slow {
            let ln_k: Vec<f64> = k.iter().map(|&value| math::ln(value)).collect();
            if let Some(state) = newton_acceleration(
                spec,
                pressure_pa,
                temperature_k,
                z,
                &ln_k,
                split.beta(),
                iteration,
                &mut newton_steps,
            ) {
                return Ok(state);
            }
            next_newton_attempt = iteration + NEWTON_RETRY_INTERVAL;
        }

        // Successive substitution: K_i <- phi_i^L / phi_i^V. Equilibrium is
        // `x_i phi_i^L = y_i phi_i^V`, so `K_i = y_i / x_i = phi_i^L / phi_i^V` at the fixed
        // point, and the update assigns that ratio rather than multiplying by it. Multiplying is
        // the same expression written with fugacities instead of fugacity coefficients -
        // `K_i *= (x_i phi_i^L)/(y_i phi_i^V)` - and mixing the two forms leaves an extra factor
        // of `K` in the step, which drives `K` away from equilibrium toward the degenerate split
        // into two pure phases instead of toward it.
        for i in 0..n {
            k[i] = math::exp(liquid.ln_phi[i] - vapour.ln_phi[i]);
            if !k[i].is_finite() || k[i] <= 0.0 {
                return Err(FlashError::InvalidFeed {
                    reason: "an equilibrium ratio left the physical range",
                });
            }
        }
    }
    Err(FlashError::NotConverged {
        iterations: MAX_SUBSTITUTION_ITERATIONS + newton_steps,
        equilibrium_residual: residual,
        beta: solve_rachford_rice_extended(&k, z).unwrap_or(f64::NAN),
    })
}

/// Li's pseudo-critical temperature for labelling a single-phase state.
///
/// `Tc_est = sum_i (Vc_i Tc_i z_i) / sum_i (Vc_i z_i)` — a critical-volume-weighted mean of the
/// component critical temperatures. From `PTFlash.hpp::li_single_phase_label_`, citing Li's
/// method. The feed is labelled liquid when `T < Tc_est` and vapour otherwise.
///
/// It is worth being explicit that this is a **label**, not a thermodynamic result. A single-phase
/// fluid above its pseudo-critical temperature is not physically distinguishable into "liquid" and
/// "vapour" at all; the label exists so reporting and relative permeability have something to key
/// on, and it must not be read as a stability claim. The obvious alternatives — the compressibility
/// factor, or comparing the two branches' Gibbs energy — both get this wrong: at 250 bar the C1/C10
/// binary has `Z = 0.975`, which looks like a vapour and is a dense supercritical liquid.
///
/// The units matter and cancel: `criticalVolume` is m³/kmol here, as everywhere in this crate, and
/// the ratio is unaffected by the factor of 1000.
pub fn li_pseudo_critical_temperature(spec: &FluidSpecification, z: &[f64]) -> f64 {
    let mut sum_vz = 0.0;
    for i in 0..spec.component_count() {
        sum_vz += spec.component(i).critical_volume_m3_per_kmol * z[i];
    }
    if !(sum_vz > 0.0) {
        return f64::NAN;
    }
    let mut tc = 0.0;
    for i in 0..spec.component_count() {
        let c = spec.component(i);
        tc += c.critical_volume_m3_per_kmol * c.critical_temperature_k * z[i] / sum_vz;
    }
    tc
}

/// Build the result for a stable feed.
///
/// The phase label comes from [`li_pseudo_critical_temperature`], matching OPM. Both EOS branches
/// are evaluated first because at a single-root state they are the same root, and at a three-root
/// state the label decides which one is the physical phase.
fn single_phase(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
) -> Result<FlashState, FlashError> {
    let tc_est = li_pseudo_critical_temperature(spec, z);
    if !tc_est.is_finite() {
        return Err(FlashError::InvalidFeed {
            reason: "feed has no material to label",
        });
    }
    let is_liquid = temperature_k < tc_est;

    let props = evaluate(
        spec,
        pressure_pa,
        temperature_k,
        z,
        if is_liquid {
            PhaseBranch::Liquid
        } else {
            PhaseBranch::Vapour
        },
    )
    .map_err(FlashError::Eos)?;
    let (liquid, vapour) = if is_liquid {
        (Some(props), None)
    } else {
        (None, Some(props))
    };

    let k = stability::wilson_k(spec, pressure_pa, temperature_k);
    Ok(FlashState {
        phase_state: if is_liquid {
            PhaseState::SingleLiquid
        } else {
            PhaseState::SingleVapour
        },
        beta: if is_liquid { 0.0 } else { 1.0 },
        x: z.to_vec(),
        y: z.to_vec(),
        k,
        liquid,
        vapour,
        equilibrium_residual: 0.0,
        iterations: 0,
    })
}
