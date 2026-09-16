//! Compositional wells: connection sources and controls (compositional plan, C11).
//!
//! **Read [`docs/COMPOSITIONAL_WELL_DESIGN.md`] before this file.** It states the unknowns, the
//! connection law, the injection rule, the controls and — importantly — the provenance: this is
//! derived from first principles plus the repository's own Peaceman geometry, because C11's named
//! OPM reference is not available in this environment. No OPM well header was read, so no claim of
//! agreement with OPM's well model is made or gated.
//!
//! [`docs/COMPOSITIONAL_WELL_DESIGN.md`]: ../../../../docs/COMPOSITIONAL_WELL_DESIGN.md
//!
//! # Sign convention
//!
//! **Positive `source_i` means moles entering the cell.** A producer's sources are negative and an
//! injector's positive, which is what C8's residual `R_i = n_i(new) - n_i(prev) - dt*source_i`
//! expects. Getting this backwards produces a well that fills the reservoir while reporting
//! production, and every test below that checks a sign is checking this one.
//!
//! # One completion
//!
//! Multiple completions sharing a BHP are a separate commit, as the plan requires. With a single
//! connection there is no crossflow to implement or clip, and no wellbore mixing to resolve — the
//! producer's stream is the cell's and the injector's is prescribed. That is why no wellbore flash
//! appears here, and it is exactly what changes when a second completion is added.

use crate::ad::Ad;
use crate::fluid::derivatives::{DerivativeError, FlashDerivatives, flash_derivatives};
use crate::fluid::flash::{FlashError, FlashState, PhaseState, flash};
use crate::fluid::specification::FluidSpecification;
use crate::fluid::transport::{TransportError, lbc_viscosity, surface_separation};
use crate::fluid::units::{PA_S_PER_CP, bar_to_pa, dpa_to_dbar};

use super::flux::HydrocarbonRelPerm;
use super::state::CompositionalCellState;

/// What a well is controlled on.
///
/// Units are in the variant names because "rate" means three different quantities here.
#[derive(Clone, Debug, PartialEq)]
pub enum WellControl {
    /// Bottom-hole pressure at the datum [bar].
    Bhp { target_bar: f64 },
    /// Total component molar rate [mol/day], signed: positive injects.
    ///
    /// `bhp_limit_bar` is a maximum for an injector and a minimum for a producer. When it binds,
    /// the well reverts to BHP control at the limit and reports what it achieved.
    MolarRate {
        target_mol_per_day: f64,
        bhp_limit_bar: f64,
    },
    /// Surface volumetric rate [m³/day] at the specification's pinned surface conditions.
    ///
    /// **Not a reservoir rate and not a black-oil RESV target.** The conversion is C5's
    /// single-stage equilibrium flash; the two differ by the formation volume factor of a fluid
    /// whose composition is changing.
    SurfaceRate {
        /// Positive for injection of the prescribed composition, negative for production.
        target_m3_per_day: f64,
        /// Which surface phase the target refers to for a producer.
        phase: SurfacePhase,
        bhp_limit_bar: f64,
    },
}

/// Which surface stream a volumetric target refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfacePhase {
    Liquid,
    Vapour,
    /// Both, summed. Rarely what anyone means, but unambiguous when it is.
    Total,
}

/// A single-completion compositional well.
#[derive(Clone, Debug, PartialEq)]
pub struct CompositionalWell {
    /// Stable identifier for reporting.
    pub id: String,
    /// The connected cell.
    pub cell: usize,
    /// Geometric well index [m³·cP/(day·bar)] — Peaceman **without** mobility folded in.
    pub well_index: f64,
    /// Hydrostatic head from the datum down to this completion [bar]; `p_conn = bhp + head`.
    pub head_offset_bar: f64,
    pub control: WellControl,
    /// Prescribed injection composition, `N` overall mole fractions. Required for an injector.
    pub injection_composition: Option<Vec<f64>>,
}

/// Why a well source could not be evaluated.
#[derive(Clone, Debug, PartialEq)]
pub enum WellError {
    /// The connected cell's flash failed.
    CellFlash(FlashError),
    /// The cell's flash could not be differentiated.
    CellDerivative(DerivativeError),
    /// The injection stream's flash failed.
    InjectionFlash(FlashError),
    /// A viscosity could not be evaluated.
    Transport(TransportError),
    /// An injector has no prescribed composition, or it is invalid.
    MissingInjectionComposition { well: String },
    /// The well index is negative or not finite.
    InvalidWellIndex { value: f64 },
    /// No AD instantiation exists for this component count.
    UnsupportedComponentCount { count: usize },
    /// A rate target cannot be reached at any BHP inside the admissible range.
    UnreachableRate {
        well: String,
        target: f64,
        achieved_at_limit: f64,
    },
    /// The surface conversion needs surface conditions the specification does not carry.
    SurfaceConditionsNotPinned,
}

impl core::fmt::Display for WellError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::CellFlash(e) => write!(f, "connected cell's flash failed: {e}"),
            Self::CellDerivative(e) => write!(f, "connected cell's derivatives failed: {e}"),
            Self::InjectionFlash(e) => write!(f, "injection stream's flash failed: {e}"),
            Self::Transport(e) => write!(f, "{e}"),
            Self::MissingInjectionComposition { well } => {
                write!(f, "well {well} injects but has no prescribed composition")
            }
            Self::InvalidWellIndex { value } => {
                write!(f, "well index {value} is not finite and non-negative")
            }
            Self::UnsupportedComponentCount { count } => {
                write!(f, "no AD instantiation compiled for {count} components")
            }
            Self::UnreachableRate {
                well,
                target,
                achieved_at_limit,
            } => write!(
                f,
                "well {well}: a target of {target} cannot be reached; the BHP limit achieves \
                 {achieved_at_limit}"
            ),
            Self::SurfaceConditionsNotPinned => write!(
                f,
                "surface volumetric control needs pinned surface conditions on the specification"
            ),
        }
    }
}

/// A well's contribution to one cell, and its derivatives.
#[derive(Clone, Debug, PartialEq)]
pub struct WellSource {
    /// Component molar sources [mol/day]. Positive means entering the cell.
    pub component_moles_per_day: Vec<f64>,
    /// `d source_i / d u`, row-major `N x N`, `u = [p_bar, z_0..z_(N-2)]` of the connected cell.
    pub cell_jacobian: Vec<Vec<f64>>,
    /// `d source_i / d bhp` [mol/day/bar], one entry per component.
    pub bhp_derivative: Vec<f64>,
    /// The BHP actually used [bar] — the target, or the limit when it bound.
    pub bhp_bar: f64,
    /// Total reservoir volumetric rate at connection conditions [m³/day], signed into the cell.
    pub reservoir_rate_m3_per_day: f64,
    /// True when a rate target was overridden by its BHP limit.
    pub on_bhp_limit: bool,
}

impl WellSource {
    /// Total molar rate into the cell [mol/day]. Negative for a producer.
    pub fn total_moles_per_day(&self) -> f64 {
        self.component_moles_per_day.iter().sum()
    }

    /// True when the well is producing, i.e. removing material.
    pub fn is_producing(&self) -> bool {
        self.total_moles_per_day() < 0.0
    }
}

/// Evaluate a well's source at a given BHP, ignoring its control.
///
/// Public because the rate-control solve drives it, and because a caller that wants to ask "what
/// would this well do at that pressure" should not have to construct a fake control to find out.
pub fn source_at_bhp(
    spec: &FluidSpecification,
    relperm: HydrocarbonRelPerm,
    well: &CompositionalWell,
    cell: &CompositionalCellState,
    bhp_bar: f64,
) -> Result<WellSource, WellError> {
    if !well.well_index.is_finite() || well.well_index < 0.0 {
        return Err(WellError::InvalidWellIndex {
            value: well.well_index,
        });
    }
    let n = spec.component_count();
    match n {
        2 => source_sized::<2>(spec, relperm, well, cell, bhp_bar),
        3 => source_sized::<3>(spec, relperm, well, cell, bhp_bar),
        4 => source_sized::<4>(spec, relperm, well, cell, bhp_bar),
        count => Err(WellError::UnsupportedComponentCount { count }),
    }
}

/// Seed a flash-derivative row into the well's slot layout.
///
/// `N` is the cell's primary count and `M = N + 1` is the slot count, so the row occupies slots
/// `0..N` and the BHP slot stays zero. Writing this over `M` alone is an off-by-one that indexes
/// a length-`N` row with `N + 1` and panics, which is how it was found.
fn seed_cell<const N: usize, const M: usize>(row: &[f64]) -> [f64; M] {
    let mut out = [0.0; M];
    if !row.is_empty() {
        out[0] = dpa_to_dbar(row[0]);
        out[1..N].copy_from_slice(&row[1..N]);
    }
    out
}

/// Seed the **pressure** derivative of the injected stream into the BHP slot.
///
/// The injected fluid is evaluated at `p_conn = bhp + head`, so its density, composition and
/// viscosity all move with the BHP and nothing else. Leaving that out would freeze the injected
/// stream's mobility and give a `dS/dbhp` that finite differences immediately disagree with.
fn seed_bhp<const N: usize, const M: usize>(row: &[f64]) -> [f64; M] {
    let mut out = [0.0; M];
    if !row.is_empty() {
        out[N] = dpa_to_dbar(row[0]);
    }
    out
}

/// AD over `N + 1` slots: the cell's `N` primaries, then `bhp`.
///
/// One evaluation gives both the cell block and the BHP column. Producing them separately is how a
/// BHP derivative ends up disagreeing with the cell derivative it shares every term with.
fn source_sized<const N: usize>(
    spec: &FluidSpecification,
    relperm: HydrocarbonRelPerm,
    well: &CompositionalWell,
    cell: &CompositionalCellState,
    bhp_bar: f64,
) -> Result<WellSource, WellError> {
    // Slots: 0 = p_cell, 1..N = independent z, N = bhp. `Ad` needs a const size, so the four
    // supported component counts are instantiated explicitly.
    match N {
        2 => source_ad::<2, 3>(spec, relperm, well, cell, bhp_bar),
        3 => source_ad::<3, 4>(spec, relperm, well, cell, bhp_bar),
        4 => source_ad::<4, 5>(spec, relperm, well, cell, bhp_bar),
        count => Err(WellError::UnsupportedComponentCount { count }),
    }
}

fn source_ad<const N: usize, const M: usize>(
    spec: &FluidSpecification,
    relperm: HydrocarbonRelPerm,
    well: &CompositionalWell,
    cell: &CompositionalCellState,
    bhp_bar: f64,
) -> Result<WellSource, WellError> {
    debug_assert_eq!(M, N + 1);
    let t = spec.reservoir_temperature_k();

    let mut p_deriv = [0.0; M];
    p_deriv[0] = 1.0;
    let p_cell = Ad::<M>::seeded(cell.pressure_bar, p_deriv);

    let mut bhp_deriv = [0.0; M];
    bhp_deriv[N] = 1.0;
    let bhp = Ad::<M>::seeded(bhp_bar, bhp_deriv);
    let p_conn = bhp + well.head_offset_bar;
    let dp = p_cell - p_conn;

    let injecting = dp.value() < 0.0 && well.injection_composition.is_some();

    let mut component_flux = vec![Ad::<M>::constant(0.0); N];
    let reservoir_rate;

    if injecting {
        // --- Injector: the prescribed stream, at its own total mobility.
        let z_inj = well.injection_composition.as_ref().ok_or_else(|| {
            WellError::MissingInjectionComposition {
                well: well.id.clone(),
            }
        })?;
        let p_conn_pa = bar_to_pa(p_conn.value());
        let inj_state =
            flash(spec, p_conn_pa, t, z_inj, None).map_err(WellError::InjectionFlash)?;
        let inj_derivatives = flash_derivatives(spec, p_conn_pa, t, z_inj, &inj_state)
            .map_err(WellError::CellDerivative)?;

        // The injected fluid's own saturations sum to one, so no phase is evaluated at a
        // saturation belonging to a different fluid and the injectivity cannot vanish for the
        // wrong reason. See the design note.
        let mut lambda = Ad::<M>::constant(0.0);
        for liquid in [true, false] {
            let Some(phase) =
                injected_phase::<N, M>(spec, relperm, &inj_state, &inj_derivatives, liquid)
                    .map_err(WellError::Transport)?
            else {
                continue;
            };
            lambda = lambda + phase.mobility;
        }

        // Positive into the cell. Every injected-stream property carries a BHP derivative and no
        // cell-primary derivative: the cell's own fluid plays no part in what is injected, which
        // is what makes injection auditable. The *rate* still depends on the cell pressure
        // through the drawdown, which is not the same thing.
        let q = (-dp) * lambda * well.well_index;
        reservoir_rate = q.value();
        let c_mixture = mixture_molar_density::<N, M>(&inj_state, &inj_derivatives);
        for (i, flux) in component_flux.iter_mut().enumerate() {
            *flux = q * c_mixture * z_inj[i];
        }
    } else if dp.value() > 0.0 {
        // --- Producer: the cell's own fluid.
        let z = cell.overall_composition();
        let state =
            flash(spec, bar_to_pa(cell.pressure_bar), t, &z, None).map_err(WellError::CellFlash)?;
        let derivatives = flash_derivatives(spec, bar_to_pa(cell.pressure_bar), t, &z, &state)
            .map_err(WellError::CellDerivative)?;

        let mut total_rate = Ad::<M>::constant(0.0);
        for (index, liquid) in [(0usize, true), (1usize, false)] {
            let _ = index;
            let Some(phase) = cell_phase::<N, M>(spec, relperm, &state, &derivatives, liquid)
                .map_err(WellError::Transport)?
            else {
                continue;
            };
            let q = phase.mobility * dp * well.well_index;
            total_rate = total_rate + q;
            for (i, flux) in component_flux.iter_mut().enumerate() {
                // Negative: production removes moles from the cell.
                *flux = *flux - q * phase.molar_density * phase.composition[i];
            }
        }
        reservoir_rate = -total_rate.value();
    } else {
        // dp == 0, or a producer with dp < 0 and no injection composition. A single-completion
        // producer flowing backwards is not production, and there is no specified wellbore
        // composition to inject, so the rate is zero rather than something invented.
        reservoir_rate = 0.0;
    }

    let mut cell_jacobian = vec![vec![0.0; N]; N];
    let mut bhp_derivative = vec![0.0; N];
    for (i, flux) in component_flux.iter().enumerate() {
        cell_jacobian[i].copy_from_slice(&flux.deriv()[..N]);
        bhp_derivative[i] = flux.d(N);
    }

    Ok(WellSource {
        component_moles_per_day: component_flux.iter().map(|f| f.value()).collect(),
        cell_jacobian,
        bhp_derivative,
        bhp_bar,
        reservoir_rate_m3_per_day: reservoir_rate,
        on_bhp_limit: false,
    })
}

/// Mixture molar density `1 / v_mix` of a flashed stream, with its pressure derivative in the BHP
/// slot.
fn mixture_molar_density<const N: usize, const M: usize>(
    state: &FlashState,
    d: &FlashDerivatives,
) -> Ad<M> {
    match state.phase_state {
        PhaseState::SingleLiquid => Ad::<M>::seeded(
            state.liquid.as_ref().expect("liquid").molar_density,
            seed_bhp::<N, M>(&d.dliquid_molar_density),
        ),
        PhaseState::SingleVapour => Ad::<M>::seeded(
            state.vapour.as_ref().expect("vapour").molar_density,
            seed_bhp::<N, M>(&d.dvapour_molar_density),
        ),
        PhaseState::TwoPhase => {
            let cl = Ad::<M>::seeded(
                state.liquid.as_ref().expect("liquid").molar_density,
                seed_bhp::<N, M>(&d.dliquid_molar_density),
            );
            let cv = Ad::<M>::seeded(
                state.vapour.as_ref().expect("vapour").molar_density,
                seed_bhp::<N, M>(&d.dvapour_molar_density),
            );
            let beta = Ad::<M>::seeded(state.beta, seed_bhp::<N, M>(&d.dbeta));
            let one = Ad::<M>::constant(1.0);
            let v_mix = (one - beta) / cl + beta / cv;
            one / v_mix
        }
    }
}

/// One phase of the **injected** stream, with derivatives in the BHP slot only.
fn injected_phase<const N: usize, const M: usize>(
    spec: &FluidSpecification,
    relperm: HydrocarbonRelPerm,
    state: &FlashState,
    d: &FlashDerivatives,
    liquid: bool,
) -> Result<Option<CellPhase<M>>, TransportError> {
    phase_common::<N, M>(spec, relperm, state, d, liquid, true)
}

/// The connected cell's phase properties as AD quantities over the well's slot layout.
struct CellPhase<const M: usize> {
    mobility: Ad<M>,
    molar_density: Ad<M>,
    composition: Vec<Ad<M>>,
}

fn cell_phase<const N: usize, const M: usize>(
    spec: &FluidSpecification,
    relperm: HydrocarbonRelPerm,
    state: &FlashState,
    d: &FlashDerivatives,
    liquid: bool,
) -> Result<Option<CellPhase<M>>, TransportError> {
    phase_common::<N, M>(spec, relperm, state, d, liquid, false)
}

/// Build one phase's mobility, molar density and composition as AD quantities.
///
/// `into_bhp_slot` chooses where the derivatives land: the cell's primaries for the connected
/// cell's fluid, or the BHP slot for the injected stream, which is evaluated at `p_conn`. The two
/// share every formula, which is the point — a separate injected-fluid path would be a second
/// place for the saturation and mobility expressions to drift apart.
fn phase_common<const N: usize, const M: usize>(
    spec: &FluidSpecification,
    relperm: HydrocarbonRelPerm,
    state: &FlashState,
    d: &FlashDerivatives,
    liquid: bool,
    into_bhp_slot: bool,
) -> Result<Option<CellPhase<M>>, TransportError> {
    let present = match (state.phase_state, liquid) {
        (PhaseState::TwoPhase, _) => true,
        (PhaseState::SingleLiquid, true) => true,
        (PhaseState::SingleVapour, false) => true,
        _ => false,
    };
    if !present {
        return Ok(None);
    }

    let place = |row: &[f64]| -> [f64; M] {
        if into_bhp_slot {
            seed_bhp::<N, M>(row)
        } else {
            seed_cell::<N, M>(row)
        }
    };

    let props = if liquid {
        state.liquid.as_ref().expect("liquid")
    } else {
        state.vapour.as_ref().expect("vapour")
    };
    let (dc, dcomp) = if liquid {
        (&d.dliquid_molar_density, &d.dx)
    } else {
        (&d.dvapour_molar_density, &d.dy)
    };

    let molar_density = Ad::<M>::seeded(props.molar_density, place(dc));
    let composition: Vec<Ad<M>> = (0..N)
        .map(|c| {
            let value = if liquid { state.x[c] } else { state.y[c] };
            Ad::<M>::seeded(value, place(&dcomp[c]))
        })
        .collect();

    let saturation = match state.phase_state {
        PhaseState::SingleLiquid | PhaseState::SingleVapour => Ad::<M>::constant(1.0),
        PhaseState::TwoPhase => {
            let cl = Ad::<M>::seeded(
                state.liquid.as_ref().expect("liquid").molar_density,
                place(&d.dliquid_molar_density),
            );
            let cv = Ad::<M>::seeded(
                state.vapour.as_ref().expect("vapour").molar_density,
                place(&d.dvapour_molar_density),
            );
            let beta = Ad::<M>::seeded(state.beta, place(&d.dbeta));
            let one = Ad::<M>::constant(1.0);
            let v_mix = (one - beta) / cl + beta / cv;
            let s_v = (beta / cv) / v_mix;
            if liquid { one - s_v } else { s_v }
        }
    };

    let mu_pa_s = lbc_viscosity::<Ad<M>>(
        spec,
        spec.reservoir_temperature_k(),
        &composition,
        molar_density,
    )?;
    let mobility = relperm.kr(saturation) / (mu_pa_s / PA_S_PER_CP);

    Ok(Some(CellPhase {
        mobility,
        molar_density,
        composition,
    }))
}

/// Largest BHP considered when solving a rate target [bar].
const BHP_SEARCH_MAX_BAR: f64 = 2000.0;
/// Smallest BHP considered [bar].
const BHP_SEARCH_MIN_BAR: f64 = 1.0;
/// Bracketing tolerance on BHP [bar].
const BHP_SOLVE_TOLERANCE_BAR: f64 = 1e-9;

/// Evaluate a well under its control, solving for BHP when the control is a rate.
pub fn well_source(
    spec: &FluidSpecification,
    relperm: HydrocarbonRelPerm,
    well: &CompositionalWell,
    cell: &CompositionalCellState,
) -> Result<WellSource, WellError> {
    match &well.control {
        WellControl::Bhp { target_bar } => source_at_bhp(spec, relperm, well, cell, *target_bar),
        WellControl::MolarRate {
            target_mol_per_day,
            bhp_limit_bar,
        } => solve_for_rate(
            spec,
            relperm,
            well,
            cell,
            *bhp_limit_bar,
            *target_mol_per_day,
            |source, _| Ok(source.total_moles_per_day()),
        ),
        WellControl::SurfaceRate {
            target_m3_per_day,
            phase,
            bhp_limit_bar,
        } => {
            let phase = *phase;
            solve_for_rate(
                spec,
                relperm,
                well,
                cell,
                *bhp_limit_bar,
                *target_m3_per_day,
                move |source, spec| surface_rate(spec, source, phase),
            )
        }
    }
}

/// Surface volumetric rate of a source [m³/day], signed the same way the source is.
fn surface_rate(
    spec: &FluidSpecification,
    source: &WellSource,
    phase: SurfacePhase,
) -> Result<f64, WellError> {
    if spec.surface().is_none() {
        return Err(WellError::SurfaceConditionsNotPinned);
    }
    let total: f64 = source.component_moles_per_day.iter().sum();
    if total == 0.0 {
        return Ok(0.0);
    }
    // The separation works on a non-negative stream, so a producer's stream is negated and the
    // sign reapplied afterwards. Flashing a negative composition would be flashing nothing.
    let sign = if total < 0.0 { -1.0 } else { 1.0 };
    let stream: Vec<f64> = source
        .component_moles_per_day
        .iter()
        .map(|m| sign * m)
        .collect();
    let separated = surface_separation(spec, &stream).map_err(WellError::Transport)?;
    let volume = match phase {
        SurfacePhase::Liquid => separated.liquid_volume,
        SurfacePhase::Vapour => separated.vapour_volume,
        SurfacePhase::Total => separated.liquid_volume + separated.vapour_volume,
    };
    Ok(sign * volume)
}

/// Solve for the BHP that achieves `target` under `measure`, applying the BHP limit.
///
/// `measure` is monotone in BHP — raising the BHP reduces production and increases injection — so a
/// bracketed bisection is unconditionally safe here. Bisection rather than Newton because
/// `measure` may be a surface rate, whose derivative would require differentiating a second flash,
/// and because a well control solve is not where the run's time goes.
fn solve_for_rate<F>(
    spec: &FluidSpecification,
    relperm: HydrocarbonRelPerm,
    well: &CompositionalWell,
    cell: &CompositionalCellState,
    bhp_limit_bar: f64,
    target: f64,
    measure: F,
) -> Result<WellSource, WellError>
where
    F: Fn(&WellSource, &FluidSpecification) -> Result<f64, WellError>,
{
    let injecting = target > 0.0;

    // The limit is a maximum for an injector and a minimum for a producer, so it is one end of the
    // search interval and the "hardest achievable" point.
    let (mut lo, mut hi) = if injecting {
        (BHP_SEARCH_MIN_BAR, bhp_limit_bar.min(BHP_SEARCH_MAX_BAR))
    } else {
        (bhp_limit_bar.max(BHP_SEARCH_MIN_BAR), BHP_SEARCH_MAX_BAR)
    };
    if !(lo < hi) {
        let mut at_limit = source_at_bhp(spec, relperm, well, cell, bhp_limit_bar)?;
        at_limit.on_bhp_limit = true;
        return Ok(at_limit);
    }

    // At the limit the well is working as hard as it is allowed to.
    let limit_bhp = if injecting { hi } else { lo };
    let mut at_limit = source_at_bhp(spec, relperm, well, cell, limit_bhp)?;
    let achieved_at_limit = measure(&at_limit, spec)?;

    let limit_binds = if injecting {
        achieved_at_limit < target
    } else {
        achieved_at_limit > target
    };
    if limit_binds {
        // The target is beyond what the limit allows: revert to BHP control at the limit and
        // report what was achieved, rather than pretending the target was met.
        at_limit.on_bhp_limit = true;
        return Ok(at_limit);
    }

    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        let source = source_at_bhp(spec, relperm, well, cell, mid)?;
        let value = measure(&source, spec)?;

        if hi - lo < BHP_SOLVE_TOLERANCE_BAR {
            return Ok(source);
        }
        // `measure` increases with BHP for an injector and decreases for a producer.
        let too_little = if injecting {
            value < target
        } else {
            value > target
        };
        if too_little {
            if injecting {
                lo = mid;
            } else {
                hi = mid;
            }
        } else if injecting {
            hi = mid;
        } else {
            lo = mid;
        }
    }

    let source = source_at_bhp(spec, relperm, well, cell, 0.5 * (lo + hi))?;
    Ok(source)
}
