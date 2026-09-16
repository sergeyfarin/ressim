//! Peng–Robinson mixture EOS and single-phase properties (compositional plan, C2).
//!
//! Scalar, allocation-light, no phase equilibrium. Given a pressure, a temperature and **one**
//! phase composition, this module produces that phase's compressibility factor, molar volume,
//! densities and log fugacity coefficients. Deciding *which* phase is present, or whether two
//! coexist, is C3's job — this module only ever labels an algebraic root.
//!
//! That distinction is the reason `PhaseBranch` is called a branch rather than a phase. The cubic
//! can have three real roots at a state where only one phase is stable; selecting the largest and
//! smallest of them yields two consistent sets of properties, and neither the count of roots nor
//! the fact that both evaluate cleanly says anything about stability.
//!
//! # Source
//!
//! Every formula is from the installed OPM 2026.04 headers, transcribed rather than rederived so
//! that `opm/compositional/ptflash_fixtures.json` and this module describe one EOS:
//!
//! * `eos/PRParams.hpp` — `Omega_A`, `Omega_B`, `m1`, `m2`, `f(w)`
//! * `eos/CubicEOSParams.hpp` — `A_i`, `B_i`, the `a_ij` mixing rule, mixture `A` and `B`
//! * `eos/CubicEOS.hpp` — the cubic coefficients, root labelling, fugacity coefficient
//! * `common/PolynomialUtils.hpp` — the trigonometric/hyperbolic cubic solver
//!
//! Two deliberate divergences from OPM, both required by the plan and both tested:
//!
//! * **No clamping.** OPM clamps `phi` into `[1e-10, 1e10]`, clamps mole fractions into `[0,1]`
//!   inside the mixing rule, and floors the molar volume at `1e-7`. None of that is reproduced:
//!   an out-of-domain state returns a typed error, because a clamped reference value is not a
//!   reference value. Tests confirm no fixture state comes near any of those limits, so the
//!   divergence cannot explain a comparison failure.
//! * **The gas constant.** OPM uses the superseded `8.314472`; this crate uses the exact SI
//!   value. `A` and `B` contain no `R`, so `Z` and `ln phi` are unaffected; only molar volume and
//!   density carry it, linearly. See [`super::units::opm_density_to_si`].

use super::specification::{EosVariant, FluidSpecification};
use super::units::GAS_CONSTANT_J_PER_MOL_K;

/// Which root of the cubic to take.
///
/// A *label*, not a stability claim. See the module docs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseBranch {
    /// The smallest admissible root when three exist.
    Liquid,
    /// The largest admissible root when three exist.
    Vapour,
}

/// Why an EOS evaluation could not produce a physical result.
#[derive(Clone, Debug, PartialEq)]
pub enum EosError {
    /// Pressure or temperature is not finite and strictly positive.
    NonPhysicalState {
        pressure_pa: f64,
        temperature_k: f64,
    },
    /// A phase composition has the wrong length.
    CompositionLength { expected: usize, actual: usize },
    /// A phase composition entry is non-finite or negative.
    CompositionEntry { index: usize, value: f64 },
    /// A phase composition does not sum to one within tolerance.
    CompositionSum { sum: f64, tolerance: f64 },
    /// The mixture `A` or `B` is not finite and positive.
    DegenerateMixtureParameters { a: f64, b: f64 },
    /// The cubic solver produced no root satisfying `Z > B`, so `ln(Z - B)` has no real value.
    ///
    /// This is the covolume constraint: the fluid cannot occupy less volume than its molecules do.
    NoAdmissibleRoot { roots: Vec<f64>, b: f64 },
    /// The fugacity expression's logarithm argument is non-positive.
    LogDomain { what: &'static str, argument: f64 },
    /// A computed quantity is not finite for a reason the checks above did not catch.
    NotFinite { what: &'static str, value: f64 },
}

impl core::fmt::Display for EosError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NonPhysicalState {
                pressure_pa,
                temperature_k,
            } => write!(
                f,
                "p = {pressure_pa} Pa, T = {temperature_k} K is not a finite positive state"
            ),
            Self::CompositionLength { expected, actual } => {
                write!(
                    f,
                    "phase composition has {actual} entries, expected {expected}"
                )
            }
            Self::CompositionEntry { index, value } => {
                write!(f, "phase composition entry {index} = {value} is invalid")
            }
            Self::CompositionSum { sum, tolerance } => {
                write!(
                    f,
                    "phase composition sums to {sum}, outside 1 +/- {tolerance}"
                )
            }
            Self::DegenerateMixtureParameters { a, b } => {
                write!(f, "mixture parameters A = {a}, B = {b} are degenerate")
            }
            Self::NoAdmissibleRoot { roots, b } => write!(
                f,
                "no cubic root satisfies Z > B = {b}; roots were {roots:?}"
            ),
            Self::LogDomain { what, argument } => {
                write!(
                    f,
                    "log argument for {what} is {argument}, which is not positive"
                )
            }
            Self::NotFinite { what, value } => write!(f, "{what} evaluated to {value}"),
        }
    }
}

/// Tolerance on `sum(x) - 1` for a phase composition handed to the EOS.
///
/// Looser than the overall-composition tolerance in `specification.rs` on purpose: a phase
/// composition arrives from a flash iterate, where the normalization is a computed result rather
/// than an input, and demanding 1e-10 of an intermediate would reject states the flash is entitled
/// to pass through. It is still tight enough that a genuinely unnormalized vector is rejected.
pub const PHASE_COMPOSITION_SUM_TOLERANCE: f64 = 1.0e-9;

/// The dimensionless PR parameters of a mixture at one `(p, T, x)`.
///
/// `A` and `B` are the usual dimensionless groups; `a_i`/`b_i` are the per-component ones, and
/// `a_ij` the mixed matrix. All are `R`-independent: `A_i = Omega_A * p_r / T_r^2` and
/// `B_i = Omega_B * p_r / T_r` contain only reduced pressure and temperature.
#[derive(Clone, Debug, PartialEq)]
pub struct MixtureParams {
    pub a: f64,
    pub b: f64,
    pub a_i: Vec<f64>,
    pub b_i: Vec<f64>,
    pub a_ij: Vec<Vec<f64>>,
}

/// The real roots of the cubic, ascending.
#[derive(Clone, Debug, PartialEq)]
pub enum CubicRoots {
    One(f64),
    Three([f64; 3]),
}

impl CubicRoots {
    pub fn as_slice(&self) -> &[f64] {
        match self {
            Self::One(z) => core::slice::from_ref(z),
            Self::Three(z) => z,
        }
    }

    /// True when two roots are within `tol` of each other.
    ///
    /// A near-repeated pair is where the liquid and vapour labels stop being distinguishable, and
    /// where a derivative taken across the pair is meaningless. C2 only reports it; C3 and C4
    /// decide what to do about it.
    pub fn has_near_repeated(&self, tol: f64) -> bool {
        match self {
            Self::One(_) => false,
            Self::Three(z) => (z[1] - z[0]).abs() < tol || (z[2] - z[1]).abs() < tol,
        }
    }
}

/// A fully evaluated single-phase EOS state.
#[derive(Clone, Debug, PartialEq)]
pub struct PhaseProperties {
    pub branch: PhaseBranch,
    pub z_factor: f64,
    /// [m³/mol]
    pub molar_volume: f64,
    /// [mol/m³]
    pub molar_density: f64,
    /// [kg/m³]
    pub mass_density: f64,
    /// Mean molar mass of this phase [kg/mol].
    pub mean_molar_mass: f64,
    /// Natural log of each component's fugacity coefficient.
    ///
    /// Stored as the log, not `phi`: the log is what equilibrium is expressed in, it is what stays
    /// representable when `phi` underflows, and exponentiating it is the caller's choice.
    pub ln_phi: Vec<f64>,
    pub params: MixtureParams,
    pub roots: CubicRoots,
}

impl PhaseProperties {
    /// `phi_i`, for callers that want the coefficient rather than its log.
    pub fn fugacity_coefficient(&self, i: usize) -> f64 {
        self.ln_phi[i].exp()
    }

    /// `f_i = x_i * phi_i * p`, in Pa.
    pub fn fugacity_pa(&self, i: usize, x_i: f64, pressure_pa: f64) -> f64 {
        x_i * self.fugacity_coefficient(i) * pressure_pa
    }
}

/// `f(w)` for unmodified Peng–Robinson. `eos/PRParams.hpp`.
///
/// The `PRCORR` branch of that header swaps this for a quartic when `w > 0.49`. V1 pins `PR`, so
/// this is the only branch, and [`EosVariant`] has one variant for the same reason.
fn pr_f_omega(acentric_factor: f64) -> f64 {
    0.37464 + acentric_factor * (1.54226 + acentric_factor * (-0.26992))
}

/// `Omega_A(T, i)`. `eos/PRParams.hpp::calcOmegaA`.
fn pr_omega_a(reduced_temperature: f64, acentric_factor: f64) -> f64 {
    let tmp = 1.0 + pr_f_omega(acentric_factor) * (1.0 - reduced_temperature.sqrt());
    0.457_235_529 * tmp * tmp
}

/// `Omega_B`. `eos/PRParams.hpp::calcOmegaB`.
const PR_OMEGA_B: f64 = 0.077_796_074;

/// `m1 = 1 + sqrt(2)`, `m2 = 1 - sqrt(2)`. `eos/PRParams.hpp::calcm1`, `calcm2`.
fn pr_m1_m2() -> (f64, f64) {
    let s = core::f64::consts::SQRT_2;
    (1.0 + s, 1.0 - s)
}

/// Solve `a z^3 + b z^2 + c z + d = 0` for its real roots, ascending.
///
/// Transcribed from `common/PolynomialUtils.hpp::cubicRoots`, including its use of the
/// trigonometric form for three real roots and the hyperbolic form for one. The leading
/// coefficient is always 1 here, so that header's quadratic fallback cannot be reached and is not
/// reproduced.
///
/// Returns `None` only when the depressed cubic is degenerate in a way the source throws on.
fn cubic_roots(b: f64, c: f64, d: f64) -> Option<CubicRoots> {
    let a = 1.0;
    let p = (3.0 * a * c - b * b) / (3.0 * a * a);
    let q = (2.0 * b * b * b - 9.0 * a * b * c + 27.0 * d * a * a) / (27.0 * a * a * a);
    let shift = -b / (3.0 * a);

    let discriminant = 4.0 * p * p * p + 27.0 * q * q;

    if discriminant < 0.0 {
        // Three distinct real roots: the trigonometric solution. `p < 0` is implied by
        // `discriminant < 0`, so `sqrt(-3/p)` and `sqrt(-p/3)` are both real.
        let theta = (1.0 / 3.0) * (((3.0 * q) / (2.0 * p)) * (-3.0 / p).sqrt()).acos();
        let r = 2.0 * (-p / 3.0).sqrt();
        let two_pi_3 = 2.0 * core::f64::consts::PI / 3.0;
        let mut z = [
            r * theta.cos() + shift,
            r * (theta - two_pi_3).cos() + shift,
            r * (theta - 2.0 * two_pi_3).cos() + shift,
        ];
        z.sort_by(|l, r| l.partial_cmp(r).expect("cubic roots are finite here"));
        Some(CubicRoots::Three(z))
    } else if discriminant > 0.0 {
        // One real root: the hyperbolic solution, branching on the sign of p.
        let t = if p < 0.0 {
            let theta = (1.0 / 3.0) * (((-3.0 * q.abs()) / (2.0 * p)) * (-3.0 / p).sqrt()).acosh();
            ((-2.0 * q.abs()) / q) * (-p / 3.0).sqrt() * theta.cosh()
        } else if p > 0.0 {
            let theta = (1.0 / 3.0) * (((3.0 * q) / (2.0 * p)) * (3.0 / p).sqrt()).asinh();
            -2.0 * (p / 3.0).sqrt() * theta.sinh()
        } else {
            // p == 0 with a positive discriminant forces q != 0; the source throws here.
            return None;
        };
        Some(CubicRoots::One(t + shift))
    } else if p == 0.0 {
        // Triple root.
        Some(CubicRoots::Three([shift, shift, shift]))
    } else {
        // A simple root and a double root.
        let mut z = [
            (3.0 * q / p) + shift,
            (-3.0 * q) / (2.0 * p) + shift,
            (-3.0 * q) / (2.0 * p) + shift,
        ];
        z.sort_by(|l, r| l.partial_cmp(r).expect("cubic roots are finite here"));
        Some(CubicRoots::Three(z))
    }
}

/// Dimensionless PR parameters for one phase composition.
///
/// `x` must be that phase's mole fractions, not the overall composition — they coincide only in a
/// single-phase state.
pub fn mixture_params(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    x: &[f64],
) -> Result<MixtureParams, EosError> {
    let EosVariant::PengRobinson = spec.eos();

    if !pressure_pa.is_finite()
        || pressure_pa <= 0.0
        || !temperature_k.is_finite()
        || temperature_k <= 0.0
    {
        return Err(EosError::NonPhysicalState {
            pressure_pa,
            temperature_k,
        });
    }
    let n = spec.component_count();
    if x.len() != n {
        return Err(EosError::CompositionLength {
            expected: n,
            actual: x.len(),
        });
    }
    let mut sum = 0.0;
    for (index, &value) in x.iter().enumerate() {
        if !value.is_finite() || value < 0.0 {
            return Err(EosError::CompositionEntry { index, value });
        }
        sum += value;
    }
    if (sum - 1.0).abs() > PHASE_COMPOSITION_SUM_TOLERANCE {
        return Err(EosError::CompositionSum {
            sum,
            tolerance: PHASE_COMPOSITION_SUM_TOLERANCE,
        });
    }

    let mut a_i = Vec::with_capacity(n);
    let mut b_i = Vec::with_capacity(n);
    for i in 0..n {
        let c = spec.component(i);
        let p_r = pressure_pa / c.critical_pressure_pa;
        let t_r = temperature_k / c.critical_temperature_k;
        let omega_a = pr_omega_a(t_r, c.acentric_factor);
        a_i.push(omega_a * p_r / (t_r * t_r));
        b_i.push(PR_OMEGA_B * p_r / t_r);
    }

    // a_ij = sqrt(a_i a_j) (1 - k_ij). `CubicEOSParams.hpp::updateACache_`. Note this mixes the
    // already-dimensionless A_i, not the dimensional attraction parameters; the two differ by a
    // factor that is identical for every component and therefore cancels out of the square root.
    let mut a_ij = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            a_ij[i][j] = (a_i[i] * a_i[j]).sqrt() * (1.0 - spec.interaction(i, j));
        }
    }

    // A = sum_i sum_j x_i x_j a_ij, B = sum_i x_i b_i. OPM clamps each x into [0,1] first; the
    // composition was validated above, so there is nothing to clamp.
    let mut a = 0.0;
    let mut b = 0.0;
    for i in 0..n {
        for j in 0..n {
            a += x[i] * x[j] * a_ij[i][j];
        }
        b += x[i] * b_i[i];
    }

    if !a.is_finite() || !b.is_finite() || a <= 0.0 || b <= 0.0 {
        return Err(EosError::DegenerateMixtureParameters { a, b });
    }
    Ok(MixtureParams {
        a,
        b,
        a_i,
        b_i,
        a_ij,
    })
}

/// Real roots of the PR cubic in `Z` for the given mixture parameters.
///
/// `Z^3 + [(m1 + m2 - 1) B - 1] Z^2 + [A + m1 m2 B^2 - (m1 + m2) B (B + 1)] Z
///      - [A B + m1 m2 B^2 (B + 1)] = 0`, from `eos/CubicEOS.hpp::computeMolarVolume`.
/// For PR, `m1 + m2 = 2` and `m1 m2 = -1`.
pub fn z_roots(params: &MixtureParams) -> Result<CubicRoots, EosError> {
    let (m1, m2) = pr_m1_m2();
    let (a, b) = (params.a, params.b);
    let c2 = (m1 + m2 - 1.0) * b - 1.0;
    let c1 = a + m1 * m2 * b * b - (m1 + m2) * b * (b + 1.0);
    let c0 = -a * b - m1 * m2 * b * b * (b + 1.0);

    cubic_roots(c2, c1, c0).ok_or(EosError::DegenerateMixtureParameters { a, b })
}

/// Pick the root for a branch, requiring `Z > B`.
///
/// Three roots: largest for vapour, smallest for liquid. One root: that root for both, which is
/// how a single-phase state ends up reporting identical properties on both branches — the fixture
/// shows exactly that. `eos/CubicEOS.hpp::computeMolarVolume`.
///
/// Where OPM would floor the molar volume at `1e-7`, this returns [`EosError::NoAdmissibleRoot`].
/// `Z > B` is the covolume constraint; below it `ln(Z - B)` has no real value and the state is not
/// a fluid.
fn select_root(roots: &CubicRoots, b: f64, branch: PhaseBranch) -> Result<f64, EosError> {
    let candidate = match (roots, branch) {
        (CubicRoots::One(z), _) => *z,
        (CubicRoots::Three(z), PhaseBranch::Liquid) => z[0],
        (CubicRoots::Three(z), PhaseBranch::Vapour) => z[2],
    };
    if !candidate.is_finite() {
        return Err(EosError::NotFinite {
            what: "compressibility factor",
            value: candidate,
        });
    }
    if candidate <= b {
        // Fall back to the other admissible root if there is one: with three roots the smallest is
        // sometimes below the covolume while the largest is not, and rejecting the whole state in
        // that case would discard a perfectly good vapour branch.
        if let CubicRoots::Three(z) = roots {
            if let Some(&alt) = z.iter().rev().find(|&&r| r > b) {
                if branch == PhaseBranch::Liquid {
                    // The liquid label has no admissible root of its own here.
                    return Err(EosError::NoAdmissibleRoot {
                        roots: z.to_vec(),
                        b,
                    });
                }
                return Ok(alt);
            }
        }
        return Err(EosError::NoAdmissibleRoot {
            roots: roots.as_slice().to_vec(),
            b,
        });
    }
    Ok(candidate)
}

/// Evaluate one phase branch: compressibility factor, volume, densities and log fugacities.
pub fn evaluate(
    spec: &FluidSpecification,
    pressure_pa: f64,
    temperature_k: f64,
    x: &[f64],
    branch: PhaseBranch,
) -> Result<PhaseProperties, EosError> {
    let params = mixture_params(spec, pressure_pa, temperature_k, x)?;
    let roots = z_roots(&params)?;
    let z = select_root(&roots, params.b, branch)?;

    let rt = GAS_CONSTANT_J_PER_MOL_K * temperature_k;
    let molar_volume = z * rt / pressure_pa;
    if !molar_volume.is_finite() || molar_volume <= 0.0 {
        return Err(EosError::NotFinite {
            what: "molar volume",
            value: molar_volume,
        });
    }
    let molar_density = 1.0 / molar_volume;

    let mean_molar_mass: f64 = (0..spec.component_count())
        .map(|i| x[i] * spec.component(i).molar_mass_kg_per_mol)
        .sum();
    let mass_density = super::units::mass_density_kg_per_m3(molar_density, mean_molar_mass);

    let ln_phi = ln_fugacity_coefficients(spec, &params, z, x)?;

    Ok(PhaseProperties {
        branch,
        z_factor: z,
        molar_volume,
        molar_density,
        mass_density,
        mean_molar_mass,
        ln_phi,
        params,
        roots,
    })
}

/// `ln phi_i` for every component. `eos/CubicEOS.hpp::computeFugacityCoefficient`.
///
/// ```text
/// alpha = -ln(Z - B) + (Bi/B)(Z - 1)
/// beta  = ln((Z + m2 B)/(Z + m1 B)) * A / ((m1 - m2) B)
/// gamma = (2/A) sum_j a_ij x_j - Bi/B
/// ln phi_i = alpha + beta * gamma
/// ```
///
/// Both logarithm arguments are checked rather than clamped. `Z - B > 0` is the covolume
/// constraint, already enforced by root selection but rechecked here because this function is
/// public and may be called with a `Z` that did not come from [`select_root`]. `Z + m2 B > 0` is
/// the weaker of the two attraction-term bounds — `m2 = 1 - sqrt(2)` is negative, so this is the
/// argument that can go non-positive first.
pub fn ln_fugacity_coefficients(
    spec: &FluidSpecification,
    params: &MixtureParams,
    z: f64,
    x: &[f64],
) -> Result<Vec<f64>, EosError> {
    let (m1, m2) = pr_m1_m2();
    let (a, b) = (params.a, params.b);
    let n = spec.component_count();

    let z_minus_b = z - b;
    if !(z_minus_b > 0.0) {
        return Err(EosError::LogDomain {
            what: "Z - B",
            argument: z_minus_b,
        });
    }
    let upper = z + m2 * b;
    let lower = z + m1 * b;
    if !(upper > 0.0) {
        return Err(EosError::LogDomain {
            what: "Z + m2*B",
            argument: upper,
        });
    }
    if !(lower > 0.0) {
        return Err(EosError::LogDomain {
            what: "Z + m1*B",
            argument: lower,
        });
    }

    let beta = (upper / lower).ln() * a / ((m1 - m2) * b);

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let bi_over_b = params.b_i[i] / b;
        let a_sum: f64 = (0..n).map(|j| params.a_ij[i][j] * x[j]).sum();
        let alpha = -z_minus_b.ln() + bi_over_b * (z - 1.0);
        let gamma = (2.0 / a) * a_sum - bi_over_b;
        let value = alpha + beta * gamma;
        if !value.is_finite() {
            return Err(EosError::NotFinite {
                what: "ln phi",
                value,
            });
        }
        out.push(value);
    }
    Ok(out)
}
