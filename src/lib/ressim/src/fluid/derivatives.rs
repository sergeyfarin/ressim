//! Derivatives of the converged equilibrium (compositional plan, C4).
//!
//! The flash solves a nonlinear system; its answer is a function of `(p, z)`, and the transport
//! equations need that function's derivatives. This module produces them by **implicit
//! differentiation of the converged state**, which is the pattern
//! `/usr/include/opm/material/constraintsolvers/PTFlash.hpp` uses: `solve` obtains equilibrium in
//! scalar arithmetic and `updateDerivatives_` reconstructs the derivatives afterwards.
//!
//! Differentiating the iteration instead — carrying AD through every substitution step — would
//! give derivatives of *the algorithm*, which agree with the true ones only in the limit and
//! depend on the starting guess along the way.
//!
//! # The system
//!
//! Primary variables, in this plan's order:
//!
//! ```text
//! u = [p_Pa, z_0, ..., z_(N-2)]          N entries; z_(N-1) = 1 - sum(z_0..z_(N-2))
//! ```
//!
//! Equilibrium unknowns:
//!
//! ```text
//! y = [ln K_0, ..., ln K_(N-1), beta]    N+1 entries
//! ```
//!
//! and the equations `F(y, u) = 0`:
//!
//! ```text
//! F_i = ln K_i - ln phi_i^L(x) + ln phi_i^V(v)     i = 0 .. N-1   (fugacity equality)
//! F_N = sum_i (v_i - x_i)                                         (Rachford-Rice)
//! ```
//!
//! with `x_i = z_i / (1 + beta (K_i - 1))` and `v_i = K_i x_i`. `ln K` rather than `K` because the
//! equilibrium ratios span many orders of magnitude — 13 to 0.04 for the C1/C10 binary at 20 bar —
//! and the log form keeps the Jacobian's columns comparable. Writing the Rachford–Rice row as
//! `sum(v_i - x_i)` rather than `sum z_i (K_i - 1) / (1 + beta (K_i - 1))` is the same equation
//! with the poles cleared, which matters because the Jacobian is evaluated at `beta` values that
//! can sit close to one.
//!
//! That is `N+1` equations in `N+1` unknowns, and it is square and nonsingular away from a phase
//! boundary and the critical point. Then
//!
//! ```text
//! dy/du = -F_y^{-1} F_u
//! ```
//!
//! computed by a **linear solve**, never by forming an inverse.
//!
//! # How the two passes work
//!
//! 1. Evaluate `F` once with `Ad<2N+1>` seeded over `[ln K, beta, p, z_0..z_(N-2)]`. The first
//!    `N+1` derivative columns are `F_y`; the rest are `F_u`. One evaluation, both blocks, no
//!    hand-written analytic Jacobian to get wrong.
//! 2. Solve for `dy/du`, then re-evaluate every dependent quantity with `Ad<N>` whose seeds are
//!    already the **total** derivatives: `ln K_j` seeded with row `j` of `dy/du`, `beta` with row
//!    `N`, `p` with the unit vector in slot 0, `z_k` with the unit vector in slot `k+1`, and the
//!    dependent `z_(N-1)` with `-1` in every composition slot. Anything computed from those then
//!    carries `d/du` directly, with no second chain-rule pass to write down separately.
//!
//! # The compressibility factor
//!
//! `Z` is a root of the cubic, and the root solver needs inverse trigonometric and hyperbolic
//! functions that the `Scalar` trait does not carry. It does not need to: `Z` satisfies
//! `P(Z; A, B) = 0`, so
//!
//! ```text
//! dZ = -(dP/dA dA + dP/dB dB) / (dP/dZ)
//! ```
//!
//! and evaluating `P` at the fixed root value with AD-valued `A` and `B` yields exactly that
//! numerator. This is both simpler and more accurate than differentiating a trigonometric
//! solution, and `dP/dZ` vanishing is precisely the near-repeated-root condition — which is
//! therefore reported as [`DerivativeError::DegenerateRoot`] rather than producing a large
//! meaningless number.
//!
//! # Single-phase states
//!
//! There is no equilibrium system to differentiate: the phase composition *is* `z`, and the
//! properties are differentiated on that branch alone. Solving a two-phase derivative system at a
//! single-phase state would be solving a singular one.

use super::eos::{
    CubicRoots, EosError, PhaseBranch, ln_fugacity_coefficients_generic, mixture_params_generic,
    z_roots,
};
use super::flash::{FlashState, PhaseState};
use super::specification::FluidSpecification;
use super::units::GAS_CONSTANT_J_PER_MOL_K;
use crate::ad::{Ad, Scalar};

/// Smallest `|dP/dZ|` this module will divide by.
///
/// `dP/dZ -> 0` is the near-repeated-root condition: the liquid and vapour branches are merging
/// and the derivative of the root with respect to the mixture parameters is genuinely unbounded.
/// The threshold is on the derivative of a cubic in a quantity of order one, so it is an absolute
/// tolerance rather than a relative one.
pub const MIN_ROOT_SEPARATION: f64 = 1e-10;

/// Smallest pivot magnitude accepted in the equilibrium solve.
pub const MIN_PIVOT: f64 = 1e-14;

/// Derivatives of a converged flash with respect to `u = [p_Pa, z_0 .. z_(N-2)]`.
///
/// Every field has `n_vars = N` entries per row, indexed in `u` order. Absent phases carry no
/// entries at all, rather than zeros: a zero derivative for a phase that does not exist reads as
/// a real quantity that happens not to be moving.
#[derive(Clone, Debug, PartialEq)]
pub struct FlashDerivatives {
    /// `d beta / du`. Zero-length for a single-phase state, where `beta` is pinned at 0 or 1 and
    /// has no derivative until the phase boundary is crossed.
    pub dbeta: Vec<f64>,
    /// `d x_i / du`, one row per component. Empty when no liquid phase exists.
    pub dx: Vec<Vec<f64>>,
    /// `d y_i / du`, one row per component. Empty when no vapour phase exists.
    pub dy: Vec<Vec<f64>>,
    /// `d cL / du` [mol/m³ per unit of u]. Empty when no liquid phase exists.
    pub dliquid_molar_density: Vec<f64>,
    /// `d cV / du`. Empty when no vapour phase exists.
    pub dvapour_molar_density: Vec<f64>,
    /// `d rho_L / du` [kg/m³ per unit of u].
    pub dliquid_mass_density: Vec<f64>,
    /// `d rho_V / du`.
    pub dvapour_mass_density: Vec<f64>,
    /// Condition diagnostics from the equilibrium solve, for the failure report a mismatch needs.
    pub diagnostics: DerivativeDiagnostics,
}

/// What the solve had to work with, recorded whether or not it succeeded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DerivativeDiagnostics {
    /// Smallest pivot magnitude seen during elimination. Small means an ill-conditioned
    /// equilibrium Jacobian, which is what happens approaching the critical point.
    pub min_pivot: f64,
    /// Smallest `|dP/dZ|` across the phases that exist.
    pub min_root_separation: f64,
    /// Residual norm of the equilibrium equations at the point differentiated. Large means the
    /// flash was not actually converged, and the derivatives describe a state nobody is at.
    pub equilibrium_residual: f64,
}

/// Why derivatives could not be produced.
#[derive(Clone, Debug, PartialEq)]
pub enum DerivativeError {
    /// The component count has no compiled AD instantiation.
    UnsupportedComponentCount { count: usize },
    /// Two cubic roots have merged, so `dZ/dA` and `dZ/dB` are unbounded.
    DegenerateRoot { branch: PhaseBranch, dp_dz: f64 },
    /// The equilibrium Jacobian is singular to working precision.
    SingularEquilibriumJacobian { min_pivot: f64 },
    /// The state handed in is not converged, so its derivatives describe nothing.
    NotAtEquilibrium { residual: f64 },
    /// An EOS evaluation failed while differentiating.
    Eos(EosError),
}

impl core::fmt::Display for DerivativeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedComponentCount { count } => {
                write!(f, "no AD instantiation compiled for {count} components")
            }
            Self::DegenerateRoot { branch, dp_dz } => write!(
                f,
                "the {branch:?} root is degenerate: dP/dZ = {dp_dz:e}, so dZ/dA is unbounded"
            ),
            Self::SingularEquilibriumJacobian { min_pivot } => write!(
                f,
                "the equilibrium Jacobian is singular: smallest pivot {min_pivot:e}"
            ),
            Self::NotAtEquilibrium { residual } => write!(
                f,
                "the state is not converged (residual {residual:e}); its derivatives describe \
                 no physical state"
            ),
            Self::Eos(e) => write!(f, "EOS evaluation failed while differentiating: {e}"),
        }
    }
}

/// The compressibility factor of an already-selected root, carrying derivatives.
///
/// `Z` solves `P(Z; A, B) = 0`, so `dZ = -(dP/dA dA + dP/dB dB) / (dP/dZ)`. Evaluating `P` at the
/// fixed root value with derivative-carrying `A` and `B` produces a quantity whose value is zero
/// to roundoff and whose derivatives are exactly that numerator, so
///
/// ```text
/// Z = z_value - P(z_value; A, B) / (dP/dZ)
/// ```
///
/// is the root with the right derivatives attached, and is written once over the `Scalar` trait
/// rather than twice. With `f64` it reduces to `z_value` minus a roundoff-sized correction.
///
/// `dP/dZ -> 0` is the near-repeated-root condition — the branches are merging and `dZ/dA` really
/// is unbounded — so it is reported rather than divided by.
fn z_factor_with_derivatives<S: Scalar>(
    a: S,
    b: S,
    z_value: f64,
    branch: PhaseBranch,
) -> Result<S, DerivativeError> {
    let sqrt2 = core::f64::consts::SQRT_2;
    let (m1, m2) = (1.0 + sqrt2, 1.0 - sqrt2);

    // The same cubic as `eos::z_roots`.
    let c2 = b * (m1 + m2 - 1.0) - 1.0;
    let c1 = a + b * b * (m1 * m2) - b * (b + 1.0) * (m1 + m2);
    let c0 = -(a * b) - b * b * (b + 1.0) * (m1 * m2);
    let p_at_root = c2 * (z_value * z_value) + c1 * z_value + c0 + (z_value * z_value * z_value);

    let dp_dz = dp_dz_at(a.value(), b.value(), z_value);
    if dp_dz.abs() < MIN_ROOT_SEPARATION {
        return Err(DerivativeError::DegenerateRoot { branch, dp_dz });
    }
    Ok(S::from_f64(z_value) - p_at_root / dp_dz)
}

/// `dP/dZ` at a root, for the diagnostics.
fn dp_dz_at(a: f64, b: f64, z_value: f64) -> f64 {
    let sqrt2 = core::f64::consts::SQRT_2;
    let (m1, m2) = (1.0 + sqrt2, 1.0 - sqrt2);
    let c2 = (m1 + m2 - 1.0) * b - 1.0;
    let c1 = a + m1 * m2 * b * b - (m1 + m2) * b * (b + 1.0);
    3.0 * z_value * z_value + 2.0 * c2 * z_value + c1
}

/// Solve `A w = rhs` in place by Gaussian elimination with partial pivoting.
///
/// A dense solve on an `(N+1) x (N+1)` system with `N <= 4`, so at most `5 x 5`. The plan asks for
/// a linear solve rather than an explicit inverse, and at this size the pivoting is what makes it
/// worth writing rather than the asymptotics. Returns the smallest pivot magnitude seen, which is
/// the conditioning signal the diagnostics report.
fn solve_in_place(matrix: &mut [Vec<f64>], rhs: &mut [Vec<f64>]) -> f64 {
    let n = matrix.len();
    let cols = if rhs.is_empty() { 0 } else { rhs[0].len() };
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
        if pivot.abs() < MIN_PIVOT {
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
            for c in 0..cols {
                rhs[r][c] -= factor * rhs[k][c];
            }
        }
    }

    // Back substitution.
    for k in (0..n).rev() {
        for c in 0..cols {
            let mut acc = rhs[k][c];
            for j in (k + 1)..n {
                acc -= matrix[k][j] * rhs[j][c];
            }
            rhs[k][c] = acc / matrix[k][k];
        }
    }
    min_pivot
}

/// Phase compositions from `ln K`, `z` and `beta`, over any scalar.
///
/// Returned unnormalized: at a converged state `sum x = sum v = 1` already holds, and normalizing
/// would add a derivative of the normalization that does not belong in the equilibrium equations.
fn compositions_from<S: Scalar>(ln_k: &[S], z: &[S], beta: S) -> (Vec<S>, Vec<S>) {
    let n = ln_k.len();
    let mut x = Vec::with_capacity(n);
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let k = ln_k[i].exp();
        let d = (k - 1.0) * beta + 1.0;
        let xi = z[i] / d;
        x.push(xi);
        v.push(k * xi);
    }
    (x, v)
}

/// Derivatives for a two-phase state, with `M = 2N+1` AD slots for pass one and `N` for pass two.
fn two_phase<const N: usize, const M: usize, const NU: usize>(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    state: &FlashState,
) -> Result<FlashDerivatives, DerivativeError> {
    debug_assert_eq!(M, 2 * N + 1);
    debug_assert_eq!(NU, N);

    // ---- Pass one: F, F_y and F_u in a single AD evaluation. Slots:
    //   0 .. N-1   ln K_i          | F_y
    //   N          beta            |
    //   N+1        p               | F_u
    //   N+2 .. 2N  z_0 .. z_(N-2)  |
    let ln_k: Vec<Ad<M>> = (0..N)
        .map(|i| Ad::variable((state.y[i] / state.x[i]).ln(), i))
        .collect();
    let beta = Ad::<M>::variable(state.beta, N);
    let p = Ad::<M>::variable(pressure_pa, N + 1);

    let mut z_ad: Vec<Ad<M>> = Vec::with_capacity(N);
    for (k, &value) in z.iter().enumerate().take(N - 1) {
        z_ad.push(Ad::variable(value, N + 2 + k));
    }
    // The dependent component: z_(N-1) = 1 - sum(z_0..z_(N-2)), so its derivative is -1 in every
    // independent composition slot. Seeding it here is what makes C4's required invariant a
    // property of the construction rather than something to check afterwards.
    let mut last = [0.0; M];
    for k in 0..(N - 1) {
        last[N + 2 + k] = -1.0;
    }
    z_ad.push(Ad::seeded(z[N - 1], last));

    let residual = equilibrium_residual::<Ad<M>>(spec, p, temperature_k, &ln_k, &z_ad, beta)
        .map_err(DerivativeError::Eos)?;

    let residual_norm = residual.iter().map(|r| r.value().abs()).fold(0.0, f64::max);
    if residual_norm > 1e-6 {
        return Err(DerivativeError::NotAtEquilibrium {
            residual: residual_norm,
        });
    }

    // F_y is the first N+1 derivative columns; F_u the remaining N.
    let mut f_y: Vec<Vec<f64>> = (0..=N)
        .map(|row| (0..=N).map(|col| residual[row].d(col)).collect())
        .collect();
    let mut minus_f_u: Vec<Vec<f64>> = (0..=N)
        .map(|row| (0..N).map(|col| -residual[row].d(N + 1 + col)).collect())
        .collect();

    let min_pivot = solve_in_place(&mut f_y, &mut minus_f_u);
    if min_pivot < MIN_PIVOT {
        return Err(DerivativeError::SingularEquilibriumJacobian { min_pivot });
    }
    // `minus_f_u` now holds dy/du: row j is d y_j / du.
    let dy_du = minus_f_u;

    // ---- Pass two: every dependent quantity, seeded with total derivatives.
    let ln_k_t: Vec<Ad<NU>> = (0..N)
        .map(|i| {
            let mut d = [0.0; NU];
            d[..N].copy_from_slice(&dy_du[i][..N]);
            Ad::seeded(state.y[i].ln() - state.x[i].ln(), d)
        })
        .collect();
    let mut beta_d = [0.0; NU];
    beta_d[..N].copy_from_slice(&dy_du[N][..N]);
    let beta_t = Ad::<NU>::seeded(state.beta, beta_d);
    let p_t = Ad::<NU>::variable(pressure_pa, 0);

    let mut z_t: Vec<Ad<NU>> = Vec::with_capacity(N);
    for (k, &value) in z.iter().enumerate().take(N - 1) {
        z_t.push(Ad::variable(value, 1 + k));
    }
    let mut last_t = [0.0; NU];
    for k in 0..(N - 1) {
        last_t[1 + k] = -1.0;
    }
    z_t.push(Ad::seeded(z[N - 1], last_t));

    let (x_raw, v_raw) = compositions_from(&ln_k_t, &z_t, beta_t);
    let (x, v) = normalize_pair(&x_raw, &v_raw);

    let liquid = state.liquid.as_ref().expect("two-phase state has a liquid");
    let vapour = state.vapour.as_ref().expect("two-phase state has a vapour");

    let (cl, rho_l, sep_l) = phase_density_ad::<NU>(
        spec,
        p_t,
        temperature_k,
        &x,
        liquid.z_factor,
        PhaseBranch::Liquid,
    )?;
    let (cv, rho_v, sep_v) = phase_density_ad::<NU>(
        spec,
        p_t,
        temperature_k,
        &v,
        vapour.z_factor,
        PhaseBranch::Vapour,
    )?;

    Ok(FlashDerivatives {
        dbeta: beta_t.deriv()[..N].to_vec(),
        dx: (0..N).map(|i| x[i].deriv()[..N].to_vec()).collect(),
        dy: (0..N).map(|i| v[i].deriv()[..N].to_vec()).collect(),
        dliquid_molar_density: cl.deriv()[..N].to_vec(),
        dvapour_molar_density: cv.deriv()[..N].to_vec(),
        dliquid_mass_density: rho_l.deriv()[..N].to_vec(),
        dvapour_mass_density: rho_v.deriv()[..N].to_vec(),
        diagnostics: DerivativeDiagnostics {
            min_pivot,
            min_root_separation: sep_l.abs().min(sep_v.abs()),
            equilibrium_residual: residual_norm,
        },
    })
}

/// Normalize both phase compositions, carrying derivatives.
///
/// At a converged state both sums are one to roundoff, so this changes no value materially — but
/// it does keep the reported `dx` consistent with the constraint `sum x = 1`, whose derivative
/// must be exactly zero.
fn normalize_pair<const M: usize>(x: &[Ad<M>], v: &[Ad<M>]) -> (Vec<Ad<M>>, Vec<Ad<M>>) {
    let sum_x = x.iter().fold(Ad::<M>::constant(0.0), |acc, e| acc + *e);
    let sum_v = v.iter().fold(Ad::<M>::constant(0.0), |acc, e| acc + *e);
    (
        x.iter().map(|e| *e / sum_x).collect(),
        v.iter().map(|e| *e / sum_v).collect(),
    )
}

/// Molar and mass density of one phase, with derivatives, given its already-selected root.
fn phase_density_ad<const M: usize>(
    spec: &FluidSpecification,
    pressure_pa: Ad<M>,
    temperature_k: f64,
    x: &[Ad<M>],
    z_value: f64,
    branch: PhaseBranch,
) -> Result<(Ad<M>, Ad<M>, f64), DerivativeError> {
    let params = mixture_params_generic::<Ad<M>>(spec, pressure_pa, temperature_k, x)
        .map_err(DerivativeError::Eos)?;
    let separation = dp_dz_at(params.a.value(), params.b.value(), z_value);
    let z = z_factor_with_derivatives(params.a, params.b, z_value, branch)?;

    // c = p / (Z R T), the reciprocal of V_m = Z R T / p.
    let molar_density = pressure_pa / (z * (GAS_CONSTANT_J_PER_MOL_K * temperature_k));

    let mut mean_mw = Ad::<M>::constant(0.0);
    for i in 0..spec.component_count() {
        mean_mw = mean_mw + x[i] * spec.component(i).molar_mass_kg_per_mol;
    }
    Ok((molar_density, molar_density * mean_mw, separation))
}

/// The equilibrium equations `F(y, u)`, over any scalar.
///
/// `N` fugacity-equality rows plus the Rachford–Rice row, in that order.
fn equilibrium_residual<S: Scalar>(
    spec: &FluidSpecification,
    pressure_pa: S,
    temperature_k: f64,
    ln_k: &[S],
    z: &[S],
    beta: S,
) -> Result<Vec<S>, EosError> {
    let n = spec.component_count();
    let (x_raw, v_raw) = compositions_from(ln_k, z, beta);

    let sum_x = x_raw.iter().fold(S::from_f64(0.0), |acc, e| acc + *e);
    let sum_v = v_raw.iter().fold(S::from_f64(0.0), |acc, e| acc + *e);
    let x: Vec<S> = x_raw.iter().map(|e| *e / sum_x).collect();
    let v: Vec<S> = v_raw.iter().map(|e| *e / sum_v).collect();

    let pl = mixture_params_generic::<S>(spec, pressure_pa, temperature_k, &x)?;
    let pv = mixture_params_generic::<S>(spec, pressure_pa, temperature_k, &v)?;

    // Root values come from the scalar state; their derivatives from the cubic, via
    // `z_factor_ad`. Here only the values are needed, because `ln_fugacity_coefficients_generic`
    // receives the AD-valued Z built by the caller's branch.
    let zl = scalar_root(&pl, PhaseBranch::Liquid)?;
    let zv = scalar_root(&pv, PhaseBranch::Vapour)?;

    let ln_phi_l = ln_fugacity_coefficients_generic::<S>(spec, &pl, zl, &x)?;
    let ln_phi_v = ln_fugacity_coefficients_generic::<S>(spec, &pv, zv, &v)?;

    let mut f = Vec::with_capacity(n + 1);
    for i in 0..n {
        f.push(ln_k[i] - ln_phi_l[i] + ln_phi_v[i]);
    }
    let mut rr = S::from_f64(0.0);
    for i in 0..n {
        rr = rr + (v_raw[i] - x_raw[i]);
    }
    f.push(rr);
    Ok(f)
}

/// The compressibility factor for a branch, as a scalar of the same type, with its derivatives
/// supplied by implicit differentiation of the cubic.
fn scalar_root<S: Scalar>(
    params: &super::eos::MixtureParams<S>,
    branch: PhaseBranch,
) -> Result<S, EosError> {
    // Root *selection* uses only values, so the plain cubic solver is enough to find it.
    let plain = super::eos::MixtureParams::<f64> {
        a: params.a.value(),
        b: params.b.value(),
        a_i: params.a_i.iter().map(|v| v.value()).collect(),
        b_i: params.b_i.iter().map(|v| v.value()).collect(),
        a_ij: params
            .a_ij
            .iter()
            .map(|row| row.iter().map(|v| v.value()).collect())
            .collect(),
    };
    let roots = z_roots(&plain)?;
    // The same admissibility rule `eos::evaluate` uses, reached through the same function rather
    // than restated: a derivative taken at a different root from the one the flash converged on
    // would be a derivative of a different state.
    let value = super::eos::select_root(&roots, plain.b, branch)?;

    z_factor_with_derivatives(params.a, params.b, value, branch).map_err(|e| match e {
        DerivativeError::DegenerateRoot { dp_dz, .. } => EosError::NotFinite {
            what: "dP/dZ at the selected root",
            value: dp_dz,
        },
        // `z_factor_with_derivatives` returns no other variant.
        _ => EosError::NotFinite {
            what: "compressibility factor derivative",
            value: f64::NAN,
        },
    })
}

/// Derivatives for a single-phase state.
///
/// No equilibrium system exists, so none is solved. The present phase's composition is `z`, whose
/// derivatives with respect to `u` are the identity in the composition slots and `-1` in the
/// dependent row, and its density is differentiated on its own branch.
fn single_phase<const N: usize, const NU: usize>(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    state: &FlashState,
) -> Result<FlashDerivatives, DerivativeError> {
    let p_t = Ad::<NU>::variable(pressure_pa, 0);
    let mut z_t: Vec<Ad<NU>> = Vec::with_capacity(N);
    for (k, &value) in z.iter().enumerate().take(N - 1) {
        z_t.push(Ad::variable(value, 1 + k));
    }
    let mut last = [0.0; NU];
    for k in 0..(N - 1) {
        last[1 + k] = -1.0;
    }
    z_t.push(Ad::seeded(z[N - 1], last));

    let (branch, props) = match state.phase_state {
        PhaseState::SingleLiquid => (
            PhaseBranch::Liquid,
            state.liquid.as_ref().expect("single liquid"),
        ),
        PhaseState::SingleVapour => (
            PhaseBranch::Vapour,
            state.vapour.as_ref().expect("single vapour"),
        ),
        PhaseState::TwoPhase => unreachable!("single_phase called on a two-phase state"),
    };

    let (c, rho, separation) =
        phase_density_ad::<NU>(spec, p_t, temperature_k, &z_t, props.z_factor, branch)?;

    let dz: Vec<Vec<f64>> = (0..N).map(|i| z_t[i].deriv()[..N].to_vec()).collect();
    let liquid_present = branch == PhaseBranch::Liquid;

    Ok(FlashDerivatives {
        // beta is pinned at an endpoint and has no derivative there; an empty vector says that,
        // where a vector of zeros would read as a quantity that happens not to be moving.
        dbeta: Vec::new(),
        dx: if liquid_present {
            dz.clone()
        } else {
            Vec::new()
        },
        dy: if liquid_present { Vec::new() } else { dz },
        dliquid_molar_density: if liquid_present {
            c.deriv()[..N].to_vec()
        } else {
            Vec::new()
        },
        dvapour_molar_density: if liquid_present {
            Vec::new()
        } else {
            c.deriv()[..N].to_vec()
        },
        dliquid_mass_density: if liquid_present {
            rho.deriv()[..N].to_vec()
        } else {
            Vec::new()
        },
        dvapour_mass_density: if liquid_present {
            Vec::new()
        } else {
            rho.deriv()[..N].to_vec()
        },
        diagnostics: DerivativeDiagnostics {
            min_pivot: f64::INFINITY,
            min_root_separation: separation.abs(),
            equilibrium_residual: 0.0,
        },
    })
}

/// Differentiate a converged flash with respect to `u = [p_Pa, z_0 .. z_(N-2)]`.
///
/// `state` must be the result of [`super::flash::flash`] at the same `(p, T, z)`. The two-phase
/// path checks that by evaluating the equilibrium residual before differentiating, and refuses a
/// state that is not converged rather than returning derivatives of nothing.
///
/// Component counts are dispatched to explicit AD instantiations rather than computed from a
/// const-generic expression, which the plan's C7 requires and current Rust does not support.
pub fn flash_derivatives(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    z: &[f64],
    state: &FlashState,
) -> Result<FlashDerivatives, DerivativeError> {
    let n = spec.component_count();
    match state.phase_state {
        PhaseState::TwoPhase => match n {
            2 => two_phase::<2, 5, 2>(spec, pressure_pa, temperature_k, z, state),
            3 => two_phase::<3, 7, 3>(spec, pressure_pa, temperature_k, z, state),
            4 => two_phase::<4, 9, 4>(spec, pressure_pa, temperature_k, z, state),
            count => Err(DerivativeError::UnsupportedComponentCount { count }),
        },
        _ => match n {
            2 => single_phase::<2, 2>(spec, pressure_pa, temperature_k, z, state),
            3 => single_phase::<3, 3>(spec, pressure_pa, temperature_k, z, state),
            4 => single_phase::<4, 4>(spec, pressure_pa, temperature_k, z, state),
            count => Err(DerivativeError::UnsupportedComponentCount { count }),
        },
    }
}
