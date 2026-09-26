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
//! This deliberately does not reproduce OPM's `ssi+newton` staging. OPM runs five substitution
//! steps and then a Newton solve on the full equilibrium system, because it needs a fixed small
//! iteration count per cell. Both methods converge to the same fixed point, so reproducing the
//! *path* would buy nothing while making the comparison against the fixture circular: converging
//! independently and agreeing is evidence, converging by the same route is not. Newton
//! acceleration belongs in C14 if profiling asks for it.
//!
//! # Single-phase results
//!
//! A stable feed is a **success**, not a failure. It returns the present phase's own composition
//! (which is `z`) and its density, with `beta` exactly 0 or 1 — an exact physical phase amount,
//! not an approximation. There is no incipient-phase estimate in the result, because an incipient
//! phase carries no material and reporting one invites it to be treated as though it did.

use super::eos::{EosError, PhaseBranch, PhaseProperties, evaluate};
use super::specification::FluidSpecification;
use super::stability::{self, StabilityError, StabilityVerdict};
use crate::math;

/// Iteration cap for successive substitution on `K`.
///
/// Generous because C3 is a correctness task: a state that needs many iterations must converge and
/// be measured, not be declared a failure to keep a budget. C14 owns the performance question and
/// may add acceleration; it may not tighten this into a correctness limit.
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
    /// Substitution iterations used. Zero for a single-phase state.
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
    /// Successive substitution exhausted its budget.
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
        beta = if next.is_finite() && next > lo && next < hi {
            next
        } else {
            0.5 * (lo + hi)
        };
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

/// Phase compositions from `K`, `z` and `beta`.
///
/// `x_i = z_i / (1 + beta (K_i - 1))`, `y_i = K_i x_i`. An absent component stays exactly absent
/// in both phases — dividing zero by anything finite gives zero, and nothing here adds material.
fn phase_compositions(k: &[f64], z: &[f64], beta: f64) -> (Vec<f64>, Vec<f64>) {
    let n = k.len();
    let mut x = vec![0.0; n];
    let mut y = vec![0.0; n];
    for i in 0..n {
        let d = 1.0 + beta * (k[i] - 1.0);
        x[i] = z[i] / d;
        y[i] = k[i] * x[i];
    }
    (x, y)
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
    let mut beta;
    let mut residual = f64::INFINITY;

    for iteration in 1..=MAX_SUBSTITUTION_ITERATIONS {
        beta = solve_rachford_rice_extended(&k, z)?;
        let (x, y) = phase_compositions(&k, z, beta);

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

            // Successive substitution: K_i <- phi_i^L / phi_i^V. Equilibrium is
            // `x_i phi_i^L = y_i phi_i^V`, so `K_i = y_i / x_i = phi_i^L / phi_i^V` at the fixed
            // point, and the update assigns that ratio rather than multiplying by it. Multiplying
            // is the same expression written with fugacities instead of fugacity coefficients -
            // `K_i *= (x_i phi_i^L)/(y_i phi_i^V)` - and mixing the two forms leaves an extra
            // factor of `K` in the step, which drives `K` away from equilibrium toward the
            // degenerate split into two pure phases instead of toward it.
            k[i] = math::exp(liquid.ln_phi[i] - vapour.ln_phi[i]);
            if !k[i].is_finite() || k[i] <= 0.0 {
                return Err(FlashError::InvalidFeed {
                    reason: "an equilibrium ratio left the physical range",
                });
            }
        }

        if residual < EQUILIBRIUM_TOLERANCE {
            let beta = solve_rachford_rice_extended(&k, z)?;
            // Converged with the vapour fraction outside the physical window: the two phases have
            // collapsed onto one another, and the stability test's instability verdict is not
            // borne out. That is a determination, not a failure — Whitson & Michelsen's negative
            // flash is exactly the test for it — so the single-phase result is returned rather
            // than an error.
            if !(0.0..=1.0).contains(&beta) {
                return single_phase(spec, pressure_pa, temperature_k, z);
            }
            let (x, y) = phase_compositions(&k, z, beta);
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
                beta,
                x: xn,
                y: yn,
                k,
                liquid: Some(liquid),
                vapour: Some(vapour),
                equilibrium_residual: residual,
                iterations: iteration,
            });
        }
    }

    Err(FlashError::NotConverged {
        iterations: MAX_SUBSTITUTION_ITERATIONS,
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
