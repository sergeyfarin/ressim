//! Hydrocarbon liquid/vapour relative permeability (compositional plan, C9).
//!
//! # Why this is a model rather than a constant
//!
//! The plan is explicit that the existing water/oil curves are **not** implicitly a hydrocarbon
//! liquid/vapour law — they were fitted for a different pair of phases — and that critical data
//! and curve parameters may not be invented. No sourced hydrocarbon liquid/vapour table exists in
//! this environment.
//!
//! So V1 runs on [`RelativePermeabilityModel::Linear`], and the resolution is structural: the
//! model is a **choice the caller makes**, not a constant the engine assumes. [`Corey`] and
//! [`Tabulated`] exist alongside it so a benchmark case can supply its own curves, and **neither
//! carries a default**. Every parameter is a required input, validated on construction, so no
//! number in this file describes any particular rock.
//!
//! [`Corey`]: RelativePermeabilityModel::Corey
//! [`Tabulated`]: RelativePermeabilityModel::Tabulated
//!
//! # `Linear` is a verification model, not a physical one
//!
//! `kr_L = S_L`, `kr_V = S_V`, no residual saturations, no endpoint scaling. Straight lines add no
//! fitted parameters, so an error in a displacement front is attributable to the thermodynamics
//! and the discretization rather than to a curve nobody can cite — which is what makes them the
//! right choice for verifying a new transport implementation.
//!
//! They are **not** a description of any real rock, and a real front will be less sharp under them
//! than under a Corey curve. [`RelativePermeabilityModel::is_verification_only`] exists so a
//! scenario-admission check can refuse to ship a case that is still running on them, rather than
//! relying on someone noticing.

use crate::ad::Scalar;

/// A hydrocarbon liquid/vapour relative permeability model.
///
/// **No `Default`, deliberately.** Choosing a relative permeability curve is a modelling decision,
/// and a default would make the most consequential unsourced assumption in the model the one
/// nobody had to type.
#[derive(Clone, Debug, PartialEq)]
pub enum RelativePermeabilityModel {
    /// `kr_p = S_p`. A **verification** model — see the module docs.
    Linear,

    /// Corey-type power law with caller-supplied endpoints and exponents.
    ///
    /// ```text
    /// S_e     = (S_L - S_Lr) / (1 - S_Lr - S_Vr)
    /// kr_L    = kr_L_max * S_e ^ n_L
    /// kr_V    = kr_V_max * (1 - S_e) ^ n_V
    /// ```
    ///
    /// Every field is required. There are no default exponents and no default residual
    /// saturations here, because a Corey curve with made-up parameters is made-up data wearing a
    /// respectable name.
    Corey(CoreyParameters),

    /// A tabulated curve, interpolated linearly in liquid saturation.
    ///
    /// The table is the caller's — from a deck, a benchmark definition or a measurement — and is
    /// validated on construction rather than trusted.
    Tabulated(RelativePermeabilityTable),
}

/// Corey exponents and endpoints. Construct with [`CoreyParameters::new`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoreyParameters {
    liquid_residual: f64,
    vapour_residual: f64,
    liquid_exponent: f64,
    vapour_exponent: f64,
    liquid_endpoint: f64,
    vapour_endpoint: f64,
}

/// A tabulated curve. Construct with [`RelativePermeabilityTable::new`].
#[derive(Clone, Debug, PartialEq)]
pub struct RelativePermeabilityTable {
    /// Liquid saturations, strictly increasing, spanning at least `[0, 1]` when clamped.
    liquid_saturation: Vec<f64>,
    /// `kr_L` at each tabulated saturation.
    kr_liquid: Vec<f64>,
    /// `kr_V` at each tabulated saturation.
    kr_vapour: Vec<f64>,
}

/// Why a relative permeability model could not be constructed.
#[derive(Clone, Debug, PartialEq)]
pub enum RelPermError {
    /// A parameter is non-finite or outside its permitted range.
    InvalidParameter {
        field: &'static str,
        value: f64,
        reason: &'static str,
    },
    /// The residual saturations leave no mobile range.
    NoMobileRange {
        liquid_residual: f64,
        vapour_residual: f64,
    },
    /// A table's columns have different lengths, or it has fewer than two rows.
    TableShape {
        rows: usize,
        kr_liquid: usize,
        kr_vapour: usize,
    },
    /// A table's saturations are not strictly increasing.
    TableNotIncreasing {
        index: usize,
        previous: f64,
        current: f64,
    },
    /// A table entry is non-finite or negative.
    TableEntry {
        column: &'static str,
        index: usize,
        value: f64,
    },
}

impl core::fmt::Display for RelPermError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidParameter {
                field,
                value,
                reason,
            } => {
                write!(f, "{field} = {value} is invalid: {reason}")
            }
            Self::NoMobileRange {
                liquid_residual,
                vapour_residual,
            } => write!(
                f,
                "residual saturations {liquid_residual} and {vapour_residual} leave no mobile range"
            ),
            Self::TableShape {
                rows,
                kr_liquid,
                kr_vapour,
            } => write!(
                f,
                "table has {rows} saturations, {kr_liquid} liquid and {kr_vapour} vapour values; \
                 all three must match and be at least 2"
            ),
            Self::TableNotIncreasing {
                index,
                previous,
                current,
            } => write!(
                f,
                "table saturation {current} at row {index} does not exceed {previous}"
            ),
            Self::TableEntry {
                column,
                index,
                value,
            } => {
                write!(
                    f,
                    "table {column} row {index} = {value} is not finite and non-negative"
                )
            }
        }
    }
}

impl CoreyParameters {
    /// Validate and construct.
    ///
    /// Exponents must be at least one — a Corey exponent below one gives a curve that is convex at
    /// the endpoint and is almost always a transcription error rather than a fluid.
    pub fn new(
        liquid_residual: f64,
        vapour_residual: f64,
        liquid_exponent: f64,
        vapour_exponent: f64,
        liquid_endpoint: f64,
        vapour_endpoint: f64,
    ) -> Result<Self, RelPermError> {
        let check = |field: &'static str, value: f64, lo: f64, hi: f64, reason: &'static str| {
            if !value.is_finite() || value < lo || value > hi {
                Err(RelPermError::InvalidParameter {
                    field,
                    value,
                    reason,
                })
            } else {
                Ok(())
            }
        };
        check(
            "liquid_residual",
            liquid_residual,
            0.0,
            1.0,
            "must lie in [0, 1]",
        )?;
        check(
            "vapour_residual",
            vapour_residual,
            0.0,
            1.0,
            "must lie in [0, 1]",
        )?;
        check(
            "liquid_exponent",
            liquid_exponent,
            1.0,
            100.0,
            "must lie in [1, 100]",
        )?;
        check(
            "vapour_exponent",
            vapour_exponent,
            1.0,
            100.0,
            "must lie in [1, 100]",
        )?;
        check(
            "liquid_endpoint",
            liquid_endpoint,
            0.0,
            1.0,
            "must lie in [0, 1]",
        )?;
        check(
            "vapour_endpoint",
            vapour_endpoint,
            0.0,
            1.0,
            "must lie in [0, 1]",
        )?;

        if liquid_residual + vapour_residual >= 1.0 {
            return Err(RelPermError::NoMobileRange {
                liquid_residual,
                vapour_residual,
            });
        }
        Ok(Self {
            liquid_residual,
            vapour_residual,
            liquid_exponent,
            vapour_exponent,
            liquid_endpoint,
            vapour_endpoint,
        })
    }

    pub fn liquid_residual(&self) -> f64 {
        self.liquid_residual
    }

    pub fn vapour_residual(&self) -> f64 {
        self.vapour_residual
    }
}

impl RelativePermeabilityTable {
    /// Validate and construct from a caller-supplied table.
    pub fn new(
        liquid_saturation: Vec<f64>,
        kr_liquid: Vec<f64>,
        kr_vapour: Vec<f64>,
    ) -> Result<Self, RelPermError> {
        if liquid_saturation.len() < 2
            || liquid_saturation.len() != kr_liquid.len()
            || liquid_saturation.len() != kr_vapour.len()
        {
            return Err(RelPermError::TableShape {
                rows: liquid_saturation.len(),
                kr_liquid: kr_liquid.len(),
                kr_vapour: kr_vapour.len(),
            });
        }
        for (index, window) in liquid_saturation.windows(2).enumerate() {
            if !(window[1] > window[0]) {
                return Err(RelPermError::TableNotIncreasing {
                    index: index + 1,
                    previous: window[0],
                    current: window[1],
                });
            }
        }
        for (column, values) in [
            ("saturation", &liquid_saturation),
            ("kr_liquid", &kr_liquid),
            ("kr_vapour", &kr_vapour),
        ] {
            for (index, &value) in values.iter().enumerate() {
                if !value.is_finite() || value < 0.0 {
                    return Err(RelPermError::TableEntry {
                        column,
                        index,
                        value,
                    });
                }
            }
        }
        Ok(Self {
            liquid_saturation,
            kr_liquid,
            kr_vapour,
        })
    }

    /// Linear interpolation, clamped outside the tabulated range.
    ///
    /// Clamping at the ends rather than extrapolating: a relative permeability extrapolated past
    /// its table is a number the table's author never stood behind, and can easily go negative.
    fn interpolate<S: Scalar>(&self, saturation: S, values: &[f64]) -> S {
        let s = saturation.value();
        if s <= self.liquid_saturation[0] {
            return S::from_f64(values[0]);
        }
        let last = self.liquid_saturation.len() - 1;
        if s >= self.liquid_saturation[last] {
            return S::from_f64(values[last]);
        }
        // Binary search for the bracketing interval, then interpolate in the AD saturation so the
        // derivative comes out with it.
        let mut lo = 0usize;
        let mut hi = last;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if self.liquid_saturation[mid] <= s {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let s0 = self.liquid_saturation[lo];
        let s1 = self.liquid_saturation[hi];
        let slope = (values[hi] - values[lo]) / (s1 - s0);
        (saturation - s0) * slope + values[lo]
    }
}

impl RelativePermeabilityModel {
    /// Relative permeability of the liquid phase at liquid saturation `s_liquid`.
    pub fn kr_liquid<S: Scalar>(&self, s_liquid: S) -> S {
        match self {
            Self::Linear => s_liquid,
            Self::Corey(p) => {
                let s_e = normalized_liquid(s_liquid, p);
                s_e.powf(p.liquid_exponent) * p.liquid_endpoint
            }
            Self::Tabulated(t) => t.interpolate(s_liquid, &t.kr_liquid),
        }
    }

    /// Relative permeability of the vapour phase at liquid saturation `s_liquid`.
    ///
    /// Keyed on the **liquid** saturation for both phases, so one table and one normalization
    /// serve both and the two curves cannot be evaluated at inconsistent saturations.
    pub fn kr_vapour<S: Scalar>(&self, s_liquid: S) -> S {
        match self {
            Self::Linear => S::from_f64(1.0) - s_liquid,
            Self::Corey(p) => {
                let s_e = normalized_liquid(s_liquid, p);
                (S::from_f64(1.0) - s_e).powf(p.vapour_exponent) * p.vapour_endpoint
            }
            Self::Tabulated(t) => t.interpolate(s_liquid, &t.kr_vapour),
        }
    }

    /// Relative permeability of one phase, by whether it is the liquid.
    pub fn kr<S: Scalar>(&self, s_liquid: S, liquid: bool) -> S {
        if liquid {
            self.kr_liquid(s_liquid)
        } else {
            self.kr_vapour(s_liquid)
        }
    }

    /// True when this model is a verification device rather than a description of a rock.
    ///
    /// Exists so a scenario-admission check can refuse to ship a case still running on
    /// [`Self::Linear`], instead of relying on someone noticing.
    pub fn is_verification_only(&self) -> bool {
        matches!(self, Self::Linear)
    }

    /// A short name for diagnostics and reporting.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Corey(_) => "corey",
            Self::Tabulated(_) => "tabulated",
        }
    }
}

/// Normalized liquid saturation, clamped to `[0, 1]`.
///
/// The clamp is on the **normalization**, not on the relative permeability: outside the mobile
/// range the phase is immobile, and `S_e` saturates rather than the curve being cut off. The
/// branch is selected on the value, so the derivative belongs to whichever side it lands on — the
/// same convention the upwind switch uses.
fn normalized_liquid<S: Scalar>(s_liquid: S, p: &CoreyParameters) -> S {
    let span = 1.0 - p.liquid_residual - p.vapour_residual;
    let raw = (s_liquid - p.liquid_residual) / span;
    raw.max_floor(0.0).min_ceil(1.0)
}
