//! Component molar flux across a face (compositional plan, C9).
//!
//! # The equation
//!
//! For each phase `P`, at zero hydrocarbon capillary pressure:
//!
//! ```text
//! grav_P     = rho_P(avg) * 9.80665 * (depth_i - depth_j) * 1e-5   [bar]
//! dphi_P     = (p_i - p_j) - grav_P                                 [bar]
//! q_P        = geom_t * (kr_P / mu_P)|upstream(P) * dphi_P          [m3/day]
//! flux_i     = sum_P q_P * c_P|upstream(P) * x_(P,i)|upstream(P)    [mol/day]
//! ```
//!
//! `geom_t` is the precomputed `DARCY_METRIC_FACTOR * geometric_transmissibility` the black-oil
//! path already uses — the same geometry and the same metric conversion, in m³/day/bar per unit of
//! mobility in 1/cP. Reusing it rather than re-deriving a conversion is the point: a compositional
//! model with its own Darcy constant would disagree with the black-oil one for no physical reason.
//!
//! Positive flux is from cell `i` to cell `j`. The contribution is inserted with opposite signs
//! into the two cells' residuals, so an internal face conserves by construction rather than by
//! arithmetic that happens to cancel.
//!
//! # Gravity
//!
//! Off by default and enabled per face through [`Gravity`], matching V1's scope. The head uses
//! **mass** density, `rho = c_molar * sum(x_i MW_i)` — passing the molar density instead is
//! dimensionally undetectable and wrong by three orders of magnitude, which is why
//! [`crate::fluid::units::mass_density_kg_per_m3`] is a named function.
//!
//! The constant and the sign convention are `fim/flux.rs::gravity_head_generic`'s, reproduced
//! exactly: `9.80665 * (depth_i - depth_j) * 1e-5` with depth increasing downward, so a deeper
//! cell sits at higher pressure at equilibrium. A compositional model that disagreed with the
//! black-oil one about which way is down would be a remarkable bug to find later.
//!
//! **With gravity on, the two phases have different potentials and therefore different upstream
//! sides.** A vapour can rise through a face while the liquid falls through it. That is why the
//! upstream side is per phase rather than per face, and why it stays per phase even at zero
//! gravity where the two happen to agree.
//!
//! When a phase is present in only one of the two cells, the head uses that cell's density alone;
//! averaging against an absent phase's density would be averaging against a number that does not
//! describe anything.
//!
//! # Zero capillary pressure
//!
//! A V1 scope decision, not a simplification made here: unequal phase pressures need a separately
//! derived equilibrium contract and cannot be enabled by passing an old flag.
//!
//! # Upstream weighting
//!
//! The branch is selected on the **value** of `dphi` and then frozen for the whole Jacobian
//! evaluation, matching `fim/flux.rs`'s `dphi >= 0.0` convention exactly. Freezing it is what
//! makes the Jacobian the derivative of a differentiable function: the upwind switch is not
//! differentiable at `dphi = 0`, and a Jacobian that tried to differentiate through it would be
//! differentiating a discontinuity.
//!
//! # Relative permeability
//!
//! Supplied by the caller as a [`RelativePermeabilityModel`], never assumed here. V1 runs on its
//! `Linear` variant, which is a verification device rather than a description of a rock; see that
//! module's docs and `docs/COMPOSITIONAL_VALIDATION.md` §6.

use crate::ad::{Ad, Scalar};
use crate::fluid::derivatives::{DerivativeError, FlashDerivatives, flash_derivatives};
use crate::fluid::flash::{FlashError, FlashState, PhaseState, flash};
use crate::fluid::specification::FluidSpecification;
use crate::fluid::transport::{TransportError, lbc_viscosity};
use crate::fluid::units::{bar_to_pa, dpa_to_dbar};

use super::relperm::RelativePermeabilityModel;
use super::state::CompositionalCellState;

/// Why a face flux could not be produced.
#[derive(Clone, Debug, PartialEq)]
pub enum FluxError {
    /// A cell's flash failed.
    Flash { cell: usize, source: FlashError },
    /// A cell's flash could not be differentiated.
    Derivative {
        cell: usize,
        source: DerivativeError,
    },
    /// A viscosity could not be evaluated.
    Transport { cell: usize, source: TransportError },
    /// No AD instantiation exists for this component count.
    UnsupportedComponentCount { count: usize },
    /// The geometric transmissibility is negative or not finite.
    InvalidTransmissibility { value: f64 },
}

impl core::fmt::Display for FluxError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Flash { cell, source } => write!(f, "cell {cell}: flash failed: {source}"),
            Self::Derivative { cell, source } => {
                write!(f, "cell {cell}: derivatives failed: {source}")
            }
            Self::Transport { cell, source } => write!(f, "cell {cell}: {source}"),
            Self::UnsupportedComponentCount { count } => {
                write!(f, "no AD instantiation compiled for {count} components")
            }
            Self::InvalidTransmissibility { value } => {
                write!(
                    f,
                    "geometric transmissibility {value} is not finite and non-negative"
                )
            }
        }
    }
}

/// Standard gravity [m/s²], as `fim/flux.rs` uses it.
pub const STANDARD_GRAVITY: f64 = 9.80665;

/// Gravity configuration for one face.
///
/// Depths increase downward, in metres, and are the cell-centre depths of the two neighbours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gravity {
    pub enabled: bool,
    pub depth_i_m: f64,
    pub depth_j_m: f64,
}

impl Gravity {
    /// Gravity off — V1's default, and what every case before the gravity subtask uses.
    pub const OFF: Self = Self {
        enabled: false,
        depth_i_m: 0.0,
        depth_j_m: 0.0,
    };

    /// Gravity on, between two cell-centre depths.
    pub fn between(depth_i_m: f64, depth_j_m: f64) -> Self {
        Self {
            enabled: true,
            depth_i_m,
            depth_j_m,
        }
    }

    /// `9.80665 * (depth_i - depth_j) * 1e-5`, the bar-per-(kg/m³) head coefficient.
    fn head_coefficient(&self) -> f64 {
        if !self.enabled {
            return 0.0;
        }
        STANDARD_GRAVITY * (self.depth_i_m - self.depth_j_m) * 1e-5
    }
}

/// Which side of a face a phase is drawn from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpstreamSide {
    /// Cell `i`, the first of the pair.
    First,
    /// Cell `j`, the second.
    Second,
}

/// One face's component molar flux and its dependence on both neighbours.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceFlux {
    /// Component molar flux from cell `i` to cell `j` [mol/day]. Positive means `i` loses.
    pub component_moles_per_day: Vec<f64>,
    /// `d flux_i / d u`, row-major `N x 2N`.
    ///
    /// Columns `0..N` are cell `i`'s primaries `[p_bar, z_0..z_(N-2)]`, columns `N..2N` are cell
    /// `j`'s. This is the `2N`-seed face kernel the plan's C7 specifies.
    pub jacobian: Vec<Vec<f64>>,
    /// The frozen upstream side, per phase: `[liquid, vapour]`. With gravity on these can differ.
    pub upstream: [UpstreamSide; 2],
    /// Per-phase potential difference `[liquid, vapour]` [bar]. Equal at zero gravity.
    pub phase_potential_bar: [f64; 2],
    /// Pressure difference `p_i - p_j` [bar], without any gravity head.
    pub potential_difference_bar: f64,
    /// Phase volumetric rates `[liquid, vapour]` [m³/day], positive from `i` to `j`.
    pub phase_rates_m3_per_day: [f64; 2],
}

/// Everything one cell contributes to a face, evaluated once.
struct CellFaceData {
    state: FlashState,
    derivatives: FlashDerivatives,
    pressure_bar: f64,
}

fn evaluate_cell(
    spec: &FluidSpecification,
    cell_index: usize,
    cell: &CompositionalCellState,
) -> Result<CellFaceData, FluxError> {
    let z = cell.overall_composition();
    let p_pa = bar_to_pa(cell.pressure_bar);
    let t = spec.reservoir_temperature_k();
    let state = flash(spec, p_pa, t, &z, None).map_err(|source| FluxError::Flash {
        cell: cell_index,
        source,
    })?;
    let derivatives =
        flash_derivatives(spec, p_pa, t, &z, &state).map_err(|source| FluxError::Derivative {
            cell: cell_index,
            source,
        })?;
    Ok(CellFaceData {
        state,
        derivatives,
        pressure_bar: cell.pressure_bar,
    })
}

/// Component molar flux across one face, with its Jacobian on both neighbours.
///
/// `geom_t` is `DARCY_METRIC_FACTOR * geometric_transmissibility` [m³/day/bar per 1/cP], the same
/// quantity the black-oil assembler passes to `fim/flux.rs`.
pub fn face_flux(
    spec: &FluidSpecification,
    relperm: &RelativePermeabilityModel,
    geom_t: f64,
    gravity: Gravity,
    cell_i: (usize, &CompositionalCellState),
    cell_j: (usize, &CompositionalCellState),
) -> Result<FaceFlux, FluxError> {
    if !geom_t.is_finite() || geom_t < 0.0 {
        return Err(FluxError::InvalidTransmissibility { value: geom_t });
    }
    let n = spec.component_count();
    let i = evaluate_cell(spec, cell_i.0, cell_i.1)?;
    let j = evaluate_cell(spec, cell_j.0, cell_j.1)?;

    match n {
        2 => face_flux_sized::<2, 4>(spec, relperm, geom_t, gravity, &i, &j),
        3 => face_flux_sized::<3, 6>(spec, relperm, geom_t, gravity, &i, &j),
        4 => face_flux_sized::<4, 8>(spec, relperm, geom_t, gravity, &i, &j),
        count => Err(FluxError::UnsupportedComponentCount { count }),
    }
}

/// Seeds for one cell's quantities within the face's `2N`-slot derivative space.
///
/// `offset` is 0 for cell `i` and `N` for cell `j`. `FlashDerivatives` is per pascal in its
/// pressure slot and the primary is bar, so the conversion happens here, once, as the seeds are
/// built.
fn seed<const N: usize, const M: usize>(row: &[f64], offset: usize) -> [f64; M] {
    let mut out = [0.0; M];
    if !row.is_empty() {
        out[offset] = dpa_to_dbar(row[0]);
        out[offset + 1..offset + N].copy_from_slice(&row[1..N]);
    }
    out
}

/// Phase molar density, composition and viscosity of the upstream cell, as AD quantities.
struct UpstreamPhase<const M: usize> {
    /// **Liquid** saturation of the upstream cell, whichever phase this is.
    ///
    /// Both curves are keyed on the liquid saturation, so one value serves both phases and the
    /// two cannot be evaluated at inconsistent saturations.
    liquid_saturation: Ad<M>,
    /// Molar density [mol/m³].
    molar_density: Ad<M>,
    /// Viscosity [cP].
    viscosity_cp: Ad<M>,
    /// Phase composition.
    composition: Vec<Ad<M>>,
}

fn face_flux_sized<const N: usize, const M: usize>(
    spec: &FluidSpecification,
    relperm: &RelativePermeabilityModel,
    geom_t: f64,
    gravity: Gravity,
    i: &CellFaceData,
    j: &CellFaceData,
) -> Result<FaceFlux, FluxError> {
    debug_assert_eq!(M, 2 * N);

    let mut p_i_deriv = [0.0; M];
    p_i_deriv[0] = 1.0;
    let p_i = Ad::<M>::seeded(i.pressure_bar, p_i_deriv);
    let mut p_j_deriv = [0.0; M];
    p_j_deriv[N] = 1.0;
    let p_j = Ad::<M>::seeded(j.pressure_bar, p_j_deriv);
    let dp = p_i - p_j;
    let head_coefficient = gravity.head_coefficient();

    let mut component_flux = vec![Ad::<M>::constant(0.0); N];
    let mut phase_rates = [0.0f64; 2];
    let mut phase_potentials = [dp.value(); 2];
    let mut upstream = [UpstreamSide::First; 2];

    for (phase_index, liquid) in [(0usize, true), (1usize, false)] {
        // The gravity head needs both cells' mass densities, so it is built before the upstream
        // side is known — that is the whole reason gravity makes the phases disagree about which
        // way material moves.
        let rho_i = phase_mass_density::<N, M>(i, 0, liquid);
        let rho_j = phase_mass_density::<N, M>(j, N, liquid);
        let rho_avg = match (rho_i, rho_j) {
            (Some(a), Some(b)) => Some((a + b) * 0.5),
            // Present in one cell only: use that cell's density rather than averaging against a
            // number that describes nothing.
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let Some(rho_avg) = rho_avg else {
            // The phase exists nowhere on this face.
            continue;
        };

        let dphi = dp - rho_avg * head_coefficient;
        phase_potentials[phase_index] = dphi.value();

        // Branch on the value, then freeze. `dphi >= 0` selects the first cell, matching
        // `fim/flux.rs` exactly, including at zero — a convention rather than a physical fact.
        let upstream_is_first = dphi.value() >= 0.0;
        upstream[phase_index] = if upstream_is_first {
            UpstreamSide::First
        } else {
            UpstreamSide::Second
        };
        let (up, offset) = if upstream_is_first {
            (i, 0usize)
        } else {
            (j, N)
        };

        let Some(phase) = upstream_phase::<N, M>(spec, up, offset, liquid).map_err(|source| {
            FluxError::Transport {
                cell: if upstream_is_first { 0 } else { 1 },
                source,
            }
        })?
        else {
            // The phase is absent in the cell it would be drawn from, so it carries no material
            // across the face. Not a zero mobility multiplied by an undefined density — no term.
            continue;
        };

        let kr = relperm.kr(phase.liquid_saturation, liquid);
        let mobility = kr / phase.viscosity_cp;
        let q = mobility * dphi * geom_t;
        phase_rates[phase_index] = q.value();

        for (component, flux) in component_flux.iter_mut().enumerate() {
            *flux = *flux + q * phase.molar_density * phase.composition[component];
        }
    }

    let jacobian = component_flux
        .iter()
        .map(|f| f.deriv().to_vec())
        .collect::<Vec<_>>();

    Ok(FaceFlux {
        component_moles_per_day: component_flux.iter().map(|f| f.value()).collect(),
        jacobian,
        upstream,
        phase_potential_bar: phase_potentials,
        potential_difference_bar: dp.value(),
        phase_rates_m3_per_day: phase_rates,
    })
}

/// A phase's **mass** density in one cell, with derivatives seeded into that cell's half of the
/// face's derivative space. `None` when the phase is not present there.
fn phase_mass_density<const N: usize, const M: usize>(
    cell: &CellFaceData,
    offset: usize,
    liquid: bool,
) -> Option<Ad<M>> {
    let present = match (cell.state.phase_state, liquid) {
        (PhaseState::TwoPhase, _) => true,
        (PhaseState::SingleLiquid, true) => true,
        (PhaseState::SingleVapour, false) => true,
        _ => false,
    };
    if !present {
        return None;
    }
    let (props, row) = if liquid {
        (
            cell.state.liquid.as_ref().expect("liquid present"),
            &cell.derivatives.dliquid_mass_density,
        )
    } else {
        (
            cell.state.vapour.as_ref().expect("vapour present"),
            &cell.derivatives.dvapour_mass_density,
        )
    };
    Some(Ad::<M>::seeded(
        props.mass_density,
        seed::<N, M>(row, offset),
    ))
}

/// Build one phase's upstream quantities, or `None` when that phase is absent there.
fn upstream_phase<const N: usize, const M: usize>(
    spec: &FluidSpecification,
    up: &CellFaceData,
    offset: usize,
    liquid: bool,
) -> Result<Option<UpstreamPhase<M>>, TransportError> {
    let present = match (up.state.phase_state, liquid) {
        (PhaseState::TwoPhase, _) => true,
        (PhaseState::SingleLiquid, true) => true,
        (PhaseState::SingleVapour, false) => true,
        _ => false,
    };
    if !present {
        return Ok(None);
    }

    let props = if liquid {
        up.state.liquid.as_ref().expect("liquid present")
    } else {
        up.state.vapour.as_ref().expect("vapour present")
    };
    let d = &up.derivatives;
    let (dc, dcomp) = if liquid {
        (&d.dliquid_molar_density, &d.dx)
    } else {
        (&d.dvapour_molar_density, &d.dy)
    };

    let molar_density = Ad::<M>::seeded(props.molar_density, seed::<N, M>(dc, offset));
    let composition: Vec<Ad<M>> = (0..N)
        .map(|c| {
            let value = if liquid { up.state.x[c] } else { up.state.y[c] };
            Ad::<M>::seeded(value, seed::<N, M>(&dcomp[c], offset))
        })
        .collect();

    // The upstream cell's liquid saturation. In a single-phase state it is exactly one or zero
    // and carries no derivative: a constant, not a quantity that happens to sit at an endpoint.
    let liquid_saturation = match up.state.phase_state {
        PhaseState::SingleLiquid => Ad::<M>::constant(1.0),
        PhaseState::SingleVapour => Ad::<M>::constant(0.0),
        PhaseState::TwoPhase => {
            let cl = Ad::<M>::seeded(
                up.state.liquid.as_ref().expect("liquid").molar_density,
                seed::<N, M>(&d.dliquid_molar_density, offset),
            );
            let cv = Ad::<M>::seeded(
                up.state.vapour.as_ref().expect("vapour").molar_density,
                seed::<N, M>(&d.dvapour_molar_density, offset),
            );
            let beta = Ad::<M>::seeded(up.state.beta, seed::<N, M>(&d.dbeta, offset));
            let one = Ad::<M>::constant(1.0);
            let v_mix = (one - beta) / cl + beta / cv;
            let s_v = (beta / cv) / v_mix;
            one - s_v
        }
    };

    // LBC returns Pa·s; `geom_t` expects mobility in 1/cP. The conversion is
    // `crate::fluid::units::pa_s_to_cp` written over the `Scalar` trait, which that function
    // cannot be because it takes an `f64`.
    let viscosity_pa_s = lbc_viscosity::<Ad<M>>(
        spec,
        spec.reservoir_temperature_k(),
        &composition,
        molar_density,
    )?;
    let viscosity_cp = viscosity_pa_s / crate::fluid::units::PA_S_PER_CP;

    Ok(Some(UpstreamPhase {
        liquid_saturation,
        molar_density,
        viscosity_cp,
        composition,
    }))
}
