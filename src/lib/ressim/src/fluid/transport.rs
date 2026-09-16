//! Viscosity, saturations and the surface flash (compositional plan, C5).
//!
//! Three things the transport equations need from thermodynamics that the EOS and flash do not
//! themselves provide: phase viscosity, the volume-fraction saturations implied by a mole split,
//! and the conversion of a produced component stream into surface volumes.
//!
//! # Viscosity
//!
//! Lohrenz–Bray–Clark, transcribed from `/usr/include/opm/material/viscositymodels/LBC.hpp`
//! (`libopm-common-dev 2026.04-1~noble`), citing Lohrenz, Bray & Clark, JPT 16.10 (1964). That
//! header notes the paper's `-0.40758` is a typo for `-0.040758`, and carries the corrected value;
//! this does the same, and says so, because silently "fixing" a published coefficient is how two
//! implementations of the same correlation end up disagreeing.
//!
//! LBC does not follow from Peng–Robinson. It is a separate correlation with its own parameters —
//! critical volumes in particular, which no part of the EOS uses — and the plan is explicit that a
//! viscosity model does not come along for free with an EOS choice.
//!
//! **Molar density is an input, not something this module derives.** LBC reads the compressibility
//! factor off OPM's fluid state and recovers the density from `V = Z R T / p`; taking the density
//! directly is the same calculation with one fewer place for a gas constant to enter. That matters
//! here: OPM's `R` is the superseded `8.314472` and this crate's is the exact SI value, so passing
//! the density lets a test feed OPM's own value and compare the correlation exactly, instead of
//! absorbing a units difference into a tolerance. See `docs/COMPOSITIONAL_VALIDATION.md` §2.
//!
//! # Surface conditions
//!
//! A **single equilibrium stage**. The produced stream is flashed once at the declared surface
//! pressure and temperature, and the resulting phase mole rates are converted to volumes with the
//! phase molar densities at those conditions. This is not a separator train, and it deliberately
//! produces no `Bo`, `Bg` or `Rs`: those are black-oil quantities defined against a specific
//! fluid model, and reusing their names for an arbitrary-composition calculation would invite
//! exactly the misreading the plan forbids. The outputs are "surface liquid" and "surface gas"
//! volumes, and the reporting contract has to say what it means by them.

use super::eos::EosError;
use super::flash::{FlashError, FlashState, PhaseState, flash};
use super::specification::{FluidSpecification, SurfaceConditions};
use crate::ad::Scalar;

/// LBC's polynomial coefficients in reduced density.
///
/// `LBC.hpp`, with the fourth entry carrying that header's correction of the 1964 paper's
/// `-0.40758` typo to `-0.040758`.
const LBC_COEFFICIENTS: [f64; 5] = [0.10230, 0.023364, 0.058533, -0.040758, 0.0093324];

/// One standard atmosphere in Pa, from `opm/input/eclipse/Units/Units.hpp`: `atm = 101325 Pa`.
pub const STANDARD_PRESSURE_PA: f64 = 101_325.0;

/// Standard temperature in K.
///
/// `288.71 K` (15.56 °C, 60 °F), from
/// `opm/input/eclipse/EclipseState/Compositional/CompositionalConfig.hpp`, which sets
/// `standard_temperature = 288.71` and `standard_pressure = 1 * unit::atm` as the defaults for
/// **compositional** runs specifically. That is the source this crate uses; it is not inherited
/// from the black-oil path, whose standard-volume outputs keep their own separate meaning.
pub const STANDARD_TEMPERATURE_K: f64 = 288.71;

/// The surface conditions V1 pins: 1 atm, 288.71 K, single equilibrium stage.
///
/// This closes the open decision recorded by C0 in `docs/COMPOSITIONAL_VALIDATION.md` §6. It is
/// sourced rather than chosen: OPM's compositional configuration carries exactly these two values
/// as its defaults.
pub fn pinned_surface_conditions() -> SurfaceConditions {
    SurfaceConditions {
        pressure_pa: STANDARD_PRESSURE_PA,
        temperature_k: STANDARD_TEMPERATURE_K,
    }
}

/// Why a transport or surface calculation could not produce a result.
#[derive(Clone, Debug, PartialEq)]
pub enum TransportError {
    /// A phase composition has the wrong length, or is otherwise unusable.
    InvalidComposition { reason: &'static str },
    /// Molar density is not finite and positive.
    InvalidMolarDensity { value: f64 },
    /// A viscosity came out non-positive or non-finite.
    NonPhysicalViscosity { value: f64 },
    /// The surface flash failed.
    SurfaceFlash(FlashError),
    /// The specification has no surface conditions pinned.
    SurfaceConditionsNotPinned,
    /// An EOS evaluation failed.
    Eos(EosError),
}

impl core::fmt::Display for TransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidComposition { reason } => write!(f, "invalid phase composition: {reason}"),
            Self::InvalidMolarDensity { value } => {
                write!(f, "molar density {value} is not finite and positive")
            }
            Self::NonPhysicalViscosity { value } => write!(f, "viscosity evaluated to {value}"),
            Self::SurfaceFlash(e) => write!(f, "surface flash failed: {e}"),
            Self::SurfaceConditionsNotPinned => write!(
                f,
                "the fluid specification has no surface conditions; surface volumes are undefined \
                 without them"
            ),
            Self::Eos(e) => write!(f, "EOS evaluation failed: {e}"),
        }
    }
}

/// Lohrenz–Bray–Clark phase viscosity [Pa·s].
///
/// `x` is that phase's mole fractions and `molar_density` its molar density [mol/m³] at the same
/// conditions — the one the EOS produced for the branch in question, not the mixture's.
///
/// Generic over the scalar type so the same code yields the value with `f64` and its derivatives
/// with `Ad<N>`, exactly as the EOS does.
pub fn lbc_viscosity<S: Scalar>(
    spec: &FluidSpecification,
    temperature_k: f64,
    x: &[S],
    molar_density: S,
) -> Result<S, TransportError> {
    let n = spec.component_count();
    if x.len() != n {
        return Err(TransportError::InvalidComposition {
            reason: "length does not match the specification",
        });
    }
    if !(molar_density.value() > 0.0) || !molar_density.value().is_finite() {
        return Err(TransportError::InvalidMolarDensity {
            value: molar_density.value(),
        });
    }

    // MPa per atm, LBC.hpp's `MPa_atm`.
    const MPA_PER_ATM: f64 = 0.101325;

    // Pseudo-critical molar volume, from critical volumes in m³/kmol converted to m³/mol.
    let mut sum_volume = S::from_f64(0.0);
    for i in 0..n {
        let v_c = spec.component(i).critical_volume_m3_per_kmol / 1000.0;
        sum_volume = sum_volume + x[i] * v_c;
    }
    if !(sum_volume.value() > 0.0) {
        return Err(TransportError::InvalidComposition {
            reason: "pseudo-critical volume is not positive",
        });
    }
    // Reduced density. LBC.hpp recovers `rho` from `Z R T / p`; the caller has already done that
    // and hands the same quantity in, which keeps the gas constant out of this function entirely.
    let rho_r = molar_density * sum_volume;

    // Mixture pseudo-critical properties, in LBC's own units: Mm in kg/kmol, p_c in atm.
    let mut xsum_t_c = S::from_f64(0.0);
    let mut xsum_mm = S::from_f64(0.0);
    let mut xsum_p_ca = S::from_f64(0.0);
    for i in 0..n {
        let c = spec.component(i);
        let p_ca = (c.critical_pressure_pa / 1.0e6) / MPA_PER_ATM;
        xsum_t_c = xsum_t_c + x[i] * c.critical_temperature_k;
        xsum_mm = xsum_mm + x[i] * (c.molar_mass_kg_per_mol * 1000.0);
        xsum_p_ca = xsum_p_ca + x[i] * p_ca;
    }
    let zeta_tot = (xsum_t_c / (xsum_mm.powf(3.0) * xsum_p_ca.powf(4.0))).powf(1.0 / 6.0);

    // Dilute-gas mixture viscosity, Herning-Zipperer weighted by sqrt(Mm).
    let mut my0 = S::from_f64(0.0);
    let mut sum_xrm = S::from_f64(0.0);
    for i in 0..n {
        let c = spec.component(i);
        let p_ca = (c.critical_pressure_pa / 1.0e6) / MPA_PER_ATM;
        let mm = c.molar_mass_kg_per_mol * 1000.0;
        let zeta = (c.critical_temperature_k / (mm.powi(3) * p_ca.powi(4))).powf(1.0 / 6.0);
        let t_r = temperature_k / c.critical_temperature_k;
        let xrm = x[i] * mm.sqrt();
        // The correlation is piecewise in reduced temperature. The branch is selected on the
        // component's own `T_r`, which is a constant in this isothermal model, so no derivative
        // crosses the switch.
        let mys = if t_r <= 1.5 {
            34.0e-5 * t_r.powf(0.94) / zeta
        } else {
            17.78e-5 * (4.58 * t_r - 1.67).powf(0.625) / zeta
        };
        my0 = my0 + xrm * mys;
        sum_xrm = sum_xrm + xrm;
    }
    let my0 = my0 / sum_xrm;

    let mut sum_lbc = S::from_f64(0.0);
    let mut power = S::from_f64(1.0);
    for coefficient in LBC_COEFFICIENTS {
        sum_lbc = sum_lbc + power * coefficient;
        power = power * rho_r;
    }

    // mPa·s -> Pa·s.
    let viscosity = (my0 + (sum_lbc.powf(4.0) - 1.0e-4) / zeta_tot) / 1.0e3;
    if !(viscosity.value() > 0.0) || !viscosity.value().is_finite() {
        return Err(TransportError::NonPhysicalViscosity {
            value: viscosity.value(),
        });
    }
    Ok(viscosity)
}

/// Per-phase viscosities of a converged flash [Pa·s].
///
/// `None` for a phase that is not present. A viscosity for an absent phase is not zero — it is
/// undefined, and reporting zero invites a mobility calculation to use it.
pub fn flash_viscosities(
    spec: &FluidSpecification,
    temperature_k: f64,
    state: &FlashState,
) -> Result<(Option<f64>, Option<f64>), TransportError> {
    let liquid = match &state.liquid {
        Some(p) => Some(lbc_viscosity::<f64>(
            spec,
            temperature_k,
            &state.x,
            p.molar_density,
        )?),
        None => None,
    };
    let vapour = match &state.vapour {
        Some(p) => Some(lbc_viscosity::<f64>(
            spec,
            temperature_k,
            &state.y,
            p.molar_density,
        )?),
        None => None,
    };
    Ok((liquid, vapour))
}

/// Surface volumes from a produced component stream.
///
/// Rates are in mol/day and volumes in m³/day, but the calculation is rate-agnostic: the same
/// function converts a cumulative inventory in mol to volumes in m³. What matters is that the
/// input and output share one basis, which is why the field names say `moles` and `volume`
/// rather than borrowing `Sm3`.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceStream {
    /// Total moles in, unchanged by the separation. Reported so conservation is checkable at the
    /// boundary rather than assumed across it.
    pub total_moles: f64,
    /// Moles leaving in the surface liquid.
    pub liquid_moles: f64,
    /// Moles leaving in the surface gas.
    pub vapour_moles: f64,
    /// Surface liquid volume [m³ per unit of the input basis].
    pub liquid_volume: f64,
    /// Surface gas volume.
    pub vapour_volume: f64,
    /// Per-component moles in the surface liquid.
    pub liquid_component_moles: Vec<f64>,
    /// Per-component moles in the surface gas.
    pub vapour_component_moles: Vec<f64>,
    /// The phase state the stream separated into at surface conditions.
    pub phase_state: PhaseState,
}

impl SurfaceStream {
    /// An empty stream: no moles, no volume, and a phase state that reflects nothing being there.
    ///
    /// Defined explicitly so a shut-in well does not divide a zero rate by a molar density that
    /// was never computed. The plan asks for the zero-flow case to have an answer rather than an
    /// exception.
    pub fn zero(component_count: usize) -> Self {
        Self {
            total_moles: 0.0,
            liquid_moles: 0.0,
            vapour_moles: 0.0,
            liquid_volume: 0.0,
            vapour_volume: 0.0,
            liquid_component_moles: vec![0.0; component_count],
            vapour_component_moles: vec![0.0; component_count],
            phase_state: PhaseState::SingleLiquid,
        }
    }

    /// Gas-to-liquid ratio at surface conditions [m³/m³].
    ///
    /// Deliberately **not** called GOR. A black-oil GOR is a property of a specific fluid model
    /// with a specific separator convention; this is the ratio of two volumes produced by one
    /// declared equilibrium stage. `None` when there is no surface liquid to divide by.
    pub fn surface_gas_liquid_ratio(&self) -> Option<f64> {
        if self.liquid_volume > 0.0 {
            Some(self.vapour_volume / self.liquid_volume)
        } else {
            None
        }
    }
}

/// Separate a produced component stream at the specification's pinned surface conditions.
///
/// `component_moles` is the stream's composition in moles (or mol/day — see [`SurfaceStream`]).
/// Total component moles are conserved exactly: the separation redistributes them between two
/// phases and creates none.
pub fn surface_separation(
    spec: &FluidSpecification,
    component_moles: &[f64],
) -> Result<SurfaceStream, TransportError> {
    let n = spec.component_count();
    if component_moles.len() != n {
        return Err(TransportError::InvalidComposition {
            reason: "stream length does not match the specification",
        });
    }
    for &m in component_moles {
        if !m.is_finite() || m < 0.0 {
            return Err(TransportError::InvalidComposition {
                reason: "stream has a negative or non-finite component",
            });
        }
    }
    let surface = spec
        .surface()
        .ok_or(TransportError::SurfaceConditionsNotPinned)?;

    let total: f64 = component_moles.iter().sum();
    if total == 0.0 {
        return Ok(SurfaceStream::zero(n));
    }
    let z: Vec<f64> = component_moles.iter().map(|m| m / total).collect();

    let state = flash(spec, surface.pressure_pa, surface.temperature_k, &z, None)
        .map_err(TransportError::SurfaceFlash)?;

    // beta is the vapour mole fraction, so these are mole rates, not volumes.
    let vapour_moles = total * state.beta;
    let liquid_moles = total - vapour_moles;

    let liquid_component_moles: Vec<f64> = (0..n).map(|i| liquid_moles * state.x[i]).collect();
    let vapour_component_moles: Vec<f64> = (0..n).map(|i| vapour_moles * state.y[i]).collect();

    // Volume = moles / molar density, evaluated at surface conditions for the phase that exists.
    // An absent phase contributes exactly zero volume and its molar density is never read — not
    // multiplied by a zero mole count, but never read, because it does not exist.
    let liquid_volume = match &state.liquid {
        Some(p) => liquid_moles / p.molar_density,
        None => 0.0,
    };
    let vapour_volume = match &state.vapour {
        Some(p) => vapour_moles / p.molar_density,
        None => 0.0,
    };

    Ok(SurfaceStream {
        total_moles: total,
        liquid_moles,
        vapour_moles,
        liquid_volume,
        vapour_volume,
        liquid_component_moles,
        vapour_component_moles,
        phase_state: state.phase_state,
    })
}
