//! Component molar flux across a face (compositional plan, C9).
//!
//! # The equation
//!
//! For each phase `P` present in the upstream cell, at zero gravity and zero hydrocarbon
//! capillary pressure:
//!
//! ```text
//! dphi       = p_i - p_j                                    [bar]
//! q_P        = geom_t * (kr_P / mu_P)|upstream * dphi        [m3/day]
//! flux_i     = sum_P q_P * c_P|upstream * x_(P,i)|upstream   [mol/day]
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
//! # Zero gravity, zero capillary pressure
//!
//! Both are V1 scope decisions, not simplifications made here. Gravity gets its own subtask and
//! its own commit; hydrocarbon capillary pressure stays zero for V1 because unequal phase
//! pressures need a separately derived equilibrium contract and cannot be enabled by passing an
//! old flag. With both zero, every phase shares one potential difference — which is why the
//! upstream side is computed once per face and the per-phase structure below is nonetheless kept:
//! gravity is what makes the phases disagree, and the shape has to already be there.
//!
//! # Upstream weighting
//!
//! The branch is selected on the **value** of `dphi` and then frozen for the whole Jacobian
//! evaluation, matching `fim/flux.rs`'s `dphi >= 0.0` convention exactly. Freezing it is what
//! makes the Jacobian the derivative of a differentiable function: the upwind switch is not
//! differentiable at `dphi = 0`, and a Jacobian that tried to differentiate through it would be
//! differentiating a discontinuity.
//!
//! # Relative permeability — a declared choice, not sourced data
//!
//! See [`HydrocarbonRelPerm`]. This is the one place in the compositional model where V1 uses a
//! modelling assumption rather than a sourced dataset, and it is flagged in
//! `docs/COMPOSITIONAL_VALIDATION.md` §6 as still owed before any case is admitted.

use crate::ad::{Ad, Scalar};
use crate::fluid::derivatives::{DerivativeError, FlashDerivatives, flash_derivatives};
use crate::fluid::flash::{FlashError, FlashState, PhaseState, flash};
use crate::fluid::specification::FluidSpecification;
use crate::fluid::transport::{TransportError, lbc_viscosity};
use crate::fluid::units::{bar_to_pa, dpa_to_dbar};

use super::state::CompositionalCellState;

/// The hydrocarbon liquid/vapour relative permeability law.
///
/// **One variant, and it is a declared modelling choice rather than sourced data.** The plan is
/// explicit that the existing water/oil curves are not implicitly a hydrocarbon liquid/vapour law,
/// and no sourced hydrocarbon table is available in this environment — so rather than adapt a
/// curve that was fitted for a different pair of phases, or invent parameters, V1 declares
/// straight-line relative permeability and says so everywhere it matters.
///
/// Straight lines are the standard neutral choice for verifying compositional transport, because
/// they add no fitted parameters and no residual saturations: any error in a displacement front is
/// then attributable to the thermodynamics and the discretization rather than to a curve nobody
/// can cite. It is **not** a claim about any real rock.
///
/// A sourced table is still owed before a case is admitted to the catalog
/// (`docs/COMPOSITIONAL_VALIDATION.md` §6). When one arrives it becomes a second variant here;
/// the enum exists so that addition cannot be made silently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HydrocarbonRelPerm {
    /// `kr_L = S_L`, `kr_V = S_V`, no residual saturations, no endpoint scaling.
    StraightLine,
}

impl HydrocarbonRelPerm {
    /// Relative permeability of a phase at its saturation.
    pub fn kr<S: Scalar>(self, saturation: S) -> S {
        match self {
            Self::StraightLine => saturation,
        }
    }
}

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
    /// The frozen upstream side, per phase: `[liquid, vapour]`.
    pub upstream: [UpstreamSide; 2],
    /// Potential difference `p_i - p_j` [bar].
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
    relperm: HydrocarbonRelPerm,
    geom_t: f64,
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
        2 => face_flux_sized::<2, 4>(spec, relperm, geom_t, &i, &j),
        3 => face_flux_sized::<3, 6>(spec, relperm, geom_t, &i, &j),
        4 => face_flux_sized::<4, 8>(spec, relperm, geom_t, &i, &j),
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
    /// Saturation of this phase in the upstream cell.
    saturation: Ad<M>,
    /// Molar density [mol/m³].
    molar_density: Ad<M>,
    /// Viscosity [cP].
    viscosity_cp: Ad<M>,
    /// Phase composition.
    composition: Vec<Ad<M>>,
}

fn face_flux_sized<const N: usize, const M: usize>(
    spec: &FluidSpecification,
    relperm: HydrocarbonRelPerm,
    geom_t: f64,
    i: &CellFaceData,
    j: &CellFaceData,
) -> Result<FaceFlux, FluxError> {
    debug_assert_eq!(M, 2 * N);

    // The potential difference. With zero gravity and zero capillary pressure it is the pressure
    // difference, and every phase shares it.
    let mut p_i_deriv = [0.0; M];
    p_i_deriv[0] = 1.0;
    let p_i = Ad::<M>::seeded(i.pressure_bar, p_i_deriv);
    let mut p_j_deriv = [0.0; M];
    p_j_deriv[N] = 1.0;
    let p_j = Ad::<M>::seeded(j.pressure_bar, p_j_deriv);
    let dphi = p_i - p_j;

    // Branch on the value, then freeze. Matches `fim/flux.rs`'s `dphi >= 0.0`: at exactly zero the
    // first cell is upstream, which is a convention rather than a physical fact and is therefore
    // tested as one.
    let upstream_is_first = dphi.value() >= 0.0;
    let side = if upstream_is_first {
        UpstreamSide::First
    } else {
        UpstreamSide::Second
    };
    let (up, offset) = if upstream_is_first {
        (i, 0usize)
    } else {
        (j, N)
    };

    let mut component_flux = vec![Ad::<M>::constant(0.0); N];
    let mut phase_rates = [0.0f64; 2];

    for (phase_index, liquid) in [(0usize, true), (1usize, false)] {
        let Some(phase) = upstream_phase::<N, M>(spec, up, offset, liquid).map_err(|source| {
            FluxError::Transport {
                cell: if upstream_is_first { 0 } else { 1 },
                source,
            }
        })?
        else {
            // The phase is absent upstream, so it carries no material across the face. Not a zero
            // mobility multiplied by an undefined density — no term at all.
            continue;
        };

        let kr = relperm.kr(phase.saturation);
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
        upstream: [side, side],
        potential_difference_bar: dphi.value(),
        phase_rates_m3_per_day: phase_rates,
    })
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

    // Saturation. In a single-phase state it is exactly one and carries no derivative: it is a
    // constant, not a quantity that happens to sit at an endpoint.
    let saturation = match up.state.phase_state {
        PhaseState::SingleLiquid | PhaseState::SingleVapour => Ad::<M>::constant(1.0),
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
            if liquid { one - s_v } else { s_v }
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
        saturation,
        molar_density,
        viscosity_cp,
        composition,
    }))
}
